#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# P13 benchmark harness: Leandro v1 (vhost-user-nvrm, API forwarding), v2 Caraxes
# (vfio-user-nvgpu, stock guest driver) and native, the same binaries on each side.
# Method, conditions and results: docs/caraxes/benchmark.md.
#
#   bench-run.sh stage  TARGET                  copy target/bench/bin/* into the guest's ~/bench
#   bench-run.sh series --targets "native v1 v2" --out DIR [--reps 5] [--metrics "..."]
#   bench-run.sh startup TARGET --out DIR [--reps 5]   modprobe (v2) / module load (v1) to first cuInit
#   bench-run.sh v2-restart | v2-nodes     restart the v2 VM and device (down, up, probe, nodes) |
#                                          only load nvidia-uvm and make the /dev/nvidia* nodes
# V1_RESTART=1 / V2_RESTART=1: restart that path's VM and device process before every rep but the
# first (v1: freed pinned registrations are kept until guest restart; v2: symmetric conditions).
#
# Targets: native (this host, target/bench/bin, host venv ../Leandro/vendor/hostvenv),
# v1 (caraxes/tools/bench-v1vm.sh, 192.168.105.25), v2 (tools/caraxes-vm.sh with VM, CARAXES_IP ...
# as exported by the caller). Binaries: bench-cu (caraxes/tools/bench-cu.c) and the Leandro-Test
# probes nvprobe, ioctlping, ctrlping, all built once in a guest (glibc 2.39, cuda.h 13.0,
# libcuda 610.57.04) and copied to target/bench/bin; Python probes: Leandro-Test probe/python
# (read in place on the host, synced by `tools/caraxes-vm.sh sync-tests` into guests).
# Metrics (P13): launch-{auto,blocking} (nvprobe 4, 100 x 30M-cycle kernels),
# short-{auto,blocking} (nvprobe 4, NVPROBE_CYCLES=1000), copy (bench-cu copy), mm (mmsweep
# 1024/2048/4096), rl (rlprobe 50 episodes), init (bench-cu init), time0/time1 (bash time of
# nvprobe 0 / 1; the host has no /usr/bin/time), ioctl (ioctlping 10000), ctrl (ctrlping 2000 x 64/512/4096/16384),
# churn (bench-cu churn 20), ppo (caraxes/tools/pporl.py, PPO CartPole with 128 vector envs for
# PPO_SECONDS, default 60: steps per second and time to solved; not in ALL_METRICS).
# The rep loop is outermost and the targets interleave inside it, so drift (temperature,
# desktop load) spreads over all targets. Each run writes DIR/<target>/<metric>-<rep>.txt:
# "# " header lines (time, target, command, GPU state, running VMs), the raw output, then
# "# MEAS" with the in-target wall time, the device process's utime+stime delta and RSS
# (v1: vhost-user-nvrm, v2: vfio-user-nvgpu), the VMM's CPU delta, and the CPU delta of
# every OTHER device/VMM process (concurrent load marker).
set -uo pipefail
ROOT=$(cd "$(dirname "$(readlink -f "$0")")/../.." && pwd -P)
LT=${LT:-$(cd "$ROOT/.." && pwd -P)/Leandro-Test}
BIN=$ROOT/target/bench/bin
HOSTPY=${LEA_HOSTVENV:-$(cd "$ROOT/.." && pwd -P)/Leandro/vendor/hostvenv}/bin/python
TICK=$(getconf CLK_TCK)
ALL_METRICS="launch-auto launch-blocking short-auto short-blocking copy mm rl init time0 time1 ioctl ctrl churn"

err() { echo "ERROR: $*" >&2; exit 1; }

# The command of a metric, as a shell string run inside the target from its bench dir.
metric_cmd() {
    local m=$1 py=$2 pyd=$3
    case $m in
        launch-auto)     echo 'NVPROBE_ITERS=100 NVPROBE_SCHED=auto ./nvprobe 4' ;;
        launch-blocking) echo 'NVPROBE_ITERS=100 NVPROBE_SCHED=blocking ./nvprobe 4' ;;
        short-auto)      echo 'NVPROBE_ITERS=100 NVPROBE_CYCLES=1000 NVPROBE_SCHED=auto ./nvprobe 4' ;;
        short-blocking)  echo 'NVPROBE_ITERS=100 NVPROBE_CYCLES=1000 NVPROBE_SCHED=blocking ./nvprobe 4' ;;
        copy)            echo './bench-cu copy 10' ;;
        mm)              echo "NVMM_SIZES=1024,2048,4096 NVMM_ITERS=10 $py $pyd/mmsweep.py" ;;
        rl)              echo "NVRL_EPISODES=50 $py $pyd/rlprobe.py" ;;
        init)            echo './bench-cu init' ;;
        time0)           echo "TIMEFORMAT='TIME wall_s=%3R user_s=%3U sys_s=%3S'; time ./nvprobe 0" ;;
        time1)           echo "TIMEFORMAT='TIME wall_s=%3R user_s=%3U sys_s=%3S'; time ./nvprobe 1" ;;
        ioctl)           echo './ioctlping 10000' ;;
        ctrl)            echo './ctrlping 2000 64 512 4096 16384' ;;
        churn)           echo './bench-cu churn 20' ;;
        ppo)             echo "$py ./pporl.py --seconds ${PPO_SECONDS:-60}" ;;
        *) err "unknown metric $m" ;;
    esac
}

# Per target: how to run a shell string there, its python, probe dir, device and VMM pid.
target_exec() {
    local t=$1; shift
    case $t in
        native) bash -c "$*" ;;
        v1) "$ROOT/caraxes/tools/bench-v1vm.sh" ssh "$*" ;;
        v2) "$ROOT/tools/caraxes-vm.sh" ssh "$*" ;;
    esac
}
target_prefix() {  # environment + cd for the run
    case $1 in
        native) echo "cd $BIN && export PYTHONDONTWRITEBYTECODE=1 NVPROBE_PTX=$BIN/kernels.ptx" ;;
        *) echo 'cd ~/bench && export PYTHONDONTWRITEBYTECODE=1 LD_LIBRARY_PATH=/opt/nvrm/lib NVPROBE_PTX=$HOME/bench/kernels.ptx' ;;
    esac
}
target_py()   { [[ $1 == native ]] && echo "$HOSTPY" || echo '$HOME/gpu/venv/bin/python'; }
target_pyd()  { [[ $1 == native ]] && echo "$LT/probe/python" || echo '$HOME/caraxes-tests/leandro-test/probe/python'; }
target_pids() {  # "device_pid vmm_pid" or "- -"
    local W
    case $1 in
        native) echo "- -" ;;
        v1) "$ROOT/caraxes/tools/bench-v1vm.sh" pids ;;
        v2) W=${CARAXES_STORE:-/mnt/vmstore/caraxes}/${VM:?VM unset for v2}
            echo "$(cat "$W/gpu.pid") $(cat "$W/ch.pid")" ;;
    esac
}

ticks() { [[ $1 == - ]] && { echo 0; return; }; awk '{print $14+$15}' "/proc/$1/stat" 2>/dev/null || echo 0; }
rss()   { [[ $1 == - ]] && { echo "- -"; return; }; awk '/^VmRSS/{r=$2} /^VmHWM/{h=$2} END{print r, h}' "/proc/$1/status" 2>/dev/null || echo "- -"; }
# Every device and VMM process on the host: "pid:name" (name from the socket/state dir).
all_procs() {
    local p cmd dir
    for p in $(pgrep -f 'vfio-user-nvgpu --socket|vhost-user-nvrm --nvrm|cloud-hypervisor'); do
        cmd=$(tr '\0' ' ' < "/proc/$p/cmdline" 2>/dev/null) || continue
        dir=$(grep -oE '/mnt/vmstore/caraxes/[^/ ]+' <<< "$cmd" | head -1)
        case $cmd in *vfio-user-nvgpu*) n=dev ;; *vhost-user-nvrm*) n=dev ;; *) n=vmm ;; esac
        echo "$p:${dir##*/}-$n"
    done
}
conditions() {
    echo "# time $(date -Iseconds)"
    echo "# gpu $(nvidia-smi --query-gpu=temperature.gpu,clocks.sm,clocks.mem,pstate,utilization.gpu,memory.used,persistence_mode --format=csv,noheader 2>&1)"
    echo "# procs $(all_procs | tr '\n' ' ')"
    echo "# loadavg $(cut -d' ' -f1-3 /proc/loadavg)"
}

run_one() {  # target metric rep dir
    local t=$1 m=$2 r=$3 dir=$4 f cmd pids dev vmm d0 d1 v0 v1_ o0 o1 p
    f=$dir/$t/$m-$r.txt
    mkdir -p "$dir/$t"
    cmd=$(metric_cmd "$m" "$(target_py "$t")" "$(target_pyd "$t")")
    read -r dev vmm <<< "$(target_pids "$t")"
    local -A oth0=()
    for p in $(all_procs); do [[ ${p%%:*} == "$dev" || ${p%%:*} == "$vmm" ]] || oth0[$p]=$(ticks "${p%%:*}"); done
    {
        echo "# target $t metric $m rep $r"
        echo "# cmd $cmd"
        # /proc/<pid>/exe of the capability-carrying v2 binary is not readable; hash the
        # file its command line names.
        if [[ $dev != - ]]; then
            local exe; exe=$(tr '\0' '\n' < "/proc/$dev/cmdline" | head -1)
            echo "# device $exe sha256 $(sha256sum < "$exe" | cut -c1-16)"
        fi
        conditions
    } > "$f"
    d0=$(ticks "$dev"); v0=$(ticks "$vmm")
    target_exec "$t" "$(target_prefix "$t"); t0=\$(date +%s%N); timeout 900 bash -c \"$cmd\" 2>&1; rc=\$?; t1=\$(date +%s%N); echo \"# WALL wall_ms=\$(( (t1 - t0) / 1000000 )) rc=\$rc\"" >> "$f" 2>&1
    d1=$(ticks "$dev"); v1_=$(ticks "$vmm")
    local others=""
    for p in "${!oth0[@]}"; do others+="${p#*:}=$(( ( $(ticks "${p%%:*}") - oth0[$p] ) * 1000 / TICK ))ms "; done
    echo "# MEAS dev_pid=$dev dev_cpu_ms=$(( (d1 - d0) * 1000 / TICK )) dev_rss_hwm_kb=$(rss "$dev" | tr ' ' /) vmm_cpu_ms=$(( (v1_ - v0) * 1000 / TICK )) others: $others" >> "$f"
    printf '%s %-3s %-15s rep %s: %s | %s\n' "$(date +%T)" "$t" "$m" "$r" \
        "$(grep -E 'MSPERLAUNCH|mean10|WALL|p50_us|INIT|TIME|GFLOP' "$f" | head -3 | tr '\n' ' ' | cut -c1-150)" \
        "$(tail -1 "$f" | cut -c1-120)"
}

# Guest part of a v2 driver load: nvidia-uvm and the device nodes (the image has no
# nvidia-modprobe; the nodes tools/caraxes-vm.sh uvm makes, plus nvidia-uvm-tools).
V2_NODES='sudo modprobe nvidia-uvm
    [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
    [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
    u=$(grep nvidia-uvm /proc/devices | cut -d" " -f1); sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools
    sudo mknod -m 666 /dev/nvidia-uvm c $u 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c $u 1'
v2_restart() {
    "$ROOT/tools/caraxes-vm.sh" ssh sync || true
    "$ROOT/tools/caraxes-vm.sh" down
    # `up` gives the guest-RAM registration 60 s; under memory pressure that can be too
    # short (series2): one more attempt after a clean stop.
    "$ROOT/tools/caraxes-vm.sh" up || { "$ROOT/tools/caraxes-vm.sh" down; sleep 5; "$ROOT/tools/caraxes-vm.sh" up; } &&
        "$ROOT/tools/caraxes-vm.sh" wait 120 &&
        "$ROOT/tools/caraxes-vm.sh" probe "bench-$(date +%H%M%S)" | tail -n 2 &&
        "$ROOT/tools/caraxes-vm.sh" ssh "$V2_NODES" && echo "v2 restarted: server pid $(cat "${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM/gpu.pid")"
}

case ${1:-} in
v2-restart) v2_restart ;;
v2-nodes) "$ROOT/tools/caraxes-vm.sh" ssh "$V2_NODES" ;;
stage)
    t=${2:?target}
    [[ $t == native ]] && exit 0
    cp "$ROOT/caraxes/tools/pporl.py" "$BIN/pporl.py"   # the PPO probe travels with the binaries (metric ppo)
    tar -C "$BIN" -cf - . | target_exec "$t" 'mkdir -p ~/bench && tar -xf - -C ~/bench && cd ~/bench && sha256sum bench-cu nvprobe ioctlping ctrlping kernels.ptx'
    ;;
series)
    shift; targets="native" out="" reps=5 metrics=$ALL_METRICS
    while [[ $# -gt 0 ]]; do
        case $1 in
            --targets) targets=$2; shift 2 ;; --out) out=$2; shift 2 ;;
            --reps) reps=$2; shift 2 ;; --metrics) metrics=$2; shift 2 ;;
            *) err "unknown option $1" ;;
        esac
    done
    [[ -n $out ]] || err "--out DIR"
    mkdir -p "$out"
    { echo "# series $(date -Iseconds) targets=[$targets] reps=$reps metrics=[$metrics]"
      echo "# caraxes $(git -C "$ROOT" describe --always --dirty) leandro-test $(git -C "$LT" describe --always --dirty)"
      (cd "$BIN" && sha256sum *) | sed 's/^/# bin /'; conditions; } >> "$out/series.txt"
    for r in $(seq 1 "$reps"); do
        # v1 keeps guest pages of freed pinned (0x71) registrations until the guest restarts
        # and refuses new ones past LEA_MAX_PIN_TOTAL_MIB (1 GiB); one rep fits, so with
        # V1_RESTART=1 the v1 VM and its backend restart before every rep after the first.
        if [[ ${V1_RESTART:-0} == 1 && $r -gt 1 && " $targets " == *" v1 "* ]]; then
            { echo "# $(date +%T) v1 restart before rep $r"
              "$ROOT/caraxes/tools/bench-v1vm.sh" down && "$ROOT/caraxes/tools/bench-v1vm.sh" up &&
              "$ROOT/caraxes/tools/bench-v1vm.sh" modules 2>&1 | grep -E "ready|virtio_nvrm "; } >> "$out/series.txt" 2>&1
        fi
        # v2 (docs/caraxes/benchmark.md): with V2_RESTART=1 the v2 VM and its device process
        # restart before every rep after the first, symmetric with V1_RESTART.
        if [[ ${V2_RESTART:-0} == 1 && $r -gt 1 && " $targets " == *" v2 "* ]]; then
            { echo "# $(date +%T) v2 restart before rep $r"; v2_restart; } >> "$out/series.txt" 2>&1
        fi
        for m in $metrics; do
            for t in $targets; do run_one "$t" "$m" "$r" "$out" | tee -a "$out/series.txt"; done
        done
    done
    echo "# end $(date -Iseconds)" >> "$out/series.txt"
    ;;
startup)
    t=${2:?target}; shift 2; out="" reps=5
    while [[ $# -gt 0 ]]; do
        case $1 in --out) out=$2; shift 2 ;; --reps) reps=$2; shift 2 ;; *) err "unknown option $1" ;; esac
    done
    [[ -n $out ]] || err "--out DIR"
    mkdir -p "$out/$t"
    # Module load to the first successful cuInit, measured in the guest. v2: unload
    # nvidia_uvm/nvidia, function-level reset of the device, modprobe nvidia (RmInitAdapter
    # through the device model), then nvprobe 0 until it passes. v1: unload virtio_nvrm and
    # nvrm_nodes (a device reset), insmod both, provision, then nvprobe 0.
    case $t in
        v2) load='for m in nvidia_uvm nvidia; do lsmod | grep -q "^$m " && sudo rmmod $m; done
                  for d in /sys/bus/pci/devices/*; do [ "$(cat $d/vendor)" = 0x10de ] && echo 1 | sudo tee $d/reset >/dev/null; done
                  t0=$(date +%s%N); sudo modprobe nvidia; '"$V2_NODES"'; t1=$(date +%s%N)' ;;
        v1) load='sudo rmmod virtio_nvrm; sudo rmmod nvrm_nodes
                  t0=$(date +%s%N); sudo insmod ~/guest-module/nvrm_nodes/nvrm_nodes.ko create_nodes=0
                  sudo ~/guest-module/nvrm_nodes/nvrm-nodes-tool provision params ~/gpu/params.txt >/dev/null
                  sudo insmod ~/guest-module/virtio_nvrm/virtio_nvrm.ko; t1=$(date +%s%N)' ;;
        *) err "startup is v1|v2 only (the host modules are not touched)" ;;
    esac
    for r in $(seq 1 "$reps"); do
        f=$out/$t/startup-$r.txt
        { echo "# target $t metric startup rep $r"; conditions; } > "$f"
        target_exec "$t" "cd ~/bench && export LD_LIBRARY_PATH=/opt/nvrm/lib; $load
            n=0; while :; do n=\$((n+1)); ./nvprobe 0 >/dev/null 2>&1 && break; [ \$n -ge 300 ] && break; sleep 0.05; done
            t2=\$(date +%s%N)
            echo \"STARTUP load_ms=\$(( (t1 - t0) / 1000000 )) load_to_cuinit_ms=\$(( (t2 - t0) / 1000000 )) attempts=\$n\"" >> "$f" 2>&1
        echo "$(date +%T) $t startup rep $r: $(grep STARTUP "$f")"
    done
    ;;
*) sed -n '2,/^set -uo/p' "$0" | sed '$d'; exit 2 ;;
esac

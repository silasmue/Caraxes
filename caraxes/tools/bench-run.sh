#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# P13 benchmark harness: Leandro v1 (vhost-user-nvrm, API forwarding), v2 Caraxes
# (vfio-user-nvgpu, stock guest driver) and native, the same binaries on each side.
# Method, conditions and results: docs/caraxes/benchmark.md.
#
#   bench-run.sh stage  TARGET                  copy target/bench/bin/* into the guest's ~/bench
#   bench-run.sh stage-llama TARGET             copy the llama.cpp build and model into ~/bench-llama
#   bench-run.sh series --targets "native v1 v2" --out DIR [--reps 5] [--metrics "..."]
#   bench-run.sh startup TARGET --out DIR [--reps 5]   modprobe (v2) / module load (v1) to first cuInit
#   bench-run.sh v2-restart | v2-nodes     restart the v2 VM and device (down, up, probe, nodes) |
#                                          only load nvidia-uvm and make the /dev/nvidia* nodes
#   bench-run.sh libdir TARGET              print the guest LD_LIBRARY_PATH of v1|v2 (v2 per
#                                          CARAXES_DRIVER, tools/lib/driver.sh)
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
# Workload gate (docs/caraxes/workload-gate.md, not in ALL_METRICS): ppo-full (pporl.py for
# PPO_FULL_SECONDS, default 420), llama (llamaprobe.py: llama-bench pp512/tg128, LLAMA_REPS
# default 5, LLAMA_ARGS extra; needs `stage-llama` for a guest), llama-nograph (tg128 only,
# CUDA graphs off: one launch per kernel instead of one graph per token), transformer (transformer.py,
# GPT-style training steps) and convnet (convnet.py, ResNet-18 training), the last two for
# WORKLOAD_SECONDS (default 60) with TRANSFORMER_ARGS / CONVNET_ARGS appended. BENCH_TIMEOUT
# (default 900 s) bounds each run.
# The recommended v2 device windows for CUDA (2026-09-25, docs/caraxes/bar2-memfd.md §8,
# intr-page.md §4): CARAXES_INTR_PAGE=1 CARAXES_BAR1_PASSTHROUGH=pool (CARAXES_BAR1_POOL_MIB=32
# with other VMs up) CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid CARAXES_BAR2_MEMFD=1
# CARAXES_MSI_REARM_IOEVENTFD=1, on top of the host-execution environment. The caller exports
# them before `v2-restart` or tools/caraxes-vm.sh up; this script does not set them.
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
# The llama.cpp build and the model of the workload gate (caraxes/tools/build-llama.sh).
LLAMA_DIST=${LLAMA_DIST:-/mnt/vmstore/caraxes/llama/dist}
LLAMA_GGUF=${LLAMA_GGUF:-/mnt/vmstore/caraxes/models/llama-gate.gguf}
# Probes that travel with the binaries into ~/bench (and into target/bench/bin for native).
PROBES="pporl.py llamaprobe.py transformer.py convnet.py"
ALL_METRICS="launch-auto launch-blocking short-auto short-blocking copy mm rl init time0 time1 ioctl ctrl churn"

err() { echo "ERROR: $*" >&2; exit 1; }

# The guest library directory (libcuda.so.1) per target. v2 follows the guest driver the
# way the gates do (tools/lib/driver.sh: CARAXES_DRIVER=610|615, default DRIVER_VERSION;
# CARAXES_GUEST_NVRM overrides): 610 keeps /opt/nvrm/lib, 615 takes the system path,
# because the 615 base image still carries the 610.57.04 userspace in /opt/nvrm/lib
# (docs/caraxes/guest-driver-versions.md section 8). A fixed /opt/nvrm/lib made every bench
# binary of the 615 guest fail with cuInit 803 while the gate's cuInit passed (runs 4 to 6
# in caraxes/runs/cutover-615-2026-09-24/v2-615guest*/v2/; verified in
# caraxes/runs/bench-615-2026-09-24/). v1 keeps /opt/nvrm/lib: its 615 image repoints it
# (docs/caraxes/v1-615-image.md). Empty when the driver selection fails (checked on use).
V2_LIBDIR=$(LEA_ROOT=$ROOT; source "$ROOT/tools/lib/driver.sh" && caraxes_driver_select &&
    printf '%s' "$CARAXES_GUEST_LIBDIR") || V2_LIBDIR=""
guest_libdir() {
    case $1 in
        v2) [[ -n $V2_LIBDIR ]] || err "no guest library directory for v2 (tools/lib/driver.sh refused the driver)"
            echo "$V2_LIBDIR" ;;
        *) echo /opt/nvrm/lib ;;
    esac
}

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
        ppo-full)        echo "$py ./pporl.py --seconds ${PPO_FULL_SECONDS:-420}" ;;
        llama)           echo "$py ./llamaprobe.py --reps ${LLAMA_REPS:-5} ${LLAMA_ARGS:-}" ;;
        llama-nograph)   echo "GGML_CUDA_DISABLE_GRAPHS=1 $py ./llamaprobe.py --reps ${LLAMA_REPS:-5} -p 0 -n 128 ${LLAMA_ARGS:-}" ;;
        transformer)     echo "$py ./transformer.py --seconds ${WORKLOAD_SECONDS:-60} ${TRANSFORMER_ARGS:-}" ;;
        convnet)         echo "$py ./convnet.py --seconds ${WORKLOAD_SECONDS:-60} ${CONVNET_ARGS:-}" ;;
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
        native) echo "cd $BIN && export PYTHONDONTWRITEBYTECODE=1 NVPROBE_PTX=$BIN/kernels.ptx LLAMA_DIR=$LLAMA_DIST LLAMA_MODEL=$LLAMA_GGUF" ;;
        *) echo "cd ~/bench && export PYTHONDONTWRITEBYTECODE=1 LD_LIBRARY_PATH=$(guest_libdir "$1")"' NVPROBE_PTX=$HOME/bench/kernels.ptx LLAMA_DIR=$HOME/bench-llama LLAMA_MODEL=$HOME/bench-llama/model.gguf' ;;
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
    target_exec "$t" "$(target_prefix "$t"); t0=\$(date +%s%N); timeout ${BENCH_TIMEOUT:-900} bash -c \"$cmd\" 2>&1; rc=\$?; t1=\$(date +%s%N); echo \"# WALL wall_ms=\$(( (t1 - t0) / 1000000 )) rc=\$rc\"" >> "$f" 2>&1
    d1=$(ticks "$dev"); v1_=$(ticks "$vmm")
    local others=""
    for p in "${!oth0[@]}"; do others+="${p#*:}=$(( ( $(ticks "${p%%:*}") - oth0[$p] ) * 1000 / TICK ))ms "; done
    echo "# MEAS dev_pid=$dev dev_cpu_ms=$(( (d1 - d0) * 1000 / TICK )) dev_rss_hwm_kb=$(rss "$dev" | tr ' ' /) vmm_cpu_ms=$(( (v1_ - v0) * 1000 / TICK )) others: $others" >> "$f"
    printf '%s %-3s %-15s rep %s: %s | %s\n' "$(date +%T)" "$t" "$m" "$r" \
        "$(grep -E '^# (PPO|LLAMA|TRANSFORMER|CONVNET) |MSPERLAUNCH|mean10|WALL|p50_us|INIT|TIME|GFLOP' "$f" | head -3 | tr '\n' ' ' | cut -c1-150)" \
        "$(tail -1 "$f" | cut -c1-120)"
}

# Guest part of a v2 driver load: nvidia-uvm and the device nodes (the image has no
# nvidia-modprobe; the nodes tools/caraxes-vm.sh uvm makes, plus nvidia-uvm-tools).
V2_NODES='sudo modprobe nvidia-uvm
    [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
    [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
    u=$(grep nvidia-uvm /proc/devices | cut -d" " -f1); sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools
    sudo mknod -m 666 /dev/nvidia-uvm c $u 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c $u 1'
v2_up() {
    # `up` gives the guest-RAM registration 60 s; under memory pressure that can be too
    # short (series2): one more attempt after a clean stop.
    "$ROOT/tools/caraxes-vm.sh" up || { "$ROOT/tools/caraxes-vm.sh" down; sleep 5; "$ROOT/tools/caraxes-vm.sh" up; }
}
v2_restart() {
    local n=0 log=${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM/gpu.log
    "$ROOT/tools/caraxes-vm.sh" ssh sync || true
    "$ROOT/tools/caraxes-vm.sh" down
    # Morning matrix 2026-09-25: with V2_LOG_KEEP=FILE (set by `series` per rep) keep the
    # stopped device's log, whose exit summary (the BAR1 pool's page movements, the doorbell
    # and mirror counters) the next `up` would truncate. The workload gate's Xid 31 came in a
    # rep whose summary was lost that way (caraxes/runs/matrix-2026-09-25/workload-615/xid31-note.txt).
    [[ -n ${V2_LOG_KEEP:-} ]] && cp "$log" "$V2_LOG_KEEP" 2>/dev/null
    # Night shift 2026-09-24 (docs/caraxes/benchmark.md "Night series"): with other VMs on the
    # card the device may refuse its reservation under the D11 headroom rule (hostmem.rs:1328,
    # "below the 1024 MiB headroom"); wait five minutes and try again, up to
    # V2_HEADROOM_RETRIES (default 12) times, instead of losing the rep.
    until v2_up; do
        grep -q 'MiB headroom' "$log" 2>/dev/null && (( n++ < ${V2_HEADROOM_RETRIES:-12} )) || return 1
        echo "$(date +%T) v2 reservation refused by the headroom rule; retry $n in 300 s"
        "$ROOT/tools/caraxes-vm.sh" down; sleep 300
    done &&
        "$ROOT/tools/caraxes-vm.sh" wait 120 &&
        "$ROOT/tools/caraxes-vm.sh" probe "bench-$(date +%H%M%S)" | tail -n 2 &&
        "$ROOT/tools/caraxes-vm.sh" ssh "$V2_NODES" && echo "v2 restarted: server pid $(cat "${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM/gpu.pid")"
}

case ${1:-} in
libdir) guest_libdir "${2:?target}" ;;
v2-restart) v2_restart ;;
v2-nodes) "$ROOT/tools/caraxes-vm.sh" ssh "$V2_NODES" ;;
stage)
    t=${2:?target}
    # The Python probes travel with the binaries (metrics ppo, ppo-full, llama, transformer,
    # convnet); native runs them from $BIN too.
    for p in $PROBES; do cp "$ROOT/caraxes/tools/$p" "$BIN/$p"; done
    [[ $t == native ]] && exit 0
    tar -C "$BIN" -cf - . | target_exec "$t" 'mkdir -p ~/bench && tar -xf - -C ~/bench && cd ~/bench && sha256sum bench-cu nvprobe ioctlping ctrlping kernels.ptx'
    ;;
stage-llama)
    # llama-bench, its CUDA runtime and cuBLAS (dist/lib) and the model as ~/bench-llama/model.gguf;
    # skipped when the guest already holds the same files (sha256). Native reads them in place.
    t=${2:?target}
    [[ -x $LLAMA_DIST/llama-bench && -s $LLAMA_GGUF ]] || err "no $LLAMA_DIST/llama-bench or $LLAMA_GGUF (caraxes/tools/build-llama.sh all)"
    [[ $t == native ]] && exit 0
    sums=$( (cd "$LLAMA_DIST" && sha256sum llama-bench lib/*); echo "$(sha256sum < "$LLAMA_GGUF" | cut -d' ' -f1)  model.gguf")
    if target_exec "$t" "cd ~/bench-llama 2>/dev/null && echo '$sums' > SHA256SUMS.want && sha256sum --quiet -c SHA256SUMS.want" >/dev/null 2>&1; then
        echo "stage-llama $t: up to date"; exit 0
    fi
    m=$(readlink -f "$LLAMA_GGUF")
    tar -C "$LLAMA_DIST" -chf - . -C "$(dirname "$m")" --transform "s|^$(basename "$m")\$|model.gguf|" "$(basename "$m")" |
        target_exec "$t" 'rm -rf ~/bench-llama && mkdir -p ~/bench-llama && tar -xf - -C ~/bench-llama && cd ~/bench-llama && sha256sum llama-bench model.gguf && df -h ~ | tail -1'
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
    # Fail before the first run when a guest target has no library directory.
    for t in $targets; do [[ $t == native ]] || guest_libdir "$t" > /dev/null || exit 1; done
    mkdir -p "$out"
    { echo "# series $(date -Iseconds) targets=[$targets] reps=$reps metrics=[$metrics]"
      for t in $targets; do [[ $t == native ]] || echo "# libdir $t $(guest_libdir "$t")"; done
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
            { echo "# $(date +%T) v2 restart before rep $r"
              V2_LOG_KEEP="$out/v2-device-rep$((r - 1)).txt" v2_restart; } >> "$out/series.txt" 2>&1
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
    libdir=$(guest_libdir "$t") || exit 1
    for r in $(seq 1 "$reps"); do
        f=$out/$t/startup-$r.txt
        { echo "# target $t metric startup rep $r"; conditions; } > "$f"
        target_exec "$t" "cd ~/bench && export LD_LIBRARY_PATH=$libdir; $load
            n=0; while :; do n=\$((n+1)); ./nvprobe 0 >/dev/null 2>&1 && break; [ \$n -ge 300 ] && break; sleep 0.05; done
            t2=\$(date +%s%N)
            echo \"STARTUP load_ms=\$(( (t1 - t0) / 1000000 )) load_to_cuinit_ms=\$(( (t2 - t0) / 1000000 )) attempts=\$n\"" >> "$f" 2>&1
        echo "$(date +%T) $t startup rep $r: $(grep STARTUP "$f")"
    done
    ;;
*) sed -n '2,/^set -uo/p' "$0" | sed '$d'; exit 2 ;;
esac

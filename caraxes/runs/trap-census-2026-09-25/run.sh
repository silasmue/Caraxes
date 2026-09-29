#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The trap census of 2026-09-25 (docs/caraxes/optimization-backlog.md): every CUDA driver-API
# operation alone, bracketed by census-cu's marker writes, with the device trace on; then
# rlprobe and a 30 s PPO run as workload windows. vm3 only, in the environment of
# caraxes/runs/gsp-rpc-2026-09-24/run.sh gate610 (1 GiB at reservation slot 5, host RM, host
# execution, token pool 64, hybrid reactor) plus the windows of the mode:
#   bare:  no window at all (no interrupt page, no BAR1 pool, no token pool, trapped doorbell)
#   off:   interrupt page, BAR1 pool (32 MiB), token pool, hybrid reactor; BAR2 memfd off
#   on:    the same with CARAXES_BAR2_MEMFD=1
#   on-syncdb: on plus CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1 (bar2-memfd.md §7: the reference
#          setting when on hangs where off passes)
#   on-nopool: interrupt page, token pool, hybrid reactor and BAR2 memfd, no BAR1 pool (on and
#          on-syncdb hang in the first cuCtxCreate; the memfd without the pool does not)
#   native: census-cu on the host (a CUDA client of the host RM; nothing else touched)
#   build: vm3 up once in the off environment, census-cu and census.cubin built in the guest
#          and copied to target/bench/bin (the guest has gcc and cuda.h 13.0; the binary then
#          runs unchanged on the host)
# Usage: run.sh bare|off|on|on-syncdb|on-nopool|native|build [label]
# Every census-cu process in the guest is bounded by timeout 240; the workloads by BENCH_TIMEOUT=300.
set -uo pipefail
MODE=${1:?usage: run.sh bare|off|on|on-syncdb|on-nopool|native|build [label]}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
BIN=$ROOT/target/bench/bin
C=$HERE/$MODE${2:+-$2}
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
cd "$ROOT" || exit 1
export LT=/home/silas/git/Leandro-Test CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test
export LEA_HOSTVENV=/home/silas/git/Leandro/vendor/hostvenv
REPS=${REPS:-10}

census_guest='cd ~/bench && export CENSUS_PTX=$HOME/bench/kernels.ptx CENSUS_CUBIN=$HOME/bench/census.cubin
R0=$(for d in /sys/bus/pci/devices/*; do [ "$(cat $d/vendor)" = 0x10de ] && echo $d/resource0; done | head -1)
echo "# marker resource $R0"
E="sudo timeout 240 env LD_LIBRARY_PATH=/opt/nvrm/lib CENSUS_PTX=$CENSUS_PTX CENSUS_CUBIN=$CENSUS_CUBIN CENSUS_MARK=$R0"
for i in 1 2 3; do $E ./census-cu init || exit 1; sleep 1; done
$E ./census-cu ctx 3 || exit 1
$E ./census-cu ops '"$REPS"' || exit 1
$E ./census-cu ops '"$REPS"' || exit 1'

if [[ $MODE == native ]]; then
    mkdir -p "$C"
    { head -1 /proc/driver/nvidia/version; (cd "$BIN" && sha256sum census-cu census.cubin kernels.ptx); date -Iseconds
      nvidia-smi --query-gpu=temperature.gpu,clocks.sm,pstate,utilization.gpu,memory.used --format=csv,noheader
      pgrep -af 'vfio-user-nvgpu --socket' | cut -c1-120; cut -d' ' -f1-3 /proc/loadavg; } > "$C/setup.txt" 2>&1
    caraxes/tools/bench-hostwatch.sh start "$C"
    ( cd "$BIN" && export CENSUS_PTX=$BIN/kernels.ptx CENSUS_CUBIN=$BIN/census.cubin
      for i in 1 2 3; do ./census-cu init; sleep 1; done
      ./census-cu ctx 3; ./census-cu ops "$REPS"; ./census-cu ops "$REPS" ) > "$C/census.txt" 2>&1
    echo "native census rc=$? $(grep -c '^OP' "$C/census.txt") OP lines"
    caraxes/tools/bench-hostwatch.sh stop "$C"
    exit 0
fi

if pgrep -f /mnt/vmstore/caraxes/vm3 >/dev/null; then
    echo "ERROR: a vm3 process runs; refusing to start" >&2; pgrep -af /mnt/vmstore/caraxes/vm3 >&2; exit 2
fi
mkdir -p "$C"
export VM=vm3 CARAXES_IP=192.168.103.23 CARAXES_TAP=caraxes3 CARAXES_MAC=02:00:00:00:00:23
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1 CARAXES_MIRROR_GR_SHADOW=1
export CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=5
export CARAXES_SERVER=${CARAXES_SERVER:-/mnt/vmstore/caraxes/vfio-user-nvgpu-main-1ea2529}
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_BAR1_POOL_MIB CARAXES_DISPLAY
unset CARAXES_DISPLAY_PAGE CARAXES_BAR2_MEMFD CARAXES_DOORBELL_TOKENS CARAXES_DOORBELL_POLL CARAXES_ALLOC_LAZY
unset CARAXES_TRACE_ACCESSES CARAXES_TRACE_FLUSH CARAXES_BAR2_MEMFD_SYNC_DOORBELL
case $MODE in
    bare) ;;
    off|build) export CARAXES_INTR_PAGE=1 CARAXES_BAR1_PASSTHROUGH=pool CARAXES_BAR1_POOL_MIB=32
         export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid ;;
    on-nopool) export CARAXES_INTR_PAGE=1 CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
         export CARAXES_BAR2_MEMFD=1 ;;
    on-syncdb) export CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1 ;&
    on)  export CARAXES_INTR_PAGE=1 CARAXES_BAR1_PASSTHROUGH=pool CARAXES_BAR1_POOL_MIB=32
         export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid CARAXES_BAR2_MEMFD=1 ;;
    *) echo "ERROR: unknown mode $MODE" >&2; exit 2 ;;
esac
W=/mnt/vmstore/caraxes/$VM; T0=$(date '+%F %T')
{ head -1 /proc/driver/nvidia/version; sha256sum "$CARAXES_SERVER"; git -C "$ROOT" log --oneline -1
  (cd "$BIN" && sha256sum census-cu census.cubin kernels.ptx 2>/dev/null)
  env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; date -Iseconds; cut -d' ' -f1-3 /proc/loadavg
  pgrep -af 'vfio-user-nvgpu --socket' | cut -c1-120; } > "$C/setup.txt"
caraxes/tools/bench-hostwatch.sh start "$C"

# Up, with the reservation retried after five minutes when the host lacks headroom.
up_ok=0
for attempt in 1 2 3 4 5 6; do
    tools/caraxes-vm.sh up >> "$C/up.txt" 2>&1 && tools/caraxes-vm.sh wait >> "$C/up.txt" 2>&1 && { up_ok=1; break; }
    tail -3 "$C/up.txt"
    tools/caraxes-vm.sh down >> "$C/up.txt" 2>&1
    if grep -qE 'NO_MEMORY|did not give|headroom' "$C/up.txt"; then
        echo "reservation refused (attempt $attempt), waiting 5 min"; sleep 300
    else
        break
    fi
done
if [[ $up_ok != 1 ]]; then
    caraxes/tools/bench-hostwatch.sh stop "$C"; tools/caraxes-vm.sh down >> "$C/down.txt" 2>&1
    journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
    echo "ERROR: vm3 did not come up"; exit 1
fi
pgrep -af "vfio-user-nvgpu.*$W/" > "$C/device-pid.txt"
tools/caraxes-vm.sh probe census >> "$C/up.txt" 2>&1; tail -2 "$C/up.txt" | cut -c1-200
caraxes/tools/bench-run.sh v2-nodes >> "$C/up.txt" 2>&1
grep -E 'intr page|bar2 memfd|bar1 passthrough|bar1 pool|doorbell tokens|doorbell poll|reserved host VRAM|host ABI|trace' "$W/gpu.log" | cut -c1-400 > "$C/device-start.txt"

caraxes/tools/bench-run.sh stage v2 >> "$C/up.txt" 2>&1
if [[ $MODE == build ]]; then
    tools/caraxes-vm.sh ssh 'mkdir -p ~/bench && cat > ~/bench/census-cu.c' < caraxes/tools/census-cu.c
    tools/caraxes-vm.sh ssh 'cd ~/bench && I=$(dirname $(find ~/gpu/venv -name cuda.h -path "*cu13*" | head -1)) && echo "cuda.h $I" &&
        gcc -O2 -Wall -o census-cu census-cu.c -I$I -L/opt/nvrm/lib -lcuda && ls -la census-cu && gcc --version | head -1 && ldd --version | head -1' > "$C/build.txt" 2>&1
    tools/caraxes-vm.sh ssh 'cd ~/bench && LD_LIBRARY_PATH=/opt/nvrm/lib ./census-cu ptx2cubin kernels.ptx census.cubin' >> "$C/build.txt" 2>&1
    tools/caraxes-vm.sh ssh 'cat ~/bench/census-cu' > "$BIN/census-cu"; chmod 755 "$BIN/census-cu"
    tools/caraxes-vm.sh ssh 'cat ~/bench/census.cubin' > "$BIN/census.cubin"
    (cd "$BIN" && sha256sum census-cu census.cubin) >> "$C/build.txt"
    cat "$C/build.txt"
else
    tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
    # The census: one trace slice for all census-cu processes.
    L0=$(wc -l < "$W/gpu-trace.log")
    # The other devices' CPU during the census (another VM's GPU work shares the GR engine by
    # timeslice, which the census windows then carry).
    others0=$(for p in $(pgrep -f 'vfio-user-nvgpu --socket'); do grep -q "$W/" /proc/$p/cmdline 2>/dev/null || echo "$p:$(awk '{print $14+$15}' /proc/$p/stat)"; done)
    tools/caraxes-vm.sh ssh "$census_guest" > "$C/census.txt" 2>&1
    for e in $others0; do p=${e%%:*}; echo "other device pid $p $(tr '\0' ' ' < /proc/$p/cmdline | grep -oE '/mnt/vmstore/caraxes/[^/ ]+' | head -1) cpu_ms $(( ($(awk '{print $14+$15}' /proc/$p/stat) - ${e#*:}) * 1000 / $(getconf CLK_TCK) )) during the census"; done > "$C/census-others.txt" 2>/dev/null
    echo "census rc=$? $(grep -c '^OP' "$C/census.txt") OP lines"
    tail -n +"$((L0 + 1))" "$W/gpu-trace.log" > "$C/census-trace.txt"
    python3 caraxes/tools/census-table.py "$C/census-trace.txt" --names "$C/census.txt" --pool-mib 32 \
        --json "$C/census.json" > "$C/census-table.md"
    # Workload windows: rlprobe (50 episodes), PPO for 30 s, the short kernel.
    for m in rl ppo short-auto; do
        L0=$(wc -l < "$W/gpu-trace.log")
        BENCH_TIMEOUT=300 PPO_SECONDS=30 caraxes/tools/bench-run.sh series --targets v2 --reps 1 --metrics "$m" --out "$C/work" 2>&1 | tail -1
        tail -n +"$((L0 + 1))" "$W/gpu-trace.log" > "$C/$m-trace.txt"
        python3 caraxes/tools/rpc-cost.py "$C/$m-trace.txt" > "$C/$m-rpc-cost.md" 2>&1
    done
fi

caraxes/tools/bench-hostwatch.sh stop "$C"
tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
grep -E 'intr page|bar2 memfd|bar1 pool|bar1 passthrough|doorbell|mirror: |shadow: |syncs \(|ERROR|WARN' "$W/gpu.log" | tail -40 | cut -c1-1200 > "$C/device-summary.txt"
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
for f in "$C"/*-trace.txt; do [[ -f $f ]] && xz -T0 "$f"; done
echo "$MODE done $(date -Iseconds); host Xid lines $(grep -c Xid "$C/host-xid.txt"); vm3 left: $(pgrep -f /mnt/vmstore/caraxes/vm3 | wc -l)"

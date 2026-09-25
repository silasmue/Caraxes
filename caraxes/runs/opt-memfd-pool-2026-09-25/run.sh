#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Backlog items 1 and 4 of docs/caraxes/optimization-backlog.md on vm3: the BAR2 memfd beside
# the BAR1 pool (the fix of bar2-memfd.md §8) and the MSI re-arm ioeventfd (intr-page.md §4).
# The environment is caraxes/runs/trap-census-2026-09-25/run.sh's (gate610 of
# caraxes/runs/gsp-rpc-2026-09-24/run.sh: vm3, 610 guest, 1 GiB at reservation slot 5, host RM,
# host execution, token pool 64, hybrid reactor) with the windows of the mode:
#   on:   interrupt page, BAR1 pool 32 MiB, token pool 64, hybrid reactor, BAR2 memfd (the
#         census's `on`, which hung before the fix)
#   off:  the same without the BAR2 memfd (the census's `off`, the pool's reference)
# Parts, each switchable (1 = run):
#   CENSUS=1  census-cu init x3, ctx 3, ops REPS x2 (the census's windows), census-table.py
#   WORK=1    rlprobe 50 episodes, PPO 30 s, short-auto, one each, traced (workload-rates.py)
#   GATE=1    tools/caraxes-gate.sh cuda
#   COPY=N    bench-run.sh SERIES (default: copy), N reps, into LABEL/copy (0 = none)
# The device binary is CARAXES_SERVER (required: a frozen copy under /mnt/vmstore/caraxes).
# CARAXES_MSI_REARM_IOEVENTFD is passed through as the caller sets it (item 4).
# Usage: run.sh on|off LABEL
set -uo pipefail
MODE=${1:?usage: run.sh on|off LABEL}
LABEL=${2:?usage: run.sh on|off LABEL}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
OUT=${OUT:-$HERE}
C=$OUT/$LABEL
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
: "${CARAXES_SERVER:?set CARAXES_SERVER to the frozen device binary}"
cd "$ROOT" || exit 1
export CARAXES_SERVER
export LT=/home/silas/git/Leandro-Test CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test
export LEA_HOSTVENV=/home/silas/git/Leandro/vendor/hostvenv
REPS=${REPS:-10}
CENSUS=${CENSUS:-1} WORK=${WORK:-1} GATE=${GATE:-1} COPY=${COPY:-3} SERIES=${SERIES:-copy}

census_guest='cd ~/bench && export CENSUS_PTX=$HOME/bench/kernels.ptx CENSUS_CUBIN=$HOME/bench/census.cubin
R0=$(for d in /sys/bus/pci/devices/*; do [ "$(cat $d/vendor)" = 0x10de ] && echo $d/resource0; done | head -1)
echo "# marker resource $R0"
E="sudo timeout 240 env LD_LIBRARY_PATH=/opt/nvrm/lib CENSUS_PTX=$CENSUS_PTX CENSUS_CUBIN=$CENSUS_CUBIN CENSUS_MARK=$R0"
for i in 1 2 3; do $E ./census-cu init || exit 1; sleep 1; done
$E ./census-cu ctx 3 || exit 1
$E ./census-cu ops '"$REPS"' || exit 1
$E ./census-cu ops '"$REPS"' || exit 1'

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
export CARAXES_GATE_RUNS=$C
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_BAR1_POOL_MIB CARAXES_DISPLAY
unset CARAXES_DISPLAY_PAGE CARAXES_BAR2_MEMFD CARAXES_DOORBELL_TOKENS CARAXES_DOORBELL_POLL CARAXES_ALLOC_LAZY
unset CARAXES_TRACE_ACCESSES CARAXES_TRACE_FLUSH CARAXES_BAR2_MEMFD_SYNC_DOORBELL
export CARAXES_INTR_PAGE=1 CARAXES_BAR1_PASSTHROUGH=pool CARAXES_BAR1_POOL_MIB=32
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
case $MODE in
    on) export CARAXES_BAR2_MEMFD=1 ;;
    off) ;;
    *) echo "ERROR: unknown mode $MODE" >&2; exit 2 ;;
esac
W=/mnt/vmstore/caraxes/$VM; T0=$(date '+%F %T')
{ head -1 /proc/driver/nvidia/version; sha256sum "$CARAXES_SERVER"; git log --oneline -1
  (cd "$ROOT/target/bench/bin" && sha256sum census-cu census.cubin kernels.ptx 2>/dev/null)
  echo "parts: CENSUS=$CENSUS WORK=$WORK GATE=$GATE COPY=$COPY SERIES=\"$SERIES\" REPS=$REPS"
  env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; date -Iseconds; cut -d' ' -f1-3 /proc/loadavg
  pgrep -af 'vfio-user-nvgpu --socket' | cut -c1-120; } > "$C/setup.txt"
caraxes/tools/bench-hostwatch.sh start "$C"

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
tools/caraxes-vm.sh probe opt >> "$C/up.txt" 2>&1; tail -2 "$C/up.txt" | cut -c1-200
caraxes/tools/bench-run.sh v2-nodes >> "$C/up.txt" 2>&1
grep -E 'intr page|msi re-arm|bar2 memfd|bar1 passthrough|bar1 pool|doorbell tokens|doorbell poll|reserved host VRAM|host ABI|trace' "$W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
caraxes/tools/bench-run.sh stage v2 >> "$C/up.txt" 2>&1
tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"

if [[ $CENSUS == 1 ]]; then
    L0=$(wc -l < "$W/gpu-trace.log")
    others0=$(for p in $(pgrep -f 'vfio-user-nvgpu --socket'); do grep -q "$W/" /proc/$p/cmdline 2>/dev/null || echo "$p:$(awk '{print $14+$15}' /proc/$p/stat)"; done)
    tools/caraxes-vm.sh ssh "$census_guest" > "$C/census.txt" 2>&1
    rc=$?
    for e in $others0; do p=${e%%:*}; echo "other device pid $p $(tr '\0' ' ' < /proc/$p/cmdline | grep -oE '/mnt/vmstore/caraxes/[^/ ]+' | head -1) cpu_ms $(( ($(awk '{print $14+$15}' /proc/$p/stat) - ${e#*:}) * 1000 / $(getconf CLK_TCK) )) during the census"; done > "$C/census-others.txt" 2>/dev/null
    echo "census rc=$rc $(grep -c '^OP' "$C/census.txt") OP lines"
    tail -n +"$((L0 + 1))" "$W/gpu-trace.log" > "$C/census-trace.txt"
    python3 caraxes/tools/census-table.py "$C/census-trace.txt" --names "$C/census.txt" --pool-mib 32 \
        --json "$C/census.json" > "$C/census-table.md"
fi
if [[ $WORK == 1 ]]; then
    for m in rl ppo short-auto; do
        L0=$(wc -l < "$W/gpu-trace.log")
        BENCH_TIMEOUT=300 PPO_SECONDS=30 caraxes/tools/bench-run.sh series --targets v2 --reps 1 --metrics "$m" --out "$C/work" 2>&1 | tail -1
        tail -n +"$((L0 + 1))" "$W/gpu-trace.log" > "$C/$m-trace.txt"
        python3 caraxes/tools/rpc-cost.py "$C/$m-trace.txt" > "$C/$m-rpc-cost.md" 2>&1
    done
fi
if [[ $GATE == 1 ]]; then
    tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -1 "$C/gate-cuda.txt" | cut -c1-300
fi
if [[ $COPY -gt 0 ]]; then
    BENCH_TIMEOUT=600 caraxes/tools/bench-run.sh series --targets v2 --reps "$COPY" --metrics "$SERIES" --out "$C/copy" 2>&1 | tail -1
fi

caraxes/tools/bench-hostwatch.sh stop "$C"
tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
grep -E 'intr page|msi re-arm|bar2 memfd|bar1 pool|bar1 passthrough|doorbell|mirror: |shadow: |syncs \(|ERROR|WARN' "$W/gpu.log" | tail -40 | cut -c1-1200 > "$C/device-summary.txt"
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
# rl and ppo traces go beside the VM state (tens of MB); the rest stays here.
S=/mnt/vmstore/caraxes/$(basename "$OUT")/$LABEL; mkdir -p "$S"
for f in "$C"/*-trace.txt; do [[ -f $f ]] && xz -T0 "$f"; done
for w in rl ppo; do [[ -f $C/$w-trace.txt.xz ]] && mv "$C/$w-trace.txt.xz" "$S/"; done
echo "$MODE $LABEL done $(date -Iseconds); host Xid lines $(grep -c Xid "$C/host-xid.txt"); vm3 left: $(pgrep -f /mnt/vmstore/caraxes/vm3 | wc -l)"

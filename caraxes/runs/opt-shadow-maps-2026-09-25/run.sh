#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Backlog item 2 (docs/caraxes/optimization-backlog.md): the shadow's host RM map work at context
# creation and destruction (docs/caraxes/memory-model.md §7). vm0 only, 1 GiB at reservation
# slot 0, the environment of caraxes/runs/intr-page-2026-09-24/env.sh without the display: host
# RM, host execution, token pool 64, hybrid reactor, interrupt page, BAR2 memfd, no BAR1 pool.
# Modes (frozen binaries; the switches select the behaviour):
#   before:     4fec5bd, CARAXES_SHADOW_DEFER_TLB=0 CARAXES_SHADOW_REGION_POOL=0: the calls of
#               main d466e9d, with the new counters
#   defer-pool: 4fec5bd, deferred TLB invalidation and the region free list on
#   final:      f0edbb8, its defaults: whole-region frees and the free list on, deferral off
#   gather:     ac950e9, its defaults: final plus gather descriptors for scattered sysmem
# Per mode: the census probe as caraxes/runs/trap-census-2026-09-25/run.sh runs it (cuInit x3,
# cuCtxCreate/cuCtxDestroy x5, the ops once), census-table.py, rpc-cost.py and calls.py on its
# trace, then bench-run.sh init and churn with 3 reps; every mode but before adds the cuda gate.
# Host watch around the part, vm0 down at the end, host Xids.
# Usage: run.sh before|defer-pool|final|gather [label]
set -uo pipefail
MODE=${1:?usage: run.sh before|defer-pool|final|gather [label]}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
C=$HERE/$MODE${2:+-$2}
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
cd "$ROOT" || exit 1
export LT=/home/silas/git/Leandro-Test CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test
export LEA_HOSTVENV=/home/silas/git/Leandro/vendor/hostvenv
REPS=${REPS:-10}
CTX=${CTX:-5}

census_guest='cd ~/bench && export CENSUS_PTX=$HOME/bench/kernels.ptx CENSUS_CUBIN=$HOME/bench/census.cubin
R0=$(for d in /sys/bus/pci/devices/*; do [ "$(cat $d/vendor)" = 0x10de ] && echo $d/resource0; done | head -1)
echo "# marker resource $R0"
E="sudo timeout 240 env LD_LIBRARY_PATH=/opt/nvrm/lib CENSUS_PTX=$CENSUS_PTX CENSUS_CUBIN=$CENSUS_CUBIN CENSUS_MARK=$R0"
for i in 1 2 3; do $E ./census-cu init || exit 1; sleep 1; done
$E ./census-cu ctx '"$CTX"' || exit 1
$E ./census-cu ops '"$REPS"' || exit 1'

if pgrep -f /mnt/vmstore/caraxes/vm0/ >/dev/null; then
    echo "ERROR: a vm0 process runs; refusing to start" >&2; pgrep -af /mnt/vmstore/caraxes/vm0/ >&2; exit 2
fi
mkdir -p "$C"
# intr-page-2026-09-24/env.sh, then the brief's changes (1 GiB, slot 0, no display, memfd, no pool).
source "$ROOT/caraxes/runs/intr-page-2026-09-24/env.sh"
export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=0
unset CARAXES_DISPLAY CARAXES_DISPLAY_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_BAR1_POOL_MIB CARAXES_DOORBELL_IOEVENTFD
unset CARAXES_ALLOC_LAZY CARAXES_TRACE_ACCESSES CARAXES_TRACE_FLUSH CARAXES_BAR2_MEMFD_SYNC_DOORBELL
export CARAXES_INTR_PAGE=1 CARAXES_BAR2_MEMFD=1
unset LEA_TOOLS CARAXES_SHADOW_DEFER_TLB CARAXES_SHADOW_REGION_POOL CARAXES_SHADOW_BULK_FREE CARAXES_SHADOW_GATHER
case $MODE in
    before) export CARAXES_SHADOW_DEFER_TLB=0 CARAXES_SHADOW_REGION_POOL=0 BIN_ID=4fec5bd ;;
    defer-pool) BIN_ID=4fec5bd ;;
    final) BIN_ID=f0edbb8 ;;
    gather) BIN_ID=ac950e9 ;;
    *) echo "ERROR: unknown mode $MODE" >&2; exit 2 ;;
esac
export CARAXES_SERVER=${OPT_SERVER:-/mnt/vmstore/caraxes/vfio-user-nvgpu-opt2-$BIN_ID}
W=/mnt/vmstore/caraxes/$VM; T0=$(date '+%F %T')
{ head -1 /proc/driver/nvidia/version; sha256sum "$CARAXES_SERVER"; git -C "$ROOT" log --oneline -1
  (cd "$ROOT/target/bench/bin" && sha256sum census-cu census.cubin kernels.ptx bench-cu 2>/dev/null)
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
    echo "ERROR: vm0 did not come up"; exit 1
fi
pgrep -af "vfio-user-nvgpu.*$W/" > "$C/device-pid.txt"
tools/caraxes-vm.sh probe census >> "$C/up.txt" 2>&1; tail -2 "$C/up.txt" | cut -c1-200
caraxes/tools/bench-run.sh v2-nodes >> "$C/up.txt" 2>&1
grep -E 'intr page|bar2 memfd|bar1 passthrough|bar1 pool|doorbell tokens|doorbell poll|reserved host VRAM|host ABI|trace' "$W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
caraxes/tools/bench-run.sh stage v2 >> "$C/up.txt" 2>&1
tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"

# The census: one trace slice for all census-cu processes.
L0=$(wc -l < "$W/gpu-trace.log")
tools/caraxes-vm.sh ssh "$census_guest" > "$C/census.txt" 2>&1
echo "census rc=$? $(grep -c '^OP' "$C/census.txt") OP lines"
tail -n +"$((L0 + 1))" "$W/gpu-trace.log" > "$C/census-trace.txt"
python3 caraxes/tools/census-table.py "$C/census-trace.txt" --names "$C/census.txt" \
    --json "$C/census.json" > "$C/census-table.md"
python3 caraxes/tools/rpc-cost.py "$C/census-trace.txt" > "$C/census-rpc-cost.md" 2>&1
python3 "$HERE/calls.py" "$C/census-trace.txt" --names "$C/census.txt" > "$C/calls.md" 2>&1

# init and churn, 3 reps.
BENCH_TIMEOUT=300 caraxes/tools/bench-run.sh series --targets v2 --reps "${BENCH_REPS:-3}" --metrics "init churn" --out "$C" 2>&1 | tail -2
if [[ $MODE != before ]]; then
    tools/caraxes-gate.sh cuda > "$C/cuda-gate.txt" 2>&1; tail -3 "$C/cuda-gate.txt"
fi

caraxes/tools/bench-hostwatch.sh stop "$C"
tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
grep -E 'intr page|bar2 memfd|doorbell|mirror: |shadow: VAS|ERROR|WARN' "$W/gpu.log" | tail -60 | cut -c1-1600 > "$C/device-summary.txt"
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
xz -T0 "$C/census-trace.txt"
echo "$MODE done $(date -Iseconds); host Xid lines $(grep -c Xid "$C/host-xid.txt"); vm0 left: $(pgrep -f /mnt/vmstore/caraxes/vm0/ | wc -l)"

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The allocation path, night part (docs/caraxes/alloc-path.md §6): vm2, 610 guest, 1 GiB
# profile at slot 3, main 57693d8 frozen, pool, token pool, hybrid reactor
# (caraxes/runs/night-2026-09-24/env.sh; the night configuration instead of §6's P21 area
# and spin reactor, so stack-off repeats the "on" mode of the BAR2 memfd night part). Modes:
#   off        interrupt page off, BAR2 memfd off, lazy off: the 24-trap baseline (§1.1)
#   on         CARAXES_ALLOC_LAZY=1 alone
#   intr       CARAXES_INTR_PAGE=1 alone (also the memfd-off baseline of the BAR2 memfd night
#              part's on-nopool: same configuration)
#   stack-off  CARAXES_INTR_PAGE=1 CARAXES_BAR2_MEMFD=1 (step 1)
#   stack-on   step 1 plus CARAXES_ALLOC_LAZY=1
# Per mode: one traced `bench-cu-gap churn 1` with its phases 300 ms apart and rpc-cost.py's
# per-operation table, then init and churn with 5 reps (restart per rep); the cuda gate in the
# modes of GATE_MODES (default stack-on); host watch, Xids.
# bench-cu-gap is bench-cu of this tree (BENCH_CU_PHASE_GAP_MS), built in the guest the way
# bench-cu was (caraxes/runs/bench-2026-09-24/commands.txt:19).
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
source "$HERE/../../night-2026-09-24/env.sh"
night_guest 610
cd "$NIGHT_ROOT" || exit 1
LIB=$(caraxes/tools/bench-run.sh libdir v2)
# No BAR1 pool in any mode: with the pool and CARAXES_BAR2_MEMFD=1 the first context creation
# hangs (caraxes/runs/bar2-memfd-2026-09-24/night/on, on-syncdb, on-nointr; on-nopool runs).
unset CARAXES_BAR1_PASSTHROUGH
for mode in ${MODES:-off intr stack-off stack-on}; do
  night_vm2_free || exit 2
  unset CARAXES_ALLOC_LAZY CARAXES_INTR_PAGE CARAXES_BAR2_MEMFD
  [[ $mode == on || $mode == stack-on ]] && export CARAXES_ALLOC_LAZY=1
  [[ $mode == stack-* ]] && export CARAXES_INTR_PAGE=1 CARAXES_BAR2_MEMFD=1
  [[ $mode == intr ]] && export CARAXES_INTR_PAGE=1
  D=$HERE/$mode; [[ -e $D ]] && { echo "ERROR: $D exists" >&2; exit 2; }
  mkdir -p "$D"; T0=$(date '+%F %T'); night_setup "$D"
  caraxes/tools/bench-hostwatch.sh start "$D"
  caraxes/tools/bench-run.sh v2-restart > "$D/up.txt" 2>&1 || { tail -3 "$D/up.txt"; caraxes/tools/bench-hostwatch.sh stop "$D"; tools/caraxes-vm.sh down >/dev/null 2>&1; continue; }
  grep -E 'alloc lazy|intr page|bar2 memfd|bar1 (passthrough|pool)|doorbell tokens|doorbell poll|host ABI|ERROR' "$NIGHT_W/gpu.log" | cut -c1-300 > "$D/device-start.txt"
  caraxes/tools/bench-run.sh stage v2 >> "$D/up.txt" 2>&1
  tools/caraxes-vm.sh ssh 'cat > ~/bench/bench-cu.c' < caraxes/tools/bench-cu.c
  tools/caraxes-vm.sh ssh "cd ~/bench && inc=\$(ls -d \$HOME/gpu/venv/lib/python3*/site-packages/nvidia/cu13/include | head -1) && gcc -O2 -Wall -o bench-cu-gap bench-cu.c -I\$inc -L$LIB -lcuda && sha256sum bench-cu-gap" >> "$D/up.txt" 2>&1
  L0=$(wc -l < "$NIGHT_W/gpu-trace.log")
  tools/caraxes-vm.sh ssh "cd ~/bench && BENCH_CU_PHASE_GAP_MS=300 LD_LIBRARY_PATH=$LIB ./bench-cu-gap churn 1" > "$D/churn-traced.txt" 2>&1
  tail -n +"$((L0 + 1))" "$NIGHT_W/gpu-trace.log" > "$D/churn-trace.txt"
  python3 caraxes/tools/rpc-cost.py "$D/churn-trace.txt" --gap-ms 200 \
    --phases ctx:0,alloc1:64,free1:64,alloc64:8,free64:8,destroy:0 > "$D/rpc-cost.md" 2>&1
  V2_RESTART=1 caraxes/tools/bench-run.sh series --targets v2 --reps "${REPS:-5}" --metrics "init churn" --out "$D" 2>&1 | tail -2
  if [[ " ${GATE_MODES:-stack-on} " == *" $mode "* ]]; then
    tools/caraxes-vm.sh sync-tests > "$D/sync-tests.txt" 2>&1
    CARAXES_GATE_RUNS=$D tools/caraxes-gate.sh cuda > "$D/cuda-gate.txt" 2>&1; echo "$mode cuda gate rc=$?"; tail -2 "$D/cuda-gate.txt" | cut -c1-200
  fi
  caraxes/tools/bench-hostwatch.sh stop "$D"
  tools/caraxes-vm.sh down > "$D/down.txt" 2>&1
  grep -E 'alloc lazy|mirror: |syncs \(|intr page|bar2 memfd|ERROR' "$NIGHT_W/gpu.log" | cut -c1-900 > "$D/device-summary.txt"
  night_xid "$D" "$T0"
  xz -T0 "$D/churn-trace.txt"
  echo "$mode: $(grep -h CHURN "$D/churn-traced.txt" | head -1) Xid $(grep -c Xid "$D/host-xid.txt")"
done
cols=(); for m in off intr stack-off stack-on on; do [[ -d $HERE/$m/v2 ]] && cols+=("$HERE/$m/v2"); done
python3 caraxes/tools/bench-summary.py "${cols[@]}" > "$HERE/summary.md"
echo "alloc path night part done $(date -Iseconds)"

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# BAR2 memfd, night part (docs/caraxes/bar2-memfd.md §7; the earlier off/ is partial): vm2,
# 610 guest, 1 GiB profile at slot 3, main 57693d8 frozen, pool, token pool, hybrid reactor,
# interrupt page on in both modes (caraxes/runs/night-2026-09-24/env.sh); CARAXES_BAR2_MEMFD
# off against on: the trapped BAR2 accesses of one context creation, bench-cu init and churn
# with 5 reps (restart per rep), the cuda gate once with the memfd on, host watch, Xids.
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
source "$HERE/../../night-2026-09-24/env.sh"
night_guest 610
cd "$NIGHT_ROOT" || exit 1
for mode in ${MODES:-off on}; do
  night_vm2_free || exit 2
  # on-syncdb: the reference setting of bar2-memfd.md §3 rule 2 (sync before every usermode
  # doorbell), the follow-up §7 names when on is red; on-nointr: on without the interrupt page.
  unset CARAXES_BAR2_MEMFD CARAXES_BAR2_MEMFD_SYNC_DOORBELL; export CARAXES_INTR_PAGE=1
  case $mode in on*) export CARAXES_BAR2_MEMFD=1 ;; esac
  [[ $mode == on-syncdb ]] && export CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1
  [[ $mode == on-nointr ]] && unset CARAXES_INTR_PAGE
  # on-nopool, off-nopool: without the BAR1 pool (on, on-syncdb and on-nointr hung in
  # cuCtxCreate with the pool; off-nopool is the baseline for on-nopool).
  export CARAXES_BAR1_PASSTHROUGH=pool; [[ $mode == *-nopool ]] && unset CARAXES_BAR1_PASSTHROUGH
  D=$HERE/$mode; [[ -e $D ]] && { echo "ERROR: $D exists" >&2; exit 2; }
  mkdir -p "$D"; T0=$(date '+%F %T'); night_setup "$D"
  caraxes/tools/bench-hostwatch.sh start "$D"
  caraxes/tools/bench-run.sh v2-restart > "$D/up.txt" 2>&1 || { tail -3 "$D/up.txt"; caraxes/tools/bench-hostwatch.sh stop "$D"; tools/caraxes-vm.sh down >/dev/null 2>&1; continue; }
  grep -E 'bar2 memfd|intr page|bar1 (passthrough|pool)|doorbell tokens|doorbell poll|ERROR' "$NIGHT_W/gpu.log" | cut -c1-300 > "$D/device-start.txt"
  caraxes/tools/bench-run.sh stage v2 >> "$D/up.txt" 2>&1
  L0=$(wc -l < "$NIGHT_W/gpu-trace.log")
  # Bounded: the first on run (on/) hung in cuCtxCreate for 16 minutes (on/hang-guest.txt).
  tools/caraxes-vm.sh ssh "cd ~/bench && LD_LIBRARY_PATH=$(caraxes/tools/bench-run.sh libdir v2) timeout ${INIT_TIMEOUT:-120} ./bench-cu init; echo rc=\$?" > "$D/init-once.txt" 2>&1
  tail -n +"$((L0 + 1))" "$NIGHT_W/gpu-trace.log" > "$D/init-trace.txt"
  awk '$3=="bar3"{print $2, $4}' "$D/init-trace.txt" | sort | uniq -c | sort -rn | head > "$D/bar2-traps.txt"
  awk '$3=="bar3" && $2=="W"' "$D/init-trace.txt" | wc -l > "$D/bar2-trapped-writes.txt"
  awk '$3=="bar3"' "$D/init-trace.txt" | wc -l > "$D/bar2-trapped-accesses.txt"
  if ! grep -q '^INIT' "$D/init-once.txt"; then
    echo "$mode: the traced context creation did not finish ($(tail -1 "$D/init-once.txt")); series skipped"
    tools/caraxes-vm.sh ssh "sudo dmesg | grep -v 'NVRM: ioctl' | tail -60" > "$D/hang-guest.txt" 2>&1
  else
  BENCH_TIMEOUT=${BENCH_TIMEOUT:-300} V2_RESTART=1 caraxes/tools/bench-run.sh series --targets v2 --reps "${REPS:-5}" --metrics "init churn" --out "$D" 2>&1 | tail -2
  fi
  if [[ $mode == on* ]] && grep -q '^INIT' "$D/init-once.txt"; then
    tools/caraxes-vm.sh sync-tests > "$D/sync-tests.txt" 2>&1
    CARAXES_GATE_RUNS=$D tools/caraxes-gate.sh cuda > "$D/cuda-gate.txt" 2>&1; echo "cuda gate rc=$?"; tail -3 "$D/cuda-gate.txt" | cut -c1-200
  fi
  caraxes/tools/bench-hostwatch.sh stop "$D"
  tools/caraxes-vm.sh down > "$D/down.txt" 2>&1
  grep -E 'bar2 memfd|intr page|bar tlb|ERROR' "$NIGHT_W/gpu.log" | cut -c1-900 > "$D/device-summary.txt"
  night_xid "$D" "$T0"
  xz -T0 "$D/init-trace.txt"
  echo "$mode: trapped BAR2 writes in one init $(cat "$D/bar2-trapped-writes.txt"), Xid $(grep -c Xid "$D/host-xid.txt"); $(grep -h INIT "$D"/v2/init-*.txt | head -2 | tr '\n' ' ')"
done
cols=(); for m in off on on-syncdb on-nointr off-nopool on-nopool; do [[ -d $HERE/$m/v2 ]] && cols+=("$HERE/$m/v2"); done
python3 caraxes/tools/bench-summary.py "${cols[@]}" > "$HERE/summary.md"
echo "bar2 memfd night part done $(date -Iseconds)"

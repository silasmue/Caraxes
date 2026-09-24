#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# BAR2 memfd (docs/caraxes/bar2-memfd.md §7): vm2 at slot 0 with the 2 GiB profile, main f343f87
# frozen, CARAXES_BAR2_MEMFD off against on; the trapped BAR2 accesses of one context creation,
# bench-cu init and churn with 5 reps, the cuda gate, Xids. Token pool and spin reactor on.
set -uo pipefail
R=/home/silas/git/Caraxes; M=$R/caraxes/runs/bar2-memfd-2026-09-24; cd "$R"
source "$R/caraxes/runs/mapping-2026-09-24/env.sh"
export CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-bar2-f343f87 LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=spin CARAXES_BAR1_PASSTHROUGH=pool
unset CARAXES_DOORBELL_IOEVENTFD
W=/mnt/vmstore/caraxes/vm2
for mode in ${MODES:-off on}; do
  if [[ $mode == on ]]; then export CARAXES_BAR2_MEMFD=1; else unset CARAXES_BAR2_MEMFD; fi
  D=$M/$mode; mkdir -p "$D"; T0=$(date '+%F %T')
  caraxes/tools/bench-hostwatch.sh start "$D"
  caraxes/tools/bench-run.sh v2-restart > "$D/up.txt" 2>&1 || { tail -3 "$D/up.txt"; caraxes/tools/bench-hostwatch.sh stop "$D"; continue; }
  grep -E 'bar2 memfd|bar1 (passthrough|pool)|doorbell tokens|doorbell poll' "$W/gpu.log" | cut -c1-300 > "$D/device-start.txt"
  caraxes/tools/bench-run.sh stage v2 >> "$D/up.txt" 2>&1
  L0=$(wc -l < "$W/gpu-trace.log")
  tools/caraxes-vm.sh ssh 'cd ~/bench && LD_LIBRARY_PATH=/opt/nvrm/lib ./bench-cu init' > "$D/init-once.txt" 2>&1
  tail -n +"$((L0 + 1))" "$W/gpu-trace.log" > "$D/init-trace.txt"
  awk '$3=="bar3"{print $2, $4}' "$D/init-trace.txt" | sort | uniq -c | sort -rn | head > "$D/bar2-traps.txt"
  awk '$3=="bar3" && $2=="W"' "$D/init-trace.txt" | wc -l > "$D/bar2-trapped-writes.txt"
  V2_RESTART=1 caraxes/tools/bench-run.sh series --targets v2 --reps "${REPS:-5}" --metrics "init churn" --out "$D" 2>&1 | tail -2
  tools/caraxes-gate.sh cuda > "$D/cuda-gate.txt" 2>&1; tail -3 "$D/cuda-gate.txt"
  caraxes/tools/bench-hostwatch.sh stop "$D"
  tools/caraxes-vm.sh down > "$D/down.txt" 2>&1
  grep -E 'bar2 memfd|ERROR' "$W/gpu.log" | cut -c1-600 > "$D/device-summary.txt"
  journalctl -k --since "$T0" --no-pager -g Xid > "$D/host-xid.txt" 2>/dev/null
  echo "$mode: trapped BAR2 writes in one init $(cat "$D/bar2-trapped-writes.txt"), Xid $(grep -c Xid "$D/host-xid.txt"); $(grep -h INIT "$D"/v2/init-*.txt | head -2 | tr '\n' ' ')"
done
python3 caraxes/tools/bench-summary.py "$M/off/v2" "$M/on/v2" > "$M/summary.md"
echo "bar2 memfd series done $(date -Iseconds)"

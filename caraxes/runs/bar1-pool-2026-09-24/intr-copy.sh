#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The copy rows with the interrupt page (docs/caraxes/intr-page.md): vm2, 1 MiB area, token
# pool, hybrid reactor (the page requires spin or hybrid); CARAXES_INTR_PAGE off against on;
# copy, rl, short-auto with 5 reps and PPO with 2. Hypothesis (benchmark.md, design H section):
# the 0.9 ms stall of a 16 MiB d2d copy is the trapped completion interrupt.
set -uo pipefail
R=/home/silas/git/Caraxes; M=$R/caraxes/runs/intr-copy-2026-09-24; mkdir -p "$M"; cd "$R"
source "$R/caraxes/runs/mapping-2026-09-24/env.sh"
export CARAXES_SERVER=${CARAXES_SERVER:-/mnt/vmstore/caraxes/vfio-user-nvgpu-main-f48a655} LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_BAR1_PASSTHROUGH=1 CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
unset CARAXES_DOORBELL_IOEVENTFD
for mode in ${MODES:-off on}; do
  if [[ $mode == on ]]; then export CARAXES_INTR_PAGE=1; else unset CARAXES_INTR_PAGE; fi
  D=$M/$mode; mkdir -p "$D"; T0=$(date '+%F %T')
  caraxes/tools/bench-hostwatch.sh start "$D"
  caraxes/tools/bench-run.sh v2-restart > "$D/up.txt" 2>&1 || { tail -3 "$D/up.txt"; caraxes/tools/bench-hostwatch.sh stop "$D"; continue; }
  grep -E 'intr page|doorbell poll|doorbell tokens|bar1 passthrough' /mnt/vmstore/caraxes/vm2/gpu.log | cut -c1-300 > "$D/device-start.txt"
  caraxes/tools/bench-run.sh stage v2 >> "$D/up.txt" 2>&1
  V2_RESTART=1 caraxes/tools/bench-run.sh series --targets v2 --reps "${REPS:-5}" --metrics "copy rl short-auto" --out "$D" 2>&1 | tail -2
  PPO_SECONDS=60 caraxes/tools/bench-run.sh series --targets v2 --reps 2 --metrics ppo --out "$D/ppo" 2>&1 | tail -1
  caraxes/tools/bench-hostwatch.sh stop "$D"
  tools/caraxes-vm.sh down > "$D/down.txt" 2>&1
  grep -E 'intr page|ERROR' /mnt/vmstore/caraxes/vm2/gpu.log | cut -c1-500 | tail -3 > "$D/device-summary.txt"
  journalctl -k --since "$T0" --no-pager -g Xid > "$D/host-xid.txt" 2>/dev/null
  echo "$mode: $(grep -h 'BW dir=d2d' "$D"/v2/copy-1.txt | grep -o 'size_mib=[0-9]* .*med_us=[0-9.]*' | tr '\n' ';') Xid $(grep -c Xid "$D/host-xid.txt")"
done
python3 caraxes/tools/bench-summary.py "$M/off/v2" "$M/on/v2" > "$M/summary.md"
echo "intr copy series done $(date -Iseconds)"

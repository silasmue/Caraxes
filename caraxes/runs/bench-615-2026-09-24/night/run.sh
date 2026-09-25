#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The like-for-like 615 series, night part (docs/caraxes/benchmark.md "First 615 series",
# "Next"): the closing series' configuration (caraxes/runs/closing-610-2026-09-24/run.sh: P21
# area CARAXES_BAR1_PASSTHROUGH=1 (refused by this binary: the pool instead), token pool 64, spin reactor; all metrics of bench-run.sh
# with 5 reps and a restart per rep, the startup metric with 5, then PPO 60 s with 2) with the
# 615 guest and then the 610 guest, both on vm2 at 1 GiB slot 3 with main 57693d8 frozen
# (caraxes/runs/night-2026-09-24/env.sh; the interrupt page stays on, the night default).
# Only on a quiet host: before each guest it waits until no other device process runs
# (`pgrep -af vfio-user-nvgpu`), polling every five minutes for up to three hours
# (QUIET_WAIT_MIN); the MEAS lines record any that appears during the part.
# Usage: run.sh [615 610]   (results in <guest>/ next to this script, never overwritten)
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
source "$HERE/../../night-2026-09-24/env.sh"
# The frozen binary refuses the fixed-layout area (CARAXES_BAR1_PASSTHROUGH=1: "ERROR: ... is
# refused: it mismatched 128 pages on 2026-09-24; ... starting without the area", the first
# attempt, 615-area-refused/device-start.txt), so the pool takes its place as the task says.
export CARAXES_BAR1_PASSTHROUGH=pool CARAXES_DOORBELL_POLL=spin
cd "$NIGHT_ROOT" || exit 1
for g in "${@:-615 610}"; do
  for g1 in $g; do
    night_guest "$g1" || exit 2
    C=$HERE/$g1; [[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
    night_vm2_free || exit 2
    night_quiet || { echo "$(date +%T) $g1: the host did not become quiet within ${QUIET_WAIT_MIN:-180} min; part skipped"; continue; }
    mkdir -p "$C"; T0=$(date '+%F %T'); night_setup "$C"
    echo "$(date +%T) quiet host, $g1 guest starts"
    caraxes/tools/bench-hostwatch.sh start "$C"
    caraxes/tools/bench-run.sh v2-restart > "$C/up.txt" 2>&1 || { tail -3 "$C/up.txt"; caraxes/tools/bench-hostwatch.sh stop "$C"; tools/caraxes-vm.sh down >/dev/null 2>&1; continue; }
    grep -E 'host ABI|guest ABI|intr page|bar1 passthrough|doorbell tokens|doorbell poll|ERROR' "$NIGHT_W/gpu.log" | cut -c1-300 | head -12 > "$C/device-start.txt"
    caraxes/tools/bench-run.sh stage v2 > "$C/stage.txt" 2>&1
    caraxes/tools/bench-run.sh startup v2 --out "$C"
    V2_RESTART=1 caraxes/tools/bench-run.sh series --targets v2 --reps "${REPS:-5}" --out "$C"
    V2_RESTART=1 PPO_SECONDS=60 caraxes/tools/bench-run.sh series --targets v2 --reps 2 --metrics ppo --out "$C/ppo"
    caraxes/tools/bench-hostwatch.sh stop "$C"
    tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
    grep -E 'bar1 passthrough|doorbell tokens|intr page|unanswered|ERROR' "$NIGHT_W/gpu.log" | cut -c1-600 | tail -8 > "$C/device-summary.txt"
    night_xid "$C" "$T0"
    python3 caraxes/tools/bench-summary.py "$C/v2" > "$C/summary.md" 2>&1
    echo "$(date +%T) bench-615 night $g1 done; Xid lines $(grep -c Xid "$C/host-xid.txt"); other device processes during runs: $(grep -h '^# procs' "$C"/v2/*.txt "$C"/ppo/v2/*.txt | grep -c -- '-dev.*-dev')"
  done
done

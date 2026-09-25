#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The workload gate, night part (docs/caraxes/workload-gate.md): ../run.sh --targets "native v2"
# (3 reps, the default metrics, about 70 minutes) on the 615 guest (vm2-615, 1 GiB at slot 3,
# main 57693d8 frozen v615 build, env.sh here), on a quiet host: it first waits until no other
# device process runs (night_quiet, every five minutes, up to three hours). Results in gate/.
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
source "$HERE/env.sh"
[[ -e $HERE/gate ]] && { echo "ERROR: $HERE/gate exists" >&2; exit 2; }
night_vm2_free || exit 2
night_quiet || { echo "$(date +%T) the host did not become quiet; workload gate skipped"; exit 1; }
echo "$(date +%T) quiet host, workload gate starts"
export WORKLOAD_ENV=$HERE/env.sh CARAXES_SERVER=$NIGHT_BIN615
"$NIGHT_ROOT/caraxes/runs/workload-2026-09-24/run.sh" --targets "native v2" --out "$HERE/gate"
rc=$?
"$NIGHT_ROOT/tools/caraxes-vm.sh" down > "$HERE/gate/down.txt" 2>&1
grep -E 'bar1 pool|doorbell tokens|intr page|unanswered|ERROR' "$NIGHT_W/gpu.log" | cut -c1-600 | tail -8 > "$HERE/gate/v2-device-exit.txt"
night_xid "$HERE/gate" "$(head -1 "$HERE/gate/setup.txt" | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:]{8}' | tr T ' ')"
echo "workload gate night part done rc=$rc $(date -Iseconds)"

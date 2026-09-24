#!/bin/bash
# SPDX-License-Identifier: MIT
# One guest experiment on vm0 (docs/caraxes/display-cd.md):
#   bash caraxes/runs/display-cd-2026-09-24/exp.sh <label> <timeout-s> <guest shell command>
# Saves the command, the guest output and the events of the device trace under
# caraxes/runs/display-cd-2026-09-24/<label>/. The full trace window (every register and
# BAR1 access, hundreds of MB for a compositor run) stays in the VM store.
set -u
HERE=$(cd "$(dirname "$0")" && pwd -P)
WT=$(cd "$HERE/../../.." && pwd -P)
LABEL=$1 TMO=$2; shift 2
OUT=$HERE/$LABEL
TRACE=/mnt/vmstore/caraxes/vm0/gpu-trace.log
FULL=/mnt/vmstore/caraxes/vm0/display-cd/$LABEL
mkdir -p "$OUT" "$FULL"
start=$(wc -l < "$TRACE")
{ echo "# SPDX-License-Identifier: MIT"; echo "# $(date -Iseconds) VM=vm0 guest command (timeout ${TMO}s):"; echo "$*"; } > "$OUT/command.txt"
{ echo "# SPDX-License-Identifier: MIT"; timeout "$TMO" env VM=vm0 "$WT/tools/caraxes-vm.sh" ssh "$*" 2>&1; echo "exit=$?"; } > "$OUT/guest.txt"
tail -n +"$((start + 1))" "$TRACE" > "$FULL/device-trace-full.txt"
bash "$HERE/events.sh" "$LABEL"
echo "== $LABEL: guest output ($(wc -l < "$OUT/guest.txt") lines), trace $(wc -l < "$FULL/device-trace-full.txt") lines, $(wc -l < "$OUT/device-trace.txt") events -> $OUT"

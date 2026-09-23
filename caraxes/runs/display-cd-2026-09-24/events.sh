#!/bin/bash
# SPDX-License-Identifier: MIT
# events.sh <label>: write <label>/device-trace.txt, the event lines (RPCs, display notes,
# MSIs) of the full trace window kept in the VM store (moved there if still in the run).
set -u
HERE=$(cd "$(dirname "$0")" && pwd -P)
for L in "$@"; do
FULL=/mnt/vmstore/caraxes/vm0/display-cd/$L
mkdir -p "$FULL"
[ -f "$HERE/$L/device-trace-full.txt" ] && mv "$HERE/$L/device-trace-full.txt" "$FULL/"
[ -f "$FULL/device-trace-full.txt" ] || continue
{
    echo "# SPDX-License-Identifier: MIT"
    echo "# events of the device trace (full window: $FULL/device-trace-full.txt)"
    grep -E '^ *[0-9]+ E ' "$FULL/device-trace-full.txt"
} > "$HERE/$L/device-trace.txt"
done

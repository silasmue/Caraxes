#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The vm1 queue of 2026-09-25 00:15: rung 1's identity rerun (the first run's host watch had
# stopped the device before the identity was read), then the SM-version rungs 7.0 and 8.6 on
# the TU106 identity, one at a time, each stopped at the first host Xid outside the device
# pid or host watch alert (gpu-run.sh small). A stop ends the queue.
set -uo pipefail
R=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
HW_SLOW=5 "$R/gpu-run.sh" "$R/r1-tu104/gpu-ident" ident CARAXES_COSTUME=rtx2080 > "$R/r1-tu104/gpu-ident-transcript.txt" 2>&1
tail -2 "$R/r1-tu104/gpu-ident-transcript.txt"
for sm in 7.0 8.6; do
    d=$R/sm${sm/./}-tu106; mkdir -p "$d"
    "$R/gpu-run.sh" "$d/gpu" small CARAXES_SM_VERSION=$sm > "$d/gpu-transcript.txt" 2>&1
    tail -6 "$d/gpu-transcript.txt"
    if grep -q "STOP\|ALERT" "$d/gpu-transcript.txt" "$d/gpu/hostwatch.txt" 2>/dev/null; then
        echo "queue stopped after SM $sm"; exit 1
    fi
done

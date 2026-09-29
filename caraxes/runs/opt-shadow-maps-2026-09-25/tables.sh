#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Regenerate the tables of every run of this series from its census trace: calls.md (calls.py),
# census-rpc-cost.md (caraxes/tools/rpc-cost.py, with the RM call histogram), compare.md
# (compare.py) and bench-summary.md (caraxes/tools/bench-summary.py over the bench-run.sh runs).
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
cd "$HERE" || exit 1
RUNS=(before defer-pool final gather gather-2)
for d in "${RUNS[@]}"; do
    [[ -d $d ]] || continue
    python3 calls.py "$d/census-trace.txt.xz" --names "$d/census.txt" > "$d/calls.md"
    python3 ../../tools/rpc-cost.py "$d/census-trace.txt.xz" > "$d/census-rpc-cost.md" 2>&1
done
python3 compare.py "${RUNS[@]}" > compare.md
python3 ../../tools/bench-summary.py before/v2 defer-pool/v2 final/v2 gather/v2 gather-2/v2 > bench-summary.md 2>&1

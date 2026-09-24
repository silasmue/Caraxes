#!/bin/bash
# SPDX-License-Identifier: MIT
# Run one display gate on vm0 from this worktree (docs/caraxes/display-cd.md):
#   bash caraxes/runs/display-cd-2026-09-24/gate.sh vdisplay|display <tag>
# The worktree sits below .claude/worktrees, so Leandro-Test is named explicitly.
set -u
HERE=$(cd "$(dirname "$0")" && pwd -P)
WT=$(cd "$HERE/../../.." && pwd -P)
export CARAXES_LEANDRO_TEST=${CARAXES_LEANDRO_TEST:-/home/silas/git/Leandro-Test}
export VM=vm0
timeout 1500 "$WT/tools/caraxes-gate.sh" "$1" > "$HERE/gate-$1-$2.json" 2> "$HERE/gate-$1-$2.stderr"
rc=$?
echo "exit=$rc"
cat "$HERE/gate-$1-$2.json"
grep -E "GATE" "$HERE/gate-$1-$2.stderr"

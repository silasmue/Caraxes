#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The cost of the untrapped-GPPut rule (docs/caraxes/bar1-pool.md §9): the matrix series'
# allocation-side metrics (churn: bench-cu churn 20, cuMemAlloc/cuMemFree; init: bench-cu init,
# cuInit and cuCtxCreate; short-auto: nvprobe 4 with short kernels) on vm5-615 in the matrix
# configuration with this directory's build, once with the rule off
# (CARAXES_SHADOW_UNTRAPPED_HOLD=0) and once on, a fresh device per pass.
# Usage: bench.sh [label]
set -uo pipefail
HERE=$(dirname "$(readlink -f "$0")")
source "$HERE/../matrix-2026-09-25/env.sh"
mx_guest 615 || exit 2
export CARAXES_SERVER=${XP_BIN:-/mnt/vmstore/caraxes/vfio-user-nvgpu-xidpool-4d4306e}
C=$HERE/bench-615${1:+-$1}
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
mx_free || exit 2
if pgrep -f bench-v1 >/dev/null; then echo "ERROR: a bench-v1 process runs (the tap is shared)" >&2; exit 2; fi
mkdir -p "$C"; cd "$MX_ROOT" || exit 1
T0=$(date '+%F %T')
mx_setup "$C"
caraxes/tools/bench-hostwatch.sh start "$C"
for hold in 0 1; do
    export CARAXES_SHADOW_UNTRAPPED_HOLD=$hold
    D=$C/hold$hold; mkdir -p "$D"
    caraxes/tools/bench-run.sh v2-restart > "$D/v2-up.txt" 2>&1 || { echo "v2 did not come up"; tail -5 "$D/v2-up.txt"; continue; }
    caraxes/tools/bench-run.sh stage v2 >> "$D/v2-up.txt" 2>&1
    caraxes/tools/bench-run.sh series --targets v2 --reps 3 --metrics "churn init short-auto" --out "$D" 2>&1 | tail -2
    "$LEA_TOOLS/caraxes-vm.sh" down > "$D/down.txt" 2>&1
    grep -a '^mirror: .*untrapped GPPut' "$MX_W/gpu.log" | tail -1 | cut -c1-1600 > "$D/device-mirror-summary.txt"
done
caraxes/tools/bench-hostwatch.sh stop "$C"
python3 caraxes/tools/bench-summary.py "$C/hold0/v2" "$C/hold1/v2" > "$C/summary.md" 2>&1
mx_finish "$C" "$T0"

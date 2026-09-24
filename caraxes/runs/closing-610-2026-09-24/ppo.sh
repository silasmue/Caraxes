#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# PPO CartPole (caraxes/tools/pporl.py, the operator's script) on native, v1 and v2 right after
# the closing series, same environment: 2 reps of 60 s each.
set -uo pipefail
R=/home/silas/git/Caraxes; F=$R/caraxes/runs/closing-610-2026-09-24/ppo
mkdir -p "$F"; cd "$R"
source "$R/caraxes/runs/mapping-2026-09-24/env.sh"
export CARAXES_SERVER=$(grep -o '/mnt/vmstore/caraxes/vfio-user-nvgpu-closing-610-[0-9a-f]*' "$R/caraxes/runs/closing-610-2026-09-24/run.sh" | head -1)
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_BAR1_PASSTHROUGH=1 CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=${CARAXES_DOORBELL_POLL:-spin}
unset CARAXES_DOORBELL_IOEVENTFD
caraxes/tools/bench-hostwatch.sh start "$F"
caraxes/tools/bench-run.sh v2-restart > "$F/v2-up.txt" 2>&1
caraxes/tools/bench-run.sh stage v2 >> "$F/v2-up.txt" 2>&1
caraxes/tools/bench-run.sh stage v1 > "$F/v1-up.txt" 2>&1
PPO_SECONDS=${PPO_SECONDS:-60} caraxes/tools/bench-run.sh series --targets "native v1 v2" --reps "${REPS:-2}" --metrics ppo --out "$F"
caraxes/tools/bench-hostwatch.sh stop "$F"
python3 caraxes/tools/bench-summary.py "$F/native" "$F/v1" "$F/v2" > "$F/summary.md"
grep -h '# PPO' "$F"/*/ppo-*.txt
echo "ppo done $(date -Iseconds)"

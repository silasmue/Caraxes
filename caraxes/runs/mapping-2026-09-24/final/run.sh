#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Final series of the mapping package (docs/caraxes/benchmark.md, dated section 2026-09-24
# afternoon): native, v1 and v2 interleaved, N=5, restart per rep on both paths. v2 = the
# checkpoint b58cb12 (target/mapping-commit), vm2 slot 0, 2 GiB profile.
set -uo pipefail
R=/home/silas/git/Caraxes; M=$R/caraxes/runs/mapping-2026-09-24; F=$M/final
cd "$R"
source "$M/env.sh"
export CARAXES_SERVER=$R/target/mapping-commit/release/vfio-user-nvgpu
sha256sum "$CARAXES_SERVER" target/bench/release/vhost-user-nvrm > "$F/device-sha256.txt"
caraxes/tools/bench-hostwatch.sh start "$F"
caraxes/tools/bench-run.sh v2-restart > "$F/v2-up.txt" 2>&1
caraxes/tools/bench-run.sh stage v2 >> "$F/v2-up.txt" 2>&1
caraxes/tools/bench-run.sh stage v1 >> "$F/v1-up.txt" 2>&1
caraxes/tools/bench-run.sh startup v1 --out "$F"
caraxes/tools/bench-run.sh startup v2 --out "$F"
V1_RESTART=1 V2_RESTART=1 caraxes/tools/bench-run.sh series --targets "native v1 v2" --reps 5 --out "$F"
caraxes/tools/bench-hostwatch.sh stop "$F"
python3 caraxes/tools/bench-summary.py "$F/v1" "$F/v2" "$F/native" > "$M/summary-final.md"
echo "final series done $(date -Iseconds)"

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Restart vm2 with the current environment, then the trap round trip (bench-trap) and the
# short-kernel launch (nvprobe 4, 1000 cycles, sched=auto) into $1.
set -uo pipefail
R=/home/silas/git/Caraxes; M=$R/caraxes/runs/mapping-2026-09-24; D=$1; mkdir -p "$D"
source "$M/env.sh"
env | grep -E '^CARAXES_(TRACE|SHADOW|BAR|MAP)' > "$D/env-extra.txt"
"$R/caraxes/tools/bench-run.sh" v2-restart > "$D/up.txt" 2>&1 || { tail "$D/up.txt"; exit 1; }
"$R/tools/caraxes-vm.sh" ssh 'cat > ~/bench/bench-trap.c' < "$R/caraxes/tools/bench-trap.c"
"$R/tools/caraxes-vm.sh" ssh 'cd ~/bench && gcc -O2 -o bench-trap bench-trap.c && sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource0 20000' | tee "$D/trap.txt"
"$R/caraxes/tools/bench-run.sh" series --targets v2 --reps 1 --metrics "${METRICS:-short-auto}" --out "$D" 2>&1 | cut -c1-200 | tail -5

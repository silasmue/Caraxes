#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Morning matrix, benchmarks part, on one guest (env.sh mx_guest 610|615) in the matrix
# configuration. Modes:
#   series: caraxes/tools/bench-run.sh, stage first, then the series "short-auto rl copy init
#           churn" with 3 reps and ppo (60 s) with 2 reps, the VM and its device restarted before
#           every rep (V2_RESTART=1, and a v2-restart before the ppo series' first rep).
#   census: the census probe as caraxes/runs/opt-shadow-maps-2026-09-25/run.sh runs it (cuInit x3,
#           cuCtxCreate/cuCtxDestroy x5, the ops with 10 reps once), census-table.py with the
#           pool at 32 MiB, rpc-cost.py; one trace slice for all census-cu processes.
# Host watch around the part, down at the end, host Xid lines with the device pid.
# Usage: bench.sh series|census 610|615 [label]
set -uo pipefail
source "$(dirname "$(readlink -f "$0")")/env.sh"
MODE=${1:?usage: bench.sh series|census 610|615 [label]}; G=${2:?guest 610|615}
mx_guest "$G" || exit 2
C=$MX_HERE/$MODE-$G${3:+-$3}
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
mx_free || exit 2
mkdir -p "$C"; cd "$MX_ROOT" || exit 1
VMSH=$LEA_TOOLS/caraxes-vm.sh
REPS=${REPS:-10} CTX=${CTX:-5}
T0=$(date '+%F %T')
mx_setup "$C"
{ echo "# bench bin"; (cd target/bench/bin && sha256sum census-cu census.cubin kernels.ptx bench-cu nvprobe 2>/dev/null); } >> "$C/setup.txt"
caraxes/tools/bench-hostwatch.sh start "$C"
case $MODE in
series)
    # v2-restart brings the VM up (headroom retries inside), probes and makes the nodes.
    caraxes/tools/bench-run.sh v2-restart > "$C/v2-up.txt" 2>&1 || { echo "v2 did not come up"; tail -5 "$C/v2-up.txt"; }
    pgrep -af "vfio-user-nvgpu.*$MX_W/" | cut -c1-200 > "$C/device-pid.txt"
    grep -iE 'intr page|bar2 memfd|bar1 pool|msi|lazy|doorbell tokens|doorbell poll|host ABI|guest ABI' "$MX_W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
    caraxes/tools/bench-run.sh stage v2 >> "$C/v2-up.txt" 2>&1
    V2_RESTART=1 caraxes/tools/bench-run.sh series --targets v2 --reps 3 --metrics "short-auto rl copy init churn" --out "$C" 2>&1 | tail -3
    { echo "# $(date +%T) v2 restart before the ppo series"; caraxes/tools/bench-run.sh v2-restart; } >> "$C/series.txt" 2>&1
    V2_RESTART=1 PPO_SECONDS=60 caraxes/tools/bench-run.sh series --targets v2 --reps 2 --metrics ppo --out "$C" 2>&1 | tail -2
    ;;
census)
    census_guest='cd ~/bench && export CENSUS_PTX=$HOME/bench/kernels.ptx CENSUS_CUBIN=$HOME/bench/census.cubin
R0=$(for d in /sys/bus/pci/devices/*; do [ "$(cat $d/vendor)" = 0x10de ] && echo $d/resource0; done | head -1)
echo "# marker resource $R0"
E="sudo timeout 240 env LD_LIBRARY_PATH='"$(caraxes/tools/bench-run.sh libdir v2)"' CENSUS_PTX=$CENSUS_PTX CENSUS_CUBIN=$CENSUS_CUBIN CENSUS_MARK=$R0"
for i in 1 2 3; do $E ./census-cu init || exit 1; sleep 1; done
$E ./census-cu ctx '"$CTX"' || exit 1
$E ./census-cu ops '"$REPS"' || exit 1'
    if mx_up "$C"; then
        "$VMSH" probe "mx-census" >> "$C/up.txt" 2>&1; tail -2 "$C/up.txt" | cut -c1-200
        caraxes/tools/bench-run.sh v2-nodes >> "$C/up.txt" 2>&1
        grep -iE 'intr page|bar2 memfd|bar1 pool|msi|lazy|doorbell tokens|doorbell poll|host ABI|guest ABI' "$MX_W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
        caraxes/tools/bench-run.sh stage v2 >> "$C/up.txt" 2>&1
        L0=$(wc -l < "$MX_W/gpu-trace.log")
        "$VMSH" ssh "$census_guest" > "$C/census.txt" 2>&1
        echo "census rc=$? $(grep -c '^OP' "$C/census.txt") OP lines"
        tail -n +"$((L0 + 1))" "$MX_W/gpu-trace.log" > "$C/census-trace.txt"
        python3 caraxes/tools/census-table.py "$C/census-trace.txt" --names "$C/census.txt" --pool-mib 32 \
            --json "$C/census.json" > "$C/census-table.md"
        python3 caraxes/tools/rpc-cost.py "$C/census-trace.txt" > "$C/census-rpc-cost.md" 2>&1
        xz -T0 "$C/census-trace.txt"
    else
        echo "VM did not come up (up.txt, retries.txt)"
    fi
    ;;
*) echo "ERROR: mode series|census" >&2 ;;
esac
caraxes/tools/bench-hostwatch.sh stop "$C"
mx_finish "$C" "$T0"

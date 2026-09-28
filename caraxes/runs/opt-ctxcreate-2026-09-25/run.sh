#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# cuCtxCreate optimisation of 2026-09-25 (docs/caraxes/ctx-create.md). vm3 only, 610 guest, in
# the gate610 environment of caraxes/runs/gsp-rpc-2026-09-24/run.sh (1 GiB at reservation slot
# 5, host RM, host execution) plus the recommended configuration (matrix-2026-09-25/env.sh
# mx_guest 610: interrupt page, BAR1 pool 32 MiB, token pool 64, hybrid reactor, BAR2 memfd,
# MSI re-arm ioeventfd, CARAXES_ALLOC_LAZY=1), with the device binary BIN (a frozen build).
# Modes:
#   measure: the census (census-cu init x3, ctx 5, ops 10, device trace), census-table.py and
#            rpc-cost.py on its trace, then bench-run.sh init and churn with 3 reps; GATE=1 adds
#            the cuda gate at the end.
#   allocburn: 20 iterations of convburn plus allocburn.py (xid31-pool-2026-09-25/loop.sh's
#            loop with XP_ALLOCBURN=1 and the untrapped-GPPut rule on).
# Host watch around the part, vm3 down at the end, host Xid lines with the device pid.
# Usage: BIN=/mnt/vmstore/caraxes/vfio-user-nvgpu-ctx610-<id> run.sh measure|allocburn LABEL
set -uo pipefail
MODE=${1:?usage: run.sh measure|allocburn LABEL}; LABEL=${2:?label}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
source "$HERE/../matrix-2026-09-25/env.sh"
mx_guest 610 || exit 2
export CARAXES_SERVER=${BIN:?BIN: the frozen device binary}
unset CARAXES_SHADOW_UNTRAPPED_HOLD
C=$HERE/$LABEL
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
mx_free || exit 2
mkdir -p "$C"; cd "$MX_ROOT" || exit 1
VMSH=$LEA_TOOLS/caraxes-vm.sh
T0=$(date '+%F %T')
mx_setup "$C"
(cd target/bench/bin && sha256sum census-cu census.cubin kernels.ptx bench-cu) >> "$C/setup.txt" 2>/dev/null
caraxes/tools/bench-hostwatch.sh start "$C"

census_guest='cd ~/bench && export CENSUS_PTX=$HOME/bench/kernels.ptx CENSUS_CUBIN=$HOME/bench/census.cubin
R0=$(for d in /sys/bus/pci/devices/*; do [ "$(cat $d/vendor)" = 0x10de ] && echo $d/resource0; done | head -1)
echo "# marker resource $R0"
E="sudo timeout 240 env LD_LIBRARY_PATH=/opt/nvrm/lib CENSUS_PTX=$CENSUS_PTX CENSUS_CUBIN=$CENSUS_CUBIN CENSUS_MARK=$R0"
for i in 1 2 3; do $E ./census-cu init || exit 1; sleep 1; done
$E ./census-cu ctx 5 || exit 1
$E ./census-cu ops 10 || exit 1'

if mx_up "$C"; then
    "$VMSH" probe "ctx-$LABEL" >> "$C/up.txt" 2>&1; tail -2 "$C/up.txt" | cut -c1-200
    caraxes/tools/bench-run.sh v2-nodes >> "$C/up.txt" 2>&1
    grep -iE 'bar1 pool|lazy|untrapped|doorbell tokens|doorbell poll|host ABI|guest ABI|intr page|bar2 memfd|mirror' "$MX_W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
    "$VMSH" sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
    case $MODE in
    measure)
        caraxes/tools/bench-run.sh stage v2 >> "$C/up.txt" 2>&1
        L0=$(wc -l < "$MX_W/gpu-trace.log"); G0=$(wc -l < "$MX_W/gpu.log")
        "$VMSH" ssh "$census_guest" > "$C/census.txt" 2>&1
        echo "census rc=$? $(grep -c '^OP' "$C/census.txt") OP lines"
        tail -n +"$((L0 + 1))" "$MX_W/gpu-trace.log" > "$C/census-trace.txt"
        tail -n +"$((G0 + 1))" "$MX_W/gpu.log" | grep -aE 'ctx phase|pool empty|untrapped|mirror: ' | cut -c1-1600 > "$C/census-device-lines.txt"
        python3 caraxes/tools/census-table.py "$C/census-trace.txt" --names "$C/census.txt" --pool-mib 32 \
            --json "$C/census.json" > "$C/census-table.md" 2>&1
        python3 caraxes/tools/rpc-cost.py "$C/census-trace.txt" > "$C/census-rpc-cost.md" 2>&1
        [[ -x $HERE/phases.py ]] && python3 "$HERE/phases.py" "$C/census-trace.txt" --names "$C/census.txt" > "$C/phases.md" 2>&1
        BENCH_TIMEOUT=300 caraxes/tools/bench-run.sh series --targets v2 --reps 3 --metrics "init churn" --out "$C" 2>&1 | tail -2
        if [[ ${GATE:-0} == 1 ]]; then
            export CARAXES_GATE_RUNS=$C/gate
            tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -1 "$C/gate-cuda.txt" | cut -c1-300
        fi
        xz -T0 "$C/census-trace.txt" ;;
    allocburn)
        "$VMSH" ssh 'cat > ~/allocburn.py' < "$HERE/../xid31-pool-2026-09-25/allocburn.py"
        pid=$(cat "$MX_W/gpu.pid" 2>/dev/null)
        for i in $(seq 1 "${N:-20}"); do
            ts=$(date '+%F %T'); off=$(wc -l < "$MX_W/gpu.log")
            row=$("$VMSH" ssh "~/caraxes-tests/caraxes/run-tests.sh /tmp/xp-$i convburn" 2>&1 | grep CXROW | cut -f2-4 | tr '\t' ' ' | tr '\n' ';')
            row+=" allocburn: $("$VMSH" ssh 'timeout 300 ~/gpu/venv/bin/python ~/allocburn.py 2>&1 | tail -1; echo "rc=${PIPESTATUS[0]}"' 2>&1 | tail -2 | tr '\n' ' ' | cut -c1-240)"
            sleep 3
            xids=$(journalctl -k --since "$ts" --no-pager -g Xid 2>/dev/null | grep -v '^-- No entries')
            nx=$(grep -c 'Xid' <<< "$xids")
            win=$(tail -n +"$((off + 1))" "$MX_W/gpu.log")
            nref=$(grep -ac 'bar1 pool: at .* refused for' <<< "$win")
            nahead=$(grep -ac 'ran ahead of its doorbell' <<< "$win")
            echo "iter $i $(date +%T) [$row] host_xid=$nx refused_barriers=$nref ahead_with_plans_pending=$nahead" | tee -a "$C/loop.txt"
            { echo "iter $i"; grep -a ' pool empty: bar1 pool' <<< "$win" | tail -1 | cut -c1-1400; } >> "$C/pool-per-iter.txt"
            grep -aE 'refused for|ran ahead|HOST RC|ERROR' <<< "$win" | cut -c1-600 | sed "s/^/iter $i: /" >> "$C/device-events.txt"
            ((nx > 0)) && { echo "# iter $i (device pid $pid)"; echo "$xids"; } >> "$C/host-xid-iter.txt"
        done ;;
    *) echo "ERROR: unknown mode $MODE" >&2 ;;
    esac
    "$VMSH" ssh 'sudo dmesg | grep -iE "nvrm|xid" | tail -40' > "$C/guest-dmesg.txt" 2>&1
else
    echo "VM did not come up (up.txt, retries.txt)"
fi
caraxes/tools/bench-hostwatch.sh stop "$C"
mx_finish "$C" "$T0"
grep -a '^mirror: ' "$MX_W/gpu.log" | tail -3 | cut -c1-2400 > "$C/device-mirror-summary.txt"

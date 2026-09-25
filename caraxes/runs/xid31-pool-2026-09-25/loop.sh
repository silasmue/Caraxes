#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The Xid 31 of the morning matrix (docs/caraxes/bar1-pool.md §9): a loop of the cuda gate's
# convburn stage (run-tests.sh convburn, as tools/caraxes-gate.sh cuda runs it: a failed run is
# re-run once under the v1 tracer) on vm5-615 in the matrix configuration (matrix-2026-09-25/
# env.sh mx_guest 615: 1 GiB at slot 2, interrupt page, token pool 64, hybrid reactor, BAR1 pool
# 32 MiB, BAR2 memfd, MSI re-arm ioeventfd, ALLOC_LAZY) with this directory's device build.
# Per iteration: the stage's row, the host Xid lines since its start (with the device pid), the
# refused barriers and the doorbells that found their host channel ahead with plans pending
# (device log lines of that window), and the device's pool and untrapped-GPPut counters at the
# end of the iteration's CUDA context (the "pool empty" line).
# XP_GUEST=610: the 610 guest on vm3 instead (mx_guest 610, the 610 build).
# XP_TESTS: the run-tests.sh tests per iteration (default convburn). XP_ALLOCBURN=1: also
# allocburn.py per iteration (fresh mappings written while the GR channel is busy).
# Usage: loop.sh LABEL HOLD [N] [stop]
#   HOLD 0: CARAXES_SHADOW_UNTRAPPED_HOLD=0 (the rule off: the device as on main fbe4dfb, plus
#           the counters); 1: the fix. N iterations (default 20); "stop": end at the first Xid.
set -uo pipefail
HERE=$(dirname "$(readlink -f "$0")")
source "$HERE/../matrix-2026-09-25/env.sh"
L=${1:?usage: loop.sh LABEL HOLD [N] [stop]}; HOLD=${2:?}; N=${3:-20}; STOP=${4:-}
mx_guest "${XP_GUEST:-615}" || exit 2
if [[ ${XP_GUEST:-615} == 610 ]]; then
    export CARAXES_SERVER=${XP_BIN610:-/mnt/vmstore/caraxes/vfio-user-nvgpu-xidpool610-4d4306e}
else
    export CARAXES_SERVER=${XP_BIN:-/mnt/vmstore/caraxes/vfio-user-nvgpu-xidpool-4d4306e}
fi
export CARAXES_SHADOW_UNTRAPPED_HOLD=$HOLD
C=$HERE/$L
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
mx_free || exit 2
if pgrep -f bench-v1 >/dev/null; then echo "ERROR: a bench-v1 process runs (the tap is shared)" >&2; exit 2; fi
mkdir -p "$C"; cd "$MX_ROOT" || exit 1
VMSH=$LEA_TOOLS/caraxes-vm.sh
T0=$(date '+%F %T')
mx_setup "$C"
caraxes/tools/bench-hostwatch.sh start "$C"
if mx_up "$C"; then
    # The DMA table and the guest driver's load, as the gate parts do (caraxes-vm.sh probe).
    "$VMSH" probe "xp-$L" >> "$C/up.txt" 2>&1; tail -2 "$C/up.txt" | cut -c1-200
    # The device nodes (the 610 image has no nvidia-modprobe; bench-run.sh V2_NODES).
    caraxes/tools/bench-run.sh v2-nodes >> "$C/up.txt" 2>&1
    grep -iE 'bar1 pool|lazy|untrapped|doorbell tokens|doorbell poll|host ABI|guest ABI' "$MX_W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
    "$VMSH" sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
    [[ ${XP_ALLOCBURN:-0} == 1 ]] && "$VMSH" ssh 'cat > ~/allocburn.py' < "$HERE/allocburn.py"
    pid=$(cat "$MX_W/gpu.pid" 2>/dev/null)
    for i in $(seq 1 "$N"); do
        ts=$(date '+%F %T'); off=$(wc -l < "$MX_W/gpu.log")
        row=$("$VMSH" ssh "~/caraxes-tests/caraxes/run-tests.sh /tmp/xp-$i ${XP_TESTS:-convburn}" 2>&1 | grep CXROW | cut -f2-4 | tr '\t' ' ' | tr '\n' ';')
        if [[ ${XP_ALLOCBURN:-0} == 1 ]]; then
            row+=" allocburn: $("$VMSH" ssh 'timeout 300 ~/gpu/venv/bin/python ~/allocburn.py 2>&1 | tail -1; echo "rc=${PIPESTATUS[0]}"' 2>&1 | tail -2 | tr '\n' ' ' | cut -c1-240)"
        fi
        sleep 3
        xids=$(journalctl -k --since "$ts" --no-pager -g Xid 2>/dev/null | grep -v '^-- No entries')
        nx=$(grep -c 'Xid' <<< "$xids")
        win=$(tail -n +"$((off + 1))" "$MX_W/gpu.log")
        nref=$(grep -ac 'bar1 pool: at .* refused for' <<< "$win")
        nahead=$(grep -ac 'ran ahead of its doorbell' <<< "$win")
        echo "iter $i $(date +%T) [$row] host_xid=$nx refused_barriers=$nref ahead_with_plans_pending=$nahead" | tee -a "$C/loop.txt"
        { echo "iter $i"; grep -a ' pool empty: bar1 pool' <<< "$win" | tail -1 | cut -c1-1400; } >> "$C/pool-per-iter.txt"
        grep -aE 'refused for|ran ahead|HOST RC|ERROR' <<< "$win" | cut -c1-600 | sed "s/^/iter $i: /" >> "$C/device-events.txt"
        if ((nx > 0)); then
            { echo "# iter $i (device pid $pid)"; echo "$xids"; } >> "$C/host-xid-iter.txt"
            "$VMSH" ssh "tail -5 /tmp/xp-$i/convburn.log" > "$C/convburn-iter$i.txt" 2>&1
            [[ $STOP == stop ]] && break
        fi
    done
    "$VMSH" ssh 'sudo dmesg | grep -iE "nvrm|xid" | tail -40' > "$C/guest-dmesg.txt" 2>&1
else
    echo "VM did not come up (up.txt, retries.txt)"
fi
caraxes/tools/bench-hostwatch.sh stop "$C"
mx_finish "$C" "$T0"
# The mirror's exit line (written at the device's exit, so after the down).
grep -a '^mirror: .*untrapped GPPut' "$MX_W/gpu.log" | tail -1 | cut -c1-1600 > "$C/device-mirror-summary.txt"

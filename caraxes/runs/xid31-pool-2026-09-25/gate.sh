#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The cuda gate with the fix of docs/caraxes/bar1-pool.md §9, on the 615 guest (vm5-615) or the
# 610 guest (vm3, the gate610 environment of caraxes/runs/gsp-rpc-2026-09-24/run.sh), in the
# matrix configuration (matrix-2026-09-25/env.sh mx_guest: interrupt page, token pool 64, hybrid
# reactor, BAR1 pool 32 MiB, BAR2 memfd, MSI re-arm ioeventfd, ALLOC_LAZY), as the matrix's
# gate.sh runs it (up, sync-tests, nvml gate, cuda gate, down), with this directory's builds.
# Usage: gate.sh 610|615 [label]
set -uo pipefail
HERE=$(dirname "$(readlink -f "$0")")
source "$HERE/../matrix-2026-09-25/env.sh"
G=${1:?usage: gate.sh 610|615 [label]}
mx_guest "$G" || exit 2
case $G in
    615) export CARAXES_SERVER=${XP_BIN:-/mnt/vmstore/caraxes/vfio-user-nvgpu-xidpool-4d4306e} ;;
    610) export CARAXES_SERVER=${XP_BIN610:-/mnt/vmstore/caraxes/vfio-user-nvgpu-xidpool610-4d4306e} ;;
esac
unset CARAXES_SHADOW_UNTRAPPED_HOLD
C=$HERE/gate-$G${2:+-$2}
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
mx_free || exit 2
if pgrep -f bench-v1 >/dev/null; then echo "ERROR: a bench-v1 process runs (the tap is shared)" >&2; exit 2; fi
mkdir -p "$C"; cd "$MX_ROOT" || exit 1
export CARAXES_GATE_RUNS=$C
VMSH=$LEA_TOOLS/caraxes-vm.sh
T0=$(date '+%F %T')
mx_setup "$C"
caraxes/tools/bench-hostwatch.sh start "$C"
if mx_up "$C"; then
    "$VMSH" probe "xp-gate-$G" >> "$C/up.txt" 2>&1; tail -2 "$C/up.txt" | cut -c1-200
    grep -iE 'bar1 pool|lazy|untrapped|doorbell tokens|doorbell poll|host ABI|guest ABI' "$MX_W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
    "$VMSH" sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
    tools/caraxes-gate.sh nvml > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"; tail -1 "$C/gate-nvml.txt" | cut -c1-200
    tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -1 "$C/gate-cuda.txt" | cut -c1-200
    "$VMSH" ssh 'sudo dmesg | grep -iE "nvrm|xid" | tail -40' > "$C/guest-dmesg.txt" 2>&1
else
    echo "VM did not come up (up.txt, retries.txt)"
fi
caraxes/tools/bench-hostwatch.sh stop "$C"
grep -aE ' pool empty: bar1 pool' "$MX_W/gpu.log" | cut -c1-1400 > "$C/pool-per-context.txt"
grep -aE 'refused for|ran ahead|HOST RC' "$MX_W/gpu.log" | cut -c1-600 > "$C/device-events.txt"
mx_finish "$C" "$T0"
grep -a '^mirror: .*untrapped GPPut' "$MX_W/gpu.log" | tail -1 | cut -c1-1600 > "$C/device-mirror-summary.txt"

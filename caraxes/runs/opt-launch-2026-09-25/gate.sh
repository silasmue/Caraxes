#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The cuda gate on vm5 (env.sh) with the after binary and B's switches (ab.sh): host watch, up,
# probe, sync-tests, cuda gate, the guest's log, down, host Xid lines with the device pid, as the
# morning matrix's gate.sh runs it. Usage: gate.sh LABEL
set -uo pipefail
source "$(dirname "$(readlink -f "$0")")/env.sh"
C=$OL_HERE/${1:?usage: gate.sh LABEL}
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
ol_free || exit 2
# GATE_CFG=A: the before binary without the switches (the reference); default B.
if [[ ${GATE_CFG:-B} == A ]]; then
    export CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-launch-before-7095b3c
else
    export CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-launch-after-25c164b
    export CARAXES_LAUNCH_FAST=1 CARAXES_DOORBELL_POLL_ADAPT_US=2000 CARAXES_INTR_GATE_MS=50
fi
mkdir -p "$C"
cd "$OL_ROOT" || exit 1
export CARAXES_GATE_RUNS=$C
VMSH=$LEA_TOOLS/caraxes-vm.sh
T0=$(date '+%F %T')
ol_setup "$C"
caraxes/tools/bench-hostwatch.sh start "$C"
if caraxes/tools/bench-run.sh v2-restart > "$C/up.txt" 2>&1; then
    pgrep -af "vfio-user-nvgpu.*$OL_W/" | cut -c1-200 > "$C/device-pid.txt"
    "$VMSH" sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
    tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -1 "$C/gate-cuda.txt" | cut -c1-200
    "$VMSH" ssh 'sudo dmesg | grep -iE "nvrm|xid" | tail -40' > "$C/guest-dmesg.txt" 2>&1
else
    echo "VM did not come up (up.txt)"
fi
caraxes/tools/bench-hostwatch.sh stop "$C"
ol_finish "$C" "$T0"

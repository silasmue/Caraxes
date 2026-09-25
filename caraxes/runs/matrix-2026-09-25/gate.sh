#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Morning matrix, gates part: the nvml and cuda gates on one guest (env.sh mx_guest 610|615|
# costume) in the matrix configuration, as caraxes/runs/gsp-round2-c-2026-09-24/run.sh runs them:
# host watch, up (reservation retried), probe, sync-tests, nvml gate, cuda gate, the device's
# trace issues, down, host Xid lines with the device pid. The costume adds the identity as the
# guest sees it (costume-2026-09-24/run.sh). Usage: gate.sh 610|615|costume [label]
set -uo pipefail
source "$(dirname "$(readlink -f "$0")")/env.sh"
G=${1:?usage: gate.sh 610|615|costume [label]}
mx_guest "$G" || exit 2
C=$MX_HERE/gates-$G${2:+-$2}
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
mx_free || exit 2
mkdir -p "$C"; cd "$MX_ROOT" || exit 1
export CARAXES_GATE_RUNS=$C
VMSH=$LEA_TOOLS/caraxes-vm.sh
T0=$(date '+%F %T')
mx_setup "$C"
caraxes/tools/bench-hostwatch.sh start "$C"
if mx_up "$C"; then
    "$VMSH" probe "mx-$G" >> "$C/up.txt" 2>&1; tail -2 "$C/up.txt" | cut -c1-200
    grep -iE 'intr page|bar2 memfd|bar1 pool|msi|lazy|doorbell tokens|doorbell poll|host ABI|guest ABI|guest name' "$MX_W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
    "$VMSH" sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
    tools/caraxes-gate.sh nvml > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"; tail -1 "$C/gate-nvml.txt" | cut -c1-200
    tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -1 "$C/gate-cuda.txt" | cut -c1-200
    if [[ $G == costume ]]; then
        L='sudo LD_LIBRARY_PATH=/opt/nvrm/lib'; SMI="$L /opt/nvrm/bin/nvidia-smi"
        "$VMSH" ssh 'lspci -nn -d 10de:' > "$C/guest-lspci.txt" 2>&1
        "$VMSH" ssh "$SMI --query-gpu=name,compute_cap,pci.device_id,pci.sub_device_id,vbios_version,memory.total --format=csv" > "$C/guest-nvidia-smi.txt" 2>&1
    fi
    "$VMSH" ssh 'sudo dmesg | grep -iE "nvrm|xid" | tail -40' > "$C/guest-dmesg.txt" 2>&1
else
    echo "VM did not come up (up.txt, retries.txt)"
fi
caraxes/tools/bench-hostwatch.sh stop "$C"
T=$MX_W/gpu-trace.log
grep -aE 'unanswered|NOT_SUPPORTED|status 0x56|refused' "$T" 2>/dev/null | sed -E 's/^.*(ctrl 0x[0-9a-f]+|alloc: |rx seq [0-9]+ )/\1/' | sort | uniq -c | sort -rn | head -25 | cut -c1-300 > "$C/device-trace-issues.txt"
mx_finish "$C" "$T0"

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Night 2026-09-28: the lean compute VM of the 2026-09-25 matrix (vm2-615, slot 0, env.sh wl615)
# with a chosen device binary, extra CARAXES_* settings and a guest script (stdin of the ssh).
# Usage: lean-run.sh OUTDIR BINARY GUEST_SCRIPT [VAR=VALUE ...]
set -uo pipefail
source /home/silas/git/Caraxes/caraxes/runs/matrix-2026-09-25/env.sh
mx_guest wl615
FB=${LEAN_FB_MIB:-2048}; export CARAXES_FB_MIB=$FB CARAXES_GATE_FB_MIB=$FB
V2_NODES='sudo modprobe nvidia-uvm
    [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
    [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
    u=$(grep nvidia-uvm /proc/devices | cut -d" " -f1); sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools
    sudo mknod -m 666 /dev/nvidia-uvm c $u 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c $u 1'
C=$1 bin=$2 script=$3; shift 3
export CARAXES_SERVER=$bin CARAXES_LAUNCH_TIMING=1
for kv in "$@"; do export "${kv?}"; done
[[ -e $C ]] && { echo "ERROR: $C exists" >&2; exit 2; }
mkdir -p "$C"
mx_free || exit 2
T0=$(date '+%F %T')
mx_setup "$C"
if mx_up "$C" && "$LEA_TOOLS/caraxes-vm.sh" probe "lean-$(basename "$C")" | tail -n 2 >> "$C/up.txt" &&
    "$LEA_TOOLS/caraxes-vm.sh" ssh "$V2_NODES" >> "$C/up.txt" 2>&1; then
    "$LEA_TOOLS/caraxes-vm.sh" ssh 'bash -s' < "$script" > "$C/guest.txt" 2>&1
else
    echo "up failed" | tee -a "$C/guest.txt"
fi
mx_finish "$C" "$T0"
grep -E "launch timing" "$MX_W/gpu.log" > "$C/launch-timing.txt"

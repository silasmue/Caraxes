#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Night 2026-09-28: short-kernel latency, binary against configuration. The lean compute VM of
# the 2026-09-25 matrix (vm2-615, 2 GiB, slot 0, mx_windows set; env.sh there) with a chosen
# device binary and extra CARAXES_* settings; 5 x nvprobe 4 (1000 x 1000-cycle kernels, auto),
# the device's launch timing histograms (CARAXES_LAUNCH_TIMING=1) and the host Xid lines.
# Usage: lat-ab.sh ARM BINARY [VAR=VALUE ...]
set -uo pipefail
H=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
source /home/silas/git/Caraxes/caraxes/runs/matrix-2026-09-25/env.sh
mx_guest wl615
V2_NODES='sudo modprobe nvidia-uvm
    [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
    [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
    u=$(grep nvidia-uvm /proc/devices | cut -d" " -f1); sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools
    sudo mknod -m 666 /dev/nvidia-uvm c $u 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c $u 1'
arm=$1 bin=$2; shift 2
export CARAXES_SERVER=$bin CARAXES_LAUNCH_TIMING=1
for kv in "$@"; do export "${kv?}"; done
C=$H/arm-$arm
[[ -e $C ]] && { echo "ERROR: $C exists" >&2; exit 2; }
mkdir -p "$C"
mx_free || exit 2
T0=$(date '+%F %T')
mx_setup "$C"
if mx_up "$C" && "$LEA_TOOLS/caraxes-vm.sh" probe "lat-$arm" | tail -n 2 >> "$C/up.txt" &&
    "$LEA_TOOLS/caraxes-vm.sh" ssh "$V2_NODES" >> "$C/up.txt" 2>&1; then
    "$LEA_TOOLS/caraxes-vm.sh" ssh 'cd ~/bench && for i in 1 2 3 4 5; do
        NVPROBE_PTX=$HOME/bench/kernels.ptx NVPROBE_ITERS=1000 NVPROBE_SCHED=auto NVPROBE_CYCLES=1000 ./nvprobe 4 2>&1 | grep -E "MSPERLAUNCH|FAIL|error"; done' > "$C/short.txt" 2>&1
else
    echo "up failed" | tee -a "$C/short.txt"
fi
mx_finish "$C" "$T0"
grep -E "launch timing" "$MX_W/gpu.log" > "$C/launch-timing.txt"
echo "arm $arm: $(grep -o 'MSPERLAUNCH=[0-9.]*' "$C/short.txt" | cut -d= -f2 | tr '\n' ' ')"

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The short kernel on several device binaries in one sitting, the lean VM
# of v2-compute.sh (vm2-615, 615 guest, 4 GiB, slot 0, trace and profile off), one boot per
# binary: nvprobe 4 with 1000 x 1000 cycles (auto) five times, then bench-run.sh short-auto,
# short-blocking and launch-auto with 3 reps. Usage (inside vmlock.sh vm2-615):
#   ab-short.sh OUTDIR LABEL=BINARY...
set -uo pipefail
O=$1; shift
MAIN=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd -P)
source "$MAIN/caraxes/runs/matrix-2026-09-25/env.sh"
mx_guest wl615
export CARAXES_FB_MIB=4096 CARAXES_GATE_FB_MIB=4096 CARAXES_TRACE=0
unset CARAXES_PROFILE CARAXES_LAUNCH_TIMING
V2_NODES='sudo modprobe nvidia-uvm
    [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
    [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
    u=$(grep nvidia-uvm /proc/devices | cut -d" " -f1); sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools
    sudo mknod -m 666 /dev/nvidia-uvm c $u 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c $u 1'
VMSH=$LEA_TOOLS/caraxes-vm.sh
mkdir -p "$O"
for lb in "$@"; do
    L=${lb%%=*} BIN=${lb#*=}
    C=$O/$L
    [[ -e $C ]] && { echo "ERROR: $C exists" >&2; continue; }
    mkdir -p "$C"
    export CARAXES_SERVER=$BIN
    mx_free || exit 2
    T0=$(date '+%F %T')
    mx_setup "$C"
    if mx_up "$C" && : > "$MX_W/gpu-trace.log" && "$VMSH" probe "ab-$L" | tail -n 2 >> "$C/up.txt" &&
        "$VMSH" ssh "$V2_NODES" >> "$C/up.txt" 2>&1; then
        "$VMSH" ssh 'cd ~/bench; for i in 1 2 3 4 5; do NVPROBE_PTX=$HOME/bench/kernels.ptx NVPROBE_ITERS=1000 NVPROBE_SCHED=auto NVPROBE_CYCLES=1000 ./nvprobe 4 2>&1 | grep MSPERLAUNCH; done' > "$C/short.txt" 2>&1
        "$MAIN/caraxes/tools/bench-run.sh" series --targets v2 --reps 3 --out "$C/p13" \
            --metrics "short-auto short-blocking launch-auto" > "$C/p13.txt" 2>&1
        echo "$L $(sha256sum < "$BIN" | cut -c1-16): $(grep -o '0\.[0-9]*' "$C/short.txt" | tr '\n' ' ')| p13 $(grep -o 'short-auto .*MSPERLAUNCH=[0-9.]*' "$C/p13.txt" | grep -o '[0-9.]*$' | tr '\n' ' ')" | tee -a "$O/summary.txt"
    else
        echo "$L: up failed" | tee -a "$O/summary.txt"
    fi
    mx_finish "$C" "$T0"
done

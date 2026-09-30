#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# v2 compute on the lean VM (vm2-615, 615 guest, 8 vCPUs, 4 GiB RAM, slot 0): the workload
# series of caraxes/runs/night-2026-09-28 (short kernel, llama.cpp, transformer, ResNet-18,
# nvenc-probe; the full guest script with the nvenc-probe blob is in the VM store) and the P13
# metrics of caraxes/tools/bench-run.sh (v2 only, 3 reps). The launcher calls of
# night-2026-09-28/lean-run.sh; device trace, CARAXES_PROFILE and CARAXES_LAUNCH_TIMING off.
# Usage (inside vmlock.sh vm2-615): v2-compute.sh OUTDIR BINARY [FB_MIB]
set -uo pipefail
C=$1 BIN=$2 FB=${3:-4096}
REPO=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd -P)
GS=${GUEST_SERIES:-/mnt/vmstore/caraxes/night-2026-09-28/guest-scripts/guest-series.sh}
source "$REPO/caraxes/runs/matrix-2026-09-25/env.sh"
mx_guest wl615
export CARAXES_FB_MIB=$FB CARAXES_GATE_FB_MIB=$FB CARAXES_SERVER=$BIN CARAXES_TRACE=0
unset CARAXES_PROFILE CARAXES_LAUNCH_TIMING
V2_NODES='sudo modprobe nvidia-uvm
    [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
    [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
    u=$(grep nvidia-uvm /proc/devices | cut -d" " -f1); sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools
    sudo mknod -m 666 /dev/nvidia-uvm c $u 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c $u 1'
[[ -e $C ]] && { echo "ERROR: $C exists" >&2; exit 2; }
[[ -r $GS ]] || { echo "ERROR: no guest series script $GS" >&2; exit 2; }
mkdir -p "$C"
mx_free || exit 2
T0=$(date '+%F %T')
mx_setup "$C"
"$REPO/caraxes/tools/bench-hostwatch.sh" start "$C"
VMSH=$LEA_TOOLS/caraxes-vm.sh
if mx_up "$C" && : > "$MX_W/gpu-trace.log" && "$VMSH" probe "bench-$(basename "$C")" | tail -n 2 >> "$C/up.txt" &&
    "$VMSH" ssh "$V2_NODES" >> "$C/up.txt" 2>&1; then
    "$VMSH" sync-tests > "$C/sync-tests.txt" 2>&1
    "$REPO/caraxes/tools/bench-run.sh" stage v2 > "$C/stage.txt" 2>&1
    "$REPO/caraxes/tools/bench-run.sh" stage-llama v2 >> "$C/stage.txt" 2>&1
    echo "series start $(date +%T)"
    "$VMSH" ssh 'bash -s' < "$GS" > "$C/guest-series.txt" 2>&1
    echo "series done $(date +%T); P13 metrics"
    "$REPO/caraxes/tools/bench-run.sh" series --targets v2 --reps 3 --out "$C/p13" \
        --metrics "short-auto short-blocking launch-auto launch-blocking copy init churn ioctl ctrl rl mm" > "$C/p13.txt" 2>&1
    echo "P13 done $(date +%T)"
    "$VMSH" ssh 'sudo dmesg | grep -iE "nvrm|xid" | tail -40' > "$C/guest-dmesg.txt" 2>&1
else
    echo "up failed" | tee -a "$C/guest-series.txt"
fi
"$REPO/caraxes/tools/bench-hostwatch.sh" stop "$C"
mx_finish "$C" "$T0"
cp "$MX_W/gpu.log" "$C/gpu.log"

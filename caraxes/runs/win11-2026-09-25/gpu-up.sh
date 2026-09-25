#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# The synthetic GPU for the Windows guest, in pure emulation (no host RM, no reservation, no
# host execution): the same start as tools/caraxes-vm.sh `up` builds, on $W/gpu.sock, so that
# QEMU (`-device vfio-user-pci,socket=`) or Cloud Hypervisor (`--user-device socket=`) attach
# it. The trace shows which RM path the Windows driver takes (nova-guest.md section 7, P23).
# Usage: gpu-up.sh        (WIN_GPU_SERVER: the device binary, default the frozen v615 build;
#                          WIN_GPU_FB_MIB default 1024; CARAXES_GUEST_ABI optional)
set -euo pipefail
W=/mnt/vmstore/caraxes/win11
LEA_ROOT=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd -P)
SERVER=${WIN_GPU_SERVER:-/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-nvenc-1c4c848}
REF=$LEA_ROOT/caraxes/reference/rtx2070
# WIN_GPU_CONFIG: a config-space dump to present instead of the capture (config-space-3d.txt
# next to this script: the same card as a "3D controller", class 03 02, so Windows' basic
# display driver leaves it alone).
CONFIG=${WIN_GPU_CONFIG:-$REF/config-space.txt}
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
[[ -x $SERVER ]] || die "no device binary at $SERVER"
[[ -s $CONFIG && -s $REF/rom.bin ]] || die "no config dump $CONFIG or ROM in $REF"
[[ -s $W/gpu.pid ]] && kill -0 "$(cat "$W/gpu.pid")" 2>/dev/null && die "device already running (pid $(cat "$W/gpu.pid"))"
rm -f "$W/gpu.sock"
args=(--boot-model --display --fb-mib "${WIN_GPU_FB_MIB:-1024}")
[[ -n ${CARAXES_GUEST_ABI:-} ]] && args+=(--guest-abi "$CARAXES_GUEST_ABI")
LEA_ROOT=$LEA_ROOT "$SERVER" --socket "$W/gpu.sock" --trace "$W/gpu-trace.log" \
    --config "$CONFIG" --rom "$REF/rom.bin" "${args[@]}" > "$W/gpu.log" 2>&1 &
echo $! > "$W/gpu.pid"
for _ in $(seq 50); do [[ -S $W/gpu.sock ]] && break; sleep 0.1; done
[[ -S $W/gpu.sock ]] || { cat "$W/gpu.log"; die "the device did not create $W/gpu.sock"; }
echo "device up: pid $(cat "$W/gpu.pid"), pure emulation, config $CONFIG, FB ${WIN_GPU_FB_MIB:-1024} MiB, socket $W/gpu.sock, log $W/gpu.log, trace $W/gpu-trace.log"

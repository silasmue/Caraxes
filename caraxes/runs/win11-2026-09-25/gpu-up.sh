#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# The synthetic GPU for the Windows guest, in pure emulation (no host RM, no reservation, no
# host execution): the same start as tools/caraxes-vm.sh `up` builds, on $W/gpu.sock, so that
# QEMU (`-device vfio-user-pci,socket=`) or Cloud Hypervisor (`--user-device socket=`) attach
# it. The trace shows which RM path the Windows driver takes (nova-guest.md section 7, P23).
# Usage: gpu-up.sh        (WIN_GPU_SERVER: the device binary, default the frozen v615 build;
#                          WIN_GPU_FB_MIB default 1024; WIN_GPU_ROM, WIN_GPU_BOARD,
#                          WIN_GPU_REGS, WIN_GPU_CONFIG, CARAXES_GUEST_ABI optional)
set -euo pipefail
W=${WIN_STATE_DIR:-/mnt/vmstore/caraxes/win11}
LEA_ROOT=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd -P)
SERVER=${WIN_GPU_SERVER:-/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-ptimer-365dff26}
REF=$LEA_ROOT/caraxes/reference/rtx2070
# WIN_GPU_BOARD: see below. WIN_GPU_CONFIG: a config-space dump to present instead of the capture (config-space-3d.txt
# next to this script: the same card as a "3D controller", class 03 02, so Windows' basic
# display driver leaves it alone).
CONFIG=${WIN_GPU_CONFIG:-$REF/config-space.txt}
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
[[ -x $SERVER ]] || die "no device binary at $SERVER"
[[ -s $CONFIG ]] || die "no config dump $CONFIG"
[[ -s $W/gpu.pid ]] && kill -0 "$(cat "$W/gpu.pid")" 2>/dev/null && die "device already running (pid $(cat "$W/gpu.pid"))"
rm -f "$W/gpu.sock"
args=(--boot-model --fb-mib "${WIN_GPU_FB_MIB:-1024}")
# WIN_GPU_DISPLAY=0: headless (the display fuse reads 1, no display engine), as in E0-E5.
[[ ${WIN_GPU_DISPLAY:-1} == 1 ]] && args+=(--display)
# WIN_GPU_EXTRA: further device flags, word-split (gpu-up-host.sh passes --host-rm).
[[ -n ${WIN_GPU_EXTRA:-} ]] && read -ra extra <<< "$WIN_GPU_EXTRA" && args+=("${extra[@]}")
# WIN_GPU_BOARD: a board-data file for a costume (crates/vfio-user-nvgpu/boards/tesla-t4-on-tu106.toml:
# the card as a Tesla T4 for the data-centre driver's TCC path).
[[ -n ${WIN_GPU_BOARD:-} ]] && args+=(--board-data "$WIN_GPU_BOARD")
# WIN_GPU_REGS: a captured-register file for the table (bar0-registers-t4-costume.txt next to this
# script for the Tesla T4 costume, whose chip identity refuses the RTX 2070's own capture).
[[ -n ${WIN_GPU_REGS:-} ]] && args+=(--bar0-registers "$WIN_GPU_REGS")
[[ -n ${CARAXES_GUEST_ABI:-} ]] && args+=(--guest-abi "$CARAXES_GUEST_ABI")
# WIN_GPU_ROM: the ROM image to serve instead of the capture (make-rom-legacy.py writes
# $W/rom-legacy-only.bin, the ROM without the EFI GOP image, for the expansion-ROM-BAR experiment:
# run-qemu-gpu.sh with WIN_GPU_ROMBAR=1 then keeps the BAR on without the firmware hang).
ROM=${WIN_GPU_ROM:-$REF/rom.bin}
[[ -s $ROM ]] || die "no ROM image $ROM"
# WIN_GPU_VBIOS: the image served through the BAR0 PROM window (--vbios), as the Linux launcher
# serves synth-vbios.bin there; without it the PROM window shows the ROM image only (127 KiB,
# no NVIDIA extension image), which the Windows driver scans to the end and then gives up
# (docs/research/windows-driver-code43.md).
[[ -n ${WIN_GPU_VBIOS:-} ]] && { [[ -s $WIN_GPU_VBIOS ]] || die "no VBIOS image $WIN_GPU_VBIOS"; args+=(--vbios "$WIN_GPU_VBIOS"); }
LEA_ROOT=$LEA_ROOT "$SERVER" --socket "$W/gpu.sock" --trace "$W/gpu-trace.log" \
    --config "$CONFIG" --rom "$ROM" "${args[@]}" > "$W/gpu.log" 2>&1 &
echo $! > "$W/gpu.pid"
for _ in $(seq 50); do [[ -S $W/gpu.sock ]] && break; sleep 0.1; done
[[ -S $W/gpu.sock ]] || { cat "$W/gpu.log"; die "the device did not create $W/gpu.sock"; }
echo "device up: pid $(cat "$W/gpu.pid"), ${WIN_GPU_EXTRA:-pure emulation}, config $CONFIG, rom $ROM, vbios ${WIN_GPU_VBIOS:-(rom)}, FB ${WIN_GPU_FB_MIB:-1024} MiB, socket $W/gpu.sock, log $W/gpu.log, trace $W/gpu-trace.log"

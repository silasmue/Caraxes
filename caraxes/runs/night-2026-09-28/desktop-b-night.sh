#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Linux B (desktop-b, the 615 guest) for the night runs: the device settings of 2026-09-28/29.
# Usage: desktop-b-night.sh up|down|wait|ssh ... (caraxes-vm-night.sh); NIGHT_SERVER picks the binary.
set -euo pipefail
export CARAXES_STORE=/mnt/vmstore/caraxes/alpha-2026-09-27 VM=desktop-b
export LEA_CH=/mnt/vmstore/caraxes/ch-readonly-mmap/target/release/cloud-hypervisor
export CARAXES_SERVER=${NIGHT_SERVER:-/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-night-945431f9}
export CARAXES_DRIVER=615 CARAXES_GUEST_ABI=615
export CARAXES_IP=192.168.106.26 CARAXES_TAP=caraxes6 CARAXES_MAC=02:00:00:00:00:26
export CARAXES_MEM=8G CARAXES_CPUS=8 CARAXES_FB_MIB=${NIGHT_FB_MIB:-1024} CARAXES_RESERVE_SLOT=${NIGHT_SLOT:-4}
export CARAXES_GAMES_DISK=/mnt/vmstore/leandro/vm/games.qcow2
export CARAXES_BOOT_MODEL=1 CARAXES_DISPLAY=1 CARAXES_DISPLAY_PAGE=1
export CARAXES_DISPLAY_PAGE_READONLY=1
export CARAXES_HOST_RM=1 CARAXES_RESERVE_VRAM=1 CARAXES_HOST_EXEC=1 CARAXES_DMA=register
export CARAXES_COMPRESSION=slice
export CARAXES_BAR1_POOL_MIB=32
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 CARAXES_ALLOC_LAZY=1
export CARAXES_HOST_BAR1_FLOOR_MIB=64
# The launcher copy of the night of 2026-09-28 next to this wrapper (the repository's own is
# tools/caraxes-vm.sh).
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
exec "$HERE/caraxes-vm-night.sh" "$@"

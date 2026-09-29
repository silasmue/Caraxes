#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Linux B's dmem variant (docs/research/dmem-cap-2026-09-29.md): the disks of desktop-b copied
# to desktop-b-dmem on 2026-09-29 (rootfs.qcow2 on vm4-615/rootfs.qcow2, games.qcow2 on the
# games base, seed.img), with Linux B's launcher, device settings, network and slot
# (night-2026-09-28/desktop-b-night.sh), so it never runs beside Linux B. It boots the Ubuntu
# HWE kernel copied to $W/boot when that is there (DMEM_KERNEL=hwe, the default), otherwise, or
# with DMEM_KERNEL=stock, the stock 6.8.0-136 of the other guests.
# 2026-09-29 copy for the 6 GiB SotTR run: NIGHT_VRAM_FLOOR_MIB sets the host VRAM floor.
# Usage: desktop-b-dmem-6g.sh up|down|status|wait|ssh [command...]
set -euo pipefail
W=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b-dmem
KVER=${DMEM_KVER:-7.0.0-34-generic}
if [[ ${DMEM_KERNEL:-hwe} == hwe && -f $W/boot/vmlinuz-$KVER && -f $W/boot/initrd.img-$KVER ]]; then
    export CARAXES_KERNEL=$W/boot/vmlinuz-$KVER CARAXES_INITRD=$W/boot/initrd.img-$KVER
    [[ ${1:-} == up ]] && echo "desktop-b-dmem: kernel $KVER from $W/boot"
else
    [[ ${1:-} == up ]] && echo "desktop-b-dmem: stock guest kernel"
fi
export CARAXES_STORE=/mnt/vmstore/caraxes/alpha-2026-09-27 VM=desktop-b-dmem
export LEA_CH=/mnt/vmstore/caraxes/ch-readonly-mmap/target/release/cloud-hypervisor
export CARAXES_SERVER=${NIGHT_SERVER:-/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-night-945431f9}
export CARAXES_DRIVER=615 CARAXES_GUEST_ABI=615
export CARAXES_IP=192.168.106.26 CARAXES_TAP=caraxes6 CARAXES_MAC=02:00:00:00:00:26
export CARAXES_MEM=8G CARAXES_CPUS=8 CARAXES_FB_MIB=${NIGHT_FB_MIB:-1024} CARAXES_RESERVE_SLOT=${NIGHT_SLOT:-4}
export CARAXES_GAMES_DISK=/mnt/vmstore/leandro/vm/games.qcow2
export CARAXES_BOOT_MODEL=1 CARAXES_DISPLAY=1 CARAXES_DISPLAY_PAGE=1
export CARAXES_DISPLAY_PAGE_READONLY=1
export CARAXES_HOST_RM=1 CARAXES_RESERVE_VRAM=1 CARAXES_HOST_EXEC=1 CARAXES_DMA=register CARAXES_GUEST_DMA_GPA=1
export CARAXES_MIRROR_CHANNELS=1 CARAXES_MIRROR_GR_SHADOW=1 CARAXES_CE_SHADOW=1 CARAXES_MIRROR_BARE_CE=1
export CARAXES_COMPRESSION=slice
export CARAXES_INTR_PAGE=1 CARAXES_BAR1_PASSTHROUGH=pool CARAXES_BAR1_POOL_MIB=32
export CARAXES_BAR1_WINDOWS=2 CARAXES_BAR1_WINDOW_MIB=16
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 CARAXES_ALLOC_LAZY=1
export CARAXES_HOST_VRAM_FLOOR_MIB=${NIGHT_VRAM_FLOOR_MIB:-1024} CARAXES_HOST_BAR1_FLOOR_MIB=64
export CARAXES_TRACE_FLUSH=0 CARAXES_LAUNCH_FAST=${NIGHT_FAST:-2}
exec /mnt/vmstore/caraxes/night-2026-09-28/caraxes-vm-night.sh "$@"

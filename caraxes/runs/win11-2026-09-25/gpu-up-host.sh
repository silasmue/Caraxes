#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# The Windows device with the host RM behind it (night-2026-09-25/WINDOWS-LOOP.md, "Host RM"): the
# same start as gpu-up.sh (WIN_GPU_SERVER, WIN_GPU_ROM, WIN_GPU_VBIOS, the 1 GiB profile) plus the
# host-RM flags of the Linux stage (caraxes/runs/sottr-2026-09-24/env.sh, tools/caraxes-vm.sh), no BAR1
# pool by default, BAR1 windows 2 x 16 MiB, floors (VRAM 768 MiB, BAR1 96 MiB).
# WIN_BAR1_POOL_MIB opts into the experimental mmap pool for host execution.
#   WIN_HOST_MODE=rm (default): --host-rm only. The host RM answers the forwardable controls as an
#     unprivileged client (the FB/BUS/GR info the KMD asks); nothing executes on the card, no VRAM is
#     reserved, no guest RAM is registered. Needs no privileges.
#   WIN_HOST_MODE=exec: --host-rm --reserve-vram --reserve-slot ${WIN_RESERVE_SLOT:-0} --host-exec
#     --register-guest-ram, as the Linux stage, plus CARAXES_GUEST_DMA_GPA=1: Windows has no
#     caraxes_dma_range and hands the device GPAs, which the device aliases to the registered IOVAs.
#     Needs WIN_GPU_SERVER = a copy of a build >= 18fae970 with cap_sys_admin+p (the user runs setcap).
# Refuses to start while a vm4-615 device (the Linux stage) runs: BAR1 and VRAM budget.
set -euo pipefail
W=${WIN_STATE_DIR:-/mnt/vmstore/caraxes/win11}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
if [[ -n ${WIN_BAR1_POOL_MIB:-} ]]; then
    [[ ${WIN_HOST_MODE:-rm} == exec ]] || die "WIN_BAR1_POOL_MIB requires host execution"
    [[ $WIN_BAR1_POOL_MIB =~ ^[1-9][0-9]*$ ]] || die "WIN_BAR1_POOL_MIB must be a positive integer"
fi
# Host sharing (2026-09-26): the Linux queue holds /mnt/vmstore/caraxes/linux-busy while it uses the
# GPU; a host-RM Windows run holds win-busy from here until gpu-down.sh removes it.
V=/mnt/vmstore/caraxes
[[ -e $V/linux-busy ]] && die "$V/linux-busy exists: the Linux queue has the host GPU"
if pgrep -f '/vm4-615/vfio-user-nvgpu' >/dev/null; then die "a vm4-615 device runs (Linux stage): the host is not free"; fi
touch "$V/win-busy"
trap 'rm -f "$V/win-busy"' ERR
case ${WIN_HOST_MODE:-rm} in
    rm) export WIN_GPU_EXTRA="--host-rm" ;;
    exec)
        # Host execution as the Linux stage (tools/caraxes-vm.sh), for a guest that speaks GPA:
        # CARAXES_GUEST_DMA_GPA=1 (device 18fae970 on) aliases every registered GPA range.
        # WIN_GPU_SERVER must be a copy with cap_sys_admin+p (setcap by the user).
        getcap "${WIN_GPU_SERVER:?WIN_GPU_SERVER: the cap_sys_admin copy}" 2>/dev/null | grep -q cap_sys_admin ||
            die "$WIN_GPU_SERVER lacks cap_sys_admin+p (the user sets it; this script does not)"
        export WIN_GPU_EXTRA="--host-rm --reserve-vram --reserve-slot ${WIN_RESERVE_SLOT:-0} --host-exec --register-guest-ram --dma-ranges $W/dma-ranges"
        export CARAXES_GUEST_DMA_GPA=1 CARAXES_MIRROR_CHANNELS=1 CARAXES_MIRROR_GR_SHADOW=1 CARAXES_MIRROR_BARE_CE=1
        export CARAXES_GUEST_RAM_BYTES=$(( ${WIN_MEM_GIB:-4} << 30 ))
        rm -f "$W/dma-ranges" "$W/dma-ranges.guest" ;;
    *) die "WIN_HOST_MODE must be rm or exec" ;;
esac
export CARAXES_HOST_VRAM_FLOOR_MIB=${CARAXES_HOST_VRAM_FLOOR_MIB:-768}
export CARAXES_HOST_BAR1_FLOOR_MIB=${CARAXES_HOST_BAR1_FLOOR_MIB:-96}
export CARAXES_BAR1_WINDOW_MIB=${CARAXES_BAR1_WINDOW_MIB:-16} CARAXES_BAR1_WINDOWS=${CARAXES_BAR1_WINDOWS:-2}
unset CARAXES_BAR1_PASSTHROUGH CARAXES_BAR1_POOL_MIB
if [[ -n ${WIN_BAR1_POOL_MIB:-} ]]; then
    export CARAXES_BAR1_PASSTHROUGH=pool CARAXES_BAR1_POOL_MIB=$WIN_BAR1_POOL_MIB
fi
"$HERE/gpu-up.sh" || { rm -f "$V/win-busy"; exit 1; }

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# The Windows device with the host RM behind it (night-2026-09-25/WINDOWS-LOOP.md, "Host RM"): the
# same start as gpu-up.sh (WIN_GPU_SERVER, WIN_GPU_ROM, WIN_GPU_VBIOS, the 1 GiB profile) plus the
# host-RM flags of the Linux stage (caraxes/runs/sottr-2026-09-24/env.sh, tools/caraxes-vm.sh), no BAR1
# pool, BAR1 windows 2 x 16 MiB, the floors (VRAM 768 MiB, BAR1 96 MiB).
#   WIN_HOST_MODE=rm (default): --host-rm only. The host RM answers the forwardable controls as an
#     unprivileged client (the FB/BUS/GR info the KMD asks); nothing executes on the card, no VRAM is
#     reserved, no guest RAM is registered. Needs no privileges.
#   WIN_HOST_MODE=exec: --host-rm --reserve-vram --reserve-slot ${WIN_RESERVE_SLOT:-0} --host-exec
#     --register-guest-ram, as the Linux stage. Needs a copy of the binary with cap_sys_admin+p in
#     $W (the launcher does `sudo -n setcap`; this script does not) AND a guest that hands the device
#     IOVAs: Linux loads caraxes_dma_range for that, Windows has no such module, so the device would
#     need a "guest speaks GPA" alias mode first (dmamap.rs, guestmem.rs host_iova). Refused until then.
# Refuses to start while a vm4-615 device (the Linux stage) runs: BAR1 and VRAM budget.
set -euo pipefail
W=/mnt/vmstore/caraxes/win11
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
if pgrep -f '/vm4-615/vfio-user-nvgpu' >/dev/null; then die "a vm4-615 device runs (Linux stage): the host is not free"; fi
case ${WIN_HOST_MODE:-rm} in
    rm) export WIN_GPU_EXTRA="--host-rm" ;;
    exec) die "WIN_HOST_MODE=exec needs a GPA alias mode for a guest without caraxes_dma_range (see the header)" ;;
    *) die "WIN_HOST_MODE must be rm or exec" ;;
esac
export CARAXES_HOST_VRAM_FLOOR_MIB=${CARAXES_HOST_VRAM_FLOOR_MIB:-768}
export CARAXES_HOST_BAR1_FLOOR_MIB=${CARAXES_HOST_BAR1_FLOOR_MIB:-96}
export CARAXES_BAR1_WINDOW_MIB=${CARAXES_BAR1_WINDOW_MIB:-16} CARAXES_BAR1_WINDOWS=${CARAXES_BAR1_WINDOWS:-2}
unset CARAXES_BAR1_PASSTHROUGH CARAXES_BAR1_POOL_MIB
exec "$HERE/gpu-up.sh"

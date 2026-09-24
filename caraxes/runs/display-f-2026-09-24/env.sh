# SPDX-License-Identifier: MIT
# The display package's VM for series F (docs/caraxes/display-e.md): vm0 at reservation slot 1
# with the 2 GiB profile, host execution, the display engine, the configuration the display
# gate passed 11/11 with (gate-display-2026-09-24-7). Source it from the worktree root.
R=$(pwd -P)
export VM=vm0 CARAXES_IP=192.168.100.20 CARAXES_TAP=caraxes0 CARAXES_MAC=02:00:00:00:00:20
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
export CARAXES_MIRROR_GR_SHADOW=1 CARAXES_MIRROR_BARE_CE=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=2048 CARAXES_GATE_FB_MIB=2048 CARAXES_RESERVE_SLOT=1 CARAXES_DISPLAY=1
export CARAXES_SERVER=${CARAXES_SERVER:-$R/target/display/release/vfio-user-nvgpu}

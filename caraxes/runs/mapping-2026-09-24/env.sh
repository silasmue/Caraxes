# SPDX-License-Identifier: MIT
# The mapping package's VM (docs/caraxes/mapping.md): vm2 alone at reservation slot 0 with a
# 2 GiB profile (the display agent holds slot 1 on vm0), the green host-execution
# configuration of testing.md §7b. Source it, then pick the device with CARAXES_SERVER.
R=/home/silas/git/Caraxes
export VM=vm2 CARAXES_IP=192.168.102.22 CARAXES_TAP=caraxes2 CARAXES_MAC=02:00:00:00:00:22
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
export CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=2048 CARAXES_GATE_FB_MIB=2048 CARAXES_RESERVE_SLOT=0
export CARAXES_SERVER=${CARAXES_SERVER:-$R/target/mapping/release/vfio-user-nvgpu}

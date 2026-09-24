# SPDX-License-Identifier: MIT
# The interrupt page series on vm0: the environment of series F (display-e.md), vm0 at slot 1
# with the 2 GiB profile, host execution, the display engine, the configuration the display
# gate passed 11/11 with (gate-display-2026-09-24-7). Source it from the worktree root.
R=$(pwd -P)
export VM=vm0 CARAXES_IP=192.168.100.20 CARAXES_TAP=caraxes0 CARAXES_MAC=02:00:00:00:00:20
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
export CARAXES_MIRROR_GR_SHADOW=1 CARAXES_MIRROR_BARE_CE=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=2048 CARAXES_GATE_FB_MIB=2048 CARAXES_RESERVE_SLOT=1 CARAXES_DISPLAY=1
export CARAXES_SERVER=${CARAXES_SERVER:-/mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-intrpage}
# The interrupt page series (docs/caraxes/intr-page.md): Cloud Hypervisor 0001-0006 (the
# token pool needs 0006), the token pool and the hybrid reactor in both configurations;
# CARAXES_INTR_PAGE is set per configuration by series.sh.
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid CARAXES_DOORBELL_POLL_IDLE_US=200
# The launcher and the gate of the main checkout: they find the git-ignored material (the ROM
# capture, local.env, the vendor trees, ../Leandro-Test) beside themselves.
export LEA_TOOLS=${LEA_TOOLS:-/home/silas/git/Caraxes/tools}

# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# vm5 for the GSP plan B and C package (docs/caraxes/gsp-plan/): the 610 guest image, a
# 1 GiB profile at reservation slot 6, the green host-execution configuration of
# caraxes/runs/doorbell-2026-09-24/env-vm1.sh otherwise, token pool and hybrid reactor,
# no BAR1 area. The tap caraxes5 is shared with the v1 bench VM (down during the runs).
R=/home/silas/git/Caraxes
export VM=vm5 CARAXES_IP=192.168.105.26 CARAXES_TAP=caraxes5 CARAXES_MAC=02:00:00:00:00:26
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
export CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=6
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_DRIVER CARAXES_GUEST_ABI

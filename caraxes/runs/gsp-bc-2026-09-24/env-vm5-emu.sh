# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# vm5 in pure emulation: slot 6 of env-vm5.sh was refused by the host RM on 2026-09-24
# (NV_ERR_NO_MEMORY for [0x29800000, 0x69800000), before-slot6-refused/up.txt), and the
# device fails closed. The same VM with the software device only: no host RM client, no
# reservation, no host execution; the 610 guest image, a 1 GiB profile.
R=/home/silas/git/Caraxes
export VM=vm5 CARAXES_IP=192.168.105.26 CARAXES_TAP=caraxes5 CARAXES_MAC=02:00:00:00:00:26
export CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
unset CARAXES_HOST_RM CARAXES_HOST_EXEC CARAXES_MIRROR_CHANNELS CARAXES_MIRROR_GR_SHADOW
unset CARAXES_RESERVE_VRAM CARAXES_RESERVE_SLOT CARAXES_DMA CARAXES_DOORBELL_TOKENS CARAXES_DOORBELL_POLL
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_DRIVER CARAXES_GUEST_ABI

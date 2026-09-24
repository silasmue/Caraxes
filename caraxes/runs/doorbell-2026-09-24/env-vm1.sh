# SPDX-License-Identifier: MIT
# vm1 for the doorbell measurement while vm2 (mapping) and vm0 (display) are busy: a 1 GiB
# profile at reservation slot 4, which lies below vm2's slot 0 and vm0's slot 1 (both 2 GiB
# profiles: slot n is n profile sizes below the top, hostmem.rs slot_range), the green
# host-execution configuration of testing.md §7b otherwise.
R=/home/silas/git/Caraxes
export VM=vm1 CARAXES_IP=192.168.101.21 CARAXES_TAP=caraxes1 CARAXES_MAC=02:00:00:00:00:21
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
export CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=4

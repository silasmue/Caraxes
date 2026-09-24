# SPDX-License-Identifier: MIT
# Environment of the doorbell token pool check in vm1 (pure emulation: no CARAXES_RESERVE_VRAM,
# no CARAXES_HOST_EXEC, no CARAXES_DMA). Source it, then set CARAXES_DOORBELL_TOKENS per run.
export VM=vm1 CARAXES_IP=192.168.101.21 CARAXES_TAP=caraxes1 CARAXES_MAC=02:00:00:00:00:21
export CARAXES_HOST_RM=1 CARAXES_BOOT_MODEL=1
unset CARAXES_RESERVE_VRAM CARAXES_RESERVE_SLOT CARAXES_HOST_EXEC CARAXES_DMA CARAXES_FB_MIB
unset CARAXES_MIRROR_CHANNELS CARAXES_MIRROR_GR_SHADOW CARAXES_BAR1_PASSTHROUGH CARAXES_DOORBELL_IOEVENTFD
# Cloud Hypervisor v53.0 + patches 0001-0006 (v53.0-6-g88e92aded), with -v.
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
# The device built from the token-pool tree, frozen.
export CARAXES_SERVER=/mnt/vmstore/caraxes/vm1/vfio-user-nvgpu-tokens

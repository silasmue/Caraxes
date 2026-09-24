# SPDX-License-Identifier: MIT
# Environment of the patch 0005 check in vm3 (pure emulation: no CARAXES_RESERVE_VRAM, no
# CARAXES_HOST_EXEC, no CARAXES_DMA). Source it, then set CARAXES_SERVER per run.
export VM=vm3 CARAXES_TAP=caraxes3 CARAXES_IP=192.168.103.23 CARAXES_MAC=02:00:00:00:00:23
export CARAXES_HOST_RM=1 CARAXES_BOOT_MODEL=1
# Cloud Hypervisor v53.0 + patches 0001-0005 (v53.0-5-gba20497bc), with -v.
export LEA_CH=/mnt/vmstore/caraxes/vm3/ch-0005-v.sh

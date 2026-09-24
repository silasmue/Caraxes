# SPDX-License-Identifier: MIT
# The SotTR stage (docs/caraxes/sottr-stage.md): vm4 as the game VM. The display settings are
# those of configuration C of the display page series (caraxes/runs/display-page-2026-09-24/,
# env.sh there and ../intr-page-2026-09-24/env.sh: host execution, the display engine, the
# interrupt page and the display page, the doorbell token pool with the hybrid reactor, Cloud
# Hypervisor 0001-0006, the frozen fepage device binary), with a 3 GiB profile at reservation
# slot 0 (the top 3 GiB of the host FB) and the v1 Steam library as a thin overlay.
# Slot 0 of a 3 GiB profile overlaps vm0 (2 GiB, slot 1 = top-4G..top-2G) and vm2 (2 GiB,
# slot 0): up.sh refuses while either runs. vm1 (1 GiB, slot 4 = top-5G..top-4G) may stay up.
# SOTTR_EMULATION=1 gives the same VM in pure emulation (no host RM, no reservation, no host
# execution, no DMA registration): the plumbing test of caraxes/runs/sottr-2026-09-24/plumbing/.
# Source it from any directory; the scripts beside it do.
export VM=vm4 CARAXES_IP=192.168.103.24 CARAXES_TAP=caraxes3 CARAXES_MAC=02:00:00:00:00:24
export CARAXES_MEM=${CARAXES_MEM:-8G} CARAXES_CPUS=${CARAXES_CPUS:-8}
export CARAXES_BOOT_MODEL=1 CARAXES_DISPLAY=1 CARAXES_FB_MIB=3072 CARAXES_GATE_FB_MIB=3072
if [[ ${SOTTR_EMULATION:-0} == 1 ]]; then
    unset CARAXES_HOST_RM CARAXES_HOST_EXEC CARAXES_RESERVE_VRAM CARAXES_RESERVE_SLOT CARAXES_DMA
    unset CARAXES_MIRROR_CHANNELS CARAXES_MIRROR_GR_SHADOW CARAXES_MIRROR_BARE_CE
else
    export CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1 CARAXES_MIRROR_GR_SHADOW=1
    export CARAXES_MIRROR_BARE_CE=1 CARAXES_RESERVE_VRAM=1 CARAXES_RESERVE_SLOT=0 CARAXES_DMA=register
fi
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid CARAXES_DOORBELL_POLL_IDLE_US=200
export CARAXES_INTR_PAGE=1 CARAXES_DISPLAY_PAGE=1
# Not in the display series, so not here: the BAR1 pool (docs/caraxes/bar1-pool.md, never run on
# a VM with the display), BAR1 passthrough, ioeventfd doorbells, the 615 guest ABI.
unset CARAXES_BAR1_PASSTHROUGH CARAXES_BAR1_POOL_MIB CARAXES_DOORBELL_IOEVENTFD CARAXES_GUEST_ABI
unset CARAXES_DRIVER CARAXES_GAMES_DISK_RW CARAXES_INPUT
# The device of main 3cb33af (display page, interrupt page, the FB_FLUSH_GPU_CACHE fix for the
# 615 host, GSP E+A, LCE shim). Not the display series' frozen fepage binary: it predates the
# flush fix and floods the 615 host's log with kmemsysFlushGpuCache "Invalid aperture".
export CARAXES_SERVER=${SOTTR_SERVER:-/mnt/vmstore/caraxes/vfio-user-nvgpu-main-3cb33af}
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
# The v1 Steam library (ext4, label LEAGAMES, /SteamLibrary on it); vm4 gets $W/games.qcow2.
export CARAXES_GAMES_DISK=${SOTTR_GAMES_BASE:-/mnt/vmstore/leandro/vm/games.qcow2}
# The launcher and the gate of the main checkout (they find the git-ignored ROM capture and
# local.env beside themselves); the guest image is the 610.57.04 base of local.env.
export LEA_TOOLS=${LEA_TOOLS:-/home/silas/git/Caraxes/tools}
# Guest preparation inputs: the open kernel modules source of 610.57.04 (sha256 4e0f09ed...)
# and the 610.57.04 GSP firmware (sha256 d157e3b7...; the host now has only 615.71.09, so up.sh
# takes it from vm3's 610 overlay with guestfish --ro when $W lacks it).
export SOTTR_OGKM=${SOTTR_OGKM:-/home/silas/git/Caraxes/target/caraxes/ogkm-610.57.04.tar.zst}
export SOTTR_GSP_SHA256=d157e3b7dd5da2ca8d1ccb6ca98958f9e35d10a9ef7326277ebac133e4b0d1a7
# Sunshine's web login and pairing PIN, as the display stream stage (display-e.md section 7)
# and v1 (Leandro-Test scripts/lib/config.sh:204-206) set them.
export SOTTR_SUN_USER=lea SOTTR_SUN_PASS=leastream SOTTR_SUN_PIN=4321

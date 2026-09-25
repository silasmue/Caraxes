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
# 4 GiB of guest RAM: the host has 31 GB, and 8 GiB pushed the host desktop into swap (2026-09-25).
# 8 GiB: with 4 GiB the game failed at vkCreateDevice with VK_ERROR_OUT_OF_HOST_MEMORY (-1),
# 861 MiB free beside GNOME, Sunshine and Steam (2026-09-25).
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
# 2026-09-25: the recommended configuration of the morning matrix (benchmark.md "Morning matrix",
# the 610 display gate passed 11 of 11 with it): the BAR1 pool (128 MiB, the game VM is alone on
# the host's BAR1), the BAR2 memfd, the MSI re-arm ioeventfd and the lazy allocation plans, on the
# main b22ab48 build with the pool's untrapped-GPPut hold (bar1-pool.md §9). SOTTR_PLAIN=1 restores
# the display-series configuration C the stage was first tested with.
if [[ ${SOTTR_PLAIN:-0} == 1 ]]; then
    unset CARAXES_BAR1_PASSTHROUGH CARAXES_BAR1_POOL_MIB CARAXES_BAR2_MEMFD CARAXES_MSI_REARM_IOEVENTFD CARAXES_ALLOC_LAZY
else
    # 64 MiB, not 128: the host desktop keeps its own BAR1 (the host BAR1 floor, 2026-09-25).
    # SOTTR_NO_POOL=1 keeps the reactor paths but not the pool: on 615 the guest maps
    # page-table pages through BAR1 windows, which the pool cannot watch (2026-09-25).
    if [[ ${SOTTR_NO_POOL:-0} == 1 ]]; then
        unset CARAXES_BAR1_PASSTHROUGH CARAXES_BAR1_POOL_MIB
    else
        export CARAXES_BAR1_PASSTHROUGH=pool CARAXES_BAR1_POOL_MIB=${SOTTR_POOL_MIB:-64}
    fi
    export CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 CARAXES_ALLOC_LAZY=1
fi
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_GUEST_ABI
unset CARAXES_DRIVER CARAXES_GAMES_DISK_RW CARAXES_INPUT
# The device of main 3cb33af (display page, interrupt page, the FB_FLUSH_GPU_CACHE fix for the
# 615 host, GSP E+A, LCE shim). Not the display series' frozen fepage binary: it predates the
# flush fix and floods the 615 host's log with kmemsysFlushGpuCache "Invalid aperture".
export CARAXES_SERVER=${SOTTR_SERVER:-/mnt/vmstore/caraxes/vfio-user-nvgpu-main-b22ab48}
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
# The base image of the 610 path; SOTTR_DRIVER=615 switches to vm4-615 on the 615 image.
export SOTTR_DRIVER=${SOTTR_DRIVER:-610}
export SOTTR_BASE=/mnt/vmstore/leandro/vm/guest-baked-20260801-610.57.04-20260820-105733.qcow2
if [[ $SOTTR_DRIVER == 615 ]]; then
    # shellcheck source=caraxes/runs/sottr-2026-09-24/env-615.sh
    source "$(dirname "${BASH_SOURCE[0]}")/env-615.sh"
elif [[ $SOTTR_DRIVER != 610 ]]; then
    echo "ERROR: SOTTR_DRIVER=$SOTTR_DRIVER: 610 or 615" >&2; return 1
fi
# Sunshine's encoder: software (libx264, ultrafast, zerolatency) on 610; env-615.sh prefers nvenc.
export SOTTR_SUN_ENCODER=${SOTTR_SUN_ENCODER:-software}

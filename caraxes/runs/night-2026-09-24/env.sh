# SPDX-License-Identifier: MIT
# The night shift of 2026-09-24 (docs/caraxes/benchmark.md "Night series"): the common
# environment of its parts. vm2 only, 1 GiB profile at reservation slot 3 as
# caraxes/runs/cutover-615-2026-09-24/v2-615guest-run.sh has it, Cloud Hypervisor 0001-0006,
# token pool 64, hybrid reactor, interrupt page on, BAR1 pool; a part overrides what it varies.
# Frozen device binaries of main 57693d8 (they carry the FB_FLUSH_GPU_CACHE fix for the 615
# host). Source it from a run script, then call night_guest 610|615.
# The tools run from the checkout this file lives in (a worktree: the git-ignored ROM capture,
# local.env, target/bench and the vendor tree are links into the main checkout).
NIGHT_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)
export LT=/home/silas/git/Leandro-Test CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test
export LEA_HOSTVENV=/home/silas/git/Leandro/vendor/hostvenv
export CARAXES_IP=192.168.102.22 CARAXES_TAP=caraxes2 CARAXES_MAC=02:00:00:00:00:22
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
export CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=3
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid CARAXES_INTR_PAGE=1
export CARAXES_BAR1_PASSTHROUGH=pool
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_BAR2_MEMFD CARAXES_ALLOC_LAZY CARAXES_DISPLAY CARAXES_GUEST_ABI
NIGHT_BIN610=/mnt/vmstore/caraxes/vfio-user-nvgpu-main-57693d8
NIGHT_BIN615=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-57693d8

night_guest() {  # 610: vm2 on the default image; 615: vm2-615 with the 615 guest driver
    case $1 in
        610) export VM=vm2 CARAXES_SERVER=$NIGHT_BIN610; unset CARAXES_DRIVER ;;
        615) export VM=vm2-615 CARAXES_DRIVER=615 CARAXES_SERVER=$NIGHT_BIN615 ;;
        *) echo "ERROR: guest 610 or 615" >&2; return 2 ;;
    esac
    NIGHT_W=/mnt/vmstore/caraxes/$VM
}

night_vm2_free() {  # refuse to start while any vm2 process runs
    if pgrep -f /mnt/vmstore/caraxes/vm2 >/dev/null; then
        echo "ERROR: a vm2 process runs; refusing to start" >&2; pgrep -af /mnt/vmstore/caraxes/vm2 >&2; return 2
    fi
}

night_setup() {  # DIR: the configuration record
    { head -1 /proc/driver/nvidia/version; sha256sum "$CARAXES_SERVER" "$(readlink -f "$LEA_CH")" 2>/dev/null
      git -C "$NIGHT_ROOT" log --oneline -1
      echo "bench-run.sh libdir v2: $("$NIGHT_ROOT/caraxes/tools/bench-run.sh" libdir v2)"
      env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; } > "$1/setup.txt"
    pgrep -af 'vfio-user-nvgpu --socket|vhost-user-nvrm' | grep -v pgrep > "$1/other-vms-at-start.txt"
}

night_xid() {  # DIR T0: host Xid lines since T0, and the device pids seen in the part
    journalctl -k --since "$2" --no-pager -g Xid > "$1/host-xid.txt" 2>/dev/null
    grep -h 'dev_pid=' "$1"/v2/*.txt "$1"/*/v2/*.txt 2>/dev/null | grep -o 'dev_pid=[0-9]*' | sort -u > "$1/device-pids.txt"
}

night_quiet() {  # wait until no other device process runs (every 5 min, QUIET_WAIT_MIN, default 180)
    local t=0 max=${QUIET_WAIT_MIN:-180} pat='vfio-user-nvgpu --socket|vhost-user-nvrm --nvrm'
    while pgrep -f "$pat" >/dev/null; do
        (( t >= max )) && return 1
        echo "$(date +%T) waiting for a quiet host: $(pgrep -af "$pat" | grep -oE '/mnt/vmstore/caraxes/[^/ ]+' | sort -u | tr '\n' ' ')"
        sleep 300; t=$((t + 5))
    done
}

# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The launch path series (docs/caraxes/launch-path.md): vm5 with the 610 guest, 1 GiB at
# reservation slot 2, the host-execution environment of caraxes/runs/doorbell-2026-09-24/env-vm1.sh
# and the recommended device windows (interrupt page, token pool 64, hybrid reactor, BAR1 pool
# 32 MiB, BAR2 memfd, MSI re-arm ioeventfd, lazy plans), plus CARAXES_LAUNCH_TIMING=1. The
# device binary is CARAXES_SERVER (a frozen build under /mnt/vmstore/caraxes). Source it from
# bench.sh.
OL_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)
OL_HERE=$OL_ROOT/caraxes/runs/opt-launch-2026-09-25
export LT=/home/silas/git/Leandro-Test CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test
export LEA_HOSTVENV=/home/silas/git/Leandro/vendor/hostvenv
export LEA_TOOLS=$OL_ROOT/tools
export VM=vm5 CARAXES_IP=192.168.105.26 CARAXES_TAP=caraxes5 CARAXES_MAC=02:00:00:00:00:26
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
export CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=2
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_INTR_PAGE=1 CARAXES_BAR1_PASSTHROUGH=pool CARAXES_BAR1_POOL_MIB=32
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
export CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 CARAXES_ALLOC_LAZY=1
export CARAXES_LAUNCH_TIMING=1
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_BAR2_MEMFD_SYNC_DOORBELL CARAXES_TRACE_ACCESSES CARAXES_TRACE_FLUSH
unset CARAXES_GUEST_ABI CARAXES_COSTUME CARAXES_DRIVER CARAXES_DISPLAY CARAXES_DISPLAY_PAGE
unset CARAXES_MIRROR_BARE_CE CARAXES_DOORBELL_POLL_IDLE_US
OL_W=/mnt/vmstore/caraxes/$VM

ol_free() {  # the brief's precondition: no vm5 process and no bench-v1 VM
    if pgrep -af "/mnt/vmstore/caraxes/$VM/" >/dev/null || pgrep -af bench-v1 >/dev/null; then
        echo "ERROR: a $VM or bench-v1 process runs; refusing to start" >&2
        pgrep -af "/mnt/vmstore/caraxes/$VM/|bench-v1" | cut -c1-160 >&2
        return 2
    fi
}

ol_setup() {  # DIR: the configuration record
    { head -1 /proc/driver/nvidia/version; sha256sum "$CARAXES_SERVER" "$(readlink -f "$LEA_CH")" 2>/dev/null
      git -C "$OL_ROOT" log --oneline -1; date -Iseconds; cut -d' ' -f1-3 /proc/loadavg
      env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; } > "$1/setup.txt"
    pgrep -af 'vfio-user-nvgpu --socket|vhost-user-nvrm|cloud-hypervisor' | grep -v pgrep | cut -c1-160 > "$1/other-vms-at-start.txt"
}

ol_finish() {  # DIR T0: down, keep the last device log, the host's Xid lines with the device pid
    local pid; pid=$(cat "$OL_W/gpu.pid" 2>/dev/null)
    "$LEA_TOOLS/caraxes-vm.sh" down > "$1/down.txt" 2>&1
    cp "$OL_W/gpu.log" "$1/last-device-log.txt" 2>/dev/null
    { echo "# host kernel log, Xid lines since $2 (last device pid $pid; each rep's pid in */v2-device-rep*.txt)"
      journalctl -k --since "$2" --no-pager -g Xid 2>/dev/null | grep -v '^-- No entries'; } > "$1/host-xid.txt"
    echo "$(basename "$1") done $(date -Iseconds): host Xid lines $(grep -vc '^#' "$1/host-xid.txt"), $VM processes left $(pgrep -fc "/mnt/vmstore/caraxes/$VM/")"
}

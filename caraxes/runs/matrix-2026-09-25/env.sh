# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The morning matrix of 2026-09-25 (docs/caraxes/benchmark.md "Morning matrix"): the common
# environment of its parts. Frozen device binaries of main fbe4dfb (610 guest ABI and the v615
# build), the recommended device windows of caraxes/tools/bench-run.sh's header (interrupt page,
# BAR1 pool 32 MiB, token pool 64, hybrid reactor, BAR2 memfd, MSI re-arm as an ioeventfd, no
# P21 area) plus CARAXES_ALLOC_LAZY=1 (alloc-path.md §7). The pool is 32 MiB throughout because a
# second VM of this matrix or the costume ladder's vm1 may be up (bench-run.sh header).
# Source it from a part script, then call mx_guest.
# The tools run from the checkout this file lives in (a worktree: local.env, target/bench, the ROM
# capture and the vendor tree are links into the main checkout).
MX_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)
MX_HERE=$MX_ROOT/caraxes/runs/matrix-2026-09-25
export LT=/home/silas/git/Leandro-Test CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test
export LEA_HOSTVENV=/home/silas/git/Leandro/vendor/hostvenv
export LEA_TOOLS=$MX_ROOT/tools
MX_BIN610=/mnt/vmstore/caraxes/vfio-user-nvgpu-main-fbe4dfb
MX_BIN615=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-fbe4dfb

mx_windows() {  # the recommended configuration, on top of a guest selector's base
    export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
    export CARAXES_INTR_PAGE=1 CARAXES_BAR1_PASSTHROUGH=pool CARAXES_BAR1_POOL_MIB=32
    export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
    export CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 CARAXES_ALLOC_LAZY=1
    unset CARAXES_DOORBELL_IOEVENTFD CARAXES_BAR2_MEMFD_SYNC_DOORBELL CARAXES_TRACE_ACCESSES CARAXES_TRACE_FLUSH
    unset CARAXES_GUEST_ABI CARAXES_COSTUME
}
mx_hostexec() {  # host RM, host execution, reserved VRAM (caraxes/runs/gsp-rpc-2026-09-24/run.sh gate610)
    export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
    export CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
    unset CARAXES_DISPLAY CARAXES_DISPLAY_PAGE CARAXES_MIRROR_BARE_CE CARAXES_DOORBELL_POLL_IDLE_US
}

# Selectors. 610: vm3 (1 GiB, slot 5) as gsp-rpc-2026-09-24/run.sh gate610; 615: vm5-615 (1 GiB,
# slot 2) as gsp-round2-c-2026-09-24/run.sh 615; costume: vm3 with CARAXES_COSTUME=rtx5060ti
# (costume-2026-09-24/run.sh); wl615: the workload gate's 615 guest (vm2-615, the night's
# workload overlay with the llama build staged, workload-2026-09-24/night/env.sh) at the 2 GiB
# profile in slot 0.
mx_guest() {
    mx_hostexec; mx_windows
    case $1 in
        610|costume)
            export VM=vm3 CARAXES_IP=192.168.103.23 CARAXES_TAP=caraxes3 CARAXES_MAC=02:00:00:00:00:23
            export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=5
            export CARAXES_SERVER=$MX_BIN610; unset CARAXES_DRIVER
            [[ $1 == costume ]] && export CARAXES_COSTUME=rtx5060ti ;;
        615)
            export VM=vm5-615 CARAXES_IP=192.168.105.26 CARAXES_TAP=caraxes5 CARAXES_MAC=02:00:00:00:00:26
            export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=2
            export CARAXES_SERVER=$MX_BIN615 CARAXES_DRIVER=615 ;;
        wl615)
            export VM=vm2-615 CARAXES_IP=192.168.102.22 CARAXES_TAP=caraxes2 CARAXES_MAC=02:00:00:00:00:22
            export CARAXES_FB_MIB=2048 CARAXES_GATE_FB_MIB=2048 CARAXES_RESERVE_SLOT=0
            export CARAXES_SERVER=$MX_BIN615 CARAXES_DRIVER=615 ;;
        *) echo "ERROR: guest 610|615|costume|wl615" >&2; return 2 ;;
    esac
    MX_W=/mnt/vmstore/caraxes/$VM
}

mx_free() {  # refuse to start while a process of the VM's state folder runs
    if pgrep -f "/mnt/vmstore/caraxes/$VM/" >/dev/null; then
        echo "ERROR: a $VM process runs; refusing to start" >&2; pgrep -af "/mnt/vmstore/caraxes/$VM/" >&2; return 2
    fi
}

mx_setup() {  # DIR: the configuration record
    { head -1 /proc/driver/nvidia/version; sha256sum "$CARAXES_SERVER" "$(readlink -f "$LEA_CH")" 2>/dev/null
      git -C "$MX_ROOT" log --oneline -1; date -Iseconds; cut -d' ' -f1-3 /proc/loadavg
      env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; } > "$1/setup.txt"
    pgrep -af 'vfio-user-nvgpu --socket|vhost-user-nvrm|cloud-hypervisor' | grep -v pgrep | cut -c1-160 > "$1/other-vms-at-start.txt"
}

mx_up() {  # DIR: up and wait; a reservation refused for headroom is retried every 5 min, up to an hour
    local try
    for try in $(seq 1 13); do
        if "$LEA_TOOLS/caraxes-vm.sh" up >> "$1/up.txt" 2>&1 && "$LEA_TOOLS/caraxes-vm.sh" wait 180 >> "$1/up.txt" 2>&1; then
            pgrep -af "vfio-user-nvgpu.*$MX_W/" | cut -c1-200 > "$1/device-pid.txt"; return 0
        fi
        tail -3 "$1/up.txt"
        "$LEA_TOOLS/caraxes-vm.sh" down >> "$1/up.txt" 2>&1
        if grep -qE 'headroom|NO_MEMORY|did not give|refused: reserving' "$1/up.txt" "$MX_W/gpu.log" 2>/dev/null && ((try < 13)); then
            echo "try $try $(date -Iseconds): reservation refused, next try in 5 min" | tee -a "$1/retries.txt"
            sleep 300; continue
        fi
        return 1
    done
    return 1
}

mx_finish() {  # DIR T0: down, the device's summary, the host's Xid lines with the device pid
    local pid; pid=$(cat "$MX_W/gpu.pid" 2>/dev/null)
    "$LEA_TOOLS/caraxes-vm.sh" down > "$1/down.txt" 2>&1
    grep -E 'intr page|bar2 memfd|bar1 pool|bar1 passthrough|msi|lazy|doorbell|mirror: |shadow: |swiotlb|alias|unanswered|ERROR|WARN|panic' "$MX_W/gpu.log" \
        | tail -60 | cut -c1-1600 > "$1/device-summary.txt"
    cp "$MX_W/dma-ranges" "$1/dma-ranges.txt" 2>/dev/null
    cp "$MX_W/dma-ranges.guest" "$1/dma-ranges-guest.txt" 2>/dev/null
    { echo "# host kernel log, Xid lines since $2 (device pid $pid)"
      journalctl -k --since "$2" --no-pager -g Xid 2>/dev/null | grep -v '^-- No entries'; } > "$1/host-xid.txt"
    echo "$(basename "$1") done $(date -Iseconds): device pid $pid, host Xid lines $(grep -vc '^#' "$1/host-xid.txt"), $VM processes left $(pgrep -fc "/mnt/vmstore/caraxes/$VM/")"
}

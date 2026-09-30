#!/usr/bin/env bash
# Bounded Ubuntu/QEMU comparison for the desktop-a Cloud Hypervisor configuration.
# This is a diagnostic harness; desktop.sh is the measured Cloud Hypervisor baseline.
set -euo pipefail

BASE=${CARAXES_QEMU_BASE:-/mnt/vmstore/caraxes/alpha-2026-09-27}
SOURCE=$BASE/desktop-a
W=${CARAXES_QEMU_STATE:-$BASE/ubuntu-qemu-a}
QEMU=${CARAXES_QEMU_BIN:-qemu-system-x86_64}
SERVER=${CARAXES_QEMU_SERVER:-$SOURCE/vfio-user-nvgpu}
KERNEL=${CARAXES_QEMU_KERNEL:-/mnt/vmstore/leandro/vm/guest-image/vmlinuz}
INITRD=${CARAXES_QEMU_INITRD:-/mnt/vmstore/leandro/vm/guest-image/initrd}
TAP=${CARAXES_QEMU_TAP:-caraxes5}
IP=${CARAXES_QEMU_IP:-192.168.105.24}
MAC=${CARAXES_QEMU_MAC:-02:00:00:00:00:25}
SSH_KEY=${CARAXES_QEMU_SSH_KEY:-/mnt/vmstore/leandro/vm/id_leandro}
REF=/home/silas/git/Caraxes/caraxes/reference/rtx2070
ROOTFS=$W/rootfs.qcow2
SEED=$W/seed.img

die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
alive() { [[ -s $1 ]] && kill -0 "$(cat "$1")" 2>/dev/null; }
owned_alive() {
    local pidfile=$1 marker=$2 pid
    alive "$pidfile" || return 1
    pid=$(cat "$pidfile")
    [[ -r /proc/$pid/cmdline ]] || return 1
    tr '\0' ' ' < "/proc/$pid/cmdline" | grep -Fq -- "$marker"
}
desktop_stopped() {
    ! alive "$SOURCE/ch.pid" && ! alive "$SOURCE/gpu.pid" ||
        die "desktop-a is active; stop it with $BASE/desktop.sh down before this comparison"
}
exclusive_slot() {
    local pid
    for pid in $(pgrep -x vfio-user-nvgpu 2>/dev/null || true); do
        [[ -r /proc/$pid/cmdline ]] || continue
        tr '\0' ' ' < "/proc/$pid/cmdline" | grep -q -- '--reserve-slot 0' &&
            die "vfio-user-nvgpu pid $pid already owns reserve slot 0"
    done
    return 0
}
preflight() {
    desktop_stopped
    exclusive_slot
    command -v "$QEMU" >/dev/null || die "$QEMU missing"
    [[ -c /dev/kvm && -r /dev/kvm && -w /dev/kvm ]] || die "/dev/kvm is not usable"
    [[ -x $SERVER ]] || die "$SERVER missing"
    getcap "$SERVER" | grep -q 'cap_sys_admin=p' || die "$SERVER lacks cap_sys_admin=p"
    [[ -f $KERNEL && -f $INITRD ]] || die "direct-boot kernel/initrd missing"
    [[ -f $REF/config-space.txt && -f $REF/rom.bin && -f $REF/synth-vbios.bin ]] ||
        die "RTX 2070 reference captures missing"
    ip link show "$TAP" >/dev/null 2>&1 || die "dedicated tap $TAP missing"
    [[ -f $ROOTFS && -f $SEED ]] || die "run '$0 setup' after desktop-a is stopped"
    qemu-img info -U --backing-chain "$ROOTFS" | grep -Fq "image: $SOURCE/rootfs.qcow2" ||
        die "$ROOTFS is not an isolated child of desktop-a/rootfs.qcow2"
}
setup() {
    desktop_stopped
    command -v qemu-img >/dev/null || die "qemu-img missing"
    command -v truncate >/dev/null || die "truncate missing"
    command -v mkfs.vfat >/dev/null || die "mkfs.vfat missing"
    command -v mcopy >/dev/null || die "mcopy missing"
    command -v mtype >/dev/null || die "mtype missing"
    command -v guestfish >/dev/null || die "guestfish missing (needed for child-only network configuration)"
    [[ ! -e $ROOTFS && ! -e $SEED ]] || die "$W already contains rootfs.qcow2 or seed.img"
    mkdir -p "$W"
    qemu-img create -q -f qcow2 -F qcow2 -b "$SOURCE/rootfs.qcow2" "$ROOTFS"
    local tmp
    tmp=$(mktemp -d)
    trap 'rm -f -- "$tmp/user-data" "$tmp/meta-data" "$tmp/network-config"; rmdir -- "$tmp"' RETURN
    sed -e "s/192\.168\.103\.24/$IP/" -e 's/192\.168\.103\.1/192.168.105.1/' \
        -e 's/caraxes-vm4-615/caraxes-qemu-615/g' \
        < <(mtype -i "$SOURCE/seed.img" ::user-data) > "$tmp/user-data"
    mtype -i "$SOURCE/seed.img" ::meta-data > "$tmp/meta-data"
    printf 'version: 2\nethernets: {}\n' > "$tmp/network-config"
    # Match the source NoCloud FAT12 seed without requiring cloud-localds.
    truncate -s 1440K "$SEED"
    mkfs.vfat -F 12 -n cidata "$SEED" >/dev/null
    mcopy -i "$SEED" "$tmp/user-data" ::user-data
    mcopy -i "$SEED" "$tmp/meta-data" ::meta-data
    mcopy -i "$SEED" "$tmp/network-config" ::network-config
    # cloud-init will not rewrite an already initialized clone. Change the installed network
    # configuration in the child overlay itself; the source image remains read-only.
    guestfish --rw -a "$ROOTFS" -i sh \
        "files=\$(grep -rlE '192\\.168\\.103\\.(1|24)|02:00:00:00:00:24' /etc/systemd/network /etc/NetworkManager 2>/dev/null); test -n \"\$files\"; sed -i -e 's/192\\.168\\.103\\.24/192.168.105.24/g' -e 's/192\\.168\\.103\\.1/192.168.105.1/g' -e 's/02:00:00:00:00:24/02:00:00:00:00:25/g' \$files"
    printf 'Created isolated QEMU state in %s. The source desktop disk was not modified.\n' "$W"
}
up() {
    preflight
    ! alive "$W/qemu.pid" || die "QEMU already running (pid $(cat "$W/qemu.pid"))"
    ! alive "$W/gpu.pid" || die "backend already running (pid $(cat "$W/gpu.pid"))"
    rm -f "$W/gpu.sock" "$W/qmp.sock" "$W/dma-ranges"
    export LEA_ROOT=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd -P)
    export CARAXES_DRIVER=615 CARAXES_GUEST_ABI=615 CARAXES_GUEST_RAM_BYTES=$((4 << 30))
    export CARAXES_GUEST_DMA_GPA=1
    export CARAXES_MIRROR_CHANNELS=1 CARAXES_MIRROR_GR_SHADOW=1 CARAXES_CE_SHADOW=1
    export CARAXES_INTR_PAGE=1 CARAXES_DISPLAY_PAGE=1 CARAXES_BAR1_PASSTHROUGH=pool
    export CARAXES_BAR1_POOL_MIB=32 CARAXES_BAR1_WINDOWS=2 CARAXES_BAR1_WINDOW_MIB=16
    export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid CARAXES_BAR2_MEMFD=1
    export CARAXES_MSI_REARM_IOEVENTFD=1 CARAXES_ALLOC_LAZY=1
    export CARAXES_HOST_VRAM_FLOOR_MIB=1024 CARAXES_HOST_BAR1_FLOOR_MIB=64 CARAXES_TRACE_FLUSH=0
    "$SERVER" --socket "$W/gpu.sock" --trace "$W/gpu-trace.log" \
        --config "$REF/config-space.txt" --rom "$REF/rom.bin" --vbios "$REF/synth-vbios.bin" \
        --fb-mib 2048 --boot-model --host-rm --reserve-vram --reserve-slot 0 --host-exec \
        --display --guest-abi 615 --register-guest-ram --dma-ranges "$W/dma-ranges" \
        >"$W/gpu.log" 2>&1 &
    echo $! > "$W/gpu.pid"
    for _ in $(seq 50); do [[ -S $W/gpu.sock ]] && break; sleep 0.1; done
    [[ -S $W/gpu.sock ]] || { tail -n 30 "$W/gpu.log"; die "backend did not create its socket"; }

    "$QEMU" -enable-kvm -machine q35,accel=kvm,memory-backend=mem \
        -cpu host -smp 4 -m 4G -object memory-backend-memfd,id=mem,size=4G,share=on \
        -kernel "$KERNEL" -initrd "$INITRD" \
        -append 'root=/dev/vda1 rw console=ttyS0 net.ifnames=0' \
        -drive "if=none,id=root,file=$ROOTFS,format=qcow2,cache=writeback,discard=unmap" \
        -device virtio-blk-pci,drive=root \
        -drive "if=none,id=seed,file=$SEED,format=raw,readonly=on" -device virtio-blk-pci,drive=seed \
        -netdev "tap,id=n0,ifname=$TAP,script=no,downscript=no" \
        -device "virtio-net-pci,netdev=n0,mac=$MAC" \
        -device "{\"driver\":\"vfio-user-pci\",\"socket\":{\"type\":\"unix\",\"path\":\"$W/gpu.sock\"},\"rombar\":0}" \
        -vga none -display none -serial "file:$W/serial.log" \
        -qmp "unix:$W/qmp.sock,server=on,wait=off" >"$W/qemu.log" 2>&1 &
    echo $! > "$W/qemu.pid"
    printf 'QEMU pid %s, backend pid %s; SSH leandro@%s; logs in %s\n' \
        "$(cat "$W/qemu.pid")" "$(cat "$W/gpu.pid")" "$IP" "$W"
}
down() {
    local pid
    if alive "$W/qemu.pid" && ! owned_alive "$W/qemu.pid" "$W/qmp.sock"; then
        die "qemu.pid names a foreign process; refusing to signal it"
    fi
    if alive "$W/gpu.pid" && ! owned_alive "$W/gpu.pid" "$W/gpu.sock"; then
        die "gpu.pid names a foreign process; refusing to signal it"
    fi
    if owned_alive "$W/qemu.pid" "$W/qmp.sock"; then
        # Ask the guest to shut down first. QMP needs capabilities negotiation before commands.
        if command -v nc >/dev/null && [[ -S $W/qmp.sock ]]; then
            printf '%s\n%s\n' '{"execute":"qmp_capabilities"}' '{"execute":"system_powerdown"}' |
                nc -U -q 1 "$W/qmp.sock" >/dev/null 2>&1 || true
        fi
        for _ in $(seq 60); do owned_alive "$W/qemu.pid" "$W/qmp.sock" || break; sleep 0.5; done
        if owned_alive "$W/qemu.pid" "$W/qmp.sock"; then
            pid=$(cat "$W/qemu.pid"); kill "$pid"
            for _ in $(seq 20); do owned_alive "$W/qemu.pid" "$W/qmp.sock" || break; sleep 0.25; done
        fi
        if owned_alive "$W/qemu.pid" "$W/qmp.sock"; then
            pid=$(cat "$W/qemu.pid"); kill -KILL "$pid"
        fi
    fi
    rm -f "$W/qemu.pid"
    if owned_alive "$W/gpu.pid" "$W/gpu.sock"; then
        pid=$(cat "$W/gpu.pid"); kill "$pid"
        for _ in $(seq 20); do owned_alive "$W/gpu.pid" "$W/gpu.sock" || break; sleep 0.25; done
        owned_alive "$W/gpu.pid" "$W/gpu.sock" && kill -KILL "$pid"
    fi
    rm -f "$W/gpu.pid"
}
status() {
    for p in qemu gpu; do if alive "$W/$p.pid"; then echo "$p running pid $(cat "$W/$p.pid")"; else echo "$p stopped"; fi; done
}
ssh_guest() { ssh -i "$SSH_KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null "leandro@$IP" "$@"; }

case ${1:-} in
    setup) setup ;;
    preflight) preflight; echo "preflight OK" ;;
    up) up ;;
    down) down ;;
    status) status ;;
    ssh) shift; ssh_guest "$@" ;;
    *) echo "usage: $0 setup|preflight|up|down|status|ssh [command...]" >&2; exit 2 ;;
esac

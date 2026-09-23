#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Leandro v1 VM for the P13 benchmark (docs/caraxes/benchmark.md): vhost-user-nvrm plus the
# patched Cloud Hypervisor (v53.0 + patches/0001-0004) with --generic-vhost-user, guest modules
# nvrm_nodes and virtio_nvrm built in the guest from this tree. Same command lines as
# caraxes/runs/vhost-user-check-2026-09-23/vm.sh (patches/README.md "Manual VM check") without
# -v and LEA_DEBUG, 8 vCPUs, 4 GiB shared memory, the baked 610.57.04 guest image.
#
#   bench-v1vm.sh prepare     new overlay + seed in $V (seed copied from the v1check VM)
#   bench-v1vm.sh up          start the backend and the VM, wait for ssh
#   bench-v1vm.sh modules     copy guest-module/{nvrm_nodes,virtio_nvrm}, build, provision, insmod
#   bench-v1vm.sh ssh CMD     run CMD in the guest
#   bench-v1vm.sh pids        print "backend_pid vm_pid"
#   bench-v1vm.sh down        poweroff, stop the backend
# Environment: V (default /mnt/vmstore/caraxes/bench-v1), IP (192.168.105.25), TAP (caraxes5),
# MAC, CH (default vendor/cloud-hypervisor/target/release/cloud-hypervisor), NVRM (default
# target/bench/release/vhost-user-nvrm, built from this tree with CARGO_TARGET_DIR=target/bench).
set -euo pipefail
ROOT=$(cd "$(dirname "$(readlink -f "$0")")/../.." && pwd -P)
V=${V:-/mnt/vmstore/caraxes/bench-v1}
CH=${CH:-$ROOT/vendor/cloud-hypervisor/target/release/cloud-hypervisor}
NVRM=${NVRM:-$ROOT/target/bench/release/vhost-user-nvrm}
IMG=/mnt/vmstore/leandro/vm/guest-image
BASE=/mnt/vmstore/leandro/vm/guest-baked-20260801-610.57.04-20260820-105733.qcow2
KEY=/mnt/vmstore/leandro/vm/id_leandro
IP=${IP:-192.168.105.25}
TAP=${TAP:-caraxes5}
MAC=${MAC:-02:00:00:00:00:25}

gssh() {
    ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        -o LogLevel=ERROR -o ConnectTimeout=5 -o BatchMode=yes "leandro@$IP" "$@"
}

case ${1:?} in
prepare)
    mkdir -p "$V"
    [[ -f $V/rootfs.qcow2 ]] && { echo "$V/rootfs.qcow2 exists"; exit 1; }
    qemu-img create -q -f qcow2 -F qcow2 -b "$BASE" "$V/rootfs.qcow2"
    cp /mnt/vmstore/caraxes/v1check/seed.img "$V/seed.img"
    echo "prepared $V"
    ;;
up)
    ip link show "$TAP" >/dev/null || { echo "tap $TAP missing"; exit 1; }
    rm -f "$V/nvrm.sock"
    ( ulimit -n 65536
      setsid nohup "$NVRM" --nvrm "$V/nvrm.sock" >"$V/backend.log" 2>&1 </dev/null &
      echo $! >"$V/backend.pid" )
    for _ in $(seq 50); do [[ -S $V/nvrm.sock ]] && break; sleep 0.2; done
    [[ -S $V/nvrm.sock ]] || { echo "backend did not come up"; cat "$V/backend.log"; exit 1; }
    setsid nohup "$CH" --cpus boot=8 --memory size=4G,shared=on \
      --kernel "$IMG/vmlinuz" --initramfs "$IMG/initrd" \
      --cmdline 'root=/dev/vda1 rw console=ttyS0 net.ifnames=0' \
      --disk path="$V/rootfs.qcow2",image_type=qcow2,backing_files=on \
             path="$V/seed.img",readonly=on \
      --net tap="$TAP",mac="$MAC" \
      --generic-vhost-user "device_type=60,socket=$V/nvrm.sock,queue_sizes=[256,256]" \
      --serial file="$V/serial.log" --console off \
      >"$V/hypervisor.log" 2>&1 </dev/null &
    echo $! >"$V/hypervisor.pid"
    for _ in $(seq 90); do gssh true 2>/dev/null && break; sleep 2; done
    gssh true || { echo "no ssh"; tail -20 "$V/hypervisor.log"; exit 1; }
    echo "backend pid $(cat "$V/backend.pid"), hypervisor pid $(cat "$V/hypervisor.pid"), ssh up at $IP"
    ;;
modules)
    tar -C "$ROOT" -cf - --exclude=test guest-module/nvrm_nodes guest-module/virtio_nvrm | gssh 'rm -rf ~/guest-module && tar -xf - -C ~'
    # host read: the RM parameter file, as in the v1 procedure
    gssh 'mkdir -p ~/gpu && cat > ~/gpu/params.txt' < /proc/driver/nvidia/params
    gssh 'set -e; make -C ~/guest-module/nvrm_nodes >/dev/null; make -C ~/guest-module/virtio_nvrm >/dev/null
        sudo insmod ~/guest-module/nvrm_nodes/nvrm_nodes.ko create_nodes=0
        sudo ~/guest-module/nvrm_nodes/nvrm-nodes-tool provision params ~/gpu/params.txt
        sudo usermod -aG render,video leandro
        sudo insmod ~/guest-module/virtio_nvrm/virtio_nvrm.ko
        sudo dmesg | grep -E "virtio_nvrm" | tail -3; lsmod | grep -E "^(nvrm_nodes|virtio_nvrm) "'
    ;;
ssh) shift; gssh "$@" ;;
pids) echo "$(cat "$V/backend.pid") $(cat "$V/hypervisor.pid")" ;;
down)
    gssh sudo poweroff || true
    hp=$(cat "$V/hypervisor.pid")
    for _ in $(seq 60); do kill -0 "$hp" 2>/dev/null || break; sleep 1; done
    kill -0 "$hp" 2>/dev/null && { echo "hypervisor $hp still running"; exit 1; }
    bp=$(cat "$V/backend.pid")
    if kill -0 "$bp" 2>/dev/null && [[ $(readlink "/proc/$bp/exe") == "$NVRM" ]]; then
        kill "$bp"; for _ in $(seq 20); do kill -0 "$bp" 2>/dev/null || break; sleep 0.5; done
    fi
    echo "stopped (hypervisor $hp, backend $bp)"
    ;;
*) sed -n '2,/^set -euo/p' "$0" | sed '$d'; exit 2 ;;
esac

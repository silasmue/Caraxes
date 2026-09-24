#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Leandro v1 VM for the P13 benchmark (docs/caraxes/benchmark.md): vhost-user-nvrm plus the
# patched Cloud Hypervisor (v53.0 + patches/0001-0004) with --generic-vhost-user, guest modules
# nvrm_nodes and virtio_nvrm built in the guest. Same command lines as
# caraxes/runs/vhost-user-check-2026-09-23/vm.sh (patches/README.md "Manual VM check") without
# -v and LEA_DEBUG, 8 vCPUs, 4 GiB shared memory. v1's material (guest module sources, host
# binary, driver version) comes from a Leandro checkout (docs/caraxes/repo-split.md section 2.3),
# the guest image by driver version (docs/caraxes/v1-615-image.md).
#
#   bench-v1vm.sh [-n] prepare     new overlay on the base image + seed in $V
#   bench-v1vm.sh [-n] up          start the backend and the VM, wait for ssh
#   bench-v1vm.sh [-n] modules     copy guest-module/{nvrm_nodes,virtio_nvrm} from $LEA_V1_ROOT, build
#                                  them in the guest for its kernel, provision, insmod, check the nodes
#   bench-v1vm.sh ssh CMD          run CMD in the guest
#   bench-v1vm.sh pids             print "backend_pid vm_pid"
#   bench-v1vm.sh [-n] down        poweroff, stop the backend
#   bench-v1vm.sh config           print the resolved configuration and the checks, start nothing
# -n (or BENCH_V1_DRY_RUN=1): print the commands instead of running them (reads only).
#
# Driver: tools/lib/driver.sh on $LEA_V1_ROOT's DRIVER_VERSION; CARAXES_DRIVER=610|615 overrides.
# v1 forwards ioctls verbatim, so `up` and `modules` refuse when the running host driver
# (/proc/driver/nvidia/version, a read) is not that version.
# Environment (defaults for 615 / 610):
#   LEA_V1_ROOT  Leandro checkout: guest-module/, DRIVER_VERSION (default $ROOT/../Leandro;
#                2026-09-24: /mnt/vmstore/leandro/wt-615, branch port/615). LEA_V1_ROOT=$ROOT with
#                CARAXES_DRIVER=610 is the old in-tree 610 behaviour.
#   LEA_V1_BIN   directory of the host binaries (default $LEA_V1_ROOT/target/release;
#                2026-09-24: /mnt/vmstore/leandro/l615-host-bin-8c0d2b2)
#   NVRM         the backend binary (default $LEA_V1_BIN/vhost-user-nvrm; the old 610 bench
#                build is $ROOT/target/bench/release/vhost-user-nvrm)
#   LEA_V1_BASE  base image of `prepare`: $CARAXES_IMAGES/guest-v1-2026-09-24-615.71.09.qcow2 /
#                caraxes_driver_image of $LEA_VM_DIR (the baked 610.57.04 image)
#   LEA_V1_SEED  seed copied by `prepare`: the image's own seed (<image>.seed.img without .qcow2) /
#                /mnt/vmstore/caraxes/v1check/seed.img
#   V            VM state directory (/mnt/vmstore/caraxes/bench-v1-615.71.09 / /mnt/vmstore/caraxes/bench-v1)
#   LEA_MANAGED_COMPAT  host switch of the backend, default 1 as in Leandro-Test's gpu gate
#                (managed memory works); LEA_MANAGED_COMPAT= (empty) or 0 starts it without.
#                Other LEA_* host switches are inherited by the backend unchanged.
#   IP (192.168.105.25), TAP (caraxes5), MAC, CH (default $ROOT/vendor/cloud-hypervisor/target/
#   release/cloud-hypervisor), LEA_V1_KERNEL_DIR (/mnt/vmstore/leandro/vm/guest-image),
#   LEA_V1_KEY (/mnt/vmstore/leandro/vm/id_leandro), LEA_VM_DIR (/mnt/vmstore/leandro/vm).
set -euo pipefail
ROOT=$(cd "$(dirname "$(readlink -f "$0")")/../.." && pwd -P)
DRY=${BENCH_V1_DRY_RUN:-0}
if [[ ${1:-} == -n ]]; then DRY=1; shift; fi

die() { printf 'bench-v1vm: ERROR: %s\n' "$*" >&2; exit 1; }
note() { printf 'bench-v1vm: %s\n' "$*" >&2; }
# run CMD...: execute, or print it in dry-run mode
run() {
    if [[ $DRY == 1 ]]; then printf '+'; printf ' %q' "$@"; printf '\n'; else "$@"; fi
}

LEA_V1_ROOT=${LEA_V1_ROOT:-$ROOT/../Leandro}
[[ -d $LEA_V1_ROOT ]] || die "LEA_V1_ROOT=$LEA_V1_ROOT is not a directory (set it to the Leandro checkout, e.g. /mnt/vmstore/leandro/wt-615)"
LEA_V1_ROOT=$(cd "$LEA_V1_ROOT" && pwd -P)

# shellcheck source=../../tools/lib/driver.sh
source "$ROOT/tools/lib/driver.sh"
LEA_ROOT=$LEA_V1_ROOT caraxes_driver_select || die "driver selection failed (LEA_V1_ROOT=$LEA_V1_ROOT)"
DRV=$CARAXES_DRIVER_VERSION
LEA_VM_DIR=${LEA_VM_DIR:-/mnt/vmstore/leandro/vm}
CARAXES_IMAGES=${CARAXES_IMAGES:-/mnt/vmstore/caraxes/images}
case $CARAXES_DRIVER in
    610) def_base=$(caraxes_driver_image "$LEA_VM_DIR")
         def_seed=/mnt/vmstore/caraxes/v1check/seed.img
         def_v=/mnt/vmstore/caraxes/bench-v1 ;;
    615) def_base=$CARAXES_IMAGES/guest-v1-2026-09-24-615.71.09.qcow2
         def_seed=${def_base%.qcow2}.seed.img
         def_v=/mnt/vmstore/caraxes/bench-v1-$DRV ;;
esac
LEA_V1_BASE=${LEA_V1_BASE:-$def_base}
LEA_V1_SEED=${LEA_V1_SEED:-$def_seed}
V=${V:-$def_v}
LEA_V1_BIN=${LEA_V1_BIN:-$LEA_V1_ROOT/target/release}
NVRM=${NVRM:-$LEA_V1_BIN/vhost-user-nvrm}
CH=${CH:-$ROOT/vendor/cloud-hypervisor/target/release/cloud-hypervisor}
IMG=${LEA_V1_KERNEL_DIR:-$LEA_VM_DIR/guest-image}
KEY=${LEA_V1_KEY:-$LEA_VM_DIR/id_leandro}
IP=${IP:-192.168.105.25}
TAP=${TAP:-caraxes5}
MAC=${MAC:-02:00:00:00:00:25}
LEA_MANAGED_COMPAT=${LEA_MANAGED_COMPAT-1}

gssh() {
    ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        -o LogLevel=ERROR -o ConnectTimeout=5 -o BatchMode=yes "leandro@$IP" "$@"
}

# The running host driver, parsed as nvrm-sys' running_driver_version (version.rs:17-23).
host_driver() {
    awk '{ for (i = 1; i <= NF; i++) if ($i ~ /^[0-9]+\.[0-9]+\.[0-9]+$/) { print $i; exit } }' \
        /proc/driver/nvidia/version 2>/dev/null
}
check_host() {
    local run; run=$(host_driver) || true
    [[ -n $run ]] || die "no NVIDIA host driver (/proc/driver/nvidia/version not readable)"
    [[ $run == "$DRV" ]] && return 0
    local msg="host driver $run, v1 selection $DRV (LEA_V1_ROOT/DRIVER_VERSION or CARAXES_DRIVER): v1 forwards ioctls verbatim, guest module and guest userspace must be the host's version"
    [[ $DRY == 1 ]] && { note "WARNING: $msg"; return 0; }
    die "$msg"
}
# The guest module's wire version (nvrm_wire.h NVRM_DRIVER_VERSION) must be the selected one.
wire_version() {
    sed -n 's/^#define[[:space:]]*NVRM_DRIVER_VERSION[[:space:]]*"\([^"]*\)".*/\1/p' \
        "$LEA_V1_ROOT/guest-module/virtio_nvrm/nvrm_wire.h" 2>/dev/null
}
check_sources() {
    local d w
    for d in nvrm_nodes virtio_nvrm; do
        [[ -f $LEA_V1_ROOT/guest-module/$d/Makefile ]] || die "$LEA_V1_ROOT/guest-module/$d/Makefile missing"
    done
    w=$(wire_version)
    [[ $w == "$DRV" ]] || die "$LEA_V1_ROOT/guest-module/virtio_nvrm/nvrm_wire.h says NVRM_DRIVER_VERSION \"$w\", the selection is $DRV"
}
# The overlay in $V must sit on the selected base image (a 610 guest must not meet a 615 host).
check_overlay() {
    [[ -f $V/rootfs.qcow2 ]] || { [[ $DRY == 1 ]] && return 0; die "$V/rootfs.qcow2 missing: run prepare"; }
    local b
    b=$(qemu-img info -U --output=json "$V/rootfs.qcow2" | jq -r '."full-backing-filename" // ."backing-filename" // ""')
    [[ $(readlink -f "$b") == "$(readlink -f "$LEA_V1_BASE")" ]] && return 0
    local msg="$V/rootfs.qcow2 is an overlay on '${b:-nothing}', the selected base is $LEA_V1_BASE (prepare a new V, or set LEA_V1_BASE)"
    [[ $DRY == 1 ]] && { note "WARNING: $msg"; return 0; }
    die "$msg"
}
# After the start: the binary's own report. nvrm.rs serve() prints "driver X ABI" after
# detect() (nvrm-sys version.rs:48) and exits before binding the socket when X is not built in.
check_backend_log() {
    local log=$V/backend.log abi run
    run=$(host_driver)
    if grep -q "is not supported" "$log" 2>/dev/null; then
        die "$NVRM does not serve the running host driver $run: $(grep -m1 'is not supported' "$log")"
    fi
    abi=$(sed -n 's/.*vhost-user-nvrm: driver \([0-9.]*\) ABI.*/\1/p' "$log" | head -1)
    if [[ -z $abi ]]; then
        note "WARNING: $log has no 'driver X ABI' line (an older build); the running driver is $run"
    elif [[ $abi != "$run" ]]; then
        die "$NVRM selected the $abi ABI, the host runs $run"
    fi
}

config() {
    printf '%-20s %s\n' driver "$CARAXES_DRIVER ($DRV)$([[ ${CARAXES_DRIVER_EXPLICIT:-} == 1 ]] && echo ', CARAXES_DRIVER' || echo ", $LEA_V1_ROOT/DRIVER_VERSION")" \
        host-driver "$(host_driver || true)" LEA_V1_ROOT "$LEA_V1_ROOT" nvrm_wire.h "$(wire_version)" \
        LEA_V1_BASE "$LEA_V1_BASE" LEA_V1_SEED "$LEA_V1_SEED" V "$V" NVRM "$NVRM" CH "$CH" \
        kernel "$IMG" LEA_MANAGED_COMPAT "${LEA_MANAGED_COMPAT:-(off)}" guest "leandro@$IP tap $TAP mac $MAC"
    local f
    for f in "$LEA_V1_BASE" "$LEA_V1_SEED" "$NVRM" "$CH" "$IMG/vmlinuz" "$IMG/initrd" "$KEY"; do
        [[ -e $f ]] || note "WARNING: $f does not exist"
    done
}

case ${1:-} in
config) config ;;
prepare)
    [[ -f $LEA_V1_BASE ]] || die "base image $LEA_V1_BASE missing"
    [[ -f $LEA_V1_SEED ]] || die "seed $LEA_V1_SEED missing"
    [[ -f $V/rootfs.qcow2 ]] && die "$V/rootfs.qcow2 exists"
    run mkdir -p "$V"
    run qemu-img create -q -f qcow2 -F qcow2 -b "$LEA_V1_BASE" "$V/rootfs.qcow2"
    run install -m 644 "$LEA_V1_SEED" "$V/seed.img"
    echo "prepared $V (base $LEA_V1_BASE, driver $DRV)"
    ;;
up)
    check_host
    check_overlay
    [[ -x $NVRM ]] || [[ $DRY == 1 ]] || die "backend $NVRM missing (set NVRM or LEA_V1_BIN)"
    [[ -x $CH ]] || [[ $DRY == 1 ]] || die "cloud-hypervisor $CH missing (set CH)"
    ip link show "$TAP" >/dev/null 2>&1 || { [[ $DRY == 1 ]] && note "WARNING: tap $TAP missing"; } || die "tap $TAP missing"
    nvrm_abs=$(readlink -f "$NVRM" || printf '%s' "$NVRM")
    backend=(env "LEA_MANAGED_COMPAT=$LEA_MANAGED_COMPAT" setsid nohup "$nvrm_abs" --nvrm "$V/nvrm.sock")
    vmm=(setsid nohup "$CH" --cpus boot=8 --memory size=4G,shared=on
         --kernel "$IMG/vmlinuz" --initramfs "$IMG/initrd"
         --cmdline 'root=/dev/vda1 rw console=ttyS0 net.ifnames=0'
         --disk path="$V/rootfs.qcow2",image_type=qcow2,backing_files=on
                path="$V/seed.img",readonly=on
         --net tap="$TAP",mac="$MAC"
         --generic-vhost-user "device_type=60,socket=$V/nvrm.sock,queue_sizes=[256,256]"
         --serial file="$V/serial.log" --console off)
    if [[ $DRY == 1 ]]; then
        run rm -f "$V/nvrm.sock"
        printf '+ ( ulimit -n 65536;'; printf ' %q' "${backend[@]}"; printf ' >%q 2>&1 </dev/null & )\n' "$V/backend.log"
        printf '+'; printf ' %q' "${vmm[@]}"; printf ' >%q 2>&1 </dev/null &\n' "$V/hypervisor.log"
        echo "+ wait for ssh leandro@$IP"
        exit 0
    fi
    rm -f "$V/nvrm.sock"
    ( ulimit -n 65536
      "${backend[@]}" >"$V/backend.log" 2>&1 </dev/null &
      echo $! >"$V/backend.pid" )
    printf '%s\n' "$nvrm_abs" >"$V/backend.bin"
    printf 'LEA_MANAGED_COMPAT=%s\n' "$LEA_MANAGED_COMPAT" >"$V/backend.env"
    for _ in $(seq 50); do
        [[ -S $V/nvrm.sock ]] && break
        kill -0 "$(cat "$V/backend.pid")" 2>/dev/null || break
        sleep 0.2
    done
    check_backend_log
    [[ -S $V/nvrm.sock ]] || { cat "$V/backend.log"; die "backend did not come up"; }
    "${vmm[@]}" >"$V/hypervisor.log" 2>&1 </dev/null &
    echo $! >"$V/hypervisor.pid"
    for _ in $(seq 90); do gssh true 2>/dev/null && break; sleep 2; done
    gssh true || { tail -20 "$V/hypervisor.log"; die "no ssh"; }
    echo "backend pid $(cat "$V/backend.pid") ($(grep -m1 'driver .* ABI' "$V/backend.log" || echo 'no ABI line'), LEA_MANAGED_COMPAT=${LEA_MANAGED_COMPAT:-off}), hypervisor pid $(cat "$V/hypervisor.pid"), ssh up at $IP"
    ;;
modules)
    check_host
    check_sources
    # The guest part, as run by hand on 2026-09-24 (caraxes/runs/cutover-615-2026-09-24/v1/commands.txt
    # section 4) in the order of Leandro-Test's lea_guest_build_nvrm (scripts/lib/provision.sh:1157-1225):
    # build for the running guest kernel, check vermagic and the wire version, reload idempotently,
    # provision the params, virtio_nvrm creates /dev/nvidia*.
    guest_script='set -e
        lsmod | grep -qE "^nvidia(_uvm|_modeset|_drm)? " && { echo "ERROR: the guest has its own nvidia module loaded (image without the v1 blacklist?)"; exit 1; }
        make -C ~/guest-module/nvrm_nodes >/dev/null; make -C ~/guest-module/virtio_nvrm >/dev/null
        k=$(uname -r); m=$(modinfo -F vermagic ~/guest-module/virtio_nvrm/virtio_nvrm.ko | cut -d" " -f1)
        [ "$m" = "$k" ] || { echo "ERROR: virtio_nvrm.ko vermagic $m, guest kernel $k"; exit 1; }
        grep -qaF "'"$DRV"'" ~/guest-module/virtio_nvrm/virtio_nvrm.ko || { echo "ERROR: virtio_nvrm.ko does not carry '"$DRV"'"; exit 1; }
        lsmod | grep -q "^virtio_nvrm " && sudo rmmod virtio_nvrm
        lsmod | grep -q "^nvrm_nodes " && sudo rmmod nvrm_nodes
        sudo insmod ~/guest-module/nvrm_nodes/nvrm_nodes.ko create_nodes=0
        sudo ~/guest-module/nvrm_nodes/nvrm-nodes-tool provision params ~/gpu/params.txt
        sudo usermod -aG render,video leandro
        sudo insmod ~/guest-module/virtio_nvrm/virtio_nvrm.ko
        echo "built for $k: virtio_nvrm.ko carries '"$DRV"'"
        sudo dmesg | grep -E "virtio_nvrm" | tail -3; lsmod | grep -E "^(nvrm_nodes|virtio_nvrm) "
        test -c /dev/nvidiactl || { echo "ERROR: /dev/nvidiactl missing"; exit 1; }
        ls -l /dev/nvidiactl /dev/nvidia0 /dev/nvidia-uvm 2>&1 | sed "s/^/  /"'
    if [[ $DRY == 1 ]]; then
        printf '+ tar -C %q -cf - --exclude=test guest-module/nvrm_nodes guest-module/virtio_nvrm | ssh leandro@%s %q\n' \
            "$LEA_V1_ROOT" "$IP" 'rm -rf ~/guest-module && tar -xf - -C ~'
        printf '+ ssh leandro@%s %q < /proc/driver/nvidia/params\n' "$IP" 'mkdir -p ~/gpu && cat > ~/gpu/params.txt'
        printf '+ ssh leandro@%s %q\n' "$IP" "$guest_script"
        exit 0
    fi
    tar -C "$LEA_V1_ROOT" -cf - --exclude=test guest-module/nvrm_nodes guest-module/virtio_nvrm | gssh 'rm -rf ~/guest-module && tar -xf - -C ~'
    # host read: the RM parameter file, as in the v1 procedure
    gssh 'mkdir -p ~/gpu && cat > ~/gpu/params.txt' < /proc/driver/nvidia/params
    gssh "$guest_script"
    ;;
ssh) shift; gssh "$@" ;;
pids) echo "$(cat "$V/backend.pid") $(cat "$V/hypervisor.pid")" ;;
down)
    if [[ $DRY == 1 ]]; then
        echo "+ ssh leandro@$IP sudo poweroff; wait for $V/hypervisor.pid; kill $V/backend.pid if its exe is $(cat "$V/backend.bin" 2>/dev/null || readlink -f "$NVRM")"
        exit 0
    fi
    gssh sudo poweroff || true
    hp=$(cat "$V/hypervisor.pid")
    for _ in $(seq 60); do kill -0 "$hp" 2>/dev/null || break; sleep 1; done
    kill -0 "$hp" 2>/dev/null && die "hypervisor $hp still running"
    bp=$(cat "$V/backend.pid")
    bin=$(cat "$V/backend.bin" 2>/dev/null || readlink -f "$NVRM")
    if kill -0 "$bp" 2>/dev/null && [[ $(readlink "/proc/$bp/exe") == "$bin" ]]; then
        kill "$bp"; for _ in $(seq 20); do kill -0 "$bp" 2>/dev/null || break; sleep 0.5; done
    fi
    echo "stopped (hypervisor $hp, backend $bp)"
    ;;
*) sed -n '2,/^set -euo/p' "$0" | sed '$d'; exit 2 ;;
esac

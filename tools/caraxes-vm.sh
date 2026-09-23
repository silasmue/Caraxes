#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Run one Caraxes guest: the synthetic GPU (vfio-user-nvgpu) plus Cloud Hypervisor
# attaching it with --user-device. Experiment ladder: docs/caraxes/experiments.md.
# Usage: tools/caraxes-vm.sh up|down|status|wait|ssh [command...]|serial|probe [label]
#        |uvm [label] [flags]|dma|name
# `probe` reloads nvidia.ko in the guest and saves the guest log and the device trace
# of that window under $W/probe-<label>/. `uvm` (experiment E5a, docs/caraxes/uvm.md)
# reloads nvidia-uvm with debug prints, runs crates/vfio-user-nvgpu/guest/uvm-register.c
# (UVM_INITIALIZE with <flags>, default 0, then UVM_REGISTER_GPU) and saves both logs
# under $W/uvm-<label>/; CARAXES_UVM_PARAMS adds nvidia-uvm module parameters (for
# example uvm_disable_hmm=1). CARAXES_VBIOS selects the image served
# through the PROM window (default: caraxes/reference/rtx2070/synth-vbios.bin when
# present, else the PCI-visible rom.bin). CARAXES_BOOT_MODEL=1 attaches the E2 boot
# model; CARAXES_FB_MIB sets the framebuffer size the guest is told; CARAXES_HOST_RM=1
# lets the device open the host RM and forward the forwardable RPCs.
# CARAXES_NAME_FORMAT=transport names the card "Leandro VFIO RTX 2070-4G" instead of the
# default "Leandro RTX 2070-4G" (nvrm_abi::naming, docs/caraxes/identity-profile.md);
# `name` prints the guest-visible name the device presents with the current settings.
# E5b part 1 (docs/caraxes/memory-model.md): CARAXES_RESERVE_VRAM=1 makes the device hold
# the profile's size of host VRAM at a fixed address for its life (refused when the host
# would keep less than 1 GiB free); CARAXES_DMA=register registers guest RAM with the host
# RM and publishes the IOVA table in $W/dma-ranges (the device then needs CAP_SYS_ADMIN
# permitted: a copy of the binary in $W gets the file capability cap_sys_admin+p through
# `sudo -n setcap`), CARAXES_DMA=identity publishes an identity table. `dma` (also run by
# `probe` and `uvm`) loads the guest module caraxes_dma_range with that table before
# nvidia binds (guest-module/caraxes_dma_range/README.md); the guest keeps nvidia
# blacklisted from autoloading so that it never binds without the table.
# CARAXES_DISPLAY=1 presents the display engine (--display, docs/caraxes/display-ab.md).
# E5b part 2 (docs/caraxes/execution.md): CARAXES_HOST_EXEC=1 executes on the physical GPU
# (the FB is the reserved VRAM, guest channels get host mirror channels, engine non-stall
# events become guest interrupts); it needs the three flags above.
# State lives under $CARAXES_STORE/$VM (default /mnt/vmstore/caraxes/vm0, falling
# back to target/caraxes/$VM when the store is absent): rootfs.qcow2 (overlay on
# LEA_BASE_IMAGE), seed.img (cloud-init), gpu.sock, gpu-trace.log, serial.log.
# The host tap (default caraxes0, 192.168.100.1/24) must exist and be owned by the user:
#   sudo ip tuntap add dev caraxes0 mode tap user $USER
#   sudo ip addr add 192.168.100.1/24 dev caraxes0 && sudo ip link set caraxes0 up
set -euo pipefail

LEA_ROOT=$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd -P)
# shellcheck disable=SC1091
[[ -f $LEA_ROOT/local.env ]] && source "$LEA_ROOT/local.env"
: "${LEA_VM_DIR:=/mnt/vmstore/leandro/vm}"
: "${LEA_CH:=/mnt/vmstore/leandro/cloud-hypervisor/bin/cloud-hypervisor}"
: "${LEA_BASE_IMAGE:=$LEA_VM_DIR/guest-baked-20260801-610.57.04-20260820-105733.qcow2}"
VM=${VM:-vm0}
if [[ -z ${CARAXES_STORE:-} ]]; then
    if [[ -d /mnt/vmstore/caraxes ]]; then CARAXES_STORE=/mnt/vmstore/caraxes; else CARAXES_STORE=$LEA_ROOT/target/caraxes; fi
fi
W=$CARAXES_STORE/$VM
IP=${CARAXES_IP:-192.168.100.20}
TAP=${CARAXES_TAP:-caraxes0}
MAC=${CARAXES_MAC:-02:00:00:00:00:20}
KERNEL=${CARAXES_KERNEL:-$LEA_VM_DIR/guest-image/vmlinuz}
INITRD=${CARAXES_INITRD:-$LEA_VM_DIR/guest-image/initrd}
SSH_KEY=${CARAXES_SSH_KEY:-$LEA_VM_DIR/id_leandro}
SERVER=$LEA_ROOT/target/release/vfio-user-nvgpu
# The settings the guest-visible identity follows (`up` and `name` pass the same ones).
IDENTITY=()
[[ -n ${CARAXES_FB_MIB:-} ]] && IDENTITY+=(--fb-mib "$CARAXES_FB_MIB")
[[ -n ${CARAXES_NAME_FORMAT:-} ]] && IDENTITY+=(--name-format "$CARAXES_NAME_FORMAT")
REF=$LEA_ROOT/caraxes/reference/rtx2070
if [[ -z ${CARAXES_VBIOS:-} && -f $REF/synth-vbios.bin ]]; then CARAXES_VBIOS=$REF/synth-vbios.bin; fi

error() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
# Guest shell snippet: unload the named modules in order when loaded; exit 3 with a
# message when one refuses (nvidia_uvm holds nvidia, so it goes first).
UNLOAD='unload() { for m in "$@"; do if lsmod | grep -q "^$m "; then sudo rmmod "$m" || { echo "ERROR: rmmod $m failed (in use?)"; lsmod | grep -E "^(nvidia|caraxes)"; exit 3; }; fi; done; }'
alive() { [[ -f $1 ]] && kill -0 "$(cat "$1")" 2>/dev/null; }

do_ssh() {
    ssh -i "$SSH_KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        -o LogLevel=ERROR -o ConnectTimeout=5 "leandro@$IP" "$@"
}

up() {
    [[ -x $SERVER ]] || error "$SERVER missing: cargo build --release -p vfio-user-nvgpu"
    [[ -x $LEA_CH ]] || error "cloud-hypervisor missing at $LEA_CH"
    [[ -f $W/rootfs.qcow2 && -f $W/seed.img ]] || error "$W needs rootfs.qcow2 and seed.img"
    [[ -f $REF/rom.bin ]] || error "$REF/rom.bin missing (git-ignored capture of the card's ROM)"
    ip link show "$TAP" >/dev/null 2>&1 || error "tap $TAP missing; see the header of this script"
    alive "$W/gpu.pid" && error "server already running (pid $(cat "$W/gpu.pid"))"
    alive "$W/ch.pid" && error "VM already running (pid $(cat "$W/ch.pid"))"
    rm -f "$W/gpu.sock" "$W/ch.sock"

    local vbios=()
    [[ -n ${CARAXES_VBIOS:-} ]] && vbios=(--vbios "$CARAXES_VBIOS")
    [[ ${CARAXES_BOOT_MODEL:-0} == 1 ]] && vbios+=(--boot-model)
    vbios+=("${IDENTITY[@]}")
    [[ ${CARAXES_HOST_RM:-0} == 1 ]] && vbios+=(--host-rm)
    [[ ${CARAXES_RESERVE_VRAM:-0} == 1 ]] && vbios+=(--reserve-vram)
    [[ ${CARAXES_HOST_EXEC:-0} == 1 ]] && vbios+=(--host-exec)
    [[ ${CARAXES_DISPLAY:-0} == 1 ]] && vbios+=(--display)
    local server=$SERVER
    rm -f "$W/dma-ranges"
    case ${CARAXES_DMA:-} in
        '') ;;
        register) vbios+=(--register-guest-ram --dma-ranges "$W/dma-ranges"); server=$(privileged_server) ;;
        identity) vbios+=(--identity-dma-table --dma-ranges "$W/dma-ranges") ;;
        *) error "CARAXES_DMA must be register or identity" ;;
    esac
    "$server" --socket "$W/gpu.sock" --trace "$W/gpu-trace.log" \
        --config "$REF/config-space.txt" --rom "$REF/rom.bin" "${vbios[@]}" > "$W/gpu.log" 2>&1 &
    echo $! > "$W/gpu.pid"
    for _ in $(seq 50); do [[ -S $W/gpu.sock ]] && break; sleep 0.1; done
    [[ -S $W/gpu.sock ]] || { cat "$W/gpu.log"; error "server did not create $W/gpu.sock"; }

    "$LEA_CH" --api-socket "$W/ch.sock" \
        --cpus boot="${CARAXES_CPUS:-8}" --memory size="${CARAXES_MEM:-4G}",shared=on \
        --kernel "$KERNEL" --initramfs "$INITRD" \
        --cmdline 'root=/dev/vda1 rw console=ttyS0 net.ifnames=0' \
        --disk path="$W/rootfs.qcow2",image_type=qcow2,backing_files=on \
               path="$W/seed.img",readonly=on \
        --net tap="$TAP",mac="$MAC" \
        --user-device socket="$W/gpu.sock",id=gpu0 \
        --serial file="$W/serial.log" --console off \
        > "$W/ch.log" 2>&1 &
    echo $! > "$W/ch.pid"
    if [[ -n ${CARAXES_DMA:-} ]]; then
        # The device rewrites the table at each DMA_MAP (one per memory zone); wait until
        # it covers all guest RAM.
        local want got
        want=$(mem_bytes "${CARAXES_MEM:-4G}")
        for _ in $(seq 300); do
            got=$(table_bytes)
            (( got == want )) && break
            alive "$W/gpu.pid" || { tail -n 20 "$W/gpu.log"; error "server exited"; }
            alive "$W/ch.pid" || { tail -n 20 "$W/ch.log"; error "VM exited"; }
            sleep 0.2
        done
        (( got == want )) || error "DMA table in $W/dma-ranges covers $got of $want bytes"
        echo "dma table: $(cat "$W/dma-ranges")"
    fi
    echo "server pid $(cat "$W/gpu.pid"), VM pid $(cat "$W/ch.pid"); logs in $W"
}

# Bytes of a Cloud Hypervisor memory size (4G, 4096M).
mem_bytes() {
    local v=${1%[GgMm]}
    case ${1: -1} in G|g) echo $(( v << 30 )) ;; M|m) echo $(( v << 20 )) ;; *) echo "$1" ;; esac
}

# Guest RAM the published table covers (the sum of its sizes).
table_bytes() {
    local sum=0 e
    [[ -s $W/dma-ranges ]] || { echo 0; return; }
    for e in $(tr ',' ' ' < "$W/dma-ranges"); do sum=$(( sum + 0x${e##*:} )); done
    echo "$sum"
}

# D12: the device may raise CAP_SYS_ADMIN only around GET_SURFACE_PHYS_ATTR. It gets
# the capability as a file capability, permitted but not effective, on a copy of the
# binary in the state directory; the process runs as the user throughout and the build
# tree keeps an unprivileged binary. Prints the copy's path.
privileged_server() {
    local copy=$W/vfio-user-nvgpu
    if ! cmp -s "$SERVER" "$copy" || ! getcap "$copy" | grep -q cap_sys_admin; then
        install -m 0755 "$SERVER" "$copy"
        sudo -n setcap cap_sys_admin+p "$copy" >&2 || error "sudo -n setcap cap_sys_admin+p $copy failed"
    fi
    getcap "$copy" | grep -q 'cap_sys_admin=p' || error "$copy lacks cap_sys_admin=p"
    echo "$copy"
}

# Install the device's DMA table in the guest (caraxes_dma_range) before nvidia binds.
# Reloads the module only when the table changed; then nvidia is unloaded first and
# the device gets a function-level reset, so that the next modprobe starts clean.
dma_load() {
    [[ -s $W/dma-ranges ]] || return 0
    local ranges pool lo hi
    ranges=$(cat "$W/dma-ranges")
    # With 64-bit host IOVAs (every entry at or above 2^40), the guest's sysmem flush
    # buffer, which must sit below 2^40 and is bounced through swiotlb, needs the pool
    # aliased low (dmamap.rs `with_alias`, docs/caraxes/memory-model.md §2).
    if grep -qE ':[0-9a-f]{11,}:' "$W/dma-ranges"; then
        pool=$(do_ssh "sudo journalctl -k -b | grep -o 'software IO TLB: mapped \[mem 0x[0-9a-f]*-0x[0-9a-f]*\]' | tail -n 1")
        [[ $pool =~ 0x([0-9a-f]+)-0x([0-9a-f]+) ]] || error "no swiotlb pool in the guest's kernel log"
        lo=${BASH_REMATCH[1]} hi=${BASH_REMATCH[2]}
        ranges=$("$SERVER" --dma-table-alias "$W/dma-ranges" "$lo" "$(printf %x $(( 0x$hi - 0x$lo )))") \
            || error "cannot alias the swiotlb pool $pool"
        echo "dma table: swiotlb pool [0x$lo, 0x$hi) aliased below 2^40"
    fi
    echo "$ranges" > "$W/dma-ranges.guest"
    if ! do_ssh 'test -f caraxes_dma_range/caraxes_dma_range.ko'; then
        tar -C "$LEA_ROOT/guest-module" -cf - caraxes_dma_range | do_ssh 'rm -rf caraxes_dma_range && tar xf - && make -C caraxes_dma_range >/dev/null'
    fi
    do_ssh "P=/sys/module/caraxes_dma_range/parameters
        if [ -r \$P/ranges ] && [ \"\$(cat \$P/ranges)\" = '$ranges' ]; then echo \"dma table: installed (\$(cat \$P/status))\"; exit 0; fi
        $UNLOAD; unload nvidia_uvm nvidia caraxes_dma_range
        sudo insmod caraxes_dma_range/caraxes_dma_range.ko ranges='$ranges' || exit 1
        for dev in /sys/bus/pci/devices/*; do [ \"\$(cat \$dev/vendor)\" = 0x10de ] && echo 1 | sudo tee \$dev/reset >/dev/null; done
        echo \"dma table: loaded (\$(cat \$P/status))\"; sudo dmesg | grep caraxes_dma_range | tail -n 8"
}

down() {
    # Stop the VM first; the server then sees the socket close, prints its access
    # summary into gpu.log and exits on its own. Kill it only if it does not.
    if alive "$W/ch.pid"; then kill "$(cat "$W/ch.pid")" 2>/dev/null || true; fi
    for _ in $(seq 50); do alive "$W/gpu.pid" || break; sleep 0.1; done
    if alive "$W/gpu.pid"; then kill "$(cat "$W/gpu.pid")" 2>/dev/null || true; fi
    rm -f "$W/ch.pid" "$W/gpu.pid"
    echo "stopped"
}

status() {
    for p in gpu ch; do
        if alive "$W/$p.pid"; then echo "$p: running (pid $(cat "$W/$p.pid"))"; else echo "$p: not running"; fi
    done
    [[ -f $W/ch.log ]] && { echo "-- ch.log"; tail -n 5 "$W/ch.log"; }
    [[ -f $W/gpu.log ]] && { echo "-- gpu.log"; tail -n 5 "$W/gpu.log"; }
    [[ -f $W/gpu-trace.log ]] && echo "-- trace: $(wc -l < "$W/gpu-trace.log") lines"
}

wait_ssh() {
    for _ in $(seq "${1:-90}"); do
        alive "$W/ch.pid" || { tail -n 20 "$W/ch.log"; error "VM exited"; }
        do_ssh -o ConnectTimeout=2 true 2>/dev/null && { echo "ssh up at $IP"; return 0; }
        sleep 2
    done
    error "no ssh at $IP"
}

# Reload the guest module against the device and keep both logs of that window.
probe() {
    local label=${1:-$(date +%Y%m%d-%H%M%S)} out
    out=$W/probe-$label
    mkdir -p "$out"
    alive "$W/gpu.pid" || error "server not running"
    wc -l < "$W/gpu-trace.log" > "$out/trace-start"
    dma_load > "$out/dma.txt" 2>&1 || { cat "$out/dma.txt"; error "DMA table not installed"; }
    # A function-level reset puts the device model back to its initial state, so
    # every probe starts from the same point (the driver itself asks for a reset
    # after a failed boot).
    # nvidia_uvm (loaded by nvidia-smi or a CUDA program) holds nvidia: unload both, and
    # stop when either refuses, instead of probing the old driver instance.
    do_ssh "$UNLOAD"'; unload nvidia_uvm nvidia; for dev in /sys/bus/pci/devices/*; do [ "$(cat $dev/vendor)" = 0x10de ] && echo 1 | sudo tee $dev/reset >/dev/null; done; sudo dmesg -C; sudo modprobe nvidia; rc=$?; sleep 2; sudo dmesg | grep -v "NVRM: VM:"; echo "modprobe rc=$rc"; ls -la /dev/nvidia* 2>/dev/null; true' \
        > "$out/guest-dmesg.txt" 2>&1 || { grep ERROR "$out/guest-dmesg.txt"; error "probe $label: the guest driver could not be unloaded"; }
    tail -n +"$(( $(cat "$out/trace-start") + 1 ))" "$W/gpu-trace.log" > "$out/device-trace.txt"
    echo "probe $label: $(wc -l < "$out/device-trace.txt") device accesses, guest log in $out/guest-dmesg.txt"
    grep -E "RmInitAdapter|kgsp|Xid|modprobe rc" "$out/guest-dmesg.txt" | tail -n 8
}

# Register the GPU with nvidia-uvm from a small client and keep both logs of that window.
uvm() {
    local label=${1:-$(date +%Y%m%d-%H%M%S)} flags=${2:-0} out start
    out=$W/uvm-$label
    mkdir -p "$out"
    alive "$W/gpu.pid" || error "server not running"
    start=$(wc -l < "$W/gpu-trace.log")
    dma_load > "$out/dma.txt" 2>&1 || { cat "$out/dma.txt"; error "DMA table not installed"; }
    do_ssh 'cat > uvm-register.c' < "$LEA_ROOT/crates/vfio-user-nvgpu/guest/uvm-register.c"
    # The guest image has no nvidia-modprobe, so the device nodes are made here; the
    # client holds /dev/nvidia0 open itself (nv_gpu_ops.c:7671-7674).
    do_ssh "cc -O2 -o uvm-register uvm-register.c && sudo dmesg -C
        sudo rmmod nvidia-uvm 2>/dev/null; sudo modprobe nvidia-uvm uvm_debug_prints=1 ${CARAXES_UVM_PARAMS:-}
        [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
        [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
        sudo rm -f /dev/nvidia-uvm; sudo mknod -m 666 /dev/nvidia-uvm c \$(grep nvidia-uvm /proc/devices | cut -d' ' -f1) 0
        timeout 120 ./uvm-register --flags $flags; echo \"uvm-register rc=\$?\"
        sudo dmesg | grep -vE 'NVRM: (VM:|ioctl)'" > "$out/guest.txt" 2>&1 || true
    tail -n +"$(( start + 1 ))" "$W/gpu-trace.log" > "$out/device-trace.txt"
    echo "uvm $label: $(grep -c 'pushbuffer va' "$out/device-trace.txt") pushes executed, logs in $out"
    grep -E "UVM_|rc=|nvidia-uvm:" "$out/guest.txt" | head -n 12
}

# Guest-side test material (docs/caraxes/testing.md). Leandro-Test is read, never written.
LEA_TEST=${CARAXES_LEANDRO_TEST:-$(cd "$LEA_ROOT/.." && pwd -P)/Leandro-Test}
GUEST_TESTS=${CARAXES_GUEST_TESTS:-/home/leandro/caraxes-tests}
# The CUDA driver-API header ships in the baked image's torch venv (nvidia/cu13); libcuda
# is the 610.57.04 userspace in /opt/nvrm/lib. The image has no CUDA toolkit and no nvcc.
GUEST_VENV=${CARAXES_GUEST_VENV:-/home/leandro/gpu/venv}
# The probes that build without Vulkan, EGL or OpenCL headers (probe/Makefile targets).
GUEST_PROBES=(nvprobe managedprobe hostregprobe oomprobe vrampress rmdirect ctrlping ioctlping)

# Copy probe/, the guest scripts and the matrix library of Leandro-Test into the guest,
# keeping Leandro-Test's own layout under $GUEST_TESTS/leandro-test so its scripts find
# each other unmodified; add the Caraxes guest runner; build the C probes in the guest;
# record what the guest has. Idempotent (rsync, make).
sync_tests() {
    [[ -f $LEA_TEST/probe/Makefile ]] || error "no Leandro-Test checkout at $LEA_TEST (CARAXES_LEANDRO_TEST)"
    command -v rsync >/dev/null || error "rsync missing on the host"
    do_ssh true || error "guest $IP not reachable (tools/caraxes-vm.sh up && tools/caraxes-vm.sh wait)"
    do_ssh 'command -v rsync >/dev/null' || error "rsync missing in the guest"
    local rsh="ssh -i $SSH_KEY -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"
    local py f targets
    targets=$(printf 'bin/%s ' "${GUEST_PROBES[@]}")
    local -a filt=(--include=/probe/ --include=/probe/Makefile --include=/probe/README.md
        --include='/probe/c/***' --include='/probe/kernels/***' --include='/probe/matrix/***'
        --include='/probe/suites/***' --include=/probe/run/ --include=/probe/run/matrix-guest.sh
        --include=/probe/python/)
    # GPU workloads only; the decoders (traceread, guestdiff, answerdiff, ...) run on the host.
    for py in convburn convoom drmfd framecopy mmsweep mtprobe pinwin rlprobe streamprobe torchprobe vramcap; do
        filt+=("--include=/probe/python/$py.py")
    done
    filt+=(--include=/scripts/ --include='/scripts/guest/***' --include=/scripts/lib/)
    for f in config common matrix; do filt+=("--include=/scripts/lib/$f.sh"); done
    filt+=(--include=/README.md --include=/LICENSE --exclude='__pycache__' --exclude='*')
    # Leandro-Test has no lib/ and no DRIVER_VERSION; both are added below and kept by
    # rsync --delete because excluded paths are left alone on the receiving side.
    do_ssh "mkdir -p $GUEST_TESTS/leandro-test $GUEST_TESTS/caraxes $GUEST_TESTS/out && ln -sfn leandro-test/probe $GUEST_TESTS/probe"
    rsync -a --delete -e "$rsh" "${filt[@]}" "$LEA_TEST/" "leandro@$IP:$GUEST_TESTS/leandro-test/"
    # The matrix payload reads DRIVER_VERSION from its own root (probe/run/matrix-guest.sh).
    do_ssh "cat > $GUEST_TESTS/leandro-test/DRIVER_VERSION" < "$LEA_ROOT/DRIVER_VERSION"
    rsync -a -e "$rsh" "$LEA_ROOT/tools/caraxes-guest-tests.sh" "leandro@$IP:$GUEST_TESTS/caraxes/run-tests.sh"
    # The v1 tracer, where probe/run/matrix-guest.sh looks for it; the gates re-run a
    # failed test under it. Host-built (it needs GLIBC 2.34, the guest has 2.39).
    local tracer=$LEA_ROOT/target/release/libnvrm_trace.so
    if [[ -f $tracer ]]; then
        do_ssh "mkdir -p $GUEST_TESTS/leandro-test/lib"
        rsync -a -e "$rsh" "$tracer" "leandro@$IP:$GUEST_TESTS/leandro-test/lib/libnvrm_trace.so"
    else
        echo "note: $tracer missing (cargo build --release -p nvrm-trace); failed tests get no RM trace"
    fi
    local lt_rev
    lt_rev=$(GIT_OPTIONAL_LOCKS=0 git -C "$LEA_TEST" describe --always --dirty --abbrev=12 2>/dev/null || echo unknown)
    # Build in the guest: its glibc and its libcuda, not the host's (v1 lea_guest_cc).
    do_ssh "set -e; cd $GUEST_TESTS
        cuda=\$(dirname \$(dirname \$(ls $GUEST_VENV/lib/python3*/site-packages/nvidia/cu*/include/cuda.h | head -1)))
        make -s -C leandro-test/probe CUDA_HOME=\$cuda LEANDRO=/nonexistent \
            CUDA_LDLIBS='-L/opt/nvrm/lib -lcuda' $targets > out/build.log 2>&1 \
            || { tail -20 out/build.log; exit 1; }
        {
            echo \"# SPDX-License-Identifier: MIT\"
            echo \"# caraxes-tests sync, \$(date -Iseconds)\"
            echo \"leandro-test $lt_rev (from $LEA_TEST)\"
            echo \"caraxes $(git -C "$LEA_ROOT" describe --always --dirty --abbrev=12 2>/dev/null)\"
            echo \"kernel \$(uname -r); \$(. /etc/os-release; echo \$PRETTY_NAME)\"
            echo \"nvidia.ko \$(sed -n 's/.*Kernel Module for x86_64 *\([0-9.]*\).*/\1/p' /proc/driver/nvidia/version 2>/dev/null)\"
            echo \"libcuda \$(readlink /opt/nvrm/lib/libcuda.so.1)\"
            echo \"cuda.h \$cuda/include/cuda.h CUDA_VERSION \$(sed -n 's/^#define CUDA_VERSION //p' \$cuda/include/cuda.h)\"
            echo \"gcc \$(gcc -dumpfullversion); python \$(python3 -V | cut -d' ' -f2)\"
            echo \"venv $GUEST_VENV: \$($GUEST_VENV/bin/pip list 2>/dev/null | awk '/^(torch|numpy|triton|cupy|cudf|cuml|nvidia-cuda-runtime|nvidia-cudnn)/{printf \"%s=%s \", \$1, \$2}')\"
            for t in nvcc ffmpeg strace cuda-gdb vulkaninfo clinfo glxinfo; do
                printf '%s %s\n' \$t \"\$(command -v \$t || echo missing)\"
            done
            echo \"built: ${GUEST_PROBES[*]}\"
        } > SYNC.txt
        cat SYNC.txt"
    echo "synced into $IP:$GUEST_TESTS"
}

case ${1:-} in
    up) up ;;
    sync-tests) sync_tests ;;
    probe) probe "${2:-}" ;;
    uvm) uvm "${2:-}" "${3:-}" ;;
    dma) dma_load ;;
    down) down ;;
    status) status ;;
    wait) wait_ssh "${2:-90}" ;;
    ssh) shift; do_ssh "$@" ;;
    serial) tail -n "${2:-40}" "$W/serial.log" ;;
    name) [[ -x $SERVER ]] || error "$SERVER missing: cargo build --release -p vfio-user-nvgpu"
          "$SERVER" --print-name "${IDENTITY[@]}" ;;
    *) sed -n '5,/^set -euo/p' "$0" | sed '$d'; exit 1 ;;
esac

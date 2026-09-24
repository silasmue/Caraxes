#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# P20 parallelism proof helpers (docs/caraxes/parallel.md). Every command takes the VM
# name (vm1, vm2, vm3) and sets the launcher's environment for it: the green
# configuration of testing.md §7b with a 2 GiB profile and one reservation slot per VM.
# Usage: caraxes/tools/p20.sh <vm> up|down|nodes|patprobe|vm [args...]
#        caraxes/tools/p20.sh - smi <file> | xid <file> <since> | host <dir> <tag>
#   up        tools/caraxes-vm.sh up, wait, probe p20-<vm>, then nodes and the probe build
#   down      sync in the guest, then tools/caraxes-vm.sh down
#   nodes     nvidia-uvm and the /dev/nvidia* nodes in the guest (no nvidia-modprobe there)
#   patprobe  build caraxes/tools/p20-patprobe.c in the guest (driver API, guest libcuda)
#   vm ...    any tools/caraxes-vm.sh command with this VM's environment
#   smi       host nvidia-smi (full and memory query) into <file> (read-only NVML)
#   xid       host kernel log lines with Xid since <since> into <file>, plus the count of
#             refused host BAR1 mappings (the P20 BAR1-exhaustion finding)
#   host      smi + a timestamp into <dir>/smi-<tag>.txt
# CARAXES_SERVER defaults to target/p20/release/vfio-user-nvgpu (built with
# CARGO_TARGET_DIR=target/p20 so the shared target/release binary is left alone).
set -euo pipefail
ROOT=$(cd "$(dirname "$(readlink -f "$0")")/../.." && pwd -P)
VMSH=$ROOT/tools/caraxes-vm.sh
error() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

vm_env() {
    case $1 in
        vm1) export VM=vm1 CARAXES_IP=192.168.101.21 CARAXES_TAP=caraxes1 CARAXES_MAC=02:00:00:00:00:21 CARAXES_RESERVE_SLOT=${P20_SLOT:-0} ;;
        vm2) export VM=vm2 CARAXES_IP=192.168.102.22 CARAXES_TAP=caraxes2 CARAXES_MAC=02:00:00:00:00:22 CARAXES_RESERVE_SLOT=${P20_SLOT:-1} ;;
        vm3) export VM=vm3 CARAXES_IP=192.168.103.23 CARAXES_TAP=caraxes3 CARAXES_MAC=02:00:00:00:00:23 CARAXES_RESERVE_SLOT=${P20_SLOT:-2} ;;
        *) error "vm1, vm2 or vm3 (vm0 is the display VM and is never touched)" ;;
    esac
    export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1 \
        CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register \
        CARAXES_FB_MIB=${P20_FB_MIB:-2048} CARAXES_GATE_FB_MIB=${P20_FB_MIB:-2048}
    export CARAXES_SERVER=${CARAXES_SERVER:-$ROOT/target/p20/release/vfio-user-nvgpu}
}

smi() {
    { echo "# SPDX-License-Identifier: MIT"; echo "# host nvidia-smi, $(date -Iseconds)"
      nvidia-smi --query-gpu=memory.used,memory.free,memory.total --format=csv
      nvidia-smi --query-compute-apps=pid,used_memory --format=csv
      nvidia-smi; } > "$1" 2>&1
}

xid() {
    { echo "# SPDX-License-Identifier: MIT"; echo "# host kernel log, Xid lines since $2"
      journalctl -k --no-pager --since "$2" -g 'Xid' 2>&1 | grep -v '^-- No entries --' || true
      # BAR1 exhaustion on the host (P20 finding): refused BAR1 mappings in the host RM.
      echo "# host RM refused-BAR1-mapping asserts (reusemappingdbMap/pMapCb NV_ERR_NO_MEMORY) since $2: $(journalctl -k --no-pager --since "$2" -g 'reusemappingdbMap|_reusemappingdbAddMappingCallback' 2>/dev/null | grep -c 'NO_MEMORY')"
    } > "$1"
}

nodes() {
    "$VMSH" ssh 'sudo modprobe nvidia-uvm
        [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
        [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
        u=$(grep nvidia-uvm /proc/devices | cut -d" " -f1)
        sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools
        sudo mknod -m 666 /dev/nvidia-uvm c "$u" 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c "$u" 1
        ls -la /dev/nvidia*'
}

patprobe() {
    "$VMSH" ssh 'mkdir -p p20 && cat > p20/p20-patprobe.c' < "$ROOT/caraxes/tools/p20-patprobe.c"
    "$VMSH" ssh 'cd p20 && inc=$(dirname $(ls ~/gpu/venv/lib/python3*/site-packages/nvidia/cu*/include/cuda.h | head -1)) &&
        gcc -O2 -Wall -o p20-patprobe p20-patprobe.c -I$inc -L/opt/nvrm/lib -lcuda -Wl,-rpath,/opt/nvrm/lib && echo built p20/p20-patprobe'
}

[[ $# -ge 2 ]] || { sed -n '5,19p' "$0"; exit 1; }
v=$1 cmd=$2; shift 2
if [[ $v == - ]]; then
    case $cmd in
        smi) smi "$1" ;;
        xid) xid "$1" "$2" ;;
        host) mkdir -p "$1"; smi "$1/smi-$2.txt" ;;
        *) error "unknown host command $cmd" ;;
    esac
    exit 0
fi
vm_env "$v"
case $cmd in
    up) "$VMSH" up && "$VMSH" wait 150 && "$VMSH" probe "p20-${1:-$v}" && nodes >/dev/null && patprobe ;;
    down) "$VMSH" ssh sync 2>/dev/null || true; "$VMSH" down ;;
    nodes) nodes ;;
    patprobe) patprobe ;;
    vm) "$VMSH" "$@" ;;
    *) error "unknown command $cmd" ;;
esac

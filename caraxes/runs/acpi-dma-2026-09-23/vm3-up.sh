#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Start vm3 for the ACPI _DMA tests (patches/ACPI-DMA.md §6), 2026-09-23.
# The same device server and Cloud Hypervisor command line as `tools/caraxes-vm.sh up`
# with CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096 CARAXES_HOST_RM=1, plus what that launcher
# cannot express: the GPU on PCI segment 1 (--user-device ...,pci_segment=1), a
# --pci-segment dma_ranges= table, and extra kernel command line words.
# Environment: CH (Cloud Hypervisor binary), SEG1=1 (two segments, GPU on segment 1),
# DMA_RANGES (the dma_ranges=[...] list, needs SEG1=1), KERNEL, INITRD, CMDLINE_EXTRA,
# CH_EXTRA (more Cloud Hypervisor arguments, e.g. -v). $W/launch-times gets the wall clock
# (date +%s.%N) at the server and at the Cloud Hypervisor start, to align gpu-trace.log
# (microseconds since server start) with ch.log (seconds since Cloud Hypervisor start).
# The other launcher commands (wait, ssh, probe, down) work unchanged with
# VM=vm3 CARAXES_TAP=caraxes3 CARAXES_IP=192.168.103.23 CARAXES_MAC=02:00:00:00:00:23.
set -euo pipefail
ROOT=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd -P)
W=/mnt/vmstore/caraxes/vm3
REF=$ROOT/caraxes/reference/rtx2070
VMDIR=/mnt/vmstore/leandro/vm
CH=${CH:-/mnt/vmstore/leandro/cloud-hypervisor/bin/cloud-hypervisor}
KERNEL=${KERNEL:-$VMDIR/guest-image/vmlinuz}
INITRD=${INITRD:-$VMDIR/guest-image/initrd}
SERVER=$W/vfio-user-nvgpu   # frozen copy of target/release/vfio-user-nvgpu
alive() { [[ -f $1 ]] && kill -0 "$(cat "$1")" 2>/dev/null; }
alive "$W/ch.pid" && { echo "vm3 already running" >&2; exit 1; }
rm -f "$W/gpu.sock" "$W/ch.sock" "$W/dma-ranges"

seg=() dev=socket=$W/gpu.sock,id=gpu0
if [[ ${SEG1:-0} == 1 ]]; then
    seg=(--platform num_pci_segments=2)
    [[ -n ${DMA_RANGES:-} ]] && seg+=(--pci-segment "pci_segment=1,dma_ranges=[$DMA_RANGES]")
    dev+=,pci_segment=1
fi
echo "server $(date +%s.%N)" > "$W/launch-times"
"$SERVER" --socket "$W/gpu.sock" --trace "$W/gpu-trace.log" \
    --config "$REF/config-space.txt" --rom "$REF/rom.bin" \
    --vbios "$REF/synth-vbios.bin" --boot-model --fb-mib 4096 --host-rm > "$W/gpu.log" 2>&1 &
echo $! > "$W/gpu.pid"
for _ in $(seq 50); do [[ -S $W/gpu.sock ]] && break; sleep 0.1; done
[[ -S $W/gpu.sock ]] || { cat "$W/gpu.log"; exit 1; }
echo "ch $(date +%s.%N)" >> "$W/launch-times"
set -x
"$CH" ${CH_EXTRA:-} --api-socket "$W/ch.sock" \
    --cpus boot=8 --memory size=4G,shared=on \
    --kernel "$KERNEL" --initramfs "$INITRD" \
    --cmdline "root=/dev/vda1 rw console=ttyS0 net.ifnames=0${CMDLINE_EXTRA:+ $CMDLINE_EXTRA}" \
    --disk path="$W/rootfs.qcow2",image_type=qcow2,backing_files=on \
           path="$W/seed.img",readonly=on \
    --net tap=caraxes3,mac=02:00:00:00:00:23 \
    "${seg[@]}" \
    --user-device "$dev" \
    --serial file="$W/serial.log" --console off \
    > "$W/ch.log" 2>&1 &
{ set +x; } 2>/dev/null
echo $! > "$W/ch.pid"
echo "server pid $(cat "$W/gpu.pid"), VM pid $(cat "$W/ch.pid"); logs in $W"

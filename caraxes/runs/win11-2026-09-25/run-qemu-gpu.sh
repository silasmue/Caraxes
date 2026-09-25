#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# The installed Windows 11 under QEMU with the synthetic GPU attached over vfio-user
# (`-device vfio-user-pci`, QEMU 11.1), beside the std VGA that shows the desktop. Guest RAM
# comes from a shared memfd so the device can map it. rombar=0 by default: the VBIOS GOP driver
# in the device's ROM spins on NV_PTIMER_TIME (0 before the PTIMER model) and hung the firmware.
# Same firmware, TPM and disk as install-qemu.sh; the virtio-win ISO stays attached for drivers.
# Start gpu-up.sh first.
# Usage: run-qemu-gpu.sh      (WIN_MEM default 8G, WIN_CPUS default 4, WIN_GPU_ROMBAR default 0,
#                               WIN_VGA default std; WIN_VGA=none leaves the synthetic GPU as the
#                               only display adapter: the desktop is then reached over RDP only;
#                               WIN_GPU_ROOTPORT=1 attaches the card behind a PCIe root port)
set -euo pipefail
W=/mnt/vmstore/caraxes/win11
VIRTIO=$W/iso/virtio-win.iso
CODE=/usr/share/edk2/x64/OVMF_CODE.secboot.4m.fd
VARS=$W/OVMF_VARS.4m.fd
MEM=${WIN_MEM:-8G} CPUS=${WIN_CPUS:-4}
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
# WIN_GPU_ROMBAR=1 keeps the expansion ROM BAR (Windows then sees the ROM the device serves;
# only with a ROM without the EFI image, gpu-up.sh WIN_GPU_ROM, or the firmware hangs in the GOP).
# WIN_GPU_ROOTPORT=1 puts the card behind a PCIe root port (as on a real board and as libvirt
# does), instead of directly on the root bus as an integrated endpoint; the parent is then a
# port with a link, which the Windows driver may query.
port=()
bus=""
if [[ ${WIN_GPU_ROOTPORT:-0} == 1 ]]; then
    port=(-device pcie-root-port,id=rp1,bus=pcie.0,chassis=1,slot=1)
    bus=',"bus":"rp1"'
fi
gpu=("${port[@]}" -device "{\"driver\":\"vfio-user-pci\",\"socket\":{\"type\":\"unix\",\"path\":\"$W/gpu.sock\"},\"rombar\":${WIN_GPU_ROMBAR:-0}$bus}")
if [[ ${WIN_GPU:-1} == 0 ]]; then gpu=(); echo "WIN_GPU=0: memfd RAM, no device (isolation run)"; else
    [[ -S $W/gpu.sock ]] || die "no device socket $W/gpu.sock: run gpu-up.sh first"
fi
[[ -s $W/win11.qcow2 ]] || die "no $W/win11.qcow2: run install-qemu.sh first"
[[ -f $VARS ]] || die "no $VARS (install-qemu.sh creates it)"
mkdir -p "$W/swtpm"
if ! pgrep -f "swtpm socket.*$W/swtpm" >/dev/null; then
    swtpm socket --tpm2 --tpmstate "dir=$W/swtpm" --ctrl "type=unixio,path=$W/swtpm/sock" \
        --daemon --log "file=$W/swtpm/swtpm.log"
    sleep 0.5
fi
echo "QEMU window on this desktop: Windows 11 with the synthetic GPU on $W/gpu.sock ($MEM shared memfd, $CPUS vCPUs)"
exec qemu-system-x86_64 -enable-kvm -machine q35,accel=kvm,smm=on,memory-backend=mem -cpu host,hv_relaxed,hv_vapic,hv_spinlocks=0x1fff,hv_time \
    -smp "$CPUS" -m "$MEM" -object "memory-backend-memfd,id=mem,size=$MEM,share=on" \
    -global driver=cfi.pflash01,property=secure,value=on \
    -drive "if=pflash,format=raw,readonly=on,file=$CODE" \
    -drive "if=pflash,format=raw,file=$VARS" \
    -chardev "socket,id=chrtpm,path=$W/swtpm/sock" -tpmdev emulator,id=tpm0,chardev=chrtpm -device tpm-crb,tpmdev=tpm0 \
    -drive "file=$W/win11.qcow2,if=virtio,format=qcow2,cache=writeback,discard=unmap" \
    -drive "file=$VIRTIO,media=cdrom,readonly=on" \
    -device virtio-net-pci,netdev=n0 -netdev user,id=n0,hostfwd=tcp:127.0.0.1:3389-:3389 \
    "${gpu[@]}" \
    -device qemu-xhci -device usb-tablet -device usb-kbd \
    -vga "${WIN_VGA:-std}" -display gtk -rtc base=localtime

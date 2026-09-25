#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# The one-time Windows 11 install under QEMU (README.md): OVMF with Secure Boot, swtpm TPM 2.0,
# a virtio-blk disk and virtio-net, the Windows ISO and the virtio-win ISO as CDs. The window
# opens on the operator's desktop and the operator drives the setup ("Load driver" from the
# virtio-win CD: viostor\w11\amd64). The TPM state and the disk are the ones up-ch.sh boots.
# Usage: install-qemu.sh [Win11.iso]      (WIN_MEM default 8G, WIN_CPUS default 4)
set -euo pipefail
W=/mnt/vmstore/caraxes/win11
ISO=${1:-$(ls -t "$HOME"/Downloads/Win11*.iso 2>/dev/null | head -1)}
VIRTIO=$W/iso/virtio-win.iso
CODE=/usr/share/edk2/x64/OVMF_CODE.secboot.4m.fd
VARS=$W/OVMF_VARS.4m.fd
MEM=${WIN_MEM:-8G} CPUS=${WIN_CPUS:-4}
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
[[ -s ${ISO:-} ]] || die "no Windows ISO (argument or ~/Downloads/Win11*.iso)"
[[ -s $VIRTIO ]] || die "no $VIRTIO (the download is still running?)"
[[ -s $CODE ]] || die "no $CODE (edk2-ovmf)"
command -v swtpm >/dev/null || die "swtpm missing"
mkdir -p "$W/swtpm"
[[ -f $VARS ]] || cp /usr/share/edk2/x64/OVMF_VARS.4m.fd "$VARS"
[[ -f $W/win11.qcow2 ]] || { qemu-img create -q -f qcow2 "$W/win11.qcow2" 80G; echo "new $W/win11.qcow2 (80 GiB, thin)"; }
if ! pgrep -f "swtpm socket.*$W/swtpm" >/dev/null; then
    swtpm socket --tpm2 --tpmstate "dir=$W/swtpm" --ctrl "type=unixio,path=$W/swtpm/sock" \
        --daemon --log "file=$W/swtpm/swtpm.log"
    sleep 0.5
fi
echo "QEMU window on this desktop: $ISO, $MEM, $CPUS vCPUs; Secure Boot UEFI, TPM 2.0, virtio disk and net"
exec qemu-system-x86_64 -enable-kvm -machine q35,accel=kvm,smm=on -cpu host,hv_relaxed,hv_vapic,hv_spinlocks=0x1fff,hv_time \
    -smp "$CPUS" -m "$MEM" \
    -global driver=cfi.pflash01,property=secure,value=on \
    -drive "if=pflash,format=raw,readonly=on,file=$CODE" \
    -drive "if=pflash,format=raw,file=$VARS" \
    -chardev "socket,id=chrtpm,path=$W/swtpm/sock" -tpmdev emulator,id=tpm0,chardev=chrtpm -device tpm-crb,tpmdev=tpm0 \
    -drive "file=$W/win11.qcow2,if=virtio,format=qcow2,cache=writeback,discard=unmap" \
    -drive "file=$ISO,media=cdrom,readonly=on" \
    -drive "file=$VIRTIO,media=cdrom,readonly=on" \
    -device virtio-net-pci,netdev=n0 -netdev user,id=n0,hostfwd=tcp:127.0.0.1:3389-:3389 \
    -device qemu-xhci -device usb-tablet -device usb-kbd \
    -vga std -display gtk -rtc base=localtime -boot menu=on

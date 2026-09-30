#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The installed Windows 11 under Cloud Hypervisor with the synthetic GPU attached over vfio-user
# (`--user-device`), the counterpart of run-qemu-gpu.sh on the same state directory: the same
# disk chain and swtpm state; the firmware is EDK2's CLOUDHV.fd (Secure Boot and TPM2 built in,
# README.md "Firmware build"). There is no VGA, no GOP and no guest agent: nothing is visible
# before the NVIDIA driver loads, the desktop scans out on the device's display engine only
# (Sunshine), and the guest is reached over SSH on the tap (tools/windows/ssh-setup.ps1 sets the
# guest's address on the NIC with WIN_TAP_MAC at every boot). Installer and WinRE work stay on
# QEMU (run-qemu-gpu.sh), which boots the same disk.
# The GPU sits at PCI slot WIN_GPU_SLOT (default 6) of segment 0, where QEMU's q35 put it: the
# root complex is ACPI\PNP0A08\0 under both, so Windows keeps the same device instance and its
# driver key (the GSP firmware switch EnableGpuFirmware lives there).
# Start the device first (gpu-up-host.sh). Guest RAM is a shared memfd (shared=on), which the
# device maps; Hyper-V enlightenments with kvm_hyperv=on.
# Usage: run-ch-gpu.sh     (WIN_STATE_DIR, WIN_MEM default 8G, WIN_CPUS default 4, WIN_GPU=0 boots
#                           without the device, WIN_TAP default caraxes7, WIN_TAP_MAC default
#                           02:00:00:00:00:47, WIN_CH the binary, WIN_CH_FW the firmware,
#                           WIN_CH_SERIAL the serial log, default $W/ch-serial.log; with
#                           WIN_HOST_MODE=exec (the interrupt page) the tree page WIN_INTR_TREE, default 0xb81000,
#                           is mapped read-only)
set -euo pipefail
W=${WIN_STATE_DIR:-/mnt/vmstore/caraxes/win11}
CH=${WIN_CH:-/mnt/vmstore/caraxes/ch-readonly-mmap/target/release/cloud-hypervisor}
FW=${WIN_CH_FW:-/mnt/vmstore/caraxes/win11/CLOUDHV.fd}
MEM=${WIN_MEM:-8G} CPUS=${WIN_CPUS:-4}
TAP=${WIN_TAP:-caraxes7} MAC=${WIN_TAP_MAC:-02:00:00:00:00:47}
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
[[ -x $CH ]] || die "no Cloud Hypervisor at $CH"
[[ -s $FW ]] || die "no firmware $FW"
# The disk: the flattened raw image when it exists (win11.raw: image_type=raw, 4 queues, no
# direct=on, since the ublk device under /mnt/vmstore has 4096-byte logical sectors and Windows
# issues 512-byte I/O), else the qcow2 chain.
if [[ -s $W/win11.raw ]]; then
    disk="path=$W/win11.raw,image_type=raw,num_queues=4"
else
    [[ -s $W/win11.qcow2 ]] || die "no $W/win11.raw or $W/win11.qcow2"
    disk="path=$W/win11.qcow2,image_type=qcow2,backing_files=on"
fi
ip link show "$TAP" >/dev/null 2>&1 || die "no tap $TAP (created by the operator, owned by this user)"
gpu=()
if [[ ${WIN_GPU:-1} == 0 ]]; then
    echo "WIN_GPU=0: memfd RAM, no device"
else
    [[ -S $W/gpu.sock ]] || die "no device socket $W/gpu.sock: run gpu-up-host.sh first"
    dev="socket=$W/gpu.sock,id=gpu0,pci_device_id=${WIN_GPU_SLOT:-6}"
    # The device's interrupt page (on with host execution, WIN_HOST_MODE=exec): the interrupt
    # tree's page read-only for the guest, so its stores trap and the page shows the registers'
    # own values (intr_page.rs exact mode). TU106's tree page is BAR0 0xb81000 (chips/tu106 intr_tree).
    if [[ ${WIN_HOST_MODE:-rm} == exec ]]; then
        help=$("$CH" --help 2>&1)
        [[ $help == *x_readonly_mmap_areas* ]] ||
            die "the interrupt page (host execution) needs Cloud Hypervisor with read-only vfio-user mmap areas"
        dev+=",x_readonly_mmap_areas=[0:${WIN_INTR_TREE:-0xb81000}:0x1000]"
    fi
    gpu=(--user-device "$dev")
fi
mkdir -p "$W/swtpm"
if ! pgrep -f "swtpm socket.*$W/swtpm" >/dev/null; then
    swtpm socket --tpm2 --tpmstate "dir=$W/swtpm" --ctrl "type=unixio,path=$W/swtpm/sock" \
        --daemon --log "file=$W/swtpm/swtpm.log"
    sleep 0.5
fi
rm -f "$W/ch.sock"
echo "Cloud Hypervisor: Windows 11 ($MEM shared memfd, $CPUS vCPUs, kvm_hyperv), disk ${disk%%,*}, tap $TAP ($MAC), ${gpu[*]:-no GPU}"
exec "$CH" -v --api-socket "$W/ch.sock" \
    --cpus "boot=$CPUS,kvm_hyperv=on" --memory "size=$MEM,shared=on" \
    --kernel "$FW" \
    --disk "$disk" \
    --net "tap=$TAP,mac=$MAC" --rng src=/dev/urandom \
    --tpm "socket=$W/swtpm/sock" \
    --serial "file=${WIN_CH_SERIAL:-$W/ch-serial.log}" --console off \
    "${gpu[@]}"

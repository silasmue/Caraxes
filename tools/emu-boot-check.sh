#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Boot the synthetic GPU in pure emulation (no host RM, no reservation, no GPU) to
# RmInitAdapter and nvidia-smi, the check that keeps the regressions of 2026-09-24 from
# recurring silently (docs/caraxes/blackwell-gate.md section 5).
# Usage: tools/emu-boot-check.sh [--driver 610|615] [--chip tu106|gb206] [--record DIR]
# The guest is a tapless VM (CARAXES_TAP=none) whose overlay runs a boot-only check at boot
# and powers off: vm7 for the 610 guest, vm7-615 for the 615 guest (CARAXES_EMU_VM overrides),
# prepared by caraxes/runs/emu-regress-2026-09-25/prepare-vm7.sh. The device binary is
# CARAXES_SERVER, default target/release (one binary for every guest ABI: the guest chooses,
# docs/research/one-abi-binary-2026-09-28.md), built when absent.
# Exit 0 when the guest's RmInitAdapter succeeds and nvidia-smi returns 0; 1 when not, with
# the device's warnings about the guest registry (gspboot.rs); 77 with a SKIP note when a
# prerequisite is absent (Cloud Hypervisor, the guest kernel, the overlay, the check service
# or the GSP firmware in the image, the ROM capture) or the VM is busy. tools/check.sh runs
# it as its emu-boot step with CARAXES_CHECK_EMU_BOOT=1.
set -uo pipefail

LEA_ROOT=$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd -P)
DRIVER=610 CHIP=tu106 RECORD=
while [[ $# -gt 0 ]]; do
    case $1 in
        --driver) DRIVER=${2:?}; shift 2 ;;
        --chip) CHIP=${2:?}; shift 2 ;;
        --record) RECORD=${2:?}; shift 2 ;;
        -h|--help) sed -n '5,17p' "$0"; exit 0 ;;
        *) printf 'ERROR: unknown argument %s\n' "$1" >&2; exit 2 ;;
    esac
done
skip() { printf 'SKIP: %s\n' "$*"; exit 77; }
case $DRIVER in
    610) VER=610.57.04; VM=${CARAXES_EMU_VM:-vm7}
         SERVER=${CARAXES_SERVER:-$LEA_ROOT/target/release/vfio-user-nvgpu}; FEAT=() ;;
    615) VER=615.71.09; VM=${CARAXES_EMU_VM:-vm7-615}
         SERVER=${CARAXES_SERVER:-$LEA_ROOT/target/release/vfio-user-nvgpu}; FEAT=() ;;
    *) printf 'ERROR: --driver must be 610 or 615\n' >&2; exit 2 ;;
esac
case $CHIP in
    tu106) FIRMWARE=gsp_tu10x.bin; ROM=$LEA_ROOT/caraxes/reference/rtx2070/rom.bin; FB=1024 ;;
    gb206) FIRMWARE=gsp_ga10x.bin; ROM=$LEA_ROOT/caraxes/reference/gb206/gpu-0000_01_00.0/rom.bin; FB=4096 ;;
    *) printf 'ERROR: --chip must be tu106 or gb206\n' >&2; exit 2 ;;
esac

# Prerequisites; the launcher reads the same local.env.
# shellcheck disable=SC1091
[[ -f $LEA_ROOT/local.env ]] && source "$LEA_ROOT/local.env"
: "${LEA_VM_DIR:=/mnt/vmstore/leandro/vm}"
: "${LEA_CH:=/mnt/vmstore/leandro/cloud-hypervisor/bin/cloud-hypervisor}"
STORE=${CARAXES_STORE:-/mnt/vmstore/caraxes}
W=$STORE/$VM
[[ -x $LEA_CH ]] || skip "no Cloud Hypervisor at $LEA_CH"
[[ -f ${CARAXES_KERNEL:-$LEA_VM_DIR/guest-image/vmlinuz} ]] || skip "no guest kernel under $LEA_VM_DIR/guest-image"
[[ -f $W/rootfs.qcow2 && -f $W/seed.img ]] || skip "no overlay $W (caraxes/runs/emu-regress-2026-09-25/prepare-vm7.sh prepares vm7 and vm7-615)"
[[ -f $ROM ]] || skip "no ROM capture $ROM (git-ignored)"
command -v guestfish >/dev/null || skip "guestfish missing: cannot check the image for the firmware and the check service"
if pgrep -f "(vfio-user-nvgpu|cloud-hypervisor)[^ ]* .*$W/" >/dev/null; then skip "$VM is running"; fi
# The image must carry the firmware and the boot-only check; the module options are read
# back so that a missing key is named before the boot (docs/caraxes/quickstart.md section 2).
IMG=$(timeout 300 guestfish --ro -a "$W/rootfs.qcow2" -i sh \
    "ls /lib/firmware/nvidia/$VER/$FIRMWARE /etc/systemd/system/multi-user.target.wants/caraxes-check.service 2>&1; grep -h RegistryDwords /etc/modprobe.d/*.conf; true" 2>&1) \
    || skip "guestfish could not read $W/rootfs.qcow2"
grep -q "^/lib/firmware/nvidia/$VER/$FIRMWARE\$" <<< "$IMG" || skip "the image has no /lib/firmware/nvidia/$VER/$FIRMWARE"
grep -q "caraxes-check.service\$" <<< "$IMG" || skip "the image has no caraxes-check.service (prepare-vm7.sh)"
for k in RmDisableGlobalCeUtils=1 RMDisableScrubOnFree=1 RmRcWatchdog=0; do
    grep -q "$k" <<< "$IMG" || echo "note: the guest's module options lack $k; gspboot.rs missing_guest_keys names what fails without it"
done

if [[ ! -x $SERVER ]]; then
    (cd "$LEA_ROOT" && cargo build --release -p vfio-user-nvgpu "${FEAT[@]}") || { echo "FAIL: device build"; exit 1; }
fi
RECORD=${RECORD:-$LEA_ROOT/target/emu-boot-check/$DRIVER-$CHIP-$(date +%Y%m%d-%H%M%S)}
mkdir -p "$RECORD"

# Pure emulation: nothing of the host GPU.
unset CARAXES_HOST_RM CARAXES_RESERVE_VRAM CARAXES_HOST_EXEC CARAXES_DMA CARAXES_COSTUME
export VM CARAXES_STORE=$STORE CARAXES_TAP=none CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=$FB
export CARAXES_CHIP=$CHIP CARAXES_SERVER=$SERVER CARAXES_DRIVER=$DRIVER
t0=$(date +%s)
rm -f "$W/serial.log"
"$LEA_ROOT/tools/caraxes-vm.sh" up > "$RECORD/up.txt" 2>&1 || { cat "$RECORD/up.txt"; echo "FAIL: the launcher did not start $VM"; exit 1; }
for _ in $(seq 300); do
    kill -0 "$(cat "$W/ch.pid" 2>/dev/null)" 2>/dev/null || break
    grep -q "CARAXES-CHECK end" "$W/serial.log" 2>/dev/null && sleep 5 && break
    sleep 2
done
"$LEA_ROOT/tools/caraxes-vm.sh" down > "$RECORD/down.txt" 2>&1 || true
cp "$W/serial.log" "$RECORD/serial.log" 2>/dev/null
cp "$W/gpu.log" "$RECORD/gpu.log" 2>/dev/null
grep -aE "RmInitAdapter|memmgrMemSet|numGpcs|nvidia-smi rc|^[^,]+, [0-9.]+," "$RECORD/serial.log" 2>/dev/null | head -n 12
grep -a "WARNING guest registry" "$RECORD/gpu.log" 2>/dev/null | sort -u
# `RmInitAdapter succeeded` is printed only at NVreg_ResmanDebugLevel=0; the verdict is
# nvidia-smi's exit code with no `RmInitAdapter failed` in the guest log.
if grep -aq "nvidia-smi rc=0" "$RECORD/serial.log" 2>/dev/null &&
   ! grep -aq "RmInitAdapter failed" "$RECORD/serial.log"; then
    echo "PASS: $CHIP, $VER guest, pure emulation: RmInitAdapter succeeded, nvidia-smi 0 ($(( $(date +%s) - t0 )) s; $RECORD)"
    exit 0
fi
echo "FAIL: $CHIP, $VER guest, pure emulation: no RmInitAdapter success or nvidia-smi failed ($RECORD)"
exit 1

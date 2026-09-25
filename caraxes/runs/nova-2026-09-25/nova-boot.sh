#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# One pure-emulation boot of the nova guest (docs/caraxes/nova-guest.md): vm9 (prepare-vm9.sh)
# with the Linux 7.2.7 nova kernel booted directly (no initramfs), no tap, no host RM, no
# reservation, no GPU; the device is a 610 build serving CARAXES_GUEST_ABI=570 with the nova
# VBIOS image (synth-vbios.py --ext-sig 0x4E56 --appid 0x85). nova-check.sh in the guest
# probes, checks the DRM side, cycles unbind/bind and powers off. Usage:
#   nova-boot.sh <record dir> [launcher environment as NAME=value ...]
# Keeps serial.log and gpu.log (ignored as *.log) and the committed extracts guest-nova.txt
# and device-boot.txt under <dir>.
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
R=$(cd "$HERE/../../.." && pwd -P)
out=$1; shift
[[ -e $out ]] && { echo "ERROR: $out exists; never overwrite a run" >&2; exit 2; }
mkdir -p "$out"
unset CARAXES_HOST_RM CARAXES_RESERVE_VRAM CARAXES_HOST_EXEC CARAXES_DMA CARAXES_COSTUME CARAXES_DRIVER
export VM=vm9 CARAXES_TAP=none CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=${CARAXES_FB_MIB:-1024}
export CARAXES_MEM=${CARAXES_MEM:-1G} CARAXES_CPUS=${CARAXES_CPUS:-2}
export CARAXES_GUEST_ABI=570
export CARAXES_KERNEL=${CARAXES_KERNEL:-$R/target/nova-build/arch/x86/boot/bzImage} CARAXES_INITRD=none
export CARAXES_VBIOS=${CARAXES_VBIOS:-$R/caraxes/reference/rtx2070/synth-vbios-nova.bin}
export CARAXES_SERVER=${CARAXES_SERVER:-$R/target/nova/release/vfio-user-nvgpu}
for kv in "$@"; do export "${kv?}"; done
W=/mnt/vmstore/caraxes/$VM
if pgrep -f "(vfio-user-nvgpu|cloud-hypervisor)[^ ]* .*$W/" >/dev/null; then echo "ERROR: a $VM process runs" >&2; exit 2; fi
{ sha256sum "$CARAXES_SERVER" "$CARAXES_KERNEL" "$CARAXES_VBIOS"; git -C "$R" log --oneline -1; env | grep -E '^CARAXES_|^VM=' | sort; } > "$out/setup.txt"
rm -f "$W/serial.log"
"$R/tools/caraxes-vm.sh" up | tee "$out/up.txt"
for _ in $(seq 200); do
    kill -0 "$(cat "$W/ch.pid")" 2>/dev/null || break
    grep -q "CARAXES-CHECK end" "$W/serial.log" 2>/dev/null && sleep 5 && break
    sleep 2
done
"$R/tools/caraxes-vm.sh" down > "$out/down.txt" 2>&1 || true
cp "$W/serial.log" "$out/serial.log"
cp "$W/gpu.log" "$out/gpu.log"
grep -aE "CARAXES-CHECK|nova|NVIDIA|GPU name|GSP|WPR2|rc=|/dev/dri|card|render|watchdog|Linux version|panic|BUG|Oops" \
    "$out/serial.log" | cut -c1-300 > "$out/guest-nova.txt" || true
grep -aE "gsp start|sec2 start|fwsec|wpr meta|doorbell|queues|libos arg|rmargs|tx seq|rx seq|static-info|unloading|rejected|unanswered|ERROR|WARN" \
    "$W/gpu-trace.log" "$out/gpu.log" 2>/dev/null | sed -E 's/^[^:]*://' | cut -c1-300 | head -n 400 > "$out/device-boot.txt" || true
grep -aE "NVIDIA \(Chipset|GPU name|successfully unloaded|modprobe nova_core rc|nova-core.*(error|failed|timed out|EIO|EINVAL|ENODEV)" "$out/serial.log" | head -n 30

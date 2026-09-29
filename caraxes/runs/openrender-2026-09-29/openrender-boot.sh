#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# One boot of the open-driver render guest (docs/research/open-render-2026-09-29.md): vm10
# (prepare-vm10.sh) with the Linux 7.2.7 openrender kernel booted directly (no initramfs),
# no tap, host execution on a VRAM reservation. nouveau loads GSP-RM 570.144, so the device
# detects guest ABI 570 by itself (nothing pinned). openrender-check.sh in the guest walks
# the rungs R1 probe, R2 kernel channels, R3 NVK device, R4 clear+copy, R5 triangle, then
# unbinds and powers off. Usage:
#   openrender-boot.sh <record dir> [launcher environment as NAME=value ...]
# The settings are production's (night-2026-09-28/desktop-b-night.sh) without the display,
# the games disk and the fast paths that are shaped by the 615 kernel's layout of BAR1,
# BAR2, the interrupt leaves and the doorbell (BAR1 pool, BAR2 memfd, interrupt page,
# doorbell tokens and polling, MSI re-arm ioeventfd, lazy allocations); any of them can be
# added back as NAME=value arguments. Compression per VM slice is required: nouveau keeps
# only FB regions that report supportCompressed and supportISO (Linux 7.2.7
# nvkm/subdev/gsp/rm/r535/gsp.c:191-199), and D16 reports none.
# Keeps serial.log and gpu.log (ignored as *.log), the committed extracts guest.txt and
# device.txt, and vk-offscreen.png when the guest printed its image.
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
R=$(cd "$HERE/../../.." && pwd -P)
out=${1:?usage: openrender-boot.sh <record dir> [NAME=value ...]}; shift
# The production flags a caller's shell may export and this run leaves out (header).
unset CARAXES_GUEST_ABI CARAXES_DISPLAY CARAXES_DISPLAY_PAGE CARAXES_DISPLAY_PAGE_READONLY \
    CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_BAR1_POOL_MIB CARAXES_DOORBELL_TOKENS \
    CARAXES_DOORBELL_POLL CARAXES_BAR2_MEMFD CARAXES_MSI_REARM_IOEVENTFD CARAXES_ALLOC_LAZY \
    CARAXES_GAMES_DISK CARAXES_COSTUME
export VM=vm10 CARAXES_TAP=none CARAXES_DRIVER=615 CARAXES_INITRD=none
# Everything below may come from the environment or from NAME=value arguments; the
# arguments win.
for kv in "$@"; do
    [[ $kv == [A-Z_]*=* ]] || { echo "ERROR: $kv is not NAME=value" >&2; exit 2; }
    export "${kv?}"
done
export CARAXES_MEM=${CARAXES_MEM:-4G} CARAXES_CPUS=${CARAXES_CPUS:-4}
export CARAXES_FB_MIB=${CARAXES_FB_MIB:-1024} CARAXES_RESERVE_SLOT=${CARAXES_RESERVE_SLOT:-5}
export CARAXES_KERNEL=${CARAXES_KERNEL:-/mnt/vmstore/caraxes/nova-kernel/build-openrender/arch/x86/boot/bzImage}
export CARAXES_VBIOS=${CARAXES_VBIOS:-/mnt/vmstore/caraxes/night-2026-09-28/synth-vbios-nova.bin}
export CARAXES_SERVER=${CARAXES_SERVER:-}
[[ -x $CARAXES_SERVER ]] || { echo "ERROR: CARAXES_SERVER (the openrender device binary) missing: '$CARAXES_SERVER'" >&2; exit 2; }
export CARAXES_BOOT_MODEL=${CARAXES_BOOT_MODEL:-1}
export CARAXES_HOST_RM=${CARAXES_HOST_RM:-1} CARAXES_RESERVE_VRAM=${CARAXES_RESERVE_VRAM:-1} \
    CARAXES_HOST_EXEC=${CARAXES_HOST_EXEC:-1} CARAXES_DMA=${CARAXES_DMA:-register} \
    CARAXES_GUEST_DMA_GPA=${CARAXES_GUEST_DMA_GPA:-1}
export CARAXES_MIRROR_CHANNELS=${CARAXES_MIRROR_CHANNELS:-1} CARAXES_MIRROR_GR_SHADOW=${CARAXES_MIRROR_GR_SHADOW:-1} \
    CARAXES_CE_SHADOW=${CARAXES_CE_SHADOW:-1} CARAXES_MIRROR_BARE_CE=${CARAXES_MIRROR_BARE_CE:-1}
export CARAXES_COMPRESSION=${CARAXES_COMPRESSION:-slice}
export CARAXES_BAR1_WINDOWS=${CARAXES_BAR1_WINDOWS:-2} CARAXES_BAR1_WINDOW_MIB=${CARAXES_BAR1_WINDOW_MIB:-16}
export CARAXES_HOST_VRAM_FLOOR_MIB=${CARAXES_HOST_VRAM_FLOOR_MIB:-1024} CARAXES_HOST_BAR1_FLOOR_MIB=${CARAXES_HOST_BAR1_FLOOR_MIB:-64}
for f in "$CARAXES_KERNEL" "$CARAXES_VBIOS"; do
    [[ -f $f ]] || { echo "ERROR: $f missing" >&2; exit 2; }
done
[[ -e $out ]] && { echo "ERROR: $out exists; never overwrite a run" >&2; exit 2; }
mkdir -p "$out"
W=/mnt/vmstore/caraxes/$VM
if pgrep -f "(vfio-user-nvgpu|cloud-hypervisor)[^ ]* .*$W/" >/dev/null; then echo "ERROR: a $VM process runs" >&2; exit 2; fi
{ sha256sum "$CARAXES_SERVER" "$CARAXES_KERNEL" "$CARAXES_VBIOS"; git -C "$R" log --oneline -1; env | grep -E '^CARAXES_|^VM=' | sort; } > "$out/setup.txt"
rm -f "$W/serial.log"
"$R/tools/caraxes-vm.sh" up | tee "$out/up.txt"
for _ in $(seq 360); do
    kill -0 "$(cat "$W/ch.pid")" 2>/dev/null || break
    grep -q "CARAXES-CHECK end" "$W/serial.log" 2>/dev/null && sleep 5 && break
    sleep 2
done
"$R/tools/caraxes-vm.sh" down > "$out/down.txt" 2>&1 || true
cp "$W/serial.log" "$out/serial.log"
cp "$W/gpu.log" "$out/gpu.log"
grep -aE "CARAXES-CHECK|nouveau|NVK|nvk|vk-offscreen|step [AB]|device [0-9]|driverName|driverInfo|deviceName|/dev/dri|renderD|card|rc=|Xid|fault|timeout|Linux version|panic|BUG|Oops|WARNING" \
    "$out/serial.log" | grep -v "^[A-Za-z0-9+/=]\{76\}$" | cut -c1-300 > "$out/guest.txt" || true
grep -aE "guest ABI|gsp start|sec2 start|fwsec|queues|static-info|unanswered|refused|not supported|ERROR|WARN|mirror|chid|doorbell|alloc: .*(0xc46f|0xc597|0xc5c0|0xc5b5|0x90f1)|ctrl 0x2080013[789]|ctrl 0x20801206|ctrl 0x20800a46|outside" \
    "$W/gpu-trace.log" "$out/gpu.log" 2>/dev/null | sed -E 's/^[^:]*://' | cut -c1-300 | head -n 2000 > "$out/device.txt" || true
python3 "$HERE/extract-ppm.py" "$out/serial.log" "$out/vk-offscreen.png" 2>/dev/null || true
# The rung lines: probe, render node, kernel channels, the Vulkan devices, the checks.
grep -aE "CARAXES-CHECK R|modprobe nouveau rc|probe with driver|init failed|renderD128|MM: using|deviceName|driverName|device [0-9]:|step [AB]|vk-offscreen: (PASS|FAIL)|vk-offscreen rc|vkmark|vkcube|eglinfo rc|OpenGL|FPS|Score" \
    "$out/serial.log" | grep -v "gsp: " | cut -c1-200 | head -n 40

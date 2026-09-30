#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Night 2026-09-28: Linux B (1 GiB, slot 4, FAST=2) with a device binary; the desktop comes up,
# then vkmark cube and clear for 10 s each at 1280x720 on Wayland. The device trace window of
# each scene and the launch timing are saved for the per-frame analysis. Usage: vkmark-trace.sh OUT BINARY
set -uo pipefail
OUT=$1 BIN=$2
W=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)/desktop-b-night.sh
T=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b/gpu-trace.log
mkdir -p "$OUT"
NIGHT_SERVER=$BIN CARAXES_LAUNCH_TIMING=1 timeout 400 "$W" up > "$OUT/up.txt" 2>&1 || { tail -5 "$OUT/up.txt"; exit 1; }
timeout 240 "$W" wait 200 >> "$OUT/up.txt" 2>&1
timeout 200 "$W" ssh 'bash -s' > "$OUT/desktop.txt" 2>&1 <<'G'
sudo ip route replace default via 192.168.106.1 dev eth0 2>/dev/null
sudo modprobe nvidia && sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm modeset=1 && echo modules-ok
bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1; sudo systemctl start gdm3; sleep 12
echo "gnome-shell=$(pgrep -c gnome-shell)"
G
for scene in cube clear; do
    s=$(wc -l < "$T")
    timeout 120 "$W" ssh "export XDG_RUNTIME_DIR=/run/user/1000; export WAYLAND_DISPLAY=\$(find /run/user/1000 -maxdepth 1 -type s -name 'wayland-*' -printf '%f\n' | head -1); vkmark --winsys wayland -s 1280x720 -b $scene:duration=10 2>&1 | grep -E 'FPS|Score|rror'" > "$OUT/vkmark-$scene.txt" 2>&1
    sleep 1
    tail -n +"$((s + 1))" "$T" > "$OUT/trace-$scene.txt"
    echo "$scene: $(grep -h FPS "$OUT/vkmark-$scene.txt" | head -1) trace lines $(wc -l < "$OUT/trace-$scene.txt")"
done
echo "guest xid: $(timeout 60 "$W" ssh "sudo dmesg | grep -c 'NVRM: Xid'" 2>/dev/null)"

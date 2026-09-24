#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Guest (as the desktop user, with sudo), display phase E (docs/caraxes/display-e.md): the
# before/after measurement set in one go, printing one line per number.
#   eglprobe 1080p (GPU clear + readback), Weston GL frame callbacks at 1080p60 with the
#   7 ms and 12 ms repaint windows, Vulkan present (vkprobe-wl --present 120, twice),
#   the KMS capture of a GPU-rendered scanout (sunshine-kms.sh), and Mutter's frame
#   callbacks in the GNOME session (started here, stopped at the end).
set -u
cd "$(dirname "$0")"
echo "== $(date -u +%FT%TZ) load $(cat /proc/loadavg)"
./eglprobe 1920 1080 120 | grep -E "FRAMES|READBACK"
for w in 7 12; do
    echo "== weston gl repaint-window $w"
    sudo WESTON_RENDERER=gl REPAINT_WINDOW=$w SETTLE=3 timeout 300 bash weston-pace.sh 1920x1080@60 20 \
        | grep -E "first frame|FRAMES|PRESENTED|presented="
done
for i in 1 2; do
    echo "== present $i"
    sudo REPAINT_WINDOW=12 timeout 300 bash weston-run.sh 1920x1080@60 gl ./vkprobe-wl --present 120 | grep presented
done
echo "== capture"
sudo WESTON_RENDERER=gl REPAINT_WINDOW=12 SETTLE=8 timeout 300 bash sunshine-kms.sh 640x480 30 \
    | grep -E "^SAMPLE|Found monitor|Found H.264" | sort -u
echo "== mutter"
sudo bash gnome-check.sh start 60 | grep -E "Type=wayland|gbm renderer|page_flip"
U=$(id -u)
XDG_RUNTIME_DIR=/run/user/$U WAYLAND_DISPLAY=wayland-0 timeout 120 ./wlframes 20 64 8 | grep -E "first frame|FRAMES|PRESENTED|presented="
sudo systemctl stop gdm3

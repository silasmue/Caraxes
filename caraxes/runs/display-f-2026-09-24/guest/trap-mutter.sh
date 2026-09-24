#!/bin/bash
# SPDX-License-Identifier: MIT
# Series F (guest, desktop user), the per-frame trap table on Mutter: start GNOME when it does
# not run (gdm3, Wayland, autologin), let it settle, then wlframes in the session committing
# once per frame callback for 14 s; the host cuts a 10 s window out of the device trace.
cd ~/caraxes-guest || exit 1
unset DISPLAY
if ! pgrep -x gnome-shell >/dev/null; then
    sudo bash gnome-check.sh start 60 | grep -E "Type=wayland|gbm renderer|page_flip"
fi
sleep "${2:-5}"
echo "== $(date -u +%FT%TZ) load $(cat /proc/loadavg)"
XDG_RUNTIME_DIR=/run/user/$(id -u) WAYLAND_DISPLAY=wayland-0 timeout 120 ./wlframes "${1:-14}" 64 8 \
    | grep -E "first frame|FRAMES|PRESENTED|presented="

#!/bin/bash
# SPDX-License-Identifier: MIT
# Series F (guest, desktop user), the per-frame trap table on Weston: stop GNOME if it runs,
# then Weston (GL renderer, 12 ms repaint window, 1080p60) with wlframes committing once per
# frame callback for 14 s; the host cuts a 10 s window out of the device trace.
cd ~/caraxes-guest || exit 1
unset DISPLAY
sudo systemctl stop gdm3 2>/dev/null; sleep 3
echo "== $(date -u +%FT%TZ) load $(cat /proc/loadavg)"
sudo WESTON_RENDERER=gl REPAINT_WINDOW=12 SETTLE=3 timeout 300 bash weston-pace.sh 1920x1080@60 "${1:-14}" \
    | grep -E "first frame|FRAMES|PRESENTED|presented="

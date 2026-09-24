#!/bin/bash
# SPDX-License-Identifier: MIT
# The display page series (guest, desktop user): one fresh GNOME start, the moment of the Xid 31
# of display-e.md §9 (f2, B1). Stops gdm3 (and Weston, if it runs), starts GNOME (Wayland,
# autologin), waits for gnome-shell to render, and reports what the session loaded.
cd ~/caraxes-guest || exit 1
unset DISPLAY
sudo pkill -x weston 2>/dev/null
sudo systemctl stop gdm3
sleep 3
echo "== $(date -u +%FT%TZ) start"
sudo bash gnome-check.sh start 60 | grep -E "Type=wayland|gbm renderer|page_flip"
sleep "${1:-20}"
pid=$(pgrep -x gnome-shell | head -1)
echo "gnome-shell pid ${pid:-none}; eglcore $(sudo grep -c libnvidia-eglcore "/proc/${pid:-0}/maps" 2>/dev/null || echo 0)"

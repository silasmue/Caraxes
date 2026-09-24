#!/bin/bash
# SPDX-License-Identifier: MIT
# Guest (root), phase D of docs/caraxes/display-cd.md: start gdm3 (GNOME, Wayland, autologin)
# on the Caraxes display, wait, and report what came up: sessions, the compositor's log,
# the CRTC's framebuffer, page-flip failures, colord.
# Usage: gnome-check.sh start|status [wait-seconds]
set -u
cd "$(dirname "$0")"
ACT=${1:-status} WAIT=${2:-180}
if [ "$ACT" = start ]; then
    date +%s > /tmp/gnome-check.t0
    systemctl start gdm3
    sleep "$WAIT"
fi
T0=$(cat /tmp/gnome-check.t0 2>/dev/null || echo 0)
since="@$T0"
echo "== sessions"; loginctl list-sessions --no-legend
echo "== processes"; ps -eo pid,etimes,comm | grep -E "gnome-shell|gdm|Xwayland|colord|gsd-color|mutter" | grep -v grep
echo "== session type"; for s in $(loginctl list-sessions --no-legend | awk '{print $1}'); do loginctl show-session "$s" -p Type -p Class -p State -p Name; done
echo "== kms"; ./kmsgrab info
echo "== gnome-shell / mutter log (errors, KMS, flips, gamma)"
journalctl --since "$since" -o short-monotonic --no-pager 2>/dev/null \
    | grep -iE "gnome-shell|mutter|gdm|colord|gsd-color" \
    | grep -iE "error|fail|warn|kms|drm|egl|gbm|render|flip|gamma|lut|crtc|mode|monitor|critical|backend" | tail -60
echo "== counts"
echo "page_flip_failed=$(journalctl --since "$since" --no-pager 2>/dev/null | grep -ci 'page flip failed')"
echo "== dmesg"; dmesg | grep -v "NVRM: VM:" | grep -E "WARNING|ERROR|timeout|nvidia-drm" | tail -10

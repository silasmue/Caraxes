#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Night 2026-09-28: Linux B (1 GiB, slot 4, FAST=2, host boost) per arm in a fresh VM: vkmark x2
# and glmark2 at 1280x720 on Wayland; arms: trace on, trace off, trace off plus the usermode,
# invalidate and display kick pages (docs/research/vkmark-frame-2026-09-28.md fixes 0-2).
# Usage: vkmark-ab.sh BINARY [ARM ...]
set -uo pipefail
N=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
BIN=$1; shift
W=/mnt/vmstore/caraxes/night-2026-09-28/desktop-b-night.sh
G=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b/gpu.log
for arm in "${@:-trace notrace pages}"; do
    O=$N/vkmark-ab/$arm; mkdir -p "$O"
    env=(NIGHT_SERVER="$BIN")
    case $arm in
        trace) ;;
        notrace) env+=(CARAXES_NO_TRACE=1) ;;
        pages) env+=(CARAXES_NO_TRACE=1 CARAXES_USERMODE_PAGE=1 CARAXES_INVALIDATE_PAGE=1 CARAXES_DISPLAY_KICK_PAGE=1) ;;
        notrace2) env+=(CARAXES_NO_TRACE=1) ;;
        um) env+=(CARAXES_NO_TRACE=1 CARAXES_USERMODE_PAGE=1) ;;
        inv) env+=(CARAXES_NO_TRACE=1 CARAXES_INVALIDATE_PAGE=1) ;;
        kick) env+=(CARAXES_NO_TRACE=1 CARAXES_DISPLAY_KICK_PAGE=1) ;;
        clean-ctl|clean-ctl2) env+=(CARAXES_NO_TRACE=1) ;;
        clean-kick|clean-kick2) env+=(CARAXES_NO_TRACE=1 CARAXES_DISPLAY_KICK_PAGE=1) ;;
        vb-*) ;;
        *) echo "unknown arm $arm" >&2; continue ;;
    esac
    env "${env[@]}" timeout 400 "$W" up > "$O/up.txt" 2>&1 || { echo "$arm: up failed"; tail -3 "$O/up.txt"; timeout 200 "$W" down >/dev/null 2>&1; continue; }
    timeout 240 "$W" wait 200 >> "$O/up.txt" 2>&1
    timeout 900 "$W" ssh 'bash -s' > "$O/guest.txt" 2>&1 <<'GUEST'
sudo ip route replace default via 192.168.106.1 dev eth0 2>/dev/null
sudo modprobe nvidia && sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm modeset=1 && echo modules-ok
bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1; sudo systemctl start gdm3; sleep 12
export XDG_RUNTIME_DIR=/run/user/1000
export WAYLAND_DISPLAY=$(find /run/user/1000 -maxdepth 1 -type s -name 'wayland-*' -printf '%f\n' | head -1)
echo "gnome-shell=$(pgrep -c gnome-shell) wayland=$WAYLAND_DISPLAY"
for i in 1 2; do timeout 300 vkmark --winsys wayland -s 1280x720 2>&1 | grep -E "Score|rror"; done
timeout 200 glmark2-wayland -s 1280x720 2>&1 | grep -E "Score|rror"
timeout --foreground 30s vkcube-wayland --c 300 >/dev/null 2>&1; echo "vkcube-wayland rc=$?"
echo "guest xid: $(sudo dmesg | grep -c 'NVRM: Xid')"
GUEST
    timeout 200 "$W" down > "$O/down.txt" 2>&1
    grep -E "launch timing|display page|usermode|invalidate page|kick page|host boost|ERROR" "$G" | cut -c1-400 > "$O/device-summary.txt"
    echo "$arm: $(grep -hE 'Score|rc=|xid' "$O/guest.txt" | tr -s ' ' | tr '\n' ';')"
done

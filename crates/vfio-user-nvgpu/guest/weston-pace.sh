#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Guest side of the Caraxes phase C compositor check (docs/caraxes/display-cd.md): start
# Weston on card0 with the DRM backend (atomic KMS), run a Wayland client whose frame
# callbacks and wp_presentation feedback are measured, stop Weston, print the logs.
# Usage (as root): weston-pace.sh [mode, e.g. 640x480 or 1920x1080@60] [seconds] [renderer]
# REPAINT_WINDOW (ms, Weston's default 7) widens the time Weston gives a repaint before
# the vblank: with the GL renderer the frame waits for the GPU's completion interrupt.
set -u
MODE=${1:-640x480} SECS=${2:-10} RENDERER=${3:-pixman}
D=/tmp/weston-pace
rm -rf "$D"; mkdir -p "$D"; chmod 700 "$D"
export XDG_RUNTIME_DIR=$D
cat > "$D/weston.ini" <<EOF
[core]
idle-time=0
repaint-window=${REPAINT_WINDOW:-7}
require-input=false
[shell]
background-color=0xff202020
panel-position=none
[output]
name=DP-1
mode=$MODE
EOF
LIBSEAT_BACKEND=noop weston --backend=drm --renderer="$RENDERER" --continue-without-input \
    --shell=kiosk --config="$D/weston.ini" --socket=wayland-1 --log="$D/weston.log" \
    --debug > "$D/weston.out" 2>&1 &
WPID=$!
for _ in $(seq 240); do [ -S "$D/wayland-1" ] && break; sleep 0.5; done
[ -S "$D/wayland-1" ] || { echo "WESTON-DOWN"; cat "$D/weston.log" "$D/weston.out" | tail -40; kill $WPID 2>/dev/null; exit 1; }
echo "weston up (pid $WPID) after $(ps -o etimes= -p $WPID) s"
# Let the first full-frame repaints (pixman into BAR1) finish before measuring.
sleep "${SETTLE:-5}"
WAYLAND_DISPLAY=wayland-1 "$(dirname "$0")/wlframes" "$SECS" "${SIZE:-64}" "${DAMAGE:-8}"
echo "wlframes exit $?"
cp "$D/weston.log" /tmp/ 2>/dev/null
kill -TERM $WPID; wait $WPID 2>/dev/null
echo "---- weston.log (tail)"
grep -vE "^\s*$" "$D/weston.log" | tail -n 60

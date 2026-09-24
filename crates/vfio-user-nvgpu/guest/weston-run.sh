#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Guest (root), display phase E (docs/caraxes/display-e.md): start Weston on card0 (DRM
# backend, atomic KMS, kiosk shell), run one command as a Wayland client, stop Weston.
# The display gate's GPU stages (swapchain, present, rt, events) run their probes this
# way. The command runs as the invoking user when SUDO_USER is set.
# Usage: weston-run.sh <mode, e.g. 1920x1080@60> <renderer: gl|pixman> <command...>
set -u
MODE=$1 RENDERER=$2
shift 2
D=/tmp/weston-run
rm -rf "$D"; mkdir -p "$D"; chmod 755 "$D"
export XDG_RUNTIME_DIR=$D
cat > "$D/weston.ini" <<EOF
[core]
idle-time=0
require-input=false
repaint-window=${REPAINT_WINDOW:-12}
[shell]
background-color=0xff202020
panel-position=none
[output]
name=DP-1
mode=$MODE
EOF
LIBSEAT_BACKEND=noop weston --backend=drm --renderer="$RENDERER" --continue-without-input \
    --shell=kiosk --config="$D/weston.ini" --socket=wayland-1 --log="$D/weston.log" \
    > "$D/weston.out" 2>&1 &
WPID=$!
for _ in $(seq 120); do [ -S "$D/wayland-1" ] && break; sleep 0.5; done
[ -S "$D/wayland-1" ] || { echo "WESTON-DOWN"; tail -30 "$D/weston.log" "$D/weston.out"; kill $WPID 2>/dev/null; exit 1; }
chmod 777 "$D/wayland-1"
echo "weston up (pid $WPID, renderer $RENDERER, $MODE); $(grep -m1 -o 'GL renderer: .*' "$D/weston.log")"
sleep "${SETTLE:-2}"
if [ -n "${SUDO_USER:-}" ]; then
    sudo -u "$SUDO_USER" env XDG_RUNTIME_DIR=$D WAYLAND_DISPLAY=wayland-1 "$@"
else
    WAYLAND_DISPLAY=wayland-1 "$@"
fi
rc=$?
echo "command exit $rc"
kill -TERM $WPID; wait $WPID 2>/dev/null
exit $rc

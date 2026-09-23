#!/bin/bash
# SPDX-License-Identifier: MIT
# Guest: Sunshine with capture=kms against the Caraxes display while Weston shows a client
# (docs/caraxes/display-cd.md, the display gate's capture stage). Sunshine probes its
# encoders at start by capturing test frames from the display; its log says whether the
# KMS capture found the plane and produced frames. Usage (root): sunshine-kms.sh [mode] [secs]
set -u
MODE=${1:-640x480} SECS=${2:-45}
cd "$(dirname "$0")"
D=/tmp/weston-pace
export SETTLE=${SETTLE:-6}
# Weston plus a client for the whole Sunshine window.
bash ./weston-pace.sh "$MODE" $((SECS + 20)) > /tmp/sunshine-weston.txt 2>&1 &
W=$!
for _ in $(seq 120); do [ -S $D/wayland-1 ] && break; sleep 0.5; done
sleep $((SETTLE + 5))
./kmsgrab info
./kmsgrab sample 320 240 4 4
S=/tmp/sunshine-cx
rm -rf $S; mkdir -p $S
cat > $S/sunshine.conf <<EOF
capture = kms
encoder = software
sw_preset = ultrafast
min_log_level = debug
file_apps = $S/apps.json
credentials_file = $S/credentials.json
log_path = $S/sunshine.log
state_file = $S/state.json
EOF
echo '{"env":{},"apps":[{"name":"Desktop"}]}' > $S/apps.json
HOME=$S timeout -s INT "$SECS" sunshine $S/sunshine.conf > $S/stdout.txt 2>&1
echo "sunshine exit $?"
grep -iE "kms|drm|screencast|monitor|display|capture|encoder|found|error|fatal|plane|framebuffer" $S/sunshine.log $S/stdout.txt 2>/dev/null | cut -c1-220 | head -60
wait $W
grep -E "FRAMES|PRESENTED|wlframes exit" /tmp/sunshine-weston.txt

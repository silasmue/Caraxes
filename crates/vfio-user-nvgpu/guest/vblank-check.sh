#!/bin/bash
# SPDX-License-Identifier: MIT
# Guest side of the vblank (LAST_DATA) check (docs/caraxes/display-cd.md): hold a mode
# with kmsflip --keep, ask NVKMS for N vblank notifications, report their spacing.
# Usage (root): vblank-check.sh [count] [mode-index]
set -u
N=${1:-120} MI=${2:-0}
cd "$(dirname "$0")"
# The image has no nvidia-modprobe: make the NVKMS node (char 195:254) by hand.
[ -e /dev/nvidia-modeset ] || mknod -m 666 /dev/nvidia-modeset c 195 254
./kmsflip --mode-index "$MI" --flips 2 --keep > /tmp/kmsflip-keep.txt 2>&1 &
K=$!
sleep 3
grep -E "^connector|RESULT" /tmp/kmsflip-keep.txt
./nvkms-vblank "$N" 0 0
rc=$?
grep nvidia /proc/interrupts
kill $K; wait $K 2>/dev/null
exit $rc

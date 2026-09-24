#!/bin/sh
# SPDX-License-Identifier: MIT
# Build the guest probes that need the 610.57.04 driver headers on the host, statically
# (the guest has no driver source headers): nvkms-vblank. Output: target/guest/.
set -e
ROOT=$(cd "$(dirname "$0")/../../.." && pwd -P)
O=$ROOT/vendor/open-gpu-kernel-modules/src
mkdir -p "$ROOT/target/guest"
cc -O2 -Wall -static -I"$O/nvidia-modeset/interface" -I"$O/common/sdk/nvidia/inc" \
    -I"$O/common/inc" -I"$O/common/unix/common/inc" \
    -o "$ROOT/target/guest/nvkms-vblank" "$(dirname "$0")/nvkms-vblank.c" -lm
echo "built $ROOT/target/guest/nvkms-vblank"

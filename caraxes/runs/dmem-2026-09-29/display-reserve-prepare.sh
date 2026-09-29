#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# On the host, with desktop-b-dmem up (desktop-b-dmem-6g.sh up; wait): build the display
# reserve nvidia.ko in the guest for its running kernel (docs/research/display-reserve-2026-09-29.md).
# Copies the 615.71.09 source (git archive of the vendored tag), patches/nvidia/0001 and
# crates/vfio-user-nvgpu/guest/display-reserve/ into the guest and runs build.sh there. The
# build lands in /opt/caraxes/display-reserve/<kernel>/ and is loaded only by
# sottr-bench-6g.sh with SOTTR_DISPLAY_RESERVE set. Log: OUT/prepare.txt.
# Usage: display-reserve-prepare.sh OUT
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
V=$HERE/desktop-b-dmem-6g.sh
SRC_GIT=/home/silas/git/Leandro/vendor/open-gpu-kernel-modules
TAG=615.71.09
OUT=${1:?usage: display-reserve-prepare.sh OUT}
mkdir -p "$OUT"
exec > >(tee -a "$OUT/prepare.txt") 2>&1
gssh() { timeout "${T:-120}" "$V" ssh "$@" 2> >(grep -v "unable to resolve" >&2); }

tarball=$OUT/open-gpu-kernel-modules-$TAG.tar.gz
git -C "$SRC_GIT" archive --format=tar.gz --prefix="open-gpu-kernel-modules-$TAG/" "$TAG" > "$tarball" || exit 1
sha=$(sha256sum "$tarball" | cut -d' ' -f1)
patch=$(ls "$REPO"/patches/nvidia/0001-*.patch)
echo "$(date +%T) source $TAG ($(git -C "$SRC_GIT" rev-parse "$TAG^{commit}")) sha256 $sha"
echo "patch $(sha256sum "$patch")"
echo "guest: $(gssh 'uname -r; ls -d /usr/src/nvidia-*' | tr '\n' ' ')"

gssh 'rm -rf ~/caraxes-display-reserve && mkdir -p ~/caraxes-display-reserve'
gssh "cat > ~/caraxes-display-reserve/source.tar.gz" < "$tarball"
gssh "cat > ~/caraxes-display-reserve/display-reserve.patch" < "$patch"
tar -C "$REPO/crates/vfio-user-nvgpu/guest" -cf - display-reserve |
    gssh 'tar -C ~/caraxes-display-reserve --strip-components=1 -xf -'
T=1800 gssh "cd ~/caraxes-display-reserve && sudo ./build.sh source.tar.gz $sha display-reserve.patch" || { echo "build failed"; exit 1; }
gssh 'ls -la /opt/caraxes/display-reserve/*/ /opt/caraxes/display-reserve/load.sh'
echo "$(date +%T) done"

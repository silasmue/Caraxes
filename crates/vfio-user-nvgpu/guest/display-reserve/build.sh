#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
#
# Build nvidia.ko and nvidia-modeset.ko with the display reserve patch (patches/nvidia/) for the
# running kernel, for game tests only (docs/research/display-reserve-2026-09-29.md). In the guest,
# as root:
#
#   build.sh SOURCE.tar.gz SOURCE_SHA256 PATCH
#
# SOURCE is NVIDIA's open-gpu-kernel-modules at the version of the installed driver. The patch
# changes the RM core (src/nvidia), the NVKMS core (src/nvidia-modeset) and nvidia-modeset's
# kernel interface; distribution packages ship both cores prebuilt, so the script builds the whole
# patched tree with the kernel's own build tree and installs only nvidia.ko and nvidia-modeset.ko,
# with load.sh, to /opt/caraxes/display-reserve-v2/<kernel>/. That is neither updates/ nor dkms/,
# and depmod is not run: a normal boot never loads them. The installed nvidia-drm and nvidia-uvm
# load on top of them: the patch changes no exported symbol, and the kernel's modversion check
# refuses them at load if one differed. The MANIFEST records the version line nvidia.ko reports
# in /proc/driver/nvidia/version (it names the build host and time), which load.sh checks.
set -euo pipefail

src_tar=${1:?usage: build.sh SOURCE.tar.gz SOURCE_SHA256 PATCH}
src_sha=${2:?usage: build.sh SOURCE.tar.gz SOURCE_SHA256 PATCH}
patch_file=${3:?usage: build.sh SOURCE.tar.gz SOURCE_SHA256 PATCH}
here=$(cd "$(dirname "$0")" && pwd)
kver=$(uname -r)
top=/opt/caraxes/display-reserve-v2
prefix=$top/$kver
work=/var/tmp/caraxes-display-reserve-v2

die() { echo "build.sh: $*" >&2; exit 1; }
(( EUID == 0 )) || die "run as root"
echo "$src_sha  $src_tar" | sha256sum -c --quiet - || die "$src_tar does not match sha256 $src_sha"

version=$(modinfo -F version nvidia) || die "no installed nvidia module for $kver"

rm -rf "$work"
mkdir -p "$work"
tar -C "$work" -xzf "$src_tar"
tree=$(ls -d "$work"/open-gpu-kernel-modules-*)
grep -q "^NVIDIA_VERSION = $version$" "$tree/version.mk" || die "$src_tar is not version $version"
patch -d "$tree" -p1 --forward --quiet < "$patch_file"

echo "== modules for $kver, $(gcc --version | head -n 1)"
make -C "$tree" -j"$(nproc)" modules SYSSRC="/lib/modules/$kver/build" > "$work/build.log" 2>&1 \
    || { tail -n 30 "$work/build.log"; die "build failed"; }

install -D -m 0644 "$tree/kernel-open/nvidia.ko" "$prefix/nvidia.ko"
install -D -m 0644 "$tree/kernel-open/nvidia-modeset.ko" "$prefix/nvidia-modeset.ko"
install -D -m 0755 "$here/load.sh" "$top/load.sh"
{
    echo "kernel $kver"
    echo "driver $version (installed $(modinfo -n nvidia))"
    echo "source $src_tar sha256 $src_sha"
    echo "patch $(sha256sum "$patch_file")"
    echo "compiler $(gcc --version | head -n 1)"
    echo "nvidia.ko $(sha256sum "$prefix/nvidia.ko" | cut -d' ' -f1)"
    echo "nvidia-modeset.ko $(sha256sum "$prefix/nvidia-modeset.ko" | cut -d' ' -f1)"
    echo "version $(strings "$prefix/nvidia.ko" | sed -n 's/^nvidia id: *//p' | head -n 1)"
    echo "built $(date -u +%FT%TZ)"
} > "$prefix/MANIFEST"
cat "$prefix/MANIFEST"
rm -rf "$work"

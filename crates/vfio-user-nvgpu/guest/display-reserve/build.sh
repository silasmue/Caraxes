#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
#
# Build nvidia.ko with the display reserve patch (patches/nvidia/) for the running kernel, for
# game tests only (docs/research/display-reserve-2026-09-29.md). In the guest, as root:
#
#   build.sh SOURCE.tar.gz SOURCE_SHA256 PATCH
#
# SOURCE is NVIDIA's open-gpu-kernel-modules at the version of the installed DKMS source
# (/usr/src/nvidia-<version>); the patch changes only the RM core (src/nvidia), which that DKMS
# source carries prebuilt as nvidia/nv-kernel.o_binary. The script applies the patch to SOURCE,
# builds the RM core, puts it into a copy of the DKMS source, builds the modules there with the
# kernel's own build tree, and installs nvidia.ko and load.sh to
# /opt/caraxes/display-reserve/<kernel>/. That is neither updates/ nor dkms/, and depmod is not
# run: a normal boot never loads it. load.sh loads it. The installed nvidia-modeset, nvidia-drm
# and nvidia-uvm load on top of it: the patch changes no exported symbol, and the kernel's
# modversion check refuses them at load if one differed. The MANIFEST records the version line
# the module reports in /proc/driver/nvidia/version (it names the build host and time), which
# load.sh checks after loading.
set -euo pipefail

src_tar=${1:?usage: build.sh SOURCE.tar.gz SOURCE_SHA256 PATCH}
src_sha=${2:?usage: build.sh SOURCE.tar.gz SOURCE_SHA256 PATCH}
patch_file=${3:?usage: build.sh SOURCE.tar.gz SOURCE_SHA256 PATCH}
here=$(cd "$(dirname "$0")" && pwd)
kver=$(uname -r)
prefix=/opt/caraxes/display-reserve/$kver
work=/var/tmp/caraxes-display-reserve

die() { echo "build.sh: $*" >&2; exit 1; }
(( EUID == 0 )) || die "run as root"
echo "$src_sha  $src_tar" | sha256sum -c --quiet - || die "$src_tar does not match sha256 $src_sha"

dkms_src=$(ls -d /usr/src/nvidia-[0-9]* 2>/dev/null | head -n 1)
[[ -n $dkms_src && -f $dkms_src/nvidia/nv-kernel.o_binary ]] || die "no NVIDIA DKMS source in /usr/src"
version=${dkms_src##*/nvidia-}
stock=$(modinfo -n nvidia 2>/dev/null) || die "no installed nvidia module for $kver"
[[ $(modinfo -F version nvidia) == "$version" ]] || die "installed nvidia is $(modinfo -F version nvidia), DKMS source is $version"

rm -rf "$work"
mkdir -p "$work"
tar -C "$work" -xzf "$src_tar"
tree=$(ls -d "$work"/open-gpu-kernel-modules-*)
grep -q "^NVIDIA_VERSION = $version$" "$tree/version.mk" || die "$src_tar is not version $version"
patch -d "$tree" -p1 --forward --quiet < "$patch_file"

echo "== RM core (src/nvidia), $(gcc --version | head -n 1)"
make -C "$tree/src/nvidia" -j"$(nproc)" > "$work/rm-core.log" 2>&1 || { tail -n 30 "$work/rm-core.log"; die "RM core build failed"; }

echo "== modules for $kver from $dkms_src"
cp -a "$dkms_src" "$work/dkms"
cp "$tree/src/nvidia/_out/Linux_x86_64/nv-kernel.o" "$work/dkms/nvidia/nv-kernel.o_binary"
make -C "$work/dkms" -j"$(nproc)" modules SYSSRC="/lib/modules/$kver/build" > "$work/modules.log" 2>&1 \
    || { tail -n 30 "$work/modules.log"; die "module build failed"; }

install -D -m 0644 "$work/dkms/nvidia.ko" "$prefix/nvidia.ko"
install -D -m 0755 "$here/load.sh" /opt/caraxes/display-reserve/load.sh
{
    echo "kernel $kver"
    echo "driver $version (DKMS source $dkms_src, installed $stock)"
    echo "source $src_tar sha256 $src_sha"
    echo "patch $(sha256sum "$patch_file")"
    echo "compiler $(gcc --version | head -n 1)"
    echo "nvidia.ko $(sha256sum "$prefix/nvidia.ko" | cut -d' ' -f1)"
    echo "version $(strings "$prefix/nvidia.ko" | sed -n 's/^nvidia id: *//p' | head -n 1)"
    echo "built $(date -u +%FT%TZ)"
} > "$prefix/MANIFEST"
cat "$prefix/MANIFEST"
rm -rf "$work"

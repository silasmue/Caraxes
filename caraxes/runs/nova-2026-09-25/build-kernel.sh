#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The nova guest kernel of docs/caraxes/nova-guest.md: Linux 7.2.7 (kernel.org stable, the
# newest release whose nova-core requests the versioned r570.144 firmware that linux-firmware
# ships) with CONFIG_NOVA_CORE=m and CONFIG_DRM_NOVA=m, built on the host with clang and the
# host's binutils (no ld.lld needed), rustc from rustup's stable toolchain (with rust-src).
# Everything lands under target/ (git-ignored):
#   target/nova-src/linux-7.2.7         the unpacked source (sha256 checked against kernel.org)
#   target/nova-build                   O= build tree: arch/x86/boot/bzImage, the modules
#   target/nova-modules                 INSTALL_MOD_PATH (lib/modules/7.2.7-nova)
# The three may be links to the NAS when the root disk is short (2026-09-25:
# /mnt/vmstore/caraxes/nova-kernel/{src,build,modules}); the build needs about 6 GB.
# Parallelism: JOBS (default 4); run it under nice -n 10 beside other work.
# Usage: caraxes/runs/nova-2026-09-25/build-kernel.sh [fetch|config|build|all]
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
V=7.2.7
SHA=4ac34c47db2540ffb2713943f8d891ff1702e0ba6934525a493b7d1cad43145a
SRC=$ROOT/target/nova-src/linux-$V
OUT=$ROOT/target/nova-build
MODS=$ROOT/target/nova-modules
export RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN:-stable}
MK=(make -C "$SRC" O="$OUT" CC=clang HOSTCC=clang -j"${JOBS:-4}")

fetch() {
    mkdir -p "$ROOT/target/nova-src"
    cd "$ROOT/target/nova-src"
    [[ -f linux-$V.tar.xz ]] || curl -sSfLO "https://cdn.kernel.org/pub/linux/kernel/v7.x/linux-$V.tar.xz"
    echo "$SHA  linux-$V.tar.xz" | sha256sum -c -
    [[ -d linux-$V ]] || tar xf "linux-$V.tar.xz"
}

config() {
    mkdir -p "$OUT"
    "${MK[@]}" x86_64_defconfig
    "${MK[@]}" kvm_guest.config
    "${MK[@]}" rustavailable
    # Rust, nova and what the Ubuntu 24.04 user space needs without an initramfs.
    "$SRC/scripts/config" --file "$OUT/.config" \
        -e RUST -d MODVERSIONS -d RANDSTRUCT_FULL -d RANDSTRUCT_PERFORMANCE -e RANDSTRUCT_NONE \
        -d DEBUG_INFO_BTF \
        -e DRM -m NOVA_CORE -m DRM_NOVA -d DRM_NOUVEAU \
        -e DEBUG_FS -e DYNAMIC_DEBUG \
        -e VIRTIO_PCI -e VIRTIO_BLK -e VIRTIO_NET -e VIRTIO_CONSOLE -e HW_RANDOM_VIRTIO \
        -e EXT4_FS -e VFAT_FS -e ISO9660_FS -e JOLIET -e FAT_DEFAULT_UTF8 \
        -e NLS_CODEPAGE_437 -e NLS_ISO8859_1 -e NLS_UTF8 \
        -e AUTOFS_FS -e FANOTIFY -e CGROUPS -e MEMCG -e CGROUP_BPF -e BPF_SYSCALL \
        -e DEVTMPFS -e DEVTMPFS_MOUNT -e TMPFS_POSIX_ACL -e TMPFS_XATTR \
        -e SECCOMP -e SECCOMP_FILTER -e EFI_PARTITION -e MSDOS_PARTITION \
        -e FW_LOADER -e FW_LOADER_COMPRESS -e FW_LOADER_COMPRESS_ZSTD -e FW_LOADER_COMPRESS_XZ \
        -e PVH -e SERIAL_8250 -e SERIAL_8250_CONSOLE \
        -e MODULE_COMPRESS_NONE -e IKCONFIG -e IKCONFIG_PROC \
        --set-str LOCALVERSION "-nova"
    "${MK[@]}" olddefconfig
    # olddefconfig silently drops what a dependency refuses; say which.
    for k in RUST NOVA_CORE DRM_NOVA DRM DRM_NOUVEAU AUXILIARY_BUS RUST_FW_LOADER_ABSTRACTIONS \
             VIRTIO_BLK EXT4_FS FW_LOADER_COMPRESS_ZSTD; do
        grep -E "^(# )?CONFIG_$k[= ]" "$OUT/.config" || echo "CONFIG_$k unset"
    done
    cp "$OUT/.config" "$HERE/kernel.config"
}

build() {
    "${MK[@]}" bzImage modules
    # target/nova-modules may be a link to the NAS (below): empty it, keep the link.
    mkdir -p "$MODS"
    find "$MODS" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
    "${MK[@]}" INSTALL_MOD_PATH="$MODS" INSTALL_MOD_STRIP=1 modules_install
    ls -la "$OUT/arch/x86/boot/bzImage"
    find "$MODS" -name 'nova*.ko*'
}

case ${1:-all} in
    fetch) fetch ;;
    config) config ;;
    build) build ;;
    all) fetch; config; build ;;
    *) echo "usage: $0 [fetch|config|build|all]" >&2; exit 2 ;;
esac

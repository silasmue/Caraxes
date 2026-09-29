#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The openrender guest kernel (docs/research/open-render-2026-09-29.md): the nova kernel of
# caraxes/runs/nova-2026-09-25 (Linux 7.2.7, same source, clang, rustc stable) with nouveau
# added as a module (CONFIG_DRM_NOUVEAU=m, trace level compiled in, no SVM). nova-core and
# nova-drm stay modules, so one kernel serves both guests; LOCALVERSION stays "-nova".
# The build tree starts as a copy of the nova tree, so only nouveau, the helpers it selects and
# vmlinux are rebuilt. Everything lives on the NAS:
#   $L/build-openrender      O= build tree (bzImage, modules)
#   $L/modules-openrender    INSTALL_MOD_PATH (lib/modules/7.2.7-nova)
# Parallelism: JOBS (default 2) under nice -n 19, beside the other agents' work.
# Usage: caraxes/runs/openrender-2026-09-29/build-kernel.sh [config|build|all]
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
L=/mnt/vmstore/caraxes/nova-kernel
SRC=$L/src/linux-7.2.7
OUT=$L/build-openrender
MODS=$L/modules-openrender
export RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN:-stable}
MK=(nice -n 19 make -C "$SRC" O="$OUT" CC=clang HOSTCC=clang -j"${JOBS:-2}")

config() {
    [[ -d $OUT ]] || cp -a "$L/build" "$OUT"
    cp "$HERE/../nova-2026-09-25/kernel.config" "$OUT/.config"
    "$SRC/scripts/config" --file "$OUT/.config" \
        -m DRM_NOUVEAU --set-val NOUVEAU_DEBUG 5 --set-val NOUVEAU_DEBUG_DEFAULT 3 \
        -d DRM_NOUVEAU_SVM -e DRM_NOUVEAU_BACKLIGHT
    "${MK[@]}" olddefconfig
    for k in DRM_NOUVEAU NOUVEAU_DEBUG DRM_GPUVM DRM_EXEC DRM_SCHED DRM_TTM_HELPER NOVA_CORE \
             DRM_NOVA FW_LOADER_COMPRESS_ZSTD; do
        grep -E "^(# )?CONFIG_$k[= ]" "$OUT/.config" || echo "CONFIG_$k unset"
    done
    cp "$OUT/.config" "$HERE/kernel.config"
}

build() {
    "${MK[@]}" bzImage modules
    mkdir -p "$MODS"
    find "$MODS" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
    "${MK[@]}" INSTALL_MOD_PATH="$MODS" INSTALL_MOD_STRIP=1 modules_install
    ls -la "$OUT/arch/x86/boot/bzImage"
    find "$MODS" -name 'nouveau.ko*' -o -name 'nova*.ko*'
    sha256sum "$OUT/arch/x86/boot/bzImage"
}

case ${1:-all} in
    config) config ;;
    build) build ;;
    all) config; build ;;
    *) echo "usage: $0 [config|build|all]" >&2; exit 2 ;;
esac

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# vm10, the open-driver render guest of docs/research/open-render-2026-09-29.md: a fresh
# overlay on the 615 base image (Ubuntu 24.04, Mesa 25.2.8 with NVK and lavapipe) with vm9's
# seed, offline edits only, no boot. It gets
#   - the modules of the Linux 7.2.7 openrender kernel (build-kernel.sh: nouveau and nova-core
#     as modules), from $KMODS (default /mnt/vmstore/caraxes/nova-kernel/modules-openrender),
#   - the r570.144 GSP firmware nouveau and nova request (nvidia/tu106/gsp -> ../tu102/gsp, the
#     host's linux-firmware files, zstd as packaged; the kernel has FW_LOADER_COMPRESS_ZSTD),
#     and nvidia/tu106/acr/bl.bin, the generic falcon bootloader nouveau loads a v2 FWSEC
#     descriptor with (Linux 7.2.7 nvkm/subdev/gsp/fwsec.c `nvkm_gsp_fwsec_v2`; nova uses
#     gen_bootloader instead; vm10-boot2: "fwsec-sb(v2): -2" without it),
#   - nouveau and nova_core blacklisted from autoload (openrender-check.sh loads nouveau), the
#     stock nvidia modules blacklisted (built for 6.8, they never load under 7.2.7 anyway),
#   - vk-offscreen (vk-offscreen/build.sh) in /usr/local/bin,
#   - when $REPO exists (fetch-weston-debs.sh), that local apt repository in
#     /var/cache/caraxes-repo and a source line for it in its own list file, which only the
#     check's weston install reads (the image's other sources are untouched),
#   - vm9's service masks (gdm, serial getty, network waits) and openrender-check.sh as
#     caraxes-check.service.
# The kernel itself is not in the image: the launcher boots the bzImage directly
# (CARAXES_KERNEL, CARAXES_INITRD=none). Rerun safe: the overlay is made once, the files are
# replaced every time.
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
S=/mnt/vmstore/caraxes
W=$S/vm10
KV=7.2.7-nova
KMODS=${KMODS:-$S/nova-kernel/modules-openrender}
BASE=${BASE:-$S/images/guest-baked-2026-09-24-615.71.09.qcow2}
REPO=${REPO:-$S/openrender-debs}
MODS=$KMODS/lib/modules/$KV
[[ -d $MODS ]] || { echo "ERROR: $MODS missing (build-kernel.sh build)" >&2; exit 1; }
[[ -n $(find "$MODS" -name 'nouveau.ko*' -print -quit) ]] || { echo "ERROR: no nouveau.ko under $MODS" >&2; exit 1; }
[[ -x $HERE/vk-offscreen/vk-offscreen ]] || { echo "ERROR: vk-offscreen/build.sh first" >&2; exit 1; }
if [[ ! -e $W/rootfs.qcow2 ]]; then
    mkdir -p "$W"
    qemu-img create -f qcow2 -F qcow2 -b "$BASE" "$W/rootfs.qcow2"
    cp "$S/vm9/seed.img" "$W/seed.img"
fi
tmp=$(mktemp -d -p "$S")
trap 'rm -rf "$tmp"' EXIT
tar -C "$KMODS/lib/modules" -cf "$tmp/modules.tar" "$KV"
tar -C / -chf "$tmp/firmware.tar" \
    lib/firmware/nvidia/tu102/gsp/booter_load-570.144.bin.zst \
    lib/firmware/nvidia/tu102/gsp/booter_unload-570.144.bin.zst \
    lib/firmware/nvidia/tu102/gsp/bootloader-570.144.bin.zst \
    lib/firmware/nvidia/tu102/gsp/gen_bootloader-570.144.bin.zst \
    lib/firmware/nvidia/tu102/gsp/gsp-570.144.bin.zst \
    lib/firmware/nvidia/tu106/acr/bl.bin.zst
printf '%s\n' "# Caraxes openrender guest: openrender-check.sh loads nouveau itself" \
    "blacklist nouveau" "blacklist nova_core" "blacklist nova_drm" "blacklist nvidia" \
    "blacklist nvidia_drm" "blacklist nvidia_modeset" "blacklist nvidia_uvm" > "$tmp/openrender.conf"
repo=()
if [[ -f $REPO/Release ]]; then
    tar -C "$REPO" --exclude=.index -cf "$tmp/repo.tar" .
    echo "deb [trusted=yes] file:/var/cache/caraxes-repo ./" > "$tmp/repo.list"
    repo=("rm-rf /var/cache/caraxes-repo" "mkdir-p /var/cache/caraxes-repo"
          "tar-in $tmp/repo.tar /var/cache/caraxes-repo"
          "upload $tmp/repo.list /etc/apt/sources.list.d/caraxes-openrender.list")
fi
# No systemd status lines on the console: the check runs for minutes and its serial output
# must not be interleaved with "Job ... running" updates (vm10-boot6).
printf '%s\n' "[Manager]" "ShowStatus=no" > "$tmp/showstatus.conf"
printf '%s\n' "[Unit]" "Description=Caraxes open-driver render check" "After=multi-user.target" "" \
    "[Service]" "Type=oneshot" "ExecStart=/usr/local/sbin/caraxes-check.sh" "" \
    "[Install]" "WantedBy=multi-user.target" > "$tmp/unit"
guestfish -a "$W/rootfs.qcow2" -i <<GF
rm-rf /lib/modules/$KV
tar-in $tmp/modules.tar /lib/modules
tar-in $tmp/firmware.tar /
mkdir-p /lib/firmware/nvidia/tu106
rm-rf /lib/firmware/nvidia/tu106/gsp
ln-sf ../tu102/gsp /lib/firmware/nvidia/tu106/gsp
upload $tmp/openrender.conf /etc/modprobe.d/caraxes-openrender.conf
mkdir-p /etc/systemd/system.conf.d
upload $tmp/showstatus.conf /etc/systemd/system.conf.d/caraxes-showstatus.conf
upload $tmp/unit /etc/systemd/system/caraxes-check.service
upload $HERE/openrender-check.sh /usr/local/sbin/caraxes-check.sh
chmod 0755 /usr/local/sbin/caraxes-check.sh
upload $HERE/vk-offscreen/vk-offscreen /usr/local/bin/vk-offscreen
chmod 0755 /usr/local/bin/vk-offscreen
ln-sf /etc/systemd/system/caraxes-check.service /etc/systemd/system/multi-user.target.wants/caraxes-check.service
ln-sf /dev/null /etc/systemd/system/gdm.service
ln-sf /dev/null /etc/systemd/system/nvidia-persistenced.service
ln-sf /dev/null /etc/systemd/system/serial-getty@ttyS0.service
ln-sf /dev/null /etc/systemd/system/NetworkManager-wait-online.service
ln-sf /dev/null /etc/systemd/system/systemd-networkd-wait-online.service
$(printf '%s\n' "${repo[@]}")
GF
guestfish --ro -a "$W/rootfs.qcow2" -i sh \
    "ls -la /lib/firmware/nvidia/tu106/ /lib/firmware/nvidia/tu106/acr/ /lib/firmware/nvidia/tu102/gsp/; find /lib/modules/$KV -name 'nouveau*' -o -name 'nova*'; cat /etc/modprobe.d/caraxes-openrender.conf; ls -la /usr/local/bin/vk-offscreen /usr/share/vulkan/icd.d/nouveau_icd.json; ls /var/cache/caraxes-repo 2>/dev/null | wc -l; cat /etc/apt/sources.list.d/caraxes-openrender.list 2>/dev/null"

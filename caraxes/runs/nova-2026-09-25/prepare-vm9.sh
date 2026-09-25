#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# vm9, the nova guest of docs/caraxes/nova-guest.md: a fresh overlay on the 610 base image
# (Ubuntu 24.04) with vm7's seed, offline edits only, no boot. It gets
#   - the modules of the Linux 7.2.7 nova kernel (build-kernel.sh: target/nova-modules),
#   - the r570.144 GSP firmware nova requests (nvidia/tu106/gsp -> ../tu102/gsp, the host's
#     linux-firmware files, zstd as packaged; the kernel has FW_LOADER_COMPRESS_ZSTD),
#   - nova_core blacklisted from autoload (nova-check.sh loads it with dyndbg) and the stock
#     nvidia modules blacklisted (they are built for 6.8 and never load under 7.2.7 anyway),
#   - vm7's service masks (gdm, serial getty, network waits) and nova-check.sh as
#     caraxes-check.service.
# The kernel itself is not in the image: the launcher boots target/nova-build's bzImage
# directly (CARAXES_KERNEL, CARAXES_INITRD=none). Rerun safe: the overlay is made once.
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
S=/mnt/vmstore/caraxes
W=$S/vm9
KV=7.2.7-nova
MODS=$ROOT/target/nova-modules/lib/modules/$KV
# shellcheck disable=SC1091
[[ -f $ROOT/local.env ]] && source "$ROOT/local.env"
: "${LEA_BASE_IMAGE:?LEA_BASE_IMAGE (the 610 base image) unset}"
[[ -d $MODS ]] || { echo "ERROR: $MODS missing (build-kernel.sh build)" >&2; exit 1; }
if [[ ! -e $W/rootfs.qcow2 ]]; then
    mkdir -p "$W"
    qemu-img create -f qcow2 -F qcow2 -b "$LEA_BASE_IMAGE" "$W/rootfs.qcow2"
    cp "$S/vm7/seed.img" "$W/seed.img"
fi
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
tar -C "$ROOT/target/nova-modules/lib/modules" -cf "$tmp/modules.tar" "$KV"
tar -C / -chf "$tmp/firmware.tar" \
    lib/firmware/nvidia/tu102/gsp/booter_load-570.144.bin.zst \
    lib/firmware/nvidia/tu102/gsp/booter_unload-570.144.bin.zst \
    lib/firmware/nvidia/tu102/gsp/bootloader-570.144.bin.zst \
    lib/firmware/nvidia/tu102/gsp/gen_bootloader-570.144.bin.zst \
    lib/firmware/nvidia/tu102/gsp/gsp-570.144.bin.zst
printf '%s\n' "# Caraxes nova guest: nova-check.sh loads nova_core itself" \
    "blacklist nova_core" "blacklist nova_drm" "blacklist nvidia" "blacklist nouveau" > "$tmp/nova.conf"
printf '%s\n' "[Unit]" "Description=Caraxes nova boot check" "After=multi-user.target" "" \
    "[Service]" "Type=oneshot" "ExecStart=/usr/local/sbin/caraxes-check.sh" "" \
    "[Install]" "WantedBy=multi-user.target" > "$tmp/unit"
guestfish -a "$W/rootfs.qcow2" -i <<GF
rm-rf /lib/modules/$KV
tar-in $tmp/modules.tar /lib/modules
tar-in $tmp/firmware.tar /
mkdir-p /lib/firmware/nvidia/tu106
rm-rf /lib/firmware/nvidia/tu106/gsp
ln-sf ../tu102/gsp /lib/firmware/nvidia/tu106/gsp
upload $tmp/nova.conf /etc/modprobe.d/caraxes-nova.conf
upload $tmp/unit /etc/systemd/system/caraxes-check.service
upload $HERE/nova-check.sh /usr/local/sbin/caraxes-check.sh
chmod 0755 /usr/local/sbin/caraxes-check.sh
ln-sf /etc/systemd/system/caraxes-check.service /etc/systemd/system/multi-user.target.wants/caraxes-check.service
ln-sf /dev/null /etc/systemd/system/gdm.service
ln-sf /dev/null /etc/systemd/system/nvidia-persistenced.service
ln-sf /dev/null /etc/systemd/system/serial-getty@ttyS0.service
ln-sf /dev/null /etc/systemd/system/NetworkManager-wait-online.service
ln-sf /dev/null /etc/systemd/system/systemd-networkd-wait-online.service
GF
guestfish --ro -a "$W/rootfs.qcow2" -i sh \
    "ls -la /lib/firmware/nvidia/tu106/ /lib/firmware/nvidia/tu102/gsp/; find /lib/modules/$KV -name 'nova*'; cat /etc/modprobe.d/caraxes-nova.conf"

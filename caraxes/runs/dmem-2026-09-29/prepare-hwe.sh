#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# On the host, with desktop-b-dmem up on its stock kernel (desktop-b-dmem.sh up; wait): install
# the HWE kernel in the guest (guest-hwe-kernel.sh), copy its vmlinuz and initrd to
# $W/boot (checked by sha256), install caraxes-dmem-cap in the guest and shut it down. The next
# `desktop-b-dmem.sh up` boots the HWE kernel. Log: $OUT/prepare.txt.
# Usage: prepare-hwe.sh [OUT]
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
V=$HERE/desktop-b-dmem.sh
W=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b-dmem
K=${K:-7.0.0-34-generic}
OUT=${1:-$HERE/prepare}
mkdir -p "$OUT" "$W/boot"
exec > >(tee -a "$OUT/prepare.txt") 2>&1
gssh() { timeout "${T:-120}" "$V" ssh "$@" 2> >(grep -v "unable to resolve" >&2); }
echo "$(date +%T) prepare: guest $(gssh uname -r)"
T=3600 gssh "K=$K bash -s" < "$HERE/guest-hwe-kernel.sh" || { echo "guest step failed"; exit 1; }
for f in vmlinuz-$K initrd.img-$K; do
    gssh "sudo cat /boot/$f" > "$W/boot/$f.part" && mv "$W/boot/$f.part" "$W/boot/$f"
done
guest_sums=$(gssh "cd /boot && sudo sha256sum vmlinuz-$K initrd.img-$K")
host_sums=$(cd "$W/boot" && sha256sum "vmlinuz-$K" "initrd.img-$K")
echo "$host_sums"
[[ $guest_sums == "$host_sums" ]] || { echo "ERROR: boot files differ from the guest's"; echo "$guest_sums"; exit 1; }
echo "boot files copied to $W/boot, sha256 equal to the guest's"
tar -C "$REPO/crates/vfio-user-nvgpu/guest" -cf - dmem-cap |
    gssh 'rm -rf ~/caraxes-dmem-cap && mkdir ~/caraxes-dmem-cap && tar -C ~/caraxes-dmem-cap --strip-components=1 -xf - && sudo ~/caraxes-dmem-cap/install.sh'
gssh 'sync; sudo systemctl poweroff' || true
sleep 20
"$V" down
echo "$(date +%T) prepared; next: $V up (kernel $K)"

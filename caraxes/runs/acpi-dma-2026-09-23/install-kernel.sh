#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Install the patched guest kernel (Ubuntu-6.8.0-136.136 + patches/linux/0001, LOCALVERSION
# -caraxesdma) from the host build into vm3, 2026-09-23. Host side; the guest must be up.
# 1. make modules_install into a staging dir, 2. the external-module build tree
# (scripts/package/install-extmod-build, as Debian's linux-headers package does),
# 3. rsync both plus bzImage/System.map/config into the guest, 4. in the guest: depmod,
# update-initramfs, update-grub (the stock 6.8.0-136-generic entry stays), rebuild
# nvidia.ko/nvidia-uvm.ko from ~/ogkm against the new tree, 5. copy the guest's initrd
# back so Cloud Hypervisor can boot the kernel directly (--kernel/--initramfs).
set -euo pipefail
S=${S:?kernel source tree}
O=${O:?kernel build tree}
G=${G:?gcc used for the build}
W=/mnt/vmstore/caraxes/vm3
KEY=/mnt/vmstore/leandro/vm/id_leandro
IP=192.168.103.23
SSH="ssh -i $KEY -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"
KREL=$(cat "$O/include/config/kernel.release")
echo "kernel release $KREL"
rm -rf "$O/stage" && mkdir -p "$O/stage/build"
make -C "$S" O="$O" CC="$G" HOSTCC=gcc LOCALVERSION= INSTALL_MOD_PATH="$O/stage" INSTALL_MOD_STRIP=1 modules_install > "$O/modules_install.log" 2>&1
rm -f "$O/stage/lib/modules/$KREL/build" "$O/stage/lib/modules/$KREL/source"
(cd "$O" && srctree="$S" SRCARCH=x86 KCONFIG_CONFIG=.config sh "$S/scripts/package/install-extmod-build" "$O/stage/build")
mkdir -p "$W/boot"
cp "$O/arch/x86/boot/bzImage" "$W/boot/vmlinuz-$KREL"
$SSH "leandro@$IP" "mkdir -p stage-kernel"
rsync -a --delete -e "$SSH" "$O/stage/lib/modules/$KREL/" "leandro@$IP:stage-kernel/modules/"
rsync -a --delete -e "$SSH" "$O/stage/build/" "leandro@$IP:stage-kernel/build/"
rsync -a -e "$SSH" "$O/arch/x86/boot/bzImage" "$O/System.map" "$O/.config" "leandro@$IP:stage-kernel/"
$SSH "leandro@$IP" "set -e; K=$KREL
    sudo rsync -a --delete stage-kernel/modules/ /lib/modules/\$K/
    sudo rsync -a --delete stage-kernel/build/ /usr/src/linux-headers-\$K/
    sudo ln -sfn /usr/src/linux-headers-\$K /lib/modules/\$K/build
    sudo install -m 600 stage-kernel/bzImage /boot/vmlinuz-\$K
    sudo install -m 600 stage-kernel/System.map /boot/System.map-\$K
    sudo install -m 644 stage-kernel/.config /boot/config-\$K
    sudo depmod -a \$K
    sudo update-initramfs -c -k \$K 2>&1 | tail -n 3
    sudo update-grub 2>&1 | grep -E 'Found linux image|done'
    cd ~/ogkm/open-gpu-kernel-modules && make clean >/dev/null 2>&1 || true
    nice make modules -j8 SYSSRC=/lib/modules/\$K/build > ~/build-\$K.log 2>&1 || { tail -n 30 ~/build-\$K.log; exit 1; }
    sudo mkdir -p /lib/modules/\$K/updates
    sudo install -m 644 kernel-open/nvidia.ko kernel-open/nvidia-uvm.ko /lib/modules/\$K/updates/
    sudo depmod -a \$K
    modinfo -k \$K nvidia | grep -E '^(filename|version|vermagic)'
    sudo cp /boot/initrd.img-\$K /tmp/initrd.img-\$K && sudo chmod 644 /tmp/initrd.img-\$K"
rsync -a -e "$SSH" "leandro@$IP:/tmp/initrd.img-$KREL" "$W/boot/initrd.img-$KREL"
ls -la "$W/boot"

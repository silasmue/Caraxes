#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# In the guest (desktop-b-dmem on its stock 6.8 kernel), as the ssh user with sudo: install
# Ubuntu's HWE kernel with `apt-get install linux-generic-hwe-24.04` (the stock Ubuntu path;
# 7.0.0-34 on 2026-09-29), which makes DKMS build the packaged NVIDIA 615.71.09 open modules for
# it, and check the result. Public Ubuntu repositories only, through the host's NAT (the rules of
# alpha-2026-09-27/desktop-b/sottr-nat). Run: desktop-b-dmem.sh ssh 'bash -s' < guest-hwe-kernel.sh
set -uo pipefail
K=${K:-7.0.0-34-generic}
step() { echo "== $*"; }
step "before: kernel $(uname -r); $(dpkg-query -W -f '${Package} ${Version}\n' nvidia-dkms-open 2>/dev/null)"
df -h / | tail -n 1
avail=$(df --output=avail -k / | tail -n 1)
(( avail > 4 * 1024 * 1024 )) || { echo "ERROR: less than 4 GiB free on /"; exit 1; }
sudo ip route replace default via 192.168.106.1 dev eth0 && sudo resolvectl dns eth0 192.168.188.1 && sudo resolvectl domain eth0 "~."
step "apt-get update"
sudo apt-get update -q 2>&1 | tail -n 3
apt-cache policy linux-generic-hwe-24.04 | head -n 3
step "apt-get install linux-generic-hwe-24.04 (log: /var/tmp/hwe-install.log)"
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y linux-generic-hwe-24.04 > /tmp/hwe-install.out 2>&1
rc=$?
sudo cp /tmp/hwe-install.out /var/tmp/hwe-install.log
grep -E "^(Setting up linux|E:)|dkms|DKMS|nvidia" /tmp/hwe-install.out | tail -n 30
echo "apt-get rc=$rc"
step "dkms"
dkms status 2>&1
ls -la /lib/modules/"$K"/updates/dkms/ 2>&1
for m in /lib/modules/"$K"/updates/dkms/nvidia.ko*; do
    [[ -e $m ]] || continue
    echo "$m: $(modinfo -F version "$m") vermagic $(modinfo -F vermagic "$m")"
    # dmem_cgroup_* imported means conftest found NV_DMEM_CGROUP_PRESENT.
    echo "dmem symbols: $(case $m in *.zst) zstd -dcq "$m";; *.xz) xz -dcq "$m";; *) cat "$m";; esac | strings | grep -c '^dmem_cgroup_')"
done
step "kernel config and boot files"
grep -E "^CONFIG_CGROUP_(DMEM|MISC)=" /boot/config-"$K"
sudo sha256sum /boot/vmlinuz-"$K" /boot/initrd.img-"$K"
echo "nvidia files in the initrd: $(lsinitramfs /boot/initrd.img-"$K" 2>/dev/null | grep -c nvidia)"
grep -H . /etc/modprobe.d/*caraxes* 2>/dev/null
exit $rc

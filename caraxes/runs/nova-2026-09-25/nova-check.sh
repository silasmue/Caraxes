#!/bin/sh
# SPDX-License-Identifier: MIT
# The boot-only nova check (docs/caraxes/nova-guest.md), run once per boot in vm9 as
# caraxes-check.service: loads nova-core with its debug lines, reads the probe's outcome, the
# DRM side (nova-drm, /dev/dri, the DRM driver name), then N4: unbind, bind again and a module
# reload, three cycles without a device reset; everything onto the serial console, then power
# off. nova_core is blacklisted from autoload in the image, so this script decides when it binds.
exec > /dev/ttyS0 2>&1
echo "CARAXES-CHECK begin $(date -Is) $(uname -r)"
echo 1 > /proc/sys/kernel/sysrq
( sleep 300; echo "CARAXES-CHECK watchdog: forced power-off"; sync; echo o > /proc/sysrq-trigger ) &
lspci -nn -d 10de:
dev=$(lspci -D -n -d 10de: | awk '{print $1; exit}')
echo "CARAXES-CHECK firmware"
ls -la /lib/firmware/nvidia/tu106/gsp/ 2>&1
mount -t debugfs none /sys/kernel/debug 2>/dev/null
echo "CARAXES-CHECK probe 1"
modprobe nova_core dyndbg=+p; echo "modprobe nova_core rc=$?"
sleep 3
dmesg | grep -iE "nova|gsp|wpr2|fwsec|booter|risc-v" | tail -n 80
echo "CARAXES-CHECK drm"
modprobe nova_drm; echo "modprobe nova_drm rc=$?"
sleep 1
ls -la /dev/dri 2>&1
for c in /sys/class/drm/*; do echo "$c: $(cat "$c/device/uevent" 2>/dev/null | tr '\n' ' ')"; done
cat /sys/kernel/debug/dri/*/name 2>/dev/null
command -v modetest >/dev/null && timeout 20 modetest -M nova-drm -c 2>&1 | head -n 20
for i in 2 3 4; do
    echo "CARAXES-CHECK cycle $i unbind/bind"
    echo "$dev" > /sys/bus/pci/drivers/nova-core/unbind; echo "unbind rc=$?"
    sleep 2
    echo "$dev" > /sys/bus/pci/drivers/nova-core/bind; echo "bind rc=$?"
    sleep 3
    dmesg | grep -iE "nova" | tail -n 12
done
echo "CARAXES-CHECK reload"
rmmod nova_drm nova_core; echo "rmmod rc=$?"
modprobe nova_core dyndbg=+p; echo "modprobe nova_core rc=$?"
sleep 3
echo "CARAXES-CHECK dmesg-after"
dmesg | grep -iE "nova|gsp" | tail -n 60
echo "CARAXES-CHECK end"
sync
systemctl poweroff

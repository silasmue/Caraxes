#!/bin/sh
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The open-driver render check (docs/research/open-render-2026-09-29.md), run once per boot in
# vm10 as caraxes-check.service: loads nouveau (GSP-RM 570.144 on the synthetic TU106) with
# its GSP debug lines, reports the probe, the kernel's own channels (accel), the DRM nodes and
# NVK, then runs vk-offscreen (a 3D clear plus a copy, then one triangle, both read back and
# compared byte by byte) and prints its image as gzip+base64 between markers, so the host can
# rebuild it from serial.log (extract-ppm.sh). Everything goes to the serial console; then
# power off. nouveau is blacklisted from autoload in the image, so this script decides when
# it binds. Rungs: R1 probe, R2 kernel channels, R3 NVK device, R4 clear+copy, R5 triangle.
exec > /dev/ttyS0 2>&1
echo "CARAXES-CHECK begin $(date -Is) $(uname -r)"
echo 1 > /proc/sys/kernel/sysrq
( sleep 420; echo "CARAXES-CHECK watchdog: forced power-off"; sync; echo o > /proc/sysrq-trigger ) &
lspci -nn -d 10de:
mount -t debugfs none /sys/kernel/debug 2>/dev/null
echo "CARAXES-CHECK firmware"
ls -la /lib/firmware/nvidia/tu106/gsp/ 2>&1
echo "CARAXES-CHECK R1 probe"
# debug=gsp=debug: the RPC and event lines of rm/r535/rpc.c; the rest at the default level.
modprobe nouveau debug=gsp=debug; echo "modprobe nouveau rc=$?"
sleep 5
dmesg | grep -iE "nouveau|gsp|drm" | tail -n 150
echo "CARAXES-CHECK R2 accel"
dmesg | grep -iE "nouveau.*(accel|channel|fence|DRM: )" | tail -n 20
ls -la /dev/dri 2>&1
for c in /sys/class/drm/*; do echo "$c: $(tr '\n' ' ' < "$c/device/uevent" 2>/dev/null)"; done
cat /sys/kernel/debug/dri/*/name 2>/dev/null
echo "CARAXES-CHECK R3 nvk"
timeout 60 vulkaninfo --summary 2>&1 | grep -E "deviceName|driverName|driverInfo|driverID|apiVersion|ERROR|error" | head -n 20
echo "CARAXES-CHECK R4-R5 vk-offscreen"
timeout 120 /usr/local/bin/vk-offscreen /var/tmp/vk-offscreen.ppm; echo "vk-offscreen rc=$?"
if [ -s /var/tmp/vk-offscreen.ppm ]; then
    echo "CARAXES-PPM-BEGIN"
    gzip -9c /var/tmp/vk-offscreen.ppm | base64 -w 76
    echo "CARAXES-PPM-END"
fi
echo "CARAXES-CHECK dmesg-after"
dmesg | grep -iE "nouveau|gsp|fault|xid|timeout|channel" | tail -n 80
echo "CARAXES-CHECK unbind"
dev=$(lspci -D -n -d 10de: | awk '{print $1; exit}')
echo "$dev" > /sys/bus/pci/drivers/nouveau/unbind; echo "unbind rc=$?"
sleep 3
dmesg | grep -iE "nouveau|gsp" | tail -n 20
echo "CARAXES-CHECK end"
sync
systemctl poweroff

#!/bin/sh
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The open-driver render check (docs/research/open-render-2026-09-29.md), run once per boot in
# vm10 as caraxes-check.service: loads nouveau (GSP-RM 570.144 on the synthetic TU106) with
# its GSP debug lines, reports the probe, the kernel's own channels (accel), the DRM nodes and
# NVK, then runs vk-offscreen (a 3D clear plus a copy, then one triangle, both read back and
# compared byte by byte) and prints its image as gzip+base64 between markers, so the host can
# rebuild it from serial.log (extract-ppm.py). Everything goes to the serial console; then
# power off. nouveau is blacklisted from autoload in the image, so this script decides when
# it binds. Rungs: R1 probe, R2 kernel channels, R3 NVK device, R4 clear+copy, R5 triangle,
# R6 headless scenes (vkmark, vkcube, where the image's builds have a headless WSI), R7 GL
# through EGL surfaceless.
exec > /dev/ttyS0 2>&1
echo "CARAXES-CHECK begin $(date -Is) $(uname -r)"
echo 1 > /proc/sys/kernel/sysrq
( sleep 600; echo "CARAXES-CHECK watchdog: forced power-off"; sync; echo o > /proc/sysrq-trigger ) &
lspci -nn -d 10de:
mount -t debugfs none /sys/kernel/debug 2>/dev/null
echo "CARAXES-CHECK firmware"
ls -la /lib/firmware/nvidia/tu106/gsp/ 2>&1
echo "CARAXES-CHECK R1 probe"
# debug=gsp=debug: the RPC and event lines of rm/r535/rpc.c; the rest at the default level.
# The image's NVIDIA packaging keeps nouveau off in modprobe.d (vm10-boot1: modprobe resolved
# "nouveau" to a module "off", i.e. "alias nouveau off"; the grep below records the lines). Load
# it without any modprobe.d configuration (-C /dev/null: no aliases, blacklists or options from
# files; dependencies still come from modules.dep), with the options given here only.
grep -rsn "nouveau" /etc/modprobe.d /lib/modprobe.d /usr/lib/modprobe.d /run/modprobe.d | head -n 10
modprobe -C /dev/null nouveau debug=gsp=debug; echo "modprobe nouveau rc=$?"
sleep 5
dmesg | grep -iE "nouveau|gsp|drm" | tail -n 150
echo "CARAXES-CHECK R2 accel"
dmesg | grep -iE "nouveau.*(accel|channel|fence|DRM: )" | tail -n 20
ls -la /dev/dri 2>&1
for c in /sys/class/drm/*; do echo "$c: $(tr '\n' ' ' < "$c/device/uevent" 2>/dev/null)"; done
cat /sys/kernel/debug/dri/*/name 2>/dev/null
echo "CARAXES-CHECK packages"
dpkg-query -W -f '${Package} ${Version}\n' mesa-vulkan-drivers libgl1-mesa-dri vulkan-tools vkmark \
    glmark2 glmark2-wayland weston mutter mesa-utils 2>&1 | head -n 12
echo "CARAXES-CHECK R3 nvk"
timeout 60 vulkaninfo --summary 2>&1 | grep -E "deviceName|driverName|driverInfo|driverID|apiVersion|ERROR|error" | head -n 20
echo "CARAXES-CHECK R4-R5 vk-offscreen"
timeout 120 /usr/local/bin/vk-offscreen /var/tmp/vk-offscreen.ppm; echo "vk-offscreen rc=$?"
if [ -s /var/tmp/vk-offscreen.ppm ]; then
    echo "CARAXES-PPM-BEGIN"
    gzip -9c /var/tmp/vk-offscreen.ppm | base64 -w 76
    echo "CARAXES-PPM-END"
fi
# R6: moving scenes without a window system, where the image's tools offer one: vkmark's
# headless winsys plugin (VK_EXT_headless_surface), then vkcube's --wsi headless. Frames are
# rendered and presented to nothing; FPS and exit codes are the result.
echo "CARAXES-CHECK R6 headless scenes"
ls /usr/lib/x86_64-linux-gnu/vkmark/ 2>&1 | tr '\n' ' '; echo
if ls /usr/lib/x86_64-linux-gnu/vkmark/ 2>/dev/null | grep -q headless; then
    timeout 120 vkmark --winsys headless -s 640x480 -b cube:duration=5 -b clear:duration=3 > /var/tmp/vkmark.txt 2>&1
    echo "vkmark rc=$?"
    grep -E "^\[|FPS|Score|rror|ERR|not supported" /var/tmp/vkmark.txt | head -n 20
else
    echo "vkmark: no headless winsys in this image"
fi
vkcube --help 2>&1 | grep -iE "wsi|headless" | head -n 4
if vkcube --help 2>&1 | grep -qi headless; then
    t0=$(date +%s%N)
    timeout 60 vkcube --wsi headless --c 60 > /var/tmp/vkcube.txt 2>&1
    rc=$?
    echo "vkcube headless --c 60 rc=$rc in $(( ($(date +%s%N) - t0) / 1000000 )) ms"
    tail -n 5 /var/tmp/vkcube.txt
else
    echo "vkcube: no headless WSI in this image"
fi
# R7: OpenGL on the render node without a window system (EGL surfaceless; for Turing Mesa 25.x
# runs GL as zink on NVK): the renderer and version it reports.
echo "CARAXES-CHECK R7 egl surfaceless"
if command -v eglinfo >/dev/null 2>&1; then
    timeout 60 eglinfo -B -p surfaceless 2>&1 | grep -iE "renderer|version|vendor|error|fail" | head -n 12
    echo "eglinfo rc=$?"
else
    echo "eglinfo not installed"
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

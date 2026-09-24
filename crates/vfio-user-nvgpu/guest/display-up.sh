#!/bin/bash
# SPDX-License-Identifier: MIT
# Guest: load the display modules after `tools/caraxes-vm.sh probe` loaded nvidia.ko
# (docs/caraxes/display-cd.md): video, nvidia-modeset, nvidia-drm modeset=1 fbdev=0, and
# the device nodes the image lacks (it has no nvidia-modprobe). Prints the connector state.
set -u
modprobe video
modprobe nvidia-modeset || exit 1
modprobe nvidia-drm modeset=1 fbdev=0 || exit 1
[ -e /dev/nvidiactl ] || mknod -m 666 /dev/nvidiactl c 195 255
[ -e /dev/nvidia0 ] || mknod -m 666 /dev/nvidia0 c 195 0
[ -e /dev/nvidia-modeset ] || mknod -m 666 /dev/nvidia-modeset c 195 254
# nvidia-uvm and its nodes: the NVIDIA Vulkan driver opens /dev/nvidia-uvm for a device
# with VK_KHR_acceleration_structure (display-e.md, the display gate's rt stage).
if modprobe nvidia-uvm 2>/dev/null; then
    M=$(grep nvidia-uvm /proc/devices | cut -d' ' -f1)
    [ -e /dev/nvidia-uvm ] || mknod -m 666 /dev/nvidia-uvm c "$M" 0
    [ -e /dev/nvidia-uvm-tools ] || mknod -m 666 /dev/nvidia-uvm-tools c "$M" 1
fi
for _ in $(seq 20); do [ -e /dev/dri/card0 ] && break; sleep 0.5; done
sleep 2
for c in /sys/class/drm/card0-*; do echo "$(basename "$c") $(cat "$c/status") $(tr '\n' ' ' < "$c/modes")"; done

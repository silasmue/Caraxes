#!/bin/bash
# SPDX-License-Identifier: MIT
# Guest (root), phase D of docs/caraxes/display-cd.md: record the connector before and
# after a host-side EDID change (the host writes `edid-alt` into <state>/display-ctl
# while this waits), with the udev events nvidia-drm sends.
# Usage: hotplug-watch.sh [seconds]
set -u
SECS=${1:-40}
C=/sys/class/drm/card0-DP-1
show() { echo "$1: status $(cat $C/status), edid $(md5sum < $C/edid | cut -c1-12) $(stat -c%s $C/edid) B, modes: $(tr '\n' ' ' < $C/modes)"; }
show before
timeout "$SECS" udevadm monitor --kernel --subsystem-match=drm > /tmp/hotplug-udev.txt 2>&1 &
sleep "$SECS"
show after
echo "== udev (kernel, drm)"; grep -E "change|add|remove" /tmp/hotplug-udev.txt | head -10
echo "== edid-decode (name, established)"; edid-decode $C/edid 2>/dev/null | grep -E "Display Product Name|800x600|Manufacturer|Product" | head -6
echo "== dmesg"; dmesg | grep -v "NVRM: VM:" | grep -E "WARNING|ERROR|hotplug|nvidia-drm|DP" | tail -8

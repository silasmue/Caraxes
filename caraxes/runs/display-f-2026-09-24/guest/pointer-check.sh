#!/bin/bash
# SPDX-License-Identifier: MIT
# Series F (guest, desktop user): the virtio-input pointer of CARAXES_INPUT=fifo.
#   pointer-check.sh start [secs]  GNOME up if needed, record the device's evdev node for secs
#   pointer-check.sh collect       what arrived: events, and what the session saw
set -u
cd ~/caraxes-guest || exit 1
node=$(grep -A5 'Name="caraxes-vm' /proc/bus/input/devices | sed -n 's/^H: Handlers=.*\(event[0-9]*\).*/\1/p' | head -1)
case ${1:-start} in
start)
    if ! pgrep -x gnome-shell >/dev/null; then
        sudo bash gnome-check.sh start 60 | grep -E "Type=wayland|gbm renderer|page_flip"
    fi
    echo "node /dev/input/$node"
    sudo sh -c "timeout ${2:-10} cat /dev/input/$node > /tmp/pointer-ev.bin" >/dev/null 2>&1 &
    sleep 1
    echo recording
    ;;
collect)
    sleep 1
    n=$(( $(stat -c %s /tmp/pointer-ev.bin) / 24 ))
    echo "evdev events recorded on /dev/input/$node: $n"
    od -A n -t d4 -w24 /tmp/pointer-ev.bin | awk '{print "type", $5 % 65536, "code", int($5 / 65536), "value", $6}' | sort | uniq -c | sort -rn | head -6
    echo "== udev / libinput view"
    udevadm info "/dev/input/$node" | grep -E "ID_INPUT(_MOUSE|_KEYBOARD)?=|NAME="
    echo "== the session's seat"
    loginctl seat-status seat0 2>/dev/null | grep -iE "input|caraxes" | head -5
    ;;
esac

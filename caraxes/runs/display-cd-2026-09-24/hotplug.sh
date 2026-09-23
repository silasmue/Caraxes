#!/bin/bash
# SPDX-License-Identifier: MIT
# Host side of the EDID-change hotplug (docs/caraxes/display-cd.md, phase D):
#   bash caraxes/runs/display-cd-2026-09-24/hotplug.sh <label> <command>
# The guest watches the connector (cx/hotplug-watch.sh) while the host writes <command>
# (edid-alt, edid-main, unplug, plug) into the device's control file after 10 s.
set -u
HERE=$(cd "$(dirname "$0")" && pwd -P)
bash "$HERE/exp.sh" "$1" 120 'sudo bash ./cx/hotplug-watch.sh 40' &
E=$!
sleep 10
echo "$2" > /mnt/vmstore/caraxes/vm0/display-ctl
echo "host: wrote '$2' to /mnt/vmstore/caraxes/vm0/display-ctl at $(date -Iseconds)" | tee "$HERE/$1-host.txt"
wait $E
cat "$HERE/$1/guest.txt"
grep -oE "display: (connector command|tx seq|hotplug)[^[]*" "$HERE/$1/device-trace.txt" | head
grep -cE "ctrl 0x730401|0x73028c|DP_AUXCH" "$HERE/$1/device-trace.txt"

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Series F, f6: move the guest pointer from the host through the CARAXES_INPUT=fifo virtio-input
# device (60 relative moves of +12/+6, 50 ms apart) while GNOME runs, and keep what arrived:
# the guest's evdev record, udev's classification, and the device's display events of the
# window (cursor channel updates). Run from the worktree root with vm0 up (CARAXES_INPUT=fifo).
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
D=$here/${1:-f6-pointer}
VMSH=("$here/vm.sh")
mkdir -p "$D"
tar -C "$here/guest" -cf - . | "${VMSH[@]}" ssh 'mkdir -p caraxes-f && tar -C caraxes-f -xf -'
"${VMSH[@]}" ssh 'bash caraxes-f/pointer-check.sh start 12' > "$D/guest-start.txt" 2>&1
b0=$(stat -c %s /mnt/vmstore/caraxes/vm0/gpu-trace.log)
date '+%F %T moves start' > "$D/host.txt"
for _ in $(seq 60); do
    "${VMSH[@]}" input '2 0 12' '2 1 6' '0 0 0'
    sleep 0.05
done
date '+%F %T moves end' >> "$D/host.txt"
sleep 12
"${VMSH[@]}" ssh 'bash caraxes-f/pointer-check.sh collect' > "$D/guest-collect.txt" 2>&1
tail -c +$((b0 + 1)) /mnt/vmstore/caraxes/vm0/gpu-trace.log | grep -a ' E ' > "$D/device-events.txt"
cat "$D/guest-start.txt" "$D/guest-collect.txt"
awk '{print $4, $5, $6}' "$D/device-events.txt" | sort | uniq -c | sort -rn | head -12

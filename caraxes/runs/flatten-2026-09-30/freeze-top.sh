#!/usr/bin/env bash
# 2026-09-30 00:00: the Windows top layer has one refcount error from the disk-full write at 22:48
# (cluster 650549 referenced with refcount 0; 6965 leaks). Reads go through the L2 tables and stay
# correct; only new allocations in that file could reuse the cluster. So the file is never written
# again: renamed, read-only, under a fresh overlay. No repair of the damaged file itself.
set -uo pipefail
W=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain
TOP=$W/win11.qcow2 FROZEN=$W/win11-pre-repair-2026-09-29.qcow2
LOG=/mnt/vmstore/caraxes/runs-2026-09-30/flatten/snapshot-before-repair.log
[[ -e $FROZEN ]] && { echo "frozen layer already exists" | tee -a "$LOG"; exit 1; }
mv "$TOP" "$FROZEN" && chmod a-w "$FROZEN" &&
qemu-img create -q -f qcow2 -b "$FROZEN" -F qcow2 "$TOP" >> "$LOG" 2>&1 || { echo "overlay FAILED" | tee -a "$LOG"; exit 1; }
echo "$(date +%T) froze $FROZEN under a new overlay" | tee -a "$LOG"
qemu-img info --backing-chain "$TOP" | grep -E '^image|disk size|backing file:' | tee -a "$LOG"

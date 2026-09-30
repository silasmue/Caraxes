#!/usr/bin/env bash
# 2026-09-30 ~00:00: after the host disk-full at 22:48 damaged the Windows C:, check the qcow2 top
# layer read-only, then freeze it (renamed, read-only) under a fresh overlay so the repair writes
# only into the overlay and the damaged state stays as a fallback.
set -uo pipefail
W=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain
TOP=$W/win11.qcow2 FROZEN=$W/win11-pre-repair-2026-09-29.qcow2
LOG=/mnt/vmstore/caraxes/runs-2026-09-30/flatten/snapshot-before-repair.log
qemu-img check -f qcow2 "$TOP" > "$LOG" 2>&1; rc=$?
echo "qemu-img check rc $rc (0 clean, 3 leaks only, 2 corruptions)" | tee -a "$LOG"
(( rc == 0 || rc == 3 )) || { echo "not freezing: check rc $rc" | tee -a "$LOG"; exit 1; }
[[ -e $FROZEN ]] && { echo "frozen layer already exists" | tee -a "$LOG"; exit 1; }
mv "$TOP" "$FROZEN" && chmod a-w "$FROZEN" &&
qemu-img create -q -f qcow2 -b "$FROZEN" -F qcow2 "$TOP" >> "$LOG" 2>&1 || { echo "overlay FAILED" | tee -a "$LOG"; exit 1; }
qemu-img info --backing-chain "$TOP" | grep -E '^image|disk size' | tee -a "$LOG"

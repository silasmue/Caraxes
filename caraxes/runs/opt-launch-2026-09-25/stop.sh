#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# End a part that was stopped early: the VM down (its device log kept as
# <dir>/stopped-device-log.txt), the host watch stopped, the host Xid lines since T0.
# Usage: stop.sh LABEL 'YYYY-MM-DD HH:MM:SS'
set -uo pipefail
source "$(dirname "$(readlink -f "$0")")/env.sh"
C=$OL_HERE/${1:?label}
T0=${2:?start time}
cd "$OL_ROOT" || exit 1
pid=$(cat "$OL_W/gpu.pid" 2>/dev/null)
"$LEA_TOOLS/caraxes-vm.sh" down > "$C/down.txt" 2>&1
cp "$OL_W/gpu.log" "$C/stopped-device-log.txt" 2>/dev/null
caraxes/tools/bench-hostwatch.sh stop "$C"
{ echo "# host kernel log, Xid lines since $T0 (last device pid $pid; each rep's pid in */v2-device-rep*.txt)"
  journalctl -k --since "$T0" --no-pager -g Xid 2>/dev/null | grep -v '^-- No entries'; } > "$C/host-xid.txt"
echo "$(basename "$C") stopped $(date -Iseconds): host Xid lines $(grep -vc '^#' "$C/host-xid.txt"), $VM processes left $(pgrep -fc "/mnt/vmstore/caraxes/$VM/")"

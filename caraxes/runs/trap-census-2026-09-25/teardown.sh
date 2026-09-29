#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Tear down a run of run.sh that had to be stopped by hand (a hung guest process): stop its
# host watch, bring vm3 down, keep the device summary and the host Xid lines since T0.
# Usage: teardown.sh <run dir> "<T0 as YYYY-MM-DD HH:MM:SS>"
set -uo pipefail
C=${1:?run dir}; T0=${2:?T0}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
cd "$ROOT" || exit 1
export VM=vm3 CARAXES_IP=192.168.103.23 CARAXES_TAP=caraxes3 CARAXES_MAC=02:00:00:00:00:23
W=/mnt/vmstore/caraxes/$VM
caraxes/tools/bench-hostwatch.sh stop "$C"
tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
grep -E 'intr page|bar2 memfd|bar1 pool|bar1 passthrough|doorbell|mirror: |shadow: |syncs \(|ERROR|WARN' "$W/gpu.log" | tail -40 | cut -c1-1200 > "$C/device-summary.txt"
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
echo "teardown $C: host Xid lines $(grep -c Xid "$C/host-xid.txt"); vm3 left: $(pgrep -f /mnt/vmstore/caraxes/vm3 | wc -l)"

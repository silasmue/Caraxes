#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Morning matrix, v1 part: Leandro v1 on the 615 image (docs/caraxes/v1-615-image.md), the smi and
# cuda gates of caraxes/tools/v1-gates.sh. bench-v1vm.sh config, prepare (only when the overlay
# is missing: prepare refuses an existing one, and the one of 2026-09-24 is reused), up, modules,
# the gates, down. Tap caraxes5: refuses while vm5 or vm5-615 runs. The worktree has no
# vendor/cloud-hypervisor build, so CH names the main checkout's (the one bench-v1vm.sh's
# default resolves to there). Host watch around the part, host Xid lines with the backend pid.
# Usage: v1.sh [label]
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
C=$HERE/v1${1:+-$1}
[[ -e $C ]] && { echo "ERROR: $C exists" >&2; exit 2; }
if pgrep -f /mnt/vmstore/caraxes/vm5 >/dev/null || pgrep -f bench-v1 >/dev/null; then
    echo "ERROR: a vm5 process or the v1 bench VM runs; refusing to start" >&2; exit 2
fi
export LEA_V1_ROOT=/mnt/vmstore/leandro/wt-615 LEA_V1_BIN=/mnt/vmstore/leandro/l615-host-bin-8c0d2b2
export CH=${CH:-/home/silas/git/Caraxes/vendor/cloud-hypervisor/target/release/cloud-hypervisor}
V1=$ROOT/caraxes/tools/bench-v1vm.sh
mkdir -p "$C"; cd "$ROOT" || exit 1
T0=$(date '+%F %T')
caraxes/tools/bench-hostwatch.sh start "$C"
"$V1" config > "$C/config.txt" 2>&1
V=$(awk '$1 == "V" { print $2 }' "$C/config.txt")
if [[ ! -f $V/rootfs.qcow2 ]]; then "$V1" prepare >> "$C/prepare.txt" 2>&1; else echo "overlay $V/rootfs.qcow2 reused ($(stat -c %y "$V/rootfs.qcow2"))" > "$C/prepare.txt"; fi
if "$V1" up > "$C/up.txt" 2>&1 && "$V1" modules > "$C/modules.txt" 2>&1; then
    "$V1" pids > "$C/pids.txt" 2>&1
    caraxes/tools/v1-gates.sh "$C/gates" all > "$C/gates.txt" 2>&1; echo "v1 gates rc=$? (failed rows)"
    tail -5 "$C/gates.txt"
else
    echo "v1 did not come up (up.txt, modules.txt)"; tail -5 "$C/up.txt" "$C/modules.txt"
fi
caraxes/tools/bench-hostwatch.sh stop "$C"
pid=$(cut -d' ' -f1 "$C/pids.txt" 2>/dev/null)
"$V1" down > "$C/down.txt" 2>&1
{ echo "# host kernel log, Xid lines since $T0 (backend pid $pid)"
  journalctl -k --since "$T0" --no-pager -g Xid 2>/dev/null | grep -v '^-- No entries'; } > "$C/host-xid.txt"
echo "v1 done $(date -Iseconds): backend pid $pid, host Xid lines $(grep -vc '^#' "$C/host-xid.txt"), bench-v1 processes left $(pgrep -fc bench-v1)"

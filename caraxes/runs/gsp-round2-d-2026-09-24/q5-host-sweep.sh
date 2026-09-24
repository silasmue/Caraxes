#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Measurement Q5 (d-device-model.md §8): the one read-only sweep of the host's ZBC clear
# table, as an unprivileged RM client (caraxes/tools/zbc-probe.rs without --set: RM_ALLOC
# of root, device, subdevice and GF100_ZBC_CLEAR, GET_ZBC_CLEAR_TABLE_SIZE and
# GET_ZBC_CLEAR_TABLE_ENTRY, RM_FREE). Authorised by the operator for round 2.
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
SRC=$(cd "$HERE/../../.." && pwd -P)
D=$HERE/q5-host-sweep
[[ -e $D/sweep.txt ]] && { echo "ERROR: $D/sweep.txt exists; the sweep runs once" >&2; exit 2; }
mkdir -p "$D"
BIN=$SRC/target/zbc-probe
rustc -O --edition 2021 -C target-feature=+crt-static "$SRC/caraxes/tools/zbc-probe.rs" -o "$BIN" || exit 1
T0=$(date '+%F %T')
{ date -Iseconds; head -1 /proc/driver/nvidia/version; sha256sum "$BIN" "$SRC/caraxes/tools/zbc-probe.rs"; } > "$D/setup.txt"
"$BIN" > "$D/sweep.txt" 2>&1
echo "rc=$?" >> "$D/sweep.txt"
journalctl -k --since "$T0" --no-pager -g 'Xid|NVRM' > "$D/host-kernel-log.txt" 2>&1
cat "$D/sweep.txt"
echo "host kernel lines (Xid|NVRM) since $T0: $(grep -cE 'Xid|NVRM' "$D/host-kernel-log.txt")"

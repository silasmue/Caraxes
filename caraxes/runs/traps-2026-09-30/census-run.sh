#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Start census-linux.sh on desktop-b-dmem once no priority file names another guest, holding
# Linux B's guest lock too (same IP, tap and VRAM slot as desktop-b-dmem).
# Usage: census-run.sh OUT BINARY
set -uo pipefail
L=/mnt/vmstore/caraxes/locks H=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
while p=$(cat "$L/priority" 2>/dev/null) && [[ -n $p && $p != desktop-b-dmem ]]; do sleep 30; done
exec flock -w 1800 "$L/guest-desktop-b.lock" "$L/vmlock.sh" desktop-b-dmem 1800 "$H/census-linux.sh" "$@"

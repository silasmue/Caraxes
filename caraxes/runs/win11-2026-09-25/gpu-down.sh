#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Stop the Windows guest's synthetic GPU of gpu-up.sh (SIGTERM, the device frees and exits).
set -uo pipefail
W=/mnt/vmstore/caraxes/win11
if [[ -s $W/gpu.pid ]] && kill -0 "$(cat "$W/gpu.pid")" 2>/dev/null; then
    kill "$(cat "$W/gpu.pid")"; for _ in $(seq 30); do kill -0 "$(cat "$W/gpu.pid")" 2>/dev/null || break; sleep 0.5; done
    echo "device stopped; $(tail -1 "$W/gpu.log" | cut -c1-120)"
fi
rm -f "$W/gpu.pid" "$W/gpu.sock"

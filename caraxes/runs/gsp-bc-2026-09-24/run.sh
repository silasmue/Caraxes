#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The nvml and cuda gates of the GSP plan B and C package on vm5 (env-vm5.sh), one phase per
# device binary: run.sh <phase> <binary>. Refuses to start while vm5 or the v1 bench VM
# runs. Per phase: setup, up, the nvml gate, the scored cuda tests without torch and the
# unscored suites (short runs: other VMs share the card), the device log's issues, down,
# host Xid lines of the window.
set -uo pipefail
PHASE=${1:?phase}; BIN=${2:?device binary}
R=/home/silas/git/Caraxes
O=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
C=$O/$PHASE; mkdir -p "$C"
[[ ${GSPBC_EMU:-0} == 1 ]] && source "$O/env-vm5-emu.sh" || source "$O/env-vm5.sh"
export CARAXES_SERVER=$BIN
if pgrep -f /mnt/vmstore/caraxes/vm5/ >/dev/null || pgrep -f bench-v1 >/dev/null; then
    echo "vm5 or the v1 bench VM runs: not starting"; exit 2
fi
cd "$R" || exit 1
W=/mnt/vmstore/caraxes/$VM; T0=$(date '+%F %T')
{ head -1 /proc/driver/nvidia/version; sha256sum "$CARAXES_SERVER"; env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; } > "$C/setup.txt"
caraxes/tools/bench-hostwatch.sh start "$C"
caraxes/tools/bench-run.sh v2-restart > "$C/up.txt" 2>&1 || tail -5 "$C/up.txt"
grep -E 'host ABI|reservation|unanswered|ERROR|REFUSED|refused' "$W/gpu.log" | head -40 | cut -c1-400 > "$C/device-log-first.txt"
tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
tools/caraxes-gate.sh nvml > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"; tail -2 "$C/gate-nvml.txt" | cut -c1-300
CARAXES_GATE_RUNS=$C tools/caraxes-gate.sh cuda ${CUDA_TESTS:-cuda-init cuda-context cuda-core cuda-jit cuda-launch cuda-managed cuda-hostreg robustness} > "$C/gate-cuda.txt" 2>&1
echo "cuda gate rc=$?"; tail -2 "$C/gate-cuda.txt" | cut -c1-300
caraxes/tools/bench-hostwatch.sh stop "$C"
# Every control the device answered in the window: id, status, label, count (gpu-trace.log
# lines "E boot: ctrl <id>: <note> -> status <st> [<label>]"); the issues are the non-zero ones.
sed -nE 's/.* E boot: ctrl (0x[0-9a-f]{8}): .* -> status (0x[0-9a-f]+) \[(.*)\]$/\1 \2 [\3]/p' "$W/gpu-trace.log" \
    | sort | uniq -c | sort -k2 > "$C/device-controls.txt"
{ grep -v ' 0x0 \[' "$C/device-controls.txt"; grep -E 'ERROR|REFUSED' "$W/gpu.log" | sed -E 's/^ *[0-9]+ //' | sort | uniq -c | sort -rn | head -20 | cut -c1-300; } > "$C/device-log-issues.txt"
tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
echo "$PHASE done $(date -Iseconds); host Xid lines $(grep -c Xid "$C/host-xid.txt")"

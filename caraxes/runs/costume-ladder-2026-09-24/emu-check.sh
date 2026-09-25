#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Pure-emulation boot check of one ladder rung (docs/caraxes/costume-ladder.md §2): the
# tapless VM vm6 (no host RM, no reservation, no GPU), whose caraxes-check.service
# (guest-check.sh) loads nvidia, runs nvidia-smi and powers off. Usage:
#   emu-check.sh <record dir> [launcher environment as NAME=value ...]
# Keeps serial.log, gpu.log and the device trace's unanswered/refused lines under <dir>.
set -euo pipefail
R=$(cd "$(dirname "$0")/../../.." && pwd -P)
out=$1; shift
mkdir -p "$out"
export VM=vm6 CARAXES_TAP=none CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=${CARAXES_FB_MIB:-1024}
export CARAXES_SERVER=${CARAXES_SERVER:-$R/target/ladder/release/vfio-user-nvgpu}
for kv in "$@"; do export "${kv?}"; done
W=/mnt/vmstore/caraxes/vm6
rm -f "$W/serial.log"
"$R/tools/caraxes-vm.sh" up | tee "$out/up.txt"
for _ in $(seq 240); do
    kill -0 "$(cat "$W/ch.pid")" 2>/dev/null || break
    grep -q "CARAXES-CHECK end" "$W/serial.log" 2>/dev/null && sleep 5 && break
    sleep 2
done
"$R/tools/caraxes-vm.sh" down
cp "$W/serial.log" "$out/serial.log"
cp "$W/gpu.log" "$out/gpu.log"
grep -E "unanswered|refused|NOT_SUPPORTED|assert|panic|ERROR" "$W/gpu-trace.log" | sort | uniq -c | sort -rn | head -n 200 > "$out/trace-notable.txt" || true
printf '%s\n' "$@" > "$out/env.txt"
grep -E "RmInitAdapter|NVRM|modprobe rc|nvidia-smi rc|query rc|compute_cap|^[^,]+, [0-9.]+," "$out/serial.log" | head -n 40

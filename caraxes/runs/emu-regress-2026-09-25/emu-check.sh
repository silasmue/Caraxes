#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# One pure-emulation boot of the regression runs (README.md next to this script): the tapless
# VM vm7 (or vm7-615 with CARAXES_DRIVER=615), no host RM, no reservation, no GPU, the boot
# model on. The guest's caraxes-check.service (guest-check.sh) loads nvidia, runs nvidia-smi
# (and on a GB206 the UVM registration) and powers off. Usage:
#   emu-check.sh <record dir> [launcher environment as NAME=value ...]
# CARAXES_SERVER defaults to this checkout's target/release binary (target/v615 for 615);
# CARAXES_RIG=<tree> runs the launcher of another tree (the bisect runs a frozen copy of the
# HEAD tree, target/rig, because older commits' launchers have no CARAXES_TAP=none).
# Keeps serial.log, gpu.log and the device trace's notable lines under <dir>; prints the
# oracle lines (RmInitAdapter, the memmgrMemSet timeout, numGpcs, nvidia-smi).
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
R=$(cd "$HERE/../../.." && pwd -P)
L=${CARAXES_RIG:-$R}
out=$1; shift
[[ -e $out ]] && { echo "ERROR: $out exists; never overwrite a run" >&2; exit 2; }
mkdir -p "$out"
unset CARAXES_HOST_RM CARAXES_RESERVE_VRAM CARAXES_HOST_EXEC CARAXES_DMA
export VM=vm7 CARAXES_TAP=none CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=${CARAXES_FB_MIB:-1024}
for kv in "$@"; do export "${kv?}"; done
if [[ ${CARAXES_DRIVER:-610} == 615 ]]; then
    VM=vm7-615
    : "${CARAXES_SERVER:=$R/target/v615/release/vfio-user-nvgpu}"
else
    : "${CARAXES_SERVER:=$R/target/release/vfio-user-nvgpu}"
fi
export VM CARAXES_SERVER
W=/mnt/vmstore/caraxes/$VM
for _ in $(seq 30); do pgrep -f "(vfio-user-nvgpu|cloud-hypervisor)[^ ]* .*$W/" >/dev/null || break; sleep 1; done
if pgrep -f "(vfio-user-nvgpu|cloud-hypervisor)[^ ]* .*$W/" >/dev/null; then echo "ERROR: a $VM process runs" >&2; exit 2; fi
{ sha256sum "$CARAXES_SERVER"; git -C "$R" log --oneline -1; env | grep -E '^CARAXES_|^VM=' | sort; } > "$out/setup.txt"
rm -f "$W/serial.log"
"$L/tools/caraxes-vm.sh" up | tee "$out/up.txt"
for _ in $(seq 300); do
    kill -0 "$(cat "$W/ch.pid")" 2>/dev/null || break
    grep -q "CARAXES-CHECK end" "$W/serial.log" 2>/dev/null && sleep 5 && break
    sleep 2
done
"$L/tools/caraxes-vm.sh" down > "$out/down.txt" 2>&1 || true
cp "$W/serial.log" "$out/serial.log"
cp "$W/gpu.log" "$out/gpu.log"
grep -aE "unanswered|refused|NOT_SUPPORTED|assert|panic|ERROR|VAS root" "$W/gpu-trace.log" \
    | sed -E 's/^ *[0-9]+ //' | sort | uniq -c | sort -rn | head -n 200 | cut -c1-400 > "$out/trace-notable.txt" || true
# The Blackwell N gate's device-side evidence (docs/caraxes/blackwell-gate.md section 1, N2 and N4).
grep -aE "fsp: COT version|acr: fb|target mask released|doorbell 1: libos args PA|queues linked|GET_GSP_STATIC_INFO|alloc 0x(ca6f|c96f) " "$W/gpu-trace.log" \
    | cut -c1-400 > "$out/gate-evidence.txt" || true
grep -aE "RmInitAdapter|memmgrMemSet|numGpcs|modprobe rc|nvidia-smi rc|query rc|smi-q rc|uvm-register rc|Product Name|FB Memory|Total  |VBIOS Version|^[^,]+, [0-9.]+,|watchdog" \
    "$out/serial.log" | head -n 40

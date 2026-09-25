#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# One pure-emulation boot of the limits runs (README.md next to this script): uploads
# scenarios/<scenario>.sh into vm8 as the boot's scenario, boots vm8 without a network device
# (CARAXES_TAP=none), no host RM, no reservation, no GPU, the boot model on, and keeps the
# serial log, the device log and the device trace's family and refusal lines. Usage:
#   run.sh <record dir> <scenario> [launcher environment as NAME=value ...]
# The device keeps its persistent settings (the ECC and MIG modes, the InfoROM analogue) in
# $W/gpu-persist.toml across boots; FRESH=1 removes it first (a card as shipped).
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
R=$(cd "$HERE/../../.." && pwd -P)
out=$1 scen=$2
shift 2
[[ -e $out ]] && { echo "ERROR: $out exists; never overwrite a run" >&2; exit 2; }
[[ -f $HERE/scenarios/$scen.sh ]] || { echo "ERROR: no scenario $scen" >&2; exit 2; }
unset CARAXES_HOST_RM CARAXES_RESERVE_VRAM CARAXES_HOST_EXEC CARAXES_DMA
export VM=vm8 CARAXES_TAP=none CARAXES_BOOT_MODEL=1 CARAXES_PERSIST=1 CARAXES_FB_MIB=${CARAXES_FB_MIB:-4096}
for kv in "$@"; do export "${kv?}"; done
: "${CARAXES_SERVER:=$R/target/limits/release/vfio-user-nvgpu}"
export CARAXES_SERVER
W=/mnt/vmstore/caraxes/$VM
if pgrep -f "(vfio-user-nvgpu|cloud-hypervisor)[^ ]* .*$W/" >/dev/null; then echo "ERROR: a $VM process runs" >&2; exit 2; fi
mkdir -p "$out"
[[ ${FRESH:-0} == 1 ]] && rm -f "$W/gpu-persist.toml"
{
    sha256sum "$CARAXES_SERVER"
    git -C "$R" log --oneline -1
    env | grep -E '^CARAXES_|^VM=|^FRESH=' | sort
    echo "scenario $scen"
    echo "persist before:"
    cat "$W/gpu-persist.toml" 2>/dev/null || echo "(none)"
} > "$out/setup.txt"
# The libguestfs appliance failed to start once on this shared host (2026-09-25 09:05):
# three tries, then stop.
for try in 1 2 3; do
    guestfish -a "$W/rootfs.qcow2" -i upload "$HERE/scenarios/$scen.sh" /usr/local/sbin/caraxes-scenario.sh && break
    [[ $try == 3 ]] && { echo "ERROR: guestfish upload failed" >&2; exit 2; }
    sleep 5
done
rm -f "$W/serial.log"
"$R/tools/caraxes-vm.sh" up | tee "$out/up.txt"
for _ in $(seq 330); do
    kill -0 "$(cat "$W/ch.pid")" 2>/dev/null || break
    grep -q "CARAXES-CHECK end" "$W/serial.log" 2>/dev/null && sleep 5 && break
    sleep 2
done
"$R/tools/caraxes-vm.sh" down > "$out/down.txt" 2>&1 || true
tr -d '\r' < "$W/serial.log" > "$out/serial.txt"
cp "$W/gpu.log" "$out/device-log.txt"
{
    echo "persist after:"
    cat "$W/gpu-persist.toml" 2>/dev/null || echo "(none)"
} >> "$out/setup.txt"
grep -aE "unanswered|refused|NOT_SUPPORTED|assert|panic|ERROR" "$W/gpu-trace.log" \
    | sed -E 's/^ *[0-9]+ //' | sort | uniq -c | sort -rn | head -n 200 | cut -c1-400 > "$out/trace-notable.txt" || true
grep -aE "family|ecc|ECC|mig|MIG|SMC|smc|vgpu|VGPU|0xc637|0xc638|PARTITION|partition|instance|pce|remap|inforom" "$W/gpu-trace.log" \
    | cut -c1-500 > "$out/trace-family.txt" || true
grep -aE "CARAXES-STEP|RmInitAdapter|modprobe rc|Xid" "$out/serial.txt" | tr -d '\r' | head -n 80

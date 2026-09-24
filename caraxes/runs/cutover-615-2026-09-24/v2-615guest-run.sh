#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# cutover-615.md step (d), runs 5 onwards of the 615 guest on the 615 host: v2-615guest.sh
# (run #4) with three changes. The results go to v2-615guest-<run>/ next to this script;
# the device binary is the v615-feature build of the checkout this script lives in
# (CARAXES_SERVER, default <checkout>/target/v615/release/vfio-user-nvgpu); the tools run
# from the main checkout R as in run #4. The FB profile is 1 GiB at reserve slot 3
# ([top-4 GiB, top-3 GiB)) instead of 2 GiB at slot 0, because a 3 GiB VM takes slot 0 and
# vm1 holds 1 GiB at slot 4. Usage: v2-615guest-run.sh r5
set -uo pipefail
RUN=${1:?usage: v2-615guest-run.sh <run name, e.g. r5>}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
SRC=$(cd "$HERE/../../.." && pwd -P)
R=/home/silas/git/Caraxes; C=$HERE/v2-615guest-$RUN
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
mkdir -p "$C"; cd "$R" || exit 1
export VM=vm2-615 CARAXES_IP=192.168.102.22 CARAXES_TAP=caraxes2 CARAXES_MAC=02:00:00:00:00:22
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
export CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=3
export CARAXES_DRIVER=615 CARAXES_SERVER=${CARAXES_SERVER:-$SRC/target/v615/release/vfio-user-nvgpu}
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH
W=/mnt/vmstore/caraxes/vm2-615; T0=$(date '+%F %T')
if pgrep -f /mnt/vmstore/caraxes/vm2 >/dev/null; then
    echo "ERROR: a vm2 process runs; refusing to start" >&2; pgrep -af /mnt/vmstore/caraxes/vm2 >&2; exit 2
fi
{ cat /proc/driver/nvidia/version | head -1; sha256sum "$CARAXES_SERVER"; git -C "$SRC" log --oneline -1; env | grep -E '^CARAXES_|^LEA_CH' | sort; } > "$C/setup.txt"
caraxes/tools/bench-hostwatch.sh start "$C"
tools/caraxes-vm.sh up > "$C/up.txt" 2>&1 || { tail -5 "$C/up.txt"; }
tools/caraxes-vm.sh wait >> "$C/up.txt" 2>&1
tools/caraxes-vm.sh probe cut615 >> "$C/up.txt" 2>&1; tail -3 "$C/up.txt" | cut -c1-200
grep -E 'host ABI|guest ABI|msgq|GMCAPI|transport|GSS legacy|ERROR' "$W/gpu.log" | cut -c1-300 | head -8 > "$C/device-log-first.txt"; head -4 "$C/device-log-first.txt" | cut -c1-260
tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
tools/caraxes-gate.sh nvml > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"; tail -3 "$C/gate-nvml.txt" | cut -c1-200
tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -3 "$C/gate-cuda.txt" | cut -c1-200
caraxes/tools/bench-run.sh stage v2 > "$C/stage.txt" 2>&1
caraxes/tools/bench-run.sh series --targets v2 --reps 2 --metrics "init short-auto" --out "$C" 2>&1 | tail -2
caraxes/tools/bench-hostwatch.sh stop "$C"
grep -E 'unanswered|ERROR|refused|NOT_SUPPORTED' "$W/gpu.log" | sort | uniq -c | sort -rn | head -12 | cut -c1-250 > "$C/device-log-issues.txt"
# The per-control lines are in the trace, not in gpu.log.
grep -a 'lce shim info' "$W/gpu-trace.log" | sed -E 's/^.*(ctrl 0x20802a0c)/\1/' | sort | uniq -c | cut -c1-300 > "$C/device-log-lce-shim.txt"
grep -aE 'unanswered|NOT_SUPPORTED|status 0x56' "$W/gpu-trace.log" | sed -E 's/^.*(ctrl 0x[0-9a-f]+)/\1/' | sort | uniq -c | sort -rn | head -15 | cut -c1-250 > "$C/device-trace-issues.txt"
tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
echo "615 guest on 615 host $RUN done $(date -Iseconds); Xid $(grep -c Xid "$C/host-xid.txt")"

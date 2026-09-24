#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# cutover-615.md step (c): the 610 guest on the 615 host. vm2 at slot 0 with the 2 GiB
# profile, the device built from main ae9b460 after the host upgrade (frozen as
# /mnt/vmstore/caraxes/vfio-user-nvgpu-cut615-ae9b460), the closing configuration (P21 area,
# token pool, hybrid reactor, Cloud Hypervisor 0001-0006). Log line first, then the nvml and
# cuda gates, then bench-cu init and the short kernel.
set -uo pipefail
R=/home/silas/git/Caraxes; C=$R/caraxes/runs/cutover-615-2026-09-24/v2-610guest; cd "$R"
source "$R/caraxes/runs/mapping-2026-09-24/env.sh"
export CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-cut615-ae9b460 LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_BAR1_PASSTHROUGH=1 CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE
W=/mnt/vmstore/caraxes/vm2; T0=$(date '+%F %T')
{ cat /proc/driver/nvidia/version | head -1; sha256sum "$CARAXES_SERVER"; env | grep -E '^CARAXES_|^LEA_CH' | sort; } > "$C/setup.txt"
caraxes/tools/bench-hostwatch.sh start "$C"
caraxes/tools/bench-run.sh v2-restart > "$C/up.txt" 2>&1 || { tail -5 "$C/up.txt"; }
grep -E 'host ABI|GSS legacy|kgr floorsweeping|kgr global SM order|host TSG|unanswered|ERROR' "$W/gpu.log" | cut -c1-400 > "$C/device-log-first.txt"
head -3 "$C/device-log-first.txt" | cut -c1-300
tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
tools/caraxes-gate.sh nvml > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"; tail -4 "$C/gate-nvml.txt" | cut -c1-200
tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -4 "$C/gate-cuda.txt" | cut -c1-200
caraxes/tools/bench-run.sh stage v2 > "$C/stage.txt" 2>&1
caraxes/tools/bench-run.sh series --targets v2 --reps 2 --metrics "init short-auto rl" --out "$C" 2>&1 | tail -3
caraxes/tools/bench-hostwatch.sh stop "$C"
grep -E 'unanswered|ERROR|refused' "$W/gpu.log" | sort | uniq -c | sort -rn | head -12 | cut -c1-250 > "$C/device-log-issues.txt"
tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
echo "610 guest on 615 host done $(date -Iseconds); Xid $(grep -c Xid "$C/host-xid.txt")"

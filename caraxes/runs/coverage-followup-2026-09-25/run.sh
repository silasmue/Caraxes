#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# The coverage follow-up of 2026-09-25 (docs/caraxes/coverage-map.md, status lines): the
# sweep getters answered (part 1) and the forward cache (part 2, src/fwdcache.rs), measured
# with the device's profile on vm2 in the coverage map's environment
# (../coverage-2026-09-25/run.sh: the night environment, 610 guest, 1 GiB at reservation
# slot 3, plus BAR1 pool 32 MiB, CARAXES_BAR2_MEMFD=1, CARAXES_MSI_REARM_IOEVENTFD=1,
# CARAXES_ALLOC_LAZY=1), with this branch's plain release build (target/cov2/release).
#   run.sh off [label]   CARAXES_FORWARD_CACHE=0: nvidia-smi -q x10, a --query-gpu loop x20,
#                        dmon/pmon/query loop, the control sweep
#   run.sh on [label]    the cache on: the same, then the nvml and cuda gates
# One SIGUSR2 profile dump after each workload (marks.tsv). SERVER=path overrides the binary.
set -uo pipefail
MODE=${1:?usage: run.sh off|on [label]}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
C=$HERE/$MODE${2:+-$2}
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
cd "$ROOT" || exit 1
# shellcheck source=../night-2026-09-24/env.sh
source "$ROOT/caraxes/runs/night-2026-09-24/env.sh"
night_guest 610
export CARAXES_BAR1_POOL_MIB=32 CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 CARAXES_ALLOC_LAZY=1
unset CARAXES_PROFILE CARAXES_PROFILE_OUT LLVM_PROFILE_FILE CARAXES_TRACE_ACCESSES CARAXES_TRACE_FLUSH CARAXES_FORWARD_CACHE
case $MODE in
    off) export CARAXES_FORWARD_CACHE=0 ;;
    on) ;;
    *) echo "ERROR: unknown mode $MODE" >&2; exit 2 ;;
esac
SERVER=${SERVER:-$ROOT/target/cov2/release/vfio-user-nvgpu}
export CARAXES_SERVER=$SERVER CARAXES_PROFILE=1 CARAXES_PROFILE_OUT=$C/profile/dev
night_vm2_free || exit 2
mkdir -p "$C/work" "$C/profile"
W=$NIGHT_W; T0=$(date '+%F %T')
night_setup "$C"
caraxes/tools/bench-hostwatch.sh start "$C"

up_ok=0
for attempt in 1 2 3 4 5 6; do
    tools/caraxes-vm.sh up >> "$C/up.txt" 2>&1 && tools/caraxes-vm.sh wait >> "$C/up.txt" 2>&1 && { up_ok=1; break; }
    tail -3 "$C/up.txt"
    tools/caraxes-vm.sh down >> "$C/up.txt" 2>&1
    if grep -qE 'NO_MEMORY|did not give|headroom|Could not mmap sparse area' "$C/up.txt"; then
        echo "reservation or host BAR1 refused (attempt $attempt), waiting 5 min"; sleep 300
    else
        break
    fi
done
finish() {
    caraxes/tools/bench-hostwatch.sh stop "$C"
    tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
    sleep 2
    grep -E 'forward cache|profile|ERROR|WARN' "$W/gpu.log" | tail -60 | cut -c1-600 > "$C/device-summary.txt"
    journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
    local pid; pid=$(cat "$C/device-pid.txt" 2>/dev/null)
    echo "host Xid lines $(grep -c Xid "$C/host-xid.txt"), of the device pid ${pid:-?}: $(grep -c "pid=${pid:-none}\b" "$C/host-xid.txt")" | tee "$C/xid-summary.txt"
    echo "$MODE done $(date -Iseconds); vm2 left: $(pgrep -f /mnt/vmstore/caraxes/vm2 | wc -l)"
}
if [[ $up_ok != 1 ]]; then echo "ERROR: vm2 did not come up"; finish; exit 1; fi
cat "$W/gpu.pid" > "$C/device-pid.txt"
tools/caraxes-vm.sh probe cov >> "$C/up.txt" 2>&1; tail -1 "$C/up.txt" | cut -c1-200
caraxes/tools/bench-run.sh v2-nodes >> "$C/up.txt" 2>&1
grep -E 'forward cache|profile|host ABI|host RM open' "$W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
caraxes/tools/bench-run.sh stage v2 >> "$C/up.txt" 2>&1

DEVPID=$(cat "$C/device-pid.txt"); P=$CARAXES_PROFILE_OUT; n=0
mark() {
    n=$((n + 1)); kill -USR2 "$DEVPID"
    for _ in $(seq 100); do [[ -f $P.$n.tsv ]] && break; sleep 0.1; done
    printf '%s\t%s\t%s\t%s\n' "$n" "$1" "$(date +%s)" "$([[ -f $P.$n.tsv ]] && echo dumped || echo MISSING)" >> "$C/marks.tsv"
}
wl() {  # LABEL CMD...: run one workload, then dump
    local label=$1; shift
    echo "== $label $(date +%T)"
    timeout -k 10 900 "$@" > "$C/work/$label.txt" 2>&1; echo "# rc=$?" >> "$C/work/$label.txt"
    grep -E '^COV|GATE:|CUDA-GATE|NVML-GATE|^# rc' "$C/work/$label.txt" | tail -2 | cut -c1-300
    mark "$label"
}
GW=(tools/caraxes-vm.sh ssh)
mark boot
tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
tools/caraxes-vm.sh ssh 'mkdir -p ~/cov'
for f in caraxes/runs/coverage-2026-09-25/covguest.sh caraxes/runs/coverage-followup-2026-09-25/fuguest.sh \
         caraxes/tools/ctlsweep.c caraxes/runs/coverage-2026-09-25/ctlsweep-table.h; do
    tools/caraxes-vm.sh ssh "cat > ~/cov/$(basename "$f")" < "$f"
done
tools/caraxes-vm.sh ssh 'bash ~/cov/covguest.sh build-sweep' > "$C/build.txt" 2>&1; tail -1 "$C/build.txt"
mark setup
for w in smi-q-timed smi-query-timed smi-dmon ctlsweep; do
    wl "$w" "${GW[@]}" "bash ~/cov/fuguest.sh $w"
done
tools/caraxes-vm.sh ssh 'cat ~/cov/ctlsweep.txt' > "$C/work/ctlsweep-table-out.txt" 2>&1
if [[ $MODE == on ]]; then
    wl gate-nvml tools/caraxes-gate.sh nvml
    wl gate-cuda tools/caraxes-gate.sh cuda
fi
finish

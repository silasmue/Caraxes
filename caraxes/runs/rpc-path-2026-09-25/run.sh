#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The RPC path of 2026-09-25 (docs/caraxes/gsp-transport-classes.md §6, §7): the guest-cached
# GSS-legacy answers (CACHEABLE reply flag, nvml.rs `legacy_reply_flags`) and the queue-pointer
# ioeventfd (CARAXES_RPC_IOEVENTFD=1, device.rs `rpc_ioeventfd`), measured on vm2 (610 guest,
# 1 GiB at reservation slot 3) and vm2-615 (615 guest), each in the night environment
# (caraxes/runs/night-2026-09-24/env.sh) plus the recommended configuration: interrupt page,
# token pool 64, hybrid reactor, BAR1 pool 32 MiB, BAR2 memfd, MSI re-arm ioeventfd, lazy
# allocation. The device profile is on (CARAXES_PROFILE=1), one dump per workload.
#   run.sh 610|615 LABEL      with SERVER=<frozen binary>; RPCFD=1 turns the ioeventfd on,
#                             CACHE=0 turns the CACHEABLE flag off (opt-in since the dev
#                             merge of 2026-09-28: CACHE=1, the default here, exports =1)
# Workloads: sync-tests, 10 x nvidia-smi -q (timed each), the CUDA gate (load), 10 x
# nvidia-smi -q again, nvidia-smi dmon 20 s (timed), the NVML gate. Results go to LABEL/.
set -uo pipefail
G=${1:?usage: run.sh 610|615 LABEL}; L=${2:?usage: run.sh 610|615 LABEL}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
C=$HERE/$L
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
cd "$ROOT" || exit 1
# shellcheck source=../night-2026-09-24/env.sh
source "$ROOT/caraxes/runs/night-2026-09-24/env.sh"
night_guest "$G" || exit 2
export CARAXES_BAR1_POOL_MIB=32 CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 CARAXES_ALLOC_LAZY=1
export CARAXES_SERVER=${SERVER:?SERVER=<frozen device binary>}
export CARAXES_PROFILE=1 CARAXES_PROFILE_OUT=$C/profile/dev
unset CARAXES_TRACE_ACCESSES CARAXES_TRACE_FLUSH CARAXES_RPC_IOEVENTFD CARAXES_LEGACY_CACHEABLE
[[ ${RPCFD:-0} == 1 ]] && export CARAXES_RPC_IOEVENTFD=1
[[ ${CACHE:-1} == 1 ]] && export CARAXES_LEGACY_CACHEABLE=1
night_vm2_free || exit 2
mkdir -p "$C/work" "$C/profile"
export CARAXES_GATE_RUNS=$C
W=$NIGHT_W; T0=$(date '+%F %T')
night_setup "$C"
caraxes/tools/bench-hostwatch.sh start "$C"
up_ok=0
for attempt in 1 2 3 4 5 6 7 8 9 10 11 12; do
    tools/caraxes-vm.sh up >> "$C/up.txt" 2>&1 && tools/caraxes-vm.sh wait 180 >> "$C/up.txt" 2>&1 && { up_ok=1; break; }
    tail -3 "$C/up.txt"
    tools/caraxes-vm.sh down >> "$C/up.txt" 2>&1
    if grep -qE 'NO_MEMORY|did not give|headroom|refused: reserving|refused: pinning' "$C/up.txt" "$W/gpu.log" 2>/dev/null; then
        echo "reservation refused (attempt $attempt), waiting 5 min" | tee -a "$C/retries.txt"; sleep 300
    else
        break
    fi
done
finish() {
    caraxes/tools/bench-hostwatch.sh stop "$C"
    tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
    sleep 2
    grep -E 'profile|intr page|bar2 memfd|bar1 pool|doorbell|rpc ioeventfd|msi re-arm|mirror: |shadow: |ERROR|WARN' "$W/gpu.log" | tail -80 | cut -c1-600 > "$C/device-summary.txt"
    grep -aE 'GSS legacy' "$W/gpu-trace.log" | grep -oE 'ctrl 0x[0-9a-f]+: .*' | cut -c1-400 > "$C/legacy-forwards.txt"
    grep -acE 'rmctrlFlags 0x408' "$W/gpu-trace.log" > "$C/cacheable-replies.txt"
    xz -T0 -c "$W/gpu-trace.log" > "$C/device-trace.txt.xz"
    xz -T0 "$C"/profile/*.tsv 2>/dev/null
    journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
    local pid; pid=$(cat "$C/device-pid.txt" 2>/dev/null)
    echo "host Xid lines $(grep -c Xid "$C/host-xid.txt"), of the device pid ${pid:-?}: $(grep -c "pid=${pid:-none}\b" "$C/host-xid.txt")" | tee "$C/xid-summary.txt"
    echo "$L done $(date -Iseconds); vm2 left: $(pgrep -f /mnt/vmstore/caraxes/vm2 | wc -l)"
}
if [[ $up_ok != 1 ]]; then echo "ERROR: $VM did not come up"; finish; exit 1; fi
cat "$W/gpu.pid" > "$C/device-pid.txt"
tools/caraxes-vm.sh probe rpcpath >> "$C/up.txt" 2>&1; tail -1 "$C/up.txt" | cut -c1-200
grep -E 'profile|intr page|bar2 memfd|bar1 pool|doorbell tokens|doorbell poll|msi re-arm|rpc ioeventfd|lazy|host ABI|guest ABI' "$W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
DEVPID=$(cat "$C/device-pid.txt"); P=$CARAXES_PROFILE_OUT; n=0
mark() {
    n=$((n + 1)); kill -USR2 "$DEVPID"
    for _ in $(seq 100); do [[ -f $P.$n.tsv ]] && break; sleep 0.1; done
    printf '%s\t%s\t%s\t%s\n' "$n" "$1" "$(date +%s)" "$([[ -f $P.$n.tsv ]] && echo dumped || echo MISSING)" >> "$C/marks.tsv"
}
SMI='sudo LD_LIBRARY_PATH=/opt/nvrm/lib /opt/nvrm/bin/nvidia-smi'
smiq() {  # 10 x nvidia-smi -q, wall ms each
    tools/caraxes-vm.sh ssh "for i in \$(seq 10); do s=\$(date +%s%N); $SMI -q > /tmp/smi-q.txt || echo FAIL; e=\$(date +%s%N); echo smi-q-ms \$(( (e - s) / 1000000 )); done; grep -cE 'N/A' /tmp/smi-q.txt | sed 's/^/n-a-fields /'; cat /tmp/smi-q.txt" > "$C/work/$1.txt" 2>&1
    grep -E '^smi-q-ms' "$C/work/$1.txt" | awk '{s+=$2; a[NR]=$2} END {asort(a); printf "%s: 10 x nvidia-smi -q, mean %.0f ms, median %d ms, first %s\n", "'"$1"'", s/NR, a[int((NR+1)/2)], "'"$(grep -m1 -E '^smi-q-ms' "$C/work/$1.txt" | cut -d' ' -f2)"'"}' | tee -a "$C/summary.txt"
    mark "$1"
}
mark boot
tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
mark sync-tests
smiq smi-q-1
tools/caraxes-gate.sh cuda > "$C/work/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?" | tee -a "$C/summary.txt"; tail -1 "$C/work/gate-cuda.txt" | cut -c1-200
mark gate-cuda
smiq smi-q-2
tools/caraxes-vm.sh ssh "s=\$(date +%s%N); $SMI dmon -c 20 -s pucvmet | tail -4; e=\$(date +%s%N); echo dmon-ms \$(( (e - s) / 1000000 ))" > "$C/work/dmon.txt" 2>&1
grep dmon-ms "$C/work/dmon.txt" | sed 's/^/dmon 20 samples: /' | tee -a "$C/summary.txt"
mark dmon
tools/caraxes-gate.sh nvml > "$C/work/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?" | tee -a "$C/summary.txt"; tail -1 "$C/work/gate-nvml.txt" | cut -c1-200
mark gate-nvml
tools/caraxes-vm.sh ssh 'sudo dmesg | grep -iE "nvrm|xid" | tail -40' > "$C/guest-dmesg.txt" 2>&1
finish

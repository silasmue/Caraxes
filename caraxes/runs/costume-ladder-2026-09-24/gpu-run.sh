#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The GPU check of one costume-ladder rung (docs/caraxes/costume-ladder.md §2): vm1 with the
# environment of caraxes/runs/doorbell-2026-09-24/env-vm1.sh (1 GiB at reservation slot 4,
# host RM, host execution, DMA register, Cloud Hypervisor 0001-0006, token pool 64, hybrid
# reactor, no BAR1 area), the device of this worktree (target/ladder). Usage:
#   gpu-run.sh <record dir> full|stepped|small|ident [NAME=value ...]
# full: probe, the nvml gate, the cuda gate, the identity (lspci, nvidia-smi, torch).
# ident: probe and the identity only.
# small (the SM-version rungs, small blast radius): probe, then cuda-init, cuda-jit and one
# launch probe (cuda-launch) as single gate tests, the host Xid log read after each, the
# identity. Host watch with KILL=<device pidfile> and SLOW=${HW_SLOW:-1}: a host nvidia-smi
# slower than that stops the device (1 s for the SM-version rungs as the operator set it;
# the costume rungs run with HW_SLOW=5, since single samples of 1.7 and 2.5 s occurred on
# this shared host with no guest GPU work in flight, r1-tu104/gpu-ident/hostwatch.txt). Refuses to start while vm1 runs (its own ch.sock or gpu.sock).
set -uo pipefail
TREE=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd -P)
out=$1 mode=$2; shift 2
source "$TREE/caraxes/runs/doorbell-2026-09-24/env-vm1.sh"
R=$TREE; mkdir -p "$out"; C=$(cd "$out" && pwd -P); cd "$R" || exit 1
export CARAXES_SERVER=$R/target/ladder/release/vfio-user-nvgpu LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_DRIVER CARAXES_GUEST_ABI
for kv in "$@"; do export "${kv?}"; done
W=/mnt/vmstore/caraxes/$VM; T0=$(date '+%F %T')
# The gate run directories that exist before this run stay where they are.
BEFORE=$(ls -d "$R"/caraxes/runs/gate-cuda-* 2>/dev/null)
VMSH=$R/tools/caraxes-vm.sh
L='sudo LD_LIBRARY_PATH=/opt/nvrm/lib'; SMI="$L /opt/nvrm/bin/nvidia-smi"
if pgrep -f "$W/ch.sock|$W/gpu.sock" >/dev/null; then echo "vm1 is in use"; exit 3; fi
{ head -1 /proc/driver/nvidia/version; git -C "$R" log --oneline -1; sha256sum "$CARAXES_SERVER"; env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; echo "start $T0"; } > "$C/setup.txt"
# Host Xid lines since T0 that do not name the device's pid: the stop rule of the ladder.
DEVPID=none
foreign_xid() {
    local pid=$DEVPID
    journalctl -k --since "$T0" --no-pager -g Xid 2>/dev/null > "$C/host-xid.txt"
    grep Xid "$C/host-xid.txt" | grep -v "pid=$pid," | grep -c Xid
}
up_ok=0
for attempt in 1 2 3; do
    "$VMSH" up > "$C/up.txt" 2>&1 && { up_ok=1; break; }
    tail -5 "$C/up.txt"
    grep -qiE 'headroom|free|reserv' "$C/up.txt" || break
    echo "reservation refused (attempt $attempt), retry in 5 minutes"; "$VMSH" down >/dev/null 2>&1; sleep 300
done
(( up_ok )) || { "$VMSH" down >/dev/null 2>&1; exit 1; }
DEVPID=$(cat "$W/gpu.pid")
INTERVAL=5 SLOW=${HW_SLOW:-1} KILL="$W/gpu.pid" caraxes/tools/bench-hostwatch.sh start "$C"
"$VMSH" wait >> "$C/up.txt" 2>&1 || { tail -5 "$C/up.txt"; caraxes/tools/bench-hostwatch.sh stop "$C"; "$VMSH" down; exit 1; }
"$VMSH" probe ladder > "$C/probe.txt" 2>&1; echo "probe rc=$?"; tail -8 "$C/probe.txt"
grep -E 'guest name|BOOT_0|host ABI|SM version|unanswered|ERROR' "$W/gpu.log" | cut -c1-400 > "$C/device-log-first.txt"
"$VMSH" sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
stop=0
if [[ $mode == ident ]]; then
    :
elif [[ $mode == stepped ]]; then
    # stepped (the chip costumes past Turing): the nvml gate, then cuda-init, cuda-jit and
    # cuda-launch one at a time with the Xid check after each; the whole cuda gate only
    # when those three pass.
    tools/caraxes-gate.sh nvml > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"; tail -2 "$C/gate-nvml.txt" | cut -c1-300
    ok3=1
    for t in cuda-init cuda-jit cuda-launch; do
        tools/caraxes-gate.sh cuda "$t" > "$C/gate-$t.txt" 2>&1; rc=$?; echo "$t rc=$rc"; tail -1 "$C/gate-$t.txt" | cut -c1-300
        (( rc == 0 )) || ok3=0
        n=$(foreign_xid); (( n > 0 )) && { echo "STOP: $n host Xid lines outside the device pid"; stop=1; break; }
        grep -q "Xid" "$C/host-xid.txt" && echo "host Xid (device pid): $(grep -c Xid "$C/host-xid.txt") lines"
        (( rc == 0 )) || break
    done
    if (( ok3 && ! stop )); then
        tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -2 "$C/gate-cuda.txt" | cut -c1-300
        n=$(foreign_xid); (( n > 0 )) && { echo "STOP: $n host Xid lines outside the device pid"; stop=1; }
    fi
elif [[ $mode == full ]]; then
    tools/caraxes-gate.sh nvml > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"; tail -2 "$C/gate-nvml.txt" | cut -c1-300
    tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -2 "$C/gate-cuda.txt" | cut -c1-300
    n=$(foreign_xid); (( n > 0 )) && { echo "STOP: $n host Xid lines outside the device pid"; stop=1; }
else
    # The cuda gate needs /dev/nvidia0, which the guest's nvidia-smi (run as root) creates;
    # the full mode's nvml gate does that first.
    "$VMSH" ssh "$SMI -L" > "$C/guest-smi-L.txt" 2>&1
    for t in cuda-init cuda-jit cuda-launch; do
        tools/caraxes-gate.sh cuda "$t" > "$C/gate-$t.txt" 2>&1; echo "$t rc=$?"; tail -1 "$C/gate-$t.txt" | cut -c1-300
        n=$(foreign_xid); (( n > 0 )) && { echo "STOP: $n host Xid lines outside the device pid"; stop=1; break; }
        grep -q ALERT "$C/hostwatch.txt" 2>/dev/null && { echo "STOP: host watch alert"; stop=1; break; }
    done
fi
"$VMSH" ssh 'lspci -nn -d 10de:; cat /proc/driver/nvidia/gpus/*/information' > "$C/guest-lspci.txt" 2>&1
"$VMSH" ssh "$SMI --query-gpu=name,compute_cap,pci.device_id,vbios_version,memory.total --format=csv; $SMI -q | head -60" > "$C/guest-nvidia-smi.txt" 2>&1
(( stop )) || "$VMSH" ssh "LD_LIBRARY_PATH=/opt/nvrm/lib timeout 120 /home/leandro/gpu/venv/bin/python -c 'import torch; print(torch.__version__, torch.version.cuda); print(torch.cuda.get_device_name(0)); print(torch.cuda.get_device_capability(0)); print(torch.cuda.get_device_properties(0))'" > "$C/guest-torch.txt" 2>&1
"$VMSH" ssh 'sudo dmesg | grep -iE "nvrm|xid" | tail -80' > "$C/guest-dmesg.txt" 2>&1
# Where the compute capability comes from: the RM controls nvidia-smi's query and torch's
# capability query send, through the v1 tracer (LD_PRELOAD, crates/nvrm-trace).
if (( ! stop )); then
    "$VMSH" ssh "T=/home/leandro/caraxes-tests/leandro-test/lib/libnvrm_trace.so; rm -f /tmp/cc-smi.tsv /tmp/cc-torch.tsv
        sudo LEA_TRACE_FILE=/tmp/cc-smi.tsv LEA_TRACE_FORMAT=tsv LD_PRELOAD=\$T LD_LIBRARY_PATH=/opt/nvrm/lib /opt/nvrm/bin/nvidia-smi --query-gpu=compute_cap --format=csv
        LEA_TRACE_FILE=/tmp/cc-torch.tsv LEA_TRACE_FORMAT=tsv LD_PRELOAD=\$T LD_LIBRARY_PATH=/opt/nvrm/lib timeout 120 /home/leandro/gpu/venv/bin/python -c 'import torch; print(torch.cuda.get_device_capability(0))'
        sudo chmod 644 /tmp/cc-*.tsv" > "$C/guest-cc-trace.txt" 2>&1
    "$VMSH" ssh 'cat /tmp/cc-smi.tsv' > "$C/cc-smi.tsv" 2>/dev/null
    "$VMSH" ssh 'cat /tmp/cc-torch.tsv' > "$C/cc-torch.tsv" 2>/dev/null
fi
caraxes/tools/bench-hostwatch.sh stop "$C"
grep -E 'unanswered|ERROR|refused|class_map|RC|rc notifier' "$W/gpu.log" | sort | uniq -c | sort -rn | head -30 | cut -c1-300 > "$C/device-log-issues.txt"
echo "device pid $DEVPID" > "$C/device-pid.txt"
"$VMSH" down > "$C/down.txt" 2>&1
n=$(foreign_xid)
for d in "$R"/caraxes/runs/gate-cuda-*; do
    [[ -d $d ]] && ! grep -qxF "$d" <<< "$BEFORE" && mv "$d" "$C/gate-cuda-run-$(basename "$d")"
done
echo "ladder GPU run done $(date -Iseconds); host Xid lines $(grep -c Xid "$C/host-xid.txt"), outside the device pid $n, device pid $DEVPID"

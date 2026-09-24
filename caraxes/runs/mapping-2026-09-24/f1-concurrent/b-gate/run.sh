#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The concurrent cuda gate of the concurrency check (docs/caraxes/parallel.md §8.4): vm1 with
# the 2 GiB profile at slot 0, vm2 with the 1 GiB profile at slot 4 (vm0 holds 2 GiB at slot 1),
# the guest test material synced, the cuda gate (scored tests plus probe/suites) on both at
# once under the host watch, then the teardown: vm1 clean, vm2 with one injected failed host
# free (fail-closed), and p20-check over both devices.
set -u
ROOT=/home/silas/git/Caraxes; F=$ROOT/caraxes/runs/mapping-2026-09-24/f1-concurrent/b-gate
cd "$ROOT"
export CARAXES_SERVER=$ROOT/target/mapping/release/vfio-user-nvgpu
unset CARAXES_BAR1_PASSTHROUGH CARAXES_DOORBELL_TOKENS
P=caraxes/tools/p20.sh
v1() { P20_SLOT=0 P20_FB_MIB=2048 "$P" vm1 "$@"; }
v2() { CARAXES_INJECT_TEARDOWN_FAILURE=1 P20_SLOT=4 P20_FB_MIB=1024 "$P" vm2 "$@"; }
log() { echo "$(date +%T) $*" | tee -a "$F/steps.txt"; }
date '+%Y-%m-%d %H:%M:%S' > "$F/t0.txt"
{ sha256sum "$CARAXES_SERVER" | cut -c1-16; git log -1 --format='%h %s'; } > "$F/device-sha256.txt"
"$P" - smi "$F/smi-before.txt"
log "vm1 up"; v1 up > "$F/vm1-up.txt" 2>&1 || { log "vm1 up failed"; exit 1; }
log "vm2 up"; v2 up > "$F/vm2-up.txt" 2>&1 || { log "vm2 up failed"; v1 down; exit 1; }
"$P" - smi "$F/smi-up.txt"
for v in vm1 vm2; do mkdir -p "$F/dev-$v"; echo "pid $(cat /mnt/vmstore/caraxes/$v/gpu.pid)" > "$F/dev-$v/pid.txt"; done
log "sync-tests"; v1 vm sync-tests > "$F/vm1-sync.txt" 2>&1; v2 vm sync-tests > "$F/vm2-sync.txt" 2>&1
gate() {  # vm
    local v=$1 t0; mkdir -p "$F/gate-$v"; t0=$(date +%s)
    (source <(sed -n '/^vm_env()/,/^}/p' "$P")
     case $v in vm1) P20_SLOT=0 P20_FB_MIB=2048 vm_env vm1 ;; vm2) P20_SLOT=4 P20_FB_MIB=1024 vm_env vm2 ;; esac
     export CARAXES_GATE_RUNS=$F/gate-$v CARAXES_TEST_TIMEOUT=300
     tools/caraxes-gate.sh cuda > "$F/gate-$v/verdict.json" 2> "$F/gate-$v/transcript.log"
     echo "exit=$? dur_s=$(( $(date +%s) - t0 )) start=$(date -d @$t0 +%T)" > "$F/gate-$v/exit.txt")
}
INTERVAL=5 caraxes/tools/bench-hostwatch.sh start "$F"
log "gates start"; gate vm1 & a=$!; gate vm2 & b=$!; wait $a $b; log "gates done: vm1 $(cat "$F/gate-vm1/exit.txt"), vm2 $(cat "$F/gate-vm2/exit.txt")"
caraxes/tools/bench-hostwatch.sh stop "$F" | tee -a "$F/steps.txt"
"$P" - smi "$F/smi-before-down.txt"
log "down"; v1 down > "$F/vm1-down.txt" 2>&1; v2 down > "$F/vm2-down.txt" 2>&1
"$P" - smi "$F/smi-after-down.txt"
for v in vm1 vm2; do
    W=/mnt/vmstore/caraxes/$v; mkdir -p "$F/device/$v"
    cp "$W/gpu.log" "$F/device/$v/gpu.log"; cp "$W/gpu.log" "$F/dev-$v/gpu.log"
    grep -a ' shadow: ' "$W/gpu-trace.log" > "$F/dev-$v/shadow.txt"
    grep -aE 'hostmem: (freed [0-9]+ of|freed the VRAM|freed [0-9]+ guest RAM|FAIL-CLOSED|earlier failed)|exit [0-9]|teardown failed' "$W/gpu.log" | tail -n 12 > "$F/dev-$v/teardown.txt"
done
"$P" - xid "$F/host-xid.txt" "$(cat "$F/t0.txt")"
python3 caraxes/tools/p20-check.py "$F/dev-vm1" "$F/dev-vm2" "$F/host-xid.txt" > "$F/check.txt" 2>&1; echo "p20-check exit $?" >> "$F/check.txt"
log "done"

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The P20 concurrency check on the mapping tree (docs/caraxes/parallel.md §8.4): vm1 with the
# 2 GiB profile at slot 0 and vm2 with the 1 GiB profile at slot 4 (vm0 holds 2 GiB at slot 1,
# so two 2 GiB profiles would break the 1 GiB desktop floor), the cuda gate on both at once
# under the host watch, the pattern test in both directions, clean teardown, p20-check.
# Run at 13:19 with F=.../f1-concurrent (the files were moved into a-pattern/ afterwards); the
# gate step failed (a script error, then stale guest test material), the pattern steps ran.
set -u
ROOT=/home/silas/git/Caraxes; F=$ROOT/caraxes/runs/mapping-2026-09-24/f1-concurrent/a-pattern
cd "$ROOT"
export CARAXES_SERVER=$ROOT/target/mapping/release/vfio-user-nvgpu
unset CARAXES_BAR1_PASSTHROUGH CARAXES_DOORBELL_TOKENS
P=caraxes/tools/p20.sh
v1() { P20_SLOT=0 P20_FB_MIB=2048 "$P" vm1 "$@"; }
v2() { P20_SLOT=4 P20_FB_MIB=1024 "$P" vm2 "$@"; }
log() { echo "$(date +%T) $*" | tee -a "$F/steps.txt"; }
if [[ ${SKIP_UP:-0} != 1 ]]; then   # SKIP_UP=1: the VMs are up already (the first attempt died at the gate step)
date '+%Y-%m-%d %H:%M:%S' > "$F/t0.txt"
{ sha256sum "$CARAXES_SERVER" | cut -c1-16; git log -1 --format='%h %s'; } > "$F/device-sha256.txt"
"$P" - smi "$F/smi-before.txt"
log "vm1 up"; v1 up > "$F/vm1-up.txt" 2>&1 || { log "vm1 up failed"; exit 1; }
log "vm2 up"; v2 up > "$F/vm2-up.txt" 2>&1 || { log "vm2 up failed"; exit 1; }
"$P" - smi "$F/smi-up.txt"
for v in vm1 vm2; do mkdir -p "$F/dev-$v"; echo "pid $(cat /mnt/vmstore/caraxes/$v/gpu.pid)" > "$F/dev-$v/pid.txt"; done
fi
gate() {  # vm
    local v=$1 t0; mkdir -p "$F/gate-$v"; t0=$(date +%s)
    (source <(sed -n '/^vm_env()/,/^}/p' "$P")
     case $v in vm1) P20_SLOT=0 P20_FB_MIB=2048 vm_env vm1 ;; vm2) P20_SLOT=4 P20_FB_MIB=1024 vm_env vm2 ;; esac
     export CARAXES_GATE_RUNS=$F/gate-$v CARAXES_TEST_TIMEOUT=300
     tools/caraxes-gate.sh cuda > "$F/gate-$v/verdict.json" 2> "$F/gate-$v/transcript.log"
     echo "exit=$? dur_s=$(( $(date +%s) - t0 )) start=$(date -d @$t0 +%T)" > "$F/gate-$v/exit.txt")
}
INTERVAL=5 caraxes/tools/bench-hostwatch.sh start "$F"
log "gates start"; gate vm1 & a=$!; gate vm2 & b=$!; wait $a $b; log "gates done"
caraxes/tools/bench-hostwatch.sh stop "$F" | tee -a "$F/steps.txt"
log "pattern: vm1 fills 1024 MiB A1 and holds, vm2 scans"
v1 vm ssh 'cd p20 && ./p20-patprobe fill 1024 0xa1 100' > "$F/vm1-fill-A1.txt" 2>&1 & h=$!
sleep 15; v2 vm ssh 'cd p20 && ./p20-patprobe scan' > "$F/vm2-scan-while-A1.txt" 2>&1
v1 vm ssh 'cd p20 && ./p20-patprobe scan' > "$F/vm1-scan-other-process-while-A1.txt" 2>&1
wait $h
log "pattern: vm2 fills 512 MiB B2 and holds, vm1 scans"
v2 vm ssh 'cd p20 && ./p20-patprobe fill 512 0xb2 60' > "$F/vm2-fill-B2.txt" 2>&1 & h=$!
sleep 15; v1 vm ssh 'cd p20 && ./p20-patprobe scan' > "$F/vm1-scan-while-B2.txt" 2>&1
wait $h
"$P" - smi "$F/smi-before-down.txt"
log "down"; v1 down > "$F/vm1-down.txt" 2>&1; v2 down > "$F/vm2-down.txt" 2>&1
"$P" - smi "$F/smi-after-down.txt"
for v in vm1 vm2; do
    W=/mnt/vmstore/caraxes/$v; mkdir -p "$F/dev-$v" "$F/device/$v"
    cp "$W/gpu.log" "$F/dev-$v/gpu.log"; cp "$W/gpu.log" "$F/device/$v/gpu.log"
    grep -a ' shadow: ' "$W/gpu-trace.log" > "$F/dev-$v/shadow.txt"
    grep -aE 'hostmem: (freed|FAIL-CLOSED|earlier)|exit|teardown' "$W/gpu.log" | tail -n 12 > "$F/dev-$v/teardown.txt"
done
"$P" - xid "$F/host-xid.txt" "$(cat "$F/t0.txt")"
python3 caraxes/tools/p20-check.py "$F/dev-vm1" "$F/dev-vm2" "$F/host-xid.txt" > "$F/check.txt" 2>&1; echo "p20-check exit $?" >> "$F/check.txt"
log "done"

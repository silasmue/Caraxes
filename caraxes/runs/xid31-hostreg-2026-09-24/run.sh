#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The Xid 31 of cuda-hostreg (vm1, 2026-09-24 19:38:37, caraxes/runs/gsp-bc-2026-09-24/gpu-vm1)
# on vm1 with the environment of caraxes/runs/gsp-bc-2026-09-24/gpu-vm1.sh (1 GiB profile,
# slot 4, 610 guest, token pool of 64, hybrid reactor, no BAR1 area), one VM start per mode:
#
#   run.sh before   the device of main 7621cfb (the failing binary), LOOPS=3 by default
#   run.sh after    the device with the fix (vfio-user-nvgpu-xid31-<rev>), LOOPS=20 by
#                   default, then the full cuda gate once (GATE=0 skips it)
#
# Each loop iteration runs caraxes/tools/hostreg-margin (pinned pages taken from the 2 MiB
# rounding of the guest's swiotlb alias, docs/caraxes/memory-model.md §2) and Leandro-Test's
# hostregprobe (the gate's cuda-hostreg workload). Output: <mode>/ with the device pid and
# every host Xid line of the window.
set -uo pipefail
TREE=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd -P)
source "$TREE/caraxes/runs/doorbell-2026-09-24/env-vm1.sh"
R=$TREE
MODE=${1:?usage: run.sh before|after}
case $MODE in
    before) SRV=/mnt/vmstore/caraxes/vfio-user-nvgpu-main-7621cfb; LOOPS=${LOOPS:-3}; GATE=${GATE:-0} ;;
    after) SRV=${SRV:-/mnt/vmstore/caraxes/vfio-user-nvgpu-xid31-9f3831c}; LOOPS=${LOOPS:-20}; GATE=${GATE:-1} ;;
    *) echo "usage: run.sh before|after" >&2; exit 2 ;;
esac
C=$R/caraxes/runs/xid31-hostreg-2026-09-24/${OUT:-$MODE}; mkdir -p "$C/loop"; cd "$R" || exit 1
export CARAXES_SERVER=$SRV LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_DRIVER CARAXES_GUEST_ABI
W=/mnt/vmstore/caraxes/$VM; T0=${T0:-$(date '+%F %T')}
VMSH=$R/tools/caraxes-vm.sh
LIB=/opt/nvrm/lib; HRP=/home/leandro/caraxes-tests/leandro-test/probe/bin/hostregprobe
# RESUME=1 T0=<start>: continue on the VM this mode already started (after a script fix).
if [[ ${RESUME:-0} != 1 ]]; then
{ head -1 /proc/driver/nvidia/version; git -C "$R" log --oneline -1; sha256sum "$CARAXES_SERVER"; env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; } > "$C/setup.txt"
caraxes/tools/bench-hostwatch.sh start "$C"
"$VMSH" up > "$C/up.txt" 2>&1 || { tail -5 "$C/up.txt"; caraxes/tools/bench-hostwatch.sh stop "$C"; exit 1; }
DEVPID=$(cat "$W/gpu.pid" 2>/dev/null); echo "device pid $DEVPID" > "$C/device-pid.txt"
"$VMSH" wait >> "$C/up.txt" 2>&1 || { tail -5 "$C/up.txt"; exit 1; }
"$VMSH" probe xid31 > "$C/probe.txt" 2>&1; echo "probe rc=$?"
"$VMSH" sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
fi
DEVPID=$(cat "$W/gpu.pid" 2>/dev/null)
grep -E 'dma table' "$W/gpu.log" | cut -c1-400 > "$C/device-dma.txt"
cp "$W/probe-xid31/dma.txt" "$C/guest-dma.txt"
pool=$(grep -o 'swiotlb pool \[0x[0-9a-f]*, 0x[0-9a-f]*)' "$C/guest-dma.txt" | tail -1)
[[ $pool =~ 0x([0-9a-f]+),\ 0x([0-9a-f]+) ]] || { echo "no swiotlb alias in guest-dma.txt; vm1 left up"; exit 1; }
lo=${BASH_REMATCH[1]} hi=${BASH_REMATCH[2]}
"$VMSH" ssh 'cat > hostreg-margin.c' < caraxes/tools/hostreg-margin.c
"$VMSH" ssh 'cuda=$(dirname $(ls /home/leandro/gpu/venv/lib/python3*/site-packages/nvidia/cu*/include/cuda.h | head -1)); gcc -O2 -Wall -o hostreg-margin hostreg-margin.c -I$cuda -L/opt/nvrm/lib -lcuda' > "$C/build.txt" 2>&1 || { cat "$C/build.txt"; exit 1; }
echo "# SPDX-License-Identifier: MIT" > "$C/loop.txt"
echo "# i margin_rc margin_summary hostreg_correct(of 2)" >> "$C/loop.txt"
mp=0 mn=0 hp=0
for i in $(seq 1 "$LOOPS"); do
    "$VMSH" ssh "sync; echo 3 | sudo tee /proc/sys/vm/drop_caches >/dev/null; sudo LD_LIBRARY_PATH=$LIB ./hostreg-margin $lo $hi" > "$C/loop/margin-$i.txt" 2>&1
    mrc=$?
    "$VMSH" ssh "LD_LIBRARY_PATH=$LIB $HRP" > "$C/loop/hostreg-$i.txt" 2>&1
    hn=$(grep -c 'bad=0 correct' "$C/loop/hostreg-$i.txt")
    # rc 2: no margin page was free this time (the loop cannot force the guest's free lists).
    ((mrc == 0)) && mp=$((mp + 1)); ((mrc == 2)) && mn=$((mn + 1)); ((hn == 2)) && hp=$((hp + 1))
    printf '%d %d %s | %s\n' "$i" "$mrc" "$(grep -E '^scanned' "$C/loop/margin-$i.txt")" "$hn" | tee -a "$C/loop.txt"
    tail -1 "$C/loop/margin-$i.txt" >> "$C/loop.txt"
done
echo "loop: hostreg-margin $mp of $LOOPS passed ($mn found no margin page), hostregprobe $hp of $LOOPS passed" | tee -a "$C/loop.txt"
if ((GATE)); then
    before=$(ls -d "$R"/caraxes/runs/gate-cuda-* 2>/dev/null)
    tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -3 "$C/gate-cuda.txt" | cut -c1-200
    for d in "$R"/caraxes/runs/gate-cuda-*; do
        [[ -d $d ]] && ! grep -qxF "$d" <<<"$before" && mv "$d" "$C/gate-cuda-run"
    done
fi
"$VMSH" ssh 'sudo dmesg | grep -iE "nvrm|xid" | tail -60' > "$C/guest-dmesg.txt" 2>&1
caraxes/tools/bench-hostwatch.sh stop "$C"
grep -E 'unanswered|ERROR|refused|REFUSED|guest aliases' "$W/gpu.log" | sort | uniq -c | sort -rn | head -20 | cut -c1-300 > "$C/device-log-issues.txt"
"$VMSH" down > "$C/down.txt" 2>&1
grep -E 'dma table|shadow: .*REFUSED|mirror: ' "$W/gpu.log" | cut -c1-600 > "$C/device-summary.txt"
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
echo "xid31-hostreg $MODE done $(date -Iseconds); device pid $DEVPID; Xid lines $(grep -c 'Xid' "$C/host-xid.txt"); $(grep -c "pid=$DEVPID" "$C/host-xid.txt") on the device pid"

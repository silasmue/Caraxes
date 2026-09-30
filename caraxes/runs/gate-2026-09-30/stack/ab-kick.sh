#!/usr/bin/env bash
# The gate's Linux B glmark2 [build] fell from 1309 (deferred-v2 b6d01293, gate-fix2) to 432 (the
# stack ffec4d49). An A/B on the same launcher after the gate's legs: dv2, then the stack again.
R=/mnt/vmstore/caraxes/wt-flagsrb G=/mnt/vmstore/caraxes/runs-2026-09-30/gate-stack
L=/mnt/vmstore/caraxes/locks/vmlock.sh
cd "$R" || exit 1
say() { echo "$(date +%T) $*" | tee -a "$G/legs.txt"; }
while kill -0 3251455 2>/dev/null || kill -0 3268632 2>/dev/null; do sleep 20; done
for x in dv2:/mnt/vmstore/caraxes/vfio-user-nvgpu-dv2fix2-b6d01293 stack:/mnt/vmstore/caraxes/vfio-user-nvgpu-stack-ffec4d49; do
    n=${x%%:*} b=${x#*:}
    bash $L desktop-b 3600 timeout 1800 caraxes/runs/night-2026-09-28/kick-smoke.sh $G/ab-kick-$n $b > $G/ab-kick-$n.log 2>&1
    say "ab-kick $n rc=$? $(grep -h 'glmark2\|RESULT' $G/ab-kick-$n/smoke.txt 2>/dev/null | tr '\n' ' ')"
done
say "ab done"

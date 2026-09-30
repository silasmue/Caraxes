#!/usr/bin/env bash
# The gather-cache A/B series: train1 (without) and train2 (with), ABAB, each session in vmlock.
H=/mnt/vmstore/caraxes/runs-2026-09-30/train
A=/mnt/vmstore/caraxes/vfio-user-nvgpu-train1-026b837d
B=/mnt/vmstore/caraxes/vfio-user-nvgpu-train2-148165cd
for s in "A1 $A" "B1 $B" "A2 $A" "B2 $B"; do
    set -- $s
    echo "$(date +%T) start $1 $2"
    /mnt/vmstore/caraxes/locks/vmlock.sh vm2-615 1800 timeout 1500 "$H/ab.sh" "$2" "$1"
    echo "$(date +%T) end $1 rc=$?"
done

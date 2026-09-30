#!/usr/bin/env bash
# Round 2 of the gather-cache A/B, reverse order (B then A), each session in vmlock with a long wait
# (the priority rule of 2026-09-30 00:05: Windows C95 first).
H=/mnt/vmstore/caraxes/runs-2026-09-30/train
A=/mnt/vmstore/caraxes/vfio-user-nvgpu-train1-026b837d
B=/mnt/vmstore/caraxes/vfio-user-nvgpu-train2-148165cd
for s in "B3 $B" "A3 $A"; do
    set -- $s
    echo "$(date +%T) start $1 $2"
    /mnt/vmstore/caraxes/locks/vmlock.sh vm2-615 3600 timeout 1500 "$H/ab.sh" "$2" "$1"
    echo "$(date +%T) end $1 rc=$?"
done

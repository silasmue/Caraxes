#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# P20 (docs/caraxes/parallel.md item 2), run in the guest: nvprobe 2, 3 and 4 in order
# with wall times, as the cuda gate runs them. Prints one line per stage plus nvprobe 4's
# ITERS and MSPERLAUNCH. Usage (from the host):
#   caraxes/tools/p20.sh vm1 vm ssh 'bash -s' < caraxes/tools/p20-cuda-guest.sh
LT=$HOME/caraxes-tests/leandro-test
export NVPROBE_PTX=$LT/probe/kernels/kernels.ptx
for s in 2 3 4; do
    t0=$(date +%s.%N)
    out=$(timeout -k 5 300 "$LT/probe/bin/nvprobe" $s 2>&1); rc=$?
    t1=$(date +%s.%N)
    printf 'stage %s rc=%s wall_s=%.2f start=%s last=%s\n' "$s" "$rc" "$(echo "$t1 - $t0" | bc)" \
        "$(date -d @${t0%.*} +%T)" "$(echo "$out" | grep -E 'stage [0-9] (ok|FAIL)|ERROR|WRONG' | tail -1)"
    [[ $s == 4 ]] && echo "$out" | grep -E '^(ITERS|MSPERLAUNCH)='
done

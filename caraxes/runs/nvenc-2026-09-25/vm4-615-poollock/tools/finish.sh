#!/usr/bin/env bash
# finish.sh <name> [note]: guest journal, down-615.sh, host state, run summary.
S=/tmp/claude-1000/-home-silas-git-Caraxes/43784b5f-fc25-49bd-8e67-5dbab7e19aca/scratchpad
N=/home/silas/git/Caraxes/caraxes/runs/nvenc-2026-09-25/vm4-615-poollock/$1; R=/home/silas/git/Caraxes; W=/mnt/vmstore/caraxes/vm4-615; TR=$W/gpu-trace.log
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout 60 tools/caraxes-vm.sh ssh "$1"); }
[[ -n ${2:-} ]] && echo "$(date +%T) $2" >> $N/run.log
g 'pgrep -x ShadowOfTheTomb; journalctl -b --no-pager -o short-monotonic 2>/dev/null | grep -E "steam\[" | grep -iE "750920|crash|signal|Vulkan|Feral|Shadow" | tail -60' > $N/guest-journal-game.txt 2>&1
T0=$(cat $N/t0); (cd $R/caraxes/runs/sottr-2026-09-24 && ./down-615.sh) > $N/down-console.txt 2>&1; tail -1 $N/down-console.txt
{ date -Iseconds; nvidia-smi --query-gpu=memory.used,memory.free --format=csv; nvidia-smi -q -d MEMORY | sed -n '/BAR1/,/Free/p'; grep MemAvailable /proc/meminfo; } > $N/host-after-down.txt
journalctl -k --since "$T0" --no-pager > $N/host-kernel-run.txt; cp $W/gpu.log $N/gpu.log
w=$(grep -c ' W bar0 ' $TR); s=$(grep -c 'held the device' $TR); span=$(tail -c 3000 $TR | tail -1 | awk '{print $1/1e6}')
echo "trapped_bar0_writes=$w slow=$s pct=$(echo "scale=3; $s*100/($w+1)" | bc) span_s=$span per_min=$(echo "scale=1; $s*60/$span" | bc)" > $N/slowpoll-total.txt
grep 'held the device' $TR | cut -c1-800 | python3 $S/sp.py >> $N/slowpoll-total.txt
echo "va_space_alloc_failed=$(grep -c 'VA Space alloc failed' $N/host-kernel-run.txt) xid=$(grep -c 'Xid' $N/host-kernel-run.txt)" >> $N/slowpoll-total.txt; cat $N/slowpoll-total.txt
echo "vm4-615 devices left: $(ps -eo args | grep -c '[v]m4-615/gpu')"

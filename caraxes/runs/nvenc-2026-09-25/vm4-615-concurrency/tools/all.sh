#!/bin/bash
# all.sh: claim the host (linux-busy), setup, matrix, down, release.
C=/tmp/claude-1000/-home-silas-git-Caraxes/43784b5f-fc25-49bd-8e67-5dbab7e19aca/scratchpad/conc
R=/home/silas/git/Caraxes; ST=$R/caraxes/runs/sottr-2026-09-24; W=/mnt/vmstore/caraxes/vm4-615
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-concurrency; B=/mnt/vmstore/caraxes/linux-busy
[[ -e /mnt/vmstore/caraxes/win-busy ]] && { echo "win-busy present"; exit 1; }
( set -o noclobber; echo "vm4-615-concurrency $$ $(date +%T)" > $B ) 2>/dev/null || { echo "linux-busy taken: $(cat $B)"; exit 1; }
trap 'rm -f $B' EXIT
bash $C/setup.sh || { echo "setup failed"; }
if ! grep -q '^build rc=0' $N/guest-setup.txt; then
  echo "$(date +%T) build problem: waiting up to 20 min for $C/fixed" >> $N/run.log
  for i in $(seq 120); do [[ -e $C/fixed ]] && break; sleep 10; done
fi
[[ -e $C/abort ]] || bash $C/matrix.sh ${PHASES:-H D X I S}
echo "$(date +%T) down" >> $N/run.log
(cd $ST && ./down-615.sh) > $N/down-console.txt 2>&1
{ date -Iseconds; nvidia-smi --query-gpu=memory.used,memory.free --format=csv; nvidia-smi -q -d MEMORY | sed -n '/BAR1/,/Free/p'; grep MemAvailable /proc/meminfo; } > $N/host-after-down.txt
T0=$(cat $N/t0); journalctl -k --since "$T0" --no-pager > $N/host-kernel-run.txt 2>/dev/null
cp $W/gpu.log $N/gpu.log 2>/dev/null; xz -f -T1 $N/gpu.log
grep -E 'ERROR' $N/host-kernel-run.txt | head -0
echo "$(date +%T) done; vm4-615 devices left: $(ps -eo args | grep -c '[v]m4-615/gpu')" | tee -a $N/run.log

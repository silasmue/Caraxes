#!/usr/bin/env bash
# guard.sh <outdir>: host guard every 15 s while vm4-615 runs; on a trip it records why and runs down-615.sh.
OUT=$1; T0=$(cat "$OUT/t0"); W=/mnt/vmstore/caraxes/vm4-615
echo $$ > "$OUT/guard.pid"
while [[ -s $W/ch.pid ]] && kill -0 "$(cat $W/ch.pid)" 2>/dev/null; do
  free=$(nvidia-smi --query-gpu=memory.free --format=csv,noheader,nounits 2>/dev/null | head -1)
  avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
  xid=$(journalctl -k --since "$T0" --no-pager -g Xid 2>/dev/null | grep -vc '^-- ')
  echo "$(date +%T) vram_free=${free}MiB memavail=${avail}MiB xid=$xid" >> "$OUT/guard.log"
  why=""
  [[ -n $free ]] && (( free < 600 )) && why="host VRAM free ${free} MiB < 600"
  (( avail < 3072 )) && why="$why MemAvailable ${avail} MiB < 3 GB"
  (( xid > 0 )) && why="$why Xid ($xid line(s))"
  if [[ -n $why ]]; then
    echo "$(date +%T) GUARD TRIP: $why" | tee -a "$OUT/guard.log" > "$OUT/GUARD-TRIPPED"
    journalctl -k --since "$T0" --no-pager -g Xid > "$OUT/guard-xid.txt" 2>&1
    /home/silas/git/Caraxes/caraxes/runs/sottr-2026-09-24/down-615.sh >> "$OUT/guard-down.txt" 2>&1
    break
  fi
  sleep 15
done
rm -f "$OUT/guard.pid"

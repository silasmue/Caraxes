#!/usr/bin/env bash
# guard.sh <rundir> <t0>: every 15 s while vm4-615 runs; MemAvailable < 3 GB or an Xid -> record and down.
N=$1; T0=$2; W=/mnt/vmstore/caraxes/vm4-615; ST=$(dirname "$0")/stage
echo $$ > "$N/guard.pid"
sleep 20
while [[ -s $W/ch.pid ]] && kill -0 "$(cat $W/ch.pid)" 2>/dev/null; do
  avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
  xid=$(journalctl -k --since "$T0" --no-pager -g Xid 2>/dev/null | grep -vc '^-- ')
  why=""
  (( avail < 3072 )) && why="MemAvailable ${avail} MiB < 3 GB"
  (( xid > 0 )) && why="$why Xid ($xid line(s))"
  if [[ -n $why ]]; then
    echo "$(date +%T) GUARD TRIP: $why" > "$N/GUARD-TRIPPED"
    journalctl -k --since "$T0" --no-pager -g Xid > "$N/guard-xid.txt" 2>&1
    (cd "$ST" && ./down-615.sh) >> "$N/guard-down.txt" 2>&1
    break
  fi
  sleep 15
done
rm -f "$N/guard.pid"

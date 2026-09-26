#!/bin/bash
# cell.sh <label> <spec>...: one matrix cell, host side (vm4-615 up).
set -u
L=$1; shift
R=/home/silas/git/Caraxes; W=/mnt/vmstore/caraxes/vm4-615; TR=$W/gpu-trace.log
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-concurrency; D=$N/$L; mkdir -p "$D"
T0=$(cat "$N/t0")
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-400} tools/caraxes-vm.sh ssh "$1"); }
now_tr() { tail -c 8000 "$TR" | tail -1 | awk '{print $1}'; }
s0=$(now_tr); h0=$(date +%s.%N); l0=$(wc -l < $W/gpu.log)
g "bash ~/conc/${GS:-grun.sh} $L $*" > "$D/guest.txt" 2>&1
s1=$(now_tr); h1=$(date +%s.%N); l1=$(wc -l < $W/gpu.log)
g "tar -C ~/conc/out -cf - $L" 2>/dev/null | tar -C "$D" --strip-components=1 -xf - 2>/dev/null
echo "$s0 $s1 $h0 $h1" > "$D/window.txt"
awk -v a="$s0" -v b="$s1" '$1>=a && $1<=b && / W bar0 0x00696000 /{n++} END{d=(b-a)/1e6; printf "window_s=%.1f kicks=%d kicks_per_s=%.1f\n", d, n, (d>0?n/d:0)}' "$TR" > "$D/kicks.txt"
sed -n "$((l0+1)),${l1}p" $W/gpu.log | grep -E 'ERROR' | cut -c1-200 | sed -E 's/0x[0-9a-f]+/0x_/g' | sort | uniq -c | sort -rn > "$D/device-errors.txt"
nerr=$(awk '{s+=$1} END{print s+0}' "$D/device-errors.txt")
hwsig=$(grep -c 'SIGTERM\|nvidia-smi took' $N/session/hostwatch.txt 2>/dev/null); xid=$(journalctl -k --since "$T0" --no-pager -g Xid 2>/dev/null | grep -vc '^-- ')
avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
# failure windows: for each non-PASS process, trace from (start of the stuck call - 10 s) to (+10 s)
for f in "$D"/p*.out; do
  [[ -f $f ]] || continue
  grep -q "^RESULT PASS" "$f" && continue; [[ $f == *-hold.out ]] && continue
  st=$(awk '/^START/{split($2,a,"="); print a[2]}' "$f"); t=$(grep -o ' t=[0-9.]*' "$f" | head -1 | cut -d= -f2)
  [[ -z $st ]] && continue; [[ -z $t ]] && t=0
  back=0; grep -q 'RESULT HANG' "$f" && { back=30; grep -q 'dispatch=' "$f" && back=10; }
  ft=$(echo "$st + $t - $back" | bc)
  ftr=$(echo "$s0 + ($ft - $h0)*1000000" | bc | cut -d. -f1)
  b=$(basename "$f" .out)
  awk -v a="$((ftr-10000000))" -v b="$((ftr+10000000))" '$1>=a && $1<=b' "$TR" | grep -v PROM | cut -c1-400 | xz -T1 > "$D/$b-trace-20s.txt.xz"
  echo "$b fail_trace_us=$ftr" >> "$D/failures.txt"
  python3 /tmp/claude-1000/-home-silas-git-Caraxes/43784b5f-fc25-49bd-8e67-5dbab7e19aca/scratchpad/conc/stall.py "$D/$b-trace-20s.txt.xz" "$D"/*.rm > "$D/$b-stall.txt" 2>&1
done
res=$(grep -h '^RESULT' "$D"/p*.out 2>/dev/null | sed 's/^RESULT //' | awk '{print $1}' | sort | uniq -c | tr -s ' ' | tr '\n' ',')
echo "$(date +%T) $L | $res | $(cat "$D/kicks.txt") | dev_errors=$nerr | xid=$xid | hostwatch_sigterm_lines=${hwsig:-0} | memavail=${avail}MiB" | tee -a "$N/table.txt"
for f in "$D"/p*.out; do echo "   $(basename $f): $(grep -h '^RESULT' $f)"; done | tee -a "$N/table.txt"
(( xid > 0 )) && { echo "XID" > "$N/STOP"; }
(( avail < 3072 )) && { echo "MEM" > "$N/STOP"; }
exit 0

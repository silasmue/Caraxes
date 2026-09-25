#!/usr/bin/env bash
# cycles.sh <name> [KEY=VAL ...]: up, then 5 Moonlight connect/disconnect cycles with vkcube and a short glmark2 after each connect, down.
set -u
NAME=$1; shift
S=/tmp/claude-1000/-home-silas-git-Caraxes/43784b5f-fc25-49bd-8e67-5dbab7e19aca/scratchpad
M=$S/m.sh; R=/home/silas/git/Caraxes; ST=$R/caraxes/runs/sottr-2026-09-24
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-poollock/$NAME; W=/mnt/vmstore/caraxes/vm4-615; TR=$W/gpu-trace.log
mkdir -p "$N"
log() { echo "$(date +%T) $*" | tee -a "$N/run.log"; }
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-120} tools/caraxes-vm.sh ssh "$1"); }
tripped() { [[ -f $N/session/GUARD-TRIPPED ]]; }
SENV='export XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-0;'
log "run $NAME env: $*"
{ date -Iseconds; nvidia-smi --query-gpu=memory.used,memory.free --format=csv; nvidia-smi -q -d MEMORY | sed -n '/BAR1/,/Free/p'; grep MemAvailable /proc/meminfo; } > "$N/host-baseline.txt"
T0=$(date '+%F %T')
(cd "$ST" && env "$@" SOTTR_SUN_ENCODER=nvenc CARAXES_MEM=8G ./up-615.sh) > "$N/up-console.txt" 2>&1; rc=$?
log "up rc=$rc"
OUT=$(cat "$W/sottr-session"); ln -sfn "$OUT" "$N/session"; cp "$OUT/t0" "$N/t0"; rm -f "$N/last_tr"
setsid nohup bash $S/guard.sh "$OUT" > /dev/null 2>&1 < /dev/null &
grep -iE 'dma-buf mapping|POOL_PIN' "$W/gpu.log" | head -2 | cut -c1-200 > "$N/pool-mode.txt"
[[ $rc == 0 ]] || { (cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1; exit 1; }
gl() { g "$SENV"' timeout 40 glmark2-es2-wayland -b build:duration=8 2>&1 | grep -E "FPS|Score"; echo "glrc=${PIPESTATUS[0]}"'; }
for i in 1 2 3 4 5; do
  (cd "$ST" && ./stream-615.sh) > "$N/stream-$i.txt" 2>&1
  for k in $(seq 60); do [[ $(grep -ac 'Received first video packet' "$OUT/moonlight.log") -ge $i ]] && break; sleep 2; done
  log "cycle $i connect: $(grep -a 'Received first video packet\|Connection terminated' "$OUT/moonlight.log" | tail -1 | cut -c1-90)"
  sleep 5
  bash $M "$N" step c$i-vkcube-after-connect vk
  gl > "$N/c$i-vkcube-after-connect/glmark2.txt" 2>&1; log "cycle $i glmark2: $(tr '\n' ' ' < "$N/c$i-vkcube-after-connect/glmark2.txt" | cut -c1-120)"
  g 'grep -a "Frame processing latency" ~/.config/sunshine/sunshine.log | tail -2; grep -aiE "fatal|hang detected" ~/.config/sunshine/sunshine.log | tail -2' > "$N/c$i-sunshine.txt" 2>&1
  mp=$(cat "$W/moonlight.pid" 2>/dev/null); [[ -n $mp ]] && kill -0 "$mp" 2>/dev/null && [[ $(cat /proc/$mp/comm) == moonlight ]] && kill "$mp"; rm -f "$W/moonlight.pid"
  log "cycle $i: moonlight pid $mp killed"
  sleep 30
  g 'pgrep -x sunshine >/dev/null && echo sunshine alive || echo sunshine GONE; grep -aiE "fatal|hang detected" ~/.config/sunshine/sunshine.log | tail -2' > "$N/c$i-after-disconnect.txt" 2>&1
  gl > "$N/c$i-glmark2-after-disconnect.txt" 2>&1
  log "cycle $i after disconnect: $(tr '\n' ' ' < "$N/c$i-after-disconnect.txt" | cut -c1-100) | glmark2 $(tr '\n' ' ' < "$N/c$i-glmark2-after-disconnect.txt" | cut -c1-100)"
  tripped && { log "guard tripped"; break; }
done
g 'bash caraxes-sottr/sottr-session.sh sunshine-stop; cp ~/.config/sunshine/sunshine.log ~/sunshine-cycles-$(date +%H%M).log' > "$N/sunshine-stop.txt" 2>&1
sleep 10; bash $M "$N" step f-vkcube-after-sunshine-stop vk
log "down"; (cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1
{ date -Iseconds; nvidia-smi --query-gpu=memory.used,memory.free --format=csv; nvidia-smi -q -d MEMORY | sed -n '/BAR1/,/Free/p'; grep MemAvailable /proc/meminfo; } > "$N/host-after-down.txt"
journalctl -k --since "$T0" --no-pager > "$N/host-kernel-run.txt"; cp "$W/gpu.log" "$N/gpu.log"
grep -E 'ERROR' "$W/gpu.log" | cut -c1-220 | sort | uniq -c | sort -rn | head -20 > "$N/device-errors.txt"
w=$(grep -c ' W bar0 ' "$TR"); s=$(grep -c 'held the device' "$TR"); span=$(tail -c 3000 "$TR" | tail -1 | awk '{print $1/1e6}')
echo "trapped_bar0_writes=$w slow=$s pct=$(echo "scale=3; $s*100/($w+1)" | bc) span_s=$span per_min=$(echo "scale=1; $s*60/$span" | bc)" > "$N/slowpoll-total.txt"
grep 'held the device' "$TR" | cut -c1-800 | python3 $S/sp.py >> "$N/slowpoll-total.txt"
echo "va_space_alloc_failed=$(grep -c 'VA Space alloc failed' "$N/host-kernel-run.txt") xid=$(grep -c 'Xid' "$N/host-kernel-run.txt")" >> "$N/slowpoll-total.txt"
log "done: $(head -1 "$N/slowpoll-total.txt")"

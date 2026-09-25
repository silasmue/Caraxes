#!/usr/bin/env bash
# soak.sh <name> [KEY=VAL ...]: up, stream, an off-screen and an on-screen glmark2 while streaming, 30 min idle streaming under soak-watch.sh, down.
set -u
NAME=$1; shift
S=/tmp/claude-1000/-home-silas-git-Caraxes/43784b5f-fc25-49bd-8e67-5dbab7e19aca/scratchpad
M=$S/m.sh; R=/home/silas/git/Caraxes; ST=$R/caraxes/runs/sottr-2026-09-24
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-poollock/$NAME; W=/mnt/vmstore/caraxes/vm4-615; TR=$W/gpu-trace.log
CSV=/mnt/vmstore/caraxes/soak-2026-09-25-poollock-$NAME.csv
mkdir -p "$N"
log() { echo "$(date +%T) $*" | tee -a "$N/run.log"; }
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-120} tools/caraxes-vm.sh ssh "$1"); }
SENV='export XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-0;'
log "run $NAME env: $*"
T0=$(date '+%F %T')
(cd "$ST" && env "$@" SOTTR_SUN_ENCODER=nvenc CARAXES_MEM=8G ./up-615.sh) > "$N/up-console.txt" 2>&1; rc=$?
log "up rc=$rc"
OUT=$(cat "$W/sottr-session"); ln -sfn "$OUT" "$N/session"; cp "$OUT/t0" "$N/t0"; rm -f "$N/last_tr"
setsid nohup bash $S/guard.sh "$OUT" > /dev/null 2>&1 < /dev/null &
[[ $rc == 0 ]] || { (cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1; exit 1; }
g "$SENV"' echo "== offscreen, sunshine idle"; timeout 40 glmark2-es2-wayland --off-screen -b build:duration=8 2>&1 | grep -E "FPS|Score"; echo "rc=${PIPESTATUS[0]}"' > "$N/glmark2-sunshine-idle.txt" 2>&1
(cd "$ST" && ./stream-615.sh) > "$N/stream-console.txt" 2>&1
for k in $(seq 60); do grep -qa 'Received first video packet' "$OUT/moonlight.log" && break; sleep 2; done
log "moonlight: $(grep -a -m1 'Received first video packet' "$OUT/moonlight.log" | cut -c1-80)"
sleep 5
g "$SENV"' echo "== offscreen, streaming"; timeout 40 glmark2-es2-wayland --off-screen -b build:duration=8 2>&1 | grep -E "FPS|Score"; echo "rc=${PIPESTATUS[0]}"; echo "== onscreen, streaming"; timeout 40 glmark2-es2-wayland -b build:duration=8 2>&1 | grep -E "FPS|Score"; echo "rc=${PIPESTATUS[0]}"; echo "== vkmark offscreen? (headless winsys)"; timeout 60 vkmark --winsys headless -b vertex:duration=5 2>&1 | grep -E "FPS|Score|Error"; echo "rc=${PIPESTATUS[0]}"' > "$N/gl-while-streaming.txt" 2>&1
log "gl while streaming: $(tr '\n' ' ' < "$N/gl-while-streaming.txt" | cut -c1-300)"
setsid nohup bash $R/caraxes/tools/soak-watch.sh "$CSV" 1800 > "$N/soak-watch.log" 2>&1 < /dev/null &
SW=$!; echo $SW > "$N/soak-watch.pid"; ST0=$(date +%s)
bash $M "$N" step s01-streaming-idle-1min 20
bash $M "$N" cursor s02-cursor-streaming
while (( $(date +%s) < ST0 + 900 )); do sleep 15; done
bash $M "$N" step s15-streaming-idle-15min 20
while (( $(date +%s) < ST0 + 1740 )); do sleep 15; done
bash $M "$N" step s29-streaming-idle-29min 20
bash $M "$N" cursor s29-cursor-streaming
while kill -0 $SW 2>/dev/null; do sleep 5; done
bash $R/caraxes/tools/soak-watch.sh --summary "$CSV" > "$N/soak-summary.txt" 2>&1
cp "$CSV" "$N/" 2>/dev/null
log "down"; (cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1
{ date -Iseconds; nvidia-smi --query-gpu=memory.used,memory.free --format=csv; nvidia-smi -q -d MEMORY | sed -n '/BAR1/,/Free/p'; grep MemAvailable /proc/meminfo; } > "$N/host-after-down.txt"
journalctl -k --since "$T0" --no-pager > "$N/host-kernel-run.txt"; cp "$W/gpu.log" "$N/gpu.log"
w=$(grep -c ' W bar0 ' "$TR"); s=$(grep -c 'held the device' "$TR"); span=$(tail -c 3000 "$TR" | tail -1 | awk '{print $1/1e6}')
echo "trapped_bar0_writes=$w slow=$s pct=$(echo "scale=3; $s*100/($w+1)" | bc) span_s=$span per_min=$(echo "scale=1; $s*60/$span" | bc)" > "$N/slowpoll-total.txt"
grep 'held the device' "$TR" | cut -c1-800 | python3 $S/sp.py >> "$N/slowpoll-total.txt"
echo "va_space_alloc_failed=$(grep -c 'VA Space alloc failed' "$N/host-kernel-run.txt") xid=$(grep -c 'Xid' "$N/host-kernel-run.txt")" >> "$N/slowpoll-total.txt"
log "done: $(head -1 "$N/slowpoll-total.txt")"

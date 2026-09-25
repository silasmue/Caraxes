#!/usr/bin/env bash
# game.sh <name> [KEY=VAL ...]: up; SotTR with Sunshine running (crash check); Sunshine stopped, SotTR again for 10 min;
# then Sunshine started beside the running game and Moonlight connected (the game streamed); down.
set -u
NAME=$1; shift
S=/tmp/claude-1000/-home-silas-git-Caraxes/43784b5f-fc25-49bd-8e67-5dbab7e19aca/scratchpad
M=$S/m.sh; R=/home/silas/git/Caraxes; ST=$R/caraxes/runs/sottr-2026-09-24
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-poollock/$NAME; W=/mnt/vmstore/caraxes/vm4-615; TR=$W/gpu-trace.log
mkdir -p "$N"
log() { echo "$(date +%T) $*" | tee -a "$N/run.log"; }
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-120} tools/caraxes-vm.sh ssh "$1"); }
tripped() { [[ -f $N/session/GUARD-TRIPPED ]]; }
GENV='export XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-0 DISPLAY=:0 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus XDG_CURRENT_DESKTOP=GNOME;'
CR='$HOME/.local/share/feral-interactive/Shadow of the Tomb Raider/crashes'
launch() { g "$GENV"' touch /tmp/game-launch; sleep 1; setsid nohup steam steam://rungameid/750920 > /tmp/steam-launch.log 2>&1 < /dev/null & sleep 2' > /dev/null 2>&1; }
newdump() { g 'find "'"$CR"'" -maxdepth 1 -name "*.dmp" -newer /tmp/game-launch | grep -q .' 2>/dev/null; }
gamepid() { g 'pgrep -x ShadowOfTheTomb | head -1' 2>/dev/null; }
killdialog() { g 'for p in $(pgrep -x FeralLinuxMessa); do kill $p; echo "killed FeralLinuxMessage $p"; done' 2>/dev/null; }
log "run $NAME env: $*"
T0=$(date '+%F %T')
(cd "$ST" && env "$@" SOTTR_SUN_ENCODER=${SOTTR_SUN_ENCODER:-nvenc} CARAXES_MEM=8G ./up-615.sh) > "$N/up-console.txt" 2>&1; rc=$?
log "up rc=$rc"
OUT=$(cat "$W/sottr-session"); ln -sfn "$OUT" "$N/session"; cp "$OUT/t0" "$N/t0"; rm -f "$N/last_tr"
setsid nohup bash $S/guard.sh "$OUT" > /dev/null 2>&1 < /dev/null &
grep -iE 'dma-buf mapping|POOL_PIN' "$W/gpu.log" | head -2 | cut -c1-200 > "$N/pool-mode.txt"
[[ $rc == 0 ]] || { (cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1; exit 1; }
# 1. the game with Sunshine running (no client)
if [[ ${SKIP1:-0} != 1 ]]; then
  launch; log "g1: game launched with Sunshine running"
  c=0; for k in $(seq 9); do sleep 20; newdump && { c=1; break; }; done
  if (( c )); then log "g1: crashed: $(g 'cat "'"$CR"'/VersionInfo.txt"' | cut -d, -f5- | head -1)"; else log "g1: no crash in 3 min (pid $(gamepid))"; fi
  bash $M "$N" step g1-game-sunshine-running 20
  killdialog >> "$N/run.log"; p=$(gamepid); [[ -n $p ]] && g "kill $p" && log "g1: game pid $p ended"; sleep 10
fi
# 2. Sunshine stopped, the game again
g 'bash caraxes-sottr/sottr-session.sh sunshine-stop; cp ~/.config/sunshine/sunshine.log ~/sunshine-game-$(date +%H%M).log' > "$N/sunshine-stop.txt" 2>&1
sleep 5; bash $M "$N" step g2a-idle-nosunshine 20
launch; GL=$(date +%s); log "g2: game launched without Sunshine"
c=0; for k in $(seq 5); do sleep 20; newdump && { c=1; break; }; done
if (( c )); then
  log "g2: crashed: $(g 'cat "'"$CR"'/VersionInfo.txt"' | cut -d, -f5- | head -1)"; bash $M "$N" step g2-game-crash 20
else
  log "g2: running (pid $(gamepid))"
  bash $M "$N" step g2-game-2min 20
  bash $M "$N" cursor g2-cursor-game
  while (( $(date +%s) < GL + 300 )); do sleep 10; done
  bash $M "$N" step g2-game-5min 20
  while (( $(date +%s) < GL + 600 )); do sleep 10; done
  newdump && log "g2: crashed before 10 min"
  bash $M "$N" step g2-game-10min 20
  log "g2: at 10 min pid $(gamepid)"
  # 3. Sunshine beside the running game, then Moonlight
  GT=250 g "SUN_USER=lea SUN_PASS=leastream SUN_ENCODER=${SOTTR_SUN_ENCODER:-nvenc} bash caraxes-sottr/sottr-session.sh sunshine-start" > "$N/sunshine-start-game.txt" 2>&1
  log "g3: sunshine: $(grep -E 'Found H.264|did not|never' "$N/sunshine-start-game.txt" | head -1 | cut -c1-120)"
  (cd "$ST" && ./stream-615.sh) > "$N/stream-console.txt" 2>&1
  for k in $(seq 60); do grep -qa 'Received first video packet' "$OUT/moonlight.log" && break; sleep 2; done
  log "g3: moonlight: $(grep -a -m1 'Received first video packet' "$OUT/moonlight.log" | cut -c1-80)"
  sleep 10
  bash $M "$N" step g3-game-streaming 20
  bash $M "$N" cursor g3-cursor-game-streaming
  sleep 120
  newdump && log "g3: crashed while streaming"
  bash $M "$N" step g3-game-streaming-3min 20
  log "g3: pid $(gamepid)"
fi
g 'journalctl -b --no-pager -o short-monotonic 2>/dev/null | grep -E "steam\[" | grep -iE "750920|crash|signal|Vulkan|Feral|Shadow|name:|renderer" | tail -200' > "$N/guest-journal-game.txt" 2>&1
g 'D="'"$CR"'"; ls -la --time-style=+%T "$D"; cat "$D/VersionInfo.txt"' > "$N/game-crashes-dir.txt" 2>&1
log "down"; (cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1
{ date -Iseconds; nvidia-smi --query-gpu=memory.used,memory.free --format=csv; nvidia-smi -q -d MEMORY | sed -n '/BAR1/,/Free/p'; grep MemAvailable /proc/meminfo; } > "$N/host-after-down.txt"
journalctl -k --since "$T0" --no-pager > "$N/host-kernel-run.txt"; cp "$W/gpu.log" "$N/gpu.log"
w=$(grep -c ' W bar0 ' "$TR"); s=$(grep -c 'held the device' "$TR"); span=$(tail -c 3000 "$TR" | tail -1 | awk '{print $1/1e6}')
echo "trapped_bar0_writes=$w slow=$s pct=$(echo "scale=3; $s*100/($w+1)" | bc) span_s=$span per_min=$(echo "scale=1; $s*60/$span" | bc)" > "$N/slowpoll-total.txt"
grep 'held the device' "$TR" | cut -c1-800 | python3 $S/sp.py >> "$N/slowpoll-total.txt"
echo "va_space_alloc_failed=$(grep -c 'VA Space alloc failed' "$N/host-kernel-run.txt") xid=$(grep -c 'Xid' "$N/host-kernel-run.txt")" >> "$N/slowpoll-total.txt"
log "done: $(head -1 "$N/slowpoll-total.txt")"

#!/usr/bin/env bash
# run.sh <name> [KEY=VAL ...]: one configuration of the poollock night run, all steps; env from args.
set -u
NAME=$1; shift
S=/tmp/claude-1000/-home-silas-git-Caraxes/43784b5f-fc25-49bd-8e67-5dbab7e19aca/scratchpad
M=$S/m.sh
R=/home/silas/git/Caraxes
ST=$R/caraxes/runs/sottr-2026-09-24
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-poollock/$NAME
W=/mnt/vmstore/caraxes/vm4-615
TR=$W/gpu-trace.log
mkdir -p "$N"
log() { echo "$(date +%T) $*" | tee -a "$N/run.log"; }
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-120} tools/caraxes-vm.sh ssh "$1"); }
tripped() { [[ -f $(cat "$W/sottr-session" 2>/dev/null)/GUARD-TRIPPED ]] || [[ -f $N/session/GUARD-TRIPPED ]]; }
SKIPGAME=${SKIPGAME:-0}; GAMEMIN=${GAMEMIN:-10}
log "run $NAME env: $*"
{ date -Iseconds; nvidia-smi --query-gpu=memory.used,memory.free --format=csv; nvidia-smi -q -d MEMORY | sed -n '/BAR1/,/Free/p'; grep MemAvailable /proc/meminfo; cat /proc/loadavg; } > "$N/host-baseline.txt"
T0=$(date '+%F %T')
(cd "$ST" && env "$@" SOTTR_SUN_ENCODER=${SOTTR_SUN_ENCODER:-nvenc} CARAXES_MEM=8G ./up-615.sh) > "$N/up-console.txt" 2>&1; rc=$?
log "up rc=$rc"
OUT=$(cat "$W/sottr-session" 2>/dev/null); ln -sfn "$OUT" "$N/session"; cp "$OUT/t0" "$N/t0" 2>/dev/null || echo "$T0" > "$N/t0"
rm -f "$N/last_tr"
setsid nohup bash $S/guard.sh "$OUT" > /dev/null 2>&1 < /dev/null &
grep -iE 'dma-buf mapping|POOL_PIN|pool' "$W/gpu.log" | head -3 | cut -c1-300 > "$N/pool-mode.txt"; log "pool mode: $(head -1 "$N/pool-mode.txt" | cut -c1-160)"
grep -E 'ERROR' "$W/gpu.log" | cut -c1-200 | sort | uniq -c | sort -rn | head > "$N/boot-errors.txt"
if [[ $rc != 0 ]]; then log "up failed"; (cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1; exit 1; fi
{ nvidia-smi -q -d MEMORY | sed -n '/BAR1/,/Free/p'; } > "$N/host-bar1-after-boot.txt"
grep -A3 '== vkcube' "$OUT/checks-session.txt" > "$N/stage-checks-vkcube.txt" 2>/dev/null
# a: no Sunshine
g 'cp ~/.config/sunshine/sunshine.log ~/sunshine-boot-probe-$(date +%H%M).log; bash caraxes-sottr/sottr-session.sh sunshine-stop' > "$N/sunshine-stop-a.txt" 2>&1
bash $M "$N" step a0-idle-nosunshine 20
bash $M "$N" step a-vkcube-nosunshine vk
bash $M "$N" cursor a-cursor-nosunshine
[[ ${SKIPBENCH:-0} == 1 ]] || bash $M "$N" bench a-bench-nosunshine
tripped && { log "guard tripped"; exit 2; }
# b: Sunshine, no client
GT=250 g "SUN_USER=lea SUN_PASS=leastream SUN_ENCODER=${SOTTR_SUN_ENCODER:-nvenc} bash caraxes-sottr/sottr-session.sh sunshine-start" > "$N/sunshine-restart.txt" 2>&1
log "sunshine: $(grep -E 'Found H.264|did not|never' "$N/sunshine-restart.txt" | head -1)"
bash $M "$N" step b-vkcube-sunshine-noclient vk
bash $M "$N" cursor b-cursor-noclient
tripped && { log "guard tripped"; exit 2; }
# c: stream
(cd "$ST" && ./stream-615.sh) > "$N/stream-console.txt" 2>&1
log "stream: $(tail -1 "$N/stream-console.txt" | cut -c1-80)"
for i in $(seq 60); do grep -qa 'Received first video packet\|Connection terminated\|failed' "$OUT/moonlight.log" 2>/dev/null && break; sleep 2; done
CONN=$(date +%s); log "moonlight: $(grep -a -m1 -E 'Received first video packet|Connection terminated|failed' "$OUT/moonlight.log" | cut -c1-100)"
sleep 5
bash $M "$N" step c-vkcube-after-connect vk
bash $M "$N" step c1-idle-streaming 20
bash $M "$N" cursor c2-cursor-streaming
[[ ${SKIPBENCH:-0} == 1 ]] || bash $M "$N" bench c3-bench-streaming
tripped && { log "guard tripped"; exit 2; }
while (( $(date +%s) < CONN + 300 )); do sleep 10; done
bash $M "$N" step d-vkcube-after-5min vk
log "moonlight state: $(grep -a -E 'Connection terminated|error' "$OUT/moonlight.log" | tail -1 | cut -c1-100)"
tripped && { log "guard tripped"; exit 2; }
# e: the game
if [[ $SKIPGAME != 1 ]]; then
  GL=$(date +%s); log "game launch"
  g 'export XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-0 DISPLAY=:0 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus XDG_CURRENT_DESKTOP=GNOME; date +%s > /tmp/game-launch; setsid nohup steam steam://rungameid/750920 > /tmp/steam-launch.log 2>&1 < /dev/null & sleep 2' > /dev/null 2>&1
  crashed=0; m1=0; m5=0
  while (( $(date +%s) < GL + GAMEMIN*60 )); do
    sleep 20
    st=$(g 'journalctl -b --no-pager 2>/dev/null | grep -E "steam\[" | awk -v t=$(cat /tmp/game-launch) "1" | grep -cE "Game crashed with signal"; pgrep -f "ShadowOfTheTombRaider$|bin/ShadowOfTheTomb" | head -1' 2>/dev/null | tr '\n' ' ')
    echo "$(date +%T) $st" >> "$N/game-watch.txt"
    c=$(awk '{print $1}' <<<"$st")
    if [[ ${c:-0} -gt ${CR0:=${c:-0}} ]]; then :; fi
    if (( ! crashed )) && g 'D="$HOME/.local/share/feral-interactive/Shadow of the Tomb Raider/crashes"; find "$D" -maxdepth 1 -name "*.dmp" -newer /tmp/game-launch | grep -q .' 2>/dev/null; then
      crashed=1; CT=$(date +%s); log "game crashed (new dump)"
      g 'D="$HOME/.local/share/feral-interactive/Shadow of the Tomb Raider/crashes"; cat "$D/VersionInfo.txt"; echo; for f in $(find "$D" -maxdepth 1 -name "*.dmp" -newer /tmp/game-launch); do ls -la --time-style=+%T "$f"; done' > "$N/game-crash-info.txt" 2>&1
      bash $M "$N" step e-game-crash 20
      break
    fi
    if (( ! m1 && $(date +%s) > GL + 90 )); then m1=1; bash $M "$N" step e1-game-1min 20; fi
    if (( ! m5 && $(date +%s) > GL + 300 )); then m5=1; bash $M "$N" step e5-game-5min 20; bash $M "$N" cursor e5-cursor-game; fi
    tripped && { log "guard tripped"; exit 2; }
  done
  (( crashed )) || bash $M "$N" step e10-game-end 20
  g 'journalctl -b --no-pager -o short-monotonic 2>/dev/null | grep -E "steam\[" | grep -iE "750920|crash|signal|Vulkan|Feral|Shadow|gpu|renderer|name:" | tail -150' > "$N/guest-journal-game.txt" 2>&1
  g 'sudo dmesg | grep -vE "NVRM: (ioctl|nvidia_open|nvidia_ctl|nvidia_close|GPU .*Opening)" | tail -200' > "$N/guest-dmesg-tail.txt" 2>&1
fi
{ nvidia-smi -q -d MEMORY | sed -n '/BAR1/,/Free/p'; nvidia-smi --query-gpu=memory.used,memory.free --format=csv; } > "$N/host-before-down.txt"
log "down"
(cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1
{ date -Iseconds; nvidia-smi --query-gpu=memory.used,memory.free --format=csv; nvidia-smi -q -d MEMORY | sed -n '/BAR1/,/Free/p'; grep MemAvailable /proc/meminfo; } > "$N/host-after-down.txt"
journalctl -k --since "$T0" --no-pager > "$N/host-kernel-run.txt"
cp "$W/gpu.log" "$N/gpu.log"
grep -E 'held the device over|^bar1 pool: guest BAR1' "$W/gpu.log" | cut -c1-1200 > "$N/device-pool-summary.txt"
w=$(grep -c ' W bar0 ' "$TR"); s=$(grep -c 'held the device' "$TR"); span=$(tail -c 3000 "$TR" | tail -1 | awk '{print $1/1e6}')
echo "trapped_bar0_writes=$w slow=$s pct=$(echo "scale=3; $s*100/($w+1)" | bc) span_s=$span per_min=$(echo "scale=1; $s*60/$span" | bc)" > "$N/slowpoll-total.txt"
grep 'held the device' "$TR" | cut -c1-800 | xz > "$N/slowpolls-all.txt.xz"
xzcat "$N/slowpolls-all.txt.xz" | python3 $S/sp.py >> "$N/slowpoll-total.txt"
echo "va_space_alloc_failed=$(grep -c 'VA Space alloc failed' "$N/host-kernel-run.txt") xid=$(grep -c 'Xid' "$N/host-kernel-run.txt")" >> "$N/slowpoll-total.txt"
log "done: $(head -1 "$N/slowpoll-total.txt")"

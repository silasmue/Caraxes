#!/usr/bin/env bash
# run.sh <name> <override file> <A|B|F>: one cell (after vm4-615-invhold/tools/run.sh).
#   F3: as F with the work test three times.
#   F: the A windows w1-w6, then vkcube and glmark2 while streaming (c0), no cycles.
#   A: w1 idle, w2 cursor (no client), stream, w3/w5 idle streaming, w4/w6 cursor streaming,
#      then five Moonlight disconnect/connect cycles with vkcube and glmark2 after each connect.
#   B: w1, w2, stream, w3 idle streaming, w4 cursor streaming.
set -u
NAME=$1; OVR=$2; MODE=$3
SP=$(cd "$(dirname "$0")" && pwd -P); ST=$SP/stage
R=/home/silas/git/Caraxes
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-flippace/$NAME
W=/mnt/vmstore/caraxes/vm4-615; TR=$W/gpu-trace.log
mkdir -p "$N"; cp "$OVR" "$N/override.env"
log() { echo "$(date +%T) $*" | tee -a "$N/run.log"; }
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-120} tools/caraxes-vm.sh ssh "$1"); }
tripped() { [[ -f $N/GUARD-TRIPPED ]]; }
xid31() { journalctl -k --since "$T0" --no-pager -g 'Xid.*: 31,' 2>/dev/null | grep -vc '^-- '; }
SENV='U=$(id -u); wd=wayland-0; for c in /run/user/$U/wayland-*; do case $c in *.lock) continue;; esac; [ -S "$c" ] && { wd=${c##*/}; break; }; done; export XDG_RUNTIME_DIR=/run/user/$U WAYLAND_DISPLAY=$wd DISPLAY=:0 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$U/bus XDG_CURRENT_DESKTOP=GNOME;'
T0=$(date '+%F %T'); echo "$T0" > "$N/t0"
log "run $NAME mode $MODE override $(grep -v '^#' "$OVR" | tr '\n' ';')"
export SOTTR_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-flippace-02c0a625 SOTTR_SUN_ENCODER=nvenc CARAXES_MEM=8G SOTTR_NAT=0 SOTTR_OUT=$N/session LAT_OVERRIDE=$OVR
{ date -Iseconds; grep MemAvailable /proc/meminfo; nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader; } > "$N/host-before.txt"
(cd "$ST" && ./up-615.sh) > "$N/up-console.txt" 2>&1; rc=$?
log "up rc=$rc"
setsid nohup bash "$SP/guard.sh" "$N" "$T0" > /dev/null 2>&1 < /dev/null &
grep -E '^vfio-user-nvgpu: (intr page|display page|msi re-arm|doorbell poll|bar2 memfd|WARN|ERROR held)' "$W/gpu.log" | cut -c1-240 > "$N/device-mode.txt"
if [[ $rc != 0 ]]; then log "up failed"; (cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1; exit 1; fi
grep -a -E 'Found H.264|Creating encoder|did not|never' "$N/session/sunshine-start.txt" | head -3 > "$N/sunshine-encoder.txt"
sleep 20
bash $SP/win.sh "$N" w1-idle-noclient idle
bash $SP/win.sh "$N" w2-cursor-noclient cursor
tripped && { log "guard tripped"; exit 2; }
connect() {  # connect <tag>: stream-615.sh, wait for the first video packet; returns 0 on video
  local L0 res
  L0=$(wc -l < "$N/session/moonlight.log" 2>/dev/null || echo 0)
  (cd "$ST" && ./stream-615.sh) > "$N/stream-console-$1.txt" 2>&1
  res=""
  for i in $(seq 60); do res=$(tail -n +$((L0 + 1)) "$N/session/moonlight.log" 2>/dev/null | grep -a -m1 -E 'Received first video packet|Connection terminated|No video traffic'); [[ -n $res ]] && break; sleep 2; done
  log "moonlight $1: $(cut -c1-100 <<<"$res")"
  grep -q 'Received first video packet' <<<"$res"
}
killml() {  # kill this stage's Moonlight by the pid in the session folder
  local mp
  mp=$(cat "$N/session/moonlight.pid" 2>/dev/null)
  if [[ -n $mp ]] && kill -0 "$mp" 2>/dev/null && [[ $(cat /proc/$mp/comm) == moonlight ]]; then kill "$mp"; log "moonlight pid $mp killed"; fi
  rm -f "$W/moonlight.pid"
}
ok=0
for a in 1 2 3; do
  GT=250 g "SUN_USER=lea SUN_PASS=leastream SUN_ENCODER=nvenc bash caraxes-sottr/sottr-session.sh sunshine-stop; SUN_USER=lea SUN_PASS=leastream SUN_ENCODER=nvenc bash caraxes-sottr/sottr-session.sh sunshine-start" > "$N/sunshine-restart-$a.txt" 2>&1
  if connect "attempt$a"; then ok=1; break; fi
  killml; sleep 5
done
(( ok )) || log "no stream after 3 attempts"
sleep 15
bash $SP/win.sh "$N" w3-idle-streaming idle
bash $SP/win.sh "$N" w4-cursor-streaming cursor
if [[ $MODE != B ]]; then
  bash $SP/win.sh "$N" w5-idle-streaming idle
  bash $SP/win.sh "$N" w6-cursor-streaming cursor
fi
tripped && { log "guard tripped"; exit 2; }
work() {  # work <label>: vkcube and a short glmark2 in the guest session
  local D=$N/$1; mkdir -p "$D"
  g "$SENV"' s=$(date +%s.%N); timeout 60 vkcube-wayland --c 120 > /tmp/vk.log 2>&1; rc=$?; e=$(date +%s.%N); echo "vkcube rc=$rc elapsed=$(echo "$e - $s" | bc)"; tail -2 /tmp/vk.log' > "$D/vkcube.txt" 2>&1
  g "$SENV"' timeout 40 glmark2-es2-wayland -b build:duration=8 2>&1 | grep -E "FPS|Score"; echo "glrc=${PIPESTATUS[0]}"' > "$D/glmark2.txt" 2>&1
  g 'grep -a "Frame processing latency" ~/.config/sunshine/sunshine.log | tail -2; grep -aiE "fatal|hang detected" ~/.config/sunshine/sunshine.log | tail -2' > "$D/sunshine.txt" 2>&1
  log "$1: $(head -1 "$D/vkcube.txt" | cut -c1-60) | $(tr '\n' ' ' < "$D/glmark2.txt" | cut -c1-120)"
}
[[ $MODE != B ]] && work c0-work-streaming
if [[ $MODE == F3 ]]; then sleep 20; work c0b-work-streaming; sleep 20; work c0c-work-streaming; fi
if [[ $MODE == A ]]; then
  for c in 1 2 3 4 5; do
    killml; sleep 30
    connect "cycle$c" || log "cycle $c: no video"
    sleep 5
    work "c$c-work-after-connect"
    x=$(xid31); log "cycle $c: host Xid 31 lines so far: $x"
    (( x > 0 )) && { log "Xid 31: stop"; break; }
    tripped && { log "guard tripped"; break; }
  done
fi
g 'sudo dmesg' > "$N/guest-dmesg.txt" 2>&1
log "flip warnings: $(grep -c 'WARNING.*__nv_drm_handle_flip_event' "$N/guest-dmesg.txt"); TLB invalidation failed: $(grep -c 'TLB invalidation failed waiting' "$N/guest-dmesg.txt")"
log "down"
(cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1
cp "$W/gpu.log" "$N/gpu.log"
grep -E '^(bar2 memfd:|doorbell poll \(|intr page:|display page:|mirror:|bar0 config-mirror|display engine:|interrupts)' "$W/gpu.log" | cut -c1-2500 > "$N/device-summary.txt"
echo "slow_poll_trace_lines=$(grep -c 'held the device' "$TR")" > "$N/slowpoll.txt"
journalctl -k --since "$T0" --no-pager -g Xid > "$N/host-xid.txt" 2>&1
journalctl -k --since "$T0" --no-pager -g 'NVRM|Xid' > "$N/host-nvrm.txt" 2>&1
{ date -Iseconds; grep MemAvailable /proc/meminfo; nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader; } > "$N/host-after-down.txt"
log "done"

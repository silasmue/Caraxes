#!/usr/bin/env bash
# run.sh <name> <override file> : one configuration of the latency matrix.
set -u
NAME=$1; OVR=$2
SP=$(cd "$(dirname "$0")" && pwd -P); ST=$SP/stage
R=/home/silas/git/Caraxes
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-latency/$NAME
W=/mnt/vmstore/caraxes/vm4-615; TR=$W/gpu-trace.log
mkdir -p "$N"; cp "$OVR" "$N/override.env"
log() { echo "$(date +%T) $*" | tee -a "$N/run.log"; }
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-120} tools/caraxes-vm.sh ssh "$1"); }
tripped() { [[ -f $N/GUARD-TRIPPED ]]; }
T0=$(date '+%F %T'); echo "$T0" > "$N/t0"
log "run $NAME override $(grep -v '^#' "$OVR" | tr '\n' ';')"
export SOTTR_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-poollock-304012cc SOTTR_SUN_ENCODER=nvenc CARAXES_MEM=8G SOTTR_NAT=0 SOTTR_OUT=$N/session LAT_OVERRIDE=$OVR
if [[ ${RESUME:-0} == 1 ]]; then rc=0; else (cd "$ST" && ./up-615.sh) > "$N/up-console.txt" 2>&1; rc=$?; fi
log "up rc=$rc"
setsid nohup bash "$SP/guard.sh" "$N" "$T0" > /dev/null 2>&1 < /dev/null &
grep -E '^vfio-user-nvgpu: (intr page|display page|msi re-arm|doorbell poll)' "$W/gpu.log" | cut -c1-240 > "$N/device-mode.txt"
if [[ $rc != 0 ]]; then log "up failed"; (cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1; exit 1; fi
grep -a -E 'Found H.264|Creating encoder|did not|never' "$N/session/sunshine-start.txt" | head -3 > "$N/sunshine-encoder.txt"
sleep 20
bash $SP/win.sh "$N" w1-idle-noclient idle
bash $SP/win.sh "$N" w2-cursor-noclient cursor
tripped && { log "guard tripped"; exit 2; }
STALL=0
if grep -q 'kicks_per_s=0.0' "$N/w2-cursor-noclient/kicks.txt"; then STALL=1; log "display stalled (0 kicks under the cursor test): no stream"; fi
ok=0
if (( ! STALL )); then
for a in 1 2 3; do
  GT=250 g "SUN_USER=lea SUN_PASS=leastream SUN_ENCODER=nvenc bash caraxes-sottr/sottr-session.sh sunshine-stop; SUN_USER=lea SUN_PASS=leastream SUN_ENCODER=nvenc bash caraxes-sottr/sottr-session.sh sunshine-start" > "$N/sunshine-restart-$a.txt" 2>&1
  L0=$(wc -l < "$N/session/moonlight.log" 2>/dev/null || echo 0)
  (cd "$ST" && ./stream-615.sh) > "$N/stream-console-$a.txt" 2>&1
  res=""
  for i in $(seq 60); do res=$(tail -n +$((L0 + 1)) "$N/session/moonlight.log" 2>/dev/null | grep -a -m1 -E 'Received first video packet|Connection terminated|No video traffic'); [[ -n $res ]] && break; sleep 2; done
  log "moonlight attempt $a: $(cut -c1-100 <<<"$res")"
  if grep -q 'Received first video packet' <<<"$res"; then ok=1; break; fi
  mp=$(cat "$W/moonlight.pid" 2>/dev/null); [[ -n $mp ]] && kill -0 "$mp" 2>/dev/null && [[ $(cat /proc/$mp/comm) == moonlight ]] && kill "$mp"; rm -f "$W/moonlight.pid"; sleep 5
done
(( ok )) || { log "no stream after 3 attempts"; }
sleep 15
bash $SP/win.sh "$N" w3-idle-streaming idle
bash $SP/win.sh "$N" w4-cursor-streaming cursor
bash $SP/win.sh "$N" w5-idle-streaming idle
bash $SP/win.sh "$N" w6-cursor-streaming cursor
tripped && { log "guard tripped"; exit 2; }
fi
g 'sudo dmesg' > "$N/guest-dmesg.txt" 2>&1
log "flip warnings: $(grep -c 'WARNING.*__nv_drm_handle_flip_event' "$N/guest-dmesg.txt")"
log "down"
(cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1
cp "$W/gpu.log" "$N/gpu.log"
grep -E '^(doorbell poll \(|intr page:|display page:|mirror:|bar0 config-mirror|display engine:|interrupts)' "$W/gpu.log" | cut -c1-2000 > "$N/device-summary.txt"
journalctl -k --since "$T0" --no-pager -g Xid > "$N/host-xid.txt" 2>&1
{ date -Iseconds; grep MemAvailable /proc/meminfo; nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader; } > "$N/host-after-down.txt"
log "done"

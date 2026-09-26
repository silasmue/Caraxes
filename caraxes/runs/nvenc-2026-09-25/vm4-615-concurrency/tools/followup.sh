#!/bin/bash
# followup.sh: second session: desktop ladders (F), Sunshine idle / streaming repeats (G).
C=/tmp/claude-1000/-home-silas-git-Caraxes/43784b5f-fc25-49bd-8e67-5dbab7e19aca/scratchpad/conc
R=/home/silas/git/Caraxes; ST=$R/caraxes/runs/sottr-2026-09-24; W=/mnt/vmstore/caraxes/vm4-615
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-concurrency; B=/mnt/vmstore/caraxes/linux-busy
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-400} tools/caraxes-vm.sh ssh "$@"); }
log() { echo "$(date +%T) $*" | tee -a "$N/run.log"; }
[[ -e /mnt/vmstore/caraxes/win-busy ]] && { echo "win-busy present"; exit 1; }
( set -o noclobber; echo "vm4-615-concurrency followup $$ $(date +%T)" > $B ) 2>/dev/null || { echo "linux-busy taken: $(cat $B)"; exit 1; }
trap 'rm -f $B' EXIT
mv $N/t0 $N/t0-session1; mv $N/session $N/session1; for f in up-console.txt guest-setup.txt host-baseline.txt; do mv $N/$f $N/session1-$f; done
log "== session 2 (follow-up)"
bash $C/setup.sh > /dev/null 2>&1; grep -q '^build rc=0' $N/guest-setup.txt || { log "setup failed"; }
tar -C $C -cf - ladder2.sh hold.sh | g 'tar -C ~/conc -xf -'
stop() { gp=$(cat $W/gpu.pid 2>/dev/null); [[ -n $gp ]] && ! kill -0 "$gp" 2>/dev/null && echo "device gone" > $N/STOP; [[ -f $N/STOP || -f $N/session/GUARD-TRIPPED ]]; }
cell() { stop && { log "STOP"; return 1; }; log "cell $*"; GS=${GS:-grun.sh} bash $C/cell.sh "$@" > /dev/null; sleep 5; }
if grep -q '^build rc=0' $N/guest-setup.txt; then
  g 'bash caraxes-sottr/sottr-session.sh sunshine-stop' > "$N/F-sunshine-stop.txt" 2>&1; sleep 10
  GS=ladder2.sh cell F1 vk ./hold.sh vkcube
  GS=ladder2.sh cell F2 cu ./hold.sh vkcube
  GS=ladder2.sh cell F3 vk ./hold.sh eglhigh
  GS=ladder2.sh cell F4 cu ./hold.sh eglhigh
  GS=ladder2.sh cell F5 vk ./hold.sh nvenc
  GS=ladder2.sh cell F6 vk ./hold.sh vkcube+nvenc
  GS=ladder2.sh cell F7 cu ./hold.sh vkcube+nvenc
  GT=250 g "SUN_USER=lea SUN_PASS=leastream SUN_ENCODER=nvenc bash caraxes-sottr/sottr-session.sh sunshine-start" > "$N/G-sunshine-start.txt" 2>&1
  log "sunshine idle: $(grep -E 'sunshine pid|Found H.264' "$N/G-sunshine-start.txt" | tr '\n' ' ' | cut -c1-160)"; sleep 60
  cell G1 vk:0:30; cell G2 cu:0:30; sleep 60; cell G3 vk:0:30
  OUT=$(cat "$W/sottr-session"); (cd "$ST" && ./stream-615.sh) > "$N/G-stream-console.txt" 2>&1
  for k in $(seq 60); do grep -qa 'Received first video packet' "$OUT/moonlight.log" && break; sleep 2; done
  log "stream: $(grep -a 'Received first video packet' "$OUT/moonlight.log" | tail -1 | cut -c1-90)"; sleep 10
  cell G4 vk:0:30; cell G5 cu:0:30
  mp=$(cat "$W/moonlight.pid" 2>/dev/null); [[ -n $mp ]] && kill -0 "$mp" 2>/dev/null && [[ $(cat /proc/$mp/comm) == moonlight ]] && kill "$mp"; rm -f "$W/moonlight.pid"
  log "moonlight pid $mp ended"; sleep 20
  cell G6 vk:0:30; cell G7 cu:0:30
fi
log "down (session 2)"
(cd $ST && ./down-615.sh) > $N/down2-console.txt 2>&1
T0=$(cat $N/t0); journalctl -k --since "$T0" --no-pager > $N/host-kernel-run2.txt 2>/dev/null
cp $W/gpu.log $N/gpu2.log 2>/dev/null; xz -f -T1 $N/gpu2.log
log "done (session 2); vm4-615 devices left: $(ps -eo args | grep -c '[v]m4-615/gpu')"

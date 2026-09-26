#!/bin/bash
# session3.sh: the desktop ladders F1-F7 again (session 2's holders never started: hold.sh not executable).
C=/tmp/claude-1000/-home-silas-git-Caraxes/43784b5f-fc25-49bd-8e67-5dbab7e19aca/scratchpad/conc
R=/home/silas/git/Caraxes; ST=$R/caraxes/runs/sottr-2026-09-24; W=/mnt/vmstore/caraxes/vm4-615
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-concurrency; B=/mnt/vmstore/caraxes/linux-busy
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-400} tools/caraxes-vm.sh ssh "$@"); }
log() { echo "$(date +%T) $*" | tee -a "$N/run.log"; }
[[ -e /mnt/vmstore/caraxes/win-busy ]] && { echo "win-busy present"; exit 1; }
( set -o noclobber; echo "vm4-615-concurrency session3 $$ $(date +%T)" > $B ) 2>/dev/null || { echo "linux-busy taken: $(cat $B)"; exit 1; }
trap 'rm -f $B' EXIT
for c in F1 F2 F3 F4 F5 F6 F7; do [[ -d $N/$c ]] && mv $N/$c $N/invalid-s2-$c; done
mv $N/t0 $N/t0-session2; mv $N/session $N/session2; for f in up-console.txt guest-setup.txt host-baseline.txt; do mv $N/$f $N/session2-$f; done
log "== session 3 (desktop ladders again)"
bash $C/setup.sh > /dev/null 2>&1
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
fi
log "down (session 3)"
(cd $ST && ./down-615.sh) > $N/down3-console.txt 2>&1
T0=$(cat $N/t0); journalctl -k --since "$T0" --no-pager > $N/host-kernel-run3.txt 2>/dev/null
cp $W/gpu.log $N/gpu3.log 2>/dev/null; xz -f -T1 $N/gpu3.log
log "done (session 3); vm4-615 devices left: $(ps -eo args | grep -c '[v]m4-615/gpu')"

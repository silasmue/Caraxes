#!/bin/bash
# matrix.sh [phases]: the concurrency matrix on the running vm4-615. Phases H D I S (default all).
set -u
C=/tmp/claude-1000/-home-silas-git-Caraxes/43784b5f-fc25-49bd-8e67-5dbab7e19aca/scratchpad/conc
R=/home/silas/git/Caraxes; ST=$R/caraxes/runs/sottr-2026-09-24; W=/mnt/vmstore/caraxes/vm4-615
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-concurrency
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-400} tools/caraxes-vm.sh ssh "$@"); }
log() { echo "$(date +%T) $*" | tee -a "$N/run.log"; }
stop() { gp=$(cat $W/gpu.pid 2>/dev/null); [[ -n $gp ]] && ! kill -0 "$gp" 2>/dev/null && echo "device pid $gp gone (host watch SIGTERM?)" > $N/STOP; [[ -f $N/STOP || -f $N/session/GUARD-TRIPPED ]] && { log "STOP: $(cat $N/STOP $N/session/GUARD-TRIPPED 2>/dev/null)"; exit 2; }; }
cell() { stop; log "cell $*"; GS=${GS:-grun.sh} bash $C/cell.sh "$@" > /dev/null; tail -n +1 "$N/table.txt" | grep -A5 " $1 |" | tail -6 >> /dev/null; stop; sleep 5; }
state() { g 'echo "sunshine: $(pgrep -x sunshine)"; echo "gnome-shell: $(pgrep -x gnome-shell)"; echo "gdm: $(systemctl is-active gdm)"; sudo fuser -v /dev/nvidia* 2>&1 | tail -n +2 | awk "{print \$NF}" | sort | uniq -c | tr "\n" " "; echo' 2>&1; }
five() { P=$1
  cell ${P}1 cu:0:60
  cell ${P}2 cu:0:60 cu:0:60 cu:0:60
  cell ${P}3 cu:0:90 vk:15:60
  cell ${P}4a vk:0:60
  cell ${P}4b vk:0:60 vk:0:60
  cell ${P}5 cu:0:100 vk:10:80 egl:20:60
  cell ${P}5e egl:0:60
}
for ph in ${*:-H D I S}; do
case $ph in
H)
  g 'bash caraxes-sottr/sottr-session.sh sunshine-stop' > "$N/H-sunshine-stop.txt" 2>&1
  g 'sudo systemctl stop gdm; sleep 8; pkill -u leandro -x steam; sleep 2' > "$N/H-gdm-stop.txt" 2>&1
  log "headless: $(state | tr '\n' ' ')"; state > "$N/H-state.txt"
  [[ ${NOLADDER:-0} == 1 ]] || {
    GS=ladder.sh cell C1 ./cuhold init 120
    GS=ladder.sh cell C2 ./cuhold ctx 120
    GS=ladder.sh cell C3 ./cuhold alloc 120
    GS=ladder.sh cell C4 env NVENC_HOLD_S=120 ./nvenc-hold -D -q -n 1
    GS=ladder.sh cell C5 ./nvenc-hold -D -q -n 100000 -r 60
  }
  five H ;;
D)
  GT=200 g 'sudo systemctl start gdm; sleep 5; bash caraxes-sottr/sottr-session.sh gnome 60' > "$N/D-gnome.txt" 2>&1
  sleep 30; log "desktop: $(state | tr '\n' ' ')"; state > "$N/D-state.txt"
  five D ;;
X)
  # the cxdbg agent's stall sampler (repro2.sh, rmioctl-log3), desktop up, Sunshine stopped
  X=/tmp/claude-1000/-home-silas-git-Caraxes/43784b5f-fc25-49bd-8e67-5dbab7e19aca/scratchpad/cxdbg
  tar -C $X -cf - rmioctl-log3.c repro2.sh | g 'mkdir -p ~/nvenc && tar -C ~/nvenc -xf - && cd ~/nvenc && gcc -O2 -shared -fPIC -o rmioctl-log3.so rmioctl-log3.c -ldl && ls nvenc-probe rmioctl-log3.so' > "$N/X-setup.txt" 2>&1
  s0=$(tail -c 8000 $W/gpu-trace.log | tail -1 | awk '{print $1}'); log "cxdbg repro2 start (trace $s0)"
  GT=500 g 'bash ~/nvenc/repro2.sh conc-X 3' > "$N/X-repro2.txt" 2>&1
  s1=$(tail -c 8000 $W/gpu-trace.log | tail -1 | awk '{print $1}')
  mkdir -p $C/X-out; g 'tar -C ~/cxrun -cf - conc-X' | tar -C $C/X-out -xf -
  awk -v a="$s0" -v b="$s1" '$1>=a && $1<=b' $W/gpu-trace.log | grep -v PROM | xz -T1 > $C/X-out/trace-$s0-$s1.txt.xz
  log "cxdbg repro2 done (trace $s0..$s1): $(tail -5 "$N/X-repro2.txt" | tr '\n' ' ' | cut -c1-200)" ;;
I)
  GT=250 g "SUN_USER=lea SUN_PASS=leastream SUN_ENCODER=nvenc bash caraxes-sottr/sottr-session.sh sunshine-start" > "$N/I-sunshine-start.txt" 2>&1
  log "sunshine idle: $(grep -E 'sunshine pid|Found H.264|did not|never' "$N/I-sunshine-start.txt" | tr '\n' ' ' | cut -c1-200)"
  state > "$N/I-state.txt"
  cell I1 cu:0:60; cell I2 vk:0:60; cell I3 egl:0:60 ;;
S)
  OUT=$(cat "$W/sottr-session")
  n0=$(grep -ac 'Received first video packet' "$OUT/moonlight.log" 2>/dev/null); n0=${n0:-0}
  (cd "$ST" && ./stream-615.sh) > "$N/S-stream-console.txt" 2>&1
  for k in $(seq 60); do (( $(grep -ac 'Received first video packet' "$OUT/moonlight.log" 2>/dev/null || echo 0) > n0 )) && break; sleep 2; done
  log "stream: $(grep -a 'Received first video packet' "$OUT/moonlight.log" | tail -1 | cut -c1-90)"
  sleep 10; state > "$N/S-state.txt"
  cell S1 cu:0:60; cell S2 vk:0:60; cell S3 egl:0:60
  g 'grep -a "Frame processing latency" ~/.config/sunshine/sunshine.log | tail -3; grep -aiE "fatal|hang detected" ~/.config/sunshine/sunshine.log | tail -3' > "$N/S-sunshine-tail.txt" 2>&1
  mp=$(cat "$W/moonlight.pid" 2>/dev/null); [[ -n $mp ]] && kill -0 "$mp" 2>/dev/null && [[ $(cat /proc/$mp/comm) == moonlight ]] && kill "$mp"; rm -f "$W/moonlight.pid"
  log "moonlight pid $mp ended" ;;
esac
done
log "matrix done: $*"

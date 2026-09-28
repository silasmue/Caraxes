#!/usr/bin/env bash
# cell2.sh <name> <binary> [pace]: cell.sh plus case (e) (E_ITERS ladder2 F6 iterations; PLAN.md on tokvas-2026-09-26); pace adds the
# display-pacing windows (w1 idle, w2 cursor without a client) for the combo binary.
set -u
NAME=$1; BIN=$2; PACE=${3:-}
SP=$(cd "$(dirname "$0")" && pwd -P); ST=$SP/stage
R=/home/silas/git/Caraxes
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-tokenfix/$NAME
W=/mnt/vmstore/caraxes/vm4-615; TR=$W/gpu-trace.log
mkdir -p "$N"
log() { echo "$(date +%T) $*" | tee -a "$N/run.log"; }
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-120} tools/caraxes-vm.sh ssh "$1"); }
tripped() { [[ -f $N/GUARD-TRIPPED ]]; }
now_tr() { tail -c 8000 "$TR" | tail -1 | awk '{print $1}'; }
hostn() { journalctl -k --since "$1" --no-pager 2>/dev/null | grep -E 'NVRM|Xid' ; }
SENV='U=$(id -u); wd=wayland-0; for c in /run/user/$U/wayland-*; do case $c in *.lock) continue;; esac; [ -S "$c" ] && { wd=${c##*/}; break; }; done; export XDG_RUNTIME_DIR=/run/user/$U WAYLAND_DISPLAY=$wd DISPLAY=:0 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$U/bus XDG_CURRENT_DESKTOP=GNOME;'
SUN='SUN_USER=lea SUN_PASS=leastream SUN_ENCODER=nvenc bash caraxes-sottr/sottr-session.sh'
# case window bookkeeping: cs <label> starts, ce <label> ends (trace slice of the doorbell/shadow lines, host NVRM lines)
cs() { CS_T=$(now_tr); CS_H=$(date '+%F %T'); }
ce() {
  local D=$N/$1 t1; mkdir -p "$D"; t1=$(now_tr)
  awk -v a="$CS_T" -v b="$t1" '$1>=a && $1<=b' "$TR" | grep -E 'doorbell|shadow|collision|chid|token|FREE|TSG' | cut -c1-400 | xz -T1 > "$D/trace-doorbell.txt.xz"
  echo "$CS_T $t1" > "$D/trace-window.txt"
  hostn "$CS_H" > "$D/host-nvrm.txt"
  echo "collisions_avoided=$(xzgrep -c 'collision avoided' "$D/trace-doorbell.txt.xz") host_nvrm_lines=$(grep -c NVRM "$D/host-nvrm.txt") va_alloc_failed=$(grep -c 'VA Space alloc failed' "$D/host-nvrm.txt") xid=$(grep -c Xid "$D/host-nvrm.txt")" > "$D/counts.txt"
}
T0=$(date '+%F %T'); echo "$T0" > "$N/t0"
log "cell $NAME binary $BIN $(md5sum "$BIN" | cut -c1-8) pace=${PACE:-no}"
export SOTTR_SERVER=$BIN SOTTR_SUN_ENCODER=nvenc CARAXES_MEM=8G SOTTR_NAT=0 SOTTR_OUT=$N/session LAT_OVERRIDE=$SP/cfg.env
(cd "$ST" && ./up-615.sh) > "$N/up-console.txt" 2>&1; rc=$?
log "up rc=$rc"
setsid nohup bash "$SP/guard.sh" "$N" "$T0" > /dev/null 2>&1 < /dev/null &
if [[ $rc != 0 ]]; then log "up failed"; (cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1; exit 1; fi
# guest tools: repro2.sh and rmioctl-log3.so in ~/nvenc (the overlay keeps them; rebuilt when missing)
g 'mkdir -p ~/nvenc; cat > ~/nvenc/repro2.sh' < "$SP/repro2.sh"
g '[ -f ~/nvenc/rmioctl-log3.so ] || { cat > /tmp/rl3.c; gcc -O2 -shared -fPIC -o ~/nvenc/rmioctl-log3.so /tmp/rl3.c -ldl; }; ls -la ~/nvenc/rmioctl-log3.so ~/nvenc/nvenc-probe ~/nvenc/repro2.sh' < "$SP/rmioctl-log3.c" > "$N/guest-tools.txt" 2>&1
log "guest tools: $(tr '\n' ' ' < "$N/guest-tools.txt" | cut -c1-300)"
sleep 20
if [[ -n $PACE ]]; then
  bash $SP/win.sh "$N" w1-idle-noclient idle
  bash $SP/win.sh "$N" w2-cursor-noclient cursor
fi
# (a) the collision repro, Sunshine stopped
GT=250 g "$SUN sunshine-stop" > "$N/sunshine-stop-a.txt" 2>&1
na=0; runs=0
for k in $(seq ${A_MAX:-8}); do
  (( na >= ${A_RUNS:-5} )) && break
  tries=5; (( k > 5 )) && tries=10
  L=a$k; cs
  GT=1000 g "bash ~/nvenc/repro2.sh tf-$NAME-$L $tries" > "$N/$L-console.txt" 2>&1
  ce $L
  g "cd ~/cxrun/tf-$NAME-$L && cat summary.txt; for f in vk-*.rmlog; do echo \"\$f 0x57=\$(grep -c status=0x57 \$f)\"; done" > "$N/$L/summary.txt" 2>&1
  g "tar -C ~/cxrun -cf - --exclude='*.rmlog' --exclude=kmsg.txt tf-$NAME-$L" 2>/dev/null | tar -C "$N/$L" -xf - 2>/dev/null
  c=$(grep -o 'collisions_avoided=[0-9]*' "$N/$L/counts.txt" | cut -d= -f2)
  rcs=$(grep -o 'vk [0-9]* rc=[0-9]*' "$N/$L/summary.txt" | tr '\n' ' ')
  n57=$(grep -o '0x57=[0-9]*' "$N/$L/summary.txt" | cut -d= -f2 | awk '{s+=$1} END{print s+0}')
  runs=$((runs+1)); (( c > 0 )) && na=$((na+1))
  log "(a) $L tries=$tries: $rcs| 0x57 total $n57 | $(cat "$N/$L/counts.txt")"
  tripped && { log "guard tripped"; exit 2; }
  [[ ${A_STOP_ON_FAIL:-0} == 1 && ( $n57 -gt 0 || $rcs == *rc=134* ) ]] && { log "(a) failure seen: baseline stop"; break; }
done
log "(a) counting runs (with a collision): $na of $runs"
# (e) multi-client teardown: ladder2.sh F6 shape (vkcomp probe before/during/after a vkcube+NVENC holder), Sunshine stopped
if (( ${E_ITERS:-0} > 0 )); then
  tar -C "$SP" -cf - ladder2.sh hold.sh | g 'mkdir -p ~/conc && tar -C ~/conc -xf -'
  g 'ls -la ~/conc/vkcomp ~/conc/nvenc-hold ~/conc/rmioctl-log.so ~/conc/ladder2.sh ~/conc/hold.sh' > "$N/guest-conc-tools.txt" 2>&1
  for k in $(seq $E_ITERS); do
    L=e$k; cs; l0=$(wc -l < $W/gpu.log)
    GT=300 g "bash ~/conc/ladder2.sh tf-$NAME-$L vk ./hold.sh vkcube+nvenc" > "$N/$L-guest.txt" 2>&1
    ce $L; l1=$(wc -l < $W/gpu.log)
    a=$(cat "$N/$L/trace-window.txt")
    awk -v a="${a% *}" -v b="${a#* }" '$1>=a && $1<=b' "$TR" | grep -E 'object refused|per-leaf VA object|VAS of guest root|kept under|held out of set A|released back to set A|DUP alias|registry: vaspace' | cut -c1-400 > "$N/$L/trace-va.txt"
    sed -n "$((l0+1)),${l1}p" $W/gpu.log | grep -E 'VAS of guest root|object refused|kept under|DUP alias' | cut -c1-600 > "$N/$L/gpulog-va.txt"
    res=$(grep -E '^RESULT' "$N/$L-guest.txt" | awk '{print $2}' | tr '
' ' ')
    mf=$(grep -o '[0-9]* map-failed' "$N/$L/gpulog-va.txt" | grep -vc '^0 ')
    log "(e) $L: probes $res| $(cat "$N/$L/counts.txt") object_refused=$(grep -c 'object refused' "$N/$L/trace-va.txt") per_leaf=$(grep -c 'per-leaf VA object' "$N/$L/trace-va.txt") kept=$(grep -c 'kept under' "$N/$L/trace-va.txt") held_out=$(grep -c 'held out of set A' "$N/$L/trace-va.txt") released_back=$(grep -c 'released back to set A' "$N/$L/trace-va.txt") vas_freed=$(grep -c 'VAS of guest root' "$N/$L/gpulog-va.txt") nonzero_map_failed=$mf"
    tripped && { log "guard tripped"; exit 2; }
    sleep 5
  done
fi
# (b) streaming: vkcube and glmark2, 4 times
ok=0
for a in 1 2 3; do
  GT=250 g "$SUN sunshine-stop; $SUN sunshine-start" > "$N/sunshine-restart-$a.txt" 2>&1
  L0=$(wc -l < "$N/session/moonlight.log" 2>/dev/null || echo 0)
  (cd "$ST" && ./stream-615.sh) > "$N/stream-console-$a.txt" 2>&1
  res=""
  for i in $(seq 60); do res=$(tail -n +$((L0 + 1)) "$N/session/moonlight.log" 2>/dev/null | grep -a -m1 -E 'Received first video packet|Connection terminated|No video traffic'); [[ -n $res ]] && break; sleep 2; done
  log "moonlight attempt $a: $(cut -c1-100 <<<"$res")"
  grep -q 'Received first video packet' <<<"$res" && { ok=1; break; }
  mp=$(cat "$N/session/moonlight.pid" 2>/dev/null); [[ -n $mp ]] && kill -0 "$mp" 2>/dev/null && [[ $(cat /proc/$mp/comm) == moonlight ]] && kill "$mp"; rm -f "$W/moonlight.pid"; sleep 5
done
(( ok )) || log "no stream after 3 attempts"
sleep 10
if [[ -n $PACE ]]; then
  bash $SP/win.sh "$N" w3-idle-streaming idle
  bash $SP/win.sh "$N" w4-cursor-streaming cursor
fi
for k in 1 2 3 4; do
  L=b$k; cs; mkdir -p "$N/$L"
  GT=200 g "$SENV"' s=$(date +%s.%N); LD_PRELOAD=~/nvenc/rmioctl-log3.so timeout 90 vkcube-wayland --c 300 > /tmp/vk.out 2> /tmp/vk.rmlog; rc=$?; e=$(date +%s.%N); echo "vkcube rc=$rc elapsed=$(echo "$e - $s" | bc) 0x57=$(grep -c status=0x57 /tmp/vk.rmlog)"; tail -2 /tmp/vk.out' > "$N/$L/vkcube.txt" 2>&1
  GT=200 g "$SENV"' timeout 90 glmark2-es2-wayland -b build 2>&1 | grep -E "FPS|Score"; echo "glrc=${PIPESTATUS[0]}"' > "$N/$L/glmark2.txt" 2>&1
  ce $L
  log "(b) $L: $(head -1 "$N/$L/vkcube.txt" | cut -c1-90) | $(tr '\n' ' ' < "$N/$L/glmark2.txt" | cut -c1-110) | $(cat "$N/$L/counts.txt")"
  sleep 15
done
# (c) a Vulkan application start while NVENC streams: vkmark for up to 150 s
L=c1; cs; mkdir -p "$N/$L"
GT=260 g "$SENV"' s=$(date +%s.%N); timeout 150 vkmark --winsys wayland > /tmp/vkmark.out 2>&1; rc=$?; e=$(date +%s.%N); echo "vkmark rc=$rc elapsed=$(echo "$e - $s" | bc)"; grep -vE "^\s*$" /tmp/vkmark.out | tail -20' > "$N/$L/vkmark.txt" 2>&1
g 'grep -a "Frame processing latency" ~/.config/sunshine/sunshine.log | tail -4; grep -aiE "fatal|hang detected" ~/.config/sunshine/sunshine.log | tail -2' > "$N/$L/sunshine.txt" 2>&1
ce $L
log "(c) $L: $(head -1 "$N/$L/vkmark.txt") score [$(grep -i 'vkmark score' "$N/$L/vkmark.txt")] | $(cat "$N/$L/counts.txt")"
if [[ -n $PACE ]]; then bash $SP/win.sh "$N" w6-cursor-streaming cursor; fi
g 'sudo dmesg' > "$N/guest-dmesg.txt" 2>&1
log "guest: flip warnings $(grep -c 'WARNING.*__nv_drm_handle_flip_event' "$N/guest-dmesg.txt"), TLB invalidation failed $(grep -c 'TLB invalidation failed' "$N/guest-dmesg.txt"), Xid $(grep -c 'Xid' "$N/guest-dmesg.txt")"
log "down"
(cd "$ST" && ./down-615.sh) > "$N/down-console.txt" 2>&1
cp "$W/gpu.log" "$N/gpu.log"
grep -E '^(doorbell routing|doorbell tokens|doorbell poll \(|bar2 memfd:|mirror:|bar0 config-mirror|soak|display engine:)' "$W/gpu.log" | cut -c1-2500 > "$N/device-summary.txt"
grep -iE 'tlb.*(fail|refus)|invalidat.*fail|ERROR freeing host TSG' "$W/gpu.log" | cut -c1-300 > "$N/device-failures.txt"
hostn "$T0" > "$N/host-nvrm.txt"
{ date -Iseconds; grep MemAvailable /proc/meminfo; nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader; } > "$N/host-after-down.txt"
log "done: host NVRM lines $(grep -c NVRM "$N/host-nvrm.txt"), VA alloc failed $(grep -c 'VA Space alloc failed' "$N/host-nvrm.txt"), Xid $(grep -c Xid "$N/host-nvrm.txt"); device failures $(wc -l < "$N/device-failures.txt")"

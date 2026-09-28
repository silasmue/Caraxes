#!/usr/bin/env bash
# win.sh <outdir> <label> idle|cursor: one 20 s window with kicks, Sunshine latency, guest and host CPU.
set -u
OUT=$1; L=$2; KIND=$3
R=/home/silas/git/Caraxes; W=/mnt/vmstore/caraxes/vm4-615; TR=$W/gpu-trace.log
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-120} tools/caraxes-vm.sh ssh "$1"); }
now_tr() { tail -c 8000 "$TR" | tail -1 | awk '{print $1}'; }
SENV='U=$(id -u); wd=wayland-0; for c in /run/user/$U/wayland-*; do case $c in *.lock) continue;; esac; [ -S "$c" ] && { wd=${c##*/}; break; }; done; export XDG_RUNTIME_DIR=/run/user/$U WAYLAND_DISPLAY=$wd DISPLAY=:0 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$U/bus XDG_CURRENT_DESKTOP=GNOME;'
D=$OUT/$L; mkdir -p "$D"
gs=$(g 'date +%s'); s0=$(now_tr); H0=$(date +%T)
if [[ $KIND == cursor ]]; then
  g "$SENV"' sudo timeout 40 python3 - <<"PY" > /tmp/cursor.log 2>&1; echo "rc=$?"; cat /tmp/cursor.log
import time, evdev
from evdev import UInput, ecodes as e
cap = {e.EV_REL: [e.REL_X, e.REL_Y], e.EV_KEY: [e.BTN_LEFT, e.BTN_RIGHT]}
ui = UInput(cap, name="caraxes-lat-mouse")
time.sleep(2)
t0 = time.time(); n = 0; d = 6
while time.time() - t0 < 20:
    if n % 100 == 0: d = -d
    ui.write(e.EV_REL, e.REL_X, d); ui.write(e.EV_REL, e.REL_Y, d // 2); ui.syn(); n += 1
    time.sleep(0.01)
print("moves", n, "in", round(time.time() - t0, 2), "s")
ui.close()
PY' > "$D/cursor.txt" 2>&1
  # the 2 s uinput settle is not part of the window
else
  sleep 20
fi
s1=$(now_tr); H1=$(date +%T); ge=$(g 'date +%s')
[[ $KIND == cursor ]] && s0=$(awk -v a="$s0" 'BEGIN{printf "%d", a+2e6}')
awk -v a="$s0" -v b="$s1" '$1>=a && $1<=b && / W bar0 0x00696000 /{n++} END{d=(b-a)/1e6; printf "window_s=%.1f kicks=%d kicks_per_s=%.1f\n", d, n, (d>0?n/d:0)}' "$TR" > "$D/kicks.txt"
awk -v a="$s0" -v b="$s1" '$1>=a && $1<=b' "$TR" | grep -v PROM | cut -c1-400 | xz -T2 > "$D/trace-window.txt.xz"
# guest and host CPU at the end of the window (the load of the window's kind is still on for idle/streaming)
g 'echo ==top; top -bn2 -d1 -o %CPU | awk "/^top -/{n++} n==2" | head -12; echo ==vmstat; vmstat 1 5' > "$D/guest.txt" 2>&1 &
gp=$(cat $W/gpu.pid); cp=$(cat $W/ch.pid)
{ echo "== device threads (pid $gp)"; top -bn2 -d1 -H -p "$gp" -o %CPU | awk '/^top -/{n++} n==2' | head -20
  echo "== cloud-hypervisor threads (pid $cp)"; top -bn2 -d1 -H -p "$cp" -o %CPU | awk '/^top -/{n++} n==2' | sed -n '7,20p'
  echo "== host"; grep MemAvailable /proc/meminfo; cat /proc/loadavg; } > "$D/host.txt" 2>&1
wait
# Sunshine's report covering the window: the first line stamped >= window start + 15 s (it reports every 20 s)
lat=""
for i in $(seq 12); do
  lat=$(g "grep -a 'Frame processing latency' ~/.config/sunshine/sunshine.log | while read -r a b rest; do t=\$(date -u -d \"\${a#[} \${b%]:}\" +%s 2>/dev/null); [ \"\${t:-0}\" -ge $((gs+15)) ] && { echo \"\$a \$b \$rest\"; break; }; done")
  [[ -n $lat ]] && break
  sleep 3
done
echo "$lat" > "$D/sunshine-lat.txt"
cpu=$(awk '/==vmstat/{f=1;next} f && $1 ~ /^[0-9]+$/ {n++; if(n>1){us+=$13; sy+=$14; st+=$17; in_+=$11; c++}} END{if(c) printf "us=%d sy=%d st=%d in=%d/s", us/c, sy/c, st/c, in_/c}' "$D/guest.txt")
dev=$(awk '/== device threads/{f=1;next} /== cloud/{f=0} f && $1 ~ /^[0-9]+$/ && $9+0>=2 {printf "%s:%s ", $12, $9; s+=$9} END{printf "sum:%s", s}' "$D/host.txt")
chv=$(awk '/== cloud-hypervisor/{f=1;next} /== host/{f=0} f && $12 ~ /vcpu/ {s+=$9} END{print s+0}' "$D/host.txt")
echo "$H0-$H1 $L $(grep -o 'moves [0-9]* in [0-9.]* s' "$D/cursor.txt" 2>/dev/null) | $(cat "$D/kicks.txt") | lat:$(sed 's/.*latency (min\/max\/avg)://' "$D/sunshine-lat.txt") | $cpu | dev: $dev | vcpu_sum:$chv" | tee -a "$OUT/table.txt"

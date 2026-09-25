#!/usr/bin/env bash
# m.sh <outdir> <cmd> <label> ...: measurements of the poollock night run.
#   step <label> [vk|novk|<secs>]   vkcube-wayland --c 120 (or a plain window), trace window, slow polls, guest+host CPU
#   bench <label>                   vulkaninfo --summary, vkmark, glmark2-es2-wayland (with the same trace window stats)
#   cursor <label>                  20 s of 100 Hz uinput pointer moves in the guest, with the same stats
set -u
OUT=$1; CMD=$2; L=$3; ARG=${4:-vk}
R=/home/silas/git/Caraxes
W=/mnt/vmstore/caraxes/vm4-615
TR=$W/gpu-trace.log
T0=$(cat "$OUT/t0")
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-200} tools/caraxes-vm.sh ssh "$1"); }
now_tr() { tail -c 8000 "$TR" | tail -1 | awk '{print $1}'; }
SENV='U=$(id -u); wd=wayland-0; for c in /run/user/$U/wayland-*; do case $c in *.lock) continue;; esac; [ -S "$c" ] && { wd=${c##*/}; break; }; done; export XDG_RUNTIME_DIR=/run/user/$U WAYLAND_DISPLAY=$wd DISPLAY=:0 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$U/bus XDG_CURRENT_DESKTOP=GNOME;'
D=$OUT/$L; mkdir -p "$D"
PREV=$(cat "$OUT/last_tr" 2>/dev/null || echo 0)
H0=$(date '+%T'); s0=$(now_tr)
# guest interrupts before
g 'cat /proc/interrupts' > "$D/irq0.txt" 2>/dev/null; i0=$(date +%s.%N)
extra=""
case $CMD in
step)
  if [[ $ARG == vk ]]; then
    g "$SENV"' s=$(date +%s.%N); timeout 60 vkcube-wayland --c 120 > /tmp/vk.log 2>&1; rc=$?; e=$(date +%s.%N); echo "rc=$rc elapsed=$(echo "$e - $s" | bc) wd=$wd"; tail -3 /tmp/vk.log' > "$D/vkcube.txt" 2>&1
  else
    sleep "${ARG/novk/20}"
  fi ;;
bench)
  GT=400 g "$SENV"' echo "== vulkaninfo"; env -u DISPLAY vulkaninfo --summary 2>&1 | grep -E "GPU[0-9]|deviceName|driverName|driverInfo|deviceType|apiVersion" ; echo "== vkmark"; timeout 150 vkmark --winsys wayland 2>&1 | grep -vE "^\s*$" | tail -30; echo "rc=${PIPESTATUS[0]}"; echo "== glmark2"; timeout 150 glmark2-es2-wayland -b build -b texture -b shading -b bump -b effect2d -b refract -b terrain 2>&1 | grep -vE "^\s*$" | tail -30; echo "rc=${PIPESTATUS[0]}"' > "$D/bench.txt" 2>&1
  extra="vkmark=$(grep -m1 -i 'vkmark score' "$D/bench.txt" | grep -o '[0-9]*$') glmark2=$(grep -m1 -i 'glmark2 score' "$D/bench.txt" | grep -o '[0-9]\+' | tail -1) vkdev=[$(grep -m1 -iE 'device name|Vendor' "$D/bench.txt" | cut -c1-60)] gldev=[$(grep -m1 GL_RENDERER "$D/bench.txt" | cut -c1-60)] vkinfo=[$(grep deviceName "$D/bench.txt" | awk -F= '{printf "%s;", $2}' | cut -c1-80)]" ;;
cursor)
  g "$SENV"' sudo timeout 40 python3 - <<"PY" > /tmp/cursor.log 2>&1; echo "rc=$?"; cat /tmp/cursor.log
import time, evdev
from evdev import UInput, ecodes as e
cap = {e.EV_REL: [e.REL_X, e.REL_Y], e.EV_KEY: [e.BTN_LEFT, e.BTN_RIGHT]}
ui = UInput(cap, name="caraxes-night-mouse")
time.sleep(2)
t0 = time.time(); n = 0; d = 6
while time.time() - t0 < 20:
    if n % 100 == 0: d = -d
    ui.write(e.EV_REL, e.REL_X, d); ui.write(e.EV_REL, e.REL_Y, d // 2); ui.syn(); n += 1
    time.sleep(0.01)
print("moves", n, "in", round(time.time() - t0, 2), "s")
ui.close()
PY' > "$D/cursor.txt" 2>&1
  extra="moves=[$(grep -m1 moves "$D/cursor.txt")]" ;;
esac
i1=$(date +%s.%N); g 'cat /proc/interrupts' > "$D/irq1.txt" 2>/dev/null
s1=$(now_tr); H1=$(date '+%T')
python3 - "$D/irq0.txt" "$D/irq1.txt" "$i0" "$i1" > "$D/irq-rate.txt" <<'PY'
import sys
def rd(f):
    r={}
    for l in open(f):
        p=l.split()
        if not p or not p[0].endswith(':'): continue
        k=p[0][:-1]; n=0; name=[]
        for x in p[1:]:
            if x.isdigit() and not name: n+=int(x)
            else: name.append(x)
        r[k]=(n,' '.join(name[-2:]))
    return r
a,b=rd(sys.argv[1]),rd(sys.argv[2]); dt=float(sys.argv[4])-float(sys.argv[3])
rows=sorted(((b[k][0]-a[k][0])/dt,k,b[k][1]) for k in b if k in a)
for r,k,n in reversed(rows[-10:]): print(f"{k:>6} {r:9.1f}/s {n}")
PY
rc=$(grep -o 'rc=[0-9]*' "$D/vkcube.txt" 2>/dev/null | head -1); el=$(grep -o 'elapsed=[0-9.]*' "$D/vkcube.txt" 2>/dev/null | head -1 | cut -d= -f2)
awk -v a="$s0" -v b="$s1" '$1>=a && $1<=b && / W bar0 0x00696000 /{n++; if ($NF!="0x0") nz++} END{d=(b-a)/1e6; printf "window_s=%.1f kicks=%d kicks_per_s=%.1f nonzero_per_s=%.1f\n", d, n, (d>0?n/d:0), (d>0?nz/d:0)}' "$TR" > "$D/kicks.txt"
awk -v a="$s0" -v b="$s1" '$1>=a && $1<=b && tolower($0) ~ /cursor/' "$TR" | cut -c1-300 > "$D/cursor-trace-lines.txt"
awk -v a="$PREV" -v b="$s1" '$1>=a && $1<=b && /held the device/' "$TR" | cut -c1-800 > "$D/slowpolls.txt"
awk -v a="$s0" -v b="$s1" '$1>=a && $1<=b && /held the device/' "$TR" | wc -l > "$D/slowpolls-window.txt"
awk -v a="$s0" -v b="$s1" '$1>=a && $1<=b && $1<=a+45e6' "$TR" | grep -v PROM | cut -c1-400 | xz -T2 > "$D/trace-window.txt.xz"
echo "$s1" > "$OUT/last_tr"
python3 - "$D/slowpolls.txt" "$PREV" "$s1" > "$D/slowpoll-summary.txt" <<'PY'
import re,sys,collections
f,a,b=sys.argv[1],float(sys.argv[2]),float(sys.argv[3])
parts=['sync','walk','decide','userd','refs','barrier','moves','resync']
tot=collections.Counter(); reg=collections.Counter(); n=0; held=[]; homes=0; tpr=0
for l in open(f):
    n+=1
    m=re.search(r'at (0x[0-9a-f]+) held the device (\d+) us',l)
    if m: reg[m.group(1)]+=1; held.append(int(m.group(2)))
    for p in parts:
        mm=re.search(r'\b%s (\d+) us'%p,l)
        if mm: tot[p]+=int(mm.group(1))
    mm=re.search(r'(\d+) home\(s\) read',l); homes+=int(mm.group(1)) if mm else 0
    mm=re.search(r'(\d+) table page\(s\) read',l); tpr+=int(mm.group(1)) if mm else 0
mins=(b-a)/60e6 if a>0 else 0
held.sort()
dom=tot.most_common(1)[0][0] if tot else '-'
print(f"slow={n} per_min={(n/mins if mins else 0):.1f} dominant={dom}")
if held: print(f"held_us p50={held[len(held)//2]} max={held[-1]} sum_ms={sum(held)/1000:.0f} homes_read={homes} table_pages_read={tpr}")
print("parts_ms", {k:round(v/1000,1) for k,v in tot.most_common()})
print("registers", dict(reg.most_common(6)))
PY
g 'echo ==lat; grep -a "Frame processing latency" ~/.config/sunshine/sunshine.log | tail -4; echo ==top; top -bn2 -d1 -o %CPU | awk "/^top -/{n++} n==2" | head -15; echo ==vmstat; vmstat 1 5; echo ==D; ps -eo pid,stat,wchan:24,comm | awk "\$2 ~ /D/"; echo ==smi; nvidia-smi --query-compute-apps=pid,name,used_memory --format=csv,noheader; nvidia-smi --query-gpu=memory.used --format=csv,noheader' > "$D/guest.txt" 2>&1
xid=$(journalctl -k --since "$T0" --no-pager -g Xid 2>/dev/null | grep -vc '^-- ')
gp=$(cat $W/gpu.pid 2>/dev/null); cp=$(cat $W/ch.pid 2>/dev/null)
{ echo "== host"; nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader; grep MemAvailable /proc/meminfo; cat /proc/loadavg; nvidia-smi -q -d MEMORY | sed -n '/BAR1/,/Free/p'
  echo "== device threads (pid $gp)"; top -bn2 -d1 -H -p "$gp" -o %CPU | awk '/^top -/{n++} n==2' | sed -n '7,22p'
  echo "== cloud-hypervisor threads (pid $cp)"; top -bn2 -d1 -H -p "$cp" -o %CPU | awk '/^top -/{n++} n==2' | sed -n '7,20p'; } > "$D/host.txt" 2>&1
lat=$(awk '/==lat/{f=1;next} /==top/{f=0} f' "$D/guest.txt" | tail -1 | sed 's/.*latency//' | cut -c1-80)
cpu=$(awk '/==vmstat/{f=1;next} /==D/{f=0} f && $1 ~ /^[0-9]+$/ {n++; if(n>1){us+=$13; sy+=$14; st+=$17; in_+=$11; c++}} END{if(c) printf "us=%d sy=%d st=%d in=%d/s", us/c, sy/c, st/c, in_/c}' "$D/guest.txt")
dst=$(awk '/==D/{f=1;next} /==smi/{f=0} f {printf "%s ", $4}' "$D/guest.txt")
dev=$(awk '/== device threads/{f=1;next} /== cloud/{f=0} f && $9+0>=5 {printf "%s:%s ", $12, $9}' "$D/host.txt")
chv=$(awk '/== cloud-hypervisor/{f=1;next} f && $9+0>=5 {printf "%s:%s ", $12, $9}' "$D/host.txt")
la=$(awk '/^[0-9.]+ [0-9.]+ [0-9.]+ /{print $1; exit}' "$D/host.txt")
echo "$H0-$H1 $L ${rc:-} el=${el:-} fps=$( [[ -n ${el:-} ]] && echo "scale=1; 120/$el" | bc) $extra | $(cat "$D/kicks.txt") | win_slow=$(cat "$D/slowpolls-window.txt") $(head -1 "$D/slowpoll-summary.txt") | lat:$lat | xid=$xid | $cpu | D: $dst | host dev: $dev ch: $chv load=$la" | tee -a "$OUT/table.txt"

#!/usr/bin/env bash
# One Moonlight stream against Sunshine on Linux B with the given encoder (nvenc | software),
# vkcube (X11, 60 fps) as the load. Measures over the stream: the guest's Sunshine CPU
# (/proc stat delta), guest total CPU (/proc/stat), host CPU of Cloud Hypervisor and the device,
# Moonlight's fps and latency. Usage: stream.sh OUTDIR ENCODER SECONDS
set -uo pipefail
OUT=$1 ENC=$2 SECS=${3:-150}
W=$(cd "$(dirname "$(readlink -f "$0")")/../../night-2026-09-28" && pwd -P)/desktop-b-night.sh
L=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b
mkdir -p "$OUT"
g() { timeout ${T:-40} "$W" ssh "$@" 2>&1 | grep -v "unable to resolve"; }
hz=$(getconf CLK_TCK)
ticks() { awk '{print $14+$15}' /proc/$1/stat 2>/dev/null || echo 0; }
CH=$(cat $L/ch.pid) DEV=$(cat $L/gpu.pid)
g "f=~/.config/sunshine/sunshine.conf; touch \$f; if grep -q '^encoder' \$f; then sed -i 's/^encoder.*/encoder = $ENC/' \$f; else echo 'encoder = $ENC' >> \$f; fi; cat \$f; systemctl --user restart app-dev.lizardbyte.app.Sunshine.service; sleep 3; systemctl --user is-active app-dev.lizardbyte.app.Sunshine.service" > "$OUT/sunshine-conf.txt"
cat "$OUT/sunshine-conf.txt"
sleep ${ML_SETTLE:-20}
g 'export DISPLAY=:0 XAUTHORITY=$(ls -t /run/user/1000/.mutter-Xwaylandauth.* | head -1); (setsid timeout '$((SECS + 40))' vkcube > /dev/null 2>&1 &)' > /dev/null
sleep 3
sticks='p=$(pgrep -x sunshine | head -1); [ -n "$p" ] && awk "{print \$14+\$15}" /proc/$p/stat || echo 0; head -1 /proc/stat'
t0=$(date +%s.%N); c0=$(ticks $CH); d0=$(ticks $DEV); gs0=$(g "$sticks")
setsid nohup env WAYLAND_DISPLAY=wayland-1 DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 LANG=C.UTF-8 /usr/bin/moonlight stream 192.168.106.26 Desktop --resolution 1920x1080 --fps 60 --bitrate 20000 --display-mode windowed --absolute-mouse --video-decoder auto > "$OUT/moonlight.log" 2>&1 < /dev/null &
MP=$!; echo "moonlight pid $MP at $(date +%T)" | tee "$OUT/stream.txt"
end=$((SECONDS + SECS))
while (( SECONDS < end )); do
    kill -0 $MP 2>/dev/null || { echo "moonlight exited at $(date +%T)" | tee -a "$OUT/stream.txt"; break; }
    grep -qE "No video received|Connection terminated" "$OUT/moonlight.log" && { echo "moonlight: $(grep -m1 -oE 'No video received.*|Connection terminated.*' "$OUT/moonlight.log")" | tee -a "$OUT/stream.txt"; break; }
    sleep 5
done
t1=$(date +%s.%N); c1=$(ticks $CH); d1=$(ticks $DEV); gs1=$(g "$sticks")
kill $MP 2>/dev/null; sleep 3
python3 - "$t0" "$t1" "$c0" "$c1" "$d0" "$d1" "$hz" "$gs0" "$gs1" >> "$OUT/stream.txt" <<'EOF'
import sys
t0, t1, c0, c1, d0, d1, hz = map(float, sys.argv[1:8])
a, b = sys.argv[8].split('\n'), sys.argv[9].split('\n')
w = t1 - t0
s0, s1 = float(a[0] or 0), float(b[0] or 0)
cpu0 = [int(x) for x in a[1].split()[1:]]; cpu1 = [int(x) for x in b[1].split()[1:]]
tot = sum(cpu1) - sum(cpu0); idle = (cpu1[3] + cpu1[4]) - (cpu0[3] + cpu0[4])
print(f"window {w:.1f} s: host CH {100*(c1-c0)/hz/w:.1f} % of a core, device {100*(d1-d0)/hz/w:.1f} %, guest sunshine {100*(s1-s0)/hz/w:.1f} % of a core, guest total {100*(tot-idle)/max(tot,1):.1f} % (8 vCPUs)")
EOF
grep -E "Received first video packet|Video stream is|frame rate|Host processing latency|Average network latency|Frames dropped" "$OUT/moonlight.log" | tail -10 >> "$OUT/stream.txt"
g 'journalctl --user -u app-dev.lizardbyte.app.Sunshine.service --since "-5min" --no-pager 2>/dev/null | tail -300' > "$OUT/sunshine-tail.log"
grep -iE "encoder|nvenc|x264|Error|Fatal|CLIENT|Found (H.264|HEVC)|capture" "$OUT/sunshine-tail.log" | tail -25 | cut -c1-200 >> "$OUT/stream.txt"
echo "host Xid lines for $DEV: $(sudo -n dmesg | grep -c "pid=$DEV,")" >> "$OUT/stream.txt"
cat "$OUT/stream.txt"

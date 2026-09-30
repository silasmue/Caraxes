#!/usr/bin/env bash
# One Moonlight stream against Sunshine on the Windows guest with the given encoder, with a
# D3D11 present load (present-probe, 60 fps windowed) during it. Measures over the stream:
# the guest's Sunshine CPU (TotalProcessorTime delta), the guest's total CPU (WMI), the host
# CPU of Cloud Hypervisor and the device (/proc stat deltas), Moonlight's fps and latency.
# Usage: stream.sh OUTDIR ENCODER(nvenc|software) SECONDS
set -uo pipefail
OUT=$1 ENC=$2 SECS=${3:-150}
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain
cd $K || exit 1
mkdir -p "$OUT"
s() { timeout ${T:-40} python3 CH-build/run44.py ssh "$@" 2>/dev/null | tr -d '\r'; }
hz=$(getconf CLK_TCK)
ticks() { awk '{print $14+$15}' /proc/$1/stat 2>/dev/null || echo 0; }
CH=$(cat $K/ch.pid) DEV=$(cat $K/gpu.pid)
# the encoder, then a service restart
T=90 s "\$c='C:\Program Files\Sunshine\config\sunshine.conf'; \$t = @(Get-Content \$c); if (\$t -match '^encoder\s*=') { \$t = \$t -replace '^encoder\s*=.*','encoder = $ENC' } else { \$t += 'encoder = $ENC' }; Set-Content -Encoding ascii \$c \$t; Select-String -Path \$c -Pattern '^(encoder|min_threads)'; Restart-Service -Force SunshineService; (Get-Service SunshineService).Status" > "$OUT/sunshine-conf.txt"
cat "$OUT/sunshine-conf.txt"
for i in $(seq 1 30); do s '(Get-Service SunshineService).Status' | grep -q Running && break; sleep 5; done
sleep ${ML_SETTLE:-40}
if ! s 'Get-Process sunshine -ErrorAction SilentlyContinue | Measure-Object | ForEach-Object Count' | grep -q '^[1-9]'; then
    echo "no sunshine.exe (no user session?): stream skipped" | tee "$OUT/stream.txt"; exit 0
fi
# the present load in the signed-in session
# the codec branch's present-probe (052f8611: the kit's copy does not compile)
timeout 150 bash "$(dirname "$(readlink -f "$0")")/upack.sh" start /home/silas/git/Caraxes/tools/windows/present-probe.ps1 codec-present $((SECS + 150)) -Seconds $((SECS + 60)) > "$OUT/present-start.txt" 2>&1
sleep 5
setsid nohup env WAYLAND_DISPLAY=wayland-1 DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 LANG=C.UTF-8 /usr/bin/moonlight stream 192.168.107.2 Desktop --resolution 1920x1080 --fps 60 --bitrate 20000 --display-mode windowed --absolute-mouse --video-decoder auto > "$OUT/moonlight.log" 2>&1 < /dev/null &
MP=$!; echo "moonlight pid $MP at $(date +%T)" | tee "$OUT/stream.txt"
# The measurement window starts 15 s after the first video (the encoder's creation is not in it).
for i in $(seq 1 30); do grep -q "Received first video packet" "$OUT/moonlight.log" && break; sleep 1; done
grep -o "Received first video packet after [0-9]* ms" "$OUT/moonlight.log" | tee -a "$OUT/stream.txt"
sleep 15
kill -USR2 $DEV 2>/dev/null
t0=$(date +%s.%N); c0=$(ticks $CH); d0=$(ticks $DEV)
g0=$(s '$p = Get-Process sunshine -ErrorAction SilentlyContinue | Select-Object -First 1; if ($p) { $p.TotalProcessorTime.TotalSeconds } else { 0 }' | tr ',' '.' | tail -1)
# guest CPU samples during the window (1 s apart)
T=$((SECS + 30)) s "for (\$i = 0; \$i -lt $((SECS / 3)); \$i++) { \$p = Get-CimInstance -ClassName Win32_PerfFormattedData_PerfOS_Processor | Where-Object Name -eq '_Total'; '{0:HH:mm:ss} total {1} priv {2}' -f (Get-Date), \$p.PercentProcessorTime, \$p.PercentPrivilegedTime; Start-Sleep 1 }" > "$OUT/guest-cpu.txt" &
SP=$!
end=$((SECONDS + SECS))
while (( SECONDS < end )); do
    kill -0 $MP 2>/dev/null || { echo "moonlight exited at $(date +%T)" | tee -a "$OUT/stream.txt"; break; }
    grep -qE "No video received|Connection terminated" "$OUT/moonlight.log" && { echo "moonlight: $(grep -m1 -oE 'No video received.*|Connection terminated.*' "$OUT/moonlight.log")" | tee -a "$OUT/stream.txt"; break; }
    sleep 5
done
t1=$(date +%s.%N); c1=$(ticks $CH); d1=$(ticks $DEV)
kill -USR2 $DEV 2>/dev/null
g1=$(s '$p = Get-Process sunshine -ErrorAction SilentlyContinue | Select-Object -First 1; if ($p) { $p.TotalProcessorTime.TotalSeconds } else { 0 }' | tr ',' '.' | tail -1)
kill $MP 2>/dev/null; sleep 3
echo "raw: t $t0 $t1 ch $c0 $c1 dev $d0 $d1 sunshine $g0 $g1 hz $hz" >> "$OUT/stream.txt"
timeout 90 bash "$(dirname "$(readlink -f "$0")")/upack.sh" status codec-present 2>&1 | tail -12 > "$OUT/present-status.txt"
wait $SP 2>/dev/null
sleep 2; mv $K/gpu-profile.[0-9]*.tsv "$OUT/" 2>/dev/null
python3 - "$t0" "$t1" "$c0" "$c1" "$d0" "$d1" "$g0" "$g1" "$hz" >> "$OUT/stream.txt" <<'EOF'
import sys
t0, t1, c0, c1, d0, d1, g0, g1, hz = map(float, sys.argv[1:])
w = t1 - t0
print(f"window {w:.1f} s: host CH {100*(c1-c0)/hz/w:.1f} % of a core, device {100*(d1-d0)/hz/w:.1f} %, guest sunshine {100*(g1-g0)/w:.1f} %")
EOF
awk '{print $3}' "$OUT/guest-cpu.txt" | python3 -c 'import sys; v=[float(x) for x in sys.stdin if x.strip()]; print(f"guest total CPU (8 vCPUs) mean {sum(v)/max(len(v),1):.1f} % over {len(v)} samples")' >> "$OUT/stream.txt"
grep -E "Received first video packet|Video stream is|IDR frame|frame rate|Host processing latency|Average network latency|Frames dropped|decoder|codec|HEVC|H.264|AV1" "$OUT/moonlight.log" | grep -v "SDL Info (0): Skipping" | tail -14 >> "$OUT/stream.txt"
T=40 s 'Get-Content -Tail 4000 "C:\Program Files\Sunshine\config\sunshine.log"' > "$OUT/sunshine-tail.log"
grep -iE "encoder|nvenc|x264|Error|Fatal|CLIENT|timeout|Start capturing|Found (H.264|HEVC)" "$OUT/sunshine-tail.log" | tail -30 | cut -c1-200 >> "$OUT/stream.txt"
echo "host Xid lines for $DEV: $(sudo -n dmesg | grep -c "pid=$DEV,")" >> "$OUT/stream.txt"
cat "$OUT/stream.txt"

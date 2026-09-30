#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Trap census session on Linux B's dmem copy (desktop-b-dmem, 615, stock guest kernel), 2026-09-30.
# Runs inside `vmlock.sh desktop-b-dmem ...`; desktop-b-dmem shares Linux B's IP, tap and VRAM
# slot, so the caller also holds guest-desktop-b.lock (census-run.sh). Always takes the VM down.
# Workloads, each between two SIGUSR2 profile dumps (CARAXES_PROFILE=1) and /proc thread CPU
# snapshots of Cloud Hypervisor and the device:
#   trap RTT (bench-trap reads/writes before the driver loads), idle desktop, glmark2, vkmark,
#   a Moonlight stream (vkcube as the load), a CUDA loop (~/conc/cuwork), idle again.
# Usage: census-linux.sh OUT BINARY
set -uo pipefail
OUT=$1 BIN=$2
H=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
W=$(cd "$(dirname "$(readlink -f "$0")")/../dmem-2026-09-29" && pwd -P)/desktop-b-dmem.sh
G=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b-dmem
TOOLS=$(cd "$H/../../tools" && pwd -P)
IDLE_S=${IDLE_S:-60} BENCH_S=${BENCH_S:-30} STREAM_S=${STREAM_S:-60} CUDA_S=${CUDA_S:-40}
mkdir -p "$OUT"
log() { echo "$(date +%T) $*" | tee -a "$OUT/timeline.txt"; }
g() { timeout "${T:-60}" "$W" ssh "$@" 2>&1 | grep -v "unable to resolve"; }
DEV="" CH="" n=0
mark() {
    n=$((n + 1))
    kill -USR2 "$DEV" 2>/dev/null
    python3 "$H/taskstat.py" snap "$CH" "$OUT/ch.$n.json"
    python3 "$H/taskstat.py" snap "$DEV" "$OUT/dev.$n.json"
    echo "$n $(date +%s.%N) $*" >> "$OUT/marks.txt"
    log "mark $n: $*"
}
down() {
    log "down"
    timeout 240 "$W" down > "$OUT/down.txt" 2>&1
    cp "$G/gpu.log" "$OUT/gpu.log" 2>/dev/null
    if [[ -n $DEV ]]; then echo "host Xid lines for device pid $DEV: $(sudo -n dmesg | grep -c "pid=$DEV,")" >> "$OUT/timeline.txt"; fi
    for f in "$OUT"/profile.*.tsv; do [[ -f $f ]] && xz -T2 -f "$f"; done
}
trap down EXIT

log "up $BIN"
DMEM_KERNEL=stock NIGHT_SERVER=$BIN CARAXES_PROFILE=1 CARAXES_PROFILE_OUT="$OUT/profile" CARAXES_NO_TRACE=1 \
    timeout 400 "$W" up > "$OUT/up.txt" 2>&1 || { log "up failed"; tail -5 "$OUT/up.txt"; exit 1; }
timeout 240 "$W" wait 200 >> "$OUT/up.txt" 2>&1
DEV=$(cat "$G/gpu.pid") CH=$(cat "$G/ch.pid")
log "vm up: device $DEV, ch $CH"
tr '\0' '\n' < "/proc/$DEV/environ" | grep '^CARAXES_' | sort > "$OUT/device-env.txt"

# Trap round trip before the driver loads: 20000 reads each of BOOT_0 (memory), SW_SCRATCH(0)
# (trapped), PTIMER (lock-free) and the usermode TIME; 20000 writes of SW_SCRATCH(0)'s own value.
g 'mkdir -p ~/census && cat > ~/census/bench-trap.c' < "$TOOLS/bench-trap.c"
mark "rtt start"
T=240 g 'bash -s' > "$OUT/rtt.txt" <<'GS'
cd ~/census && gcc -O2 -o bench-trap bench-trap.c 2>&1 | tail -2
R=$(for d in /sys/bus/pci/devices/*; do [ "$(cat $d/vendor)" = 0x10de ] && [ "$(cat $d/class | cut -c1-4)" = 0x03 ] && echo $d/resource0 && break; done)
echo "resource $R"
V=$(sudo python3 -c "import mmap,os,struct;f=os.open('$R',os.O_RDWR|os.O_SYNC);m=mmap.mmap(f,0x2000,offset=0);print(hex(struct.unpack_from('<I',m,0x1400)[0]))")
echo "SW_SCRATCH(0) = $V"
echo "--- reads BOOT_0 (BAR0 page 0: the interrupt page's memory, no trap)"; sudo ./bench-trap $R 20000 none 0x0 2>&1 | tail -4
echo "--- reads SW_SCRATCH(0) 0x1400 (trapped, model under the device lock)"; sudo ./bench-trap $R 20000 none 0x1400 2>&1 | tail -4
echo "--- reads PTIMER_TIME_0 (lock-free fast read)"; sudo ./bench-trap $R 20000 none 0x9400 2>&1 | tail -4
echo "--- reads usermode TIME_0"; sudo ./bench-trap $R 20000 none 0xbb0080 2>&1 | tail -4
echo "--- writes SW_SCRATCH(0) = $V"; sudo ./bench-trap $R 20000 0x1400 0x0 $V 2>&1 | tail -4
GS
cat "$OUT/rtt.txt"
mark "rtt end"

# The desktop and Sunshine, as the SotTR bench brings them up.
T=240 g 'bash -s' > "$OUT/guest-prep.txt" <<'GS'
sudo ip route replace default via 192.168.106.1 dev eth0 && sudo resolvectl dns eth0 192.168.188.1 && sudo resolvectl domain eth0 "~."
sudo modprobe nvidia && sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm modeset=1 && echo modules-ok
bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1; sudo systemctl start gdm3; sleep 12
export XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
for i in $(seq 1 30); do ls /run/user/1000/wayland-* >/dev/null 2>&1 && ls /run/user/1000/.mutter-Xwaylandauth.* >/dev/null 2>&1 && break; sleep 1; done
echo "desktop: gnome-shell=$(pgrep -c gnome-shell) Xwayland=$(pgrep -c Xwayland) display=$(cat /sys/class/drm/card*-DP-1/status 2>/dev/null | head -1) $(cat /sys/class/drm/card*-DP-1/modes 2>/dev/null | head -1)"
systemctl --user stop caraxes-dmem-cap.service 2>/dev/null
systemctl --user start app-dev.lizardbyte.app.Sunshine.service; sleep 6
echo "sunshine: $(systemctl --user is-active app-dev.lizardbyte.app.Sunshine.service) encoder: $(grep '^encoder' ~/.config/sunshine/sunshine.conf 2>/dev/null)"
nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv,noheader
ls ~/conc/ 2>/dev/null | tr '\n' ' '; echo
GS
cat "$OUT/guest-prep.txt"
sleep 20

mark "idle desktop start"
sleep "$IDLE_S"
mark "idle desktop end / glmark2 start"
T=$((BENCH_S * 3 + 60)) g "export XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=\$(cd /run/user/1000 && ls wayland-? | head -1); echo wayland=\$WAYLAND_DISPLAY; timeout $((BENCH_S * 3 + 30)) glmark2-wayland -s 1280x720 -b build:duration=$BENCH_S -b texture:duration=$BENCH_S -b shading:duration=$BENCH_S 2>&1 | grep -E 'FPS|Score|rror'" > "$OUT/glmark2.txt"
cat "$OUT/glmark2.txt"
mark "glmark2 end / vkmark start"
T=$((BENCH_S * 3 + 60)) g "export XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=\$(cd /run/user/1000 && ls wayland-? | head -1); timeout $((BENCH_S * 3 + 30)) vkmark --winsys wayland -s 1280x720 -b vertex:duration=$BENCH_S -b texture:duration=$BENCH_S -b shading:duration=$BENCH_S 2>&1 | grep -E '^\[|Score|rror'" > "$OUT/vkmark.txt"
cat "$OUT/vkmark.txt"
mark "vkmark end"

# The stream: vkcube on X11 as the load, Moonlight on the host.
g 'export DISPLAY=:0 XAUTHORITY=$(ls -t /run/user/1000/.mutter-Xwaylandauth.* | head -1); (setsid timeout '$((STREAM_S + 60))' vkcube > /dev/null 2>&1 & echo $! > /tmp/census-vkcube.pid)' > /dev/null
sleep 3
setsid nohup env WAYLAND_DISPLAY=wayland-1 DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 LANG=C.UTF-8 /usr/bin/moonlight stream 192.168.106.26 Desktop --resolution 1920x1080 --fps 60 --bitrate 20000 --display-mode windowed --absolute-mouse --video-decoder auto > "$OUT/moonlight.log" 2>&1 < /dev/null &
MP=$!
for i in $(seq 1 30); do grep -q "Received first video packet" "$OUT/moonlight.log" 2>/dev/null && break; kill -0 $MP 2>/dev/null || break; sleep 1; done
log "moonlight pid $MP first video: $(grep -c 'Received first video packet' "$OUT/moonlight.log")"
sleep 5
mark "stream start"
sleep "$STREAM_S"
mark "stream end"
kill "$MP" 2>/dev/null; sleep 3
grep -E "Received first video packet|Video stream is|frame rate|Host processing latency|Average network latency|Frames dropped" "$OUT/moonlight.log" | tail -8 >> "$OUT/timeline.txt"
g 'kill $(cat /tmp/census-vkcube.pid) 2>/dev/null' > /dev/null 2>&1
sleep 5

mark "cuda start"
T=$((CUDA_S + 60)) g "ls ~/conc/cuwork && timeout $CUDA_S ~/conc/cuwork 2>&1 | tail -4" > "$OUT/cuda.txt"
cat "$OUT/cuda.txt"
mark "cuda end / idle again"
sleep 30
mark "idle again end"
g "echo guest Xid: \$(sudo dmesg | grep -c 'NVRM: Xid'); sudo dmesg | grep 'NVRM: Xid' | tail -3 | cut -c1-200" >> "$OUT/timeline.txt"

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Night 2026-09-29: SotTR's built-in benchmark in Linux B with evidence that it runs and that the
# client shows pictures. Linux B: 4 GiB on slot 0, the given device binary, trace off. In the guest:
# the desktop, Sunshine, a monitor (VRAM, NVKMS refusals, Xids) and the game through Steam; its
# preferences carry ExtraCommandLine=-benchmark (backup preferences.pre-benchmark-2026-09-29).
# On the host: Moonlight windowed and a grim screenshot of its window every 30 s with the mean
# brightness and the change against the previous shot. Ends at the results file or after
# SOTTR_MAX_S (default 2100 s). SOTTR_RES=WxH (default 1920x1080) sets the fullscreen resolution;
# the script sets -benchmark and skips the options dialog itself, and restores the guest's
# preferences at the end. Usage: sottr-bench.sh OUT BINARY
set -uo pipefail
OUT=$1 BIN=$2
MAX=${SOTTR_MAX_S:-2100}
RES=${SOTTR_RES:-1920x1080}; RW=${RES%x*}; RH=${RES#*x}
W=/mnt/vmstore/caraxes/night-2026-09-28/desktop-b-night.sh
mkdir -p "$OUT/host-shots" "$OUT/guest-shots"
log() { echo "$(date +%T) $*" | tee -a "$OUT/timeline.txt"; }
gssh() { timeout "${T:-60}" "$W" ssh "$@" 2>&1 | grep -v "unable to resolve"; }

NIGHT_SERVER=$BIN NIGHT_FB_MIB=4096 NIGHT_SLOT=0 CARAXES_NO_TRACE=1 timeout 400 "$W" up > "$OUT/up.txt" 2>&1 \
    || { log "up failed"; tail -5 "$OUT/up.txt"; exit 1; }
timeout 240 "$W" wait 200 >> "$OUT/up.txt" 2>&1
log "vm up"
T=200 gssh 'bash -s' > "$OUT/guest-prep.txt" <<'G'
sudo ip route replace default via 192.168.106.1 dev eth0 && sudo resolvectl dns eth0 192.168.188.1 && sudo resolvectl domain eth0 "~."
sudo modprobe nvidia && sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm modeset=1 && echo modules-ok
bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1; sudo systemctl start gdm3; sleep 12
export XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
systemctl --user start app-dev.lizardbyte.app.Sunshine.service; sleep 6
echo "sunshine: $(systemctl --user is-active app-dev.lizardbyte.app.Sunshine.service)"
cat > /tmp/sottr-mon.sh <<'M'
#!/bin/bash
G=Shadow; G="${G}OfTheTombRaider"
while true; do
  p=$(pgrep -f "bin/$G" | head -1)
  echo "$(date +%T) vram=$(nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits | tr -d ' ') gpu=$(nvidia-smi --query-gpu=utilization.gpu --format=csv,noheader,nounits | tr -d ' ') game=${p:-none} nvkms_fail=$(sudo dmesg | grep -c 'Failed to allocate NVKMS') xid=$(sudo dmesg | grep -c 'NVRM: Xid')"
  sleep 5
done
M
chmod +x /tmp/sottr-mon.sh; setsid -f /tmp/sottr-mon.sh > /tmp/sottr-mon.log 2>&1
echo "saves before: $(ls ~/.local/share/feral-interactive/'Shadow of the Tomb Raider'/SaveData | grep -c benchmarkresults)"
G
# The benchmark variant of the preferences: -benchmark, no options dialog, the resolution.
T=60 gssh 'bash -s' >> "$OUT/timeline.txt" <<PREFS
P="\$HOME/.local/share/feral-interactive/Shadow of the Tomb Raider/preferences"
cp "\$P" "\$P.before-sottr-bench"
sed -i -e 's|<value name="ExtraCommandLine" type="string">[^<]*</value>|<value name="ExtraCommandLine" type="string">-benchmark</value>|' \\
       -e 's|<value name="ExtraCommandLineEnabled" type="integer">0</value>|<value name="ExtraCommandLineEnabled" type="integer">1</value>|' \\
       -e 's|<value name="GameOptionsDialogShouldShow" type="integer">1</value>|<value name="GameOptionsDialogShouldShow" type="integer">0</value>|' \\
       -e 's|<value name="FullscreenWidth" type="integer">[0-9]*</value>|<value name="FullscreenWidth" type="integer">$RW</value>|' \\
       -e 's|<value name="ScreenW" type="integer">[0-9]*</value>|<value name="ScreenW" type="integer">$RW</value>|' \\
       -e 's|<value name="FullscreenHeight" type="integer">[0-9]*</value>|<value name="FullscreenHeight" type="integer">$RH</value>|' \\
       -e 's|<value name="ScreenH" type="integer">[0-9]*</value>|<value name="ScreenH" type="integer">$RH</value>|' "\$P"
grep -E '"(ExtraCommandLine|ExtraCommandLineEnabled|GameOptionsDialogShouldShow|FullscreenWidth|FullscreenHeight|ScreenW|ScreenH)"' "\$P" | sed -E 's/^ +//' | sort | uniq -c
PREFS
cat "$OUT/guest-prep.txt" >> "$OUT/timeline.txt"

# The host client.
setsid nohup env WAYLAND_DISPLAY=wayland-1 DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 LANG=C.UTF-8 /usr/bin/moonlight stream 192.168.106.26 Desktop --resolution 1920x1080 --fps 60 --bitrate 20000 --display-mode windowed --absolute-mouse --video-decoder auto > "$OUT/moonlight.log" 2>&1 < /dev/null &
MP=$!; echo $MP > "$OUT/moonlight.pid"; sleep 10
log "moonlight pid $MP alive=$(kill -0 $MP 2>/dev/null && echo yes || echo no)"

# The game (the launcher window, then Return).
T=330 gssh 'bash -s' > "$OUT/game-start.txt" <<'G'
export XDG_RUNTIME_DIR=/run/user/1000 DISPLAY=:0 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
export XAUTHORITY=$(ls -t /run/user/1000/.mutter-Xwaylandauth.* | head -1)
setsid nohup /usr/games/steam steam://rungameid/750920 > /tmp/steam-run.log 2>&1 < /dev/null &
G=Shadow; G="${G}OfTheTombRaider"
for i in $(seq 1 100); do
  w=$(xdotool search --name "Shadow of the Tomb Raider Options" 2>/dev/null | head -1)
  if [ -n "$w" ]; then
    echo "launcher window $w after $((i*3)) s: clicking PLAY"
    xdotool windowactivate $w 2>/dev/null; sleep 1; xdotool mousemove --window $w 878 650 click 1; sleep 10
  fi
  g=$(xdotool search --name "^Shadow of the Tomb Raider$" 2>/dev/null | head -1)
  if [ -n "$g" ]; then echo "game window $g after $((i*3)) s: game starting"; break; fi
  sleep 3
done
ps -eo pid,etime,pcpu,rss,args | grep "bin/$G" | grep -v grep | cut -c1-120
G
cat "$OUT/game-start.txt" >> "$OUT/timeline.txt"

shot_host() {  # the Moonlight window only
    local g
    g=$(hyprctl clients -j 2>/dev/null | python3 -c 'import json,sys
for c in json.load(sys.stdin):
    if "moonlight" in (c.get("class","")+c.get("title","")).lower():
        (x,y),(w,h)=c["at"],c["size"]; print(f"{x},{y} {w}x{h}"); break' 2>/dev/null)
    [[ -n $g ]] || { echo "no-window"; return; }
    grim -g "$g" "$1" 2>/dev/null && magick "$1" -colorspace gray -format "%[fx:mean]" info: 2>/dev/null
}
shot_guest() {  # the game window (Xwayland), else the root window
    T=60 gssh 'export DISPLAY=:0 XAUTHORITY=$(ls -t /run/user/1000/.mutter-Xwaylandauth.* | head -1); w=$(xdotool search --name "^Shadow of the Tomb Raider$" 2>/dev/null | head -1); if [ -n "$w" ]; then xwd -id $w -silent; else xwd -root -silent; fi | gzip -1 | base64 -w0' \
        | base64 -d 2>/dev/null | gunzip 2>/dev/null > "$1.xwd" && magick "xwd:$1.xwd" "$1" 2>/dev/null && rm -f "$1.xwd" && magick "$1" -colorspace gray -format "%[fx:mean]" info: 2>/dev/null
}

t0=$(date +%s); prev=""; n=0
while (( $(date +%s) - t0 < MAX )); do
    n=$((n + 1)); ts=$(date +%H%M%S)
    hm=$(shot_host "$OUT/host-shots/$ts.png")
    diff="-"
    [[ -n $prev && -f $prev && -f $OUT/host-shots/$ts.png ]] && diff=$(magick compare -metric RMSE "$prev" "$OUT/host-shots/$ts.png" null: 2>&1 | cut -d' ' -f1)
    [[ -f $OUT/host-shots/$ts.png ]] && prev=$OUT/host-shots/$ts.png
    gm="-"; (( n % 2 == 1 )) && gm=$(shot_guest "$OUT/guest-shots/$ts.png")
    mon=$(T=30 gssh 'tail -1 /tmp/sottr-mon.log')
    res=$(T=30 gssh 'ls ~/.local/share/feral-interactive/"Shadow of the Tomb Raider"/SaveData | grep benchmarkresults | grep -v frametimes | grep -v feral_ | tail -1')
    log "host-mean=${hm:-?} host-rmse=$diff guest-mean=${gm:-?} | $mon | result=${res:-none}"
    [[ -n $res && $res != *"No such"* ]] && break
    sleep 30
done
T=60 gssh 'D=~/.local/share/feral-interactive/"Shadow of the Tomb Raider"/SaveData; f=$(ls -t "$D" | grep benchmarkresults | grep -v frametimes | grep -v feral_ | head -1); [ -n "$f" ] && head -60 "$D/$f"' > "$OUT/benchmark-result.txt"
T=60 gssh 'cat /tmp/sottr-mon.log; echo ===; sudo dmesg | grep -E "NVKMS|NVRM: Xid|nvidia-modeset|drm" | tail -20' > "$OUT/guest-monitor.txt"
log "result: $(grep -m1 'Average FPS' "$OUT/benchmark-result.txt" | tr -s ' \t' ' ')"
kill "$MP" 2>/dev/null; log "moonlight stopped"
T=60 gssh 'P="$HOME/.local/share/feral-interactive/Shadow of the Tomb Raider/preferences"; cp "$P" "$P.after-sottr-bench"; cp "$P.before-sottr-bench" "$P"; sync; echo restored' >> "$OUT/timeline.txt"

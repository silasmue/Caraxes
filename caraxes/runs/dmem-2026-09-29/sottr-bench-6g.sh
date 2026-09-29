#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SotTR's built-in benchmark in desktop-b-dmem (Linux B's dmem variant) with the dmem display
# reserve on or off (docs/research/dmem-cap-2026-09-29.md). Derived from
# night-2026-09-28/sottr-bench.sh (same evidence: the monitor, host and guest shots, the results
# file, the preferences restored), with these changes:
# - DMEM_CAP=on (default): after Sunshine starts, `systemctl --user restart caraxes-dmem-cap` so
#   that the cap is measured with Sunshine up, as for a Sunshine that starts with the session;
#   DMEM_CAP=off: `systemctl --user stop caraxes-dmem-cap`, which sets app.slice/dmem.max to max.
# - Sunshine runs in session.slice (the installed drop-in); the game starts through
#   `systemd-run --user --scope --slice=app.slice` (a running Steam takes the URL and the game
#   follows that Steam, so Steam's and the game's cgroups are logged and checked).
# - The monitor adds free VRAM and the dmem usage and limits of app.slice and session.slice.
# 2026-09-29 copy for the 6 GiB run (is 6 GiB enough, or does VRAM leak?): SOTTR_FB_MIB (default
# 6144) and SOTTR_VRAM_FLOOR_MIB (default 512) size the VM, DMEM_CAP defaults to off, Steam starts
# -silent and the game directly through Steam's scout run.sh (Steam's shader-cache prompt blocks
# steam://rungameid; without the runtime the game lacks libopenal), the host's
# free VRAM is logged and the run stops below 200 MiB, and the game is ended after the result to
# log the VRAM that stays in use. SOTTR_DMEM_RESERVE_MIB=<n> (with DMEM_CAP=on) sets
# DISPLAY_RESERVE_MIB=<n> in the guest's /etc/caraxes/dmem-cap.conf for this run (a tagged line,
# removed at the end). SOTTR_DISPLAY_RESERVE=<RegistryDwords> (for example RmDisplayReserve=1, or
# RmDisplayReserve=0 as the control) loads the display reserve build of nvidia.ko
# (display-reserve-prepare.sh) with those registry keys and NVreg_RmMsg=display_reserve.c (its
# state lines in the kernel log) instead of the installed one, and stops if that build is not
# the loaded one; unset keeps the stock modules (docs/research/display-reserve-2026-09-29.md).
# Usage: sottr-bench-6g.sh OUT BINARY   (SOTTR_RES, SOTTR_MAX_S as in sottr-bench.sh)
set -uo pipefail
OUT=$1 BIN=$2
MAX=${SOTTR_MAX_S:-2100}
RES=${SOTTR_RES:-1920x1080}; RW=${RES%x*}; RH=${RES#*x}
W=$(cd "$(dirname "$0")" && pwd)/desktop-b-dmem-6g.sh
CAP=${DMEM_CAP:-off}
[[ $CAP == on || $CAP == off ]] || { echo "DMEM_CAP must be on or off"; exit 2; }
mkdir -p "$OUT/host-shots" "$OUT/guest-shots"
log() { echo "$(date +%T) $*" | tee -a "$OUT/timeline.txt"; }
gssh() { timeout "${T:-60}" "$W" ssh "$@" 2>&1 | grep -v "unable to resolve"; }

NIGHT_SERVER=$BIN NIGHT_FB_MIB=${SOTTR_FB_MIB:-6144} NIGHT_VRAM_FLOOR_MIB=${SOTTR_VRAM_FLOOR_MIB:-512} NIGHT_SLOT=0 CARAXES_NO_TRACE=1 timeout 400 "$W" up > "$OUT/up.txt" 2>&1 \
    || { log "up failed"; tail -5 "$OUT/up.txt"; exit 1; }
timeout 240 "$W" wait 200 >> "$OUT/up.txt" 2>&1
log "vm up"
T=200 gssh "DMEM_CAP=$CAP DMEM_R=${SOTTR_DMEM_RESERVE_MIB:-} DRSV=${SOTTR_DISPLAY_RESERVE:-} bash -s" > "$OUT/guest-prep.txt" <<'G'
sudo ip route replace default via 192.168.106.1 dev eth0 && sudo resolvectl dns eth0 192.168.188.1 && sudo resolvectl domain eth0 "~."
if [ -n "$DRSV" ]; then sudo /opt/caraxes/display-reserve/load.sh "$DRSV" NVreg_RmMsg=display_reserve.c || exit 1; fi
sudo modprobe nvidia && sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm modeset=1 && echo modules-ok
bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1; sudo systemctl start gdm3; sleep 12
export XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
systemctl --user start app-dev.lizardbyte.app.Sunshine.service; sleep 6
echo "sunshine: $(systemctl --user is-active app-dev.lizardbyte.app.Sunshine.service) $(cat /proc/$(pgrep -x sunshine | head -1)/cgroup 2>/dev/null)"
if [ -n "$DMEM_R" ]; then sudo mkdir -p /etc/caraxes; sudo touch /etc/caraxes/dmem-cap.conf; sudo sed -i '/# sottr-bench$/d' /etc/caraxes/dmem-cap.conf; echo "DISPLAY_RESERVE_MIB=$DMEM_R # sottr-bench" | sudo tee -a /etc/caraxes/dmem-cap.conf; fi
if [ "$DMEM_CAP" = on ]; then systemctl --user restart caraxes-dmem-cap.service; else systemctl --user stop caraxes-dmem-cap.service; fi
echo "caraxes-dmem-cap ($DMEM_CAP): $(systemctl --user is-active caraxes-dmem-cap.service)"
journalctl --user -u caraxes-dmem-cap.service -b --no-pager -o cat | tail -n 6
/usr/local/libexec/caraxes/caraxes-dmem-cap status
cat > /tmp/sottr-mon.sh <<'M'
#!/bin/bash
G=Shadow; G="${G}OfTheTombRaider"
U=/sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service
R=$(awk '$1 ~ /^nvidia\/.*\/vidmem$/ { print $1; exit }' /sys/fs/cgroup/dmem.capacity 2>/dev/null)
dm() { awk -v r="$R" '$1 == r { v = $2 } END { if (v == "") print "-"; else if (v == "max") print "max"; else printf "%d", v / 1048576 }' "$1" 2>/dev/null || echo -; }
while true; do
  p=$(pgrep -f "bin/$G" | head -1)
  echo "$(date +%T) vram=$(nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits | tr -d ' ') free=$(nvidia-smi --query-gpu=memory.free --format=csv,noheader,nounits | tr -d ' ') gpu=$(nvidia-smi --query-gpu=utilization.gpu --format=csv,noheader,nounits | tr -d ' ') app=$(dm $U/app.slice/dmem.current)/$(dm $U/app.slice/dmem.max) session=$(dm $U/session.slice/dmem.current) game=${p:-none} nvkms_fail=$(sudo dmesg | grep -c 'Failed to allocate NVKMS') xid=$(sudo dmesg | grep -c 'NVRM: Xid')"
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
if [[ -n ${SOTTR_DISPLAY_RESERVE:-} ]] && ! grep -q "^loaded /opt/caraxes/display-reserve/" "$OUT/guest-prep.txt"; then
    log "the display reserve build of nvidia.ko is not loaded (guest-prep.txt): stopping"; exit 1
fi

# The host client.
setsid nohup env WAYLAND_DISPLAY=wayland-1 DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 LANG=C.UTF-8 /usr/bin/moonlight stream 192.168.106.26 Desktop --resolution 1920x1080 --fps 60 --bitrate 20000 --display-mode windowed --absolute-mouse --video-decoder auto > "$OUT/moonlight.log" 2>&1 < /dev/null &
MP=$!; echo $MP > "$OUT/moonlight.pid"; sleep 10
log "moonlight pid $MP alive=$(kill -0 $MP 2>/dev/null && echo yes || echo no)"

# The game (the launcher window, then Return).
T=900 gssh 'bash -s' > "$OUT/game-start.txt" <<'G'
export XDG_RUNTIME_DIR=/run/user/1000 DISPLAY=:0 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
export XAUTHORITY=$(ls -t /run/user/1000/.mutter-Xwaylandauth.* | head -1)
for p in $(pgrep -x steam); do echo "steam $p before launch: $(cat /proc/$p/cgroup)"; done
setsid nohup systemd-run --user --scope --slice=app.slice --collect -- /usr/games/steam -silent > /tmp/steam-run.log 2>&1 < /dev/null &
# The game needs Steam logged on: started too early it exits at once with "SteamAPI_Init()
# failed; connect to global user failed". Retry until it stays up (at most 8 tries).
GD="/games/SteamLibrary/steamapps/common/Shadow of the Tomb Raider"
G=Shadow; G="${G}OfTheTombRaider"
for t in 1 2 3 4 5 6 7 8; do
  sleep 30
  (cd "$GD" && setsid nohup systemd-run --user --scope --slice=app.slice --collect -- "$HOME/.steam/debian-installation/ubuntu12_32/steam-runtime/run.sh" env SteamAppId=750920 SteamGameId=750920 "$GD/ShadowOfTheTombRaider.sh" > /tmp/game-run.log 2>&1 < /dev/null &)
  sleep 25
  if pgrep -f "bin/$G" > /dev/null; then echo "steam -silent, game started directly (try $t)"; break; fi
  echo "try $t: the game exited: $(grep -m1 -E 'FAIL|rror' /tmp/game-run.log)"
done
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
for p in $(pgrep -x steam) $(pgrep -f "bin/$G" | head -1); do
  c=$(cat /proc/$p/cgroup 2>/dev/null)
  case $c in */app.slice/*) w="";; *) w=" WARNING: outside app.slice, not capped";; esac
  echo "cgroup $(cat /proc/$p/comm 2>/dev/null) $p: $c$w"
done
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
    hf=$(nvidia-smi --query-gpu=memory.free --format=csv,noheader,nounits | tr -d ' ')
    log "host-free=$hf host-mean=${hm:-?} host-rmse=$diff guest-mean=${gm:-?} | $mon | result=${res:-none}"
    (( ${hf:-0} < 200 )) && { log "host free VRAM ${hf:-?} MiB below 200 MiB: stopping"; break; }
    # A refusal freezes the stream while the game keeps drawing; at 720p that ended in a host
    # Xid 109 about a minute later. End the game at the first refusal (the evidence is logged).
    if [[ $mon =~ nvkms_fail=([1-9][0-9]*) ]]; then
        log "NVKMS refusal seen: ending the game now (host Xid 109 risk of a frozen run)"
        T=60 gssh 'G=Shadow; G="${G}OfTheTombRaider"; for p in $(pgrep -f "bin/$G"); do kill -9 $p; done'
        break
    fi
    [[ -n $res && $res != *"No such"* ]] && break
    sleep 30
done
T=60 gssh 'D=~/.local/share/feral-interactive/"Shadow of the Tomb Raider"/SaveData; f=$(ls -t "$D" | grep benchmarkresults | grep -v frametimes | grep -v feral_ | head -1); [ -n "$f" ] && head -60 "$D/$f"' > "$OUT/benchmark-result.txt"
T=60 gssh 'cat /tmp/sottr-mon.log; echo ===; sudo dmesg | grep -E "NVKMS|NVRM: Xid|nvidia-modeset|drm|display reserve" | tail -30' > "$OUT/guest-monitor.txt"
T=60 gssh 'export XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus; uname -r; journalctl -b -u user@1000.service --no-pager -o cat | grep caraxes-dmem-cap; journalctl --user -u caraxes-dmem-cap.service -b --no-pager -o cat; /usr/local/libexec/caraxes/caraxes-dmem-cap status' > "$OUT/guest-dmem.txt"
log "dmem ($CAP): $(grep -c 'read back ok' "$OUT/guest-dmem.txt") cap line(s); nvkms_fail=$(grep -v '^===' "$OUT/guest-monitor.txt" | grep -o 'nvkms_fail=[0-9]*' | tail -1)"
log "result: $(grep -m1 'Average FPS' "$OUT/benchmark-result.txt" | tr -s ' \t' ' ')"
T=60 gssh 'G=Shadow; G="${G}OfTheTombRaider"; p=$(pgrep -f "bin/$G" | head -1); [ -n "$p" ] && kill $p; sleep 25; echo "after the game: $(tail -1 /tmp/sottr-mon.log)"' >> "$OUT/timeline.txt"
kill "$MP" 2>/dev/null; log "moonlight stopped"
T=60 gssh 'P="$HOME/.local/share/feral-interactive/Shadow of the Tomb Raider/preferences"; cp "$P" "$P.after-sottr-bench"; cp "$P.before-sottr-bench" "$P"; sync; echo restored' >> "$OUT/timeline.txt"
T=60 gssh "sudo sed -i '/# sottr-bench\$/d' /etc/caraxes/dmem-cap.conf 2>/dev/null; echo conf-restored" >> "$OUT/timeline.txt"

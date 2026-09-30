#!/usr/bin/env bash
# The user's Linux B desktop session (615 guest, desktop-b) with the merged dev binary (stack ffec4d49, 2026-09-30):
# kick-smoke's bring-up (desktop-b-night.sh: display and kick page, BAR2 memfd, pool, FB 1024 MiB),
# Sunshine forced to encoder = nvenc.
#   up / moonlight / down, as sottr-linux.sh; the log says "ready". NIGHT_FB_MIB=2048 for more VRAM.
set -uo pipefail
W=/home/silas/git/Caraxes/caraxes/runs/night-2026-09-28/desktop-b-night.sh L=/mnt/vmstore/caraxes/runs-2026-09-30/user
G=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b
BIN=${BIN:-/mnt/vmstore/caraxes/vfio-user-nvgpu-stack-ffec4d49}
case ${1:-} in
up)
    setsid nohup /mnt/vmstore/caraxes/locks/vmlock.sh desktop-b 7200 "$0" hold > "$L/linuxb.log" 2>&1 < /dev/null &
    echo "starting; follow: tail -f $L/linuxb.log   (wait for 'ready')" ;;
hold)
    echo "$(date +%T) up with $BIN"
    NIGHT_SERVER=$BIN timeout 400 "$W" up || { echo "up failed"; exit 1; }
    timeout 240 "$W" wait 200
    timeout 60 "$W" ssh "sed -i 's/^encoder = .*/encoder = nvenc/' ~/.config/sunshine/sunshine.conf; grep -E ^encoder ~/.config/sunshine/sunshine.conf; systemctl --user restart sunshine; sleep 5; systemctl --user is-active sunshine"
    echo "$(date +%T) ready: $0 moonlight"
    p=$(cat "$G/gpu.pid" 2>/dev/null)
    while [ -n "$p" ] && kill -0 "$p" 2>/dev/null; do sleep 10; done
    echo "$(date +%T) guest down, card free" ;;
moonlight)
    setsid nohup env WAYLAND_DISPLAY=wayland-1 DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 LANG=C.UTF-8 \
        /usr/bin/moonlight stream 192.168.106.26 Desktop --resolution 1920x1080 --fps 60 --bitrate 20000 \
        --display-mode windowed --absolute-mouse --video-decoder auto --frame-pacing \
        > "$L/moonlight-linuxb.log" 2>&1 < /dev/null &
    echo "moonlight pid $!" ;;
down) timeout 240 "$W" down ;;
*) sed -n '2,6p' "$0"; exit 2 ;;
esac

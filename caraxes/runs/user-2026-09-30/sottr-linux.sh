#!/usr/bin/env bash
# The user's own SotTR session on Linux (desktop-b-dmem) with the gate candidate fix2 (2026-09-30).
# Same guest settings as the evening's SotTR A/B runs (sottr-ab.sh): stock kernel, 12 GiB RAM,
# 4 GiB FB, host VRAM floor 1 GiB, display reserve v2 on (-1), dmem cap off, Sunshine.
#   up         waits for the card (vmlock: one VM at a time), boots, prepares GDM and Sunshine,
#              then holds the lock until the guest is down; progress in the log (wait for "ready")
#   moonlight  starts the stream on the host (frame pacing on)
#   down       shuts the guest down (frees the card)
# BIN=<path> picks another device binary.
set -uo pipefail
H=/home/silas/git/Caraxes/caraxes/runs/dmem-2026-09-29 W=$H/desktop-b-dmem-6g.sh
G=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b-dmem L=/mnt/vmstore/caraxes/runs-2026-09-30/user
BIN=${BIN:-/mnt/vmstore/caraxes/vfio-user-nvgpu-dv2fix2-b6d01293}
case ${1:-} in
up)
    setsid nohup /mnt/vmstore/caraxes/locks/vmlock.sh desktop-b-dmem 7200 "$0" hold > "$L/sottr-linux.log" 2>&1 < /dev/null &
    echo "starting; follow: tail -f $L/sottr-linux.log   (wait for 'ready')" ;;
hold)
    export DMEM_KERNEL=stock
    echo "$(date +%T) up with $BIN"
    NIGHT_SERVER=$BIN NIGHT_MEM=12G NIGHT_FB_MIB=4096 NIGHT_VRAM_FLOOR_MIB=1024 NIGHT_SLOT=0 CARAXES_NO_TRACE=1 \
        timeout 400 "$W" up || { echo "up failed"; exit 1; }
    timeout 240 "$W" wait 200
    timeout 200 "$W" ssh 'bash -s' <<'GUEST'
sudo ip route replace default via 192.168.106.1 dev eth0 && sudo resolvectl dns eth0 192.168.188.1 && sudo resolvectl domain eth0 "~."
sudo /opt/caraxes/display-reserve-v2/load.sh -1 NVreg_RmMsg=display_reserve.c
sudo modprobe nvidia && sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm modeset=1 && echo modules-ok
bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1; sudo systemctl start gdm3; sleep 12
export XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
systemctl --user start app-dev.lizardbyte.app.Sunshine.service; sleep 6
systemctl --user stop caraxes-dmem-cap.service
echo "sunshine: $(systemctl --user is-active app-dev.lizardbyte.app.Sunshine.service)"
GUEST
    echo "$(date +%T) ready: $0 moonlight"
    p=$(cat "$G/gpu.pid" 2>/dev/null)
    while [ -n "$p" ] && kill -0 "$p" 2>/dev/null; do sleep 10; done
    echo "$(date +%T) guest down, card free" ;;
moonlight)
    setsid nohup env WAYLAND_DISPLAY=wayland-1 DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 LANG=C.UTF-8 \
        /usr/bin/moonlight stream 192.168.106.26 Desktop --resolution 1920x1080 --fps 60 --bitrate 20000 \
        --display-mode windowed --absolute-mouse --video-decoder auto --frame-pacing \
        > "$L/moonlight-sottr-linux.log" 2>&1 < /dev/null &
    echo "moonlight pid $!" ;;
down) timeout 240 "$W" down ;;
*) sed -n '2,11p' "$0"; exit 2 ;;
esac

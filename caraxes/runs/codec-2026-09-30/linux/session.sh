#!/usr/bin/env bash
# One Linux B (desktop-b, 615) session (run inside vmlock.sh desktop-b): up, steps, always down.
# Usage: session.sh TAG BINARY-cap STEPS...   steps: copy matrix[=regex] nvenc software probe sleep=N
set -uo pipefail
TAG=$1 BIN=$2; shift 2
C=/mnt/vmstore/caraxes/runs-2026-09-30/codec
W=/mnt/vmstore/caraxes/night-2026-09-28/desktop-b-night.sh
L=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b
OUT=$C/linux-$TAG; mkdir -p "$OUT"
g() { timeout ${T:-60} "$W" ssh "$@" 2>&1 | grep -v "unable to resolve"; }
down() { echo "down $(date +%T)"; timeout 200 "$W" down > "$OUT/down.txt" 2>&1; tail -3 "$OUT/down.txt";
         cp $L/gpu.log "$OUT/gpu.log" 2>/dev/null; grep "interrupt vectors by the guest" "$OUT/gpu.log" | tail -1 | cut -c1-900; }
trap down EXIT
echo "up $TAG $(date +%T) $BIN"
env NIGHT_SERVER=$BIN CARAXES_DISPLAY_KICK_PAGE=1 CARAXES_PROFILE=1 ${DEV_EXTRA:-} timeout 400 "$W" up > "$OUT/up.txt" 2>&1 || { tail -5 "$OUT/up.txt"; exit 1; }
timeout 240 "$W" wait 200 >> "$OUT/up.txt" 2>&1
cat "$OUT/up.txt" | tail -3
for st in "$@"; do
    echo "=== $st $(date +%T)"
    case $st in
    copy)
        g 'mkdir -p ~/codec; ls -la ~/codec | tail -n +2 | awk "{print \$5, \$9}"; which ffmpeg; ffmpeg -hide_banner -encoders 2>/dev/null | grep -c nvenc'
        for f in $C/dl/linux64/ffmpeg $C/dl/linux64/ffprobe $C/dl/clip-* $C/linux/codec-matrix.sh; do
            b=$(basename $f)
            have=$(g "stat -c %s ~/codec/$b 2>/dev/null || echo 0" | tail -1)
            [[ $have == "$(stat -c %s $f)" && $b != *.sh ]] && continue
            cat $f | T=600 g "cat > ~/codec/$b && chmod +x ~/codec/$b" && echo "copied $b"
        done ;;
    matrix*)
        re=${st#matrix}; re=${re#=}
        T=2400 g "FF=~/codec/ffmpeg FP=~/codec/ffprobe bash ~/codec/codec-matrix.sh '$re'" | tee "$OUT/matrix.txt" ;;
    desk)
        T=120 g 'sudo modprobe nvidia && sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm modeset=1 && echo "modules: ok"; bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1; sudo systemctl start gdm3; sleep 10; for i in $(seq 1 30); do ls /run/user/1000/wayland-* >/dev/null 2>&1 && ls /run/user/1000/.mutter-Xwaylandauth.* >/dev/null 2>&1 && break; sleep 1; done; echo "desktop: gnome-shell=$(pgrep -c gnome-shell) Xwayland=$(pgrep -c Xwayland) dri=$(ls /dev/dri | tr "\n" " ") display=$(cat /sys/class/drm/card*-DP-1/status 2>/dev/null | head -1)"' ;;
    probe)
        T=120 g 'timeout 60 ~/nvenc/nvenc-probe 2>&1 | grep -E "^ENC|^DEC|RESULT"' | tee "$OUT/probe.txt" ;;
    nvenc|software)
        bash $C/linux/stream.sh "$OUT/stream-$st" $st ${STREAM_S:-150} ;;
    sleep=*) sleep ${st#sleep=} ;;
    *) echo "unknown step $st" ;;
    esac
done

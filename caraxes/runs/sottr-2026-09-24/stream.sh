#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The SotTR stage, the stream (docs/caraxes/sottr-stage.md section 4), on this host:
#  1. Sunshine in the guest with capture=kms and the web login (guest/sottr-session.sh
#     sunshine-start), unless it listens already;
#  2. Moonlight paired with it only when `moonlight list` cannot reach it paired, the flow of
#     v1's bench.sh do_stream (Leandro-Test scripts/bench.sh:1031-1042) with the retries of the
#     display stream stage (caraxes/runs/display-f-2026-09-24/stream.sh): `moonlight pair --pin`
#     in the background, the PIN posted to Sunshine's /api/pin with the web login;
#  3. `moonlight stream <ip> Desktop` opened in a window (SOTTR_MOUSE=--no-absolute-mouse for the
#     game's relative mouse; the default is the remote-desktop mouse for the login and Steam) on the operator's desktop session
#     (Wayland, WAYLAND_DISPLAY=wayland-1, DISPLAY=:0), detached with setsid nohup; its pid goes
#     to $W/moonlight.pid so that down.sh quits exactly this instance.
# The script does not wait for the stream. Moonlight prints its session summary into
# moonlight.log of the session folder when the stream ends (down.sh runs `moonlight quit`).
# Usage: stream.sh [WxH] [fps] [bitrate kbps]    (default 1920x1080 60 20000, as display-e.md §7)
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
# shellcheck source=caraxes/runs/sottr-2026-09-24/env.sh
source "$here/env.sh"
res=${1:-1920x1080} fps=${2:-60} bitrate=${3:-20000}
W=${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM
IP=$CARAXES_IP
OUT=$(cat "$W/sottr-session" 2>/dev/null) || OUT=$here/session-stream-$(date +%Y%m%d-%H%M%S)
mkdir -p "$OUT"
ML=${MOONLIGHT:-/usr/bin/moonlight}
VMSH=("$LEA_TOOLS/caraxes-vm.sh")
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
[[ -x $ML ]] || die "$ML missing"
[[ -s $W/ch.pid ]] && kill -0 "$(cat "$W/ch.pid")" 2>/dev/null || die "vm4 is not running (up.sh)"
if [[ -s $W/moonlight.pid ]] && kill -0 "$(cat "$W/moonlight.pid")" 2>/dev/null; then
    die "a Moonlight of this stage runs already (pid $(cat "$W/moonlight.pid")); down.sh or close it"
fi

# 1. Sunshine.
if ! "${VMSH[@]}" ssh '(exec 3<>/dev/tcp/127.0.0.1/47989) 2>/dev/null'; then
    "${VMSH[@]}" ssh "SUN_USER=$SOTTR_SUN_USER SUN_PASS=$SOTTR_SUN_PASS bash caraxes-sottr/sottr-session.sh sunshine-start" \
        > "$OUT/sunshine-start-stream.txt" 2>&1 || { cat "$OUT/sunshine-start-stream.txt"; die "Sunshine did not start"; }
fi
echo "sunshine listens on $IP:47989"

# 2. Pairing, only when needed. One pair attempt at a time: two at once give "Incorrect PIN"
#    while the API still answers {"status":true} (bench.sh:1031-1032).
if timeout 25 "$ML" list "$IP" > "$OUT/moonlight-list.txt" 2>&1; then
    echo "moonlight: already paired with $IP"
else
    echo "moonlight: pairing with $IP (PIN $SOTTR_SUN_PIN through Sunshine's API)"
    ( timeout 60 "$ML" pair "$IP" --pin "$SOTTR_SUN_PIN" > "$OUT/pair.log" 2>&1 ) &
    pp=$!
    for i in 1 2 3 4 5 6; do
        sleep 4
        curl -sk -m 10 -u "$SOTTR_SUN_USER:$SOTTR_SUN_PASS" -H 'Content-Type: application/json' \
            -d "{\"pin\":\"$SOTTR_SUN_PIN\",\"name\":\"host\"}" "https://$IP:47990/api/pin" > "$OUT/pin-$i.txt" 2>&1
        grep -q '"status":true' "$OUT/pin-$i.txt" && break
    done
    wait $pp
    timeout 25 "$ML" list "$IP" > "$OUT/moonlight-list.txt" 2>&1 || die "pairing failed, $OUT/pair.log"
    echo "moonlight: paired"
fi

# 3. The stream window on the operator's desktop.
echo "$(date -Iseconds) moonlight stream $IP Desktop --resolution $res --fps $fps --bitrate $bitrate --display-mode windowed ${SOTTR_MOUSE:---absolute-mouse} --video-decoder ${SOTTR_DECODER:-software}" >> "$OUT/stream.txt"
setsid nohup env WAYLAND_DISPLAY="${SOTTR_WAYLAND_DISPLAY:-wayland-1}" DISPLAY="${SOTTR_DISPLAY:-:0}" \
    XDG_RUNTIME_DIR="/run/user/$(id -u)" \
    "$ML" stream "$IP" Desktop --resolution "$res" --fps "$fps" --bitrate "$bitrate" --display-mode windowed ${SOTTR_MOUSE:---absolute-mouse} --video-decoder ${SOTTR_DECODER:-software} \
    >> "$OUT/moonlight.log" 2>&1 < /dev/null &
mp=$!
sleep 2
kill -0 "$mp" 2>/dev/null || { tail -20 "$OUT/moonlight.log"; die "Moonlight exited at once"; }
[[ $(cat "/proc/$mp/comm" 2>/dev/null) == moonlight ]] || echo "note: pid $mp is $(cat "/proc/$mp/comm" 2>/dev/null)"
echo "$mp" > "$W/moonlight.pid"; echo "$mp" > "$OUT/moonlight.pid"
echo "moonlight pid $mp, log $OUT/moonlight.log; close the window or run down.sh to end the stream"

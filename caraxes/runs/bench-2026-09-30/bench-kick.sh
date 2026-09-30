#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Linux B (desktop-b): caraxes/runs/night-2026-09-28/kick-smoke.sh with the whole
# guest-smoke-graphics.sh (vkmark twice and glmark2-wayland at 1280x720 after the smoke
# suite), then the same Moonlight stream check. Usage: bench-kick.sh OUT BINARY
set -uo pipefail
OUT=$1 BIN=$2; N=$(cd "$(dirname "$(readlink -f "$0")")/../night-2026-09-28" && pwd -P)
W=$N/desktop-b-night.sh
mkdir -p "$OUT"
NIGHT_SERVER=$BIN timeout 400 "$W" up > "$OUT/up.txt" 2>&1 || { tail -3 "$OUT/up.txt"; exit 1; }
timeout 240 "$W" wait 200 >> "$OUT/up.txt" 2>&1
timeout 1500 "$W" ssh 'bash -s' < "$N/guest-smoke-graphics.sh" 2>&1 | grep -v "unable to resolve" > "$OUT/smoke.txt"
# keep the desktop busy for the stream: vkcube on X11 for 60 s
timeout 30 "$W" ssh 'export DISPLAY=:0 XAUTHORITY=$(ls -t /run/user/1000/.mutter-Xwaylandauth.* | head -1); (timeout 70 vkcube > /dev/null 2>&1 &)' > /dev/null 2>&1
setsid nohup env WAYLAND_DISPLAY=wayland-1 DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 LANG=C.UTF-8 /usr/bin/moonlight stream 192.168.106.26 Desktop --resolution 1920x1080 --fps 60 --bitrate 20000 --display-mode windowed --absolute-mouse --video-decoder auto > "$OUT/moonlight.log" 2>&1 < /dev/null &
MP=$!; sleep 15; prev=""
for i in 1 2 3; do
    g=$(hyprctl clients -j | python3 -c 'import json,sys
for c in json.load(sys.stdin):
    if "moonlight" in (c.get("class","")+c.get("title","")).lower():
        (x,y),(w,h)=c["at"],c["size"]; print(f"{x},{y} {w}x{h}"); break')
    f="$OUT/stream-$i.png"; grim -g "$g" "$f"
    m=$(magick "$f" -colorspace gray -format "%[fx:mean]" info:)
    d="-"; [[ -n $prev ]] && d=$(magick compare -metric RMSE "$prev" "$f" null: 2>&1 | cut -d' ' -f1)
    echo "shot $i mean=$m rmse=$d" | tee -a "$OUT/stream.txt"; prev=$f; sleep 10
done
kill "$MP" 2>/dev/null
echo "guest xid: $(timeout 30 "$W" ssh "sudo dmesg | grep -c 'NVRM: Xid'" 2>/dev/null | grep -v unable)" >> "$OUT/stream.txt"
timeout 200 "$W" down > "$OUT/down.txt" 2>&1
grep -hE "display kick page|ERROR" /mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b/gpu.log | grep -v "bar1 pool" | cut -c1-250 > "$OUT/device-summary.txt"
grep -a "config: " /mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b/gpu.log | cut -c1-600 > "$OUT/config.txt"; cp /mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b/gpu.log "$OUT/gpu.log"

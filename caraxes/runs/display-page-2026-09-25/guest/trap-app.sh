#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The desktop series of 2026-09-25 (guest, desktop user): the per-frame trap table of a client
# the desktop really runs, under Mutter. Starts GNOME when it does not run, then runs the client
# for SECS seconds in the session; the host cuts a 10 s window out of the device trace.
#   trap-app.sh browser [SECS]  firefox or chromium on a local animated page, if installed
#   trap-app.sh vkcube [SECS]   vkcube-wayland (FIFO), the display gate's Vulkan demo
#   trap-app.sh glmark2 [SECS]  glmark2-wayland's build scene, the display gate's GL demo
cd ~/caraxes-guest || exit 1
unset DISPLAY
app=${1:?usage: trap-app.sh browser|vkcube|glmark2 [secs]}
secs=${2:-16}
if ! pgrep -x gnome-shell >/dev/null; then
    sudo bash gnome-check.sh start 60 | grep -E "Type=wayland|gbm renderer|page_flip"
fi
sleep 5
U=$(id -u)
export XDG_RUNTIME_DIR=/run/user/$U WAYLAND_DISPLAY=wayland-0
echo "== $(date -u +%FT%TZ) $app load $(cat /proc/loadavg)"
case $app in
    browser)
        b=$(command -v firefox firefox-esr chromium chromium-browser google-chrome 2>/dev/null | head -1)
        if [ -z "$b" ]; then echo "APP browser: none installed"; exit 3; fi
        page=/tmp/caraxes-anim.html
        cat > "$page" <<'HTML'
<!doctype html><html><body style="margin:0;background:#123"><canvas id=c width=1280 height=720></canvas>
<script>let c=document.getElementById('c').getContext('2d'),n=0,t0=performance.now();
function f(t){c.fillStyle='#123';c.fillRect(0,0,1280,720);c.fillStyle='#e84';
for(let i=0;i<40;i++){c.beginPath();c.arc(640+500*Math.sin(t/700+i),360+300*Math.cos(t/900+i*1.3),20,0,7);c.fill();}
n++;document.title='frames '+n+' fps '+(n*1000/(t-t0)).toFixed(1);requestAnimationFrame(f);}requestAnimationFrame(f);</script></body></html>
HTML
        case $b in
            *firefox*) MOZ_ENABLE_WAYLAND=1 timeout "$secs" "$b" --kiosk --new-instance --profile "$(mktemp -d)" "file://$page" > /tmp/app-browser.txt 2>&1 ;;
            *) timeout "$secs" "$b" --ozone-platform=wayland --kiosk --no-first-run --user-data-dir="$(mktemp -d)" "file://$page" > /tmp/app-browser.txt 2>&1 ;;
        esac
        echo "APP browser $b exit $?"; tail -3 /tmp/app-browser.txt ;;
    vkcube)
        n=$((secs * 60))
        TIMEFORMAT="ELAPSED %R"
        { time timeout $((secs + 30)) vkcube-wayland --c "$n" > /tmp/app-vkcube.txt 2>&1; } 2> /tmp/app-vkcube.time
        el=$(sed -n 's/^ELAPSED //p' /tmp/app-vkcube.time)
        echo "APP vkcube frames=$n elapsed_s=${el:-?} fps=$( [ -n "$el" ] && echo "scale=2; $n / $el" | bc) $(grep -m1 -iE 'Selected (GPU|WSI)|GPU [0-9]' /tmp/app-vkcube.txt)" ;;
    glmark2)
        timeout $((secs + 30)) glmark2-wayland -s 1280x720 -b "build:duration=$secs" > /tmp/app-glmark2.txt 2>&1
        echo "APP glmark2 exit $? $(grep -E 'GL_RENDERER|\[build\]|glmark2 Score' /tmp/app-glmark2.txt | tr -s ' ' | tr '\n' ';')" ;;
    *) echo "unknown app $app"; exit 2 ;;
esac

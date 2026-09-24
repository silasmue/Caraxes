#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Guest (as the desktop user, with sudo), display phase E (docs/caraxes/display-e.md): the
# classic demos on the GPU path, each for at least 20 s, with the driver that answered.
#   demos.sh weston   each demo as a client of its own Weston (GL renderer, Xwayland on)
#   demos.sh gnome    each demo in the running GNOME session (Mutter, its Xwayland)
# Prints one "DEMO <name> ..." line per demo with the frame rate, and the info probes
# (vulkaninfo --summary, eglinfo -B, glxinfo -B) once per compositor.
set -u
cd "$(dirname "$0")"
WHERE=${1:-weston}
SECS=${SECS:-22}
MODE=1920x1080@60
U=$(id -u)

# run <name> <x11 0|1> <command...>: under the chosen compositor, stdout+stderr to /tmp/demo-<name>.txt
run() {
    local name=$1 x=$2; shift 2
    local out=/tmp/demo-$name.txt t0 t1 rc
    t0=$(date +%s.%N)
    if [ "$WHERE" = weston ]; then
        sudo XWAYLAND="$x" SETTLE=3 timeout $((SECS + 200)) bash ./weston-run.sh "$MODE" gl "$@" > "$out" 2>&1
    else
        local xa
        xa=$(ls /run/user/"$U"/.mutter-Xwaylandauth.* 2>/dev/null | head -1)
        env XDG_RUNTIME_DIR=/run/user/"$U" WAYLAND_DISPLAY=wayland-0 DISPLAY=:0 XAUTHORITY="$xa" \
            timeout $((SECS + 60)) "$@" > "$out" 2>&1
    fi
    rc=$?
    t1=$(date +%s.%N)
    echo "$name: exit $rc wall $(echo "$t1 - $t0" | bc) s"
}

frames=$((SECS * 60))
echo "== $WHERE, $(date -u +%FT%TZ), load $(cat /proc/loadavg)"
# The info probes: which driver answered.
run vulkaninfo 0 vulkaninfo --summary
grep -E "deviceName|driverName|driverInfo|apiVersion|GPU id" /tmp/demo-vulkaninfo.txt | head -8
run eglinfo 0 eglinfo -B -p wayland
grep -E "EGL vendor|OpenGL.*renderer|OpenGL.*version" /tmp/demo-eglinfo.txt | head -6
run glxinfo 1 glxinfo -B
grep -E "direct rendering|OpenGL vendor|OpenGL renderer|OpenGL core profile version" /tmp/demo-glxinfo.txt | head -5

# vkcube: --c N frames under FIFO, the rate is N over the wall time of the frames.
for v in "vkcube-wayland 0 vkcube-wayland --c $frames" "vkcube-xwayland 1 vkcube --c $frames"; do
    set -- $v
    n=$1 x=$2; shift 2
    run "$n" "$x" bash -c 'TIMEFORMAT="ELAPSED %R"; time "$@"' _ "$@"
    el=$(sed -n 's/^ELAPSED //p' /tmp/demo-$n.txt)
    echo "DEMO $n frames=$frames elapsed_s=${el:-?} fps=$( [ -n "$el" ] && echo "scale=1; $frames / $el" | bc) $(grep -m1 -iE 'Selected (GPU|WSI)|GPU [0-9]' /tmp/demo-$n.txt)"
done
# glxgears (Xwayland, GLX), es2gears_wayland and weston-simple-egl (EGL on Wayland) print
# FPS every 5 s (eglgears_wayland prints none)
# (line-buffered, or the timeout's SIGTERM loses them). vkcube is the XCB build of the cube.
for v in "glxgears 1 glxgears" "es2gears_wayland 0 es2gears_wayland" "weston-simple-egl 0 weston-simple-egl"; do
    set -- $v
    n=$1 x=$2; shift 2
    run "$n" "$x" timeout "$SECS" stdbuf -oL "$@"
    echo "DEMO $n $(grep -oE '[0-9]+ frames in [0-9.]+ seconds(:| =) [0-9.]+ (FPS|fps)' /tmp/demo-$n.txt | tr '\n' ';')"
done
# glmark2-wayland: the default scene list at 2 s per scene, the score at the end.
run glmark2-wayland 0 glmark2-wayland -s 1280x720 -b :duration=2.0
echo "DEMO glmark2-wayland $(grep -E 'GL_RENDERER|glmark2 Score' /tmp/demo-glmark2-wayland.txt | tr -s ' ' | tr '\n' ';')"

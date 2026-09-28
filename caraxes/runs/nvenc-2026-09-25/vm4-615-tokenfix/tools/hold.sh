#!/bin/bash
# hold.sh <kind>: holders for ladder2.sh (run with the Wayland env).
cd ~/conc
case $1 in
vkcube) exec vkcube-wayland --c 100000 ;;
eglhigh) EGL_PRIO=high exec ./eglwork 100 ;;
eglnorm) exec ./eglwork 100 ;;
nvenc) exec ./nvenc-hold -D -q -n 100000 -r 60 ;;
vkcube+nvenc) vkcube-wayland --c 100000 > /dev/null 2>&1 & sleep 8; exec ./nvenc-hold -D -q -n 100000 -r 60 ;;
cuwork) exec ./cuwork 100 ;;
esac

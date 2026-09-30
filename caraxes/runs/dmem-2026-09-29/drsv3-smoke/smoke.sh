# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# In the guest (desktop-b-dmem, stock 6.8 kernel, the display reserve build installed): load it
# with display_reserve_mib=${MIB:--1}, walk the connector through 1920x1080, 1280x720 and off as
# DRM master, start GNOME, then fill the VRAM from CUDA and keep grabbing what becomes free (as
# a game does), and ask for dumb buffers: one of the reserve's run shape (1920x1152), two of it
# at once, then a smaller one (1920x1080). Prints VRAM use and the reserve's kernel log lines
# after each step.
set -u
t() { echo "$(date +%T) $*"; }
used() { nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader; }
log() { sudo dmesg | grep -E "display reserve" | tail -n ${1:-4}; sudo dmesg -C; }
cd ~/drsv3-smoke
gcc -O2 -o modeset modeset.c && gcc -O2 -o dumb dumb.c || exit 1
sudo systemctl stop gdm3 2>/dev/null; sleep 3
for m in nvidia_drm nvidia_modeset nvidia_uvm nvidia; do [ -d /sys/module/$m ] && sudo rmmod $m; done
[ -d /sys/module/ecc ] && sudo rmmod ecc && t "ecc unloaded (dependency test)"
sudo dmesg -C
t "== load"; sudo /opt/caraxes/display-reserve-v2/load.sh ${MIB:--1} NVreg_RmMsg=display_reserve.c || { echo "load failed: stopping"; exit 1; }
sudo modprobe nvidia_drm modeset=1 && sudo modprobe nvidia_uvm && echo drm-uvm-ok
bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1
t "no mode: $(used)"; log
t "== modes (DRM master)"
sudo ./modeset -w 5 -c 'echo "  used $(nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader)"; sudo dmesg | grep -E "display reserve" | tail -n 8 | sed "s/^/  /"; sudo dmesg -C' 1920x1080 1280x720
t "== GNOME"; sudo systemctl start gdm3
sleep 30; t "gnome +30s: $(used)"; log 8
t "== serve: CUDA fills the free VRAM and keeps grabbing, then dumb buffers"
python3 fill.py 60 grab > fill.log 2>&1 &
FP=$!
sleep 15; cat fill.log; t "filled: $(used)"
t "run shape: $(sudo ./dumb 3 1920x1152)"; sleep 3; t "after: $(used)"; log 8
t "run shape, two at once:"
sudo ./dumb 6 1920x1152 > a.txt & A=$!
sleep 1; sudo ./dumb 3 1920x1152 > b.txt; wait $A; cat a.txt b.txt
sleep 3; t "after: $(used)"; log 10
t "smaller: $(sudo ./dumb 3 1920x1080)"; sleep 5; t "after: $(used)"; log 8
wait $FP; cat fill.log
sleep 4; t "filler gone: $(used)"; log 6

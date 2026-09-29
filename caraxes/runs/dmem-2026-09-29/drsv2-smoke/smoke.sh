# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# In the guest (desktop-b-dmem, stock 6.8 kernel, the v2 build installed): load the display
# reserve build with display_reserve_mib=${MIB:--1}, walk the connector through 1920x1080, 1280x720
# and off as DRM master, then start GNOME, then fill the VRAM from CUDA and ask for a 1080p dumb
# buffer. Prints VRAM use and the reserve's kernel log lines after each step.
set -u
t() { echo "$(date +%T) $*"; }
used() { nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader; }
log() { sudo dmesg | grep -E "display reserve" | tail -n ${1:-4}; sudo dmesg -C; }
cd ~/drsv2-smoke
gcc -O2 -o modeset modeset.c && gcc -O2 -o dumb dumb.c || exit 1
sudo systemctl stop gdm3 2>/dev/null; sleep 3
for m in nvidia_drm nvidia_modeset nvidia_uvm nvidia; do [ -d /sys/module/$m ] && sudo rmmod $m; done
[ -d /sys/module/ecc ] && sudo rmmod ecc && t "ecc unloaded (dependency test)"
sudo dmesg -C
t "== load"; sudo /opt/caraxes/display-reserve-v2/load.sh ${MIB:--1} NVreg_RmMsg=display_reserve.c; echo "load rc=$?"
sudo modprobe nvidia_drm modeset=1 && sudo modprobe nvidia_uvm && echo drm-uvm-ok
bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1
t "no mode: $(used)"; log
t "== modes (DRM master)"
sudo ./modeset -w 5 -c 'echo "  used $(nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader)"; sudo dmesg | grep -E "display reserve" | tail -n 4 | sed "s/^/  /"; sudo dmesg -C' 1920x1080 1280x720
t "== GNOME"; sudo systemctl start gdm3
sleep 30; t "gnome +30s: $(used)"; log 6
t "== serve: CUDA fills the free VRAM, then 1080p dumb buffers"
python3 fill.py 45 > fill.log 2>&1 &
FP=$!
sleep 15; cat fill.log; t "filled: $(used)"
t "dumb 1: $(sudo ./dumb 3)"; sleep 3; t "after dumb 1: $(used)"; log 6
t "dumb 2: $(sudo ./dumb 3)"; sleep 3; t "after dumb 2: $(used)"; log 6
kill $FP 2>/dev/null; wait $FP 2>/dev/null
sleep 4; t "filler gone: $(used)"; log 4

# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# guest script: the smoke suite (scratchpad smoke-all.sh of 2026-09-28) plus vkmark/glmark2 at 1280x720
sudo ip route replace default via 192.168.106.1 dev eth0 && sudo resolvectl dns eth0 192.168.188.1 && sudo resolvectl domain eth0 "~."
sudo modprobe nvidia && sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm modeset=1 && echo "modules: ok"
grep -o "RegistryDwords.*" /proc/driver/nvidia/params | head -1
bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1; sudo systemctl start gdm3; sleep 10
export XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
for i in $(seq 1 20); do WAYLAND_DISPLAY=$(find $XDG_RUNTIME_DIR -maxdepth 1 -type s -name 'wayland-*' -printf '%f\n' 2>/dev/null | head -1); [ -n "$WAYLAND_DISPLAY" ] && break; sleep 1; done; export WAYLAND_DISPLAY
for i in $(seq 1 20); do XA=$(ls -t /run/user/1000/.mutter-Xwaylandauth.* 2>/dev/null | head -1); [ -n "$XA" ] && break; sleep 1; done
echo "desktop: WAYLAND_DISPLAY=$WAYLAND_DISPLAY gnome-shell=$(pgrep -c gnome-shell) Xwayland=$(pgrep -c Xwayland)"
echo "display: $(cat /sys/class/drm/card*-DP-1/status 2>/dev/null | head -1) $(cat /sys/class/drm/card*-DP-1/modes 2>/dev/null | head -1)"
nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv,noheader
timeout --foreground 30s vkcube-wayland --c 300 >/dev/null 2>&1; echo "vkcube-wayland rc=$?"
DISPLAY=:0 XAUTHORITY=$XA timeout --foreground 30s vkcube --c 300 >/dev/null 2>&1; echo "vkcube-x11 rc=$?"
timeout --foreground 25s glmark2-wayland --benchmark build:duration=10 2>&1 | grep -E "FPS" | sed 's/^/glmark2 /'
timeout 40 ~/conc/vkcomp 2>&1 | tail -2 | sed 's/^/vkcomp: /'; echo "vkcomp rc=${PIPESTATUS[0]}"
timeout 40 ~/conc/cuwork 2>&1 | tail -2 | sed 's/^/cuwork: /'; echo "cuwork rc=${PIPESTATUS[0]}"
timeout 40 ~/nvenc/nvenc-probe 2>&1 | grep -E "^ENC|^DEC|RESULT"
(timeout 45 ~/conc/nvenc-hold > /tmp/hold.log 2>&1 &); sleep 5; DISPLAY=:0 XAUTHORITY=$XA timeout --foreground 30s vkcube --c 300 >/dev/null 2>&1; echo "vkcube-x11 beside NVENC holder rc=$?"; sleep 12; grep RESULT /tmp/hold.log | sed 's/^/holder /'
systemctl --user start app-dev.lizardbyte.app.Sunshine.service; sleep 6; echo "sunshine: $(systemctl --user is-active app-dev.lizardbyte.app.Sunshine.service)"
echo "guest Xid: $(sudo dmesg | grep -c 'NVRM: Xid')"; sudo dmesg | grep 'NVRM: Xid' | tail -3 | cut -c1-200
echo "=== vkmark wayland 1280x720"; for i in 1 2; do timeout --foreground 300 vkmark --winsys wayland -s 1280x720 2>&1 | grep -E "^\[|Score|rror" ; done
echo "=== glmark2-wayland 1280x720"; timeout --foreground 200 glmark2-wayland -s 1280x720 2>&1 | grep -E "Score|rror"
echo "guest Xid after graphics: $(sudo dmesg | grep -c 'NVRM: Xid')"; sudo dmesg | grep 'NVRM: Xid' | tail -3 | cut -c1-200

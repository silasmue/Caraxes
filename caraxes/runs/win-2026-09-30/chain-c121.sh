#!/usr/bin/env bash
# reconnect2: Heaven on a fresh boot, then after one reboot (C119: 130.9 fps after two reboots
# against 153-158 in C116-C118). After the gate's vm10 leg (the host FB floor: Windows needs
# 5.5 GiB free) and with Linux B idle.
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-reconnect2-cap
sleep 30
while pgrep -f "gate-fix2/v[m]10" >/dev/null || pgrep -f "desktop-b/vfio-user-nvgp[u]" >/dev/null; do sleep 15; done
cd /mnt/vmstore/caraxes/runs-2026-09-30/win
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C121-reconnect2-ab "$K" sleep=120 heaven reboot clients sleep=60 heaven events

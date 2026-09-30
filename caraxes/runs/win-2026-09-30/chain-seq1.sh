#!/usr/bin/env bash
# In order, once Linux B (the codec agent's l2, slot 5) has left the card: the gate's vm10 leg
# (its reservation is slot 5 too), then C121 (Windows needs 5.5 GiB of host FB free).
while pgrep -f "desktop-b/vfio-user-nvgp[u]" >/dev/null; do sleep 15; done
cd /mnt/vmstore/caraxes/runs-2026-09-30/gate-fix2 && ./legs.sh vm10
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-reconnect2-cap
cd /mnt/vmstore/caraxes/runs-2026-09-30/win
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C121-reconnect2-ab "$K" sleep=120 heaven reboot clients sleep=60 heaven events

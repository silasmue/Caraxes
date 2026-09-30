#!/usr/bin/env bash
# The reboot with the reconnect fix (reconnect2 = deferred-v2 + 0939364d): two guest reboots, the
# desktop and the host events back each time, then one Heaven.
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-reconnect2-cap
while [ ! -x "$K" ]; do sleep 20; done
cd /mnt/vmstore/caraxes/runs-2026-09-30/win
bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C119-reconnect2 "$K" sleep=60 arm reboot clients reboot clients heaven events
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-flags3-cap
while [ ! -x "$K" ]; do sleep 20; done
nohup python3 watch-hang.py C120-flags3 30 > C120-flags3.watch.log 2>&1 &
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session3.sh C120-flags3 "$K" sleep=120 config gpuclients arm stages heaven superposition heaven superposition gpuclients refused events

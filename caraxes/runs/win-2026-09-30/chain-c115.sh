#!/usr/bin/env bash
# Which api_bundle the refused NV5080_CTRL_CMD_DEFERRED_API_INTERNAL requests carry (diag2 logs
# them): the natural condition (Steam and Edge start by themselves), Heaven, Superposition,
# Heaven, with watch-hang's dumps.
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-diag2-cap
while [ ! -x "$K" ]; do sleep 20; done
while pgrep -f "session2.sh C114-diag1" >/dev/null; do sleep 15; done
cd /mnt/vmstore/caraxes/runs-2026-09-30/win
nohup python3 watch-hang.py C115-diag2-natural 30 > C115-diag2-natural.watch.log 2>&1 &
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C115-diag2-natural "$K" sleep=120 gpuclients heaven superposition heaven gpuclients events

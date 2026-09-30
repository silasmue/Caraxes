#!/usr/bin/env bash
# The gate candidate fix2 (deferred-v2 b6d01293): the natural condition, four runs, the
# migration lines with the old ring's last GPPut (exactness).
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-dv2fix2-cap
while pgrep -f "chain-c11[6]" >/dev/null || pgrep -f "session2.sh C11[7]" >/dev/null; do sleep 15; done
cd /mnt/vmstore/caraxes/runs-2026-09-30/win
nohup python3 watch-hang.py C118-fix2-natural 30 > C118-fix2-natural.watch.log 2>&1 &
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C118-fix2-natural "$K" sleep=120 gpuclients arm stages heaven superposition heaven superposition gpuclients refused events

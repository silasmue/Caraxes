#!/usr/bin/env bash
# KNOWN BUG #1 fix (dv2fix1 = deferred-v2 196b3f78): the C112 repro, present-probe beside three
# Heavens, then the natural condition (Steam and Edge start by themselves) with four runs.
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-dv2fix1-cap
while [ ! -x "$K" ]; do sleep 20; done
while pgrep -f "session2.sh C115-diag2" >/dev/null; do sleep 15; done
cd /mnt/vmstore/caraxes/runs-2026-09-30/win
nohup python3 watch-hang.py C116-fix1-repro 30 > C116-fix1-repro.watch.log 2>&1 &
bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C116-fix1-repro "$K" sleep=120 quiet present-bg=330 heaven present-status=330 quiet present-bg=331 heaven present-status=331 quiet present-bg=332 heaven present-status=332 gpuclients events
nohup python3 watch-hang.py C117-fix1-natural 30 > C117-fix1-natural.watch.log 2>&1 &
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C117-fix1-natural "$K" sleep=120 gpuclients heaven superposition heaven superposition gpuclients events

#!/usr/bin/env bash
# KNOWN BUG #1 with the live state dump (diag1 = final1 + device-ctl): present-probe beside
# three Heavens; watch-hang.py requests dumps at a 10 s flip gap.
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-diag1-cap
while [ ! -x "$K" ]; do sleep 20; done
while pgrep -f "session2.sh C113-lf23remap" >/dev/null; do sleep 15; done
cd /mnt/vmstore/caraxes/runs-2026-09-30/win
nohup python3 watch-hang.py C114-diag1-hang > C114-diag1-hang.watch.log 2>&1 &
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C114-diag1-hang "$K" sleep=120 quiet present-bg=330 heaven present-status=330 quiet present-bg=331 heaven present-status=331 quiet present-bg=332 heaven present-status=332 gpuclients events

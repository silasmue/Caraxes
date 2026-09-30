#!/usr/bin/env bash
# Heaven's crash on final1 (C101, C102: engine timeouts, then an access violation in d3d11.dll):
# D16 alone on lf24.
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session.sh C103-lf24d16-heaven /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-lf24-d16-cap heaven events

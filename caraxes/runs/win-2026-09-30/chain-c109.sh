#!/usr/bin/env bash
# The benchmark crash's baseline: lf23-remap (C91: Heaven 115.6 fps, Superposition 8758, before
# the full disk) now, under the same load as C107 (lf24: 2 of 4 crashed).
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C109-lf23remap-rate /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-lf23-remap-cap superposition heaven superposition heaven events

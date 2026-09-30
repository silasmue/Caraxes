#!/usr/bin/env bash
# Reproduce KNOWN BUG #1 on purpose: Steam and Edge stopped (C111: 4 of 4 passed so), a second
# D3D11 client (present-probe, windowed, 60 presents/s) running beside Heaven and Superposition.
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C112-final1-second-client /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-final1-cap sleep=120 quiet present-bg=330 heaven present-status=330 quiet present-bg=300 superposition present-status=300 quiet present-bg=331 heaven present-status=331 gpuclients events

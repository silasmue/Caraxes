#!/usr/bin/env bash
# KNOWN BUG #1's repro (C112: present-probe beside Heaven; the third Heaven stopped presenting
# 95 s in and hung) under lf23-remap, the evening binary.
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C113-lf23remap-second-client /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-lf23-remap-cap sleep=120 quiet present-bg=330 heaven present-status=330 quiet present-bg=331 heaven present-status=331 quiet present-bg=332 heaven present-status=332 gpuclients events

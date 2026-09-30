#!/usr/bin/env bash
# C110: Steam (4 steamwebhelper) and Edge started minutes after the logon, during the runs.
# Stopped before every run and listed after it; the four-run load on final1.
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C111-final1-quiet2 /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-final1-cap sleep=120 gpuclients quiet superposition gpuclients quiet heaven gpuclients quiet superposition gpuclients quiet heaven gpuclients events

#!/usr/bin/env bash
# Concurrent GPU clients (coordinator): Steam (steamwebhelper, D3D11) and the SystemInfo service
# came with the user's installs between C91 and C92. Listed, stopped for this session only, then
# the four-run load on final1.
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session2.sh C110-final1-quiet /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-final1-cap gpuclients quiet gpuclients superposition heaven superposition heaven gpuclients events

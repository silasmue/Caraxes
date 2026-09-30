#!/usr/bin/env bash
# The repair session once the host has room for an 8 GiB guest (preflight: MemAvailable >= 14 GiB).
until [ $(( $(awk '/MemAvailable/ {print $2}' /proc/meminfo) >> 20 )) -ge 15 ]; do sleep 30; done
echo "MemAvailable ok at $(date +%T)"
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 3600 /mnt/vmstore/caraxes/runs-2026-09-30/win/session.sh C100-lf24 /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-lf24-cap etwclean arm restart heaven reboot stages

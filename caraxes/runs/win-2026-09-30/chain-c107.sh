#!/usr/bin/env bash
# The intermittent benchmark crash (engine timeouts, then an access violation in the app;
# final1 2/4 Heaven, 2/3 Superposition): its rate on plain lf24.
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session.sh C107-lf24-rate /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-lf24-cap superposition heaven superposition heaven events

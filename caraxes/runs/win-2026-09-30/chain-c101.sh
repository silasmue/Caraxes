#!/usr/bin/env bash
# The Windows gate leg on final1 (train + D16), after C100.
while kill -0 1959224 2>/dev/null; do sleep 15; done
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 14400 /mnt/vmstore/caraxes/runs-2026-09-30/win/session.sh C101-final1 /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-final1-cap arm stages heaven superposition refused reboot

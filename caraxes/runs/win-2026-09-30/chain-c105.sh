#!/usr/bin/env bash
# The shader cache: every Heaven/Superposition run with the NVIDIA DXCache entry of an earlier
# run crashed (C101-C104), the first run of each passed (C100 Heaven, C101 Superposition).
# final1: caches cleared, Heaven twice (fresh, then from the cache), the records.
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session.sh C105-final1-cache /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-final1-cap cacheclear heaven heaven events

#!/usr/bin/env bash
# The Windows gate leg on train3 (c3e9bd5b = final1 without D16): Heaven crashed with D16 on
# final1 (C101, C102) and on lf24 (C103).
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session.sh C104-train3 /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-train3-cap arm stages heaven superposition refused events

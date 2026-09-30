#!/usr/bin/env bash
# The rest of the Windows gate leg on final1 after C105 (Heaven fresh and cached passed):
# Superposition fresh and from its cache, the D3D11 stages, the records.
exec bash /mnt/vmstore/caraxes/locks/vmlock.sh windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session.sh C106-final1-superpos /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-final1-cap arm superposition superposition stages refused events

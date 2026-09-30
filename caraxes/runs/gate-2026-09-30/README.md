# The four-guest gates of 2026-09-30

`legs-*.sh LEG` runs the Linux legs (nova, vm10, kick) under `vmlock.sh`, each with the same
flags as the lf16 gate. Results go to `/mnt/vmstore/caraxes/runs-2026-09-30/gate-*`; the
Windows leg is a session of caraxes/runs/win-2026-09-30.

- `legs-final1.sh`: final-2026-09-30 (424a43e8). nova timed out on the priority file.
- `legs-fix2.sh`: deferred-v2 (b6d01293). kick-smoke and nova passed. vm10 was refused its
  VRAM slot while another VM held it, then ran beside the census VM.

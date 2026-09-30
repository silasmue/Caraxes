# The four-guest gates of 2026-09-30

`legs-*.sh LEG` runs the Linux legs (nova, vm10, kick) under `vmlock.sh`, each with the same
flags as the lf16 gate. Results go to `/mnt/vmstore/caraxes/runs-2026-09-30/gate-*`; the
Windows leg is a session of caraxes/runs/win-2026-09-30.

- `legs-final1.sh`: final-2026-09-30 (424a43e8). nova timed out on the priority file.
- `legs-fix2.sh`: deferred-v2 (b6d01293). kick-smoke and nova passed. vm10 was refused its
  VRAM slot while another VM held it, then ran beside the census VM.
- `frame-check.sh SHOT.png [REF.png]`: the rendering check added to the final gate. It compares
  a SotTR guest screenshot with the evening's lfpool reference frame by colour statistics:
  - the share of pixels above 0.7 HSL saturation: 4.7 % in the reference, 74.5 % in the broken
    Linux frame of 2026-09-30;
  - the mean grey: 0.17 against 0.72.
  All six shots of the 2026-09-29 SotTR A/B runs pass, and the broken frame fails.

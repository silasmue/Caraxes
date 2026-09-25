<!-- SPDX-License-Identifier: MIT -->
# probe-hole610: the VA-hole placement, stopped at a host VA collision, no encode result

- Setup: binary `vfio-user-nvgpu-main-nvenc-74385c3` (the hole placement), 610 guest on vm2,
  host alone.
- What worked: with the guest's context page out of the shadow, the host's first fit was the
  hole, and the host context took it: `its context took the hole [0x12002d000, 0x12002e000)
  (first fit 0x12002d000 before its object, 0x12002e000 after)`. So the host maps a video
  context at 4 KiB alignment.
- What failed: the region split around the hole was refused (`0x1a`) at every sync, and the
  fallback's allocations then collided with the host context: three `VA Space alloc failed`
  lines at 14:54:49 (`host-kernel.txt`), `RangeLo 0x120020000` size 0x1e0000 and
  `RangeLo 0x120000000` size 0x30000. The watcher stopped the device.
- At the exit, 13 `Object handle is not valid` lines (mapping.c:302): the shadow worker
  still unmapped after the signal thread had freed the RM client. Fixed in the next commit
  (hostmem.rs `closed`).
- No Xid. The probe's ssh hung and was killed; vm2 went down cleanly.
- Superseded by the video layout (docs/caraxes/video-engines.md): the host context goes to a
  host-only area instead of the guest's VA.

<!-- SPDX-License-Identifier: MIT -->
# probe-merged610: stopped at a host VA collision, no encode result

- Setup: binary `vfio-user-nvgpu-main-nvenc-49af46d` (main dae79b11 merged), 610 guest on vm2.
  The root disk was full during the run: the empty record files were dropped, and
  `device-gpu.log` and `device-trace-excerpt.txt` were restored from the NAS copy.
- Sequence (`device-trace-excerpt.txt`):
  - chid 22 (NVENC0, bare) built on the shadow VAS of guest root 0x12729d000, bound to
    engineType 0x1b, first push forwarded;
  - the guest allocated NVC4B7; the guest's context [0x12002d000, 0x12002e000) left set A,
    then the host object 0x800000b7 was allocated;
  - 90 ms later the guest's 2 MiB page at 0x120400000 could not be mirrored: region object
    refused 0x1a, and the host kernel logged `VA Space alloc failed ... Size 0x200000 RangeLo
    0x120400000` twice (`host-kernel.txt`). The watcher stopped device pid 611479.
- No Xid; host BAR1 peaked at 120 MiB (`host-bar1.txt`).
- Cause, from the RM source (610.57.04):
  - the host kernel RM allocates the NVENC context itself at the object's allocation
    (`kflcnAllocContext`, kernel_falcon.c:279-298: 0x1000 bytes on TU106, the
    `GPU_GET_CONSTRUCTED_FALCON_INFO` capture of 2026-09-24) and maps it first fit
    (`kchannelMapEngineCtxBuf`, kernel_channel.c:3848-3870);
  - unmapping the guest's page left no VA hole: the shadow's 2 MiB region object over
    0x120000000 still reserves it. The first free VA was the unmapped region at 0x120400000,
    which the guest filled next;
  - a `GPU_PROMOTE_CTX` sent before the object cannot redirect this: from user space it is
    routed to GSP (flags 0x10244, no kernel-RM handler), so the kernel RM still allocates and
    promotes its own VA.
- Consequence (commit after 9cb51b39): a video object on a mirrored shadow channel is refused
  to the guest (`NV_ERR_NOT_SUPPORTED`, one line) unless `CARAXES_MIRROR_VIDEO_OBJECT=1`.

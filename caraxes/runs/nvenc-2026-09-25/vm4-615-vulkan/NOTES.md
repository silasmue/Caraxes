<!-- SPDX-License-Identifier: MIT -->
# vm4-615: Vulkan device creation fails, the guest kernel refuses TURING_COMPUTE_A

Session `sottr-2026-09-24/session-615-20260925-173139`, binary `vfio-user-nvgpu-v615-68838c54`.

- The failing call (`vkcube-rmlog.txt`, rmioctl-log.so in the guest):
  - `RMALLOC class=0xc5c0 parent=0xbeed0100 new=0xbeed90c0 status=0x57`
    (NV_ERR_OBJECT_NOT_FOUND). vkcube then aborts in `demo_create_device`.
  - Channel 0xbeed0100 (GR, chid 50) already holds 902d, c5b5, a140 and c597.
  - Its sibling 0xbeed0104 (GR, chid 51, same TSG 0xcef80001, ctxshares 0xcef00040/41 on
    VAS 0xbeef0300) got its c5c0 without error.
- The refusal is the guest kernel's own: no `GSP_RM_ALLOC` for it reaches the device
  (`device-trace-810s.txt`). The sibling's c5c0 arrives as rx seq 7509.
- The same sequence (c5c0 on 0xbeed0100 after the 3D objects) succeeded earlier in this
  session, at 144-179 s during Sunshine's encoder probes (`device-trace-144s.txt`).
- Candidate steps, in 615.71.09 `kgrobjConstruct_IMPL` -> `_kgrAlloc`
  (gr/kernel_graphics_object.c:173-228):
  1. `kgrmgrCtrlRouteKGRWithDevice`
  2. `kgrctxAllocCtxBuffers`
  3. `kgrctxMapCtxBuffers` (its VA-list lookups return OBJECT_NOT_FOUND)
  4. `kgrobjPromoteContext`
- Ruled out:
  - `chandesConstruct_IMPL` (fifo/channel_descendant.c): it only returns
    INVALID_OBJECT_PARENT or INVALID_CLASS.
  - Guest VRAM exhaustion: 1082 of 3072 MiB used.
  - A device refusal: no refused, floor or NO_MEMORY answer in the trace; the device's
    answers for both channels are identical.
- Not traceable in the guest: ResmanDebugLevel 0 prints nothing for this failure, and
  ftrace/kprobes refuse every RM function ("Could not probe notrace function
  kgrobjConstruct_IMPL [nvidia]").
- The wider state:
  - `vulkaninfo --summary` fails the same way (vkCreateDevice -> ERROR_DEVICE_LOST).
  - The CUDA NVENC probe, which passed on this VM at about 16:48, now hangs in context
    creation (90 s timeout, no output), after its c5c0 allocations succeeded.
- nvidia-drm: two `WARNING ... __nv_drm_handle_flip_event` (nvidia-drm-crtc.h:368) during
  one vkcube run (`guest-dmesg-vkcube.txt`), from the nvidia-modeset flip-event path.
- Option 3 (`vkcube-rmlog-params.txt`, the logger now dumps the channel allocation
  parameters): the next vkcube run failed earlier, already on the first TSG.
  - TSG 0xcef80000 with ctxshare 0xcef00000. GR channel 0xbeef0100: hObjectError
    0xbeef000c, hObjectBuffer 0, flags 0, hContextShare 0xcef00000, hVASpace 0,
    engineType 0x1, internalFlags 0.
  - The two CE channels 0xbeef0101/0102 (engineType 0xb/0xc) have hContextShare 0 and
    hVASpace 0xbeef0300.
  - Objects on 0xbeef0100: c597 OK, then c5c0 refused with 0x57. The GR allocs carry no
    parameters (size 0).
- Ordering: the same order (902d, c597, then c5c0 on the same channel) succeeded at 144 s,
  on these same handles. So the order alone is not the cause. There is no tool here to
  build a TSG and channel by hand; the reversed-order test was skipped.
- When it began (device trace, a Vulkan client's c5c0 after its c597 on the same channel):
  - The last success on both TSGs was at 179.7 s.
  - At 750, 775 and 810 s the first TSG still succeeds and the second TSG's refusal is new.
  - At 1998 s the first TSG fails too.
  - The failure spreads over time, which points at something in the guest kernel that
    accumulates. Not yet identified.

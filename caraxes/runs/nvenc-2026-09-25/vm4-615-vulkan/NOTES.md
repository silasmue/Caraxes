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

# SPDX-License-Identifier: MIT
probe-defer (binary vfio-user-nvgpu-nvenc-defer, commit 3279a532), 2026-09-25 12:14-12:17:
killed by the host guard at BAR1 exhaustion, no result.

- The game VM restarted at the same time with its BAR1 pool (64 MiB) plus 64 MiB of windows;
  vm2 added its own 64 MiB of windows beside the host desktop, and the host 256 MiB BAR1 ran out:
  129 host lines in one second (86 "dmaAllocMapping_GM107: can't alloc VA space for mapping",
  43 NO_MEMORY from the BAR1 reuse-mapping database), host-kernel-window.txt.
- The coordinator's guard stopped the device (pid 145550) at 12:16:43; the guest probe then saw
  cuInit fail with CUDA_ERROR_UNKNOWN (999). vm2 was brought down afterwards.
- No Xid. The NVENC mirror was not reached; the run says nothing about the deferred video object.

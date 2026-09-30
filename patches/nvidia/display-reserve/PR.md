<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Pull request to NVIDIA/open-gpu-kernel-modules

## Before submitting (checked on 2026-09-29, anonymous fetch)

- **Base.** The patch is against tag `615.71.09` (61dcc93), which is also `main` and the latest release tag upstream. It applies with `git am`; host compile check: kernel 7.2.7, gcc 16, `make modules -j4`, no new warnings.
- **Their rules** ([README.md "How to Contribute"](https://github.com/NVIDIA/open-gpu-kernel-modules#how-to-contribute), [CONTRIBUTING.md](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/main/CONTRIBUTING.md)):
  - Submit a pull request on GitHub. GitHub prompts you to accept NVIDIA's **Contributor License Agreement** when you open it.
  - No DCO or `Signed-off-by` is required. The commit carries one anyway.
  - No cosmetic changes. The patch changes no unrelated lines.
  - The repository is a snapshot per release; contributions are merged into the shared code base by hand.
  - For large refactoring they ask to be contacted in advance. This is a new feature, not a refactoring, but about 1,600 lines; opening a thread in their Discussions first is optional.
  - There is no published code style; the patch follows the surrounding code.
- **Attach** the bare-metal test pack results (see [README.md](README.md)) and fill in "Bare metal" below.
- **Title:** `nvidia, nvidia-modeset: hold a video memory reserve for scanout surfaces`

## PR body

### Problem

A scanout surface is physically contiguous video memory with no fallback: nvidia-drm falls back to system memory only for `NV_GEM_ALLOC_NO_SCANOUT` buffers. A process that sizes its pools to the free video memory can leave no room for the next scanout surface; a game sizing its pools to the Vulkan memory budget does this.

The display client's next allocation then fails. For Xwayland through the GBM backend the failing call is `DRM_IOCTL_NVIDIA_GEM_ALLOC_NVKMS_MEMORY`, logged as `Failed to allocate NVKMS memory for GEM object`. Xwayland loses its buffer, the display stops updating, and the game typically exits. We hit it with Shadow of the Tomb Raider under GNOME/Xwayland when the game switched to fullscreen: six scanout allocations within 9 s at a moment when the game held nearly all VRAM.

Reproducible on bare metal without a game: `scanout-oom.c` (attached) fills VRAM from CUDA and then allocates scanout buffers as the GBM backend does. The stock driver refuses the first one.

### Change

Off by default: nothing changes unless `nvidia-modeset` is loaded with `display_reserve_mib`.

- **NVKMS declares the need.** NVKMS knows what it scans out.
  - Per active head it counts five scanout surfaces and four cursors of the maximum cursor size. A head's surface is the larger of an XRGB8888 surface of its ViewPortIn and the largest surface bound to one of its layers.
  - It sends the sum and the largest surface size to RM with a new kernel-privileged control, `NV0073_CTRL_CMD_SYSTEM_SET_DISPLAY_RESERVE`.
  - It sends this after every modeset, and after a flip that binds a larger surface (grow-only until the next modeset). It declares 0 in `nvRmDestroyDisplays`.
  - `display_reserve_mib`: 0 = off (default), −1 = automatic as above, N = N MiB while a head is active.
- **RM holds the reserve as PMA page runs.**
  - One run per surface, with the page size RM gives such a surface, plus one rest run.
  - The runs are not RM objects and are charged to no cgroup. They leave PMA's free count, so processes (and the Vulkan budget) see less free memory.
- **Serve.** When PMA refuses a scanout allocation of a kernel client for lack of memory in `_vidmemPmaAllocate`, RM frees the smallest held run that covers it (alignment and range checked) and retries.
  - A scanout allocation of a kernel client means kernel privilege, `ATTR2_ISO_YES`, `PHYSICALITY_CONTIGUOUS`, not `NO_SCANOUT`.
- **Take back.** When such an allocation is freed while the reserve lacks a run of exactly its page size and page count, `vidmemPmaFree` hands its pages to the reserve instead of to PMA. They never become free in between, so a process sizing itself to free memory cannot take them first.
- **Refill and re-cut.**
  - A served allocation schedules a refill 1 s later; a short refill retries on a backoff from 1 s up to 30 s.
  - A new declaration re-cuts the reserve. Runs of a new run's shape move over; the others are kept, and still serve, until the new runs hold enough without them.
  - The refill runs as a work item with the API lock and without the GPU lock.
- **Lifecycle.**
  - The runs stay held over suspend and GC6/RTD3; no PMA call happens on those paths. NVKMS declares 0 before it allows GC6 anyway.
  - At unload the runs are freed from memmgr's pre-scheduling-disable handler, before the scrubber is destroyed. With MIG enabled the reserve holds nothing.
  - A generation counter keeps a refill from committing into a reserve stopped meanwhile.
- **Scrubbing is kept.**
  - Runs the reserve allocated are never mapped or written, so they are freed with `PMA_FREE_SKIP_SCRUB`.
  - Runs taken back hold a freed surface's contents and are freed without it, so PMA scrubs them as the original free would have.

### Why RM plus NVKMS

- The free-and-retry must be atomic with respect to other allocations. From NVKMS it would be two RM calls with the locks dropped in between.
- In `_vidmemPmaAllocate` it runs under the API lock, which allocations through the RM API take for writing unless `RmRoApiLock` enables read-only allocations. So no other RM-API allocation can take the freed run before the retry.
- UVM allocates from PMA directly and can. In that case the next covering run is freed, and the log says so.
- Only NVKMS knows the committed modes and bound surfaces, so NVKMS declares the size. A process cannot influence it.

### Limits

- **Served set.**
  - Served: NVKMS's own allocations and the scanout allocations nvidia-drm makes for its GEM objects, including dumb buffers, for any client of the render node, such as Xwayland and GBM clients.
  - Not served: a compositor's framebuffers allocated through its user-mode driver (for example mutter's), which run as user clients.
- **Only exact shapes are taken back.** A freed surface smaller than a run (for example a pitch-linear 1080p dumb buffer, 8 MiB against 10 MiB runs) goes back to PMA, and its run waits for the timer.
- **A request larger than every run is not served.** S follows a larger surface only after it is bound, and a new mode's framebuffers are allocated before the commit that declares their size.
- **A surface bound on several heads** (an X root pixmap) counts once per head.
- **Reporting.** The runs show as used video memory without a process.

### What NVIDIA would need to adapt

- **Control ID:** `0x730162` (message ID 0x62) is a local choice; control IDs are assigned centrally. An `NV2080` FB control may fit better than a display-object control.
- **Generated files:** `g_disp_objs_nvoc.c/h` (the exported-method entry, its trampoline, `numEntries`) and `g_mem_mgr_nvoc.h` (`pDisplayReserve`) are edited as shipped. They need regenerating from the NVOC sources.
- **OS layers:** `nvkms_display_reserve_mib()` is a new NVKMS OS-interface function, implemented for Linux only. Other NVKMS OS layers need it or a default.
- **Constant:** the factor five comes from one measurement, GNOME 46 on one head, which peaked at 4.67 surfaces above idle.
- **Reporting:** whether the runs should be reported as reserved rather than used.

### Testing

- **Host compile** (kernel 7.2.7, gcc 16): no new warnings.
- **Linux guest** (Ubuntu 24.04, kernel 6.8, GNOME 46, 615.71.09, 1 GiB of VRAM):
  - The declared size follows 1920x1080 → 1280x720 → off (44224 → 20224 → 0 KiB).
  - Under GNOME the size follows the bound framebuffers (52224 KiB for 10240 KiB surfaces).
  - With VRAM filled by a process that keeps taking every freed page, run-shaped scanout buffers were granted and taken back on free, two at a time.
  - With `display_reserve_mib=0` every one was refused.
  - This ran on a virtual GPU device, not bare metal.
- **Shadow of the Tomb Raider** at 1080p with 4 GiB: [fill in the v3 run: NVKMS failures, served/taken-back counts].
- **Bare metal:** [fill in from the test pack: GPU, kernel, steps passed; attach the result tarball].

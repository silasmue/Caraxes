<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Display reserve as a 615 driver patch (2026-09-29)

Scope: v1's display balloon ([display-balloon-2026-09-29.md](display-balloon-2026-09-29.md) "v1's display balloon (Q1)") as a patch to NVIDIA's open kernel modules 615.71.09. Covers the layer decision, the design, the build for game tests only, and the test plan. The dmem cap stays parked ([dmem-cap-2026-09-29.md](dmem-cap-2026-09-29.md)); its 5 GiB run showed no leak, the game takes 94-97 % of the free VRAM at every size.

Abbreviations:
- `T` = 615.71.09 `src/nvidia/`, `K` = `kernel-open/`, `M` = `src/nvidia-modeset/`.
- `v1` = `/home/silas/git/Leandro/guest-module/virtio_nvrm/`.

## Layer: RM (`nvidia.ko`), not NVKMS

- **What v1 did.** v1 sat at the RM entry every caller passes. It served NVKMS's LOCAL_USER VIDMEM ISO allocations refused with NV_ERR_NO_MEMORY, freeing chunks and retrying in the same call. While doing so it held process LOCAL_USER allocations (NV_ESC_RM_ALLOC, VID_HEAP_CONTROL ALLOC_SIZE, not UVM) back with a rwsem [v1 virtio_nvrm.c:2811-2835, 6264-6353].
- **NVKMS cannot give that guarantee.** From NVKMS, freeing a chunk and retrying are two RM calls [M kapi/src/nvkms-kapi.c:1563-1590, 763-895]. RM drops its locks between them, so a process allocation can take the freed room.
- **RM already serializes every allocation by default.**
  - The API lock is taken read-only only for APIs in `apiLockMask`. The default mask is `NVBIT(RS_API_CTRL)` [T src/kernel/core/system.c:385-421, 832].
  - An allocation therefore takes the API lock for writing [T src/kernel/rmapi/alloc_free.c:1687-1718]. VideoMemory's `RS_FLAGS_ACQUIRE_RO_API_LOCK_ON_ALLOC` [T src/kernel/rmapi/resource_list.h:558-568] only counts when the key `RmRoApiLock` enables read-only allocations [T interface/nvrm_registry.h:748-757].
  - So freeing reserved pages and retrying inside `vidmemConstruct` of the refused NVKMS allocation cannot lose the room to another API-locked allocation, process or kernel. No extra gate is needed.
  - Two paths are not API-locked and can take the freed room:
    - UVM allocates from PMA directly [T src/kernel/rmapi/nv_gpu_ops.c:5491-5520], which matches v1's exemption;
    - with `RmRoApiLock` allocations on, allocations run concurrently.
  - In both cases the loop releases the next run, as v1 did ("another NVKMS allocation took the room first") [v1 virtio_nvrm.c:6300-6318].
- **The refusal is visible in RM.**
  - `pmaAllocatePages` refuses inside `_vidmemPmaAllocate`, before the GPU lock is taken [T src/kernel/mem_mgr/video_mem.c:314-345, 707-735].
  - A contiguous request is never retried as non-contiguous [:325-338].
  - The release hooks exactly there, so a CPU-side NV_ERR_NO_MEMORY does not trigger it.
- **Kernel ISO/scanout requests are identifiable.** They are the requests with:
  - kernel privilege: NVKMS calls RM through `rm_kernel_rmapi_op`, whose entry points build a kernel secInfo [T src/kernel/rmapi/entry_points.c:124, 261-268]; `vidmemConstruct` already checks the same level for the reserved heap [video_mem.c:632];
  - ATTR2_ISO_YES and PHYSICALITY_CONTIGUOUS, without NO_SCANOUT, which is NVKMS's SCANOUT request [M kapi/src/nvkms-kapi.c:823-845];
  - VIDMEM and not VIRTUAL, which `vidmemConstruct` already requires [video_mem.c:619-622].
  - v1 used the same test [v1 nvrm_vram.c:152-180].
- **Why not nvidia-drm.** Same two-call problem as NVKMS, and kapi returns only NULL, not the RM status [M kapi/src/nvkms-kapi.c:1627-1671].

## Review of the first design (2026-09-29, tree f1f5c5d)

An independent review found five blocking defects. The coordinator decided to redesign instead of patching them.

- **Blocking.**
  1. No refill after a release. The timer was armed only after a short fill; v1 kicks 1 s after every give-back [v1 virtio_nvrm.c:6336].
  2. Any process controls the size. S was the largest ISO allocation of any client; LOCAL_USER is non-privileged, and `stdmemValidateParams` sets ISO for every PRIMARY, VIDEO and CURSOR allocation [T src/kernel/mem_mgr/standard_mem.c:104-116]. The ×5 amplifies it.
  3. It never shrank while full, so after unplugging a 4K display it stayed at 161 MiB.
  4. A re-cut freed the old reserve before allocating the new one.
  5. A deadlock on RTD3/GCx: `gpuStateUnload` runs with the GPU locks and without the API lock. The fill work item held the internal client's lock while waiting for the GPU lock in `vidmemConstruct`, and Stop's Free waited for that client lock. There was also no full-power check.
- **Must fix.**
  - `heldSize` mixed requested and page-rounded bytes.
  - The atomicity claim ignored UVM and `RmRoApiLock`.
  - Nothing checked that PMA is initialised or MIG is off.
  - Frees went through the scrubber after it was destroyed at full unload.
  - memacct charged the chunks to the cgroup of the process that ran `RmInitAdapter`.
- **Nits:**
  - `TMR_FLAG_USE_OS_TIMER`;
  - ignored return values;
  - one warning per chunk;
  - no validation of the size key;
  - a CPU-side NO_MEMORY also released;
  - a false comment and a redundant NULL check;
  - per-GPU state in a file-static array.
- **What the smoke taught** (kept below as history): mutter allocates its framebuffers from userspace GL. So sizing from allocations cannot be both automatic and safe. The display driver knows every head's committed mode and must be the one to declare the reserve.

## Review response (v2, patch commit c14f81c)

`P` = the patched tree (`/mnt/vmstore/caraxes/ogkm-615-display-reserve`, branch `display-reserve-v2`); `D` = `P/src/nvidia/src/kernel/gpu/mem_mgr/display_reserve.c`.

| # | Finding | v2 |
|---|---|---|
| 1 | No refill after a release | Every release schedules a refill 1 s later [D:671-676]; a short refill re-arms at 1 s doubling to 30 s [D:404-416] |
| 2 | Any process controls the size | The size is NVKMS's declaration from the committed modes [P/src/nvidia-modeset/src/nvkms-rm.c:5215-5260, nvkms-modeset.c:4364], through a kernel-only control [P/src/nvidia/generated/g_disp_objs_nvoc.c:4024-4040 (flags 0x0)]. No allocation is tracked any more |
| 3 | Never shrinks while full | Every declaration change queues a refill that re-cuts to the new need, smaller included [D:555-588, 320-363]. Heads off → need 0 → no runs |
| 4 | A re-cut freed the old reserve first | New runs are allocated first, swapped in under the mutex, then the old ones freed [D:320-363] |
| 5 | RTD3/GCx deadlock, no full-power check | No RM objects remain, so no client lock exists to wait on. Stop frees PMA runs directly under the GPU lock [D:514-548]. The refill takes no GPU lock while in PMA and returns unless `gpuIsGpuFullPower` [D:303] |
| 6 | `heldSize` mixed request and page sizes | `heldBytes` and `targetBytes` count PMA bytes (`pageCount × pageSize`) [D:192-196, 348-353, 388, 658] |
| 7 | Atomicity overstated | Stated for API-locked allocations only; UVM and `RmRoApiLock` can take freed pages, and the loop releases the next run [D:40-46; note "Layer"] |
| 8 | No PMA or MIG gate | No reserve without PMA [D:434]; refill and declaration refused with MIG enabled [D:303, 566] |
| 9 | Scrubber already destroyed at unload | Runs are freed with `PMA_FREE_SKIP_SCRUB` (never mapped or written; PMA hands out zeroed pages) [D:211-216; D:53-56] |
| 10 | memacct charge to the RmInitAdapter process | PMA-level runs, no `vidmemConstruct`, no charge [D:198-209] |
| nit | `TMR_FLAG_USE_OS_TIMER` | [D:449-450] |
| nit | Ignored return values | Init failure is logged [P/src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr.c:829-833]; timer and work-item errors are logged [D:234-274] |
| nit | Rate of the warning | One `LEVEL_WARNING` per release, not per run [D:671-674] |
| nit | Size key MB vs MiB, validation | Registry keys removed. `display_reserve_mib` is validated (< −1 rejected with a warning) [P/src/nvidia-modeset/src/nvkms-rm.c:5227-5232]; RM rejects a size without a surface [D:569-570] |
| nit | A CPU-side NO_MEMORY also released | The release sits right after `pmaAllocatePages` in `_vidmemPmaAllocate` [P/src/nvidia/src/kernel/mem_mgr/video_mem.c:320-333] |
| nit | False comment, redundant NULL check | The file is rewritten; the comments describe v2 |
| nit | Per-GPU state in a file-static array | `MemoryManager.pDisplayReserve` [P/src/nvidia/generated/g_mem_mgr_nvoc.h:745; D:459, 480] |

## v3 (patch commit b00029a)

`P` and `D` as above; D line numbers refer to v3. `B` = `caraxes/runs/dmem-2026-09-29/`.

### What drsv2-on-1 showed (SotTR 1080p, 4 GiB, `-1`)

Evidence: `B/drsv2-on-1/guest-monitor.txt:76-92`.

- **The reserve held its size until the fullscreen switch:** 44224 KiB declared, 52224 KiB in 6 runs at 16 s.
- **Six 10240 KiB scanout allocations were refused in 9 s** (282.5-291.8 s), from Xwayland and the game swapchain at the fullscreen switch.
  - The reserve served five of them with its 10 MiB runs.
  - For the sixth it released the 1 MiB rest run, which cannot cover 10 MiB. The allocation failed ("Failed to allocate NVKMS memory for GEM object"), and the game died.
- **Every refill in between failed with 0x51**, on the 1, 2, 4, 8, 16 and 30 s backoff.
  - The served buffers were freed, but the game took their pages from PMA before the timer came. The reserve was full again only at 338 s.
  - v1 took the room back when a served buffer was freed; v2 did so only on the timer.
- **Is 5·S enough?** The six allocations overlapped the frees, so the log does not show how many buffers were alive at once.
  - With take-back, a freed surface refills its run at once, so the concurrent peak is what matters, not the count of allocations.
  - The v3 run logs every take-back ("took back …, holds X of Y"), and X at its lowest gives the peak.

### Take back on free

- **Mark.** `_vidmemPmaAllocate` marks an allocation `bScanout` in its `PMA_ALLOC_INFO` when `displayReserveIsScanoutAllocation` matches it [P/src/nvidia/src/kernel/mem_mgr/video_mem.c:358-359; P/src/nvidia/inc/kernel/gpu/mem_mgr/heap_base.h:128-129; D:903-920]. The test is the one Serve uses.
- **Take.** `vidmemPmaFree` offers a marked allocation to the reserve when its last reference goes, before any PMA call [video_mem.c:403-409]. `displayReserveTake` [D:1013-1072]:
  - accepts it only if the reserve is started and lacks a run of the allocation's exact page size and page count;
  - accepts it only from the default regions and scrubbable (`DISPLAY_RESERVE_TAKE_FLAGS` [D:101-110]: not protected, localized, persistent, `NO_ZERO` or blacklist-off);
  - then records its base in that run, under the reserve mutex and with no PMA call.
  - The pages stay allocated, so no other client can take them in between.
- **Scrubbing is kept.**
  - PMA scrubs when pages are freed and reports allocations as zeroed without scrubbing them again [T src/kernel/gpu/mem_mgr/phys_mem_allocator/phys_mem_allocator.c:680-693, 1385-1463].
  - A run the reserve allocated was never written, so it is still freed with `PMA_FREE_SKIP_SCRUB`.
  - A taken run holds the freed surface's contents. It is marked `bTaken` and freed without the flag [D:252-258], so PMA scrubs it as the original free would have.
  - When Serve frees a taken run, the retry waits for the scrubber, which is PMA's own path for recently freed pages [phys_mem_allocator.c:1057-1093].
- **Lifecycle and the scrubber.**
  - The scrubber is destroyed in memmgr's pre-scheduling-disable handler [T src/kernel/gpu/mem_mgr/mem_mgr.c:529-541; arch/pascal/mem_mgr_scrub_gp100.c:83-121]. KernelFifo runs that handler at unload only, not for PRESERVING transitions [T src/kernel/gpu/fifo/arch/maxwell/kernel_fifo_gm107.c:237-259; mem_mgr.c:955-962 "fifo won't send a PreSchedulingDisable callback on StateUnload"].
  - v3 therefore frees every run in that handler, before the scrubber goes [P/src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr.c:536-537], and again in `memmgrStatePreUnload` at unload [:956-957].
  - Over suspend and GC6/RTD3 the runs stay held and Stop only stops [D:771-802]. They are PMA pages like any client allocation, and no PMA call happens on the GC6/RTD3 path.
- **Serve.**
  - It frees the smallest held run, old or new, that can hold the request with its PMA alignment and address range [D:286-360], and retries.
  - It never frees a run that cannot cover the request; v2 freed the 1 MiB rest for a 10 MiB request.
  - It prefers runs not taken back, which need no scrub wait. One warning per refusal [D:922-1011].
- **Limit: smaller surfaces.** A freed allocation is taken only into a run of its exact shape.
  - A smaller surface, for example a 1080p pitch-linear dumb buffer (8192 KiB against 10240 KiB runs), goes back to PMA. Its run then waits for the timer.
  - The observed failure is exactly run-shaped (10240 KiB).

### Second review (no blockers) and the v3 changes

| # | Finding | v3 |
|---|---|---|
| 1 | No refill when a served surface is freed (the drsv2-on-1 failure) | Take back on free, above [video_mem.c:403-409; D:1013-1072] |
| 2 | A re-cut under pressure dropped the reserve (partial cut swapped in, all old runs freed) | Held runs of a new run's shape move over; the rest stay as old runs, which Serve still uses. The fill frees an old run only while held + old − run ≥ target, so the total never drops below the target [D:260-284, 414-520, 522-589] |
| 3 | S undersized: XRGB8888 at ViewPortIn only (FP16, root pixmaps, larger surfaces); declared after the commit | S = max(formula, the largest `rmObjectSizeInBytes` bound to a layer of the head) [P/src/nvidia-modeset/src/nvkms-rm.c:5216-5247, 5288-5289]; declared after every modeset and, grow-only, after every flip [nvkms-modeset.c:4364; nvkms-flip.c:738; nvkms-rm.c:5299-5302]. Declaring before the commit is not possible: the client allocates the new mode's framebuffer before NVKMS sees the mode (limit below) |
| 4 | 2-4 KiB of runs on the stack | Cut and old runs on the heap [D:439, 455]; Serve, trim and Stop move one run at a time outside the mutex [D:286-360, 522-589, 806-836] |
| 5 | Refill vs Stop/Start race (RTD3/GC6 without the API lock) | `generation`, advanced by Stop under the mutex [D:797-799]; the refill snapshots it and commits a run or a cut only while it is unchanged, else frees what it got [D:441-447, 569-589] |
| 6 | MIG: held runs kept; 0 refused | The refill cuts to nothing with MIG enabled [D:622-626]; SetNeed accepts 0 always [D:870-884] |
| 7 | Served set overclaimed | Commit message and "Limits" below |
| nit | `display_reserve_mib` < −1 checked per modeset | Once at load [P/kernel-open/nvidia-modeset/nvidia-modeset-linux.c:2168-2173] |
| nit | Unbounded N; a size below one surface | RM rejects reserveSize > FB or < surfaceSize [D:870-879] |
| nit | Cache not updated when RM refuses | NVKMS caches the request, declares 0 on a refusal and warns once per value [nvkms-rm.c:5153-5201] |
| nit | Several warnings per refusal | One per refusal, from Serve [D:974-1008] |
| nit | Runs from which region | The default regions (no `PROTECTED_REGION` or `LOCALIZED` flag [D:234-250]); take-back only from these [D:101-110] |
| nit | `displayReserveDestroy` comment | States that Stop freed the runs, and asserts it [D:716-741] |
| nit | Lock statement for the control | Flags 0x0: API lock for writing and all GPU locks [T inc/kernel/rmapi/control.h:167-244; src/kernel/rmapi/control.c:928-995]; corrected under "The interface" |

### Limits (for the PR text)

- **Served set.**
  - Served: kernel-privileged, ISO, contiguous allocations without NO_SCANOUT, which are NVKMS's own and nvidia-drm's GEM allocations. These include dumb buffers and buffers from any render-node client (`DRM_RENDER_ALLOW`), such as Xwayland and GBM clients.
  - Not served: a compositor's framebuffers allocated by its user-mode driver (mutter's GL framebuffers). These run as user clients.
- **A request larger than every run is not served.** S follows the largest bound surface only after it is bound, so a new larger surface goes unserved before its first flip.
- **A growing mode under full VRAM.** The client allocates the new mode's framebuffers before the commit declares the new S; they are served only if a run covers them.
- **A surface bound on several heads** (an X root pixmap) counts once per head.
- **The runs show as used video memory without a process**, like any RM-internal allocation.

### For NVIDIA (cannot be settled locally)

- Control IDs are assigned centrally; 0x730162 and message ID 0x62 are local choices.
- The NVOC files (`g_disp_objs_nvoc.c/h`, `g_mem_mgr_nvoc.h`) are edited as shipped; NVIDIA regenerates them from sources that are not public.
- `nvkms_display_reserve_mib()` is a new NVKMS OS hook; non-Linux NVKMS OS layers need it too.
- The factor five comes from one measurement (GNOME 46, one head, 4.67 surfaces).
- An NV2080 FB control could replace the display-object control.
- The runs could be reported as reserved instead of used.

## Design (redesign)

As built in v2. Where v3 differs (take-back, Serve, re-cut, S, lifecycle, scrubbing), the section "v3" above applies.

### NVKMS declares the need

- **Why NVKMS.** Every mode reaches the hardware through `nvSetDispModeEvo`: clients (kapi/nvidia-drm, the X driver), console restore and head shut-down all go through it [M src/nvkms-modeset.c:4208; nvkms.c:2246, 3672; nvkms-console-restore.c:393, 680, 960].
  - After a successful commit, the api-head state holds the committed timings, including ViewPortIn, the scanned source size [M include/nvkms-types.h:1339-1365, 1497, 1975-1984]. `nvApiHeadIsActive` says which heads are lit [:2566-2571].
  - A process cannot influence the need: it follows the committed configuration.
- **What it declares, per display (subdevice).**
  - Each active head contributes 5 × S plus four cursors. S = align(4W, 64) × align(H, 128) rounded to 64 KiB, W × H = ViewPortIn (v1's formula and measured 4.67 × S peak [v1 nvrm_vram.c:40-95]). A cursor is the device's maximum, `cursorHal->caps.maxSize`² × 4 bytes [M src/nvkms.c:1477].
  - The largest S of all heads is the chunk size.
  - With no active head the need is 0.
- **When.**
  - After every successful commit, before GC6 is allowed for an all-off configuration [M src/nvkms-modeset.c:4339-4368]. This covers modeset, hotplug (the client's modeset), mode changes and heads off.
  - The need is 0 in `nvRmDestroyDisplays`, before the display-common object is freed [M src/nvkms-rm.c:2194].
  - It is sent only when it changed, like `nvRmSetGc6Allowed`'s cached state [M src/nvkms-rm.c:5095-5123].
- **Knob.** The `nvidia-modeset` parameter `display_reserve_mib`, declared like the module's other parameters [K nvidia-modeset/nvidia-modeset-linux.c:83-140]:
  - 0 = off, the default: nothing is declared and RM holds nothing;
  - −1 = automatic, as above;
  - N = N MiB while any head is active, cut into chunks of the largest S.

### The interface

- **`NV0073_CTRL_CMD_SYSTEM_SET_DISPLAY_RESERVE`** (0x730162), on NVKMS's display-common object. Params: `subDeviceInstance`, `reserveSize` (bytes; 0 releases), `surfaceSize` (bytes of the largest scanout surface).
- **Precedent:** `NV0073_CTRL_CMD_SYSTEM_ALLOCATE_DISPLAY_BANDWIDTH` is the display driver declaring a display resource to RM, handled in kernel RM [T ../common/sdk/nvidia/inc/ctrl/ctrl0073/ctrl0073system.h:1725-1765; src/kernel/gpu/disp/disp_common_kern_ctrl_minimal.c:108-160].
- **Privilege and locks.**
  - Flags `RMCTRL_FLAGS_KERNEL_PRIVILEGED` (0x0): only kernel clients may call it [T inc/kernel/rmapi/control.h:168-175]. Kernel RM handles it; it is not routed to GSP.
  - Flags 0x0: it runs with the API lock for writing and all GPU locks [T inc/kernel/rmapi/control.h:167-244; src/kernel/rmapi/control.c:928-995]. It only records the need and queues the fill.
- **NVOC wiring.** The exported-method table is sorted by methodId and searched binarily [T src/libraries/nvoc/src/runtime.c:307-335]. The entry goes after 0x730161, and `numEntries` grows by one [T generated/g_disp_objs_nvoc.c:3449, 6118-6122]. The NVOC sources are not public; the generated files are edited as the tree ships them.

### RM holds the reserve at PMA level

- **Runs of PMA pages.**
  - The reserve is a list of runs from `pmaAllocatePages` with `PMA_ALLOCATE_PINNED | PMA_ALLOCATE_CONTIGUOUS | PMA_ALLOCATE_FORCE_ALIGNMENT` [T inc/kernel/gpu/mem_mgr/phys_mem_allocator/phys_mem_allocator.h:86-104, 351].
  - A full run has the page size `memmgrDeterminePageSize` gives a pitch surface of S, which is also what `_vidmemPmaAllocate` gets for NVKMS's request [T src/kernel/mem_mgr/standard_mem.c:236-247; src/kernel/gpu/mem_mgr/mem_mgr.c:1808-1870; video_mem.c:190-199]. It covers S, so a freed run is exactly the shape the refused surface needs.
  - The rest run takes the remainder at its own page size. Everything is counted in PMA bytes.
- **No RM objects.** There is no internal client, no nested `vidmemConstruct` and no memacct charge (memacct charges in `vidmemConstruct` only [T video_mem.c:626-630]).
  - The pages leave PMA's free count, so HEAP_FREE, NVML free and the Vulkan budget drop by the reserve, which the 5 GiB run shows games follow [dmem-cap-2026-09-29.md "Status"].
- **Free without scrubbing.**
  - Runs are freed with `PMA_FREE_SKIP_SCRUB` [:107, 410]. A run is never mapped or written, and with scrub-on-free PMA hands out zeroed pages (`PMA_ALLOCATE_RESULT_IS_ZERO` [T src/kernel/gpu/mem_mgr/phys_mem_allocator/phys_mem_allocator.c:686-693]).
  - This also keeps the scrubber out of unload: KernelFifo's pre-scheduling-disable callback destroys it before `memmgrStatePreUnload` [T src/kernel/gpu/mem_mgr/mem_mgr.c:529-541, 556, 696; arch/pascal/mem_mgr_scrub_gp100.c:95-121].
- **Release.**
  - Where: in `_vidmemPmaAllocate`, when `pmaAllocatePages` refuses a request with NV_ERR_NO_MEMORY that is kernel-privileged (TLS call context, as `stdmemValidateParams` reads it [T standard_mem.c:40-47]), ISO, CONTIGUOUS and not NO_SCANOUT.
  - What: runs covering the request's PMA bytes go back to PMA, v1's pick (rest first if it alone covers, else a full run), and `pmaAllocatePages` is retried. This repeats while runs are held.
  - Atomicity is as stated in the layer section.
- **Refill.** A release schedules a refill 1 s later [v1 virtio_nvrm.c:6336]; a short fill retries on a backoff of 1 s doubling to 30 s.
  - The timer is an OS timer event created with `TMR_FLAG_USE_OS_TIMER` [T generated/g_objtmr_nvoc.h:84-90]. Its callback runs as a work item under the GPU group lock [T arch/nvalloc/unix/src/osapi.c:3536-3568] and only queues the fill.
  - The fill runs as a work item with the API lock for writing and without the GPU lock [T inc/kernel/os/workitem.h; mem_mapper.c:336-342], because PMA must not be entered under the GPU lock [video_mem.c:717-733].
- **Re-cut and shrink.**
  - When the declared need or surface size changes, the fill allocates the new shape first, then frees the old runs, then allocates what the new shape still lacks. A smaller need frees the surplus.
- **Locks.**
  - A mutex guards the run list, and no PMA call is made while it is held.
  - The fill re-checks after each allocation that the reserve is still started, and frees what it got otherwise.
  - No RM API object is involved, so the client-lock cycle of finding 5 cannot occur.
- **Lifecycle.**
  - The state lives in MemoryManager (`pDisplayReserve`, a member added to the generated class; NVOC zero-fills objects [T generated/g_mem_mgr_nvoc.c:1663-1670]).
  - It is created in `memmgrStateInitLocked`, next to memacct [mem_mgr.c:826], and removed in `memmgrStateDestroy`.
  - Start in `memmgrStatePostLoad` (load, resume, GC6/RTD3 exit). Stop in `memmgrStatePreUnload` (unload, suspend, GC6/RTD3 entry): it frees every run under the GPU lock without taking the API lock.
  - The fill does nothing unless the GPU is at full power (`gpuIsGpuFullPower`), PMA is initialised and MIG is off.
  - NVKMS keeps GC6 blocked while any head is active and declares 0 before it allows it [M src/nvkms-modeset.c:4364-4368].
- **Logging.** State messages are LEVEL_INFO, shown with `NVreg_RmMsg=display_reserve.c`. One LEVEL_WARNING per served refusal; LEVEL_ERROR on failures.

## History: where RM can learn the display size (first design)

The reserve has to follow the modes of all heads through mode changes and hotplug. The sources were checked in this order:

- **IMP (`NVC372_CTRL_CMD_IS_MODE_POSSIBLE`).** Not usable.
  - NVKMS calls it for configurations it validates, not only the one it commits [M src/nvkms-evo3.c:3360-3370; src/nvkms-evo4.c:2446].
  - Kernel RM routes it to the physical RM (flags `0x48` = `RMCTRL_FLAGS_ROUTE_TO_PHYSICAL | RMCTRL_FLAGS_NON_PRIVILEGED`) and keeps nothing [T generated/g_disp_objs_nvoc.c:850-860; inc/kernel/rmapi/control.h:205, 230].
- **Display bandwidth (`NV0073_CTRL_CMD_SYSTEM_ALLOCATE_DISPLAY_BANDWIDTH`).** Not usable: NVKMS calls it only for SOC display and returns before the call on a discrete GPU [M src/nvkms-evo.c:7575-7600].
- **Kernel RM head state.** Not usable: `KernelHead` holds vblank and RG-line interrupt state only, no raster [T generated/g_kernel_head_nvoc.h:137-174].
- **The display surfaces themselves.** Chosen.
  - A display surface here is video memory allocated with ATTR2_ISO_YES and without NO_SCANOUT, by any client. `vidmemConstruct` sees every one.
  - The largest live one is one scanout surface of the largest mode in use. It grows when a larger mode or display appears. No new interface is needed, and no display means no reserve.
  - It does not see the head count: 5 × S covers one head's measured peak [v1 nvrm_vram.c:65-95]. Concurrent transitions on several heads are not covered beyond that; v1 measured one head, and `RmDisplayReserveSizeMB` sets a larger size.
  - **Any client, measured.** Tracing every allocation during a GNOME start (RmMsg `stdmemDumpInputAllocParams`, `B/drsv-debug/trace-journal.txt`) showed:
    - Mutter allocates its four 1080p framebuffers itself: GL owner 0x474C0000, user privilege, 0x870000, ISO, physicality default.
    - NVKMS (owner 0xDCBA) allocates only small ISO surfaces at idle, 0x4200 and 0x2100, 64 KiB after rounding. It allocates and frees the 0x4200 one every 5 ms during its start.
    - A kernel-only source therefore sized the first build for 64 KiB (2 MiB; the operator's smoke: +2 MiB). NVKMS's own 0x870000 SCANOUT buffers (Xwayland's GBM buffers, the refused one [display-balloon-2026-09-29.md]) appear only once an X client runs.
  - **Size in PMA.** The tracked size is the one `vidmemConstruct` returns, rounded to the page: 0x870000 becomes 10 MiB (five 2 MiB pages). A chunk of it frees exactly the pages a refused 1080p buffer needs. R at 1080p is 5 × 10 MiB + 1 MiB = 51 MiB (v1: 44 MiB of requested bytes).
  - **Any client can raise the size.** A process can make the automatic reserve larger with a large ISO allocation. The memory then sits in the reserve instead of in that process; `RmDisplayReserveSizeMB` pins the size. Open for upstream review.

## History: first design

- **Chunks.** They are NV01_MEMORY_LOCAL_USER allocations on the MemoryManager's internal client [T src/kernel/gpu/mem_mgr/mem_mgr.c:1226-1250, 807, 991].
  - The params are NVKMS's SCANOUT request, pitch layout as in v1 [v1 nvrm_vram.c:182-214]: type PRIMARY, CONTIGUOUS, ISO, GPU_CACHEABLE_NO, forced 4 KiB alignment, FORCE_MEM_GROWS_UP.
  - The owner is `HEAP_OWNER_RM_CLIENT_GENERIC`, as other RM-internal LOCAL_USER allocations use [T src/kernel/gpu/mem_mgr/ce_utils.c:186].
  - A freed chunk leaves the hole the refused buffer fits: same PMA page size, 2 MiB at ≥ 2 MiB [mem_mgr.c:1781-1786], and contiguous.
  - The internal client's own allocations are neither served nor tracked.
  - Only a kernel client's scanout request is served (kernel privilege, ISO, CONTIGUOUS, not NO_SCANOUT), as in v1, where "a process never" takes room [v1 nvrm_vram.c:152-160]. A process's display surfaces (mutter's) size the reserve but are not served.
- **Size and shape (v1).**
  - S is the largest live display surface. R is 5 × S plus four 256×256×4 cursors, rounded up to MiB (v1's measured 4.67 × S peak, rounded to five [v1 nvrm_vram.c:65-95]), or `RmDisplayReserveSizeMB`.
  - Full chunks are one S each, one rest chunk takes the remainder, and there are at most 64 chunks; a larger R grows the chunk [v1 nvrm_vram.c:115-132].
  - Each fill cuts the reserve for the largest display surface at that time, as v1 applied changed display dimensions at refill [v1 virtio_nvrm.c:176, 6209-6225].
    - A surface larger than the current cut starts a fill at once. A smaller largest surface applies at the next fill: start, served free or backoff.
    - The churn of small surfaces therefore cannot re-cut the reserve in a loop, which the first build did (64 KiB ↔ 0 every 5 ms during NVKMS's start).
  - The fill work item holds the API lock for writing, so freeing and re-allocating is atomic against other allocations.
- **Release (v1).** On a refusal the rest chunk goes first when it covers the request alone, else a full chunk; this repeats until the request is granted or nothing is held [v1 nvrm_vram.c:134-149; virtio_nvrm.c:6170-6193, 6300-6318].
- **Refill (v1).**
  - At once when a surface the reserve made room for is freed [v1 virtio_nvrm.c:5951-5956, 6327-6337, 6353-6372]. Every live display allocation is tracked with that mark in a map keyed by its Memory object (a `containers/map.h` MAP, as memacct keeps its groups [T src/kernel/mem_mgr/memacct.c:37-50]), so v1's limit of 16 remembered buffers falls away.
  - Otherwise on a backoff of 1 s, doubling to 30 s, while the reserve stays below its size [v1 virtio_nvrm.c:5916-5921, 6252-6262].
  - Fill runs in an RM work item with the API lock (read-write) and no GPU lock [T inc/kernel/os/workitem.h; mem_mapper.c:336-342]. A PMA allocation must not start under the GPU lock [video_mem.c:717-733].
  - The backoff is an OS timer event [T src/kernel/gpu/timer/timer.c; kern_fsp.c:101]. Its callback runs as a work item with the GPU group lock [T arch/nvalloc/unix/src/osapi.c:3536-3568], so it only queues the fill.
- **The reserve counts as used.** The chunks come out of PMA, so HEAP_FREE, NVML free and the Vulkan budget drop by R, and processes get that much less. The 5 GiB run shows the game sizes itself to free VRAM [dmem-cap-2026-09-29.md "Status"].
- **Lifecycle, per GPU.**
  - Configured in `memmgrStateInitLocked`, next to `memacctInitGpuInfo` [mem_mgr.c:826].
  - Filled from `memmgrStatePostLoad`: at probe and after every resume.
  - Emptied in `memmgrStatePreUnload`: before every suspend and at teardown. Its content is irrelevant, so it is not saved; no timer fires while suspended. The allocation map survives suspend with the allocations.
  - Removed in `memmgrStateDestroy` [mem_mgr.c:989].
  - Each GPU (each MemoryManager) has its own reserve.
- **Configuration: registry keys**, as `RmMemacctMode` is [T interface/nvrm_registry.h:3553-3561; arch/nvalloc/unix/src/os.c:2171]. They are set with `NVreg_RegistryDwords` and are off by default.
  - `RmDisplayReserve`: 0 = off, 1 = on.
  - `RmDisplayReserveSizeMB`: 0 = automatic; N = N MiB, still cut into S chunks.
  - No mode key: the automatic source is sound.

## Patch (v3)

- **File.** `patches/nvidia/0001-nvidia-nvidia-modeset-hold-a-video-memory-reserve-fo.patch` (sha256 2d9c13e5…), against 615.71.09. It is one commit (P branch `display-reserve-v3`, b00029a), author Silas Müller, MIT like the files it touches. It applies with `git apply` and `patch -p1`. v2 stays on branch `display-reserve-v2` (c14f81c).
- **What it changes.**
  - RM:
    - `src/nvidia/src/kernel/gpu/mem_mgr/display_reserve.c` and its header (new, 1072 + 55 lines);
    - Serve and the `bScanout` mark in `_vidmemPmaAllocate`, and take-back in `vidmemPmaFree`;
    - `bScanout` in `PMA_ALLOC_INFO` (`heap_base.h`);
    - five lifecycle hooks in `mem_mgr.c`;
    - the control handler in `disp_common_kern_ctrl_minimal.c`;
    - `srcs.mk`.
  - Interface: `NV0073_CTRL_CMD_SYSTEM_SET_DISPLAY_RESERVE` in `ctrl0073system.h`.
  - Generated NVOC files:
    - `g_disp_objs_nvoc.c/h`: the export entry, its trampoline and prototype; the table's index comments renumber after the insertion;
    - `g_mem_mgr_nvoc.h`: `pDisplayReserve`.
  - NVKMS:
    - `nvRmUpdateDisplayReserve` in `nvkms-rm.c`, called from `nvSetDispModeEvo` and, grow-only, from `nvFlipEvo`;
    - the release in `nvRmDestroyDisplays`;
    - the cached request in `NVDispEvoRec`.
  - `nvidia-modeset`: the `display_reserve_mib` parameter, validated at load, and its getter, in both copies of the os-interface header.
- **Host compile check** (v3 as for v2: 7.2.7, gcc 16, `make -j4 modules`):
  - no new compiler warning (three in NVKMS exist in the pristine tree);
  - `modinfo nvidia-modeset.ko` lists `display_reserve_mib`;
  - `nvidia.ko` has the new symbols.
  - Nothing was loaded on the host.

## Build and use (game tests only)

- **The stock modules stay the default.** Nothing goes to `updates/`, `dkms/` or `modprobe.d`, and depmod is not run.
- **Build** (`crates/vfio-user-nvgpu/guest/display-reserve/build.sh`, in the guest).
  - It checks the source tarball's sha256, applies the patch and builds the whole 615.71.09 tree against the running kernel with the guest's gcc.
  - It installs only `nvidia.ko` and `nvidia-modeset.ko` to `/opt/caraxes/display-reserve-v2/<kernel>/`, with a MANIFEST of sources, sha256s and `nvidia.ko`'s version line.
  - The first design's build under `/opt/caraxes/display-reserve/` stays for the runs made with it.
- **Load** (`load.sh DISPLAY_RESERVE_MIB [NVreg_…]`).
  - It unloads the NVIDIA modules and loads the dependencies of both modules (`ecc`, `video`), since `insmod` does not.
  - It `insmod`s both modules with their `modprobe.d` options, the extra `nvidia` parameters and `display_reserve_mib`.
  - It fails unless `/proc/driver/nvidia/version` shows the build's version line and `nvidia_modeset` has the value asked for.
  - The installed `nvidia-drm` and `nvidia-uvm` bind on top; the kernel's modversion check refuses them if an exported symbol differed.
- **Run scripts.**
  - `display-reserve-prepare.sh` builds in the guest and copies `drsv3-smoke/`.
  - `drsv3-smoke/smoke.sh` loads the build and walks the connector through 1920x1080, 1280x720 and off as DRM master (`modeset.c`, raw ioctls). It then starts GNOME, fills VRAM from CUDA and keeps grabbing whatever becomes free (`fill.py … grab`, as the game does). Finally it asks for dumb buffers: one of the run shape (1920x1152, 8640 KiB in 2 MiB pages), two of them at once, and a smaller one (1920x1080).
  - `drsv2-smoke/` is v2's smoke.
- **Expected** (declared = 5·S + 1 MiB of cursors; PMA = runs rounded to their page size):
  - 1920x1080: S = 0x870000, runs 5 × 10 MiB + 1 MiB = 51 MiB;
  - 1280x720: S = 0x3C0000, 5 × 4 MiB + 1 MiB = 21 MiB;
  - off: 0.

## Logging

- **Levels follow RM's convention for state.** State messages (enabled, cut, holds, next try) are `NV_PRINTF(LEVEL_INFO)`, as memacct's [T src/kernel/mem_mgr/memacct.c]. A release build does not print them: the runtime threshold is warnings [K nvidia/os-interface.c:843-862].
- **A served refusal is `LEVEL_WARNING`** and is printed, one line per refusal, as is a refusal that no held run covers; failures are `LEVEL_ERROR`.
- **A take-back is `LEVEL_INFO`:** "took back a freed scanout allocation of N KiB, holds X of Y KiB".
- **Tests turn the state lines on with NVIDIA's RmMsg filter**, not with a code change: `NVreg_RmMsg=display_reserve.c` [K nvidia/nv-reg.h `RmMsg`; T src/kernel/diagnostics/nvlog_printf.c:953-974]. `load.sh` passes extra `nvidia` parameters.

## Smoke results (v3; 2026-09-29, desktop-b-dmem, 1 GiB, stock 6.8.0-136, GNOME)

Evidence: `B/drsv3-smoke/smoke-1.txt` (`-1`), `smoke-2-mib0.txt` (`0`); the build is `B/drsv3-build/prepare.txt` (patch sha256 2d9c13e5…, version line "… Tue Sep 29 09:12:12 UTC 2026"). The filler keeps grabbing freed VRAM every 10 ms.

| | `display_reserve_mib=-1` | `0` (same build) |
|---|---|---|
| 1920x1080 | declared 44224 KiB for 8640 KiB, 6 runs, 52224 KiB in PMA; used 60 MiB | used 9 MiB |
| 1280x720 | re-cut: 20224 KiB declared, 21504 KiB in PMA; used 34 MiB | used 13 MiB |
| Off | 0 runs; used 13 MiB | used 13 MiB |
| GNOME (+30 s) | declared 52224 KiB for 10240 KiB surfaces (the bound framebuffers are larger than the formula's 8640 KiB), 52224 KiB in PMA; used 199 MiB | used 148 MiB |
| 1920x1152 dumb buffer (run shape) | served ("released 10240 KiB … holds 41984 of 52224"); refills refused (0x51, the filler); on the free, "took back … holds 52224 of 52224 KiB" | ENOMEM |
| Two at once | two serves, then two take-backs, full again without a refill | ENOMEM ×2 |
| 1920x1080 dumb buffer (8192 KiB) | served; not taken back (smaller than a run); the filler took the pages; full again at 214 s, after the filler exited | ENOMEM |

- **Take-back wins where v2's refill lost:** every refill in this smoke failed with 0x51, and the reserve still came back to full at each free of a run-shaped buffer.
- **The limit shows as documented:** a surface smaller than a run returns its pages to PMA.

## Smoke results (v2; 2026-09-29, desktop-b-dmem, 1 GiB, stock 6.8.0-136, GNOME)

Evidence: `B/drsv2-smoke/smoke-2.txt` (`-1`), `smoke-3-mib0.txt` (`0`); the build is `B/drsv2-build/prepare.txt` (MANIFEST version line "… (root@caraxes-desktop-b) Tue Sep 29 08:27:07 UTC 2026").

| | `display_reserve_mib=-1` | `display_reserve_mib=0` (same build) |
|---|---|---|
| 1920x1080 (DRM master) | declared 44224 KiB (5 × 8640 + 1024), 6 runs, 52224 KiB in PMA; used 60 MiB | nothing declared; used 9 MiB |
| 1280x720 | re-cut to 20224 KiB declared, 21504 KiB in PMA; used 34 MiB | used 13 MiB |
| Head off | declared 0, 0 runs; used 13 MiB | used 13 MiB |
| GNOME up (+30 s) | 1080p size again (52224 KiB); used 199 MiB | used 148 MiB |
| CUDA fills free VRAM (4-5 MiB left), then a 1920x1080 dumb buffer (8100 KiB, 8192 KiB in 2 MiB pages) | granted twice: "released 10240 KiB for a refused scanout allocation of 8192 KiB, holds 41984 of 52224 KiB"; refill refused at +1 s and +2 s (0x51, backoff), full again 4 s after the release, once the buffer was freed | refused twice, ENOMEM |
| Filler gone | used 199 MiB | used 148 MiB |

- **The numbers match the expected sizes** under "Build and use", within 1 MiB of v1's display balloon (51 MiB).
- **The backoff timer ran** (1 s, then 2 s) while the dumb buffer held the released run.
- **`load.sh` fix found in the first v2 smoke (`smoke-1.txt`):** `nvidia-modeset.ko` depends on `video` (`acpi_video_register_backlight`), which `insmod` does not load. The stock `nvidia_modeset` then came up, nothing was declared and the dumb buffers were refused (the same numbers as the `0` column). `load.sh` now loads the dependencies of both modules, and `smoke.sh` stops when `load.sh` fails.
- **Not yet exercised:** suspend/resume, GC6/RTD3, unload with a held reserve under load, several GPUs.

## Smoke results of the first design (history; 2026-09-29, desktop-b-dmem, 1 GiB, stock 6.8.0-136, GNOME)

Evidence: `B/drsv-smoke3/`; the build is `B/drsv-build2/prepare.txt`.

| | `RmDisplayReserve=1` | `RmDisplayReserve=0` (same build) | stock (operator's smoke) |
|---|---|---|---|
| VRAM used, GNOME up | 199 MiB | 148 MiB | 148 MiB |
| Reserve | 51 MiB for a 10240 KiB surface (5 × 10240 + 1024 KiB), held within 4 s of the first framebuffer | — | — |
| CUDA fills free VRAM (3-5 MiB left), then a 1920×1080 dumb buffer (NVKMS SCANOUT, 8100 KiB) | granted twice: "released 10240 KiB for a refused display allocation of 8100 KiB", refilled to 51 MiB 3 s after each buffer was freed | refused twice, ENOMEM | — |

- **`load.sh` fixes found in the operator's first smoke:**
  - It now loads the modules `nvidia.ko` depends on (`ecc`), since `insmod` does not.
  - It fails unless `/proc/driver/nvidia/version` shows the build's version line from the MANIFEST (build host and time). In the first smoke the failed `insmod` let the following `modprobe` load the stock module unnoticed.
- **`build.sh` dropped its modversion comparison.** `modprobe --dump-modversions` fails on these modules, so it compared two empty lists. The patch changes no exported symbol, and the kernel refuses a mismatch at load.
- **Not yet exercised:** suspend/resume, the backoff timer (every fill so far succeeded at once), several GPUs.

## Test plan (v3)

1. **Smoke** (`display-reserve-prepare.sh drsv3-build`, then `drsv3-smoke/smoke.sh`; control with `MIB=0`).
   - The modes are as in v2. At 720p the rest run is reused, and the 10 MiB runs are freed once the 4 MiB runs are held.
   - With the filler grabbing freed VRAM:
     - a 1920x1152 dumb buffer is served, and on its free "took back … 10240 KiB, holds 52224 of 52224 KiB";
     - two such buffers at once: two releases, two take-backs;
     - 1920x1080 (8192 KiB) is served and not taken back (limit), so its run waits for the filler to exit.
   - `MIB=0` refuses every dumb buffer.
   - Passed on 2026-09-29 (see "Smoke results (v3)").
2. **SotTR 1080p at 4 GiB**, `-1` and a control with 0. Pass: no NVKMS failure, no Xid, a results file. The lowest "holds X" of the take-back lines gives the concurrent peak (is 5·S enough).
3. **Not yet exercised:** suspend/resume with a held reserve, GC6/RTD3, unload under load, several GPUs, MIG. These are for the bare-metal test pack.


1. **Smoke** (`display-reserve-prepare.sh`, then `drsv2-smoke/smoke.sh` in the 1 GiB guest).
   - The reserve holds the declared size at 1080p, re-cuts to the 720p size, and is released when the head goes off.
   - GNOME gives the 1080p size again.
   - With VRAM filled, the dumb buffer is granted with one run released and refilled 1 s later; `display_reserve_mib=0` refuses it.
   - Passed on 2026-09-29 (see "Smoke results (v2)").
2. **SotTR 1080p at 4 GiB** with `display_reserve_mib=-1`, and a control with 0: `SOTTR_FB_MIB=4096 DMEM_CAP=off SOTTR_DISPLAY_RESERVE_MIB=-1 sottr-bench-6g.sh OUT BINARY`.

## Test plan (first design, history)

Linux B's disposable copy `desktop-b-dmem` on the stock 6.8.0-136 kernel (`DMEM_KERNEL=stock`; no dmem is needed), 4 GiB, SotTR 1080p, `sottr-bench-6g.sh` with `DMEM_CAP=off`. Commands: [caraxes/runs/dmem-2026-09-29/README.md](../../caraxes/runs/dmem-2026-09-29/README.md) "Display reserve".

1. **Build once:** `display-reserve-prepare.sh`. It logs the source and patch sha256 and the `MANIFEST`.
2. **`RmDisplayReserve=1`.** Pass:
   - 0 NVKMS failures (`nvkms_fail=0`) and 0 Xids;
   - a results file;
   - the Moonlight picture changing until the end.
   - Before the game, `vram` is higher by the reserve (51 MiB at 1080p).
   - A refusal the reserve absorbed shows as `display reserve: released …` in `guest-monitor.txt`.
3. **Control, `RmDisplayReserve=0`** (the same build, reserve off): the refusal must reproduce, as with the stock driver in `cap-on-1` and on 2026-09-28.
4. **Record** FPS against the stock driver's runs.

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

## Design (redesign)

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
  - It runs with the GPU lock and the read-only API lock. It only records the need and queues the fill.
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

## Patch (redesign)

- **File.** `patches/nvidia/0001-nvidia-nvidia-modeset-hold-a-video-memory-reserve-fo.patch`, against 615.71.09: one commit, author Silas Müller, MIT like the files it touches. It applies with `git apply` and `patch -p1`.
- **What it changes.**
  - RM: `src/nvidia/src/kernel/gpu/mem_mgr/display_reserve.c` and its header (new, 679 + 47 lines); the release in `_vidmemPmaAllocate`; four lifecycle hooks in `mem_mgr.c`; the control handler in `disp_common_kern_ctrl_minimal.c`; `srcs.mk`.
  - Interface: `NV0073_CTRL_CMD_SYSTEM_SET_DISPLAY_RESERVE` in `ctrl0073system.h`.
  - Generated NVOC files:
    - `g_disp_objs_nvoc.c/h`: the export entry, its trampoline and prototype; the table's index comments renumber after the insertion;
    - `g_mem_mgr_nvoc.h`: `pDisplayReserve`.
  - NVKMS: `nvRmUpdateDisplayReserve` in `nvkms-rm.c`, called from `nvSetDispModeEvo`; the release in `nvRmDestroyDisplays`; the cached declaration in `NVDispEvoRec`.
  - `nvidia-modeset`: the `display_reserve_mib` parameter and its getter, in both copies of the os-interface header.
- **Host compile check** (7.2.7, gcc 16, `make -j4 modules`):
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
  - It unloads the NVIDIA modules and loads `nvidia.ko`'s dependencies.
  - It `insmod`s both modules with their `modprobe.d` options, the extra `nvidia` parameters and `display_reserve_mib`.
  - It fails unless `/proc/driver/nvidia/version` shows the build's version line and `nvidia_modeset` has the value asked for.
  - The installed `nvidia-drm` and `nvidia-uvm` bind on top; the kernel's modversion check refuses them if an exported symbol differed.
- **Run scripts.**
  - `display-reserve-prepare.sh` builds in the guest and copies `drsv2-smoke/`.
  - `drsv2-smoke/smoke.sh` loads the build, walks the connector through 1920x1080, 1280x720 and off as DRM master (`modeset.c`, raw ioctls), starts GNOME, then fills VRAM from CUDA and asks for 1080p dumb buffers.
- **Expected** (declared = 5·S + 1 MiB of cursors; PMA = runs rounded to their page size):
  - 1920x1080: S = 0x870000, runs 5 × 10 MiB + 1 MiB = 51 MiB;
  - 1280x720: S = 0x3C0000, 5 × 4 MiB + 1 MiB = 21 MiB;
  - off: 0.

## Logging

- **Levels follow RM's convention for state.** State messages (enabled, cut, holds, next try) are `NV_PRINTF(LEVEL_INFO)`, as memacct's [T src/kernel/mem_mgr/memacct.c]. A release build does not print them: the runtime threshold is warnings [K nvidia/os-interface.c:843-862].
- **A served refusal is `LEVEL_WARNING`** and is printed; failures are `LEVEL_ERROR`.
- **Tests turn the state lines on with NVIDIA's RmMsg filter**, not with a code change: `NVreg_RmMsg=display_reserve.c` [K nvidia/nv-reg.h `RmMsg`; T src/kernel/diagnostics/nvlog_printf.c:953-974]. `load.sh` passes extra `nvidia` parameters.

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

## Test plan (redesign)

1. **Smoke** (`display-reserve-prepare.sh`, then `drsv2-smoke/smoke.sh` in the 1 GiB guest).
   - The reserve holds the declared size at 1080p, re-cuts to the 720p size, and is released when the head goes off.
   - GNOME gives the 1080p size again.
   - With VRAM filled, the dumb buffer is granted with one run released and refilled 1 s later; `display_reserve_mib=0` refuses it.
2. **SotTR 1080p at 4 GiB** with `display_reserve_mib=-1`, and a control with 0. The bench gets `SOTTR_DISPLAY_RESERVE_MIB` once the first design's runs are done.

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

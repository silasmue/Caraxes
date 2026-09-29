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
  - So freeing chunks and retrying inside `vidmemConstruct` of the refused NVKMS allocation is atomic against every other RM allocation, process or kernel. No extra gate is needed.
  - UVM allocates from PMA without the API lock, which matches v1's exemption.
  - With `RmRoApiLock` allocations on, a concurrent allocation can win the freed room. The loop then frees the next chunk, as v1 did ("another NVKMS allocation took the room first") [v1 virtio_nvrm.c:6300-6318].
- **The refusal is visible in RM.**
  - `_vidmemPmaAllocate` returns NV_ERR_NO_MEMORY before the GPU lock is taken [T src/kernel/mem_mgr/video_mem.c:314-345, 707-735].
  - A contiguous request is never retried as non-contiguous [:325-338].
- **Kernel ISO/scanout requests are identifiable.** They are the requests with:
  - kernel privilege: NVKMS calls RM through `rm_kernel_rmapi_op`, whose entry points build a kernel secInfo [T src/kernel/rmapi/entry_points.c:124, 261-268]; `vidmemConstruct` already checks the same level for the reserved heap [video_mem.c:632];
  - ATTR2_ISO_YES and PHYSICALITY_CONTIGUOUS, without NO_SCANOUT, which is NVKMS's SCANOUT request [M kapi/src/nvkms-kapi.c:823-845];
  - VIDMEM and not VIRTUAL, which `vidmemConstruct` already requires [video_mem.c:619-622].
  - v1 used the same test [v1 nvrm_vram.c:152-180].
- **Why not nvidia-drm.** Same two-call problem as NVKMS, and kapi returns only NULL, not the RM status [M kapi/src/nvkms-kapi.c:1627-1671].

## Where RM can learn the display size

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

## Design

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

## Patch

- `patches/nvidia/0001-nvidia-add-a-display-reserve-for-kernel-display-allo.patch`, against 615.71.09, one commit, author Silas Müller, MIT like the files it touches. It changes:
  - `src/nvidia/src/kernel/mem_mgr/display_reserve.c` and its header, which are new;
  - the two registry keys;
  - four hooks in `mem_mgr.c`;
  - the retry and the allocation tracking in `video_mem.c`;
  - `srcs.mk`.
- **Host compile check** (615.71.09 against 7.2.7, gcc 16, `make -j4 modules`):
  - no compiler warning;
  - objtool reports the same "naked return" notes for the new functions as for every other function of the RM core (11672 before, 11684 after);
  - the `displayReserve*` symbols are in `nvidia.ko`.
  - No module was loaded on the host.

## Build and use (game tests only)

- **The stock modules stay the default.** The stock 615.71.09 modules stay the default in every image; nothing goes to `updates/`, `dkms/` or `modprobe.d`, and depmod is not run.
- **Build** (`crates/vfio-user-nvgpu/guest/display-reserve/build.sh`, in the guest).
  - It checks the source tarball's sha256, applies the patch to the full 615.71.09 source and builds the RM core (`src/nvidia`) with the guest's gcc.
  - It puts the RM core into a copy of the DKMS source (`/usr/src/nvidia-615.71.09`, whose `nvidia/nv-kernel.o_binary` is the prebuilt RM core) and builds there against the running kernel.
  - It installs only `nvidia.ko` to `/opt/caraxes/display-reserve/<kernel>/`, with a MANIFEST of sources, sha256s and the version line.
  - The patch touches only the RM core, so `nvidia-modeset`, `nvidia-drm` and `nvidia-uvm` stay the installed modules.
- **Load** (`load.sh RmDisplayReserve=1 [NVreg_…]`).
  - It unloads the NVIDIA modules, loads the modules `nvidia.ko` depends on, then `insmod`s the patched `nvidia.ko` with the options `modprobe.d` gives `nvidia`, adding the value to `NVreg_RegistryDwords` and appending the extra parameters.
  - It fails unless the loaded module reports the build's version line.
  - The bench's own `modprobe` of the other modules then finds `nvidia` loaded. The next boot is stock again.

## Logging

- **Levels follow RM's convention for state.** State messages (enabled, cut, holds, next try) are `NV_PRINTF(LEVEL_INFO)`, as memacct's [T src/kernel/mem_mgr/memacct.c]. A release build does not print them: the runtime threshold is warnings [K nvidia/os-interface.c:843-862].
- **A served refusal is `LEVEL_WARNING`** and is printed; failures are `LEVEL_ERROR`.
- **Tests turn the state lines on with NVIDIA's RmMsg filter**, not with a code change: `NVreg_RmMsg=display_reserve.c` [K nvidia/nv-reg.h `RmMsg`; T src/kernel/diagnostics/nvlog_printf.c:953-974]. `load.sh` passes extra parameters and the bench sets this one.

## Smoke results (2026-09-29, desktop-b-dmem, 1 GiB, stock 6.8.0-136, GNOME)

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

## Test plan

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

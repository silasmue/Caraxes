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

## Design

- **Chunks.** They are NV01_MEMORY_LOCAL_USER allocations on the MemoryManager's internal client [T src/kernel/gpu/mem_mgr/mem_mgr.c:1226-1250, 807, 991].
  - The params are NVKMS's SCANOUT request, pitch layout as in v1 [v1 nvrm_vram.c:182-214]: type PRIMARY, CONTIGUOUS, ISO, GPU_CACHEABLE_NO, forced 4 KiB alignment, FORCE_MEM_GROWS_UP.
  - The owner is `HEAP_OWNER_RM_CLIENT_GENERIC`, as other RM-internal LOCAL_USER allocations use [T src/kernel/gpu/mem_mgr/ce_utils.c:186].
  - A freed chunk leaves the hole the refused buffer fits: same PMA page size, 2 MiB at ≥ 2 MiB [mem_mgr.c:1781-1786], and contiguous.
  - The internal client's own allocations are never served.
- **Shape (v1).**
  - Full chunks are one scanout buffer S = align(4W, 64) × align(H, 128), rounded to 64 KiB. One rest chunk takes the remainder (the cursors). There are at most 64 chunks; a larger R grows the chunk [v1 nvrm_vram.c:40-132].
  - On a refusal the rest chunk goes first when it covers the request alone, else a full chunk; this repeats until the request is granted or nothing is held [v1 nvrm_vram.c:134-149; virtio_nvrm.c:6170-6193, 6300-6318].
- **Refill (v1).**
  - At once when a buffer the reserve made room for is freed; up to 16 are remembered [v1 virtio_nvrm.c:5951-5956, 6327-6337, 6353-6372].
  - Otherwise on a backoff of 1 s, doubling to 30 s, while the reserve stays below its size [v1 virtio_nvrm.c:5916-5921, 6252-6262].
  - Fill runs in an RM work item with the API lock (read-write) and no GPU lock [T inc/kernel/os/workitem.h; mem_mapper.c:336-342]. A PMA allocation must not start under the GPU lock [video_mem.c:717-733].
  - The backoff is an OS timer event [T src/kernel/gpu/timer/timer.c; kern_fsp.c:101]. Its callback runs as a work item with the GPU group lock [T arch/nvalloc/unix/src/osapi.c:3536-3568], so it only queues the fill.
- **The reserve counts as used.** The chunks come out of PMA, so HEAP_FREE, NVML free and the Vulkan budget drop by R, and processes get that much less. The 5 GiB run shows the game sizes itself to free VRAM [dmem-cap-2026-09-29.md "Status"].
- **Lifecycle, per GPU.**
  - Configured in `memmgrStateInitLocked`, next to `memacctInitGpuInfo` [mem_mgr.c:826].
  - Filled from `memmgrStatePostLoad`: at probe and after every resume.
  - Emptied in `memmgrStatePreUnload`: before every suspend and at teardown. Its content is irrelevant, so it is not saved; no timer fires while suspended.
  - Removed in `memmgrStateDestroy` [mem_mgr.c:989].
  - Each GPU (each MemoryManager) has its own reserve.
- **Configuration: registry keys**, as `RmMemacctMode` is [T interface/nvrm_registry.h:3553-3561; arch/nvalloc/unix/src/os.c:2171]. They are set with `NVreg_RegistryDwords` and are off by default.
  - `RmDisplayReserve`: the display mode the reserve is cut for, width 15:0 and height 31:16; 0 = off.
  - `RmDisplayReserveSizeMB`: 0 = automatic, 5 × S + four 256×256×4 cursors, rounded up to MiB (v1's measured 4.67 × S peak, rounded to five [v1 nvrm_vram.c:65-95]); N = N MiB.
  - RM does not know the display modes: NVKMS sets them through the display channel. So the mode is configured, and the reserve does not follow mode changes or hotplug.
  - v1 was also cut for a configured display size (`vdisplay_width`/`height`) [v1 virtio_nvrm.c:5988-6004].

## Build and use (game tests only)

- The stock 615.71.09 modules stay the default in every image.
- The patched `nv-kernel.o` is built in the guest from the patched source. The DKMS source tree with it builds all modules, which go to `/opt/caraxes/display-reserve/<kernel>/`. That is not `updates/` or `dkms/`, and there is no depmod, so a normal boot never loads them.
- The game bench unloads the stock modules and `insmod`s the patched set with `NVreg_RegistryDwords="RmDisplayReserve=…"`. The next boot is stock again.

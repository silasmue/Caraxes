<!-- SPDX-License-Identifier: MIT -->
# Compression isolation in vGPU and MIG, and the Caraxes slice (2026-09-28)

Research for the compression decision (D16 → real compression per VM slice, user 2026-09-28). `K` = `vendor/open-gpu-kernel-modules/src/nvidia/src/kernel/`, `SDK` = `.../src/common/sdk/nvidia/inc/`, `SW` = `.../src/common/inc/swref/published/`; tree 610.57.04 (e4a5faa). [V] verified in source, [I] inferred.

## Bottom line
- Neither vGPU nor MIG shares a comptag allocator between tenants. From Turing on the kernel RM only supports "1:1" or "raw" comptag modes, where a page's comptag line follows from its **physical address**; compression isolation follows from physical-memory isolation.
- The host RTX 2070 reports `1TO1_COMPTAG=1` (caraxes/reference/rtx2070/kmd-controls-2026-09-26.md:42, host RM 615.71.09).
- On the shadow path the host RM writes COMPTAGLINE itself from the physical address; the guest never supplies one. The D16 hazard (a raw guest COMPTAGLINE indexing the global compbit store) exists only where the hardware walks PTEs the device or guest wrote directly.
- ZBC (the fast-clear value table) is the only explicitly shared and partitioned piece.

## Turing base
- [V] `kmemsysAllocComprResources_KERNEL` refuses unless 1:1 or raw mode (K/gpu/mem_sys/kern_mem_sys.c:442), refuses a fixed comptag offset (:445-447), needs scrub-on-free (:449-465) and CE virtual mode in 1:1 (:466-469), and only marks `ctagId=-1` "physically based" plus `COMPR_REQUIRED` (:474-475).
- [V] At map time a compressible kind with `ctagId` 0/-1 sets `bPhysBasedComptags` (K/gpu/mem_mgr/mem_mgr.c:2598-2610); `compTagLineMin = memmgrDetermineComptag(physAddr)` (K/gpu/mmu/kern_gmmu.c:951-967) = `1 + SPA[35:16]` (K/gpu/mem_mgr/arch/turing/mem_mgr_tu102.c:504-511, SW/turing/tu102/dev_fb_addendum.h:27): one comptag line per 64 KiB, owned by whoever owns the page. Raw mode, SR-IOV and the vGPU plugin write `compTagLineMin=1` ("HW handles compression tags", kern_gmmu.c:958-963).
- [V] Stale compression state is cleared by the scrubber: CE in virtual mode (K/gpu/mem_mgr/mem_scrub.c:152-155) through a whole-FB alias of kind 0x09 (K/gpu/mem_mgr/arch/maxwell/mem_utils_gm107.c:975-989, mem_mgr_tu102.c:567-577); 1:1 forces it (K/gpu/mem_mgr/arch/maxwell/mem_mgr_gm107.c:1553-1557).

## vGPU
- [V] The guest learns compression from the static info: FB region `supportCompressed` (K/gpu/mem_mgr/mem_mgr_vgpu.c:87), the memsys static config incl. `bOneToOneComptagLineAllocation` and `bDisableCompbitBacking` (K/gpu/mem_sys/kern_mem_sys_ctrl.c:1651-1666, K/vgpu/rpcstructurecopy.c:3846-3850), cap `VGPU_1TO1_COMPTAG_ENABLED`, "always TRUE when SR-IOV is enabled" (SDK/ctrl/ctrla080.h:291-293, 369-371).
- [V] Legacy (Turing) vGPU guests allocate no HW resources (`SKIP_RESOURCE_ALLOC`, K/gpu/mem_mgr/mem_mgr.c:1434-1443); allocations and mappings go to the host by RPC (K/mem_mgr/video_mem.c:971-1010, K/mem_mgr/hw_resources.c:136-157, K/mem_mgr/virtual_mem.c:1537), so the host RM writes the PTEs.
- [V] SR-IOV: "For VF, HW does 1 to 1 FB-comptag mapping" (K/gpu/mem_mgr/arch/maxwell/virt_mem_allocator_gm107.c:1892-1896).
- [V] ZBC: a VF's ZBC index range comes from static info (K/gpu/mem_sys/zbc_api.c:27-43); legacy vGPU forwards ZBC sets to the host (K/vgpu/rpc.c:4582-4592, 5762-5860) with references counted there (mem_mgr_gm107.c:277-279); "If PF and all VFs report false, ZBC table can be flushed" (SDK/ctrl/ctrl2080/ctrl2080internal.h:2062-2066).
- Web: Turing vGPU profiles >= 12 GB reserve FB "for the higher compression overhead" (T4-16Q: 28 MB), so compression is on in vGPU ([known limitations](https://docs.nvidia.com/vgpu/20.0/grid-vgpu-release-notes-microsoft-windows-server/known-product-limitations.html)); each vGPU has an exclusive fixed FB ([user guide](https://docs.nvidia.com/vgpu/latest/grid-vgpu-user-guide/index.html)).

## MIG
- [V] Each swizzId is a contiguous physical range (K/gpu/mem_sys/kern_mem_sys.c:591-626) with its own heap (K/gpu/mig_mgr/kernel_mig_manager.c:3487); ZBC refcounts per GR engine (mem_mgr_gm107.c:293-335); MIG chips use raw mode (K/gpu/mem_mgr/arch/ampere/mem_mgr_ga100.c:472-478); the Hopper+ PTE has no comptag line (K/gpu/mmu/arch/hopper/kern_gmmu_fmt_gh10x.c:176).
- Web: crossbar ports, L2 banks, memory controllers and DRAM buses are assigned per instance ([MIG introduction](https://docs.nvidia.com/datacenter/tesla/mig-user-guide/latest/introduction.html)).

## Uncompressed behind a compressed view (NVIDIA's own fallbacks)
- [V] A compressible kind override over uncompressed backing is silently downgraded (`memmgrGetUncompressedKind`, K/mem_mgr/virtual_mem.c:1370-1393; same in 615.71.09); per-PTE fallback with COMPTAGLINE 0 in regions without compression or when tags run out (virt_mem_allocator_gm107.c:1853-1881, 1926-1938, 2332-2342). The driver sees NV_OK and keeps believing the surface is compressed.
- [V] Downgrade table = `memmgrGetUncompressedKind_TU102(..., bReleaseReacquire=FALSE)` (mem_mgr_tu102.c:303-351): 0x08/0x09→0x06, 0x0A→0x02, 0x0B→0x01, 0x0C→0x03, 0x0D→0x04, 0x0E→0x05.

## Real compression per Caraxes slice (the chosen route)
- [V pieces / I whole] Allocate the reservation with `COMPR_REQUIRED` (host marks it physically based, kern_mem_sys.c:474-475), pass the guest's compressible kind as override (not downgraded, the backing is compressible); the host RM writes `1 + SPA[35:16]` of the reservation's own pages. This matches legacy Turing vGPU (host-written PTEs into the VM's exclusive slice). `NV01_MEMORY_LOCAL_PHYSICAL` with a compressed kind is allowed in 1:1 (K/mem_mgr/phys_mem.c:109-118).
- Caveats:
  - 4 KiB compressible leaves: the host widens them to the physical page size, which is 2 MiB here (virt_mem_allocator_gm107.c:740-752); downgrade them.
  - Keep guest→host PA alignment at 64 KiB.
  - Clear the compbits before the reservation is freed (write through a compressible mapping, or prove that scrub-on-free covers it), or the desktop inherits stale comptags.
  - BAR1, dma-buf and capture readers see raw compressed bytes and need kind-matched maps.
  - Windows compbit-store eviction cannot reach the host store.
- Guest view:
  - report `bOneToOneComptagLineAllocation=1` (else compressed allocations fail, kern_mem_sys.c:442). This switches the guest scrubber to CE virtual mode over a whole-FB kind-0x09 alias, which the device must shadow;
  - answer `FB_GET_COMPBIT_STORE_INFO` synthetically in guest-FB terms;
  - `CBC_EVICTION` → NoCompTags; refuse `COMPBITCOPY`; the comptag L2 flush (K/gpu/mem_mgr/dma.c:694-697) is a no-op.
- ZBC: on Turing the ZBC kinds are exactly the compressible kinds (K/gpu/mem_mgr/arch/turing/mem_mgr_tu102_base.c:52-53). With real compression the guest's fast clears store a ZBC index in the compbits and the hardware consults the host's global table (shared with the desktop, mem_mgr_gm107.c:516-523), so guest ZBC entries must reach the host table as legacy vGPU does, with refcounting.
- Guards:
  - an explicit kind allowlist in the device (0x00-0x06 as is, 0x08-0x0E only on compressed slice pages, refuse 0x07, 0x0F, >0x0F);
  - zero COMPTAGLINE only for VIDEO-aperture raw PTEs (PEER/SYS use those bits for the address, SW/turing/tu102/dev_mmu.h:117);
  - an audit with `NV0080_CTRL_CMD_DMA_GET_PTE_INFO` (K/gpu/mmu/kern_gmmu.c:2633-2658): every comptagLine must lie in `1 + SPA[35:16]` of this VM's reservation.
- Risk: the Vulkan driver might still 2D-fill the depth buffer in its compressed path. The 2D post-mortem (c3a226e0) shows it.

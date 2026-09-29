<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Memory model

Scope: how guest FB and guest sysmem addresses reach the physical GPU (reservation, guest-RAM registration, swiotlb alias, shadow page tables, gathers, region pools) and the rules that keep it fail closed.

## Facts

### Decisions

| D | Rule | Evidence |
|---|---|---|
| D11 | the device reserves a fixed-address, contiguous VRAM range sized to the profile, and refuses it when the host would be left without enough free VRAM | [docs/caraxes/README.md §Decisions] |
| D13 | guest DMA addresses equal host IOVAs through a temporary guest-side module (`caraxes_dma_range`), the only guest-side patch | [docs/caraxes/README.md §Decisions] |
| D16 | no guest comptag line reaches the host: the guest is told there is no compression, the device forces every vidmem PTE uncompressed with COMPTAGLINE 0, an audit sweeps the tables after each gate | [docs/caraxes/README.md §Decisions] |
| D17 | shadow page tables in the device are the DMA route and the guest stays bare metal; the only configuration deviation is `uvm_page_table_location=vid` plus the matching `RmInstLoc` fields | [docs/caraxes/README.md §Decisions] |
| D18 | target model: VRAM slice + sparse, demand-driven sysmem registration (2 MiB regions, refcounted, released with hysteresis) + validated shadow tables, with the guest writing GPAs and the device translating | [docs/caraxes/README.md §Decisions] |

- `uvm_page_table_location` accepts `vid` and `sys` and otherwise keeps UVM's default aperture [615 tree: kernel-open/nvidia-uvm/uvm_mmu.c:72-95].
- The kernel RM applies the `RM_INST_LOC` fields `_PDE`, `_BAR_PDE`, `_PTE` and `_BAR_PTE` from `pGpu->instLocOverrides` to its page-table memdescs [615 tree: src/nvidia/src/kernel/gpu/mmu/kern_gmmu.c:521-533].
- Shadowing costs no VRAM beyond the guest's own tables, 8 bytes per PTE: 2 MiB per GiB mapped at 4 KiB pages, 128 KiB at 64 KiB, 4 KiB at 2 MiB [docs/caraxes/README.md §Decisions D17].

### FB: the VRAM slice

- Guest FB address X is host VRAM address X, and every FB address the guest can write lies in the reservation `[B, E)`, with `E` the host FB heap end aligned down to 2 MiB and `B = E - S` [docs/caraxes/memory-model.md §1].
- The guest is told the card's full 8 GiB as FB address space so that its fake WPR lands above `E` in the host's reserved top 366 MiB, where no engine ever uses it [docs/caraxes/memory-model.md §1; run: caraxes/runs/e5b-2026-09-23].
- The top 2 MiB of the reservation is a device-owned carve-out holding the BAR2 and BAR1 root pages [docs/caraxes/memory-model.md §1].
- Slot n lies n profile sizes below the top, `[E-(n+1)S, E-nS)`, one slot per VM (`CARAXES_RESERVE_SLOT`) [docs/caraxes/memory-model.md §3; docs/caraxes/parallel.md §1].
- The reservation is `NV01_MEMORY_LOCAL_USER`, FIXED_ADDRESS, CONTIGUOUS, 2 MiB pages, never `NVOS32_ATTR_COMPR`, with no floating fallback on NO_MEMORY [docs/caraxes/memory-model.md §4, §6].
- The FB store counts every access outside `[B, E)`, and every recorded run counted 0 [docs/caraxes/memory-model.md §1; docs/caraxes/parallel.md §3a].

### Sysmem: guest RAM as host OS descriptors

- Each descriptor is one `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR` over the device's own mapping of the guest memfd, pinned with `FOLL_LONGTERM` [docs/caraxes/memory-model.md §2; 615 tree: kernel-open/nvidia/os-mlock.c:216-233].
- One descriptor of at most 4 GiB - 4 KiB is one `dma_map_sg` call and gets one contiguous IOVA range aligned to its power-of-two size, allocated top-down [docs/caraxes/sysmem-iova.md §2; run: caraxes/runs/sysmem-iova-2026-09-23].
- Registration refuses when pinning would leave the host under 4 GiB of `MemAvailable`, and checks each chunk's IOVA with `GET_SURFACE_PHYS_ATTR` (aperture SYSMEM, base + offset everywhere) [docs/caraxes/memory-model.md §2].
- The first descriptor that does not fit below 4 GiB switches the host GPU to 64-bit IOVAs until the host reboots, after which chunks lie near 2^47 [docs/caraxes/sysmem-iova.md §1.4; docs/caraxes/memory-model.md §2].
- Guest RAM is registered as IOVA = GPA + one offset: 1 GiB chunks from the highest GPA down plus a filler descriptor over the [3, 4) GiB hole, restarting below any host allocation in the way (`CARAXES_LINEAR_IOVA`, default 1) [docs/caraxes/mapping.md §5a; run: caraxes/runs/mapping-2026-09-24/d1-isolation].
- The linear layout closed the chunk-crossing hole: 128 refused crossing leaves in async_streams before, 0 after [docs/caraxes/mapping.md §5; docs/caraxes/parallel.md §8.2].
- `GuestMemory::translate` is the device's only path to guest RAM, and an address outside the table is refused and counted, never passed through [docs/caraxes/memory-model.md §2].

### The swiotlb alias

- A GSP-client guest refuses to initialize when its one-page sysmem flush buffer has a DMA address at or above 2^40 [615 tree: src/nvidia/src/kernel/gpu/mem_sys/arch/maxwell/kern_mem_sys_gm107.c:228-272; docs/caraxes/memory-model.md §2].
- With 64-bit IOVAs the launcher aliases the guest's swiotlb pool, rounded out to 2 MiB, at IOVA 0xf000000000 in the guest's table only (`--dma-table-alias`, `$W/dma-ranges.guest`) [docs/caraxes/memory-model.md §2].
- The 2 MiB rounding margins around the pool are ordinary guest pages that also carry alias IOVAs [docs/caraxes/memory-model.md §2 "The alias in the device"; run: caraxes/runs/xid31-hostreg-2026-09-24].
- The device reads the guest's table at every reset and accepts an alias only if the table describes exactly the registered guest RAM and each alias names only registered GPAs (`DmaTable::aliases_of`), then maps alias leaves from the registered descriptor at the page's offset [docs/caraxes/memory-model.md §2 "The alias in the device"].
- The physical GPU never sees an alias IOVA, and 960 GiB lies above every host 32-bit IOVA and below the 64-bit ones [docs/caraxes/memory-model.md §2].
- The Xid 31 that the missing alias caused is catalogued in [isolation-and-xids.md](isolation-and-xids.md).

### Shadow page tables

- Each guest root has a walk cache (directory copies, leaves per 2 MiB unit, the nodes that read each VRAM table page), and every trapped guest write to a watched page patches the copy and marks it dirty (write-through) [docs/caraxes/mapping.md §1; run: caraxes/runs/mapping-2026-09-24].
- An update walks only dirty pages and hands the mirror only the units whose leaves may have changed; a tree with a table in guest sysmem is walked in full every time [docs/caraxes/mapping.md §1].
- `resolve_leaf` admits a leaf only if it lies wholly in this VM's reservation or in one registered guest-RAM descriptor, is not compressible, and is not in a set-B range [docs/caraxes/memory-model.md §7.4].
- Leaves coalesce into one `RM_MAP_MEMORY_DMA` when contiguous in VA and backing offset with the same page size, kind, handle and snoop inside one 2 MiB region object [docs/caraxes/mapping.md §2].
- An RM unmap of part of a run splits the mapping and keeps the rest [615 tree: src/nvidia/src/kernel/mem_mgr/virtual_mem.c:1636].
- The server thread only walks and filters; a worker applies plans in order, and a doorbell or channel creation waits for every earlier plan (`wait_all`) before the host doorbell [docs/caraxes/mapping.md §4].
- `CARAXES_ALLOC_LAZY=1` defers an invalidate's plan to the next shadow doorbell, channel creation or a 20 ms deadline, so an allocation freed before then never reaches the host RM [docs/caraxes/alloc-path.md §3].
- CE channels run on the filtered shadow VAS of their root like GR, and 400 of 400 channels ran on shadow VASes with 0 `DMA_SET_PAGE_DIRECTORY` [docs/caraxes/mapping.md §5b; run: caraxes/runs/mapping-2026-09-24/d1-isolation].
- `MapBudget` refuses any run past `CARAXES_MAP_BUDGET` (default 32768) live mappings, and the guest's access then faults on its own channel [docs/caraxes/mapping.md §2].

### Gathers and region pools

- No host call takes a page list: a map covers one offset range of one memory object at one fixed VA [docs/caraxes/memory-model.md §7.2].
- A gather is one OS descriptor over a device VA window into which the scattered guest pages of a stretch are placed back to back with `mremap`, built when a stretch has at least 16 leaves that would still split into at least 8 runs (`CARAXES_SHADOW_GATHER`) [docs/caraxes/memory-model.md §7.3].
- The window is unmapped after the descriptor is made because the RM holds the pages by page array [615 tree: kernel-open/nvidia/os-mlock.c:311; docs/caraxes/memory-model.md §7.3].
- An emptied region with at least 4 runs is freed with one `RM_FREE` of its region object, which unmaps its mappings inside the free (`CARAXES_SHADOW_BULK_FREE`) [docs/caraxes/memory-model.md §7.3; 615 tree: src/nvidia/src/libraries/resserv/src/rs_client.c:1347].
- A clean emptied region object goes on a per-VAS free list of at most 64 (`CARAXES_SHADOW_REGION_POOL`), and is freed instead if an unmap in it failed or it touches set B [docs/caraxes/memory-model.md §7.3].
- Deferred TLB invalidation (`CARAXES_SHADOW_DEFER_TLB=1`) saved nothing and stays off [docs/caraxes/memory-model.md §7.3].

| per cuCtxCreate (vm0, 1 GiB, census) | before | gather-2 | native |
|---|---|---|---|
| host RM calls | 13171 | 228 | - |
| shadow apply | 770 ms | 67 ms | - |
| guest median | 815 ms | 309 ms | 146 ms |
| whole census, shadow host time | 4811 ms | 724 ms | - |

[docs/caraxes/memory-model.md §7, §7.5; run: caraxes/runs/opt-shadow-maps-2026-09-25]

### Fail-closed rules

- A gather is built only from (descriptor, offset) pairs the filter resolved, rechecked against the registered descriptors, so it adds a second pin and IOMMU mapping but no page [docs/caraxes/memory-model.md §7.4].
- A gather is freed only after its last mapping, a pooled region holds no mapping, and a refused gather or map falls back to smaller maps from the registered descriptor [docs/caraxes/memory-model.md §7.3, §7.4].
- The guest-RAM descriptors are freed only after the worker finished every shadow teardown (`wait_idle`), since DMA-FQ unmaps lazily and reuses freed IOVAs at once [docs/caraxes/mapping.md §4; docs/caraxes/sysmem-iova.md §2].
- An invalidate of a root whose mirrored channel has its USERD in the BAR1 pool or guest RAM is planned and applied before the guest continues (`untrapped.rs`, `CARAXES_SHADOW_UNTRAPPED_HOLD`, default on), because the GPPut store through that page never traps [docs/caraxes/bar1-pool.md §9; run: caraxes/runs/xid31-pool-2026-09-25].

| untrapped-GPPut hold, vm5-615, off -> on | value | Evidence |
|---|---|---|
| cuMemAlloc 1 MiB | 175 -> 200 us | [docs/caraxes/bar1-pool.md §9] |
| cuMemAlloc 64 MiB | 511 -> 2277 us | [docs/caraxes/bar1-pool.md §9] |
| cuMemFree 64 MiB | 791 -> 2402 us | [docs/caraxes/bar1-pool.md §9] |
| cuCtxCreate | 395 -> 486 ms | [docs/caraxes/bar1-pool.md §9] |
| invalidates held, allocburn 20 runs (615) | 181500, 70.5 s total, longest 102 ms | [run: caraxes/runs/xid31-pool-2026-09-25] |

- 610 delta: the swiotlb alias and the gather series were measured on the 610 guest, and the untrapped-GPPut fault reproduced on both guests [docs/caraxes/memory-model.md §2, §7.5; docs/caraxes/bar1-pool.md §9].

## Consequence for Caraxes

- Every mapping the GPU can use is a shadow leaf that passed `resolve_leaf` in the same plan, so optimisations change call counts, never the reachable set.
- Any guest store that reaches the GPU without a trap needs a barrier at the last trapped point before it, as the untrapped-GPPut hold does at the invalidate.
- The guest module and the whole-RAM pin stay until D18's sparse registration exists; the trap windows that feed the walk cache are in [windows-and-traps.md](windows-and-traps.md), the isolation rules and Xids in [isolation-and-xids.md](isolation-and-xids.md).

## Open

- D18 sparse, demand-driven registration with the cached mapping object, after which the guest DMA module and the `_DMA` patches are dropped (the vanilla gate) [docs/caraxes/roadmap.md item 9].
- Whether the GSP side honours the same `RmInstLoc` keys for its own tables (Q32) [docs/caraxes/dma-routes.md].
- The clean fix for the flush buffer: a registered chunk below 2^40, possible only before the host's 64-bit switch or with a host-side IOVA mapping [docs/caraxes/memory-model.md §2].
- A cheaper hold that waits only for plans that map something new [docs/caraxes/bar1-pool.md §9].
- The guest RM's CeUtils channel needs the device's default VA space root and a software path before `RmDisableGlobalCeUtils=1` can go [docs/caraxes/mapping.md §9].
- 8 refused DMA addresses on vm1 that look like GPAs, source open [docs/caraxes/parallel.md §8.4].

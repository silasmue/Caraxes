<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GPU page tables and the shadow

The guest's kernel RM and UVM build GMMU page tables in the guest's own video memory, as on
bare metal. The physical GPU never walks them: VFIO Leandro's device keeps a validated copy,
the shadow, and hands the host RM only the mappings that passed its filter. This is the DMA
route (decision D17) and the isolation boundary (decisions D16 and D18).

## The guest's tables

- GMMU v2 on Turing: a 5-level tree from PDE3 to PTE with 4 KiB, 64 KiB and 2 MiB pages and
  dual PDEs; entries carry an aperture (vidmem, sysmem coherent or non-coherent), a page
  kind and a comptag line. Hopper and later use the v3 format with 57-bit VAs.
- The kernel RM places PDEs and PTEs in `ADDR_FBMEM` unless `RMInstLoc` says otherwise
  (`src/nvidia/src/kernel/gpu/mmu/kern_gmmu.c`); UVM defaults to `UVM_APERTURE_VID` with a
  sysmem fallback (`kernel-open/nvidia-uvm/uvm_mmu.c`, `uvm_page_table_location`).
- Writes reach the tables through BAR2 ([bar2-page-tables-and-userd](bar2-page-tables-and-userd.md)),
  PRAMIN and copy-engine pushbuffers; each change ends with an MMU invalidate on BAR0 page
  0xb83000 and a flush read.
- An RM unmap of part of a mapped run splits the mapping and keeps the rest
  (`src/nvidia/src/kernel/mem_mgr/virtual_mem.c`).

## The shadow

1. Per guest root the device keeps a walk cache: directory copies, leaves per 2 MiB unit,
   and the nodes that read each VRAM table page. Every trapped guest write to a watched page,
   and every memfd sync, patches the copy and marks it dirty (write-through).
2. An update walks only dirty pages and hands the mirror only the units whose leaves may
   have changed; a tree with a table in guest sysmem is walked in full every time.
3. `resolve_leaf` admits a leaf only if it lies wholly in this VM's VRAM reservation or in
   one registered guest-RAM descriptor, is not compressible, and is not in a set-B range (the
   device-owned pages). Anything else is counted, refused, and faults on the guest's own
   channel.
4. Admitted leaves coalesce into one `RM_MAP_MEMORY_DMA` when contiguous in VA and backing
   with the same page size, kind and snoop inside one 2 MiB region object; scattered sysmem
   stretches of at least 16 leaves become one gather descriptor (the pages placed back to
   back in a device VA window with `mremap`, one OS descriptor over it); an emptied region is
   freed with one `RM_FREE`, and clean region objects go on a per-VAS free list.
5. A worker applies plans in order; a doorbell or a channel creation waits for every
   earlier plan (`wait_all`) before the host doorbell. `CARAXES_ALLOC_LAZY=1` defers an
   invalidate's plan to the next doorbell, channel creation or a 20 ms deadline, so an
   allocation freed before then never reaches the host RM.
6. `MapBudget` refuses any run past `CARAXES_MAP_BUDGET` (32768) live mappings.

- Shadowing costs no VRAM beyond the guest's own tables: 8 bytes per PTE, 2 MiB per GiB at
  4 KiB pages. The translated tables live in the VRAM slice, the untranslated guest view in
  host RAM.
- GR channels run on a host-managed shadow VA space (the GR context buffers the host RM
  builds are mapped there, never into the guest's root); CE channels run on the same filtered
  shadow (400 of 400 channels, 0 `DMA_SET_PAGE_DIRECTORY` on a guest root).

## Fail-closed rules

- Every mapping the GPU can use is a shadow leaf that passed `resolve_leaf` in the same
  plan; optimisations change call counts, never the reachable set.
- Any guest store that reaches the GPU without a trap needs a barrier at the last trapped
  point before it: an invalidate of a root whose mirrored channel has its USERD in the BAR1
  pool or in guest RAM is applied before the guest continues (`untrapped.rs`), because the
  GPPut store through that page never traps (host Xid 31 GPC FAULT_PDE, 20 of 20
  reproductions clean after).
- A gather is built only from (descriptor, offset) pairs the filter resolved and freed only
  after its last mapping; a refused gather or map falls back to smaller maps.
- The guest-RAM descriptors are freed only after the worker finished every teardown, since
  DMA-FQ unmaps lazily and reuses IOVAs at once.

## Measured

| per cuCtxCreate (1 GiB, census) | before | gathers and region frees | native |
|---|---|---|---|
| host RM calls | 13171 | 228 | - |
| shadow apply | 770 ms | 67 ms | - |
| guest median | 815 ms | 309 ms | 146 ms |

The untrapped-GPPut hold costs cuMemAlloc 64 MiB 511 to 2277 us and cuMemFree 64 MiB 791 to
2402 us; a hold limited to plans that map something new is not built.

## Depth

- [research/memory-model.md](../research/memory-model.md): the rules with their lines.
- [caraxes/mapping.md](../caraxes/mapping.md): the walk cache, coalescing, the worker.
- [caraxes/memory-model.md](../caraxes/memory-model.md) §7: gathers, region pools, the
  fail-closed argument.
- [caraxes/exec-formats.md](../caraxes/exec-formats.md): the GMMU v2 entry layouts and the
  RM's flush and invalidate sequence.

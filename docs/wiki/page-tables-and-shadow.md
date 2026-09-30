<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GPU page tables and the shadow

The guest's kernel RM and UVM build GMMU page tables in the guest's own video memory, as on
bare metal. The physical GPU never walks them: Leandro VFIO's device keeps a validated copy,
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
  PRAMIN and copy-engine pushbuffers. The kernel RM ends each change with an MMU invalidate
  on BAR0 page 0xb83000; UVM and the Windows KMD's paging channel push a `MEM_OP` TLB
  invalidate on a copy channel instead.
- An RM unmap of part of a mapped run splits the mapping and keeps the rest
  (`src/nvidia/src/kernel/mem_mgr/virtual_mem.c`).

## The shadow

1. Per guest root the device keeps a walk cache: directory copies, leaves per 2 MiB unit, and
   the nodes that read each VRAM table page. Every trapped guest write to a watched page, and
   every BAR2 memfd sync, patches the copy and marks it dirty. A table in guest sysmem cannot
   be watched: its nodes are re-read at every update, the VRAM copies stay.
2. An update walks only dirty pages and hands the mirror only the units whose leaves may
   have changed.
3. `resolve_leaf` admits a leaf only if it lies wholly in this VM's VRAM reservation below the
   device's private pages, or in one registered guest-RAM descriptor; has a kind the
   compression setting allows (below); and is not in a set-B range (where the host's own
   context buffers sit). Anything else is counted, refused, and faults on the guest's own
   channel.
4. Admitted leaves coalesce into one `RM_MAP_MEMORY_DMA` when contiguous in VA and backing,
   with the same page size, kind and snoop, inside one 2 MiB region object. Scattered sysmem
   stretches of at least 16 leaves become one gather descriptor (the pages placed back to back
   in a device VA window with `mremap`, one OS descriptor over it); a freed gather is kept, up
   to 64, and taken back for the same pages in the same order.
5. A worker applies plans in order. A channel build waits for every earlier plan; a doorbell or
   a trapped GPPut store waits only for its own root's plans.
   `CARAXES_ALLOC_LAZY=1` (opt-in, Linux B) defers an invalidate's plan to the next doorbell,
   channel creation or a 20 ms deadline, so an allocation freed before then never reaches
   the host RM.
6. At most 32768 live mappings; a run past that is refused.

- Shadowing costs no VRAM beyond the guest's own tables: 8 bytes per PTE, 2 MiB per GiB at
  4 KiB pages.
- GR and CE channels run on host-managed shadow VA spaces; the GR context buffers the host RM
  builds are mapped there, never into the guest's root ([execution-mirror](execution-mirror.md)).
- A VA space shared by DUP keeps its root when its owner client is freed first: the records
  move to the first surviving alias ([research: va-alloc-bursts-2026-09-26](../research/va-alloc-bursts-2026-09-26.md)).

## Invalidates

- A register invalidate plans the roots it names and arms a gate; `TRIGGER` reads busy until
  the worker applied those plans, and the device lock is free between reads. After 1 s a read
  waits to the end, so the guest's 4 s timeout never lets it pass early
  ([vfio/execution.md](../vfio/execution.md) §8).
- A software `MEM_OP` TLB invalidate that names a shadow root stops its push right after the
  method until the worker applied the plans; UVM's semaphore release comes after
  ([research: shadow-fault-race-2026-09-28](../research/shadow-fault-race-2026-09-28.md)).
- Any guest store that reaches the GPU without a trap needs a barrier at the last trapped
  point before it: an invalidate of a root whose mirrored channel has its USERD in the BAR1
  pool or in guest RAM is applied before the guest continues (`untrapped.rs`), because the
  GPPut store through that page never traps.

## Kinds, compression, managed memory

- `CARAXES_COMPRESSION=off` (the default, D16): the guest is told there is no compression and
  compressible leaves are refused. The Windows KMD still maps kind-0x09 surfaces; they stay
  refused and are never touched (windows-findings, Cloud Hypervisor step 23).
- `CARAXES_COMPRESSION=slice` (Linux B and nouveau): the reservation is compressible, kinds
  0x08 to 0x0E pass on slice pages, and the host RM writes each comptag line as
  `1 + SPA[35:16]` of the page, so a VM's lines are its own pages'
  ([research: compression-isolation](../research/compression-isolation-2026-09-28.md),
  [research: compression-guest-view](../research/compression-guest-view-2026-09-28.md)).
  Guest ZBC entries go to the host's shared table, at most 4 new ones per table and VM
  ([research: zbc-host-table](../research/zbc-host-table-2026-09-28.md)).
- A shadow VA space cannot fault: an unmodified host gives the device no GPU fault it can
  hand to the guest. A GPU touch of a managed page that UVM has not mapped ends in a host
  Xid 31 and an RC of that channel. The device therefore answers `GSP_GET_FEATURES`
  `UVM_ENABLED` 0 to a Linux guest (managed allocations fail with 801) and 1 to Windows
  (`ctrl.rs`; [research: uvm-faults](../research/uvm-faults-2026-09-28.md),
  [research: uvm-guest-ram](../research/uvm-guest-ram-2026-09-28.md)).

## Measured

| vm2-615, 1 GiB, 2026-09-30 binary | value | native |
|---|---|---|
| cuCtxCreate mean, gather cache on / off | 240 to 256 ms / 292 to 316 ms | about 130 ms |
| a context's first plan | 15876 leaves in 35 RM calls, 97.8 ms, 91.3 ms of it 30 gathers | - |
| cuMemAlloc / cuMemFree 64 MiB | 29 map calls, 1.5 ms / 32 unmaps, 1.8 ms | - |

Source: [research: ctxcreate-2026-09-29](../research/ctxcreate-2026-09-29.md) (the first-plan
and 64 MiB rows are from the 2026-09-29 base binary).

## Depth

- [research: memory-model](../research/memory-model.md): the rules with their lines.
- [vfio/mapping.md](../vfio/mapping.md): the walk cache, coalescing, the worker.
- [vfio/memory-model.md](../vfio/memory-model.md) §7: gathers, region pools, the
  fail-closed argument.
- [vfio/exec-formats.md](../vfio/exec-formats.md): the GMMU v2 entry layouts and the
  RM's flush and invalidate sequence.

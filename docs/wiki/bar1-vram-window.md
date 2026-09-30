<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# BAR1: the scarce VRAM window

BAR1 is the CPU's window onto video memory, translated by GMMU page tables that the RM
builds. On the RTX 2070 it is 256 MiB and is shared by the host desktop and every VFIO
Leandro device on the card. The guest uses it for the writes that submit work, so the device
hands the bottom of guest BAR1 to the guest as plain memory: the BAR1 pool.

## What the guest does with BAR1

- The RM-owned USERD pages are mapped top-down in BAR1 (`src/nvidia/src/kernel/gpu/fifo/arch/turing/kernel_fifo_tu102.c`);
  CPU mappings of FB allocations get allocator-chosen BAR1 addresses
  (`kbusMapFbAperture_GM107`, `src/nvidia/src/kernel/gpu/bus/arch/maxwell/kern_bus_gm107.c`).
- A CUDA submission writes two dwords of the GPFIFO entry and the USERD GPPut through BAR1,
  then rings the doorbell in BAR0 ([channels-and-submission](channels-and-submission.md)).
- Vulkan host-visible device-local memory, the framebuffer console and display imports are
  the other Linux users; CUDA's steady state has no BAR1 access beyond submission.
- The Windows KMD maps and unmaps 64 KiB CPU-visible allocations hundreds of times per second
  and maps some of its page tables through BAR1 ([vfio/bar1-pool.md](../vfio/bar1-pool.md) §13).

## The host side

- Host BAR1 is one resource with no per-client quota; the desktop's own mappings live there.
- Each device reaches its VRAM slice through two 16 MiB BAR1 windows of its own
  ([run: flags-2026-09-30](../../caraxes/runs/flags-2026-09-30/NOTE.md)), at 1.6 us per
  uncached read and about 2.5 ms per window map
  ([research: performance-facts](../research/performance-facts.md)).
- The device's own writes through a window are write-combined: it flushes (`sfence` and a
  read-back) before every host doorbell and TLB invalidate, or the PBDMA fetches stale VRAM
  ([vfio/execution.md](../vfio/execution.md) §2).
- At start the device reads the host's free BAR1 (`nvidia-smi`, read-only) and refuses a pool
  that, with its windows, would leave less than `CARAXES_HOST_BAR1_FLOOR_MIB` (96 MiB unless
  set; the Linux wrappers set 64, an open A/B). Beside a Windows VM with its pool, a second VM's pool
  breaks the floor ([research: ctxcreate-2026-09-29](../research/ctxcreate-2026-09-29.md)).
- A leak is silent: 732088 refused host BAR1 mappings came without any Xid
  ([research: isolation-and-xids](../research/isolation-and-xids.md)).

## The BAR1 pool

On by default with `--host-exec`. `CARAXES_BAR1_POOL_MIB` sizes it: 32 MiB by default, 0 for
none, otherwise an even number from 2 to 128, at most half of guest BAR1 and a quarter of the
usable FB. Code: `crates/vfio-user-nvgpu/src/bar1pool.rs`, `fb.rs` `FakeFb::remap`.

1. The device carves the pool from the top of the VM's slice, below the 2 MiB carve-out, and
   takes it out of the guest's usable FB region (`FbLayout::with_pool`).
2. It exports the pool as a dma-buf that backs one sparse mmap area of guest BAR1 `[0, pool)`:
   area page `g` is pool page `pool0 + g`, reached without a trap.
3. The device learns BAR1 table changes from trapped table writes and from BAR2 memfd syncs
   (`FakeFb::note_pages_changed`) and walks only the changed 2 MiB units. A page the guest maps
   to FB page `X` is copied to its pool page, and every device translation (FB store, BAR2,
   PRAMIN, shadow leaves, a host channel's USERD) treats `X` as living there.
4. An unmap copies nothing: `X` stays in the pool, retired. It goes home when another page
   wants its area page, when the guest maps it at another area page, when it becomes a page
   table the device follows, or after 2 s unmapped.
5. A page the GPU may be using moves only behind a barrier: the rings of the affected roots are
   held, their TSGs preempted as worker jobs in order with the roots' plans, the shadows
   re-synced; `TRIGGER` reads busy meanwhile. A page that cannot move safely is refused and
   retried (fail closed).
6. A page mapped twice in the area, or a page table mapped through it, is served as a
   replica. Replicas are merged home where an engine starts: a channel doorbell (the pages its
   channel can observe), the GSP command queue, a BAR2 bind, the 1 ms flush clock, a display
   `PUT`; table replicas also at an MMU invalidate.

- USERD in the pool means a GPPut store that never traps; the shadow therefore applies an
  invalidate of such a root before the guest continues (`untrapped.rs`;
  [page-tables-and-shadow](page-tables-and-shadow.md)).
- A fixed map of a traced guest layout is unsafe: a layout 2 MiB off mapped 128 area pages to FB
  pages the area did not back. The pool follows the guest's tables instead.
- Isolation: the pool is part of the VM's slice and stops below the carve-out; a trapped BAR1
  access that touches the device's private pages is refused.
- Cost: 13 ms for the 1024 page moves at a CUDA process's start and end
  ([research: ctxcreate-2026-09-29](../research/ctxcreate-2026-09-29.md)). Under Windows the
  pool took a session's trapped BAR1 accesses from 5.64 M to 148 (windows-findings, Cloud
  Hypervisor step 5).

## The ReBAR outlook

- TU106's resizable-BAR capability offers only 64, 128 and 256 MiB. On a card whose BAR1
  covers its VRAM, the pool could cover all of guest BAR1 and the device could map its slice
  once; zero-copy pool moves still need a VMM change that re-points one 4 KiB page of a
  sparse area, since Cloud Hypervisor maps areas once.

## Depth

- [vfio/bar1-pool.md](../vfio/bar1-pool.md): the design, the barrier, the lazy move back,
  the merge points, the measurements.
- [vfio/bar1-passthrough.md](../vfio/bar1-passthrough.md): the traced guest layout, the
  sparse-area mechanics, the NVIDIA mmap rule.
- [vfio/optimization-backlog.md](../vfio/optimization-backlog.md) §5: ReBAR.

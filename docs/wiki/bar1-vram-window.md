<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# BAR1: the scarce VRAM window

BAR1 is the CPU's window onto video memory, translated by GMMU page tables that the RM
builds. On the RTX 2070 it is 256 MiB, cannot grow, and is shared by the host desktop and
every VFIO Leandro device on the card. The guest uses it for the writes that submit work,
so the device hands part of it to the guest as plain memory.

## What the guest does with BAR1

- The RM-owned USERD pages are mapped top-down in BAR1 (`src/nvidia/src/kernel/gpu/fifo/arch/turing/kernel_fifo_tu102.c`);
  CPU mappings of FB allocations get allocator-chosen BAR1 addresses
  (`kbusMapFbAperture_GM107`, `src/nvidia/src/kernel/gpu/bus/arch/maxwell/kern_bus_gm107.c`).
- A submission writes two dwords of the GPFIFO entry and the USERD GPPut through BAR1, then
  rings the doorbell in BAR0 ([channels-and-submission](channels-and-submission.md)).
- Vulkan host-visible device-local memory, the framebuffer console and display imports are
  the other users; CUDA's steady state has no BAR1 access beyond submission.
- BAR1 is write-combined: the PBDMA fetched stale VRAM after a guest write until the device
  flushed (`sfence` and a read-back) before every host doorbell and TLB invalidate (host Xid
  32 PBENTRY, fixed).

## The host side

- BAR1 is one host-wide resource with no per-client quota; the desktop's own mappings live
  there too. Each device is budgeted `CARAXES_BAR1_WINDOWS` x `CARAXES_BAR1_WINDOW_MIB`
  (default 2 x 32 MiB) and evicts its own windows on a refused map; a leak once produced
  732088 refused host BAR1 mappings without any Xid.
- The device reaches its VRAM slice through those windows: 1.6 us per uncached read and
  2.5 ms per window miss, which is why the pool plus the windows must stay under 192 MiB.

## The BAR1 pool (`CARAXES_BAR1_PASSTHROUGH=pool`)

1. The device carves a private pool (`CARAXES_BAR1_POOL_MIB`, default 64, at most half of
   guest BAR1) from the top of the VM's slice, below the carve-out, and takes it out of the
   guest's usable FB region.
2. It exports the pool as a dma-buf that backs one sparse mmap area of guest BAR1
   `[0, pool)`, so a guest access of area page `g` reaches pool page `pool0 + g` without a
   trap.
3. After every trapped write with which the guest flushes and invalidates a BAR1 mapping
   change, the device walks the guest's BAR1 tables over the area: a page the guest maps to FB
   page `X` is copied to its pool page, and every device translation (FB store, shadow leaves,
   host USERD) treats `X` as living there; unmapping copies it back.
4. A page the GPU may be using moves only behind a barrier: the affected host channels idle,
   their TSGs preempted, the shadows re-synced; a page that cannot be moved safely is refused
   and retried (fail closed).
5. Cost: 1024 pool moves per CUDA process (363 ms median) and the barrier's TSG preempts,
   both outside every benchmark row.

- The earlier prototype (`CARAXES_BAR1_PASSTHROUGH=1`) mapped the traced layout directly and
  is refused unless `CARAXES_BAR1_PASSTHROUGH_UNSAFE_LAYOUT=1`, because a guest layout 2 MiB
  off mapped area pages to FB pages the area did not back.
- USERD in the pool means a GPPut store that never traps; the shadow therefore applies an
  invalidate of such a root before the guest continues (`untrapped.rs`), which is the cost
  on frees ([page-tables-and-shadow](page-tables-and-shadow.md)).

## The ReBAR outlook

- TU106's resizable-BAR capability offers only 64, 128 and 256 MiB. On a card whose BAR1
  covers its VRAM, the pool could cover all of guest BAR1 and the device could map its slice
  once; zero-copy pool moves still need a VMM change that re-points one 4 KiB page of a
  sparse area, since Cloud Hypervisor maps areas once.

## Depth

- [caraxes/bar1-pool.md](../caraxes/bar1-pool.md): the design, the barrier, the measurements.
- [caraxes/bar1-passthrough.md](../caraxes/bar1-passthrough.md): the traced guest layout, the
  sparse-area mechanics, the NVIDIA mmap rule.
- [caraxes/optimization-backlog.md](../caraxes/optimization-backlog.md) §5: ReBAR.

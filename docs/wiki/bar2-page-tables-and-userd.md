<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# BAR2: the page-table and USERD aperture

BAR2 (PCI BAR3 on Turing, 32 MiB) is the RM's own virtual aperture into video memory: the
kernel RM maps page tables, instance blocks and scrub targets through it, translated by a
GMMU page table it bootstraps itself. The RM's USERD pages sit behind the same mechanisms.
In VFIO Leandro BAR2 is a memfd in host RAM, synced at the RM's own flush points.

## How the kernel RM uses BAR2

1. Bootstrap: the RM writes the BAR2 page tables through the PRAMIN window of BAR0
   (`NV_PBUS_BAR0_WINDOW` at 0x1700 pointing into FB; `kern_bus_gm107.c`, `bar2_walk.c`),
   reads back its PDE3[0] through PRAMIN and sends it to GSP-RM with `UPDATE_BAR_PDE`
   (`kbusPatchBar2Pdb_GSPCLIENT`, `src/nvidia/src/kernel/gpu/bus/kern_bus.c`), which merges
   the kernel's root into GSP's `bar2PdeBase`.
2. Verify: `kbusVerifyBar2_GM107` writes a pattern through BAR2, reads it through PRAMIN
   and back; PRAMIN and BAR2 must alias the same FB bytes or state init fails with
   `NV_ERR_MEMORY_ERROR`.
3. Use: `kbusMapBar2ApertureCached_VBAR2` (`kern_bus_vbar2.c`) allocates BAR2 VA from an
   eheap and writes the PTEs with `kbusUpdateRmAperture_HAL` through the BAR2 mapping of the
   BAR2 tables themselves; each update ends with `osFlushCpuWriteCombineBuffer`, a flush
   read and an MMU invalidate.
- The kernel RM never writes `NV_PBUS_BAR2_BLOCK`; GSP-RM owns the bind. A teardown writes
  MODE=PHYSICAL only on the failure path.
- A context creation writes about 72700 BAR2 dwords through this path (page tables for the
  new VA space); Mutter writes 71 per frame.

## USERD

- USERD is the per-channel page holding GPPut and GPGet. The RM places its own with the
  `RMInstLoc` fields (default vidmem; `kern_gmmu.c` for page tables, `kernel_fifo_tu102.c` for
  the BAR1 mapping) and a user-space channel's through the channel-alloc parameters.
- `RMInstLoc=0x10000` (USERD in coherent sysmem) left the guest stuck in a `BOOT_0` poll loop
  before `RmInitAdapter` finished, so USERD stays in the slice or the BAR1 pool.
- A deferred-API channel-info update (GL, Vulkan) registers a USERD through `ALLOC_MEMORY`
  ([gsp-rpc-and-events](gsp-rpc-and-events.md)).

## The BAR2 memfd (`CARAXES_BAR2_MEMFD=1`)

1. Region 3 is a sealed memfd with sparse mmap areas over all of it except one trapped page,
   the flush page at BAR2 0xfea000. Guest BAR2 offset `o` is memfd offset `o`.
2. While an FB page is mapped through the guest's BAR2 tables, its current content lives in
   the memfd page of the BAR2 offset that maps it; `FakeFb::backing` is the single
   translation for the FB store, PRAMIN, the walks and the display.
3. The device cannot see guest writes through the VMM's mapping (soft-dirty, userfaultfd
   write-protect and mprotect on the device's own mapping do not see another mapping's
   stores; KVM dirty logging would, with a VMM change), so at the RM's own synchronisation
   points, which stay trapped, it compares each mapped page with its snapshot: the flush
   read, the MMU invalidate, the BAR2 bind, the GSP command-queue doorbell.
4. A sync writes back exactly the changed bytes to VRAM, marks those pages dirty in the
   shadow's walk caches, and follows the BAR2 tables so that a page leaving the mapping is
   written back and one entering it is loaded.
5. Fail closed: if the memfd cannot be created, sealed or mapped, or a second mapping's
   write is not seen, region 3 stays trapped; the same without the boot model or on a GMMU
   v3 chip.

- Measured: trapped BAR2 writes per context creation from about 96000 to 0; cuCtxCreate 4.1
  s to 0.87 s alone and 0.61 s with the BAR1 pool; 834 syncs per CUDA process at 126 us mean,
  which is what the flush read now carries (about 200 us instead of 30 us).
- The pool learns BAR1 table changes only from trapped writes; with the memfd those tables
  go through BAR2, so `FakeFb::note_pages_changed` tells the pool (the hang of the first
  combined run, fixed with two unit tests).

## Depth

- [caraxes/bar2-memfd.md](../caraxes/bar2-memfd.md): the design, hazards, tests, measurement.
- [caraxes/fb-model.md](../caraxes/fb-model.md): PRAMIN, the BAR2 walk and the FB layout.
- [caraxes/kernel-rm-hw-surface.md](../caraxes/kernel-rm-hw-surface.md) §C KernelBus.
- [caraxes/mapping.md](../caraxes/mapping.md) §3: the BAR2 accounting of a context creation.

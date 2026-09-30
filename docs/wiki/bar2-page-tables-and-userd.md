<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# BAR2: the page-table and USERD aperture

BAR2 (PCI BAR3 on Turing, 32 MiB) is the RM's own virtual aperture into video memory: the
kernel RM maps page tables, instance blocks and scrub targets through it, translated by a
GMMU page table it bootstraps itself. The RM's USERD pages sit behind the same mechanisms.
In VFIO Leandro BAR2 traps by default; with the BAR2 memfd it is host RAM, synced where the
driver itself orders its writes. Trap rates are on [traps-and-fast-paths](traps-and-fast-paths.md).

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

- The flush read is a read of FB address 0 through BAR2, at BAR2 0xfea000 for the 615 RM;
  nouveau reads it through its own mapping at BAR2 0x8000 or writes `NV_UFLUSH_FB_FLUSH`
  ([research: open-render-2026-09-29](../research/open-render-2026-09-29.md)).
- The kernel RM never writes `NV_PBUS_BAR2_BLOCK`; GSP-RM owns the bind. A teardown writes
  MODE=PHYSICAL only on the failure path.
- The Windows KMD writes BAR2 in byte stores, about 4096 per 4 KiB page
  ([research: windows-traps-exit-2026-09-29](../research/windows-traps-exit-2026-09-29.md)).

## USERD

- USERD is the per-channel page holding GPPut and GPGet. The RM places its own with the
  `RMInstLoc` fields (default vidmem; `kern_gmmu.c` for page tables, `kernel_fifo_tu102.c` for
  the BAR1 mapping) and a user-space channel's through the channel-alloc parameters.
- `RMInstLoc=0x10000` (USERD in coherent sysmem) left the guest stuck in a `BOOT_0` poll loop
  before `RmInitAdapter` finished, so USERD stays in the slice or the BAR1 pool.
- A deferred-API channel-info update (GL, Vulkan) registers a USERD through `ALLOC_MEMORY`
  ([gsp-rpc-and-events](gsp-rpc-and-events.md)).

## The BAR2 memfd (`CARAXES_BAR2_MEMFD=1`, opt-in)

1. Region 3 is a sealed memfd with sparse mmap areas over all of it except the flush page at
   BAR2 0xfea000; guest BAR2 offset `o` is memfd offset `o` (`crates/vfio-user-nvgpu/src/bar2area.rs`).
2. While an FB page is mapped through the guest's BAR2 tables, its current content lives in
   the memfd page that maps it; `FakeFb::backing` is the single translation for the FB store,
   PRAMIN, the walks and the display.
3. The device cannot see the guest's stores through the VMM's mapping (soft-dirty,
   userfaultfd write-protect and `mprotect` act on the device's own mapping; KVM dirty logging
   would need a VMM change). It compares mapped pages with their snapshots, in place, where an
   engine may consume:
   - a flush read, or `NV_UFLUSH_FB_FLUSH`, only marks a sync owed; the next engine start
     settles it: a channel doorbell, the GSP command-queue head, a BAR2 bind, or the 1 ms
     flush clock;
   - an MMU invalidate syncs the live page-table pages the device follows (guest roots, BAR
     tables, the pool's area walk), not the data;
   - a channel doorbell without an owed flush syncs only the pages the rung channel can
     observe (its VA space's leaves less set B, USERD, instance block, RAMFC, method buffer);
     a channel the device cannot name syncs every live page.
4. A sync writes back exactly the changed bytes to VRAM, marks those pages dirty in the
   shadow's walk caches, and follows the BAR2 tables: a page leaving the mapping is written
   back, one entering it is loaded.
5. Device reads of a data page behind BAR2 merge VRAM with the guest's unsynced BAR2 bytes,
   since other writers exist (GPPut through the BAR1 pool, GPGet from the PBDMA); page-table
   reads stay on the memfd. A table page that leaves BAR2 keeps its snapshot while a walk still
   reads it (at most 4096 pages).
6. Fail closed: if the memfd cannot be created, sealed or mapped, or a write through a second
   mapping is not seen, region 3 stays trapped; the same without `--boot-model` or on a GMMU
   v3 chip. The memfd never maps a device-private page and never reads one as a table.

- nouveau on a quiet card, one binary: `vkCreateDevice` 13385 ms with BAR2 trapped, 48 ms with
  the memfd; glmark2 and vkmark within the run-to-run spread. Per sync: 1.6 us at a doorbell,
  149 us at an invalidate, 183 us at a GSP queue head
  ([research: open-render-2026-09-29](../research/open-render-2026-09-29.md)).
- Linux B 615 and nouveau run it. Windows does not: under QEMU it halved context setup and cut
  steady rates by 40 % (cause open), and under Cloud Hypervisor the shadow missed PTEs written
  through BAR2 (54 host Xid 31), which syncing the table pages before every re-plan
  addresses. The Windows A/B is open ([research: windows-findings](../research/windows-findings.md)
  P37, P38, Cloud Hypervisor step 19).

## Depth

- [caraxes/bar2-memfd.md](../vfio/bar2-memfd.md): the design, the sync rules, hazards, tests.
- [caraxes/fb-model.md](../vfio/fb-model.md): PRAMIN, the BAR2 walk and the FB layout.
- [caraxes/kernel-rm-hw-surface.md](../vfio/kernel-rm-hw-surface.md) §C KernelBus.
- [caraxes/mapping.md](../vfio/mapping.md) §3: the BAR2 accounting of a context creation.

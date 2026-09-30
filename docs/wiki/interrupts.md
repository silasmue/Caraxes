<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Interrupts

A Turing GPU signals the CPU through one MSI and a tree of interrupt registers in BAR0. The
kernel RM reads the tree to find the pending vector, acknowledges it, re-arms the MSI, and
toggles the enables at every RM lock. Leandro VFIO serves the tree from memory, the interrupt
page: a Linux guest stores to it as memory (tagged mode), a Windows guest's stores trap
(exact mode). What still traps and what it costs is on
[traps-and-fast-paths](traps-and-fast-paths.md).

## The tree

- The VF CPU interrupt tree at BAR0 0xb80000 (`dev_vm.h`): leaves (`LEAF(n)`, `LEAF_EN_SET`,
  `LEAF_EN_CLEAR`), the top (`TOP(0)`, `TOP_EN_SET`, `TOP_EN_CLEAR`) and `LEAF_TRIGGER`; the
  vectors come from `NV2080_CTRL_CMD_INTERNAL_INTR_GET_KERNEL_TABLE`
  (`src/nvidia/src/kernel/gpu/intr/intr.c`).
- Stall servicing: `intrGetPendingStallEngines_TU102`, `intrClearLeafVector_TU102` (write 1 to
  clear; `src/nvidia/src/kernel/gpu/intr/arch/turing/intr_tu102.c`); non-stall:
  `intrGetPendingNonStall_TU102` (`intr_nonstall_tu102.c`).
- The RM lock: `_intrDisableStall_TU102` at acquire and `_intrEnableStall_TU102` at release write
  the leaf and top enables; `_intrSetIntrEnInHw_GP100` (`intr_gp100.c`) writes
  `NV_PMC_INTR_EN_SET/CLEAR` on page 0, masked with `pmcRmOwnsIntrMask`, which is 0 in
  GSP-client mode: both writes are 0 and change nothing.
- The display vector (156 on TU106) is enabled alone by `intrSetDisplayInterruptEnable_TU102`.
- The Blackwell tree differs (vectors 0x84, 0x40, 0x48, 0x94; subtrees RUNLIST and
  RUNLIST_NOTIFICATION), held in `chips/gb206/interrupts.toml`.

## MSI and the re-arm

- One MSI vector on TU106. No synthetic board offers MSI-X (`board.rs`: `msix=false`).
- The RM re-arms by writing 0 to `NV_XVE_CYA_2` through the config mirror (BAR0 0x88704,
  `kbifRearmMSI_GM107` in `kernel_bif_gm107.c`); the hardware looks only at the offset.
- The device writes the pending state first, then signals its MSI through the vfio-user IRQ
  eventfd.
- Level semantics: while an enabled vector is pending, every re-arm sends the MSI again, as on
  the card. A handler that re-arms without an ack therefore loops.
- `CARAXES_MSI_REARM_IOEVENTFD=1` (opt-in) makes the re-arm write an ioeventfd without
  datamatch, read by the token reactor; it needs the token pool or the guest tokens.

## The interrupt page

1. With `--host-exec` the device offers BAR0 page 0 (`NV_PMC`) and the tree page 0xb81000 as
   sparse mmap areas of one memfd (`crates/vfio-user-nvgpu/src/intr_page.rs`); the rest of
   BAR0 stays trapped. The reactor (`CARAXES_DOORBELL_POLL`, `hybrid` by default) owns the
   page's device side and writes the MSI eventfd only after the page.
2. Tagged mode, a writable page (Linux): a memory page cannot report a store of the value it
   holds, so every register the guest writes shows a value the guest never stores: bit 31 in
   the leaves and `TOP_EN_SET` (no vector of the kernel table ends in 31), a sentinel in the
   clear and trigger registers. The reactor scans the 28 watched words each loop and applies
   the stores with the trapped semantics.
3. Leaf enables are reported, not modelled: the page shows every vector of the kernel table
   and learns the bits the guest sets, since eight enable stores within one lock cycle leave
   only the last in memory.
4. A set store while a vector is pending re-sends the MSI, as a lock release does on the card.
   A 10 ms retrigger watchdog bounds what a scan can miss to a late interrupt, never a lost one.
5. Exact mode (Windows): Cloud Hypervisor maps the tree page read-only (patches 0007 and 0008;
   0008 keeps the flag when EDK2 moves the BAR). The first trapped store switches the page to
   the registers' own values; each store is applied as it arrives, off the device lock. Page 0
   stays writable memory.

- The tagged page gave the Windows 617.14 KMD three GSP resets and 3898 watchdog MSIs under
  QEMU; the hypothesis is that the KMD takes bit 31 for a vector it can never clear
  ([research: windows-findings](../research/windows-findings.md) P37). Linux keeps the
  tagged page until an A/B on Linux B.

## The display page

- `CARAXES_DISPLAY_PAGE=1` (opt-in, needs `--display`): the front-end status page 0x611000 as a
  third area of the same memfd (`display/fe_page.rs`). The display clock runs the engine at
  every tick and raises vector 156 after the page is written.
- On Linux B Cloud Hypervisor maps it read-only: reads are memory, and `LAST_DATA_RESET` and
  the other acks trap to a route served off the device lock. They are no BAR1 pool sync point
  ([research: vblank-path-2026-09-29](../research/vblank-path-2026-09-29.md)).
- The window AWAKEN, whose ack is an echo memory cannot see, is published with a CORE marker
  whose `CORE_RESET` acknowledges exactly the bits shown.
- The display vector is raised only while the front end's interrupt line is asserted after a
  tick: a raise with nothing pending made the KMD's handler leave the leaf, and exact mode then
  re-sent the MSI at every re-arm (windows-findings, Cloud Hypervisor step 11).

## Host completions

- The mirror arms each host engine's non-stall notifier
  (`NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION`, REPEAT) on its host subdevice; a host event raises
  the guest's non-stall vector of that engine (vectors 4 to 9).
- These notifiers fire for every client of the host GPU, the desktop included: a wake-up
  broadcast, a cost and a timing side channel. `NVC46F_NOTIFIERS_NONSTALL` is defined per
  channel, but the RM never notifies it.
- Withdrawal: when a re-arm's MSI would be for pending host hints only and the guest re-armed
  256 times without an ack, the hints are muted for 1 s or until the guest acks or enables
  them. The device's own vectors are never withdrawn. This ends the MSI storms of the Windows
  KMD's reset and shutdown paths (windows-findings, step 18).
- A mirror's teardown sets each notifier back to DISABLE; a notifier still armed by an earlier
  mirror gets DISABLE, then REPEAT, so host events reach the guest again after an FLR
  ([research: flr-host-events-2026-09-29](../research/flr-host-events-2026-09-29.md)).
- A device reset resets the boot model before the page, so the old mirror's late raises are
  dropped.

## In Leandro VirtIO

- Queue 1 carries RM events (`KIND_EVENT_FIRED`) from the backend's nested epoll; semaphore
  waiters use a separate polling thread; a blocking wait falls back to a 100 ms poll
  ([architecture.md](../virtio/architecture.md)).

## Depth

- [vfio/intr-page.md](../vfio/intr-page.md): the per-register census, both modes, the
  ambiguity list, the display page.
- [vfio/kernel-rm-hw-surface.md](../vfio/kernel-rm-hw-surface.md) §C Intr.
- [research: windows-and-traps](../research/windows-and-traps.md),
  [research: trap-census-2026-09-30](../research/trap-census-2026-09-30.md): the traps per
  frame, per operation and per workload.

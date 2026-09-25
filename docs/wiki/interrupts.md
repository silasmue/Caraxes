<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Interrupts

A Turing GPU signals the CPU through one MSI and a tree of interrupt registers in BAR0 that
the kernel RM reads to find the pending vector, acknowledges, and toggles at every RM lock.
Serviced through traps, one interrupt costs about 60 register accesses; VFIO Leandro offers
the pages as memory and applies the guest's stores from a polling thread.

## The tree

- The VF CPU interrupt tree at BAR0 0xb80000 (`dev_vm.h`): leaves (`LEAF(n)`, `LEAF_EN_SET`,
  `LEAF_EN_CLEAR`), the top (`TOP(0)`, `TOP_EN_SET`, `TOP_EN_CLEAR`) and `LEAF_TRIGGER`; the
  vectors come from `NV2080_CTRL_CMD_INTERNAL_INTR_GET_KERNEL_TABLE`
  (`src/nvidia/src/kernel/gpu/intr/intr.c`).
- Stall servicing: `intrGetPendingStallEngines_TU102`, `intrClearLeafVector_TU102`
  (write 1 to clear; `src/nvidia/src/kernel/gpu/intr/arch/turing/intr_tu102.c`); non-stall:
  `intrGetPendingNonStall_TU102` (`intr_nonstall_tu102.c`).
- The RM lock: `_intrDisableStall_TU102` at acquire and `_intrEnableStall_TU102` at release
  write the leaf and top enables; `_intrSetIntrEnInHw_GP100` (`intr_gp100.c`) writes
  `NV_PMC_INTR_EN_SET/CLEAR` on page 0, masked with `pmcRmOwnsIntrMask`, which is 0 in
  GSP-client mode: both writes are 0 and change nothing, 183 per Mutter frame.
- The display vector (156 on TU106) is enabled alone by `intrSetDisplayInterruptEnable_TU102`;
  the display FE interrupt status page at 0x611000 has its own 28 watched words.
- The Blackwell tree differs (vectors 0x84, 0x40, 0x48, 0x94; subtrees RUNLIST and
  RUNLIST_NOTIFICATION), held in `chips/gb206/interrupts.toml`.

## MSI

- One vector on TU106 (GB206 has MSI-X with 9). The RM re-arms it by writing 0 to
  `NV_XVE_CYA_2` through the config mirror (BAR0 0x88704, `kernel_bif_gm107.c`); the hardware
  looks only at the offset.
- The device signals its MSI through the vfio-user IRQ eventfd after it wrote the pending
  bits; with the re-arm as an ioeventfd (`CARAXES_MSI_REARM_IOEVENTFD=1`) the re-arm trap
  goes from 140 to 160 per second to 0 in every CUDA workload.

## The interrupt page (`CARAXES_INTR_PAGE=1`)

1. BAR0 page 0 (`NV_PMC`) and page 0xb81000 are offered as sparse mmap areas of a memfd, so
   the VMM maps them into the guest as memory; the rest of BAR0 stays trapped.
2. The device writes the pending bits and the summary before it signals the MSI; the
   doorbell reactor compares the page with what the device last wrote on every loop and
   applies the guest's stores with the trapped semantics (write-1-to-clear, enable set and
   clear, trigger).
3. A memory page cannot report a store of the value it already holds, so every register the
   guest writes shows a value the guest never stores: a tag bit in the leaves and the top
   enable, a sentinel in the clear and trigger registers; a retrigger watchdog bounds what
   stays ambiguous to a late interrupt, never a lost one.
4. The display page (`CARAXES_DISPLAY_PAGE=1`) does the same for 0x611000, with
   `LAST_DATA_RESET` and the other acks applied by the reactor.

| Per Mutter frame | no page | interrupt page | plus display page (615 guest) |
|---|---:|---:|---:|
| trapped accesses | 662 | 230 | 139 |
| frame callbacks | 41.5 Hz | 59.7 Hz | 58.6 Hz |

Weston: 157, 103 and 29 trapped accesses per frame at 58 to 60 Hz.

## Host completions and the side channel

- The host RM's non-stall completion notifier is per engine and host-wide: the desktop's own
  GR completions reached an idle guest's interrupt path (32000 events in 3 minutes; 331
  MSIs per second in the coverage map). It is a wake-up broadcast, not data, but a timing
  side channel and a cost.
- Mitigation in the backlog: forward an event only when one of the VM's mirrored channels
  on that engine made progress, judged from its USERD, with the retrigger watchdog as the
  bound. Full per-VM interrupt routing exists only on SR-IOV parts.

## In VirtIO Leandro

- Queue 1 carries RM events (`KIND_EVENT_FIRED`) from the backend's nested epoll; semaphore
  waiters use a separate polling thread; a blocking wait falls back to a 100 ms poll
  ([ARCHITECTURE.md](../ARCHITECTURE.md)).

## Depth

- [caraxes/intr-page.md](../caraxes/intr-page.md): the per-register census, the design, the
  ambiguity list, the measurement.
- [caraxes/kernel-rm-hw-surface.md](../caraxes/kernel-rm-hw-surface.md) §C Intr.
- [research/windows-and-traps.md](../research/windows-and-traps.md): per frame and per operation.

<!-- SPDX-License-Identifier: MIT -->
# The interrupt page: interrupt servicing without traps

Written 2026-09-24 for the operator's order "pack die Display-Optimierungen an" (14:40): the guest
driver's interrupt handling must stop trapping. Spec first: every claim names a file and line.
Sources:

- NVIDIA open kernel modules 610.57.04: `vendor/open-gpu-kernel-modules` (`src/nvidia/...`,
  `src/common/inc/swref/published/turing/tu102/...`).
- vfio-user specification: `vendor/qemu-vfio-user/vfio-user.rst`.
- Cloud Hypervisor v53.0 with patches 0001-0005: `/mnt/vmstore/caraxes/ch-0005`.
- The vendored server crate `vendor/vfio-user-rs/src/lib.rs` (vfio_user 0.1.6 with patches 0001
  and 0002).
- The per-frame traces of series F: `caraxes/runs/display-f-2026-09-24/f3-traps-mutter`,
  `f3b-traps-mutter-fresh`, `f4-traps-weston` (`device-trace.txt.xz`, the window of
  `caraxes/tools/trap-table.py`: 10 s ending 1 s before the last scanout latch).

The implementation is in `crates/vfio-user-nvgpu/src/intr_page.rs` behind `CARAXES_INTR_PAGE=1`
(default off). Section 8 has the build and test state and the measurement.

## Summary

1. Two 4 KiB pages of BAR0 carry almost all of the interrupt servicing: page 0 (`NV_PMC`, where
   every RM lock acquire and release writes 0 to `NV_PMC_INTR_EN_SET/CLEAR`) and the VF CPU
   interrupt tree at 0xb81000 (leaves, leaf enables, top, top enables, trigger); per Mutter frame
   they take 437 of the 662 trapped accesses (f3b), per Weston frame 81 of 157 (f4).
2. With the switch on, the device offers exactly these two pages as sparse mmap areas of region 0,
   backed by a memfd that becomes region 0's descriptor, so Cloud Hypervisor (with 0005, no new
   patch) maps them into the guest as memory; the rest of BAR0, the doorbell ioeventfd
   sub-regions at 0xbb0090 included, stays trapped.
3. The device keeps the page current (it writes the pending bits and the summary before it
   signals the MSI), and the doorbell reactor, which must spin or run hybrid, compares the page
   with what the device last wrote there on every loop iteration and applies the guest's stores
   with the semantics of the trapped path (write-1-to-clear, enable set and clear, trigger).
4. A memory page cannot report a store of the value it already holds, so every register the
   guest writes shows a value the guest never stores: a tag bit that is no interrupt vector in
   the leaves and the top enable, a sentinel in the clear and trigger registers; what stays
   ambiguous (two stores to one register between two scans) is listed with its effect, and a
   retrigger watchdog bounds every such case to a late interrupt, never a lost one.
5. The expected effect is 662 to about 225 traps per Mutter frame (711 to about 251 in f3, 157 to
   about 76 for Weston); below 100 needs the display engine's interrupt page (0x611000, 91 per
   frame, a read with a side effect today) and Mutter's BAR2 page-table churn (94 per frame),
   which this change does not touch.

## 1. What the driver touches per interrupt

Per presented frame in the trap-table window (counted per BAR0 page and per register and value
with the same window as `trap-table.py`; MSIs are the device's `msi: signalled` plus the host
notifier events that wrote the MSI):

| Per frame | Mutter f3b (406 frames) | Mutter f3 (300) | Weston f4 (585) |
|---|---|---|---|
| all trapped accesses | 661.5 | 711.7 | 157.0 |
| MSIs (device edges and host events) | 7.22 | 8.75 | 3.58 |
| page 0x000000 writes (`NV_PMC_INTR_EN_SET/CLEAR`) | 183.37 | 186.65 | 9.94 |
| page 0x000000 reads (`BOOT_0`, `NV_PMC_INTR_EN(0)`) | 2.74 | 3.24 | 1.99 |
| page 0xb81000 writes (VF CPU interrupt tree) | 171.41 | 175.71 | 13.27 |
| page 0xb81000 reads | 79.57 | 95.01 | 56.20 |
| **the two pages** | **437.1** | **460.6** | **81.4** |
| page 0x611000 reads / writes (display FE interrupt status) | 88.79 / 2.46 | 110.69 / 2.88 | 54.29 / 2.02 |
| page 0xbb0000 reads (usermode `TIME_0/1`) / writes (doorbell) | 14.73 / 3.00 | 17.26 / 3.01 | 8.95 / 2.00 |
| BAR2 writes + reads (Mutter's per-frame PTEs) | 93.8 | 94.3 | 0 |

With about 7 MSIs per Mutter frame, the two pages are 60 trapped accesses per interrupt and the
display page another 13: the "about 70 per interrupt" of display-e.md §6. Not all of it is the
interrupt handler: the enable toggles are the RM's GPU lock (below), which the display and
channel paths take many times per frame.

The registers and values (f3b, per frame; addresses in `turing/tu102/dev_vm.h` relative to the
VF window 0xb80000, `dev_boot.h:35-44` for PMC):

| Register | Access, value | Per frame | Who, with its semantics |
|---|---|---|---|
| `NV_PMC_INTR_EN_CLEAR(0)` 0x180, `_SET(0)` 0x160 | W 0x0, W 0x0 | 91.69 each | `_intrSetIntrEnInHw_GP100` (`src/nvidia/src/kernel/gpu/intr/arch/pascal/intr_gp100.c:35-67`) at every RM lock acquire and release; both values are masked with `pmcRmOwnsIntrMask`, which `intr_tu102.c:109-115` builds from the table's `pmcIntrMask` entries and which is 0 here, so both writes are 0: no effect in hardware, latched by the device (`bar0.rs` `WriteEffect::Latched`) |
| `NV_PMC_INTR_EN(0)` 0x140 | R 0x0 | 1.36 | `intrGetIntrEnFromHw_GP100` (`intr_gp100.c:150`), latched value |
| `NV_PMC_BOOT_0` 0x0 | R 0x166000a1 | 1.38 | a constant |
| `LEAF_EN_CLEAR(2)` 0xb81408, `LEAF_EN_CLEAR(4)` 0xb81410, `TOP_EN_CLEAR(0)` 0xb81610 | W 0x3, W 0xffffffff, W 0x8 | 18.34 each | lock acquire: `_intrDisableStall_TU102` (`intr_tu102.c:527-637`): the UVM leaf mask, the client-shared subtree's leaves, then `intrTopEnMask` (the stall subtree 3) at the top |
| `LEAF_EN_SET(2)` 0xb81208, `LEAF_EN_SET(4)` 0xb81210, `TOP_EN_SET(0)` 0xb81608 | W 0x3, W 0x10000000, W 0xfffffffe | 18.34 each | lock release: `_intrEnableStall_TU102` (`intr_tu102.c:434-517`): the same leaves, then every top subtree except the non-stall one |
| `LEAF_EN_SET(4)` / `LEAF_EN_CLEAR(4)` | W 0x10000000 | 18.33 / 0.74 | `intrSetDisplayInterruptEnable_TU102` (`intr_tu102.c:1309-1331`): the display vector 156 alone |
| `TOP_EN_SET(0)` | W 0x1 | 18.34 (of 36.68) | `intrEnableTopNonstall_TU102` (`intr_nonstall_tu102.c:91-120`): the non-stall subtree 0 |
| `TOP(0)` 0xb81600 | R | 11.61 | `intrGetPendingNonStall_TU102` (`intr_nonstall_tu102.c:194-250`, the read at :233): pending subtrees; only non-stall subtrees are looked up through TOP |
| `LEAF(0)`, `LEAF_EN_SET(0)`, `LEAF(1)`, `LEAF_EN_SET(1)` | R | 8.50 each | the same function, `LEAF & LEAF_EN_SET` per non-stall leaf (:243-244) |
| `LEAF(4)`, `LEAF(2)`, `LEAF(6)` | R | 16.54, 8.72, 8.72 | stall servicing: `intrGetPendingDispLowLatencyAndStaticReg_TU102` (`intr_tu102.c:1175-1210`, leaf 4 at :1200), `_intrGetLeafStatus_TU102` (:833-880); the table loop of `intrGetPendingStallEngines_TU102` (:929-980) tests only the table's vectors and deliberately ignores the leaf enables (:964-969) |
| `LEAF(0)` / `LEAF(4)` | W 0x10 / W 0x10000000 | 4.25 / 1.36 | the ack, write 1 to clear: `intrClearLeafVector_TU102` (`intr_tu102.c:648-660`) writes `NVBIT(bit)` of the one vector serviced |
| `LEAF_TRIGGER` 0xb81640 | W vector | 0 | the environment self-test (vector 129) and `intrSetStallSWIntr_TU102` (`intr_swintr_tu102.c:40-49`) |

Two more readers matter for the design: `intrGetNonStallEnable_TU102` reads `TOP_EN_SET(0)` and
tests the non-stall bit (`intr_nonstall_tu102.c:40-80`, the reads at :65 and :68), and the RM
caches `LEAF_EN_SET(4)` and `(5)` once at state load as `uvmSharedCpuLeafEn`
(`intr_tu102.c:250-270`) and writes that cached value back at every release (:477-488).
`LEAF_EN_CLEAR`, `TOP_EN_CLEAR` and `LEAF_TRIGGER` are never read.

No register of the two pages has a read side effect: `IntrTree::read32` (`intr.rs`) only folds
vectors raised from other threads into the leaves, which the device can equally do before it
signals, and page 0 holds constants and latched values only.

## 2. The pages as sparse mmap areas of region 0

**What the specification says.** A mappable region comes with a file descriptor in the region
info reply (`vfio-user.rst:861-864`), `offset` is the base offset for its sparse areas
(`:875-877`), each area's offset is added to it for the `mmap()` call (`:943-947`), and "the
server is responsible for polling for client updates to memory mapped regions" (`:111-113`).
Sparse regions exist for exactly this split, registers trapped and a hot page mapped
(`:123-135`).

**The crate needs no change.** `ServerRegion` already carries `sparse_areas` and `mmap_fd` for any
region (`vendor/vfio-user-rs/src/lib.rs:1401-1405`); the region info reply adds the sparse-mmap
capability and sends the descriptor with it (`:1787-1860`, the `SCM_RIGHTS` send at `:1854`).
P21 uses the same path for region 1 (`device.rs` `regions_with`). The ioeventfd sub-regions of
the doorbell are a separate list (`io_fds`, `add_ioeventfd`) and do not interact. So there is no
patch `0003` for the crate.

**Cloud Hypervisor needs no new patch.** With 0005, `map_mmio_regions`
(`/mnt/vmstore/caraxes/ch-0005/pci/src/vfio_user.rs:299-392`) takes every region flagged
`VFIO_REGION_INFO_FLAG_MMAP`, mmaps each sparse area at `file_offset.start() + area.offset` with
the region's read/write protection (`:328-354`) and makes it a KVM memory slot at
`BAR base + area.offset` (`:371-381`); then it registers the ioeventfds (`:395`). Accesses
outside the areas stay VM exits that reach the server as `VFIO_USER_REGION_READ/WRITE`
(`read_bar`/`write_bar`, `:708-714`, through `region_read`/`region_write`, `:564-577`). BAR moves
keep the areas (`move_bar`, `:716-756`). One thing is not available: the slot is always
read-write (`create_user_memory_region(..., false, false)`, `:377-378`), so "reads from memory,
writes trapped" cannot be had without a patch, and the design has to take the guest's writes from
memory too.

**Declaration.** Region 0 gets `VFIO_REGION_INFO_FLAG_MMAP` next to READ and WRITE, file offset
0, the memfd as its descriptor and two sparse areas, `[0x0, 0x1000)` and `[0xb81000, 0xb82000)`.
The memfd is sized to the whole BAR (16 MiB, sparse: only the two pages ever get memory), so the
area offsets are the file offsets. The page size is the host's (4096, checked at start); the
chip data's tree base (`chips/tu106/chip.toml:49-50`, 0xb81000 + 0x800) and the absence of any
other modelled register in both pages are checked at start (§5). The doorbell at 0xbb0090 lies
in page 0xbb0000, which is not an area: its ioeventfd sub-regions keep working, and the MSI-X
table is not in play (the device offers MSI only, `device.rs` `irqs`).

## 3. Keeping the page and the model in step

The model state is the trapped path's (`intr.rs` `IntrTree`): 8 leaves, 8 leaf enables, the top
enable. In page mode it lives in `intr_page.rs` beside the page, and the reactor thread owns the
page's device side. Every register gets a display value, the word the device writes into the
page, chosen so that any store of the guest changes the word:

| Register | Page shows | A guest store is seen as | Applied as |
|---|---|---|---|
| `LEAF(i)` | pending bits \| TAG (bit 31) | the word lost TAG; the word is the mask | write 1 to clear |
| `LEAF_EN_SET(i)` | the kernel table's vectors of leaf i plus every bit the guest ever set (reported, not modelled; see below) | the word differs from the mask | learn new bits; a re-arm |
| `LEAF_EN_CLEAR(i)` | 0 | a word other than 0 | counted, dropped |
| `TOP(0)` | pending subtrees (leaf & enable), no TAG | a change (a store to a read-only register) | restored, noted |
| `TOP_EN_SET(0)` | top enable (subtree bits 0-3) \| bit 31 | the word differs | set bits 0-3; a re-arm |
| `TOP_EN_CLEAR(0)` | 0 | a word other than 0 | clear bits |
| `LEAF_TRIGGER` | 0xffffffff (vector field 0xfff, no vector) | another word | raise vector `v & 0xfff` |
| page 0 `BOOT_0/1/42` | the chip's constants | a change | restored, noted (writes are dropped when trapped) |
| page 0, every other word | what the guest stored (latched, as trapped) | nothing to do | nothing |

**Why TAG is safe.** Bit 31 of every leaf is vector 32i+31. The start-up check refuses page mode
if any stall or non-stall vector of the chip's kernel table (`chips/tu106/interrupts.toml`, the
answer to `INTERNAL_INTR_GET_KERNEL_TABLE`) or the display's vector ends in 31. Every reader of a
leaf tests the table's vectors bit by bit (`intr_tu102.c:929-980`, `:1200-1206`, `:737-743`,
`intr_nonstall_tu102.c:243-261`), the non-stall path masks the leaf with `LEAF_EN_SET`, which
never shows TAG, and the ack writes `NVBIT(bit)` of a table vector, so no guest store carries
TAG. `TOP` is computed without TAG. For `TOP_EN_SET` the guest's only read tests the non-stall bit
(`intr_nonstall_tu102.c:65`), and its stores (0xfffffffe, 0x1, the retrigger masks of
`intr_tu102.c:1094-1160`) never equal a word with bit 31 and nothing above bit 3.

**Leaf enables are reported, not modelled** (changed after the first run on vm0, §9). The
guest enables its vectors one store per vector at state load (`intr_tu102.c:340`, `NVBIT(leafBit)`
in a loop), eight stores to one register within nanoseconds. A scan sees only the last, so a
modelled enable kept one bit of eight: `LEAF_EN_SET(0)` read 0x1 instead of 0xff in the guest,
the non-stall vectors never reached `TOP`, and Mutter and Weston stalled after their first
frame (run B2). The page therefore shows, from the start, every vector of the kernel table
(`IntrPage::enable_vectors`, the stall and non-stall vectors of `interrupts.toml`) and learns
every further bit the guest sets (the self-test's vector 129); clears are counted and dropped.
Delivery never depended on the enables in the trapped path either (`IntrShared::raise` signals
regardless), and the guest's stall servicing ignores them (`intr_tu102.c:964-969`); what the
toggles do in hardware, re-arm the interrupt at a lock release, is kept as its own rule: any
detected set store (`LEAF_EN_SET` or `TOP_EN_SET`, one of which every release writes) while a
table vector is pending signals the MSI again.

**The scan.** Every reactor iteration compares the 28 watched words with the display values it
wrote last (one load each, no lock; about 20 ns). On a difference, or when another thread raised
a vector, it takes the page lock and:

1. snapshots the changed words;
2. applies them in the order of a lock cycle: `LEAF_EN_CLEAR` and `TOP_EN_CLEAR` first, then
   `LEAF_EN_SET` and `TOP_EN_SET`, then the acks, then triggers, then the vectors raised by other
   threads (`IntrShared`, the host event thread and the display clock);
3. decides the MSI: for a vector raised by another thread or triggered that was not pending in
   the page (a raise of a vector still pending is coalesced and remembered; the ack that clears
   it pends it again with one MSI, so an event after the guest's read is not lost: run B1
   without this turned 14825 host events into as many MSIs in one Mutter window); and for a
   re-arm (a set store while a vector is pending);
4. writes the new display values with a compare-and-swap from the value it read, so a guest store
   that lands in between is not overwritten but found by the next iteration;
5. then, with the page written, signals the MSI eventfd. The page stores come before the
   `write(2)` to the eventfd in program order, x86 keeps stores in order, and KVM injects the MSI
   after that write: the guest's handler reads the new status.

**The raise path.** `IntrShared::raise` (the host event thread, `mirror.rs`, and the display
clock, `display/engine.rs`) keeps its coalescing, but in page mode it does not write the MSI
itself: it marks the vector and writes a wake eventfd that sits in the reactor's epoll set, so a
hybrid reactor that blocked wakes up and the page is written before the MSI.

**The reactor.** Page mode needs `CARAXES_DOORBELL_POLL=spin` or `hybrid` (and therefore the
token pool or the doorbell ioeventfd, which the reactor reads). In spin the page is scanned about
every 0.2 us (one `epoll_wait(0)` of about 170 ns, doorbell-poll.md). In hybrid a page change or
a wake counts as an event and restarts the spin window (`CARAXES_DOORBELL_POLL_IDLE_US`, 200 us),
and the reactor never blocks for longer than `CARAXES_INTR_PAGE_SLEEP_MS` (default 1): acks and
enable writes that come after the window are then seen within 1 ms, and a raise wakes it at once.

**Trapped accesses to the pages** (a client that did not map the areas) go to the page memory
too (`bar0.rs`), so the reactor sees them like a guest store and the model has one writer.

## 4. What stays trapped

- **The display FE interrupt status page (0x611000)**: 88.8 reads per Mutter frame, nearly all
  `NV_PDISP_FE_RM_INTR_DISPATCH` (0x611ec0, `display/engine.rs:47`). Its reads advance the
  display model's clock (`display/mod.rs:1232-1236`, `catch_up` before the read), and its
  status registers are write-1-to-clear with the model's own state behind them. It could become a
  third page only if the display clock thread wrote its status into the page itself; that is the
  next step (phase 2), not this change. Done since as `CARAXES_DISPLAY_PAGE=1` (§10).
- **The usermode page (0xbb0000)**: the doorbell must trap into its ioeventfd sub-regions, and
  `TIME_0/1` (14.7 reads per frame) are answered from the clock at the moment of the read.
- **`NV_PCFG` (0x88000)**: writes land in config space (`bar0.rs` `WriteEffect::Mirror`). One
  of them is the exception: the MSI re-arm (below).

**The MSI re-arm through an ioeventfd (`CARAXES_MSI_REARM_IOEVENTFD=1`, 2026-09-25).** The RM
writes `NV_XVE_CYA_2` (BAR0 0x88704 through the `NV_PCFG` mirror) once per interrupt service.
The value is ignored (kernel-rm-hw-surface.md:119, `kernel_bif_gm107.c:136-151`). With the page
it was the last trap of every interrupt: 140 to 160 per second in every CUDA workload, 55 per
second at idle, and 9.6 per Mutter frame (optimization-backlog.md item 4).
- **What the trapped write does in the model.** `Bar0::write` hands it to the config mirror. At
  config offset 0x704 no bit is writable, so the config space does not change. It does not touch
  the interrupt tree. The tree's MSI edge comes from the tree's own writes, which here are the
  page's stores that the reactor applies.
- **The steps after it.** What follows is `Device::after_bar0_write` (`device.rs`): the notes,
  a function-level reset if one was requested, a pending MSI of the model, and the MSI enable.
  Since 356dd4e4 a write that lands in the config mirror skips the P21 check and the BAR1 pool's
  poll: under Sunshine's capture that poll, run after every re-arm with the device lock held,
  kept the display clock from latching flips (sottr-stage.md §11).
- **With the switch.** The device offers the dword as one more ioeventfd sub-region of BAR0: 4
  bytes, no datamatch, in the same `VFIO_USER_DEVICE_GET_REGION_IO_FDS` reply as the token
  pool's (patch 0005). The token pool's reactor reads its eventfd beside the tokens' eventfds
  (`spawn_token_reader_hooked`; a wakeup never goes to a token). `Device::msi_rearm_ioeventfd`
  then does what the trapped write does: the same config mirror write, the same steps after it,
  and one trace event with the number of writes the eventfd counted. Several writes between two
  wakeups count as one re-arm, because each would repeat the same idempotent steps.
- **Fail closed.** The switch needs `CARAXES_DOORBELL_TOKENS=N`, or the device does not start. A
  client that does not register the sub-region (no patch 0005, or a refusal) keeps trapping the
  write, and the trapped path is unchanged. The exit summary has a line
  `msi re-arm ioeventfd: W guest write(s) in N wakeup(s)`. Test:
  `device::tests::the_msi_rearm_ioeventfd_is_read_by_the_token_reactor`.
- **Measured on vm3** (`caraxes/runs/opt-msi-ioeventfd-2026-09-25/`). Interrupt page, BAR1 pool,
  BAR2 memfd, token pool and hybrid reactor, with the re-arm trapped (the `on` run of
  `caraxes/runs/opt-memfd-pool-2026-09-25/`) and through the eventfd (`on-rearm`), whole-process
  windows (`workload-rates.md` there):

  | Workload | re-arm traps per second, trapped | through the eventfd | all traps per second, trapped | through the eventfd |
  |---|---|---|---|---|
  | rlprobe, 50 episodes | 141 | 0 (165/s through the eventfd) | 508 | 456 |
  | PPO, 30 s | 153 | 0 (154/s through the eventfd) | 423 | 314 |
  | idle census window, per 100 ms | 10.2 | 0 | 38.6 | 25.6 |

  The idle row is 102 re-arms per second; optimization-backlog.md's census had 55 and coverage-map.md 331 per second, because idle MSIs are other VMs' work forwarded and follow their load.


  - The whole census trace of `on-rearm` has no write to 0x88704.
  - The eventfd counted 26017 writes over the session.
  - The cuda gate is 10 of 12 (the gate script's count: the declared-unsupported cuda-gdb row is among the 12; documents that count only the 11 scored rows write the same result as 10 of 11, testing.md section 3, cuda) as before (cuda-managed), and there was 0 host Xid.
  - What still traps per second is the PCI config capability walk (200-290 reads), the MMU
    invalidate and flush page, and the GSP queue (optimization-backlog.md items 3, 6 and 7).
  - Mutter was not measured (vm0 not borrowed). The expected saving is the 9.6 re-arm traps per
    frame of §9, about 0.3-0.5 ms per frame.
- **The VF MMU invalidate and fault-buffer page (0xb83000)** and every other register with a
  model behind it: they act synchronously.
- **BAR2**: Mutter's 71 PTE writes and 23 reads per frame are page-table traffic of the guest RM.
  The count depends on the series: the BAR2 writes per Mutter frame differ by series, not by method: 71 (70.85) is series F on the 610 guest without the interrupt and display pages (`caraxes/runs/display-f-2026-09-24/f3b-traps-mutter-fresh/trap-table.txt`, "BAR2 writes, all"), 73.5 is run B3 of intr-page.md section 9, and 73.63 the 615 guest with both pages in display-615.md section 5.

## 5. Fail-closed rules

Start-up (page mode refuses to start, the device exits with the reason):

- `CARAXES_INTR_PAGE=1` without `CARAXES_DOORBELL_POLL=spin|hybrid`: the acks would never be seen.
- no boot model; a tree base that is not page-aligned or a tree larger than a page; the host page
  size not 4096;
- a modelled register in either page other than the tree and the three boot constants (every
  dword of both pages is asked `BootModel::owns_register`);
- a table vector (stall or non-stall) or the display vector with bit 31 in its leaf.

At run time, each logged once to the device log and the trace, counted in the exit summary, and
followed by an MSI when a vector is pending ("raise the interrupt again"):

- a store to `TOP` or to a boot constant: restored;
- a store with TAG in a leaf or in `LEAF_EN_SET`: TAG is ignored, the rest applied (in
  `TOP_EN_SET` bit 31 is ordinary: the guest's 0xfffffffe carries it, and only bits 0-3 exist);
- a trigger outside the 256 vectors: ignored;
- a store to a word of the tree that is no register: restored to 0 (the trapped path reads 0);
- the MSI eventfd missing or MSI disabled: the vectors stay pending, as when trapped.

What cannot be told apart, and what it costs:

| Case | Effect | Bound |
|---|---|---|
| two acks of one leaf between two scans | the first ack is lost, the vector reads pending once more, the guest services it again (a spurious non-stall notify or an empty stall service) and acks again | converges at the next ack |
| the guest reads a leaf between its ack and the next scan | it reads the mask it just wrote, so the same spurious second service | one reactor iteration (0.2 us in spin) |
| `LEAF_EN_SET` of exactly the current mask while a `LEAF_EN_CLEAR` of the same leaf is unscanned (acquire and release within one scan) | the clear is applied, the set is invisible, the leaf stays disabled | the next release, or the watchdog |
| two `TOP_EN_SET` stores between two scans (0xfffffffe then 0x1 happen back to back, `intr_tu102.c:504` and `intr_nonstall_tu102.c:109`) | the first is lost: the stall subtree stays disabled | the next release, or the watchdog |
| two triggers of different vectors between two scans | the first vector is not raised | not bounded; the guest's triggers are its self-test and the SW stall interrupt, neither seen per frame |

**The retrigger watchdog** makes the bounded cases late instead of lost: when an enabled vector
(`leaf & leaf_en`) has been pending for `CARAXES_INTR_PAGE_RETRIGGER_MS` (default 10 ms) with no
guest store to its leaf and no MSI in that time, the reactor restores the top enable to every
subtree and signals the MSI again. The guest's stall servicing reads the leaves without looking at
the enables (`intr_tu102.c:964-969`), so the retriggered MSI is serviced. The watchdog sends at
most one MSI per period and counts them; a count above zero in a measurement is a finding.

## 6. Expected effect and latency

Traps per frame, from §1: Mutter f3b 661.5 - 437.1 = **224.4**, f3 711.7 - 460.6 = **251.1**,
Weston f4 157.0 - 81.4 = **75.6** (measured in §9: Mutter 229.8, Weston 103.0). At 23-27 us per trap (mapping.md, c0/c1, a quiet host; under load a trap costs up to 35 us, optimization-backlog.md section 4) that is 10-12 ms of
vCPU exits less per Mutter frame, out of 15-19 ms. The operator's "below 100" needs two more
steps: the display FE page (-91 per Mutter frame, §4) and Mutter's BAR2 PTE churn (-94);
together about 40 per frame.

Latency of one interrupt, raise to handler:

| Step | Trapped (today) | Page, spin | Page, hybrid |
|---|---|---|---|
| raise to MSI eventfd | direct `write(2)` from the raising thread, ~1-2 us | wake `write(2)` ~1-2 us, reactor sees it within one iteration (0.2 us), page stores and MSI `write(2)` ~2 us | as spin inside the 200 us window after the last event; else the blocked reactor's wake-up first (the scheduler wake-up of doorbell-poll.md, about 10 us) |
| handler's reads (TOP, 2-3 leaves, 2 enables per pass, two passes) | 10-14 traps at 23-27 us: 250-380 us | memory loads, well under 1 us each | same |
| handler's writes (ack, lock acquire and release, ~12) | 12 traps: ~300 us | memory stores | same |
| ack seen by the device | at the trap | within 0.2 us | within the window; else within 1 ms |

The budget for a hybrid reactor: the handler runs within microseconds of the MSI, inside the
200 us window the MSI opened, so in the common case hybrid equals spin; the 1 ms sleep bound
covers acks and enables that come later, the 10 ms watchdog covers the ambiguous cases.

## 6a. Next steps: the two remaining sources

(For CUDA launches the completion interrupt is not on the path: libcuda's auto wait spins on the
semaphore. launch-path.md §2 measures the completion side, host event to raise 2 to 4 us and raise
to MSI 11 to 15 us at the median, and §4 adds the interrupt gate for other clients' events,
`CARAXES_INTR_GATE_MS`, backlog item 5.)

After this change a Mutter frame still traps about 225 times (f3b). Two sources make up 185 of
them, and each needs its own change:

| Next step | Traps per frame (f3b / f3 / f4) | What it takes | Estimate |
|---|---|---|---|
| **Display FE interrupt page** (0x611000): `NV_PDISP_FE_RM_INTR_DISPATCH` 0x611ec0 (70.7 reads per Mutter frame), `FE_RM_INTR_STAT_CTRL_DISP` 0x611c30, `FE_RM_INTR_STAT_HEAD_TIMING` 0x611c0c, `FE_EVT_STAT_HEAD_TIMING` 0x61180c (write 1 to clear), `FE_EVT_STAT_AWAKEN_WIN` 0x611858 (`display/engine.rs:44-51`) | 91.3 / 113.6 / 56.3 | a third sparse area. Today a read of these registers advances the display clock first (`catch_up`, `display/mod.rs:1232-1236`), so the status is computed at the moment of the read. With the page, the display clock thread (`display/engine.rs`) must write the status into the page at every event (vblank, awaken, channel completion) before it raises vector 156, and the reactor must apply the write-1-to-clear stores with the same visibility rules as here (a sentinel or tag bit per register, checked against `dev_disp.h`). The page must not hold any other display register with a synchronous answer (the check of §5 on 0x611000-0x611fff). | Mutter 225 to about 134, Weston 76 to about 20; 1-2 days, most of it the display engine's event path |
| **Mutter's BAR2 page-table churn** (region 3): 71 PTE/PDE writes (pages 0x23000 and 0x39000 of the BAR2 window, each PTE written twice) and 23 reads per frame, plus 6 MMU invalidate writes | 93.8 / 94.3 / 0 | first find which guest mapping the RM rebuilds per frame for Mutter and not for Weston (the PTEs map FB 0x168c50000 onwards; `fb 0x168dd09a8` PDEs), then either keep that mapping alive (a guest-side cause: map once, which is outside the device) or serve the BAR2 window's table pages as memory with the write-through shadow of the mapping package seeing the stores at the next MMU invalidate (the invalidate stays trapped and is the natural sync point, mapping.md). | Mutter 134 to about 40; 1 day to identify the mapping, 2-3 days for a memory-backed BAR2 table window with its consistency rules |

With both, a Mutter frame is left with the usermode page (`TIME_0/1`, 14.7 reads, answered from
the clock), `NV_PCFG` writes (7.4), the MMU invalidate (6) and the doorbell (0 with the token
pool): about 30-40 traps, below the operator's 100.

**State of the first row (2026-09-24 evening):** implemented behind `CARAXES_DISPLAY_PAGE=1`
(§10), not yet measured. The same package found the Xid 31 at GNOME's start to be a GPPut store
that the host channel can see before the doorbell barrier, and added a barrier to that store
(display-e.md §9). The BAR2 row is open here. docs/caraxes/bar2-memfd.md (`CARAXES_BAR2_MEMFD=1`)
serves the BAR2 window from memory and may already take most of those 94 accesses, but this
series does not include it.

**Measurement on vm0** (about 40 minutes, the card and vm0 from the coordinator). Freeze the
binary of this branch, then run the series from the worktree root:

```
cp target/fepage/release/vfio-user-nvgpu /mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-fepage
caraxes/runs/display-page-2026-09-24/series.sh B C
```

`env.sh` there is the interrupt page series' environment (vm0, slot 1, 2 GiB, host execution,
display engine, Cloud Hypervisor 0001-0006, `CARAXES_DOORBELL_TOKENS=64`,
`CARAXES_DOORBELL_POLL=hybrid`) with that binary. Configuration B is `CARAXES_INTR_PAGE=1` (the
reference, B3's configuration with the GPPut barrier); C adds `CARAXES_DISPLAY_PAGE=1`. Per
configuration it takes:
- five fresh GNOME starts (`guest/gnome-restart.sh`: gdm3 stopped, started, 20 s), each its
  own window with `host-xid.txt` and the count of `GPPut of a mirrored channel held` events;
- the Mutter and Weston trap windows (`run.sh --full`, `trap-table.py --pages`, wlframes'
  `FRAMES`/`PRESENTED` lines for the frame callbacks);
- the display gate (`caraxes-gate.sh display`, 11 stages);
- the device's summary lines (`intr page:`, `display page:`).

Pass criteria:
- C's page 0x611000 rows are gone from the trap table; Mutter is about 132 traps per frame
  (B about 230), Weston about 20 (B about 103).
- Callbacks are at 59.7 Hz or better.
- The gate passes 11/11.
- `display page:` shows as many markers as `CORE_RESET` acks and `inconsistencies: none`.
- There are 0 host Xid lines in the ten GNOME starts.

## 7. Switches

| Variable | Default | Meaning |
|---|---|---|
| `CARAXES_INTR_PAGE` | off | `1`: offer page 0 and the VF interrupt tree page as memory (needs `CARAXES_DOORBELL_POLL=spin\|hybrid`) |
| `CARAXES_INTR_PAGE_SLEEP_MS` | 1 | hybrid: the longest blocking wait of the reactor while the page is on |
| `CARAXES_INTR_PAGE_RETRIGGER_MS` | 10 | the watchdog period; 0 turns the watchdog off (also for the display page's markers) |
| `CARAXES_DISPLAY_PAGE` | off | `1`: offer the display front-end page 0x611000 as memory too (needs `CARAXES_INTR_PAGE=1` and `--display`, §10) |
| `CARAXES_MSI_REARM_IOEVENTFD` | off | `1`: the MSI re-arm write (`NV_XVE_CYA_2`, BAR0 0x88704) as an ioeventfd read by the token pool's reactor (needs `CARAXES_DOORBELL_TOKENS=N`, §4) |
| `CARAXES_BAR2_FLUSH_DEFER_US` | off | microseconds (2 to 5000, `on` = 2000): a BAR2 flush read owes its memfd sync instead of running it; paid by the next consumer or by the reactor at the bound (needs `CARAXES_BAR2_MEMFD=1` and `CARAXES_INTR_PAGE=1`, §11.2) |
| `CARAXES_INVALIDATE_PAGE` | off | `1`: offer the VF MMU invalidate page 0xb83000 as memory; the reactor runs each `TRIGGER` (needs `CARAXES_INTR_PAGE=1` and the boot model, §11.3) |
| `CARAXES_INTR_PAGE_VBLANK_SPIN_US` | 0 | microseconds (0 to 16000): the reactor spins from this long before each display tick (display page only, §11.4; no measured gain) |

The exit summary adds `bar2 flush deferral: ...`, `invalidate page: ...`, `display page
delivery: ...`, `reactor wake-up after a display raise ...` and the display clock's timing in
the `display engine:` line when those are on (§11). It adds `intr page: ...` with the scans that found work, the guest stores applied
per register kind, the vectors raised through the page, MSIs by cause (raise, edge, watchdog),
compare-and-swap retries, and the inconsistencies.

## 8. State

**Code** (`crates/vfio-user-nvgpu`):

- `src/intr_page.rs` (new): the memfd and its two mappings, the model, the display values, the
  scan (`IntrPage::scan`), the slow checks and the watchdog (`IntrPage::tick`), the start-up
  checks (`IntrPage::check`), the reactor hook (`PageHook`), the exit summary.
- `src/intr.rs`: `IntrShared::raise` hands the vector to the reactor in page mode
  (`attach_page`, a wake eventfd) instead of writing the MSI; `signal_msi` is the MSI write both
  paths use.
- `src/poll.rs` (additive): `Hook` and `Reactor::spawn_hooked`; the wake eventfd joins the epoll
  set, the hook runs every iteration, a blocking wait is bounded by the hook.
- `src/device.rs` (additive): `regions_with_pages`, `Bar0Areas`, `spawn_token_reader_hooked`,
  `spawn_doorbell_reader_hooked`.
- `src/bar0.rs`, `src/boot.rs` (additive): trapped accesses to the two pages go to the page
  memory, a reset resets the page, `Bar0::model_owns` and `BootModel::owns_register` for the
  start-up check.
- `src/main.rs` (additive): the switch, the refusals of §5, the page before the server, the hook
  on the token or ioeventfd reactor, the summary line.
- `caraxes/tools/trap-table.py --pages`: the per-page rows of §1 for the measurement.

**Tests** (`cargo test -p vfio-user-nvgpu`, 173 passed, 12 of them new, after the fixes of §9):

| Test | What it shows |
|---|---|
| `intr_page::switch_and_settings` | the three variables, refusals of settings without the switch and of bad numbers |
| `intr_page::initial_displays_and_page_zero` | the initial page: TAG in the leaves and `TOP_EN_SET`, 0 in the clears, the trigger sentinel, the boot constants; page 0's lock writes need no work |
| `intr_page::raise_writes_the_page_then_the_msi_and_the_ack_clears_it` | a raise from another thread is coalesced before the fold, the page shows the bit before the MSI eventfd is written, an enable makes the trapped path's edge, the ack `NVBIT(4)` through the page clears it, the next raise signals again |
| `intr_page::enables_are_reported_and_a_release_rearms` | the per-vector enable loop of state load before one scan still shows the whole table, a bit outside the table is learned, clears are dropped; a lock cycle between two scans with a pending vector re-arms (one MSI), a release with nothing pending does not; reset keeps the table's enables |
| `intr_page::raises_while_pending_coalesce_until_the_ack` | 100 raises of a pending vector cost no MSI; the ack pends it again with one MSI; the next ack clears it |
| `intr_page::fail_closed_cases` | stores to `TOP`, a boot constant and a tree hole are restored, triggers outside the vectors or at a tag are ignored, a leaf store with TAG applies the rest, MSI off writes nothing, each counted once, reset restores everything |
| `intr_page::watchdog_turns_a_lost_enable_into_a_late_interrupt` | 0xfffffffe and 0x1 to `TOP_EN_SET` before one scan lose the stall enable; the watchdog signals after 10 ms, at most once per period, restores the top enable, stops after the ack; off means never |
| `intr_page::a_store_racing_the_write_back_is_kept` | a guest store between the snapshot and the write-back survives the compare-and-swap and is applied by the next scan |
| `intr_page::start_up_refusals` | a vector at a tag bit, a tree larger than a page, a modelled register in either page, an unaligned tree base |
| `intr_page::tu106_pages_are_free` | the TU106 boot model owns nothing in page 0 or the tree page but the tree and the constants; no table vector sits at a tag |
| `intr_page::hook_in_the_reactor` | in a real spin and hybrid reactor: a raise wakes a blocked hybrid reactor, the MSI comes after the page, a guest ack through the page is applied |
| `device::bar0_areas_make_region_0_mmapable_over_two_pages` | region 0 with MMAP, file offset 0, the two areas and the fd; areas outside the BAR are refused; the token pool's ioeventfd sub-regions at 0xbb0090 still declare next to the areas |

`cargo build --release -p vfio-user-nvgpu`, the workspace clippy with and without
`--all-features` (`-D warnings` and the allowances of `tools/check.sh`) and `cargo doc` with
`-D warnings` are clean (`CARGO_TARGET_DIR=target/intr`).

**Measurement**: §9.

## 9. Measurement on vm0 (2026-09-24, 15:16-16:03)

Environment: `caraxes/runs/intr-page-2026-09-24/env.sh` (run directories `A-page-off`, `B1-page-on-uncoalesced`, `B2-page-on-modelled-enables`, `B3-page-on`; series F's vm0 configuration: slot 1,
2 GiB profile, host execution, display engine) plus the token pool (`CARAXES_DOORBELL_TOKENS=64`),
the hybrid reactor (`CARAXES_DOORBELL_POLL=hybrid`, 200 us, pinned automatically) and Cloud
Hypervisor 0001-0006 (`/mnt/vmstore/caraxes/vm1/ch-0006-v.sh`); `series.sh` per configuration:
up, probe, display-up, the Mutter and Weston trap windows (wlframes 14 s, `trap-table.py
--pages`), the display gate, down. Frozen binaries in `/mnt/vmstore/caraxes/vm0/`:
`vfio-user-nvgpu-intrpage` (commit 00cc12c, runs A, B1), `-intrpage2` (coalescing, B2),
`-intrpage3` (commit 08651c9, B3).

| Run | Build | Mutter: traps per frame / callbacks | Weston: traps per frame / callbacks | Gate | Host Xid |
|---|---|---|---|---|---|
| A, page off | 00cc12c | **638.9** / **48.83 Hz** | 155.4 / 59.71 Hz | pass 11/11 (compositor 59.87 Hz, present 55.1 FPS, fence median 0.56 ms) | 0 |
| B1, page on | 00cc12c | stalled after the first frame (0 intervals) | stalled | fail (compositor, capture, present, events) | 1: Xid 31 `GPCCLIENT_PROP_0` at GNOME's start, device pid (the known one of display-e.md §9) |
| B2, page on | + coalescing | stalled after the first frame | stalled | fail | 0 |
| B3, page on | 08651c9 | **229.8** / **59.71 Hz** | **103.0** / 59.78 Hz | not run (time) | 0 |

B3 in detail (per frame; A in brackets): Mutter BAR0 writes 18.7 (368.5), reads 111.6 (170.1),
BAR2 96.5 (97.2); pages 0x000000 and 0xb81000 have no trapped access left (in A
182.37 + 2.42 + 169.92 + 73.15 = 427.9 per frame); what remains is the display FE page
0x611000 (95.5 reads, 2.0 writes), BAR2 (73.5 writes, 23.0 reads), the usermode page (11.8
`TIME` reads), `NV_PCFG` (9.6), the MMU invalidate (10.0). Mutter now presents at the full
59.7 Hz: frame callbacks 48.83 to 59.71 Hz, max interval 102.6 ms. Weston's rate was already 60 Hz;
its traps go from 155.4 to 103.0 (expected 76: the display page's reads rose from 52.8 to 80.7 per
frame with more interrupts serviced per frame). The device's summary for B3 (254.6 s):
106222 scans with work, slowest 89.2 us; 30624 acks, 28869 `LEAF_EN_SET`, 45237 `LEAF_EN_CLEAR`,
42765 `TOP_EN_SET`, 22145 `TOP_EN_CLEAR`; 32602 raise folds (8262 coalesced while pending, 6283
pended again at the ack); MSIs 29926 for raises, 17286 re-arms, 26 by the watchdog; 4482
compare-and-swaps lost to a guest store and applied at the next scan; no inconsistency. The
hybrid reactor used 5.1 % of a core (13.1 s in 254.6 s) with the page on.

What B1 and B2 showed, and what changed (commit 08651c9):

- **B1: raises were not coalesced.** In the trapped path a raised vector stays in `IntrShared`
  until the guest's next trapped tree access; with the page the reactor folds it within
  microseconds, so every host event became an MSI (14825 in the Mutter window; 175145 MSIs over
  the run). Now a raise of a vector the page still shows pending costs no MSI and is pended again
  when the guest acks it.
- **B2: modelled leaf enables lose the guest's enable loop.** A guest read of the page during
  the stall (`guest/peek-tree.py`, read-only, through `resource0`) showed `LEAF_EN_SET(0)` 0x1,
  `LEAF_EN_SET(6)` 0x80, leaf 0 bit 4 pending and `TOP` 0: of the eight per-vector stores of state
  load only one survived per register, so the non-stall vectors never reached `TOP`. The leaf
  enables are now reported (§3).

Pending: the display gate with the page on (B3 ran without it because of the host upgrade
window), a second pass of A and B3 for the run-to-run spread (series F had Mutter at 30-41 Hz
between runs; A here was 48.8 Hz), and spin instead of hybrid.

The copy rows with the page on and off were measured on vm0 on 2026-09-24 (18:24-19:07):
benchmark.md, "The interrupt page removes the copy stall". Pinned 16 MiB copies go from 6.3 to
11.9 GB/s (native 12-13), d2d 16 MiB from 14.6 to 90 GB/s, PPO gains 15-30 percent; 0 Xid.

## 10. The display front-end page (`CARAXES_DISPLAY_PAGE=1`)

Written 2026-09-24 (evening) for the first row of §6a. Implementation:
`crates/vfio-user-nvgpu/src/display/fe_page.rs` (new), the display engine's status moved behind
it (`display/engine.rs` `IrqHome`, `Engine::attach_page`), the display clock's driver
(`Ticker::set_driver`, `Device::display_tick`), the reactor hook (`intr_page.rs`
`PageHook::with_display`). Default off; it needs `CARAXES_INTR_PAGE=1` (the same memfd and
the same reactor) and `--display`.

**Design in five sentences.** BAR0 page 0x611000 becomes a third sparse mmap area of the
interrupt page's memfd, and the display's interrupt status (`FeIrq`: `EVT_STAT_HEAD_TIMING`,
`RM_INTR_EN_HEAD_TIMING`, `AWAKEN_WIN/OTHER`) moves out of the engine into the page's own
lock, so the reactor can apply the guest's stores without the device lock. Because the guest's
status reads no longer trap and so no longer advance the clock, the display clock thread
runs the engine's catch-up itself at every tick (under the device lock: latches, notifiers,
`LAST_DATA`, AWAKEN), writes the page details first and summaries last, and only then raises
vector 156, whose MSI the reactor writes after the tree page (§3). The reactor scans the 28
watched words every iteration and applies `LAST_DATA_RESET` and the other write-1-to-clear
acks and the vblank enables. Two registers carry a tag bit (bit 31, no field) so their acks
change the word. The window AWAKEN, whose ack is an echo that memory cannot see, is published
with a CORE marker whose `CORE_RESET` acknowledges exactly the window bits shown. Everything
else in the page stays latched memory (as trapped), and the other display pages (core and
window USERD, `RG_DPCA`, `FE_CORE_HEAD_STATE`) stay trapped.

### 10.1 What the guest touches in the page

B3 (intr-page-2026-09-24, interrupt page on), Mutter window, 597 frames; Weston window, 600
frames; values from the full trace:

| Register (dev_disp.h v03_00) | Access, value | Mutter per frame | Weston per frame | Reader |
|---|---|---|---|---|
| `FE_RM_INTR_DISPATCH` 0x611ec0 | R 0x0 / R 0x8 (head 3) | 54.61 / 24.08 | 65.53 in all | `kheadReadPendingRgLineIntr_v03_00`, `kheadReadPendingVblank_v03_00` (kern_disp_0300.c:434-525), once each per head (4 heads) and pass, and `kdispReadPendingVblank_IMPL` again in every loop step of `kdispServiceLowLatencyIntrs_KERNEL` (kern_disp.c:1618, :1673) |
| `FE_RM_INTR_STAT_CTRL_DISP` 0x611c30 | R 0x0 / R 0x100 (AWAKEN) | 6.83 / 1.00 | 6.19 | `kdispHandleWinSemEvt_v04_01` (kern_disp_0401.c:41-51) and `kdispServiceAwakenIntr_v03_00` (kern_disp_0300.c:715-747) per pass |
| `FE_RM_INTR_STAT_HEAD_TIMING(3)` 0x611c0c | R 0x2 | 6.02 | 6.00 | after a DISPATCH hit |
| `FE_EVT_STAT_HEAD_TIMING(3)` 0x61180c | R 0x2, W 0x2 | 1.00, 1.00 | 1.00, 1.00 | `kheadResetPendingLastData_v03_00`: reads, discards, writes `LAST_DATA_RESET` |
| `FE_EVT_STAT_AWAKEN_WIN` 0x611858 | R 0x40, W 0x40 (window 6) | 1.00, 1.00 | 1.00, 1.00 | `kdispReadAwakenChannelNumMask_v03_00` (:542-590), `_kdispResetAwakenChannelNumMask` (:597-624) |
| `FE_EVT_STAT_AWAKEN_OTHER` 0x61185c | R 0x0 | 1.00 | (in the rest) | the same call, core class |
| **the page** | | **95.54 R + 2.00 W = 97.5** | **80.72 R + 2.00 W = 82.7** | |

Over the whole Mutter trace the only other writes are one `RM_INTR_EN_HEAD_TIMING(3)` store of
`LAST_DATA` (the vblank enable) and one `AWAKEN_OTHER` `CORE_RESET` (the modeset's core
notifier). No other register of 0x611000-0x611fff is read or written.

### 10.2 The watched words and what shows there

| Register | Page shows | A guest store is | Applied as |
|---|---|---|---|
| `EVT_STAT_HEAD_TIMING(h)`, 8 | pending \| TAG (bit 31) | a word without TAG (the guest writes `LAST_DATA_RESET`, `RG_LINE_*_RESET`, `VBLANK_RESET`: kern_disp_0300.c:467-488, :527-539) | write 1 to clear |
| `RM_INTR_EN_HEAD_TIMING(h)`, 8 | the enable | a changed word (a store of the same value is a no-op) | the enable; the clock's vblank flag (`Ticker::set_vblank`) |
| `RM_INTR_STAT_HEAD_TIMING(h)`, 8 | `evt & en` | read-only | restored, counted |
| `RM_INTR_DISPATCH` | heads with `evt & en` | read-only | restored, counted |
| `RM_INTR_STAT_CTRL_DISP` | `AWAKEN` while a marker is out | read-only | restored, counted |
| `EVT_STAT_AWAKEN_WIN` | the window bits of the outstanding marker, else 0 | the echo is invisible; another word means a pass echoed an earlier marker's bits | restored, counted (the residual case below) |
| `EVT_STAT_AWAKEN_OTHER` | `CORE` while a marker is out \| TAG | `CORE_RESET` (0x1) | the marker's ack: the shown window bits and the core awaken are cleared |

**Why the window AWAKEN needs a marker.** All 32 bits of `AWAKEN_WIN` are channels
(dev_disp.h:271-278), and the guest writes back exactly the mask it read (kern_disp_0300.c:
563-575 builds it from the 32 bits, :611-624 writes it). The page shows that mask and the guest
stores the same word, so the ack is never seen, and no bit is free for a tag: a tag bit is part
of the mask and comes back with the echo. Getting the ack wrong has visible effects. Each window
awaken is one `NVKMS_EVENT_TYPE_FLIP_OCCURRED` (nvkms-evo.c:4994-5009). Each event completes the
oldest pending nvidia-drm flip of the head (nvidia-drm-modeset.c:642-690). So a lost awaken
stalls the compositor, and a repeated one completes the next flip early.

**The marker rule.** When awakens wait and no marker is out, the page shows them in
`AWAKEN_WIN`, `CORE` in `AWAKEN_OTHER` and `AWAKEN` in `STAT_CTRL_DISP`, in that store order.
`kdispServiceAwakenIntr_v03_00` reads `STAT_CTRL_DISP`, then `AWAKEN_WIN` (and acks it with the
echo), then `AWAKEN_OTHER`, and acks `CORE` with 0x1 (kern_disp_0300.c:726-748). That last
store changes the word (the page shows `CORE | TAG`), and it follows the window read in the
same call. So it acknowledges exactly the window bits shown: they are frozen while the marker is
out, and awakens that come meanwhile wait for the next marker, raised when this one is acked.
The extra core awaken does nothing in the guest. `_kdispHandleAwakenChnMask` finds the core
channel in use and notifies its event list (kern_disp_0300.c:680-694), and that list is empty:
NVKMS registers its completion callbacks on the layer channels (nvkms-evo.c:5019-5049), and
nothing else on the core channel. The core notifier's own `WRITE_AWAKEN` (engine.rs
`core_update`) is the same bit and rides on the same marker.

**The residual window of the marker rule.** A second display pass that starts after the
guest's `CORE_RESET` and before the reactor has seen it reads the old window bits again, handles
them a second time, and echoes them over the next marker's bits. The echo is seen and counted
(`an AWAKEN_WIN store that is not the echo`), but the extra flip event has already reached the
guest. The window is one reactor iteration, because a hybrid reactor spins while a marker is
out (`FePage::busy`, up to `MARKER_SPIN` = 20 ms). The rest of the first pass runs after
the ack: the head loop's `DISPATCH` reads, the return, and the GPU lock release. Passes are
serialized (kern_disp.c:1160, the low-latency lock), but that does not eliminate the window.
The [2026-09-27 graphics trial](../research/linux-graphics-optimization-2026-09-27.md)
observed stale echoes during `vkcube` with a zero-microsecond hybrid spin window. The setting
has not been established as the cause. Restoring page contents cannot undo an extra flip
event already delivered to the guest.

The correction under validation maps this page read-only for the guest, retaining direct
reads while writes exit to the backend. `FePage::write` consumes each trapped transaction
synchronously. An `AWAKEN_WIN` echo clears the displayed bits before the guest resumes;
`CORE_RESET` completes the marker and publishes queued window bits. No reactor scan is
needed between these writes. Partial write-one-to-clear transactions preserve unwritten
bits. The writable mapping still uses the observer described above and retains its race.
This requires both the backend change and VMM mapping support; a backend update alone
does not protect direct guest stores.

For the experimental path, apply Cloud Hypervisor patches 0001 through 0007 and
set `CARAXES_DISPLAY_PAGE_READONLY=1` alongside `CARAXES_DISPLAY_PAGE=1`,
`CARAXES_INTR_PAGE=1`, and `CARAXES_DISPLAY=1`. The launcher requires the VMM's
`x_readonly_mmap_areas` option and selects `[0:0x611000:0x1000]`. The VMM keeps
the region's write permission for vfio-user requests while protecting its KVM
mapping. This option is off by default pending the paired graphics validation.

### 10.3 Order between the clock, the page and the MSI

1. The display clock thread wakes at a tick of a head that has work: a pending presentation or
   vblank enabled, including an enable made through the page (`TickHead::vblank`). It calls the
   driver, which locks the device and runs `Display::tick` (`Engine::catch_up`).
2. Each status change goes through `FePage::update` under the page lock. The guest's unapplied
   stores are applied first, then the change, then the display values are written. A word is
   written by compare-and-swap from what was read, so a guest store that lands in between is
   kept for the next scan. Details come before summaries: `EVT_STAT` before `RM_INTR_STAT`
   before `DISPATCH`, and `AWAKEN_WIN` before `AWAKEN_OTHER` before `STAT_CTRL_DISP`.
3. Then vector 156 is raised: by `Engine::signal` when the status changed, and by the clock
   after the driver at every tick with work, as the trapped path raises at every tick (a
   `LAST_DATA` the guest leaves pending while vblank callbacks are queued is serviced again at
   the next frame). The raise (`Ticker::raise`, `IntrShared::raise`) in page mode wakes the
   reactor. The reactor folds the vector into the tree page and writes the
   MSI after it (§3 step 5). Program order and x86 store order mean the guest's handler reads
   the display page as the clock left it.
4. The guest's acks are applied by the reactor (`PageHook::poll`, display page first, then the
   tree). An ack that leaves the line asserted raises the vector again, which is the trapped
   path's level rule (`Engine::write32`). A pending ack is applied before the next clock
   change: an ack that races a tick clears that tick's bit too, as hardware merges them.
5. Connector commands (`display-ctl`) are applied by the driver as well (`BootModel::display_tick`),
   because a display read no longer reaches the server thread.

The trapped path is unchanged with the switch off: `IrqHome::Local`, a catch-up at every
display read.

### 10.4 Fail-closed rules

Start-up refusals:
- `CARAXES_DISPLAY_PAGE=1` without `CARAXES_INTR_PAGE=1` or without `--display`.
- The display engine does not model exactly the 28 watched words of the page, or any other
  model (the boot model's `owns_register`) owns a word of it (`FePage::check`, run on TU106
  in the tests).
- The page is not a free page of the memfd (`IntrPage::map_page`).

At run time, each case is counted, logged once to the trace, and restored:
- a store to `DISPATCH`, `RM_INTR_STAT` or `STAT_CTRL_DISP`;
- a store with TAG (the bit is ignored and the rest applied);
- an `AWAKEN_WIN` word that is not the echo;
- `CORE_RESET` with no marker out;
- an `AWAKEN_OTHER` store without `CORE_RESET`.

The watchdog raises vector 156 again when a marker has been out for
`CARAXES_INTR_PAGE_RETRIGGER_MS` (10 ms), at most once per period. This makes an awaken that
the guest missed late rather than lost.

### 10.5 Tests and expected effect

Tests (`cargo test -p vfio-user-nvgpu`, 200 passed; 9 of them are new for the page, and one more is the GPPut barrier of display-e.md §9):
- `display::fe_page::switch_needs_the_interrupt_page_and_the_display`
- `initial_page_and_the_area`: the rest of the page is latched memory; `map_page` refuses the
  intr pages, an unaligned page and a page outside BAR0.
- `vblank_through_the_page`: the clock's store before the raise; `LAST_DATA_RESET` through
  memory; two acks in one scan; the level rule.
- `awaken_marker`: echo invisible; `CORE_RESET` acks exactly the shown bits; queued awakens
  get the next marker with one raise; the core notifier's awaken.
- `fail_closed_cases`
- `racing_store_and_watchdog`
- `tu106_page_check`
- `display::engine::page_mode_the_clock_writes_the_page`: catch-up writes vblank and the
  window AWAKEN marker into the page; guest acks through memory clear the engine's status.
- `display::engine::the_clock_drives_the_engine_with_the_page`: a head with only a page-enabled
  vblank ticks, and the driver runs instead of the raise.

Build, clippy (both variants of `tools/check.sh`) and `cargo doc -D warnings` are clean with
`CARGO_TARGET_DIR=target/fepage`.

Expected on vm0, from 10.1:
- Mutter: 229.8 to about **132** traps per frame.
- Weston: 103.0 to about **20** traps per frame.

The exit summary's `display page:` line must show `markers` equal to `acked by CORE_RESET`
(within one at exit), and `inconsistencies: none`. Measured in §10.6.

### 10.6 Measurement on vm0 (2026-09-24, series B and C)

`caraxes/runs/display-page-2026-09-24/series.sh` ran the display gate, five GNOME starts and the
per-frame trap tables of Mutter and Weston, first with the interrupt page alone (B), then with the
display page as well (C), on the device built from main with both pages (the frozen binary
`vfio-user-nvgpu-fepage` of vm0; `series.log`, `B-intr-page/`, `C-display-page/`).

| Series | Mutter: traps per frame / callbacks | Weston: traps per frame / callbacks | GNOME starts | Host Xid |
|---|---|---|---|---|
| B: interrupt page | 230.24 / 59.72 Hz (sd 3.56 ms) | 106.46 / 59.71 Hz (sd 1.51 ms) | 5 of 5 | 0 |
| C: plus the display page | 139.86 / 59.86 Hz (sd 2.90 ms) | 31.08 / 57.41 Hz (sd 4.11 ms) | 5 of 5 | 0 |

- Mutter loses 90 traps per frame (39 percent), a little short of the expected 132: the page
  removes the front-end interrupt traffic, and what stays is the BAR2 page-table churn and the
  channel work of §6a.
- Weston loses 75 per frame (71 percent), against an expected 20: eleven traps per frame stay,
  and its frame rate sits at 57.4 Hz with a wider spread (sd 4.1 ms against 1.5 ms in B). The
  trap table of `C-display-page/traps-weston/` names them; that is the next thing to look at.
- The device's exit summary of C: 41335 display-page scans with work, 33668 EVT_STAT acks and
  28 RM_INTR_EN stores applied from the page, 7615 awaken markers and 7615 acked by
  `CORE_RESET` (longest 10.3 ms), `inconsistencies: none`; the interrupt page: 772184 scans,
  296318 acks, `inconsistencies: none` (`C-display-page/device-summary.txt`).
- GPPut barrier waits per GNOME start: 23 to 36 in C, 21 to 33 in B (display-e.md §9).

### 10.7 Read-only page hardware validation (2026-09-27)

The experimental read-only mapping was tested on the stock 615.71.09 Linux
guest with Cloud Hypervisor `e7b1965c0` and backend `419a308f` (binary SHA-256
`4e1a0c3d57855ef3605c0f14175e49b9782eee2318a154fdd9a1dc937331aa8b`).
The 0-us trace recorded 13,898 trapped FE-page writes and no FE-page reads;
the latter is expected because reads remain direct through the shared mmap and
therefore leave no vfio-user trace record. Fixed-CPU3 runs at both the default
200-us hybrid spin window and 0 us completed the same Wayland graphics
sequence with equal marker and `CORE_RESET` counts and no display-page or
interrupt-page inconsistency. At 0 us there were 3,516 markers, the longest
acknowledgement was 29.6 ms, and two watchdog raises were needed; at 200 us
there were 3,501 markers and the longest acknowledgement was 34.0 ms.

Combined backend and VMM CPU fell from 14.73% to 7.20% in the desktop-idle
window and from 185.05% to 163.45% in the graphics window. The 0-us setting
with the read-only page is therefore a measured per-VM CPU-saving option, but
it remains experimental pending a long soak and end-to-end input and frame
latency measurements. The global default remains 200 us. See
`docs/research/linux-graphics-optimization-2026-09-27.md` for the workload,
separate backend and VMM CPU figures, native-Wayland limitations, and raw
artifact paths. NVENC was not exercised in this validation.
## 11. The desktop with the recommended configuration, and the next windows (2026-09-25)

Written 2026-09-25 for the operator's direction "optimise the desktop". Runs:
`caraxes/runs/display-page-2026-09-25/` (`series.sh`, `env.sh`, `run.sh`, one folder per
series with `series-output.txt`). Environment: vm0, 610 guest, 2 GiB at reservation slot 0,
the display page series' environment (`display-page-2026-09-24/env.sh`) plus the recommended
configuration of the morning matrix: interrupt page, display page, BAR1 pool 32 MiB, token pool
64, hybrid reactor, BAR2 memfd, MSI re-arm ioeventfd, lazy plans. Each series has the host
watch around it, the trap windows of Mutter (wlframes), vkcube and glmark2 under Mutter (the
image's `firefox` is the snap stub, `snap install firefox`, so no browser window), and Weston
(wlframes); `trap-table.py --pages --regs` per window (new: every trapped register per frame
and the spread of the latch intervals), and from series F on `host-load.txt` per window (the
host's busy CPU and the other VMs' CPU). Other agents' VMs (vm2, vm3, vm5, a v1 VM, a smoke
device) shared the card and the CPUs throughout; see §11.5.

### 11.1 Series D: the memfd and the re-arm on the desktop

| Per presented frame | C (display page, 2026-09-24) | D (recommended, main d96cc59) |
|---|---|---|
| Mutter: all traps | 139.86 | **50.50** |
| of which BAR2 page-table writes | 73.60 | 0 |
| of which BAR2 flush-page reads (0xfea000) | 23.02 | 23.06 |
| of which `NV_PCFG` re-arm (0x88704) | 12.17 | 0 |
| of which the usermode page (`TIME_0/1`, 0xbb0080/84) | 16.69 | 16.04 |
| of which the MMU invalidate page (0xb830a0/a4/b0: 6 W, 4 R) | 10.03 | 10.05 |
| of which display channel kicks (0x696000 W, 0x696004 R) | 1.33 | 1.33 |
| of which USERD GPPut through BAR1 | 3.00 | 0 (the pool) |
| Mutter callbacks | 59.86 Hz, sd 2.90 ms | 60.12 Hz, sd 2.49 ms (presentation sd 0.51 ms) |
| Weston: all traps | 31.08 | **17.11** (`TIME_0/1` 15.75, kicks 1.33) |
| Weston callbacks | 57.41 Hz, sd 4.11 ms | 52.24 Hz, sd 6.36 ms |

The expectation (Mutter about 60, Weston about 20) is met for the traps. No register of the
display page 0x611000 traps any more in any window of D to I, so the display page's watched
words need no extension (§10.1). vkcube ran at 45.8 FPS and glmark2's build scene at 1608 FPS
(the compositor at 56.8 Hz). The display gate failed its compositor stage (Weston 48.95 Hz
against 60 +- 0.5); 0 host Xid.

What the memfd left: no BAR2 write traps in any window. The flush reads stay, because the flush
page stays trapped (bar2-memfd.md §3), and each of them ran a whole memfd sync: 107298 syncs
over D's run, 7.96 s, 74 us each (exit summary). They come in batches: the guest RM rewrites
the PTEs of five scanout pages through BAR2, one flush read after each PTE pair, then one MMU
invalidate (twice per frame). So the table for the next windows was: the flush reads (23), the
usermode page (16), the invalidate page (10), the display kicks (1.3).

### 11.2 The flush deferral (`CARAXES_BAR2_FLUSH_DEFER_US`)

> **Dev (merge of 2026-09-28): superseded, not in the code.** Dev defers flush reads by
> default with its own mechanism (`CARAXES_BAR2_MEMFD_DEFER_FLUSH`, bar2area.rs
> `DeferredFlush`, `BootModel::flush_point`/`settle_deferred_flush`, the 1 ms flush clock;
> display-pacing.md). It settles at the MMU invalidate, the GSP command-queue doorbell and the
> channel doorbells, not at every trapped BAR0 write or BAR1 access. `Owed`, `Payer`,
> `bar2_pay_owed` and `PageHook::poll_owed` named below are the branch's and were not merged;
> the measurements stay as the evidence for deferring.

A flush read only says "the BAR2 writes so far must become visible" (kbusFlushSingle_GV100).
Nothing reads those bytes until something consumes them. So with the deferral, a flush read
starts a debt instead of a sync (bar2area.rs `Owed`, `FakeFb::bar2_flush`, fb.rs:1021). The
debt is paid by whichever comes first:
- a sync that runs anyway: the MMU invalidate, the BAR2 bind, the GSP doorbell (they sync
  before they act, as before);
- a trapped BAR0 write other than the invalidate's three words (boot.rs:1078), a BAR1 access
  (a GPPut store may start work, boot.rs:764), or a doorbell, trapped or through the token or
  doorbell eventfd (`bar2_doorbell_sync`, boot.rs:1269);
- the interrupt page's reactor, once the bound has run out (`PageHook::poll_owed`,
  intr_page.rs:911, through `Device::bar2_pay_owed`). While a debt is open the reactor spins.

Fail closed: refused without `CARAXES_INTR_PAGE=1` (the reactor must be there to pay). The one
consumer that is not seen is the GPU reading BAR2-written VRAM through work it is already
running, with no doorbell in between (a semaphore the CPU releases through BAR2). That reader
waits at most the bound. No trace of this series has a BAR2 write followed by such a use, and
the RM's own rule (the flush precedes an invalidate, an RPC or a doorbell) is the same one the
memfd's design already relies on (bar2-memfd.md §3).

Measured: E (200 us bound) cut the syncs from 107298 to 37104, but the reactor paid 17697 of
the debts in the middle of a PTE batch. With 2 ms (F to I) the invalidate's own sync pays most
of them: F 21670 syncs, 11476 paid by the invalidate, 1113 by the reactor, 652 by an access.
The flush reads stay 23 per Mutter frame, now as bare traps (30 to 60 us apart in the trace
instead of 90 to 100). `CARAXES_BAR2_FLUSH_DEFER_US=on` is 2000 us.

Why the flush page itself stays trapped: a read of a memory page tells the device nothing, so
the debt could not start. The page could only become memory if every consumer synced
unconditionally: the doorbell eventfds too, 28000 times per second under PPO at 100 to 300 us
each. That is not viable without a cheap test for "the guest wrote BAR2 since the last sync",
which the device cannot see (the VMM's mapping, bar2-memfd.md §2).

### 11.3 The MMU invalidate page (`CARAXES_INVALIDATE_PAGE=1`)

> **Dev (merge of 2026-09-28):** the page is in dev, opt-in. Dev's held invalidate completes
> through the trigger's busy bit (invgate.rs, execution.md §8), which a memory word cannot
> show while the worker runs. So `BootModel::mmu_invalidate_from_page` waits the gate out
> (`Mirror::invalidate_busy` until false) before the page clears `TRIGGER`: with the page on,
> a held invalidate waits under the device lock as before §8. Needs a GPU run before the knob is
> trusted with the async gate.

BAR0 page 0xb83000 holds `NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE_PDB`, `_UPPER_PDB` and
`_INVALIDATE` (0xb830a0/a4/b0) beside the VF fault-buffer and access-counter pointers. The guest
RM's sequence per invalidate is a pending check (read), the two PDB words, the trigger with
`TRIGGER` (bit 31), and a poll until `TRIGGER` clears (`kgmmuCommitTlbInvalidate_TU102` and
`kgmmuCheckPendingInvalidates_TU102`, kern_gmmu_tu102.c:50-121). `TRIGGER` is the hardware's
own busy bit, so the page can serve the whole sequence from memory: inv_page.rs.
- The page is a fourth sparse area of the interrupt page's memfd (`InvPage::create`,
  `IntrPage::map_page`).
- The reactor finds `TRIGGER` in the stored word (`InvPage::scan`, inv_page.rs:250). It runs
  the device's invalidate under the device lock, as the trapped write did
  (`Device::invalidate_from_page`, device.rs:810, `BootModel::mmu_invalidate_from_page`): the
  BAR2 memfd sync (which pays the flush debt), the host TLB invalidate on the mirror VA
  spaces, the shadow resync, and then the steps after every trapped BAR0 write
  (`after_bar0_write`, device.rs:1107, where the BAR1 pool moves pages). Only then does it
  clear `TRIGGER` (`InvPage::complete`, inv_page.rs:297), which is what the guest polls for.
- Fail closed: the read-only VF words (the PUT pointers, the access-counter buffer size and
  INFO) are restored and counted, and the GET words are masked to their 20-bit field, as the
  trapped path does. A change of `_INVALIDATE` while its trigger runs is counted and left for
  the next scan. Any other register model in the page refuses the start (`InvPage::check`).
  The switch needs `CARAXES_INTR_PAGE=1` and the boot model. Trapped accesses of the page (a
  client that does not map it) go to the same memory.
- Latency: a trigger waits for the reactor's next iteration. On the desktop the reactor
  spins, because the flush debt that precedes the invalidate keeps it spinning. A trigger
  that finds the reactor blocked waits up to the sleep bound (1 ms). F: 11629 triggers, 3085
  of them found right after a blocking wait. For CUDA allocations, which invalidate without a
  flush batch before them, this is the open cost: measure it before recommending the switch
  there.

The first version skipped `after_bar0_write`. The pool then saw the guest's new BAR1 mappings
only at the next trapped write, and the gate's KMS readback read an empty pool page (capture
stage of F, G and H: `SAMPLE 320 240 0x00000000`). Fixed in f37b354, with a test that the
page's trigger leaves the registers as the trapped writes do.

Measured (F to I): the page's 10 accesses per Mutter frame are gone, with inconsistencies
none. The invalidate the device runs costs 400 to 716 us on average (the sync is in it) and
does not stall the guest's vCPU in a trap.

### 11.4 Frame timing: why Weston's frame stretches

Weston runs with a 12 ms repaint window: it starts compositing about 4.7 ms after each vblank
and must hand the flip to the display channel before the next one. Per-frame analysis of the
traces (the latch of window 6, the compositor's first doorbell after it, its last doorbell and
the display-channel kick; `analysis/wframe.py`, `isr.py` and `latchlate.py` in the run folder) gives, for Weston:

| Series | first doorbell after the latch, p50 (on time / before a missed tick) | last doorbell to kick, p50 (on time / missed) | latch lateness before a missed tick, p50 |
|---|---|---|---|
| B (trapped display) | 4.83 / 33.8 ms (4 misses) | 0.41 / 2.84 ms | 0.16 ms |
| C (display page) | 5.48 / 14.31 ms (34) | 0.80 / 5.03 ms | 0.07 ms |
| D | 6.55 / 14.39 ms (106) | 1.46 / 4.81 ms | 0.07 ms |
| E | 5.58 / 9.01 ms (17) | 2.03 / 9.02 ms | 0.06 ms |

So a missed tick is not a late display clock: the latch before it came on time. A missed
tick is a frame that Weston started late, about 14 ms after the vblank instead of 5, and whose
GPU work took longer. The guest's first interrupt service after the latch (the MSI re-arm write
that ends every handler) shows where the time goes: under Weston it ends p50 0.4 to 1.4 ms and
p90 2.2 to 6.8 ms after the latch in C, D and E, and under Mutter p90 0.4 ms. vkcube under
Mutter shows the same pattern (F: p90 5.3 ms, p99 12.5 ms).

The new counters split the delivery (display/engine.rs `Ticker::timing_summary`, fe_page.rs
`display page delivery`, the reactor's `reactor wake-up after a display raise`; G and H):
- the display clock fires on time: 1 to 2 % of ticks 1 ms or more late;
- a blocked reactor wakes for a display raise within 2 ms at p99 (1.6 to 1.9 % at 1 ms or
  more);
- but from the raise to the guest's `EVT_STAT` ack, 17 % (G) to 21 % (H) of the vblanks take
  1 ms or more, with p99 16 ms.

The wait is in the guest's response to the interrupt, not in the device. It grows with the
other VMs' load: the same binary and configuration gave Weston 52.2 Hz in D and 60.00 Hz
(sd 1.39 ms) in D2, and 52.45 and 59.94 Hz in G and H. In G, vm5's VMM used 19.8 s of CPU and
its device 5.3 s during the 17 s Weston window. In F, vm2's device hit a CTX SWITCH TIMEOUT
(Xid 109, its own pid) during Weston's window, and Weston's callbacks stalled for 4.3 s. The GPU
part (last doorbell to kick) grows in the same windows, because the GR engine is shared.

`CARAXES_INTR_PAGE_VBLANK_SPIN_US=N` keeps the reactor spinning from N us before each tick. It
removes the reactor's wake-up from the path, but G against H shows no measurable gain, so it
stays off. The gate's compositor stage needs Weston at 60 +- 0.5 Hz over 30 s, and in this
session it passed in none of D to H. With other agents' GPU work on the card, that stage
measures the card's load. The matrix of the morning passed it with the same windows.

### 11.5 What still traps per frame, and why

| Source | Mutter | Weston | Why it stays |
|---|---|---|---|
| BAR2 flush page (0xfea000) | 23 (cheap: no sync) | 0 | the deferral's signal (§11.2) |
| usermode page `TIME_0/1` (0xbb0080/84) | 16 | 16 | the doorbell (0xbb0090) shares the page and must trap for the token ioeventfds. A memory clock would need a thread writing the time continuously, and the guest reads hi-lo-hi, so a stale value is a wrong timestamp (backlog item 8) |
| display channel kicks (0x696000 W, 0x696004 R) | 1.33 | 1.33 | the display engine runs the pushed methods at the kick and answers GET := PUT |
| MMU invalidate page (0xb830xx) | 0 with the page | 0 | |
| display FE page (0x611000) | 0 | 0 | |
| PMC/PBUS | 0.01 (GSP, L2 flush) | 0 | rare |

The usermode page cannot be covered by the token pool's area either: the pool's ioeventfds
need the doorbell register to exit (tokens.rs header), and a sparse area is never trapped.

### 11.6 Series table

| Series | Configuration | Mutter traps/frame, callbacks | Weston traps/frame, callbacks | vkcube FPS | glmark2 FPS | Gate | Host Xid (this device) |
|---|---|---|---|---|---|---|---|
| C (2026-09-24) | intr + display page | 139.86, 59.86 Hz sd 2.90 | 31.08, 57.41 Hz sd 4.11 | - | - | not run | 0 |
| D | recommended (main) | 50.50, 60.12 Hz sd 2.49 | 17.11, 52.24 Hz sd 6.36 | 45.8 | 1608 | fail: compositor 48.95 Hz | 0 |
| E | D + deferral 200 us | 55.00, 48.38 Hz sd 11.16 | 15.63, 58.64 Hz sd 4.47 | 45.0 | 518 | fail: compositor 58.96, present | 0 |
| F | D + deferral 2 ms + invalidate page | 40.47, 60.06 Hz sd 1.72 | 17.53, 43.38 Hz (4.3 s stall, vm2 Xid) | 40.3 | 1964 | fail: compositor 59.33, capture | 0 (2 of vm2's device) |
| D2 | D again | 51.09, 60.10 Hz sd 3.03 | 15.38, 60.00 Hz sd 1.39 | 51.0 | 935 | not run | 0 |
| G | F + counters + spin 1.5 ms | 40.11, 60.05 Hz sd 1.88 | 17.08, 52.45 Hz sd 6.61 | 53.2 | 2715 | fail: compositor 54.09, capture | 0 |
| H | G without spin | 39.76, 59.98 Hz sd 3.36 | 15.16, 59.94 Hz sd 1.92 | 52.1 | 924 | fail: compositor 55.02, capture | 0 |
| I | H + the pool poll after a page trigger (f37b354), deferral `on` | 42.79, 52.15 Hz sd 9.36 (host 57 % busy) | 15.64, 59.93 Hz sd 2.04 | 36.5 | 610 | fail: compositor 55.77, present 23 FPS, events 3.16 ms; capture passes | 0 |
| I2 | I again (binary `vfio-user-nvgpu-desk-i2`, I's code rebuilt), guest interrupt placement recorded | 43.76, 47.83 Hz sd 4.20 (host 30 % busy) | 15.68, 59.58 Hz sd 2.20 | 44.9 | 1698 | stopped (slot 0 for the operator's game VM) | 0 |

Every host Xid line of the session names another agent's device pid (F: vm2, pid 3245134);
none named a vm0 device. E ran with vm3, vm5, a v1 VM and a smoke device up; the frame rates
of E are the load's, not the deferral's.

### 11.7 Per-frame trap tables of every series

Per presented frame of window 6 (`trap-table.py --pages --regs`, one 10 s window each; the
columns are the rows of §11.5; the numbers are the sums of the `per register` rows of each
`trap-table.txt`, `analysis/tab.py` run from `caraxes/runs/`). Rate: the client's frame callbacks (Mutter,
Weston) or the application's own FPS (vkcube, glmark2), and the latch rate of window 6.

Mutter (wlframes):

| Series | all | BAR2 W | flush R | `TIME_0/1` | `NV_PCFG` | invalidate | kicks | BAR1 | callbacks | latches |
|---|---|---|---|---|---|---|---|---|---|---|
| B | 230.24 | 73.60 | 23.02 | - | - | - | - | 3.00 | 59.72 Hz sd 3.56 | 59.6 Hz |
| C | 139.86 | 73.60 | 23.02 | 16.69 | 12.17 | 10.03 | 1.33 | 3.00 | 59.86 Hz sd 2.90 | 59.7 Hz |
| D | 50.50 | 0 | 23.06 | 16.04 | 0 | 10.05 | 1.33 | 0 | 60.12 Hz sd 2.49 | 59.9 Hz sd 0.10 |
| E | 55.00 | 0 | 23.01 | 20.61 | 0 | 10.01 | 1.34 | 0 | 48.38 Hz sd 11.16 | 44.2 Hz sd 11.86 |
| F | 40.47 | 0 | 22.98 | 16.14 | 0 | 0 | 1.33 | 0 | 60.06 Hz sd 1.72 | 60.1 Hz sd 0.03 |
| D2 | 51.09 | 0 | 23.02 | 16.68 | 0 | 10.04 | 1.33 | 0 | 60.10 Hz sd 3.03 | 60.0 Hz sd 0.24 |
| G | 40.11 | 0 | 23.02 | 15.75 | 0 | 0 | 1.33 | 0 | 60.05 Hz sd 1.88 | 60.0 Hz sd 0.03 |
| H | 39.76 | 0 | 22.98 | 15.43 | 0 | 0 | 1.33 | 0 | 59.98 Hz sd 3.36 | 60.1 Hz sd 0.14 |
| I | 42.79 | 0 | 23.04 | 18.39 | 0 | 0 | 1.33 | 0 | 52.15 Hz sd 9.36 | 49.8 Hz sd 8.76 |
| I2 | 43.76 | 0 | 23.02 | 19.38 | 0 | 0 | 1.33 | 0 | 47.83 Hz sd 4.20 | 47.7 Hz sd 7.50 |

(B and C: the trap tables of 2026-09-24 name the registers differently; C's `TIME`, `NV_PCFG`
and invalidate numbers are those of §11.1, B's are not split.)

Weston (wlframes):

| Series | all | `TIME_0/1` | kicks | other | callbacks | latches |
|---|---|---|---|---|---|---|
| B | 106.46 | | | | 59.71 Hz sd 1.51 | 59.9 Hz |
| C | 31.08 | | 1.33 | | 57.42 Hz sd 4.11 | 57.0 Hz |
| D | 17.11 | 15.75 | 1.33 | 0.03 | 52.24 Hz sd 6.36 | 52.6 Hz sd 6.04 |
| E | 15.63 | 14.30 | 1.33 | 0 | 58.64 Hz sd 4.47 | 58.3 Hz sd 3.69 |
| F | 17.53 | 16.19 | 1.33 | 0.01 | 43.38 Hz (4.3 s stall) | 40.1 Hz |
| D2 | 15.38 | 14.05 | 1.33 | 0 | 60.00 Hz sd 1.39 | 60.0 Hz sd 0.20 |
| G | 17.08 | 15.73 | 1.33 | 0.02 | 52.45 Hz sd 6.61 | 52.9 Hz sd 6.17 |
| H | 15.16 | 13.83 | 1.33 | 0 | 59.94 Hz sd 1.92 | 60.0 Hz sd 0.25 |
| I | 15.64 | 14.31 | 1.33 | 0 | 59.93 Hz sd 2.04 | 60.0 Hz sd 0.17 |
| I2 | 15.68 | 14.34 | 1.33 | 0.01 | 59.58 Hz sd 2.20 | 59.5 Hz sd 1.52 |

vkcube and glmark2 (build scene) under Mutter, the third and fourth workload (the image's
`firefox` is the snap stub, so no browser):

| Series | vkcube all | flush R | `TIME` | invalidate | FPS | glmark2 all | flush R | `TIME` | invalidate | FPS |
|---|---|---|---|---|---|---|---|---|---|---|
| D | 56.78 | 23.00 | 22.44 | 10.00 | 45.8 | 53.50 | 23.39 | 17.70 | 11.06 | 1608 |
| E | 54.84 | 23.00 | 20.51 | 10.00 | 45.0 | 53.36 | 23.25 | 18.15 | 10.61 | 518 |
| F | 46.89 | 22.95 | 22.61 | 0 | 40.3 | 42.21 | 23.97 | 16.89 | 0 | 1964 |
| D2 | 53.42 | 23.00 | 19.08 | 10.00 | 51.0 | 52.78 | 23.45 | 16.89 | 11.10 | 935 |
| G | 42.33 | 23.00 | 17.99 | 0 | 53.2 | 41.60 | 23.95 | 16.31 | 0 | 2715 |
| H | 43.34 | 23.00 | 19.01 | 0 | 52.1 | 43.42 | 23.46 | 18.46 | 0 | 924 |
| I | 52.02 | 24.20 | 25.96 | 0 | 36.5 | 42.03 | 23.34 | 17.35 | 0 | 610 |
| I2 | 46.07 | 23.66 | 20.73 | 0 | 44.9 | 42.07 | 23.71 | 17.03 | 0 | 1698 |

Every series keeps 1.33 display kicks per frame; BAR2 writes, BAR1 accesses and `NV_PCFG` are 0
from D on. Two readings of the table:
- The `TIME_0/1` reads come at a fixed rate, about 925 per second in every Mutter window (H
  15.43 x 60, I2 19.38 x 47.7), not per frame. They rise per frame whenever the frame rate
  falls, so they are not what slows a frame.
- The flush reads are exactly 23 per Mutter frame in every series: the guest RM's PTE batch of
  the scanout surfaces (§11.1), the only per-frame trap source Mutter has left besides `TIME`.

### 11.8 Series I2 and Mutter's rate with the branch's windows

I2 repeated I with a fresh build of the branch (vm0, 2026-09-25 09:53 to 10:03, host 26 to 30 %
busy, vm2 and vm3 up; vm2's VMM used 35 s of CPU in Mutter's 10 s window). Weston ran at
59.58 Hz, vkcube at 44.9 FPS and glmark2 at 1698 FPS with 59.6 Hz latches, but Mutter's own
window ran at 47.83 Hz, as I's did (52.15 Hz). F, G and H, with the same windows but without
the pool poll after a page trigger (f37b354), held Mutter at 60 Hz. The device's own waits did
not grow in I2 (`doorbell poll (tokens)` line: device lock wait per wakeup p99 22 us against H's
149 us; ring to host doorbell p50 10.4 us; invalidate mean 781 us against H's 716 us), so the
pool poll's cost is not visible in them. Per frame (`analysis/invgap.py`,
`batch.py` and `phase.py` over the Mutter traces):

| Mutter | F | G | H | I | I2 |
|---|---|---|---|---|---|
| PTE batch, first flush read to the display kick, p50 / p90 (ms) | 0.79 / 1.48 | 0.86 / 1.80 | 1.12 / 3.05 | 2.28 / 11.0 | 2.96 / 6.81 |
| an invalidate trigger to the guest's next access, p50 / p90 (ms) | 0.23 / 0.45 | 0.25 / 0.48 | 0.28 / 0.87 | 0.75 / 3.36 | 0.49 / 1.02 |
| last flush read to the reactor's trigger event, p50 / p90 (ms) | 0.19 / 0.77 | 0.19 / 0.77 | 0.28 / 0.90 | 0.50 / 2.27 | 0.22 / 0.98 |

The trigger-to-next-access time is what the guest's vCPU spins on `TRIGGER`: it doubled from H
to I2 and tripled to I, and the whole PTE batch grew from about 1 ms to about 3 ms. That is the
step f37b354 added (the pool's walk and page moves inside the trigger, before `TRIGGER`
clears). In I2 Mutter's frame also changed shape: its render doorbell now comes about 16 ms
before the PTE batch instead of 1 ms (p50 16.5 against 0.8 to 1.4 ms in F to I), so the batch
and the kick fall at the end of the frame and 120 of 476 latch intervals ran over 20 ms. Whether
that is the poll or the other VMs' GPU work cannot be split from one run: the A/B is prepared in
`series.sh` (J1 K1 J2 K2 interleaved: H's binary without the poll against this branch's with
it) and waits for the GPU (the operator's game session holds slot 0).

The watchdog lines added for it (8801bfe): `invalidate page: ... N at 1 ms or more` and an
inconsistency for a trigger held past the guest RM's 4 s timeout; `bar2 flush deferral: ...
N paid after 2x the bound or later`. I's run had a 10.3 s invalidate and a 144 ms debt, which
the old summary showed only as maxima.

### 11.9 Where the guest's interrupt lands (I2, `irq-info/guest.txt`)

- The device's MSI is irq 36, affinity 0-7, effective CPU 1 only (62893 interrupts on CPU 1, 0
  elsewhere); its thread `irq/36-nvidia` runs SCHED_FIFO 50 on CPU 1; `irqbalance` is inactive
  and the guest has no cpuidle driver (a halted vCPU waits in the host's KVM halt).
- So every vblank, every GPU notifier and every GSP event is serviced by one vCPU. When the
  host deschedules that vCPU's thread (the other VMs' VMMs used 20 to 66 s of CPU in some 10 s
  windows, §11.4), the interrupt waits for it. That is the tail §11.4 measured from the raise to
  the guest's `EVT_STAT` ack: in I2 37 % of the vblanks at 1 ms or more, p99 16 ms (I 41 %, G 17 %,
  H 21 %).
- Weston's 57.4 Hz and 4.1 ms spread in C are this tail: Weston starts its repaint a fixed time
  after the vblank it is told of, so a late vblank delivery starts the frame late (14 ms instead
  of 5, §11.4), and the frame misses the next tick. No trap stretches it: Weston traps 15 to 17
  times per frame from D on, 14 to 16 of them `TIME` reads at about 20 us each.
- What would shorten it lies outside the device: pinning vm0's vCPU 1 on the host, or spreading
  the guest's MSI (the device offers one vector). It is not a device change, so it is left for
  the operator (and for the game VM, which has the same single vector).


## 12. The remaining per-frame traps as memory (2026-09-25 afternoon)

Written for the operator's direction "get rid of as many traps as possible, and where they are
necessary, optimise them out with the reactor". Starting point: series H (§11.7), 40 traps per
Mutter frame: 23 BAR2 flush-page reads, 16 `TIME_0/1` reads on the usermode page, 1.33 display
kicks. Runs: `caraxes/runs/display-page-2026-09-25/` series P1, T1, U1, U2, W1, W2, J1, K1
(`series.sh` header), vm0 at reservation slot 2 beside the game VM (vm4, slot 0 of its 3 GiB
profile), `CARAXES_RESERVE_SLOT_DESK=2`.

### 12.1 The BAR2 flush page as memory (`CARAXES_BAR2_FLUSH_PAGE=memory`)

> **Dev (merge of 2026-09-28):** ported onto dev's deferred flush: with `memory` the owed
> flush stands from every sync on (`DeferredFlush::standing`), dev's hand-overs (MMU
> invalidate, GSP and channel doorbells) settle it, and the flush clock syncs 1 ms after the
> last sync (the branch's reactor bound was 2 ms, and it also paid at every trapped BAR0 write
> and BAR1 access; those payers are not ported). Refused with
> `CARAXES_BAR2_MEMFD_DEFER_FLUSH=0` or a trap list holding the flush page. Needs a GPU run
> before the knob is trusted.

- The flush is `kbusFlushSingle_GV100`'s read of a BAR2 CPU pointer (kern_bus_gv100.c:384-395),
  a plain read. A memory page cannot report a read, so without the trap no read can start a debt.
- With the switch no BAR2 page traps (bar2area.rs `config_from`: the trap list defaults to none,
  refused if it still holds 0xfea000), and the debt stands from every sync on (`Owed` standing
  mode, bar2area.rs): `since` is the last sync.
- Who syncs:
  - the MMU invalidate (the reactor's trigger, inv_page.rs), the BAR2 bind and the GSP doorbell,
    as before;
  - every trapped BAR0 write (the display kick), BAR1 access and doorbell (fb.rs `bar2_pay`:
    in standing mode every payer syncs);
  - the reactor, when the bound (2 ms) has run out since the last sync (intr_page.rs
    `poll_owed`; it does not spin for a standing debt, the 1 ms sleep bound wakes it).
- Why no GPU use of a page-table update can come before its sync (the D17 ordering): the guest
  RM writes page tables only through BAR2 (its BAR2 CPU mapping, bar2-memfd.md §1), and a PTE
  becomes usable by the GPU only after an MMU invalidate (`kgmmuCommitTlbInvalidate_TU102`,
  kern_gmmu_tu102.c:95-123, at the end of every `dmaUpdateVASpace` path and of
  `kbusUpdateRmAperture_GM107`, kern_bus_gm107.c:2757-2785). The invalidate is a sync point,
  and the device syncs before it walks. Other BAR2 data (instance blocks, RAMFC) reaches the
  GPU through a GSP RPC, which syncs, or a BAR2 bind, which syncs. Userspace has no BAR2
  mapping. Test: fb.rs `bar2_flush_page_as_memory_syncs_at_the_consumers` (a PTE write, no
  flush read, reaches VRAM at the invalidate's sync; a doorbell and the reactor pay too).
- Fail closed: refused with the bound off or a trap list with the flush page; needs the
  interrupt page. Counters (exit summary `bar2 flush page as memory:`): syncs per payer, the
  reactor syncs that wrote bytes back (writes no consumer had paid within the bound), the
  longest time between syncs and the number past twice the bound.
- Measured (P1, 550 s of device life): 372327 syncs, 19.2 s in total (51 us each): 216910 by
  the reactor at the bound, of which 262 wrote bytes back; 136716 by a trapped write or a
  doorbell; 18701 that ran anyway. The ring to the host doorbell went from p50 10.4 us (I2) to
  56.4 us, because each doorbell now syncs. That cost is why the switch is a display mode: the
  CUDA path keeps the trapped flush page (a PPO run rings about 28000 times per second, §11.2).

| Per presented frame | H | P1 |
|---|---|---|
| Mutter: all traps | 39.76 | **17.68** |
| of which flush reads | 22.98 | 0 |
| of which `TIME_0/1` | 15.43 | 16.34 |
| of which display kicks | 1.33 | 1.33 |
| Mutter callbacks | 59.98 Hz sd 3.36 ms | 60.06 Hz sd 2.68 ms (presentation sd 0.74, latches sd 0.25) |
| Weston: all traps | 15.16 | 14.70 |
| Weston callbacks | 59.94 Hz sd 1.92 | 60.00 Hz sd 0.70 |
| vkcube FPS, traps | 52.1, 43.34 | 53.2, 20.15 |
| glmark2 FPS, traps | 924, 43.42 | 2045, 18.86 |
| display gate | fail (compositor) | **pass** |

P1 ran with the pool poll after a page trigger (f37b354) and with only the game VM beside it;
Mutter held 60 Hz, unlike I and I2 (§11.8), which points at the other VMs' load there rather
than at the poll (the J/K A/B, §12.6).

### 12.2 The usermode page as memory (`CARAXES_USERMODE_PAGE=1`)

- The page 0xbb0000 holds `TIME_0/1` (0x80/0x84) and the doorbell `NOTIFY_CHANNEL_PENDING`
  (0x90). It cannot be split, so the whole page becomes a sparse area the reactor owns
  (um_page.rs).
- Who reads `TIME`: the guest RM's `tmrGetCurrentTime` on a VF (timer_tu102.c:107-159,
  hi-lo-hi), UVM through `time0Offset/time1Offset` (nv_gpu_ops.c:2541-2553), and the GL/Vulkan
  userspace through its VOLTA_USERMODE_A mapping (clc361.h:31-32: GPU timestamps, fence and
  wait timeouts). nvidia-drm and nvkms time with the CPU clock (`nvkms_get_usec`, nvidia-modeset-linux.c:746; the fence
  timeouts in nvidia-drm-fence.c:43, :124-138 are jiffies), and Mutter's frame clock uses the
  DRM vblank timestamps (CLOCK_MONOTONIC), not the GPU's. So no reader needs more than
  millisecond freshness, and none may see time go back.
- The clock: the reactor stores both words as one aligned 64-bit store every iteration
  (`UmPage::publish_time`, the trapped model's format, uvm.rs:73-74). A reader never sees a
  torn pair (two 32-bit stores would show (H, small) or (H+1, max) at a `TIME_0` wrap, every
  4.3 s). While the reactor blocks (at most its 1 ms sleep bound) the clock stands still:
  stale by up to 1 ms, never backwards.
- The doorbell: a write lands in memory. The reactor swaps the word to a sentinel
  (0xffffffff, no token has all bits) and then scans every channel's GPPut
  (`doorbell_pending`, ctrl.rs:353), ringing each that advanced. Several writes before the
  reactor looks leave only the last value, and the same value twice is only a change because
  the swap put the sentinel back; the value never decides what runs. Ordering: the guest
  writes GPPut before the doorbell, the scan runs after the swap, so a channel is rung only
  with a GPPut that is visible. A watchdog scan every 10 ms counts channels it finds pending
  without a doorbell seen. The token pool's ioeventfds are not offered with the page (they
  could never fire on a mapped page).
- Fail closed: TIME_0/1 must be one aligned qword in the doorbell's page, no other register
  model may answer in the page (`UmPage::check`), and the interrupt page and the boot model
  are required; a guest store to TIME is an inconsistency.
- Tests: um_page.rs (layout and check, the clock never going back, doorbell coalescing and the
  watchdog), channel.rs `one_page_doorbell_rings_every_advanced_channel`.

### 12.3 The display kick page (`CARAXES_DISPLAY_KICK_PAGE=1`)

- The window channels' USERD pages (NVC57E and NVC57B, 64 pages from 0x690000,
  display/mod.rs `userd_base`) hold `PUT` (+0) and `GET` (+4). nvkms writes the methods into
  its pushbuffer in guest RAM, then `PUT`; it reads `GET` for free space. Trapped, the `PUT`
  write runs the kick (display/mod.rs `kick`): 1.11 writes and 0.22 `GET` reads per frame.
- With the switch the 64 pages are one sparse area (display/chan_page.rs). The reactor
  compares each `PUT` with the last one it saw and runs the kick under the device lock
  (`Device::display_kick_from_page`: the BAR2 payment and the steps after a BAR0 write, as the
  trapped write had), then stores `GET`. Two `PUT` writes before the reactor looks are one
  kick to the newest, as the walk from `GET` to `PUT` is for the trapped second write.
- Fail closed: needs the display page; a guest store to `GET` is restored and counted, a `PUT`
  in a page without a channel is counted and left, and the watchdog counts a kick not
  completed within 20 ms. Tests: chan_page.rs, display/tests.rs `a_page_kick_is_the_trapped_kick`.
- Latency: a kick waits for the reactor's next iteration, up to its 1 ms sleep bound when it
  blocks. The latch is at the next tick; a flip that came less than a millisecond before it
  can slip one frame.

### 12.4 The host BAR1 floor

> **Dev (merge of 2026-09-28):** dev already had the same floor (bar1pool.rs
> `host_bar1_floor`, `CARAXES_HOST_BAR1_FLOOR_MIB`, 96 MiB); dev's code was kept.

The game VM's 128 MiB pool plus vm0's pool exhausted the host's 256 MiB BAR1, and the host
desktop's Vulkan failed at `vkCreateDevice` (INITIALIZATION_FAILED), which ended Moonlight.
bar1pool.rs `check_host_bar1` now reads the host's free BAR1 at start (`nvidia-smi -q -d
MEMORY`, read-only) and refuses a pool whose size plus window budget would leave less than
`CARAXES_HOST_BAR1_FLOOR_MIB` (default 96 MiB); an unreadable free size refuses too, 0 skips
the check. With the game VM up the host showed 91 MiB free (10:59), so vm0's 32 MiB pool with
its 64 MiB window budget is refused beside it from now on.

### 12.5 The game session's two findings

- **The hardware cursor.** Mutter puts the pointer on nvidia-drm's cursor plane (window 7 in
  the device, 143 updates in P1). Sunshine's KMS capture exports the framebuffer of one
  plane, the primary (its "Couldn't get drm fb for plane [0]" at 08:38:13,
  `session-20260925-102541/sunshine.log:223169`, is that plane's framebuffer going away at the
  session restart); a pointer on its own plane is in no buffer the capture reads. The stage
  forces `MUTTER_DEBUG_DISABLE_HW_CURSORS=1`. A device-side
  alternative, compositing the cursor plane into the scanout buffer the capture reads, is not
  worth it: that buffer is the compositor's own render target in guest VRAM, which it reuses
  with damage tracking, so writing cursor pixels into it leaves trails in the next frames and
  races the GPU's rendering; a separate composited copy would need a device-side blit per frame
  and a buffer the capture does not know. Mutter's software cursor costs one small blend in a
  frame it renders anyway. Keep the variable.
- **The second GNOME start.** No device-side cause in the records: both failing lives show 0
  refusals, all channels freed, the token pool never exhausted and 0 host Xid
  (`caraxes/runs/sottr-2026-09-24/session-20260925-102541/device-log-tail.txt:168-174`). The
  "not supported by EGL" message is from a first start (life 10:18,
  `session-20260925-101825/guest-journal.txt:5,8`); the second-start failure (life 10:25) left
  only gdm's "maximum number of X display failures" (`session.log:198`). The difference from
  this series' five restarts per life: the series stops gdm3, waits and starts it
  (`display-page-2026-09-24/guest/gnome-restart.sh`), so gdm autologins again; the game stage
  restarted the session by hand with Sunshine (KMS capture, holding card0 and the NVIDIA nodes)
  and Steam still running, and gdm does not autologin twice in one daemon life
  (`sottr-session.sh gnome` only starts gdm3 when no gnome-shell runs). Fix in the stage, not
  the device: a scripted restart that stops Sunshine and Steam, stops gdm3, waits until
  `fuser /dev/dri/card0 /dev/nvidia*` is empty and starts gdm3; and keep the whole `gpu.log`
  and a `sudo journalctl -b` with the gdm lines per life (the failing lives' device logs were
  overwritten).

### 12.6 Status and the runs still owed

- Measured: P1 (§12.1). Built, unit-tested and prepared in `series.sh`, not yet run on the GPU
  (the operator held vm0 down for the game session at 11:0x): T1 (the token-pool reference
  with the final binary), U1 and U2 (the usermode page with the hybrid and the spinning
  reactor, each with the CUDA launch rows `CUDA=1`: nvprobe 4 short and long kernels), W1
  (every window: flush page, usermode page, kick page) and W2 (the kick page with the token
  pool), and the J/K A/B of the pool poll after a page trigger.
- Expected per Mutter frame with W1: 0 flush reads, 0 `TIME` reads, 0 kicks; what traps is
  then the rare PMC/GSP/L2 rows (0.01 per frame).
- The usermode page's decision needs the U/T numbers: the reactor's CPU (`doorbell poll`
  summary line, `reader CPU`), the launch time per kernel from nvprobe 4, and the ring latency.
  Expected from the design: with the hybrid reactor a doorbell that finds the reactor blocked
  waits up to its 1 ms sleep bound, which a short CUDA launch (about 0.1 ms, benchmark.md)
  cannot afford; the token pool's eventfd wakes it at once. So unless U2's spinning reactor is
  the configuration, the usermode page stays a display mode.
- vm0 beside the game VM is now refused by the host BAR1 floor (§12.4) unless the pool and
  window budget shrink or the floor is lowered for the run.

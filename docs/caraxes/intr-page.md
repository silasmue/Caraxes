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

The exit summary adds `intr page: ...` with the scans that found work, the guest stores applied
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

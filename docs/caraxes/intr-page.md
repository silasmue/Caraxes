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
| `LEAF_EN_SET(i)` | the enable mask (exact: the guest reads it) | the word differs from the mask | set bits |
| `LEAF_EN_CLEAR(i)` | 0 | a word other than 0 | clear bits |
| `TOP(0)` | pending subtrees (leaf & enable), no TAG | a change (a store to a read-only register) | restored, noted |
| `TOP_EN_SET(0)` | top enable (subtree bits 0-3) \| bit 31 | the word differs | set bits 0-3 |
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

**The scan.** Every reactor iteration compares the 28 watched words with the display values it
wrote last (one load each, no lock; about 20 ns). On a difference, or when another thread raised
a vector, it takes the page lock and:

1. snapshots the changed words;
2. applies them in the order of a lock cycle: `LEAF_EN_CLEAR` and `TOP_EN_CLEAR` first, then
   `LEAF_EN_SET` and `TOP_EN_SET`, then the acks, then triggers, then the vectors raised by other
   threads (`IntrShared`, the host event thread and the display clock);
3. decides the MSI as the trapped path does, on an edge of `TOP & TOP_EN` (`IntrTree::edge`),
   evaluated after each of those phases, so a clear and a set seen in one scan still make the
   edge they made when trapped; and, as the trapped path's `IntrShared::raise` does, on every
   vector raised from another thread that was not already pending;
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
  next step (phase 2), not this change.
- **The usermode page (0xbb0000)**: the doorbell must trap into its ioeventfd sub-regions, and
  `TIME_0/1` (14.7 reads per frame) are answered from the clock at the moment of the read.
- **`NV_PCFG` (0x88000)**: writes land in config space (`bar0.rs` `WriteEffect::Mirror`).
- **The VF MMU invalidate and fault-buffer page (0xb83000)** and every other register with a
  model behind it: they act synchronously.
- **BAR2**: Mutter's 71 PTE writes and 23 reads per frame are page-table traffic of the guest RM.

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
Weston f4 157.0 - 81.4 = **75.6**. At 23-27 us per trap (mapping.md, c0/c1) that is 10-12 ms of
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

## 7. Switches

| Variable | Default | Meaning |
|---|---|---|
| `CARAXES_INTR_PAGE` | off | `1`: offer page 0 and the VF interrupt tree page as memory (needs `CARAXES_DOORBELL_POLL=spin\|hybrid`) |
| `CARAXES_INTR_PAGE_SLEEP_MS` | 1 | hybrid: the longest blocking wait of the reactor while the page is on |
| `CARAXES_INTR_PAGE_RETRIGGER_MS` | 10 | the watchdog period; 0 turns the watchdog off |

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

**Tests** (`cargo test -p vfio-user-nvgpu`, 172 passed, 11 of them new; the same in release):

| Test | What it shows |
|---|---|
| `intr_page::switch_and_settings` | the three variables, refusals of settings without the switch and of bad numbers |
| `intr_page::initial_displays_and_page_zero` | the initial page: TAG in the leaves and `TOP_EN_SET`, 0 in the clears, the trigger sentinel, the boot constants; page 0's lock writes need no work |
| `intr_page::raise_writes_the_page_then_the_msi_and_the_ack_clears_it` | a raise from another thread is coalesced before the fold, the page shows the bit before the MSI eventfd is written, an enable makes the trapped path's edge, the ack `NVBIT(4)` through the page clears it, the next raise signals again |
| `intr_page::page_matches_the_trapped_tree` | the same write sequence (triggers, enables, a lock cycle, acks) through `IntrTree` and through the page gives the same leaves, enables, `TOP` and MSI count; a whole lock cycle between two scans still makes its edge |
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

**Measurement**: pending the coordinator's vm0 window (§9 when taken).

<!-- SPDX-License-Identifier: MIT -->
# The allocation path: where cuMemAlloc and cuMemFree spend their time, and lazy shadow plans

Written 2026-09-24 after the closing series (benchmark.md, "Closing series on driver 610.57.04":
cuMemAlloc 1 MiB 830 us against native 36 and v1 93, cuMemFree 1 MiB 903 us, cuMemAlloc 64 MiB
6.7 ms). Analysis first, then the cheapest safe step, built behind `CARAXES_ALLOC_LAZY=1`. The
analysis was written before any VM run; the plan is §6, the measurement §7.

Sources:

- device traces with access tracing on: `caraxes/runs/mapping-2026-09-24/p3-profile/init-trace*.txt`
  (one `bench-cu init` each, committed), and the newer `init-trace.txt` of the BAR2 memfd series,
  off mode (main f343f87, vm2, 2 GiB profile, token pool and spin reactor on; in the main
  checkout under `caraxes/runs/bar2-memfd-2026-09-24/off/`, not committed when this was written).
  No trace of the churn metric exists: every churn run so far had only its `CHURN` lines.
- NVIDIA open kernel modules 610.57.04, `vendor/open-gpu-kernel-modules/src/nvidia/src/kernel/gpu/`.
- The device, `crates/vfio-user-nvgpu/src` (`boot.rs`, `ctrl.rs`, `mirror.rs`, `shadow.rs`,
  `hostmem.rs`), and the new `lazy.rs`.
- The new tool `caraxes/tools/rpc-cost.py`, which attributes a trace's time to what the guest
  trapped on, per GSP RPC, per invalidate episode and per phase of a churn run.

## Summary

- **No allocation reaches the device as an RPC.** A context creation sends 71 `GSP_RM_ALLOC`
  (channels, TSGs, engine objects, VA spaces, OS events), 46 `GSP_RM_CONTROL` and 71 `FREE`, and
  not one memory class. The device answers each in 2-14 us of its own time (§1.2). The guest's
  CPU RM allocates video memory from its own PMA and writes the GPU page tables itself, through
  BAR2.
- **The 830 us are traps, and most of them are the guest RM's lock.** One guest mapping change
  costs 24 trapped accesses at 27-28 us each (§1.1): 18 interrupt-enable writes that the RM makes
  at every lock acquire and release, 5 for the MMU invalidate and 1 flush read. PTE writes through
  BAR2 add 4 traps for a 2 MiB page and 128 for 64 MiB.
- **The device does its host work off the critical path.** The walk on the server thread takes
  13 us at the median. The host RM calls (region objects, `RM_MAP_MEMORY_DMA`, NVOS47) run in the
  shadow worker: 244 us median per plan, 2.1 ms at p90. A guest vCPU waits for them only at the
  next doorbell of a shadow channel (mapping.md §4).
- **Two switches that already exist remove 19 to 147 of the traps per operation.**
  `CARAXES_INTR_PAGE=1` (intr-page.md) and `CARAXES_BAR2_MEMFD=1` (bar2-memfd.md) together leave
  6 per 1 MiB operation, about 170-200 us. Neither has been measured on the churn metric.
- **Implemented: lazy shadow plans** (`CARAXES_ALLOC_LAZY=1`, `lazy.rs`). A guest invalidate
  records its root. The next shadow doorbell, a channel creation or a 20 ms deadline plans every
  pending root at once. An allocation freed before the next doorbell never reaches the host RM.
  This takes the walk off the invalidate trap and most of the host RM calls off churn. It is the
  prerequisite of step 3, which is the step that takes the remaining invalidate traps away (§3).

## Design in five sentences

1. The guest's allocation path is guest bookkeeping: the guest RM keeps its PMA and its page
   tables, the device never answers an allocation, and the physical GPU only ever sees the shadow,
   which `resolve_leaf` validates leaf by leaf when it is planned (D18).
2. So the shadow only has to match the guest's tables when a host channel may run. That is at a
   doorbell or a channel creation, which already plan and wait for the worker. The per-invalidate
   plan can be deferred and batched (step 2, built). Pairs that cancel then never reach the host
   RM. What must stay synchronous is that doorbell barrier, the filter, the budget and the
   teardown order.
3. Most of what the guest waits for is traps, not host calls. The next steps therefore take trapped
   registers out of the path: the interrupt-enable toggles and the BAR2 PTE writes with the
   existing switches (step 1), then the invalidate and flush registers, served from memory
   (step 3), which is possible only once nothing needs the device synchronously at the invalidate.
4. What can be cached on the host is the virtual region object of a 2 MiB region, kept on a
   per-VAS free list when the region empties (step 4). There are no per-allocation host memory
   objects to reuse: every vidmem leaf maps from the one reservation handle, and every sysmem leaf
   from the registered guest-RAM descriptors.
5. Expected per 1 MiB allocation: 830 us today, about 200-250 us with step 1, a few percent less
   with step 2 (plus no host RM calls for churn), and about 40-80 us with step 3, which is between
   native (36) and v1 (93). Each step leaves the isolation argument of the shadow unchanged: every
   mapped leaf lies in this VM's reservation or its registered guest RAM.

## 1. What the device does today

### 1.1 One guest mapping change, in order

The unit is an *episode*: everything from one MMU invalidate of a channel VA space to the next.
The BAR invalidates are excluded; they set `HUBTLB_ONLY` (flags 0x5), the channel ones do not
(0xc1). One episode is one guest RM mapping change (`rpc-cost.py`, "Episodes").

The most frequent episode in all three committed init traces is 24 trapped accesses: 640 us at the
median in the newer trace (20 episodes, minimum 597 us) and 899 us in `init-trace-d3.txt`.
Example: `p3-profile/init-trace.txt:97407-97432`. With a 2 MiB PTE it adds four BAR2 writes and a
second flush read (`init-trace.txt:83280-83299`: the PD0 entry at FB 0x169008210, both qwords
written twice, then the flush reads and the invalidate, interleaved with an interrupt handler
on another vCPU).

| # | What the guest does | Trapped accesses | Cost | Device work on the trap | Host RM call |
|---|---|---|---|---|---|
| 1 | `ioctl` into the guest RM (vidmem alloc, `MAP_MEMORY_DMA`, `UNMAP`, `FREE`), RM API and GPU lock acquire | `NV_PMC_INTR_EN_CLEAR/SET` 0x180/0x160, written 0 at every acquire and release (`_intrSetIntrEnInHw_GP100`, intr_gp100.c:35-67); `LEAF_EN_CLEAR(2,4)`, `TOP_EN_CLEAR` (`_intrDisableStall_TU102`, intr_tu102.c:397, intr-page.md §1) | about 9 | latched (bar0.rs) | none |
| 2 | PMA allocation or free | none (guest CPU only) | - | - | none |
| 3 | PTE / PDE writes through BAR2 (the page-table page's own BAR2 mapping) | 0 (the episode above), 4 (one 2 MiB PTE, both qwords twice), 128 (64 MiB), 1024 (a 4 KiB page table) | 27 us each | FB store write, walk-cache dirty mark (mapping.md §1), BAR TLB flush | none |
| 4 | `kbusFlush`, the flush read of BAR2 0xfea000 | 1-2 | 30 us | answered from the FB store (bar2-memfd.md §1) | none |
| 5 | TLB invalidate: `kgmmuCheckPendingInvalidates_TU102` reads `MMU_INVALIDATE` (kern_gmmu_tu102.c:50-84), `kgmmuSetPdbToInvalidate_TU102` writes `_PDB` and `_UPPER_PDB` (:134-160), `kgmmuCommitTlbInvalidate_TU102` writes the trigger and polls it (:95-123) | 5 | 28 us each | at the trigger (boot.rs:653-690): BAR2 memfd sync (memfd mode only), `fb.flush`, `mirror_invalidate` (none in this configuration, "no mirror VA space rooted there"), then the shadow plan: cached walk plus filter, 13 us median, and a job for the worker (mirror.rs:376-402) | none on this thread |
| 6 | lock release | `PMC_INTR_EN` pairs, `LEAF_EN_SET(2,4)`, `TOP_EN_SET` (`_intrEnableStall_TU102`, intr_tu102.c:434-517) | about 9 | latched / interrupt tree | none |
| 7 | (worker, in the background) | - | apply 244 us median, 2.1 ms p90 | `alloc_virtual_fixed` (region object, hostmem.rs:849) at a region's first leaf, `map_dma_fixed` per coalesced run (hostmem.rs:923), `unmap_dma` (hostmem.rs:898) and the region object freed when a region empties (shadow.rs:728) | `NV50_MEMORY_VIRTUAL`, `RM_MAP_MEMORY_DMA`, NVOS47, `FREE` |
| 8 | the next doorbell of a shadow channel | 1 (or an eventfd) | the worker's remaining queue | `gr_shadow_doorbell` plans the root and waits for the worker (`wait_all`, mirror.rs:530-553) | whatever is still queued |

Over the whole context creation (`rpc-cost.py` on the newer init trace; 112928 trapped accesses,
3.61 s attributed):

| class | accesses | time | share | median per access |
|---|---|---|---|---|
| BAR2 writes (page tables, instance memory, scrub) | 96269 | 2975 ms | 82.3 % | 27 us |
| VF interrupt tree (lock toggles and interrupt servicing) | 7814 | 283 ms | 7.8 % | 27 us |
| `NV_PMC_INTR_EN_SET/CLEAR` (RM lock) | 5416 | 226 ms | 6.3 % | 28 us |
| GSP queue doorbell | 188 | 37 ms | 1.0 % | 56 us |
| MMU invalidate | 825 | 26 ms | 0.7 % | 28 us |
| GSP falcon registers | 564 | 21 ms | 0.6 % | 31 us |
| BAR2 flush read | 478 | 16 ms | 0.4 % | 30 us |
| rest (PCI config, interrupt re-arm, usermode page) | 1350 | 30 ms | 0.8 % | - |

The worker applied 36 plans in that window: 19619 leaves mapped in 590 RM map calls and 17538
unmapped, 57 ms of worker time in total.

### 1.2 The GSP RPCs

These are the RPCs the guest actually sends. The same trace has none on the allocation path:

| RPC | count per context creation and destruction | device time, median (rx to tx) | with a host RM call |
|---|---|---|---|
| `FREE` | 71 | 3 us | 24 (the mirror's channel and TSG teardown) |
| `GSP_RM_ALLOC` 0xc46f channel / 0xc5b5 CE / 0xc5c0 compute | 20 / 20 / 8 | 3-4 us | 0 (the host objects are built at the first doorbell) |
| `GSP_RM_ALLOC` 0x79 OS event, 0xa06c TSG, 0x90f1 VAS, 0x80, 0x2080, 0x2081, 0x9067, 0x83de | 8, 4, 2, 2, 2, 1, 1, 1 | 3-9 us | 0 |
| `GSP_RM_CONTROL` (24 commands) | 46 | 2-14 us | 3 (0xa06c0105 preempt, 0xa06c0103) |
| any memory class (0x40, 0x3e, 0x50a0, 0x71) | **0** | - | - |

So "the RM round trip per allocation through the emulated GSP" (benchmark.md, closing-series
reading) is not what costs: there is no round trip. The cost is the guest RM's trapped register
traffic around its own allocation.

### 1.3 Per operation

Measured totals are from the closing series. The composition is the episode of §1.1 plus the PTE
writes the page size implies. Libcuda's 1 MiB allocations use 2 MiB pages, as the PD0-entry writes
in the traces show; with two 1 MiB blocks per 2 MiB page, only every other allocation writes a PTE.
A churn trace (§6) confirms or corrects the split.

| Operation | Measured | Episode (lock 18, invalidate 5, flush 1-2) | BAR2 PTE writes | Unexplained |
|---|---|---|---|---|
| cuMemAlloc 1 MiB | 830 us | 24-25 traps, 650-700 us | 0-4, on average 2 (55 us) | 75-125 us: the guest RM's own CPU time and interrupt servicing (host notifier events of other clients' GR work, mapping.md §3) |
| cuMemFree 1 MiB | 903 us | 24-25 traps, 650-700 us | 0-4 (PD0 entry cleared) | 150-200 us, same kind |
| cuMemAlloc 64 MiB | 6.3-7.0 ms | 650 us | 128 (32 PD0 entries x 2 qwords x 2), 3.5 ms | about 2.5 ms: likely a second episode (more lock cycles or a fresh PD0 page); the churn trace decides |
| cuMemFree 64 MiB | 4.3-4.8 ms | 650 us | 128, 3.5 ms | about 0.2-0.6 ms |

On the device side, every operation adds one walk on the server thread (13 us median) and one
plan in the worker. In the worker, a 2 MiB region's first allocation costs a region object and a
map, and its last free an unmap and the object's free: four host RM calls per churned region. The
churn run's device CPU (17.6 s over 11 s of wall time in the BAR2 memfd series, off mode) is the
spin reactor's core plus that worker and walk time.

## 2. The next steps, cost and isolation

"Cost" is the expected time of one cuMemAlloc of 1 MiB as the guest sees it; 64 MiB in brackets.

| Step | What | Expected | Isolation argument |
|---|---|---|---|
| 0, today | eager plans at every invalidate, trapped BAR0 page 0, VF interrupt tree, BAR2 | 830 us (6.7 ms) | the shadow filter (`resolve_leaf`, shadow.rs), the budget, the doorbell barrier |
| 1 | `CARAXES_INTR_PAGE=1` + `CARAXES_BAR2_MEMFD=1`, both built and merged; only the display (intr page) and `bench-cu init` (memfd, running) have been measured | 6-7 traps: about 170-200 us plus the guest's own time, so about 200-250 us (0.3-2.7 ms, depending on what the 64 MiB remainder is) | unchanged: both pages are VM-private memory, the GPU still reads only the shadow (intr-page.md, bar2-memfd.md §3) |
| 2, built | `CARAXES_ALLOC_LAZY=1`: the invalidate records the root. Plans happen at the next shadow doorbell or channel creation, or at an invalidate past a 20 ms deadline | minus the walk on the invalidate trap (13 us median, milliseconds at p99 for large tables): about 190-240 us. Host RM calls for allocations freed before the next doorbell: 0 (4 per churned region today). The first doorbell after N allocations pays one walk and one apply of the net change | a deferred plan is filtered exactly like an eager one. A mapping the guest dropped stays on the host until the next doorbell or deadline, not just until the worker's next job. It was validated when made, so it still points into this VM's reservation or its registered guest RAM. The GPU can use it only for work the guest submitted before the free, which the guest must not do (the same argument as the worker's deferred unmaps, mapping.md §4) |
| 3, design | the MMU invalidate registers and the flush page as memory. The invalidate's page 0xb83000 (`_PDB` 0x30a0, `_UPPER_PDB` 0x30a4, trigger 0x30b0) becomes a sparse area, scanned by the reactor as the intr page is. The reactor applies a trigger by recording the root for the lazy plan and clearing bit 31, which the RM polls (kern_gmmu_tu102.c:69-72). The BAR2 flush page becomes memory too, and the BAR2 memfd sync moves to the doorbells (`CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1`, which exists) and the GSP doorbell (already a sync point) | 0 traps on the allocation path: the guest RM's own time, about 40-80 us (native 36, v1 93). 64 MiB about the same | the same as step 2: nothing the GPU reads changes without a doorbell or GSP doorbell, both of which stay trapped and sync. The fault-buffer registers on the same page (0x3070-0x3094) must stay trapped or be modelled from memory. That is the open design question of this step |
| 4, design | a per-VAS free list of empty region objects (`NV50_MEMORY_VIRTUAL` of 2 MiB at a fixed VA, keyed by VA and page class), reused instead of freed and allocated again, bounded, and drained by the teardown | no change to the allocation. The first doorbell after a region is reused saves two host RM calls | a virtual object has no backing and lives in the device's own shadow VAS; every leaf mapped into it still passes `resolve_leaf`, and the budget counts mappings, not objects |

What must stay synchronous in every step:

- the doorbell barrier (`wait_all` before the host doorbell, mirror.rs:547);
- the plan and wait before a channel's creation, because the host validates the GPFIFO VA
  against the shadow VAS (mirror.rs:670);
- the filter on every leaf;
- the mapping budget;
- the teardown order: a shadow VAS is freed by the worker before the guest-RAM descriptors,
  `wait_idle`, parallel.md §4a.

The BAR1 pool (P21 design H) plans its own roots and waits (`bar1pool.rs` `resync`). It does not
go through the invalidate path and is unaffected by lazy mode.

What does not help: caching host memory objects per allocation size. There are none. Every
vidmem leaf maps from the reservation handle, and every sysmem leaf from a registered descriptor.

## 3. What was implemented: `CARAXES_ALLOC_LAZY=1`

**Code.**

- `lazy.rs` (new): the policy `LazyPlans`.
  - `on_invalidate` (lazy.rs:149) returns `Plan(root)` with the switch off, `Defer` with it on,
    or `Flush(batch)` once the oldest deferred invalidate is older than the deadline.
  - `take` (lazy.rs:169) hands the pending batch to a doorbell.
  - `ALL_PDB` makes the batch cover every root.
- `mirror.rs`:
  - a field `lazy`, read from the environment in `Mirror::new` (mirror.rs:289), with a start line
    `alloc lazy: on, ... deadline 20 ms` in `gpu.log`. A malformed value logs `ERROR alloc lazy`
    and stays eager.
  - `shadow_invalidated` (mirror.rs:408), which the invalidate trap now calls through
    `Ctrl::mirror_shadow_resync` (ctrl.rs:656). With the switch off it is the old `shadow_resync`
    call.
  - `flush_lazy` (mirror.rs:448), called first in `gr_shadow_doorbell` (mirror.rs:538). It
    covers both the doorbell of a built channel and a channel's creation, and it runs before
    their plan and wait.
  - the exit summary's mirror line gains `alloc lazy: N invalidate(s) deferred, planned in B
    batch(es) at a doorbell and D at the deadline (20 ms), largest batch L invalidate(s)`.
- `shadow.rs`, `hostmem.rs`, `ptcache.rs`, `boot.rs`: unchanged. The walk cache already keeps
  every dirty table page until the next plan, so a late plan is the difference between the mirror
  and the guest's tables at that moment.

**Switches.**

| Variable | Default | Effect |
|---|---|---|
| `CARAXES_ALLOC_LAZY` | 0 | 1 or on: defer the shadow plan of a guest MMU invalidate to the next shadow doorbell, channel creation or deadline |
| `CARAXES_ALLOC_LAZY_MS` | 20 | the deadline in ms, checked at the next invalidate; 0: plan only at doorbells and creations |

**What it changes and what it does not.**

- It changes the server thread's work at an invalidate: a set insert instead of a walk and a plan.
- It changes the worker's work for allocations that are freed, or re-made at the same place,
  before the next doorbell: none instead of four RM calls per 2 MiB region.
- It changes the first doorbell after a burst of allocations. That doorbell plans them all and
  waits for their apply, work that the eager path did in the background while the guest was still
  allocating.
- It does not change the traps. With step 1 off, the guest-visible cost per allocation moves by
  the device's share of the invalidate trap, a few percent. The step matters as the prerequisite of
  step 3 and for the device CPU and host RM lock time during churn.

**Hazard.** A host channel that is already running when the guest maps new memory, and that uses
the new mapping without a new doorbell, sees it only at the next plan. This is a persistent kernel
that polls for pointers the CPU writes after allocating. Eagerly that delay is the worker's queue,
milliseconds. Lazily it is the next doorbell or the next invalidate past the deadline. The
documented ordering guarantee is the same in both modes: at a doorbell (mapping.md §4). A fault
there is an MMU fault on the VM's own channel, not an isolation breach.

**Tests** (`cargo test -p vfio-user-nvgpu`, 196 unit tests plus 2, all pass):

- `lazy::tests::off_plans_at_every_invalidate`: the switch off is the old behaviour.
- `lazy::tests::on_defers_until_the_doorbell_and_collects_the_roots`: roots are collected, a
  batch is taken once, and the counters.
- `lazy::tests::all_pdb_plans_every_root`.
- `lazy::tests::the_deadline_flushes_at_the_next_invalidate`: the flush includes the invalidate
  that triggered it, and the clock restarts.
- `lazy::tests::the_switches_parse_strictly`.
- `lazy::tests::a_late_plan_sees_the_final_tables_and_skips_what_was_freed`: on the fake FB with
  a real `ShadowFront`, several table changes between two plans give one plan with only the
  final leaf. An allocate-and-free between two plans diffs no unit at all.

**Build state** (`CARGO_TARGET_DIR=target/alloc`):

- `cargo build --release -p vfio-user-nvgpu`: clean.
- `cargo test -p vfio-user-nvgpu`: 196 + 2 pass.
- `cargo clippy --locked -p vfio-user-nvgpu --all-targets [--all-features] -- -D warnings -A clippy::unnecessary_cast -A clippy::field_reassign_with_default`: clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc -p vfio-user-nvgpu --no-deps`: clean.

**Tools.**

- `caraxes/tools/rpc-cost.py` (new): per access class, per GSP RPC (with the host calls between rx
  and tx), per invalidate episode, per churn phase (`--gap-ms`, `--phases`). It also projects the
  cost with `CARAXES_INTR_PAGE=1` and `CARAXES_BAR2_MEMFD=1`.
- `caraxes/tools/bench-cu.c`: `BENCH_CU_PHASE_GAP_MS=N` makes `churn` sleep between the context
  creation and each alloc and free loop, outside every timed operation and subtracted from the
  cycle time. With the variable unset the binary behaves as before.

## 4. Numbers the churn trace has to give

The episode analysis stands on context-creation traces, so four numbers are inferred rather than
measured:

- the number of lock cycles per cuMemAlloc;
- whether every second 1 MiB allocation writes a PTE;
- the 2.5 ms remainder of the 64 MiB allocation;
- the worker's apply time per churned region.

`rpc-cost.py --gap-ms` on one traced `bench-cu churn 1` per mode (§6) gives all four per
operation.

## 5. Where this leaves the targets

cuMemAlloc 1 MiB at 830 us is 23x native. Step 1 is already built and brings it to about 6-7x
native, 2-3x v1's 93 us. Step 3 brings it to about 1-2x native. The context
creation (bar2-memfd.md) and the allocation path share their largest item, the BAR2 PTE writes, and
their second, the RM lock's interrupt-enable toggles. So the measurement below also shows how much
of cuCtxCreate step 1 takes.

## 6. Measurement (coordinator, vm2, closing configuration, nothing run here)

vm2 alone at slot 0 with the mapping environment. The closing configuration is the P21 area, the
token pool of 64 and the spin reactor, on the Cloud Hypervisor of the closing series. The lazy
switch is measured off against on. `stack-off` and `stack-on` repeat both modes with step 1's
switches, and are optional (`MODES="off on stack-off stack-on"`).

Before the run, build `bench-cu` of this tree once as `bench-cu-gap`, the same way as `bench-cu`
(`caraxes/runs/bench-2026-09-24/commands.txt:19`), and put it into `target/bench/bin/`. The stage
step copies that directory to `~/bench`.

```sh
R=/home/silas/git/Caraxes          # the merged tree
(cd "$R" && CARGO_TARGET_DIR=target/alloc cargo build --release -p vfio-user-nvgpu)
S=/mnt/vmstore/caraxes/vfio-user-nvgpu-alloc-$(git -C "$R" rev-parse --short HEAD)
cp "$R/target/alloc/release/vfio-user-nvgpu" "$S"
M=$R/caraxes/runs/alloc-lazy-$(date +%F); mkdir -p "$M"; cd "$R"
source caraxes/runs/mapping-2026-09-24/env.sh
export CARAXES_SERVER=$S LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_BAR1_PASSTHROUGH=1 CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=spin
unset CARAXES_DOORBELL_IOEVENTFD
W=/mnt/vmstore/caraxes/vm2
for mode in ${MODES:-off on}; do
  unset CARAXES_ALLOC_LAZY CARAXES_INTR_PAGE CARAXES_BAR2_MEMFD
  [[ $mode == on || $mode == stack-on ]] && export CARAXES_ALLOC_LAZY=1
  [[ $mode == stack-* ]] && export CARAXES_INTR_PAGE=1 CARAXES_BAR2_MEMFD=1
  D=$M/$mode; mkdir -p "$D"; T0=$(date '+%F %T')
  caraxes/tools/bench-hostwatch.sh start "$D"
  caraxes/tools/bench-run.sh v2-restart > "$D/up.txt" 2>&1 || { tail -3 "$D/up.txt"; caraxes/tools/bench-hostwatch.sh stop "$D"; continue; }
  grep -E 'alloc lazy|intr page|bar2 memfd|bar1 passthrough|doorbell tokens|doorbell poll|host ABI' "$W/gpu.log" | cut -c1-300 > "$D/device-start.txt"
  caraxes/tools/bench-run.sh stage v2 >> "$D/up.txt" 2>&1
  # One traced churn process, its phases 300 ms apart, and the per-operation cost table.
  L0=$(wc -l < "$W/gpu-trace.log")
  tools/caraxes-vm.sh ssh 'cd ~/bench && BENCH_CU_PHASE_GAP_MS=300 LD_LIBRARY_PATH=/opt/nvrm/lib ./bench-cu-gap churn 1' > "$D/churn-traced.txt" 2>&1
  tail -n +"$((L0 + 1))" "$W/gpu-trace.log" > "$D/churn-trace.txt"
  python3 caraxes/tools/rpc-cost.py "$D/churn-trace.txt" --gap-ms 200 \
    --phases ctx:0,alloc1:64,free1:64,alloc64:8,free64:8,destroy:0 > "$D/rpc-cost.md"
  # bench-cu churn 20 and init, 5 reps each, device CPU per run in the MEAS lines.
  V2_RESTART=1 caraxes/tools/bench-run.sh series --targets v2 --reps 5 --metrics "init churn" --out "$D" 2>&1 | tail -2
  tools/caraxes-gate.sh cuda > "$D/cuda-gate.txt" 2>&1; tail -3 "$D/cuda-gate.txt"
  caraxes/tools/bench-hostwatch.sh stop "$D"
  tools/caraxes-vm.sh down > "$D/down.txt" 2>&1
  grep -E 'alloc lazy|mirror: |syncs \(|intr page|bar2 memfd|ERROR' "$W/gpu.log" | cut -c1-900 > "$D/device-summary.txt"
  journalctl -k --since "$T0" --no-pager -g Xid > "$D/host-xid.txt" 2>/dev/null
  xz -T0 "$D/churn-trace.txt"
done
python3 caraxes/tools/bench-summary.py "$M"/off/v2 "$M"/on/v2 > "$M/summary.md"
```

**What to read:**

- `on/device-start.txt` has the `alloc lazy: on` line. `off` has none.
- `*/rpc-cost.md`, section 4:
  - the segment list must show six segments per traced process (otherwise adjust `--gap-ms` or
    `--skip`);
  - per operation, the trapped accesses by class. This gives §4's four numbers;
  - the `stack-*` projection rows against the measured `stack-*` modes.
- `summary.md`: `CHURN` alloc and free medians for 1 MiB and 64 MiB, `INIT` cuCtxCreate and
  cuCtxDestroy, and the device CPU per run (`MEAS dev_cpu_ms`), off against on. Expected for `on`:
  alloc and free within a few percent of `off`, and device CPU for churn clearly lower.
- `on/device-summary.txt`:
  - the mirror line's `alloc lazy:` part: deferred invalidates, batches at doorbells and at the
    deadline, the largest batch;
  - the shadow summary's maps and unmaps, which should fall well below `off`'s for the same churn.
- The cuda gate green in both modes and 0 host Xid in both.
- If `on` is red where `off` is green, repeat `on` with `CARAXES_ALLOC_LAZY_MS=1`, which is nearly
  eager. Green there means a workload in the gate uses mappings without a doorbell in between (the
  hazard of §3). Both results are worth keeping.

## 7. Measured (night series, 2026-09-25 00:24-01:17, vm2)

`caraxes/runs/alloc-path-2026-09-24/night/` (`run.sh`, `summary.md`, `*/rpc-cost.md`; full
tables and reading in benchmark.md "Night series", Part 2). §6's plan with three differences:
vm2 at 1 GiB slot 3 on host 615.71.09 with other VMs active, the hybrid reactor, and no BAR1
area or pool, because the BAR2 memfd hangs with the pool (bar2-memfd.md §8). Modes: `off` (the
baseline of §1.1), `intr` (the interrupt page alone), `stack-off` (step 1) and `stack-on`
(step 1 plus `CARAXES_ALLOC_LAZY=1`); the lazy switch alone was left out for time.

| Row (median, n=5) | off | intr | stack-off | stack-on | native (closing) |
|---|---|---|---|---|---|
| trapped accesses per cuMemAlloc 1 MiB | 45.5 | 6.5 | 4.2 | 4.3 | - |
| trapped accesses per cuMemAlloc 64 MiB | 370.6 | 272.6 | 77.9 | 13.9 | - |
| cuMemAlloc 1 MiB, us | 972 | 581 | 297 | **175** | 35.8 |
| cuMemFree 1 MiB, us | 1020 | 680 | 541 | **401** | 27.1 |
| cuMemAlloc 64 MiB, us | 7337 | 5590 | 650 | **541** | 78.3 |
| cuMemFree 64 MiB, us | 4926 | 3945 | 961 | **838** | 92.6 |
| churn device CPU per run, ms | 6830 | 4960 | 2410 | 1850 | - |

§4's numbers, from `off/rpc-cost.md` (section 4, per operation): a 1 MiB allocation has 14.7
`NV_PMC_INTR_EN` writes and 24.4 VF interrupt-tree accesses, 39 interrupt-related traps against
the 18 lock toggles of §1.1 (the rest is interrupt servicing of other clients' notifier events,
mapping.md §3, which the busy host added); 2.5 invalidate accesses and 1.0 flush read, so about
half an invalidate episode per allocation; 2.0 BAR2 PTE writes on average, which confirms one
2 MiB PTE per two allocations (§1.3). The 64 MiB allocation writes 192.5 BAR2 words, not 128,
and has 134.5 interrupt-tree accesses: the 2.5 ms remainder of §1.3 is the extra PTE traffic
and interrupt servicing, not a second lock episode (27.5 lock writes, 5.6 invalidate accesses).
The worker's apply times per plan are in section 3 of each `rpc-cost.md`.

What it says against §5: step 1 (the two existing switches) brings cuMemAlloc 1 MiB from 972
to 297 us, and the lazy plans to 175 us, 4.9x native and 1.9x v1, better than the 200-250 us
expected for step 1 alone because the lazy plans also take the eager plan's work out from under
the 64 MiB invalidates (77.9 to 13.9 traps). The free path is the next target: 7.2 traps and
401 us per 1 MiB, 15x native, the two invalidates and the flush read of step 3. `on` against
`off` was not run; `stack-on` against `stack-off` shows the switch's effect: alloc 297 to 175
us, device CPU per churn run down a quarter, cuCtxCreate unchanged (533 against 534 ms), cuda
gate 10 of 11 (10 of the 11 scored rows; the gate script counts the same result as 10 of 12, with the declared-unsupported cuda-gdb row in the 12, testing.md section 3, cuda) scored (the known cuda-managed 801), 0 host Xid. Lazy exit summary of the last
instance: 8001 invalidates deferred, 330 batches at a doorbell and 407 at the deadline, largest
batch 68, and no fault attributable to a late plan.

## README file-table row

| [alloc-path.md](alloc-path.md) | Where cuMemAlloc/cuMemFree time goes (830/903 us, 6.7 ms for 64 MiB): no memory RPC reaches the device, the cost is the guest RM's trapped traffic (24 traps per mapping change: 18 RM-lock interrupt-enable writes, 5 MMU invalidate, 1 flush; plus BAR2 PTE writes), the device's host calls sit in the worker; the four steps (existing intr page + BAR2 memfd, lazy shadow plans, invalidate and flush page as memory, region-object free list) with expected cost and isolation argument; `CARAXES_ALLOC_LAZY=1` built (`lazy.rs`), `caraxes/tools/rpc-cost.py`, `BENCH_CU_PHASE_GAP_MS`, measurement plan (subagent report) |

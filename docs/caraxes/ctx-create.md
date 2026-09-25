<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# cuCtxCreate: where the time goes, and what was cut (2026-09-25)

The operator's direction of 2026-09-25: optimise cuCtxCreate. Target cuCtxCreate under 250 ms and
cuCtxDestroy under 100 ms, with the gate unchanged. Starting point: benchmark.md "Morning matrix
(2026-09-25, main fbe4dfb)" (cuCtxCreate 319 ms in bench-cu init, 437 ms in the census, native
195 and 146 ms), then main b22ab48 with the untrapped-GPPut hold of bar1-pool.md §9, which cost
cuCtxCreate 395 to 486 ms. Records: `caraxes/runs/opt-ctxcreate-2026-09-25/` (README there).

## 1. Summary

@SUMMARY@

## 2. Method

- **Census.** `census-cu` as in the trap census (optimization-backlog.md §1): cuInit x3,
  cuCtxCreate/cuCtxDestroy x5, the operations x10, each bracketed by `NV_PBUS_SW_SCRATCH(30)`
  markers, with the device trace on (`run.sh measure`). `census-table.py` gives the guest medians
  and the traps per operation, `rpc-cost.py` the host RM call histogram.
- **Phases** (`phases.py`, new): per cuCtxCreate and cuCtxDestroy window, the device's time from
  the start marker to the end marker, split into
  - *trapped*: gaps of at most 2 ms before a trapped access (the guest's time on that trap, as
    `rpc-cost.py` attributes it), by class;
  - *first-doorbell builds*: the device's handling of a shadow channel's first doorbell, by
    phase, from the line each build now logs (`shadow: chid N built in T us: plan and apply,
    TSG, channel, objects, schedule and token, doorbell`, `Mirror::note_build`,
    `crates/vfio-user-nvgpu/src/mirror.rs:1324`); in a trace without those lines (`before`) the
    gap before the doorbell line stands in;
  - *untrapped*: the other gaps above 2 ms, named by the access that ended them: the guest's own
    work, a wait for the GPU, or a trap whose handling took longer than 2 ms;
  - the background: the worker's apply time and gather descriptors in the window, and the host
    channel frees.
- **bench-cu** init and churn with 3 reps (`caraxes/tools/bench-run.sh series`), in the same
  device session after the census.
- **Scatter.** The host carried up to five other agents' device processes during these runs
  (`other-vms-at-start.txt` per run). Two `before` sessions of the same binary gave census
  cuCtxCreate 356 ms and 1.90 s (cuInit 14 and 160 ms). The rows below are therefore paired: the
  device's own phase times (which the other VMs move little) and back-to-back A/B sessions.

## 3. Where the time went (before: d96cc59, the §9 hold on)

One context creation of the `before` census (repetition 2, 415 ms; the device trace, times from
the start marker):

| from, ms | to, ms | what the guest waits on |
|---|---|---|
| 0 | 94 | its own setup: GSP RPCs (TSGs, context shares, 20 channels at 61 to 94 ms, 20 CE and 8 compute objects), 30 invalidates of its own root 0xa9600000, BAR2 flush reads |
| 94 | 212 | the first doorbell (chid 10, a CE channel): the device builds the shadow VAS and plans the context's root for the first time, 16128 leaves, and waits for the worker: apply 109 ms, of it 99 ms in 30 gather descriptors (3.3 ms each) |
| 212 | 222 | the first doorbells of CE chids 10 to 13: 2 to 4 ms each |
| 222 | 319 | the first doorbells of GR chids 2 to 9: 9 to 19 ms each (host TSG, channel, compute and CE objects, schedule, token) |
| 319 | 346 | CE chids 18 to 21 and 14 to 17: 2 to 4 ms each |
| 346 | 415 | invalidates of the context's root after libcuda's next allocations, held until applied (§9: the USERDs are in the pool now), 512-leaf gathers of 1 to 6 ms |

The split over the five repetitions (`before/phases.md`, medians, ms):

| part | cuCtxCreate | cuCtxDestroy |
|---|---|---|
| window | 356 | 152 |
| trapped, all | 129 | 103 |
| of it BAR2 flush reads (the memfd sync inside, bar2-memfd.md §1) | 60 | 45 |
| of it MMU invalidates | 30 | 17 |
| of it the GSP queue (RPCs; at destroy the mirror's host frees run in them) | 21 | 39 |
| of it usermode doorbells | 9 | 0 |
| first doorbells (builds, including the first plan's wait) | 173 | 0 |
| untrapped gaps above 2 ms | 47 | 55 |
| worker apply in the background | 28 | 7 |

By kind:
- **Host RM calls.** The mirror's channel builds (TSG, channel, engine objects, schedule, token)
  and, at the first doorbell, the first plan's apply. In the whole census the shadow made 3889
  host RM calls in 694 ms; 133 gather allocations took 361 ms of it, 2.7 ms each
  (`before/census-rpc-cost.md` section 3). A gather is dominated by its window: 512 `mremap`
  calls (about 1.7 ms for 512 scattered pages on this host) and the `munmap` (0.6 ms), then the
  OS descriptor (`hostmem.rs:1473`).
- **Device work.** The 20 builds: GR channels 9 to 19 ms, CE channels 2 to 4 ms; the host RM does
  the same work for native libcuda, whose whole creation is 146 to 195 ms.
- **Waiting.** Before the first doorbell the guest waits on its traps (flush reads, invalidates,
  the GSP queue). At each first doorbell it waits on the trapped doorbell write for the build.
  After the last one it waits on the invalidates the §9 rule holds. It never waits for the GPU:
  libcuda rings each channel right after the previous doorbell returns (0.1 ms), and the first
  work completes behind the later builds.

## 4. What changed

### 4.1 Part 1: the §9 hold only for plans that add or change mappings (commit 3880068)

bar1-pool.md §9.1 has the argument, the build and the tests. An invalidate of a root with an
untrapped USERD is planned at once, and the guest waits for the worker only when the plan adds or
changes a mapping (`untrapped::must_wait`, `crates/vfio-user-nvgpu/src/untrapped.rs:68`); a plan
that only removes mappings leaves the shadow a superset of the guest's tables. Frees get their
lazy cost back (cuMemFree 64 MiB 2.6 to 0.9-1.2 ms); context creation keeps its held allocations.

### 4.2 Part 2: the shadow ahead of the first channel (commit 56aacd2)

The prefetch (`Mirror::prefetch`, `mirror.rs:641`; `CARAXES_SHADOW_PREFETCH=0` turns it off): at a
guest invalidate of a root that a guest TSG runs on and that has no shadow channel yet, the
device builds the shadow VAS and plans the root without waiting; the worker applies it while the
guest goes on with its setup. The roots are `Registry::tsg_roots` (`channel.rs:412`): a TSG's
own VAS, the client's only rooted VAS, and the VAS of every context share of a client that holds
a TSG. CUDA's TSGs name no VAS and its client has more than one rooted VAS, so without the
context shares the prefetch started only after the last TSG (run `wip2`: 0 ms gained). The first
channel's creation then plans what changed since and waits for the worker exactly as before
(`gr_shadow_doorbell`, `mirror.rs:938`), so no host channel starts on a mirror older than the
guest's tables, and every plan is filtered as before (D16/D17). A prefetched shadow whose root no
guest TSG uses any more goes to the worker's teardown (`reap_prefetched`, `mirror.rs:684`); the
normal reap skips a shadow that is waiting for its first channel.

Fail-closed points: the prefetch only plans (the shadow has no channel, so nothing runs on it);
it applies to roots in this VM's reservation only (`prefetch_candidate`, `mirror.rs:383`); the
untrapped-GPPut rule and the lazy path take over at the first channel.

The build phases are counted per build and in the mirror's exit line (`build_summary`,
`mirror.rs:2685`), with the host channel and TSG frees.

### 4.3 Tried and dropped

- **One error-notifier object per guest page** (commit 56aacd2, removed in 9f712dc): 0 reuses in
  240 builds; every channel names its own notifier page.
- **Retire host channels at the guest's free, free them with the TSG** (run `wip4`, not
  committed): each channel free became a TSG preempt (137 ms for 240, against 691 to 902 ms of
  channel frees), but the host RM frees the TSG's channels one by one inside the TSG free (806
  ms for 48 TSGs, against 71 to 115 ms): no saving, so the synchronous frees stay.
- **Create the host channels lazily at their first doorbell**: that is already the design (the
  mirror builds a shadow channel at its first doorbell, `gr_shadow_doorbell`), and libcuda rings
  all 20 channels inside cuCtxCreate, so there is nothing to defer.
- **Build the channels in parallel on the worker**: the host RM serialises channel and object
  allocation under its locks and one GSP queue, and a build must restore the guest's GPGet and
  GPPut in the USERD after the host RM cleared it (`mirror.rs` "Restore the guest's GPGet/GPPut"),
  which only the server thread can do through the FB store; an asynchronous first doorbell
  would race the guest's next GPPut store through the pool (§9). Not built.
- **A pool of host channels of a freed context**: a host channel's USERD offset, GPFIFO VA and
  VAS are fixed at allocation, and its GR context would carry the previous context's state where
  a fresh channel starts from the golden context (a guest-visible difference). Not built.

## 5. Numbers

@NUMBERS@

## 6. What is left

@LEFT@

## 7. Tests

- `untrapped::tests::a_plan_that_only_removes_mappings_cannot_fault_a_run_ahead_channel`
  (`untrapped.rs:222`), `ptcache::tests::an_update_that_only_removes_leaves_does_not_grow`,
  `ptcache::tests::leaves_grew_compares_whole_leaves`,
  `lazy::tests::the_rule_takes_only_its_roots_out_of_the_batch` (part 1);
- `channel::tests::tsg_roots_name_the_vas_a_tsg_runs_on` (`channel.rs:1799`: a TSG's VAS, the
  client's only rooted VAS, a context share only while its client holds a TSG, nothing once the
  TSG, the VAS or the client is being freed), `mirror::tests::only_a_tsg_root_in_the_reservation_is_prefetched`
  (`mirror.rs:3146`) (part 2).

`cargo test -p vfio-user-nvgpu`: @TESTS@.

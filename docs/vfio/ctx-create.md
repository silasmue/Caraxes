<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# cuCtxCreate: where the time goes, and what was cut (2026-09-25)

> **Not ported to dev (history merge of 2026-09-28):** the prefetch, the hold refinement and the build-phase timing described here are not in dev's code; see bar1-pool.md §9.1.

The operator's direction of 2026-09-25: optimise cuCtxCreate. Target cuCtxCreate under 250 ms and
cuCtxDestroy under 100 ms, with the gate unchanged. Starting point: benchmark.md "Morning matrix
(2026-09-25, main fbe4dfb)" (cuCtxCreate 319 ms in bench-cu init, 437 ms in the census, native
195 and 146 ms), then main b22ab48 with the untrapped-GPPut hold of bar1-pool.md §9, which cost
cuCtxCreate 395 to 486 ms. Records: `caraxes/runs/opt-ctxcreate-2026-09-25/` (README there).

## 1. Summary

- **cuCtxCreate** is three spans (§5, `spans.py`): the guest's setup up to its first doorbell,
  the 20 first doorbells (the host channel builds), and the tail of libcuda's allocations after
  them. The medians of the reference session `before` are 179, 76 and 70 ms. The setup is
  guest-side work: about 208 BAR2 flush reads, 85 GSP RPCs and 97 invalidates. The device spends
  about 8 ms of it on invalidates and 4 to 6 ms on the RPCs. The rest moves with the host's load:
  104 ms at best, session medians 179 to 715 ms over the three `before` sessions, at a load
  average of 20 to 35 on 16 cores. The rings are host RM work, serial per channel: channel 0.9
  to 2.6 ms, objects 0.2 to 5 ms, TSG 0.8 ms, schedule and token 0.1 to 0.6 ms.
- **Part 1** (the §9 hold refinement, bar1-pool.md §9.1): frees are not held any more.
  cuMemFree 64 MiB went from 2.6 ms to 1.0 to 1.2 ms (bench-cu churn) and from 5.4 ms to 1.6 ms
  (census). The allocburn reproduction stays at 0 Xid (20 of 20, `part1-allocburn`,
  `after-allocburn`).
- **Part 2** (the prefetch): the context's shadow is built and planned in the background during
  the guest's setup. The rings span went from 76 ms to 61 ms (median; minimum 72 to 50 ms). With
  `CARAXES_SHADOW_PREFETCH=0` in the same series (`after-noprefetch`) it was 218 ms.
- **Targets.** cuCtxCreate under 250 ms and cuCtxDestroy under 100 ms were not reached on the
  shared host. The census medians of the end state are 332 ms and 181 ms (`after`). The best
  single creations were 232 ms (`before`) and 245 ms (`part1`). What is left lies outside the
  mirror and the shadow: the guest's own work in the setup and the tail, and the BAR2 flush
  reads and GSP RPCs of the destroy (§6). No cut was made that weakens a fail-closed rule (§4.3).

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
    `crates/vfio-user-nvgpu/src/mirror.rs:1325`); in a trace without those lines (`before`) the
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

The prefetch (`Mirror::prefetch`, `mirror.rs:643`; `CARAXES_SHADOW_PREFETCH=0` turns it off): at a
guest invalidate of a root that a guest TSG runs on and that has no shadow channel yet, the
device builds the shadow VAS and plans the root without waiting; the worker applies it while the
guest goes on with its setup. The roots are `Registry::tsg_roots` (`channel.rs:412`): a TSG's
own VAS, the client's only rooted VAS, and the VAS of every context share of a client that holds
a TSG. CUDA's TSGs name no VAS and its client has more than one rooted VAS, so without the
context shares the prefetch started only after the last TSG (run `wip2`: 0 ms gained). The first
channel's creation then plans what changed since and waits for the worker exactly as before
(`gr_shadow_doorbell`, `mirror.rs:938`), so no host channel starts on a mirror older than the
guest's tables, and every plan is filtered as before (D16/D17). A prefetched shadow whose root no
guest TSG uses any more goes to the worker's teardown (`reap_prefetched`, `mirror.rs:686`); the
normal reap skips a shadow that is waiting for its first channel.

Fail-closed points: the prefetch only plans (the shadow has no channel, so nothing runs on it);
it applies to roots in this VM's reservation only (`prefetch_candidate`, `mirror.rs:383`); the
untrapped-GPPut rule and the lazy path take over at the first channel.

The build phases are counted per build and in the mirror's exit line (`build_summary`,
`mirror.rs:2687`), with the host channel and TSG frees.

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
- **Build the channels ahead of their first doorbell, or in parallel on the worker.** The host
  channel's USERD is the guest's page (in the BAR1 pool, bar1-pool.md §9), and the host clears
  it when it creates the channel: the mirror saves the guest's GPGet and GPPut before the alloc
  and writes them back after it (`mirror.rs:1284`, measured 2026-09-23: without the restore the
  PBDMA saw GPPut 0 and fetched nothing). The CPU-side RM clears an FB USERD only under full
  SR-IOV (`kernel_channel.c:2305-2318` of 615.71.09), so the clear comes from GSP-RM, inside the
  alloc. A build that runs while the guest may store GPPut therefore loses a store it cannot
  see: the store goes through the pool without a trap, and the clear can land after it. At the
  first doorbell the store has happened and the doorbell traps, so the save and restore are
  exact; before it, or asynchronously after it, nothing tells the device which value is the
  guest's. The builds also do not overlap each other: the host RM serialises channel and object
  allocation under its API lock and the one GSP queue. Not built: it would weaken the rule that
  no host channel starts on a USERD the device cannot vouch for.
- **Free the host channels on the worker** (cuCtxDestroy: 20 channel frees of 1.5 ms each in
  `after`, 30 ms of its 181 ms). A host channel that is still scheduled after the guest's free
  can load a GPPut the guest writes into the reused USERD slot and fetch from it; freeing it
  later needs a synchronous stop first. `NVA06F_CTRL_CMD_STOP_CHANNEL` also writes an RC
  notification into the error notifier, which is the guest's page (`kchannelCtrlCmdStopChannel_IMPL`,
  `kernel_channel.c:1892-1925`). A preempt costs 0.57 ms (`wip4`). The saving would be about
  20 ms of the 30 ms, and the memory frees that follow in the same destroy would have to wait
  for the worker. Not built.
- **A pool of host channels of a freed context**: a host channel's USERD offset, GPFIFO VA and
  VAS are fixed at allocation, and its GR context would carry the previous context's state where
  a fresh channel starts from the golden context (a guest-visible difference). Not built.

## 5. Numbers

All on vm3, 610 guest, the recommended configuration (README of the run folder). The host was
shared with up to five other device processes, so a session's medians move by a factor of two
to seven with the load (`before` against `before-2`, `after` against `after-2`, both the same
binary). The device's own spans are steadier than the window.

**The spans of cuCtxCreate** (`spans.py`, census, 5 repetitions; median, minimum in brackets, ms):

| run | binary | window | setup | rings | tail |
|---|---|---|---|---|---|
| before | d96cc59 | 356 (232) | 179 (104) | 76 (72) | 70 (52) |
| before-3 | d96cc59 | 548 (314) | 257 (134) | 90 (73) | 96 (89) |
| part1 | 3880068 | 407 (245) | 225 (106) | 167 (79) | 92 (60) |
| part2 | 56aacd2 | 337 (242) | 180 (113) | 131 (59) | 75 (62) |
| after | 9f712dc | 332 (312) | 181 (119) | 61 (50) | 80 (58) |
| after-noprefetch | 9f712dc, prefetch off | 801 (408) | 415 (193) | 218 (69) | 150 (143) |

**The census and bench-cu rows** (`compare.py`; census medians; bench-cu medians over 3 reps with
the range, us or ms as named):

| row | before | before-3 | part1 | part2 | after | after-noprefetch |
|---|---|---|---|---|---|---|
| census cuInit, us | 13819 | 53338 | 29639 | 41130 | 32040 | 81517 |
| census cuCtxCreate, us | 355996 | 547654 | 406736 | 336600 | 332386 | 799283 |
| census cuCtxDestroy, us | 151766 | 214798 | 156822 | 165420 | 180619 | 332996 |
| census cuMemAlloc 1 MiB, us | 332 | 1533 | 405 | 496 | 630 | 797 |
| census cuMemAlloc 64 MiB, us | 2972 | 17061 | 3720 | 3655 | 3498 | 6718 |
| census cuMemFree 1 MiB, us | 404 | 2246 | 553 | 544 | 541 | 682 |
| census cuMemFree 64 MiB, us | 5370 | 13639 | 1562 | 1884 | 1549 | 3384 |
| bench init cuinit_ms | 17.5 [17.3-21.3] | 37.5 [27.4-50.6] | 20.4 [17.9-21.4] | 28.2 [14.0-33.4] | 20.8 [15.4-24.7] | 20.6 [17.0-21.9] |
| bench init ctxcreate_ms | 410.8 [384.0-577.9] | 446.1 [386.5-498.6] | 600.0 [567.3-686.9] | 380.1 [357.5-516.5] | 409.7 [401.6-599.4] | 532.7 [414.5-552.7] |
| bench init ctxdestroy_ms | 194.2 [173.7-208.2] | 247.2 [224.3-398.5] | 157.5 [146.4-181.0] | 222.7 [154.0-261.5] | 268.2 [196.1-462.2] | 225.2 [171.6-319.8] |
| bench churn alloc 1 MiB, us | 206.9 [206.0-388.4] | 409.0 [400.1-425.0] | 214.8 [206.7-233.9] | 234.4 [212.1-398.0] | 205.8 [203.6-408.7] | 373.2 [217.7-440.5] |
| bench churn alloc 64 MiB, us | 2388.3 [2374.3-2418.1] | 3654.9 [2484.1-4811.8] | 2681.6 [2393.9-2697.6] | 2908.9 [2761.2-3529.7] | 2748.0 [2445.6-2859.1] | 3081.4 [3002.8-3391.4] |
| bench churn free 1 MiB, us | 340.5 [327.7-405.8] | 656.7 [442.7-672.9] | 405.9 [306.0-564.3] | 329.9 [317.6-429.4] | 407.4 [401.8-597.8] | 588.3 [410.5-612.5] |
| bench churn free 64 MiB, us | 2594.0 [2552.5-2606.7] | 3941.3 [2568.1-4979.6] | 1006.1 [901.2-1106.8] | 1236.7 [1178.9-1563.0] | 1219.6 [1018.2-1266.0] | 1306.7 [1211.4-1405.3] |
| other device processes at start | 1 | 3 | 1 | 3 | 3 | 3 |

**The phases** (`phases.py`, medians, ms; the `before` trace has no build lines):

| part | before | part2 | after | after-noprefetch |
|---|---|---|---|---|
| cuCtxCreate window | 356 | 337 | 332 | 801 |
| trapped, all classes | 129 | 146 | 156 | 216 |
| first-doorbell builds | (76 as rings) | 161 | 99 | 296 |
| untrapped gaps above 2 ms | 47 | 51 | 85 | 152 |
| worker apply in the background | 28 | 39 | 36 | 53 |
| cuCtxDestroy window | 152 | 166 | 181 | 333 |
| of it host channel frees in RPC handling | (not logged) | 35 | 30 | 44 |

**The gate and the loops.**
- The cuda gate (`after`, 9f712dc): 10 of 11 scored pass, only the known cuda-managed 801 fails,
  cuda-gdb declared unsupported (`after/gate-cuda.txt`).
- allocburn (20 iterations of convburn plus `allocburn.py`, bar1-pool.md §9.1): `part1-allocburn`
  and `after-allocburn` 20 of 20 each, 0 host Xid, 0 refused barriers. The final build:
  @FINAL_ALLOCBURN@.
- Host Xid: none in any run of this folder. `host-xid.txt` of each run lists the device pid.

**The final build** (a8f66e3, the code of a8b3a54: the ran-ahead counter narrowed to the channel's
own root): @FINAL@

## 6. What is left

- **The guest's setup and tail** (about 250 of 332 ms in `after`). The device's work in it is
  small: invalidate handling about 8 ms, GSP RPC handling 4 to 6 ms per creation. About 270
  BAR2 flush reads each end a gap of the guest's own work, 45 to 63 ms per creation in total
  (counted from the gap after each flush read in the census traces). That is vCPU time of the guest RM and libcuda. It
  scales with the host's load and would shrink with a quiet host or fewer trap round trips.
  The next lever is the flush page as memory, with the memfd sync moved to the doorbell
  (optimization-backlog.md item 3; bar2area.rs, not this package).
- **The 20 host channel builds** (50 to 60 ms at best): serial host RM allocations. They cannot
  start before the guest's first GPPut store (§4.3). What remains is making each one cheaper on
  the host (it takes 146 to 195 ms for native libcuda's whole creation, with the same calls).
- **cuCtxDestroy** (152 to 181 ms): the trapped GSP RPCs of 65 frees (45 ms, 30 of them the host
  channel frees), 169 flush reads (52 ms) and the guest's gaps. The worker-side free of §4.3
  would save about 20 ms at the cost of a synchronous stop per channel.

## 7. Tests

- `untrapped::tests::a_plan_that_only_removes_mappings_cannot_fault_a_run_ahead_channel`
  (`untrapped.rs:222`), `ptcache::tests::an_update_that_only_removes_leaves_does_not_grow`,
  `ptcache::tests::leaves_grew_compares_whole_leaves`,
  `lazy::tests::the_rule_takes_only_its_roots_out_of_the_batch` (part 1);
- `channel::tests::tsg_roots_name_the_vas_a_tsg_runs_on` (`channel.rs:1799`: a TSG's VAS, the
  client's only rooted VAS, a context share only while its client holds a TSG, nothing once the
  TSG, the VAS or the client is being freed), `mirror::tests::only_a_tsg_root_in_the_reservation_is_prefetched`
  (`mirror.rs:3148`) (part 2).

`cargo test -p vfio-user-nvgpu`: 366 passed (default), 370 passed and 1 ignored (`--features v615`), plus 2 in the binary; clippy and rustdoc clean.

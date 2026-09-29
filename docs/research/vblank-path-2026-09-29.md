<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# The vblank path: display-page writes, invalidates, flush reads (2026-09-29)

Scope: why a trapped display-page write costs 147-180 µs and an MMU invalidate trigger 267-279 µs in the traced vkmark run of [vkmark-frame-2026-09-28.md](vkmark-frame-2026-09-28.md); what is needed for correctness and what is not; the fix on branch `vblank-2026-09-29`. `T` = `/mnt/vmstore/caraxes/night-2026-09-28/vkmark-trace-dev/trace-{cube,clear}.txt`, `G` = `gpu.log` in the same folder, `D` = `crates/vfio-user-nvgpu/src/`. Gaps are p50 cube / clear between consecutive trapped accesses (R/W lines) unless marked.

## Status on train-2026-09-30

- The branch's commit 95f1af11 was not taken as a whole; each part is on the train branch in this form:
  - display-page writes as no sync point: the display page's trapped writes are served before the device mutex (f294c275, device.rs `Routes`) and never reach `after_bar0_write` or the BAR1 pool's poll;
  - in-place page compare: 5fcf62ae (`SharedView::page_equals`, the BAR2 memfd syncs compare in place);
  - trapped acks feed the delivery histograms and one trace flush per write: ported (026b837d on the train branch);
  - the clippy allow on the software CE's plan closure: not needed on the train branch (clippy clean with tools/check.sh's flags).
- The A/B below was never run; the expected gains are estimates.

## Method

- A trapped access's line is stamped by `Trace::access` after the model handled it and **before** `Device::after_bar0_write` [D device.rs `region_write`]. The work after a write therefore shows in the gap *after* its stamp, and the guest waits for it: the server replies when `region_write` returns, and Cloud Hypervisor's client waits for the reply [vendor/vfio-user-rs src/lib.rs `region_write`, both sides].
- Where the guest follows a write at once with another trap, the gap is the round trip (25-30 µs, `TIME`→`TIME`) plus the device's work after the write: no guest time in between.

## Facts

### A display-page write

- The page's own handler is cheap: `FePage::write` takes the page lock, scans 28 watched words, publishes, and raises at most once through an eventfd [D display/fe_page.rs].
- `Bar0::write` returned `WriteEffect::Model` for it, so `after_bar0_write` treated every display-page write as a BAR1 pool sync point [D bar0.rs, device.rs]:
  - `bar1pool::poll_model` → `poll_at` → `sync_inner` → **`merge_replica`** for each replica, merge on, because `BootModel::pool_merge_after` does not exclude the page [D boot.rs].
  - The run had one replica: guest BAR1 0x210000 maps FB 0xa9990000 a second time [G: `ERROR: bar1 pool ... the guest maps it twice`; `7 replica(s) made, 1 now, 12710 merge(s) at sync points`].
  - Its home lives in the pool, so every merge reads two 4 KiB pages from the pinned BAR1 mapping, the area page and the home [D bar1pool.rs `merge_replica`, `read_home`].
- The merge costs about 110-160 µs on the guest's trap:

| Write, then the guest's next trap | Poll after the write | p50 cube / clear (µs) | n | Evidence |
|---|---|---|---|---|
| `W 0x70010` (L2 flush) → its own poll read | merge | 149 / 200 | 9 / 6 | T |
| `W 0xb80f00` (L2 sysmem invalidate) → its poll read | merge | 160 / 190 | 10 / 6 | T |
| `W 0x696000` (window kick) → `R 0x696004` | merge | 152 / 147 | 45 / 46 | T |
| `W 0xb830b0` (invalidate trigger) → its busy read | no merge | 41 / 40 | 1539 / 1560 | T |
| `W 0x110c84` (GSP MSGQ_TAIL) → `R 0x110094` | no merge | 26 / 25 | 383 / 390 | T |

- **Per latched vblank the handler paid three merges** [T]:

| Step of the vblank handler | p50 cube / clear (µs) | Contains |
|---|---|---|
| window 6 latch → `W 0x611858` (AWAKEN_WIN echo) | 73 / 70 | raise, reactor, MSI, handler entry |
| `W 0x611858` → `W 0x61185c` (CORE_RESET) | 180 / 147 | merge after 0x611858, `notifyEvents` |
| `W 0x61185c` → first `TIME` read | 152 / 133 | merge after 0x61185c |
| 6 `TIME` reads | ~30 each | plain traps |
| last `TIME` → `W 0x61180c` (LAST_DATA reset) | 37 / 37 | plain trap (its merge comes after the stamp) |
| `W 0x61180c` → first MSI re-arm | 110 / 101 | merge after 0x61180c, handler exit |

- `W 0x61180c` looks cheap only because its merge falls after its stamp; the re-arm, and with it every held GR completion, waited for it [T; vkmark-frame §"The 60 Hz window"].
- The page's delivery histograms stayed empty: the trapped path did not record them [G `display page delivery: ... 0 of 0`].

### The MMU invalidate trigger

- 1216 / 1222 of 1582 / 1620 invalidates name pdb 0xe7543000, flags 0xc1, no shadow or mirror VA space [T `mmu invalidate: pdb 0xe7543000 flags 0xc1 (no mirror or shadow VA space rooted there ...)`].
- Trigger write `W 0xb830a4` → `W 0xb830b0`: p50 279 / 267 µs, p10 191 / 192 [T]. Upper-PDB and trigger writes follow each other with no guest work.
- `mmu_invalidate_write` runs `flush_point` before anything else [D boot.rs]:
  - `FakeFb::bar2_sync`: 7471 syncs, 971 ms, **130 µs mean** [G `bar2 memfd`]. It compares 162 mapped memfd pages with their snapshots per sync (1211043 compares), about 1.3 MB of memory traffic.
  - `sync_untracked`: the replica merge, about 110 µs (above).
  - Then `fb.flush`, the P17 root watch, `mirror_invalidate`, `shadow_invalidated`.
- **Needed for correctness, kept synchronous:**
  - `shadow_invalidated` hands *every* invalidate, a non-shadow pdb included, to the lazy planner. `OnInvalidate::Flush` at the 20 ms deadline plans every pending shadow root in this call [D mirror.rs, lazy.rs]. Its walks must see the guest's BAR2 table writes and merged replicas, so `flush_point` has to precede it.
  - With the untrapped-GPPut rule, a held root's plan and the busy bit (the invalidate gate) follow the same flush point [D mirror.rs `shadow_invalidated`, invgate.rs].
- **Cost without a correctness role:** only the memory traffic of the compare. `SharedView::load_page` copied each page before comparing it.
  - Host microbenchmark, 160 pages, cache thrashed first: copy+compare 128.5 µs, in-place compare 111.0 µs; warm: 16.1 and 13.4 µs [C microbenchmark on the host, memfd pages against heap snapshots, not committed].
  - The sync is bound by memory bandwidth. Only write tracking would make it O(changed); the device cannot see the guest's stores to the memfd.

### The flush read

- `R bar3 0xfea000` → next: p10 29, p50 42-43 µs against `TIME` p10 25, p50 29-33 [T].
- The handler only marks the sync owed (`DeferredFlush::mark`) and reads 4 bytes of FB [D boot.rs `bar_access`; G `33428 deferred`]. About 4 µs is device time; the rest of the p50 is the RM's BAR2 PTE writes between two flushes.
- Nothing to take out on the device side; the trap round trip remains [vkmark-frame fix 5].

### The window kick (not changed)

- `R 0xb830b0` → `W 0x696000`: p50 263 / 272 µs, about 230 µs of it the kick's own handling [T].
- Inferred from the code: each window `UPDATE` resolves up to three context DMAs. Each `engine::resolve` reads the whole 8 KiB RAMHT from FB [D display/engine.rs `window_update`, `resolve`].
- With the kick page (dev default since 4dde63e1) the guest no longer waits for it; the reactor does, under the device lock, followed by a merge (`Device::display_kick_from_page`).

## Consequence for Caraxes

Changes on `vblank-2026-09-29` (no new switch):

- **Display-page writes are no sync point.**
  - `Bar0::write` returns `WriteEffect::Status` for a write the display page takes. `after_bar0_write` then runs neither the P21 check nor the pool poll, as for a config-mirror write.
  - Counted in the exit summary: `display page writes: N (the vblank and awaken acks), no pool poll after them either`.
- **Trapped acks feed the delivery histograms.** `FePage::write` records the raise → `EVT_STAT` ack and marker → `CORE_RESET` times as the scan does: the device's own vblank latency in `gpu.log`.
- **In-place page compare.** `RemapTable::scan` compares each mapped page in place (`SharedView::page_equals`, `memcmp`) and copies only a page that differs.
- **One flush for a write's notes.** `after_write` writes the boot notes with `Trace::events`. The ~190 queued host-notifier lines of one window cost one write syscall instead of ~190 [vkmark-frame, trace artefact].
- `#[allow(clippy::type_complexity)]` on the software CE's plan closure (channel.rs, 8dbce31a): clippy 0.1.89 failed dev's `-D warnings` gate there.

Why the ordering holds:

- `FePage::write` touches no FB, BAR1 or guest memory; a pool poll after it has nothing of its own to publish. The guest orders its BAR1 changes with the flush read and the MMU invalidate (P21, bar1-pool.md §10). Those stay sync points, as do the L2 operations, the GSP registers, the kicks and every other model write.
- Data written only through an aliased BAR1 page reaches its home at the guest's next flush or invalidate, as documented (bar1-pool.md §10). The display acks merged it earlier only by accident.
- Untouched: the untrapped-GPPut ran-ahead detector (`note_ran_ahead`, the doorbell's check), the GPPut barrier (`gp_put_barrier`), the MEM_OP hold (c8f9ce08, channel.rs `doorbell_holding`), the invalidate gate and the lazy planner. None of them runs in or depends on the display page's handler. The invalidate's `flush_point` still runs in full before `shadow_invalidated`.
- The in-place compare reads the same bytes at the same point of the scan; a page that differs takes the old path.

Tests:

- `device::tests::a_display_page_write_is_no_bar1_pool_sync_point`: a guest write through the replica survives the three acks unmerged, and the next L2 flush merges it. It fails on the old code (`Some(0xabcd)` after the acks).
- `display::fe_page::tests::trapped_acks_feed_the_delivery_histograms`.
- `bar2area::tests::the_in_place_compare_sees_every_byte`.
- `device::tests::a_config_mirror_write_does_not_poll_the_bar1_pool`: unchanged, passes.
- `cargo test -p vfio-user-nvgpu`: 709 passed, 1 ignored, and the 2 selftests; clippy with the flags of tools/check.sh is clean, with and without `--all-features`.

Expected (estimate; the gain needs a replica, check `G`: `replica(s) made, K now` with K ≥ 1):

| Where | Before (p50) | Expected | Why |
|---|---|---|---|
| latch → `W 0x61180c` | 642 / 572 µs | ~350-420 µs | two merges out |
| `W 0x611858` → `W 0x61185c` | 180 / 147 µs | ≤ 80 µs | merge out; `notifyEvents` stays |
| `W 0x61185c` → first `TIME` | 152 / 133 µs | ≤ 60 µs | merge out |
| `W 0x61180c` → first re-arm | 110 / 101 µs | ≤ 30 µs | merge out |
| completions held per latched vblank | ~0.6-0.75 ms | ~0.3-0.45 ms | the three merges |
| MMU invalidate trigger | 279 / 267 µs | ~255 µs | in-place compare, ~15-20 µs |
| device lock per second at 60 Hz | - | ~20 ms less | 180 merges |

Per flip (Mutter's path): about −110 µs (the ack's merge) −30-40 µs (two cheaper syncs); the 23 flush reads and the invalidates' round trips stay.

## What to measure

- vkmark cube and clear, glmark2, 3 reps each, old and new binary, same launcher settings (trace on); then the smoke suite and a Moonlight stream.
- From `T`, the rows of the vblank table above (same awk: pairs of consecutive R/W lines, latch lines, the first `msi re-arm` after `W 0x61180c`).
- From `G`:
  - `display page writes: N` ≈ 3 × latched + 1 × unlatched vblanks;
  - `display page delivery: raise to the guest's EVT_STAT ack p50 <= ...` is now filled (target: bucket ≤ 511 µs);
  - `inconsistencies: none` and `0 compare-and-swap(s) lost`;
  - the `bar2 memfd` mean (total / syncs) about 10 % lower;
  - fewer `merge(s) at sync points` per second.
- Stream check: flips complete at 60 Hz, no stall and no repeated flip (the awaken acks behave as before).

## Open

- **`TIME` reads**: 6 per vblank handler, about 180 µs.
  - `CARAXES_USERMODE_PAGE=1` serves them from memory, but it also moves the doorbell into the page.
  - A read-only usermode page (Cloud Hypervisor patch 0007, as for the display page) would keep the doorbell tokens' ioeventfds only if KVM routes stores to a `KVM_MEM_READONLY` slot to an ioeventfd (gsp-transport-classes.md, question 2).
- **Invalidate round trips**: `CARAXES_INVALIDATE_PAGE=1` removes four traps per invalidate (about 120 µs); not GPU-validated [intr-page.md §11.3].
- **The window kick**:
  - a validated cache of resolved context DMAs (re-read the cached entry and object, 28 bytes, instead of 16-24 KiB) would cut about 200 µs per `UPDATE` off the reactor's device-lock time;
  - the merge after the kick orders nothing for the kick itself.
- **The replica**: one aliased BAR1 page costs about 110 µs at every remaining sync point.
- **The memfd compare**: O(mapped pages) until the VMM supplies a dirty log of the BAR2 memfd slot.

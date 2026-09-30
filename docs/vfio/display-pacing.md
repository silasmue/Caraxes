<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Display pacing: why GNOME flips at 15-35 Hz on a 60 Hz display clock

Question (2026-09-26): with the device lock holds excluded (`vm4-615-invhold/NOTES.md`), why
does the guest compositor kick the display channel only 21-44 times per second with the cursor
moving and no client, and 15-20 times while Sunshine streams with NVENC?

- Evidence: the device traces of `caraxes/runs/nvenc-2026-09-25/vm4-615-invhold/A-invhold/w*/`
  (`trace-window.txt.xz`, microsecond timestamps), cross-checked with
  `vm4-615-latency/c1-baseline/`.
- Tool: `caraxes/tools/display-pace.py TRACE...`. Output for all windows:
  `vm4-615-invhold/pace-windows.txt`.
- Terms: *kick* is `W bar0 0x00696000` with an `UPDATE`, *latch* is
  `display: window 6 latch N at tick T`, *flush read* is `R bar3 0x00fea000` (the RM's
  `kbusFlush`, trapped BAR2 page), *invalidate* is `W bar0 0x00b830b0`.

## 1. The per-frame timeline (A-invhold)

| Window | Kicks/s | Latch → next kick p50 / p90 ms | of it: latch → pre-kick run | the run (accesses) | run end → kick |
|---|---|---|---|---|---|
| w1 idle, no client | 21.0 | 20.2 / 83.5 | 11.0 | 7.2 (25) | 0.7 |
| w2 cursor, no client | 34.8 | 21.6 / 33.9 | 6.6 | 12.8 (25) | 0.8 |
| w3 idle, streaming | 19.6 | 43.9 / 68.0 | 36.1 | 6.7 (14) | 0.7 |
| w4 cursor, streaming | 15.4 | 54.0 / 110.6 | 47.7 | 4.4 (7; p90 25) | 0.8 |
| w6 cursor, streaming | 18.1 | 32.6 / 67.3 | 24.5 | 6.3 (13) | 0.6 |

What the device does is on time:
- **Tick period** 16.665-16.667 ms in every window. The device reports 60.000 Hz.
- **Kick → latch**: p50 8.5-10.5 ms, which is half a period, as it should be.
  - 2212 of 2294 flips latch at the first tick after their kick.
  - The other 82 latch one tick *earlier* by the fit: the kick came in while the tick waited
    for the device lock.
  - No flip completes a frame late. That rules out candidate 1.
- **Latch after its tick** (the device lock wait of the clock thread): p50 1.3-2.5 ms, p99
  10-21 ms. The vblank MSI follows the latch in the same `fire()`. That is late, but constant
  enough (candidates 2 and 3).

The frame is lost between the latch and the guest's next kick:
- 60 Hz needs latch → kick under about 15 ms. The median is 20-22 ms without a client, so most
  frames take 2 ticks: w2 has 174 one-tick and 360 two-tick frames, and 104 of 3 or more.
- Every kick is preceded by a burst of trapped accesses, ending 0.7 ms before the kick:
  - about 23 flush reads and 2 invalidates;
  - two runs of 11-12 flushes, each closed by an invalidate (flags 0xc1, pdb 0x1e5552000, "no
    mirror or shadow VA space rooted there");
  - this is two guest GPU VA changes (map and unmap) in the compositor's commit path. The RM's
    page-table walker flushes after every batch of BAR2 page-table writes.
- **The run takes 7-13 ms per frame (p50)**. Flush reads come back to back:
  - spacing p10 0.21 ms, p50 0.3-0.5 ms, against 1-2 us on hardware;
  - each trapped flush read runs a full BAR2 memfd sync: a compare of every mapped memfd page,
    about 245 pages, plus the BAR1 pool's replica merges (VRAM page reads through BAR1);
  - session totals: 377327 syncs, 65.6 s, mean 174 us; 385k replica merges;
  - the p10 spacing is that sync plus the trap round trip. The rest is waiting for the device
    lock.
- **Without a client the run is the largest piece.** 6.6 ms (render dispatch, GPU work) +
  12.8 ms (run) + 0.8 ms = 21.6 ms.
  - Cross-check: `c1-baseline` w2, after Sunshine hung, did no GL rendering. The same run was
    2.6 ms, with spacing 0.21 ms and less lock contention, and the guest kicked 56.9/s at 1
    tick per frame.
- **While streaming the run is 20-25 % of latch → kick.**
  - Sunshine's per-frame chain sits in front of it: `d43` EGL import, CUDA `d11`/`d12`/`d14`,
    NVENC notifier `0x8000026`, and its own flush runs and invalidates for the per-frame map
    and unmap (sottr-stage.md §11). Together that is 25-48 ms (p50).
  - The chain and the compositor share the guest RM lock and the GPU.

## 2. Cause

- **In the device: the trapped BAR2 flush read is 100-300x too expensive.**
  - `boot.rs` `bar_access` ran `bar2_sync` (the whole-memfd compare) and
    `bar1pool::sync_untracked` on every flush read.
  - The guest RM reads the flush page about 23 times per frame in the compositor's commit path,
    and more for Sunshine's capture.
  - At 0.2-0.5 ms per read, that alone uses most of the 16.7 ms budget, so the commit misses
    the next tick.
- Not the display model:
  - the tick period is exact;
  - the latch takes the first tick;
  - the vblank and AWAKEN MSI go out with the latch.
- The device lock holds that `vm4-615-invhold` excluded were the MMU-invalidate *holds*. The
  flush-read syncs are a different, per-frame cost, and they are what the trace shows.

## 3. The change (device)

- **A flush read defers its sync** (`bar2area.rs` `DeferredFlush`, `CARAXES_BAR2_MEMFD_DEFER_FLUSH`,
  default on; `=0` restores the sync on every flush read).
  - The trapped read only marks the sync owed and returns.
  - The first hand-over to the GPU settles the owed sync: `bar2_sync` plus `sync_untracked`,
    `BootModel::flush_point`.
    - The MMU invalidate and the GSP command-queue doorbell sync anyway.
    - A channel doorbell (trapped, ioeventfd or token path) now syncs when a flush is owed.
  - The **flush clock** thread (`bar2-flush`, main.rs) settles an owed flush that no
    hand-over settled within `DEFER_BOUND` = 1 ms. It covers data a running engine polls
    without a doorbell.
  - The device's own reads are unaffected: they read the memfd (`read_through`).
- **Why this is safe.** It is bar2-memfd.md §3's own rules. The GPU uses what the flush made
  visible only after the invalidate (page tables), the RPC doorbell (RPC data) or the channel
  doorbell (channel data). Each of those now syncs first.
  - Coalescing makes one sync cover several flush epochs. Within one sync, pages are written
    back in page order. That was already so within one epoch, so there is no new ordering class.
- **Expected per frame:**
  - about 23 trapped reads at trap cost (tens of us each) plus the 2 invalidate syncs, so
    about 1.5-3 ms instead of 7-13 ms;
  - latch → kick about 9-10 ms without a client, under one period;
  - fewer syncs also mean less device-lock time for the display clock and the reactor.
- **Counters** in the `bar2 memfd:` exit line: `flush reads deferred: N deferred, settled A at
  a sync point, B at a doorbell, C by the clock`.
- **Tests:**
  - `boot::tests::a_flush_read_defers_its_sync_to_the_next_hand_over`: the flush read does not
    write VRAM, device reads see the memfd, and the ioeventfd doorbell, the clock and the MMU
    invalidate each settle it;
  - `boot::tests::without_the_deferral_every_flush_read_syncs`;
  - `bar2area::tests::the_flush_clock_settles_an_owed_flush_after_the_bound`.

## 4. What stays on the guest side (streaming)

- Sunshine's KMS capture imports the scanout buffer every frame:
  - `drmModeGetFB2`, prime FD, EGL import, blit, CUDA and NVENC;
  - in the guest RM that is a map and an unmap per frame: 7-25 more flush reads, 2 invalidates
    and GPU waits.
- The change makes Sunshine's flush reads cheap too. What remains is its serial per-frame
  import and its GPU work.
- Sunshine-side change: the import cache of sottr-stage.md §11 (keep the imported
  `egl::rgb_t` per `fb_id`/modifier/pitch/offset, at most 4 entries, cleared on `reinit`). It
  removes the per-frame map and unmap.

## 5. What a GPU run must show

With the frozen binary `vfio-user-nvgpu-v615-flippace-<sha>`, vm4-615, and the invhold
matrix windows (`vm4-615-invhold/tools/`):
- **Cursor test, no client:** kicks/s approach 60 (at least 55). In `display-pace.py`:
  - ticks between latches mostly 1;
  - latch → kick p50 under 12 ms;
  - pre-kick run p50 under 3 ms, flush-read spacing p50 under 0.1 ms.
- **Cursor test while streaming:** kicks/s above 45, Sunshine latency average under 30 ms.
  - If it stays lower, the remaining time is the guest capture chain (section 4). The window's
    `latch -> pre-kick run` then stays long while the run itself is short.
- **Exit line:**
  - `flush reads deferred: ...`, with `settled ... by the clock` a small share;
  - `sync(s)` per second of session far below the 317/s of A-invhold.
- **No regressions:**
  - host Xid 0 and no guest `TLB invalidation failed`;
  - vkcube and glmark2 complete while streaming (the invhold cycle test).
- **A/B:** `CARAXES_BAR2_MEMFD_DEFER_FLUSH=0` in the same binary must reproduce the
  A-invhold numbers.

## 6. What the flush-read sync does, and when it runs with the deferral

Written for the deferral's work regression (`vm4-615-flippace/NOTES.md`). That regression was
traced to a doorbell token collision (`ctrl.rs` `doorbell`, `vm4-615-cuda-vulkan/NOTES.md`),
not to a sync path. The list stays as the deferral's contract.

A flush point (`BootModel::flush_point`) does the following:
- **Guest to VRAM** (`Bar2Area::sync`, the scan): writes back the bytes the guest changed
  through BAR2 to the page's home, which is its BAR1 pool page when pooled. The GPU and the
  guest's BAR1 view then see them.
- **Alias to alias** (the scan): the merged qwords are copied to the page's other BAR2 pages.
  A read through another BAR2 page of the same FB page sees a write only after this.
- **VRAM to guest, entering pages** (`follow_tables`, when a BAR2 table page changed): a page
  newly mapped into BAR2 is loaded from VRAM (or guest RAM) into its memfd page. A page that
  leaves is dropped, after the scan wrote it back.
- **Replica merges, both ways** (`bar1pool::sync_untracked`): area to home for guest BAR1
  writes, home to area for GPU and device writes. The area is the second BAR1 mapping of a
  page the guest maps twice.
- **Device caches**: the dirty pages are marked in the shadow walks, the BAR1 pool's walk, the
  BAR TLB and the watched dwords (`note_pages_changed`, `bar1_pt_resync`).

When each runs:

| Mode | At the flush read | Otherwise |
|---|---|---|
| `DEFER_FLUSH=0` (before 02c0a625) | all of the above | MMU invalidate, GSP command-queue doorbell, BAR2 bind (scan and follow only); replica merges also at the pool polls after BAR0 writes |
| deferral (02c0a625) | nothing; the sync is marked owed | all of the above at the MMU invalidate, the GSP command-queue doorbell, a channel doorbell while a sync is owed (trapped, ioeventfd or token), or the flush clock 1 ms after the first owed flush read; replica merges also at the pool polls |

What the deferral changes beyond the GPU hand-overs:
- **Longer lost-update window.** Guest BAR2 bytes reach VRAM up to 1 ms (plus the device-lock
  wait) after the flush instead of before it returns. A write by another agent to the same
  bytes in that window (the guest's BAR1 view, or the GPU) can be overwritten by the later
  write-back. That needs a page the RM writes through BAR2 and another agent writes too, for
  example a semaphore or notifier page. Page tables are written only by the RM.
- **Stale alias reads.** A read through a second BAR2 alias of a page the RM just wrote returns
  the old bytes until the settle. Up to 22 aliases of one page were measured.
- **Fewer replica merges.** About 84 per second instead of about 430 (A2-defer against
  B-sync). This only delays when the guest sees GPU writes to a replicated page.
- **Unchanged:**
  - Entering pages: the RM always flushes and invalidates after a BAR2 PTE change
    (`kbusUpdateRmAperture_GM107`, kern_bus_gm107.c:2787-2815), and the invalidate settles.
  - Device reads, which read the memfd.
- **Tighter variant, if the next A/B shows a problem in these classes:** defer only page-table
  pages at the flush read.
  - Page-table pages are those the shadow walks watch, plus the tables of guest roots the RM
    invalidates often, learned by a capped walk. Each must have a single BAR2 alias and must
    not be pooled or a replica home.
  - Everything else, and the replica merges, stays synchronous at the flush read.
  - A clean scan of 245 live pages measured 45 us (CPU only).

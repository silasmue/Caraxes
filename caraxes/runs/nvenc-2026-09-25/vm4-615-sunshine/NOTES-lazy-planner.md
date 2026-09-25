<!-- SPDX-License-Identifier: MIT -->
# Note for the launch agent: the lazy planner misses its deadline at game start

Reported by the coordinator; the numbers are mine, from its evidence. Not fixed here.

- Evidence: `/mnt/vmstore/caraxes/evidence-2026-09-25/game-hang-trace-tail.txt`, vm4-615 at
  game start. The trace covers 682.4 to 919.0 s; the device log is `game-hang-gpu.log`.
- `alloc lazy: deadline passed, planning N deferred invalidate(s) now` (mirror.rs:793,
  lazy.rs) appears 2237 times, for 7604 invalidates in total, at most 14 per batch.
- The rate is 10 to 30 per second over long stretches (689-701 s, 755-760 s, 769-808 s).
  The coordinator counted about 500 per 30 s, with held doorbells.
- The default deadline is `CARAXES_ALLOC_LAZY_MS` = 20 ms (lazy.rs:38). At game start the
  guest's invalidates come faster than doorbells, so nearly every invalidate after the
  first waits out the deadline and then plans its batch synchronously. The design meant the
  deadline as a rare bound; here it is the normal path.
- In the same window, the BAR1 pool's poll after a BAR0 write held the device 2 to 4 ms
  each time (`bar1 pool: the poll after a BAR0 write held the device ...`,
  sottr-stage.md §11). It competes with the planner for the device lock.
- Questions to settle:
  - Is the deadline worth keeping during bursts, or should a pending batch be planned by
    the worker without holding the doorbell? Or should the deadline scale with the batch?
  - Does a held doorbell wait for the whole deferred batch, or only for the channel's own
    root?
- Where to look: mirror.rs `flush_lazy` and the `OnInvalidate` branch in
  `shadow_invalidated` (around mirror.rs:780-800); lazy.rs `Lazy::on_invalidate`.

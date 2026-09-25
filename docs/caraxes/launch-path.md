<!-- SPDX-License-Identifier: MIT -->
# The launch path: where a short kernel's microseconds go

Written 2026-09-25 for the operator's direction of the morning: optimise the short kernel. The
morning matrix had it at 15 us per launch against 6 us native (2.5x), rlprobe stage 4 at 1.7x
and PPO at 0.67x ([benchmark.md](benchmark.md) "Morning matrix"). This document measures the
chain of one launch on the device side, names what changed, gives the numbers and the switches.
Code: `crates/vfio-user-nvgpu/src/launch.rs` (new), `poll.rs`, `channel.rs`, `ctrl.rs`,
`device.rs`, `fb.rs`, `mirror.rs` (the ring and the event loop only), `intr_page.rs`, `trace.rs`.
Records: `caraxes/runs/opt-launch-2026-09-25/`.

## Summary

1. **The device's share of a ring was 6.4 to 9.8 us at the median, and three uncached BAR1
   reads, a BAR1 flush and the notes made it.** One uncached read of the pool's USERD page
   through the device's BAR1 window costs 0.7 to 1.2 us on this host; a ring made five of them
   (`note_rung`, twice two for GPGet and GPPut) plus a flush read per window.
2. **`CARAXES_LAUNCH_FAST=1` halves it** without changing a ring's order: ring to host doorbell
   p50 6.4 to 3.0 us (launch loop), 9.8 to 3.7 us (rlprobe), 8.0 to 3.4 us (PPO); the work after
   the host doorbell 4.2 to 1.9 us.
3. **1000 back-to-back launches (`bench-cu launch`): interval p50 18.9 to 13.4 us, the
   synchronize wait 14.1 to 9.3 us**, three interleaved reps each on a loaded host. The launch
   call itself (4.0 to 5.5 us, the guest's pushbuffer and GPPut stores and the doorbell exit) is
   unchanged.
4. **What remains is outside the device's ring**: the guest's launch call (about 4.5 us) and,
   in the synchronize wait, about 6 us from the host doorbell to the guest seeing the semaphore
   (the physical GPU's fetch and run at P5/P8 clocks, and the host scheduling the mirror's TSG
   among the other clients' work). The completion interrupt is not on the path: libcuda's auto
   (spin) wait never asks for one (158 host events within 1 ms of a host doorbell against
   9106 rings).
5. **rlprobe and PPO did not move measurably** under this morning's host load (load average
   5 to 27, three other VMs with GR work); see §5. The interrupt gate (`CARAXES_INTR_GATE_MS`,
   backlog item 5) holds foreign interrupts while the VM is idle; the adaptive hybrid window
   (`CARAXES_DOORBELL_POLL_ADAPT_US`) did not pay off.

## 1. Method

`CARAXES_LAUNCH_TIMING=1` (launch.rs) times each step of a ring into a fixed histogram
(100 ns buckets to 100 us, then 10 us to 10 ms) and prints one `launch timing:` line per step in
the exit summary. Off, every hook is one relaxed load. The steps:

| Step | From, to | Where |
|---|---|---|
| lock | `epoll_wait` returned, Device mutex held | device.rs `spawn_token_reader_hooked` |
| dispatch | mutex held, the shadow doorbell entered (token to channel, BAR2 memfd check, `note_rung`, notes) | mirror.rs:815 |
| ran-ahead check | `note_ran_ahead`: GPGet and GPPut of USERD | mirror.rs:819 |
| plan | lazy flush and shadow re-sync planning | mirror.rs:830 |
| worker wait | `wait_all`: the ordering barrier | mirror.rs:834 |
| BAR1 flush | `FakeFb::flush` | mirror.rs:836 |
| USERD read | GPPut for the forward record | mirror.rs:842 |
| host doorbell write | the store to the host's doorbell page | mirror.rs:1935 |
| tail | host doorbell to the end of the ring (error scan, notes, trace) | launch.rs `end` |
| ring to host doorbell (sum) | `epoll_wait` returned to the host doorbell | launch.rs `host_doorbell` |
| host doorbell to host event | last host doorbell to a host non-stall event in the event thread | mirror.rs `event_loop` |
| host event to raise | the event to `IntrShared::raise` done | mirror.rs `event_loop` |
| raise to MSI | the raise to the MSI eventfd write by the interrupt page's reactor | intr_page.rs:686 |

The reactor also counts the rings whose `epoll_wait` had blocked (the scheduler wake-up lies
before `seen` and is not in the histograms) and the host events by the age of the last host
doorbell (item 5's population).

The guest side: `bench-cu launch 1000` (caraxes/tools/bench-cu.c, metric `launch1k` of
`bench-run.sh`) launches the 1000-cycle spin kernel 1000 times, each followed by
`cuCtxSynchronize` (nvprobe 4's loop), and reports percentiles of the launch call, the
synchronize wait and the interval. It was built on the host against cuda.h 13.4 (glibc symbols up
to 2.38) and staged as `bench-cu-launch` beside the guest-built `bench-cu`, so the P13 metrics
keep their binary.

Setup (`env.sh`): vm5, 610 guest, 1 GiB at slot 2, the host-execution environment and the
recommended windows (interrupt page, token pool 64, hybrid reactor 200 us, BAR1 pool 32 MiB,
BAR2 memfd, MSI re-arm ioeventfd, lazy plans). A is the frozen before-binary
(`vfio-user-nvgpu-launch-before-7095b3c`, the recommended configuration plus timing); B is
`vfio-user-nvgpu-launch-after-25c164b` with `CARAXES_LAUNCH_FAST=1
CARAXES_DOORBELL_POLL_ADAPT_US=2000 CARAXES_INTR_GATE_MS=50`. The host was busy all morning (other
agents' vm0, vm1, vm2, vm3 and vm7, load average 5 to 27), so `ab.sh` interleaves A and B per
rep, each in fresh VM lives: `launch` (short-auto, launch1k, copy), `rl`, and `ppo` (60 s).
`caraxes/tools/launch-chain.py RUN` makes the tables below from the kept device logs.

## 2. The chain, before and after

Medians over three reps of each step's p50 (`ab/chain.md`; `launch` lives, which the launch loops
dominate, and `rl` lives):

| Step, p50 us | A launch | B launch | A rl | B rl | A ppo | B ppo |
|---|---|---|---|---|---|---|
| lock | 0.6 | 0.6 | 0.5 | 0.6 | 0.6 | 0.6 |
| dispatch | 1.2 | **0.1** | 1.6 | **0.1** | 1.5 | **0.2** |
| ran-ahead check | 1.5 | **0.9** | 2.5 | **1.4** | 2.0 | **1.1** |
| plan | 0.2 | 0.2 | 0.2 | 0.2 | 0.2 | 0.2 |
| worker wait | 0.1 | 0.1 | 0.1 | 0.1 | 0.1 | 0.1 |
| BAR1 flush | 1.4 | **0.1** | 2.3 | **0.1** | 1.5 | **0.1** |
| USERD read | 1.5 | **0.8** | 2.5 | **1.2** | 2.0 | **1.1** |
| host doorbell write | 0.1 | 0.1 | 0.1 | 0.1 | 0.1 | 0.2 |
| **ring to host doorbell** | **6.4** | **3.0** | **9.8** | **3.7** | **8.0** | **3.4** |
| tail after the host doorbell | 4.2 | **1.9** | 4.2 | **1.9** | 4.8 | **2.0** |
| rings after a blocking wait | 807 of 9106 | 269 of 8931 | 1232 of 77280 | 662 of 82420 | 33833 of 844935 | 14415 of 875695 |

The before series (`before/chain.md`, per-metric lives, first half of the morning) has the same
shape with the reads slower (ring to host doorbell 8.9 us in short-auto, 9.4 us in rlprobe).

The launch loop as the guest sees it (`bench-cu launch 1000`, p50 per rep):

| | A rep 1 / 2 / 3 | B rep 1 / 2 / 3 | A median | B median |
|---|---|---|---|---|
| interval | 19.45 / 18.94 / 18.20 | 12.33 / 15.05 / 13.37 | 18.94 | **13.37** |
| launch call | 5.49 / 4.23 / 4.02 | 4.57 / 4.91 / 3.94 | 4.23 | 4.57 |
| synchronize wait | 13.74 / 14.64 / 14.10 | 7.66 / 10.01 / 9.33 | 14.10 | **9.33** |
| interval p99 | 79 / 43 / 339 | 54 / 62 / 188 | | |

Reading the chain of one launch (B, p50):
- **Launch call, 4.6 us (guest):** libcuda writes the pushbuffer (guest RAM), the GPFIFO entry
  and GPPut (the pool, write-combined BAR1) and rings the pool token; the doorbell write exits to
  KVM, which signals the ioeventfd (1.2 to 1.4 us, doorbell-token-pool.md (b)). Not changed here.
- **Eventfd to the reactor, under 0.2 us:** at a 13 us rhythm the hybrid reactor is inside its
  200 us window, one `epoll_wait(0)` per 170 ns (doorbell-poll.md).
- **The device's ring, 3.0 us** (was 6.4): one uncached qword read for the ran-ahead check, the
  plan and barrier (0.3 us), the flush skipped, one uncached read of GPPut, the host doorbell.
- **Host doorbell to the guest seeing the semaphore, about 6 us:** the synchronize wait minus the
  ring. It holds the PBDMA's fetch of the GPFIFO entry and the pushbuffer, the 1000-cycle kernel
  (1.0 to 2.4 us at the 420 to 960 MHz graphics clocks the `# gpu` lines of the records show at
  the start of each run, P8 or P5), the
  semaphore release to guest RAM and libcuda's spin seeing it; and, with other VMs on the card,
  the host's time-slicing of the mirror's TSG. Native pays the fetch, the kernel and the release
  too; its 6 us per launch (closing series) is launch call plus wait.
- The synchronize wait fell by 4.8 us while the ring fell by 3.4 us; the rest lies within the
  reps' spread (7.7 to 10.0 us in B) or in the shorter tail, which holds the Device lock and the
  reactor's core.

The completion side (`host event` steps): the host's non-stall notifier reaches the event thread,
which raises the vector in 2 to 4 us; the interrupt page's reactor writes the MSI 11 to 15 us
after the raise at the median (a blocked reactor's wake-up; p90 in the hundreds of us when the
vector is still pending in the page and the MSI waits for the guest's ack, intr-page.md §3). In
the launch lives only 145 to 158 host events came within 1 ms of a host doorbell, against about
9000 rings: the launches ask for no completion interrupt, and 80 % of the events are other
clients' work on the engine (backlog item 5).

### 2a. The per-submission chain of rlprobe and PPO

The step "host doorbell to the next ring" (`ab/chain.md`) is the device's view of the guest's
rhythm: from one host doorbell to the next guest doorbell of any channel.

| p50 us | A rl | B rl | A ppo | B ppo |
|---|---|---|---|---|
| ring to host doorbell | 9.8 | 3.7 | 8.0 | 3.4 |
| tail after the host doorbell | 4.2 | 1.9 | 4.8 | 2.0 |
| host doorbell to the next ring | 7.5 | 13.5 | 11.8 | 17.9 |
| one ring cycle (sum) | 17.3 | 17.2 | 19.6 | 21.3 |
| rings per life | 77280 | 82420 | 844935 | 875695 |

- rlprobe submits about 50 rings per environment step (82420 rings for 1437 H2D and 1437 D2H of
  stage 4 plus stages 0 to 3), one every 17 us inside a step, in A and in B alike. In A the
  device was busy 14 of those 17 us (ring plus tail) and the next doorbell waited in its eventfd;
  in B it is busy 5.6 us and then waits 13.5 us for the guest. The cycle is set by the guest
  (PyTorch's dispatch, libcuda's pushbuffer and GPPut stores, the doorbell exit), not by the
  device, which is why stage 4 did not move.
- PPO is the same at a coarser grain (14000 rings per second, a 20 us cycle).
- Per step, the chain around the rings is the H2D of the observation (a pageable copy: libcuda's
  staging buffer and a copy-engine submission), the forward kernels, the sample, and the D2H of
  `.item()` with its synchronize, which waits for the GPU's completion of everything before it.
  `caraxes/tools/stepchain.py` (metric `stepchain`) times these phases one by one in the guest;
  §7 has its numbers.

## 3. What changed (`CARAXES_LAUNCH_FAST=1`)

Each cut keeps a ring's order: plans applied (`wait_all`), BAR1 flush, GPPut read, host
doorbell.

| Cut | Code | Saves (p50) | Ordering argument |
|---|---|---|---|
| GPGet and GPPut of USERD as one aligned qword (0x88, 0x8C) for the ran-ahead check | mirror.rs:655 `userd_ring`, launch.rs:529 `userd_pair` | 0.6 to 1.1 us | the same two dwords, now from one bus read, so also a consistent pair |
| GPPut alone for the forward record | mirror.rs:676 `userd_put` | 0.7 to 1.3 us | the same read, after the flush, as before |
| No `note_rung` read under the token pool | channel.rs:398 | about 1 us of dispatch | `gp_put_seen` is read only by `Registry::pending`, the token-less reader's, which the pool excludes (main.rs:358) |
| BAR1 flush only after a device store | fb.rs:632 `VramStore::flush`, launch.rs:372 `flush_needed` | 1.3 to 2.2 us | the flush drains the device's own write-combine buffers; skipped only when the device stored nothing through a window since the last flush (the store sets the flag, fb.rs `VramStore::write`). The guest's pool stores never were in the device's buffers; the GPPut read that follows is a read of the same VRAM through the same BAR |
| The ring's notes: the pool note formatted after the host doorbell, the redundant `doorbell ioeventfd` note dropped, one trace flush per ring | ctrl.rs:253, device.rs:588, trace.rs `events` | dispatch 1.1 us, tail 2.3 us | notes only; the census's two counted notes (`doorbell token ... (eventfd)`, `doorbell: guest chid`) stay |

Flushes in B: 455 to 1636 done, 78426 to 1473543 skipped per life (the done ones are the
device's own stores: pool moves, memfd syncs, token hand-outs).

Tests (`cargo test -p vfio-user-nvgpu`, both variants): `fb::fast_flush_never_skips_a_device_store`
(a store is always drained before the next doorbell; only an empty flush is skipped; switch off,
every flush runs), `launch::flush_gate_skips_only_an_empty_flush`,
`launch::userd_pair_splits_the_qword`, `channel::pending_follows_gpput_on_both_paths` (the read is
skipped only with the pool on), `launch::a_ring_records_its_steps_in_order`. The switch is read
per thread in tests (`set_fast_here`), so no test leaks it into another.

### 3a. The lean evidence (`CARAXES_LAUNCH_FAST=2`)

After the cuts of §3 the ran-ahead check is still 0.9 to 1.4 us of the ring (one uncached qword
read of USERD). It is evidence only: it counts doorbells whose host channel already fetched
entries past the last forward, and it logs the ones with plans pending (untrapped.rs `HoldStats`);
nothing in the ring depends on it. `2` skips its read when no plan is pending (mirror.rs:695,
launch.rs:346 `evidence_needed`): the logged lines and the pending count stay exact, only the
total count stops covering doorbells without plans. The forward record (GPPut after the barrier
and the flush) is still read at every ring, so the next pending doorbell compares against a fresh
value. Test: `launch::lean_evidence_reads_whenever_plans_are_pending`.

### 3b. The direct completion (`CARAXES_INTR_DIRECT=1`)

§2 measured the raise-to-MSI step at 11 to 18 us p50 (83 us in B's PPO lives): the event thread
set the pending bit, wrote the interrupt page reactor's wake eventfd, and the reactor, often
blocked after its hybrid window, woke, scanned the page and wrote the MSI. With the switch the
raise runs that same scan on the event thread (intr.rs:94, `IntrShared::attach_direct`): the
vector is folded into the page, the page stores are fenced, then the MSI eventfd is written, as
the reactor does it (intr_page.rs `scan_racing`). The page's mutex orders the two scanners;
whichever takes the raised flag writes the page and the MSI, and the other finds nothing.
Rules and tests:
- nothing is reported before the host reported it: the direct scan runs only inside a raise,
  and a scan without a raise writes no MSI;
- the page shows the vector before the MSI is written, and a raise while the vector is still
  pending costs no second MSI (`intr_page::direct_completion_orders_the_page_before_the_msi`);
- racing the reactor's scan, every raise ends with exactly one MSI and the vector in the page
  (`intr_page::direct_completion_racing_the_reactor_loses_nothing`, 2000 rounds);
- refused without `CARAXES_INTR_PAGE=1` (intr_page.rs:129 `direct_requested`); a dropped page
  falls back to the wake.

This is also the MSI coalescing the brief asked for: while the guest has not acked a vector, the
page already drops the second MSI (intr-page.md §3), so a guest that is polling its semaphore
and has not taken the first interrupt gets no second one.

## 4. The interrupt gate (`CARAXES_INTR_GATE_MS`, backlog item 5)

The host's non-stall notifiers are engine-wide: the desktop's and the other VMs' GR work raise
this guest's vector too. The gate (launch.rs `Gate`, mirror.rs `event_loop`) holds an event that
comes while this device wrote no host doorbell for ten holds, and delivers every held vector
together once the hold has passed since the first, or at once with the next event inside the hot
window. Rules, each in `launch::gate_delays_foreign_events_and_never_loses_one`:
- nothing is delivered before the host reported it (the gate only delays);
- nothing held is lost: every held vector is delivered at most one hold late (the event thread's
  poll timeout is the hold's remainder);
- a vector outside the 64-bit mask is never held; RC events are never gated.

This is "forward only on progress" in its safe form: the device cannot see a completion (the
semaphore is guest memory it does not watch), so it cannot drop an event, only delay the ones
that come while it has no recent work of its own. A kernel longer than ten holds that the guest
waits on with a blocking synchronize gets its completion up to one hold late. With a 50 ms hold:
181 to 1058 events held per launch life, 573 to 2187 per rlprobe life, delivered in 23 to 268
releases; MSIs for raises per launch life 774 to 1547 (A) against 620 to 757 (B). The idle rate
(55 per second, trap census) was not measured separately.

## 5. What did not pay, and the loaded host

- **The adaptive hybrid window** (`CARAXES_DOORBELL_POLL_ADAPT_US`, poll.rs `Wait`): after an
  event the reactor spins for 1.5 times the longest of the last 8 gaps below the cap. rlprobe's
  blocking wake-ups were 1.6 % of its rings in A already (1232 of 77280) and fell to 0.8 %; PPO's
  from 4.0 % to 1.6 %; the reactor's CPU rose with it (PPO device CPU 27 to 37 s in A, 32 to 49 s
  in B). No guest metric improved beyond the noise; `ab-rl-noadapt/` repeats rlprobe with the
  fast path and the gate but without the window; it was stopped at the first agent's cut-off
  after one A life and is not used (`ab-rl-noadapt/stopped.txt`). Keep it off.
- **rlprobe stage 4:** A 2.4 s [2.2-3.0], B 2.7 s [2.6-3.0]; transfers per second 1192 against
  1052. The ring is 6 us shorter, but a step of rlprobe waits on its H2D and D2H copies, whose
  time the host's GR and copy engine sharing sets this morning (vm3's device used 5 to 30 s of CPU
  in the same minutes).
- **PPO (60 s):** A 16223 and 34093, B 8800 and 42747 steps per second, at load averages of 11
  to 27; not a comparison. Its rings per second (14000) make the per-ring cut worth about 5 %
  of a core.
- **Copy:** unchanged within the known bimodality (h2d pinned 1 MiB 4.7 to 10.9 GB/s in A,
  6.0 to 6.1 in B; the link state, benchmark.md "Morning matrix").
- **P24, polling GPPut in memory instead of the eventfd:** with the pool, GPPut lives in VRAM
  behind BAR1, and each poll would be an uncached read of 0.7 to 1.2 us per channel per
  iteration, against 170 ns for one `epoll_wait(0)` over all 64 tokens. Not built; it needs a
  USERD in guest RAM (doorbell-poll.md "Next step").
- **Batching host rings:** a wakeup that carries several tokens rings each once; two advances of
  one channel within one wakeup were not seen often enough to matter (the eventfd counts them as
  one event). §2a shows why a window would not help either: in B the device waits 13 to 18 us
  per ring for the guest, so a second advance of the same channel inside one ring is rare, and
  holding the first to wait for it would add its wait to every launch.
- **The tail after the host doorbell (1.9 us in B), not built:** it is the pool note's
  formatting in ctrl.rs and the trace write, one `write` of the buffered notes per ring
  (trace.rs `events`). A 91-byte `write` to a file on /mnt/vmstore costs 1.36 us p50 on this host
  (1.11 us on /tmp; measured from Python, so an upper bound). The tail does not delay the launch
  whose doorbell it follows; it delays only a ring that arrives during it, and §2a shows the next
  ring arriving 13 to 18 us later. Deferring the flush would move the write off the reactor at
  the price of trace lines that lag behind the guest; not worth it at these rhythms.

## 6. Switches

| Variable | Default | Meaning |
|---|---|---|
| `CARAXES_LAUNCH_TIMING` | off | `1`: the per-step histograms of §1 in the exit summary (`launch timing:` lines) |
| `CARAXES_LAUNCH_FAST` | off | `1`: the cuts of §3; the exit summary adds `launch fast: BAR1 flushes N done, M skipped`. Recommended on |
| `CARAXES_INTR_GATE_MS` | off | a hold of 1 to 1000 ms: the gate of §4, hot for ten holds after a host doorbell; the device log gets `intr gate:` at the end |
| `CARAXES_LAUNCH_FAST=2` | off | `1` plus the lean evidence of §3a; the `launch fast:` line adds the skipped ran-ahead reads |
| `CARAXES_INTR_DIRECT` | off | `1` (needs `CARAXES_INTR_PAGE=1`): the direct completion of §3b; the `intr page` summary adds `intr page direct: N raise(s)` |
| `CARAXES_DOORBELL_POLL_ADAPT_US` | off | hybrid only, at least the idle time: the adaptive spin window's cap (§5); `doorbell poll` shows the last window |

Records: `before/` (the first series, per metric, stopped after rlprobe's second rep when the load
made a serial comparison useless), `ab/` (the interleaved A/B), `ab-rl-noadapt/`,
`gate-after/` (the cuda gate with B's switches), each with `setup.txt`, `hostwatch.txt` and
`host-xid.txt`. Scripts: `env.sh`, `bench.sh`, `ab.sh`, `gate.sh`, `stop.sh`.

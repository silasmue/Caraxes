<!-- SPDX-License-Identifier: MIT -->
# Doorbell reactor: polling the doorbell eventfds

Written 2026-09-24 for the operator's idea: "a polling mode that uses reactor polling like SPDK
instead of eventfd wake-ups, set by an option at boot; later wake-up polling; so we push latency
further without changing much code". It builds on the datamatch token pool of
[doorbell-token-pool.md](doorbell-token-pool.md) and the zero-trap measurement in
[benchmark.md](benchmark.md) (short kernel 0.018 to 0.024 ms per launch, rlprobe stage 4 3.0 s).
Code: `crates/vfio-user-nvgpu/src/poll.rs`. Runs: `caraxes/runs/doorbell-poll-2026-09-24/`.

## Design

In five sentences:
1. The thread that reads the doorbell eventfds can block in `epoll_wait` (`off`), poll it with
   timeout 0 in a loop (`spin`), or poll for a while after each event and then block (`hybrid`).
   `CARAXES_DOORBELL_POLL` selects the mode at device start.
2. A polling reader pins itself to one core: the one `CARAXES_DOORBELL_POLL_CPU` names, or one
   it picks itself (not CPU 0, not a vCPU thread's, the highest-numbered idle core).
3. The loop allocates and logs nothing and takes the Device mutex only when an eventfd fired,
   so the lock discipline and the dispatch path are unchanged.
4. The exit summary reports iterations, events, the time from ring to host doorbell write
   (mean, p50, p99), the mutex wait and the reader's CPU time.
5. The token-less ioeventfd reader goes through the same reactor, so the option covers both.

In detail:

1. One thread reads the doorbell eventfds over one epoll set (the pool's 64, or the token-less
   one). `CARAXES_DOORBELL_POLL` sets how it waits: `off` blocks in `epoll_wait` as before,
   `spin` calls `epoll_wait` with timeout 0 in a loop (one syscall per iteration over all
   eventfds, non-blocking reads of the ready ones), and `hybrid` spins for
   `CARAXES_DOORBELL_POLL_IDLE_US` (default 200) after the last event, then blocks until the
   next one and spins again (`poll.rs` `Wait`, a two-state machine).
2. A spinning reader pins itself with `sched_setaffinity`. It takes the core from
   `CARAXES_DOORBELL_POLL_CPU=<n>`, or with `auto` (the default) it picks one itself: not CPU 0,
   not a core a vCPU thread of the VMM serving its socket runs on at that moment, and the
   highest-numbered core that was at least 90 % idle in a 1 s `/proc/stat` sample. It logs the
   choice. `none` leaves it unpinned, which is only useful for comparison. A poll mode refuses
   to start when no core can be chosen, when the mode would ignore a setting it was given, or
   when there is no doorbell eventfd to poll.
3. The loop allocates nothing and logs nothing. The event and ring buffers are sized at start,
   the counters are atomics that only the reader writes, and the two latency histograms (100 ns
   buckets up to 200 us) are fixed arrays. The Device mutex is taken once per wakeup with
   events, as before, so an idle spinning reader never contends with the vfio-user server
   thread.
4. The exit summary line `doorbell poll (tokens): ...` reports:
   - loop iterations, those with events, and blocking waits;
   - eventfd events and the guest writes they counted;
   - the time from `epoll_wait` returning to the host doorbell write (mean, p50, p99, max),
     measured with the trace's clock (`Instant`, CLOCK_MONOTONIC);
   - the wait for the Device mutex per wakeup;
   - the reader's CPU time, from `/proc/self/task/<tid>/schedstat`.

   The end point of that time is stamped by the dispatcher when the mirror's doorbell counter
   moves (`ctrl.rs` `host_doorbell_at`). `mirror.rs` is unchanged.
5. The token-less ioeventfd reader (`CARAXES_DOORBELL_IOEVENTFD=1`) used to be a separate
   thread with a blocking `read`. It now goes through the same reactor with its one eventfd,
   so the option covers it too. It was not measured on the GPU, because the device refuses it
   together with P21.

What the in-process time does not see: the wake-up itself. KVM signals the eventfd from the
vCPU's exit, and in `off` mode the scheduler then has to run the reader. That delay happens
before `epoll_wait` returns, so it only shows in the guest-side numbers (short kernel, rlprobe)
and in the number of loop iterations per event.

## Measurement (vm1, 2026-09-24)

Frozen device `/mnt/vmstore/caraxes/vm1/vfio-user-nvgpu-poll` (commit b6b52a4, sha256
`dfcaddab...`), Cloud Hypervisor v53.0 with patches 0001-0006, `CARAXES_BAR1_PASSTHROUGH=1`,
`CARAXES_DOORBELL_TOKENS=64`, vm1 with the 1 GiB profile at slot 4 (the environment of
[benchmark.md](benchmark.md), "Zero-trap submission"). The metrics were short-auto and rl, 3 reps
each. Script and logs: `caraxes/runs/doorbell-poll-2026-09-24/run.sh`.

The four configurations:
- C: `off`, the blocking reader;
- D0: `spin`, unpinned;
- D: `spin`, pinned automatically;
- E: `hybrid`, 200 us idle, pinned automatically.

The series ran twice:
- pass 1 (13:44-13:58) in the order C, D0, D, E;
- pass 2 (13:59-14:12) in reverse order.

vm0 (the display agent) was up the whole time. In pass 1 its device was active (above 20 % of a
CPU) during every run of C and D0 and during none of D and E, so pass 1 favours D and E. In pass
2 it was idle during all runs. Pass 2 is therefore the clean comparison.

**Pass 2 (reverse order, no other device active):**

| Value | C: off | D0: spin, unpinned | D: spin, pinned | E: hybrid 200 us, pinned |
|---|---|---|---|---|
| reader core | floating (CPU 7 at the sample) | floating (CPU 9 at the sample) | 14 (auto, 98 % idle) | 14 (auto, 95 % idle) |
| short kernel, ms/launch (3 reps) | 0.024, 0.027, 0.026 | 0.020, 0.088, 0.014 | 0.021, 0.014, 0.015 | 0.014, 0.014, 0.014 |
| rlprobe stage 4, median [min-max] | 2.7 s [2.5-2.8] | 2.3 s [2.1-2.3] | 2.1 s [2.1-2.1] | 2.3 s [2.0-2.3] |
| rlprobe transfers per second, median | 1062 | 1270 | 1375 | 1257 |
| ring to host doorbell: mean / p50 / p99 | 7.01 / 4.1 / 9.8 us | 4.91 / 3.6 / 7.1 us | 4.58 / 3.6 / 4.9 us | 4.61 / 3.6 / 6.9 us |
| ring to host doorbell: max | 32.2 ms | 43.6 ms | 34.5 ms | 21.7 ms |
| Device lock wait per wakeup: mean / p99 / max | 0.99 / 1.3 us / 2.3 ms | 0.50 / 0.9 us / 2.0 ms | 0.51 / 0.6 us / 2.0 ms | 0.57 / 0.9 us / 3.0 ms |
| rings (eventfd events) / guest doorbell writes | 245703 / 264612 | 251823 / 264612 | 258711 / 264612 | 255762 / 264612 |
| loop iterations / blocking waits | 245312 / 245312 | 1185708209 / 0 | 1173882020 / 0 | 19563095 / 2538 |
| reader CPU over the VM's life | 2.9 s in 200 s (1.4 %) | 196.7 s in 202 s (97.6 %) | 194.1 s in 202 s (96.0 %) | 6.2 s in 198 s (3.1 %) |
| device process CPU per rl run, median | 4.6 s | 14.1 s | 15.4 s | 5.6 s |
| trapped doorbells, host Xid | 120, 0 | 120, 0 | 120, 0 | 120, 0 |

**Pass 1 (vm0's device active during C and D0):**

| Value | C | D0 | D | E |
|---|---|---|---|---|
| reader core | floating | floating | 6 (auto, 86 % idle; no candidate at 90 %) | 15 (auto, 99 % idle) |
| short kernel, ms/launch (3 reps) | 0.027, 0.043, 0.034 | 0.016, 0.012, 0.024 | 0.014, 0.014, 0.020 | 0.020, 0.018, 0.020 |
| rlprobe stage 4, median | 2.7 s | 2.9 s | 2.1 s | 2.2 s |
| ring to host doorbell: mean / p50 / p99 | 5.38 / 3.3 / 29.8 us | 5.28 / 3.5 / 21.8 us | 4.41 / 3.6 / 5.4 us | 5.05 / 3.6 / 6.1 us |
| Device lock wait: p99 | 9.6 us | 1.9 us | 0.7 us | 0.8 us |
| reader CPU | 2.7 s (1.3 %) | 209.7 s (95.1 %) | 192.7 s (94.4 %) | 6.1 s (3.1 %) |

Readings:

- **The wake-up is the gain.**
  - The short kernel drops from 0.024-0.027 ms (C, pass 2) to 0.014-0.015 ms per launch with a
    spinning or hybrid pinned reader, about 10 to 12 us per launch. The time from ring to host
    doorbell inside the device barely moves (p50 4.1 to 3.6 us), so most of the gain lies before
    `epoll_wait` returns: the blocked thread's scheduler wake-up.
  - rlprobe stage 4 goes from 2.7 s to 2.1-2.3 s, and transfers per second from 1062 to about
    1260-1380 (native 1.3 s and 2266 per second, v1 3.4 s, benchmark.md).
  - Compared with this morning's C (benchmark.md: 0.018-0.024 ms, 3.0 s), today's C is also a
    little faster on stage 4. The host was quieter.
- **Pinning buys the tail, not the median.**
  - Unpinned spinning (D0) has the same median ring-to-host time as pinned. Its p99 rises from
    4.9-5.4 us to 7.1 us on a quiet host and to 21.8 us with vm0's device busy.
  - D0 also had the one outlier launch of the series (0.088 ms, rep 2 of pass 2) and the
    slowest rlprobe of pass 1 (2.9-3.6 s).
  - The blocking reader's p99 is 9.8 us on a quiet host and 29.8 us on a busy one.
- **Hybrid is nearly as fast as spinning at a thirtieth of the CPU.**
  - E blocked 2538 times in 198 s against about 256000 rings: it sleeps in the pauses (boot,
    setup, between reps), because while a workload runs the next submission comes within 200 us.
  - E costs 3.1 % of a core over the VM's life, against 96-98 % for spin.
  - Its short kernel equals D's; its rlprobe is within 0.2 s.
  - For a desktop host, hybrid is the setting to use.
- **Spin costs one core for the whole life of the VM.** That includes boot and idle time, and it
  shows in the device process CPU (4.6 s against 14-15 s per rl run). One `epoll_wait(0)` over 64
  eventfds takes about 170 ns (1.17 billion iterations in 202 s).
- **No contention on the Device mutex.** The reader takes the lock only for a ring. The wait per
  wakeup is 0.5 us at the median and at most 0.6-0.9 us at p99 in the polling modes, lower than
  the blocking reader's (1.3 us p99 in pass 2, 9.6 us in pass 1). The maxima of 2-3 ms in every
  mode are the vfio-user server thread holding the lock through long trapped requests, such as
  a channel's first doorbell that builds its mirror (inferred from their size, not traced).
- **What remains inside the device.** About 3.6 us at p50 from the eventfd to the host doorbell
  write. That time goes to the shadow re-sync of the channel's VA space, the worker barrier,
  the BAR1 write-combine flush and the mirror bookkeeping (`Mirror::gr_shadow_doorbell`). It is
  the next target on the device side, independent of the event source. The millisecond maxima
  of the ring-to-host time occur in every mode alike, so they come from the device's work
  (most likely a shadow re-sync after new mappings), not from the reactor.
- The automatic choice worked all four times. It took the highest-numbered idle core that no
  vCPU thread occupied at the sample: 6, 15, 14, 14. In pass 1 the vCPUs sat on 8 of the 16
  cores and no remaining candidate was 90 % idle, so it fell back to the most idle one, CPU 6
  (86 %). The vCPU threads are not pinned, so the choice only avoids where they were at that
  moment.

## Host tuning for the reactor core (recommendations, not done here)

The pinned reader still shares its core with everything the scheduler and the interrupt
controller put there. To make the core the reactor's alone:

- **Keep the scheduler off it.** Use `isolcpus=domain,managed_irq,<n>` on the kernel command
  line (`/etc/default/grub`, a reboot), or at run time a cgroup v2 cpuset partition:
  - `cpuset.cpus.partition=isolated` on a child cgroup that owns `<n>`;
  - every other slice (`system.slice`, `user.slice`, the VMs' vCPU threads) confined to the
    remaining cores through `AllowedCPUs=`.

  The runtime route needs no reboot and can be undone.
- **Stop the tick.** With `nohz_full=<n> rcu_nocbs=<n>` a core that runs a single task gets no
  periodic tick. This is also a boot parameter. Check `CONFIG_NO_HZ_FULL` in the running
  kernel first.
- **Move interrupts away.** Set `/proc/irq/*/smp_affinity_list` to the other cores, and mark
  the core as excluded for `irqbalance` (`IRQBALANCE_BANNED_CPULIST=<n>`). The NVIDIA
  interrupt and the NIC's queues matter most.
- **Keep the SMT sibling quiet too.** On this Ryzen 7 3800X, CPU n and n+8 share one core. A
  spinning reader on 15 slows whatever runs on 7, and the other way round.
- **Keep the core awake.** A core in a deep C-state has to wake up first. Spinning keeps it
  in C0 anyway; in hybrid mode `cpuidle` states can be limited per core through
  `/sys/devices/system/cpu/cpu<n>/cpuidle/state*/disable`.

The desktop runs on this host, so all of this is the operator's call. None of it was changed
for the measurement.

## Next step: P24, polling memory instead of eventfds

The reactor removes the scheduler wake-up. It still leaves one KVM exit per doorbell (the
ioeventfd write, about 1.2 to 1.4 us on this host, doorbell-token-pool.md (b)) and one
`epoll_wait` syscall per loop iteration. P24 (README "Ideas", the poll-mode GSP) would change
both:

- **No exit at all.** The guest's doorbell write, or its GPPut write, lands in memory that the
  device polls. The reactor would read GPPut (or a memfd-backed doorbell page in BAR0) of every
  live channel per iteration and compare it with the last value it saw. That is a load per
  channel instead of a syscall.
- **What that needs.** GPPut must be readable by the device without an uncached BAR1 read:
  - with P21, USERD sits in VRAM behind the guest's write-combined BAR1 mapping, and a device
    read goes through the host's BAR1 window (tens of microseconds, bar1-passthrough.md);
  - so USERD has to live in guest sysmem (the RM's instance-location keys), or the channel's
    USERD has to be placed in a page the device maps as ordinary memory (the channel-alloc
    USERD parameters);
  - alternatively the doorbell register page becomes a memfd-backed sparse mmap area that the
    poller watches, which needs no USERD change but costs one cache line per device.
- **What stays.** The loop of this package, the pinning and its automatic choice, the
  histograms and the summary carry over unchanged. Only the source of events changes: memory
  loads replace `epoll_wait`, and the eventfds remain as the fallback for channels without a
  polled page. One reactor core could then serve several devices, as P24 plans.

## The launch path (2026-09-25)

[launch-path.md](launch-path.md) splits the "ring to host doorbell" time of this reactor into its
steps (`CARAXES_LAUNCH_TIMING=1`) and halves it with `CARAXES_LAUNCH_FAST=1` (p50 6.4 to 3.0 us in
a launch loop, 9.8 to 3.7 us in rlprobe): the 3.6 us of the first measurement had become 6 to
10 us once USERD moved into the BAR1 pool, because every GPGet and GPPut read and the flush are
uncached BAR1 reads. It also measured the adaptive spin window (`CARAXES_DOORBELL_POLL_ADAPT_US`,
`Wait::with_adapt`): fewer blocking wake-ups, more reactor CPU, no measurable guest gain; off by
default. P24 stays unbuilt: with the pool, polling GPPut costs an uncached read per channel per
iteration (launch-path.md §5).

<!-- SPDX-License-Identifier: MIT -->
# vm4-615 invalidate gate: A/B against the hold under the device lock (2026-09-26)

Question: the invalidate gate (`vfio-user-nvgpu-v615-invhold-3bad9810`, branch
`worktree-agent-ab02521256d875ecb`, execution.md §8) makes the guest's MMU-invalidate trigger read
busy until the shadow worker applied the plan. The device lock is no longer held during that wait.
Does this remove the streaming cap measured in `../vm4-615-latency/NOTES.md`, and does new GPU
work complete while Moonlight streams?

## Setup

- One VM: vm4-615, 8 GiB, the stage defaults of `sottr-2026-09-24/`. Those are the interrupt page
  and the display page, token pool 64, the hybrid reactor, the MSI re-arm ioeventfd, the BAR1 pool
  (64 MiB), the BAR2 memfd and lazy allocation. The encoder is NVENC, `SOTTR_NAT=0`, and Moonlight
  runs at 1920x1080, 60 fps.
- The stage scripts are a private copy with the override line of the latency matrix
  (`tools/stage-env-override.diff`). Cell A has no override (`tools/cfgA.env`). Cell B sets
  `CARAXES_SHADOW_INVALIDATE_ASYNC=0` (`tools/cfgB.env`), and the device confirmed it with
  `WARN held invalidates wait under the device lock` (`B-hold/device-mode.txt`).
- The tools are those of the latency matrix: `win.sh` and `ana.py` are unchanged, and `run.sh` is
  adapted.
  - `tools/main.sh` waits for the preconditions and runs A, then B.
  - `tools/gate.py` gives per-window trigger, busy-read, slow-poll and kick-gap statistics
    (`gate-windows.txt`).
- Both cells were fresh boots. A: 01:13-01:33. B: 01:34-01:42.
  - Moonlight's first connect gave video in both cells (4.9 s and 3.7 s). Sunshine was restarted
    before it, as in the matrix.
  - Windows: w1 idle and w2 cursor test without a client, then w3/w5 streaming idle and w4/w6
    cursor test while streaming. B has w1-w4 only.
- The win11 device ran without `--host-rm` (no host RM) the whole time. Host MemAvailable was at
  least 17.9 GB. Host Xid: none in either cell (`*/host-xid.txt`).

## Table

The row format is that of the latency matrix. Streaming kicks are w3/w5 (idle) and w4/w6
(cursor). Sunshine latency is the min of the minima / max of the maxima / mean of the averages
over the streaming windows. Reactor lock wait, slow polls and the mirror counters come from the
device's exit summary, so they cover the whole session: A includes the five cycles and ran
1191 s, B ran 514 s.

| Cell | Kicks/s idle (w1) | Kicks/s streaming idle | Cursor kicks/s: no client / streaming | Sunshine latency min/max/avg ms | Guest sy / st % | Lock wait p99 (session) | Slow polls >2 ms (session; streaming windows) | Held invalidates | Busy trigger reads / waited past 1 s | Flip warnings | TLB invalidation failed | Host Xid |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A gate (async) | 21.0 | 19.6 / 17.3 | 34.8 / 15.4, 18.1 | 20.6 / 168.7 / 53.4 (53.9, 59.4, 56.9, 43.6) | 7.5 / 4 | 121.5 ms | 434; 0, 0, 0, 2 | 30553, 70.4 s until applied, longest 102 ms | **41491 / 0** | 5 | 0 | 0 |
| B hold (`ASYNC=0`) | 20.4 | 19.1 | 43.6 / 17.0 | 28.6 / 176.8 / 56.5 (53.3, 59.8) | 10 / 3 | 70.9 ms | 288; 6, 0 | 8217, 15.6 s under the lock, longest 94 ms | 0 / 0 | 8 | 0 | 0 |
| c1 baseline, poollock (matrix) | 31.2 | 9.0 / 0.0 | 24.6 / 21.2, 24.8 | 17.8 / 219.2 / 53.8 | 7.0 / 1.8 | 185 ms | 146 | 16440, 30.8 s, longest 102 ms | - | 0 | - | 0 |
| c6 baseline repeat (matrix) | 10.7 | 12.4 / 0.0 | 37.6 / 18.3, 12.8 | 20.2 / 223.7 / 56.5 | 4.2 / 3.0 | 5.0 s | - | 13577, 28.3 s, longest 77 ms | - | 2 | - | 0 |

### Per window, trigger and hold (`gate-windows.txt`)

| Window | Trigger writes/s | Busy reads | Trigger write to TRIGGER FALSE: p50 / p99 / max, sum over the window | Longest trace gap |
|---|---|---|---|---|
| A w3 streaming idle | 87 | 747 | 0.2 / 5.2 / 14.9 ms, 0.80 s (4 %) | < 40 ms |
| A w4 cursor streaming | 82 | 748 | 0.2 / 6.4 / 25.0 ms, 0.91 s | < 40 ms |
| A w6 cursor streaming | 105 | 916 | 0.2 / 3.8 / 32.9 ms, 0.70 s | < 40 ms |
| B w3 streaming idle | 83 | 0 | 0.1 / 4.9 / 14.9 ms, 0.49 s (2.5 %) | < 40 ms |
| B w4 cursor streaming | 79 | 0 | 0.1 / 5.6 / 48.7 ms, 0.62 s | < 40 ms |

- In B the time is lock hold. In A the lock is free between busy reads.
- The trace has no gap over 40 ms in any streaming window, in either cell. The device was never
  stalled for the length of a lost frame.

## Five Moonlight connect/disconnect cycles (cell A)

Each cycle killed Moonlight by the pid in `session/moonlight.pid`, paused 30 s, ran
`stream-615.sh`, waited 5 s after the first video packet, then ran `vkcube-wayland --c 120` and
`glmark2-es2-wayland -b build:duration=8` in the guest session. `c0` is the same work on the first
stream, before the cycles.

| Cycle | First video packet | vkcube (120 frames) | glmark2 build (on-screen) | Host Xid 31 |
|---|---|---|---|---|
| c0 (first stream) | 4.9 s | rc 0, 20.0 s | 12 fps, score 11 | 0 |
| 1 | 3.1 s | rc 0, 30.0 s | 16 fps, score 15 | 0 |
| 2 | 1.9 s | rc 0, 18.2 s | 22 fps, score 21 | 0 |
| 3 | 2.2 s | rc 0, 16.8 s | 21 fps, score 20 | 0 |
| 4 | 2.3 s | rc 0, 17.9 s | 17 fps, score 16 | 0 |
| 5 | 2.0 s | rc 0, 17.2 s | 22 fps, score 21 | 0 |

- Sunshine kept streaming through all cycles. It logged no "Hang detected" and no fatal line
  (`A-invhold/c*/sunshine.txt`). Its latency lines during the cycles were 17.6-21.3 /
  107-280 / 44-68 ms.
- The poollock night run for comparison: glmark2 rendered 0 frames while streaming (5/5 cycles),
  and vkcube aborted in `demo_create_device` whenever Sunshine's CUDA/NVENC context was alive.

## Other observations

- One `ERROR untrapped GPPut: ... host channel ran ahead of its doorbell ... with plans pending`
  in A: "13 doorbells found the host channel ahead, 1 with plans pending".
  - c1 of the matrix (the old hold) had the same single line, so it is not new with the gate.
  - No FAULT_PDE and no Xid followed it.
- The rest of the device errors in A are the known pool ones: "remap refused, the guest maps it
  twice" and one host BAR1 window refused at the 96 MiB floor.
- Flip-event warnings (`__nv_drm_handle_flip_event`) came in both cells (5 in A, 8 in B), so they
  are independent of the gate. The matrix had 0-2.

## Verdict

- **The streaming cap is not gone, and the gate was not what held it.**
  - A and B are the same within run-to-run spread. Cursor-while-streaming: 15.4-18.1 (A) against
    17.0 (B) kicks/s. Sunshine average: 53 against 57 ms. Maxima: 169 against 177 ms.
  - Both match the matrix baselines (13-25 kicks/s, 54-57 ms).
  - With the hold under the lock (B), the held invalidates took 2.5-3 % of streaming wall time per
    window (p99 5 ms, max 49 ms). That is much less than the 17-22 % the matrix inferred from its
    session totals.
  - The gate does what it should: 41491 busy reads, none past 1 s, and no "TLB invalidation failed
    waiting" in the guest. But the kick rate does not move.
  - The reactor lock-wait p99 stays high in both cells: 121 ms in A (a longer session with
    cycles), 71 ms in B. So some other lock holder dominates the tail, or the tail is not the cap
    at all.
  - The trace shows no stall over 40 ms in a streaming window. The low kick rate while streaming
    looks paced by the guest (compositor or Sunshine capture), not blocked in the device.
- **New GPU work completes while streaming: yes, on this build, 6 of 6.**
  - vkcube finishes (rc 0, 4-7 fps) and glmark2 renders 12-22 fps on every connect of five cycles
    and on the first stream.
  - Host Xid stayed 0.
  - This is not attributed to the gate:
    - Cell B did not run the work test (A/B was specified for idle and cursor only).
    - main gained other changes since the poollock night run, for example the RTX 2070 context
      buffer sizes 0x11-0x19 (870f5030), a candidate for the c5c0 `NV_ERR_OBJECT_NOT_FOUND`
      behind the vkcube abort.
- **Next:**
  - A B-cell work test (vkcube and glmark2 while streaming with `ASYNC=0`) to attribute the
    work fix.
  - Find the lock holder behind the 70-120 ms lock-wait tail, for example with a trace point at
    lock hold > 20 ms.
  - Measure the guest-side pacing of Sunshine's KMS capture, which the kick rate appears to follow.

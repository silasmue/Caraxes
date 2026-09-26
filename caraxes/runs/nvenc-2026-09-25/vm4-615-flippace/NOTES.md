<!-- SPDX-License-Identifier: MIT -->
# vm4-615 display pacing: deferred BAR2 flush sync, A/B (2026-09-26)

## Verdict

- **The no-client display is fixed: 60 Hz.** The cursor test without a client gives 59.6 kicks/s
  in both deferred cells, with 99-100 % one-tick frames. latch → kick p50 is 2.3 ms and the
  pre-kick run p50 0.9 ms.
- **Streaming is about 2x better but still below 45.** The cursor test while streaming gives
  26-35 kicks/s against 11-13 with the old sync. Sunshine's average is 41-49 ms against 62-81.
  - The remaining time sits before the pre-kick run: latch → run p50 18-29 ms, against a run of
    only 1.4-2.0 ms. That is the guest side (Sunshine's capture chain, display-pacing.md §4), not
    the device.
- **Regression: new GPU work does not complete while streaming with the deferral.**
  - With defer: 0 of 4. vkcube aborted in `demo_create_device` and glmark2 rendered 0 frames, in
    A-defer once and in A2-defer three times, 2 min apart.
  - With the old sync in the same binary: 1 of 1 (vkcube rc 0, glmark2 10 fps).
  - vm4-615-invhold, sync on every read: 6 of 6.
  - The cause is not identified. The poollock night run had the same vkcube assertion under a live
    NVENC context. The deferral appears to reopen that race or ordering. **Do not merge as default
    before this is understood.**
- **The exit line is as expected.** About 5-6 % of the owed flushes are settled by the clock, and
  syncs run at 103-116/s instead of 411/s (B).
- Host: no NVRM or Xid line from 02:37 to 03:14. Guest: no "TLB invalidation failed" in any cell.
- B-sync reproduces A-invhold: cursor without a client 34.6 against 34.8 kicks/s, run 12.2 against
  12.8 ms.

## Setup

- Binary `vfio-user-nvgpu-v615-flippace-02c0a625` (main 39bade29 plus the deferral). vm4-615,
  8 GiB, stage defaults, NVENC, `SOTTR_NAT=0`, Moonlight 1080p60.
- The tools are those of `vm4-615-invhold/tools`: `run.sh` in mode `F` (w1-w6, then vkcube and
  glmark2 while streaming) or `F3` (the work test 3 times), `win.sh`, `gate.py`. The stage is a
  private copy with the override line.
- `display-pace.py` from the pacing worktree gives `pace-*.txt`.
- The cells ran on fresh boots, in the order A-defer (02:37), B-sync (02:48, with
  `CARAXES_BAR2_MEMFD_DEFER_FLUSH=0`), A2-defer (02:59, a repeat to check the work regression).
  linux-busy was claimed each time.
- The Windows VM ran beside it and was not touched.

## Kicks and Sunshine (win.sh)

| Cell | w1 idle | w2 cursor, no client | w3 / w5 streaming idle | w4 / w6 cursor streaming | Sunshine min/max/avg ms (w3-w6) |
|---|---|---|---|---|---|
| A-defer | 36.5 | **66.3** | 25.1 / 32.2 | 35.0 / 29.6 | 22.9 / 158.7 / 44.5 |
| A2-defer | 47.2 | **66.3** | 24.7 / 20.9 | 34.5 / 28.5 | 25.9 / 190.9 / 47.1 |
| B-sync | 21.9 | 34.6 | 14.1 / 15.4 | 12.6 / 11.8 | 33.5 / 195.4 / 71.5 |
| A-invhold (ref.) | 21.0 | 34.8 | 19.6 / 17.3 | 15.4 / 18.1 | 20.6 / 168.7 / 53.4 |

`win.sh` counts every 0x696000 write, `display-pace.py` only the UPDATE kicks, which gives 59.6
against 66.3.

## Frame timeline (display-pace.py, p50 ms)

| Cell / window | UPDATE kicks/s | One-tick frames | latch → kick | latch → run | Run | Flush-read spacing |
|---|---|---|---|---|---|---|
| A w2 cursor, no client | 59.6 | 99 % | 2.3 | 0.6 | 0.9 | 0.0 |
| A2 w2 cursor, no client | 59.6 | 100 % | 2.3 | 0.7 | 0.9 | 0.0 |
| B w2 cursor, no client | 31.2 | 26 % | 21.3 | 6.8 | 12.2 | 0.5 |
| A w4 / w6 cursor, streaming | 31.6 / 26.7 | 22 / 14 % | 20.9 / 25.1 | 18.4 / 22.1 | 1.4 / 1.5 | 0.1 |
| A2 w4 / w6 cursor, streaming | 31.1 / 25.6 | 22 / 14 % | 21.3 / 26.0 | 18.6 / 23.6 | 1.6 / 1.6 | 0.1 |
| B w4 / w6 cursor, streaming | 11.3 / 10.9 | 0 % | 68.4 / 77.8 | 62.0 / 71.8 | 2.1 / 0.0 | 0.5-0.6 |

All criteria for the cursor test without a client are met: at least 55 kicks/s, mostly one
tick, latch → kick under 12 ms, the run under 3 ms and the spacing under 0.1 ms.

## Device counters (exit summary, whole session)

| Cell | Session | BAR2 syncs (/s) | Flush reads deferred / settled at sync point / at doorbell / by clock | Lock wait p99 | Slow polls > 2 ms | Busy trigger reads (past 1 s) |
|---|---|---|---|---|---|---|
| A-defer | 650 s | 75334 (116/s), 33.5 s | 354860 / 43656 / 14983 / 3317 (5.4 %) | 111 ms | 187 | 15601 (0) |
| A2-defer | 906 s | 93040 (103/s), 53.6 s | 386865 / 57051 / 14638 / 4710 (6.2 %) | 47 ms | 255 | 26486 (0) |
| B-sync | 600 s | 246779 (411/s), 37.7 s | - (flush reads sync at once) | 117 ms | 228 | 14344 (0) |

## Work while streaming (vkcube-wayland --c 120, glmark2-es2-wayland build 8 s)

| Cell | vkcube | glmark2 |
|---|---|---|
| A-defer c0 | rc 134, `demo_create_device: Assertion '!err'` (37.9 s) | 0 frames, timeout |
| A2-defer c0 / c0b / c0c | rc 134 x3, same assertion | 0 frames x3 |
| B-sync c0 | rc 0, 26.0 s | 10 fps, score 9 |

- Sunshine kept streaming during all of them. Its latency lines were 21-32 / 83-211 / 37-53 ms.
- The device log shows no new error class, only the known pool lines: "remap refused", USERD
  pages held, one host BAR1 window refused at the floor.

## Next

- Find why the deferral breaks new work while NVENC streams.
  - Candidates: data a GPU engine or the GSP reads without a hand-over (a USERD or semaphore page
    polled across contexts); the 1 ms clock bound when a hand-over does not settle.
  - Test: run vkcube with defer and no Sunshine, then with `CARAXES_BAR2_MEMFD_DEFER_FLUSH=0`
    only for the no-client windows. Also run vkcube with the RM log, as in `vm4-615-poollock`
    run 2.
- For the streaming rate, the remaining time is guest-side (Sunshine's import cache,
  sottr-stage.md §11).

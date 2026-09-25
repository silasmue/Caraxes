<!-- SPDX-License-Identifier: MIT -->
# vm4-615 signalling-path latency matrix (2026-09-25/26)

Question: does the per-frame latency while streaming (display kicks down to about 10 per second,
Sunshine frame processing 55-96 ms in the poollock night run) come from the device's signalling
paths: (a) the interrupt page, (b) the doorbell reactor's wait mode, (c) the MSI re-arm path?

## Setup

- Device `/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-poollock-304012cc`, vm4-615, 8 GiB, NVENC
  (`h264_nvenc`), Moonlight 1920x1080 60 fps 20 Mbit/s on the host desktop.
- Base configuration = night run 1: interrupt page and display page on, token pool 64, hybrid
  reactor (200 us), MSI re-arm ioeventfd on, BAR1 pool 64 MiB pinned, BAR2 memfd, lazy alloc.
- The stage's `env.sh` exports the four switches unconditionally. It was not edited. The runs
  used a private copy of the stage scripts with one extra line: an override file sourced last
  (`tools/stage-env-override.diff`, `tools/cfg*.env`).
- Differences from the night run: `SOTTR_NAT=0` (no guest internet, so no host sudo), and Sunshine is
  restarted before the stream. The first connect to the boot probe's Sunshine failed, "No video
  traffic", and Sunshine then logged `Hang detected` (c1, `attempt0/`). c6 needed a second
  attempt.
- Each cell was a fresh boot and ran these 20 s windows:
  - w1: idle, Sunshine running, no client.
  - w2: cursor test (uinput, 100 moves/s), no client.
  - Stream start.
  - w3 and w5: streaming idle.
  - w4 and w6: cursor test while streaming.
- Scripts are in `tools/`: `run.sh` runs one cell, `win.sh` one window, and `ana.py` does the
  trace analysis.
- Host Xid: 0 in every cell. MemAvailable stayed at 17 GB or more.

## Table

Kicks = `W bar0 0x00696000` per second in the window. Streaming values are w3/w5 (idle) and
w4/w6 (cursor). Sunshine latency: min of the minima / max of the maxima / mean of the averages
over w3-w6, one 20 s report per window. Guest sy/st and device threads (sum of `top -H` %, doorbell
reader + vfio-user thread) are means over w3-w6. Reactor stats are the device's exit summary
(the whole session).

| Configuration | Kicks/s idle (w1) | Kicks/s streaming idle | Cursor test kicks/s: no client / streaming | Sunshine latency min/max/avg ms | Guest sy / st % (irq/s) | Device threads % (doorbell, vfio-user) | Flip warnings | Reactor stats (session) |
|---|---|---|---|---|---|---|---|---|
| c1 baseline | 31.2 | 9.0 / 0.0 | 24.6 (with Sunshine; 62.4 after Sunshine hung) / 21.2, 24.8 | 17.8 / 219.2 / 53.8 (83.5, 58.1, 35.6, 37.8) | 7.0 / 1.8 (10233) | 86 (40, 45) | 0 | hybrid, reader 21 % of a core; lock wait p99 185 ms; intr page 1.06 M scans, MSIs 262k raise / 142k re-arm / 517 watchdog; 16440 invalidates held, 30.8 s, longest 102 ms |
| c2 interrupt page off (and display page) | 0.0 | - | **0.0** / - | - (no stream) | - | 90 (2, 86) | 0 | **display stalls**: guest soft lockup in `RmIsrBottomHalf` (15x), gnome-shell, nvidia-modeset and Sunshine hung; 13932 pool polls over 2 ms |
| c3 spin | 39.9 | 6.1 / 16.3 | 58.3 / 17.1, 13.5 | 22.5 / 461.8 / 69.5 (117.6, 51.7, 54.9, 53.7) | 8.2 / 8.2 (9622) | 128 (81, 46) | 0 | spin, reader 86 % of a core; lock wait p99 94 ms; 0.99 M scans, MSIs 123k / 172k / 1240; 11439 held, 25.4 s, longest 90 ms |
| c4 interrupt page off + spin | 0.0 | - | **0.0** / - | - (no stream) | - | 182 (97, 84) | 0 | same stall as c2 (5 soft lockups); reader 96 % |
| c5 baseline + re-arm ioeventfd off (re-arm traps) | 14.3 | 11.6 / 14.1 | 50.4 / 8.5, 8.3 | 27.7 / 229.9 / 77.2 (80.7, 84.5, 60.6, 82.9) | 10.2 / 6.5 (8907) | 90 (37, 51) | 1 | hybrid 23 %; lock wait p99 105 ms; 0.47 M scans, MSIs 104k / 98k / 1869; 10910 held, 23.6 s, longest 130 ms |
| c6 baseline, repeat | 10.7 | 12.4 / 0.0 | 37.6 / 18.3, 12.8 | 20.2 / 223.7 / 56.5 (71.1, 45.7, 38.9, 70.3) | 4.2 / 3.0 (8290) | 88 (35, 52) | 2 | hybrid 25 %; lock wait p99 5.0 s (one stall, around the failed first connect); 0.68 M scans, MSIs 151k / 113k / 797; 13577 held, 28.3 s, longest 77 ms |

Streaming idle kicks depend on screen content: 0.0 means a static desktop with no damage. The
cursor rows are the comparable load.

### Notifier to handler

- The trace cannot give the time from a host notifier firing to its MSI write:
  - The event thread's `host notifier ... fired` notes are queued. They reach the trace at the next
    traced event, not when they fire.
  - Page mode writes the MSI from the reactor without a trace line.
- Proxy (`tools/ana.py`): the time from a flushed `fired ... MSI` note to the next MSI re-arm, the
  guest's end of the ISR (eventfd E line, or trapped `W 0x88704` in c5). It includes the queueing
  delay.

| Cell | p50 / p90 / p99 over the streaming windows |
|---|---|
| c1 | 2.5-3.4 / 5.8-7.9 / 15-19 ms |
| c3 spin | 2.8-4.8 / 9.3-24.8 / 21-61 ms |
| c5 re-arm trapped | 2.8-3.8 / 7.9-10.9 / 22-30 ms |
| c6 | 2.9-3.4 / 6.0-9.8 / 14-28 ms |

- No configuration brings this into the sub-millisecond range. An interrupt's handling takes
  milliseconds end to end in every cell.

## What the device is doing while streaming

- **The held invalidates are the largest per-frame cost the counters show.**
  - Sunshine's KMS capture maps and unmaps the scanout buffer every frame (sottr-stage.md §11,
    "Sunshine's share"). Its root has an untrapped USERD. Each guest MMU invalidate on that root
    is held until the shadow worker has applied the plan (`mirror.rs` `shadow_invalidated`:
    `self.worker.wait_all()` under `&mut self`, so under the Device mutex).
  - Without a stream (c2, c4): 323 and 591 held invalidates over the whole session, under 1 s in total.
  - Each streaming cell held 10.9k-16.4k invalidates in about 140 s of streaming. That is 78-115
    per second (about 2 per frame at 60 fps) and 23.6-30.8 s of hold, **17-22 % of streaming wall
    time with the device lock taken**. The longest single hold was 77-130 ms.
  - While an invalidate is held, the display clock cannot latch flips or raise the vblank. The
    trapped accesses of every vCPU, the doorbells and the re-arms queue behind it too.
  - The reactor's "Device lock wait per wakeup" p99 is 94-185 ms in every streaming cell, and 13.6 ms
    without a stream (c2). The maximum held invalidate (77-130 ms) and the lock-wait p99 are of the same
    order.
- The signalling paths do not move the streaming numbers beyond the run-to-run spread:
  - c1 and c6 are the same configuration. Cursor-while-streaming kicks: 21-25 (c1), 13-18 (c6). Latency average 54 and 57 ms.
  - Spin (c3) gives 13.5-17.1 kicks/s and a 69.5 ms average. It costs a whole host core (reader 86 %,
    device threads 128 % against 86-90 %). The guest's steal time rose to 8 %.
  - Without a stream, spin helps the cursor test (58 kicks/s against 25-38 with hybrid, the
    same order as the 62 with Sunshine gone). Hybrid's wake-up does cost something when the device lock is free.
    With the stream, the lock is not free, and spin gains nothing.
- **Interrupt page off does not work on this stage.** With the trapped interrupt tree and trapped
  display FE page (c2, and c4 with spin):
  - The guest's ISR bottom half soft-locks (`_intrServiceStallExactList` / `RmIsrBottomHalf`,
    first at 232 s and 420 s).
  - The desktop never flips again. The cursor test gives 0 kicks, Sunshine never opens its port, and
    gnome-shell, nvidia-modeset and Sunshine sit in D state.
  - The trapped reads are dominated by `NV_PDISP_FE_RM_INTR_DISPATCH` 0x611ec0: 36k in a 20 s
    window (about 1750/s), plus 0x611c0c and the VF tree.
  - Each trapped write is a BAR1-pool sync point: 24-34 polls per second over 2 ms, 73-91 ms of
    every second held.
  - Hypothesis, not verified: the display ISR's dispatch loop sees a new pending state at every
    read. A FE-register read advances the display clock (intr-page.md §6a), and each read waits
    behind the pool poll, so the loop never drains.
  - The "trapped tree, MSI from the event thread" path is therefore no latency alternative. It
    needs a fix before it can even be measured with the BAR1 pool on.
- **Re-arm ioeventfd off (c5) is worse**:
  - Cursor-while-streaming kicks 8.3-8.5 against 13-25, Sunshine average 77 ms against 54-57.
  - Two of the four windows had a larger kick count (idle streaming 11.6/14.1). That is screen content, not the path.
  - The trapped re-arm is one more vCPU exit per interrupt, and it waits behind the same lock.

## Conclusion

- **Largest latency contributor:** the device-lock hold of the untrapped-GPPut invalidate rule
  (`CARAXES_SHADOW_UNTRAPPED_HOLD`, bar1-pool.md §9). It is triggered by Sunshine's per-frame
  import of the scanout buffer. None of the signalling paths is it:
  - The hold takes the Device mutex for 17-22 % of streaming time, in holds of up to about 100 ms.
  - The flip latch, the vblank MSI and every trapped access wait behind it. That matches the
    kick rate collapsing only when Sunshine captures, and the 100-200 ms Sunshine maxima.
  - Interrupt page, reactor mode and re-arm path each shift the numbers by less than the spread
    between two baseline boots, or make them worse.
- **Recommended default: keep the base configuration.**
  - Interrupt page and display page on (off stalls the 615 guest).
  - Hybrid reactor (spin burns a host core and gains nothing while streaming).
  - MSI re-arm ioeventfd on (trapping it is worse).
- **Next work:**
  1. Stop holding the Device mutex while `wait_all()` waits for the shadow worker, or apply the
     held root's plan without the lock.
  2. Or remove the per-frame map/unmap on the guest side: the Sunshine cache of the imported
     `egl::rgb_t`, sottr-stage.md §11.
  3. A cell with `CARAXES_SHADOW_UNTRAPPED_HOLD=0` would confirm the attribution. It was not run
     here, because the rule exists to prevent a host `FAULT_PDE` Xid.
- **Left open:**
  - The page-off stall's mechanism.
  - The notifier-to-MSI time, which needs a trace point at the MSI write.
  - A cell with the hold removed.

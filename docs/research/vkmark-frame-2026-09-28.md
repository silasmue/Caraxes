<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# vkmark per frame (2026-09-28)

Scope: where a vkmark frame's time goes in a 615 Linux guest (scenes cube and clear, 1280x720, Wayland under GNOME/Mutter) against native; channels, traps, interrupts, idle gaps and the 60 Hz compositor; fixes ranked by gain per effort. Run: `caraxes/runs/night-2026-09-28/vkmark-trace-dev/` (Linux B, 8 vCPUs, 1 GiB slice, compression slice, read-only display page, `CARAXES_LAUNCH_FAST=2`, host boost on, `CARAXES_LAUNCH_TIMING=1`). Device traces `T` = `/mnt/vmstore/caraxes/night-2026-09-28/vkmark-trace-dev/trace-{cube,clear}.txt` (µs since device start; cube T:34.388-44.492 s, clear T:46.664-56.562 s); `G` = `gpu.log` in the same folder. Native: `caraxes/runs/night-2026-09-28/native/vkmark-wl-720-1.txt`. `D` = `crates/vfio-user-nvgpu/src/`.

## Method

- Reliable markers: `doorbell token ... (eventfd) -> chid N` (the reactor's ring), `msi re-arm` (the end of a guest ISR), trapped `R`/`W` lines (stamped after the device handled them), `display: window 6 latch`, and `W bar0 0x0061180c` (the guest's vblank ack, taken as phase 0 of each 60 Hz period).
- `event: host notifier ... fired` lines are queued and written late [video-path-2026-09-28.md, Method]; they are only counted.
- Frames = vkmark FPS × scene span; rings per frame = chid 10 rings / frames.
- Two views: events folded on the vblank ack (0.5 ms bins), and every gap between consecutive markers assigned to its pair (ring→ring, re-arm→ring, ring→re-arm, re-arm→re-arm, trap). Scratch scripts, not committed.

## Facts

### Per frame, by channel

| Per frame | cube | clear | Evidence |
|---|---|---|---|
| guest FPS, traced (frame time) | 3389 (295 µs) | 4266 (234 µs) | vkmark-cube.txt, vkmark-clear.txt |
| guest FPS, smoke runs | 3834, 5422 | 3985, 5335 | linuxb-night/smoke-graphics.txt |
| native FPS (frame time) | 17850 (56 µs) | 15731 (64 µs) | native/vkmark-wl-720-1.txt |
| app GR channel chid 10: rings (pool token 0xca700003, eventfd, untrapped) | 4.47 | 4.83 | T |
| app CE or other channels | 0 | 0 | T |
| guest interrupts: MSI re-arm writes (vector 4 = host notifier 0x800000c, GR non-stall) | 2.72 | 3.13 | T |
| trapped accesses on the app's own path | 0 | 0 | T |
| trapped accesses in total (all from the vblank ISR and the compositor's flip path) | 0.88 | 0.72 | T |

- **Mutter is not in the app's GPU path.** Its GR channel chid 3 rings 2.9 times per 60 Hz period (173-178/s), and window 6 latches at 60 Hz [T; G `display engine`]. vkmark presents in mailbox mode (its default per `vkmark --help` on the host; the guest's build was not checked).
- **The traps come at a fixed rate, about 3000/s, in both scenes.** Per 60 Hz period there are about 50 [T]:
  - 22.5 BAR2 flush reads;
  - 13.8 `TIME_0/1` reads;
  - 2 MMU invalidates of the guest RM's BAR2 VAS (pdb 0xe7543000), 5 accesses each;
  - 2 display-page writes (0x611858, 0x61185c) and the vblank ack;
  - 1.1 window-channel kicks.
- **Cost per trap** (p50 gap between consecutive traps on one guest path, stamped after handling) [T]:
  - `TIME` read: 29-33 µs;
  - flush read: 42-43 µs;
  - one invalidate: about 430 µs, of which the trigger write (the BAR2 sync) is 267-276 µs (p90 470-630);
  - display-page write: 147-180 µs;
  - window kick: 229-237 µs.
  - A plain trap costs 23-27 µs [windows-and-traps.md, Cost per trap]. On hardware each of these is about 1 µs.

### The 60 Hz window

- **Folded on the vblank ack**, the app's ring rate is 10-25 % of its steady rate from the latch to 4 ms after the ack (about 40 % in the first 0.5 ms). It is back to 90 % or more from about 7 ms to the next latch [T].
- **vblank ISR (latch → ack): p50 642 / 572 µs** (cube / clear). It traps 2 display-page writes and 6 `TIME` reads.
  - Re-arms inside the window: 0.01-0.03 per window.
  - The first re-arm comes 98-115 µs after the ack, followed by 5.4-6.4 in the next 300 µs.
  - So the vblank ISR holds every GR completion for about 0.6-0.75 ms. The device sends no second MSI before the guest's re-arm, and one vCPU takes all of the device's interrupts [T; intr-page.md §11.9].
- **Flip path (ack → last trap): p50 3.72 / 3.65 ms.** It holds two runs of 11-12 flush reads, each closed by an invalidate, plus the window kick, about 8 `TIME` reads and Mutter's 3 rings. Trap time is about 2.3 ms of it [T].
- **In the flip path the app is not waiting for the GPU** [T]:
  - 1193 / 1198 app stalls of 500 µs or more start there;
  - 98-99 % of them have 1-6 re-arms inside, the first p50 48-57 µs after the app's last ring;
  - the app then stays idle for another p50 1.19 ms after the last re-arm.
  - So its completions arrive, and it waits for something the compositor's kernel path holds: Mutter's thread (the Wayland commit and release) or a guest RM or nvkms lock. The device trace cannot tell which.
- **Cost:**
  - inside the windows the app rings 14.3 / 16.8 times per period, against 255 / 334 in the remaining 12.4-12.6 ms (20.6 / 26.5 rings per ms);
  - at the steady rate a period would give 77-80 / 92-102 frames instead of 60 / 73;
  - that is 22-25 % / 20-28 % of the frame time: **60-68 / 47-66 µs per frame**.
- **Trace artefact inside the window:** about 190 queued notifier lines are written in one batch at the window's first boot-model write (`W 0x00b830a0`). That adds about 0.2 ms to the guest's trap [T:37985143-37985456, 38000372-38000640].
- **Native:** the same ~50 register accesses cost about 0.05-0.1 ms per period (estimate at ~1 µs each). That is under 1 % of the time.

### The steady frame (5-15 ms after the ack)

| Per frame, µs | cube | clear | Note |
|---|---|---|---|
| steady frame (20.2 / 27.6 rings per ms) | 221 | 175 | T |
| guest between rings of a burst (ring→ring, p50 gap 7 µs) | 38 | 39 | guest CPU and the doorbell exits |
| re-arm → next ring (p50 8 / 7 µs; share beyond 100 µs: 12) | 36 | 36 | wake-up and driver |
| ring → re-arm, with work outstanding | 54 | 43 | GPU, host IRQ, event, MSI, guest ISR |
| re-arm → re-arm | 47 | 33 | as above |
| traps, remaining flush runs, gaps at the phase boundary | ~47 | ~23 | |

- **The loop is released by interrupts.** 37-41 % of the app's rings come 5-8 µs after a re-arm, that is 1.6 / 2.0 wake-ups per frame. The fastest recurring shape is: re-arm; 3 rings within 17 µs; re-arm at +39 µs; 2 rings; re-arm at +62 µs; idle until +112 µs [T:37990245-37990357].
- **Device legs of one interrupt** (whole VM life; 99 % of the host events fall in the two scenes) [G `launch timing`]:
  - ring → host doorbell: p50 2.4 µs (mean 3.9), then 2.3 µs of trace write after it;
  - host doorbell → host event: p50 27.2 µs, p90 52.4, p99 180. It is measured from the last host doorbell, so it is a lower bound;
  - host event → raise: 0.7 µs;
  - raise → MSI: 1.1 µs (p90 6.2, p99 190).
- **MSI → guest re-arm is not measured.** The shortest re-arm → re-arm gap is 10-14 µs (p10) [T]. So one interrupt-released step is roughly 2.4 + ≥27 + 1.8 + ~10 + 7-8 ≈ 50 µs (estimate).
- **Long gaps without a trap:** 12-16 % of the scene time is gaps of 200 µs or more with no trap inside (p50 about 310 µs, 1-2 re-arms each), spread evenly over the steady phase [T]. They match the tails of the interrupt path (p99 180 µs and 190 µs above).
- Cube waits 25 µs more per frame than clear, although native cube is faster. The host clocks were not sampled in this run.

### Budget per frame

| µs | native cube | guest cube | native clear | guest clear | Confidence |
|---|---|---|---|---|---|
| frame | 56 | 295 | 64 | 234 | high |
| 60 Hz windows (vblank ISR, flip path) | ≈1 (estimate) | 60-68 | ≈1 (estimate) | 47-66 | high (guest) |
| steady: guest CPU (bursts, interrupt → ring) | ≤56, not split | 74 | ≤64, not split | 75 | medium |
| steady: waiting with work outstanding | not measured | 101 | not measured | 76 | medium |
| steady: traps, other | 0 | ~47 | 0 | ~23 | low |
| scene start, remainder | - | ~5-15 | - | ~0-10 | low |

- The device cannot see guest CPU work that falls inside a "waiting" gap. The CPU and waiting split is therefore a bound, not a measurement.
- No native split exists (no native trace).

## Consequence for Leandro VFIO

| # | Fix | Effort | Gain per frame, cube / clear (estimate) | Proof | SotTR | Confidence |
|---|---|---|---|---|---|---|
| 0 | Baseline without the per-access trace. The launcher always passes `--trace`: 2 lines and a flush per ring and per re-arm on the reactor, and the 0.2 ms notifier batch in the flip path | trivial (launcher flag) | −10 to −25 µs | same binary, trace on/off, 3 reps each | small, free | medium |
| 1 | vblank ISR without slow traps: find why a read-only display-page write costs 147-180 µs against ~30 µs for a trap (`FePage::write`, `after_bar0_write`); `TIME` via `CARAXES_USERMODE_PAGE=1` | small-moderate | −6 to −10 µs | latch → ack p50 ≤ 150 µs; first re-arm ≤ 30 µs after the ack | yes: every vblank holds all of a game's completions for 0.6-0.75 ms (pacing, latency) | high (mechanism), medium (size) |
| 2 | Flip path: `CARAXES_INVALIDATE_PAGE=1`, `CARAXES_USERMODE_PAGE=1`, `CARAXES_DISPLAY_KICK_PAGE=1` (in dev, opt-in, not yet GPU-validated [intr-page.md §11.3, §12.2-12.3]), plus a cheaper invalidate sync (267-276 µs today; profile the page compares, the replica merges of the doubly mapped BAR1 page [G `ERROR: bar1 pool ... replica`] and the pool poll) | config + moderate | −15 to −25 µs | ack → last trap p50 ≤ 2 ms; app rate 0-4 ms after the ack ≥ 60 % of steady | yes: every flip runs it, at the game's rate with direct scanout | medium |
| 3 | Completion path: the host event fds in the spinning reactor's epoll set, with `CARAXES_INTR_DIRECT=1`; new histograms for MSI → guest ack store and ack → re-arm | moderate | −10 to −30 µs | ring → re-arm steps; host doorbell → MSI p50 at least 5 µs lower | minor (under 1 % of a 7-16 ms frame) | medium |
| 4 | vCPU placement: pin the vCPU that takes the MSI (and the app's vCPU) on idle host cores; KVM halt polling for this VM | small, host decision | −10 to −40 µs (unknown) | host `perf kvm stat` or `kvm:kvm_vcpu_wakeup` (root); guest `perf sched latency` | yes: the vblank delivery tail [intr-page.md §11.9] | low (size) |
| 5 | Faster trap round trip (VMM ↔ device, 25-30 µs → ≤ 8 µs). This makes the 23 remaining flush reads per period cheap | large | −10 to −15 µs | `TIME` → `TIME` p50 ≤ 10 µs | yes: all remaining traps | medium |

- **Do not use `CARAXES_BAR2_FLUSH_PAGE=memory` for this workload.** With it every doorbell pays a sync (P1: ring → host doorbell 10 → 56 µs), and there are 4.5 rings per frame [intr-page.md §12.1].
- **Expected with 0-3:** about 205-255 µs per cube frame, 0.22-0.27× of native frame time (0.19× in this traced run). With 4 and 5 perhaps 0.3-0.4× (estimate).
- **±10 % at 4000+ FPS is out of reach for this design.** Every frame still pays 1.6-2 mediated interrupt round trips and the guest's exits and wake-ups. That would only change if the guest's waits stopped depending on interrupts, which the device does not control.
- **Games are a different case.** At 60-150 FPS the fixed per-frame costs (about 0.15-0.25 ms) are 1-4 % of a frame. The 60 Hz windows (fixes 1 and 2) matter for pacing and for any present path that waits on Mutter. So vkmark's ratio is not a proxy for SotTR; SotTR's own trace should check the flip-path windows per game frame.

## Open

- **What holds the app in the flip path** (Mutter's release or a guest RM/nvkms lock): an off-CPU profile (`perf sched`/offcputime) of vkmark and gnome-shell during one cube run in the guest; `vkmark --present-mode fifo` and `immediate` as contrasts.
- **Whether native waits on interrupts at all:** a native `perf trace` of vkmark (poll/ioctl waits); there is no native split today.
- **Host GPU time per frame and clocks during the scenes:** sample host `nvidia-smi` (utilisation, clocks) every 100 ms for one native and one guest cube run. During the smoke run's graphics block the host sat at P0, 1.0-1.85 GHz, with 27-42 % utilisation [linuxb-night/clocks-smoke.csv]; which minutes were vkmark is not recorded.
- **The MSI → guest re-arm leg**, and the cause of the gaps of 200 µs or more without a trap (host GPU or IRQ tail, or guest ack latency).

<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Night 2026-09-28: CUDA, llama.cpp, PyTorch, Windows

The user's end goal (2026-09-28): SotTR on Windows and Linux, CUDA/LLM/PyTorch, the desktop and
NVENC/NVDEC all within ±10 % of native, with one universal device configuration.
Tonight's focus: what can be tested and validated without the user.

## Setup

- **Linux B** ("desktop-b"):
  - device dev cb4974fc, compression=slice, FB 4096 MiB slot 0, 8 vCPU / 8 GiB;
  - wrapper `alpha-2026-09-27/desktop-checks/D6-dev-c049a59e/desktop-b-comp4g.sh`;
  - taken down at 21:31 for the latency A/B.
- **Lean compute VM** (vm2-615): the 2026-09-25 matrix configuration (`matrix-2026-09-25/env.sh wl615`), used for the latency A/B (`latency/lat-ab.sh`).
- **Native:** the host, same binaries (`/mnt/vmstore/caraxes/llama/dist`, `target/bench/bin`, the host venv).
- **Windows** ran in parallel (P23-P25, the Windows agent's run dirs), except for the native block.

## Results

First pass, before the fixes (Linux B, cb4974fc):

| Workload | native | Linux B, cb4974fc | ratio |
|---|---|---|---|
| short kernel, µs/launch (1000 × 1000 cycles, auto) | 6 | 40-48 | 7-8× (regression, below) |
| llama 1B Q4_K_M pp512, t/s | 7639 | 7084 ± 1667 (Windows booting) | 0.93 |
| llama 1B tg128, t/s | 305 | 242 | 0.79 |
| llama 7B Q4_K_M, -ngl 20, pp512 | 1085 | 914 | 0.84 |
| llama 7B -ngl 20, tg128 | 16.6 | 11.4 | 0.69 |
| llama 7B -ngl 99 (4.36 GiB > guest VRAM) | 1652 / 70.3 | clean "failed to load model", no Xid | expected |
| llama 1B with GGML_CUDA_ENABLE_UNIFIED_MEMORY=1 | n/a | abort; host Xid 31 FAULT_PDE (guest gap) | UVM, below |
| PyTorch transformer, tokens/s | 107485 | 105625 | 0.98 |
| PyTorch ResNet-18, images/s | 532.5 | crash, host Xid 31 FAULT_PDE (shadow covered) | race, below |
| NVENC 1080p H.264 P1 ULL, fps (median latency) | 166-180 (2.3-2.8 ms) | 89-101 (5.1-8.2 ms), lean VM | 0.55 |
| NVDEC 1080p H.264, fps | 141-143 | 78-79, lean VM | 0.55 |

After the fixes: dev 945431f9 (race fix c8f9ce08, set-B stamp 8d9ab89c, UVM off for Linux),
lean VM with 4 GiB on slot 0, CARAXES_LAUNCH_FAST=2, 0 Xid in every stage (`series945-fast2/`,
clocks in `clocks-series945.csv`):

| Workload | native | Linux, 945431f9 | ratio |
|---|---|---|---|
| short kernel, µs/launch | 6 | 13-14 | 2.2× |
| llama 1B pp512, t/s | 7639 | 8669 ± 777 | **1.13** |
| llama 1B tg128, t/s | 305 | 282 ± 35 | **0.92** |
| PyTorch transformer, tokens/s | 107485 | 110248 | **1.03** |
| PyTorch ResNet-18 ×3, images/s | 532.5 | 500 / 530 / 499 | **0.94-1.00** (was a crash) |
| NVENC fps | 165-180 | 95-125 | 0.6-0.7 |
| NVDEC fps | 137-143 | 77-94 | 0.6 |
| llama 1B with GGML_CUDA_ENABLE_UNIFIED_MEMORY=1 | n/a | clean "failed to load model", rc 1, no Xid (`uvm-off-945/`) | fail closed |

With UVM off, libcuda also reports "VMM: no" (no cuMemCreate/cuMemMap family). llama and PyTorch
run without it, but it is a compatibility gap to check.

The ENC/DEC rows of the first pass come from the lean VM (`venc-cur2/`). The probe was built on the host from
`caraxes/tools/nvenc-probe.c` with FFmpeg's nv-codec-headers (master tarball, fetched without
identifiers into `target/nvenc`); it needs GLIBC 2.38. Both sides PASS (PSNR min 63.37 dB).
The per-frame analysis runs as a separate research note (docs/research/video-path-2026-09-28.md).

## Findings

1. **Short-kernel regression.** Same lean VM, same configuration:
   - fbe4dfb (2026-09-25): 14-18 µs per launch;
   - cb4974fc: 40-48 µs.
   - The configuration is not the cause: Linux B's desktop on or off gives the same numbers.
   - `CARAXES_LAUNCH_TIMING=1`: the `plan` step of the shadow doorbell (mirror.rs `shadow_channel`: `flush_lazy` + `shadow_resync`) costs p50 20.6 µs on every doorbell. `CARAXES_ALLOC_LAZY=0` does not change it.
   - Cause (branch planfast-2026-09-28, 8d9ab89c): set B was recomputed twice on every shadow doorbell.
     - Each pass is 10.4 µs: a walk over all registry channels, 80 global extents and walk-cache scans for CUDA's 8 GR channels × 10 promoted globals.
     - 4a6ba83b made it per-doorbell, 72a8034c added the second pass.
   - Fix: set B is cached per root under a stamp (registry set-B generation, walk-cache generation, host-size count). A clean doorbell costs 0.1 µs instead of 20.8 µs (micro-benchmark `bench_set_b_clean_doorbell`).
   - Measured (`arm-planfast`, lean VM with 1 GiB on slot 5): **22-26 µs per launch**, plan p50 0.3 µs, ring to host doorbell p50 9.8 µs.
   - The rest of the gap to fbe4dfb is the untrapped-GPPut evidence reads (two uncached USERD read pairs, ~4.3 µs), which CARAXES_LAUNCH_FAST removes.
   - Evidence: `latency/arm-cur`, `arm-old`, `arm-cur-nolazy`.
2. **ResNet-18 crash: a shadow ordering race.**
   - Host Xid 31 FAULT_PDE at 0x7794_b5c03000 (GPC0 T1 write).
   - The device diagnostic shows the guest and the shadow covering the VA with the same 2 MiB leaf, so the kernel ran before the host mapping was in place.
   - Root cause (docs/research/shadow-fault-race-2026-09-28.md on branch shadowrace-2026-09-28):
     - Since 9ddc665c (gspFeatures UVM_ENABLED=1) libcuda maps through nvidia-uvm.
     - UVM's page-table updates reach the device only as MEM_OP TLB invalidates in the software executor, which never held the shadow.
     - The untrapped GPPut lets the host channel run the next kernel before the doorbell has planned the new regions (a ~3.4 ms window here).
   - Fix c8f9ce08: a software MEM_OP invalidate of a shadow root stops its push until the plan is applied. Plus the F3 TSG fault-record read.
   - Validated in `series945-fast2`: three fresh ResNet-18 processes at 499-530 images/s, 0 Xid.
3. **UVM demand paging.**
   - The guest has no mapping at the fault VA; UVM waits for a replayable fault that never reaches the guest.
   - Since 9ddc665c (2026-09-26) the device answers GSP_GET_FEATURES with UVM_ENABLED=1.
   - Research: docs/research/uvm-faults-2026-09-28.md. Not possible on Turing with an unmodified host driver.
   - The user decided (22:35): report UVM=0 for now, "the vGPU simply cannot do it" (as NVIDIA vGPU without UVM). Implemented in 945431f9.
   - A compatibility mode that pins managed memory in guest RAM is separate research (docs/research/uvm-guest-ram-2026-09-28.md). No guest patch for now.
4. **Fault containment holds.** After each Xid the guest got per-channel Xid 31 (host mirror), and CUDA worked again right after (nvprobe 0.056 ms).
5. **SotTR (Linux, run12-comp-4g).** The guest filled its VRAM to 3890 of 4096 MiB, and NVKMS refused the next GEM buffer (a still picture, sound on). A bigger profile only delays this; v1 solves it with its display balloon. This is a decision for the user.
6. **The host card stays in P8 while the guest works (the biggest remaining lever).**
   - Read-only `nvidia-smi` sampling during the NVENC/NVDEC probe, 500 ms interval (`clocks-*.csv`):
     - native: P2, video 1305 MHz, memory 6801 MHz, PCIe gen 3;
     - guest (lean VM, binary 945431f9): P8 in 308 of 322 samples, video 540 MHz, memory 405 MHz, PCIe gen 1.
   - Natively every CUDA context sends PERF_BOOST. The device acknowledges the guest's boost (0x20800a9a) and drops it (cuda.rs), so the host never raises the clocks for the VM's work. Analysis: docs/research/video-path-2026-09-28.md.
   - The user asked (22:30) for an activity-driven host clock boost "like the reactor". The design is a forwarded PERF_BOOST with a short duration, renewed while the VM rings.
   - The agent for it was first blocked by the auto-mode classifier ("Modify Shared Resources": host GPU clocks). The user then allowed it explicitly (22:45): "You are allowed to boost, and if possible integrate a dynamic boosting into the host process." Being built on branch hostboost-2026-09-28.
7. **Latency after the fixes** (combined branch night-2026-09-28), lean VM, µs per launch:
   - 945431f9 (UVM off for Linux): 20 without FAST, 14-17 with LAUNCH_FAST=1, 13-14 with LAUNCH_FAST=2. Native 6.
   - Ring to host doorbell p50: 9.5, 3.5 and 2.4 µs.
   - The rest (host doorbell to the next ring ~10 µs vs ~6 µs native) is on the host GPU side, which is probably also the P8 clocks.
8. **UVM:** 945431f9 answers GSP_GET_FEATURES bit 0 by the guest's NVGMC_SI_HOST_OS_TYPE: Unix 0 (cudaMallocManaged returns 801, as on vGPU without UVM), Windows 1 for now. The user thinks Windows does not need it; an A/B on Windows is planned.

## Stability gate (the user's priority from 23:00: one stable device for nova, the Linux open driver and Windows)

| Guest | Binary | Result | Evidence |
|---|---|---|---|
| Linux, NVIDIA open 615.71.09, desktop + compression + display, 1 GiB slot 4, FAST=2 | v615 945431f9 | modules, GNOME/Wayland, 1920x1080 display, vkcube Wayland/X11, glmark2, NVENC/NVDEC PASS (174.8 / 110.5 fps), vkcube beside the NVENC holder, Sunshine active; 0 guest Xid, 0 host Xid. vkcomp/cuwork rc=124 is their endless loop cut by the 40 s timeout, the same in runs 9-11 | `linuxb-night/smoke-graphics.txt` |
| Linux, compute series, 4 GiB slot 0 | v615 945431f9 | short kernel, llama, transformer, ResNet-18 ×3, ENC/DEC; 0 Xid | `series945-fast2/` |
| nova-core (Linux 7.2.7), pure emulation, 1 GiB | **v610** 945431f9 | 5 probes, 4 GSP unloads, unbind/bind ×2, rmmod/modprobe, nova-drm each time; 0 rejected or unanswered RPCs, no host NVRM line | `nova-945/` |
| Windows 11, 617.14 | P28 (win branch + night) | not run: the test image is broken by a half-applied Windows update (KB5129195, winload.efi 0xc0430001 at code integrity, without our device); repair boot M28 in progress | the Windows agent's run dirs |

- **One device, two builds:** nova's guest ABI 570 needs the 610 build (`--guest-abi: guest ABI 570 needs a 610 build`). The v615 build serves the 615 Linux guest and the 617 Windows guest. For "one universal device" the version choice has to move from compile time to run time (D14: per-version generated ABI, chosen by the guest).
- The nova VBIOS was regenerated: `synth-vbios.py --ext-sig 0x4E56 --appid 0x85 caraxes/reference/rtx2070/rom.bin` (the oracle passes). The file is not in git; it is at /mnt/vmstore/caraxes/night-2026-09-28/synth-vbios-nova.bin.

## Graphics (Linux, windowed Wayland 1280x720; native on the host's Wayland session at P0)

| Benchmark | native | Linux B 945431f9 (1 GiB, FAST=2) | ratio |
|---|---|---|---|
| vkmark score | 15973 / 15940 | 4141 / 4955 | 0.26-0.31 |
| glmark2 score | 12214 | 3332 | 0.27 |

- Frame times of 0.2 ms against 0.06 ms native: these tiny scenes measure per-frame submit and present overhead (the guest compositor, the virtual display, doorbells and fences), not GPU throughput.
- vkmark's headless winsys needs VK_EXT_headless_surface, which the NVIDIA driver lacks, so both sides run windowed.

## Host clock boost (merged into dev as 78d4b0fd; branch hostboost-2026-09-28, 0a068b0d)

The user approved it (22:45): "You are allowed to boost, and if possible integrate a dynamic boosting into the host process." Design: docs/research/host-boost-2026-09-28.md.
- A thread with its own unprivileged host RM client sends NV2080_CTRL_CMD_PERF_BOOST {BOOST_TO_MAX, 3 s}.
- It renews at most once per second while the VM rings or its host engines raise events, and never on the ring path.
- It never forwards the guest's CUDA bits or an infinite duration. The boost lapses 2-4 s after the last activity.
- Off switch for A/B: CARAXES_HOST_BOOST=0.

The A/B in the lean VM (1 GiB, slot 5, FAST=2), run while Windows P28 ran on slot 0 (`boost-ab.sh`, `boost-*-venc/`, `clocks-boost-*-venc.csv`):

| | native | boost on | boost off |
|---|---|---|---|
| NVENC 1080p fps | 165-180 | 129 / 145 / 148 | 127 / 110 / 103 |
| NVENC median latency | 2.3-2.8 ms | 2.85 ms | 3.6-6.6 ms |
| NVDEC fps | 137-143 | 109-112 | 82-86 |
| short kernel µs | 6 | 11-12 (21 cold) | 13-17 (36 once) |
| host P0 samples (100 ms) | - | 195 | 0 (P8/P5 only) |

- The device log shows 20 renewals (5 from guest requests), 0 refused; the RM call takes 7 ms mean, 45 ms max, off the ring path.
- 0 Xid in the Linux VM. The host Xid lines in the same windows are all from the Windows device (pid 1192852, FAULT_PRIV_VIOLATION on HUBCLIENT_FE, the F2 class), reported by the Windows agent.

## Graphics per frame (docs/research/vkmark-frame-2026-09-28.md) and the A/B of the trace and the pages

Analysis of the traced vkmark cube/clear (0.295 / 0.234 ms per frame against 0.056 / 0.064 ms native):
- The app's frame path traps 0 times and rings ~4.5 times per frame, untrapped.
- Every vblank handler holds the GPU completions for 0.6-0.75 ms: two display-page writes of ~150 µs and 6 TIME reads.
- Mutter's flip path costs ~3.7 ms per 60 Hz period, 2.3 ms of it in traps: 23 flush reads, 2 invalidates of ~430 µs.
- The steady loop is paced by interrupt round trips of ~50 µs each.
- ±10 % at 4000+ FPS is not reachable with mediated interrupts; for games at 60-150 FPS these costs are 1-4 % of a frame.

The A/B, each arm a fresh Linux B (1 GiB, slot 4, FAST=2, boost on, dev 7f1143be), vkmark ×2 and glmark2 at 1280x720 (`vkmark-ab.sh`, `vkmark-ab/`):

| Arm | vkmark | glmark2 | Note |
|---|---|---|---|
| trace on | 5339 / 5173 | 4620 | ran without Windows load |
| trace off | 5847 / 6101 | 5049 | +10 %; Windows P30 started during this arm |
| trace off + usermode, invalidate, kick pages | 2181 / 2103 | 2018 | −60 % |
| control: trace off (bisect window, Windows P30 workloads alongside) | 4920 / 4246 | 3254 | |
| + CARAXES_USERMODE_PAGE=1 | 3042 / 2529 | 1907 | −40 % against the control: its reactor scans every channel's GPPut after each doorbell and publishes TIME every iteration |
| + CARAXES_INVALIDATE_PAGE=1 | 4127 / 4721 | 3762 | neutral, but **unsafe**: "ERROR untrapped GPPut: host channel ran ahead of its doorbell with plans pending" (the hold only acts on the trapped invalidate; the same class as the 2026-09-28 race) |
| + CARAXES_DISPLAY_KICK_PAGE=1 | 5039 / 4873 | 4103 | +2-15 % vkmark, +26 % glmark2; 29550 kicks by the reactor, mean 117 µs, 0 inconsistencies |

Consequences for the one universal configuration:
- The device trace should be off by default: +10 % in these micro-benchmarks.
- The display kick page is a candidate for the default, after a re-measure without Windows load and a game run.
- The usermode page needs its reactor scan fixed first.
- The invalidate page needs the untrapped-GPPut hold before it can be used.

## Validation of the Windows branch's software-CE slicing on Linux (6c566092, one binary)

`validate-b-6c566092/`: Linux B (1 GiB, slot 4, compression, display), the binary built from win-2026-09-28 at 6c566092 (dev + one ABI + the Windows fixes, including B: the software CE in slices of ≤40 ms).
- Smoke all PASS: NVENC 165.4 fps, NVDEC 127.6 fps (boost on), glmark2 build 4324 FPS.
- Scrub stress: 30 × 300 MiB alloc/fill/free, each free scrubbed by the guest RM through the software CE as a 300 MiB memset. Total 28 s, worst loop 1.07 s; vkcube ran alongside without break (rc 0).
- 0 guest Xid, 0 host Xid.

## SotTR with evidence (Linux B, 4 GiB slot 0, dev ca2559f5 with boost and FAST=2, trace off; `sottr-bench.sh`, `sottr-ca2559f5/`)

The user asked (2026-09-28): "You can also test sottr but ensure to KNOW if the benchmark runs and the client displays pictures".

The tooling:
- Feral's `ExtraCommandLine=-benchmark` plus `GameOptionsDialogShouldShow=0` in the guest's preferences, so the game starts straight into the built-in benchmark. The guest's original preferences were restored afterwards; the benchmark variant is kept as `preferences.benchmark-2026-09-29`.
- Moonlight windowed on the host, and `grim` of its window every 30 s with the mean brightness and the RMSE against the previous shot.
- xwd of the game window in the guest.
- A guest monitor (VRAM, NVKMS refusals, Xids).
- The benchmark writes `SaveData/*benchmarkresults*.txt`, which gives a verifiable end.

What happened, in order (`timeline.txt`, `host-shots/`, `guest-shots/`, `guest-monitor.txt`):
- The Moonlight client showed the live guest desktop (Steam), then the game's loading screen; the guest's own screenshot of the game window matched.
- VRAM rose to 3884 of 4096 MiB (01:01-01:02 guest time).
- At 335 s: `Failed to allocate NVKMS memory for GEM object`. The host picture froze from then on (RMSE ~2 between shots) and GPU load fell to 0-5 %.
- The game exited about a minute later (VRAM back to 450 MiB). No crash dialog, no Xid, no results file.

- **The same failure as on 2026-09-28 at 3 and 4 GiB. It blocks SotTR on v2.**
- The native reference exists: 2026-08-20, 1080p exclusive fullscreen, VSync off, higher settings (textures/shadows/LOD 3), 91.7 FPS average, budget 5888 MB.
- v1 ran two 3 GiB guests of the game with zero NVKMS failures thanks to its display balloon.
- A v2 balloon design for the user's decision is being written (docs/research/display-balloon-2026-09-29.md).

## Device CE on Linux, the kick page as default (2026-09-29 ~03:00)

- **The device CE** (Windows branch 70614b41: validated software-CE pieces ≥64 KiB copied or filled by a device-owned host CE whose VA space maps only the VM's usable FB), `validate-devce-70614b41/`:
  - Linux B scrub stress 30 × 300 MiB in **12.4 s** (CPU slicing at 6c566092: 28 s), worst loop 0.81 s;
  - vkcube ran alongside;
  - device CE: 168 pushes, 9242 MiB, 277 ms busy, all NV_OK; 0 Xid.
  - On Windows (P34): 981 pushes, 29251 MiB, 1600 ms busy; ResNet-18 first step 98-121 → 62 s.
  - Log noise to fix: 20 × "ERROR bar1 pool … cannot move back … the page is the USERD of mirrored channel chid N … consistent, retried" under CUDA context churn. The case is handled, so the line should not be an ERROR.
- **The kick page, clean pair** (dev ca2559f5, fresh VMs, trace off; Windows P34 started during the kick arm):
  - control: vkmark 5026 / 4500, glmark2 3900;
  - kick page: 5687 / 5551, glmark2 4347 (+13-23 %, +11 %).
  - Together with the bisect this is the second positive A/B.
- **Kick page smoke plus stream** (70614b41 + kick, `kick-smoke-70614b41/`):
  - smoke ALL PASS: glmark2 build 5063 FPS, NVENC 178.4 fps, NVDEC 154.5 fps (boost on), Sunshine active;
  - Moonlight windowed on the host with three grim shots: means 0.19 / 0.14 / 0.14, RMSE 12670 / 1528 (the picture changes while vkcube runs); 0 Xid;
  - 4610 kicks, 0 inconsistencies.
- Two "ERROR untrapped GPPut: chid 3/6's host channel ran ahead of its doorbell … with plans pending (worker true)" lines, no fault.
  - The same detector lines were already in Linux B on 2026-09-28 at 20:48 with cb4974fc, without the kick page and without the device CE (reported by the Windows agent in P22). So this is a pre-existing detector condition; it is listed separately.
  - The entries the host channel fetched were written before the guest's invalidate, so they need none of the pending plan's mappings. The detector cannot tell that apart.
- **dev 4dde63e1:** the display kick page is on by default with the display page (`CARAXES_DISPLAY_KICK_PAGE=0` turns it off), after LAUNCH_FAST=2 (ca2559f5) and the host boost (78d4b0fd).

## Gate of win-2026-09-29 (8e621e0a) and its merge into dev (fast-forward to b2e0d749)

Contents: the device CE, the private-pages security fix (the device's own tables and rings, top 512 KiB of the reservation, are no longer reachable through the software CE, the shadow, trapped BAR1/BAR2, the BAR2 memfd or PRAMIN), and the async guest-RAM registration (DMA_MAP answered at once, pinning off the loop).

| Guest | Result | Evidence |
|---|---|---|
| nova (570 detected) | 5 probes, 9 unloads, 0 refused, 0 private-page refusals | `nova-8e621e0a/` |
| Linux B smoke + scrub (1 GiB, slot 4, compression, display, kick default) | ALL PASS, NVENC 164 / NVDEC 148 fps, scrub 30 × 300 MiB in 12.4 s, device CE 611 pushes / 10040 MiB / 351 ms, 4879 kicks, 0 inconsistencies, 0 private-page refusals, 0 Xid | `smoke-8e621e0a/` |
| Linux compute (lean VM, 4 GiB, defaults) | short kernel 11-13 µs; llama 1B 8037 / 282 (1.05 / 0.93 of native); transformer 109579 (1.02); ResNet-18 549 / 531 / 525 (0.99-1.03); NVENC 151-153 (~0.9); NVDEC 129-133 (~0.93); 0 Xid | `series-8e621e0a/` |
| Windows 11 (P36, the Windows agent) | smoke ALL PASS, 0 TDR, 0 ERROR, 0 private-page refusals, registrar "the shadow waited 0 times", device CE all NV_OK, teardown clean | the Windows run dirs, windows-findings.md |

dev after the fast-forward: 706 tests pass.

## SotTR at 720p (the user: "We don't want the balloon yet, first test on 720p that should be sufficient"; `sottr720-8e621e0a/`)

Linux B, 4 GiB slot 0, the gated dev code (8e621e0a: boost, FAST=2 and the kick page by default), trace off. `sottr-bench.sh` with `SOTTR_RES=1280x720` sets the resolution, `-benchmark` and no options dialog, and restores the preferences afterwards.

Timeline (guest time):
- 216 s: the loading screen was on the client, and VRAM reached 3883 MiB.
  - At 720p the game fills VRAM the same way: it sizes its texture pool by the budget, not the resolution.
  - `Failed to allocate NVKMS memory for GEM object`.
- Unlike at 1080p, the benchmark kept rendering (GPU 88 %), but the stream froze on the loading screen (RMSE 29 between shots).
- 279 s: **host Xid 109 CTX SWITCH TIMEOUT** on the game's GR channel (host channel 0x97; guest channels 65/66, delivered as per-channel RC).
- Then `vkWaitForFences failed: -4` (VK_ERROR_DEVICE_LOST), and the game aborted with signal 6.
  - Feral's dialog offered only "OK", so nothing was sent. The dump is set aside in `crashes-held-2026-09-29` so the next start does not offer to send it.
- The preferences were restored.

Result: 720p is not sufficient.
- The NVKMS refusal is independent of the resolution.
- A new stability finding: the game's GR context could not be switched out within the host's timeout, probably because long benchmark draws run without graphics preemption (GFXP) on the host context. It is being analysed separately.

## The vblank fix (branch vblank-2026-09-29, 95f1af1; docs/research/vblank-path-2026-09-29.md)

- Cause:
  - Every trapped display-page write (vblank and awaken acks) was treated as a sync point.
  - Each one ran the BAR1 pool's replica merge, two uncached 4 KiB page reads, ~110 µs, whenever the guest mapped a page twice.
  - That made three merges per latched vblank, and every GR completion waited for them.
- Fix:
  - display-page writes are no longer sync points; flush and invalidate still are;
  - the BAR2 memfd compare runs in place;
  - one trace flush per trapped write;
  - the delivery histograms are filled for trapped acks.
- A/B, fresh Linux B VMs, trace on, old 8e621e0a vs new 95f1af1, alternating (`vkmark-ab/vb-*`):

| Arm | vkmark | glmark2 |
|---|---|---|
| old 1 | 5148 / 4968 | 5078 |
| new 1 | 5132 / 5018 | 4841 |
| old 2 | 5802 / 5763 | 4925 |
| new 2 (Windows P40 started alongside) | 4950 / 5521 | 3902 |

- Replicas were present in all arms (9-11 "maps twice" lines). The new arms made ~84,800 display-page writes without a pool poll. Raise to vblank ack is now measured at p50 ≤ 511 µs, with 764-1636 of ~28,700 at ≥1 ms.
- The micro-benchmark scores do not move: their limit is the steady interrupt loop, not the vblank handler. The fix matters for frame pacing in games.
- 0 Xid, vkcube passes. It goes into the next combined gate rather than being judged on vkmark.

## The production candidate P40 (branch gfxp-2026-09-29, 9a3134ba) on Linux and nova

The production candidate, with no diagnostic patch:
- owned GFXP in the VM's slice (slots carved below the BAR1 pool, reported reserved, device-private);
- the GR preemption steps 0 and 1 (preempt-2026-09-29);
- the Windows pool-query answer.

- **nova:** 5 probes, 0 refused (`nova-p40/`).
- **Linux B** (1 GiB, slot 4; `smoke-p40/`):
  - smoke ALL PASS: glmark2 build 5243 FPS, NVENC 193.5 fps, NVDEC 161.1 fps, vkcube beside the NVENC holder, Sunshine active;
  - scrub 30 × 300 MiB in 12.1 s; 0 Xid; 0 private-page or slot-VA refusals.
  - Slots: 2 × 0x6a0000 = 14 MiB (1.4 % of the profile).
- **Compute parity works.** Desktop GR TSGs get "guest SET compute CTA (1) … bound" at build, and CTA again at teardown. The CUDA TSG (grpID 27) had no SET at build (host default gfxp 0, cilp 0 = WFI); its SET came later, the device idled the TSG and bound CILP, and teardown shows cilp 2.
- **GFXP did not bind on Linux:** "WARNING gfxp: tsg grpID N stays in graphics WFI: GFXP VA 0x1000000000000: 0x1a" for every graphics TSG. The host refuses the fixed VA 2^48 in the shadow VAS. The fix is with the Windows agent (slot VAs inside the host VAS range, outside anything the guest can map).
- **Windows P40:** the pool query is answered, and the next blocker is the display caps (Code 43 after 116 loops on 5070 control 0x50700117). P41 carries the captured TU106 TMO/ILUT/OLUT caps for every guest.

## P41, P42: Windows without the diagnostic patch, and the GFXP VA limit

- **P41** (gfxp-2026-09-29 f8a450b8 = P40 plus the captured TU106 display caps, productized for every guest; no patch): **the production binary runs Windows.**
  - D3D11 and smoke ALL PASS, 0 TDR; CUDA 53.7 µs, 12.9/13.1 GB/s; transformer 85854 tok/s; NVENC 99 / 132 fps.
  - Compute parity works on Windows too (SET CILP_SET compute CTA → bound after an idle).
  - GFXP stayed WFI (the 2^48 VA was refused).
- **P42** (the slot VA window at 2^47, shadow VAS 2^48): GFXP bound (step 0 read gfxp 1 at build). Then host **Xid 31 HUBCLIENT_SCC_NB FAULT_PDE VIRT_WRITE** on both devices, at the slot VAs with bit 47 dropped.
  - The context-switch unit keeps 47 VA bits, so GFXP buffer VAs must be below 2^47, inside the guest's range.
  - The Linux B smoke with P42 (`smoke-p42/`): 6 guest Xid 31 (glmark2, Xwayland, sunshine), vkcube timeouts, glmark2 1 FPS.
  - Reverted (1c581554).
- **Safe production state:** P41 (Windows without the patch, compute CTA/CILP parity, graphics WFI). P43 = P41 without the unused slot carve-out is the production candidate for the next gate.
- **Open design question for the user:** where GFXP buffers can live structurally guest-free below 2^47.
  - (a) The guest's own server-RM range [4 GiB, 4.5 GiB), which needs an RM-source check.
  - (d) Buffers allocated by the host RM in its server range: host VRAM outside the slice, ~6.6 MiB per GR context, capped per VM.

## P43 merged into dev (29d4a4dd)

- The Linux compute series on P43 (`series-p43/`, lean VM 4 GiB, defaults): short kernel 11 µs (one rep 112 µs); llama 1B 8580 / 289 (1.12 / 0.95); transformer 109393 (1.02); ResNet-18 550 / 531 / 519 (0.97-1.03); NVENC 146-166 (0.88-1.0); NVDEC 120-125 (~0.88); 0 Xid.
- With Windows P43 (all stages PASS), Linux B (`smoke-p43/`: ALL PASS, compute CTA 18 / CILP 5 bound, 0 REFUSED, 0 Xid) and nova (`nova-p43/`), P43 is gated on all three guest types. gfxp-2026-09-29 is merged into dev.

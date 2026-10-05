<!-- SPDX-License-Identifier: MIT -->
# Windows 3DMark investigation, 2026-10-04

- Goal: verify a complete Windows 3DMark workload, then reduce submission and completion overhead. Diagnose Steel Nomad separately when it faults.
- Caraxes baseline: `dev` at `dff996e3`. Worktree: `/mnt/vmstore/caraxes/wt-windows-3dmark`, branch `codex/windows-3dmark-2026-10-04`.
- Leandro v1 was reviewed first: clean `main` at `73eb298`. No v1 edits or VM starts.
- The original Caraxes checkout contains unrelated work in the screenshot tool and diagnostic work in `grcapture.rs`/`mirror.rs`. The diagnostic patch was preserved and incorporated only in this worktree. Original patch: `/mnt/vmstore/caraxes/windows-3dmark-2026-10-04/inherited-diagnostics.patch`.

## Evidence before changes

- Latest completed diagnostic run: `windows-plain/C134-gr-consumed-image`.
- Steel Nomad still causes host Xid 31: graphics GCC constant-cache virtual read at VA 0. This is not a completed performance baseline.
- Final macro 0 reads MME DMEM `0x57`/`0x56` and emits constant-buffer selector B/C (`0x2384`/`0x2388`). The trace does not establish the DMEM values or the pointer's origin.
- Capturing only direct `DMA_READ` methods misses macro-generated reads and `DWRITE` initialization.
- Private GPPut was published before capture. An active GPU channel could consume and release pushbuffer storage before it was recorded.
- Channel 58 migrated from 8192 to 65536 GPFIFO entries before the fault. The old `fwd_put` cursor was reused for capture of the new ring, skipping its prefix.
- Startup MME images can differ from the final image. The inherited diagnostic retained only 24 dwords per startup call, which can omit initialization instructions.

## Changes under validation

- Retain the installed startup macro image up to the next known macro start or first uncaptured word, within the existing 16K-word MME RAM bound.
- Snapshot direct DMA source payloads, at most 64 words each. These are CPU observations before publication; pending GPU producers can change them before DMA executes.
- Capture both private rings before publishing either sampled CPU submission. Each ring uses its own published cursor; publication reuses the captured snapshot.
- Mark fault cutoff uncertainty while a migration remains in flight. No production MME or constant-buffer initialization fix is justified yet.

## Lessons from v1

- V1 directly maps native host submission registers after setup. V2 can reduce mediation only after its shadow mappings and context state are ready.
- Preserve shared DMA aliases and explicit ownership. Never copy GPU-owned USERD fields into CPU submissions or vice versa.
- Measure interrupt/completion tails separately from host doorbell latency. V1's event delivery changes mattered substantially to desktop frame time.
- V1's historical 99% native offscreen glmark2 result excludes presentation and is not a Windows or 3DMark baseline.

## Validation and next steps

- Device tests: 863 passed, 1 ignored. Release binary `vfio-user-nvgpu-gr-before-publish-ff8a6bc5`, SHA256 `7725a9e9905671b4d5826fbc138f6a68cc0f781bdef9ccb3bf8c0b9c641b8258`.
- `CH_SCORED=1` selects a measurement configuration in `run44.py`: capture, profiling, launch timing, semaphore diagnostics, access trace and host-control recording off. `gpu-up.sh` accepts `WIN_GPU_TRACE=0` to omit the trace file. Diagnostic defaults remain unchanged.
- First performance A/B candidate: `CARAXES_INTR_DIRECT=1`. C118 measured raise-to-MSI p50 15.4 us and p99 4.53 ms; direct completion can remove a reactor wakeup, but delays awaiting guest ACK remain.
- Hardware testing: one host-backed Windows VM, host driver remains bound, existing RAM/VRAM/BAR1 floors enforced. Stop the backend on its first new host Xid.
- First establish exact startup/migration evidence and verify D3D12 compute/readback. Change GPU state only with a supported cause.
- Once Steel Nomad completes correctly: fixed workload, resolution, VRAM, vCPU count and driver; interleaved A/B runs with tracing, capture, profiling, semaphore diagnostics and launch histograms disabled.
- Record scores/FPS, frame-time tails, backend/guest CPU, traps, interrupts and host errors. A faster incomplete or incorrect render is not an improvement.

## C135 result

- `C135-gr-before-publication`: host-backed Windows booted; D3D12 compute gate exited 0 and verified all 256 readback words.
- The retained 3DMark input is `SteelNomadGt1LoopDXLT`, DX12, `gt1_light_stress.json`, windowed, 20 loops. It is a stress-test reproduction, not a normal score benchmark. The diagnostic used a 180-second process timeout.
- Steel Nomad reproduced GCC read VA 0 on channel 54. The watcher stopped the backend at its first observed Xid; three Xid 31 records arrived before teardown completed. Guest and backend are stopped; host FB returned to 809 MiB used / 6971 MiB free.
- Startup macro 27 received `[0x56, 0x7105000]` and `[0x57, 0]`. Windows supplied a nonzero CB address. The captured macro body has only two words, so its effect on MME DMEM remains unknown.
- 54 of 64 startup image lengths are incompatible with complete 96-bit groups. Raw instruction upload pointers and payloads must be retained before deciding pointer units or interpreting overwritten code.
- Added bounded raw uploads with their observed `SET_MME_VERSION`, plus recent root-table selector/data and visibility methods. Root tables are an additional constant-buffer path, separate from direct CB binds.
- `gr <chid>` written to the existing `device-ctl` file prints a retained capture snapshot. Requires `CARAXES_GR_CAPTURE=1`; no RC is needed. Use only in a diagnostic run.
- Added `tools/windows/d3d12-render.ps1`: offscreen root-CBV and descriptor-heap-CBV graphics, two colors and full pixel readback; hardware validation pending. This uses this repository's test conventions and verified Microsoft ABI declarations.
- No production GPU-state fix or performance improvement is claimed. The main checkout and Leandro v1 remain unchanged.

## C136 progress

- `C136-d3d12-cbv-raw`: stock Windows driver, host-backed RTX 2070, Cloud Hypervisor, 8 vCPUs / 8 GiB RAM / 4 GiB guest VRAM. Capture and profiling enabled; not a scored run.
- The graphics gate initially failed before submission because its descriptor-heap IID contained a typo. Fixed the IID and checked all interface IDs against Microsoft headers.
- The corrected gate exited 0: root CBV and descriptor-heap CBV each rendered two frames. All four readbacks contained exactly 1152 triangle pixels and 2944 clear pixels; updating the CB changed exactly 1152 pixels. No WARP or swapchain.
- Live desktop channel 22 raw uploads establish MME64 group units: pointer `0` / 30 dwords, `0xa` / 6, `0xc` / 9. Fixed capture normalization and bootstrap bounds to use three dwords per group. Raw pointers remain unchanged.
- Macro 27 is six dwords and writes `DMEM[LOAD0] = LOAD1`. C136 initialization supplies `0x56 = 0x14c50000`, `0x57 = 0`. The previous two-dword reconstruction was incomplete.
- Fixed C597 inference after an exact binding to another owned class. C5C0 Falcon methods and NV902D upload methods overlap graphics offsets and must not change retained C597 state.
- The ring migration keeps the existing host channel and TSG. C135 has no intervening channel free/rebuild; the trace does not support a context recreation as the cause.
- Latest device tests: 867 passed, 1 ignored; both integration tests passed. Release binary `vfio-user-nvgpu-mme64-a84fe706-cap`, SHA256 `70e7cc36acc3ce202d7fbdc0228e21f4e74ca3ef3e30289c22f31326fe3365de`.
- The normal 3DMark UI starts through Steam (`steam.exe -applaunch 223850`); direct execution showed a Steam connection error. Initial capability collection exceeded two 15-second timeouts before the UI appeared. This setup delay is separate from benchmark FPS.
- Scheduled test wrappers now hide their PowerShell windows to avoid obscuring the benchmark. Artifact UI helpers require the 3DMark window to own the foreground before clicking.
- First scored A/B: baseline versus direct completion only. Audit found coalesced exact-mode rearm writes currently count as one toward the 256-rearm host-hint storm guard; fix and validate that accounting before a rearm-eventfd A/B.

## MSI rearm accounting and installation checkpoint

- `900b4230` adds bounded `rearm_count(writes)`: one MSI decision per wake, actual coalesced write count for the host-hint storm guard, zero no-op, saturating counters. Trapped rearm remains count 1.
- Six tests cover batched withdrawal, threshold boundaries, real device vectors, zero/overflow, exact-mode direct completion ordering/races, and the exact eventfd callback while the Device mutex is held. Full suite: 873 passed, 1 ignored; both integration tests passed.
- Release binary `vfio-user-nvgpu-rearm-900b4230-cap`, SHA256 `fddde4fdf5c83815e9ffa5a97aa8448101431dfec88be86fa1436799c2ce5f8a`.
- C136 stopped cleanly after a D3D11 three-frame readback pass. `check-ch.sh`: all pass; no host Xid, no guest RC, no shadow collision/fence refusal. Host FB returned to 809 MiB used / 6971 MiB free.
- Fire Strike's files exist, but 3DMark reports the DLC as not installed. Installation was queued through its UI. Cloud Hypervisor's tap has no host NAT and DNS times out.
- `M137-3dmark-install` is a QEMU maintenance boot without the synthetic GPU, using the existing user-network NAT to finish Steam's installation. No host firewall/NAT changes. Windows updates paused until `2026-10-11T19:44:53Z`, automatic driver installation excluded, CBS and Windows Update have no pending reboot. Then return to Cloud Hypervisor for GPU measurements.
- No completed 3DMark score or measured speedup yet. Diagnostic corrections and successful graphics gates establish a narrower, reliable starting point.

## C138 Fire Strike baseline

- Steam completed the Fire Strike installation in M137. The maintenance VM was stopped before the host-backed Cloud Hypervisor boot.
- C138: same RTX 2070, stock 617.14 driver, 8 vCPUs / 8 GiB RAM / 4 GiB guest VRAM. Binary `900b4230` above; direct completion and rearm eventfd both off.
- `CH_SCORED=1` disables investigation features. Automatic scanout PNGs were manually disabled before measurement; later harness starts skip arming them in scored runs (`a89a67d9`). No GPU trace file.
- Fire Strike 1.1, preset P, 1920×1080, fullscreen, VSync off. Executed sets: GT1, GT2, Physics, Combined. The demo toggle was off. SystemInfo scan and monitoring were on.
- Completed local scores: overall **5888**, graphics **5590**, physics **11531**, combined **4413**. FPS: GT1 **19.665331**, GT2 **31.817068**, physics **36.609074**, combined **20.529837**.
- The UI warning says the score was not verified online. The guest has no Internet; this is not a failed workload. Do not present this as an online-validated score.
- D3D11 readback and root/heap-CBV D3D12 graphics gates passed before the benchmark. No host Xid during the run or clean shutdown. Trace-dependent health checks are unavailable in a scored run; `95cec5bc` reports them as SKIP and still rejects missing traces in diagnostic runs.
- Host samples include loading and unloading. GT1/GT2 mean GPU utilization was about 60%, peaks 97%/95%, boost up to 1860 MHz. Earlier low-utilization snapshots were primarily loading or Physics. These samples do not establish the render-only bottleneck.
- A separate host build was active before launch. No native Windows baseline exists. This single run establishes a reference, not a repeatable speedup.
- Artifacts: `/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/C138-3dmark-baseline/`. Result SHA256: `ddd20e3799bf5870338f1d222c76aee87ce620d81c2bfd0a103e0e580bf26f75`. Raw result archives contain machine/licensing metadata; publish only selected benchmark fields.
- Diagnostic fixes `41116bfa` / `151163c1` retain all 11 legacy COUNT_OLD bits and bounded raw provenance for unexpected graphics bindings. Full suite after these changes: **876 passed, 1 ignored; 2 integration tests passed**. C138 uses the unchanged `900b4230` binary for the flag A/B.
- C139 boots the same binary with direct completion alone. Next comparisons: direct completion, rearm eventfd, then monitoring on/off while preserving SystemInfo scanning. Read-only PMGR controls `0x2080a613`/`0x2080a612` accumulated about 41 s under the Device lock during the entire C138 session; this is a candidate, not yet render-only attribution.

## C139 and rendering checks

- C139 direct completion only completed all four Fire Strike workloads: overall 5941, graphics 5647; GT1 19.866365 FPS, GT2 32.135181 FPS. Compared with C138: about 1% higher graphics score, one sample each. This does not establish a repeatable gain.
- One host scanout snapshot was requested during GT1 but produced no image. The scored run has no trace explaining the refusal; record this probe as a measurement caveat.
- A separate DDA/NVENC recording started before another Fire Strike run. The encoder produced 1917 frames over 81.77 seconds, then DDA returned `0x887a0026` when the benchmark changed to exclusive fullscreen. This first clip is not proof of an animated 3D scene. The capture now restarts after display transitions and checks total recorded duration.
- That recording attempt's Fire Strike workloads still completed (overall 5890); this is a functional run with extra encode overhead, not a scored A/B sample.
- `f1a210b0` makes detailed Device-lock statistics optional (`CARAXES_DLOCK_STATS=0`), including software-part timers and per-piece aggregation. Default diagnostics stay on. Scored mode defaults off; an explicit `CH_DEV_EXTRA=CARAXES_DLOCK_STATS=1` permits a same-binary A/B. Validation: 881 device tests passed, 1 ignored; 2 integration tests passed. Release SHA256 `f79b419722611f4ec79eb1b340c1be592750c4ba81b943d234fd622064915182`.
- A later recording recovered from fullscreen transitions: six segments, 180.03 recorded seconds. GT1 shows changing camera positions and an advancing benchmark frame counter; GT2 also shows a rendered scene. Reviewed images show no obvious corruption, without a native pixel reference. Capture FPS is encoder delivery, not benchmark rendering FPS; transitions leave gaps.
- Motion artifacts: `/mnt/vmstore/caraxes/windows-3dmark-2026-10-04/FireStrike-motion-20s.mp4`, `FireStrike-rendered-gt1.mp4`, `FireStrike-rendered-gt2.mp4`, and `FireStrike-rendered-segments.json`. These are real guest DDA captures encoded with NVENC.
- That functional run completed all four workloads: overall 5685, graphics 5455; GT1 19.32626 / GT2 30.695971 FPS. Recording adds work, so exclude it from performance comparisons. C139 stopped cleanly with no host Xid; available health checks passed, trace-dependent checks skipped. Its full-session log contains 23 ERROR lines; this is not a claim of error-free operation.

## Windows adapter name and next measurement

- FriendlyName alone displays **Leandro VFIO RTX 2070** in both Device Manager and Task Manager on Windows 26200.8037 / stock NVIDIA 617.14. Screenshots: `C139-devmgr.png` and `C139-taskmgr-gpu.png` in the artifact directory above. DriverDesc remains NVIDIA GeForce RTX 2070; DXGI description has not been rechecked.
- `tools/windows/display-name.ps1` reports the exact present RTX 2070 and offers backed-up Apply/Restore operations without restarting the driver. The name is a guest property; it may need reapplying after driver installation or devnode recreation.
- Repeated Apply exposed a Windows PowerShell 5.1 atomic backup replacement error. `5ae5216f` uses an explicit recovery path, validates the published backup and preserves the previous copy on failure. C140 passed Apply, Apply, Restore, Restore, Apply in order; Restore recovered the original absent FriendlyName and DriverDesc stayed unchanged. The final name is Leandro VFIO RTX 2070.
- C140 uses the new `f1a210b0` binary with detailed lock statistics off, direct completion on, rearm eventfd off, compression off. D3D11 and root/heap-CBV D3D12 pixel gates passed. First Fire Strike: overall 5299, graphics 5064; GT1 19.749294 / GT2 24.884005 FPS. The warm repeat is recorded below. This first result is slower than C139; it does not isolate statistics overhead from run/boot variability or intervening source changes.
- Next GPU-side candidate: existing `CARAXES_COMPRESSION=slice`, retaining host coverage, ownership, ZBC and alignment checks. Qualify Windows rendering first, then compare with compression off using the same binary and other settings. Linux SOTTR follows Windows optimization.

## C140 result and diagnostic savings

- Warm repeat: overall 5497, graphics 5453, physics 6905, combined 4419; GT1 19.835596 / GT2 29.468491 FPS. Graphics improved from the first C140 sample but stays below C139; CPU Physics changed substantially. No repeatable statistics-off gain is established. Host memory clock 7000 MHz; graphics boost reached 1860 MHz. The available samples include load/unload periods.
- C140 stopped cleanly with no host Xid. Available health checks passed; RC/other trace-dependent checks remain unavailable. Full-session backend log: 21 ERROR lines.
- `17c9c834` skips successful legacy RPC FNV fingerprints and normal receive/control/send descriptions when the device has no trace sink. Failures, reply bytes/cache flags, host calls and queue/continuation ordering are preserved. With lock statistics off, RPC timing/key setup is also skipped. Standalone models retain diagnostics; an initially enabled sink still formats notes if the disk-space guard later stops it.
- Validation: 885 device tests passed, 1 ignored; both integration tests passed. Release `vfio-user-nvgpu-rpcdiag-17c9c834-cap`, SHA256 `385567ee7e542350c7ebcc2da3eb7dbe9b71c4f4e11b1ce039490214bdaa1bbe`. Graph update completed. GPU qualification of this new binary is pending.
- C141 compares compression slice against C140 using the unchanged `f1a210b0` binary and other GPU settings. Host granted physically based compression coverage for all 4096 MiB; D3D11 and both D3D12 CBV pixel gates passed. Animated-render and scored results are recorded below. Coverage does not prove actual compressed PTE use or compression ratio.

## Compatibility follow-up

- ZCULL: guest separate-buffer mode 2 is acknowledged but dropped; measured C139/C140 host mode values remain 1 (no context save/restore). The current comment claiming GLOBAL mode is inaccurate. This is a compatibility discrepancy, without measured FPS causality. The bounded translation needs embedded client/channel ownership, host-sized aligned buffer coverage/access/kind, synchronization and lifetime proof.
- Internal TU106 memsys reports 8 LTCs × 2 LTS while public/native reference reports 32 total LTS. Corrected to 8 × 4 in `2951830f`; the regression compares the serialized internal reply against the tracked native public capture. The full audit and source/trace references are in `/mnt/vmstore/caraxes/windows-3dmark-2026-10-04/zcull-capability-audit.md`.
- Windows BAR2 memfd stays off for the current comparison. C80 previously faulted on stale PTEs; the documented synchronization fix must be checked against current re-plan paths before another qualification. Its large trap count alone is not sufficient to enable it.

## External RTX 2070 orientation

- Public valid Fire Strike 1.1 results for desktop RTX 2070: [22409 graphics points](https://www.3dmark.com/fs/31401213) at 1845 MHz peak / 1750 MHz memory; [23663 graphics points](https://www.3dmark.com/fs/30795799) at 1860 MHz peak / 1750 MHz memory. These are independent systems, not a controlled native reference for this host.
- C139's 5647 graphics points are about 24–25% of those examples. The current Windows graphics gap is large; direct completion alone did not explain it.
- A [24650-point result](https://www.3dmark.com/compare/fs/18768236/fs/17442968) reports GT1 117.91 / GT2 98.23 FPS, with higher core and memory clocks (2010 / 2001 MHz); treat it as an overclocked example, not a stock target.
- Compare the graphics score for optimization; the overall score also includes CPU Physics and Combined. User priority: visible, correct motion and stable completion first. A later provisional goal of 18000–21000 graphics points is an optimization target, not a prediction or a native-speed claim.


## C141 compression result and shared fixes

- Same `f1a210b0` binary as C140, direct completion on, rearm eventfd off, lock statistics off; only compression changed to `slice`. Fire Strike completed without recording: overall **5974**, graphics **5700**, physics **11063**, combined **4496**; GT1 **19.926304** / GT2 **32.772884** FPS. Benchmark interval: 23:38:23–23:42:23 CEST. Result SHA256 `40c0f14293e2745e3b5b37c27964b1c6f2dfbff99599249fa6ea66294a5dbcd2`.
- This exceeds C140's 5064/5453 graphics scores, but remains close to earlier 5590/5647 results. One compression sample plus changing boot/CPU conditions does not establish a repeatable gain. GT1 barely changes.
- Separate functional recording: six segments, 180.27 recorded seconds; actual GT1/GT2 scene frames reviewed. Camera positions and frame counters advance, with no obvious corruption in the sampled images. Recording run: overall 5527, graphics 5198; exclude it from performance comparisons. Files: `C141-firestrike-motion-1.mp4`, `C141-firestrike-motion-3.mp4`, and `C141-firestrike-motion-segments.json` in the artifact directory.
- Host compression grant covers all 4096 MiB. All 107 attempted-audit summaries report unavailable, zero sampled PTEs, and compressible maps on; the host refuses the test-only PTE readback with `0x7e`. Zero violation counters therefore do not establish an audited invariant. Guest kind histograms include compressible kind 9 but do not measure accepted host maps or compression ratio.
- C141 stopped cleanly, no observed host Xid. Available health checks passed; trace-dependent checks skipped. Full-session backend log contains 21 ERROR lines.
- `aea21d77` fixes private USERD publication with BAR2 memfd: identify pending channels before scanning; preserve the existing hand-over; sample both migration rings, synchronize data/replicas and flush Store, then publish exactly those samples. Later guest GPPuts remain pending. Idle polls avoid scans and BAR2-off behavior retains its existing path. New PTE mappings still need the existing invalidate/plan barriers.
- `2951830f` corrects internal TU106 topology to eight active LTCs × four LTS, matching the native 615 capture's 32 total active LTS and LTC mask `0xff`. This is a compatibility fix, without an FPS claim.
- Validation after both fixes: **888 device tests passed, 1 ignored; 2 integration tests passed**. Includes Store visibility before either ring publication, late GPPut/data, owed flushes, migration cutoffs, idle polls, and serialized memsys counts against native capture.
- Hardware qualification next: new binary with BAR2 memfd off, then a bounded memfd test with `CARAXES_BAR2_FLUSH_PAGE=trap`. Deferred flush is now always enabled; the old `CARAXES_BAR2_MEMFD_DEFER_FLUSH` argument is removed and must not be used as a test control. Do not untrap the flush page for this qualification. Linux SOTTR follows Windows.

- Release after both shared fixes: `vfio-user-nvgpu-bar2lts-aea21d77-cap`, SHA256 `177c299fc1d5572467d3a176f2f17fbee9b81880f2e65d693e4c11ac223b8f55`. Also includes `17c9c834` RPC diagnostic savings. Graph update completed: 38436 nodes / 67343 edges. C142 starts with BAR2 memfd off and the C141 compression/direct/rearm settings for initial hardware qualification.


## C142 baseline for the BAR2 comparison

- New `aea21d77` binary, BAR2 memfd off, otherwise C141's direct/compression/scored settings. D3D11 and both D3D12 CBV pixel gates passed; stock Windows 617.14 reports adapter status OK. Interactive desktop and 3DMark UI also verified.
- Fire Strike: overall **5979**, graphics **5726**, physics **10669**, combined **4503**; GT1 **19.939253** / GT2 **33.133926** FPS. Interval: 23:59:41 CEST Oct 4 to 00:03:33 Oct 5. Result SHA256 `9b7debb9a4c3aa47bb544d18eac846aaf4e12c5f32e7f81b90fafad3d5c69863`. No recording/profiling. This is effectively unchanged from C141's 5700 graphics, not evidence of a significant RPC/topology speedup.
- Clean shutdown, no observed host Xid; available checks passed, trace-dependent checks skipped. Full-session log: 19 ERROR lines. The main checkout's original WIP is unchanged.
- A process's `MainWindowHandle=0` from a noninteractive SSH session is insufficient to diagnose a missing desktop window. Interactive `upack` UI checks and screenshots are authoritative for this harness; an SSH readiness loop unnecessarily delayed navigation.
- C143 uses the same binary with only BAR2 memfd enabled and its flush page trapped. Hardware qualification is pending. Keep it opt-in until pixel tests and a moving scene complete without host faults.
- Further read-only preparation: `/mnt/vmstore/caraxes/windows-3dmark-2026-10-04/posted-mmio-audit.md` and `linux-sottr-readiness.md`. Current CH still acknowledges every trapped store; mapped reads/eventfds/private USERDs can overtake queued NoReply stores, so a header-only posting change is unsafe. Linux's earlier 34.7 FPS SOTTR gate used patched display-reserve modules despite a stock kernel; use a fresh stock-module boot for vanilla qualification.


## C143 private BAR2 publication qualification

- Same `aea21d77` binary as C142; only BAR2 memfd enabled, with the flush page still trapped. D3D11 passed three hardware-readback frames; both D3D12 CBV paths passed both frames and changed pixels after a constant-buffer update.
- D3D11 device creation fell from 4145 to 508 ms; resource creation plus frames from 297 to 17 ms. D3D12 creation fell from about 3002 to 586 ms. These are one pair of startup measurements, not a throughput benchmark.
- Pure Fire Strike: overall **5929**, graphics **5678**, physics **10969**, combined **4370**; GT1 **19.874176** / GT2 **32.585064** FPS. Interval: 00:20:27–00:24:12 CEST, Oct 5. Result SHA256 `d978bcc57c5be72730a6ede94d5038a7ca3bc45d62907bb5a7b2b9195aea59f8`. Graphics is effectively unchanged from C142's 5726. No recording or profiling during this run.
- Separate functional recording: eight segments, 180.27 seconds. Sampled GT1 and GT2 images show rendered geometry, advancing counters and changing camera positions. No obvious corruption in those images; no native pixel reference. Artifacts include `C143-firestrike-motion-1.mp4`, `C143-firestrike-motion-3.mp4` and `C143-rendered-segment*-frame*.png` in the artifact directory. The recorded run's 5563 graphics score is excluded from the A/B.
- Clean shutdown, no observed host Xid; available health checks passed, trace-dependent checks skipped. Full-session backend log: 21 ERROR lines. BAR2 memfd is qualified for these bounded Windows gates, not all games or DX12 stress.
- Whole-session BAR2 summary: 570255 syncs, 20.59 s total, 104864218 page comparisons, 9164 dirty pages, 420779 bytes written back, zero alias conflicts. BAR3 trapped accesses fell to one read. These totals include setup, both benchmarks and idle time; they do not identify render-only cost.
- Interrupt totals remain large: about 429000 acknowledged raises and 894000 rearm MSIs. Doorbell-to-host mean 184 us, p99 2750 us; Device-lock wait per wake mean 91 us, p99 3000 us. Measure render intervals before attributing the FPS gap.
- `0248a365` starts the Xid watcher at the recorded VM-start uptime, covering early boot before the watch process starts. Synthetic cases and shell checks passed. C143 used the earlier watcher plus an explicit initial check from VM start.
- Read-only memory audit confirms the physical VRAM slice is GPU-cacheable; shadow mappings inherit that policy. The private fence compares memfd RAM with CPU snapshots, not the entire VRAM slice. Sysmem caching remains conservative; resource residency and coherence need measurement before changing it. Details: `graphics-memory-performance-audit.md` in the artifact directory.
- C143 is stopped. Next: a separate Windows render profile with the same GPU settings, followed by bounded rearm-eventfd qualification if the measurements support it. Linux SOTTR remains next; isolated stock-guest overlays and a launcher are prepared under `linux-stock-oct05`, with no Linux GPU boot yet.


## C144 render-phase attribution

- Separate diagnostic run: same `aea21d77` GPU settings as C143, profile and detailed lock statistics on, normal GSP trace on; GR/access capture off. D3D11 and both D3D12 CBV pixel gates passed. Automatic scanout dumps disabled before measurement.
- Passive GDI screenshots require no foreground/window change or encoder. At 00:40:01.734 CEST GT1 shows frame 448 / time 25.96; at 00:41:01.752 GT2 shows frame 924 / time 30.65. These support approximate render starts at 00:39:35.77 and 00:40:31.10. Images: `C144-phase-060.png` and `C144-phase-120.png` in the artifact directory. Rendering and camera motion are visible.
- Profile dumps are cumulative; interval summaries subtract the same backend's device scope. Routed lock-free costs live in separate window counters and must be paired with access counts. Lifetime maxima cannot recover interval percentiles. Host/control/RPC/access timing overlaps and must not be added as exclusive time.
- GT1 interval 00:39:47.5–00:40:02.7: about 448 calls each of `0x2080a613/612`, 10.09 s in control handling versus 3.08 s inside the two host ioctls. BAR2 syncs took 284 ms; posted-to-rung 73 ms and rung work 185 ms. GT2 interval 00:40:33.1–00:40:48.3 shows the same monitoring pattern. Diagnostics re-enable successful RPC formatting/fingerprinting, so these costs cannot be assigned unchanged to scored runs.
- GT1 host samples near the screenshot: GPU utilization about 86%, boost 1845 MHz, memory 7000 MHz. Dmon memory utilization 6–7%; PCIe traffic was tens to hundreds of MB/s. These counters do not measure resource residency, peak bandwidth, exclusive shader time or a native-equivalent GPU workload.
- All Fire Strike workloads completed: graphics **5058**, GT1 **17.895412** / GT2 **28.527464** FPS. This instrumented run is excluded from scored comparisons. Result SHA256 `b95aebb7561418126cb6bffc26c0b71bc1e4e82493a33884b030dd23f4e40990`.
- C144 stopped cleanly. Available checks passed, no observed host Xid, zero guest RC notes, zero fence collision/refusal; capture-dependent GPFIFO coverage skipped. Full-session backend log: 19 ERROR lines.
- Safe interval helpers and source notes: `profile-interval.py`, `render-phase-stills.ps1`, `render-profile-guide.md`, `C144-GT1-9-10.md` and `C144-GT2-12-13.md` in the artifact directory. The result extractor now records the harness's default trace-on setting correctly for diagnostic runs.
- Next Windows experiment: qualify rearm eventfd with the same binary, BAR2/compression/direct settings and diagnostics off. Keep the Device mutex, ownership checks and private publication fence intact. Linux SOTTR follows this Windows comparison.


## C145 rearm eventfd qualification

- Same `aea21d77` binary and scored C143 settings, with only MSI rearm eventfd enabled. Cloud Hypervisor registered BAR0 `0x88704`, length 4, no datamatch. D3D11 and both D3D12 CBV pixel gates passed; Windows uses stock 617.14.
- Pure Fire Strike: overall **6003**, graphics **5724**, physics **11342**, combined **4479**; GT1 **19.883417** / GT2 **33.268246** FPS. Interval: 00:58:07.945–01:01:50.284 CEST, Oct 5. Result SHA256 `077edf4010c554ba4066725e8ca8562d29c648e7d5a92dd9e2e5c87a4f4cfc15`. No recording/profiling. Compared with C143's 5678, this single sample does not establish a repeatable performance gain.
- Separate NVENC motion qualification: six segments, 180.50 recorded seconds. Reviewed GT1 frames show different camera positions and advancing counters; GT2 shows rendered geometry. No obvious corruption in these samples, without a native pixel reference. Recorded graphics score 5569 is excluded from comparisons. Files: `C145-firestrike-motion-1.mp4`, `C145-firestrike-motion-3.mp4`, `C145-firestrike-motion-segments.json` and `C145-rendered-segment*-frame*.png` in the artifact directory.
- Clean shutdown, no observed host Xid; available checks passed, trace-dependent checks skipped. Full-session backend log: 21 ERROR lines. Host RC events at exit: zero. This qualifies the rearm flag for these bounded gates, not all Windows workloads.
- Whole-session counters include setup, two benchmarks and idle: 617455 rearm notifications, 58838 MSIs at rearm, 567930 BAR2 syncs / 19.69 s, 8393 dirty pages, 396145 writeback bytes, zero alias conflicts. Raw totals cannot isolate render cost or be compared as rates across different session lengths.
- Corrected source comments in `eeeaca23`: shadow GPU cache policy inherits allocation; guest kind override is not restricted to uncompressed memory; measured ZCULL mode is NO_CTXSW and can still cull within a context. Runtime unchanged.
- Windows throughput remains around 5.7k graphics. The existing legacy cache option needs Windows-specific validation; dynamic PMGR samples remain uncached. Linux SOTTR follows Windows. Identical owned EGL/Vulkan probes are prepared for native versus stock-guest attribution, with no claim of native graphics parity.


## C146 draw timing and cache eligibility

- Same scored C145 binary/settings. Stock Windows 617.14 reports device status OK / ConfigManagerErrorCode 0. The original D3D11 gate passed; no Fire Strike score was taken.
- Owned `d3d11-timing.ps1` extends our triangle/readback probe with timestamp-disjoint queries, strict GPU identity, bounded completion polling and alternating full-image clear/draw validation. Compile-only ABI/math checks and all three hardware cases passed, eight measured samples each. Final script SHA256 `5ddda2e8794db33d87611c15813aa8bd6bc38b08b9a238c0eac25b52c00c6be1`; artifact guide and status files are in the existing artifact directory.
- Median GPU draw intervals: 256²/1 draw **1.888 us**, 256²/64 draws **21.264 us**, 1024²/64 draws **303.120 us**. The last case scales about 14.25× for 16× the pixels. CPU Flush medians: 177/108/216 us; nonblocking Map completion: 72/106/880 us. Full-image CPU validation is reported separately and takes 12.7 ms at 1024²; it is not GPU render cost.
- First probe version used `Thread.Sleep(1)` and reported 30.5–31.3 ms Map waits. Replacing sleeps with bounded spin/yield changed the 256²/1 median to 72 us. Those older waits are polling-influenced and cannot establish driver/emulator latency. Both versions and their exact outputs are archived.
- Native host owned Vulkan probe passed identical image gates and timestamp checks: median render scopes 5.184/27.184/350.000 us for the same dimensions/counts. Vulkan includes clear/layout transitions and has different geometry/API; this is an attribution contrast, not an equivalent workload or game-throughput ratio. Clocks were not locked or sampled inside these short scopes. Simple draw scaling does not establish shader, vertex, texture, depth or full GPU capacity.
- Clean C146 shutdown, no observed host Xid; available checks passed, trace-dependent checks skipped. Full-session backend log: 15 ERROR lines.
- Cache audit: C144 GPU8159 has 14 successful calls with 14 distinct request/reply fingerprints. Removed it from experimental cache eligibility; normal forwarding remains. Unknown byte semantics do not justify reusing the first response. Nine measured candidates remain opt-in and off by default; original 610 sampling is not a guarantee for future inputs/guest versions.
- A612 has 6520 byte-identical 54352-byte replies to all-zero requests in C144, but no authoritative public payload contract. No new cache mode was added. The guest's GPU+command cache would bypass validation of later different inputs or larger schemas. Details: `legacy-cacheable-windows-audit.md`.
- Cache-fix validation: 10 focused NVML tests, all **889 device tests passed / 1 ignored**, and both integration tests passed. The running hardware binary remains `aea21d77`; its cache option is off. Main-checkout WIP is unchanged.
- Next Windows attribution: controlled depth overlap with front/back order, pipeline statistics and a depth-disabled color control; shader/vertex width and measured residency follow. Design: `windows-next-attribution.md`. Stock Linux comparison and SOTTR follow the completed Windows gates.


## Stock Linux attribution and SOTTR limit

- `linux-stock-oct05`: isolated root/games overlays, Cloud Hypervisor, 8 vCPUs / 12 GiB RAM / 4 GiB VRAM; same `aea21d77` backend with BAR2 memfd, trapped flush, compression slice, direct completion and rearm eventfd on. No concurrent Windows VM. Guest kernel 6.8.0-136-generic and stock open NVIDIA 615.71.09; installed/loaded NVIDIA and modeset source versions match. The patched display-reserve parameter is absent, `caraxes_dma_range` is absent, registered guest RAM works without a guest helper. Stock `nvidia_drm` reports modeset on / fbdev off.
- Owned identical EGL binary, three 15-second headless repetitions before GNOME/Sunshine: native **16120.9 / 16074.8 / 16075.7 FPS**, guest **8659.6 / 8866.9 / 8277.2 FPS**. The shader uses 64 sine/cosine iterations, one synchronous finish and pixel readback per frame. This CPU/wall measurement checks GL errors and alpha; it is not a complete RGB oracle, GPU-only throughput or SOTTR prediction.
- Identical owned Vulkan timing binary, eight samples per case with NVIDIA hardware identity and full-image gates: render medians native/guest **5.184/5.568 us** (256²/1 draw), **27.184/27.200 us** (256²/64), **350.000/349.952 us** (1024²/64). Render plus copy is also nearly equal. CPU submit rises from roughly 2.6–2.9 to 6.1–7.6 us; fence waits rise by about 58–76 us. This simple workload supports investigating submission/completion overhead; it does not establish complex game or full GPU capacity. Clocks were not locked or sampled inside these scopes.
- GNOME runs a Wayland session. Sunshine KMS capture / guest NVENC HEVC streams 1080p60 to host Moonlight. A 600-frame `vkcube-wayland` run exited 0; reviewed frames at 23:48:20/24 UTC show different cube orientations. This is a bounded streaming gate, not simultaneous multi-guest encode/decode qualification or a desktop frame-latency benchmark.
- Native Feral SOTTR first exited before Vulkan initialization because Steam was not ready. Added temporary routing/DNS and three exact guest-IP/interface-scoped host forwarding/NAT rules. The second start passed Steam initialization and showed the game's temple loading screen. Requested 720p and minimum numeric graphics leaf values; no fresh result exists to verify the actual applied preset/settings.
- At guest uptime **1575.169706**, the stock DRM driver reports **Failed to allocate NVKMS memory for GEM object**; GNOME reports **Error getting buffer**. Stopped on this first failure. No benchmark completed and no moving benchmark scene was qualified. Earlier desktop stills do not establish a frozen stream; the later loading image differs.
- Guest memory snapshot during stopping: 4096 MiB total / 3461 used / 430 free. The physical host slice had been allocated successfully; aggregate guest free memory does not prove a suitable contiguous scanout allocation existed at failure. Exact allocation size/status is missing, so fragmentation, exhaustion, fallback or another KAPI allocation failure remains to be distinguished. Do not treat the patched Sept 29 34.7 FPS result as vanilla qualification.
- Game TERM did not finish within the bounded stop wait; killed only its owned scope, then requested guest shutdown. Original SOTTR preferences restored byte-for-byte, SHA256 `a821e2dff0b8b00aca977e2303e60855dca579875d4a42ca6827ac002034f8e6`. VM/backend and Moonlight stopped; the three owned host rules and temporary tap were removed. No observed host Xid. Full-session backend ERROR notes remain, including BAR1 alias replicas; this is not an error-free-operation claim.
- Artifacts: `linux-stock-oct05/`, `owned-graphics-attribution/` and `owned-vk-timing/` under the existing artifact directory. Windows remains first: depth/order controls next, then heavier shader/vertex/resource-residency attribution. Stock-only Linux SOTTR needs a later allocation fix or a qualified resource/configuration change.


## C147 depth qualification and recording cost

- Same scored C145 binary/settings, stock 617.14; device status OK / error code 0. Owned depth probe SHA256 `5bbf319251346893b9407ed83804baf9421634a7eadae116b6687a76e3b25e0e`; compile/ABI checks and all nine selected hardware cases passed, eight measured samples each. Every case first passes four full-image depth/order controls; pipeline statistics are a separate untimed pass.
- At 1024²/64 instanced layers: nearest-first LESS/write median **77.296 us**, 294912 PS invocations; farthest-first **339.888 us**, 18874368 invocations. Depth-disabled orders are **304.384/303.920 us**, also 18874368 invocations. The one-layer depth baseline is **7.664 us**. Early depth rejection works in this owned batch. These results do not measure hierarchical-Z retention across contexts or explain Fire Strike by themselves.
- C144's `CARAXES_RECORD_HOST` is inside the recorded control timing. The old encoder formats every request/reply byte with a temporary string before trimming zero tails, including all-zero large requests. Replaced it with zero-byte-tail scanning and direct hexadecimal output, preserving the exact table format. This reduces diagnostic work; scored runs already disable recording. No production FPS gain is claimed.
- Encoder tests preserve empty/all-zero data, both nibbles including a low zero nibble, all 256 final-byte values, and a 54352-byte zero request through the actual parser. Validation: 11 focused recorded tests; **890 device tests passed / 1 ignored**, both integration tests passed. Graph update: 38445 nodes / 67353 edges. Hardware binary is still `aea21d77`.
- Topology audit decodes the actual C144 replies: three active GPCs, 18 TPCs and 36 SMs, with all shared SM-order fields matching the native capture. No reduced topology or partition mismatch was established. Unsupported internal PPC masks remain a compatibility uncertainty; per-draw physical SM occupancy is unmeasured. Details: `windows-shader-width-audit.md` in the artifact directory.


## C147 shader attribution and measurement limits

- Five of six selected heavy integer-PS cases completed before the session bound, each with four depth/order controls, two independent full-image seed gates and eight measured samples. Script SHA256 `bf5cf27a8e306ccc97ab8faf3112483fad3a4230e02a82fc397c94073eae45d0`. One nearest-first instance, depth disabled; runtime constant-buffer loop counts prevent compile-time unrolling/elimination. Pipeline statistics are outside timing.
- GPU medians: 256² loops 0/64/256 **3.520/5.808/12.368 us**; 1024² loops 64/256 **28.688/100.800 us**. Actual PS invocations are 18432/294912. The 1024² loops 0 case remains pending. This qualifies integer arithmetic, not FP32 capacity, texture bandwidth or a game workload.
- The monitoring-off Fire Strike attempt finished GT1 at 20.00717 FPS, then was canceled. A foreground UI snapshot likely stole focus from the full-screen workload; the UI reports user cancellation. Overall/graphics scores are zero and this run is excluded from comparisons. Avoid all foreground/UI automation during measured full-screen tests. Raw result SHA256 `16d2dc9c8368f67b00f799715d763ff428b2462dca4140e2e28f83d3250dbe04`. SystemInfo scanning remained enabled; hardware monitoring was restored, confirmed in `C147-systeminfo-monitor-restored.png`. A complete monitoring A/B remains pending.
- Clean C147 stop at 02:33 CEST, no observed host Xid, 18 backend ERROR notes. The general harness checker failed because `d3d-probe-status.json` was not generated; dedicated depth/heavy probes passed, but do not describe this as all harness checks passing. Main-checkout WIP remains unchanged.
- Recorder microbenchmark pins old `5f879135` and new `30f629a3`, with synthetic inputs only. All seven cases pass exact output equality and independent decode; three alternating paired repetitions per case. The 54352-byte zero request drops from 2900.105 to 27.338 us (**106.083x**). Dense/sparse 54 KiB and 128 KiB outputs improve **21.746–28.607x**. This measures encoder CPU cost, not complete control handling or production FPS; scored runs already disable recording. Provenance/results: `hex-trim-microbench/` in the artifact directory.
- First matching native instanced/ALU probes pass ten image/timestamp gates, but repeated CPU reads from the selected uncached readback mapping take 0.50–0.62 s at 1024². Between short draws the GPU falls to 420 MHz/memory 405 MHz; native draw medians are therefore unsuitable for a throughput ratio. Fast native 1024² integer draws are 28.768/100.352 us for 64/256 loops, close to Windows's measured scopes. No arithmetic-capacity deficit is established. Preserve this initial packet and repeat with explicit cached host readback memory; clocks remain unlocked and API/compiler/timestamp differences still apply.
- Next: finish native attribution, repeat monitoring-off Fire Strike without foreground interference, then qualify owned dependent texture loads. Linux SOTTR follows Windows; its stock NVKMS allocation failure remains unresolved.

## C148 completed attribution

- Same scored `aea21d77` backend/settings, stock Windows 617.14; device status OK / error code 0. Standard three-frame D3D11 pixel gate passed. Device creation took 477 ms, resource creation 17 ms.
- Complete Fire Strike with hardware monitoring off, SystemInfo scanning on, demo off: overall **6063**, graphics **5740**, physics **12109**, combined **4574**; GT1 **20.050035** / GT2 **33.055027** FPS. Interval: 02:53:43.927–02:56:53.768 CEST, Oct 5. Result SHA256 `be2e28c334a1641c45328da8c91a4f2f14fd81868258178342ea2b72a4268ebf`. No foreground automation, profiling or recording during rendering. Graphics differs by only **0.28%** from C145's monitoring-on 5724; this single pair establishes no convincing gain. Monitoring was restored, confirmed in `C148-systeminfo-monitor-restored.png`.
- Finished the missing 1024² zero-loop integer-PS baseline: **8.176 us**, 294912 PS invocations, all image/query gates passed. This completes the six heavy-shader cases across C147/C148.
- Owned dependent-texture probe: eight cases passed two independent full-image seed gates, depth/order controls, separately measured pipeline statistics and eight timed samples. SHA256 `19c1dd5d5ceb1edacd949cfdece5b858fcfa1dc82ca7484d5e84df06e9260c12`. R32_UINT, one nearest depth-tested instance; runtime loop count and loaded data control subsequent addresses. Allocation size is addressable capacity, not measured residency, unique bytes touched or peak bandwidth.

| Size | 4 MiB / 1 load | 4 MiB / 16 loads | 64 MiB / 1 load | 64 MiB / 16 loads |
| --- | ---: | ---: | ---: | ---: |
| 256² | 8.880 us | 26.624 us | 8.160 us | 123.392 us |
| 1024² | 40.960 us | 232.384 us | 131.072 us | 1805.264 us |

- Owned tessellation probe: six cases passed complete color/coverage gates, queries and separate actual HS/DS statistics. SHA256 `9f64f27cf1b630be731143e8fb844a14fcd8ec48bc5265bf3312814307d39b3c`. One three-control-point patch; factors 1/8/32 produce 3/90/1243 DS invocations and 1/96/1536 clipping primitives. GPU medians: 256² **3.696/4.064/4.976 us**, 1024² **8.320/9.472/10.304 us**. This proves this path works; a single patch does not measure dense-geometry throughput or explain Fire Strike.
- The display-name tool found `Leandro VFIO RTX 2070` already present; Apply preserved it. DriverDesc/CIM Name remain NVIDIA GeForce RTX 2070, as expected. No new branding change is claimed.
- Clean C148 stop at 03:05:56 CEST, no observed host Xid. Available general checks passed; trace-dependent checks skipped. Full-session backend: 18 ERROR notes. Summaries and exact outputs: `C148-texture-heavy-summary.json`, `C148-tessellation-summary.json`, and `windows-plain/C148-texture-monitoring/` in the existing artifact locations.

## Native comparison and next profile

- Repeated the ten owned flat/ALU cases with explicitly cached/coherent host readback memory, removing the earlier slow CPU reads. All pixel/timestamp gates passed; no observed host Xid. Minimal native medians, 256²/1024² and one/64 instances: **2.304/21.984/7.744/304.320 us**. Heavy integer loops 0/64/256: 256² **3.056/4.368/10.688 us**, 1024² **6.880/25.136/87.536 us**. Native samples report 1410 MHz for minimal, 1620 MHz for heavy, memory 7000 MHz; clocks were unlocked and sampled outside the short draw scopes.
- Native texture matrix passed all eight cases, including actual A/B/A uploads, independent full-image gates, depth writes and readback freshness barriers. Source SHA256 `266c38e7dff8630111451fccb0dc5cff0abadbfecc8f8be3519186da04fdb239`; binary SHA256 `74da82aa41a5ab4ef8f78003b7a655fe3fa272e0c588742eaff462a53e729932`. GPU medians in the table order above: 256² **7.216/62.912/8.208/123.696 us**; 1024² **59.648/894.400/132.448/1806.560 us**. The 64 MiB results are close to Windows; the 4 MiB/16-load native path is substantially slower. API, shader compilation, cache behavior and unlocked clocks prevent a general throughput/parity claim. No fourfold basic arithmetic or raster deficit was established.
- Preserve the initial uncached packet under `owned-vk-instanced/native/initial-uncached/`. Corrected native packets and environment manifests live under `owned-vk-instanced/`; Windows seed/pixel gates do not establish complex-game stability.
- ZCULL review confirms the dropped separate-buffer bind, without performance causality. A one-time mapping check cannot protect an active save buffer through shadow remaps, coalesced unmaps or teardown. Translation requires persistent mapping leases, channel/TSG/VAS generation checks and group-wide quiescence. No setter shortcut was applied. Details: `zcull-retention-critical-review.md`.
- C149 is a bounded profile-only diagnostic: same backend/GPU behavior, tracing, recording and automatic scanout dumps off; PROFILE on. Its score is excluded from scored comparisons. Use same-PID cumulative intervals and passive GDI phase images without foreground automation to measure control/host costs after removing C144's recorder confound. Windows optimization remains first; stock Linux SOTTR follows.

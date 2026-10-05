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

## C149 without the recorder confound

- Saved launch configuration confirms PROFILE on, trace/recording/automatic dumps off, same `aea21d77` binary and GPU settings. Standard D3D11 three-frame pixel gate passed. Private artifact launcher preserves the normal launcher; its two optional controls suppress recorder injection and auto dumps.
- Passive GT1 images at 03:20:12.608 and 03:20:27.595 CEST show changed camera positions and counters, frames 165→448 / time 8.77→23.77 s. GT2 at 03:21:12.599 shows frame 679 / time 15.74 s. Estimated render starts: GT1 03:20:03.83, GT2 03:20:56.86. Phase images support approximate interior intervals, without native pixel-reference validation.
- GT1 snapshots 17→18, 03:20:13.899–03:20:29.100: **513 calls each** of A612/A613, **3.514 s** enclosing control time versus **3.493 s** nested host time. GT2 20→21, 03:20:59.503–03:21:14.705: **541 calls each**, **2.911 s** control versus **2.751 s** nested host. Most of this cost survives disabling recording and occurs inside host RM. These groups overlap and must not be added as exclusive stalls.
- BAR2 synchronization costs 251.5 / 347.8 ms in those intervals. Posted-to-rung costs 78.8 / 136.7 ms; rung handling 110.9 / 240.6 ms. Host samples during GT1: GPU utilization 94–96%, graphics 1860 MHz; GT2 86–94%, 1845 MHz. Memory 7000 MHz. Utilization does not measure exclusive shader work or prove the cause of the FPS gap.
- GT1, GT2 and Physics completed at **19.594521 / 32.720882 / 34.423313 FPS**. Combined is absent; overall score is zero. Exclude the incomplete run from score comparisons. Result interval 03:19:24.641–03:22:26.303 CEST; SHA256 `1b6bbd5c192202a4be082f824bc0c3a28e6e6c73fe803dc4e35c34da45c528ad`. A second interactive capture task began at 03:22:24.542, just before termination; a focus effect is plausible but unconfirmed. Stage all interactive tasks before full-screen rendering. SystemInfo processes can coexist with rendering; their presence must not classify a phase as scanning.
- C149 stopped cleanly at 03:27:29, no observed host Xid, 19 backend ERROR notes. Original checker failed solely because this diagnostic intentionally omitted the trace. Updated it to honor explicit `WIN_GPU_TRACE=0` in saved launch metadata, reporting trace checks as SKIP. Archived C148/C149 checks pass; a diagnostic fixture requesting trace still fails without it. Preserve `checks-original.txt` and the original session log; this correction does not supply missing trace evidence.
- Off-lock host calls need a real deferred-RPC state machine and an ordered owner of the existing RM client, including L2 flushes and large-message continuations. Taking the client away would silently skip other uses; RPC ioeventfd receive credits need separate treatment. NVIDIA also holds its GPU-group lock during these calls, so moving the user-space ioctl does not remove all contention. No asynchronous forwarding change was applied. Review: `host-control-offlock-critical-plan.md`.
- Artifacts: `C149-GT1-17-18.md`, `C149-GT2-20-21.md`, `C149-profile-attribution-owned-summary.md` and corresponding JSON; first passive image packet `C149-phase-*`, second `C149B-phase-*`. Later intervals remain unclassified unless phase evidence supports them.

## C150 reproduced depth-rejection gap

- Added the owned matched native depth derivative outside the repository. Same triangle, layer colors/Z, one instanced draw, D32 LESS/write, two orders, clear policy and full-image freshness checks. All four depth/order controls, separate actual pipeline-statistics pass and eight timed samples passed for both native arms. Vulkan 1.2 shader compilation/SPIR-V validation, C warnings-as-errors, CPU self-test and 17 invalid CLI checks passed. Native source SHA256 `55d5afe0a1b3fd1ae64abd71223a775e65b423257fc1f5ea4dc2e190d0aed46a`; binary `64d27484b127c4ed23437be514c7b5c5223cc721cacd04f08cad9330f80fdbfa`.
- At 1024²/64 layers, native nearest-first median **35.280 us** (34.816–36.256), farthest-first **338.928 us** (338.048–339.744). IA vertices/primitives and VS invocations are 192/64/192; fragment invocations 294912 / 18874368. Native after-arm clocks 1410 / 7000 MHz, outside timing; no observed host Xid.
- Fresh scored Windows C150 repeats the unchanged C147 depth probe: nearest **77.472 us** (76.192–77.696), farthest **339.904 us** (338.944–339.968), with identical IA/VS/PS counts and all image/query gates passed. C147 was 77.296 / 339.888 us. Windows after-test clocks also 1410 / 7000 MHz, outside timing.
- The nearest-first gap repeats at about **2.20x** while the farthest-first path is nearly equal. This narrows the investigation to this rejection-sensitive workload. Equal PS counts do not distinguish coarse ZCULL from other early-depth mechanisms; APIs/compiler/pacing and in-scope clocks remain uncontrolled. It does not establish context-retention causality or explain Fire Strike by itself.
- C150 stopped cleanly, no observed host Xid. General D3D11 pixel gate and available checks passed; trace-dependent checks skipped. Full-session backend: 15 ERROR notes. Packets: `owned-vk-instanced/depth/native/`, `C150-depth-matrix-summary.json` and dedicated status files in the artifact directory.
- Next: bounded direct ZCULL/depth-method diagnostics on the existing capture path, preserving exact class binding and capture cutoff; no GPU-state setters or pushbuffer rewriting. Windows optimization remains first. Vanilla Linux SOTTR still needs its NVKMS allocation failure resolved.

## Bounded depth-state diagnostics

- Opt-in graphics capture now retains 96 recent direct observations across 27 C597 depth/ZCULL, clear and draw methods. Each records capture sequence, GP index, subchannel and the accepted SET_OBJECT provenance at submission; another owned class cannot be interpreted by its matching numeric offset. Cutoff filtering uses capture sequence, including wrapped GP indices.
- Reports distinguish exact and inferred C597 bindings, count whole-capture evictions and state that macro-generated values and effective GPU state remain unknown. Existing dumps require a shadowed channel. Capture remains disabled in scored runs; no pushbuffer, register or host-control behavior changes.
- Validation: 12 focused capture tests; **893 device tests passed / 1 ignored**, both integration tests passed; release build passed. The three new regressions cover rebinding/class exclusions, bounded history with wrapped GP indices, and inferred/unbound provenance.
- C151 will use a separate non-scored capture run, recording/profile/automatic scanout dumps disabled. An artifact-only variant of the unchanged depth probe adds a bounded pause after all draw/image/query gates, before releasing its own resources. This permits channel inspection; its timings are excluded from scored comparisons.

## C151: requested and actual ZCULL modes

- Separate diagnostic binary SHA256 `26239d3d13aff2daf87fd4c7de1566fae6819ac2f2a72bbd3ca5f2a60d4eed60`, source commit `c9014a70`. Capture enabled, trace enabled, recording/profile/automatic scanout dumps disabled; other qualified GPU settings preserved. Both 1024²/64-layer depth orders and an earlier initialization capture passed all four image controls, statistics and completion gates. No scored timing comparison.
- Probe channel 44 is identified separately for each guest client. Windows requests ZCULL BIND mode **2**, VA `0xbf000`; the control is acknowledged without host translation. Host GET independently reports **1** at build, after compute-mode binding and teardown. Cleanup requests mode 1 / VA zero. This confirms a compatibility discrepancy, not its performance impact.
- The late nearest/farthest direct method/value sequences match: 96 retained observations each, 421 total / 325 evicted. They include ZF32, region 1152 × 1024 × 1, 1536 aliquots, sixteen static subregions and Z/RGBA clears. These are submitted configuration, not proof of backing, effective GPU state or retention across context switches.
- An earlier pause after the first validated control retains 44 direct observations without eviction. It captures initial depth-test/write zeros, yet image/statistics gates verify working depth rejection. Later effective state cannot be reconstructed from direct methods alone; macro-generated values remain unknown. All captured C597 classifications are inferred, without a retained exact SET_OBJECT.
- C151 stopped cleanly at 03:57 CEST; available harness checks passed, no observed host Xid, 15 backend ERROR notes. Detailed probe-only scalars and bind evidence: `C151-probe-depth-scalar-summary.json` / `C151-probe-zcull-bind-evidence.txt` in the artifact directory. Earlier controller aggregate observation counts include desktop channels and must not be attributed to the probe.
- Next discriminator: an unchanged-probe nearest-first instance sweep at 1/16/64 layers. Safe ZCULL translation needs validated owned-channel identity, buffer sizing and mapping lifetime; a mode setter alone is insufficient. Windows optimization remains first; Linux SOTTR follows.
- Fixed malformed ZCULL BIND handling separately: require the SDK's 24-byte payload and documented modes 0–2. Short/oversized payloads and invalid modes return INVALID_ARGUMENT rather than a successful zero-filled diagnostic. Valid payload echoes, embedded-client handling and host translation behavior are preserved. Regression checks all short lengths, an oversized request, invalid modes, 64-bit VA and padding preservation, with generic host forwarding forbidden. Full validation: **894 device tests passed / 1 ignored**, both integration tests and release build passed. This is input validation, not ZCULL save/restore support.

## C152: depth rejection versus instance count

- Scored diagnostic flags all off; separate binary from `7ae5a130`, SHA256 `d9be5b0bb9d7b9e4d2c081f42a1adca35a9849bcea8eeadff58a13e99922e6af`. Stock Windows 617.14 on host 615.71.09. Unchanged Windows/native probes, no helper, no clock changes. Native sweep completed under the shared slot-A lock before Windows started. Each arm passed four image controls, actual separate statistics, two warmups and eight timing samples.

| Nearest-first instances, 1024² | Windows guest D3D11 median us | Native Linux Vulkan median us | Observed ratio |
| --- | ---: | ---: | ---: |
| 1 | 7.088 | 6.448 | 1.099 |
| 16 | 22.736 | 13.056 | 1.741 |
| 64 | 77.216 | 35.136 | 2.198 |

- IA vertices/VS invocations are 3N, primitives N, selected PS/FS invocations 294912 throughout. Increment per added instance: Windows **1.043/1.135 us**, native **0.441/0.460 us** for 1→16 / 16→64. These are finite differences within each measured pipeline, not a general linear performance model. Windows CPU issue medians remain 15.95/15.80/13.65 us, separate from GPU timestamps.
- The difference grows with submitted instances rather than staying at a fixed entry cost. It still does not distinguish coarse-Z behavior, context retention, primitive processing or compiler/driver state. Windows 617 D3D11 versus Linux 615 Vulkan is not a same-OS native control; per-draw clocks are unmeasured. No specific emulator defect or Fire Strike improvement is proved by these ratios.
- Detailed packets: `C152-depth-sweep/` and `owned-vk-instanced/depth/native/depth-sweep/`. Native returned to its 6937 MiB free baseline after every arm. Windows D3D11 three-frame gate passed, create 506 ms / render 21 ms. C152 stopped cleanly at 04:15 CEST; available checks passed, trace-dependent checks skipped, no observed host Xid, 15 backend ERROR notes.

## Verified cleanup and MME shadow diagnostics

- Fixed optional owned-GFXP failure cleanup: require successful idle when scheduling may have occurred, BIND to WFI and GET confirmation on the exact live channel before releasing backing. Unknown results retain the channel, USERD/pages, TSG, remaining buffers, slot and exclusions. Missing channel/TSG cannot stand in for successful detach. Partial release keeps its ledger for retry.
- A failed build remains teardown-only even after later buffer cleanup succeeds. Channel readiness, USERD polling, doorbells, scheduling-enable and deferred promotion respect this quarantine. Successful ordinary engine-object cleanup restores prior readiness. Earlier GFXP builds are absorbed before another member of their TSG schedules.
- GFXP remains disabled; no new VA scheme or ZCULL translation was enabled. Six regressions exercise the shared cleanup/readiness gates with injected idle, BIND, GET and release failures. Hardware has not qualified the optional path.
- Added raw `SET_MME_SHADOW_RAM_CONTROL` (0x0124) to the existing 96-event capture history: 28 scalar methods. Chronology, class binding and capture cutoff remain explicit. TRACK/PASSTHROUGH/REPLAY observations do not reconstruct tracked values or effective GPU state. No command rewriting.
- Validation: 11 GFXP-related and 13 capture tests passed; **901 device tests passed / 1 ignored**, both integration tests and release build passed. AST graph refresh completed: 38493 nodes / 67495 edges; existing header extraction limitations remain.
- Commits: `d75714d9` (diagnostics), `86a31e85` (cleanup). C153 uses a separate immutable binary, SHA256 `14d39849df55dcfebc48fb5dfd07b1aedce8cffad261e3a23235824f7ab69179`, capture/trace enabled and recording/profile/automatic scanout dumps disabled. Timings from this run are excluded from scored comparisons.


## C153: initialization shadow control

- Fresh Windows boot and three-frame D3D11 pixel gate passed: create 512 ms / render 21 ms. The diagnostic depth probe passed four full-image controls, actual IA/VS/PS statistics and timestamp completion. Its timing is not scored evidence.
- Probe channel 44 / client `c1d000ac` retains 46 observations without eviction through capture sequence 87. At sequence 17, direct MME shadow control is 0 (TRACK), followed by initial depth-test/write zeros and function 1, then shadow control 1 (TRACK_WITH_FILTER). All C597 bindings are inferred; tracked values, macro-emitted methods and effective GPU state remain unknown. This closes an observation gap, not the depth-performance diagnosis.
- C153 stopped cleanly at 04:36 CEST. Available harness checks passed, no observed host Xid or guest RC note; 15 backend ERROR notes. Probe-only evidence: `C153-probe-depth-scalar-summary.json`. Aggregate capture count includes desktop channels. A manifest-name preflight refusal occurred before any VM started and was retained separately.


## C154: same Vulkan code and shaders on Windows

- Ported the owned native probe to Windows outside the repository. Only the monotonic CPU clock and Windows include/macros differ; Vulkan commands, SPIR-V, image/statistics gates and timestamp scopes are unchanged. Cross-build with warnings-as-errors, POSIX CPU self-test and Windows CPU self-test passed. PE SHA256 `6d7749009e7550a29005976b8c4cf529f67dd7806839d59c634753b5942a73ae`; no guest helper or host package installation.
- Scored diagnostics off; backend `86a31e85` / SHA256 `14d39849...`. The 256² qualification and 1024² instance sweep passed all four full-image controls, actual separate statistics and eight measured samples per arm. Windows Vulkan exposes `Leandro VFIO RTX 2070-4G`, hardware 10de:1f02, NVIDIA driver 617.14. This is Vulkan's reported name, not a new Device Manager/Task Manager check.

| Nearest-first instances, 1024² | Windows guest Vulkan median us | Native Linux Vulkan median us |
| --- | ---: | ---: |
| 1 | 6.448 | 6.448 |
| 16 | 13.696 | 13.056 |
| 64 | 35.056 | 35.136 |

- IA/VS are 3N, primitives N, FS 294912. Readback memory is cached/coherent/host-visible, type 4 / heap 1 on both observed platforms. Windows finite differences are 0.483/0.445 us per added instance. Same-session unchanged D3D11 N64 repeats at **76.832 us**, with matching image and IA/VS/PS gates. This substantially weakens a general emulator-wide depth-rejection slowdown hypothesis. API/compiler/state, OS and driver differences still remain; no native Windows D3D11 control exists.
- These microtests do not establish Fire Strike performance, native game speed or the cause of D3D11's difference. The last complete scored Fire Strike graphics result remains **5740**. Next diagnostic compares submitted Vulkan versus D3D11 state and BIND requests without rewriting commands.
- C154 stopped cleanly; available harness checks passed, trace checks skipped, no observed host Xid, 15 backend ERROR notes. Packets: `C154-vk-depth/`, `C154-d3d64-control-summary.json`, `owned-vk-instanced/depth/windows-port/` and its independent read-only review.


## C155: Vulkan submission state and the same ZCULL discrepancy

- Unscored trace/capture run, same C153 backend. Artifact-only Vulkan derivative pauses 30 s after the first fenced full-image control; remaining controls, statistics and eight samples still passed. All diagnostic timing is excluded. Held PE SHA256 `2a1dbeac9b84060738f6d0a3ef83e3503040b61e3d01a76a53873d2cf751f9a4`; GPU commands/shaders unchanged.
- Graphics channel 48 / client `c1d000cf` / channel handle `ff04002d` is scoped separately. It retains 53 observations without eviction, with an exact C597 SET_OBJECT at sequence 15. Later direct shadow modes include PASSTHROUGH/TRACK_WITH_FILTER/REPLAY/TRACK_WITH_FILTER; depth-write 1, function 0x201 and depth-test 1 are visible. Tracked and macro-generated values remain unknown; this is not a reconstructed effective-state comparison with D3D11's inferred initialization capture.
- Vulkan also requests ZCULL BIND mode 2, VA `0x1e6000`, acknowledged without host translation. Its owned host reports mode 1 at build, after compute binding and teardown. Mode 1 / VA zero is requested at cleanup. Thus the BIND/GET discrepancy exists alongside C154's near-native Vulkan GPU timing; it is a compatibility gap, insufficient alone to explain D3D11's timing difference.
- The first capture controller refused 33 total live channels before taking GR snapshots; its actual probe completed successfully. Preserved that attempt and repeated with snapshots limited to at most 16 newly created channels, plus a 64-total guard. Five new channels were inspected; the unrelated new channel 43 must not be counted as probe graphics state.
- C155 stopped cleanly at 05:01 CEST; available harness checks passed, no observed host Xid or guest RC note, 15 backend ERROR notes. Evidence: `C155-probe-depth-scalar-summary.json`, scoped BIND/host mode files and preserved first-attempt packet. No ZCULL override, new mapping exception or command rewrite was applied.

## Stock Linux: 5 GiB SOTTR allocation diagnostic

- Separate root/games overlays on the stopped stock packet; unchanged backend SHA256 `177c299f...`, stock kernel 6.8.0-136 / open driver 615.71.09, 12 GiB RAM, eight vCPUs and a 5120 MiB GPU reservation. GPA registration, compression slice, BAR2 memfd and direct/rearm completion preserved. No guest helper or patched NVIDIA module. GNOME Wayland and Sunshine HEVC/NVENC to the host Moonlight hardware Vulkan decoder produced a visible desktop.
- The first game launch exited because SteamAPI was not ready. The second reached the temple loading screen, but no completed benchmark or moving gameplay scene was qualified. A UUID-derived 5G graphics profile was prepared with requested 720p/zero numeric quality settings. The game rewrote these to 1080p, AA 2, texture/shadow/LOD 2 and Defaults Preset 3. These numbers do not establish a named preset; this is neither a qualified lowest-settings run nor an isolated VRAM-size comparison.
- Stock kprobes in an owned trace instance captured Xwayland TID 2702 requesting **8,847,360 bytes**, flags 0, block-linear 1, compressible 0, at guest uptime 560.343105. Its paired return is **-22** at 560.346586; the NVKMS GEM allocation error appears at 560.465775. The first subsequent memory snapshot reports total/reserved/used/free **5120/209/4871/41 MiB**; later snapshots show 4908/5 and 4910/3 MiB used/free. Memory pressure is observed. Free bytes at the precise failed allocation, fragmentation and allocator overhead remain unmeasured; no allocator defect is proved.
- The failed pair's qualification snapshot reports 106 entry and 106 return hits, zero probe misses. All eight CPU buffers report zero overruns/drops. The final profile filter incorrectly expected group names; the kernel lists event names alone, so its empty output must not be treated as final zero misses. Preserve the earlier qualification evidence and fix the filter before reuse. Kprobes record the DRM handler result, not the underlying RM allocation status. Interface reference: [kernel kprobe event documentation](https://docs.kernel.org/trace/kprobetrace.html).
- Stopped the owned game scope, removed the two probes and trace instance, restored the original preferences byte-for-byte (`a821e2df...`), stopped streaming/VM/backend and removed the three exact owned forwarding/NAT rules and temporary tap. Host VRAM returned to **6937 MiB free**, with no observed host Xid; 11 backend ERROR notes remain. The initial comment-extension firewall error occurred before VM startup and was preserved; the corrected launcher removed only rules it had added.
- Packet: `linux-stock-5g-oct05/stop-summary.json`, `guest-evidence-private.tar`, settings/profile observations and decoded-window captures. Next Linux step: qualify effective settings after first device initialization, then repeat in a fresh bounded session. Windows remains first: ordinary D3D11 fully occluded draws with depth writes off/on are being prepared outside the repository to narrow the API-dependent cost. No new Fire Strike improvement is established.

## C156: fully occluded D3D11 draws

- Same scored GPU settings/backend as C154, Windows 617.14, recording/profile/capture/trace off. Owned artifact source SHA256 `68b95f6b...` adds a completed, independently validated seed at depth .05 before one hidden instanced draw. LESS depth writes off/on are ordinary API states; seed, validation and selected state bindings precede timestamps.
- Independent review caught a false-positive oracle: a broken write-zero state could reject everything and pass the hidden test. Added two fresh depth=1 positive write-zero controls, both orders, requiring last-layer images and exact PS counts. All six full-image/statistics controls pass for every GPU arm; separate hidden counters are IA/VS 3N, primitives N, **PS zero**. Two warmups and eight timestamped samples per arm pass. A CPU-only child-wrapper exit-code bug was corrected and its failed attempt retained before GPU execution.

| Fully hidden instances, 1024² | Depth writes off, median us | Depth writes on, median us |
| --- | ---: | ---: |
| 1 | 3.616 | 3.712 |
| 16 | 20.576 | 20.592 |
| 64 | 74.720 | 75.296 |

- The 256²/N1 qualification is 1.920 us. These results retain substantial instance-dependent cost with zero PS invocations; changing the write mask has little observed effect. There is no matched native Windows D3D11 baseline, so this does not establish an emulator defect, rejection stage or Fire Strike cause. A matched preseeded Vulkan derivative is being prepared; C154's existing Vulkan workload must not be substituted for this changed workload. Last complete Fire Strike graphics remains **5740**.
- C156 stopped cleanly with no observed host Xid and host VRAM back to **6937 MiB free**; 15 backend ERROR notes. The harness checker reports a failure because this session omitted its separate `probe start/status` three-frame fixture. The seven owned GPU arms passed their gates, but they do not retroactively supply that missing fixture; record the harness limitation and run the standard fixture in the next Windows session.
- Packet: `C156-d3d-occlusion/`, `owned-d3d-occlusion/` with source/schema/review/runtime hashes, controller and bounded child wrapper. Individual shaders are exported and hashed; actual GPU machine code and in-scope clocks remain unmeasured. Before/after clocks are 1410/7000 MHz outside timing. No emulator GPU-state change was applied.


## Compatibility correction: sparse GPC masks

- Commit `c3f6e0bf` separates physical TPC/ZCULL/PES queries from logical TPC-count indices. The shared TPC-to-PES map is copied from the first enabled physical GPC, including when physical GPC 0 is disabled. Current TU106 mask `0x7` is unchanged; this is a portability fix, not a measured performance improvement.
- Regression covers sparse `0b101`, dense `0x7`, bit 31, mixed high bits, all bits and zero across 610/615 host and guest layouts. Independent source review passed; full device suite **902 passed / 1 ignored**, both integration tests passed, release build passed. Graphify AST refreshed; existing vendor-header extraction warnings remain. Further capability-response issues are recorded in `graphics-capability-readonly-audit.md`, with no demonstrated link to current D3D11 timing.

## Stock Linux: options UI follow-up

- A separate stock 5G overlay reproduced the Feral launcher through GNOME Wayland, Xwayland and Sunshine HEVC/NVENC. Loading actual post-initialization preferences avoids fabricating specification-seen flags. The launcher exposes resolution/window mode; its advanced tab exposes renderer/command-line controls, not game graphics quality.
- The display tab showed 1080p and listed 720p, but the owned 600 s launcher timeout ended the session during selection. Readbacks remained at 1080p/AA2/texture-shadow-LOD2. No reduced-settings persistence, gameplay or benchmark is qualified. Numeric profile Preset0 still does not establish a named preset.
- Original preferences restored byte-for-byte to `a821e2df...`; game/Steam/VM/backend/Moonlight exited, temporary tap/rules removed and host VRAM returned to **6937 MiB free**. No observed NVKMS allocation error or host Xid in this UI-only attempt. `linux-stock-5g-ui-oct05/ui-stop-summary.json` retains the qualification limit and private archive identity. One first crop caught a host-window transition and is explicitly excluded; later inspected guest crops are scoped to Moonlight.
- The stock allocator audit found no demonstrated allocation defect and no current emulator hook for a selective scanout reserve. Guest RM/PMA manages those allocations. A separate multi-region largest-free offset diagnostic inconsistency is untested and not linked to the observed failure. Linux SOTTR remains after Windows optimization; the next Linux run must qualify actual in-game quality and persistence before benchmarking.


## C157: matched fully hidden Vulkan and area controls

- Root built one owned C/SPIR-V workload for native Linux and Windows: final C SHA256 `5d603377...`, native ELF `a7ec7248...`, Windows PE `32ca26c5...`. An explicit depth-write visibility barrier completes before the timed LOAD pass. Six positive full-image/statistics controls include write-zero states; a separate hidden pass requires IA/VS 3N, primitives N and FS zero. Every arm validates two warmups and eight timed images. Source, raw output, hashes and interpretation received independent review.
- Same `86a31e85` / `14d39849...` backend and scored diagnostics off. The standard three-frame fixture and Windows CPU self-test passed before GPU tests, resolving C156's missing fixture for this new session. Eleven Windows Vulkan arms, eleven native Vulkan arms and five unchanged D3D11 area/repeat arms passed. Native Linux 615 and Windows 617.14 retain distinct OS/driver/compiler/CPU pacing; no native Windows D3D11 baseline exists.

| Fully hidden instances, 1024² | Windows Vulkan off / on, median us | Native Linux Vulkan off / on, median us |
| --- | ---: | ---: |
| 1 | 1.888 / 0.896 | 0.880 / 1.216 |
| 16 | 9.008 / 8.832 | 9.552 / 9.456 |
| 64 | 32.480 / 32.640 | 32.304 / 32.672 |

- N64 ranges overlap; medians differ by at most 0.55%. Tiny N1 intervals vary strongly and must not support a write-mask gain claim. This further weakens a generic emulator-wide hidden-draw slowdown hypothesis. C156's D3D11 N64 result remains about 2.3× this Windows Vulkan interval, with API/shader/state/query-placement differences unresolved. It does not identify an emulator defect or Fire Strike cause.
- Unchanged D3D11 1024²/N64/off repeats at **74.416 us**, range 74.176–75.296, within 0.41% of C156. At 256²/N64, both write states split into four samples near **5–6 us** and four near **17–18 us**; midpoint medians 11.632/11.616 describe neither cluster. The same five compiled shaders match across area choices. Target/viewport area and completed seed/readback CPU pacing change together, so this is not an isolated ROP or coarse-Z measurement.
- Matched Vulkan 256²/N64 medians are Windows **4.880/6.144 us** versus native **4.352/5.808 us** off/on, with overlapping ranges. Source and shaders are unchanged. All image/statistics gates still pass. A later ordinary-API repeated-draw/pacing control is documented outside the repository; no GPU-state override was applied.
- C157 stopped cleanly at 06:28 CEST. Available harness checks passed; trace checks skipped, 15 backend ERROR notes, no observed host Xid, host VRAM back to **6937 MiB free**. Read-only PnP confirms the existing FriendlyName **Leandro VFIO RTX 2070** with status OK; DeviceDesc remains NVIDIA GeForce RTX 2070. This is not a Task Manager label check or a new naming edit.
- Evidence: `C157-closeout.json`, `C157-hidden-vulkan-comparison.json`, `C157-vk-occlusion/`, `C157-vk-area/`, `C157-d3d-area/results/`, `owned-vk-occlusion/native{,-area}/` and critical review files. Last complete Fire Strike graphics remains **5740**. Next common-path candidate caches only sorted BAR2 live membership while preserving all comparisons, aliases and barriers; measured savings remain unknown.


## Common-path optimization: BAR2 full-scan membership

- Commit `e41b3551` retains sorted live keys for full scans until membership changes. It still reads current aliases and snapshots and compares every page on each scan. Partial-key scans, dirty merging, write-back ordering, synchronization barriers and mapping lifetimes are unchanged. Reset releases the retained vector.
- Two behavioral regressions cover shared-mapping writes, alias changes without a membership change, conflicting/disjoint bytes, unrelated GPU-written bytes, removal/retarget/reuse, FB/sysmem collisions, partial selection and reset/repopulation. Independent source review found no blocker. Focused BAR2 tests **7 passed**; full device suite **904 passed / 1 ignored**, both integration tests passed; release build and AST refresh passed.
- Root compared production clean full scans against a preserved pre-change release library with the same owned CPU benchmark. At 1/16/64/256/1024 pages and one/two aliases, warmed scans allocate **100 to zero times per 100 scans**, retaining exact comparison counts. Seven-batch median times fell **2.7–20.1%** depending on case; the 1024-page cases fell 11.3/14.9%. Random map layout, scheduler variance and allocator instrumentation remain. Dirty scans, membership churn and partial scans were not timed.
- These are scoped CPU results, not FPS or short-kernel latency gains. The C149 synchronization bucket includes surrounding work; its total must not be presented as saved time. Evidence: `bar2-membership-cache-review.md`, `bar2-cache-{focused-tests,full-tests,release-build,graphify-update}.log`, `bar2-membership-cpu/comparison.{json,md}`.
- Candidate `e41b3551` binary SHA256 `521cae12...` and exact pre-change `6ade0bf6` binary `ba684aab...` are retained separately with source manifests and `cap_sys_admin=p`. C158 candidate passed the standard three-frame D3D11 gate, CPU self-test and two unchanged Vulkan image/statistics arms. Its 1024²/N64/off GPU interval is 32.384 us; this is compatibility evidence, not an application speedup.
- Fresh stock Windows Fire Strike comparison is in progress with scored diagnostics off. No new complete score or gain is claimed at this snapshot. Windows remains first; stock Linux SOTTR follows. Main checkout and Leandro v1 remain unchanged.


## C158: stock Windows application qualification

- Candidate `e41b3551` / `521cae12...`, unchanged stock 617.14 driver, 4G/8G/8CPU, compression slice, BAR2 memfd/direct completion/rearm/pool32. Profile, trace, capture, recording and foreground automation off during scored rendering. Standard three-frame D3D11 fixture and two Vulkan six-control/statistics arms passed before Fire Strike.
- Two complete locally validated Fire Strike P runs: graphics **5754 / 5733**, GT1 **20.043587 / 19.822844** FPS, GT2 **33.287777 / 33.579170** FPS; benchmark intervals 190.356 / 200.880 s. Average graphics **5743.5**, approximately the previous C148 5740 level. No relevant FPS gain is established. Exact pre-change C159 comparison remains pending.
- A separate stricter artifact parser verifies four unique positive finite workload results, four scores, global preset/resolution/scan-on/monitor-off configuration, timestamp order, scored flags and actual backend bytes. Root accepted the real complete C148 archive and rejected incomplete C147 and monitoring-on C145. Raw ZIPs remain private; these are local checks, not online validation or native pixel comparison.
- Present NVIDIA display device reports status OK, problem code zero and existing FriendlyName Leandro VFIO RTX 2070. System event log was readable and contains zero Display 4101 events since launch. Initial PnP query included disconnected devices and failed cardinality; the qualified query uses PresentOnly. Task Manager name remains unverified. Monitoring/scanning restored ON through observed UI; the online-verification setting was unchanged and appeared unavailable.
- C158 stopped cleanly at 07:18 CEST: owned processes absent, host NVIDIA bound, **6937 MiB FB / 232 MiB BAR1 free**, no observed host Xid. Available checks passed; trace and guest-RC checks skipped. **19 backend ERROR notes** remain, including held status-queue retry notes; do not equate a complete score with a clean log. Initial manual driver check used the NVMe BDF; NVIDIA-reported physical GPU `0000:2d:00.0` corrected that verification.
- Evidence: `C158-closeout.json`, `C158-guest-health.json`, `C158-FireStrike-run{1,2}-validated.json`, immutable binary manifest, bounded root controllers and score/result archives under the Windows run directory. Moving Fire Strike footage exists from C145; no new C158 recording was mixed into these scores.


## C159 parent BAR2 control, first complete run

- Exact pre-cache parent `6ade0bf6`, freshly built immutable backend `ba684aab...`; C158 behavior/resource flags and stock Windows driver preserved. Standard three-frame D3D11 fixture and both Vulkan control/statistics arms passed. GPU medians were 2.048 us (256/N1) and 32.320 us (1024/N64).
- Fire Strike P completed all four scored workloads; strict local archive checks passed: graphics **5587**, GT1 **20.025030**, GT2 **30.878220**, Physics **34.126278**, Combined **21.121222** FPS; archive duration **196.545387 s**. Monitoring off/scanning on, diagnostics and capture off. This single baseline run does not establish a BAR2 speedup against C158's 5754/5733; repeat control is pending in C159B.
- C159 ended before its 30-minute bound instead of starting a second run without enough collection/restore time. Monitoring restored on; present NVIDIA adapter status OK/problem code0, qualified System-log query found zero Display4101 events since this boot. Owned processes absent after stop; physical GPU `0000:2d:00.0` stays bound to NVIDIA; 6937 MiB FB and232 MiB BAR1 free. No observed new host Xid; available checks passed, trace/RC checks skipped;18 backend ERROR notes retained.
- One UI-task launch exceeded the caller's20-second timeout. Existing task output was retained and a fresh screenshot checked before retrying the unselected DX11 filter. New artifact-only caller uses45 seconds; no repeated Run click. Scores were collected without foreground actions during rendering.
- Trap follow-up: C149 is an incomplete profile-only run with qualified estimated GT1/GT2 interiors, not a full current event trace. Routed access totals omit per-register identity. A complete candidate census and profile-only routed display counters are next; no Windows tagged-tree/display-page flag was promoted.
- Evidence: `C159-closeout.json`, `C159-guest-health.json`, `C159-vk-qualification/summary.json`, current run's `FireStrike-run1.owned-validation.json`, and `windows-trap-elimination-critical-next.md`. Private result archives/logs retain account and machine metadata; publish curated evidence only.


## C159B repeated parent control

- Same immutable pre-cache parent as C159, fresh bounded boot. Standard D3D11 fixture passed; Fire Strike P completed and strict local archive checks passed: graphics **5756**, GT1 **20.032911**, GT2 **33.341473**, Physics **37.045277**, Combined **20.590214** FPS; duration191.676330 s. Scored diagnostics/capture off, monitoring off/scanning on.
- Parent scores5587/5756 overlap candidate5754/5733. The parent repeat matches the candidate; **no Fire Strike improvement is established**. Two scores per arm, unequal boot grouping, no randomized order or matched physical clocks; descriptive means must not be called an optimization gain. Evidence: `bar2-windows-scored-comparison.json`.
- Monitoring restored on; present adapter OK/problem0, qualified zero Display4101 since boot. Owned PIDs gone, no observed host Xid, physical NVIDIA binding retained,6937 MiB FB/232 MiB BAR1 free. Available checks passed with trace/guestRC checks skipped;18 ERROR notes retained. Evidence: `C159B-closeout.json` and `C159B-guest-health.json`.

## Routed display register observations

- Existing successful routing bypasses ordinary access tracing. Added bounded profile-only count/time subrows for52 modeled display addresses, read/write direction and overflow; reused the existing route timer. Dispatch, data, writes, locks and ordinary access accounting unchanged. Fixed atomic storage3552 bytes; labels created once at dump. `CARAXES_PROFILE=0` does not execute the new hook.
- Subrows are **nonadditive** to their original parent counters. Concurrent relaxed snapshots are not transactional; exact parent/subrow reconciliation is valid only when writers are quiescent. This change removes no trap and qualifies no memory-page option.
- Independent source review passed. Root focused routed tests6 passed; full device906 passed/1 ignored plus2 integration tests; release and AST update passed. Coverage independently checks `Engine::handles`, both directions/parents, overflow/shared time, disabled profiling and existing no-Device-lock routing.
- Frozen interval tools retained. A separate derivative consumer pairs count-column nanoseconds, separates display subrows from callback totals and does not invent a mean for duration-only deltas. Root22 synthetic-schema cases passed; both original C149 intervals reproduced their exact tree counts/times. Missing raw partners remain unqualified, including exporter-elided zero-time ambiguity.
- Evidence: `routed-display-root-qualification.json`, `routed-display-profile-critical-review.md`, `routed-display-consumer-root-qualification.json`, `routed-display-consumer-backcompat-root.json`. The C161 packet is preparation only; the complete instrumented census is planned as C162, with its score excluded from A/B. No Windows tree/display-page behavior flag was promoted.

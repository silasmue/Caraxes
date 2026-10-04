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

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

<!-- SPDX-License-Identifier: MIT -->
# Windows 3DMark investigation, 2026-10-04

- Goal: complete and verify Steel Nomad, then reduce Windows submission and completion overhead.
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

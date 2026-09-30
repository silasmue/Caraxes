<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Verification methods

How both architectures are tested and measured: the four-guest gate every change passes before a
merge, the rendering check, the Windows test pack, the per-area gates, the census and profile
that attribute cost, and the checks that need no GPU. A number counts only with its run
recorded and 0 host Xid on the device's pid.

## The four-guest gate

One device binary, four guests, before every merge into dev
([research: one-abi-binary-2026-09-28](../research/one-abi-binary-2026-09-28.md)).

| Leg | Guest | Passes when | Script |
|---|---|---|---|
| kick-smoke | Linux 615 open driver (Linux B desktop VM, host execution) | the smoke suite (vkcube X11 and Wayland, glmark2, the NVENC holder), then a Moonlight stream whose three screenshots are not black and differ (RMSE); guest Xid 0, no host Xid | `caraxes/runs/night-2026-09-28/kick-smoke.sh OUT BINARY` |
| nova 5-probe | nova-core on Linux 7.2.7, 570 firmware, pure emulation (vm9) | 5 probes name the GPU, 4 unloads, every bind rc 0, 0 device ERROR | `caraxes/runs/nova-2026-09-25/nova-boot.sh` |
| vm10 R1 to R8 | nouveau with GSP-RM 570.144 and Mesa NVK, host execution | R1 probe, R2 kernel channels, R3 NVK device, R4 clear and copy read back, R5 triangle read back, R6 headless scenes, R7 GL through EGL surfaceless, R8 weston headless with vkcube-wayland | `caraxes/runs/openrender-2026-09-29/openrender-boot.sh` (guest side `openrender-check.sh`) |
| Windows session | Windows 11, driver 617.14, Cloud Hypervisor | D3D11 stages 12 of 12; Heaven and Superposition run to their result; 0 host Xid, 0 FLR | `caraxes/runs/win-2026-09-30/session.sh` with the Windows test pack below |

- The Linux legs run under `vmlock.sh`, one VM per slot: `caraxes/runs/gate-2026-09-30/legs-*.sh LEG`.
- Last recorded results:

| Leg | Build | Result | Record |
|---|---|---|---|
| kick-smoke, nova | b6d01293 (2026-09-30) | PASS | `caraxes/runs/gate-2026-09-30/README.md` |
| vm10 | 5fcf62ae (2026-09-29) | PASS, `vkCreateDevice` 80.8 ms, triangle 20808 pixels | `caraxes/runs/openrender-2026-09-29/vm10-gate/RESULT.md` |
| Windows | b6d01293 (2026-09-30, C118) | stages 12/12, Heaven 155.9 and 153.3 fps, Superposition 12228 and 12160 | [research: windows-findings](../research/windows-findings.md) step 23 |

- On b6d01293 the vm10 leg was refused its VRAM slot while another VM held it and then ran
  beside the census VM; the README records no verdict.
- The b6d01293 kick-smoke streamed x264: the guest's Sunshine had been left on the software
  encoder (`caraxes/runs/codec-2026-09-30/matrix.md`).

## The rendering check

- `caraxes/runs/gate-2026-09-30/frame-check.sh SHOT.png [REF.png]` compares a SotTR guest
  screenshot with a known-good reference frame by colour statistics, not pixels, because the
  camera moves. It needs ImageMagick (`magick`).
- Statistics: the share of pixels above 0.7 HSL saturation and the mean grey. The reference has
  4.7 % and 0.17; the broken ("neon") Linux frame of 2026-09-30 has 74.5 % and 0.72.
- PASS: saturated share at most 4 times the reference's (or at most 0.08) and at most 0.25, grey
  mean at most 0.25 above the reference and at least 0.02. All six shots of the 2026-09-29 SotTR
  A/B runs pass; the broken frame fails.

## The Windows test pack

- `tools/windows/*.ps1`, stock Windows components only (PowerShell 5.1, C# 5); each prints one
  PASS or FAIL line per check and exits non-zero on a failure ([tools/windows/README.md](../../tools/windows/README.md)).
- They run as SYSTEM in a scheduled task (`run44.py pack <script>` under Cloud Hypervisor, over
  SSH set up by `ssh-setup.ps1`), or in the signed-in session (`run44.py upack`) when they need a
  desktop (`caraxes/runs/win11-2026-09-25/run44.py`).

| Script | Checks |
|---|---|
| `smoke.ps1` | Device Manager status, adapter strings, `nvidia-smi`, `vulkaninfo`, the D3D11 and D3D12 gates, TDR events |
| `d3d11-render.ps1`, `d3d11-stages.ps1` | a triangle and clears read back; 12 stages: raster, clear, D32 depth, texture Load, constant buffer, compute UAV, 4x MSAA resolve, event and occlusion queries, tessellation |
| `d3d12-probe.ps1`, `cuda-probe.ps1` | 100 fence round trips and a compute dispatch; deviceQuery, PTX JIT, launch cost, pinned bandwidth |
| `nvenc-probe.ps1`, `media-probe.ps1` | NVENC through its API; NVENC and NVDEC through ffmpeg |
| `heaven.ps1`, `superposition.ps1` | Unigine Heaven 4.0 (DX11, 1920x1080, `-Quality`) and Superposition 1.1 (`-Preset 1080p_medium`) |
| `present-probe.ps1`, `edge-anim.ps1` | a flip-model swap chain windowed and fullscreen; a second GPU client beside a benchmark |
| `workload.ps1`, `soak-media.ps1`, `cuda-loop.ps1` | llama.cpp and torch against Linux and native; the media soak; a CUDA load |
| `dxgk-etw.ps1`, `display-mode.ps1`, `quiet.ps1` | a DxgKrnl ETW trace across a TDR; mode changes; Windows Update paused |

## The per-area gates

- `tools/caraxes-gate.sh` runs `nvml` (11 checks against two host readings), `cuda` (12 scored
  tests, one ssh call each), `gpu` (v1's stages), `vdisplay` and `display` (60 +- 0.5 Hz).
- v1's verdict rules: one JSON line, exit 0 pass, 1 fail, 2 skip (a missing precondition only),
  binary verdicts, a dying gate reports a fail ([testing.md](../virtio/testing.md)).
- A failed `cuda` test is re-run under the v1 tracer (`LD_PRELOAD=libnvrm_trace.so`); each run
  writes `caraxes/runs/gate-<name>-<date>/`. v1 on the 615 image: `caraxes/tools/v1-gates.sh`.

## Benchmarks, census and profile

- `caraxes/tools/bench-run.sh series` interleaves native and the virtual targets inside every
  rep, restarts the device per rep and marks a run concurrent above 20 % of a CPU in another VM's
  device; `bench-hostwatch.sh` stops the device when the host's `nvidia-smi` takes over 5 s.
- Census: `census-cu.c` brackets each operation with writes to `NV_PBUS_SW_SCRATCH(30)`;
  `census-table.py` splits the device trace at the markers.
- Profile: `CARAXES_PROFILE=1`, per row a count, device time and a status histogram; `SIGUSR2`
  dumps a TSV; 84 ns per trapped access. Rows also cover trapped doorbells by token and the
  accesses routes serve without the device mutex ([research: trap-census-2026-09-30](../research/trap-census-2026-09-30.md)).
- `caraxes/tools/trap-census.py DUMP --base DUMP [--scope client:0x..]` groups trapped accesses
  into sources with rate, device time and guest-seen time; a client scope is one guest RM
  client's lifetime.
- A live state dump: `echo state > <state dir>/device-ctl` logs every channel, host object and
  the interrupt state (`crates/vfio-user-nvgpu/src/main.rs`, `device.rs` `state_dump`).

## Checks without a GPU

- `tools/check.sh`: formatting, clippy, unit tests, licence headers, the `nvrm-sys` ABI
  generation, the `gsp-abi` drift step ([gsp-abi-615](gsp-abi-615.md)); with
  `CARAXES_CHECK_EMU_BOOT=1` the emu-boot step.
- `tools/emu-boot-check.sh [--driver 610|615] [--chip tu106|gb206]` boots the one release binary
  in pure emulation and exits 0 on `RmInitAdapter succeeded` and a working `nvidia-smi`.
- `settings::tests` keep the variable table equal to the sources ([device-configuration](device-configuration.md)).
- The lab kit captures cards the project does not own ([contributing-a-capture](contributing-a-capture.md)).

## How a new workload is measured

1. A `bench-run.sh` metric printing one `# NAME key=value` line.
2. The VM in the configuration under test with `CARAXES_PROFILE=1` and the host watch.
3. `kill -USR2` before and after; native and virtual interleaved, 3 reps, `V2_RESTART=1`.
4. Rank the profile delta; read the ratio, device CPU against wall time, step p90 against median.
5. Keep the dumps and the host Xid lines of the device's pid; count only with 0 host Xid there.

## Depth

- [research/methods.md](../research/methods.md): every tool with its lines.
- [vfio/testing.md](../vfio/testing.md), [vfio/benchmark.md](../vfio/benchmark.md) §2,
  [vfio/coverage-map.md](../vfio/coverage-map.md).

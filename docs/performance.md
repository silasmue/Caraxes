<!-- SPDX-License-Identifier: MIT -->
# Performance

Native, Leandro VirtIO (v1) and Leandro VFIO (v2) on one RTX 2070. v1 and v2 were measured
on 2026-09-30; native values come from records, because the card drives the user's desktop.
`R` below is the run folder `/mnt/vmstore/caraxes/runs-2026-09-30/bench/`.

## Setup

| | |
|---|---|
| Host | AMD Ryzen 7 3800X (8 cores, 16 threads), 32 GiB, Arch Linux, kernel 7.2.7 |
| GPU | RTX 2070 (TU106, 8 GiB), also the host desktop's card; driver 615.71.09, open modules, GSP firmware |
| v2 device | `vfio-user-nvgpu-stack-ffec4d49`, sha256 `32b1c1f37b3be2e8…`: dev's device code and the four-guest gate's binary; Windows ran the same bytes as `vfio-user-nvgpu-stack-cap` |
| v1 backend | `vhost-user-nvrm` of Leandro 8c0d2b2, sha256 `2fe56aa07903abcd…`; guest modules built in the guest from the same sources |
| Hypervisor | Cloud Hypervisor v53.0; v1 with patches 0001-0004, v2 with 0005-0008 (the compute VM with 0001-0006) |
| v2 compute guest | Ubuntu 24.04, 615.71.09, 8 vCPUs, 4 GiB RAM, 4 GiB VRAM, no display, device trace off |
| v2 desktop guest | Ubuntu 24.04, 615.71.09, GNOME on Wayland, 8 vCPUs, 8 GiB RAM, 1 GiB VRAM; for SotTR 12 GiB RAM and 4 GiB VRAM |
| v2 Windows guest | Windows 11, driver 617.14, 8 vCPUs, 8 GiB RAM, 4 GiB VRAM |
| v2 open drivers | nova-core (pure emulation); nouveau with GSP-RM 570.144 and Mesa NVK (1 GiB VRAM) |
| v1 guest | the 615.71.09 image on a fresh overlay, 8 vCPUs, 4 GiB RAM, VRAM cap 4096 MiB |

- One VM at a time; `CARAXES_PROFILE` off in every v2 run.
- `v2 / native`: above 1 is slower for times, below 1 is slower for rates. "±10 %" says
  whether v2 is within 10 % of native (a faster value also meets it).
- Native records: [a] the closing series of 2026-09-24, driver 610.57.04
  (`caraxes/runs/closing-610-2026-09-24/summary.md`); [b] the night of 2026-09-28, 615.71.09
  (`caraxes/runs/night-2026-09-28/native/`); [c] the ctxcreate records of 2026-09-29,
  615.71.09 (`caraxes/runs/ctxcreate-2026-09-29/native/`).

## Compute and latency

Leandro-Test's probes and `bench-cu` in the P13 metrics of `caraxes/tools/bench-run.sh`
(median [min-max], 3 reps, v1 restarted per rep) and the workload series of 2026-09-28.
Records: `R/v2-compute/`, `R/ab-short/stack/`, `R/v1/`, `R/v1b/`.

| Metric | native | v1 | v2 | v2 / native | ±10 % |
|---|---|---|---|---|---|
| short kernel (1000 cycles), µs per launch, auto sync | 6 [a, b] | 8 [7-9] | 23 [16-27], n=6 | 3.9 | misses |
| short kernel, blocking sync, µs | 21 [a] | 15 [15-16] | 68 [42-237] | 3.2 | misses |
| 30M-cycle kernel, ms, auto sync | 16.3 [a] | 16.5 | 17.1 | 1.05 | meets |
| 30M-cycle kernel, ms, blocking sync | 16.1 [a] | 100 | 16.9 | 1.05 | meets |
| cuInit, ms | 29.4 [c] | 29.9 | 12.1 | 0.41 | meets |
| cuCtxCreate, ms | 129 [c] | 153 | 210 [184-264] | 1.63 | misses |
| cuCtxDestroy, ms | 71 [c] | 71 | 96 | 1.35 | misses |
| cuMemAlloc 1 MiB, µs | 42 [c] | 74 | 153 | 3.6 | misses |
| cuMemFree 1 MiB, µs | 31 [c] | 42 | 254 | 8.2 | misses |
| cuMemAlloc 64 MiB, µs | 82 [c] | 141 | 2124 | 26 | misses |
| cuMemFree 64 MiB, µs | 103 [c] | 115 | 2125 | 21 | misses |
| copy h2d pinned 1 MiB, GB/s | 12.1 [a] | 12.1 | 10.2 | 0.84 | misses |
| copy h2d pinned 16 MiB, GB/s | 13.0 [a] | 13.0 | 12.8 | 0.98 | meets |
| copy h2d pinned 256 MiB, GB/s | 13.1 [a] | 13.1 | 13.1 | 1.00 | meets |
| copy d2d 1 MiB, GB/s | 132 [a] | 124 | 44.5 | 0.34 | misses |
| copy d2d 16 MiB, GB/s | 183 [a] | 182 | 155 | 0.85 | misses |
| copy d2d 256 MiB, GB/s | 186 [a] | 189 | 164 [132-190] | 0.88 | misses |
| RM call (`CHECK_VERSION_STR`), µs, p50 | 2.9 [a] | 15.5 | 1.9 | 0.66 | meets |
| RM control 232 B, µs, p50 | 5.4 [a] | 18.4 | 2.3 | 0.42 | meets |
| RM control 49192 B, µs, p50 | 4.3 [a] | 44.4 | 2.4 | 0.55 | meets |
| rlprobe, 50 episodes, s | 1.20 [a] | 1.40 | 2.10 | 1.75 | misses |
| rlprobe, host transfers per s | 2350 [a] | 2109 | 1398 | 0.59 | misses |
| matmul fp32 N=1024, GFLOP/s | 4894 [a] | 5304 | 4570 | 0.93 | meets |
| matmul fp32 N=4096, GFLOP/s | 6052 [a] | 6765 | 6061 | 1.00 | meets |
| llama.cpp 1B Q4_K_M pp512, tokens/s | 7639 [b] | 7769 ± 1352 | 7461 ± 514 | 0.98 | meets |
| llama.cpp 1B Q4_K_M tg128, tokens/s | 305 [b] | 304 ± 7 | 243 ± 22 | 0.80 | misses |
| PyTorch transformer training, tokens/s | 107485 [b] | 110242 | 109482 | 1.02 | meets |
| PyTorch ResNet-18 training, images/s | 532 [b] | 530 [524-550], n=3 | 515 [497-549], n=3 | 0.97 | meets |

- v2's RM calls stay in the guest's own RM; v1 forwards every call to the host.
- The short kernel spreads: 16 to 27 µs over six runs of 100 launches, 24 to 53 µs over eight
  runs of 1000 (v1: 7 µs in all three). The 13 to 14 µs of 2026-09-28 came from binary
  945431f9 (`caraxes/runs/night-2026-09-28/series945-fast2/`); that binary did not boot in
  today's configuration (`R/ab-short/n945/`, RmInitAdapter 0x62), so there is no same-day
  comparison.
- v1's blocking sync is a flat 100 ms poll; its RM calls cost 13 to 44 µs each.

## Linux graphics

v2 Linux desktop guest at 1280x720 on GNOME, native on the host's own Wayland desktop.
Records: `R/kick/smoke.txt`, `R/vm10/`, `R/sottr/`.

| Metric | native | v1 | v2 | v2 / native | ±10 % |
|---|---|---|---|---|---|
| glmark2-wayland 1280x720, score | 12214 [b] | not measured | 3324 | 0.27 | misses |
| vkmark Wayland 1280x720, score | 15973, 15940 [b] | not measured | 2635, 2388 | 0.16 | misses |
| glmark2 `build`, 10 s, fps | not measured | not measured | 3944 | - | - |
| glmark2 off-screen 1920x1080, score (Leandro-Test `bench.sh render`) | 21486 (2026-08-07) | 21277 (2026-08-07); fails today: Xvfb refuses the GLX context, the image's `/opt/nvrm-gl` holds 610's GL user space ([v1 615 image](vfio/v1-615-image.md), `R/v1-gl/`) | not measured | - | - |
| nouveau + NVK: glmark2-wayland, score (`build`, `texture`, `shading`, 5 s each) | not measured | - | 3875 | - | - |
| nouveau + NVK: vkcube-wayland, fps | not measured | - | 131 | - | - |
| nouveau + NVK: vkmark | not measured | - | fails, `DeviceLostError` ([known issue 8](vfio/known-issues.md)) | - | - |
| Shadow of the Tomb Raider, 1080p, average fps | 91.7 (2026-08-20, before 615) | not measured | fails: `VK_ERROR_DEVICE_LOST` at start, no Xid ([known issue 2](vfio/known-issues.md)) | - | misses |

- glmark2 `build` spread on the same binary this morning: 4501 to 4756, and 432 once
  (`caraxes/runs/gate-2026-09-30/stack/RESULT.md`). The gate's SotTR run at 10:02 on the same
  binary reached 34.7 fps average (0.38 of native).
- The native SotTR value is cited in `caraxes/runs/night-2026-09-28/README.md`; its result
  file is not in the records.

## Windows graphics

v2 only: v1 has no Windows guest, and the host runs Linux, so there is no native Windows
value on this card. Record: `R/win-bench2/`, the kit's `windows-plain/bench2/`.

| Test | v2 | Earlier records (gate: the same binary, 2026-09-30 morning) |
|---|---|---|
| D3D11 stages (triangle, depth, texture, constant buffer, compute, MSAA, queries, tessellation) | 12 of 12 pass | 12 of 12 (gate, 09:52) |
| Unigine Heaven 4.0, 1080p High, tessellation normal, windowed, fps | 152.6 | 140.4 (gate); 152 to 158 (2026-09-30, [windows-findings](research/windows-findings.md) step 23) |
| Unigine Superposition, 1080p Medium, points | 11656 (87.2 fps, min 14.9) | 11622 (gate); up to 12247 (step 23) |
| 3DMark Steel Nomad (DX12) | fails ([known issue 1](vfio/known-issues.md)); not run | - |
| Guest reboot | back after 65 s, served as the device's second client | pass (gate) |

## Video

ffmpeg N-126965 on 30 s 1080p60 clips (1800 frames), the scripts of
`caraxes/runs/codec-2026-09-30`. No native ffmpeg run exists on record; v1's 615 image has
no NVENC or NVDEC user space. Records: `R/codec-linux-b1/`, `R/win-bench2/matrix.txt`.

| fps | Linux v2 | Windows v2 |
|---|---|---|
| NVENC H.264, frames from NVDEC in VRAM | 414 | 311 |
| NVENC H.264, frames from the CPU | 409 | 190 |
| NVENC HEVC, frames from NVDEC in VRAM | 333 | 281 |
| NVENC HEVC, frames from the CPU | 326 | 175 |
| NVDEC H.264: cuda / cuvid / d3d11va | 661 / 650 / - | 574 / 375 / 343 |
| NVDEC HEVC: cuda / cuvid / d3d11va | 1307 / 1189 / - | 777 / 343 / 487 |
| NVDEC VP9: cuda / cuvid / d3d11va | 900 / 874 / - | 612 / 327 / 432 |
| Decode bit-exact against software (framemd5) | 1800 of 1800, all three codecs | 1800 of 1800, cuda and d3d11va |
| AV1 encode and decode | refused cleanly (TU106 has neither) | refused cleanly |

`nvenc-probe` (300 frames, 1080p, H.264), the only codec test with a native value:

| | native [b] | v1 | v2 desktop guest | v2 compute guest | v2 / native |
|---|---|---|---|---|---|
| encode, fps | 166, 180 | not available | 188, 178 | 154, 144 | 0.83 to 1.09 |
| decode, fps | 141, 143 | not available | 154, 166 | 122, 123 | 0.86 to 1.17 |

## Streaming

Sunshine in the guest, Moonlight on the host, 1080p60 at 20 Mbit/s, 150 s windows; load:
vkcube on Linux, a 60 fps D3D11 present loop on Windows. No native or v1 stream is on record.
Records: `R/codec-linux-b1/stream-*`, `R/win-bench2/stream-*`.

| | Linux nvenc (HEVC) | Linux x264 | Windows nvenc | Windows x264 |
|---|---|---|---|---|
| frames rendered per second | 59.9 | 59.8 | 59.8 | 58.8 |
| host processing latency, ms, average (min-max) | 5.3 (3.5-23.8) | 17.5 (13.4-78.5) | 6.9 (4.4-74.2) | 12.6 (7.6-85.1) |
| Sunshine in the guest, % of a core | 22.5 | 138 | 15.9 | 205 |
| guest total, % of 8 vCPUs | 8.6 | 24.2 | 13.8 | 35.0 |
| Cloud Hypervisor on the host, % of a core | 59.1 | 173 | 102 | 274 |
| device process on the host, % of a core | 48.3 | 50.5 | 50.3 | 32.9 |

- Windows NVENC at 60 fps now averages 6.9 ms; [known issue 3](vfio/known-issues.md) measured
  31 to 70 ms at 21 fps on a static desktop.

## Leandro-Test suite

`lt-all.sh` (`caraxes/runs/ctxcreate-2026-09-29`): the CUDA, suite and gpu tests of
`tools/caraxes-guest-tests.sh`, every `probe/matrix` probe, the C and Python probes and `bench-cu`, 65 tests, one
pass after another in one guest. Both ran in their compute guests (no display, no ffmpeg in
the image). Records: `R/lt/v2-lt/rows.tsv`, `R/v1b/rows.tsv`.

| | pass | fail | skip |
|---|---|---|---|
| v2 | 36 | 19 | 10 |
| v1 | 19 | 36 | 10 |

| Failing tests | v2 | v1 | Cause | Kind |
|---|---|---|---|---|
| cuda-managed, gpu-kernel, managedprobe | fail | pass, fail, fail | v2: `cudaMallocManaged` refused (801), no replayable faults ([known issue 6](vfio/known-issues.md)); v1: see the cascade | v2: known gap |
| robustness | pass | fail | v1: the parallel and SIGKILL parts pass; the check reads `nvidia.ko`'s refcount, which a v1 guest does not have | harness |
| 20 CUDA tests after robustness (suite async streams, gpu-torch, gpu-own, nvprobe-4, hostregprobe, oomprobe, vrampress, torchprobe, torchprobe-conv, mmsweep, mtprobe, streamprobe, pinwin, convoom, vramcap, bench-init, bench-churn, bench-copy, and the two above) | pass | fail | v1: from the test after robustness on, every `cuCtxCreate` fails (304, "device busy or unavailable") until the VM restarts; the same tests pass in the restarted VM's cuda gate (14 of 14) | v1 defect, cause open |
| gpu-encode, matrix-nvenc, matrix-nvdec, matrix-vk-offscreen | fail | fail | no ffmpeg in the compute images | environment |
| matrix-opencl, matrix-vk-rt, matrix-egl-gbm, -wayland, -xcb, -xlib | fail | fail | these probes are not built in the guests' Leandro-Test copy | environment |
| matrix-vk-enum, matrix-gl-enum, matrix-gles | fail | fail | the compute images load no NVIDIA GL or Vulkan ICD (`libGLX_nvidia.so.0` missing, GL falls back to Mesa) | environment |
| matrix-rm-direct | fail | pass | v2: the display class 0x0073 is refused, because the compute VM runs without a display | environment |
| ctrlping | fail | fail | ran clean (2000 calls, 0 fails); the criterion expects its 16384-byte line, the probe prints the total 49192 | harness |
| bench-launch | fail | fail | this `bench-cu` build has no `launch` command | harness |

- The 10 skips are the same on both: cuda-gdb, cupy and cuDF not installed; NvOFA, NvFBC,
  PKCS#11 and Vulkan SC declared unsupported; two probes that need a kernel trace point.

## Stability and gates

| Check | v2 | v1 |
|---|---|---|
| Linux 615 kick-smoke: vkcube Wayland and X11, glmark2, nvenc-probe, NVENC holder, Moonlight picture changes | pass, guest Xid 0 (`R/kick/`) | - |
| nova-core: probe, three unbind/bind cycles, reload | pass, 5 of 5 (`R/nova/`) | - |
| nouveau + NVK R1-R8 (vm10) | checks pass (offscreen and Weston hashes); vkmark fails ([known issue 8](vfio/known-issues.md)); 16 BAR1 pool remap refusals (`R/vm10/`) | - |
| Windows 617.14 session: stages, Heaven, Superposition, codec matrix, two streams, one reboot | pass; 0 host Xid, 0 FLR, no Display or nvlddmkm event; Windows Error Reporting filed six `LiveKernelEvent 141` reports at 12:21 (during the codec matrix), origin not determined | - |
| Shadow of the Tomb Raider (Linux) | fails at start ([known issue 2](vfio/known-issues.md)) | not run |
| nvml gate | pass (`R/lt/v2-lt/gate-nvml.txt`) | - |
| gpu gate (compute guest) | fails: smi (the host's `Disp.A On` against the guest's `Off`), kernel (managed memory), encode (no ffmpeg), torch (no native reference, not run on the desktop card) | - |
| smi gate | - | fails only on the `-4G` suffix the VRAM cap adds to the name (the gate's mask predates it) (`R/v1b/gates/`) |
| cuda gate | - | 14 of 14 (`R/v1b/gates/cuda/rows.tsv`) |
| Host Xid, 10:30 to 13:00 | 0 | 0 |

## Limitations

Ranked by distance from native, v2 first.

1. **v2: VRAM allocation and free**, 3.6 to 26 times native (64 MiB: alloc 26×, free 21×).
   Each allocation's page-table update reaches the device as trapped MMU invalidates and
   BAR2 flush reads plus a shadow sync
   ([ctxcreate](research/ctxcreate-2026-09-29.md), [trap census](research/trap-census-2026-09-30.md)).
2. **v2: trap-bound 3D**: vkmark 0.16, glmark2 0.27 of native. 3200 to 8600 exits per second
   at 35 to 40 µs per round trip, and one vfio-user socket carries every vCPU's traps
   ([known issue 7](vfio/known-issues.md), [trap census](research/trap-census-2026-09-30.md)).
3. **v2: submission latency**: short kernel 3.9×, rlprobe 1.75×, llama.cpp token generation
   0.80, small device copies 0.34 (d2d 1 MiB): each submission passes the device's doorbell
   reactor and shadow barrier ([launch path](vfio/launch-path.md), [doorbell poll](vfio/doorbell-poll.md)).
4. **v2: failing workloads**: Shadow of the Tomb Raider on Linux
   ([known issue 2](vfio/known-issues.md)), DX12 3DMark ([known issue 1](vfio/known-issues.md)),
   CUDA managed memory ([known issue 6](vfio/known-issues.md)), vkmark on nouveau
   ([known issue 8](vfio/known-issues.md)).
5. **v2: setup paths**: cuCtxCreate 1.6×, cuCtxDestroy 1.35×: host channel builds, gather
   descriptors, BAR2 syncs ([ctxcreate](research/ctxcreate-2026-09-29.md)).
6. **v2: one VM at a time, 4 GiB guest VRAM**: the slice is fixed at start, and two VMs can
   exhaust host VRAM because the shadow page tables live outside the slice
   ([known issue 4](vfio/known-issues.md), [host resources](vfio/host-resources.md)).
7. **v1: blocking synchronisation**: a flat 100 ms poll, 6.2× native on a 16 ms kernel
   ([leandro.md](leandro.md#12-strengths-weaknesses-and-the-road-to-christmas-2026), weakness 16).
8. **v1: forwarded RM calls**: 15.5 µs per call (5.3×) and 18 to 44 µs per control (3.4 to
   10×) ([leandro.md](leandro.md#12-strengths-weaknesses-and-the-road-to-christmas-2026), weakness 16).
9. **v1: lost contexts and retained pins**: after Leandro-Test's robustness test (two parallel
   clients, a SIGKILL, an FD holder) every new context fails until the VM restarts (cause
   open); freed pinned registrations stay charged until the guest restarts
   ([benchmark §6](vfio/benchmark.md), [open question 83](virtio/open-questions.md#83-conservative-pin-retention-can-exhaust-the-vm-budget)).
10. **v1: no display, video or Windows**: its 615 image has no display path and no NVENC or
    NVDEC user space, and v1 has no Windows guest ([v1 615 image](vfio/v1-615-image.md)).

## Reproduce

```sh
caraxes/runs/bench-2026-09-30/run.sh compute    OUTDIR   # v2 CUDA: workloads, P13 metrics, Leandro-Test (15 min)
caraxes/runs/bench-2026-09-30/run.sh v2-linux   OUTDIR   # v2 desktop: smoke, vkmark, glmark2, codecs, Sunshine, SotTR (45 to 80 min)
caraxes/runs/bench-2026-09-30/run.sh open       OUTDIR   # nova-core and nouveau + NVK (10 min)
caraxes/runs/bench-2026-09-30/run.sh v2-windows OUTDIR   # D3D11, Heaven, Superposition, codecs, Sunshine, reboot (40 min)
caraxes/runs/bench-2026-09-30/run.sh v1         OUTDIR   # v1: P13 metrics, workloads, Leandro-Test, gates (35 min)
caraxes/runs/bench-2026-09-30/run.sh all        OUTDIR   # all of the above, one VM after another
```

- Every area writes `OUTDIR/summary.md`; a third argument names another v2 device binary.
- Prerequisites:
  - host: KVM, the NVIDIA open modules 615.71.09 with GSP firmware, the taps `caraxes2`,
    `caraxes6` and `caraxes7` (192.168.102.1, .106.1, .107.1) owned by the user;
  - the VM store `/mnt/vmstore/caraxes`: `locks/vmlock.sh`, the gate binary
    `vfio-user-nvgpu-stack-ffec4d49`, the guest disks (`vm2-615`, `vm9`, `vm10`,
    `alpha-2026-09-27/desktop-b`, `desktop-b-dmem`), the Windows kit
    `alpha-2026-09-27/windows-plain` with its `-cap` copy of the binary, the v1 image
    `images/guest-v1-2026-09-24-615.71.09.qcow2`, the codec clips
    (`runs-2026-09-30/codec`), the workload script with its nvenc-probe
    (`night-2026-09-28/guest-scripts/guest-series.sh`), the llama.cpp build and model
    (`llama/dist`, `models/llama-gate.gguf`);
  - the patched Cloud Hypervisor builds (`patches/README.md`), `target/bench/bin`
    (`caraxes/tools/bench-run.sh` header), and Leandro and Leandro-Test checkouts beside
    this repository;
  - on the host desktop: Moonlight, grim, ImageMagick and Hyprland's `hyprctl` for the stream
    checks, mtools for the v1 seed.
- The native columns come from `caraxes/tools/bench-run.sh series --targets native` and the
  same workload scripts run on the host; they are not part of `run.sh`, which never loads
  the host card.

## Not measured

- Native: no run today, the card drives the user's desktop. No native record exists for the
  ffmpeg codec matrix, Sunshine, glmark2 `build`, nouveau, nova or Windows (the host runs
  Linux).
- v1: graphics, video, streaming and Windows. Its 615 image has no display path and no NVENC
  or NVDEC user space; v1 has no Windows guest. SotTR on v1 has no frame rate on record.
- llama.cpp with Qwen 7B (`-ngl 20`): the model is not staged in the guests.
- Two or more GPU VMs at once ([known issue 4](vfio/known-issues.md)).

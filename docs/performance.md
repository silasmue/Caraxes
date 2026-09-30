<!-- SPDX-License-Identifier: MIT -->
# Performance

Native, Leandro VirtIO (v1) and Leandro VFIO (v2) on one RTX 2070. v1 and v2 were measured
on 2026-09-30 unless a value carries a date; native values come from records, because the
card drives the user's desktop. `R` is the run folder `/mnt/vmstore/caraxes/runs-2026-09-30/bench/`.

## Setup

| | |
|---|---|
| Host | AMD Ryzen 7 3800X (8 cores, 16 threads), 32 GiB, Arch Linux, kernel 7.2.7 |
| GPU | RTX 2070 (TU106, 8 GiB), also the host desktop's card; driver 615.71.09, open modules, GSP firmware |
| v2 device | `vfio-user-nvgpu-stack-ffec4d49`, sha256 `32b1c1f37b3be2e8…`: dev's device code and the four-guest gate's binary; Windows ran the same bytes as `vfio-user-nvgpu-stack-cap` |
| v1 backend | `vhost-user-nvrm` of Leandro 8c0d2b2, sha256 `2fe56aa07903abcd…`; guest modules built in the guest from the same sources |
| Hypervisor | Cloud Hypervisor v53.0; v1 with patches 0001-0004, v2 with 0005-0008 (the compute VM with 0001-0006) |
| v1 guest | the 615.71.09 image on a fresh overlay, 8 vCPUs, 4 GiB RAM, VRAM cap 4096 MiB, no display |
| v2 Linux | Ubuntu 24.04, 615.71.09, 8 vCPUs. Compute: 4 GiB RAM, 4 GiB VRAM, no display. Desktop: GNOME on Wayland, 8 GiB RAM, 1 GiB VRAM; for SotTR 12 GiB RAM and 4 GiB VRAM. Open drivers: nova-core (pure emulation), nouveau with GSP-RM 570.144 and Mesa NVK (1 GiB VRAM) |
| v2 Windows | Windows 11, driver 617.14, 8 vCPUs, 8 GiB RAM, 4 GiB VRAM |

- One VM at a time; `CARAXES_PROFILE` off in every v2 run. `-`: not run on that guest.
- The goal is ±10 % of native; [Limitations](#limitations) names where each version misses it.

Sources:

- P13: `caraxes/tools/bench-run.sh series`, median [min-max] of 3 reps, v1 restarted per
  rep (`R/v2-compute/p13`, `R/ab-short/stack/p13`, `R/v1/p13`). WS: the workload series of
  2026-09-28 (`R/v2-compute/guest-series.txt`, `R/v1b/guest-series.txt`).
- Native: [a] the closing series of 2026-09-24, driver 610.57.04
  (`caraxes/runs/closing-610-2026-09-24/summary.md`); [b] the night of 2026-09-28
  (`caraxes/runs/night-2026-09-28/native/`); [c] the ctxcreate records of 2026-09-29
  (`caraxes/runs/ctxcreate-2026-09-29/native/`); [d] Leandro-Test `scripts/bench.sh`
  (render 2026-08-07; vk 2026-08-18 on driver 610.43.03,
  `/mnt/vmstore/leandro/vm/bench-vk-20260818-172559`).
- Older v1: [e] SotTR result files of the v1 desktop guests, driver 610.57.04, copied from
  their disks to `R/v1-sottr-records/`; [f] the v1 display gate of 2026-09-19, 610.57.04
  (`/mnt/vmstore/leandro/vm/out-gates/display.log`).

## Compute and latency

| Metric | native | v1 (Linux) | v2 Linux | v2 Windows | source |
|---|---|---|---|---|---|
| short kernel (1000 cycles), µs per launch, auto sync | 6 | 8 [7-9] | 23 [16-27], n=6 | - | P13, [a] |
| short kernel, blocking sync, µs | 21 | 15 [15-16] | 68 [42-237] | - | P13, [a] |
| 30M-cycle kernel, ms, auto sync | 16.3 | 16.5 | 17.1 | - | P13, [a] |
| 30M-cycle kernel, ms, blocking sync | 16.1 | 100 | 16.9 | - | P13, [a] |
| cuInit, ms | 29.4 | 29.9 | 12.1 | - | P13, [c] |
| cuCtxCreate, ms | 129 | 153 | 210 [184-264] | - | P13, [c] |
| cuCtxDestroy, ms | 71 | 71 | 96 | - | P13, [c] |
| cuMemAlloc 1 MiB, µs | 42 | 74 | 153 | - | P13, [c] |
| cuMemFree 1 MiB, µs | 31 | 42 | 254 | - | P13, [c] |
| cuMemAlloc 64 MiB, µs | 82 | 141 | 2124 | - | P13, [c] |
| cuMemFree 64 MiB, µs | 103 | 115 | 2125 | - | P13, [c] |
| copy h2d pinned 1 / 16 / 256 MiB, GB/s | 12.1 / 13.0 / 13.1 | 12.1 / 13.0 / 13.1 | 10.2 / 12.8 / 13.1 | - | P13, [a] |
| copy d2d 1 / 16 / 256 MiB, GB/s | 132 / 183 / 186 | 124 / 182 / 189 | 44.5 / 155 / 164 [132-190] | - | P13, [a] |
| RM call (`CHECK_VERSION_STR`), µs, p50 | 2.9 | 15.5 | 1.9 | - | P13, [a] |
| RM control 232 B / 49192 B, µs, p50 | 5.4 / 4.3 | 18.4 / 44.4 | 2.3 / 2.4 | - | P13, [a] |
| rlprobe, 50 episodes, s | 1.20 | 1.40 | 2.10 | - | P13, [a] |
| rlprobe, host transfers per s | 2350 | 2109 | 1398 | - | P13, [a] |
| matmul fp32 N=1024 / N=4096, GFLOP/s | 4894 / 6052 | 5304 / 6765 | 4570 / 6061 | - | P13, [a] |
| llama.cpp 1B Q4_K_M pp512, tokens/s | 7639 | 7769 ± 1352 | 7461 ± 514 | - | WS, [b] |
| llama.cpp 1B Q4_K_M tg128, tokens/s | 305 | 304 ± 7 | 243 ± 22 | - | WS, [b] |
| PyTorch transformer training, tokens/s | 107485 | 110242 | 109482 | - | WS, [b] |
| PyTorch ResNet-18 training, images/s | 532 | 530 [524-550], n=3 | 515 [497-549], n=3 | - | WS, [b] |

- v2's RM calls stay in the guest's own RM; v1 forwards every call to the host, and its
  blocking sync is a flat 100 ms poll.
- The v2 short kernel spreads: 16 to 27 µs over six runs of 100 launches, 24 to 53 µs over
  eight runs of 1000 (v1: 7 µs in all three). The 13 to 14 µs of 2026-09-28 came from binary
  945431f9 (`caraxes/runs/night-2026-09-28/series945-fast2/`), which did not boot in today's
  configuration (`R/ab-short/n945/`), so there is no same-day comparison.

## Graphics

v2 Linux on the GNOME desktop guest, native on the host's own Wayland desktop.
Records: `R/kick/smoke.txt`, `R/vm10/`, `R/sottr/`, `R/win-bench2/`.

| Metric | native | v1 (Linux) | v2 Linux | v2 Windows | source |
|---|---|---|---|---|---|
| glmark2-wayland 1280x720, score | 12214 | not measured[^v1] | 3324 | - | [b] |
| vkmark Wayland 1280x720, score | 15973, 15940 | not measured[^v1] | 2635, 2388 | - | [b] |
| glmark2 off-screen 1920x1080, score | 21486 (2026-08-07) | 21277 (2026-08-07) | not measured | - | [d] |
| vkmark, six scenes, immediate present, score | 8188 (2026-08-18) | 4655 (2026-08-18) | not measured | - | [d] |
| glmark2 `build`, 10 s, fps | not measured | not measured[^v1] | 3944 | - | |
| nouveau + NVK: glmark2-wayland (3 scenes, 5 s each), vkcube, vkmark | - | - | 3875; 131 fps; fails ([known issue 8](vfio/known-issues.md)) | - | |
| Shadow of the Tomb Raider, 1080p, texture, shadow and LOD 3, VSync off, average fps | 91.7 (2026-08-20) | 68.2, 70.1 (2026-08-20) | fails at start ([known issue 2](vfio/known-issues.md)); 34.7 in the morning's gate | not measured | [e] |
| D3D11 stages (triangle to tessellation) | - | - | - | 12 of 12 | |
| Unigine Heaven 4.0, 1080p High, tessellation normal, windowed, fps | - | - | - | 152.6 | |
| Unigine Superposition, 1080p Medium, points | - | - | - | 11656 (87.2 fps, min 14.9) | |
| 3DMark Steel Nomad (DX12) | - | - | - | fails ([known issue 1](vfio/known-issues.md)), not run | |

- glmark2 `build` on the same binary this morning: 4501 to 4756, and 432 once
  (`caraxes/runs/gate-2026-09-30/stack/RESULT.md`).
- v1 SotTR at texture and shadow 1, LOD 2: 67.2 fps (2026-08-21) and 48.0 (2026-09-19) [e].
  The native 91.7 is cited in `caraxes/runs/night-2026-09-28/README.md`; its result file is
  not in the records.
- Windows earlier on this line: Heaven 140.4 and Superposition 11622 in the morning's gate,
  Heaven 152 to 158 and Superposition up to 12247 in
  [windows-findings](research/windows-findings.md) step 23. No native Windows value exists on
  this card (the host runs Linux).

## Video and streaming

ffmpeg N-126965 on 30 s 1080p60 clips (1800 frames), the scripts of
`caraxes/runs/codec-2026-09-30`; Sunshine in the guest and Moonlight on the host at 1080p60
and 20 Mbit/s over 150 s, with vkcube (Linux) or a 60 fps D3D11 present loop (Windows) as
load. Records: `R/codec-linux-b1/`, `R/win-bench2/`, `R/kick/`, `R/v2-compute/`.

| fps unless stated | native | v1 (Linux) | v2 Linux | v2 Windows | source |
|---|---|---|---|---|---|
| NVENC H.264, from NVDEC / from the CPU | not measured | not measured[^v1] | 414 / 409 | 311 / 190 | |
| NVENC HEVC, from NVDEC / from the CPU | not measured | not measured[^v1] | 333 / 326 | 281 / 175 | |
| NVDEC H.264: cuda / cuvid / d3d11va | not measured | not measured[^v1] | 661 / 650 / - | 574 / 375 / 343 | |
| NVDEC HEVC: cuda / cuvid / d3d11va | not measured | not measured[^v1] | 1307 / 1189 / - | 777 / 343 / 487 | |
| NVDEC VP9: cuda / cuvid / d3d11va | not measured | not measured[^v1] | 900 / 874 / - | 612 / 327 / 432 | |
| decode bit-exact against software (framemd5) | - | - | 1800 of 1800 | 1800 of 1800 | |
| AV1 encode and decode | - | - | refused cleanly (TU106 has neither) | refused cleanly | |
| `nvenc-probe` 300 frames H.264: encode / decode | 166, 180 / 141, 143 | not measured[^v1] | desktop 188, 178 / 154, 166; compute 154, 144 / 122, 123 | not measured | [b] |
| Sunshine nvenc: frames rendered per s | not measured | 58.8 (2026-09-19, X session) | 59.9 (HEVC) | 59.8 | [f] |
| Sunshine nvenc: host latency, ms, average (min-max) | not measured | 6.8 (5.0-23.3) (2026-09-19) | 5.3 (3.5-23.8) | 6.9 (4.4-74.2) | [f] |
| Sunshine nvenc: Sunshine / Cloud Hypervisor / device, % of a core | not measured | not measured | 22.5 / 59.1 / 48.3 | 15.9 / 102 / 50.3 | |
| Sunshine x264: frames rendered per s | not measured | not measured | 59.8 | 58.8 | |
| Sunshine x264: host latency, ms, average (min-max) | not measured | not measured | 17.5 (13.4-78.5) | 12.6 (7.6-85.1) | |
| Sunshine x264: Sunshine / Cloud Hypervisor / device, % of a core | not measured | not measured | 138 / 173 / 50.5 | 205 / 274 / 32.9 | |

## Leandro-Test suite

`lt-all.sh` (`caraxes/runs/ctxcreate-2026-09-29`): the CUDA, suite and gpu tests of
`tools/caraxes-guest-tests.sh`, every `probe/matrix` probe, the C and Python probes and
`bench-cu`, 65 tests in one guest, one after another. Both ran in compute guests without a
display or ffmpeg. Records: `R/v1b/rows.tsv`, `R/lt/v2-lt/rows.tsv`.

| | v1 (Linux) | v2 Linux | v2 Windows |
|---|---|---|---|
| pass / fail / skip | 19 / 36 / 10 | 36 / 19 / 10 | - |

| Failing tests | v1 | v2 Linux | Cause | Kind |
|---|---|---|---|---|
| cuda-managed, gpu-kernel, managedprobe | pass, fail, fail | fail | v2: `cudaMallocManaged` refused (801), no replayable faults ([known issue 6](vfio/known-issues.md)); v1: the row below | v2: known gap |
| robustness | fail | pass | v1: the parallel and SIGKILL parts pass; the check reads `nvidia.ko`'s refcount, which a v1 guest does not have | harness |
| 20 CUDA tests after robustness (async streams, gpu-torch, gpu-own, nvprobe-4, hostregprobe, oomprobe, vrampress, the torch probes, mmsweep, mtprobe, streamprobe, pinwin, convoom, vramcap, bench-init, -churn, -copy, and the two above) | fail | pass | v1: every `cuCtxCreate` after the robustness test fails (304) until the VM restarts; the restarted VM's cuda gate passes 14 of 14 ([open question 84](virtio/open-questions.md#84-cuda-contexts-fail-after-the-robustness-test)) | v1 defect |
| gpu-encode, matrix-nvenc, matrix-nvdec, matrix-vk-offscreen | fail | fail | no ffmpeg in the compute images | environment |
| matrix-opencl, matrix-vk-rt, matrix-egl-gbm, -wayland, -xcb, -xlib | fail | fail | these probes are not built in the guests' Leandro-Test copy | environment |
| matrix-vk-enum, matrix-gl-enum, matrix-gles | fail | fail | no NVIDIA GL or Vulkan ICD in the compute images (GL falls back to Mesa) | environment |
| matrix-rm-direct | pass | fail | v2: the display class 0x0073 is refused, because the compute VM runs without a display | environment |
| ctrlping | fail | fail | ran clean (2000 calls, 0 fails); the criterion expects the 16384-byte line, the probe prints 49192 | harness |
| bench-launch | fail | fail | this `bench-cu` build has no `launch` command | harness |

- The 10 skips are the same on both: cuda-gdb, cupy and cuDF not installed; NvOFA, NvFBC,
  PKCS#11 and Vulkan SC declared unsupported; two probes that need a kernel trace point.

## Stability and gates

| Check | v1 (Linux) | v2 Linux | v2 Windows |
|---|---|---|---|
| Linux kick-smoke: vkcube Wayland and X11, glmark2, nvenc-probe, NVENC holder, a changing Moonlight picture | - | pass, guest Xid 0 (`R/kick/`) | - |
| nova-core: probe, three unbind/bind cycles, reload | - | pass, 5 of 5 (`R/nova/`) | - |
| nouveau + NVK R1-R8 | - | checks pass (offscreen and Weston hashes); vkmark fails ([known issue 8](vfio/known-issues.md)) (`R/vm10/`) | - |
| Session: D3D11 stages, Heaven, Superposition, codec matrix, two streams, one reboot | - | - | pass; 0 host Xid, 0 FLR; six `LiveKernelEvent 141` reports during the codec matrix ([known issue 9](vfio/known-issues.md)) |
| Guest reboot | - | - | back after 65 s, served as the device's second client |
| Shadow of the Tomb Raider | ran (2026-08-20) [e] | fails at start ([known issue 2](vfio/known-issues.md)) | not measured |
| nvml gate | - | pass | - |
| gpu gate, compute guest | - | fails: smi (host `Disp.A On`, guest `Off`), kernel (managed memory), encode (no ffmpeg), torch (no native reference) | - |
| smi gate | fails only on the `-4G` suffix the VRAM cap adds to the name (`R/v1b/gates/`) | - | - |
| cuda gate | 14 of 14 | - | - |
| Host Xid, 10:30 to 13:00 | 0 | 0 | 0 |

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
8. **v1: 3D and SotTR**: SotTR 0.75 of native (2026-08-20), vkmark 0.57 (2026-08-18): every
   RM call and every mapping is forwarded ([leandro.md](leandro.md#12-strengths-weaknesses-and-the-road-to-christmas-2026)).
9. **v1: forwarded RM calls**: 15.5 µs per call (5.3×) and 18 to 44 µs per control (3.4 to
   10×) ([leandro.md](leandro.md#12-strengths-weaknesses-and-the-road-to-christmas-2026), weakness 16).
10. **v1: lost contexts and retained pins**: after Leandro-Test's robustness test every new
    context fails until the VM restarts ([open question 84](virtio/open-questions.md#84-cuda-contexts-fail-after-the-robustness-test));
    freed pinned registrations stay charged until the guest restarts
    ([open question 83](virtio/open-questions.md#83-conservative-pin-retention-can-exhaust-the-vm-budget)).

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
  same workload scripts run on the host; `run.sh` never loads the host card.

## Not measured

- Native today: the card drives the user's desktop. No native record exists for the ffmpeg
  codec matrix, Sunshine, glmark2 `build`, nouveau, nova or Windows (the host runs Linux).
- v1 Windows: v1 has no Windows guest.
- llama.cpp with Qwen 7B (`-ngl 20`): the model is not staged in the guests.
- Two or more GPU VMs at once ([known issue 4](vfio/known-issues.md)).

[^v1]: The v1 615 image carries 610's GL user space and no NVENC or NVDEC libraries; they
    need restaging to 615 ([v1 615 image](vfio/v1-615-image.md)). Today's glmark2 off-screen
    attempt failed on it (`R/v1-gl/`).

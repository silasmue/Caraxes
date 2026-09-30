<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Performance shape

Where each architecture is native, where it pays, and what one unit of each costs. All numbers
are from the RTX 2070 (host 615.71.09) with the host desktop running; a ratio is virtual over
native (above 1 slower for times, below 1 slower for rates). Each row carries its date; a
2026-09-25 row stays where nothing newer exists.

## The units

| Cost | VirtIO Leandro | VFIO Leandro |
|---|---|---|
| one trapped access (exit, Cloud Hypervisor, socket) | - | 23 to 27 us p50 on a quiet host; 35.5 us read, 40.0 us write beside two VMs |
| one datamatch ioeventfd; one read of a mapped page | - | 1.28 us (1.91 us inside a read-only slot); about 1 ns |
| one RM call | 13 to 35 us (a forwarded ioctl) | 0 (the guest kernel answers) or one GSP RPC: four traps and 267 us of device time |
| one submission | 0 (shared mapping) | 0 traps through a token; 25 to 30 us for a trapped doorbell |
| one interrupt | an event on queue 1 | Linux 0 traps (tagged page, re-arm ioeventfd); Windows about 9 trapped tree stores (exact mode) |
| one mapping change | a few forwarded calls | 5 invalidate accesses of 35 to 40 us plus 22 to 50 us of sync at `TRIGGER`; a flush read of 35 to 40 us with a 0.4 to 0.8 us memfd sync |
| one blocking synchronisation | 100 ms (a poll fallback) | mostly native; a launch after 30 ms idle 680 us against 155 to 177 us (2026-09-25) |
| startup, boot | 0.19 s to the first cuInit | Linux about 75k traps (about 2 s) per boot, Windows about 1 M (about 27 s of vCPU) |

- VFIO rows: the train binary, 2026-09-30 ([research: trap-census-2026-09-30](../research/trap-census-2026-09-30.md)); VirtIO: [caraxes/benchmark.md](../caraxes/benchmark.md) §5.

## CUDA per operation (615 guest)

| Metric | native | v2 | ratio | Date |
|---|---|---|---|---|
| short kernel, ms per launch | 0.006 | 0.015 | 2.5 | 2026-09-25 |
| 30M-cycle kernel, ms | 16.3 | 17.3 | 1.06 | 2026-09-25 |
| rlprobe stage 4, s | 1.20 | 2.00 | 1.67 | 2026-09-25 |
| h2d pinned 256 MiB / 1 MiB, GB/s | 13.1 / 12.1 | 13.1 / 6.08 | 1.00 / 0.50 | 2026-09-25 |
| d2d 16 MiB, GB/s | 183 | 17.6 (171 in one rep) | 0.10 | 2026-09-25 |
| cuInit, ms | 29.4 | 13.9 | 0.47 | 2026-09-29 |
| cuCtxCreate, ms | 129 | 240 to 256 | 1.9 to 2.0 | 2026-09-30 |
| cuCtxDestroy, ms (median) | 71 | 108 to 116 | 1.5 to 1.6 | 2026-09-30 |
| cuMemAlloc / cuMemFree 1 MiB, us | 42 / 31 | 280 / 456 | 6.7 / 15 | 2026-09-29 |
| cuMemAlloc / cuMemFree 64 MiB, us | 82 / 103 | 2456 / 2434 | 30 / 24 | 2026-09-29 |
| cuMemHostRegister 1 MiB, us | 348 | 3441 | 9.9 | 2026-09-29 |
| root h2d / d2h 16 MiB, ms | 1.31 | 1.41 to 1.51 | 1.1 | 2026-09-30 |

- 2026-09-25: vm5-615, 1 GiB, fast paths on ([research: performance-facts](../research/performance-facts.md)); 2026-09-29: vm2-615,
  2 GiB, trace off ([research: ctxcreate-2026-09-29](../research/ctxcreate-2026-09-29.md)); 2026-09-30: vm2-615, 1 GiB, the gather cache,
  beside another VM, no pool (`caraxes/runs/train-2026-09-30/NOTE.md`).
- Pinned copies follow the PCIe link state (6.5 or 12.9 GB/s); allocations spread up to 2x between sessions.

## Compute workloads (615 guest, 2 GiB, 2026-09-25)

| Workload | native | v2 | ratio |
|---|---|---|---|
| PPO CartPole 420 s, steps/s (submission-bound: 407 s of device CPU in 424 s) | 116313 | 74714 | 0.64 |
| llama.cpp pp512 / tg128, tokens/s | 8475 / 295 | 8150 / 272 | 0.96 / 0.92 |
| transformer training, tokens/s | 103760 | 104754 | 1.01 |
| ResNet-18 training, images/s | 506 | 526 | 1.04 |

## Graphics

| Workload | Guest | Result | Record |
|---|---|---|---|
| glmark2 / vkmark, 1280x720 | Linux 615, train, beside two VMs | 4410 / 3922 | trap-census L0 |
| glmark2 / vkmark on weston | nouveau + NVK, quiet card, BAR2 memfd | 4383 / 5632; `vkCreateDevice` 48 ms (13385 ms without the memfd) | [research: open-render-2026-09-29](../research/open-render-2026-09-29.md) |
| SotTR 1080p benchmark | Linux 615, 4 GiB, display reserve | average 15.4 FPS, max 106.7 (native 91.7), 2026-09-29 | [research: sottr-invgate-2026-09-29](../research/sottr-invgate-2026-09-29.md) |
| Heaven 4.0 DX11 1920x1080 High | Windows 617.14 | 152.5 to 158.3 fps (C116 to C118) | [research: windows-findings](../research/windows-findings.md) step 23 |
| Superposition 1.1 1080p Medium | Windows 617.14 | 12160 to 12247 (C117, C118); min fps 52 to 59 in C117 | same |

- No native Heaven or Superposition run on this card is recorded; the note gives about 12k to
  13k for Superposition on a desktop RTX 2070. SotTR has not run on the train binary.

## Video (2026-09-30, one binary, 1080p60 clips of 1800 frames)

| Test | Linux 615, fps | Windows, fps |
|---|---|---|
| NVENC H.264: NVDEC-CUDA-NVENC / CPU source | 419.9 / 406.2 | 315.1 / 178.6 |
| NVENC HEVC: NVDEC-CUDA-NVENC / CPU source | 333.3 / 325.3 | 183.9 / 176.9 |
| NVDEC H.264, HEVC, VP9 (`-hwaccel cuda`) | 650.1, 1310.0, 900.0 | 587, 636, 584 |

- Every decode is bit-exact against the host's software decode; AV1 fails cleanly on both (TU106
  has none). The Windows NVDEC-CUDA-NVENC rows ran 1800 frames, but their count check had a
  script bug. No native codec run is recorded (`caraxes/runs/codec-2026-09-30/matrix.md`).
- Sunshine on Linux 615, card alone, 60 fps: nvenc against x264 host latency 5.9 against 18.3 ms,
  guest Sunshine CPU 23.7 against 142.3 % of a core, Cloud Hypervisor 56.1 against 175.4 %.
- Sunshine on Windows (static desktop, 21 fps): nvenc latency 31 to 70 ms against x264's 12 to 14 ms; open.

## Trap rates per workload

| Workload (2026-09-30) | traps/s | modelled vCPU share | top sources, /s |
|---|---:|---:|---|
| Linux glmark2 | 3206 | 13.9 % | flush read 1407, `TIME` 907, invalidate 704 |
| Linux Moonlight stream, 60 fps | 4300 | 23.2 % | flush read 1620, invalidate 1323, `TIME` 1177 |
| Linux CUDA loop | 474 | 2.2 % | the idle desktop's; 6087 rings/s all on the token pool |
| Windows Heaven, 133 fps | 3290 + about 5330 routed | 9.9 + 14 % | tree stores, BAR2 1228, PTIMER 896, doorbell 497 |

- Every workload, the host CPU and the cost model: [traps-and-fast-paths](traps-and-fast-paths.md).

## Where the remaining time goes (v2)

- Windows: exact-mode tree stores (12 to 14 % of a vCPU in 3D) and BAR2 writes without the memfd
  (58 % of a vCPU at desktop start). Linux: the invalidate, flush-read and clock traps. The
  census plan expects Heaven at about 250 traps/s and glmark2 at about 190.
- cuCtxCreate (2026-09-29): the first channel build waits for the context's first plan; 30 gathers
  of libcuda's scattered sysmem took 91.3 ms; per process 129 ms of gathers, 67 ms of RPCs, 64 ms
  of memfd syncs. The gather cache took 52 to 60 ms off.
- SotTR (2026-09-29): 624 invalidates/s at 889 us of device time and 648 BAR2 full syncs/s at 452 us.
- The software executor: about 1.1 ms of device time per push of a guest kernel channel.
- TU106's BAR1 (256 MiB, resizable only to 64, 128, 256 MiB) makes the pool copy and the device
  window its slice; a BAR1 that covers VRAM would not ([bar1-vram-window](bar1-vram-window.md)).

## v1 against v2 (610 closing series, 2026-09-24, quiet host, n=5)

| Metric | native | v1 | v2 |
|---|---|---|---|
| short kernel, ms per launch | 0.006 | 0.009 | 0.015 |
| cuCtxCreate, ms | 195 | 165 | 3770 |
| cuMemAlloc 1 MiB, us | 35.8 | 93.4 | 830 |
| h2d pinned 1 MiB, GB/s | 12.1 | 12.2 | 6.06 |
| matmul N=2048, GFLOP/s | 5494 | 5569 | 2248 |

- The only interleaved series of both ([caraxes/benchmark.md](../caraxes/benchmark.md), closing
  series). v1 is native on GPU-bound work and pays per RM call and per blocking sync; v2's gap
  sits in mapping changes and per-ring work.

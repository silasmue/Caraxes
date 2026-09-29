<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Performance shape

Where each architecture is native, where it pays, and what one unit of each cost is. All
numbers are from the RTX 2070 with the host desktop running; ratios are the virtual path
over native (above 1 slower for times, below 1 slower for rates).

## The units

| Cost | VirtIO Leandro | VFIO Leandro |
|---|---|---|
| one RM call | 13 to 35 us (a forwarded ioctl) | 0 (the guest kernel answers) or one RPC of four traps, 108 us median, when routed to GSP |
| one submission | 0 (shared mapping) | 0 with the windows; 36 us for a trapped doorbell, 4 to 6 us for a pooled token |
| one trapped BAR0 access | - | 23 to 27 us quiet, 28 to 35 us busy; 1 to 2 us of it in the device |
| one blocking synchronisation | 100 ms (a poll fallback) | mostly native; a launch after 30 ms idle 680 us against 155 to 177 us |
| one mapping change | a few forwarded calls (about 2x) | the guest RM's 24 trapped accesses (18 lock toggles, 5 invalidate, 1 flush) without windows; 4.3 traps per 1 MiB allocation with them; a memfd sync of 126 us mean |
| one interrupt | an event on queue 1 | 60 trapped accesses without the interrupt page, 1 to 2 us with it; the re-arm 30 to 50 us until the ioeventfd |
| startup | 0.19 s | about 3.4 s (the boot exchange, BAR2 bootstrap, the first context's 20 channels) |

## Per operation (615 guest, 1 GiB profile, windows on)

| Metric | native | v2 | ratio |
|---|---|---|---|
| short kernel, ms per launch | 0.006 | 0.015 | 2.5 |
| 30M-cycle kernel, ms | 16.3 | 17.3 | 1.06 |
| rlprobe stage 4, s | 1.20 | 2.00 | 1.67 |
| h2d pinned 256 MiB, GB/s | 13.1 | 13.1 | 1.00 |
| h2d pinned 1 MiB, GB/s | 12.1 | 6.08 | 0.50 (bimodal with the PCIe link state) |
| d2d 16 MiB, GB/s | 183 | 17.6 (171 in one rep) | 0.10 |
| cuInit, ms | 106 | 14.8 | 0.14 |
| cuCtxCreate, ms | 195 | 323 | 1.7 |
| cuCtxDestroy, ms | 92.6 | 116 | 1.25 |
| cuMemAlloc 1 MiB, us | 35.8 | 201 | 5.6 |
| cuMemFree 1 MiB, us | 27.1 | 339 | 12.5 |
| cuMemAlloc 64 MiB, us | 78.3 | 614 | 7.8 |

## Per workload (615 guest, 2 GiB profile)

| Workload | native | v2 | ratio |
|---|---|---|---|
| PPO CartPole 420 s, steps/s | 116313 | 74714 | 0.64 |
| llama.cpp pp512 / tg128, tokens/s | 8475 / 295 | 8150 / 272 | 0.96 / 0.92 |
| transformer training, tokens/s | 103760 | 104754 | 1.01 |
| ResNet-18 training, images/s | 506 | 526 | 1.04 |
| Weston / Mutter presented | 60 Hz mode | 60.00 / 59.40 Hz | - |

- Device-bound work runs at 0.88 to 1.04 of native; submission-bound PPO at 0.64 to 0.67,
  bound by the device's per-ring work (407 s of device CPU in a 424 s run).

## Where the remaining time goes (v2)

- Per CUDA process about 0.55 s outside every benchmark row: 1024 BAR1 pool page moves
  (363 ms median) and 834 BAR2 memfd syncs (180 ms).
- Host TSG preempts from the pool's warm-move barrier: 3.9 s in 1112 calls over the
  coverage map.
- The invalidate and flush path on every allocation; the untrapped-GPPut hold on frees
  (64 MiB free 791 to 2402 us).
- The RPC transport: 108 us of traps per RPC against 3 us of device work, 187 RPCs per CUDA
  process; the HOST class forwards (1.85 s of GSS-legacy over the profiles).
- Non-trap stalls (D2D 16 MiB 3.85 ms against 0.25, a launch after idle) without a device-side
  timeline yet.
- Idle interrupt forwarding of other clients' work: 331 MSIs per second.

## v1 against v2 (610 closing series, quiet host, n=5)

| Metric | native | v1 | v2 (then) | v2 (matrix, 615) |
|---|---|---|---|---|
| short kernel, ms per launch | 0.006 | 0.009 | 0.015 | 0.015 |
| cuCtxCreate, ms | 195 | 165 | 3770 | 323 |
| cuMemAlloc 1 MiB, us | 35.8 | 93.4 | 830 | 201 |
| h2d pinned 1 MiB, GB/s | 12.1 | 12.2 | 6.06 | 6.08 |
| matmul N=2048, GFLOP/s | 5494 | 5569 | 2248 | - |
| module load to first cuInit, ms | - | 218 | 3509 | - |

- v1 is native on GPU-bound work and near native on allocation and context paths, and pays
  per RM call, per blocking sync and a pinned-page retention until guest restart. v2's gap is
  not per call: it sits in the device's handling of mapping changes and in the per-ring work,
  and it moved from 19x to 1.7x on context creation between the two series.

## The ReBAR outlook

- TU106's BAR1 (256 MiB, resizable only to 64, 128, 256 MiB) forces the pool to copy and the
  device to window its slice. A card whose BAR1 covers its VRAM lets each VM own its window
  and the device map its slice once; zero-copy pool moves then need a VMM patch
  ([bar1-vram-window](bar1-vram-window.md)).

## Depth

- [research/performance-facts.md](../research/performance-facts.md): every ratio with its run.
- [caraxes/benchmark.md](../caraxes/benchmark.md): the harness and every series.
- [caraxes/optimization-backlog.md](../caraxes/optimization-backlog.md), [caraxes/coverage-map.md](../caraxes/coverage-map.md):
  the census, the ranked backlog, the costs outside the benchmarks.
- [caraxes/workload-gate.md](../caraxes/workload-gate.md): the workloads.

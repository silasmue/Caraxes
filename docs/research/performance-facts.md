<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Performance facts

Scope: ratios of Caraxes to native on the RTX 2070 (host 615.71.09) per operation and per workload, where the remaining time goes, and what ReBAR would change.

## Facts

- Native is the closing series' host column (quiet host, n=5) unless a table says otherwise, and a ratio is Caraxes median / native median, so above 1 is slower for times and below 1 is slower for rates [docs/caraxes/benchmark.md §2, §Closing series on driver 610.57.04; run: caraxes/runs/closing-610-2026-09-24/summary.md].
- "Windows on" is the recommended CUDA configuration: `CARAXES_INTR_PAGE=1`, `CARAXES_BAR1_PASSTHROUGH=pool`, `CARAXES_DOORBELL_TOKENS=64`, `CARAXES_DOORBELL_POLL=hybrid`, `CARAXES_BAR2_MEMFD=1`, `CARAXES_MSI_REARM_IOEVENTFD=1` [caraxes/tools/bench-run.sh:37-41].

### Per operation: night series part 3 and morning matrix

Caption: 615 guest, 1 GiB profile, restart per rep. Night = vm2, frozen 57693d8, BAR1 pool, interrupt page, spin reactor, no BAR2 memfd, other VMs active in 25 of 65 runs. Matrix = vm5-615, frozen fbe4dfb, windows on plus `CARAXES_ALLOC_LAZY=1`, no P21 area, no other device active.

| Metric | native | night | night / native | matrix | matrix / native |
|---|---|---|---|---|---|
| short kernel, ms/launch, auto | 0.006 | 0.018 | 3.0 | 0.015 | 2.5 |
| rlprobe stage 4, s | 1.20 | 2.60 | 2.2 | 2.00 | 1.67 |
| h2d pinned 1 MiB, GB/s | 12.1 | 6.07 | 0.50 | 6.08 | 0.50 |
| d2d 16 MiB, GB/s | 183 | 16.5 | 0.09 | 17.6 | 0.10 |
| h2d pinned 256 MiB, GB/s | 13.1 | 13.1 | 1.00 | 13.1 | 1.00 |
| cuInit, ms | 106 | 198 | 1.9 | 14.8 | 0.14 |
| cuCtxCreate, ms | 195 | 4444 | 23 | 323 | 1.7 |
| cuCtxDestroy, ms | 92.6 | 3057 | 33 | 116 | 1.25 |
| cuMemAlloc 1 MiB, us | 35.8 | 238 | 6.6 | 201 | 5.6 |
| cuMemFree 1 MiB, us | 27.1 | 463 | 17 | 339 | 12.5 |
| cuMemAlloc 64 MiB, us | 78.3 | 4919 | 63 | 614 | 7.8 |
| PPO 60 s, steps/s | 107663, 112259 | 64423, 51178 | 0.54 | 75070, 72706 | 0.67 |

[docs/caraxes/benchmark.md §Night series Part 3, §Morning matrix Benchmarks; run: caraxes/runs/bench-615-2026-09-24/night/comparison.md; run: caraxes/runs/matrix-2026-09-25/series-615]

- Long kernels run at native speed: the 30M-cycle launch takes 17.3 ms against 16.3 ms (1.06) [docs/caraxes/benchmark.md §Night series Part 3].
- The matrix copy rows are bimodal per device instance: the 615 guest's h2d stayed at 6.1 GB/s in all three reps while its d2d 16 MiB reached 171 GB/s in rep 3 [docs/caraxes/benchmark.md §Morning matrix Benchmarks; run: caraxes/runs/matrix-2026-09-25/series-615].
- The pinned 1 and 16 MiB copies switch between about 6.5 and 12.9 GB/s with the card's PCIe link state (gen 1 or 2 at P8 or P5, gen 3 at P0) [docs/caraxes/optimization-backlog.md §4 Status, item 1; run: caraxes/runs/opt-memfd-pool-2026-09-25].
- The Xid 31 fail-closed rule added after the matrix costs cuMemAlloc 1 MiB 175 to 200 us, 64 MiB 511 to 2277 us, cuMemFree 64 MiB 791 to 2402 us and cuCtxCreate 395 to 486 ms on vm5-615, launches unchanged [docs/caraxes/README.md §Status 2026-09-25 "The morning matrix's Xid 31"; run: caraxes/runs/xid31-pool-2026-09-25].

### Per operation: the census

Caption: 610 guest, vm3, 1 GiB slot 5, other VMs up (load 7 to 9). `off` = frozen 1ea2529 with interrupt page, BAR1 pool 32 MiB, token pool, hybrid reactor; `on-nopool` = the same plus BAR2 memfd, without the pool; matrix = fbe4dfb, windows on (memfd and pool together); native = the census's own host run. Traps are per operation, gross.

| Operation | traps `off` | `off` / native | traps `on-nopool` | `on-nopool` / native | traps matrix | matrix / native |
|---|---|---|---|---|---|---|
| cuInit | 3290 | 1.9 | 174 | 0.22 | 174 | 0.23 |
| cuCtxCreate | 75070 | 28 | 1884 | 6.0 | 1290 | 3.0 |
| cuCtxDestroy | 21507 | 14 | 924 | 2.1 | 828 | 2.0 |
| cuMemAlloc 1 MiB | 5.6 | 4.5 | 3.6 | 4.7 | 3.5 | 4.8 |
| cuMemFree 1 MiB | 9.4 | 6.7 | 6.6 | 8.9 | 6.5 | 9.5 |
| cuMemAlloc 64 MiB | 188 | 34 | 8.0 | 6.5 | 7.8 | 7.3 |
| cuMemAllocHost 1 MiB | 807 | 33 | 8.2 | 2.2 | 7.5 | 2.5 |
| cuMemHostRegister 1 MiB | 1049 | 137 | 15.2 | 6.1 | 15.0 | 7.3 |

[docs/caraxes/optimization-backlog.md §1, §2; docs/caraxes/benchmark.md §Morning matrix Census; run: caraxes/runs/trap-census-2026-09-25/summary.md; run: caraxes/runs/matrix-2026-09-25/census-610/census-table.md]

- With the windows on, a submission, a copy, an event and a synchronise trap 0 times net of background, and PPO submits 28000 times per second with no trap per submission [docs/caraxes/optimization-backlog.md Summary 1, §3.5; run: caraxes/runs/trap-census-2026-09-25/workload-rates.md].

### Per workload: the workload gate

Caption: 615 guest, v2 against native only (v1 dropped on the 615 host), 3 reps. Night = vm2-615, 1 GiB slot 3, frozen v615-57693d8, pool, token pool, hybrid, interrupt page, no memfd, quiet host. Matrix = vm2-615, 2 GiB slot 0, fbe4dfb, windows on plus lazy plans; its native column is the night's.

| Workload | native | night | night / native | matrix | matrix / native |
|---|---|---|---|---|---|
| short kernel, ms/launch | 0.006 | 0.020 | 3.3 | 0.016 | 2.7 |
| rlprobe stage 4, s | 1.20 | 2.40 | 2.0 | 1.80 | 1.5 |
| PPO 420 s, steps/s | 116313 | 74056 | 0.64 | 74714 | 0.64 |
| PPO time to solved, s | 150 | 234 | 1.56 | 238 | 1.6 |
| llama.cpp pp512, tokens/s | 8475 | model does not load (791 MiB visible) | - | 8150 | 0.96 |
| llama.cpp tg128, tokens/s | 295 | does not load | - | 272 | 0.92 |
| llama.cpp tg128 without CUDA graphs | 293 | does not load | - | 257 | 0.88 |
| transformer, tokens/s | 103760 (batch 32) | 87831 (batch 8) | 0.85, not like for like | 104754 (batch 32) | 1.01 |
| ResNet-18, images/s | 506 (batch 32) | 465 (batch 16) | 0.92, not like for like | 526 (batch 32, n=2) | 1.04 |

[docs/caraxes/workload-gate.md §5, §7; docs/caraxes/benchmark.md §Morning matrix Workload gate; run: caraxes/runs/workload-2026-09-24/night/gate/summary.md; run: caraxes/runs/matrix-2026-09-25/workload-615/summary.md]

- PPO is bound by the device's per-ring work: 407 s of device CPU in a 424 s run in the matrix and 411 s of 427 s in the night [docs/caraxes/benchmark.md §Morning matrix Workload gate; run: caraxes/runs/matrix-2026-09-25/workload-615/v2/ppo-full-1.txt; docs/caraxes/workload-gate.md §5, §7].

### Display frame rates

Caption: 615 rows = vm0-615, 2 GiB slot 0, host execution, token pool, hybrid reactor, `CARAXES_INTR_PAGE=1 CARAXES_DISPLAY_PAGE=1` (configuration C); stream row = 610 guest, display-e series F, before the interrupt page; the reference is the 60 Hz mode, no native display run exists.

| Measure | Value | Evidence |
|---|---|---|
| Mutter, presented / frame callbacks | 59.40 Hz / 58.62 Hz (sd 7.70 ms) | display-615.md §5 |
| Weston GL, presented / frame callbacks | 60.00 Hz / 59.79 Hz (sd 1.98 ms) | display-615.md §5 |
| trapped accesses per presented frame, Mutter / Weston | 138.92 / 29.25 | display-615.md §5 |
| Sunshine kms + x264 to Moonlight | 25.85 FPS, host latency avg 101 ms, 2.09 guest cores | display-e.md §1, §7 |

[run: caraxes/runs/display-615-2026-09-24/s4-series/trap-compare.txt; run: caraxes/runs/matrix-2026-09-25/display-615; run: caraxes/runs/display-f-2026-09-24]

- No Caraxes SotTR frame rate exists; the stage ran only its plumbing test in pure emulation [docs/caraxes/sottr-stage.md §6, §7].

### Where the remaining time goes

- cuCtxCreate after the batched shadow maps is the GPU's first work on each of its 20 channels, not host RM calls (13171 to 228 host calls per creation) [docs/caraxes/optimization-backlog.md §4 Status of item 2; run: caraxes/runs/opt-shadow-maps-2026-09-25].
- An allocation keeps the MMU invalidate (page 0xb83000) and the BAR2 flush read (0xfea000), and with the memfd each flush read carries a sync of 126 us mean, which makes the free path the larger gap [docs/caraxes/optimization-backlog.md §2.1, §3.2; docs/caraxes/benchmark.md §What closes the gaps].
- Every CUDA process pays about 0.55 s of device time no benchmark row names: 1024 BAR1 pool page moves (363 ms median) and 834 BAR2 memfd syncs (180 ms) [docs/caraxes/coverage-map.md Summary, §4 items 1-2; run: caraxes/runs/coverage-2026-09-25/combined-coverage-map.md].
- Host TSG preempts, mostly from the pool's warm-move barrier, cost 3.9 s in 1112 calls over the map, and one preempt at a kill -9 took 3.2 s with host Xid 109 [docs/caraxes/coverage-map.md §4 item 3; run: caraxes/runs/coverage-2026-09-25/map].
- D2D 16 MiB (3.85 against 0.25 ms), H2D 16 MiB pinned (1.9 to 5.2 against 1.3 ms) and a launch after 30 ms idle (680 against 155-177 us) have no trap in their window [docs/caraxes/optimization-backlog.md §3.4].
- 610 delta: the 610 guest matches the 615 guest within run spread on the same device configuration (first 615 series, night part 3, matrix), the census and display-e ran the 610 guest, and only 615 guest devices showed the GPC FAULT_PDE Xid 31 in the matrix [docs/caraxes/benchmark.md §First 615 series, §Night series Part 3, §Morning matrix; run: caraxes/runs/bench-615-2026-09-24/comparison.md].

### ReBAR

- TU106's BAR1 is 256 MiB, and its resizable-BAR capability offers only 64, 128 and 256 MiB [docs/caraxes/optimization-backlog.md §5].
- The pool plus the device's own BAR1 windows must stay under 192 MiB beside the desktop, so the census and the matrix ran the pool at 32 MiB [docs/caraxes/optimization-backlog.md §1, §5].
- The device reaches the rest of the reservation through two 32 MiB BAR1 windows at 1.6 us per uncached read and 2.5 ms per window miss [docs/caraxes/optimization-backlog.md §5; docs/caraxes/benchmark.md §Doorbell over KVM ioeventfd].
- With BAR1 covering VRAM the pool could cover all of guest BAR1 and the device could map the whole reservation once, but zero-copy pool moves still need a VMM patch that re-points one 4 KiB page of the area mapping [docs/caraxes/optimization-backlog.md §5].

## Consequence for Caraxes

- Device-bound workloads on the 615 guest run at 0.88-1.04 of native with the 2 GiB profile, and submission-bound PPO at 0.64-0.67, so per-ring device work is the lever for PPO-like loads.
- Allocation and free (5.6x and 12.5x in the matrix) and the invalidate and flush path are the largest per-operation gaps left.
- Pool moves, memfd syncs and barrier preempts are device time outside every benchmark row, so the profile measures their fixes (see [methods.md](methods.md)).

## Open

- Holding only plans that map something new would recover the Xid 31 rule's cost on frees and is not built [docs/caraxes/README.md §Status 2026-09-25].
- The non-trap stalls (D2D and H2D 16 MiB, launch after idle) need a device-side timeline of host doorbell, GPGet and notifier [docs/caraxes/optimization-backlog.md §4 item 10].
- Invalidate and flush page as memory, progress-gated interrupt forwarding and poll-mode GSP are not built [docs/caraxes/optimization-backlog.md §4 items 3, 5, 6].
- SotTR has no trap table and no frame rate, and its guest BAR1 use outside the pool is unknown [docs/caraxes/optimization-backlog.md §4 item 9; docs/caraxes/sottr-stage.md §8].

<!-- SPDX-License-Identifier: MIT -->
# The like-for-like 615 series against the closing series

Columns: native and v2 of the closing series (caraxes/runs/closing-610-2026-09-24, host 610.57.04, vm2 2 GiB slot 0, P21 area, spin reactor, quiet host), then the night series on the 615.71.09 host (vm2 1 GiB slot 3, BAR1 pool in place of the refused P21 area, spin reactor, interrupt page on): the 615 guest (other VMs active in 25 of 65 runs) and the 610 guest (quiet host). Ratios are to native. PPO in ppo/ of each part. Generated with caraxes/tools/bench-summary.py.

Sources: native=closing-610-2026-09-24/native, v2 (closing-610-2026-09-24)=closing-610-2026-09-24/v2, v2 (615)=bench-615-2026-09-24/night/615/v2, v2 (610)=bench-615-2026-09-24/night/610/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): native 65/65, v2 (closing-610-2026-09-24) 0/65, v2 (615) 25/65, v2 (610) 0/65
Host 1-min load average at run start, median [min-max]: native 3.48 [2.09-4.23] n=65, v2 (closing-610-2026-09-24) 3.52 [2.08-4.23] n=65, v2 (615) 7.50 [3.42-13.9] n=65, v2 (610) 4.15 [2.61-7.61] n=65

### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 16.3 [16.2-16.5] n=5 | 17.3 [17.0-18.3] n=5 | 17.3 [16.8-18.0] n=5 | 17.0 [16.7-17.3] n=5 | 1.06 | 1.06 | 1.04 |

### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 16.1 [16.1-16.1] n=5 | 18.0 [17.6-18.6] n=5 | 17.4 [17.0-17.7] n=5 | 17.2 [16.7-17.7] n=5 | 1.12 | 1.08 | 1.07 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.006 [0.006-0.009] n=5 | 0.015 [0.014-0.355] n=5 | 0.018 [0.015-0.040] n=5 | 0.016 [0.012-0.023] n=5 | 2.50 | 3.00 | 2.67 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.021 [0.015-0.026] n=5 | 0.100 [0.039-0.580] n=5 | 0.085 [0.023-0.414] n=5 | 0.089 [0.031-0.142] n=5 | 4.76 | 4.05 | 4.24 |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 12.1 [12.0-12.1] n=5 | 6.06 [3.14-6.12] n=5 | 6.07 [6.03-11.2] n=5 | 6.08 [5.97-11.1] n=5 | 0.50 | 0.50 | 0.50 |
| d2h pinned 1 MiB | GB/s (high is better) | 12.3 [12.2-12.3] n=5 | 6.18 [3.21-6.22] n=5 | 6.20 [6.17-11.4] n=5 | 6.20 [6.18-11.6] n=5 | 0.50 | 0.50 | 0.50 |
| d2d device 1 MiB | GB/s (high is better) | 132 [132-136] n=5 | 62.4 [47.8-65.9] n=5 | 66.5 [62.3-76.4] n=5 | 65.9 [63.4-76.9] n=5 | 0.47 | 0.50 | 0.50 |
| h2d pageable 1 MiB | GB/s (high is better) | 7.25 [7.22-7.38] n=5 | 4.50 [2.66-4.62] n=5 | 4.54 [4.53-7.01] n=5 | 4.55 [4.50-7.08] n=5 | 0.62 | 0.63 | 0.63 |
| d2h pageable 1 MiB | GB/s (high is better) | 6.07 [6.05-6.38] n=5 | 4.17 [2.58-4.23] n=5 | 4.21 [3.95-5.82] n=5 | 4.28 [4.21-6.32] n=5 | 0.69 | 0.69 | 0.70 |
| h2d pinned 16 MiB | GB/s (high is better) | 13.0 [13.0-13.1] n=5 | 6.57 [3.27-6.58] n=5 | 6.57 [6.53-12.9] n=5 | 6.57 [6.55-12.9] n=5 | 0.50 | 0.50 | 0.50 |
| d2h pinned 16 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 | 6.62 [3.23-6.65] n=5 | 6.61 [6.54-12.9] n=5 | 6.64 [5.71-13.0] n=5 | 0.50 | 0.50 | 0.51 |
| d2d device 16 MiB | GB/s (high is better) | 183 [182-185] n=5 | 16.5 [16.2-167] n=5 | 16.5 [11.5-176] n=5 | 16.9 [11.4-178] n=5 | 0.09 | 0.09 | 0.09 |
| h2d pageable 16 MiB | GB/s (high is better) | 9.90 [8.42-10.6] n=5 | 6.30 [6.12-10.1] n=5 | 6.98 [5.84-9.31] n=5 | 9.21 [4.61-10.0] n=5 | 0.64 | 0.70 | 0.93 |
| d2h pageable 16 MiB | GB/s (high is better) | 8.77 [8.10-9.09] n=5 | 6.32 [6.28-8.73] n=5 | 8.22 [6.15-8.69] n=5 | 7.92 [6.33-8.28] n=5 | 0.72 | 0.94 | 0.90 |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 | 13.1 [13.1-13.1] n=5 | 13.1 [9.34-13.1] n=5 | 13.1 [7.47-13.1] n=5 | 1.00 | 1.00 | 1.00 |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [12.9-13.2] n=5 | 13.2 [13.1-13.2] n=5 | 13.1 [13.1-13.2] n=5 | 13.2 [13.2-13.2] n=5 | 1.00 | 1.00 | 1.00 |
| d2d device 256 MiB | GB/s (high is better) | 186 [183-188] n=5 | 190 [183-193] n=5 | 192 [158-194] n=5 | 193 [190-194] n=5 | 1.02 | 1.03 | 1.04 |
| h2d pageable 256 MiB | GB/s (high is better) | 9.27 [8.71-9.46] n=5 | 8.86 [8.73-8.95] n=5 | 8.65 [7.30-8.94] n=5 | 8.69 [8.52-9.12] n=5 | 0.96 | 0.93 | 0.94 |
| d2h pageable 256 MiB | GB/s (high is better) | 9.00 [8.78-9.32] n=5 | 8.35 [8.15-8.74] n=5 | 7.58 [5.69-8.58] n=5 | 8.68 [8.54-9.04] n=5 | 0.93 | 0.84 | 0.96 |

### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 4894 [4720-5264] n=5 | 1340 [379-1386] n=5 | 1310 [1071-4540] n=5 | 4850 [1334-5252] n=5 | 0.27 | 0.27 | 0.99 |
| N=2048 | GFLOP/s (high is better) | 5494 [5329-6093] n=5 | 2248 [2045-2422] n=5 | 2407 [1458-4788] n=5 | 5149 [2579-5430] n=5 | 0.41 | 0.44 | 0.94 |
| N=4096 | GFLOP/s (high is better) | 6052 [5834-6505] n=5 | 4433 [4230-5214] n=5 | 4627 [3696-5304] n=5 | 5419 [5258-5797] n=5 | 0.73 | 0.76 | 0.90 |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | 1.00 | 1.00 | 1.00 |
| process wall (incl. torch import) | s (low is better) | 4.64 [4.54-4.83] n=5 | 12.2 [11.1-13.3] n=5 | 13.5 [11.4-22.5] n=5 | 11.8 [11.5-12.3] n=5 | 2.63 | 2.92 | 2.55 |
| stage4 (50 training episodes) | s (low is better) | 1.20 [1.20-1.40] n=5 | 2.10 [2.00-2.50] n=5 | 2.60 [2.20-4.90] n=5 | 2.00 [1.90-2.40] n=5 | 1.75 | 2.17 | 1.67 |
| H2D+D2H transfers per second | 1/s (high is better) | 2350 [2129-2454] n=5 | 1340 [1155-1448] n=5 | 1106 [585-1332] n=5 | 1403 [1219-1534] n=5 | 0.57 | 0.47 | 0.60 |

### (5) Module load to first successful cuInit (in the guest)

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| module load | ms (low is better) | n/a | 3337 [3136-3627] n=5 | 3630 [3569-4039] n=5 | 3635 [3324-7901] n=5 | - | - | - |
| load to first cuInit | ms (low is better) | n/a | 3509 [3285-3792] n=5 | 3825 [3797-4230] n=5 | 3813 [3489-8381] n=5 | - | - | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| cuInit | ms (low is better) | 106 [93.9-117] n=5 | 151 [148-177] n=5 | 198 [169-1493] n=5 | 161 [139-219] n=5 | 1.42 | 1.87 | 1.52 |
| cuCtxCreate | ms (low is better) | 195 [190-197] n=5 | 3770 [3537-4049] n=5 | 4444 [3650-13110] n=5 | 4017 [3856-4245] n=5 | 19.29 | 22.74 | 20.55 |
| cuCtxDestroy | ms (low is better) | 92.6 [89.2-99.9] n=5 | 1352 [1104-1383] n=5 | 3057 [2385-3795] n=5 | 2794 [2502-2846] n=5 | 14.60 | 33.01 | 30.17 |

### (5) nvprobe 0 process, bash time

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 225 [205-231] n=5 | 185 [170-199] n=5 | 268 [219-299] n=5 | 201 [176-236] n=5 | 0.82 | 1.19 | 0.89 |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 158 [147-182] n=5 | 185 [170-198] n=5 | 268 [219-299] n=5 | 201 [176-235] n=5 | 1.17 | 1.70 | 1.27 |

### (5) nvprobe 1 process, bash time

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 409 [384-445] n=5 | 4172 [3768-4565] n=5 | 5922 [4620-9626] n=5 | 4530 [4264-4739] n=5 | 10.20 | 14.48 | 11.08 |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 349 [336-361] n=5 | 4170 [3768-4563] n=5 | 5973 [4621-10281] n=5 | 4537 [4264-4739] n=5 | 11.95 | 17.11 | 13.00 |

### (6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 2.92 [2.59-3.33] n=5 | 1.72 [1.49-2.47] n=5 | 1.59 [1.32-2.43] n=5 | 1.58 [1.33-2.46] n=5 | 0.59 | 0.54 | 0.54 |
| CHECK_VERSION_STR p99 | us (low is better) | 5.20 [3.20-15.2] n=5 | 4.39 [2.76-7.31] n=5 | 2.52 [1.92-8.08] n=5 | 2.37 [1.96-2.93] n=5 | 0.84 | 0.48 | 0.46 |
| CHECK_VERSION_STR mean | us (low is better) | 3.01 [2.63-3.66] n=5 | 2.21 [2.06-2.42] n=5 | 1.74 [1.44-3.27] n=5 | 1.73 [1.39-2.54] n=5 | 0.73 | 0.58 | 0.57 |

### (6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 5.41 [4.22-5.47] n=5 | 2.46 [2.32-2.92] n=5 | 2.37 [2.36-2.95] n=5 | 2.99 [2.38-3.01] n=5 | 0.45 | 0.44 | 0.55 |
| RM_CONTROL 232 B p99 | us (low is better) | 6.47 [5.40-7.06] n=5 | 4.80 [3.07-8.08] n=5 | 4.74 [2.76-12.3] n=5 | 4.08 [3.10-4.96] n=5 | 0.74 | 0.73 | 0.63 |
| RM_CONTROL 1576 B p50 | us (low is better) | 4.30 [4.20-6.67] n=5 | 2.48 [2.36-2.92] n=5 | 2.91 [2.36-4.16] n=5 | 3.00 [2.38-3.08] n=5 | 0.58 | 0.68 | 0.70 |
| RM_CONTROL 1576 B p99 | us (low is better) | 5.62 [5.39-8.03] n=5 | 3.76 [3.47-21.7] n=5 | 3.90 [3.06-18.6] n=5 | 4.01 [3.45-5.23] n=5 | 0.67 | 0.69 | 0.71 |
| RM_CONTROL 12328 B p50 | us (low is better) | 4.30 [4.22-5.40] n=5 | 2.50 [2.38-2.91] n=5 | 2.38 [2.33-4.73] n=5 | 3.02 [2.38-3.04] n=5 | 0.58 | 0.55 | 0.70 |
| RM_CONTROL 12328 B p99 | us (low is better) | 5.66 [5.43-7.77] n=5 | 4.02 [3.10-10.8] n=5 | 6.51 [3.02-22.2] n=5 | 4.20 [3.24-6.14] n=5 | 0.71 | 1.15 | 0.74 |
| RM_CONTROL 49192 B p50 | us (low is better) | 4.28 [4.23-5.32] n=5 | 2.51 [2.38-2.91] n=5 | 2.37 [2.34-5.64] n=5 | 3.03 [2.47-3.08] n=5 | 0.59 | 0.55 | 0.71 |
| RM_CONTROL 49192 B p99 | us (low is better) | 6.95 [5.36-7.73] n=5 | 5.10 [3.45-18.7] n=5 | 3.58 [2.58-20.6] n=5 | 4.04 [3.64-6.53] n=5 | 0.73 | 0.52 | 0.58 |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 35.8 [33.6-50.9] n=5 | 830 [824-835] n=5 | 238 [184-616] n=5 | 204 [182-218] n=5 | 23.18 | 6.64 | 5.71 |
| 64x1 MiB cuMemFree | us (low is better) | 27.1 [25.6-46.2] n=5 | 903 [887-1777] n=5 | 463 [245-989] n=5 | 273 [239-482] n=5 | 33.33 | 17.08 | 10.07 |
| 64x1 MiB cycle | ms (low is better) | 3.81 [3.77-4.52] n=5 | 168 [148-179] n=5 | 47.5 [40.2-277] n=5 | 43.2 [39.3-57.8] n=5 | 44.19 | 12.48 | 11.34 |
| 8x64 MiB cuMemAlloc | us (low is better) | 78.3 [75.0-81.2] n=5 | 6747 [5751-7226] n=5 | 4919 [4441-16320] n=5 | 4841 [4706-8156] n=5 | 86.17 | 62.82 | 61.83 |
| 8x64 MiB cuMemFree | us (low is better) | 92.6 [88.2-97.5] n=5 | 4422 [4190-5040] n=5 | 3067 [2855-9456] n=5 | 2850 [2842-5499] n=5 | 47.75 | 33.13 | 30.77 |
| 8x64 MiB cycle | ms (low is better) | 10.6 [9.09-11.1] n=5 | 115 [99.5-143] n=5 | 118 [82.3-246] n=5 | 97.0 [94.1-138] n=5 | 10.86 | 11.22 | 9.19 |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| churn device CPU | ms (low is better) | n/a | 17160 [15330-17780] n=5 | 16740 [15340-26960] n=5 | 16150 [15830-18500] n=5 | - | - | - |
| churn VMM CPU | ms (low is better) | n/a | 18650 [7460-18920] n=5 | 14050 [4630-18390] n=5 | 13190 [5650-13500] n=5 | - | - | - |
| copy device CPU | ms (low is better) | n/a | 24690 [24030-26060] n=5 | 28920 [25480-31700] n=5 | 25690 [24990-27260] n=5 | - | - | - |
| copy VMM CPU | ms (low is better) | n/a | 30480 [29500-31240] n=5 | 30580 [11620-32990] n=5 | 29300 [28140-30920] n=5 | - | - | - |
| ctrl device CPU | ms (low is better) | n/a | 260 [250-500] n=5 | 280 [220-500] n=5 | 230 [220-250] n=5 | - | - | - |
| ctrl VMM CPU | ms (low is better) | n/a | 440 [210-490] n=5 | 460 [240-510] n=5 | 390 [200-460] n=5 | - | - | - |
| init device CPU | ms (low is better) | n/a | 8140 [7420-8620] n=5 | 12150 [10360-18160] n=5 | 11210 [10930-11530] n=5 | - | - | - |
| init VMM CPU | ms (low is better) | n/a | 8150 [3000-8630] n=5 | 8480 [2640-11330] n=5 | 8760 [8550-9710] n=5 | - | - | - |
| ioctl device CPU | ms (low is better) | n/a | 240 [230-270] n=5 | 260 [220-590] n=5 | 230 [220-240] n=5 | - | - | - |
| ioctl VMM CPU | ms (low is better) | n/a | 410 [200-510] n=5 | 390 [200-1110] n=5 | 400 [200-440] n=5 | - | - | - |
| launch-auto device CPU | ms (low is better) | n/a | 8300 [7980-8590] n=5 | 9230 [8120-9930] n=5 | 8080 [7880-9050] n=5 | - | - | - |
| launch-auto VMM CPU | ms (low is better) | n/a | 11230 [10650-11310] n=5 | 10510 [8540-11760] n=5 | 8080 [6740-10900] n=5 | - | - | - |
| launch-blocking device CPU | ms (low is better) | n/a | 9860 [9660-10110] n=5 | 12840 [12060-17120] n=5 | 12030 [11890-12140] n=5 | - | - | - |
| launch-blocking VMM CPU | ms (low is better) | n/a | 9920 [8920-10570] n=5 | 10600 [3030-12710] n=5 | 9420 [9070-10590] n=5 | - | - | - |
| mm device CPU | ms (low is better) | n/a | 14960 [14110-15400] n=5 | 16520 [13630-17040] n=5 | 14100 [13500-15290] n=5 | - | - | - |
| mm VMM CPU | ms (low is better) | n/a | 20430 [19880-21300] n=5 | 16310 [6770-19110] n=5 | 17710 [17230-19980] n=5 | - | - | - |
| rl device CPU | ms (low is better) | n/a | 14810 [14140-15520] n=5 | 16750 [14870-23260] n=5 | 15610 [15100-15840] n=5 | - | - | - |
| rl VMM CPU | ms (low is better) | n/a | 23240 [11420-24600] n=5 | 21490 [9530-29810] n=5 | 21710 [21310-22210] n=5 | - | - | - |
| short-auto device CPU | ms (low is better) | n/a | 6980 [6570-7400] n=5 | 7440 [7010-8650] n=5 | 6710 [6490-7490] n=5 | - | - | - |
| short-auto VMM CPU | ms (low is better) | n/a | 7870 [7560-8310] n=5 | 7620 [2900-8240] n=5 | 7030 [6810-8110] n=5 | - | - | - |
| short-blocking device CPU | ms (low is better) | n/a | 8390 [8130-8650] n=5 | 11080 [10850-13540] n=5 | 10810 [10680-11710] n=5 | - | - | - |
| short-blocking VMM CPU | ms (low is better) | n/a | 8440 [8060-8710] n=5 | 8750 [2900-11240] n=5 | 8660 [7640-9370] n=5 | - | - | - |
| time0 device CPU | ms (low is better) | n/a | 820 [490-850] n=5 | 610 [540-800] n=5 | 530 [490-710] n=5 | - | - | - |
| time0 VMM CPU | ms (low is better) | n/a | 650 [270-700] n=5 | 640 [260-730] n=5 | 620 [470-670] n=5 | - | - | - |
| time1 device CPU | ms (low is better) | n/a | 6310 [5820-6870] n=5 | 8310 [7260-11720] n=5 | 7050 [6660-7380] n=5 | - | - | - |
| time1 VMM CPU | ms (low is better) | n/a | 6650 [2850-7570] n=5 | 8030 [2710-10060] n=5 | 6970 [2730-7440] n=5 | - | - | - |

### (8) Device process memory (over all runs)

| value | unit | native | v2 (closing-610-2026-09-24) | v2 (615) | v2 (610) | v2 (closing-610-2026-09-24) / native | v2 (615) / native | v2 (610) / native |
|---|---|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | n/a | 9222 [9222-9222] n=65 | 8199 [8199-8199] n=65 | 8199 [8199-8199] n=65 | - | - | - |
| device RSS after run | MiB (low is better) | n/a | 5141 [5130-5142] n=65 | 5139 [5026-5144] n=65 | 5143 [5131-5145] n=65 | - | - | - |


## PPO (60 s, 2 reps)

```
615 guest: # PPO steps_per_s=51178 solve_s=-1 episodes=35856 reward100=349.0 envs=128 rollout=256
615 guest: # PPO steps_per_s=64423 solve_s=-1 episodes=38212 reward100=415.8 envs=128 rollout=256
610 guest: # PPO steps_per_s=71281 solve_s=-1 episodes=39443 reward100=341.6 envs=128 rollout=256
610 guest: # PPO steps_per_s=71378 solve_s=-1 episodes=39443 reward100=341.6 envs=128 rollout=256
```

Sources: v1=/home/silas/git/Caraxes/caraxes/runs/mapping-2026-09-24/final/v1, v2=/home/silas/git/Caraxes/caraxes/runs/mapping-2026-09-24/final/v2, native=/home/silas/git/Caraxes/caraxes/runs/mapping-2026-09-24/final/native
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v1 38/65, v2 35/65, native 46/65
Host 1-min load average at run start, median [min-max]: v1 4.73 [2.66-10.8] n=65, v2 4.73 [2.66-12.7] n=65, native 4.70 [2.66-10.8] n=65

### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 16.1 [16.0-16.1] n=5 | 17.6 [17.3-17.8] n=5 | 16.4 [16.2-16.6] n=5 | 0.98 | 1.08 |

### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 100 [99.6-100] n=5 | 18.4 [18.0-21.7] n=5 | 16.1 [16.1-16.3] n=5 | 6.21 | 1.14 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.008 [0.008-0.053] n=5 | 0.200 [0.164-0.257] n=5 | 0.006 [0.006-0.009] n=5 | 1.33 | 33.33 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 11.1 [0.029-86.3] n=5 | 0.321 [0.200-0.522] n=5 | 0.030 [0.021-0.050] n=5 | 368.97 | 10.70 |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 12.2 [12.0-12.2] n=5 | 3.63 [2.14-3.90] n=5 | 12.1 [11.8-12.1] n=5 | 1.01 | 0.30 |
| d2h pinned 1 MiB | GB/s (high is better) | 12.4 [12.3-12.4] n=5 | 3.69 [2.45-3.99] n=5 | 12.3 [12.1-12.3] n=5 | 1.01 | 0.30 |
| d2d device 1 MiB | GB/s (high is better) | 141 [135-144] n=5 | 8.65 [8.22-9.21] n=5 | 134 [118-134] n=5 | 1.06 | 0.06 |
| h2d pageable 1 MiB | GB/s (high is better) | 7.35 [3.61-7.44] n=5 | 3.13 [2.13-3.17] n=5 | 7.34 [7.29-7.40] n=5 | 1.00 | 0.43 |
| d2h pageable 1 MiB | GB/s (high is better) | 6.45 [2.50-6.53] n=5 | 2.71 [2.03-3.02] n=5 | 6.28 [6.13-6.39] n=5 | 1.03 | 0.43 |
| h2d pinned 16 MiB | GB/s (high is better) | 13.0 [13.0-13.1] n=5 | 5.94 [3.19-6.28] n=5 | 13.0 [13.0-13.0] n=5 | 1.00 | 0.46 |
| d2h pinned 16 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 | 6.18 [3.01-9.32] n=5 | 13.1 [13.1-13.1] n=5 | 1.00 | 0.47 |
| d2d device 16 MiB | GB/s (high is better) | 176 [174-178] n=5 | 14.7 [3.77-87.4] n=5 | 182 [179-183] n=5 | 0.97 | 0.08 |
| h2d pageable 16 MiB | GB/s (high is better) | 9.45 [7.88-10.2] n=5 | 3.64 [2.69-4.63] n=5 | 8.63 [8.53-9.40] n=5 | 1.10 | 0.42 |
| d2h pageable 16 MiB | GB/s (high is better) | 8.04 [7.32-8.82] n=5 | 4.07 [2.63-4.24] n=5 | 8.80 [8.43-9.01] n=5 | 0.91 | 0.46 |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 | 12.9 [12.8-13.0] n=5 | 13.1 [13.0-13.1] n=5 | 1.00 | 0.98 |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [13.1-13.2] n=5 | 13.0 [13.0-13.1] n=5 | 13.2 [13.1-13.2] n=5 | 1.00 | 0.99 |
| d2d device 256 MiB | GB/s (high is better) | 183 [157-190] n=5 | 173 [168-175] n=5 | 186 [183-188] n=5 | 0.99 | 0.93 |
| h2d pageable 256 MiB | GB/s (high is better) | 7.51 [1.93-8.70] n=5 | 3.30 [3.28-3.89] n=5 | 8.66 [8.24-8.89] n=5 | 0.87 | 0.38 |
| d2h pageable 256 MiB | GB/s (high is better) | 7.38 [2.81-8.61] n=5 | 3.54 [3.12-4.09] n=5 | 8.25 [7.20-8.42] n=5 | 0.89 | 0.43 |

### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 5232 [4407-5316] n=5 | 1219 [1180-1306] n=5 | 5561 [4924-6139] n=5 | 0.94 | 0.22 |
| N=2048 | GFLOP/s (high is better) | 5336 [5279-5589] n=5 | 2220 [2156-2448] n=5 | 6170 [5969-6611] n=5 | 0.86 | 0.36 |
| N=4096 | GFLOP/s (high is better) | 6271 [6015-6548] n=5 | 5245 [4615-5330] n=5 | 6464 [6317-6811] n=5 | 0.97 | 0.81 |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | 1.00 | 1.00 |
| process wall (incl. torch import) | s (low is better) | 82.3 [12.1-145] n=5 | 30.1 [27.2-34.0] n=5 | 4.83 [4.69-5.48] n=5 | 17.05 | 6.24 |
| stage4 (50 training episodes) | s (low is better) | 2.00 [1.60-4.80] n=5 | 18.2 [16.2-20.8] n=5 | 1.30 [1.20-1.40] n=5 | 1.54 | 14.00 |
| H2D+D2H transfers per second | 1/s (high is better) | 1467 [601-1743] n=5 | 158 [138-178] n=5 | 2208 [1993-2481] n=5 | 0.66 | 0.07 |

### (5) Module load to first successful cuInit (in the guest)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| module load | ms (low is better) | 31.0 [25.0-36.0] n=5 | 3806 [3258-3893] n=5 | n/a | - | - |
| load to first cuInit | ms (low is better) | 242 [128-276] n=5 | 3948 [3417-4090] n=5 | n/a | - | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| cuInit | ms (low is better) | 34.4 [30.7-47.0] n=5 | 174 [130-178] n=5 | 81.6 [68.7-120] n=5 | 0.42 | 2.14 |
| cuCtxCreate | ms (low is better) | 167 [157-178] n=5 | 4016 [3796-4314] n=5 | 196 [188-229] n=5 | 0.85 | 20.54 |
| cuCtxDestroy | ms (low is better) | 92.3 [74.0-96.9] n=5 | 1160 [1147-1288] n=5 | 93.4 [84.6-98.2] n=5 | 0.99 | 12.42 |

### (5) nvprobe 0 process, bash time

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 168 [164-188] n=5 | 182 [151-231] n=5 | 310 [217-337] n=5 | 0.54 | 0.59 |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 5.00 [5.00-8.00] n=5 | 182 [151-231] n=5 | 196 [160-224] n=5 | 0.03 | 0.93 |

### (5) nvprobe 1 process, bash time

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 366 [341-387] n=5 | 4439 [4242-5083] n=5 | 452 [390-475] n=5 | 0.81 | 9.82 |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 45.0 [38.0-60.0] n=5 | 4529 [4360-5093] n=5 | 351 [345-373] n=5 | 0.13 | 12.90 |

### (6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 16.4 [15.8-16.8] n=5 | 1.40 [1.38-1.50] n=5 | 2.58 [2.54-3.13] n=5 | 6.37 | 0.54 |
| CHECK_VERSION_STR p99 | us (low is better) | 39.3 [28.5-82.7] n=5 | 3.20 [2.55-4.86] n=5 | 5.42 [3.41-11.4] n=5 | 7.24 | 0.59 |
| CHECK_VERSION_STR mean | us (low is better) | 19.5 [17.6-21.4] n=5 | 1.76 [1.61-2.24] n=5 | 2.83 [2.69-3.34] n=5 | 6.90 | 0.62 |

### (6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 19.5 [18.0-24.9] n=5 | 2.39 [2.30-3.01] n=5 | 4.45 [4.20-4.51] n=5 | 4.38 | 0.54 |
| RM_CONTROL 232 B p99 | us (low is better) | 44.0 [33.5-60.0] n=5 | 4.04 [3.61-9.07] n=5 | 6.14 [5.70-7.68] n=5 | 7.16 | 0.66 |
| RM_CONTROL 1576 B p50 | us (low is better) | 19.5 [16.9-24.7] n=5 | 2.47 [2.38-3.04] n=5 | 4.50 [4.17-4.51] n=5 | 4.34 | 0.55 |
| RM_CONTROL 1576 B p99 | us (low is better) | 66.3 [26.5-130] n=5 | 4.07 [3.75-31.2] n=5 | 5.99 [5.18-7.24] n=5 | 11.06 | 0.68 |
| RM_CONTROL 12328 B p50 | us (low is better) | 24.7 [20.3-32.5] n=5 | 2.59 [2.41-3.60] n=5 | 4.53 [4.14-4.59] n=5 | 5.45 | 0.57 |
| RM_CONTROL 12328 B p99 | us (low is better) | 78.8 [35.8-90.1] n=5 | 4.28 [3.80-12.7] n=5 | 6.47 [5.36-11.1] n=5 | 12.19 | 0.66 |
| RM_CONTROL 49192 B p50 | us (low is better) | 37.6 [32.4-38.4] n=5 | 2.62 [2.44-3.37] n=5 | 4.53 [4.15-4.83] n=5 | 8.31 | 0.58 |
| RM_CONTROL 49192 B p99 | us (low is better) | 68.9 [54.6-126] n=5 | 4.67 [3.64-4.85] n=5 | 6.57 [5.06-17.9] n=5 | 10.48 | 0.71 |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 81.7 [70.1-94.8] n=5 | 906 [809-1654] n=5 | 35.0 [33.4-44.7] n=5 | 2.33 | 25.89 |
| 64x1 MiB cuMemFree | us (low is better) | 44.5 [42.5-75.0] n=5 | 998 [894-1027] n=5 | 29.1 [27.1-45.8] n=5 | 1.53 | 34.30 |
| 64x1 MiB cycle | ms (low is better) | 7.81 [7.25-8.91] n=5 | 182 [156-216] n=5 | 4.01 [3.88-4.59] n=5 | 1.95 | 45.47 |
| 8x64 MiB cuMemAlloc | us (low is better) | 197 [178-231] n=5 | 7934 [6508-9304] n=5 | 95.9 [89.1-106] n=5 | 2.06 | 82.73 |
| 8x64 MiB cuMemFree | us (low is better) | 134 [127-168] n=5 | 5484 [4876-6638] n=5 | 95.9 [91.6-99.0] n=5 | 1.39 | 57.19 |
| 8x64 MiB cycle | ms (low is better) | 9.33 [8.51-9.63] n=5 | 135 [111-151] n=5 | 11.0 [9.11-12.6] n=5 | 0.85 | 12.23 |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| churn device CPU | ms (low is better) | 390 [370-400] n=5 | 6550 [6120-7510] n=5 | n/a | - | - |
| churn VMM CPU | ms (low is better) | 520 [420-770] n=5 | 8650 [8140-10030] n=5 | n/a | - | - |
| copy device CPU | ms (low is better) | 330 [300-440] n=5 | 8210 [7480-9150] n=5 | n/a | - | - |
| copy VMM CPU | ms (low is better) | 18700 [3480-20780] n=5 | 17390 [13900-35570] n=5 | n/a | - | - |
| ctrl device CPU | ms (low is better) | 110 [100-120] n=5 | 20.0 [10.0-20.0] n=5 | n/a | - | - |
| ctrl VMM CPU | ms (low is better) | 540 [350-590] n=5 | 220 [210-240] n=5 | n/a | - | - |
| init device CPU | ms (low is better) | 230 [210-230] n=5 | 2980 [2880-3060] n=5 | n/a | - | - |
| init VMM CPU | ms (low is better) | 800 [780-850] n=5 | 3390 [3280-6590] n=5 | n/a | - | - |
| ioctl device CPU | ms (low is better) | 110 [90.0-120] n=5 | 20.0 [10.0-20.0] n=5 | n/a | - | - |
| ioctl VMM CPU | ms (low is better) | 410 [330-480] n=5 | 210 [190-220] n=5 | n/a | - | - |
| launch-auto device CPU | ms (low is better) | 210 [210-230] n=5 | 2080 [2010-2300] n=5 | n/a | - | - |
| launch-auto VMM CPU | ms (low is better) | 2320 [2240-4220] n=5 | 11160 [10960-11570] n=5 | n/a | - | - |
| launch-blocking device CPU | ms (low is better) | 260 [220-270] n=5 | 3120 [2870-3760] n=5 | n/a | - | - |
| launch-blocking VMM CPU | ms (low is better) | 340 [320-9500] n=5 | 10490 [7330-12130] n=5 | n/a | - | - |
| mm device CPU | ms (low is better) | 400 [390-530] n=5 | 4110 [3330-4540] n=5 | n/a | - | - |
| mm VMM CPU | ms (low is better) | 28500 [13340-46260] n=5 | 11560 [9570-21160] n=5 | n/a | - | - |
| rl device CPU | ms (low is better) | 400 [340-1060] n=5 | 9980 [9580-10390] n=5 | n/a | - | - |
| rl VMM CPU | ms (low is better) | 95850 [11720-128470] n=5 | 25370 [24010-53060] n=5 | n/a | - | - |
| short-auto device CPU | ms (low is better) | 270 [250-280] n=5 | 2460 [2290-2690] n=5 | n/a | - | - |
| short-auto VMM CPU | ms (low is better) | 570 [350-900] n=5 | 8050 [3680-8820] n=5 | n/a | - | - |
| short-blocking device CPU | ms (low is better) | 230 [230-250] n=5 | 3120 [2960-3310] n=5 | n/a | - | - |
| short-blocking VMM CPU | ms (low is better) | 300 [260-2300] n=5 | 8200 [3770-9780] n=5 | n/a | - | - |
| time0 device CPU | ms (low is better) | 130 [120-130] n=5 | 450 [330-480] n=5 | n/a | - | - |
| time0 VMM CPU | ms (low is better) | 140 [120-150] n=5 | 310 [280-340] n=5 | n/a | - | - |
| time1 device CPU | ms (low is better) | 230 [220-230] n=5 | 2190 [2140-2550] n=5 | n/a | - | - |
| time1 VMM CPU | ms (low is better) | 230 [230-260] n=5 | 3310 [3100-3530] n=5 | n/a | - | - |

### (8) Device process memory (over all runs)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 457 [64.3-559] n=65 | 6149 [6149-6149] n=65 | n/a | - | - |
| device RSS after run | MiB (low is better) | 428 [19.3-516] n=65 | 5141 [5129-5142] n=65 | n/a | - | - |


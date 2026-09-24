Sources: native=caraxes/runs/closing-610-2026-09-24/native, v2-615guest=caraxes/runs/bench-615-2026-09-24/615/v2, v2-610guest=caraxes/runs/bench-615-2026-09-24/610/v2, v2=caraxes/runs/closing-610-2026-09-24/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): native 65/65, v2-615guest 5/14, v2-610guest 9/14, v2 0/65
Host 1-min load average at run start, median [min-max]: native 3.48 [2.09-4.23] n=65, v2-615guest 3.49 [1.60-5.80] n=14, v2-610guest 4.65 [2.22-10.9] n=14, v2 3.52 [2.08-4.23] n=65

### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 16.3 [16.2-16.5] n=5 | n/a | n/a | 17.3 [17.0-18.3] n=5 | - | - | 1.06 |

### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 16.1 [16.1-16.1] n=5 | n/a | n/a | 18.0 [17.6-18.6] n=5 | - | - | 1.12 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.006 [0.006-0.009] n=5 | 0.144 [0.127-0.148] n=3 | 0.307 [0.157-0.382] n=3 | 0.015 [0.014-0.355] n=5 | 24.00 | 51.17 | 2.50 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.021 [0.015-0.026] n=5 | n/a | n/a | 0.100 [0.039-0.580] n=5 | - | - | 4.76 |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 12.1 [12.0-12.1] n=5 | 3.85 [3.61-3.90] n=3 | 3.90 [2.58-4.30] n=3 | 6.06 [3.14-6.12] n=5 | 0.32 | 0.32 | 0.50 |
| d2h pinned 1 MiB | GB/s (high is better) | 12.3 [12.2-12.3] n=5 | 3.83 [3.61-3.95] n=3 | 3.95 [2.37-4.34] n=3 | 6.18 [3.21-6.22] n=5 | 0.31 | 0.32 | 0.50 |
| d2d device 1 MiB | GB/s (high is better) | 132 [132-136] n=5 | 8.44 [4.91-9.87] n=3 | 9.60 [6.04-10.6] n=3 | 62.4 [47.8-65.9] n=5 | 0.06 | 0.07 | 0.47 |
| h2d pageable 1 MiB | GB/s (high is better) | 7.25 [7.22-7.38] n=5 | 2.65 [2.42-2.81] n=3 | 2.92 [2.55-3.18] n=3 | 4.50 [2.66-4.62] n=5 | 0.37 | 0.40 | 0.62 |
| d2h pageable 1 MiB | GB/s (high is better) | 6.07 [6.05-6.38] n=5 | 2.77 [2.31-2.89] n=3 | 2.96 [2.48-3.33] n=3 | 4.17 [2.58-4.23] n=5 | 0.46 | 0.49 | 0.69 |
| h2d pinned 16 MiB | GB/s (high is better) | 13.0 [13.0-13.1] n=5 | 6.29 [6.25-6.29] n=3 | 6.28 [3.13-11.5] n=3 | 6.57 [3.27-6.58] n=5 | 0.48 | 0.48 | 0.50 |
| d2h pinned 16 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 | 6.23 [6.00-6.33] n=3 | 6.28 [5.88-6.32] n=3 | 6.62 [3.23-6.65] n=5 | 0.48 | 0.48 | 0.50 |
| d2d device 16 MiB | GB/s (high is better) | 183 [182-185] n=5 | 14.1 [13.3-14.6] n=3 | 14.5 [13.5-14.6] n=3 | 16.5 [16.2-167] n=5 | 0.08 | 0.08 | 0.09 |
| h2d pageable 16 MiB | GB/s (high is better) | 9.90 [8.42-10.6] n=5 | 4.36 [4.25-4.82] n=3 | 4.11 [2.23-4.60] n=3 | 6.30 [6.12-10.1] n=5 | 0.44 | 0.42 | 0.64 |
| d2h pageable 16 MiB | GB/s (high is better) | 8.77 [8.10-9.09] n=5 | 4.50 [3.68-4.99] n=3 | 2.83 [2.82-4.65] n=3 | 6.32 [6.28-8.73] n=5 | 0.51 | 0.32 | 0.72 |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 | 12.8 [12.6-12.9] n=3 | 12.9 [12.2-13.0] n=3 | 13.1 [13.1-13.1] n=5 | 0.98 | 0.99 | 1.00 |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [12.9-13.2] n=5 | 13.1 [12.9-13.1] n=3 | 13.1 [13.0-13.1] n=3 | 13.2 [13.1-13.2] n=5 | 0.99 | 0.99 | 1.00 |
| d2d device 256 MiB | GB/s (high is better) | 186 [183-188] n=5 | 175 [173-177] n=3 | 177 [167-180] n=3 | 190 [183-193] n=5 | 0.94 | 0.95 | 1.02 |
| h2d pageable 256 MiB | GB/s (high is better) | 9.27 [8.71-9.46] n=5 | 3.32 [3.13-4.34] n=3 | 4.43 [3.39-4.65] n=3 | 8.86 [8.73-8.95] n=5 | 0.36 | 0.48 | 0.96 |
| d2h pageable 256 MiB | GB/s (high is better) | 9.00 [8.78-9.32] n=5 | 3.62 [2.69-3.65] n=3 | 3.33 [3.30-4.67] n=3 | 8.35 [8.15-8.74] n=5 | 0.40 | 0.37 | 0.93 |

### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 4894 [4720-5264] n=5 | n/a | n/a | 1340 [379-1386] n=5 | - | - | 0.27 |
| N=2048 | GFLOP/s (high is better) | 5494 [5329-6093] n=5 | n/a | n/a | 2248 [2045-2422] n=5 | - | - | 0.41 |
| N=4096 | GFLOP/s (high is better) | 6052 [5834-6505] n=5 | n/a | n/a | 4433 [4230-5214] n=5 | - | - | 0.73 |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=3 | 56.9 [56.9-56.9] n=3 | 56.9 [56.9-56.9] n=5 | 1.00 | 1.00 | 1.00 |
| process wall (incl. torch import) | s (low is better) | 4.64 [4.54-4.83] n=5 | 27.5 [26.3-33.0] n=3 | 41.0 [35.2-42.3] n=3 | 12.2 [11.1-13.3] n=5 | 5.92 | 8.84 | 2.63 |
| stage4 (50 training episodes) | s (low is better) | 1.20 [1.20-1.40] n=5 | 15.2 [15.0-20.2] n=3 | 20.4 [19.5-22.4] n=3 | 2.10 [2.00-2.50] n=5 | 12.67 | 17.00 | 1.75 |
| H2D+D2H transfers per second | 1/s (high is better) | 2350 [2129-2454] n=5 | 189 [143-192] n=3 | 141 [128-147] n=3 | 1340 [1155-1448] n=5 | 0.08 | 0.06 | 0.57 |

### (4) PPO CartPole, 128 vector environments, one H2D and one synchronous D2H per step (pporl, 60 s)

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| environment steps per second | 1/s (high is better) | n/a | 6226 [5727-6726] n=2 | 6718 [6471-6964] n=2 | n/a | - | - | - |
| reward100 at the end (not a time) |  | n/a | 33.5 [30.2-36.8] n=2 | 36.8 [36.8-36.8] n=2 | n/a | - | - | - |

### (5) Module load to first successful cuInit (in the guest)

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| module load | ms (low is better) | n/a | n/a | n/a | 3337 [3136-3627] n=5 | - | - | - |
| load to first cuInit | ms (low is better) | n/a | n/a | n/a | 3509 [3285-3792] n=5 | - | - | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| cuInit | ms (low is better) | 106 [93.9-117] n=5 | 135 [134-233] n=3 | 212 [148-315] n=3 | 151 [148-177] n=5 | 1.28 | 2.00 | 1.42 |
| cuCtxCreate | ms (low is better) | 195 [190-197] n=5 | 4349 [3683-4788] n=3 | 4144 [3381-5122] n=3 | 3770 [3537-4049] n=5 | 22.25 | 21.20 | 19.29 |
| cuCtxDestroy | ms (low is better) | 92.6 [89.2-99.9] n=5 | 1517 [1115-1644] n=3 | 1350 [942-1607] n=3 | 1352 [1104-1383] n=5 | 16.38 | 14.58 | 14.60 |

### (5) nvprobe 0 process, bash time

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 225 [205-231] n=5 | n/a | n/a | 185 [170-199] n=5 | - | - | 0.82 |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 158 [147-182] n=5 | n/a | n/a | 185 [170-198] n=5 | - | - | 1.17 |

### (5) nvprobe 1 process, bash time

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 409 [384-445] n=5 | n/a | n/a | 4172 [3768-4565] n=5 | - | - | 10.20 |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 349 [336-361] n=5 | n/a | n/a | 4170 [3768-4563] n=5 | - | - | 11.95 |

### (6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 2.92 [2.59-3.33] n=5 | n/a | n/a | 1.72 [1.49-2.47] n=5 | - | - | 0.59 |
| CHECK_VERSION_STR p99 | us (low is better) | 5.20 [3.20-15.2] n=5 | n/a | n/a | 4.39 [2.76-7.31] n=5 | - | - | 0.84 |
| CHECK_VERSION_STR mean | us (low is better) | 3.01 [2.63-3.66] n=5 | n/a | n/a | 2.21 [2.06-2.42] n=5 | - | - | 0.73 |

### (6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 5.41 [4.22-5.47] n=5 | n/a | n/a | 2.46 [2.32-2.92] n=5 | - | - | 0.45 |
| RM_CONTROL 232 B p99 | us (low is better) | 6.47 [5.40-7.06] n=5 | n/a | n/a | 4.80 [3.07-8.08] n=5 | - | - | 0.74 |
| RM_CONTROL 1576 B p50 | us (low is better) | 4.30 [4.20-6.67] n=5 | n/a | n/a | 2.48 [2.36-2.92] n=5 | - | - | 0.58 |
| RM_CONTROL 1576 B p99 | us (low is better) | 5.62 [5.39-8.03] n=5 | n/a | n/a | 3.76 [3.47-21.7] n=5 | - | - | 0.67 |
| RM_CONTROL 12328 B p50 | us (low is better) | 4.30 [4.22-5.40] n=5 | n/a | n/a | 2.50 [2.38-2.91] n=5 | - | - | 0.58 |
| RM_CONTROL 12328 B p99 | us (low is better) | 5.66 [5.43-7.77] n=5 | n/a | n/a | 4.02 [3.10-10.8] n=5 | - | - | 0.71 |
| RM_CONTROL 49192 B p50 | us (low is better) | 4.28 [4.23-5.32] n=5 | n/a | n/a | 2.51 [2.38-2.91] n=5 | - | - | 0.59 |
| RM_CONTROL 49192 B p99 | us (low is better) | 6.95 [5.36-7.73] n=5 | n/a | n/a | 5.10 [3.45-18.7] n=5 | - | - | 0.73 |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 35.8 [33.6-50.9] n=5 | n/a | n/a | 830 [824-835] n=5 | - | - | 23.18 |
| 64x1 MiB cuMemFree | us (low is better) | 27.1 [25.6-46.2] n=5 | n/a | n/a | 903 [887-1777] n=5 | - | - | 33.33 |
| 64x1 MiB cycle | ms (low is better) | 3.81 [3.77-4.52] n=5 | n/a | n/a | 168 [148-179] n=5 | - | - | 44.19 |
| 8x64 MiB cuMemAlloc | us (low is better) | 78.3 [75.0-81.2] n=5 | n/a | n/a | 6747 [5751-7226] n=5 | - | - | 86.17 |
| 8x64 MiB cuMemFree | us (low is better) | 92.6 [88.2-97.5] n=5 | n/a | n/a | 4422 [4190-5040] n=5 | - | - | 47.75 |
| 8x64 MiB cycle | ms (low is better) | 10.6 [9.09-11.1] n=5 | n/a | n/a | 115 [99.5-143] n=5 | - | - | 10.86 |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| copy device CPU | ms (low is better) | n/a | 10200 [9540-10740] n=3 | 10260 [8860-10520] n=3 | 24690 [24030-26060] n=5 | - | - | - |
| copy VMM CPU | ms (low is better) | n/a | 32910 [31440-36140] n=3 | 34660 [12440-35440] n=3 | 30480 [29500-31240] n=5 | - | - | - |
| init device CPU | ms (low is better) | n/a | 2980 [2760-3170] n=3 | 2640 [2590-3160] n=3 | 8140 [7420-8620] n=5 | - | - | - |
| init VMM CPU | ms (low is better) | n/a | 8870 [4580-9890] n=3 | 8710 [3180-10260] n=3 | 8150 [3000-8630] n=5 | - | - | - |
| ppo device CPU | ms (low is better) | n/a | 54310 [50460-58160] n=2 | 58305 [57640-58970] n=2 | n/a | - | - | - |
| ppo VMM CPU | ms (low is better) | n/a | 108990 [103280-114700] n=2 | 66445 [47020-85870] n=2 | n/a | - | - | - |
| rl device CPU | ms (low is better) | n/a | 21000 [20990-22190] n=3 | 21710 [21160-23500] n=3 | 14810 [14140-15520] n=5 | - | - | - |
| rl VMM CPU | ms (low is better) | n/a | 47480 [45830-52350] n=3 | 57630 [52450-63000] n=3 | 23240 [11420-24600] n=5 | - | - | - |
| short-auto device CPU | ms (low is better) | n/a | 2070 [2070-2140] n=3 | 2490 [2070-2630] n=3 | 6980 [6570-7400] n=5 | - | - | - |
| short-auto VMM CPU | ms (low is better) | n/a | 5950 [5220-7640] n=3 | 8190 [7290-9060] n=3 | 7870 [7560-8310] n=5 | - | - | - |
| churn device CPU | ms (low is better) | n/a | n/a | n/a | 17160 [15330-17780] n=5 | - | - | - |
| churn VMM CPU | ms (low is better) | n/a | n/a | n/a | 18650 [7460-18920] n=5 | - | - | - |
| ctrl device CPU | ms (low is better) | n/a | n/a | n/a | 260 [250-500] n=5 | - | - | - |
| ctrl VMM CPU | ms (low is better) | n/a | n/a | n/a | 440 [210-490] n=5 | - | - | - |
| ioctl device CPU | ms (low is better) | n/a | n/a | n/a | 240 [230-270] n=5 | - | - | - |
| ioctl VMM CPU | ms (low is better) | n/a | n/a | n/a | 410 [200-510] n=5 | - | - | - |
| launch-auto device CPU | ms (low is better) | n/a | n/a | n/a | 8300 [7980-8590] n=5 | - | - | - |
| launch-auto VMM CPU | ms (low is better) | n/a | n/a | n/a | 11230 [10650-11310] n=5 | - | - | - |
| launch-blocking device CPU | ms (low is better) | n/a | n/a | n/a | 9860 [9660-10110] n=5 | - | - | - |
| launch-blocking VMM CPU | ms (low is better) | n/a | n/a | n/a | 9920 [8920-10570] n=5 | - | - | - |
| mm device CPU | ms (low is better) | n/a | n/a | n/a | 14960 [14110-15400] n=5 | - | - | - |
| mm VMM CPU | ms (low is better) | n/a | n/a | n/a | 20430 [19880-21300] n=5 | - | - | - |
| short-blocking device CPU | ms (low is better) | n/a | n/a | n/a | 8390 [8130-8650] n=5 | - | - | - |
| short-blocking VMM CPU | ms (low is better) | n/a | n/a | n/a | 8440 [8060-8710] n=5 | - | - | - |
| time0 device CPU | ms (low is better) | n/a | n/a | n/a | 820 [490-850] n=5 | - | - | - |
| time0 VMM CPU | ms (low is better) | n/a | n/a | n/a | 650 [270-700] n=5 | - | - | - |
| time1 device CPU | ms (low is better) | n/a | n/a | n/a | 6310 [5820-6870] n=5 | - | - | - |
| time1 VMM CPU | ms (low is better) | n/a | n/a | n/a | 6650 [2850-7570] n=5 | - | - | - |

### (8) Device process memory (over all runs)

| value | unit | native | v2-615guest | v2-610guest | v2 | v2-615guest / native | v2-610guest / native | v2 / native |
|---|---|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | n/a | 6150 [6150-6151] n=14 | 7174 [5131-7175] n=14 | 9222 [9222-9222] n=65 | - | - | - |
| device RSS after run | MiB (low is better) | n/a | 5135 [5130-5143] n=14 | 5136 [5130-5143] n=14 | 5141 [5130-5142] n=65 | - | - | - |


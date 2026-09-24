Sources: native=/home/silas/git/Caraxes/caraxes/runs/closing-610-2026-09-24/native, v1=/home/silas/git/Caraxes/caraxes/runs/closing-610-2026-09-24/v1, v2=/home/silas/git/Caraxes/caraxes/runs/closing-610-2026-09-24/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): native 65/65, v1 52/65, v2 0/65
Host 1-min load average at run start, median [min-max]: native 3.48 [2.09-4.23] n=65, v1 3.49 [2.08-4.23] n=65, v2 3.52 [2.08-4.23] n=65

### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 16.3 [16.2-16.5] n=5 | 16.0 [16.0-16.4] n=4 | 17.3 [17.0-18.3] n=5 | 0.98 | 1.06 |

### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 16.1 [16.1-16.1] n=5 | 100 [100-100] n=4 | 18.0 [17.6-18.6] n=5 | 6.23 | 1.12 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.006 [0.006-0.009] n=5 | 0.009 [0.008-0.014] n=4 | 0.015 [0.014-0.355] n=5 | 1.50 | 2.50 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.021 [0.015-0.026] n=5 | 0.028 [0.017-4.42] n=4 | 0.100 [0.039-0.580] n=5 | 1.33 | 4.76 |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 12.1 [12.0-12.1] n=5 | 12.2 [12.1-12.2] n=4 | 6.06 [3.14-6.12] n=5 | 1.01 | 0.50 |
| d2h pinned 1 MiB | GB/s (high is better) | 12.3 [12.2-12.3] n=5 | 12.3 [12.3-12.4] n=4 | 6.18 [3.21-6.22] n=5 | 1.00 | 0.50 |
| d2d device 1 MiB | GB/s (high is better) | 132 [132-136] n=5 | 141 [135-143] n=4 | 62.4 [47.8-65.9] n=5 | 1.06 | 0.47 |
| h2d pageable 1 MiB | GB/s (high is better) | 7.25 [7.22-7.38] n=5 | 7.42 [7.28-7.44] n=4 | 4.50 [2.66-4.62] n=5 | 1.02 | 0.62 |
| d2h pageable 1 MiB | GB/s (high is better) | 6.07 [6.05-6.38] n=5 | 6.41 [6.18-6.54] n=4 | 4.17 [2.58-4.23] n=5 | 1.06 | 0.69 |
| h2d pinned 16 MiB | GB/s (high is better) | 13.0 [13.0-13.1] n=5 | 13.0 [13.0-13.1] n=4 | 6.57 [3.27-6.58] n=5 | 1.00 | 0.50 |
| d2h pinned 16 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 | 13.1 [13.1-13.1] n=4 | 6.62 [3.23-6.65] n=5 | 1.00 | 0.50 |
| d2d device 16 MiB | GB/s (high is better) | 183 [182-185] n=5 | 178 [178-181] n=4 | 16.5 [16.2-167] n=5 | 0.97 | 0.09 |
| h2d pageable 16 MiB | GB/s (high is better) | 9.90 [8.42-10.6] n=5 | 10.5 [8.97-10.8] n=4 | 6.30 [6.12-10.1] n=5 | 1.06 | 0.64 |
| d2h pageable 16 MiB | GB/s (high is better) | 8.77 [8.10-9.09] n=5 | 8.63 [8.33-8.83] n=4 | 6.32 [6.28-8.73] n=5 | 0.98 | 0.72 |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 | 13.1 [13.1-13.1] n=4 | 13.1 [13.1-13.1] n=5 | 1.00 | 1.00 |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [12.9-13.2] n=5 | 13.2 [13.2-13.2] n=4 | 13.2 [13.1-13.2] n=5 | 1.00 | 1.00 |
| d2d device 256 MiB | GB/s (high is better) | 186 [183-188] n=5 | 187 [183-189] n=4 | 190 [183-193] n=5 | 1.01 | 1.02 |
| h2d pageable 256 MiB | GB/s (high is better) | 9.27 [8.71-9.46] n=5 | 8.75 [8.40-9.01] n=4 | 8.86 [8.73-8.95] n=5 | 0.94 | 0.96 |
| d2h pageable 256 MiB | GB/s (high is better) | 9.00 [8.78-9.32] n=5 | 8.81 [8.15-8.91] n=4 | 8.35 [8.15-8.74] n=5 | 0.98 | 0.93 |

### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 4894 [4720-5264] n=5 | 5058 [4715-5331] n=4 | 1340 [379-1386] n=5 | 1.03 | 0.27 |
| N=2048 | GFLOP/s (high is better) | 5494 [5329-6093] n=5 | 5569 [5522-5616] n=4 | 2248 [2045-2422] n=5 | 1.01 | 0.41 |
| N=4096 | GFLOP/s (high is better) | 6052 [5834-6505] n=5 | 6163 [5823-6520] n=4 | 4433 [4230-5214] n=5 | 1.02 | 0.73 |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=4 | 56.9 [56.9-56.9] n=5 | 1.00 | 1.00 |
| process wall (incl. torch import) | s (low is better) | 4.64 [4.54-4.83] n=5 | 105 [10.1-119] n=4 | 12.2 [11.1-13.3] n=5 | 22.72 | 2.63 |
| stage4 (50 training episodes) | s (low is better) | 1.20 [1.20-1.40] n=5 | 1.50 [1.40-1.70] n=4 | 2.10 [2.00-2.50] n=5 | 1.25 | 1.75 |
| H2D+D2H transfers per second | 1/s (high is better) | 2350 [2129-2454] n=5 | 1930 [1662-2006] n=4 | 1340 [1155-1448] n=5 | 0.82 | 0.57 |

### (5) Module load to first successful cuInit (in the guest)

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| module load | ms (low is better) | n/a | 35.0 [29.0-61.0] n=5 | 3337 [3136-3627] n=5 | - | - |
| load to first cuInit | ms (low is better) | n/a | 218 [149-272] n=5 | 3509 [3285-3792] n=5 | - | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| cuInit | ms (low is better) | 106 [93.9-117] n=5 | 35.2 [33.3-41.0] n=4 | 151 [148-177] n=5 | 0.33 | 1.42 |
| cuCtxCreate | ms (low is better) | 195 [190-197] n=5 | 165 [157-170] n=4 | 3770 [3537-4049] n=5 | 0.84 | 19.29 |
| cuCtxDestroy | ms (low is better) | 92.6 [89.2-99.9] n=5 | 85.4 [79.1-88.8] n=4 | 1352 [1104-1383] n=5 | 0.92 | 14.60 |

### (5) nvprobe 0 process, bash time

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 225 [205-231] n=5 | 165 [154-191] n=4 | 185 [170-199] n=5 | 0.73 | 0.82 |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 158 [147-182] n=5 | 5.50 [5.00-6.00] n=4 | 185 [170-198] n=5 | 0.03 | 1.17 |

### (5) nvprobe 1 process, bash time

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 409 [384-445] n=5 | 356 [328-393] n=4 | 4172 [3768-4565] n=5 | 0.87 | 10.20 |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 349 [336-361] n=5 | 43.0 [38.0-47.0] n=4 | 4170 [3768-4563] n=5 | 0.12 | 11.95 |

### (6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 2.92 [2.59-3.33] n=5 | 20.4 [13.9-22.3] n=4 | 1.72 [1.49-2.47] n=5 | 6.97 | 0.59 |
| CHECK_VERSION_STR p99 | us (low is better) | 5.20 [3.20-15.2] n=5 | 46.5 [30.5-65.5] n=4 | 4.39 [2.76-7.31] n=5 | 8.94 | 0.84 |
| CHECK_VERSION_STR mean | us (low is better) | 3.01 [2.63-3.66] n=5 | 21.9 [17.7-26.4] n=4 | 2.21 [2.06-2.42] n=5 | 7.29 | 0.73 |

### (6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 5.41 [4.22-5.47] n=5 | 21.5 [18.1-23.9] n=4 | 2.46 [2.32-2.92] n=5 | 3.98 | 0.45 |
| RM_CONTROL 232 B p99 | us (low is better) | 6.47 [5.40-7.06] n=5 | 52.5 [36.4-63.6] n=4 | 4.80 [3.07-8.08] n=5 | 8.11 | 0.74 |
| RM_CONTROL 1576 B p50 | us (low is better) | 4.30 [4.20-6.67] n=5 | 23.0 [22.6-23.4] n=4 | 2.48 [2.36-2.92] n=5 | 5.35 | 0.58 |
| RM_CONTROL 1576 B p99 | us (low is better) | 5.62 [5.39-8.03] n=5 | 65.6 [34.9-265] n=4 | 3.76 [3.47-21.7] n=5 | 11.67 | 0.67 |
| RM_CONTROL 12328 B p50 | us (low is better) | 4.30 [4.22-5.40] n=5 | 27.2 [26.5-28.0] n=4 | 2.50 [2.38-2.91] n=5 | 6.33 | 0.58 |
| RM_CONTROL 12328 B p99 | us (low is better) | 5.66 [5.43-7.77] n=5 | 66.7 [41.1-1819] n=4 | 4.02 [3.10-10.8] n=5 | 11.78 | 0.71 |
| RM_CONTROL 49192 B p50 | us (low is better) | 4.28 [4.23-5.32] n=5 | 43.7 [32.8-49.0] n=4 | 2.51 [2.38-2.91] n=5 | 10.20 | 0.59 |
| RM_CONTROL 49192 B p99 | us (low is better) | 6.95 [5.36-7.73] n=5 | 229 [85.4-1949] n=4 | 5.10 [3.45-18.7] n=5 | 33.01 | 0.73 |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 35.8 [33.6-50.9] n=5 | 93.4 [83.0-128] n=4 | 830 [824-835] n=5 | 2.61 | 23.18 |
| 64x1 MiB cuMemFree | us (low is better) | 27.1 [25.6-46.2] n=5 | 52.5 [48.7-54.7] n=4 | 903 [887-1777] n=5 | 1.94 | 33.33 |
| 64x1 MiB cycle | ms (low is better) | 3.81 [3.77-4.52] n=5 | 8.51 [7.19-8.88] n=4 | 168 [148-179] n=5 | 2.23 | 44.19 |
| 8x64 MiB cuMemAlloc | us (low is better) | 78.3 [75.0-81.2] n=5 | 184 [151-195] n=4 | 6747 [5751-7226] n=5 | 2.34 | 86.17 |
| 8x64 MiB cuMemFree | us (low is better) | 92.6 [88.2-97.5] n=5 | 147 [119-153] n=4 | 4422 [4190-5040] n=5 | 1.59 | 47.75 |
| 8x64 MiB cycle | ms (low is better) | 10.6 [9.09-11.1] n=5 | 3.01 [2.93-4.32] n=4 | 115 [99.5-143] n=5 | 0.29 | 10.86 |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| churn device CPU | ms (low is better) | n/a | 380 [0.000-410] n=5 | 17160 [15330-17780] n=5 | - | - |
| churn VMM CPU | ms (low is better) | n/a | 420 [0.000-750] n=5 | 18650 [7460-18920] n=5 | - | - |
| copy device CPU | ms (low is better) | n/a | 300 [0.000-310] n=5 | 24690 [24030-26060] n=5 | - | - |
| copy VMM CPU | ms (low is better) | n/a | 1930 [0.000-2840] n=5 | 30480 [29500-31240] n=5 | - | - |
| ctrl device CPU | ms (low is better) | n/a | 130 [0.000-150] n=5 | 260 [250-500] n=5 | - | - |
| ctrl VMM CPU | ms (low is better) | n/a | 370 [0.000-460] n=5 | 440 [210-490] n=5 | - | - |
| init device CPU | ms (low is better) | n/a | 210 [0.000-220] n=5 | 8140 [7420-8620] n=5 | - | - |
| init VMM CPU | ms (low is better) | n/a | 770 [0.000-850] n=5 | 8150 [3000-8630] n=5 | - | - |
| ioctl device CPU | ms (low is better) | n/a | 100 [0.000-130] n=5 | 240 [230-270] n=5 | - | - |
| ioctl VMM CPU | ms (low is better) | n/a | 460 [0.000-570] n=5 | 410 [200-510] n=5 | - | - |
| launch-auto device CPU | ms (low is better) | n/a | 200 [0.000-220] n=5 | 8300 [7980-8590] n=5 | - | - |
| launch-auto VMM CPU | ms (low is better) | n/a | 2410 [0.000-4390] n=5 | 11230 [10650-11310] n=5 | - | - |
| launch-blocking device CPU | ms (low is better) | n/a | 230 [0.000-250] n=5 | 9860 [9660-10110] n=5 | - | - |
| launch-blocking VMM CPU | ms (low is better) | n/a | 310 [0.000-500] n=5 | 9920 [8920-10570] n=5 | - | - |
| mm device CPU | ms (low is better) | n/a | 310 [0.000-350] n=5 | 14960 [14110-15400] n=5 | - | - |
| mm VMM CPU | ms (low is better) | n/a | 12030 [0.000-22190] n=5 | 20430 [19880-21300] n=5 | - | - |
| rl device CPU | ms (low is better) | n/a | 430 [0.000-490] n=5 | 14810 [14140-15520] n=5 | - | - |
| rl VMM CPU | ms (low is better) | n/a | 96040 [0.000-113330] n=5 | 23240 [11420-24600] n=5 | - | - |
| short-auto device CPU | ms (low is better) | n/a | 280 [0.000-290] n=5 | 6980 [6570-7400] n=5 | - | - |
| short-auto VMM CPU | ms (low is better) | n/a | 270 [0.000-390] n=5 | 7870 [7560-8310] n=5 | - | - |
| short-blocking device CPU | ms (low is better) | n/a | 220 [0.000-220] n=5 | 8390 [8130-8650] n=5 | - | - |
| short-blocking VMM CPU | ms (low is better) | n/a | 240 [0.000-290] n=5 | 8440 [8060-8710] n=5 | - | - |
| time0 device CPU | ms (low is better) | n/a | 100 [0.000-140] n=5 | 820 [490-850] n=5 | - | - |
| time0 VMM CPU | ms (low is better) | n/a | 140 [0.000-150] n=5 | 650 [270-700] n=5 | - | - |
| time1 device CPU | ms (low is better) | n/a | 230 [0.000-250] n=5 | 6310 [5820-6870] n=5 | - | - |
| time1 VMM CPU | ms (low is better) | n/a | 230 [0.000-260] n=5 | 6650 [2850-7570] n=5 | - | - |

### (8) Device process memory (over all runs)

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | n/a | 145 [3.13-541] n=65 | 9222 [9222-9222] n=65 | - | - |
| device RSS after run | MiB (low is better) | n/a | 103 [3.13-495] n=65 | 5141 [5130-5142] n=65 | - | - |


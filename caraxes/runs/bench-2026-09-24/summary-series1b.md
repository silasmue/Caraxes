Sources: v1=series1b/v1, native=series1b/native
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v1 27/65, native 28/65
Host 1-min load average at run start, median [min-max]: v1 7.01 [5.07-16.4] n=65, native 6.92 [5.07-16.2] n=65

### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 16.1 [16.0-16.3] n=5 | 16.4 [16.4-16.5] n=5 | 0.98 |

### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 100 [100-101] n=5 | 16.1 [16.1-16.1] n=5 | 6.23 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.009 [0.007-0.043] n=5 | 0.007 [0.006-0.016] n=5 | 1.29 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 31.3 [0.020-72.6] n=5 | 0.024 [0.015-0.097] n=5 | 1305.58 |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 12.0 [7.33-12.2] n=5 | 12.0 [12.0-12.1] n=5 | 1.00 |
| d2h pinned 1 MiB | GB/s (high is better) | 12.3 [9.14-12.4] n=5 | 12.3 [12.1-12.3] n=5 | 1.00 |
| d2d device 1 MiB | GB/s (high is better) | 139 [5.48-147] n=5 | 132 [120-142] n=5 | 1.06 |
| h2d pageable 1 MiB | GB/s (high is better) | 7.25 [3.46-7.33] n=5 | 7.27 [6.73-7.35] n=5 | 1.00 |
| d2h pageable 1 MiB | GB/s (high is better) | 6.28 [5.09-6.32] n=5 | 6.19 [4.91-6.39] n=5 | 1.01 |
| h2d pinned 16 MiB | GB/s (high is better) | 13.0 [12.8-13.1] n=5 | 13.0 [13.0-13.1] n=5 | 1.00 |
| d2h pinned 16 MiB | GB/s (high is better) | 13.1 [13.0-13.1] n=5 | 13.1 [13.0-13.1] n=5 | 1.00 |
| d2d device 16 MiB | GB/s (high is better) | 173 [151-180] n=5 | 182 [180-183] n=5 | 0.95 |
| h2d pageable 16 MiB | GB/s (high is better) | 7.85 [6.33-9.09] n=5 | 9.34 [6.06-10.6] n=5 | 0.84 |
| d2h pageable 16 MiB | GB/s (high is better) | 7.18 [5.82-8.90] n=5 | 8.02 [5.53-8.34] n=5 | 0.89 |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 | 13.1 [13.0-13.1] n=5 | 1.00 |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [13.2-13.2] n=5 | 13.2 [13.0-13.2] n=5 | 1.00 |
| d2d device 256 MiB | GB/s (high is better) | 184 [183-187] n=5 | 185 [165-189] n=5 | 1.00 |
| h2d pageable 256 MiB | GB/s (high is better) | 6.95 [3.76-8.23] n=5 | 8.21 [5.88-8.78] n=5 | 0.85 |
| d2h pageable 256 MiB | GB/s (high is better) | 7.54 [4.16-8.39] n=5 | 8.01 [5.59-8.39] n=5 | 0.94 |

### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 4742 [2750-5239] n=5 | 5304 [4517-6148] n=5 | 0.89 |
| N=2048 | GFLOP/s (high is better) | 5489 [5175-5566] n=5 | 5619 [5565-6267] n=5 | 0.98 |
| N=4096 | GFLOP/s (high is better) | 6496 [6103-6547] n=5 | 6481 [5869-6677] n=5 | 1.00 |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | 1.00 |
| process wall (incl. torch import) | s (low is better) | 32.7 [16.9-85.5] n=5 | 5.94 [5.24-9.58] n=5 | 5.51 |
| stage4 (50 training episodes) | s (low is better) | 2.10 [1.90-4.30] n=5 | 1.40 [1.30-2.30] n=5 | 1.50 |
| H2D+D2H transfers per second | 1/s (high is better) | 1356 [662-1543] n=5 | 2045 [1253-2231] n=5 | 0.66 |

### (5) Module load to first successful cuInit (in the guest)

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| module load | ms (low is better) | 30.0 [22.0-146] n=5 | n/a | - |
| load to first cuInit | ms (low is better) | 183 [148-320] n=5 | n/a | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| cuInit | ms (low is better) | 34.8 [29.9-59.4] n=5 | 35.1 [32.2-55.4] n=5 | 0.99 |
| cuCtxCreate | ms (low is better) | 190 [162-291] n=5 | 161 [146-247] n=5 | 1.18 |
| cuCtxDestroy | ms (low is better) | 92.0 [84.1-153] n=5 | 85.3 [76.1-116] n=5 | 1.08 |

### (5) nvprobe 0 process, bash time

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 106 [67.0-127] n=5 | 93.0 [63.0-126] n=5 | 1.14 |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 5.00 [4.00-16.0] n=5 | 46.0 [41.0-55.0] n=5 | 0.11 |

### (5) nvprobe 1 process, bash time

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 355 [332-563] n=5 | 348 [314-462] n=5 | 1.02 |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 46.0 [42.0-95.0] n=5 | 285 [267-296] n=5 | 0.16 |

### (6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 16.7 [14.1-38.8] n=5 | 2.68 [2.59-3.18] n=5 | 6.22 |
| CHECK_VERSION_STR p99 | us (low is better) | 44.0 [33.8-798] n=5 | 3.62 [3.36-3.85] n=5 | 12.17 |
| CHECK_VERSION_STR mean | us (low is better) | 20.0 [16.1-65.0] n=5 | 3.22 [2.66-5.78] n=5 | 6.21 |

### (6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 27.3 [19.7-68.8] n=5 | 4.51 [4.33-5.78] n=5 | 6.06 |
| RM_CONTROL 232 B p99 | us (low is better) | 67.7 [37.3-920] n=5 | 7.48 [5.62-9.64] n=5 | 9.05 |
| RM_CONTROL 1576 B p50 | us (low is better) | 27.7 [18.8-65.8] n=5 | 4.53 [4.31-5.28] n=5 | 6.12 |
| RM_CONTROL 1576 B p99 | us (low is better) | 71.2 [36.9-1552] n=5 | 6.57 [5.54-18.1] n=5 | 10.83 |
| RM_CONTROL 12328 B p50 | us (low is better) | 24.7 [23.7-61.0] n=5 | 4.50 [4.30-5.30] n=5 | 5.48 |
| RM_CONTROL 12328 B p99 | us (low is better) | 270 [70.8-2085] n=5 | 6.21 [5.50-6.76] n=5 | 43.49 |
| RM_CONTROL 49192 B p50 | us (low is better) | 44.5 [37.9-73.1] n=5 | 4.50 [4.31-5.34] n=5 | 9.90 |
| RM_CONTROL 49192 B p99 | us (low is better) | 75.0 [56.9-2124] n=5 | 5.94 [5.53-16.6] n=5 | 12.62 |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 87.6 [72.2-129] n=5 | 45.2 [36.4-68.2] n=5 | 1.94 |
| 64x1 MiB cuMemFree | us (low is better) | 56.1 [51.7-82.8] n=5 | 31.5 [28.7-51.7] n=5 | 1.78 |
| 64x1 MiB cycle | ms (low is better) | 9.98 [7.22-12.4] n=5 | 4.19 [3.94-4.95] n=5 | 2.38 |
| 8x64 MiB cuMemAlloc | us (low is better) | 199 [166-338] n=5 | 94.3 [88.9-110] n=5 | 2.11 |
| 8x64 MiB cuMemFree | us (low is better) | 152 [124-243] n=5 | 95.6 [91.1-114] n=5 | 1.59 |
| 8x64 MiB cycle | ms (low is better) | 9.46 [8.37-9.71] n=5 | 10.4 [9.60-12.8] n=5 | 0.91 |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| churn device CPU | ms (low is better) | 420 [400-470] n=5 | n/a | - |
| churn VMM CPU | ms (low is better) | 520 [390-640] n=5 | n/a | - |
| copy device CPU | ms (low is better) | 310 [300-380] n=5 | n/a | - |
| copy VMM CPU | ms (low is better) | 8540 [5610-34170] n=5 | n/a | - |
| ctrl device CPU | ms (low is better) | 140 [100-240] n=5 | n/a | - |
| ctrl VMM CPU | ms (low is better) | 490 [300-570] n=5 | n/a | - |
| init device CPU | ms (low is better) | 230 [230-270] n=5 | n/a | - |
| init VMM CPU | ms (low is better) | 310 [260-530] n=5 | n/a | - |
| ioctl device CPU | ms (low is better) | 110 [90.0-180] n=5 | n/a | - |
| ioctl VMM CPU | ms (low is better) | 400 [290-480] n=5 | n/a | - |
| launch-auto device CPU | ms (low is better) | 220 [200-240] n=5 | n/a | - |
| launch-auto VMM CPU | ms (low is better) | 3680 [2740-4330] n=5 | n/a | - |
| launch-blocking device CPU | ms (low is better) | 250 [230-270] n=5 | n/a | - |
| launch-blocking VMM CPU | ms (low is better) | 8570 [490-8820] n=5 | n/a | - |
| mm device CPU | ms (low is better) | 370 [350-380] n=5 | n/a | - |
| mm VMM CPU | ms (low is better) | 10510 [8320-28830] n=5 | n/a | - |
| rl device CPU | ms (low is better) | 360 [330-380] n=5 | n/a | - |
| rl VMM CPU | ms (low is better) | 30010 [12660-75650] n=5 | n/a | - |
| short-auto device CPU | ms (low is better) | 240 [210-250] n=5 | n/a | - |
| short-auto VMM CPU | ms (low is better) | 770 [220-1140] n=5 | n/a | - |
| short-blocking device CPU | ms (low is better) | 230 [220-250] n=5 | n/a | - |
| short-blocking VMM CPU | ms (low is better) | 770 [320-5320] n=5 | n/a | - |
| time0 device CPU | ms (low is better) | 50.0 [40.0-60.0] n=5 | n/a | - |
| time0 VMM CPU | ms (low is better) | 140 [120-180] n=5 | n/a | - |
| time1 device CPU | ms (low is better) | 240 [230-270] n=5 | n/a | - |
| time1 VMM CPU | ms (low is better) | 240 [230-290] n=5 | n/a | - |

### (8) Device process memory (over all runs)

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| device HWM | MiB (low is better) | 460 [67.7-530] n=65 | n/a | - |
| device RSS after run | MiB (low is better) | 420 [30.9-486] n=65 | n/a | - |


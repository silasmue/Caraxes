Sources: v1=series1b/v1, v2 (series2)=series2/v2, v2 (series2-rep2-build-c09fcccc)=series2-rep2-build-c09fcccc/v2, native (series1b)=series1b/native, native (series2)=series2/native
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v1 27/65, v2 (series2) 2/13, v2 (series2-rep2-build-c09fcccc) 0/13, native (series1b) 28/65, native (series2) 25/65
Host 1-min load average at run start, median [min-max]: v1 7.01 [5.07-16.4] n=65, v2 (series2) 3.16 [2.73-7.99] n=13, v2 (series2-rep2-build-c09fcccc) 4.91 [3.73-5.27] n=13, native (series1b) 6.92 [5.07-16.2] n=65, native (series2) 5.02 [2.71-17.6] n=65

### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 16.1 [16.0-16.3] n=5 | 18.4 [18.4-18.4] n=1 | 18.1 [18.1-18.1] n=1 | 16.4 [16.4-16.5] n=5 | 16.4 [16.4-16.5] n=5 | 0.98 | 1.12 | 1.10 |

### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 100 [100-101] n=5 | 101 [101-101] n=1 | 18.7 [18.7-18.7] n=1 | 16.1 [16.1-16.1] n=5 | 16.6 [16.3-16.8] n=5 | 6.23 | 6.10 | 1.13 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.009 [0.007-0.043] n=5 | 0.208 [0.208-0.208] n=1 | 0.402 [0.402-0.402] n=1 | 0.007 [0.006-0.016] n=5 | 0.007 [0.006-0.007] n=5 | 1.29 | 29.71 | 57.43 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 31.3 [0.020-72.6] n=5 | 59.5 [59.5-59.5] n=1 | 0.322 [0.322-0.322] n=1 | 0.024 [0.015-0.097] n=5 | 0.021 [0.014-0.031] n=5 | 1305.58 | 2831.90 | 15.33 |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 12.0 [7.33-12.2] n=5 | 2.22 [2.22-2.22] n=1 | 3.43 [3.43-3.43] n=1 | 12.0 [12.0-12.1] n=5 | 12.1 [12.1-12.1] n=5 | 1.00 | 0.18 | 0.28 |
| d2h pinned 1 MiB | GB/s (high is better) | 12.3 [9.14-12.4] n=5 | 1.88 [1.88-1.88] n=1 | 3.57 [3.57-3.57] n=1 | 12.3 [12.1-12.3] n=5 | 12.3 [12.2-12.3] n=5 | 1.00 | 0.15 | 0.29 |
| d2d device 1 MiB | GB/s (high is better) | 139 [5.48-147] n=5 | 3.65 [3.65-3.65] n=1 | 7.37 [7.37-7.37] n=1 | 132 [120-142] n=5 | 136 [126-149] n=5 | 1.06 | 0.03 | 0.05 |
| h2d pageable 1 MiB | GB/s (high is better) | 7.25 [3.46-7.33] n=5 | 1.65 [1.65-1.65] n=1 | 3.05 [3.05-3.05] n=1 | 7.27 [6.73-7.35] n=5 | 7.29 [6.61-7.32] n=5 | 1.00 | 0.23 | 0.42 |
| d2h pageable 1 MiB | GB/s (high is better) | 6.28 [5.09-6.32] n=5 | 1.92 [1.92-1.92] n=1 | 2.60 [2.60-2.60] n=1 | 6.19 [4.91-6.39] n=5 | 6.04 [4.66-6.48] n=5 | 1.01 | 0.32 | 0.43 |
| h2d pinned 16 MiB | GB/s (high is better) | 13.0 [12.8-13.1] n=5 | 3.09 [3.09-3.09] n=1 | 3.17 [3.17-3.17] n=1 | 13.0 [13.0-13.1] n=5 | 13.0 [13.0-13.1] n=5 | 1.00 | 0.24 | 0.24 |
| d2h pinned 16 MiB | GB/s (high is better) | 13.1 [13.0-13.1] n=5 | 3.15 [3.15-3.15] n=1 | 3.15 [3.15-3.15] n=1 | 13.1 [13.0-13.1] n=5 | 13.1 [13.1-13.1] n=5 | 1.00 | 0.24 | 0.24 |
| d2d device 16 MiB | GB/s (high is better) | 173 [151-180] n=5 | 4.09 [4.09-4.09] n=1 | 57.2 [57.2-57.2] n=1 | 182 [180-183] n=5 | 181 [180-184] n=5 | 0.95 | 0.02 | 0.32 |
| h2d pageable 16 MiB | GB/s (high is better) | 7.85 [6.33-9.09] n=5 | 4.07 [4.07-4.07] n=1 | 3.78 [3.78-3.78] n=1 | 9.34 [6.06-10.6] n=5 | 8.68 [4.63-11.0] n=5 | 0.84 | 0.47 | 0.44 |
| d2h pageable 16 MiB | GB/s (high is better) | 7.18 [5.82-8.90] n=5 | 4.33 [4.33-4.33] n=1 | 3.82 [3.82-3.82] n=1 | 8.02 [5.53-8.34] n=5 | 8.76 [5.39-8.83] n=5 | 0.89 | 0.49 | 0.44 |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 | 12.9 [12.9-12.9] n=1 | 13.0 [13.0-13.0] n=1 | 13.1 [13.0-13.1] n=5 | 13.1 [13.0-13.1] n=5 | 1.00 | 0.98 | 0.99 |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [13.2-13.2] n=5 | 13.0 [13.0-13.0] n=1 | 13.1 [13.1-13.1] n=1 | 13.2 [13.0-13.2] n=5 | 13.2 [13.1-13.2] n=5 | 1.00 | 0.99 | 0.99 |
| d2d device 256 MiB | GB/s (high is better) | 184 [183-187] n=5 | 169 [169-169] n=1 | 175 [175-175] n=1 | 185 [165-189] n=5 | 188 [187-190] n=5 | 1.00 | 0.90 | 0.93 |
| h2d pageable 256 MiB | GB/s (high is better) | 6.95 [3.76-8.23] n=5 | 4.04 [4.04-4.04] n=1 | 3.14 [3.14-3.14] n=1 | 8.21 [5.88-8.78] n=5 | 9.10 [5.99-9.36] n=5 | 0.85 | 0.44 | 0.34 |
| d2h pageable 256 MiB | GB/s (high is better) | 7.54 [4.16-8.39] n=5 | 4.10 [4.10-4.10] n=1 | 3.49 [3.49-3.49] n=1 | 8.01 [5.59-8.39] n=5 | 8.45 [5.43-9.24] n=5 | 0.94 | 0.49 | 0.41 |

### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 4742 [2750-5239] n=5 | 250 [250-250] n=1 | 352 [352-352] n=1 | 5304 [4517-6148] n=5 | 5054 [4786-5430] n=5 | 0.89 | 0.05 | 0.07 |
| N=2048 | GFLOP/s (high is better) | 5489 [5175-5566] n=5 | 681 [681-681] n=1 | 648 [648-648] n=1 | 5619 [5565-6267] n=5 | 5740 [5298-6318] n=5 | 0.98 | 0.12 | 0.11 |
| N=4096 | GFLOP/s (high is better) | 6496 [6103-6547] n=5 | 5050 [5050-5050] n=1 | 4650 [4650-4650] n=1 | 6481 [5869-6677] n=5 | 6200 [5862-6777] n=5 | 1.00 | 0.81 | 0.75 |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=1 | 56.9 [56.9-56.9] n=1 | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | 1.00 | 1.00 | 1.00 |
| process wall (incl. torch import) | s (low is better) | 32.7 [16.9-85.5] n=5 | 91.8 [91.8-91.8] n=1 | 39.4 [39.4-39.4] n=1 | 5.94 [5.24-9.58] n=5 | 5.36 [4.62-6.69] n=5 | 5.51 | 17.15 | 7.35 |
| stage4 (50 training episodes) | s (low is better) | 2.10 [1.90-4.30] n=5 | 17.8 [17.8-17.8] n=1 | 22.7 [22.7-22.7] n=1 | 1.40 [1.30-2.30] n=5 | 1.40 [1.20-1.70] n=5 | 1.50 | 12.71 | 16.21 |
| H2D+D2H transfers per second | 1/s (high is better) | 1356 [662-1543] n=5 | 161 [161-161] n=1 | 127 [127-127] n=1 | 2045 [1253-2231] n=5 | 2072 [1668-2484] n=5 | 0.66 | 0.08 | 0.06 |

### (5) Module load to first successful cuInit (in the guest)

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| module load | ms (low is better) | 30.0 [22.0-146] n=5 | 3376 [3248-4871] n=5 | n/a | n/a | n/a | - | - | - |
| load to first cuInit | ms (low is better) | 183 [148-320] n=5 | 3591 [3414-5077] n=5 | n/a | n/a | n/a | - | - | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| cuInit | ms (low is better) | 34.8 [29.9-59.4] n=5 | 147 [147-147] n=1 | 172 [172-172] n=1 | 35.1 [32.2-55.4] n=5 | 95.8 [74.8-110] n=5 | 0.99 | 1.54 | 1.80 |
| cuCtxCreate | ms (low is better) | 190 [162-291] n=5 | 64621 [64621-64621] n=1 | 5418 [5418-5418] n=1 | 161 [146-247] n=5 | 190 [174-190] n=5 | 1.18 | 340.36 | 28.54 |
| cuCtxDestroy | ms (low is better) | 92.0 [84.1-153] n=5 | 3212 [3212-3212] n=1 | 3730 [3730-3730] n=1 | 85.3 [76.1-116] n=5 | 91.8 [85.1-97.6] n=5 | 1.08 | 34.97 | 40.62 |

### (5) nvprobe 0 process, bash time

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 106 [67.0-127] n=5 | 187 [187-187] n=1 | 223 [223-223] n=1 | 93.0 [63.0-126] n=5 | 201 [165-216] n=5 | 1.14 | 0.93 | 1.11 |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 5.00 [4.00-16.0] n=5 | 186 [186-186] n=1 | 223 [223-223] n=1 | 46.0 [41.0-55.0] n=5 | 124 [117-153] n=5 | 0.11 | 1.50 | 1.80 |

### (5) nvprobe 1 process, bash time

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 355 [332-563] n=5 | 67921 [67921-67921] n=1 | 8625 [8625-8625] n=1 | 348 [314-462] n=5 | 426 [391-615] n=5 | 1.02 | 159.44 | 20.25 |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 46.0 [42.0-95.0] n=5 | 67902 [67902-67902] n=1 | 8628 [8628-8628] n=1 | 285 [267-296] n=5 | 350 [338-499] n=5 | 0.16 | 194.01 | 24.65 |

### (6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 16.7 [14.1-38.8] n=5 | 2.04 [2.04-2.04] n=1 | 1.33 [1.33-1.33] n=1 | 2.68 [2.59-3.18] n=5 | 2.58 [2.58-2.72] n=5 | 6.22 | 0.79 | 0.52 |
| CHECK_VERSION_STR p99 | us (low is better) | 44.0 [33.8-798] n=5 | 3.25 [3.25-3.25] n=1 | 2.53 [2.53-2.53] n=1 | 3.62 [3.36-3.85] n=5 | 3.72 [3.28-5.20] n=5 | 12.17 | 0.87 | 0.68 |
| CHECK_VERSION_STR mean | us (low is better) | 20.0 [16.1-65.0] n=5 | 1.95 [1.95-1.95] n=1 | 1.50 [1.50-1.50] n=1 | 3.22 [2.66-5.78] n=5 | 2.81 [2.65-2.84] n=5 | 6.21 | 0.69 | 0.53 |

### (6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 27.3 [19.7-68.8] n=5 | 2.34 [2.34-2.34] n=1 | 2.82 [2.82-2.82] n=1 | 4.51 [4.33-5.78] n=5 | 4.47 [4.34-5.41] n=5 | 6.06 | 0.52 | 0.63 |
| RM_CONTROL 232 B p99 | us (low is better) | 67.7 [37.3-920] n=5 | 4.10 [4.10-4.10] n=1 | 5.28 [5.28-5.28] n=1 | 7.48 [5.62-9.64] n=5 | 6.87 [5.70-7.15] n=5 | 9.05 | 0.60 | 0.77 |
| RM_CONTROL 1576 B p50 | us (low is better) | 27.7 [18.8-65.8] n=5 | 2.41 [2.41-2.41] n=1 | 2.84 [2.84-2.84] n=1 | 4.53 [4.31-5.28] n=5 | 4.43 [4.16-5.43] n=5 | 6.12 | 0.54 | 0.64 |
| RM_CONTROL 1576 B p99 | us (low is better) | 71.2 [36.9-1552] n=5 | 3.50 [3.50-3.50] n=1 | 4.91 [4.91-4.91] n=1 | 6.57 [5.54-18.1] n=5 | 5.94 [5.05-6.44] n=5 | 10.83 | 0.59 | 0.83 |
| RM_CONTROL 12328 B p50 | us (low is better) | 24.7 [23.7-61.0] n=5 | 2.45 [2.45-2.45] n=1 | 2.82 [2.82-2.82] n=1 | 4.50 [4.30-5.30] n=5 | 4.48 [4.17-4.50] n=5 | 5.48 | 0.55 | 0.63 |
| RM_CONTROL 12328 B p99 | us (low is better) | 270 [70.8-2085] n=5 | 3.38 [3.38-3.38] n=1 | 7.64 [7.64-7.64] n=1 | 6.21 [5.50-6.76] n=5 | 5.40 [5.29-6.38] n=5 | 43.49 | 0.63 | 1.41 |
| RM_CONTROL 49192 B p50 | us (low is better) | 44.5 [37.9-73.1] n=5 | 2.56 [2.56-2.56] n=1 | 2.82 [2.82-2.82] n=1 | 4.50 [4.31-5.34] n=5 | 4.49 [4.23-5.30] n=5 | 9.90 | 0.57 | 0.63 |
| RM_CONTROL 49192 B p99 | us (low is better) | 75.0 [56.9-2124] n=5 | 3.81 [3.81-3.81] n=1 | 4.55 [4.55-4.55] n=1 | 5.94 [5.53-16.6] n=5 | 5.70 [5.48-6.46] n=5 | 12.62 | 0.67 | 0.80 |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 87.6 [72.2-129] n=5 | 20497 [20497-20497] n=1 | 20454 [20454-20454] n=1 | 45.2 [36.4-68.2] n=5 | 41.2 [35.2-63.8] n=5 | 1.94 | 497.50 | 496.45 |
| 64x1 MiB cuMemFree | us (low is better) | 56.1 [51.7-82.8] n=5 | 20504 [20504-20504] n=1 | 21378 [21378-21378] n=1 | 31.5 [28.7-51.7] n=5 | 29.3 [25.1-46.7] n=5 | 1.78 | 699.81 | 729.61 |
| 64x1 MiB cycle | ms (low is better) | 9.98 [7.22-12.4] n=5 | 2902 [2902-2902] n=1 | 3066 [3066-3066] n=1 | 4.19 [3.94-4.95] n=5 | 4.08 [3.86-5.54] n=5 | 2.38 | 711.75 | 751.96 |
| 8x64 MiB cuMemAlloc | us (low is better) | 199 [166-338] n=5 | 53329 [53329-53329] n=1 | 59557 [59557-59557] n=1 | 94.3 [88.9-110] n=5 | 108 [100-142] n=5 | 2.11 | 491.96 | 549.42 |
| 8x64 MiB cuMemFree | us (low is better) | 152 [124-243] n=5 | 48301 [48301-48301] n=1 | 53421 [53421-53421] n=1 | 95.6 [91.1-114] n=5 | 101 [94.6-123] n=5 | 1.59 | 479.66 | 530.50 |
| 8x64 MiB cycle | ms (low is better) | 9.46 [8.37-9.71] n=5 | 867 [867-867] n=1 | 946 [946-946] n=1 | 10.4 [9.60-12.8] n=5 | 11.9 [10.1-12.7] n=5 | 0.91 | 72.70 | 79.26 |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| churn device CPU | ms (low is better) | 420 [400-470] n=5 | 101430 [101430-101430] n=1 | 80310 [80310-80310] n=1 | n/a | n/a | - | - | - |
| churn VMM CPU | ms (low is better) | 520 [390-640] n=5 | 51660 [51660-51660] n=1 | 11770 [11770-11770] n=1 | n/a | n/a | - | - | - |
| copy device CPU | ms (low is better) | 310 [300-380] n=5 | 49000 [49000-49000] n=1 | 24850 [24850-24850] n=1 | n/a | n/a | - | - | - |
| copy VMM CPU | ms (low is better) | 8540 [5610-34170] n=5 | 57340 [57340-57340] n=1 | 42620 [42620-42620] n=1 | n/a | n/a | - | - | - |
| ctrl device CPU | ms (low is better) | 140 [100-240] n=5 | 0.000 [0.000-0.000] n=1 | 10.0 [10.0-10.0] n=1 | n/a | n/a | - | - | - |
| ctrl VMM CPU | ms (low is better) | 490 [300-570] n=5 | 180 [180-180] n=1 | 220 [220-220] n=1 | n/a | n/a | - | - | - |
| init device CPU | ms (low is better) | 230 [230-270] n=5 | 30320 [30320-30320] n=1 | 6520 [6520-6520] n=1 | n/a | n/a | - | - | - |
| init VMM CPU | ms (low is better) | 310 [260-530] n=5 | 46690 [46690-46690] n=1 | 4070 [4070-4070] n=1 | n/a | n/a | - | - | - |
| ioctl device CPU | ms (low is better) | 110 [90.0-180] n=5 | 0.000 [0.000-0.000] n=1 | 10.0 [10.0-10.0] n=1 | n/a | n/a | - | - | - |
| ioctl VMM CPU | ms (low is better) | 400 [290-480] n=5 | 420 [420-420] n=1 | 200 [200-200] n=1 | n/a | n/a | - | - | - |
| launch-auto device CPU | ms (low is better) | 220 [200-240] n=5 | 28560 [28560-28560] n=1 | 6110 [6110-6110] n=1 | n/a | n/a | - | - | - |
| launch-auto VMM CPU | ms (low is better) | 3680 [2740-4330] n=5 | 41580 [41580-41580] n=1 | 8530 [8530-8530] n=1 | n/a | n/a | - | - | - |
| launch-blocking device CPU | ms (low is better) | 250 [230-270] n=5 | 29710 [29710-29710] n=1 | 6190 [6190-6190] n=1 | n/a | n/a | - | - | - |
| launch-blocking VMM CPU | ms (low is better) | 8570 [490-8820] n=5 | 43780 [43780-43780] n=1 | 11470 [11470-11470] n=1 | n/a | n/a | - | - | - |
| mm device CPU | ms (low is better) | 370 [350-380] n=5 | 31260 [31260-31260] n=1 | 7760 [7760-7760] n=1 | n/a | n/a | - | - | - |
| mm VMM CPU | ms (low is better) | 10510 [8320-28830] n=5 | 52490 [52490-52490] n=1 | 26830 [26830-26830] n=1 | n/a | n/a | - | - | - |
| rl device CPU | ms (low is better) | 360 [330-380] n=5 | 38650 [38650-38650] n=1 | 16270 [16270-16270] n=1 | n/a | n/a | - | - | - |
| rl VMM CPU | ms (low is better) | 30010 [12660-75650] n=5 | 62480 [62480-62480] n=1 | 55400 [55400-55400] n=1 | n/a | n/a | - | - | - |
| short-auto device CPU | ms (low is better) | 240 [210-250] n=5 | 30240 [30240-30240] n=1 | 6190 [6190-6190] n=1 | n/a | n/a | - | - | - |
| short-auto VMM CPU | ms (low is better) | 770 [220-1140] n=5 | 46240 [46240-46240] n=1 | 11710 [11710-11710] n=1 | n/a | n/a | - | - | - |
| short-blocking device CPU | ms (low is better) | 230 [220-250] n=5 | 30530 [30530-30530] n=1 | 6370 [6370-6370] n=1 | n/a | n/a | - | - | - |
| short-blocking VMM CPU | ms (low is better) | 770 [320-5320] n=5 | 47180 [47180-47180] n=1 | 11090 [11090-11090] n=1 | n/a | n/a | - | - | - |
| time0 device CPU | ms (low is better) | 50.0 [40.0-60.0] n=5 | 100 [100-100] n=1 | 120 [120-120] n=1 | n/a | n/a | - | - | - |
| time0 VMM CPU | ms (low is better) | 140 [120-180] n=5 | 480 [480-480] n=1 | 320 [320-320] n=1 | n/a | n/a | - | - | - |
| time1 device CPU | ms (low is better) | 240 [230-270] n=5 | 30470 [30470-30470] n=1 | 6070 [6070-6070] n=1 | n/a | n/a | - | - | - |
| time1 VMM CPU | ms (low is better) | 240 [230-290] n=5 | 46790 [46790-46790] n=1 | 3350 [3350-3350] n=1 | n/a | n/a | - | - | - |

### (8) Device process memory (over all runs)

| value | unit | v1 | v2 (series2) | v2 (series2-rep2-build-c09fcccc) | native (series1b) | native (series2) | v1 / native (series1b) | v2 (series2) / native (series2) | v2 (series2-rep2-build-c09fcccc) / native (series2) |
|---|---|---|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 460 [67.7-530] n=65 | 4108 [4108-4109] n=13 | 4122 [4108-4122] n=13 | n/a | n/a | - | - | - |
| device RSS after run | MiB (low is better) | 420 [30.9-486] n=65 | 3290 [3290-4109] n=13 | 4120 [1515-4120] n=13 | n/a | n/a | - | - | - |


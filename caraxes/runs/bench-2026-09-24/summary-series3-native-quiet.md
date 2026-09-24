Sources: native=caraxes/runs/bench-2026-09-24/series3-native-quiet/native
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): native 0/65
Host 1-min load average at run start, median [min-max]: native 2.30 [1.62-2.51] n=65

### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | native |  |
|---|---|---|---|
| ms/launch | ms (low is better) | 16.0 [16.0-16.2] n=5 |  |

### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | native |  |
|---|---|---|---|
| ms/launch | ms (low is better) | 16.1 [16.1-16.1] n=5 |  |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | native |  |
|---|---|---|---|
| ms/launch | ms (low is better) | 0.006 [0.006-0.006] n=5 |  |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | native |  |
|---|---|---|---|
| ms/launch | ms (low is better) | 0.016 [0.014-0.017] n=5 |  |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | native |  |
|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 12.2 [12.2-12.2] n=5 |  |
| d2h pinned 1 MiB | GB/s (high is better) | 12.4 [12.4-12.4] n=5 |  |
| d2d device 1 MiB | GB/s (high is better) | 155 [153-155] n=5 |  |
| h2d pageable 1 MiB | GB/s (high is better) | 7.44 [7.26-7.45] n=5 |  |
| d2h pageable 1 MiB | GB/s (high is better) | 6.45 [6.02-6.53] n=5 |  |
| h2d pinned 16 MiB | GB/s (high is better) | 13.1 [13.0-13.1] n=5 |  |
| d2h pinned 16 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 |  |
| d2d device 16 MiB | GB/s (high is better) | 180 [179-182] n=5 |  |
| h2d pageable 16 MiB | GB/s (high is better) | 11.2 [10.7-12.2] n=5 |  |
| d2h pageable 16 MiB | GB/s (high is better) | 9.04 [8.94-9.23] n=5 |  |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 |  |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [13.2-13.2] n=5 |  |
| d2d device 256 MiB | GB/s (high is better) | 185 [185-187] n=5 |  |
| h2d pageable 256 MiB | GB/s (high is better) | 9.63 [9.59-9.67] n=5 |  |
| d2h pageable 256 MiB | GB/s (high is better) | 9.23 [9.03-9.33] n=5 |  |

### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | native |  |
|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 7062 [6950-7071] n=5 |  |
| N=2048 | GFLOP/s (high is better) | 7489 [7377-7597] n=5 |  |
| N=4096 | GFLOP/s (high is better) | 6655 [6609-6720] n=5 |  |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | native |  |
|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 |  |
| process wall (incl. torch import) | s (low is better) | 4.09 [4.00-4.17] n=5 |  |
| stage4 (50 training episodes) | s (low is better) | 1.20 [1.10-1.20] n=5 |  |
| H2D+D2H transfers per second | 1/s (high is better) | 2494 [2333-2602] n=5 |  |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | native |  |
|---|---|---|---|
| cuInit | ms (low is better) | 41.3 [31.9-41.6] n=5 |  |
| cuCtxCreate | ms (low is better) | 141 [138-155] n=5 |  |
| cuCtxDestroy | ms (low is better) | 82.5 [75.8-93.0] n=5 |  |

### (5) nvprobe 0 process, bash time

| value | unit | native |  |
|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 89.0 [68.0-109] n=5 |  |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 44.0 [41.0-49.0] n=5 |  |

### (5) nvprobe 1 process, bash time

| value | unit | native |  |
|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 279 [262-297] n=5 |  |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 241 [236-260] n=5 |  |

### (6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR

| value | unit | native |  |
|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 2.68 [2.58-2.80] n=5 |  |
| CHECK_VERSION_STR p99 | us (low is better) | 3.69 [2.89-4.45] n=5 |  |
| CHECK_VERSION_STR mean | us (low is better) | 2.81 [2.62-2.93] n=5 |  |

### (6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size

| value | unit | native |  |
|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 4.44 [4.31-4.46] n=5 |  |
| RM_CONTROL 232 B p99 | us (low is better) | 5.79 [5.24-7.31] n=5 |  |
| RM_CONTROL 1576 B p50 | us (low is better) | 4.50 [4.31-5.07] n=5 |  |
| RM_CONTROL 1576 B p99 | us (low is better) | 5.57 [5.38-8.03] n=5 |  |
| RM_CONTROL 12328 B p50 | us (low is better) | 4.48 [4.29-4.52] n=5 |  |
| RM_CONTROL 12328 B p99 | us (low is better) | 5.50 [5.38-6.00] n=5 |  |
| RM_CONTROL 49192 B p50 | us (low is better) | 4.50 [4.30-4.54] n=5 |  |
| RM_CONTROL 49192 B p99 | us (low is better) | 5.65 [5.41-6.31] n=5 |  |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | native |  |
|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 35.3 [33.0-36.6] n=5 |  |
| 64x1 MiB cuMemFree | us (low is better) | 26.8 [24.9-27.5] n=5 |  |
| 64x1 MiB cycle | ms (low is better) | 3.82 [3.76-3.84] n=5 |  |
| 8x64 MiB cuMemAlloc | us (low is better) | 73.0 [72.2-73.7] n=5 |  |
| 8x64 MiB cuMemFree | us (low is better) | 89.0 [87.4-92.7] n=5 |  |
| 8x64 MiB cycle | ms (low is better) | 1.39 [1.36-1.44] n=5 |  |


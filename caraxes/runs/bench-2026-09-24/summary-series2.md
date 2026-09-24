Sources: v2=v2, native=native
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 2/26, native 25/65
Host 1-min load average at run start, median [min-max]: v2 4.48 [2.73-7.99] n=26, native 5.02 [2.71-17.6] n=65

### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 18.3 [18.1-18.4] n=2 | 16.4 [16.4-16.5] n=5 | 1.11 |

### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 59.9 [18.7-101] n=2 | 16.6 [16.3-16.8] n=5 | 3.61 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.305 [0.208-0.402] n=2 | 0.007 [0.006-0.007] n=5 | 43.57 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 29.9 [0.322-59.5] n=2 | 0.021 [0.014-0.031] n=5 | 1423.62 |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 2.82 [2.22-3.43] n=2 | 12.1 [12.1-12.1] n=5 | 0.23 |
| d2h pinned 1 MiB | GB/s (high is better) | 2.72 [1.88-3.57] n=2 | 12.3 [12.2-12.3] n=5 | 0.22 |
| d2d device 1 MiB | GB/s (high is better) | 5.51 [3.65-7.37] n=2 | 136 [126-149] n=5 | 0.04 |
| h2d pageable 1 MiB | GB/s (high is better) | 2.35 [1.65-3.05] n=2 | 7.29 [6.61-7.32] n=5 | 0.32 |
| d2h pageable 1 MiB | GB/s (high is better) | 2.26 [1.92-2.60] n=2 | 6.04 [4.66-6.48] n=5 | 0.37 |
| h2d pinned 16 MiB | GB/s (high is better) | 3.13 [3.09-3.17] n=2 | 13.0 [13.0-13.1] n=5 | 0.24 |
| d2h pinned 16 MiB | GB/s (high is better) | 3.15 [3.15-3.15] n=2 | 13.1 [13.1-13.1] n=5 | 0.24 |
| d2d device 16 MiB | GB/s (high is better) | 30.6 [4.09-57.2] n=2 | 181 [180-184] n=5 | 0.17 |
| h2d pageable 16 MiB | GB/s (high is better) | 3.93 [3.78-4.07] n=2 | 8.68 [4.63-11.0] n=5 | 0.45 |
| d2h pageable 16 MiB | GB/s (high is better) | 4.08 [3.82-4.33] n=2 | 8.76 [5.39-8.83] n=5 | 0.47 |
| h2d pinned 256 MiB | GB/s (high is better) | 12.9 [12.9-13.0] n=2 | 13.1 [13.0-13.1] n=5 | 0.99 |
| d2h pinned 256 MiB | GB/s (high is better) | 13.0 [13.0-13.1] n=2 | 13.2 [13.1-13.2] n=5 | 0.99 |
| d2d device 256 MiB | GB/s (high is better) | 172 [169-175] n=2 | 188 [187-190] n=5 | 0.91 |
| h2d pageable 256 MiB | GB/s (high is better) | 3.59 [3.14-4.04] n=2 | 9.10 [5.99-9.36] n=5 | 0.39 |
| d2h pageable 256 MiB | GB/s (high is better) | 3.79 [3.49-4.10] n=2 | 8.45 [5.43-9.24] n=5 | 0.45 |

### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 301 [250-352] n=2 | 5054 [4786-5430] n=5 | 0.06 |
| N=2048 | GFLOP/s (high is better) | 664 [648-681] n=2 | 5740 [5298-6318] n=5 | 0.12 |
| N=4096 | GFLOP/s (high is better) | 4850 [4650-5050] n=2 | 6200 [5862-6777] n=5 | 0.78 |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=2 | 56.9 [56.9-56.9] n=5 | 1.00 |
| process wall (incl. torch import) | s (low is better) | 65.6 [39.4-91.8] n=2 | 5.36 [4.62-6.69] n=5 | 12.25 |
| stage4 (50 training episodes) | s (low is better) | 20.2 [17.8-22.7] n=2 | 1.40 [1.20-1.70] n=5 | 14.46 |
| H2D+D2H transfers per second | 1/s (high is better) | 144 [127-161] n=2 | 2072 [1668-2484] n=5 | 0.07 |

### (5) Module load to first successful cuInit (in the guest)

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| module load | ms (low is better) | 3376 [3248-4871] n=5 | n/a | - |
| load to first cuInit | ms (low is better) | 3591 [3414-5077] n=5 | n/a | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| cuInit | ms (low is better) | 160 [147-172] n=2 | 95.8 [74.8-110] n=5 | 1.67 |
| cuCtxCreate | ms (low is better) | 35020 [5418-64621] n=2 | 190 [174-190] n=5 | 184.45 |
| cuCtxDestroy | ms (low is better) | 3471 [3212-3730] n=2 | 91.8 [85.1-97.6] n=5 | 37.80 |

### (5) nvprobe 0 process, bash time

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 205 [187-223] n=2 | 201 [165-216] n=5 | 1.02 |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 204 [186-223] n=2 | 124 [117-153] n=5 | 1.65 |

### (5) nvprobe 1 process, bash time

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 38273 [8625-67921] n=2 | 426 [391-615] n=5 | 89.84 |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 38265 [8628-67902] n=2 | 350 [338-499] n=5 | 109.33 |

### (6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 1.69 [1.33-2.04] n=2 | 2.58 [2.58-2.72] n=5 | 0.65 |
| CHECK_VERSION_STR p99 | us (low is better) | 2.89 [2.53-3.25] n=2 | 3.72 [3.28-5.20] n=5 | 0.78 |
| CHECK_VERSION_STR mean | us (low is better) | 1.73 [1.50-1.95] n=2 | 2.81 [2.65-2.84] n=5 | 0.61 |

### (6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 2.58 [2.34-2.82] n=2 | 4.47 [4.34-5.41] n=5 | 0.58 |
| RM_CONTROL 232 B p99 | us (low is better) | 4.69 [4.10-5.28] n=2 | 6.87 [5.70-7.15] n=5 | 0.68 |
| RM_CONTROL 1576 B p50 | us (low is better) | 2.62 [2.41-2.84] n=2 | 4.43 [4.16-5.43] n=5 | 0.59 |
| RM_CONTROL 1576 B p99 | us (low is better) | 4.21 [3.50-4.91] n=2 | 5.94 [5.05-6.44] n=5 | 0.71 |
| RM_CONTROL 12328 B p50 | us (low is better) | 2.63 [2.45-2.82] n=2 | 4.48 [4.17-4.50] n=5 | 0.59 |
| RM_CONTROL 12328 B p99 | us (low is better) | 5.51 [3.38-7.64] n=2 | 5.40 [5.29-6.38] n=5 | 1.02 |
| RM_CONTROL 49192 B p50 | us (low is better) | 2.69 [2.56-2.82] n=2 | 4.49 [4.23-5.30] n=5 | 0.60 |
| RM_CONTROL 49192 B p99 | us (low is better) | 4.18 [3.81-4.55] n=2 | 5.70 [5.48-6.46] n=5 | 0.73 |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 20475 [20454-20497] n=2 | 41.2 [35.2-63.8] n=5 | 496.98 |
| 64x1 MiB cuMemFree | us (low is better) | 20941 [20504-21378] n=2 | 29.3 [25.1-46.7] n=5 | 714.71 |
| 64x1 MiB cycle | ms (low is better) | 2984 [2902-3066] n=2 | 4.08 [3.86-5.54] n=5 | 731.85 |
| 8x64 MiB cuMemAlloc | us (low is better) | 56443 [53329-59557] n=2 | 108 [100-142] n=5 | 520.69 |
| 8x64 MiB cuMemFree | us (low is better) | 50861 [48301-53421] n=2 | 101 [94.6-123] n=5 | 505.08 |
| 8x64 MiB cycle | ms (low is better) | 906 [867-946] n=2 | 11.9 [10.1-12.7] n=5 | 75.98 |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| churn device CPU | ms (low is better) | 90870 [80310-101430] n=2 | n/a | - |
| churn VMM CPU | ms (low is better) | 31715 [11770-51660] n=2 | n/a | - |
| copy device CPU | ms (low is better) | 36925 [24850-49000] n=2 | n/a | - |
| copy VMM CPU | ms (low is better) | 49980 [42620-57340] n=2 | n/a | - |
| ctrl device CPU | ms (low is better) | 5.00 [0.000-10.0] n=2 | n/a | - |
| ctrl VMM CPU | ms (low is better) | 200 [180-220] n=2 | n/a | - |
| init device CPU | ms (low is better) | 18420 [6520-30320] n=2 | n/a | - |
| init VMM CPU | ms (low is better) | 25380 [4070-46690] n=2 | n/a | - |
| ioctl device CPU | ms (low is better) | 5.00 [0.000-10.0] n=2 | n/a | - |
| ioctl VMM CPU | ms (low is better) | 310 [200-420] n=2 | n/a | - |
| launch-auto device CPU | ms (low is better) | 17335 [6110-28560] n=2 | n/a | - |
| launch-auto VMM CPU | ms (low is better) | 25055 [8530-41580] n=2 | n/a | - |
| launch-blocking device CPU | ms (low is better) | 17950 [6190-29710] n=2 | n/a | - |
| launch-blocking VMM CPU | ms (low is better) | 27625 [11470-43780] n=2 | n/a | - |
| mm device CPU | ms (low is better) | 19510 [7760-31260] n=2 | n/a | - |
| mm VMM CPU | ms (low is better) | 39660 [26830-52490] n=2 | n/a | - |
| rl device CPU | ms (low is better) | 27460 [16270-38650] n=2 | n/a | - |
| rl VMM CPU | ms (low is better) | 58940 [55400-62480] n=2 | n/a | - |
| short-auto device CPU | ms (low is better) | 18215 [6190-30240] n=2 | n/a | - |
| short-auto VMM CPU | ms (low is better) | 28975 [11710-46240] n=2 | n/a | - |
| short-blocking device CPU | ms (low is better) | 18450 [6370-30530] n=2 | n/a | - |
| short-blocking VMM CPU | ms (low is better) | 29135 [11090-47180] n=2 | n/a | - |
| time0 device CPU | ms (low is better) | 110 [100-120] n=2 | n/a | - |
| time0 VMM CPU | ms (low is better) | 400 [320-480] n=2 | n/a | - |
| time1 device CPU | ms (low is better) | 18270 [6070-30470] n=2 | n/a | - |
| time1 VMM CPU | ms (low is better) | 25070 [3350-46790] n=2 | n/a | - |

### (8) Device process memory (over all runs)

| value | unit | v2 | native | v2/native |
|---|---|---|---|---|
| device HWM | MiB (low is better) | 4109 [4108-4122] n=26 | n/a | - |
| device RSS after run | MiB (low is better) | 4108 [1515-4120] n=26 | n/a | - |


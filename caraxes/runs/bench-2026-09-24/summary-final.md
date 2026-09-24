Sources: v1=final/v1, v2=final/v2, native=final/native
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v1 0/65, v2 0/65, native 7/65
Host 1-min load average at run start, median [min-max]: v1 3.05 [1.74-4.39] n=65, v2 3.05 [1.91-4.39] n=65, native 3.05 [1.74-4.39] n=65

### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 16.0 [15.9-16.1] n=5 | 17.6 [17.3-18.5] n=5 | 16.2 [16.2-16.5] n=5 | 0.99 | 1.08 |

### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 100 [100-100] n=5 | 18.7 [18.5-101] n=5 | 16.4 [16.3-16.6] n=5 | 6.12 | 1.14 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.007 [0.007-0.008] n=5 | 0.257 [0.155-0.585] n=5 | 0.006 [0.006-0.010] n=5 | 1.17 | 42.83 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 8.50 [0.036-99.6] n=5 | 0.251 [0.223-12.1] n=5 | 0.018 [0.014-0.019] n=5 | 472.22 | 13.94 |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 12.2 [12.2-12.2] n=5 | 3.33 [2.15-3.58] n=5 | 12.1 [12.0-12.1] n=5 | 1.01 | 0.28 |
| d2h pinned 1 MiB | GB/s (high is better) | 12.4 [12.3-12.4] n=5 | 3.38 [1.88-3.55] n=5 | 12.3 [12.3-12.3] n=5 | 1.01 | 0.27 |
| d2d device 1 MiB | GB/s (high is better) | 145 [137-148] n=5 | 6.60 [5.85-7.40] n=5 | 135 [132-137] n=5 | 1.07 | 0.05 |
| h2d pageable 1 MiB | GB/s (high is better) | 7.35 [7.27-7.43] n=5 | 2.84 [1.86-2.97] n=5 | 7.39 [7.34-7.41] n=5 | 0.99 | 0.38 |
| d2h pageable 1 MiB | GB/s (high is better) | 6.50 [5.81-6.55] n=5 | 1.97 [1.72-2.85] n=5 | 6.44 [6.30-6.48] n=5 | 1.01 | 0.31 |
| h2d pinned 16 MiB | GB/s (high is better) | 13.0 [13.0-13.0] n=5 | 3.13 [3.08-3.17] n=5 | 13.0 [13.0-13.1] n=5 | 1.00 | 0.24 |
| d2h pinned 16 MiB | GB/s (high is better) | 13.1 [13.0-13.1] n=5 | 3.16 [3.15-6.35] n=5 | 13.1 [13.1-13.1] n=5 | 1.00 | 0.24 |
| d2d device 16 MiB | GB/s (high is better) | 177 [165-182] n=5 | 79.0 [4.42-82.3] n=5 | 183 [182-183] n=5 | 0.97 | 0.43 |
| h2d pageable 16 MiB | GB/s (high is better) | 8.62 [8.28-10.5] n=5 | 4.23 [3.37-4.41] n=5 | 10.6 [9.46-11.0] n=5 | 0.81 | 0.40 |
| d2h pageable 16 MiB | GB/s (high is better) | 8.88 [7.38-8.99] n=5 | 4.10 [3.52-4.36] n=5 | 9.13 [8.81-9.19] n=5 | 0.97 | 0.45 |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 | 13.0 [12.9-13.0] n=5 | 13.1 [13.1-13.1] n=5 | 1.00 | 0.99 |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [13.2-13.2] n=5 | 13.1 [13.0-13.1] n=5 | 13.2 [13.2-13.2] n=5 | 1.00 | 0.99 |
| d2d device 256 MiB | GB/s (high is better) | 186 [185-188] n=5 | 177 [171-180] n=5 | 186 [183-188] n=5 | 1.00 | 0.95 |
| h2d pageable 256 MiB | GB/s (high is better) | 8.92 [8.18-9.34] n=5 | 4.12 [3.27-4.33] n=5 | 9.22 [8.84-9.45] n=5 | 0.97 | 0.45 |
| d2h pageable 256 MiB | GB/s (high is better) | 8.82 [8.42-9.07] n=5 | 4.00 [3.16-4.16] n=5 | 9.12 [8.75-9.30] n=5 | 0.97 | 0.44 |

### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 5218 [3803-5267] n=5 | 1351 [1326-1386] n=5 | 5041 [4775-5333] n=5 | 1.04 | 0.27 |
| N=2048 | GFLOP/s (high is better) | 5598 [5316-5617] n=5 | 774 [678-840] n=5 | 5570 [5301-5911] n=5 | 1.01 | 0.14 |
| N=4096 | GFLOP/s (high is better) | 6015 [5899-6512] n=5 | 3287 [3117-4940] n=5 | 6512 [5962-6708] n=5 | 0.92 | 0.50 |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | 1.00 | 1.00 |
| process wall (incl. torch import) | s (low is better) | 88.1 [53.4-107] n=5 | 33.2 [32.8-33.3] n=5 | 4.43 [4.34-4.61] n=5 | 19.89 | 7.49 |
| stage4 (50 training episodes) | s (low is better) | 1.50 [1.40-1.80] n=5 | 18.6 [18.3-18.9] n=5 | 1.20 [1.10-1.30] n=5 | 1.25 | 15.50 |
| H2D+D2H transfers per second | 1/s (high is better) | 1963 [1572-1991] n=5 | 154 [152-157] n=5 | 2417 [2237-2523] n=5 | 0.81 | 0.06 |

### (5) Module load to first successful cuInit (in the guest)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| module load | ms (low is better) | 27.0 [25.0-29.0] n=5 | 3236 [3070-3809] n=5 | n/a | - | - |
| load to first cuInit | ms (low is better) | 192 [131-240] n=5 | 3401 [3265-4056] n=5 | n/a | - | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| cuInit | ms (low is better) | 31.1 [27.5-35.3] n=5 | 149 [140-182] n=5 | 77.4 [71.6-116] n=5 | 0.40 | 1.92 |
| cuCtxCreate | ms (low is better) | 159 [149-164] n=5 | 4727 [4555-4975] n=5 | 188 [183-197] n=5 | 0.84 | 25.14 |
| cuCtxDestroy | ms (low is better) | 87.7 [73.9-90.5] n=5 | 3390 [3356-3450] n=5 | 89.0 [83.7-93.7] n=5 | 0.99 | 38.07 |

### (5) nvprobe 0 process, bash time

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 165 [151-203] n=5 | 166 [161-175] n=5 | 198 [128-209] n=5 | 0.83 | 0.84 |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 5.00 [5.00-6.00] n=5 | 166 [161-174] n=5 | 127 [112-138] n=5 | 0.04 | 1.31 |

### (5) nvprobe 1 process, bash time

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 330 [310-347] n=5 | 8303 [7872-8452] n=5 | 397 [377-405] n=5 | 0.83 | 20.91 |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 38.0 [37.0-42.0] n=5 | 8300 [7867-8478] n=5 | 336 [333-362] n=5 | 0.11 | 24.70 |

### (6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 16.0 [13.6-18.8] n=5 | 1.37 [1.33-1.98] n=5 | 2.68 [2.47-2.95] n=5 | 5.97 | 0.51 |
| CHECK_VERSION_STR p99 | us (low is better) | 37.6 [29.1-70.9] n=5 | 3.13 [3.07-6.97] n=5 | 3.57 [2.99-4.11] n=5 | 10.52 | 0.88 |
| CHECK_VERSION_STR mean | us (low is better) | 17.4 [16.1-22.4] n=5 | 1.53 [1.47-2.12] n=5 | 2.79 [2.51-3.14] n=5 | 6.23 | 0.55 |

### (6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 21.6 [18.5-21.9] n=5 | 2.39 [2.35-2.45] n=5 | 4.41 [4.32-5.52] n=5 | 4.89 | 0.54 |
| RM_CONTROL 232 B p99 | us (low is better) | 53.1 [36.9-127] n=5 | 3.49 [3.24-7.21] n=5 | 5.63 [5.44-7.02] n=5 | 9.44 | 0.62 |
| RM_CONTROL 1576 B p50 | us (low is better) | 20.2 [16.9-24.2] n=5 | 2.36 [2.36-2.47] n=5 | 4.32 [4.30-4.49] n=5 | 4.68 | 0.55 |
| RM_CONTROL 1576 B p99 | us (low is better) | 48.6 [27.2-257] n=5 | 3.73 [3.19-6.75] n=5 | 5.51 [5.42-7.04] n=5 | 8.81 | 0.68 |
| RM_CONTROL 12328 B p50 | us (low is better) | 25.9 [24.1-28.9] n=5 | 2.42 [2.35-2.49] n=5 | 4.31 [4.30-4.48] n=5 | 6.01 | 0.56 |
| RM_CONTROL 12328 B p99 | us (low is better) | 60.4 [38.2-79.4] n=5 | 3.54 [3.22-7.10] n=5 | 5.52 [5.37-5.66] n=5 | 10.94 | 0.64 |
| RM_CONTROL 49192 B p50 | us (low is better) | 38.8 [36.9-46.8] n=5 | 2.50 [2.36-2.54] n=5 | 4.30 [4.30-4.35] n=5 | 9.02 | 0.58 |
| RM_CONTROL 49192 B p99 | us (low is better) | 107 [71.8-162] n=5 | 3.66 [3.26-6.55] n=5 | 5.40 [5.30-6.58] n=5 | 19.78 | 0.68 |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 86.6 [69.6-103] n=5 | 20569 [18550-20940] n=5 | 37.7 [32.9-62.2] n=5 | 2.30 | 545.60 |
| 64x1 MiB cuMemFree | us (low is better) | 52.8 [46.3-71.3] n=5 | 20902 [20518-21783] n=5 | 29.2 [26.7-45.8] n=5 | 1.81 | 715.84 |
| 64x1 MiB cycle | ms (low is better) | 7.90 [7.28-10.2] n=5 | 2928 [2901-2940] n=5 | 3.89 [3.79-3.95] n=5 | 2.03 | 752.76 |
| 8x64 MiB cuMemAlloc | us (low is better) | 182 [177-192] n=5 | 53381 [51364-53846] n=5 | 93.8 [87.8-98.1] n=5 | 1.94 | 569.10 |
| 8x64 MiB cuMemFree | us (low is better) | 127 [121-131] n=5 | 48999 [48309-49960] n=5 | 96.4 [93.6-96.5] n=5 | 1.32 | 508.29 |
| 8x64 MiB cycle | ms (low is better) | 9.23 [9.12-9.76] n=5 | 860 [858-866] n=5 | 12.7 [11.9-12.9] n=5 | 0.73 | 67.64 |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| churn device CPU | ms (low is better) | 370 [370-390] n=5 | 77550 [76970-77720] n=5 | n/a | - | - |
| churn VMM CPU | ms (low is better) | 480 [400-650] n=5 | 9800 [8390-10080] n=5 | n/a | - | - |
| copy device CPU | ms (low is better) | 370 [280-410] n=5 | 23870 [22670-24560] n=5 | n/a | - | - |
| copy VMM CPU | ms (low is better) | 36030 [13020-69550] n=5 | 22870 [13750-43930] n=5 | n/a | - | - |
| ctrl device CPU | ms (low is better) | 130 [120-130] n=5 | 10.0 [0.000-10.0] n=5 | n/a | - | - |
| ctrl VMM CPU | ms (low is better) | 540 [360-620] n=5 | 200 [180-210] n=5 | n/a | - | - |
| init device CPU | ms (low is better) | 220 [200-240] n=5 | 5990 [5880-6120] n=5 | n/a | - | - |
| init VMM CPU | ms (low is better) | 780 [750-790] n=5 | 3110 [3040-3270] n=5 | n/a | - | - |
| ioctl device CPU | ms (low is better) | 100 [90.0-110] n=5 | 10.0 [0.000-10.0] n=5 | n/a | - | - |
| ioctl VMM CPU | ms (low is better) | 300 [250-500] n=5 | 180 [170-190] n=5 | n/a | - | - |
| launch-auto device CPU | ms (low is better) | 200 [200-210] n=5 | 5760 [5510-6010] n=5 | n/a | - | - |
| launch-auto VMM CPU | ms (low is better) | 2520 [2420-4280] n=5 | 8250 [4670-13540] n=5 | n/a | - | - |
| launch-blocking device CPU | ms (low is better) | 230 [230-240] n=5 | 5930 [5910-5990] n=5 | n/a | - | - |
| launch-blocking VMM CPU | ms (low is better) | 670 [590-9770] n=5 | 12000 [3480-13750] n=5 | n/a | - | - |
| mm device CPU | ms (low is better) | 380 [370-480] n=5 | 7280 [7010-7670] n=5 | n/a | - | - |
| mm VMM CPU | ms (low is better) | 54070 [35380-68860] n=5 | 9990 [9500-23990] n=5 | n/a | - | - |
| rl device CPU | ms (low is better) | 370 [290-450] n=5 | 15300 [14780-15480] n=5 | n/a | - | - |
| rl VMM CPU | ms (low is better) | 86390 [52250-106220] n=5 | 23500 [21790-24040] n=5 | n/a | - | - |
| short-auto device CPU | ms (low is better) | 230 [200-230] n=5 | 6330 [6050-6470] n=5 | n/a | - | - |
| short-auto VMM CPU | ms (low is better) | 590 [560-740] n=5 | 10130 [2950-10780] n=5 | n/a | - | - |
| short-blocking device CPU | ms (low is better) | 220 [210-260] n=5 | 6280 [5840-6560] n=5 | n/a | - | - |
| short-blocking VMM CPU | ms (low is better) | 570 [290-1480] n=5 | 12420 [3020-12880] n=5 | n/a | - | - |
| time0 device CPU | ms (low is better) | 120 [90.0-130] n=5 | 90.0 [90.0-110] n=5 | n/a | - | - |
| time0 VMM CPU | ms (low is better) | 140 [130-150] n=5 | 250 [230-270] n=5 | n/a | - | - |
| time1 device CPU | ms (low is better) | 220 [210-230] n=5 | 6080 [5750-6160] n=5 | n/a | - | - |
| time1 VMM CPU | ms (low is better) | 230 [200-240] n=5 | 3120 [2800-3450] n=5 | n/a | - | - |

### (8) Device process memory (over all runs)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 425 [59.6-538] n=65 | 4123 [4108-4123] n=65 | n/a | - | - |
| device RSS after run | MiB (low is better) | 384 [14.3-493] n=65 | 4121 [4108-4121] n=65 | n/a | - | - |


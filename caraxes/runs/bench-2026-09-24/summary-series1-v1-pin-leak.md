Sources: v1=series1/v1, native=series1/native
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v1 41/65, native 42/65
Host 1-min load average at run start, median [min-max]: v1 21.2 [6.92-27.6] n=65, native 21.2 [6.92-27.3] n=65

### churn

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 173 [71.8-181] n=3 | 47.3 [36.0-59.2] n=5 | 3.66 |
| 64x1 MiB cuMemFree | us (low is better) | 112 [71.9-125] n=3 | 43.5 [29.4-46.8] n=5 | 2.58 |
| 64x1 MiB cycle | ms (low is better) | 26.0 [15.6-28.7] n=3 | 5.52 [3.98-25.5] n=5 | 4.71 |
| 8x64 MiB cuMemAlloc | us (low is better) | 371 [237-417] n=3 | 147 [110-538] n=5 | 2.52 |
| 8x64 MiB cuMemFree | us (low is better) | 265 [173-266] n=3 | 133 [99.5-485] n=5 | 2.00 |
| 8x64 MiB cycle | ms (low is better) | 11.0 [8.93-11.7] n=3 | 10.6 [9.60-11.6] n=5 | 1.04 |

### cpu

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| churn device CPU | ms (low is better) | 520 [180-560] n=5 | n/a | - |
| churn VMM CPU | ms (low is better) | 530 [200-620] n=5 | n/a | - |
| copy device CPU | ms (low is better) | 270 [180-390] n=5 | n/a | - |
| copy VMM CPU | ms (low is better) | 680 [240-3050] n=5 | n/a | - |
| ctrl device CPU | ms (low is better) | 130 [110-220] n=5 | n/a | - |
| ctrl VMM CPU | ms (low is better) | 380 [340-630] n=5 | n/a | - |
| init device CPU | ms (low is better) | 280 [190-390] n=5 | n/a | - |
| init VMM CPU | ms (low is better) | 300 [250-890] n=5 | n/a | - |
| ioctl device CPU | ms (low is better) | 110 [90.0-240] n=5 | n/a | - |
| ioctl VMM CPU | ms (low is better) | 500 [310-630] n=5 | n/a | - |
| launch-auto device CPU | ms (low is better) | 250 [180-280] n=5 | n/a | - |
| launch-auto VMM CPU | ms (low is better) | 1160 [230-2170] n=5 | n/a | - |
| launch-blocking device CPU | ms (low is better) | 260 [220-320] n=5 | n/a | - |
| launch-blocking VMM CPU | ms (low is better) | 420 [250-550] n=5 | n/a | - |
| mm device CPU | ms (low is better) | 330 [270-370] n=5 | n/a | - |
| mm VMM CPU | ms (low is better) | 2700 [2000-3260] n=5 | n/a | - |
| rl device CPU | ms (low is better) | 370 [300-400] n=5 | n/a | - |
| rl VMM CPU | ms (low is better) | 7310 [2560-14000] n=5 | n/a | - |
| short-auto device CPU | ms (low is better) | 260 [190-280] n=5 | n/a | - |
| short-auto VMM CPU | ms (low is better) | 400 [240-640] n=5 | n/a | - |
| short-blocking device CPU | ms (low is better) | 270 [180-510] n=5 | n/a | - |
| short-blocking VMM CPU | ms (low is better) | 330 [230-520] n=5 | n/a | - |
| time0 device CPU | ms (low is better) | 70.0 [40.0-120] n=5 | n/a | - |
| time0 VMM CPU | ms (low is better) | 170 [140-230] n=5 | n/a | - |
| time1 device CPU | ms (low is better) | 260 [190-270] n=5 | n/a | - |
| time1 VMM CPU | ms (low is better) | 310 [200-360] n=5 | n/a | - |

### rss

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| device HWM | MiB (low is better) | 983 [421-1061] n=65 | n/a | - |
| device RSS after run | MiB (low is better) | 941 [375-1005] n=65 | n/a | - |

### copy

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 12.2 [12.1-12.2] n=3 | 12.1 [11.7-12.1] n=5 | 1.01 |
| d2h pinned 1 MiB | GB/s (high is better) | 12.4 [12.3-12.4] n=3 | 12.2 [12.0-12.3] n=5 | 1.01 |
| d2d device 1 MiB | GB/s (high is better) | 134 [110-146] n=3 | 122 [116-146] n=5 | 1.10 |
| h2d pageable 1 MiB | GB/s (high is better) | 7.01 [6.62-7.19] n=3 | 6.96 [6.75-7.26] n=5 | 1.01 |
| d2h pageable 1 MiB | GB/s (high is better) | 5.32 [4.45-6.34] n=3 | 5.14 [4.51-6.12] n=5 | 1.04 |
| h2d pinned 16 MiB | GB/s (high is better) | 13.0 [12.7-13.0] n=3 | 13.0 [5.03-13.1] n=5 | 1.00 |
| d2h pinned 16 MiB | GB/s (high is better) | 13.0 [12.9-13.1] n=3 | 13.1 [12.8-13.1] n=5 | 0.99 |
| d2d device 16 MiB | GB/s (high is better) | 175 [175-179] n=3 | 179 [177-183] n=5 | 0.98 |
| h2d pageable 16 MiB | GB/s (high is better) | 5.47 [3.21-8.38] n=3 | 7.89 [6.24-9.16] n=5 | 0.69 |
| d2h pageable 16 MiB | GB/s (high is better) | 6.02 [2.64-8.37] n=3 | 6.33 [5.46-7.87] n=5 | 0.95 |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=1 | 13.1 [13.1-13.1] n=5 | 1.00 |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [13.2-13.2] n=1 | 13.2 [12.5-13.2] n=5 | 1.00 |
| d2d device 256 MiB | GB/s (high is better) | 182 [182-182] n=1 | 188 [180-189] n=5 | 0.97 |
| h2d pageable 256 MiB | GB/s (high is better) | 7.81 [7.81-7.81] n=1 | 7.19 [5.47-9.10] n=5 | 1.09 |
| d2h pageable 256 MiB | GB/s (high is better) | 7.83 [7.83-7.83] n=1 | 7.31 [3.18-8.32] n=5 | 1.07 |

### ctrl

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 20.2 [19.7-31.8] n=5 | 4.43 [4.39-5.56] n=5 | 4.57 |
| RM_CONTROL 232 B p99 | us (low is better) | 49.2 [32.0-240] n=5 | 6.72 [5.72-7.08] n=5 | 7.33 |
| RM_CONTROL 1576 B p50 | us (low is better) | 21.6 [17.2-40.5] n=5 | 4.51 [4.42-5.65] n=5 | 4.78 |
| RM_CONTROL 1576 B p99 | us (low is better) | 75.1 [35.0-1529] n=5 | 6.63 [5.14-13.6] n=5 | 11.33 |
| RM_CONTROL 12328 B p50 | us (low is better) | 26.1 [20.4-62.7] n=5 | 4.47 [4.45-5.67] n=5 | 5.84 |
| RM_CONTROL 12328 B p99 | us (low is better) | 63.8 [36.8-1019] n=5 | 6.05 [5.63-13.6] n=5 | 10.55 |
| RM_CONTROL 49192 B p50 | us (low is better) | 37.0 [32.7-50.7] n=5 | 4.47 [4.40-5.61] n=5 | 8.27 |
| RM_CONTROL 49192 B p99 | us (low is better) | 109 [66.4-300] n=5 | 6.10 [5.51-7.78] n=5 | 17.80 |

### init

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| cuInit | ms (low is better) | 98.7 [98.7-178] n=3 | 37.4 [32.9-87.2] n=5 | 2.64 |
| cuCtxCreate | ms (low is better) | 486 [269-490] n=3 | 185 [166-206] n=5 | 2.62 |
| cuCtxDestroy | ms (low is better) | 209 [125-444] n=3 | 109 [103-135] n=5 | 1.92 |

### ioctl

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 16.9 [15.5-51.5] n=5 | 3.17 [2.58-3.42] n=5 | 5.35 |
| CHECK_VERSION_STR p99 | us (low is better) | 62.5 [34.5-1865] n=5 | 4.85 [3.21-10.2] n=5 | 12.90 |
| CHECK_VERSION_STR mean | us (low is better) | 22.5 [18.2-110] n=5 | 3.29 [2.62-12.8] n=5 | 6.83 |

### launch-auto

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 17.0 [16.3-17.3] n=4 | 16.2 [16.1-16.4] n=5 | 1.05 |

### launch-blocking

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 99.3 [98.7-100] n=4 | 16.2 [16.1-16.3] n=5 | 6.11 |

### mm

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 5140 [4834-5160] n=3 | 4534 [3448-5276] n=5 | 1.13 |
| N=2048 | GFLOP/s (high is better) | 5400 [4737-5532] n=3 | 5472 [5316-5595] n=5 | 0.99 |
| N=4096 | GFLOP/s (high is better) | 5990 [5922-6300] n=3 | 6140 [6026-6508] n=5 | 0.98 |

### rl

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=3 | 56.9 [56.9-56.9] n=5 | 1.00 |
| process wall (incl. torch import) | s (low is better) | 15.0 [12.1-21.1] n=3 | 8.69 [5.05-12.8] n=5 | 1.72 |
| stage4 (50 training episodes) | s (low is better) | 3.40 [2.60-4.40] n=3 | 2.30 [1.20-2.80] n=5 | 1.48 |
| H2D+D2H transfers per second | 1/s (high is better) | 833 [651-1118] n=3 | 1252 [1042-2376] n=5 | 0.67 |

### short-auto

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.009 [0.008-0.040] n=4 | 0.007 [0.007-0.032] n=5 | 1.29 |

### short-blocking

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 2.08 [0.027-20.3] n=3 | 0.026 [0.015-0.035] n=5 | 79.88 |

### time0

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 168 [90.0-275] n=5 | 99.0 [79.0-159] n=5 | 1.70 |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 17.0 [4.00-21.0] n=5 | 56.0 [45.0-62.0] n=5 | 0.30 |

### time1

| value | unit | v1 | native | v1/native |
|---|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 587 [292-789] n=5 | 432 [286-562] n=5 | 1.36 |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 113 [27.0-227] n=5 | 364 [248-373] n=5 | 0.31 |


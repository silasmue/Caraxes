Sources: v2=final-always-resync-all/v2, native=final-always-resync-all/native
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 0/25, native 0/25
Host 1-min load average at run start, median [min-max]: v2 2.79 [1.55-3.95] n=25, native 2.77 [1.55-4.04] n=25

### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 44.0 [43.4-46.8] n=4 | 16.4 [16.3-16.6] n=4 | 2.68 |

### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 88.4 [82.0-102] n=4 | 16.3 [16.3-16.4] n=4 | 5.42 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 44.1 [43.1-44.5] n=4 | 0.006 [0.006-0.006] n=4 | 7342.33 |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 122 [110-128] n=4 | 0.019 [0.015-0.024] n=4 | 6437.74 |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 2.20 [2.13-2.25] n=4 | 12.1 [12.1-12.1] n=4 | 0.18 |
| d2h pinned 1 MiB | GB/s (high is better) | 2.04 [1.86-2.26] n=4 | 12.3 [12.2-12.3] n=4 | 0.17 |
| d2d device 1 MiB | GB/s (high is better) | 0.022 [0.022-0.023] n=4 | 134 [130-141] n=4 | 0.00 |
| h2d pageable 1 MiB | GB/s (high is better) | 1.99 [1.89-2.01] n=4 | 7.38 [7.25-7.42] n=4 | 0.27 |
| d2h pageable 1 MiB | GB/s (high is better) | 1.94 [1.91-1.97] n=4 | 6.44 [6.29-6.47] n=4 | 0.30 |
| h2d pinned 16 MiB | GB/s (high is better) | 3.16 [3.13-3.18] n=4 | 13.1 [13.0-13.1] n=4 | 0.24 |
| d2h pinned 16 MiB | GB/s (high is better) | 3.23 [3.19-3.24] n=4 | 13.1 [13.1-13.1] n=4 | 0.25 |
| d2d device 16 MiB | GB/s (high is better) | 0.426 [0.303-0.521] n=4 | 182 [182-184] n=4 | 0.00 |
| h2d pageable 16 MiB | GB/s (high is better) | 4.05 [2.80-4.29] n=4 | 10.9 [9.58-11.1] n=4 | 0.37 |
| d2h pageable 16 MiB | GB/s (high is better) | 3.80 [3.69-4.08] n=4 | 8.99 [8.73-9.04] n=4 | 0.42 |
| h2d pinned 256 MiB | GB/s (high is better) | 12.8 [12.5-13.0] n=4 | 13.1 [13.1-13.1] n=4 | 0.98 |
| d2h pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=4 | 13.2 [13.2-13.2] n=4 | 0.99 |
| d2d device 256 MiB | GB/s (high is better) | 2.41 [2.31-2.47] n=4 | 189 [186-190] n=4 | 0.01 |
| h2d pageable 256 MiB | GB/s (high is better) | 3.67 [3.62-3.96] n=4 | 9.11 [8.73-9.48] n=4 | 0.40 |
| d2h pageable 256 MiB | GB/s (high is better) | 3.49 [3.38-3.70] n=4 | 8.56 [8.39-9.05] n=4 | 0.41 |

### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 23.1 [23.1-23.2] n=4 | 5276 [4565-5322] n=4 | 0.00 |
| N=2048 | GFLOP/s (high is better) | 156 [137-159] n=4 | 5545 [5249-5630] n=4 | 0.03 |
| N=4096 | GFLOP/s (high is better) | 2571 [2342-2969] n=4 | 6126 [5908-6413] n=4 | 0.42 |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| mean10 (reward, not a time) |  | n/a | 56.9 [56.9-56.9] n=1 | - |
| process wall (incl. torch import) | s (low is better) | n/a | 4.50 [4.50-4.50] n=1 | - |
| stage4 (50 training episodes) | s (low is better) | n/a | 1.30 [1.30-1.30] n=1 | - |
| H2D+D2H transfers per second | 1/s (high is better) | n/a | 2288 [2288-2288] n=1 | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| copy device CPU | ms (low is better) | 34000 [33870-35330] n=4 | n/a | - |
| copy VMM CPU | ms (low is better) | 47850 [46210-50120] n=4 | n/a | - |
| launch-auto device CPU | ms (low is better) | 13720 [13430-13750] n=4 | n/a | - |
| launch-auto VMM CPU | ms (low is better) | 13110 [10610-14100] n=4 | n/a | - |
| launch-blocking device CPU | ms (low is better) | 19935 [19770-22050] n=4 | n/a | - |
| launch-blocking VMM CPU | ms (low is better) | 18845 [16640-20330] n=4 | n/a | - |
| mm device CPU | ms (low is better) | 28705 [28610-28780] n=4 | n/a | - |
| mm VMM CPU | ms (low is better) | 32425 [16620-37710] n=4 | n/a | - |
| rl device CPU | ms (low is better) | 875020 [875020-875020] n=1 | n/a | - |
| rl VMM CPU | ms (low is better) | 513000 [513000-513000] n=1 | n/a | - |
| short-auto device CPU | ms (low is better) | 16100 [16080-16120] n=4 | n/a | - |
| short-auto VMM CPU | ms (low is better) | 18640 [15890-19680] n=4 | n/a | - |
| short-blocking device CPU | ms (low is better) | 24330 [22790-24960] n=4 | n/a | - |
| short-blocking VMM CPU | ms (low is better) | 20705 [17060-26730] n=4 | n/a | - |

### (8) Device process memory (over all runs)

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| device HWM | MiB (low is better) | 4109 [4108-4129] n=25 | n/a | - |
| device RSS after run | MiB (low is better) | 4109 [4108-4121] n=25 | n/a | - |


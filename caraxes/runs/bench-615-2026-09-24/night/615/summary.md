Sources: v2=/home/silas/git/Caraxes/.claude/worktrees/agent-a25d59d08c801f257/caraxes/runs/bench-615-2026-09-24/night/615/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 25/65
Host 1-min load average at run start, median [min-max]: v2 7.50 [3.42-13.9] n=65

### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| ms/launch | ms (low is better) | 17.3 [16.8-18.0] n=5 | - |

### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | v2 | v2 / None |
|---|---|---|---|
| ms/launch | ms (low is better) | 17.4 [17.0-17.7] n=5 | - |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 | v2 / None |
|---|---|---|---|
| ms/launch | ms (low is better) | 0.018 [0.015-0.040] n=5 | - |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | v2 | v2 / None |
|---|---|---|---|
| ms/launch | ms (low is better) | 0.085 [0.023-0.414] n=5 | - |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 6.07 [6.03-11.2] n=5 | - |
| d2h pinned 1 MiB | GB/s (high is better) | 6.20 [6.17-11.4] n=5 | - |
| d2d device 1 MiB | GB/s (high is better) | 66.5 [62.3-76.4] n=5 | - |
| h2d pageable 1 MiB | GB/s (high is better) | 4.54 [4.53-7.01] n=5 | - |
| d2h pageable 1 MiB | GB/s (high is better) | 4.21 [3.95-5.82] n=5 | - |
| h2d pinned 16 MiB | GB/s (high is better) | 6.57 [6.53-12.9] n=5 | - |
| d2h pinned 16 MiB | GB/s (high is better) | 6.61 [6.54-12.9] n=5 | - |
| d2d device 16 MiB | GB/s (high is better) | 16.5 [11.5-176] n=5 | - |
| h2d pageable 16 MiB | GB/s (high is better) | 6.98 [5.84-9.31] n=5 | - |
| d2h pageable 16 MiB | GB/s (high is better) | 8.22 [6.15-8.69] n=5 | - |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [9.34-13.1] n=5 | - |
| d2h pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.2] n=5 | - |
| d2d device 256 MiB | GB/s (high is better) | 192 [158-194] n=5 | - |
| h2d pageable 256 MiB | GB/s (high is better) | 8.65 [7.30-8.94] n=5 | - |
| d2h pageable 256 MiB | GB/s (high is better) | 7.58 [5.69-8.58] n=5 | - |

### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 1310 [1071-4540] n=5 | - |
| N=2048 | GFLOP/s (high is better) | 2407 [1458-4788] n=5 | - |
| N=4096 | GFLOP/s (high is better) | 4627 [3696-5304] n=5 | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 | v2 / None |
|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 | - |
| process wall (incl. torch import) | s (low is better) | 13.5 [11.4-22.5] n=5 | - |
| stage4 (50 training episodes) | s (low is better) | 2.60 [2.20-4.90] n=5 | - |
| H2D+D2H transfers per second | 1/s (high is better) | 1106 [585-1332] n=5 | - |

### (5) Module load to first successful cuInit (in the guest)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| module load | ms (low is better) | 3630 [3569-4039] n=5 | - |
| load to first cuInit | ms (low is better) | 3825 [3797-4230] n=5 | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| cuInit | ms (low is better) | 198 [169-1493] n=5 | - |
| cuCtxCreate | ms (low is better) | 4444 [3650-13110] n=5 | - |
| cuCtxDestroy | ms (low is better) | 3057 [2385-3795] n=5 | - |

### (5) nvprobe 0 process, bash time

| value | unit | v2 | v2 / None |
|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 268 [219-299] n=5 | - |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 268 [219-299] n=5 | - |

### (5) nvprobe 1 process, bash time

| value | unit | v2 | v2 / None |
|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 5922 [4620-9626] n=5 | - |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 5973 [4621-10281] n=5 | - |

### (6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR

| value | unit | v2 | v2 / None |
|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 1.59 [1.32-2.43] n=5 | - |
| CHECK_VERSION_STR p99 | us (low is better) | 2.52 [1.92-8.08] n=5 | - |
| CHECK_VERSION_STR mean | us (low is better) | 1.74 [1.44-3.27] n=5 | - |

### (6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size

| value | unit | v2 | v2 / None |
|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 2.37 [2.36-2.95] n=5 | - |
| RM_CONTROL 232 B p99 | us (low is better) | 4.74 [2.76-12.3] n=5 | - |
| RM_CONTROL 1576 B p50 | us (low is better) | 2.91 [2.36-4.16] n=5 | - |
| RM_CONTROL 1576 B p99 | us (low is better) | 3.90 [3.06-18.6] n=5 | - |
| RM_CONTROL 12328 B p50 | us (low is better) | 2.38 [2.33-4.73] n=5 | - |
| RM_CONTROL 12328 B p99 | us (low is better) | 6.51 [3.02-22.2] n=5 | - |
| RM_CONTROL 49192 B p50 | us (low is better) | 2.37 [2.34-5.64] n=5 | - |
| RM_CONTROL 49192 B p99 | us (low is better) | 3.58 [2.58-20.6] n=5 | - |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 238 [184-616] n=5 | - |
| 64x1 MiB cuMemFree | us (low is better) | 463 [245-989] n=5 | - |
| 64x1 MiB cycle | ms (low is better) | 47.5 [40.2-277] n=5 | - |
| 8x64 MiB cuMemAlloc | us (low is better) | 4919 [4441-16320] n=5 | - |
| 8x64 MiB cuMemFree | us (low is better) | 3067 [2855-9456] n=5 | - |
| 8x64 MiB cycle | ms (low is better) | 118 [82.3-246] n=5 | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 | v2 / None |
|---|---|---|---|
| churn device CPU | ms (low is better) | 16740 [15340-26960] n=5 | - |
| churn VMM CPU | ms (low is better) | 14050 [4630-18390] n=5 | - |
| copy device CPU | ms (low is better) | 28920 [25480-31700] n=5 | - |
| copy VMM CPU | ms (low is better) | 30580 [11620-32990] n=5 | - |
| ctrl device CPU | ms (low is better) | 280 [220-500] n=5 | - |
| ctrl VMM CPU | ms (low is better) | 460 [240-510] n=5 | - |
| init device CPU | ms (low is better) | 12150 [10360-18160] n=5 | - |
| init VMM CPU | ms (low is better) | 8480 [2640-11330] n=5 | - |
| ioctl device CPU | ms (low is better) | 260 [220-590] n=5 | - |
| ioctl VMM CPU | ms (low is better) | 390 [200-1110] n=5 | - |
| launch-auto device CPU | ms (low is better) | 9230 [8120-9930] n=5 | - |
| launch-auto VMM CPU | ms (low is better) | 10510 [8540-11760] n=5 | - |
| launch-blocking device CPU | ms (low is better) | 12840 [12060-17120] n=5 | - |
| launch-blocking VMM CPU | ms (low is better) | 10600 [3030-12710] n=5 | - |
| mm device CPU | ms (low is better) | 16520 [13630-17040] n=5 | - |
| mm VMM CPU | ms (low is better) | 16310 [6770-19110] n=5 | - |
| rl device CPU | ms (low is better) | 16750 [14870-23260] n=5 | - |
| rl VMM CPU | ms (low is better) | 21490 [9530-29810] n=5 | - |
| short-auto device CPU | ms (low is better) | 7440 [7010-8650] n=5 | - |
| short-auto VMM CPU | ms (low is better) | 7620 [2900-8240] n=5 | - |
| short-blocking device CPU | ms (low is better) | 11080 [10850-13540] n=5 | - |
| short-blocking VMM CPU | ms (low is better) | 8750 [2900-11240] n=5 | - |
| time0 device CPU | ms (low is better) | 610 [540-800] n=5 | - |
| time0 VMM CPU | ms (low is better) | 640 [260-730] n=5 | - |
| time1 device CPU | ms (low is better) | 8310 [7260-11720] n=5 | - |
| time1 VMM CPU | ms (low is better) | 8030 [2710-10060] n=5 | - |

### (8) Device process memory (over all runs)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| device HWM | MiB (low is better) | 8199 [8199-8199] n=65 | - |
| device RSS after run | MiB (low is better) | 5139 [5026-5144] n=65 | - |


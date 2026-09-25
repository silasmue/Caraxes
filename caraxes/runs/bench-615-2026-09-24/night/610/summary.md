Sources: v2=/home/silas/git/Caraxes/.claude/worktrees/agent-a25d59d08c801f257/caraxes/runs/bench-615-2026-09-24/night/610/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 0/65
Host 1-min load average at run start, median [min-max]: v2 4.15 [2.61-7.61] n=65

### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| ms/launch | ms (low is better) | 17.0 [16.7-17.3] n=5 | - |

### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | v2 | v2 / None |
|---|---|---|---|
| ms/launch | ms (low is better) | 17.2 [16.7-17.7] n=5 | - |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 | v2 / None |
|---|---|---|---|
| ms/launch | ms (low is better) | 0.016 [0.012-0.023] n=5 | - |

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | v2 | v2 / None |
|---|---|---|---|
| ms/launch | ms (low is better) | 0.089 [0.031-0.142] n=5 | - |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 6.08 [5.97-11.1] n=5 | - |
| d2h pinned 1 MiB | GB/s (high is better) | 6.20 [6.18-11.6] n=5 | - |
| d2d device 1 MiB | GB/s (high is better) | 65.9 [63.4-76.9] n=5 | - |
| h2d pageable 1 MiB | GB/s (high is better) | 4.55 [4.50-7.08] n=5 | - |
| d2h pageable 1 MiB | GB/s (high is better) | 4.28 [4.21-6.32] n=5 | - |
| h2d pinned 16 MiB | GB/s (high is better) | 6.57 [6.55-12.9] n=5 | - |
| d2h pinned 16 MiB | GB/s (high is better) | 6.64 [5.71-13.0] n=5 | - |
| d2d device 16 MiB | GB/s (high is better) | 16.9 [11.4-178] n=5 | - |
| h2d pageable 16 MiB | GB/s (high is better) | 9.21 [4.61-10.0] n=5 | - |
| d2h pageable 16 MiB | GB/s (high is better) | 7.92 [6.33-8.28] n=5 | - |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [7.47-13.1] n=5 | - |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [13.2-13.2] n=5 | - |
| d2d device 256 MiB | GB/s (high is better) | 193 [190-194] n=5 | - |
| h2d pageable 256 MiB | GB/s (high is better) | 8.69 [8.52-9.12] n=5 | - |
| d2h pageable 256 MiB | GB/s (high is better) | 8.68 [8.54-9.04] n=5 | - |

### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 4850 [1334-5252] n=5 | - |
| N=2048 | GFLOP/s (high is better) | 5149 [2579-5430] n=5 | - |
| N=4096 | GFLOP/s (high is better) | 5419 [5258-5797] n=5 | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 | v2 / None |
|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 | - |
| process wall (incl. torch import) | s (low is better) | 11.8 [11.5-12.3] n=5 | - |
| stage4 (50 training episodes) | s (low is better) | 2.00 [1.90-2.40] n=5 | - |
| H2D+D2H transfers per second | 1/s (high is better) | 1403 [1219-1534] n=5 | - |

### (5) Module load to first successful cuInit (in the guest)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| module load | ms (low is better) | 3635 [3324-7901] n=5 | - |
| load to first cuInit | ms (low is better) | 3813 [3489-8381] n=5 | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| cuInit | ms (low is better) | 161 [139-219] n=5 | - |
| cuCtxCreate | ms (low is better) | 4017 [3856-4245] n=5 | - |
| cuCtxDestroy | ms (low is better) | 2794 [2502-2846] n=5 | - |

### (5) nvprobe 0 process, bash time

| value | unit | v2 | v2 / None |
|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 201 [176-236] n=5 | - |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 201 [176-235] n=5 | - |

### (5) nvprobe 1 process, bash time

| value | unit | v2 | v2 / None |
|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 4530 [4264-4739] n=5 | - |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 4537 [4264-4739] n=5 | - |

### (6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR

| value | unit | v2 | v2 / None |
|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 1.58 [1.33-2.46] n=5 | - |
| CHECK_VERSION_STR p99 | us (low is better) | 2.37 [1.96-2.93] n=5 | - |
| CHECK_VERSION_STR mean | us (low is better) | 1.73 [1.39-2.54] n=5 | - |

### (6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size

| value | unit | v2 | v2 / None |
|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 2.99 [2.38-3.01] n=5 | - |
| RM_CONTROL 232 B p99 | us (low is better) | 4.08 [3.10-4.96] n=5 | - |
| RM_CONTROL 1576 B p50 | us (low is better) | 3.00 [2.38-3.08] n=5 | - |
| RM_CONTROL 1576 B p99 | us (low is better) | 4.01 [3.45-5.23] n=5 | - |
| RM_CONTROL 12328 B p50 | us (low is better) | 3.02 [2.38-3.04] n=5 | - |
| RM_CONTROL 12328 B p99 | us (low is better) | 4.20 [3.24-6.14] n=5 | - |
| RM_CONTROL 49192 B p50 | us (low is better) | 3.03 [2.47-3.08] n=5 | - |
| RM_CONTROL 49192 B p99 | us (low is better) | 4.04 [3.64-6.53] n=5 | - |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 204 [182-218] n=5 | - |
| 64x1 MiB cuMemFree | us (low is better) | 273 [239-482] n=5 | - |
| 64x1 MiB cycle | ms (low is better) | 43.2 [39.3-57.8] n=5 | - |
| 8x64 MiB cuMemAlloc | us (low is better) | 4841 [4706-8156] n=5 | - |
| 8x64 MiB cuMemFree | us (low is better) | 2850 [2842-5499] n=5 | - |
| 8x64 MiB cycle | ms (low is better) | 97.0 [94.1-138] n=5 | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 | v2 / None |
|---|---|---|---|
| churn device CPU | ms (low is better) | 16150 [15830-18500] n=5 | - |
| churn VMM CPU | ms (low is better) | 13190 [5650-13500] n=5 | - |
| copy device CPU | ms (low is better) | 25690 [24990-27260] n=5 | - |
| copy VMM CPU | ms (low is better) | 29300 [28140-30920] n=5 | - |
| ctrl device CPU | ms (low is better) | 230 [220-250] n=5 | - |
| ctrl VMM CPU | ms (low is better) | 390 [200-460] n=5 | - |
| init device CPU | ms (low is better) | 11210 [10930-11530] n=5 | - |
| init VMM CPU | ms (low is better) | 8760 [8550-9710] n=5 | - |
| ioctl device CPU | ms (low is better) | 230 [220-240] n=5 | - |
| ioctl VMM CPU | ms (low is better) | 400 [200-440] n=5 | - |
| launch-auto device CPU | ms (low is better) | 8080 [7880-9050] n=5 | - |
| launch-auto VMM CPU | ms (low is better) | 8080 [6740-10900] n=5 | - |
| launch-blocking device CPU | ms (low is better) | 12030 [11890-12140] n=5 | - |
| launch-blocking VMM CPU | ms (low is better) | 9420 [9070-10590] n=5 | - |
| mm device CPU | ms (low is better) | 14100 [13500-15290] n=5 | - |
| mm VMM CPU | ms (low is better) | 17710 [17230-19980] n=5 | - |
| rl device CPU | ms (low is better) | 15610 [15100-15840] n=5 | - |
| rl VMM CPU | ms (low is better) | 21710 [21310-22210] n=5 | - |
| short-auto device CPU | ms (low is better) | 6710 [6490-7490] n=5 | - |
| short-auto VMM CPU | ms (low is better) | 7030 [6810-8110] n=5 | - |
| short-blocking device CPU | ms (low is better) | 10810 [10680-11710] n=5 | - |
| short-blocking VMM CPU | ms (low is better) | 8660 [7640-9370] n=5 | - |
| time0 device CPU | ms (low is better) | 530 [490-710] n=5 | - |
| time0 VMM CPU | ms (low is better) | 620 [470-670] n=5 | - |
| time1 device CPU | ms (low is better) | 7050 [6660-7380] n=5 | - |
| time1 VMM CPU | ms (low is better) | 6970 [2730-7440] n=5 | - |

### (8) Device process memory (over all runs)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| device HWM | MiB (low is better) | 8199 [8199-8199] n=65 | - |
| device RSS after run | MiB (low is better) | 5143 [5131-5145] n=65 | - |


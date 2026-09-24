Sources: v2=/home/silas/git/Caraxes/.claude/worktrees/agent-a075385b5adb08982/caraxes/runs/bench-615-2026-09-24/610/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 9/14
Host 1-min load average at run start, median [min-max]: v2 4.65 [2.22-10.9] n=14

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 | v2 / None |
|---|---|---|---|
| ms/launch | ms (low is better) | 0.307 [0.157-0.382] n=3 | - |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 3.90 [2.58-4.30] n=3 | - |
| d2h pinned 1 MiB | GB/s (high is better) | 3.95 [2.37-4.34] n=3 | - |
| d2d device 1 MiB | GB/s (high is better) | 9.60 [6.04-10.6] n=3 | - |
| h2d pageable 1 MiB | GB/s (high is better) | 2.92 [2.55-3.18] n=3 | - |
| d2h pageable 1 MiB | GB/s (high is better) | 2.96 [2.48-3.33] n=3 | - |
| h2d pinned 16 MiB | GB/s (high is better) | 6.28 [3.13-11.5] n=3 | - |
| d2h pinned 16 MiB | GB/s (high is better) | 6.28 [5.88-6.32] n=3 | - |
| d2d device 16 MiB | GB/s (high is better) | 14.5 [13.5-14.6] n=3 | - |
| h2d pageable 16 MiB | GB/s (high is better) | 4.11 [2.23-4.60] n=3 | - |
| d2h pageable 16 MiB | GB/s (high is better) | 2.83 [2.82-4.65] n=3 | - |
| h2d pinned 256 MiB | GB/s (high is better) | 12.9 [12.2-13.0] n=3 | - |
| d2h pinned 256 MiB | GB/s (high is better) | 13.1 [13.0-13.1] n=3 | - |
| d2d device 256 MiB | GB/s (high is better) | 177 [167-180] n=3 | - |
| h2d pageable 256 MiB | GB/s (high is better) | 4.43 [3.39-4.65] n=3 | - |
| d2h pageable 256 MiB | GB/s (high is better) | 3.33 [3.30-4.67] n=3 | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 | v2 / None |
|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=3 | - |
| process wall (incl. torch import) | s (low is better) | 41.0 [35.2-42.3] n=3 | - |
| stage4 (50 training episodes) | s (low is better) | 20.4 [19.5-22.4] n=3 | - |
| H2D+D2H transfers per second | 1/s (high is better) | 141 [128-147] n=3 | - |

### (4) PPO CartPole, 128 vector environments, one H2D and one synchronous D2H per step (pporl, 60 s)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| environment steps per second | 1/s (high is better) | 6718 [6471-6964] n=2 | - |
| reward100 at the end (not a time) |  | 36.8 [36.8-36.8] n=2 | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| cuInit | ms (low is better) | 212 [148-315] n=3 | - |
| cuCtxCreate | ms (low is better) | 4144 [3381-5122] n=3 | - |
| cuCtxDestroy | ms (low is better) | 1350 [942-1607] n=3 | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 | v2 / None |
|---|---|---|---|
| copy device CPU | ms (low is better) | 10260 [8860-10520] n=3 | - |
| copy VMM CPU | ms (low is better) | 34660 [12440-35440] n=3 | - |
| init device CPU | ms (low is better) | 2640 [2590-3160] n=3 | - |
| init VMM CPU | ms (low is better) | 8710 [3180-10260] n=3 | - |
| ppo device CPU | ms (low is better) | 58305 [57640-58970] n=2 | - |
| ppo VMM CPU | ms (low is better) | 66445 [47020-85870] n=2 | - |
| rl device CPU | ms (low is better) | 21710 [21160-23500] n=3 | - |
| rl VMM CPU | ms (low is better) | 57630 [52450-63000] n=3 | - |
| short-auto device CPU | ms (low is better) | 2490 [2070-2630] n=3 | - |
| short-auto VMM CPU | ms (low is better) | 8190 [7290-9060] n=3 | - |

### (8) Device process memory (over all runs)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| device HWM | MiB (low is better) | 7174 [5131-7175] n=14 | - |
| device RSS after run | MiB (low is better) | 5136 [5130-5143] n=14 | - |


Sources: v2=/home/silas/git/Caraxes/.claude/worktrees/agent-a075385b5adb08982/caraxes/runs/bench-615-2026-09-24/615/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 5/14
Host 1-min load average at run start, median [min-max]: v2 3.49 [1.60-5.80] n=14

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 | v2 / None |
|---|---|---|---|
| ms/launch | ms (low is better) | 0.144 [0.127-0.148] n=3 | - |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 3.85 [3.61-3.90] n=3 | - |
| d2h pinned 1 MiB | GB/s (high is better) | 3.83 [3.61-3.95] n=3 | - |
| d2d device 1 MiB | GB/s (high is better) | 8.44 [4.91-9.87] n=3 | - |
| h2d pageable 1 MiB | GB/s (high is better) | 2.65 [2.42-2.81] n=3 | - |
| d2h pageable 1 MiB | GB/s (high is better) | 2.77 [2.31-2.89] n=3 | - |
| h2d pinned 16 MiB | GB/s (high is better) | 6.29 [6.25-6.29] n=3 | - |
| d2h pinned 16 MiB | GB/s (high is better) | 6.23 [6.00-6.33] n=3 | - |
| d2d device 16 MiB | GB/s (high is better) | 14.1 [13.3-14.6] n=3 | - |
| h2d pageable 16 MiB | GB/s (high is better) | 4.36 [4.25-4.82] n=3 | - |
| d2h pageable 16 MiB | GB/s (high is better) | 4.50 [3.68-4.99] n=3 | - |
| h2d pinned 256 MiB | GB/s (high is better) | 12.8 [12.6-12.9] n=3 | - |
| d2h pinned 256 MiB | GB/s (high is better) | 13.1 [12.9-13.1] n=3 | - |
| d2d device 256 MiB | GB/s (high is better) | 175 [173-177] n=3 | - |
| h2d pageable 256 MiB | GB/s (high is better) | 3.32 [3.13-4.34] n=3 | - |
| d2h pageable 256 MiB | GB/s (high is better) | 3.62 [2.69-3.65] n=3 | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 | v2 / None |
|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=3 | - |
| process wall (incl. torch import) | s (low is better) | 27.5 [26.3-33.0] n=3 | - |
| stage4 (50 training episodes) | s (low is better) | 15.2 [15.0-20.2] n=3 | - |
| H2D+D2H transfers per second | 1/s (high is better) | 189 [143-192] n=3 | - |

### (4) PPO CartPole, 128 vector environments, one H2D and one synchronous D2H per step (pporl, 60 s)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| environment steps per second | 1/s (high is better) | 6226 [5727-6726] n=2 | - |
| reward100 at the end (not a time) |  | 33.5 [30.2-36.8] n=2 | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| cuInit | ms (low is better) | 135 [134-233] n=3 | - |
| cuCtxCreate | ms (low is better) | 4349 [3683-4788] n=3 | - |
| cuCtxDestroy | ms (low is better) | 1517 [1115-1644] n=3 | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 | v2 / None |
|---|---|---|---|
| copy device CPU | ms (low is better) | 10200 [9540-10740] n=3 | - |
| copy VMM CPU | ms (low is better) | 32910 [31440-36140] n=3 | - |
| init device CPU | ms (low is better) | 2980 [2760-3170] n=3 | - |
| init VMM CPU | ms (low is better) | 8870 [4580-9890] n=3 | - |
| ppo device CPU | ms (low is better) | 54310 [50460-58160] n=2 | - |
| ppo VMM CPU | ms (low is better) | 108990 [103280-114700] n=2 | - |
| rl device CPU | ms (low is better) | 21000 [20990-22190] n=3 | - |
| rl VMM CPU | ms (low is better) | 47480 [45830-52350] n=3 | - |
| short-auto device CPU | ms (low is better) | 2070 [2070-2140] n=3 | - |
| short-auto VMM CPU | ms (low is better) | 5950 [5220-7640] n=3 | - |

### (8) Device process memory (over all runs)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| device HWM | MiB (low is better) | 6150 [6150-6151] n=14 | - |
| device RSS after run | MiB (low is better) | 5135 [5130-5143] n=14 | - |


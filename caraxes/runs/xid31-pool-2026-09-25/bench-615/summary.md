Sources: v2 (hold0)=/home/silas/git/Caraxes/.claude/worktrees/agent-a1883803234bbb972/caraxes/runs/xid31-pool-2026-09-25/bench-615/hold0/v2, v2 (hold1)=/home/silas/git/Caraxes/.claude/worktrees/agent-a1883803234bbb972/caraxes/runs/xid31-pool-2026-09-25/bench-615/hold1/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (hold0) 0/9, v2 (hold1) 0/9
Host 1-min load average at run start, median [min-max]: v2 (hold0) 2.13 [1.97-2.20] n=9, v2 (hold1) 2.22 [1.98-2.45] n=9

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 (hold0) | v2 (hold1) | v2 (hold0) / None | v2 (hold1) / None |
|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.019 [0.017-0.023] n=3 | 0.020 [0.018-0.032] n=3 | - | - |

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v2 (hold0) | v2 (hold1) | v2 (hold0) / None | v2 (hold1) / None |
|---|---|---|---|---|---|
| cuInit | ms (low is better) | 13.6 [12.2-15.0] n=3 | 16.3 [16.0-27.3] n=3 | - | - |
| cuCtxCreate | ms (low is better) | 395 [374-396] n=3 | 486 [438-593] n=3 | - | - |
| cuCtxDestroy | ms (low is better) | 188 [181-196] n=3 | 174 [160-183] n=3 | - | - |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | v2 (hold0) | v2 (hold1) | v2 (hold0) / None | v2 (hold1) / None |
|---|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 175 [171-182] n=3 | 200 [197-218] n=3 | - | - |
| 64x1 MiB cuMemFree | us (low is better) | 282 [281-396] n=3 | 312 [304-414] n=3 | - | - |
| 64x1 MiB cycle | ms (low is better) | 32.9 [31.2-34.2] n=3 | 39.5 [37.1-48.0] n=3 | - | - |
| 8x64 MiB cuMemAlloc | us (low is better) | 511 [484-517] n=3 | 2277 [2185-2378] n=3 | - | - |
| 8x64 MiB cuMemFree | us (low is better) | 791 [766-808] n=3 | 2402 [2329-2523] n=3 | - | - |
| 8x64 MiB cycle | ms (low is better) | 11.1 [10.7-11.5] n=3 | 39.2 [37.2-42.6] n=3 | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (hold0) | v2 (hold1) | v2 (hold0) / None | v2 (hold1) / None |
|---|---|---|---|---|---|
| churn device CPU | ms (low is better) | 1180 [1170-1280] n=3 | 1840 [1810-1940] n=3 | - | - |
| churn VMM CPU | ms (low is better) | 2690 [1370-2710] n=3 | 3630 [2250-3890] n=3 | - | - |
| init device CPU | ms (low is better) | 490 [490-500] n=3 | 580 [550-670] n=3 | - | - |
| init VMM CPU | ms (low is better) | 1060 [950-1100] n=3 | 1260 [1150-1290] n=3 | - | - |
| short-auto device CPU | ms (low is better) | 460 [410-680] n=3 | 430 [410-530] n=3 | - | - |
| short-auto VMM CPU | ms (low is better) | 1070 [990-1320] n=3 | 1010 [1000-1360] n=3 | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (hold0) | v2 (hold1) | v2 (hold0) / None | v2 (hold1) / None |
|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 9223 [9223-9223] n=9 | 9224 [9224-9224] n=9 | - | - |
| device RSS after run | MiB (low is better) | 5133 [5133-5133] n=9 | 5134 [5133-5134] n=9 | - | - |


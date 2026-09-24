Sources: v2 (off)=/home/silas/git/Caraxes/.claude/worktrees/agent-a25d59d08c801f257/caraxes/runs/alloc-path-2026-09-24/night/off/v2, v2 (intr)=/home/silas/git/Caraxes/.claude/worktrees/agent-a25d59d08c801f257/caraxes/runs/alloc-path-2026-09-24/night/intr/v2, v2 (stack-off)=/home/silas/git/Caraxes/.claude/worktrees/agent-a25d59d08c801f257/caraxes/runs/alloc-path-2026-09-24/night/stack-off/v2, v2 (stack-on)=/home/silas/git/Caraxes/.claude/worktrees/agent-a25d59d08c801f257/caraxes/runs/alloc-path-2026-09-24/night/stack-on/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (off) 6/10, v2 (intr) 5/10, v2 (stack-off) 4/10, v2 (stack-on) 4/10
Host 1-min load average at run start, median [min-max]: v2 (off) 5.28 [4.20-6.04] n=10, v2 (intr) 3.90 [2.56-6.01] n=10, v2 (stack-off) 2.58 [1.33-3.14] n=10, v2 (stack-on) 2.90 [2.02-3.66] n=10

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v2 (off) | v2 (intr) | v2 (stack-off) | v2 (stack-on) | v2 (off) / None | v2 (intr) / None | v2 (stack-off) / None | v2 (stack-on) / None |
|---|---|---|---|---|---|---|---|---|---|
| cuInit | ms (low is better) | 209 [175-307] n=5 | 196 [144-240] n=5 | 44.3 [18.9-63.8] n=5 | 51.3 [13.0-105] n=5 | - | - | - | - |
| cuCtxCreate | ms (low is better) | 3493 [3306-4305] n=5 | 3863 [3129-4900] n=5 | 534 [471-1183] n=5 | 533 [389-943] n=5 | - | - | - | - |
| cuCtxDestroy | ms (low is better) | 1272 [949-1457] n=5 | 1166 [946-1403] n=5 | 174 [139-335] n=5 | 150 [119-240] n=5 | - | - | - | - |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | v2 (off) | v2 (intr) | v2 (stack-off) | v2 (stack-on) | v2 (off) / None | v2 (intr) / None | v2 (stack-off) / None | v2 (stack-on) / None |
|---|---|---|---|---|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 972 [835-1689] n=5 | 581 [199-1138] n=4 | 297 [181-338] n=5 | 175 [166-222] n=5 | - | - | - | - |
| 64x1 MiB cuMemFree | us (low is better) | 1020 [899-1698] n=5 | 680 [234-1680] n=4 | 541 [292-561] n=5 | 401 [280-532] n=5 | - | - | - | - |
| 64x1 MiB cycle | ms (low is better) | 227 [147-276] n=5 | 102 [37.0-191] n=4 | 48.1 [36.7-60.4] n=5 | 37.1 [33.9-43.2] n=5 | - | - | - | - |
| 8x64 MiB cuMemAlloc | us (low is better) | 7337 [6452-8664] n=5 | 5590 [4605-9301] n=4 | 650 [611-815] n=5 | 541 [529-588] n=5 | - | - | - | - |
| 8x64 MiB cuMemFree | us (low is better) | 4926 [4393-5313] n=5 | 3945 [2694-6222] n=4 | 961 [942-1228] n=5 | 838 [808-892] n=5 | - | - | - | - |
| 8x64 MiB cycle | ms (low is better) | 154 [112-169] n=5 | 113 [86.9-172] n=4 | 15.5 [14.5-21.5] n=5 | 11.7 [11.4-12.6] n=5 | - | - | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (off) | v2 (intr) | v2 (stack-off) | v2 (stack-on) | v2 (off) / None | v2 (intr) / None | v2 (stack-off) / None | v2 (stack-on) / None |
|---|---|---|---|---|---|---|---|---|---|
| churn device CPU | ms (low is better) | 6830 [5970-7720] n=5 | 4960 [690-9040] n=5 | 2410 [2230-2910] n=5 | 1850 [1800-2210] n=5 | - | - | - | - |
| churn VMM CPU | ms (low is better) | 21230 [17550-22740] n=5 | 11610 [2420-16800] n=5 | 3290 [1510-4040] n=5 | 2170 [1360-3320] n=5 | - | - | - | - |
| init device CPU | ms (low is better) | 2100 [2020-2830] n=5 | 2320 [2200-3410] n=5 | 490 [440-1230] n=5 | 550 [390-1090] n=5 | - | - | - | - |
| init VMM CPU | ms (low is better) | 7780 [5310-8000] n=5 | 6780 [4540-8390] n=5 | 500 [490-1960] n=5 | 460 [460-1520] n=5 | - | - | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (off) | v2 (intr) | v2 (stack-off) | v2 (stack-on) | v2 (off) / None | v2 (intr) / None | v2 (stack-off) / None | v2 (stack-on) / None |
|---|---|---|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 8198 [6151-8199] n=10 | 8199 [6151-8199] n=10 | 8199 [6151-8199] n=10 | 8199 [8199-8199] n=10 | - | - | - | - |
| device RSS after run | MiB (low is better) | 5132 [5131-5132] n=10 | 5132 [5131-5132] n=10 | 5134 [5132-5134] n=10 | 5134 [5132-5134] n=10 | - | - | - | - |


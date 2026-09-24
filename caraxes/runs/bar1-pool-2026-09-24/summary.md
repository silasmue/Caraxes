Sources: v2 (1)=/home/silas/git/Caraxes/caraxes/runs/bar1-pool-2026-09-24/1/v2, v2 (pool)=/home/silas/git/Caraxes/caraxes/runs/bar1-pool-2026-09-24/pool/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (1) 0/15, v2 (pool) 0/15
Host 1-min load average at run start, median [min-max]: v2 (1) 2.57 [1.91-3.14] n=15, v2 (pool) 2.75 [1.98-4.07] n=15

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 (1) | v2 (pool) | v2 (1) / None | v2 (pool) / None |
|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.015 [0.013-0.024] n=5 | 0.020 [0.016-0.027] n=5 | - | - |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v2 (1) | v2 (pool) | v2 (1) / None | v2 (pool) / None |
|---|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 6.11 [3.13-6.13] n=5 | 6.08 [6.02-6.10] n=5 | - | - |
| d2h pinned 1 MiB | GB/s (high is better) | 6.22 [3.21-6.24] n=5 | 6.19 [6.10-6.21] n=5 | - | - |
| d2d device 1 MiB | GB/s (high is better) | 69.2 [54.6-72.2] n=5 | 64.8 [61.2-66.0] n=5 | - | - |
| h2d pageable 1 MiB | GB/s (high is better) | 4.58 [2.69-4.61] n=5 | 4.58 [4.55-4.59] n=5 | - | - |
| d2h pageable 1 MiB | GB/s (high is better) | 4.23 [2.59-4.31] n=5 | 4.20 [3.98-4.25] n=5 | - | - |
| h2d pinned 16 MiB | GB/s (high is better) | 6.58 [6.56-6.59] n=5 | 6.57 [6.55-6.58] n=5 | - | - |
| d2h pinned 16 MiB | GB/s (high is better) | 6.63 [6.61-6.64] n=5 | 6.64 [6.64-6.65] n=5 | - | - |
| d2d device 16 MiB | GB/s (high is better) | 16.0 [14.7-17.0] n=5 | 17.3 [15.7-17.7] n=5 | - | - |
| h2d pageable 16 MiB | GB/s (high is better) | 6.33 [6.17-8.38] n=5 | 6.30 [6.25-8.62] n=5 | - | - |
| d2h pageable 16 MiB | GB/s (high is better) | 6.43 [6.31-8.87] n=5 | 8.10 [6.61-8.61] n=5 | - | - |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [13.0-13.1] n=5 | 13.1 [12.5-13.1] n=5 | - | - |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [13.2-13.2] n=5 | 13.2 [13.2-13.2] n=5 | - | - |
| d2d device 256 MiB | GB/s (high is better) | 189 [183-194] n=5 | 192 [189-193] n=5 | - | - |
| h2d pageable 256 MiB | GB/s (high is better) | 8.77 [8.15-9.13] n=5 | 8.79 [8.55-9.06] n=5 | - | - |
| d2h pageable 256 MiB | GB/s (high is better) | 8.26 [7.67-8.64] n=5 | 8.62 [8.04-8.83] n=5 | - | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 (1) | v2 (pool) | v2 (1) / None | v2 (pool) / None |
|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | - | - |
| process wall (incl. torch import) | s (low is better) | 15.8 [15.5-16.6] n=5 | 16.8 [16.7-17.1] n=5 | - | - |
| stage4 (50 training episodes) | s (low is better) | 2.30 [2.20-2.30] n=5 | 2.30 [2.10-2.50] n=5 | - | - |
| H2D+D2H transfers per second | 1/s (high is better) | 1263 [1244-1309] n=5 | 1267 [1159-1352] n=5 | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (1) | v2 (pool) | v2 (1) / None | v2 (pool) / None |
|---|---|---|---|---|---|
| copy device CPU | ms (low is better) | 24430 [24040-25060] n=5 | 27190 [26960-28370] n=5 | - | - |
| copy VMM CPU | ms (low is better) | 28780 [28060-30560] n=5 | 29490 [29360-30380] n=5 | - | - |
| rl device CPU | ms (low is better) | 19740 [19280-20390] n=5 | 21350 [21190-21560] n=5 | - | - |
| rl VMM CPU | ms (low is better) | 29970 [28980-30580] n=5 | 30540 [29540-31350] n=5 | - | - |
| short-auto device CPU | ms (low is better) | 8110 [7740-8320] n=5 | 9220 [8330-9560] n=5 | - | - |
| short-auto VMM CPU | ms (low is better) | 7640 [7460-8340] n=5 | 8420 [8120-8890] n=5 | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (1) | v2 (pool) | v2 (1) / None | v2 (pool) / None |
|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 9222 [9222-9222] n=15 | 9222 [9222-9222] n=15 | - | - |
| device RSS after run | MiB (low is better) | 5142 [5141-5143] n=15 | 5139 [5139-5140] n=15 | - | - |


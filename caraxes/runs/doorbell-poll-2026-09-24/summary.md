Sources: v2 (C-off)=/home/silas/git/Caraxes/.claude/worktrees/agent-ab69fdfb34ad680cb/caraxes/runs/doorbell-poll-2026-09-24/C-off/v2, v2 (D0-spin-unpinned)=/home/silas/git/Caraxes/.claude/worktrees/agent-ab69fdfb34ad680cb/caraxes/runs/doorbell-poll-2026-09-24/D0-spin-unpinned/v2, v2 (D-spin-pinned)=/home/silas/git/Caraxes/.claude/worktrees/agent-ab69fdfb34ad680cb/caraxes/runs/doorbell-poll-2026-09-24/D-spin-pinned/v2, v2 (E-hybrid-200us)=/home/silas/git/Caraxes/.claude/worktrees/agent-ab69fdfb34ad680cb/caraxes/runs/doorbell-poll-2026-09-24/E-hybrid-200us/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (C-off) 6/6, v2 (D0-spin-unpinned) 6/6, v2 (D-spin-pinned) 0/6, v2 (E-hybrid-200us) 0/6
Host 1-min load average at run start, median [min-max]: v2 (C-off) 4.01 [3.08-4.33] n=6, v2 (D0-spin-unpinned) 4.97 [3.66-5.99] n=6, v2 (D-spin-pinned) 2.59 [2.09-2.99] n=6, v2 (E-hybrid-200us) 1.82 [1.47-2.21] n=6

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 (C-off) | v2 (D0-spin-unpinned) | v2 (D-spin-pinned) | v2 (E-hybrid-200us) | v2 (C-off) / None | v2 (D0-spin-unpinned) / None | v2 (D-spin-pinned) / None | v2 (E-hybrid-200us) / None |
|---|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.034 [0.027-0.043] n=3 | 0.016 [0.012-0.024] n=3 | 0.014 [0.014-0.020] n=3 | 0.020 [0.018-0.020] n=3 | - | - | - | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 (C-off) | v2 (D0-spin-unpinned) | v2 (D-spin-pinned) | v2 (E-hybrid-200us) | v2 (C-off) / None | v2 (D0-spin-unpinned) / None | v2 (D-spin-pinned) / None | v2 (E-hybrid-200us) / None |
|---|---|---|---|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=3 | 56.9 [56.9-56.9] n=3 | 56.9 [56.9-56.9] n=3 | 56.9 [56.9-56.9] n=3 | - | - | - | - |
| process wall (incl. torch import) | s (low is better) | 13.0 [12.8-17.7] n=3 | 16.0 [14.7-20.4] n=3 | 11.5 [10.9-16.7] n=3 | 10.9 [10.7-15.9] n=3 | - | - | - | - |
| stage4 (50 training episodes) | s (low is better) | 2.70 [2.60-2.90] n=3 | 2.90 [2.90-3.60] n=3 | 2.10 [2.00-2.20] n=3 | 2.20 [2.20-2.30] n=3 | - | - | - | - |
| H2D+D2H transfers per second | 1/s (high is better) | 1062 [989-1088] n=3 | 977 [792-981] n=3 | 1379 [1335-1455] n=3 | 1278 [1250-1299] n=3 | - | - | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (C-off) | v2 (D0-spin-unpinned) | v2 (D-spin-pinned) | v2 (E-hybrid-200us) | v2 (C-off) / None | v2 (D0-spin-unpinned) / None | v2 (D-spin-pinned) / None | v2 (E-hybrid-200us) / None |
|---|---|---|---|---|---|---|---|---|---|
| rl device CPU | ms (low is better) | 5270 [5200-6030] n=3 | 19990 [18140-24810] n=3 | 14960 [14570-20560] n=3 | 5670 [5450-5850] n=3 | - | - | - | - |
| rl VMM CPU | ms (low is better) | 24320 [22510-31310] n=3 | 25300 [24510-33800] n=3 | 20370 [19880-29380] n=3 | 19780 [18720-28210] n=3 | - | - | - | - |
| short-auto device CPU | ms (low is better) | 3220 [2060-3700] n=3 | 9420 [7080-10220] n=3 | 7710 [6030-8050] n=3 | 3120 [1850-3390] n=3 | - | - | - | - |
| short-auto VMM CPU | ms (low is better) | 9240 [3220-10020] n=3 | 9410 [3400-9860] n=3 | 7760 [2970-8080] n=3 | 7830 [2810-8070] n=3 | - | - | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (C-off) | v2 (D0-spin-unpinned) | v2 (D-spin-pinned) | v2 (E-hybrid-200us) | v2 (C-off) / None | v2 (D0-spin-unpinned) / None | v2 (D-spin-pinned) / None | v2 (E-hybrid-200us) / None |
|---|---|---|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 8197 [8197-8197] n=6 | 8198 [8198-8198] n=6 | 9222 [9222-9222] n=6 | 9222 [9222-9222] n=6 | - | - | - | - |
| device RSS after run | MiB (low is better) | 5131 [5130-5131] n=6 | 5131 [5130-5131] n=6 | 5131 [5130-5131] n=6 | 5131 [5130-5132] n=6 | - | - | - | - |


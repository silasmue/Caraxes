Sources: v2 (C-off)=/home/silas/git/Caraxes/.claude/worktrees/agent-ab69fdfb34ad680cb/caraxes/runs/doorbell-poll-2026-09-24/pass2/C-off/v2, v2 (D0-spin-unpinned)=/home/silas/git/Caraxes/.claude/worktrees/agent-ab69fdfb34ad680cb/caraxes/runs/doorbell-poll-2026-09-24/pass2/D0-spin-unpinned/v2, v2 (D-spin-pinned)=/home/silas/git/Caraxes/.claude/worktrees/agent-ab69fdfb34ad680cb/caraxes/runs/doorbell-poll-2026-09-24/pass2/D-spin-pinned/v2, v2 (E-hybrid-200us)=/home/silas/git/Caraxes/.claude/worktrees/agent-ab69fdfb34ad680cb/caraxes/runs/doorbell-poll-2026-09-24/pass2/E-hybrid-200us/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (C-off) 0/6, v2 (D0-spin-unpinned) 0/6, v2 (D-spin-pinned) 0/6, v2 (E-hybrid-200us) 0/6
Host 1-min load average at run start, median [min-max]: v2 (C-off) 1.59 [0.950-2.19] n=6, v2 (D0-spin-unpinned) 2.89 [2.26-3.17] n=6, v2 (D-spin-pinned) 2.54 [1.90-3.25] n=6, v2 (E-hybrid-200us) 2.40 [1.94-2.76] n=6

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 (C-off) | v2 (D0-spin-unpinned) | v2 (D-spin-pinned) | v2 (E-hybrid-200us) | v2 (C-off) / None | v2 (D0-spin-unpinned) / None | v2 (D-spin-pinned) / None | v2 (E-hybrid-200us) / None |
|---|---|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.026 [0.024-0.027] n=3 | 0.020 [0.014-0.088] n=3 | 0.015 [0.014-0.021] n=3 | 0.014 [0.014-0.014] n=3 | - | - | - | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 (C-off) | v2 (D0-spin-unpinned) | v2 (D-spin-pinned) | v2 (E-hybrid-200us) | v2 (C-off) / None | v2 (D0-spin-unpinned) / None | v2 (D-spin-pinned) / None | v2 (E-hybrid-200us) / None |
|---|---|---|---|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=3 | 56.9 [56.9-56.9] n=3 | 56.9 [56.9-56.9] n=3 | 56.9 [56.9-56.9] n=3 | - | - | - | - |
| process wall (incl. torch import) | s (low is better) | 11.5 [10.9-16.0] n=3 | 11.3 [10.7-16.2] n=3 | 11.5 [11.5-15.5] n=3 | 11.0 [10.9-14.8] n=3 | - | - | - | - |
| stage4 (50 training episodes) | s (low is better) | 2.70 [2.50-2.80] n=3 | 2.30 [2.10-2.30] n=3 | 2.10 [2.10-2.10] n=3 | 2.30 [2.00-2.30] n=3 | - | - | - | - |
| H2D+D2H transfers per second | 1/s (high is better) | 1062 [1019-1130] n=3 | 1270 [1239-1357] n=3 | 1375 [1340-1401] n=3 | 1257 [1227-1425] n=3 | - | - | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (C-off) | v2 (D0-spin-unpinned) | v2 (D-spin-pinned) | v2 (E-hybrid-200us) | v2 (C-off) / None | v2 (D0-spin-unpinned) / None | v2 (D-spin-pinned) / None | v2 (E-hybrid-200us) / None |
|---|---|---|---|---|---|---|---|---|---|
| rl device CPU | ms (low is better) | 4590 [4580-4780] n=3 | 14070 [14040-19240] n=3 | 15400 [15090-19240] n=3 | 5640 [5580-5700] n=3 | - | - | - | - |
| rl VMM CPU | ms (low is better) | 20440 [20170-29020] n=3 | 20150 [9060-29890] n=3 | 20910 [20770-29320] n=3 | 19800 [19600-26590] n=3 | - | - | - | - |
| short-auto device CPU | ms (low is better) | 3290 [2110-3290] n=3 | 7910 [6460-8320] n=3 | 7990 [6390-8250] n=3 | 3170 [1900-3390] n=3 | - | - | - | - |
| short-auto VMM CPU | ms (low is better) | 7610 [3170-7850] n=3 | 4080 [3070-8260] n=3 | 7720 [3040-7980] n=3 | 6910 [2940-7040] n=3 | - | - | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (C-off) | v2 (D0-spin-unpinned) | v2 (D-spin-pinned) | v2 (E-hybrid-200us) | v2 (C-off) / None | v2 (D0-spin-unpinned) / None | v2 (D-spin-pinned) / None | v2 (E-hybrid-200us) / None |
|---|---|---|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 9222 [9222-9222] n=6 | 9222 [9222-9222] n=6 | 9222 [9222-9222] n=6 | 9222 [9222-9222] n=6 | - | - | - | - |
| device RSS after run | MiB (low is better) | 5131 [5130-5131] n=6 | 5131 [5130-5131] n=6 | 5131 [5130-5131] n=6 | 5131 [5130-5131] n=6 | - | - | - | - |


Sources: v2 (A-p21-trapped)=/home/silas/git/Caraxes/caraxes/runs/doorbell-tokens-gpu-2026-09-24/A-p21-trapped/v2, v2 (C-p21-tokens)=/home/silas/git/Caraxes/caraxes/runs/doorbell-tokens-gpu-2026-09-24/C-p21-tokens/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (A-p21-trapped) 0/6, v2 (C-p21-tokens) 6/6
Host 1-min load average at run start, median [min-max]: v2 (A-p21-trapped) 7.23 [2.57-7.74] n=6, v2 (C-p21-tokens) 4.66 [3.74-4.81] n=6

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 (A-p21-trapped) | v2 (C-p21-tokens) | v2 (A-p21-trapped) / None | v2 (C-p21-tokens) / None |
|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.058 [0.050-0.280] n=3 | 0.024 [0.018-0.053] n=3 | - | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 (A-p21-trapped) | v2 (C-p21-tokens) | v2 (A-p21-trapped) / None | v2 (C-p21-tokens) / None |
|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=3 | 56.9 [56.9-56.9] n=3 | - | - |
| process wall (incl. torch import) | s (low is better) | 22.2 [18.4-33.6] n=3 | 13.5 [11.9-19.6] n=3 | - | - |
| stage4 (50 training episodes) | s (low is better) | 11.2 [8.60-11.8] n=3 | 3.00 [2.90-3.00] n=3 | - | - |
| H2D+D2H transfers per second | 1/s (high is better) | 257 [244-334] n=3 | 973 [952-994] n=3 | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (A-p21-trapped) | v2 (C-p21-tokens) | v2 (A-p21-trapped) / None | v2 (C-p21-tokens) / None |
|---|---|---|---|---|---|
| rl device CPU | ms (low is better) | 6250 [5580-7310] n=3 | 5000 [4410-5610] n=3 | - | - |
| rl VMM CPU | ms (low is better) | 33750 [27570-45600] n=3 | 22280 [20870-28370] n=3 | - | - |
| short-auto device CPU | ms (low is better) | 2990 [2560-3320] n=3 | 2720 [2160-3190] n=3 | - | - |
| short-auto VMM CPU | ms (low is better) | 7810 [3780-8690] n=3 | 7990 [3290-8260] n=3 | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (A-p21-trapped) | v2 (C-p21-tokens) | v2 (A-p21-trapped) / None | v2 (C-p21-tokens) / None |
|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 8197 [8197-8197] n=6 | 8197 [8197-8197] n=6 | - | - |
| device RSS after run | MiB (low is better) | 5130 [5128-5130] n=6 | 5131 [5129-5131] n=6 | - | - |


Sources: v2 (A-off)=/home/silas/git/Caraxes/caraxes/runs/p21-bar1-2026-09-24/A-off/v2, v2 (B-on)=/home/silas/git/Caraxes/caraxes/runs/p21-bar1-2026-09-24/B-on/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (A-off) 0/4, v2 (B-on) 0/4
Host 1-min load average at run start, median [min-max]: v2 (A-off) 4.31 [3.28-4.81] n=4, v2 (B-on) 3.94 [3.62-4.36] n=4

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 (A-off) | v2 (B-on) | v2 (A-off) / None | v2 (B-on) / None |
|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.157 [0.153-0.161] n=2 | 0.100 [0.077-0.122] n=2 | - | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 (A-off) | v2 (B-on) | v2 (A-off) / None | v2 (B-on) / None |
|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=2 | 56.9 [56.9-56.9] n=2 | - | - |
| process wall (incl. torch import) | s (low is better) | 34.5 [29.8-39.2] n=2 | 23.4 [20.2-26.7] n=2 | - | - |
| stage4 (50 training episodes) | s (low is better) | 19.0 [18.3-19.6] n=2 | 9.30 [9.00-9.60] n=2 | - | - |
| H2D+D2H transfers per second | 1/s (high is better) | 152 [147-157] n=2 | 308 [299-318] n=2 | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (A-off) | v2 (B-on) | v2 (A-off) / None | v2 (B-on) / None |
|---|---|---|---|---|---|
| rl device CPU | ms (low is better) | 10455 [9810-11100] n=2 | 6335 [6150-6520] n=2 | - | - |
| rl VMM CPU | ms (low is better) | 55010 [47500-62520] n=2 | 38875 [32590-45160] n=2 | - | - |
| short-auto device CPU | ms (low is better) | 2945 [2330-3560] n=2 | 2645 [2190-3100] n=2 | - | - |
| short-auto VMM CPU | ms (low is better) | 7545 [5920-9170] n=2 | 5755 [3140-8370] n=2 | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (A-off) | v2 (B-on) | v2 (A-off) / None | v2 (B-on) / None |
|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 6148 [6148-6148] n=4 | 6149 [6149-6149] n=4 | - | - |
| device RSS after run | MiB (low is better) | 5130 [5129-5130] n=4 | 5131 [5130-5131] n=4 | - | - |


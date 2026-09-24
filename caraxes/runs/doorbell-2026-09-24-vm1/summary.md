Sources: v2 (A-off)=/home/silas/git/Caraxes/caraxes/runs/doorbell-2026-09-24-vm1/A-off/v2, v2 (B-on)=/home/silas/git/Caraxes/caraxes/runs/doorbell-2026-09-24-vm1/B-on/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (A-off) 0/4, v2 (B-on) 0/4
Host 1-min load average at run start, median [min-max]: v2 (A-off) 3.34 [2.77-4.23] n=4, v2 (B-on) 3.56 [2.50-4.77] n=4

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 (A-off) | v2 (B-on) | v2 (A-off) / None | v2 (B-on) / None |
|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.167 [0.159-0.176] n=2 | 0.214 [0.189-0.239] n=2 | - | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 (A-off) | v2 (B-on) | v2 (A-off) / None | v2 (B-on) / None |
|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=2 | 56.9 [56.9-56.9] n=2 | - | - |
| process wall (incl. torch import) | s (low is better) | 33.5 [30.7-36.3] n=2 | 36.3 [34.4-38.2] n=2 | - | - |
| stage4 (50 training episodes) | s (low is better) | 20.4 [20.0-20.8] n=2 | 22.8 [22.8-22.8] n=2 | - | - |
| H2D+D2H transfers per second | 1/s (high is better) | 141 [138-144] n=2 | 126 [126-126] n=2 | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (A-off) | v2 (B-on) | v2 (A-off) / None | v2 (B-on) / None |
|---|---|---|---|---|---|
| rl device CPU | ms (low is better) | 11215 [11010-11420] n=2 | 14510 [14340-14680] n=2 | - | - |
| rl VMM CPU | ms (low is better) | 54270 [50300-58240] n=2 | 53430 [50540-56320] n=2 | - | - |
| short-auto device CPU | ms (low is better) | 2665 [2140-3190] n=2 | 2620 [2120-3120] n=2 | - | - |
| short-auto VMM CPU | ms (low is better) | 5800 [3220-8380] n=2 | 5765 [3150-8380] n=2 | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (A-off) | v2 (B-on) | v2 (A-off) / None | v2 (B-on) / None |
|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 6149 [6149-6149] n=4 | 6149 [6149-6149] n=4 | - | - |
| device RSS after run | MiB (low is better) | 5130 [5129-5130] n=4 | 5131 [5130-5131] n=4 | - | - |


Sources: v2 (A-off)=/home/silas/git/Caraxes/caraxes/runs/doorbell-2026-09-24-vm1-watch/A-off/v2, v2 (B-on)=/home/silas/git/Caraxes/caraxes/runs/doorbell-2026-09-24-vm1-watch/B-on/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (A-off) 0/0, v2 (B-on) 0/4
Host 1-min load average at run start, median [min-max]: v2 (A-off) n/a, v2 (B-on) 5.04 [3.68-6.27] n=4

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 (A-off) | v2 (B-on) | v2 (A-off) / None | v2 (B-on) / None |
|---|---|---|---|---|---|
| ms/launch | ms (low is better) | n/a | 0.148 [0.144-0.153] n=2 | - | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 (A-off) | v2 (B-on) | v2 (A-off) / None | v2 (B-on) / None |
|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | n/a | 56.9 [56.9-56.9] n=2 | - | - |
| process wall (incl. torch import) | s (low is better) | n/a | 37.6 [31.3-43.9] n=2 | - | - |
| stage4 (50 training episodes) | s (low is better) | n/a | 20.5 [18.8-22.2] n=2 | - | - |
| H2D+D2H transfers per second | 1/s (high is better) | n/a | 141 [129-153] n=2 | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (A-off) | v2 (B-on) | v2 (A-off) / None | v2 (B-on) / None |
|---|---|---|---|---|---|
| rl device CPU | ms (low is better) | n/a | 12375 [11210-13540] n=2 | - | - |
| rl VMM CPU | ms (low is better) | n/a | 56590 [48320-64860] n=2 | - | - |
| short-auto device CPU | ms (low is better) | n/a | 2780 [2300-3260] n=2 | - | - |
| short-auto VMM CPU | ms (low is better) | n/a | 6765 [3480-10050] n=2 | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (A-off) | v2 (B-on) | v2 (A-off) / None | v2 (B-on) / None |
|---|---|---|---|---|---|
| device HWM | MiB (low is better) | n/a | 6149 [6149-6149] n=4 | - | - |
| device RSS after run | MiB (low is better) | n/a | 5130 [5129-5131] n=4 | - | - |


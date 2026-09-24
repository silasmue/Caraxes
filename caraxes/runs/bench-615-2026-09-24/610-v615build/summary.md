Sources: v2=/home/silas/git/Caraxes/.claude/worktrees/agent-a075385b5adb08982/caraxes/runs/bench-615-2026-09-24/610/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 8/14
Host 1-min load average at run start, median [min-max]: v2 3.06 [1.40-5.57] n=14

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 | v2 / None |
|---|---|---|---|
| copy device CPU | ms (low is better) | 0.000 [0.000-0.000] n=3 | - |
| copy VMM CPU | ms (low is better) | 370 [360-520] n=3 | - |
| init device CPU | ms (low is better) | 0.000 [0.000-0.000] n=3 | - |
| init VMM CPU | ms (low is better) | 360 [330-540] n=3 | - |
| ppo device CPU | ms (low is better) | 15.0 [10.0-20.0] n=2 | - |
| ppo VMM CPU | ms (low is better) | 6845 [3790-9900] n=2 | - |
| rl device CPU | ms (low is better) | 30.0 [30.0-30.0] n=3 | - |
| rl VMM CPU | ms (low is better) | 10990 [10920-11320] n=3 | - |
| short-auto device CPU | ms (low is better) | 0.000 [0.000-0.000] n=3 | - |
| short-auto VMM CPU | ms (low is better) | 410 [400-410] n=3 | - |

### (8) Device process memory (over all runs)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| device HWM | MiB (low is better) | 5128 [5128-5128] n=14 | - |
| device RSS after run | MiB (low is better) | 5128 [5128-5128] n=14 | - |


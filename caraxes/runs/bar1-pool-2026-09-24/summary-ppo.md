Sources: v2 (ppo)=/home/silas/git/Caraxes/caraxes/runs/bar1-pool-2026-09-24/pool/ppo/v2, v2 (ppo)=/home/silas/git/Caraxes/caraxes/runs/bar1-pool-2026-09-24/pool/ppo/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (ppo) 0/4, v2 (ppo) 0/4
Host 1-min load average at run start, median [min-max]: v2 (ppo) 3.04 [2.88-3.46] n=4, v2 (ppo) 3.04 [2.88-3.46] n=4

### (4) PPO CartPole, 128 vector environments, one H2D and one synchronous D2H per step (pporl, 60 s)

| value | unit | v2 (ppo) | v2 (ppo) | v2 (ppo) / None | v2 (ppo) / None |
|---|---|---|---|---|---|
| environment steps per second | 1/s (high is better) | 68603 [66032-71994] n=4 | 68603 [66032-71994] n=4 | - | - |
| reward100 at the end (not a time) |  | 340 [266-376] n=4 | 340 [266-376] n=4 | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (ppo) | v2 (ppo) | v2 (ppo) / None | v2 (ppo) / None |
|---|---|---|---|---|---|
| ppo device CPU | ms (low is better) | 74150 [72720-74950] n=4 | 74150 [72720-74950] n=4 | - | - |
| ppo VMM CPU | ms (low is better) | 91310 [66660-114940] n=4 | 91310 [66660-114940] n=4 | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (ppo) | v2 (ppo) | v2 (ppo) / None | v2 (ppo) / None |
|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 9222 [9222-9222] n=4 | 9222 [9222-9222] n=4 | - | - |
| device RSS after run | MiB (low is better) | 5140 [5139-5142] n=4 | 5140 [5139-5142] n=4 | - | - |


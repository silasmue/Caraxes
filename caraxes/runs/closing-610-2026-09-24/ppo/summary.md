Sources: native=/home/silas/git/Caraxes/caraxes/runs/closing-610-2026-09-24/ppo/native, v1=/home/silas/git/Caraxes/caraxes/runs/closing-610-2026-09-24/ppo/v1, v2=/home/silas/git/Caraxes/caraxes/runs/closing-610-2026-09-24/ppo/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): native 2/2, v1 2/2, v2 0/2
Host 1-min load average at run start, median [min-max]: native 5.20 [4.98-5.41] n=2, v1 4.32 [3.70-4.94] n=2, v2 4.62 [3.00-6.23] n=2

### (4) PPO CartPole, 128 vector environments, one H2D and one synchronous D2H per step (pporl, 60 s)

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| environment steps per second | 1/s (high is better) | 109961 [107663-112259] n=2 | 76260 [56592-95928] n=2 | 67030 [61082-72977] n=2 | 0.69 | 0.61 |
| reward100 at the end (not a time) |  | 306 [212-400] n=2 | 326 [255-397] n=2 | 382 [348-417] n=2 | 1.06 | 1.25 |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ppo device CPU | ms (low is better) | n/a | 380 [370-390] n=2 | 74105 [73430-74780] n=2 | - | - |
| ppo VMM CPU | ms (low is better) | n/a | 60660 [59480-61840] n=2 | 69425 [69250-69600] n=2 | - | - |

### (8) Device process memory (over all runs)

| value | unit | native | v1 | v2 | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | n/a | 560 [551-569] n=2 | 9222 [9222-9222] n=2 | - | - |
| device RSS after run | MiB (low is better) | n/a | 521 [513-529] n=2 | 5131 [5130-5131] n=2 | - | - |


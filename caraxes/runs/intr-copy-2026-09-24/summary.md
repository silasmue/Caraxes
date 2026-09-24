Sources: v2 (off)=/home/silas/git/Caraxes/caraxes/runs/intr-copy-2026-09-24/off/v2, v2 (on)=/home/silas/git/Caraxes/caraxes/runs/intr-copy-2026-09-24/on/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (off) 3/15, v2 (on) 7/15
Host 1-min load average at run start, median [min-max]: v2 (off) 4.80 [2.78-9.61] n=15, v2 (on) 4.33 [3.70-10.6] n=15

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 (off) | v2 (on) | v2 (off) / None | v2 (on) / None |
|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.232 [0.108-0.333] n=5 | 0.128 [0.118-0.343] n=5 | - | - |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v2 (off) | v2 (on) | v2 (off) / None | v2 (on) / None |
|---|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 3.89 [3.10-5.77] n=5 | 5.36 [5.28-5.94] n=5 | - | - |
| d2h pinned 1 MiB | GB/s (high is better) | 3.74 [3.29-3.99] n=5 | 5.41 [3.97-5.83] n=5 | - | - |
| d2d device 1 MiB | GB/s (high is better) | 9.99 [8.73-12.0] n=5 | 9.76 [0.045-10.2] n=5 | - | - |
| h2d pageable 1 MiB | GB/s (high is better) | 3.01 [2.62-4.11] n=5 | 4.06 [3.48-4.31] n=5 | - | - |
| d2h pageable 1 MiB | GB/s (high is better) | 2.82 [2.22-3.23] n=5 | 3.78 [2.48-3.86] n=5 | - | - |
| h2d pinned 16 MiB | GB/s (high is better) | 6.29 [6.18-10.9] n=5 | 11.9 [11.8-12.0] n=5 | - | - |
| d2h pinned 16 MiB | GB/s (high is better) | 6.31 [5.97-11.4] n=5 | 12.0 [11.8-12.2] n=5 | - | - |
| d2d device 16 MiB | GB/s (high is better) | 14.6 [11.9-55.5] n=5 | 90.1 [0.731-90.6] n=5 | - | - |
| h2d pageable 16 MiB | GB/s (high is better) | 4.30 [3.02-5.27] n=5 | 5.11 [4.41-5.16] n=5 | - | - |
| d2h pageable 16 MiB | GB/s (high is better) | 3.41 [2.81-4.45] n=5 | 4.76 [2.88-4.99] n=5 | - | - |
| h2d pinned 256 MiB | GB/s (high is better) | 12.9 [12.8-12.9] n=5 | 12.9 [7.66-13.0] n=5 | - | - |
| d2h pinned 256 MiB | GB/s (high is better) | 13.0 [12.9-13.1] n=5 | 13.1 [13.0-13.1] n=5 | - | - |
| d2d device 256 MiB | GB/s (high is better) | 178 [164-181] n=5 | 175 [154-176] n=5 | - | - |
| h2d pageable 256 MiB | GB/s (high is better) | 3.81 [2.98-4.48] n=5 | 3.88 [2.56-4.66] n=5 | - | - |
| d2h pageable 256 MiB | GB/s (high is better) | 3.90 [2.79-4.25] n=5 | 4.10 [3.72-4.73] n=5 | - | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 (off) | v2 (on) | v2 (off) / None | v2 (on) / None |
|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | - | - |
| process wall (incl. torch import) | s (low is better) | 34.0 [26.0-49.0] n=5 | 29.4 [27.3-56.1] n=5 | - | - |
| stage4 (50 training episodes) | s (low is better) | 18.7 [13.5-21.0] n=5 | 14.7 [13.3-38.7] n=5 | - | - |
| H2D+D2H transfers per second | 1/s (high is better) | 154 [137-214] n=5 | 195 [74.0-217] n=5 | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (off) | v2 (on) | v2 (off) / None | v2 (on) / None |
|---|---|---|---|---|---|
| copy device CPU | ms (low is better) | 8960 [8430-9780] n=5 | 8730 [8230-9460] n=5 | - | - |
| copy VMM CPU | ms (low is better) | 14160 [12840-16090] n=5 | 11360 [11130-13850] n=5 | - | - |
| rl device CPU | ms (low is better) | 21600 [19990-23990] n=5 | 20830 [20240-26670] n=5 | - | - |
| rl VMM CPU | ms (low is better) | 25510 [22930-30970] n=5 | 23280 [22100-31920] n=5 | - | - |
| short-auto device CPU | ms (low is better) | 2930 [2800-3460] n=5 | 3240 [3050-3900] n=5 | - | - |
| short-auto VMM CPU | ms (low is better) | 3180 [2800-3840] n=5 | 2670 [2440-3630] n=5 | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (off) | v2 (on) | v2 (off) / None | v2 (on) / None |
|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 5142 [5142-5143] n=15 | 5142 [5142-5143] n=15 | - | - |
| device RSS after run | MiB (low is better) | 5142 [5142-5143] n=15 | 5142 [5141-5143] n=15 | - | - |


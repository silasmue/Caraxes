Sources: v2 (pool)=caraxes/runs/opt-memfd-pool-2026-09-25/off/copy/v2, v2 (pool+memfd)=caraxes/runs/opt-memfd-pool-2026-09-25/on/copy/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (pool) 3/3, v2 (pool+memfd) 3/3
Host 1-min load average at run start, median [min-max]: v2 (pool) 8.38 [7.41-9.32] n=3, v2 (pool+memfd) 8.40 [8.21-8.69] n=3

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v2 (pool) | v2 (pool+memfd) | v2 (pool) / None | v2 (pool+memfd) / None |
|---|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 6.05 [2.99-11.2] n=3 | 6.04 [5.87-6.05] n=3 | - | - |
| d2h pinned 1 MiB | GB/s (high is better) | 6.16 [3.06-11.4] n=3 | 6.13 [6.13-6.17] n=3 | - | - |
| d2d device 1 MiB | GB/s (high is better) | 46.9 [0.052-63.3] n=3 | 57.0 [55.6-65.0] n=3 | - | - |
| h2d pageable 1 MiB | GB/s (high is better) | 4.46 [2.62-6.94] n=3 | 4.35 [4.20-4.39] n=3 | - | - |
| d2h pageable 1 MiB | GB/s (high is better) | 4.09 [2.47-5.74] n=3 | 3.75 [3.75-3.86] n=3 | - | - |
| h2d pinned 16 MiB | GB/s (high is better) | 6.54 [6.54-12.9] n=3 | 6.50 [6.47-6.52] n=3 | - | - |
| d2h pinned 16 MiB | GB/s (high is better) | 6.60 [6.58-13.0] n=3 | 6.55 [6.50-6.56] n=3 | - | - |
| d2d device 16 MiB | GB/s (high is better) | 16.2 [0.834-16.5] n=3 | 13.9 [12.2-14.2] n=3 | - | - |
| h2d pageable 16 MiB | GB/s (high is better) | 6.19 [6.15-8.19] n=3 | 3.16 [2.67-3.49] n=3 | - | - |
| d2h pageable 16 MiB | GB/s (high is better) | 8.10 [7.59-8.41] n=3 | 3.27 [3.12-3.47] n=3 | - | - |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [13.0-13.1] n=3 | 13.1 [13.0-13.1] n=3 | - | - |
| d2h pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.2] n=3 | 13.1 [13.0-13.1] n=3 | - | - |
| d2d device 256 MiB | GB/s (high is better) | 190 [12.2-190] n=3 | 187 [167-189] n=3 | - | - |
| h2d pageable 256 MiB | GB/s (high is better) | 8.09 [7.93-8.56] n=3 | 3.72 [2.54-3.88] n=3 | - | - |
| d2h pageable 256 MiB | GB/s (high is better) | 8.17 [7.96-8.32] n=3 | 3.67 [3.42-3.80] n=3 | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (pool) | v2 (pool+memfd) | v2 (pool) / None | v2 (pool+memfd) / None |
|---|---|---|---|---|---|
| copy device CPU | ms (low is better) | 10870 [8750-11760] n=3 | 2570 [2460-3890] n=3 | - | - |
| copy VMM CPU | ms (low is better) | 30540 [28600-35260] n=3 | 3380 [3280-3490] n=3 | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (pool) | v2 (pool+memfd) | v2 (pool) / None | v2 (pool+memfd) / None |
|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 8199 [8199-8199] n=3 | 8198 [8198-8198] n=3 | - | - |
| device RSS after run | MiB (low is better) | 5137 [5137-5137] n=3 | 4094 [4094-4094] n=3 | - | - |


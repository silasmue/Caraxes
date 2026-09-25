Sources: v2 (pool)=caraxes/runs/opt-memfd-pool-2026-09-25/off-2/copy/v2, v2 (pool+memfd)=caraxes/runs/opt-memfd-pool-2026-09-25/on-2/copy/v2, v2 (pool+memfd+rearm)=caraxes/runs/opt-msi-ioeventfd-2026-09-25/on-rearm-2/copy/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (pool) 6/8, v2 (pool+memfd) 0/8, v2 (pool+memfd+rearm) 0/8
Host 1-min load average at run start, median [min-max]: v2 (pool) 3.54 [2.44-3.67] n=8, v2 (pool+memfd) 1.98 [1.74-2.16] n=8, v2 (pool+memfd+rearm) 1.83 [1.55-1.88] n=8

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 (pool) | v2 (pool+memfd) | v2 (pool+memfd+rearm) | v2 (pool) / None | v2 (pool+memfd) / None | v2 (pool+memfd+rearm) / None |
|---|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 2.51 [0.021-21.1] n=4 | 0.021 [0.017-0.047] n=4 | 0.020 [0.013-0.027] n=4 | - | - | - |

### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v2 (pool) | v2 (pool+memfd) | v2 (pool+memfd+rearm) | v2 (pool) / None | v2 (pool+memfd) / None | v2 (pool+memfd+rearm) / None |
|---|---|---|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 11.3 [5.97-11.5] n=4 | 6.06 [5.98-6.08] n=4 | 6.08 [5.99-6.10] n=4 | - | - | - |
| d2h pinned 1 MiB | GB/s (high is better) | 11.5 [6.07-11.7] n=4 | 6.19 [6.10-6.21] n=4 | 6.17 [6.07-6.19] n=4 | - | - | - |
| d2d device 1 MiB | GB/s (high is better) | 30.5 [0.050-90.6] n=4 | 64.5 [59.6-68.3] n=4 | 64.5 [61.0-67.3] n=4 | - | - | - |
| h2d pageable 1 MiB | GB/s (high is better) | 7.03 [4.30-7.07] n=4 | 4.49 [4.43-4.54] n=4 | 4.47 [4.11-4.52] n=4 | - | - | - |
| d2h pageable 1 MiB | GB/s (high is better) | 6.02 [3.74-6.26] n=4 | 4.06 [3.98-4.19] n=4 | 3.86 [3.64-4.10] n=4 | - | - | - |
| h2d pinned 16 MiB | GB/s (high is better) | 12.8 [6.55-12.9] n=4 | 6.56 [6.55-6.57] n=4 | 6.56 [6.54-6.56] n=4 | - | - | - |
| d2h pinned 16 MiB | GB/s (high is better) | 12.9 [6.51-13.0] n=4 | 6.60 [6.57-7.50] n=4 | 6.59 [6.58-6.61] n=4 | - | - | - |
| d2d device 16 MiB | GB/s (high is better) | 5.74 [0.784-16.7] n=4 | 16.9 [15.9-28.9] n=4 | 15.8 [15.7-16.5] n=4 | - | - | - |
| h2d pageable 16 MiB | GB/s (high is better) | 9.62 [5.38-10.3] n=4 | 7.19 [6.14-10.1] n=4 | 6.28 [6.27-6.30] n=4 | - | - | - |
| d2h pageable 16 MiB | GB/s (high is better) | 8.46 [3.62-8.85] n=4 | 7.25 [6.05-8.48] n=4 | 8.35 [7.43-8.68] n=4 | - | - | - |
| h2d pinned 256 MiB | GB/s (high is better) | 13.0 [12.9-13.1] n=4 | 13.1 [13.1-13.1] n=4 | 13.1 [13.1-13.1] n=4 | - | - | - |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [13.1-13.2] n=4 | 13.2 [13.2-13.2] n=4 | 13.2 [13.2-13.2] n=4 | - | - | - |
| d2d device 256 MiB | GB/s (high is better) | 177 [12.4-187] n=4 | 192 [189-193] n=4 | 192 [191-193] n=4 | - | - | - |
| h2d pageable 256 MiB | GB/s (high is better) | 6.56 [4.29-7.96] n=4 | 8.85 [8.38-8.99] n=4 | 8.48 [7.05-9.24] n=4 | - | - | - |
| d2h pageable 256 MiB | GB/s (high is better) | 7.62 [4.67-8.26] n=4 | 8.64 [8.24-8.66] n=4 | 8.41 [6.59-8.74] n=4 | - | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (pool) | v2 (pool+memfd) | v2 (pool+memfd+rearm) | v2 (pool) / None | v2 (pool+memfd) / None | v2 (pool+memfd+rearm) / None |
|---|---|---|---|---|---|---|---|
| copy device CPU | ms (low is better) | 11420 [10100-11780] n=4 | 3525 [3500-3570] n=4 | 3490 [3420-3600] n=4 | - | - | - |
| copy VMM CPU | ms (low is better) | 32520 [31240-34990] n=4 | 6315 [5300-6510] n=4 | 7225 [6100-7690] n=4 | - | - | - |
| short-auto device CPU | ms (low is better) | 2535 [2270-2990] n=4 | 555 [520-560] n=4 | 525 [510-560] n=4 | - | - | - |
| short-auto VMM CPU | ms (low is better) | 8535 [8060-12300] n=4 | 1115 [1050-1180] n=4 | 1145 [1100-1190] n=4 | - | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (pool) | v2 (pool+memfd) | v2 (pool+memfd+rearm) | v2 (pool) / None | v2 (pool+memfd) / None | v2 (pool+memfd+rearm) / None |
|---|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 8199 [8199-8199] n=8 | 8199 [8199-8199] n=8 | 8199 [8199-8199] n=8 | - | - | - |
| device RSS after run | MiB (low is better) | 5140 [5132-5142] n=8 | 5143 [5133-5144] n=8 | 5143 [5133-5144] n=8 | - | - | - |


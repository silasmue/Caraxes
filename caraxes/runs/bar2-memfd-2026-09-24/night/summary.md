<!-- SPDX-License-Identifier: MIT -->
# BAR2 memfd, night part: summary

Columns: off (interrupt page on, BAR1 pool, memfd off); off-nopool = alloc-path-2026-09-24/night/intr (interrupt page on, no pool, memfd off, the same configuration recorded by the allocation part); on-nopool (interrupt page on, no pool, memfd on). on, on-syncdb and on-nointr (memfd with the pool) hung in the first cuCtxCreate and have no series. Generated with caraxes/tools/bench-summary.py.

Sources: v2 (off)=bar2-memfd-2026-09-24/night/off/v2, v2 (off-nopool)=alloc-path-2026-09-24/night/intr/v2, v2 (on-nopool)=bar2-memfd-2026-09-24/night/on-nopool/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (off) 4/10, v2 (off-nopool) 5/10, v2 (on-nopool) 8/10
Host 1-min load average at run start, median [min-max]: v2 (off) 6.96 [4.80-9.47] n=10, v2 (off-nopool) 3.90 [2.56-6.01] n=10, v2 (on-nopool) 7.92 [6.95-11.9] n=10

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v2 (off) | v2 (off-nopool) | v2 (on-nopool) | v2 (off) / None | v2 (off-nopool) / None | v2 (on-nopool) / None |
|---|---|---|---|---|---|---|---|
| cuInit | ms (low is better) | 187 [167-446] n=5 | 196 [144-240] n=5 | 71.6 [13.7-200] n=5 | - | - | - |
| cuCtxCreate | ms (low is better) | 3793 [2930-11185] n=5 | 3863 [3129-4900] n=5 | 1077 [763-1726] n=5 | - | - | - |
| cuCtxDestroy | ms (low is better) | 1462 [877-4920] n=5 | 1166 [946-1403] n=5 | 391 [149-726] n=5 | - | - | - |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | v2 (off) | v2 (off-nopool) | v2 (on-nopool) | v2 (off) / None | v2 (off-nopool) / None | v2 (on-nopool) / None |
|---|---|---|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 194 [182-347] n=5 | 581 [199-1138] n=4 | 284 [228-391] n=5 | - | - | - |
| 64x1 MiB cuMemFree | us (low is better) | 251 [247-464] n=5 | 680 [234-1680] n=4 | 561 [349-703] n=5 | - | - | - |
| 64x1 MiB cycle | ms (low is better) | 47.6 [40.9-60.9] n=5 | 102 [37.0-191] n=4 | 64.7 [43.5-110] n=5 | - | - | - |
| 8x64 MiB cuMemAlloc | us (low is better) | 5174 [4544-13151] n=5 | 5590 [4605-9301] n=4 | 892 [780-4826] n=5 | - | - | - |
| 8x64 MiB cuMemFree | us (low is better) | 3372 [2679-9568] n=5 | 3945 [2694-6222] n=4 | 1347 [1206-6930] n=5 | - | - | - |
| 8x64 MiB cycle | ms (low is better) | 113 [88.7-265] n=5 | 113 [86.9-172] n=4 | 24.3 [21.6-100] n=5 | - | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (off) | v2 (off-nopool) | v2 (on-nopool) | v2 (off) / None | v2 (off-nopool) / None | v2 (on-nopool) / None |
|---|---|---|---|---|---|---|---|
| churn device CPU | ms (low is better) | 7050 [5840-14030] n=5 | 4960 [690-9040] n=5 | 3910 [2390-4940] n=5 | - | - | - |
| churn VMM CPU | ms (low is better) | 14090 [12290-19910] n=5 | 11610 [2420-16800] n=5 | 4780 [3140-6420] n=5 | - | - | - |
| init device CPU | ms (low is better) | 2920 [2320-10000] n=5 | 2320 [2200-3410] n=5 | 720 [600-1180] n=5 | - | - | - |
| init VMM CPU | ms (low is better) | 8590 [2890-13890] n=5 | 6780 [4540-8390] n=5 | 1190 [650-1930] n=5 | - | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (off) | v2 (off-nopool) | v2 (on-nopool) | v2 (off) / None | v2 (off-nopool) / None | v2 (on-nopool) / None |
|---|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 7175 [7175-8199] n=10 | 8199 [6151-8199] n=10 | 8199 [8199-8199] n=10 | - | - | - |
| device RSS after run | MiB (low is better) | 5132 [5090-5133] n=10 | 5132 [5131-5132] n=10 | 5133 [5132-5135] n=10 | - | - | - |


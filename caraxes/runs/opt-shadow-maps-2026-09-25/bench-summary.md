Sources: v2 (before)=before/v2, v2 (defer-pool)=defer-pool/v2, v2 (final)=final/v2, v2 (gather)=gather/v2, v2 (gather-2)=gather-2/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 (before) 0/6, v2 (defer-pool) 0/6, v2 (final) 0/6, v2 (gather) 6/6, v2 (gather-2) 0/6
Host 1-min load average at run start, median [min-max]: v2 (before) 4.01 [3.77-4.03] n=6, v2 (defer-pool) 5.02 [4.86-5.11] n=6, v2 (final) 1.24 [1.17-1.38] n=6, v2 (gather) 2.49 [2.49-2.69] n=6, v2 (gather-2) 1.76 [1.63-1.90] n=6

### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v2 (before) | v2 (defer-pool) | v2 (final) | v2 (gather) | v2 (gather-2) | v2 (before) / None | v2 (defer-pool) / None | v2 (final) / None | v2 (gather) / None | v2 (gather-2) / None |
|---|---|---|---|---|---|---|---|---|---|---|---|
| cuInit | ms (low is better) | 16.6 [14.7-35.5] n=3 | 17.1 [16.0-18.9] n=3 | 532 [21.8-583] n=3 | 15.7 [14.3-22.4] n=3 | 16.3 [13.6-19.4] n=3 | - | - | - | - | - |
| cuCtxCreate | ms (low is better) | 1719 [612-1879] n=3 | 1513 [592-1824] n=3 | 675 [508-902] n=3 | 307 [266-455] n=3 | 353 [339-396] n=3 | - | - | - | - | - |
| cuCtxDestroy | ms (low is better) | 193 [144-338] n=3 | 197 [156-332] n=3 | 172 [106-211] n=3 | 125 [120-184] n=3 | 151 [135-154] n=3 | - | - | - | - | - |

### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | v2 (before) | v2 (defer-pool) | v2 (final) | v2 (gather) | v2 (gather-2) | v2 (before) / None | v2 (defer-pool) / None | v2 (final) / None | v2 (gather) / None | v2 (gather-2) / None |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 180 [178-184] n=3 | 180 [175-183] n=3 | 182 [180-183] n=3 | 342 [201-354] n=3 | 177 [174-179] n=3 | - | - | - | - | - |
| 64x1 MiB cuMemFree | us (low is better) | 323 [306-324] n=3 | 303 [296-564] n=3 | 296 [290-304] n=3 | 349 [323-558] n=3 | 303 [286-483] n=3 | - | - | - | - | - |
| 64x1 MiB cycle | ms (low is better) | 34.2 [33.7-35.4] n=3 | 34.5 [34.1-37.3] n=3 | 33.7 [32.9-34.2] n=3 | 39.5 [37.7-42.6] n=3 | 34.0 [33.5-43.4] n=3 | - | - | - | - | - |
| 8x64 MiB cuMemAlloc | us (low is better) | 583 [533-584] n=3 | 591 [514-666] n=3 | 553 [540-594] n=3 | 567 [558-623] n=3 | 594 [541-741] n=3 | - | - | - | - | - |
| 8x64 MiB cuMemFree | us (low is better) | 848 [845-902] n=3 | 899 [808-969] n=3 | 830 [823-892] n=3 | 896 [864-980] n=3 | 887 [823-1114] n=3 | - | - | - | - | - |
| 8x64 MiB cycle | ms (low is better) | 11.9 [11.8-13.7] n=3 | 14.0 [11.6-15.0] n=3 | 12.0 [11.4-13.1] n=3 | 12.7 [12.4-14.1] n=3 | 13.0 [12.0-16.0] n=3 | - | - | - | - | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 (before) | v2 (defer-pool) | v2 (final) | v2 (gather) | v2 (gather-2) | v2 (before) / None | v2 (defer-pool) / None | v2 (final) / None | v2 (gather) / None | v2 (gather-2) / None |
|---|---|---|---|---|---|---|---|---|---|---|---|
| churn device CPU | ms (low is better) | 2180 [2060-2800] n=3 | 2000 [2000-2930] n=3 | 2120 [1930-2200] n=3 | 1640 [1520-1680] n=3 | 1700 [1630-1860] n=3 | - | - | - | - | - |
| churn VMM CPU | ms (low is better) | 1050 [1050-1110] n=3 | 1060 [1060-1160] n=3 | 1020 [1000-1080] n=3 | 1180 [1090-1290] n=3 | 1040 [1030-1270] n=3 | - | - | - | - | - |
| init device CPU | ms (low is better) | 1970 [1140-2250] n=3 | 1820 [1190-2150] n=3 | 1710 [600-1840] n=3 | 400 [350-550] n=3 | 480 [430-530] n=3 | - | - | - | - | - |
| init VMM CPU | ms (low is better) | 340 [340-540] n=3 | 360 [350-370] n=3 | 330 [330-370] n=3 | 340 [330-390] n=3 | 320 [320-350] n=3 | - | - | - | - | - |

### (8) Device process memory (over all runs)

| value | unit | v2 (before) | v2 (defer-pool) | v2 (final) | v2 (gather) | v2 (gather-2) | v2 (before) / None | v2 (defer-pool) / None | v2 (final) / None | v2 (gather) / None | v2 (gather-2) / None |
|---|---|---|---|---|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 8199 [8199-8199] n=6 | 8199 [8199-8199] n=6 | 8199 [8199-8199] n=6 | 8199 [8199-8199] n=6 | 8199 [8199-8199] n=6 | - | - | - | - | - |
| device RSS after run | MiB (low is better) | 5137 [5135-5137] n=6 | 5137 [5136-5137] n=6 | 5135 [5134-5135] n=6 | 5133 [5132-5133] n=6 | 5134 [5133-5134] n=6 | - | - | - | - | - |


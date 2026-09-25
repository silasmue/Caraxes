## Census table

Traps per operation, gross and net of the background (idle windows: on 0.39 per ms, on-rearm 0.26 per ms). Wall time is the guest's median (census-cu, both ops runs pooled), native the host's median over the same probe.

| operation | traps on (net) | traps on-rearm (net) | covered in bare by: intr / memfd / tokens / pool | wall on, us | wall on-rearm, us | native, us | off / native |
|---|---|---|---|---|---|---|---|
| cuInit | 174.3 (163.4) | 174.3 (169.5) | - | 28173.6 | 18786.6 | 93697.2 | - |
| cuCtxCreate | 1228.3 (993.0) | 1189.7 (1051.0) | - | 610661 | 541560 | 146463 | - |
| cuCtxDestroy | 925.7 (846.7) | 1067.0 (1013.9) | - | 204755 | 207534 | 86189.9 | - |
| cuMemAlloc 1 MiB | 3.8 (3.6) | 3.5 (3.4) | - | 387.5 | 340.4 | 65.0 | - |
| cuMemFree 1 MiB | 6.6 (6.4) | 6.5 (6.4) | - | 481.6 | 469.0 | 51.0 | - |
| cuMemAlloc 64 MiB | 8.2 (7.8) | 7.8 (7.5) | - | 970.2 | 1128.0 | 131.9 | - |
| cuMemFree 64 MiB | 13.3 (12.7) | 13.1 (12.7) | - | 1487.2 | 1423.9 | 172.1 | - |
| cuMemAllocHost 1 MiB | 8.2 (7.6) | 7.5 (7.1) | - | 1721.8 | 1844.7 | 758.3 | - |
| cuMemFreeHost 1 MiB | 7.7 (7.2) | 7.0 (6.6) | - | 1244.6 | 1171.2 | 323.2 | - |
| cuMemHostRegister 1 MiB | 15.4 (14.5) | 15.0 (14.5) | - | 2285.1 | 2026.9 | 283.6 | - |
| cuMemHostUnregister 1 MiB | 14.3 (13.5) | 14.0 (13.5) | - | 2043.4 | 2576.9 | 176.4 | - |
| launch + sync, single (after 30 ms idle) | 0.3 (0.0) | 0.0 (0.0) | - | 742.1 | 656.8 | 163.4 | - |
| launch + sync, 1000 back to back (per launch) | 0.0 (0.0) | 0.0 (0.0) | - | 20.4 | 20.0 | 6.3 | - |
| H2D 1 MiB pinned | 0.1 (0.0) | 0.0 (0.0) | - | 435.1 | 396.4 | 106.0 | - |
| D2H 1 MiB pinned | 1.4 (1.2) | 0.9 (0.8) | - | 402.0 | 400.2 | 108.8 | - |
| D2D 1 MiB | 0.2 (0.0) | 0.0 (0.0) | - | 802.2 | 913.5 | 175.6 | - |
| H2D 16 MiB pinned | 0.5 (0.0) | 0.0 (0.0) | - | 3927.0 | 5170.2 | 1305.2 | - |
| D2H 16 MiB pinned | 0.7 (0.0) | 0.0 (0.0) | - | 3976.9 | 5212.7 | 1292.5 | - |
| D2D 16 MiB | 0.7 (0.0) | 10.3 (9.3) | - | 3284.6 | 3909.6 | 251.4 | - |
| H2D 1 MiB pageable | 0.1 (0.0) | 0.0 (0.0) | - | 527.3 | 479.1 | 197.6 | - |
| D2H 1 MiB pageable | 0.2 (0.1) | 0.0 (0.0) | - | 432.3 | 540.3 | 191.1 | - |
| cuStreamSynchronize (idle stream) | 0.2 (0.2) | 0.0 (0.0) | - | 9.3 | 9.6 | 4.8 | - |
| cuEventRecord + cuEventSynchronize | 0.6 (0.0) | 0.0 (0.0) | - | 1369.5 | 657.8 | 151.6 | - |
| cuModuleLoad (4.6 KiB cubin) | 0.1 (0.0) | 0.0 (0.0) | - | 330.4 | 797.0 | 231.1 | - |
| cuModuleUnload | 0.0 (0.0) | 0.0 (0.0) | - | 24.8 | 23.4 | 16.2 | - |
| idle, 100 ms (background) | 38.6 (38.6) | 25.6 (25.6) | - | 100079 | 100092 | 100062 | - |

## Run to run (guest median per run, us)

| operation | ../opt-memfd-pool-2026-09-25/on | on-rearm | ../trap-census-2026-09-25/native | ../trap-census-2026-09-25/native-2 |
|---|---|---|---|---|
| launch + sync, single (after 30 ms idle) | 742.1 | 656.8 | 154.8 | 177.2 |
| launch + sync, 1000 back to back (per launch) | 20.4 | 20.0 | 6.3 | 6.5 |
| H2D 1 MiB pinned | 435.1 | 396.4 | 103.0 | 115.5 |
| D2D 1 MiB | 802.2 | 913.5 | 167.3 | 187.8 |
| H2D 16 MiB pinned | 3927.0 | 5170.2 | 1300.8 | 1310.7 |
| D2D 16 MiB | 3284.6 | 3909.6 | 244.3 | 268.9 |
| cuEventRecord + cuEventSynchronize | 1369.5 | 657.8 | 151.6 | 156.1 |
| cuModuleLoad (4.6 KiB cubin) | 330.4 | 797.0 | 231.1 | 238.9 |
| cuMemAlloc 1 MiB | 387.5 | 340.4 | 65.0 | 72.5 |
| cuCtxCreate | 610661 | 541560 | 143380 | 149546 |


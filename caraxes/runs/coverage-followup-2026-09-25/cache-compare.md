| Workload | Run | Guest controls | Device ms | Host RM controls | Host ms | Cached getters: guest controls | their host calls | their device ms |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| smi-q-timed | off | 913 | 4237.7 | 823 | 4174.7 | 93 | 93 | 154.72 |
| smi-q-timed | on | 913 | 817.6 | 742 | 809.3 | 93 | 12 | 15.07 |
| smi-query-timed | off | 500 | 1816.7 | 480 | 1805.7 | 160 | 160 | 176.66 |
| smi-query-timed | on | 500 | 267.3 | 320 | 263.1 | 160 | 0 | 0.61 |
| smi-dmon | off | 529 | 1481.8 | 499 | 1468.6 | 10 | 10 | 20.17 |
| smi-dmon | on | 529 | 1158.9 | 489 | 1150.2 | 10 | 0 | 0.03 |
on, device lifetime: forward cache: fills = 21
on, device lifetime: forward cache: hits = 311
on, device lifetime: forward cache: not kept = 5
| Workload | Run | Guest controls | Device ms | Host RM controls | Host ms | Cached getters: guest controls | their host calls | their device ms |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| smi-q-timed | off | 913 | 829.4 | 823 | 823.9 | 93 | 93 | 27.86 |
| smi-q-timed | on | 913 | 817.6 | 742 | 809.3 | 93 | 12 | 15.07 |
| smi-query-timed | off | 500 | 254.9 | 480 | 253.4 | 160 | 160 | 77.41 |
| smi-query-timed | on | 500 | 267.3 | 320 | 263.1 | 160 | 0 | 0.61 |
| smi-dmon | off | 544 | 824.2 | 514 | 821.1 | 10 | 10 | 1.17 |
| smi-dmon | on | 529 | 1158.9 | 489 | 1150.2 | 10 | 0 | 0.03 |
on, device lifetime: forward cache: fills = 21
on, device lifetime: forward cache: hits = 311
on, device lifetime: forward cache: not kept = 5

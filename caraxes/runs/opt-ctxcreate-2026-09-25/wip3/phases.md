# phases: wip3/census-trace.txt.xz

Medians over the repetitions, ms. Trapped: gaps up to the idle bound before a trapped access; builds: the device's first-doorbell handling; untrapped: the other gaps; worker: background.

## cuCtxCreate (5 repetitions)

| part | ms |
|---|---|
| window | 569.9 |
| trapped, all classes | 178.3 |
| trapped: bar0-other | 6.1 |
| trapped: bar2-flush-read | 80.8 |
| trapped: cfg-other | 0.0 |
| trapped: gsp | 35.5 |
| trapped: mmu-invalidate | 43.2 |
| trapped: usermode-doorbell | 0.0 |
| first-doorbell builds (20) | 238.7 |
| build: plan and apply | 7.1 |
| build: TSG | 10.9 |
| build: channel | 63.8 |
| build: objects | 49.5 |
| build: schedule and token | 11.0 |
| build: doorbell | 0.3 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 133.7 |
| untrapped: before a first doorbell (guest) | 7.2 |
| untrapped: before bar0-other | 0.0 |
| untrapped: before bar2-flush-read | 55.6 |
| untrapped: before cfg-other | 0.0 |
| untrapped: before gsp | 14.7 |
| untrapped: before mmu-invalidate | 47.5 |
| worker apply (background) | 52.7 |
| of it gather allocs (8) | 27.5 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 85 |

## cuCtxDestroy (5 repetitions)

| part | ms |
|---|---|
| window | 243.2 |
| trapped, all classes | 121.1 |
| trapped: bar2-flush-read | 58.5 |
| trapped: gsp | 41.1 |
| trapped: mmu-invalidate | 17.4 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 115.9 |
| untrapped: before bar2-flush-read | 16.1 |
| untrapped: before gsp | 56.2 |
| untrapped: before mmu-invalidate | 22.0 |
| worker apply (background) | 7.4 |
| of it gather allocs (0) | 0.0 |
| host channel frees or retires in RPC handling (20) | 55.1 |
| RPCs (count) | 69 |


# phases: part2/census-trace.txt.xz

Medians over the repetitions, ms. Trapped: gaps up to the idle bound before a trapped access; builds: the device's first-doorbell handling; untrapped: the other gaps; worker: background.

## cuCtxCreate (5 repetitions)

| part | ms |
|---|---|
| window | 336.7 |
| trapped, all classes | 145.6 |
| trapped: bar0-other | 4.6 |
| trapped: bar2-flush-read | 70.9 |
| trapped: cfg-other | 0.0 |
| trapped: gsp | 27.2 |
| trapped: mmu-invalidate | 34.7 |
| trapped: usermode-doorbell | 1.8 |
| first-doorbell builds (19) | 161.0 |
| build: plan and apply | 14.2 |
| build: TSG | 5.6 |
| build: channel | 55.7 |
| build: objects | 43.1 |
| build: schedule and token | 12.9 |
| build: doorbell | 0.2 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 50.6 |
| untrapped: before a first doorbell (guest) | 2.7 |
| untrapped: before bar0-other | 0.0 |
| untrapped: before bar2-flush-read | 17.9 |
| untrapped: before gsp | 6.3 |
| untrapped: before mmu-invalidate | 23.0 |
| worker apply (background) | 39.4 |
| of it gather allocs (7) | 12.8 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 85 |

## cuCtxDestroy (5 repetitions)

| part | ms |
|---|---|
| window | 165.5 |
| trapped, all classes | 107.8 |
| trapped: bar2-flush-read | 48.9 |
| trapped: gsp | 44.9 |
| trapped: mmu-invalidate | 16.0 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 47.4 |
| untrapped: before bar2-flush-read | 9.1 |
| untrapped: before gsp | 16.0 |
| untrapped: before mmu-invalidate | 17.1 |
| worker apply (background) | 8.1 |
| of it gather allocs (0) | 0.0 |
| host channel frees or retires in RPC handling (20) | 35.1 |
| RPCs (count) | 69 |


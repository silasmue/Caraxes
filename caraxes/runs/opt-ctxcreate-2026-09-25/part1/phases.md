# phases: part1/census-trace.txt.xz

Medians over the repetitions, ms. Trapped: gaps up to the idle bound before a trapped access; builds: the device's first-doorbell handling; untrapped: the other gaps; worker: background.

## cuCtxCreate (5 repetitions)

| part | ms |
|---|---|
| window | 406.8 |
| trapped, all classes | 131.8 |
| trapped: bar0-other | 4.8 |
| trapped: bar2-flush-read | 67.1 |
| trapped: cfg-other | 0.0 |
| trapped: gsp | 24.5 |
| trapped: mmu-invalidate | 32.3 |
| trapped: usermode-doorbell | 0.0 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (20) | 210.6 |
| untrapped gaps above 2.0 ms | 51.2 |
| untrapped: before bar2-flush-read | 25.8 |
| untrapped: before gsp | 5.9 |
| untrapped: before mmu-invalidate | 19.2 |
| worker apply (background) | 26.9 |
| of it gather allocs (7) | 8.4 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 85 |

## cuCtxDestroy (5 repetitions)

| part | ms |
|---|---|
| window | 156.9 |
| trapped, all classes | 110.4 |
| trapped: bar2-flush-read | 46.1 |
| trapped: gsp | 41.5 |
| trapped: mmu-invalidate | 14.8 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 49.1 |
| untrapped: before bar2-flush-read | 6.1 |
| untrapped: before gsp | 28.1 |
| untrapped: before mmu-invalidate | 16.7 |
| worker apply (background) | 6.2 |
| of it gather allocs (0) | 0.0 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 69 |


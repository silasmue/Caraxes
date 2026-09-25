# phases: before/census-trace.txt.xz

Medians over the repetitions, ms. Trapped: gaps up to the idle bound before a trapped access; builds: the device's first-doorbell handling; untrapped: the other gaps; worker: background.

## cuCtxCreate (5 repetitions)

| part | ms |
|---|---|
| window | 356.1 |
| trapped, all classes | 128.6 |
| trapped: bar0-other | 3.4 |
| trapped: bar2-flush-read | 60.0 |
| trapped: cfg-other | 0.0 |
| trapped: gsp | 20.6 |
| trapped: mmu-invalidate | 29.6 |
| trapped: usermode-doorbell | 8.9 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (15) | 172.6 |
| untrapped gaps above 2.0 ms | 47.2 |
| untrapped: before bar2-flush-read | 15.5 |
| untrapped: before cfg-other | 0.0 |
| untrapped: before gsp | 8.5 |
| untrapped: before mmu-invalidate | 19.9 |
| worker apply (background) | 28.2 |
| of it gather allocs (7) | 6.2 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 85 |

## cuCtxDestroy (5 repetitions)

| part | ms |
|---|---|
| window | 151.8 |
| trapped, all classes | 102.8 |
| trapped: bar2-flush-read | 45.3 |
| trapped: gsp | 38.5 |
| trapped: mmu-invalidate | 16.5 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 54.7 |
| untrapped: before bar2-flush-read | 5.2 |
| untrapped: before gsp | 25.6 |
| untrapped: before mmu-invalidate | 13.1 |
| worker apply (background) | 7.3 |
| of it gather allocs (0) | 0.0 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 69 |


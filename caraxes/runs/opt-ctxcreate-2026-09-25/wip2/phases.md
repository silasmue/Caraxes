# phases: wip2/census-trace.txt.xz

Medians over the repetitions, ms. Trapped: gaps up to the idle bound before a trapped access; builds: the device's first-doorbell handling; untrapped: the other gaps; worker: background.

## cuCtxCreate (5 repetitions)

| part | ms |
|---|---|
| window | 2332.7 |
| trapped, all classes | 111.7 |
| trapped: bar0-other | 5.1 |
| trapped: bar2-flush-read | 24.6 |
| trapped: cfg-other | 11.4 |
| trapped: gsp | 28.7 |
| trapped: mmu-invalidate | 40.2 |
| trapped: usermode-doorbell | 0.0 |
| first-doorbell builds (19) | 347.0 |
| build: plan and apply | 45.4 |
| build: TSG | 16.8 |
| build: channel | 96.7 |
| build: objects | 121.9 |
| build: schedule and token | 18.9 |
| build: doorbell | 0.3 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 1958.2 |
| untrapped: before a first doorbell (guest) | 7.6 |
| untrapped: before bar2-flush-read | 437.9 |
| untrapped: before cfg-other | 0.0 |
| untrapped: before gsp | 8.8 |
| untrapped: before mmu-invalidate | 1309.9 |
| worker apply (background) | 50.8 |
| of it gather allocs (12) | 18.7 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 85 |

## cuCtxDestroy (5 repetitions)

| part | ms |
|---|---|
| window | 1636.0 |
| trapped, all classes | 76.5 |
| trapped: bar2-flush-read | 10.6 |
| trapped: cfg-other | 4.2 |
| trapped: gsp | 41.6 |
| trapped: mmu-invalidate | 17.2 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 1561.6 |
| untrapped: before bar2-flush-read | 288.5 |
| untrapped: before cfg-other | 0.0 |
| untrapped: before gsp | 34.4 |
| untrapped: before mmu-invalidate | 1233.6 |
| worker apply (background) | 5.9 |
| of it gather allocs (0) | 0.0 |
| host channel frees or retires in RPC handling (20) | 43.1 |
| RPCs (count) | 69 |


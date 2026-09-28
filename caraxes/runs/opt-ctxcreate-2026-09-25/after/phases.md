# phases: after/census-trace.txt.xz

Medians over the repetitions, ms. Trapped: gaps up to the idle bound before a trapped access; builds: the device's first-doorbell handling; untrapped: the other gaps; worker: background.

## cuCtxCreate (5 repetitions)

| part | ms |
|---|---|
| window | 332.5 |
| trapped, all classes | 155.5 |
| trapped: bar0-other | 4.5 |
| trapped: bar2-flush-read | 73.3 |
| trapped: gsp | 28.8 |
| trapped: mmu-invalidate | 40.7 |
| trapped: usermode-doorbell | 8.5 |
| first-doorbell builds (14) | 99.0 |
| build: plan and apply | 8.8 |
| build: TSG | 5.5 |
| build: channel | 23.5 |
| build: objects | 15.6 |
| build: schedule and token | 8.1 |
| build: doorbell | 0.1 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 84.8 |
| untrapped: before a first doorbell (guest) | 2.5 |
| untrapped: before bar2-flush-read | 57.5 |
| untrapped: before gsp | 6.8 |
| untrapped: before mmu-invalidate | 21.8 |
| worker apply (background) | 35.7 |
| of it gather allocs (8) | 12.6 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 85 |

## cuCtxDestroy (5 repetitions)

| part | ms |
|---|---|
| window | 180.7 |
| trapped, all classes | 106.9 |
| trapped: bar2-flush-read | 50.1 |
| trapped: gsp | 45.3 |
| trapped: mmu-invalidate | 14.3 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 69.3 |
| untrapped: before bar2-flush-read | 11.2 |
| untrapped: before gsp | 20.3 |
| untrapped: before mmu-invalidate | 21.9 |
| worker apply (background) | 10.1 |
| of it gather allocs (0) | 0.0 |
| host channel frees or retires in RPC handling (20) | 29.6 |
| RPCs (count) | 69 |


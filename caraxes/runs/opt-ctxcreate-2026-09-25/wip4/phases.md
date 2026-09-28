# phases: wip4/census-trace.txt.xz

Medians over the repetitions, ms. Trapped: gaps up to the idle bound before a trapped access; builds: the device's first-doorbell handling; untrapped: the other gaps; worker: background.

## cuCtxCreate (5 repetitions)

| part | ms |
|---|---|
| window | 414.0 |
| trapped, all classes | 137.4 |
| trapped: bar0-other | 3.5 |
| trapped: bar2-flush-read | 67.5 |
| trapped: cfg-other | 0.0 |
| trapped: gsp | 22.2 |
| trapped: mmu-invalidate | 31.7 |
| trapped: usermode-doorbell | 5.7 |
| first-doorbell builds (17) | 190.0 |
| build: plan and apply | 5.9 |
| build: TSG | 10.1 |
| build: channel | 46.3 |
| build: objects | 54.8 |
| build: schedule and token | 11.1 |
| build: doorbell | 0.2 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 80.4 |
| untrapped: before a first doorbell (guest) | 2.4 |
| untrapped: before bar0-other | 0.0 |
| untrapped: before bar2-flush-read | 25.3 |
| untrapped: before gsp | 10.8 |
| untrapped: before mmu-invalidate | 33.3 |
| worker apply (background) | 26.9 |
| of it gather allocs (6) | 7.3 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 85 |

## cuCtxDestroy (5 repetitions)

| part | ms |
|---|---|
| window | 176.3 |
| trapped, all classes | 92.4 |
| trapped: bar2-flush-read | 49.7 |
| trapped: gsp | 27.4 |
| trapped: mmu-invalidate | 14.3 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 83.5 |
| untrapped: before bar2-flush-read | 4.0 |
| untrapped: before gsp | 57.6 |
| untrapped: before mmu-invalidate | 21.7 |
| worker apply (background) | 7.4 |
| of it gather allocs (0) | 0.0 |
| host channel frees or retires in RPC handling (20) | 10.1 |
| RPCs (count) | 69 |


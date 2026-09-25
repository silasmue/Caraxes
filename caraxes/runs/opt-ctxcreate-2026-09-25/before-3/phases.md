# phases: before-3/census-trace.txt.xz

Medians over the repetitions, ms. Trapped: gaps up to the idle bound before a trapped access; builds: the device's first-doorbell handling; untrapped: the other gaps; worker: background.

## cuCtxCreate (5 repetitions)

| part | ms |
|---|---|
| window | 547.8 |
| trapped, all classes | 148.9 |
| trapped: bar0-other | 5.7 |
| trapped: bar2-flush-read | 70.4 |
| trapped: cfg-other | 0.0 |
| trapped: gsp | 29.1 |
| trapped: mmu-invalidate | 41.1 |
| trapped: usermode-doorbell | 3.8 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (18) | 190.8 |
| untrapped gaps above 2.0 ms | 97.6 |
| untrapped: before bar0-other | 3.9 |
| untrapped: before bar2-flush-read | 26.4 |
| untrapped: before gsp | 19.2 |
| untrapped: before mmu-invalidate | 41.6 |
| worker apply (background) | 69.5 |
| of it gather allocs (7) | 13.5 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 85 |

## cuCtxDestroy (5 repetitions)

| part | ms |
|---|---|
| window | 214.9 |
| trapped, all classes | 138.4 |
| trapped: bar2-flush-read | 51.2 |
| trapped: gsp | 56.5 |
| trapped: mmu-invalidate | 30.7 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 84.9 |
| untrapped: before bar2-flush-read | 23.0 |
| untrapped: before gsp | 24.4 |
| untrapped: before mmu-invalidate | 20.2 |
| worker apply (background) | 15.5 |
| of it gather allocs (0) | 0.0 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 69 |


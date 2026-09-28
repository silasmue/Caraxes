# phases: after-noprefetch/census-trace.txt.xz

Medians over the repetitions, ms. Trapped: gaps up to the idle bound before a trapped access; builds: the device's first-doorbell handling; untrapped: the other gaps; worker: background.

## cuCtxCreate (5 repetitions)

| part | ms |
|---|---|
| window | 800.9 |
| trapped, all classes | 216.0 |
| trapped: bar0-other | 6.8 |
| trapped: bar2-flush-read | 99.7 |
| trapped: gsp | 49.9 |
| trapped: mmu-invalidate | 60.1 |
| trapped: usermode-doorbell | 0.0 |
| first-doorbell builds (19) | 295.7 |
| build: plan and apply | 45.9 |
| build: TSG | 11.6 |
| build: channel | 96.9 |
| build: objects | 69.0 |
| build: schedule and token | 16.7 |
| build: doorbell | 0.5 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 152.4 |
| untrapped: before a first doorbell (guest) | 11.6 |
| untrapped: before bar0-other | 0.0 |
| untrapped: before bar2-flush-read | 84.7 |
| untrapped: before gsp | 22.4 |
| untrapped: before mmu-invalidate | 51.2 |
| worker apply (background) | 53.0 |
| of it gather allocs (10) | 26.7 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 85 |

## cuCtxDestroy (5 repetitions)

| part | ms |
|---|---|
| window | 333.1 |
| trapped, all classes | 184.1 |
| trapped: bar2-flush-read | 71.8 |
| trapped: cfg-other | 0.0 |
| trapped: gsp | 71.0 |
| trapped: mmu-invalidate | 38.2 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 127.1 |
| untrapped: before bar2-flush-read | 38.2 |
| untrapped: before cfg-other | 0.0 |
| untrapped: before gsp | 71.0 |
| untrapped: before mmu-invalidate | 17.9 |
| worker apply (background) | 16.3 |
| of it gather allocs (0) | 0.0 |
| host channel frees or retires in RPC handling (20) | 44.1 |
| RPCs (count) | 69 |


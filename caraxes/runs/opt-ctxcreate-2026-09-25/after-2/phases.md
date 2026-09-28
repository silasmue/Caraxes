# phases: after-2/census-trace.txt.xz

Medians over the repetitions, ms. Trapped: gaps up to the idle bound before a trapped access; builds: the device's first-doorbell handling; untrapped: the other gaps; worker: background.

## cuCtxCreate (5 repetitions)

| part | ms |
|---|---|
| window | 2485.9 |
| trapped, all classes | 446.8 |
| trapped: bar0-other | 17.1 |
| trapped: bar2-flush-read | 104.5 |
| trapped: cfg-other | 45.2 |
| trapped: gsp | 113.2 |
| trapped: mmu-invalidate | 155.4 |
| first-doorbell builds (20) | 427.3 |
| build: plan and apply | 18.4 |
| build: TSG | 20.3 |
| build: channel | 170.9 |
| build: objects | 154.5 |
| build: schedule and token | 61.6 |
| build: doorbell | 1.6 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 1602.3 |
| untrapped: before a first doorbell (guest) | 40.1 |
| untrapped: before bar0-other | 37.6 |
| untrapped: before bar2-flush-read | 790.4 |
| untrapped: before cfg-other | 23.8 |
| untrapped: before gsp | 294.8 |
| untrapped: before mmu-invalidate | 415.5 |
| worker apply (background) | 145.1 |
| of it gather allocs (5) | 35.9 |
| host channel frees or retires in RPC handling (0) | 0.0 |
| RPCs (count) | 85 |

## cuCtxDestroy (5 repetitions)

| part | ms |
|---|---|
| window | 1197.5 |
| trapped, all classes | 249.0 |
| trapped: bar2-flush-read | 82.9 |
| trapped: gsp | 88.1 |
| trapped: mmu-invalidate | 78.0 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 976.2 |
| untrapped: before bar2-flush-read | 370.5 |
| untrapped: before gsp | 344.7 |
| untrapped: before mmu-invalidate | 171.5 |
| worker apply (background) | 34.0 |
| of it gather allocs (0) | 0.0 |
| host channel frees or retires in RPC handling (20) | 134.6 |
| RPCs (count) | 69 |


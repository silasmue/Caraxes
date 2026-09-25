# phases: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/before-2/census-trace.txt

Medians over the repetitions, ms. Trapped: gaps up to the idle bound before a trapped access; builds: the device's first-doorbell handling; untrapped: the other gaps; worker: background.

## cuCtxCreate (5 repetitions)

| part | ms |
|---|---|
| window | 1896.5 |
| trapped, all classes | 222.8 |
| trapped: bar0-other | 7.2 |
| trapped: bar2-flush-read | 102.3 |
| trapped: cfg-other | 11.2 |
| trapped: gsp | 41.8 |
| trapped: mmu-invalidate | 52.2 |
| trapped: usermode-doorbell | 0.0 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (20) | 373.7 |
| untrapped gaps above 2.0 ms | 660.5 |
| untrapped: before bar0-other | 0.0 |
| untrapped: before bar2-flush-read | 104.9 |
| untrapped: before cfg-other | 0.0 |
| untrapped: before gsp | 32.3 |
| untrapped: before mmu-invalidate | 517.3 |
| worker apply (background) | 118.7 |
| of it gather allocs (9) | 15.7 |
| host channel frees in RPC handling (0) | 0.0 |
| RPCs (count) | 85 |

## cuCtxDestroy (5 repetitions)

| part | ms |
|---|---|
| window | 408.2 |
| trapped, all classes | 146.6 |
| trapped: bar2-flush-read | 69.2 |
| trapped: gsp | 53.7 |
| trapped: mmu-invalidate | 26.7 |
| first-doorbell builds (0) | 0.0 |
| build: plan and apply | 0.0 |
| build: TSG | 0.0 |
| build: channel | 0.0 |
| build: objects | 0.0 |
| build: schedule and token | 0.0 |
| build: doorbell | 0.0 |
| doorbell gaps without a build line (0) | 0.0 |
| untrapped gaps above 2.0 ms | 261.1 |
| untrapped: before bar2-flush-read | 55.7 |
| untrapped: before gsp | 64.0 |
| untrapped: before mmu-invalidate | 87.8 |
| worker apply (background) | 16.8 |
| of it gather allocs (0) | 0.0 |
| host channel frees in RPC handling (0) | 0.0 |
| RPCs (count) | 69 |


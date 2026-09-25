# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a7e883e004e873949/caraxes/runs/opt-shadow-maps-2026-09-25/final/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 15.1 | 100140 | 100088.3 |
| 2 | cuInit | 3 | 174.3 | 22893 | 22811.2 |
| 3 | cuCtxCreate | 5 | 1881.4 | 837958 | 837913.4 |
| 4 | cuCtxDestroy | 5 | 748.8 | 154227 | 154186.2 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 352 | 265.7 |
| 11 | cuMemFree_1M | 20 | 6.5 | 437 | 395.1 |
| 12 | cuMemAlloc_64M | 10 | 8.0 | 902 | 850.0 |
| 13 | cuMemFree_64M | 10 | 13.3 | 1270 | 1217.4 |
| 14 | cuMemHostAlloc_1M | 10 | 8.0 | 1564 | 1499.6 |
| 15 | cuMemFreeHost_1M | 10 | 7.4 | 1117 | 1075.4 |
| 16 | cuMemHostRegister_1M | 10 | 15.3 | 1741 | 1697.5 |
| 17 | cuMemHostUnregister_1M | 10 | 14.4 | 1522 | 1488.2 |
| 20 | launch_1_sync | 20 | 3.1 | 783 | 731.0 |
| 21 | launch_1000_sync | 1 | 3025.0 | 170663 | 111.3 |
| 30 | h2d_pinned_1M | 10 | 3.1 | 523 | 478.8 |
| 31 | d2h_pinned_1M | 10 | 3.1 | 640 | 567.4 |
| 32 | d2d_1M | 10 | 3.4 | 1010 | 965.8 |
| 33 | h2d_pinned_16M | 10 | 3.7 | 5308 | 5264.3 |
| 34 | d2h_pinned_16M | 10 | 3.7 | 5410 | 5348.0 |
| 35 | d2d_16M | 10 | 3.0 | 4012 | 3972.5 |
| 36 | h2d_pageable_1M | 10 | 3.0 | 620 | 574.0 |
| 37 | d2h_pageable_1M | 10 | 3.1 | 672 | 604.7 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 36 | 4.0 |
| 41 | cuEventRecord_Synchronize | 20 | 3.2 | 756 | 719.3 |
| 50 | cuModuleLoad_cubin | 10 | 3.2 | 922 | 881.7 |
| 51 | cuModuleUnload | 10 | 0.1 | 66 | 23.9 |

## op 1 idle_100ms: 10 repetition(s), 15.1 trapped accesses per repetition, window median 100140 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 15.1 | 210 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 15.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none


## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 22893 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 3202 |
| mmu-invalidate | 40.0 | 1435 |
| gsp-queue-doorbell | 27.3 | 8395 |
| bar2-flush-read | 24.0 | 1926 |
| pcfg-mirror (intr re-arm) | 1.0 | 35 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 27.33 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 27.33 |
| BAR0 | 0x110000 | R | 0x110804 NV_PGSP | 27.33 |
| BAR0 | 0x110000 | R | 0x110808 NV_PGSP | 27.33 |
| BAR2 | flush page | R |  | 24.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 16.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 8.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 8.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 8.00 |
| BAR0 | 0x088000 | R | 0x088084 NV_PCFG | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 27.3 per rep (91 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 54.7

## op 3 cuCtxCreate: 5 repetition(s), 1881.4 trapped accesses per repetition, window median 837958 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 642.0 | 34439 |
| mmu-invalidate | 487.0 | 19740 |
| bar2-flush-read | 272.2 | 64016 |
| gsp-falcon | 255.0 | 9351 |
| gsp-queue-doorbell | 85.0 | 11918 |
| usermode page, other | 60.0 | 2413 |
| pcfg-mirror (intr re-arm) | 52.2 | 6203 |
| usermode-doorbell | 20.0 | 3465 |
| bar0-other | 8.0 | 1144 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 424.00 |
| BAR2 | flush page | R |  | 272.20 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 218.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 194.80 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.40 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.40 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.40 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 85.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 85.00 |
| BAR0 | 0x110000 | R | 0x110804 NV_PGSP | 85.00 |
| BAR0 | 0x110000 | R | 0x110808 NV_PGSP | 85.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 52.20 |
| (rest) | | | | 88.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 642.0, tokens 20.0

- GSP RPCs 85.0 per rep (362 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.8 per rep: 19619 leaves mapped and 5987 unmapped in 7414.4 RM map call(s), walk 2009 us, worker apply 681085 us per rep
- events naming a host RM call 446.4 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x214.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x137.6; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X (X/X) [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 194.0; guest MMU invalidates 97.4; host doorbells written (forwarded) 214.0; host notifier events -> MSI 137.6; other events 567.6

## op 4 cuCtxDestroy: 5 repetition(s), 748.8 trapped accesses per repetition, window median 154227 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 14000 |
| gsp-falcon | 207.0 | 9353 |
| bar2-flush-read | 169.0 | 48265 |
| gsp-queue-doorbell | 69.0 | 27982 |
| pcfg-mirror (intr re-arm) | 23.8 | 2775 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 169.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 112.00 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 69.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | R | 0x110804 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | R | 0x110808 NV_PGSP | 69.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 23.80 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (211 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.2 per rep: 0 leaves mapped and 11551 unmapped in 0.0 RM map call(s), walk 194 us, worker apply 22463 us per rep
- events naming a host RM call 55.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x29.8; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events -> MSI 29.8; other events 153.2

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 352 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 132 |
| bar2-flush-read | 1.0 | 174 |
| pcfg-mirror (intr re-arm) | 0.1 | 3 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 6 us, worker apply 103 us per rep
- events naming a host RM call 13.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x13.9
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 13.9; other events 0.5

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 437 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 248 |
| bar2-flush-read | 1.5 | 193 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 5 us, worker apply 87 us per rep
- events naming a host RM call 4.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.3
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.3; other events 0.5

## op 12 cuMemAlloc_64M: 10 repetition(s), 8.0 trapped accesses per repetition, window median 902 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 280 |
| bar2-flush-read | 2.3 | 604 |
| pcfg-mirror (intr re-arm) | 0.2 | 18 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 29 us, worker apply 3566 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 5.0; other events 1.0

## op 13 cuMemFree_64M: 10 repetition(s), 13.3 trapped accesses per repetition, window median 1270 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 522 |
| bar2-flush-read | 3.1 | 642 |
| pcfg-mirror (intr re-arm) | 0.2 | 10 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 33 us, worker apply 3560 us per rep
- events naming a host RM call 4.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.7
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.7; other events 1.0

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 8.0 trapped accesses per repetition, window median 1564 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 249 |
| bar2-flush-read | 2.5 | 708 |
| pcfg-mirror (intr re-arm) | 0.5 | 293 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 15.5 RM map call(s), walk 20 us, worker apply 1121 us per rep
- events naming a host RM call 4.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.6
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.6; other events 0.5

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.4 trapped accesses per repetition, window median 1117 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 240 |
| bar2-flush-read | 2.0 | 722 |
| pcfg-mirror (intr re-arm) | 0.4 | 42 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 51 leaves mapped and 205 unmapped in 3.4 RM map call(s), walk 13 us, worker apply 614 us per rep
- events naming a host RM call 5.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.3
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.3; other events 0.5

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.3 trapped accesses per repetition, window median 1741 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 446 |
| bar2-flush-read | 5.0 | 1240 |
| pcfg-mirror (intr re-arm) | 0.3 | 13 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 230 leaves mapped and 51 unmapped in 10.9 RM map call(s), walk 23 us, worker apply 1055 us per rep
- events naming a host RM call 4.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.9
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.9; other events 1.0

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.4 trapped accesses per repetition, window median 1522 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 400 |
| bar2-flush-read | 4.0 | 1025 |
| pcfg-mirror (intr re-arm) | 0.4 | 33 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 26 leaves mapped and 230 unmapped in 0.1 RM map call(s), walk 17 us, worker apply 553 us per rep
- events naming a host RM call 4.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.6
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.6; other events 1.0

## op 20 launch_1_sync: 20 repetition(s), 3.1 trapped accesses per repetition, window median 783 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 135 |
| pcfg-mirror (intr re-arm) | 0.1 | 65 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 21 launch_1000_sync: 1 repetition(s), 3025.0 trapped accesses per repetition, window median 170663 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3000.0 | 156800 |
| pcfg-mirror (intr re-arm) | 25.0 | 13812 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1984.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1016.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 25.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3000.0

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1000.0

## op 30 h2d_pinned_1M: 10 repetition(s), 3.1 trapped accesses per repetition, window median 523 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 154 |
| pcfg-mirror (intr re-arm) | 0.1 | 4 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 31 d2h_pinned_1M: 10 repetition(s), 3.1 trapped accesses per repetition, window median 640 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 218 |
| pcfg-mirror (intr re-arm) | 0.1 | 6 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 32 d2d_1M: 10 repetition(s), 3.4 trapped accesses per repetition, window median 1010 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 157 |
| pcfg-mirror (intr re-arm) | 0.4 | 145 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 33 h2d_pinned_16M: 10 repetition(s), 3.7 trapped accesses per repetition, window median 5308 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 142 |
| pcfg-mirror (intr re-arm) | 0.7 | 86 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.70 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 34 d2h_pinned_16M: 10 repetition(s), 3.7 trapped accesses per repetition, window median 5410 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 150 |
| pcfg-mirror (intr re-arm) | 0.7 | 242 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.70 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 35 d2d_16M: 10 repetition(s), 3.0 trapped accesses per repetition, window median 4012 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 137 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 36 h2d_pageable_1M: 10 repetition(s), 3.0 trapped accesses per repetition, window median 620 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 252 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 10 repetition(s), 3.1 trapped accesses per repetition, window median 672 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 164 |
| pcfg-mirror (intr re-arm) | 0.1 | 48 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1.90 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.10 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 36 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 3.2 trapped accesses per repetition, window median 756 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 121 |
| pcfg-mirror (intr re-arm) | 0.2 | 95 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1.95 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.05 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 50 cuModuleLoad_cubin: 10 repetition(s), 3.2 trapped accesses per repetition, window median 922 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 262 |
| pcfg-mirror (intr re-arm) | 0.2 | 51 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 51 cuModuleUnload: 10 repetition(s), 0.1 trapped accesses per repetition, window median 66 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 4 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none



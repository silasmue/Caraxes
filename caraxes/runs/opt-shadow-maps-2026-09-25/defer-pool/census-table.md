# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a7e883e004e873949/caraxes/runs/opt-shadow-maps-2026-09-25/after/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 14.5 | 100184 | 100092.8 |
| 2 | cuInit | 3 | 174.3 | 22238 | 22167.7 |
| 3 | cuCtxCreate | 5 | 1875.8 | 935830 | 935763.8 |
| 4 | cuCtxDestroy | 5 | 749.8 | 190859 | 190815.5 |
| 10 | cuMemAlloc_1M | 20 | 3.7 | 331 | 284.2 |
| 11 | cuMemFree_1M | 20 | 6.6 | 593 | 528.0 |
| 12 | cuMemAlloc_64M | 10 | 7.8 | 993 | 942.8 |
| 13 | cuMemFree_64M | 10 | 13.4 | 1428 | 1350.0 |
| 14 | cuMemHostAlloc_1M | 10 | 7.9 | 2702 | 2634.4 |
| 15 | cuMemFreeHost_1M | 10 | 7.2 | 1206 | 1155.3 |
| 16 | cuMemHostRegister_1M | 10 | 15.9 | 3243 | 3038.7 |
| 17 | cuMemHostUnregister_1M | 10 | 14.4 | 3638 | 3559.1 |
| 20 | launch_1_sync | 20 | 3.3 | 2078 | 1993.4 |
| 21 | launch_1000_sync | 1 | 3125.0 | 906161 | 193.7 |
| 30 | h2d_pinned_1M | 10 | 3.3 | 805 | 682.5 |
| 31 | d2h_pinned_1M | 10 | 3.4 | 4472 | 4162.5 |
| 32 | d2d_1M | 10 | 3.4 | 2320 | 2091.8 |
| 33 | h2d_pinned_16M | 10 | 3.7 | 6284 | 6170.9 |
| 34 | d2h_pinned_16M | 10 | 3.8 | 6472 | 6372.3 |
| 35 | d2d_16M | 10 | 3.2 | 4228 | 4126.0 |
| 36 | h2d_pageable_1M | 10 | 3.2 | 806 | 734.4 |
| 37 | d2h_pageable_1M | 10 | 3.5 | 860 | 744.7 |
| 40 | cuStreamSynchronize_idle | 20 | 0.1 | 82 | 16.5 |
| 41 | cuEventRecord_Synchronize | 20 | 3.5 | 2865 | 2566.7 |
| 50 | cuModuleLoad_cubin | 10 | 3.2 | 928 | 871.1 |
| 51 | cuModuleUnload | 10 | 0.0 | 60 | 23.5 |

## op 1 idle_100ms: 10 repetition(s), 14.5 trapped accesses per repetition, window median 100184 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 14.5 | 223 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 14.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none


## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 22238 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 3773 |
| mmu-invalidate | 40.0 | 1490 |
| gsp-queue-doorbell | 27.3 | 9139 |
| bar2-flush-read | 24.0 | 1755 |
| pcfg-mirror (intr re-arm) | 1.0 | 40 |

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

- GSP RPCs 27.3 per rep (89 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 54.7

## op 3 cuCtxCreate: 5 repetition(s), 1875.8 trapped accesses per repetition, window median 935830 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 642.0 | 38916 |
| mmu-invalidate | 487.0 | 24108 |
| bar2-flush-read | 272.2 | 63226 |
| gsp-falcon | 255.0 | 10560 |
| gsp-queue-doorbell | 85.0 | 13379 |
| usermode page, other | 60.0 | 3030 |
| pcfg-mirror (intr re-arm) | 46.6 | 9696 |
| usermode-doorbell | 20.0 | 3554 |
| bar0-other | 8.0 | 1824 |

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
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 46.60 |
| (rest) | | | | 88.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 642.0, tokens 20.0

- GSP RPCs 85.0 per rep (332 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.8 per rep: 19619 leaves mapped and 5987 unmapped in 7435.6 RM map call(s), walk 2085 us, worker apply 906566 us per rep
- events naming a host RM call 480.4 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x214.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x171.6; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X (X/X) [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 194.0; guest MMU invalidates 97.4; host doorbells written (forwarded) 214.0; host notifier events -> MSI 171.6; other events 568.2

## op 4 cuCtxDestroy: 5 repetition(s), 749.8 trapped accesses per repetition, window median 190859 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 13391 |
| gsp-falcon | 207.0 | 11681 |
| bar2-flush-read | 169.0 | 48327 |
| gsp-queue-doorbell | 69.0 | 28645 |
| pcfg-mirror (intr re-arm) | 24.8 | 4728 |

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
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 24.80 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (266 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.2 per rep: 0 leaves mapped and 11551 unmapped in 0.0 RM map call(s), walk 216 us, worker apply 46074 us per rep
- events naming a host RM call 60.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x34.8; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events -> MSI 34.8; other events 153.2

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.7 trapped accesses per repetition, window median 331 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 161 |
| bar2-flush-read | 1.0 | 218 |
| pcfg-mirror (intr re-arm) | 0.2 | 9 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 8 us, worker apply 187 us per rep
- events naming a host RM call 14.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.1
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 14.1; other events 0.5

## op 11 cuMemFree_1M: 20 repetition(s), 6.6 trapped accesses per repetition, window median 593 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 382 |
| bar2-flush-read | 1.5 | 376 |
| pcfg-mirror (intr re-arm) | 0.1 | 5 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 8 us, worker apply 6540 us per rep
- events naming a host RM call 4.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.1
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.1; other events 0.5

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 993 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 315 |
| bar2-flush-read | 2.3 | 702 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 35 us, worker apply 6354 us per rep
- events naming a host RM call 5.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.1
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 5.1; other events 1.0

## op 13 cuMemFree_64M: 10 repetition(s), 13.4 trapped accesses per repetition, window median 1428 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 599 |
| bar2-flush-read | 3.1 | 691 |
| pcfg-mirror (intr re-arm) | 0.3 | 32 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 37 us, worker apply 3215 us per rep
- events naming a host RM call 4.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.6
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.6; other events 1.0

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.9 trapped accesses per repetition, window median 2702 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 320 |
| bar2-flush-read | 2.5 | 926 |
| pcfg-mirror (intr re-arm) | 0.4 | 325 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 0.9 RM map call(s), walk 26 us, worker apply 465 us per rep
- events naming a host RM call 4.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.6
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.6; other events 0.5

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.2 trapped accesses per repetition, window median 1206 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 382 |
| bar2-flush-read | 2.0 | 847 |
| pcfg-mirror (intr re-arm) | 0.2 | 10 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 51 leaves mapped and 205 unmapped in 0.1 RM map call(s), walk 12 us, worker apply 140 us per rep
- events naming a host RM call 5.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.5; other events 0.5

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.9 trapped accesses per repetition, window median 3243 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 992 |
| bar2-flush-read | 5.0 | 1710 |
| pcfg-mirror (intr re-arm) | 0.9 | 52 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.90 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 230 leaves mapped and 51 unmapped in 5.0 RM map call(s), walk 39 us, worker apply 1516 us per rep
- events naming a host RM call 5.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.2
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.2; other events 1.0

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.4 trapped accesses per repetition, window median 3638 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1690 |
| bar2-flush-read | 4.0 | 1667 |
| pcfg-mirror (intr re-arm) | 0.4 | 62 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 26 leaves mapped and 230 unmapped in 0.1 RM map call(s), walk 26 us, worker apply 523 us per rep
- events naming a host RM call 5.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.1
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.1; other events 1.0

## op 20 launch_1_sync: 20 repetition(s), 3.3 trapped accesses per repetition, window median 2078 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 566 |
| pcfg-mirror (intr re-arm) | 0.3 | 10 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 21 launch_1000_sync: 1 repetition(s), 3125.0 trapped accesses per repetition, window median 906161 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3000.0 | 512221 |
| pcfg-mirror (intr re-arm) | 125.0 | 83831 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1984.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1016.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 125.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3000.0

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1000.0

## op 30 h2d_pinned_1M: 10 repetition(s), 3.3 trapped accesses per repetition, window median 805 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 356 |
| pcfg-mirror (intr re-arm) | 0.3 | 77 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 31 d2h_pinned_1M: 10 repetition(s), 3.4 trapped accesses per repetition, window median 4472 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 718 |
| pcfg-mirror (intr re-arm) | 0.4 | 394 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 32 d2d_1M: 10 repetition(s), 3.4 trapped accesses per repetition, window median 2320 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 497 |
| pcfg-mirror (intr re-arm) | 0.4 | 144 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 33 h2d_pinned_16M: 10 repetition(s), 3.7 trapped accesses per repetition, window median 6284 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 236 |
| pcfg-mirror (intr re-arm) | 0.7 | 184 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.70 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 34 d2h_pinned_16M: 10 repetition(s), 3.8 trapped accesses per repetition, window median 6472 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 470 |
| pcfg-mirror (intr re-arm) | 0.8 | 52 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.80 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 35 d2d_16M: 10 repetition(s), 3.2 trapped accesses per repetition, window median 4228 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 231 |
| pcfg-mirror (intr re-arm) | 0.2 | 164 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 36 h2d_pageable_1M: 10 repetition(s), 3.2 trapped accesses per repetition, window median 806 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 485 |
| pcfg-mirror (intr re-arm) | 0.2 | 102 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 10 repetition(s), 3.5 trapped accesses per repetition, window median 860 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 411 |
| pcfg-mirror (intr re-arm) | 0.5 | 99 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1.90 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.10 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.1 trapped accesses per repetition, window median 82 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 39 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 3.5 trapped accesses per repetition, window median 2865 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 269 |
| pcfg-mirror (intr re-arm) | 0.5 | 90 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1.95 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.05 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.45 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 50 cuModuleLoad_cubin: 10 repetition(s), 3.2 trapped accesses per repetition, window median 928 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 252 |
| pcfg-mirror (intr re-arm) | 0.2 | 141 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 60 us



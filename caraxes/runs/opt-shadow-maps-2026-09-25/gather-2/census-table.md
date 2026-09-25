# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a7e883e004e873949/caraxes/runs/opt-shadow-maps-2026-09-25/gather-2/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 15.2 | 100152 | 100082.8 |
| 2 | cuInit | 3 | 174.3 | 28357 | 28269.3 |
| 3 | cuCtxCreate | 5 | 1863.6 | 309079 | 309027.1 |
| 4 | cuCtxDestroy | 5 | 740.6 | 107061 | 107017.2 |
| 10 | cuMemAlloc_1M | 20 | 3.6 | 333 | 287.9 |
| 11 | cuMemFree_1M | 20 | 6.5 | 418 | 368.6 |
| 12 | cuMemAlloc_64M | 10 | 7.8 | 869 | 821.9 |
| 13 | cuMemFree_64M | 10 | 13.2 | 1115 | 1074.4 |
| 14 | cuMemHostAlloc_1M | 10 | 7.8 | 1652 | 1605.1 |
| 15 | cuMemFreeHost_1M | 10 | 7.2 | 1196 | 1095.5 |
| 16 | cuMemHostRegister_1M | 10 | 15.4 | 1798 | 1763.2 |
| 17 | cuMemHostUnregister_1M | 10 | 14.1 | 1325 | 1292.1 |
| 20 | launch_1_sync | 20 | 3.0 | 398 | 343.1 |
| 21 | launch_1000_sync | 1 | 3016.0 | 102930 | 94.6 |
| 30 | h2d_pinned_1M | 10 | 3.1 | 263 | 228.7 |
| 31 | d2h_pinned_1M | 10 | 3.0 | 274 | 234.1 |
| 32 | d2d_1M | 10 | 3.1 | 580 | 540.8 |
| 33 | h2d_pinned_16M | 10 | 3.5 | 2724 | 2684.3 |
| 34 | d2h_pinned_16M | 10 | 3.5 | 2720 | 2675.3 |
| 35 | d2d_16M | 10 | 3.3 | 1828 | 1785.8 |
| 36 | h2d_pageable_1M | 10 | 3.0 | 524 | 469.4 |
| 37 | d2h_pageable_1M | 10 | 3.0 | 492 | 435.8 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 43 | 3.6 |
| 41 | cuEventRecord_Synchronize | 20 | 3.2 | 834 | 797.0 |
| 50 | cuModuleLoad_cubin | 10 | 3.3 | 939 | 902.6 |
| 51 | cuModuleUnload | 10 | 0.1 | 71 | 20.4 |

## op 1 idle_100ms: 10 repetition(s), 15.2 trapped accesses per repetition, window median 100152 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 15.2 | 325 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 15.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none


## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 28357 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 3691 |
| mmu-invalidate | 40.0 | 1870 |
| gsp-queue-doorbell | 27.3 | 9522 |
| bar2-flush-read | 24.0 | 1857 |
| pcfg-mirror (intr re-arm) | 1.0 | 36 |

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

- GSP RPCs 27.3 per rep (95 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 54.7

## op 3 cuCtxCreate: 5 repetition(s), 1863.6 trapped accesses per repetition, window median 309079 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 642.0 | 34577 |
| mmu-invalidate | 487.0 | 19208 |
| bar2-flush-read | 272.2 | 55763 |
| gsp-falcon | 255.0 | 10209 |
| gsp-queue-doorbell | 85.0 | 10872 |
| usermode page, other | 60.0 | 2759 |
| pcfg-mirror (intr re-arm) | 34.4 | 4407 |
| usermode-doorbell | 20.0 | 9396 |
| bar0-other | 8.0 | 847 |

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
| BAR0 | 0xbb0000 | R | 0xbb0084 NV_VIRTUAL_FUNCTION_TIME_0/1 | 40.00 |
| (rest) | | | | 82.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 642.0, tokens 20.0

- GSP RPCs 85.0 per rep (292 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.0 per rep: 19619 leaves mapped and 252 unmapped in 122.6 RM map call(s), walk 1827 us, worker apply 66832 us per rep
- events naming a host RM call 352.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x214.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x43.6; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X (X/X) [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 194.0; guest MMU invalidates 97.4; host doorbells written (forwarded) 214.0; host notifier events -> MSI 43.6; other events 565.0

## op 4 cuCtxDestroy: 5 repetition(s), 740.6 trapped accesses per repetition, window median 107061 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 10966 |
| gsp-falcon | 207.0 | 7873 |
| bar2-flush-read | 169.0 | 42327 |
| gsp-queue-doorbell | 69.0 | 34946 |
| pcfg-mirror (intr re-arm) | 15.6 | 1038 |

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
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 15.60 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (216 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 16.0 per rep: 0 leaves mapped and 17286 unmapped in 0.0 RM map call(s), walk 194 us, worker apply 6722 us per rep
- events naming a host RM call 47.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x20.8; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0; shadow: VAS of guest root X freed: N syncs (N skipped clean, N full walks, N table reads,  x0.6
- device events per rep: guest MMU invalidates 56.0; host notifier events -> MSI 20.8; other events 154.6

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.6 trapped accesses per repetition, window median 333 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 148 |
| bar2-flush-read | 1.0 | 192 |
| pcfg-mirror (intr re-arm) | 0.1 | 22 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 7 us, worker apply 217 us per rep
- events naming a host RM call 14.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.5
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 14.5; other events 0.5

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 418 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 205 |
| bar2-flush-read | 1.5 | 215 |
| pcfg-mirror (intr re-arm) | 0.1 | 3 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 7 us, worker apply 356 us per rep
- events naming a host RM call 4.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.3
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.3; other events 0.6

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 869 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 280 |
| bar2-flush-read | 2.3 | 547 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 28 us, worker apply 3888 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 5.0; other events 0.9

## op 13 cuMemFree_64M: 10 repetition(s), 13.2 trapped accesses per repetition, window median 1115 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 434 |
| bar2-flush-read | 3.1 | 543 |
| pcfg-mirror (intr re-arm) | 0.1 | 6 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 31 us, worker apply 5890 us per rep
- events naming a host RM call 4.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.7
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.7; other events 1.0

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 1652 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 223 |
| bar2-flush-read | 2.5 | 1022 |
| pcfg-mirror (intr re-arm) | 0.3 | 110 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 0.8 RM map call(s), walk 22 us, worker apply 799 us per rep
- events naming a host RM call 4.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.3
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.3; other events 0.5

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.2 trapped accesses per repetition, window median 1196 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 219 |
| bar2-flush-read | 2.0 | 904 |
| pcfg-mirror (intr re-arm) | 0.2 | 8 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 51 leaves mapped and 205 unmapped in 0.2 RM map call(s), walk 12 us, worker apply 567 us per rep
- events naming a host RM call 6.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 6.5; other events 0.5

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.4 trapped accesses per repetition, window median 1798 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 468 |
| bar2-flush-read | 5.0 | 1319 |
| pcfg-mirror (intr re-arm) | 0.4 | 38 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 230 leaves mapped and 51 unmapped in 2.3 RM map call(s), walk 21 us, worker apply 5126 us per rep
- events naming a host RM call 7.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x7.0
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 7.0; other events 1.0

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.1 trapped accesses per repetition, window median 1325 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 456 |
| bar2-flush-read | 4.0 | 774 |
| pcfg-mirror (intr re-arm) | 0.1 | 73 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 16 us, worker apply 237 us per rep
- events naming a host RM call 4.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.7
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.7; other events 1.1

## op 20 launch_1_sync: 20 repetition(s), 3.0 trapped accesses per repetition, window median 398 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 181 |
| pcfg-mirror (intr re-arm) | 0.1 | 2 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 21 launch_1000_sync: 1 repetition(s), 3016.0 trapped accesses per repetition, window median 102930 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3000.0 | 98366 |
| pcfg-mirror (intr re-arm) | 16.0 | 4520 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1984.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1016.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 16.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3000.0

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1000.0

## op 30 h2d_pinned_1M: 10 repetition(s), 3.1 trapped accesses per repetition, window median 263 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 127 |
| pcfg-mirror (intr re-arm) | 0.1 | 3 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 31 d2h_pinned_1M: 10 repetition(s), 3.0 trapped accesses per repetition, window median 274 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 138 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 32 d2d_1M: 10 repetition(s), 3.1 trapped accesses per repetition, window median 580 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 148 |
| pcfg-mirror (intr re-arm) | 0.1 | 6 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 33 h2d_pinned_16M: 10 repetition(s), 3.5 trapped accesses per repetition, window median 2724 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 126 |
| pcfg-mirror (intr re-arm) | 0.5 | 167 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 34 d2h_pinned_16M: 10 repetition(s), 3.5 trapped accesses per repetition, window median 2720 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 137 |
| pcfg-mirror (intr re-arm) | 0.5 | 244 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 35 d2d_16M: 10 repetition(s), 3.3 trapped accesses per repetition, window median 1828 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 151 |
| pcfg-mirror (intr re-arm) | 0.3 | 160 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 36 h2d_pageable_1M: 10 repetition(s), 3.0 trapped accesses per repetition, window median 524 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 328 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 10 repetition(s), 3.0 trapped accesses per repetition, window median 492 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 160 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1.90 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 43 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 3.2 trapped accesses per repetition, window median 834 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 151 |
| pcfg-mirror (intr re-arm) | 0.2 | 94 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1.95 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.05 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 50 cuModuleLoad_cubin: 10 repetition(s), 3.3 trapped accesses per repetition, window median 939 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 264 |
| pcfg-mirror (intr re-arm) | 0.3 | 11 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 51 cuModuleUnload: 10 repetition(s), 0.1 trapped accesses per repetition, window median 71 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 4 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none



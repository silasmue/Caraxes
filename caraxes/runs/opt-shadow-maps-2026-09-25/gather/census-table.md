# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a7e883e004e873949/caraxes/runs/opt-shadow-maps-2026-09-25/gather/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 15.0 | 100135 | 100081.4 |
| 2 | cuInit | 3 | 174.3 | 12208 | 12144.3 |
| 3 | cuCtxCreate | 5 | 1872.8 | 352297 | 352251.7 |
| 4 | cuCtxDestroy | 5 | 744.8 | 132961 | 132912.6 |
| 10 | cuMemAlloc_1M | 20 | 3.6 | 493 | 395.4 |
| 11 | cuMemFree_1M | 20 | 6.7 | 868 | 838.2 |
| 12 | cuMemAlloc_64M | 10 | 8.0 | 1018 | 945.1 |
| 13 | cuMemFree_64M | 10 | 13.4 | 1234 | 1195.7 |
| 14 | cuMemHostAlloc_1M | 10 | 7.7 | 1508 | 1470.3 |
| 15 | cuMemFreeHost_1M | 10 | 7.1 | 1364 | 1326.0 |
| 16 | cuMemHostRegister_1M | 10 | 15.3 | 1693 | 1660.1 |
| 17 | cuMemHostUnregister_1M | 10 | 14.2 | 1461 | 1400.1 |
| 20 | launch_1_sync | 20 | 3.1 | 576 | 543.0 |
| 21 | launch_1000_sync | 1 | 3019.0 | 126471 | 102.3 |
| 30 | h2d_pinned_1M | 10 | 3.1 | 394 | 338.5 |
| 31 | d2h_pinned_1M | 10 | 3.1 | 291 | 247.2 |
| 32 | d2d_1M | 10 | 3.0 | 297 | 262.4 |
| 33 | h2d_pinned_16M | 10 | 3.3 | 2714 | 2677.9 |
| 34 | d2h_pinned_16M | 10 | 3.5 | 2760 | 2714.0 |
| 35 | d2d_16M | 10 | 3.0 | 1613 | 1564.6 |
| 36 | h2d_pageable_1M | 10 | 3.2 | 480 | 429.3 |
| 37 | d2h_pageable_1M | 10 | 3.1 | 414 | 374.5 |
| 40 | cuStreamSynchronize_idle | 20 | 0.1 | 48 | 9.4 |
| 41 | cuEventRecord_Synchronize | 20 | 3.2 | 778 | 735.0 |
| 50 | cuModuleLoad_cubin | 10 | 3.2 | 907 | 867.8 |
| 51 | cuModuleUnload | 10 | 0.1 | 68 | 28.1 |

## op 1 idle_100ms: 10 repetition(s), 15.0 trapped accesses per repetition, window median 100135 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 15.0 | 471 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 15.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none


## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 12208 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 2891 |
| mmu-invalidate | 40.0 | 1294 |
| gsp-queue-doorbell | 27.3 | 7348 |
| bar2-flush-read | 24.0 | 1651 |
| pcfg-mirror (intr re-arm) | 1.0 | 31 |

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

- GSP RPCs 27.3 per rep (96 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 54.7

## op 3 cuCtxCreate: 5 repetition(s), 1872.8 trapped accesses per repetition, window median 352297 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 642.0 | 36624 |
| mmu-invalidate | 487.0 | 27581 |
| bar2-flush-read | 272.2 | 63896 |
| gsp-falcon | 255.0 | 12103 |
| gsp-queue-doorbell | 85.0 | 14080 |
| usermode page, other | 60.0 | 2948 |
| pcfg-mirror (intr re-arm) | 43.6 | 7151 |
| usermode-doorbell | 20.0 | 10965 |
| bar0-other | 8.0 | 1090 |

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
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 43.60 |
| (rest) | | | | 88.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 642.0, tokens 20.0

- GSP RPCs 85.0 per rep (1170 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.0 per rep: 19619 leaves mapped and 252 unmapped in 118.0 RM map call(s), walk 2350 us, worker apply 79335 us per rep
- events naming a host RM call 357.8 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x211.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x52.6; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X (X/X) [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 191.0; guest MMU invalidates 97.4; host doorbells written (forwarded) 211.0; host notifier events -> MSI 52.6; other events 562.4

## op 4 cuCtxDestroy: 5 repetition(s), 744.8 trapped accesses per repetition, window median 132961 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 12557 |
| gsp-falcon | 207.0 | 11244 |
| bar2-flush-read | 169.0 | 46674 |
| gsp-queue-doorbell | 69.0 | 33832 |
| pcfg-mirror (intr re-arm) | 19.8 | 2157 |

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
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 19.80 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (464 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 16.0 per rep: 0 leaves mapped and 17286 unmapped in 0.0 RM map call(s), walk 253 us, worker apply 16270 us per rep
- events naming a host RM call 50.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x24.0; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0; shadow: VAS of guest root X freed: N syncs (N skipped clean, N full walks, N table reads,  x0.6
- device events per rep: guest MMU invalidates 56.0; host notifier events -> MSI 24.0; other events 154.6

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.6 trapped accesses per repetition, window median 493 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 208 |
| bar2-flush-read | 1.0 | 270 |
| pcfg-mirror (intr re-arm) | 0.1 | 12 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 7 us, worker apply 368 us per rep
- events naming a host RM call 14.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.2
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 14.2; other events 0.5

## op 11 cuMemFree_1M: 20 repetition(s), 6.7 trapped accesses per repetition, window median 868 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 260 |
| bar2-flush-read | 1.5 | 264 |
| pcfg-mirror (intr re-arm) | 0.1 | 10 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 7 us, worker apply 187 us per rep
- events naming a host RM call 4.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.4
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.4; other events 0.6

## op 12 cuMemAlloc_64M: 10 repetition(s), 8.0 trapped accesses per repetition, window median 1018 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 289 |
| bar2-flush-read | 2.3 | 690 |
| pcfg-mirror (intr re-arm) | 0.2 | 10 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.8 per rep: 26 leaves mapped and 0 unmapped in 25.6 RM map call(s), walk 25 us, worker apply 3686 us per rep
- events naming a host RM call 5.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.1
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 5.1; other events 0.8

## op 13 cuMemFree_64M: 10 repetition(s), 13.4 trapped accesses per repetition, window median 1234 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 524 |
| bar2-flush-read | 3.1 | 617 |
| pcfg-mirror (intr re-arm) | 0.3 | 18 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 6 leaves mapped and 29 unmapped in 6.4 RM map call(s), walk 33 us, worker apply 8107 us per rep
- events naming a host RM call 4.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.6
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.6; other events 1.1

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.7 trapped accesses per repetition, window median 1508 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 281 |
| bar2-flush-read | 2.5 | 787 |
| pcfg-mirror (intr re-arm) | 0.2 | 14 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 0.7 RM map call(s), walk 22 us, worker apply 932 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.5; other events 0.5

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.1 trapped accesses per repetition, window median 1364 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 315 |
| bar2-flush-read | 2.0 | 721 |
| pcfg-mirror (intr re-arm) | 0.1 | 49 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.2 RM map call(s), walk 18 us, worker apply 1751 us per rep
- events naming a host RM call 4.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.2
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.2; other events 0.6

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.3 trapped accesses per repetition, window median 1693 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 639 |
| bar2-flush-read | 5.0 | 1254 |
| pcfg-mirror (intr re-arm) | 0.3 | 39 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 1.7 RM map call(s), walk 42 us, worker apply 280 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.0; other events 0.9

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.2 trapped accesses per repetition, window median 1461 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 530 |
| bar2-flush-read | 4.0 | 925 |
| pcfg-mirror (intr re-arm) | 0.2 | 56 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 26 leaves mapped and 230 unmapped in 0.1 RM map call(s), walk 15 us, worker apply 553 us per rep
- events naming a host RM call 4.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.8
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.8; other events 1.0

## op 20 launch_1_sync: 20 repetition(s), 3.1 trapped accesses per repetition, window median 576 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 142 |
| pcfg-mirror (intr re-arm) | 0.1 | 67 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 21 launch_1000_sync: 1 repetition(s), 3019.0 trapped accesses per repetition, window median 126471 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3000.0 | 117005 |
| pcfg-mirror (intr re-arm) | 19.0 | 4569 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1984.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1016.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 19.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3000.0

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1000.0

## op 30 h2d_pinned_1M: 10 repetition(s), 3.1 trapped accesses per repetition, window median 394 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 202 |
| pcfg-mirror (intr re-arm) | 0.1 | 6 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 31 d2h_pinned_1M: 10 repetition(s), 3.1 trapped accesses per repetition, window median 291 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 176 |
| pcfg-mirror (intr re-arm) | 0.1 | 8 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 32 d2d_1M: 10 repetition(s), 3.0 trapped accesses per repetition, window median 297 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 322 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 33 h2d_pinned_16M: 10 repetition(s), 3.3 trapped accesses per repetition, window median 2714 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 156 |
| pcfg-mirror (intr re-arm) | 0.3 | 382 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 34 d2h_pinned_16M: 10 repetition(s), 3.5 trapped accesses per repetition, window median 2760 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 150 |
| pcfg-mirror (intr re-arm) | 0.5 | 146 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 35 d2d_16M: 10 repetition(s), 3.0 trapped accesses per repetition, window median 1613 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 176 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 36 h2d_pageable_1M: 10 repetition(s), 3.2 trapped accesses per repetition, window median 480 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 302 |
| pcfg-mirror (intr re-arm) | 0.2 | 57 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 10 repetition(s), 3.1 trapped accesses per repetition, window median 414 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 188 |
| pcfg-mirror (intr re-arm) | 0.1 | 17 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1.90 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.10 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.1 trapped accesses per repetition, window median 48 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 3 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 3.2 trapped accesses per repetition, window median 778 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 155 |
| pcfg-mirror (intr re-arm) | 0.2 | 82 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1.95 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.05 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 50 cuModuleLoad_cubin: 10 repetition(s), 3.2 trapped accesses per repetition, window median 907 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 319 |
| pcfg-mirror (intr re-arm) | 0.2 | 58 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 51 cuModuleUnload: 10 repetition(s), 0.1 trapped accesses per repetition, window median 68 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 7 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none



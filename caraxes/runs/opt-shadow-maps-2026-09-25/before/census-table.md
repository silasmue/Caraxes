# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a7e883e004e873949/caraxes/runs/opt-shadow-maps-2026-09-25/before/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 15.1 | 100170 | 100105.1 |
| 2 | cuInit | 3 | 174.3 | 13937 | 13846.8 |
| 3 | cuCtxCreate | 5 | 1879.8 | 815087 | 815037.8 |
| 4 | cuCtxDestroy | 5 | 749.0 | 170131 | 170081.1 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 372 | 319.1 |
| 11 | cuMemFree_1M | 20 | 6.6 | 498 | 442.1 |
| 12 | cuMemAlloc_64M | 10 | 7.9 | 944 | 907.0 |
| 13 | cuMemFree_64M | 10 | 13.3 | 1244 | 1204.4 |
| 14 | cuMemHostAlloc_1M | 10 | 7.8 | 2114 | 2046.1 |
| 15 | cuMemFreeHost_1M | 10 | 7.5 | 1148 | 1088.7 |
| 16 | cuMemHostRegister_1M | 10 | 15.3 | 1798 | 1749.6 |
| 17 | cuMemHostUnregister_1M | 10 | 14.4 | 1590 | 1553.2 |
| 20 | launch_1_sync | 20 | 3.3 | 971 | 912.1 |
| 21 | launch_1000_sync | 1 | 3019.0 | 133419 | 109.4 |
| 30 | h2d_pinned_1M | 10 | 3.1 | 529 | 497.7 |
| 31 | d2h_pinned_1M | 10 | 3.4 | 608 | 546.1 |
| 32 | d2d_1M | 10 | 3.0 | 1127 | 1070.0 |
| 33 | h2d_pinned_16M | 10 | 3.8 | 5326 | 5284.9 |
| 34 | d2h_pinned_16M | 10 | 3.6 | 5373 | 5333.8 |
| 35 | d2d_16M | 10 | 3.2 | 4178 | 4136.1 |
| 36 | h2d_pageable_1M | 10 | 3.0 | 638 | 586.4 |
| 37 | d2h_pageable_1M | 10 | 3.1 | 636 | 585.2 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 58 | 12.4 |
| 41 | cuEventRecord_Synchronize | 20 | 3.4 | 864 | 808.7 |
| 50 | cuModuleLoad_cubin | 10 | 3.1 | 828 | 781.2 |
| 51 | cuModuleUnload | 10 | 0.0 | 56 | 17.4 |

## op 1 idle_100ms: 10 repetition(s), 15.1 trapped accesses per repetition, window median 100170 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 15.1 | 624 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 15.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none


## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 13937 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 2812 |
| mmu-invalidate | 40.0 | 1215 |
| gsp-queue-doorbell | 27.3 | 8805 |
| bar2-flush-read | 24.0 | 1511 |
| pcfg-mirror (intr re-arm) | 1.0 | 28 |

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

- GSP RPCs 27.3 per rep (85 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 54.7

## op 3 cuCtxCreate: 5 repetition(s), 1879.8 trapped accesses per repetition, window median 815087 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 642.0 | 34122 |
| mmu-invalidate | 487.0 | 20363 |
| bar2-flush-read | 272.2 | 63416 |
| gsp-falcon | 255.0 | 9319 |
| gsp-queue-doorbell | 85.0 | 11885 |
| usermode page, other | 60.0 | 2683 |
| pcfg-mirror (intr re-arm) | 50.6 | 4642 |
| usermode-doorbell | 20.0 | 1061 |
| bar0-other | 8.0 | 1495 |

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
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 50.60 |
| (rest) | | | | 88.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 642.0, tokens 20.0

- GSP RPCs 85.0 per rep (315 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.8 per rep: 19619 leaves mapped and 5987 unmapped in 7333.8 RM map call(s), walk 1910 us, worker apply 769986 us per rep
- events naming a host RM call 463.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x214.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x154.2; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X (X/X) [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 194.0; guest MMU invalidates 97.4; host doorbells written (forwarded) 214.0; host notifier events -> MSI 154.2; other events 567.0

## op 4 cuCtxDestroy: 5 repetition(s), 749.0 trapped accesses per repetition, window median 170131 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 10912 |
| gsp-falcon | 207.0 | 8261 |
| bar2-flush-read | 169.0 | 46573 |
| gsp-queue-doorbell | 69.0 | 26705 |
| pcfg-mirror (intr re-arm) | 24.0 | 2179 |

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
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 24.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (222 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0; GSP_RM_CONTROL cmd 0x802009 x1.0
- shadow plans 15.2 per rep: 0 leaves mapped and 11551 unmapped in 0.0 RM map call(s), walk 161 us, worker apply 38256 us per rep
- events naming a host RM call 57.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x31.6; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events -> MSI 31.6; other events 153.2

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 372 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 136 |
| bar2-flush-read | 1.0 | 209 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 8 us, worker apply 370 us per rep
- events naming a host RM call 14.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.3
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 14.3; other events 0.5

## op 11 cuMemFree_1M: 20 repetition(s), 6.6 trapped accesses per repetition, window median 498 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 224 |
| bar2-flush-read | 1.5 | 252 |
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

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 7 us, worker apply 443 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.5; other events 0.5

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.9 trapped accesses per repetition, window median 944 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 284 |
| bar2-flush-read | 2.3 | 613 |
| pcfg-mirror (intr re-arm) | 0.1 | 5 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 30 us, worker apply 7495 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 5.0; other events 1.0

## op 13 cuMemFree_64M: 10 repetition(s), 13.3 trapped accesses per repetition, window median 1244 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 604 |
| bar2-flush-read | 3.1 | 660 |
| pcfg-mirror (intr re-arm) | 0.2 | 170 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 33 us, worker apply 3180 us per rep
- events naming a host RM call 5.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.2
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.2; other events 1.0

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 2114 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 236 |
| bar2-flush-read | 2.5 | 848 |
| pcfg-mirror (intr re-arm) | 0.3 | 203 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 12.7 RM map call(s), walk 21 us, worker apply 1724 us per rep
- events naming a host RM call 4.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.6
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.6; other events 0.5

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 1148 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 197 |
| bar2-flush-read | 2.0 | 776 |
| pcfg-mirror (intr re-arm) | 0.5 | 50 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 51 leaves mapped and 205 unmapped in 0.2 RM map call(s), walk 12 us, worker apply 442 us per rep
- events naming a host RM call 5.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.3
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.3; other events 0.5

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.3 trapped accesses per repetition, window median 1798 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 530 |
| bar2-flush-read | 5.0 | 1234 |
| pcfg-mirror (intr re-arm) | 0.3 | 35 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 230 leaves mapped and 51 unmapped in 31.9 RM map call(s), walk 26 us, worker apply 4390 us per rep
- events naming a host RM call 4.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.9
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.9; other events 1.0

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.4 trapped accesses per repetition, window median 1590 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 470 |
| bar2-flush-read | 4.0 | 1106 |
| pcfg-mirror (intr re-arm) | 0.4 | 15 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 26 leaves mapped and 230 unmapped in 0.1 RM map call(s), walk 18 us, worker apply 2422 us per rep
- events naming a host RM call 4.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.9
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.9; other events 1.0

## op 20 launch_1_sync: 20 repetition(s), 3.3 trapped accesses per repetition, window median 971 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 176 |
| pcfg-mirror (intr re-arm) | 0.3 | 144 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 21 launch_1000_sync: 1 repetition(s), 3019.0 trapped accesses per repetition, window median 133419 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3000.0 | 122300 |
| pcfg-mirror (intr re-arm) | 19.0 | 11070 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1984.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1016.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 19.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3000.0

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1000.0

## op 30 h2d_pinned_1M: 10 repetition(s), 3.1 trapped accesses per repetition, window median 529 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 137 |
| pcfg-mirror (intr re-arm) | 0.1 | 8 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 31 d2h_pinned_1M: 10 repetition(s), 3.4 trapped accesses per repetition, window median 608 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 182 |
| pcfg-mirror (intr re-arm) | 0.4 | 87 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 32 d2d_1M: 10 repetition(s), 3.0 trapped accesses per repetition, window median 1127 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 187 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 33 h2d_pinned_16M: 10 repetition(s), 3.8 trapped accesses per repetition, window median 5326 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 159 |
| pcfg-mirror (intr re-arm) | 0.8 | 237 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.80 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 34 d2h_pinned_16M: 10 repetition(s), 3.6 trapped accesses per repetition, window median 5373 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 159 |
| pcfg-mirror (intr re-arm) | 0.6 | 38 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.60 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 35 d2d_16M: 10 repetition(s), 3.2 trapped accesses per repetition, window median 4178 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 147 |
| pcfg-mirror (intr re-arm) | 0.2 | 122 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 36 h2d_pageable_1M: 10 repetition(s), 3.0 trapped accesses per repetition, window median 638 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 250 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 10 repetition(s), 3.1 trapped accesses per repetition, window median 636 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 147 |
| pcfg-mirror (intr re-arm) | 0.1 | 4 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1.90 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.10 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 58 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 3.4 trapped accesses per repetition, window median 864 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 157 |
| pcfg-mirror (intr re-arm) | 0.3 | 57 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 1.95 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.05 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.35 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 50 cuModuleLoad_cubin: 10 repetition(s), 3.1 trapped accesses per repetition, window median 828 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 247 |
| pcfg-mirror (intr re-arm) | 0.1 | 51 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 64 MiB) | W |  | 2.00 |
| BAR1 | [0, 64 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 56 us



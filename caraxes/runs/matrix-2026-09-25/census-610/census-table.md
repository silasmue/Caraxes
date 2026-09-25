# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-ad0bd7a6ec1b6cc05/caraxes/runs/matrix-2026-09-25/census-610/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 51.3 | 100202 | 100091.4 |
| 2 | cuInit | 3 | 174.3 | 21430 | 21345.8 |
| 3 | cuCtxCreate | 5 | 1289.8 | 436654 | 436605.5 |
| 4 | cuCtxDestroy | 5 | 827.6 | 173638 | 173582.9 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 386 | 313.0 |
| 11 | cuMemFree_1M | 20 | 6.5 | 544 | 481.8 |
| 12 | cuMemAlloc_64M | 10 | 7.8 | 1028 | 966.2 |
| 13 | cuMemFree_64M | 10 | 13.1 | 1528 | 1471.3 |
| 14 | cuMemHostAlloc_1M | 10 | 7.5 | 1972 | 1899.5 |
| 15 | cuMemFreeHost_1M | 10 | 7.0 | 1375 | 1307.2 |
| 16 | cuMemHostRegister_1M | 10 | 15.0 | 2131 | 2066.9 |
| 17 | cuMemHostUnregister_1M | 10 | 14.0 | 1844 | 1789.0 |
| 20 | launch_1_sync | 20 | 0.0 | 759 | 690.3 |
| 21 | launch_1000_sync | 1 | 0.0 | 56255 | 20.1 |
| 30 | h2d_pinned_1M | 10 | 2.7 | 466 | 396.8 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 451 | 385.4 |
| 32 | d2d_1M | 10 | 0.0 | 980 | 916.6 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 5245 | 5158.8 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 5326 | 5249.1 |
| 35 | d2d_16M | 10 | 0.0 | 3913 | 3832.9 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 578 | 512.8 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 574 | 511.1 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 74 | 13.5 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 694 | 654.7 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 820 | 770.6 |
| 51 | cuModuleUnload | 10 | 0.0 | 68 | 25.7 |

## op 1 idle_100ms: 10 repetition(s), 51.3 trapped accesses per repetition, window median 100202 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 51.3 | 1078 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x34 | 5.80 |
| PCI config |  | R | 0x60 | 5.80 |
| PCI config |  | R | 0x68 | 5.80 |
| PCI config |  | R | 0x78 | 5.80 |
| PCI config |  | R | 0x6 | 5.70 |
| PCI config |  | R | 0x61 | 5.70 |
| PCI config |  | R | 0x69 | 5.70 |
| PCI config |  | R | 0x79 | 5.70 |
| PCI config |  | R | 0x0 | 0.10 |
| PCI config |  | R | 0x4 | 0.10 |
| PCI config |  | R | 0x8 | 0.10 |
| PCI config |  | R | 0xc | 0.10 |
| (rest) | | | | 4.90 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- device events per rep: other events 15.4

## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 21430 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 4800 |
| mmu-invalidate | 40.0 | 1904 |
| gsp-queue-doorbell | 27.3 | 9668 |
| bar2-flush-read | 24.0 | 2273 |
| pcfg-mirror (intr re-arm) | 1.0 | 32 |

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

- GSP RPCs 27.3 per rep (101 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 55.7

## op 3 cuCtxCreate: 5 repetition(s), 1289.8 trapped accesses per repetition, window median 436654 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 487.0 | 22900 |
| bar2-flush-read | 272.2 | 65347 |
| gsp-falcon | 255.0 | 11352 |
| pci-config | 102.6 | 2058 |
| gsp-queue-doorbell | 85.0 | 12568 |
| usermode page, other | 60.0 | 2640 |
| usermode-doorbell | 20.0 | 2776 |
| bar0-other | 8.0 | 1384 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 272.20 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 194.80 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.40 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.40 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.40 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 85.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 85.00 |
| BAR0 | 0x110000 | R | 0x110804 NV_PGSP | 85.00 |
| BAR0 | 0x110000 | R | 0x110808 NV_PGSP | 85.00 |
| BAR0 | 0xbb0000 | R | 0xbb0084 NV_VIRTUAL_FUNCTION_TIME_0/1 | 40.00 |
| BAR0 | 0xbb0000 | R | 0xbb0080 NV_VIRTUAL_FUNCTION_TIME_0/1 | 20.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 20.00 |
| (rest) | | | | 110.60 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): tokens 20.0

- GSP RPCs 85.0 per rep (339 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 12.0 per rep: 20130 leaves mapped and 252 unmapped in 108.6 RM map call(s), walk 3429 us, worker apply 70355 us per rep
- events naming a host RM call 319.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x165.6; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x47.0; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X (X/X) [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 145.6; guest MMU invalidates 97.4; host doorbells written (forwarded) 165.6; host notifier events (no MSI) 12.4; host notifier events -> MSI 47.0; other events 551.0

## op 4 cuCtxDestroy: 5 repetition(s), 827.6 trapped accesses per repetition, window median 173638 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 13329 |
| gsp-falcon | 207.0 | 9409 |
| bar2-flush-read | 169.0 | 49610 |
| pci-config | 102.6 | 2068 |
| gsp-queue-doorbell | 69.0 | 27110 |

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
| PCI config |  | R | 0x34 | 11.60 |
| PCI config |  | R | 0x60 | 11.60 |
| PCI config |  | R | 0x68 | 11.60 |
| (rest) | | | | 67.80 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (226 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 3.8 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 259 us, worker apply 16944 us per rep
- events naming a host RM call 57.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x29.2; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0; shadow: VAS of guest root X freed: N syncs (N skipped clean, N full walks, N table reads,  x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 0.8; host notifier events -> MSI 29.2; other events 172.0

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 386 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 272 |
| bar2-flush-read | 1.0 | 261 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.2 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 4 us, worker apply 694 us per rep
- events naming a host RM call 14.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.5
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 14.5; other events 0.8

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 544 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 355 |
| bar2-flush-read | 1.5 | 241 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 7 us, worker apply 90 us per rep
- events naming a host RM call 4.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.3
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.3; other events 1.1

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 1028 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 316 |
| bar2-flush-read | 2.3 | 636 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 28 us, worker apply 5074 us per rep
- events naming a host RM call 5.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.1
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 5.1; other events 1.1

## op 13 cuMemFree_64M: 10 repetition(s), 13.1 trapped accesses per repetition, window median 1528 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 726 |
| bar2-flush-read | 3.1 | 781 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.8 per rep: 0 leaves mapped and 26 unmapped in 0.0 RM map call(s), walk 26 us, worker apply 3773 us per rep
- events naming a host RM call 4.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.8
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.8; other events 2.2

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 1972 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 426 |
| bar2-flush-read | 2.5 | 1155 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 0.6 RM map call(s), walk 29 us, worker apply 1084 us per rep
- events naming a host RM call 4.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.4
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.4; other events 1.3

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.0 trapped accesses per repetition, window median 1375 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 617 |
| bar2-flush-read | 2.0 | 836 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.4 per rep: 0 leaves mapped and 205 unmapped in 0.0 RM map call(s), walk 8 us, worker apply 745 us per rep
- events naming a host RM call 5.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.2
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.2; other events 1.1

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.0 trapped accesses per repetition, window median 2131 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 748 |
| bar2-flush-read | 5.0 | 1497 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 1.5 RM map call(s), walk 31 us, worker apply 1035 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.0; other events 2.3

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.0 trapped accesses per repetition, window median 1844 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 631 |
| bar2-flush-read | 4.0 | 1264 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 0 leaves mapped and 230 unmapped in 0.0 RM map call(s), walk 14 us, worker apply 1268 us per rep
- events naming a host RM call 4.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.7
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.7; other events 2.2

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 759 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 56255 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1006.0

## op 30 h2d_pinned_1M: 10 repetition(s), 2.7 trapped accesses per repetition, window median 466 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 2.7 | 47 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x6 | 0.30 |
| PCI config |  | R | 0x34 | 0.30 |
| PCI config |  | R | 0x60 | 0.30 |
| PCI config |  | R | 0x61 | 0.30 |
| PCI config |  | R | 0x68 | 0.30 |
| PCI config |  | R | 0x69 | 0.30 |
| PCI config |  | R | 0x78 | 0.30 |
| PCI config |  | R | 0x79 | 0.30 |
| PCI config |  | R | 0x54 | 0.10 |
| PCI config |  | R | 0x58 | 0.10 |
| PCI config |  | R | 0x5c | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 451 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 980 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.3

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5245 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.7

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5326 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.7

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 3913 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 578 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 574 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 74 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 694 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 820 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 68 us



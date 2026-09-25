# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a502fb60566dbcfc2/caraxes/runs/opt-memfd-pool-2026-09-25/on/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 20 | 38.6 | 100150 | 100079.3 |
| 2 | cuInit | 3 | 174.3 | 28266 | 28173.6 |
| 3 | cuCtxCreate | 3 | 1228.3 | 610709 | 610660.9 |
| 4 | cuCtxDestroy | 3 | 925.7 | 204837 | 204755.1 |
| 10 | cuMemAlloc_1M | 40 | 3.8 | 476 | 387.5 |
| 11 | cuMemFree_1M | 40 | 6.6 | 529 | 481.6 |
| 12 | cuMemAlloc_64M | 20 | 8.2 | 1054 | 970.2 |
| 13 | cuMemFree_64M | 20 | 13.3 | 1550 | 1487.2 |
| 14 | cuMemHostAlloc_1M | 20 | 8.2 | 1746 | 1721.8 |
| 15 | cuMemFreeHost_1M | 20 | 7.7 | 1362 | 1244.6 |
| 16 | cuMemHostRegister_1M | 20 | 15.4 | 2299 | 2285.1 |
| 17 | cuMemHostUnregister_1M | 20 | 14.3 | 2110 | 2043.4 |
| 20 | launch_1_sync | 40 | 0.3 | 792 | 742.1 |
| 21 | launch_1000_sync | 2 | 4.5 | 30176 | 20.4 |
| 30 | h2d_pinned_1M | 20 | 0.1 | 479 | 435.1 |
| 31 | d2h_pinned_1M | 20 | 1.4 | 468 | 402.0 |
| 32 | d2d_1M | 20 | 0.2 | 850 | 802.2 |
| 33 | h2d_pinned_16M | 20 | 0.5 | 5094 | 3927.0 |
| 34 | d2h_pinned_16M | 20 | 0.7 | 5260 | 3976.9 |
| 35 | d2d_16M | 20 | 0.7 | 3523 | 3284.6 |
| 36 | h2d_pageable_1M | 20 | 0.1 | 556 | 527.3 |
| 37 | d2h_pageable_1M | 20 | 0.2 | 518 | 432.3 |
| 40 | cuStreamSynchronize_idle | 40 | 0.2 | 59 | 9.3 |
| 41 | cuEventRecord_Synchronize | 40 | 0.6 | 1951 | 1369.5 |
| 50 | cuModuleLoad_cubin | 20 | 0.1 | 386 | 330.4 |
| 51 | cuModuleUnload | 20 | 0.0 | 74 | 24.8 |

## op 1 idle_100ms: 20 repetition(s), 38.6 trapped accesses per repetition, window median 100150 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 25.6 | 430 |
| pcfg-mirror (intr re-arm) | 12.9 | 467 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 12.95 |
| PCI config |  | R | 0x34 | 2.90 |
| PCI config |  | R | 0x60 | 2.90 |
| PCI config |  | R | 0x68 | 2.90 |
| PCI config |  | R | 0x78 | 2.90 |
| PCI config |  | R | 0x6 | 2.85 |
| PCI config |  | R | 0x61 | 2.85 |
| PCI config |  | R | 0x69 | 2.85 |
| PCI config |  | R | 0x79 | 2.85 |
| PCI config |  | R | 0x0 | 0.05 |
| PCI config |  | R | 0x4 | 0.05 |
| PCI config |  | R | 0x8 | 0.05 |
| (rest) | | | | 2.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none


## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 28266 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 4486 |
| mmu-invalidate | 40.0 | 2359 |
| gsp-queue-doorbell | 27.3 | 9053 |
| bar2-flush-read | 24.0 | 2250 |
| pcfg-mirror (intr re-arm) | 1.0 | 48 |

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

- GSP RPCs 27.3 per rep (102 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 54.7

## op 3 cuCtxCreate: 3 repetition(s), 1228.3 trapped accesses per repetition, window median 610709 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 488.3 | 25786 |
| bar2-flush-read | 273.0 | 70923 |
| gsp-falcon | 255.0 | 10969 |
| gsp-queue-doorbell | 85.0 | 13757 |
| usermode page, other | 60.0 | 3023 |
| pcfg-mirror (intr re-arm) | 39.0 | 5415 |
| usermode-doorbell | 20.0 | 4289 |
| bar0-other | 8.0 | 1321 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 273.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 195.33 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.67 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.67 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.67 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 85.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 85.00 |
| BAR0 | 0x110000 | R | 0x110804 NV_PGSP | 85.00 |
| BAR0 | 0x110000 | R | 0x110808 NV_PGSP | 85.00 |
| BAR0 | 0xbb0000 | R | 0xbb0084 NV_VIRTUAL_FUNCTION_TIME_0/1 | 40.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 39.00 |
| BAR0 | 0xbb0000 | R | 0xbb0080 NV_VIRTUAL_FUNCTION_TIME_0/1 | 20.00 |
| (rest) | | | | 28.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): tokens 20.0

- GSP RPCs 85.0 per rep (394 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 19.7 per rep: 20046 leaves mapped and 168 unmapped in 6175.3 RM map call(s), walk 4015 us, worker apply 180617 us per rep
- events naming a host RM call 339.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x169.7; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x72.3; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X (X/X) [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 149.7; guest MMU invalidates 97.7; host doorbells written (forwarded) 169.7; host notifier events (no MSI) 2.3; host notifier events -> MSI 72.3; other events 514.0

## op 4 cuCtxDestroy: 3 repetition(s), 925.7 trapped accesses per repetition, window median 204837 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 18771 |
| gsp-falcon | 207.0 | 14476 |
| pci-config | 171.0 | 5671 |
| bar2-flush-read | 169.0 | 50217 |
| gsp-queue-doorbell | 69.0 | 36018 |
| pcfg-mirror (intr re-arm) | 29.7 | 4696 |

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
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 29.67 |
| PCI config |  | R | 0x34 | 19.33 |
| PCI config |  | R | 0x60 | 19.33 |
| (rest) | | | | 132.33 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (369 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 16.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 760 us, worker apply 559412 us per rep
- events naming a host RM call 132.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x105.7; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.3
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 0.3; host notifier events -> MSI 105.7; other events 154.0

## op 10 cuMemAlloc_1M: 40 repetition(s), 3.8 trapped accesses per repetition, window median 476 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 205 |
| bar2-flush-read | 1.0 | 225 |
| pci-config | 0.2 | 5 |
| pcfg-mirror (intr re-arm) | 0.0 | 4 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| PCI config |  | R | 0x6 | 0.03 |
| PCI config |  | R | 0x34 | 0.03 |
| PCI config |  | R | 0x60 | 0.03 |
| PCI config |  | R | 0x61 | 0.03 |
| PCI config |  | R | 0x68 | 0.03 |
| PCI config |  | R | 0x69 | 0.03 |
| PCI config |  | R | 0x78 | 0.03 |
| (rest) | | | | 0.07 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 8 us, worker apply 144 us per rep
- events naming a host RM call 12.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x12.8; shadow: VAS of guest root X freed: N syncs (N skipped clean, N full walks, N table reads,  x0.0
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 12.8; other events 0.5

## op 11 cuMemFree_1M: 40 repetition(s), 6.6 trapped accesses per repetition, window median 529 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 428 |
| bar2-flush-read | 1.5 | 322 |
| pcfg-mirror (intr re-arm) | 0.1 | 56 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 8 us, worker apply 107 us per rep
- events naming a host RM call 4.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.3
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.3; other events 0.5

## op 12 cuMemAlloc_64M: 20 repetition(s), 8.2 trapped accesses per repetition, window median 1054 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 519 |
| bar2-flush-read | 2.3 | 651 |
| pcfg-mirror (intr re-arm) | 0.4 | 108 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 40 us, worker apply 2723 us per rep
- events naming a host RM call 5.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.6
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 5.6; other events 0.9

## op 13 cuMemFree_64M: 20 repetition(s), 13.3 trapped accesses per repetition, window median 1550 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 733 |
| bar2-flush-read | 3.1 | 740 |
| pcfg-mirror (intr re-arm) | 0.2 | 14 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 42 us, worker apply 3313 us per rep
- events naming a host RM call 4.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.9
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.9; other events 1.0

## op 14 cuMemHostAlloc_1M: 20 repetition(s), 8.2 trapped accesses per repetition, window median 1746 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 469 |
| bar2-flush-read | 2.5 | 1288 |
| pci-config | 0.5 | 7 |
| pcfg-mirror (intr re-arm) | 0.3 | 87 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |
| PCI config |  | R | 0x6 | 0.05 |
| PCI config |  | R | 0x34 | 0.05 |
| PCI config |  | R | 0x60 | 0.05 |
| PCI config |  | R | 0x61 | 0.05 |
| PCI config |  | R | 0x68 | 0.05 |
| PCI config |  | R | 0x69 | 0.05 |
| (rest) | | | | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 28.5 RM map call(s), walk 37 us, worker apply 1281 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.5; other events 0.5

## op 15 cuMemFreeHost_1M: 20 repetition(s), 7.7 trapped accesses per repetition, window median 1362 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 375 |
| bar2-flush-read | 2.0 | 841 |
| pci-config | 0.5 | 13 |
| pcfg-mirror (intr re-arm) | 0.2 | 52 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.25 |
| PCI config |  | R | 0x6 | 0.05 |
| PCI config |  | R | 0x34 | 0.05 |
| PCI config |  | R | 0x60 | 0.05 |
| PCI config |  | R | 0x61 | 0.05 |
| PCI config |  | R | 0x68 | 0.05 |
| PCI config |  | R | 0x69 | 0.05 |
| (rest) | | | | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 51 leaves mapped and 205 unmapped in 0.5 RM map call(s), walk 13 us, worker apply 2574 us per rep
- events naming a host RM call 5.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.1
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.1; other events 0.5

## op 16 cuMemHostRegister_1M: 20 repetition(s), 15.4 trapped accesses per repetition, window median 2299 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1080 |
| bar2-flush-read | 5.0 | 1540 |
| pcfg-mirror (intr re-arm) | 0.4 | 115 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 230 leaves mapped and 51 unmapped in 36.6 RM map call(s), walk 38 us, worker apply 4364 us per rep
- events naming a host RM call 5.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.2
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.2; other events 1.0

## op 17 cuMemHostUnregister_1M: 20 repetition(s), 14.3 trapped accesses per repetition, window median 2110 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 691 |
| bar2-flush-read | 4.0 | 1197 |
| pcfg-mirror (intr re-arm) | 0.3 | 22 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.35 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 26 leaves mapped and 218 unmapped in 0.5 RM map call(s), walk 20 us, worker apply 2575 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.0; other events 0.9

## op 20 launch_1_sync: 40 repetition(s), 0.3 trapped accesses per repetition, window median 792 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.3 | 125 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.28 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 21 launch_1000_sync: 2 repetition(s), 4.5 trapped accesses per repetition, window median 30176 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 4.5 | 721 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 4.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1000.0

## op 30 h2d_pinned_1M: 20 repetition(s), 0.1 trapped accesses per repetition, window median 479 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 23 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 31 d2h_pinned_1M: 20 repetition(s), 1.4 trapped accesses per repetition, window median 468 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 1.4 | 37 |
| pcfg-mirror (intr re-arm) | 0.1 | 13 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x6 | 0.15 |
| PCI config |  | R | 0x34 | 0.15 |
| PCI config |  | R | 0x60 | 0.15 |
| PCI config |  | R | 0x61 | 0.15 |
| PCI config |  | R | 0x68 | 0.15 |
| PCI config |  | R | 0x69 | 0.15 |
| PCI config |  | R | 0x78 | 0.15 |
| PCI config |  | R | 0x79 | 0.15 |
| PCI config |  | R | 0xc0 | 0.05 |
| PCI config |  | R | 0xc4 | 0.05 |
| PCI config |  | R | 0xc8 | 0.05 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 32 d2d_1M: 20 repetition(s), 0.2 trapped accesses per repetition, window median 850 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.2 | 100 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 33 h2d_pinned_16M: 20 repetition(s), 0.5 trapped accesses per repetition, window median 5094 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.5 | 322 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 34 d2h_pinned_16M: 20 repetition(s), 0.7 trapped accesses per repetition, window median 5260 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.7 | 120 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.65 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 35 d2d_16M: 20 repetition(s), 0.7 trapped accesses per repetition, window median 3523 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.7 | 223 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.70 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 36 h2d_pageable_1M: 20 repetition(s), 0.1 trapped accesses per repetition, window median 556 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 39 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 20 repetition(s), 0.2 trapped accesses per repetition, window median 518 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.2 | 56 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.25 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 40 repetition(s), 0.2 trapped accesses per repetition, window median 59 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 0.2 | 4 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x6 | 0.03 |
| PCI config |  | R | 0x34 | 0.03 |
| PCI config |  | R | 0x60 | 0.03 |
| PCI config |  | R | 0x61 | 0.03 |
| PCI config |  | R | 0x68 | 0.03 |
| PCI config |  | R | 0x69 | 0.03 |
| PCI config |  | R | 0x78 | 0.03 |
| PCI config |  | R | 0x79 | 0.03 |
| PCI config |  | R | 0x88 | 0.03 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none


## op 41 cuEventRecord_Synchronize: 40 repetition(s), 0.6 trapped accesses per repetition, window median 1951 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.6 | 58 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.57 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 50 cuModuleLoad_cubin: 20 repetition(s), 0.1 trapped accesses per repetition, window median 386 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 0 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 51 cuModuleUnload: 20 repetition(s), 0.0 trapped accesses per repetition, window median 74 us



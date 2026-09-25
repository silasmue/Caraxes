# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/part1/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 0.0 | 100182 | 100088.2 |
| 2 | cuInit | 3 | 174.3 | 30338 | 29639.3 |
| 3 | cuCtxCreate | 5 | 1289.8 | 406788 | 406735.7 |
| 4 | cuCtxDestroy | 5 | 725.0 | 156859 | 156821.9 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 856 | 405.0 |
| 11 | cuMemFree_1M | 20 | 7.0 | 682 | 553.0 |
| 12 | cuMemAlloc_64M | 10 | 7.8 | 3797 | 3720.5 |
| 13 | cuMemFree_64M | 10 | 13.1 | 1628 | 1562.5 |
| 14 | cuMemHostAlloc_1M | 10 | 7.5 | 2660 | 2559.5 |
| 15 | cuMemFreeHost_1M | 10 | 7.0 | 1380 | 1320.5 |
| 16 | cuMemHostRegister_1M | 10 | 15.0 | 4593 | 4499.1 |
| 17 | cuMemHostUnregister_1M | 10 | 35.6 | 3600 | 3257.0 |
| 20 | launch_1_sync | 20 | 0.0 | 1240 | 859.0 |
| 21 | launch_1000_sync | 1 | 0.0 | 368868 | 90.1 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 634 | 471.2 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 647 | 475.2 |
| 32 | d2d_1M | 10 | 0.0 | 4032 | 3584.7 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 5327 | 5246.9 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 5387 | 5294.8 |
| 35 | d2d_16M | 10 | 0.0 | 5126 | 4482.6 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 1606 | 1041.2 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 678 | 584.6 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 199 | 21.3 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 2358 | 1452.4 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 1926 | 1459.6 |
| 51 | cuModuleUnload | 10 | 0.0 | 454 | 157.1 |

## op 1 idle_100ms: 10 repetition(s), 0.0 trapped accesses per repetition, window median 100182 us

- device events per rep: other events 14.5

## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 30338 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 4500 |
| mmu-invalidate | 40.0 | 3021 |
| gsp-queue-doorbell | 27.3 | 9520 |
| bar2-flush-read | 24.0 | 4765 |
| pcfg-mirror (intr re-arm) | 1.0 | 97 |

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

- GSP RPCs 27.3 per rep (208 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 55.3

## op 3 cuCtxCreate: 5 repetition(s), 1289.8 trapped accesses per repetition, window median 406788 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 487.0 | 39956 |
| bar2-flush-read | 272.2 | 69666 |
| gsp-falcon | 255.0 | 12536 |
| pci-config | 102.6 | 2736 |
| gsp-queue-doorbell | 85.0 | 12710 |
| usermode page, other | 60.0 | 3052 |
| usermode-doorbell | 20.0 | 4390 |
| bar0-other | 8.0 | 1905 |

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

- GSP RPCs 85.0 per rep (480 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.0 per rep: 20130 leaves mapped and 252 unmapped in 107.6 RM map call(s), walk 2847 us, worker apply 68062 us per rep
- events naming a host RM call 310.8 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x152.4; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x64.0; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X (X/X) [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 132.4; guest MMU invalidates 97.4; host doorbells written (forwarded) 152.4; host notifier events -> MSI 64.0; other events 551.0

## op 4 cuCtxDestroy: 5 repetition(s), 725.0 trapped accesses per repetition, window median 156859 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 15705 |
| gsp-falcon | 207.0 | 11021 |
| bar2-flush-read | 169.0 | 48354 |
| gsp-queue-doorbell | 69.0 | 31176 |

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

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (428 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 386 us, worker apply 7043 us per rep
- events naming a host RM call 55.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x28.6; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0; shadow: VAS of guest root X freed: N syncs (N skipped clean, N full walks, N table reads,  x0.6
- device events per rep: guest MMU invalidates 56.0; host notifier events -> MSI 28.6; other events 177.6

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 856 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 244 |
| bar2-flush-read | 1.0 | 270 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 8 us, worker apply 66 us per rep
- events naming a host RM call 14.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.2
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 14.2; other events 0.7

## op 11 cuMemFree_1M: 20 repetition(s), 7.0 trapped accesses per repetition, window median 682 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 571 |
| bar2-flush-read | 1.5 | 408 |
| pci-config | 0.5 | 10 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| PCI config |  | R | 0x6 | 0.05 |
| PCI config |  | R | 0x34 | 0.05 |
| PCI config |  | R | 0x60 | 0.05 |
| PCI config |  | R | 0x61 | 0.05 |
| PCI config |  | R | 0x68 | 0.05 |
| PCI config |  | R | 0x69 | 0.05 |
| PCI config |  | R | 0x78 | 0.05 |
| (rest) | | | | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 17 us, worker apply 152 us per rep
- events naming a host RM call 4.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.6
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.6; other events 0.7

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 3797 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 439 |
| bar2-flush-read | 2.3 | 743 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 78 us, worker apply 7810 us per rep
- events naming a host RM call 6.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.6; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.6
- device events per rep: guest MMU invalidates 1.1; host notifier events (no MSI) 0.6; host notifier events -> MSI 5.6; other events 2.0

## op 13 cuMemFree_64M: 10 repetition(s), 13.1 trapped accesses per repetition, window median 1628 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 628 |
| bar2-flush-read | 3.1 | 800 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 47 us, worker apply 7606 us per rep
- events naming a host RM call 5.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.2
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.2; host notifier events -> MSI 5.0; other events 1.3

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 2660 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 497 |
| bar2-flush-read | 2.5 | 937 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 0.9 RM map call(s), walk 26 us, worker apply 525 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.5; other events 1.1

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.0 trapped accesses per repetition, window median 1380 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 597 |
| bar2-flush-read | 2.0 | 770 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 33 us, worker apply 1158 us per rep
- events naming a host RM call 5.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.4
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.4; other events 1.0

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.0 trapped accesses per repetition, window median 4593 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1585 |
| bar2-flush-read | 5.0 | 1673 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 2.0 RM map call(s), walk 55 us, worker apply 900 us per rep
- events naming a host RM call 5.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.3
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.3; other events 2.3

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 35.6 trapped accesses per repetition, window median 3600 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 21.6 | 471 |
| mmu-invalidate | 10.0 | 1335 |
| bar2-flush-read | 4.0 | 1465 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| PCI config |  | R | 0x60 | 2.50 |
| PCI config |  | R | 0x68 | 2.50 |
| PCI config |  | R | 0x78 | 2.50 |
| PCI config |  | R | 0x6 | 2.40 |
| PCI config |  | R | 0x34 | 2.40 |
| PCI config |  | R | 0x61 | 2.40 |
| PCI config |  | R | 0x69 | 2.40 |
| PCI config |  | R | 0x79 | 2.40 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| (rest) | | | | 4.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 26 leaves mapped and 230 unmapped in 0.4 RM map call(s), walk 48 us, worker apply 1623 us per rep
- events naming a host RM call 5.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.1
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.1; other events 1.4

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 1240 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.3

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 368868 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1050.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 634 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 647 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.4

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 4032 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.6

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5327 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.8

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5387 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.7

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5126 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.5

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1606 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 678 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 199 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 2358 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.4

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1926 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.8

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 454 us

- device events per rep: other events 0.1


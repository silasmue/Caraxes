# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/after-noprefetch/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 0.0 | 100230 | 100095.5 |
| 2 | cuInit | 3 | 174.3 | 81689 | 81516.6 |
| 3 | cuCtxCreate | 5 | 1187.2 | 800860 | 799282.8 |
| 4 | cuCtxDestroy | 5 | 827.6 | 333109 | 332995.8 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 960 | 797.1 |
| 11 | cuMemFree_1M | 20 | 6.5 | 886 | 682.5 |
| 12 | cuMemAlloc_64M | 10 | 7.8 | 6820 | 6717.9 |
| 13 | cuMemFree_64M | 10 | 13.1 | 3584 | 3384.1 |
| 14 | cuMemHostAlloc_1M | 10 | 7.5 | 4353 | 4207.5 |
| 15 | cuMemFreeHost_1M | 10 | 7.0 | 1932 | 1854.5 |
| 16 | cuMemHostRegister_1M | 10 | 15.0 | 7184 | 7104.7 |
| 17 | cuMemHostUnregister_1M | 10 | 14.0 | 4450 | 4388.4 |
| 20 | launch_1_sync | 20 | 0.0 | 810 | 737.7 |
| 21 | launch_1000_sync | 1 | 216.0 | 225581 | 29.3 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 721 | 642.7 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 676 | 485.9 |
| 32 | d2d_1M | 10 | 0.0 | 1209 | 1082.5 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 5384 | 5295.0 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 5730 | 5445.1 |
| 35 | d2d_16M | 10 | 4.5 | 3995 | 3863.0 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 704 | 619.9 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 710 | 628.7 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 78 | 16.4 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 778 | 711.8 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 872 | 779.7 |
| 51 | cuModuleUnload | 10 | 0.0 | 80 | 33.2 |

## op 1 idle_100ms: 10 repetition(s), 0.0 trapped accesses per repetition, window median 100230 us

- device events per rep: other events 16.4

## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 81689 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 18159 |
| mmu-invalidate | 40.0 | 5851 |
| gsp-queue-doorbell | 27.3 | 13173 |
| bar2-flush-read | 24.0 | 4918 |
| pcfg-mirror (intr re-arm) | 1.0 | 57 |

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

- GSP RPCs 27.3 per rep (222 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 55.7

## op 3 cuCtxCreate: 5 repetition(s), 1187.2 trapped accesses per repetition, window median 800860 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 487.0 | 76849 |
| bar2-flush-read | 272.2 | 99413 |
| gsp-falcon | 255.0 | 34617 |
| gsp-queue-doorbell | 85.0 | 32550 |
| usermode page, other | 60.0 | 8622 |
| usermode-doorbell | 20.0 | 3504 |
| bar0-other | 8.0 | 2749 |

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
| (rest) | | | | 8.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): tokens 20.0

- GSP RPCs 85.0 per rep (667 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.0 per rep: 20130 leaves mapped and 252 unmapped in 108.6 RM map call(s), walk 6223 us, worker apply 124799 us per rep
- events naming a host RM call 362.6 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x115.8; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x96.4; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x36.4; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0
- device events per rep: doorbell eventfd rings (token pool) 95.8; guest MMU invalidates 97.4; host doorbells written (forwarded) 115.8; host notifier events (no MSI) 36.4; host notifier events -> MSI 96.4; other events 585.2

## op 4 cuCtxDestroy: 5 repetition(s), 827.6 trapped accesses per repetition, window median 333109 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 42445 |
| gsp-falcon | 207.0 | 34466 |
| bar2-flush-read | 169.0 | 74140 |
| pci-config | 102.6 | 9817 |
| gsp-queue-doorbell | 69.0 | 40282 |

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

- GSP RPCs 69.0 per rep (525 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 778 us, worker apply 14899 us per rep
- events naming a host RM call 83.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x53.4; free X class Some(N); mirror: chid N: host channel X freed in N us after N doorbell(s), N  x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x2.6; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 2.6; host notifier events -> MSI 53.4; other events 205.8

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 960 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 519 |
| bar2-flush-read | 1.0 | 379 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 16 us, worker apply 1078 us per rep
- events naming a host RM call 17.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x17.3; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 0.5; host notifier events (no MSI) 0.1; host notifier events -> MSI 17.3; other events 0.9

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 886 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 763 |
| bar2-flush-read | 1.5 | 543 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 10 us, worker apply 621 us per rep
- events naming a host RM call 5.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.7
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.7; other events 0.8

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 6820 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 903 |
| bar2-flush-read | 2.3 | 1088 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 42 us, worker apply 4664 us per rep
- events naming a host RM call 8.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x8.5; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.3
- device events per rep: guest MMU invalidates 1.1; host notifier events (no MSI) 0.3; host notifier events -> MSI 8.5; other events 2.8

## op 13 cuMemFree_64M: 10 repetition(s), 13.1 trapped accesses per repetition, window median 3584 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1156 |
| bar2-flush-read | 3.1 | 1312 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 3 leaves mapped and 26 unmapped in 3.2 RM map call(s), walk 52 us, worker apply 3301 us per rep
- events naming a host RM call 10.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x9.9; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 9.9; other events 2.0

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 4353 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 1240 |
| bar2-flush-read | 2.5 | 1038 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 205 leaves mapped and 6 unmapped in 0.5 RM map call(s), walk 36 us, worker apply 5492 us per rep
- events naming a host RM call 5.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.3; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.2
- device events per rep: guest MMU invalidates 1.0; host notifier events (no MSI) 0.2; host notifier events -> MSI 5.3; other events 2.2

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.0 trapped accesses per repetition, window median 1932 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 750 |
| bar2-flush-read | 2.0 | 1300 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.2 RM map call(s), walk 28 us, worker apply 7701 us per rep
- events naming a host RM call 5.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.6
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.6; other events 1.0

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.0 trapped accesses per repetition, window median 7184 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 2741 |
| bar2-flush-read | 5.0 | 2427 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 3.2 RM map call(s), walk 53 us, worker apply 2164 us per rep
- events naming a host RM call 6.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.1; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 6.1; other events 2.7

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.0 trapped accesses per repetition, window median 4450 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1736 |
| bar2-flush-read | 4.0 | 1925 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.5 RM map call(s), walk 33 us, worker apply 1226 us per rep
- events naming a host RM call 5.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.5
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.5; other events 1.6

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 810 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 21 launch_1000_sync: 1 repetition(s), 216.0 trapped accesses per repetition, window median 225581 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 216.0 | 9055 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x34 | 25.00 |
| PCI config |  | R | 0x60 | 25.00 |
| PCI config |  | R | 0x68 | 25.00 |
| PCI config |  | R | 0x78 | 25.00 |
| PCI config |  | R | 0x6 | 24.00 |
| PCI config |  | R | 0x61 | 24.00 |
| PCI config |  | R | 0x69 | 24.00 |
| PCI config |  | R | 0x79 | 24.00 |
| PCI config |  | R | 0x0 | 1.00 |
| PCI config |  | R | 0x4 | 1.00 |
| PCI config |  | R | 0x8 | 1.00 |
| PCI config |  | R | 0xc | 1.00 |
| (rest) | | | | 16.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1031.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 721 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 676 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.3

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1209 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5384 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.9

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5730 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.9

## op 35 d2d_16M: 10 repetition(s), 4.5 trapped accesses per repetition, window median 3995 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 4.5 | 549 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x6 | 0.50 |
| PCI config |  | R | 0x34 | 0.50 |
| PCI config |  | R | 0x60 | 0.50 |
| PCI config |  | R | 0x61 | 0.50 |
| PCI config |  | R | 0x68 | 0.50 |
| PCI config |  | R | 0x69 | 0.50 |
| PCI config |  | R | 0x78 | 0.50 |
| PCI config |  | R | 0x79 | 0.50 |
| PCI config |  | R | 0x7c | 0.10 |
| PCI config |  | R | 0x80 | 0.10 |
| PCI config |  | R | 0x84 | 0.10 |
| PCI config |  | R | 0x88 | 0.10 |
| (rest) | | | | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.4

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 704 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 710 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 78 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 778 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 872 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.3

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 80 us



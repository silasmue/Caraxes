# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/before-3/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 0.0 | 100262 | 100130.2 |
| 2 | cuInit | 3 | 174.3 | 53432 | 53338.1 |
| 3 | cuCtxCreate | 5 | 1289.8 | 547782 | 547653.6 |
| 4 | cuCtxDestroy | 5 | 725.0 | 214873 | 214798.3 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 1857 | 1533.1 |
| 11 | cuMemFree_1M | 20 | 6.5 | 3600 | 2246.2 |
| 12 | cuMemAlloc_64M | 10 | 7.8 | 17234 | 17061.2 |
| 13 | cuMemFree_64M | 10 | 26.6 | 14132 | 13639.1 |
| 14 | cuMemHostAlloc_1M | 10 | 7.5 | 6428 | 6062.5 |
| 15 | cuMemFreeHost_1M | 10 | 7.0 | 4316 | 4230.4 |
| 16 | cuMemHostRegister_1M | 10 | 15.0 | 3375 | 3320.7 |
| 17 | cuMemHostUnregister_1M | 10 | 14.0 | 11112 | 10806.7 |
| 20 | launch_1_sync | 20 | 0.0 | 708 | 448.3 |
| 21 | launch_1000_sync | 1 | 0.0 | 495419 | 263.4 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 551 | 282.8 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 1175 | 567.8 |
| 32 | d2d_1M | 10 | 0.0 | 1434 | 761.7 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 4668 | 4411.6 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 4127 | 3340.5 |
| 35 | d2d_16M | 10 | 0.0 | 3216 | 2930.3 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 1448 | 621.7 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 1978 | 1101.8 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 80 | 19.0 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 386 | 324.0 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 490 | 433.9 |
| 51 | cuModuleUnload | 10 | 0.9 | 118 | 37.9 |

## op 1 idle_100ms: 10 repetition(s), 0.0 trapped accesses per repetition, window median 100262 us

- device events per rep: other events 16.1

## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 53432 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 4619 |
| mmu-invalidate | 40.0 | 1819 |
| gsp-queue-doorbell | 27.3 | 8755 |
| bar2-flush-read | 24.0 | 11225 |
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

- GSP RPCs 27.3 per rep (106 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 55.3

## op 3 cuCtxCreate: 5 repetition(s), 1289.8 trapped accesses per repetition, window median 547782 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 487.0 | 42198 |
| bar2-flush-read | 272.2 | 73353 |
| gsp-falcon | 255.0 | 14187 |
| pci-config | 102.6 | 2429 |
| gsp-queue-doorbell | 85.0 | 16034 |
| usermode page, other | 60.0 | 4142 |
| usermode-doorbell | 20.0 | 3900 |
| bar0-other | 8.0 | 1577 |

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

- GSP RPCs 85.0 per rep (346 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.0 per rep: 20130 leaves mapped and 252 unmapped in 110.8 RM map call(s), walk 3477 us, worker apply 105562 us per rep
- events naming a host RM call 276.8 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x112.6; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x56.0; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X (X/X) [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 92.6; guest MMU invalidates 97.4; host doorbells written (forwarded) 112.6; host notifier events (no MSI) 14.2; host notifier events -> MSI 56.0; other events 515.8

## op 4 cuCtxDestroy: 5 repetition(s), 725.0 trapped accesses per repetition, window median 214873 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 30010 |
| gsp-falcon | 207.0 | 17535 |
| bar2-flush-read | 169.0 | 51371 |
| gsp-queue-doorbell | 69.0 | 36894 |

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

- GSP RPCs 69.0 per rep (267 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0; GSP_RM_CONTROL cmd 0x802009 x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 372 us, worker apply 16184 us per rep
- events naming a host RM call 66.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x37.8; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x1.4; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 1.4; host notifier events -> MSI 37.8; other events 188.8

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 1857 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 834 |
| bar2-flush-read | 1.0 | 369 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 15 us, worker apply 570 us per rep
- events naming a host RM call 15.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x15.9; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 0.5; host notifier events (no MSI) 0.1; host notifier events -> MSI 15.9; other events 1.1

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 3600 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 1487 |
| bar2-flush-read | 1.5 | 1027 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 13 us, worker apply 1314 us per rep
- events naming a host RM call 5.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.7
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.7; other events 1.8

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 17234 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 784 |
| bar2-flush-read | 2.3 | 670 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 82 us, worker apply 10156 us per rep
- events naming a host RM call 7.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x7.7; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.2
- device events per rep: guest MMU invalidates 1.1; host notifier events (no MSI) 0.2; host notifier events -> MSI 7.7; other events 3.0

## op 13 cuMemFree_64M: 10 repetition(s), 26.6 trapped accesses per repetition, window median 14132 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 13.5 | 878 |
| mmu-invalidate | 10.0 | 1292 |
| bar2-flush-read | 3.1 | 1426 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| PCI config |  | R | 0x78 | 1.60 |
| PCI config |  | R | 0x6 | 1.50 |
| PCI config |  | R | 0x34 | 1.50 |
| PCI config |  | R | 0x60 | 1.50 |
| PCI config |  | R | 0x61 | 1.50 |
| PCI config |  | R | 0x68 | 1.50 |
| PCI config |  | R | 0x69 | 1.50 |
| (rest) | | | | 2.90 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 3 leaves mapped and 32 unmapped in 3.2 RM map call(s), walk 50 us, worker apply 8880 us per rep
- events naming a host RM call 7.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.7; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.3
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.3; host notifier events -> MSI 6.7; other events 3.1

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 6428 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 1289 |
| bar2-flush-read | 2.5 | 1189 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.4 per rep: 205 leaves mapped and 0 unmapped in 0.6 RM map call(s), walk 22 us, worker apply 806 us per rep
- events naming a host RM call 5.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.6
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.6; other events 1.7

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.0 trapped accesses per repetition, window median 4316 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 1247 |
| bar2-flush-read | 2.0 | 1052 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.2 RM map call(s), walk 16 us, worker apply 678 us per rep
- events naming a host RM call 5.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.8
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.8; other events 1.1

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.0 trapped accesses per repetition, window median 3375 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1670 |
| bar2-flush-read | 5.0 | 1377 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 2.8 RM map call(s), walk 63 us, worker apply 1569 us per rep
- events naming a host RM call 4.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.8
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.8; other events 1.6

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.0 trapped accesses per repetition, window median 11112 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 2042 |
| bar2-flush-read | 4.0 | 2023 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.5 RM map call(s), walk 35 us, worker apply 680 us per rep
- events naming a host RM call 6.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.5; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 6.5; other events 3.2

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 708 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 495419 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1076.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 551 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1175 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.5

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1434 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.4

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 4668 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.6

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 4127 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.5

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 3216 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.5

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1448 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1978 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.3

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 80 us

- device events per rep: other events 0.1

## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 386 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 490 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 51 cuModuleUnload: 10 repetition(s), 0.9 trapped accesses per repetition, window median 118 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 0.9 | 23 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x6 | 0.10 |
| PCI config |  | R | 0x34 | 0.10 |
| PCI config |  | R | 0x60 | 0.10 |
| PCI config |  | R | 0x61 | 0.10 |
| PCI config |  | R | 0x68 | 0.10 |
| PCI config |  | R | 0x69 | 0.10 |
| PCI config |  | R | 0x78 | 0.10 |
| PCI config |  | R | 0x79 | 0.10 |
| PCI config |  | R | 0xc4 | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- device events per rep: other events 0.1


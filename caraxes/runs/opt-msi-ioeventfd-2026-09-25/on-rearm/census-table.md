# census-table: caraxes/runs/opt-msi-ioeventfd-2026-09-25/on-rearm/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 20 | 25.6 | 100196 | 100092.3 |
| 2 | cuInit | 3 | 174.3 | 18851 | 18786.6 |
| 3 | cuCtxCreate | 3 | 1189.7 | 541621 | 541559.7 |
| 4 | cuCtxDestroy | 3 | 1067.0 | 207605 | 207533.9 |
| 10 | cuMemAlloc_1M | 40 | 3.5 | 402 | 340.4 |
| 11 | cuMemFree_1M | 40 | 6.5 | 521 | 469.0 |
| 12 | cuMemAlloc_64M | 20 | 7.8 | 1212 | 1128.0 |
| 13 | cuMemFree_64M | 20 | 13.1 | 1476 | 1423.9 |
| 14 | cuMemHostAlloc_1M | 20 | 7.5 | 1573 | 1844.7 |
| 15 | cuMemFreeHost_1M | 20 | 7.0 | 1416 | 1171.2 |
| 16 | cuMemHostRegister_1M | 20 | 15.0 | 1908 | 2026.9 |
| 17 | cuMemHostUnregister_1M | 20 | 14.0 | 1837 | 2576.9 |
| 20 | launch_1_sync | 40 | 0.0 | 708 | 656.8 |
| 21 | launch_1000_sync | 2 | 0.0 | 31486 | 20.0 |
| 30 | h2d_pinned_1M | 20 | 0.0 | 456 | 396.4 |
| 31 | d2h_pinned_1M | 20 | 0.9 | 471 | 400.2 |
| 32 | d2d_1M | 20 | 0.0 | 974 | 913.5 |
| 33 | h2d_pinned_16M | 20 | 0.0 | 5268 | 5170.2 |
| 34 | d2h_pinned_16M | 20 | 0.0 | 5274 | 5212.7 |
| 35 | d2d_16M | 20 | 10.3 | 4020 | 3909.6 |
| 36 | h2d_pageable_1M | 20 | 0.0 | 566 | 479.1 |
| 37 | d2h_pageable_1M | 20 | 0.0 | 612 | 540.3 |
| 40 | cuStreamSynchronize_idle | 40 | 0.0 | 72 | 9.6 |
| 41 | cuEventRecord_Synchronize | 40 | 0.0 | 706 | 657.8 |
| 50 | cuModuleLoad_cubin | 20 | 0.0 | 894 | 797.0 |
| 51 | cuModuleUnload | 20 | 0.0 | 84 | 23.4 |

## op 1 idle_100ms: 20 repetition(s), 25.6 trapped accesses per repetition, window median 100196 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 25.6 | 369 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
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
| PCI config |  | R | 0xc | 0.05 |
| (rest) | | | | 2.45 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- device events per rep: other events 13.0

## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 18851 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 2948 |
| mmu-invalidate | 40.0 | 1370 |
| gsp-queue-doorbell | 27.3 | 10235 |
| bar2-flush-read | 24.0 | 1416 |
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

- GSP RPCs 27.3 per rep (87 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 54.7

## op 3 cuCtxCreate: 3 repetition(s), 1189.7 trapped accesses per repetition, window median 541621 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 488.3 | 27333 |
| bar2-flush-read | 273.0 | 66941 |
| gsp-falcon | 255.0 | 10825 |
| gsp-queue-doorbell | 85.0 | 13515 |
| usermode page, other | 60.0 | 3247 |
| usermode-doorbell | 20.0 | 0 |
| bar0-other | 8.3 | 1558 |

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
| BAR0 | 0xbb0000 | R | 0xbb0080 NV_VIRTUAL_FUNCTION_TIME_0/1 | 20.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 20.00 |
| (rest) | | | | 8.33 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): tokens 20.0

- GSP RPCs 85.0 per rep (334 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.0 per rep: 20130 leaves mapped and 252 unmapped in 6259.3 RM map call(s), walk 3246 us, worker apply 248222 us per rep
- events naming a host RM call 358.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x169.7; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x48.7; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x45.3; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0
- device events per rep: doorbell eventfd rings (token pool) 149.7; guest MMU invalidates 97.7; host doorbells written (forwarded) 169.7; host notifier events (no MSI) 45.3; host notifier events -> MSI 48.7; other events 559.7

## op 4 cuCtxDestroy: 3 repetition(s), 1067.0 trapped accesses per repetition, window median 207605 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 342.0 | 7712 |
| mmu-invalidate | 280.0 | 16998 |
| gsp-falcon | 207.0 | 12056 |
| bar2-flush-read | 169.0 | 49967 |
| gsp-queue-doorbell | 69.0 | 25805 |

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
| PCI config |  | R | 0x34 | 38.67 |
| PCI config |  | R | 0x60 | 38.67 |
| PCI config |  | R | 0x68 | 38.67 |
| (rest) | | | | 226.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (263 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 16.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 262 us, worker apply 494668 us per rep
- events naming a host RM call 127.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x67.3; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x33.3; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 67.3; host notifier events -> MSI 33.3; other events 185.7

## op 10 cuMemAlloc_1M: 40 repetition(s), 3.5 trapped accesses per repetition, window median 402 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 217 |
| bar2-flush-read | 1.0 | 260 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 9 us, worker apply 412 us per rep
- events naming a host RM call 13.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x13.3; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.0
- device events per rep: guest MMU invalidates 0.5; host notifier events (no MSI) 0.0; host notifier events -> MSI 13.3; other events 0.7

## op 11 cuMemFree_1M: 40 repetition(s), 6.5 trapped accesses per repetition, window median 521 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 427 |
| bar2-flush-read | 1.5 | 285 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 8 us, worker apply 167 us per rep
- events naming a host RM call 4.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.4; shadow: VAS of guest root X freed: N syncs (N skipped clean, N full walks, N table reads,  x0.0
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.4; other events 0.7

## op 12 cuMemAlloc_64M: 20 repetition(s), 7.8 trapped accesses per repetition, window median 1212 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 518 |
| bar2-flush-read | 2.3 | 737 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 39 us, worker apply 5215 us per rep
- events naming a host RM call 5.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.2
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 5.2; other events 1.1

## op 13 cuMemFree_64M: 20 repetition(s), 13.1 trapped accesses per repetition, window median 1476 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 578 |
| bar2-flush-read | 3.1 | 754 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 41 us, worker apply 4942 us per rep
- events naming a host RM call 4.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.7
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.7; other events 1.4

## op 14 cuMemHostAlloc_1M: 20 repetition(s), 7.5 trapped accesses per repetition, window median 1573 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 351 |
| bar2-flush-read | 2.5 | 1019 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 34.6 RM map call(s), walk 32 us, worker apply 3446 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.5; other events 0.9

## op 15 cuMemFreeHost_1M: 20 repetition(s), 7.0 trapped accesses per repetition, window median 1416 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 452 |
| bar2-flush-read | 2.0 | 787 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 51 leaves mapped and 205 unmapped in 3.7 RM map call(s), walk 15 us, worker apply 613 us per rep
- events naming a host RM call 5.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.3
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.3; other events 0.8

## op 16 cuMemHostRegister_1M: 20 repetition(s), 15.0 trapped accesses per repetition, window median 1908 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 614 |
| bar2-flush-read | 5.0 | 1475 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 230 leaves mapped and 51 unmapped in 75.5 RM map call(s), walk 31 us, worker apply 5807 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.0; other events 1.3

## op 17 cuMemHostUnregister_1M: 20 repetition(s), 14.0 trapped accesses per repetition, window median 1837 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 623 |
| bar2-flush-read | 4.0 | 1229 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 26 leaves mapped and 230 unmapped in 0.2 RM map call(s), walk 120 us, worker apply 3463 us per rep
- events naming a host RM call 4.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.9
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.9; other events 1.3

## op 20 launch_1_sync: 40 repetition(s), 0.0 trapped accesses per repetition, window median 708 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 21 launch_1000_sync: 2 repetition(s), 0.0 trapped accesses per repetition, window median 31486 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1004.5

## op 30 h2d_pinned_1M: 20 repetition(s), 0.0 trapped accesses per repetition, window median 456 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 31 d2h_pinned_1M: 20 repetition(s), 0.9 trapped accesses per repetition, window median 471 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 0.9 | 22 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x78 | 0.15 |
| PCI config |  | R | 0x6 | 0.10 |
| PCI config |  | R | 0x34 | 0.10 |
| PCI config |  | R | 0x60 | 0.10 |
| PCI config |  | R | 0x61 | 0.10 |
| PCI config |  | R | 0x68 | 0.10 |
| PCI config |  | R | 0x69 | 0.10 |
| PCI config |  | R | 0x79 | 0.10 |
| PCI config |  | R | 0x74 | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 32 d2d_1M: 20 repetition(s), 0.0 trapped accesses per repetition, window median 974 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 33 h2d_pinned_16M: 20 repetition(s), 0.0 trapped accesses per repetition, window median 5268 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.7

## op 34 d2h_pinned_16M: 20 repetition(s), 0.0 trapped accesses per repetition, window median 5274 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.8

## op 35 d2d_16M: 20 repetition(s), 10.3 trapped accesses per repetition, window median 4020 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 10.3 | 195 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x60 | 1.20 |
| PCI config |  | R | 0x68 | 1.20 |
| PCI config |  | R | 0x78 | 1.20 |
| PCI config |  | R | 0x6 | 1.15 |
| PCI config |  | R | 0x34 | 1.15 |
| PCI config |  | R | 0x61 | 1.15 |
| PCI config |  | R | 0x69 | 1.15 |
| PCI config |  | R | 0x79 | 1.15 |
| PCI config |  | R | 0x50 | 0.05 |
| PCI config |  | R | 0x54 | 0.05 |
| PCI config |  | R | 0x58 | 0.05 |
| PCI config |  | R | 0x5c | 0.05 |
| (rest) | | | | 0.80 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 36 h2d_pageable_1M: 20 repetition(s), 0.0 trapped accesses per repetition, window median 566 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 20 repetition(s), 0.0 trapped accesses per repetition, window median 612 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 40 cuStreamSynchronize_idle: 40 repetition(s), 0.0 trapped accesses per repetition, window median 72 us

- device events per rep: other events 0.0

## op 41 cuEventRecord_Synchronize: 40 repetition(s), 0.0 trapped accesses per repetition, window median 706 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 50 cuModuleLoad_cubin: 20 repetition(s), 0.0 trapped accesses per repetition, window median 894 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 51 cuModuleUnload: 20 repetition(s), 0.0 trapped accesses per repetition, window median 84 us



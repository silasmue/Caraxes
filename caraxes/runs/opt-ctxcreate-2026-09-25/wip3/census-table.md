# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/wip3/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 51.3 | 100174 | 100081.4 |
| 2 | cuInit | 3 | 174.3 | 33603 | 33504.3 |
| 3 | cuCtxCreate | 5 | 1392.4 | 569896 | 569809.3 |
| 4 | cuCtxDestroy | 5 | 725.0 | 243237 | 243191.2 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 548 | 470.4 |
| 11 | cuMemFree_1M | 20 | 6.5 | 1208 | 1111.4 |
| 12 | cuMemAlloc_64M | 10 | 7.8 | 3738 | 3694.2 |
| 13 | cuMemFree_64M | 10 | 13.1 | 1664 | 1613.4 |
| 14 | cuMemHostAlloc_1M | 10 | 7.5 | 2824 | 2775.6 |
| 15 | cuMemFreeHost_1M | 10 | 7.0 | 2377 | 1464.9 |
| 16 | cuMemHostRegister_1M | 10 | 15.0 | 3585 | 3541.7 |
| 17 | cuMemHostUnregister_1M | 10 | 14.0 | 2206 | 2072.9 |
| 20 | launch_1_sync | 20 | 0.0 | 783 | 727.0 |
| 21 | launch_1000_sync | 1 | 0.0 | 71684 | 25.7 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 472 | 415.5 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 482 | 432.0 |
| 32 | d2d_1M | 10 | 0.0 | 944 | 886.8 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 5468 | 5359.9 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 5412 | 5327.6 |
| 35 | d2d_16M | 10 | 0.0 | 3867 | 3783.1 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 691 | 619.0 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 630 | 551.2 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 63 | 14.7 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 517 | 457.5 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 622 | 575.6 |
| 51 | cuModuleUnload | 10 | 0.0 | 82 | 33.9 |

## op 1 idle_100ms: 10 repetition(s), 51.3 trapped accesses per repetition, window median 100174 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 51.3 | 1210 |

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

- device events per rep: other events 14.7

## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 33603 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 4028 |
| mmu-invalidate | 40.0 | 1879 |
| gsp-queue-doorbell | 27.3 | 9775 |
| bar2-flush-read | 24.0 | 1962 |
| pcfg-mirror (intr re-arm) | 1.0 | 42 |

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

- GSP RPCs 27.3 per rep (109 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 55.7

## op 3 cuCtxCreate: 5 repetition(s), 1392.4 trapped accesses per repetition, window median 569896 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 487.0 | 43438 |
| bar2-flush-read | 272.2 | 83194 |
| gsp-falcon | 255.0 | 15178 |
| pci-config | 205.2 | 8389 |
| gsp-queue-doorbell | 85.0 | 17527 |
| usermode page, other | 60.0 | 4594 |
| usermode-doorbell | 20.0 | 1481 |
| bar0-other | 8.0 | 1976 |

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
| PCI config |  | R | 0x34 | 23.20 |
| PCI config |  | R | 0x60 | 23.20 |
| (rest) | | | | 206.80 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): tokens 20.0

- GSP RPCs 85.0 per rep (682 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 27.0 per rep: 20130 leaves mapped and 252 unmapped in 105.6 RM map call(s), walk 3162 us, worker apply 106062 us per rep
- events naming a host RM call 309.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x120.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x62.8; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N built in N us: plan and apply N us, TSG N us, channel N us, objects N us, s x20.0
- device events per rep: doorbell eventfd rings (token pool) 100.0; guest MMU invalidates 97.4; host doorbells written (forwarded) 120.0; host notifier events (no MSI) 12.2; host notifier events -> MSI 62.8; other events 558.6

## op 4 cuCtxDestroy: 5 repetition(s), 725.0 trapped accesses per repetition, window median 243237 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 20669 |
| gsp-falcon | 207.0 | 15744 |
| bar2-flush-read | 169.0 | 57951 |
| gsp-queue-doorbell | 69.0 | 28133 |

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

- GSP RPCs 69.0 per rep (1988 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 382 us, worker apply 14060 us per rep
- events naming a host RM call 65.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x36.2; free X class Some(N); mirror: chid N: host channel X freed in N us after N doorbell(s), N  x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x2.2; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 2.2; host notifier events -> MSI 36.2; other events 187.8

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 548 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 355 |
| bar2-flush-read | 1.0 | 222 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 9 us, worker apply 364 us per rep
- events naming a host RM call 14.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.4
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 14.4; other events 0.6

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 1208 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 477 |
| bar2-flush-read | 1.5 | 409 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 9 us, worker apply 98 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.5; other events 0.8

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 3738 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 715 |
| bar2-flush-read | 2.3 | 821 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 39 us, worker apply 6290 us per rep
- events naming a host RM call 6.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.1
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 6.1; other events 1.8

## op 13 cuMemFree_64M: 10 repetition(s), 13.1 trapped accesses per repetition, window median 1664 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 674 |
| bar2-flush-read | 3.1 | 960 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 45 us, worker apply 4964 us per rep
- events naming a host RM call 4.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.9
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.9; other events 1.4

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 2824 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 756 |
| bar2-flush-read | 2.5 | 1060 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 1.3 RM map call(s), walk 29 us, worker apply 787 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.5; other events 1.5

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.0 trapped accesses per repetition, window median 2377 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 452 |
| bar2-flush-read | 2.0 | 801 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 17 us, worker apply 486 us per rep
- events naming a host RM call 5.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.3
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.3; other events 0.7

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.0 trapped accesses per repetition, window median 3585 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1429 |
| bar2-flush-read | 5.0 | 1707 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 2.2 RM map call(s), walk 29 us, worker apply 1991 us per rep
- events naming a host RM call 5.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.1
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.1; other events 2.1

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.0 trapped accesses per repetition, window median 2206 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 791 |
| bar2-flush-read | 4.0 | 1392 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 26 leaves mapped and 230 unmapped in 0.3 RM map call(s), walk 24 us, worker apply 469 us per rep
- events naming a host RM call 5.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.1
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.1; other events 1.4

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 783 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.3

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 71684 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1011.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 472 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 482 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 944 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5468 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.6

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5412 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.6

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 3867 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 691 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 630 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 63 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 517 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 622 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 82 us



# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/after/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 16.2 | 100216 | 100095.7 |
| 2 | cuInit | 3 | 174.3 | 32108 | 32040.5 |
| 3 | cuCtxCreate | 5 | 1187.2 | 332453 | 332386.5 |
| 4 | cuCtxDestroy | 5 | 725.0 | 180686 | 180619.0 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 698 | 629.7 |
| 11 | cuMemFree_1M | 20 | 6.5 | 611 | 540.9 |
| 12 | cuMemAlloc_64M | 10 | 38.4 | 3578 | 3498.5 |
| 13 | cuMemFree_64M | 10 | 13.1 | 1619 | 1549.3 |
| 14 | cuMemHostAlloc_1M | 10 | 7.5 | 2850 | 2788.5 |
| 15 | cuMemFreeHost_1M | 10 | 7.0 | 1302 | 1247.1 |
| 16 | cuMemHostRegister_1M | 10 | 15.0 | 3934 | 3876.8 |
| 17 | cuMemHostUnregister_1M | 10 | 14.0 | 2553 | 2134.6 |
| 20 | launch_1_sync | 20 | 0.0 | 769 | 705.9 |
| 21 | launch_1000_sync | 1 | 0.0 | 89887 | 26.9 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 502 | 422.3 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 432 | 375.8 |
| 32 | d2d_1M | 10 | 0.0 | 884 | 819.4 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 5277 | 5184.0 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 5178 | 5109.0 |
| 35 | d2d_16M | 10 | 0.0 | 4274 | 4196.9 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 574 | 509.9 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 638 | 557.3 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 94 | 16.7 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 781 | 705.2 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 860 | 797.3 |
| 51 | cuModuleUnload | 10 | 0.0 | 99 | 35.5 |

## op 1 idle_100ms: 10 repetition(s), 16.2 trapped accesses per repetition, window median 100216 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 16.2 | 313 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x34 | 1.90 |
| PCI config |  | R | 0x60 | 1.90 |
| PCI config |  | R | 0x6 | 1.80 |
| PCI config |  | R | 0x61 | 1.80 |
| PCI config |  | R | 0x68 | 1.80 |
| PCI config |  | R | 0x69 | 1.80 |
| PCI config |  | R | 0x78 | 1.80 |
| PCI config |  | R | 0x79 | 1.80 |
| PCI config |  | R | 0x0 | 0.10 |
| PCI config |  | R | 0x4 | 0.10 |
| PCI config |  | R | 0x8 | 0.10 |
| PCI config |  | R | 0xc | 0.10 |
| (rest) | | | | 1.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- device events per rep: other events 15.6

## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 32108 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 4510 |
| mmu-invalidate | 40.0 | 1407 |
| gsp-queue-doorbell | 27.3 | 10428 |
| bar2-flush-read | 24.0 | 1874 |
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

- GSP RPCs 27.3 per rep (102 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 55.7

## op 3 cuCtxCreate: 5 repetition(s), 1187.2 trapped accesses per repetition, window median 332453 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 487.0 | 44207 |
| bar2-flush-read | 272.2 | 73773 |
| gsp-falcon | 255.0 | 12700 |
| gsp-queue-doorbell | 85.0 | 15399 |
| usermode page, other | 60.0 | 3066 |
| usermode-doorbell | 20.0 | 6837 |
| bar0-other | 8.0 | 1436 |

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

- GSP RPCs 85.0 per rep (609 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 27.0 per rep: 20130 leaves mapped and 252 unmapped in 106.6 RM map call(s), walk 3165 us, worker apply 73377 us per rep
- events naming a host RM call 302.4 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x137.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x46.8; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N built in N us: plan and apply N us, TSG N us, channel N us, objects N us, s x20.0
- device events per rep: doorbell eventfd rings (token pool) 117.0; guest MMU invalidates 97.4; host doorbells written (forwarded) 137.0; host notifier events (no MSI) 4.6; host notifier events -> MSI 46.8; other events 556.4

## op 4 cuCtxDestroy: 5 repetition(s), 725.0 trapped accesses per repetition, window median 180686 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 17240 |
| gsp-falcon | 207.0 | 13028 |
| bar2-flush-read | 169.0 | 50837 |
| gsp-queue-doorbell | 69.0 | 33237 |

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

- GSP RPCs 69.0 per rep (305 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 367 us, worker apply 11738 us per rep
- events naming a host RM call 57.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x29.6; free X class Some(N); mirror: chid N: host channel X freed in N us after N doorbell(s), N  x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x1.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 1.0; host notifier events -> MSI 29.6; other events 181.4

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 698 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 379 |
| bar2-flush-read | 1.0 | 256 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 8 us, worker apply 236 us per rep
- events naming a host RM call 14.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.0
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 14.0; other events 0.8

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 611 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 455 |
| bar2-flush-read | 1.5 | 379 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 9 us, worker apply 149 us per rep
- events naming a host RM call 2.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x2.7
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 2.7; other events 0.7

## op 12 cuMemAlloc_64M: 10 repetition(s), 38.4 trapped accesses per repetition, window median 3578 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 30.6 | 650 |
| mmu-invalidate | 5.5 | 1064 |
| bar2-flush-read | 2.3 | 750 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x34 | 3.50 |
| PCI config |  | R | 0x60 | 3.50 |
| PCI config |  | R | 0x68 | 3.50 |
| PCI config |  | R | 0x78 | 3.50 |
| PCI config |  | R | 0x6 | 3.40 |
| PCI config |  | R | 0x61 | 3.40 |
| PCI config |  | R | 0x69 | 3.40 |
| PCI config |  | R | 0x79 | 3.40 |
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| (rest) | | | | 4.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 41 us, worker apply 1904 us per rep
- events naming a host RM call 3.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x3.9
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 3.9; other events 1.5

## op 13 cuMemFree_64M: 10 repetition(s), 13.1 trapped accesses per repetition, window median 1619 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 624 |
| bar2-flush-read | 3.1 | 698 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 48 us, worker apply 1976 us per rep
- events naming a host RM call 4.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.4
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.4; other events 1.5

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 2850 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 762 |
| bar2-flush-read | 2.5 | 670 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 0.7 RM map call(s), walk 24 us, worker apply 2300 us per rep
- events naming a host RM call 3.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x3.7
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 3.7; other events 1.2

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.0 trapped accesses per repetition, window median 1302 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 307 |
| bar2-flush-read | 2.0 | 790 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 51 leaves mapped and 205 unmapped in 0.2 RM map call(s), walk 12 us, worker apply 210 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.0; other events 0.6

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.0 trapped accesses per repetition, window median 3934 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 968 |
| bar2-flush-read | 5.0 | 1583 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 230 leaves mapped and 51 unmapped in 1.9 RM map call(s), walk 36 us, worker apply 10308 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.1; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.9
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.9; host notifier events -> MSI 4.1; other events 2.0

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.0 trapped accesses per repetition, window median 2553 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 810 |
| bar2-flush-read | 4.0 | 1279 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 24 us, worker apply 336 us per rep
- events naming a host RM call 2.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x2.2
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 2.2; other events 1.4

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 769 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 89887 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1011.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 502 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 432 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 884 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5277 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.4

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5178 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.3

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 4274 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 574 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 638 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 94 us

- device events per rep: other events 0.1

## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 781 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 860 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 99 us



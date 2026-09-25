# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/wip4/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 0.0 | 100157 | 100077.3 |
| 2 | cuInit | 3 | 174.3 | 21831 | 21753.6 |
| 3 | cuCtxCreate | 5 | 1289.8 | 413997 | 413947.7 |
| 4 | cuCtxDestroy | 5 | 725.0 | 176348 | 176295.5 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 410 | 371.0 |
| 11 | cuMemFree_1M | 20 | 12.3 | 486 | 432.3 |
| 12 | cuMemAlloc_64M | 10 | 7.8 | 2926 | 2882.9 |
| 13 | cuMemFree_64M | 10 | 13.1 | 1456 | 1392.7 |
| 14 | cuMemHostAlloc_1M | 10 | 7.5 | 2399 | 2345.8 |
| 15 | cuMemFreeHost_1M | 10 | 7.0 | 1268 | 1215.4 |
| 16 | cuMemHostRegister_1M | 10 | 15.0 | 3090 | 3024.9 |
| 17 | cuMemHostUnregister_1M | 10 | 14.0 | 1544 | 1506.8 |
| 20 | launch_1_sync | 20 | 0.0 | 781 | 700.2 |
| 21 | launch_1000_sync | 1 | 0.0 | 37474 | 25.8 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 452 | 389.1 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 424 | 379.4 |
| 32 | d2d_1M | 10 | 0.0 | 942 | 891.9 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 5264 | 5178.5 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 5288 | 5242.6 |
| 35 | d2d_16M | 10 | 0.0 | 4424 | 4340.3 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 540 | 496.1 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 515 | 471.5 |
| 40 | cuStreamSynchronize_idle | 20 | 0.5 | 60 | 11.2 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 780 | 713.8 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 777 | 729.6 |
| 51 | cuModuleUnload | 10 | 0.0 | 64 | 20.7 |

## op 1 idle_100ms: 10 repetition(s), 0.0 trapped accesses per repetition, window median 100157 us

- device events per rep: other events 15.0

## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 21831 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 2974 |
| mmu-invalidate | 40.0 | 1369 |
| gsp-queue-doorbell | 27.3 | 9714 |
| bar2-flush-read | 24.0 | 2505 |
| pcfg-mirror (intr re-arm) | 1.0 | 29 |

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

- GSP RPCs 27.3 per rep (94 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 55.7

## op 3 cuCtxCreate: 5 repetition(s), 1289.8 trapped accesses per repetition, window median 413997 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 487.0 | 32818 |
| bar2-flush-read | 272.2 | 69642 |
| gsp-falcon | 255.0 | 9487 |
| pci-config | 102.6 | 1692 |
| gsp-queue-doorbell | 85.0 | 13742 |
| usermode page, other | 60.0 | 2329 |
| usermode-doorbell | 20.0 | 4883 |
| bar0-other | 8.0 | 1357 |

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

- GSP RPCs 85.0 per rep (333 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 27.0 per rep: 20130 leaves mapped and 252 unmapped in 117.8 RM map call(s), walk 2860 us, worker apply 68649 us per rep
- events naming a host RM call 306.8 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x133.2; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x47.2; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N built in N us: plan and apply N us, TSG N us, channel N us, objects N us, s x20.0
- device events per rep: doorbell eventfd rings (token pool) 113.2; guest MMU invalidates 97.4; host doorbells written (forwarded) 133.2; host notifier events (no MSI) 12.4; host notifier events -> MSI 47.2; other events 553.2

## op 4 cuCtxDestroy: 5 repetition(s), 725.0 trapped accesses per repetition, window median 176348 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 14315 |
| gsp-falcon | 207.0 | 9485 |
| bar2-flush-read | 169.0 | 50564 |
| gsp-queue-doorbell | 69.0 | 18569 |

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

- GSP RPCs 69.0 per rep (213 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0; GSP_RM_CONTROL cmd 0x802009 x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 308 us, worker apply 9287 us per rep
- events naming a host RM call 59.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x29.4; free X class Some(N); mirror: chid N: host channel X retired in N us (TSG X preempted; fre x20.0; free X class Some(N); mirror: tsg X/X: N retired host channel(s) freed with host TSG X [ho x4.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x2.8; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; shadow: VAS of guest root X freed: N syncs (N skipped clean, N full walks, N table reads,  x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 2.8; host notifier events -> MSI 29.4; other events 180.2

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 410 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 293 |
| bar2-flush-read | 1.0 | 186 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 7 us, worker apply 100 us per rep
- events naming a host RM call 14.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.4
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 14.4; other events 0.5

## op 11 cuMemFree_1M: 20 repetition(s), 12.3 trapped accesses per repetition, window median 486 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 5.8 | 105 |
| mmu-invalidate | 5.0 | 316 |
| bar2-flush-read | 1.5 | 362 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| PCI config |  | R | 0x78 | 0.70 |
| PCI config |  | R | 0x6 | 0.65 |
| PCI config |  | R | 0x34 | 0.65 |
| PCI config |  | R | 0x60 | 0.65 |
| PCI config |  | R | 0x61 | 0.65 |
| PCI config |  | R | 0x68 | 0.65 |
| PCI config |  | R | 0x69 | 0.65 |
| (rest) | | | | 1.25 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 8 us, worker apply 58 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 1.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 4.5; other events 0.8

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 2926 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 1384 |
| bar2-flush-read | 2.3 | 570 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 30 us, worker apply 4321 us per rep
- events naming a host RM call 5.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.8
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 5.8; other events 1.6

## op 13 cuMemFree_64M: 10 repetition(s), 13.1 trapped accesses per repetition, window median 1456 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1036 |
| bar2-flush-read | 3.1 | 765 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 36 us, worker apply 2535 us per rep
- events naming a host RM call 4.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.8
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.8; other events 1.2

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 2399 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 637 |
| bar2-flush-read | 2.5 | 984 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 0.5 RM map call(s), walk 20 us, worker apply 3860 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.5; other events 1.2

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.0 trapped accesses per repetition, window median 1268 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 377 |
| bar2-flush-read | 2.0 | 792 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.2 RM map call(s), walk 13 us, worker apply 507 us per rep
- events naming a host RM call 5.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.4
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.4; other events 0.9

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.0 trapped accesses per repetition, window median 3090 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 866 |
| bar2-flush-read | 5.0 | 1199 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 1.7 RM map call(s), walk 259 us, worker apply 582 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.0; other events 2.0

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.0 trapped accesses per repetition, window median 1544 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 535 |
| bar2-flush-read | 4.0 | 1044 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.5 RM map call(s), walk 16 us, worker apply 924 us per rep
- events naming a host RM call 4.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.8
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.8; other events 1.4

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 781 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 37474 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1006.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 452 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 424 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 942 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5264 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.6

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5288 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.8

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 4424 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 540 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 515 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.5 trapped accesses per repetition, window median 60 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 0.5 | 8 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x6 | 0.05 |
| PCI config |  | R | 0x34 | 0.05 |
| PCI config |  | R | 0x60 | 0.05 |
| PCI config |  | R | 0x61 | 0.05 |
| PCI config |  | R | 0x68 | 0.05 |
| PCI config |  | R | 0x69 | 0.05 |
| PCI config |  | R | 0x78 | 0.05 |
| PCI config |  | R | 0x79 | 0.05 |
| PCI config |  | R | 0x48 | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 780 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 777 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 64 us



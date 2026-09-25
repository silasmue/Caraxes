# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/part2/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 36.0 | 100210 | 100096.3 |
| 2 | cuInit | 3 | 174.3 | 41200 | 41130.2 |
| 3 | cuCtxCreate | 5 | 1289.8 | 336683 | 336600.2 |
| 4 | cuCtxDestroy | 5 | 725.0 | 165476 | 165419.6 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 588 | 496.5 |
| 11 | cuMemFree_1M | 20 | 6.5 | 606 | 543.7 |
| 12 | cuMemAlloc_64M | 10 | 7.8 | 3707 | 3654.9 |
| 13 | cuMemFree_64M | 10 | 13.1 | 1946 | 1883.6 |
| 14 | cuMemHostAlloc_1M | 10 | 7.5 | 2878 | 2827.9 |
| 15 | cuMemFreeHost_1M | 10 | 7.0 | 1438 | 1368.2 |
| 16 | cuMemHostRegister_1M | 10 | 15.0 | 4332 | 4162.1 |
| 17 | cuMemHostUnregister_1M | 10 | 14.0 | 2186 | 2091.9 |
| 20 | launch_1_sync | 20 | 0.0 | 627 | 557.0 |
| 21 | launch_1000_sync | 1 | 0.0 | 40207 | 25.2 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 584 | 523.2 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 476 | 412.7 |
| 32 | d2d_1M | 10 | 0.0 | 898 | 823.3 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 5329 | 5239.5 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 5250 | 5197.8 |
| 35 | d2d_16M | 10 | 0.0 | 4088 | 4013.5 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 594 | 543.2 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 1726 | 1131.6 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 53 | 12.1 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 816 | 743.8 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 886 | 837.9 |
| 51 | cuModuleUnload | 10 | 0.0 | 70 | 29.5 |

## op 1 idle_100ms: 10 repetition(s), 36.0 trapped accesses per repetition, window median 100210 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 36.0 | 658 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x34 | 4.10 |
| PCI config |  | R | 0x60 | 4.10 |
| PCI config |  | R | 0x68 | 4.10 |
| PCI config |  | R | 0x78 | 4.10 |
| PCI config |  | R | 0x6 | 4.00 |
| PCI config |  | R | 0x61 | 4.00 |
| PCI config |  | R | 0x69 | 4.00 |
| PCI config |  | R | 0x79 | 4.00 |
| PCI config |  | R | 0x0 | 0.10 |
| PCI config |  | R | 0x4 | 0.10 |
| PCI config |  | R | 0x8 | 0.10 |
| PCI config |  | R | 0xc | 0.10 |
| (rest) | | | | 3.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- device events per rep: other events 24.3

## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 41200 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 6867 |
| mmu-invalidate | 40.0 | 1671 |
| gsp-queue-doorbell | 27.3 | 9772 |
| bar2-flush-read | 24.0 | 3129 |
| pcfg-mirror (intr re-arm) | 1.0 | 51 |

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

- GSP RPCs 27.3 per rep (127 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 55.7

## op 3 cuCtxCreate: 5 repetition(s), 1289.8 trapped accesses per repetition, window median 336683 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 487.0 | 36557 |
| bar2-flush-read | 272.2 | 70859 |
| gsp-falcon | 255.0 | 12060 |
| pci-config | 102.6 | 2296 |
| gsp-queue-doorbell | 85.0 | 13870 |
| usermode page, other | 60.0 | 3364 |
| usermode-doorbell | 20.0 | 3772 |
| bar0-other | 8.0 | 1778 |

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

- GSP RPCs 85.0 per rep (400 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 27.0 per rep: 20130 leaves mapped and 252 unmapped in 106.4 RM map call(s), walk 2703 us, worker apply 70518 us per rep
- events naming a host RM call 317.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x140.4; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x53.6; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N built in N us: plan and apply N us, TSG N us, channel N us, objects N us, s x20.0
- device events per rep: doorbell eventfd rings (token pool) 120.4; guest MMU invalidates 97.4; host doorbells written (forwarded) 140.4; host notifier events (no MSI) 9.0; host notifier events -> MSI 53.6; other events 566.2

## op 4 cuCtxDestroy: 5 repetition(s), 725.0 trapped accesses per repetition, window median 165476 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 15931 |
| gsp-falcon | 207.0 | 12372 |
| bar2-flush-read | 169.0 | 50533 |
| gsp-queue-doorbell | 69.0 | 32379 |

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

- GSP RPCs 69.0 per rep (230 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 564 us, worker apply 9512 us per rep
- events naming a host RM call 62.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x34.4; free X class Some(N); mirror: chid N: host channel X freed in N us after N doorbell(s), N  x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x1.4; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 1.4; host notifier events -> MSI 34.4; other events 185.2

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 588 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 473 |
| bar2-flush-read | 1.0 | 326 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 11 us, worker apply 534 us per rep
- events naming a host RM call 24.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x24.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 0.5; host notifier events (no MSI) 0.1; host notifier events -> MSI 24.0; other events 0.8

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 606 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 357 |
| bar2-flush-read | 1.5 | 284 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 10 us, worker apply 458 us per rep
- events naming a host RM call 7.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x7.5; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 1.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 7.5; other events 0.7

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 3707 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 447 |
| bar2-flush-read | 2.3 | 771 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 36 us, worker apply 4782 us per rep
- events naming a host RM call 10.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x10.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.6
- device events per rep: guest MMU invalidates 1.1; host notifier events (no MSI) 0.6; host notifier events -> MSI 10.0; other events 1.9

## op 13 cuMemFree_64M: 10 repetition(s), 13.1 trapped accesses per repetition, window median 1946 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 942 |
| bar2-flush-read | 3.1 | 805 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 41 us, worker apply 5199 us per rep
- events naming a host RM call 8.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x8.2; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.3
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.3; host notifier events -> MSI 8.2; other events 1.3

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 2878 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 472 |
| bar2-flush-read | 2.5 | 834 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 0.5 RM map call(s), walk 23 us, worker apply 726 us per rep
- events naming a host RM call 7.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x7.0
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 7.0; other events 2.0

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.0 trapped accesses per repetition, window median 1438 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 431 |
| bar2-flush-read | 2.0 | 810 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 16 us, worker apply 688 us per rep
- events naming a host RM call 7.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x7.7
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 7.7; other events 1.0

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.0 trapped accesses per repetition, window median 4332 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1610 |
| bar2-flush-read | 5.0 | 1762 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 2.3 RM map call(s), walk 34 us, worker apply 1720 us per rep
- events naming a host RM call 9.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x9.1
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 9.1; other events 2.3

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.0 trapped accesses per repetition, window median 2186 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 751 |
| bar2-flush-read | 4.0 | 1317 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 27 us, worker apply 946 us per rep
- events naming a host RM call 8.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x8.1
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 8.1; other events 1.2

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 627 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 40207 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1012.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 584 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 476 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 898 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5329 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 2.9

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5250 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 2.6

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 4088 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 594 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1726 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.3

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 53 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 816 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 886 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.5

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 70 us



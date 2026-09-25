# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/before-2/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 0.0 | 100177 | 100086.3 |
| 2 | cuInit | 3 | 174.3 | 159969 | 159849.2 |
| 3 | cuCtxCreate | 5 | 1597.6 | 1896527 | 1896420.5 |
| 4 | cuCtxDestroy | 5 | 725.0 | 408175 | 408122.1 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 715 | 636.4 |
| 11 | cuMemFree_1M | 20 | 6.5 | 1456 | 1300.0 |
| 12 | cuMemAlloc_64M | 10 | 7.8 | 9862 | 9801.4 |
| 13 | cuMemFree_64M | 10 | 13.1 | 4186 | 4142.7 |
| 14 | cuMemHostAlloc_1M | 10 | 7.5 | 2725 | 2630.2 |
| 15 | cuMemFreeHost_1M | 10 | 7.0 | 1656 | 1565.6 |
| 16 | cuMemHostRegister_1M | 10 | 15.0 | 7568 | 7501.6 |
| 17 | cuMemHostUnregister_1M | 10 | 14.0 | 2690 | 2644.9 |
| 20 | launch_1_sync | 20 | 0.0 | 816 | 721.2 |
| 21 | launch_1000_sync | 1 | 0.0 | 48588 | 26.0 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 420 | 379.6 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 414 | 375.4 |
| 32 | d2d_1M | 10 | 0.0 | 1014 | 966.9 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 5412 | 5352.0 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 5561 | 5503.6 |
| 35 | d2d_16M | 10 | 0.0 | 4186 | 4121.2 |
| 36 | h2d_pageable_1M | 10 | 4.5 | 605 | 545.1 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 619 | 573.9 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 52 | 12.2 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 702 | 659.4 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 854 | 782.7 |
| 51 | cuModuleUnload | 10 | 0.0 | 92 | 38.6 |

## op 1 idle_100ms: 10 repetition(s), 0.0 trapped accesses per repetition, window median 100177 us

- device events per rep: other events 13.0

## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 159969 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 21504 |
| mmu-invalidate | 40.0 | 8077 |
| gsp-queue-doorbell | 27.3 | 14097 |
| bar2-flush-read | 24.0 | 10495 |
| pcfg-mirror (intr re-arm) | 1.0 | 213 |

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

- GSP RPCs 27.3 per rep (250 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 57.0

## op 3 cuCtxCreate: 5 repetition(s), 1597.6 trapped accesses per repetition, window median 1896527 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 487.0 | 52928 |
| pci-config | 410.4 | 12906 |
| bar2-flush-read | 272.2 | 101893 |
| gsp-falcon | 255.0 | 22942 |
| gsp-queue-doorbell | 85.0 | 19573 |
| usermode page, other | 60.0 | 5128 |
| usermode-doorbell | 20.0 | 1044 |
| bar0-other | 8.0 | 1889 |

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
| PCI config |  | R | 0x34 | 46.40 |
| PCI config |  | R | 0x60 | 46.40 |
| PCI config |  | R | 0x68 | 46.40 |
| (rest) | | | | 359.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): tokens 20.0

- GSP RPCs 85.0 per rep (433 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.0 per rep: 20130 leaves mapped and 252 unmapped in 111.6 RM map call(s), walk 2719 us, worker apply 123386 us per rep
- events naming a host RM call 451.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x161.2; doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x112.6; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x83.4; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0
- device events per rep: doorbell eventfd rings (token pool) 92.6; guest MMU invalidates 97.4; host doorbells written (forwarded) 112.6; host notifier events (no MSI) 161.2; host notifier events -> MSI 83.4; other events 548.8

## op 4 cuCtxDestroy: 5 repetition(s), 725.0 trapped accesses per repetition, window median 408175 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 28391 |
| gsp-falcon | 207.0 | 20562 |
| bar2-flush-read | 169.0 | 74205 |
| gsp-queue-doorbell | 69.0 | 29914 |

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

- GSP RPCs 69.0 per rep (346 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 455 us, worker apply 23286 us per rep
- events naming a host RM call 129.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x51.8; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x51.0; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 51.0; host notifier events -> MSI 51.8; other events 203.8

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 715 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 567 |
| bar2-flush-read | 1.0 | 306 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 9 us, worker apply 275 us per rep
- events naming a host RM call 13.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x13.2; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 0.5; host notifier events (no MSI) 0.1; host notifier events -> MSI 13.2; other events 0.7

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 1456 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 642 |
| bar2-flush-read | 1.5 | 429 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 9 us, worker apply 999 us per rep
- events naming a host RM call 4.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.8
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.8; other events 0.9

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 9862 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 534 |
| bar2-flush-read | 2.3 | 680 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 47 us, worker apply 11331 us per rep
- events naming a host RM call 6.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.3; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.6
- device events per rep: guest MMU invalidates 1.1; host notifier events (no MSI) 0.6; host notifier events -> MSI 6.3; other events 2.5

## op 13 cuMemFree_64M: 10 repetition(s), 13.1 trapped accesses per repetition, window median 4186 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1145 |
| bar2-flush-read | 3.1 | 744 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 3 leaves mapped and 32 unmapped in 3.2 RM map call(s), walk 39 us, worker apply 13198 us per rep
- events naming a host RM call 6.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.7; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x1.1
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 1.1; host notifier events -> MSI 5.7; other events 2.3

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 2725 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 784 |
| bar2-flush-read | 2.5 | 1116 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.4 per rep: 205 leaves mapped and 0 unmapped in 0.4 RM map call(s), walk 19 us, worker apply 454 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.5; other events 1.0

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.0 trapped accesses per repetition, window median 1656 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 487 |
| bar2-flush-read | 2.0 | 894 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.7 RM map call(s), walk 14 us, worker apply 5802 us per rep
- events naming a host RM call 6.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.6; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.4
- device events per rep: guest MMU invalidates 1.0; host notifier events (no MSI) 0.4; host notifier events -> MSI 5.6; other events 1.1

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.0 trapped accesses per repetition, window median 7568 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1447 |
| bar2-flush-read | 5.0 | 2009 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 2.5 RM map call(s), walk 420 us, worker apply 24819 us per rep
- events naming a host RM call 9.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.1; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x3.1
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 3.1; host notifier events -> MSI 6.1; other events 2.2

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.0 trapped accesses per repetition, window median 2690 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1115 |
| bar2-flush-read | 4.0 | 1241 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.2 RM map call(s), walk 31 us, worker apply 399 us per rep
- events naming a host RM call 4.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.9
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.9; other events 1.5

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 816 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 48588 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1006.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 420 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 414 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1014 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.4

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5412 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.8

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5561 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 2.0

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 4186 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 36 h2d_pageable_1M: 10 repetition(s), 4.5 trapped accesses per repetition, window median 605 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 4.5 | 70 |

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
| PCI config |  | R | 0xb8 | 0.10 |
| PCI config |  | R | 0xbc | 0.10 |
| PCI config |  | R | 0xc0 | 0.10 |
| PCI config |  | R | 0xc4 | 0.10 |
| (rest) | | | | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 619 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 52 us

- device events per rep: other events 0.1

## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 702 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 854 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 92 us



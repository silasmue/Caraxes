# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/on-nopool/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 20 | 66.4 | 100148 | 100085.4 |
| 2 | cuInit | 3 | 174.3 | 20493 | 20411.0 |
| 3 | cuCtxCreate | 3 | 1884.0 | 874039 | 874001.6 |
| 4 | cuCtxDestroy | 3 | 924.0 | 182727 | 182666.7 |
| 10 | cuMemAlloc_1M | 40 | 3.6 | 320 | 305.9 |
| 11 | cuMemFree_1M | 40 | 6.6 | 440 | 452.9 |
| 12 | cuMemAlloc_64M | 20 | 8.0 | 987 | 858.2 |
| 13 | cuMemFree_64M | 20 | 13.2 | 1396 | 1428.3 |
| 14 | cuMemHostAlloc_1M | 20 | 8.2 | 1684 | 1650.5 |
| 15 | cuMemFreeHost_1M | 20 | 7.1 | 1100 | 1104.8 |
| 16 | cuMemHostRegister_1M | 20 | 15.2 | 1760 | 1727.1 |
| 17 | cuMemHostUnregister_1M | 20 | 14.2 | 1692 | 1651.0 |
| 20 | launch_1_sync | 40 | 3.1 | 810 | 771.6 |
| 21 | launch_1000_sync | 2 | 3560.0 | 341650 | 130.2 |
| 30 | h2d_pinned_1M | 20 | 3.0 | 561 | 502.4 |
| 31 | d2h_pinned_1M | 20 | 3.0 | 532 | 479.6 |
| 32 | d2d_1M | 20 | 3.4 | 970 | 936.3 |
| 33 | h2d_pinned_16M | 20 | 4.0 | 5340 | 5284.1 |
| 34 | d2h_pinned_16M | 20 | 3.6 | 5340 | 5268.5 |
| 35 | d2d_16M | 20 | 3.2 | 3981 | 3933.8 |
| 36 | h2d_pageable_1M | 20 | 3.1 | 664 | 613.5 |
| 37 | d2h_pageable_1M | 20 | 3.1 | 677 | 622.7 |
| 40 | cuStreamSynchronize_idle | 40 | 0.0 | 38 | 3.1 |
| 41 | cuEventRecord_Synchronize | 40 | 3.1 | 796 | 762.0 |
| 50 | cuModuleLoad_cubin | 20 | 3.1 | 882 | 879.4 |
| 51 | cuModuleUnload | 20 | 0.1 | 64 | 17.6 |

## op 1 idle_100ms: 20 repetition(s), 66.4 trapped accesses per repetition, window median 100148 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 51.3 | 1081 |
| pcfg-mirror (intr re-arm) | 15.1 | 349 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 15.10 |
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
| (rest) | | | | 5.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none


## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 20493 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 4232 |
| mmu-invalidate | 40.0 | 2090 |
| gsp-queue-doorbell | 27.3 | 9336 |
| bar2-flush-read | 24.0 | 1642 |
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

- GSP RPCs 27.3 per rep (94 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 54.7

## op 3 cuCtxCreate: 3 repetition(s), 1884.0 trapped accesses per repetition, window median 874039 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 642.0 | 44740 |
| mmu-invalidate | 488.3 | 22901 |
| bar2-flush-read | 273.0 | 71209 |
| gsp-falcon | 255.0 | 10196 |
| gsp-queue-doorbell | 85.0 | 13854 |
| usermode page, other | 60.0 | 2951 |
| pcfg-mirror (intr re-arm) | 52.7 | 6924 |
| usermode-doorbell | 20.0 | 661 |
| bar0-other | 8.0 | 1699 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 424.00 |
| BAR2 | flush page | R |  | 273.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 218.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 195.33 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.67 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.67 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.67 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 85.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 85.00 |
| BAR0 | 0x110000 | R | 0x110804 NV_PGSP | 85.00 |
| BAR0 | 0x110000 | R | 0x110808 NV_PGSP | 85.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 52.67 |
| (rest) | | | | 88.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 642.0, tokens 20.0

- GSP RPCs 85.0 per rep (381 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.7 per rep: 19619 leaves mapped and 5031 unmapped in 6041.7 RM map call(s), walk 2336 us, worker apply 761219 us per rep
- events naming a host RM call 464.7 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x214.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x156.0; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 194.0; guest MMU invalidates 97.7; host doorbells written (forwarded) 214.0; host notifier events -> MSI 156.0; other events 565.3

## op 4 cuCtxDestroy: 3 repetition(s), 924.0 trapped accesses per repetition, window median 182727 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 13735 |
| gsp-falcon | 207.0 | 11204 |
| pci-config | 171.0 | 3524 |
| bar2-flush-read | 169.0 | 53724 |
| gsp-queue-doorbell | 69.0 | 29602 |
| pcfg-mirror (intr re-arm) | 28.0 | 2222 |

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
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 28.00 |
| PCI config |  | R | 0x34 | 19.33 |
| PCI config |  | R | 0x60 | 19.33 |
| (rest) | | | | 132.33 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (290 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.3 per rep: 0 leaves mapped and 12507 unmapped in 0.0 RM map call(s), walk 253 us, worker apply 40283 us per rep
- events naming a host RM call 62.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x36.3; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X: host TSG X (engineType X) freed [host] [synthesized x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X: host TSG X (engineType X) freed [host]; shadow: VAS x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events -> MSI 36.3; other events 153.3

## op 10 cuMemAlloc_1M: 40 repetition(s), 3.6 trapped accesses per repetition, window median 320 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 163 |
| bar2-flush-read | 1.0 | 208 |
| pcfg-mirror (intr re-arm) | 0.1 | 28 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.07 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 7 us, worker apply 63 us per rep
- events naming a host RM call 14.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.3
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 14.3; other events 0.5

## op 11 cuMemFree_1M: 40 repetition(s), 6.6 trapped accesses per repetition, window median 440 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 253 |
| bar2-flush-read | 1.5 | 234 |
| pcfg-mirror (intr re-arm) | 0.1 | 3 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.12 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 8 us, worker apply 256 us per rep
- events naming a host RM call 4.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.4; shadow: VAS of guest root X freed: N syncs (N skipped clean, N full walks, N table reads,  x0.0
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.4; other events 0.6

## op 12 cuMemAlloc_64M: 20 repetition(s), 8.0 trapped accesses per repetition, window median 987 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 308 |
| bar2-flush-read | 2.3 | 573 |
| pcfg-mirror (intr re-arm) | 0.2 | 20 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 146 us, worker apply 4447 us per rep
- events naming a host RM call 5.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.1
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 5.1; other events 0.9

## op 13 cuMemFree_64M: 20 repetition(s), 13.2 trapped accesses per repetition, window median 1396 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 619 |
| bar2-flush-read | 3.1 | 673 |
| pcfg-mirror (intr re-arm) | 0.1 | 21 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 34 us, worker apply 4997 us per rep
- events naming a host RM call 4.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.7
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.7; other events 1.0

## op 14 cuMemHostAlloc_1M: 20 repetition(s), 8.2 trapped accesses per repetition, window median 1684 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 236 |
| bar2-flush-read | 2.5 | 1254 |
| pci-config | 0.5 | 7 |
| pcfg-mirror (intr re-arm) | 0.2 | 27 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
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

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 18.8 RM map call(s), walk 20 us, worker apply 2926 us per rep
- events naming a host RM call 4.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.3
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.3; other events 0.5

## op 15 cuMemFreeHost_1M: 20 repetition(s), 7.1 trapped accesses per repetition, window median 1100 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 216 |
| bar2-flush-read | 2.0 | 822 |
| pcfg-mirror (intr re-arm) | 0.1 | 7 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 51 leaves mapped and 205 unmapped in 3.0 RM map call(s), walk 12 us, worker apply 1682 us per rep
- events naming a host RM call 5.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.2
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.2; other events 0.5

## op 16 cuMemHostRegister_1M: 20 repetition(s), 15.2 trapped accesses per repetition, window median 1760 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 468 |
| bar2-flush-read | 5.0 | 1218 |
| pcfg-mirror (intr re-arm) | 0.2 | 11 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 230 leaves mapped and 51 unmapped in 15.2 RM map call(s), walk 25 us, worker apply 3626 us per rep
- events naming a host RM call 4.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.8
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.8; other events 1.0

## op 17 cuMemHostUnregister_1M: 20 repetition(s), 14.2 trapped accesses per repetition, window median 1692 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 503 |
| bar2-flush-read | 4.0 | 1095 |
| pcfg-mirror (intr re-arm) | 0.2 | 9 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 26 leaves mapped and 230 unmapped in 0.3 RM map call(s), walk 15 us, worker apply 1073 us per rep
- events naming a host RM call 4.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.8
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.8; other events 1.0

## op 20 launch_1_sync: 40 repetition(s), 3.1 trapped accesses per repetition, window median 810 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 159 |
| pcfg-mirror (intr re-arm) | 0.1 | 66 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 21 launch_1000_sync: 2 repetition(s), 3560.0 trapped accesses per repetition, window median 341650 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3000.0 | 241392 |
| pci-config | 513.0 | 8681 |
| pcfg-mirror (intr re-arm) | 47.0 | 24204 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 1984.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1016.00 |
| PCI config |  | R | 0x34 | 58.00 |
| PCI config |  | R | 0x60 | 58.00 |
| PCI config |  | R | 0x68 | 58.00 |
| PCI config |  | R | 0x78 | 58.00 |
| PCI config |  | R | 0x6 | 57.00 |
| PCI config |  | R | 0x61 | 57.00 |
| PCI config |  | R | 0x69 | 57.00 |
| PCI config |  | R | 0x79 | 57.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 47.00 |
| PCI config |  | R | 0x0 | 1.00 |
| (rest) | | | | 52.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3000.0

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1000.0

## op 30 h2d_pinned_1M: 20 repetition(s), 3.0 trapped accesses per repetition, window median 561 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 183 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 31 d2h_pinned_1M: 20 repetition(s), 3.0 trapped accesses per repetition, window median 532 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 144 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 32 d2d_1M: 20 repetition(s), 3.4 trapped accesses per repetition, window median 970 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 180 |
| pcfg-mirror (intr re-arm) | 0.3 | 163 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.35 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 33 h2d_pinned_16M: 20 repetition(s), 4.0 trapped accesses per repetition, window median 5340 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 164 |
| pcfg-mirror (intr re-arm) | 1.1 | 239 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 1.05 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 34 d2h_pinned_16M: 20 repetition(s), 3.6 trapped accesses per repetition, window median 5340 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 137 |
| pcfg-mirror (intr re-arm) | 0.6 | 239 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.60 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 35 d2d_16M: 20 repetition(s), 3.2 trapped accesses per repetition, window median 3981 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 173 |
| pcfg-mirror (intr re-arm) | 0.2 | 24 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 36 h2d_pageable_1M: 20 repetition(s), 3.1 trapped accesses per repetition, window median 664 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 295 |
| pcfg-mirror (intr re-arm) | 0.1 | 29 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 20 repetition(s), 3.1 trapped accesses per repetition, window median 677 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 163 |
| pcfg-mirror (intr re-arm) | 0.1 | 21 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 1.90 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.10 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 40 repetition(s), 0.0 trapped accesses per repetition, window median 38 us


## op 41 cuEventRecord_Synchronize: 40 repetition(s), 3.1 trapped accesses per repetition, window median 796 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 127 |
| pcfg-mirror (intr re-arm) | 0.1 | 76 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 1.95 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.05 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 50 cuModuleLoad_cubin: 20 repetition(s), 3.1 trapped accesses per repetition, window median 882 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 234 |
| pcfg-mirror (intr re-arm) | 0.1 | 43 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 51 cuModuleUnload: 20 repetition(s), 0.1 trapped accesses per repetition, window median 64 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 2 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none



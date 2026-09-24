# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/off-2/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 20 | 40.6 | 100124 | 100073.5 |
| 2 | cuInit | 3 | 3290.3 | 173203 | 173129.8 |
| 3 | cuCtxCreate | 3 | 74762.7 | 3432562 | 3432495.6 |
| 4 | cuCtxDestroy | 3 | 21564.7 | 907482 | 907435.8 |
| 10 | cuMemAlloc_1M | 40 | 5.6 | 337 | 297.8 |
| 11 | cuMemFree_1M | 40 | 7.5 | 410 | 343.4 |
| 12 | cuMemAlloc_64M | 20 | 188.4 | 4261 | 4344.7 |
| 13 | cuMemFree_64M | 20 | 77.8 | 2604 | 2571.8 |
| 14 | cuMemHostAlloc_1M | 20 | 808.5 | 24517 | 24678.1 |
| 15 | cuMemFreeHost_1M | 20 | 265.3 | 7800 | 7868.0 |
| 16 | cuMemHostRegister_1M | 20 | 1051.5 | 43454 | 41081.5 |
| 17 | cuMemHostUnregister_1M | 20 | 275.1 | 11687 | 11232.2 |
| 20 | launch_1_sync | 40 | 0.1 | 718 | 680.2 |
| 21 | launch_1000_sync | 2 | 4.0 | 30042 | 19.6 |
| 30 | h2d_pinned_1M | 20 | 0.0 | 406 | 366.0 |
| 31 | d2h_pinned_1M | 20 | 0.9 | 440 | 383.9 |
| 32 | d2d_1M | 20 | 0.2 | 931 | 874.7 |
| 33 | h2d_pinned_16M | 20 | 0.7 | 5241 | 5197.6 |
| 34 | d2h_pinned_16M | 20 | 2.4 | 5212 | 5147.9 |
| 35 | d2d_16M | 20 | 0.2 | 3946 | 3849.9 |
| 36 | h2d_pageable_1M | 20 | 0.1 | 552 | 495.2 |
| 37 | d2h_pageable_1M | 20 | 0.1 | 570 | 520.2 |
| 40 | cuStreamSynchronize_idle | 40 | 0.0 | 50 | 6.3 |
| 41 | cuEventRecord_Synchronize | 40 | 0.1 | 709 | 659.5 |
| 50 | cuModuleLoad_cubin | 20 | 0.2 | 842 | 792.1 |
| 51 | cuModuleUnload | 20 | 0.0 | 74 | 25.9 |

## op 1 idle_100ms: 20 repetition(s), 40.6 trapped accesses per repetition, window median 100124 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 25.6 | 488 |
| pcfg-mirror (intr re-arm) | 14.9 | 375 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 14.95 |
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
| (rest) | | | | 2.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none


## op 2 cuInit: 3 repetition(s), 3290.3 trapped accesses per repetition, window median 173203 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 3116.0 | 132484 |
| gsp-falcon | 82.0 | 2845 |
| mmu-invalidate | 40.0 | 1845 |
| gsp-queue-doorbell | 27.3 | 9176 |
| bar2-flush-read | 24.0 | 1672 |
| pcfg-mirror (intr re-arm) | 1.0 | 28 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 3116.00 |
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

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 3116.0

- GSP RPCs 27.3 per rep (95 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 54.7

## op 3 cuCtxCreate: 3 repetition(s), 74762.7 trapped accesses per repetition, window median 3432562 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 72736.3 | 2535294 |
| pci-config | 513.0 | 7890 |
| mmu-invalidate | 488.3 | 20228 |
| pcfg-mirror (intr re-arm) | 324.0 | 23639 |
| bar2-flush-read | 273.0 | 11860 |
| gsp-falcon | 255.0 | 9029 |
| gsp-queue-doorbell | 85.0 | 10266 |
| usermode page, other | 60.0 | 2512 |
| usermode-doorbell | 20.0 | 0 |
| bar0-other | 8.0 | 1484 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 72736.33 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 324.00 |
| BAR2 | flush page | R |  | 273.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 195.33 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.67 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.67 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.67 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 85.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 85.00 |
| BAR0 | 0x110000 | R | 0x110804 NV_PGSP | 85.00 |
| BAR0 | 0x110000 | R | 0x110808 NV_PGSP | 85.00 |
| PCI config |  | R | 0x34 | 58.00 |
| (rest) | | | | 543.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 72736.3, tokens 20.0

- GSP RPCs 85.0 per rep (316 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 19.7 per rep: 20046 leaves mapped and 168 unmapped in 6268.7 RM map call(s), walk 4141 us, worker apply 228607 us per rep
- events naming a host RM call 627.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x368.3; doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x161.7; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 141.7; guest MMU invalidates 97.7; host doorbells written (forwarded) 161.7; host notifier events (no MSI) 3.0; host notifier events -> MSI 368.3; other events 505.3

## op 4 cuCtxDestroy: 3 repetition(s), 21564.7 trapped accesses per repetition, window median 907482 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 20360.0 | 687134 |
| pci-config | 342.0 | 6035 |
| mmu-invalidate | 280.0 | 11503 |
| gsp-falcon | 207.0 | 8576 |
| bar2-flush-read | 169.0 | 8246 |
| pcfg-mirror (intr re-arm) | 137.7 | 6316 |
| gsp-queue-doorbell | 69.0 | 26292 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 20360.00 |
| BAR2 | flush page | R |  | 169.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 137.67 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 112.00 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 69.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | R | 0x110804 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | R | 0x110808 NV_PGSP | 69.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| PCI config |  | R | 0x34 | 38.67 |
| (rest) | | | | 303.33 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 20360.0

- GSP RPCs 69.0 per rep (224 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0; GSP_RM_CONTROL cmd 0x802009 x1.0
- shadow plans 16.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 230 us, worker apply 530910 us per rep
- events naming a host RM call 241.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x214.0; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X: host TSG X (engineType X) freed [host] [synthesized x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X: host TSG X (engineType X) freed [host]; shadow: VAS x1.0; shadow: VAS of guest root X freed: N syncs (N skipped clean, N full walks, N table reads,  x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events -> MSI 214.0; other events 155.0

## op 10 cuMemAlloc_1M: 40 repetition(s), 5.6 trapped accesses per repetition, window median 337 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 220 |
| bar2-write | 2.0 | 175 |
| bar2-flush-read | 1.0 | 37 |
| pcfg-mirror (intr re-arm) | 0.1 | 10 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 2.00 |
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.12 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 2.0

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 54 us, worker apply 261 us per rep
- events naming a host RM call 14.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.3
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 14.3; other events 0.5

## op 11 cuMemFree_1M: 40 repetition(s), 7.5 trapped accesses per repetition, window median 410 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 227 |
| bar2-flush-read | 1.5 | 89 |
| bar2-write | 1.0 | 94 |
| pcfg-mirror (intr re-arm) | 0.0 | 2 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR2 | other pages | W |  | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.03 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 1.0

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 7 us, worker apply 178 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.5; other events 0.5

## op 12 cuMemAlloc_64M: 20 repetition(s), 188.4 trapped accesses per repetition, window median 4261 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 179.6 | 5978 |
| mmu-invalidate | 5.5 | 230 |
| bar2-flush-read | 2.3 | 76 |
| pcfg-mirror (intr re-arm) | 1.1 | 35 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 179.60 |
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 1.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 179.6

- shadow plans 1.0 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 35 us, worker apply 4998 us per rep
- events naming a host RM call 6.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.0
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 6.0; other events 1.0

## op 13 cuMemFree_64M: 20 repetition(s), 77.8 trapped accesses per repetition, window median 2604 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 64.1 | 2220 |
| mmu-invalidate | 10.0 | 482 |
| bar2-flush-read | 3.1 | 155 |
| pcfg-mirror (intr re-arm) | 0.6 | 31 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 64.10 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.55 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 64.1

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 31 us, worker apply 4930 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.0; other events 1.0

## op 14 cuMemHostAlloc_1M: 20 repetition(s), 808.5 trapped accesses per repetition, window median 24517 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 771.0 | 24835 |
| pci-config | 25.6 | 329 |
| mmu-invalidate | 5.0 | 301 |
| pcfg-mirror (intr re-arm) | 4.3 | 296 |
| bar2-flush-read | 2.5 | 117 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 771.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 4.30 |
| PCI config |  | R | 0x34 | 2.90 |
| PCI config |  | R | 0x60 | 2.90 |
| PCI config |  | R | 0x68 | 2.90 |
| PCI config |  | R | 0x78 | 2.90 |
| PCI config |  | R | 0x6 | 2.85 |
| PCI config |  | R | 0x61 | 2.85 |
| PCI config |  | R | 0x69 | 2.85 |
| PCI config |  | R | 0x79 | 2.85 |
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| (rest) | | | | 5.65 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 771.0

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 26.9 RM map call(s), walk 50 us, worker apply 3014 us per rep
- events naming a host RM call 8.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x8.4
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 8.4; other events 0.5

## op 15 cuMemFreeHost_1M: 20 repetition(s), 265.3 trapped accesses per repetition, window median 7800 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 257.0 | 7903 |
| mmu-invalidate | 5.0 | 244 |
| bar2-flush-read | 2.0 | 113 |
| pcfg-mirror (intr re-arm) | 1.3 | 126 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 257.00 |
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 1.30 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 257.0

- shadow plans 0.5 per rep: 51 leaves mapped and 205 unmapped in 0.6 RM map call(s), walk 18 us, worker apply 2721 us per rep
- events naming a host RM call 6.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.2
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 6.2; other events 0.5

## op 16 cuMemHostRegister_1M: 20 repetition(s), 1051.5 trapped accesses per repetition, window median 43454 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 1030.0 | 37823 |
| mmu-invalidate | 10.0 | 648 |
| pcfg-mirror (intr re-arm) | 6.5 | 451 |
| bar2-flush-read | 5.0 | 247 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 1030.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 6.50 |
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 1030.0

- shadow plans 1.0 per rep: 230 leaves mapped and 51 unmapped in 25.1 RM map call(s), walk 109 us, worker apply 3364 us per rep
- events naming a host RM call 10.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x10.9
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 10.9; other events 1.0

## op 17 cuMemHostUnregister_1M: 20 repetition(s), 275.1 trapped accesses per repetition, window median 11687 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 258.0 | 13278 |
| mmu-invalidate | 10.0 | 608 |
| bar2-flush-read | 4.0 | 211 |
| pcfg-mirror (intr re-arm) | 3.1 | 783 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 258.00 |
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 258.0

- shadow plans 1.0 per rep: 26 leaves mapped and 230 unmapped in 0.2 RM map call(s), walk 42 us, worker apply 1207 us per rep
- events naming a host RM call 7.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x7.5
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 7.5; other events 1.0

## op 20 launch_1_sync: 40 repetition(s), 0.1 trapped accesses per repetition, window median 718 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 64 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 21 launch_1000_sync: 2 repetition(s), 4.0 trapped accesses per repetition, window median 30042 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 4.0 | 0 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 4.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1000.0

## op 30 h2d_pinned_1M: 20 repetition(s), 0.0 trapped accesses per repetition, window median 406 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 31 d2h_pinned_1M: 20 repetition(s), 0.9 trapped accesses per repetition, window median 440 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 0.9 | 24 |
| pcfg-mirror (intr re-arm) | 0.1 | 13 |

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
| PCI config |  | R | 0x4 | 0.05 |
| PCI config |  | R | 0x8 | 0.05 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 32 d2d_1M: 20 repetition(s), 0.2 trapped accesses per repetition, window median 931 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.2 | 99 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 33 h2d_pinned_16M: 20 repetition(s), 0.7 trapped accesses per repetition, window median 5241 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.7 | 239 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.70 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 34 d2h_pinned_16M: 20 repetition(s), 2.4 trapped accesses per repetition, window median 5212 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 1.8 | 36 |
| pcfg-mirror (intr re-arm) | 0.6 | 194 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.60 |
| PCI config |  | R | 0x6 | 0.20 |
| PCI config |  | R | 0x34 | 0.20 |
| PCI config |  | R | 0x60 | 0.20 |
| PCI config |  | R | 0x61 | 0.20 |
| PCI config |  | R | 0x68 | 0.20 |
| PCI config |  | R | 0x69 | 0.20 |
| PCI config |  | R | 0x78 | 0.20 |
| PCI config |  | R | 0x79 | 0.20 |
| PCI config |  | R | 0x0 | 0.05 |
| PCI config |  | R | 0x4 | 0.05 |
| PCI config |  | R | 0x8 | 0.05 |
| (rest) | | | | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 35 d2d_16M: 20 repetition(s), 0.2 trapped accesses per repetition, window median 3946 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.2 | 136 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.25 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 36 h2d_pageable_1M: 20 repetition(s), 0.1 trapped accesses per repetition, window median 552 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 39 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 20 repetition(s), 0.1 trapped accesses per repetition, window median 570 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 21 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 40 repetition(s), 0.0 trapped accesses per repetition, window median 50 us


## op 41 cuEventRecord_Synchronize: 40 repetition(s), 0.1 trapped accesses per repetition, window median 709 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 66 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.12 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 50 cuModuleLoad_cubin: 20 repetition(s), 0.2 trapped accesses per repetition, window median 842 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.2 | 129 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 51 cuModuleUnload: 20 repetition(s), 0.0 trapped accesses per repetition, window median 74 us



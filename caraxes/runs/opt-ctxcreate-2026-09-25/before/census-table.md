# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/before/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 51.3 | 100146 | 100075.2 |
| 2 | cuInit | 3 | 174.3 | 13876 | 13819.1 |
| 3 | cuCtxCreate | 5 | 1289.8 | 356087 | 355995.8 |
| 4 | cuCtxDestroy | 5 | 725.0 | 151821 | 151766.3 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 380 | 332.2 |
| 11 | cuMemFree_1M | 20 | 6.5 | 453 | 404.0 |
| 12 | cuMemAlloc_64M | 10 | 9.6 | 3030 | 2971.6 |
| 13 | cuMemFree_64M | 10 | 13.1 | 5413 | 5369.8 |
| 14 | cuMemHostAlloc_1M | 10 | 7.5 | 2476 | 2321.8 |
| 15 | cuMemFreeHost_1M | 10 | 7.0 | 1228 | 1189.6 |
| 16 | cuMemHostRegister_1M | 10 | 15.0 | 3723 | 3667.9 |
| 17 | cuMemHostUnregister_1M | 10 | 14.0 | 2578 | 2512.5 |
| 20 | launch_1_sync | 20 | 0.0 | 702 | 651.3 |
| 21 | launch_1000_sync | 1 | 0.0 | 32077 | 25.5 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 434 | 385.7 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 423 | 378.6 |
| 32 | d2d_1M | 10 | 0.0 | 924 | 884.6 |
| 33 | h2d_pinned_16M | 10 | 32.4 | 5316 | 5247.7 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 5332 | 5277.1 |
| 35 | d2d_16M | 10 | 0.0 | 4058 | 3980.9 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 534 | 476.0 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 562 | 509.2 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 48 | 5.5 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 730 | 672.9 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 762 | 714.3 |
| 51 | cuModuleUnload | 10 | 0.0 | 60 | 20.3 |

## op 1 idle_100ms: 10 repetition(s), 51.3 trapped accesses per repetition, window median 100146 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 51.3 | 898 |

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

- device events per rep: other events 14.9

## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 13876 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 3181 |
| mmu-invalidate | 40.0 | 1607 |
| gsp-queue-doorbell | 27.3 | 8531 |
| bar2-flush-read | 24.0 | 2111 |
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

- GSP RPCs 27.3 per rep (82 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 55.7

## op 3 cuCtxCreate: 5 repetition(s), 1289.8 trapped accesses per repetition, window median 356087 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 487.0 | 29583 |
| bar2-flush-read | 272.2 | 62466 |
| gsp-falcon | 255.0 | 8973 |
| pci-config | 102.6 | 1643 |
| gsp-queue-doorbell | 85.0 | 12016 |
| usermode page, other | 60.0 | 2429 |
| usermode-doorbell | 20.0 | 7748 |
| bar0-other | 8.0 | 1378 |

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

- GSP RPCs 85.0 per rep (335 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.0 per rep: 20130 leaves mapped and 252 unmapped in 111.8 RM map call(s), walk 2737 us, worker apply 66736 us per rep
- events naming a host RM call 285.8 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x138.4; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x50.0; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X (X/X) [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 118.4; guest MMU invalidates 97.4; host doorbells written (forwarded) 138.4; host notifier events (no MSI) 3.0; host notifier events -> MSI 50.0; other events 526.6

## op 4 cuCtxDestroy: 5 repetition(s), 725.0 trapped accesses per repetition, window median 151821 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 17264 |
| gsp-falcon | 207.0 | 9534 |
| bar2-flush-read | 169.0 | 45709 |
| gsp-queue-doorbell | 69.0 | 28960 |

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

- GSP RPCs 69.0 per rep (206 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 254 us, worker apply 8747 us per rep
- events naming a host RM call 54.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x27.4; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0; shadow: VAS of guest root X freed: N syncs (N skipped clean, N full walks, N table reads,  x0.6
- device events per rep: guest MMU invalidates 56.0; host notifier events -> MSI 27.4; other events 177.6

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 380 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 272 |
| bar2-flush-read | 1.0 | 181 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 7 us, worker apply 505 us per rep
- events naming a host RM call 14.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.3
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 14.3; other events 0.6

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 453 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 293 |
| bar2-flush-read | 1.5 | 217 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 7 us, worker apply 203 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.5; other events 0.6

## op 12 cuMemAlloc_64M: 10 repetition(s), 9.6 trapped accesses per repetition, window median 3030 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 778 |
| bar2-flush-read | 2.3 | 566 |
| pci-config | 1.8 | 43 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| PCI config |  | R | 0x6 | 0.20 |
| PCI config |  | R | 0x34 | 0.20 |
| PCI config |  | R | 0x60 | 0.20 |
| PCI config |  | R | 0x61 | 0.20 |
| PCI config |  | R | 0x68 | 0.20 |
| PCI config |  | R | 0x69 | 0.20 |
| PCI config |  | R | 0x78 | 0.20 |
| (rest) | | | | 0.40 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 35 us, worker apply 4872 us per rep
- events naming a host RM call 5.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.8; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 1.1; host notifier events (no MSI) 0.1; host notifier events -> MSI 5.8; other events 1.5

## op 13 cuMemFree_64M: 10 repetition(s), 13.1 trapped accesses per repetition, window median 5413 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1108 |
| bar2-flush-read | 3.1 | 1211 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 3 leaves mapped and 32 unmapped in 3.2 RM map call(s), walk 234 us, worker apply 4654 us per rep
- events naming a host RM call 5.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.8
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.8; other events 2.0

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 2476 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 416 |
| bar2-flush-read | 2.5 | 1175 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.4 per rep: 205 leaves mapped and 0 unmapped in 0.7 RM map call(s), walk 16 us, worker apply 565 us per rep
- events naming a host RM call 4.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.5; other events 0.9

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.0 trapped accesses per repetition, window median 1228 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 486 |
| bar2-flush-read | 2.0 | 772 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.2 RM map call(s), walk 14 us, worker apply 248 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.0; other events 0.7

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.0 trapped accesses per repetition, window median 3723 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1250 |
| bar2-flush-read | 5.0 | 1306 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 1.4 RM map call(s), walk 249 us, worker apply 1677 us per rep
- events naming a host RM call 5.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.5
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.5; other events 1.5

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.0 trapped accesses per repetition, window median 2578 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1202 |
| bar2-flush-read | 4.0 | 1527 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.6 RM map call(s), walk 22 us, worker apply 1057 us per rep
- events naming a host RM call 5.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.3
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.3; other events 1.6

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 702 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 32077 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1004.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 434 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 423 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 924 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 33 h2d_pinned_16M: 10 repetition(s), 32.4 trapped accesses per repetition, window median 5316 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 32.4 | 519 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x60 | 3.70 |
| PCI config |  | R | 0x68 | 3.70 |
| PCI config |  | R | 0x78 | 3.70 |
| PCI config |  | R | 0x6 | 3.60 |
| PCI config |  | R | 0x34 | 3.60 |
| PCI config |  | R | 0x61 | 3.60 |
| PCI config |  | R | 0x69 | 3.60 |
| PCI config |  | R | 0x79 | 3.60 |
| PCI config |  | R | 0x64 | 0.10 |
| PCI config |  | R | 0x6c | 0.10 |
| PCI config |  | R | 0x70 | 0.10 |
| PCI config |  | R | 0x74 | 0.10 |
| (rest) | | | | 2.90 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.7

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5332 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 2.1

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 4058 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 534 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 562 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 48 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 730 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 762 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.3

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 60 us



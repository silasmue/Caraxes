# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/off/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 20 | 31.1 | 100152 | 100087.2 |
| 2 | cuInit | 3 | 3290.3 | 187212 | 187125.3 |
| 3 | cuCtxCreate | 3 | 75377.0 | 4740230 | 4740053.0 |
| 4 | cuCtxDestroy | 3 | 21450.0 | 1566943 | 1566862.1 |
| 10 | cuMemAlloc_1M | 40 | 5.5 | 422 | 286.4 |
| 11 | cuMemFree_1M | 40 | 11.3 | 360 | 337.0 |
| 12 | cuMemAlloc_64M | 20 | 187.8 | 4878 | 4863.0 |
| 13 | cuMemFree_64M | 20 | 77.5 | 3202 | 3868.1 |
| 14 | cuMemHostAlloc_1M | 20 | 806.2 | 24807 | 26911.4 |
| 15 | cuMemFreeHost_1M | 20 | 264.6 | 8192 | 9028.2 |
| 16 | cuMemHostRegister_1M | 20 | 1047.0 | 36977 | 37203.1 |
| 17 | cuMemHostUnregister_1M | 20 | 272.9 | 11976 | 11727.0 |
| 20 | launch_1_sync | 40 | 4.7 | 30498 | 30926.2 |
| 21 | launch_1000_sync | 2 | 1907.5 | 6721118 | 9952.5 |
| 30 | h2d_pinned_1M | 20 | 0.0 | 193 | 141.1 |
| 31 | d2h_pinned_1M | 20 | 0.0 | 205 | 149.4 |
| 32 | d2d_1M | 20 | 0.6 | 13938 | 15330.4 |
| 33 | h2d_pinned_16M | 20 | 0.3 | 1400 | 1879.9 |
| 34 | d2h_pinned_16M | 20 | 0.2 | 1408 | 1338.9 |
| 35 | d2d_16M | 20 | 2.5 | 15542 | 15709.9 |
| 36 | h2d_pageable_1M | 20 | 0.1 | 242 | 196.3 |
| 37 | d2h_pageable_1M | 20 | 0.1 | 316 | 257.5 |
| 40 | cuStreamSynchronize_idle | 40 | 0.0 | 44 | 5.7 |
| 41 | cuEventRecord_Synchronize | 40 | 0.6 | 14814 | 15697.5 |
| 50 | cuModuleLoad_cubin | 20 | 9.3 | 11178 | 11213.3 |
| 51 | cuModuleUnload | 20 | 0.0 | 68 | 25.6 |

## op 1 idle_100ms: 20 repetition(s), 31.1 trapped accesses per repetition, window median 100152 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 25.6 | 466 |
| pcfg-mirror (intr re-arm) | 5.5 | 223 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 5.45 |
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


## op 2 cuInit: 3 repetition(s), 3290.3 trapped accesses per repetition, window median 187212 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 3116.0 | 139370 |
| gsp-falcon | 82.0 | 3912 |
| mmu-invalidate | 40.0 | 2121 |
| gsp-queue-doorbell | 27.3 | 9208 |
| bar2-flush-read | 24.0 | 1272 |
| pcfg-mirror (intr re-arm) | 1.0 | 52 |

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

- GSP RPCs 27.3 per rep (121 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 54.7

## op 3 cuCtxCreate: 3 repetition(s), 75377.0 trapped accesses per repetition, window median 4740230 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 72736.3 | 3277851 |
| pci-config | 993.0 | 19861 |
| mmu-invalidate | 488.3 | 31210 |
| pcfg-mirror (intr re-arm) | 458.3 | 48420 |
| bar2-flush-read | 273.0 | 27193 |
| gsp-falcon | 255.0 | 12646 |
| gsp-queue-doorbell | 85.0 | 15111 |
| usermode page, other | 60.0 | 3515 |
| usermode-doorbell | 20.0 | 2314 |
| bar0-other | 8.0 | 1727 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 72736.33 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 458.33 |
| BAR2 | flush page | R |  | 273.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 195.33 |
| PCI config |  | R | 0x34 | 112.33 |
| PCI config |  | R | 0x78 | 112.33 |
| PCI config |  | R | 0x60 | 112.00 |
| PCI config |  | R | 0x68 | 112.00 |
| PCI config |  | R | 0x6 | 110.33 |
| PCI config |  | R | 0x61 | 110.33 |
| PCI config |  | R | 0x69 | 110.33 |
| PCI config |  | R | 0x79 | 110.33 |
| (rest) | | | | 824.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 72736.3, tokens 20.0

- GSP RPCs 85.0 per rep (655 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 19.7 per rep: 20046 leaves mapped and 168 unmapped in 6865.0 RM map call(s), walk 15804 us, worker apply 342631 us per rep
- events naming a host RM call 767.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x521.3; doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x147.3; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X [host] x16.0
- device events per rep: doorbell eventfd rings (token pool) 127.3; guest MMU invalidates 97.7; host doorbells written (forwarded) 147.3; host notifier events (no MSI) 4.3; host notifier events -> MSI 521.3; other events 491.0

## op 4 cuCtxDestroy: 3 repetition(s), 21450.0 trapped accesses per repetition, window median 1566943 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 20360.0 | 957474 |
| mmu-invalidate | 280.0 | 15441 |
| gsp-falcon | 207.0 | 13488 |
| pcfg-mirror (intr re-arm) | 194.0 | 22501 |
| pci-config | 171.0 | 3515 |
| bar2-flush-read | 169.0 | 10516 |
| gsp-queue-doorbell | 69.0 | 27377 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 20360.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 194.00 |
| BAR2 | flush page | R |  | 169.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 112.00 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 69.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | R | 0x110804 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | R | 0x110808 NV_PGSP | 69.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| PCI config |  | R | 0x34 | 19.33 |
| (rest) | | | | 151.67 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 20360.0

- GSP RPCs 69.0 per rep (321 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 16.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 293 us, worker apply 580681 us per rep
- events naming a host RM call 315.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x288.0; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X: host TSG X (engineType X) freed [host] [synthesized x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X: host TSG X (engineType X) freed [host]; shadow: VAS x1.0; shadow: VAS of guest root X freed: N syncs (N skipped clean, N full walks, N table reads,  x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events -> MSI 288.0; other events 155.0

## op 10 cuMemAlloc_1M: 40 repetition(s), 5.5 trapped accesses per repetition, window median 422 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 211 |
| bar2-write | 2.0 | 186 |
| bar2-flush-read | 1.0 | 39 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 2.00 |
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 2.0

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 8 us, worker apply 91 us per rep
- events naming a host RM call 5.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.2; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.0
- device events per rep: guest MMU invalidates 0.5; host notifier events (no MSI) 0.0; host notifier events -> MSI 5.2; other events 0.5

## op 11 cuMemFree_1M: 40 repetition(s), 11.3 trapped accesses per repetition, window median 360 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 226 |
| pci-config | 3.8 | 70 |
| bar2-flush-read | 1.5 | 120 |
| bar2-write | 1.0 | 154 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR2 | other pages | W |  | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| PCI config |  | R | 0x60 | 0.45 |
| PCI config |  | R | 0x68 | 0.45 |
| PCI config |  | R | 0x78 | 0.45 |
| PCI config |  | R | 0x6 | 0.42 |
| PCI config |  | R | 0x34 | 0.42 |
| PCI config |  | R | 0x61 | 0.42 |
| (rest) | | | | 1.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 1.0

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 8 us, worker apply 155 us per rep
- events naming a host RM call 1.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x1.6
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 1.6; other events 0.5

## op 12 cuMemAlloc_64M: 20 repetition(s), 187.8 trapped accesses per repetition, window median 4878 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 179.6 | 7527 |
| mmu-invalidate | 5.5 | 277 |
| bar2-flush-read | 2.3 | 183 |
| pcfg-mirror (intr re-arm) | 0.5 | 21 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 179.60 |
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.45 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 179.6

- shadow plans 0.8 per rep: 24 leaves mapped and 0 unmapped in 24.0 RM map call(s), walk 34 us, worker apply 1461 us per rep
- events naming a host RM call 2.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x2.3
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 2.3; other events 0.8

## op 13 cuMemFree_64M: 20 repetition(s), 77.5 trapped accesses per repetition, window median 3202 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 64.1 | 3074 |
| mmu-invalidate | 10.0 | 482 |
| bar2-flush-read | 3.1 | 172 |
| pcfg-mirror (intr re-arm) | 0.2 | 99 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 64.10 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.25 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 64.1

- shadow plans 1.1 per rep: 8 leaves mapped and 29 unmapped in 8.0 RM map call(s), walk 44 us, worker apply 12416 us per rep
- events naming a host RM call 1.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x1.9
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 1.9; other events 1.1

## op 14 cuMemHostAlloc_1M: 20 repetition(s), 806.2 trapped accesses per repetition, window median 24807 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 771.0 | 32903 |
| pci-config | 25.6 | 434 |
| mmu-invalidate | 5.0 | 456 |
| bar2-flush-read | 2.5 | 122 |
| pcfg-mirror (intr re-arm) | 2.0 | 222 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 771.00 |
| PCI config |  | R | 0x34 | 2.90 |
| PCI config |  | R | 0x60 | 2.90 |
| PCI config |  | R | 0x68 | 2.90 |
| PCI config |  | R | 0x78 | 2.90 |
| PCI config |  | R | 0x6 | 2.85 |
| PCI config |  | R | 0x61 | 2.85 |
| PCI config |  | R | 0x69 | 2.85 |
| PCI config |  | R | 0x79 | 2.85 |
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 2.05 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| (rest) | | | | 5.65 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 771.0

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 48.5 RM map call(s), walk 32 us, worker apply 1723 us per rep
- events naming a host RM call 3.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x3.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 3.5; other events 0.5

## op 15 cuMemFreeHost_1M: 20 repetition(s), 264.6 trapped accesses per repetition, window median 8192 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 257.0 | 9541 |
| mmu-invalidate | 5.0 | 259 |
| bar2-flush-read | 2.0 | 217 |
| pcfg-mirror (intr re-arm) | 0.7 | 150 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 257.00 |
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.65 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 257.0

- shadow plans 0.5 per rep: 51 leaves mapped and 205 unmapped in 0.4 RM map call(s), walk 24 us, worker apply 606 us per rep
- events naming a host RM call 2.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x2.4
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 2.4; other events 0.5

## op 16 cuMemHostRegister_1M: 20 repetition(s), 1047.0 trapped accesses per repetition, window median 36977 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 1030.0 | 36821 |
| mmu-invalidate | 10.0 | 596 |
| bar2-flush-read | 5.0 | 234 |
| pcfg-mirror (intr re-arm) | 2.0 | 193 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 1030.00 |
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 1030.0

- shadow plans 1.0 per rep: 230 leaves mapped and 51 unmapped in 33.0 RM map call(s), walk 68 us, worker apply 1838 us per rep
- events naming a host RM call 3.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x3.8
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 3.8; other events 1.0

## op 17 cuMemHostUnregister_1M: 20 repetition(s), 272.9 trapped accesses per repetition, window median 11976 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 258.0 | 12725 |
| mmu-invalidate | 10.0 | 441 |
| bar2-flush-read | 4.0 | 422 |
| pcfg-mirror (intr re-arm) | 0.8 | 32 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 258.00 |
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.85 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 258.0

- shadow plans 1.0 per rep: 26 leaves mapped and 230 unmapped in 0.5 RM map call(s), walk 20 us, worker apply 670 us per rep
- events naming a host RM call 2.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x2.6
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 2.6; other events 1.0

## op 20 launch_1_sync: 40 repetition(s), 4.7 trapped accesses per repetition, window median 30498 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 3.6 | 66 |
| pcfg-mirror (intr re-arm) | 1.1 | 56 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 1.12 |
| PCI config |  | R | 0x6 | 0.40 |
| PCI config |  | R | 0x34 | 0.40 |
| PCI config |  | R | 0x60 | 0.40 |
| PCI config |  | R | 0x61 | 0.40 |
| PCI config |  | R | 0x68 | 0.40 |
| PCI config |  | R | 0x69 | 0.40 |
| PCI config |  | R | 0x78 | 0.40 |
| PCI config |  | R | 0x79 | 0.40 |
| PCI config |  | R | 0xc0 | 0.03 |
| PCI config |  | R | 0xc4 | 0.03 |
| PCI config |  | R | 0xc8 | 0.03 |
| (rest) | | | | 0.33 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 21 launch_1000_sync: 2 repetition(s), 1907.5 trapped accesses per repetition, window median 6721118 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 1539.0 | 31614 |
| pcfg-mirror (intr re-arm) | 368.5 | 6050 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 368.50 |
| PCI config |  | R | 0x34 | 174.00 |
| PCI config |  | R | 0x60 | 174.00 |
| PCI config |  | R | 0x68 | 174.00 |
| PCI config |  | R | 0x78 | 174.00 |
| PCI config |  | R | 0x6 | 171.00 |
| PCI config |  | R | 0x61 | 171.00 |
| PCI config |  | R | 0x69 | 171.00 |
| PCI config |  | R | 0x79 | 171.00 |
| PCI config |  | R | 0x0 | 3.00 |
| PCI config |  | R | 0x4 | 3.00 |
| PCI config |  | R | 0x8 | 3.00 |
| (rest) | | | | 150.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1000.0

## op 30 h2d_pinned_1M: 20 repetition(s), 0.0 trapped accesses per repetition, window median 193 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 31 d2h_pinned_1M: 20 repetition(s), 0.0 trapped accesses per repetition, window median 205 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 32 d2d_1M: 20 repetition(s), 0.6 trapped accesses per repetition, window median 13938 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.6 | 5 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.55 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 33 h2d_pinned_16M: 20 repetition(s), 0.3 trapped accesses per repetition, window median 1400 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.3 | 239 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 34 d2h_pinned_16M: 20 repetition(s), 0.2 trapped accesses per repetition, window median 1408 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.2 | 291 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.25 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 35 d2d_16M: 20 repetition(s), 2.5 trapped accesses per repetition, window median 15542 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 1.8 | 38 |
| pcfg-mirror (intr re-arm) | 0.7 | 28 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.65 |
| PCI config |  | R | 0x34 | 0.25 |
| PCI config |  | R | 0x6 | 0.20 |
| PCI config |  | R | 0x60 | 0.20 |
| PCI config |  | R | 0x61 | 0.20 |
| PCI config |  | R | 0x68 | 0.20 |
| PCI config |  | R | 0x69 | 0.20 |
| PCI config |  | R | 0x78 | 0.20 |
| PCI config |  | R | 0x79 | 0.20 |
| PCI config |  | R | 0x28 | 0.05 |
| PCI config |  | R | 0x2c | 0.05 |
| PCI config |  | R | 0x38 | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 36 h2d_pageable_1M: 20 repetition(s), 0.1 trapped accesses per repetition, window median 242 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 21 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 37 d2h_pageable_1M: 20 repetition(s), 0.1 trapped accesses per repetition, window median 316 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.1 | 6 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 40 cuStreamSynchronize_idle: 40 repetition(s), 0.0 trapped accesses per repetition, window median 44 us


## op 41 cuEventRecord_Synchronize: 40 repetition(s), 0.6 trapped accesses per repetition, window median 14814 us

| class | per rep | us per rep |
|---|---|---|
| pcfg-mirror (intr re-arm) | 0.6 | 7 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.62 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 50 cuModuleLoad_cubin: 20 repetition(s), 9.3 trapped accesses per repetition, window median 11178 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 8.6 | 128 |
| pcfg-mirror (intr re-arm) | 0.8 | 542 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x34 | 1.00 |
| PCI config |  | R | 0x60 | 1.00 |
| PCI config |  | R | 0x6 | 0.95 |
| PCI config |  | R | 0x61 | 0.95 |
| PCI config |  | R | 0x68 | 0.95 |
| PCI config |  | R | 0x69 | 0.95 |
| PCI config |  | R | 0x78 | 0.95 |
| PCI config |  | R | 0x79 | 0.95 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.75 |
| PCI config |  | R | 0x0 | 0.05 |
| PCI config |  | R | 0x4 | 0.05 |
| PCI config |  | R | 0x8 | 0.05 |
| (rest) | | | | 0.70 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 51 cuModuleUnload: 20 repetition(s), 0.0 trapped accesses per repetition, window median 68 us



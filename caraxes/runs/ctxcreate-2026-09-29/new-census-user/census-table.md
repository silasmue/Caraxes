# census-table: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new-census-user/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 0.0 | 100178 | 100083.3 |
| 2 | cuInit | 3 | 185.7 | 13939 | 13866.4 |
| 3 | cuCtxCreate | 5 | 1394.0 | 296683 | 296617.8 |
| 4 | cuCtxDestroy | 5 | 739.8 | 132674 | 132633.1 |
| 10 | cuMemAlloc_1M | 20 | 11.8 | 466 | 407.5 |
| 11 | cuMemFree_1M | 20 | 6.5 | 668 | 534.8 |
| 12 | cuMemAlloc_64M | 10 | 210.4 | 4709 | 4626.9 |
| 13 | cuMemFree_64M | 10 | 28.2 | 3750 | 3688.1 |
| 14 | cuMemHostAlloc_1M | 10 | 14.7 | 2080 | 1992.0 |
| 15 | cuMemFreeHost_1M | 10 | 7.4 | 1252 | 1187.8 |
| 16 | cuMemHostRegister_1M | 10 | 19.1 | 1850 | 1807.9 |
| 17 | cuMemHostUnregister_1M | 10 | 14.5 | 1471 | 1420.1 |
| 20 | launch_1_sync | 20 | 0.0 | 240 | 190.9 |
| 21 | launch_1000_sync | 1 | 0.0 | 13054 | 10.6 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 160 | 116.2 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 204 | 151.8 |
| 32 | d2d_1M | 10 | 0.0 | 284 | 231.0 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 1410 | 1356.9 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 1408 | 1344.3 |
| 35 | d2d_16M | 10 | 0.9 | 408 | 334.8 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 310 | 262.3 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 364 | 309.8 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 78 | 15.7 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 278 | 206.9 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 382 | 308.1 |
| 51 | cuModuleUnload | 10 | 0.0 | 88 | 33.0 |

## op 1 idle_100ms: 10 repetition(s), 0.0 trapped accesses per repetition, window median 100178 us

- device events per rep: other events 20.7

## op 2 cuInit: 3 repetition(s), 185.7 trapped accesses per repetition, window median 13939 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 90.0 | 4324 |
| mmu-invalidate | 40.0 | 1477 |
| gsp-queue-doorbell | 30.7 | 6695 |
| bar2-flush-read | 24.0 | 851 |
| pcfg-mirror (intr re-arm) | 1.0 | 34 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 30.00 |
| BAR0 | 0x110000 | R | 0x110c80 NV_PGSP | 30.00 |
| BAR0 | 0x110000 | W | 0x110c84 NV_PGSP | 30.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 30.00 |
| BAR2 | flush page | R |  | 24.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 16.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 8.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 8.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 8.00 |
| BAR0 | 0x088000 | R | 0x088084 NV_PCFG | 1.00 |
| BAR0 | 0x110000 | R | 0x110c04 NV_PGSP_QUEUE_HEAD/TAIL | 0.67 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 30.0 per rep (113 us device time, 15.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20802a0c x3.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0
- events naming a host RM call 159.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x144.0; ctrl X: ce lce shim info: ceEngineType X (LCE N) -> host status X, shimInstance N shimLoca x3.0; ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0
- device events per rep: guest MMU invalidates 8.0; host notifier events (no MSI) 0.3; host notifier events -> MSI 144.0; other events 64.0

## op 3 cuCtxCreate: 5 repetition(s), 1394.0 trapped accesses per repetition, window median 296683 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 591.2 | 69577 |
| bar2-flush-read | 271.0 | 25262 |
| gsp-falcon | 255.0 | 10847 |
| pci-config | 102.6 | 2133 |
| gsp-queue-doorbell | 86.2 | 13650 |
| usermode page, other | 60.0 | 2597 |
| usermode-doorbell | 20.0 | 12520 |
| bar0-other | 8.0 | 1279 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 300.20 |
| BAR2 | flush page | R |  | 271.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 97.00 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 85.00 |
| BAR0 | 0x110000 | R | 0x110c80 NV_PGSP | 85.00 |
| BAR0 | 0x110000 | W | 0x110c84 NV_PGSP | 85.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 85.00 |
| BAR0 | 0xbb0000 | R | 0xbb0084 NV_VIRTUAL_FUNCTION_TIME_0/1 | 40.00 |
| BAR0 | 0xbb0000 | R | 0xbb0080 NV_VIRTUAL_FUNCTION_TIME_0/1 | 20.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 20.00 |
| PCI config |  | R | 0x34 | 11.60 |
| PCI config |  | R | 0x60 | 11.60 |
| PCI config |  | R | 0x68 | 11.60 |
| PCI config |  | R | 0x78 | 11.60 |
| PCI config |  | R | 0x6 | 11.40 |
| PCI config |  | R | 0x61 | 11.40 |
| PCI config |  | R | 0x69 | 11.40 |
| PCI config |  | R | 0x79 | 11.40 |
| (rest) | | | | 19.80 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): tokens 20.0

- GSP RPCs 85.0 per rep (372 us device time, 2.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 22.0 per rep: 19878 leaves mapped and 0 unmapped in 95.8 RM map call(s), walk 1327 us, worker apply 88067 us per rep
- events naming a host RM call 367.6 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x180.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x58.8; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N built in N us, N us of it waiting for the shadow worker [host] x19.0
- device events per rep: BAR1 pool events 1.0; doorbell eventfd rings (token pool) 160.0; guest MMU invalidates 97.0; host doorbells written (forwarded) 180.0; host notifier events (no MSI) 12.4; host notifier events -> MSI 58.8; other events 445.2

## op 4 cuCtxDestroy: 5 repetition(s), 739.8 trapped accesses per repetition, window median 132674 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 293.6 | 22804 |
| gsp-falcon | 207.0 | 10713 |
| bar2-flush-read | 169.0 | 13597 |
| gsp-queue-doorbell | 70.2 | 41811 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 169.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 125.60 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 69.00 |
| BAR0 | 0x110000 | R | 0x110c80 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | W | 0x110c84 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 69.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0x110000 | R | 0x110c04 NV_PGSP_QUEUE_HEAD/TAIL | 1.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (220 us device time, 54.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 259 us, worker apply 9021 us per rep
- events naming a host RM call 90.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x34.0; free X class Some(N); mirror: chid N: engine X host object X freed [host] [synthesized] x28.0; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x19.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x2.4; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (grpID N): host TSG X (engineType X) freed [host]; s x2.0
- device events per rep: BAR1 pool events 1.0; guest MMU invalidates 56.0; host notifier events (no MSI) 2.4; host notifier events -> MSI 34.0; other events 182.8

## op 10 cuMemAlloc_1M: 20 repetition(s), 11.8 trapped accesses per repetition, window median 466 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 6.3 | 108 |
| mmu-invalidate | 4.5 | 530 |
| bar2-flush-read | 1.0 | 151 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 3.05 |
| BAR2 | flush page | R |  | 1.00 |
| PCI config |  | R | 0x6 | 0.70 |
| PCI config |  | R | 0x34 | 0.70 |
| PCI config |  | R | 0x60 | 0.70 |
| PCI config |  | R | 0x61 | 0.70 |
| PCI config |  | R | 0x68 | 0.70 |
| PCI config |  | R | 0x69 | 0.70 |
| PCI config |  | R | 0x78 | 0.70 |
| PCI config |  | R | 0x79 | 0.70 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| PCI config |  | R | 0xa8 | 0.05 |
| PCI config |  | R | 0xac | 0.05 |
| PCI config |  | R | 0xb0 | 0.05 |
| PCI config |  | R | 0xb4 | 0.05 |
| PCI config |  | R | 0xb8 | 0.05 |
| PCI config |  | R | 0xbc | 0.05 |
| PCI config |  | R | 0xc0 | 0.05 |
| (rest) | | | | 0.35 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 8 us, worker apply 353 us per rep
- events naming a host RM call 20.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x20.1
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 20.1; other events 0.7

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 668 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 498 |
| bar2-flush-read | 1.5 | 234 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.05 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 10 us, worker apply 60 us per rep
- events naming a host RM call 6.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 6.5; other events 0.8

## op 12 cuMemAlloc_64M: 10 repetition(s), 210.4 trapped accesses per repetition, window median 4709 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 208.1 | 32113 |
| bar2-flush-read | 2.3 | 478 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 204.80 |
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 39 us, worker apply 32011 us per rep
- events naming a host RM call 15.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x15.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.2
- device events per rep: guest MMU invalidates 1.1; host notifier events (no MSI) 0.2; host notifier events -> MSI 15.0; other events 8.5

## op 13 cuMemFree_64M: 10 repetition(s), 28.2 trapped accesses per repetition, window median 3750 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 25.1 | 3406 |
| bar2-flush-read | 3.1 | 357 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 19.10 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 3 leaves mapped and 32 unmapped in 3.2 RM map call(s), walk 44 us, worker apply 2775 us per rep
- events naming a host RM call 7.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x7.3; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 7.3; other events 1.8

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 14.7 trapped accesses per repetition, window median 2080 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 9.5 | 2051 |
| pci-config | 2.7 | 122 |
| bar2-flush-read | 2.5 | 679 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 6.50 |
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| PCI config |  | R | 0x6 | 0.30 |
| PCI config |  | R | 0x34 | 0.30 |
| PCI config |  | R | 0x60 | 0.30 |
| PCI config |  | R | 0x61 | 0.30 |
| PCI config |  | R | 0x68 | 0.30 |
| PCI config |  | R | 0x69 | 0.30 |
| PCI config |  | R | 0x78 | 0.30 |
| PCI config |  | R | 0x79 | 0.30 |
| PCI config |  | R | 0x0 | 0.10 |
| PCI config |  | R | 0x4 | 0.10 |
| PCI config |  | R | 0x8 | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.4 per rep: 205 leaves mapped and 0 unmapped in 0.6 RM map call(s), walk 22 us, worker apply 932 us per rep
- events naming a host RM call 6.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.0
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 6.0; other events 0.9

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.4 trapped accesses per repetition, window median 1252 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.4 | 919 |
| bar2-flush-read | 2.0 | 221 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.40 |
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 16 us, worker apply 186 us per rep
- events naming a host RM call 7.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x7.2
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 7.2; other events 0.6

## op 16 cuMemHostRegister_1M: 10 repetition(s), 19.1 trapped accesses per repetition, window median 1850 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 14.1 | 1988 |
| bar2-flush-read | 5.0 | 401 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 8.10 |
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 1.1 RM map call(s), walk 30 us, worker apply 822 us per rep
- events naming a host RM call 6.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.8
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 6.8; other events 1.6

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.5 trapped accesses per repetition, window median 1471 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.5 | 1095 |
| bar2-flush-read | 4.0 | 306 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.50 |
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 16 us, worker apply 178 us per rep
- events naming a host RM call 6.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.4
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 6.4; other events 1.7

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 240 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.1

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 13054 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 4.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 160 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 204 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 284 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.1

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1410 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.2

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1408 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 35 d2d_16M: 10 repetition(s), 0.9 trapped accesses per repetition, window median 408 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 0.9 | 40 |

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
| PCI config |  | R | 0x8 | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.1

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 310 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.1

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 364 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.4

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 78 us

- device events per rep: other events 0.1

## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 278 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.2

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 382 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.2

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 88 us



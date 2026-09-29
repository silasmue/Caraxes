# census-table: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 51.3 | 100168 | 100085.7 |
| 2 | cuInit | 3 | 185.7 | 12795 | 12708.8 |
| 3 | cuCtxCreate | 5 | 1386.6 | 387572 | 387519.7 |
| 4 | cuCtxDestroy | 5 | 732.6 | 102995 | 102943.8 |
| 10 | cuMemAlloc_1M | 20 | 5.1 | 601 | 277.0 |
| 11 | cuMemFree_1M | 20 | 6.6 | 506 | 460.6 |
| 12 | cuMemAlloc_64M | 10 | 18.8 | 2512 | 2467.4 |
| 13 | cuMemFree_64M | 10 | 22.1 | 3318 | 3257.1 |
| 14 | cuMemHostAlloc_1M | 10 | 16.4 | 1606 | 1562.7 |
| 15 | cuMemFreeHost_1M | 10 | 7.3 | 1082 | 1022.3 |
| 16 | cuMemHostRegister_1M | 10 | 18.2 | 2265 | 2190.6 |
| 17 | cuMemHostUnregister_1M | 10 | 15.5 | 1687 | 1631.6 |
| 20 | launch_1_sync | 20 | 0.0 | 82 | 36.8 |
| 21 | launch_1000_sync | 1 | 0.0 | 11125 | 10.7 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 950 | 898.7 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 3728 | 3662.3 |
| 32 | d2d_1M | 10 | 0.0 | 338 | 236.7 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 14888 | 14783.1 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 55668 | 55555.8 |
| 35 | d2d_16M | 10 | 0.0 | 241 | 176.7 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 1104 | 1052.7 |
| 37 | d2h_pageable_1M | 10 | 0.9 | 3230 | 3175.2 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 84 | 14.3 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 268 | 197.7 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 740 | 654.6 |
| 51 | cuModuleUnload | 10 | 0.0 | 102 | 34.2 |

## op 1 idle_100ms: 10 repetition(s), 51.3 trapped accesses per repetition, window median 100168 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 51.3 | 766 |

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
| PCI config |  | R | 0x28 | 0.10 |
| PCI config |  | R | 0x2c | 0.10 |
| PCI config |  | R | 0x38 | 0.10 |
| PCI config |  | R | 0x3c | 0.10 |
| PCI config |  | R | 0x40 | 0.10 |
| PCI config |  | R | 0x44 | 0.10 |
| PCI config |  | R | 0x48 | 0.10 |
| PCI config |  | R | 0x4c | 0.10 |
| (rest) | | | | 4.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- device events per rep: other events 1.1

## op 2 cuInit: 3 repetition(s), 185.7 trapped accesses per repetition, window median 12795 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 90.0 | 3716 |
| mmu-invalidate | 40.0 | 1433 |
| gsp-queue-doorbell | 30.7 | 6936 |
| bar2-flush-read | 24.0 | 908 |
| pcfg-mirror (intr re-arm) | 1.0 | 32 |

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

- GSP RPCs 30.0 per rep (94 us device time, 15.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20802a0c x3.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0
- events naming a host RM call 26.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x11.3; ctrl X: ce lce shim info: ceEngineType X (LCE N) -> host status X, shimInstance N shimLoca x3.0; ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0
- device events per rep: guest MMU invalidates 8.0; host notifier events -> MSI 11.3; other events 61.0

## op 3 cuCtxCreate: 5 repetition(s), 1386.6 trapped accesses per repetition, window median 387572 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 583.8 | 69391 |
| bar2-flush-read | 271.0 | 23625 |
| gsp-falcon | 255.0 | 10264 |
| pci-config | 102.6 | 1923 |
| gsp-queue-doorbell | 86.2 | 12726 |
| usermode page, other | 60.0 | 2551 |
| usermode-doorbell | 20.0 | 992 |
| bar0-other | 8.0 | 1030 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 292.80 |
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

- GSP RPCs 85.0 per rep (331 us device time, 2.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 19.0 per rep: 19878 leaves mapped and 0 unmapped in 95.8 RM map call(s), walk 1480 us, worker apply 83246 us per rep
- events naming a host RM call 139.2 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x52.0; shadow: chid N host object class X X [host] x16.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x13.2; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x12.6; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x8.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x8.0
- device events per rep: BAR1 pool events 1.0; doorbell eventfd rings (token pool) 62.0; guest MMU invalidates 97.0; host doorbells written (forwarded) 52.0; host notifier events (no MSI) 12.6; host notifier events -> MSI 13.2; other events 724.0

## op 4 cuCtxDestroy: 5 repetition(s), 732.6 trapped accesses per repetition, window median 102995 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 286.4 | 20960 |
| gsp-falcon | 207.0 | 7881 |
| bar2-flush-read | 169.0 | 13191 |
| gsp-queue-doorbell | 70.2 | 26807 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 169.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 118.40 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 69.00 |
| BAR0 | 0x110000 | R | 0x110c80 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | W | 0x110c84 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 69.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0x110000 | R | 0x110c04 NV_PGSP_QUEUE_HEAD/TAIL | 1.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (220 us device time, 27.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 14.0 per rep: 0 leaves mapped and 2949 unmapped in 0.0 RM map call(s), walk 105 us, worker apply 1610 us per rep
- events naming a host RM call 35.8 per rep: free X class Some(N); mirror: chid N: engine X host object X freed [host] [synthesized] x16.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x7.4; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x7.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: preemption: tsg grpID N (guest X/X, chid N, its last host ch x1.0; free X class Some(N); mirror: tsg X/X (grpID N): host TSG X (engineType X) freed [host]; s x1.0
- device events per rep: BAR1 pool events 1.0; guest MMU invalidates 56.0; host notifier events (no MSI) 0.4; host notifier events -> MSI 7.4; other events 162.0

## op 10 cuMemAlloc_1M: 20 repetition(s), 5.1 trapped accesses per repetition, window median 601 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 4.1 | 521 |
| bar2-flush-read | 1.0 | 120 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.60 |
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 8 us, worker apply 263 us per rep
- events naming a host RM call 0.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.8
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 0.8; other events 0.5

## op 11 cuMemFree_1M: 20 repetition(s), 6.6 trapped accesses per repetition, window median 506 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.1 | 342 |
| bar2-flush-read | 1.5 | 158 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.10 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 8 us, worker apply 51 us per rep
- events naming a host RM call 0.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.4
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.4; other events 0.7

## op 12 cuMemAlloc_64M: 10 repetition(s), 18.8 trapped accesses per repetition, window median 2512 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 16.5 | 2213 |
| bar2-flush-read | 2.3 | 273 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 13.20 |
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 33 us, worker apply 1464 us per rep
- events naming a host RM call 0.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.4
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 0.4; other events 0.9

## op 13 cuMemFree_64M: 10 repetition(s), 22.1 trapped accesses per repetition, window median 3318 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 19.0 | 2539 |
| bar2-flush-read | 3.1 | 380 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 13.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 3 leaves mapped and 32 unmapped in 3.2 RM map call(s), walk 44 us, worker apply 1755 us per rep
- events naming a host RM call 0.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.6
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 0.6; other events 1.3

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 16.4 trapped accesses per repetition, window median 1606 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 13.9 | 2079 |
| bar2-flush-read | 2.5 | 676 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10.90 |
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.4 per rep: 205 leaves mapped and 0 unmapped in 0.8 RM map call(s), walk 17 us, worker apply 743 us per rep
- device events per rep: guest MMU invalidates 1.0; other events 0.4

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.3 trapped accesses per repetition, window median 1082 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.3 | 874 |
| bar2-flush-read | 2.0 | 170 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.30 |
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 16 us, worker apply 756 us per rep
- events naming a host RM call 0.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.4
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.4; other events 0.6

## op 16 cuMemHostRegister_1M: 10 repetition(s), 18.2 trapped accesses per repetition, window median 2265 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 13.2 | 1687 |
| bar2-flush-read | 5.0 | 583 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 7.20 |
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 1.6 RM map call(s), walk 36 us, worker apply 513 us per rep
- device events per rep: guest MMU invalidates 2.0; other events 0.9

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 15.5 trapped accesses per repetition, window median 1687 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 11.5 | 1316 |
| bar2-flush-read | 4.0 | 365 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 5.50 |
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 23 us, worker apply 400 us per rep
- events naming a host RM call 0.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.6
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 0.6; other events 1.1

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 82 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 11125 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 950 us

- device events per rep: doorbell eventfd rings (token pool) 1.0; other events 5.0

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 3728 us

- device events per rep: doorbell eventfd rings (token pool) 1.0; other events 5.2

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 338 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.2

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 14888 us

- device events per rep: doorbell eventfd rings (token pool) 1.0; other events 5.0

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 55668 us

- device events per rep: doorbell eventfd rings (token pool) 1.0; other events 10.6

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 241 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1104 us

- device events per rep: doorbell eventfd rings (token pool) 1.0; other events 5.0

## op 37 d2h_pageable_1M: 10 repetition(s), 0.9 trapped accesses per repetition, window median 3230 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 0.9 | 13 |

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
| PCI config |  | R | 0x0 | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- device events per rep: doorbell eventfd rings (token pool) 1.0; other events 5.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 84 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 268 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.1

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 740 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.5

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 102 us



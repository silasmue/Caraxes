# census-table: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new2-census-user/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 51.3 | 100162 | 100079.5 |
| 2 | cuInit | 3 | 185.7 | 11850 | 11783.7 |
| 3 | cuCtxCreate | 5 | 1318.8 | 262747 | 262707.7 |
| 4 | cuCtxDestroy | 5 | 741.2 | 120674 | 120612.2 |
| 10 | cuMemAlloc_1M | 20 | 3.8 | 358 | 300.9 |
| 11 | cuMemFree_1M | 20 | 6.5 | 400 | 355.1 |
| 12 | cuMemAlloc_64M | 10 | 24.2 | 2534 | 2489.3 |
| 13 | cuMemFree_64M | 10 | 24.2 | 2278 | 2240.6 |
| 14 | cuMemHostAlloc_1M | 10 | 10.7 | 1655 | 1599.0 |
| 15 | cuMemFreeHost_1M | 10 | 7.9 | 878 | 834.9 |
| 16 | cuMemHostRegister_1M | 10 | 17.8 | 1407 | 1370.5 |
| 17 | cuMemHostUnregister_1M | 10 | 15.4 | 1196 | 1155.8 |
| 20 | launch_1_sync | 20 | 0.0 | 134 | 59.2 |
| 21 | launch_1000_sync | 1 | 0.0 | 13451 | 11.5 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 152 | 115.7 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 147 | 109.7 |
| 32 | d2d_1M | 10 | 0.0 | 78 | 33.3 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 1399 | 1342.0 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 1370 | 1315.3 |
| 35 | d2d_16M | 10 | 0.0 | 158 | 116.5 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 390 | 319.7 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 238 | 201.4 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 41 | 3.7 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 66 | 27.0 |
| 50 | cuModuleLoad_cubin | 10 | 0.9 | 174 | 119.9 |
| 51 | cuModuleUnload | 10 | 0.0 | 68 | 20.3 |

## op 1 idle_100ms: 10 repetition(s), 51.3 trapped accesses per repetition, window median 100162 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 51.3 | 808 |

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

- device events per rep: other events 0.4

## op 2 cuInit: 3 repetition(s), 185.7 trapped accesses per repetition, window median 11850 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 90.0 | 2964 |
| mmu-invalidate | 40.0 | 1378 |
| gsp-queue-doorbell | 30.7 | 6501 |
| bar2-flush-read | 24.0 | 795 |
| pcfg-mirror (intr re-arm) | 1.0 | 27 |

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

- GSP RPCs 30.0 per rep (99 us device time, 15.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20802a0c x3.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0
- events naming a host RM call 18.0 per rep: ctrl X: ce lce shim info: ceEngineType X (LCE N) -> host status X, shimInstance N shimLoca x3.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x3.0; ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0
- device events per rep: guest MMU invalidates 8.0; host notifier events -> MSI 3.0; other events 61.0

## op 3 cuCtxCreate: 5 repetition(s), 1318.8 trapped accesses per repetition, window median 262747 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 618.6 | 67502 |
| bar2-flush-read | 271.0 | 22794 |
| gsp-falcon | 255.0 | 10184 |
| gsp-queue-doorbell | 86.2 | 13475 |
| usermode page, other | 60.0 | 2880 |
| usermode-doorbell | 20.0 | 11554 |
| bar0-other | 8.0 | 925 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 327.60 |
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
| BAR0 | 0x070000 | W | 0x070010 NV_UFLUSH_L2_FLUSH_DIRTY | 2.00 |
| BAR0 | 0x070000 | R | 0x070010 NV_UFLUSH_L2_FLUSH_DIRTY | 2.00 |
| BAR0 | 0xb80000 | W | 0xb80f00 NV_VIRTUAL_FUNCTION_PRIV_L2_SYSMEM_INVALIDATE | 2.00 |
| BAR0 | 0xb80000 | R | 0xb80f00 NV_VIRTUAL_FUNCTION_PRIV_L2_SYSMEM_INVALIDATE | 2.00 |
| BAR0 | 0x110000 | R | 0x110c04 NV_PGSP_QUEUE_HEAD/TAIL | 1.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): tokens 20.0

- GSP RPCs 85.0 per rep (333 us device time, 2.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 22.4 per rep: 19878 leaves mapped and 0 unmapped in 95.2 RM map call(s), walk 1321 us, worker apply 86199 us per rep
- events naming a host RM call 321.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x202.0; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N built in N us, N us of it waiting for the shadow worker [host] x19.0; shadow: chid N joins host TSG X of guest TSG X (grpID N) [host] x16.0
- device events per rep: BAR1 pool events 1.0; doorbell eventfd rings (token pool) 182.0; guest MMU invalidates 97.0; host doorbells written (forwarded) 202.0; host notifier events -> MSI 2.8; other events 394.4

## op 4 cuCtxDestroy: 5 repetition(s), 741.2 trapped accesses per repetition, window median 120674 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 295.0 | 23328 |
| gsp-falcon | 207.0 | 9158 |
| bar2-flush-read | 169.0 | 13299 |
| gsp-queue-doorbell | 70.2 | 41280 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 169.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 127.00 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 69.00 |
| BAR0 | 0x110000 | R | 0x110c80 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | W | 0x110c84 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 69.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0x110000 | R | 0x110c04 NV_PGSP_QUEUE_HEAD/TAIL | 1.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (224 us device time, 54.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 282 us, worker apply 7012 us per rep
- events naming a host RM call 64.4 per rep: free X class Some(N); mirror: chid N: engine X host object X freed [host] [synthesized] x28.0; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x19.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x9.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (grpID N): host TSG X (engineType X) freed [host]; s x2.0; free X class Some(N); mirror: tsg X/X (grpID N): host TSG X (engineType X) freed [host] [s x2.0
- device events per rep: BAR1 pool events 1.0; guest MMU invalidates 56.0; host notifier events (no MSI) 0.6; host notifier events -> MSI 9.0; other events 164.8

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.8 trapped accesses per repetition, window median 358 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.8 | 394 |
| bar2-flush-read | 1.0 | 175 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.30 |
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 7 us, worker apply 320 us per rep
- events naming a host RM call 0.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.3
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 0.3; other events 0.5

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 400 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 300 |
| bar2-flush-read | 1.5 | 118 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 51 us, worker apply 53 us per rep
- events naming a host RM call 0.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.2; other events 0.6

## op 12 cuMemAlloc_64M: 10 repetition(s), 24.2 trapped accesses per repetition, window median 2534 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 21.9 | 3095 |
| bar2-flush-read | 2.3 | 232 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 18.60 |
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 31 us, worker apply 2358 us per rep
- events naming a host RM call 0.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 0.2; other events 0.9

## op 13 cuMemFree_64M: 10 repetition(s), 24.2 trapped accesses per repetition, window median 2278 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 21.1 | 2354 |
| bar2-flush-read | 3.1 | 236 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 15.10 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 3 leaves mapped and 32 unmapped in 3.2 RM map call(s), walk 31 us, worker apply 1929 us per rep
- device events per rep: guest MMU invalidates 2.0; other events 1.1

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 10.7 trapped accesses per repetition, window median 1655 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 8.2 | 1178 |
| bar2-flush-read | 2.5 | 557 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 5.20 |
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.4 per rep: 205 leaves mapped and 0 unmapped in 1.2 RM map call(s), walk 17 us, worker apply 494 us per rep
- events naming a host RM call 0.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.2; other events 0.4

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.9 trapped accesses per repetition, window median 878 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.9 | 687 |
| bar2-flush-read | 2.0 | 119 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.90 |
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.5 RM map call(s), walk 9 us, worker apply 206 us per rep
- device events per rep: guest MMU invalidates 1.0; other events 0.6

## op 16 cuMemHostRegister_1M: 10 repetition(s), 17.8 trapped accesses per repetition, window median 1407 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 12.8 | 1194 |
| bar2-flush-read | 5.0 | 311 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 6.80 |
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 2.2 RM map call(s), walk 26 us, worker apply 420 us per rep
- events naming a host RM call 0.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 0.2; other events 0.9

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 15.4 trapped accesses per repetition, window median 1196 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 11.4 | 1011 |
| bar2-flush-read | 4.0 | 237 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 5.40 |
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 14 us, worker apply 275 us per rep
- device events per rep: guest MMU invalidates 2.0; other events 1.1

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 134 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 13451 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 4.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 152 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 147 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 78 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1399 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1370 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 158 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 390 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.1

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 238 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 41 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 66 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.9 trapped accesses per repetition, window median 174 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 0.9 | 20 |

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

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 68 us



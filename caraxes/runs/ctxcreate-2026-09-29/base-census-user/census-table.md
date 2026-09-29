# census-table: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census-user/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 51.3 | 100147 | 100078.6 |
| 2 | cuInit | 3 | 185.7 | 12343 | 12281.2 |
| 3 | cuCtxCreate | 5 | 1277.4 | 303215 | 303144.3 |
| 4 | cuCtxDestroy | 5 | 878.6 | 135555 | 135481.0 |
| 10 | cuMemAlloc_1M | 20 | 3.9 | 394 | 326.6 |
| 11 | cuMemFree_1M | 20 | 7.1 | 540 | 466.7 |
| 12 | cuMemAlloc_64M | 10 | 29.4 | 3926 | 3845.2 |
| 13 | cuMemFree_64M | 10 | 32.2 | 3566 | 3506.0 |
| 14 | cuMemHostAlloc_1M | 10 | 11.5 | 1679 | 1602.7 |
| 15 | cuMemFreeHost_1M | 10 | 8.0 | 1189 | 1088.2 |
| 16 | cuMemHostRegister_1M | 10 | 18.0 | 3759 | 3689.2 |
| 17 | cuMemHostUnregister_1M | 10 | 15.1 | 1252 | 1212.9 |
| 20 | launch_1_sync | 20 | 0.0 | 145 | 78.0 |
| 21 | launch_1000_sync | 1 | 0.0 | 11685 | 10.5 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 216 | 153.5 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 317 | 261.3 |
| 32 | d2d_1M | 10 | 0.0 | 79 | 35.1 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 1375 | 1322.5 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 1338 | 1301.8 |
| 35 | d2d_16M | 10 | 0.0 | 238 | 167.7 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 276 | 207.5 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 376 | 311.0 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 76 | 12.6 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 114 | 44.0 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 242 | 164.0 |
| 51 | cuModuleUnload | 10 | 0.0 | 94 | 29.2 |

## op 1 idle_100ms: 10 repetition(s), 51.3 trapped accesses per repetition, window median 100147 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 51.3 | 771 |

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

- device events per rep: other events 0.8

## op 2 cuInit: 3 repetition(s), 185.7 trapped accesses per repetition, window median 12343 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 90.0 | 3469 |
| mmu-invalidate | 40.0 | 1507 |
| gsp-queue-doorbell | 30.7 | 6697 |
| bar2-flush-read | 24.0 | 876 |
| pcfg-mirror (intr re-arm) | 1.0 | 31 |

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

- GSP RPCs 30.0 per rep (95 us device time, 15.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20802a0c x3.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0
- events naming a host RM call 49.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x33.7; ctrl X: ce lce shim info: ceEngineType X (LCE N) -> host status X, shimInstance N shimLoca x3.0; ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0
- device events per rep: guest MMU invalidates 8.0; host notifier events (no MSI) 1.0; host notifier events -> MSI 33.7; other events 61.0

## op 3 cuCtxCreate: 5 repetition(s), 1277.4 trapped accesses per repetition, window median 303215 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 577.2 | 62865 |
| bar2-flush-read | 271.0 | 23892 |
| gsp-falcon | 255.0 | 9988 |
| gsp-queue-doorbell | 86.2 | 12905 |
| usermode page, other | 60.0 | 2831 |
| usermode-doorbell | 20.0 | 12287 |
| bar0-other | 8.0 | 1193 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 286.20 |
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

- GSP RPCs 85.0 per rep (309 us device time, 2.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 19.0 per rep: 19878 leaves mapped and 0 unmapped in 101.6 RM map call(s), walk 1889 us, worker apply 97267 us per rep
- events naming a host RM call 307.8 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x207.4; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N joins host TSG X of guest TSG X (grpID N) [host] x16.0; shadow: host TSG X for guest TSG X (grpID N) on VAS X [host] x4.0
- device events per rep: BAR1 pool events 1.0; doorbell eventfd rings (token pool) 187.4; guest MMU invalidates 97.0; host doorbells written (forwarded) 207.4; host notifier events -> MSI 3.8; other events 372.8

## op 4 cuCtxDestroy: 5 repetition(s), 878.6 trapped accesses per repetition, window median 135555 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 329.8 | 30317 |
| gsp-falcon | 207.0 | 9906 |
| bar2-flush-read | 169.0 | 14637 |
| pci-config | 102.6 | 1985 |
| gsp-queue-doorbell | 70.2 | 38211 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 169.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 161.80 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 69.00 |
| BAR0 | 0x110000 | R | 0x110c80 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | W | 0x110c84 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 69.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| PCI config |  | R | 0x34 | 11.60 |
| PCI config |  | R | 0x60 | 11.60 |
| PCI config |  | R | 0x68 | 11.60 |
| PCI config |  | R | 0x78 | 11.60 |
| PCI config |  | R | 0x6 | 11.40 |
| PCI config |  | R | 0x61 | 11.40 |
| PCI config |  | R | 0x69 | 11.40 |
| PCI config |  | R | 0x79 | 11.40 |
| BAR0 | 0x110000 | R | 0x110c04 NV_PGSP_QUEUE_HEAD/TAIL | 1.20 |
| PCI config |  | R | 0x0 | 0.20 |
| PCI config |  | R | 0x4 | 0.20 |
| (rest) | | | | 10.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (260 us device time, 54.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 1128 us, worker apply 13844 us per rep
- events naming a host RM call 62.6 per rep: free X class Some(N); mirror: chid N: engine X host object X freed [host] [synthesized] x28.0; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x19.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x8.4; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (grpID N): host TSG X (engineType X) freed [host]; s x2.0; free X class Some(N); mirror: tsg X/X (grpID N): host TSG X (engineType X) freed [host] [s x2.0
- device events per rep: BAR1 pool events 1.0; guest MMU invalidates 56.0; host notifier events -> MSI 8.4; other events 162.2

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.9 trapped accesses per repetition, window median 394 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.9 | 293 |
| bar2-flush-read | 1.0 | 212 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.35 |
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 8 us, worker apply 73 us per rep
- events naming a host RM call 0.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.8
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 0.8; other events 0.6

## op 11 cuMemFree_1M: 20 repetition(s), 7.1 trapped accesses per repetition, window median 540 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.6 | 489 |
| bar2-flush-read | 1.5 | 177 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.60 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 8 us, worker apply 153 us per rep
- events naming a host RM call 0.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.9
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.9; other events 0.6

## op 12 cuMemAlloc_64M: 10 repetition(s), 29.4 trapped accesses per repetition, window median 3926 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 27.1 | 4334 |
| bar2-flush-read | 2.3 | 433 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 23.80 |
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 38 us, worker apply 3666 us per rep
- events naming a host RM call 3.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x3.6
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 3.6; other events 1.2

## op 13 cuMemFree_64M: 10 repetition(s), 32.2 trapped accesses per repetition, window median 3566 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 29.1 | 4018 |
| bar2-flush-read | 3.1 | 345 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 23.10 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 3 leaves mapped and 32 unmapped in 3.2 RM map call(s), walk 38 us, worker apply 3270 us per rep
- events naming a host RM call 4.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.6
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.6; other events 1.5

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 11.5 trapped accesses per repetition, window median 1679 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 9.0 | 1341 |
| bar2-flush-read | 2.5 | 442 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 6.00 |
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.4 per rep: 205 leaves mapped and 0 unmapped in 1.0 RM map call(s), walk 24 us, worker apply 554 us per rep
- events naming a host RM call 0.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.4
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.4; other events 0.6

## op 15 cuMemFreeHost_1M: 10 repetition(s), 8.0 trapped accesses per repetition, window median 1189 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 6.0 | 838 |
| bar2-flush-read | 2.0 | 211 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 3.00 |
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.2 RM map call(s), walk 16 us, worker apply 234 us per rep
- events naming a host RM call 0.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.2; other events 0.6

## op 16 cuMemHostRegister_1M: 10 repetition(s), 18.0 trapped accesses per repetition, window median 3759 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 13.0 | 1889 |
| bar2-flush-read | 5.0 | 1019 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 7.00 |
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 1.5 RM map call(s), walk 32 us, worker apply 612 us per rep
- events naming a host RM call 4.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.6
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.6; other events 1.5

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 15.1 trapped accesses per repetition, window median 1252 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 11.1 | 914 |
| bar2-flush-read | 4.0 | 259 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 5.10 |
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 15 us, worker apply 235 us per rep
- events naming a host RM call 0.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.3
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 0.3; other events 1.1

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 145 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 11685 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 216 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 317 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 79 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1375 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1338 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 238 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 276 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 376 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 76 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 114 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 242 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 94 us



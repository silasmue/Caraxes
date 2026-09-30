# census-table: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new-census/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 0.0 | 100166 | 100074.5 |
| 2 | cuInit | 3 | 185.7 | 12709 | 12654.6 |
| 3 | cuCtxCreate | 5 | 1389.2 | 270901 | 270856.4 |
| 4 | cuCtxDestroy | 5 | 738.2 | 136759 | 136711.1 |
| 10 | cuMemAlloc_1M | 20 | 3.7 | 428 | 279.7 |
| 11 | cuMemFree_1M | 20 | 7.3 | 350 | 305.7 |
| 12 | cuMemAlloc_64M | 10 | 19.7 | 2826 | 2763.3 |
| 13 | cuMemFree_64M | 10 | 24.7 | 2642 | 2595.1 |
| 14 | cuMemHostAlloc_1M | 10 | 10.1 | 1596 | 1540.0 |
| 15 | cuMemFreeHost_1M | 10 | 7.7 | 1204 | 1132.4 |
| 16 | cuMemHostRegister_1M | 10 | 17.3 | 3164 | 3095.9 |
| 17 | cuMemHostUnregister_1M | 10 | 16.0 | 2154 | 2064.6 |
| 20 | launch_1_sync | 20 | 0.0 | 246 | 188.4 |
| 21 | launch_1000_sync | 1 | 0.0 | 13427 | 11.4 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 174 | 128.5 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 183 | 122.5 |
| 32 | d2d_1M | 10 | 0.0 | 226 | 179.3 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 1374 | 1315.5 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 1392 | 1339.2 |
| 35 | d2d_16M | 10 | 0.0 | 190 | 141.0 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 280 | 218.3 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 268 | 213.7 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 66 | 9.2 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 800 | 747.6 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 369 | 298.0 |
| 51 | cuModuleUnload | 10 | 0.0 | 65 | 21.2 |

## op 1 idle_100ms: 10 repetition(s), 0.0 trapped accesses per repetition, window median 100166 us

- device events per rep: other events 4.7

## op 2 cuInit: 3 repetition(s), 185.7 trapped accesses per repetition, window median 12709 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 90.0 | 3206 |
| mmu-invalidate | 40.0 | 1397 |
| gsp-queue-doorbell | 30.7 | 6288 |
| bar2-flush-read | 24.0 | 830 |
| pcfg-mirror (intr re-arm) | 1.0 | 33 |

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
- events naming a host RM call 19.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.0; ctrl X: ce lce shim info: ceEngineType X (LCE N) -> host status X, shimInstance N shimLoca x3.0; ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0
- device events per rep: guest MMU invalidates 8.0; host notifier events -> MSI 4.0; other events 61.0

## op 3 cuCtxCreate: 5 repetition(s), 1389.2 trapped accesses per repetition, window median 270901 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 586.4 | 70685 |
| bar2-flush-read | 271.0 | 24050 |
| gsp-falcon | 255.0 | 11209 |
| pci-config | 102.6 | 1945 |
| gsp-queue-doorbell | 86.2 | 13578 |
| usermode page, other | 60.0 | 2741 |
| usermode-doorbell | 20.0 | 11874 |
| bar0-other | 8.0 | 892 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 295.40 |
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

- GSP RPCs 85.0 per rep (379 us device time, 2.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 22.4 per rep: 19878 leaves mapped and 0 unmapped in 99.8 RM map call(s), walk 1338 us, worker apply 86178 us per rep
- events naming a host RM call 339.2 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x202.2; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0; shadow: chid N: the guest's privileged engine X channel (an application's VA space) runs u x20.0; shadow: chid N built in N us, N us of it waiting for the shadow worker [host] x19.0
- device events per rep: BAR1 pool events 1.0; doorbell eventfd rings (token pool) 182.2; guest MMU invalidates 97.0; host doorbells written (forwarded) 202.2; host notifier events -> MSI 0.4; other events 414.0

## op 4 cuCtxDestroy: 5 repetition(s), 738.2 trapped accesses per repetition, window median 136759 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 292.0 | 24177 |
| gsp-falcon | 207.0 | 10645 |
| bar2-flush-read | 169.0 | 13403 |
| gsp-queue-doorbell | 70.2 | 39330 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 169.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 124.00 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 69.00 |
| BAR0 | 0x110000 | R | 0x110c80 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | W | 0x110c84 NV_PGSP | 69.00 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 69.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 56.00 |
| BAR0 | 0x110000 | R | 0x110c04 NV_PGSP_QUEUE_HEAD/TAIL | 1.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (228 us device time, 54.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 262 us, worker apply 6871 us per rep
- events naming a host RM call 56.6 per rep: free X class Some(N); mirror: chid N: engine X host object X freed [host] [synthesized] x28.0; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x19.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x2.4; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (grpID N): host TSG X (engineType X) freed [host]; s x2.0; free X class Some(N); mirror: tsg X/X (grpID N): host TSG X (engineType X) freed [host] [s x2.0
- device events per rep: BAR1 pool events 1.0; guest MMU invalidates 56.0; host notifier events -> MSI 2.4; other events 157.2

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.7 trapped accesses per repetition, window median 428 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.7 | 326 |
| bar2-flush-read | 1.0 | 161 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.20 |
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 6 us, worker apply 67 us per rep
- events naming a host RM call 3.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x3.2
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 3.2; other events 0.5

## op 11 cuMemFree_1M: 20 repetition(s), 7.3 trapped accesses per repetition, window median 350 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.8 | 512 |
| bar2-flush-read | 1.5 | 118 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.80 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 6 us, worker apply 147 us per rep
- events naming a host RM call 0.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.8
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.8; other events 0.7

## op 12 cuMemAlloc_64M: 10 repetition(s), 19.7 trapped accesses per repetition, window median 2826 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 17.4 | 2461 |
| bar2-flush-read | 2.3 | 386 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 14.10 |
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 33 us, worker apply 1635 us per rep
- events naming a host RM call 2.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x2.4
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 2.4; other events 1.2

## op 13 cuMemFree_64M: 10 repetition(s), 24.7 trapped accesses per repetition, window median 2642 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 18.0 | 2158 |
| pci-config | 3.6 | 164 |
| bar2-flush-read | 3.1 | 303 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 12.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| PCI config |  | R | 0x6 | 0.40 |
| PCI config |  | R | 0x34 | 0.40 |
| PCI config |  | R | 0x60 | 0.40 |
| PCI config |  | R | 0x61 | 0.40 |
| PCI config |  | R | 0x68 | 0.40 |
| PCI config |  | R | 0x69 | 0.40 |
| PCI config |  | R | 0x78 | 0.40 |
| PCI config |  | R | 0x79 | 0.40 |
| PCI config |  | R | 0x0 | 0.10 |
| PCI config |  | R | 0x4 | 0.10 |
| PCI config |  | R | 0x8 | 0.10 |
| PCI config |  | R | 0xc | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 3 leaves mapped and 32 unmapped in 3.2 RM map call(s), walk 40 us, worker apply 1696 us per rep
- events naming a host RM call 2.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x2.2
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 2.2; other events 1.3

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 10.1 trapped accesses per repetition, window median 1596 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 7.6 | 1059 |
| bar2-flush-read | 2.5 | 403 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.60 |
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.4 per rep: 205 leaves mapped and 0 unmapped in 1.1 RM map call(s), walk 18 us, worker apply 358 us per rep
- events naming a host RM call 0.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.9
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.9; other events 0.4

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.7 trapped accesses per repetition, window median 1204 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.7 | 899 |
| bar2-flush-read | 2.0 | 215 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.70 |
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 16 us, worker apply 222 us per rep
- events naming a host RM call 1.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x1.2
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 1.2; other events 0.6

## op 16 cuMemHostRegister_1M: 10 repetition(s), 17.3 trapped accesses per repetition, window median 3164 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 12.3 | 2536 |
| bar2-flush-read | 5.0 | 606 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 6.30 |
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 1.1 RM map call(s), walk 35 us, worker apply 476 us per rep
- events naming a host RM call 1.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x1.5; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 1.5; other events 1.3

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 16.0 trapped accesses per repetition, window median 2154 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 12.0 | 1644 |
| bar2-flush-read | 4.0 | 660 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 6.00 |
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 23 us, worker apply 431 us per rep
- events naming a host RM call 1.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x1.0
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 1.0; other events 1.3

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 246 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 13427 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 2.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 174 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 183 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 226 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1374 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1392 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.1

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 190 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 280 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 268 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 66 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 800 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.1

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 369 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 0.1

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 65 us



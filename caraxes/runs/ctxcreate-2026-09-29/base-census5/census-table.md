# census-table: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census5/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 20.7 | 100178 | 100075.3 |
| 2 | cuInit | 3 | 185.7 | 12111 | 12031.7 |
| 3 | cuCtxCreate | 5 | 1390.4 | 373124 | 373047.5 |
| 4 | cuCtxDestroy | 5 | 732.6 | 96363 | 96324.7 |
| 10 | cuMemAlloc_1M | 20 | 3.7 | 434 | 361.4 |
| 11 | cuMemFree_1M | 20 | 6.5 | 420 | 355.1 |
| 12 | cuMemAlloc_64M | 10 | 34.5 | 2516 | 2474.8 |
| 13 | cuMemFree_64M | 10 | 24.5 | 2846 | 2795.8 |
| 14 | cuMemHostAlloc_1M | 10 | 8.4 | 1978 | 1930.7 |
| 15 | cuMemFreeHost_1M | 10 | 9.0 | 906 | 857.6 |
| 16 | cuMemHostRegister_1M | 10 | 23.9 | 2286 | 2250.2 |
| 17 | cuMemHostUnregister_1M | 10 | 14.8 | 1398 | 1332.3 |
| 20 | launch_1_sync | 20 | 0.0 | 75 | 38.7 |
| 21 | launch_1000_sync | 1 | 0.0 | 11823 | 11.1 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 974 | 926.1 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 2804 | 2753.2 |
| 32 | d2d_1M | 10 | 0.0 | 80 | 38.2 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 14173 | 14069.8 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 52430 | 52325.6 |
| 35 | d2d_16M | 10 | 0.0 | 189 | 136.2 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 1174 | 1107.9 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 3097 | 3034.4 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 40 | 2.4 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 66 | 28.6 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 188 | 127.4 |
| 51 | cuModuleUnload | 10 | 0.0 | 52 | 11.3 |

## op 1 idle_100ms: 10 repetition(s), 20.7 trapped accesses per repetition, window median 100178 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 20.7 | 331 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x34 | 2.40 |
| PCI config |  | R | 0x60 | 2.40 |
| PCI config |  | R | 0x68 | 2.40 |
| PCI config |  | R | 0x6 | 2.30 |
| PCI config |  | R | 0x61 | 2.30 |
| PCI config |  | R | 0x69 | 2.30 |
| PCI config |  | R | 0x78 | 2.30 |
| PCI config |  | R | 0x79 | 2.30 |
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
| (rest) | | | | 0.80 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- device events per rep: other events 3.5

## op 2 cuInit: 3 repetition(s), 185.7 trapped accesses per repetition, window median 12111 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 90.0 | 3088 |
| mmu-invalidate | 40.0 | 1431 |
| gsp-queue-doorbell | 30.7 | 6231 |
| bar2-flush-read | 24.0 | 806 |
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

- GSP RPCs 30.0 per rep (95 us device time, 15.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20802a0c x3.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0
- events naming a host RM call 35.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x20.7; ctrl X: ce lce shim info: ceEngineType X (LCE N) -> host status X, shimInstance N shimLoca x3.0; ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0
- device events per rep: guest MMU invalidates 8.0; host notifier events -> MSI 20.7; other events 61.7

## op 3 cuCtxCreate: 5 repetition(s), 1390.4 trapped accesses per repetition, window median 373124 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 587.6 | 67173 |
| bar2-flush-read | 271.0 | 21848 |
| gsp-falcon | 255.0 | 9816 |
| pci-config | 102.6 | 2871 |
| gsp-queue-doorbell | 86.2 | 12231 |
| usermode page, other | 60.0 | 2811 |
| usermode-doorbell | 20.0 | 804 |
| bar0-other | 8.0 | 1504 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 296.60 |
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

- GSP RPCs 85.0 per rep (602 us device time, 2.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 19.0 per rep: 19878 leaves mapped and 0 unmapped in 100.4 RM map call(s), walk 1754 us, worker apply 81641 us per rep
- events naming a host RM call 117.2 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x53.0; shadow: chid N host object class X X [host] x16.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x8.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x8.0; shadow: chid N: the guest's privileged engine X channel (an application's VA space) runs u x8.0; shadow: chid N joins host TSG X of guest TSG X (grpID N) [host] x7.0
- device events per rep: BAR1 pool events 1.0; doorbell eventfd rings (token pool) 62.8; guest MMU invalidates 97.0; host doorbells written (forwarded) 53.0; host notifier events (no MSI) 0.4; host notifier events -> MSI 0.8; other events 720.6

## op 4 cuCtxDestroy: 5 repetition(s), 732.6 trapped accesses per repetition, window median 96363 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 286.4 | 21230 |
| gsp-falcon | 207.0 | 7785 |
| bar2-flush-read | 169.0 | 12914 |
| gsp-queue-doorbell | 70.2 | 24600 |

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

- GSP RPCs 69.0 per rep (891 us device time, 27.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 14.0 per rep: 0 leaves mapped and 2949 unmapped in 0.0 RM map call(s), walk 104 us, worker apply 1804 us per rep
- events naming a host RM call 29.6 per rep: free X class Some(N); mirror: chid N: engine X host object X freed [host] [synthesized] x16.0; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x7.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x1.6; free X class Some(N); mirror: preemption: tsg grpID N (guest X/X, chid N, its last host ch x1.0; free X class Some(N); mirror: tsg X/X (grpID N): host TSG X (engineType X) freed [host]; s x1.0
- device events per rep: BAR1 pool events 1.0; guest MMU invalidates 56.0; host notifier events -> MSI 1.6; other events 156.4

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.7 trapped accesses per repetition, window median 434 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.7 | 281 |
| bar2-flush-read | 1.0 | 130 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.20 |
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 6 us, worker apply 60 us per rep
- events naming a host RM call 2.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x2.9
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 2.9; other events 0.5

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 420 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 426 |
| bar2-flush-read | 1.5 | 129 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 7 us, worker apply 42 us per rep
- events naming a host RM call 0.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.1
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.1; other events 0.6

## op 12 cuMemAlloc_64M: 10 repetition(s), 34.5 trapped accesses per repetition, window median 2516 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 19.6 | 2722 |
| pci-config | 12.6 | 208 |
| bar2-flush-read | 2.3 | 222 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 16.30 |
| BAR2 | flush page | R |  | 2.30 |
| PCI config |  | R | 0x34 | 1.50 |
| PCI config |  | R | 0x60 | 1.50 |
| PCI config |  | R | 0x6 | 1.40 |
| PCI config |  | R | 0x61 | 1.40 |
| PCI config |  | R | 0x68 | 1.40 |
| PCI config |  | R | 0x69 | 1.40 |
| PCI config |  | R | 0x78 | 1.40 |
| PCI config |  | R | 0x79 | 1.40 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| PCI config |  | R | 0x28 | 0.10 |
| PCI config |  | R | 0x2c | 0.10 |
| PCI config |  | R | 0x38 | 0.10 |
| PCI config |  | R | 0x3c | 0.10 |
| PCI config |  | R | 0x40 | 0.10 |
| PCI config |  | R | 0x44 | 0.10 |
| PCI config |  | R | 0x48 | 0.10 |
| (rest) | | | | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 31 us, worker apply 2173 us per rep
- events naming a host RM call 0.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 0.2; other events 0.9

## op 13 cuMemFree_64M: 10 repetition(s), 24.5 trapped accesses per repetition, window median 2846 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 21.4 | 2755 |
| bar2-flush-read | 3.1 | 239 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 15.40 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 3 leaves mapped and 32 unmapped in 3.2 RM map call(s), walk 37 us, worker apply 1958 us per rep
- events naming a host RM call 0.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 0.2; other events 1.1

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 8.4 trapped accesses per repetition, window median 1978 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.9 | 956 |
| bar2-flush-read | 2.5 | 352 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.90 |
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.4 per rep: 205 leaves mapped and 0 unmapped in 0.8 RM map call(s), walk 17 us, worker apply 298 us per rep
- device events per rep: guest MMU invalidates 1.0; other events 0.4

## op 15 cuMemFreeHost_1M: 10 repetition(s), 9.0 trapped accesses per repetition, window median 906 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 7.0 | 961 |
| bar2-flush-read | 2.0 | 152 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 11 us, worker apply 334 us per rep
- events naming a host RM call 0.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.4
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.4; other events 0.6

## op 16 cuMemHostRegister_1M: 10 repetition(s), 23.9 trapped accesses per repetition, window median 2286 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 18.9 | 2137 |
| bar2-flush-read | 5.0 | 373 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 12.90 |
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 1.2 RM map call(s), walk 24 us, worker apply 1068 us per rep
- device events per rep: guest MMU invalidates 2.0; other events 0.9

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.8 trapped accesses per repetition, window median 1398 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.8 | 1013 |
| bar2-flush-read | 4.0 | 284 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.80 |
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 17 us, worker apply 425 us per rep
- events naming a host RM call 0.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 0.2; other events 1.1

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 75 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 11823 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 974 us

- device events per rep: doorbell eventfd rings (token pool) 1.0; other events 5.0

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 2804 us

- device events per rep: doorbell eventfd rings (token pool) 1.0; other events 5.0

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 80 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 14173 us

- device events per rep: doorbell eventfd rings (token pool) 1.0; other events 5.0

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 52430 us

- device events per rep: doorbell eventfd rings (token pool) 1.0; other events 10.0

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 189 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1174 us

- device events per rep: doorbell eventfd rings (token pool) 1.0; other events 5.0

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 3097 us

- device events per rep: doorbell eventfd rings (token pool) 1.0; other events 5.1

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 40 us


## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 66 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 188 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 52 us



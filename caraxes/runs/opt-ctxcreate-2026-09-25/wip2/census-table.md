# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/wip2/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 51.3 | 100168 | 100082.5 |
| 2 | cuInit | 3 | 261.3 | 137344 | 137253.6 |
| 3 | cuCtxCreate | 5 | 1597.6 | 2332659 | 2332605.7 |
| 4 | cuCtxDestroy | 5 | 982.4 | 1636041 | 1635930.5 |
| 10 | cuMemAlloc_1M | 20 | 6.2 | 1692 | 1604.5 |
| 11 | cuMemFree_1M | 20 | 6.5 | 1614 | 1564.2 |
| 12 | cuMemAlloc_64M | 10 | 7.8 | 5915 | 5837.9 |
| 13 | cuMemFree_64M | 10 | 13.1 | 4124 | 4061.6 |
| 14 | cuMemHostAlloc_1M | 10 | 7.5 | 5266 | 5224.7 |
| 15 | cuMemFreeHost_1M | 10 | 7.9 | 3612 | 3513.7 |
| 16 | cuMemHostRegister_1M | 10 | 15.0 | 9500 | 9463.2 |
| 17 | cuMemHostUnregister_1M | 10 | 14.0 | 6931 | 6883.5 |
| 20 | launch_1_sync | 20 | 0.0 | 766 | 700.7 |
| 21 | launch_1000_sync | 1 | 0.0 | 48793 | 23.4 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 516 | 462.4 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 456 | 393.5 |
| 32 | d2d_1M | 10 | 0.0 | 976 | 926.7 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 5210 | 5158.7 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 5306 | 5252.1 |
| 35 | d2d_16M | 10 | 0.0 | 4322 | 4268.0 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 552 | 502.7 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 516 | 477.0 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 55 | 5.8 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 814 | 774.7 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 868 | 823.2 |
| 51 | cuModuleUnload | 10 | 0.0 | 69 | 28.4 |

## op 1 idle_100ms: 10 repetition(s), 51.3 trapped accesses per repetition, window median 100168 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 51.3 | 1085 |

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

- device events per rep: other events 15.1

## op 2 cuInit: 3 repetition(s), 261.3 trapped accesses per repetition, window median 137344 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 87.0 | 1571 |
| gsp-falcon | 82.0 | 15858 |
| mmu-invalidate | 40.0 | 8267 |
| gsp-queue-doorbell | 27.3 | 11402 |
| bar2-flush-read | 24.0 | 4301 |
| pcfg-mirror (intr re-arm) | 1.0 | 564 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 27.33 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 27.33 |
| BAR0 | 0x110000 | R | 0x110804 NV_PGSP | 27.33 |
| BAR0 | 0x110000 | R | 0x110808 NV_PGSP | 27.33 |
| BAR2 | flush page | R |  | 24.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 16.00 |
| PCI config |  | R | 0x34 | 10.00 |
| PCI config |  | R | 0x60 | 10.00 |
| PCI config |  | R | 0x68 | 10.00 |
| PCI config |  | R | 0x78 | 10.00 |
| PCI config |  | R | 0x6 | 9.67 |
| PCI config |  | R | 0x61 | 9.67 |
| (rest) | | | | 52.67 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 27.3 per rep (111 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 57.0

## op 3 cuCtxCreate: 5 repetition(s), 1597.6 trapped accesses per repetition, window median 2332659 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 487.0 | 38224 |
| pci-config | 410.4 | 8662 |
| bar2-flush-read | 272.2 | 26646 |
| gsp-falcon | 255.0 | 13985 |
| gsp-queue-doorbell | 85.0 | 15044 |
| usermode page, other | 60.0 | 3147 |
| usermode-doorbell | 20.0 | 1359 |
| bar0-other | 8.0 | 1859 |

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
| PCI config |  | R | 0x34 | 46.40 |
| PCI config |  | R | 0x60 | 46.40 |
| PCI config |  | R | 0x68 | 46.40 |
| (rest) | | | | 359.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): tokens 20.0

- GSP RPCs 85.0 per rep (355 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.0 per rep: 20130 leaves mapped and 252 unmapped in 105.0 RM map call(s), walk 3288 us, worker apply 96061 us per rep
- events naming a host RM call 540.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x194.0; doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x133.8; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x98.6; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0
- device events per rep: doorbell eventfd rings (token pool) 113.8; guest MMU invalidates 97.4; host doorbells written (forwarded) 133.8; host notifier events (no MSI) 194.0; host notifier events -> MSI 98.6; other events 608.2

## op 4 cuCtxDestroy: 5 repetition(s), 982.4 trapped accesses per repetition, window median 1636041 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 17296 |
| pci-config | 257.4 | 5717 |
| gsp-falcon | 207.0 | 10622 |
| bar2-flush-read | 169.0 | 12130 |
| gsp-queue-doorbell | 69.0 | 31306 |

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
| PCI config |  | R | 0x34 | 29.20 |
| PCI config |  | R | 0x60 | 29.20 |
| PCI config |  | R | 0x68 | 29.20 |
| (rest) | | | | 169.80 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- GSP RPCs 69.0 per rep (265 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 430 us, worker apply 6282 us per rep
- events naming a host RM call 271.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x177.4; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x67.0; free X class Some(N); mirror: chid N: host channel X freed in N us after N doorbell(s), N  x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 177.4; host notifier events -> MSI 67.0; other events 220.2

## op 10 cuMemAlloc_1M: 20 repetition(s), 6.2 trapped accesses per repetition, window median 1692 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 2.7 | 208 |
| mmu-invalidate | 2.5 | 328 |
| bar2-flush-read | 1.0 | 102 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| PCI config |  | R | 0x6 | 0.30 |
| PCI config |  | R | 0x34 | 0.30 |
| PCI config |  | R | 0x60 | 0.30 |
| PCI config |  | R | 0x61 | 0.30 |
| PCI config |  | R | 0x68 | 0.30 |
| PCI config |  | R | 0x69 | 0.30 |
| PCI config |  | R | 0x78 | 0.30 |
| (rest) | | | | 0.60 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 10 us, worker apply 74 us per rep
- events naming a host RM call 15.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x15.2; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.6
- device events per rep: guest MMU invalidates 0.5; host notifier events (no MSI) 0.6; host notifier events -> MSI 15.2; other events 1.1

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 1614 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 213 |
| bar2-flush-read | 1.5 | 128 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 8 us, worker apply 96 us per rep
- events naming a host RM call 4.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.7; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 1.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 4.7; other events 1.0

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 5915 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 1672 |
| bar2-flush-read | 2.3 | 209 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 63 us, worker apply 1945 us per rep
- events naming a host RM call 6.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.2
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 6.2; other events 2.0

## op 13 cuMemFree_64M: 10 repetition(s), 13.1 trapped accesses per repetition, window median 4124 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 515 |
| bar2-flush-read | 3.1 | 328 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 3 leaves mapped and 29 unmapped in 3.2 RM map call(s), walk 41 us, worker apply 4813 us per rep
- events naming a host RM call 4.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.9
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 4.9; other events 1.7

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 5266 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 605 |
| bar2-flush-read | 2.5 | 258 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 205 leaves mapped and 3 unmapped in 0.4 RM map call(s), walk 118 us, worker apply 712 us per rep
- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.0; other events 1.8

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.9 trapped accesses per repetition, window median 3612 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 296 |
| bar2-flush-read | 2.0 | 142 |
| pci-config | 0.9 | 17 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| PCI config |  | R | 0x6 | 0.10 |
| PCI config |  | R | 0x34 | 0.10 |
| PCI config |  | R | 0x60 | 0.10 |
| PCI config |  | R | 0x61 | 0.10 |
| PCI config |  | R | 0x68 | 0.10 |
| PCI config |  | R | 0x69 | 0.10 |
| PCI config |  | R | 0x78 | 0.10 |
| (rest) | | | | 0.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 51 leaves mapped and 205 unmapped in 0.2 RM map call(s), walk 13 us, worker apply 135 us per rep
- events naming a host RM call 5.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.8; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 1.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 5.8; other events 1.5

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.0 trapped accesses per repetition, window median 9500 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 1425 |
| bar2-flush-read | 5.0 | 441 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 230 leaves mapped and 51 unmapped in 2.9 RM map call(s), walk 33 us, worker apply 644 us per rep
- events naming a host RM call 6.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.2; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 6.2; other events 3.0

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.0 trapped accesses per repetition, window median 6931 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 580 |
| bar2-flush-read | 4.0 | 251 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.0 per rep: 26 leaves mapped and 230 unmapped in 0.5 RM map call(s), walk 21 us, worker apply 407 us per rep
- events naming a host RM call 5.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.8
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 5.8; other events 2.4

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 766 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 21 launch_1000_sync: 1 repetition(s), 0.0 trapped accesses per repetition, window median 48793 us

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1007.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 516 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 456 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 976 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5210 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.5

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5306 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 2.1

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 4322 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.0

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 552 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 516 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 55 us

- device events per rep: other events 0.1

## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 814 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 868 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 69 us



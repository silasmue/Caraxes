# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/after-2/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 10 | 31.5 | 101202 | 100131.7 |
| 2 | cuInit | 3 | 174.3 | 134603 | 133606.0 |
| 3 | cuCtxCreate | 5 | 1495.0 | 2485871 | 2485495.3 |
| 4 | cuCtxDestroy | 5 | 725.0 | 1197453 | 1196709.2 |
| 10 | cuMemAlloc_1M | 20 | 3.5 | 6362 | 3745.8 |
| 11 | cuMemFree_1M | 20 | 6.5 | 4670 | 2432.5 |
| 12 | cuMemAlloc_64M | 10 | 7.8 | 19355 | 17547.4 |
| 13 | cuMemFree_64M | 10 | 13.1 | 30608 | 29553.3 |
| 14 | cuMemHostAlloc_1M | 10 | 7.5 | 27810 | 26038.5 |
| 15 | cuMemFreeHost_1M | 10 | 7.0 | 11357 | 8268.7 |
| 16 | cuMemHostRegister_1M | 10 | 15.0 | 25475 | 24228.0 |
| 17 | cuMemHostUnregister_1M | 10 | 14.0 | 51091 | 47567.7 |
| 20 | launch_1_sync | 20 | 0.0 | 2581 | 2399.6 |
| 21 | launch_1000_sync | 1 | 513.0 | 3722150 | 1599.6 |
| 30 | h2d_pinned_1M | 10 | 0.0 | 1335 | 1204.2 |
| 31 | d2h_pinned_1M | 10 | 0.0 | 1262 | 873.2 |
| 32 | d2d_1M | 10 | 0.0 | 1962 | 1425.6 |
| 33 | h2d_pinned_16M | 10 | 0.0 | 10826 | 8266.2 |
| 34 | d2h_pinned_16M | 10 | 0.0 | 7430 | 6216.7 |
| 35 | d2d_16M | 10 | 0.0 | 6144 | 4813.3 |
| 36 | h2d_pageable_1M | 10 | 0.0 | 5200 | 4017.6 |
| 37 | d2h_pageable_1M | 10 | 0.0 | 2896 | 2256.2 |
| 40 | cuStreamSynchronize_idle | 20 | 0.0 | 252 | 67.2 |
| 41 | cuEventRecord_Synchronize | 20 | 0.0 | 2700 | 1494.8 |
| 50 | cuModuleLoad_cubin | 10 | 0.0 | 2592 | 2461.9 |
| 51 | cuModuleUnload | 10 | 0.0 | 900 | 60.1 |

## op 1 idle_100ms: 10 repetition(s), 31.5 trapped accesses per repetition, window median 101202 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 31.5 | 4485 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x34 | 3.60 |
| PCI config |  | R | 0x6 | 3.50 |
| PCI config |  | R | 0x60 | 3.50 |
| PCI config |  | R | 0x61 | 3.50 |
| PCI config |  | R | 0x68 | 3.50 |
| PCI config |  | R | 0x69 | 3.50 |
| PCI config |  | R | 0x78 | 3.50 |
| PCI config |  | R | 0x79 | 3.50 |
| PCI config |  | R | 0x0 | 0.10 |
| PCI config |  | R | 0x4 | 0.10 |
| PCI config |  | R | 0x8 | 0.10 |
| PCI config |  | R | 0xc | 0.10 |
| (rest) | | | | 3.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- device events per rep: other events 13.9

## op 2 cuInit: 3 repetition(s), 174.3 trapped accesses per repetition, window median 134603 us

| class | per rep | us per rep |
|---|---|---|
| gsp-falcon | 82.0 | 19596 |
| mmu-invalidate | 40.0 | 7635 |
| gsp-queue-doorbell | 27.3 | 12608 |
| bar2-flush-read | 24.0 | 7457 |
| pcfg-mirror (intr re-arm) | 1.0 | 123 |

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

- GSP RPCs 27.3 per rep (384 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 56.0

## op 3 cuCtxCreate: 5 repetition(s), 1495.0 trapped accesses per repetition, window median 2485871 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 487.0 | 169986 |
| pci-config | 307.8 | 42495 |
| bar2-flush-read | 272.2 | 107239 |
| gsp-falcon | 255.0 | 77982 |
| gsp-queue-doorbell | 85.0 | 40102 |
| usermode page, other | 60.0 | 15537 |
| usermode-doorbell | 20.0 | 0 |
| bar0-other | 8.0 | 3584 |

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
| PCI config |  | R | 0x34 | 34.80 |
| PCI config |  | R | 0x60 | 34.80 |
| (rest) | | | | 286.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): tokens 20.0

- GSP RPCs 85.0 per rep (21967 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 27.0 per rep: 20130 leaves mapped and 252 unmapped in 110.6 RM map call(s), walk 12123 us, worker apply 306931 us per rep
- events naming a host RM call 600.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x302.8; doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x96.2; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x87.6; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0
- device events per rep: doorbell eventfd rings (token pool) 76.2; guest MMU invalidates 97.4; host doorbells written (forwarded) 96.2; host notifier events (no MSI) 87.6; host notifier events -> MSI 302.8; other events 776.6

## op 4 cuCtxDestroy: 5 repetition(s), 725.0 trapped accesses per repetition, window median 1197453 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 280.0 | 82334 |
| gsp-falcon | 207.0 | 63749 |
| bar2-flush-read | 169.0 | 81467 |
| gsp-queue-doorbell | 69.0 | 24596 |

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

- GSP RPCs 69.0 per rep (1307 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x802009 x1.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0
- shadow plans 15.0 per rep: 0 leaves mapped and 17797 unmapped in 0.0 RM map call(s), walk 1127 us, worker apply 35362 us per rep
- events naming a host RM call 244.4 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x194.4; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x23.0; free X class Some(N); mirror: chid N: host channel X freed in N us after N doorbell(s), N  x20.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host] [synth x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X (X/X): host TSG X (engineType X) freed [host]; shado x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 23.0; host notifier events -> MSI 194.4; other events 332.4

## op 10 cuMemAlloc_1M: 20 repetition(s), 3.5 trapped accesses per repetition, window median 6362 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 2.5 | 841 |
| bar2-flush-read | 1.0 | 360 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.5 per rep: 0 leaves mapped and 0 unmapped in 0.5 RM map call(s), walk 21 us, worker apply 414 us per rep
- events naming a host RM call 17.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x15.8; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x1.4
- device events per rep: guest MMU invalidates 0.5; host notifier events (no MSI) 1.4; host notifier events -> MSI 15.8; other events 2.1

## op 11 cuMemFree_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 4670 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 2501 |
| bar2-flush-read | 1.5 | 865 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 0 leaves mapped and 0 unmapped in 0.1 RM map call(s), walk 26 us, worker apply 1147 us per rep
- events naming a host RM call 7.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.9; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.2
- device events per rep: guest MMU invalidates 1.0; host notifier events (no MSI) 0.2; host notifier events -> MSI 6.9; other events 2.5

## op 12 cuMemAlloc_64M: 10 repetition(s), 7.8 trapped accesses per repetition, window median 19355 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.5 | 1462 |
| bar2-flush-read | 2.3 | 1436 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.30 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.20 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 29 leaves mapped and 0 unmapped in 28.8 RM map call(s), walk 99 us, worker apply 5464 us per rep
- events naming a host RM call 12.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x7.4; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x4.6
- device events per rep: guest MMU invalidates 1.1; host notifier events (no MSI) 4.6; host notifier events -> MSI 7.4; other events 3.6

## op 13 cuMemFree_64M: 10 repetition(s), 13.1 trapped accesses per repetition, window median 30608 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 5169 |
| bar2-flush-read | 3.1 | 1750 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 3 leaves mapped and 32 unmapped in 3.2 RM map call(s), walk 81 us, worker apply 16571 us per rep
- events naming a host RM call 9.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x9.6; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.2
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.2; host notifier events -> MSI 9.6; other events 5.4

## op 14 cuMemHostAlloc_1M: 10 repetition(s), 7.5 trapped accesses per repetition, window median 27810 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 2402 |
| bar2-flush-read | 2.5 | 570 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.4 per rep: 205 leaves mapped and 0 unmapped in 0.7 RM map call(s), walk 341 us, worker apply 3357 us per rep
- events naming a host RM call 11.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x10.6; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.9
- device events per rep: guest MMU invalidates 1.0; host notifier events (no MSI) 0.9; host notifier events -> MSI 10.6; other events 6.1

## op 15 cuMemFreeHost_1M: 10 repetition(s), 7.0 trapped accesses per repetition, window median 11357 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 5.0 | 2529 |
| bar2-flush-read | 2.0 | 849 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.6 per rep: 51 leaves mapped and 256 unmapped in 0.2 RM map call(s), walk 127 us, worker apply 2513 us per rep
- events naming a host RM call 6.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.4; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.2
- device events per rep: guest MMU invalidates 1.0; host notifier events (no MSI) 0.2; host notifier events -> MSI 6.4; other events 2.3

## op 16 cuMemHostRegister_1M: 10 repetition(s), 15.0 trapped accesses per repetition, window median 25475 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 4542 |
| bar2-flush-read | 5.0 | 2385 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 5.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 0.9 per rep: 230 leaves mapped and 0 unmapped in 1.1 RM map call(s), walk 240 us, worker apply 2595 us per rep
- events naming a host RM call 12.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x10.9; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x1.4
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 1.4; host notifier events -> MSI 10.9; other events 5.6

## op 17 cuMemHostUnregister_1M: 10 repetition(s), 14.0 trapped accesses per repetition, window median 51091 us

| class | per rep | us per rep |
|---|---|---|
| mmu-invalidate | 10.0 | 3621 |
| bar2-flush-read | 4.0 | 1182 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830a4 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb83000 | W | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- shadow plans 1.1 per rep: 26 leaves mapped and 256 unmapped in 0.1 RM map call(s), walk 38 us, worker apply 596 us per rep
- events naming a host RM call 13.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x12.3; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.9
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.9; host notifier events -> MSI 12.3; other events 6.6

## op 20 launch_1_sync: 20 repetition(s), 0.0 trapped accesses per repetition, window median 2581 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.9

## op 21 launch_1000_sync: 1 repetition(s), 513.0 trapped accesses per repetition, window median 3722150 us

| class | per rep | us per rep |
|---|---|---|
| pci-config | 513.0 | 49322 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| PCI config |  | R | 0x34 | 58.00 |
| PCI config |  | R | 0x60 | 58.00 |
| PCI config |  | R | 0x68 | 58.00 |
| PCI config |  | R | 0x78 | 58.00 |
| PCI config |  | R | 0x6 | 57.00 |
| PCI config |  | R | 0x61 | 57.00 |
| PCI config |  | R | 0x69 | 57.00 |
| PCI config |  | R | 0x79 | 57.00 |
| PCI config |  | R | 0x0 | 1.00 |
| PCI config |  | R | 0x4 | 1.00 |
| PCI config |  | R | 0x8 | 1.00 |
| PCI config |  | R | 0xc | 1.00 |
| (rest) | | | | 49.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): none

- events naming a host RM call 1000.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0
- device events per rep: doorbell eventfd rings (token pool) 1000.0; host doorbells written (forwarded) 1000.0; other events 1358.0

## op 30 h2d_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1335 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.1

## op 31 d2h_pinned_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1262 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.2

## op 32 d2d_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 1962 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.4

## op 33 h2d_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 10826 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 2.1

## op 34 d2h_pinned_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 7430 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.4

## op 35 d2d_16M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 6144 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.8

## op 36 h2d_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 5200 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.8

## op 37 d2h_pageable_1M: 10 repetition(s), 0.0 trapped accesses per repetition, window median 2896 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.8

## op 40 cuStreamSynchronize_idle: 20 repetition(s), 0.0 trapped accesses per repetition, window median 252 us

- device events per rep: other events 0.1

## op 41 cuEventRecord_Synchronize: 20 repetition(s), 0.0 trapped accesses per repetition, window median 2700 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.6

## op 50 cuModuleLoad_cubin: 10 repetition(s), 0.0 trapped accesses per repetition, window median 2592 us

- events naming a host RM call 1.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0
- device events per rep: doorbell eventfd rings (token pool) 1.0; host doorbells written (forwarded) 1.0; other events 1.3

## op 51 cuModuleUnload: 10 repetition(s), 0.0 trapped accesses per repetition, window median 900 us

- device events per rep: other events 0.4


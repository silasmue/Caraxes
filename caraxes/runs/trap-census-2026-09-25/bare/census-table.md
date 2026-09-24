# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/bare/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 20 | 245.4 | 100155 | 100081.4 |
| 2 | cuInit | 3 | 4237.3 | 188768 | 188692.6 |
| 3 | cuCtxCreate | 3 | 89375.7 | 4463945 | 4463839.3 |
| 4 | cuCtxDestroy | 3 | 28299.3 | 1870423 | 1870283.9 |
| 10 | cuMemAlloc_1M | 40 | 35.5 | 1008 | 958.4 |
| 11 | cuMemFree_1M | 40 | 38.5 | 1312 | 973.9 |
| 12 | cuMemAlloc_64M | 20 | 265.9 | 7669 | 7491.7 |
| 13 | cuMemFree_64M | 20 | 142.8 | 5020 | 6137.4 |
| 14 | cuMemHostAlloc_1M | 20 | 906.4 | 25416 | 25716.8 |
| 15 | cuMemFreeHost_1M | 20 | 321.0 | 10108 | 9653.7 |
| 16 | cuMemHostRegister_1M | 20 | 1197.0 | 41062 | 40817.2 |
| 17 | cuMemHostUnregister_1M | 20 | 358.9 | 12846 | 12464.0 |
| 20 | launch_1_sync | 40 | 10.5 | 910 | 883.6 |
| 21 | launch_1000_sync | 2 | 4517.5 | 245918 | 137.8 |
| 30 | h2d_pinned_1M | 20 | 4.9 | 556 | 504.8 |
| 31 | d2h_pinned_1M | 20 | 6.6 | 557 | 550.9 |
| 32 | d2d_1M | 20 | 6.7 | 1034 | 1002.2 |
| 33 | h2d_pinned_16M | 20 | 13.5 | 5356 | 5298.9 |
| 34 | d2h_pinned_16M | 20 | 24.8 | 5442 | 5621.5 |
| 35 | d2d_16M | 20 | 5.8 | 3982 | 3888.8 |
| 36 | h2d_pageable_1M | 20 | 4.3 | 708 | 612.8 |
| 37 | d2h_pageable_1M | 20 | 5.7 | 720 | 670.9 |
| 40 | cuStreamSynchronize_idle | 40 | 0.1 | 54 | 8.2 |
| 41 | cuEventRecord_Synchronize | 40 | 8.4 | 864 | 802.9 |
| 50 | cuModuleLoad_cubin | 20 | 10.1 | 1129 | 1177.5 |
| 51 | cuModuleUnload | 20 | 0.2 | 70 | 26.3 |

## op 1 idle_100ms: 20 repetition(s), 245.4 trapped accesses per repetition, window median 100155 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 204.6 | 7538 |
| pci-config | 25.6 | 776 |
| pcfg-mirror (intr re-arm) | 14.5 | 575 |
| pmc-intr-en (RM lock) | 0.6 | 27 |
| pmc page 0, other | 0.1 | 4 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 29.20 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 29.15 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 29.15 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 29.15 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 29.05 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 14.55 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 14.50 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 14.50 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 14.50 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 14.50 |
| PCI config |  | R | 0x34 | 2.90 |
| PCI config |  | R | 0x60 | 2.90 |
| (rest) | | | | 21.35 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): intr 205.2

- events naming a host RM call 14.6 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.6
- device events per rep: host notifier events -> MSI 14.6

## op 2 cuInit: 3 repetition(s), 4237.3 trapped accesses per repetition, window median 188768 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 3116.0 | 115924 |
| pmc-intr-en (RM lock) | 525.3 | 23777 |
| vf-intr-tree | 421.3 | 13988 |
| gsp-falcon | 82.0 | 3818 |
| mmu-invalidate | 40.0 | 1753 |
| gsp-queue-doorbell | 27.3 | 6828 |
| bar2-flush-read | 24.0 | 970 |
| pcfg-mirror (intr re-arm) | 1.0 | 30 |
| pmc page 0, other | 0.3 | 13 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 3116.00 |
| BAR0 | 0x000000 | W | 0x000180 latched | 262.67 |
| BAR0 | 0x000000 | W | 0x000160 latched | 262.67 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 158.00 |
| BAR0 | 0xb81000 | W | 0xb81408 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 52.67 |
| BAR0 | 0xb81000 | W | 0xb81410 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 52.67 |
| BAR0 | 0xb81000 | W | 0xb81610 NV_VF_CPU_INTR_TOP_EN_CLEAR(0) | 52.67 |
| BAR0 | 0xb81000 | W | 0xb81208 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 52.67 |
| BAR0 | 0xb81000 | W | 0xb81210 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 52.67 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 27.33 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 27.33 |
| BAR0 | 0x110000 | R | 0x110804 NV_PGSP | 27.33 |
| (rest) | | | | 92.67 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 3116.0, intr 947.0

- GSP RPCs 27.3 per rep (105 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 54.7

## op 3 cuCtxCreate: 3 repetition(s), 89375.7 trapped accesses per repetition, window median 4463945 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 72736.3 | 3359060 |
| vf-intr-tree | 10268.7 | 756235 |
| pmc-intr-en (RM lock) | 2722.0 | 181334 |
| pci-config | 1026.0 | 28947 |
| bar1-write | 642.0 | 46900 |
| pcfg-mirror (intr re-arm) | 593.3 | 41029 |
| mmu-invalidate | 488.3 | 30602 |
| bar2-flush-read | 273.0 | 14629 |
| gsp-falcon | 255.0 | 18336 |
| usermode-doorbell | 214.0 | 10652 |
| gsp-queue-doorbell | 85.0 | 13374 |
| usermode page, other | 60.0 | 5359 |
| bar0-other | 8.0 | 2429 |
| pmc page 0, other | 4.0 | 264 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 72736.33 |
| BAR0 | 0x000000 | W | 0x000180 latched | 1361.00 |
| BAR0 | 0x000000 | W | 0x000160 latched | 1361.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 1165.00 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 1143.33 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1143.33 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 1143.33 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1143.33 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 817.00 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 593.33 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 593.33 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 593.33 |
| (rest) | | | | 5582.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 642.0, bar2memfd 72736.3, intr 12994.7, tokens 214.0

- GSP RPCs 85.0 per rep (428 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.3 per rep: 19535 leaves mapped and 4947 unmapped in 6398.3 RM map call(s), walk 4027 us, worker apply 861347 us per rep
- events naming a host RM call 973.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x613.0; doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x214.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x52.0; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0
- device events per rep: guest MMU invalidates 97.7; host doorbells written (forwarded) 214.0; host notifier events (no MSI) 52.0; host notifier events -> MSI 613.0; other events 349.3

## op 4 cuCtxDestroy: 3 repetition(s), 28299.3 trapped accesses per repetition, window median 1870423 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 20360.0 | 936369 |
| vf-intr-tree | 4931.3 | 327277 |
| pmc-intr-en (RM lock) | 2012.0 | 119637 |
| mmu-invalidate | 280.0 | 16580 |
| pcfg-mirror (intr re-arm) | 245.7 | 12442 |
| gsp-falcon | 207.0 | 11770 |
| bar2-flush-read | 169.0 | 9729 |
| gsp-queue-doorbell | 69.0 | 18510 |
| pci-config | 24.0 | 568 |
| pmc page 0, other | 1.3 | 79 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 20360.00 |
| BAR0 | 0x000000 | W | 0x000180 latched | 1006.00 |
| BAR0 | 0x000000 | W | 0x000160 latched | 1006.00 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 604.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 479.67 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 467.67 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 467.67 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 467.67 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 467.33 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 245.67 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 245.67 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 245.67 |
| (rest) | | | | 2236.33 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 20360.0, intr 6944.7

- GSP RPCs 69.0 per rep (868 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0; GSP_RM_CONTROL cmd 0x802009 x1.0
- shadow plans 15.3 per rep: 0 leaves mapped and 12507 unmapped in 0.0 RM map call(s), walk 304 us, worker apply 48435 us per rep
- events naming a host RM call 281.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x249.3; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x5.0; free X class Some(N); mirror: tsg X/X: host TSG X (engineType X) freed [host] [synthesized x3.0; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X: host TSG X (engineType X) freed [host]; shadow: VAS x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 5.0; host notifier events -> MSI 249.3; other events 154.0

## op 10 cuMemAlloc_1M: 40 repetition(s), 35.5 trapped accesses per repetition, window median 1008 us

| class | per rep | us per rep |
|---|---|---|
| pmc-intr-en (RM lock) | 15.0 | 611 |
| vf-intr-tree | 14.8 | 458 |
| mmu-invalidate | 2.5 | 88 |
| bar2-write | 2.0 | 124 |
| bar2-flush-read | 1.0 | 36 |
| pcfg-mirror (intr re-arm) | 0.2 | 6 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x000000 | W | 0x000180 latched | 7.50 |
| BAR0 | 0x000000 | W | 0x000160 latched | 7.50 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 4.50 |
| BAR2 | other pages | W |  | 2.00 |
| BAR0 | 0xb81000 | W | 0xb81408 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 1.50 |
| BAR0 | 0xb81000 | W | 0xb81410 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 1.50 |
| BAR0 | 0xb81000 | W | 0xb81610 NV_VF_CPU_INTR_TOP_EN_CLEAR(0) | 1.50 |
| BAR0 | 0xb81000 | W | 0xb81208 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.50 |
| BAR0 | 0xb81000 | W | 0xb81210 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.50 |
| BAR2 | flush page | R |  | 1.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0.50 |
| (rest) | | | | 4.03 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 2.0, intr 29.8

- shadow plans 0.2 per rep: 0 leaves mapped and 0 unmapped in 0.2 RM map call(s), walk 3 us, worker apply 24 us per rep
- events naming a host RM call 0.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 0.2; other events 0.2

## op 11 cuMemFree_1M: 40 repetition(s), 38.5 trapped accesses per repetition, window median 1312 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 15.6 | 505 |
| pmc-intr-en (RM lock) | 15.0 | 640 |
| mmu-invalidate | 5.0 | 177 |
| bar2-flush-read | 1.5 | 60 |
| bar2-write | 1.0 | 47 |
| pcfg-mirror (intr re-arm) | 0.3 | 10 |
| pmc page 0, other | 0.0 | 1 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x000000 | W | 0x000180 latched | 7.53 |
| BAR0 | 0x000000 | W | 0x000160 latched | 7.50 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 4.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb81000 | W | 0xb81408 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 1.50 |
| BAR0 | 0xb81000 | W | 0xb81410 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 1.50 |
| BAR0 | 0xb81000 | W | 0xb81610 NV_VF_CPU_INTR_TOP_EN_CLEAR(0) | 1.50 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb81000 | W | 0xb81208 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.50 |
| BAR0 | 0xb81000 | W | 0xb81210 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.50 |
| BAR2 | other pages | W |  | 1.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 1.00 |
| (rest) | | | | 5.95 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 1.0, intr 30.6

- shadow plans 0.4 per rep: 0 leaves mapped and 0 unmapped in 0.0 RM map call(s), walk 5 us, worker apply 59 us per rep
- events naming a host RM call 0.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.3
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.3; other events 0.4

## op 12 cuMemAlloc_64M: 20 repetition(s), 265.9 trapped accesses per repetition, window median 7669 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 179.6 | 7162 |
| vf-intr-tree | 46.9 | 1921 |
| pmc-intr-en (RM lock) | 30.0 | 1238 |
| mmu-invalidate | 5.5 | 226 |
| bar2-flush-read | 2.3 | 82 |
| pcfg-mirror (intr re-arm) | 1.6 | 51 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 179.60 |
| BAR0 | 0x000000 | W | 0x000180 latched | 15.00 |
| BAR0 | 0x000000 | W | 0x000160 latched | 15.00 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 9.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 3.35 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 3.20 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 3.20 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 3.20 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 3.20 |
| BAR0 | 0xb81000 | W | 0xb81408 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 3.00 |
| BAR0 | 0xb81000 | W | 0xb81410 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 3.00 |
| BAR0 | 0xb81000 | W | 0xb81610 NV_VF_CPU_INTR_TOP_EN_CLEAR(0) | 3.00 |
| (rest) | | | | 22.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 179.6, intr 76.8

- events naming a host RM call 1.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x1.9
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 1.9

## op 13 cuMemFree_64M: 20 repetition(s), 142.8 trapped accesses per repetition, window median 5020 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 64.1 | 2413 |
| vf-intr-tree | 34.7 | 1908 |
| pmc-intr-en (RM lock) | 30.0 | 1486 |
| mmu-invalidate | 10.0 | 436 |
| bar2-flush-read | 3.1 | 277 |
| pcfg-mirror (intr re-arm) | 0.8 | 34 |
| pmc page 0, other | 0.1 | 4 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 64.10 |
| BAR0 | 0x000000 | W | 0x000180 latched | 15.00 |
| BAR0 | 0x000000 | W | 0x000160 latched | 15.00 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 9.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR2 | flush page | R |  | 3.10 |
| BAR0 | 0xb81000 | W | 0xb81408 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 3.00 |
| BAR0 | 0xb81000 | W | 0xb81410 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 3.00 |
| BAR0 | 0xb81000 | W | 0xb81610 NV_VF_CPU_INTR_TOP_EN_CLEAR(0) | 3.00 |
| BAR0 | 0xb81000 | W | 0xb81208 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 3.00 |
| BAR0 | 0xb81000 | W | 0xb81210 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 3.00 |
| BAR0 | 0xb83000 | W | 0xb830a0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| (rest) | | | | 15.55 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 64.1, intr 64.8

- shadow plans 0.4 per rep: 0 leaves mapped and 13 unmapped in 0.0 RM map call(s), walk 15 us, worker apply 705 us per rep
- events naming a host RM call 0.9 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.9
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 0.9; other events 0.4

## op 14 cuMemHostAlloc_1M: 20 repetition(s), 906.4 trapped accesses per repetition, window median 25416 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 771.0 | 27716 |
| vf-intr-tree | 82.2 | 3725 |
| pci-config | 25.6 | 348 |
| pmc-intr-en (RM lock) | 15.0 | 1220 |
| pcfg-mirror (intr re-arm) | 5.0 | 155 |
| mmu-invalidate | 5.0 | 238 |
| bar2-flush-read | 2.5 | 113 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 771.00 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 10.05 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 10.00 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 10.00 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 10.00 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 10.00 |
| BAR0 | 0x000000 | W | 0x000180 latched | 7.50 |
| BAR0 | 0x000000 | W | 0x000160 latched | 7.50 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 5.05 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 5.05 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 5.05 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 5.05 |
| (rest) | | | | 50.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 771.0, intr 97.2

- events naming a host RM call 5.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x5.0
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 5.0

## op 15 cuMemFreeHost_1M: 20 repetition(s), 321.0 trapped accesses per repetition, window median 10108 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 257.0 | 9518 |
| vf-intr-tree | 40.0 | 1828 |
| pmc-intr-en (RM lock) | 15.0 | 691 |
| mmu-invalidate | 5.0 | 179 |
| bar2-flush-read | 2.0 | 78 |
| pcfg-mirror (intr re-arm) | 1.9 | 73 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 257.00 |
| BAR0 | 0x000000 | W | 0x000180 latched | 7.50 |
| BAR0 | 0x000000 | W | 0x000160 latched | 7.50 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 4.50 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 4.05 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 4.05 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 4.05 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 4.05 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 4.00 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 2.00 |
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| (rest) | | | | 18.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 257.0, intr 55.0

- shadow plans 0.2 per rep: 0 leaves mapped and 128 unmapped in 0.0 RM map call(s), walk 7 us, worker apply 397 us per rep
- events naming a host RM call 2.0 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x2.0
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 2.0; other events 0.2

## op 16 cuMemHostRegister_1M: 20 repetition(s), 1197.0 trapped accesses per repetition, window median 41062 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 1030.0 | 35845 |
| vf-intr-tree | 115.2 | 4481 |
| pmc-intr-en (RM lock) | 30.0 | 1150 |
| mmu-invalidate | 10.0 | 504 |
| pcfg-mirror (intr re-arm) | 6.7 | 219 |
| bar2-flush-read | 5.0 | 190 |
| pmc page 0, other | 0.1 | 2 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 1030.00 |
| BAR0 | 0x000000 | W | 0x000180 latched | 15.00 |
| BAR0 | 0x000000 | W | 0x000160 latched | 15.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 13.10 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 12.95 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 12.95 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 12.85 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 12.85 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 9.00 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 6.70 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 6.70 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 6.70 |
| (rest) | | | | 43.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 1030.0, intr 145.2

- events naming a host RM call 6.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.8; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 6.8

## op 17 cuMemHostUnregister_1M: 20 repetition(s), 358.9 trapped accesses per repetition, window median 12846 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 258.0 | 9192 |
| vf-intr-tree | 54.8 | 2838 |
| pmc-intr-en (RM lock) | 30.0 | 1672 |
| mmu-invalidate | 10.0 | 380 |
| bar2-flush-read | 4.0 | 161 |
| pcfg-mirror (intr re-arm) | 2.1 | 66 |
| pmc page 0, other | 0.1 | 1 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 258.00 |
| BAR0 | 0x000000 | W | 0x000180 latched | 15.00 |
| BAR0 | 0x000000 | W | 0x000160 latched | 15.00 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 9.00 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 4.45 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 4.45 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 4.45 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 4.35 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 4.30 |
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb81000 | W | 0xb81408 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 3.00 |
| (rest) | | | | 28.95 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 258.0, intr 84.8

- shadow plans 0.7 per rep: 0 leaves mapped and 166 unmapped in 0.0 RM map call(s), walk 12 us, worker apply 2134 us per rep
- events naming a host RM call 2.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x2.3
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 2.3; other events 0.7

## op 20 launch_1_sync: 40 repetition(s), 10.5 trapped accesses per repetition, window median 910 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 4.3 | 246 |
| bar1-write | 3.0 | 320 |
| pci-config | 1.8 | 26 |
| usermode-doorbell | 1.0 | 50 |
| pcfg-mirror (intr re-arm) | 0.3 | 10 |
| pmc-intr-en (RM lock) | 0.1 | 29 |
| pmc page 0, other | 0.0 | 1 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.57 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.57 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.57 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.55 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.53 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.35 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.33 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.33 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.28 |
| (rest) | | | | 2.45 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 4.5, tokens 1.0

- events naming a host RM call 1.4 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.4; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.0
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events (no MSI) 0.0; host notifier events -> MSI 0.4

## op 21 launch_1000_sync: 2 repetition(s), 4517.5 trapped accesses per repetition, window median 245918 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3000.0 | 145648 |
| usermode-doorbell | 1000.0 | 36169 |
| vf-intr-tree | 483.0 | 37936 |
| pcfg-mirror (intr re-arm) | 34.5 | 1212 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 1984.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1016.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1000.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 69.00 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 69.00 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 69.00 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 69.00 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 69.00 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 34.50 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 34.50 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 34.50 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 34.50 |
| (rest) | | | | 34.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3000.0, intr 483.0, tokens 1000.0

- events naming a host RM call 1034.5 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x34.5
- device events per rep: host doorbells written (forwarded) 1000.0; host notifier events -> MSI 34.5

## op 30 h2d_pinned_1M: 20 repetition(s), 4.9 trapped accesses per repetition, window median 556 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 172 |
| usermode-doorbell | 1.0 | 43 |
| vf-intr-tree | 0.8 | 75 |
| pcfg-mirror (intr re-arm) | 0.1 | 10 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| (rest) | | | | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 0.8, tokens 1.0

- events naming a host RM call 1.1 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.1
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.1

## op 31 d2h_pinned_1M: 20 repetition(s), 6.6 trapped accesses per repetition, window median 557 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 186 |
| vf-intr-tree | 2.5 | 87 |
| usermode-doorbell | 1.0 | 48 |
| pcfg-mirror (intr re-arm) | 0.1 | 54 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.40 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.35 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.35 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.35 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.35 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.20 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.20 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| (rest) | | | | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 2.5, tokens 1.0

- events naming a host RM call 1.2 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.2

## op 32 d2d_1M: 20 repetition(s), 6.7 trapped accesses per repetition, window median 1034 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 152 |
| vf-intr-tree | 2.5 | 165 |
| usermode-doorbell | 1.0 | 52 |
| pcfg-mirror (intr re-arm) | 0.2 | 6 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.40 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.40 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.40 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.35 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.35 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.20 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| (rest) | | | | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 2.5, tokens 1.0

- events naming a host RM call 1.2 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 0.2

## op 33 h2d_pinned_16M: 20 repetition(s), 13.5 trapped accesses per repetition, window median 5356 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 8.9 | 533 |
| bar1-write | 3.0 | 142 |
| usermode-doorbell | 1.0 | 43 |
| pcfg-mirror (intr re-arm) | 0.6 | 22 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 1.30 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.30 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 1.25 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 1.25 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.25 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.65 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.65 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.65 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.60 |
| (rest) | | | | 0.60 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 8.9, tokens 1.0

- events naming a host RM call 1.6 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.7
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.7

## op 34 d2h_pinned_16M: 20 repetition(s), 24.8 trapped accesses per repetition, window median 5442 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 10.5 | 760 |
| pci-config | 9.4 | 257 |
| bar1-write | 3.0 | 253 |
| usermode-doorbell | 1.0 | 226 |
| pcfg-mirror (intr re-arm) | 0.8 | 28 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.55 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 1.50 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 1.45 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.45 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 1.40 |
| PCI config |  | R | 0x6 | 1.05 |
| PCI config |  | R | 0x34 | 1.05 |
| PCI config |  | R | 0x60 | 1.05 |
| PCI config |  | R | 0x61 | 1.05 |
| PCI config |  | R | 0x68 | 1.05 |
| PCI config |  | R | 0x69 | 1.05 |
| (rest) | | | | 9.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 10.5, tokens 1.0

- events naming a host RM call 1.8 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.8
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.8

## op 35 d2d_16M: 20 repetition(s), 5.8 trapped accesses per repetition, window median 3982 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 169 |
| vf-intr-tree | 1.6 | 70 |
| usermode-doorbell | 1.0 | 48 |
| pcfg-mirror (intr re-arm) | 0.1 | 6 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.25 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.25 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.20 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.20 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.15 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| (rest) | | | | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 1.6, tokens 1.0

- events naming a host RM call 1.1 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.1
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.1

## op 36 h2d_pageable_1M: 20 repetition(s), 4.3 trapped accesses per repetition, window median 708 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 360 |
| usermode-doorbell | 1.0 | 50 |
| vf-intr-tree | 0.3 | 23 |
| pcfg-mirror (intr re-arm) | 0.1 | 2 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 0.3, tokens 1.0

- events naming a host RM call 1.1 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.1
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.1

## op 37 d2h_pageable_1M: 20 repetition(s), 5.7 trapped accesses per repetition, window median 720 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 263 |
| vf-intr-tree | 1.6 | 79 |
| usermode-doorbell | 1.0 | 50 |
| pcfg-mirror (intr re-arm) | 0.1 | 4 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 1.90 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.10 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.25 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.25 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.25 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.25 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.20 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| (rest) | | | | 0.10 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 1.6, tokens 1.0

- events naming a host RM call 1.1 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.1
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.1

## op 40 cuStreamSynchronize_idle: 40 repetition(s), 0.1 trapped accesses per repetition, window median 54 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 0.1 | 2 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.03 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.03 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): intr 0.1


## op 41 cuEventRecord_Synchronize: 40 repetition(s), 8.4 trapped accesses per repetition, window median 864 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 4.2 | 259 |
| bar1-write | 3.0 | 147 |
| usermode-doorbell | 1.0 | 48 |
| pcfg-mirror (intr re-arm) | 0.1 | 3 |
| pmc-intr-en (RM lock) | 0.1 | 2 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 1.95 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.05 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.68 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.62 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.62 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.62 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.60 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.30 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.25 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.23 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.20 |
| (rest) | | | | 0.28 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 4.3, tokens 1.0

- events naming a host RM call 1.3 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.3
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.3

## op 50 cuModuleLoad_cubin: 20 repetition(s), 10.1 trapped accesses per repetition, window median 1129 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 3.3 | 229 |
| bar1-write | 3.0 | 244 |
| pci-config | 2.2 | 78 |
| usermode-doorbell | 1.0 | 50 |
| pmc-intr-en (RM lock) | 0.3 | 8 |
| pcfg-mirror (intr re-arm) | 0.1 | 4 |
| pmc page 0, other | 0.1 | 1 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.45 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.45 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.40 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.40 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.40 |
| PCI config |  | R | 0x6 | 0.25 |
| PCI config |  | R | 0x34 | 0.25 |
| PCI config |  | R | 0x60 | 0.25 |
| PCI config |  | R | 0x61 | 0.25 |
| (rest) | | | | 2.95 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 3.6, tokens 1.0

- events naming a host RM call 1.2 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.2

## op 51 cuModuleUnload: 20 repetition(s), 0.2 trapped accesses per repetition, window median 70 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 0.2 | 7 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.05 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): intr 0.2

- events naming a host RM call 0.1 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.1
- device events per rep: host notifier events -> MSI 0.1


# census-table: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/bare-2/census-trace.txt

| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |
|---|---|---|---|---|---|
| 1 | idle_100ms | 20 | 249.2 | 100120 | 100077.0 |
| 2 | cuInit | 3 | 4232.3 | 187930 | 187836.3 |
| 3 | cuCtxCreate | 3 | 84905.7 | 3393211 | 3393128.2 |
| 4 | cuCtxDestroy | 3 | 27135.7 | 941139 | 941098.9 |
| 10 | cuMemAlloc_1M | 40 | 36.5 | 968 | 934.5 |
| 11 | cuMemFree_1M | 40 | 46.2 | 1016 | 995.8 |
| 12 | cuMemAlloc_64M | 20 | 266.5 | 6930 | 6830.5 |
| 13 | cuMemFree_64M | 20 | 143.7 | 4396 | 4705.9 |
| 14 | cuMemHostAlloc_1M | 20 | 902.7 | 23822 | 24412.8 |
| 15 | cuMemFreeHost_1M | 20 | 313.2 | 8383 | 8615.0 |
| 16 | cuMemHostRegister_1M | 20 | 1221.3 | 39124 | 39806.8 |
| 17 | cuMemHostUnregister_1M | 20 | 356.7 | 10536 | 10384.5 |
| 20 | launch_1_sync | 40 | 9.4 | 854 | 799.5 |
| 21 | launch_1000_sync | 2 | 4552.5 | 276000 | 127.2 |
| 30 | h2d_pinned_1M | 20 | 5.0 | 564 | 501.7 |
| 31 | d2h_pinned_1M | 20 | 5.0 | 506 | 466.1 |
| 32 | d2d_1M | 20 | 6.5 | 969 | 931.5 |
| 33 | h2d_pinned_16M | 20 | 12.4 | 5312 | 5267.7 |
| 34 | d2h_pinned_16M | 20 | 15.8 | 5398 | 5343.6 |
| 35 | d2d_16M | 20 | 8.0 | 3862 | 3828.6 |
| 36 | h2d_pageable_1M | 20 | 6.7 | 652 | 623.0 |
| 37 | d2h_pageable_1M | 20 | 4.5 | 642 | 592.9 |
| 40 | cuStreamSynchronize_idle | 40 | 0.1 | 40 | 5.4 |
| 41 | cuEventRecord_Synchronize | 40 | 5.6 | 808 | 755.5 |
| 50 | cuModuleLoad_cubin | 20 | 8.4 | 944 | 896.6 |
| 51 | cuModuleUnload | 20 | 0.1 | 59 | 16.6 |

## op 1 idle_100ms: 20 repetition(s), 249.2 trapped accesses per repetition, window median 100120 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 207.9 | 7603 |
| pci-config | 25.6 | 555 |
| pcfg-mirror (intr re-arm) | 14.8 | 508 |
| pmc-intr-en (RM lock) | 0.6 | 114 |
| pmc page 0, other | 0.1 | 3 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 29.60 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 29.55 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 29.55 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 29.55 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 29.50 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 14.90 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 14.85 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 14.85 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 14.85 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 14.80 |
| PCI config |  | R | 0x34 | 2.90 |
| PCI config |  | R | 0x60 | 2.90 |
| (rest) | | | | 21.35 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): intr 208.7

- events naming a host RM call 14.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x14.8
- device events per rep: host notifier events -> MSI 14.8

## op 2 cuInit: 3 repetition(s), 4232.3 trapped accesses per repetition, window median 187930 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 3116.0 | 113349 |
| pmc-intr-en (RM lock) | 523.3 | 25010 |
| vf-intr-tree | 418.7 | 14873 |
| gsp-falcon | 82.0 | 4787 |
| mmu-invalidate | 40.0 | 1412 |
| gsp-queue-doorbell | 27.3 | 7005 |
| bar2-flush-read | 24.0 | 1013 |
| pcfg-mirror (intr re-arm) | 1.0 | 20 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 3116.00 |
| BAR0 | 0x000000 | W | 0x000180 latched | 261.67 |
| BAR0 | 0x000000 | W | 0x000160 latched | 261.67 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 157.00 |
| BAR0 | 0xb81000 | W | 0xb81408 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 52.33 |
| BAR0 | 0xb81000 | W | 0xb81410 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 52.33 |
| BAR0 | 0xb81000 | W | 0xb81610 NV_VF_CPU_INTR_TOP_EN_CLEAR(0) | 52.33 |
| BAR0 | 0xb81000 | W | 0xb81208 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 52.33 |
| BAR0 | 0xb81000 | W | 0xb81210 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 52.33 |
| BAR0 | 0x110000 | W | 0x110c00 NV_PGSP_QUEUE_HEAD/TAIL | 27.33 |
| BAR0 | 0x110000 | R | 0x110094 NV_PGSP | 27.33 |
| BAR0 | 0x110000 | R | 0x110804 NV_PGSP | 27.33 |
| (rest) | | | | 92.33 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 3116.0, intr 942.0

- GSP RPCs 27.3 per rep (565 us device time, 12.0 with a host RM call): GSP_RM_CONTROL cmd 0x20802a12 x5.0; GSP_RM_CONTROL cmd 0x20801303 x2.0; GSP_RM_ALLOC class 0x90f1 x2.0; GSP_RM_CONTROL cmd 0x90f10106 x2.0; GSP_RM_ALLOC class 0x0 x1.0; GSP_RM_ALLOC class 0x80 x1.0; GSP_RM_ALLOC class 0x2080 x1.0; GSP_RM_ALLOC class 0x2081 x1.0
- events naming a host RM call 12.0 per rep: ctrl X: fb info vN: host status X; N size entries set to N MiB -> status X [host (FB_GET_I x2.0; ctrl X: GSS legacy gpu X (cuInit): host status X -> status X [host (GSS legacy, forwarded  x2.0; ctrl X: binapi X: host status X -> status X [host (BINAPI object of the host client, forwa x1.0; ctrl X: BUS_GET_INFO_VN: host status X -> status X [host (forwarded as-is, public control) x1.0; ctrl X: BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS: host status X -> status X [host (forwarded as- x1.0; ctrl X: GPU_QUERY_ECC_STATUS: host status X -> status X [host (forwarded as-is, public con x1.0
- device events per rep: guest MMU invalidates 8.0; other events 54.7

## op 3 cuCtxCreate: 3 repetition(s), 84905.7 trapped accesses per repetition, window median 3393211 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 72736.3 | 2338226 |
| vf-intr-tree | 6584.0 | 233578 |
| pmc-intr-en (RM lock) | 2726.0 | 109250 |
| bar1-write | 642.0 | 33207 |
| pci-config | 513.0 | 8156 |
| mmu-invalidate | 488.3 | 17081 |
| pcfg-mirror (intr re-arm) | 318.3 | 10627 |
| bar2-flush-read | 273.0 | 9879 |
| gsp-falcon | 255.0 | 10437 |
| usermode-doorbell | 214.0 | 6216 |
| gsp-queue-doorbell | 85.0 | 6767 |
| usermode page, other | 60.0 | 1986 |
| bar0-other | 8.0 | 1787 |
| pmc page 0, other | 2.7 | 116 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 72736.33 |
| BAR0 | 0x000000 | W | 0x000180 latched | 1363.00 |
| BAR0 | 0x000000 | W | 0x000160 latched | 1363.00 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 819.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 631.00 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 625.33 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 625.33 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 625.33 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 625.33 |
| BAR1 | [0, 32 MiB) | W |  | 424.00 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 318.33 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 318.33 |
| (rest) | | | | 4431.33 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 642.0, bar2memfd 72736.3, intr 9312.7, tokens 214.0

- GSP RPCs 85.0 per rep (1583 us device time, 1.0 with a host RM call): GSP_RM_ALLOC class 0xc46f x20.0; GSP_RM_ALLOC class 0xc5b5 x20.0; GSP_RM_ALLOC class 0xc5c0 x8.0; GSP_RM_ALLOC class 0x79 x8.0; GSP_RM_CONTROL cmd 0x20802a08 x4.0; GSP_RM_ALLOC class 0xa06c x4.0; GSP_RM_CONTROL cmd 0xa06c010a x4.0; GSP_RM_CONTROL cmd 0xa06c0101 x4.0
- shadow plans 20.3 per rep: 19535 leaves mapped and 4947 unmapped in 6500.3 RM map call(s), walk 5205 us, worker apply 761908 us per rep
- events naming a host RM call 689.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x330.3; doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x214.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x51.0; shadow: chid N host object class X X [host] x28.0; shadow: host GR channel X in TSG X on VAS X, gpfifo va X [host] x20.0; shadow: chid N scheduled; host token X (host chid N, runlist N) [host] x20.0
- device events per rep: guest MMU invalidates 97.7; host doorbells written (forwarded) 214.0; host notifier events (no MSI) 51.0; host notifier events -> MSI 330.3; other events 349.7

## op 4 cuCtxDestroy: 3 repetition(s), 27135.7 trapped accesses per repetition, window median 941139 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 20360.0 | 614162 |
| vf-intr-tree | 3557.0 | 122722 |
| pmc-intr-en (RM lock) | 2010.0 | 82243 |
| pci-config | 342.0 | 5113 |
| mmu-invalidate | 280.0 | 10060 |
| gsp-falcon | 207.0 | 7317 |
| bar2-flush-read | 169.0 | 6286 |
| pcfg-mirror (intr re-arm) | 140.7 | 4703 |
| gsp-queue-doorbell | 69.0 | 25841 |
| pmc page 0, other | 1.0 | 44 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 20360.00 |
| BAR0 | 0x000000 | W | 0x000180 latched | 1005.00 |
| BAR0 | 0x000000 | W | 0x000160 latched | 1005.00 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 603.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 279.33 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 277.33 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 277.33 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 277.33 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 277.00 |
| BAR0 | 0xb81000 | W | 0xb81408 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 201.00 |
| BAR0 | 0xb81000 | W | 0xb81410 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 201.00 |
| BAR0 | 0xb81000 | W | 0xb81610 NV_VF_CPU_INTR_TOP_EN_CLEAR(0) | 201.00 |
| (rest) | | | | 2171.33 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 20360.0, intr 5568.0

- GSP RPCs 69.0 per rep (848 us device time, 26.0 with a host RM call): FREE x65.0; GSP_RM_CONTROL cmd 0xa06c0105 x2.0; GSP_RM_CONTROL cmd 0x20800a9a x1.0; GSP_RM_CONTROL cmd 0x802009 x1.0
- shadow plans 15.3 per rep: 0 leaves mapped and 12507 unmapped in 0.0 RM map call(s), walk 196 us, worker apply 35162 us per rep
- events naming a host RM call 170.7 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x141.3; free X class Some(N); mirror: chid N: host channel X freed after N doorbell(s), N ms of li x20.0; free X class Some(N); mirror: tsg X/X: host TSG X (engineType X) freed [host] [synthesized x3.0; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x2.7; ctrl X: control X forwarded to the host tsg X: X NV_OK [host]; host status X -> status X [ x2.0; free X class Some(N); mirror: tsg X/X: host TSG X (engineType X) freed [host]; shadow: VAS x1.0
- device events per rep: guest MMU invalidates 56.0; host notifier events (no MSI) 2.7; host notifier events -> MSI 141.3; other events 154.0

## op 10 cuMemAlloc_1M: 40 repetition(s), 36.5 trapped accesses per repetition, window median 968 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 15.7 | 627 |
| pmc-intr-en (RM lock) | 15.0 | 667 |
| mmu-invalidate | 2.5 | 108 |
| bar2-write | 2.0 | 102 |
| bar2-flush-read | 1.0 | 41 |
| pcfg-mirror (intr re-arm) | 0.3 | 12 |
| pmc page 0, other | 0.0 | 2 |

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
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.55 |
| (rest) | | | | 4.92 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 2.0, intr 30.7

- shadow plans 0.2 per rep: 0 leaves mapped and 0 unmapped in 0.2 RM map call(s), walk 3 us, worker apply 27 us per rep
- events naming a host RM call 0.3 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.3
- device events per rep: guest MMU invalidates 0.5; host notifier events -> MSI 0.3; other events 0.2

## op 11 cuMemFree_1M: 40 repetition(s), 46.2 trapped accesses per repetition, window median 1016 us

| class | per rep | us per rep |
|---|---|---|
| pmc-intr-en (RM lock) | 15.0 | 495 |
| vf-intr-tree | 14.2 | 419 |
| pci-config | 9.2 | 232 |
| mmu-invalidate | 5.0 | 190 |
| bar2-flush-read | 1.5 | 70 |
| bar2-write | 1.0 | 58 |
| pcfg-mirror (intr re-arm) | 0.2 | 7 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0x000000 | W | 0x000180 latched | 7.50 |
| BAR0 | 0x000000 | W | 0x000160 latched | 7.50 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 4.50 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb81000 | W | 0xb81408 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 1.50 |
| BAR0 | 0xb81000 | W | 0xb81410 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 1.50 |
| BAR0 | 0xb81000 | W | 0xb81610 NV_VF_CPU_INTR_TOP_EN_CLEAR(0) | 1.50 |
| BAR2 | flush page | R |  | 1.50 |
| BAR0 | 0xb81000 | W | 0xb81208 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.50 |
| BAR0 | 0xb81000 | W | 0xb81210 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.50 |
| PCI config |  | R | 0x60 | 1.05 |
| PCI config |  | R | 0x68 | 1.05 |
| (rest) | | | | 13.57 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 1.0, intr 29.2

- shadow plans 0.4 per rep: 0 leaves mapped and 0 unmapped in 0.0 RM map call(s), walk 6 us, worker apply 93 us per rep
- events naming a host RM call 0.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 0.2; other events 0.4

## op 12 cuMemAlloc_64M: 20 repetition(s), 266.5 trapped accesses per repetition, window median 6930 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 179.6 | 6157 |
| vf-intr-tree | 47.5 | 1801 |
| pmc-intr-en (RM lock) | 30.0 | 1242 |
| mmu-invalidate | 5.5 | 317 |
| bar2-flush-read | 2.3 | 84 |
| pcfg-mirror (intr re-arm) | 1.6 | 51 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 179.60 |
| BAR0 | 0x000000 | W | 0x000180 latched | 15.00 |
| BAR0 | 0x000000 | W | 0x000160 latched | 15.00 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 9.00 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 3.40 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 3.40 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 3.30 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 3.30 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 3.30 |
| BAR0 | 0xb81000 | W | 0xb81408 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 3.00 |
| BAR0 | 0xb81000 | W | 0xb81410 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 3.00 |
| BAR0 | 0xb81000 | W | 0xb81610 NV_VF_CPU_INTR_TOP_EN_CLEAR(0) | 3.00 |
| (rest) | | | | 22.20 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 179.6, intr 77.5

- shadow plans 0.1 per rep: 2 leaves mapped and 0 unmapped in 1.6 RM map call(s), walk 2 us, worker apply 1927 us per rep
- events naming a host RM call 1.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x1.8
- device events per rep: guest MMU invalidates 1.1; host notifier events -> MSI 1.8; other events 0.1

## op 13 cuMemFree_64M: 20 repetition(s), 143.7 trapped accesses per repetition, window median 4396 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 64.1 | 2235 |
| vf-intr-tree | 35.6 | 1087 |
| pmc-intr-en (RM lock) | 30.0 | 1084 |
| mmu-invalidate | 10.0 | 346 |
| bar2-flush-read | 3.1 | 198 |
| pcfg-mirror (intr re-arm) | 0.8 | 22 |

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
| (rest) | | | | 16.45 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 64.1, intr 65.7

- shadow plans 0.3 per rep: 0 leaves mapped and 11 unmapped in 0.0 RM map call(s), walk 11 us, worker apply 2275 us per rep
- events naming a host RM call 0.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.8
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 0.8; other events 0.3

## op 14 cuMemHostAlloc_1M: 20 repetition(s), 902.7 trapped accesses per repetition, window median 23822 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 771.0 | 27074 |
| vf-intr-tree | 78.7 | 2724 |
| pci-config | 25.6 | 365 |
| pmc-intr-en (RM lock) | 15.0 | 901 |
| mmu-invalidate | 5.0 | 224 |
| pcfg-mirror (intr re-arm) | 4.8 | 153 |
| bar2-flush-read | 2.5 | 108 |
| pmc page 0, other | 0.1 | 15 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 771.00 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 9.55 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 9.55 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 9.55 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 9.50 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 9.50 |
| BAR0 | 0x000000 | W | 0x000180 latched | 7.50 |
| BAR0 | 0x000000 | W | 0x000160 latched | 7.50 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 4.80 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 4.75 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 4.75 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 4.75 |
| (rest) | | | | 50.00 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 771.0, intr 93.8

- shadow plans 0.1 per rep: 26 leaves mapped and 0 unmapped in 0.9 RM map call(s), walk 3 us, worker apply 36 us per rep
- events naming a host RM call 4.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x4.8
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 4.8; other events 0.1

## op 15 cuMemFreeHost_1M: 20 repetition(s), 313.2 trapped accesses per repetition, window median 8383 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 257.0 | 8312 |
| vf-intr-tree | 32.7 | 1063 |
| pmc-intr-en (RM lock) | 15.0 | 495 |
| mmu-invalidate | 5.0 | 162 |
| bar2-flush-read | 2.0 | 77 |
| pcfg-mirror (intr re-arm) | 1.5 | 43 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 257.00 |
| BAR0 | 0x000000 | W | 0x000180 latched | 7.50 |
| BAR0 | 0x000000 | W | 0x000160 latched | 7.50 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 4.50 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 2.95 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 2.95 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 2.95 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 2.95 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 2.90 |
| BAR2 | flush page | R |  | 2.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 2.00 |
| BAR0 | 0xb81000 | W | 0xb81408 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 1.50 |
| (rest) | | | | 16.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 257.0, intr 47.7

- shadow plans 0.2 per rep: 0 leaves mapped and 102 unmapped in 0.0 RM map call(s), walk 5 us, worker apply 69 us per rep
- events naming a host RM call 1.5 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x1.5
- device events per rep: guest MMU invalidates 1.0; host notifier events -> MSI 1.5; other events 0.2

## op 16 cuMemHostRegister_1M: 20 repetition(s), 1221.3 trapped accesses per repetition, window median 39124 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 1030.0 | 35382 |
| vf-intr-tree | 113.9 | 4531 |
| pmc-intr-en (RM lock) | 30.0 | 1325 |
| pci-config | 25.6 | 396 |
| mmu-invalidate | 10.0 | 507 |
| pcfg-mirror (intr re-arm) | 6.7 | 290 |
| bar2-flush-read | 5.0 | 193 |
| pmc page 0, other | 0.1 | 5 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 1030.00 |
| BAR0 | 0x000000 | W | 0x000180 latched | 15.00 |
| BAR0 | 0x000000 | W | 0x000160 latched | 15.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 13.00 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 12.70 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 12.70 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 12.65 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 12.65 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 9.00 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 6.65 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 6.65 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 6.60 |
| (rest) | | | | 68.70 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 1030.0, intr 144.0

- shadow plans 0.1 per rep: 26 leaves mapped and 26 unmapped in 0.5 RM map call(s), walk 22 us, worker apply 1792 us per rep
- events naming a host RM call 6.8 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x6.7; event: host notifier X fired (N record(s)) -> guest vector N, no MSI (disabled or no fd) [ x0.1
- device events per rep: guest MMU invalidates 2.0; host notifier events (no MSI) 0.1; host notifier events -> MSI 6.7; other events 0.1

## op 17 cuMemHostUnregister_1M: 20 repetition(s), 356.7 trapped accesses per repetition, window median 10536 us

| class | per rep | us per rep |
|---|---|---|
| bar2-write | 258.0 | 9068 |
| vf-intr-tree | 52.6 | 2446 |
| pmc-intr-en (RM lock) | 30.0 | 1024 |
| mmu-invalidate | 10.0 | 324 |
| bar2-flush-read | 4.0 | 154 |
| pcfg-mirror (intr re-arm) | 2.1 | 65 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR2 | other pages | W |  | 258.00 |
| BAR0 | 0x000000 | W | 0x000180 latched | 15.00 |
| BAR0 | 0x000000 | W | 0x000160 latched | 15.00 |
| BAR0 | 0xb81000 | W | 0xb81608 NV_VF_CPU_INTR_TOP_EN_SET(0) | 9.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 4.10 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 4.05 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 4.05 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 4.00 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 4.00 |
| BAR2 | flush page | R |  | 4.00 |
| BAR0 | 0xb83000 | R | 0xb830b0 NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 4.00 |
| BAR0 | 0xb81000 | W | 0xb81408 NV_VF_CPU_INTR_LEAF_EN_CLEAR(i) | 3.00 |
| (rest) | | | | 28.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar2memfd 258.0, intr 82.6

- shadow plans 0.8 per rep: 0 leaves mapped and 205 unmapped in 0.0 RM map call(s), walk 13 us, worker apply 226 us per rep
- events naming a host RM call 2.2 per rep: event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x2.2
- device events per rep: guest MMU invalidates 2.0; host notifier events -> MSI 2.2; other events 0.8

## op 20 launch_1_sync: 40 repetition(s), 9.4 trapped accesses per repetition, window median 854 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 3.1 | 188 |
| bar1-write | 3.0 | 208 |
| pci-config | 2.0 | 34 |
| usermode-doorbell | 1.0 | 70 |
| pcfg-mirror (intr re-arm) | 0.2 | 15 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.45 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.45 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.42 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.42 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.42 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.25 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.25 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.25 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.23 |
| (rest) | | | | 2.25 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 3.1, tokens 1.0

- events naming a host RM call 1.2 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.2
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.2

## op 21 launch_1000_sync: 2 repetition(s), 4552.5 trapped accesses per repetition, window median 276000 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3000.0 | 152937 |
| usermode-doorbell | 1000.0 | 35552 |
| vf-intr-tree | 515.0 | 37962 |
| pcfg-mirror (intr re-arm) | 37.5 | 2010 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 1984.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1016.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1000.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 74.00 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 73.00 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 73.00 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 73.00 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 73.00 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 37.50 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 37.50 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 37.50 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 37.50 |
| (rest) | | | | 36.50 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3000.0, intr 515.0, tokens 1000.0

- events naming a host RM call 1039.0 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1000.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x39.0
- device events per rep: host doorbells written (forwarded) 1000.0; host notifier events -> MSI 39.0

## op 30 h2d_pinned_1M: 20 repetition(s), 5.0 trapped accesses per repetition, window median 564 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 163 |
| usermode-doorbell | 1.0 | 49 |
| vf-intr-tree | 0.9 | 142 |
| pmc-intr-en (RM lock) | 0.1 | 4 |
| pcfg-mirror (intr re-arm) | 0.1 | 2 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.15 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| (rest) | | | | 0.30 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 1.0, tokens 1.0

- events naming a host RM call 1.1 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.1
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.1

## op 31 d2h_pinned_1M: 20 repetition(s), 5.0 trapped accesses per repetition, window median 506 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 124 |
| usermode-doorbell | 1.0 | 44 |
| vf-intr-tree | 0.9 | 35 |
| pcfg-mirror (intr re-arm) | 0.1 | 3 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.15 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| (rest) | | | | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 0.9, tokens 1.0

- events naming a host RM call 1.1 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.1
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.1

## op 32 d2d_1M: 20 repetition(s), 6.5 trapped accesses per repetition, window median 969 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 136 |
| vf-intr-tree | 2.3 | 90 |
| usermode-doorbell | 1.0 | 45 |
| pcfg-mirror (intr re-arm) | 0.2 | 7 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.30 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.30 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.30 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.30 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.30 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.25 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.25 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.25 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| (rest) | | | | 0.15 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 2.3, tokens 1.0

- events naming a host RM call 1.1 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.1
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.1

## op 33 h2d_pinned_16M: 20 repetition(s), 12.4 trapped accesses per repetition, window median 5312 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 7.8 | 387 |
| bar1-write | 3.0 | 140 |
| usermode-doorbell | 1.0 | 43 |
| pcfg-mirror (intr re-arm) | 0.6 | 16 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 1.10 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 1.10 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.10 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.10 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 1.05 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.55 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.55 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.55 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.55 |
| (rest) | | | | 0.75 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 7.8, tokens 1.0

- events naming a host RM call 1.6 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.6
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.6

## op 34 d2h_pinned_16M: 20 repetition(s), 15.8 trapped accesses per repetition, window median 5398 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 11.0 | 403 |
| bar1-write | 3.0 | 133 |
| usermode-doorbell | 1.0 | 49 |
| pcfg-mirror (intr re-arm) | 0.8 | 24 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 1.60 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 1.60 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.60 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 1.55 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 1.55 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.80 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.80 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.80 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.75 |
| (rest) | | | | 0.75 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 11.0, tokens 1.0

- events naming a host RM call 1.8 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.8
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.8

## op 35 d2d_16M: 20 repetition(s), 8.0 trapped accesses per repetition, window median 3862 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 3.7 | 151 |
| bar1-write | 3.0 | 218 |
| usermode-doorbell | 1.0 | 43 |
| pcfg-mirror (intr re-arm) | 0.2 | 9 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.55 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.55 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.50 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.50 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.50 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.30 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.30 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.25 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.25 |
| (rest) | | | | 0.25 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 3.7, tokens 1.0

- events naming a host RM call 1.3 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.3
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.3

## op 36 h2d_pageable_1M: 20 repetition(s), 6.7 trapped accesses per repetition, window median 652 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 220 |
| vf-intr-tree | 2.1 | 201 |
| usermode-doorbell | 1.0 | 57 |
| pmc-intr-en (RM lock) | 0.3 | 11 |
| pcfg-mirror (intr re-arm) | 0.2 | 7 |
| pmc page 0, other | 0.1 | 2 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.30 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.25 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.25 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.20 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.20 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.20 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.15 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.15 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.15 |
| (rest) | | | | 0.85 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 2.5, tokens 1.0

- events naming a host RM call 1.1 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.1
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.1

## op 37 d2h_pageable_1M: 20 repetition(s), 4.5 trapped accesses per repetition, window median 642 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 147 |
| usermode-doorbell | 1.0 | 42 |
| vf-intr-tree | 0.4 | 53 |
| pcfg-mirror (intr re-arm) | 0.1 | 4 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 1.90 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.10 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.10 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.10 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 0.4, tokens 1.0

- events naming a host RM call 1.1 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.1
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.1

## op 40 cuStreamSynchronize_idle: 40 repetition(s), 0.1 trapped accesses per repetition, window median 40 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 0.0 | 1 |
| pcfg-mirror (intr re-arm) | 0.0 | 1 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.03 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.03 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): intr 0.0


## op 41 cuEventRecord_Synchronize: 40 repetition(s), 5.6 trapped accesses per repetition, window median 808 us

| class | per rep | us per rep |
|---|---|---|
| bar1-write | 3.0 | 128 |
| vf-intr-tree | 1.4 | 90 |
| usermode-doorbell | 1.0 | 83 |
| pcfg-mirror (intr re-arm) | 0.1 | 4 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 1.95 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.05 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.23 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.20 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.20 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.20 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.20 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.12 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.12 |
| BAR0 | 0x088000 | W | 0x088704 NV_PCFG | 0.12 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.10 |
| (rest) | | | | 0.07 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 1.4, tokens 1.0

- events naming a host RM call 1.1 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.1
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.1

## op 50 cuModuleLoad_cubin: 20 repetition(s), 8.4 trapped accesses per repetition, window median 944 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 4.2 | 176 |
| bar1-write | 3.0 | 204 |
| usermode-doorbell | 1.0 | 45 |
| pcfg-mirror (intr re-arm) | 0.2 | 7 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR1 | [0, 32 MiB) | W |  | 2.00 |
| BAR1 | [0, 32 MiB) | W | USERD GPPut | 1.00 |
| BAR0 | 0xbb0000 | W | 0xbb0090 NV_VIRTUAL_FUNCTION_DOORBELL | 1.00 |
| BAR0 | 0xb81000 | R | 0xb81004 NV_VF_CPU_INTR_LEAF(i) | 0.60 |
| BAR0 | 0xb81000 | R | 0xb81204 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.60 |
| BAR0 | 0xb81000 | R | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.55 |
| BAR0 | 0xb81000 | R | 0xb81200 NV_VF_CPU_INTR_LEAF_EN_SET(i) | 0.55 |
| BAR0 | 0xb81000 | R | 0xb81600 NV_VF_CPU_INTR_TOP(0) | 0.50 |
| BAR0 | 0xb81000 | W | 0xb81000 NV_VF_CPU_INTR_LEAF(i) | 0.35 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.35 |
| BAR0 | 0xb81000 | R | 0xb81008 NV_VF_CPU_INTR_LEAF(i) | 0.35 |
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.30 |
| (rest) | | | | 0.25 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): bar1pool 3.0, intr 4.2, tokens 1.0

- events naming a host RM call 1.4 per rep: doorbell: guest chid N -> host token X written to the host doorbell page (N on this channe x1.0; event: host notifier X fired (N record(s)) -> guest vector N, MSI [host] x0.3
- device events per rep: host doorbells written (forwarded) 1.0; host notifier events -> MSI 0.3

## op 51 cuModuleUnload: 20 repetition(s), 0.1 trapped accesses per repetition, window median 59 us

| class | per rep | us per rep |
|---|---|---|
| vf-intr-tree | 0.1 | 2 |

| BAR | page | R/W | register | per rep |
|---|---|---|---|---|
| BAR0 | 0xb81000 | R | 0xb81018 NV_VF_CPU_INTR_LEAF(i) | 0.05 |
| BAR0 | 0xb81000 | R | 0xb81010 NV_VF_CPU_INTR_LEAF(i) | 0.05 |

On pages an existing window serves (trapped here, so that window was off or the client did not map it): intr 0.1



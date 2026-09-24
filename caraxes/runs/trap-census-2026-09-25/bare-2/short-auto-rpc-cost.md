# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/bare-2/short-auto-trace.txt

Window 271508762 .. 277396959 us (5888.2 ms), 121635 trapped accesses, 3992.4 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 96199 | 3087370 | 77.3 % | 29.0 | 37.0 |
| vf-intr-tree | 14786 | 501421 | 12.6 % | 28.0 | 39.0 |
| pmc-intr-en (RM lock) | 5442 | 202938 | 5.1 % | 28.0 | 49.0 |
| bar1-write | 978 | 48331 | 1.2 % | 31.0 | 55.0 |
| mmu-invalidate | 830 | 30055 | 0.8 % | 30.0 | 44.0 |
| pcfg-mirror (intr re-arm) | 750 | 26825 | 0.7 % | 29.0 | 42.0 |
| gsp-queue-doorbell | 187 | 26292 | 0.7 % | 54.0 | 315.0 |
| gsp-falcon | 561 | 19878 | 0.5 % | 32.0 | 50.0 |
| pci-config | 1026 | 18169 | 0.5 % | 15.0 | 26.0 |
| bar2-flush-read | 477 | 17645 | 0.4 % | 31.0 | 48.0 |
| usermode-doorbell | 326 | 9777 | 0.2 % | 30.0 | 38.0 |
| usermode page, other | 60 | 2085 | 0.1 % | 31.0 | 47.0 |
| bar0-other | 8 | 1450 | 0.0 % | 121.5 | 350.0 |
| pmc page 0, other | 5 | 182 | 0.0 % | 37.0 | 48.0 |
| all | 121635 | 3992418 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 4 | 13 | 24 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 8 | 73 | 0 |
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 6 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 8 | 9 | 9 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 8 | 13 | 13 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 8 | 11 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 2 | 6 | 9 | 9 | 2 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 10 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 4 | 4 | 4 | 1 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 89 |
| 0xa9600000 | 0x5 | 73 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

36 plan(s) applied by the worker: 3202 leaves unmapped, 19620 mapped in 17790 RM map call(s); walk median 13 us (server thread), apply median 402 us p90 14277 us total 673.6 ms (worker), queued median 12 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 10 | 8 | 0 | 0 | 15 | 801 | 670 |
| 2 | 2 | 5 | 20 | 16 | 0 | 0 | 3 | 1377 | 1294 |
| 4 | 2 | 5 | 30 | 24 | 0 | 0 | 2 | 1957 | 1812 |
| 4 | 2 | 5 | 30 | 38 | 1 | 1 | 2 | 4458 | 3467 |
| 0 | 1 | 5 | 10 | 17 | 0 | 1 | 2 | 1040 | 1005 |
| 2 | 2 | 5 | 30 | 24 | 0 | 4 | 2 | 2467 | 2425 |
| 514 | 3 | 5 | 20 | 58 | 3 | 3 | 2 | 19928 | 19260 |
| 2 | 2 | 5 | 30 | 38 | 1 | 1 | 2 | 4672 | 3257 |
| 1036 | 8 | 15 | 30 | 94 | 5 | 5 | 1 | 35446 | 35446 |
| 4 | 2 | 5 | 20 | 30 | 1 | 1 | 1 | 1791 | 1791 |
| 132 | 8 | 15 | 80 | 78 | 1 | 17 | 1 | 9521 | 9521 |
| 43176 | 113 | 145 | 50 | 3366 | 239 | 752 | 1 | 1590704 | 1590704 |
| 1070 | 10 | 20 | 60 | 132 | 6 | 11 | 1 | 39211 | 39211 |
| 1009 | 6 | 10 | 0 | 56 | 4 | 4 | 1 | 32229 | 32229 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 121635 | 3992418 |
| + CARAXES_INTR_PAGE=1 | 101402 | 3287877 |
| + CARAXES_BAR2_MEMFD=1 | 25436 | 905048 |
| + both | 5203 | 200507 |

Median per access by class, us: bar0-other 121.5, bar1-write 31.0, bar2-flush-read 31.0, bar2-write 29.0, gsp-falcon 32.0, gsp-queue-doorbell 54.0, mmu-invalidate 30.0, pcfg-mirror (intr re-arm) 29.0, pci-config 15.0, pmc page 0, other 37.0, pmc-intr-en (RM lock) 28.0, usermode page, other 31.0, usermode-doorbell 30.0, vf-intr-tree 28.0


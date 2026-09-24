# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/bare-2/rl-trace.txt

Window 193745490 .. 227356834 us (33611.3 ms), 543773 trapped accesses, 22617.9 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar1-write | 263754 | 12595038 | 55.7 % | 32.0 | 77.0 |
| bar2-write | 100641 | 3282881 | 14.5 % | 28.0 | 38.0 |
| usermode-doorbell | 87918 | 3187156 | 14.1 % | 31.0 | 43.0 |
| vf-intr-tree | 70776 | 2897376 | 12.8 % | 29.0 | 42.0 |
| pmc-intr-en (RM lock) | 5864 | 213530 | 0.9 % | 28.0 | 46.0 |
| pcfg-mirror (intr re-arm) | 4764 | 181655 | 0.8 % | 31.0 | 40.0 |
| pci-config | 7695 | 148010 | 0.7 % | 15.0 | 30.0 |
| mmu-invalidate | 960 | 33469 | 0.1 % | 29.0 | 50.0 |
| gsp-queue-doorbell | 194 | 31950 | 0.1 % | 57.0 | 378.0 |
| gsp-falcon | 582 | 22651 | 0.1 % | 31.0 | 51.0 |
| bar2-flush-read | 525 | 19537 | 0.1 % | 32.0 | 51.0 |
| bar0-other | 8 | 1865 | 0.0 % | 111.5 | 487.0 |
| usermode page, other | 60 | 1722 | 0.0 % | 26.0 | 37.0 |
| pmc page 0, other | 32 | 1049 | 0.0 % | 29.0 | 42.0 |
| all | 543773 | 22617889 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 4 | 17 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 5 | 6 | 8 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 4 | 61 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 3 | 4 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 6 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 4 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 7 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 5 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 5 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 113 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

29 plan(s) applied by the worker: 29 leaves unmapped, 20447 mapped in 17046 RM map call(s); walk median 32 us (server thread), apply median 588 us p90 10101 us total 604.2 ms (worker), queued median 12 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 10 | 8 | 0 | 0 | 16 | 616 | 593 |
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 13 | 201 | 162 |
| 2 | 2 | 5 | 10 | 8 | 0 | 0 | 12 | 888 | 803 |
| 4 | 2 | 5 | 30 | 38 | 1 | 1 | 3 | 2884 | 2637 |
| 514 | 3 | 5 | 10 | 36 | 2 | 2 | 3 | 22298 | 15245 |
| 514 | 3 | 5 | 10 | 64 | 4 | 4 | 3 | 26529 | 26276 |
| 4 | 2 | 5 | 30 | 38 | 1 | 13 | 2 | 6538 | 5443 |
| 0 | 1 | 5 | 10 | 19 | 1 | 0 | 2 | 1290 | 1168 |
| 514 | 3 | 5 | 10 | 50 | 3 | 3 | 2 | 19948 | 18027 |
| 1036 | 8 | 15 | 30 | 80 | 4 | 4 | 1 | 32416 | 32416 |
| 4 | 2 | 5 | 20 | 30 | 1 | 1 | 1 | 1615 | 1615 |
| 4 | 2 | 5 | 30 | 24 | 0 | 0 | 1 | 2245 | 2245 |
| 132 | 8 | 15 | 80 | 78 | 1 | 17 | 1 | 9553 | 9553 |
| 43176 | 113 | 145 | 50 | 3048 | 217 | 730 | 1 | 1472209 | 1472209 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 543773 | 22617889 |
| + CARAXES_INTR_PAGE=1 | 467101 | 19505934 |
| + CARAXES_BAR2_MEMFD=1 | 443132 | 19335008 |
| + both | 366460 | 16223053 |

Median per access by class, us: bar0-other 111.5, bar1-write 32.0, bar2-flush-read 32.0, bar2-write 28.0, gsp-falcon 31.0, gsp-queue-doorbell 57.0, mmu-invalidate 29.0, pcfg-mirror (intr re-arm) 31.0, pci-config 15.0, pmc page 0, other 29.0, pmc-intr-en (RM lock) 28.0, usermode page, other 26.0, usermode-doorbell 31.0, vf-intr-tree 29.0


# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/bare/rl-trace.txt

Window 208704931 .. 242224141 us (33519.2 ms), 542323 trapped accesses, 22146.3 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar1-write | 263754 | 12602852 | 56.9 % | 32.0 | 78.0 |
| usermode-doorbell | 87918 | 3182088 | 14.4 % | 31.0 | 42.0 |
| bar2-write | 100641 | 3131872 | 14.1 % | 28.0 | 36.0 |
| vf-intr-tree | 69424 | 2620273 | 11.8 % | 29.0 | 40.0 |
| pmc-intr-en (RM lock) | 5870 | 212918 | 1.0 % | 27.0 | 44.0 |
| pcfg-mirror (intr re-arm) | 4659 | 161664 | 0.7 % | 30.0 | 39.0 |
| pci-config | 7695 | 136484 | 0.6 % | 14.0 | 26.0 |
| mmu-invalidate | 960 | 30001 | 0.1 % | 28.0 | 40.0 |
| gsp-falcon | 582 | 21516 | 0.1 % | 29.0 | 46.0 |
| gsp-queue-doorbell | 194 | 21389 | 0.1 % | 54.0 | 207.0 |
| bar2-flush-read | 525 | 19837 | 0.1 % | 31.0 | 47.0 |
| pmc page 0, other | 33 | 2580 | 0.0 % | 28.0 | 36.0 |
| usermode page, other | 60 | 1828 | 0.0 % | 28.5 | 44.0 |
| bar0-other | 8 | 1012 | 0.0 % | 67.0 | 243.0 |
| all | 542323 | 22146314 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 3 | 12 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 5 | 9 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 7 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 31 | 31 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 4 | 7 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 2 | 6 | 6 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 5 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 113 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

29 plan(s) applied by the worker: 29 leaves unmapped, 20447 mapped in 17367 RM map call(s); walk median 31 us (server thread), apply median 501 us p90 12822 us total 604.6 ms (worker), queued median 12 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 10 | 8 | 0 | 0 | 20 | 628 | 585 |
| 2 | 2 | 5 | 10 | 8 | 0 | 0 | 12 | 797 | 766 |
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 11 | 196 | 166 |
| 514 | 3 | 5 | 10 | 36 | 2 | 2 | 3 | 15549 | 15346 |
| 514 | 3 | 5 | 10 | 50 | 3 | 3 | 3 | 17163 | 16638 |
| 4 | 2 | 5 | 30 | 38 | 1 | 1 | 2 | 2334 | 2313 |
| 1542 | 5 | 10 | 30 | 122 | 7 | 7 | 2 | 47029 | 46729 |
| 514 | 3 | 5 | 10 | 64 | 4 | 4 | 2 | 20148 | 19393 |
| 1036 | 8 | 15 | 30 | 94 | 5 | 5 | 1 | 34494 | 34494 |
| 4 | 2 | 5 | 20 | 16 | 0 | 0 | 1 | 1309 | 1309 |
| 132 | 8 | 15 | 80 | 78 | 1 | 17 | 1 | 9502 | 9502 |
| 43176 | 113 | 145 | 50 | 3190 | 230 | 743 | 1 | 1525201 | 1525201 |
| 1070 | 10 | 20 | 60 | 132 | 6 | 11 | 1 | 43953 | 43953 |
| 1009 | 6 | 10 | 0 | 70 | 5 | 5 | 1 | 33112 | 33112 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 542323 | 22146314 |
| + CARAXES_INTR_PAGE=1 | 466996 | 19310543 |
| + CARAXES_BAR2_MEMFD=1 | 441682 | 19014442 |
| + both | 366355 | 16178671 |

Median per access by class, us: bar0-other 67.0, bar1-write 32.0, bar2-flush-read 31.0, bar2-write 28.0, gsp-falcon 29.0, gsp-queue-doorbell 54.0, mmu-invalidate 28.0, pcfg-mirror (intr re-arm) 30.0, pci-config 14.0, pmc page 0, other 28.0, pmc-intr-en (RM lock) 27.0, usermode page, other 28.5, usermode-doorbell 31.0, vf-intr-tree 29.0


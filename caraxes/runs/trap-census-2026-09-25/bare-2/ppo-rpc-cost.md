# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/bare-2/ppo-trace.txt

Window 228814381 .. 269514179 us (40699.8 ms), 789898 trapped accesses, 34931.4 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar1-write | 431745 | 21255676 | 60.8 % | 33.0 | 77.0 |
| usermode-doorbell | 143915 | 5323648 | 15.2 % | 31.0 | 43.0 |
| vf-intr-tree | 89692 | 3939200 | 11.3 % | 30.0 | 43.0 |
| bar2-write | 100713 | 3651903 | 10.5 % | 29.0 | 43.0 |
| pmc-intr-en (RM lock) | 6056 | 252466 | 0.7 % | 29.0 | 56.0 |
| pcfg-mirror (intr re-arm) | 6114 | 234904 | 0.7 % | 31.0 | 41.0 |
| pci-config | 9234 | 159455 | 0.5 % | 15.0 | 25.0 |
| mmu-invalidate | 1005 | 37012 | 0.1 % | 30.0 | 47.0 |
| gsp-queue-doorbell | 194 | 25560 | 0.1 % | 61.5 | 261.0 |
| gsp-falcon | 582 | 24221 | 0.1 % | 33.0 | 56.0 |
| bar2-flush-read | 540 | 22167 | 0.1 % | 32.0 | 53.0 |
| usermode page, other | 60 | 2368 | 0.0 % | 30.0 | 55.0 |
| bar0-other | 8 | 1419 | 0.0 % | 124.0 | 290.0 |
| pmc page 0, other | 40 | 1417 | 0.0 % | 29.0 | 54.0 |
| all | 789898 | 34931416 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| GSP_RM_ALLOC class 0xc46f | 20 | 5 | 10 | 173 | 0 |
| FREE | 71 | 3 | 3 | 9 | 24 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 6 | 95 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 4 | 9 | 68 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 4 | 8 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 4 | 10 | 10 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 4 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 6 | 6 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 2 | 2 | 2 | 1 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 122 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

32 plan(s) applied by the worker: 29 leaves unmapped, 20459 mapped in 1281 RM map call(s); walk median 28 us (server thread), apply median 410 us p90 5090 us total 64.1 ms (worker), queued median 10 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 10 | 8 | 0 | 0 | 19 | 668 | 594 |
| 2 | 2 | 5 | 10 | 8 | 0 | 0 | 13 | 702 | 675 |
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 11 | 178 | 161 |
| 514 | 3 | 5 | 10 | 36 | 2 | 2 | 6 | 15558 | 14724 |
| 4 | 2 | 5 | 30 | 24 | 0 | 0 | 4 | 2286 | 1996 |
| 514 | 3 | 5 | 10 | 50 | 3 | 3 | 4 | 17207 | 15536 |
| 1542 | 5 | 10 | 30 | 122 | 7 | 7 | 3 | 48796 | 46925 |
| 20 | 2 | 5 | 10 | 8 | 0 | 0 | 2 | 1273 | 1233 |
| 8 | 2 | 5 | 10 | 8 | 0 | 0 | 2 | 970 | 961 |
| 0 | 1 | 5 | 0 | 5 | 1 | 0 | 2 | 322 | 315 |
| 0 | 1 | 5 | 10 | 17 | 0 | 1 | 2 | 858 | 829 |
| 1036 | 8 | 15 | 30 | 165 | 11 | 10 | 1 | 69140 | 69140 |
| 4 | 2 | 5 | 20 | 19 | 0 | 2 | 1 | 2026 | 2026 |
| 132 | 8 | 15 | 80 | 106 | 3 | 19 | 1 | 20460 | 20460 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 789898 | 34931416 |
| + CARAXES_INTR_PAGE=1 | 694110 | 30738333 |
| + CARAXES_BAR2_MEMFD=1 | 689185 | 31279513 |
| + both | 593397 | 27086430 |

Median per access by class, us: bar0-other 124.0, bar1-write 33.0, bar2-flush-read 32.0, bar2-write 29.0, gsp-falcon 33.0, gsp-queue-doorbell 61.5, mmu-invalidate 30.0, pcfg-mirror (intr re-arm) 31.0, pci-config 15.0, pmc page 0, other 29.0, pmc-intr-en (RM lock) 29.0, usermode page, other 30.0, usermode-doorbell 31.0, vf-intr-tree 30.0


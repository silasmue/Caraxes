# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/on-nopool/rl-trace.txt

Window 172129223 .. 198851247 us (26722.0 ms), 275946 trapped accesses, 14880.7 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar1-write | 263754 | 14080808 | 94.6 % | 35.0 | 86.0 |
| pcfg-mirror (intr re-arm) | 3687 | 435958 | 2.9 % | 62.0 | 482.0 |
| bar2-flush-read | 525 | 148459 | 1.0 % | 90.0 | 1079.0 |
| pci-config | 6156 | 110839 | 0.7 % | 14.0 | 28.0 |
| mmu-invalidate | 960 | 46878 | 0.3 % | 34.0 | 75.0 |
| gsp-queue-doorbell | 194 | 29752 | 0.2 % | 109.0 | 326.0 |
| gsp-falcon | 582 | 23630 | 0.2 % | 34.0 | 57.0 |
| usermode page, other | 60 | 2684 | 0.0 % | 35.0 | 71.0 |
| bar0-other | 8 | 1738 | 0.0 % | 143.0 | 394.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 275946 | 14880746 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 3 | 8 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 9 | 10 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 8 | 12 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 6 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 6 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 7 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 4 | 4 | 4 | 1 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 113 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

29 plan(s) applied by the worker: 29 leaves unmapped, 20447 mapped in 17146 RM map call(s); walk median 18 us (server thread), apply median 368 us p90 10603 us total 525.8 ms (worker), queued median 13 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 33 | 369 | 215 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 24 | 326 | 290 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 12 | 1412 | 644 |
| 0 | 5 | 10 | 0 | 0 | 1 | 1 | 3 | 5745 | 3031 |
| 0 | 2 | 5 | 0 | 0 | 0 | 1 | 2 | 780 | 471 |
| 0 | 3 | 5 | 0 | 0 | 1 | 1 | 2 | 962 | 485 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 2 | 534 | 480 |
| 0 | 8 | 15 | 0 | 0 | 1 | 0 | 1 | 1378 | 1378 |
| 0 | 8 | 15 | 0 | 0 | 0 | 16 | 1 | 3005 | 3005 |
| 0 | 113 | 145 | 0 | 0 | 9 | 9 | 1 | 55508 | 55508 |
| 0 | 10 | 20 | 0 | 0 | 0 | 4 | 1 | 2326 | 2326 |
| 0 | 6 | 10 | 0 | 0 | 1 | 1 | 1 | 1752 | 1752 |
| 0 | 5 | 10 | 0 | 0 | 1 | 24 | 1 | 4478 | 4478 |
| 0 | 43 | 105 | 0 | 0 | 4 | 310 | 1 | 30451 | 30451 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 275946 | 14880746 |
| + CARAXES_INTR_PAGE=1 | 275946 | 14880746 |
| + CARAXES_BAR2_MEMFD=1 | 275946 | 14880746 |
| + both | 275946 | 14880746 |

Median per access by class, us: bar0-other 143.0, bar1-write 35.0, bar2-flush-read 90.0, gsp-falcon 34.0, gsp-queue-doorbell 109.0, mmu-invalidate 34.0, pcfg-mirror (intr re-arm) 62.0, pci-config 14.0, usermode page, other 35.0


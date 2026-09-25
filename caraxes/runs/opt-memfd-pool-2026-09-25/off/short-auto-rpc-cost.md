# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a502fb60566dbcfc2/caraxes/runs/opt-memfd-pool-2026-09-25/off/short-auto-trace.txt

Window 215856454 .. 221760980 us (5904.5 ms), 100231 trapped accesses, 4048.1 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 96199 | 3797567 | 93.8 % | 28.0 | 59.0 |
| pcfg-mirror (intr re-arm) | 863 | 90451 | 2.2 % | 39.0 | 233.0 |
| mmu-invalidate | 830 | 44702 | 1.1 % | 33.0 | 65.0 |
| gsp-queue-doorbell | 187 | 43253 | 1.1 % | 110.0 | 561.0 |
| gsp-falcon | 561 | 26130 | 0.6 % | 33.0 | 62.0 |
| bar2-flush-read | 477 | 25219 | 0.6 % | 36.0 | 76.0 |
| pci-config | 1026 | 15652 | 0.4 % | 13.0 | 24.0 |
| usermode page, other | 60 | 2609 | 0.1 % | 36.5 | 71.0 |
| bar0-other | 8 | 2488 | 0.1 % | 268.0 | 629.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 100231 | 4048071 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 3 | 9 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 5 | 8 | 11 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 6 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 6 | 9 | 9 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 10 | 10 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 7 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 2 | 2 | 2 | 2 | 2 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 2 | 2 | 2 | 1 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 89 |
| 0xa9600000 | 0x5 | 73 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

38 plan(s) applied by the worker: 18050 leaves unmapped, 20131 mapped in 859 RM map call(s); walk median 12 us (server thread), apply median 590 us p90 10371 us total 97.6 ms (worker), queued median 11 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 25 | 237 | 190 |
| 2 | 2 | 5 | 0 | 0 | 0 | 0 | 9 | 496 | 297 |
| 4 | 2 | 5 | 0 | 0 | 0 | 0 | 3 | 579 | 512 |
| 4 | 2 | 5 | 0 | 0 | 1 | 1 | 3 | 6320 | 2158 |
| 514 | 3 | 5 | 0 | 0 | 3 | 3 | 2 | 16348 | 15911 |
| 1036 | 8 | 15 | 0 | 0 | 9 | 9 | 1 | 58921 | 58921 |
| 132 | 8 | 15 | 0 | 0 | 5 | 20 | 1 | 34616 | 34616 |
| 43176 | 113 | 145 | 0 | 0 | 257 | 766 | 1 | 1712372 | 1712372 |
| 1070 | 10 | 20 | 0 | 0 | 5 | 9 | 1 | 33911 | 33911 |
| 1009 | 6 | 10 | 0 | 0 | 5 | 5 | 1 | 29617 | 29617 |
| 12 | 3 | 5 | 0 | 0 | 0 | 0 | 1 | 532 | 532 |
| 144 | 3 | 5 | 0 | 0 | 0 | 0 | 1 | 4005 | 4005 |
| 1542 | 5 | 10 | 0 | 0 | 10 | 33 | 1 | 55297 | 55297 |
| 11308 | 43 | 105 | 0 | 0 | 84 | 391 | 1 | 545572 | 545572 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 100231 | 4048071 |
| + CARAXES_INTR_PAGE=1 | 100231 | 4048071 |
| + CARAXES_BAR2_MEMFD=1 | 4032 | 250504 |
| + both | 4032 | 250504 |

Median per access by class, us: bar0-other 268.0, bar2-flush-read 36.0, bar2-write 28.0, gsp-falcon 33.0, gsp-queue-doorbell 110.0, mmu-invalidate 33.0, pcfg-mirror (intr re-arm) 39.0, pci-config 13.0, usermode page, other 36.5


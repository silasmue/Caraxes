# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/off/rl-trace.txt

Window 220448999 .. 240187368 us (19738.4 ms), 110410 trapped accesses, 4425.0 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 100641 | 4038475 | 91.3 % | 30.0 | 44.0 |
| pcfg-mirror (intr re-arm) | 2803 | 137187 | 3.1 % | 41.0 | 249.0 |
| pci-config | 4617 | 101231 | 2.3 % | 16.0 | 33.0 |
| mmu-invalidate | 960 | 46363 | 1.0 % | 33.0 | 62.0 |
| gsp-queue-doorbell | 194 | 38459 | 0.9 % | 109.0 | 438.0 |
| gsp-falcon | 582 | 32389 | 0.7 % | 35.0 | 84.0 |
| bar2-flush-read | 525 | 23598 | 0.5 % | 36.0 | 69.0 |
| usermode page, other | 60 | 3798 | 0.1 % | 37.5 | 114.0 |
| usermode-doorbell | 20 | 1908 | 0.0 % | 1908.0 | 1908.0 |
| bar0-other | 8 | 1569 | 0.0 % | 120.0 | 355.0 |
| all | 110410 | 4424977 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 5 | 22 | 24 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 18 | 85 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 2 | 172 | 172 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 9 | 122 | 0 |
| GSP_RM_ALLOC class 0xc46f | 20 | 5 | 10 | 15 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 4 | 9 | 90 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 6 | 12 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 12 | 12 | 12 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 10 | 18 | 18 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 113 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

56 plan(s) applied by the worker: 1052 leaves unmapped, 21470 mapped in 17341 RM map call(s); walk median 334 us (server thread), apply median 2370 us p90 8989 us total 936.6 ms (worker), queued median 13 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 31 | 216 | 169 |
| 2 | 2 | 5 | 0 | 0 | 0 | 0 | 14 | 323 | 295 |
| 4 | 2 | 5 | 0 | 0 | 1 | 1 | 4 | 2957 | 1158 |
| 514 | 3 | 5 | 0 | 0 | 3 | 3 | 4 | 20552 | 18877 |
| 4 | 2 | 5 | 0 | 0 | 0 | 0 | 3 | 718 | 476 |
| 1542 | 5 | 10 | 0 | 0 | 10 | 10 | 2 | 61910 | 60682 |
| 514 | 3 | 5 | 0 | 0 | 2 | 2 | 2 | 16990 | 16831 |
| 514 | 3 | 5 | 0 | 0 | 4 | 4 | 2 | 27632 | 27376 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 2 | 286 | 224 |
| 1036 | 8 | 15 | 0 | 0 | 5 | 5 | 1 | 37033 | 37033 |
| 132 | 8 | 15 | 0 | 0 | 6 | 22 | 1 | 41192 | 41192 |
| 43176 | 113 | 145 | 0 | 0 | 282 | 795 | 1 | 1911305 | 1911305 |
| 1070 | 10 | 20 | 0 | 0 | 5 | 9 | 1 | 38577 | 38577 |
| 1009 | 6 | 10 | 0 | 0 | 9 | 9 | 1 | 63027 | 63027 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 110410 | 4424977 |
| + CARAXES_INTR_PAGE=1 | 110410 | 4424977 |
| + CARAXES_BAR2_MEMFD=1 | 9769 | 386502 |
| + both | 9769 | 386502 |

Median per access by class, us: bar0-other 120.0, bar2-flush-read 36.0, bar2-write 30.0, gsp-falcon 35.0, gsp-queue-doorbell 109.0, mmu-invalidate 33.0, pcfg-mirror (intr re-arm) 41.0, pci-config 16.0, usermode page, other 37.5, usermode-doorbell 1908.0


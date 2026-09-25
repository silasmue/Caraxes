# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a502fb60566dbcfc2/caraxes/runs/opt-memfd-pool-2026-09-25/on/rl-trace.txt

Window 172920925 .. 189103970 us (16183.0 ms), 8215 trapped accesses, 467.0 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 525 | 139085 | 29.8 % | 94.0 | 1005.0 |
| pcfg-mirror (intr re-arm) | 2275 | 106404 | 22.8 % | 100.0 | 1710.0 |
| pci-config | 3591 | 81122 | 17.4 % | 16.0 | 35.0 |
| mmu-invalidate | 960 | 59033 | 12.6 % | 37.0 | 79.0 |
| gsp-queue-doorbell | 194 | 39106 | 8.4 % | 118.0 | 441.0 |
| gsp-falcon | 582 | 38196 | 8.2 % | 36.0 | 80.0 |
| usermode page, other | 60 | 2616 | 0.6 % | 34.5 | 67.0 |
| bar0-other | 8 | 1460 | 0.3 % | 165.0 | 251.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 8215 | 467022 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| GSP_RM_ALLOC class 0x83de | 1 | 1724 | 1724 | 1724 | 0 |
| FREE | 71 | 3 | 5 | 44 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 10 | 19 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 38 | 75 | 75 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 8 | 9 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 3 | 8 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 9 | 9 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 8 | 13 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 2 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 11 | 11 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 4 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 6 | 6 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 113 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

50 plan(s) applied by the worker: 1052 leaves unmapped, 21470 mapped in 17015 RM map call(s); walk median 64 us (server thread), apply median 2536 us p90 11640 us total 968.0 ms (worker), queued median 14 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 31 | 284 | 222 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 23 | 390 | 327 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 10 | 1368 | 394 |
| 0 | 5 | 10 | 0 | 0 | 1 | 1 | 4 | 5099 | 4334 |
| 0 | 2 | 5 | 0 | 0 | 1 | 1 | 3 | 2946 | 937 |
| 0 | 3 | 5 | 0 | 0 | 1 | 1 | 3 | 1403 | 479 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 3 | 3357 | 1094 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 2 | 1512 | 1445 |
| 0 | 8 | 15 | 0 | 0 | 0 | 0 | 1 | 1217 | 1217 |
| 0 | 8 | 15 | 0 | 0 | 5 | 19 | 1 | 29795 | 29795 |
| 0 | 113 | 145 | 0 | 0 | 8 | 8 | 1 | 58175 | 58175 |
| 0 | 10 | 20 | 0 | 0 | 0 | 4 | 1 | 1992 | 1992 |
| 0 | 6 | 10 | 0 | 0 | 0 | 0 | 1 | 1573 | 1573 |
| 0 | 5 | 10 | 0 | 0 | 0 | 23 | 1 | 4039 | 4039 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 8215 | 467022 |
| + CARAXES_INTR_PAGE=1 | 8215 | 467022 |
| + CARAXES_BAR2_MEMFD=1 | 8215 | 467022 |
| + both | 8215 | 467022 |

Median per access by class, us: bar0-other 165.0, bar2-flush-read 94.0, gsp-falcon 36.0, gsp-queue-doorbell 118.0, mmu-invalidate 37.0, pcfg-mirror (intr re-arm) 100.0, pci-config 16.0, usermode page, other 34.5


# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/off-2/short-auto-trace.txt

Window 263364322 .. 270316375 us (6952.1 ms), 99132 trapped accesses, 3311.0 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 96199 | 3138818 | 94.8 % | 27.0 | 35.0 |
| pcfg-mirror (intr re-arm) | 790 | 59900 | 1.8 % | 39.0 | 92.0 |
| gsp-queue-doorbell | 187 | 33456 | 1.0 % | 92.0 | 352.0 |
| mmu-invalidate | 830 | 32341 | 1.0 % | 30.0 | 52.0 |
| gsp-falcon | 561 | 22195 | 0.7 % | 32.0 | 59.0 |
| bar2-flush-read | 477 | 20241 | 0.6 % | 31.0 | 68.0 |
| usermode page, other | 60 | 2560 | 0.1 % | 31.5 | 65.0 |
| bar0-other | 8 | 1505 | 0.0 % | 121.5 | 458.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 99132 | 3311016 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 3 | 13 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 5 | 8 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 7 | 8 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 3 | 14 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 6 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 2 | 2 | 3 | 3 | 2 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 2 | 2 | 2 | 1 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 89 |
| 0xa9600000 | 0x5 | 73 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

38 plan(s) applied by the worker: 18050 leaves unmapped, 20131 mapped in 17625 RM map call(s); walk median 16 us (server thread), apply median 1590 us p90 24332 us total 2257.7 ms (worker), queued median 14 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 25 | 228 | 178 |
| 2 | 2 | 5 | 0 | 0 | 0 | 0 | 7 | 441 | 314 |
| 4 | 2 | 5 | 0 | 0 | 0 | 0 | 5 | 545 | 429 |
| 514 | 3 | 5 | 0 | 0 | 3 | 3 | 3 | 17968 | 17543 |
| 1542 | 5 | 10 | 0 | 0 | 8 | 8 | 2 | 49656 | 46811 |
| 1542 | 5 | 10 | 0 | 0 | 7 | 7 | 2 | 46856 | 46121 |
| 2 | 2 | 5 | 0 | 0 | 0 | 4 | 2 | 695 | 519 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 2 | 1594 | 291 |
| 2 | 2 | 5 | 0 | 0 | 1 | 1 | 2 | 2376 | 464 |
| 1036 | 8 | 15 | 0 | 0 | 6 | 6 | 1 | 37679 | 37679 |
| 132 | 8 | 15 | 0 | 0 | 3 | 19 | 1 | 22093 | 22093 |
| 43176 | 113 | 145 | 0 | 0 | 255 | 256 | 1 | 1541787 | 1541787 |
| 1070 | 10 | 20 | 0 | 0 | 12 | 14 | 1 | 60802 | 60802 |
| 1009 | 6 | 10 | 0 | 0 | 4 | 4 | 1 | 33301 | 33301 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 99132 | 3311016 |
| + CARAXES_INTR_PAGE=1 | 99132 | 3311016 |
| + CARAXES_BAR2_MEMFD=1 | 2933 | 172198 |
| + both | 2933 | 172198 |

Median per access by class, us: bar0-other 121.5, bar2-flush-read 31.0, bar2-write 27.0, gsp-falcon 32.0, gsp-queue-doorbell 92.0, mmu-invalidate 30.0, pcfg-mirror (intr re-arm) 39.0, usermode page, other 31.5


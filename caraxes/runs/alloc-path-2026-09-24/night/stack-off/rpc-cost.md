# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a25d59d08c801f257/caraxes/runs/alloc-path-2026-09-24/night/stack-off/churn-trace.txt

Window 139441884 .. 141685303 us (2243.4 ms), 4411 trapped accesses, 376.7 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 682 | 156432 | 41.5 % | 90.0 | 962.0 |
| mmu-invalidate | 1430 | 60263 | 16.0 % | 32.0 | 67.0 |
| gsp-queue-doorbell | 188 | 52233 | 13.9 % | 104.0 | 917.0 |
| bar1-write | 642 | 49010 | 13.0 % | 33.0 | 115.0 |
| pcfg-mirror (intr re-arm) | 304 | 23663 | 6.3 % | 53.0 | 1440.0 |
| gsp-falcon | 564 | 21746 | 5.8 % | 33.0 | 55.0 |
| pci-config | 513 | 9453 | 2.5 % | 13.0 | 27.0 |
| usermode page, other | 60 | 2337 | 0.6 % | 31.0 | 58.0 |
| bar0-other | 8 | 1531 | 0.4 % | 124.0 | 443.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 4411 | 376668 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 4 | 14 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 8 | 9 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 3 | 24 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 30 | 30 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 3 | 7 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 3 | 8 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 23 | 23 | 23 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 6 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 8 | 11 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 10 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 2 | 4 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 2 | 3 | 3 | 3 | 2 |
| GSP_RM_ALLOC class 0x0 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0xe9800000 | 0xc1 | 206 |
| 0x129600000 | 0x5 | 76 |
| 0x129601000 | 0xc5 | 2 |
| 0x129601000 | 0x5 | 1 |
| 0xe9804000 | 0xc1 | 1 |

116 plan(s) applied by the worker: 17826 leaves unmapped, 19907 mapped in 524 RM map call(s); walk median 6 us (server thread), apply median 200 us p90 1828 us total 140.6 ms (worker), queued median 12 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 78 | 420 | 277 |
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 61 | 289 | 234 |
| 0 | 2 | 5 | 0 | 0 | 1 | 1 | 10 | 600 | 515 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 9 | 1080 | 343 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 4 | 1113 | 415 |
| 0 | 5 | 10 | 0 | 0 | 1 | 1 | 3 | 4152 | 868 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 3 | 3926 | 1021 |
| 0 | 2 | 5 | 0 | 0 | 44 | 44 | 2 | 300828 | 300799 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 2 | 1147 | 1013 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 2 | 1598 | 932 |
| 0 | 8 | 15 | 0 | 0 | 0 | 0 | 1 | 1150 | 1150 |
| 0 | 14 | 25 | 0 | 0 | 0 | 16 | 1 | 3069 | 3069 |
| 0 | 113 | 145 | 0 | 0 | 0 | 0 | 1 | 54965 | 54965 |
| 0 | 10 | 20 | 0 | 0 | 0 | 4 | 1 | 2028 | 2028 |

## 4. Segments split at idle gaps of 200.0 ms or more

Segments: 818.2 ms, 311.5 ms, 324.8 ms, 304.8 ms, 303.4 ms, 170.0 ms

### alloc1: 1 segment(s), 64 operation(s)

| class (alloc1) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 1.0 | 125 | 56.2 % | 126.0 | 171.0 |
| mmu-invalidate | 2.5 | 96 | 43.1 % | 29.0 | 60.0 |
| pcfg-mirror (intr re-arm) | 0.7 | 1 | 0.6 % | 45.5 | 52.0 |
| all | 4.2 | 222 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 4.2 | 222 |
| + CARAXES_INTR_PAGE=1 | 4.2 | 222 |
| + CARAXES_BAR2_MEMFD=1 | 4.2 | 222 |
| + both | 4.2 | 222 |

Median per access by class, us: bar2-flush-read 126.0, mmu-invalidate 29.0, pcfg-mirror (intr re-arm) 45.5

### free1: 1 segment(s), 64 operation(s)

| class (free1) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| mmu-invalidate | 5.0 | 225 | 45.6 % | 34.0 | 71.0 |
| bar2-flush-read | 1.5 | 192 | 39.0 % | 93.5 | 163.0 |
| pcfg-mirror (intr re-arm) | 0.8 | 76 | 15.4 % | 275.0 | 1440.0 |
| all | 7.3 | 493 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 7.3 | 493 |
| + CARAXES_INTR_PAGE=1 | 7.3 | 493 |
| + CARAXES_BAR2_MEMFD=1 | 7.3 | 493 |
| + both | 7.3 | 493 |

Median per access by class, us: bar2-flush-read 93.5, mmu-invalidate 34.0, pcfg-mirror (intr re-arm) 275.0

### alloc64: 1 segment(s), 8 operation(s)

| class (alloc64) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| pci-config | 64.1 | 1182 | 57.9 % | 13.0 | 27.0 |
| bar2-flush-read | 2.4 | 398 | 19.5 % | 94.0 | 290.0 |
| mmu-invalidate | 5.6 | 233 | 11.4 % | 30.0 | 78.0 |
| pcfg-mirror (intr re-arm) | 5.8 | 230 | 11.2 % | 31.0 | 1774.0 |
| all | 77.9 | 2042 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 77.9 | 2042 |
| + CARAXES_INTR_PAGE=1 | 77.9 | 2042 |
| + CARAXES_BAR2_MEMFD=1 | 77.9 | 2042 |
| + both | 77.9 | 2042 |

Median per access by class, us: bar2-flush-read 94.0, mmu-invalidate 30.0, pcfg-mirror (intr re-arm) 31.0, pci-config 13.0

### free64: 1 segment(s), 8 operation(s)

| class (free64) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3.1 | 547 | 48.4 % | 87.0 | 335.0 |
| mmu-invalidate | 10.0 | 404 | 35.7 % | 31.5 | 75.0 |
| pcfg-mirror (intr re-arm) | 5.6 | 180 | 15.9 % | 720.0 | 1404.0 |
| all | 18.8 | 1131 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 18.8 | 1131 |
| + CARAXES_INTR_PAGE=1 | 18.8 | 1131 |
| + CARAXES_BAR2_MEMFD=1 | 18.8 | 1131 |
| + both | 18.8 | 1131 |

Median per access by class, us: bar2-flush-read 87.0, mmu-invalidate 31.5, pcfg-mirror (intr re-arm) 720.0


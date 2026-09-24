# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a25d59d08c801f257/caraxes/runs/alloc-path-2026-09-24/night/stack-on/churn-trace.txt

Window 140147641 .. 142356208 us (2208.6 ms), 3894 trapped accesses, 326.2 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 682 | 138740 | 42.5 % | 80.0 | 796.0 |
| mmu-invalidate | 1430 | 53331 | 16.3 % | 30.0 | 54.0 |
| gsp-queue-doorbell | 188 | 51149 | 15.7 % | 101.5 | 981.0 |
| bar1-write | 642 | 37028 | 11.4 % | 30.0 | 54.0 |
| gsp-falcon | 564 | 21697 | 6.7 % | 31.0 | 53.0 |
| pcfg-mirror (intr re-arm) | 300 | 19959 | 6.1 % | 56.0 | 1366.0 |
| usermode page, other | 60 | 2345 | 0.7 % | 31.0 | 66.0 |
| bar0-other | 8 | 1967 | 0.6 % | 124.5 | 499.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 3894 | 326216 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 4 | 14 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 5 | 17 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 7 | 23 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 3 | 6 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 8 | 12 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 2 | 6 | 11 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 2 | 3 | 3 | 3 | 2 |
| GSP_RM_ALLOC class 0x0 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0xe9800000 | 0xc1 | 206 |
| 0x129600000 | 0x5 | 76 |
| 0x129601000 | 0xc5 | 2 |
| 0x129601000 | 0x5 | 1 |
| 0xe9804000 | 0xc1 | 1 |

20 plan(s) applied by the worker: 17792 leaves unmapped, 19874 mapped in 739 RM map call(s); walk median 34 us (server thread), apply median 1210 us p90 10458 us total 66.7 ms (worker), queued median 10 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 79 | 362 | 304 |
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 62 | 257 | 230 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 9 | 1054 | 322 |
| 0 | 2 | 5 | 0 | 0 | 1 | 1 | 8 | 504 | 383 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 4 | 1788 | 741 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 4 | 434 | 310 |
| 0 | 2 | 5 | 0 | 0 | 46 | 46 | 3 | 301026 | 300780 |
| 0 | 5 | 10 | 0 | 0 | 1 | 1 | 2 | 5017 | 4546 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 2 | 1152 | 799 |
| 0 | 3 | 5 | 0 | 0 | 1 | 1 | 2 | 1493 | 1408 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 2 | 1086 | 829 |
| 0 | 8 | 15 | 0 | 0 | 0 | 0 | 1 | 1076 | 1076 |
| 0 | 14 | 25 | 0 | 0 | 0 | 16 | 1 | 3826 | 3826 |
| 0 | 113 | 145 | 0 | 0 | 0 | 0 | 1 | 58556 | 58556 |

## 4. Segments split at idle gaps of 200.0 ms or more

Segments: 817.8 ms, 307.4 ms, 316.7 ms, 299.1 ms, 300.1 ms, 139.7 ms

### alloc1: 1 segment(s), 64 operation(s)

| class (alloc1) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 1.0 | 94 | 45.4 % | 76.0 | 145.0 |
| mmu-invalidate | 2.5 | 84 | 40.6 % | 28.0 | 52.0 |
| pcfg-mirror (intr re-arm) | 0.8 | 29 | 13.9 % | 91.0 | 1595.0 |
| all | 4.3 | 208 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 4.3 | 208 |
| + CARAXES_INTR_PAGE=1 | 4.3 | 208 |
| + CARAXES_BAR2_MEMFD=1 | 4.3 | 208 |
| + both | 4.3 | 208 |

Median per access by class, us: bar2-flush-read 76.0, mmu-invalidate 28.0, pcfg-mirror (intr re-arm) 91.0

### free1: 1 segment(s), 64 operation(s)

| class (free1) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| mmu-invalidate | 5.0 | 191 | 52.9 % | 29.0 | 56.0 |
| bar2-flush-read | 1.5 | 135 | 37.4 % | 81.0 | 128.0 |
| pcfg-mirror (intr re-arm) | 0.8 | 35 | 9.8 % | 167.0 | 1876.0 |
| all | 7.2 | 361 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 7.2 | 361 |
| + CARAXES_INTR_PAGE=1 | 7.2 | 361 |
| + CARAXES_BAR2_MEMFD=1 | 7.2 | 361 |
| + both | 7.2 | 361 |

Median per access by class, us: bar2-flush-read 81.0, mmu-invalidate 29.0, pcfg-mirror (intr re-arm) 167.0

### alloc64: 1 segment(s), 8 operation(s)

| class (alloc64) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| pcfg-mirror (intr re-arm) | 5.9 | 422 | 43.0 % | 1366.0 | 1889.0 |
| bar2-flush-read | 2.4 | 355 | 36.2 % | 109.0 | 288.0 |
| mmu-invalidate | 5.6 | 205 | 20.9 % | 30.0 | 54.0 |
| all | 13.9 | 981 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 13.9 | 981 |
| + CARAXES_INTR_PAGE=1 | 13.9 | 981 |
| + CARAXES_BAR2_MEMFD=1 | 13.9 | 981 |
| + both | 13.9 | 981 |

Median per access by class, us: bar2-flush-read 109.0, mmu-invalidate 30.0, pcfg-mirror (intr re-arm) 1366.0

### free64: 1 segment(s), 8 operation(s)

| class (free64) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| pcfg-mirror (intr re-arm) | 5.8 | 440 | 36.4 % | 1696.0 | 1781.0 |
| bar2-flush-read | 3.1 | 396 | 32.7 % | 76.0 | 277.0 |
| mmu-invalidate | 10.0 | 374 | 30.9 % | 28.5 | 51.0 |
| all | 18.9 | 1210 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 18.9 | 1210 |
| + CARAXES_INTR_PAGE=1 | 18.9 | 1210 |
| + CARAXES_BAR2_MEMFD=1 | 18.9 | 1210 |
| + both | 18.9 | 1210 |

Median per access by class, us: bar2-flush-read 76.0, mmu-invalidate 28.5, pcfg-mirror (intr re-arm) 1696.0


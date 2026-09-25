# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a25d59d08c801f257/caraxes/runs/alloc-path-2026-09-24/night/off/churn-trace.txt

Window 144681597 .. 151893810 us (7212.2 ms), 126315 trapped accesses, 5089.8 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 98514 | 3845026 | 75.5 % | 29.0 | 48.0 |
| vf-intr-tree | 14262 | 588712 | 11.6 % | 30.0 | 55.0 |
| pmc-intr-en (RM lock) | 7822 | 368372 | 7.2 % | 31.0 | 72.0 |
| mmu-invalidate | 1430 | 70328 | 1.4 % | 34.0 | 71.0 |
| bar1-write | 642 | 61426 | 1.2 % | 41.0 | 152.0 |
| pci-config | 1539 | 45210 | 0.9 % | 14.0 | 28.0 |
| bar2-flush-read | 682 | 33066 | 0.6 % | 37.0 | 69.0 |
| gsp-falcon | 564 | 24719 | 0.5 % | 34.0 | 65.0 |
| pcfg-mirror (intr re-arm) | 578 | 24424 | 0.5 % | 33.0 | 55.0 |
| gsp-queue-doorbell | 188 | 23954 | 0.5 % | 67.0 | 367.0 |
| bar0-other | 8 | 2189 | 0.0 % | 197.0 | 508.0 |
| usermode page, other | 60 | 2072 | 0.0 % | 31.0 | 48.0 |
| pmc page 0, other | 6 | 344 | 0.0 % | 39.0 | 59.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 126315 | 5089842 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 4 | 8 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 5 | 10 | 12 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 7 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 62 | 62 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 23 | 23 | 23 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 2 | 4 | 4 | 4 | 2 |
| GSP_RM_ALLOC class 0x0 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0xe9800000 | 0xc1 | 206 |
| 0x129600000 | 0x5 | 76 |
| 0x129601000 | 0xc5 | 2 |
| 0x129601000 | 0x5 | 1 |
| 0xe9804000 | 0xc1 | 1 |

116 plan(s) applied by the worker: 17826 leaves unmapped, 19907 mapped in 1023 RM map call(s); walk median 9 us (server thread), apply median 162 us p90 2650 us total 190.5 ms (worker), queued median 11 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 10 | 8 | 0 | 0 | 44 | 903 | 661 |
| 4 | 2 | 5 | 30 | 24 | 0 | 0 | 19 | 2228 | 1804 |
| 2 | 2 | 5 | 20 | 16 | 0 | 0 | 19 | 1821 | 1325 |
| 2 | 2 | 5 | 20 | 30 | 1 | 1 | 11 | 3020 | 1900 |
| 4 | 2 | 5 | 30 | 38 | 1 | 1 | 9 | 2954 | 2290 |
| 0 | 1 | 5 | 10 | 22 | 1 | 1 | 7 | 1905 | 1341 |
| 64 | 2 | 5 | 20 | 30 | 1 | 1 | 5 | 5228 | 4202 |
| 514 | 3 | 5 | 20 | 58 | 3 | 3 | 4 | 22376 | 17983 |
| 0 | 1 | 5 | 10 | 13 | 1 | 0 | 3 | 967 | 953 |
| 128 | 2 | 5 | 30 | 38 | 1 | 1 | 3 | 7236 | 6659 |
| 0 | 1 | 5 | 10 | 14 | 0 | 1 | 2 | 1884 | 1067 |
| 128 | 2 | 5 | 30 | 52 | 2 | 2 | 2 | 13046 | 10784 |
| 1036 | 8 | 15 | 30 | 24 | 0 | 0 | 1 | 63002 | 63002 |
| 4 | 2 | 5 | 20 | 16 | 0 | 0 | 1 | 2861 | 2861 |

## 4. Segments split at idle gaps of 200.0 ms or more

Segments: 4476.8 ms, 387.8 ms, 441.9 ms, 425.2 ms, 363.7 ms, 1104.2 ms

### alloc1: 1 segment(s), 64 operation(s)

| class (alloc1) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| vf-intr-tree | 24.4 | 910 | 52.0 % | 30.0 | 51.0 |
| pmc-intr-en (RM lock) | 14.7 | 571 | 32.7 % | 31.0 | 61.0 |
| mmu-invalidate | 2.5 | 106 | 6.1 % | 33.5 | 57.0 |
| bar2-write | 2.0 | 90 | 5.2 % | 36.0 | 62.0 |
| bar2-flush-read | 1.0 | 39 | 2.3 % | 32.5 | 59.0 |
| pcfg-mirror (intr re-arm) | 0.9 | 31 | 1.8 % | 33.0 | 39.0 |
| all | 45.5 | 1748 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 45.5 | 1748 |
| + CARAXES_INTR_PAGE=1 | 6.4 | 268 |
| + CARAXES_BAR2_MEMFD=1 | 43.5 | 1658 |
| + both | 4.4 | 177 |

Median per access by class, us: bar2-flush-read 32.5, bar2-write 36.0, mmu-invalidate 33.5, pcfg-mirror (intr re-arm) 33.0, pmc-intr-en (RM lock) 31.0, vf-intr-tree 30.0

### free1: 1 segment(s), 64 operation(s)

| class (free1) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| vf-intr-tree | 26.8 | 1244 | 48.1 % | 35.0 | 85.0 |
| pmc-intr-en (RM lock) | 15.4 | 826 | 32.0 % | 38.0 | 106.0 |
| mmu-invalidate | 5.0 | 291 | 11.2 % | 38.0 | 109.0 |
| bar2-flush-read | 1.5 | 90 | 3.5 % | 40.5 | 112.0 |
| bar2-write | 1.0 | 87 | 3.4 % | 45.0 | 114.0 |
| pcfg-mirror (intr re-arm) | 1.0 | 47 | 1.8 % | 37.0 | 67.0 |
| pmc page 0, other | 0.0 | 1 | 0.0 % | 35.0 | 35.0 |
| all | 50.8 | 2586 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 50.8 | 2586 |
| + CARAXES_INTR_PAGE=1 | 8.5 | 515 |
| + CARAXES_BAR2_MEMFD=1 | 49.8 | 2499 |
| + both | 7.5 | 428 |

Median per access by class, us: bar2-flush-read 40.5, bar2-write 45.0, mmu-invalidate 38.0, pcfg-mirror (intr re-arm) 37.0, pmc page 0, other 35.0, pmc-intr-en (RM lock) 38.0, vf-intr-tree 35.0

### alloc64: 1 segment(s), 8 operation(s)

| class (alloc64) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 192.5 | 10318 | 55.0 % | 33.0 | 108.0 |
| vf-intr-tree | 134.5 | 5712 | 30.5 % | 33.0 | 69.0 |
| pmc-intr-en (RM lock) | 27.5 | 1538 | 8.2 % | 35.0 | 89.0 |
| mmu-invalidate | 5.6 | 480 | 2.6 % | 38.5 | 102.0 |
| pcfg-mirror (intr re-arm) | 8.1 | 370 | 2.0 % | 34.0 | 87.0 |
| bar2-flush-read | 2.4 | 339 | 1.8 % | 36.0 | 149.0 |
| all | 370.6 | 18756 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 370.6 | 18756 |
| + CARAXES_INTR_PAGE=1 | 208.6 | 11506 |
| + CARAXES_BAR2_MEMFD=1 | 178.1 | 8438 |
| + both | 16.1 | 1188 |

Median per access by class, us: bar2-flush-read 36.0, bar2-write 33.0, mmu-invalidate 38.5, pcfg-mirror (intr re-arm) 34.0, pmc-intr-en (RM lock) 35.0, vf-intr-tree 33.0

### free64: 1 segment(s), 8 operation(s)

| class (free64) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| vf-intr-tree | 118.5 | 5210 | 47.7 % | 34.0 | 73.0 |
| bar2-write | 64.1 | 2936 | 26.9 % | 34.0 | 77.0 |
| pmc-intr-en (RM lock) | 30.0 | 1624 | 14.9 % | 35.0 | 105.0 |
| mmu-invalidate | 10.0 | 662 | 6.1 % | 36.0 | 131.0 |
| pcfg-mirror (intr re-arm) | 6.8 | 296 | 2.7 % | 36.0 | 71.0 |
| bar2-flush-read | 3.1 | 192 | 1.8 % | 47.0 | 131.0 |
| all | 232.5 | 10921 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 232.5 | 10921 |
| + CARAXES_INTR_PAGE=1 | 84.0 | 4086 |
| + CARAXES_BAR2_MEMFD=1 | 168.4 | 7986 |
| + both | 19.9 | 1151 |

Median per access by class, us: bar2-flush-read 47.0, bar2-write 34.0, mmu-invalidate 36.0, pcfg-mirror (intr re-arm) 36.0, pmc-intr-en (RM lock) 35.0, vf-intr-tree 34.0


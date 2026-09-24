# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a25d59d08c801f257/caraxes/runs/alloc-path-2026-09-24/night/intr/churn-trace.txt

Window 141445296 .. 149316336 us (7871.0 ms), 104830 trapped accesses, 4718.2 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 98514 | 4078213 | 86.4 % | 28.0 | 47.0 |
| bar2-flush-read | 682 | 296857 | 6.3 % | 648.0 | 946.0 |
| bar1-write | 642 | 88311 | 1.9 % | 35.0 | 157.0 |
| mmu-invalidate | 1430 | 72505 | 1.5 % | 34.0 | 63.0 |
| pcfg-mirror (intr re-arm) | 670 | 51737 | 1.1 % | 43.0 | 94.0 |
| gsp-queue-doorbell | 188 | 48949 | 1.0 % | 137.5 | 909.0 |
| pci-config | 2052 | 46151 | 1.0 % | 15.0 | 29.0 |
| gsp-falcon | 564 | 29017 | 0.6 % | 35.0 | 64.0 |
| usermode page, other | 60 | 4713 | 0.1 % | 36.0 | 87.0 |
| bar0-other | 8 | 1766 | 0.0 % | 90.0 | 441.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 104830 | 4718219 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| GSP_RM_ALLOC class 0xc46f | 20 | 5 | 11 | 669 | 0 |
| FREE | 71 | 3 | 7 | 46 | 24 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 4 | 8 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 4 | 14 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 3 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 17 | 17 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 12 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 2 | 3 | 3 | 3 | 2 |
| GSP_RM_CONTROL cmd 0x20800a9a | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 2 | 2 | 2 | 1 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0xe9800000 | 0xc1 | 206 |
| 0x129600000 | 0x5 | 76 |
| 0x129601000 | 0xc5 | 2 |
| 0x129601000 | 0x5 | 1 |
| 0xe9804000 | 0xc1 | 1 |

116 plan(s) applied by the worker: 17826 leaves unmapped, 19907 mapped in 1146 RM map call(s); walk median 8 us (server thread), apply median 178 us p90 2905 us total 144.7 ms (worker), queued median 10 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 62 | 252 | 175 |
| 4 | 2 | 5 | 0 | 0 | 0 | 0 | 23 | 2189 | 1804 |
| 2 | 2 | 5 | 0 | 0 | 0 | 0 | 20 | 1780 | 1636 |
| 2 | 2 | 5 | 0 | 0 | 1 | 1 | 15 | 3024 | 1771 |
| 4 | 2 | 5 | 0 | 0 | 1 | 1 | 10 | 2880 | 1983 |
| 128 | 2 | 5 | 0 | 0 | 2 | 2 | 4 | 10884 | 9596 |
| 64 | 2 | 5 | 0 | 0 | 1 | 1 | 4 | 4738 | 4256 |
| 2 | 2 | 5 | 0 | 0 | 2 | 2 | 3 | 9216 | 8171 |
| 1542 | 5 | 10 | 0 | 0 | 12 | 12 | 2 | 81841 | 80387 |
| 1542 | 5 | 10 | 0 | 0 | 11 | 11 | 2 | 71978 | 71229 |
| 2 | 2 | 5 | 0 | 0 | 45 | 45 | 2 | 304346 | 302323 |
| 128 | 2 | 5 | 0 | 0 | 1 | 1 | 2 | 6556 | 6065 |
| 514 | 3 | 5 | 0 | 0 | 3 | 3 | 2 | 21816 | 19435 |
| 514 | 3 | 5 | 0 | 0 | 4 | 4 | 2 | 27879 | 26450 |

## 4. Segments split at idle gaps of 200.0 ms or more

Segments: 4742.9 ms, 396.3 ms, 396.0 ms, 416.5 ms, 347.3 ms, 1535.6 ms

### alloc1: 1 segment(s), 64 operation(s)

| class (alloc1) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 2.0 | 489 | 47.8 % | 65.0 | 851.0 |
| bar2-flush-read | 1.0 | 347 | 33.9 % | 118.0 | 754.0 |
| mmu-invalidate | 2.5 | 121 | 11.8 % | 38.5 | 71.0 |
| pcfg-mirror (intr re-arm) | 1.0 | 66 | 6.4 % | 64.5 | 124.0 |
| all | 6.5 | 1022 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 6.5 | 1022 |
| + CARAXES_INTR_PAGE=1 | 6.5 | 1022 |
| + CARAXES_BAR2_MEMFD=1 | 4.5 | 533 |
| + both | 4.5 | 533 |

Median per access by class, us: bar2-flush-read 118.0, bar2-write 65.0, mmu-invalidate 38.5, pcfg-mirror (intr re-arm) 64.5

### free1: 1 segment(s), 64 operation(s)

| class (free1) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 1.5 | 419 | 38.8 % | 72.0 | 753.0 |
| bar2-write | 1.0 | 388 | 36.0 % | 118.0 | 874.0 |
| mmu-invalidate | 5.0 | 239 | 22.2 % | 33.0 | 56.0 |
| pcfg-mirror (intr re-arm) | 0.9 | 33 | 3.1 % | 49.0 | 84.0 |
| all | 8.4 | 1080 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 8.4 | 1080 |
| + CARAXES_INTR_PAGE=1 | 8.4 | 1080 |
| + CARAXES_BAR2_MEMFD=1 | 7.4 | 692 |
| + both | 7.4 | 692 |

Median per access by class, us: bar2-flush-read 72.0, bar2-write 118.0, mmu-invalidate 33.0, pcfg-mirror (intr re-arm) 49.0

### alloc64: 1 segment(s), 8 operation(s)

| class (alloc64) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 192.5 | 9094 | 74.6 % | 31.0 | 51.0 |
| bar2-flush-read | 2.4 | 1228 | 10.1 % | 571.5 | 1057.0 |
| pci-config | 64.1 | 1007 | 8.3 % | 12.0 | 27.0 |
| pcfg-mirror (intr re-arm) | 8.0 | 497 | 4.1 % | 46.0 | 634.0 |
| mmu-invalidate | 5.6 | 367 | 3.0 % | 36.0 | 84.0 |
| all | 272.6 | 12193 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 272.6 | 12193 |
| + CARAXES_INTR_PAGE=1 | 272.6 | 12193 |
| + CARAXES_BAR2_MEMFD=1 | 80.1 | 3099 |
| + both | 80.1 | 3099 |

Median per access by class, us: bar2-flush-read 571.5, bar2-write 31.0, mmu-invalidate 36.0, pcfg-mirror (intr re-arm) 46.0, pci-config 12.0

### free64: 1 segment(s), 8 operation(s)

| class (free64) | accesses per op | time per op, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 64.1 | 3937 | 66.5 % | 33.0 | 62.0 |
| bar2-flush-read | 3.1 | 1048 | 17.7 % | 80.0 | 893.0 |
| pcfg-mirror (intr re-arm) | 6.5 | 487 | 8.2 % | 77.0 | 996.0 |
| mmu-invalidate | 10.0 | 452 | 7.6 % | 32.0 | 64.0 |
| all | 83.8 | 5924 | 100 % | | |

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 83.8 | 5924 |
| + CARAXES_INTR_PAGE=1 | 83.8 | 5924 |
| + CARAXES_BAR2_MEMFD=1 | 19.6 | 1987 |
| + both | 19.6 | 1987 |

Median per access by class, us: bar2-flush-read 80.0, bar2-write 33.0, mmu-invalidate 32.0, pcfg-mirror (intr re-arm) 77.0


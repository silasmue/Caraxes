# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/off-2/rl-trace.txt

Window 198092709 .. 218570968 us (20478.3 ms), 107965 trapped accesses, 4963.1 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 100641 | 4496102 | 90.6 % | 35.0 | 49.0 |
| pcfg-mirror (intr re-arm) | 2923 | 237223 | 4.8 % | 52.0 | 1015.0 |
| mmu-invalidate | 960 | 59808 | 1.2 % | 39.0 | 73.0 |
| pci-config | 2052 | 46265 | 0.9 % | 16.0 | 34.0 |
| gsp-falcon | 582 | 42670 | 0.9 % | 44.0 | 100.0 |
| gsp-queue-doorbell | 194 | 42142 | 0.8 % | 138.5 | 515.0 |
| bar2-flush-read | 525 | 32629 | 0.7 % | 46.0 | 88.0 |
| usermode page, other | 60 | 4773 | 0.1 % | 42.0 | 81.0 |
| bar0-other | 8 | 1515 | 0.0 % | 66.0 | 285.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 107965 | 4963127 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 4 | 7 | 18 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 6 | 13 | 17 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 4 | 12 | 15 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 12 | 13 | 13 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 4 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 4 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 17 | 17 | 17 | 1 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 8 | 11 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 4 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 4 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 4 | 6 | 6 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 4 | 5 | 5 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 4 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 7 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 113 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

51 plan(s) applied by the worker: 1052 leaves unmapped, 21470 mapped in 17200 RM map call(s); walk median 111 us (server thread), apply median 2962 us p90 12961 us total 783.6 ms (worker), queued median 18 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 34 | 232 | 206 |
| 2 | 2 | 5 | 0 | 0 | 0 | 0 | 14 | 368 | 324 |
| 514 | 3 | 5 | 0 | 0 | 3 | 3 | 6 | 21249 | 19579 |
| 4 | 2 | 5 | 0 | 0 | 1 | 1 | 4 | 2955 | 836 |
| 4 | 2 | 5 | 0 | 0 | 0 | 0 | 2 | 1150 | 919 |
| 1542 | 5 | 10 | 0 | 0 | 15 | 15 | 2 | 99854 | 99706 |
| 514 | 3 | 5 | 0 | 0 | 5 | 5 | 2 | 31663 | 29339 |
| 514 | 3 | 5 | 0 | 0 | 4 | 4 | 2 | 22670 | 21204 |
| 1036 | 8 | 15 | 0 | 0 | 13 | 13 | 1 | 85070 | 85070 |
| 132 | 8 | 15 | 0 | 0 | 10 | 25 | 1 | 69813 | 69813 |
| 43176 | 113 | 145 | 0 | 0 | 326 | 839 | 1 | 2200551 | 2200551 |
| 1070 | 10 | 20 | 0 | 0 | 11 | 15 | 1 | 70837 | 70837 |
| 1009 | 6 | 10 | 0 | 0 | 11 | 11 | 1 | 73753 | 73753 |
| 12 | 3 | 5 | 0 | 0 | 0 | 0 | 1 | 713 | 713 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 107965 | 4963127 |
| + CARAXES_INTR_PAGE=1 | 107965 | 4963127 |
| + CARAXES_BAR2_MEMFD=1 | 7324 | 467025 |
| + both | 7324 | 467025 |

Median per access by class, us: bar0-other 66.0, bar2-flush-read 46.0, bar2-write 35.0, gsp-falcon 44.0, gsp-queue-doorbell 138.5, mmu-invalidate 39.0, pcfg-mirror (intr re-arm) 52.0, pci-config 16.0, usermode page, other 42.0


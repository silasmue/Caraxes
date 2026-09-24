# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/off/ppo-trace.txt

Window 240855027 .. 280289924 us (39434.9 ms), 118221 trapped accesses, 4398.1 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 100713 | 3852170 | 87.6 % | 29.0 | 46.0 |
| pci-config | 9234 | 234454 | 5.3 % | 18.0 | 39.0 |
| pcfg-mirror (intr re-arm) | 5865 | 154955 | 3.5 % | 40.0 | 572.0 |
| mmu-invalidate | 1005 | 50205 | 1.1 % | 32.0 | 65.0 |
| gsp-queue-doorbell | 194 | 40740 | 0.9 % | 95.0 | 656.0 |
| gsp-falcon | 582 | 31613 | 0.7 % | 38.0 | 76.0 |
| bar2-flush-read | 540 | 29211 | 0.7 % | 36.0 | 72.0 |
| usermode page, other | 60 | 2922 | 0.1 % | 43.5 | 73.0 |
| bar0-other | 8 | 1840 | 0.0 % | 180.5 | 443.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 118221 | 4398110 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 5 | 31 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 5 | 9 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 40 | 40 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 35 | 35 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 6 | 6 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 4 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 11 | 12 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 11 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 17 | 17 | 17 | 1 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 12 | 12 | 12 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 9 | 9 | 9 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 6 | 6 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 6 | 6 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 122 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

53 plan(s) applied by the worker: 1052 leaves unmapped, 21482 mapped in 1918 RM map call(s); walk median 86 us (server thread), apply median 2125 us p90 6308 us total 200.1 ms (worker), queued median 12 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 35 | 198 | 166 |
| 2 | 2 | 5 | 0 | 0 | 0 | 0 | 16 | 266 | 259 |
| 514 | 3 | 5 | 0 | 0 | 2 | 2 | 7 | 15416 | 14130 |
| 4 | 2 | 5 | 0 | 0 | 0 | 0 | 4 | 1320 | 685 |
| 4 | 2 | 5 | 0 | 0 | 1 | 1 | 3 | 3276 | 1405 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 3 | 247 | 215 |
| 1542 | 5 | 10 | 0 | 0 | 7 | 7 | 2 | 57445 | 51905 |
| 40 | 2 | 5 | 0 | 0 | 0 | 0 | 2 | 2200 | 1801 |
| 20 | 2 | 5 | 0 | 0 | 0 | 0 | 2 | 1498 | 740 |
| 8 | 2 | 5 | 0 | 0 | 0 | 0 | 2 | 446 | 422 |
| 514 | 3 | 5 | 0 | 0 | 3 | 3 | 2 | 20022 | 19512 |
| 1036 | 8 | 15 | 0 | 0 | 11 | 11 | 1 | 69320 | 69320 |
| 132 | 8 | 15 | 0 | 0 | 9 | 24 | 1 | 65361 | 65361 |
| 43176 | 113 | 145 | 0 | 0 | 270 | 777 | 1 | 1832728 | 1832728 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 118221 | 4398110 |
| + CARAXES_INTR_PAGE=1 | 118221 | 4398110 |
| + CARAXES_BAR2_MEMFD=1 | 17508 | 545940 |
| + both | 17508 | 545940 |

Median per access by class, us: bar0-other 180.5, bar2-flush-read 36.0, bar2-write 29.0, gsp-falcon 38.0, gsp-queue-doorbell 95.0, mmu-invalidate 32.0, pcfg-mirror (intr re-arm) 40.0, pci-config 18.0, usermode page, other 43.5


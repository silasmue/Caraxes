# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/off/short-auto-trace.txt

Window 285496758 .. 292913545 us (7416.8 ms), 101195 trapped accesses, 3606.1 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 96199 | 3355900 | 93.1 % | 29.0 | 36.0 |
| pcfg-mirror (intr re-arm) | 801 | 70733 | 2.0 % | 34.0 | 70.0 |
| gsp-queue-doorbell | 187 | 49569 | 1.4 % | 85.0 | 1044.0 |
| pci-config | 2052 | 44078 | 1.2 % | 18.0 | 29.0 |
| mmu-invalidate | 830 | 35838 | 1.0 % | 33.0 | 54.0 |
| gsp-falcon | 561 | 24017 | 0.7 % | 34.0 | 52.0 |
| bar2-flush-read | 477 | 21794 | 0.6 % | 34.0 | 69.0 |
| usermode page, other | 60 | 2349 | 0.1 % | 30.0 | 59.0 |
| bar0-other | 8 | 1861 | 0.1 % | 190.5 | 411.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 101195 | 3606139 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 5 | 12 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 8 | 10 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 4 | 14 | 14 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 4 | 8 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 8 | 13 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 7 | 12 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 2 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 6 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 2 | 5 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 2 | 4 | 4 | 4 | 2 |
| GSP_RM_CONTROL cmd 0x802009 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 4 | 4 | 4 | 1 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 89 |
| 0xa9600000 | 0x5 | 73 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

38 plan(s) applied by the worker: 18050 leaves unmapped, 20131 mapped in 17611 RM map call(s); walk median 14 us (server thread), apply median 1243 us p90 13612 us total 2071.8 ms (worker), queued median 12 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 23 | 239 | 195 |
| 2 | 2 | 5 | 0 | 0 | 0 | 0 | 8 | 469 | 285 |
| 4 | 2 | 5 | 0 | 0 | 0 | 0 | 5 | 625 | 469 |
| 514 | 3 | 5 | 0 | 0 | 3 | 3 | 4 | 19314 | 18867 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 3 | 274 | 243 |
| 1542 | 5 | 10 | 0 | 0 | 8 | 8 | 2 | 53238 | 53029 |
| 2 | 2 | 5 | 0 | 0 | 0 | 4 | 2 | 772 | 639 |
| 514 | 3 | 5 | 0 | 0 | 2 | 2 | 2 | 16692 | 16109 |
| 1036 | 8 | 15 | 0 | 0 | 6 | 6 | 1 | 41337 | 41337 |
| 132 | 8 | 15 | 0 | 0 | 4 | 20 | 1 | 24773 | 24773 |
| 43176 | 113 | 145 | 0 | 0 | 259 | 772 | 1 | 1736852 | 1736852 |
| 1070 | 10 | 20 | 0 | 0 | 5 | 9 | 1 | 37376 | 37376 |
| 1009 | 6 | 10 | 0 | 0 | 5 | 5 | 1 | 37086 | 37086 |
| 12 | 3 | 5 | 0 | 0 | 0 | 0 | 1 | 547 | 547 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 101195 | 3606139 |
| + CARAXES_INTR_PAGE=1 | 101195 | 3606139 |
| + CARAXES_BAR2_MEMFD=1 | 4996 | 250239 |
| + both | 4996 | 250239 |

Median per access by class, us: bar0-other 190.5, bar2-flush-read 34.0, bar2-write 29.0, gsp-falcon 34.0, gsp-queue-doorbell 85.0, mmu-invalidate 33.0, pcfg-mirror (intr re-arm) 34.0, pci-config 18.0, usermode page, other 30.0


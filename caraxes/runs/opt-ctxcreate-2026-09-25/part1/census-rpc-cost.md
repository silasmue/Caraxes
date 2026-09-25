# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/part1/census-trace.txt

Window 263866492 .. 282523220 us (18656.7 ms), 17755 trapped accesses, 2138.0 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 888326 | 41.5 % | 106.0 | 851.0 |
| mmu-invalidate | 5590 | 480094 | 22.5 % | 40.0 | 127.0 |
| gsp-queue-doorbell | 1087 | 310854 | 14.5 % | 129.0 | 980.0 |
| gsp-falcon | 3261 | 211933 | 9.9 % | 37.0 | 94.0 |
| bar0-other | 616 | 107470 | 5.0 % | 182.0 | 1190.0 |
| pci-config | 3591 | 85094 | 4.0 % | 16.0 | 38.0 |
| usermode-doorbell | 120 | 35750 | 1.7 % | 1668.0 | 1889.0 |
| usermode page, other | 360 | 18024 | 0.8 % | 35.0 | 76.0 |
| pcfg-mirror (intr re-arm) | 5 | 463 | 0.0 % | 100.0 | 157.0 |
| all | 17755 | 2138008 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 5 | 997 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 4 | 9 | 193 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 6 | 89 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 10 | 84 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 59 | 84 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 4 | 123 | 124 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 93 | 121 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 3 | 118 | 118 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 2 | 60 | 85 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 58 | 85 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 4 | 8 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 2 | 24 | 69 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 3 | 60 | 60 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 11 | 84 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 3 | 7 | 84 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 9 | 14 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 3 | 85 | 85 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 8 | 58 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 4 | 16 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 4 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 3 | 57 | 57 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 7 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 2 | 58 | 58 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 60 | 60 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 2 | 4 | 57 | 6 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 58 | 58 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 4 | 21 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 4 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 3 | 4 | 4 | 10 |
| GSP_RM_ALLOC class 0x83de | 6 | 3 | 3 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 2 | 3 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 3 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 2 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 638 |
| 0xa9600000 | 0x5 | 457 |
| 0xa9601000 | 0xc5 | 12 |
| 0xa9601000 | 0x5 | 6 |
| 0x69804000 | 0xc1 | 5 |

298 plan(s) applied by the worker: 101596 leaves unmapped, 131368 mapped in 1074 RM map call(s); walk median 25 us (server thread), apply median 355 us p90 5388 us total 903.5 ms (worker), queued median 12 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1074 | 71.3 | 66 |
| unmap | 753 | 95.6 | 127 |
| object alloc | 889 | 126.3 | 142 |
| object free | 895 | 82.9 | 93 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 138 | 368.2 | 2668 |
| gather free | 138 | 27.7 | 201 |
| total | 3887 | 772.0 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 173 | 329 | 232 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 82 | 624 | 329 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 41 | 1206 | 342 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 24 | 5240 | 1498 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 23 | 2866 | 741 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 19 | 2496 | 381 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 17 | 1111 | 268 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 13 | 1201 | 526 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 13 | 4978 | 789 |
| 0 | 2 | 5 | 0 | 0 | 9 | 4 | 10 | 61892 | 61030 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 9 | 1027 | 883 |
| 0 | 2 | 5 | 0 | 0 | 5 | 2 | 8 | 33276 | 31576 |
| 0 | 5 | 10 | 0 | 0 | 5 | 2 | 8 | 35144 | 33104 |
| 0 | 3 | 5 | 0 | 0 | 5 | 2 | 7 | 32697 | 31315 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 17755 | 2138008 |
| + CARAXES_INTR_PAGE=1 | 17755 | 2138008 |
| + CARAXES_BAR2_MEMFD=1 | 17755 | 2138008 |
| + both | 17755 | 2138008 |

Median per access by class, us: bar0-other 182.0, bar2-flush-read 106.0, gsp-falcon 37.0, gsp-queue-doorbell 129.0, mmu-invalidate 40.0, pcfg-mirror (intr re-arm) 100.0, pci-config 16.0, usermode page, other 35.0, usermode-doorbell 1668.0


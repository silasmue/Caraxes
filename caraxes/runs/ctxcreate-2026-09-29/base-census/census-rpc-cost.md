# rpc-cost: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census/census-trace.txt

Window 134096150 .. 160619129 us (26523.0 ms), 100266 trapped accesses, 8405.7 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| mmu-invalidate | 61290 | 5221999 | 62.1 % | 45.0 | 149.0 |
| bar2-flush-read | 19613 | 1606215 | 19.1 % | 53.0 | 130.0 |
| gsp-queue-doorbell | 2860 | 872772 | 10.4 % | 134.0 | 789.0 |
| gsp-falcon | 8445 | 356168 | 4.2 % | 34.0 | 61.0 |
| pci-config | 6156 | 118641 | 1.4 % | 15.0 | 30.0 |
| usermode-doorbell | 300 | 114227 | 1.4 % | 1225.0 | 1453.0 |
| bar0-other | 688 | 74361 | 0.9 % | 108.0 | 450.0 |
| usermode page, other | 900 | 40910 | 0.5 % | 33.0 | 66.0 |
| pcfg-mirror (intr re-arm) | 14 | 393 | 0.0 % | 29.0 | 34.0 |
| all | 100266 | 8405686 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 1059 | 3 | 4 | 35 | 618 |
| GSP_RM_CONTROL cmd 0x90f10106 | 28 | 3 | 9 | 1747 | 0 |
| GSP_RM_ALLOC class 0xc46f | 300 | 4 | 10 | 17 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 300 | 3 | 6 | 17 | 0 |
| GSP_RM_ALLOC class 0x79 | 120 | 3 | 3 | 16 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 120 | 3 | 3 | 12 | 0 |
| GSP_RM_ALLOC class 0xa06c | 60 | 3 | 8 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 70 | 2 | 3 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 60 | 3 | 7 | 16 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 60 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 60 | 3 | 3 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0c | 42 | 3 | 4 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 30 | 3 | 6 | 12 | 0 |
| GSP_RM_ALLOC class 0x80 | 29 | 3 | 4 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 29 | 3 | 8 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 28 | 3 | 7 | 14 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 28 | 3 | 4 | 9 | 28 |
| GSP_RM_ALLOC class 0x90f1 | 28 | 3 | 4 | 8 | 0 |
| GSP_RM_ALLOC class 0x2080 | 29 | 3 | 3 | 8 | 0 |
| GSP_RM_ALLOC class 0x0 | 29 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 29 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 15 | 3 | 8 | 9 | 15 |
| GSP_RM_CONTROL cmd 0x2080012b | 15 | 3 | 9 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 14 | 3 | 8 | 13 | 0 |
| GSP_RM_ALLOC class 0x2081 | 14 | 3 | 4 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 14 | 3 | 8 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 14 | 3 | 7 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 15 | 3 | 3 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 14 | 3 | 7 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 14 | 3 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0x83de | 15 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 14 | 3 | 3 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 15 | 3 | 3 | 3 | 15 |
| GSP_RM_ALLOC class 0x9067 | 15 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 14 | 3 | 3 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 14 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 14 | 2 | 4 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 14 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a02 | 3 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x169800000 | 0xc1 | 8612 |
| 0x1e9600000 | 0x5 | 1174 |
| 0x1e9601000 | 0xc5 | 30 |
| 0x1e9601000 | 0x5 | 15 |
| 0x169804000 | 0xc1 | 14 |

5381 plan(s) applied by the worker: 203297 leaves unmapped, 326038 mapped in 19106 RM map call(s); walk median 6 us (server thread), apply median 36 us p90 1553 us total 2999.5 ms (worker), queued median 8 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 19106 | 551.6 | 29 |
| unmap | 18250 | 476.3 | 26 |
| object alloc | 14271 | 507.8 | 36 |
| object free | 14286 | 407.2 | 29 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 334 | 890.3 | 2666 |
| gather free | 334 | 27.0 | 81 |
| total | 66581 | 2860.2 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 4351 | 368 | 260 |
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 2185 | 277 | 201 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 209 | 810 | 245 |
| 0 | 1 | 15 | 0 | 0 | 0 | 0 | 133 | 1700 | 1533 |
| 0 | 2 | 16 | 0 | 0 | 0 | 0 | 105 | 2094 | 1957 |
| 0 | 1 | 6 | 0 | 0 | 0 | 0 | 92 | 476 | 357 |
| 0 | 1 | 14 | 0 | 0 | 0 | 0 | 81 | 1658 | 1506 |
| 0 | 2 | 15 | 0 | 0 | 0 | 0 | 59 | 2122 | 1810 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 55 | 2149 | 586 |
| 0 | 2 | 17 | 0 | 0 | 0 | 0 | 55 | 2188 | 2033 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 48 | 991 | 465 |
| 0 | 1 | 13 | 0 | 0 | 0 | 0 | 46 | 1713 | 1356 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 44 | 602 | 297 |
| 0 | 2 | 10 | 0 | 0 | 0 | 0 | 40 | 1223 | 1067 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 100266 | 8405686 |
| + CARAXES_INTR_PAGE=1 | 100266 | 8405686 |
| + CARAXES_BAR2_MEMFD=1 | 100266 | 8405686 |
| + both | 100266 | 8405686 |

Median per access by class, us: bar0-other 108.0, bar2-flush-read 53.0, gsp-falcon 34.0, gsp-queue-doorbell 134.0, mmu-invalidate 45.0, pcfg-mirror (intr re-arm) 29.0, pci-config 15.0, usermode page, other 33.0, usermode-doorbell 1225.0


# rpc-cost: gather-2/census-trace.txt.xz

Window 140161940 .. 156009605 us (15847.7 ms), 23309 trapped accesses, 2084.4 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 702012 | 33.7 % | 81.0 | 674.0 |
| bar1-write | 7284 | 342068 | 16.4 % | 30.0 | 63.0 |
| gsp-queue-doorbell | 1087 | 287215 | 13.8 % | 109.0 | 1002.0 |
| mmu-invalidate | 5590 | 224565 | 10.8 % | 31.0 | 62.0 |
| pcfg-mirror (intr re-arm) | 1866 | 166921 | 8.0 % | 59.0 | 1203.0 |
| bar0-other | 616 | 163521 | 7.8 % | 183.0 | 1388.0 |
| gsp-falcon | 3261 | 127465 | 6.1 % | 32.0 | 62.0 |
| usermode-doorbell | 120 | 54454 | 2.6 % | 1316.0 | 1801.0 |
| usermode page, other | 360 | 16200 | 0.8 % | 38.0 | 71.0 |
| all | 23309 | 2084421 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 3 | 29 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 4 | 8 | 14 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 3 | 14 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 3 | 10 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 2 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 2 | 7 | 8 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 6 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 3 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 2 | 3 | 8 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 12 | 18 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 3 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 2 | 4 | 11 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 4 | 9 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 3 | 3 | 8 | 10 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 2 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 2 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 14 | 14 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 2 | 7 | 8 | 6 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 2 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x1a9800000 | 0xc1 | 638 |
| 0x1e9600000 | 0x5 | 457 |
| 0x1e9601000 | 0xc5 | 12 |
| 0x1e9601000 | 0x5 | 6 |
| 0x1a9804000 | 0xc1 | 5 |

282 plan(s) applied by the worker: 98018 leaves unmapped, 127790 mapped in 1102 RM map call(s); walk median 14 us (server thread), apply median 224 us p90 3465 us total 701.9 ms (worker), queued median 10 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1102 | 87.0 | 79 |
| unmap | 682 | 47.3 | 69 |
| object alloc | 889 | 82.9 | 93 |
| object free | 895 | 110.8 | 124 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 135 | 376.6 | 2789 |
| gather free | 135 | 19.2 | 142 |
| total | 3838 | 723.8 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 182 | 267 | 213 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 81 | 472 | 290 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 54 | 1184 | 311 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 15 | 2056 | 771 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 14 | 743 | 556 |
| 0 | 2 | 5 | 0 | 0 | 5 | 7 | 12 | 30984 | 30609 |
| 0 | 5 | 10 | 0 | 0 | 1 | 1 | 11 | 2241 | 743 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 10 | 1179 | 751 |
| 0 | 2 | 5 | 0 | 0 | 9 | 13 | 9 | 60977 | 60714 |
| 0 | 3 | 5 | 0 | 0 | 1 | 1 | 8 | 1333 | 991 |
| 0 | 2 | 5 | 0 | 0 | 0 | 9 | 7 | 2263 | 1055 |
| 0 | 3 | 5 | 0 | 0 | 5 | 7 | 7 | 31010 | 30923 |
| 0 | 6 | 10 | 0 | 0 | 0 | 0 | 6 | 1058 | 963 |
| 0 | 2 | 5 | 0 | 0 | 0 | 3 | 6 | 784 | 704 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 23309 | 2084421 |
| + CARAXES_INTR_PAGE=1 | 23309 | 2084421 |
| + CARAXES_BAR2_MEMFD=1 | 23309 | 2084421 |
| + both | 23309 | 2084421 |

Median per access by class, us: bar0-other 183.0, bar1-write 30.0, bar2-flush-read 81.0, gsp-falcon 32.0, gsp-queue-doorbell 109.0, mmu-invalidate 31.0, pcfg-mirror (intr re-arm) 59.0, usermode page, other 38.0, usermode-doorbell 1316.0


# rpc-cost: defer-pool/census-trace.txt.xz

Window 141703549 .. 164044998 us (22341.4 ms), 23553 trapped accesses, 2879.7 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 801300 | 27.8 % | 97.0 | 798.0 |
| bar1-write | 7284 | 799829 | 27.8 % | 48.0 | 140.0 |
| pcfg-mirror (intr re-arm) | 2110 | 330877 | 11.5 % | 128.5 | 1433.0 |
| mmu-invalidate | 5590 | 305456 | 10.6 % | 36.0 | 78.0 |
| gsp-queue-doorbell | 1087 | 269362 | 9.4 % | 122.0 | 609.0 |
| bar0-other | 616 | 175130 | 6.1 % | 277.0 | 1465.0 |
| gsp-falcon | 3261 | 158348 | 5.5 % | 36.0 | 71.0 |
| usermode-doorbell | 120 | 21406 | 0.7 % | 1801.0 | 1917.0 |
| usermode page, other | 360 | 17988 | 0.6 % | 35.0 | 72.0 |
| all | 23553 | 2879696 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 4 | 96 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 4 | 9 | 15 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 7 | 13 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 9 | 16 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 4 | 17 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 4 | 67 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 8 | 11 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 4 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 4 | 11 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 3 | 11 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 4 | 19 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 3 | 36 | 36 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 4 | 18 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 4 | 8 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 4 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 4 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 4 | 5 | 9 | 10 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 7 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 3 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 3 | 4 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 3 | 17 | 17 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 3 | 4 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 4 | 4 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 3 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 3 | 3 | 3 | 6 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x1a9800000 | 0xc1 | 638 |
| 0x1e9600000 | 0x5 | 457 |
| 0x1e9601000 | 0xc5 | 12 |
| 0x1e9601000 | 0x5 | 6 |
| 0x1a9804000 | 0xc1 | 5 |

282 plan(s) applied by the worker: 98018 leaves unmapped, 127790 mapped in 54927 RM map call(s); walk median 17 us (server thread), apply median 450 us p90 17269 us total 5657.9 ms (worker), queued median 14 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 54927 | 1799.6 | 33 |
| unmap | 37943 | 3644.1 | 96 |
| object alloc | 888 | 120.7 | 136 |
| object free | 743 | 45.3 | 61 |
| invalidate | 599 | 65.1 | 109 |
| total | 95100 | 5674.9 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 175 | 299 | 233 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 73 | 523 | 319 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 50 | 1179 | 347 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 20 | 2457 | 811 |
| 0 | 2 | 5 | 0 | 0 | 1 | 1 | 12 | 1190 | 567 |
| 0 | 3 | 5 | 0 | 0 | 1 | 1 | 12 | 1578 | 415 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 11 | 1279 | 818 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 10 | 628 | 340 |
| 0 | 2 | 5 | 0 | 0 | 5 | 7 | 10 | 31172 | 31094 |
| 0 | 5 | 10 | 0 | 0 | 1 | 1 | 9 | 3758 | 2602 |
| 0 | 3 | 5 | 0 | 0 | 5 | 7 | 9 | 32890 | 31268 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 8 | 1115 | 566 |
| 0 | 2 | 5 | 0 | 0 | 9 | 13 | 8 | 61218 | 60845 |
| 0 | 2 | 5 | 0 | 0 | 4 | 6 | 8 | 31170 | 31099 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 23553 | 2879696 |
| + CARAXES_INTR_PAGE=1 | 23553 | 2879696 |
| + CARAXES_BAR2_MEMFD=1 | 23553 | 2879696 |
| + both | 23553 | 2879696 |

Median per access by class, us: bar0-other 277.0, bar1-write 48.0, bar2-flush-read 97.0, gsp-falcon 36.0, gsp-queue-doorbell 122.0, mmu-invalidate 36.0, pcfg-mirror (intr re-arm) 128.5, usermode page, other 35.0, usermode-doorbell 1801.0


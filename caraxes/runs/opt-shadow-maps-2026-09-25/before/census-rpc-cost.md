# rpc-cost: before/census-trace.txt.xz

Window 148950484 .. 169293695 us (20343.2 ms), 23497 trapped accesses, 2189.7 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 776456 | 35.5 % | 85.0 | 838.0 |
| bar1-write | 7284 | 354810 | 16.2 % | 31.0 | 56.0 |
| gsp-queue-doorbell | 1087 | 254668 | 11.6 % | 101.0 | 640.0 |
| pcfg-mirror (intr re-arm) | 2054 | 250551 | 11.4 % | 62.0 | 1391.0 |
| mmu-invalidate | 5590 | 237386 | 10.8 % | 32.0 | 64.0 |
| bar0-other | 616 | 158255 | 7.2 % | 313.5 | 1110.0 |
| gsp-falcon | 3261 | 122484 | 5.6 % | 31.0 | 55.0 |
| usermode-doorbell | 120 | 19421 | 0.9 % | 1748.0 | 1889.0 |
| usermode page, other | 360 | 15647 | 0.7 % | 31.0 | 59.0 |
| all | 23497 | 2189678 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 3 | 28 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 4 | 9 | 22 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 7 | 9 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 7 | 10 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 3 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 7 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 7 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 2 | 3 | 7 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 3 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 11 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 3 | 12 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 9 | 10 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 8 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 7 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 3 | 5 | 8 | 10 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 4 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 5 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 5 | 7 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 13 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 3 | 13 | 13 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 2 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 3 | 3 | 4 | 6 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 16 | 16 | 16 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 8 | 8 | 8 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x1a9800000 | 0xc1 | 638 |
| 0x1e9600000 | 0x5 | 457 |
| 0x1e9601000 | 0xc5 | 12 |
| 0x1e9601000 | 0x5 | 6 |
| 0x1a9804000 | 0xc1 | 5 |

282 plan(s) applied by the worker: 98018 leaves unmapped, 127790 mapped in 55025 RM map call(s); walk median 14 us (server thread), apply median 510 us p90 9851 us total 4784.5 ms (worker), queued median 12 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 55025 | 1576.6 | 29 |
| unmap | 37923 | 3074.6 | 81 |
| object alloc | 894 | 113.3 | 127 |
| object free | 813 | 46.4 | 57 |
| invalidate | 0 | 0.0 | 0 |
| total | 94655 | 4811.0 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 182 | 270 | 220 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 74 | 461 | 303 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 55 | 1220 | 344 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 21 | 2299 | 701 |
| 0 | 2 | 5 | 0 | 0 | 1 | 1 | 13 | 715 | 380 |
| 0 | 5 | 10 | 0 | 0 | 1 | 1 | 10 | 2470 | 2221 |
| 0 | 2 | 5 | 0 | 0 | 9 | 13 | 10 | 61116 | 60940 |
| 0 | 2 | 5 | 0 | 0 | 5 | 7 | 10 | 31100 | 30738 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 9 | 831 | 523 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 9 | 1305 | 776 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 7 | 393 | 310 |
| 0 | 6 | 10 | 0 | 0 | 0 | 0 | 6 | 1141 | 1050 |
| 0 | 2 | 5 | 0 | 0 | 0 | 3 | 6 | 914 | 790 |
| 0 | 3 | 5 | 0 | 0 | 1 | 1 | 6 | 1552 | 987 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 23497 | 2189678 |
| + CARAXES_INTR_PAGE=1 | 23497 | 2189678 |
| + CARAXES_BAR2_MEMFD=1 | 23497 | 2189678 |
| + both | 23497 | 2189678 |

Median per access by class, us: bar0-other 313.5, bar1-write 31.0, bar2-flush-read 85.0, gsp-falcon 31.0, gsp-queue-doorbell 101.0, mmu-invalidate 32.0, pcfg-mirror (intr re-arm) 62.0, usermode page, other 31.0, usermode-doorbell 1748.0


# rpc-cost: final/census-trace.txt.xz

Window 139968188 .. 159893602 us (19925.4 ms), 23417 trapped accesses, 2212.4 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 784560 | 35.5 % | 87.0 | 828.0 |
| bar1-write | 7284 | 386890 | 17.5 % | 32.0 | 62.0 |
| gsp-queue-doorbell | 1087 | 259405 | 11.7 % | 114.0 | 663.0 |
| mmu-invalidate | 5590 | 244300 | 11.0 % | 33.0 | 68.0 |
| pcfg-mirror (intr re-arm) | 1974 | 211782 | 9.6 % | 62.0 | 1147.0 |
| bar0-other | 616 | 163171 | 7.4 % | 354.5 | 1222.0 |
| gsp-falcon | 3261 | 130274 | 5.9 % | 33.0 | 60.0 |
| usermode-doorbell | 120 | 17325 | 0.8 % | 1735.0 | 1791.0 |
| usermode page, other | 360 | 14736 | 0.7 % | 32.0 | 65.0 |
| all | 23417 | 2212443 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 4 | 14 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 4 | 10 | 43 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 5 | 52 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 7 | 15 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 8 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 6 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 4 | 18 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 5 | 14 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 3 | 13 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 7 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 10 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 3 | 4 | 15 | 10 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 5 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 4 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 3 | 8 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 3 | 21 | 21 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 3 | 8 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 3 | 7 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 3 | 3 | 11 | 6 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 2 | 2 | 12 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 3 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 3 | 10 | 10 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 3 | 3 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 2 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 2 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 2 | 3 | 3 | 0 |
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

282 plan(s) applied by the worker: 98018 leaves unmapped, 127790 mapped in 55661 RM map call(s); walk median 14 us (server thread), apply median 347 us p90 8528 us total 4180.2 ms (worker), queued median 11 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 55661 | 1601.4 | 29 |
| unmap | 618 | 50.3 | 81 |
| object alloc | 893 | 62.2 | 70 |
| object free | 748 | 2464.4 | 3295 |
| invalidate | 0 | 0.0 | 0 |
| total | 57920 | 4178.2 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 177 | 286 | 229 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 78 | 474 | 295 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 52 | 1226 | 316 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 17 | 2362 | 806 |
| 0 | 5 | 10 | 0 | 0 | 1 | 1 | 14 | 2542 | 2055 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 11 | 1040 | 542 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 11 | 329 | 287 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 10 | 1126 | 779 |
| 0 | 2 | 5 | 0 | 0 | 1 | 1 | 9 | 2429 | 415 |
| 0 | 2 | 5 | 0 | 0 | 9 | 13 | 9 | 60896 | 60726 |
| 0 | 2 | 5 | 0 | 0 | 4 | 6 | 9 | 31021 | 30594 |
| 0 | 5 | 10 | 0 | 0 | 5 | 7 | 9 | 31915 | 31555 |
| 0 | 2 | 5 | 0 | 0 | 5 | 7 | 8 | 31107 | 30904 |
| 0 | 3 | 5 | 0 | 0 | 1 | 1 | 7 | 1642 | 935 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 23417 | 2212443 |
| + CARAXES_INTR_PAGE=1 | 23417 | 2212443 |
| + CARAXES_BAR2_MEMFD=1 | 23417 | 2212443 |
| + both | 23417 | 2212443 |

Median per access by class, us: bar0-other 354.5, bar1-write 32.0, bar2-flush-read 87.0, gsp-falcon 33.0, gsp-queue-doorbell 114.0, mmu-invalidate 33.0, pcfg-mirror (intr re-arm) 62.0, usermode page, other 32.0, usermode-doorbell 1735.0


# rpc-cost: gather/census-trace.txt.xz

Window 140401826 .. 156577296 us (16175.5 ms), 23278 trapped accesses, 2308.8 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 767000 | 33.2 % | 93.0 | 719.0 |
| bar1-write | 7284 | 366213 | 15.9 % | 32.0 | 62.0 |
| gsp-queue-doorbell | 1087 | 304103 | 13.2 % | 115.0 | 1018.0 |
| mmu-invalidate | 5590 | 290124 | 12.6 % | 34.0 | 73.0 |
| pcfg-mirror (intr re-arm) | 1835 | 177184 | 7.7 % | 64.0 | 1272.0 |
| gsp-falcon | 3261 | 161619 | 7.0 % | 34.0 | 69.0 |
| bar0-other | 616 | 156880 | 6.8 % | 164.0 | 1281.0 |
| usermode-doorbell | 120 | 68516 | 3.0 % | 1582.0 | 1891.0 |
| usermode page, other | 360 | 17204 | 0.7 % | 32.0 | 62.0 |
| all | 23278 | 2308843 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 4 | 137 | 144 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 10 | 2145 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 4 | 1656 | 0 |
| GSP_RM_ALLOC class 0xc46f | 120 | 4 | 9 | 30 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 6 | 57 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 11 | 89 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 8 | 82 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 4 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 3 | 7 | 56 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 9 | 12 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 7 | 9 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 4 | 14 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 4 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 3 | 3 | 3 | 10 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 6 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 3 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 2 | 3 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 2 | 3 | 3 | 6 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 2 | 2 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x1a9800000 | 0xc1 | 638 |
| 0x1e9600000 | 0x5 | 457 |
| 0x1e9601000 | 0xc5 | 12 |
| 0x1e9601000 | 0x5 | 6 |
| 0x1a9804000 | 0xc1 | 5 |

282 plan(s) applied by the worker: 98018 leaves unmapped, 127790 mapped in 1066 RM map call(s); walk median 14 us (server thread), apply median 367 us p90 3946 us total 820.5 ms (worker), queued median 10 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1066 | 136.5 | 128 |
| unmap | 704 | 99.4 | 141 |
| object alloc | 889 | 115.8 | 130 |
| object free | 895 | 149.5 | 167 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 136 | 369.2 | 2715 |
| gather free | 136 | 21.6 | 159 |
| total | 3826 | 892.1 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 175 | 282 | 224 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 74 | 488 | 298 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 52 | 1184 | 344 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 17 | 2787 | 1039 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 11 | 1081 | 545 |
| 0 | 2 | 5 | 0 | 0 | 5 | 7 | 11 | 31069 | 30842 |
| 0 | 5 | 10 | 0 | 0 | 1 | 1 | 10 | 3914 | 993 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 10 | 1194 | 783 |
| 0 | 2 | 5 | 0 | 0 | 9 | 13 | 10 | 61075 | 60800 |
| 0 | 3 | 5 | 0 | 0 | 1 | 1 | 9 | 1612 | 484 |
| 0 | 2 | 5 | 0 | 0 | 1 | 1 | 9 | 3515 | 597 |
| 0 | 1 | 5 | 0 | 0 | 0 | 1 | 7 | 414 | 249 |
| 0 | 6 | 10 | 0 | 0 | 0 | 0 | 6 | 1156 | 993 |
| 0 | 2 | 5 | 0 | 0 | 1 | 10 | 6 | 3433 | 2089 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 23278 | 2308843 |
| + CARAXES_INTR_PAGE=1 | 23278 | 2308843 |
| + CARAXES_BAR2_MEMFD=1 | 23278 | 2308843 |
| + both | 23278 | 2308843 |

Median per access by class, us: bar0-other 164.0, bar1-write 32.0, bar2-flush-read 93.0, gsp-falcon 34.0, gsp-queue-doorbell 115.0, mmu-invalidate 34.0, pcfg-mirror (intr re-arm) 64.0, usermode page, other 32.0, usermode-doorbell 1582.0


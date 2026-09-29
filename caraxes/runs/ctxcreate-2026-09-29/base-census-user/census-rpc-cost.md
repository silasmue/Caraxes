# rpc-cost: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census-user/census-trace.txt

Window 135277512 .. 162842314 us (27564.8 ms), 106805 trapped accesses, 9770.9 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| mmu-invalidate | 67316 | 6285599 | 64.3 % | 55.0 | 151.0 |
| bar2-flush-read | 19613 | 1753907 | 18.0 % | 53.0 | 141.0 |
| gsp-queue-doorbell | 2860 | 917276 | 9.4 % | 145.0 | 839.0 |
| gsp-falcon | 8445 | 370531 | 3.8 % | 35.0 | 65.0 |
| usermode-doorbell | 300 | 190883 | 2.0 % | 1411.0 | 1568.0 |
| pci-config | 6669 | 122564 | 1.3 % | 14.0 | 28.0 |
| bar0-other | 688 | 88175 | 0.9 % | 118.5 | 373.0 |
| usermode page, other | 900 | 41549 | 0.4 % | 34.0 | 67.0 |
| pcfg-mirror (intr re-arm) | 14 | 455 | 0.0 % | 30.5 | 42.0 |
| all | 106805 | 9770939 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 1059 | 3 | 4 | 117 | 780 |
| GSP_RM_ALLOC class 0xc46f | 300 | 4 | 9 | 2070 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 300 | 3 | 4 | 18 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 120 | 3 | 3 | 22 | 0 |
| GSP_RM_ALLOC class 0x79 | 120 | 3 | 4 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 60 | 3 | 9 | 24 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 60 | 3 | 4 | 55 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 70 | 3 | 7 | 11 | 0 |
| GSP_RM_ALLOC class 0xa06c | 60 | 3 | 7 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 60 | 3 | 4 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0c | 42 | 3 | 3 | 11 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 28 | 3 | 8 | 14 | 28 |
| GSP_RM_CONTROL cmd 0x802009 | 29 | 3 | 8 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 30 | 3 | 4 | 20 | 0 |
| GSP_RM_ALLOC class 0x0 | 29 | 3 | 4 | 16 | 0 |
| GSP_RM_ALLOC class 0x80 | 29 | 3 | 4 | 13 | 0 |
| GSP_RM_ALLOC class 0x2080 | 29 | 3 | 3 | 16 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 28 | 3 | 4 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 29 | 3 | 4 | 8 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 28 | 3 | 5 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 28 | 3 | 4 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 15 | 3 | 9 | 16 | 15 |
| GSP_RM_CONTROL cmd 0x20801823 | 14 | 3 | 8 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 14 | 3 | 8 | 9 | 0 |
| GSP_RM_ALLOC class 0x83de | 15 | 3 | 3 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 14 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 15 | 3 | 3 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 14 | 3 | 9 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 14 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 15 | 3 | 7 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 14 | 3 | 4 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 14 | 3 | 4 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 14 | 3 | 3 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 15 | 3 | 3 | 4 | 15 |
| GSP_RM_CONTROL cmd 0x20810108 | 14 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2081 | 14 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 15 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a02 | 3 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 8612 |
| 0xa9600000 | 0x5 | 1174 |
| 0xa9601000 | 0xc5 | 30 |
| 0xa9601000 | 0x5 | 15 |
| 0x69804000 | 0xc1 | 14 |

5397 plan(s) applied by the worker: 277537 leaves unmapped, 326038 mapped in 19229 RM map call(s); walk median 6 us (server thread), apply median 49 us p90 2082 us total 4090.6 ms (worker), queued median 9 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 19229 | 940.9 | 49 |
| unmap | 18507 | 785.7 | 42 |
| object alloc | 14270 | 652.7 | 46 |
| object free | 14285 | 579.6 | 41 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 342 | 909.9 | 2660 |
| gather free | 342 | 42.5 | 124 |
| total | 66975 | 3911.3 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 4261 | 391 | 232 |
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 2095 | 290 | 191 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 210 | 888 | 251 |
| 0 | 1 | 6 | 0 | 0 | 0 | 0 | 113 | 487 | 344 |
| 0 | 1 | 18 | 0 | 0 | 0 | 0 | 82 | 2210 | 2003 |
| 0 | 2 | 6 | 0 | 0 | 0 | 0 | 70 | 572 | 435 |
| 0 | 1 | 19 | 0 | 0 | 0 | 0 | 62 | 2286 | 2109 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 61 | 2311 | 617 |
| 0 | 2 | 20 | 0 | 0 | 0 | 0 | 58 | 2632 | 2445 |
| 0 | 1 | 17 | 0 | 0 | 0 | 0 | 58 | 2086 | 1961 |
| 0 | 2 | 19 | 0 | 0 | 0 | 0 | 57 | 2603 | 2321 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 43 | 1127 | 496 |
| 0 | 2 | 17 | 0 | 0 | 0 | 0 | 41 | 2379 | 1951 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 38 | 624 | 369 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 106805 | 9770939 |
| + CARAXES_INTR_PAGE=1 | 106805 | 9770939 |
| + CARAXES_BAR2_MEMFD=1 | 106805 | 9770939 |
| + both | 106805 | 9770939 |

Median per access by class, us: bar0-other 118.5, bar2-flush-read 53.0, gsp-falcon 35.0, gsp-queue-doorbell 145.0, mmu-invalidate 55.0, pcfg-mirror (intr re-arm) 30.5, pci-config 14.0, usermode page, other 34.0, usermode-doorbell 1411.0


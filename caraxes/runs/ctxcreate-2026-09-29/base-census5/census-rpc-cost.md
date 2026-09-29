# rpc-cost: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census5/census-trace.txt

Window 132868126 .. 158727459 us (25859.3 ms), 101211 trapped accesses, 8104.8 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| mmu-invalidate | 62235 | 5127642 | 63.3 % | 45.0 | 147.0 |
| bar2-flush-read | 19613 | 1497888 | 18.5 % | 52.0 | 125.0 |
| gsp-queue-doorbell | 2860 | 806052 | 9.9 % | 128.0 | 732.0 |
| gsp-falcon | 8445 | 350328 | 4.3 % | 33.0 | 60.0 |
| pci-config | 6156 | 108814 | 1.3 % | 14.0 | 27.0 |
| usermode-doorbell | 300 | 106575 | 1.3 % | 1165.0 | 1422.0 |
| bar0-other | 688 | 64346 | 0.8 % | 82.5 | 313.0 |
| usermode page, other | 900 | 42688 | 0.5 % | 32.0 | 64.0 |
| pcfg-mirror (intr re-arm) | 14 | 508 | 0.0 % | 30.5 | 48.0 |
| all | 101211 | 8104841 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 1059 | 3 | 4 | 2239 | 618 |
| GSP_RM_ALLOC class 0xc5b5 | 300 | 3 | 6 | 1581 | 0 |
| GSP_RM_ALLOC class 0xc46f | 300 | 4 | 8 | 18 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 120 | 3 | 7 | 9 | 0 |
| GSP_RM_ALLOC class 0x79 | 120 | 3 | 4 | 14 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 60 | 3 | 3 | 32 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 70 | 2 | 3 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 60 | 2 | 7 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 60 | 3 | 4 | 9 | 0 |
| GSP_RM_ALLOC class 0xa06c | 60 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0c | 42 | 3 | 8 | 13 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 28 | 3 | 8 | 31 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 29 | 3 | 8 | 17 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 28 | 3 | 8 | 10 | 0 |
| GSP_RM_ALLOC class 0x2080 | 29 | 3 | 8 | 15 | 0 |
| GSP_RM_ALLOC class 0x0 | 29 | 3 | 8 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 30 | 3 | 7 | 15 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 28 | 3 | 4 | 10 | 28 |
| GSP_RM_ALLOC class 0x80 | 29 | 3 | 3 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 28 | 3 | 3 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 29 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 15 | 3 | 10 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 15 | 3 | 8 | 9 | 15 |
| GSP_RM_ALLOC class 0x2081 | 14 | 3 | 7 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 14 | 3 | 7 | 11 | 0 |
| GSP_RM_ALLOC class 0x9067 | 15 | 3 | 7 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 15 | 3 | 6 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 14 | 3 | 7 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 14 | 3 | 4 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 14 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 14 | 3 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 14 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 14 | 3 | 4 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 14 | 3 | 4 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 14 | 3 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 14 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 14 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 15 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 15 | 3 | 3 | 3 | 15 |
| GSP_RM_CONTROL cmd 0x2080182a | 14 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20802a02 | 3 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 8612 |
| 0xa9600000 | 0x5 | 1174 |
| 0xa9601000 | 0xc5 | 30 |
| 0xa9601000 | 0x5 | 15 |
| 0x69804000 | 0xc1 | 14 |

5380 plan(s) applied by the worker: 202785 leaves unmapped, 326037 mapped in 19141 RM map call(s); walk median 5 us (server thread), apply median 35 us p90 1613 us total 3030.0 ms (worker), queued median 8 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 19141 | 569.5 | 30 |
| unmap | 18256 | 462.9 | 25 |
| object alloc | 14269 | 522.8 | 37 |
| object free | 14284 | 426.2 | 30 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 341 | 905.4 | 2655 |
| gather free | 341 | 28.8 | 84 |
| total | 66632 | 2915.6 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 4364 | 374 | 254 |
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 2200 | 280 | 200 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 208 | 824 | 254 |
| 0 | 1 | 14 | 0 | 0 | 0 | 0 | 109 | 1651 | 1511 |
| 0 | 1 | 6 | 0 | 0 | 0 | 0 | 95 | 461 | 360 |
| 0 | 2 | 16 | 0 | 0 | 0 | 0 | 87 | 2101 | 1953 |
| 0 | 1 | 15 | 0 | 0 | 0 | 0 | 79 | 1707 | 1585 |
| 0 | 1 | 16 | 0 | 0 | 0 | 0 | 75 | 1876 | 1700 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 69 | 2147 | 640 |
| 0 | 2 | 17 | 0 | 0 | 0 | 0 | 58 | 2339 | 2085 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 49 | 1157 | 464 |
| 0 | 2 | 15 | 0 | 0 | 0 | 0 | 41 | 2043 | 1848 |
| 0 | 2 | 10 | 0 | 0 | 0 | 0 | 38 | 1258 | 1006 |
| 0 | 2 | 18 | 0 | 0 | 0 | 0 | 38 | 2400 | 2095 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 101211 | 8104841 |
| + CARAXES_INTR_PAGE=1 | 101211 | 8104841 |
| + CARAXES_BAR2_MEMFD=1 | 101211 | 8104841 |
| + both | 101211 | 8104841 |

Median per access by class, us: bar0-other 82.5, bar2-flush-read 52.0, gsp-falcon 33.0, gsp-queue-doorbell 128.0, mmu-invalidate 45.0, pcfg-mirror (intr re-arm) 30.5, pci-config 14.0, usermode page, other 32.0, usermode-doorbell 1165.0


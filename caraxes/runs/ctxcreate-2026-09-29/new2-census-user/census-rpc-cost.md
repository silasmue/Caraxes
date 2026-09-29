# rpc-cost: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new2-census-user/census-trace.txt

Window 132147544 .. 155699812 us (23552.3 ms), 100034 trapped accesses, 8273.6 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| mmu-invalidate | 62084 | 5098857 | 61.6 % | 43.0 | 147.0 |
| bar2-flush-read | 19613 | 1486237 | 18.0 % | 52.0 | 121.0 |
| gsp-queue-doorbell | 2860 | 934435 | 11.3 % | 147.0 | 775.0 |
| gsp-falcon | 8445 | 369542 | 4.5 % | 34.0 | 62.0 |
| usermode-doorbell | 300 | 172111 | 2.1 % | 1302.5 | 1420.0 |
| pci-config | 5130 | 86634 | 1.0 % | 13.0 | 27.0 |
| bar0-other | 688 | 76286 | 0.9 % | 90.5 | 330.0 |
| usermode page, other | 900 | 49025 | 0.6 % | 33.0 | 65.0 |
| pcfg-mirror (intr re-arm) | 14 | 465 | 0.0 % | 31.0 | 41.0 |
| all | 100034 | 8273592 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 1059 | 3 | 4 | 1926 | 780 |
| GSP_RM_ALLOC class 0xc46f | 300 | 4 | 9 | 24 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 300 | 3 | 7 | 23 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 120 | 3 | 7 | 11 | 0 |
| GSP_RM_ALLOC class 0x79 | 120 | 3 | 3 | 8 | 0 |
| GSP_RM_ALLOC class 0xa06c | 60 | 3 | 7 | 28 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 70 | 3 | 3 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 60 | 3 | 7 | 11 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 60 | 3 | 4 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 60 | 3 | 4 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0c | 42 | 3 | 7 | 10 | 0 |
| GSP_RM_ALLOC class 0x0 | 29 | 3 | 9 | 22 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 28 | 3 | 9 | 12 | 28 |
| GSP_RM_CONTROL cmd 0x20801303 | 28 | 3 | 8 | 9 | 0 |
| GSP_RM_ALLOC class 0x80 | 29 | 3 | 4 | 13 | 0 |
| GSP_RM_ALLOC class 0x2080 | 29 | 3 | 3 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 28 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 30 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 29 | 3 | 3 | 10 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 28 | 2 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 29 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 14 | 3 | 11 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 15 | 3 | 7 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 15 | 3 | 7 | 8 | 15 |
| GSP_RM_CONTROL cmd 0x2080012f | 14 | 3 | 8 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 15 | 3 | 4 | 12 | 0 |
| GSP_RM_ALLOC class 0x83de | 15 | 3 | 8 | 10 | 0 |
| GSP_RM_ALLOC class 0x2081 | 14 | 3 | 3 | 16 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 14 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 14 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 14 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 15 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 14 | 2 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 15 | 3 | 3 | 3 | 15 |
| GSP_RM_CONTROL cmd 0x20800102 | 14 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 14 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 14 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a02 | 3 | 3 | 8 | 8 | 0 |
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

5441 plan(s) applied by the worker: 277536 leaves unmapped, 326037 mapped in 19197 RM map call(s); walk median 5 us (server thread), apply median 33 us p90 1626 us total 3122.7 ms (worker), queued median 8 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 19197 | 548.6 | 29 |
| unmap | 18456 | 508.2 | 28 |
| object alloc | 14273 | 526.9 | 37 |
| object free | 14288 | 418.6 | 29 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 319 | 888.5 | 2785 |
| gather free | 319 | 28.0 | 88 |
| total | 66852 | 2918.9 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 4401 | 341 | 239 |
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 2190 | 263 | 198 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 211 | 834 | 286 |
| 0 | 1 | 15 | 0 | 0 | 0 | 0 | 124 | 1708 | 1538 |
| 0 | 2 | 17 | 0 | 0 | 0 | 0 | 101 | 2247 | 2037 |
| 0 | 1 | 6 | 0 | 0 | 0 | 0 | 89 | 460 | 356 |
| 0 | 1 | 16 | 0 | 0 | 0 | 0 | 82 | 1870 | 1684 |
| 0 | 1 | 14 | 0 | 0 | 0 | 0 | 79 | 1593 | 1488 |
| 0 | 2 | 16 | 0 | 0 | 0 | 0 | 71 | 2116 | 1946 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 69 | 2110 | 612 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 49 | 1069 | 462 |
| 0 | 2 | 15 | 0 | 0 | 0 | 0 | 48 | 2002 | 1790 |
| 0 | 2 | 10 | 0 | 0 | 0 | 0 | 38 | 1202 | 1052 |
| 0 | 2 | 18 | 0 | 0 | 0 | 0 | 35 | 2424 | 2167 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 100034 | 8273592 |
| + CARAXES_INTR_PAGE=1 | 100034 | 8273592 |
| + CARAXES_BAR2_MEMFD=1 | 100034 | 8273592 |
| + both | 100034 | 8273592 |

Median per access by class, us: bar0-other 90.5, bar2-flush-read 52.0, gsp-falcon 34.0, gsp-queue-doorbell 147.0, mmu-invalidate 43.0, pcfg-mirror (intr re-arm) 31.0, pci-config 13.0, usermode page, other 33.0, usermode-doorbell 1302.5


# rpc-cost: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new-census/census-trace.txt

Window 133724132 .. 160219518 us (26495.4 ms), 100661 trapped accesses, 8438.5 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| mmu-invalidate | 61685 | 5112717 | 60.6 % | 43.0 | 145.0 |
| bar2-flush-read | 19613 | 1531639 | 18.2 % | 52.0 | 116.0 |
| gsp-queue-doorbell | 2860 | 938319 | 11.1 % | 153.0 | 802.0 |
| gsp-falcon | 8445 | 418985 | 5.0 % | 36.0 | 66.0 |
| usermode-doorbell | 300 | 172497 | 2.0 % | 1330.0 | 1497.0 |
| pci-config | 6156 | 118565 | 1.4 % | 15.0 | 30.0 |
| bar0-other | 688 | 99656 | 1.2 % | 138.5 | 431.0 |
| usermode page, other | 900 | 45648 | 0.5 % | 37.0 | 74.0 |
| pcfg-mirror (intr re-arm) | 14 | 503 | 0.0 % | 33.0 | 48.0 |
| all | 100661 | 8438529 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 1059 | 3 | 4 | 222 | 780 |
| GSP_RM_ALLOC class 0xc46f | 300 | 5 | 10 | 1882 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 120 | 3 | 5 | 1614 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 300 | 3 | 6 | 187 | 0 |
| GSP_RM_ALLOC class 0x79 | 120 | 3 | 4 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 70 | 3 | 7 | 18 | 0 |
| GSP_RM_ALLOC class 0xa06c | 60 | 3 | 7 | 17 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 60 | 3 | 6 | 15 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 60 | 3 | 4 | 7 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 60 | 3 | 5 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0c | 42 | 3 | 4 | 15 | 0 |
| GSP_RM_ALLOC class 0x2080 | 29 | 3 | 13 | 16 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 29 | 3 | 8 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 30 | 3 | 4 | 17 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 28 | 3 | 7 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 28 | 3 | 7 | 14 | 0 |
| GSP_RM_ALLOC class 0x0 | 29 | 3 | 4 | 19 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 28 | 3 | 4 | 10 | 28 |
| GSP_RM_ALLOC class 0x80 | 29 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 28 | 3 | 3 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 14 | 3 | 13 | 16 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 29 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 15 | 4 | 8 | 9 | 15 |
| GSP_RM_CONTROL cmd 0x2080012b | 15 | 3 | 10 | 15 | 0 |
| GSP_RM_ALLOC class 0x2081 | 14 | 3 | 3 | 24 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 15 | 3 | 9 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 14 | 3 | 8 | 11 | 0 |
| GSP_RM_ALLOC class 0x9067 | 15 | 3 | 9 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 14 | 3 | 7 | 12 | 0 |
| GSP_RM_ALLOC class 0x83de | 15 | 3 | 4 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 15 | 3 | 4 | 8 | 15 |
| GSP_RM_CONTROL cmd 0x2080182a | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 14 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 14 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 14 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a02 | 3 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 8612 |
| 0xa9600000 | 0x5 | 1174 |
| 0xa9601000 | 0xc5 | 30 |
| 0xa9601000 | 0x5 | 15 |
| 0x69804000 | 0xc1 | 14 |

5440 plan(s) applied by the worker: 277025 leaves unmapped, 326037 mapped in 19181 RM map call(s); walk median 5 us (server thread), apply median 34 us p90 1540 us total 3048.9 ms (worker), queued median 8 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 19181 | 523.7 | 27 |
| unmap | 18454 | 504.7 | 27 |
| object alloc | 14269 | 504.2 | 35 |
| object free | 14284 | 390.5 | 27 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 330 | 925.5 | 2804 |
| gather free | 330 | 30.2 | 91 |
| total | 66848 | 2878.7 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 4372 | 339 | 258 |
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 2165 | 259 | 201 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 209 | 835 | 277 |
| 0 | 1 | 15 | 0 | 0 | 0 | 0 | 136 | 1656 | 1507 |
| 0 | 2 | 16 | 0 | 0 | 0 | 0 | 132 | 2014 | 1889 |
| 0 | 1 | 14 | 0 | 0 | 0 | 0 | 101 | 1581 | 1504 |
| 0 | 1 | 6 | 0 | 0 | 0 | 0 | 100 | 476 | 312 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 66 | 2220 | 617 |
| 0 | 2 | 17 | 0 | 0 | 0 | 0 | 65 | 2184 | 1998 |
| 0 | 1 | 9 | 0 | 0 | 0 | 0 | 49 | 893 | 809 |
| 0 | 2 | 10 | 0 | 0 | 0 | 0 | 45 | 1175 | 979 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 39 | 942 | 518 |
| 0 | 1 | 12 | 0 | 0 | 0 | 0 | 34 | 1238 | 1103 |
| 0 | 1 | 13 | 0 | 0 | 0 | 0 | 34 | 1740 | 1275 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 100661 | 8438529 |
| + CARAXES_INTR_PAGE=1 | 100661 | 8438529 |
| + CARAXES_BAR2_MEMFD=1 | 100661 | 8438529 |
| + both | 100661 | 8438529 |

Median per access by class, us: bar0-other 138.5, bar2-flush-read 52.0, gsp-falcon 36.0, gsp-queue-doorbell 153.0, mmu-invalidate 43.0, pcfg-mirror (intr re-arm) 33.0, pci-config 15.0, usermode page, other 37.0, usermode-doorbell 1330.0


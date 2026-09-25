# rpc-cost: caraxes/runs/opt-msi-ioeventfd-2026-09-25/on-rearm/rl-trace.txt

Window 180004929 .. 191909514 us (11904.6 ms), 5428 trapped accesses, 343.3 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 525 | 152321 | 44.4 % | 104.0 | 1044.0 |
| pci-config | 3078 | 63482 | 18.5 % | 15.0 | 33.0 |
| mmu-invalidate | 960 | 61796 | 18.0 % | 39.0 | 89.0 |
| gsp-queue-doorbell | 194 | 33565 | 9.8 % | 120.0 | 410.0 |
| gsp-falcon | 582 | 27610 | 8.0 % | 37.0 | 59.0 |
| usermode page, other | 60 | 2489 | 0.7 % | 31.5 | 62.0 |
| bar0-other | 8 | 1981 | 0.6 % | 138.5 | 512.0 |
| pcfg-mirror (intr re-arm) | 1 | 33 | 0.0 % | 33.0 | 33.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 5428 | 343277 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 4 | 6 | 13 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 5 | 6 | 9 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 6 | 8 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 2 | 3 | 8 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 5 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 7 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 2 | 2 | 2 | 1 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 113 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

50 plan(s) applied by the worker: 1052 leaves unmapped, 21470 mapped in 16982 RM map call(s); walk median 58 us (server thread), apply median 2836 us p90 5636 us total 756.5 ms (worker), queued median 16 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 33 | 343 | 284 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 25 | 467 | 377 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 10 | 1418 | 375 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 5 | 5053 | 3412 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 4 | 712 | 400 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 4 | 2508 | 1617 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 3 | 3453 | 1548 |
| 0 | 2 | 5 | 0 | 0 | 5 | 0 | 2 | 28815 | 26172 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 2 | 1698 | 1661 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 2 | 352 | 343 |
| 0 | 8 | 15 | 0 | 0 | 1 | 0 | 1 | 3162 | 3162 |
| 0 | 8 | 15 | 0 | 0 | 4 | 16 | 1 | 22479 | 22479 |
| 0 | 113 | 145 | 0 | 0 | 9 | 0 | 1 | 59273 | 59273 |
| 0 | 10 | 20 | 0 | 0 | 2 | 4 | 1 | 11151 | 11151 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 5428 | 343277 |
| + CARAXES_INTR_PAGE=1 | 5428 | 343277 |
| + CARAXES_BAR2_MEMFD=1 | 5428 | 343277 |
| + both | 5428 | 343277 |

Median per access by class, us: bar0-other 138.5, bar2-flush-read 104.0, gsp-falcon 37.0, gsp-queue-doorbell 120.0, mmu-invalidate 39.0, pcfg-mirror (intr re-arm) 33.0, pci-config 15.0, usermode page, other 31.5


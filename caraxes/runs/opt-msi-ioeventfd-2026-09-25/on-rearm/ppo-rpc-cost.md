# rpc-cost: caraxes/runs/opt-msi-ioeventfd-2026-09-25/on-rearm/ppo-trace.txt

Window 193275151 .. 227107559 us (33832.4 ms), 10618 trapped accesses, 511.2 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| pci-config | 8208 | 201972 | 39.5 % | 17.0 | 38.0 |
| bar2-flush-read | 540 | 134539 | 26.3 % | 92.0 | 718.0 |
| gsp-queue-doorbell | 194 | 66864 | 13.1 % | 141.5 | 1213.0 |
| mmu-invalidate | 1005 | 61441 | 12.0 % | 36.0 | 80.0 |
| gsp-falcon | 582 | 33985 | 6.6 % | 33.0 | 68.0 |
| usermode page, other | 60 | 7337 | 1.4 % | 37.0 | 88.0 |
| usermode-doorbell | 20 | 3784 | 0.7 % | 1892.0 | 1979.0 |
| bar0-other | 8 | 1201 | 0.2 % | 88.0 | 382.0 |
| pcfg-mirror (intr re-arm) | 1 | 31 | 0.0 % | 31.0 | 31.0 |
| all | 10618 | 511154 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 5 | 16 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 5 | 13 | 16 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 4 | 23 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 6 | 12 | 12 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 10 | 17 | 17 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 8 | 11 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 16 | 16 | 16 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 15 | 15 | 15 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 2 | 5 | 5 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 122 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

53 plan(s) applied by the worker: 1052 leaves unmapped, 21482 mapped in 1338 RM map call(s); walk median 53 us (server thread), apply median 2031 us p90 4591 us total 154.6 ms (worker), queued median 13 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 38 | 264 | 252 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 31 | 367 | 330 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 11 | 1031 | 378 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 4 | 834 | 471 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 4 | 6126 | 3287 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 3 | 2296 | 860 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 3 | 1087 | 1078 |
| 0 | 2 | 5 | 0 | 0 | 3 | 0 | 2 | 21072 | 20578 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 2 | 1114 | 995 |
| 0 | 8 | 15 | 0 | 0 | 0 | 0 | 1 | 1418 | 1418 |
| 0 | 8 | 15 | 0 | 0 | 4 | 16 | 1 | 34535 | 34535 |
| 0 | 113 | 145 | 0 | 0 | 9 | 0 | 1 | 62273 | 62273 |
| 0 | 10 | 20 | 0 | 0 | 2 | 4 | 1 | 5095 | 5095 |
| 0 | 6 | 10 | 0 | 0 | 0 | 0 | 1 | 1807 | 1807 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 10618 | 511154 |
| + CARAXES_INTR_PAGE=1 | 10618 | 511154 |
| + CARAXES_BAR2_MEMFD=1 | 10618 | 511154 |
| + both | 10618 | 511154 |

Median per access by class, us: bar0-other 88.0, bar2-flush-read 92.0, gsp-falcon 33.0, gsp-queue-doorbell 141.5, mmu-invalidate 36.0, pcfg-mirror (intr re-arm) 31.0, pci-config 17.0, usermode page, other 37.0, usermode-doorbell 1892.0


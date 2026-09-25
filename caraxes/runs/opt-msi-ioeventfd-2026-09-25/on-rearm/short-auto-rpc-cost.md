# rpc-cost: caraxes/runs/opt-msi-ioeventfd-2026-09-25/on-rearm/short-auto-trace.txt

Window 233127653 .. 236688848 us (3561.2 ms), 3170 trapped accesses, 281.8 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 477 | 126148 | 44.8 % | 88.0 | 973.0 |
| gsp-queue-doorbell | 187 | 48497 | 17.2 % | 115.5 | 613.0 |
| mmu-invalidate | 830 | 47438 | 16.8 % | 33.0 | 88.0 |
| gsp-falcon | 561 | 29397 | 10.4 % | 33.0 | 73.0 |
| pci-config | 1026 | 25343 | 9.0 % | 24.0 | 38.0 |
| usermode page, other | 60 | 2756 | 1.0 % | 30.5 | 64.0 |
| bar0-other | 8 | 2162 | 0.8 % | 123.0 | 652.0 |
| pcfg-mirror (intr re-arm) | 1 | 31 | 0.0 % | 31.0 | 31.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 3170 | 281772 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 4 | 85 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 5 | 11 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 3 | 9 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 3 | 4 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 2 | 6 | 8 | 8 | 2 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 10 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 2 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 10 | 10 | 10 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 9 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 2 | 4 | 5 | 5 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 7 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 89 |
| 0xa9600000 | 0x5 | 73 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

38 plan(s) applied by the worker: 18050 leaves unmapped, 20131 mapped in 17323 RM map call(s); walk median 13 us (server thread), apply median 1448 us p90 26898 us total 2132.7 ms (worker), queued median 28 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 25 | 285 | 230 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 13 | 621 | 364 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 8 | 1310 | 394 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 3 | 3931 | 2837 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 3 | 4353 | 1732 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 2 | 2208 | 415 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 2 | 253 | 235 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 2 | 780 | 606 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 2 | 1518 | 1414 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 2 | 1304 | 1040 |
| 0 | 8 | 15 | 0 | 0 | 0 | 0 | 1 | 1273 | 1273 |
| 0 | 8 | 15 | 0 | 0 | 3 | 16 | 1 | 20290 | 20290 |
| 0 | 113 | 145 | 0 | 0 | 8 | 0 | 1 | 50790 | 50790 |
| 0 | 10 | 20 | 0 | 0 | 0 | 4 | 1 | 2057 | 2057 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 3170 | 281772 |
| + CARAXES_INTR_PAGE=1 | 3170 | 281772 |
| + CARAXES_BAR2_MEMFD=1 | 3170 | 281772 |
| + both | 3170 | 281772 |

Median per access by class, us: bar0-other 123.0, bar2-flush-read 88.0, gsp-falcon 33.0, gsp-queue-doorbell 115.5, mmu-invalidate 33.0, pcfg-mirror (intr re-arm) 31.0, pci-config 24.0, usermode page, other 30.5


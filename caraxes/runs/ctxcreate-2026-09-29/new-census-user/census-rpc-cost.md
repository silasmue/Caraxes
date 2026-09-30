# rpc-cost: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new-census-user/census-trace.txt

Window 133246889 .. 161263392 us (28016.5 ms), 107590 trapped accesses, 10899.0 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| mmu-invalidate | 68614 | 7074589 | 64.9 % | 65.0 | 161.0 |
| bar2-flush-read | 19613 | 1867649 | 17.1 % | 60.0 | 158.0 |
| gsp-queue-doorbell | 2860 | 1005797 | 9.2 % | 161.0 | 867.0 |
| gsp-falcon | 8445 | 457156 | 4.2 % | 38.0 | 74.0 |
| usermode-doorbell | 300 | 189130 | 1.7 % | 1421.0 | 1724.0 |
| pci-config | 6156 | 156621 | 1.4 % | 16.0 | 34.0 |
| bar0-other | 688 | 101768 | 0.9 % | 165.0 | 434.0 |
| usermode page, other | 900 | 45718 | 0.4 % | 37.0 | 71.0 |
| pcfg-mirror (intr re-arm) | 14 | 581 | 0.0 % | 40.5 | 64.0 |
| all | 107590 | 10899009 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 1059 | 3 | 4 | 22 | 780 |
| GSP_RM_ALLOC class 0xc46f | 300 | 5 | 10 | 1332 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 300 | 3 | 5 | 11 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 120 | 3 | 6 | 37 | 0 |
| GSP_RM_ALLOC class 0x79 | 120 | 3 | 7 | 23 | 0 |
| GSP_RM_ALLOC class 0xa06c | 60 | 3 | 9 | 31 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 70 | 3 | 7 | 27 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 60 | 3 | 6 | 20 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 60 | 3 | 6 | 14 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 60 | 3 | 4 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0c | 42 | 3 | 7 | 17 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 28 | 3 | 4 | 18 | 0 |
| GSP_RM_ALLOC class 0x80 | 29 | 3 | 5 | 27 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 29 | 3 | 7 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 28 | 3 | 8 | 17 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 29 | 3 | 9 | 17 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 30 | 3 | 6 | 9 | 0 |
| GSP_RM_ALLOC class 0x0 | 29 | 3 | 5 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 28 | 3 | 6 | 7 | 0 |
| GSP_RM_ALLOC class 0x2080 | 29 | 3 | 4 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 15 | 4 | 15 | 18 | 15 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 28 | 3 | 4 | 5 | 28 |
| GSP_RM_CONTROL cmd 0x2080182a | 14 | 3 | 8 | 39 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 15 | 3 | 4 | 31 | 15 |
| GSP_RM_CONTROL cmd 0x20803801 | 14 | 3 | 8 | 20 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 15 | 3 | 9 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 14 | 3 | 8 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 15 | 3 | 5 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 14 | 3 | 9 | 10 | 0 |
| GSP_RM_ALLOC class 0x2081 | 14 | 3 | 4 | 18 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 14 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 14 | 3 | 7 | 9 | 0 |
| GSP_RM_ALLOC class 0x83de | 15 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 14 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 14 | 3 | 4 | 7 | 0 |
| GSP_RM_ALLOC class 0x9067 | 15 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 14 | 3 | 5 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 14 | 3 | 4 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 14 | 3 | 4 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 14 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a02 | 3 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 8612 |
| 0xa9600000 | 0x5 | 1174 |
| 0xa9601000 | 0xc5 | 30 |
| 0xa9601000 | 0x5 | 15 |
| 0x69804000 | 0xc1 | 14 |

5438 plan(s) applied by the worker: 277537 leaves unmapped, 326038 mapped in 19147 RM map call(s); walk median 7 us (server thread), apply median 68 us p90 2134 us total 4774.5 ms (worker), queued median 10 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 19147 | 1169.4 | 61 |
| unmap | 18478 | 824.9 | 45 |
| object alloc | 14269 | 964.3 | 68 |
| object free | 14284 | 559.1 | 39 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 348 | 1020.7 | 2933 |
| gather free | 348 | 49.8 | 143 |
| total | 66874 | 4588.2 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 3824 | 424 | 259 |
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 1876 | 325 | 204 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 366 | 628 | 318 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 185 | 917 | 275 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 173 | 423 | 247 |
| 0 | 1 | 6 | 0 | 0 | 0 | 0 | 95 | 487 | 379 |
| 0 | 2 | 6 | 0 | 0 | 0 | 0 | 70 | 622 | 433 |
| 0 | 2 | 5 | 0 | 0 | 2 | 0 | 60 | 2467 | 449 |
| 0 | 1 | 18 | 0 | 0 | 0 | 0 | 44 | 2216 | 1968 |
| 0 | 2 | 19 | 0 | 0 | 0 | 0 | 40 | 2636 | 2392 |
| 0 | 1 | 7 | 0 | 0 | 0 | 0 | 38 | 655 | 473 |
| 0 | 1 | 5 | 0 | 0 | 2 | 0 | 37 | 1973 | 283 |
| 0 | 1 | 17 | 0 | 0 | 0 | 0 | 37 | 2199 | 1869 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 36 | 2258 | 631 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 107590 | 10899009 |
| + CARAXES_INTR_PAGE=1 | 107590 | 10899009 |
| + CARAXES_BAR2_MEMFD=1 | 107590 | 10899009 |
| + both | 107590 | 10899009 |

Median per access by class, us: bar0-other 165.0, bar2-flush-read 60.0, gsp-falcon 38.0, gsp-queue-doorbell 161.0, mmu-invalidate 65.0, pcfg-mirror (intr re-arm) 40.5, pci-config 16.0, usermode page, other 37.0, usermode-doorbell 1421.0


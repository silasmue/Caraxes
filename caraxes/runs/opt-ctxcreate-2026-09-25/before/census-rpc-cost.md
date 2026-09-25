# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/before/census-trace.txt

Window 143537255 .. 160182699 us (16645.4 ms), 18268 trapped accesses, 1751.2 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 762127 | 43.5 % | 90.0 | 759.0 |
| mmu-invalidate | 5590 | 363277 | 20.7 % | 34.0 | 82.0 |
| gsp-queue-doorbell | 1087 | 282263 | 16.1 % | 112.0 | 858.0 |
| gsp-falcon | 3261 | 129151 | 7.4 % | 33.0 | 62.0 |
| bar0-other | 616 | 89702 | 5.1 % | 143.0 | 740.0 |
| pci-config | 4104 | 71346 | 4.1 % | 14.0 | 29.0 |
| usermode-doorbell | 120 | 38741 | 2.2 % | 1856.0 | 1974.0 |
| usermode page, other | 360 | 14409 | 0.8 % | 30.0 | 62.0 |
| pcfg-mirror (intr re-arm) | 5 | 165 | 0.0 % | 29.0 | 51.0 |
| all | 18268 | 1751181 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 3 | 15 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 4 | 8 | 15 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 7 | 31 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 8 | 48 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 4 | 35 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 6 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 2 | 7 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 5 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 2 | 3 | 4 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 9 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 9 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 7 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 2 | 4 | 8 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 3 | 5 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 3 | 3 | 3 | 10 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 3 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 3 | 4 | 8 | 6 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 3 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 3 | 3 | 8 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 2 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 638 |
| 0xa9600000 | 0x5 | 457 |
| 0xa9601000 | 0xc5 | 12 |
| 0xa9601000 | 0x5 | 6 |
| 0x69804000 | 0xc1 | 5 |

298 plan(s) applied by the worker: 101596 leaves unmapped, 131368 mapped in 1087 RM map call(s); walk median 17 us (server thread), apply median 353 us p90 3490 us total 727.9 ms (worker), queued median 10 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1087 | 72.7 | 67 |
| unmap | 752 | 74.8 | 100 |
| object alloc | 889 | 91.4 | 103 |
| object free | 895 | 82.6 | 92 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 133 | 360.9 | 2713 |
| gather free | 133 | 12.0 | 90 |
| total | 3889 | 694.4 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 172 | 290 | 224 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 85 | 557 | 329 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 48 | 1312 | 347 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 24 | 3671 | 813 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 24 | 1528 | 368 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 18 | 4261 | 2885 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 16 | 396 | 270 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 14 | 912 | 640 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 14 | 1428 | 367 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 10 | 1278 | 859 |
| 0 | 2 | 5 | 0 | 0 | 5 | 2 | 10 | 35582 | 30877 |
| 0 | 2 | 5 | 0 | 0 | 9 | 4 | 9 | 61045 | 60860 |
| 0 | 3 | 5 | 0 | 0 | 5 | 2 | 9 | 32469 | 31843 |
| 0 | 8 | 15 | 0 | 0 | 0 | 0 | 6 | 1260 | 1114 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 18268 | 1751181 |
| + CARAXES_INTR_PAGE=1 | 18268 | 1751181 |
| + CARAXES_BAR2_MEMFD=1 | 18268 | 1751181 |
| + both | 18268 | 1751181 |

Median per access by class, us: bar0-other 143.0, bar2-flush-read 90.0, gsp-falcon 33.0, gsp-queue-doorbell 112.0, mmu-invalidate 34.0, pcfg-mirror (intr re-arm) 29.0, pci-config 14.0, usermode page, other 30.0, usermode-doorbell 1856.0


# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/after/census-trace.txt

Window 145142386 .. 162390320 us (17247.9 ms), 18268 trapped accesses, 2063.4 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 863874 | 41.9 % | 104.0 | 850.0 |
| mmu-invalidate | 5590 | 452524 | 21.9 % | 41.0 | 123.0 |
| gsp-queue-doorbell | 1087 | 316347 | 15.3 % | 137.0 | 989.0 |
| gsp-falcon | 3261 | 179218 | 8.7 % | 37.0 | 80.0 |
| bar0-other | 616 | 106222 | 5.1 % | 163.0 | 829.0 |
| pci-config | 4104 | 77223 | 3.7 % | 15.0 | 31.0 |
| usermode-doorbell | 120 | 49201 | 2.4 % | 1416.5 | 1938.0 |
| usermode page, other | 360 | 18642 | 0.9 % | 38.0 | 76.0 |
| pcfg-mirror (intr re-arm) | 5 | 161 | 0.0 % | 32.0 | 35.0 |
| all | 18268 | 2063412 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 5 | 77 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 4 | 10 | 91 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 8 | 89 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 5 | 274 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 7 | 117 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 13 | 101 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 10 | 71 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 4 | 68 | 68 | 10 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 61 | 61 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 12 | 80 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 10 | 26 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 10 | 16 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 4 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 9 | 27 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 9 | 20 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 4 | 10 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 6 | 9 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 5 | 5 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 3 | 4 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 4 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 3 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 3 | 4 | 5 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 3 | 4 | 5 | 6 |
| GSP_RM_ALLOC class 0x2081 | 5 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 3 | 3 | 4 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 638 |
| 0xa9600000 | 0x5 | 457 |
| 0xa9601000 | 0xc5 | 12 |
| 0xa9601000 | 0x5 | 6 |
| 0x69804000 | 0xc1 | 5 |

340 plan(s) applied by the worker: 101596 leaves unmapped, 131368 mapped in 1067 RM map call(s); walk median 25 us (server thread), apply median 291 us p90 3077 us total 820.7 ms (worker), queued median 12 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1067 | 49.5 | 46 |
| unmap | 766 | 57.5 | 75 |
| object alloc | 889 | 59.5 | 67 |
| object free | 895 | 59.5 | 67 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 136 | 492.9 | 3624 |
| gather free | 136 | 24.0 | 176 |
| total | 3889 | 742.9 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 175 | 332 | 232 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 86 | 690 | 393 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 48 | 1402 | 372 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 24 | 4803 | 3069 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 22 | 1366 | 398 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 17 | 3870 | 846 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 16 | 438 | 290 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 15 | 1473 | 552 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 14 | 1898 | 1114 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 10 | 1176 | 878 |
| 0 | 6 | 5 | 0 | 0 | 0 | 0 | 6 | 744 | 673 |
| 0 | 2 | 5 | 0 | 0 | 4 | 2 | 6 | 33072 | 31181 |
| 0 | 4 | 5 | 0 | 0 | 0 | 4 | 5 | 607 | 525 |
| 0 | 5 | 10 | 0 | 0 | 0 | 23 | 5 | 3896 | 3594 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 18268 | 2063412 |
| + CARAXES_INTR_PAGE=1 | 18268 | 2063412 |
| + CARAXES_BAR2_MEMFD=1 | 18268 | 2063412 |
| + both | 18268 | 2063412 |

Median per access by class, us: bar0-other 163.0, bar2-flush-read 104.0, gsp-falcon 37.0, gsp-queue-doorbell 137.0, mmu-invalidate 41.0, pcfg-mirror (intr re-arm) 32.0, pci-config 15.0, usermode page, other 38.0, usermode-doorbell 1416.5


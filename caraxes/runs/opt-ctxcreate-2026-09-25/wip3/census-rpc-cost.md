# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/wip3/census-trace.txt

Window 147199282 .. 165423051 us (18223.8 ms), 18268 trapped accesses, 2241.6 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 969044 | 43.2 % | 116.0 | 970.0 |
| mmu-invalidate | 5590 | 478357 | 21.3 % | 45.0 | 122.0 |
| gsp-queue-doorbell | 1087 | 325747 | 14.5 % | 149.0 | 989.0 |
| gsp-falcon | 3261 | 212752 | 9.5 % | 40.0 | 87.0 |
| pci-config | 4104 | 112992 | 5.0 % | 16.0 | 37.0 |
| bar0-other | 616 | 107536 | 4.8 % | 225.0 | 814.0 |
| usermode page, other | 360 | 27546 | 1.2 % | 48.0 | 99.0 |
| usermode-doorbell | 120 | 7406 | 0.3 % | 1865.0 | 1888.0 |
| pcfg-mirror (intr re-arm) | 5 | 196 | 0.0 % | 39.0 | 45.0 |
| all | 18268 | 2241576 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 5 | 2473 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 5 | 13 | 1212 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 5 | 32 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 13 | 22 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 10 | 97 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 4 | 10 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 11 | 32 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 4 | 9 | 25 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 4 | 21 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 4 | 14 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 4 | 16 | 24 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 4 | 4 | 35 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 4 | 20 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 6 | 16 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 11 | 13 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 9 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 3 | 6 | 12 | 10 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 4 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 4 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 3 | 10 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 4 | 6 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 3 | 4 | 12 | 6 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 4 | 12 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 3 | 13 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 3 | 11 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 9 | 9 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 4 | 4 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 638 |
| 0xa9600000 | 0x5 | 457 |
| 0xa9601000 | 0xc5 | 12 |
| 0xa9601000 | 0x5 | 6 |
| 0x69804000 | 0xc1 | 5 |

340 plan(s) applied by the worker: 101596 leaves unmapped, 131368 mapped in 1058 RM map call(s); walk median 28 us (server thread), apply median 332 us p90 3748 us total 1065.0 ms (worker), queued median 13 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1058 | 86.6 | 82 |
| unmap | 748 | 89.2 | 119 |
| object alloc | 891 | 109.2 | 123 |
| object free | 897 | 102.6 | 114 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 137 | 623.2 | 4549 |
| gather free | 137 | 23.5 | 172 |
| total | 3868 | 1034.3 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 165 | 369 | 226 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 76 | 792 | 331 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 46 | 1356 | 422 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 30 | 2416 | 519 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 23 | 6132 | 1173 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 21 | 594 | 313 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 16 | 2050 | 509 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 14 | 5406 | 1008 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 13 | 1176 | 865 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 9 | 1369 | 1008 |
| 0 | 2 | 5 | 0 | 0 | 9 | 4 | 9 | 62184 | 61422 |
| 0 | 2 | 5 | 0 | 0 | 5 | 2 | 9 | 31916 | 31261 |
| 0 | 2 | 5 | 0 | 0 | 1 | 28 | 6 | 6964 | 5793 |
| 0 | 6 | 5 | 0 | 0 | 0 | 0 | 6 | 958 | 723 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 18268 | 2241576 |
| + CARAXES_INTR_PAGE=1 | 18268 | 2241576 |
| + CARAXES_BAR2_MEMFD=1 | 18268 | 2241576 |
| + both | 18268 | 2241576 |

Median per access by class, us: bar0-other 225.0, bar2-flush-read 116.0, gsp-falcon 40.0, gsp-queue-doorbell 149.0, mmu-invalidate 45.0, pcfg-mirror (intr re-arm) 39.0, pci-config 16.0, usermode page, other 48.0, usermode-doorbell 1865.0


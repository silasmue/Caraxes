# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/part2/census-trace.txt

Window 145890605 .. 162738468 us (16847.9 ms), 17755 trapped accesses, 1967.1 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 854028 | 43.4 % | 102.0 | 843.0 |
| mmu-invalidate | 5590 | 405062 | 20.6 % | 39.0 | 105.0 |
| gsp-queue-doorbell | 1087 | 299891 | 15.2 % | 133.0 | 918.0 |
| gsp-falcon | 3261 | 174577 | 8.9 % | 37.0 | 69.0 |
| bar0-other | 616 | 104898 | 5.3 % | 164.0 | 859.0 |
| pci-config | 3591 | 76844 | 3.9 % | 16.0 | 32.0 |
| usermode-doorbell | 120 | 32126 | 1.6 % | 1488.0 | 1806.0 |
| usermode page, other | 360 | 19397 | 1.0 % | 37.0 | 76.0 |
| pcfg-mirror (intr re-arm) | 5 | 238 | 0.0 % | 45.0 | 78.0 |
| all | 17755 | 1967061 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 4 | 17 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 5 | 11 | 42 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 8 | 19 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 6 | 13 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 4 | 9 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 12 | 75 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 8 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 4 | 81 | 81 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 6 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 4 | 11 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 4 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 16 | 19 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 12 | 18 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 10 | 16 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 4 | 9 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 3 | 4 | 8 | 10 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 15 | 15 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 4 | 4 | 12 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 4 | 4 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 3 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 3 | 3 | 4 | 6 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 4 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 3 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 4 | 4 | 0 |
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

340 plan(s) applied by the worker: 101596 leaves unmapped, 131368 mapped in 1063 RM map call(s); walk median 26 us (server thread), apply median 326 us p90 3474 us total 818.3 ms (worker), queued median 12 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1063 | 91.9 | 86 |
| unmap | 759 | 69.4 | 91 |
| object alloc | 889 | 101.9 | 115 |
| object free | 895 | 63.1 | 70 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 131 | 405.2 | 3093 |
| gather free | 131 | 14.7 | 112 |
| total | 3868 | 746.1 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 177 | 327 | 218 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 78 | 636 | 328 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 48 | 1199 | 406 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 28 | 2160 | 378 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 17 | 4865 | 2971 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 15 | 548 | 261 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 14 | 3370 | 960 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 14 | 1712 | 389 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 11 | 900 | 581 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 9 | 1113 | 837 |
| 0 | 2 | 5 | 0 | 0 | 9 | 2 | 8 | 33229 | 31481 |
| 0 | 5 | 10 | 0 | 0 | 2 | 0 | 7 | 5376 | 3967 |
| 0 | 2 | 5 | 0 | 0 | 17 | 4 | 7 | 61595 | 61082 |
| 0 | 4 | 5 | 0 | 0 | 0 | 4 | 5 | 719 | 659 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 17755 | 1967061 |
| + CARAXES_INTR_PAGE=1 | 17755 | 1967061 |
| + CARAXES_BAR2_MEMFD=1 | 17755 | 1967061 |
| + both | 17755 | 1967061 |

Median per access by class, us: bar0-other 164.0, bar2-flush-read 102.0, gsp-falcon 37.0, gsp-queue-doorbell 133.0, mmu-invalidate 39.0, pcfg-mirror (intr re-arm) 45.0, pci-config 16.0, usermode page, other 37.0, usermode-doorbell 1488.0


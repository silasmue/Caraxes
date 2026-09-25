# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/after-2/census-trace.txt

Window 178275258 .. 227407582 us (49132.3 ms), 17755 trapped accesses, 5192.7 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| mmu-invalidate | 5590 | 1842553 | 35.5 % | 182.0 | 1038.0 |
| bar2-flush-read | 3125 | 1329288 | 25.6 % | 430.0 | 1543.0 |
| gsp-falcon | 3261 | 931646 | 17.9 % | 124.0 | 915.0 |
| gsp-queue-doorbell | 1087 | 455235 | 8.8 % | 480.0 | 1457.0 |
| pci-config | 3591 | 409437 | 7.9 % | 34.0 | 255.0 |
| bar0-other | 616 | 137617 | 2.7 % | 707.0 | 1696.0 |
| usermode page, other | 360 | 86234 | 1.7 % | 120.5 | 689.0 |
| pcfg-mirror (intr re-arm) | 5 | 708 | 0.0 % | 137.0 | 283.0 |
| usermode-doorbell | 120 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 17755 | 5192718 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| GSP_RM_ALLOC class 0xc46f | 120 | 6 | 24 | 102966 | 0 |
| FREE | 420 | 4 | 16 | 5654 | 144 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 4 | 7 | 604 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 4 | 12 | 70 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 4 | 5 | 370 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 4 | 479 | 479 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 4 | 10 | 372 | 10 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 4 | 258 | 258 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 4 | 32 | 149 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 4 | 15 | 152 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 5 | 148 | 148 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 10 | 36 | 86 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 4 | 6 | 109 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 4 | 16 | 25 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 4 | 7 | 24 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 4 | 6 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 4 | 74 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 4 | 5 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 4 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 4 | 12 | 41 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 15 | 21 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 4 | 56 | 56 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 4 | 7 | 15 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 5 | 19 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 4 | 6 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 4 | 29 | 29 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 4 | 15 | 15 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 4 | 6 | 7 | 6 |
| GSP_RM_ALLOC class 0x83de | 6 | 4 | 4 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 4 | 6 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 3 | 4 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 638 |
| 0xa9600000 | 0x5 | 457 |
| 0xa9601000 | 0xc5 | 12 |
| 0xa9601000 | 0x5 | 6 |
| 0x69804000 | 0xc1 | 5 |

341 plan(s) applied by the worker: 101596 leaves unmapped, 131368 mapped in 1069 RM map call(s); walk median 54 us (server thread), apply median 704 us p90 14447 us total 3687.7 ms (worker), queued median 131 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1069 | 289.2 | 271 |
| unmap | 752 | 292.5 | 389 |
| object alloc | 889 | 371.1 | 417 |
| object free | 895 | 398.5 | 445 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 132 | 2107.5 | 15966 |
| gather free | 132 | 38.1 | 289 |
| total | 3869 | 3496.9 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 84 | 1398 | 444 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 62 | 4942 | 601 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 31 | 2075 | 544 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 30 | 5306 | 1094 |
| 0 | 1 | 5 | 0 | 0 | 2 | 0 | 24 | 12094 | 3026 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 19 | 4794 | 2651 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 16 | 2786 | 802 |
| 0 | 2 | 5 | 0 | 0 | 2 | 0 | 14 | 11874 | 2525 |
| 0 | 2 | 5 | 0 | 0 | 3 | 0 | 14 | 16844 | 8071 |
| 0 | 1 | 5 | 0 | 0 | 3 | 0 | 10 | 14833 | 12078 |
| 0 | 2 | 5 | 0 | 0 | 4 | 0 | 9 | 18929 | 10490 |
| 0 | 3 | 5 | 0 | 0 | 4 | 0 | 9 | 24457 | 12006 |
| 0 | 3 | 5 | 0 | 0 | 2 | 0 | 9 | 6438 | 2301 |
| 0 | 5 | 10 | 0 | 0 | 3 | 0 | 7 | 22639 | 14679 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 17755 | 5192718 |
| + CARAXES_INTR_PAGE=1 | 17755 | 5192718 |
| + CARAXES_BAR2_MEMFD=1 | 17755 | 5192718 |
| + both | 17755 | 5192718 |

Median per access by class, us: bar0-other 707.0, bar2-flush-read 430.0, gsp-falcon 124.0, gsp-queue-doorbell 480.0, mmu-invalidate 182.0, pcfg-mirror (intr re-arm) 137.0, pci-config 34.0, usermode page, other 120.5


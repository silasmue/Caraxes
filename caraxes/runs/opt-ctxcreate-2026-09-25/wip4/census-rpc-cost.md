# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/wip4/census-trace.txt

Window 144770039 .. 161862894 us (17092.9 ms), 18268 trapped accesses, 1772.3 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 825105 | 46.6 % | 88.0 | 843.0 |
| mmu-invalidate | 5590 | 362769 | 20.5 % | 33.0 | 85.0 |
| gsp-queue-doorbell | 1087 | 239712 | 13.5 % | 116.0 | 518.0 |
| gsp-falcon | 3261 | 136519 | 7.7 % | 33.0 | 62.0 |
| bar0-other | 616 | 92749 | 5.2 % | 133.0 | 794.0 |
| pci-config | 4104 | 71226 | 4.0 % | 13.0 | 28.0 |
| usermode-doorbell | 120 | 28379 | 1.6 % | 1795.0 | 1983.0 |
| usermode page, other | 360 | 15664 | 0.9 % | 31.0 | 60.0 |
| pcfg-mirror (intr re-arm) | 5 | 150 | 0.0 % | 30.0 | 35.0 |
| all | 18268 | 1772273 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 4 | 23 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 4 | 9 | 41 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 4 | 13 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 7 | 10 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 3 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 6 | 16 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 5 | 18 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 6 | 9 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 5 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 9 | 34 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 8 | 9 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 3 | 14 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 7 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 9 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 4 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 7 | 19 | 19 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 3 | 4 | 9 | 10 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 3 | 4 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 3 | 3 | 9 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 3 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 3 | 3 | 8 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 3 | 3 | 4 | 6 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 2 | 3 | 3 | 0 |
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

337 plan(s) applied by the worker: 101084 leaves unmapped, 131367 mapped in 1119 RM map call(s); walk median 21 us (server thread), apply median 271 us p90 3402 us total 721.5 ms (worker), queued median 11 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1119 | 71.8 | 64 |
| unmap | 725 | 68.2 | 94 |
| object alloc | 889 | 62.2 | 70 |
| object free | 895 | 70.9 | 79 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 132 | 365.5 | 2769 |
| gather free | 132 | 14.0 | 106 |
| total | 3892 | 652.6 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 183 | 274 | 229 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 89 | 525 | 314 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 50 | 1214 | 363 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 26 | 4588 | 3085 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 19 | 1335 | 332 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 15 | 3189 | 783 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 12 | 1520 | 1209 |
| 0 | 2 | 5 | 0 | 0 | 9 | 4 | 11 | 61117 | 60816 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 10 | 1424 | 543 |
| 0 | 2 | 5 | 0 | 0 | 5 | 2 | 10 | 31292 | 30903 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 9 | 1428 | 951 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 8 | 502 | 247 |
| 0 | 5 | 10 | 0 | 0 | 5 | 2 | 8 | 34494 | 32166 |
| 0 | 3 | 5 | 0 | 0 | 5 | 2 | 7 | 31304 | 31252 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 18268 | 1772273 |
| + CARAXES_INTR_PAGE=1 | 18268 | 1772273 |
| + CARAXES_BAR2_MEMFD=1 | 18268 | 1772273 |
| + both | 18268 | 1772273 |

Median per access by class, us: bar0-other 133.0, bar2-flush-read 88.0, gsp-falcon 33.0, gsp-queue-doorbell 116.0, mmu-invalidate 33.0, pcfg-mirror (intr re-arm) 30.0, pci-config 13.0, usermode page, other 31.0, usermode-doorbell 1795.0


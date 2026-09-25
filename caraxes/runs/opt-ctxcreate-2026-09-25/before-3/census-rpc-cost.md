# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/before-3/census-trace.txt

Window 152213501 .. 172326120 us (20112.6 ms), 17755 trapped accesses, 2496.8 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 974793 | 39.0 % | 121.0 | 847.0 |
| mmu-invalidate | 5590 | 611361 | 24.5 % | 49.0 | 183.0 |
| gsp-queue-doorbell | 1087 | 356495 | 14.3 % | 147.0 | 1233.0 |
| gsp-falcon | 3261 | 228090 | 9.1 % | 42.0 | 93.0 |
| pci-config | 3591 | 151531 | 6.1 % | 25.0 | 53.0 |
| bar0-other | 616 | 129715 | 5.2 % | 353.0 | 1174.0 |
| usermode page, other | 360 | 25110 | 1.0 % | 49.0 | 88.0 |
| usermode-doorbell | 120 | 19499 | 0.8 % | 1873.0 | 1942.0 |
| pcfg-mirror (intr re-arm) | 5 | 186 | 0.0 % | 36.0 | 45.0 |
| all | 17755 | 2496780 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 5 | 31 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 5 | 10 | 19 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 8 | 13 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 5 | 13 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 4 | 9 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 8 | 17 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 7 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 4 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 3 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 3 | 4 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 4 | 15 | 17 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 4 | 13 | 13 | 10 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 11 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 4 | 10 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 5 | 12 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 4 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 19 | 19 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 4 | 9 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 4 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 4 | 10 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 4 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 15 | 15 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 3 | 9 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 4 | 15 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 4 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 3 | 9 | 9 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 3 | 4 | 4 | 6 |
| GSP_RM_ALLOC class 0x2081 | 5 | 3 | 6 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 3 | 3 | 3 | 0 |
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

298 plan(s) applied by the worker: 101596 leaves unmapped, 131368 mapped in 1091 RM map call(s); walk median 24 us (server thread), apply median 780 us p90 7085 us total 1168.8 ms (worker), queued median 15 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1091 | 155.9 | 143 |
| unmap | 762 | 134.0 | 176 |
| object alloc | 890 | 195.6 | 220 |
| object free | 896 | 136.4 | 152 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 130 | 469.2 | 3610 |
| gather free | 130 | 22.1 | 170 |
| total | 3899 | 1113.3 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 140 | 416 | 244 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 75 | 835 | 361 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 48 | 1352 | 316 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 41 | 1559 | 294 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 31 | 3364 | 533 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 24 | 6444 | 2782 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 14 | 3900 | 1295 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 11 | 3371 | 1000 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 9 | 1291 | 1026 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 8 | 1284 | 970 |
| 0 | 2 | 5 | 0 | 0 | 11 | 4 | 7 | 69815 | 64783 |
| 0 | 6 | 5 | 0 | 0 | 0 | 0 | 6 | 1176 | 772 |
| 0 | 5 | 10 | 0 | 0 | 5 | 2 | 6 | 33798 | 32734 |
| 0 | 3 | 5 | 0 | 0 | 6 | 2 | 6 | 41760 | 34630 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 17755 | 2496780 |
| + CARAXES_INTR_PAGE=1 | 17755 | 2496780 |
| + CARAXES_BAR2_MEMFD=1 | 17755 | 2496780 |
| + both | 17755 | 2496780 |

Median per access by class, us: bar0-other 353.0, bar2-flush-read 121.0, gsp-falcon 42.0, gsp-queue-doorbell 147.0, mmu-invalidate 49.0, pcfg-mirror (intr re-arm) 36.0, pci-config 25.0, usermode page, other 49.0, usermode-doorbell 1873.0


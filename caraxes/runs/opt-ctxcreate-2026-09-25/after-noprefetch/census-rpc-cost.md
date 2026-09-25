# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/after-noprefetch/census-trace.txt

Window 176811170 .. 198219183 us (21408.0 ms), 17242 trapped accesses, 3316.2 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 1169897 | 35.3 % | 185.0 | 1065.0 |
| mmu-invalidate | 5590 | 840717 | 25.4 % | 71.0 | 320.0 |
| gsp-falcon | 3261 | 469123 | 14.1 % | 66.0 | 318.0 |
| gsp-queue-doorbell | 1087 | 466377 | 14.1 % | 286.0 | 1420.0 |
| pci-config | 3078 | 161185 | 4.9 % | 23.0 | 71.0 |
| bar0-other | 616 | 136124 | 4.1 % | 288.0 | 1165.0 |
| usermode page, other | 360 | 48147 | 1.5 % | 65.0 | 296.0 |
| usermode-doorbell | 120 | 24265 | 0.7 % | 1734.5 | 1929.0 |
| pcfg-mirror (intr re-arm) | 5 | 325 | 0.0 % | 55.0 | 104.0 |
| all | 17242 | 3316160 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 4 | 6 | 648 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 5 | 18 | 940 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 5 | 19 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 6 | 22 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 119 | 119 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 4 | 16 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 9 | 145 | 145 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 4 | 5 | 132 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 17 | 34 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 4 | 44 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 13 | 24 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 5 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 4 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 4 | 16 | 17 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 47 | 47 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 4 | 6 | 22 | 10 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 5 | 30 | 30 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 5 | 14 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 3 | 16 | 24 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 5 | 17 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 4 | 15 | 23 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 4 | 18 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 6 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 4 | 5 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 27 | 27 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 4 | 7 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 4 | 7 | 14 | 6 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 4 | 5 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 4 | 5 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 3 | 11 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 4 | 4 | 4 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 638 |
| 0xa9600000 | 0x5 | 457 |
| 0xa9601000 | 0xc5 | 12 |
| 0xa9601000 | 0x5 | 6 |
| 0x69804000 | 0xc1 | 5 |

298 plan(s) applied by the worker: 101596 leaves unmapped, 131368 mapped in 1086 RM map call(s); walk median 37 us (server thread), apply median 734 us p90 6036 us total 1351.0 ms (worker), queued median 16 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1086 | 125.4 | 116 |
| unmap | 769 | 204.8 | 266 |
| object alloc | 890 | 192.8 | 217 |
| object free | 896 | 86.4 | 96 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 139 | 620.2 | 4462 |
| gather free | 139 | 23.4 | 168 |
| total | 3919 | 1253.1 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 149 | 592 | 222 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 66 | 1221 | 426 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 49 | 1827 | 432 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 34 | 1633 | 509 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 24 | 2819 | 459 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 16 | 10534 | 3008 |
| 0 | 2 | 5 | 0 | 0 | 2 | 0 | 11 | 5256 | 1516 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 11 | 5659 | 1505 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 11 | 2409 | 1385 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 9 | 2796 | 1258 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 6 | 2505 | 1724 |
| 0 | 2 | 5 | 0 | 0 | 11 | 4 | 6 | 63664 | 61853 |
| 0 | 2 | 5 | 0 | 0 | 10 | 2 | 6 | 34914 | 32349 |
| 0 | 6 | 10 | 0 | 0 | 0 | 0 | 5 | 1858 | 1316 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 17242 | 3316160 |
| + CARAXES_INTR_PAGE=1 | 17242 | 3316160 |
| + CARAXES_BAR2_MEMFD=1 | 17242 | 3316160 |
| + both | 17242 | 3316160 |

Median per access by class, us: bar0-other 288.0, bar2-flush-read 185.0, gsp-falcon 66.0, gsp-queue-doorbell 286.0, mmu-invalidate 71.0, pcfg-mirror (intr re-arm) 55.0, pci-config 23.0, usermode page, other 65.0, usermode-doorbell 1734.5


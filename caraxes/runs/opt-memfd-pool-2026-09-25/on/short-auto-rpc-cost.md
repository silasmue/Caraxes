# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a502fb60566dbcfc2/caraxes/runs/opt-memfd-pool-2026-09-25/on/short-auto-trace.txt

Window 228994330 .. 235795602 us (6801.3 ms), 4009 trapped accesses, 488.4 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 477 | 172782 | 35.4 % | 160.0 | 1044.0 |
| mmu-invalidate | 830 | 86737 | 17.8 % | 57.0 | 172.0 |
| pci-config | 1539 | 66761 | 13.7 % | 20.0 | 67.0 |
| gsp-falcon | 561 | 58632 | 12.0 % | 46.0 | 125.0 |
| gsp-queue-doorbell | 187 | 55383 | 11.3 % | 151.5 | 882.0 |
| pcfg-mirror (intr re-arm) | 327 | 42708 | 8.7 % | 126.0 | 1116.0 |
| usermode page, other | 60 | 4215 | 0.9 % | 37.0 | 68.0 |
| bar0-other | 8 | 1166 | 0.2 % | 73.0 | 305.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 4009 | 488384 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 7 | 1532 | 0 |
| FREE | 71 | 3 | 5 | 70 | 24 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 9 | 11 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 11 | 26 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 24 | 45 | 45 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 4 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 4 | 6 | 6 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 2 | 9 | 9 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 11 | 11 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 2 | 5 | 5 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 4 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 2 | 4 | 5 | 5 | 2 |
| GSP_RM_ALLOC class 0x80 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 89 |
| 0xa9600000 | 0x5 | 73 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

38 plan(s) applied by the worker: 18050 leaves unmapped, 20131 mapped in 17397 RM map call(s); walk median 22 us (server thread), apply median 2264 us p90 53555 us total 4321.8 ms (worker), queued median 214 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 16 | 421 | 317 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 7 | 1456 | 452 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 6 | 2701 | 464 |
| 0 | 2 | 5 | 0 | 0 | 1 | 1 | 4 | 4170 | 1402 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 4 | 2629 | 1205 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 3 | 933 | 560 |
| 0 | 3 | 5 | 0 | 0 | 0 | 1 | 2 | 2668 | 552 |
| 0 | 5 | 10 | 0 | 0 | 1 | 1 | 2 | 8552 | 8026 |
| 0 | 2 | 5 | 0 | 0 | 2 | 6 | 2 | 14702 | 14633 |
| 0 | 3 | 5 | 0 | 0 | 1 | 1 | 2 | 5252 | 3234 |
| 0 | 1 | 5 | 0 | 0 | 0 | 1 | 2 | 833 | 640 |
| 0 | 8 | 15 | 0 | 0 | 1 | 1 | 1 | 6030 | 6030 |
| 0 | 8 | 15 | 0 | 0 | 3 | 18 | 1 | 18797 | 18797 |
| 0 | 113 | 145 | 0 | 0 | 10 | 11 | 1 | 65380 | 65380 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 4009 | 488384 |
| + CARAXES_INTR_PAGE=1 | 4009 | 488384 |
| + CARAXES_BAR2_MEMFD=1 | 4009 | 488384 |
| + both | 4009 | 488384 |

Median per access by class, us: bar0-other 73.0, bar2-flush-read 160.0, gsp-falcon 46.0, gsp-queue-doorbell 151.5, mmu-invalidate 57.0, pcfg-mirror (intr re-arm) 126.0, pci-config 20.0, usermode page, other 37.0


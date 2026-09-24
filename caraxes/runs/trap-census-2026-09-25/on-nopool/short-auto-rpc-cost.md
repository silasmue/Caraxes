# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/on-nopool/short-auto-trace.txt

Window 238139822 .. 240155415 us (2015.6 ms), 3846 trapped accesses, 353.3 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 477 | 128658 | 36.4 % | 90.0 | 913.0 |
| bar1-write | 978 | 66100 | 18.7 % | 33.0 | 57.0 |
| gsp-queue-doorbell | 187 | 54753 | 15.5 % | 119.0 | 1255.0 |
| mmu-invalidate | 830 | 43044 | 12.2 % | 33.0 | 65.0 |
| pcfg-mirror (intr re-arm) | 212 | 24788 | 7.0 % | 51.0 | 622.0 |
| gsp-falcon | 561 | 23336 | 6.6 % | 32.0 | 60.0 |
| pci-config | 513 | 8381 | 2.4 % | 14.0 | 31.0 |
| usermode page, other | 60 | 2506 | 0.7 % | 29.0 | 61.0 |
| bar0-other | 8 | 1728 | 0.5 % | 126.5 | 381.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 3846 | 353294 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 3 | 10 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 5 | 5 | 10 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 4 | 34 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 3 | 9 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 2 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 6 | 6 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 7 | 7 | 7 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 2 | 3 | 3 | 3 | 2 |
| GSP_RM_ALLOC class 0x80 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 89 |
| 0xa9600000 | 0x5 | 73 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

36 plan(s) applied by the worker: 3202 leaves unmapped, 19620 mapped in 17948 RM map call(s); walk median 10 us (server thread), apply median 1428 us p90 13275 us total 641.6 ms (worker), queued median 10 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 23 | 298 | 257 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 11 | 399 | 337 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 9 | 1532 | 655 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 5 | 2437 | 817 |
| 0 | 2 | 5 | 0 | 0 | 1 | 1 | 3 | 991 | 638 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 3 | 422 | 355 |
| 0 | 8 | 15 | 0 | 0 | 0 | 0 | 1 | 1115 | 1115 |
| 0 | 8 | 15 | 0 | 0 | 0 | 16 | 1 | 2064 | 2064 |
| 0 | 113 | 145 | 0 | 0 | 10 | 10 | 1 | 65134 | 65134 |
| 0 | 10 | 20 | 0 | 0 | 1 | 5 | 1 | 10976 | 10976 |
| 0 | 6 | 10 | 0 | 0 | 1 | 1 | 1 | 2773 | 2773 |
| 0 | 5 | 10 | 0 | 0 | 1 | 24 | 1 | 4384 | 4384 |
| 0 | 43 | 105 | 0 | 0 | 4 | 309 | 1 | 30315 | 30315 |
| 0 | 2 | 5 | 0 | 0 | 107 | 110 | 1 | 745066 | 745066 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 3846 | 353294 |
| + CARAXES_INTR_PAGE=1 | 3846 | 353294 |
| + CARAXES_BAR2_MEMFD=1 | 3846 | 353294 |
| + both | 3846 | 353294 |

Median per access by class, us: bar0-other 126.5, bar1-write 33.0, bar2-flush-read 90.0, gsp-falcon 32.0, gsp-queue-doorbell 119.0, mmu-invalidate 33.0, pcfg-mirror (intr re-arm) 51.0, pci-config 14.0, usermode page, other 29.0


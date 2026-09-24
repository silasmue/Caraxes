# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/bare/ppo-trace.txt

Window 243579351 .. 285308067 us (41728.7 ms), 790692 trapped accesses, 35485.5 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar1-write | 431745 | 21863403 | 61.6 % | 34.0 | 80.0 |
| usermode-doorbell | 143915 | 5445163 | 15.3 % | 31.0 | 46.0 |
| vf-intr-tree | 90398 | 3949673 | 11.1 % | 30.0 | 43.0 |
| bar2-write | 100713 | 3450439 | 9.7 % | 29.0 | 41.0 |
| pmc-intr-en (RM lock) | 6068 | 258455 | 0.7 % | 29.0 | 51.0 |
| pcfg-mirror (intr re-arm) | 6189 | 242150 | 0.7 % | 31.0 | 41.0 |
| pci-config | 9234 | 166394 | 0.5 % | 15.0 | 27.0 |
| mmu-invalidate | 1005 | 36126 | 0.1 % | 31.0 | 51.0 |
| gsp-falcon | 582 | 24377 | 0.1 % | 34.0 | 51.0 |
| gsp-queue-doorbell | 194 | 23438 | 0.1 % | 63.0 | 205.0 |
| bar2-flush-read | 540 | 20496 | 0.1 % | 34.0 | 53.0 |
| usermode page, other | 60 | 2070 | 0.0 % | 31.0 | 48.0 |
| bar0-other | 8 | 1987 | 0.0 % | 157.5 | 506.0 |
| pmc page 0, other | 41 | 1341 | 0.0 % | 30.0 | 47.0 |
| all | 790692 | 35485512 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 4 | 12 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 6 | 10 | 18 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 4 | 18 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 8 | 52 | 52 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 3 | 19 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 4 | 10 | 10 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 6 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 6 | 10 | 10 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 6 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 8 | 8 | 8 | 1 |
| GSP_RM_ALLOC class 0x0 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 7 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 122 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

32 plan(s) applied by the worker: 29 leaves unmapped, 20459 mapped in 1478 RM map call(s); walk median 24 us (server thread), apply median 408 us p90 6901 us total 68.9 ms (worker), queued median 13 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 10 | 8 | 0 | 0 | 20 | 723 | 630 |
| 2 | 2 | 5 | 10 | 8 | 0 | 0 | 14 | 816 | 775 |
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 13 | 197 | 176 |
| 514 | 3 | 5 | 10 | 50 | 3 | 3 | 4 | 18583 | 17825 |
| 4 | 2 | 5 | 30 | 38 | 1 | 13 | 2 | 6312 | 3192 |
| 20 | 2 | 5 | 10 | 8 | 0 | 0 | 2 | 1380 | 1320 |
| 514 | 3 | 5 | 10 | 36 | 2 | 2 | 2 | 17582 | 17303 |
| 514 | 3 | 5 | 10 | 78 | 5 | 5 | 2 | 30586 | 24501 |
| 0 | 1 | 5 | 10 | 22 | 1 | 1 | 2 | 1282 | 1271 |
| 514 | 3 | 5 | 10 | 64 | 4 | 4 | 2 | 26286 | 23355 |
| 1036 | 8 | 15 | 30 | 154 | 10 | 523 | 1 | 58370 | 58370 |
| 4 | 2 | 5 | 20 | 16 | 0 | 0 | 1 | 1467 | 1467 |
| 4 | 2 | 5 | 30 | 38 | 1 | 1 | 1 | 4920 | 4920 |
| 132 | 8 | 15 | 80 | 92 | 2 | 18 | 1 | 12115 | 12115 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 790692 | 35485512 |
| + CARAXES_INTR_PAGE=1 | 694185 | 31276043 |
| + CARAXES_BAR2_MEMFD=1 | 689979 | 32035073 |
| + both | 593472 | 27825604 |

Median per access by class, us: bar0-other 157.5, bar1-write 34.0, bar2-flush-read 34.0, bar2-write 29.0, gsp-falcon 34.0, gsp-queue-doorbell 63.0, mmu-invalidate 31.0, pcfg-mirror (intr re-arm) 31.0, pci-config 15.0, pmc page 0, other 30.0, pmc-intr-en (RM lock) 29.0, usermode page, other 31.0, usermode-doorbell 31.0, vf-intr-tree 30.0


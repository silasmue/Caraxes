# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a502fb60566dbcfc2/caraxes/runs/opt-memfd-pool-2026-09-25/off/rl-trace.txt

Window 146561430 .. 167567744 us (21006.3 ms), 109466 trapped accesses, 4295.5 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 100717 | 3807468 | 88.6 % | 28.0 | 41.0 |
| bar2-flush-read | 531 | 148919 | 3.5 % | 58.0 | 783.0 |
| pci-config | 4617 | 115206 | 2.7 % | 16.0 | 34.0 |
| pcfg-mirror (intr re-arm) | 1763 | 70901 | 1.7 % | 43.0 | 684.0 |
| mmu-invalidate | 970 | 61837 | 1.4 % | 33.0 | 80.0 |
| gsp-queue-doorbell | 195 | 51687 | 1.2 % | 117.5 | 1053.0 |
| gsp-falcon | 585 | 32730 | 0.8 % | 35.5 | 78.0 |
| usermode page, other | 60 | 3741 | 0.1 % | 38.5 | 87.0 |
| usermode-doorbell | 20 | 1997 | 0.0 % | 1997.0 | 1997.0 |
| bar0-other | 8 | 1014 | 0.0 % | 86.0 | 224.0 |
| all | 109466 | 4295500 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 4 | 84 | 24 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 8 | 61 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 13 | 82 | 0 |
| GSP_RM_ALLOC class 0xc46f | 20 | 5 | 5 | 14 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 5 | 10 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 14 | 24 | 24 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 11 | 14 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 10 | 15 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 2 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 6 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 9 | 9 | 9 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 113 |
| 0xa9600000 | 0x5 | 77 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

66 plan(s) applied by the worker: 1052 leaves unmapped, 21470 mapped in 423 RM map call(s); walk median 838 us (server thread), apply median 2205 us p90 3254 us total 131.8 ms (worker), queued median 13 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 35 | 195 | 167 |
| 2 | 2 | 5 | 0 | 0 | 0 | 0 | 14 | 340 | 297 |
| 514 | 3 | 5 | 0 | 0 | 2 | 2 | 6 | 14962 | 14280 |
| 4 | 2 | 5 | 0 | 0 | 0 | 0 | 4 | 2197 | 1748 |
| 1542 | 5 | 10 | 0 | 0 | 9 | 9 | 2 | 61557 | 61381 |
| 4 | 2 | 5 | 0 | 0 | 1 | 1 | 2 | 4502 | 4299 |
| 514 | 3 | 5 | 0 | 0 | 3 | 3 | 2 | 18062 | 15985 |
| 1036 | 8 | 15 | 0 | 0 | 0 | 0 | 1 | 94526 | 94526 |
| 208 | 14 | 25 | 0 | 0 | 0 | 88 | 1 | 886493 | 886493 |
| 43176 | 113 | 145 | 0 | 0 | 0 | 441 | 1 | 2345873 | 2345873 |
| 1070 | 10 | 20 | 0 | 0 | 0 | 418 | 1 | 125472 | 125472 |
| 1009 | 6 | 10 | 0 | 0 | 0 | 99 | 1 | 58632 | 58632 |
| 12 | 3 | 5 | 0 | 0 | 0 | 0 | 1 | 3149 | 3149 |
| 144 | 3 | 5 | 0 | 0 | 0 | 0 | 1 | 6999 | 6999 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 109466 | 4295500 |
| + CARAXES_INTR_PAGE=1 | 109466 | 4295500 |
| + CARAXES_BAR2_MEMFD=1 | 8749 | 488032 |
| + both | 8749 | 488032 |

Median per access by class, us: bar0-other 86.0, bar2-flush-read 58.0, bar2-write 28.0, gsp-falcon 35.5, gsp-queue-doorbell 117.5, mmu-invalidate 33.0, pcfg-mirror (intr re-arm) 43.0, pci-config 16.0, usermode page, other 38.5, usermode-doorbell 1997.0


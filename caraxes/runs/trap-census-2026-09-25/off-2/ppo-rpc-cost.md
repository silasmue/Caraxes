# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/off-2/ppo-trace.txt

Window 219363903 .. 258429708 us (39065.8 ms), 108883 trapped accesses, 3718.3 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 100713 | 3455291 | 92.9 % | 28.0 | 38.0 |
| pcfg-mirror (intr re-arm) | 5761 | 112026 | 3.0 % | 43.0 | 326.0 |
| gsp-queue-doorbell | 194 | 47440 | 1.3 % | 100.0 | 918.0 |
| mmu-invalidate | 1005 | 39684 | 1.1 % | 31.0 | 58.0 |
| gsp-falcon | 582 | 30381 | 0.8 % | 31.0 | 64.0 |
| bar2-flush-read | 540 | 25953 | 0.7 % | 35.0 | 70.0 |
| bar0-other | 8 | 3063 | 0.1 % | 141.0 | 419.0 |
| usermode page, other | 60 | 2447 | 0.1 % | 28.0 | 60.0 |
| usermode-doorbell | 20 | 1986 | 0.1 % | 1986.0 | 1986.0 |
| all | 108883 | 3718271 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 4 | 14 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 5 | 8 | 15 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 3 | 9 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 3 | 4 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 4 | 5 | 5 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 6 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 6 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 10 | 10 | 10 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 4 | 5 | 5 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 4 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 4 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 9 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 9 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 7 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 7 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 6 | 6 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 6 | 6 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 122 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

53 plan(s) applied by the worker: 1052 leaves unmapped, 21482 mapped in 1897 RM map call(s); walk median 66 us (server thread), apply median 1977 us p90 4685 us total 172.4 ms (worker), queued median 10 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 37 | 186 | 160 |
| 2 | 2 | 5 | 0 | 0 | 0 | 0 | 16 | 257 | 244 |
| 4 | 2 | 5 | 0 | 0 | 0 | 0 | 5 | 798 | 521 |
| 514 | 3 | 5 | 0 | 0 | 2 | 2 | 4 | 15738 | 13528 |
| 514 | 3 | 5 | 0 | 0 | 3 | 3 | 4 | 16740 | 15172 |
| 1542 | 5 | 10 | 0 | 0 | 7 | 7 | 3 | 49980 | 46732 |
| 1542 | 5 | 10 | 0 | 0 | 8 | 8 | 2 | 52444 | 52373 |
| 4 | 2 | 5 | 0 | 0 | 1 | 1 | 2 | 6559 | 4193 |
| 40 | 2 | 5 | 0 | 0 | 0 | 0 | 2 | 1580 | 1543 |
| 20 | 2 | 5 | 0 | 0 | 0 | 0 | 2 | 715 | 701 |
| 8 | 2 | 5 | 0 | 0 | 0 | 0 | 2 | 416 | 415 |
| 1036 | 8 | 15 | 0 | 0 | 9 | 9 | 1 | 65168 | 65168 |
| 132 | 8 | 15 | 0 | 0 | 7 | 21 | 1 | 46098 | 46098 |
| 43176 | 113 | 145 | 0 | 0 | 249 | 249 | 1 | 1667082 | 1667082 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 108883 | 3718271 |
| + CARAXES_INTR_PAGE=1 | 108883 | 3718271 |
| + CARAXES_BAR2_MEMFD=1 | 8170 | 262980 |
| + both | 8170 | 262980 |

Median per access by class, us: bar0-other 141.0, bar2-flush-read 35.0, bar2-write 28.0, gsp-falcon 31.0, gsp-queue-doorbell 100.0, mmu-invalidate 31.0, pcfg-mirror (intr re-arm) 43.0, usermode page, other 28.0, usermode-doorbell 1986.0


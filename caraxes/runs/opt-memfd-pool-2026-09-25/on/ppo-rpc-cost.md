# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a502fb60566dbcfc2/caraxes/runs/opt-memfd-pool-2026-09-25/on/ppo-trace.txt

Window 189653178 .. 225180350 us (35527.2 ms), 15033 trapped accesses, 821.6 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| pcfg-mirror (intr re-arm) | 5442 | 235994 | 28.7 % | 463.0 | 1611.0 |
| pci-config | 7182 | 225629 | 27.5 % | 20.0 | 45.0 |
| bar2-flush-read | 540 | 152986 | 18.6 % | 99.0 | 829.0 |
| mmu-invalidate | 1005 | 80568 | 9.8 % | 42.0 | 100.0 |
| gsp-queue-doorbell | 194 | 66861 | 8.1 % | 181.0 | 1238.0 |
| gsp-falcon | 582 | 51517 | 6.3 % | 46.0 | 173.0 |
| usermode page, other | 60 | 4066 | 0.5 % | 38.5 | 77.0 |
| bar0-other | 8 | 2106 | 0.3 % | 130.0 | 347.0 |
| usermode-doorbell | 20 | 1854 | 0.2 % | 1854.0 | 1854.0 |
| all | 15033 | 821581 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 7 | 45 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 5 | 11 | 97 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 4 | 9 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 4 | 4 | 44 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 3 | 10 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 4 | 14 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 4 | 11 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 10 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 2 | 9 | 9 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 8 | 12 | 12 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 7 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 6 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 9 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 9 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 122 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

53 plan(s) applied by the worker: 1052 leaves unmapped, 21482 mapped in 1295 RM map call(s); walk median 56 us (server thread), apply median 1668 us p90 4010 us total 111.8 ms (worker), queued median 12 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 34 | 416 | 226 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 33 | 455 | 375 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 9 | 1150 | 424 |
| 0 | 5 | 10 | 0 | 0 | 1 | 1 | 5 | 6730 | 4262 |
| 0 | 1 | 5 | 0 | 0 | 1 | 1 | 3 | 1891 | 353 |
| 0 | 2 | 5 | 0 | 0 | 2 | 2 | 2 | 12846 | 10542 |
| 0 | 5 | 5 | 0 | 0 | 1 | 1 | 2 | 2796 | 1883 |
| 0 | 3 | 5 | 0 | 0 | 1 | 1 | 2 | 4065 | 3085 |
| 0 | 8 | 15 | 0 | 0 | 1 | 1 | 1 | 1912 | 1912 |
| 0 | 8 | 15 | 0 | 0 | 5 | 20 | 1 | 33975 | 33975 |
| 0 | 113 | 145 | 0 | 0 | 13 | 13 | 1 | 85220 | 85220 |
| 0 | 10 | 20 | 0 | 0 | 0 | 85 | 1 | 3283 | 3283 |
| 0 | 6 | 10 | 0 | 0 | 1 | 199 | 1 | 4232 | 4232 |
| 0 | 3 | 5 | 0 | 0 | 0 | 54 | 1 | 1541 | 1541 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 15033 | 821581 |
| + CARAXES_INTR_PAGE=1 | 15033 | 821581 |
| + CARAXES_BAR2_MEMFD=1 | 15033 | 821581 |
| + both | 15033 | 821581 |

Median per access by class, us: bar0-other 130.0, bar2-flush-read 99.0, gsp-falcon 46.0, gsp-queue-doorbell 181.0, mmu-invalidate 42.0, pcfg-mirror (intr re-arm) 463.0, pci-config 20.0, usermode page, other 38.5, usermode-doorbell 1854.0


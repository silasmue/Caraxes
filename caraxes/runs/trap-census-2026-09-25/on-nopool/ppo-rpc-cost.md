# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/on-nopool/ppo-trace.txt

Window 199873096 .. 236248998 us (36375.9 ms), 506132 trapped accesses, 28492.3 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar1-write | 490878 | 27359018 | 96.0 % | 35.0 | 87.0 |
| pcfg-mirror (intr re-arm) | 5150 | 685359 | 2.4 % | 50.0 | 372.0 |
| pci-config | 7695 | 160982 | 0.6 % | 15.0 | 30.0 |
| bar2-flush-read | 540 | 146963 | 0.5 % | 95.0 | 906.0 |
| mmu-invalidate | 1005 | 56641 | 0.2 % | 37.0 | 80.0 |
| gsp-queue-doorbell | 194 | 40896 | 0.1 % | 125.0 | 447.0 |
| gsp-falcon | 582 | 29274 | 0.1 % | 34.0 | 63.0 |
| usermode-doorbell | 20 | 8408 | 0.0 % | 1726.0 | 1842.0 |
| usermode page, other | 60 | 2862 | 0.0 % | 34.0 | 81.0 |
| bar0-other | 8 | 1853 | 0.0 % | 139.0 | 471.0 |
| all | 506132 | 28492256 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 12 | 101 | 0 |
| FREE | 71 | 3 | 4 | 14 | 24 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 5 | 10 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 3 | 10 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 4 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 4 | 10 | 10 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 4 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 8 | 12 | 12 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 15 | 15 | 15 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 4 | 5 | 5 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 7 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 10 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 9 | 9 | 9 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 122 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

32 plan(s) applied by the worker: 29 leaves unmapped, 20459 mapped in 1122 RM map call(s); walk median 21 us (server thread), apply median 1051 us p90 11021 us total 106.0 ms (worker), queued median 14 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 37 | 249 | 233 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 28 | 382 | 317 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 12 | 1234 | 416 |
| 0 | 5 | 10 | 0 | 0 | 1 | 1 | 2 | 2977 | 2857 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 2 | 3832 | 3236 |
| 0 | 2 | 5 | 0 | 0 | 0 | 9 | 2 | 1757 | 1164 |
| 0 | 8 | 15 | 0 | 0 | 0 | 0 | 1 | 3128 | 3128 |
| 0 | 8 | 15 | 0 | 0 | 1 | 17 | 1 | 5573 | 5573 |
| 0 | 113 | 145 | 0 | 0 | 12 | 12 | 1 | 71573 | 71573 |
| 0 | 10 | 20 | 0 | 0 | 1 | 5 | 1 | 2626 | 2626 |
| 0 | 6 | 10 | 0 | 0 | 1 | 1 | 1 | 8129 | 8129 |
| 0 | 5 | 10 | 0 | 0 | 2 | 25 | 1 | 10827 | 10827 |
| 0 | 43 | 105 | 0 | 0 | 7 | 312 | 1 | 43948 | 43948 |
| 0 | 2 | 5 | 0 | 0 | 36 | 103 | 1 | 288366 | 288366 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 506132 | 28492256 |
| + CARAXES_INTR_PAGE=1 | 506132 | 28492256 |
| + CARAXES_BAR2_MEMFD=1 | 506132 | 28492256 |
| + both | 506132 | 28492256 |

Median per access by class, us: bar0-other 139.0, bar1-write 35.0, bar2-flush-read 95.0, gsp-falcon 34.0, gsp-queue-doorbell 125.0, mmu-invalidate 37.0, pcfg-mirror (intr re-arm) 50.0, pci-config 15.0, usermode page, other 34.0, usermode-doorbell 1726.0


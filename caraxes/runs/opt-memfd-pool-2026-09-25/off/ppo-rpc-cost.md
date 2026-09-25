# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a502fb60566dbcfc2/caraxes/runs/opt-memfd-pool-2026-09-25/off/ppo-trace.txt

Window 168304707 .. 211111999 us (42807.3 ms), 117974 trapped accesses, 5409.3 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 100713 | 4757223 | 87.9 % | 29.0 | 63.0 |
| pci-config | 8721 | 281259 | 5.2 % | 18.0 | 44.0 |
| pcfg-mirror (intr re-arm) | 6131 | 191165 | 3.5 % | 50.0 | 565.0 |
| mmu-invalidate | 1005 | 61523 | 1.1 % | 35.0 | 76.0 |
| gsp-falcon | 582 | 45427 | 0.8 % | 36.0 | 104.0 |
| gsp-queue-doorbell | 194 | 35404 | 0.7 % | 108.0 | 383.0 |
| bar2-flush-read | 540 | 33971 | 0.6 % | 39.0 | 90.0 |
| usermode page, other | 60 | 2417 | 0.0 % | 29.5 | 69.0 |
| bar0-other | 8 | 873 | 0.0 % | 78.5 | 216.0 |
| usermode-doorbell | 20 | 0 | 0.0 % | 0.0 | 0.0 |
| all | 117974 | 5409262 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 4 | 6 | 72 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 10 | 18 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 3 | 14 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 4 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 10 | 10 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 4 | 12 | 12 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 3 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20809009 | 2 | 6 | 10 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 6 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2080 | 2 | 5 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20809001 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080a084 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 5 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x2080a026 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20809064 | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 122 |
| 0xa9600000 | 0x5 | 75 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

54 plan(s) applied by the worker: 1052 leaves unmapped, 21482 mapped in 17262 RM map call(s); walk median 56 us (server thread), apply median 2518 us p90 11800 us total 738.7 ms (worker), queued median 14 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 36 | 228 | 196 |
| 2 | 2 | 5 | 0 | 0 | 0 | 0 | 15 | 352 | 311 |
| 4 | 2 | 5 | 0 | 0 | 0 | 0 | 4 | 640 | 440 |
| 4 | 2 | 5 | 0 | 0 | 1 | 1 | 4 | 6991 | 1658 |
| 514 | 3 | 5 | 0 | 0 | 4 | 4 | 4 | 24974 | 22210 |
| 1542 | 5 | 10 | 0 | 0 | 9 | 9 | 3 | 59583 | 56657 |
| 514 | 3 | 5 | 0 | 0 | 3 | 3 | 3 | 20007 | 19641 |
| 1542 | 5 | 10 | 0 | 0 | 12 | 12 | 2 | 84234 | 84033 |
| 20 | 2 | 5 | 0 | 0 | 0 | 0 | 2 | 2262 | 1159 |
| 514 | 3 | 5 | 0 | 0 | 6 | 6 | 2 | 38100 | 34819 |
| 1036 | 8 | 15 | 0 | 0 | 4 | 4 | 1 | 31437 | 31437 |
| 132 | 8 | 15 | 0 | 0 | 19 | 21 | 1 | 123233 | 123233 |
| 43176 | 113 | 145 | 0 | 0 | 289 | 802 | 1 | 1926132 | 1926132 |
| 1070 | 10 | 20 | 0 | 0 | 5 | 9 | 1 | 36253 | 36253 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 117974 | 5409262 |
| + CARAXES_INTR_PAGE=1 | 117974 | 5409262 |
| + CARAXES_BAR2_MEMFD=1 | 17261 | 652039 |
| + both | 17261 | 652039 |

Median per access by class, us: bar0-other 78.5, bar2-flush-read 39.0, bar2-write 29.0, gsp-falcon 36.0, gsp-queue-doorbell 108.0, mmu-invalidate 35.0, pcfg-mirror (intr re-arm) 50.0, pci-config 18.0, usermode page, other 29.5


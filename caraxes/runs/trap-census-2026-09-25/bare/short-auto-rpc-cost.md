# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-aed304f50d6d7015b/caraxes/runs/trap-census-2026-09-25/bare/short-auto-trace.txt

Window 287218677 .. 293037952 us (5819.3 ms), 120975 trapped accesses, 3821.8 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-write | 96199 | 2942372 | 77.0 % | 27.0 | 34.0 |
| vf-intr-tree | 14174 | 490401 | 12.8 % | 27.0 | 36.0 |
| pmc-intr-en (RM lock) | 5436 | 203693 | 5.3 % | 27.0 | 56.0 |
| bar1-write | 978 | 57307 | 1.5 % | 36.0 | 72.0 |
| mmu-invalidate | 830 | 27525 | 0.7 % | 29.0 | 43.0 |
| pcfg-mirror (intr re-arm) | 709 | 20939 | 0.5 % | 30.0 | 34.0 |
| gsp-falcon | 561 | 18906 | 0.5 % | 30.0 | 49.0 |
| bar2-flush-read | 477 | 15922 | 0.4 % | 30.0 | 44.0 |
| pci-config | 1026 | 15197 | 0.4 % | 13.0 | 22.0 |
| gsp-queue-doorbell | 187 | 14629 | 0.4 % | 48.0 | 124.0 |
| usermode-doorbell | 326 | 11376 | 0.3 % | 30.0 | 63.0 |
| usermode page, other | 60 | 1802 | 0.0 % | 26.0 | 41.0 |
| bar0-other | 8 | 1577 | 0.0 % | 129.0 | 418.0 |
| pmc page 0, other | 4 | 199 | 0.0 % | 43.5 | 83.0 |
| all | 120975 | 3821845 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 71 | 3 | 4 | 1524 | 24 |
| GSP_RM_ALLOC class 0xc46f | 20 | 4 | 8 | 9 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 20 | 3 | 3 | 8 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 8 | 3 | 3 | 7 | 0 |
| GSP_RM_ALLOC class 0x79 | 8 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 4 | 4 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 4 | 3 | 8 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0xa06c | 4 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 4 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 2 | 4 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 1 | 8 | 8 | 8 | 0 |
| GSP_RM_ALLOC class 0x0 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x80 | 2 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 2 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 2 | 3 | 3 | 3 | 2 |
| GSP_RM_ALLOC class 0x2080 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 2 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 2 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x2081 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x9067 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 1 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 1 | 3 | 3 | 3 | 1 |
| GSP_RM_CONTROL cmd 0x20800a4c | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 1 | 2 | 2 | 2 | 0 |
| GSP_RM_ALLOC class 0x83de | 1 | 2 | 2 | 2 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 89 |
| 0xa9600000 | 0x5 | 73 |
| 0xa9601000 | 0xc5 | 2 |
| 0xa9601000 | 0x5 | 1 |
| 0x69804000 | 0xc1 | 1 |

36 plan(s) applied by the worker: 3202 leaves unmapped, 19620 mapped in 17610 RM map call(s); walk median 13 us (server thread), apply median 706 us p90 29155 us total 755.6 ms (worker), queued median 11 us.

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 10 | 8 | 0 | 0 | 17 | 728 | 624 |
| 2 | 2 | 5 | 20 | 16 | 0 | 0 | 4 | 1567 | 1338 |
| 4 | 2 | 5 | 30 | 38 | 1 | 1 | 3 | 2659 | 2487 |
| 1542 | 5 | 10 | 30 | 136 | 8 | 8 | 3 | 51580 | 48856 |
| 0 | 1 | 5 | 10 | 22 | 1 | 1 | 2 | 1540 | 1263 |
| 2 | 2 | 5 | 20 | 30 | 1 | 1 | 2 | 1938 | 1928 |
| 2 | 2 | 5 | 30 | 24 | 0 | 4 | 2 | 2124 | 2060 |
| 2 | 2 | 5 | 30 | 38 | 1 | 1 | 2 | 4840 | 2567 |
| 1036 | 8 | 15 | 30 | 80 | 5 | 4 | 1 | 72436 | 72436 |
| 4 | 2 | 5 | 20 | 16 | 0 | 0 | 1 | 1495 | 1495 |
| 132 | 8 | 15 | 80 | 64 | 0 | 16 | 1 | 9533 | 9533 |
| 43176 | 113 | 145 | 50 | 2918 | 208 | 720 | 1 | 1503013 | 1503013 |
| 1070 | 10 | 20 | 60 | 146 | 7 | 12 | 1 | 47304 | 47304 |
| 1009 | 6 | 10 | 0 | 70 | 5 | 5 | 1 | 32622 | 32622 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 120975 | 3821845 |
| + CARAXES_INTR_PAGE=1 | 101361 | 3127552 |
| + CARAXES_BAR2_MEMFD=1 | 24776 | 879473 |
| + both | 5162 | 185180 |

Median per access by class, us: bar0-other 129.0, bar1-write 36.0, bar2-flush-read 30.0, bar2-write 27.0, gsp-falcon 30.0, gsp-queue-doorbell 48.0, mmu-invalidate 29.0, pcfg-mirror (intr re-arm) 30.0, pci-config 13.0, pmc page 0, other 43.5, pmc-intr-en (RM lock) 27.0, usermode page, other 26.0, usermode-doorbell 30.0, vf-intr-tree 27.0


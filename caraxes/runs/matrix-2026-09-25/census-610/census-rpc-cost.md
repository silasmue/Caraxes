# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-ad0bd7a6ec1b6cc05/caraxes/runs/matrix-2026-09-25/census-610/census-trace.txt

Window 139140488 .. 156068072 us (16927.6 ms), 18268 trapped accesses, 1743.6 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 810403 | 46.5 % | 94.0 | 876.0 |
| mmu-invalidate | 5590 | 287181 | 16.5 % | 36.0 | 70.0 |
| gsp-queue-doorbell | 1087 | 262372 | 15.0 % | 114.0 | 637.0 |
| gsp-falcon | 3261 | 148423 | 8.5 % | 35.0 | 68.0 |
| bar0-other | 616 | 97274 | 5.6 % | 159.0 | 801.0 |
| pci-config | 4104 | 96233 | 5.5 % | 16.0 | 36.0 |
| usermode-doorbell | 120 | 25568 | 1.5 % | 1700.0 | 1886.0 |
| usermode page, other | 360 | 15986 | 0.9 % | 33.0 | 71.0 |
| pcfg-mirror (intr re-arm) | 5 | 158 | 0.0 % | 33.0 | 36.0 |
| all | 18268 | 1743598 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 4 | 27 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 4 | 9 | 17 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 4 | 12 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 9 | 33 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 3 | 14 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 9 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 6 | 12 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 6 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 3 | 7 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 3 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 4 | 9 | 18 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 40 | 40 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 10 | 15 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 3 | 15 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 9 | 9 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 10 | 11 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 4 | 11 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 6 | 9 | 11 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 3 | 4 | 9 | 10 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 3 | 20 | 20 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 3 | 4 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 3 | 3 | 10 | 6 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 3 | 9 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 13 | 13 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 3 | 3 | 8 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 7 | 7 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 3 | 3 | 3 | 0 |
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

171 plan(s) applied by the worker: 100796 leaves unmapped, 130568 mapped in 1031 RM map call(s); walk median 40 us (server thread), apply median 1767 us p90 6804 us total 826.4 ms (worker), queued median 13 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1031 | 79.2 | 77 |
| unmap | 714 | 97.0 | 136 |
| object alloc | 855 | 95.5 | 112 |
| object free | 861 | 59.4 | 69 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 136 | 377.2 | 2774 |
| gather free | 136 | 25.1 | 184 |
| total | 3733 | 733.5 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 181 | 299 | 233 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 91 | 554 | 328 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 47 | 1257 | 331 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 23 | 3235 | 2172 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 20 | 5048 | 456 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 19 | 2351 | 736 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 15 | 1354 | 379 |
| 0 | 2 | 5 | 0 | 0 | 9 | 4 | 13 | 61155 | 60928 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 12 | 1148 | 584 |
| 0 | 5 | 5 | 0 | 0 | 0 | 0 | 12 | 1248 | 799 |
| 0 | 2 | 5 | 0 | 0 | 5 | 2 | 12 | 31219 | 30946 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 11 | 405 | 280 |
| 0 | 5 | 10 | 0 | 0 | 5 | 2 | 8 | 32412 | 31783 |
| 0 | 10 | 20 | 0 | 0 | 0 | 4 | 6 | 2012 | 1744 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 18268 | 1743598 |
| + CARAXES_INTR_PAGE=1 | 18268 | 1743598 |
| + CARAXES_BAR2_MEMFD=1 | 18268 | 1743598 |
| + both | 18268 | 1743598 |

Median per access by class, us: bar0-other 159.0, bar2-flush-read 94.0, gsp-falcon 35.0, gsp-queue-doorbell 114.0, mmu-invalidate 36.0, pcfg-mirror (intr re-arm) 33.0, pci-config 16.0, usermode page, other 33.0, usermode-doorbell 1700.0


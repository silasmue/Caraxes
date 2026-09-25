# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/wip2/census-trace.txt

Window 149308173 .. 190831792 us (41523.6 ms), 21859 trapped accesses, 1637.8 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| mmu-invalidate | 5590 | 454839 | 27.8 % | 44.0 | 111.0 |
| bar2-flush-read | 3125 | 331781 | 20.3 % | 112.0 | 355.0 |
| gsp-queue-doorbell | 1087 | 319011 | 19.5 % | 151.0 | 1028.0 |
| gsp-falcon | 3261 | 233765 | 14.3 % | 40.0 | 95.0 |
| pci-config | 7695 | 170274 | 10.4 % | 15.0 | 33.0 |
| bar0-other | 616 | 97386 | 5.9 % | 134.0 | 822.0 |
| usermode page, other | 360 | 22086 | 1.3 % | 42.0 | 93.0 |
| usermode-doorbell | 120 | 6793 | 0.4 % | 1691.0 | 1933.0 |
| pcfg-mirror (intr re-arm) | 5 | 1843 | 0.1 % | 103.0 | 1513.0 |
| all | 21859 | 1637778 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 7 | 161 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 5 | 11 | 18 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 5 | 20 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 3 | 307 | 307 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 4 | 9 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 15 | 48 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 4 | 14 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 4 | 12 | 20 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 5 | 11 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 4 | 13 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 7 | 15 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 4 | 19 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 4 | 20 | 0 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 3 | 18 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 4 | 4 | 6 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 4 | 5 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 3 | 3 | 4 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 4 | 13 | 13 | 0 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 15 | 15 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 3 | 3 | 3 | 10 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 4 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 4 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 3 | 3 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 3 | 3 | 4 | 6 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 2 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 638 |
| 0xa9600000 | 0x5 | 457 |
| 0xa9601000 | 0xc5 | 12 |
| 0xa9601000 | 0x5 | 6 |
| 0x69804000 | 0xc1 | 5 |

298 plan(s) applied by the worker: 101596 leaves unmapped, 131368 mapped in 1059 RM map call(s); walk median 21 us (server thread), apply median 294 us p90 3804 us total 811.0 ms (worker), queued median 12 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1059 | 52.1 | 49 |
| unmap | 729 | 39.0 | 53 |
| object alloc | 889 | 63.4 | 71 |
| object free | 895 | 131.8 | 147 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 150 | 453.1 | 3021 |
| gather free | 150 | 12.9 | 86 |
| total | 3872 | 752.5 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 166 | 337 | 236 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 54 | 3082 | 358 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 49 | 3515 | 537 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 34 | 5970 | 1384 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 21 | 8684 | 6540 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 19 | 2800 | 1175 |
| 0 | 5 | 10 | 0 | 0 | 2 | 0 | 16 | 12501 | 8144 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 16 | 378 | 278 |
| 0 | 2 | 5 | 0 | 0 | 5 | 2 | 11 | 35413 | 33352 |
| 0 | 2 | 5 | 0 | 0 | 1 | 4 | 7 | 4061 | 2986 |
| 0 | 2 | 5 | 0 | 0 | 10 | 4 | 7 | 64525 | 63273 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 6 | 3009 | 2678 |
| 0 | 5 | 5 | 0 | 0 | 1 | 0 | 6 | 7532 | 7004 |
| 0 | 3 | 5 | 0 | 0 | 5 | 2 | 6 | 36534 | 35850 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 21859 | 1637778 |
| + CARAXES_INTR_PAGE=1 | 21859 | 1637778 |
| + CARAXES_BAR2_MEMFD=1 | 21859 | 1637778 |
| + both | 21859 | 1637778 |

Median per access by class, us: bar0-other 134.0, bar2-flush-read 112.0, gsp-falcon 40.0, gsp-queue-doorbell 151.0, mmu-invalidate 44.0, pcfg-mirror (intr re-arm) 103.0, pci-config 15.0, usermode page, other 42.0, usermode-doorbell 1691.0


# rpc-cost: /home/silas/git/Caraxes/.claude/worktrees/agent-a9897c9b92c374cca/caraxes/runs/opt-ctxcreate-2026-09-25/before-2/census-trace.txt

Window 144292902 .. 174462822 us (30169.9 ms), 19294 trapped accesses, 2732.2 ms attributed (gaps over 2.0 ms are idle).

## 1. Trapped accesses

| class (per access class) | accesses | time, us | share | median us | p90 us |
|---|---|---|---|---|---|
| bar2-flush-read | 3125 | 1162083 | 42.5 % | 136.5 | 1346.0 |
| mmu-invalidate | 5590 | 588817 | 21.6 % | 51.0 | 173.0 |
| gsp-queue-doorbell | 1087 | 347054 | 12.7 % | 170.0 | 1133.0 |
| gsp-falcon | 3261 | 342089 | 12.5 % | 47.0 | 169.0 |
| pci-config | 5130 | 135622 | 5.0 % | 16.0 | 36.0 |
| bar0-other | 616 | 107344 | 3.9 % | 208.0 | 855.0 |
| usermode page, other | 360 | 28987 | 1.1 % | 54.0 | 105.0 |
| usermode-doorbell | 120 | 19425 | 0.7 % | 1582.5 | 1788.0 |
| pcfg-mirror (intr re-arm) | 5 | 762 | 0.0 % | 78.0 | 500.0 |
| all | 19294 | 2732183 | 100 % | | |

## 2. GSP RPCs (device time from rx to the tx reply)

| RPC | count | device us median | p90 | max | with a host RM call |
|---|---|---|---|---|---|
| FREE | 420 | 3 | 5 | 292 | 144 |
| GSP_RM_ALLOC class 0xc46f | 120 | 5 | 11 | 62 | 0 |
| GSP_RM_ALLOC class 0xc5b5 | 120 | 3 | 11 | 133 | 0 |
| GSP_RM_CONTROL cmd 0x20801303 | 10 | 3 | 14 | 541 | 0 |
| GSP_RM_CONTROL cmd 0x20803801 | 5 | 3 | 243 | 243 | 0 |
| GSP_RM_ALLOC class 0x79 | 48 | 3 | 4 | 83 | 0 |
| GSP_RM_ALLOC class 0xc5c0 | 48 | 3 | 4 | 20 | 0 |
| GSP_RM_CONTROL cmd 0x20802a12 | 25 | 3 | 4 | 107 | 0 |
| GSP_RM_ALLOC class 0xa06c | 24 | 3 | 4 | 18 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0101 | 24 | 3 | 8 | 10 | 0 |
| GSP_RM_CONTROL cmd 0xa06c010a | 24 | 3 | 5 | 10 | 0 |
| GSP_RM_CONTROL cmd 0x20802a08 | 24 | 3 | 4 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x20800a9a | 11 | 3 | 20 | 21 | 0 |
| GSP_RM_CONTROL cmd 0x90f10106 | 10 | 3 | 4 | 38 | 0 |
| GSP_RM_ALLOC class 0x80 | 11 | 3 | 11 | 24 | 0 |
| GSP_RM_ALLOC class 0x2080 | 11 | 3 | 4 | 24 | 0 |
| GSP_RM_CONTROL cmd 0x20800a70 | 12 | 4 | 4 | 13 | 0 |
| GSP_RM_ALLOC class 0x0 | 11 | 4 | 5 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x20800aa2 | 5 | 4 | 29 | 29 | 0 |
| GSP_RM_ALLOC class 0x90f1 | 10 | 3 | 4 | 18 | 0 |
| GSP_RM_CONTROL cmd 0x20802a0b | 5 | 3 | 20 | 20 | 0 |
| GSP_RM_CONTROL cmd 0x2080012b | 6 | 4 | 8 | 19 | 0 |
| GSP_RM_CONTROL cmd 0x20808162 | 5 | 3 | 23 | 23 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0105 | 10 | 3 | 4 | 5 | 10 |
| GSP_RM_CONTROL cmd 0x802009 | 11 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20808159 | 5 | 4 | 11 | 11 | 0 |
| GSP_RM_CONTROL cmd 0xa06c0103 | 6 | 4 | 5 | 11 | 6 |
| GSP_RM_CONTROL cmd 0x20800a4c | 5 | 3 | 11 | 11 | 0 |
| GSP_RM_ALLOC class 0x2081 | 5 | 4 | 5 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20810108 | 5 | 4 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x2080012f | 5 | 3 | 5 | 5 | 0 |
| GSP_RM_ALLOC class 0x83de | 6 | 3 | 3 | 5 | 0 |
| GSP_RM_CONTROL cmd 0x20801210 | 6 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20801823 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_ALLOC class 0x9067 | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182a | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x20800102 | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x83de0309 | 6 | 3 | 3 | 3 | 0 |
| GSP_RM_CONTROL cmd 0x2080182b | 5 | 3 | 4 | 4 | 0 |
| GSP_RM_CONTROL cmd 0x802004 | 1 | 14 | 14 | 14 | 0 |
| GSP_RM_CONTROL cmd 0x20800ab8 | 1 | 3 | 3 | 3 | 0 |

## 3. MMU invalidates and shadow plans

| pdb | flags | count |
|---|---|---|
| 0x69800000 | 0xc1 | 638 |
| 0xa9600000 | 0x5 | 457 |
| 0xa9601000 | 0xc5 | 12 |
| 0xa9601000 | 0x5 | 6 |
| 0x69804000 | 0xc1 | 5 |

298 plan(s) applied by the worker: 101596 leaves unmapped, 131368 mapped in 1095 RM map call(s); walk median 24 us (server thread), apply median 521 us p90 9271 us total 1589.5 ms (worker), queued median 14 us.

The shadow's host RM calls by kind (plans and VAS teardowns in the window):

| kind | calls | ms | us per call |
|---|---|---|---|
| map | 1095 | 269.5 | 246 |
| unmap | 751 | 182.6 | 243 |
| object alloc | 890 | 258.3 | 290 |
| object free | 896 | 178.9 | 200 |
| invalidate | 0 | 0.0 | 0 |
| gather alloc | 139 | 609.3 | 4383 |
| gather free | 139 | 21.9 | 157 |
| total | 3910 | 1520.4 | - |

Episodes from one channel-VAS invalidate to the next (one guest mapping change each), by what they trapped on, most frequent first:

| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1 | 5 | 0 | 0 | 0 | 0 | 154 | 394 | 227 |
| 0 | 2 | 5 | 0 | 0 | 0 | 0 | 70 | 1210 | 300 |
| 0 | 3 | 5 | 0 | 0 | 0 | 0 | 41 | 2224 | 376 |
| 0 | 1 | 5 | 0 | 0 | 1 | 0 | 32 | 800 | 333 |
| 0 | 2 | 5 | 0 | 0 | 1 | 0 | 29 | 3468 | 563 |
| 0 | 3 | 5 | 0 | 0 | 1 | 0 | 21 | 3988 | 464 |
| 0 | 5 | 10 | 0 | 0 | 1 | 0 | 18 | 7019 | 1077 |
| 0 | 2 | 5 | 0 | 0 | 9 | 4 | 12 | 62756 | 61259 |
| 0 | 2 | 5 | 0 | 0 | 0 | 4 | 10 | 2338 | 770 |
| 0 | 5 | 10 | 0 | 0 | 2 | 0 | 9 | 11883 | 6741 |
| 0 | 2 | 5 | 0 | 0 | 2 | 0 | 8 | 9042 | 4072 |
| 0 | 5 | 10 | 0 | 0 | 0 | 0 | 8 | 4322 | 1033 |
| 0 | 3 | 5 | 0 | 0 | 5 | 2 | 7 | 32790 | 31600 |
| 0 | 6 | 10 | 0 | 0 | 0 | 0 | 6 | 1570 | 1275 |

## 5. Projection with the existing switches (whole window)

| configuration | trapped accesses per op | time per op, us |
|---|---|---|
| as traced | 19294 | 2732183 |
| + CARAXES_INTR_PAGE=1 | 19294 | 2732183 |
| + CARAXES_BAR2_MEMFD=1 | 19294 | 2732183 |
| + both | 19294 | 2732183 |

Median per access by class, us: bar0-other 208.0, bar2-flush-read 136.5, gsp-falcon 47.0, gsp-queue-doorbell 170.0, mmu-invalidate 51.0, pcfg-mirror (intr re-arm) 78.0, pci-config 16.0, usermode page, other 54.0, usermode-doorbell 1582.5


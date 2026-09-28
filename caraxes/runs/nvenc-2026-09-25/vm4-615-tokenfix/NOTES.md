<!-- SPDX-License-Identifier: MIT -->
# vm4-615 token-fix GPU test (2026-09-26, 04:31 to 05:55)

Written on 2026-09-28 from the cell folders; the test agent was stopped by an API limit before it
wrote its notes. Plan: [PLAN.md](PLAN.md). Cases: (a) Vulkan clients created beside a live CUDA
holder until a guest chid equals a live host chid, (b) vkcube and glmark2 while Sunshine streams,
(c) the SotTR-shaped start, (d) host log, (e) the multi-client loop for the VA-space fix.

## Verdict

- **The doorbell token fix removes the cross-client stall.** Every repro iteration with the fix
  passed (vkcube rc 0, no c5c0 status 0x57); the baseline reproduced the stall in both of its runs.
- **The VA-space fix removes the host "VA Space alloc failed" bursts:** 0 host lines in the cells
  with it (D, E) against 155 in the cells without it (A, C).
- **The flush deferral works once the token fix is in** (cell C): no-client cursor 59.9 kicks/s,
  latch to next kick p50 2.2 ms; streaming 35.3 kicks/s. New work while streaming completes.
- Host Xid 0 in every cell.

## Cells

| Cell | Binary | (a) repro | (b) work while streaming | Collisions avoided | Host VA-alloc lines |
|---|---|---|---|---|---|
| A | tokenfix-d1084c0a | 5 of 5 pass | vkcube pass, glmark2 18-19 FPS | 49 | 155 |
| B | invhold-3bad9810 (baseline) | vkcube abort, 0x57 | stopped: own guard at MemAvailable < 3 GB | - | 31 |
| B2 | invhold-3bad9810 (baseline) | - | vkcube abort twice (0x57), glmark2 1 FPS | - | 0 |
| C | combo-3111dbf9 (+ deferral) | 5 of 5 pass | vkcube pass, glmark2 27-33 FPS | 60 | 155 |
| D | tokvas-73d882e3 (+ VA-space fix) | 8 of 8 pass | vkcube pass, glmark2 18-20 FPS | 11 | 0 |
| E | night-c117ce9e (all three) | 8 of 8 pass, vkcube 5-6 s | vkcube pass, glmark2 21-22 FPS | 5 | 0 |

- C's first boot failed at VM start (`C-combo-upfail`) and was repeated.
- Pacing of C: `pace-C.txt` (idle 47.4 kicks/s, cursor without client 59.9, cursor while streaming
  35.3 with latch to kick p50 18.3 ms).
- Device logs larger than 1 MiB are archived: see `caraxes/runs/ARCHIVE-2026-09-28.tsv`.

## Consequence

- All three fixes are in `dev` (the token and VA-space fixes as the Codex branch's equivalents
  583d995b and efb2c307, the deferral as 02c0a625, default on with
  `CARAXES_BAR2_MEMFD_DEFER_FLUSH=0` as the switch back).
- Streaming stays below 45 kicks/s; the remaining gap is Sunshine's per-frame chain in the guest
  (display-pacing.md §4).

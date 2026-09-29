<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# SotTR on Linux: the held invalidate, the BAR2 syncs and the gate (2026-09-29)

Scope: SotTR (Linux 615 guest, 4 GiB, display reserve on, 12 GB RAM) stuck on its loading screen,
the cause, the fixes on `invgate-2026-09-29` (f2af77c6) and `bar2victim-2026-09-29` (f49bb314),
the benchmark and the four-guest gate. Evidence: `/mnt/vmstore/caraxes/runs-2026-09-29/sottr-stall/`
(`S/` below), `/mnt/vmstore/caraxes/runs-2026-09-29/gate-invgate/`, `caraxes/runs/invgate-2026-09-29/`.

## The stall (P43)

- The game polled TRIGGER of `NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE` inside VRAM allocations and
  DMA maps: 90 % of its samples, 250-570 ms per ioctl, 3-4 per second [S/guest-perf-1228.txt,
  S/guest-ioctl-hist-1227.txt].
- 54213 held invalidates, 1850 s held (34 ms mean, 794 ms longest), 3.47 M busy trigger reads; the
  holds waited for other roots' jobs in the FIFO shadow worker and the other roots' deferred plans
  each hold flushed first [S/gpu-exit.log].
- Fix (f2af77c6, docs/caraxes/execution.md §8.1): TRIGGER busy only until the invalidated root's
  jobs are applied (ALL_PDB: the roots whose GPPut store does not trap); per-root worker queues, a
  waited-for root first; per-job progress.

## The benchmark (f2af77c6)

- Past the loading screen; SotTR 1080p: Average 15.4, Max 106.7, Min 0.0 FPS (native 91.7)
  [dmem-2026-09-29/drsv3-on-2/benchmark-result.txt].
- 20 s of benchmark (profile dumps .2/.3): 624 MMU invalidates/s at 889 us of device time each,
  648 BAR2 full syncs/s at 452 us, 457 VRAM window maps/s, 1368 flush reads/s
  [S/benchmark-window-20s-device.md].
- Whole run: 281680 BAR2 syncs in 115 s (195193 at invalidates with a flush owed, 93 s); 157815
  window maps; 57 GB of VRAM reads with no caller known; 9779 BAR1 pool polls over 2 ms, the
  slowest 441 ms; display clock ticks at 1 ms or more behind: 25 % [S/gpu-exit-on-2.log].
- Next (f49bb314, bar2-memfd.md §3 rule 6): the BAR2 victim cache (page tables that leave BAR2
  keep their snapshot; re-entries and reads without an uncached VRAM read) and the VRAM reads by
  caller. Open: the pool polls under the device lock (pool agent), the window geometry A/B.

## Gate

| Build | nova 5-probe | vm10 R1-R8 | Linux B kick-smoke | Windows smoke |
|---|---|---|---|---|
| f2af77c6 | PASS (5 names, 4 unloads, 0 ERROR) | PASS: vk-offscreen, `vkCreateDevice` 70.4 ms, glmark2 3483, vkmark 4736 beside Windows C60b, 0 ERROR | PASS: vkcube x11/wayland, glmark2 build 3165, NVENC holder, stream shots change, guest Xid 0, no host Xid; 3 ran-ahead lines | next Windows session |
| f49bb314 | PASS | queued | queued | next Windows session |

## The ran-ahead detector and the scoped hold

- The detector (mirror.rs `note_ran_ahead`) fires at a doorbell whose host channel already
  fetched entries published through the untrapped GPPut while plans were pending.
- Counts:

  | Run | Build | Doorbells | Ran ahead with plans pending |
  |---|---|---|---|
  | Linux B, openrender gate | 5fcf62ae (old hold) | not kept | 2 logged |
  | Linux B, invgate gate | f2af77c6 | 402841 | 3 |
  | SotTR stall | P43 (old hold) | 252874 | 60 |
  | SotTR drsv3-on-2 | f2af77c6 | 364709 | 50 |

- Not more likely: SotTR 60 per 253k doorbells before, 50 per 365k after; Linux B 2 and 3.
- Not more dangerous: a channel of root r translates through r's shadow only. The guest's order is
  map, invalidate, poll until TRIGGER FALSE, GPFIFO entry, GPPut; TRIGGER reads FALSE only when
  r's jobs are applied, before and after the change. The old hold waited for other roots' jobs
  too, which do not change r's translations. Hazards outside that order are unchanged: mappings
  deferred while r's GPPut still trapped (planned at the next doorbell or hold of r), MEM_OP
  invalidates (their hold is unchanged), and mappings made without an invalidate (not bound on
  hardware either).
- The detector's "plans pending" was global (any root), so the scoped hold can only make it fire
  for other roots' plans. It now also reports plans of the channel's own root and logs ERROR only
  for those (WARN otherwise); the summary counts them apart (3c4f7dbc, `ahead_own_root`).

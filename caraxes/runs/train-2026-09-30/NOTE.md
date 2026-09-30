# train-2026-09-30: lf24 plus the open day-branch commits (2026-09-30)

- Base: lf24 = win-lf18-2026-09-29 at 991e3d86 (binary a8e754b7, 844 tests).
- Worktree `/mnt/vmstore/caraxes/wt-train`, branch `train-2026-09-30`. Not merged into dev, not pushed.
- Open commits found with `git log --no-merges --cherry-pick --right-only win-lf18-2026-09-29...<branch>`.
- The ctxcreate branch has a third device commit besides 0fd0f3df and 8fa5af25: 83bfe733 (prefetch, admin copy channels, timing rows).

## Verdicts

| Commit (branch) | What | Verdict | Evidence / reason | On train |
|---|---|---|---|---|
| 83bfe733 (ctxcreate) | admin (root) processes' copy channels mirrored | TAKE, ported | Root copies measured on the gfxp base: d2h 16 MiB 52.3 -> 1.34 ms. On this base: lf24 L1 21.7/68.0 ms, train 1.41-1.51 ms, 0 Xid. The Unix-only limit is dropped: the privilege level is the RM's own on every guest OS (615 kernel_channel.c:288-297), and a Windows KMD's channels are KERNEL | 9b5ffe9f |
| 83bfe733 (ctxcreate) | shadow prefetch ahead of the first channel, timing rows | LEAVE OUT | Conflicts with the lf24 design: the build path moved to the worker, off the device lock (a883c6f2, f1985978); 16 mirror.rs hunks. Gain on its own base 303 -> 263 ms (census-user, with 8fa5af25), bench mean 251 -> 233/258, inside about twice the ±25 ms spread. The earlier prefetch 56aacd2a was left out at 46dd2582 for the same reason. Re-port is its own task | - |
| 8fa5af25 (ctxcreate) | pool barrier skips prefetched shadows | SUPERSEDED | 2a0dbb41 + 0667070d: the pool no longer waits for the shadow worker under the device lock. It serves only the prefetch | - |
| 0fd0f3df (ctxcreate) | freed gather descriptors cached for the same pages | A/B NEEDED -> TAKE | A/B below: cuCtxCreate -52/-60 ms per pair; gather time per session 2.1-2.6 s -> 0.77-0.82 s; 0 Xid; 0 cached at exit | 148165cd |
| 6f4ff4cf 655d11b6 6454304a b1f5ba65 be3cb80b 357ad887 (ctxcreate) | run script, records, research note, Leandro-Test suite scripts | TAKE (docs/runs) | Clean; README conflict kept both sides | db1e5a1e..2bbfc07f |
| d64c7d1d (bar1pool, poolwin) | BAR1 pool: no worker drain in the GPU check; slices behind the trigger | SUPERSEDED | 48948b2e in lf24 is the same change on per-root progress (range-diff: only the ghost bookkeeping per root differs), then 2a0dbb41, 0667070d | - |
| 95f1af11 (vblank) | display-page acks no pool sync point | SUPERSEDED | f294c275: the display page's trapped writes are served before the device mutex and never reach `after_bar0_write` or the pool poll | - |
| 95f1af11 (vblank) | in-place BAR2 page compare | SUPERSEDED | 5fcf62ae (dev): `SharedView::page_equals` | - |
| 95f1af11 (vblank) | trapped acks feed the delivery histograms; one trace flush per write | TAKE, ported (diagnostics, no behaviour change) | 'display page delivery' stayed 0 of 0 with the read-only page; test trapped_acks_feed_the_delivery_histograms | 026b837d |
| 95f1af11 (vblank) | research note vblank-path-2026-09-29 | TAKE (docs) | With a status section: where each fix is on train | fb3813b5 |
| 20 commits e84a3b42..54cde715 (dmem) | display reserve v3 for SotTR's guest NVKMS OOM: guest nvidia-modeset/RM patch (patches/nvidia, game tests only), guest scripts under crates/vfio-user-nvgpu/guest, test pack, docs, runs | TAKE (docs/tools/guest) | No device source touched (crates/ changes are guest scripts only); clean cherry-pick | fe5a7b0b..6333b1bc |
| 39cfd033 (bench-pack) | render benchmark pack | TAKE (tools) | Clean | 54d1abdd |
| eff0d449 (passthrough-plan) | R4 plan, passthrough capture scripts | TAKE (docs/tools) | Clean | 95552b3b |
| a848385e (win-lf23-remap) | Windows step 21 docs, COMPRESSION_SIZE item closed | SUPERSEDED | lf24's 025b675c has step 21 in full (C87-C91). Its compression-guest-view line closes the item with fc4c297e (D16), which is the Windows agent's | - |
| 5cf08793 (uvm-replay-poc) | replayable shadow faults PoC | LEAVE OUT (PoC, own branch) | Applies to train with 3 add/add hunks (boot.rs, device.rs, mirror.rs; keep both sides, boot.rs and device.rs need the closing brace between them); then 851 tests pass. Not committed | - |
| fc4c297e / 506b3ca7 (D16), CARAXES_BAR2_MEMFD=1 on Windows | - | not mine | The Windows agent's A/B | - |
| new | clippy type_complexity on FakeFb::live_tables (d3f91625) | fix | tools/check.sh's clippy gate (-D warnings) failed on lf24; a type alias, no behaviour change | deeeaab7 |

## A/B: gather cache (0fd0f3df)

- Method: the ctxcreate branch's bench mode (run.sh). Guest vm2-615 (615), 1 GiB, VRAM slot 4, trace off, CARAXES_PROFILE=1.
- Each session: bench-cu init ×20, churn 20 ×3, census-cu ctx 5 and ops 10, then bench-cu copy as user and as root, and root init ×3.
- Script: `ab.sh`, sessions in `ab/`, table from `ab-table.py`.
- No BAR1 pool (MX_NOPOOL=1) in any session. Beside the Windows VM the host had 160 MiB of BAR1 free, and the pool's 96 MiB would break the 96 MiB floor, so the first try was refused (`ab/refused-bar1-floor`).
- Binaries:
  - train1 026b837d, without the cache, sha 3f3c51c0;
  - train2 148165cd, with the cache, sha cd9c9432;
  - lf24 control a8e754b7 (`vfio-user-nvgpu-train0-991e3d86`).

| Session | Binary | Beside | cuCtxCreate mean / median (fast half / slow half), ms | destroy med | gather alloc (profile) | root h2d / d2h 16 MiB, ms | Xid |
|---|---|---|---|---|---|---|---|
| A1 23:51 | train1 | Windows C93 | 292 / 280 (227 / 345) | 119 | 876, 2396 ms | 1.42 / 1.41 | 0 |
| B1 23:54 | train2 | Windows C93 | 240 / 240 (207 / 274) | 108 | 867, 768 ms (486 from the cache) | 1.43 / 1.41 | 0 |
| A2 23:57 | train1 | desktop-b booting | 316 / 305 (251 / 382) | 117 | 862, 2604 ms | 1.43 / 1.43 | 0 |
| B2 23:59 | train2 | desktop-b codec | 256 / 253 (231 / 275) | 116 | 855, 818 ms (477 from the cache) | 1.51 / 1.42 | 0 |
| L1 00:03 | lf24 | desktop-b codec | 259 / 255 (214 / 301) | 105 | 897, 2104 ms | 21.7 / 68.0 | 0 |

- A second round (B3, A3) was queued. It was stopped before it started at 00:41 because the priority file named Windows from 00:05 (C95/C96).
- Before that, a first try with the 32 MiB pool was refused at the host BAR1 floor: `ab/refused-bar1-floor`.

- Native (ctxcreate note): cuCtxCreate 129-130 ms.
- Load was not the same across sessions:
  - the host RM's RM_ALLOC_MEMORY cost 0.63 ms mean in L1, against 1.5-2.8 ms in A1-B2;
  - so L1 ran on a quieter card, and its user numbers are no train1 control.
- The cache mechanism is independent of that load:
  - about 56 % of the gathers came from the cache;
  - the worker's gather time per session fell by 1.3-1.8 s;
  - every exit line says 0 cached at exit (freed at the teardown: no leak);
  - churn and destroy are unchanged within the noise.
- Verdict TAKE: both same-load pairs favour the cache by 52-60 ms, most of it in the slow half of the alternating contexts (-70 to -107 ms), which is what the note predicted.

## Train head

- Branch `train-2026-09-30` at **c3e9bd5b**: 34 commits on lf24 991e3d86.
- Binary `/mnt/vmstore/caraxes/vfio-user-nvgpu-train3-c3e9bd5b`, sha256 14c5e728733f2836... Its code is that of deeeaab7; the later commits are docs.
- `cargo test -p vfio-user-nvgpu`: 848 passed, 1 ignored (lf24: 844).
- clippy with tools/check.sh's flags (`-D warnings`) is clean.
- Logs: `test-final.log`, `clippy.log`, `build-final.log`.
- Reads of host GPU state:
  - the launcher's and device's own start checks: /proc/driver/nvidia/version, `nvidia-smi -q -d MEMORY` for host BAR1, host VRAM free from the host RM;
  - after each session, `journalctl -k -g Xid` (mx_finish).
  - No module, register or parameter was touched; sudo was used only by the launcher's `setcap cap_sys_admin+p` on its binary copy.

## For the Windows agent

- Add your D16 (506b3ca7) and Windows BAR2 memfd verdicts on top.
- With D16, also take the compression-guest-view-2026-09-28.md line of a848385e (the COMPRESSION_SIZE item closed).
- Then run the one four-guest gate on the final candidate and merge into dev.
- Not gated here: nova 5-probe, vm10 R1-R8, Windows. The admin rule leaves Windows unchanged by design (no ADMIN-level channels there), but the gate should confirm it.

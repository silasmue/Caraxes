<!-- SPDX-License-Identifier: MIT -->
# vm4-615 tokenfix: GPU test plan

Binary: `/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-tokenfix-d1084c0a` (branch `tokenfix-2026-09-26`,
commit d1084c0a on main dbc3950e; md5 afd3aabf). Baseline for A/B:
`vfio-user-nvgpu-v615-invhold-3bad9810` (the same stage settings, the old doorbell order).

What changed (docs/caraxes/doorbell-token-pool.md (e) rules 9-10):
- `ctrl.rs` `route_token`: a trapped value is resolved guest ChID first. The host token is used
  only when the value names no guest channel. On a collision both channels are rung, and the
  trace shows `[collision avoided]`.
- `pin_pool_token`: a mirror scheduled in a doorbell gets its pool token in its notifier entry
  right away (only over the proven host token).
- Exit line: `doorbell routing: N ring(s) by host token, M collision(s) avoided, K pool token(s)
  written at the mirror's scheduling`.

Stage: `caraxes/runs/sottr-2026-09-24/up-615.sh` with `SOTTR_SERVER=<binary> CARAXES_MEM=8G`,
`SOTTR_NAT=0`. Sunshine is stopped unless a case says otherwise. The guest tools are
`~/nvenc/rmioctl-log3.so` and `~/nvenc/repro2.sh` (copies in
`../vm4-615-cuda-vulkan/`). Record the device trace window per case. Follow the host-sharing
protocol (win-busy, linux-busy).

## (a) The collision itself (CUDA holder alive)

- `bash ~/nvenc/repro2.sh tf-a 5` five times: a long vkcube, the CUDA NVENC probe held, then
  five short vkcubes.
- Collisions must occur, so a run counts only if the trace shows at least one
  `[collision avoided]` note, or the summary counts one. If none occurred, repeat until one
  does: create more GR channels while the holder lives (extra short vkcubes or
  `vulkaninfo --summary`) until a guest chid equals a live host chid.
- Pass:
  - For each colliding ring, the named guest channel's first submission runs (a
    `shadow: GR chid N ... scheduled` or `doorbell chid N` line within 50 ms of the ring).
  - Every vkcube returns rc 0 or 124 (the `--c 30` frame limit); none returns 134.
  - No `status=0x57` in any `vk-*.rmlog`.
  - 5 of 5 runs.
- Baseline on invhold-3bad9810: the same case, expected to fail as in phase X (vk 1 rc 134,
  0x57 after about 15 s).

## (b) While Sunshine streams (the flippace symptom)

- Sunshine with NVENC and a Moonlight client streaming (`stream-615.sh`), then `vkcube-wayland --c 300`
  and `glmark2-es2-wayland -b build` started in the session.
- Pass: 4 of 4. vkcube renders (rc 0/124, no 0x57), and glmark2 reports a score above 0 frames.
- Also run once with the flippace binary's deferred flush sync, if that build is rebased onto
  this fix, since its 4 of 4 failures are the same symptom.

## (c) The SotTR-shaped case

- Sunshine with NVENC streaming to Moonlight, then start a Vulkan application: SotTR through
  Steam, or vkmark if Steam is not wanted.
- Pass: the application creates its device (no `demo_create_device`-style abort, no signal 6 in
  the SotTR crash folder) and renders for 2 minutes while streaming.

## (d) Always

- Host: 0 Xid lines, from `journalctl -k` since up. Count every `NVRM:` line, including
  `virtmemAllocResources: VA Space alloc failed`, per case.
- Device: no `TLB` or `invalidate` failures in gpu.log (`grep -iE 'tlb.*(fail|refus)|invalidat.*fail'`),
  no `ERROR freeing host TSG`.
- Exit lines to report per session:
  - `doorbell routing:` (collisions avoided, by host token, pinned at scheduling);
  - `doorbell tokens:` (mismatch, own-token rings, stale);
  - the soak/exit line.
- Guest: no Xid in dmesg; the nvidia-drm flip-event warnings are known and only counted.
- Report a pass/fail table per case with the A/B against invhold-3bad9810.

<!-- Section (e) only, from branch vaalloc-2026-09-26; cases (a)-(d) are on tokenfix-2026-09-26. Append at merge. -->

## (e) VA alloc bursts at a multi-client teardown (not run)

- Binary: `/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-vasdup-30a92496` (commit 30a92496).
- Why: docs/research/va-alloc-bursts-2026-09-26.md.
- Setup: vm4-615, one VM beside the desktop, the same environment as tonight's loop (BAR1 pool on).
- Run: the looping multi-client iteration that produced the 04:37-04:48 bursts: a Vulkan client
  (vkcube) beside a live GR client, created and torn down per iteration. This is the concurrency
  F6 `p3-during-vk` shape. Run at least 5 iterations.
- Record per iteration: the host `journalctl -k` window, the device trace window around each
  client teardown, and the device's `VAS of guest root ... freed` summaries.
- Pass:
  - 0 host `virtmemAllocResources: VA Space alloc failed` lines;
  - 0 `shadow: region ... object refused` and 0 `per-leaf VA object` lines in the device trace;
  - every `VAS of guest root ... freed` summary shows `0 map-failed` and `9 refused` is gone;
  - a `registry: vaspace ... kept under ...` note at the free of the VAS-owning client;
  - `set-B range(s) of the freed guest channel held out of set A` notes at the GR channel frees;
  - host Xid 0; the guest's clients pass as before.
- Fail: any of the above, or a `bar1 pool re-sync: shadow: root ... released back to set A` line
  before the host TSG frees of that root.

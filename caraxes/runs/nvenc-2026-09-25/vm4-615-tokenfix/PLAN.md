<!-- SPDX-License-Identifier: MIT -->
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

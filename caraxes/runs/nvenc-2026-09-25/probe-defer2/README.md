<!-- SPDX-License-Identifier: MIT -->
# probe-defer2: killed at host BAR1 exhaustion, no result

- Setup: binary `vfio-user-nvgpu-nvenc-defer` (commit 3279a532), 610 guest on vm2, 2 GiB,
  1 GiB at slot 3. BAR1 pool 32 MiB and 2 windows of 32 MiB (a budget of 96 MiB). The game
  VM restarted in parallel without its pool (64 MiB of windows). The host had 3768 MiB of
  VRAM free before the start.
- 12:26:40.21: host BAR1 exhausted. 42 lines in `host-kernel-window.txt`:
  - 28 `dmaAllocMapping_GM107: can't alloc VA space for mapping`;
  - 14 `NV_ERR_NO_MEMORY` from the BAR1 reuse-mapping DB (`mapping_reuse.c:273`);
  - a pair every 15 to 18 ms.
- The run's watcher stopped device pid 201962 at the first line; the coordinator's guard did
  the same. The coordinator saw host BAR1 at 120 MiB with vm2 up (12:25:48), then more than
  130 MiB more within a second.
- Where the run was: in the probe step's driver reload (the function-level reset, then
  RmInitAdapter at guest time 249.9 s), before any CUDA or NVENC call.
  - The guest's RmInitAdapter failed at the VBIOS parse because the device was already
    gone (the PROM read 0x88888888).
  - The probe's `cuInit` then returned 999.
- So the BAR1 did not go to the probe's CUDA context, its frames or the mirror's video
  surfaces: none of these existed yet.
- What maps BAR1 on the host during a driver reload, as far as the device's configuration
  goes: the pool (32 MiB, carried across the reset) and the reservation windows (up to
  2 x 32 MiB, opened lazily). A jump of more than 130 MiB exceeds that budget, so the source is
  not established by this record.
  - Checked and ruled out: the reset builds the fresh model before the old one drops
    (`boot.rs` `BootModel::reset`), but the fresh model opens no window at construction; and
    the RM's reuse DB unmaps a mapping when its refcount reaches 0.
  - Open: whether vm2's windows or the game VM's windows opened in that second.
- For the next run: `run.sh` records host BAR1 used every 0.5 s for the whole run
  (`host-bar1.txt`). That separates vm2's windows from the game VM's start.
- vm2 was brought down afterwards (none of its processes left); no Xid.

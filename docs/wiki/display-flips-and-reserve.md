<!-- SPDX-License-Identifier: MIT -->
# Display flips and the display reserve

The emulated display engine latches a guest's window flips on its own clock and writes the
flip notifiers and semaphores the drivers wait for. Three drivers read the notifier
differently, so its timing is fixed by what all three accept. This page covers that model,
the latch point, the cost of a vblank, the modeset and reset cases, and the VRAM reserve a
Linux guest needs so that NVKMS can always allocate a scanout buffer. The overview is
[display-and-kms](display-and-kms.md). Step numbers and run names (C78) refer to
[research: windows-findings](../research/windows-findings.md).

## The flip notifier

- A window `UPDATE` latches at a tick once its acquire semaphore holds, `MIN_PRESENT_INTERVAL`
  ticks have passed since the window's last latch, and any `SET_UPDATE_TIMESTAMP` time has
  come (`crates/vfio-user-nvgpu/src/display/engine.rs` `window_update`;
  [caraxes/display-connector.md](../vfio/display-connector.md)).
- The notifier states:
  - NOT_BEGUN, as the guest left it, until the latch;
  - BEGUN with the latch's PTIMER time at the latch;
  - FINISHED when the next presentation displaces it;
  - FINISHED at once on a head without scanout.
- Why: the Windows KMD presets its notifier slots FINISHED ("slot free") and resets one to
  NOT_BEGUN before a flip. About 0.6 ms after programming the flip it checks the notifier; a
  FINISHED there sends it into a GET/PUT poll and a `TdrPayloadVSyncTimeout`. nouveau waits
  for exactly BEGUN after its `UPDATE` (`base507c_ntfy_wait_begun`); NVKMS headsurface
  asserts BEGUN for the previous flip (`nvkms-headsurface.c`). nvidia-drm reads neither and
  waits for `AWAKEN` (steps 5 to 16). Test: `a_displaced_flip_is_finished`.
- Rejected on Windows: FINISHED at the latch (a VSync TDR for every flip that latched inside
  the KMD's 0.6 ms, C71 to C76) and BEGUN without FINISHED (no unblank after DWM's modeset,
  C53, C54).

- Timestamps are PTIMER, from one device clock per process that an FLR does not restart,
  PTIMER-granular (low five bits 0). A flip-away release writes the record's timestamp;
  `TIMESTAMP_MODE` holds a flip until PTIMER reaches `SET_UPDATE_TIMESTAMP` (82fb8741, step 5).

## The latch point and the clock

- The card latches armed updates at the blank's start and raises `LAST_DATA` there. The
  device's tick is the blank's first line (`RASTER_BLANK_START`, else the viewport height);
  `GET_RG_SCAN_LINE` reads that line at the tick, and an `UPDATE` armed after it waits for
  the next frame (test `the_latch_is_at_the_blank_start_line`; step 15).
- One timer per active head with absolute deadlines, re-armed at every modeset. The clock
  runs the engine at every tick; an owed FINISHED keeps its head ticking (44c7c9a4, step 8).
- `SET_GET_BLANKING_CTRL` reads the head's state: BLANK (0x1) while blanked, UNBLANK (0x2)
  otherwise (ed82e0db, step 4).
- The engine has its own lock: the ticker, the FE interrupt status registers and the scan
  position never wait for the device lock; FB records go through record windows
  ([caraxes/display-own-clock.md](../vfio/display-own-clock.md) §7). The clock's driver:
  p99 at most 4 ms, max 6.5 ms (C67), against p99 524 ms, max 757 to 801 ms (C64, C65;
  step 13). Channel kicks (`PUT` writes) still take the device lock.

## The vblank path on Linux

- The display page: the FE interrupt status page at BAR0 0x611000 is mapped read-only
  (Cloud Hypervisor patch 0007, [patches](../../patches/README.md)), reads are memory,
  writes trap into `display/fe_page.rs` `FePage::write`. The kick page makes the window
  channels' `PUT` memory; the reactor runs the kick. `CARAXES_DISPLAY_PAGE=1` is an open
  switch, set by Linux B's launcher only ([flags](../../caraxes/runs/flags-2026-09-30/NOTE.md)).
- Costs in the traced vkmark run of 2026-09-28 ([research: vblank-path](../research/vblank-path-2026-09-29.md),
  [research: vkmark-frame](../research/vkmark-frame-2026-09-28.md)):

| Step | p50, cube / clear |
|---|---|
| vblank ISR, latch to ack | 642 / 572 us |
| one display-page write, of it a BAR1 pool replica merge about 110 to 160 us | 147 to 180 us |
| six `TIME` reads per vblank | about 30 us each |
| window kick (each context DMA resolve reads the whole 8 KiB RAMHT, inferred from the code) | 229 to 237 us |

- Since the 2026-09-30 train, display-page writes are served before the device lock and are
  no sync point (f294c275); before, Linux SotTR spent 42.6 s in 41k trapped acks under the
  lock (step 13). Expected: completions held per latched vblank from 0.6 to 0.75 ms down to
  0.3 to 0.45 ms; an estimate, the A/B was never run.

## Modeset and reset

- A window drops its last latch when its head's clock restarts (a modeset) or it changes its
  owner head (32b1f25b, test `a_modeset_restarts_the_present_interval`). Before, a 1080p to
  720p modeset left every flip held by `MIN_PRESENT_INTERVAL` for about 9 minutes and the
  KMD reset the GPU (step 17).
- After an FLR the host's non-stall events must reach the new mirror. The RM arms a notifier
  with REPEAT only from DISABLE, and freeing an event does not disarm it, so the new
  mirror's REPEAT was refused with 0x40 and no completion reached the guest. The mirror now
  sets DISABLE before it frees an event and re-arms DISABLE then REPEAT (flrdisp1;
  [research: flr-host-events](../research/flr-host-events-2026-09-29.md)). It applies to a
  VM reboot and a Linux reset too.
- Host non-stall events fire for every client of the host GPU. In the KMD's reset and
  shutdown paths its handler re-arms the MSI without acknowledging, and level semantics sent
  19.09M MSIs (C78). The device withdraws pending host hints (vectors 4 to 9) after 256
  re-arms without an ack and mutes them for 1 s or until an ack or enable; its own vectors
  are never withdrawn (26e67b26, step 18).
- The display vector is raised only while the FE line is asserted (8a12eada); a reset drops
  raised vectors and resets the boot model before the interrupt page (a76dd4a3; steps 11, 12).
  Open: the `NV_PCFG` mirror answers the BARs with their sizing masks (step 17).

## The display reserve for NVKMS (Linux)

- A game fills the free VRAM, then NVKMS's SCANOUT allocation (LOCAL_USER, contiguous, ISO,
  five adjacent 2 MiB pages at 1080p) is refused: "Failed to allocate NVKMS memory for GEM
  object", and the compositor stays wedged. The driver keeps no display reserve natively.
  WDDM's VidMm evicts application allocations for display surfaces, so Windows should need
  none (not measured) ([research: display-balloon](../research/display-balloon-2026-09-29.md)).

| Remedy | Result |
|---|---|
| v1's balloon in `virtio_nvrm.ko` (R = 5 S plus cursors, released on a refused kernel ISO request and retried in the call) | 0 NVKMS failures, against 7 of 9 SotTR runs frozen |
| more VRAM (5 GiB, no cap) | the game takes 94 to 97 % of the free VRAM at every size; the same refusal, later; no leak ([dmem-cap](../research/dmem-cap-2026-09-29.md)) |
| dmem cap on `app.slice` | the cap held, but 175 MiB grew outside it against R = 84 MiB; parked ([dmem-cap](../research/dmem-cap-2026-09-29.md)) |
| the 615 RM patch, v3 | SotTR 1080p at 4 GiB: 11 refused scanout allocations served and taken back, peak 2 runs out, no NVKMS failure, no Xid ([display-reserve](../research/display-reserve-2026-09-29.md)); the benchmark ran with it ([sottr-invgate](../research/sottr-invgate-2026-09-29.md)) |

- [The patch](../../patches/nvidia/0001-nvidia-nvidia-modeset-hold-a-video-memory-reserve-fo.patch):
  NVKMS declares the need from the committed modes (`nvRmUpdateDisplayReserve`, called from
  `nvSetDispModeEvo` and grow-only from `nvFlipEvo`) through the kernel-only
  `NV0073_CTRL_CMD_SYSTEM_SET_DISPLAY_RESERVE` (0x730162, a local id). RM holds PMA runs
  (`display_reserve.c`), releases them in `_vidmemPmaAllocate` for a refused kernel ISO
  contiguous scanout request, and takes a freed run-shaped scanout allocation back in
  `vidmemPmaFree`.
- `nvidia-modeset` parameter `display_reserve_mib`: 0 off (the default), -1 automatic (5 S
  plus cursors per active head: 44224 KiB declared, 52224 KiB in PMA at 1080p), N MiB.
- Loaded only for game tests; the stock modules stay the default. Not served: a compositor's
  own GL framebuffers (user clients). Not taken back: a surface smaller than a run.

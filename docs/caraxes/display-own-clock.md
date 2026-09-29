<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# The display on its own clock and lock (plan, 2026-09-29)

Status: stages 1-3 coded on win-ch-2026-09-29 (§7), not yet measured on a guest. Evidence:
windows-findings.md step 10 (C61).

## 1. Why

- Windows TDRs as a VSync timeout: dxgkrnl waits for flip 0x4B, the KMD keeps reporting 0x4A.
- The display clock's catch-up waits for the device mutex: p99 524 ms, max 821 ms (C61); the
  catch-up itself p99 63 us. The holders: mirror builds (a new GR channel, 495 ms right before the
  timeout), shadow full walks (250-290 ms per VAS), paging doorbells.
- The guest's ISR reads (interrupt tree, display status) trap into the same mutex, so no vblank
  is reported while it is held, whatever the clock does.
- The card: the display engine latches, writes its records and raises its interrupt on its own
  clock; an ISR register read never waits for another engine's work.

## 2. Pieces

1. **Display core behind its own mutex** (`Arc<Mutex<DisplayCore>>`, "display lock"):
   - `Engine` (heads, windows, interrupt status, the ticker's work), the channels' PUT/GET, the
     kick walk, ASSY/ARMED state, the instance memory base, the auto dump's state.
   - `Display` (NV0073/NV5070/NVC370/NVC372 controls, DP/EDID, display object allocation,
     hotplug events) stays in the boot model and holds an `Arc` of the core.
2. **Guest memory for the core:** pushbuffers and sysmem records (Windows' notifiers) are read and
   written without the device mutex. `GuestMemory` is `!Sync` today (raw pointers, `Cell`
   counters); the core gets a read-mostly shared view of the same mappings (`Arc`'d regions,
   atomic counters), swapped under the device mutex at DMA map, unmap and registration.
3. **FB records without the device mutex:** RAMHT/context DMA objects (instance memory), FB
   semaphores and notifiers (Windows' acquire/release records at fb 0xf8f80000), resolved at
   `UPDATE`.
   - Record pages are pinned when resolved, under the device mutex where the FB store's
     translation lives (BAR1 pool remap, BAR2 memfd backing): the core keeps a small window
     per 2 MiB region used by records (one or two), admitted by the host BAR1 floor.
   - A translation change of a pinned page (a pool move, a BAR2 bind or unbind) is told to the
     core under the device mutex (it takes the display lock briefly) and the page is re-pinned.
   - Surface dumps (MBs, not time-critical) keep using the FB store under the device mutex.
4. **Trapped-access routing before the device mutex** (`SharedDevice::region_read/write`):
   - display registers (FE interrupt status, channel pages, ARMED/ASSY, the caps page, core
     PIO) go to the core under the display lock only;
   - the interrupt page's trapped tree stores (exact mode) go to `IntrPage` (its own lock);
   - the MSI re-arm (`NV_XVE_CYA_2`) goes to `IntrShared` (lock-free), trapped or ioeventfd.
   With the interrupt page (exact mode) and these routes the Windows ISR never waits for the
   device mutex.
5. **The ticker** runs the core's catch-up under the display lock only and raises through
   `IntrShared` (lock-free). It never takes the device mutex.

## 3. Lock order

- device mutex -> display lock -> record-window lock. Leaf locks: `IntrPage` state,
  `IntrShared` delivery.
- Code holding the display lock never takes the device mutex (the ticker, the routed accesses,
  `FePage` scans).
- Device-mutex code may take the display lock briefly: controls, channel allocation, FLR (a new
  core), DMA map changes, pinned-page translation changes.
- The shadow worker, invgate and the pool's barrier never take the display lock.

## 4. The four guests

- Linux B (615, display page + kick page): the reactor's `FePage` and `ChanPage` scans call into
  the core under the display lock instead of the device mutex; semantics unchanged; vblank and
  flips no longer wait for mirror builds (SotTR's Min-FPS-0 stalls share this cause).
- nova, vm10 (no scanout on the model): only the routing check on each trapped access.
- Windows: interrupt page in exact mode plus the routes; no stall of vblank or flip completion
  behind device work.

## 5. Stages and tests

1. Routing for `IntrPage` trapped stores and the MSI re-arm (small): unit tests with a held
   device mutex (a routed access completes).
2. Shared guest-memory view (medium): tests for swap at DMA map/unmap.
3. Display core split and its routing, record pages pinned (large): the engine's tests unchanged
   plus a held-device-mutex test (a tick latches, writes FINISHED, raises).
4. Four-guest gate; Windows: desktop soak with the long holders present (before invgate's async
   builds land) and without.

## 6. Open

- Record pages that move under the pool while a flip is pending: re-pin before the latch reads
  the acquire (the pool's install happens under the device mutex, which tells the core first).
- BAR2 memfd pages: the record window reads VRAM; unsynced BAR2 bytes of the KMD (a CPU reset of
  a record) must be merged as `FakeFb::read` does, so the pin carries the memfd view of that page.

## 7. As built (2026-09-29)

- **Stage 1** (1355def5): the interrupt page's trapped stores and the MSI re-arm are routed before
  the device mutex (device.rs `Routes`).
- **Stage 2** (f2795e86): guest memory is reached through a published view (guestmem.rs
  `MemView`), republished at every map, unmap and registration.
- **Stage 3:**
  - Record I/O goes through a trait (engine.rs `RecordIo`). `DeviceIo` is the FB store under the
    device mutex; `CoreIo` (display/core_path.rs) is the view plus the pinned FB record windows.
    A latch whose acquire, previous release, notifier or owed FINISHED is not reachable on the
    path writes nothing and waits.
  - `Display::engine` is `Arc<Mutex<Engine>>`, the display lock.
  - Record windows (core_path.rs `RecordPins`): a window `UPDATE` that resolves a semaphore or
    notifier in FB maps its 2 MiB region once (at most 4) through the VRAM store
    (fb.rs `share_record_window`, `SharedWindow`). The store serves that range from the same
    window. The BAR1 pool, the BAR2 memfd and BAR1 passthrough keep FB records on the device
    path, since the page is not the VRAM page itself for good there.
  - The ticker's driver tries the device mutex. When it is held, the tick runs without it
    (`DisplayRoute::tick`) and returns whether the FE line is asserted; the clock raises only then.
  - Routed registers: `Engine::handles` offsets (FE interrupt status, enables, head state, scan
    position) are read and written under the display lock only. The display page keeps its own
    path.
  - Device: the engine is published in `DisplaySlot` at start and after every reset. A reset
    first unpublishes it, waits out a running tick and clears the old record windows; the shutdown
    clears them before the FB store's.
  - Counted in the exit summary: "without the device mutex: N clock ticks, N register reads,
    N register writes, N FB record window(s), N region(s) left to the device path".
- **Not yet:** display channel kicks (`PUT` writes: the pushbuffer walk and `UPDATE`) and the
  core PIO pages still take the device mutex.

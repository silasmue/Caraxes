<!-- SPDX-License-Identifier: MIT -->
# Display phases C and D: flips on the device clock, Wayland, GNOME, the gates

Run 2026-09-24 on vm0, branch `worktree-agent-adbfba499c7110889` (from main `5de9b40`). Plan:
phases C and D of [display-connector.md](display-connector.md) §4 on top of phases A and B
([display-ab.md](display-ab.md), D15 DisplayPort SST). Evidence:
[../../caraxes/runs/display-cd-2026-09-24/](../../caraxes/runs/display-cd-2026-09-24)
(commands word for word in `commands.md`; each run has the guest command, the guest output and
the events of the device trace), the gate runs `caraxes/runs/gate-vdisplay-2026-09-24-3/` and
`caraxes/runs/gate-display-2026-09-24-2/`. Nothing scans out and nothing runs on the physical
GPU: the device ran without host execution, and the host GPU was reached only through the
existing unprivileged RM client (nvml forwarding).

## Outcome

**Phase C passes.** The stock 610.57.04 guest (`nvidia-drm modeset=1 fbdev=0`, `vblank` left at
0, no guest patch) sets modes on the emulated DP monitor, and flip completion runs on the
device's per-head clock:

| Check | Result | Run |
|---|---|---|
| First modeset (legacy `SETCRTC` 1920x1080@60) | taken; the core notifier FINISHED through the context DMA found in the display instance memory; no `Lost display notification` | c1 |
| 60 / 120 page flips, 1080p60 | 60/60 at 60.01 Hz (min 16.05, max 17.26 ms, rms 0.15 ms); 120/120 at 60.00 Hz (rms 0.18 ms) | c1, c2 |
| Mode switch 1080p60 -> 720p60 -> 1080p50 -> 1080p60 | 59.99, 60.00, **50.00**, 58.92 Hz; the device clock re-arms on each core `UPDATE` | c9 |
| vblank (`LAST_DATA`) serviced by the guest RM | NVKMS `NOTIFY_VBLANK` (the NV9010 / X swap-interval path): 120/120 notifications at 60.01 Hz (sd 0.39 ms); the guest kernel enabled `FE_RM_INTR_EN_HEAD_TIMING.LAST_DATA` for each request | c11 |
| Weston 13, DRM backend, atomic KMS, 1920x1080@60 | frame callbacks **59.86 Hz** over 2091 intervals (sd 1.1 ms), `wp_presentation` 59.86 Hz, refresh 16 666 666 ns, flags VSYNC, HW_CLOCK, HW_COMPLETION, 0 discarded | c8 |
| Weston at 1080p50 | 49.87 Hz, refresh 20 000 000 ns (the timer follows the mode) | d10 |
| Weston at 640x480@59.94 | 59.65 Hz and 59.39 Hz | c6, c12 |
| Hardware cursor (legacy `DRM_IOCTL_MODE_CURSOR`, 64x64) | set, 60 moves one frame apart and off all succeed; NVKMS drives cursor channel 3 (PIO `Update` and `SetCursorHotSpotPointOut`, 62 writes each); no kernel messages | d14 |
| Kernel log | 0 WARNINGs, 0 `Flip event timeout` after the AWAKEN rule below | c12, gates |

The expectation of display-connector.md §4 C was 60.0 ± 0.5 Hz for the frame callbacks; the
measured 59.86 Hz is inside it. The device clock itself reports 60.000/60.010/59.983 Hz and
50.000/50.078/49.966 Hz per 600 ticks (the `[clock]` lines of d10); what is lost between it and
the compositor is the occasional late frame on the guest side (below, "Jitter").

**Phase D passes, with limits.**
- **GNOME on Wayland is up** (d7): gdm3 autologin, session `Type=wayland`, gnome-shell 46 /
  Mutter: `Added device '/dev/dri/card0' (nvidia-drm) using atomic mode setting`, `Created gbm
  renderer`; colord and gsd-color run; NVKMS armed the head's output LUT (control 0x40508:
  DIRECT10, 1029 entries, LUT context DMA set); **0 `Page flip failed`**, 0 kernel warnings. The
  scanout read back through GETFB2 shows the Ubuntu wallpaper and the dark top bar. Rendering is
  Mesa llvmpipe through `kms_swrast` (the device has no GPU execution on vm0). A GNOME X11
  session came up first (d2/d3, Xorg modesetting) as the stepping stone.
- **EDID-change hotplug through POST_EVENT works** (d9): a host command swaps the monitor's EDID;
  the device posts `NV2080_NOTIFIERS_HOTPLUG`, the guest's NVKMS re-reads the EDID over AUX and
  nvidia-drm sends a `change` uevent; sysfs gains 800x600 and `edid-decode` names the new monitor.
  Unplug (sysfs `disconnected`, no modes) and replug with the first EDID (identical md5) work
  (d11, d12).
- **Mode switch**: 1080p60 -> 720p60 -> 1080p50 on the KMS level (c9) and for Weston's frame
  callbacks at 1080p50 (d10).
- **Cursor channel**: the legacy cursor ioctls reach NVKMS's cursor channel (d14); under GNOME
  no pointer device exists in the guest, so Mutter drew no cursor.
- Not reached: frame-callback rates under GNOME (d8: llvmpipe renders into dumb buffers through
  trapped BAR1; the first client map did not repaint within 200 s), GPU-rendered GBM/EGL (needs
  host execution; not allowed on vm0 tonight).

**Gates.** Both adapted gates pass on the final (rebased) build (verdicts in "The gates" below):
- `tools/caraxes-gate.sh vdisplay`: **pass** (setup, route, device, edid, pixel, flips, teardown);
- `tools/caraxes-gate.sh display`: **pass** (rig, connector, edid, modeset, compositor, capture,
  kernel), with v1's GPU-execution stages (swapchain, present, rt, events) listed as not run in
  the JSON facts, and Moonlight reception not part of the capture stage. The compositor stage sits
  at the edge of its criterion under host load: one run of the final build measured 59.50 Hz
  (fail by 0.01 Hz, load average 5.5 from the other agents), the next 60.00 Hz (load 2.6).

## What was built

| Where | What |
|---|---|
| `src/display/engine.rs` (new) | The phase C engine. Core `UPDATE`: window owners, per-head raster and pixel clock (the tick period), SOR owner masks (head active), OLUT state, `FE_CORE_HEAD_STATE`, the core notifier written FINISHED at once, core AWAKEN. Window `UPDATE`: resolves the notifier, semaphore and ISO context DMAs through RAMHT in the instance memory (`disp_inst_mem_0300.c` layout, scanned by handle and channel number because the hash needs the client), queues a presentation. The per-head clock latches presentations at ticks (acquire semaphore, `MIN_PRESENT_INTERVAL`, release at flip-away, notifier FINISHED with the tick time, AWAKEN_WIN), `BEGIN_MODE_IMMEDIATE` and windows on inactive heads latch at once. Every tick sets `LAST_DATA` and advances `RG_DPCA` and the LOADV counter. The interrupt registers of dev_disp.h v03/v04 (EVT_STAT, RM_INTR_STAT/EN of HEAD_TIMING, DISPATCH, STAT_CTRL_DISP, AWAKEN_WIN/OTHER) with write-1-to-clear and level re-raise. A ticker thread (`CLOCK_MONOTONIC`, absolute deadlines `t0 + n * period`) raises vector 156 while a head has work; the state advances on the server thread when the guest's handler reads display registers, so notifier writes precede the interrupt bits the guest sees. Rate reports every 600 ticks go into the trace |
| `src/display/mod.rs` | The channels' methods go to the engine (core, windows 0-31; window-immediate and cursor accepted); `GET_CHANNEL_INFO` reports a window BUSY while its `UPDATE` waits for a tick (NVKMS's "method pending"); `SYSTEM_GET_SCANLINE` from the clock; **a real `IS_MODE_POSSIBLE`** (head count, each head's `maxPixelClkKHz` against min(link budget, `head_clk_cap_khz`)); the four modeset-path controls of display-ab.md answered; hotplug events recorded and **connector commands** (`edid-alt`, `edid-main`, `unplug`, `plug`) that build `POST_EVENT` payloads |
| `src/display/edid.rs` | `monitor_edid_alt()`: the second monitor (product 3, "Caraxes DP2", 800x600 added) |
| `src/ver/v610/display.rs`, `display-layouts.txt` | `NVC372_CTRL_IMP_HEAD` layout (92 bytes, `maxPixelClkKHz` at 4) and the four control numbers |
| `chips/tu106/display.toml` | `head_clk_cap_khz = 1330000`, `gsp_vector = 192` |
| shared files (small hooks) | `bar0.rs`: `read` takes the guest memory, the display is consulted through `BootModel::display_read32/write32`. `boot.rs`: those two methods, the GSP falcon `IRQSTAT.SWGEN0` for device-originated events (and SWGEN0 routed in IRQMASK/IRQDEST), `post_display_command` (POST_EVENT 0x1003 on the status queue), the display's interrupt path in the interrupt tree, the engine summary at exit. `ctrl.rs`: one line recording event allocations. `device.rs`: one argument. `main.rs`: the interrupt path and the control file `<state dir>/display-ctl` |
| `guest/` | `kmsflip.c` (legacy modeset + flip pacing), `kmsgrab.c` (pattern -> GETFB2 readback hash; pixel samples of the active CRTC's framebuffer), `wlframes.c` (xdg_toplevel with `wl_surface.frame` and `wp_presentation`), `nvkms-vblank.c` (NVKMS `NOTIFY_VBLANK`, built statically on the host by `build-host.sh`), `weston-pace.sh`, `sunshine-kms.sh`, `display-up.sh`, `vblank-check.sh`, `gdm-wayland-setup.sh`, `gnome-check.sh`, `hotplug-watch.sh`, `build-wl.sh` |
| `tools/caraxes-gate.sh` | `vdisplay` and `display` adapted (below) |

Headless stays the default: without `--display` the fuse reads 1, the engine is inert and the
GSP interrupt registers are untouched.

## Design choices the runs confirmed or corrected

1. **AWAKEN follows nvidia-drm's model of the hardware** (c3-c8 vs c12). The first version raised
   AWAKEN for every presentation with a WRITE_AWAKEN notifier. Every Weston start then logged the
   `nvidia-drm-crtc.h:355` WARNING (an event without a pending flip) and every stop a `Flip event
   timeout`. nvidia-drm expects a flip event only for planes that were scanning out before
   (`__will_generate_flip_event`, nvidia-drm-modeset.c:96-140). The device now raises AWAKEN only
   when the window had a latched surface before the presentation: enabling a window writes its
   notifier without an event, disabling one (no notifier, no surface) raises one. Since then: no
   warnings, no timeouts in any run or gate.
2. **Per-head registers are 0x800 apart and NVKMS used head 3.** The first build decoded
   `FE_CORE_HEAD_STATE` only for heads 0-2 (an address-range bug); head 3 read 0, so the kernel's
   `kheadGetDisplayInitialized` said "not initialized". Fixed with a per-head decoder and a test for
   heads 3 and 7.
3. **LAST_DATA is enabled per consumer.** The guest kernel enables
   `FE_RM_INTR_EN_HEAD_TIMING.LAST_DATA` when a vblank callback is registered and disables it when
   it goes: 120 enables and 120 disables for 120 `NOTIFY_VBLANK` requests (c11). Wayland alone
   never enabled it (c1-c9): as display-connector.md §2.1 predicted, the compositors pace on flip
   completion only. The ticker therefore runs a head only while it has a pending presentation or
   vblank is enabled.
4. **The window is BUSY until the tick.** `GET_CHANNEL_INFO` answers BUSY while an `UPDATE` waits
   for its tick, so NVKMS's `IsChannelMethodPending` (flip queue, post-flip IMP, usage bounds)
   sees what hardware would show.
5. **GNOME needs `kms_swrast` by name** (d6/d7). With `LIBGL_ALWAYS_SOFTWARE=1`, Mutter's explicit
   EGL device makes Mesa refuse the software path ("Not allowed to force software rendering when
   API explicitly selects a hardware device") and gnome-shell segfaults in libgallium, so gdm falls
   back to the X11 greeter. `MESA_LOADER_DRIVER_OVERRIDE=kms_swrast` works.
6. **Hotplug is a GSP-originated event.** GSP-RM posts `POST_EVENT` on the status queue and raises
   SWGEN0; the kernel reads the queue in `kgspService_TU102`. The device does the same: SWGEN0 in
   the GSP falcon's IRQSTAT (routed in IRQMASK/IRQDEST, falcon and RISC-V views), the GSP stall
   vector, one `POST_EVENT` per hotplug event the guest allocated (hClient and hEvent recorded
   from the `GSP_RM_ALLOC` of `NV01_EVENT_KERNEL_CALLBACK_EX` with notify index 1). NVKMS then asks
   `GET_HOTPLUG_UNPLUG_STATE` (plug 0x100) and the DP library re-detects over AUX (85 transactions).

## Jitter

- The device clock is exact to the reporting precision (60.000 +- 0.03 Hz, 50.000 +- 0.08 Hz per
  600 ticks).
- KMS page flips: rms deviation from the period 0.15-0.27 ms on a quiet host, up to 3.5 ms with
  single late completions (52 ms) when the host was loaded (c9, load average about 6 from the other
  agents' work).
- Weston frame callbacks: sd 1.1 ms at 1080p60 (c8), 1.4 ms at 1080p50, 4.3 ms at 720p60 in d10
  (host load). The missed frames come from the guest side of the chain (MSI -> guest RM -> NVKMS
  deferred work -> nvidia-drm event -> Weston's 7 ms repaint window); the device's tick reports stay
  exact in the same windows.
- The latency from tick to guest event was not measured directly (the guest's and the host's
  monotonic clocks are not aligned); the sd above bounds its variation.

## BAR1: the cost of CPU access to scanout buffers

Dumb buffers live in vidmem and the guest maps them through BAR1, which the device traps access by
access (vfio-user round trips, each access also logged). Measured: **0.38-0.39 MB/s write,
0.14-0.37 MB/s read** (a 16-row probe, and kmsgrab's 640x480 frame in the gate). Consequences:
- a full 1920x1080 XRGB frame costs about 21 s to write and 22-60 s to read;
- v1's `drm-modeset --flips 30` fills two 1080p buffers first, so the display gate's modeset
  stage needs about 45-50 s where v1 allowed 45 (the gate now allows 300);
- Weston's first full repaint at 1080p takes about 2 min, and a client's first fullscreen map about
  25 s; after that only damaged rectangles cross BAR1, which is why the small-damage probe paces at
  the refresh rate;
- the vdisplay pixel check uses 640x480 (1.2 MB, about 3 s each way);
- GNOME with llvmpipe renders into the dumb buffer through BAR1 and cannot sustain frames.

The later fix (coordinator, 2026-09-24): hand the guest a direct mapping of the device's VRAM
windows (the host RM's BAR1 mapping fd as the vfio-user region's sparse mmap), so CPU access runs at
native BAR speed; the device then stops seeing BAR1 writes, which only matters for page tables, and
those go through BAR2/PRAMIN and CE. That is a separate package.

## The gates

The v1 display gates (`../Leandro-Test scripts/test.sh:452-1128`) stood on virtio_nvrm's virtual
display and on X. Adapted (`tools/caraxes-gate.sh`, verdict format unchanged):

**vdisplay** (fail-fast like v1): *setup* (nvidia-modeset, nvidia-drm `modeset=1` on the device's
display engine), *route* (replaces v1's descriptor-table checksum: every display control in the
device trace answered by the display model, none unanswered, none host-forwarded), *device* (one
nvidia-drm node, exactly one connector, connected, offering 1920x1080), *edid* (byte-identical to
the device's EDID, i.e. the golden file the unit test `edid_is_what_the_guest_exposed` ties to
`edid.rs`; `edid-decode --check` on the host), *pixel* (640x480 pattern written through BAR1,
`SETCRTC`, read back through `GETCRTC` -> `GETFB2` -> map, FNV-1a hash equal to the committed
reference; plus the device's latched surface: 640x480, X8R8G8B8, pitch 2560), *flips* (120 flips
at 1080p60 within 60 +- 0.5 Hz), *teardown* (no oops, only the known flip WARNING allowed).

**display**: *rig*, *connector*, *edid* as above; *modeset* (v1's `drm-modeset --flips 30` with its
unchanged criteria, budget 300 s); *compositor* (replaces X: Weston on the DRM backend, atomic KMS,
wlframes' frame callbacks and `wp_presentation` within 60 +- 0.5 Hz, refresh 16.6-16.7 ms, 0
discarded); *capture* (replaces v1's X capture and stream: under Weston with a client, the scanout
read through GETFB2 shows the client's grey at the screen centre, and Sunshine with `capture=kms`
finds the monitor over DRM and passes its encoder test on captured frames); *kernel* (v1's rule).
v1's *swapchain*, *present*, *rt* and *events* need the GPU to execute (Vulkan device, WSI, fences);
without host execution they are not run and the JSON facts say so (`gpu_stages_not_run`). v1's
Moonlight reception is not part of the capture stage.

Runs (vm0):

| Run | Build | Verdict | Key facts |
|---|---|---|---|
| gate-vdisplay-2026-09-24-3 | before the rebase (`3e52e36`+) | pass | 1437 display controls, 0 unanswered; frame 0x727dcdf3e49208c5 = reference; BAR1 0.38/0.37 MB/s; flips 60.00 Hz |
| gate-display-2026-09-24 | before the rebase | fail (modeset, compositor) | gate budgets: v1's 45 s against the BAR1 fill; the 1080p first repaint outlasted the measuring window |
| gate-display-2026-09-24-2 | before the rebase | pass | frame callbacks 59.90 Hz, presentation 59.90 Hz, refresh 16 666 666 ns; Sunshine capture=kms encoder test passed; sample 0x80808080 |
| gate-vdisplay-2026-09-24-4 | **final** (`a8485e4`, rebased on main `223075d`) | **pass** | 296 display controls (fresh device), 0 unanswered; frame hash = reference; BAR1 0.32/0.28 MB/s; flips 60.02 Hz; 0 warnings |
| gate-display-2026-09-24-3 | final | fail (compositor) | frame callbacks 59.50 Hz (criterion 59.5 excluded by 0.01), max interval 40 ms, 14 late frames of 1777; host load average 5.5 |
| gate-display-2026-09-24-4 | final | **pass** | frame callbacks 60.00 Hz, presentation 60.00 Hz; host load average 2.6 |

The first `display` failure was a budget error of the gate, fixed by measuring from the first
frame callback and a 300 s modeset budget; no device change. The final-build failure is the
guest-side latency under host load described in "Jitter": the device clock is exact, and a late
event costs the compositor one refresh.

## vm0 changes (recorded in experiments.md)

- Offline packages installed: weston 13, sway 1.9, libdrm-tests (modetest), edid-decode,
  wayland-utils, grim, wlr-randr and dependencies; libwayland-dev, libwayland-bin,
  wayland-protocols, libffi-dev.
- gdm3 enabled again (default target stays `multi-user.target`; gdm3 is started by hand after the
  display modules), `WaylandEnable=true`, gdm's NVIDIA udev rule masked, the user session
  `ubuntu-wayland`, `/etc/environment` with Mesa's EGL vendor file and
  `MESA_LOADER_DRIVER_OVERRIDE=kms_swrast`; gdm `[debug] Enable=true`. Originals kept as
  `*.caraxes-orig`; `gdm-wayland-setup.sh` prints the undo line.
- `/dev/nvidia-modeset`, `/dev/nvidiactl`, `/dev/nvidia0` are created by `display-up.sh` (the image
  has no nvidia-modprobe).

## Open items

1. **GPU-rendered compositors** (GBM/EGL on the NVIDIA driver, Mutter at speed, v1's Vulkan
   stages): need host execution on vm0 with the 3D class on the shadow GR channel; waits for the
   coordinator's "host-exec on vm0 allowed" (one VRAM reservation at a time today).
2. **BAR1 direct mapping** (above): the package that makes CPU-rendered and readback paths
   usable at 1080p.
3. **Cursor channel**: the legacy cursor ioctls work (d14), but the device only records the
   cursor surface and position (`HEAD_SET_CONTEXT_DMA_CURSOR`, PIO `SetCursorHotSpotPointOut`); a
   host sink would need them latched. GNOME had no pointer device to draw one.
4. **Gamma LUT content**: the OLUT control and context DMA are armed and accepted; whether the LUT
   came from colord or from NVKMS's default ramp was not distinguished (modetest's property dump
   was inconclusive).
5. **D5 at guest level**: the withheld-acquire case is covered by the unit test
   (`a_flip_completes_at_the_next_tick_not_before`); no guest client used display semaphores (they
   come with GPU rendering).
6. **Device tick count vs guest vblank count**: not compared (the guest's per-head counter is not
   exported); the NOTIFY_VBLANK count matches the requests.
7. **DPMS / `SET_MONITOR_POWER`** does not stop the clock; `SET_UPDATE_TIMESTAMP` (timed flips) is
   recorded, not honoured; `RG_DPCA`'s line count is interpolated from the clock.
8. **Moonlight reception** for the display gate's stream stage (needs pairing from the host).
9. The route stage counts every display control in the whole trace log, not only the gate's window.
10. **Compositor pacing under host load**: 0.5-1 % late frames when the host is busy; the gate's
    +- 0.5 Hz window is met at load average up to about 4, missed by 0.01 Hz at 5.5. Candidates:
    the trace write per BAR1 access on the server thread, the guest's deferred NVKMS work.

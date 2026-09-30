<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Display and KMS

A guest desktop needs a monitor the driver believes in, a vblank cadence, page flips that
complete, and a way to capture the result for streaming. Neither architecture scans out
physically. VirtIO Leandro uses NVIDIA's displayless class with guest timers; VFIO Leandro
emulates a DisplayPort monitor and the display engine in the GSP role, so the stock
`nvidia-modeset` and `nvidia-drm` on Linux and the WDDM KMD on Windows see ordinary
hardware. The flip, vblank, reset and reserve mechanics are on
[display-flips-and-reserve](display-flips-and-reserve.md).

## The stock display stack

- `nvidia-modeset` (nvkms, `src/nvidia-modeset/`) owns heads, windows, SORs and connectors
  through the NV0073 display-common controls and the display channels (core and window
  channels of the display IP, `NVC57D` and `NVC57E` on Turing's display v04.00); `nvidia-drm`
  exposes KMS on top.
- Detection and EDID: DP AUX through `NV0073_CTRL_CMD_DP_AUXCH_CTRL`, DPCD reads, link
  training (`nvkms-dpy.c`, `nvkms-rm.c`); mode validation through `IS_MODE_POSSIBLE`
  (`NVC372_DISPLAY_SW`).
- Vblank and flips: the core channel's `AWAKEN` and the `LAST_DATA` interrupt of the
  window channel; nvkms registers `NV9010_VBLANK_CALLBACK` and the hotplug and DP IRQ
  events; there is no DRM vblank interrupt of its own (`nvkms-evo.c`, `nvkms-hw-flip.c`).
- Hotplug reaches the kernel as `POST_EVENT` from GSP-RM.
- The Windows KMD drives the same channels with its own flip model: notifier mode WRITE,
  only `LAST_DATA` enabled, and the core's `SET_GET_BLANKING_CTRL`, which NVKMS never uses.

## VirtIO Leandro

- `virtio_nvrm.ko` exposes the displayless class (`NVA083_GRID_DISPLAYLESS`) and generates
  an EDID; guest hrtimers supply vblank timing; fence wake-ups return as RM events on queue 1.
- Module parameters: `vdisplay`, `vdisplay_width/height`, `vdisplay_vblank_hz` (60 by
  default), NVKMS ceilings, a display reserve (the balloon).
- GNOME on Wayland with Sunshine `capture=kms` and NVENC passed a client-frame check on
  2026-09-19; two Shadow of the Tomb Raider guests at 720p completed concurrently on one card.
  The vblank configuration changes throttling (open question 27).
- [DISPLAY.md](../DISPLAY.md), [WAYLAND.md](../WAYLAND.md).

## VFIO Leandro

Step numbers and run names (C87, P41) refer to
[research: windows-findings](../research/windows-findings.md).

1. The device declares head, window and SOR counts and the connector table itself (decision
   D15): one DisplayPort SST connector per head, no MST, no HDCP, DSC offered; heads are
   synthetic and independent of the physical card. The display is on with `--display`.
2. EDID over AUX and a DPCD model come from the `dp-sink` crate behind an NV0073 AUX
   adapter; link training answers as a sink would; the modes are the EDID's (1080p60 in the
   gate), an over-budget mode pruned.
3. The display caps page (NVC573, BAR0 0x640000) serves the RTX 2070's captured caps. Without
   the TMO, ILUT and OLUT caps the Windows KMD loops and unloads (Code 43, P40, P41); with an
   incomplete page it accepted flips but programmed no window (steps 4, 5).
4. The engine latches flips on its own clock and lock at the blank's start, writes the flip
   notifiers and semaphores, and raises `LAST_DATA` and `AWAKEN` as interrupts
   ([display-flips-and-reserve](display-flips-and-reserve.md)).
5. Rendering runs on the physical GPU through the shadow GR path; the scanout surface is a
   guest allocation in the VRAM slice that the device can read (block-linear, de-tiled for
   a dump) and never a physical head.
6. Hotplug (`POST_EVENT`), a hardware cursor, DP unplug controls (`DP_SET_WAR_FLAGS`,
   `SPECIFIC_HDCP_CTRL`) are answered; nothing in the display path is forwarded to the host.

| Measure | Value | Record |
|---|---|---|
| display gate, 615 guest | 11 of 11 | [caraxes/display-615.md](../caraxes/display-615.md) |
| Weston GL, presented / callbacks | 60.00 Hz / 59.79 Hz | same |
| Mutter, presented / callbacks | 59.40 Hz / 58.62 Hz | same |
| trapped accesses per frame, Weston / Mutter | 29.25 / 138.92 | same |
| Windows present probe, windowed 960x540, during a stream | 59.9 fps, 0 failed presents; the MPO overlay latched 59.2 to 60.0 flips/s (C87) | step 20 |
| Windows desktop to Moonlight, Sunshine x264 | 54.8 FPS, host latency 11.9 ms (C87) | step 20 |
| Linux B desktop to Moonlight, Sunshine KMS capture and NVENC HEVC | 59.96 fps, host latency 5.9 ms mean (l3) | [codec matrix](../../caraxes/runs/codec-2026-09-30/matrix.md) |
| Heaven 4.0 DX11 1080p High, windowed | 155.9 and 153.3 fps (C118) | step 23 |
| SotTR 1080p, Linux B, display reserve on | 15.4 FPS average, native 91.7 | [sottr-invgate](../research/sottr-invgate-2026-09-29.md) |

- Open:
  - 3DMark's DX12 exclusive fullscreen fails (`dx12_swapchain.cpp:238`, step 23).
  - Channel kicks still take the device lock.
- The gate compares a SotTR screenshot with a known-good frame by colour statistics
  (`frame-check.sh`, [gate-2026-09-30](../../caraxes/runs/gate-2026-09-30/README.md)).
- Why Wayland: the compositor runs on KMS with a vblank and page-flip cadence the device
  controls, which v1's guest timers could not settle; X11 was a stepping stone.

## Capture and encoding

- The stream encodes every captured frame: x264 on the vCPUs, or NVENC from VRAM.
- VirtIO Leandro: the guest's NVENC objects are host objects; Sunshine used NVENC.
- VFIO Leandro: the guest's NVENC channel runs on a host mirror channel; Sunshine
  `encoder = nvenc` streams on Linux B and on Windows ([video-engines](video-engines.md)).
  Linux B at 60 fps: Sunshine took 23.7 % of a core of guest CPU with NVENC against 142.3 %
  with x264, and host latency 5.9 ms against 18.3 ms (l3,
  [codec matrix](../../caraxes/runs/codec-2026-09-30/matrix.md)).
- A device-side dump (`dump` in display-ctl) writes the scanout as PNG; a 1080p surface
  reads in 135 steps, 17 ms under the device lock (step 7).

## Depth

- [display-flips-and-reserve.md](display-flips-and-reserve.md): the notifier model, the
  latch, the vblank costs, modeset and FLR, the NVKMS reserve.
- [caraxes/display-ab.md](../caraxes/display-ab.md): connector, EDID, link training (D2, D3).
- [caraxes/display-cd.md](../caraxes/display-cd.md), [caraxes/display-e.md](../caraxes/display-e.md),
  [caraxes/display-615.md](../caraxes/display-615.md): modeset, flips, the GPU-rendered desktop,
  the per-frame trap tables.
- [caraxes/display-connector.md](../caraxes/display-connector.md), [caraxes/display-scaling.md](../caraxes/display-scaling.md),
  [caraxes/display-adapter.md](../caraxes/display-adapter.md): the studies behind D15.
- [caraxes/display-own-clock.md](../caraxes/display-own-clock.md): the engine's own lock.

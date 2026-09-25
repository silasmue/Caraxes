<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Display and KMS

A guest desktop needs a monitor the driver believes in, a vblank cadence, page flips that
complete, and a way to capture the result for streaming. Neither architecture scans out
physically. VirtIO Leandro uses NVIDIA's displayless class with guest timers; VFIO Leandro
emulates a DisplayPort monitor in the GSP role, so the stock `nvidia-modeset` and
`nvidia-drm` see ordinary hardware and a Wayland compositor runs on KMS.

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

## VirtIO Leandro

- `virtio_nvrm.ko` exposes the displayless class (`NVA083_GRID_DISPLAYLESS`) and generates
  an EDID; guest hrtimers supply vblank timing; fence wake-ups return as RM events on queue 1.
- Module parameters: `vdisplay`, `vdisplay_width/height`, `vdisplay_vblank_hz` (60 by
  default), NVKMS ceilings, a display reserve.
- GNOME on Wayland with Sunshine `capture=kms` and NVENC passed a client-frame check on
  2026-09-19; two Shadow of the Tomb Raider guests at 720p completed concurrently on one card.
  The vblank configuration changes throttling (open question 27).
- [DISPLAY.md](../DISPLAY.md), [WAYLAND.md](../WAYLAND.md).

## VFIO Leandro

1. The device declares head, window and SOR counts and the connector table itself (decision
   D15): one DisplayPort SST connector per head, no MST, no HDCP, DSC offered; heads are
   synthetic and independent of the physical card, up to the emulated display IP's own cap.
2. EDID over AUX and a DPCD model come from the `dp-sink` crate behind an NV0073 AUX
   adapter; link training answers as a sink would; the modes are the EDID's (1080p60 in the
   gate), an over-budget mode pruned.
3. Flips are paced by the device's own timer at the mode's rate: vblank, `AWAKEN` and
   `LAST_DATA` reach the guest as interrupts; the display FE interrupt status page holds the
   28 words nvkms reads and acks.
4. Rendering runs on the physical GPU through the shadow GR path: the guest's EGL, GL and
   Vulkan stack, Weston and GNOME on it; the scanout surface is a guest allocation in the VRAM
   slice that the device can read (block-linear, de-tiled for capture) and never a physical
   head.
5. Hotplug (`POST_EVENT`), a hardware cursor, DP unplug controls (`DP_SET_WAR_FLAGS`,
   `SPECIFIC_HDCP_CTRL`) are answered; nothing in the display path is forwarded to the host.

| Measure | 615 guest | Configuration |
|---|---|---|
| display gate | 11 of 11 with host execution | vm0-615 |
| Weston GL presented / callbacks | 60.00 Hz / 59.79 Hz (sd 1.98 ms) | interrupt and display pages, token pool, hybrid reactor |
| Mutter presented / callbacks | 59.40 Hz / 58.62 Hz (sd 7.70 ms) | same |
| trapped accesses per frame, Weston / Mutter | 29.25 / 138.92 | same; Mutter's remainder is BAR2 page-table churn (74 writes, 23 reads) |
| Sunshine kms + x264 to Moonlight | 25.85 FPS, 101 ms host latency, 2.09 guest cores | 610 guest, before the interrupt page |
| classic demos on Weston | vkcube 44.1 FPS, glmark2-wayland 1804, glxgears 57 to 60 | 610 guest |

- What is missing: guest frames encoded by NVENC through the mirror (see Encoding below); a
  Shadow of the Tomb Raider frame rate; the 12 display MODEL controls of category D; Mutter and Weston with
  the BAR2 memfd.
- Why Wayland: the compositor runs on KMS with a vblank and page-flip cadence the device
  controls, which v1's guest timers could not settle; X11 was a stepping stone.

## Encoding

- The stream encodes every captured frame:
  - x264 on the vCPUs cost 2.09 guest cores at 1080p (Sunshine `encoder = software`);
  - NVENC encodes from VRAM for almost no CPU.
- VirtIO Leandro: the guest's NVENC objects are host objects; Sunshine used NVENC.
- VFIO Leandro ([video-engines.md](video-engines.md)):
  - the session controls are forwarded;
  - the guest's NVENC channel is mirrored on the shadow VA space and bound to NVENC0 at its
    first doorbell;
  - the host's NVC4B7 object follows the guest's allocation once the guest's context page has
    left the shadow.
  - No guest frame has been encoded yet, and Sunshine stays on x264 until the probe passes.
- NVDEC (video playback in the desktop) takes the same path with NVC4B0.

## Depth

- [caraxes/display-ab.md](../caraxes/display-ab.md): connector, EDID, link training (D2, D3).
- [caraxes/display-cd.md](../caraxes/display-cd.md), [caraxes/display-e.md](../caraxes/display-e.md),
  [caraxes/display-615.md](../caraxes/display-615.md): modeset, flips, the GPU-rendered desktop,
  the per-frame trap tables.
- [caraxes/display-connector.md](../caraxes/display-connector.md), [caraxes/display-scaling.md](../caraxes/display-scaling.md),
  [caraxes/display-adapter.md](../caraxes/display-adapter.md): the studies behind D15.

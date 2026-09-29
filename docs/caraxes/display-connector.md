<!-- SPDX-License-Identifier: MIT -->
# One emulated DVI-D connector: a display through the GSP role, paced by the device

Written 2026-09-23 (source survey, no VM run, no host GPU access). It answers the
operator's question of 17:20: *"explore what it means to emulate a display through the GSP,
for example something simple like DVI-D, because HDMI/DP are very complex protocols; with
hw/sw timers we can emulate the refresh rate and overcome the Wayland refresh-rate issue of
Leandro v1"*, with the operator's follow-up direction: **Wayland is the goal, X is at most a
stepping stone.** The target is a Wayland compositor (Weston or a wlroots compositor such as
sway first, GNOME/Mutter as the desktop case) on the guest's stock nvidia-drm KMS.

It builds on [display-path.md](display-path.md) (the classes, RPCs, register pages and route
E) and does not repeat it. Pointers: `OGK/` is `vendor/open-gpu-kernel-modules` at 610.57.04;
`NVKMS/` is `OGK/src/nvidia-modeset/src`; `DRM/` is `OGK/kernel-open/nvidia-drm`; `KDISP/` is
`OGK/src/nvidia/src/kernel/gpu/disp`; nouveau is `linux-6.12/drivers/gpu/drm/nouveau` in the
session scratchpad. Tags: [V] read in source, [M] measured by v1 (journal or v1 history),
[I] inferred, [K] compositor behaviour from general knowledge of Weston/wlroots/Mutter, not
read in a local source tree (no compositor source is checked out here; the host has only
Hyprland/aquamarine binaries).

**Short answer.**
- **DVI-D single-link TMDS is the right connector.** In GSP-client mode it costs one SOR
  reporting protocol `SINGLE_TMDS_A`, a connector-data entry of type `DVI_D`, DFP flags
  without `HDMI_CAPABLE`, and a 128-byte EDID with no CEA extension. NVKMS then never sends a
  DP, HDMI, SCDC, infoframe or link-training control, and caps the pixel clock at 165 MHz,
  which 1920x1080@60 (148.5 MHz) fits (section 1).
- **The refresh cadence has to be carried by page-flip completion, not by a vblank
  counter.** On Turing with the 610.57.04 nvidia-drm, the DRM vblank counter never runs
  (its interrupt source is the RG semaphore, which the Turing HAL lacks). Compositors pace on
  the flip-complete events. Those come from the window channel's AWAKEN interrupt. A device
  timer that completes each flip at its next tick, and raises the head's `LAST_DATA` vblank
  interrupt on the same tick, gives compositors and the NV9010 consumers one stable cadence
  (section 2).
- **Why v1 ran too fast under Wayland:** its GRID displayless HAL completes a flip as soon
  as the flip's acquire semaphore is satisfied: a 0 µs timer, then 100 µs polling. It ignores
  `minPresentInterval` and every vblank. Its v1 `NV9010` hrtimer never fed the flip path. The
  same HAL has no output-LUT capability, so Mutter's gamma-LUT commits failed about 1000 times
  per second while a client drew. The device-timer design removes all three causes
  (section 2.4).
- **No host-side scanout is needed for the gates.** Every readback path (dumb-buffer
  readback, KMS capture, compositor screencopy) reads guest-visible memory. De-tiling
  block-linear surfaces for a later host sink is cheap from host RAM, but it is infeasible by
  CPU from real VRAM (section 3).
- **Effort: 9-14 agent-days** for phases A-D, against 15-25 for the full route E. Add 2-4 for
  an optional host sink. The first guest-visible milestone is a DRM connector with EDID and
  modes (phase B, no modeset). The first Wayland milestone is a compositor whose frame
  callbacks run at the mode's rate (phase C) (section 4).

---

## 1. "One DVI-D single-link TMDS connector, one head" in GSP-client terms

### 1.1 The model the device presents

| Object | Value | Why |
|---|---|---|
| Heads | 1 (head 0) | `INTERNAL_DISPLAY_GET_STATIC_INFO.numHeads`, `NV0073 SYSTEM_GET_NUM_HEADS`, `SPECIFIC_GET_ALL_HEAD_MASK` = 0x1 must agree |
| Windows | 2 (windows 0, 1 on head 0) | `windowPresentMask` = 0x3; `GET_VALID_HEAD_WINDOW_ASSIGNMENT`; the caps page must give `numWindows` > 0 (`NVKMS/nvkms-evo.c:5275`) |
| SOR | 1 (SOR 0), crossbar reported like the real Turing | `SPECIFIC_OR_GET_INFO`: `type` = SOR (2), `index` 0, `protocol` = `SINGLE_TMDS_A` (1), `location` CHIP, `bIsDispDynamic` = FALSE, `bIsLitByVbios` = FALSE (`ctrl0073specific.h:2039-2071`) |
| displayId | one bit, for example 0x100 | `SYSTEM_GET_SUPPORTED.displayMask`, `GET_CONNECT_STATE.displayMask` |
| Connector | `DVI_D` (0x31), index 0 | `SPECIFIC_GET_CONNECTOR_DATA`: `count` 1, `data[0].type` 0x31, `DDCPartners` 0 (`ctrl0073specific.h:392,432`) |
| DFP flags | `SIGNAL_TMDS`, `LINK_SINGLE`, `HDMI_CAPABLE_FALSE`, `HDMI_ALLOWED_FALSE` | `DFP_GET_INFO` (`ctrl0073dfp.h:132-186`) |
| Display type | DFP | `SPECIFIC_GET_TYPE` (`ctrl0073specific.h:66,79`) |
| Pixel clock limit | 165000 kHz | `SPECIFIC_GET_PCLK_LIMIT`; NVKMS overrides it with 165 MHz for non-HDMI TMDS anyway |

How NVKMS turns this into a connector [V]:
- `ProbeValidDisplays` takes `SYSTEM_GET_SUPPORTED` and keeps every displayId whose
  `OR_GET_INFO` says not dynamic (`NVKMS/nvkms-rm.c:426-486`).
- `AllocConnector` computes `isDP` from the OR protocol. For `SINGLE_TMDS_A` it is false, so
  no DP-library connector is created and no `DP_*` control is ever sent. The type comes from
  `GET_CONNECTOR_DATA` through the table at `nvkms-rm.c:690-750` (`DVI_D` →
  `NVKMS_CONNECTOR_TYPE_DVI_D`). The name becomes `DVI-D-0`; nvidia-drm exposes it as
  `card0-DVI-D-1`, which is also the name v1's displayless connector had
  (journal `claude/leandro-test-report.md:438`).
- `GetSignalFormat` maps SOR plus `SINGLE_TMDS_A` to `SIGNAL_FORMAT_TMDS` (`nvkms-rm.c:507-563`).
- `GetDfpInfo` reads the DFP flags (`nvkms-rm.c:565-597`). `DpyGetStaticDfpProperties` sets
  `hdmiCapable` only from `HDMI_CAPABLE` (`NVKMS/nvkms-dpy.c:996-1010`).
- `nvDpyIsHdmiEvo` needs both GPU support and an HDMI VSDB in a CEA-861 extension
  (`NVKMS/nvkms-hdmi.c:215-257`). With the flag clear and no extension block, the whole HDMI
  path is dead: infoframes, SCDC, FRL, `SET_HDMI_*`.
- `nvDpyProbeMaxPixelClock` asks `GET_PCLK_LIMIT`. For non-HDMI TMDS it then sets
  165 MHz, or 330 MHz if both the SOR caps (`dualTMDS`) and the DFP `LINK_DUAL` flag allow
  it (`nvkms-dpy.c:57,809-958`).

What is **absent** for a DVI-D connector, and why:
- **No DP AUX, no link training.** `isDP` is false (above). TMDS has no training. The SOR's
  power-up happens inside the supervisor sequence, which GSP owns and the device does not
  model (section 1.4).
- **No HDCP.** The open NVKMS sends no HDCP control for a TMDS DFP [V, by the absence of any
  HDCP control in the control census below].
- **No HDMI.** See above.
- **Audio.** One piece remains. NVKMS still sends `DFP_SET_ELD_AUDIO_CAPS` around every modeset,
  with the ELD empty, so `PD`/`ELDV` are 0. Before that it sends
  `NV2080_CTRL_CMD_OS_UNIX_AUDIO_DYNAMIC_POWER` (`nvkms-hdmi.c:1079-1160`). The device
  answers `NV_OK` and does nothing.

### 1.2 The EDID

- **Delivery.** It is not I2C. `SPECIFIC_GET_EDID_V2` (0x730245) is `PRIVILEGED |
  ROUTE_TO_PHYSICAL` (flags 0x44 in `OGK/src/nvidia/generated/g_disp_objs_nvoc.c`), so it
  reaches the device as a `GSP_RM_CONTROL`. The device returns `bufferSize` and up to 2048
  bytes (`ctrl0073specific.h:145-156`). NVKMS reads it in `ReadEdidFromResman`
  (`nvkms-dpy.c:1289-1360`). nouveau's r535 client does the same for a TMDS output
  (`nvkm/engine/disp/r535.c:1154-1184`).
- **Init-time EDID.** `INTERNAL_SET_STATIC_EDID_DATA` at init is kernel → GSP and is only
  acknowledged. `INTERNAL_DISPLAY_GET_STATIC_INFO.i2cPort` = 16 (`NV402C_CTRL_NUM_I2C_PORTS`)
  means "no external device" (`KDISP/kern_disp.c:611-623`). No I2C object is needed.
- **Content.** An EDID 1.4 base block, 128 bytes, no extension (byte 126 = 0).
  - Byte 20 = 0xA1: digital, 8 bpc, **interface DVI (1)**. v1's generator writes 5,
    DisplayPort (`guest-module/virtio_nvrm/nvrm_edid.c:212`); the Rust port must change that
    one field.
  - DTD 1: 1920x1080@60, CEA-861 timing (148.5 MHz, 2200x1125 total, exactly 60.000 Hz).
  - DTD 2: 1280x720@60 (74.25 MHz, 1650x750, 60.000 Hz).
  - A third DTD, 1920x1080@50 (148.5 MHz, 2640x1125), proves in phase D that the device
    timer follows the mode.
  - Range limits: 49-76 Hz vertical, max pixel clock 170 MHz. Monitor name descriptor.
- **Reuse.** The layout rules of `nvrm_edid.c` (DTD field clamps, range limits containing
  every mode, checksum) carry over. v1's gate tool `tests/tools/edid-verify.c` and
  `edid-decode --check` stay the conformance checks.
- **Deliberately not in the EDID:**
  - no CEA-861 extension, so no HDMI VSDB, no audio data block and no YCbCr claims;
  - no mode above 165 MHz while the connector is single-link. A CVT-RB 1920x1080@75
    (about 174 MHz) is a useful **negative** test: NVKMS must prune it.

### 1.3 The RPC and control set, by phase

Every display control NVKMS sends is `ROUTE_TO_PHYSICAL` (flag 0x40). The kernel caches some
of them (`GET_SUPPORTED`, `GET_TYPE` are `PERSISTENT_CACHEABLE`, 0x800000). The census below
is `grep` over `NVKMS/*.c` plus `KDISP/` [V]. It is the display-path.md §2.2 list, cut down
to what one DVI-D connector touches.

| When | RPC / control | Device answer |
|---|---|---|
| kernel init (PreInit/Init/Load) | `INTERNAL_DISPLAY_GET_IP_VERSION` 0x20800a4b | 0x04000000 |
| | `INTERNAL_DISPLAY_GET_STATIC_INFO` 0x20800a01 | `numHeads` 1, `windowPresentMask` 0x3, `feHwSysCap` from the real card if captured, else synthesized, `numDispChannels` ≥ 81, `i2cPort` 16, masks 0 (`ctrl2080internal.h`, struct `..._GET_STATIC_INFO_PARAMS`) |
| | `INTERNAL_INIT_BRIGHTC_STATE_LOAD`, `INTERNAL_SET_STATIC_EDID_DATA`, `INTERNAL_DISPLAY_WRITE_INST_MEM`, `INTERNAL_DISPLAY_SET_IMP_INIT_INFO`, `NV0073 SYSTEM_MAP_SHARED_DATA`, `INTERNAL_DISPLAY_PRE/POST_UNIX_CONSOLE` | `NV_OK`; record the instance-memory base and size |
| | `GSP_RM_ALLOC NV04_DISPLAY_COMMON` | `NV_OK` |
| NVKMS device alloc (`nvRmAllocDisplays`, `nvAllocCoreChannelEvo`) | `NV0073 SYSTEM_GET_CAPS_V2` | caps bits, with `CROSS_BAR_SUPPORTED` as on real Turing |
| | `SYSTEM_GET_NUM_HEADS`, `SPECIFIC_GET_ALL_HEAD_MASK`, `SPECIFIC_GET_VALID_HEAD_WINDOW_ASSIGNMENT` | 1, 0x1, windows 0/1 → head 0 |
| | `SYSTEM_GET_SUPPORTED`, `SPECIFIC_OR_GET_INFO`, `SPECIFIC_GET_CONNECTOR_DATA`, `SPECIFIC_GET_TYPE`, `DFP_GET_INFO` | table 1.1 |
| | `SYSTEM_GET_BOOT_DISPLAYS`, `SYSTEM_GET_ACTIVE`, `SYSTEM_GET_INTERNAL_DISPLAYS` | 0 (nothing lit by a VBIOS) |
| | `GSP_RM_ALLOC NVC570_DISPLAY`, `NV5070 SYSTEM_GET_CAPS_V2` | `NV_OK`, caps |
| | `INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER` + `GSP_RM_ALLOC NVC57D` (core), `NVC57E`/`NVC57B` (windows 0-1), `NVC57A` (cursor) | record hclass, instance, pushbuffer address, aperture, limit |
| | `GSP_RM_ALLOC NVC573` caps object + the guest's map of BAR0 0x640000 | caps page: 1 head, 2 windows, SOR 0 with TMDS, `maxTMDSClkKHz` 340000, `dualTMDS` 0 |
| | `NVC370 GET_CHANNEL_INFO` | `STATE_IDLE` when GET == PUT (`NVKMS/nvkms-evo3.c:6458-6490`) |
| | `NVC370 EVENT_SET_NOTIFICATION` (`RG_SEM_NOTIFICATION`) | `NV_OK`. The event never fires on Turing (section 2.1) |
| | `NV2080 EVENT_SET_NOTIFICATION` (`HOTPLUG`) | `NV_OK` (`nvkms-rm.c:1929-1935`) |
| connector probe (nvidia-drm hotplug processing) | `SYSTEM_GET_CONNECT_STATE` | the displayId bit (`nvkms-rm.c:2422-2473`) |
| | `SPECIFIC_GET_EDID_V2` | the EDID |
| | `SPECIFIC_GET_PCLK_LIMIT` | 165000 |
| | `NVC372 IS_MODE_POSSIBLE` (IMP, mode validation and modeset) | possible, with plausible bandwidth numbers |
| modeset | `DFP_ASSIGN_SOR` | SOR 0 for the displayId |
| | `SPECIFIC_DISPLAY_CHANGE` (`nvRmBeginEndModeset`, `nvkms-rm.c:2220`), `SYSTEM_ALLOCATE_DISPLAY_BANDWIDTH` | `NV_OK` |
| | `DFP_SET_ELD_AUDIO_CAPS`, `NV2080 OS_UNIX_AUDIO_DYNAMIC_POWER` | `NV_OK` |
| runtime | `SYSTEM_GET_SCANLINE`, `NV5070 GET_RG_STATUS` | from the device's raster model |
| EDID change | `POST_EVENT` RPC `NV2080_NOTIFIERS_HOTPLUG`, plug mask = displayId (`kernel_gsp.c:505-570`) | device-initiated |

nouveau's r535 client uses the same NV0073 core: `GET_CONNECTOR_DATA`, `OR_GET_INFO`,
`DFP_GET_INFO`, `GET_CONNECT_STATE`, `GET_EDID_V2`, `DFP_ASSIGN_SOR`, `GET_ACTIVE`
(`r535.c:639-890,1154-1206`). That is an independent confirmation of the minimum [V].

### 1.4 The OR/head assignment, and what NVKMS does from there

At modeset NVKMS writes the core channel [V]:
- `SOR_SET_CONTROL(0)`: owner mask = head 0, protocol `SINGLE_TMDS_A`;
- `HEAD_SET_CONTROL(0)`, `HEAD_SET_PIXEL_CLOCK_FREQUENCY(0)`, `HEAD_SET_RASTER_SIZE(0)` plus
  sync and blank, `WINDOW_SET_CONTROL(w)` owner = head 0, the OLUT methods;
- then `UPDATE` (`clc57d.h:80,299,333,539,662,793,998`).

The device keeps from this only what it needs:
- the head's raster total and pixel clock. They give the tick period:
  `period = htotal * vtotal / pclk`, 2200·1125/148.5 MHz = 16.667 ms;
- which windows the head owns;
- whether the head is active.

The sequence NVKMS runs, in `nvkms-modeset.c` and `nvkms-evo.c` [V]:
1. It validates the mode: EDID timings, pixel clock limit, IMP through `IS_MODE_POSSIBLE`.
2. It assigns a SOR (`DFP_ASSIGN_SOR`).
3. It registers the flip-occurred event, an `NV01_EVENT_KERNEL_CALLBACK_EX` on each window
   channel, notify index 0 (`nvEvoPreModesetRegisterFlipOccurredEvent`,
   `nvkms-evo.c:5006-5050`).
4. It pushes the core methods and kicks off `UPDATE`, then waits for the core notifier to
   read FINISHED (`nvEvoWaitForCompNotifierC3`, `nvkms-evo3.c:6261-6288`).
5. It re-enables audio (ELD) and the LUT.

**Supervisor interrupts.** In GSP-client mode on a dGPU the three-stage supervisor sequence
is serviced entirely inside GSP. The kernel has no handler. The modeset start/end
notification is consumed only by the Tegra DCE client
(`kdispInvokeDisplayModesetCallback_KERNEL`, `KDISP/kern_disp.c:1950-2000`; display-path.md §2.5). So the device does not raise
supervisor interrupts. On `UPDATE` it applies the state and does four things:
- sets `FE_CORE_HEAD_STATE(0).OPERATING_MODE` to active (0x612078, `dev_disp.h` v04 line 31).
  The kernel reads it in `kheadGetDisplayInitialized_KERNEL` to decide whether vblank is
  available (`KDISP/head/arch/v04/kernel_head_0400.c:26-40`, `KDISP/head/kernel_head.c:239-296`);
- writes the core notifier FINISHED;
- raises the core AWAKEN if the notifier mode asked for it;
- moves GET to PUT.

**Channel state.** The `NVC370 GET_CHANNEL_INFO` answer must say IDLE when no method is
pending: NVKMS uses it for "idle" and "method pending" (`nvkms-evo3.c:6458-6490`).

**Flips** use the window channel [V: `nvkms-evo3.c:3670-3748`, `clc57e.h`]:
- `SET_NOTIFIER_CONTROL` with mode `WRITE_AWAKEN`. nvidia-drm's KAPI always asks for awaken
  (`OGK/src/nvidia-modeset/kapi/src/nvkms-kapi.c:2988,3188`);
- `SET_PRESENT_CONTROL`: `MIN_PRESENT_INTERVAL`, `BEGIN_MODE` `NON_TEARING` or `IMMEDIATE`,
  `TIMESTAMP_MODE`;
- `SET_SEMAPHORE_CONTROL`, `SET_SEMAPHORE_ACQUIRE`, `SET_SEMAPHORE_RELEASE`;
- `SET_CONTEXT_DMA_ISO`, `SET_OFFSET`, `SET_UPDATE_TIMESTAMP_LO/HI`;
- `UPDATE`.

The device resolves the context-DMA handles through RAMHT in the display instance memory
(display-path.md §2.4). Section 2.2 gives the completion rule.

## 2. The refresh-rate mechanism

### 2.1 How vblank and flip completion reach the guest on Turing in GSP-client mode

Two interrupt sources, one vector, both serviced by the **guest kernel RM**, not by GSP:
- `intrCacheDispIntrVectors_TU102` takes `displayIntrVector` from the kernel interrupt
  table when not running on GSP (`OGK/src/nvidia/src/kernel/gpu/intr/arch/turing/intr_tu102.c:277-291`).
- `kdispRegisterIntrService` registers `MC_ENGINE_IDX_DISP` (`KDISP/kern_disp.c:1895-1911`).
- `kdispServiceInterrupt_KERNEL` runs the low-latency service (`kern_disp.c:1918-1945`).

nouveau's r535 does the same: it takes the DISP vector from the GSP kernel interrupt table
(`nvkm/subdev/gsp/r535.c:883-890`) and services head timing itself (`r535.c:1383-1410`).

| Source | Registers the device sets (BAR0) | Guest path |
|---|---|---|
| **vblank** (`LAST_DATA`) | `FE_EVT_STAT_HEAD_TIMING(h)` 0x611800+4h bit 1; `FE_RM_INTR_STAT_HEAD_TIMING(h)` 0x611C00+4h bit 1; `FE_RM_INTR_DISPATCH` 0x611EC0 bit h. Only if `FE_RM_INTR_EN_HEAD_TIMING(h)` 0x611D80+4h bit 1 is set (`dev_disp.h` v03 lines 217-264, v04 lines 36-40) | `kheadReadPendingVblank_v03_00` → per-head counter `kheadSetVblankTotalCounter` → `kheadProcessVblankCallbacks` → each `NV9010_VBLANK_CALLBACK`'s `pProc` (`KDISP/arch/v03/kern_disp_0300.c:491-527`, `kern_disp.c:1700-1760`, `KDISP/vblank_callback/vblank_callback.c:34-100`); the guest clears the bit by writing 1 to 0x611800+4h |
| **flip completion** (AWAKEN) | `FE_EVT_STAT_AWAKEN_WIN` 0x611858 bit w (or `_AWAKEN_OTHER` 0x61185C bit 0 for core); `FE_RM_INTR_STAT_CTRL_DISP` 0x611C30 bit 8 | `kdispServiceAwakenIntr_v03_00` → `_kdispHandleAwakenChnMask` → `notifyEvents` on the window channel (`kern_disp_0300.c:643-760`) → NVKMS `CompletionNotifierEvent` → `nvSendFlipOccurredEventEvo` (`NVKMS/nvkms-evo.c:4991-5003`) → nvidia-drm `__nv_drm_handle_flip_event` → `DRM_EVENT_FLIP_COMPLETE` (`DRM/nvidia-drm-modeset.c:642-690`) |
| counters the guest reads | `RG_DPCA(h)` 0x616330+0x800h: line and frame count (`kern_disp_0300.c:297-318`); `POSTCOMP_HEAD_LOADV_COUNTER(h)` 0x61611C+0x800h (`:403`, served by `NV0073 SYSTEM_GET_LOADV_COUNTER_INFO`) | scanline queries, RC watchdog |
| CPU interrupt tree | the DISP vector's leaf bit, then MSI | the vector must be in leaf 4 (128-159): the low-latency path reads leaf 4 only (`intr_tu102.c:1174-1212`) |

What each consumer uses:
- **NV9010 is still the vblank callback class in the GSP era.** It is kernel-local, not an
  RPC: `RS_FLAGS_ALLOC_KERNEL_PRIVILEGED` only (`OGK/src/nvidia/src/kernel/rmapi/resource_list.h:1202-1211`).
  NVKMS allocates it when anything registers on a head's `vblankCallbackList`
  (`nvRmAddVBlankCallback`, `NVKMS/nvkms-rm.c:5141-5175`). Those consumers are the
  vblank-semaphore control, headSurface, and the `NotifyVblank` ioctl that the X driver and
  GLX swap intervals use. They are fed by the `LAST_DATA` interrupt [V]. Unlike v1, Caraxes
  does not serve NV9010 itself: the guest's own RM does, driven by the device's interrupt.
- **`NV2080_NOTIFIERS_VBLANK` does not exist in 610.57.04.** `cl2080_notification.h` has
  no vblank notifier; `NV2080_NOTIFIERS_HOTPLUG` (1) is the one display notifier NVKMS
  registers [V].
- **DRM vblank does not run on Turing.**
  - nvidia-drm's `vblank` parameter defaults to **0** (`DRM/nvidia-drm-os-interface.c:47`).
  - With `vblank=1` it registers an RG-interrupt callback (`DRM/nvidia-drm-crtc.c:3151-3175`).
    That callback needs `hal->SetupVBlankRgSemaphoreInterrupt`, which is NULL in the Turing
    HAL `nvEvoC5` (`NVKMS/nvkms-evo3.c:7806`; Turing uses C5, `NVKMS/nvkms-hal.c:157`). It is
    set only from C6 (Ampere) on. So registration fails with "RG semaphore vblank interrupt
    not supported" (`NVKMS/nvkms-modeset.c:4466-4513`).
  - The DRM vblank counter therefore never advances on a real RTX 2070 either. Flip events
    then carry a millisecond clock (`nvidia-drm-modeset.c:589-633`).
  - With the default `vblank=0`, `drm_vblank_init` is skipped on current kernels
    (`DRM/nvidia-drm-drv.c:865-876`). `DRM_IOCTL_WAIT_VBLANK` is unsupported. Flip events
    carry the kernel's `ktime_get()` at event time and sequence 0 (the kernel's
    `drm_crtc_send_vblank_event` without vblank support) [V for nvidia-drm, K for the DRM
    core].
  - **Consequence: on this card and driver, a compositor's only timing source is
    flip-complete events.** The device must make them tick-aligned. That is the whole
    refresh-rate mechanism for Wayland. Recommendation: run the guest with nvidia-drm
    `modeset=1` and the default `vblank=0`, not v1's `vblank=1`. v1 already measured that
    `vblank=0` throttles better (v1 history OQ 27).

### 2.2 The device timer

One periodic timer per active head.
- **Clock.** A `timerfd` on the host's `CLOCK_MONOTONIC` with absolute deadlines
  (`TFD_TIMER_ABSTIME`): `t_n = t_0 + n·period`. Deadlines are never re-armed relative to
  "now", so the phase does not drift and a late wakeup does not stretch the next period.
- **Period.** Computed from the raster the core `UPDATE` programmed (section 1.4). It is
  re-armed on every modeset. It stops when the head is disabled or when DPMS off arrives
  (`SPECIFIC_SET_MONITOR_POWER`).
- **On each tick, in this order:**
  1. **Latch pending window flips.** A window `UPDATE` is eligible when three things hold:
     its acquire semaphore (if any) holds the acquire value, read from guest memory through
     the context DMA; at least `MIN_PRESENT_INTERVAL` ticks have passed since that window's
     last latch; and any `SET_UPDATE_TIMESTAMP` time has arrived. An eligible flip latches:
     - the device releases the **previous** flip's release semaphore (hardware releases on
       flip-away; the displayless HAL copies this rule, `nvkms-displayless.c:262-276`);
     - it writes the window notifier: the device's PTIMER value in words 2-3
       (`NV_DISP_NOTIFIER__2/3_TIMESTAMP`, `clc67d.h:45-54`), status BEGUN in word 0,
       and FINISHED at the next tick, when the first frame from the new surface has been
       scanned out. The class lists the states without timing; both drivers fix it: NVKMS
       tests the newest flip for BEGUN at the following vblank (`nvkms-headsurface.c:
       1896-1922`), the Windows KMD waits for FINISHED after a modeset's single flip
       before it unblanks (windows-findings.md, C49-C54). A flip on an inactive head is
       FINISHED at once. nvidia-drm reads neither (it waits for AWAKEN);
     - it sets the window's `AWAKEN_WIN` bit if the notifier mode is `WRITE_AWAKEN`.
  2. **Advance the raster.** Increment the `RG_DPCA` frame count and the LOADV counter.
  3. **Raise vblank.** Set `LAST_DATA` if the RM enabled it.
  4. **Signal.** Set the leaf bit and send one MSI if any bit above went pending.
- **Immediate flips** (`BEGIN_MODE_IMMEDIATE`, the DRM async page flip / tearing-control
  path) latch as soon as their acquire is satisfied, without waiting for a tick.
- **PTIMER.** It must be the same clock the notifier timestamps use.
  `NV2080_CTRL_CMD_TIMER_GET_TIME` (`nvkms-rm.c:4914-4933`) and the notifier words must
  agree, so the device serves PTIMER from the same `CLOCK_MONOTONIC` base: one device
  clock per process (`channel::device_epoch`), kept across a function-level reset.
- **Latency.** The chain is: device thread → vfio-user `SET_IRQS` eventfd → Cloud
  Hypervisor/KVM irqfd → guest MSI → RM ISR → NVKMS deferred timer work
  (`nvkms_alloc_timer_with_ref_ptr(..., 0)`) → nvidia-drm event. Tens to a few hundred µs
  [I]. That is constant enough for every compositor below. It is the one number phase C must
  measure.

"hw/sw timers" in the operator's question map onto this: the only timer that needs to exist
is the device's host timer. The guest's own PTIMER reads and the guest kernel timestamps
follow it. No guest-side hrtimer is involved, unlike v1.

### 2.3 What the compositors need, and how the tick has to behave for them [K unless marked]

Common requirements (Weston, sway/wlroots, Mutter on nvidia-drm):
- **KMS.** nvidia-drm with `modeset=1`, atomic KMS (all three prefer atomic), a connected
  connector with an EDID and a mode list, one CRTC, and a primary and cursor plane.
- **Buffers.** GBM from NVIDIA's userspace (`nvidia-drm_gbm.so` from `libnvidia-egl-gbm` /
  `libnvidia-extra`, measured necessary in v1: journal `claude/desktop-handoff.md:59`).
  No EGLStreams: Weston and wlroots dropped them; Mutter's EGLStream path is legacy [K].
- **Output LUT.** The atomic `GAMMA_LUT`, used by Mutter/colord, must validate. On a C57x
  head it does: the C5 HAL always reports OLUT support (`nvkms-evo3.c:5583-5589`). The device
  accepts `HEAD_SET_OLUT_*` and the LUT context DMA and ignores the contents while it has no
  sink.
- **Pacing.** Page-flip completion is the pacing source. Clients' FIFO presentation
  (Vulkan WSI, EGL swap interval 1) throttles on `wl_surface.frame` callbacks, which the
  compositor sends once per repaint. So **client frame rate = compositor repaint rate = the
  flip-completion rate**.

Per compositor:

| Compositor | What drives the next frame | Sensitivity |
|---|---|---|
| **Weston** (DRM backend) | `weston_output_finish_frame` takes the flip event's timestamp and schedules the next repaint at `timestamp + refresh − repaint-window` (default window 7 ms). Starting from idle, `drm_output_start_repaint_loop` first tries `drmWaitVBlank` (relative, sequence 0) for the last vblank time. On failure, or a stale time, it flips the current buffer to get a fresh timestamp | With `vblank=0`, `drmWaitVBlank` fails, so Weston always takes the flip path. That works if flips complete at ticks. Weston tolerates jitter up to the repaint window. Timestamps in the past, or zero, are clamped with a warning |
| **wlroots / sway** | The DRM backend's page-flip handler emits `wlr_output` `present` (timestamp, `refresh` from the mode, flags VSYNC, HW_CLOCK and HW_COMPLETION) and then `frame`. sway renders on `frame`, optionally delayed by `max_render_time` | Most direct: sway repaints exactly once per completion. A too-early completion means a too-fast compositor. A lost completion freezes the output: there is no timer fallback while a flip is pending |
| **Mutter** (native, `meta-kms`) | The `ClutterFrameClock` schedules the next dispatch at `last_presentation + refresh_interval − max_render_time`, with a dynamic render-time estimate (and triple buffering in recent versions). A failed commit is treated as not presented, and the clock proceeds at once | Tolerates jitter. A **failed** commit loop makes it run as fast as its clients (this is what v1 hit, section 2.4). A missing or zero presentation time falls back to "now" |

How the tick must behave:
- **Periodic, in CLOCK_MONOTONIC terms.** Constant latency is harmless: the compositor's
  schedule just shifts phase. Jitter up to a few hundred µs is harmless against 7 ms (Weston)
  or the render-time margins (Mutter).
- **A late tick** of more than one repaint window makes the compositor's next commit miss:
  one dropped frame, and the next completion at `t + 2·period`. Compositors handle this. It
  must stay rare.
- **An early tick,** or completing a flip before the tick, shortens or removes the render
  budget. At the limit it removes throttling altogether. That is the v1 failure.
- **Never lose a completion.** Every eligible window `UPDATE` completes at exactly one tick.
  A flip whose acquire semaphore never arrives stays pending, and the compositor waits
  on it, as with real hardware.
- **Timestamps.** The compositor sees the guest's `ktime_get()` when nvidia-drm sends the
  event, and sequence 0 (section 2.1). That matches the real RTX 2070 with this driver, so
  the compositors' NVIDIA-on-Turing behaviour applies unchanged [I]. `wp_presentation`
  feedback to clients reports that time and the mode's refresh.

**X (optional check).** The NVIDIA X driver talks to NVKMS directly, not through DRM. Its
swap-interval pacing uses `NotifyVblank`/NV9010, so it exercises the `LAST_DATA` half of the
tick. A Wayland compositor exercises the AWAKEN half. Xwayland clients are paced by their
Wayland compositor.

### 2.4 Why v1's Wayland refresh broke, and whether the device timer fixes it

What v1 measured [M]:
- `vkcube` under FIFO ran at **58.1 FPS on Xorg, but 1520 on the Wayland path** (v1 history
  `docs/history/OPEN-QUESTIONS-2026-09-18.md` issue 24).
- `vkcube-wayland` under MangoHud averaged **1367 FPS** (journal
  `claude/leandro-test-report.md:1587-1591`).
- X11 `vkcube` on the compositor's Xwayland ran at **590 FPS while about 64 reached the
  screen** (issue 41).
- `vblank=0` throttled better than `vblank=1` (issue 27).
- The host-side waiter limiter `LEA_FRL_HZ` was the workaround
  (`crates/vhost-user-nvrm/src/waiters.rs:21-40`). It had to stay opt-in: with it on, the guest
  hit a double free in `nv_drm_free` (issue 38).
- The operator saw the gears standing still while `glxgears` counted 58 FPS (issue 30).

The mechanism, from the source and the journal:
1. **Flip completion was not tied to any refresh** [V]. v1's display was NVIDIA's GRID
   displayless HAL. Its `DisplaylessEnqueueFlip` arms a timer with timeout 0, and
   `ProcessPendingFlips` completes each queued flip as soon as its acquire semaphore holds;
   otherwise it polls every 100 µs (`NVKMS/nvkms-displayless.c:241-381`,
   `DISPLAYLESS_POLL_INTERVAL_USEC 100`). `DisplaylessFlip` passes only the semaphores and
   drops `minPresentInterval` and the begin mode (`:383-411`). A compositor that repaints on
   completion (wlroots) therefore repainted as fast as rendering finished, and so did the
   clients throttled on its frame callbacks.
2. **v1's vblank never reached the flip path** [V, M]. The NV9010 hrtimer v1's guest module
   ran at the EDID rate (`stat_vblank_fired` 60/s) feeds the RM vblank-callback list only. The
   displayless HAL does not use it (issue 18: "the displayless HAL polls at 100 us and does
   not use the raster-generator callback"). nvidia-drm's `vblank=1` hook needs RG semaphores,
   which the displayless HAL does not have either (`nvkms-displayless.c:855`, NULL). So DRM
   saw neither vblanks nor paced flips.
3. **Mutter's commits failed in a loop** [M, V]. The displayless HAL reports no output LUT
   (`DisplaylessGetCapabilities`, `nvkms-displayless.c:149-191`). Mutter 46 puts colord's
   linear ramp into the first modeset as `GAMMA_LUT`. Every later flip inherits the LUT
   surface, and `ValidateHeadLutHwState` rejects it (`NVKMS/nvkms-hw-flip.c:1525-1534`). The
   journal measured about 1000 `Page flip failed` per second as soon as `vkcube` drew, one per
   client frame (`claude/leandro-test-report.md:465-540`; workaround: `colord` masked, or
   `MUTTER_DEBUG_FORCE_KMS_MODE=simple`). A failed commit releases Mutter's frame clock at once
   [K], so GNOME ran at the client's speed.
4. Xorg looked right (58.1 FPS) because the NVIDIA X driver's swap pacing uses NV9010
   `NotifyVblank`, which v1 did serve at 60 Hz [I from the source split above; not measured
   separately in v1].

Does the device-timer design fix it?

| v1 cause | With one emulated C57x head and the device timer |
|---|---|
| Flips complete immediately | **Fixed.** Completion (notifier + AWAKEN) happens only at a tick and honours `MIN_PRESENT_INTERVAL` (section 2.2). Every compositor then repaints at the mode rate |
| DRM vblank does not run | **Unchanged, and harmless.** It does not run on a real RTX 2070 with 610.57.04 either (section 2.1). Compositors use flip events. `vblank=0` is the configuration |
| No OLUT, gamma commits fail | **Fixed.** The C5 HAL reports OLUT (`nvkms-evo3.c:5583-5589`). The device accepts the LUT methods |
| NV9010 served by a guest-side hrtimer | **Replaced.** The guest's own RM serves NV9010 from the device's `LAST_DATA` interrupt. X and vblank-semaphore users get the same tick as the flips |
| Limiter and its guest double free | **Not needed.** Pacing is display-side, as on hardware |

The extra flip completions of v1 issue 18 (NVKMS sends one per displayless flip while
nvidia-drm expects none on the first flip after activation) also go away. On a real head,
NVKMS registers flip-occurred events per layer only for changed layers
(`nvkms-evo.c:5011-5055`), and nvidia-drm's expectation (`__will_generate_flip_event`)
models hardware.

## 3. What the scanout must do for the gates

**Nothing beyond state.** Every frame check in the v1 gates and the Wayland path reads
memory the guest can reach:

| Check | Reads | Needs a device scanout? |
|---|---|---|
| vdisplay `pixel`: dumb buffer, `SETCRTC`, `GETFB2` readback, hash (`../Leandro-Test/scripts/test.sh:452-731`) | the dumb buffer in guest-visible GPU memory | no. The modeset must only complete |
| display `modeset`: `drm-modeset --flips 30`, "30 flips all completed, CRTC names the new framebuffer" (`test.sh:853-864`) | flip-complete events, CRTC state | no. Tick-paced completion |
| Wayland screencopy (`wlr-screencopy`, `ext-image-copy-capture`), Mutter screencast, Sunshine `capture=kms` (`GETFB2` + PRIME import of the plane's buffer) | the compositor's or the plane's buffer | no |
| display `present`: 120 frames at 50-75 FPS (`test.sh:942-967`) | client timing | no. It needs the tick at 60 Hz |
| a human watching, a boot console, a Windows guest | the actual scanout | **yes**, later |

The device keeps, per window, the latched surface (ISO context DMA + offset, format, pitch
or block-linear layout from the window methods). That costs nothing, and a sink can use it
later.

**De-tiling a block-linear surface** (the host sink, later):
- **Why it is needed.** NVIDIA's GBM allocates scanout buffers with block-linear modifiers
  by default, so a host sink must de-tile [I].
- **Layout.** A Turing GOB is 64 bytes by 8 rows (512 B), arranged in 16-byte sectors.
  Blocks stack 2^h GOBs vertically (NVKMS surfaces typically use h = 4, 128 rows) [I, from
  the nouveau/Mesa layout].
- **From host RAM (the fake FB, before E5b).** One 1920x1080 XRGB8888 frame is 8.3 MB, or
  about 518 000 sector copies. At 60 Hz that is about 500 MB/s of scattered 16-byte copies:
  roughly 10-25 % of one host core [I]. A dirty-frame rule (de-tile only after a latch) and a
  lower sink rate cut it further.
- **From real VRAM (E5b on, D11 reservation).** CPU reads through a BAR1 mapping are
  uncached PCIe reads. At tens to low hundreds of MB/s, 8.3 MB takes 50-300 ms: infeasible
  at 60 Hz [I; to be measured].
  - The workable route is a **host copy engine** blit with a block-linear source into a
    pitch-linear sysmem buffer. `TURING_DMA_COPY_A` does the layout conversion itself. The
    host-exec primitive already moves VRAM to sysmem through a CE ([host-exec.md](host-exec.md)
    stage D).
  - That is a host GPU action per frame and **needs the operator's word** (the display-path.md
    §2.4 risk, unchanged).

## 4. Effort and the phased plan (connector-only route E)

Scope compared with display-path.md's route E: the same GSP-role and register work. The
method interpreter is cut to a decoder for the methods in section 1.4 (the others are
recorded and ignored). No scanout sink before phase E. Wayland first.

| Phase | Content | Agent-days |
|---|---|---|
| **A. Display present, nothing visible** | fuse 0x21C04 = 0; FB layout with display (64 KiB instance memory, VGA workspace; `fb.rs` and `chip/turing.rs` layout; re-verify `kbusVerifyBar2`); `GET_IP_VERSION`; `GET_STATIC_INFO`; the init RPCs; `NV04_DISPLAY_COMMON`; the DISP entry in `interrupts.toml` (leaf 4) | 1-2 |
| **B. Smallest guest-visible connector: EDID and a connected DRM connector with modes, no modeset** | `NVC570`, the core/window/cursor channel allocs and the pushbuffer RPC; USERD GET := PUT; caps page 0x640000; all NV0073 answers of §1.3; IMP "possible"; the EDID in Rust. Guest: `nvidia-modeset`, `nvidia-drm modeset=1 fbdev=0`. `fbdev` defaults to 1 (`DRM/nvidia-drm-os-interface.c:46`) and would modeset at load | 2-3 |
| **C. A Wayland compositor starts on the emulated connector and its frame callbacks run at the mode's refresh rate** | the method decoder; RAMHT/ctxdma lookup; core and window notifiers; semaphores; the per-head timer; `LAST_DATA` and AWAKEN; `FE_CORE_HEAD_STATE`; `RG_DPCA` and LOADV; OLUT methods accepted. First compositor: Weston `--backend=drm` or sway, on GBM. Optional X check afterwards | 4-6 |
| **D. GNOME/Mutter desktop and the gates** | Mutter atomic with colord's gamma LUT; cursor channel; mode switch 1080p60 → 720p60 → 1080p50; EDID-change hotplug through `POST_EVENT`; adapt `tools/caraxes-gate.sh vdisplay` and `display` from X to Wayland (Sunshine `capture=kms`); stages that need compute stay on E5 | 2-3 |
| **E. (optional) Host-side sink** | de-tile to a memfd/PNG per frame from the fake FB; the CE blit route after E5b with the operator's word; a viewer later | 2-4 |
| **Total** | A-D: **9-14**; with E: **11-18** | vs 15-25 for full route E |

Where the saving comes from: no scanout sink (2-4 days out of the old 2-4), a decoder
instead of an interpreter (3-5 instead of 5-8), a smaller NV0073 surface without DP (the GSP
role: 2-3 instead of 3-5).

### Falsifiable expectations per phase

**A.**
- *Expect:*
  - `RmInitAdapter succeeded!` with KernelDisplay present;
  - the device trace shows `GET_IP_VERSION`, `GET_STATIC_INFO`, `WRITE_INST_MEM` (base and
    size inside the new FB layout) and the `NV04_DISPLAY_COMMON` alloc;
  - `tools/caraxes-gate.sh nvml` still 11/11, `uvm verify 1` still passes;
  - no `nvidia-modeset` loaded yet.
- *Falsified if:* init aborts at a display RPC (the log names it; `gpu.c:2272-2275` aborts on
  any error but `NOT_SUPPORTED`), or `kbusVerifyBar2` fails after the layout shift.

**B.**
- *Expect:*
  - `/sys/class/drm/card0-DVI-D-1/status` = `connected`;
  - `modes` lists `1920x1080` first, then `1280x720` and the 50 Hz mode, and **not** the
    174 MHz 75 Hz test mode;
  - `edid` is byte-identical to the device's EDID, and `edid-decode --check` and
    `edid-verify` pass;
  - `modetest -c -p` shows one connector, one CRTC, primary and cursor planes;
  - dmesg has no `nvidia-modeset: ERROR`;
  - the device trace has **no** core `UPDATE` carrying `HEAD_SET_RASTER_SIZE`.
- *Falsified if:* zero connectors (NVKMS logs the failing step by name: "Failed to determine
  connector type", "Failed to get supported display device(s)", "Failed to query DFP info"),
  a missing or empty EDID, or the 75 Hz mode listed (pixel-clock limit wrong).

**C** (the first Wayland milestone).
- *Expect:*
  - Weston or sway starts on `card0` with atomic KMS and GBM;
  - a frame-callback probe (a small `wl_surface.frame` loop, or `weston-presentation-shm`)
    reports **60.0 ± 0.5 Hz**, and `wp_presentation` refresh 16.667 ms;
  - flip timestamps (DRM debug or a KMS probe) are spaced 16.67 ms with jitter under 1 ms;
  - `drm-modeset --flips 30` completes 30 flips in 0.50 ± 0.03 s;
  - `vkcube --wsi wayland` (FIFO) reports 58-62 FPS;
  - zero `Page flip failed`;
  - the device's tick counter equals the guest's head vblank counter.
- *Falsified if:* frame callbacks above 62 Hz (completion not tick-gated: the v1 symptom),
  below 58 Hz or stalls (a lost completion, or a late tick), NVKMS notifier-wait timeouts
  (post-`UPDATE` state wrong), or a Wayland client drawing without a frame on the connector.

**D.**
- *Expect:*
  - GNOME Wayland on `card0` with colord active, **0** `Page flip failed`;
  - after switching to 1080p50, callbacks at 50.0 Hz (the timer follows the mode);
  - after an EDID change pushed as a hotplug event, the new mode list appears;
  - the adapted `vdisplay` gate passes: connector, EDID, pixel readback, flips;
  - the `display` gate passes every stage that does not need compute.
- *Falsified if:* gamma-LUT commits fail (OLUT or LUT-method handling), the rate stays at
  60 after the switch, or a hotplug is not picked up.

### Risks

1. **The post-`UPDATE` state GSP would present is unspecified** (display-path.md §2.5). NVKMS
   waits on core notifiers, `GET_CHANNEL_INFO`, head state and IMP answers, and reports a
   mistake only as a timeout. Mitigation: NVKMS logs name the wait; nouveau's r535 is a
   second reading of the same boundary; a passthrough capture (R1/R3) would settle it once the
   procedure is allowed.
2. **Cadence semantics.** On Turing the compositors' only clock is flip completion
   (section 2.1). Early completion reproduces v1. A lost completion freezes wlroots. Late
   ticks drop frames. The MSI path through vfio-user and the guest's deferred NVKMS work add
   latency the device cannot see. The DRM vblank counter stays dead with sequence 0, as on
   the real card, so any compositor feature that needs a working `drmWaitVBlank` (Weston's
   start path) takes its fallback. Mitigation: phase C's jitter measurement, and absolute
   timer deadlines.
3. **Per-version drift** (D14, P9). The internal display RPCs, the NV0073 parameter layouts
   and NVKMS's use of the C57x methods move between driver versions. The display subsystem
   adds a whole family to the per-version ABI crate, and a guest driver that differs from the
   host multiplies it.

Further risks:
- **HDMI-vs-DVI EDID blocks.** A CEA-861 extension with an HDMI VSDB plus `HDMI_CAPABLE`
  would switch on infoframes, audio and SCDC controls. Keep the EDID a bare base block and
  the flag clear.
- **Audio.** `SET_ELD_AUDIO_CAPS` and the audio power control still arrive. The device
  presents no HDA function (the real card's function 1). If the config space seed advertises
  multifunction or the HDA function, the guest's snd driver may probe it.
- **Pixel-clock ceiling.** Single-link DVI caps at 165 MHz: 1080p60 yes, 1080p120 or 1440p60
  no. Dual-link DVI (DFP `LINK_DUAL` + caps `dualTMDS`) raises it to 330 MHz (1440p60,
  2560x1600@60 RB, 4K30). Beyond that needs HDMI 2.0 or DP, which is the complexity this
  plan avoids.
- **Scanout from VRAM after E5b** needs the host CE (section 3).
- **Shared code with the CUDA path.** Phase A touches the FB layout and the interrupt table,
  which E5b/E5d also change. Schedule it after CUDA, as P12 says.
- **fbdev modeset at module load** (`fbdev=1` default) makes phase B's "no modeset" depend on
  a module parameter. With `fbdev=1` the first modeset happens at load, which is phase C
  territory.

## 5. What must be true in the device today for the driver to even ask

| Condition | Today | Needed |
|---|---|---|
| `NV_FUSE_STATUS_OPT_DISPLAY` 0x21C04 bit 0 | 1 (`crates/vfio-user-nvgpu/chips/tu106/boot.toml:48-50`), so `gpuStatePreInit` deletes KernelDisplay and its classes before any display RPC (`gpu.c:2231-2271`) | 0 |
| FB layout | computed "with no display" (`src/chip/turing.rs:152`) | with the 64 KiB display instance memory and the VGA workspace: the reserved block moves from 0x20000 to 0x30000 (fb-model.md §3.2, `kernel_gsp_tu102.c:787`); `NV_PDISP_VGA_WORKSPACE_BASE` 0x625F04 answered |
| `INTERNAL_DISPLAY_GET_IP_VERSION` | never asked | 0x04000000. `NOT_SUPPORTED` silently removes the display again; any other error aborts init (`gpu.c:2272-2275`) |
| `GspStaticConfigInfo` | no display field exists | nothing to change (display-path.md §2.1) |
| Class list | TU106's static list already has the C57x classes (`g_gpu_class_list.c:338-359`) | nothing; with KernelDisplay present, `NVA083_GRID_DISPLAYLESS` is dropped as in any stock build |
| Kernel interrupt table | no DISP entry (`chips/tu106/interrupts.toml`) | `MC_ENGINE_IDX_DISP` with a vector in leaf 4 (128-159) that does not collide with the existing entries |
| Control dispatcher | display controls unanswered (`ctrl.rs:470-477`: `NOT_SUPPORTED`, "unanswered") | the §1.3 set, each labelled with its source (D9): EDID and connector model `synthesized` or `profile`, never `host`; nothing about the host's real monitors is forwarded |
| Guest modules | nvidia only | `nvidia-modeset` + `nvidia-drm modeset=1` (`fbdev=0` for phase B), `vblank` left at 0 |

With the fuse at 0, the stock non-GRID driver asks for a display by itself. No build flag,
no guest patch and no temporary aid are involved. That is the point of this route compared
with D1.

## Sources

- Prior study: [display-path.md](display-path.md) §2 (classes, RPCs, BAR0, instance memory,
  notifiers), §4 (displayless HAL), §5 (route E breakdown).
- v1: `docs/history/OPEN-QUESTIONS-2026-09-18.md` issues 7, 18, 24, 25, 27, 30, 37-41;
  `docs/history/llm-2026-09-18.md` §4-5; `docs/DISPLAY.md`, `docs/WAYLAND.md`;
  `guest-module/virtio_nvrm/nvrm_edid.c`.
- Journal (read-only): `../Leandro-Journal/claude/leandro-test-report.md` (4b: the gamma-LUT
  cause, MangoHud numbers), `claude/desktop-handoff.md` (Wayland guest configuration, flip
  failures), `WAYLAND-QUICKSTART-CHECK.md`, `claude/sources-2026-09-18/src-evidence.md`
  (Q14-Q18).
- Tests (read-only): `../Leandro-Test/scripts/test.sh` (vdisplay 452-731, display 734-1128).
- Vendor: files and lines cited inline under `OGK/`. nouveau `nvkm/engine/disp/r535.c`,
  `nvkm/subdev/gsp/r535.c` (linux-6.12).

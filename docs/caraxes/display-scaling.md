<!-- SPDX-License-Identifier: MIT -->
# Scaling the emulated display: which connector, 4K 144 Hz per head, several heads

Written 2026-09-23 (source survey, no VM run, no host GPU access). It answers the
operator's questions of 2026-09-23: *"research if HDMI or DisplayPort are easier, or what
else exists; this software targets homelabs and small businesses who cannot afford vGPU,
and me because I am curious; what would it take to scale up to 4K 144 Hz per head, and what
about multiple heads; we then think about how to implement it"*. Wayland compositors in the
guest are the target (X at most a check). Frames are consumed inside the guest (guest-side
capture and streaming, as v1's gates do with Sunshine/Moonlight) unless a host-side sink is
chosen later.

This is the second display study. It builds on [display-connector.md](display-connector.md)
(one DVI-D single-link connector, the timer-gated flip completion, phases A-D at 9-14
agent-days) and [display-path.md](display-path.md) (classes, RPCs, BAR0 pages, route E), and
does not repeat them. Pointers: `OGK/` is `vendor/open-gpu-kernel-modules` at 610.57.04;
`NVKMS/` is `OGK/src/nvidia-modeset/src`; `DPLIB/` is `OGK/src/common/displayport/src`;
`HDMIPKT/` is `OGK/src/common/modeset/hdmipacket`; `CLS/` is
`OGK/src/common/sdk/nvidia/inc/class`; `CTRL/` is `OGK/src/common/sdk/nvidia/inc/ctrl`;
nouveau is `linux-6.12/drivers/gpu/drm/nouveau` in the session scratchpad. Tags: [V] read in
source, [M] measured (v1 journal), [W] public documentation fetched on 2026-09-23 (URLs in
Sources), [S] public specification numbers from general knowledge (DVI, HDMI, DP, CVT),
[K] general knowledge of compositors, Sunshine or NVIDIA products, not read in a local source,
[I] inferred, [G] guess.

**Short answer.**
- **Connector: DisplayPort SST (single stream, no MST, no HDCP), one DP connector per
  head.** It costs about 3-5 agent-days more than DVI-D (a DPCD register model, an AUX
  engine with I2C-over-AUX for the EDID, and "training succeeded" answers), but it is the
  only connector whose honest Turing limits reach 4K 144 Hz (HBR3 x4 plus DSC). It is the
  connector a real RTX 2070 mostly has (3 of its 4 outputs are DP), it keeps 10 bpc and VRR
  open, and it runs NVIDIA's most-used code path. HDMI 2.0 tops out at 600 MHz (4K60) and
  costs almost as much (infoframe registers, SCDC, HDMI EDID blocks). DVI-D stays the
  first-light connector of phases A-D (section 1).
- **4K 144 Hz per head is possible on TU106 emulated.** 4K144 with CVT-RB2 timing is a
  1306 MHz pixel clock, 31.4 Gbit/s at 8 bpc. That is more than HBR3 x4 carries (25.92
  Gbit/s), so DSC at 8-12 bpp is needed, exactly as on a real RTX 20 card. 4K120 at 8 bpc
  (1076 MHz, 25.8 Gbit/s) just fits without DSC. Every check that could say no in GSP-client
  mode is answered by the device: IMP (`IS_MODE_POSSIBLE`), the caps page, `DP_GET_CAPS` and
  the DPCD. What really limits it is outside the display. NVENC encodes roughly 210-230 4K
  frames/s on Turing, so one 4K144 stream fits and two do not. Each 4K head needs 161 MiB of
  VRAM for the display. The device timer needs about 0.5 ms p99 accuracy against a 6.94 ms
  period (section 2).
- **Several heads: at most 4.** TU106 and NVKMS both cap it (`NV_MAX_HEADS` = 4). There are
  4 SORs and 8 windows, 2 per head. Each head is one connector, one timer and one
  head-timing interrupt bit, with its own mode and refresh. NVIDIA vGPU's best Q profiles
  offer 4 heads up to 5K, or 2 at 8K, paced by a 60 FPS frame-rate limiter. Caraxes can match
  the head count and exceed the refresh rate. The limits are NVENC and VRAM, not the display
  model (section 3).
- **Product: guest-side streaming is the display.** Sunshine/Moonlight, GNOME Remote
  Desktop (RDP) or wayvnc run inside the guest. A host-side sink is only a low-rate console.
  A physical monitor is impossible with a synthetic device. Runtime hotplug and EDID changes
  are cheap and worth a small control API. HDR, audio over the connector and MST are skipped
  (section 4).
- **Effort:** phases A-D as planned (9-14), then multi-head (+2-3), DP SST (+3-5), 4K144 with
  DSC and 144 Hz timing (+2-3), and the runtime display API (+1-2). **17-27 agent-days to 4
  heads of 4K144 on DP**, against 9-14 for one 1080p60 DVI-D head (section 5).

---

## 1. Connector comparison for an emulated link

### 1.1 What "complexity" means here

No physical link exists. In GSP-client mode everything below the NV0073/NV2080 control
interface belongs to GSP, and GSP here is the device. A connector's cost is therefore:
- the controls NVKMS sends for it (all `ROUTE_TO_PHYSICAL`, so they reach the device);
- the state NVKMS reads back and cross-checks (DPCD, SOR caps, EDID blocks);
- the BAR0 registers the guest touches directly, outside any control (the HDMI infoframe
  page);
- the limits the **guest itself** enforces, which the device cannot talk its way past.

Common to every connector (display-connector.md §1.3): `SYSTEM_GET_SUPPORTED`,
`OR_GET_INFO`, `GET_CONNECTOR_DATA`, `DFP_GET_INFO`, `GET_CONNECT_STATE`, `GET_EDID_V2`,
`GET_PCLK_LIMIT`, `DFP_ASSIGN_SOR`, IMP, `SET_ELD_AUDIO_CAPS`. The rows below list only what
comes on top.

### 1.2 Where the guest enforces pixel-clock limits [V]

| Check | Source | Who controls the input |
|---|---|---|
| EDID range-limit max pixel clock (unless `NO_EDID_MAX_PCLK_CHECK`) | `NVKMS/nvkms-modepool.c:1224-1284` | device (EDID) |
| Raster size ≤ the class field width (`HEAD_SET_RASTER_SIZE` mask) | `NVKMS/nvkms-modepool.c:1289-1292`, `nvkms-hal.c:92-93` | fixed by the class |
| DVI (non-HDMI TMDS): single link above 165 MHz fails; `DUAL_TMDS` switches to dual link above 165 MHz; the 165/330 MHz figures are hard-coded (`TMDS_SINGLE/DUAL_LINK_PCLK_MAX`), and the override to the SOR's TMDS maximum (`allowDVISpecPClkOverride`) is an NVKMS API request flag that nvidia-drm never sets | `nvkms-dpy.c:926-944`, `nvkms-evo.c:6816-6850`, `nvkms-dpy.c:3156` | NVKMS constants |
| HDMI TMDS: pixel clock ≤ the SOR caps' `TMDS_MAX`, downgrading bpc/format first | `nvkms-dpy.c:877-883`, `nvkms-hdmi.c:2070-2160` | device (caps page 0x640000, `SOR_CLK_CAP` bits 23:16, units of 10 MHz) |
| DP: the DP library's bandwidth check against the GPU's `DP_GET_CAPS` (max link rate, DSC caps) and the sink's DPCD (rate, lanes, DSC) | `nvkms-modepool.c:1747-1753` → `nvDPValidateModeEvo`; `DPLIB/dp_evoadapter.cpp:279-333` | device (control plus DPCD) |
| 2Heads1OR: if the pixel clock exceeds the **head's** `HEAD_CLK_CAP.PCLK_MAX` (x10 MHz) and the mode is DSC or YUV420, one API head takes two hardware heads | `nvkms-evo.c:9837-9877`, `nvkms-evo3.c:5184-5200` (`supportsMergeMode` TRUE on the Turing HAL, `nvkms-evo3.c:7812`) | device (caps page) |
| IMP: `NVC372_CTRL_CMD_IS_MODE_POSSIBLE`, flags 0x48 = `NON_PRIVILEGED \| ROUTE_TO_PHYSICAL` (`OGK/src/nvidia/generated/g_disp_objs_nvoc.c:1870-1872`); all heads in one call (`nvkms-evo3.c:3229-3345`) | `nvkms-modepool.c:1766-1778` | **device**: `bIsPossible`, `minRequiredBandwidthKBPS`, `floorBandwidthKBPS` |
| Heads per disp ≤ `NV_MAX_HEADS` = 4 | `CTRL/../nvlimits.h:52`, `nvkms-api-types.h:32` | fixed |

**Consequence:** where the guest compares against a constant (DVI 165/330 MHz, 4 heads, the
EDID DTD field), the device cannot help. Where it compares against a caps value (HDMI
`TMDS_MAX`, head `PCLK_MAX`, DP rates, IMP), the device sets the value.

### 1.3 Per connector

Pixel-clock ceilings are for 8 bpc RGB. Link arithmetic: DVI/HDMI TMDS carry 24 bit per
pixel clock per link; DP 1.x uses 8b/10b, so the payload is 0.8 of the raw rate. [S]

| Connector | Extra controls / state beyond DVI-D single link | Guest-side ceiling | What the driver validates | Effort vs DVI-D design |
|---|---|---|---|---|
| **DVI-D single link** (the current design) | none | **165 MHz** (NVKMS constant): 1080p60 (148.5 CEA) | EDID; `RequiresDualLink` → fail | 0 |
| **DVI-D dual link** | DFP flag `LINK_DUAL`, SOR caps `DUAL_TMDS`, OR protocol `DUAL_TMDS` (5) | **330 MHz** (NVKMS constant): 1440p60 (RB2 234.6), 2560x1600@60 RB, 1080p120, 4K30 (297). 1080p144 is 333.2 MHz RB2: 1 % too much | same | +0.1 day (three fields) |
| **HDMI 1.4 / 2.0 (TMDS)** | EDID: CTA-861 extension with HDMI VSDB (else `nvDpyIsHdmiEvo` is false, `nvkms-hdmi.c:215-257`), HF-VSDB for >340 MHz and 4K60 RGB (`nvkms-hdmi.c:297-315,1680-1692`); DFP `HDMI_CAPABLE`; controls `GET_HDMI_GPU_CAPS`, `SET_HDMI_SINK_CAPS` (SCDC, >340 MHz scrambling), `SET_HDMI_ENABLE`, `CTRL_HDMI`, `GET_HDMI_SCDC_DATA`, `SET_OD_PACKET(_CTRL)`, `SET_HDMI_AUDIO_MUTESTREAM`, `DFP_SET_AUDIO_ENABLE`, ELD with audio. **Infoframes are BAR0 register writes**, not controls: on Turing the HDMI packet library uses class C571 with the C371 interface, which writes AVI/generic/GCP/VSI into the NVC371 SF user page (0x6F0000) and polls `SF_HDMI_INFO_STATUS.SENT` = DONE (`HDMIPKT/nvhdmipkt.c:112-124`, `nvhdmipkt_9171.c:75-135`). SCDC itself is an I2C transaction GSP runs on its own, so the device only acknowledges | honest Turing: `TMDS_MAX` 600 MHz (`CLS/clc573.h:598-599` reset value 0x3C): 4K60 RGB (594), 4K120 4:2:0 (594). No FRL on Turing (`supportsHDMIFRL` FALSE, `nvkms-evo3.c:7808`). **No 10 bpc over HDMI** on the Turing HAL (`supportsHDMI10BPC` FALSE, `:7813`) | TMDS vs SOR caps; HF-VSDB presence; colour-format downgrade | **+1.5-2.5** |
| HDMI with "overclocked" `TMDS_MAX` | as HDMI | the caps field reaches 2550 MHz, and NVKMS checks HDMI only against it and the EDID (above), so a 1306 MHz 4K144 TMDS mode would validate [I] | same | as HDMI. **Rejected**: no real Turing can produce it, it is untested NVKMS territory, and it needs an EDID that claims a TMDS rate no HDMI version defines |
| **DisplayPort SST** | `DP_GET_CAPS` (GPU max link rate, MST, FEC, DSC encoder caps; `DPLIB/dp_evoadapter.cpp:279-333`); a **DPCD register model** behind `DP_AUXCH_CTRL`: receiver caps 0x000-0x00F and extended caps 0x2200, `MSTM_CAP` 0x021 = 0, DSC caps 0x060-0x06F, FEC 0x090, link config 0x100-0x10F, `SINK_COUNT`/IRQ vectors/lane status 0x200-0x207, `SET_POWER` 0x600, source/sink OUI, HDCP caps 0x68028 = 0 (the register names the DP library uses are in `dp_configcaps.cpp`); **I2C-over-AUX** at 0x50/0x30 for the EDID (the SST path reads over AUX, `dp_connectorimpl.cpp:8159`; `GET_EDID_V2` only in the ACPI case, `:8437-8443`); **link training** = `DP_CTRL` returns success and the device sets `LINK_BW_SET`/`LANE_COUNT_SET` and lane status CR/EQ/symbol-lock/interlane-align (the DP library loops on `DP_CTRL`, `dp_evoadapter.cpp:1273-1460`; nouveau r535 does the same, `nvkm/engine/disp/r535.c:955-1015`); `DP_GET_LINK_CONFIG`, `DP_CONFIG_STREAM`, `DP_SET_MSA_PROPERTIES`, `DP_NOTIFY_LT`, `DP_MAIN_LINK_CTRL`, `DP_SET/GET_LANE_DATA`, `DP_CONFIGURE_FEC`, `GET_HDCP_STATE` (all false), `DFP_UPDATE_DYNAMIC_DFP_CACHE`; hotplug as `POST_EVENT` `HOTPLUG` plus `NV2080_NOTIFIERS_DP_IRQ` for short pulses (`nvkms-rm.c:1929-1968`). The DP library references 41 distinct NV0073 controls; about 12 need a real answer for SST without MST, eDP, HDCP or test patterns [V count, I split] | HBR2 x4 17.28 Gbit/s: 720 MHz (4K60 RB2 522.6); **HBR3 x4 25.92 Gbit/s: 1080 MHz** (4K120 RB2 1075.8, 0.4 % margin); **with DSC** (Turing has it: `DP_GET_CAPS.DSC`, `POSTCOMP_HEAD_HDR_CAPA_DSC_PRESENT`, `CLS/clc573.h:316-387`; NVKMS writes `HEAD_SET_DSC_*`, `nvkms-evo3.c:7233-7311`): 4K144 at 12 bpp = 15.7 Gbit/s | DP library: link bandwidth, DSC slice/bpp, watermark; DPCD consistency (link-lost checks re-read lane status, `dp_connectorimpl.cpp:5521`) | **+3-5**, DSC +0.5-1 |
| DP MST | a branch device: sideband messages (`DOWN_REQ`/`UP_REQ` in DPCD 0x1000-0x1BFF), topology, payload table, `DP_TOPOLOGY_ALLOCATE_DISPLAYID`, `SEND_ACT` | shares one link's bandwidth across heads | full MST state machine | +5-10. **Not recommended**: several SST connectors give the same heads without a branch device |
| eDP | `DP_GET_EDP_DATA`, panel power sequencing with `retryTimeMs`, backlight (`SET/GET_BACKLIGHT_BRIGHTNESS`), PSR DPCD, `internalDispActiveMask` | as DP | as DP plus panel rules | +DP +1-2, no benefit for a VM |
| VGA/DAC, LVDS | Turing's caps class has no DAC field and no Turing dGPU drives LVDS [I from `CLS/clc573.h`: no `DAC_EXISTS`] | — | — | not applicable |
| `VIRTUAL_WFD` (0x70) | NVKMS maps it to `UNKNOWN` and **drops the connector** (`nvkms-rm.c:731,781-784`) | — | — | useless |
| GRID displayless (NVA083), vGPU's virtual display | needs a `GRID_BUILD=1` guest `nvidia.ko` (display-path.md §4); flips complete in software, no vblank, fixed "NVIDIA VGX" EDID, unlicensed default 1 head, 2560x1600, 4 096 000 pixels (journal, `objgriddisplayless.c:34-58`) | max resolution steerable by one GSP event | NVKMS mode pool vs `maxPixels` | outside this design (the D1 temporary aid) |

**Recommendation: DP SST, one connector per head, no MST, no HDCP, DSC as a capability.**
DVI-D stays the connector for first light (phases A-D). Its only virtue is the smallest
state, and that matters while the post-`UPDATE` state is still being found by trial
(display-connector.md risk 1). Then switch the connector kind to DP. Reasons:
1. **Headroom.** HDMI tops out honestly at 600 MHz on Turing and DVI at 330 MHz, while DP
   reaches 1080 MHz and, with DSC, 4K144 per head.
2. **Least unusual path.** The DP library is what every modern NVIDIA desktop runs, so an
   honest HBR3 plus DSC sink hits well-trodden code. The overclocked-TMDS shortcut would not.
3. **The extra state is data, not behaviour.** The DPCD is a register file with about ten
   side effects (training, power, the DSC and FEC enables). There is no timing and no link to
   simulate. The HDMI path, by contrast, needs a BAR0 register page with send-status
   semantics.
4. **Features that only DP has on this HAL:** 10 bpc output (HDMI 10 bpc is off on Turing),
   and adaptive sync (VRR) later.

## 2. 4K 144 Hz per head

### 2.1 Pixel clock and link arithmetic [S, computed in the session scratchpad]

CVT reduced blanking v2 (horizontal blank 80 pixels, minimum vertical blank 460 µs):

| Mode | h x v total | Pixel clock | 8 bpc (24 bit) | 10 bpc (30 bit) |
|---|---|---|---|---|
| 1920x1080@60 | 2000 x 1111 | 133.3 MHz | 3.20 Gbit/s | 4.00 |
| 1920x1080@144 | 2000 x 1157 | 333.2 MHz | 8.00 | 10.00 |
| 2560x1440@144 | 2640 x 1543 | 586.6 MHz | 14.08 | 17.60 |
| 3840x2160@60 | 3920 x 2222 | 522.6 MHz | 12.54 | 15.68 |
| 3840x2160@120 | 3920 x 2287 | **1075.8 MHz** | **25.82** | 32.27 |
| 3840x2160@144 | 3920 x 2314 | **1306.2 MHz** | **31.35** | 39.19 |
| 5120x2880@60 | 5200 x 2962 | 924.1 MHz | 22.18 | 27.72 |
| 7680x4320@60 | 7760 x 4443 | 2068.7 MHz | 49.65 | 62.06 |

**A correction to the question's figures.** "4K144 8-bit RGB ≈ 1.07 GHz, 25.8 Gbit/s" is the
**4K120** RB2 mode. The active pixels of 4K144 alone are 3840 x 2160 x 144 = 1194 MHz
(28.7 Gbit/s), so no timing brings 4K144 at 8 bpc under the 25.92 Gbit/s of HBR3 x4.

| Link | Payload | Max pixel clock at 24 bpp | 4K144? |
|---|---|---|---|
| DVI single / dual | 3.96 / 7.92 Gbit/s | 165 / 330 MHz | no |
| HDMI 1.4 (340 MHz) | 8.16 | 340 MHz | no |
| HDMI 2.0 (600 MHz) | 14.40 | 600 MHz | no; also not at 4:2:0 (653 MHz RB2) |
| DP 1.2 HBR2 x4 | 17.28 | 720 MHz | only with DSC ≤ 13 bpp |
| DP 1.4 HBR3 x4 | 25.92 | 1080 MHz | 4K120 without DSC (0.4 % margin; a 0.5 % downspread allowance would break it, so the device reports `bSupportDPDownSpread` FALSE [I]); **4K144 with DSC**: 8 bpp 10.45, 10 bpp 13.06, 12 bpp 15.67 Gbit/s |

**EDID encoding.** A base-block detailed timing stores the pixel clock in 16 bits of
10 kHz, so it cannot express more than 655.35 MHz [S]. 4K120 and 4K144 must come from a
CTA-861 extension with a DisplayID Type VII timing block (T7VTDB, 1 kHz units, 24 bits) or
from a DisplayID 2.0 extension. NVKMS's parser handles both
(`OGK/src/common/modeset/timing/nvt_edid.c:1082`, `nvt_edidext_861.c:1769-1785`,
`nvt_displayid20.c:1569-1572`). This is also how real 4K144 monitors do it [K]. The
range-limit descriptor's maximum pixel clock (x10 MHz) reaches 2550 MHz.

### 2.2 What TU106's display engine allows per head, and who says so

- **DSC on Turing: yes, for DP.** The C57D class has `HEAD_SET_DSC_CONTROL`/`PPS_*`
  (`nvkms-evo3.c:7233-7311`), and the caps class has DSC presence and buffer sizes per head
  (`CLS/clc573.h:316-387`). HDMI DSC needs FRL, which Turing lacks [V].
- **Head pixel clock.** NVKMS reads `HEAD_CLK_CAP(i).PCLK_MAX` x 10 MHz from the caps page
  (`nvkms-evo3.c:5169-5200`). The Turing header lacks the register, so NVKMS carries a
  workaround definition whose reset value is 0x85 = **1330 MHz** (bug 4028718 comment,
  `:5169-5175`). If that is Turing's real value, 4K144 RB2 (1306 MHz) fits one head. Above
  the cap, a DSC mode takes two hardware heads (2Heads1OR) [V mechanism, I value]. The real
  RTX 2070 value needs a passthrough capture (R1/R3) or stays synthesized (D9 label
  `synthesized`).
- **SOR clocks.** `SOR_CLK_CAP.DP_MAX` reset 0x51 (HBR3) and `TMDS_MAX` 0x3C (600 MHz)
  (`CLS/clc573.h:594-601`) [V reset values; I that TU106 reports them].
- **Who answers "is mode possible" in GSP-client mode: the device.** IMP is
  `ROUTE_TO_PHYSICAL` (above), so it reaches the device as `GSP_RM_CONTROL`. NVKMS uses only
  `bIsPossible` and two bandwidth numbers (`nvkms-evo3.c:3307-3345`). The bandwidth numbers
  go back into `SYSTEM_ALLOCATE_DISPLAY_BANDWIDTH`, which the device also answers. The device
  can therefore say "possible" to every combination that passes the guest's own checks.
  - Policy: answer from a small budget model (sum of head pixel clocks ≤ a configured
    maximum), not "always yes". That keeps mode lists sane and gives the operator one knob.
- **What the guest still checks itself:** the EDID (timings, range limits), the DP
  library's link budget against the device's `DP_GET_CAPS` and DPCD, the DSC parameters (the
  guest computes the PPS itself), the raster field widths, the scaler caps from the caps
  page, and the 4-head limit (section 1.2).
- **Verdict:** 4K144 on one head is reachable on emulated TU106 without any guest change:
  DP, HBR3 x4, DSC at 12 bpp, `HEAD_CLK_CAP` ≥ 0x83, and IMP "possible". That is the same
  configuration a real RTX 2070 would use for a real 4K144 monitor [I].

### 2.3 The timer at 144 Hz

- **Period.** 6.944 ms. The display-connector.md design is unchanged: absolute
  `CLOCK_MONOTONIC` deadlines, one timer per head, period = `htotal * vtotal / pclk` from the
  core `UPDATE` (2314 x 3920 / 1306.2 MHz = 6.944 ms).
- **Jitter budget** [I, K]. Each compositor has its own margin:
  - **sway** repaints on the flip event (with `max_render_time` off), so the whole latency
    chain eats into the render time.
  - **Weston**'s default `repaint-window` of 7 ms is longer than the period. At 144 Hz it has
    to be lowered (about 4-5 ms in `weston.ini`) [K].
  - **Mutter** adapts its render-time estimate.

  Targets for phase G: tick-to-`DRM_EVENT_FLIP_COMPLETE` latency p99 < 0.5 ms (7 % of the
  period), p99.9 < 1 ms, **no lost completion**, and the tick count equal to the guest's
  frame count over 60 s.
- **Where the risk is.** The host runs the operator's desktop on the same GPU and CPU. A late
  device thread is the main source of dropped frames.
  - Mitigations: a dedicated device thread with `SCHED_FIFO` (needs the operator's word, like
    any host scheduling change), `timerfd` slack set to 1 ns (`prctl(PR_SET_TIMERSLACK)`), and
    the absolute-deadline rule.
  - At 4 heads x 144 Hz that is 576 MSIs/s on the one DISP vector. Ticks of several heads that
    fall within about 50 µs can share one MSI [I].

### 2.4 Memory and bandwidth

- **One 4K surface.** `S = align(4*3840, 64) x align(2160, 128)` = 33 423 360 B = **31.9 MiB**.
  That is the journal's pitch rule (`research-vram-display-reserve.md` §5) [M].
- **Display VRAM reserve per head** `R = 5 S + 1 MiB`: **44 / 76 / 161 MiB** at 1080p /
  1440p / 4K. Measured peaks: 34 / 40 / 149 MiB with GNOME, Xwayland full screen and
  Moonlight (journal §4-5) [M].
  - The refresh rate does not change R: buffer count and size, not rate, drive it [I].
  - R was measured for **one** head. For N heads it is an estimate, N x R [I]: 4 x 161 =
    644 MiB at 4K, 16 % of a 4 GiB profile.
  - The v1 finding still applies: the display must never be the allocation that fails. A
    single refused 8.4 MiB scanout buffer froze the picture in 9 of 9 runs [M].
- **Compositor bandwidth.** A composited 4K144 frame is written once (4.48 GiB/s) and reads
  about as much from client buffers: roughly 9-10 GB/s per head, about 2 % of TU106's
  448 GB/s. For 4 heads that is under 10 %. A full-screen client with direct scanout skips the
  composition [I, K].
- **The scanout read a real head does** (4.8 GB/s at 4K144) **does not exist**: the device
  has no sink.
- **Capture for streaming.** Sunshine's KMS capture imports the plane's buffer (`GETFB2` +
  PRIME) and converts RGB to NV12 on the GPU. No copy over PCIe. That is small next to the
  compositor's traffic [K].
- **NVENC is the real ceiling.** Turing measured on an RTX 8000 at 1920x1080 [W]:
  - H.264 P1: 855 FPS (low-latency CBR);
  - HEVC P1: 932 FPS;
  - P3: 600 / 463 FPS.

  Scaled by 4x the pixels [I], 4K gives about 214 (H.264) / 233 (HEVC) FPS at P1 and about
  150 / 116 at P3. TU106 has one NVENC of the same generation [K], at a possibly lower video
  clock [G].
  - **One 4K144 stream fits at P1/P2, and 4K120 fits comfortably.** Two 4K144 streams, or
    4K144 at P3 or higher quality, do not.
  - 1080p144 streams: about 5-6 at P1.
  - H.264 level 5.2 is specified for about 4K60. Moonlight clients handle higher levels
    loosely, so **HEVC is the codec for 4K120+** [K, G].
  - Turing has no AV1 encoder.
  - The NVENC is shared with the host desktop and any other VM on the card. NVENC in the
    guest itself waits on E5d (mirror channels).

## 3. Multiple heads

### 3.1 What TU106 and the driver expose

| Resource | Count | Source |
|---|---|---|
| Heads | 4 (NVKMS maximum `NV_MAX_HEADS` = 4; class arrays allow 8) | `nvlimits.h:52`; `CLS/clc573.h:71-72`; TU106 = 4 simultaneous displays [K] |
| Windows | 8, 2 per head (`windowPresentMask` 0xFF) | class allows 32 (`CLS/clc573.h:242-246`); nouveau reads the mask from `GET_STATIC_INFO` (`r535.c:1488-1494`); 8 on TU106 [K] |
| Cursor channels | 1 per head | `dev_disp.h` v03 `CHN_NUM_CURS__SIZE_1` 8 |
| SORs | 4, crossbar, assigned at modeset by `DFP_ASSIGN_SOR` | `CLS/clc573.h:108-112,389`; 4 on TU106 [K] |
| Head timing interrupts | 1 bit per head in `FE_EVT_STAT_HEAD_TIMING(h)` 0x611800+4h, dispatch 0x611EC0 bit h; AWAKEN 1 bit per window, 0x611858 | display-connector.md §2.1; `dev_disp.h` v03 lines 217-289 |
| Per-head counters | `RG_DPCA(h)` 0x616330+0x800h, LOADV 0x61611C+0x800h, `FE_CORE_HEAD_STATE(h)` 0x612078+0x800h | as above |

### 3.2 How N connectors are declared

Everything scales by table size. Nothing new in the protocol:
- **Static info:** `numHeads` N, `windowPresentMask` = (1 << 2N) - 1;
  `SYSTEM_GET_NUM_HEADS` = N; `SPECIFIC_GET_ALL_HEAD_MASK` = (1 << N) - 1;
  `GET_VALID_HEAD_WINDOW_ASSIGNMENT`: windows 2h, 2h+1 → head h; caps page `SYS_CAP`
  `HEAD_EXISTS(h)` and `SOR_EXISTS(s)` for h, s < N, `SYS_CAPB.WINDOW_EXISTS` for 2N windows.
- **Connectors:** N displayId bits in `SYSTEM_GET_SUPPORTED` (for example 0x100 << k). Per
  displayId:
  - `OR_GET_INFO` (SOR, protocol `DP_A` or `SINGLE_TMDS_A`);
  - `GET_CONNECTOR_DATA` (index k, type `DP_EXT` or `DVI_D`);
  - `DFP_GET_INFO`;
  - its own EDID and, for DP, its own DPCD instance;
  - `GET_CONNECT_STATE` returns the mask of plugged connectors.
- **Names:** nvidia-drm names them `DP-1` … `DP-4` (or `DVI-D-1` …) [I].
- **Heads vs connectors.** NVKMS assigns heads to connectors at modeset. Declaring more
  connectors than heads is legal (real cards do), but only N light at once.

### 3.3 Independent modes and refresh rates

- **In the device:** one `Head` record per head, holding the raster from its own core
  methods, its own timer and phase, and its own window latches. A flip on window w is latched
  on the tick of the head that owns w.
- **In the guest:** IMP carries all heads in one call (`nvkms-evo3.c:3237-3290`). The
  device's budget model sums the heads.
- **Heads are not locked to each other.** NVKMS asks for no fliplock or scanlock unless
  framelock is configured (`nvkms-evo3.c:3094-3099`: `NO_LOCK`), so free-running timers per
  head are what it expects [V].

### 3.4 The compositor side [K]

- **Separate repaint loops per output.** wlroots, Mutter and KWin each run one repaint loop
  per output, paced by that output's own flip events. Mixed rates (144 + 60) work.
- **Streaming one output.** Sunshine captures one output per session (`output_name`). Two
  concurrent streams need two Sunshine instances on different ports, and each takes NVENC
  capacity.
- **Moonlight takes the client's resolution** only when that mode exists in the connector's
  EDID. That is why a runtime EDID API (section 4) is useful.

### 3.5 Cost model per head

| Item | Per head | Note |
|---|---|---|
| Device state | ~1 KiB (head, 2 windows, EDID ≤ 512 B, DPCD ~1 KiB sparse) | [I] |
| Device CPU | one timer wakeup and a few register writes per tick; a method decode per flip | microseconds per tick [I] |
| Guest interrupts | refresh rate MSIs/s | 144/s at 144 Hz |
| VRAM (display reserve) | 44 / 76 / 161 MiB at 1080p / 1440p / 4K | journal formula, one head measured |
| GPU bandwidth | ~2 % of TU106 VRAM bandwidth for a composited 4K144 head | [I] |
| NVENC | ~60 % of the engine per 4K144 HEVC P1 stream; ~15 % per 1080p144 | from the [W] figures, scaled [I] |

### 3.6 Against NVIDIA vGPU [W]

Current vGPU types (L4 and A16 tables, vGPU 19.0; Turing-era Q types used the same
`maxPixels` scheme, journal §2):

| Type class | Available pixels | Heads |
|---|---|---|
| -8Q and larger | 66 355 200 | 2 x 7680x4320, or 4 at ≤ 5120x2880 |
| -4Q | 58 982 400 | 1 x 8K, or 4 at ≤ 5K |
| -2Q | 36 864 000 | 1 x 8K, 2 x 5K, or 4 at ≤ 3840x2400 |
| -1Q, -2B/-3B | 18 432 000 | 1 x 5K, 2 x 4K, or 4 at ≤ 2560x1600 |
| -1B | 16 384 000 | 1 x 5K, 1 x 4K, or 4 at ≤ 2560x1600 |
| -nA | — | 1 x 1280x1024 |

Also in vGPU:
- A frame-rate limiter, 60 FPS for Q and 45 for B, "not supported" under Wayland (journal §2,
  KVM20 §3.11 / §2.5.4).
- Wayland guests only since vGPU 20.0, and **not on Turing** (journal §4).
- Console VNC shows only the primary display, as a "management convenience" (journal §2).
- GeForce cards such as the RTX 2070 cannot run vGPU at all.

**Caraxes against that:**
- 4 heads, the same as a -2Q or larger.
- 4K144 on each head, as far as NVENC and VRAM allow.
- No frame-rate limiter.
- A stock GeForce driver, and Wayland first.

It matches vGPU on head count and exceeds it on refresh rate. It does not do 8K: 7680x4320@60
needs 2069 MHz, which is 2Heads1OR with DSC on DP, possible in principle, with no NVENC
headroom to stream it [I].

## 4. Product view for homelabs and small businesses

| Question | Recommendation | Why |
|---|---|---|
| What "display" means | **Guest-side streaming is the product display**: Sunshine/Moonlight for desktops and games, GNOME Remote Desktop (RDP; also what vGPU 20 supports under Wayland [W, journal §4]) for office use, wayvnc for wlroots | frames never leave the GPU until they are encoded; this is what v1's gates already measure |
| Host-side sink (VNC/SPICE by the device or the VMM) | only as a **low-rate console** (boot, recovery, 5-30 FPS), later | from real VRAM each frame needs a host copy-engine blit, a host GPU action that needs the operator's word (display-connector.md §3); 4K144 would be 4.5 GiB/s over PCIe per head, a third of a Gen3 x16 link [I]. vGPU's console VNC is the same idea and also primary-only |
| A real monitor | **not possible** | a synthetic device has no PHY; the physical outputs belong to the host driver. Only whole-GPU passthrough gives a guest a monitor |
| Hotplug, EDID changes at runtime | **yes, small API**: add or remove a head, and set the EDID (mode list, preferred mode, name) through a device control socket; the device raises `HOTPLUG` (and DP long-pulse HPD) | lets a streaming host offer exactly the client's resolution and refresh; about 1-2 agent-days on top of the phase-D EDID-change path |
| HDR | **skip for now** | HDMI 10 bpc is off on the Turing HAL; DP 10 bpc would work, but compositor HDR on NVIDIA Wayland and Sunshine HDR on Linux are both young [K]; revisit after 4K144 |
| Audio over the connector | **skip** | the real card's audio is PCI function 1 (HDA); emulating an HDA controller and codec is a separate device (5-10 agent-days [G]). Sunshine captures a PipeWire/PulseAudio virtual sink in the guest, which needs nothing from the device. The ELD stays empty |
| VRR (adaptive sync) | **later option, DP only** | the device completes flips on its own clock, so VRR is "tick when the flip arrives, within [min, max] period"; NVKMS allows it for DP sinks with an adaptive-sync range (`nvkms-vrr.c:177-229`) [V condition, I device side]. Useful for games over Moonlight only if the client side keeps up [K] |
| Licensing | **stock driver, no vGPU licence, no GRID build** | route E needs neither; the D1 `GRID_BUILD=1` module stays a temporary aid. GeForce NVENC has a per-system limit on concurrent sessions (8 in recent drivers [K, unverified]) |

## 5. Implementation sketch

### 5.1 Data model in the device

```text
DisplayConfig            (profile axis: CLI/TOML, runtime API)
  heads: 1..=4
  connectors: Vec<ConnectorCfg>       kind = DviD | DviDual | Dp { max_rate, lanes, dsc, vrr }
  edid: per connector, synthesized from a mode list (base block + CTA-861 ext with T7VTDB)
  imp_budget_khz: sum of head pixel clocks the device calls "possible"

Connector                (state)
  display_id: u32 bit, sor: Option<u8>, kind, edid: Vec<u8>, plugged: bool
  dpcd: Option<Dpcd>     sparse register file + side effects (training, power, DSC/FEC enable)
Head
  index, window_mask, raster {htotal, vtotal, pclk_khz}, active, dsc, vrr_range
  timer: HeadTimer       absolute deadlines, phase, tick count (RG_DPCA / LOADV)
Window
  index, head, pending: Option<WindowUpdate>, latched surface (ctxdma, offset, format, layout)
Display                  (owner)
  connectors, heads[4], windows[8], channels (core, window, window-imm, cursor), instmem, caps page
```

Answers by source (D9 labels): EDID, DPCD and connector table `synthesized` (or
`profile`); the caps page `synthesized` or, after R1/R3, `capture`; nothing from the host's
own monitors.

### 5.2 How it fits the D14 layout

| Axis | New content |
|---|---|
| Chip (`chips/tu106/display.toml`, `src/chip/`) | head, window and SOR counts; caps page values (`SYS_CAP`, `SYS_CAPB`, `HEAD_CLK_CAP`, `SOR_CAP`, `SOR_CLK_CAP` DP/TMDS maxima, postcomp DSC caps, scaler caps); `DP_GET_CAPS` values (max rate HBR3, DSC encoder caps, no UHBR); the IP version 0x04000000; the DISP interrupt vector in `interrupts.toml`. Register offsets of the v03/v04 display (head timing, AWAKEN, RG, USERD) as a `nvdisplay_v04` module shared by later chips |
| Driver version (`src/ver/v610.rs`) | control ids and parameter layouts of the ~25 DVI-path and ~12 DP-path controls, `GET_STATIC_INFO`, `IS_MODE_POSSIBLE`, the channel-pushbuffer RPC; method numbers of C57D/C57E/C57A used by the decoder |
| Generic (`src/display/`) | `model.rs` (the structs above), `timer.rs`, `methods.rs` (decoder), `ctrl.rs` (NV0073/NVC372 answers from the model), `edid.rs` (generator, port of `nvrm_edid.c` plus CTA/T7VTDB), `dpcd.rs` and `aux.rs` (native and I2C-over-AUX, DEFER/NACK), `api.rs` (runtime hotplug/EDID socket) |

### 5.3 What changes between DVI-D and DP

- **Only the connector layer changes.** Heads, windows, the method decoder, notifiers,
  semaphores, the timer and interrupts are identical.
- **Per connector, the kind switches:**
  - OR protocol `SINGLE_TMDS_A` → `DP_A`;
  - connector type `DVI_D` → `DP_EXT`;
  - DFP flags: signal DP;
  - the EDID moves from `GET_EDID_V2` to I2C-over-AUX, and may keep the CTA extension;
  - new: `DP_GET_CAPS`, the DPCD and the DP controls of section 1.3;
  - hotplug adds the DP IRQ notifier.
- **Kept as a runtime option:** a `DviD` connector kind for regression and first-light runs.

### 5.4 Phased plan

| Phase | Content | Agent-days | Milestone (falsifiable) |
|---|---|---|---|
| A-D | as display-connector.md §4: one DVI-D head, 1080p60, Weston/sway/Mutter paced at the mode rate, gates adapted | 9-14 | frame callbacks 60.0 ± 0.5 Hz; GNOME with colord, 0 `Page flip failed` |
| E. Multi-head DVI-D | N ≤ 4 connectors and heads, per-head timers, window ownership, IMP budget model, dual-link option (330 MHz) | 2-3 | `modetest` shows 4 connectors/4 CRTCs; sway with 4 outputs at 60/50/75/60 Hz, each output's callbacks at its rate ± 0.5 Hz; IMP refuses a combination over budget |
| F. DP SST | `DP_GET_CAPS`, DPCD model, AUX engine (native + I2C EDID), `DP_CTRL` training, link config/stream/MSA/lane controls, HPD long/short pulse | 3-5 | `card0-DP-1..4` connected; dmesg without DP-library assertions; 4K120 8 bpc (1075.8 MHz) validates **without** DSC and 4K144 does **not** while DSC caps are off; link stays trained across 1000 flips (no retrain in the AUX trace) |
| G. 4K144 | DSC caps (GPU and DPCD), accept `HEAD_SET_DSC_*` and DPCD 0x160/FEC, EDID CTA ext with T7VTDB, `HEAD_CLK_CAP`, timer tuning and jitter measurement | 2-3 | 4K144 listed and set on one head with `bEnableDsc` in the IMP trace; sway callbacks 144.0 ± 1 Hz; p99 tick-to-flip-event < 0.5 ms, zero lost completions over 10 min; Sunshine 4K120 HEVC stream from the guest (after E5d) |
| H. Runtime display API | control socket: add/remove head, set EDID/mode list, force preferred mode; hotplug events | 1-2 | a new mode appears in the guest's mode list within 1 s of the API call, without a guest reboot |
| **A-H total** | | **17-27** | 4 DP heads, each up to 4K144 |
| optional | VRR (2-3), low-rate host console sink (2-4, operator's word for CE use), DP 10 bpc/HDR (2-4), HDA audio function (5-10, not recommended), MST (5-10, not recommended) | | |

**Order with respect to the rest.** After CUDA (P12). Phases A-D before anything here. E and
F are independent and either can go first: E reuses the DVI path and is the cheaper proof
that the model is per-head clean.

### 5.5 Top risks

1. **DP library strictness.** 33 000 lines of C++ that read DPCD in many places. An
   inconsistent answer shows up as a retrain loop, "link lost" or a silently pruned mode, not
   as an error.
   - Mitigations: a DPCD trace in the device; `DP_PRINTF` output (the library's debug level);
     modelling the sink on one real DP 1.4 monitor's DPCD dump (a host read of a monitor's
     DPCD is a host GPU read and needs the operator's word); nouveau r535 as the second
     reading of the GSP boundary.
2. **Unknown real caps values** (`HEAD_CLK_CAP`, the exact `SYS_CAP`, DSC buffer sizes). A
   value below what 4K144 needs pushes NVKMS into 2Heads1OR, which halves the head count.
   - Mitigations: choose values that are consistent and labelled `synthesized`; replace them
     with captured values after R1/R3.
3. **Cadence at 144 Hz on a busy desktop host.** 6.94 ms leaves little slack, and the host
   GPU and CPU also serve the operator's desktop. This is a measurement, not a design risk;
   phase G gates on it.
4. **NVENC and VRAM, not the display, limit the product.**
   - NVENC: one 4K144 stream per TU106, shared with the host.
   - VRAM: 161 MiB per 4K head. The multi-head reserve is extrapolated, not measured.
   - Mitigation: the device's IMP budget and the display reserve become profile settings, and
     documented limits per profile ("4GB profile: 2 x 4K144 or 4 x 1440p144").
5. **Per-version drift.** The DP controls add about 12 layouts to the version ABI (D14, P9).
   Mitigation: tests in `v610.rs` against the bindings, as for the other controls.

## Sources

- Prior studies: [display-connector.md](display-connector.md), [display-path.md](display-path.md),
  [kickoff.md](kickoff.md), README P12/P13, D9, D14; [versatility.md](versatility.md).
- Journal (read only): `../Leandro-Journal/claude/research-vram-display-reserve.md` (§4-5 reserve
  formula and measurements, §2 vGPU type fields and FRL, §4 Wayland on vGPU),
  `research-vram-profiles.md`.
- Vendor: files and lines cited inline under `OGK/`, `NVKMS/`, `DPLIB/`, `HDMIPKT/`, `CLS/`,
  `CTRL/`; nouveau `nvkm/engine/disp/r535.c`, `tu102.c` (linux-6.12).
- Arithmetic: CVT-RB2 and link budgets computed with a short script in the session scratchpad
  (`linkbudget.py`).
- Web (2026-09-23):
  [NVIDIA vGPU 19.0 Virtual GPU Types Reference](https://docs.nvidia.com/vgpu/19.0/grid-vgpu-user-guide/virtual-gpu-types-grid-reference.html)
  (L4/A16 Q/B/A tables),
  [NVIDIA vGPU 16.0 user guide](https://docs.nvidia.com/vgpu/16.0/grid-vgpu-user-guide/index.html)
  (mixed-display rules),
  [NVENC Application Note, Video Codec SDK 13.0](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.0/nvenc-application-note/index.html)
  (Turing RTX 8000 1080p FPS per preset).

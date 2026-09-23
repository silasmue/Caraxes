<!-- SPDX-License-Identifier: MIT -->
# Display phases A and B: a DisplayPort monitor on the stock driver (gate D3)

Run 2026-09-23 on vm0, branch `worktree-agent-a6f9005032084a629` (from checkpoint `66682ae`).
Plan: phases A and B of [display-connector.md](display-connector.md) §4 with decision D15
(DisplayPort SST), the checklist A0-A12/B1-B18 of [display-adapter.md](display-adapter.md) §5.
Evidence: [../../caraxes/runs/display-ab-2026-09-23/](../../caraxes/runs/display-ab-2026-09-23/)
(commands word for word in `commands.md`). Nothing ran on the physical GPU. The display is pure
emulation. The host GPU was reached only through the existing unprivileged host RM client
(nvml forwarding) and the gate's read-only `nvidia-smi -q`.

## Outcome

**Gate D3 passes.** With `--display`, the stock 610.57.04 guest (`nvidia`, `nvidia-modeset`,
`nvidia-drm modeset=1 fbdev=0`, no guest patch) shows one connected DisplayPort connector:

```
/dev/dri/card0: 1 connector(s), 4 crtc(s), 1 encoder(s), max 65535x65535
connector 127: DP-1 connected, 530x300 mm, 6 mode(s), encoder 0
  1920x1080    1920x1080   60.000 Hz  clock  148500 kHz  htotal 2200 vtotal 1125  preferred
  1920x1080    1920x1080   59.939 Hz  clock  148350 kHz  htotal 2200 vtotal 1125
  1920x1080    1920x1080   50.000 Hz  clock  148500 kHz  htotal 2640 vtotal 1125
  1280x720     1280x720    60.000 Hz  clock   74250 kHz  htotal 1650 vtotal  750
  1280x720     1280x720    59.943 Hz  clock   74180 kHz  htotal 1650 vtotal  750
  640x480       640x480    59.940 Hz  clock   25175 kHz  htotal  800 vtotal  525
```

What the gate checked:
- `/sys/class/drm/card0-DP-1/status` is `connected`, `enabled` is `disabled`: no mode is set.
- The `edid` file (256 bytes) is byte-identical to the device's EDID. The unit test
  `display::edid::tests::edid_is_what_the_guest_exposed` compares the two.
- `edid-decode --check` on the guest's file: `EDID conformity: PASS`.
- The mode list holds 1920x1080 at 60 and 50 Hz and 1280x720 at 60 Hz. The 59.94 Hz twins come
  from the CTA VICs 16 and 4. 640x480 is the established timing.
- The deliberately impossible mode is absent: CTA VIC 201, 7680x4320 at 120 Hz, 4752 MHz. It is
  above the HBR3 x4 budget (1080 MHz at 24 bpp) and, at 8 bpp, even above what DSC could carry
  (3240 MHz).
- Run b3's device trace has no core `UPDATE` while the driver loads and detects. Zero controls
  are labelled "unanswered".
- The D0 baseline is unchanged after each phase: `tools/caraxes-gate.sh nvml` passes 11/11 and
  `tools/caraxes-vm.sh uvm` registers the GPU (`rmStatus 0x0`). This held before A (display off),
  after A and after B.

**Phase A passes:**
- `RmInitAdapter succeeded!` with KernelDisplay present.
- The fuse reads 0.
- The init controls come in the order of the checklist (table below). The display instance
  memory lands at FB 0xF75D0000, size 64 KiB, exactly where [fb-model.md](fb-model.md) §3.2
  predicted it.
- The WPR meta is unchanged: `vgaWorkspace 0xfff00000`, because `NV_PDISP_VGA_WORKSPACE_BASE`
  reads STATUS invalid.
- No control is unanswered. The guest log differs from the headless baseline by one line,
  `nv_acpi_ddc_method LCD not found`.

## What was built

| Where | What |
|---|---|
| `crates/dp-sink/` | The DP sink of package A, moved from `caraxes/dp-sink` into the workspace (same code). New: a `! train <bw> <lanes> [ef]` transcript action for trainings behind a control. The hand-written golden transcript was replaced by the guest capture `tests/transcripts/nvidia-610-guest-capture.txt` (85 request/reply pairs). The capture EDID is in `caraxes-dp-monitor.edid.bin` |
| `crates/vfio-user-nvgpu/src/display/mod.rs` | The display in the GSP role. Chip and board data, per-connector state (`DpSink`, EDID override, last answer, hotplug masks), every control of the tables below, the display allocations, and the BAR0 side: fuse, VGA workspace register, caps page, USERD PUT/GET, the core ARMED mirror, cursor `Free`. The decoder walks the core pushbuffer from GET to PUT, keeps the assembly state and copies it to ARMED at `UPDATE`. It moves GET to PUT on every channel |
| `.../display/aux.rs` | The NV0073 `DP_AUXCH_CTRL` adapter onto the sink: size minus one in the request, `bAddrOnly` tested first, bytes transferred in the reply, sink NACK/DEFER returned as `NV_OK` plus `replyType`, malformed requests as `NV_ERR_INVALID_ARGUMENT`, no sink as `replyType` TIMEOUT |
| `.../display/edid.rs` | The monitor EDID: base block with DTDs for 1080p60, 720p60 and 1080p50 plus the name; CTA extension with VICs 16, 4, 31 and 201 |
| `.../src/ver/v610/display.rs`, `display-layouts.txt` | Version-bound control numbers, struct sizes and offsets. A test checks them against gcc-measured `sizeof`/`offsetof` from the 610.57.04 SDK headers (generator `caraxes/runs/display-ab-2026-09-23/abi/gen.py`) |
| `chips/tu106/display.toml` | Synthetic chip data: IP v04.00, 4 heads, 8 windows, 4 SORs, 81 channels, I2C port 16 (none), VGA workspace invalid, interrupt vector 156, the `[link]` profile (HBR3, 4 lanes, DSC, FEC), and the NVC573 caps page register by register (Turing reset values where the headers have them) |
| `boards/rtx2070.toml` `[display]` | One connector: displayId 0x100, SOR0, index 0, kind DP |
| shared files (small) | `ctrl.rs`: a `display` field, the alloc and control hooks, one test count. `bar0.rs`: the display registers are consulted before the boot model. `board.rs`: the optional `display` table. `main.rs`: `--display`. `boot.toml`: fuse constant 0 (the chip fact; the display model answers 1 without `--display`). `interrupts.toml`: `DISP` at vector 156 (leaf 4). `v610.rs`: `mod display`, `MC_ENGINE_IDX_DISP`. `tools/caraxes-vm.sh`: `CARAXES_DISPLAY=1`. `chip/tu106.rs`: one test row |
| `crates/vfio-user-nvgpu/guest/drm-modes.c` | A libdrm-free DRM readout for the guest (connectors, status, modes with clock and refresh) |

Headless stays the default. Without `--display` the fuse reads 1 and the display answers nothing,
so the E0-E5 runs and the CUDA work see the device they had.

## Design choices the runs confirmed

1. **Fuse and layout.** `NV_PDISP_VGA_WORKSPACE_BASE` (0x625F04) reads 0, i.e. STATUS invalid.
   `kgspCalculateFbLayout_TU102` then keeps the VGA workspace at `fbSize - 1 MiB`
   (`kernel_gsp_tu102.c:787-802`), so the WPR layout is the headless one. Only the CPU-RM
   reserved block grows by the 64 KiB instance memory. The guest computes that itself, and
   `kbusVerifyBar2` passes (`RmInitAdapter succeeded!`).
2. **One link profile.** `display.toml [link]` feeds every link answer:
   - `DP_GET_CAPS.maxLinkRate` = 4 (8.1 Gbit/s), `dpVersionsSupported` 1.2 and 1.4, FEC, and
     the DSC encoder fields;
   - `DFP_GET_INFO` `DP_LINK_BW` = 4, `SIGNAL` DP, `LANE` quad;
   - `SPECIFIC_GET_PCLK_LIMIT` = 1 080 000 kHz;
   - the sink profile (dp-sink `dp14_hbr3_x4_dsc`): DPCD 0x2200 reads `14 1e c4 ...`;
   - `DP_CTRL` training and `DP_GET_LINK_CONFIG`.

   The unit test `link_answers_agree_on_one_profile` checks that they agree. In the guest the
   library trained HBR3 x4 (`DP_CTRL data 0x1e04`) and powered the link down again after
   detection.
3. **EDID state per display.** `SET_EDID_V2` with size 0 clears the override. `GET_EDID_V2`
   returns the override unless the request asks for RAW, and `COPY_CACHE` repeats the last
   answer. In b3 the EDID was written once right after the AUX read, then NVKMS ran the
   ping-pong twice (clear, write 256 bytes, read 256 bytes back): 5 `SET_EDID_V2`, 2
   `GET_EDID_V2`. Every write was identical to the sink EDID.
4. **The DisplayID probe.** The library probes DisplayID 20 times in a row: a segment write to
   0x30, then a MOT write at 0x52. Each probe gets `NV_OK` with `replyType` I2C NACK. The library
   then falls back to the EDID at 0x50. No DDC ping at size 0 was sent, so the encoding trap of
   the dp-sink README section 6 did not occur.
5. **Channels.**
   - Core, 8 window, 8 window-immediate and 4 cursor channels were allocated. Each has its
     `CHANNEL_PUSHBUFFER` RPC: sysmem, 4 KiB, `valid` 1. The cursor channels have `valid` 0 (PIO).
   - The caps page was read once. Precomp window caps were read for all 32 windows, twice.
   - PUT/GET are followed on every channel. GET moves to PUT, so `GET_CHANNEL_INFO` says IDLE.

## The route (gate D2): every display RPC and control, in order

Phase A (`a1-route.txt`, probe `display-a1`), all `NV_OK`:

| # | Request | Answer | Label |
|---|---|---|---|
| 1 | `NV0073 SYSTEM_MAP_SHARED_DATA` 0x730151, `bMap` 0 | unmap of the previous driver instance | guest |
| 2 | `INTERNAL_DISPLAY_GET_IP_VERSION` 0x20800a4b | 0x04000000 | synthesized |
| 3 | `GSP_RM_ALLOC NV04_DISPLAY_COMMON` | recorded | synthesized |
| 4 | `INTERNAL_DISPLAY_GET_STATIC_INFO` 0x20800a01 | 4 heads, windows 0xFF, `feHwSysCap` 0xF, i2cPort 16, 81 channels | synthesized |
| 5 | `INTERNAL_INIT_BRIGHTC_STATE_LOAD` 0x20800ac6 | ack (no panel) | synthesized |
| 6 | `INTERNAL_SET_STATIC_EDID_DATA` 0x20800adf | ack (`tableLen` 16, the guest's ACPI list) | synthesized |
| 7 | `INTERNAL_DISPLAY_WRITE_INST_MEM` 0x20800a49 | recorded: FB 0xF75D0000, 64 KiB, FBMEM | guest |
| 8 | `SYSTEM_MAP_SHARED_DATA`, `bMap` 1 | recorded: FB 0xF75CE000, 0x20 bytes | guest |
| 9 | `GSP_RM_ALLOC NV04_DISPLAY_COMMON` (`nv->rmapi.hDisp`, A11) | recorded | synthesized |
| 10 | `SYSTEM_GET_SUPPORTED` 0x730107 (A12) | 0x100 | board |
| 11 | `SPECIFIC_OR_GET_INFO` 0x73028b | SOR0, `DP_A`, chip, static | board |
| 12 | `SPECIFIC_GET_I2C_PORTID` 0x730211 | `ddcPortId` 0 (no Linux I2C adapter) | board |

`SET_IMP_INIT_INFO` (0x20800a54, A7) never came. `kdispStateInitLocked` calls
`kdispImportImpData_HAL`, and the TU106 binding evidently does not send it (inferred from
the trace, not read in the generated HAL table). `WRITE_INST_MEM` came where it was expected.

Phase B (`b3-route.txt`, loading `nvidia-modeset` and `nvidia-drm` with no DRM client), in order
of first appearance, with counts:

| Request | n | Answer | Label |
|---|---|---|---|
| `GSP_RM_ALLOC NV04_DISPLAY_COMMON` | 1 | recorded | synthesized |
| `SYSTEM_GET_CAPS_V2` 0x730101 | 1 | `CROSS_BAR_SUPPORTED` | synthesized |
| `SYSTEM_GET_NUM_HEADS` 0x730102 | 1 | 4 | synthesized |
| `SPECIFIC_GET_ALL_HEAD_MASK` 0x730287 | 2 | 0xF (NVKMS, then the DP library) | synthesized |
| `SPECIFIC_GET_VALID_HEAD_WINDOW_ASSIGNMENT` 0x7302ad | 1 | windows 2h, 2h+1 on head h | synthesized |
| `SPECIFIC_OR_GET_INFO` 0x73028b | 2 | SOR0 `DP_A` | board |
| `SPECIFIC_GET_CONNECTOR_DATA` 0x730250 | 1 | 1 connector, `DP_EXT`, index 0 | board |
| `DP_GET_CAPS` 0x731369 | 2 | HBR3, DP 1.2+1.4, no MST, FEC, DSC. The second request carries `sorIndex` 0xFFFFFFFF (before SOR assignment) | link profile |
| `DP_SET_MANUAL_DISPLAYPORT` 0x731365 | 1 | ack | synthesized |
| `DFP_GET_INFO` 0x731140 | 2 | flags 0x18001B | link profile |
| `SPECIFIC_GET_TYPE` 0x730240 | 1 | DFP | board |
| `SYSTEM_GET_BOOT_DISPLAYS` 0x73011e | 1 | 0 | board |
| `SPECIFIC_GET_PCLK_LIMIT` 0x73028a | 3 | 1 080 000 kHz | link profile |
| `GSP_RM_ALLOC NVC372_DISPLAY_SW` | 1 | recorded | synthesized |
| `INTERNAL_DISPLAY_PRE/POST_UNIX_CONSOLE` 0x20800a76/77 | 1 each | ack | synthesized |
| `GSP_RM_ALLOC NVC570_DISPLAY` | 1 | recorded | synthesized |
| NV5070 `SYSTEM_GET_CAPS_V2` 0x50700709 | 1 | 0 | synthesized |
| `INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER` 0x20800a58 | 21 | recorded per channel | guest |
| `GSP_RM_ALLOC NVC57D` / `NVC57E` x8 / `NVC57B` x8 / `NVC57A` x4 | 21 | USERD page per channel | synthesized |
| `SYSTEM_GET_ACTIVE` 0x73010c | 4 | 0 per head | synthesized |
| NVC370 `GET_LOCKPINS_CAPS` 0xc3700201 | 1 | no stereo pin | synthesized |
| `SYSTEM_GET_CONNECT_STATE` 0x730108 | 1 | 0x100 | sink |
| `DFP_ASSIGN_SOR` 0x731152 | 1 | 0x100 on SOR0, single | board |
| `DP_AUXCH_CTRL` 0x731341 | 85 | the transcript below | sink |
| `SPECIFIC_SET_EDID_V2` 0x730246 | 5 | override set or cleared | guest-derived |
| `DP_NOTIFY_LT` 0x73138f | 3 | ack | synthesized |
| `DP_CTRL` 0x731343 | 2 | HBR3 x4 trained, then lane count 0 (power down) | sink |
| `SYSTEM_GET_INTERNAL_DISPLAYS` 0x730116 | 1 | 0 | board |
| `SYSTEM_QUERY_DISPLAY_IDS_WITH_MUX` 0x73013d | 1 | `NOT_SUPPORTED` (no mux) | board |
| `SPECIFIC_GET_BACKLIGHT_BRIGHTNESS` 0x730291 | 2 | `NOT_SUPPORTED` (no panel) | board |
| `SPECIFIC_GET_EDID_V2` 0x730245 | 2 | 256 bytes from the override | guest-derived |
| `SPECIFIC_IS_DIRECTMODE_DISPLAY` 0x7302a7 | 1 | no | synthesized |
| `DP_GET_LINK_CONFIG` 0x731360 | 2 | 0 lanes, bw 0 (link down after detection) | sink |
| NVC372 `IS_MODE_POSSIBLE` 0xc3720101 | 14 | possible | synthesized |

Checks against display-adapter.md §1.3:
- Local rows never reached the device: `EVENT_SET_NOTIFICATION` of NVC370, `DP_SET_MSA_PROPERTIES`
  (0x73136a) and `QUERY_UEFI_DISPLAY_BANDWIDTH`.
- The internal hotplug control 0x730401 did not appear, because no hotplug was raised.
- Cached rows appeared once per input: `GET_SUPPORTED` appears in phase A only (cached since
  A12), and `DP_GET_CAPS` once per `sorIndex`.
- Every answer carries a D9 label. None is host-forwarded.

The AUX transcript of b3 (`b3-aux-transcript.txt`, the dp-sink golden file) in short:
- detection reads: `nr 00e 1 -> 80`; the DP-tunneling registers `e000d`/`e0030` (read 0, one
  write); `nr 2200 16` (DPCD 1.4 caps); sink count `2002`; MST cap `020`; DPRX feature list
  `2211` and `070`; the ESI block `2002`-`200c`; `nw 111 04`;
- DisplayID probe: 20 x (`iwm 30 00` ack, `iwm 52 00` I2C NACK);
- base block: `iwm 50 00`, 8 x `irm/ir 50 16`; extension block: `iwm 50 80`, 8 more reads;
- `nr 000 1`; training (`! train 1e 4 ef`); power `600`; HDCP `6921d`, `68028`, `6802a`
  (all 0); DSC caps `060` (16); `0b0`/`0b1`/`02e`; FEC `090`;
- `nw 600 02` (D3), then `DP_CTRL` lane count 0.

`cargo test -p dp-sink` replays all 85 pairs exactly.

## What the phase B runs taught (runs b1 and b2)

- **The guest image starts gdm3.** Its Xorg (modesetting driver) opened card0 about 25 s after
  `nvidia-drm` loaded and set a mode. The trace (`b1-gdm-modeset-route.txt`) shows what phase
  C will face:
  - `HEAD_ROUTING_MAP` and `IS_MODE_POSSIBLE`;
  - `DISPLAY_CHANGE`, `DFP_ASSIGN_SOR`, `SET_SOR_FLUSH_MODE`;
  - `DP_CTRL` training, which failed with `NotPowered`: the library had left the sink in D3.
    Fixed: the device now puts the sink in D0 before training, as GSP's training does;
  - four controls that stay unanswered in phase B: `DP_SET_STEREO_MSA_PROPERTIES` (0x731378),
    `DP_SET_MSA_PROPERTIES_V2` (0x731381), `SPECIFIC_SET_OD_PACKET` (0x730288) and
    `SYSTEM_VRR_DISPLAY_INFO` (0x73012c);
  - a core `UPDATE` with 400 methods, window `UPDATE`s, then `Lost display notification` and
    `Flip event timeout on head 0`, because no notifier is written yet.

  gdm3 was therefore disabled in vm0 (`systemctl disable gdm3`, default target
  `multi-user.target`); experiments.md records it.
- **Opening and closing card0 as DRM master** (drm-modes, run b3 after the D3 capture) makes
  NVKMS commit its initial window mapping: one core `UPDATE`, 66 methods, **no raster method**.
  The methods are `WINDOW_SET_CONTROL(w)` owners 0,0,1,1,2,2,3,3, the usage bounds,
  `SET_CONTEXT_DMA_NOTIFIER` and `SET_NOTIFIER_CONTROL`. The ARMED mirror took the owners. It
  still logs `Lost display notification (0:0x00000000)`: NVKMS waits for the core notifier,
  which the device does not write yet. This is not a modeset; D3's "no modeset" holds for
  both the load and the readout.
- **Guest disk state.** `tools/caraxes-vm.sh down` kills Cloud Hypervisor without a guest
  shutdown. The first four-module build and its install were lost: every `.ko` read as zeros
  after the restart. Run `sync` in the guest before `down`.

## What phase C needs (first modeset, flips)

1. **Core notifier.** Resolve the `SET_CONTEXT_DMA_NOTIFIER` handle through RAMHT in the
   instance memory at FB 0xF75D0000, which the device now knows from `WRITE_INST_MEM`. Write
   FINISHED at `UPDATE`, raise AWAKEN if asked, and set `FE_CORE_HEAD_STATE`
   (display-adapter.md §4.3-4.4). This removes `Lost display notification`.
2. **Window channel completion.** Semaphores, notifier and the device timer
   (display-connector.md §2.2), so that `Flip event timeout` goes away.
3. **The four modeset-path controls.** `SET_MSA_PROPERTIES_V2` and `SET_STEREO_MSA_PROPERTIES`
   (accept), `SET_OD_PACKET` (accept, no infoframes on DP), and `VRR_DISPLAY_INFO` (not VRR).
4. **IMP.** `IS_MODE_POSSIBLE` answers "possible" for everything today. For DSC and 2Heads1OR it
   should check the per-head pixel clock against `HEAD_CLK_CAP` (1330 MHz).
5. **The display interrupt.** Vector 156 is in the kernel table. Raise it for `LAST_DATA` and
   AWAKEN from the timer.
6. **A guest without gdm3** (done on vm0), or a compositor started on purpose.

## Not verified

- The closed GSP's own answers: no passthrough trace exists (R1/R3). Every value above is
  either from headers, from NVKMS's consumers, or a choice.
- `edid-decode` in the guest: the guest has no copy, so the file was checked on the host.
- `modetest`: absent in the guest. `drm-modes.c` reads the same uapi.

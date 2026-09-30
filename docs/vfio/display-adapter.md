<!-- SPDX-License-Identifier: MIT -->
# The NVIDIA display adapter: control dispatch, EDID lifecycle, classes, channels

Package B of the side assignment ([briefing-display-nova.md](../history/caraxes-briefing-display-nova-2026-09-23.md) §2, §3,
§7, §10, §12; P16 in [README.md](../history/caraxes-record-2026-09-28.md)). Written 2026-09-23. Source survey only: no VM,
no host GPU access, no change to the device crate. It inventories what the guest driver
sends for **one DisplayPort SST connector** (D15) and where each request terminates, so the
device knows which controls it must answer, which state it must keep, and in which order
the first unanswered request appears.

It builds on [display-path.md](display-path.md) (classes, RPCs, BAR0 pages, instance memory),
[display-connector.md](display-connector.md) (phases A-D, the device timer, flip pacing) and
[display-scaling.md](display-scaling.md) (DP as the connector, DPCD, link arithmetic) and
does not repeat them.

Source of truth: `vendor/open-gpu-kernel-modules` at 610.57.04, commit
`e4a5faa2567f28c8eabe0ebb6422b6d0abcf37eb`. Pointers: `OGK/` is that tree's `src/`;
`NVKMS/` is `OGK/nvidia-modeset/src`; `DPLIB/` is `OGK/common/displayport/src`; `KRM/` is
`OGK/nvidia/src/kernel`; `KDISP/` is `KRM/gpu/disp`; `GEN/` is `OGK/nvidia/generated`;
`CTRL/` is `OGK/common/sdk/nvidia/inc/ctrl`; `CLS/` is `OGK/common/sdk/nvidia/inc/class`.
nouveau (second reference) is `linux-6.12/drivers/gpu/drm/nouveau` in the session scratchpad.
Struct sizes come from compiling the 610.57.04 SDK headers with the host gcc (x86_64, the
guest ABI). Tags: [V] read in source, [N] read in nouveau r535, [I] inferred, [O] open (only
a trace of the real GSP or a guest run can settle it).

**Short answer.**
- **AUX terminates at the device, as a `GSP_RM_CONTROL`.** `NV0073_CTRL_CMD_DP_AUXCH_CTRL`
  has no body in the guest kernel. Its export entry is `PRIVILEGED | ROUTE_TO_PHYSICAL |
  COPYOUT_ON_ERROR` (0x844). `rmresControl_Prologue_IMPL` forwards every `ROUTE_TO_PHYSICAL`
  control of a GSP client verbatim, as a flat struct (no display interface is FINN-serialized)
  (section 1.1).
- **Of the controls NVKMS and its DP library send for an SST DP connector, 36 NV0073, 3 NV5070,
  4 NVC370 and 2 NVC372 controls reach the device.** Two NV0073 controls are kernel wrappers
  around an internal control that also reaches it. NVC370 `EVENT_SET_NOTIFICATION`,
  `DP_SET_MSA_PROPERTIES` (not exported, refused locally, tolerated) and two Tegra/DP2
  helpers stay in the guest. **No control is served by MMIO** on this path. BAR0 carries the
  channels, the caps page, the interrupts and the counters, not the controls (section 1.3).
- **The EDID crosses the device three times per detection:** first over AUX as I2C-over-AUX
  reads at address 0x50 (8 reads of 16 bytes per block). Then NVKMS writes it back with
  `SET_EDID_V2` and reads it again with `GET_EDID_V2`, after first clearing it with
  `SET_EDID_V2` and `bufferSize` 0. The device must keep the sink EDID and an override per
  displayId. It must treat `bufferSize` 0 as "clear". It must answer `GET_EDID_V2` with
  exactly what was written (section 2).
- **The class list is not the device's to answer.** `GET_CLASSLIST` is local and comes from the
  guest's static TU106 table, which already lists all C57x classes and NVC372. The device has
  no class table. What removes the display today is the fuse `0x21C04 = 1` (`boot.toml:50`),
  and after that the unanswered `GET_IP_VERSION`. `NVC573` is allocated locally (no RPC) and
  read from BAR0 0x640000 (section 3).
- **The first missing behaviour** after the fuse reads 0 is `INTERNAL_DISPLAY_GET_IP_VERSION`.
  The device answers it "unanswered / NOT_SUPPORTED" today, which silently removes the display
  again. Once that control is answered, `INTERNAL_DISPLAY_GET_STATIC_INFO` and three more
  internal controls become fatal if missing (section 5).
- **Riskiest ABI details:** (1) the AUX size and status layering, (2) the EDID ping-pong with
  `bufferSize` 0, (3) link-training coherence across four controls and the DPCD, some of them
  cached for good by the guest kernel (section 7).

---

## 1. Where AUX terminates, and every display control on the DP path

### 1.1 The call chain [V]

| Step | Where | What happens |
|---|---|---|
| DP library builds the request | `DPLIB/dp_evoadapter.cpp:994-1144` `EvoAuxBus::transaction` | fills `NV0073_CTRL_DP_AUXCH_CTRL_PARAMS`; `size = sizeRequested - 1`, but 0 for a 0-byte request; `bAddrOnly` for a 0-byte I2C write; up to 3 tries while `retryTimeMs` is non-zero |
| NVKMS bridge | `NVKMS/dp/nvdp-evo-interface.cpp:41-63` `rmControl0073` / `rmControl5070` | `nvRmApiControl` on `displayCommonHandle` (NV04_DISPLAY_COMMON) or `displayHandle` (NVC570) |
| NVKMS to RM | `NVKMS/nvkms-rmapi-dgpu.c:81-101` `nvRmApiControl` | `NV04_CONTROL` through `nvkms_call_rm` (kernel client, privileged) |
| RM lookup | `OGK/nvidia/src/libraries/resserv/src/rs_resource.c:118-146` `resControlLookup_IMPL` | not in the export table: `NV_ERR_NOT_SUPPORTED`; `paramsSize` different from `sizeof`: `NV_ERR_INVALID_PARAM_STRUCT`. So the device always sees exact sizes |
| Display prologue | `KDISP/disp_objs.c:283-365` `dispapiControl_Prologue_IMPL` | DispCommon: routes by device; NvDispApi (C570) and DispSwObj (C372): requires `NVC370_/NVC372_CTRL_CMD_BASE_PARAMS` first, picks the subdevice |
| Routing | `KRM/rmapi/resource.c:254-297` `rmresControl_Prologue_IMPL` | GSP client plus `RMCTRL_FLAGS_ROUTE_TO_PHYSICAL`: `NV_RM_RPC_CONTROL`, then `NV_WARN_NOTHING_TO_DO`, so no kernel body runs |
| RPC | `KRM/vgpu/rpc.c:10740-10910` | `GSP_RM_CONTROL` with `hClient`, `hObject`, `cmd`, `paramsSize`, flat params; `COPYOUT_ON_ERROR` sets `RMAPI_RPC_FLAGS_COPYOUT_ON_ERROR` (:10802), so the reply params are copied back even when `rpc_params->status` is an error (:10871); a successful reply of a `CACHEABLE` control is stored in the kernel's control cache (:10900-10910) |
| Serialization | `KRM/rmapi/rmapi_finn.c:66-110`, `OGK/nvidia/interface/rmapi/src/g_finn_rm_api.c:610-647` | FINN serializes no NV04_DISPLAY_COMMON, NV5070, NVC370 or NVC372 interface: every display control travels as its raw C struct |

The flags come from the export tables in `GEN/g_disp_objs_nvoc.c` (for example
`DP_AUXCH_CTRL` at :5040-5054). The flag values are in `OGK/nvidia/inc/kernel/rmapi/control.h:167-344`:
`PRIVILEGED` 0x4, `NON_PRIVILEGED` 0x8, `ROUTE_TO_PHYSICAL` 0x40, `COPYOUT_ON_ERROR` 0x800,
`CACHEABLE_BY_INPUT` 0x20000, `PERSISTENT_CACHEABLE` 0x800000. "Local" below means the export
entry has no `ROUTE_TO_PHYSICAL` and a body exists under `KDISP/`.

nouveau confirms the boundary independently [N]. `r535_dp_aux_xfer`
(`nvkm/engine/disp/r535.c:1057-1100`) sends the same control through its GSP RM-control path
and reads `replyType` and `size` back on success. `r535_dp_train_target` (:955-1015) does the
same for `DP_CTRL`.

### 1.2 `NV0073_CTRL_CMD_DP_AUXCH_CTRL` (0x731341) in detail

Struct `NV0073_CTRL_DP_AUXCH_CTRL_PARAMS`, 48 bytes (`CTRL/ctrl0073/ctrl0073dp.h:151-196`):

| Offset | Field | Direction | Notes |
|---|---|---|---|
| 0 | `subDeviceInstance` | in | 0 |
| 4 | `displayId` | in | one bit |
| 8 | `bAddrOnly` (NvBool, 3 bytes padding) | in | I2C address-only (0-byte write); `size` is 0 then |
| 12 | `cmd` | in | bit 3 `TYPE` (0 I2C, 1 AUX), bit 2 `I2C_MOT`, bits 1:0 `REQ_TYPE` (0 write, 1 read, 2 write-status) |
| 16 | `addr` | in | 20 bits: DPCD address, or the 7-bit I2C address |
| 20 | `data[16]` | in/out | write payload / read result |
| 36 | `size` | in/out | **in: count minus one** (0 means 1 byte, 15 means 16); **out: bytes actually transferred** |
| 40 | `replyType` | out | 0 ACK, 1 NACK, 2 DEFER, 3 TIMEOUT, 4 I2CNACK, 8 I2CDEFER |
| 44 | `retryTimeMs` | out | non-zero asks the caller to sleep and retry (eDP power-up) |

Result layering, as the DP library consumes it (`DPLIB/dp_evoadapter.cpp:1064-1143`) [V]:

- The call returns anything but `NV_OK` or `NV_ERR_TIMEOUT`: the library returns **nack**
  without looking at `replyType`. `NV_ERR_TIMEOUT` becomes **defer**.
- The call returns `NV_OK`: `replyType` decides. ACK copies `min(size, sizeRequested)` bytes
  for a read. NACK, I2CNACK and TIMEOUT mean nack. DEFER and I2CDEFER mean defer and force
  `sizeCompleted` to 0.
- A write-status request answered `NV_ERR_NOT_SUPPORTED` counts as success.
- So the sink's NACK and DEFER must come back as **`NV_OK` plus `replyType`**. Control errors
  are only for invalid arguments. nouveau reads it the same way: status OK, then
  `ret = replyType` [N]. The control's `COPYOUT_ON_ERROR` flag still lets `replyType` come back
  on an error, and the library reports it as `pNakReason`.
- The two ambiguities of the zero encoding [V]:
  - an address-only I2C transaction also has `size` 0; the device must test `bAddrOnly` first;
  - a 0-byte I2C *read* (`sstDDCPing`, `DPLIB/dp_sst_edid.cpp:214`, unused in this tree) is
    sent with `bAddrOnly` false and `size` 0, which means a 1-byte read.
- NVKMS itself uses the control outside the library: `ReadDPCDReg` and `nvWriteDPCDReg`
  (`NVKMS/nvkms-rm.c:1463-1535`) send 1-byte native AUX transfers and require `size == 1` on
  return. They serve only the DP serializer (`nvkms-rm.c:1576-1588`, `nvkms-evo.c:9424`), not a
  normal DP connector.

The larger I2C control is `NV0073_CTRL_CMD_DP_AUXCH_I2C_TRANSFER_CTRL` (0x73137c,
`ctrl0073dp.h:2362-2415`). Its struct is 276 bytes: `addr`, `bWrite`, `data[256]`, `size` (a
plain count, not minus one). Flags 0x40: `ROUTE_TO_PHYSICAL`, kernel-privileged only.
- **NVKMS and the DP library never send it** [V: no reference under `NVKMS/` or `DPLIB/`].
- Its only caller is nvidia.ko's Linux I2C adapter layer. `RmDpAuxI2CTransfer`
  (`OGK/nvidia/arch/nvalloc/unix/src/osapi.c:3890-3945`) serves `/dev/i2c-*` traffic such as
  ddcutil on DP connectors.
- Those adapters exist only if `SPECIFIC_GET_I2C_PORTID` returns a DDC port for a DP SOR on
  the chip (`osapi.c:4320-4390`). The device answers `ddcPortId` 0 (`I2C_PORT_NONE`), and the
  control never appears [I].

### 1.3 The inventory for one SST DP connector

Scope: every NV0073, NV5070, NVC370 and NVC372 control that NVKMS (`NVKMS/*.c`) and the DP
library (`DPLIB/*.cpp` through `dp_evoadapter.cpp`) send on the connector, EDID, DP link,
head and modeset paths of the Turing HAL `nvEvoC5`. Not in scope: eDP and panels
(backlight, `GET_EDP_DATA`), MST (`TOPOLOGY_*`, `SEND_ACT`, `SET_ECF`, `CONFIG_RAD_SCRATCH_REG`,
`SINGLE_HEAD_MULTI_STREAM`, `SET_TRIGGER_*`), HDCP authentication (`HDCP_CTRL`; but see
`GET_HDCP_STATE` below), HDMI, DSI, the dynamic mux, VRR, DP2.x
(`DP2X_*`, `CALCULATE_DP_IMP` from `dp_connectorimpl2x.cpp:313`), DP compliance test patterns,
framelock and raster lock, and the DP serializer.

Routes: **GSP** means forwarded as `GSP_RM_CONTROL`; the device answers it. **Local** means
answered inside the guest kernel; the device never sees it. **Local+GSP** means a kernel body
that calls an internal control, which reaches the device. **MMIO** means served by BAR0
registers. The size is `sizeof` the params struct. "Cached" means the reply is stored in the
guest kernel's control cache (`PERSISTENT_CACHEABLE`, keyed by input): the device sees one call
per distinct input until the driver unloads.

**NV0073 (NV04_DISPLAY_COMMON)**

| Id | Control | Struct (bytes) | Route | Sent by (file:function) | Reply fields NVKMS reads |
|---|---|---|---|---|---|
| 0x730101 | `SYSTEM_GET_CAPS_V2` | `NV0073_CTRL_SYSTEM_GET_CAPS_V2_PARAMS` (2) | GSP | `nvkms-rm.c:1422` `ProbeDisplayCommonCaps` | caps table bits |
| 0x730102 | `SYSTEM_GET_NUM_HEADS` | `..._GET_NUM_HEADS_PARAMS` (12) | GSP | `nvkms-rm.c:999` `ProbeHeadCountAndWindowAssignment` | `numHeads` |
| 0x730104 | `SYSTEM_GET_SCANLINE` | `..._GET_SCANLINE_PARAMS` (20) | GSP | `nvkms-attributes.c:173` `GetScanLine` | current line, blank state (runtime attribute) |
| 0x730107 | `SYSTEM_GET_SUPPORTED` | `..._GET_SUPPORTED_PARAMS` (12) | GSP, cached | `nvkms-rm.c:426` `ProbeValidDisplays`; also nvidia.ko `osapi.c:4344` | `displayMask` |
| 0x730108 | `SYSTEM_GET_CONNECT_STATE` | `..._GET_CONNECT_STATE_PARAMS` (16) | GSP, `COPYOUT_ON_ERROR` | `nvkms-rm.c:2422` `nvRmGetConnectedDpys`; `nvkms-event.c:87` | `displayMask`; `retryTimeMs` with `NV_ERR_NOT_READY` loops |
| 0x73010b | `SYSTEM_GET_HEAD_ROUTING_MAP` | `..._GET_HEAD_ROUTING_MAP_PARAMS` (20) | GSP | `nvkms-rm.c:4504` `nvRmIsPossibleToActivateDpyIdList`, from `nvkms-modeset.c:2031` on every modeset validation | `displayMask` must come back **equal** to the request, else "not supported on this GPU" (`nvkms-rm.c:4538-4570`) |
| 0x73010c | `SYSTEM_GET_ACTIVE` | `..._GET_ACTIVE_PARAMS` (16) | GSP | `nvkms-rm.c:1308` `GetVbiosHeadAssignmentOneDisp` (per head, from `nvRMSetupEvoCoreChannel`); `dp_evoadapter.cpp:1846` `EvoMainLink::isActive` | `displayId` per head (0: nothing lit by firmware) |
| 0x730116 | `SYSTEM_GET_INTERNAL_DISPLAYS` | `..._GET_INTERNAL_DISPLAYS_PARAMS` (12) | GSP, cached | `nvkms-rm.c:5735` `nvRmRegisterBacklight`, from `nvAllocCoreChannelEvo` | internal (panel) mask: 0 |
| 0x73011e | `SYSTEM_GET_BOOT_DISPLAYS` | `..._GET_BOOT_DISPLAYS_PARAMS` (8) | GSP | `nvkms-rm.c:1373` `ProbeBootDisplays` | boot mask: 0 |
| 0x730240 | `SPECIFIC_GET_TYPE` | `NV0073_CTRL_SPECIFIC_GET_TYPE_PARAMS` (12) | GSP, cached | `nvkms-rm.c:2293` `GetLegacyConnectorType` | `displayType` DFP |
| 0x730245 | `SPECIFIC_GET_EDID_V2` | `..._GET_EDID_V2_PARAMS` (2064) | GSP | `nvkms-dpy.c:1288` `ReadEdidFromResman`; `dp_evoadapter.cpp:1754` `fetchEdidByRmCtrl` | `bufferSize`, `edidBuffer` (section 2) |
| 0x730246 | `SPECIFIC_SET_EDID_V2` | `..._SET_EDID_V2_PARAMS` (2060) | GSP | `nvkms-dpy.c:2097` `ClearCustomEdid`, `:2135` `WriteEdidToResman` | status only |
| 0x730250 | `SPECIFIC_GET_CONNECTOR_DATA` | `..._GET_CONNECTOR_DATA_PARAMS` (72) | GSP | `nvkms-rm.c:633` `AllocConnector` | `count`, `data[].type` (`DP_EXT`), `.index`, `.location`, `DDCPartners` |
| 0x730280 | `SPECIFIC_GET_HDCP_STATE` | `..._GET_HDCP_STATE_PARAMS` (12) | GSP | `dp_evoadapter.cpp:482` `configureHDCPGetHDCPState`, from `notifyAttachEnd` after **every** DP modeset, SST included (`dp_connectorimpl.cpp:4040-4060`) | `HDCP_State_*` flags: all FALSE |
| 0x730287 | `SPECIFIC_GET_ALL_HEAD_MASK` | `..._GET_ALL_HEAD_MASK_PARAMS` (8) | GSP | `nvkms-rm.c:1066`; `dp_evoadapter.cpp:163` (EvoMainLink constructor) | `headMask` (the DP library falls back to 3 on error) |
| 0x73028a | `SPECIFIC_GET_PCLK_LIMIT` | `..._GET_PCLK_LIMIT_PARAMS` (20) | GSP | `nvkms-dpy.c:809` `nvDpyProbeMaxPixelClock` | `orPclkLimit` |
| 0x73028b | `SPECIFIC_OR_GET_INFO` | `..._OR_GET_INFO_PARAMS` (56) | GSP (PRIVILEGED) | `nvkms-rm.c:465` `ProbeValidDisplays`, `:2325` `nvRmGetConnectorORInfo`, `:1249`; nvidia.ko `osapi.c:4363` | `type` SOR, `index`, `protocol` `DP_A`, `location` CHIP, `bIsDispDynamic` FALSE, dither fields |
| 0x730295 | `SPECIFIC_SET_MONITOR_POWER` | `..._SET_MONITOR_POWER_PARAMS` (20) | GSP | `nvkms-rm.c:2548` `nvRmSetDpmsEvo` | status (DPMS; stops the head timer, display-connector.md §2.2) |
| 0x7302a1 | `SPECIFIC_APPLY_EDID_OVERRIDE_V2` | `..._APPLY_EDID_OVERRIDE_V2_PARAMS` (2060) | GSP | `dp_evoadapter.cpp:1786`, only when the RM-EDID path failed and AUX succeeded (`dp_sst_edid.cpp:282-296`) | `edidBuffer` copied back (`bufferSize` must be unchanged) |
| 0x7302a4 | `SPECIFIC_DISPLAY_CHANGE` | `..._DISPLAY_CHANGE_PARAMS` (16) | GSP | `nvkms-rm.c:2220-2241` `nvRmBeginEndModeset` | status |
| 0x7302a7 | `SPECIFIC_IS_DIRECTMODE_DISPLAY` | `..._IS_DIRECTMODE_DISPLAY_PARAMS` (6) | GSP | `nvkms-dpy.c:299` `AssignIsVrHmd`, after every EDID parse | "is a VR headset": FALSE |
| 0x7302ad | `SPECIFIC_GET_VALID_HEAD_WINDOW_ASSIGNMENT` | `..._GET_VALID_HEAD_WINDOW_ASSIGNMENT_PARAMS` (36) | GSP | `nvkms-rm.c:1092` | window-to-head masks |
| 0x731140 | `DFP_GET_INFO` | `NV0073_CTRL_DFP_GET_INFO_PARAMS` (16) | GSP | `nvkms-rm.c:565` `GetDfpInfo`; `dp_evoadapter.cpp:1697` `queryAndUpdateDfpParams` | `flags`: `SIGNAL` DP, `DP_LINK_BW` 19:17 (**must be set**: 0 asserts and falls back to 1.62 Gbit/s, `dp_evoadapter.cpp:1726-1745`), `DP_PHY_REPEATER_COUNT` 0, `EMBEDDED_DISPLAYPORT` 0, `DP_FORCE_RM_EDID` (section 2.3), `DYNAMIC_MUX_CAPABLE` 0 |
| 0x731144 | `DFP_SET_ELD_AUDIO_CAPS` | `NV0073_CTRL_DFP_SET_ELD_AUDIO_CAP_PARAMS` (120) | GSP | `nvkms-hdmi.c:1151` `RmSetELDAudioCaps`, `:1306` | status |
| 0x73114e | `DFP_UPDATE_DYNAMIC_DFP_CACHE` | `..._UPDATE_DYNAMIC_DFP_CACHE_PARAMS` (24) | GSP | `dp_evoadapter.cpp:2017` `rmUpdateDynamicDfpCache`, from `notifyAttachEnd` when the head is attached (`dp_connectorimpl.cpp:3995-4010`): the sink's HDCP `BCAPS`/`BKSV` as read over AUX | status |
| 0x731150 | `DFP_SET_AUDIO_ENABLE` | `..._SET_AUDIO_ENABLE_PARAMS` (12) | GSP | `nvkms-hdmi.c:636` `SetDpAudioEnable` | status |
| 0x731152 | `DFP_ASSIGN_SOR` | `NV0073_CTRL_DFP_ASSIGN_SOR_PARAMS` (80) | GSP | `nvkms-evo.c:5455` `nvAssignSOREvo`, from `nvdp-connector.cpp:1051` (`nvDPResume`) and `:113` (`nvDPNotifyLongPulse`); `nvkms-modeset.c:2275` | `sorAssignListWithTag[s].displayMask`, `.sorType` (`nvkms-evo.c:5370-5410` `RefreshSORAssignments`) |
| 0x731341 | `DP_AUXCH_CTRL` | 48 | GSP, `COPYOUT_ON_ERROR` | section 1.2 | `size`, `replyType`, `data`, `retryTimeMs` |
| 0x731343 | `DP_CTRL` | `NV0073_CTRL_DP_CTRL_PARAMS` (28) | GSP, `COPYOUT_ON_ERROR` | `dp_evoadapter.cpp:1273-1600` `EvoMainLink::train` | status; `err` bits (`CLOCK_RECOVERY`, `CHANNEL_EQUALIZATION`, `CR_DONE_LANE`, `LINK_STATUS_DISCONNECTED`, `ENABLE_FEC`) drive the fallback; `retryTimeMs` |
| 0x731356 | `DP_MAIN_LINK_CTRL` | `..._MAIN_LINK_CTRL_PARAMS` (12) | GSP | `dp_evoadapter.cpp:1639` `configurePowerState`, from `notifyAttachBegin` (`dp_connectorimpl.cpp:3848`) | status |
| 0x731359 | `DP_SET_AUDIO_MUTESTREAM` | `..._SET_AUDIO_MUTESTREAM_PARAMS` (12) | GSP | `nvkms-hdmi.c:606` `SetDpAudioMute` | status |
| 0x731360 | `DP_GET_LINK_CONFIG` | `..._GET_LINK_CONFIG_PARAMS` (24) | GSP | `dp_evoadapter.cpp:1654` `getLinkConfig`, `:1962`; `isLinkLost` via `getCurrentLinkConfig` (`dp_connectorimpl.cpp:5521-5563`, `:8820`) | `laneCount`, `linkBW` (270 MHz units), `dp2LinkBW` (must be 0 when `linkBW` is non-zero), `bFECEnabled` |
| 0x731362 | `DP_CONFIG_STREAM` | `NV0073_CTRL_CMD_DP_CONFIG_STREAM_PARAMS` (84) | GSP | `dp_evoadapter.cpp:632` `configureSingleStream`, from `beforeAddStream` (`dp_connectorimpl.cpp:7107-7138`) | status |
| 0x731365 | `DP_SET_MANUAL_DISPLAYPORT` | 4 | GSP | `dp_evoadapter.cpp:154` (EvoMainLink constructor: "RM hands off the DP hardware") | status (asserted) |
| 0x731369 | `DP_GET_CAPS` | `NV0073_CTRL_CMD_DP_GET_CAPS_PARAMS` (64) | GSP, cached by `sorIndex` | `dp_evoadapter.cpp:279` `queryGPUCapability`, from the `ConnectorImpl` constructor (`dp_connectorimpl.cpp:143`) | `maxLinkRate` 2:0 (4 = 8.10 Gbit/s), `dpVersionsSupported`, `bIsMultistreamSupported` (FALSE), `bFECSupported`, `bIsTrainPhyRepeater`, `bSupportDPDownSpread`, `bAvoidHBR3`, `bOverrideLinkBw`, `DSC.*` |
| 0x73138f | `DP_NOTIFY_LT` | `NV0073_CTRL_DP_NOTIFY_LT_PARAMS` (12) | GSP | `dp_evoadapter.cpp:1360,1396,1580` around every training | status (a failure skips the closing call) |
| 0x73012d | `SYSTEM_GET_HOTPLUG_UNPLUG_STATE` | `..._GET_HOTPLUG_UNPLUG_STATE_PARAMS` (16) | **Local+GSP**: `KDISP/disp_common_kern_ctrl_minimal.c:49-108` calls `NV0073_CTRL_CMD_INTERNAL_GET_HOTPLUG_UNPLUG_STATE` (0x730401), then merges the masks per client | `nvkms-event.c:56-110` `nvHandleHotplugEventDeferredWork` | `hotPlugMask`, `hotUnplugMask` (both 0: NVKMS falls back to `GET_CONNECT_STATE`) |
| 0x730143 | `SYSTEM_ALLOCATE_DISPLAY_BANDWIDTH` | 12 | **Local+GSP**: `disp_common_kern_ctrl_minimal.c:111-177` checks kernel privilege, then sends `NV0073_CTRL_CMD_SYSTEM_INTERNAL_ALLOCATE_DISPLAY_BANDWIDTH` (0x730157) with the same struct (`PDB_PROP_KDISP_IMP_ALLOC_BW_IN_KERNEL_RM_DEF` is TRUE only on T234D/T239D/T264D, `GEN/g_kern_disp_nvoc.c:289-298`) | `nvkms-evo.c:7620` `nvAllocateDisplayBandwidth` | status |
| 0x73136a | `DP_SET_MSA_PROPERTIES` | 64 | **Local refusal**: not in the kernel's export table (only `_V2` 0x731381 is), so `NV_ERR_NOT_SUPPORTED`; the library ignores it (`dp_evoadapter.cpp:873-888`) | `dp_evoadapter.cpp:825` `setDpMSAParameters`, from `notifyAttachBegin` (`dp_connectorimpl.cpp:3938,3947`) | none |
| 0x730161 | `SYSTEM_QUERY_UEFI_DISPLAY_BANDWIDTH` | 12 | **Local**: `disp_common_kern_ctrl_minimal.c:179-210` calls `kdispGetUefiDisplayBandwidth_HAL`; only the Tegra v04_02 body exists (`KDISP/arch/v04/kern_disp_0402.c:162-183`); for TU106 the stub fails [I] | `nvkms-evo.c:7765` `QueryUefiIMPParams` (bandwidth realloc before a modeset) | failure tolerated [I] |

Conditional NV0073 controls, all routed to the device when they occur:
- `DFP_GET_DISPLAYPORT_DONGLE_INFO` (0x731142): only for TMDS connectors (`nvkms-dpy.c:714-745`);
- `SPECIFIC_DEFAULT_ADAPTIVESYNC_DISPLAY` (0x7302ae): only for VRR-capable EDIDs
  (`nvkms-hdmi.c:1265-1282`);
- `DP_CONFIG_INDEXED_LINK_RATES` (0x731377): only if the DPCD claims indexed link rates
  (`dp_connectorimpl.cpp:870-880`); the sink model does not claim them;
- `DP_CONFIGURE_FEC` (0x73137a): only when FEC is enabled, which DSC needs (phase G;
  `dp_connectorimpl.cpp:6549`).

**NV5070 (sent to the NVC570 object)**

| Id | Control | Struct (bytes) | Route | Sent by | Consumed |
|---|---|---|---|---|---|
| 0x50700709 | `SYSTEM_GET_CAPS_V2` | 8 | GSP | `nvkms-evo.c:5241` `nvAllocCoreChannelEvo` | caps bits |
| 0x50700457 | `SET_SOR_FLUSH_MODE` | 20 | GSP | `dp_evoadapter.cpp:892,913` `setFlushMode`/`clearFlushMode`, from `enableFlush`/`disableFlush` (`dp_connectorimpl.cpp:6665,7298`): a retrain while a head is attached | status |
| 0x50700117 | `SET_RMFREE_FLAGS` | small | GSP | `nvkms-rm.c:2960` `RmFreeEvoChannel` (teardown) | status |

Lock-only NV5070 controls, out of scope: `GET_FRAMELOCK_HEADER_LOCKPINS` and
`SET_RG_FLIPLOCK_PROP` (GSP), `GET_RG_STATUS` (GSP), `GET_RG_CONNECTED_LOCKPIN_STATELESS`
(local).

**NVC370 (NVC570 object)**

| Id | Control | Struct (bytes) | Route | Sent by | Consumed |
|---|---|---|---|---|---|
| 0xc3700104 | `GET_CHANNEL_INFO` | 20 | GSP | `nvkms-evo3.c:6428-6450` `GetChannelState`, used by `nvEvoIsChannelIdleC3` and `nvEvoIsChannelMethodPendingC3` (idle and "no method pending" are the same state, :6468-6480) | `channelState` (`STATE_IDLE` when GET == PUT) |
| 0xc3700201 | `GET_LOCKPINS_CAPS` | small | GSP | `nvkms-evo3.c:5077` `QueryStereoPinC3`, during caps parsing | failure tolerated (`:5176`) |
| 0xc3700103 / 0xc3700102 | `GET_ACCL` / `SET_ACCL` | small | GSP | `nvkms-evo3.c:6534,6565`, from the channel-accelerate paths (`:6590-6640,6730-6750`) used when idling or aborting channels | old and new accelerator masks |
| 0xc3700901 | `EVENT_SET_NOTIFICATION` | small | **Local** (`KDISP/disp_object_kern_ctrl_minimal.c:205-212` into `disp_objs.c:590-655`: flag bookkeeping, no RPC) | `nvkms-rm.c:5355,5396` `nvRmRegisterRgInterruptCallback` | status |

**NVC372 (NVC372_DISPLAY_SW)**

| Id | Control | Struct (bytes) | Route | Sent by | Consumed |
|---|---|---|---|---|---|
| 0xc3720101 | `IS_MODE_POSSIBLE` | 2048 | GSP (flags 0x48) | `nvkms-evo3.c:3319` `nvEvoIsModePossibleC3` (mode validation and modeset) | `bIsPossible`, `minRequiredBandwidthKBPS`, `floorBandwidthKBPS` (display-scaling.md §2.2) |
| 0xc3720104 | `GET_ACTIVE_VIEWPORT_POINT_IN` | small | GSP (flags 0x49, no GPU lock) | `nvkms-evo3.c:7356` `nvEvoGetActiveViewportOffsetC3` | viewport origin; conditional [I] |

**Count** (the question's "list of controls the device must answer for a DP connector"):

| Class | Reach the device (GSP) | Local + internal control to the device | Local only | MMIO |
|---|---|---|---|---|
| NV0073 | 36 (plus 4 conditional: `DFP_GET_DISPLAYPORT_DONGLE_INFO`, `SPECIFIC_DEFAULT_ADAPTIVESYNC_DISPLAY`, `DP_CONFIG_INDEXED_LINK_RATES` for an indexed-link-rate sink, `DP_CONFIGURE_FEC` with FEC for DSC; plus `SPECIFIC_GET_I2C_PORTID` 0x730211 from nvidia.ko) | 2 (`GET_HOTPLUG_UNPLUG_STATE` via 0x730401, `ALLOCATE_DISPLAY_BANDWIDTH` via 0x730157) | 2 (`DP_SET_MSA_PROPERTIES` refused, `QUERY_UEFI_DISPLAY_BANDWIDTH`) | 0 |
| NV5070 | 3 | 0 | 0 | 0 |
| NVC370 | 4 | 0 | 1 (`EVENT_SET_NOTIFICATION`) | 0 |
| NVC372 | 2 | 0 | 0 | 0 |

Beyond display-connector.md §1.3 (the DVI list) this adds the DP set (`AUXCH_CTRL`, `DP_CTRL`,
`NOTIFY_LT`, `GET_LINK_CONFIG`, `MAIN_LINK_CTRL`, `CONFIG_STREAM`, `SET_MANUAL_DISPLAYPORT`,
`GET_CAPS`, `GET_HDCP_STATE`, `DFP_UPDATE_DYNAMIC_DFP_CACHE`, `SET_SOR_FLUSH_MODE`, the DP audio
pair). It also adds four controls that list
missed and that also apply to DVI:
- `SYSTEM_GET_HEAD_ROUTING_MAP`: every modeset fails without its echo;
- `SYSTEM_GET_INTERNAL_DISPLAYS`;
- `SPECIFIC_IS_DIRECTMODE_DISPLAY`;
- NVC370 `GET_LOCKPINS_CAPS`.

It also corrects one route: `NVC573` is a local allocation, not a `GSP_RM_ALLOC` (section 3).

## 2. The EDID lifecycle for SST

### 2.1 What the device sees, in order [V]

1. **Detection** (DP library, inside `nvRmResumeDP` at bring-up or `nvDPNotifyLongPulse` on
   hotplug):
   - `ConnectorImpl::notifyLongPulseInternal` (`DPLIB/dp_connectorimpl.cpp:7489-7830`)
     first reads the DPCD caps: `DPCDHALImpl::parseAndReadCaps` (`dp_configcaps.cpp:144-185`)
     reads 1 byte at 0x00E, then 16 bytes at 0x000, or at 0x2200 if bit 7 of 0x00E is set.
   - Then the IRQ vectors and sink count, the OUI handshake, and the
     `setMessagingEnable(false)` write.
   - If the sink reports DPCD 1.4 or later, `DisplayId2ReadSST` (`dp_sst_displayid2.cpp:331`)
     tries DisplayID 2.x over I2C-over-AUX at 7-bit address 0x52 (`NV_DISPLAYID2_DDC_ADDRESS`
     0xA4). The sink model must I2C-NACK there.
   - `EdidReadSST` (`dp_sst_edid.cpp:237-330`) walks the DDC addresses 0x50, then 0x51, then
     0x53 (`DPLIB/../inc/dp_edid.h:233-235`, shifted right by one) and reads one 128-byte block at a time
     (`readNextBlock`, `:41-160`):
     - if the segment is not 0: one I2C MOT write of 1 byte to 0x30 (`EDID_SEG_SELECTOR_OFFSET`
       0x60, shifted);
     - one I2C MOT write of 1 byte (the offset, 0 or 128) to 0x50; the first read of every
       block is preceded by it, because `auxStatus` starts as nack;
     - I2C reads of 16 bytes (`size` 15): MOT set on the first 7, cleared on the 8th
       (`totalRead + 16 >= 128`). A reply with fewer bytes is accepted and continued; a
       success with 0 bytes is retried (up to `EDID_MAX_AUX_RETRIES` 10, sleeping 1 ms).
     - `EdidAssembler` then asks for the extension blocks the base block announces. The EDID
       is re-read up to 3 times if its checksum was patched.
   - Variant: if `DFP_GET_INFO` sets `DP_FORCE_RM_EDID`, the library reads with `GET_EDID_V2`
     (flags 0) instead of AUX. It reads once more with the full length if more than one block
     is announced. It falls back to AUX plus `APPLY_EDID_OVERRIDE_V2` on failure
     (`dp_sst_edid.cpp:255-300`).
2. **The connector's connected state** comes from the library, not from RM. For a DP-library
   connector, `nvDpyGetDynamicData` uses `nvDPLibDpyIsConnected`
   (`NVKMS/nvkms-dpy.c:3124-3146`). A failed DPCD or EDID read therefore makes the connector
   disconnected even while `GET_CONNECT_STATE` reports the bit.
3. **The ping-pong** (`nvDpyReadAndParseEdidEvo`, `nvkms-dpy.c:224-290`), on every dynamic-data
   query of a connected dpy (`DpyConnectEvo`, `:109-130`), for example each nvidia-drm
   connector probe:
   1. `SET_EDID_V2` with `bufferSize` 0 and a zeroed buffer (`ClearCustomEdid`, `:2097-2127`;
      return value ignored).
   2. `ReadEdidFromDP` (`:1179-1219`) copies the library's EDID. No RM call.
   3. `SET_EDID_V2` with `bufferSize` = EDID length and the bytes (`WriteEdidToResman`,
      `:2135-2172`; at most 2048 bytes; a failure is logged only).
   4. `GET_EDID_V2` with `flags` = `COPY_CACHE_NO` (0) (`ReadEdidFromResman`, `:1288-1367`).
      An error or `bufferSize` 0 is retried `NV_DP_READ_EDID_RETRIES` times for DP
      connectors. The returned bytes are what NVKMS validates, parses and exposes.
   5. `SPECIFIC_IS_DIRECTMODE_DISPLAY` with the parsed manufacturer and product id
      (`AssignIsVrHmd`, `:299-330`), and `SPECIFIC_GET_PCLK_LIMIT` (`nvDpyProbeMaxPixelClock`).
4. **Unplug:** `DpyDisconnectEvo` (`:99-107`) clears only NVKMS state. There is no RM call.
   The next connect starts at 3.1 again.

MST devices skip the ping-pong (`nvDpyEvoIsDPMST`); this does not apply to SST.

### 2.2 The contradiction in `SET_EDID_V2` and the rule the device follows

- `CTRL/ctrl0073/ctrl0073specific.h:172-215` says two things about an empty write:
  - "bufferSize of 0 would mean no bytes will be copied, but set the current cached EDID as
    static";
  - "If the EDID buffer is empty, the RM will remove any previous set EDID and allow further
    detection and EDID reads to use DDC".
- The only client in this tree sends `bufferSize` 0 with an all-zero buffer and calls it
  "clear out any stale state in RM about custom EDIDs" (`nvkms-dpy.c:2092-2096`). No
  source in the open tree implements the control: it is `ROUTE_TO_PHYSICAL` with no kernel
  body, so the closed GSP decides [O].
- **Device rule:** `bufferSize == 0` clears the override (the second reading), whatever the
  buffer holds.
  - Under the first reading, the guest's step 3.1 would pin whatever the cache held, and
    step 3.3 would then overwrite it anyway.
  - For NVKMS the only visible difference is a stale override after an EDID change. The
    second reading avoids that.
- To be confirmed by a passthrough trace (R1/R3) [O].

`GET_EDID_V2` flags (`ctrl0073specific.h:158-170`):
- `COPY_CACHE` bit 0: 0 means "use DDC", 1 means "the last EDID found";
- `READ_MODE` bit 1: 0 cooked, 1 raw (unpatched);
- `DISPMUX_READ_MODE` bits 3:2: ACPI, only for mux panels.

NVKMS sends 0; the DP library sends 0. For the device, "DDC" means the sink model's current
EDID. Rule [I]:

| Request | Answer |
|---|---|
| override set, any flags except RAW | the override bytes |
| RAW, or no override | the sink EDID; RAW also bypasses the override |
| `COPY_CACHE_YES` | the last answer given |
| disconnected displayId, or none | `NV_ERR_INVALID_ARGUMENT` (the header's "must specify a display with a positive connect state") |
| buffer too small | cannot happen: fixed 2048 bytes |

### 2.3 State per displayId

| State | Set by | Read by |
|---|---|---|
| `connected` (HPD level) | the operator's control API, hotplug | `GET_CONNECT_STATE`, the sink's AUX answers (a disconnected sink answers AUX with TIMEOUT or NACK [I]) |
| `pending_plug`, `pending_unplug` masks | hotplug | `INTERNAL_GET_HOTPLUG_UNPLUG_STATE` 0x730401 (cleared on read), after a `POST_EVENT` `NV2080_NOTIFIERS_HOTPLUG` |
| `sink_edid` (bytes, generation counter) | the sink model | I2C-over-AUX at 0x50/0x30, `GET_EDID_V2` without override |
| `override_edid` (optional bytes) | `SET_EDID_V2` with `bufferSize` > 0; cleared by `bufferSize` 0; `APPLY_EDID_OVERRIDE_V2` returns its input unchanged (no regkey overrides in the device) | `GET_EDID_V2` |
| `last_edid_answer` | every `GET_EDID_V2` | `COPY_CACHE_YES` |
| I2C transaction state (segment pointer, offset, MOT open) | AUX I2C writes | AUX I2C reads; a request without MOT closes the transaction; a DPCD reset or an unplug drops it (package A's sink) |
| DPCD register file and link state | package A's sink, `DP_CTRL` | AUX native, `DP_GET_LINK_CONFIG`, section 7 |

Design choice: set `DP_FORCE_RM_EDID` in `DFP_GET_INFO` or not.
- Setting it moves the first EDID read from AUX to `GET_EDID_V2`, which removes the I2C
  engine from the critical path of phase B.
- It is an eDP/ACPI-oriented flag, so the path is less exercised [I].
- Recommendation: leave it clear and exercise AUX. D15 wants the real DP path, and package A
  builds the I2C engine anyway. Keep the flag as a fallback switch for bring-up.

## 3. Classes and HAL selection

**How NVKMS picks the Turing HAL** [V]:
- `nvAssignEvoCaps` (`NVKMS/nvkms-hal.c:64-198`) walks `dispTable` from CC down to C5 and
  takes the first `NVxx70_DISPLAY` class present in the device's class list. For Turing that
  is `ENTRY_NVD(C5, C5, &nvEvoC5, 0, 1, 0)` (`:157`).
- It asserts that the matching core class `NVC57D_CORE_CHANNEL_DMA` is present too (`:189`)
  and records the ARMED core window as offset 0x8000, size 0x8000 (`:100-117`).
- The other classes are checked where they are used:

| Class | Checked / allocated at | Route of the alloc |
|---|---|---|
| `NV04_DISPLAY_COMMON` (0x0073) | `nvkms-rm.c:1819` (else `NVA083` displayless, else NO_HARDWARE) | `GSP_RM_ALLOC` (`RS_FLAGS_ALLOC_RPC_TO_ALL`, `KRM/rmapi/resource_list.h:1193`) |
| `NVC570_DISPLAY` | `nvkms-hal.c:183`, allocated `nvkms-evo.c:5224-5235` | `GSP_RM_ALLOC` (`:1103`) |
| `NVC57D_CORE_CHANNEL_DMA` | `nvkms-hal.c:189`, `nvkms-rm.c:3291` `nvRMSetupEvoCoreChannel` | `INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER` then `GSP_RM_ALLOC` (`:1303`) |
| `NVC57E_WINDOW_CHANNEL_DMA` + `NVC57B_WINDOW_IMM_CHANNEL_DMA` | `nvkms-rm.c:3096-3140` `nvRMAllocateWindowChannels` (the pair must both be present) | same (`:1313`, `:1293`) |
| `NVC57A_CURSOR_IMM_CHANNEL_PIO` | `nvkms-cursor.c:426-450` via `nvkms-cursor3.c:91` | `INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER` with `valid` FALSE (PIO, `KDISP/disp_channel.c:860-866`) then `GSP_RM_ALLOC` (`:1223`) |
| `NVC573_DISP_CAPABILITIES` | `nvkms-evo3.c:5769-5775` `EvoGetCapabilitiesC5` into `nvEvoGetCapabilities3` (`:5592-5660`): alloc, map, parse, unmap | **local** (`RS_FLAGS_ALLOC_PRIVILEGED \| ACQUIRE_GPUS_LOCK`, no RPC flag, `:1323`); the map is BAR0 0x640000 |
| `NVC372_DISPLAY_SW` | `nvkms-evo3.c:6500-6515` `nvEvoAllocRmCtrlObjectC3`, called at `nvkms-evo.c:9030` before the core channel | `GSP_RM_ALLOC` (`:1183`) |
| `NVC371_DISP_SF_USER` | not allocated for DP SST (the HDMI infoframe page, display-scaling.md §1.3) | — |

**Where the class list comes from** [V]:
- `NV0080_CTRL_CMD_GPU_GET_CLASSLIST` (0x800201) and `_V2` (0x800292) have flags 0x10b /
  0x1010b, without `ROUTE_TO_PHYSICAL` (`GEN/g_device_nvoc.c:522-535,672-685`). They are
  answered locally by `gpuGetClassList` (`KRM/gpu/device_ctrl.c:66,90`).
- The kernel's class database is the static TU106 table (`GEN/g_gpu_class_list.c:314-359`).
  It already has `NV04_DISPLAY_COMMON`, `NVC570`, `NVC573`, `NVC57A`, `NVC57B`, `NVC57D`,
  `NVC57E`, `NVC372`, `NVC371` and `NVA083_GRID_DISPLAYLESS`.
- The GSP static info does not gate the display: `gpuCheckEngineWithOrderList_KERNEL`
  returns `IS_FW_CLIENT` for `ENG_CLASS_KERNEL_DISPLAY` (`KRM/gpu/gpu.c:6862-6867`).
- The display classes disappear only when `kdispStatePreInitLocked` returns
  `NV_ERR_NOT_SUPPORTED`. `gpuStatePreInit` then calls
  `gpuRemoveMissingEngineClasses(ENG_KERNEL_DISPLAY)` and deletes the engine
  (`gpu.c:2227-2266`).
- `NVA083` is deleted separately unless KernelDisplay is absent on a GRID build
  (`gpu.c:2279-2287`).

**What the device answers today, and what must change** (not changed here):

| Item | Today | Needed |
|---|---|---|
| Class table | none in `crates/vfio-user-nvgpu` (no class list anywhere in the crate; the guest computes it) | nothing |
| `NV_FUSE_STATUS_OPT_DISPLAY` 0x21C04 | 1 (`chips/tu106/boot.toml:50`, named in `regmap.toml:24`) | 0 |
| `INTERNAL_DISPLAY_GET_IP_VERSION` | falls into the `_ =>` arm of `ctrl.rs` (`:682-687`): `NV_ERR_NOT_SUPPORTED`, label "unanswered", which removes the display silently | `ipVersion` 0x04000000, together with the whole phase-A set (section 5) |
| Display allocs | any unknown class is answered `NV_OK` with the params echoed (`ctrl.rs:238-283`, label "synthesized (object recorded, no host object yet)") | proper records: the channel instance, the pushbuffer from the preceding `CHANNEL_PUSHBUFFER`, the head and window counts |
| Display controls | all "unanswered" | section 1.3 |

Consequence: with the fuse at 0 and `GET_IP_VERSION` answered, GET_CLASSLIST carries the C5
classes without any device work, and NVKMS selects `nvEvoC5`.

## 4. Display channels and MMIO

### 4.1 Channel allocation: what carries what [V]

Per DMA channel, `RmAllocEvoChannel` (`NVKMS/nvkms-rm.c:2772-2893`) does the following:

1. Allocates the notifier surface: 4 KiB of sysmem, NISO (`nvRmAllocEvoDma`, `:2667-2760`,
   `nvRmAllocSysmem`). It gets a context DMA of type NOTIFIER.
2. Allocates the pushbuffer: 4 KiB of sysmem (`NV_DMA_EVO_PUSH_BUFFER_SIZE`,
   `NVKMS/../include/nvkms-types.h:78`) with its own context DMA.
3. The context DMAs are `NV01_CONTEXT_DMA` objects (local alloc, `resource_list.h:2303`) plus
   `NV0002_CTRL_CMD_BIND_CONTEXTDMA` (flags 0x8, local). Binding writes the object and the
   RAMHT entry into the **display instance memory in FB**:
   - the object layout is `KDISP/inst_mem/arch/v03/disp_inst_mem_0300.c:144-275`: word 0 is
     target node and kind and access; words 1-2 are base >> 8; words 3-4 are limit >> 8;
   - the hash is `:94-124`;
   - the kernel writes through `memmgrMemBeginTransfer`. That goes through the device's FB
     model (BAR2 or PRAMIN) [I for the transfer type].
   - **No RPC carries a context DMA.** A sysmem context DMA has target `PHYSICAL_PCI` or
     `_COHERENT`, and its base is the guest DMA address, which D13 makes a host IOVA.
4. `nvRmApiAlloc(class, NV50VAIO_CHANNELDMA_ALLOCATION_PARAMETERS{channelInstance,
   hObjectBuffer = pushbuffer ctxdma, offset 0})`. In the kernel, `dispchnConstruct`
   (`KDISP/disp_channel.c:150-260`) then does three things:
   - checks the class against the class list;
   - sends **`INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER` 0x20800a58** (`:794-871`, 56-byte struct:
     `hclass`, `channelInstance`, `subDeviceId`, `physicalAddr` of the pushbuffer, `limit`,
     `addressSpace` sysmem or FB, `cacheSnoop`, `pbTargetAperture`, `channelPBSize`, `valid`).
     The kernel ignores its return value;
   - maps the channel's BAR0 USERD page (`kdispMapDispChannel`). The `GSP_RM_ALLOC` of the
     class follows from the resource server (`RPC_TO_ALL`) [V display-path.md §2.2].
5. `nvRmApiMapMemory` of the channel handle, 0x1000 bytes: the USERD page with PUT at +0 and
   GET at +4. For the core channel NVKMS also maps the **ARMED** half read-only: offset 0x8000,
   size 0x8000 (`nvkms-rm.c:3210-3225`).

The notifier and semaphore context DMAs reach the hardware only as **handles in methods**:
- core `SET_CONTEXT_DMA_NOTIFIER` 0x208;
- window `SET_CONTEXT_DMA_NOTIFIER` 0x21C, `SET_CONTEXT_DMA_SEMAPHORE` 0x218,
  `SET_CONTEXT_DMA_ISO(0..3)` 0x240;
- head `SET_CONTEXT_DMA_OLUT` and `SET_CONTEXT_DMA_CURSOR`.

The device resolves each handle through RAMHT, using the hash of (handle, hClient, channel
number), to the context DMA object in its FB.

### 4.2 BAR0 the display path touches (additions to display-path.md §2.3) [V]

| Address | Accessor | Semantics the device must provide |
|---|---|---|
| 0x680000 +0 / +4 | NVKMS `EvoCoreKickoff` / `EvoCoreReadGet` (`nvkms-dma.c:50-69`) | PUT is a **byte offset** into the 4 KiB pushbuffer, even though the header's field is `PUT_PTR 9:0`. GET is polled in `nvEvoMakeRoom` (`:91-150`, 5 s timeout, logs "Error while waiting for GPU progress"). The pushbuffer wraps with a JUMP to 0 (`0x20000000`) at `offset_max` (4096 - 48) |
| 0x688000 + method offset | NVKMS `EvoInitWindowMapping3` (`nvkms-evo3.c:1335-1390`) | **ARMED state mirror**: reads `WINDOW_SET_CONTROL(w)` (0x1000 + 0x80·w) to see which head owns each window. The device must mirror the armed values of the core methods it applies (at least `WINDOW_SET_CONTROL.OWNER`) |
| 0x690000 + 0x1000·w, 0x690000 + (32+w)·0x1000 | window and window-imm USERD, the same PUT/GET protocol | as core. NVKMS also reads window `GET_LINE` 0x208 for the scan line (`nvkms-evo3.c:7082-7098`: bit 15 in blank, bits 14:0 the line) |
| 0x6D8000 + 0x1000·h | cursor PIO page (`CLS/clc57a.h:33-41`) | `Free` at +0x08 **must read non-zero**, or NVKMS spins (`nvkms-cursor3.c:38-50`); `Update` +0x200, `SetInterlockFlags` +0x204, `SetCursorHotSpotPointOut[2]` +0x208, `SetWindowInterlockFlags` +0x210 are written directly |
| 0x640000 (`sizeof(_NvC573DispCapabilities)`) | `nvEvoGetCapabilities3` via the local `NVC573` map | `SYS_CAP`, `SYS_CAPB`, head, SOR and window caps; `HEAD_CLK_CAP` 0x5e8+4i (the NVKMS workaround definition, reset value 0x85, `nvkms-evo3.c:5181-5200`) (display-scaling.md §2.2) |
| 0x6104E0-0x610784 `FE_CHNCTL`/`FE_CHNSTATUS` | only `nvdispapiCtrlCmdChannelCancelFlip` (`disp_object_kern_ctrl_minimal.c:175-200`, kernel-local NVC370 `CHANNEL_CANCEL_FLIP`) | NVKMS never sends it, so reads may return idle and connected values; the device models no state here [I] |
| 0x611800+, 0x611C00+, 0x611EC0, 0x611C30, 0x611858, 0x612078, 0x616330, 0x61611C | the guest kernel's display ISR and head code | as display-connector.md §2.1 |

### 4.3 Method stream basics a decoder must understand

**Header word** (`CLS/clc57d.h:58-68`, identical in `clc57e.h:35-46`):
- bits 31:29 opcode: 0 METHOD (incrementing), 1 JUMP, 2 NONINC_METHOD, 3 SET_SUBDEVICE_MASK;
- bits 27:18 count;
- bits 13:2 method offset (dword address);
- JUMP carries its offset in bits 11:2.
- The decoder walks from GET to PUT and follows JUMPs.

Core channel (C57D), for one head and one SOR [V]:

| Method | Offset | What the device takes from it |
|---|---|---|
| `UPDATE` | 0x200 | commit: apply the assembly state, write the notifier, move GET; `SPECIFIC_HANDLING` 21:20, `INHIBIT_INTERRUPTS` 24, `RELEASE_ELV` 0, `FLIP_LOCK_PIN` 8:4 |
| `SET_CONTEXT_DMA_NOTIFIER` | 0x208 | handle (0 disables; NVKMS sets 0 when no notification is wanted, `nvkms-evo3.c:2676-2690`) |
| `SET_NOTIFIER_CONTROL` | 0x20C | `MODE` 0 WRITE / 1 WRITE_AWAKEN, `OFFSET` 11:4 in **16-byte units**, `NOTIFY` 12 |
| `SET_INTERLOCK_FLAGS`, `SET_WINDOW_INTERLOCK_FLAGS` | 0x218, 0x21C | which cursor and window channels commit with this update (`UpdateCoreC3`, `nvkms-evo3.c:2693-2730`) |
| `SOR_SET_CONTROL(s)` | 0x300 + 0x20·s | owner head mask, protocol `DP_A` |
| `WINDOW_SET_CONTROL(w)` | 0x1000 + 0x80·w | owner head (mirrored into ARMED) |
| `WINDOW_SET_WINDOW_FORMAT_USAGE_BOUNDS(w)` and neighbours | 0x1004 + 0x80·w | record only |
| `HEAD_SET_CONTROL_OUTPUT_RESOURCE(h)`, `HEAD_SET_CONTROL(h)` | 0x2004, 0x2008 + 0x400·h | record |
| `HEAD_SET_PIXEL_CLOCK_FREQUENCY(h)` | 0x200C | pixel clock, feeds the timer period |
| `HEAD_SET_DISPLAY_ID(h,i)` | 0x2020 | record |
| `HEAD_SET_VIEWPORT_SIZE_IN/OUT(h)` | 0x204C, 0x2058 | record |
| `HEAD_SET_RASTER_SIZE`, `_SYNC_END`, `_BLANK_END`, `_BLANK_START(h)` | 0x2064-0x2070 | raster total; period = htotal·vtotal/pclk (display-connector.md §2.2) |
| `HEAD_SET_CONTEXT_DMA_CURSOR(h,i)`, `HEAD_SET_OLUT_CONTROL(h)`, `HEAD_SET_CONTEXT_DMA_OLUT(h)` | 0x2088, 0x2280, 0x2288 | accept and record (Mutter's gamma LUT must validate) |
| `HEAD_SET_DSC_CONTROL(h)` and the PPS | 0x22D4 onwards | record; needed only with DSC (D15) |

Window channel (C57E), one window [V `CLS/clc57e.h:94-180,266-276`; encodings from
`nvkms-evo3.c:3434-3471,3640-3655,3748,3782-3846`]:

| Method | Offset | Encoding |
|---|---|---|
| `UPDATE` | 0x200 | commit; `INTERLOCK_WITH_WIN_IMM` |
| `SET_SEMAPHORE_CONTROL` | 0x20C | `OFFSET` 7:0 in 16-byte units (NVKMS writes `offsetInWords / 4`) |
| `SET_SEMAPHORE_ACQUIRE` / `_RELEASE` | 0x210 / 0x214 | 32-bit values |
| `SET_CONTEXT_DMA_SEMAPHORE` | 0x218 | handle; 0 means no semaphore (all four are written 0) |
| `SET_CONTEXT_DMA_NOTIFIER`, `SET_NOTIFIER_CONTROL` | 0x21C, 0x220 | as core |
| `SET_SIZE` | 0x224 | width 15:0, height 31:16 |
| `SET_STORAGE` | 0x228 | `BLOCK_HEIGHT` 3:0 (log2 GOBs), `MEMORY_LAYOUT` 4 (0 block-linear, 1 pitch) |
| `SET_PARAMS` | 0x22C | `FORMAT` (for example 0xCF A8R8G8B8, 0xE6 X8R8G8B8, 0xDF A2R10G10B10) plus flags |
| `SET_PLANAR_STORAGE(p)` | 0x230 + 4p | pitch 12:0: in blocks for block-linear, in 64-byte units for pitch |
| `SET_CONTEXT_DMA_ISO(p)` | 0x240 + 4p | surface handle |
| `SET_OFFSET(p)` | 0x260 + 4p | byte offset **>> 8** (`nvCtxDmaOffsetFromBytes`, `NVKMS/../include/nvkms-utils.h:172-178`) |
| `SET_POINT_IN`, `SET_SIZE_IN`, `SET_SIZE_OUT` | 0x290, 0x298, 0x2A4 | viewport |
| `SET_COMPOSITION_*`, CSC, ILUT, TMO | 0x2EC onwards | accept and ignore |
| `SET_PRESENT_CONTROL` | 0x308 | `MIN_PRESENT_INTERVAL` 3:0, `BEGIN_MODE` 6:4 (0 non-tearing, 1 immediate), `TIMESTAMP_MODE` 8 |
| `SET_UPDATE_TIMESTAMP_LO/HI` | 0x348 / 0x34C | earliest latch time |
| `SET_INTERLOCK_FLAGS`, `SET_WINDOW_INTERLOCK_FLAGS` | 0x370 / 0x374 | commit grouping |

**Completion formats** [V]:
- Notifier `NV_DISP_NOTIFIER` (`CLS/clc57d.h:34-56`), 16 bytes:
  - word 0: `PRESENT_COUNT` 7:0, `FIELD` 8, `FLIP_TYPE` 9, `STATUS` 31:30 (0 NOT_BEGUN, 1 BEGUN,
    2 FINISHED);
  - words 2-3: 64-bit timestamp.
  - NVKMS writes NOT_BEGUN and polls word 0 of record `idx` for FINISHED
    (`nvkms-evo3.c:6262-6288`).
- Semaphore, "FOUR_WORD_NVDISPLAY" (`NVKMS/../interface/nvkms-api-types.h:150-180`): 16-byte
  stride, payload in word 0. nvidia-drm reads `pLinearAddress[fenceSemIndex * 4]`
  (`OGK/../kernel-open/nvidia-drm/nvidia-drm-fence.c:184-186`).
- Only the first 4096 bytes of a notifier or semaphore surface are addressable.

**Supervisor and completion signalling:**
- The guest kernel has no supervisor handler in GSP-client mode. `supervisor` appears only in
  a comment in `KDISP/kern_disp.c:1952`, and display-path.md §2.5 has the rest.
- The device applies the state on `UPDATE` and signals completion by the notifier, AWAKEN
  and GET = PUT. `GET_CHANNEL_INFO` answers IDLE once GET = PUT.
- The guest polls these:
  - GET (MMIO);
  - the notifier word in sysmem;
  - `GET_CHANNEL_INFO` (control);
  - ARMED `WINDOW_SET_CONTROL` (MMIO);
  - cursor `Free` (MMIO);
  - its interrupt registers after an MSI (display-connector.md §2.1).

### 4.4 The minimal decoder

**Must implement for the first modeset and the first flip:**
1. Pushbuffer walking: METHOD and NONINC_METHOD with a count, JUMP, and a work budget per
   kick (briefing §11). SET_SUBDEVICE_MASK is accepted as a no-op for one subdevice.
2. Core: the assembly state for the methods in the core table; `UPDATE` makes it ARMED
   (including the ARMED mirror at 0x688000), writes the notifier FINISHED, raises
   AWAKEN_OTHER if `WRITE_AWAKEN`, sets `FE_CORE_HEAD_STATE` and moves GET.
3. Window: the methods in the window table.
   - `UPDATE` queues a presentation: context DMA and offset, size, storage, format, pitch,
     semaphore acquire and release, notifier, present control, timestamp.
   - The device timer latches it (display-connector.md §2.2).
   - The previous presentation's release semaphore is written at latch time.
   - Then the notifier is written and AWAKEN_WIN is set.
4. Window-imm (C57B) and cursor PIO (C57A): accept `SET_POINT_OUT`, the hot spot and
   `UPDATE`; keep `Free` non-zero.
5. RAMHT lookup and context DMA decode from FB. Bounds checks against the context DMA limit
   and against the guest's memory.

**May reject visibly** (log, complete the update, surface nothing; or, for a surface,
refuse the latch and count it):
- unknown formats, planar and semi-planar YUV;
- block heights above 32 GOBs;
- input scaling (`SIZE_IN` != `SIZE_OUT`), CSC, ILUT, TMO, HDR, stereo;
- flip lock, `SPECIAL_HANDLING` values other than NONE, CRC control;
- DSC before phase G;
- more than one SOR per head (2Heads1OR);
- a context DMA whose target is neither FB nor sysmem, or which exceeds its limit.

## 5. First missing behaviour: the phase A/B checklist

State today: fuse 1, the display is removed before any display RPC (display-d0.md:
zero display RPCs in the whole trace). Below, the device answers `0x21C04 = 0`, and the
class list follows automatically. Each row gives the request, where it is sent, what the
device answers today and what happens if it stays that way.

**Phase A: nvidia.ko, `RmInitAdapter`** (connector-agnostic; display-connector.md §4 phase A)

| # | Request | Source | Today | If unanswered |
|---|---|---|---|---|
| A0 | FB layout at GSP boot: fuse read and BAR0 0x625F04 `NV_PDISP_VGA_WORKSPACE_BASE` | `KRM/gpu/gsp/arch/turing/kernel_gsp_tu102.c:787-802`, `KDISP/arch/v04/kern_disp_0400.c:46-60` | register named in `regmap.toml:38`, layout "with no display" (`chip/turing.rs:152`) | layout mismatch; `kbusVerifyBar2` fails (fb-model.md §3.2: the reserved block moves 0x20000 to 0x30000) |
| A1 | BAR0 0x21C04 | `kern_disp.c:444` via `gpuFuseSupportsDisplay_HAL` | 1 | display removed (as today) |
| A2 | **`GSP_RM_CONTROL` `INTERNAL_DISPLAY_GET_IP_VERSION` 0x20800a4b** | `kern_disp.c:435-466` | "unanswered", `NOT_SUPPORTED` | **the first missing behaviour:** the display is removed silently, init continues; answering it makes A3-A8 mandatory |
| A3 | `GSP_RM_ALLOC` `NV04_DISPLAY_COMMON` | `kern_disp.c:380-409` `kdispAllocateCommonHandle` | `NV_OK` (generic echo) | — |
| A4 | `INTERNAL_DISPLAY_GET_STATIC_INFO` 0x20800a01 | `kern_disp.c:553-647` (`NV_CHECK_OK_OR_GOTO`) | unanswered | **RmInitAdapter fails** |
| A5 | `INTERNAL_INIT_BRIGHTC_STATE_LOAD` 0x20800ac6 | `kern_disp.c:469-510` (status returned) | unanswered | RmInitAdapter fails |
| A6 | `INTERNAL_SET_STATIC_EDID_DATA` 0x20800adf (`tableLen` 0) | `kern_disp.c:512-550` | unanswered | RmInitAdapter fails |
| A7 | `INTERNAL_DISPLAY_SET_IMP_INIT_INFO` 0x20800a54 | `kern_disp.c:809-846` (`NV_ASSERT_OK_OR_RETURN`) | unanswered | RmInitAdapter fails [I: HAL bound on TU106] |
| A8 | `INTERNAL_DISPLAY_WRITE_INST_MEM` 0x20800a49 | `KDISP/inst_mem/disp_inst_mem.c:393` | unanswered | instmem load fails [I] |
| A9 | `NV0073_CTRL_CMD_SYSTEM_MAP_SHARED_DATA` 0x730151 (physical-only, not client-exported) | `kern_disp.c:668-740` from `kdispStateLoad` (`:772-786`, result ignored) | unanswered | logged only |
| A10 | `INTR_GET_KERNEL_TABLE` with a `MC_ENGINE_IDX_DISP` vector in leaf 4 | display-connector.md §5 | no DISP entry (`chips/tu106/interrupts.toml`) | no vblank or AWAKEN later (phase C) |
| A11 | `GSP_RM_ALLOC` `NV04_DISPLAY_COMMON` (`nv->rmapi.hDisp`) | `OGK/nvidia/arch/nvalloc/unix/src/osinit.c:1852-1865` | `NV_OK` | tolerated |
| A12 | NV0073 `SYSTEM_GET_SUPPORTED`, per id `SPECIFIC_OR_GET_INFO`, for a DP SOR on the chip `SPECIFIC_GET_I2C_PORTID` | `osinit.c:2291` into `osapi.c:4266-4390` `RmI2cAddGpuPorts` | unanswered | tolerated (no I2C adapters). With answers: `GET_SUPPORTED` lands in the kernel cache (by input), so NVKMS's later call may never reach the device; `GET_I2C_PORTID` answers `ddcPortId` 0 |

Phase A evidence: `RmInitAdapter succeeded!`. The trace shows A2-A12 in this order, each
with its D9 label. `nvml` 11/11 and `uvm verify` are unchanged (D0).

**Phase B: `nvidia-modeset` + `nvidia-drm modeset=1 fbdev=0`, `nvAllocDevEvo`**
(`nvkms-evo.c:8960-9045`)

| # | Request | Source | Failure mode if wrong |
|---|---|---|---|
| B1 | `GET_CLASSLIST` (local) now carries the C5 classes; `nvAssignEvoCaps` picks `nvEvoC5` | `nvkms-hal.c:183` | NO_HARDWARE, displayless fallback |
| B2 | NV2080 caps controls (`QueryGpuCapabilities`), `GSP_RM_ALLOC NV04_DISPLAY_COMMON` | `nvkms-rm.c:135`, `:1819-1833` | "Failed to initialize the display subsystem" |
| B3 | `SYSTEM_GET_CAPS_V2` | `nvkms-rm.c:1422-1440` | NO_HARDWARE |
| B4 | `SYSTEM_GET_NUM_HEADS`, `SPECIFIC_GET_ALL_HEAD_MASK`, `SPECIFIC_GET_VALID_HEAD_WINDOW_ASSIGNMENT` | `nvkms-rm.c:999-1110` | fatal |
| B5 | `SYSTEM_GET_SUPPORTED` (possibly cached from A12), `SPECIFIC_OR_GET_INFO` per id | `nvkms-rm.c:426-486` | "Failed to get supported display device(s)" |
| B6 | per connector: `SPECIFIC_OR_GET_INFO`, `SPECIFIC_GET_CONNECTOR_DATA` (`DP_EXT`) | `nvkms-rm.c:633-700` | "Failed to determine connector type"; a DP protocol with a non-DP type drops the connector (`:792-800`) |
| B7 | DP library connector: `DP_SET_MANUAL_DISPLAYPORT`, `SPECIFIC_GET_ALL_HEAD_MASK`, `DP_GET_CAPS` (`sorIndex` = `or.primary` from B6), `DFP_GET_INFO` | `nvdp-connector.cpp:45-100`, `dp_evoadapter.cpp:126-176,279-333,1697-1752`, `dp_connectorimpl.cpp:143-144` | "Failed to initialize DisplayPort support" only on allocation failure; wrong caps show up later as wrong mode limits |
| B8 | `DFP_GET_INFO` (again), `SPECIFIC_GET_TYPE` | `nvkms-rm.c:565-597`, `:2293` | wrong signal format |
| B9 | NV2080 `EVENT_SET_NOTIFICATION` for HOTPLUG, DP_IRQ, HDMI_FRL (with `NV01_EVENT_KERNEL_CALLBACK_EX` allocs) | `nvkms-rm.c:1910-1990` | warnings only |
| B10 | `SYSTEM_GET_BOOT_DISPLAYS` | `nvkms-rm.c:1373` | — |
| B11 | `GSP_RM_ALLOC NVC372_DISPLAY_SW` | `nvkms-evo.c:9030` | FATAL_ERROR |
| B12 | `nvAllocCoreChannelEvo` (`nvkms-evo.c:5177-5330`): GC6 and VT controls (the VT switch brings `INTERNAL_DISPLAY_PRE/POST_UNIX_CONSOLE`), `GSP_RM_ALLOC NVC570`, NV5070 `SYSTEM_GET_CAPS_V2` | `:5224-5245` | "Failed to allocate display engine core DMA push buffer" and neighbours |
| B13 | core channel: `SYSTEM_GET_ACTIVE` per head (`GetVbiosHeadAssignment`, `nvkms-rm.c:3321`), then notifier and pushbuffer ctxdma binds (FB instmem writes), `INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER`, `GSP_RM_ALLOC NVC57D`, USERD map 0x680000 and ARMED map 0x688000 | `nvkms-rm.c:3291-3330`, `:2772-2893` | channel alloc failure |
| B14 | caps: local `NVC573` alloc and BAR0 0x640000 read; NVC370 `GET_LOCKPINS_CAPS` | `nvkms-evo3.c:5592-5660` | "Failed to query display engine capability bits"; `numWindows` 0 asserts |
| B15 | per window: pushbuffer RPC + `GSP_RM_ALLOC NVC57E`, then `NVC57B`; per head: `NVC57A` (PIO) with `Free` non-zero | `nvkms-rm.c:3096-3140`, `nvkms-cursor.c:426-450` | "Failed to allocate display engine window channels" |
| B16 | `nvRmResumeDP`: `SYSTEM_GET_CONNECT_STATE` (connector bit), then per plugged DP connector `DFP_ASSIGN_SOR` | `nvkms-rm.c:2475-2502`, `nvdp-connector.cpp:1013-1063` | a disconnected answer ends detection here |
| B17 | `ConnectorImpl::resume` into `notifyLongPulseInternal`: **the first `DP_AUXCH_CTRL`**, a native read of DPCD 0x00E, then 0x000 (16 bytes) or 0x2200, the IRQ vectors and sink count, the OUI, the 0x111 write, DisplayID at 0x52 (must NACK) | `dp_connectorimpl.cpp:1046-1110,7489-7790` | revision below 1.0 or an AUX failure: the connector is never connected |
| B18 | **the first EDID read**: I2C MOT write of the offset to 0x50, 8 reads of 16 bytes (section 2.1) | `dp_sst_edid.cpp:41-160` | "DP-CONN> Corrupt Edid", no modes |
| after | `SYSTEM_GET_INTERNAL_DISPLAYS`, NVC370 `EVENT_SET_NOTIFICATION` (local); nvidia-drm probe: the section 2.1 ping-pong, `IS_DIRECTMODE_DISPLAY`, `GET_PCLK_LIMIT`; DP library mode validation against `DP_GET_CAPS` and the DPCD; `IS_MODE_POSSIBLE` | `nvkms-evo.c:5315-5325`, `nvkms-dpy.c` | connector connected without modes |

The core init methods need no completion before B17 [I]. NVKMS neither waits for the notifier
nor for idle in `nvAllocCoreChannelEvo`, and a 4 KiB pushbuffer does not fill up with the init
methods. Still, moving GET to PUT from phase B on costs nothing and removes a 5 s stall risk.

Phase B evidence (D3, below):
- `/sys/class/drm/card0-DP-1/status` is `connected`;
- `edid` there is byte-identical to the sink's EDID and to the device's `GET_EDID_V2` answer;
- the mode list matches the EDID minus the modes the DP link budget rejects;
- the device trace shows B1-B18 and the ping-pong, and no control labelled "unanswered" in
  the display ranges;
- there is no core `UPDATE` carrying `HEAD_SET_RASTER_SIZE`.

## 6. Gates D0-D6 on the phases of display-connector.md

D15 keeps phase A connector-agnostic and moves phase B to DP. display-scaling.md adds
phases E-H (multi-head, DP SST, 4K144, runtime API). A return code is never enough: each
gate names the observation that would be different if the device were lying.

| Gate | Phase | Evidence beyond a return code |
|---|---|---|
| **D0** baseline unchanged | before A, and again after A and after B | `tools/caraxes-gate.sh nvml` and `uvm verify` produce the same verdict JSON. With the display switched off in the profile (fuse 1), the device trace has zero display RPCs, as in display-d0.md, so the headless mode stays reproducible. The FB layout and the `kbusVerifyBar2` result are recorded for both fuse values |
| **D1** sink unit tests | package A, independent of the phases | A recorded AUX request log from a B17/B18 trace, replayed against the sink model, gives byte-identical replies (`size`, `replyType`, `data`). Plus the briefing's cases: 1 and 16 bytes, a short reply, address-only, MOT continuation, NACK, DEFER, an invalid length, an invalid displayId, a lane-configuration change, the EDID segment pointer |
| **D2** verified control and MMIO route | A and B | For every row of section 1.3 the trace shows the expected route. GSP rows appear as `GSP_RM_CONTROL` with the exact struct size. Local rows (`EVENT_SET_NOTIFICATION`, `DP_SET_MSA_PROPERTIES`) **do not** appear. Local+GSP rows appear as their internal id (0x730401, 0x730157). Cached rows appear once per input. Every answer carries its D9 label (`synthesized`/`profile`, never `host`). The MMIO side: GET follows PUT on every channel, ARMED `WINDOW_SET_CONTROL` reads back the committed owner, cursor `Free` is non-zero |
| **D3** monitor detected with the right modes | B | As in section 5, phase B evidence. Plus a negative mode: the EDID lists a mode above the device's link budget (for example 4K144 without DSC caps) and it must be pruned. A second parser (`edid-decode --check`, libdisplay-info) agrees with NVKMS's parse |
| **D4** visible test image | C (first modeset), optionally E (host sink) | The core notifier reaches FINISHED within NVKMS's wait with no "timed out" log. The device's latched surface descriptor (the context DMA resolved to an FB address, offset, format, pitch or block height, size) equals the DRM framebuffer. The guest's dumb-buffer readback hash matches the committed pattern (the v1 vdisplay pixel stage). "Visible" on the host needs phase E's dump, which must equal the same pattern after de-tiling |
| **D5** alternating flips with a deliberately delayed acquire | C | Two buffers alternate. Flip-complete timestamps are spaced one period apart (60.0 ± 0.5 Hz). The notifier and release semaphore are written only at ticks. With one acquire value deliberately withheld, the flip stays pending: no notifier write, no AWAKEN, no release, and the compositor stalls. After the value is written, it completes at the next tick, never earlier. The device's tick counter equals the guest's head vblank count |
| **D6** unplug, reset, replug | D (hotplug via `POST_EVENT`), plus a driver unload and reload | Unplug: `GET_HOTPLUG_UNPLUG_STATE` (0x730401) reports the unplug mask once; sysfs shows `disconnected`; the DPCD answers as unplugged. Reset: `rmmod`/`modprobe` of nvidia-drm and nvidia-modeset frees every display object (`SET_RMFREE_FLAGS`, channel frees), and the device's registry holds no display object afterwards. Replug: a second full B16-B18 sequence in the trace, a fresh EDID (change one byte between plugs to prove it was re-read), and the new mode list in sysfs |

## 7. The three riskiest ABI details

1. **AUX size and status layering.**
   - `size` means count-minus-one in the request, but 0 also stands for address-only (with
     `bAddrOnly`).
   - `size` means the actual count in the reply.
   - Sink-level NACK and DEFER must be `NV_OK` plus `replyType`. Any other status is a NACK
     to the DP library, whatever `replyType` holds, and only `NV_ERR_TIMEOUT` maps to defer.
   - The struct has a 3-byte hole after `bAddrOnly` (48 bytes in all), and
     `COPYOUT_ON_ERROR` returns the params even on error.
   - A device that reports the requested size instead of the transferred one, or an error for
     NACK, still passes a simple "EDID reads" test but breaks short replies, DEFER backoff and
     the DisplayID probe at 0x52.
2. **The EDID ping-pong and `bufferSize` 0.**
   - NVKMS does not use the EDID it read over AUX. It uses what `GET_EDID_V2` returns after
     its own `SET_EDID_V2`.
   - If the device answers from the sink instead of the override, or treats `bufferSize` 0 as
     "pin the cache" (one reading of the header), an EDID change can come back stale.
   - The closed GSP's real semantics are unknown [O]. The 2060/2064-byte structs also make
     these the largest routine display RPCs.
3. **Link-training coherence across controls, the DPCD and the guest's permanent cache.**
   These values must all agree:
   - `DP_GET_CAPS.maxLinkRate` (cached for good per `sorIndex`);
   - `DFP_GET_INFO` `DP_LINK_BW` (0 silently caps the link at 1.62 Gbit/s);
   - DPCD `MAX_LINK_RATE`/`MAX_LANE_COUNT`;
   - the `DP_CTRL` data (link BW in 270 MHz units at 15:8, lane count at 4:0);
   - the DPCD 0x100/0x101 and lane status 0x202-0x207 that GSP's training would have written;
   - `DP_GET_LINK_CONFIG` (`linkBW`, `dp2LinkBW` 0).

   The DP library cross-checks lane status over AUX (`isLinkLost`, `isLinkStatusValid`) and
   retrains or fails the modeset if one disagrees. Because `DP_GET_CAPS`, `GET_SUPPORTED`,
   `GET_TYPE` and `GET_INTERNAL_DISPLAYS` are `PERSISTENT_CACHEABLE`, a wrong first answer
   sticks until the guest driver reloads; a hotplug cannot repair it.

## Sources

- Vendor: `vendor/open-gpu-kernel-modules` 610.57.04 (`e4a5faa2567f`), files and lines cited
  inline. Struct sizes from a scratch program compiled against
  `src/common/sdk/nvidia/inc` with the host gcc (not kept in the tree).
- Device crate: `crates/vfio-user-nvgpu/chips/tu106/boot.toml:48-50`, `regmap.toml:24,38`,
  `src/ctrl.rs:238-283,682-687`, `src/boot.rs:600-660`, `src/chip/turing.rs:152`.
- nouveau r535 (linux-6.12): `nvkm/engine/disp/r535.c:955-1100,1281-1310,1571-1580`.
- Leandro VFIO notes: [display-path.md](display-path.md), [display-connector.md](display-connector.md),
  [display-scaling.md](display-scaling.md), [display-d0.md](../history/caraxes-display-d0-2026-09-23.md),
  [fb-model.md](fb-model.md) §3.2, [briefing-display-nova.md](../history/caraxes-briefing-display-nova-2026-09-23.md).

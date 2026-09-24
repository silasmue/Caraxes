<!-- SPDX-License-Identifier: MIT -->
<!-- Source survey of 2026-09-24 against the open-gpu-kernel-modules trees 615.71.09 (the target guest ABI, D19) and 610.57.04, read only; the table of section 3 is generated from both trees (enum, generated structs compiled for sizeof, every call site), the rest is written by hand. -->
# RPC functions of 615.71.09: what a GSP-RM stand-in must answer

Part of the GSP plan ([README.md](README.md)). This file covers the RPC *functions*, the
messages the kernel RM sends to GSP-RM (the `X(UNIT, NAME, NUM)` list of
`rpc_global_enums.h`), plus the two GMCAPI commands that replace four of them from 615 on.
Events (GSP to CPU) are in [events.md](events.md), allocations in [classes.md](classes.md),
the boot order in [boot-exchange.md](boot-exchange.md). The controls carried inside
`GSP_RM_CONTROL` belong to the controls plan (categories A to E of
[gsp-surface.md](../gsp-surface.md)).

**Trees and short paths.** "615" is the open-gpu-kernel-modules tree at 615.71.09 (the same
release as the NAS tree of [gsp-transport-615.md](../gsp-transport-615.md)); "610" is
`vendor/open-gpu-kernel-modules` at 610.57.04. Paths are relative to the tree root, with these
short forms: `rpc.c` is `src/nvidia/src/kernel/vgpu/rpc.c`; `rpc.h`, `rpc_vgpu.h` and
`rpc_global_enums.h` are under `src/nvidia/inc/kernel/vgpu/`; `g_rpc-structures.h` is under
`src/nvidia/generated/`; `kernel_gsp.c` is `src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c` and
`kernel_gsp_tu102.c` is `.../gpu/gsp/arch/turing/kernel_gsp_tu102.c`; `gmcapi_*.h` are under
`src/nvidia/interface/gmcapi/`. Other short paths in the table of section 3 are relative to
`src/nvidia/src/kernel/`. A line number without a tree is 615; "(610: n)" gives the 610 line
where it differs. Device paths are `crates/vfio-user-nvgpu/src/...` at the worktree base of
this plan (commit 61e0b6a).

## 1. Method

- **The enum.** `rpc_global_enums.h:10-242` (615) lists 232 functions, 0 to 231; 610 has 230
  (`:10-240`). The only differences: 615 adds `CTRL_FIFO_VGPU_SWRUNLIST_SUBMIT` (230) and
  `CTRL_FIFO_VGPU_SWRUNLIST_CHANNEL_UPDATE` (231) (`:240-241`) and marks `GSP_SET_SYSTEM_INFO`
  (72) and `SET_REGISTRY` (73) deprecated (`:82-83`; 610 `:82-83` without the comment).
- **Payload structs and sizes.** Every `typedef struct rpc_<name>_vXX_YY` of
  `g_rpc-structures.h` of each tree, compiled with the minimal flag set of
  `caraxes/tools/gsp-abi-sizes.sh` (`RPC_STRUCTURES` defined, `-DGSP_ABI=610|615`) and printed
  with `sizeof`. 610 has 212 structs, 615 has 219. Of the function structs present in both
  trees none changed size, and the struct text of every shared versioned name is identical.
  The changes are additions and one removal: `rpc_ctrl_fifo_vgpu_swrunlist_submit_v2F_02` and
  `_channel_update_v2F_02` (24 bytes each, new), `rpc_rm_api_control_v2F_01/_03/_05` (40 each,
  new; `rpc_rm_api_control_v` moved from `v2E_05` to `v2F_05`), and
  `rpc_gsp_set_system_info_v17_00` (4 bytes in 610, `g_rpc-structures.h:1401` of 610) removed.
  The only size change in the whole file is an event, `rpc_rc_triggered_v17_02`, 48 to 64
  bytes ([events.md](events.md)).
- **Senders.** For each function: the `rpc<Name>_vXX_YY` sender body in `rpc.c`, every
  `NV_RM_RPC_<NAME>` and `rpc<Name>_HAL` call site under `src/nvidia/src` and
  `src/nvidia/arch`, the guard in the 40 lines before it (`IS_VIRTUAL`, `IS_GSP_CLIENT`,
  `IS_FW_CLIENT`, ...), and for the `CTRL_*` functions the case of `rpcDmaControl_wrapper`
  (`rpc.c:4595`) that maps a control to them. Every site that a GSP client can reach was then
  read by hand (section 4).
- **Routing of the generic macros on a GSP client.** `NV_RM_RPC_CONTROL`, `_ALLOC_CHANNEL`,
  `_ALLOC_OBJECT`, `_FREE`, `_ALLOC_EVENT`, `_ALLOC_SUBDEVICE`, `_DUP_OBJECT` and
  `_ALLOC_SHARE_DEVICE` test `IS_FW_CLIENT` and call `GPU_GET_PHYSICAL_RMAPI(pGpu)`
  (`rpc.h:172-388`), whose GSP implementation is `rpcRmApiControl_GSP`, `rpcRmApiAlloc_GSP`,
  `rpcRmApiDupObject_GSP` and `rpcRmApiFree_GSP` (`rpc.c:10655, 10941, 11063, 11116`; 610:
  10659, 10945, 11067, 11120). So a GSP client never sends `ALLOC_OBJECT`, `ALLOC_CHANNEL_DMA`,
  `ALLOC_SUBDEVICE`, `ALLOC_EVENT`, `ALLOC_SHARE_DEVICE` or any `CTRL_*` function: it sends
  `GSP_RM_ALLOC` (103), `GSP_RM_CONTROL` (76), `FREE` (10) and `DUP_OBJECT` (21) instead.
- **The GSP side.** GSP-RM is built from the same RM core (`RMCFG_FEATURE_PLATFORM_GSP`,
  `src/nvidia/generated/rmconfig.h:266`, 0 in this build), but its RPC dispatcher is not in the
  open tree: the GMCAPI table names the GSP handler of `GSP_INIT` only as
  `gspRpcProcessInitMessage()` (`gmcapi_table.h:87`), and no file of the tree receives
  `GSP_RM_ALLOC`. What GSP-RM does is therefore stated from the physical branches of the shared
  code, from what the kernel reads back, and from the comments at the send sites; each such
  statement says "inferred".

## 2. Summary

| Group | Functions | Device today |
|---|---:|---|
| Sent by a 615 GSP client at every boot on TU106 | 7: `GSP_RM_CONTROL`, `GSP_RM_ALLOC`, `FREE`, `CONTINUATION_RECORD`, `UPDATE_BAR_PDE`, `INIT_GSP_TRACE_CRASH_BUFFER`, `ALLOC_MEMORY` | 6 answered; `ALLOC_MEMORY` refused |
| Sent in regular use | 1: `DUP_OBJECT` (every CUDA and UVM client) | answered |
| Sent on some paths | 6: `IDLE_CHANNELS`, `GPU_EXEC_REG_OPS`, `DUMP_PROTOBUF_COMPONENT`, `ECC_NOTIFIER_WRITE_ACK`, `MAP_MEMORY_DMA`, `UNMAP_MEMORY_DMA` | all 6 refused (`NV_ERR_NOT_SUPPORTED`) |
| Sent by a 610 GSP client, no longer by 615 | 5: `SET_GUEST_SYSTEM_INFO`, `GET_GSP_STATIC_INFO`, `GSP_SET_SYSTEM_INFO`, `SET_REGISTRY`, `UNLOADING_GUEST_DRIVER` | 4 answered (kept for the 610 build); `UNLOADING_GUEST_DRIVER` refused |
| vGPU guest encodings of a control (`CTRL_*`) | 106 | never reached |
| Other vGPU-guest functions | 26 | never reached |
| Macro is an empty inline, never sent | 20 | never reached |
| Reserved or deprecated, no sender | 26 | never reached |
| No sender anywhere in the tree (simulation, UVM paging for vGPU, removed features) | 35 | never reached |
| **Total** | **232** | |

Plus the GMCAPI commands of the CPU-to-GSP direction (section 5): `GSP_INIT` (0x10001, every
boot, answered) and `GSP_SUSPEND` (0x10004, every unload and suspend, answered by the
MAILBOX0 bit). So a 615 GSP client on TU106 uses **14 RPC functions and 2 GMCAPI commands**;
the device answers **7 of the 14 and both GMCAPI commands**. The first 615 boot on the device
(`caraxes/runs/transport-615-2026-09-24/boot/device-events.txt`) shows exactly the seven
every-boot functions plus `GSP_INIT`: 189 `GSP_RM_CONTROL`, 50 `GSP_RM_ALLOC`, 30 `FREE`, one
each of `INIT_GSP_TRACE_CRASH_BUFFER`, `UPDATE_BAR_PDE` and `ALLOC_MEMORY` (logged as `? (4)`
at :517, since `rpc::function_name` does not know the number), and the continuation records of
the 81952-byte SM-order control (:296-299). The seven gaps, in order of
urgency:

1. `ALLOC_MEMORY` (4). New in weight with 615: the GSP operational-event import
   (`gpu_op_event_import.c`, new in 615) registers three sysmem buffers with GSP at every
   state load of a GSP client (`gpu.c:2788-2796`), so the call comes at every boot; the device's
   refusal makes the kernel log `goeimport: step 3a failed (0x56)` and disable the import
   (gsp-transport-615.md §10 item 2). The same RPC registers the USERD memory of a
   `NV50_DEFERRED_API_CLASS` channel-info update (`deferred_api.c:371`), the PMA buffers of a
   profiler (`kern_profiler_v2_ctrl.c:299-302`) and the GSP heap backing store at suspend
   (`mem_mgr_pwr_mgmt.c:390`); there a refusal fails the operation.
2. `ECC_NOTIFIER_WRITE_ACK` (202). The kernel's reply to the new event
   `ECC_POSSIBLE_ERR_FAST_PATH` (0x102A); only after the device learns to send that event.
3. `IDLE_CHANNELS` (22). The `NV0000_CTRL_CMD_IDLE_CHANNELS` / `NV_ESC_RM_IDLE_CHANNELS` path.
4. `DUMP_PROTOBUF_COMPONENT` (78). `nvidia-bug-report` and RC journal dumps.
5. `GPU_EXEC_REG_OPS` (50). Register operations of debug tools.
6. `MAP_MEMORY_DMA` / `UNMAP_MEMORY_DMA` (14/15). Only with the regkey
   `RMSplitVasMgmtServerClientRm=0`.
7. `UNLOADING_GUEST_DRIVER` (47) no longer matters for 615 (only a vGPU guest sends it,
   `objvgpu.c:150`); nova-core still sends it (gsp-surface.md §5).

Estimated work for the six live gaps: 2 to 3 agent-days (section 6).

## 3. All 232 functions

Generated from both trees (section 1). "On a 615 GSP client" is the group of section 2. "Sender
body" is the `rpc<Name>_vXX_YY` function in `rpc.c` (the first version when there are
several). "Device today" cites `crates/vfio-user-nvgpu/src/boot.rs`; every function without an
arm of its own is answered `NV_ERR_NOT_SUPPORTED` by the default arm and noted in `unanswered`
(`boot.rs:1177`). A struct size is given once when 610 and 615 agree.

| Id | Function | Params struct: version, bytes 610 / 615 | Sender body | On a 615 GSP client | Who sends it, when (615 tree) | Device today | Recipe |
|---:|---|---|---|---|---|---|---|
| 0 | `NOP` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 1 | `SET_GUEST_SYSTEM_INFO` | v03_00: 792 | rpc.c:8901 | sent by 610 only | see the detailed section | answered, request echoed, version compared (`crates/vfio-user-nvgpu/src/boot.rs:975`); 610 path only | see the detailed section |
| 2 | `ALLOC_ROOT` | v07_00: 108 | rpc.c:3382 (610: 3337) | vGPU guest | vGPU guest (`rpcAllocRoot_v07_00`, rpc.c:3382); a GSP client allocates NV01_ROOT with GSP_RM_ALLOC | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 3 | `ALLOC_DEVICE` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 4 | `ALLOC_MEMORY` | v13_01: 56 | rpc.c:3486 (610: 3441) | sent (every boot) | see the detailed section | refused `NV_ERR_NOT_SUPPORTED`, logged as unanswered (`crates/vfio-user-nvgpu/src/boot.rs:1177`) | see the detailed section |
| 5 | `ALLOC_CTX_DMA` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 6 | `ALLOC_CHANNEL_DMA` | v1F_04: 248 | rpc.c:3531 (610: 3486) | vGPU guest | vGPU guest; a GSP client allocates channels with GSP_RM_ALLOC (rpc.h:216-240, `IS_FW_CLIENT` branch) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 7 | `MAP_MEMORY` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 8 | `BIND_CTX_DMA` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 9 | `ALLOC_OBJECT` | v25_08: 64; v26_00: 80; v27_00: 80; v29_06: 80; v2B_04: 80; v2C_01: 80 | rpc.c:3961 (610: 3916) | vGPU guest | vGPU guest; the `NV_RM_RPC_ALLOC_OBJECT` call sites (alloc_free.c:925, kernel_channel_group_api.c:460, kern_perfbuffer.c:60) take the `IS_FW_CLIENT` branch of rpc.h:242-261 to GSP_RM_ALLOC | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 10 | `FREE` | v03_00: 16 | rpc.c:3353 (610: 3308) | sent (every boot) | see the detailed section | answered, `Dispatcher::free` (`crates/vfio-user-nvgpu/src/boot.rs:1103`) | see the detailed section |
| 11 | `LOG` | v03_00: 8 | rpc.c:3018 (610: 2978) | vGPU guest | vGPU guest only: kernel_fifo.c:1473 under `IS_VIRTUAL` (610: :1399) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 12 | `ALLOC_VIDMEM` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 13 | `UNMAP_MEMORY` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 14 | `MAP_MEMORY_DMA` | v2C_05: 56 | rpc.c:4350 (610: 4305) | sent (conditional) | see the detailed section | refused `NV_ERR_NOT_SUPPORTED`, logged as unanswered (`crates/vfio-user-nvgpu/src/boot.rs:1177`) | see the detailed section |
| 15 | `UNMAP_MEMORY_DMA` | v2C_05: 48 | rpc.c:4379 (610: 4334) | sent (conditional) | see the detailed section | refused `NV_ERR_NOT_SUPPORTED`, logged as unanswered (`crates/vfio-user-nvgpu/src/boot.rs:1177`) | see the detailed section |
| 16 | `GET_EDID` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 17 | `ALLOC_DISP_CHANNEL` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 18 | `ALLOC_DISP_OBJECT` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 19 | `ALLOC_SUBDEVICE` | v08_01: 40 | rpc.c:4400 (610: 4355) | vGPU guest | vGPU guest; subdevice.c:124 takes the `IS_FW_CLIENT` branch (rpc.h:335-359) to GSP_RM_ALLOC | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 20 | `ALLOC_DYNAMIC_MEMORY` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 21 | `DUP_OBJECT` | v03_00: 28 | rpc.c:4421 (610: 4376) | sent (regular use) | see the detailed section | answered, recorded (`crates/vfio-user-nvgpu/src/boot.rs:1118`) | see the detailed section |
| 22 | `IDLE_CHANNELS` | v03_00: 12 | rpc.c:4488 (610: 4443) | sent (conditional) | see the detailed section | refused `NV_ERR_NOT_SUPPORTED`, logged as unanswered (`crates/vfio-user-nvgpu/src/boot.rs:1177`) | see the detailed section |
| 23 | `ALLOC_EVENT` | v03_00: 28 | rpc.c:4523 (610: 4478) | vGPU guest | vGPU guest; event_api.c:162 takes the `IS_FW_CLIENT` branch (rpc.h:305-333) to GSP_RM_ALLOC | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 24 | `SEND_EVENT` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 25 | `REMAPPER_CONTROL` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 26 | `DMA_CONTROL` (deprecated) | none | rpc.c:4577 (610: 4532) | reserved or deprecated | sender body rpc.c:4577 has no caller outside src/kernel/vgpu | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 27 | `DMA_FILL_PTE_MEM` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 28 | `MANAGE_HW_RESOURCE` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 29 | `BIND_ARBITRARY_CTX_DMA` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 30 | `CREATE_FB_SEGMENT` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 31 | `DESTROY_FB_SEGMENT` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 32 | `ALLOC_SHARE_DEVICE` | v03_00: 64 | rpc.c:3448 (610: 3403) | vGPU guest | vGPU guest; device.c:186 takes the `IS_FW_CLIENT` branch (rpc.h:172-192, :48-89) to two GSP_RM_ALLOCs (NV01_ROOT, NV01_DEVICE_0) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 33 | `DEFERRED_API_CONTROL` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 34 | `REMOVE_DEFERRED_API` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 35 | `SIM_ESCAPE_READ` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 36 | `SIM_ESCAPE_WRITE` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 37 | `SIM_MANAGE_DISPLAY_CONTEXT_DMA` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 38 | `FREE_VIDMEM_VIRT` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 39 | `PERF_GET_PSTATE_INFO` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 40 | `PERF_GET_PERFMON_SAMPLE` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 41 | `PERF_GET_VIRTUAL_PSTATE_INFO` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 42 | `PERF_GET_LEVEL_INFO` | v2B_0D: 788 | rpc.c:9065 (610: 8955) | vGPU guest | vGPU VF only: kern_perf_ctrl.c:222 in `subdeviceCtrlCmdPerfGetLevelInfo_V2_VF` | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 43 | `MAP_SEMA_MEMORY` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 44 | `UNMAP_SEMA_MEMORY` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 45 | `SET_SURFACE_PROPERTIES` | v07_07: 76 | rpc.c:9148 (610: 9038) | vGPU guest | vGPU guest (2 sites under src/kernel/vgpu) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 46 | `CLEANUP_SURFACE` | v03_00: 8 | rpc.c:9220 (610: 9110) | vGPU guest | vGPU guest (1 site under src/kernel/vgpu) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 47 | `UNLOADING_GUEST_DRIVER` | v1F_07: 8 | rpc.c:9256 (610: 9146) | sent by 610 only | see the detailed section | refused `NV_ERR_NOT_SUPPORTED`, logged as unanswered (`crates/vfio-user-nvgpu/src/boot.rs:1177`) | see the detailed section |
| 48 | `TDR_SET_TIMEOUT_STATE` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 49 | `SWITCH_TO_VGA` | none | rpc.c:9376 (610: 9266) | vGPU guest | vGPU guest: device_ctrl.c:306 (NV0080 `GPU_VIRTUALIZATION_SWITCH_TO_VGA`, 610: :283), unix_console.c:243 under `IS_VIRTUAL` | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 50 | `GPU_EXEC_REG_OPS` | v12_01: 56 | rpc.c:9394 (610: 9284) | sent (conditional) | see the detailed section | refused `NV_ERR_NOT_SUPPORTED`, logged as unanswered (`crates/vfio-user-nvgpu/src/boot.rs:1177`) | see the detailed section |
| 51 | `GET_STATIC_INFO` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 52 | `ALLOC_VIRTMEM` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 53 | `UPDATE_PDE_2` | none | - | never (stub macro) | stub macro (rpc_vgpu.h:51); dma.c:326 under `IS_VIRTUAL` | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 54 | `SET_PAGE_DIRECTORY` | v1E_05: 48 | rpc.c:9844 (610: 9734) | vGPU guest (control) | vGPU guest encoding of `NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY` (rpc.c:4745) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 55 | `GET_STATIC_PSTATE_INFO` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 56 | `TRANSLATE_GUEST_GPU_PTES` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 57 | `RESERVED_57` | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 58 | `RESET_CURRENT_GR_CONTEXT` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 59 | `SET_SEMA_MEM_VALIDATION_STATE` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 60 | `GET_ENGINE_UTILIZATION` | v1F_0E: 4048 | rpc.c:10321 (610: 10204) | vGPU guest | vGPU guest: client_resource.c:1936-2221 and kern_perf_ctrl.c:72, :276 under `IS_VIRTUAL` | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 61 | `UPDATE_GPU_PDES` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 62 | `GET_ENCODER_CAPACITY` | v07_00: 12 | rpc.c:10462 (610: 10345) | vGPU guest | vGPU guest: subdevice_ctrl_gpu_kernel.c:1006 under `IS_VIRTUAL` | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 63 | `VGPU_PF_REG_READ32` (deprecated) | v15_00: 16 | rpc.c:10488 (610: 10371) | reserved or deprecated | sender body rpc.c:10488 has no caller outside src/kernel/vgpu | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 64 | `SET_GUEST_SYSTEM_INFO_EXT` | v25_1B: 268 | rpc.c:9023 (610: 8913) | vGPU guest | vGPU guest (rpc.c:9012) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 65 | `GET_GSP_STATIC_INFO` | v14_00: 4 | rpc.c:9779 (610: 9669) | sent by 610 only | see the detailed section | answered with the 610 image (`crates/vfio-user-nvgpu/src/boot.rs:988`); 610 path only | see the detailed section |
| 66 | `RMFS_INIT` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 67 | `RMFS_CLOSE_QUEUE` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 68 | `RMFS_CLEANUP` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 69 | `RMFS_TEST` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 70 | `UPDATE_BAR_PDE` | v15_00: 24 | rpc.c:9819 (610: 9709) | sent (every boot) | see the detailed section | answered, PDE3[0] stored (`crates/vfio-user-nvgpu/src/boot.rs:1147`) | see the detailed section |
| 71 | `CONTINUATION_RECORD` | none | rpc.c:2177 | sent (every boot) | see the detailed section | reassembled on receive (`crates/vfio-user-nvgpu/src/boot.rs:882`), emitted for large replies (`crates/vfio-user-nvgpu/src/boot.rs:1207`) | see the detailed section |
| 72 | `GSP_SET_SYSTEM_INFO` (deprecated from 615) | v17_00: 4, removed | - | sent by 610 only | see the detailed section | consumed, BAR addresses kept (`crates/vfio-user-nvgpu/src/boot.rs:946`); 610 path only | see the detailed section |
| 73 | `SET_REGISTRY` (deprecated from 615) | none | - | sent by 610 only | see the detailed section | consumed, no reply (`crates/vfio-user-nvgpu/src/boot.rs:954`); 610 path only | see the detailed section |
| 74 | `GSP_INIT_POST_OBJGPU` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 75 | `SUBDEV_EVENT_SET_NOTIFICATION` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 76 | `GSP_RM_CONTROL` | v03_00: 40 | rpc.c:10786 | sent (every boot) | see the detailed section | answered, `chip::physical::answer` then `Dispatcher::control` (`crates/vfio-user-nvgpu/src/boot.rs:1003`) | see the detailed section |
| 77 | `GET_STATIC_INFO2` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 78 | `DUMP_PROTOBUF_COMPONENT` | v18_12: 16 | rpc.c:10543 (610: 10547) | sent (conditional) | see the detailed section | refused `NV_ERR_NOT_SUPPORTED`, logged as unanswered (`crates/vfio-user-nvgpu/src/boot.rs:1177`) | see the detailed section |
| 79 | `UNSET_PAGE_DIRECTORY` | v1E_05: 16 | rpc.c:9880 (610: 9770) | vGPU guest (control) | vGPU guest encoding of `NV0080_CTRL_CMD_DMA_UNSET_PAGE_DIRECTORY` (rpc.c:4742) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 80 | `GET_CONSOLIDATED_STATIC_INFO` (deprecated) | none | rpc.c:9482 (610: 9372) | reserved or deprecated | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 81 | `GMMU_REGISTER_FAULT_BUFFER` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 82 | `GMMU_UNREGISTER_FAULT_BUFFER` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 83 | `GMMU_REGISTER_CLIENT_SHADOW_FAULT_BUFFER` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 84 | `GMMU_UNREGISTER_CLIENT_SHADOW_FAULT_BUFFER` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 85 | `CTRL_SET_VGPU_FB_USAGE` | v1A_08: 8 | rpc.c:5601 (610: 5538) | vGPU guest (control) | vGPU guest encoding of `NVA080_CTRL_CMD_SET_FB_USAGE` (rpc.c:4609) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 86 | `CTRL_NVFBC_SW_SESSION_UPDATE_INFO` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 87 | `CTRL_NVENC_SW_SESSION_UPDATE_INFO` | v1A_09: 40 | rpc.c:5633 (610: 5570) | vGPU guest (control) | vGPU guest encoding of `NVA0BC_CTRL_CMD_NVENC_SW_SESSION_UPDATE_INFO` (rpc.c:4612) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 88 | `CTRL_RESET_CHANNEL` | v1A_09: 20 | rpc.c:5665 (610: 5602) | vGPU guest (control) | vGPU guest encoding of `NV906F_CTRL_CMD_RESET_CHANNEL` (rpc.c:4615) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 89 | `CTRL_RESET_ISOLATED_CHANNEL` | v1A_09: 16 | rpc.c:5697 (610: 5634) | vGPU guest (control) | vGPU guest encoding of `NV506F_CTRL_CMD_RESET_ISOLATED_CHANNEL` (rpc.c:4618) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 90 | `CTRL_GPU_HANDLE_VF_PRI_FAULT` | v1A_09: 12 | rpc.c:5729 (610: 5666) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GPU_HANDLE_VF_PRI_FAULT` (rpc.c:4621) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 91 | `CTRL_CLK_GET_EXTENDED_INFO` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 92 | `CTRL_PERF_BOOST` | v1A_09: 16 | rpc.c:5761 (610: 5698) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_PERF_BOOST` (rpc.c:4624) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 93 | `CTRL_PERF_VPSTATES_GET_CONTROL` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 94 | `CTRL_GET_ZBC_CLEAR_TABLE` | v1A_09: 64 | rpc.c:5793 (610: 5730) | vGPU guest (control) | vGPU guest encoding of `NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE` (rpc.c:4627) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 95 | `CTRL_SET_ZBC_COLOR_CLEAR` | v1A_09: 44 | rpc.c:5825 (610: 5762) | vGPU guest (control) | vGPU guest encoding of `NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR` (rpc.c:4630) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 96 | `CTRL_SET_ZBC_DEPTH_CLEAR` | v1A_09: 16 | rpc.c:5857 (610: 5794) | vGPU guest (control) | vGPU guest encoding of `NV9096_CTRL_CMD_SET_ZBC_DEPTH_CLEAR` (rpc.c:4633) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 97 | `CTRL_GPFIFO_SCHEDULE` | v1A_0A: 16 | rpc.c:5921 (610: 5858) | vGPU guest (control) | vGPU guest encoding of `NVA06C_CTRL_CMD_GPFIFO_SCHEDULE` (rpc.c:4639), `NVA06F_CTRL_CMD_GPFIFO_SCHEDULE` (rpc.c:4640) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 98 | `CTRL_SET_TIMESLICE` | v1A_0A: 16 | rpc.c:5954 (610: 5891) | vGPU guest (control) | vGPU guest encoding of `NVA06C_CTRL_CMD_SET_TIMESLICE` (rpc.c:4643) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 99 | `CTRL_PREEMPT` | v1A_0A: 16 | rpc.c:6088 (610: 5957) | vGPU guest (control) | vGPU guest encoding of `NVA06C_CTRL_CMD_PREEMPT` (rpc.c:4652) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 100 | `CTRL_FIFO_DISABLE_CHANNELS` | v1A_0A: 544 | rpc.c:5986 (610: 5923) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_FIFO_DISABLE_CHANNELS` (rpc.c:4646) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 101 | `CTRL_SET_TSG_INTERLEAVE_LEVEL` | v1A_0A: 12 | rpc.c:6120 (610: 5989) | vGPU guest (control) | vGPU guest encoding of `NVA06C_CTRL_CMD_SET_INTERLEAVE_LEVEL` (rpc.c:4655) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 102 | `CTRL_SET_CHANNEL_INTERLEAVE_LEVEL` | v1A_0A: 12 | rpc.c:6152 (610: 6021) | vGPU guest (control) | vGPU guest encoding of `NVA06F_CTRL_CMD_SET_INTERLEAVE_LEVEL` (rpc.c:4658) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 103 | `GSP_RM_ALLOC` | v03_00: 32 | rpc.c:10998 | sent (every boot) | see the detailed section | answered, `Dispatcher::alloc` (`crates/vfio-user-nvgpu/src/boot.rs:1046`) | see the detailed section |
| 104 | `CTRL_GET_P2P_CAPS_V2` | v1F_0D: 2208 | rpc.c:6382 (610: 6251) | vGPU guest (control) | vGPU guest encoding of `NV0000_CTRL_CMD_SYSTEM_GET_P2P_CAPS_V2` (rpc.c:4833) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 105 | `CTRL_CIPHER_AES_ENCRYPT` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 106 | `CTRL_CIPHER_SESSION_KEY` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 107 | `CTRL_CIPHER_SESSION_KEY_STATUS` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 108 | `CTRL_DBG_CLEAR_ALL_SM_ERROR_STATES` | v1A_0C: 16 | rpc.c:7375 (610: 7244) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_CLEAR_ALL_SM_ERROR_STATES` (rpc.c:4661) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 109 | `CTRL_DBG_READ_ALL_SM_ERROR_STATES` | v21_06: 3872 | rpc.c:7431 (610: 7300) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_READ_ALL_SM_ERROR_STATES` (rpc.c:4848) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 110 | `CTRL_DBG_SET_EXCEPTION_MASK` | v1A_0C: 12 | rpc.c:7403 (610: 7272) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_SET_EXCEPTION_MASK` (rpc.c:4664) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 111 | `CTRL_GPU_PROMOTE_CTX` | v1A_20: 568 | rpc.c:6494 (610: 6363) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GPU_PROMOTE_CTX` (rpc.c:4667) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 112 | `CTRL_GR_CTXSW_PREEMPTION_BIND` | v28_07: 120 | rpc.c:6184 (610: 6053) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND` (rpc.c:4670) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 113 | `CTRL_GR_SET_CTXSW_PREEMPTION_MODE` | v1A_0E: 40 | rpc.c:6240 (610: 6109) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE` (rpc.c:4673) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 114 | `CTRL_GR_CTXSW_ZCULL_BIND` | v1A_0E: 32 | rpc.c:6268 (610: 6137) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GR_CTXSW_ZCULL_BIND` (rpc.c:4676) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 115 | `CTRL_GPU_INITIALIZE_CTX` | v1A_0E: 64 | rpc.c:6296 (610: 6165) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GPU_INITIALIZE_CTX` (rpc.c:4679) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 116 | `CTRL_VASPACE_COPY_SERVER_RESERVED_PDES` | v1E_04: 192 | rpc.c:6324 (610: 6193) | vGPU guest (control) | vGPU guest encoding of `NV90F1_CTRL_CMD_VASPACE_COPY_SERVER_RESERVED_PDES` (rpc.c:4685) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 117 | `CTRL_FIFO_CLEAR_FAULTED_BIT` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 118 | `CTRL_GET_LATEST_ECC_ADDRESSES` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 119 | `CTRL_MC_SERVICE_INTERRUPTS` | v1A_0E: 12 | rpc.c:6354 (610: 6223) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_MC_SERVICE_INTERRUPTS` (rpc.c:4688) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 120 | `CTRL_DMA_SET_DEFAULT_VASPACE` | v1A_0E: 12 | rpc.c:8086 (610: 7955) | vGPU guest (control) | vGPU guest encoding of `NV0080_CTRL_DMA_SET_DEFAULT_VASPACE` (rpc.c:4691) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 121 | `CTRL_GET_CE_PCE_MASK` | v1A_0E: 16 | rpc.c:8114 (610: 7983) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_CE_GET_CE_PCE_MASK` (rpc.c:4694) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 122 | `CTRL_GET_ZBC_CLEAR_TABLE_ENTRY` | v1A_0E: 64 | rpc.c:8142 (610: 8011) | vGPU guest (control) | vGPU guest encoding of `NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY` (rpc.c:4697) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 123 | `CTRL_GET_NVLINK_PEER_ID_MASK` (deprecated) | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 124 | `CTRL_GET_NVLINK_STATUS` | v28_09: 3472; v2B_11: 3488; v2D_01: 5216 | rpc.c:8170 (610: 8039) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_NVLINK_GET_NVLINK_STATUS` (rpc.c:4700) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 125 | `CTRL_GET_P2P_CAPS` | v1F_0D: 164 | rpc.c:8299 (610: 8168) | vGPU guest (control) | vGPU guest encoding of `NV0000_CTRL_CMD_SYSTEM_GET_P2P_CAPS` (rpc.c:4830) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 126 | `CTRL_GET_P2P_CAPS_MATRIX` | v1A_0E: 1360 | rpc.c:8327 (610: 8196) | vGPU guest (control) | vGPU guest encoding of `NV0000_CTRL_CMD_SYSTEM_GET_P2P_CAPS_MATRIX` (rpc.c:4703) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 127 | `RESERVED_0` | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 128 | `CTRL_RESERVE_PM_AREA_SMPC` | v1A_0F: 12 | rpc.c:6526 (610: 6395) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_RESERVE_PM_AREA_SMPC` (rpc.c:4706) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 129 | `CTRL_RESERVE_HWPM_LEGACY` | v1A_0F: 12 | rpc.c:6558 (610: 6427) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_RESERVE_HWPM_LEGACY` (rpc.c:4709) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 130 | `CTRL_B0CC_EXEC_REG_OPS` | v1A_0F: 3988 | rpc.c:6590 (610: 6459) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_EXEC_REG_OPS` (rpc.c:4712) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 131 | `CTRL_BIND_PM_RESOURCES` | v1A_0F: 8 | rpc.c:6625 (610: 6494) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_BIND_PM_RESOURCES` (rpc.c:4715) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 132 | `CTRL_DBG_SUSPEND_CONTEXT` | v1A_10: 16 | rpc.c:7842 (610: 7711) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_SUSPEND_CONTEXT` (rpc.c:4718) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 133 | `CTRL_DBG_RESUME_CONTEXT` | v1A_10: 8 | rpc.c:7870 (610: 7739) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_RESUME_CONTEXT` (rpc.c:4721) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 134 | `CTRL_DBG_EXEC_REG_OPS` | v1A_10: 3216 | rpc.c:7890 (610: 7759) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_EXEC_REG_OPS` (rpc.c:4724) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 135 | `CTRL_DBG_SET_MODE_MMU_DEBUG` | v1A_10: 12 | rpc.c:7918 (610: 7787) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_SET_MODE_MMU_DEBUG` (rpc.c:4727) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 136 | `CTRL_DBG_READ_SINGLE_SM_ERROR_STATE` | v21_06: 64 | rpc.c:8002 (610: 7871) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_READ_SINGLE_SM_ERROR_STATE` (rpc.c:4851) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 137 | `CTRL_DBG_CLEAR_SINGLE_SM_ERROR_STATE` | v1A_10: 16 | rpc.c:7974 (610: 7843) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_CLEAR_SINGLE_SM_ERROR_STATE` (rpc.c:4733) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 138 | `CTRL_DBG_SET_MODE_ERRBAR_DEBUG` | v1A_10: 12 | rpc.c:8030 (610: 7899) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_SET_MODE_ERRBAR_DEBUG` (rpc.c:4736) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 139 | `CTRL_DBG_SET_NEXT_STOP_TRIGGER_TYPE` | v1A_10: 12 | rpc.c:8058 (610: 7927) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_SET_NEXT_STOP_TRIGGER_TYPE` (rpc.c:4739) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 140 | `CTRL_ALLOC_PMA_STREAM` | v1A_14: 64 | rpc.c:6650 (610: 6519) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_ALLOC_PMA_STREAM` (rpc.c:4748), `NVB0CC_CTRL_CMD_INTERNAL_ALLOC_PMA_STREAM` (rpc.c:4749) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 141 | `CTRL_PMA_STREAM_UPDATE_GET_PUT` | v29_0B: 56 | rpc.c:6682 (610: 6551) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_PMA_STREAM_UPDATE_GET_PUT` (rpc.c:4752) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 142 | `CTRL_FB_GET_INFO_V2` | v27_00: 468; v2B_00: 1036 | rpc.c:8355 (610: 8224) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_FB_GET_INFO_V2` (rpc.c:4755) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 143 | `CTRL_FIFO_SET_CHANNEL_PROPERTIES` | v1A_16: 24 | rpc.c:8483 (610: 8352) | vGPU guest | sender body rpc.c:8483 has no caller outside src/kernel/vgpu | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 144 | `CTRL_GR_GET_CTX_BUFFER_INFO` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 145 | `CTRL_KGR_GET_CTX_BUFFER_PTES` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 146 | `CTRL_GPU_EVICT_CTX` | v1A_1C: 28 | rpc.c:4966 (610: 4921) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GPU_EVICT_CTX` (rpc.c:4758) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 147 | `CTRL_FB_GET_FS_INFO` | v26_04: 3856; v2B_07: 3856 | rpc.c:4994 (610: 4949) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_FB_GET_FS_INFO` (rpc.c:4764) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 148 | `CTRL_GRMGR_GET_GR_FS_INFO` | v1A_1D: 1936; v2B_09: 1936 | rpc.c:5060 (610: 5015) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GRMGR_GET_GR_FS_INFO` (rpc.c:4767) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 149 | `CTRL_STOP_CHANNEL` | v1A_1E: 12 | rpc.c:4938 (610: 4893) | vGPU guest (control) | vGPU guest encoding of `NVA06F_CTRL_CMD_STOP_CHANNEL` (rpc.c:4761) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 150 | `CTRL_GR_PC_SAMPLING_MODE` | v1A_1F: 32 | rpc.c:6714 (610: 6583) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GR_PC_SAMPLING_MODE` (rpc.c:4770) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 151 | `CTRL_PERF_RATED_TDP_GET_STATUS` | v1A_1F: 40 | rpc.c:6746 (610: 6615) | vGPU guest | sender body rpc.c:6746 has no caller outside src/kernel/vgpu | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 152 | `CTRL_PERF_RATED_TDP_SET_CONTROL` | v1A_1F: 16 | rpc.c:6780 (610: 6649) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_PERF_RATED_TDP_SET_CONTROL` (rpc.c:4773) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 153 | `CTRL_FREE_PMA_STREAM` | v1A_1F: 12 | rpc.c:6812 (610: 6681) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_FREE_PMA_STREAM` (rpc.c:4776) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 154 | `CTRL_TIMER_SET_GR_TICK_FREQ` | v1A_1F: 12 | rpc.c:6844 (610: 6713) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_TIMER_SET_GR_TICK_FREQ` (rpc.c:4779) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 155 | `CTRL_FIFO_SETUP_VF_ZOMBIE_SUBCTX_PDB` | v1A_23: 32 | rpc.c:6882 (610: 6751) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_FIFO_SETUP_VF_ZOMBIE_SUBCTX_PDB` (rpc.c:4782) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 156 | `GET_CONSOLIDATED_GR_STATIC_INFO` | v1B_04: 8 | rpc.c:9693 (610: 9583) | vGPU guest | vGPU guest (rpc_vgpu.h:112, 4 sites under src/kernel/vgpu) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 157 | `CTRL_DBG_SET_SINGLE_SM_SINGLE_STEP` | v1C_02: 16 | rpc.c:6913 (610: 6782) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_SET_SINGLE_SM_SINGLE_STEP` (rpc.c:4785) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 158 | `CTRL_GR_GET_TPC_PARTITION_MODE` | v1C_04: 40 | rpc.c:6945 (610: 6814) | vGPU guest (control) | vGPU guest encoding of `NV0080_CTRL_CMD_GR_GET_TPC_PARTITION_MODE` (rpc.c:4788) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 159 | `CTRL_GR_SET_TPC_PARTITION_MODE` | v1C_04: 40 | rpc.c:6977 (610: 6846) | vGPU guest (control) | vGPU guest encoding of `NV0080_CTRL_CMD_GR_SET_TPC_PARTITION_MODE` (rpc.c:4791) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 160 | `UVM_PAGING_CHANNEL_ALLOCATE` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 161 | `UVM_PAGING_CHANNEL_DESTROY` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 162 | `UVM_PAGING_CHANNEL_MAP` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 163 | `UVM_PAGING_CHANNEL_UNMAP` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 164 | `UVM_PAGING_CHANNEL_PUSH_STREAM` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 165 | `UVM_PAGING_CHANNEL_SET_HANDLES` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 166 | `UVM_METHOD_STREAM_GUEST_PAGES_OPERATION` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 167 | `CTRL_INTERNAL_QUIESCE_PMA_CHANNEL` | v1C_08: 16 | rpc.c:7311 (610: 7180) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_INTERNAL_QUIESCE_PMA_CHANNEL` (rpc.c:4794) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 168 | `DCE_RM_INIT` | v01_00: 20 | - | vGPU guest | DCE client only (Tegra display): dce_client_rpc.c:145 | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 169 | `REGISTER_VIRTUAL_EVENT_BUFFER` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 170 | `CTRL_EVENT_BUFFER_UPDATE_GET` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 171 | `GET_PLCABLE_ADDRESS_KIND` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 172 | `CTRL_PERF_LIMITS_SET_STATUS_V2` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 173 | `CTRL_INTERNAL_SRIOV_PROMOTE_PMA_STREAM` | v1C_0C: 56 | rpc.c:7343 (610: 7212) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_INTERNAL_SRIOV_PROMOTE_PMA_STREAM` (rpc.c:4797) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 174 | `CTRL_GET_MMU_DEBUG_MODE` | v1E_06: 12 | rpc.c:8451 (610: 8320) | vGPU guest (control) | vGPU guest encoding of `NV0090_CTRL_CMD_GET_MMU_DEBUG_MODE` (rpc.c:4800) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 175 | `CTRL_INTERNAL_PROMOTE_FAULT_METHOD_BUFFERS` | v1E_07: 96 | rpc.c:7280 | vGPU guest (control) | vGPU guest encoding of `NVA06C_CTRL_CMD_INTERNAL_PROMOTE_FAULT_METHOD_BUFFERS` (rpc.c:4803) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 176 | `CTRL_FLCN_GET_CTX_BUFFER_SIZE` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 177 | `CTRL_FLCN_GET_CTX_BUFFER_INFO` | none | - | no sender | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 178 | `DISABLE_CHANNELS` | v1E_0B: 4 | rpc.c:10498 (610: 10381) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_INTERNAL_FIFO_TOGGLE_ACTIVE_CHANNEL_SCHEDULING` (rpc.c:4915) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 179 | `CTRL_FABRIC_MEMORY_DESCRIBE` | v1E_0C: 2080 | rpc.c:7173 (610: 7042) | vGPU guest (control) | vGPU guest encoding of `NV00F8_CTRL_CMD_DESCRIBE` (rpc.c:4806) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 180 | `CTRL_FABRIC_MEM_STATS` | v1E_0C: 24 | rpc.c:7218 (610: 7087) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_FLA_GET_FABRIC_MEM_STATS` (rpc.c:4809) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 181 | `SAVE_HIBERNATION_DATA` | v1E_0E: 4 | rpc.c:10205 (610: 10095) | vGPU guest | vGPU guest: gpu.c:3952 under `IS_VIRTUAL` | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 182 | `RESTORE_HIBERNATION_DATA` | v1E_0E: 4 | rpc.c:10274 (610: 10164) | vGPU guest | vGPU guest: gpu.c:2592 under `IS_VIRTUAL` | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 183 | `CTRL_INTERNAL_MEMSYS_SET_ZBC_REFERENCED` | v1F_05: 12; v2A_00: 12; v2B_0E: 16; v2E_08: 12 | rpc.c:7009 (610: 6878) | vGPU guest (control) | vGPU guest encoding of `NV0080_CTRL_CMD_INTERNAL_MEMSYS_SET_ZBC_REFERENCED` (rpc.c:4812), `NV2080_CTRL_CMD_INTERNAL_MEMSYS_SET_ZBC_REFERENCED` (rpc.c:4815) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 184 | `CTRL_EXEC_PARTITIONS_CREATE` | v24_05: 436; v2E_01: 468 | rpc.c:7479 (610: 7348) | vGPU guest (control) | vGPU guest encoding of `NVC637_CTRL_CMD_EXEC_PARTITIONS_CREATE` (rpc.c:4897) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 185 | `CTRL_EXEC_PARTITIONS_DELETE` | v1F_0A: 44 | rpc.c:7585 (610: 7454) | vGPU guest (control) | vGPU guest encoding of `NVC637_CTRL_CMD_EXEC_PARTITIONS_DELETE` (rpc.c:4818) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 186 | `CTRL_GPFIFO_GET_WORK_SUBMIT_TOKEN` | v1F_0A: 12 | rpc.c:7710 (610: 7579) | vGPU guest (control) | vGPU guest encoding of `NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN` (rpc.c:4821) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 187 | `CTRL_GPFIFO_SET_WORK_SUBMIT_TOKEN_NOTIF_INDEX` | v1F_0A: 12 | rpc.c:7751 (610: 7620) | vGPU guest (control) | vGPU guest encoding of `NVC36F_CTRL_CMD_GPFIFO_SET_WORK_SUBMIT_TOKEN_NOTIF_INDEX` (rpc.c:4824) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 188 | `PMA_SCRUBBER_SHARED_BUFFER_GUEST_PAGES_OPERATION` | none | - | never (stub macro) | the macro is an empty inline (rpc_vgpu.h:37-56 or rpc.h:418-482) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 189 | `CTRL_MASTER_GET_VIRTUAL_FUNCTION_ERROR_CONT_INTR_MASK` | v1F_0D: 16 | rpc.c:7792 (610: 7661) | vGPU guest (control) | vGPU guest encoding of `NV90E6_CTRL_CMD_MASTER_GET_VIRTUAL_FUNCTION_ERROR_CONT_INTR_MASK` (rpc.c:4827) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 190 | `RESERVED_190` | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 191 | `CTRL_SUBDEVICE_GET_P2P_CAPS` | v21_02: 1544 | rpc.c:6469 (610: 6338) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GET_P2P_CAPS` (rpc.c:4836) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 192 | `CTRL_BUS_SET_P2P_MAPPING` | v29_08: 48 | rpc.c:8490 (610: 8380) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_BUS_SET_P2P_MAPPING` (rpc.c:4839) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 193 | `CTRL_BUS_UNSET_P2P_MAPPING` | v21_03: 40 | rpc.c:8518 (610: 8408) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_BUS_UNSET_P2P_MAPPING` (rpc.c:4842) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 194 | `CTRL_FLA_SETUP_INSTANCE_MEM_BLOCK` | v21_05: 24 | rpc.c:5126 (610: 5081) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_FLA_SETUP_INSTANCE_MEM_BLOCK` (rpc.c:4845) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 195 | `CTRL_GPU_MIGRATABLE_OPS` | v21_07: 1840 | rpc.c:8746 (610: 8636) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GPU_MIGRATABLE_OPS` (rpc.c:4854) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 196 | `CTRL_GET_TOTAL_HS_CREDITS` | v21_08: 12 | rpc.c:5163 (610: 5118) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_GET_TOTAL_HS_CREDITS` (rpc.c:4857) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 197 | `CTRL_GET_HS_CREDITS` | v21_08: 264 | rpc.c:5192 (610: 5147) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_GET_HS_CREDITS` (rpc.c:4860) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 198 | `CTRL_SET_HS_CREDITS` | v21_08: 264 | rpc.c:5396 (610: 5351) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_SET_HS_CREDITS` (rpc.c:4881) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 199 | `CTRL_PM_AREA_PC_SAMPLER` | v21_0B: 12 | rpc.c:5425 (610: 5380) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_RESERVE_PM_AREA_PC_SAMPLER` (rpc.c:4890), `NVB0CC_CTRL_CMD_RELEASE_PM_AREA_PC_SAMPLER` (rpc.c:4891) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 200 | `INVALIDATE_TLB` | v23_03: 16 | rpc.c:10520 (610: 10403) | vGPU guest | vGPU guest only: kern_gmmu_gm107.c:249 when `bDoVgpuRpc`, set only with a vGPU object and the VF_INVALIDATE_TLB_TRAP cap (:149-160); same in 610 | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 201 | `CTRL_GPU_QUERY_ECC_STATUS` (deprecated) | v24_06: 1016; v26_02: 1216 | rpc.c:8774 (610: 8664) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS` (rpc.c:4894) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 202 | `ECC_NOTIFIER_WRITE_ACK` | none | rpc.c:11166 (610: 11170) | sent (conditional) | see the detailed section | refused `NV_ERR_NOT_SUPPORTED`, logged as unanswered (`crates/vfio-user-nvgpu/src/boot.rs:1177`) | see the detailed section |
| 203 | `CTRL_DBG_GET_MODE_MMU_DEBUG` | v25_04: 12 | rpc.c:8784 (610: 8674) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_GET_MODE_MMU_DEBUG` (rpc.c:4903) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 204 | `RM_API_CONTROL` | v29_09: 40; v2A_08: 40; v2C_04: 40; v2C_06: 40; v2E_05: 40; v2F_01: new, 40; v2F_03: new, 40; v2F_05: new, 40 | rpc.c:5494 (610: 5449) | vGPU guest | vGPU guest: binary_api.c:90 under `IS_VIRTUAL` (`NV_RM_RPC_API_CONTROL`, rpc_vgpu.h:171) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 205 | `CTRL_CMD_INTERNAL_GPU_START_FABRIC_PROBE` | v25_09: 1 | rpc.c:11186 (610: 11190) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_INTERNAL_GPU_START_FABRIC_PROBE` (rpc.c:4909) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 206 | `CTRL_NVLINK_GET_INBAND_RECEIVED_DATA` | v25_0C: 520 | rpc.c:11208 (610: 11212) | vGPU guest | vGPU guest: 2 site(s) under src/kernel/vgpu | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 207 | `GET_STATIC_DATA` | v27_01: 8; v2D_00: 8 | rpc.c:9539 | vGPU guest | vGPU guest (rpc_vgpu.h:104) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 208 | `RESERVED_208` | none | - | reserved or deprecated | no sender in the tree | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal |
| 209 | `CTRL_GPU_GET_INFO_V2` | v25_11: 532; v2A_04: 540; v2B_03: 548; v2B_05: 556; v2B_0C: 564; v2B_13: 572; v2E_0C: 588 | rpc.c:8546 (610: 8436) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GPU_GET_INFO_V2` (rpc.c:4884) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 210 | `GET_BRAND_CAPS` | v25_12: 4 | rpc.c:5542 (610: 5479) | vGPU guest (control) | vGPU guest encoding of `NV0080_CTRL_CMD_GPU_GET_BRAND_CAPS` (rpc.c:4887) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 211 | `CTRL_CMD_NVLINK_INBAND_SEND_DATA` | v26_05: 1028 | rpc.c:11258 (610: 11262) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_NVLINK_INBAND_SEND_DATA` (rpc.c:4912) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 212 | `UPDATE_GPM_GUEST_BUFFER_INFO` | v27_01: 24; v2B_07: 24 | rpc.c:8411 (610: 8280) | vGPU guest | vGPU guest: kernel_ccu.c:724 in `_kccuVgpuShrBufInfoToCcu` | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 213 | `CTRL_CMD_INTERNAL_CONTROL_GSP_TRACE` | v28_00: 32 | rpc.c:11282 (610: 11286) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_INTERNAL_CONTROL_GSP_TRACE` (rpc.c:4918) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 214 | `CTRL_SET_ZBC_STENCIL_CLEAR` | v27_06: 20 | rpc.c:5889 (610: 5826) | vGPU guest (control) | vGPU guest encoding of `NV9096_CTRL_CMD_SET_ZBC_STENCIL_CLEAR` (rpc.c:4636) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 215 | `CTRL_SUBDEVICE_GET_VGPU_HEAP_STATS` | v28_06: 48 | rpc.c:11304 (610: 11308) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GSP_GET_VGPU_HEAP_STATS` (rpc.c:4921) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 216 | `CTRL_SUBDEVICE_GET_LIBOS_HEAP_STATS` | v29_02: 1048 | rpc.c:11344 (610: 11348) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GSP_GET_LIBOS_HEAP_STATS` (rpc.c:4924) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 217 | `CTRL_DBG_SET_MODE_MMU_GCC_DEBUG` | v2A_05: 12 | rpc.c:7946 (610: 7815) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_SET_MODE_MMU_GCC_DEBUG` (rpc.c:4730) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 218 | `CTRL_DBG_GET_MODE_MMU_GCC_DEBUG` | v2A_05: 12 | rpc.c:8812 (610: 8702) | vGPU guest (control) | vGPU guest encoding of `NV83DE_CTRL_CMD_DEBUG_GET_MODE_MMU_GCC_DEBUG` (rpc.c:4906) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 219 | `CTRL_RESERVE_HES` | v29_07: 16 | rpc.c:5221 (610: 5176) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_RESERVE_HES` (rpc.c:4863) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 220 | `CTRL_RELEASE_HES` | v29_07: 12 | rpc.c:5248 (610: 5203) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_RELEASE_HES` (rpc.c:4866) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 221 | `CTRL_RESERVE_CCU_PROF` | v29_07: 12 | rpc.c:5275 (610: 5230) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_RESERVE_CCU_PROF` (rpc.c:4869) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 222 | `CTRL_RELEASE_CCU_PROF` | v29_07: 8 | rpc.c:5304 (610: 5259) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_RELEASE_CCU_PROF` (rpc.c:4872) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 223 | `SETUP_HIBERNATION_BUFFER` | v2A_06: 8 | rpc.c:10286 (610: 10176) | vGPU guest | vGPU guest (rpc_vgpu.h:144) | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal (a GSP client never sends it) |
| 224 | `CTRL_CMD_GET_CHIPLET_HS_CREDIT_POOL` | v29_0A: 132 | rpc.c:5327 (610: 5282) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_GET_CHIPLET_HS_CREDIT_POOL` (rpc.c:4875) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 225 | `CTRL_CMD_GET_HS_CREDITS_MAPPING` | v29_0A: 204 | rpc.c:5360 (610: 5315) | vGPU guest (control) | vGPU guest encoding of `NVB0CC_CTRL_CMD_GET_HS_CREDITS_MAPPING` (rpc.c:4878) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 226 | `CTRL_EXEC_PARTITIONS_EXPORT` | v29_0C: 112; v2E_01: 120 | rpc.c:7626 (610: 7495) | vGPU guest (control) | vGPU guest encoding of `NVC637_CTRL_CMD_EXEC_PARTITIONS_EXPORT` (rpc.c:4900) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 227 | `CTRL_CMD_INTERNAL_GPU_CHECK_CTS_ID_VALID` | v2B_12: 28 | rpc.c:11382 (610: 11386) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_INTERNAL_GPU_CHECK_CTS_ID_VALID` (rpc.c:4927) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 228 | `INIT_GSP_TRACE_CRASH_BUFFER` | v03_00: 16 | rpc.c:11422 (610: 11426) | sent (every boot) | see the detailed section | answered, PA and size recorded (`crates/vfio-user-nvgpu/src/boot.rs:1138`) | see the detailed section |
| 229 | `CTRL_GPU_SET_MIGRATION_BLOCK` | v2E_0B: 12 | rpc.c:6212 (610: 6081) | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_GPU_SET_MIGRATION_BLOCK` (rpc.c:4682) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 230 | `CTRL_FIFO_VGPU_SWRUNLIST_SUBMIT` | v2F_02: new, 24 | rpc.c:6021 | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_FIFO_VGPU_SWRUNLIST_SUBMIT` (rpc.c:4648) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |
| 231 | `CTRL_FIFO_VGPU_SWRUNLIST_CHANNEL_UPDATE` | v2F_02: new, 24 | rpc.c:6054 | vGPU guest (control) | vGPU guest encoding of `NV2080_CTRL_CMD_FIFO_VGPU_SWRUNLIST_CHANNEL_UPDATE` (rpc.c:4650) in `rpcDmaControl_wrapper`; a GSP client sends the control as GSP_RM_CONTROL | refused like any unknown function (boot.rs:1177); never reached | keep the generic refusal; the control is in the controls plan |

## 4. The functions a GSP client sends, in detail

Each entry: payload, sender and trigger (615, with 610 where different), what GSP-RM does
(inferred where the open tree has no physical code), what the device does, and the recipe
for a full answer with its truth source and isolation risk.

### 4.1 Sent at every boot, or in regular use

**`GSP_RM_CONTROL` (76).** Payload `rpc_gsp_rm_control_v03_00`, 40 bytes plus the parameters
(`g_rpc-structures.h:1452`; 610 :1423), unchanged. Sender `rpcRmApiControl_GSP`
(`rpc.c:10655`, header at :10786): every control the kernel routes to physical RM
(`ROUTE_TO_PHYSICAL`, or sent by the kernel itself), with FINN serialization of embedded
pointers (`serverSerializeCtrlDown`, :10737) and the control cache for cacheable
controls (:10755-10770). GSP-RM runs the control through its resource server on the object
named by `hClient`/`hObject`. Device: answered (`boot.rs:1003`), per control by
`chip::physical::answer` and `ctrl.rs` `Dispatcher::control`. Recipe: the controls plan; at
this level only one rule: a reply must keep `rmctrlFlags` of GSS-legacy commands at 0
(`boot.rs:1035-1040`), and a reply bigger than one element goes out as continuation records.
Truth source: per control. Isolation: per control.

**`GSP_RM_ALLOC` (103).** Payload `rpc_gsp_rm_alloc_v03_00`, 32 bytes plus the alloc
parameters (`g_rpc-structures.h:1437`; 610 :1408). Sender `rpcRmApiAlloc_GSP` (`rpc.c:10941`,
header :10998): every class with `RS_FLAGS_ALLOC_RPC_TO_PHYS_RM` and every constructor that
calls a physical-RM alloc. GSP-RM constructs the same object under the same handles. Device:
answered (`boot.rs:1046`), a new channel's USERD cleared as GSP-RM does. Recipe:
[classes.md](classes.md).

**`FREE` (10).** Payload `rpc_free_v03_00` (an `NVOS00_PARAMETERS`), 16 bytes
(`g_rpc-structures.h:162`), unchanged. Sender `rpcRmApiFree_GSP` (`rpc.c:11116`, header
:11138) for every freed object that was allocated on GSP, and `_memUnregisterFromGsp`
(`mem.c:608`) for memory registered with `ALLOC_MEMORY`. GSP-RM frees the object and its
children (inferred: resource-server semantics of the shared `serverFreeResourceTree`).
Device: answered (`boot.rs:1103`), `Dispatcher::free`, status 0. Recipe: as today; add the
release of registered memory (4.1 `ALLOC_MEMORY`) and of host mirror objects under the freed
handle. Truth source: guest state. Isolation: low (handles are the guest's own; the host
side is freed only for the device's own mirror objects).

**`DUP_OBJECT` (21).** Payload `rpc_dup_object_v03_00`, 28 bytes (`g_rpc-structures.h:200`).
Sender `rpcRmApiDupObject_GSP` (`rpc.c:11063`, header :11089), reached through
`NV_RM_RPC_DUP_OBJECT` with `IS_FW_CLIENT` (`rpc.h:361-388`) from TSG, ctxshare, vaspace,
subscription and memory dups (`kernel_channel_group_api.c:789`, `kernel_ctxshare.c:303`,
`vaspace_api.c`, ...). GSP-RM duplicates the reference into the destination client.
Device: answered and recorded (`boot.rs:1118`). Recipe: as today; when the source is a
mirrored host object, dup the host object too (mirror.rs), so that a later control on the
dup reaches the same host object. Truth source: guest state. Isolation: low.

**`CONTINUATION_RECORD` (71).** No struct: the payload continues the previous record.
Sender `_issueRpcLarge` (`rpc.c:2111`, function set at :2177; 610 :2058, :2124) for any RPC
above `maxRpcSize` (65512 bytes in 615, gsp-transport-615.md §3.1); the kernel also receives
large replies this way (`rpc.c:2245`). Device: reassembled for `GSP_RM_CONTROL` and
`GSP_RM_ALLOC` (`boot.rs:882-933`), emitted for large replies (`boot.rs:1186-1223`). Gap: a
large `ALLOC_MEMORY` (a page list above about 8000 pages, `_issuePteDescRpc`, `rpc.c:2313`)
also arrives split; the reassembly's size rule (`boot.rs:907-919`) knows only control and
alloc and must learn `rpc_alloc_memory_v13_01.pteDesc.length`. Truth source: pure emulation.
Isolation: none.

**`UPDATE_BAR_PDE` (70).** Payload `rpc_update_bar_pde_v15_00`, 24 bytes
(`g_rpc-structures.h:402`; 610 :384). Sender `kbusPatchBar2Pdb_GSPCLIENT` (`kern_bus.c:902`;
610 :899) when BAR2 is set up (`kern_bus_gm107.c:1957`), with the comment "Provide the
PDE3[0] value to GSP-RM so that GSP-RM can merge CPU-RM's page table to GSP-RM's page table"
(`kern_bus.c:897-901`), and `kbusTeardownBar2GpuVaSpace_GM107` with value 0 at teardown
(`kern_bus_gm107.c:2128`; 610 :2096). GSP-RM writes the value into PDE3[0] of its own BAR2
root. Device: answered, value stored into PDE3[0] of the BAR root (`boot.rs:1147-1176`).
Recipe: as today; the teardown value 0 is handled by the same store. Truth source: guest
state. Isolation: low (the device's own FB model).

**`INIT_GSP_TRACE_CRASH_BUFFER` (228).** Payload `rpc_init_gsp_trace_crash_buffer_v03_00`,
16 bytes (`g_rpc-structures.h:1948`; 610 :1894). Sender `rpcInitGspTraceCrashBuffer_v03_00`
(`rpc.c:11422`), called from `kgspInitRatsCrashBuffer` (`kernel_gsp.c:4284-4375`, send at
:4358) right after the boot (`kernel_gsp.c:5789`; 610: `kgspInitGspTraceCrashBuffer`
:3920, called at :5167). Default on: `NV_REG_STR_RM_GSP_TRACE_CRASH_LOGGING_DEFAULT` is
`_ENABLE` (`src/nvidia/interface/nvrm_registry.h:3446`; 610 :3326), buffer size default 50
(`:3454`). GSP-RM records RATS trace records into the buffer for crash analysis
(`_kgspLogRatsCrashBuffer`, `kernel_gsp.c:2355`, reads it after a GSP crash). Device:
answered, PA and size recorded (`boot.rs:1138`). Recipe: as today; nothing to write unless
the device models a GSP crash. Truth source: pure emulation. Isolation: none.

**`ALLOC_MEMORY` (4).** Payload `rpc_alloc_memory_v13_01`, 56 bytes plus a `pte_desc` page
list (`g_rpc-structures.h:71`), unchanged. Sender `rpcAllocMemory_v13_01` (`rpc.c:3486`;
610 :3441): on a GSP client it sends only the PTEs the memdesc really has, one for a
contiguous buffer (`rpc.c:3516-3526`), through `_issuePteDescRpc` (`rpc.c:2313`), which
splits into continuation records above `maxRpcSize`. The only GSP-client caller is
`memRegisterWithGsp` (`mem.c:513-585`, send at :566), for any memory allocated with
`NVOS32_ATTR2_REGISTER_MEMDESC_TO_PHYS_RM_TRUE` (`mem.c:484-490`) and on explicit request:

| Trigger | Where | When on a desktop TU106 |
|---|---|---|
| GSP operational-event import: header, records and variable-data sysmem (`_goeimportAllocSysmemRegion`, `NV01_MEMORY_SYSTEM`, attr2 `REGISTER_MEMDESC_TO_PHYS_RM`) | `gpu_op_event_import.c:295-327`, steps 3a-3c at :593-620, started from `gpu.c:2796` | every state load (new in 615; not in 610) |
| USERD of `NV2080_CTRL_CMD_FIFO_UPDATE_CHANNEL_INFO` through a deferred API object | `deferred_api.c:343-378` (610 same) | GL and Vulkan user-mode drivers that use `NV50_DEFERRED_API_CLASS` |
| PMA buffer and bytes-available buffer of `ALLOC_PMA_STREAM` | `kern_profiler_v2_ctrl.c:299-302` (610 same) | profilers (Nsight, CUPTI) |
| GSP heap backing store for FB save/restore | `mem_mgr_pwr_mgmt.c:338-400` (610 :383) | suspend (section 4 of [boot-exchange.md](boot-exchange.md)) |

GSP-RM creates a Memory object of class `NV01_MEMORY_LIST_SYSTEM` (or `_LIST_FBMEM`) under the
same client and handle with the given page list (`mem.c:551-556`); later GSP-side objects
name it by handle, and the event-buffer code maps it into GSP's address space ("Cross-
registered sysmem backing handles ... surface on GSP as a Memory object with address space
ADDR_SYSMEM", `src/nvidia/src/kernel/rmapi/event_buffer.c:612-619`). The kernel frees it with
`FREE` (`mem.c:608`). Device: refused (`boot.rs:1177`): the first 615 boot shows the request for the import's header
buffer (client 0xc1e00007, class 0x81 `NV01_MEMORY_LIST_SYSTEM`,
`caraxes/runs/transport-615-2026-09-24/boot/device-events.txt:517`), the free of the import's
client right after it (:520-528) and the guest's `goeimport: step 3a failed (0x56)`
(`.../boot/guest.txt:36`); the import stays off. The other three triggers would fail their
operation.

Recipe: record `(hClient, hMemory) -> {class, flags, length, pteAdjust, page list}` in a
registry beside the channel registry, answer `NV_OK` with the request echoed, drop it on
`FREE`, and let the consumers resolve a handle through it: the USERD of a channel-info
update, the operational-event buffer (whose producer the device then is: it may write
records or leave the buffer empty), a PMA stream (refused with the profiler anyway), and the
GSP heap store at suspend (not written, since the device keeps its state in its own
process). Every page must lie in guest RAM (`GuestMemory::covers`), else refuse with
`NV_ERR_INVALID_ADDRESS`. Continuation records: see `CONTINUATION_RECORD`. Truth source:
guest state. Isolation: low (guest pages only, never a host page).

### 4.2 Sent on some paths

**`IDLE_CHANNELS` (22).** Payload `rpc_idle_channels_v03_00`, 12 bytes plus 12 per channel
(`g_rpc-structures.h:207`). Sender `rpcIdleChannels_v03_00` (`rpc.c:4488`) from
`kfifoIdleChannelsPerDevice_KERNEL` (`src/nvidia/src/kernel/gpu/fifo/kernel_idle_channels.c:44`,
unguarded), reached from `RmIdleChannels` (:51) for `NV_ESC_RM_IDLE_CHANNELS`
(`src/nvidia/arch/nvalloc/unix/src/escape.c:571`) and `NV0000_CTRL_CMD_IDLE_CHANNELS`
(`cliresCtrlCmdIdleChannels_IMPL`, `client_resource.c:5507`). No caller in
nvidia-modeset, nvidia-drm or nvidia-uvm; a user-space driver may call it. GSP-RM waits
until the listed channels are idle or the timeout expires (inferred from the parameters
`flags`, `timeout`). Device: refused. Recipe: resolve each (client, device, channel) through
the channel registry; a software channel is idle when its GPGet equals GPPut after the
executor ran; a mirrored channel is idled on the host by the same RM call against the
mirror object (the host RM client API has `NV_ESC_RM_IDLE_CHANNELS` too), bounded by the
request's timeout; answer `NV_ERR_TIMEOUT` when it expires. Truth source: guest state and
host RM. Isolation: low (only the device's own mirror channels are named on the host).

**`GPU_EXEC_REG_OPS` (50).** Payload `rpc_gpu_exec_reg_ops_v12_01`, 56 bytes plus the op
array (`g_rpc-structures.h:342`; 610 :324). Sender `rpcGpuExecRegOps_v12_01` (`rpc.c:9394`)
from `subdeviceCtrlCmdGpuExecRegOps_cmn` (`subdevice_ctrl_gpu_regops.c:224`), taken on a GSP
client for `NV2080_CTRL_CMD_GPU_EXEC_REG_OPS` unless the call came through
`GPU_MIGRATABLE_OPS` (then a plain `GSP_RM_CONTROL`, :211-221). Used by debug and profiling
tools. GSP-RM validates each op against its register allow-lists and runs it on the GPU
(inferred; the regop validation is physical). Device: refused. Recipe: never forward
register operations to the physical GPU. Answer each op against the device's own BAR0
model for registers the model owns and allows (PRIV-free reads such as PTIMER), and mark
every other op `NV2080_CTRL_GPU_REG_OP_STATUS_NOACCESS` with the call `NV_OK`, which is how
RM reports a denied register; until a tool needs it, refuse as today. Truth source: pure
emulation. Isolation: high if forwarded, none as described.

**`DUMP_PROTOBUF_COMPONENT` (78).** Payload `rpc_dump_protobuf_component_v18_12`, 16 bytes
plus a blob (`g_rpc-structures.h:1468`; 610 :1439). Sender `rpcDumpProtobufComponent_v18_12`
(`rpc.c:10543`): the request carries `component`, `nvDumpType`, `countOnly`, the bugcheck
codes and `bufferSize`; the reply's `bufferSize` bytes of `blob` are appended to the
kernel's protobuf (`rpc.c:10572-10577`). Callers: `nvdEngineDumpCallbackHelper`
(`src/nvidia/src/kernel/diagnostics/nv_debug_dump.c:198`) for every non-CPU engine callback
of an NVDUMP (the `nvidia-bug-report.sh` path), and `rcdbDumpComponent_IMPL` (`journal.c:1232`;
610 :1218) for RC journal dumps. GSP-RM encodes its engine state as protobuf (inferred).
Device: refused, so the dump lacks the GSP part and the component reports the error. Recipe:
answer `NV_OK` with `bufferSize` 0 (an empty blob; `rpc.c:10576` then adds nothing), or,
later, a synthetic protobuf of the device's own state. Truth source: pure emulation.
Isolation: none; never forward the host GPU's dump (it names host clients and processes).

**`ECC_NOTIFIER_WRITE_ACK` (202).** No payload (header only, `rpc.c:11166-11184`), sent
asynchronously (`_issueRpcAsync`, :11181), no reply expected. 615 sends it from
`_kgspRpcEccPossibleErrFastPath` (`kernel_gsp.c:1145-1155`), the handler of the new event
`ECC_POSSIBLE_ERR_FAST_PATH` (0x102A): "ack back immediately that we are done writing
notifiers since we would have already processed the other RC msgs" (:1140-1142). 610 sent
it from `_kgspRpcRCTriggered` (`kernel_gsp.c:611` of 610). GSP-RM waits for it before it
proceeds with its ECC handling (inferred from the comment). Device: refused, but never
reached, because the device never sends 0x102A. Recipe: consume without a reply (an async
RPC must not be answered); pair it with the event in [events.md](events.md) when ECC errors
are mirrored. Truth source: pure emulation. Isolation: none.

**`MAP_MEMORY_DMA` (14) and `UNMAP_MEMORY_DMA` (15).** Payloads `rpc_map_memory_dma_v2C_05`
(56) and `rpc_unmap_memory_dma_v2C_05` (48) (`g_rpc-structures.h:178, 185`). Senders
`rpc.c:4350, 4379` from `virtmemMapTo_IMPL` (`src/nvidia/src/kernel/mem_mgr/virtual_mem.c:1537`)
and `virtmemUnmapFrom_IMPL` (:1892) when the memory is `bRpcAlloc`, and the macros skip the
call when `gpuIsSplitVasManagementServerClientRmEnabled` (`rpc.h:154-181`). A GSP client
enables split VAS management unless the regkey `RMSplitVasMgmtServerClientRm` says 0
(`src/nvidia/src/kernel/gpu/gpu_registry.c:170-181`, `nvrm_registry.h:1381-1383`), and
`bRpcAlloc` is false with it enabled (`virtual_mem.c:462-472`). So a stock configuration
never sends them. Device: refused. Recipe: keep the refusal and name the regkey in the log;
the device relies on the kernel owning the page tables. Isolation: none.

### 4.3 Sent by a 610 GSP client, not by 615

| Id | Function | 610 sender | 615 replacement | Device |
|---:|---|---|---|---|
| 72 | `GSP_SET_SYSTEM_INFO` | `kgspSendInitRpcs` (`kernel_gsp.c:4694` of 610) | the NVKV system info of `GMCAPI_CMD_GSP_INIT` (`kernel_gsp.c:4977-5217`) | answered (`boot.rs:946`), dead code in the v615 build |
| 73 | `SET_REGISTRY` | `kernel_gsp.c:4701` of 610 | the `REGKEY_*` keys of `GSP_INIT` (`kernel_gsp.c:4893-4958`) | consumed (`boot.rs:954`) |
| 1 | `SET_GUEST_SYSTEM_INFO` | `kgspInitRm` (`kernel_gsp.c:5153` of 610) | `KERNEL_DRIVER_VERSION_STRING` in `GSP_INIT`; the RPC stays for vGPU guests (`rpc.c:1054, 1071, 1767`) | answered (`boot.rs:975`) |
| 65 | `GET_GSP_STATIC_INFO` | `kernel_gsp.c:5160` of 610 | the NVKV response of `GSP_INIT` (`kernel_gsp.c:7729-7886`); `NV_RM_RPC_GET_GSP_STATIC_INFO` (`rpc.h:515`) has no caller | answered (`boot.rs:988`) |
| 47 | `UNLOADING_GUEST_DRIVER` | `kgspUnloadRm` (`kernel_gsp.c:5231` of 610) | `GMCAPI_CMD_GSP_SUSPEND` (`kernel_gsp.c:5890`); the RPC stays for vGPU guests (`objvgpu.c:150`) | refused (`boot.rs:1177`) |

The `GSP_INIT_DONE` event that 610 awaited (`kgspWaitForRmInitDone`, `kernel_gsp.c:6264` of
610) is not awaited by 615 either (gsp-transport-615.md §5.4). Recipe: keep the 610 arms for
the 610 build (D14 per-version modules), and add an `UNLOADING_GUEST_DRIVER` arm for nova-core
(r570 ABI, gsp-surface.md §5): `NV_OK`, then the same processor-suspended bit as for
`GSP_SUSPEND` (`boot.rs:961-974`).

## 5. GMCAPI commands (the same queue, ids above 0xFFFF)

Ids are `category << 16 | n` (`gmcapi_table.h:74`); GMCAPI ids up to 0xFFFF are reserved so
that they never collide with RPC function numbers (`gmcapi_base.h:76-80`). CPU-to-GSP
commands are `GMCAPI_PHYSICAL`, GSP-to-CPU commands `GMCAPI_KERNEL`.

| Id | Command | Direction | Payload | 615 sender | Device |
|---|---|---|---|---|---|
| 0x10001 | `GSP_INIT` | CPU to GSP, synchronous, dynamic in and out (`gmcapi_table.h:84-91`) | NVKV system info and registry in, NVKV static config out (gsp-transport-615.md §5) | `kgspSendInitRpcs` (`kernel_gsp.c:5313-5361`, send at :5328), from `kgspBootstrap_TU102` (`kernel_gsp_tu102.c:473`) | answered (`boot.rs:958`, `gsp_init` :1492) |
| 0x10004 | `GSP_SUSPEND` | CPU to GSP, fire and forget (`gmcapi_table.h:109-116`) | `GmcApiGspSuspendParams` {u64 flags}: PM_TRANSITION, GC6_ENTERING, HIBERNATE, FAST, FORCE (`gmcapi_gsp.h:39-56`) | `kgspUnloadRm` (`kernel_gsp.c:5848-5965`, send at :5890) | no reply, MAILBOX0 bit 31 set (`boot.rs:961-974`); the flags are not read |
| 0x30001 | `SCHED_CONTROL` | CPU to GSP (`gmcapi_table.h:257-264`) | NVKV (`gmcapi_sched.h`) | no sender in the open tree | refused by the default arm |
| 0x20001-0x20027 | vGPU management | CPU to GSP and back | | host vGPU manager only (`kernel_hostvgpudeviceapi.c:382, 396, 1176`) | never reached |

The GSP-to-CPU commands (`EXEC_GENERIC_BOOTLOADER` 0x10002, `EXEC_HS_BINARY` 0x10003,
`GSP_RESUME_DONE` 0x10005, `MMU_FAULT_QUEUED` 0x40001) are events; see
[events.md](events.md) and, for `GSP_RESUME_DONE`, [boot-exchange.md](boot-exchange.md).

Recipe for `GSP_SUSPEND`: read the flags and log them; with `PM_TRANSITION` set the device
must keep every object and the queue state for the resume (boot-exchange.md §5), without it
the unload is final and the next boot starts from the construction step.

## 6. Work list

| Item | Recipe | Truth source | Isolation | Agent-days |
|---|---|---|---|---:|
| `ALLOC_MEMORY` registry, `FREE` of registered memory, continuation reassembly by page count | 4.1 | guest state | low | 1 |
| `IDLE_CHANNELS` | 4.2, channel registry plus host idle of mirrors | guest state, host RM | low | 0.5 |
| `DUMP_PROTOBUF_COMPONENT` | empty blob | emulation | none | 0.25 |
| `ECC_NOTIFIER_WRITE_ACK` | consume, no reply | emulation | none | 0.1 |
| `GPU_EXEC_REG_OPS` | per-op NOACCESS against the device's model | emulation | none (never forward) | 0.5 |
| `GSP_SUSPEND` flags | log, keep state on PM transitions | emulation | none | 0.25 |
| `UNLOADING_GUEST_DRIVER` (nova-core) | NV_OK plus the suspended bit | emulation | none | 0.1 |
| **Total** | | | | **about 2.7** |

## 7. Open questions needing a measurement

1. **Does the stock 615 guest reach any conditional RPC on the gates?** Proposal: run the
   CUDA, NVML and display gates of [cutover-615.md](../cutover-615.md) with the 615 device and
   collect `BootModel::unanswered` (the `unanswered` list, `boot.rs:81`) per gate; every RPC
   function there that is not in section 4 is a finding against this survey.
2. **What does the operational-event import do once `ALLOC_MEMORY` succeeds?** After step 3
   it binds `NV_OPERATIONAL_EVENT_BUFFER_BIND` on the physical RM (`gpu_op_event_import.c:520-541`)
   and expects GSP to produce records. Proposal: implement the registry, boot the 615 guest,
   and record the controls and allocations that follow step 3 (device trace), then decide
   between an empty producer and a refusal of the bind.
3. **Which user-space drivers send `NV_ESC_RM_IDLE_CHANNELS` or allocate
   `NV50_DEFERRED_API_CLASS` with `FIFO_UPDATE_CHANNEL_INFO`?** Proposal: on the real card
   (host, operator's consent), `strace -e ioctl` of `glxgears`/`vkcube` under the 615 driver,
   filtered for escape 0x29 (`NV_ESC_RM_IDLE_CHANNELS`) and for the deferred API class on
   `NV_ESC_RM_ALLOC`; or the same inside the guest against the device, which needs no consent.
4. **Is the continuation split of `ALLOC_MEMORY` ever used?** It needs more than about 8000
   PTEs in one registered buffer. Proposal: log the `pteDesc.length` of every `ALLOC_MEMORY`
   in the gates; if none exceeds one element, the reassembly rule is a correctness item, not
   a blocker.

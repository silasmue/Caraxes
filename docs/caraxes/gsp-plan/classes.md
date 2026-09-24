<!-- SPDX-License-Identifier: MIT -->
<!-- Source survey of 2026-09-24 against the open-gpu-kernel-modules trees 615.71.09 and 610.57.04 and the device crate (read only); alloc parameter layouts compiled against each tree's SDK headers and compared field by field. -->
# Classes whose allocation reaches GSP-RM (615.71.09)

Part of the GSP plan ([README.md](README.md)). The transport of an allocation is the RPC
`GSP_RM_ALLOC` (103), its release `FREE` (10) and `DUP_OBJECT` (21)
([rpc-functions.md](rpc-functions.md) §4.1). Paths without prefix are in the 615.71.09 tree
(open-gpu-kernel-modules), "610:" marks the 610.57.04 tree, `crates/vfio-user-nvgpu/src/...` is
the device crate at the base commit of this plan (61e0b6a). Line numbers of `resource_list.h`
are the `RS_ENTRY(` line.

## Status (implementation of 2026-09-24)

Commits on the implementation branch. Run records: `caraxes/runs/gsp-rpc-2026-09-24/`.

- Status refusal of classes outside the chip's class database (§5): **done**, 4e34617. The
  TU106 database (101 classes of `g_gpu_class_list.c`) is `[classes]` of
  `chips/tu106/chip.toml`; outside it `NV_ERR_NOT_SUPPORTED`, under a channel
  `NV_ERR_INVALID_CLASS` (`crates/vfio-user-nvgpu/src/classes.rs`). Refused inside the
  database as well: 0x0001 and 0x0041 (0x22), 0x503B, 0xA0BC, 0xA0BD, and the profiler classes
  0x90CC, 0xB1CC, 0xB2CC, 0x844C (0x56). The stock classes stay accepted (unit test lists
  them); a chip without the table keeps the acknowledgement.
- Status 0x90CD `NV_EVENT_BUFFER` and 0x90D0 `NV_OPERATIONAL_EVENT_BUFFER_BIND` of the 615
  event import (§3): **done**, 29f3366 (validated against the `ALLOC_MEMORY` registry, the
  device is a producer that writes no records).
- Status TSG `internalFlags` grpID (§7 item 2, §8 TSG): **partial**, 4e34617. Recorded per TSG
  (`Registry::tsg_id`) and logged; not yet the key of the host TSG map.
- Status channel `INTERNAL_TSG_ID` (§7 item 1): **partial**, 4e34617. Recorded per channel.
- Status the other §8 recipes (parent checks of root/device/subdevice, UVM retainer
  validation, duplicated TSG/ctxshare aliasing, SW-class channels kept off the host):
  **open**.

## 1. Method

1. Class set. `caraxes/tools/gsp-surface.py --ogkm t615 --json` (and the same for t610) parses the
   RS_ENTRY table of `src/nvidia/src/kernel/rmapi/resource_list.h` and marks a class as reaching
   GSP when its flags carry `RS_FLAGS_ALLOC_RPC_TO_PHYS_RM` (bit 6,
   `src/nvidia/src/kernel/rmapi/resource_desc_flags.h:39`; `RPC_TO_ALL` is bits 4 and 6,
   `:41`) or its internal class is in the script's constructor list `CTOR_RPC_INTERNAL`
   (`caraxes/tools/gsp-surface.py:491`). A second, one-off parser read the
   "Alloc Param Info" and "Parents" fields of every RS_ENTRY in both trees.
2. Route. The generic path is `serverAllocResourceUnderLock` in
   `src/nvidia/src/kernel/rmapi/alloc_free.c:880-939`: after the kernel-side constructor has run,
   a class with `RPC_TO_PHYS_RM` on a firmware client calls `NV_RM_RPC_ALLOC_OBJECT`
   (`alloc_free.c:925`), and sets `bRpcFree` (`:938`). Every `NV_RM_RPC_ALLOC_*` macro takes the
   `IS_FW_CLIENT` branch to `GPU_GET_PHYSICAL_RMAPI(pGpu)->AllocWithHandle`
   (`src/nvidia/inc/kernel/vgpu/rpc.h:241-289, 333-387`), which is `rpcRmApiAlloc_GSP`
   (`src/nvidia/src/kernel/vgpu/rpc.c:10941`). Every constructor-side `NV_RM_RPC_ALLOC_*` call in
   the tree was read for its condition (section 3), because several are `IS_VIRTUAL` only and
   never fire on a GSP client.
3. Parameters. Each class's alloc param struct (from "Alloc Param Info") was compiled against
   each tree's SDK headers (`cc -std=gnu11 -DNV_LINUX -g -I src/common/sdk/nvidia/inc -I .../class
   -I .../ctrl -I .../alloc`) and the DWARF layouts compared with
   `pahole -E` (all fields and nested fields, not only `sizeof`).
4. Physical side. The kernel constructors were read for their GSP-side branches
   (`RMCFG_FEATURE_PLATFORM_GSP`, `IS_GSP_CLIENT`, `internalFlags`); the chip's class list is the
   generated `src/nvidia/generated/g_gpu_class_list.c:286-397` (TU106) plus the generic list
   (`:31-55`).
5. Who allocates. The native 610 ioctl traces of an RTX 2070 (21 probes,
   the `NV_ESC_RM_ALLOC` 0x2b records of the probe traces gsp-surface.md cites as
   `matrix/traces/610.57.04/control/`, which are not checked in at this plan's base commit) and the
   device's own logs of guest runs (`caraxes/runs/*/device-events.txt` and siblings, 116 files,
   the `alloc:` notes of `boot.rs`). User-space traces do not show kernel
   clients (nvidia-modeset, nvidia-uvm, kernel RM itself); the device logs do.
6. Device. `crates/vfio-user-nvgpu/src/boot.rs:1046-1101` (FN_GSP_RM_ALLOC),
   `ctrl.rs:695-750` (`Dispatcher::alloc`), `channel.rs:498-622` (`record_alloc`),
   `display/mod.rs:1159-1197` and `:1425-1437`, `mirror.rs:816-830, 1481-1497, 1583-1600`.

## 2. Summary

| Item | Count |
|---|---:|
| Classes in resource_list.h, 615 (610) | 224 (222) |
| New in 615 | 2: `NV_OPERATIONAL_EVENT_BUFFER_BIND` 0x90D0 (`resource_list.h:144`, flags `ALLOC_PRIVILEGED \| ACQUIRE_GPUS_LOCK`) and `WPPS_CONFIG_SESSION` 0xD0CD (`:282`, no RPC flag); neither carries an RPC flag; but the kernel allocates 0x90D0 on GSP itself at every 615 boot (section 3, last item) |
| Reach GSP by the script's rule (flags or constructor list), 615 and 610 | 149 in 47 internal classes (same set, same flags, same parents, same param type in both trees) |
| of which never sent as GSP_RM_ALLOC by a GSP client (section 3) | 6: `NV01_ROOT_NON_PRIV`, `NV01_ROOT_CLIENT`, `NVENC_SW_SESSION`, `NVFBC_SW_SESSION`, `NV50_P2P`, `NV_MEMORY_FABRIC` |
| Classes a 615 GSP client can put on the wire | 143 |
| of which in the TU106 class DB (the kernel RM allows them on this chip) | 48 |
| of which a stock TU106 Linux stack allocates (init, CUDA, display, GL/Vulkan, video, nvidia-smi) | 36 |
| of which seen at the device in guest runs so far | 30 (+2 Blackwell classes in one gb206 run) |
| of which only tools, tests or legacy paths allocate on TU106 | 12 |
| of which not on TU106 (other generation or feature absent): should be refused | 95 |
| Device: modelled by class id (state kept, used later) | 13: VAS, ctxshare, TSG, `TURING_CHANNEL_GPFIFO_A`, 7 display classes, 2 event classes (hotplug only) |
| Device: recorded as engine objects when the parent is a recorded channel (any class) | the 63 ChannelDescendant classes, generically |
| Device: acknowledged only (generic echo, `ctrl.rs:736-750`) | all others, including the 95 foreign classes |
| Device: refused | 0 |
| Param struct layout changes 610 to 615 among GSP-bound classes | 1 (`NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS` 20 to 28); plus 2 semantic changes without layout change (`internalFlags` of channel and TSG carry the kernel's TSG id) |
| Route changes 610 to 615 | none |

## 3. Corrections to the 149

- **Root.** The kernel RM never forwards the client's own root class. The only root allocation a
  GSP client sends is inside `NV_RM_RPC_ALLOC_SHARE_DEVICE_FWCLIENT`
  (`src/nvidia/inc/kernel/vgpu/rpc.h:48-110`), called from `deviceConstruct_IMPL`
  (`src/nvidia/src/kernel/gpu/device.c:181-194`) for the first device of a client: it allocates
  hClass `NV01_ROOT` literally (`rpc.h:84-87`) with `NV0000_ALLOC_PARAMETERS`, and
  `rpcRmApiAlloc_GSP` forces `paramsSize = sizeof(NV0000_ALLOC_PARAMETERS)` for NV01_ROOT
  (`rpc.c:10985-10988`). So 0x0001 and 0x0041 never appear on the wire; the device logs agree
  (0x0000 in 82 runs, never 0x0041, although every user trace allocates 0x0041).
- **NvencSession, NvfbcSession, P2PApi.** Their constructor RPC is guarded by `IS_VIRTUAL(pGpu)`
  (`src/nvidia/src/kernel/gpu/nvenc/nvencsession.c:67`, `src/nvidia/src/kernel/disp/nvfbc_session.c:84`,
  `src/nvidia/src/kernel/gpu/bus/p2p_api.c:650`), true only in a vGPU guest, and their RS flags
  carry no RPC bit. A GSP client never sends them.
- **MemoryFabric.** The RPC is in `_memoryfabricAllocFabricVa_VGPU`
  (`src/nvidia/src/kernel/mem_mgr/mem_fabric.c:417-438`), taken only when
  `pFabricVAS->bRpcAlloc` (`mem_fabric.c:522`), which is `IS_VIRTUAL(pGpu) && ...`
  (`src/nvidia/src/kernel/mem_mgr/fabric_vaspace.c:312`); the class is also absent from the TU106
  class DB.
- **Conditional ones that do reach GSP.** `EventApi` only when the event's source resource is not
  a ContextDma (`src/nvidia/src/kernel/rmapi/event_api.c:137-171`); `VaSpaceApi` not for FLA
  (`src/nvidia/src/kernel/gpu/mem_mgr/vaspace_api.c:259-262`); `MAXWELL_PROFILER_DEVICE` and
  `_CONTEXT` through the `_FWCLIENT` prologues (`src/nvidia/src/kernel/gpu/hwpm/profiler_v2/kern_profiler_v2.c:543-561, 771-789`);
  `G84_PERFBUFFER` unconditionally (`src/nvidia/src/kernel/gpu/perf/kern_perfbuffer.c:60`);
  `NV01_MEMORY_VIRTUAL` on `IS_GSP_CLIENT` (`src/nvidia/src/kernel/mem_mgr/virt_mem_range.c:125-135`).
  Kernel RM also allocates a device VAS (`FERMI_VASPACE_A`, index `GPU_DEVICE`) on GSP by itself
  (`src/nvidia/src/kernel/mem_mgr/gpu_vaspace.c:3834-3854`).
- **The comment in `src/nvidia/src/kernel/gpu/gpu_op_event_import.c:570`** says NV01_ROOT carries
  `RS_FLAGS_ALLOC_RPC_TO_PHYS_RM`; it does not (flags 0xA001, `resource_list.h:63`); the root
  reaches GSP through the device constructor as above.

- **Kernel-initiated allocations the flag rule does not see (615).** The GSP operational-event
  import (`src/nvidia/src/kernel/gpu/gpu_op_event_import.c`, new in 615, started at every state
  load of a GSP client from `src/nvidia/src/kernel/gpu/gpu.c:2796`) allocates through the
  physical RM API, that is as `GSP_RM_ALLOC`: `NV_EVENT_BUFFER` 0x90CD naming three sysmem
  buffers registered with `ALLOC_MEMORY` (`gpu_op_event_import.c:413-440`, step 5b at :642-650),
  then `NV_EVENT_BUFFER_CTRL_CMD_ENABLE_EVENTS` (:492-509, step 7) and
  `NV_OPERATIONAL_EVENT_BUFFER_BIND` 0x90D0 (:517-548, step 8 at :680-685), and later
  `NV_EVENT_BUFFER_CTRL_CMD_UPDATE_GET` (:1278). So 0x90D0 and 0x90CD do reach GSP on 615, from the
  kernel's own client, although their `RS_ENTRY` flags carry no RPC bit. On the device today the
  sequence stops at step 3a, because `ALLOC_MEMORY` is refused
  ([rpc-functions.md](rpc-functions.md) §4.1); once the memory registry exists, the device must
  answer the two allocations (record the buffers, act as a producer that writes no records) and
  the two controls.

## 4. Transport of the parameters (all classes)

- `rpcRmApiAlloc_GSP` (`rpc.c:10941-11060`): the size is the class's registered size
  (`rmapiGetClassAllocParamSize`, `rpc.c:10979`), then `serverSerializeAllocDown`
  (`rpc.c:11014`, `src/nvidia/src/kernel/rmapi/rmapi_finn.c:356-411`). FINN has no serializer for
  the alloc interface 0xA000 (`src/common/sdk/nvidia/inc/g_finn_rm_api.h:173`): the root switch
  of `finnSerializeRoot_FINN_RM_API` has no case for it and returns NOT_SUPPORTED
  (`src/nvidia/interface/rmapi/src/g_finn_rm_api.c:608-655`), so `FinnRmApiGetSerializedSize`
  is 0 (`:793-794`) and the alloc goes out raw (`rmapi_finn.c:376-378`). No GSP-bound alloc is
  FINN-serialized; `rpc_params->flags` stays `RMAPI_RPC_FLAGS_NONE`.
- The params are copied raw into `rpc_gsp_rm_alloc_v03_00.params` (header 32 bytes: hClient,
  hParent, hObject, hClass, status, paramsSize, flags, reserved[4];
  `src/nvidia/generated/g_rpc-structures.h:1437-1448`, 610 `:1408`, identical).
- On success the returned params are copied back over the caller's params (`rpc.c:11036-11039`).
  For the generic path those are `pRmAllocParams->pAllocParams`, the client's own copy that is
  later copied out to the user; so whatever GSP returns in any field reaches the client. On
  failure the status is `rpc_params->status` (`rpc.c:11047`).
- Embedded pointers sent raw (never dereferenced correctly on GSP): `NV0000_ALLOC_PARAMETERS.pOsPidInfo`
  at offset 112, a kernel pointer (`rpc.h:77`, `src/common/sdk/nvidia/inc/class/cl0000.h:47-52`);
  `NV50VAIO_CHANNELDMA/PIO_ALLOCATION_PARAMETERS.pControl`, set by the kernel constructor to
  the client's mapping of the channel's control page (`src/nvidia/src/kernel/gpu/disp/disp_channel.c:242-251, 374-383`)
  before the generic RPC, so GSP must return it unchanged or the client loses its mapping.
  `NV_CHANNEL_ALLOC_PARAMS` has no pointer fields (pahole).
- Fields the kernel rewrites before sending: channel `flags` (USERD index and page = ChID,
  `kernel_channel.c:2748-2762`), `internalFlags` (privilege, UVM_OWNED, and new in 615 the
  internal TSG id, `kernel_channel.c:2766-2799`), `ProcessID/SubProcessID`, and the
  instance/RAMFC/USERD/method-buffer memory descriptors (`kernel_channel.c:2690-2745`); TSG
  `internalFlags` (TSG id, `kernel_channel_group_api.c:442-455`); event params are built from
  scratch (`rpc.h:342-350`: `hParentClient`, `hClass`, `notifyIndex | NV01_EVENT_CLIENT_RM`,
  `data = 0`); subdevice params from scratch (`rpc.h:372-378`); device params from scratch
  (`rpc.h:95-106`).

## 5. How real GSP-RM refuses a class

GSP-RM runs the same resource server (RMCFG_FEATURE_PLATFORM_GSP):

- Unknown class id: `NV_ERR_INVALID_CLASS` 0x22 (`alloc_free.c:717-720`).
- Class not in the GPU's class DB (built from the chip list filtered by present engines,
  `src/nvidia/src/kernel/gpu/gpu_resource_desc.c:37-106`, `:121-128`): `resservResourceFactory`
  returns `NV_ERR_NOT_SUPPORTED` 0x56 for any GPU-parented class (`alloc_free.c:1393-1398`).
- Channel descendants: class/engine lookup failure or missing engine gives
  `NV_ERR_INVALID_CLASS` (`src/nvidia/src/kernel/gpu/fifo/channel_descendant.c:185-233`).
- Wrong parent: `NV_ERR_INVALID_OBJECT_PARENT` 0x36 (`alloc_free.c:733-737` and the resserv
  parent check).

A stock kernel RM on a TU106 applies the same checks first, so a foreign class never reaches the
wire from a correct guest; the device should still answer 0x56 (GPU-parented) or 0x22 (channel
descendant) for any class not in `g_gpu_class_list.c:286-397` + `:31-55`, instead of NV_OK, so a
modified or confused guest cannot create phantom objects.

## 6. The table

Columns: Class (id) | Internal class, route | Params (610/615 size, change) | GSP validates and
creates | TU106 Linux client allocates | Device today | Recipe, truth source, isolation.
"Echo" means `ctrl.rs:736-750`: object recorded in `Dispatcher::objects`, params returned
unchanged, NV_OK. "Engine object" means `channel.rs:606-620`: when the parent is a channel the
registry knows, the class and params are appended to that channel's `engine_objects`, the params
are echoed, and if the channel already runs on the host the object is created there with the
guest's params (`ctrl.rs:715-729`, `mirror.rs:1583-1600`; at mirroring time `mirror.rs:816-830`
and `:1481-1497`). "Refuse" means NV_ERR_NOT_SUPPORTED per section 5.

| Class | Internal class, route | Params (610/615, change) | GSP validates and creates | TU106 Linux client allocates | Device today | Recipe, truth source, isolation |
|---|---|---|---|---|---|---|
| NV01_ROOT 0x0000 | RmClientResource (`resource_list.h:63`); device ctor, `rpc.h:84-87` | NV0000_ALLOC_PARAMETERS (`class/cl0000.h:47`) 120/120, same; the RS entry says NvHandle, the RPC overrides the size | a client with hClient, processID/name for accounting (`client.c`); physical side: the client's handle space on GSP | always: every process's first device (0x0000 in 82 guest runs) | echo, named (`ctrl.rs:737`) | keep a per-client namespace (exists: `objects`); pure emulation; low |
| NV01_ROOT_NON_PRIV 0x0001, NV01_ROOT_CLIENT 0x0041 | RmClientResource (`:74`, `:84`); never on the wire (section 3) | NvHandle / NV0000 | none | user clients allocate 0x0041 in every trace, but the wire carries 0x0000 | echo | refuse 0x22 if ever seen (not a stock path); low |
| NV50_P2P 0x503b | P2PApi (`:164`); ctor RPC only IS_VIRTUAL (`p2p_api.c:650`) | NV503B_ALLOC_PARAMETERS (`class/cl503b.h:81`) 88/88 | not sent by a GSP client; P2P setup goes through INTERNAL controls | no (single GPU; not in traces) | echo | nothing to do; refuse if seen; low |
| NV01_DEVICE_0 0x0080 | Device (`:184`); `device.c:186` via `rpc.h:95-107` | NV0080_ALLOC_PARAMETERS (`class/cl0080.h:64`) 56/56 | device under the client, hClientShare/target, VA mode; only hClientShare, hTargetClient, hTargetDevice, flags, vaSpaceSize are set (`rpc.h:97-103`) | always | echo, named | record (exists); pure emulation; low |
| GT200_DEBUGGER 0x83de | KernelSMDebuggerSession (`:195`); RPC_TO_ALL | NV83DE_ALLOC_PARAMETERS (`class/cl83de.h:55`) 12/12 | debugger session bound to hAppClient/hClass3dObject; SM exception state on GSP | CUDA runtime allocates one per context (all 6 cuda traces, 9 guest runs) | echo | record the target; controls (`NV83DE_*`) decide; mirror on host only with a host channel; medium (debugger reaches SM state) |
| GF100_PROFILER 0x90cc | Profiler (`:217`); RPC_TO_PHYS_RM | none (RS_NONE) | legacy profiler object on subdevice/channel/TSG | tools only | echo | refuse 0x56 until a profiler is modelled; medium |
| MAXWELL_PROFILER_CONTEXT 0xb1cc, MAXWELL_PROFILER_DEVICE 0xb2cc | ProfilerCtx (`:227`), ProfilerDev (`:238`); `kern_profiler_v2.c:785`, `:557` | NVB1CC 4/4 (`class/clb1cc.h:49`), NVB2CC 8/8 (`class/clb2cc.h:59`) | HWPM/PMA stream objects, permissions (`NVB0CC_CTRL_CMD_INTERNAL_PERMISSIONS_INIT` changed 7 to 6 bytes, gsp-surface.md 615 section) | Nsight/CUPTI only | echo | refuse 0x56 (HWPM of the physical card is host state); high |
| G84_PERFBUFFER 0x844c | PerfBuffer (`:293`); `kern_perfbuffer.c:60` after `perfbufferPrivilegeCheck` | none | perf buffer on device | tools only | echo | refuse 0x56; medium |
| GF100_HDACODEC 0x90ec | Hdacodec (`:313`); RPC_TO_ALL | none | kernel ctor is empty (`src/nvidia/src/kernel/gpu/audio/hda_codec_api.c:26-33`); GSP object carries HDA codec controls | nobody in the open modules or traces | echo | echo is adequate (no state); low |
| GF100_CHANNEL_GPFIFO 0x906f, VOLTA_CHANNEL_GPFIFO_A 0xc36f | KernelChannel (`:336`, `:390`); `kernel_channel.c:2803` | NV_CHANNEL_ALLOC_PARAMS (`alloc/alloc_channel.h:347`) 376/376, same | as C46F below | in the TU106 DB (`g_gpu_class_list.c:290, 395`) but no stock client uses them | echo (only the chip's gpfifo class is registered, `channel.rs:533`) | treat as C46F or refuse 0x56; low |
| KEPLER_CHANNEL_GPFIFO_A 0xa06f, _B 0xa16f, MAXWELL_ 0xb06f, PASCAL_ 0xc06f, AMPERE_ 0xc56f, HOPPER_ 0xc86f, BLACKWELL_A 0xc96f | KernelChannel (`:346-434`) | same, 376/376 | not in the TU106 DB | never on TU106 | echo | refuse 0x56; low |
| TURING_CHANNEL_GPFIFO_A 0xc46f | KernelChannel (`:401`); `kernel_channel.c:2803` via `rpc.h:241-265` | 376/376, same layout; `internalFlags` gains INTERNAL_TSG_ID 23:8 and _VALID 24 in 615 (`src/nvidia/generated/g_kernel_channel_nvoc.h:199-202`, absent in 610) | GSP takes the ChID from the USERD index/page flags, privilege from internalFlags (`kernel_channel.c:257-272` under RMCFG_FEATURE_PLATFORM_GSP), programs instance block/RAMFC/USERD from the passed memdescs (physical FIFO code not in the tree); clears USERD (measured, `boot.rs:1070-1076`); nothing in the params is read back (`kernel_channel.c:2803-2821`, the copy is freed) | always (every CUDA, GL, Vulkan, video trace; 59 guest runs) | modelled: channel registry with chid, gpfifo, USERD, VAS, error notifier (`channel.rs:533-605`); USERD cleared (`boot.rs:1078-1094`); host mirror on first doorbell (`mirror.rs:1430-1497`) | exists; truth: guest state + host RM as a client; medium (host channel executes guest pushes on guest-filtered tables) |
| BLACKWELL_CHANNEL_GPFIFO_B 0xca6f | KernelChannel (`:445`) | 376/376 | not on TU106 | never on TU106 (gb206 only) | named via chips/gb206 | on TU106 refuse 0x56 |
| UVM_CHANNEL_RETAINER 0xc574 | UvmChannelRetainer (`:456`); RPC_TO_ALL, KERNEL_PRIVILEGED, DUAL_CLIENT_LOCK | NV_UVM_CHANNEL_RETAINER_ALLOC_PARAMS (`class/clc574.h:40`) 8/8 (hClient, hChannel of another client) | looks up the other client's channel, retains its ChID and instance block (`src/nvidia/src/kernel/gpu/fifo/uvm_channel_retainer.c:41-95`, same code on GSP) | nvidia-uvm when it services faults of a channel (`src/nvidia/src/kernel/rmapi/nv_gpu_ops.c`); not seen yet | echo (parent is Device/TSG, so not an engine object) | validate that (hClient, hChannel) is a recorded channel, else NV_ERR_OBJECT_NOT_FOUND; guest state; low |
| FERMI_CONTEXT_SHARE_A 0x9067 | KernelCtxShareApi (`:469`); RPC_TO_ALL | NV_CTXSHARE_ALLOCATION_PARAMETERS (`nvos.h:3231`, 610 `:3228`) 12/12 | GSP allocates the subcontext id (`kernel_ctxshare.c:524`) and binds the VAS; the kernel computed `subctxId` into the user params first (`kernel_ctxshare.c:137`) and GSP's returned params overwrite it (`rpc.c:11036`) | always (every trace, 52 runs) | modelled: VAS recorded (`channel.rs:520-524`) | echo is right as long as the kernel's subctxId comes back; guest state; low |
| NV20_SUBDEVICE_0 0x2080 | Subdevice (`:502`); `subdevice.c:124` via `rpc.h:363-387` | NV2080_ALLOC_PARAMETERS (`class/cl2080.h:45`) 4/4 | subdevice on GSP; the target of all 2080 controls | always | echo, named | record; pure emulation; low |
| NV2081_BINAPI 0x2081 | BinaryApi (`:513`); RPC_TO_ALL | NV2081 4/4 | pass-through for binary controls | every user stack (all traces but rm-direct; 60 runs) | echo, named | record; controls handled per id; low |
| NV2082_BINAPI_PRIVILEGED 0x2082 | BinaryApiPrivileged (`:525`); RPC_TO_ALL | NV2082 4/4 | privileged pass-through | tools only | echo | refuse 0x56 or echo without controls; medium (privileged controls) |
| KEPLER_CHANNEL_GROUP_A 0xa06c | KernelChannelGroupApi (`:537`); `kernel_channel_group_api.c:460` | NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS (`nvos.h:2910`, 610 `:2907`) **20/28, changed**: `reserved` @20 and `internalFlags` @24 appended | 615: GSP creates the TSG with the kernel's grpID (`internalFlags` TSG_ID 15:0, VALID 16, GSP_OWNED 17; `g_kernel_channel_group_nvoc.h:123-129`; decoded at `kernel_channel_group_api.c:273-285`); 610: GSP chose its own id | always (every trace, 57 runs) | modelled: VAS and engineType (`channel.rs:525-532`); host TSG created by the mirror with the host's own layout (`mirror.rs:704-739`) | record grpID (new); guest state; low |
| NV01_MEMORY_VIRTUAL 0x0070 | VirtualMemoryRange (`:601`); `virt_mem_range.c:127` | NV_MEMORY_VIRTUAL_ALLOCATION_PARAMS (`class/cl0070.h:70`) 24/24 | a VA range object on the VAS; the kernel returns `limit` itself (`virt_mem_range.c:137`) after the RPC | GL/Vulkan/video user stacks, nvidia-modeset (36 runs) | echo | record for later map/unmap consistency; guest state; low |
| NV_MEMORY_FABRIC 0x00f8 | MemoryFabric (`:726`); vGPU only (section 3) | NV00F8 48/48 | n/a | never on TU106 | echo | refuse 0x56 |
| FERMI_VASPACE_A 0x90f1 | VaSpaceApi (`:747`); `vaspace_api.c:267` (not FLA) and `gpu_vaspace.c:3843` | NV_VASPACE_ALLOCATION_PARAMETERS (`nvos.h:3159`, 610 `:3156`) 56/56 | GSP creates the server-side VAS (split VA management: the kernel owns the page tables and copies reserved PDEs via `NV90F1_CTRL_CMD_VASPACE_COPY_SERVER_RESERVED_PDES`); vaBase/vaSize are overwritten by the kernel from its own VAS (`vaspace_api.c:423-424`) | always (71 runs) | modelled: VAS and its params recorded (`channel.rs:507-519`); host VAS built by the mirror (`mirror.rs:1093, 1220`) | exists; guest state + host; medium |
| NVENC_SW_SESSION 0xa0bc, NVFBC_SW_SESSION 0xa0bd | NvencSession (`:769`), NvfbcSession (`:780`); IS_VIRTUAL only | NVA0BC 20/20, NVA0BD 20/20 | not sent by a GSP client (session list is kernel-side) | NVENC apps allocate 0xa0bc (nvenc trace), kernel-local | echo (never reached) | nothing; refuse if seen |
| GF100_SUBDEVICE_MASTER 0x90e6, GF100_SUBDEVICE_INFOROM 0x90e7 | GenericEngineApi (`:821`, `:832`); RPC_TO_ALL | none | engine API objects for register-region maps and InfoROM controls | INFOROM: nvidia-smi/NVML (nvml trace, 4 runs); MASTER: kernel RM (`nv_gpu_ops.c`) for UVM | echo; INFOROM named | echo (stateless); controls decide; low (MASTER maps registers: medium when mapping is served) |
| NV01_TIMER 0x0004 | TimerApi (`:843`); RPC_TO_PHYS_RM | none | timer object for NV0004 controls/notifiers | nobody in modules or traces | echo | echo; low |
| NV40_I2C 0x402c | I2cApi (`:853`); RPC_TO_PHYS_RM | none | I2C API object for NV402C controls | seen at boot in 7 runs (kernel RM external device / tools) | echo | echo; I2C controls must stay refused or synthesized (host bus access); low for alloc |
| NV20_SUBDEVICE_DIAG 0x208f | DiagApi (`:863`); RPC_TO_ALL | none | kernel ctor empty (`subdevice_diag.c:39-46`) | nvidia-smi (4 runs) | echo, named | echo; low |
| GF100_ZBC_CLEAR 0x9096 | ZbcApi (`:874`); RPC_TO_ALL | none | ZBC table API object; the table itself is GR state on GSP | GL/Vulkan (all egl/gl/vk traces, 27 runs) | echo | record; ZBC controls (`NV9096_CTRL_CMD_SET_ZBC_*`) are the real work; host RM as client when GR runs on the host; medium |
| MMU_VIDMEM_ACCESS_BIT_BUFFER 0xc763, HOPPER_ 0xc863 | VidmemAccessBitBuffer (`:1060`, `:1071`) | NV_VIDMEM_ACCESS_BIT_ALLOCATION_PARAMS (`nvos.h:3313`) 528/528 | Ampere+ feature | never on TU106 | echo | refuse 0x56 |
| AMPERE_SMC_PARTITION_REF 0xc637, AMPERE_SMC_EXEC_PARTITION_REF 0xc638 | GPUInstanceSubscription (`:1081`), ComputeInstanceSubscription (`:1092`) | NVC637 16/16, NVC638 16/16 | MIG | never on TU106 | echo | refuse 0x56 |
| NVC570_DISPLAY 0xc570 | NvDispApi (`:1124`); RPC_TO_ALL | none | the display object, parent of the channels | nvidia-modeset at load (40 runs) | modelled: display recorded (`display/mod.rs:1159-1197`) | exists; pure emulation (display model); low |
| NVC670, C770, C870, C970, CA70, CB70, CC70 _DISPLAY | NvDispApi (`:1134-1194`) | none | other display generations | never on TU106 | echo | refuse 0x56 |
| NVC372_DISPLAY_SW 0xc372 | DispSwObj (`:1204`); RPC_TO_ALL | none | display SW object (NVC372 controls: IMP, validation) | nvidia-modeset (7 runs) | modelled (recorded) | exists; low |
| NV04_DISPLAY_COMMON 0x0073 | DispCommon (`:1214`); RPC_TO_ALL | none | NV0073 controls target | nvidia-modeset and every GL/Vulkan user stack (35 runs) | modelled (recorded) | exists; low |
| NVC57A_CURSOR_IMM_CHANNEL_PIO 0xc57a | DispChannelPio (`:1244`); RPC_TO_ALL | NV50VAIO_CHANNELPIO_ALLOCATION_PARAMETERS (`nvos.h:2898`) 16/16; `pControl` is a client pointer | channel instance per head; control page | nvidia-modeset per head (40 runs) | modelled: USERD page state (`display/mod.rs:1177-1190`) | exists; echo must keep `pControl`; low |
| NVC57B_WINDOW_IMM 0xc57b, NVC57D_CORE 0xc57d, NVC57E_WINDOW 0xc57e _CHANNEL_DMA | DispChannelDma (`:1314-1334`); RPC_TO_ALL | NV50VAIO_CHANNELDMA_ALLOCATION_PARAMETERS (`nvos.h:2890`) 40/40; `pControl` client pointer; `hObjectBuffer` is a kernel ContextDma GSP does not have | the pushbuffer is registered before the alloc with `NV2080_CTRL_CMD_INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER` on the internal subdevice (`disp_channel.c:799-870`); GSP then grabs the channel | nvidia-modeset (core once, window and window-imm per window, 40 runs) | modelled: channel state (`display/mod.rs:1177-1190`) | exists; low |
| NVC67A, C97A, CA7A, CB7A, CC7A cursor PIO; C67B/D/E, C77D, C87B/D/E, C97B/D/E, CA7B/D/E, CB7B/D/E, CC7B/D/E DMA | DispChannelPio/Dma (`:1254-1615`) | 16/16, 40/40 | other display generations | never on TU106 | echo | refuse 0x56 |
| GF100_DISP_SW 0x9072 | DispSwObject (`:1626`); RPC_TO_ALL | NV9072_ALLOCATION_PARAMETERS (`class/cl9072.h:46`) 12/12 (logicalHeadId, displayMask, caps) | SW class on a channel; its methods (flip/vblank semaphore release) are executed by GSP-RM's SW method handler and reported with SEMAPHORE_SCHEDULE_CALLBACK (0x100b) | GL (all egl/gl traces; nvidia-modeset in `src/nvidia-modeset`; 22 runs) | engine object; mirrored with the guest's params | software-method executor on the device (the host cannot run guest flips); guest state + display model; low |
| GF100_TIMED_SEMAPHORE_SW 0x9074 | TimedSemaSwObject (`:1637`); RPC_TO_ALL | none | SW class; release at a timestamp, event TIMED_SEMAPHORE_RELEASE 0x1018 (`src/nvidia/src/kernel/gpu/timed_semaphore.c:119`) | GL (OpenGL sync); not yet seen | engine object | software executor + device clock; pure emulation; low |
| NV50_DEFERRED_API_CLASS 0x5080 | DeferredApiObject (`:1648`); RPC_TO_ALL | NV5080_ALLOC_PARAMS (`class/cl5080.h:43`) 1/1 | SW class executing deferred RM controls from the pushbuffer; on a GSP client some commands are handled kernel-side first (`src/nvidia/src/kernel/gpu/deferred_api.c:345-...`) | kernel RM fifo code, GL; not yet seen | engine object | software executor calling the device's control dispatcher; guest state; medium (arbitrary controls from a push) |
| GP100_UVM_SW 0xc076 | UvmSwObject (`:1659`); RPC_TO_ALL | none | UVM SW methods (fault cancel, clear) on UVM channels | nvidia-uvm for every UVM channel (`kernel-open/nvidia-uvm/uvm_turing_host.c`, allocated by `nv_gpu_ops.c`; 4 runs) | modelled: executed by the channel emulation (`channel.rs:930`, chip.toml `uvm_sw`); such channels stay in software (`mirror.rs:943-956`) | exists; low |
| NV04_SOFTWARE_TEST 0x007d | SoftwareMethodTest (`:1670`) | none | test class | never in a stock stack | engine object | refuse 0x56 or echo; low |
| TURING_DMA_COPY_A 0xc5b5 | KernelCeContext (`:1703`); RPC_TO_ALL | NVB0B5_ALLOCATION_PARAMETERS (`class/clb0b5sw.h:53`) 8/8 (version, engineType) | engine chosen from `engineType` (param-to-engdesc, `channel_descendant.c:160-190`); CE context on GSP | always (every trace; 55 runs) | named; engine object; executed by the channel emulation or the host mirror | exists; host RM as client; medium |
| MAXWELL_ 0xb0b5, PASCAL_ 0xc0b5, AMPERE_A 0xc6b5, AMPERE_B 0xc7b5, HOPPER_ 0xc8b5, BLACKWELL_A 0xc9b5, BLACKWELL_B 0xcab5 _DMA_COPY | KernelCeContext (`:1681-1758`) | 8/8 | not on TU106 | never on TU106 | engine object | refuse 0x22 (channel descendant) |
| NVC4B0_VIDEO_DECODER 0xc4b0 | NvdecContext (`:1781`); RPC_TO_ALL | NV_BSP = NV_NVDEC_ALLOCATION_PARAMETERS (`nvos.h:2941-2948`) 12/12 | NVDEC context on NVDEC0/1 (instance from `engineInstance`) | NVDEC/Vulkan video (nvdec, vk-offscreen traces); not yet seen | engine object | host mirror of an NVDEC channel; host RM as client; medium |
| NVB8B0, C6B0, C7B0, C9B0, CDB0, CEB0, CFB0, D1B0, D2B0 _VIDEO_DECODER | NvdecContext (`:1770-1869`) | 12/12 | not on TU106 | never | engine object | refuse 0x22 |
| NVB8D1, C4D1, C9D1, CDD1, CED0, CFD1, D2D1 _VIDEO_NVJPG | NvjpgContext (`:1880-1946`) | NV_NVJPG_ALLOCATION_PARAMETERS (`nvos.h:3010`) 12/12 | TU106 has no NVJPG (not in `g_gpu_class_list.c:330-397`) | never | engine object | refuse 0x22 |
| NVB8FA ... NVD2FA _VIDEO_OFA (9 classes) | OfaContext (`:1957-2045`) | NV_OFA_ALLOCATION_PARAMETERS (`nvos.h:3017`) 12/12 | no OFA on TU106 | never | engine object | refuse 0x22 |
| NVC4B7_VIDEO_ENCODER 0xc4b7 | MsencContext (`:2067`); RPC_TO_ALL | NV_MSENC = NV_NVENC_ALLOCATION_PARAMETERS (`nvos.h:2990-2997`) 12/12 | NVENC0 context | NVENC/Vulkan video (nvenc, vk-offscreen traces); not yet seen | engine object | host mirror; host RM as client; medium |
| NVD1B7, NVB4B7, NVC7B7, NVC9B7, NVCEB7, NVCFB7 _VIDEO_ENCODER | MsencContext (`:2056-2122`) | 12/12 | not on TU106 | never | engine object | refuse 0x22 |
| HOPPER_SEC2_WORK_LAUNCH_A 0xcba2 | Sec2Context (`:2133`) | none | Hopper+ | never | engine object | refuse 0x22 |
| TURING_A 0xc597, FERMI_TWOD_A 0x902d, KEPLER_INLINE_TO_MEMORY_B 0xa140 | KernelGraphicsObject (`:2304`, `:2282`, `:2293`); RPC_TO_ALL | NV_GR_ALLOCATION_PARAMETERS (`nvos.h:2730`, 610 `:2729`) 16/16 | GR object on the channel's GR context: object count, golden context and ctx buffers (kernel-managed on a GSP client and promoted with `GPU_PROMOTE_CTX`, `src/nvidia/src/kernel/gpu/gr/kernel_graphics_object.c:207-222`) | GL/Vulkan (all egl/gl/vk traces; 26 to 31 runs) | engine object; mirrored when the channel is | same as TURING_COMPUTE_A: host GR context agreeing with the guest's promoted buffers (cuda-path.md §4.1 wall); host RM as client; high (GR context of the physical card) |
| TURING_COMPUTE_A 0xc5c0 | KernelGraphicsObject (`:2314`); RPC_TO_ALL | 16/16 | as above; also compute MMIO (`kgrobjSetComputeMmio_HAL`, `kernel_graphics_object.c:197`) | CUDA and Vulkan (28 runs) | named; engine object; the stub of E5c executes semaphores and inline-to-memory, launches refused | as above |
| AMPERE_A/B, AMPERE_COMPUTE_A/B, ADA_A, ADA_COMPUTE_A, HOPPER_A, HOPPER_COMPUTE_A, BLACKWELL_A/B, BLACKWELL_COMPUTE_A/B, BLACKWELL_INLINE_TO_MEMORY_A | KernelGraphicsObject (`:2143-2271`) | 16/16 | not on TU106 | never on TU106 | engine object (BLACKWELL_COMPUTE_B named for gb206) | refuse 0x22 |
| NV01_EVENT 0x0005, NV01_EVENT_OS_EVENT 0x0079, NV01_EVENT_KERNEL_CALLBACK 0x0078, NV01_EVENT_KERNEL_CALLBACK_EX 0x007e | EventApi (`:2334-2369`); `event_api.c:162` via `rpc.h:333-360` (not for ContextDma sources) | NV0005_ALLOC_PARAMETERS (`class/cl0005.h:47`) 24/24, rebuilt by the macro: hParentClient, hClass, notifyIndex with NV01_EVENT_CLIENT_RM, data 0; hSrcResource is NOT set (`rpc.h:342-350`) | GSP registers a notifier on the parent (the object in whose namespace hObject lives) for `notifyIndex`; when it fires GSP sends POST_EVENT (0x1002) to the kernel, which fans out | 0x79 by every user stack, 0x7e by kernel RM and nvidia-modeset at init (59 runs), 0x05 by Vulkan (vk traces); 0x78 legacy | 0x79/0x7e with notifyIndex HOTPLUG recorded for POST_EVENT (`display/mod.rs:1425-1437`); others echo | an event registry keyed by (client, parent, notifyIndex) so every device-side source (hotplug, RC, nonstall, semaphore) can post; guest state; low |

Notes on the table. The 6 wire-absent classes are listed for completeness. "Seen at the device"
counts are distinct run directories whose device log shows the class in an `alloc:` note.

## 7. Parameter layout changes, 610.57.04 to 615.71.09

All 32 distinct alloc param structs of the GSP-bound classes, plus `NV0000_ALLOC_PARAMETERS`,
`NVOS00_PARAMETERS` (FREE) and `NVOS55_PARAMETERS` (DUP), compiled against both trees and
compared field by field (pahole, nested types expanded):

| Struct | 610 | 615 | Change |
|---|---:|---:|---|
| NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS | 20 | 28 | `reserved` @20 and `internalFlags` @24 appended (`nvos.h:2908-2909`); already handled by `ver/v615.rs:287-301` (offsets of hVASpace/engineType unchanged) |
| all others (NV0005 24, NV0080 56, NV00F8 48, NV2080/2081/2082 4, NV503B 88, NV5080 1, NV50VAIO DMA 40 / PIO 16, NV83DE 12, NV9072 12, NVA0BC 20, NVA0BD 20, NVB0B5 8, NVB1CC 4, NVB2CC 8, NVC637 16, NVC638 16, NV_NVDEC 12, NV_CHANNEL_ALLOC 376, NV_CTXSHARE 12, NV_GR 16, NV_MEMORY_VIRTUAL 24, NV_NVENC 12, NV_NVJPG 12, NV_OFA 12, NV_UVM_CHANNEL_RETAINER 8, NV_VASPACE 56, NV_VIDMEM_ACCESS_BIT 528, NV0000 120, NVOS00 16, NVOS55 28) | same | same | none |

Semantic changes without a layout change (the device must know them for 615):

1. `NV_CHANNEL_ALLOC_PARAMS.internalFlags`: new `INTERNAL_TSG_ID` 23:8 and `INTERNAL_TSG_ID_VALID`
   24 (`src/nvidia/generated/g_kernel_channel_nvoc.h:199-202`; 610 ends at UVM_OWNED 7:7,
   610 `g_kernel_channel_nvoc.h:194-196`), set when the kernel auto-created an internal TSG for
   a bare channel (`kernel_channel.c:2786-2799`).
2. `NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS.internalFlags`: `TSG_ID` 15:0, `TSG_ID_VALID` 16,
   `GSP_OWNED` 17 (`g_kernel_channel_group_nvoc.h:123-129`), set by the kernel for every
   GSP-client TSG (`kernel_channel_group_api.c:442-455`) and honoured by GSP
   (`:273-285`, `kchangrpInit(..., bFixedTsgID, requestedTsgID, bGspOwned)`).
3. A reading caveat: the open tree clears `internalFlags` in both constructors
   unconditionally (`kernel_channel.c:231`, `kernel_channel_group_api.c:126`) and reads them
   afterwards for the GSP build (`kernel_channel.c:250, 257-272`; `kernel_channel_group_api.c:273-285`).
   The GSP-RM build must skip or reorder the clear; the physical-side behaviour here is inferred,
   not readable.

## 8. Recipes for the classes that matter on TU106

Order: what a boot, a CUDA process, a display and a GL/Vulkan client need.

**Root, Device, Subdevice (0x0000, 0x0080, 0x2080).** Real GSP creates a client (with the
kernel's processID and the kernel pointer `pOsPidInfo`, which it cannot use), a device and a
subdevice; the kernel reads nothing back. The device records them (`ctrl.rs:703`) and echoes.
Needed: (a) reject a second root with the same handle (`NV_ERR_INSUFFICIENT_RESOURCES`/
`NV_ERR_INVALID_OBJECT_HANDLE` as resserv does) and a device/subdevice whose parent is not a
recorded root/device (`NV_ERR_INVALID_OBJECT_PARENT` 0x36), so handle bookkeeping bugs show up
as the real error instead of silently succeeding; (b) free of a client removes everything
(exists, `ctrl.rs:782-784`). Truth: pure emulation. Isolation: low. The host side has its own
single client (`hostmem.rs`); no guest root ever maps to a host root, which is correct.

**Channel GPFIFO (0xC46F).** GSP: ChID = USERD page*8 + index from `flags` (the device decodes
it, `channel.rs:537-540`), privilege from `internalFlags`, program RAMFC/instance block from the
memdescs, clear USERD, add to the TSG's runlist on bind/schedule (controls). Reply: echo (nothing
read). Needed beyond today: (1) validate that the parent is a recorded TSG or device and that
`engineType` is one of the chip's (GR0, COPY0-8, NVDEC0/1, NVENC0, SW, SEC2 per chip.toml),
else 0x1F; (2) on 615 decode `INTERNAL_TSG_ID` for bare channels and key the implicit TSG by it;
(3) refuse a ChID already live (`NV_ERR_STATE_IN_USE`) instead of the silent token release
(`channel.rs:585-592`) once FREE tracking is trusted. Truth: guest state; execution on the host
RM as a client (mirror). Isolation: medium, bounded by the shadow VAS.

**TSG (0xA06C).** GSP (615): TSG with the kernel's grpID (fixed), VAS, engine; ctx buffer pools
are on the kernel side. Needed: record `grpID` from `internalFlags` (offset 24, 615 only), use it
as the key of the host TSG map, and answer `NVA06C_CTRL_CMD_*` with it. The host TSG is the
host's own (`mirror.rs:704-739`, host layout from `hostabi`). Truth: guest state. Low.

**Context share (0x9067).** GSP allocates a subcontext id in the TSG's VEID space
(`kernel_ctxshare.c:480-530`); the kernel computed one first (`:137`) and GSP's value overwrites
it on return. Echo returns the kernel's value, which is what the kernel then uses for its own
bookkeeping; correct as long as GSP never disagrees. Record `subctxId` too (offset 8) so the
host mirror can place the channel in the same VEID (host ctxshare allocation, not yet done).
Low.

**VA space (0x90F1).** Split VA management: GSP holds a server VAS whose reserved PDEs the kernel
copies (`NV90F1_CTRL_CMD_VASPACE_COPY_SERVER_RESERVED_PDES`, recorded by
`channel.rs:record_server_reserved_pdes`). Outputs (vaBase, vaSize) are overwritten by the kernel
(`vaspace_api.c:423-424`). Today: recorded; the host VAS exists per mirrored root
(`mirror.rs:1093, 1220`). Remaining: the kernel's own device VAS allocation on GSP
(`gpu_vaspace.c:3834-3854`, index GPU_DEVICE, handle generated by the kernel) should be recorded
like any other. Medium (shadow tables are the isolation boundary).

**NV01_MEMORY_VIRTUAL (0x0070).** GSP creates a virtual memory range object under the device so
that later virtual ContextDma and free work (`virt_mem_range.c:121-135`). Record (hVASpace,
offset, limit) for consistency checks of later DMA maps. Guest state. Low.

**TURING_A / TURING_COMPUTE_A / FERMI_TWOD_A / KEPLER_INLINE_TO_MEMORY_B.** GSP binds the class to
the channel's GR context; with kernel-managed ctx buffers the promote control carries the
buffers. Today: engine object, replayed on the host channel with the guest's 16-byte params
(no handles inside, safe to forward). The wall is the GR context (cuda-path.md §4.1), not the
allocation. Validate `NV_GR_ALLOCATION_PARAMETERS.size` and that the channel's engine is GR0
(`NV_ERR_INVALID_CLASS` otherwise, as `channel_descendant.c:200-233`). Truth: host RM as a
client. High (physical GR state).

**TURING_DMA_COPY_A (0xC5B5).** `engineType` in `NVB0B5_ALLOCATION_PARAMETERS` selects the CE;
the device executes CE methods in software or on the host mirror. Validate `engineType` against
the channel's runlist engine (GSP would fail the class lookup, `channel_descendant.c:154-190`).
Truth: host RM as client. Medium.

**NVC4B0 (NVDEC) and NVC4B7 (NVENC).** Not yet exercised. Recipe: same as CE, an engine object
on a channel whose TSG engine is NVDEC0/1 or NVENC0; mirror to the host channel with the guest's
12-byte params (`engineInstance` must be translated if the host's instance numbering differs,
which on the same TU106 it does not). Truth: host RM as a client. Medium (the physical codec
executes guest command buffers through guest-filtered tables).

**Events (0x0005, 0x0079, 0x007E; 0x0078 legacy).** The wire params are rebuilt by the kernel
(`rpc.h:342-350`), so hSrcResource is absent: the event's parent handle (hParent of the RPC,
`pEvent->hNotifierResource`) is the source. GSP registers a notifier for `notifyIndex & ~flags`
on that object; every time the physical event fires GSP sends POST_EVENT with (hClient, hEvent,
notifyIndex). Today only hotplug is kept (`display/mod.rs:1425-1437`). Needed: a general
registry (client, parent, index) -> events, used by every device source: display (hotplug,
vblank if routed), RC/error notifiers (via RC_TRIGGERED), nonstall/semaphore
(`NV2080_NOTIFIERS_*` of the channel emulation), GR/CE completion. Truth: guest state plus
device events. Low.

**Display classes (0x0073, 0xC570, 0xC372, 0xC57A/B/D/E).** GSP grabs the display channel after
the kernel registered the pushbuffer with `NV2080_CTRL_CMD_INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER`
(`disp_channel.c:799-870`); `hObjectBuffer` refers to a kernel ContextDma that GSP does not have.
Today all modelled by the display model. The echo must keep `pControl` (the client's mapping,
section 4). Classes C573 and C371 have arms in `display/mod.rs:1167-1168` but never reach GSP
(no RPC flag, `resource_list.h:1344, 959`): dead arms. Truth: display model (pure emulation).
Low.

**GF100_DISP_SW (0x9072), GF100_TIMED_SEMAPHORE_SW (0x9074), NV50_DEFERRED_API_CLASS (0x5080).**
Software classes whose methods GSP-RM executes when the channel's PBDMA hits a SW method
(interrupt to GSP). On the device they are engine objects today; when such a channel is mirrored
to the host, the host RM would execute the SW methods against the host's own display and
controls, which is wrong for 0x9072 (flip/vblank release must target the guest's display model)
and 0x5080 (deferred controls must go through the device's dispatcher). Recipe: keep channels
with these classes in the software executor (as done for GP100_UVM_SW, `mirror.rs:943-956`), or
strip the SW object from the host channel and trap the SW method range; implement 0x9072
release-on-vblank from the display model, 0x9074 release-at-time from the device clock (event
0x1018), and 0x5080 by dispatching the deferred control. Truth: guest state and the display
model. Low if kept on the device, high if the host executes them.

**ZBC (0x9096).** The alloc is stateless; `NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR` and friends write
the physical GR ZBC table. Recipe: record per guest; forward to the host only while the guest's
GR runs on the host, and keep a guest table to answer GET. Host RM as client. Medium (the ZBC
table is global to the physical GPU: other host clients see the guest's entries).

**Timer (0x0004), I2C (0x402C), Diag (0x208F), InfoROM (0x90E7), Master (0x90E6), BinAPI
(0x2081).** Stateless API objects; the controls are the work. Echo is sufficient for the
allocation. I2C and Master controls reach physical buses/registers: their controls must be
refused or synthesized, never forwarded.

**UVM channel retainer (0xC574) and UVM SW (0xC076).** The retainer's params name another
client's channel (hClient, hChannel): validate against the registry and pin the channel so a
FREE of that channel while retained keeps the ChID reserved (GSP retains the ChID and instance
block, `uvm_channel_retainer.c:62-84`). UVM SW is executed in software (exists). Guest state.
Low.

**Profiler, debugger (0x90CC, 0xB1CC, 0xB2CC, 0x844C, 0x83DE).** GT200_DEBUGGER is allocated by
every CUDA context: keep the echo (refusing breaks cuInit) and refuse the SM-access controls
(regops, read/write memory) as today's category rule does. The HWPM profiler classes should be
refused with 0x56 until a profiler model exists: the physical HWPM/PMA is host-global state.
High isolation risk if forwarded.

**NvencSession, NvfbcSession (0xA0BC, 0xA0BD).** Kernel-local on a GSP client (section 3); the
session list lives in the kernel RM (`nvencsession.c`), and the host sees nothing. Nothing to do.

**P2P (0x503B) and fabric (0x00F8).** Not on the wire from a GSP client (section 3); refuse if
they ever arrive. P2P between the emulated GPU and anything else is out of scope.

## 9. FREE (10) and DUP_OBJECT (21)

**FREE.** `rpcRmApiFree_GSP` (`rpc.c:11116-11150`) sends `NVOS00_PARAMETERS` (16 bytes, hRoot,
hObjectParent = NULL, hObjectOld, status; unchanged 610/615). It is sent for every resource with
`bRpcFree` set: all generic RPC classes after a successful alloc (`alloc_free.c:938`), the TSG
(`kernel_channel_group_api.c:473`), and dup destinations with auto-free (`rpc.h:405-411`), through
`serverFreeResourceRpcUnderLock` (`alloc_free.c:968-1000`); and explicitly by the constructors'
counterparts: device (`device.c:280`), the client when its last device goes (`device.c:322`,
`alloc_free.c:1037`), subdevice (`subdevice.c:250`), channel (`kernel_channel.c:1273`), event
(`event_api.c:304`), VAS (`vaspace_api.c:576`, `gpu_vaspace.c:3872`), profiler
(`kern_profiler_v2.c:327, 664, 810`), plus memory objects (`mem.c:177, 608`,
`video_mem.c:1096`, `context_dma.c:351`). Freeing an object on GSP frees its children
(resserv), so a TSG free frees its channels and their engine objects.
Device: `boot.rs:1103-1116` -> `ctrl.rs:770-797`: forgets the object, frees the host mirrors of
a channel/TSG/VAS (`mirror.rs:1891`), updates the registry (`channel.rs:625-654`), and a client
free forgets the whole client; always answers NV_OK, also for unknown handles (real GSP-RM
returns an object-not-found error). Gap: children of a freed TSG or channel are only dropped if
the registry keyed them (engine objects go with the channel; events and display objects under a
freed parent stay in `objects` until the client dies).

**DUP_OBJECT.** `rpcRmApiDupObject_GSP` (`rpc.c:11063-11110`) sends `NVOS55_PARAMETERS` (28
bytes, unchanged). Sent from the copy constructors of: context share (`kernel_ctxshare.c:303`),
TSG (`kernel_channel_group_api.c:789`), VAS (`vaspace_api.c:480`), GPU/compute instance
subscriptions (`gpu_instance_subscription.c:281`, `compute_instance_subscription.c:138`, not
TU106), and memory (`mem.c:1046`). Other duplicated classes stay kernel-side. Device:
`boot.rs:1118-1137` -> `ctrl.rs:752-768`: copies the class, aliases a duplicated VAS in the
channel registry (`channel.rs:428-438`), answers NV_OK. Gap: a duplicated TSG or context share
is not aliased, so a channel allocated under a duplicated TSG handle finds no TSG VAS
(`channel.rs:545-556`) and falls back to the context share; UVM duplicates TSGs of CUDA
processes (`nv_gpu_ops.c`), so this matters for UVM-driven channels.

## 10. Open questions and proposed measurements

1. **Does GSP-RM really use the kernel's TSG id on 615, and what does it return for a TSG
   alloc?** Experiment: on the host (615.71.09, the RTX 2070), allocate a TSG through the host
   RM with the escape ioctl and dump the 28-byte params before and after (host is a kernel-RM
   GSP client, so its GSP answers); then read `NVA06C_CTRL_CMD_GET_INFO` or the runlist entry
   (`NV2080_CTRL_CMD_FIFO_GET_CHANNEL_MEM_INFO`) to compare the TSG id with the kernel's. With the
   guest: log the `internalFlags` word of every TSG alloc on a 615 guest (the device has the
   params; one note line in `channel.rs:525`).
2. **Which fields does GSP-RM change in the returned params?** Experiment: a host-side probe
   (`caraxes/tools` host query tool) allocates each TU106 class the stock stack uses (48 list)
   with a sentinel-filled parameter block and diffs the block returned by the ioctl; the host
   kernel RM copies GSP's reply back verbatim (`rpc.c:11036-11039`), so any change is GSP's. This
   decides whether the echo is exact for ctxshare `subctxId`, display `pControl`, and event
   params.
3. **Refusal code for foreign classes.** Experiment: on the host, allocate `AMPERE_COMPUTE_A`
   under a channel and `AMPERE_SMC_PARTITION_REF` under the subdevice; expect 0x22 and 0x56
   respectively (section 5). The kernel refuses before the RPC, so this measures the kernel's
   answer; the GSP answer can be measured only with a guest that bypasses the kernel check, so
   accept the kernel's code as the device's model.
4. **Which classes nvidia-modeset, nvidia-uvm and kernel RM allocate at 615 boot and on first
   CUDA/GL use.** Experiment: boot the 615 guest image with the device's `alloc:` notes and
   tally (one boot, one `cuInit`, one weston start, one
   Vulkan and one NVDEC/NVENC run). Currently unobserved at the device: 0xC574, 0x9074, 0x5080,
   0xC4B0, 0xC4B7, 0x0005.
5. **SW-class channels on the host mirror.** Experiment: run the GL probe with the mirror
   enabled and log whether a channel carrying 0x9072 or 0x5080 is mirrored and whether the host
   RM executes its SW methods (host dmesg for SW method errors; the guest's flip semaphore never
   released would be the symptom). Decide from it whether to keep such channels in software.
6. **Event wire format.** Experiment: log `hParent`, `hObject`, `notifyIndex` and the 24 params
   of every event alloc at the device on a 615 guest, and correlate with the kernel's
   `NV0005` usage (`event_api.c:162`); confirm hSrcResource is 0 on the wire and that the parent
   handle is the notifier source, which the general event registry depends on.
7. **FREE of unknown handles and of parents.** Experiment: a guest test that frees a TSG with
   live channels, then allocates a channel with the same ChID; check that the device's registry
   and the host mirror dropped the old channel (today: registry yes, `objects` children not).
8. **Duplicated TSG under UVM.** Experiment: a CUDA managed-memory run on the guest with the
   device logging DUP_OBJECT class and the later channel allocs under the duplicate; confirm the
   missing TSG alias (section 9) and fix by aliasing TSGs and context shares like VAS.

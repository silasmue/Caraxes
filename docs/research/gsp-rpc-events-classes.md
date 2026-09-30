<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GSP RPC functions, events and classes

Scope: what a 615.71.09 GSP client on TU106 sends to GSP-RM (RPC functions, allocations), what it expects back as events, and how the device answers each.

## Facts

### RPC functions

- `rpc_global_enums.h` lists 232 functions (0 to 231); 615 adds 230 and 231 (`CTRL_FIFO_VGPU_SWRUNLIST_*`) and marks 72 and 73 deprecated [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:10-242; 615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:82-83].
- A GSP client routes every generic alloc, control, free and dup through `rpcRmApiControl_GSP`, `rpcRmApiAlloc_GSP`, `rpcRmApiDupObject_GSP` and `rpcRmApiFree_GSP`, so it never sends `ALLOC_OBJECT` or any `CTRL_*` function [615 tree: src/nvidia/src/kernel/vgpu/rpc.c:10655; 615 tree: src/nvidia/src/kernel/vgpu/rpc.c:10941; 615 tree: src/nvidia/src/kernel/vgpu/rpc.c:11063; 615 tree: src/nvidia/src/kernel/vgpu/rpc.c:11116].
- A 615 client on TU106 uses 14 RPC functions plus the GMCAPI commands `GSP_INIT` and `GSP_SUSPEND` ([gsp-transport.md](gsp-transport.md)) [docs/vfio/gsp-plan/rpc-functions.md §2].

| Id | Function | When sent | Device answer | Evidence |
|---:|---|---|---|---|
| 76 | `GSP_RM_CONTROL` | every boot | per control (`chip::physical`, `ctrl.rs`); GSS-legacy replies keep `rmctrlFlags` 0 | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:86; docs/vfio/gsp-plan/rpc-functions.md §4.1] |
| 103 | `GSP_RM_ALLOC` | every boot | constructs under the guest's handles, clears a new channel's USERD; foreign classes refused (below) | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:113; docs/vfio/gsp-plan/rpc-functions.md §4.1] |
| 10 | `FREE` | every boot | status 0, drops registered memory | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:20; docs/vfio/gsp-plan/rpc-functions.md §Status] |
| 71 | `CONTINUATION_RECORD` | any RPC above 65512 bytes | reassembled for control, alloc and `ALLOC_MEMORY` (by `pteDesc.length`), emitted for large replies | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:81; docs/vfio/gsp-plan/rpc-functions.md §4.1, §Status] |
| 70 | `UPDATE_BAR_PDE` | BAR2 setup and teardown | value stored into PDE3[0] of the BAR root | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:80; docs/vfio/gsp-plan/rpc-functions.md §4.1] |
| 228 | `INIT_GSP_TRACE_CRASH_BUFFER` | every boot (regkey default on) | PA and size recorded | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:238; docs/vfio/gsp-plan/rpc-functions.md §4.1] |
| 4 | `ALLOC_MEMORY` | every boot (event import) | registry per (hClient, hMemory), pages checked against guest RAM or the device's FB | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:14; docs/vfio/gsp-plan/rpc-functions.md §Status] |
| 21 | `DUP_OBJECT` | every CUDA and UVM client | answered and recorded | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:31; docs/vfio/gsp-plan/rpc-functions.md §4.1] |
| 22 | `IDLE_CHANNELS` | `NV_ESC_RM_IDLE_CHANNELS`, `NV0000_CTRL_CMD_IDLE_CHANNELS` | channel registry; mirrored channels idled by the host's `FIFO_IDLE_CHANNELS`, timeout capped at 50 ms | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:32; docs/vfio/gsp-plan/rpc-functions.md §Status] |
| 50 | `GPU_EXEC_REG_OPS` | debug tools | never forwarded; BOOT_0/1/42 reads from the model, every other op `REG_OP_STATUS_NOACCESS` with `NV_OK` | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:60; docs/vfio/gsp-plan/rpc-functions.md §Status] |
| 78 | `DUMP_PROTOBUF_COMPONENT` | `nvidia-bug-report`, RC journal dumps | `NV_OK`, empty blob | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:88; docs/vfio/gsp-plan/rpc-functions.md §Status] |
| 202 | `ECC_NOTIFIER_WRITE_ACK` | reply to event 0x102A | consumed, no reply | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:212; docs/vfio/gsp-plan/rpc-functions.md §Status] |
| 14, 15 | `MAP_MEMORY_DMA`, `UNMAP_MEMORY_DMA` | only with `RMSplitVasMgmtServerClientRm=0` | refused, the log names the regkey | [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:24-25; docs/vfio/gsp-plan/rpc-functions.md §4.2, §Status] |

- The first 615 boot on the device carried 189 `GSP_RM_CONTROL`, 50 `GSP_RM_ALLOC`, 30 `FREE` and one each of `INIT_GSP_TRACE_CRASH_BUFFER`, `UPDATE_BAR_PDE` and `ALLOC_MEMORY` [run: caraxes/runs/transport-615-2026-09-24/boot/device-events.txt; docs/vfio/gsp-plan/rpc-functions.md §2].
- 610 delta: a 610 client also sent `SET_GUEST_SYSTEM_INFO`, `GET_GSP_STATIC_INFO`, `GSP_SET_SYSTEM_INFO`, `SET_REGISTRY` and `UNLOADING_GUEST_DRIVER` (20 functions in the survey against 14) [docs/vfio/gsp-plan/rpc-functions.md §4.3; docs/vfio/gsp-abi-615.md §4].

### `ALLOC_MEMORY` and the event import

- The GSP operational-event import is new in 615 and starts at every state load of a GSP client [615 tree: src/nvidia/src/kernel/gpu/gpu.c:2795-2796].
- Its step 3a registers the header sysmem with `ALLOC_MEMORY`; steps 3b and 3c register records and variable data [615 tree: src/nvidia/src/kernel/gpu/gpu_op_event_import.c:593-596; docs/vfio/gsp-plan/rpc-functions.md §4.1].
- `memRegisterWithGsp` sends class `NV01_MEMORY_LIST_SYSTEM` or `_LIST_FBMEM` with the page list under the caller's handle [615 tree: src/nvidia/src/kernel/mem_mgr/mem.c:551-556].
- The import then allocates `NV_EVENT_BUFFER` 0x90CD, enables events and allocates the bind 0x90D0 on GSP, although neither `RS_ENTRY` carries an RPC flag [615 tree: src/nvidia/src/kernel/gpu/gpu_op_event_import.c:680-685; docs/vfio/gsp-plan/classes.md §3].
- A refused `ALLOC_MEMORY` (0x56) made the guest log `goeimport: step 3a failed (0x56)` and continue with the import off [run: caraxes/runs/transport-615-2026-09-24/boot/guest.txt].
- The same refusal fails the operation for the other three callers: USERD of a deferred-API channel-info update (GL and Vulkan), the PMA buffers of a profiler, and the GSP heap backing store at suspend [docs/vfio/gsp-plan/rpc-functions.md §4.1].
- With the registry the three `ALLOC_MEMORY` (0x1000, 0x7000 and 0x10000 bytes) answer `NV_OK`, the 256-record event buffer and `ENABLE_EVENTS` answer `NV_OK`, the bind gets bindId 1, and no `goeimport` failure appears [run: caraxes/runs/gsp-rpc-2026-09-24/emu615].
- The device writes one GOE record per RC it raises into the registered buffer and wakes the kernel with `POST_EVENT`; the probe run drained 71 of 71 self-test records [docs/vfio/gsp-plan/events.md §Status; docs/vfio/README.md §Status 2026-09-25 round 2].

### Events

- 615 defines 45 events (0x1001 to 0x102D) against 41 in 610; the 615 kernel dispatches 36 of them [615 tree: src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:256-300; docs/vfio/gsp-plan/events.md §Summary].
- `GSP_INIT_DONE` is no longer awaited; the `GSP_INIT` response is the completion [docs/vfio/gsp-plan/events.md §The table].

| Need on TU106 Linux | Count | Events | Evidence |
|---|---:|---|---|
| always | 1 | `POST_EVENT` 0x1003 (nvkms registers HOTPLUG and DP_IRQ) | [docs/vfio/gsp-plan/events.md §Summary] |
| on use | 5 | `SEMAPHORE_SCHEDULE_CALLBACK`, `TIMED_SEMAPHORE_RELEASE`, `GPUACCT_PERFMON_UTIL_SAMPLES`, `FECS_ERROR`, `UPDATE_GSP_TRACE` | [docs/vfio/gsp-plan/events.md §Summary] |
| on error | 5 | `RC_TRIGGERED`, `OS_ERROR_LOG`, `RECOVERY_ACTION`, `PMU_HALTED`, `GSP_POST_NOCAT_RECORD` (optional) | [docs/vfio/gsp-plan/events.md §Summary] |
| never | 34 | NVLink, vGPU, MIG, simulation, Tegra display, dead ids | [docs/vfio/gsp-plan/events.md §Summary] |

- `POST_EVENT` is raised for display HOTPLUG and for the GOE callback event of the import; no general event registry exists for other notifiers [docs/vfio/gsp-plan/events.md §Status R6].
- `RC_TRIGGERED` (64 bytes on 615) and `OS_ERROR_LOG` are raised from the error notifier of a guest channel on a host mirror, once per channel lifetime, never from the host-global RC subscription [docs/vfio/gsp-plan/events.md §Status R1, R2].
- The snapshot counts 4 events the device sends: `GSP_INIT_DONE` (610 transport only), `POST_EVENT`, `RC_TRIGGERED`, `OS_ERROR_LOG` [caraxes/abi/gsp-615.71.09/events.tsv; docs/vfio/gsp-plan/events.md §The table].
- The device never sends `TRIGGER_BUGCHECK` (panics the guest), `FORCED_DRIVER_SHUTDOWN`, `GSP_LOCKDOWN_NOTICE` (stops the GSP interrupt service), `RECOVERY_ACTION`, `GSP_POST_NOCAT_RECORD` or `UPDATE_GSP_TRACE` [docs/vfio/gsp-plan/events.md §Status, §R14].
- GMCAPI `MMU_FAULT_QUEUED` 0x40001 is not raised: the executor raises no faults and a mirror fault is an RC of a non-UVM channel [docs/vfio/gsp-plan/events.md §Status R3].
- Unhandled events reach the kernel's default case, which logs and ignores them [615 tree: src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:1741-1743].

### Classes

| Set | Count | Evidence |
|---|---:|---|
| `RS_ENTRY` classes, 615 (610) | 224 (222) | [docs/vfio/gsp-plan/classes.md §2] |
| reach GSP by flags or constructor list | 149 | [docs/vfio/gsp-plan/classes.md §2] |
| a 615 GSP client can put on the wire | 143 | [docs/vfio/gsp-plan/classes.md §2, §3] |
| of those in TU106's class database | 48 | [docs/vfio/gsp-plan/classes.md §2] |
| of those a stock TU106 Linux stack allocates | 36 | [docs/vfio/gsp-plan/classes.md §2] |
| not on TU106 | 95 | [docs/vfio/gsp-plan/classes.md §2] |

- Six of the 149 never go on the wire from a GSP client: 0x0001 and 0x0041 (the root travels as `NV01_ROOT` inside the device constructor), `NVENC_SW_SESSION`, `NVFBC_SW_SESSION`, `NV50_P2P` and `NV_MEMORY_FABRIC` (vGPU-only constructor RPCs) [docs/vfio/gsp-plan/classes.md §3].
- 615 adds two classes, `NV_OPERATIONAL_EVENT_BUFFER_BIND` 0x90D0 and `WPPS_CONFIG_SESSION` 0xD0CD [615 tree: src/nvidia/src/kernel/rmapi/resource_list.h:144; docs/vfio/gsp-plan/classes.md §2].
- GSP-RM refuses an unknown class id with `NV_ERR_INVALID_CLASS` 0x22 [615 tree: src/nvidia/src/kernel/rmapi/alloc_free.c:717-720].
- GSP-RM refuses a GPU-parented class outside the GPU's class database with `NV_ERR_NOT_SUPPORTED` 0x56 [615 tree: src/nvidia/src/kernel/rmapi/alloc_free.c:1393-1398].
- A channel descendant whose class or engine lookup fails gets `NV_ERR_INVALID_CLASS` [docs/vfio/gsp-plan/classes.md §5].
- The device applies the same rule from the 101-class TU106 database in `chips/tu106/chip.toml`: outside it 0x56, under a channel 0x22 [docs/vfio/gsp-plan/classes.md §Status].
- Inside the database the device also refuses 0x0001 and 0x0041 (0x22), 0x503B, 0xA0BC, 0xA0BD, and the profiler classes 0x90CC, 0xB1CC, 0xB2CC, 0x844C (0x56) [docs/vfio/gsp-plan/classes.md §Status].
- The snapshot's split of the 149: 21 named and 29 acknowledged inside TU106's database, 3 refused there (profiling), 96 refused outside it [caraxes/abi/gsp-615.71.09/classes.tsv; docs/vfio/gsp-abi-615.md §4].
- The only alloc-parameter layout change among GSP-bound classes is `NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS` 20 to 28 bytes, and the channel and TSG `internalFlags` now carry the kernel's TSG id [docs/vfio/gsp-plan/classes.md §2, §7].

## Consequence for Leandro VFIO

- The RPC surface of a 615 client is closed: all 14 functions and both CPU-to-GSP GMCAPI commands have an answer.
- `ALLOC_MEMORY` is not optional on 615: without it the event import is off at every boot, and deferred-API USERD, profiler PMA and suspend fail.
- A foreign class never creates a phantom object; the device answers with GSP-RM's own codes 0x22 and 0x56.
- Errors in host-mirrored channels reach the guest kernel as `RC_TRIGGERED`, `OS_ERROR_LOG` and a GOE record, sourced only from the guest's own channels.
- Transport and the GMCAPI side: see [gsp-transport.md](gsp-transport.md); counts per snapshot: see [abi-baseline.md](abi-baseline.md).

## Open

- Whether the stock 615 guest reaches any conditional RPC on the CUDA, NVML and display gates, from `BootModel::unanswered` per gate [docs/vfio/gsp-plan/rpc-functions.md §7 item 1].
- Which user-space drivers send `NV_ESC_RM_IDLE_CHANNELS` or allocate `NV50_DEFERRED_API_CLASS` with `FIFO_UPDATE_CHANNEL_INFO` [docs/vfio/gsp-plan/rpc-functions.md §7 item 3].
- Whether the continuation split of `ALLOC_MEMORY` (above about 8000 PTEs) is ever used [docs/vfio/gsp-plan/rpc-functions.md §7 item 4].
- `PMU_HALTED` (no lost-GPU decision) and the model-dependent events `SEMAPHORE_SCHEDULE_CALLBACK`, `TIMED_SEMAPHORE_RELEASE`, `FECS_ERROR`, `GPUACCT_PERFMON_UTIL_SAMPLES` are not raised [docs/vfio/gsp-plan/events.md §Status R4, R7-R11].
- What real GSP-RM sends for a guest MMU fault on 615 (order of `OS_ERROR_LOG` and `RC_TRIGGERED`, journal size) needs a passthrough capture [docs/vfio/gsp-plan/events.md §Measurement proposals M3].
- Whether nvkms reads the hotplug mask from `data` or `eventData` [docs/vfio/gsp-plan/events.md §Measurement proposals M9].
- Which fields GSP-RM changes in returned alloc params, which decides whether the echo is exact [docs/vfio/gsp-plan/classes.md §10 item 2].
- Classes not yet observed at the device: 0xC574, 0x9074, 0x5080, 0xC4B0, 0xC4B7, 0x0005 [docs/vfio/gsp-plan/classes.md §10 item 4].

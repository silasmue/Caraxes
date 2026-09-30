<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GSP RPC functions and events

An RPC function is one message type on the command queue, an event one message type on
the status queue that GSP-RM sends unasked. The RPC vocabulary is one driver-version-wide
ABI shared by all chips; a GSP client on Turing uses a small part of it.

## RPC functions a 615 client uses

`src/nvidia/inc/kernel/vgpu/rpc_global_enums.h` lists 232 functions. A GSP client routes
every generic alloc, control, free and dup through `rpcRmApiControl_GSP`,
`rpcRmApiAlloc_GSP`, `rpcRmApiDupObject_GSP` and `rpcRmApiFree_GSP`
(`src/nvidia/src/kernel/vgpu/rpc.c`), so it never sends `ALLOC_OBJECT` or a `CTRL_*`
function. On TU106 it uses 14:

| Function | When | Device answer |
|---|---|---|
| `GSP_RM_CONTROL` (76) | every boot, every client | per control ([gsp-control-categories](gsp-control-categories.md)); GSS-legacy replies keep `rmctrlFlags` 0 unless `CARAXES_LEGACY_CACHEABLE=1` |
| `GSP_RM_ALLOC` (103) | every boot, every client | constructs under the guest's handles; foreign classes refused with GSP-RM's codes |
| `FREE` (10) | every boot | status 0, drops registered memory and mirror objects |
| `CONTINUATION_RECORD` (71) | any RPC above 65512 bytes | reassembled for control, alloc and `ALLOC_MEMORY`; emitted for large replies |
| `UPDATE_BAR_PDE` (70) | BAR2 setup and teardown | stored into PDE3[0] of the BAR root ([bar2-page-tables-and-userd](bar2-page-tables-and-userd.md)) |
| `INIT_GSP_TRACE_CRASH_BUFFER` (228) | every boot | address and size recorded |
| `ALLOC_MEMORY` (4) | every 615 boot (the operational-event import, `gpu_op_event_import.c`), deferred-API USERD, profiler buffers, suspend | a registry per (client, memory) with pages checked against guest RAM or the VM's FB |
| `DUP_OBJECT` (21) | every CUDA and UVM client | answered and recorded |
| `IDLE_CHANNELS` (22) | `NV_ESC_RM_IDLE_CHANNELS` | mirrored channels idled through the host's `FIFO_IDLE_CHANNELS`, timeout capped at 50 ms |
| `GPU_EXEC_REG_OPS` (50) | debug tools | never forwarded: the three boot ids from the model, everything else `REG_OP_STATUS_NOACCESS` |
| `DUMP_PROTOBUF_COMPONENT` (78) | `nvidia-bug-report`, RC journal dumps | `NV_OK`, empty |
| `ECC_NOTIFIER_WRITE_ACK` (202) | reply to an ECC event | consumed |
| `MAP_MEMORY_DMA`, `UNMAP_MEMORY_DMA` (14, 15) | only with `RMSplitVasMgmtServerClientRm=0` | refused, the log names the key |

- A 610 client also sent `SET_GUEST_SYSTEM_INFO`, `GET_GSP_STATIC_INFO`,
  `GSP_SET_SYSTEM_INFO`, `SET_REGISTRY` and `UNLOADING_GUEST_DRIVER`; 615 folded them into
  `GMCAPI_CMD_GSP_INIT` and `GSP_SUSPEND` ([gsp-message-queue](gsp-message-queue.md)).
- The 570 guests boot with `GSP_SET_SYSTEM_INFO`, `SET_REGISTRY`, `GET_GSP_STATIC_INFO` and
  `UNLOADING_GUEST_DRIVER`, numbered as in 610; nouveau then sends `GSP_RM_ALLOC` and
  `GSP_RM_CONTROL` with the 570 control frame. The Windows 617 KMD uses the functions of the
  table ([guest-drivers-and-abis](guest-drivers-and-abis.md)).
- `ALLOC_MEMORY` is not optional on 615: refused, the event import logs
  `goeimport: step 3a failed` and stays off, and the deferred-API USERD update of GL and
  Vulkan, the profiler's buffers and the heap backing store at suspend fail with it.
- `memRegisterWithGsp` (`src/nvidia/src/kernel/mem_mgr/mem.c`) sends class
  `NV01_MEMORY_LIST_SYSTEM` or `_LIST_FBMEM` with the page list under the caller's handle.

## Events

- 45 events (0x1001 to 0x102D) in 615, 36 dispatched by the kernel; an unhandled one reaches
  the kernel's default case, which logs and ignores it (`kernel_gsp.c`).
- `GSP_INIT_DONE` is no longer awaited on 615; the `GSP_INIT` response is the completion. A
  570 or 610 guest still waits for it, a 570 guest after `GSP_RUN_CPU_SEQUENCER`.
- Payloads differ per guest ABI: `RC_TRIGGERED` is 48 bytes in 570 and 610 and 64 in 615;
  `OS_ERROR_LOG` has `runlistId`, `chid` and `errString` at 4, 8, 12 in 570 and at 8, 12, 16
  in 610 ([research: open-render](../research/open-render-2026-09-29.md)).

| Need on TU106 Linux | Events | Device |
|---|---|---|
| always | `POST_EVENT` 0x1003 (nvkms registers HOTPLUG and DP_IRQ; the operational-event import's callback) | sent |
| on error | `RC_TRIGGERED` (64 bytes in 615), `OS_ERROR_LOG`, `RECOVERY_ACTION`, `PMU_HALTED`, `GSP_POST_NOCAT_RECORD` | the first two, from the error notifier of a guest channel's host mirror, once per channel lifetime, never from the host-global RC subscription |
| on use | `SEMAPHORE_SCHEDULE_CALLBACK`, `TIMED_SEMAPHORE_RELEASE`, `GPUACCT_PERFMON_UTIL_SAMPLES`, `FECS_ERROR`, `UPDATE_GSP_TRACE` | not raised yet |
| never | NVLink, vGPU, MIG, simulation, Tegra display, dead ids (34) | never |

- Never sent by design: `TRIGGER_BUGCHECK` (panics the guest), `FORCED_DRIVER_SHUTDOWN`,
  `GSP_LOCKDOWN_NOTICE` (stops the GSP interrupt service), `RECOVERY_ACTION`.
- A guest channel error on the host mirror becomes `RC_TRIGGERED`, `OS_ERROR_LOG` and one
  record in the registered operational-event buffer, woken with `POST_EVENT`; the guest
  kernel sets "OS reboot required" when a UVM-owned channel gets an RC
  (`src/nvidia/src/kernel/gpu/rc/kernel_rc_notification.c`).
- MMU faults: GMCAPI `MMU_FAULT_QUEUED` (0x40001) is not raised; a mirror fault is an RC of
  a non-UVM channel ([execution-mirror](execution-mirror.md)). Replayable UVM faults have no
  host producer ([research: uvm-faults](../research/uvm-faults-2026-09-28.md)).

## Classes on the wire

- 149 of the 224 `RS_ENTRY` classes reach GSP by flags or constructor; 143 can go on the
  wire from a GSP client; 48 are in TU106's class database; 36 are allocated by a stock
  TU106 Linux stack.
- Outside the database the device answers `NV_ERR_NOT_SUPPORTED`, under a channel
  `NV_ERR_INVALID_CLASS`, as `alloc_free.c` does. Inside it the device refuses the profiler
  classes 0x90CC, 0xB1CC, 0xB2CC and the perf buffer 0x844C with `NV_ERR_NOT_SUPPORTED` until
  it models a profiler (`REFUSED` in `crates/vfio-user-nvgpu/src/classes.rs`).
- The Windows build allocates classes outside the Linux database; the device records 0xB297
  under a subdevice (`WINDOWS_ONLY` in `classes.rs`,
  [guest-drivers-and-abis](guest-drivers-and-abis.md)).
- 615 added `NV_OPERATIONAL_EVENT_BUFFER_BIND` 0x90D0 and `WPPS_CONFIG_SESSION` 0xD0CD; the
  one alloc-parameter change among GSP-bound classes is `NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS`
  (20 to 28 bytes), and the channel and TSG `internalFlags` now carry the kernel's TSG id.

## Depth

- [research/gsp-rpc-events-classes.md](../research/gsp-rpc-events-classes.md): the cited lines.
- [caraxes/gsp-plan/rpc-functions.md](../caraxes/gsp-plan/rpc-functions.md),
  [caraxes/gsp-plan/events.md](../caraxes/gsp-plan/events.md),
  [caraxes/gsp-plan/classes.md](../caraxes/gsp-plan/classes.md): every function, event and class.

<!-- SPDX-License-Identifier: MIT -->
<!-- Source survey generated 2026-09-23 by reading vendor/open-gpu-kernel-modules at tag 610.57.04
     (and the Caraxes tree at e2f1aec). Every claim cites file:line; values marked "derived" were
     computed, not read. Treat as notes to be verified by experiment, not as specification. -->
# NVIDIA open kernel driver 610.57.04 in GSP-client mode: the CPU↔GSP message queue and RPC protocol, read from source

All paths are relative to `/home/silas/git/Caraxes/vendor/open-gpu-kernel-modules/`. Every claim cites file:line. Struct offsets marked "(computed)" come from the C declarations assuming x86-64 (`NvLength`=NvU64, nvtypes.h:309; `NvBool`=NvU8, nvtypes.h:272). They were not checked with a compiler.

**Three facts correct assumptions in the brief:**
- **There is no CPU sequencer in 610.57.04.**
  - `GSP_RUN_CPU_SEQUENCER` (0x1002) still has an enum entry, but `_kgspProcessRpcEvent` has no case for it. It falls into `default:` and is only logged as "Unexpected RPC event" (kernel_gsp.c:1587-1600).
  - `grep -r GSP_SEQ` finds nothing anywhere under src/.
  - `rmgspseq.h` (src/nvidia/arch/nvalloc/common/inc/rmgspseq.h) now defines only `GspLoadExecGenericBootloaderParams` and `GspLoadExecHsBinaryParams`. These are used by the new events 0x1026 and 0x1027 (see section 3).
- **`GSP_SEND_USER_SHARED_DATA` does not exist** in this version's enum (rpc_global_enums.h:245-290).
- **`RUN_CPU_SEQUENCER`-style work is not needed during init.** An emulated GSP should never send 0x1026 or 0x1027: they make the CPU reset the GSP falcon and run ucode on it.

---

## 1. Queue memory layout

### Shared buffer allocation
The buffer is set up by `GspMsgQueuesInit`, message_queue_cpu.c:204-337, using sizes from `_getMsgQueueParams`, :61-138:
- Command queue: 0x40000 bytes (256 KB) on silicon, :71 and :96. It is ×6 on non-silicon, :104.
- Status queue: 0x40000 bytes, :72 and :117. It can be overridden by the registry key `NV_REG_STR_RM_GSP_STATUS_QUEUE_SIZE` (KB), :108-114.
- `queueElementSizeMin` = RM_PAGE_SIZE (0x1000), :88. `queueElementSizeMax` = 16 pages (0x10000), :89.
- `queueHeaderAlign` = 4 (a log2 value, so 16 bytes), :90. `queueElementAlign` = RM_PAGE_SHIFT (12), :91.
- `queueElementHdrSize` = `offsetof(GSP_MSG_QUEUE_ELEMENT, payload)` = 16, :82. It grows by `sizeof(GSP_MSG_QUEUE_ENCRYPTION_TAG)` (40 bytes) when Confidential Compute is enabled, :83-86.
- Page-table sizing:
  - numPtes = (cmd+stat)>>12 = 128, plus ceil(128×8/4096) = 1, giving 129. This is `pageTableEntryCount`, :125-137.
  - `pageTableSize` = page-align(129×8) = 0x1000, :136.
- Memory type: one noncontiguous, cached SYSMEM memdesc of size pageTableSize+cmd+stat = 0x81000, :241-257. It is zeroed at :293.

### Layout
Offsets are from the start of the shared buffer (comments at message_queue_cpu.c:297-304, message_queue_priv.h:35-42):

| Offset | Content |
|---|---|
| 0x00000 | Page table: `RmPhysAddr[129]`, the GPU-view address of each 4K page including page 0 itself (`memdescGetPhysAddrs(..., AT_GPU, 0, RM_PAGE_SIZE, count, pPageTbl)`, :305-310) |
| 0x01000 | Command queue (CPU→GSP), 0x40000 bytes (:312-313) |
| 0x41000 | Status queue (GSP→CPU), 0x40000 bytes (:315-316) |

`sharedMemPA` = `pPageTbl[0]`, :329.

### Inside each queue (msgq library, src/nvidia/src/libraries/msgq/msgq.c)

- `msgqTxHeader` (msgq_priv.h:46-56), 32 bytes, at queue offset 0:

  | Offset | Field | Meaning |
  |---|---|---|
  | +0 | `version` | 0, `MSGQ_VERSION` (:36) |
  | +4 | `size` | queue size in bytes |
  | +8 | `msgSize` | 0x1000 |
  | +12 | `msgCount` | number of slots |
  | +16 | `writePtr` | index of next slot to write |
  | +20 | `flags` | bit 0 = `MSGQ_FLAGS_SWAP_RX` (msgq.h:39) |
  | +24 | `rxHdrOff` | offset of `msgqRxHeader` |
  | +28 | `entryOff` | offset of slot 0 |

- `msgqRxHeader` = `{ NvU32 readPtr; }` (msgq_priv.h:59-62).
- `msgqTxCreate` (msgq.c, TxCreate function) computes:
  - rxHdrOff = align(32, 1<<hdrAlign) = 0x20
  - entryOff = align(0x20+4, 1<<12) = 0x1000
  - msgCount = (size − entryOff)/msgSize = (0x40000−0x1000)/0x1000 = **63**
  - initial writePtr = 0
  - The local copy of the header is memcpy'd into the backing store.
- **Pointer semantics.** Slot n is at `entries + n*msgSize`.
  - free = (readPtr + msgCount − writePtr − 1) mod msgCount, so one slot is always left empty (`msgqTxGetFreeSpace`).
  - available = (writePtr − readPtr) mod msgCount (`msgqRxGetReadAvailable`).
  - Both functions treat pointers ≥ msgCount as 0 free/available.

### Who writes which pointer (`MSGQ_FLAGS_SWAP_RX`)
The CPU creates its TX queue with `MSGQ_FLAGS_SWAP_RX` (message_queue_cpu.c:174-180). Swapping is active only if **both** headers have the flag (`rxSwapped = tx.flags & rx.flags & SWAP_RX`, computed in both `msgqTxCreate` and `msgqRxLink`).

- **With swap (expected):**
  - CPU writes `cmdQ.tx.writePtr` at shared+0x1010.
  - CPU writes its **status-queue read pointer** at `cmdQ + rxHdrOff` = shared+0x1020 (`pReadOutgoing = &pOurRxHdr->readPtr`).
  - CPU reads GSP's **command-queue read pointer** from `statQ + statQ.rxHdrOff` (`pReadIncoming = &pTheirRxHdr->readPtr`).
  - CPU reads GSP's `writePtr` at statQ+0x10 (`pWriteIncoming`).
- **Without swap:** `pReadIncoming = &cmdQ.rxHdr.readPtr` and `pReadOutgoing = &statQ.rxHdr.readPtr` (else branches in `msgqRxLink`).
- **Before the status queue is linked** (the window in which `GSP_SET_SYSTEM_INFO` and `SET_REGISTRY` are sent), `rx.flags` = 0. So `pReadIncoming = &cmdQ.rxHdr.readPtr` (shared+0x1020), and the cached `txFree` starts at msgCount−1 = 62 (msgqTxCreate).
- `msgqRxLink` writes rxReadPtr=0 to `pReadOutgoing`.

### Checks `msgqRxLink` applies to the GSP-written status-queue header
The GSP-side header at shared+0x41000 must pass:
- size == `statusQueueSize` (0x40000) (−7)
- msgSize == 0x1000 (−8)
- version == 0 (−9)
- rxHdrOff ≥ 32, entryOff ≥ CPU tx.rxHdrOff+4, msgCount == (size−entryOff)/msgSize (−10)
- size ≥ entryOff+msgSize (−6)

(Error codes in parentheses are `msgqRxLink`'s return values, msgq.c.) `GspStatusQueueInit` retries `msgqRxLink` in a spin loop for 4 s (message_queue_cpu.c:339-415, `timeoutUs = 4000000` at :345). Each iteration also runs `kgspHealthCheck_HAL`, :398-403.

### Queue element (message_queue_priv.h:44-74)
`GSP_MSG_QUEUE_ELEMENT` is 16 bytes, followed by the payload:

| Offset | Field |
|---|---|
| +0 | `mctpHeader` |
| +4 | `nvdmHeader` |
| +8 | `checkSum` |
| +12 | `seqNum` |
| +16 | payload: optional `GSP_MSG_QUEUE_ENCRYPTION_TAG` (CC only: `encryptedSize`, `reserved`, `authTag[16]`, `aad[16]`), then `rpc_message_header_v`, then RPC data |

- **MCTP header**, set on send (message_queue_cpu.c:505-512; mctp_format.h):
  - `mctpHeader` = VERSION(3:0)=1 | EOM(30)=1 | SOM(31)=1 = **0xC0000001**.
  - `nvdmHeader` = TYPE(6:0)=0x7E | VENDOR_ID(23:8)=0x10DE | NVDM_TYPE(31:24)=0x25 (`NVDM_TYPE_RM_RPC`, nvdm_format.h) = **0x2510DE7E**.
  - On receive the CPU checks only MCTP version == 1 and vendor id == 0x10DE (message_queue_cpu.c:737-759).
- **Checksum** (`_checkSum32`, message_queue_priv.h:192-206):
  - XOR all 64-bit words over the message length, rounded up to 8 bytes.
  - Return hi32 ^ lo32.
  - The sender zero-pads to 8 bytes (message_queue_cpu.c:500-501), sets `checkSum`=0, then stores the checksum (:515, :545). The whole message therefore XORs to 0.
  - The receiver requires `_checkSum32(element, 16 + rpc.length) == 0` (:725-735).
  - With CC enabled, the checksum covers all elements (:541, :721).
- **Element sequence numbers** are independent of the RPC `sequence` field.
  - `seqNum` = `txSeqNum`, and `txSeqNum` increments once per message, not per element (:514, :620).
  - The receiver requires `seqNum == rxSeqNum`, starting at 0 (:761-788). If the seqNum is older, it consumes the stale message and retries (:768-783).
  - `rxSeqNum` increments even on failure (:836).
  - **An emulated GSP's first status-queue message must have seqNum 0, and each later message must increment it by 1.**
- **Message size.** One message spans ceil((16 + rpc.length)/0x1000) consecutive slots, with wrap handled (`gspMsgQueueBytesToElements`, :503, :693-701). Maximum is 16 slots (0x10000), so maxRpcSize = 0x10000−16 = 65520 (kernel_gsp.c:3181-3182; message_queue_priv.h:108-111).
- The receiver copies whole 4 KB slots into its staging buffer (message_queue_cpu.c:680-682). Fields past `length` inside the last slot are therefore whatever is in queue memory.

### How GSP learns the queue location
1. `kgspPopulateGspRmInitArgs_IMPL` (kernel_gsp.c:5467-5556) fills `GSP_ARGUMENTS_CACHED.messageQueueInitArguments` (gsp_init_args.h). The struct is 64 bytes (computed):

   | Offset | Field | Value |
   |---|---|---|
   | +0 | `sharedMemPhysAddr` | PA of page 0 = the page table |
   | +8 | `pageTableEntryCount` | 129 |
   | +16 | `cmdQueueOffset` | 0x1000 |
   | +24 | `statQueueOffset` | 0x41000 |
   | +32 | `queueElementHdrSize` | 16 |
   | +40 | `queueElementSizeMin` | 0x1000 |
   | +48 | `queueElementSizeMax` | 0x10000 |
   | +56 | `queueHeaderAlign` | 4 |
   | +60 | `queueElementAlign` | 12 |

   The rest of `GSP_ARGUMENTS_CACHED`:
   - `srInitArguments` at +64: `oldLevel`, `flags`, `bInPMTransition`.
   - `gpuInstance` at +76.
   - `bDmemStack` at +80.
   - `profilerArgs`, `sysmemHeapArgs`, `rmStateMonitorBufferArgs`, `bindataArgs` from +88 (computed).
2. This struct lives in a 4 KB sysmem page (kernel_gsp_tu102.c:180-201). `kgspSetupLibosInitArgs_IMPL` (kernel_gsp.c:6198-6240) registers it as a `LibosMemoryRegionInitArgument` (libos_init_args.h:49-56: `id8`, `pa`, `size`, u8 `kind`, u8 `loc`) with `id8` = "RMARGS" packed big-endian into a u64 (`_kgspGenerateInitArgId`, kernel_gsp.c:3219-3237). It comes after the log-buffer regions.
3. The PA of that libos-args array is written to `NV_PGSP_FALCON_MAILBOX0/1` = **0x110040/0x110044** (lo/hi) by `kgspProgramLibosBootArgsAddr_TU102` (kernel_gsp_tu102.c:392-402; dev_gsp.h:27-29). This happens before Booter Load (kernel_gsp_tu102.c:562). This `kgspBootstrap_TU102` path is used for TU10x, GA10x and AD10x (g_kernel_gsp_nvoc.c:742-744); GH100+ uses `kgspBootstrap_GH100`.

---

## 2. Send and receive paths

### Send
1. **Build the RPC header.** `rpcWriteCommonHeader` (rmapi/rpc_common.c:123-185):
   - Zeroes the message buffer.
   - `header_version` = 0x03000000, `signature` = 0x43505256 ("VRPC"), `rpc_result` = `rpc_result_private` = 0xFFFFFFFF (PENDING), `u.spare` = 0 (or `cpuRmGfid` under SR-IOV).
   - `function` = fn, `length` = 32 + paramLength.
   - `rpc_message_header_v` is 32 bytes (g_rpc-message-header.h:40-51): `header_version`, `signature`, `length`, `function`, `rpc_result`, `rpc_result_private`, `sequence`, `u`, then `rpc_message_data[]`.
2. **Assign the sequence.** `_kgspRpcSendMessage` (kernel_gsp.c:386-429):
   - Sync callers pass `&seq`, giving `sequence = pRpc->sequence++`. The counter starts at 0 (rpc_common.c:98).
   - Async callers pass NULL, giving sequence 0 **and no increment** (:403-406).
3. **Sanity check.** `_kgspRpcSanityCheck` (:275-321) rejects the send on fatal error, GPU reset/lost/shutdown, not full power, or no sysmem access.
4. **Write to the queue.** `GspMsgQueueSendCommand` (message_queue_cpu.c:478-626):
   - Checks the length (:491-497).
   - Zero-pads (:500), writes MCTP/NVDM/seqNum/checksum (:505-546).
   - For each element, spins on `msgqTxGetWriteBuffer(i)` with a 1 s timeout per element and `portAtomicMemoryFenceFull` between tries (:554-579). A timeout returns **NV_ERR_BUSY_RETRY** (:581-589).
   - Copies 4 KB per element (:595-597).
   - `portAtomicMemoryFenceStore` (:608).
   - `msgqTxSubmitBuffers(n)` advances `writePtr` modulo msgCount and stores it (msgq.c `msgqTxSubmitBuffers`). No notify, flush or barrier callbacks are registered on the CPU side: there are no `msgqSet*` calls outside the library.
   - `txSeqNum++` (:620).
5. **Doorbell.** `kgspSetCmdQueueHead_HAL(..., queueIdx=0, value=0)` (kernel_gsp.c:424) runs `GPU_REG_WR32(NV_PGSP_QUEUE_HEAD(0), 0)` = **write 0 to 0x110C00** (kernel_gsp_tu102.c:371-386; dev_gsp.h:38). The value written is always 0.
6. **Error mapping.** `_issueRpcAndWait` / `_issueRpcAsync` convert BUSY_RETRY to NV_ERR_GENERIC (vgpu/rpc.c:1900-1914, :2040-2054).

### Receive (polling)
1. `_issueRpcAndWait` calls `rpcRecvPoll(expectedFunc, expectedSequence)` (rpc.c:1990) → `_kgspRpcRecvPoll` (kernel_gsp.c:2847-3110).
   - Timeout is `defaultus × 1.5` on silicon (:2917-2928).
   - Recursive polling is forbidden (:2893).
2. The loop calls `_kgspRpcDrainEvents` (:1856-1882), which calls `_kgspRpcDrainOneEvent` (:1802-1843) repeatedly:
   - Full fence, then `GspMsgQueueReceiveStatus`.
   - If `function == expectedFunc && sequence == expectedSequence`, it returns MORE_PROCESSING_REQUIRED: this is the reply.
   - Otherwise the message is dispatched as an event via `_kgspProcessRpcEvent`.
   - After draining, `kgspHealthCheck_HAL` runs; failure gives NV_ERR_RESET_REQUIRED (:1876-1877).
3. **Timeout handling.** On timeout, the timeout is classified (`_kgspClassifyGspTimeout` :2732, heartbeat checks, `_kgspIsTimeoutFatal`) and NV_ERR_TIMEOUT is returned (:3003-3063).
4. **`GspMsgQueueReceiveStatus`** (message_queue_cpu.c:640-846):
   - Returns NV_WARN_NOTHING_TO_DO if the queue is empty (:663-668).
   - A partial read gives NOT_READY. Checksum, MCTP or sequence failures give INVALID_DATA. Each is retried up to 3 times (:654-792).
   - An invalid length gives INVALID_PARAM_STRUCT (:824-833).
   - The message is always consumed (`msgqRxMarkConsumed`, which writes the CPU's status `readPtr`) (:835-843).
5. **Result check** (rpc.c:2012-2025): if `rpc_result != 0`, it returns `rpc_result` (when < 0xFF000000), else NV_ERR_GENERIC.

### Receive (interrupt-driven, outside a synchronous wait)
- GSP raises **SWGEN0** (bit 6) in `NV_PFALCON_FALCON_IRQSTAT` (0x110008). In RISC-V mode the pending mask is IRQSTAT & `NV_PRISCV_RISCV_IRQMASK` & `IRQDEST`:
  - TU102: riscv base 0x111000 + 0x2b4/0x2b8 (kernel_falcon_tu102.c:384-392; dev_riscv_pri.h:27-33).
  - GA102: +0x528/0x52c.
- `kgspService_TU102` (kernel_gsp_tu102.c:1087-1169):
  - For HALT (bit 4): writes IRQSCLR (0x110004) bit 4, then dumps logs and runs the health check (CrashCat) (:1118-1133).
  - For SWGEN0: writes IRQSCLR bit 6 **before** servicing (:1134-1141), then calls `kgspRpcRecvEvents`, which drains with expectedFunc = NUM_FUNCTIONS (kernel_gsp.c:6243-6261).
  - Retrigger: `NV_PFALCON_FALCON_INTR_RETRIGGER(0)` = base+0x3e8 on GA100+ (kernel_falcon_ga100.c:37-45). On TU10x it is a no-op stub (g_kernel_falcon_nvoc.c:666-673).
- The GSP engine is registered as `MC_ENGINE_IDX_GSP` (kernel_gsp.c:6377-6397). Its CPU interrupt vector is **supplied by GSP** through `NV2080_CTRL_CMD_INTERNAL_INTR_GET_KERNEL_TABLE` (intr.c:1018-1076).
- Top-level registers on TU102: `NV_VIRTUAL_FUNCTION_PRIV_CPU_INTR_TOP(i)` = 0xB80000+0x1600+4i and `..._LEAF(i)` = 0xB80000+0x1000+4i. They are read through `GPU_VREG_RD32` with `virtualRegPhysOffset` = `DRF_BASE(NV_VIRTUAL_FUNCTION_FULL_PHYS_OFFSET)` = 0xB80000 (dev_vm.h:28-65; kern_gpu_tu102.c:100; intr_cpu_tu102.c:60,100,147).
- `dev_ctrl.h` (TU102) only defines `NV_CTRL_CPU_DOORBELL_VECTORID` (0xB6687C) and the legacy vector bases. It is not used by the GSP queue path.
- The init path (INIT_DONE, `SET_GUEST_SYSTEM_INFO`, `GET_GSP_STATIC_INFO`) is **polled**, so interrupts are not required to get through `kgspInitRm`.

---

## 3. RPC function numbers

Enum lists are in src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:
- CPU→GSP functions: `X(...)` 0–229, `NUM_FUNCTIONS` = 230 (:10-239).
- GSP→CPU events: 0x1000–0x1029, `NUM_EVENTS` = 0x102a (:245-290).

### CPU→GSP functions actually sent in GSP-client mode

| # | Name | Where / notes |
|---|---|---|
| 72 | `GSP_SET_SYSTEM_INFO` | Init, **async**, seq 0 (rpc.c:10432-10470) |
| 73 | `SET_REGISTRY` | Init, **async**, seq 0; uses the large path if > 65488 bytes (rpc.c:10477-10543) |
| 1 | `SET_GUEST_SYSTEM_INFO` | Sync, after INIT_DONE (kernel_gsp.c:5153; rpc.c:8730-8911). `SET_GUEST_SYSTEM_INFO_EXT` (64) is **not** sent because it uses `GPU_GET_VGPU_RPC`, which is NULL when not virtual (rpc_vgpu.h:71-78; gpu.c:6357-6364) |
| 65 | `GET_GSP_STATIC_INFO` | Sync (kernel_gsp.c:5160; rpc.c:9669-9707) |
| 228 | `INIT_GSP_TRACE_CRASH_BUFFER` | Only if `KERNEL_GSP_TRACING_RATS_ENABLED` and the regkey is set (kernel_gsp.c:3919-3998, :5167) |
| 103 | `GSP_RM_ALLOC` | rpc.c:10945-11065 |
| 76 | `GSP_RM_CONTROL` | rpc.c:10659-10943 |
| 10 | `FREE` | rpc.c:11120-11165 |
| 21 | `DUP_OBJECT` | rpc.c:11067-11118 |
| 71 | `CONTINUATION_RECORD` | Large RPCs (rpc.c:2058-2244) |
| 47 | `UNLOADING_GUEST_DRIVER` | Sync (kernel_gsp.c kgspUnloadRm ~:5227; rpc.c:9146-9170) |
| 70 | `UPDATE_BAR_PDE` | kern_bus.c:899, kern_bus_gm107.c:2096; rpc.c:9709-9732 |
| 4 | `ALLOC_MEMORY` | GSP path is `memRegisterWithGsp` → PTE-desc RPC (mem.c:513-575; rpc.c:3471-3481) |
| 78 | `DUMP_PROTOBUF_COMPONENT` | nv_debug_dump.c:198, journal.c:1218; rpc.c:10556-10583 |
| 202 | `ECC_NOTIFIER_WRITE_ACK` | **Async**, sent from the RC handler (kernel_gsp.c:611; rpc.c:11172-11188) |

Possibly reachable in GSP mode; the macros have no FW-client guard:
- 14/15 `MAP_MEMORY_DMA`/`UNMAP_MEMORY_DMA` (virtual_mem.c:1537/1892, when `!bSplitVasBetweenServerClientRm`, rpc.h macro)
- 22 `IDLE_CHANNELS` (kernel_idle_channels.c:44)
- 200 `INVALIDATE_TLB` (kern_gmmu_gm107.c:249, when `bDoVgpuRpc`)
- 50 `GPU_EXEC_REG_OPS` (subdevice_ctrl_gpu_regops.c:224)

Other numbers are used by the vGPU guest (`IS_VIRTUAL`) only.

### GSP→CPU events and CPU handling
Dispatch is in `_kgspProcessRpcEvent`, kernel_gsp.c:1398-1616.

| Event | Name | CPU action |
|---|---|---|
| 0x1001 | `GSP_INIT_DONE` | Consumed only by the polling wait (`kgspWaitForRmInitDone`); seen elsewhere it is logged "Unexpected" (:1587) |
| 0x1002 | `GSP_RUN_CPU_SEQUENCER` | **Unhandled**, logged (default case) |
| 0x1003 | `POST_EVENT` | `_kgspRpcPostEvent` (:499-582): looks up hClient/hEvent, notifies |
| 0x1004 | `RC_TRIGGERED` | `_kgspRpcRCTriggered` (:584-721) |
| 0x1005 | `MMU_FAULT_QUEUED` | `osQueueMMUFaultHandler` (:1001-1010) |
| 0x1006 | `OS_ERROR_LOG` | `nvErrorLog2_va` (Xid) (:723-764) |
| 0x1007 | `RG_LINE_INTR` | Unhandled |
| 0x1008 | `GPUACCT_PERFMON_UTIL_SAMPLES` | :766-827 |
| 0x1009 / 0x100a | `SIM_READ` / `SIM_WRITE` | Simulation only (:1012-1061) |
| 0x100b | `SEMAPHORE_SCHEDULE_CALLBACK` | :1063-1089 |
| 0x100c | `UCODE_LIBOS_PRINT` | Only `ENG_PMU` accepted, otherwise assert (:1120-1146) |
| 0x100d | `VGPU_GSP_PLUGIN_TRIGGERED` | :1148 |
| 0x100e | `PERF_GPU_BOOST_SYNC_LIMITS_CALLBACK` | `kperfDoSyncGpuBoostLimits` (:829-857) |
| 0x100f | `PERF_BRIDGELESS_INFO_UPDATE` | :859 |
| 0x1010 | `VGPU_CONFIG` | :1164 |
| 0x1011 | `DISPLAY_MODESET` | Unhandled |
| 0x1012 | `EXTDEV_INTR_SERVICE` | :1181 |
| 0x1013–0x1017 | `NVLINK_INBAND_RECEIVED_DATA_*` | :885-961 |
| 0x1018 | `TIMED_SEMAPHORE_RELEASE` | :1091 |
| 0x1019 | `NVLINK_IS_GPU_DEGRADED` | :963 |
| 0x101a | `PFM_REQ_HNDLR_STATE_SYNC_CALLBACK` | :1304 |
| 0x101b | `NVLINK_FAULT_UP` | :871 |
| 0x101c | `GSP_LOCKDOWN_NOTICE` | Sets `bInLockdown` (:1345-1364) |
| 0x101d | `MIG_CI_CONFIG_UPDATE` | :1213 |
| 0x101e | `UPDATE_GSP_TRACE` | :1254 |
| 0x101f | `NVLINK_FATAL_ERROR_RECOVERY` | :980 |
| 0x1020 | `GSP_POST_NOCAT_RECORD` | :1272 |
| 0x1021 | `FECS_ERROR` | :1622 |
| 0x1022 | `RECOVERY_ACTION` | :1682 |
| 0x1023 | `TRIGGER_BUGCHECK` | `osBugCheck` (:1735) |
| 0x1024 | `BIND_BAR2` | Unhandled |
| 0x1025 | `FORCED_DRIVER_SHUTDOWN` | Marks for reset, sets fatal (:1719) |
| 0x1026 | `GSP_LOAD_EXEC_GENERIC_BOOTLOADER` | TU10x/GA100: waits for GSP suspend (MAILBOX0 bit 31), resets the falcon, runs the bootloader, resumes RISC-V (kernel_gsp_falcon_tu102.c:510-570) |
| 0x1027 | `GSP_LOAD_EXEC_HS_BINARY` | GA102+: DMAs the HS ucode into the GSP falcon and runs it (kernel_gsp_falcon_ga102.c:311+) |
| 0x1028 | `UPDATE_GRID_DISPLAYLESS_PARAMS` | :1752 |
| 0x1029 | `PMU_HALTED` | Sets `bFatalError` (:1665) |

- **Bootup allow-list.** While waiting for INIT_DONE without the API lock (relaxed locking, the Linux default), only these events are processed: 0x100c, 0x101c, 0x1020, 0x1001, 0x1006, 0x101a, 0x1026, 0x1027 (:1418-1438). Anything else hits `NV_ASSERT(0)` and is skipped.
- **Required for init: only `GSP_INIT_DONE`.** The others are optional notifications.

---

## 4. Init handshake, in order (`kgspInitRm_IMPL` kernel_gsp.c:4877-5210 → `_kgspBootGspRm` :4795-4875 → `kgspBootstrap_TU102` kernel_gsp_tu102.c:521-619)

1. **Before boot (CPU only).**
   - The queues are created at construct time (`_kgspInitRpcInfrastructure` :3116-3150, `_kgspConstructRpcObject` :3158-3195). The CPU writes its cmdQ TX header then.
   - The CPU fills libos args and RM args (:5053, :5056) and prepares the registry and system info (:5061-5062).
2. **Boot.** FWSEC runs, the falcon resets into RISC-V, MAILBOX0/1 are programmed (:562), and Booter Load runs (:566).
3. **CPU sends `GSP_SET_SYSTEM_INFO` (72) then `SET_REGISTRY` (73)**, both async with seq 0 (`kgspSendInitRpcs_IMPL` kernel_gsp.c:4685-4709; kernel_gsp_tu102.c:576-584). These are element seqNum 0 and 1 unless the registry is large. The CPU does **not** wait for replies.
4. The CPU writes `FALCON_OS` and requires RISC-V active or processor-suspended (:588-600).
5. **`GspStatusQueueInit`** (:611): GSP must have written a valid status-queue `msgqTxHeader` within 4 s.
6. **CPU polls for `GSP_INIT_DONE`** (:614 → kernel_gsp.c:6264-6313). Requirements:
   - function 0x1001, **sequence 0** (:6282)
   - `rpc_result == 0` (:6288)
   - payload `rpc_init_done_v17_00 { NvBool bIsD3Hot; }` (g_rpc-structures.h:1589-1592)
   - Events from the allow-list may be sent before it.
7. **`SET_GUEST_SYSTEM_INFO` (1)**, sync, seq 0 (kernel_gsp.c:5153). Request: `rpc_set_guest_system_info_v03_00` (g_rpc-structures.h:36-47):
   - Fields: `vgxVersionMajorNum`, `vgxVersionMinorNum`, three buffer lengths, `guestClNum`, then `guestDriverVersion[256]`, `guestVersion[256]`, `guestTitle[256]`.
   - CPU sets major 0x2E and minor 0x0D (vgpu_version.h:33-34).
   - The reply needs `rpc_result` = 0 **and `rpc_result_private` = 0** (rpc.c:8841). Otherwise its status becomes the error (or a version-renegotiation retry, :8841-8878).
   - **The reply payload must contain `vgxVersionMajor/Minor`** (echo 0x2E/0x0D). The CPU stores them (:8891) and calls `rpcSetIpVersion` with them (:8896-8901), which re-selects the RPC HAL. An ipVersion below 0x14000000 turns `rpcGetGspStaticInfo` into a STUB (g_rpc_private.h:2949-2950).
8. **`GET_GSP_STATIC_INFO` (65)**, sync, seq 1 (kernel_gsp.c:5160). The reply payload is the full `GspStaticConfigInfo` at `rpc_message_data` offset 0. The CPU copies `sizeof(GspStaticConfigInfo)` regardless of the reply length (rpc.c:9700-9703).
9. Optional `INIT_GSP_TRACE_CRASH_BUFFER` (228).
10. `_kgspInitGpuProperties` (:6610-6622) and `_kgspSetFwWprLayoutOffset` (:4711-4728).
11. `kgspInitRm` returns. `RmInitNvHal` / state init follows (osinit.c:2113-2150), issuing many `GSP_RM_ALLOC` and `GSP_RM_CONTROL` calls:
    - The first allocs use the internal handles taken from static info (gpu.c:984-991).
    - There are about 299 `GPU_GET_PHYSICAL_RMAPI` call sites and 252 distinct `*_CTRL_CMD_INTERNAL_*` names in src/nvidia/src.
    - One of them is `INTR_GET_KERNEL_TABLE` (intr.c:1041).

### `GspSystemInfo` (CPU→GSP; gsp_static_config.h:170-229; filled at kernel_gsp.c:4470-4642)
- Physical addresses: `gpuPhysAddr`, `gpuPhysFbAddr`, `gpuPhysInstAddr`, `gpuPhysIoAddr`.
- PCI identity: `nvDomainBusDeviceFunc`, `PCIDeviceID`, `PCISubDeviceID`, `PCIRevisionID`.
- Shared buffers: `simAccessBufPhysAddr`, `notifyOpSharedSurfacePhysAddr`.
- PCIe: `pcieAtomicsOpMask`, `pcieAtomicsCplDeviceCapMask`, `pciConfigMirrorBase/Size`, `pcieConfigReg.linkCap`, `pciePowerControlValue`, `bPciePowerControlPresent`, `pf0DeviceControl2Reg`.
- Chipset/ASPM: `oorArch`, `clPdbProperties`, `Chipset`, `bGpuBehindBridge`, `bFlrSupported`, `b64bBar0Supported`, `bMnocAvailable`, `chipsetL1ssEnable`, `bUpstreamL0s/L1Unsupported`, `bUpstreamL1PorSupported/MobileOnly`, `upstreamAddressValid`, `FHBBusInfo`, `chipsetIDInfo`.
- Platform: `consoleMemSize`, `maxUserVa`, `bSystemHasMux`, `acpiMethodData`, `hypervisorType`, `virtualConfigBits`, `bIsPassthru`, `sysTimerOffsetNs`, `gspVFInfo`, `bIsPrimary`, `hostPageSize`, `bS0ixSupport`, `bIsCxlDevice`.
- Feature flags: `bIsUnixHdmiFrlComplianceEnabled`, `isGridBuild`, `gridBuildCsp`, `bPreserveVideoMemoryAllocations`, `bTdrEventSupported`, `bFeatureStretchVblankCapable`, `bEnableDynamicGranularityPageArrays`, `bClockBoostSupported`, `bIsCmcBasedHws`, `bGspNocatEnabled`, `bWindowChannelAlwaysMapped`.
- An emulator only needs to accept it.

### `SET_REGISTRY` payload
`PACKED_REGISTRY_TABLE { NvU32 size; NvU32 numEntries; PACKED_REGISTRY_ENTRY entries[]; }`. Each entry is `{ nameOffset, u8 type(1=DWORD, 2=BINARY, 3=STRING), data, length }`, followed by strings/data (g_os_nvoc.h:222-244).

### `GspStaticConfigInfo` (GSP→CPU; gsp_static_config.h:79-167) and consumers
| Field(s) | Consumer |
|---|---|
| `gidInfo` | gpu_gspclient.c:179-188 (all-zero GID is treated as missing) |
| `SKUInfo` | subdevice_ctrl_gpu_kernel.c:3749; gpu_boost_mgr.c:714 |
| **`fbRegionInfoParams`** | Required: `numFBRegions` ≥ 1 and ≤ `MAX_FB_REGIONS`, otherwise error (mem_mgr_gsp_client.c:59-73). Region base/limit/reserved etc. are copied (:76-110) |
| **`fb_length`** | → `fbTotalMemSizeMb`, asserted ≤ address-space size from the last region's limit (:118-123) |
| `bPdiValid`, `pdi` | gpu_gspclient.c:153 |
| `sriovCaps`, `sriovMaxGfid` | gpu_gspclient.c:65-69 |
| `engineCaps[]` | gpu.c:6834-6842 (the engine list) |
| `poisonFuseEnabled` | gpu_gspclient.c:199 |
| `gpuNameString`, `gpuShortNameString` | gpu_gspclient.c:296, 311 |
| `bGpuInternalSku`, `bIsQuadro*`, `bIsNvidiaNvs`, `bIsVgx`, `bGeforceSmb`, `bIsTitan`, `bIsTesla` | gpu_gspclient.c:53, 132-138 |
| `bIsMobile`, `bIsGc6/Gc8/GcOffRtd3Allowed`, `bIsGcoffLegacyAllowed`, `bIsGpuUefi`, `bIsEfiInit` | kernel_gsp.c:6613-6621 |
| `bIsMigSupported` | gpu.c:7715 |
| `RTD3GC6TotalBoardPower` / `PerstDelay` | gpu_gspclient.c:208-209 |
| **`bar1PdeBase`, `bar2PdeBase`** | kern_bus.c:794, 867 |
| `bVbiosValid`, `vbiosSubVendor/SubDevice/Revision/OemRevision` | gpu.c:2557-2560; gpu_boost_mgr.c |
| `bPageRetirementSupported` | gpu_gspclient.c:125 |
| `bSplitVasBetweenServerClientRm` | Gates MAP/UNMAP_MEMORY_DMA |
| **`hInternalClient/Device/Subdevice`** | gpu.c:989-991 (handles used for every internal control) |
| `bSelfHostedMode` | kern_gpu_gh100.c:428 |
| `bAtsSupported`, `bSysL2CacheCoherentMode` | kern_mem_sys.c:191-198 |
| `ecidInfo` | subdevice_ctrl_gpu_kernel.c:136-176 |
| `fwWprLayoutOffset` | kernel_gsp.c:4726-4727 (only if `PDB_PROP_GPU_FW_WPR_OFFSET_SET_BY_ACR`) |
| `bBusResetRequired` | gpu_gspclient.c:385 |
| `bClRootportNeedsNosnoopWAR` | chipset_pcie.c:4952 |

---

## 5. GSP_RM_ALLOC and GSP_RM_CONTROL serialization, and the kernel/physical split

### `rpc_gsp_rm_control_v03_00` (g_rpc-structures.h:1423-1435)

| Offset | Field |
|---|---|
| +0 | `hClient` |
| +4 | `hObject` |
| +8 | `cmd` |
| +12 | `status` |
| +16 | `paramsSize` |
| +20 | `rmapiRpcFlags` |
| +24 | `rmctrlFlags` |
| +28 | `rmctrlAccessRight` |
| +32 | `reserved0` (u64) |
| +40 | `params[]` |

- **CPU request** (rpc.c:10659-10943):
  - `status` = 0.
  - `rmapiRpcFlags`: COPYOUT_ON_ERROR (bit 0) if the control has `RMCTRL_FLAGS_COPYOUT_ON_ERROR`; SERIALIZED (bit 1) if FINN-serialized (rmapi.h:161-163; rpc.c:10792-10803).
  - `rmctrlFlags` = `rmctrlAccessRight` = 0.
- **Reply handling:**
  - If header `rpc_result` ≠ 0, the RPC fails and there is no copyout.
  - Else, if `params.status` ≠ 0 and the **reply's** `rmapiRpcFlags` lacks COPYOUT_ON_ERROR, it returns `params.status` with no copyout.
  - Else it copies out: for flat params, the CPU's **own** `paramsSize` bytes; for serialized params, the reply `paramsSize` bytes into the serialized buffer, then FINN-deserializes. It then returns `params.status` (:10856-10892).
  - For GSS-legacy caching the CPU reads the reply's `rmctrlFlags` / `rmctrlAccessRight` (:10884-10891).
- **Large payloads** (> maxRpcSize − 72): the request is split into the first chunk (fn 76, seq s) plus CONTINUATION_RECORD (71) chunks with seq s+1… The reply must be the same shape: the first reply has function 76, seq s; then continuation records with seq s+1…, each with `length` = 32 + chunk (rpc.c:2058-2244, bidirectional=TRUE at :10841).

### `rpc_gsp_rm_alloc_v03_00` (g_rpc-structures.h:1408-1419)

| Offset | Field |
|---|---|
| +0 | `hClient` |
| +4 | `hParent` |
| +8 | `hObject` |
| +12 | `hClass` |
| +16 | `status` |
| +20 | `paramsSize` |
| +24 | `flags` |
| +28 | `reserved[4]` |
| +32 | `params[]` |

- The request is capped at one message: no large path, `BUFFER_TOO_SMALL` otherwise (rpc.c:11021-11030).
- Root/client allocations force `paramsSize = sizeof(NV0000_ALLOC_PARAMETERS)` (:10985-10988).
- On `rpc_result` == 0 the CPU copies `paramsSize` bytes back from `params` (copyout) and **does not look at `params.status`** (:11040-11047). On failure it returns `params.status` (:11048-11054). **An emulator must therefore put the alloc error in `rpc_result` as well.**
- Alloc serialization is effectively never used: `serverSerializeAllocDown` uses FINN interface 0xA000 (rmapi_finn.c:356-378), and the generated serializer has no entries for it (grep of g_finn_rm_api.c).

### FINN serialization (controls only)
- The CPU serializes only when FINN knows the command: `FinnRmApiGetSerializedSize(cmd>>8, cmd&0xff) != 0` (rmapi_finn.c:66-150).
- The format is a bit-packed stream with a `FINN_RM_API` header (u64 `version`=0, `payloadSize`, `interface`, `message`) (g_finn_rm_api.h:31-46) and a checksum (g_finn_rm_api.c:360-384).
- The serializable set is small, the controls with embedded pointers (g_finn_rm_api.c message list):
  - `NV0000_CTRL_NVD_GET_DUMP`
  - `NV0080_CTRL_DMA_UPDATE_PDE_2`, `FB/FIFO/GR/HOST/NVENC_GET_CAPS`, `FIFO_GET_CHANNELLIST`, `GPU_GET_CLASSLIST`
  - `NV2080_CTRL_BIOS_GET_NBSI_OBJ`, `CE_GET_CAPS`, `GPU_GET_ENGINES`, `GPU_GET_ENGINE_CLASSLIST`, `GPU_RPC_GSP_TEST`, `GSP_CRYPTO_CONTROL`, `I2C_ACCESS`, `NVD_GET_DUMP`, `RC_READ_VIRTUAL_MEM`
  - `NV402C I2C_INDEXED`/`TRANSACTION`, `NV83DE DEBUG_READ`/`WRITE_MEMORY`, `NVB06F` ctx-data save/restore/get
- Everything else is sent raw (flat param struct).

### Kernel/physical split
- **Controls.**
  - A control with `RMCTRL_FLAGS_ROUTE_TO_PHYSICAL` (0x40, control.h:230) is forwarded unchanged to GSP as `GSP_RM_CONTROL`, carrying the **user's** hClient/hObject. This happens in `rmresControl_Prologue` (resource.c:258-293), with serialize/deserialize in :210-251.
  - About 753 of 1372 exported-method table entries in g_*_nvoc.c have bit 0x40 (count from scanning `/*flags=*/` values). Per-command flags are in the generated `__nvoc_exported_method_def_*` tables, e.g. g_subdevice_nvoc.c:3997.
  - Controls without the flag run in kernel RM. Their `_KERNEL`/`_IMPL` bodies often call `GPU_GET_PHYSICAL_RMAPI(pGpu)->Control(hInternalClient, hInternalSubdevice, *_CTRL_CMD_INTERNAL_*)`, which also becomes `GSP_RM_CONTROL`.
  - The CPU caches `RMCTRL_FLAGS_CACHEABLE` results (rpc.c:10699-10760).
- **Allocations.**
  - Classes with `RS_FLAGS_ALLOC_RPC_TO_PHYS_RM` (NVBIT(6), resource_desc_flags.h:39; `RPC_TO_ALL` and `CHANNEL_DESCENDANT_COMMON` include it) are first constructed in kernel RM, then mirrored with `NV_RM_RPC_ALLOC_OBJECT` → `GSP_RM_ALLOC`, and flagged `bRpcFree` (alloc_free.c:878-930). They are freed with `NV_RM_RPC_FREE` → FREE (10).
  - These classes include display, all CE/NVDEC/NVENC/NVJPG/OFA/GR classes, `FERMI_CONTEXT_SHARE_A`, `UVM_CHANNEL_RETAINER`, `GT200_DEBUGGER`, `NV01_TIMER`, `NV40_I2C`, `GF100_ZBC_CLEAR`, `GF100_SUBDEVICE_MASTER`/`INFOROM`, `NV20_SUBDEVICE_DIAG`, `NV2081`/`NV2082_BINAPI`, `GF100_PROFILER`, MMU access-bit buffers, SMC partition refs and SW classes (resource_list.h lines 185-2292).
- **Classes that RPC from their own constructors:** NV01_ROOT + NV01_DEVICE_0 (`NV_RM_RPC_ALLOC_SHARE_DEVICE_FWCLIENT`, rpc.h:48-110), subdevice, channels and events (`NV_RM_RPC_ALLOC_SUBDEVICE`/`_CHANNEL`/`_EVENT`, which use the FW-client branch, rpc.h), and memory via `ALLOC_MEMORY` (4) in `memRegisterWithGsp`.
- **Guidance for an emulator:** serve `GSP_RM_ALLOC`, `GSP_RM_CONTROL`, `FREE` and `DUP_OBJECT` for the classes and controls above, plus the internal-client controls kernel RM issues during `RmInitNvHal`.

---

## 6. Open questions (with pointers)
1. **When real GSP sends 0x1026/0x1027** (load/exec bootloader or HS binary): the CPU side does not show when (kernel_gsp.c:431-458, 1579-1585). Suspend/unload or FRTS/booter flows are the likely candidates. The GSP firmware is not in this tree.
2. **`sizeof(GspStaticConfigInfo)` and `sizeof(GspSystemInfo)`** were not computed; they depend on the NV2080 param structs. Verify both are ≤ 65488 bytes (rpc.c:9680-9689, 10445-10453) and match exactly.
3. **GSP-side element padding and checksum:** the CPU reads up to an 8-byte boundary past `16 + length` (message_queue_priv.h:192-206). The GSP must zero-pad or include the pad bytes in its checksum.
4. **`SWAP_RX` on the GSP-written status header:** the CPU works either way, but the pointer locations differ (msgq.c `msgqRxLink` swap branches). Real GSP convention was not confirmed from source.
5. **Which `GSP_RM_CONTROL` / `GSP_RM_ALLOC` calls happen during `RmInitNvHal` and state load, in order:** this needs runtime tracing. Entry point osinit.c:2150. The first one is likely `_gpuAllocateInternalObjects` (gpu.c:975).
6. **Whether 14/15/22/200/50 are actually emitted in GSP-client mode** depends on runtime flags (`bRpcAlloc`, `bDoVgpuRpc`, `bSplitVasBetweenServerClientRm`) (virtual_mem.c:1520, kern_gmmu_gm107.c:247, subdevice_ctrl_gpu_regops.c:170).
7. **Heartbeat registers** `NV_PGSP_MAILBOX(0)`=0x110804 (GSP-RM) and `(1)`=0x110808 (LibOS) are only read for timeout diagnostics (kernel_gsp.c:2308-2327; dev_gsp.h:35). No hard requirement was found.
8. **Register-level fakes needed before the queue is used** (outside this scope): GFW_BOOT, WPR2 check (kernel_gsp_tu102.c:1171-1181), Booter/FWSEC completion, RISC-V active or MAILBOX0 bit 31 "suspended" (kernel_gsp_tu102.c:331-360, 592).

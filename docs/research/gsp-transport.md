<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GSP transport

Scope: the CPU-GSP transport of 615.71.09 on TU106 (message queue v2, element framing, GMCAPI and `GSP_INIT`, the doorbell, boot, teardown, suspend) and where NVIDIA draws the stable line.

## Facts

### Queues and pointers (msgq v2)

- The shared buffer is one sysmem allocation: page table, command queue (0x40000), status queue (0x40000 unless `RmGspStatusQueueSize`); in the capture 129 pages, command queue at 0x1000, status queue at 0x41000 [docs/caraxes/gsp-transport-615.md §2.1; run: caraxes/runs/guest-driver-2026-09-24/615.71.09/unlinked].
- Each side writes only a 32-byte `msgqTxHeader` at the start of its own transmit queue: `versionMajor` 2, `versionMinor` 0, `size`, `msgSize` 0x1000, `msgCount` 63, `entryOff` 0x1000 [615 tree: src/nvidia/src/libraries/msgq/msgq.c:126-140].
- The header versions itself: "Major versions get increased with incompatible changes", "Minor versions are compatible" [615 tree: src/nvidia/inc/libraries/msgq/msgq_priv.h:31-34].
- The receiver's `msgqRxLink` fails with -7 when the peer header's `size` does not match, which is where a 610 device stops a 615 guest [615 tree: src/nvidia/src/libraries/msgq/msgq.c:246-249; docs/caraxes/guest-driver-versions.md §5.1].

| Register (queue 0) | BAR0 | Written by | Holds | Evidence |
|---|---:|---|---|---|
| `NV_PGSP_QUEUE_HEAD(0)` | 0x110C00 | CPU | command-queue write pointer | [615 tree: src/common/inc/swref/published/turing/tu102/dev_gsp.h:38] |
| `NV_PGSP_QUEUE_TAIL(0)` | 0x110C04 | GSP | command-queue read pointer | [615 tree: src/common/inc/swref/published/turing/tu102/dev_gsp.h:41] |
| `NV_PGSP_MSGQ_HEAD(0)` | 0x110C80 | GSP | status-queue write pointer | [615 tree: src/common/inc/swref/published/turing/tu102/dev_gsp.h:44] |
| `NV_PGSP_MSGQ_TAIL(0)` | 0x110C84 | CPU | status-queue read pointer | [615 tree: src/common/inc/swref/published/turing/tu102/dev_gsp.h:47] |

- `gpuGetGspMsgQueueRegisters_TU102` maps queue index 0 to these four registers, and the same HAL serves TU10x through GB20x [615 tree: src/nvidia/src/kernel/gpu/arch/turing/kern_gpu_tu102.c:596-605; docs/caraxes/gsp-transport-615.md §2.2].
- Each register holds a monotonic counter used modulo `msgCount`; the ring is full at `write - read == msgCount`, so all 63 slots are usable [615 tree: src/nvidia/inc/libraries/msgq/msgq_priv.h:55-63].
- 610 delta: pointers lived in shared memory (`writePtr` in a v0 header plus a 4-byte `msgqRxHeader`), one slot always free, element header 16 bytes with `checkSum` and `seqNum` [docs/caraxes/gsp-transport-615.md §2.1, §9].

### The doorbell

- The protocol defines no notification; a write to `NV_PGSP_QUEUE_HEAD` raises a hardware interrupt on GSP, which is how GSP learns of new messages [615 tree: src/nvidia/inc/libraries/msgq/msgq_priv.h:75-77].
- The sender copies the slots, issues a store fence, then writes `QUEUE_HEAD` [615 tree: src/nvidia/src/kernel/gpu/gsp/message_queue_cpu.c:597-607; 615 tree: src/nvidia/src/libraries/msgq/msgq.c:370-371].
- The receiver reads `MSGQ_HEAD`, then issues a load fence before it reads slots [615 tree: src/nvidia/src/libraries/msgq/msgq.c:408-409].
- The device serves the command queue on every trapped `QUEUE_HEAD` write, with the written value as the write pointer, and answers reads of `QUEUE_TAIL` and `MSGQ_HEAD` from its queue state [docs/caraxes/gsp-transport-615.md §2.3].
- A `QUEUE_HEAD` write of 0 while the device's read pointer is not 0 means the queue was created anew (reload without reset), and the device drops the link [docs/caraxes/gsp-transport-615.md §2.3].

### The element (24 bytes of MCTP framing)

| Offset | Field | Value | Evidence |
|---:|---|---|---|
| 0 | `mctpMagic` | 0x4D435450 "MCTP" | [615 tree: src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h:92] |
| 4 | `mctpPayloadSize` | 24 + payload bytes | [docs/caraxes/gsp-transport-615.md §3.1] |
| 8 | `mctpHeader` | 0xC0000001 (version 1, SOM, EOM) | [docs/caraxes/gsp-transport-615.md §3.1] |
| 12 | `nvdmHeader` | 0x2510DE7E RM RPC, 0x2610DE7E GMCAPI | [615 tree: src/nvidia/arch/nvalloc/common/inc/nvdm_format.h:62-63] |
| 16 | `nvdmPayloadSize` | payload bytes | [docs/caraxes/gsp-transport-615.md §3.1] |
| 20 | `reserved` | 0 | [docs/caraxes/gsp-transport-615.md §3.1] |

- An element spans at most 16 slots of 4096 bytes, so `maxRpcSize` is 0x10000 - 24 = 65512 and larger RPCs go out as `CONTINUATION_RECORD`s [615 tree: src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h:180-184; docs/caraxes/gsp-transport-615.md §3.1].
- The element has no checksum and no sequence number; the receiver instead re-reads up to three times on an out-of-range size and checks MCTP version 1, vendor 0x10DE and the magic, each failing with `NV_ERR_INVALID_DATA` [615 tree: src/nvidia/src/kernel/gpu/gsp/message_queue_cpu.c:648-734].
- A consumed element's magic is overwritten with "SEEN", and an element that already reads "SEEN" is skipped as stale [615 tree: src/nvidia/src/kernel/gpu/gsp/message_queue_cpu.c:739-777].
- The store fence before the pointer write replaces the ordering guarantee, and the RPC and GMCAPI requests share one sequence counter in their own headers [615 tree: src/nvidia/src/kernel/gpu/gpu.c:7852; docs/caraxes/gsp-transport-615.md §3.2].
- With confidential compute a 16-byte tag follows the MCTP words; Turing has none and the device refuses `MESSAGE_QUEUE_INIT_FLAG_ENCRYPTION` [docs/caraxes/gsp-transport-615.md §3.1].

### GMCAPI and `GSP_INIT`

- `GMCAPI_HEADER` is 40 bytes: `command` (response flag 0x01000000), `size`, u64 `sequence`, `max_response_size` or `status`, reserved [615 tree: src/nvidia/interface/gmcapi/gmcapi_base.h:27; docs/caraxes/gsp-transport-615.md §4].
- GMCAPI ids up to 0xFFFF are reserved so they never collide with RPC function numbers [615 tree: src/nvidia/interface/gmcapi/gmcapi_base.h:76-80].
- Ids are `category << 16 | n`; the GSP management category holds `GSP_INIT` 0x10001, `EXEC_GENERIC_BOOTLOADER` 0x10002, `EXEC_HS_BINARY` 0x10003, `GSP_SUSPEND` 0x10004 (fire and forget), `GSP_RESUME_DONE` 0x10005 [615 tree: src/nvidia/interface/gmcapi/gmcapi_table.h:74-124].
- One `GMCAPI_CMD_GSP_INIT` replaces `GSP_SET_SYSTEM_INFO`, `SET_REGISTRY`, `GET_GSP_STATIC_INFO` and the wait for `GSP_INIT_DONE`, and its response is the init completion [615 tree: src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:5328-5344].
- NVKV is a stream of u64 words (key 15:0, index 27:16, opcode 31:28, immediate or count 63:32) with the opcodes IMM32, SEQ32, SEQ64, ARRAY8/32/64 [docs/caraxes/gsp-transport-615.md §5.1].
- The request is the system information (`NVGMC_SI_*`: PCI ids, BAR addresses, BDF, host page size, ACPI, flags, driver name and version string) plus every registry entry as `REGKEY_NAME`/`REGKEY_VALUE_*` [615 tree: src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:4893-5217].
- The captured first request has 296 words (2368 bytes), 142 keys and 44 registry entries [run: caraxes/runs/guest-driver-2026-09-24/615.71.09/linked; docs/caraxes/gsp-transport-615.md §5.2].
- The kernel offers 48 KiB for the response and decodes it strictly: a table key of the wrong type or index fails the init with `NV_ERR_INVALID_ARGUMENT`, a key outside the table is ignored [615 tree: src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:5321; docs/caraxes/gsp-transport-615.md §5.3].
- The device's TU106 response grew from 424 to 560 bytes of static keys once the sourced B2 keys were added [run: caraxes/runs/transport-615-2026-09-24/boot; run: caraxes/runs/gsp-rpc-2026-09-24/emu615].

### Boot exchange (Turing, `kgspBootstrap_TU102`)

| # | Direction | What | Evidence |
|---:|---|---|---|
| 1 | CPU | `msgqTxCreate` writes the command-queue header, then `QUEUE_HEAD(0) = 0` while the GSP falcon is halted | [615 tree: src/nvidia/src/libraries/msgq/msgq.c:175-176] |
| 2 | CPU | FWSEC-FRTS, reset into RISC-V, libos-args PA into `MAILBOX0/1` | [docs/caraxes/gsp-plan/boot-exchange.md §2 steps 12-13] |
| 3 | CPU to SEC2 | Booter Load with the WPR meta PA; the device links its status queue here, because step 4 comes without a doorbell | [docs/caraxes/gsp-plan/boot-exchange.md §2 step 14; docs/caraxes/gsp-transport-615.md §2.3] |
| 4 | CPU | `GspStatusQueueInit` polls `msgqRxLink` for 4 s, then writes `MSGQ_TAIL(0) = 0` | [615 tree: src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_tu102.c:469; 615 tree: src/nvidia/src/libraries/msgq/msgq.c:270-272] |
| 5 | CPU to GSP | `GMCAPI_CMD_GSP_INIT`, `QUEUE_HEAD(0) = 1` | [615 tree: src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_tu102.c:473] |
| 6 | GSP to CPU | the NVKV static configuration, CPU polls `MSGQ_HEAD(0)` then writes `MSGQ_TAIL(0)` | [docs/caraxes/gsp-transport-615.md §2.3 step 4] |
| 7 | CPU to GSP | `INIT_GSP_TRACE_CRASH_BUFFER` | [docs/caraxes/gsp-plan/boot-exchange.md §2 step 18] |
| 8 | CPU to GSP | internal clients, `GSP_RM_CONTROL`s, `UPDATE_BAR_PDE`, the event import's `ALLOC_MEMORY` | [docs/caraxes/gsp-plan/boot-exchange.md §2 step 20; run: caraxes/runs/transport-615-2026-09-24/boot] |

### Teardown and suspend

- Unload sends `GMCAPI_CMD_GSP_SUSPEND` with flags (PM_TRANSITION, GC6_ENTERING, HIBERNATE, FAST, FORCE) and waits for the processor-suspended bit 31 in MAILBOX0 [615 tree: src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:5869-5890; docs/caraxes/gsp-plan/boot-exchange.md §4].
- After that come the GSP falcon reset with FWSEC-SB, Booter Unload on SEC2 (argument 0, or the SR meta PA on suspend), and a function-level reset by the OS before the next load [docs/caraxes/gsp-plan/boot-exchange.md §4 steps 5-7; run: caraxes/runs/transport-615-2026-09-24/reload-a].
- On resume the kernel sets `IN_PM_TRANSITION` in RMARGS, skips the status-queue link ("GSP-RM will restore queue state") and waits for `GSP_RESUME_DONE` instead of sending `GSP_INIT` [615 tree: src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_tu102.c:463-478].
- The device keeps the model on a PM-transition suspend and sends `GSP_RESUME_DONE` as a GMCAPI request after the next Booter Load, tested only in unit tests [docs/caraxes/gsp-plan/boot-exchange.md §Status B5].
- GC6, GC8 and GCOFF stay off on a desktop TU106, because the device sends `POWER_FEATURES` 0 [docs/caraxes/gsp-plan/boot-exchange.md §5].

### Stable and unstable

- `NV_ABI_STABLE` is a no-op tag in every build, "for human benefit only", that static checkers may use "to verify no changes across releases" [615 tree: src/common/sdk/nvidia/inc/nvtypes.h:521-533].
- It tags 12 structs and 5 enums: `msgqTxHeader`, `GSP_MSG_QUEUE_ELEMENT`, `MESSAGE_QUEUE_INIT_ARGUMENTS`, `GSP_SR_INIT_ARGUMENTS`, `GSP_ARGUMENTS_CACHED`, `GMCAPI_HEADER`, six `GSP_FMC_*`/`GSP_*_PARAMS` boot structs (Hopper and later), and the enums `GMCAPI_HEADER_COMMAND_FLAGS`, `GMCAPI_FLAGS`, `GMCAPI_COMMANDS`, `NVGMC_ENGINE_TYPE`, `GMCAPI_VGPU_PLUGIN_EVENT` [docs/caraxes/gsp-abi-615.md §2; 615 tree: src/nvidia/arch/nvalloc/common/inc/gsp/gspifpub.h:50-127].
- Stable structs grow only into reserved space: "If extending any of the NV_ABI_STABLE sub-structs, make sure to remove the corresponding reserved fields from the parent struct" [615 tree: src/nvidia/arch/nvalloc/common/inc/gsp/gspifpub.h:33-34].
- NVIDIA calls the RM RPC layer an opt-in unstable interface: `NVGMC_SI_UNSTABLE_INTERFACE_SUPPORT` 0x1110 with bits RPC 0:0 and EVENTS 1:1, "Opt in to use unstable interfaces to GSP" [615 tree: src/nvidia/interface/gmcapi/gmcapi_system_info.h:200-203].
- The 615 kernel sets both bits (value 3) [615 tree: src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:5203-5206].
- Also unstable: the internal client, device and subdevice handles, and the NVKV keys still in `*_unstable.h` (615: `NVGMC_SI_CL_FLAGS` 0x1033) [615 tree: src/nvidia/interface/gmcapi/gmcapi_gsp_config.h:74-78; 615 tree: src/nvidia/interface/gmcapi/gmcapi_system_info_unstable.h:27-31].
- Queue encryption is explicitly "*NOT* ABI stable" [615 tree: src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h:45].
- The kernel still accepts only a GSP-RM image whose `.fwversion` equals its own version [615 tree: src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:4190; docs/caraxes/gsp-abi-615.md §3].

## Consequence for Caraxes

- The v2 transport (`msgq/v2.rs`, `gmcapi.rs`) is written once for every later driver whose stable tags hold; only payloads behind it stay per version (D14).
- The device must link its status queue at Booter Load, not at a doorbell, because the kernel polls for the GSP header before it sends anything.
- `QUEUE_HEAD` is the only trapped transport register that carries work; the other three are answered from queue state.
- The device honours the unstable-interface opt-in: without the RPC bit RM RPCs are refused `NV_ERR_NOT_SUPPORTED`, without the EVENTS bit no RM event is sent [docs/caraxes/gsp-plan/boot-exchange.md §Status B7].
- One device binary serves one guest ABI; `CARAXES_GUEST_ABI=615` selects the v615 build and the RMARGS magic "GSP " refuses a mismatch [docs/caraxes/gsp-transport-615.md §8].
- RPC functions, events and classes on this transport: see [gsp-rpc-events-classes.md](gsp-rpc-events-classes.md); snapshot and delta: see [abi-baseline.md](abi-baseline.md).

## Open

- Guest suspend in the VMM: whether the VMM's guest PM suspend reaches the NVIDIA PM callbacks and whether an FLR follows on resume [docs/caraxes/gsp-plan/boot-exchange.md §7 item 4].
- The real card's `GSP_INIT` response (`POWER_FEATURES`, `CHIP_SUB_REV`, `SKU_*`, `ECID_INFO`) needs one capture in a passthrough VM [docs/caraxes/gsp-plan/boot-exchange.md §7 item 1].
- What GSP-RM does for a client that does not opt in to unstable RPCs is not in the open tree [docs/caraxes/gsp-plan/boot-exchange.md §7 item 2].
- GSP-RM log buffers (LOGRM) stay empty; a line needs the per-version metadata of the guest's `gsp_tu10x.bin` [docs/caraxes/gsp-plan/boot-exchange.md §Status B6].
- Whether any Linux path reloads GSP-RM without an FLR [docs/caraxes/gsp-plan/boot-exchange.md §7 item 5].

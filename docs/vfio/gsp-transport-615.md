<!-- SPDX-License-Identifier: MIT -->
<!-- Written 2026-09-24 from the open-gpu-kernel-modules tree at tag 615.71.09 (commit 61dcc93,
     a shallow clone on the NAS). Appendix A is generated output of caraxes/tools/gsp-abi-sizes.sh
     --abi 615; do not edit it by hand, re-run the tool. -->
# The CPU-GSP transport of driver 615.71.09, as the device serves it

Decision D19 moves the project's baseline to driver 615.71.09 before the GSP-surface work
([roadmap.md](../history/caraxes-roadmap-2026-09-27.md)). From 615 on NVIDIA marks the GSP transport and boot structures
`NV_ABI_STABLE`. This note describes that transport end to end, as the 615 kernel RM drives it on
Turing, and how `crates/vfio-user-nvgpu` plays the GSP side of it.

Every claim cites file and line in the 615.71.09 tree. Paths are relative to its root. `sdk/` is
`src/common/sdk/nvidia/inc`. `gsp/` is `src/nvidia/src/kernel/gpu/gsp`. `tu102/` is
`src/nvidia/src/kernel/gpu/gsp/arch/turing`. A citation of 610 names the tree
(`vendor/open-gpu-kernel-modules` at 610.57.04).

The guest evidence is in [guest-driver-versions.md](guest-driver-versions.md) §5 and under
`caraxes/runs/guest-driver-2026-09-24/615.71.09/`. The device's first 615 runs are under
`caraxes/runs/transport-615-2026-09-24/` ([experiments.md](../history/caraxes-experiments-2026-09-29.md), "615 transport").

## Summary

1. In 615 the queue pointers left the shared memory. The CPU writes its command-queue write
   pointer to `NV_PGSP_QUEUE_HEAD(0)` and its status-queue read pointer to `NV_PGSP_MSGQ_TAIL(0)`.
   The GSP publishes its pointers in `NV_PGSP_QUEUE_TAIL(0)` and `NV_PGSP_MSGQ_HEAD(0)`. The
   pointers are monotonic counters, and a ring may fill completely. The queue memory keeps only a
   32-byte v2 `msgqTxHeader` per queue (`msgq_priv.h:31-91`, `msgq.c:80-485`).
2. The 16-byte element header of 610 (MCTP, NVDM, `checkSum`, `seqNum`) is now 24 bytes of MCTP
   framing: the magic "MCTP", `mctpPayloadSize`, the MCTP transport header, the NVDM header with
   the message type (0x25 RM RPC, 0x26 GMCAPI), `nvdmPayloadSize` and a reserved word. The
   element has no checksum and no sequence number. The store fence before the pointer write
   replaces them, together with the receiver's checks of magic, MCTP version, vendor id and size,
   and a "SEEN" mark on consumed elements (`message_queue_priv.h:51-121`,
   `message_queue_cpu.c:483-822`).
3. One `GMCAPI_CMD_GSP_INIT` (0x10001) replaces `GSP_SET_SYSTEM_INFO`, `SET_REGISTRY`,
   `GET_GSP_STATIC_INFO` and the wait for `GSP_INIT_DONE`. Its request is the system
   information and the whole registry as NVKV key/value words, and its response is the static
   configuration as NVKV words (`gsp/kernel_gsp.c:4893-5361, 7729-7886`). The kernel links the
   status queue right after Booter-Load and before it sends anything, so the GSP must create its
   queue first (`tu102/kernel_gsp_tu102.c:463-474`).
4. The boot arguments were reset and declared stable. `GSP_ARGUMENTS_CACHED` is 184 bytes with a
   header ("GSP ", size, flags), and the queue arguments move behind it with their fields
   reordered (`gsp_init_args.h:33-113`). The RPC headers, the alloc/control/free/dup payloads and
   the function numbers stay as in 610. Seven control structs change, and the control id
   0x20800A36 now names another control (§7).
5. The device serves 615 as a second transport behind the same queue interface
   (`msgq::v2`, `gmcapi`, the GMCAPI arm in `boot.rs`). The version module of the build selects
   it (`ver::v615`, feature `v615`). `CARAXES_GUEST_ABI=615` chooses that binary, and the device
   refuses to run for any other ABI. The 615.71.09 guest on vm3 reaches
   `RmInitAdapter succeeded!` and `nvidia-smi` answers, at boot and after a reload with a
   function-level reset.

## 1. Why 615: `NV_ABI_STABLE`

`sdk/nvtypes.h:521-533` defines a tag that a type carries between `struct` and its name. In every
driver build it expands to nothing. Under `NV_ABI_STABLE_CHECKED_BUILD` it becomes
`__attribute__((nv_abi_stable))`, which "static checker tools may support … and use … to verify no
changes across releases". In 615 the tag is on exactly the transport and boot structures:

| Structure | Where | Size |
|---|---|---:|
| `msgqTxHeader` | `src/nvidia/inc/libraries/msgq/msgq_priv.h:82-91` | 32 |
| `GSP_MSG_QUEUE_ELEMENT` | `src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h:90-121` | 24 (40 with CC) |
| `GMCAPI_HEADER` | `src/nvidia/interface/gmcapi/gmcapi_base.h:27-45` | 40 |
| the `GMCAPI_HEADER_COMMAND_FLAGS`, `GMCAPI_FLAGS` and `GMCAPI_COMMANDS` enums | `gmcapi_base.h:56-88` | |
| `NVGMC_ENGINE_TYPE` | `src/nvidia/interface/gmcapi/gmcapi_engine_types.h:33-52` | |
| `MESSAGE_QUEUE_INIT_ARGUMENTS`, `GSP_SR_INIT_ARGUMENTS`, `GSP_ARGUMENTS_CACHED` | `src/nvidia/inc/kernel/gpu/gsp/gsp_init_args.h:33-113` | 64, 16, 184 |
| `GSP_FMC_INIT_PARAMS`, `GSP_ACR_BOOT_GSP_RM_PARAMS`, `GSP_RM_PARAMS`, `GSP_SPDM_PARAMS`, `GSP_RM_MEM_PARAMS`, `GSP_FMC_BOOT_PARAMS` | `src/nvidia/arch/nvalloc/common/inc/gsp/gspifpub.h:50-149` | 8, 32, 16, 16, 8, 256 |

The encryption tag is explicitly not stable ("GSP Message Queue encryption is currently *NOT* ABI
stable", `message_queue_priv.h:45`). The GSP-FMC parameters belong to the Hopper-and-later boot
through GSP-FMC. Turing never builds them (610's `gsp-abi-610.md` notes the same for 610). They
matter for a Blackwell identity ([blackwell-gb206.md](blackwell-gb206.md)), not for TU106.

For the device this means the following. The transport code written for 615 serves every later
driver whose tags hold. Only the payloads behind the transport (RPC structs, control params, NVKV
keys) stay version-bound. That is why the v2 layouts live in `msgq/v2.rs` and `gmcapi.rs` and not
in a version module.

## 2. The queues and their pointers

### 2.1 Memory

The layout of the shared buffer did not change. It is one non-contiguous sysmem allocation: the
page table (one `RmPhysAddr` per 4 KiB page), then the command queue (0x40000), then the status
queue (0x40000 by default, `RmGspStatusQueueSize` overrides it) (`message_queue_cpu.c:61-135,
300-326`). The CPU fills the page table and puts the bus address of page 0 into
`MESSAGE_QUEUE_INIT_ARGUMENTS.sharedMemPhysAddr` (`kernel_gsp.c:6157`). For the capture:
129 pages, command queue at 0x1000, status queue at 0x41000 (guest-driver-versions.md §5.2).

Each side writes the header of its own transmit queue at the start of that queue
(`msgq_priv.h:47-53`):

| Field | Offset | Value from `msgqTxCreate` (`msgq.c:126-140`) |
|---|---:|---|
| `versionMajor` | 0 | 2 (`MSGQ_VERSION_MAJOR`, `msgq_priv.h:32`) |
| `versionMinor` | 2 | 0 |
| `size` | 4 | the queue size (0x40000) |
| `msgSize` | 8 | `queueElementSizeMin` = 0x1000 (`message_queue_cpu.c:85,175-183`) |
| `msgCount` | 12 | (size - entryOff) / msgSize = 63 |
| `entryOff` | 16 | `sizeof(msgqTxHeader)` aligned to `1 << queueElementAlign` (12) = 0x1000 |
| `reserved[3]` | 20 | 0 |

In 610 the header was different: `version` 0, `size`, `msgSize`, `msgCount`, `writePtr`, `flags`
(`SWAP_RX`), `rxHdrOff`, `entryOff`, plus a 4-byte `msgqRxHeader` holding the read pointer
([gsp-abi-610.md](gsp-abi-610.md) (a)). In 615 there is no rx header. The receiver's
`msgqRxLink` checks the other side's header: `size` equal to its own idea of the queue size,
`msgSize` equal to `queueElementSizeMin`, `versionMajor` 2, and a consistent `msgCount`
(`msgq.c:240-262`). A zero header fails with -7 (`rx.size != size`, `msgq.c:246-249`). That is the
610 device's stop in guest-driver-versions.md §5.1.

### 2.2 Registers

`gpuGetGspMsgQueueRegisters_TU102` (`src/nvidia/src/kernel/gpu/arch/turing/kern_gpu_tu102.c:596-605`)
gives queue 0 (`RPC_TASK_RM_QUEUE_IDX`, `message_queue.h:31`) these four registers of the GSP
falcon block (`src/common/inc/swref/published/turing/tu102/dev_gsp.h:38-47`). The same HAL
serves TU10x through GB20x (blackwell-gb206.md §3.3).

| Register | BAR0 | The CPU's name for it (`msgq_priv.h:55-60, 102-107`) | Written by | Read by | Holds |
|---|---:|---|---|---|---|
| `NV_PGSP_QUEUE_HEAD(0)` | 0x110C00 | `regTxHead` | CPU | GSP | the command queue's write pointer |
| `NV_PGSP_QUEUE_TAIL(0)` | 0x110C04 | `regTxTail` | GSP | CPU | the command queue's read pointer |
| `NV_PGSP_MSGQ_HEAD(0)` | 0x110C80 | `regRxHead` | GSP | CPU | the status queue's write pointer |
| `NV_PGSP_MSGQ_TAIL(0)` | 0x110C84 | `regRxTail` | CPU | GSP | the status queue's read pointer |

Each register holds a monotonic u32 counter that is used modulo `msgCount`. The buffer is full
when `write - read == msgCount` and empty when the two are equal (`msgq_priv.h:61-63`). So all 63
slots can be in use, where 610 always kept one slot free. On the GSP falcon a write to
`QUEUE_HEAD` raises an interrupt, which is how the GSP learns about new messages
(`msgq_priv.h:75-77`). The protocol itself has no doorbell.

Both sides defend against a register that reads 0 after a reset, "only observed on Turing". A
pointer more than `msgCount` away from the cached one is treated as stale, and an empty queue at
the wrong moment is forced back to empty (`msgq.c:296-316, 411-423`).

### 2.3 Who writes what, when (Turing, `kgspBootstrap_TU102`)

The order, confirmed by the captures (guest-driver-versions.md §5.1, 5.3) and by the device's
runs (`caraxes/runs/transport-615-2026-09-24/boot/device-events.txt`):

1. **Construction.** `kgspConstructEngine` calls `_kgspInitRpcInfrastructure`, which calls `GspMsgQueuesInit`
   (`kernel_gsp.c:4786-4803, 3480-3497`, `message_queue_cpu.c:207-340`). `msgqTxCreate` writes
   the command-queue header, then `QUEUE_HEAD(0) = 0` (`msgq.c:175-177`). The GSP falcon is still
   halted. In 610 no register write happened here.
2. **Boot.** FWSEC-FRTS, reset into RISC-V, then `MAILBOX0/1 = libos init args PA`
   (`tu102/kernel_gsp_tu102.c:415-432, 261-272`). Booter-Load on SEC2 follows with the WPR meta
   PA (`:436-437`), and `NV_PFALCON_FALCON_OS` is written (`:446`). The CPU checks that the RISC-V
   core runs (`:450-459`). This part is as in 610, and the Turing boot model handles it.
3. **Status-queue link.** `GspStatusQueueInit` (`:469`, `message_queue_cpu.c:342-420`) polls
   `msgqRxLink` for 4 s. It needs the GSP's status-queue header in memory. When it succeeds it
   writes `MSGQ_TAIL(0) = 0` (`msgq.c:270-272`). No message and no doorbell come before this.
4. **Init.** `kgspSendInitRpcs` (`:473`, `kernel_gsp.c:5313-5361`) sends `GMCAPI_CMD_GSP_INIT`
   (§5). The send path writes the slots, fences and writes `QUEUE_HEAD(0) = 1`
   (`message_queue_cpu.c:551-607`, `msgq.c:352-377`). The CPU then polls `MSGQ_HEAD(0)`
   (`msgq.c:398-430`) in `kgspRecvPoll` (`kernel_gsp.c:3271-3475`) until the response is there.
   After reading it, it writes `MSGQ_TAIL(0)` (`msgq.c:459-485`). Before each send it reads
   `QUEUE_TAIL(0)` when its cached free count runs out (`msgq.c:283-350`).
5. **Unload.** `GMCAPI_CMD_GSP_SUSPEND` (fire and forget), then a wait until MAILBOX0 has the
   processor-suspended bit 31 that libos sets (`kernel_gsp.c:5869-5907`,
   `tu102/kernel_gsp_tu102.c:218-255`).

What the device does at each step (`boot.rs`, `impl BootModel` "The 615 transport"):

- It observes the CPU's `QUEUE_HEAD` and `MSGQ_TAIL` writes. The values still reach the falcon
  block. A doorbell before the link (step 1) is logged and not served.
- It links at Booter-Load (`BootEvent::WprMeta`, step 2), because step 3 comes without any
  doorbell. It reads the libos arguments from the GSP mailboxes (`FirmwareBoot::libos_args_pa`),
  finds RMARGS, reads the 615 `GSP_ARGUMENTS_CACHED` (§6), checks the CPU's command-queue header
  and writes the status-queue header of step 3.
- It answers reads of `QUEUE_TAIL` and `MSGQ_HEAD` from its queue state once linked.
- It serves the command queue on every `QUEUE_HEAD` write (the doorbell event of the Turing boot
  model, unchanged), with the written value as the CPU's write pointer. After an `MSGQ_TAIL`
  write it retries held replies.
- On `GSP_SUSPEND` it sets MAILBOX0 bit 31 (step 5).
- A `QUEUE_HEAD` write of 0 while its read pointer is not 0 means the command queue was
  created anew (a driver reload without a reset). The link is then dropped until GSP-RM starts
  again. A function-level reset (`BootModel::reset`) drops it too and keeps the transport.

## 3. The queue element

### 3.1 Layout

`GSP_MSG_QUEUE_ELEMENT` without confidential compute (`message_queue_priv.h:90-128`, Appendix A):

| Offset | Field | Sender's value (`GspMsgQueueSendCommand`, `message_queue_cpu.c:483-543`) |
|---:|---|---|
| 0 | `mctpMagic` | 0x4D435450 "MCTP" (`message_queue_priv.h:92`) |
| 4 | `mctpPayloadSize` | 24 + payload bytes (the header counted) |
| 8 | `mctpHeader` | `mctpCreateTransportHeader(1, 1, 0, 0, 0)` = 0xC0000001: version 1, SOM, EOM (`mctp_format.h:39-97`) |
| 12 | `nvdmHeader` | `mctpCreateNvdmHeader(type)` = 0x7E, vendor 0x10DE in 23:8, NVDM type in 31:24 (`mctp_format.h:50-121`): 0x2510DE7E for an RM RPC, 0x2610DE7E for GMCAPI (`nvdm_format.h:62-63`) |
| 16 | `nvdmPayloadSize` | the payload bytes |
| 20 | `reserved` | 0 |
| 24 | payload | an `rpc_message_header_v` plus payload, or a `GMCAPI_HEADER` plus data |

An element takes `ceil(mctpPayloadSize / 4096)` slots, and at most 16 (`queueElementSizeMax`,
`message_queue_cpu.c:85-86`). The largest RPC is `maxRpcSize = 0x10000 - 24 = 65512`
(`message_queue_priv.h:180-184`). In 610 it was 65520, because the header was 16 bytes. Large
RPCs are still split into `CONTINUATION_RECORD`s at that cap (`src/nvidia/src/kernel/vgpu/rpc.c`,
as in 610). With confidential compute a 16-byte authentication tag follows the MCTP words, and
everything after it is encrypted. `MESSAGE_QUEUE_INIT_FLAG_ENCRYPTION` announces it
(`gsp_init_args.h:35`). Turing has no CC, and the device refuses the flag.

### 3.2 What replaces the checksum and the sequence number

610's element carried `checkSum` (the XOR fold that makes the element sum to 0) and `seqNum`, and
`msgq.rs` checks both. 615 drops both. The receiving CPU does the following instead
(`message_queue_cpu.c:634-822`):

| 610 check (`msgq.rs` v0) | 615 replacement (CPU receive side) | Device, `msgq/v2.rs` |
|---|---|---|
| checksum folds to 0, else `NV_ERR_INVALID_DATA` after three re-reads | re-read up to three times when `mctpPayloadSize` is out of [24, 0x10000] (`:648-703`); `MCTP_HEADER_VERSION` must be 1 (`:707-717`); vendor id 0x10DE (`:719-726`); magic "MCTP" or "SEEN" (`:728-734`); each is `NV_ERR_INVALID_DATA` | the same four checks, plus `nvdmPayloadSize + 24 == mctpPayloadSize` (without CC the sender sets both from one size, `:501-543`); a failed element is consumed, counted and answered with `NV_ERR_INVALID_DATA` or `NV_ERR_INVALID_PARAM_STRUCT` when its RPC or GMCAPI header is intact, as in v0 (`Message::reply_to`) |
| `seqNum` equals the expected one (logged) | none on the element; a consumed element's magic is overwritten with "SEEN", and an element that already reads "SEEN" is stale and skipped, at most 16 times (`:739-777`) | marks each consumed command element "SEEN", and skips a "SEEN" element as stale without an answer; the RPC and GMCAPI requests keep one sequence counter (`kernel_gsp.c:532-539`, `gpu.c:7851-7852`), so a nonzero request sequence that is not the previous one plus one is logged as a gap |
| declared length past the published slots fails the queue | the CPU would read "Incomplete read" (`:661-670`) | the same wedge rule: fail the queue |
| write pointer out of range fails the queue | pointers more than msgCount apart are stale (`msgq.c:296-316`) | a head more than msgCount ahead of the tail fails the queue |

Ordering is a transaction. The sender copies the slots, issues a store fence, then writes the
pointer register (`message_queue_cpu.c:597-607`, `msgq.c:368-371`). The receiver reads the
register, issues a load fence, then reads the slots (`msgq.c:408-409`). On the device side a
register access is served after the previous one returns, so the CPU's `QUEUE_HEAD` write
happens after its slot stores. For the other direction the device fences after writing a status
slot and before it publishes `MSGQ_HEAD`.

## 4. GMCAPI

`GMCAPI_HEADER` (`gmcapi_base.h:27-45`; the size 40 is `ct_assert`ed in
`src/nvidia/src/kernel/core/gmcapi_impl.c:191`):

| Offset | Field | Request | Response |
|---:|---|---|---|
| 0 | `command` | the command id (23:0) | id with `GMCAPI_HEADER_COMMAND_FLAGS_RESPONSE` 0x01000000 (`:47-60`) |
| 4 | `size` | request bytes after the header | response bytes |
| 8 | `sequence` (u64) | `pRpc->sequence++`, the same counter as the RPC sequence (`gpu.c:7851-7852`) | the request's |
| 16 | `request.max_response_size` / `response.status` | the caller's output buffer size | an `NV_STATUS` |
| 20 | `reserved[5]` | 0 | 0 |
| 40 | `data` | | |

Command ids are `GMCAPI_ID(category, n)` = category << 16 | n (`gmcapi_table.h:74`). Ids up to
0xFFFF are reserved "to ensure no collisions with the 'function' constant of the VGPU style RPC"
(`gmcapi_base.h:76-80`), so RPC functions and GMCAPI ids share one namespace. The GSP management
category (`gmcapi_table.h:79-124`):

| Id | Command | Direction | Flags | Device |
|---|---|---|---|---|
| 0x10001 | `GSP_INIT` | CPU to GSP | synchronous, dynamic in and out | answered (§5) |
| 0x10002 | `EXEC_GENERIC_BOOTLOADER` | GSP to CPU | fire and forget | not sent |
| 0x10003 | `EXEC_HS_BINARY` | GSP to CPU | fire and forget | not sent |
| 0x10004 | `GSP_SUSPEND` | CPU to GSP | fire and forget | no response; MAILBOX0 bit 31 set (§2.3) |
| 0x10005 | `GSP_RESUME_DONE` | GSP to CPU | fire and forget | not sent (no suspend/resume) |

The vGPU category (0x2xxxx) belongs to the host vGPU manager and never comes from a guest.

The kernel sends a request with `gpuSendGmcRpc_KERNEL` (`gpu.c:7831-7894`). The request goes
into the RPC staging buffer as `GMCAPI_HEADER` plus data, and `kgspSendRpc(NVDM_TYPE_GMCAPI, 40 +
size)` sends it (`kernel_gsp.c:471-522`). A synchronous caller then polls for a GMCAPI element whose
command id and sequence match (`kernel_gsp.c:1989-2002`, with an assert on the response flag). It
copies `size` bytes (at most `max_response_size`) and returns `response.status`
(`gpu.c:7884-7893`, `gmcapi_impl.c:117-133`). An incoming GMCAPI element that does not match is
treated as a GSP-to-CPU event and dispatched to the kernel handlers (`kernel_gsp.c:2004-2022`).

On the device, `msgq::v2` surfaces a GMCAPI request as an ordinary `Message`. Its `function` is
the command id, its `sequence` the GMCAPI sequence, and its payload the data. The raw header is
kept in `Message::gmcapi`. The dispatcher in `boot.rs` then gains two match arms, whose ids a
610 guest can never send. A reply whose `function` is above 0xFFFF goes out as the GMCAPI
response (`QueueStateV2::send`). Everything else goes out as an RM RPC element.

## 5. `GMCAPI_CMD_GSP_INIT` and NVKV

### 5.1 NVKV

NVKV is a stream of u64 words (`src/nvidia/inc/libraries/nvkv/nvkv.h:36-118`). An instruction
word holds `key` in 15:0, `index` in 27:16, `opcode` in 31:28 and an immediate or count in 63:32.
The opcodes are IMM32 (one u32 in the word), SEQ32 (count u32 values for consecutive keys,
packed two to a word), SEQ64 (count u64 values for consecutive keys), and ARRAY8, ARRAY32 and
ARRAY64 (one key, count values padded to whole words). A string is an ARRAY8 that counts its NUL
(`nvkv.h:270-286`). The decoder `nvkvDecode` (`src/nvidia/src/libraries/nvkv/nvkv.c:27-207`)
calls a handler once per key. SEQ runs arrive as single U32 or U64 values with the key
incremented. It fails with `NV_ERR_OUT_OF_RANGE` for an instruction that runs past the buffer
and `NV_ERR_INVALID_COMMAND` for an unknown opcode. `gmcapi::decode` reproduces it word for
word, and `gmcapi::Encoder` has the kernel's encoders.

### 5.2 The request: system information and registry

`_kgspPrepareSystemInfo` builds it once, under the API lock, before the boot
(`kernel_gsp.c:5231-5262, 5704-5705`). It is `_kgspEncodeSystemInfo` (`:4977-5217`) with the
packed registry appended (`_kgspEncodePackedRegistryKeys`, `:4893-4958`). The key table is in
`src/nvidia/interface/gmcapi/gmcapi_system_info.h:26-216` and
`gmcapi_system_info_unstable.h:29`. In the order the kernel sends them:

| Keys (NVGMC_SI_) | Encoding (`kernel_gsp.c`) | What 610 sent it in | Device |
|---|---|---|---|
| `PCI_DEVICE_ID`, `PCI_SUB_DEVICE_ID`, `PCI_REVISION_ID` (1-3) | SEQ32 of 3 (`:4994-4997`) | `GspSystemInfo` | traced |
| `GPU_PHYS_ADDR`, `_FB_ADDR`, `_INST_ADDR`, `_IO_ADDR` (0x1010-0x1013) | SEQ64 of 4 (`:5003-5006`) | `GspSystemInfo.gpuPhys*` | the BAR addresses of `BUS_GET_PCI_BAR_INFO` (`ctrl.bar_offsets`), as from 610's GSP_SET_SYSTEM_INFO |
| `NV_DOMAIN_BUS_DEVICE_FUNC`, `HOST_ARCH`, `HOST_OS_TYPE` | SEQ64, IMM32, IMM32 (`:5007-5013`) | `GspSystemInfo` | traced (the BDF) |
| `PCI_CONFIG_MIRROR_BASE/SIZE`, `CHIPSET_L1SS_ENABLE`, `PCIE_POWER_CONTROL_VALUE`, `PF0_DEVICE_CONTROL2_REG`, `PCIE_LINK_CAP`, `PCIE_ATOMICS_OP_MASK`, `PCIE_ATOMICS_CPL_DEV_CAP_MASK` | with KernelBif (`:5015-5047`) | `GspSystemInfo` | the mirror is traced |
| `SIM_ACCESS_BUF_PHYS_ADDR`, `NOTIFY_OP_SHARED_SURF_PHYS_ADDR` | when present (`:5049-5059`) | `GspSystemInfo` | counted |
| `CONSOLE_MEM_SIZE`, `MAX_USER_VA` | SEQ64 of 2 (`:5061-5062`) | `GspSystemInfo` | `MAX_USER_VA` traced |
| `CL_FLAGS`, `FHB_BUSINFO`, `CHIPSET_BUSINFO`, their revision ids, `CHIPSET` | with OBJCL (`:5064-5087`) | `GspSystemInfo` | counted |
| `HOST_PAGE_SIZE` | IMM32 (`:5089`) | `GspSystemInfo.hostPageSize` | traced |
| `ACPI_*` (DOD, JT, MUX, CAPS) | when the ACPI data is valid (`:5091-5126`) | `GspSystemInfo.acpiMethodData` | counted |
| `HYPERVISOR_TYPE`, `VIRTUAL_CONFIG_BITS`, `VF_*` | (`:5128-5136`) | `GspSystemInfo` | counted |
| `SYS_TIMER_OFFSET_NS`, `GRID_BUILD_CSP` | (`:5138-5141`) | `GspSystemInfo` | counted |
| `PCIE_FLAGS`, `PLATFORM_FLAGS`, `FEATURE_FLAGS`, `VF_FLAGS` | SEQ64 of 4 bit fields (`:5143-5197`) | the booleans of `GspSystemInfo` | counted |
| `KERNEL_DRIVER_NAME` "NVRM", `KERNEL_DRIVER_VERSION_STRING`, `KERNEL_DRIVER_CHANGELIST` | strings, IMM32 (`:5199-5201`) | new (610 had only `SET_GUEST_SYSTEM_INFO`'s string) | the version is compared with the emulated one, and a mismatch is noted |
| `UNSTABLE_INTERFACE_SUPPORT` | IMM32 = 3 (RPC, events) (`:5203-5206`) | new | counted |
| `REGKEY_NAME` + `REGKEY_VALUE_U32` / `_STRING` / `_BINARY` per registry entry | string, then IMM32, string or ARRAY8 (binary above 4096 bytes skipped) (`:4906-4953`) | `SET_REGISTRY`'s `PACKED_REGISTRY_TABLE` | every entry traced (`registry RmRcWatchdog = U32(0)`) |

The captured first request (`caraxes/runs/guest-driver-2026-09-24/615.71.09/linked/`) has 296
words (2368 bytes), 142 keys and 44 registry entries. The unit test
`boot_615_link_and_gsp_init_from_the_captured_guest` replays it through the device.

### 5.3 The response: static configuration

`kgspSendInitRpcs` offers 48 KiB (`kernel_gsp.c:5321`). It decodes the response with
`_kgspDecodeGspStaticInfo` into a zeroed `GspStaticConfigInfo` (`:7864-7886`). Every key goes
through `_kgspStaticInfoKeyHandler` (`:7729-7862`). The key table is
`src/nvidia/interface/gmcapi/gmcapi_gsp_config.h:30-148`. The handler is strict. A key of its
table with the wrong value type, a nonzero index where none is allowed, an index at or above the
region count, or an array longer than the field fails the whole init with
`NV_ERR_INVALID_ARGUMENT`: the `continue` of `_NVKV_VALIDATE_*` (`nvkv.h:507-519`) leaves the
switch to `kernel_gsp.c:7859-7861`. Keys outside the table are ignored (`:7850-7853`). The device
sends these keys, in this order (`rpc::static_info_nvkv`). The values and provenance labels are
the ones of 610's `build_static_info`:

| Key (NVGMC_SC_) | Type (`kernel_gsp.c`) | 610 field | Value sent |
|---|---|---|---|
| `GPU_NAME_STRING` 0x2000, `GPU_SHORT_NAME_STRING` 0x2001 | string (`:7741-7742`) | `gpuNameString`, `gpuShortNameString` | profile name, chip short name |
| `GPU_GID_SHA1` 0x2002 | ARRAY8 of at most 256 (`:7743`) | `gidInfo.data` | the synthetic GID (16 bytes) |
| `INTERNAL_CLIENT/DEVICE/SUBDEVICE_HANDLE` 0xFF0-0xFF2 | U32 (`:7758-7760`) | `hInternal*` | 0xCA7A0001/2/3 |
| `VBIOS_SUB_VENDOR`..`VBIOS_OEM_REVISION` 0x50-0x53 | U32, each sets `bVbiosValid` (`:7786-7789`) | `vbios*`, `bVbiosValid` | subsystem ids, VBIOS revision |
| `FB_REGION_COUNT` 0x10 | U32, at most 16 (`:7762`) | `numFBRegions` | 2 |
| `FB_REGION_BASE/LIMIT/RESERVED` 0x1011-0x1013, index i | U64, i below the count (`:7764-7766`) | `fbRegion[i]` | as 610 |
| `FB_REGION_PERFORMANCE` 0x11, index i | U32 (`:7767`) | `.performance` | 1 |
| `FB_REGION_FLAGS` 0x12, index i | U32: bit 0 compression, bit 1 ISO, bit 2 protected (`:7830-7835`) | `.supportCompressed`, `.supportISO` | ISO only (no compression, fb-model.md) |
| `FB_LENGTH` 0x1010 | U64 (`:7773`) | `fb_length` | the profile size |
| `BAR1_PDE_BASE`, `BAR2_PDE_BASE` 0x1020-0x1021 | U64 (`:7778-7779`) | `bar1PdeBase`, `bar2PdeBase` | as 610 |
| `BRANDING`, `MISC_FEATURES`, `POWER_FEATURES` 0x1000-0x1002 | U64 bit fields (`:7794-7828`) | the booleans | 0; `MISC_FEATURES` bit 5 carries `bBusResetRequired` |
| `ENGINE_MASK` 0x1100, index = first NVGMC engine type | U64 array (`:7837-7843`) | `engineCaps` | GMC masks of the chip's engines (below) |

Keys the device does not send, so the kernel's zero stays: `SKU_*`, `CHIP_SUB_REV`,
`EMULATION_REV1`, `CHIP_SKU*`, `PROJECT*`, `CDP`, `PDI`, `GC6_*`, `SRIOV_*`, `ECID_INFO`,
`VMMU_SEGMENT_SIZE`, `WPR_LAYOUT_*` and `FB_REGION_TAG`. All of them were zero in 610's reply as
well. `VGX_VERSION` (0x210) calls `rpcSetIpVersion` (`:7845-7848`). Without it the kernel keeps
the RPC version it was built with (`src/nvidia/src/kernel/rmapi/rpc_common.c:100-104`), which is
the right one for a device that serves exactly that version.

**Engines.** `ENGINE_MASK` carries one u64 per `NVGMC_ENGINE_TYPE` from the key index on. Bit n
is engine index n (`gmcapi_gsp_config.h:121-124`, `gmcapi_engine_types.h:33-62`).
`gpuGetRmEngineTypeCapsFromGMCEngineIdMasks` (`src/nvidia/src/kernel/gpu/gpu_engine_type.c:255-277`)
maps each (type, index) through the one-to-one table `_NV_ENGINE_MAPPINGS` (`:36-120`) to an
`RM_ENGINE_TYPE` bit. TU106 sends GR 0x1, COPY 0x1FF, NVDEC 0x3, NVENC 0x1, SW 0x1, SEC2 0x1 and
HOST 0x1, from type GR (1) on (`gmcapi::engine_masks`). One finding for the record: the decoder
sets `RM_ENGINE_TYPE` bits in `engineCaps`, while `gpuCheckEngine` still reads that array as
`NV2080_ENGINE_TYPE` caps and converts them (`gpu.c:6730-6748`). The two numberings agree for GR
and COPY0-9 but not for NVDEC, NVENC, SW, SEC2 and HOST (for example NVDEC0 is RM 0x1D and
NV2080 0x13). A real GSP-RM sends the same GMC masks, so the device reproduces the driver's own
behaviour and does not correct it.

The device's response for the TU106 profile is 424 bytes (the runs' trace line
`gmcapi GSP_INIT: 424 bytes of NVKV static info`).

### 5.4 The four 610 RPCs

| 610 | 615 |
|---|---|
| `GSP_SET_SYSTEM_INFO` (72) with `GspSystemInfo` (952 bytes) | the NVKV system info of `GSP_INIT`; the RPC is marked deprecated (`src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:82`) |
| `SET_REGISTRY` (73) with `PACKED_REGISTRY_TABLE` | the `REGKEY_*` keys of `GSP_INIT`; deprecated (`:83`) |
| `GET_GSP_STATIC_INFO` (65), reply `GspStaticConfigInfo` (1600 bytes) | the NVKV response of `GSP_INIT`; `NV_RM_RPC_GET_GSP_STATIC_INFO` has no caller; the kernel's own `GspStaticConfigInfo` is now 1608 bytes but internal |
| the `GSP_INIT_DONE` event (0x1001), awaited before anything else | not awaited: the `GSP_INIT` response is the completion (`kernel_gsp.c:5338-5344`) |
| `SET_GUEST_SYSTEM_INFO` (1) | unchanged, and not sent by the 615 boot (none in the runs) |

## 6. Boot arguments

`GSP_ARGUMENTS_CACHED`, the RMARGS libos region (`kgspSetupLibosInitArgs`, `kernel_gsp.c:7040-7067`;
filled by `kgspPopulateGspRmInitArgs`, `:6139-6230`; layout `gsp_init_args.h:57-113`):

| Offset 615 | Offset 610 | Field | 615 notes |
|---:|---:|---|---|
| 0 | | `magic` "GSP " (0x20505347) | new (`:62`) |
| 4 | | `size` (u16) = 184 | new (`:6154`) |
| 8 | | `flags` (u64): `IN_PM_TRANSITION` 1, `STACK_IN_DMEM` 2, `RECOVERY_MARGIN_PRESENT` 4, `SCAN_RECOVERY_MARGIN` 8 | new; replaces 610's `bDmemStack` and the SR `bInPMTransition` (`:67-71, 6184, 6192`); the capture has 0x2 |
| 16 | 0 | `messageQueueInitArguments` (64) | below |
| 80 | 64 | `srInitArguments` (16; 610: 12) | `oldLevel`, `flags`, `reserved[8]` |
| 96 | | `reserved2[4]`, `gpuInstance` | |
| 104 | | `profilerArgs` {pa, size} | |
| 120 | | `sysmemHeapArgs` | |
| 136 | | `rmStateMonitorBufferArgs` | |
| 152 | | `bindataArgs` {radix3, size} | |
| 168 | | `vbiosOverrideArgs` {pa, size} | new |

In 610 the whole struct was 152 bytes. The members after the queue arguments moved by 16 to 32
bytes.

`MESSAGE_QUEUE_INIT_ARGUMENTS` (`gsp_init_args.h:33-48`), offsets inside it and from the start of
RMARGS:

| 615 (in RMARGS) | 610 | Field | Capture |
|---:|---:|---|---|
| 0 (16) | | `flags` u64 (`ENCRYPTION` 1) | 0 |
| 8 (24) | 0 | `sharedMemPhysAddr` | 0x88a00000 |
| 16 (32) | 16 | `cmdQueueOffset` u64 | 0x1000 |
| 24 (40) | 24 | `statQueueOffset` u64 | 0x41000 |
| 32 (48) | 32 (u64) | `queueElementHdrSize` u32 | 0x18 |
| 36 (52) | 40 (u64) | `queueElementSizeMin` u32 | 0x1000 |
| 40 (56) | 48 (u64) | `queueElementSizeMax` u32 | 0x10000 |
| 44 (60) | | `queueHeaderAlign` u32 | 4 |
| 48 (64) | 56 | `queueElementAlign` u32 | 12 |
| 52 (68) | 8 | `pageTableEntryCount` u32 | 129 |
| 56 (72) | | `reserved[8]` | |

The 610 offsets read a 615 region as "2 pages, status at 0x88a00000": the flags word as the page
count, and the PA as the status offset (guest-driver-versions.md §5.2). `msgq::v2::QueueArgs`
reads the 615 layout and checks the magic, the size, no encryption and the element geometry. The
`MQ_*` constants of `ver::v615` state the same offsets for generic code.

The other boot structures: `LibosMemoryRegionInitArgument` (32 bytes) and the RMARGS/LOGINIT/LOGRM
ids are unchanged. `GspFwWprMeta` is 256 bytes, revision 1, magic unchanged. Its `padding[2]` is
now `pagingConfig` (`src/nvidia/arch/nvalloc/common/inc/gsp/gsp_fw_wpr_meta.h:196`), and every
offset the device reads (`WPR_META_*`) is unchanged; the `ver::v615` test checks them against
Appendix A. `GSP_FMC_BOOT_PARAMS` (256; 88 in 610) and `GSP_FMC_INIT_PARAMS` (8; 4) are the
GSP-FMC boot of Hopper and later (`gspifpub.h:50-149`). Their new header is magic "FMC " and a
size (`:127-149`), and every member moved (blackwell-gb206.md §3.2). Turing does not use them.

## 7. What stays as in 610, and the seven changed control structs

Unchanged, and checked by `ver::v615::tests::gsp_offsets_match_the_measured_615_abi` against
Appendix A: `rpc_message_header_v` (32 bytes, header version 0x03000000, signature "VRPC"),
`rpc_gsp_rm_control_v03_00` (40), `rpc_gsp_rm_alloc_v03_00` (32), `rpc_free_v03_00` (16),
`rpc_dup_object_v03_00` (28), `rpc_set_guest_system_info_v03_00` (792), the event structs, and
every RPC function and event number the device uses. 615 only adds the functions 230 and 231 and
the events 0x102A-0x102D (blackwell-gb206.md §3.2).

The seven control parameter structs (blackwell-gb206.md §3.4, sizes from `nvrm_sys::v610` and
`nvrm_sys::v615`):

| Control | 610 | 615 | Guest-facing or host-facing in the device | Device with a 615 guest |
|---|---:|---:|---|---|
| `FIFO_GET_DEVICE_INFO_TABLE` 0x20801112 | 3212 | 3340 | guest (answered from chip data) | `DEVINFO_*` from `nvrm_sys::v615`: entry 104 bytes, `engineData[17]` (`ENGINE_INFO_TYPE_IS_PREEMPTABLE` appended, `src/nvidia/inc/kernel/gpu/fifo/engine_info.h:98-103`, 0 on Turing) |
| `GPU_GET_PES_INFO` 0x20800168 | 56 | 76 | host (the device queries the host for the floorsweeping masks) | the running host's layout (`hostabi.rs`) |
| `GR_GET_GLOBAL_SM_ORDER` 0x2080121B | 9240 | 10264 | host (query) | the running host's layout (`hostabi.rs`) |
| `INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER` 0x20800A22 | 73760 | 81952 | guest | `SM_ORDER_*` from v615: `virtualDpcId` inserted after `virtualGpcId` (ten u16 per SM instead of nine); `kgr::global_sm_order` translates the host's entries field by name (`hostabi::translate_sm_order`); on a 610 host it stays 0 (no DPC before Blackwell) |
| `INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS` 0x20800A26 | 10688 | 10848 | guest | `FS_*` from v615 |
| `INTERNAL_GET_PCIE_P2P_CAPS` 0x20800AB8 | 3 | 4 | guest | `bSkipChipsetCheck` appended; the two status bytes stay at 1 and 2 |
| `INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO` 0x20800A87 | 1088 | 1072 | guest | answered `NV_ERR_NOT_SUPPORTED` as in 610 (the size does not matter) |

`NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS` (the TSG allocation) grew from 20 to 28 bytes at its
end. The two fields the device reads stay at their offsets, and the test checks that.

**The reused id 0x20800A36.** In 610 it is `INTERNAL_GPU_GET_CHIP_INFO`. In 615 that control no
longer exists, and 0x20800A36 is `INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO`
(`sdk/ctrl/ctrl2080/ctrl2080internal.h:572-585`). The chip sub-revision travels as
`NVGMC_SC_CHIP_SUB_REV` instead. Each version module names what the id means (`CTRL_0A36`,
`ver::Id0A36`) and `ctrl.rs` `id_0a36` dispatches by it: chip info for a 610 guest, an NV_OK echo
for the 615 fabric-probe sync (no NVLink fabric on TU106), so a fabric-probe call is never
answered as chip info. In the runs the 615 guest never sent 0x20800A36.

**Guest ABI and host ABI.** `ver::v615` is the 610 module with the 615 guest-facing layouts
overridden. The layouts of controls the device sends to the host are not in `src/ver`: they
follow the running host driver (`hostabi.rs`: PES info, the public SM order, the TSG
allocation, the host-sized GSS legacy controls); `FLUSH_GPU_CACHE_*` and the GR routed queries
are the same on both. The test
`versioned_layouts_are_overridden_where_they_differ` compares every per-version binding the 610
module computes from with its 615 twin. It fails when a regenerated binding moves a layout that
is not overridden. Controls the device forwards opaquely are the exception: the `nvidia-smi`
`LEGACY` list, and anything forwarded as-is. A 615 guest and a 610 host disagree on some of their
sizes (§10).

## 8. How the device tells which version the guest runs

The GSP firmware the guest loads (`gsp_tu10x.bin` of its own version) lies in guest memory behind
the WPR meta's radix3 table. It is not something the device executes or checks. Parsing it for
`.fwversion` is possible but costly, and it would only confirm what the candidates below already
show earlier:

| Candidate | When it is known | Decisive? |
|---|---|---|
| RMARGS magic "GSP " at offset 0 of `GSP_ARGUMENTS_CACHED` | at Booter-Load, before any message (the libos arguments are in the GSP mailboxes) | yes. In 610 the first word is the low half of the page-aligned `sharedMemPhysAddr` and can never equal 0x20505347 (`msgq::v2::rmargs_is_v2`) |
| the command-queue header's `versionMajor` (2 in 615, `version` 0 in 610) | from construction on, readable at Booter-Load | yes; `QueueStateV2::link` checks it, and a v0 header is refused |
| the first register writes | 615 writes `QUEUE_HEAD(0) = 0` at construction, before the mailbox is programmed; 610 writes nothing there before its first RPC | no; a hint only, and indistinguishable from a doorbell by value |
| `GSP_ARGUMENTS_CACHED.size` = 184 | at Booter-Load | yes, together with the magic |
| the driver version string (`KERNEL_DRIVER_VERSION_STRING`, 615; `SET_GUEST_SYSTEM_INFO`, 610) | only in the first message, after the transport already had to be right | no, too late; the device notes a mismatch |
| a launcher switch `CARAXES_GUEST_ABI=610\|615` | before the device starts | yes, the fail-closed default |

What is implemented is the launcher switch with checks behind it. The build fixes the version:
a device binary emulates one driver version, as 610 already required
([gsp-abi-610.md](gsp-abi-610.md) (e)). The 615 control layouts reach `ctrl.rs`, `kgr.rs` and
`channel.rs` through `ver::active` at compile time. The switch therefore selects a binary and
checks it, as follows:

- `tools/caraxes-vm.sh` with `CARAXES_GUEST_ABI=615` runs `target/v615/release/vfio-user-nvgpu`
  (`cargo build --release -p vfio-user-nvgpu --features v615 --target-dir target/v615`) and passes
  `--guest-abi 615`.
- `ver::check_guest_abi` (called from `main.rs`) refuses to start when `--guest-abi` or
  `CARAXES_GUEST_ABI` names another ABI than the binary's. 595 passes only in a 610 build,
  together with the environment switch of guest-driver-versions.md §2.
- A 610 device facing a 615 guest links nothing. At Booter-Load it logs `guest ABI: ERROR the
  guest's RMARGS … carry the 615 magic "GSP "`, and the guest stops in its status-queue link.
  This is tested in `boot::tests::a_610_device_names_a_615_guest`.
- A 615 device facing a 610 guest refuses the RMARGS without the magic and writes no status
  header, so the 610 guest's own status-queue link times out as well.

A runtime selection in one binary would need both control-layout sets behind a runtime switch
in `ctrl.rs`, `kgr.rs`, `nvml.rs`, `uvm.rs` and `channel.rs`. That is outside this package, and
the magic check shows where such a switch would decide.

## 9. 610 against 615 for every structure the device touches

Sizes of the GSP structs from gsp-abi-610.md (a) and Appendix A; control structs from
`nvrm_sys::v610` and `nvrm_sys::v615`.

| Structure | 610.57.04 | 615.71.09 | Device code for 615 |
|---|---:|---:|---|
| `msgqTxHeader` | 32 (v0: writePtr, flags, rxHdrOff) | 32 (v2: no pointers) | `msgq::v2::TxHeaderV2` |
| `msgqRxHeader` | 4 | gone (register) | QUEUE_TAIL / MSGQ_HEAD answered from `QueueStateV2` |
| queue pointers | in memory, modulo msgCount, one slot free | 4 registers, monotonic, full ring usable | `boot.rs` register hooks |
| `GSP_MSG_QUEUE_ELEMENT` header | 16 | 24 | `msgq::v2` constants |
| element checksum / seqNum | yes / yes | no / no | §3.2 checks |
| `GSP_MSG_QUEUE_ENCRYPTION_TAG` | 40 | 16 (not stable) | refused (no CC) |
| `maxRpcSize` | 65520 | 65512 | `ver::v615::MAX_RPC_SIZE` |
| `GMCAPI_HEADER` | none | 40 | `gmcapi::Header` |
| NVKV system info / static info | none | words | `gmcapi::SystemInfo`, `rpc::static_info_nvkv` |
| `rpc_message_header_v` | 32 | 32 | shared |
| `rpc_gsp_rm_control_v03_00` / `_alloc_` / `rpc_free_v03_00` / `rpc_dup_object_v03_00` | 40 / 32 / 16 / 28 | same | shared |
| `rpc_set_guest_system_info_v03_00` | 792 | 792 | shared |
| `GspSystemInfo` | 952 (on the wire) | 952 (internal) | NVKV instead |
| `GspStaticConfigInfo` | 1600 (on the wire) | 1608 (internal) | built as the 610 image, sent as NVKV |
| `PACKED_REGISTRY_TABLE` / `_ENTRY` | 8 / 16 | 8 / 16 (internal) | REGKEY keys instead |
| `GSP_ARGUMENTS_CACHED` | 152 | 184 | `msgq::v2::QueueArgs` |
| `MESSAGE_QUEUE_INIT_ARGUMENTS` | 64 (at RMARGS+0) | 64 (at RMARGS+16, reordered) | `msgq::v2::ARGS_MQ_*`, `ver::v615::MQ_*` |
| `GSP_SR_INIT_ARGUMENTS` | 12 | 16 | not read |
| `LibosMemoryRegionInitArgument` | 32 | 32 | shared |
| `GspFwWprMeta` | 256 | 256 (`pagingConfig`) | shared offsets |
| `GSP_FMC_INIT_PARAMS` / `GSP_FMC_BOOT_PARAMS` | 4 / 88 | 8 / 256 | not on Turing |
| `NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS` | 3212 | 3340 | `ver::v615` |
| `NV2080_CTRL_GPU_GET_PES_INFO_PARAMS` | 56 | 76 | host query, the running host's (`hostabi.rs`) |
| `NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS` | 9240 | 10264 | host query, the running host's (`hostabi.rs`) |
| `NV2080_CTRL_INTERNAL_STATIC_GR_GET_GLOBAL_SM_ORDER_PARAMS` | 73760 | 81952 | `ver::v615` `SM_ORDER_GUEST`, `kgr::global_sm_order` |
| `NV2080_CTRL_INTERNAL_STATIC_GR_GET_FLOORSWEEPING_MASKS_PARAMS` | 10688 | 10848 | `ver::v615` |
| `NV2080_CTRL_INTERNAL_GET_PCIE_P2P_CAPS_PARAMS` | 3 | 4 | `ver::v615` |
| `NV2080_CTRL_INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO_PARAMS` | 1088 | 1072 | not supported, as 610 |
| `NV2080_CTRL_INTERNAL_GPU_GET_CHIP_INFO_PARAMS` (0x20800A36) | 88 | removed (id reused for the 16-byte `SYNC_FABRIC_PROBE_INFO`) | `ctrl.rs` `id_0a36` by `CTRL_0A36` |
| `NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS` | 20 | 28 | offsets unchanged |
| `NV_CHANNEL_ALLOC_PARAMS` | `nvrm_sys::V610` | `nvrm_sys::V615` | `ver::v615::RmAbi` |

## 10. Implementation, tests, and what is open

Files (`crates/vfio-user-nvgpu`):

| File | What |
|---|---|
| `src/msgq/v2.rs` | message queue v2: `TxHeaderV2`, `QueueArgs` (RMARGS), `QueueStateV2` (receive with the §3.2 checks, send with GMCAPI or RPC framing, held replies), behind `QueueState` (`QueueState::from_v2`; every method delegates) |
| `src/gmcapi.rs` | `GMCAPI_HEADER`, NVKV decode and encode, the SI and SC key tables, `SystemInfo`, engine masks |
| `src/boot.rs` | additive: the transport field, the register hooks, the link at Booter-Load, the `GSP_INIT` and `GSP_SUSPEND` arms, `gsp_init` |
| `src/rpc.rs` | `static_info_nvkv`; GMCAPI names in `function_name` and `describe` |
| `src/ver/mod.rs`, `src/ver/v615.rs`, `src/ver/v610.rs` | `GspTransport`, `check_guest_abi`, the `v615` module; v610 gained `GSP_TRANSPORT` (and later `SM_ORDER_GUEST`, `CTRL_0A36`, `FS_TPC_TO_PES_MAP_LEN`) |
| `src/chip/mod.rs`, `src/chip/turing.rs`, `chips/tu106/boot.toml` | `FirmwareBoot::libos_args_pa` and `queue_registers`; the three pointer registers |
| `src/kgr.rs` | the SM order translated field by name for any guest and host pair (docs/vfio/cutover-615.md (d)) |
| `src/main.rs`, `tools/caraxes-vm.sh`, `Cargo.toml` | `--guest-abi`, `CARAXES_GUEST_ABI=615`, feature `v615 = ["v610", "nvrm-sys/v615"]` |

Tests (the default build runs all of them; the 615 transport code does not depend on the
feature):

- `msgq::v2::tests`: the status header `msgqRxLink` accepts, and a v0 header refused;
  an RM RPC round trip and a GMCAPI round trip through both queues; monotonic pointers across the
  ring end and a full status ring; the framing checks that replace the checksum; delegation
  through `QueueState`.
- `gmcapi::tests`: the instruction layout, the request as the kernel encodes it (SEQ32 of three
  in two words, strings, the registry), `nvkvDecode`'s refusals, array padding, the GMCAPI
  header, engine masks.
- `rpc::tests::static_info_as_nvkv_keys`: the response passes a checker that applies
  `_kgspStaticInfoKeyHandler`'s rules to every key of the table, and carries the 610 values.
- `boot::tests::boot_615_link_and_gsp_init_from_the_captured_guest`: guest memory from the
  capture (WPR meta, libos arguments, RMARGS, page table, command-queue header). The pre-boot
  doorbell is ignored, the link happens at Booter-Load, the captured GSP_INIT slot is answered,
  QUEUE_TAIL and MSGQ_HEAD read 1, and the response element is checked as the CPU would check
  it. Reload and reset are covered too.
- `boot::tests::a_610_device_names_a_615_guest`, `ver::abi_tests`.
- With `--features v615`: `ver::v615::tests` (Appendix A, the versioned layouts, the reused
  id).

Result of the first 615 guest boot: see [experiments.md](../history/caraxes-experiments-2026-09-29.md), "615 transport".

Open items:

1. `nvidia-smi` shows `ERR!` for power. The device forwards some GSS legacy controls to the
   host without looking inside them (the `LEGACY` list of the version module). A 615 guest sends
   them with 615 sizes, and the 610.57.04 host refuses them with 0x1f:
   - `pmgr 0x18` (0x2080A618): 20512 bytes from the guest, 20000 on the host
   - `thermal 0x36` (0x20808536): 24532 bytes from the guest, 25552 on the host

   Two controls that are new with the 615 guest go unanswered (NOT_SUPPORTED):
   - `NV2080_CTRL_CMD_FB_GET_REMAPPED_BANKS` 0x20801361 (new in 615,
     `sdk/ctrl/ctrl2080/ctrl2080fb.h:2993`)
   - perf 0x2080A0F1 (not in the open headers, like its neighbour 0x2080A0F2 of the `LEGACY`
     list)

   All four are host-facing. Done (cutover-615.md (d)): the two sizes are per host driver in
   `hostabi.rs` `HostLayouts::legacy`; neither layout is in any open header, so a mismatched
   size is answered NOT_SUPPORTED instead of translated (power N/A while guest and host
   driver differ). perf 0xf1 is forwarded on a 615 host and NOT_SUPPORTED on a 610 host.
   FB_GET_REMAPPED_BANKS is not new in 615 (610 `ctrl2080fb.h:2990` has it with the same
   8208-byte layout) and is in the `FORWARD` table for both hosts.
2. `ALLOC_MEMORY` (RPC 4) from the new `goeimport` path is answered `NV_ERR_NOT_SUPPORTED`. The
   kernel logs `_goeimportSetupTransport: goeimport: step 3a failed (0x56)` and continues.
3. Suspend and resume (`GMCAPI_CMD_GSP_RESUME_DONE`, the `IN_PM_TRANSITION` flag) are not
   emulated.
4. A runtime choice between the two ABIs in one binary (§8).
5. The phase-5 inventory of the 615 guest beyond init (the CUDA and display gates) is not taken
   yet.

## 11. The RPC path: guest-cached answers and the head ioeventfd

Two mechanisms of [gsp-transport-classes.md](gsp-transport-classes.md) §6 and §7 (T1 and T5 of
[optimization-backlog.md](optimization-backlog.md) "By transport class"). Both work for the
610 and the 615 guest. Records: `caraxes/runs/rpc-path-2026-09-25/`.

### 11.1 CACHEABLE in the reply of a static GSS-legacy control

- The kernel caches a GSS-legacy answer when the reply's `rmctrlFlags` are cacheable, not
  `_BY_INPUT`, the access right is 0 and both statuses are NV_OK
  (`src/nvidia/src/kernel/vgpu/rpc.c:10894-10905`; 610.57.04 `:10898-10909`, same logic).
  Later calls read the cache before any RPC (`rpc.c:10762-10775`).
- The cache key is (GPU, command) only (`rmapi_cache.c:481-535`, get `:407-472`; the file is
  identical in 610 and 615). A later call gets the first answer whatever its request. So only
  commands whose request and answer never change qualify.
- The get path checks the stored flags with `rmControlValidateClientPrivilegeAccess`
  (`src/nvidia/src/kernel/rmapi/control.c:678-716`). Flags without `NON_PRIVILEGED` would
  fail every user process. The device sends `CACHEABLE | NON_PRIVILEGED` = 0x408
  (`control.h:205, :257`).
- The set: every GSS-legacy forward now carries FNV-1a fingerprints of request and reply in
  its trace note. Over 20 `nvidia-smi -q` (before and after a CUDA gate), a 20-sample dmon and
  the NVML gate (`cache-before-610/legacy-forwards.txt`), ten commands had one request, one
  answer and NV_OK, from nvidia-smi, NVML and libcuda alike, and a table-sized reply:
  gpu 0x59 (cuInit), thermal 0x2a, 0x2c, 0x2e, 0x42, clk 0x19, perf 0x79, 0xa4, 0xa7,
  pmgr 0x12 (`nvml.rs` `LEGACY_CACHEABLE`).
- Not cached: answers that changed (clk 0x04, 0x64, thermal 0x2f, 0x46, perf 0x26, 0x81,
  0x97, 0xa8), small values that only held still (1 to 52 bytes), failing commands, and
  every non-legacy control (the kernel reads the reply flags only for GSS-legacy ids).
- Opt-in: `CARAXES_LEGACY_CACHEABLE=1` sends the flag; unset (the default since the merge into
  dev, 2026-09-28) sends 0 as before. The runs of 2026-09-25 had it on by default.

### 11.2 `NV_PGSP_QUEUE_HEAD(0)` as a KVM ioeventfd (`CARAXES_RPC_IOEVENTFD=1`)

- The register is one more region-io-fds sub-region without datamatch, the shape of the MSI
  re-arm register (intr-page.md §4). The token pool's reactor reads its eventfd
  (`Device::rpc_ioeventfd`). The device also takes the count before every trapped access of
  page 0x110000, so the element is served before the guest's next poll, whatever the reactor's
  latency. KVM signals the eventfd inside the write's exit, before the guest runs on.
- The value is lost; the count is exact. The CPU writes the register once per element
  (`msgqTxSubmitBuffers`, `msgq.c:354-377`, once per `GspMsgQueueSendCommand` for all its
  slots, `message_queue_cpu.c:551-607`). The only other write is `msgqTxCreate`'s 0
  (`msgq.c:176`), at construction and on a reload.
- Element check per counted write, at the device's own read pointer
  (`QueueStateV2::peek_published`), without writing into the ring:
  - magic "MCTP", and a nonzero sequence after the last one taken (RPCs, continuation records
    and GMCAPI requests share one counter, `kernel_gsp.c:532-539`, `rpc.c:2125-2180`,
    `gpu.c:7851`; fire-and-forget requests carry 0): an element of
    `ceil(mctpPayloadSize / 4096)` slots, clamped to 1-16. The trapped path then serves it with
    head = read pointer + slots and runs every §3.2 check. Bad framing fails the queue as before.
  - "SEEN" (the device's own mark on every element it took, as for a trapped write), 0, or an
    old sequence: no element. That is `msgqTxCreate`'s 0. When linked, the link drops as for a
    trapped 0 (§2.3).
  - Writes still counted at the link come from before it, and the link discards them. The CPU
    publishes nothing before it sees the status header (`kernel_gsp_tu102.c:463-474`).
- Ordering: the CPU copies the slots, fences (`message_queue_cpu.c:597-605`, `msgq.c:370`),
  then writes the register. The eventfd is signalled from that write's exit. The device reads
  an element only for a write it has counted, and never past the counted elements. So an element
  still being copied (the one after a fire-and-forget request) is not read before its own
  write, and a partially written element cannot be misread.
- 610: the write pointer is in shared memory, and the CPU writes 0 to the register as a
  doorbell (610 `kernel_gsp.c:424`). Each counted write is that doorbell.
- Fail closed: off by default. The switch needs the token pool (its reactor) and the boot
  model. A client that does not register the sub-region keeps trapping. The trapped path is
  unchanged.
- Tests: `msgq::v2::tests::peek_published_decides_without_the_pointer`,
  `boot::tests::head_writes_without_their_value_are_served_from_the_ring`,
  `device::tests::the_msi_rearm_ioeventfd_is_read_by_the_token_reactor` (the head eventfd
  routed to `rpc_ioeventfd`).
- Measure: `caraxes/tools/gsp-transport.py --rpc-traps` takes the device's
  `rpc ioeventfd: head write` event as the start of an RPC and counts traps per RPC.

### 11.3 Ordered host controls outside Device (`CARAXES_HOST_CONTROL_OFFLOCK=1`)

Experimental, off by default. Requires `--host-exec`, `--host-rm`, `--boot-model`,
the 615 host ABI and a MsgqV2 guest. Keep `CARAXES_RPC_IOEVENTFD=0` and
`CARAXES_LEGACY_CACHEABLE=0`. With `CARAXES_ACK_UNANSWERED=1`, include both
`2080a612` and `2080a613` in `CARAXES_WIN_EXCEPT`; selected recorded or diagnostic
overrides are rejected.

- Only complete RM_CONTROL requests reaching the existing legacy host-forwarding
  route qualify: A612 with 54,352 parameter bytes, A613 with 79,936 bytes.
- One worker owns the existing host RM client. Other calls use its synchronous
  proxy, preserving host call order and the lifetime of nested parameter storage.
- The selected call runs outside the Device mutex. An independent completion pump
  acquires Device to publish the real status and bytes, then deliver the usual
  interrupt. The worker never accesses guest RAM or waits for reply consumption.
- A613 retains its ordinary two-record reply. Both records must fit the bounded
  held queue before admission. Capacity pressure retains the prepared reply;
  it does not repeat the host call. Publication failure terminates the session.
- Reset, queue replacement, relevant DMA mapping changes and disconnect drain
  accepted calls before retiring the queue generation. Shutdown closes admission
  before releasing host memory. Results from retired generations are discarded.
- Profiling separates queue wait, host execution, result-to-Device delay and reply
  serialization. The feature releases Device while waiting; it does not shorten
  the host ioctl, bypass its locking, cache answers or establish an FPS gain.

Implementation: `host_control.rs`, `BootModel::complete_host_control` and
`Device::complete_host_controls`. CPU tests exercise the real VFIO server and
queue serializer with a controllable host stub; GPU qualification is separate.

## Appendix A: gsp-abi-sizes output for 615.71.09 (generated)

`bash caraxes/tools/gsp-abi-sizes.sh --abi 615 <615 tree>`, minimal flag set. The
`ver::v615` tests parse this appendix (`ver::measured`).

<!-- compiler: cc (GCC) 16.2.1 20260810; flags: minimal; tree: ogkm-615.71.09 (commit 61dcc93) -->
<!-- generated by caraxes/tools/gsp-abi-sizes.sh; driver 615.71.09 -->

### Message queue transport (msgq library, one header per queue)

#### `msgqTxHeader`: sizeof 32 (0x20), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 2 | `versionMajor` |
| 2 | 0x0002 | 2 | `versionMinor` |
| 4 | 0x0004 | 4 | `size` |
| 8 | 0x0008 | 4 | `msgSize` |
| 12 | 0x000c | 4 | `msgCount` |
| 16 | 0x0010 | 4 | `entryOff` |
| 20 | 0x0014 | 12 | `reserved` |

### Queue element and RPC header

#### `GSP_MSG_QUEUE_ELEMENT`: sizeof 40 (0x28), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `mctpMagic` |
| 4 | 0x0004 | 4 | `mctpPayloadSize` |
| 8 | 0x0008 | 4 | `mctpHeader` |
| 12 | 0x000c | 4 | `nvdmHeader` |
| 16 | 0x0010 | 4 | `noEncryption.nvdmPayloadSize` |
| 20 | 0x0014 | 4 | `noEncryption.reserved` |
| 24 | 0x0018 | flex | `noEncryption.payload` (flexible array) |
| 16 | 0x0010 | 16 | `withEncryption.encryptionTag` |
| 32 | 0x0020 | 4 | `withEncryption.nvdmPayloadSize` |
| 36 | 0x0024 | 4 | `withEncryption.reserved` |
| 40 | 0x0028 | flex | `withEncryption.payload` (flexible array) |

#### `GSP_MSG_QUEUE_ENCRYPTION_TAG`: sizeof 16 (0x10), align 1

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 16 | `authTagBuffer` |

#### `GMCAPI_HEADER`: sizeof 40 (0x28), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `command` |
| 4 | 0x0004 | 4 | `size` |
| 8 | 0x0008 | 8 | `sequence` |
| 16 | 0x0010 | 4 | `request.max_response_size` |
| 16 | 0x0010 | 4 | `response.status` |
| 20 | 0x0014 | 20 | `reserved` |
| 40 | 0x0028 | flex | `data` (flexible array) |

#### `rpc_message_header_v`: sizeof 32 (0x20), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `header_version` |
| 4 | 0x0004 | 4 | `signature` |
| 8 | 0x0008 | 4 | `length` |
| 12 | 0x000c | 4 | `function` |
| 16 | 0x0010 | 4 | `rpc_result` |
| 20 | 0x0014 | 4 | `rpc_result_private` |
| 24 | 0x0018 | 4 | `sequence` |
| 28 | 0x001c | 4 | `u` |
| 28 | 0x001c | 4 | `u.spare` |
| 28 | 0x001c | 4 | `u.cpuRmGfid` |
| 32 | 0x0020 | flex | `rpc_message_data` (flexible array) |

### Init RPCs: CPU to GSP and replies

#### `rpc_set_guest_system_info_v03_00`: sizeof 792 (0x318), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `vgxVersionMajorNum` |
| 4 | 0x0004 | 4 | `vgxVersionMinorNum` |
| 8 | 0x0008 | 4 | `guestDriverVersionBufferLength` |
| 12 | 0x000c | 4 | `guestVersionBufferLength` |
| 16 | 0x0010 | 4 | `guestTitleBufferLength` |
| 20 | 0x0014 | 4 | `guestClNum` |
| 24 | 0x0018 | 256 | `guestDriverVersion` |
| 280 | 0x0118 | 256 | `guestVersion` |
| 536 | 0x0218 | 256 | `guestTitle` |

#### `rpc_set_guest_system_info_ext_v25_1B`: sizeof 268 (0x10c), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 256 | `guestDriverBranch` |
| 256 | 0x0100 | 4 | `domain` |
| 260 | 0x0104 | 2 | `bus` |
| 262 | 0x0106 | 2 | `device` |
| 264 | 0x0108 | 4 | `gridBuildCsp` |

#### `PACKED_REGISTRY_TABLE`: sizeof 8 (0x8), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `size` |
| 4 | 0x0004 | 4 | `numEntries` |
| 8 | 0x0008 | 0 | `entries` |

#### `PACKED_REGISTRY_ENTRY`: sizeof 16 (0x10), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `nameOffset` |
| 4 | 0x0004 | 1 | `type` |
| 8 | 0x0008 | 4 | `data` |
| 12 | 0x000c | 4 | `length` |

#### `rpc_get_gsp_static_info_v14_00`: sizeof 4 (0x4), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `data` |

#### `rpc_init_done_v17_00`: sizeof 1 (0x1), align 1

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 1 | `bIsD3Hot` |

### RM API passthrough RPCs

#### `rpc_gsp_rm_alloc_v03_00`: sizeof 32 (0x20), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `hClient` |
| 4 | 0x0004 | 4 | `hParent` |
| 8 | 0x0008 | 4 | `hObject` |
| 12 | 0x000c | 4 | `hClass` |
| 16 | 0x0010 | 4 | `status` |
| 20 | 0x0014 | 4 | `paramsSize` |
| 24 | 0x0018 | 4 | `flags` |
| 28 | 0x001c | 4 | `reserved` |
| 32 | 0x0020 | flex | `params` (flexible array) |

#### `rpc_gsp_rm_control_v03_00`: sizeof 40 (0x28), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `hClient` |
| 4 | 0x0004 | 4 | `hObject` |
| 8 | 0x0008 | 4 | `cmd` |
| 12 | 0x000c | 4 | `status` |
| 16 | 0x0010 | 4 | `paramsSize` |
| 20 | 0x0014 | 4 | `rmapiRpcFlags` |
| 24 | 0x0018 | 4 | `rmctrlFlags` |
| 28 | 0x001c | 4 | `rmctrlAccessRight` |
| 32 | 0x0020 | 8 | `reserved0` |
| 40 | 0x0028 | flex | `params` (flexible array) |

#### `rpc_free_v03_00`: sizeof 16 (0x10), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 16 | `params` |
| 0 | 0x0000 | 4 | `params.hRoot` |
| 4 | 0x0004 | 4 | `params.hObjectParent` |
| 8 | 0x0008 | 4 | `params.hObjectOld` |
| 12 | 0x000c | 4 | `params.status` |

#### `rpc_dup_object_v03_00`: sizeof 28 (0x1c), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 28 | `params` |
| 0 | 0x0000 | 4 | `params.hClient` |
| 4 | 0x0004 | 4 | `params.hParent` |
| 8 | 0x0008 | 4 | `params.hObject` |
| 12 | 0x000c | 4 | `params.hClientSrc` |
| 16 | 0x0010 | 4 | `params.hObjectSrc` |
| 20 | 0x0014 | 4 | `params.flags` |
| 24 | 0x0018 | 4 | `params.status` |

### Events GSP to CPU that the boot path accepts

#### `rpc_os_error_log_v17_00`: sizeof 272 (0x110), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `exceptType` |
| 4 | 0x0004 | 4 | `preemptiveRemovalPreviousXid` |
| 8 | 0x0008 | 4 | `runlistId` |
| 12 | 0x000c | 4 | `chid` |
| 16 | 0x0010 | 256 | `errString` |

#### `rpc_post_event_v17_00`: sizeof 32 (0x20), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `hClient` |
| 4 | 0x0004 | 4 | `hEvent` |
| 8 | 0x0008 | 4 | `notifyIndex` |
| 12 | 0x000c | 4 | `data` |
| 16 | 0x0010 | 2 | `info16` |
| 20 | 0x0014 | 4 | `status` |
| 24 | 0x0018 | 4 | `eventDataSize` |
| 28 | 0x001c | 1 | `bNotifyList` |
| 29 | 0x001d | flex | `eventData` (flexible array) |

#### `rpc_ucode_libos_print_v1E_08`: sizeof 8 (0x8), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `ucodeEngDesc` |
| 4 | 0x0004 | 4 | `libosPrintBufSize` |
| 8 | 0x0008 | flex | `libosPrintBuf` (flexible array) |

#### `rpc_gsp_lockdown_notice_v17_00`: sizeof 1 (0x1), align 1

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 1 | `bLockdownEngaging` |

### GET_GSP_STATIC_INFO reply payload

#### `GspStaticConfigInfo`: sizeof 1608 (0x648), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 268 | `gidInfo` |
| 0 | 0x0000 | 4 | `gidInfo.index` |
| 4 | 0x0004 | 4 | `gidInfo.flags` |
| 8 | 0x0008 | 4 | `gidInfo.length` |
| 12 | 0x000c | 256 | `gidInfo.data` |
| 268 | 0x010c | 48 | `SKUInfo` |
| 268 | 0x010c | 4 | `SKUInfo.BoardID` |
| 272 | 0x0110 | 9 | `SKUInfo.chipSKU` |
| 281 | 0x0119 | 5 | `SKUInfo.chipSKUMod` |
| 288 | 0x0120 | 4 | `SKUInfo.skuConfigVersion` |
| 292 | 0x0124 | 5 | `SKUInfo.project` |
| 297 | 0x0129 | 5 | `SKUInfo.projectSKU` |
| 302 | 0x012e | 6 | `SKUInfo.CDP` |
| 308 | 0x0134 | 2 | `SKUInfo.projectSKUMod` |
| 312 | 0x0138 | 4 | `SKUInfo.businessCycle` |
| 320 | 0x0140 | 904 | `fbRegionInfoParams` |
| 320 | 0x0140 | 4 | `fbRegionInfoParams.numFBRegions` |
| 328 | 0x0148 | 896 | `fbRegionInfoParams.fbRegion` |
| 1224 | 0x04c8 | 1 | `bPdiValid` |
| 1232 | 0x04d0 | 8 | `pdi` |
| 1240 | 0x04d8 | 80 | `sriovCaps` |
| 1240 | 0x04d8 | 4 | `sriovCaps.totalVFs` |
| 1244 | 0x04dc | 4 | `sriovCaps.firstVfOffset` |
| 1248 | 0x04e0 | 4 | `sriovCaps.vfFeatureMask` |
| 1256 | 0x04e8 | 8 | `sriovCaps.FirstVFBar0Address` |
| 1264 | 0x04f0 | 8 | `sriovCaps.FirstVFBar1Address` |
| 1272 | 0x04f8 | 8 | `sriovCaps.FirstVFBar2Address` |
| 1280 | 0x0500 | 8 | `sriovCaps.bar0Size` |
| 1288 | 0x0508 | 8 | `sriovCaps.bar1Size` |
| 1296 | 0x0510 | 8 | `sriovCaps.bar2Size` |
| 1304 | 0x0518 | 1 | `sriovCaps.b64bitBar0` |
| 1305 | 0x0519 | 1 | `sriovCaps.b64bitBar1` |
| 1306 | 0x051a | 1 | `sriovCaps.b64bitBar2` |
| 1307 | 0x051b | 1 | `sriovCaps.bSriovEnabled` |
| 1308 | 0x051c | 1 | `sriovCaps.bSriovHeavyEnabled` |
| 1309 | 0x051d | 1 | `sriovCaps.bEmulateVFBar0TlbInvalidationRegister` |
| 1310 | 0x051e | 1 | `sriovCaps.bClientRmAllocatedCtxBuffer` |
| 1311 | 0x051f | 1 | `sriovCaps.bNonPowerOf2ChannelCountSupported` |
| 1312 | 0x0520 | 1 | `sriovCaps.bVfResizableBAR1Supported` |
| 1320 | 0x0528 | 4 | `sriovMaxGfid` |
| 1324 | 0x052c | 12 | `engineCaps` |
| 1336 | 0x0538 | 1 | `poisonFuseEnabled` |
| 1344 | 0x0540 | 8 | `fb_length` |
| 1352 | 0x0548 | 64 | `gpuNameString` |
| 1416 | 0x0588 | 64 | `gpuShortNameString` |
| 1480 | 0x05c8 | 1 | `bGpuInternalSku` |
| 1481 | 0x05c9 | 1 | `bIsQuadroGeneric` |
| 1482 | 0x05ca | 1 | `bIsQuadroAd` |
| 1483 | 0x05cb | 1 | `bIsNvidiaNvs` |
| 1484 | 0x05cc | 1 | `bIsVgx` |
| 1485 | 0x05cd | 1 | `bGeforceSmb` |
| 1486 | 0x05ce | 1 | `bIsTitan` |
| 1487 | 0x05cf | 1 | `bIsTesla` |
| 1488 | 0x05d0 | 1 | `bIsMobile` |
| 1490 | 0x05d2 | 1 | `bIsGc6Rtd3Allowed` |
| 1491 | 0x05d3 | 1 | `bIsGc8Rtd3Allowed` |
| 1492 | 0x05d4 | 1 | `bIsGcOffRtd3Allowed` |
| 1493 | 0x05d5 | 1 | `bIsGcoffLegacyAllowed` |
| 1494 | 0x05d6 | 1 | `bIsMigSupported` |
| 1496 | 0x05d8 | 2 | `RTD3GC6TotalBoardPower` |
| 1498 | 0x05da | 2 | `RTD3GC6PerstDelay` |
| 1504 | 0x05e0 | 8 | `bar1PdeBase` |
| 1512 | 0x05e8 | 8 | `bar2PdeBase` |
| 1520 | 0x05f0 | 1 | `bVbiosValid` |
| 1524 | 0x05f4 | 4 | `vbiosSubVendor` |
| 1528 | 0x05f8 | 4 | `vbiosSubDevice` |
| 1532 | 0x05fc | 4 | `vbiosRevision` |
| 1536 | 0x0600 | 4 | `vbiosOemRevision` |
| 1540 | 0x0604 | 1 | `bPageRetirementSupported` |
| 1541 | 0x0605 | 1 | `bSplitVasBetweenServerClientRm` |
| 1542 | 0x0606 | 1 | `bClRootportNeedsNosnoopWAR` |
| 1544 | 0x0608 | 4 | `hInternalClient` |
| 1548 | 0x060c | 4 | `hInternalDevice` |
| 1552 | 0x0610 | 4 | `hInternalSubdevice` |
| 1556 | 0x0614 | 1 | `bSelfHostedMode` |
| 1557 | 0x0615 | 1 | `bAtsSupported` |
| 1558 | 0x0616 | 1 | `bSysL2CacheCoherentMode` |
| 1559 | 0x0617 | 1 | `bIsGpuUefi` |
| 1560 | 0x0618 | 1 | `bIsEfiInit` |
| 1568 | 0x0620 | 16 | `ecidInfo` |
| 1568 | 0x0620 | 16 | `ecidInfo.info` |
| 1584 | 0x0630 | 16 | `fwWprLayoutOffset` |
| 1584 | 0x0630 | 8 | `fwWprLayoutOffset.nonWprHeapOffset` |
| 1592 | 0x0638 | 8 | `fwWprLayoutOffset.frtsOffset` |
| 1600 | 0x0640 | 1 | `bBusResetRequired` |

#### `NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO`: sizeof 56 (0x38), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | `base` |
| 8 | 0x0008 | 8 | `limit` |
| 16 | 0x0010 | 8 | `reserved` |
| 24 | 0x0018 | 4 | `performance` |
| 28 | 0x001c | 1 | `supportCompressed` |
| 29 | 0x001d | 1 | `supportISO` |
| 30 | 0x001e | 1 | `bProtected` |
| 31 | 0x001f | 18 | `blackList` |
| 52 | 0x0034 | 4 | `regionTag` |

### GSP_SET_SYSTEM_INFO payload

#### `GspSystemInfo`: sizeof 952 (0x3b8), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | `gpuPhysAddr` |
| 8 | 0x0008 | 8 | `gpuPhysFbAddr` |
| 16 | 0x0010 | 8 | `gpuPhysInstAddr` |
| 24 | 0x0018 | 8 | `gpuPhysIoAddr` |
| 32 | 0x0020 | 8 | `nvDomainBusDeviceFunc` |
| 40 | 0x0028 | 8 | `simAccessBufPhysAddr` |
| 48 | 0x0030 | 8 | `notifyOpSharedSurfacePhysAddr` |
| 56 | 0x0038 | 8 | `pcieAtomicsOpMask` |
| 64 | 0x0040 | 8 | `consoleMemSize` |
| 72 | 0x0048 | 8 | `maxUserVa` |
| 80 | 0x0050 | 4 | `pciConfigMirrorBase` |
| 84 | 0x0054 | 4 | `pciConfigMirrorSize` |
| 88 | 0x0058 | 4 | `PCIDeviceID` |
| 92 | 0x005c | 4 | `PCISubDeviceID` |
| 96 | 0x0060 | 4 | `PCIRevisionID` |
| 100 | 0x0064 | 4 | `pcieAtomicsCplDeviceCapMask` |
| 104 | 0x0068 | 1 | `oorArch` |
| 112 | 0x0070 | 8 | `clPdbProperties` |
| 120 | 0x0078 | 4 | `Chipset` |
| 124 | 0x007c | 1 | `bGpuBehindBridge` |
| 125 | 0x007d | 1 | `bFlrSupported` |
| 126 | 0x007e | 1 | `b64bBar0Supported` |
| 127 | 0x007f | 1 | `bMnocAvailable` |
| 128 | 0x0080 | 4 | `chipsetL1ssEnable` |
| 132 | 0x0084 | 1 | `bUpstreamL0sUnsupported` |
| 133 | 0x0085 | 1 | `bUpstreamL1Unsupported` |
| 134 | 0x0086 | 1 | `bUpstreamL1PorSupported` |
| 135 | 0x0087 | 1 | `bUpstreamL1PorMobileOnly` |
| 136 | 0x0088 | 1 | `bSystemHasMux` |
| 137 | 0x0089 | 1 | `upstreamAddressValid` |
| 138 | 0x008a | 10 | `FHBBusInfo` |
| 138 | 0x008a | 2 | `FHBBusInfo.deviceID` |
| 140 | 0x008c | 2 | `FHBBusInfo.vendorID` |
| 142 | 0x008e | 2 | `FHBBusInfo.subdeviceID` |
| 144 | 0x0090 | 2 | `FHBBusInfo.subvendorID` |
| 146 | 0x0092 | 1 | `FHBBusInfo.revisionID` |
| 148 | 0x0094 | 10 | `chipsetIDInfo` |
| 148 | 0x0094 | 2 | `chipsetIDInfo.deviceID` |
| 150 | 0x0096 | 2 | `chipsetIDInfo.vendorID` |
| 152 | 0x0098 | 2 | `chipsetIDInfo.subdeviceID` |
| 154 | 0x009a | 2 | `chipsetIDInfo.subvendorID` |
| 156 | 0x009c | 1 | `chipsetIDInfo.revisionID` |
| 160 | 0x00a0 | 676 | `acpiMethodData` |
| 160 | 0x00a0 | 1 | `acpiMethodData.bValid` |
| 164 | 0x00a4 | 72 | `acpiMethodData.dodMethodData` |
| 164 | 0x00a4 | 4 | `acpiMethodData.dodMethodData.status` |
| 168 | 0x00a8 | 4 | `acpiMethodData.dodMethodData.acpiIdListLen` |
| 172 | 0x00ac | 64 | `acpiMethodData.dodMethodData.acpiIdList` |
| 236 | 0x00ec | 12 | `acpiMethodData.jtMethodData` |
| 236 | 0x00ec | 4 | `acpiMethodData.jtMethodData.status` |
| 240 | 0x00f0 | 4 | `acpiMethodData.jtMethodData.jtCaps` |
| 244 | 0x00f4 | 2 | `acpiMethodData.jtMethodData.jtRevId` |
| 246 | 0x00f6 | 1 | `acpiMethodData.jtMethodData.bSBIOSCaps` |
| 248 | 0x00f8 | 580 | `acpiMethodData.muxMethodData` |
| 248 | 0x00f8 | 4 | `acpiMethodData.muxMethodData.tableLen` |
| 252 | 0x00fc | 192 | `acpiMethodData.muxMethodData.acpiIdMuxModeTable` |
| 444 | 0x01bc | 192 | `acpiMethodData.muxMethodData.acpiIdMuxPartTable` |
| 636 | 0x027c | 192 | `acpiMethodData.muxMethodData.acpiIdMuxStateTable` |
| 828 | 0x033c | 8 | `acpiMethodData.capsMethodData` |
| 828 | 0x033c | 4 | `acpiMethodData.capsMethodData.status` |
| 832 | 0x0340 | 4 | `acpiMethodData.capsMethodData.optimusCaps` |
| 836 | 0x0344 | 4 | `hypervisorType` |
| 840 | 0x0348 | 2 | `virtualConfigBits` |
| 842 | 0x034a | 1 | `bIsPassthru` |
| 848 | 0x0350 | 8 | `sysTimerOffsetNs` |
| 856 | 0x0358 | 40 | `gspVFInfo` |
| 856 | 0x0358 | 4 | `gspVFInfo.totalVFs` |
| 860 | 0x035c | 4 | `gspVFInfo.firstVFOffset` |
| 864 | 0x0360 | 8 | `gspVFInfo.FirstVFBar0Address` |
| 872 | 0x0368 | 8 | `gspVFInfo.FirstVFBar1Address` |
| 880 | 0x0370 | 8 | `gspVFInfo.FirstVFBar2Address` |
| 888 | 0x0378 | 1 | `gspVFInfo.b64bitBar0` |
| 889 | 0x0379 | 1 | `gspVFInfo.b64bitBar1` |
| 890 | 0x037a | 1 | `gspVFInfo.b64bitBar2` |
| 896 | 0x0380 | 1 | `bIsPrimary` |
| 897 | 0x0381 | 1 | `bIsUnixHdmiFrlComplianceEnabled` |
| 898 | 0x0382 | 1 | `isGridBuild` |
| 900 | 0x0384 | 4 | `pcieConfigReg` |
| 900 | 0x0384 | 4 | `pcieConfigReg.linkCap` |
| 904 | 0x0388 | 4 | `gridBuildCsp` |
| 908 | 0x038c | 1 | `bPreserveVideoMemoryAllocations` |
| 909 | 0x038d | 1 | `bTdrEventSupported` |
| 910 | 0x038e | 1 | `bFeatureStretchVblankCapable` |
| 911 | 0x038f | 1 | `bEnableDynamicGranularityPageArrays` |
| 912 | 0x0390 | 1 | `bClockBoostSupported` |
| 920 | 0x0398 | 8 | `hostPageSize` |
| 928 | 0x03a0 | 1 | `bIsCmcBasedHws` |
| 929 | 0x03a1 | 1 | `bGspNocatEnabled` |
| 930 | 0x03a2 | 1 | `bS0ixSupport` |
| 931 | 0x03a3 | 1 | `bWindowChannelAlwaysMapped` |
| 932 | 0x03a4 | 4 | `pciePowerControlValue` |
| 936 | 0x03a8 | 1 | `bPciePowerControlPresent` |
| 940 | 0x03ac | 4 | `pf0DeviceControl2Reg` |
| 944 | 0x03b0 | 1 | `bIsCxlDevice` |

#### `MUX_METHOD_DATA_ELEMENT`: sizeof 12 (0xc), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `acpiId` |
| 4 | 0x0004 | 4 | `mode` |
| 8 | 0x0008 | 4 | `status` |

### Boot arguments (RMARGS region, libos init args, WPR meta)

#### `GSP_ARGUMENTS_CACHED`: sizeof 184 (0xb8), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `magic` |
| 4 | 0x0004 | 2 | `size` |
| 8 | 0x0008 | 8 | `flags` |
| 16 | 0x0010 | 64 | `messageQueueInitArguments` |
| 16 | 0x0010 | 8 | `messageQueueInitArguments.flags` |
| 24 | 0x0018 | 8 | `messageQueueInitArguments.sharedMemPhysAddr` |
| 68 | 0x0044 | 4 | `messageQueueInitArguments.pageTableEntryCount` |
| 32 | 0x0020 | 8 | `messageQueueInitArguments.cmdQueueOffset` |
| 40 | 0x0028 | 8 | `messageQueueInitArguments.statQueueOffset` |
| 48 | 0x0030 | 4 | `messageQueueInitArguments.queueElementHdrSize` |
| 52 | 0x0034 | 4 | `messageQueueInitArguments.queueElementSizeMin` |
| 56 | 0x0038 | 4 | `messageQueueInitArguments.queueElementSizeMax` |
| 60 | 0x003c | 4 | `messageQueueInitArguments.queueHeaderAlign` |
| 64 | 0x0040 | 4 | `messageQueueInitArguments.queueElementAlign` |
| 80 | 0x0050 | 16 | `srInitArguments` |
| 80 | 0x0050 | 4 | `srInitArguments.oldLevel` |
| 84 | 0x0054 | 4 | `srInitArguments.flags` |
| 88 | 0x0058 | 8 | `srInitArguments.reserved` |
| 96 | 0x0060 | 4 | `reserved2` |
| 100 | 0x0064 | 4 | `gpuInstance` |
| 104 | 0x0068 | 16 | `profilerArgs` |
| 104 | 0x0068 | 8 | `profilerArgs.pa` |
| 112 | 0x0070 | 8 | `profilerArgs.size` |
| 120 | 0x0078 | 16 | `sysmemHeapArgs` |
| 120 | 0x0078 | 8 | `sysmemHeapArgs.pa` |
| 128 | 0x0080 | 8 | `sysmemHeapArgs.size` |
| 136 | 0x0088 | 16 | `rmStateMonitorBufferArgs` |
| 136 | 0x0088 | 8 | `rmStateMonitorBufferArgs.pa` |
| 144 | 0x0090 | 8 | `rmStateMonitorBufferArgs.size` |
| 152 | 0x0098 | 16 | `bindataArgs` |
| 152 | 0x0098 | 8 | `bindataArgs.radix3` |
| 160 | 0x00a0 | 8 | `bindataArgs.size` |
| 168 | 0x00a8 | 8 | `vbiosOverrideArgs.pa` |
| 176 | 0x00b0 | 8 | `vbiosOverrideArgs.size` |

#### `MESSAGE_QUEUE_INIT_ARGUMENTS`: sizeof 64 (0x40), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | `flags` |
| 8 | 0x0008 | 8 | `sharedMemPhysAddr` |
| 52 | 0x0034 | 4 | `pageTableEntryCount` |
| 16 | 0x0010 | 8 | `cmdQueueOffset` |
| 24 | 0x0018 | 8 | `statQueueOffset` |
| 32 | 0x0020 | 4 | `queueElementHdrSize` |
| 36 | 0x0024 | 4 | `queueElementSizeMin` |
| 40 | 0x0028 | 4 | `queueElementSizeMax` |
| 44 | 0x002c | 4 | `queueHeaderAlign` |
| 48 | 0x0030 | 4 | `queueElementAlign` |
| 56 | 0x0038 | 8 | `reserved` |

#### `GSP_SR_INIT_ARGUMENTS`: sizeof 16 (0x10), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `oldLevel` |
| 4 | 0x0004 | 4 | `flags` |
| 8 | 0x0008 | 8 | `reserved` |

#### `LibosMemoryRegionInitArgument`: sizeof 32 (0x20), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | `id8` |
| 8 | 0x0008 | 8 | `pa` |
| 16 | 0x0010 | 8 | `size` |
| 24 | 0x0018 | 1 | `kind` |
| 25 | 0x0019 | 1 | `loc` |

#### `GspFwWprMeta`: sizeof 256 (0x100), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | `magic` |
| 8 | 0x0008 | 8 | `revision` |
| 16 | 0x0010 | 8 | `sysmemAddrOfRadix3Elf` |
| 24 | 0x0018 | 8 | `sizeOfRadix3Elf` |
| 32 | 0x0020 | 8 | `sysmemAddrOfBootloader` |
| 40 | 0x0028 | 8 | `sizeOfBootloader` |
| 48 | 0x0030 | 8 | `bootloaderCodeOffset` |
| 56 | 0x0038 | 8 | `bootloaderDataOffset` |
| 64 | 0x0040 | 8 | `bootloaderManifestOffset` |
| 72 | 0x0048 | 8 | `sysmemAddrOfSignature` |
| 80 | 0x0050 | 8 | `sizeOfSignature` |
| 72 | 0x0048 | 4 | `gspFwHeapFreeListWprOffset` |
| 76 | 0x004c | 4 | `unused0` |
| 80 | 0x0050 | 8 | `unused1` |
| 88 | 0x0058 | 8 | `gspFwRsvdStart` |
| 96 | 0x0060 | 8 | `nonWprHeapOffset` |
| 104 | 0x0068 | 8 | `nonWprHeapSize` |
| 112 | 0x0070 | 8 | `gspFwWprStart` |
| 120 | 0x0078 | 8 | `gspFwHeapOffset` |
| 128 | 0x0080 | 8 | `gspFwHeapSize` |
| 136 | 0x0088 | 8 | `gspFwOffset` |
| 144 | 0x0090 | 8 | `bootBinOffset` |
| 152 | 0x0098 | 8 | `frtsOffset` |
| 160 | 0x00a0 | 8 | `frtsSize` |
| 168 | 0x00a8 | 8 | `gspFwWprEnd` |
| 176 | 0x00b0 | 8 | `fbSize` |
| 184 | 0x00b8 | 8 | `vgaWorkspaceOffset` |
| 192 | 0x00c0 | 8 | `vgaWorkspaceSize` |
| 200 | 0x00c8 | 8 | `bootCount` |
| 208 | 0x00d0 | 8 | `partitionRpcAddr` |
| 216 | 0x00d8 | 2 | `partitionRpcRequestOffset` |
| 218 | 0x00da | 2 | `partitionRpcReplyOffset` |
| 220 | 0x00dc | 4 | `elfCodeOffset` |
| 224 | 0x00e0 | 4 | `elfDataOffset` |
| 228 | 0x00e4 | 4 | `elfCodeSize` |
| 232 | 0x00e8 | 4 | `elfDataSize` |
| 236 | 0x00ec | 4 | `lsUcodeVersion` |
| 208 | 0x00d0 | 16 | `partitionRpcPadding` |
| 224 | 0x00e0 | 8 | `sysmemAddrOfCrashReportQueue` |
| 232 | 0x00e8 | 4 | `sizeOfCrashReportQueue` |
| 236 | 0x00ec | 4 | `lsUcodeVersionPadding` |
| 240 | 0x00f0 | 1 | `gspFwHeapVfPartitionCount` |
| 241 | 0x00f1 | 1 | `flags` |
| 242 | 0x00f2 | 2 | `pagingConfig` |
| 244 | 0x00f4 | 4 | `pmuReservedSize` |
| 248 | 0x00f8 | 8 | `verified` |

### GSP-FMC boot parameters (Hopper and later only, not used on Turing)

#### `GSP_FMC_INIT_PARAMS`: sizeof 8 (0x8), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `regkeys` |

#### `GSP_FMC_BOOT_PARAMS`: sizeof 256 (0x100), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 32 | 0x0020 | 8 | `initParams` |
| 32 | 0x0020 | 4 | `initParams.regkeys` |
| 64 | 0x0040 | 32 | `bootGspRmParams` |
| 64 | 0x0040 | 4 | `bootGspRmParams.target` |
| 68 | 0x0044 | 4 | `bootGspRmParams.gspRmDescSize` |
| 72 | 0x0048 | 8 | `bootGspRmParams.gspRmDescOffset` |
| 80 | 0x0050 | 8 | `bootGspRmParams.wprCarveoutOffset` |
| 88 | 0x0058 | 4 | `bootGspRmParams.wprCarveoutSize` |
| 92 | 0x005c | 1 | `bootGspRmParams.bIsGspRmBoot` |
| 93 | 0x005d | 1 | `bootGspRmParams.bInstInSysMode` |
| 128 | 0x0080 | 16 | `gspRmParams` |
| 128 | 0x0080 | 4 | `gspRmParams.target` |
| 136 | 0x0088 | 8 | `gspRmParams.bootArgsOffset` |
| 160 | 0x00a0 | 16 | `gspSpdmParams` |
| 160 | 0x00a0 | 4 | `gspSpdmParams.target` |
| 168 | 0x00a8 | 8 | `gspSpdmParams.payloadBufferOffset` |
| 164 | 0x00a4 | 4 | `gspSpdmParams.payloadBufferSize` |
| 192 | 0x00c0 | 8 | `gspRmMemParams` |
| 192 | 0x00c0 | 4 | `gspRmMemParams.flushSysmemAddrValLo` |
| 196 | 0x00c4 | 4 | `gspRmMemParams.flushSysmemAddrValHi` |

### LIBOS log buffer header (no struct; NvU64 words)

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | put pointer, in NvU64 entries (liblogdecode.h:156, liblogdecode.c:2543) |
| 8 | 0x0008 | 8 | buffer PA, written by the CPU before boot (kernel_gsp.c:3649); also ring slot 0 |
| 8 | 0x0008 | ... | ring of `size/8 - 1` NvU64 slots starting at word 1 (liblogdecode.c:1231,1279) |

### Derived constants (computed by the compiler from the same headers)

| name | decimal | hex |
|---|---:|---:|
| `queueElementHdrSize (no CC)` | 24 | 0x18 |
| `queueElementHdrSize (CC)` | 40 | 0x28 |
| `queueElementSizeMin` | 4096 | 0x1000 |
| `queueElementSizeMax` | 65536 | 0x10000 |
| `maxRpcSize = SizeMax - HdrSize` | 65512 | 0xffe8 |
| `max payload after rpc_message_header_v` | 65480 | 0xffc8 |
| `hdr + GspStaticConfigInfo (must be <= maxRpcSize)` | 1640 | 0x668 |
| `hdr + GspSystemInfo (must be <= maxRpcSize)` | 984 | 0x3d8 |
| `msgq entryOff (entryAlign 12)` | 4096 | 0x1000 |
| `msgq msgCount for a 0x40000 queue` | 63 | 0x3f |
| `mctpHeader = mctpCreateTransportHeader(1,1,0,0,0)` | 3221225473 | 0xc0000001 |
| `nvdmHeader = mctpCreateNvdmHeader(NVDM_TYPE_RM_RPC)` | 621862526 | 0x2510de7e |
| `MCTP_MSG_HEADER_VENDOR_ID_NV` | 4318 | 0x10de |
| `MCTP_MSG_HEADER_TYPE_VENDOR_PCI` | 126 | 0x7e |
| `NVDM_TYPE_RM_RPC` | 37 | 0x25 |
| `MSGQ_VERSION_MAJOR` | 2 | 0x2 |
| `MSGQ_VERSION_MINOR` | 0 | 0x0 |
| `MCTP_MAGIC` | 1296258128 | 0x4d435450 |
| `MCTP_MAGIC_SEEN` | 1397048654 | 0x5345454e |
| `GMCAPI_CMD_GSP_INIT` | 65537 | 0x10001 |
| `MSGQ_MSG_SIZE_MIN` | 16 | 0x10 |
| `header_version (rpc_common.c:155-156)` | 50331648 | 0x3000000 |
| `NV_VGPU_MSG_SIGNATURE_VALID` | 1129337430 | 0x43505256 |
| `NV_VGPU_MSG_RESULT_SUCCESS` | 0 | 0x0 |
| `NV_VGPU_MSG_RESULT_RPC_PENDING` | 4294967295 | 0xffffffff |
| `NV_VGPU_MSG_UNION_INIT` | 0 | 0x0 |
| `DRF_BASE(NV_VGPU_MSG_RESULT__VMIOP)` | 4278190080 | 0xff000000 |
| `VGX_MAJOR_VERSION_NUMBER` | 47 | 0x2f |
| `VGX_MINOR_VERSION_NUMBER` | 6 | 0x6 |
| `NV_VGPU_MSG_FUNCTION_FREE` | 10 | 0xa |
| `NV_VGPU_MSG_FUNCTION_SET_GUEST_SYSTEM_INFO` | 1 | 0x1 |
| `NV_VGPU_MSG_FUNCTION_DUP_OBJECT` | 21 | 0x15 |
| `NV_VGPU_MSG_FUNCTION_SET_GUEST_SYSTEM_INFO_EXT` | 64 | 0x40 |
| `NV_VGPU_MSG_FUNCTION_GET_GSP_STATIC_INFO` | 65 | 0x41 |
| `NV_VGPU_MSG_FUNCTION_GSP_SET_SYSTEM_INFO` | 72 | 0x48 |
| `NV_VGPU_MSG_FUNCTION_SET_REGISTRY` | 73 | 0x49 |
| `NV_VGPU_MSG_FUNCTION_GSP_RM_CONTROL` | 76 | 0x4c |
| `NV_VGPU_MSG_FUNCTION_GSP_RM_ALLOC` | 103 | 0x67 |
| `NV_VGPU_MSG_EVENT_GSP_INIT_DONE` | 4097 | 0x1001 |
| `NV_VGPU_MSG_EVENT_OS_ERROR_LOG` | 4102 | 0x1006 |
| `NV_VGPU_MSG_EVENT_POST_EVENT` | 4099 | 0x1003 |
| `NV_VGPU_MSG_EVENT_UCODE_LIBOS_PRINT` | 4108 | 0x100c |
| `NV_VGPU_MSG_EVENT_GSP_LOCKDOWN_NOTICE` | 4124 | 0x101c |
| `GSP_FW_WPR_META_MAGIC` | 15869187694674993331 | 0xdc3aae21371a60b3 |
| `GSP_FW_WPR_META_REVISION` | 1 | 0x1 |
| `GSP_FW_WPR_META_VERIFIED` | 11574427654092267680 | 0xa0a0a0a0a0a0a0a0 |
| `LIBOS_MEMORY_REGION_INIT_ARGUMENTS_MAX` | 4096 | 0x1000 |
| `LIBOS_MEMORY_REGION_CONTIGUOUS` | 1 | 0x1 |
| `LIBOS_MEMORY_REGION_LOC_SYSMEM` | 1 | 0x1 |
| `id8("LOGINIT")` | 21479265821149524 | 0x4c4f47494e4954 |
| `id8("LOGRM")` | 327747588685 | 0x4c4f47524d |
| `id8("RMARGS")` | 90491761870675 | 0x524d41524753 |

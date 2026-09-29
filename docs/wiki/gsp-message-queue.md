<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GSP message queue

The kernel RM and GSP-RM talk through two rings in guest RAM, a command queue and a status
queue, whose read and write pointers live in four BAR0 registers. Every RPC, every GMCAPI
command and every event is one element on one of the rings. VFIO Leandro's device serves
the command queue on each trapped pointer write and fills the status queue itself.

## Layout (msgq v2, 615)

- One sysmem allocation holds a page table, the command queue (0x40000 bytes) and the
  status queue (0x40000 unless `RmGspStatusQueueSize`); in the capture 129 pages, command
  queue at 0x1000, status queue at 0x41000. The guest hands its physical address over
  through the libos arguments named in the falcon mailboxes.
- Each side writes only a 32-byte `msgqTxHeader` at the start of its own transmit queue:
  `versionMajor` 2, `versionMinor` 0, `size`, `msgSize` 0x1000, `msgCount` 63, `entryOff`
  0x1000 (`src/nvidia/src/libraries/msgq/msgq.c`, `src/nvidia/inc/libraries/msgq/msgq_priv.h`).
  Major versions change with incompatible layouts, minor versions are compatible.
- `msgqRxLink` fails when the peer header's `size` does not match, which is where a 610
  device stops a 615 guest.

| Register (queue 0) | BAR0 | Written by | Holds |
|---|---:|---|---|
| `NV_PGSP_QUEUE_HEAD(0)` | 0x110C00 | CPU | command-queue write pointer |
| `NV_PGSP_QUEUE_TAIL(0)` | 0x110C04 | GSP | command-queue read pointer |
| `NV_PGSP_MSGQ_HEAD(0)` | 0x110C80 | GSP | status-queue write pointer |
| `NV_PGSP_MSGQ_TAIL(0)` | 0x110C84 | CPU | status-queue read pointer |

- `gpuGetGspMsgQueueRegisters_TU102` (`src/nvidia/src/kernel/gpu/arch/turing/kern_gpu_tu102.c`)
  maps queue 0 to these registers for TU10x through GB20x (`dev_gsp.h`). Each holds a
  monotonic counter used modulo `msgCount`; the ring is full at `write - read == msgCount`.
- In 610 the pointers were words in the shared header and one slot stayed free.

## The element

| Offset | Field | Value |
|---:|---|---|
| 0 | `mctpMagic` | 0x4D435450 "MCTP" |
| 4 | `mctpPayloadSize` | 24 + payload |
| 8 | `mctpHeader` | 0xC0000001 (version 1, start and end of message) |
| 12 | `nvdmHeader` | 0x2510DE7E for an RM RPC, 0x2610DE7E for GMCAPI (`nvdm_format.h`) |
| 16 | `nvdmPayloadSize` | payload bytes |
| 20 | reserved | 0 |

- Then the RPC header (`rpc_message_header_v`, 32 bytes: function, length, sequence,
  status) or the 40-byte `GMCAPI_HEADER`, then the payload
  (`src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h`).
- An element spans at most 16 slots, so `maxRpcSize` is 65512 bytes and larger RPCs go as
  `CONTINUATION_RECORD`s. No checksum, no sequence number in the framing (610 had both);
  the receiver re-reads an out-of-range size up to three times and checks version, vendor and
  magic (`src/nvidia/src/kernel/gpu/gsp/message_queue_cpu.c`). A consumed element's magic is
  overwritten with "SEEN".
- With confidential computing a 16-byte tag follows the framing; Turing has none and the
  device refuses `MESSAGE_QUEUE_INIT_FLAG_ENCRYPTION`.

## The doorbell and the poll

- The protocol defines no notification of its own: the sender copies the slots, issues a
  store fence and writes `QUEUE_HEAD` (`GspMsgQueueSendCommand` in `message_queue_cpu.c`),
  which raises the GSP interrupt on hardware. The receiver reads `MSGQ_HEAD` and fences
  before it reads the slots.
- The device serves the command queue on every trapped `QUEUE_HEAD` write and answers reads
  of `QUEUE_TAIL` and `MSGQ_HEAD` from its queue state. Only `QUEUE_HEAD` carries work.
- The kernel spins for the reply (`kgspRecvPoll`), reads `MSGQ_HEAD`, writes `MSGQ_TAIL`
  and does a health check of the falcon: four trapped accesses per RPC, 108 us median from
  the head write to the fourth, against 3 us of device work.

## GMCAPI

- `GMCAPI_HEADER` (40 bytes): command with a response flag, size, a 64-bit sequence, the
  maximum response size or the status (`src/nvidia/interface/gmcapi/gmcapi_base.h`).
- Ids are `category << 16 | n`; the GSP management category holds `GSP_INIT` 0x10001,
  `EXEC_GENERIC_BOOTLOADER`, `EXEC_HS_BINARY`, `GSP_SUSPEND` 0x10004 (fire and forget),
  `GSP_RESUME_DONE` (`gmcapi_table.h`).
- NVKV payloads are u64 words: key 15:0, index 27:16, opcode 31:28 (IMM32, SEQ32, SEQ64,
  ARRAY8/32/64), immediate or count in 63:32.

## Toward a polled queue

- In 615 the command queue's write pointer is a register, not a word in shared RAM, so a
  poll-mode device needs BAR0 page 0x110000 as a read-only memory area plus ioeventfds on
  `QUEUE_HEAD` and `MSGQ_TAIL`; that saves about 130 us per RPC
  ([gsp-transport-classes](gsp-transport-classes.md)).

## Depth

- [research/gsp-transport.md](../research/gsp-transport.md): the cited lines.
- [caraxes/gsp-transport-615.md](../caraxes/gsp-transport-615.md): the transport key by key,
  610 against 615, the device's `msgq::v2` and `gmcapi` modules.
- [caraxes/gsp-rpc-protocol.md](../caraxes/gsp-rpc-protocol.md): the 610 layout.

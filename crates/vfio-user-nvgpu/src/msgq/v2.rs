// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Message queue v2, the CPU-GSP transport from driver 615 on, from the GSP side
//! (docs/caraxes/gsp-transport-615.md §2-3).
//!
//! Paths are relative to the open-gpu-kernel-modules tree at tag 615.71.09. The
//! structures are `NV_ABI_STABLE` from 615 on (sdk/nvtypes.h:520-533): `msgqTxHeader`
//! (src/nvidia/inc/libraries/msgq/msgq_priv.h:82-91), `GSP_MSG_QUEUE_ELEMENT`
//! (src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h:90-121), `GSP_ARGUMENTS_CACHED`
//! and `MESSAGE_QUEUE_INIT_ARGUMENTS` (src/nvidia/inc/kernel/gpu/gsp/gsp_init_args.h:33-113).
//! Their offsets are therefore constants of this transport, not of a version module; the
//! test in `ver::v615` and the tests below check them against the compiler tables in
//! appendix A of docs/caraxes/gsp-transport-615.md.
//!
//! What changed against v0 (`super`, driver 610):
//!
//! - The shared buffer is laid out as before (page table, command queue, status queue,
//!   message_queue_cpu.c:300-326), but each queue starts with a v2 `msgqTxHeader`
//!   (version 2.0, size, msgSize, msgCount, entryOff) and has no rx header: the read and
//!   write pointers live in registers. The CPU writes the command queue's write pointer
//!   to `NV_PGSP_QUEUE_HEAD(0)` and the status queue's read pointer to
//!   `NV_PGSP_MSGQ_TAIL(0)`; it reads our read pointer from `NV_PGSP_QUEUE_TAIL(0)` and our
//!   write pointer from `NV_PGSP_MSGQ_HEAD(0)` (msgq.c:128-485, kern_gpu_tu102.c:596-605).
//!   The boot model owns those registers and hands the CPU's values to this state
//!   (`QueueStateV2::cpu_write_ptr`, `QueueStateV2::cpu_read_ptr`).
//! - Pointers are monotonic u32 counters used modulo msgCount; the ring is full when
//!   `write - read == msgCount` (msgq_priv.h:55-63), so no slot is kept free.
//! - An element starts with 24 bytes: `mctpMagic` "MCTP", `mctpPayloadSize` (header
//!   included), the MCTP transport header, the NVDM header (type 0x25 RM RPC or 0x26
//!   GMCAPI), `nvdmPayloadSize`, reserved. There is no checksum and no sequence number.
//!   Integrity rests on the store fence before the pointer write
//!   (message_queue_cpu.c:597-607) and on the receiver's checks of the MCTP version, the
//!   vendor id and the magic; a reader marks a consumed element "SEEN" and skips a
//!   "SEEN" element as stale (message_queue_cpu.c:705-777). This side does the same.

use std::collections::VecDeque;
use std::sync::atomic::{fence, Ordering};

use super::{
    QueueMemory, Rejection, RpcHeader, MAX_HELD, NV_ERR_INVALID_DATA, NV_ERR_INVALID_PARAM_STRUCT,
    RPC_HEADER_SIZE, RPC_HEADER_VERSION, RPC_SIGNATURE,
};
use crate::gmcapi;
use crate::guestmem::GuestMemory;

// ---------------------------------------------------------------------------------
// msgqTxHeader v2 (msgq_priv.h:31-91).

pub const MSGQ_VERSION_MAJOR: u16 = 2;
pub const MSGQ_VERSION_MINOR: u16 = 0;
pub const TX_HEADER_SIZE: usize = 32;
pub const TX_VERSION_MAJOR: usize = 0;
pub const TX_VERSION_MINOR: usize = 2;
pub const TX_SIZE: usize = 4;
pub const TX_MSG_SIZE: usize = 8;
pub const TX_MSG_COUNT: usize = 12;
pub const TX_ENTRY_OFF: usize = 16;

// ---------------------------------------------------------------------------------
// GSP_MSG_QUEUE_ELEMENT without confidential compute (message_queue_priv.h:90-128).

pub const ELEMENT_MAGIC: usize = 0;
pub const ELEMENT_MCTP_PAYLOAD_SIZE: usize = 4;
pub const ELEMENT_MCTP_HEADER: usize = 8;
pub const ELEMENT_NVDM_HEADER: usize = 12;
pub const ELEMENT_NVDM_PAYLOAD_SIZE: usize = 16;
pub const ELEMENT_RESERVED: usize = 20;
/// `GSP_MSG_QUEUE_ELEMENT_SIZE_NO_ENCRYPTION`: the payload starts here.
pub const ELEMENT_HEADER_SIZE: usize = 24;
/// `queueElementSizeMin` (one 4 KiB slot) and `queueElementSizeMax` (16 slots),
/// message_queue_cpu.c:85-86.
pub const ELEMENT_SIZE_MIN: usize = 0x1000;
pub const ELEMENT_SIZE_MAX: usize = 0x10000;
/// `maxRpcSize` (message_queue_priv.h:180-184).
pub const MAX_RPC_SIZE: usize = ELEMENT_SIZE_MAX - ELEMENT_HEADER_SIZE;

/// "MCTP" and "SEEN" (message_queue_priv.h:92-93).
pub const MCTP_MAGIC: u32 = 0x4D43_5450;
pub const MCTP_MAGIC_SEEN: u32 = 0x5345_454E;
/// `mctpCreateTransportHeader(1, 1, 0, 0, 0)`: version 1, SOM, EOM (mctp_format.h:40-97,
/// message_queue_cpu.c:503-509).
pub const MCTP_HEADER: u32 = 0xC000_0001;
/// `MCTP_HEADER_VERSION` 3:0 must be 1 (message_queue_cpu.c:707-717).
pub const MCTP_VERSION: u32 = 1;
/// `MCTP_MSG_HEADER_TYPE_VENDOR_PCI` and `MCTP_MSG_HEADER_VENDOR_ID_NV`
/// (mctp_format.h:50-61).
pub const MCTP_MSG_TYPE_VENDOR_PCI: u32 = 0x7E;
pub const MCTP_VENDOR_ID_NV: u32 = 0x10DE;
/// `NVDM_TYPE_RM_RPC` and `NVDM_TYPE_GMCAPI` (arch/nvalloc/common/inc/nvdm_format.h:62-63).
pub const NVDM_TYPE_RM_RPC: u32 = 0x25;
pub const NVDM_TYPE_GMCAPI: u32 = 0x26;

/// `mctpCreateNvdmHeader(type)`: message type 6:0 vendor-PCI, IC 0, vendor id 23:8,
/// NVDM type 31:24 (mctp_format.h:111-121).
pub const fn nvdm_header(nvdm_type: u32) -> u32 {
    MCTP_MSG_TYPE_VENDOR_PCI | (MCTP_VENDOR_ID_NV << 8) | (nvdm_type << 24)
}

// ---------------------------------------------------------------------------------
// GSP_ARGUMENTS_CACHED, the RMARGS region (gsp_init_args.h:33-113).

/// `GSP_ARGUMENTS_MAGIC_VALUE`, "GSP " (gsp_init_args.h:62).
pub const GSP_ARGUMENTS_MAGIC: u32 = 0x2050_5347;
/// `sizeof(GSP_ARGUMENTS_CACHED)`, written to its `size` (kernel_gsp.c:6153-6154).
pub const GSP_ARGUMENTS_CACHED_SIZE: u16 = 184;
pub const ARGS_MAGIC: u64 = 0;
pub const ARGS_SIZE: u64 = 4;
pub const ARGS_FLAGS: u64 = 8;
/// `messageQueueInitArguments` at 16, then its fields (`MESSAGE_QUEUE_INIT_ARGUMENTS`).
pub const ARGS_MQ: u64 = 16;
pub const ARGS_MQ_FLAGS: u64 = ARGS_MQ;
pub const ARGS_MQ_SHARED_MEM_PA: u64 = ARGS_MQ + 8;
pub const ARGS_MQ_CMD_QUEUE_OFFSET: u64 = ARGS_MQ + 16;
pub const ARGS_MQ_STAT_QUEUE_OFFSET: u64 = ARGS_MQ + 24;
pub const ARGS_MQ_ELEMENT_HDR_SIZE: u64 = ARGS_MQ + 32;
pub const ARGS_MQ_ELEMENT_SIZE_MIN: u64 = ARGS_MQ + 36;
pub const ARGS_MQ_ELEMENT_SIZE_MAX: u64 = ARGS_MQ + 40;
pub const ARGS_MQ_HEADER_ALIGN: u64 = ARGS_MQ + 44;
pub const ARGS_MQ_ELEMENT_ALIGN: u64 = ARGS_MQ + 48;
pub const ARGS_MQ_PAGE_TABLE_ENTRY_COUNT: u64 = ARGS_MQ + 52;
/// `MESSAGE_QUEUE_INIT_FLAG_ENCRYPTION` (gsp_init_args.h:35): confidential compute, not
/// on Turing; the device refuses it.
pub const MESSAGE_QUEUE_INIT_FLAG_ENCRYPTION: u64 = 0x01;

/// Whether the RMARGS region at `pa` is a 615 `GSP_ARGUMENTS_CACHED`: its first word is
/// the magic "GSP ". In a 610 region the first word is the low half of
/// `sharedMemPhysAddr`, a 4 KiB-aligned address (gsp_init_args.h of 610:33-40), which
/// can never read 0x20505347. `None` when the region is not mapped.
pub fn rmargs_is_v2(mem: &GuestMemory, rmargs_pa: u64) -> Option<bool> {
    mem.read_u32(rmargs_pa + ARGS_MAGIC)
        .map(|m| m == GSP_ARGUMENTS_MAGIC)
}

/// The queue arguments the CPU put into RMARGS (`kgspPopulateGspRmInitArgs`,
/// kernel_gsp.c:6139-6167).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueArgs {
    pub args_size: u16,
    pub args_flags: u64,
    pub flags: u64,
    pub shared_mem_pa: u64,
    pub cmd_queue_offset: u64,
    pub stat_queue_offset: u64,
    pub element_hdr_size: u32,
    pub element_size_min: u32,
    pub element_size_max: u32,
    pub header_align: u32,
    pub element_align: u32,
    pub page_table_entry_count: u32,
}

impl QueueArgs {
    /// Read and check the arguments: the magic, a size that covers them, no encryption,
    /// the element sizes of `_getMsgQueueParams` (message_queue_cpu.c:82-88), and a
    /// status queue inside the shared buffer after the command queue.
    pub fn read(mem: &GuestMemory, rmargs_pa: u64) -> Result<Self, String> {
        let mut b = [0u8; GSP_ARGUMENTS_CACHED_SIZE as usize];
        if !mem.read(rmargs_pa, &mut b) {
            return Err(format!("RMARGS at {rmargs_pa:#x} not mapped"));
        }
        let u32_at = |o: u64| u32::from_le_bytes(b[o as usize..o as usize + 4].try_into().unwrap());
        let u64_at = |o: u64| u64::from_le_bytes(b[o as usize..o as usize + 8].try_into().unwrap());
        let magic = u32_at(ARGS_MAGIC);
        if magic != GSP_ARGUMENTS_MAGIC {
            return Err(format!(
                "RMARGS at {rmargs_pa:#x}: magic {magic:#010x}, not \"GSP \" ({GSP_ARGUMENTS_MAGIC:#010x}): not a 615 guest"
            ));
        }
        let a = QueueArgs {
            args_size: u16::from_le_bytes([b[ARGS_SIZE as usize], b[ARGS_SIZE as usize + 1]]),
            args_flags: u64_at(ARGS_FLAGS),
            flags: u64_at(ARGS_MQ_FLAGS),
            shared_mem_pa: u64_at(ARGS_MQ_SHARED_MEM_PA),
            cmd_queue_offset: u64_at(ARGS_MQ_CMD_QUEUE_OFFSET),
            stat_queue_offset: u64_at(ARGS_MQ_STAT_QUEUE_OFFSET),
            element_hdr_size: u32_at(ARGS_MQ_ELEMENT_HDR_SIZE),
            element_size_min: u32_at(ARGS_MQ_ELEMENT_SIZE_MIN),
            element_size_max: u32_at(ARGS_MQ_ELEMENT_SIZE_MAX),
            header_align: u32_at(ARGS_MQ_HEADER_ALIGN),
            element_align: u32_at(ARGS_MQ_ELEMENT_ALIGN),
            page_table_entry_count: u32_at(ARGS_MQ_PAGE_TABLE_ENTRY_COUNT),
        };
        if (a.args_size as u64) < ARGS_MQ + 64 {
            return Err(format!(
                "RMARGS size {} does not cover the queue arguments",
                a.args_size
            ));
        }
        if a.flags & MESSAGE_QUEUE_INIT_FLAG_ENCRYPTION != 0 {
            return Err(
                "encrypted message queue (confidential compute) requested; not emulated".into(),
            );
        }
        if a.element_hdr_size as usize != ELEMENT_HEADER_SIZE
            || a.element_size_min as usize != ELEMENT_SIZE_MIN
            || a.element_size_max as usize != ELEMENT_SIZE_MAX
            || a.element_align != 12
        {
            return Err(format!("unexpected element geometry: {a:?}"));
        }
        let count = a.page_table_entry_count as u64;
        if count == 0
            || count > 4096
            || a.cmd_queue_offset == 0
            || a.stat_queue_offset <= a.cmd_queue_offset
            || a.stat_queue_offset >= count * 4096
        {
            return Err(format!("implausible queue arguments: {a:?}"));
        }
        Ok(a)
    }

    pub fn cmd_size(&self) -> u32 {
        (self.stat_queue_offset - self.cmd_queue_offset) as u32
    }

    pub fn stat_size(&self) -> u32 {
        (self.page_table_entry_count as u64 * 4096 - self.stat_queue_offset) as u32
    }
}

/// `msgqTxHeader`, version 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TxHeaderV2 {
    pub version_major: u16,
    pub version_minor: u16,
    pub size: u32,
    pub msg_size: u32,
    pub msg_count: u32,
    pub entry_off: u32,
}

impl TxHeaderV2 {
    pub fn parse(b: &[u8; TX_HEADER_SIZE]) -> Self {
        let u16_at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
        let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        TxHeaderV2 {
            version_major: u16_at(TX_VERSION_MAJOR),
            version_minor: u16_at(TX_VERSION_MINOR),
            size: u32_at(TX_SIZE),
            msg_size: u32_at(TX_MSG_SIZE),
            msg_count: u32_at(TX_MSG_COUNT),
            entry_off: u32_at(TX_ENTRY_OFF),
        }
    }

    /// The header with `reserved[3]` zero (msgq.c:126-140).
    pub fn encode(&self) -> [u8; TX_HEADER_SIZE] {
        let mut b = [0u8; TX_HEADER_SIZE];
        b[TX_VERSION_MAJOR..TX_VERSION_MAJOR + 2]
            .copy_from_slice(&self.version_major.to_le_bytes());
        b[TX_VERSION_MINOR..TX_VERSION_MINOR + 2]
            .copy_from_slice(&self.version_minor.to_le_bytes());
        for (o, v) in [
            (TX_SIZE, self.size),
            (TX_MSG_SIZE, self.msg_size),
            (TX_MSG_COUNT, self.msg_count),
            (TX_ENTRY_OFF, self.entry_off),
        ] {
            b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
        b
    }

    /// What `msgqTxCreate` writes for a queue of `size` bytes with 4 KiB entries aligned
    /// to 4 KiB (msgq.c:126-140, called from message_queue_cpu.c:175-183 with
    /// `queueElementSizeMin` and `queueElementAlign` 12).
    pub fn standard(size: u32) -> Self {
        let entry_off = (TX_HEADER_SIZE as u32).next_multiple_of(ELEMENT_SIZE_MIN as u32);
        let msg_size = ELEMENT_SIZE_MIN as u32;
        TxHeaderV2 {
            version_major: MSGQ_VERSION_MAJOR,
            version_minor: MSGQ_VERSION_MINOR,
            size,
            msg_size,
            msg_count: (size - entry_off) / msg_size,
            entry_off,
        }
    }

    /// The checks `msgqRxLink` makes on the other side's header (msgq.c:240-262), for a
    /// queue of `size` bytes and `msg_size` entries.
    pub fn check(&self, size: u32, msg_size: u32) -> Result<(), String> {
        if self.version_major != MSGQ_VERSION_MAJOR {
            return Err(format!(
                "msgq version {}.{}, not 2",
                self.version_major, self.version_minor
            ));
        }
        if self.size != size || self.msg_size != msg_size {
            return Err(format!(
                "size {:#x}/{:#x}, expected {size:#x}/{msg_size:#x}",
                self.size, self.msg_size
            ));
        }
        if self.entry_off < TX_HEADER_SIZE as u32 || size < self.entry_off + msg_size {
            return Err(format!("entryOff {:#x} does not fit", self.entry_off));
        }
        if self.msg_count != (size - self.entry_off) / msg_size {
            return Err(format!("msgCount {} inconsistent", self.msg_count));
        }
        Ok(())
    }
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn put_u32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// What a command-queue head write whose value the device did not see published
/// ([`QueueStateV2::peek_published`]).
#[derive(Debug, PartialEq, Eq)]
pub enum Published {
    /// An element of this many slots at the read pointer.
    Element { slots: u32 },
    /// No element: the write was `msgqTxCreate`'s 0 (msgq.c:176), a queue created anew.
    Stale(String),
    /// The queue failed; nothing is served until the guest resets the device.
    Failed,
}

/// The GSP side of both v2 queues.
#[derive(Debug)]
pub struct QueueStateV2 {
    pub cmd_off: u64,
    pub stat_off: u64,
    pub cmd: TxHeaderV2,
    pub stat: TxHeaderV2,
    /// Our read pointer into the command queue: what `NV_PGSP_QUEUE_TAIL(0)` reads.
    pub read_ptr: u32,
    /// Our write pointer into the status queue: what `NV_PGSP_MSGQ_HEAD(0)` reads.
    pub write_ptr: u32,
    /// The CPU's command-queue write pointer, last written to `NV_PGSP_QUEUE_HEAD(0)`.
    pub cpu_write_ptr: u32,
    /// The CPU's status-queue read pointer, last written to `NV_PGSP_MSGQ_TAIL(0)`.
    pub cpu_read_ptr: u32,
    /// Set once the queue is unusable (as `QueueState::failed`).
    pub failed: Option<String>,
    /// Elements consumed without dispatch (bad MCTP framing or lengths).
    pub rejected: u64,
    /// Elements skipped because their magic read "SEEN" (message_queue_cpu.c:739-772).
    pub stale: u64,
    /// Requests whose RPC or GMCAPI sequence was not the previous one plus one (logged).
    pub sequence_gaps: u64,
    last_sequence: Option<u32>,
    /// Elements sent, the index the trace shows in place of v0's seqNum.
    sent: u32,
    held: VecDeque<Vec<u8>>,
    held_stalls: u32,
}

impl QueueStateV2 {
    /// Check the CPU's command-queue header and publish ours in the status queue
    /// (the status header is what the CPU's `msgqRxLink` polls for, message_queue_cpu.c:
    /// 342-420, 4 s). `cpu_write_ptr` is the last value of `NV_PGSP_QUEUE_HEAD(0)`
    /// (0 from `msgqTxCreate`, msgq.c:175-177).
    pub fn link(qm: &QueueMemory, args: &QueueArgs, cpu_write_ptr: u32) -> Result<Self, String> {
        let mut raw = [0u8; TX_HEADER_SIZE];
        if !qm.read(args.cmd_queue_offset, &mut raw) {
            return Err(format!(
                "command queue header at {:#x} not readable",
                args.cmd_queue_offset
            ));
        }
        let cmd = TxHeaderV2::parse(&raw);
        cmd.check(args.cmd_size(), args.element_size_min)
            .map_err(|e| format!("command queue header rejected: {e}: {cmd:?}"))?;
        let stat = TxHeaderV2::standard(args.stat_size());
        if !qm.write(args.stat_queue_offset, &stat.encode()) {
            return Err(format!(
                "status queue header at {:#x} not writable",
                args.stat_queue_offset
            ));
        }
        fence(Ordering::Release);
        Ok(QueueStateV2 {
            cmd_off: args.cmd_queue_offset,
            stat_off: args.stat_queue_offset,
            cmd,
            stat,
            read_ptr: 0,
            write_ptr: 0,
            cpu_write_ptr,
            cpu_read_ptr: 0,
            failed: None,
            rejected: 0,
            stale: 0,
            sequence_gaps: 0,
            last_sequence: None,
            sent: 0,
            held: VecDeque::new(),
            held_stalls: 0,
        })
    }

    fn slot_off(base: u64, hdr: &TxHeaderV2, ptr: u32) -> u64 {
        base + hdr.entry_off as u64 + (ptr % hdr.msg_count) as u64 * hdr.msg_size as u64
    }

    fn fail(&mut self, reason: String) -> String {
        if self.failed.is_none() {
            log::error!(
                "GSP message queue (v2) failed: {reason}; no further RPCs are serviced until the guest resets the device"
            );
            self.failed = Some(reason);
        }
        self.failed_error()
    }

    fn failed_error(&self) -> String {
        format!(
            "queue failed: {}",
            self.failed.as_deref().unwrap_or("unknown")
        )
    }

    /// Elements the CPU published that we have not consumed (msgqRxGetReadAvailable,
    /// msgq.c:398-430). More than msgCount is a pointer we cannot trust.
    pub fn available(&self) -> u32 {
        self.cpu_write_ptr.wrapping_sub(self.read_ptr)
    }

    /// Take the next element from the command queue, if any, as a [`super::Message`]:
    /// an RM RPC with its `rpc_message_header_v`, or a GMCAPI request whose
    /// `GMCAPI_HEADER` is in [`super::Message::gmcapi`] and whose command id and sequence
    /// stand in the header's `function` and `sequence` (one namespace,
    /// gmcapi_base.h:76-80). A message with `rejected` set was consumed and must not be
    /// dispatched. `seq_num` is the element's queue index.
    pub fn receive(&mut self, qm: &QueueMemory) -> Result<Option<super::Message>, String> {
        if self.failed.is_some() {
            return Err(self.failed_error());
        }
        let n = self.cmd.msg_count;
        let avail = self.available();
        if avail > n {
            return Err(self.fail(format!(
                "command queue head {} is {avail} elements ahead of our tail {} (msgCount {n})",
                self.cpu_write_ptr, self.read_ptr
            )));
        }
        if avail == 0 {
            return Ok(None);
        }
        let msg_size = self.cmd.msg_size as usize;
        let index = self.read_ptr;
        let first_off = Self::slot_off(self.cmd_off, &self.cmd, index);
        let mut raw = vec![0u8; msg_size];
        if !qm.read(first_off, &mut raw) {
            return Err("command slot unreadable".into());
        }
        let total = u32_at(&raw, ELEMENT_MCTP_PAYLOAD_SIZE) as usize;
        let declared = total.div_ceil(msg_size).max(1) as u64;
        if total > ELEMENT_SIZE_MAX || declared > avail as u64 {
            // As v0's wedge (b): no trustworthy place to resume from.
            return Err(self.fail(format!(
                "bad mctpPayloadSize {total} in command element {index}: {declared} slots declared, {avail} published"
            )));
        }
        let slots = declared as u32;
        for i in 1..slots {
            let mut more = vec![0u8; msg_size];
            if !qm.read(
                Self::slot_off(self.cmd_off, &self.cmd, index.wrapping_add(i)),
                &mut more,
            ) {
                return Err("command slot unreadable".into());
            }
            raw.extend_from_slice(&more);
        }
        let magic = u32_at(&raw, ELEMENT_MAGIC);
        let mctp = u32_at(&raw, ELEMENT_MCTP_HEADER);
        let nvdm = u32_at(&raw, ELEMENT_NVDM_HEADER);
        let nvdm_size = u32_at(&raw, ELEMENT_NVDM_PAYLOAD_SIZE) as usize;
        let nvdm_type = nvdm >> 24;
        let body = &raw[ELEMENT_HEADER_SIZE..];
        // The request this element would answer, if its header is intact enough.
        let (header, gm) = match nvdm_type {
            NVDM_TYPE_GMCAPI => match gmcapi::Header::parse(body) {
                Some(g) => (
                    RpcHeader {
                        header_version: RPC_HEADER_VERSION,
                        signature: RPC_SIGNATURE,
                        length: (RPC_HEADER_SIZE + g.size as usize) as u32,
                        function: g.id(),
                        rpc_result: 0,
                        rpc_result_private: 0,
                        sequence: g.sequence as u32,
                        u: 0,
                    },
                    Some(g),
                ),
                None => (RpcHeader::default(), None),
            },
            _ => (RpcHeader::parse(&body[..RPC_HEADER_SIZE]), None),
        };
        let answerable = match nvdm_type {
            NVDM_TYPE_RM_RPC => {
                header.signature == RPC_SIGNATURE && header.header_version == RPC_HEADER_VERSION
            }
            NVDM_TYPE_GMCAPI => gm.is_some_and(|g| g.id() > gmcapi::COMMAND_RESERVED_LAST),
            _ => false,
        };
        let reject = |status: u32, answerable: bool, why: String| {
            Some(Rejection {
                status,
                answerable,
                reason: format!("command element {index}: {why}"),
            })
        };
        let rejected = if magic == MCTP_MAGIC_SEEN {
            // message_queue_cpu.c:748-772 on the CPU side: skipped, not answered.
            self.stale += 1;
            reject(
                NV_ERR_INVALID_DATA,
                false,
                "stale (magic SEEN), skipped".into(),
            )
        } else if magic != MCTP_MAGIC {
            reject(
                NV_ERR_INVALID_DATA,
                answerable,
                format!("magic {magic:#010x}, not MCTP"),
            )
        } else if mctp & 0xF != MCTP_VERSION {
            reject(
                NV_ERR_INVALID_DATA,
                answerable,
                format!("MCTP header {mctp:#010x}: version {}", mctp & 0xF),
            )
        } else if nvdm & 0x7F != MCTP_MSG_TYPE_VENDOR_PCI
            || (nvdm >> 8) & 0xFFFF != MCTP_VENDOR_ID_NV
        {
            reject(
                NV_ERR_INVALID_DATA,
                answerable,
                format!("NVDM header {nvdm:#010x}: not vendor-PCI 0x10de"),
            )
        } else if total < ELEMENT_HEADER_SIZE || nvdm_size + ELEMENT_HEADER_SIZE != total {
            // Without encryption mctpPayloadSize is the header plus nvdmPayloadSize
            // exactly (message_queue_cpu.c:501-543).
            reject(
                NV_ERR_INVALID_PARAM_STRUCT,
                answerable,
                format!("mctpPayloadSize {total} is not 24 + nvdmPayloadSize {nvdm_size}"),
            )
        } else {
            match nvdm_type {
                NVDM_TYPE_RM_RPC
                    if (header.length as usize) < RPC_HEADER_SIZE
                        || header.length as usize > nvdm_size =>
                {
                    reject(
                        NV_ERR_INVALID_PARAM_STRUCT,
                        answerable,
                        format!("rpc length {} outside [32, {nvdm_size}]", header.length),
                    )
                }
                NVDM_TYPE_RM_RPC if !answerable => reject(
                    NV_ERR_INVALID_DATA,
                    false,
                    format!(
                        "RPC header version {:#010x}, signature {:#010x}",
                        header.header_version, header.signature
                    ),
                ),
                NVDM_TYPE_RM_RPC => None,
                NVDM_TYPE_GMCAPI => match gm {
                    Some(g) if g.is_response() => reject(
                        NV_ERR_INVALID_DATA,
                        false,
                        format!("GMCAPI response {:#x} from the CPU", g.command),
                    ),
                    Some(g) if gmcapi::HEADER_SIZE + g.size as usize > nvdm_size => reject(
                        NV_ERR_INVALID_PARAM_STRUCT,
                        answerable,
                        format!("GMCAPI size {} past nvdmPayloadSize {nvdm_size}", g.size),
                    ),
                    Some(_) => None,
                    None => reject(
                        NV_ERR_INVALID_PARAM_STRUCT,
                        false,
                        "GMCAPI header cut short".into(),
                    ),
                },
                t => reject(NV_ERR_INVALID_DATA, false, format!("NVDM type {t:#x}")),
            }
        };
        if rejected.is_none() {
            // As the CPU does with our elements (message_queue_cpu.c:773-777): a later
            // pointer mistake shows up as a stale element instead of a replay.
            qm.write_u32(first_off + ELEMENT_MAGIC as u64, MCTP_MAGIC_SEEN);
        }
        self.read_ptr = self.read_ptr.wrapping_add(slots);
        fence(Ordering::Release);
        let (payload, slot_payload) = match (&rejected, gm) {
            (Some(_), _) => (Vec::new(), Vec::new()),
            (None, Some(g)) => {
                let d = &body[gmcapi::HEADER_SIZE..];
                (d[..g.size as usize].to_vec(), d.to_vec())
            }
            (None, None) => (
                body[RPC_HEADER_SIZE..header.length as usize].to_vec(),
                body[RPC_HEADER_SIZE..].to_vec(),
            ),
        };
        if let Some(r) = &rejected {
            if magic != MCTP_MAGIC_SEEN {
                self.rejected += 1;
            }
            log::error!("GSP command queue (v2): {}", r.reason);
        } else if header.sequence != 0 {
            // v0's element seqNum is gone; the kernel numbers its synchronous requests
            // from one counter for RPCs and GMCAPI alike (kernel_gsp.c:532-539,
            // gpu.c:7851-7852; asynchronous ones carry 0). A gap is logged, as v0 logged
            // a seqNum mismatch.
            if let Some(last) = self.last_sequence {
                if header.sequence != last.wrapping_add(1)
                    && header.function != crate::ver::common::FN_CONTINUATION_RECORD
                {
                    self.sequence_gaps += 1;
                    log::warn!(
                        "GSP command queue (v2): request sequence {} after {last}",
                        header.sequence
                    );
                }
            }
            self.last_sequence = Some(header.sequence);
        }
        Ok(Some(super::Message {
            seq_num: index,
            header,
            payload,
            slot_payload,
            checksum_ok: rejected.is_none(),
            slots,
            rejected,
            gmcapi: gm,
        }))
    }

    /// What one `NV_PGSP_QUEUE_HEAD(0)` write published when the device did not see its
    /// value: the write came through a KVM ioeventfd without datamatch
    /// (`CARAXES_RPC_IOEVENTFD=1`, docs/caraxes/gsp-transport-615.md §11), which counts the
    /// writes and drops the values.
    ///
    /// The count is exact: the CPU writes the register once per element
    /// (`msgqTxSubmitBuffers`, msgq.c:354-377, called once per `GspMsgQueueSendCommand`
    /// for all its slots, message_queue_cpu.c:551-607), and otherwise only in
    /// `msgqTxCreate` (msgq.c:176), the value 0 of a queue created anew. So write k of the
    /// count published the element at our read pointer, and this decides which of the two
    /// it was without writing anything into the command ring:
    ///
    /// - magic "MCTP" and, for a request with a sequence, a sequence after the last one
    ///   (the one counter of RPC and GMCAPI requests, kernel_gsp.c:532-539, gpu.c:7851):
    ///   a published element of `ceil(mctpPayloadSize / msgSize)` slots, clamped to
    ///   [1, 16] (`queueElementSizeMax`, message_queue_cpu.c:85-86); [`Self::receive`]
    ///   then checks it as for a trapped write and fails the queue on bad framing;
    /// - anything else ("SEEN", which [`Self::receive`] leaves on every element it takes,
    ///   message_queue_cpu.c:773-777 on the CPU side; 0 of a slot never written; a
    ///   sequence at or before the last one, an element already taken): no element was
    ///   published, so the write was `msgqTxCreate`'s 0.
    ///
    /// A partially written element cannot be read here: the CPU copies every slot, then
    /// fences (`portAtomicMemoryFenceStore`, message_queue_cpu.c:597-605, msgq.c:370),
    /// then writes the register; the write exits to KVM, which signals the eventfd from
    /// that exit; the device reads the count after the eventfd is signalled and reads an
    /// element only for a write it counted. It never reads past the counted elements, so
    /// an element the CPU is still copying (the next one after a fire-and-forget request)
    /// is not looked at until its own write is counted.
    pub fn peek_published(&self, qm: &QueueMemory) -> Published {
        if self.failed.is_some() {
            return Published::Failed;
        }
        let msg_size = self.cmd.msg_size as usize;
        let off = Self::slot_off(self.cmd_off, &self.cmd, self.read_ptr);
        let mut raw = vec![0u8; msg_size.max(ELEMENT_HEADER_SIZE + RPC_HEADER_SIZE)];
        if !qm.read(off, &mut raw[..msg_size]) {
            return Published::Stale("command slot unreadable".into());
        }
        let magic = u32_at(&raw, ELEMENT_MAGIC);
        if magic != MCTP_MAGIC {
            return Published::Stale(format!(
                "element {} reads magic {magic:#010x}{}",
                self.read_ptr,
                if magic == MCTP_MAGIC_SEEN { " (SEEN: taken before)" } else { "" }
            ));
        }
        let total = u32_at(&raw, ELEMENT_MCTP_PAYLOAD_SIZE) as usize;
        let slots = total
            .div_ceil(msg_size)
            .clamp(1, ELEMENT_SIZE_MAX / ELEMENT_SIZE_MIN)
            .min(self.cmd.msg_count as usize) as u32;
        let body = &raw[ELEMENT_HEADER_SIZE..];
        let sequence = match u32_at(&raw, ELEMENT_NVDM_HEADER) >> 24 {
            NVDM_TYPE_GMCAPI => gmcapi::Header::parse(body).map(|g| g.sequence as u32),
            NVDM_TYPE_RM_RPC => {
                let h = RpcHeader::parse(&body[..RPC_HEADER_SIZE]);
                (h.signature == RPC_SIGNATURE).then_some(h.sequence)
            }
            _ => None,
        };
        if let (Some(seq), Some(last)) = (sequence, self.last_sequence) {
            if seq != 0 && !super::seq_before(last, seq) {
                return Published::Stale(format!(
                    "element {} carries sequence {seq}, not after the last taken {last}",
                    self.read_ptr
                ));
            }
        }
        Published::Element { slots }
    }

    /// Slots free in the status queue: msgCount minus what the CPU has not consumed
    /// (msgqTxGetFreeSpace, msgq.c:283-323). A CPU read pointer ahead of ours or more
    /// than msgCount behind leaves no room.
    fn free_slots(&self) -> u32 {
        let used = self.write_ptr.wrapping_sub(self.cpu_read_ptr);
        self.stat.msg_count.saturating_sub(used)
    }

    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Append one RM RPC message to the status queue. GMCAPI responses use
    /// [`QueueStateV2::send_gmcapi_response`] so their 64-bit sequence never passes through
    /// the RPC header's 32-bit sequence field. Held while the CPU has not left room, as in
    /// v0. Returns the element's index (the trace's tx seq).
    pub fn send(
        &mut self,
        qm: &QueueMemory,
        header: &RpcHeader,
        payload: &[u8],
    ) -> Result<u32, String> {
        if self.failed.is_some() {
            return Err(self.failed_error());
        }
        if header.function > gmcapi::COMMAND_RESERVED_LAST {
            return Err(format!(
                "GMCAPI response {:#x} requires send_gmcapi_response to preserve its 64-bit sequence",
                header.function
            ));
        }
        let mut h = *header;
        h.length = (RPC_HEADER_SIZE + payload.len()) as u32;
        self.enqueue(
            qm,
            NVDM_TYPE_RM_RPC,
            h.encode().to_vec(),
            payload,
            header.function,
            u64::from(header.sequence),
        )
    }

    /// Send a GMCAPI response using the request's complete 64-bit sequence. GMCAPI has no
    /// continuation-record framing: up to the transport's element maximum is one MCTP/NVDM
    /// element spanning as many queue slots as necessary.
    pub fn send_gmcapi_response(
        &mut self,
        qm: &QueueMemory,
        request: &gmcapi::Header,
        status: u32,
        payload: &[u8],
    ) -> Result<u32, String> {
        if self.failed.is_some() {
            return Err(self.failed_error());
        }
        if request.is_response() || request.id() <= gmcapi::COMMAND_RESERVED_LAST {
            return Err(format!(
                "GMCAPI response needs a CPU request id above {:#x}, got command {:#x}",
                gmcapi::COMMAND_RESERVED_LAST,
                request.command
            ));
        }
        let response =
            gmcapi::Header::response(request.id(), request.sequence, status, payload.len());
        self.enqueue(
            qm,
            NVDM_TYPE_GMCAPI,
            response.encode().to_vec(),
            payload,
            request.id(),
            request.sequence,
        )
    }

    /// Send a GMCAPI *request* of the GSP-to-CPU direction (`GMCAPI_KERNEL` commands such
    /// as `GSP_RESUME_DONE` 0x10005 or `MMU_FAULT_QUEUED` 0x40001): the command id without
    /// `GMCAPI_HEADER_COMMAND_FLAGS_RESPONSE`, sequence 0, `max_response_size` 0 (every
    /// kernel-side command is fire and forget, gmcapi_impl.c:212-215). The kernel's drain
    /// loop hands any GMCAPI element that is not the awaited response to `gmcapiCommand`
    /// (kernel_gsp.c:1989-2020); a response-flagged one would instead match a pending wait
    /// with the same id and sequence (docs/caraxes/gsp-plan/events.md, GMCAPI section).
    pub fn send_gmcapi_request(&mut self, qm: &QueueMemory, id: u32, params: &[u8]) -> Result<u32, String> {
        if self.failed.is_some() {
            return Err(self.failed_error());
        }
        let g = gmcapi::Header {
            command: id & gmcapi::COMMAND_ID_MASK,
            size: params.len() as u32,
            sequence: 0,
            request_or_status: 0,
        };
        self.enqueue(qm, NVDM_TYPE_GMCAPI, g.encode().to_vec(), params, id, 0)
    }

    /// Admit a complete asynchronous RM reply before publishing any of its records.
    /// False means temporary held-capacity pressure; no index or payload was consumed.
    pub(crate) fn send_batch(
        &mut self,
        qm: &QueueMemory,
        records: &[(RpcHeader, Vec<u8>)],
    ) -> Result<bool, String> {
        if self.failed.is_some() {
            return Err(self.failed_error());
        }
        if records.is_empty() || records.len() > 2 {
            return Err("asynchronous RM reply must contain one or two records".into());
        }
        if self.held.len() + records.len() > MAX_HELD {
            return Ok(false);
        }
        let msg_size = self.stat.msg_size as usize;
        let mut prepared = Vec::with_capacity(records.len());
        for (header, payload) in records {
            let total = ELEMENT_HEADER_SIZE + RPC_HEADER_SIZE + payload.len();
            let slots = total.div_ceil(msg_size);
            if header.function > gmcapi::COMMAND_RESERVED_LAST
                || total > ELEMENT_SIZE_MAX
                || slots > self.stat.msg_count as usize
            {
                return Err("asynchronous RM reply does not fit the status queue".into());
            }
            let mut raw = vec![0u8; slots * msg_size];
            put_u32(&mut raw, ELEMENT_MAGIC, MCTP_MAGIC);
            put_u32(&mut raw, ELEMENT_MCTP_PAYLOAD_SIZE, total as u32);
            put_u32(&mut raw, ELEMENT_MCTP_HEADER, MCTP_HEADER);
            put_u32(&mut raw, ELEMENT_NVDM_HEADER, nvdm_header(NVDM_TYPE_RM_RPC));
            put_u32(
                &mut raw,
                ELEMENT_NVDM_PAYLOAD_SIZE,
                (RPC_HEADER_SIZE + payload.len()) as u32,
            );
            let mut header = *header;
            header.length = (RPC_HEADER_SIZE + payload.len()) as u32;
            raw[ELEMENT_HEADER_SIZE..ELEMENT_HEADER_SIZE + RPC_HEADER_SIZE]
                .copy_from_slice(&header.encode());
            raw[ELEMENT_HEADER_SIZE + RPC_HEADER_SIZE..total].copy_from_slice(payload);
            prepared.push(raw);
        }
        self.sent = self.sent.wrapping_add(prepared.len() as u32);
        self.held.extend(prepared);
        self.flush(qm)?;
        Ok(true)
    }

    fn enqueue(
        &mut self,
        qm: &QueueMemory,
        nvdm_type: u32,
        mut body: Vec<u8>,
        payload: &[u8],
        function: u32,
        sequence: u64,
    ) -> Result<u32, String> {
        body.extend_from_slice(payload);
        let total = ELEMENT_HEADER_SIZE + body.len();
        let msg_size = self.stat.msg_size as usize;
        let slots = total.div_ceil(msg_size) as u32;
        if total > ELEMENT_SIZE_MAX || slots > self.stat.msg_count {
            return Err(format!(
                "element of {total} bytes does not fit the status queue"
            ));
        }
        if self.held.len() >= MAX_HELD {
            let e = format!(
                "status queue full: {} elements held, function {:#x} sequence {} not sent",
                self.held.len(),
                function,
                sequence
            );
            log::error!("GSP status queue (v2): {e}");
            return Err(e);
        }
        let mut raw = vec![0u8; slots as usize * msg_size];
        put_u32(&mut raw, ELEMENT_MAGIC, MCTP_MAGIC);
        put_u32(&mut raw, ELEMENT_MCTP_PAYLOAD_SIZE, total as u32);
        put_u32(&mut raw, ELEMENT_MCTP_HEADER, MCTP_HEADER);
        put_u32(&mut raw, ELEMENT_NVDM_HEADER, nvdm_header(nvdm_type));
        put_u32(&mut raw, ELEMENT_NVDM_PAYLOAD_SIZE, body.len() as u32);
        raw[ELEMENT_HEADER_SIZE..total].copy_from_slice(&body);
        let index = self.sent;
        self.sent = self.sent.wrapping_add(1);
        self.held.push_back(raw);
        self.flush(qm)?;
        if !self.held.is_empty() {
            log::warn!(
                "GSP status queue (v2) full: {} element(s) held for the next doorbell (function {:#x} sequence {})",
                self.held.len(),
                function,
                sequence
            );
        }
        Ok(index)
    }

    fn flush(&mut self, qm: &QueueMemory) -> Result<(), String> {
        if self.failed.is_some() {
            return Err(self.failed_error());
        }
        let msg_size = self.stat.msg_size as usize;
        while let Some(raw) = self.held.front() {
            let slots = (raw.len() / msg_size) as u32;
            if slots > self.free_slots() {
                self.held_stalls += 1;
                if self.held_stalls == 2 {
                    log::error!(
                        "GSP status queue (v2): {} element(s) still held after a retry; the CPU is not draining the status queue (MSGQ_TAIL {}, MSGQ_HEAD {})",
                        self.held.len(),
                        self.cpu_read_ptr,
                        self.write_ptr
                    );
                }
                return Ok(());
            }
            for i in 0..slots {
                let chunk = &raw[i as usize * msg_size..(i as usize + 1) * msg_size];
                if !qm.write(
                    Self::slot_off(self.stat_off, &self.stat, self.write_ptr.wrapping_add(i)),
                    chunk,
                ) {
                    return Err(self.fail("status slot unwritable".into()));
                }
            }
            // The slots are visible before the CPU can read the new MSGQ_HEAD (the
            // register read is served after this returns).
            fence(Ordering::Release);
            self.write_ptr = self.write_ptr.wrapping_add(slots);
            self.held.pop_front();
        }
        self.held_stalls = 0;
        Ok(())
    }

    pub fn retry_held(&mut self, qm: &QueueMemory) -> Option<String> {
        if self.held.is_empty() {
            return None;
        }
        let before = self.held.len();
        if let Err(e) = self.flush(qm) {
            return Some(format!("held elements: {e}"));
        }
        let after = self.held.len();
        Some(format!(
            "status queue: {} held element(s) written, {after} still held",
            before - after
        ))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::tests::queue_fixture;
    use super::super::{QueueState, ReplyTo};
    use super::*;

    const CMD: u64 = 0x1000;
    const STAT: u64 = 0x41000;
    const QSIZE: u32 = 0x40000;

    /// The queue arguments of `_getMsgQueueParams`/`kgspPopulateGspRmInitArgs` for the
    /// fixture's 129 pages: page table, 0x40000 command, 0x40000 status.
    pub(crate) fn args() -> QueueArgs {
        QueueArgs {
            args_size: GSP_ARGUMENTS_CACHED_SIZE,
            args_flags: 0,
            flags: 0,
            shared_mem_pa: 0,
            cmd_queue_offset: CMD,
            stat_queue_offset: STAT,
            element_hdr_size: 24,
            element_size_min: 0x1000,
            element_size_max: 0x10000,
            header_align: 4,
            element_align: 12,
            page_table_entry_count: 129,
        }
    }

    /// The CPU's side of the command queue, as msgq.c and message_queue_cpu.c drive it:
    /// the header at msgqTxCreate, elements written slot by slot, the new write pointer
    /// returned (it goes to NV_PGSP_QUEUE_HEAD(0)).
    pub(crate) struct Cpu {
        pub hdr: TxHeaderV2,
        pub write_ptr: u32,
        pub read_ptr: u32,
    }

    impl Cpu {
        pub fn create(qm: &QueueMemory) -> Self {
            let hdr = TxHeaderV2::standard(QSIZE);
            assert!(qm.write(CMD, &hdr.encode()));
            Cpu {
                hdr,
                write_ptr: 0,
                read_ptr: 0,
            }
        }

        /// `GspMsgQueueSendCommand` (message_queue_cpu.c:483-620) for a body of type
        /// `nvdm` (an RPC header plus payload, or a GMCAPI header plus data).
        pub fn send(&mut self, qm: &QueueMemory, nvdm: u32, body: &[u8]) -> Vec<u8> {
            let total = ELEMENT_HEADER_SIZE + body.len();
            let slots = total.div_ceil(0x1000);
            let mut raw = vec![0u8; slots * 0x1000];
            put_u32(&mut raw, 0, MCTP_MAGIC);
            put_u32(&mut raw, 4, total as u32);
            put_u32(&mut raw, 8, MCTP_HEADER);
            put_u32(&mut raw, 12, nvdm_header(nvdm));
            put_u32(&mut raw, 16, body.len() as u32);
            raw[24..total].copy_from_slice(body);
            self.raw(qm, &raw);
            raw
        }

        pub fn raw(&mut self, qm: &QueueMemory, raw: &[u8]) {
            for (i, chunk) in raw.chunks(0x1000).enumerate() {
                let slot = self.write_ptr.wrapping_add(i as u32) % self.hdr.msg_count;
                assert!(qm.write(CMD + 0x1000 + slot as u64 * 0x1000, chunk));
            }
            self.write_ptr = self
                .write_ptr
                .wrapping_add(raw.len().div_ceil(0x1000) as u32);
        }

        /// `GspMsgQueueReceiveStatus` (message_queue_cpu.c:634-822) against our write
        /// pointer: the element's bytes and type, the checks it makes, SEEN written back.
        pub fn receive(&mut self, qm: &QueueMemory, gsp_write_ptr: u32) -> Option<(u32, Vec<u8>)> {
            let avail = gsp_write_ptr.wrapping_sub(self.read_ptr);
            assert!(
                avail <= 63,
                "MSGQ_HEAD {gsp_write_ptr} vs tail {}",
                self.read_ptr
            );
            if avail == 0 {
                return None;
            }
            let at = |p: u32| STAT + 0x1000 + (p % 63) as u64 * 0x1000;
            let mut first = vec![0u8; 0x1000];
            assert!(qm.read(at(self.read_ptr), &mut first));
            let size = u32_at(&first, 4) as usize;
            assert!((24..=0x10000).contains(&size));
            assert_eq!(u32_at(&first, 8) & 0xF, 1, "MCTP version");
            assert_eq!((u32_at(&first, 12) >> 8) & 0xFFFF, 0x10DE, "vendor");
            assert_eq!(u32_at(&first, 0), MCTP_MAGIC);
            let slots = size.div_ceil(0x1000) as u32;
            let mut raw = first;
            for i in 1..slots {
                let mut more = vec![0u8; 0x1000];
                assert!(qm.read(at(self.read_ptr + i), &mut more));
                raw.extend_from_slice(&more);
            }
            assert!(qm.write_u32(at(self.read_ptr), MCTP_MAGIC_SEEN));
            self.read_ptr += slots;
            let nvdm = u32_at(&raw, 12) >> 24;
            assert_eq!(u32_at(&raw, 16) as usize + 24, size);
            Some((nvdm, raw[24..size].to_vec()))
        }
    }

    fn rpc(function: u32, sequence: u32, payload: &[u8]) -> Vec<u8> {
        let h = RpcHeader {
            header_version: RPC_HEADER_VERSION,
            signature: RPC_SIGNATURE,
            length: (32 + payload.len()) as u32,
            function,
            rpc_result: 0xFFFF_FFFF,
            rpc_result_private: 0xFFFF_FFFF,
            sequence,
            u: 0,
        };
        let mut b = h.encode().to_vec();
        b.extend_from_slice(payload);
        b
    }

    fn linked(tag: &str) -> (GuestMemory, Vec<u64>, std::path::PathBuf) {
        let (mem, pages, _base, path) = queue_fixture(tag);
        (mem, pages, path)
    }

    fn large_control_reply(sequence: u32) -> [(RpcHeader, Vec<u8>); 2] {
        // A613's 79,936 parameter bytes plus its 40-byte control frame occupy
        // one maximum-size v2 RPC record and one continuation record.
        let request = RpcHeader {
            sequence,
            ..RpcHeader::event(0x47, 0)
        };
        let first = vec![0xa6; MAX_RPC_SIZE - RPC_HEADER_SIZE];
        let continuation = vec![0x5c; 40 + 79_936 - first.len()];
        assert_eq!((first.len(), continuation.len()), (65_480, 14_496));
        [
            (RpcHeader::reply(&request, 0, first.len()), first),
            (
                RpcHeader {
                    function: crate::rpc::FN_CONTINUATION_RECORD,
                    sequence: sequence.wrapping_add(1),
                    ..RpcHeader::reply(&request, 0, continuation.len())
                },
                continuation,
            ),
        ]
    }

    fn fill_status_and_hold(q: &mut QueueStateV2, qm: &QueueMemory, held: usize) {
        for i in 0..63 + held {
            q.send(qm, &RpcHeader::event(0x1003, 1), &[i as u8])
                .unwrap();
        }
        assert_eq!(
            (q.write_ptr, q.held(), q.sent),
            (63, held, (63 + held) as u32)
        );
    }

    #[test]
    fn async_batch_capacity_refusal_changes_no_state_or_bytes() {
        let (mem, pages, path) = linked("v2batchcapacity");
        let qm = QueueMemory::new(&mem, &pages);
        let _cpu = Cpu::create(&qm);
        let mut q = QueueStateV2::link(&qm, &args(), 0).unwrap();
        let batch = large_control_reply(u32::MAX);

        fill_status_and_hold(&mut q, &qm, 63);
        let state = (q.sent, q.write_ptr, q.held.clone());
        let mut bytes = vec![0; QSIZE as usize];
        assert!(qm.read(STAT, &mut bytes));
        assert_eq!(q.send_batch(&qm, &batch), Ok(false));
        assert_eq!((q.sent, q.write_ptr, q.held.clone()), state);
        let mut unchanged = vec![0; bytes.len()];
        assert!(qm.read(STAT, &mut unchanged));
        assert_eq!(unchanged, bytes, "refused batch wrote no status bytes");
        assert!(q.failed.is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn async_batch_admits_two_records_then_tail_retries_publish_in_order() {
        let (mem, pages, path) = linked("v2batchdrain");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cpu = Cpu::create(&qm);
        let mut q = QueueStateV2::link(&qm, &args(), 0).unwrap();
        let batch = large_control_reply(u32::MAX);
        // One fewer held element leaves exactly two admission slots. The CPU first
        // drains the full ring, then the 62 older held replies; only then can the
        // 16-slot first record and four-slot continuation become visible together.
        fill_status_and_hold(&mut q, &qm, 62);
        assert_eq!(q.send_batch(&qm, &batch), Ok(true));
        assert_eq!((q.sent, q.write_ptr, q.held()), (127, 63, 64));
        for _ in 0..63 {
            cpu.receive(&qm, q.write_ptr).expect("old ring reply");
        }
        q.cpu_read_ptr = cpu.read_ptr;
        assert_eq!(
            q.retry_held(&qm).as_deref(),
            Some("status queue: 62 held element(s) written, 2 still held")
        );
        assert_eq!((q.write_ptr, q.held()), (125, 2));
        for _ in 0..62 {
            cpu.receive(&qm, q.write_ptr).expect("older held reply");
        }
        q.cpu_read_ptr = cpu.read_ptr;
        assert_eq!(
            q.retry_held(&qm).as_deref(),
            Some("status queue: 2 held element(s) written, 0 still held")
        );
        assert_eq!((q.write_ptr, q.held(), q.sent), (145, 0, 127));
        for (index, (expected, payload)) in batch.iter().enumerate() {
            let (nvdm, body) = cpu.receive(&qm, q.write_ptr).expect("batch record");
            assert_eq!(nvdm, NVDM_TYPE_RM_RPC);
            let header = RpcHeader::parse(&body);
            assert_eq!(header.function, expected.function, "record {index}");
            assert_eq!(
                header.sequence, expected.sequence,
                "wrapped sequence {index}"
            );
            assert_eq!(header.length as usize, RPC_HEADER_SIZE + payload.len());
            assert_eq!(&body[RPC_HEADER_SIZE..], payload, "record {index} payload");
        }
        assert_eq!(cpu.read_ptr, q.write_ptr);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn async_batch_validates_later_record_before_any_publication() {
        let (mem, pages, path) = linked("v2batchinvalid");
        let qm = QueueMemory::new(&mem, &pages);
        let _cpu = Cpu::create(&qm);
        let mut q = QueueStateV2::link(&qm, &args(), 0).unwrap();
        let mut batch = large_control_reply(7);
        batch[1].1.resize(MAX_RPC_SIZE - RPC_HEADER_SIZE + 1, 0x5c);
        let mut before = vec![0; QSIZE as usize];
        assert!(qm.read(STAT, &mut before));
        assert!(q
            .send_batch(&qm, &batch)
            .unwrap_err()
            .contains("does not fit"));
        assert_eq!((q.sent, q.write_ptr, q.held()), (0, 0, 0));
        let mut after = vec![0; before.len()];
        assert!(qm.read(STAT, &mut after));
        assert_eq!(
            after, before,
            "even the valid first record was not published"
        );
        assert!(
            q.failed.is_none(),
            "pre-admission validation did not fault the ring"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn async_batch_later_unwritable_slot_is_sticky_and_never_retried() {
        let (mem, pages, path) = linked("v2batchunwritable");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cpu = Cpu::create(&qm);
        let mut q = QueueStateV2::link(&qm, &args(), 0).unwrap();
        let batch = large_control_reply(91);
        let mut broken_pages = pages.clone();
        // The first record fills slots 0..15. Simulate a vanished backing page
        // for the continuation's first slot without changing the valid pages.
        let continuation_page = ((STAT + 0x1000 + 16 * 0x1000) / 0x1000) as usize;
        broken_pages[continuation_page] = 0xdead_0000;
        let broken = QueueMemory::new(&mem, &broken_pages);
        assert!(q
            .send_batch(&broken, &batch)
            .unwrap_err()
            .contains("unwritable"));
        assert_eq!((q.sent, q.write_ptr, q.held()), (2, 16, 1));
        assert!(q
            .failed
            .as_deref()
            .is_some_and(|e| e.contains("unwritable")));
        let (_, first) = cpu
            .receive(&qm, q.write_ptr)
            .expect("first record was published");
        assert_eq!(&first[RPC_HEADER_SIZE..], batch[0].1);
        assert!(
            cpu.receive(&qm, q.write_ptr).is_none(),
            "continuation is invisible"
        );
        let before = (q.sent, q.write_ptr, q.held.clone());
        q.cpu_read_ptr = cpu.read_ptr;
        assert!(q.retry_held(&qm).unwrap().contains("unwritable"));
        assert!(q
            .send_batch(&qm, &batch)
            .unwrap_err()
            .contains("unwritable"));
        assert!(q
            .send(&qm, &RpcHeader::event(0x1003, 0), &[])
            .unwrap_err()
            .contains("unwritable"));
        assert_eq!((q.sent, q.write_ptr, q.held), before);
        assert!(
            cpu.receive(&qm, q.write_ptr).is_none(),
            "faulted retry wrote no continuation"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn link_publishes_the_status_header_msgq_rx_link_accepts() {
        let (mem, pages, path) = linked("v2link");
        let qm = QueueMemory::new(&mem, &pages);
        let _cpu = Cpu::create(&qm);
        let q = QueueStateV2::link(&qm, &args(), 0).unwrap();
        let mut raw = [0u8; 32];
        assert!(qm.read(STAT, &mut raw));
        let stat = TxHeaderV2::parse(&raw);
        // msgqRxLink's checks (msgq.c:240-262): size, msgSize, version 2, msgCount.
        assert_eq!(stat.check(QSIZE, 0x1000), Ok(()));
        assert_eq!(
            (stat.version_major, stat.entry_off, stat.msg_count),
            (2, 0x1000, 63)
        );
        assert_eq!(&raw[20..32], &[0u8; 12], "reserved[3] zero");
        assert_eq!((q.read_ptr, q.write_ptr, q.cmd.msg_count), (0, 0, 63));
        // A v0 header (610 guest) is refused.
        let (mem0, pages0, path0) = linked("v2link0");
        let qm0 = QueueMemory::new(&mem0, &pages0);
        assert!(qm0.write(CMD, &super::super::TxHeader::standard(QSIZE).encode()));
        assert!(QueueStateV2::link(&qm0, &args(), 0)
            .unwrap_err()
            .contains("version 0.0"));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&path0);
    }

    /// An RM RPC request and its reply through both queues, framed as MCTP elements,
    /// with the pointers as the registers carry them.
    #[test]
    fn rpc_round_trip() {
        let (mem, pages, path) = linked("v2rpc");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cpu = Cpu::create(&qm);
        let mut q = QueueStateV2::link(&qm, &args(), cpu.write_ptr).unwrap();
        let payload: Vec<u8> = (0..40u8).collect();
        cpu.send(&qm, NVDM_TYPE_RM_RPC, &rpc(76, 5, &payload));
        assert!(q.receive(&qm).unwrap().is_none(), "no QUEUE_HEAD write yet");
        q.cpu_write_ptr = cpu.write_ptr;
        let m = q.receive(&qm).unwrap().expect("one element");
        assert!(m.rejected.is_none() && m.gmcapi.is_none() && m.checksum_ok);
        assert_eq!(
            (m.header.function, m.header.sequence, m.payload.as_slice()),
            (76, 5, &payload[..])
        );
        assert_eq!((m.seq_num, m.slots, q.read_ptr), (0, 1, 1));
        // The consumed element reads SEEN, as the CPU marks ours.
        assert_eq!(qm.read_u32(CMD + 0x1000), Some(MCTP_MAGIC_SEEN));
        assert!(q.receive(&qm).unwrap().is_none());

        let reply = RpcHeader::reply(&m.header, 0, 8);
        assert_eq!(q.send(&qm, &reply, &[7u8; 8]), Ok(0));
        assert_eq!(q.write_ptr, 1);
        let (nvdm, body) = cpu.receive(&qm, q.write_ptr).expect("reply");
        assert_eq!(nvdm, NVDM_TYPE_RM_RPC);
        let h = RpcHeader::parse(&body);
        assert_eq!(
            (h.function, h.sequence, h.rpc_result, h.length),
            (76, 5, 0, 40)
        );
        assert_eq!(&body[32..], &[7u8; 8]);
        let _ = std::fs::remove_file(&path);
    }

    /// The head write seen without its value (the ioeventfd): each counted write takes the
    /// element at the read pointer, sized by its own mctpPayloadSize; a slot that reads
    /// SEEN, 0 or an old sequence is the create write's 0; nothing is written into the ring
    /// by the peek.
    #[test]
    fn peek_published_decides_without_the_pointer() {
        let (mem, pages, path) = linked("v2peek");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cpu = Cpu::create(&qm);
        let mut q = QueueStateV2::link(&qm, &args(), cpu.write_ptr).unwrap();
        // Nothing written yet: slot 0 reads 0.
        assert!(matches!(q.peek_published(&qm), Published::Stale(ref r) if r.contains("0x00000000")));
        // One element of one slot, then one of three slots (a 9000-byte payload).
        cpu.send(&qm, NVDM_TYPE_RM_RPC, &rpc(76, 7, &[1u8; 40]));
        let mut before = vec![0u8; 0x1000];
        assert!(qm.read(CMD + 0x1000, &mut before));
        assert_eq!(q.peek_published(&qm), Published::Element { slots: 1 });
        let mut after = vec![0u8; 0x1000];
        assert!(qm.read(CMD + 0x1000, &mut after));
        assert_eq!(before, after, "the peek writes nothing");
        q.cpu_write_ptr = q.read_ptr + 1;
        assert_eq!(q.receive(&qm).unwrap().unwrap().header.sequence, 7);
        cpu.send(&qm, NVDM_TYPE_RM_RPC, &rpc(76, 8, &[2u8; 9000]));
        assert_eq!(q.peek_published(&qm), Published::Element { slots: 3 });
        q.cpu_write_ptr = q.read_ptr + 3;
        assert_eq!(q.receive(&qm).unwrap().unwrap().slots, 3);
        assert_eq!(q.read_ptr, 4);
        // Wrap to slot 0 again: the element taken there first reads SEEN.
        q.read_ptr = 63;
        q.cpu_write_ptr = 63;
        assert!(matches!(q.peek_published(&qm), Published::Stale(ref r) if r.contains("SEEN")));
        // An element with MCTP magic but a sequence already taken is stale too.
        let mut cpu2 = Cpu::create(&qm);
        cpu2.write_ptr = 63;
        cpu2.send(&qm, NVDM_TYPE_RM_RPC, &rpc(76, 8, &[3u8; 8]));
        assert!(matches!(q.peek_published(&qm), Published::Stale(ref r) if r.contains("sequence 8")));
        // A fire-and-forget request (sequence 0) counts as published.
        let mut cpu3 = Cpu::create(&qm);
        cpu3.write_ptr = 63;
        cpu3.send(&qm, NVDM_TYPE_RM_RPC, &rpc(76, 0, &[3u8; 8]));
        assert_eq!(q.peek_published(&qm), Published::Element { slots: 1 });
        let _ = std::fs::remove_file(&path);
    }

    /// A GMCAPI request of the GSP-to-CPU direction: NVDM type GMCAPI, the command id
    /// without the response flag, sequence 0, the parameters as data; after an ordinary
    /// reply in the same queue.
    #[test]
    fn unsolicited_gmcapi_request() {
        let (mem, pages, path) = linked("v2gmcreq");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cpu = Cpu::create(&qm);
        let mut q = QueueStateV2::link(&qm, &args(), 0).unwrap();
        assert_eq!(q.send_gmcapi_request(&qm, gmcapi::CMD_GSP_RESUME_DONE, &[0u8; 8]), Ok(0));
        assert_eq!(q.send_gmcapi_request(&qm, 0x0004_0001, &[]), Ok(1));
        let (nvdm, body) = cpu.receive(&qm, q.write_ptr).expect("resume done");
        assert_eq!(nvdm, NVDM_TYPE_GMCAPI);
        let g = gmcapi::Header::parse(&body).unwrap();
        assert!(!g.is_response());
        assert_eq!((g.command, g.sequence, g.size, g.request_or_status), (0x0001_0005, 0, 8, 0));
        assert_eq!(body.len(), gmcapi::HEADER_SIZE + 8);
        let (nvdm, body) = cpu.receive(&qm, q.write_ptr).expect("mmu fault queued");
        let g = gmcapi::Header::parse(&body).unwrap();
        assert_eq!((nvdm, g.command, g.size), (NVDM_TYPE_GMCAPI, 0x0004_0001, 0));
        // Through the version-independent state as well; the 610 transport has none.
        let mut s = QueueState::from_v2(q);
        assert_eq!(s.send_gmcapi_request(&qm, 0x0004_0001, &[]), Ok(2));
        let _ = std::fs::remove_file(&path);
    }

    /// A GMCAPI request surfaces with its command id as the function; the typed reply keeps
    /// its full 64-bit sequence and the largest allowed 48 KiB response remains one GMCAPI
    /// element (spanning queue slots, never RM continuation records).
    #[test]
    fn gmcapi_round_trip() {
        let (mem, pages, path) = linked("v2gmc");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cpu = Cpu::create(&qm);
        let mut q = QueueStateV2::link(&qm, &args(), 0).unwrap();
        let data = [0x11u8; 24];
        let g = gmcapi::Header {
            command: gmcapi::CMD_GSP_INIT,
            size: data.len() as u32,
            sequence: 0x1_0000_0001,
            request_or_status: gmcapi::GSP_INIT_MAX_RESPONSE as u32,
        };
        let mut body = g.encode().to_vec();
        body.extend_from_slice(&data);
        cpu.send(&qm, NVDM_TYPE_GMCAPI, &body);
        q.cpu_write_ptr = cpu.write_ptr;
        let m = q.receive(&qm).unwrap().expect("GSP_INIT");
        assert_eq!(m.gmcapi, Some(g));
        assert_eq!(
            (m.header.function, m.header.sequence),
            (gmcapi::CMD_GSP_INIT, 1)
        );
        assert_eq!(m.payload, data);
        let out = vec![0x22u8; gmcapi::GSP_INIT_MAX_RESPONSE];
        q.send_gmcapi_response(&qm, &g, 0, &out).unwrap();
        assert_eq!(q.sent, 1, "one GMCAPI element, not a continuation series");
        let (nvdm, body) = cpu.receive(&qm, q.write_ptr).expect("response");
        assert_eq!(nvdm, NVDM_TYPE_GMCAPI);
        let r = gmcapi::Header::parse(&body).unwrap();
        assert_eq!(
            (r.command, r.size, r.sequence, r.request_or_status),
            (
                gmcapi::CMD_GSP_INIT | gmcapi::COMMAND_FLAGS_RESPONSE,
                gmcapi::GSP_INIT_MAX_RESPONSE as u32,
                0x1_0000_0001,
                0
            )
        );
        assert_eq!(&body[40..], &out);
        assert!(
            cpu.receive(&qm, q.write_ptr).is_none(),
            "no continuation element"
        );
        let write_ptr = q.write_ptr;
        q.failed = Some("sticky test failure".into());
        assert!(q
            .send_gmcapi_response(&qm, &g, NV_ERR_INVALID_DATA, &[])
            .unwrap_err()
            .contains("sticky test failure"));
        assert_eq!(q.write_ptr, write_ptr, "a failed queue publishes nothing");
        let _ = std::fs::remove_file(&path);
    }

    /// A parsed GMCAPI request behind malformed outer framing is answerable. Its rejection
    /// consumes the request and a typed error response echoes all 64 sequence bits.
    #[test]
    fn malformed_answerable_gmcapi_rejection_preserves_u64_sequence() {
        let (mem, pages, path) = linked("v2gmcrej");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cpu = Cpu::create(&qm);
        let mut q = QueueStateV2::link(&qm, &args(), 0).unwrap();
        let g = gmcapi::Header {
            command: gmcapi::CMD_GSP_INIT,
            size: 0,
            sequence: 0x1_0000_0001,
            request_or_status: gmcapi::GSP_INIT_MAX_RESPONSE as u32,
        };
        let mut bad = cpu_element(NVDM_TYPE_GMCAPI, &g.encode());
        put_u32(&mut bad, ELEMENT_MAGIC, 0xFFFF_FFFF);
        cpu.raw(&qm, &bad);
        q.cpu_write_ptr = cpu.write_ptr;

        let m = q.receive(&qm).unwrap().expect("malformed GMCAPI element");
        assert_eq!(m.gmcapi, Some(g));
        assert_eq!(m.reply_to(), Some(ReplyTo::Itself));
        assert_eq!(
            m.rejected.as_ref().map(|r| r.status),
            Some(NV_ERR_INVALID_DATA)
        );
        q.send_gmcapi_response(&qm, &m.gmcapi.unwrap(), NV_ERR_INVALID_DATA, &[])
            .unwrap();
        let (nvdm, body) = cpu.receive(&qm, q.write_ptr).expect("error response");
        let response = gmcapi::Header::parse(&body).unwrap();
        assert_eq!(nvdm, NVDM_TYPE_GMCAPI);
        assert_eq!(
            (response.id(), response.sequence, response.request_or_status),
            (gmcapi::CMD_GSP_INIT, 0x1_0000_0001, NV_ERR_INVALID_DATA)
        );
        assert!(response.is_response());
        assert_eq!((q.rejected, q.failed.as_deref()), (1, None));
        let _ = std::fs::remove_file(&path);
    }

    /// A length-valid RM RPC with a foreign header schema must not reach dispatch; after
    /// consuming bad signature and version elements, the same v2 queue receives the next
    /// valid request normally.
    #[test]
    fn invalid_rpc_header_is_rejected_and_queue_recovers() {
        let (mem, pages, path) = linked("v2rpcheader");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cpu = Cpu::create(&qm);
        let mut q = QueueStateV2::link(&qm, &args(), 0).unwrap();
        let mut signature = rpc(70, 1, &[1]);
        put_u32(&mut signature, 4, RPC_SIGNATURE ^ 1);
        cpu.send(&qm, NVDM_TYPE_RM_RPC, &signature);
        let mut version = rpc(71, 2, &[2]);
        put_u32(&mut version, 0, RPC_HEADER_VERSION ^ 1);
        cpu.send(&qm, NVDM_TYPE_RM_RPC, &version);
        cpu.send(&qm, NVDM_TYPE_RM_RPC, &rpc(72, 3, &[3]));
        q.cpu_write_ptr = cpu.write_ptr;

        for field in ["signature", "version"] {
            let m = q.receive(&qm).unwrap().expect("foreign RPC schema");
            let r = m.rejected.as_ref().expect("rejected");
            assert_eq!((r.status, r.answerable), (NV_ERR_INVALID_DATA, false));
            assert!(r.reason.contains(field), "{}", r.reason);
            assert_eq!(m.reply_to(), Some(ReplyTo::Nobody));
        }
        let m = q.receive(&qm).unwrap().expect("following valid RPC");
        assert_eq!((m.header.function, m.payload.as_slice()), (72, &[3u8][..]));
        assert!(m.rejected.is_none());
        assert_eq!((q.rejected, q.failed.as_deref(), q.read_ptr), (2, None, 3));
        let _ = std::fs::remove_file(&path);
    }

    /// A 5000-byte request spans two slots and wraps at the ring end; monotonic pointers
    /// count across the wrap, and the status queue may fill all 63 slots.
    #[test]
    fn pointers_are_monotonic_and_wrap() {
        let (mem, pages, path) = linked("v2wrap");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cpu = Cpu::create(&qm);
        let mut q = QueueStateV2::link(&qm, &args(), 0).unwrap();
        cpu.write_ptr = 62;
        q.read_ptr = 62;
        let big: Vec<u8> = (0..5000u32).map(|i| i as u8).collect();
        cpu.send(&qm, NVDM_TYPE_RM_RPC, &rpc(103, 9, &big));
        q.cpu_write_ptr = cpu.write_ptr;
        assert_eq!(cpu.write_ptr, 64, "monotonic, not modulo");
        let m = q.receive(&qm).unwrap().expect("wrapped element");
        assert_eq!((m.slots, m.payload.len(), q.read_ptr), (2, 5000, 64));
        assert_eq!(m.payload, big);
        // Status side: 63 one-slot elements fit while the CPU reads nothing.
        for i in 0..63 {
            assert_eq!(q.send(&qm, &RpcHeader::event(0x1003, 4), &[0; 4]), Ok(i));
        }
        assert_eq!((q.write_ptr, q.held()), (63, 0));
        q.send(&qm, &RpcHeader::event(0x1003, 4), &[1; 4]).unwrap();
        assert_eq!((q.write_ptr, q.held()), (63, 1));
        for _ in 0..2 {
            cpu.receive(&qm, q.write_ptr).unwrap();
        }
        q.cpu_read_ptr = cpu.read_ptr;
        assert_eq!(
            q.retry_held(&qm).as_deref(),
            Some("status queue: 1 held element(s) written, 0 still held")
        );
        assert_eq!(q.write_ptr, 64);
        // A head more than msgCount ahead of our tail fails the queue.
        q.cpu_write_ptr = q.read_ptr.wrapping_add(64);
        assert!(q.receive(&qm).unwrap_err().contains("ahead of our tail"));
        let _ = std::fs::remove_file(&path);
    }

    /// The receive-side checks that replace v0's checksum: bad magic, MCTP version and
    /// vendor are rejected with NV_ERR_INVALID_DATA and answered when the RPC header is
    /// intact; a SEEN element is stale and skipped without an answer; a size mismatch
    /// is NV_ERR_INVALID_PARAM_STRUCT; an intact element after them is received.
    #[test]
    fn framing_checks_replace_the_checksum() {
        let (mem, pages, path) = linked("v2chk");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cpu = Cpu::create(&qm);
        let mut q = QueueStateV2::link(&qm, &args(), 0).unwrap();
        let mut bad_magic = cpu_element(NVDM_TYPE_RM_RPC, &rpc(76, 1, &[1]));
        put_u32(&mut bad_magic, 0, 0xFFFF_FFFF);
        cpu.raw(&qm, &bad_magic);
        let mut seen = cpu_element(NVDM_TYPE_RM_RPC, &rpc(76, 2, &[1]));
        put_u32(&mut seen, 0, MCTP_MAGIC_SEEN);
        cpu.raw(&qm, &seen);
        let mut version = cpu_element(NVDM_TYPE_RM_RPC, &rpc(76, 3, &[1]));
        put_u32(&mut version, 8, 0xC000_0002);
        cpu.raw(&qm, &version);
        let mut vendor = cpu_element(NVDM_TYPE_RM_RPC, &rpc(76, 4, &[1]));
        put_u32(&mut vendor, 12, 0x2510_DE7E ^ 0x100);
        cpu.raw(&qm, &vendor);
        let mut size = cpu_element(NVDM_TYPE_RM_RPC, &rpc(76, 5, &[1]));
        put_u32(&mut size, 16, 99);
        cpu.raw(&qm, &size);
        cpu.send(&qm, NVDM_TYPE_RM_RPC, &rpc(76, 6, &[1, 2]));
        q.cpu_write_ptr = cpu.write_ptr;
        let expect = [
            (NV_ERR_INVALID_DATA, ReplyTo::Itself),
            (NV_ERR_INVALID_DATA, ReplyTo::Nobody),
            (NV_ERR_INVALID_DATA, ReplyTo::Itself),
            (NV_ERR_INVALID_DATA, ReplyTo::Itself),
            (NV_ERR_INVALID_PARAM_STRUCT, ReplyTo::Itself),
        ];
        for (i, (status, to)) in expect.into_iter().enumerate() {
            let m = q.receive(&qm).unwrap().expect("element");
            assert_eq!(
                m.rejected.as_ref().map(|r| r.status),
                Some(status),
                "element {i}"
            );
            assert_eq!(m.reply_to(), Some(to), "element {i}");
        }
        let m = q.receive(&qm).unwrap().expect("intact");
        assert!(m.rejected.is_none());
        assert_eq!(
            (m.header.sequence, m.payload.as_slice()),
            (6, &[1u8, 2][..])
        );
        assert_eq!((q.rejected, q.stale, q.failed.as_deref()), (4, 1, None));
        let _ = std::fs::remove_file(&path);
    }

    fn cpu_element(nvdm: u32, body: &[u8]) -> Vec<u8> {
        let total = 24 + body.len();
        let mut raw = vec![0u8; total.div_ceil(0x1000) * 0x1000];
        put_u32(&mut raw, 0, MCTP_MAGIC);
        put_u32(&mut raw, 4, total as u32);
        put_u32(&mut raw, 8, MCTP_HEADER);
        put_u32(&mut raw, 12, nvdm_header(nvdm));
        put_u32(&mut raw, 16, body.len() as u32);
        raw[24..total].copy_from_slice(body);
        raw
    }

    /// The v2 state behind `QueueState`, the interface boot.rs uses: receive, send,
    /// held and retry reach the v2 implementation.
    #[test]
    fn queue_state_delegates_to_v2() {
        let (mem, pages, path) = linked("v2qs");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cpu = Cpu::create(&qm);
        let mut qs = QueueState::from_v2(QueueStateV2::link(&qm, &args(), 0).unwrap());
        cpu.send(&qm, NVDM_TYPE_RM_RPC, &rpc(10, 1, &[0u8; 16]));
        qs.v2_mut().unwrap().cpu_write_ptr = cpu.write_ptr;
        let m = qs.receive(&qm).unwrap().expect("FREE");
        assert_eq!(m.header.function, 10);
        qs.send(&qm, &RpcHeader::reply(&m.header, 0, 16), &m.payload)
            .unwrap();
        assert_eq!(qs.v2().unwrap().write_ptr, 1);
        assert_eq!(qs.held(), 0);
        assert_eq!(qs.retry_held(&qm), None);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn element_constants_are_the_headers() {
        assert_eq!(nvdm_header(NVDM_TYPE_RM_RPC), 0x2510_DE7E);
        assert_eq!(nvdm_header(NVDM_TYPE_GMCAPI), 0x2610_DE7E);
        assert_eq!(MAX_RPC_SIZE, 65512);
        assert_eq!(ARGS_MQ_PAGE_TABLE_ENTRY_COUNT, 68);
        assert_eq!(TxHeaderV2::standard(QSIZE).msg_count, 63);
    }
}

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The GSP message queues in guest RAM, from the GSP side
//! (docs/caraxes/gsp-rpc-protocol.md §1-2, docs/caraxes/gsp-abi-610.md (a) and (d)).
//!
//! The shared buffer is 0x81000 bytes of non-contiguous guest pages; page 0 holds
//! the page table. The CPU's command queue starts at `cmdQueueOffset`, our status
//! queue at `statQueueOffset`; each has a 32-byte `msgqTxHeader`, a 4-byte
//! `msgqRxHeader` at `rxHdrOff` and 4 KiB slots from `entryOff`. With `SWAP_RX`
//! set on both headers, our read pointer for the command queue lives in our
//! queue's rx header and the CPU's read pointer for our queue lives in its.
//!
//! Memory ordering follows the transaction discipline of the guest's
//! `message_queue_cpu.c` (store fence before `msgqTxSubmitBuffers`, :599-610) and of
//! upstream nova-core (docs/caraxes/nova-vgpu-upstream.md §5): the four pointer words
//! are accessed as 32-bit atomics in the shared mapping. A producer's pointer is loaded
//! with `Acquire` before any slot it covers is read (the payload is visible once the
//! pointer is), and our own pointers are stored with `Release` after the slots are
//! written or copied out (the payload is visible before the pointer, and a slot is not
//! handed back before we are done with it).

use std::sync::atomic::{AtomicU32, Ordering};

use crate::guestmem::GuestMemory;

/// Message queue v2, the transport from driver 615 on (docs/caraxes/gsp-transport-615.md):
/// a second implementation behind [`QueueState`], selected when the boot model links
/// the queues of a 615 guest ([`QueueState::from_v2`]).
pub mod v2;

/// The element and RPC header constants of message queue v0, the 610 queue: taken from
/// `ver::v610`, whatever guest ABI is in force (a 615 guest never runs this transport;
/// its element header is `msgq::v2`'s).
pub use crate::ver::v610::{
    ELEMENT_HEADER_SIZE, MCTP_HEADER, MSGQ_FLAGS_SWAP_RX, NVDM_HEADER, RPC_HEADER_SIZE,
    RPC_HEADER_VERSION, RPC_SIGNATURE,
};

/// The element layout of the v0 path: the pre-610 one for a 570 or 595 guest, else the 610
/// element (16-byte header). `ver::element_layout()` alone would give the 24-byte v2
/// header while a 615 guest is in force and misread every v0 element.
fn v0_element_layout() -> crate::ver::ElementLayout {
    use crate::ver::ElementLayout;
    match crate::ver::element_layout() {
        ElementLayout::PRE_610 => ElementLayout::PRE_610,
        _ => ElementLayout::V610,
    }
}

const PAGE: u64 = 4096;

/// `_checkSum32`: XOR of the little-endian qwords, high half folded into the low.
/// `data` is padded with zeros to a multiple of 8, as the sender does.
pub fn checksum32(data: &[u8]) -> u32 {
    let mut acc = 0u64;
    for chunk in data.chunks(8) {
        let mut b = [0u8; 8];
        b[..chunk.len()].copy_from_slice(chunk);
        acc ^= u64::from_le_bytes(b);
    }
    ((acc >> 32) as u32) ^ (acc as u32)
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

fn put_u32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// The shared queue buffer, addressed by logical offset through its page table.
pub struct QueueMemory<'a> {
    mem: &'a GuestMemory,
    pages: &'a [u64],
    /// Acquire loads and release stores of pointer words (the ordering tests).
    #[cfg(test)]
    ordered: std::cell::RefCell<Vec<(char, u64)>>,
}

impl<'a> QueueMemory<'a> {
    pub fn new(mem: &'a GuestMemory, pages: &'a [u64]) -> Self {
        QueueMemory {
            mem,
            pages,
            #[cfg(test)]
            ordered: Default::default(),
        }
    }

    /// The host address of the aligned 32-bit word at logical offset `off`.
    fn word(&self, off: u64) -> Option<&AtomicU32> {
        if off % 4 != 0 {
            return None;
        }
        let pa = self.pages.get((off / PAGE) as usize)? + off % PAGE;
        let gpa = self.mem.translate(pa, 4)?;
        let p = self.mem.host_ptr(gpa, 4)?;
        if (p as usize) % 4 != 0 {
            return None;
        }
        // SAFETY: host_ptr() guarantees four mapped bytes inside one mapping, the
        // pointer is 4-aligned, and the mapping lives as long as `self.mem`. The word is
        // shared with the guest, which is why it is only ever accessed atomically here.
        Some(unsafe { AtomicU32::from_ptr(p.cast()) })
    }

    /// Load a pointer word the other side publishes, with acquire ordering: every
    /// later read of the queue sees at least what the writer stored before it.
    pub fn load_acquire(&self, off: u64) -> Option<u32> {
        let v = u32::from_le(self.word(off)?.load(Ordering::Acquire));
        #[cfg(test)]
        self.ordered.borrow_mut().push(('A', off));
        Some(v)
    }

    /// Publish one of our pointer words with release ordering: every earlier read
    /// or write of the queue happens before the other side can see the new value.
    pub fn store_release(&self, off: u64, v: u32) -> bool {
        let Some(w) = self.word(off) else {
            return false;
        };
        w.store(v.to_le(), Ordering::Release);
        #[cfg(test)]
        self.ordered.borrow_mut().push(('R', off));
        true
    }

    /// Read the page table itself: `count` physical page addresses at `shared_pa`.
    pub fn read_page_table(mem: &GuestMemory, shared_pa: u64, count: u32) -> Option<Vec<u64>> {
        (0..count as u64)
            .map(|i| mem.read_u64(shared_pa + 8 * i))
            .collect()
    }

    pub fn read(&self, off: u64, buf: &mut [u8]) -> bool {
        let mut done = 0usize;
        while done < buf.len() {
            let o = off + done as u64;
            let Some(&pa) = self.pages.get((o / PAGE) as usize) else {
                return false;
            };
            let in_page = (o % PAGE) as usize;
            let n = (PAGE as usize - in_page).min(buf.len() - done);
            if !self.mem.read(pa + in_page as u64, &mut buf[done..done + n]) {
                return false;
            }
            done += n;
        }
        true
    }

    pub fn write(&self, off: u64, data: &[u8]) -> bool {
        let mut done = 0usize;
        while done < data.len() {
            let o = off + done as u64;
            let Some(&pa) = self.pages.get((o / PAGE) as usize) else {
                return false;
            };
            let in_page = (o % PAGE) as usize;
            let n = (PAGE as usize - in_page).min(data.len() - done);
            if !self.mem.write(pa + in_page as u64, &data[done..done + n]) {
                return false;
            }
            done += n;
        }
        true
    }

    pub fn read_u32(&self, off: u64) -> Option<u32> {
        let mut b = [0u8; 4];
        self.read(off, &mut b).then(|| u32::from_le_bytes(b))
    }

    pub fn write_u32(&self, off: u64, v: u32) -> bool {
        self.write(off, &v.to_le_bytes())
    }
}

/// `msgqTxHeader`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TxHeader {
    pub version: u32,
    pub size: u32,
    pub msg_size: u32,
    pub msg_count: u32,
    pub write_ptr: u32,
    pub flags: u32,
    pub rx_hdr_off: u32,
    pub entry_off: u32,
}

impl TxHeader {
    pub fn parse(b: &[u8; 32]) -> Self {
        TxHeader {
            version: u32_at(b, 0),
            size: u32_at(b, 4),
            msg_size: u32_at(b, 8),
            msg_count: u32_at(b, 12),
            write_ptr: u32_at(b, 16),
            flags: u32_at(b, 20),
            rx_hdr_off: u32_at(b, 24),
            entry_off: u32_at(b, 28),
        }
    }

    pub fn encode(&self) -> [u8; 32] {
        let mut b = [0u8; 32];
        for (i, v) in [
            self.version,
            self.size,
            self.msg_size,
            self.msg_count,
            self.write_ptr,
            self.flags,
            self.rx_hdr_off,
            self.entry_off,
        ]
        .iter()
        .enumerate()
        {
            put_u32(&mut b, 4 * i, *v);
        }
        b
    }

    /// The header `msgqTxCreate` builds for a 0x40000-byte queue of 4 KiB slots.
    pub fn standard(size: u32) -> Self {
        let msg_size = 0x1000;
        let entry_off = 0x1000;
        TxHeader {
            version: 0,
            size,
            msg_size,
            msg_count: (size - entry_off) / msg_size,
            write_ptr: 0,
            flags: MSGQ_FLAGS_SWAP_RX,
            rx_hdr_off: 0x20,
            entry_off,
        }
    }
}

/// `rpc_message_header_v`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RpcHeader {
    pub header_version: u32,
    pub signature: u32,
    pub length: u32,
    pub function: u32,
    pub rpc_result: u32,
    pub rpc_result_private: u32,
    pub sequence: u32,
    pub u: u32,
}

impl RpcHeader {
    pub fn parse(b: &[u8]) -> Self {
        RpcHeader {
            header_version: u32_at(b, 0),
            signature: u32_at(b, 4),
            length: u32_at(b, 8),
            function: u32_at(b, 12),
            rpc_result: u32_at(b, 16),
            rpc_result_private: u32_at(b, 20),
            sequence: u32_at(b, 24),
            u: u32_at(b, 28),
        }
    }

    pub fn encode(&self) -> [u8; 32] {
        let mut b = [0u8; 32];
        for (i, v) in [
            self.header_version,
            self.signature,
            self.length,
            self.function,
            self.rpc_result,
            self.rpc_result_private,
            self.sequence,
            self.u,
        ]
        .iter()
        .enumerate()
        {
            put_u32(&mut b, 4 * i, *v);
        }
        b
    }

    /// A GSP-originated message (event) header.
    pub fn event(function: u32, payload_len: usize) -> Self {
        RpcHeader {
            header_version: RPC_HEADER_VERSION,
            signature: RPC_SIGNATURE,
            length: (RPC_HEADER_SIZE + payload_len) as u32,
            function,
            rpc_result: 0,
            rpc_result_private: 0,
            sequence: 0,
            u: 0,
        }
    }

    /// The reply header for a request: same function and sequence, result set.
    pub fn reply(request: &RpcHeader, rpc_result: u32, payload_len: usize) -> Self {
        RpcHeader {
            length: (RPC_HEADER_SIZE + payload_len) as u32,
            rpc_result,
            rpc_result_private: 0,
            ..*request
        }
    }
}

/// `NV_ERR_INVALID_DATA` (nvstatuscodes.h:66): the status of a rejected element whose
/// checksum does not fold to zero, the one the guest's own receive path returns for the
/// same condition (message_queue_cpu.c:730-735).
pub const NV_ERR_INVALID_DATA: u32 = 0x25;
/// `NV_ERR_INVALID_PARAM_STRUCT` (nvstatuscodes.h:87): the status of a rejected element
/// whose RPC length is shorter than the RPC header, the one the guest's send path
/// returns for a bad record length (message_queue_cpu.c:491-496).
pub const NV_ERR_INVALID_PARAM_STRUCT: u32 = 0x3A;

/// `queueElementSizeMax`: the largest element either side sends, element header
/// included (message_queue_cpu.c:89, 16 pages).
pub const MAX_ELEMENT_SIZE: usize = 0x10000;

/// Elements the status queue holds back while the CPU does not drain it, before
/// `send` refuses (with an error-level log) instead of holding more.
pub const MAX_HELD: usize = 64;

/// Whether counter `a` comes before counter `b` on a 32-bit wrapping counter.
///
/// The wrap rule: `a` is before `b` when `b - a`, computed modulo 2^32, lies in
/// `1..2^31`. A counter therefore compares correctly across the wrap from
/// `0xFFFF_FFFF` to `0` as long as the two values are less than 2^31 apart, which the
/// element sequence numbers of one queue always are. Equal values are not before each
/// other. The same rule is upstream's semaphore test (`value.wrapping_sub(target) <
/// 0x8000_0000`, nova-vgpu-upstream.md §4 item 4).
pub fn seq_before(a: u32, b: u32) -> bool {
    let d = b.wrapping_sub(a);
    d != 0 && d < 0x8000_0000
}

/// Slots from ring position `from` forward to `to` on a ring of `n` slots, both
/// positions already checked to be below `n`. Ring pointers are compared only through
/// this distance, never with `<` or `>`, so a pointer that wrapped past slot 0 is
/// still ahead of one that did not.
pub fn ring_distance(from: u32, to: u32, n: u32) -> u32 {
    debug_assert!(from < n && to < n);
    ((to as u64 + n as u64 - from as u64) % n as u64) as u32
}

/// Why an element was consumed without being dispatched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejection {
    /// The `rpc_result` of the error reply.
    pub status: u32,
    /// Whether the request header is intact enough that its function/command and
    /// sequence can be echoed in an error reply.
    pub answerable: bool,
    pub reason: String,
}

/// One message received from the command queue.
#[derive(Debug)]
pub struct Message {
    pub seq_num: u32,
    pub header: RpcHeader,
    pub payload: Vec<u8>,
    /// The whole slot contents after the RPC header. `GSP_RM_ALLOC` counts only its
    /// fixed part in `length` and leaves the allocation parameters behind it in the
    /// same slot (rpc.c:11002-11025; the slot is copied whole, message_queue_cpu.c:503).
    pub slot_payload: Vec<u8>,
    pub checksum_ok: bool,
    pub slots: u32,
    /// Set when the element was consumed but must not be dispatched: the caller
    /// answers it with [`Rejection::status`] when it is answerable, and never acts on
    /// its payload.
    pub rejected: Option<Rejection>,
    /// The `GMCAPI_HEADER` of a GMCAPI request (message queue v2 only). The synthetic
    /// RPC header's `function` holds its command id for dispatch, but this original
    /// header remains the reply authority because its sequence is 64-bit. GMCAPI ids
    /// start above 0xFFFF so that the two share one namespace
    /// (interface/gmcapi/gmcapi_base.h:76-80 of 615); replies use
    /// [`v2::QueueStateV2::send_gmcapi_response`].
    pub gmcapi: Option<crate::gmcapi::Header>,
}

/// Which request the error reply for a rejected element answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyTo {
    /// The element's own function and sequence.
    Itself,
    /// The request being assembled from continuation records: a torn continuation
    /// record ends it, and the guest waits for the first record's function and
    /// sequence (rpc.c:2156-2158, `_issueRpcAndWaitLarge`).
    PendingRequest,
    /// Nobody: the RPC header is not intact, so function and sequence are unknown and
    /// the guest's RPC runs into its own timeout.
    Nobody,
}

impl Message {
    /// For a rejected element, which request its error reply answers (`None` for an
    /// element that was not rejected).
    ///
    /// An RM reply is a bare RPC header: the request's function and sequence with
    /// `rpc_result` set to [`Rejection::status`] and no payload. A GMCAPI reply is its
    /// response header with the original 64-bit sequence and `status` set likewise.
    /// The guest handles an RM error as a failed RPC: `_issueRpcAndWait` returns any `rpc_result` below the VMIOP
    /// range (0xFF000000, rpc_headers.h:122) as the call's status (rpc.c:2011-2027,
    /// large requests rpc.c:2230-2241), and `rpcRmApiControl_GSP` copies no
    /// parameters back when that status is not `NV_OK` (rpc.c:10868-10899). The
    /// status mirrors the guest's own verdict on such an element: a bad checksum is
    /// `NV_ERR_INVALID_DATA` when the guest receives one (message_queue_cpu.c:709-735,
    /// after three re-reads for data still in flight, which cannot apply here since
    /// the doorbell follows the guest's store fence), a bad record length is
    /// `NV_ERR_INVALID_PARAM_STRUCT` (:491-496). The guest itself produces bad
    /// checksums on purpose with `bErrorInjectionEnabled` (:548-552).
    pub fn reply_to(&self) -> Option<ReplyTo> {
        let rej = self.rejected.as_ref()?;
        Some(if !rej.answerable {
            ReplyTo::Nobody
        } else if self.header.function == crate::ver::v610::FN_CONTINUATION_RECORD {
            ReplyTo::PendingRequest
        } else {
            ReplyTo::Itself
        })
    }
}

/// The GSP side of both queues.
#[derive(Debug)]
pub struct QueueState {
    pub cmd_off: u64,
    pub stat_off: u64,
    pub cmd: TxHeader,
    pub stat: TxHeader,
    /// Our read pointer into the command queue.
    pub read_ptr: u32,
    /// Element sequence number of our next message.
    pub tx_seq: u32,
    /// Element sequence number we expect next from the CPU.
    pub rx_seq: u32,
    /// Set once the queue is unusable; every later `receive` and `send` fails with
    /// it. A guest reset or driver reload recovers: `BootModel::reset` drops this
    /// state and the next doorbell links fresh queues.
    pub failed: Option<String>,
    /// Elements consumed without dispatch (bad checksum, bad length).
    pub rejected: u64,
    /// Elements whose `seqNum` was not the expected one (logged, still dispatched).
    pub seq_mismatches: u64,
    /// Complete status-queue elements (sequence and checksum final) waiting for room,
    /// in order. Nothing is written past them, so the CPU sees our elements in
    /// sequence.
    held: std::collections::VecDeque<Vec<u8>>,
    /// Flushes in a row that left held elements behind (1 = the first attempt).
    held_stalls: u32,
    /// The message queue v2 state when the guest speaks the 615 transport: every
    /// method below delegates to it, and the v0 fields above stay unused.
    v2: Option<Box<v2::QueueStateV2>>,
}

impl QueueState {
    /// Validate the CPU's command-queue header and publish ours in the status queue.
    pub fn link(
        qm: &QueueMemory,
        cmd_off: u64,
        stat_off: u64,
        stat_size: u32,
    ) -> Result<Self, String> {
        let mut raw = [0u8; 32];
        if !qm.read(cmd_off, &mut raw) {
            return Err(format!("command queue header at {cmd_off:#x} not readable"));
        }
        let cmd = TxHeader::parse(&raw);
        if cmd.version != 0
            || cmd.msg_size != 0x1000
            || cmd.msg_count == 0
            || cmd.entry_off < cmd.rx_hdr_off + 4
        {
            return Err(format!("command queue header rejected: {cmd:?}"));
        }
        let stat = TxHeader::standard(stat_size);
        if !qm.write(stat_off, &stat.encode())
            || !qm.write_u32(stat_off + stat.rx_hdr_off as u64, 0)
        {
            return Err(format!("status queue header at {stat_off:#x} not writable"));
        }
        Ok(QueueState {
            cmd_off,
            stat_off,
            cmd,
            stat,
            read_ptr: 0,
            tx_seq: 0,
            rx_seq: 0,
            failed: None,
            rejected: 0,
            seq_mismatches: 0,
            held: Default::default(),
            held_stalls: 0,
            v2: None,
        })
    }

    /// The queues of a 615 guest: the v2 state behind this interface. The v0 fields
    /// describe no queue and are not used.
    pub fn from_v2(q: v2::QueueStateV2) -> Self {
        let none = TxHeader {
            version: 0,
            size: 0,
            msg_size: 0,
            msg_count: 0,
            write_ptr: 0,
            flags: 0,
            rx_hdr_off: 0,
            entry_off: 0,
        };
        QueueState {
            cmd_off: q.cmd_off,
            stat_off: q.stat_off,
            cmd: none,
            stat: none,
            read_ptr: 0,
            tx_seq: 0,
            rx_seq: 0,
            failed: None,
            rejected: 0,
            seq_mismatches: 0,
            held: Default::default(),
            held_stalls: 0,
            v2: Some(Box::new(q)),
        }
    }

    /// The first 0x40 bytes of the next unread command element (message queue v0; `None`
    /// when the queue is empty or unreadable, and for v2): what the CPU queued before its
    /// doorbell, for the frame check at the link (`ver::element_frame`).
    pub fn peek_first(&self, qm: &QueueMemory) -> Option<Vec<u8>> {
        if self.v2.is_some() || self.cmd.msg_count == 0 {
            return None;
        }
        let write_ptr = qm.load_acquire(self.cmd_off + 16)?;
        if write_ptr >= self.cmd.msg_count || write_ptr == self.read_ptr {
            return None;
        }
        let mut raw = vec![0u8; 0x40];
        qm.read(Self::slot_off(self.cmd_off, &self.cmd, self.read_ptr), &mut raw)
            .then_some(raw)
    }

    /// The v2 state, when these are the queues of a 615 guest.
    pub fn v2(&self) -> Option<&v2::QueueStateV2> {
        self.v2.as_deref()
    }

    pub fn v2_mut(&mut self) -> Option<&mut v2::QueueStateV2> {
        self.v2.as_deref_mut()
    }

    fn slot_off(base: u64, hdr: &TxHeader, slot: u32) -> u64 {
        base + hdr.entry_off as u64 + (slot % hdr.msg_count) as u64 * hdr.msg_size as u64
    }

    /// Declare the queue dead: log once at error level, keep the reason for every
    /// later call.
    fn fail(&mut self, reason: String) -> String {
        if self.failed.is_none() {
            log::error!(
                "GSP message queue failed: {reason}; no further RPCs are serviced until the guest resets the device"
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

    /// Consume `slots` elements: advance and publish our read pointer (release: the
    /// copies of the slots are complete before the CPU may reuse them).
    fn consume(&mut self, qm: &QueueMemory, slots: u32) {
        self.read_ptr = (self.read_ptr + slots) % self.cmd.msg_count;
        qm.store_release(self.stat_off + self.stat.rx_hdr_off as u64, self.read_ptr);
    }

    /// Take the next message from the command queue, if any.
    ///
    /// A message with [`Message::rejected`] set was consumed but must not be
    /// dispatched. `Err` is either transient (queue memory unreadable) or, once
    /// [`QueueState::failed`] is set, final.
    pub fn receive(&mut self, qm: &QueueMemory) -> Result<Option<Message>, String> {
        if let Some(q) = self.v2.as_deref_mut() {
            return q.receive(qm);
        }
        if self.failed.is_some() {
            return Err(self.failed_error());
        }
        let n = self.cmd.msg_count;
        let write_ptr = qm
            .load_acquire(self.cmd_off + 16)
            .ok_or("command queue write pointer unreadable")?;
        if write_ptr >= n {
            // Wedge (a): reading it as "empty" would stall the guest silently.
            return Err(self.fail(format!(
                "command queue write pointer {write_ptr} out of range (msgCount {n})"
            )));
        }
        let available = ring_distance(self.read_ptr, write_ptr, n);
        if available == 0 {
            return Ok(None);
        }
        let msg_size = self.cmd.msg_size as usize;
        let mut first = vec![0u8; msg_size];
        if !qm.read(
            Self::slot_off(self.cmd_off, &self.cmd, self.read_ptr),
            &mut first,
        ) {
            return Err("command slot unreadable".into());
        }
        let el = v0_element_layout();
        let seq_num = u32_at(&first, el.seq_off);
        let header = RpcHeader::parse(&first[el.header_size..]);
        let answerable =
            header.signature == RPC_SIGNATURE && header.header_version == RPC_HEADER_VERSION;
        let total = el.header_size + header.length as usize;
        let declared = total.div_ceil(msg_size) as u64;
        if total > MAX_ELEMENT_SIZE || declared > available as u64 {
            // Wedge (b), not skippable: the declared size runs past what the CPU
            // published or past any element it can send, so there is no trustworthy
            // place to resume from.
            return Err(self.fail(format!(
                "bad rpc length {} in command slot {}: {declared} slots declared, {available} published",
                header.length, self.read_ptr
            )));
        }
        let slots = declared as u32;
        if header.length < RPC_HEADER_SIZE as u32 {
            // Wedge (b), skippable: the element fits in what was published, so skip
            // it by its declared size (one slot) and go on with the next one.
            let reason = format!(
                "bad rpc length {} in command slot {} (seqNum {seq_num}), element skipped",
                header.length, self.read_ptr
            );
            log::error!("GSP command queue: {reason}");
            self.rejected += 1;
            self.consume(qm, slots);
            self.rx_seq = seq_num.wrapping_add(1);
            return Ok(Some(Message {
                seq_num,
                header,
                payload: Vec::new(),
                slot_payload: Vec::new(),
                checksum_ok: false,
                slots,
                rejected: Some(Rejection {
                    status: NV_ERR_INVALID_PARAM_STRUCT,
                    answerable,
                    reason,
                }),
                gmcapi: None,
            }));
        }
        let mut raw = first;
        for i in 1..slots {
            let mut more = vec![0u8; msg_size];
            if !qm.read(
                Self::slot_off(self.cmd_off, &self.cmd, self.read_ptr + i),
                &mut more,
            ) {
                return Err("command slot unreadable".into());
            }
            raw.extend_from_slice(&more);
        }
        let padded = total.div_ceil(8) * 8;
        let checksum_ok = checksum32(&raw[..padded.min(raw.len())]) == 0;
        let payload = raw[el.header_size + RPC_HEADER_SIZE..total].to_vec();
        let slot_payload = raw[el.header_size + RPC_HEADER_SIZE..].to_vec();
        let slot = self.read_ptr;
        self.consume(qm, slots);
        if seq_num != self.rx_seq {
            // The guest resynchronizes on its receive side too
            // (message_queue_cpu.c:762-787); ours only logs, since a driver reload
            // without a device reset legitimately restarts the guest's count at 0.
            self.seq_mismatches += 1;
            log::warn!(
                "GSP command queue: seqNum {seq_num}, expected {} ({})",
                self.rx_seq,
                if seq_before(seq_num, self.rx_seq) {
                    "an older element"
                } else {
                    "elements missing"
                }
            );
        }
        self.rx_seq = seq_num.wrapping_add(1);
        let rejected = if !checksum_ok {
            self.rejected += 1;
            let reason = format!(
                "bad checksum in command slot {slot} (seqNum {seq_num}, function {}, sequence {}), not dispatched",
                header.function, header.sequence
            );
            log::error!("GSP command queue: {reason}");
            Some(Rejection {
                status: NV_ERR_INVALID_DATA,
                answerable,
                reason,
            })
        } else if !answerable {
            // The checksum authenticates only the bytes' integrity. It does not make a
            // foreign RPC schema safe to dispatch. Neither function nor sequence is
            // trustworthy enough to reflect in a reply when the signature/version is not
            // the one this server implements; consume the bounded element and continue.
            self.rejected += 1;
            let reason = format!(
                "invalid RPC header in command slot {slot} (seqNum {seq_num}): version {:#010x}, signature {:#010x}, not dispatched",
                header.header_version, header.signature
            );
            log::error!("GSP command queue: {reason}");
            Some(Rejection {
                status: NV_ERR_INVALID_DATA,
                answerable: false,
                reason,
            })
        } else {
            None
        };
        Ok(Some(Message {
            seq_num,
            header,
            payload,
            slot_payload,
            checksum_ok,
            slots,
            rejected,
            gmcapi: None,
        }))
    }

    /// Slots the CPU has not consumed yet leave this much room in our queue; one
    /// slot always stays free so that a full ring is not mistaken for an empty one.
    /// A CPU read pointer out of range leaves no room (the element stays held and
    /// is logged, see [`QueueState::retry_held`]).
    fn free_slots(&self, qm: &QueueMemory) -> u32 {
        let cpu_read = qm
            .load_acquire(self.cmd_off + self.cmd.rx_hdr_off as u64)
            .unwrap_or(u32::MAX);
        let n = self.stat.msg_count;
        if cpu_read >= n || self.stat.write_ptr >= n {
            return 0;
        }
        n - 1 - ring_distance(cpu_read, self.stat.write_ptr, n)
    }

    /// Elements waiting for room in the status queue.
    pub fn held(&self) -> usize {
        if let Some(q) = self.v2.as_deref() {
            return q.held();
        }
        self.held.len()
    }

    /// Append one message to the status queue and publish the new write pointer.
    ///
    /// The element gets its sequence number and checksum now. When the CPU has not
    /// left room for it (or for an element held before it), it is held, in order, and
    /// written by a later `send` or [`QueueState::retry_held`]; `Ok` then still
    /// returns its sequence number and [`QueueState::held`] counts it. An element is
    /// only refused (`Err`, logged) when it can never fit or `MAX_HELD` elements are
    /// already waiting.
    pub fn send(
        &mut self,
        qm: &QueueMemory,
        header: &RpcHeader,
        payload: &[u8],
    ) -> Result<u32, String> {
        if let Some(q) = self.v2.as_deref_mut() {
            return q.send(qm, header, payload);
        }
        if self.failed.is_some() {
            return Err(self.failed_error());
        }
        let el = v0_element_layout();
        let total = el.header_size + RPC_HEADER_SIZE + payload.len();
        let msg_size = self.stat.msg_size as usize;
        let slots = total.div_ceil(msg_size) as u32;
        if total > MAX_ELEMENT_SIZE || slots >= self.stat.msg_count {
            return Err(format!(
                "element of {total} bytes does not fit the status queue"
            ));
        }
        if self.held.len() >= MAX_HELD {
            let e = format!(
                "status queue full: {} elements held, function {} sequence {} not sent",
                self.held.len(),
                header.function,
                header.sequence
            );
            log::error!("GSP status queue: {e}");
            return Err(e);
        }
        let mut raw = vec![0u8; slots as usize * msg_size];
        if el.mctp {
            put_u32(&mut raw, 0, MCTP_HEADER);
            put_u32(&mut raw, 4, NVDM_HEADER);
        }
        put_u32(&mut raw, el.seq_off, self.tx_seq);
        if let Some(off) = el.elem_count_off {
            put_u32(&mut raw, off, slots);
        }
        let mut h = *header;
        h.length = (RPC_HEADER_SIZE + payload.len()) as u32;
        let rpc = el.header_size;
        raw[rpc..rpc + RPC_HEADER_SIZE].copy_from_slice(&h.encode());
        raw[rpc + RPC_HEADER_SIZE..rpc + RPC_HEADER_SIZE + payload.len()].copy_from_slice(payload);
        let padded = total.div_ceil(8) * 8;
        let sum = checksum32(&raw[..padded]);
        put_u32(&mut raw, el.checksum_off, sum);
        debug_assert_eq!(checksum32(&raw[..padded]), 0);
        let seq = self.tx_seq;
        self.tx_seq = self.tx_seq.wrapping_add(1);
        self.held.push_back(raw);
        self.flush(qm)?;
        if !self.held.is_empty() {
            log::warn!(
                "GSP status queue full: {} element(s) held for the next doorbell (function {} sequence {})",
                self.held.len(),
                header.function,
                header.sequence
            );
        }
        Ok(seq)
    }

    /// Send a GMCAPI request of the GSP-to-CPU direction (msgq/v2.rs
    /// `QueueStateV2::send_gmcapi_request`); GMCAPI exists only on message queue v2
    /// (driver 615 on).
    pub fn send_gmcapi_request(
        &mut self,
        qm: &QueueMemory,
        id: u32,
        params: &[u8],
    ) -> Result<u32, String> {
        match self.v2.as_deref_mut() {
            Some(q) => q.send_gmcapi_request(qm, id, params),
            None => Err(format!(
                "GMCAPI request {id:#x}: the 610 transport (message queue v0) has no GMCAPI"
            )),
        }
    }

    /// Send the response to a CPU-originated GMCAPI request without narrowing its 64-bit
    /// sequence through [`RpcHeader`]. GMCAPI exists only on message queue v2.
    pub fn send_gmcapi_response(
        &mut self,
        qm: &QueueMemory,
        request: &crate::gmcapi::Header,
        status: u32,
        payload: &[u8],
    ) -> Result<u32, String> {
        match self.v2.as_deref_mut() {
            Some(q) => q.send_gmcapi_response(qm, request, status, payload),
            None => Err(format!(
                "GMCAPI response {:#x}: the 610 transport (message queue v0) has no GMCAPI",
                request.id()
            )),
        }
    }

    /// Write held elements while there is room, each one followed by the publish
    /// of the write pointer (release: its slots are visible first).
    fn flush(&mut self, qm: &QueueMemory) -> Result<(), String> {
        let msg_size = self.stat.msg_size as usize;
        while let Some(raw) = self.held.front() {
            let slots = (raw.len() / msg_size) as u32;
            if slots > self.free_slots(qm) {
                self.held_stalls += 1;
                if self.held_stalls == 2 {
                    log::error!(
                        "GSP status queue: {} element(s) still held after a retry; the CPU is not draining the status queue",
                        self.held.len()
                    );
                }
                return Ok(());
            }
            for i in 0..slots {
                let chunk = &raw[i as usize * msg_size..(i as usize + 1) * msg_size];
                if !qm.write(
                    Self::slot_off(self.stat_off, &self.stat, self.stat.write_ptr + i),
                    chunk,
                ) {
                    // The element keeps its sequence number; dropping it would leave a
                    // gap the CPU cannot get past, so the queue is failed instead.
                    return Err(self.fail("status slot unwritable".into()));
                }
            }
            self.stat.write_ptr = (self.stat.write_ptr + slots) % self.stat.msg_count;
            qm.store_release(self.stat_off + 16, self.stat.write_ptr);
            self.held.pop_front();
        }
        self.held_stalls = 0;
        Ok(())
    }

    /// Retry the held elements (on every doorbell; every `send` retries first too).
    /// Returns a trace line when anything was held. An element still held after a
    /// retry is logged at error level once per stall (in `flush`).
    pub fn retry_held(&mut self, qm: &QueueMemory) -> Option<String> {
        if let Some(q) = self.v2.as_deref_mut() {
            return q.retry_held(qm);
        }
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
    use super::*;
    use std::fs::File;
    use std::io::Write;

    /// A guest-memory stand-in: one file mapped at `base`, page table for a
    /// 0x81000-byte queue buffer whose pages follow the table contiguously.
    pub(crate) fn queue_fixture(tag: &str) -> (GuestMemory, Vec<u64>, u64, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("nvgpu-msgq-{tag}-{}", std::process::id()));
        let mut f = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        f.write_all(&vec![0u8; 0x100000]).unwrap();
        let base = 0x4000_0000u64;
        let mut mem = GuestMemory::new();
        mem.map(base, 0x100000, 0, &f).unwrap();
        let pages: Vec<u64> = (0..129).map(|i| base + i * PAGE).collect();
        for (i, pa) in pages.iter().enumerate() {
            assert!(mem.write(base + 8 * i as u64, &pa.to_le_bytes()));
        }
        (mem, pages, base, path)
    }

    /// Write a CPU-style command element into slot `slot` and bump the write pointer.
    pub(crate) fn cpu_send(
        qm: &QueueMemory,
        cmd: &mut TxHeader,
        seq_num: u32,
        header: &RpcHeader,
        payload: &[u8],
    ) {
        let total = 16 + 32 + payload.len();
        let mut raw = vec![0u8; total.div_ceil(8) * 8];
        put_u32(&mut raw, 0, MCTP_HEADER);
        put_u32(&mut raw, 4, NVDM_HEADER);
        put_u32(&mut raw, 12, seq_num);
        let mut h = *header;
        h.length = (32 + payload.len()) as u32;
        raw[16..48].copy_from_slice(&h.encode());
        raw[48..48 + payload.len()].copy_from_slice(payload);
        let sum = checksum32(&raw);
        put_u32(&mut raw, 8, sum);
        let slot = cmd.write_ptr;
        assert!(qm.write(0x1000 + cmd.entry_off as u64 + slot as u64 * 0x1000, &raw));
        cmd.write_ptr = (cmd.write_ptr + 1) % cmd.msg_count;
        assert!(qm.write_u32(0x1000 + 16, cmd.write_ptr));
    }

    #[test]
    fn checksum_folds_to_zero() {
        let mut raw = vec![0u8; 64];
        put_u32(&mut raw, 0, MCTP_HEADER);
        put_u32(&mut raw, 4, NVDM_HEADER);
        raw[20] = 0x77;
        raw[61] = 0x99;
        let sum = checksum32(&raw);
        put_u32(&mut raw, 8, sum);
        assert_eq!(checksum32(&raw), 0);
        assert_ne!(checksum32(&raw[..60]), 0);
    }

    #[test]
    fn link_receive_reply() {
        let (mem, pages, _base, path) = queue_fixture("basic");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cmd = TxHeader::standard(0x40000);
        assert!(qm.write(0x1000, &cmd.encode()));
        let req = RpcHeader {
            header_version: RPC_HEADER_VERSION,
            signature: RPC_SIGNATURE,
            length: 0,
            function: 1,
            rpc_result: 0xFFFF_FFFF,
            rpc_result_private: 0xFFFF_FFFF,
            sequence: 0,
            u: 0,
        };
        let mut payload = vec![0u8; 792];
        put_u32(&mut payload, 0, 0x2E);
        put_u32(&mut payload, 4, 0x0D);
        cpu_send(&qm, &mut cmd, 0, &req, &payload);

        let mut q = QueueState::link(&qm, 0x1000, 0x41000, 0x40000).unwrap();
        let mut hdr = [0u8; 32];
        assert!(qm.read(0x41000, &mut hdr));
        let stat = TxHeader::parse(&hdr);
        assert_eq!(
            (
                stat.size,
                stat.msg_size,
                stat.msg_count,
                stat.rx_hdr_off,
                stat.entry_off
            ),
            (0x40000, 0x1000, 63, 0x20, 0x1000)
        );
        assert_eq!(stat.flags, MSGQ_FLAGS_SWAP_RX);

        let m = q.receive(&qm).unwrap().expect("one message");
        assert!(m.checksum_ok);
        assert_eq!(m.header.function, 1);
        assert_eq!(m.payload.len(), 792);
        assert_eq!(m.slots, 1);
        assert_eq!(qm.read_u32(0x41020), Some(1));
        assert!(q.receive(&qm).unwrap().is_none());

        let reply = RpcHeader::reply(&m.header, 0, m.payload.len());
        let seq = q.send(&qm, &reply, &m.payload).unwrap();
        assert_eq!(seq, 0);
        assert_eq!(qm.read_u32(0x41010), Some(1));
        let mut elem = vec![0u8; 0x1000];
        assert!(qm.read(0x42000, &mut elem));
        assert_eq!(u32_at(&elem, 0), MCTP_HEADER);
        assert_eq!(u32_at(&elem, 12), 0);
        let h = RpcHeader::parse(&elem[16..]);
        assert_eq!(
            (h.function, h.rpc_result, h.rpc_result_private, h.length),
            (1, 0, 0, 32 + 792)
        );
        assert_eq!(checksum32(&elem[..(16usize + 32 + 792).div_ceil(8) * 8]), 0);
        assert_eq!(u32_at(&elem, 48), 0x2E);

        // 63 slots minus the one written and the one always kept free.
        assert_eq!(q.free_slots(&qm), 61);
        let _ = std::fs::remove_file(&path);
    }

    /// The pointer words go through acquire loads and release stores: receive loads
    /// the CPU's write pointer before it reads the slot and publishes our read pointer
    /// last; send loads the CPU's read pointer before it writes a slot and publishes
    /// our write pointer last.
    #[test]
    fn pointer_words_are_acquire_release() {
        let (mem, pages, _base, path) = queue_fixture("order");
        let qm = QueueMemory::new(&mem, &pages);
        let mut cmd = TxHeader::standard(0x40000);
        assert!(qm.write(0x1000, &cmd.encode()));
        let mut q = QueueState::link(&qm, 0x1000, 0x41000, 0x40000).unwrap();
        cpu_send(&qm, &mut cmd, 0, &request(1), &[1, 2, 3, 4]);
        qm.ordered.borrow_mut().clear();
        let m = q.receive(&qm).unwrap().expect("one message");
        assert_eq!(
            *qm.ordered.borrow(),
            vec![('A', 0x1000 + 16), ('R', 0x41000 + 0x20)]
        );
        qm.ordered.borrow_mut().clear();
        q.send(&qm, &RpcHeader::reply(&m.header, 0, 0), &[])
            .unwrap();
        assert_eq!(
            *qm.ordered.borrow(),
            vec![('A', 0x1000 + 0x20), ('R', 0x41000 + 16)]
        );
        // Both sides see the same values as through the plain accessors.
        assert_eq!(qm.read_u32(0x41000 + 16), Some(1));
        assert_eq!(qm.load_acquire(0x41000 + 0x20), Some(1));
        let _ = std::fs::remove_file(&path);
    }

    /// A well-formed CPU request header for `function`.
    pub(crate) fn request(function: u32) -> RpcHeader {
        RpcHeader {
            header_version: RPC_HEADER_VERSION,
            signature: RPC_SIGNATURE,
            length: 0,
            function,
            rpc_result: 0xFFFF_FFFF,
            rpc_result_private: 0xFFFF_FFFF,
            sequence: 7,
            u: 0,
        }
    }

    fn v610_layout() -> crate::ver::ElementLayout {
        crate::ver::ElementLayout {
            header_size: ELEMENT_HEADER_SIZE,
            checksum_off: 8,
            seq_off: 12,
            elem_count_off: None,
            mctp: true,
        }
    }

    /// A CPU element for `header` and `payload` in the selected v0 framing: `length` set,
    /// checksum folded to zero, padded to whole slots.
    pub(crate) fn element_for(
        el: crate::ver::ElementLayout,
        seq_num: u32,
        header: &RpcHeader,
        payload: &[u8],
    ) -> Vec<u8> {
        let total = el.header_size + RPC_HEADER_SIZE + payload.len();
        let mut raw = vec![0u8; total.div_ceil(0x1000) * 0x1000];
        if el.mctp {
            put_u32(&mut raw, 0, MCTP_HEADER);
            put_u32(&mut raw, 4, NVDM_HEADER);
        }
        put_u32(&mut raw, el.seq_off, seq_num);
        if let Some(off) = el.elem_count_off {
            let slots = raw.len().div_ceil(0x1000) as u32;
            put_u32(&mut raw, off, slots);
        }
        let mut h = *header;
        h.length = (RPC_HEADER_SIZE + payload.len()) as u32;
        let rpc = el.header_size;
        raw[rpc..rpc + RPC_HEADER_SIZE].copy_from_slice(&h.encode());
        raw[rpc + RPC_HEADER_SIZE..total].copy_from_slice(payload);
        reseal_for(&mut raw, el);
        raw
    }

    fn element(seq_num: u32, header: &RpcHeader, payload: &[u8]) -> Vec<u8> {
        element_for(v610_layout(), seq_num, header, payload)
    }

    /// Recompute the checksum over the length the RPC header declares.
    fn reseal(raw: &mut [u8]) {
        reseal_for(raw, v610_layout());
    }

    fn reseal_for(raw: &mut [u8], el: crate::ver::ElementLayout) {
        put_u32(raw, el.checksum_off, 0);
        let total = (el.header_size + u32_at(raw, el.header_size + 8) as usize).div_ceil(8) * 8;
        let sum = checksum32(&raw[..total.min(raw.len())]);
        put_u32(raw, el.checksum_off, sum);
    }

    /// Write whole slots at the CPU's write pointer, slot by slot so that an element
    /// wraps at the ring end, then publish the write pointer.
    pub(crate) fn cpu_raw(qm: &QueueMemory, cmd: &mut TxHeader, raw: &[u8]) {
        let slots = raw.len().div_ceil(0x1000) as u32;
        for i in 0..slots {
            let slot = (cmd.write_ptr + i) % cmd.msg_count;
            let chunk = &raw[i as usize * 0x1000..(i as usize + 1) * 0x1000];
            assert!(qm.write(0x1000 + cmd.entry_off as u64 + slot as u64 * 0x1000, chunk));
        }
        cmd.write_ptr = (cmd.write_ptr + slots) % cmd.msg_count;
        assert!(qm.write_u32(0x1000 + 16, cmd.write_ptr));
    }

    /// A fresh fixture with the CPU's command-queue header in place:
    /// (memory, pages, file, CPU command header).
    fn linked(tag: &str) -> (GuestMemory, Vec<u64>, std::path::PathBuf, TxHeader) {
        let (mem, pages, _base, path) = queue_fixture(tag);
        let cmd = TxHeader::standard(0x40000);
        assert!(QueueMemory::new(&mem, &pages).write(0x1000, &cmd.encode()));
        (mem, pages, path, cmd)
    }

    /// Wedge (a): a write pointer at or past msgCount used to read as "empty"
    /// forever; now it fails the queue, for receive and send alike.
    #[test]
    fn write_pointer_out_of_range_fails_the_queue() {
        let (mem, pages, path, _cmd) = linked("wptr");
        let qm = QueueMemory::new(&mem, &pages);
        let mut q = QueueState::link(&qm, 0x1000, 0x41000, 0x40000).unwrap();
        assert!(qm.write_u32(0x1000 + 16, 63));
        let e = q.receive(&qm).unwrap_err();
        assert!(e.contains("write pointer 63 out of range"), "{e}");
        assert!(q.failed.is_some());
        // Final: a later doorbell sees the same failure, even with a sane pointer.
        assert!(qm.write_u32(0x1000 + 16, 0));
        assert!(q.receive(&qm).is_err());
        assert!(q.send(&qm, &RpcHeader::event(0x1001, 1), &[0]).is_err());
        let _ = std::fs::remove_file(&path);
    }

    /// Wedge (b), skippable: an RPC length shorter than the RPC header used to be an
    /// error without progress; now the element is consumed by its declared size
    /// (one slot), rejected, and the next element is received normally.
    #[test]
    fn short_length_element_is_skipped() {
        let (mem, pages, path, mut cmd) = linked("short");
        let qm = QueueMemory::new(&mem, &pages);
        let mut q = QueueState::link(&qm, 0x1000, 0x41000, 0x40000).unwrap();
        let mut bad = element(0, &request(5), &[0u8; 16]);
        put_u32(&mut bad, 16 + 8, 8);
        reseal(&mut bad);
        cpu_raw(&qm, &mut cmd, &bad);
        cpu_raw(&qm, &mut cmd, &element(1, &request(6), &[9u8; 8]));

        let m = q.receive(&qm).unwrap().expect("the bad element");
        let rej = m.rejected.clone().expect("rejected");
        assert_eq!(rej.status, NV_ERR_INVALID_PARAM_STRUCT);
        assert_eq!(m.reply_to(), Some(ReplyTo::Itself));
        assert_eq!((m.slots, q.read_ptr), (1, 1));
        assert_eq!(qm.read_u32(0x41020), Some(1));
        let m = q.receive(&qm).unwrap().expect("the next element");
        assert!(m.rejected.is_none());
        assert_eq!(
            (m.header.function, m.payload.as_slice()),
            (6, &[9u8; 8][..])
        );
        assert!(q.receive(&qm).unwrap().is_none());
        assert_eq!((q.rejected, q.failed.as_deref()), (1, None));
        let _ = std::fs::remove_file(&path);
    }

    /// Wedge (b), not skippable: a length past `queueElementSizeMax`, or one that
    /// declares more slots than the CPU published, fails the queue.
    #[test]
    fn implausible_length_fails_the_queue() {
        for (tag, length) in [("huge", 0x2_0000u32), ("past", 0x1800)] {
            let (mem, pages, path, mut cmd) = linked(tag);
            let qm = QueueMemory::new(&mem, &pages);
            let mut q = QueueState::link(&qm, 0x1000, 0x41000, 0x40000).unwrap();
            let mut bad = element(0, &request(5), &[0u8; 16]);
            put_u32(&mut bad, 16 + 8, length);
            reseal(&mut bad);
            cpu_raw(&qm, &mut cmd, &bad);
            let e = q.receive(&qm).unwrap_err();
            assert!(e.contains("bad rpc length"), "{e}");
            assert!(q.failed.is_some(), "{tag}");
            assert!(q.receive(&qm).is_err());
            let _ = std::fs::remove_file(&path);
        }
    }

    /// Wedge (c): a full status queue used to drop the reply. Now it is held, in
    /// order behind anything held before it, and written once the CPU drains.
    #[test]
    fn full_status_queue_holds_replies() {
        let (mem, pages, path, _cmd) = linked("full");
        let qm = QueueMemory::new(&mem, &pages);
        let mut q = QueueState::link(&qm, 0x1000, 0x41000, 0x40000).unwrap();
        // 63 slots, one kept free: 62 elements fit while the CPU reads nothing.
        for i in 0..62 {
            assert_eq!(q.send(&qm, &RpcHeader::event(0x1001, 4), &[0; 4]), Ok(i));
        }
        assert_eq!((q.held(), q.stat.write_ptr), (0, 62));
        assert_eq!(q.send(&qm, &RpcHeader::event(0x1002, 4), &[1; 4]), Ok(62));
        assert_eq!(q.send(&qm, &RpcHeader::event(0x1003, 4), &[2; 4]), Ok(63));
        assert_eq!((q.held(), qm.read_u32(0x41000 + 16)), (2, Some(62)));
        assert_eq!(
            q.retry_held(&qm).as_deref(),
            Some("status queue: 0 held element(s) written, 2 still held")
        );
        // The CPU consumes two elements: both held ones go out, in order.
        assert!(qm.write_u32(0x1000 + 0x20, 2));
        assert_eq!(
            q.retry_held(&qm).as_deref(),
            Some("status queue: 2 held element(s) written, 0 still held")
        );
        assert_eq!((q.held(), qm.read_u32(0x41000 + 16)), (0, Some(1)));
        let mut elem = vec![0u8; 0x1000];
        for (slot, seq, function) in [(62u64, 62, 0x1002), (0, 63, 0x1003)] {
            assert!(qm.read(0x42000 + slot * 0x1000, &mut elem));
            assert_eq!(u32_at(&elem, 12), seq);
            assert_eq!(RpcHeader::parse(&elem[16..]).function, function);
            assert_eq!(checksum32(&elem[..56]), 0);
        }
        assert_eq!(q.retry_held(&qm), None);
        // Bounded: past MAX_HELD waiting elements a send is refused (logged), not
        // dropped silently.
        assert!(qm.write_u32(0x1000 + 0x20, 1));
        let mut refused = 0;
        for _ in 0..(62 + MAX_HELD + 1) {
            if q.send(&qm, &RpcHeader::event(0x1001, 4), &[0; 4]).is_err() {
                refused += 1;
            }
        }
        assert_eq!((q.held(), refused), (MAX_HELD, 1));
        let _ = std::fs::remove_file(&path);
    }

    /// A bad checksum used to be dispatched and answered as if intact. Now the
    /// element is consumed, counted and rejected with NV_ERR_INVALID_DATA; the reply
    /// goes to the request itself, to the request being assembled for a
    /// continuation record, or to nobody when the RPC header is not intact.
    #[test]
    fn bad_checksum_is_rejected() {
        let (mem, pages, path, mut cmd) = linked("csum");
        let qm = QueueMemory::new(&mem, &pages);
        let mut q = QueueState::link(&qm, 0x1000, 0x41000, 0x40000).unwrap();
        let mut torn = element(0, &request(5), &[7u8; 64]);
        torn[48 + 3] ^= 0x40;
        cpu_raw(&qm, &mut cmd, &torn);
        let fn_cont = crate::ver::v610::FN_CONTINUATION_RECORD;
        let mut cont = element(1, &request(fn_cont), &[7u8; 64]);
        cont[48] ^= 1;
        cpu_raw(&qm, &mut cmd, &cont);
        // The guest's own error injection: mctpHeader overwritten after the checksum
        // (message_queue_cpu.c:548-552); the RPC header is intact.
        let mut injected = element(2, &request(6), &[]);
        put_u32(&mut injected, 0, 0xFFFF_FFFF);
        cpu_raw(&qm, &mut cmd, &injected);
        let mut garbage = element(3, &request(6), &[]);
        put_u32(&mut garbage, 16 + 4, 0xDEAD_BEEF);
        cpu_raw(&qm, &mut cmd, &garbage);

        let expect = [
            ReplyTo::Itself,
            ReplyTo::PendingRequest,
            ReplyTo::Itself,
            ReplyTo::Nobody,
        ];
        for (i, to) in expect.into_iter().enumerate() {
            let m = q.receive(&qm).unwrap().expect("element");
            assert!(!m.checksum_ok);
            assert_eq!(m.rejected.as_ref().unwrap().status, NV_ERR_INVALID_DATA);
            assert_eq!(m.reply_to(), Some(to), "element {i}");
            assert_eq!(q.read_ptr, i as u32 + 1);
        }
        assert!(q.receive(&qm).unwrap().is_none());
        assert_eq!((q.rejected, q.failed.as_deref()), (4, None));
        // An intact element is not rejected and has nobody to answer for.
        cpu_send(&qm, &mut cmd, 4, &request(1), &[1, 2, 3]);
        let m = q.receive(&qm).unwrap().expect("intact");
        assert_eq!((m.rejected.is_none(), m.reply_to()), (true, None));
        let _ = std::fs::remove_file(&path);
    }

    /// A valid element checksum does not authorize a foreign RPC schema. Both v0 outer
    /// framings consume a bad signature/version without dispatch or reflection, then
    /// recover at the following request: 610 has the 16-byte MCTP/NVDM header, while 595
    /// and 570 use the pre-610 0x30-byte auth/AAD header and explicit element count.
    #[test]
    fn invalid_rpc_header_is_rejected_in_both_v0_framings() {
        use crate::ver::{GuestAbi, TestGuest};
        for (tag, guest, el) in [
            ("rpc-header-610", GuestAbi::V610, v610_layout()),
            (
                "rpc-header-pre610",
                GuestAbi::V595,
                crate::ver::ElementLayout::PRE_610,
            ),
        ] {
            let _g = TestGuest::set(guest);
            let (mem, pages, path, mut cmd) = linked(tag);
            let qm = QueueMemory::new(&mem, &pages);
            let mut q = QueueState::link(&qm, 0x1000, 0x41000, 0x40000).unwrap();

            let mut signature = request(5);
            signature.signature ^= 1;
            cpu_raw(&qm, &mut cmd, &element_for(el, 0, &signature, &[1]));
            let mut version = request(6);
            version.header_version ^= 1;
            cpu_raw(&qm, &mut cmd, &element_for(el, 1, &version, &[2]));
            cpu_raw(&qm, &mut cmd, &element_for(el, 2, &request(7), &[3]));

            for (i, field) in ["signature", "version"].into_iter().enumerate() {
                let m = q.receive(&qm).unwrap().expect("invalid RPC header element");
                assert!(m.checksum_ok, "{tag}: the outer checksum is valid");
                let r = m.rejected.as_ref().expect("foreign RPC schema rejected");
                assert_eq!((r.status, r.answerable), (NV_ERR_INVALID_DATA, false));
                assert!(r.reason.contains(field), "{tag}: {}", r.reason);
                assert_eq!(m.reply_to(), Some(ReplyTo::Nobody));
                assert_eq!(q.read_ptr, i as u32 + 1);
            }
            let m = q.receive(&qm).unwrap().expect("following valid RPC");
            assert!(m.rejected.is_none());
            assert_eq!((m.header.function, m.payload.as_slice()), (7, &[3u8][..]));
            assert_eq!((q.rejected, q.failed.as_deref(), q.read_ptr), (2, None, 3));
            let _ = std::fs::remove_file(&path);
        }
    }

    /// The wrap rule of `seq_before` and `ring_distance`.
    #[test]
    fn wrap_safe_comparisons() {
        assert!(seq_before(1, 2));
        assert!(!seq_before(2, 1));
        assert!(!seq_before(5, 5));
        assert!(seq_before(0xFFFF_FFFF, 0));
        assert!(seq_before(0xFFFF_FFF0, 0x10));
        assert!(!seq_before(0x10, 0xFFFF_FFF0));
        assert!(seq_before(0, 0x7FFF_FFFF));
        assert!(!seq_before(0, 0x8000_0000));
        assert_eq!(ring_distance(5, 5, 63), 0);
        assert_eq!(ring_distance(60, 2, 63), 5);
        assert_eq!(ring_distance(2, 60, 63), 58);
        assert_eq!(ring_distance(u32::MAX - 1, 0, u32::MAX), 1);
    }

    /// An element that wraps at the ring end, a write pointer numerically behind our
    /// read pointer, and element sequence numbers across 2^32: all received in
    /// order, without a sequence complaint, and free slots counted across the wrap.
    #[test]
    fn queue_pointers_and_sequences_wrap() {
        let (mem, pages, path, mut cmd) = linked("wrap");
        let qm = QueueMemory::new(&mem, &pages);
        let mut q = QueueState::link(&qm, 0x1000, 0x41000, 0x40000).unwrap();
        cmd.write_ptr = 62;
        q.read_ptr = 62;
        q.rx_seq = 0xFFFF_FFFF;
        let big: Vec<u8> = (0..5000u32).map(|i| i as u8).collect();
        cpu_raw(&qm, &mut cmd, &element(0xFFFF_FFFF, &request(3), &big));
        cpu_raw(&qm, &mut cmd, &element(0, &request(4), &[1]));
        assert_eq!(cmd.write_ptr, 2);
        let m = q.receive(&qm).unwrap().expect("wrapped element");
        assert_eq!((m.slots, m.payload.len(), q.read_ptr), (2, 5000, 1));
        assert!(m.checksum_ok && m.payload == big);
        let m = q.receive(&qm).unwrap().expect("next");
        assert_eq!((m.seq_num, m.header.function, q.read_ptr), (0, 4, 2));
        assert_eq!((q.seq_mismatches, q.rx_seq), (0, 1));
        // A replayed older element is still received, but counted.
        cpu_raw(&qm, &mut cmd, &element(0xFFFF_FFFE, &request(4), &[1]));
        assert!(q.receive(&qm).unwrap().is_some());
        assert_eq!(q.seq_mismatches, 1);
        // Status queue: CPU read pointer 62, our write pointer 1: 2 used, 60 free.
        q.stat.write_ptr = 1;
        assert!(qm.write_u32(0x1000 + 0x20, 62));
        assert_eq!(q.free_slots(&qm), 60);
        let _ = std::fs::remove_file(&path);
    }
}

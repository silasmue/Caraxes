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

/// The element and RPC header constants of the emulated driver version.
pub use crate::ver::active::{
    ELEMENT_HEADER_SIZE, MCTP_HEADER, MSGQ_FLAGS_SWAP_RX, NVDM_HEADER, RPC_HEADER_SIZE,
    RPC_HEADER_VERSION, RPC_SIGNATURE,
};

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
        })
    }

    fn slot_off(base: u64, hdr: &TxHeader, slot: u32) -> u64 {
        base + hdr.entry_off as u64 + (slot % hdr.msg_count) as u64 * hdr.msg_size as u64
    }

    /// Take the next message from the command queue, if any.
    pub fn receive(&mut self, qm: &QueueMemory) -> Result<Option<Message>, String> {
        let write_ptr = qm
            .load_acquire(self.cmd_off + 16)
            .ok_or("command queue write pointer unreadable")?;
        if write_ptr >= self.cmd.msg_count || write_ptr == self.read_ptr {
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
        let header = RpcHeader::parse(&first[ELEMENT_HEADER_SIZE..]);
        let total = ELEMENT_HEADER_SIZE + header.length as usize;
        if header.length < RPC_HEADER_SIZE as u32 || total > 0x10000 {
            return Err(format!(
                "bad rpc length {} in command slot {}",
                header.length, self.read_ptr
            ));
        }
        let slots = total.div_ceil(msg_size) as u32;
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
        let seq_num = u32_at(&raw, 12);
        let payload = raw[ELEMENT_HEADER_SIZE + RPC_HEADER_SIZE..total].to_vec();
        let slot_payload = raw[ELEMENT_HEADER_SIZE + RPC_HEADER_SIZE..].to_vec();
        self.read_ptr = (self.read_ptr + slots) % self.cmd.msg_count;
        qm.store_release(self.stat_off + self.stat.rx_hdr_off as u64, self.read_ptr);
        self.rx_seq = self.rx_seq.wrapping_add(1);
        Ok(Some(Message {
            seq_num,
            header,
            payload,
            slot_payload,
            checksum_ok,
            slots,
        }))
    }

    /// Slots the CPU has not consumed yet leave this much room in our queue.
    fn free_slots(&self, qm: &QueueMemory) -> u32 {
        let cpu_read = qm
            .load_acquire(self.cmd_off + self.cmd.rx_hdr_off as u64)
            .unwrap_or(0);
        let n = self.stat.msg_count;
        if cpu_read >= n || self.stat.write_ptr >= n {
            return 0;
        }
        (cpu_read + n - self.stat.write_ptr - 1) % n
    }

    /// Append one message to the status queue and publish the new write pointer.
    pub fn send(
        &mut self,
        qm: &QueueMemory,
        header: &RpcHeader,
        payload: &[u8],
    ) -> Result<u32, String> {
        let total = ELEMENT_HEADER_SIZE + RPC_HEADER_SIZE + payload.len();
        let msg_size = self.stat.msg_size as usize;
        let slots = total.div_ceil(msg_size) as u32;
        let free = self.free_slots(qm);
        if slots > free {
            return Err(format!(
                "status queue full: need {slots} slots, {free} free"
            ));
        }
        let mut raw = vec![0u8; slots as usize * msg_size];
        put_u32(&mut raw, 0, MCTP_HEADER);
        put_u32(&mut raw, 4, NVDM_HEADER);
        put_u32(&mut raw, 12, self.tx_seq);
        let mut h = *header;
        h.length = (RPC_HEADER_SIZE + payload.len()) as u32;
        raw[16..48].copy_from_slice(&h.encode());
        raw[48..48 + payload.len()].copy_from_slice(payload);
        let padded = total.div_ceil(8) * 8;
        let sum = checksum32(&raw[..padded]);
        put_u32(&mut raw, 8, sum);
        debug_assert_eq!(checksum32(&raw[..padded]), 0);
        for i in 0..slots {
            let chunk = &raw[i as usize * msg_size..(i as usize + 1) * msg_size];
            if !qm.write(
                Self::slot_off(self.stat_off, &self.stat, self.stat.write_ptr + i),
                chunk,
            ) {
                return Err("status slot unwritable".into());
            }
        }
        self.stat.write_ptr = (self.stat.write_ptr + slots) % self.stat.msg_count;
        qm.store_release(self.stat_off + 16, self.stat.write_ptr);
        let seq = self.tx_seq;
        self.tx_seq = self.tx_seq.wrapping_add(1);
        Ok(seq)
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
}

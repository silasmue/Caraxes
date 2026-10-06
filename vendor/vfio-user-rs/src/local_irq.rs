// Copyright © 2026 Caraxes contributors
// SPDX-License-Identifier: Apache-2.0
//
// Private, explicitly negotiated vfio-user extension. The shared queue is host
// transport, never a PCI BAR or DMA region. Both peers are trusted cooperating
// processes; mmap protections do not make the O_RDWR descriptor a security boundary.
//
// Fixed shm-fold-v1 layout (all integer fields little-endian): page 0 has the
// consumer's atomic u64 head ticket at byte 0; page 1 has producer atomic u64
// tail at 4096, atomic u64 terminal flag at 4104, nonce at 4112, initial epoch
// at 4128, magic at 4144 and version at 4148. All other header bytes are zero.
// Slots begin at 8192, 1024 x 128 bytes. Each slot holds magic/version/kind/
// payload length, nonce, epoch, per-epoch sequence, physical ticket, payload,
// and zero reserved bytes. A release tail publishes one complete slot; an
// acquire tail permits the consumer to copy it before releasing head. One Fold
// slot represents one complete eight-word backend pending-bitmap drain.

use std::fs::File;
use std::io;
use std::mem::size_of;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::net::UnixStream;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use thiserror::Error;

pub const SLOT_SIZE: usize = 128;
pub const CAPACITY: u64 = 1024;
pub const QUEUE_BYTES: usize = 8192 + SLOT_SIZE * CAPACITY as usize;
const TAIL_PAGE: usize = 4096;
const RECORDS: usize = 8192;
const MAGIC: &[u8; 4] = b"LIRQ";
const VERSION: u16 = 1;
const FAULT: usize = TAIL_PAGE + 8;
const NONCE: usize = TAIL_PAGE + 16;
const EPOCH: usize = TAIL_PAGE + 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub token: u64,
    pub session_nonce: [u8; 16],
    pub epoch: u64,
    pub bar_index: u32,
    pub tree_offset: u64,
    pub page_size: u32,
    pub rearm_offset: u64,
    pub msi_index: u32,
    pub msi_count: u32,
    pub hints: [u32; 8],
    pub watchdog_period_us: u32,
    pub msi_enabled: bool,
    pub initial_ticket: u64,
}

impl Metadata {
    /// Create a fresh per-connection ticket and nonce from the OS entropy source.
    pub fn fresh(
        bar_index: u32,
        tree_offset: u64,
        rearm_offset: u64,
        hints: [u32; 8],
        watchdog_period_us: u32,
        msi_enabled: bool,
    ) -> io::Result<Self> {
        let mut random = [0u8; 24];
        let mut done = 0;
        while done < random.len() {
            // SAFETY: the remaining slice is writable and stays alive for this call.
            let n = unsafe {
                libc::getrandom(random[done..].as_mut_ptr().cast(), random.len() - done, 0)
            };
            if n < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            if n == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            done += n as usize;
        }
        let mut nonce = [0u8; 16];
        nonce.copy_from_slice(&random[8..24]);
        let token = u64::from_le_bytes(random[0..8].try_into().unwrap());
        if token == 0 || nonce == [0; 16] {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let m = Self {
            token,
            session_nonce: nonce,
            epoch: 1,
            bar_index,
            tree_offset,
            page_size: 4096,
            rearm_offset,
            msi_index: 1,
            msi_count: 1,
            hints,
            watchdog_period_us,
            msi_enabled,
            initial_ticket: 0,
        };
        m.validate()
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        Ok(m)
    }
    pub fn validate(&self) -> Result<(), QueueError> {
        if self.token == 0
            || self.session_nonce == [0; 16]
            || self.epoch == 0
            || self.page_size != 4096
            || self.tree_offset % 4096 != 0
            || self.rearm_offset == self.tree_offset
            || self.msi_count != 1
            || self.initial_ticket != 0
            || self.watchdog_period_us > 1_000_000
        {
            return Err(QueueError::Invalid("metadata"));
        }
        Ok(())
    }

    // Fixed little-endian PREPARE reply, independent of Rust struct layout.
    pub(crate) fn encode(&self) -> [u8; 112] {
        let mut b = [0u8; 112];
        b[0..4].copy_from_slice(MAGIC);
        put16(&mut b[4..6], VERSION);
        put64(&mut b[8..16], self.token);
        b[16..32].copy_from_slice(&self.session_nonce);
        put64(&mut b[32..40], self.epoch);
        put32(&mut b[40..44], self.bar_index);
        put64(&mut b[48..56], self.tree_offset);
        put32(&mut b[56..60], self.page_size);
        put64(&mut b[64..72], self.rearm_offset);
        put32(&mut b[72..76], self.msi_index);
        put32(&mut b[76..80], self.msi_count);
        for (i, v) in self.hints.iter().enumerate() {
            put32(&mut b[80 + i * 4..84 + i * 4], *v);
        }
        // Additional controls are a fixed 16-byte suffix.
        b
    }

    pub(crate) fn decode(b: &[u8]) -> Result<Self, QueueError> {
        if b.len() != 128
            || &b[0..4] != MAGIC
            || get16(&b[4..6]) != VERSION
            || b[6..8] != [0; 2]
            || b[44..48] != [0; 4]
            || b[60..64] != [0; 4]
            || b[117..120] != [0; 3]
        {
            return Err(QueueError::Invalid("prepare reply"));
        }
        let mut nonce = [0; 16];
        nonce.copy_from_slice(&b[16..32]);
        let mut hints = [0; 8];
        for (i, v) in hints.iter_mut().enumerate() {
            *v = get32(&b[80 + i * 4..84 + i * 4]);
        }
        let m = Self {
            token: get64(&b[8..16]),
            session_nonce: nonce,
            epoch: get64(&b[32..40]),
            bar_index: get32(&b[40..44]),
            tree_offset: get64(&b[48..56]),
            page_size: get32(&b[56..60]),
            rearm_offset: get64(&b[64..72]),
            msi_index: get32(&b[72..76]),
            msi_count: get32(&b[76..80]),
            hints,
            watchdog_period_us: get32(&b[112..116]),
            msi_enabled: b[116] != 0,
            initial_ticket: get64(&b[120..128]),
        };
        if b[116] > 1 {
            return Err(QueueError::Invalid("msi enabled"));
        }
        m.validate()?;
        Ok(m)
    }

    pub(crate) fn wire(&self) -> [u8; 128] {
        let mut b = [0; 128];
        b[..112].copy_from_slice(&self.encode());
        put32(&mut b[112..116], self.watchdog_period_us);
        b[116] = u8::from(self.msi_enabled);
        put64(&mut b[120..128], self.initial_ticket);
        b
    }
}

#[derive(Error, Debug)]
pub enum QueueError {
    #[error("local IRQ queue I/O: {0}")]
    Io(#[from] io::Error),
    #[error("local IRQ queue invalid: {0}")]
    Invalid(&'static str),
    #[error("local IRQ queue full")]
    Full,
    #[error("local IRQ queue terminal")]
    Terminal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordKind {
    Fold([u32; 8]),
    Hints([u32; 8]),
    MsiEnabled(bool),
    ResetCommit { new_epoch: u64, reset_ticket: u64 },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub epoch: u64,
    pub seq: u64,
    pub kind: RecordKind,
}

pub struct Prepared {
    pub metadata: Metadata,
    pub queue_fd: File,
    pub event_fd: File,
}
pub struct PreparedServer {
    pub metadata: Metadata,
    pub producer: QueueProducer,
}

struct Mapping {
    ptr: NonNull<u8>,
    len: usize,
}
// SAFETY: the mapping owns its mmap lifetime. Sharing is limited to atomic
// headers and disjoint slots; producer writes a slot before release-publishing
// tail, and consumer finishes copying before release-advancing head.
unsafe impl Send for Mapping {}
// SAFETY: the same ticket protocol serializes cross-thread access; producer
// clones additionally serialize their slot writes under ProducerState's mutex.
unsafe impl Sync for Mapping {}
impl Mapping {
    fn new(fd: &File, offset: usize, len: usize, writable: bool) -> Result<Self, QueueError> {
        let prot = libc::PROT_READ | if writable { libc::PROT_WRITE } else { 0 };
        // SAFETY: fd is a sized memfd; result is checked and unmapped on Drop.
        let p = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                prot,
                libc::MAP_SHARED,
                fd.as_raw_fd(),
                offset as libc::off_t,
            )
        };
        if p == libc::MAP_FAILED {
            return Err(io::Error::last_os_error().into());
        }
        Ok(Self {
            ptr: NonNull::new(p.cast()).unwrap(),
            len,
        })
    }
    fn bytes(&self, offset: usize, len: usize) -> &[u8] {
        assert!(offset.checked_add(len).is_some_and(|end| end <= self.len));
        // SAFETY: callers validate offsets against fixed queue layout; release/acquire
        // on the ticket protects published record bytes from concurrent accesses.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr().add(offset), len) }
    }
    fn write(&self, offset: usize, data: &[u8]) {
        assert!(offset
            .checked_add(data.len())
            .is_some_and(|end| end <= self.len));
        // SAFETY: producer owns the destination until tail release; the consumer
        // cannot write these bytes. Header writes occur before FD export.
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.ptr.as_ptr().add(offset), data.len());
        }
    }
    fn atomic(&self, offset: usize) -> &AtomicU64 {
        assert_eq!(offset % size_of::<AtomicU64>(), 0);
        assert!(offset
            .checked_add(size_of::<AtomicU64>())
            .is_some_and(|end| end <= self.len));
        // SAFETY: mmap is aligned and initialized to zero before exposure.
        unsafe { &*(self.ptr.as_ptr().add(offset) as *const AtomicU64) }
    }
}
impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: ptr/len came from one successful mmap and this is its sole owner.
        unsafe {
            libc::munmap(self.ptr.as_ptr().cast(), self.len);
        }
    }
}

struct ProducerInner {
    map: Mapping,
    fd: File,
    event: File,
    meta: Metadata,
    state: Mutex<ProducerState>,
    shutdown: Mutex<Option<UnixStream>>,
}
struct ProducerState {
    epoch: u64,
    seq: u64,
    last_head: u64,
}
#[derive(Clone)]
pub struct QueueProducer {
    inner: Arc<ProducerInner>,
}
pub struct Reservation<'a> {
    producer: &'a QueueProducer,
    state: MutexGuard<'a, ProducerState>,
    ticket: u64,
}

impl QueueProducer {
    pub fn create(meta: &Metadata) -> Result<Self, QueueError> {
        meta.validate()?;
        let name = std::ffi::CString::new("caraxes-local-irq").unwrap();
        // SAFETY: libc returns owned descriptors or -1.
        let raw = unsafe {
            libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING)
        };
        if raw < 0 {
            return Err(io::Error::last_os_error().into());
        }
        // SAFETY: a successful memfd_create transfers ownership of this fresh fd.
        let fd = unsafe { File::from_raw_fd(raw) };
        // SAFETY: fd is live and QUEUE_BYTES fits off_t on supported 64-bit hosts.
        if unsafe { libc::ftruncate(fd.as_raw_fd(), QUEUE_BYTES as libc::off_t) } < 0 {
            return Err(io::Error::last_os_error().into());
        }
        // SAFETY: fd is live; these seals prevent later truncation or extension.
        if unsafe {
            libc::fcntl(
                fd.as_raw_fd(),
                libc::F_ADD_SEALS,
                libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL,
            )
        } < 0
        {
            return Err(io::Error::last_os_error().into());
        }
        // SAFETY: eventfd has no borrowed pointers and returns a new fd or -1.
        let eraw = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if eraw < 0 {
            return Err(io::Error::last_os_error().into());
        }
        // SAFETY: a successful eventfd call transfers ownership of this fresh fd.
        let event = unsafe { File::from_raw_fd(eraw) };
        let map = Mapping::new(&fd, 0, QUEUE_BYTES, true)?;
        // The producer only reads the consumer ticket. This cooperative mapping
        // guard does not prevent a peer holding O_RDWR from remapping the memfd.
        // SAFETY: map begins page-aligned and contains at least the first page.
        if unsafe { libc::mprotect(map.ptr.as_ptr().cast(), TAIL_PAGE, libc::PROT_READ) } < 0 {
            return Err(io::Error::last_os_error().into());
        }
        map.write(TAIL_PAGE + 48, MAGIC);
        map.write(TAIL_PAGE + 52, &VERSION.to_le_bytes());
        map.write(NONCE, &meta.session_nonce);
        map.write(EPOCH, &meta.epoch.to_le_bytes());
        Ok(Self {
            inner: Arc::new(ProducerInner {
                map,
                fd,
                event,
                meta: meta.clone(),
                state: Mutex::new(ProducerState {
                    epoch: meta.epoch,
                    seq: 1,
                    last_head: 0,
                }),
                shutdown: Mutex::new(None),
            }),
        })
    }
    pub fn metadata(&self) -> &Metadata {
        &self.inner.meta
    }
    pub fn queue_fd(&self) -> io::Result<File> {
        self.inner.fd.try_clone()
    }
    pub fn event_fd(&self) -> io::Result<File> {
        self.inner.event.try_clone()
    }
    pub fn attach_terminal_stream(&self, stream: &UnixStream) -> io::Result<()> {
        *self.inner.shutdown.lock().unwrap() = Some(stream.try_clone()?);
        Ok(())
    }
    pub fn published_tail(&self) -> u64 {
        self.inner.map.atomic(TAIL_PAGE).load(Ordering::Acquire)
    }
    pub fn is_terminal(&self) -> bool {
        self.inner.map.atomic(FAULT).load(Ordering::Acquire) != 0
    }
    pub fn terminal(&self) {
        self.inner.map.atomic(FAULT).store(1, Ordering::Release);
        let _ = wake(&self.inner.event);
        if let Some(s) = self.inner.shutdown.lock().unwrap().as_ref() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }
    pub fn reserve(&self) -> Result<Reservation<'_>, QueueError> {
        let mut state = self.inner.state.lock().unwrap();
        if self.is_terminal() {
            return Err(QueueError::Terminal);
        }
        let head = self.inner.map.atomic(0).load(Ordering::Acquire);
        let tail = self.published_tail();
        if head < state.last_head || head > tail || tail - head > CAPACITY {
            self.terminal();
            return Err(QueueError::Invalid("head/tail"));
        }
        state.last_head = head;
        if tail - head == CAPACITY {
            self.terminal();
            return Err(QueueError::Full);
        }
        if tail == u64::MAX {
            self.terminal();
            return Err(QueueError::Invalid("ticket overflow"));
        }
        Ok(Reservation {
            producer: self,
            state,
            ticket: tail + 1,
        })
    }
}
impl Reservation<'_> {
    pub fn publish(mut self, kind: RecordKind) -> Result<u64, QueueError> {
        let p = &self.producer.inner;
        if self.producer.is_terminal() {
            return Err(QueueError::Terminal);
        }
        let ticket = self.ticket;
        if self.state.seq == u64::MAX {
            self.producer.terminal();
            return Err(QueueError::Invalid("sequence overflow"));
        }
        let mut b = [0u8; SLOT_SIZE];
        b[0..4].copy_from_slice(MAGIC);
        put16(&mut b[4..6], VERSION);
        b[16..32].copy_from_slice(&p.meta.session_nonce);
        put64(&mut b[32..40], self.state.epoch);
        put64(&mut b[40..48], self.state.seq);
        put64(&mut b[48..56], ticket);
        let reset = match kind {
            RecordKind::Fold(v) => {
                put16(&mut b[6..8], 1);
                put16(&mut b[8..10], 32);
                for (i, x) in v.iter().enumerate() {
                    put32(&mut b[56 + i * 4..60 + i * 4], *x);
                }
                None
            }
            RecordKind::Hints(v) => {
                put16(&mut b[6..8], 2);
                put16(&mut b[8..10], 32);
                for (i, x) in v.iter().enumerate() {
                    put32(&mut b[56 + i * 4..60 + i * 4], *x);
                }
                None
            }
            RecordKind::MsiEnabled(v) => {
                put16(&mut b[6..8], 3);
                put16(&mut b[8..10], 1);
                b[56] = u8::from(v);
                None
            }
            RecordKind::ResetCommit {
                new_epoch,
                reset_ticket,
            } => {
                if new_epoch
                    != self
                        .state
                        .epoch
                        .checked_add(1)
                        .ok_or(QueueError::Invalid("epoch overflow"))?
                    || (reset_ticket != 0 && reset_ticket != ticket)
                {
                    self.producer.terminal();
                    return Err(QueueError::Invalid("reset"));
                }
                put16(&mut b[6..8], 4);
                put16(&mut b[8..10], 16);
                put64(&mut b[56..64], new_epoch);
                put64(&mut b[64..72], ticket);
                Some(new_epoch)
            }
        };
        let off = RECORDS + ((ticket - 1) % CAPACITY) as usize * SLOT_SIZE;
        p.map.write(off, &b);
        p.map.atomic(TAIL_PAGE).store(ticket, Ordering::Release);
        if let Some(epoch) = reset {
            self.state.epoch = epoch;
            self.state.seq = 1;
        } else {
            self.state.seq += 1;
        }
        if let Err(e) = wake(&p.event) {
            self.producer.terminal();
            return Err(e.into());
        }
        Ok(ticket)
    }
}

pub struct QueueConsumer {
    read: Mapping,
    head: Mapping,
    _fd: File,
    meta: Metadata,
    ticket: u64,
    epoch: u64,
    seq: u64,
}
impl QueueConsumer {
    pub fn map(prepared: &Prepared) -> Result<Self, QueueError> {
        prepared.metadata.validate()?;
        let len = prepared.queue_fd.metadata()?.len();
        if len != QUEUE_BYTES as u64 {
            return Err(QueueError::Invalid("queue size"));
        }
        // SAFETY: this is a live descriptor; F_GET_SEALS has no pointer argument.
        let seals = unsafe { libc::fcntl(prepared.queue_fd.as_raw_fd(), libc::F_GET_SEALS) };
        if seals < 0 {
            return Err(io::Error::last_os_error().into());
        }
        if seals & (libc::F_SEAL_GROW | libc::F_SEAL_SHRINK)
            != (libc::F_SEAL_GROW | libc::F_SEAL_SHRINK)
        {
            return Err(QueueError::Invalid("queue seals"));
        }
        // SAFETY: the event descriptor is live; F_GETFL has no pointer argument.
        let event_flags = unsafe { libc::fcntl(prepared.event_fd.as_raw_fd(), libc::F_GETFL) };
        if event_flags < 0 {
            return Err(io::Error::last_os_error().into());
        }
        if event_flags & libc::O_NONBLOCK == 0 {
            return Err(QueueError::Invalid("blocking notification fd"));
        }
        let read = Mapping::new(&prepared.queue_fd, 0, QUEUE_BYTES, false)?;
        let head = Mapping::new(&prepared.queue_fd, 0, 4096, true)?;
        if read.bytes(TAIL_PAGE + 48, 4) != MAGIC
            || get16(read.bytes(TAIL_PAGE + 52, 2)) != VERSION
            || read.bytes(NONCE, 16) != prepared.metadata.session_nonce
            || get64(read.bytes(EPOCH, 8)) != prepared.metadata.epoch
            || read.bytes(8, TAIL_PAGE - 8).iter().any(|v| *v != 0)
            || read.bytes(TAIL_PAGE + 40, 8).iter().any(|v| *v != 0)
            || read
                .bytes(TAIL_PAGE + 54, TAIL_PAGE - 54)
                .iter()
                .any(|v| *v != 0)
            || read.atomic(0).load(Ordering::Acquire) != 0
            || read.atomic(TAIL_PAGE).load(Ordering::Acquire) != 0
            || read.atomic(FAULT).load(Ordering::Acquire) != 0
        {
            return Err(QueueError::Invalid("queue header"));
        }
        Ok(Self {
            read,
            head,
            _fd: prepared.queue_fd.try_clone()?,
            meta: prepared.metadata.clone(),
            ticket: 0,
            epoch: prepared.metadata.epoch,
            seq: 1,
        })
    }
    pub fn consumed_ticket(&self) -> u64 {
        self.ticket
    }
    pub fn published_tail(&self) -> Result<u64, QueueError> {
        if self.read.atomic(FAULT).load(Ordering::Acquire) != 0 {
            return Err(QueueError::Terminal);
        }
        if self.read.atomic(0).load(Ordering::Acquire) != self.ticket {
            return Err(QueueError::Invalid("head ticket"));
        }
        let tail = self.read.atomic(TAIL_PAGE).load(Ordering::Acquire);
        if tail < self.ticket || tail - self.ticket > CAPACITY {
            return Err(QueueError::Invalid("tail"));
        }
        Ok(tail)
    }
    pub fn drain_through(&mut self, target: u64) -> Result<Vec<Record>, QueueError> {
        let tail = self.published_tail()?;
        if target > tail || target < self.ticket {
            return Err(QueueError::Invalid("target ticket"));
        }
        let mut out = Vec::with_capacity((target - self.ticket) as usize);
        while self.ticket < target {
            let ticket = self.ticket + 1;
            let off = RECORDS + ((ticket - 1) % CAPACITY) as usize * SLOT_SIZE;
            let b = self.read.bytes(off, SLOT_SIZE);
            if &b[0..4] != MAGIC
                || get16(&b[4..6]) != VERSION
                || b[10..16] != [0; 6]
                || b[16..32] != self.meta.session_nonce
                || get64(&b[32..40]) != self.epoch
                || get64(&b[40..48]) != self.seq
                || get64(&b[48..56]) != ticket
            {
                return Err(QueueError::Invalid("record envelope"));
            }
            let len = get16(&b[8..10]);
            let (kind, used) = match (get16(&b[6..8]), len) {
                (1, 32) | (2, 32) => {
                    let mut v = [0; 8];
                    for (i, x) in v.iter_mut().enumerate() {
                        *x = get32(&b[56 + i * 4..60 + i * 4]);
                    }
                    if get16(&b[6..8]) == 1 {
                        (RecordKind::Fold(v), 88)
                    } else {
                        (RecordKind::Hints(v), 88)
                    }
                }
                (3, 1) if b[56] <= 1 => (RecordKind::MsiEnabled(b[56] != 0), 57),
                (4, 16) => {
                    let new_epoch = get64(&b[56..64]);
                    let reset_ticket = get64(&b[64..72]);
                    if new_epoch
                        != self
                            .epoch
                            .checked_add(1)
                            .ok_or(QueueError::Invalid("epoch overflow"))?
                        || reset_ticket != ticket
                    {
                        return Err(QueueError::Invalid("reset"));
                    }
                    (
                        RecordKind::ResetCommit {
                            new_epoch,
                            reset_ticket,
                        },
                        72,
                    )
                }
                _ => return Err(QueueError::Invalid("record kind/length")),
            };
            if b[used..].iter().any(|x| *x != 0) {
                return Err(QueueError::Invalid("record reserved"));
            }
            let record = Record {
                epoch: self.epoch,
                seq: self.seq,
                kind,
            };
            if let RecordKind::ResetCommit { new_epoch, .. } = record.kind {
                self.epoch = new_epoch;
                self.seq = 1;
            } else {
                self.seq = self
                    .seq
                    .checked_add(1)
                    .ok_or(QueueError::Invalid("sequence overflow"))?;
            }
            self.ticket = ticket;
            self.head.atomic(0).store(ticket, Ordering::Release);
            out.push(record);
        }
        Ok(out)
    }
}
fn wake(event: &File) -> io::Result<()> {
    let b = 1u64.to_ne_bytes();
    // SAFETY: b is an initialized 8-byte buffer and event is a live eventfd.
    let n = unsafe { libc::write(event.as_raw_fd(), b.as_ptr().cast(), 8) };
    if n == 8 {
        Ok(())
    } else if n < 0 {
        Err(io::Error::last_os_error())
    } else {
        Err(io::ErrorKind::WriteZero.into())
    }
}
fn put16(b: &mut [u8], v: u16) {
    b.copy_from_slice(&v.to_le_bytes())
}
fn put32(b: &mut [u8], v: u32) {
    b.copy_from_slice(&v.to_le_bytes())
}
fn put64(b: &mut [u8], v: u64) {
    b.copy_from_slice(&v.to_le_bytes())
}
fn get16(b: &[u8]) -> u16 {
    u16::from_le_bytes(b.try_into().unwrap())
}
fn get32(b: &[u8]) -> u32 {
    u32::from_le_bytes(b.try_into().unwrap())
}
fn get64(b: &[u8]) -> u64 {
    u64::from_le_bytes(b.try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> Metadata {
        Metadata {
            token: 7,
            session_nonce: [0x5a; 16],
            epoch: 1,
            bar_index: 0,
            tree_offset: 0xb81000,
            page_size: 4096,
            rearm_offset: 0xb82000,
            msi_index: 1,
            msi_count: 1,
            hints: [0x8000_0000; 8],
            watchdog_period_us: 1000,
            msi_enabled: true,
            initial_ticket: 0,
        }
    }
    fn pair() -> (QueueProducer, QueueConsumer) {
        let meta = metadata();
        let p = QueueProducer::create(&meta).unwrap();
        let prepared = Prepared {
            metadata: meta,
            queue_fd: p.queue_fd().unwrap(),
            event_fd: p.event_fd().unwrap(),
        };
        let c = QueueConsumer::map(&prepared).unwrap();
        (p, c)
    }

    #[test]
    fn prepare_wire_roundtrip_preserves_nonzero_watchdog_and_enabled() {
        let meta = metadata();
        assert_eq!(Metadata::decode(&meta.wire()).unwrap(), meta);
        let mut bad = meta.wire();
        bad[117] = 1;
        assert!(Metadata::decode(&bad).is_err());
    }

    #[test]
    fn queue_wrap_preserves_whole_records_and_reset_sequence() {
        let (p, mut c) = pair();
        for i in 0..CAPACITY + 3 {
            let kind = if i == CAPACITY - 1 {
                RecordKind::ResetCommit {
                    new_epoch: 2,
                    reset_ticket: 0,
                }
            } else {
                RecordKind::Fold([i as u32; 8])
            };
            let ticket = p.reserve().unwrap().publish(kind).unwrap();
            assert_eq!(ticket, i + 1);
            let records = c.drain_through(c.published_tail().unwrap()).unwrap();
            assert_eq!(records.len(), 1);
            assert_eq!(c.consumed_ticket(), ticket);
            if i == CAPACITY - 1 {
                assert_eq!(
                    records[0].kind,
                    RecordKind::ResetCommit {
                        new_epoch: 2,
                        reset_ticket: ticket
                    }
                );
                assert_eq!(records[0].epoch, 1);
            }
            if i == CAPACITY {
                assert_eq!((records[0].epoch, records[0].seq), (2, 1));
            }
        }
    }

    #[test]
    fn full_queue_is_terminal_without_overwriting_unread_fold() {
        let (p, c) = pair();
        for i in 0..CAPACITY {
            p.reserve()
                .unwrap()
                .publish(RecordKind::Fold([i as u32; 8]))
                .unwrap();
        }
        assert_eq!(p.published_tail(), CAPACITY);
        assert!(matches!(p.reserve(), Err(QueueError::Full)));
        assert!(p.is_terminal());
        assert!(matches!(c.published_tail(), Err(QueueError::Terminal)));
    }

    #[test]
    fn corrupted_record_or_session_is_rejected() {
        let (p, mut c) = pair();
        p.reserve()
            .unwrap()
            .publish(RecordKind::Hints([1; 8]))
            .unwrap();
        p.inner.map.write(RECORDS + 40, &99u64.to_le_bytes());
        assert!(matches!(
            c.drain_through(1),
            Err(QueueError::Invalid("record envelope"))
        ));
        let mut wrong = metadata();
        wrong.session_nonce = [1; 16];
        let prepared = Prepared {
            metadata: wrong,
            queue_fd: p.queue_fd().unwrap(),
            event_fd: p.event_fd().unwrap(),
        };
        assert!(matches!(
            QueueConsumer::map(&prepared),
            Err(QueueError::Invalid("queue header"))
        ));
    }

    #[test]
    fn invalid_consumer_head_stops_producer_before_draining_pending() {
        let (p, _c) = pair();
        // Use a separate RW map only to simulate a corrupted cooperating peer.
        let writer = Mapping::new(&p.queue_fd().unwrap(), 0, 4096, true).unwrap();
        writer.atomic(0).store(9, Ordering::Release);
        assert!(matches!(p.reserve(), Err(QueueError::Invalid("head/tail"))));
        assert_eq!(p.published_tail(), 0);
        assert!(p.is_terminal());
    }

    #[test]
    fn consumer_refuses_blocking_notification_descriptor() {
        let meta = metadata();
        let p = QueueProducer::create(&meta).unwrap();
        let event_fd = p.event_fd().unwrap();
        // SAFETY: this test deliberately changes the flags of its own eventfd.
        let flags = unsafe { libc::fcntl(event_fd.as_raw_fd(), libc::F_GETFL) };
        assert!(flags >= 0);
        // SAFETY: F_SETFL changes only status flags of this valid descriptor.
        assert_eq!(
            unsafe {
                libc::fcntl(
                    event_fd.as_raw_fd(),
                    libc::F_SETFL,
                    flags & !libc::O_NONBLOCK,
                )
            },
            0
        );
        let prepared = Prepared {
            metadata: meta,
            queue_fd: p.queue_fd().unwrap(),
            event_fd,
        };
        assert!(matches!(
            QueueConsumer::map(&prepared),
            Err(QueueError::Invalid("blocking notification fd"))
        ));
    }
}

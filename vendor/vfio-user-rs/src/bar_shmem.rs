// Copyright © 2026 Caraxes contributors
// SPDX-License-Identifier: Apache-2.0
//
// Private, opt-in, same-host Linux/x86_64 vfio-user transport. This page is
// host-only: it is never a guest BAR or DMA mapping. Both peers are trusted.
// All shared words are native-endian AtomicU64; VERSION limits this layout to
// the cooperating x86_64 peers. One Client (&mut self) owns one request slot.

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::net::UnixStream;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const PAGE_BYTES: usize = 4096;
pub const PREPARE_BYTES: usize = 64;
pub const TIMEOUT: Duration = Duration::from_secs(30);
const MAGIC: &[u8; 4] = b"BSHM";
const VERSION: u16 = 1;
const IDLE: u64 = 0;
const REQUEST: u64 = 1;
const REPLY: u64 = 2;
const FAULT: u64 = 3;
const STATE: usize = 0;
const SEQ: usize = 8;
const NONCE0: usize = 16;
const NONCE1: usize = 24;
const OP: usize = 32;
const OFFSET: usize = 40;
const LEN: usize = 48;
const VALUE: usize = 56;
const STATUS: usize = 64;
const REPLY_SEQ: usize = 72;

fn invalid(what: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub nonce: [u8; 16],
    pub bar0_size: u64,
}
impl Metadata {
    pub fn fresh(bar0_size: u64) -> io::Result<Self> {
        if bar0_size == 0 {
            return Err(invalid("zero BAR0"));
        }
        let mut nonce = [0u8; 16];
        let mut done = 0;
        while done < nonce.len() {
            // SAFETY: the remaining nonce slice is writable for this call.
            let n = unsafe {
                libc::getrandom(nonce[done..].as_mut_ptr().cast(), nonce.len() - done, 0)
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
        if nonce == [0; 16] {
            return Err(invalid("zero nonce"));
        }
        Ok(Self { nonce, bar0_size })
    }
    pub fn encode(self) -> [u8; PREPARE_BYTES] {
        let mut b = [0u8; PREPARE_BYTES];
        b[..4].copy_from_slice(MAGIC);
        b[4..6].copy_from_slice(&VERSION.to_le_bytes());
        b[8..24].copy_from_slice(&self.nonce);
        b[24..32].copy_from_slice(&self.bar0_size.to_le_bytes());
        b[32..40].copy_from_slice(&(PAGE_BYTES as u64).to_le_bytes());
        b[40..48].copy_from_slice(&(TIMEOUT.as_millis() as u64).to_le_bytes());
        b
    }
    pub fn decode(b: &[u8]) -> io::Result<Self> {
        if b.len() != PREPARE_BYTES
            || &b[..4] != MAGIC
            || u16::from_le_bytes(b[4..6].try_into().unwrap()) != VERSION
            || b[6..8] != [0; 2]
            || b[48..].iter().any(|x| *x != 0)
            || u64::from_le_bytes(b[32..40].try_into().unwrap()) != PAGE_BYTES as u64
            || u64::from_le_bytes(b[40..48].try_into().unwrap()) != TIMEOUT.as_millis() as u64
        {
            return Err(invalid("BAR shared prepare metadata"));
        }
        let mut nonce = [0; 16];
        nonce.copy_from_slice(&b[8..24]);
        let bar0_size = u64::from_le_bytes(b[24..32].try_into().unwrap());
        if nonce == [0; 16] || bar0_size == 0 {
            return Err(invalid("BAR shared nonce or size"));
        }
        Ok(Self { nonce, bar0_size })
    }
    fn words(self) -> (u64, u64) {
        (
            u64::from_le_bytes(self.nonce[..8].try_into().unwrap()),
            u64::from_le_bytes(self.nonce[8..].try_into().unwrap()),
        )
    }
}

struct Mapping {
    ptr: NonNull<u8>,
}
// SAFETY: all shared accesses use aligned AtomicU64 words. The mapping stays
// live while either channel owns it; the single-slot state protocol serializes
// request/reply writers across processes. No non-atomic shared bytes are read.
unsafe impl Send for Mapping {}
// SAFETY: same atomic-only and state ownership invariant as Send.
unsafe impl Sync for Mapping {}
impl Mapping {
    fn map(fd: &File) -> io::Result<Self> {
        if fd.metadata()?.len() != PAGE_BYTES as u64 {
            return Err(invalid("BAR shared page size"));
        }
        let seals = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GET_SEALS) };
        if seals < 0 {
            return Err(io::Error::last_os_error());
        }
        if seals & (libc::F_SEAL_GROW | libc::F_SEAL_SHRINK)
            != libc::F_SEAL_GROW | libc::F_SEAL_SHRINK
        {
            return Err(invalid("BAR shared page seals"));
        }
        // SAFETY: fd has been sized and sealed against resizing; mmap return is checked.
        let p = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                PAGE_BYTES,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd.as_raw_fd(),
                0,
            )
        };
        if p == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            ptr: NonNull::new(p.cast()).unwrap(),
        })
    }
    fn word(&self, offset: usize) -> &AtomicU64 {
        debug_assert!(offset % 8 == 0 && offset + 8 <= PAGE_BYTES);
        // SAFETY: mmap is page aligned; offsets are private fixed aligned words.
        unsafe { &*self.ptr.as_ptr().add(offset).cast::<AtomicU64>() }
    }
    fn fault(&self) {
        self.word(STATE).store(FAULT, Ordering::Release);
    }
}
impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: this mapping owns exactly PAGE_BYTES from mmap.
        unsafe {
            libc::munmap(self.ptr.as_ptr().cast(), PAGE_BYTES);
        }
    }
}

fn eventfd() -> io::Result<File> {
    let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful eventfd returned a newly owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn check_eventfd(fd: &File) -> io::Result<()> {
    let target = std::fs::read_link(format!("/proc/self/fd/{}", fd.as_raw_fd()))?;
    if target.to_string_lossy() != "anon_inode:[eventfd]" {
        return Err(invalid("BAR descriptor is not eventfd"));
    }
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if flags & libc::O_NONBLOCK == 0 {
        return Err(invalid("blocking BAR eventfd"));
    }
    Ok(())
}
fn eventfd_id(fd: &File) -> io::Result<u64> {
    let info = std::fs::read_to_string(format!("/proc/self/fdinfo/{}", fd.as_raw_fd()))?;
    let value = info.lines().find_map(|line| {
        line.strip_prefix("eventfd-id:")
            .and_then(|s| s.trim().parse::<u64>().ok())
    });
    value.ok_or_else(|| invalid("BAR eventfd identity"))
}
fn signal(fd: &File) -> io::Result<()> {
    let one = 1u64.to_ne_bytes();
    let n = unsafe { libc::write(fd.as_raw_fd(), one.as_ptr().cast(), 8) };
    if n != 8 {
        return Err(if n < 0 {
            io::Error::last_os_error()
        } else {
            invalid("short eventfd write")
        });
    }
    Ok(())
}
fn consume(fd: &File) -> io::Result<()> {
    let mut b = [0u8; 8];
    let n = unsafe { libc::read(fd.as_raw_fd(), b.as_mut_ptr().cast(), 8) };
    if n != 8 {
        return Err(if n < 0 {
            io::Error::last_os_error()
        } else {
            invalid("short eventfd read")
        });
    }
    if u64::from_ne_bytes(b) != 1 {
        return Err(invalid("BAR shared event count"));
    }
    Ok(())
}
fn poll_two(a: i32, b: i32, deadline: Instant) -> io::Result<(i16, i16)> {
    let mut p = [
        libc::pollfd {
            fd: a,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: b,
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        let timeout = remaining.as_millis().min(i32::MAX as u128).max(1) as i32;
        let n = unsafe { libc::poll(p.as_mut_ptr(), 2, timeout) };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        if n == 0 {
            return Err(io::ErrorKind::TimedOut.into());
        }
        return Ok((p[0].revents, p[1].revents));
    }
}
fn check_access(offset: u64, len: usize, size: u64) -> bool {
    matches!(len, 1 | 2 | 4 | 8)
        && offset
            .checked_add(len as u64)
            .is_some_and(|end| end <= size)
}

pub struct PreparedServer {
    pub metadata: Metadata,
    pub channel: ServerChannel,
    pub page_fd: File,
    pub request_fd: File,
    pub reply_fd: File,
}
pub fn prepare(bar0_size: u64) -> io::Result<PreparedServer> {
    let metadata = Metadata::fresh(bar0_size)?;
    let name = b"caraxes-bar-shmem\0";
    let raw = unsafe {
        libc::memfd_create(
            name.as_ptr().cast(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful memfd_create returned a newly owned descriptor.
    let page_fd = unsafe { File::from_raw_fd(raw) };
    if unsafe { libc::ftruncate(raw, PAGE_BYTES as libc::off_t) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe {
        libc::fcntl(
            raw,
            libc::F_ADD_SEALS,
            libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL,
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    let request_fd = eventfd()?;
    let reply_fd = eventfd()?;
    let channel = ServerChannel {
        map: Mapping::map(&page_fd)?,
        request_fd: request_fd.try_clone()?,
        reply_fd: reply_fd.try_clone()?,
        metadata,
        next_seq: 1,
    };
    Ok(PreparedServer {
        metadata,
        channel,
        page_fd,
        request_fd,
        reply_fd,
    })
}

pub struct ClientChannel {
    map: Mapping,
    request_fd: File,
    reply_fd: File,
    metadata: Metadata,
    next_seq: u64,
    faulted: bool,
}
impl ClientChannel {
    pub fn map(
        metadata: Metadata,
        page_fd: File,
        request_fd: File,
        reply_fd: File,
    ) -> io::Result<Self> {
        check_eventfd(&request_fd)?;
        check_eventfd(&reply_fd)?;
        if eventfd_id(&request_fd)? == eventfd_id(&reply_fd)? {
            return Err(invalid("duplicate BAR eventfd roles"));
        }
        let map = Mapping::map(&page_fd)?;
        if map.word(STATE).load(Ordering::Acquire) != IDLE
            || map.word(SEQ).load(Ordering::Acquire) != 0
            || map.word(REPLY_SEQ).load(Ordering::Acquire) != 0
        {
            return Err(invalid("BAR shared initial slot"));
        }
        Ok(Self {
            map,
            request_fd,
            reply_fd,
            metadata,
            next_seq: 1,
            faulted: false,
        })
    }
    pub fn eligible(&self, region: u32, offset: u64, len: usize) -> bool {
        region == 0 && check_access(offset, len, self.metadata.bar0_size)
    }
    pub fn faulted(&self) -> bool {
        self.faulted || self.map.word(STATE).load(Ordering::Acquire) == FAULT
    }
    fn terminal(&mut self, stream: &UnixStream) -> io::Error {
        self.faulted = true;
        self.map.fault();
        let _ = stream.shutdown(std::net::Shutdown::Both);
        invalid("terminal BAR shared transport")
    }
    pub fn access(
        &mut self,
        stream: &UnixStream,
        offset: u64,
        data: &mut [u8],
        write: bool,
    ) -> io::Result<()> {
        self.access_with_timeout(stream, offset, data, write, TIMEOUT)
    }
    fn access_with_timeout(
        &mut self,
        stream: &UnixStream,
        offset: u64,
        data: &mut [u8],
        write: bool,
        timeout: Duration,
    ) -> io::Result<()> {
        if self.faulted()
            || !check_access(offset, data.len(), self.metadata.bar0_size)
            || self.next_seq == 0
            || self.map.word(STATE).load(Ordering::Acquire) != IDLE
        {
            return Err(self.terminal(stream));
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.checked_add(1).unwrap_or(0);
        let (n0, n1) = self.metadata.words();
        let value = if write {
            let mut b = [0u8; 8];
            b[..data.len()].copy_from_slice(data);
            u64::from_le_bytes(b)
        } else {
            0
        };
        self.map.word(SEQ).store(seq, Ordering::Relaxed);
        self.map.word(NONCE0).store(n0, Ordering::Relaxed);
        self.map.word(NONCE1).store(n1, Ordering::Relaxed);
        self.map
            .word(OP)
            .store(if write { 2 } else { 1 }, Ordering::Relaxed);
        self.map.word(OFFSET).store(offset, Ordering::Relaxed);
        self.map
            .word(LEN)
            .store(data.len() as u64, Ordering::Relaxed);
        self.map.word(VALUE).store(value, Ordering::Relaxed);
        self.map.word(STATUS).store(0, Ordering::Relaxed);
        self.map.word(REPLY_SEQ).store(0, Ordering::Relaxed);
        if self
            .map
            .word(STATE)
            .compare_exchange(IDLE, REQUEST, Ordering::Release, Ordering::Acquire)
            .is_err()
        {
            return Err(self.terminal(stream));
        }
        if signal(&self.request_fd).is_err() {
            return Err(self.terminal(stream));
        }
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(self.terminal(stream));
            }
            let (sock, reply) =
                match poll_two(stream.as_raw_fd(), self.reply_fd.as_raw_fd(), deadline) {
                    Ok(v) => v,
                    Err(_) => return Err(self.terminal(stream)),
                };
            if reply & libc::POLLIN != 0 {
                if consume(&self.reply_fd).is_err() {
                    return Err(self.terminal(stream));
                }
                if self.map.word(STATE).load(Ordering::Acquire) != REPLY
                    || self.map.word(REPLY_SEQ).load(Ordering::Relaxed) != seq
                    || self.map.word(SEQ).load(Ordering::Relaxed) != seq
                    || self.map.word(NONCE0).load(Ordering::Relaxed) != n0
                    || self.map.word(NONCE1).load(Ordering::Relaxed) != n1
                {
                    return Err(self.terminal(stream));
                }
                let status = self.map.word(STATUS).load(Ordering::Relaxed);
                if status != 0 {
                    return Err(self.terminal(stream));
                }
                if !write {
                    data.copy_from_slice(
                        &self.map.word(VALUE).load(Ordering::Relaxed).to_le_bytes()[..data.len()],
                    );
                }
                if self
                    .map
                    .word(STATE)
                    .compare_exchange(REPLY, IDLE, Ordering::Release, Ordering::Acquire)
                    .is_err()
                {
                    return Err(self.terminal(stream));
                }
                return Ok(());
            }
            if sock != 0 || reply & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                return Err(self.terminal(stream));
            }
        }
    }
}

pub struct ServerChannel {
    map: Mapping,
    request_fd: File,
    reply_fd: File,
    metadata: Metadata,
    next_seq: u64,
}
impl ServerChannel {
    pub fn request_fd(&self) -> i32 {
        self.request_fd.as_raw_fd()
    }
    pub fn terminal(&self) {
        self.map.fault();
        let _ = signal(&self.reply_fd);
    }
    pub fn serve(
        &mut self,
        backend: &mut dyn super::ServerBackend,
        stream: &UnixStream,
        before_reply: bool,
    ) -> io::Result<()> {
        consume(&self.request_fd)?;
        if self.map.word(STATE).load(Ordering::Acquire) != REQUEST
            || self.next_seq == 0
            || self.map.word(SEQ).load(Ordering::Relaxed) != self.next_seq
        {
            self.terminal();
            return Err(invalid("BAR shared request state/seq"));
        }
        let (n0, n1) = self.metadata.words();
        if self.map.word(NONCE0).load(Ordering::Relaxed) != n0
            || self.map.word(NONCE1).load(Ordering::Relaxed) != n1
        {
            self.terminal();
            return Err(invalid("BAR shared nonce"));
        }
        let op = self.map.word(OP).load(Ordering::Relaxed);
        let offset = self.map.word(OFFSET).load(Ordering::Relaxed);
        let len = self.map.word(LEN).load(Ordering::Relaxed) as usize;
        if !matches!(op, 1 | 2) || !check_access(offset, len, self.metadata.bar0_size) {
            self.terminal();
            return Err(invalid("BAR shared access"));
        }
        let mut data = self.map.word(VALUE).load(Ordering::Relaxed).to_le_bytes();
        let result = if op == 1 {
            backend.region_read(0, offset, &mut data[..len])
        } else {
            backend.region_write(0, offset, &data[..len])
        };
        let fence = if before_reply {
            backend.local_irq_before_reply()
        } else {
            Ok(())
        };
        if result.is_err() || fence.is_err() {
            self.terminal();
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return Err(invalid("BAR shared backend/fence"));
        }
        self.map
            .word(VALUE)
            .store(u64::from_le_bytes(data), Ordering::Relaxed);
        self.map.word(STATUS).store(0, Ordering::Relaxed);
        self.map
            .word(REPLY_SEQ)
            .store(self.next_seq, Ordering::Relaxed);
        self.next_seq = self.next_seq.checked_add(1).unwrap_or(0);
        if self
            .map
            .word(STATE)
            .compare_exchange(REQUEST, REPLY, Ordering::Release, Ordering::Acquire)
            .is_err()
        {
            self.terminal();
            return Err(invalid("BAR shared client abandoned request"));
        }
        if let Err(e) = signal(&self.reply_fd) {
            self.terminal();
            return Err(e);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DmaMapFlags, DmaUnmapFlags, ServerBackend};

    struct Backend;
    impl ServerBackend for Backend {
        fn region_read(&mut self, _: u32, _: u64, data: &mut [u8]) -> io::Result<()> {
            data.fill(0x81);
            Ok(())
        }
        fn region_write(&mut self, _: u32, _: u64, _: &[u8]) -> io::Result<()> {
            Ok(())
        }
        fn dma_map(
            &mut self,
            _: DmaMapFlags,
            _: u64,
            _: u64,
            _: u64,
            _: Option<File>,
        ) -> io::Result<()> {
            Ok(())
        }
        fn dma_unmap(&mut self, _: DmaUnmapFlags, _: u64, _: u64) -> io::Result<()> {
            Ok(())
        }
        fn reset(&mut self) -> io::Result<()> {
            Ok(())
        }
        fn set_irqs(&mut self, _: u32, _: u32, _: u32, _: u32, _: Vec<File>) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn metadata_fd_roles_and_timeout_are_terminal() {
        let p = prepare(4096).unwrap();
        assert_eq!(Metadata::decode(&p.metadata.encode()).unwrap(), p.metadata);
        let wrong = File::open("/dev/null").unwrap();
        assert!(ClientChannel::map(
            p.metadata,
            p.page_fd.try_clone().unwrap(),
            wrong,
            p.reply_fd.try_clone().unwrap()
        )
        .is_err());
        assert!(ClientChannel::map(
            p.metadata,
            p.page_fd.try_clone().unwrap(),
            p.request_fd.try_clone().unwrap(),
            p.request_fd.try_clone().unwrap()
        )
        .is_err());
        let mut client = ClientChannel::map(
            p.metadata,
            p.page_fd.try_clone().unwrap(),
            p.request_fd.try_clone().unwrap(),
            p.reply_fd.try_clone().unwrap(),
        )
        .unwrap();
        let (stream, _peer) = UnixStream::pair().unwrap();
        let mut data = [0u8; 4];
        assert!(client
            .access_with_timeout(&stream, 0, &mut data, false, Duration::from_millis(5))
            .is_err());
        assert!(client.faulted());
    }
    #[test]
    fn wrong_sequence_nonce_and_overwritten_slot_refuse_without_backend_access() {
        for mode in 0..3 {
            let mut p = prepare(4096).unwrap();
            let (stream, _peer) = UnixStream::pair().unwrap();
            p.channel
                .map
                .word(SEQ)
                .store(if mode == 0 { 2 } else { 1 }, Ordering::Relaxed);
            let (n0, n1) = p.metadata.words();
            p.channel
                .map
                .word(NONCE0)
                .store(if mode == 1 { n0 ^ 1 } else { n0 }, Ordering::Relaxed);
            p.channel.map.word(NONCE1).store(n1, Ordering::Relaxed);
            p.channel.map.word(OP).store(1, Ordering::Relaxed);
            p.channel.map.word(OFFSET).store(0, Ordering::Relaxed);
            p.channel.map.word(LEN).store(4, Ordering::Relaxed);
            p.channel
                .map
                .word(STATE)
                .store(if mode == 2 { REPLY } else { REQUEST }, Ordering::Release);
            signal(&p.request_fd).unwrap();
            assert!(p.channel.serve(&mut Backend, &stream, false).is_err());
            assert_eq!(p.channel.map.word(STATE).load(Ordering::Acquire), FAULT);
        }
    }
}

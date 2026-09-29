// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Guest BAR2 as a memfd (`CARAXES_BAR2_MEMFD=1`, docs/caraxes/bar2-memfd.md).
//!
//! Region 3 (the BAR2 aperture) gets a memfd as its descriptor and sparse mmap areas over
//! all of it except the trapped pages ([`sparse_areas`]), so the guest RM's page-table,
//! instance-block and scrub writes through BAR2 land in host RAM without an exit. BAR2 is
//! a virtual aperture: guest BAR2 offset `o` is memfd offset `o`, and it reaches the FB
//! page the guest's BAR2 page tables translate `o` to. While an FB page is mapped through
//! BAR2 it lives in the memfd page(s) of the BAR2 offset(s) that map it ([`RemapTable`]);
//! [`crate::fb::FakeFb::backing`] is the one translation that says so, and the FB store
//! reads such a page from the memfd.
//!
//! The device cannot see the guest's writes through the VMM's mapping of the memfd (the
//! guest writes through KVM's stage-2 mapping of the VMM process's page tables, not
//! through the device's), so dirty tracking is a comparison of every mapped memfd page
//! against a snapshot of what the device last knew ([`RemapTable::scan`]), run at the guest
//! RM's own synchronisation points, which stay trapped: the BAR2 flush read
//! (`kbusFlushSingle_GV100`, kern_bus_gv100.c:392-395, reading `pReadToFlush`, the
//! persistent mapping at BAR2 0xfea000 in every trace) and the MMU invalidate
//! (`kbusUpdateRmAperture_GM107`, kern_bus_gm107.c:2757-2785). A sync writes the changed
//! bytes back to VRAM (only those bytes, so GPU writes to the rest of a page survive),
//! marks the changed FB pages dirty in the shadow's walk caches exactly as trapped writes
//! did (`FakeFb::note_pages_changed`), and follows changes of the BAR2 tables themselves
//! with a cached walk (ptcache.rs) of the BAR2 root: a page that leaves the mapping is
//! already written back, a page that enters it is loaded from VRAM into its memfd page.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::fb::{FbStore, Target};
use crate::guestmem::GuestMemory;
use crate::ptcache::{Leaf, TableReader, WalkCache, UNIT};

const PAGE: u64 = 4096;
const WORDS: usize = 512;
/// One 4 KiB page as qwords.
pub type PageBuf = [u64; WORDS];

/// The switch.
pub const ENV: &str = "CARAXES_BAR2_MEMFD";
/// Comma-separated BAR2 offsets of pages that stay trapped (hex or decimal, page
/// aligned), or `none`; default [`DEFAULT_TRAP`].
pub const TRAP_ENV: &str = "CARAXES_BAR2_MEMFD_TRAP";
/// `1`: sync before every doorbell as well (the reference for the doorbell rule).
pub const SYNC_DOORBELL_ENV: &str = "CARAXES_BAR2_MEMFD_SYNC_DOORBELL";
/// `0`: every trapped flush read syncs at once (the behaviour before
/// docs/caraxes/display-pacing.md). On by default: a flush read only marks the sync owed
/// ([`DeferredFlush`]); the next hand-over to the GPU (MMU invalidate, GSP command-queue
/// doorbell, channel doorbell) or the flush clock after [`DEFER_BOUND`] settles it.
pub const DEFER_FLUSH_ENV: &str = "CARAXES_BAR2_MEMFD_DEFER_FLUSH";
/// How long an owed flush may wait for a hand-over before the flush clock settles it.
pub const DEFER_BOUND: Duration = Duration::from_millis(1);
/// `memory`: the flush page is memory as well, no longer trapped (docs/caraxes/intr-page.md
/// §12.1); `trap` (the default) keeps it trapped. With it no flush read marks a sync owed,
/// so the owed flush stands from every sync on ([`DeferredFlush::standing`]): the
/// hand-overs and the flush clock settle it.
pub const FLUSH_PAGE_ENV: &str = "CARAXES_BAR2_FLUSH_PAGE";
/// The guest RM's BAR2 flush page: `pReadToFlush`, a persistent (top-down) BAR2 mapping
/// made once at boot (kern_bus_gv100.c:85-87); BAR2 0xfea000 in every device trace of
/// driver 610.57.04 (`caraxes/runs/mapping-2026-09-24/p3-profile/init-trace.txt`: 472
/// reads, the only BAR2 reads of a context creation; the display runs the same).
pub const DEFAULT_TRAP: u64 = 0xfe_a000;
/// Page key tag for a guest sysmem page (FB pages are plain page numbers).
const SYS: u64 = 1 << 63;
/// Leaves the BAR2 walk cache holds at most.
const MAX_LEAVES: usize = 1 << 20;

/// The switch's settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// BAR2 offsets of the trapped pages.
    pub traps: Vec<u64>,
    pub sync_doorbell: bool,
    /// Flush reads defer their sync to the next hand-over ([`DEFER_FLUSH_ENV`]).
    pub defer_flush: bool,
    /// `CARAXES_BAR2_FLUSH_PAGE=memory`: the flush page is not trapped; the owed flush
    /// stands from every sync on ([`DeferredFlush::standing`]).
    pub flush_page_memory: bool,
}

/// `Some` when `CARAXES_BAR2_MEMFD=1`; an error for a malformed trap list.
pub fn config() -> Result<Option<Config>, String> {
    if std::env::var(ENV).as_deref() != Ok("1") {
        return Ok(None);
    }
    config_from(
        std::env::var(TRAP_ENV).ok().as_deref(),
        std::env::var(SYNC_DOORBELL_ENV).as_deref() == Ok("1"),
        std::env::var(DEFER_FLUSH_ENV).as_deref() != Ok("0"),
        std::env::var(FLUSH_PAGE_ENV).ok().as_deref(),
    )
    .map(Some)
}

/// The settings from the variables' values (unset: `None`). Fail closed: the flush page as
/// memory refuses a trap list that still holds the flush page and a switched-off deferred
/// flush (without the flush clock nothing would settle what the guest wrote through BAR2
/// between hand-overs); with it the trap list defaults to none.
pub fn config_from(
    traps: Option<&str>,
    sync_doorbell: bool,
    defer_flush: bool,
    flush_page: Option<&str>,
) -> Result<Config, String> {
    let flush_page_memory = match flush_page.map(str::trim).unwrap_or("") {
        "" | "trap" => false,
        "memory" => true,
        v => return Err(format!("{FLUSH_PAGE_ENV}={v:?}: trap or memory")),
    };
    let traps = match traps {
        Some(v) => parse_traps(v)?,
        None if flush_page_memory => Vec::new(),
        None => vec![DEFAULT_TRAP],
    };
    if flush_page_memory {
        if traps.contains(&DEFAULT_TRAP) {
            return Err(format!(
                "{FLUSH_PAGE_ENV}=memory with {TRAP_ENV} holding the flush page {DEFAULT_TRAP:#x}"
            ));
        }
        if !defer_flush {
            return Err(format!(
                "{FLUSH_PAGE_ENV}=memory needs the deferred flush and its clock ({DEFER_FLUSH_ENV} not 0)"
            ));
        }
    }
    Ok(Config {
        traps,
        sync_doorbell,
        defer_flush,
        flush_page_memory,
    })
}

fn parse_traps(v: &str) -> Result<Vec<u64>, String> {
    let v = v.trim();
    if v.is_empty() || v == "none" {
        return Ok(Vec::new());
    }
    v.split(',')
        .map(|t| {
            let t = t.trim();
            let n = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                Some(h) => u64::from_str_radix(h, 16).ok(),
                None => t.parse().ok(),
            };
            n.filter(|n| n % PAGE == 0)
                .ok_or_else(|| format!("{TRAP_ENV}: {t:?} is not a page-aligned offset"))
        })
        .collect()
}

/// The sparse mmap areas `(offset, size)` of a region of `size` bytes with the pages at
/// `traps` left out (they stay trapped): the complement, page-aligned, ascending.
pub fn sparse_areas(size: u64, traps: &[u64]) -> Result<Vec<(u64, u64)>, String> {
    if size == 0 || size % PAGE != 0 {
        return Err(format!(
            "BAR2 size {size:#x} is not a whole number of pages"
        ));
    }
    let mut t: Vec<u64> = traps.to_vec();
    t.sort_unstable();
    t.dedup();
    if let Some(bad) = t.iter().find(|&&o| o % PAGE != 0 || o >= size) {
        return Err(format!(
            "trapped page {bad:#x} is not a page inside BAR2 [0, {size:#x})"
        ));
    }
    let mut out = Vec::new();
    let mut at = 0;
    for o in t {
        if o > at {
            out.push((at, o - at));
        }
        at = o + PAGE;
    }
    if at < size {
        out.push((at, size - at));
    }
    Ok(out)
}

/// A shared mapping of a memfd: the device's view, or (in the tests and the self-test) a
/// second view standing in for the VMM's.
pub struct SharedView {
    ptr: *mut u8,
    len: u64,
}

// SAFETY: the mapping is plain shared memory; the device model that owns it is used by one
// thread at a time (it sits behind the device mutex).
unsafe impl Send for SharedView {}

impl SharedView {
    fn map(fd: RawFd, len: u64) -> Result<Self, String> {
        // SAFETY: a fresh shared mapping of a file we hold, of its full size.
        let p = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if p == libc::MAP_FAILED {
            return Err(format!("mmap: {}", std::io::Error::last_os_error()));
        }
        Ok(SharedView { ptr: p.cast(), len })
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn at(&self, off: u64, n: usize) -> *mut u8 {
        assert!(
            off.checked_add(n as u64).is_some_and(|e| e <= self.len),
            "memfd access [{off:#x}, +{n:#x}) outside [0, {:#x})",
            self.len
        );
        // SAFETY: bounds checked above.
        unsafe { self.ptr.add(off as usize) }
    }

    /// Bytes at `off` (the guest may write concurrently, as with guest RAM, guestmem.rs).
    pub fn read(&self, off: u64, buf: &mut [u8]) {
        let p = self.at(off, buf.len());
        // SAFETY: in bounds; the source is shared memory another mapping may write.
        unsafe { std::ptr::copy_nonoverlapping(p, buf.as_mut_ptr(), buf.len()) };
    }

    pub fn write(&self, off: u64, data: &[u8]) {
        let p = self.at(off, data.len());
        // SAFETY: as in read().
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len()) };
    }

    /// The 512 qwords of page `page`.
    pub fn load_page(&self, page: u64, out: &mut PageBuf) {
        let p = self.at(page * PAGE, PAGE as usize).cast::<u64>();
        // SAFETY: page-aligned mapping, in bounds; shared memory the guest may write
        // concurrently, copied as guest RAM is (guestmem.rs). A qword caught half-written
        // is not lost: the snapshot then differs from the memfd, and the next scan writes
        // the final value back.
        unsafe { std::ptr::copy_nonoverlapping(p, out.as_mut_ptr(), WORDS) };
    }

    pub fn load_word(&self, page: u64, i: usize) -> u64 {
        let p = self.at(page * PAGE + 8 * i as u64, 8).cast::<u64>();
        // SAFETY: aligned and in bounds.
        unsafe { p.read_volatile() }
    }

    pub fn store_word(&self, page: u64, i: usize, v: u64) {
        let p = self.at(page * PAGE + 8 * i as u64, 8).cast::<u64>();
        // SAFETY: aligned and in bounds.
        unsafe { p.write_volatile(v) }
    }

    pub fn store_page(&self, page: u64, src: &PageBuf) {
        for (i, w) in src.iter().enumerate() {
            self.store_word(page, i, *w);
        }
    }
}

impl Drop for SharedView {
    fn drop(&mut self) {
        // SAFETY: the mapping made in map().
        unsafe { libc::munmap(self.ptr.cast(), self.len as usize) };
    }
}

/// The memfd behind region 3 and the device's own view of it.
pub struct MemfdArea {
    file: File,
    view: SharedView,
}

impl MemfdArea {
    /// A sealed memfd of `len` bytes (the client can neither shrink it, which would turn
    /// the device's accesses into SIGBUS, nor grow it) and the device's mapping.
    pub fn create(len: u64) -> Result<Self, String> {
        if len == 0 || len % PAGE != 0 {
            return Err(format!(
                "memfd size {len:#x} is not a whole number of pages"
            ));
        }
        // SAFETY: memfd_create with a static name; the fd is owned by the File below.
        let fd = unsafe {
            libc::memfd_create(
                c"caraxes-bar2".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
            )
        };
        if fd < 0 {
            return Err(format!("memfd_create: {}", std::io::Error::last_os_error()));
        }
        // SAFETY: fd is a fresh memfd owned from here on.
        let file = unsafe { File::from_raw_fd(fd) };
        file.set_len(len)
            .map_err(|e| format!("memfd size {len:#x}: {e}"))?;
        let seals = libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_SEAL;
        // SAFETY: fcntl on our own fd.
        if unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, seals) } != 0 {
            return Err(format!("memfd seals: {}", std::io::Error::last_os_error()));
        }
        let view = SharedView::map(fd, len)?;
        Ok(MemfdArea { file, view })
    }

    pub fn raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    pub fn view(&self) -> &SharedView {
        &self.view
    }

    /// Another shared mapping of the same memfd (what the VMM maps for the guest).
    pub fn map_again(&self) -> Result<SharedView, String> {
        SharedView::map(self.raw_fd(), self.view.len)
    }

    /// Fail closed (docs/caraxes/bar2-memfd.md §2): the dirty scan relies on a write
    /// through one shared mapping being visible through another. Check it on the last
    /// page with a second mapping, and leave the page zero.
    pub fn self_test(&self) -> Result<(), String> {
        let other = self.map_again()?;
        let page = self.view.len / PAGE - 1;
        let pat = 0x5ca1_ab1e_0bad_f00du64;
        other.store_word(page, 7, pat);
        let seen = self.view.load_word(page, 7);
        other.store_word(page, 7, 0);
        if seen != pat || self.view.load_word(page, 7) != 0 {
            return Err(format!(
                "a write through a second mapping of the memfd was not seen by the device's mapping ({seen:#x})"
            ));
        }
        Ok(())
    }
}

/// An FB page mapped through BAR2: the BAR2 pages (4 KiB page numbers) that map it, and
/// the device's snapshot of its content as of the last sync (what VRAM holds, plus the
/// device's own writes since).
pub struct Live {
    pub vas: Vec<u64>,
    pub snap: Box<PageBuf>,
}

/// The drift check's period (one mapped page per period, [`Bar2Area::check_drift`]).
pub const DRIFT_EVERY: std::time::Duration = std::time::Duration::from_millis(50);

/// Page key of a page-aligned target: FB page number, or a guest sysmem page tagged.
pub fn key_of(t: Target) -> Option<u64> {
    match t {
        Target::Fb(pa) => Some(pa / PAGE),
        Target::Sys(pa) => Some((pa / PAGE) | SYS),
        _ => None,
    }
}

/// The target of a page key.
pub fn target_of(key: u64) -> Target {
    if key & SYS != 0 {
        Target::Sys((key & !SYS) * PAGE)
    } else {
        Target::Fb(key * PAGE)
    }
}

/// What one [`RemapTable::set`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// The page that left the BAR2 mapping (its last BAR2 page went).
    pub left: Option<u64>,
    /// The page the BAR2 page now maps.
    pub entered: Option<u64>,
    /// `entered` was loaded from its backing (it was not mapped elsewhere in BAR2).
    pub loaded: bool,
}

/// What one [`RemapTable::scan`] found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// BAR2 pages compared.
    pub compared: usize,
    /// Keys whose content changed, ascending.
    pub dirty: Vec<u64>,
    /// Bytes written back.
    pub bytes: usize,
    /// Qwords in which two BAR2 aliases of one page changed a byte to different values
    /// (the first alias wins).
    pub conflicts: usize,
}

/// The remap table of the BAR2 memfd: which FB (or sysmem) page each BAR2 page maps, and
/// where each mapped page lives.
#[derive(Default)]
pub struct RemapTable {
    va: BTreeMap<u64, u64>,
    live: HashMap<u64, Live>,
}

impl RemapTable {
    /// The next live FB page key after `after` (wrapping), for the drift check's round robin.
    fn next_fb_key(&self, after: u64) -> Option<u64> {
        let fb = |k: &&u64| **k & SYS == 0;
        let above = self
            .live
            .keys()
            .filter(fb)
            .filter(|k| **k > after)
            .min()
            .copied();
        above.or_else(|| self.live.keys().filter(fb).min().copied())
    }

    /// Whether VRAM (`vram`, one page) differs from the snapshot of FB page `key`.
    fn drifted(&self, key: u64, vram: &[u8]) -> bool {
        let Some(l) = self.live.get(&key) else {
            return false;
        };
        l.snap
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .zip(vram)
            .any(|(a, b)| a != *b)
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// Pages mapped through BAR2.
    pub fn live_pages(&self) -> usize {
        self.live.len()
    }

    /// BAR2 pages that map something.
    pub fn mapped(&self) -> usize {
        self.va.len()
    }

    /// The key BAR2 page `va` maps.
    pub fn target(&self, va: u64) -> Option<u64> {
        self.va.get(&va).copied()
    }

    /// The BAR2 page that holds page `key` (its first alias), when mapped.
    pub fn home(&self, key: u64) -> Option<u64> {
        self.live.get(&key).map(|l| l.vas[0])
    }

    /// Every BAR2 page mapping `key`.
    pub fn aliases(&self, key: u64) -> &[u64] {
        self.live.get(&key).map_or(&[], |l| &l.vas)
    }

    /// The most BAR2 pages any one page has.
    pub fn max_aliases(&self) -> usize {
        self.live.values().map(|l| l.vas.len()).max().unwrap_or(0)
    }

    /// Point BAR2 page `va` at `new` (or nothing). A page mapped nowhere else is loaded
    /// with `load` and becomes the content of `va` in `mem`; a page already mapped
    /// through another BAR2 page gets that page's snapshot (an alias). A page whose last
    /// BAR2 page goes leaves the table; its content must have been written back by a
    /// [`Self::scan`] before (the sync always scans first).
    pub fn set(
        &mut self,
        va: u64,
        new: Option<u64>,
        mem: &SharedView,
        load: &mut dyn FnMut(u64) -> Box<PageBuf>,
    ) -> Change {
        let old = self.va.get(&va).copied();
        let mut ch = Change::default();
        if old == new {
            return ch;
        }
        if let Some(o) = old {
            self.va.remove(&va);
            if let Some(l) = self.live.get_mut(&o) {
                l.vas.retain(|&v| v != va);
                if l.vas.is_empty() {
                    self.live.remove(&o);
                    ch.left = Some(o);
                }
            }
        }
        if let Some(n) = new {
            self.va.insert(va, n);
            ch.entered = Some(n);
            match self.live.get_mut(&n) {
                Some(l) => {
                    mem.store_page(va, &l.snap);
                    l.vas.push(va);
                    l.vas.sort_unstable();
                }
                None => {
                    let snap = load(n);
                    mem.store_page(va, &snap);
                    self.live.insert(
                        n,
                        Live {
                            vas: vec![va],
                            snap,
                        },
                    );
                    ch.loaded = true;
                }
            }
        }
        ch
    }

    /// The dirty-page scan: compare every BAR2 page of every mapped page with the
    /// snapshot and merge what changed, byte by byte (across aliases the first change of a
    /// byte wins); copy the merged qwords to the other aliases, take the result as the new
    /// snapshot, and hand each run of changed bytes to `write_back(key, offset, bytes)`.
    /// Only bytes the guest changed are written back, so what the GPU wrote to the rest of
    /// the page in VRAM stays (the RM writes 32-bit values, `MEM_WR32`, next to others).
    pub fn scan(
        &mut self,
        mem: &SharedView,
        write_back: &mut dyn FnMut(u64, usize, &[u8]),
    ) -> Scan {
        let mut out = Scan::default();
        let mut buf: Box<PageBuf> = Box::new([0; WORDS]);
        let mut cmask: Box<PageBuf> = Box::new([0; WORDS]);
        let mut keys: Vec<u64> = self.live.keys().copied().collect();
        keys.sort_unstable();
        for key in keys {
            let l = self.live.get_mut(&key).expect("listed");
            let mut merged: Option<Box<PageBuf>> = None;
            cmask.fill(0);
            for &va in &l.vas {
                out.compared += 1;
                mem.load_page(va, &mut buf);
                if buf[..] == l.snap[..] {
                    continue;
                }
                let m = merged.get_or_insert_with(|| l.snap.clone());
                for i in 0..WORDS {
                    let d = buf[i] ^ l.snap[i];
                    if d == 0 {
                        continue;
                    }
                    let bm = byte_mask(d);
                    if (m[i] ^ buf[i]) & cmask[i] & bm != 0 {
                        out.conflicts += 1;
                    }
                    let take = bm & !cmask[i];
                    m[i] = (m[i] & !take) | (buf[i] & take);
                    cmask[i] |= bm;
                }
            }
            let Some(m) = merged else { continue };
            if l.vas.len() > 1 {
                for &va in &l.vas {
                    for i in (0..WORDS).filter(|&i| cmask[i] != 0) {
                        if mem.load_word(va, i) != m[i] {
                            mem.store_word(va, i, m[i]);
                        }
                    }
                }
            }
            l.snap = m;
            let bytes: Vec<u8> = l.snap.iter().flat_map(|w| w.to_le_bytes()).collect();
            let set = |b: usize| (cmask[b / 8] >> (8 * (b % 8))) & 0xFF != 0;
            let mut b = 0;
            while b < PAGE as usize {
                if !set(b) {
                    b += 1;
                    continue;
                }
                let a = b;
                while b < PAGE as usize && set(b) {
                    b += 1;
                }
                out.bytes += b - a;
                write_back(key, a, &bytes[a..b]);
            }
            out.dirty.push(key);
        }
        out
    }

    /// A device write of `data` at FB address `pa` (PRAMIN, the software executor, an
    /// RPC): into every BAR2 page of every mapped FB page it touches, and their snapshots,
    /// so that the guest sees it through BAR2 and the next scan does not take it for a
    /// guest write. The caller writes the store (VRAM) as well.
    pub fn device_write(&mut self, mem: &SharedView, pa: u64, data: &[u8]) -> bool {
        let mut hit = false;
        let mut done = 0usize;
        while done < data.len() {
            let a = pa + done as u64;
            let in_page = (a % PAGE) as usize;
            let n = (PAGE as usize - in_page).min(data.len() - done);
            if let Some(l) = self.live.get_mut(&(a / PAGE)) {
                hit = true;
                let piece = &data[done..done + n];
                for &va in &l.vas {
                    mem.write(va * PAGE + in_page as u64, piece);
                }
                page_bytes(&mut l.snap)[in_page..in_page + n].copy_from_slice(piece);
            }
            done += n;
        }
        hit
    }

    /// Forget every mapping (a function-level reset; the memfd content is left as it is).
    pub fn clear(&mut self) {
        self.va.clear();
        self.live.clear();
    }
}

/// 0xFF in every byte of `d` that is not zero.
fn byte_mask(d: u64) -> u64 {
    (0..8)
        .filter(|k| (d >> (8 * k)) & 0xFF != 0)
        .fold(0, |m, k| m | 0xFF << (8 * k))
}

/// The 4096 bytes of a page snapshot (little-endian qwords on x86).
fn page_bytes(a: &mut PageBuf) -> &mut [u8; 4096] {
    // SAFETY: [u64; 512] and [u8; 4096] have the same size, u8 has no alignment
    // requirement, and the borrow is carried over.
    unsafe { &mut *(a as *mut PageBuf as *mut [u8; 4096]) }
}

/// Where the store holds guest FB address `pa` now: the BAR1 pool's remap
/// ([`crate::fb::FakeFb::remap`]) when both are on, else `pa` itself. The memfd is keyed by
/// the guest FB address (what the guest's BAR2 tables name); every store access of this
/// file goes through the translation, so a page that is both behind BAR2 and in the pool
/// is read from and written back to its pool page (docs/caraxes/bar2-memfd.md §8).
pub type Translate<'a> = &'a dyn Fn(u64) -> u64;

/// No pool: the store holds every page at its guest address.
pub fn identity(pa: u64) -> u64 {
    pa
}

/// Read `[pa, pa + buf.len())` of the FB (guest addresses): mapped pages from their BAR2
/// page in the memfd, the rest from the store at `tr(address)`.
fn read_through(
    remap: &RemapTable,
    mem: &SharedView,
    store: &dyn FbStore,
    tr: Translate,
    pa: u64,
    buf: &mut [u8],
) {
    let mut done = 0usize;
    while done < buf.len() {
        let a = pa + done as u64;
        let in_page = a % PAGE;
        let n = (PAGE - in_page).min((buf.len() - done) as u64) as usize;
        match remap.home(a / PAGE) {
            Some(va) => mem.read(va * PAGE + in_page, &mut buf[done..done + n]),
            None => store.read(tr(a), &mut buf[done..done + n]),
        }
        done += n;
    }
}

/// Whether `[pa, pa + len)` touches `private` (fb.rs `FakeFb::private_overlaps`).
fn overlaps(private: Option<(u64, u64)>, pa: u64, len: u64) -> bool {
    private.is_some_and(|(b, e)| pa < e && b < pa.saturating_add(len))
}

/// The FB (through the remap) and guest RAM as the BAR2 walk's table reader.
struct Reader<'a> {
    remap: &'a RemapTable,
    mem: &'a SharedView,
    store: &'a dyn FbStore,
    tr: Translate<'a>,
    fb_size: u64,
    sys: Option<&'a GuestMemory>,
    private: Option<(u64, u64)>,
}

impl TableReader for Reader<'_> {
    fn block(&self, table: Target, n: usize) -> Option<Vec<u64>> {
        let mut b = vec![0u8; n * 8];
        match table {
            Target::Fb(pa)
                if pa + b.len() as u64 <= self.fb_size
                    && !overlaps(self.private, pa, b.len() as u64) =>
            {
                read_through(self.remap, self.mem, self.store, self.tr, pa, &mut b)
            }
            Target::Sys(pa) => {
                if !self.sys?.read(pa, &mut b) {
                    return None;
                }
            }
            _ => return None,
        }
        Some(
            b.chunks_exact(8)
                .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
                .collect(),
        )
    }
}

/// What settled an owed flush ([`Bar2Area::flush_settled`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Settle {
    /// A sync point that syncs anyway: the MMU invalidate, the GSP command-queue doorbell,
    /// or a flush read with the deferral off.
    SyncPoint,
    /// A channel doorbell (trapped, ioeventfd or token): the GPU may fetch what the guest
    /// wrote before it.
    Doorbell,
    /// The flush clock, [`DEFER_BOUND`] after the flush read, with no hand-over since.
    Clock,
}

/// A flush read whose sync is owed (docs/caraxes/display-pacing.md).
///
/// The guest RM reads the flush page after every batch of BAR2 writes (`kbusFlush`,
/// about 23 times per desktop frame, 11 or 12 per GPU VA change). Each sync compares
/// every mapped memfd page with its snapshot and merges the BAR1 pool's replicas, 0.2 to
/// 0.5 ms under the device lock; on hardware the read costs a microsecond. What the sync
/// makes visible is used by the GPU only after a hand-over the device sees (bar2-memfd.md
/// §3): page tables after the MMU invalidate, RPC data after the command-queue doorbell,
/// channel data after the channel doorbell. So the flush read only marks the sync owed
/// here, and the first of those hand-overs settles it; the flush clock settles it after
/// [`DEFER_BOUND`] for data a running engine polls without one. The device's own reads
/// see the guest's BAR2 bytes without a sync (they read the memfd, `read_through`).
#[derive(Default)]
pub struct DeferredFlush {
    pending: AtomicBool,
    /// The flush page is memory (`CARAXES_BAR2_FLUSH_PAGE=memory`): no read marks a sync
    /// owed, so one stands from every sync on and the clock settles it [`DEFER_BOUND`]
    /// after the last one.
    standing: bool,
    /// When the owed flush was marked, in nanoseconds since `epoch`.
    since_ns: AtomicU64,
    epoch: std::sync::OnceLock<Instant>,
    thread: Mutex<Option<std::thread::Thread>>,
}

impl DeferredFlush {
    fn now_ns(&self) -> u64 {
        self.epoch.get_or_init(Instant::now).elapsed().as_nanos() as u64
    }

    /// Mark a sync owed; returns whether none was owed before (the clock is woken then).
    pub fn mark(&self) -> bool {
        if self.pending.load(Ordering::Acquire) {
            return false;
        }
        self.since_ns.store(self.now_ns(), Ordering::Release);
        let first = !self.pending.swap(true, Ordering::AcqRel);
        if first {
            if let Some(t) = self.thread.lock().unwrap().as_ref() {
                t.unpark();
            }
        }
        first
    }

    /// Whether a sync is owed.
    pub fn pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }

    /// The owed flush of the flush page as memory (`CARAXES_BAR2_FLUSH_PAGE=memory`,
    /// docs/caraxes/intr-page.md §12.1): owed from the start and again from every sync on,
    /// since the guest may write BAR2 at any moment and no trapped read says when.
    pub fn standing() -> Self {
        let d = DeferredFlush {
            standing: true,
            ..Default::default()
        };
        d.since_ns.store(d.now_ns(), Ordering::Release);
        d.pending.store(true, Ordering::Release);
        d
    }

    /// The sync ran: nothing is owed now (standing: owed again from now on); returns
    /// whether something was.
    fn clear(&self) -> bool {
        if self.standing {
            self.since_ns.store(self.now_ns(), Ordering::Release);
            return self.pending.swap(true, Ordering::AcqRel);
        }
        self.pending.swap(false, Ordering::AcqRel)
    }

    /// How long the owed sync may still wait (zero when due, `None` when none is owed).
    pub fn due_in(&self) -> Option<Duration> {
        if !self.pending() {
            return None;
        }
        let since = self.since_ns.load(Ordering::Acquire);
        let age = self.now_ns().saturating_sub(since);
        Some(Duration::from_nanos(
            (DEFER_BOUND.as_nanos() as u64).saturating_sub(age),
        ))
    }

    /// Start the flush clock: a thread that calls `settle` (lock the device, settle the
    /// owed flush: `Device::settle_bar2_flush`) once an owed flush is [`DEFER_BOUND`] old.
    pub fn spawn_clock(self: &Arc<Self>, settle: Box<dyn Fn() + Send>) -> std::io::Result<()> {
        let me = self.clone();
        let h = std::thread::Builder::new()
            .name("bar2-flush".into())
            .spawn(move || loop {
                match me.due_in() {
                    // Parked until a flush read marks one (and at least every 100 ms, so a
                    // mark before the thread handle was stored is not lost).
                    None => std::thread::park_timeout(Duration::from_millis(100)),
                    Some(d) if !d.is_zero() => std::thread::sleep(d),
                    Some(_) => settle(),
                }
            })?;
        *self.thread.lock().unwrap() = Some(h.thread().clone());
        Ok(())
    }
}

/// Counters for the exit summary.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bar2Stats {
    pub syncs: u64,
    /// BAR2 pages compared over all scans.
    pub compared: u64,
    /// Pages found changed over all scans.
    pub dirty: u64,
    pub bytes_written_back: u64,
    pub conflicts: u64,
    /// Walks of the BAR2 tables (after a change of a BAR2 table page).
    pub walks: u64,
    pub entered: u64,
    pub left: u64,
    /// Pages loaded from VRAM (or guest RAM) into the memfd.
    pub loads: u64,
    /// Guest sysmem pages mapped through BAR2 whose write-back had no guest RAM at hand.
    pub sys_skipped: u64,
    pub max_live: u64,
    pub max_aliases: u64,
    pub sync_ns: u64,
    pub sync_ns_max: u64,
    /// Accesses of the trapped pages (the flush reads).
    pub trapped: u64,
    /// Device writes that landed in a mapped page.
    pub device_writes: u64,
    /// Flush reads that deferred their sync, and what settled the owed syncs.
    pub deferred: u64,
    pub settled_sync_point: u64,
    pub settled_doorbell: u64,
    pub settled_clock: u64,
    /// The sampled drift check ([`DRIFT_EVERY`]): FB pages compared with VRAM after a sync,
    /// those whose VRAM had changed behind the memfd (a GPU engine wrote a page the guest
    /// reads through BAR2: its reads are stale), and the first few of them.
    pub drift_checked: u64,
    pub drift_found: u64,
    pub drift_first: Vec<u64>,
}

/// What one sync changed, for the FB's caches.
#[derive(Clone, Debug, Default)]
pub struct SyncReport {
    /// FB page numbers whose content changed (guest writes found by the scan).
    pub dirty: Vec<u64>,
    pub entered: usize,
    pub left: usize,
}

/// The BAR2 memfd, its remap table and the cached walk of the BAR2 tables.
pub struct Bar2Area {
    pub mem: MemfdArea,
    size: u64,
    /// Trapped pages (page numbers), ascending.
    traps: Vec<u64>,
    pub sync_doorbell: bool,
    pub remap: RemapTable,
    cache: RefCell<Option<WalkCache>>,
    cache_root: Cell<u64>,
    pub stats: Bar2Stats,
    trapped: Cell<u64>,
    /// The owed flush when flush reads defer ([`Config::defer_flush`]).
    defer: Option<Arc<DeferredFlush>>,
    /// The drift check's round robin: the last FB page key checked, and when.
    drift_cursor: u64,
    drift_at: Option<std::time::Instant>,
    /// The device's private FB pages (devce.rs `PRIVATE_AT`): never mapped into the memfd
    /// nor read as a BAR2 table; a guest access there traps (fb.rs `bar_access` refuses it).
    private: Option<(u64, u64)>,
}

impl Bar2Area {
    /// The memfd for a BAR2 of `size` bytes with the trapped pages of `cfg`; fails when
    /// the memfd cannot be made, mapped or shown to see writes through another mapping.
    pub fn create(size: u64, cfg: &Config) -> Result<Self, String> {
        sparse_areas(size, &cfg.traps)?;
        let mem = MemfdArea::create(size)?;
        mem.self_test()?;
        let mut traps: Vec<u64> = cfg.traps.iter().map(|o| o / PAGE).collect();
        traps.sort_unstable();
        traps.dedup();
        Ok(Bar2Area {
            mem,
            size,
            traps,
            sync_doorbell: cfg.sync_doorbell,
            remap: RemapTable::default(),
            cache: RefCell::new(None),
            cache_root: Cell::new(u64::MAX),
            stats: Bar2Stats::default(),
            trapped: Cell::new(0),
            defer: cfg.defer_flush.then(|| {
                Arc::new(if cfg.flush_page_memory {
                    DeferredFlush::standing()
                } else {
                    DeferredFlush::default()
                })
            }),
            private: None,
            drift_cursor: 0,
            drift_at: None,
        })
    }

    /// See [`Bar2Area::private`]; set before the first sync.
    pub fn set_private(&mut self, private: Option<(u64, u64)>) {
        self.private = private;
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn raw_fd(&self) -> RawFd {
        self.mem.raw_fd()
    }

    /// The region's sparse mmap areas: all of BAR2 but the trapped pages.
    pub fn sparse_areas(&self) -> Vec<(u64, u64)> {
        let t: Vec<u64> = self.traps.iter().map(|p| p * PAGE).collect();
        sparse_areas(self.size, &t).expect("checked in create")
    }

    pub fn is_trap(&self, va_page: u64) -> bool {
        self.traps.binary_search(&va_page).is_ok()
    }

    pub fn has_live(&self) -> bool {
        !self.remap.is_empty()
    }

    /// The memfd offset holding FB address `pa`, when its page is mapped through BAR2.
    pub fn backing(&self, pa: u64) -> Option<u64> {
        self.remap.home(pa / PAGE).map(|va| va * PAGE + pa % PAGE)
    }

    /// Read guest FB `[pa, pa + buf.len())`: the memfd for pages behind BAR2 (by guest
    /// address), the store at `tr(address)` for the rest.
    pub fn read_through(&self, store: &dyn FbStore, tr: Translate, pa: u64, buf: &mut [u8]) {
        read_through(&self.remap, self.mem.view(), store, tr, pa, buf);
    }

    /// See [`RemapTable::device_write`]; `pa` is the guest FB address (before the pool's
    /// remap), the memfd's key.
    pub fn device_write(&mut self, pa: u64, data: &[u8]) {
        if self.remap.device_write(&self.mem.view, pa, data) {
            self.stats.device_writes += 1;
        }
    }

    /// A trapped write of known content reached the FB (PRAMIN, an RPC): the BAR2 walk
    /// cache patches its copy if it is a BAR2 table page.
    pub fn note_table_write(&self, pa: u64, data: &[u8]) {
        let mut c = self.cache.borrow_mut();
        let Some(c) = c.as_mut() else { return };
        let last = (pa + data.len().max(1) as u64 - 1) >> 12;
        for page in pa >> 12..=last {
            if c.watches(page) {
                c.note_write(page, pa, data);
            }
        }
    }

    /// The owed flush shared with the flush clock, when flush reads defer.
    pub fn deferred_flush(&self) -> Option<Arc<DeferredFlush>> {
        self.defer.clone()
    }

    /// A flush read: with the deferral on, mark the sync owed and return true (the caller
    /// skips it); otherwise false (the caller syncs now).
    pub fn defer_flush(&mut self) -> bool {
        match &self.defer {
            Some(d) => {
                d.mark();
                self.stats.deferred += 1;
                true
            }
            None => false,
        }
    }

    /// Whether a deferred flush read's sync is owed.
    pub fn flush_pending(&self) -> bool {
        self.defer.as_ref().is_some_and(|d| d.pending())
    }

    /// A flush point's sync ran (`BootModel::flush_point`): an owed flush is settled, and
    /// counted by `cause`.
    pub fn flush_settled(&mut self, cause: Settle) {
        if self.defer.as_ref().is_some_and(|d| d.clear()) {
            match cause {
                Settle::SyncPoint => self.stats.settled_sync_point += 1,
                Settle::Doorbell => self.stats.settled_doorbell += 1,
                Settle::Clock => self.stats.settled_clock += 1,
            }
        }
    }

    /// Count an access of a trapped page.
    pub fn note_trapped(&self) {
        self.trapped.set(self.trapped.get() + 1);
    }

    pub fn stats(&self) -> Bar2Stats {
        let mut s = self.stats.clone();
        s.trapped = self.trapped.get();
        s
    }

    /// The sync (docs/caraxes/bar2-memfd.md §1): scan and write back, then follow the
    /// BAR2 tables of `root` if one of their pages changed. `store` is the FB store,
    /// `fb_size` the FB address space, `sys` guest RAM (for sysmem pages mapped through
    /// BAR2), `tr` where the store holds a guest FB address now (the BAR1 pool's remap, or
    /// [`identity`]): write-backs and loads go there, at the moment of the sync.
    pub fn sync(
        &mut self,
        store: &mut dyn FbStore,
        fb_size: u64,
        root: Option<u64>,
        sys: Option<&GuestMemory>,
        tr: Translate,
    ) -> SyncReport {
        let t0 = std::time::Instant::now();
        let mut rep = SyncReport::default();
        let mut sys_skipped = 0u64;
        let scan = self.remap.scan(
            &self.mem.view,
            &mut |key, off, bytes| match target_of(key) {
                Target::Fb(pa) => store.write(tr(pa) + off as u64, bytes),
                Target::Sys(pa) => match sys {
                    Some(m) => {
                        m.write(pa + off as u64, bytes);
                    }
                    None => sys_skipped += 1,
                },
                _ => {}
            },
        );
        self.stats.syncs += 1;
        self.stats.compared += scan.compared as u64;
        self.stats.dirty += scan.dirty.len() as u64;
        self.stats.bytes_written_back += scan.bytes as u64;
        self.stats.conflicts += scan.conflicts as u64;
        self.stats.sys_skipped += sys_skipped;
        rep.dirty = scan.dirty.into_iter().filter(|k| k & SYS == 0).collect();
        if let Some(c) = self.cache.borrow_mut().as_mut() {
            for &p in &rep.dirty {
                if c.watches(p) {
                    c.mark_dirty(p);
                }
            }
        }
        if let Some(root) = root {
            self.follow_tables(store, fb_size, root, sys, tr, &mut rep);
        }
        self.check_drift(store, tr, t0);
        self.stats.max_live = self.stats.max_live.max(self.remap.live_pages() as u64);
        self.stats.max_aliases = self.stats.max_aliases.max(self.remap.max_aliases() as u64);
        let ns = t0.elapsed().as_nanos() as u64;
        self.stats.sync_ns += ns;
        self.stats.sync_ns_max = self.stats.sync_ns_max.max(ns);
        crate::profile::record(
            crate::profile::Key::Window("bar2 memfd: syncs (units: dirty pages)"),
            std::time::Duration::from_nanos(ns),
            rep.dirty.len() as u64,
            None,
            "",
        );
        rep
    }

    /// At most once per [`DRIFT_EVERY`], after the write-back: compare one mapped FB page's
    /// VRAM with its snapshot. They must agree (the guest's writes were just written back,
    /// the device's own go to both); a difference is a GPU engine's write the memfd does not
    /// show, so the guest reads it stale through BAR2. Counted, never corrected here.
    fn check_drift(&mut self, store: &dyn FbStore, tr: Translate, now: std::time::Instant) {
        if self
            .drift_at
            .is_some_and(|t| now.saturating_duration_since(t) < DRIFT_EVERY)
        {
            return;
        }
        self.drift_at = Some(now);
        let Some(key) = self.remap.next_fb_key(self.drift_cursor) else {
            return;
        };
        self.drift_cursor = key;
        let Target::Fb(pa) = target_of(key) else {
            return;
        };
        let mut vram = vec![0u8; PAGE as usize];
        store.read(tr(pa), &mut vram);
        self.stats.drift_checked += 1;
        if self.remap.drifted(key, &vram) {
            self.stats.drift_found += 1;
            if self.stats.drift_first.len() < 8 && !self.stats.drift_first.contains(&pa) {
                self.stats.drift_first.push(pa);
            }
        }
    }

    /// Bring the BAR2 translation up to date with the tables rooted at `root`.
    fn follow_tables(
        &mut self,
        store: &dyn FbStore,
        fb_size: u64,
        root: u64,
        sys: Option<&GuestMemory>,
        tr: Translate,
        rep: &mut SyncReport,
    ) {
        if self.cache_root.get() != root {
            // A new root (first sync, or the guest moved it): walk from scratch; the
            // pages the old tables mapped are unmapped unit by unit below.
            let old: Vec<u64> = self
                .remap
                .va
                .keys()
                .map(|v| v * PAGE / UNIT * UNIT)
                .collect();
            *self.cache.borrow_mut() = Some(WalkCache::new(Target::Fb(root), MAX_LEAVES));
            self.cache_root.set(root);
            let mut units: Vec<u64> = old;
            units.dedup();
            let mut guard = self.cache.borrow_mut();
            let c = guard.as_mut().expect("just set");
            let upd = {
                let r = Reader {
                    remap: &self.remap,
                    mem: &self.mem.view,
                    store,
                    tr,
                    fb_size,
                    sys,
                    private: self.private,
                };
                c.update(&r, false)
            };
            units.extend(upd.changed);
            units.sort_unstable();
            units.dedup();
            let work: Vec<(u64, Vec<Leaf>)> = units
                .into_iter()
                .map(|u| (u, c.unit_leaves(u).to_vec()))
                .collect();
            drop(guard);
            self.stats.walks += 1;
            crate::profile::window("bar2 memfd: table walks", 0);
            for (u, leaves) in work {
                self.apply_unit(u, &leaves, store, fb_size, sys, tr, rep);
            }
            return;
        }
        let work: Vec<(u64, Vec<Leaf>)> = {
            let mut c = self.cache.borrow_mut();
            let Some(c) = c.as_mut() else { return };
            if !c.is_dirty() {
                return;
            }
            let upd = {
                let r = Reader {
                    remap: &self.remap,
                    mem: &self.mem.view,
                    store,
                    tr,
                    fb_size,
                    sys,
                    private: self.private,
                };
                c.update(&r, false)
            };
            upd.changed
                .into_iter()
                .map(|u| (u, c.unit_leaves(u).to_vec()))
                .collect()
        };
        self.stats.walks += 1;
        crate::profile::window("bar2 memfd: table walks", 0);
        for (u, leaves) in work {
            self.apply_unit(u, &leaves, store, fb_size, sys, tr, rep);
        }
    }

    /// Re-point every BAR2 page of the 2 MiB unit at `unit` from its leaves.
    #[allow(clippy::too_many_arguments)]
    fn apply_unit(
        &mut self,
        unit: u64,
        leaves: &[Leaf],
        store: &dyn FbStore,
        fb_size: u64,
        sys: Option<&GuestMemory>,
        tr: Translate,
        rep: &mut SyncReport,
    ) {
        if unit >= self.size {
            return;
        }
        let first = unit / PAGE;
        let end = (unit + UNIT).min(self.size) / PAGE;
        let mut want: HashMap<u64, u64> = HashMap::new();
        for &(va, t, sz, _) in leaves {
            for k in 0..sz / PAGE {
                let vp = va / PAGE + k;
                if vp < first || vp >= end {
                    continue;
                }
                let key = match t {
                    Target::Fb(pa)
                        if pa + (k + 1) * PAGE <= fb_size
                            && !overlaps(self.private, pa + k * PAGE, PAGE) =>
                    {
                        key_of(Target::Fb(pa + k * PAGE))
                    }
                    Target::Sys(pa) => key_of(Target::Sys(pa + k * PAGE)),
                    _ => None,
                };
                if let Some(key) = key {
                    want.insert(vp, key);
                }
            }
        }
        let Bar2Area {
            remap,
            mem,
            traps,
            stats,
            ..
        } = self;
        let mut loads = 0u64;
        let mut load = |key: u64| -> Box<PageBuf> {
            loads += 1;
            let mut b = vec![0u8; PAGE as usize];
            match target_of(key) {
                Target::Fb(pa) => store.read(tr(pa), &mut b),
                Target::Sys(pa) => {
                    if !sys.is_some_and(|m| m.read(pa, &mut b)) {
                        b.fill(0);
                    }
                }
                _ => {}
            }
            let mut p: Box<PageBuf> = Box::new([0; WORDS]);
            for (w, c) in p.iter_mut().zip(b.chunks_exact(8)) {
                *w = u64::from_le_bytes(c.try_into().unwrap());
            }
            p
        };
        for vp in first..end {
            if traps.binary_search(&vp).is_ok() {
                continue;
            }
            let ch = remap.set(vp, want.get(&vp).copied(), &mem.view, &mut load);
            if ch.left.is_some() {
                stats.left += 1;
                rep.left += 1;
            }
            if ch.entered.is_some() {
                stats.entered += 1;
                rep.entered += 1;
            }
        }
        stats.loads += loads;
    }

    /// Before a function-level reset hands the area to the fresh model: forget every
    /// mapping and the BAR2 walk (the caller has synced).
    pub fn reset_mapping(&mut self) {
        self.remap.clear();
        *self.cache.borrow_mut() = None;
        self.cache_root.set(u64::MAX);
    }

    /// The exit-summary line.
    pub fn summary(&self) -> String {
        let s = self.stats();
        let traps: Vec<String> = self
            .traps
            .iter()
            .map(|p| format!("{:#x}", p * PAGE))
            .collect();
        format!(
            "bar2 memfd: BAR2 [0, {:#x}) in host RAM, trapped page(s) [{}] with {} access(es); {} sync(s) ({} us total, {} us max), {} page compare(s), {} dirty page(s), {} byte(s) written back, {} alias conflict(s); {} BAR2 table walk(s), {} page(s) entered ({} loaded), {} left; now {} page(s) mapped at {} BAR2 page(s), at most {} live and {} alias(es) of one page; {} sysmem write-back(s) skipped; flush reads {}; VRAM behind the memfd: {} of {} sampled page(s) changed{} (docs/caraxes/bar2-memfd.md, display-pacing.md)",
            self.size,
            traps.join(" "),
            s.trapped,
            s.syncs,
            s.sync_ns / 1000,
            s.sync_ns_max / 1000,
            s.compared,
            s.dirty,
            s.bytes_written_back,
            s.conflicts,
            s.walks,
            s.entered,
            s.loads,
            s.left,
            self.remap.live_pages(),
            self.remap.mapped(),
            s.max_live,
            s.max_aliases,
            s.sys_skipped,
            if self.defer.is_some() {
                format!(
                    "deferred: {} deferred, settled {} at a sync point, {} at a doorbell, {} by the clock",
                    s.deferred, s.settled_sync_point, s.settled_doorbell, s.settled_clock
                )
            } else {
                "sync at once".to_string()
            },
            s.drift_found,
            s.drift_checked,
            if s.drift_first.is_empty() {
                String::new()
            } else {
                format!(
                    " (GPU writes the guest reads stale through BAR2: FB {})",
                    s.drift_first.iter().map(|p| format!("{p:#x}")).collect::<Vec<_>>().join(" ")
                )
            }
        )
    }
}

#[cfg(test)]
mod flush_page_tests {
    use super::*;

    /// `CARAXES_BAR2_FLUSH_PAGE`: memory drops the flush page from the traps; fail closed
    /// on a trap list with the flush page or the deferred flush off.
    #[test]
    fn flush_page_as_memory_settings() {
        let c = config_from(None, false, true, None).unwrap();
        assert_eq!(c.traps, vec![DEFAULT_TRAP]);
        assert!(!c.flush_page_memory && c.defer_flush);
        let c = config_from(None, false, true, Some("memory")).unwrap();
        assert!(c.traps.is_empty() && c.flush_page_memory);
        assert!(config_from(None, false, false, Some("memory")).is_err());
        assert!(config_from(Some("0xfea000"), false, true, Some("memory")).is_err());
        assert!(config_from(Some("0x1000"), false, true, Some("memory")).is_ok());
        assert!(config_from(None, false, true, Some("maybe")).is_err());
        assert!(!config_from(None, false, true, Some("trap")).unwrap().flush_page_memory);
    }

    /// Standing mode: owed from the start and again after every sync, the bound counting
    /// from the last one.
    #[test]
    fn a_standing_flush_is_owed_again_after_every_sync() {
        let d = DeferredFlush::standing();
        assert!(d.pending());
        assert!(d.due_in().is_some());
        assert!(d.clear());
        assert!(d.pending(), "owed again from the sync on");
        assert!(d.due_in().unwrap() <= DEFER_BOUND);
        assert!(!d.mark(), "a trapped flush read (a client without the mapping) adds nothing");
        let plain = DeferredFlush::default();
        assert!(!plain.pending());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The owed flush: one mark per burst wakes the clock, and the clock settles it
    /// [`DEFER_BOUND`] after the mark.
    #[test]
    fn the_flush_clock_settles_an_owed_flush_after_the_bound() {
        let d = Arc::new(DeferredFlush::default());
        assert_eq!(d.due_in(), None);
        assert!(d.mark());
        assert!(!d.mark(), "a second flush read in the burst changes nothing");
        assert!(d.due_in().unwrap() <= DEFER_BOUND);
        assert!(d.clear());
        assert!(!d.clear());
        let settled = Arc::new(AtomicU64::new(0));
        let (d2, s2) = (d.clone(), settled.clone());
        let marked = Instant::now();
        d.spawn_clock(Box::new(move || {
            s2.fetch_add(1, Ordering::SeqCst);
            d2.clear();
        }))
        .unwrap();
        d.mark();
        while settled.load(Ordering::SeqCst) == 0 {
            assert!(marked.elapsed() < Duration::from_secs(5), "the clock never settled");
            std::thread::sleep(Duration::from_micros(200));
        }
        assert!(marked.elapsed() >= DEFER_BOUND);
        assert!(!d.pending());
        std::thread::sleep(3 * DEFER_BOUND);
        assert_eq!(settled.load(Ordering::SeqCst), 1, "nothing owed, nothing settled");
    }

    #[test]
    fn sparse_areas_leave_the_trapped_pages_out() {
        let size = 32 << 20;
        assert_eq!(
            sparse_areas(size, &[DEFAULT_TRAP]).unwrap(),
            vec![
                (0, DEFAULT_TRAP),
                (DEFAULT_TRAP + PAGE, size - DEFAULT_TRAP - PAGE)
            ]
        );
        assert_eq!(sparse_areas(size, &[]).unwrap(), vec![(0, size)]);
        // First and last page, a duplicate, unsorted input.
        assert_eq!(
            sparse_areas(4 * PAGE, &[3 * PAGE, 0, 0]).unwrap(),
            vec![(PAGE, 2 * PAGE)]
        );
        assert!(sparse_areas(size, &[0x1234]).is_err());
        assert!(sparse_areas(size, &[size]).is_err());
        assert!(sparse_areas(0x1800, &[]).is_err());
        assert_eq!(parse_traps("0xfea000, 4096").unwrap(), vec![0xfea000, 4096]);
        assert_eq!(parse_traps("none").unwrap(), Vec::<u64>::new());
        assert!(parse_traps("0xfea001").is_err());
    }

    fn filled(v: u64) -> Box<PageBuf> {
        Box::new([v; WORDS])
    }

    #[test]
    fn the_remap_table_moves_pages_and_shares_aliases() {
        let area = MemfdArea::create(16 * PAGE).unwrap();
        let mem = area.view();
        let mut t = RemapTable::default();
        let loaded = std::cell::RefCell::new(Vec::new());
        let mut load = |k: u64| {
            loaded.borrow_mut().push(k);
            filled(k)
        };
        // BAR2 page 2 maps FB page 0x100: loaded into memfd page 2.
        let ch = t.set(2, Some(0x100), mem, &mut load);
        assert_eq!((ch.entered, ch.loaded, ch.left), (Some(0x100), true, None));
        assert_eq!(mem.load_word(2, 5), 0x100);
        assert_eq!(t.home(0x100), Some(2));
        // BAR2 page 3 maps the same FB page: an alias, filled from the snapshot, no load.
        let ch = t.set(3, Some(0x100), mem, &mut load);
        assert!(!ch.loaded);
        assert_eq!(mem.load_word(3, 5), 0x100);
        assert_eq!(t.aliases(0x100), &[2, 3]);
        assert_eq!(t.live_pages(), 1);
        // Page 2 moves to FB page 0x200: 0x100 stays live through page 3.
        let ch = t.set(2, Some(0x200), mem, &mut load);
        assert_eq!((ch.left, ch.entered), (None, Some(0x200)));
        assert_eq!(t.home(0x100), Some(3));
        // Page 3 unmapped: 0x100 leaves.
        let ch = t.set(3, None, mem, &mut load);
        assert_eq!(ch.left, Some(0x100));
        assert_eq!(t.home(0x100), None);
        // Sysmem keys round-trip.
        assert_eq!(
            target_of(key_of(Target::Sys(0x5000)).unwrap()),
            Target::Sys(0x5000)
        );
        assert_eq!(
            target_of(key_of(Target::Fb(0x7000)).unwrap()),
            Target::Fb(0x7000)
        );
        assert_eq!(*loaded.borrow(), vec![0x100, 0x200]);
        // A device write lands in every alias and the snapshot.
        t.set(4, Some(0x200), mem, &mut |k| filled(k));
        assert!(t.device_write(mem, 0x200 * PAGE + 16, &[0xAA; 8]));
        assert_eq!(mem.load_word(2, 2), 0xAAAA_AAAA_AAAA_AAAA);
        assert_eq!(mem.load_word(4, 2), 0xAAAA_AAAA_AAAA_AAAA);
        assert!(!t.device_write(mem, 0x300 * PAGE, &[1]));
        let s = t.scan(mem, &mut |_, _, _| {
            panic!("a device write is not a guest write")
        });
        assert!(s.dirty.is_empty());
        assert_eq!(s.compared, 2);
    }

    #[test]
    fn the_scan_sees_writes_through_another_mapping() {
        let area = MemfdArea::create(16 * PAGE).unwrap();
        area.self_test().unwrap();
        let guest = area.map_again().unwrap();
        let mem = area.view();
        let mut t = RemapTable::default();
        t.set(1, Some(0x40), mem, &mut |_| filled(0));
        t.set(5, Some(0x41), mem, &mut |_| filled(0));
        t.set(6, Some(0x41), mem, &mut |_| filled(0));
        // Nothing written: nothing dirty.
        assert!(t.scan(mem, &mut |_, _, _| panic!()).dirty.is_empty());
        // The guest writes a qword at 0x18, a dword at 0x20 (next to it, so one run) of
        // BAR2 page 1, and a dword at 0x4c through alias 6.
        guest.write(PAGE + 0x18, &0x1122_3344_5566_7788u64.to_le_bytes());
        guest.write(PAGE + 0x20, &0xAABB_CCDDu32.to_le_bytes());
        guest.write(6 * PAGE + 0x4c, &0x99u32.to_le_bytes());
        let mut wb = Vec::new();
        let s = t.scan(mem, &mut |k, off, b| wb.push((k, off, b.to_vec())));
        assert_eq!(s.dirty, vec![0x40, 0x41]);
        // Only the bytes that changed are written back: 12 bytes at 0x18, and at 0x4c
        // only the one byte that differs from the snapshot (the upper three were zero).
        assert_eq!(s.bytes, 13);
        let mut run = 0x1122_3344_5566_7788u64.to_le_bytes().to_vec();
        run.extend(0xAABB_CCDDu32.to_le_bytes());
        assert_eq!(wb, vec![(0x40, 0x18, run), (0x41, 0x4c, vec![0x99])]);
        // The alias written through page 6 now shows in page 5 too.
        assert_eq!(mem.load_word(5, 9), 0x99 << 32);
        // The snapshot took the writes: a second scan is clean.
        assert!(t.scan(mem, &mut |_, _, _| panic!()).dirty.is_empty());
        // Two aliases writing different dwords of one qword: both kept, no conflict.
        guest.write(5 * PAGE, &1u32.to_le_bytes());
        guest.write(6 * PAGE + 4, &2u32.to_le_bytes());
        let s = t.scan(mem, &mut |_, _, _| {});
        assert_eq!(s.conflicts, 0);
        assert_eq!(
            (mem.load_word(5, 0), mem.load_word(6, 0)),
            (2 << 32 | 1, 2 << 32 | 1)
        );
        // Two aliases writing the same byte differently: the first alias wins.
        guest.store_word(5, 1, 7);
        guest.store_word(6, 1, 8);
        let s = t.scan(mem, &mut |_, _, _| {});
        assert_eq!(s.conflicts, 1);
        assert_eq!((mem.load_word(5, 1), mem.load_word(6, 1)), (7, 7));
    }
}

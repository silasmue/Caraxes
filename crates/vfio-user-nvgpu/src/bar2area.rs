// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
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
// One key space with the walks' (ptcache.rs `observed_keys`).
const _: () = assert!(SYS == crate::ptcache::SYS_KEY);
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

    /// Whether page `page` holds exactly `snap`: one `memcmp` over the mapping, no copy (the
    /// scans' test of a clean page, the common case).
    pub fn page_eq(&self, page: u64, snap: &PageBuf) -> bool {
        let p = self.at(page * PAGE, PAGE as usize);
        // SAFETY: in bounds; both are 4096 readable bytes. The mapping is shared memory the
        // guest may write concurrently: a torn compare only says "differs", and the copy
        // that follows (`load_page`) decides.
        unsafe { libc::memcmp(p.cast(), snap.as_ptr().cast(), PAGE as usize) == 0 }
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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// The page that left the BAR2 mapping (its last BAR2 page went).
    pub left: Option<u64>,
    /// Its snapshot as it left (what VRAM holds: the sync wrote the guest's bytes back
    /// first), for the victim cache ([`Victims`]).
    pub left_snap: Option<Box<PageBuf>>,
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
    /// Bumped whenever a page enters or leaves `live` ([`Self::generation`]).
    generation: u64,
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

    /// Changes whenever the set of pages mapped through BAR2 does: equal generations, equal
    /// live sets (what the doorbell rule caches, [`Bar2Area::observed`]).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The live page keys, ascending.
    pub fn live_keys(&self) -> Vec<u64> {
        let mut k: Vec<u64> = self.live.keys().copied().collect();
        k.sort_unstable();
        k
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
                    let gone = self.live.remove(&o).expect("present");
                    self.generation += 1;
                    ch.left = Some(o);
                    ch.left_snap = Some(gone.snap);
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
                    self.generation += 1;
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
        self.scan_keys(mem, None, write_back)
    }

    /// [`Self::scan`] of the live pages among `only` (all of them for `None`).
    pub fn scan_keys(
        &mut self,
        mem: &SharedView,
        only: Option<&[u64]>,
        write_back: &mut dyn FnMut(u64, usize, &[u8]),
    ) -> Scan {
        let mut out = Scan::default();
        // Allocated at the first page that differs: a scan of clean pages is one memcmp
        // per BAR2 page and nothing else.
        let mut bufs: Option<(Box<PageBuf>, Box<PageBuf>)> = None;
        let mut keys: Vec<u64> = match only {
            Some(k) => k.iter().copied().filter(|k| self.live.contains_key(k)).collect(),
            None => self.live.keys().copied().collect(),
        };
        keys.sort_unstable();
        keys.dedup();
        for key in keys {
            let l = self.live.get_mut(&key).expect("listed");
            let mut merged: Option<Box<PageBuf>> = None;
            let mut cleared = false;
            for &va in &l.vas {
                out.compared += 1;
                if mem.page_eq(va, &l.snap) {
                    continue;
                }
                let (buf, cmask) =
                    bufs.get_or_insert_with(|| (Box::new([0; WORDS]), Box::new([0; WORDS])));
                if !cleared {
                    cmask.fill(0);
                    cleared = true;
                }
                mem.load_page(va, buf);
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
            let (_, cmask) = bufs.as_ref().expect("a page differed");
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

    /// Over `buf`, which holds bytes `[in_page, in_page + buf.len())` of page `key` as VRAM
    /// has them: the bytes the guest wrote through BAR2 since the last sync, i.e. those in
    /// which a BAR2 page of `key` differs from the snapshot (across aliases the first
    /// alias wins, as in [`Self::scan`]). Returns whether `key` is live.
    pub fn overlay(&self, mem: &SharedView, key: u64, in_page: usize, buf: &mut [u8]) -> bool {
        let Some(l) = self.live.get(&key) else {
            return false;
        };
        // SAFETY: as `page_bytes`, read-only.
        let snap: &[u8; 4096] = unsafe { &*(&*l.snap as *const PageBuf as *const [u8; 4096]) };
        let snap = &snap[in_page..in_page + buf.len()];
        let mut taken = vec![false; buf.len()];
        let mut alias = vec![0u8; buf.len()];
        for &va in &l.vas {
            mem.read(va * PAGE + in_page as u64, &mut alias);
            if alias[..] == snap[..] {
                continue;
            }
            for i in 0..buf.len() {
                if !taken[i] && alias[i] != snap[i] {
                    buf[i] = alias[i];
                    taken[i] = true;
                }
            }
        }
        true
    }

    /// Forget every mapping (a function-level reset; the memfd content is left as it is).
    pub fn clear(&mut self) {
        self.va.clear();
        self.live.clear();
        self.generation += 1;
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

/// Read `[pa, pa + buf.len())` of the FB (guest addresses) as every reader but the guest's
/// BAR2 sees it (docs/caraxes/bar2-memfd.md §3, rule 5): VRAM, i.e. the store at
/// `tr(address)`, which has what the guest wrote through the BAR1 pool and what a GPU engine
/// wrote, overlaid for a page behind BAR2 with the bytes the guest wrote through BAR2 since
/// the last sync ([`RemapTable::overlay`]). FB is one memory on the card: a write through any
/// aperture or engine is what the next read of any other returns.
#[allow(clippy::too_many_arguments)]
fn read_merged(
    remap: &RemapTable,
    mem: &SharedView,
    store: &dyn FbStore,
    tr: Translate,
    pa: u64,
    buf: &mut [u8],
    victims: &mut Victims,
    epoch: u64,
) {
    let mut done = 0usize;
    while done < buf.len() {
        let a = pa + done as u64;
        let in_page = a % PAGE;
        let n = (PAGE - in_page).min((buf.len() - done) as u64) as usize;
        let piece = &mut buf[done..done + n];
        if remap.home(a / PAGE).is_none() {
            // Not behind BAR2: a page table that left it is what VRAM holds ([`Victims`]).
            if !victims.read(a / PAGE, in_page as usize, piece, epoch) {
                store.read(tr(a), piece);
            }
        } else {
            store.read(tr(a), piece);
            remap.overlay(mem, a / PAGE, in_page as usize, piece);
        }
        done += n;
    }
}

/// Read `[pa, pa + buf.len())` of the FB (guest addresses): mapped pages from their BAR2
/// page in the memfd, the rest from the store at `tr(address)`. Only for pages no one but
/// the guest's BAR2 and the device writes (the page tables, [`Bar2Area::read_tables`]):
/// then the memfd equals [`read_merged`] and costs a RAM read instead of a VRAM read.
fn read_through(
    remap: &RemapTable,
    mem: &SharedView,
    store: &dyn FbStore,
    tr: Translate,
    pa: u64,
    buf: &mut [u8],
    mut victims: Option<(&mut Victims, u64)>,
) {
    let mut done = 0usize;
    while done < buf.len() {
        let a = pa + done as u64;
        let in_page = a % PAGE;
        let n = (PAGE - in_page).min((buf.len() - done) as u64) as usize;
        let piece = &mut buf[done..done + n];
        match remap.home(a / PAGE) {
            Some(va) => mem.read(va * PAGE + in_page, piece),
            None => {
                let hit = victims
                    .as_mut()
                    .is_some_and(|(v, e)| v.read(a / PAGE, in_page as usize, piece, *e));
                if !hit {
                    store.read(tr(a), piece);
                }
            }
        }
        done += n;
    }
}

/// Whether `[pa, pa + len)` touches `private` (fb.rs `FakeFb::private_overlaps`).
fn overlaps(private: &[(u64, u64)], pa: u64, len: u64) -> bool {
    private
        .iter()
        .any(|&(b, e)| pa < e && b < pa.saturating_add(len))
}

/// The FB (through the remap) and guest RAM as the BAR2 walk's table reader.
struct Reader<'a> {
    remap: &'a RemapTable,
    mem: &'a SharedView,
    store: &'a dyn FbStore,
    tr: Translate<'a>,
    fb_size: u64,
    sys: Option<&'a GuestMemory>,
    private: &'a [(u64, u64)],
}

impl TableReader for Reader<'_> {
    fn block(&self, table: Target, n: usize) -> Option<Vec<u64>> {
        let mut b = vec![0u8; n * 8];
        match table {
            Target::Fb(pa)
                if pa + b.len() as u64 <= self.fb_size
                    && !overlaps(self.private, pa, b.len() as u64) =>
            {
                read_through(self.remap, self.mem, self.store, self.tr, pa, &mut b, None)
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
    /// A sync point that syncs anyway: the GSP command-queue doorbell, the BAR2 bind, the
    /// MMU invalidate while a host VA space walks the guest's tables, or a flush read with
    /// the deferral off.
    SyncPoint,
    /// A channel doorbell (trapped, ioeventfd or token): the GPU may fetch what the guest
    /// wrote before it.
    Doorbell,
    /// The flush clock, [`DEFER_BOUND`] after the flush read, with no hand-over since.
    Clock,
}

/// The points at which what the guest wrote through BAR2 must be in VRAM, because the
/// card lets an engine, the MMU or GSP-RM consume it from there on (docs/caraxes/
/// bar2-memfd.md §3). A full sync runs at each, except that a flush (read or
/// `NV_UFLUSH_FB_FLUSH`) with the deferral on only marks the sync owed ([`DeferredFlush`]);
/// a channel doorbell without an owed flush syncs only the pages its channel could observe
/// ([`Bar2Area::sync_pages`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Point {
    /// A read of the trapped flush page: `kbusFlushSingle_GV100` reads FB address 0 through
    /// BAR2 "in order to trigger a flush" (615 kern_bus_gv100.c:384-399); by PCIe ordering
    /// the read completes after every earlier posted write.
    FlushRead,
    /// `NV_UFLUSH_FB_FLUSH` (BAR0 0x070000, maxwell/gm107/dev_flush.h:30): the sysmembar
    /// that "flush[es] all outstanding writes to the GPU from the PCIE path" (615
    /// kern_bus_gh100.c:2946-2950, its XAL successor); started by writing PENDING (bit 0)
    /// and done when PENDING and OUTSTANDING (bit 1) read 0 (Linux 7.2.7
    /// nvkm/subdev/bar/g84.c:29-40).
    FbFlush,
    /// `NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE` with TRIGGER (turing/tu102/dev_vm.h:120-131):
    /// the MMU walks the page tables again. A full sync only while a host VA space walks the
    /// guest's own tables (mirror.rs `walks_guest_tables`); otherwise the invalidate syncs the
    /// table pages alone and an owed flush waits for the next engine start (bar2-memfd.md §3
    /// rule 3).
    MmuInvalidate,
    /// `NV_PGSP_QUEUE_HEAD(0)` (turing/tu102/dev_gsp.h:38): GSP-RM consumes the command
    /// queue. On a GSP system the runlist submit is GSP-RM's own register write; the CPU's
    /// equivalents (channel and TSG allocation, `NVA06F_CTRL_CMD_BIND`,
    /// `NVA06C/NVA06F_CTRL_CMD_GPFIFO_SCHEDULE`, `NV2080_CTRL_CMD_FIFO_DISABLE_CHANNELS`) are
    /// RPCs through this queue, so this point covers them.
    GspQueueHead,
    /// `NV_PBUS_BAR2_BLOCK`: BAR2 (re)bound, its tables live from here on.
    Bar2Bind,
    /// A channel doorbell (`NV_VIRTUAL_FUNCTION_DOORBELL`, turing/tu102/dev_vm.h:229) that
    /// found a flush owed.
    Doorbell,
    /// The flush clock, [`DEFER_BOUND`] after an owed flush with no hand-over since.
    Clock,
}

const POINTS: usize = 7;

impl Point {
    /// How a sync at this point counts when it settles an owed flush.
    pub fn settle(self) -> Settle {
        match self {
            Point::Doorbell => Settle::Doorbell,
            Point::Clock => Settle::Clock,
            _ => Settle::SyncPoint,
        }
    }

    /// The point a settle cause stands for ([`Settle`] callers without a point).
    pub fn of(cause: Settle) -> Point {
        match cause {
            Settle::Doorbell => Point::Doorbell,
            Settle::Clock => Point::Clock,
            Settle::SyncPoint => Point::FlushRead,
        }
    }
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
    /// Full syncs by [`Point`] (in its order).
    pub by_point: [u64; POINTS],
    /// `NV_UFLUSH_FB_FLUSH` writes that started a flush.
    pub fb_flushes: u64,
    /// The doorbell rule (channel doorbells without an owed flush, [`Bar2Area::sync_pages`]):
    /// doorbells checked; those whose channels could observe nothing behind the memfd; those
    /// with a channel whose memory the device cannot name (all live pages count); the
    /// partial syncs, their page compares, the dirty pages they found and their time.
    pub doorbells_checked: u64,
    pub doorbells_clean: u64,
    pub doorbells_unknown: u64,
    pub doorbell_syncs: u64,
    pub doorbell_compared: u64,
    pub doorbell_dirty: u64,
    pub doorbell_ns: u64,
    pub doorbell_ns_max: u64,
    /// Time in the full syncs by [`Point`], in nanoseconds.
    pub point_ns: [u64; POINTS],
    /// MMU invalidates that synced only the live table pages, and the pages; of them, those
    /// that found a flush owed and left it to the next engine start (bar2-memfd.md §3 rule 3).
    pub table_syncs: u64,
    pub table_syncs_owed: u64,
    pub table_sync_pages: u64,
    /// The walks' live table page lists those syncs reused (the walk's table pages and the
    /// BAR2 live set unchanged since) and listed again (fb.rs `bar2_sync_tables`).
    pub table_key_lists_reused: u64,
    pub table_key_lists_listed: u64,
    pub table_sync_ns: u64,
    /// The doorbell rule's sets as last computed per channel: how many times, and the most
    /// live pages one held; the most live table pages of a channel left to the invalidate.
    pub observe_sets: u64,
    pub observe_pages_max: u64,
    pub observe_tables_max: u64,
}

/// What one sync changed, for the FB's caches.
#[derive(Clone, Debug, Default)]
pub struct SyncReport {
    /// FB page numbers whose content changed (guest writes found by the scan).
    pub dirty: Vec<u64>,
    pub entered: usize,
    pub left: usize,
}

/// Pages the victim cache keeps at most (16 MiB of snapshots).
pub const VICTIMS_MAX: usize = 4096;

/// What the victim cache needs from the FB (fb.rs): the table epoch, which moves whenever a
/// cached guest walk stops reading a table page (a page table may then be reused as data
/// that a GPU engine writes), and which pages a walk reads as tables now.
pub struct TableView<'a> {
    pub epoch: u64,
    pub walked: &'a dyn Fn(u64) -> bool,
}

impl TableView<'_> {
    /// No victim is ever kept (tests, and callers without the FB's walks).
    pub fn none() -> TableView<'static> {
        TableView {
            epoch: u64::MAX,
            walked: &|_| false,
        }
    }
}

/// The victim cache (docs/caraxes/bar2-memfd.md §3, rule 6): the snapshots of page-table
/// pages that left the BAR2 mapping, keyed by FB page. A snapshot equals VRAM when the page
/// leaves (the sync wrote the guest's BAR2 bytes back first), and a page table has no other
/// writer: the BAR1 pool never holds one, no host engine writes the guest's tables (the
/// host MMU reads the shadow's), the device's own writes patch the entry, and a write it
/// only learns of (`FakeFb::note_range_changed`) drops it. So while the table epoch stands,
/// the snapshot is what VRAM holds: a page that enters BAR2 again is loaded from it, and a
/// table read of it is served from it, both without an uncached BAR1 read through a window
/// that must be mapped first. The 615 RM maps a page table into BAR2 for every PTE update
/// and unmaps it after (vm SotTR run: 365 BAR2 walks and 457 window maps per second).
#[derive(Default)]
pub struct Victims {
    pages: HashMap<u64, (Box<PageBuf>, u64)>,
    /// Insertion order for the bound: (key, the insertion's number).
    order: std::collections::VecDeque<(u64, u64)>,
    seq: u64,
    stamp: HashMap<u64, u64>,
    pub stats: VictimStats,
}

/// The victim cache's counters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VictimStats {
    pub kept: u64,
    /// Loads of an entering page served from the cache, and those read from VRAM.
    pub load_hits: u64,
    pub load_misses: u64,
    /// Device reads served from the cache.
    pub read_hits: u64,
    /// Entries dropped: a stale epoch at use, a write of unknown content, the bound.
    pub stale: u64,
    pub dropped: u64,
    pub evicted: u64,
    /// Entries dropped because no walk reads the page as a table any more.
    pub unwalked: u64,
}

impl Victims {
    /// Keep `snap` for FB page `key` (not a sysmem key) as of table epoch `epoch`.
    fn keep(&mut self, key: u64, snap: Box<PageBuf>, epoch: u64) {
        if key & SYS != 0 {
            return;
        }
        self.seq += 1;
        self.pages.insert(key, (snap, epoch));
        self.stamp.insert(key, self.seq);
        self.order.push_back((key, self.seq));
        self.stats.kept += 1;
        while self.pages.len() > VICTIMS_MAX {
            let Some((k, s)) = self.order.pop_front() else { break };
            if self.stamp.get(&k) == Some(&s) {
                self.pages.remove(&k);
                self.stamp.remove(&k);
                self.stats.evicted += 1;
            }
        }
        // Stale order entries of re-kept or taken pages are skipped at their turn; compact
        // when they pile up.
        if self.order.len() > 4 * VICTIMS_MAX {
            let stamp = &self.stamp;
            self.order.retain(|(k, s)| stamp.get(k) == Some(s));
        }
    }

    /// The snapshot of FB page `key`, taken out, when it is valid at `epoch`.
    fn take(&mut self, key: u64, epoch: u64) -> Option<Box<PageBuf>> {
        let (snap, e) = self.pages.remove(&key)?;
        self.stamp.remove(&key);
        if e != epoch {
            self.stats.stale += 1;
            return None;
        }
        Some(snap)
    }

    /// Copy bytes `[in_page, in_page + buf.len())` of FB page `key` into `buf` when a valid
    /// snapshot is held; whether it was.
    fn read(&mut self, key: u64, in_page: usize, buf: &mut [u8], epoch: u64) -> bool {
        let Some((snap, e)) = self.pages.get(&key) else {
            return false;
        };
        if *e != epoch {
            self.pages.remove(&key);
            self.stamp.remove(&key);
            self.stats.stale += 1;
            return false;
        }
        // SAFETY: as `page_bytes`, read-only.
        let bytes: &[u8; 4096] = unsafe { &*(&**snap as *const PageBuf as *const [u8; 4096]) };
        buf.copy_from_slice(&bytes[in_page..in_page + buf.len()]);
        self.stats.read_hits += 1;
        true
    }

    /// A device write of `data` at FB address `pa`: patch the snapshots it touches.
    fn write(&mut self, pa: u64, data: &[u8]) {
        if self.pages.is_empty() {
            return;
        }
        let mut done = 0usize;
        while done < data.len() {
            let a = pa + done as u64;
            let in_page = (a % PAGE) as usize;
            let n = (PAGE as usize - in_page).min(data.len() - done);
            if let Some((snap, _)) = self.pages.get_mut(&(a / PAGE)) {
                page_bytes(snap)[in_page..in_page + n].copy_from_slice(&data[done..done + n]);
            }
            done += n;
        }
    }

    /// FB pages `first..=last` changed in VRAM without the device knowing the bytes: drop
    /// them.
    fn drop_pages(&mut self, first: u64, last: u64) {
        if self.pages.is_empty() {
            return;
        }
        for p in first..=last.min(first + (1 << 20)) {
            if self.pages.remove(&p).is_some() {
                self.stamp.remove(&p);
                self.stats.dropped += 1;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }
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
    private: Vec<(u64, u64)>,
    /// The live page keys, ascending, as of the remap generation they were listed at.
    live_sorted: RefCell<(u64, Vec<u64>)>,
    /// Page tables that left the BAR2 mapping ([`Victims`]).
    pub victims: RefCell<Victims>,
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
            private: Vec::new(),
            drift_cursor: 0,
            drift_at: None,
            live_sorted: RefCell::new((0, Vec::new())),
            victims: RefCell::new(Victims::default()),
        })
    }

    /// See [`Bar2Area::private`]; set before the first sync.
    pub fn set_private(&mut self, private: Vec<(u64, u64)>) {
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

    /// Whether page `page` is a table page of the BAR2 walk.
    pub fn watches_table(&self, page: u64) -> bool {
        self.cache.borrow().as_ref().is_some_and(|c| c.watches(page))
    }

    /// The BAR2 walk's tables generation (ptcache.rs `WalkCache::tables_generation`):
    /// equal generations, equal table pages. `None` before the walk exists.
    pub fn tables_generation(&self) -> Option<u64> {
        self.cache.borrow().as_ref().map(|c| c.tables_generation())
    }

    /// The table pages of the BAR2 walk.
    pub fn table_pages(&self) -> Vec<u64> {
        self.cache
            .borrow()
            .as_ref()
            .map_or_else(Vec::new, |c| c.watched_pages().collect())
    }

    /// Whether FB page `page` (4 KiB page number) is mapped through BAR2 now.
    pub fn is_live(&self, page: u64) -> bool {
        self.remap.home(page).is_some()
    }

    /// See [`RemapTable::generation`].
    pub fn live_generation(&self) -> u64 {
        self.remap.generation()
    }

    /// Every live page key, ascending (a channel whose memory the device cannot name could
    /// observe any of them).
    pub fn live_keys(&self) -> Vec<u64> {
        self.live_sorted().clone()
    }

    fn live_sorted(&self) -> std::cell::Ref<'_, Vec<u64>> {
        let gen = self.remap.generation();
        if self.live_sorted.borrow().0 != gen {
            *self.live_sorted.borrow_mut() = (gen, self.remap.live_keys());
        }
        std::cell::Ref::map(self.live_sorted.borrow(), |(_, k)| k)
    }

    /// The live page keys a channel could observe, ascending: those inside `pieces`
    /// (memory as `(start, bytes)`, FB or guest sysmem) and among `pages` (FB page numbers).
    pub fn observed(&self, pieces: &[(Target, u64)], pages: &[u64]) -> Vec<u64> {
        let live = self.live_sorted();
        let mut out = Vec::new();
        if live.is_empty() {
            return out;
        }
        let mut take = |lo: u64, hi: u64| {
            let i = live.partition_point(|k| *k < lo);
            out.extend(live[i..].iter().copied().take_while(|k| *k < hi));
        };
        for &(t, n) in pieces {
            if n == 0 {
                continue;
            }
            match t {
                Target::Fb(pa) => take(pa / PAGE, pa.saturating_add(n - 1) / PAGE + 1),
                Target::Sys(pa) => {
                    take((pa / PAGE) | SYS, (pa.saturating_add(n - 1) / PAGE + 1) | SYS)
                }
                _ => {}
            }
        }
        for &p in pages {
            take(p, p + 1);
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Count a full sync at `p` that took `ns`.
    pub fn count_point(&mut self, p: Point, ns: u64) {
        self.stats.by_point[p as usize] += 1;
        self.stats.point_ns[p as usize] += ns;
    }

    /// Count a doorbell set computed: `pages` live pages in it, `tables` live table pages
    /// of the channel left to the invalidate.
    pub fn note_observe_set(&mut self, pages: usize, tables: usize) {
        self.stats.observe_sets += 1;
        self.stats.observe_pages_max = self.stats.observe_pages_max.max(pages as u64);
        self.stats.observe_tables_max = self.stats.observe_tables_max.max(tables as u64);
    }

    /// An `NV_UFLUSH_FB_FLUSH` write that started a flush: as a flush read
    /// ([`Self::defer_flush`]), with the deferral on it marks the sync owed and returns true
    /// (the caller skips it); otherwise false (the caller syncs now). Counted apart from the
    /// flush reads.
    pub fn defer_fb_flush(&mut self) -> bool {
        self.stats.fb_flushes += 1;
        match &self.defer {
            Some(d) => {
                d.mark();
                true
            }
            None => false,
        }
    }

    /// Count a channel doorbell the rule checked: `observed` pages behind the memfd, and
    /// whether a channel's memory was unknown (all live pages).
    pub fn note_doorbell(&mut self, observed: usize, unknown: bool) {
        self.stats.doorbells_checked += 1;
        if observed == 0 {
            self.stats.doorbells_clean += 1;
        }
        if unknown {
            self.stats.doorbells_unknown += 1;
        }
    }

    /// The memfd offset holding FB address `pa`, when its page is mapped through BAR2.
    pub fn backing(&self, pa: u64) -> Option<u64> {
        self.remap.home(pa / PAGE).map(|va| va * PAGE + pa % PAGE)
    }

    /// Read guest FB `[pa, pa + buf.len())` as the device and the GPU see it: VRAM (the
    /// store at `tr(address)`) with the bytes the guest wrote through BAR2 since the last
    /// sync over it ([`read_merged`]).
    pub fn read_merged(
        &self,
        store: &dyn FbStore,
        tr: Translate,
        pa: u64,
        buf: &mut [u8],
        epoch: u64,
    ) {
        let mut v = self.victims.borrow_mut();
        read_merged(&self.remap, self.mem.view(), store, tr, pa, buf, &mut v, epoch);
    }

    /// Read page-table memory `[pa, pa + buf.len())`: the memfd for pages behind BAR2, the
    /// store at `tr(address)` for the rest. A page table has no writer but the guest (its
    /// BAR2 writes land in the memfd, its trapped BAR1 and PRAMIN writes reach the memfd
    /// as device writes) and the device: the BAR1 pool never holds a table page
    /// (bar1-pool.md, the page-table hazard) and no host engine writes the guest's tables
    /// (the shadow's are its own; the device CE takes no page behind the memfd). So the
    /// memfd is what [`Self::read_merged`] would return, at RAM speed.
    pub fn read_tables(
        &self,
        store: &dyn FbStore,
        tr: Translate,
        pa: u64,
        buf: &mut [u8],
        epoch: u64,
    ) {
        let mut v = self.victims.borrow_mut();
        read_through(&self.remap, self.mem.view(), store, tr, pa, buf, Some((&mut v, epoch)));
    }

    /// FB page `page` (4 KiB page number) is no table of any cached walk any more: its
    /// snapshot may go stale (the page may be reused as data another writer changes), so the
    /// victim cache drops it.
    pub fn forget_victim(&self, page: u64) {
        let mut v = self.victims.borrow_mut();
        if v.pages.remove(&page).is_some() {
            v.stamp.remove(&page);
            v.stats.unwalked += 1;
        }
    }

    /// FB `[pa, pa + len)` changed in VRAM without the device knowing the bytes (a host
    /// engine wrote it): the victim cache drops those pages.
    pub fn drop_victims(&self, pa: u64, len: u64) {
        if len == 0 {
            return;
        }
        self.victims
            .borrow_mut()
            .drop_pages(pa / PAGE, (pa + len - 1) / PAGE);
    }

    /// See [`RemapTable::device_write`]; `pa` is the guest FB address (before the pool's
    /// remap), the memfd's key.
    pub fn device_write(&mut self, pa: u64, data: &[u8]) {
        self.victims.get_mut().write(pa, data);
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
        self.sync_only(store, fb_size, root, sys, tr, None, &TableView::none())
    }

    /// [`Self::sync`] whose scan covers only the live pages among `only` (all for `None`):
    /// the MMU invalidate's sync of the table pages ([`crate::fb::FakeFb::bar2_sync_tables`]).
    /// A page that leaves the BAR2 mapping is written back first all the same
    /// ([`Self::apply_work`]).
    #[allow(clippy::too_many_arguments)]
    pub fn sync_only(
        &mut self,
        store: &mut dyn FbStore,
        fb_size: u64,
        root: Option<u64>,
        sys: Option<&GuestMemory>,
        tr: Translate,
        only: Option<&[u64]>,
        view: &TableView,
    ) -> SyncReport {
        let t0 = std::time::Instant::now();
        let mut rep = SyncReport::default();
        let scan = self.scan_back(store, sys, tr, only, &mut rep);
        self.stats.syncs += 1;
        self.stats.compared += scan.compared as u64;
        let kind = if only.is_some() { "BAR2 table sync" } else { "BAR2 full sync" };
        crate::dlock::work(kind, "scan and write-back".into(), t0.elapsed());
        let t1 = std::time::Instant::now();
        if let Some(root) = root {
            self.follow_tables(store, fb_size, root, sys, tr, &mut rep, view);
        }
        crate::dlock::work(kind, "BAR2 table walk".into(), t1.elapsed());
        let t2 = std::time::Instant::now();
        self.check_drift(store, tr, t0);
        crate::dlock::work(kind, "drift sample".into(), t2.elapsed());
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

    /// The doorbell rule's partial sync (docs/caraxes/bar2-memfd.md §3): scan and write back
    /// only the live pages among `keys` ([`Self::observed`]). A BAR2 table page among them
    /// is marked dirty in the BAR2 walk as at a full sync and followed at the next one (a
    /// BAR2 PTE change is followed by a flush and an MMU invalidate, rule 1).
    pub fn sync_pages(
        &mut self,
        store: &mut dyn FbStore,
        sys: Option<&GuestMemory>,
        tr: Translate,
        keys: &[u64],
    ) -> SyncReport {
        let t0 = std::time::Instant::now();
        let mut rep = SyncReport::default();
        let scan = self.scan_back(store, sys, tr, Some(keys), &mut rep);
        let ns = t0.elapsed().as_nanos() as u64;
        self.stats.doorbell_syncs += 1;
        self.stats.doorbell_compared += scan.compared as u64;
        self.stats.doorbell_dirty += scan.dirty.len() as u64;
        self.stats.doorbell_ns += ns;
        self.stats.doorbell_ns_max = self.stats.doorbell_ns_max.max(ns);
        crate::profile::record(
            crate::profile::Key::Window("bar2 memfd: doorbell syncs (units: pages compared)"),
            std::time::Duration::from_nanos(ns),
            scan.compared as u64,
            None,
            "",
        );
        rep
    }

    /// Scan the live pages (those among `only`, when given) and write the changed bytes back
    /// (FB pages to the store at `tr(address)`, sysmem pages to guest RAM); the dirty FB
    /// pages go into `rep` and are marked dirty in the BAR2 walk.
    fn scan_back(
        &mut self,
        store: &mut dyn FbStore,
        sys: Option<&GuestMemory>,
        tr: Translate,
        only: Option<&[u64]>,
        rep: &mut SyncReport,
    ) -> Scan {
        let mut sys_skipped = 0u64;
        let scan = self.remap.scan_keys(
            &self.mem.view,
            only,
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
        self.stats.dirty += scan.dirty.len() as u64;
        self.stats.bytes_written_back += scan.bytes as u64;
        self.stats.conflicts += scan.conflicts as u64;
        self.stats.sys_skipped += sys_skipped;
        let dirty: Vec<u64> = scan.dirty.iter().copied().filter(|k| k & SYS == 0).collect();
        if let Some(c) = self.cache.borrow_mut().as_mut() {
            for &p in &dirty {
                if c.watches(p) {
                    c.mark_dirty(p);
                }
            }
        }
        rep.dirty.extend(dirty);
        rep.dirty.sort_unstable();
        rep.dirty.dedup();
        scan
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
        {
            let _why = crate::fb::ReadWhy::Bar2Drift.scope();
            store.read(tr(pa), &mut vram);
        }
        self.stats.drift_checked += 1;
        if self.remap.drifted(key, &vram) {
            self.stats.drift_found += 1;
            if self.stats.drift_first.len() < 8 && !self.stats.drift_first.contains(&pa) {
                self.stats.drift_first.push(pa);
            }
        }
    }

    /// Bring the BAR2 translation up to date with the tables rooted at `root`.
    #[allow(clippy::too_many_arguments)]
    fn follow_tables(
        &mut self,
        store: &mut dyn FbStore,
        fb_size: u64,
        root: u64,
        sys: Option<&GuestMemory>,
        tr: Translate,
        rep: &mut SyncReport,
        view: &TableView,
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
                    store: &*store,
                    tr,
                    fb_size,
                    sys,
                    private: &self.private,
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
            self.apply_work(work, store, fb_size, sys, tr, rep, view);
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
                    store: &*store,
                    tr,
                    fb_size,
                    sys,
                    private: &self.private,
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
        self.apply_work(work, store, fb_size, sys, tr, rep, view);
    }

    /// Re-point the BAR2 pages of the changed units `work` from their leaves. What the guest
    /// wrote through a BAR2 page that now maps another page is written back first (with a
    /// full scan before it finds nothing; a restricted one may not have covered it).
    #[allow(clippy::too_many_arguments)]
    fn apply_work(
        &mut self,
        work: Vec<(u64, Vec<Leaf>)>,
        store: &mut dyn FbStore,
        fb_size: u64,
        sys: Option<&GuestMemory>,
        tr: Translate,
        rep: &mut SyncReport,
        view: &TableView,
    ) {
        let wants: Vec<(u64, HashMap<u64, u64>)> = work
            .iter()
            .filter(|(u, _)| *u < self.size)
            .map(|(u, leaves)| (*u, self.unit_want(*u, leaves, fb_size)))
            .collect();
        let mut leaving: Vec<u64> = Vec::new();
        for (unit, want) in &wants {
            let (first, end) = (unit / PAGE, (unit + UNIT).min(self.size) / PAGE);
            for vp in first..end {
                if let Some(old) = self.remap.target(vp) {
                    if want.get(&vp) != Some(&old) {
                        leaving.push(old);
                    }
                }
            }
        }
        if !leaving.is_empty() {
            self.scan_back(store, sys, tr, Some(&leaving), rep);
        }
        for (unit, want) in wants {
            self.apply_unit(unit, want, &*store, sys, tr, rep, view);
        }
    }

    /// The page key each BAR2 page of the 2 MiB unit at `unit` should map, from its leaves.
    fn unit_want(&self, unit: u64, leaves: &[Leaf], fb_size: u64) -> HashMap<u64, u64> {
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
                            && !overlaps(&self.private, pa + k * PAGE, PAGE) =>
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
        want
    }

    /// Re-point every BAR2 page of the 2 MiB unit at `unit` to `want`.
    #[allow(clippy::too_many_arguments)]
    fn apply_unit(
        &mut self,
        unit: u64,
        want: HashMap<u64, u64>,
        store: &dyn FbStore,
        sys: Option<&GuestMemory>,
        tr: Translate,
        rep: &mut SyncReport,
        view: &TableView,
    ) {
        let first = unit / PAGE;
        let end = (unit + UNIT).min(self.size) / PAGE;
        let Bar2Area {
            remap,
            mem,
            traps,
            stats,
            victims,
            ..
        } = self;
        let victims = victims.get_mut();
        let mut kept: Vec<(u64, Box<PageBuf>)> = Vec::new();
        let mut loads = 0u64;
        let mut load = |key: u64| -> Box<PageBuf> {
            loads += 1;
            if key & SYS == 0 && !victims.is_empty() {
                if let Some(snap) = victims.take(key, view.epoch) {
                    victims.stats.load_hits += 1;
                    return snap;
                }
            }
            if key & SYS == 0 {
                victims.stats.load_misses += 1;
            }
            let _why = crate::fb::ReadWhy::Bar2Load.scope();
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
            if let (Some(k), Some(snap)) = (ch.left, ch.left_snap) {
                // A page table the device follows leaves BAR2: keep what VRAM holds.
                if (view.walked)(k) {
                    kept.push((k, snap));
                }
            }
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
        for (k, snap) in kept {
            victims.keep(k, snap, view.epoch);
        }
    }

    /// Before a function-level reset hands the area to the fresh model: forget every
    /// mapping and the BAR2 walk (the caller has synced).
    pub fn reset_mapping(&mut self) {
        *self.victims.get_mut() = Victims::default();
        self.remap.clear();
        *self.cache.borrow_mut() = None;
        self.cache_root.set(u64::MAX);
    }

    /// The victim cache's part of the summary line.
    fn victims_line(&self) -> String {
        let v = self.victims.borrow();
        let s = &v.stats;
        format!(
            "victim cache (page tables that left BAR2): {} kept, entries loaded from it {} (from VRAM {}), reads served {}, {} stale at use, {} dropped by a write behind the device, {} dropped when no walk read them any more, {} evicted, {} held now",
            s.kept, s.load_hits, s.load_misses, s.read_hits, s.stale, s.dropped, s.unwalked, s.evicted, v.len()
        )
    }

    /// The sync points' part of the summary line.
    fn points_line(&self, s: &Bar2Stats) -> String {
        let p = |x: Point| {
            format!("{} ({} us)", s.by_point[x as usize], s.point_ns[x as usize] / 1000)
        };
        format!(
            "full syncs by point: {} flush read(s), {} FB flush(es) ({} NV_UFLUSH_FB_FLUSH write(s)), {} MMU invalidate(s), {} GSP queue head(s), {} BAR2 bind(s), {} doorbell(s) with a flush owed, {} by the clock; MMU invalidates: {} table sync(s) of {} live table page(s) ({} us; walks' live lists {} reused, {} listed), {} of them with a flush owed left to the next engine start; channel doorbells: {} set(s) computed (at most {} live page(s), at most {} live table page(s) left to the invalidate), {} checked, {} with nothing observable behind the memfd, {} with memory the device cannot name (all live pages), {} partial sync(s) ({} page compare(s), {} dirty page(s), {} us total, {} us max)",
            p(Point::FlushRead),
            p(Point::FbFlush),
            s.fb_flushes,
            p(Point::MmuInvalidate),
            p(Point::GspQueueHead),
            p(Point::Bar2Bind),
            p(Point::Doorbell),
            p(Point::Clock),
            s.table_syncs,
            s.table_sync_pages,
            s.table_sync_ns / 1000,
            s.table_key_lists_reused,
            s.table_key_lists_listed,
            s.table_syncs_owed,
            s.observe_sets,
            s.observe_pages_max,
            s.observe_tables_max,
            s.doorbells_checked,
            s.doorbells_clean,
            s.doorbells_unknown,
            s.doorbell_syncs,
            s.doorbell_compared,
            s.doorbell_dirty,
            s.doorbell_ns / 1000,
            s.doorbell_ns_max / 1000,
        )
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
            "bar2 memfd: BAR2 [0, {:#x}) in host RAM, trapped page(s) [{}] with {} access(es); {} sync(s) ({} us total, {} us max), {} page compare(s), {} dirty page(s), {} byte(s) written back, {} alias conflict(s); {} BAR2 table walk(s), {} page(s) entered ({} loaded), {} left; now {} page(s) mapped at {} BAR2 page(s), at most {} live and {} alias(es) of one page; {} sysmem write-back(s) skipped; flush reads {}; {}; VRAM behind the memfd: {} of {} sampled page(s) changed{} (docs/caraxes/bar2-memfd.md, display-pacing.md)",
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
            self.points_line(&s) + "; " + &self.victims_line(),
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

    /// The doorbell rule's pieces (docs/caraxes/bar2-memfd.md §3, rule 4): the live keys
    /// inside `(start, bytes)` pieces or among page numbers, FB and sysmem apart; the live
    /// generation moves only when a page enters or leaves; a scan of some keys writes back
    /// only those and leaves the others' changes to a later scan.
    #[test]
    fn observed_pages_and_a_keyed_scan() {
        let cfg = config_from(Some("none"), false, true, None).unwrap();
        let mut a = Bar2Area::create(16 * PAGE, &cfg).unwrap();
        let guest = a.mem.map_again().unwrap();
        assert!(a.observed(&[(Target::Fb(0), u64::MAX)], &[]).is_empty(), "nothing live");
        let g0 = a.live_generation();
        let sys = key_of(Target::Sys(0x500_0000)).unwrap();
        for (va, key) in [(1, 0x100), (2, 0x101), (3, 0x200), (4, sys)] {
            a.remap.set(va, Some(key), &a.mem.view, &mut |_| filled(0));
        }
        let g1 = a.live_generation();
        assert_ne!(g1, g0);
        a.remap.set(5, Some(0x100), &a.mem.view, &mut |_| filled(0));
        assert_eq!(a.live_generation(), g1, "an alias adds no page");
        // 0x100800..0x101800 touches FB pages 0x100 and 0x101.
        assert_eq!(a.observed(&[(Target::Fb(0x10_0800), 0x1000)], &[]), vec![0x100, 0x101]);
        assert_eq!(a.observed(&[], &[0x200, 0x300]), vec![0x200]);
        assert_eq!(a.observed(&[(Target::Sys(0x500_0000), 1)], &[]), vec![sys]);
        assert_eq!(a.observed(&[(Target::Fb(0x500_0000), 1)], &[]), Vec::<u64>::new(), "FB, not sysmem");
        assert!(a.observed(&[(Target::Fb(0x30_0000), 0x10_0000), (Target::Fb(0), 0)], &[]).is_empty());
        assert_eq!(a.observed(&[(Target::Fb(u64::MAX - 1), 16)], &[]), Vec::<u64>::new(), "no overflow");
        assert_eq!(a.live_keys(), vec![0x100, 0x101, 0x200, sys]);
        // The guest writes pages 0x100 (through its alias 5) and 0x200; a scan of 0x100 and
        // an unmapped key writes back 0x100 only.
        guest.write(5 * PAGE + 8, &1u64.to_le_bytes());
        guest.write(3 * PAGE, &3u64.to_le_bytes());
        let mut wb = Vec::new();
        let s = a.remap.scan_keys(&a.mem.view, Some(&[0x100, 0x999]), &mut |k, off, b| {
            wb.push((k, off, b.to_vec()))
        });
        assert_eq!((s.compared, s.dirty.clone()), (2, vec![0x100]), "two aliases compared");
        assert_eq!(wb, vec![(0x100, 8, vec![1])]);
        let s = a.remap.scan(&a.mem.view, &mut |_, _, _| {});
        assert_eq!(s.dirty, vec![0x200], "left for the full scan");
        a.remap.set(3, None, &a.mem.view, &mut |_| filled(0));
        assert_ne!(a.live_generation(), g1);
        assert_eq!(a.live_keys(), vec![0x100, 0x101, sys]);
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

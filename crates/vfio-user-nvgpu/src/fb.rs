// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The fake framebuffer for milestone 1 (docs/caraxes/fb-model.md §4): sparse
//! host RAM sized to the advertised FB, reached by the guest through the PRAMIN
//! window (BAR0 0x700000 plus `NV_PBUS_BAR0_WINDOW`) and through BAR3/BAR1 with a
//! walk of the guest's own GMMU page tables, which live in this same memory.
//!
//! [`FbLayout`] says which FB addresses the guest uses. Without a host VRAM
//! reservation the FB is `[0, profile)` as in milestone 1. With one (E5b,
//! `--reserve-vram`, docs/caraxes/memory-model.md) the guest's FB address space is
//! the host card's, and every address the guest uses lies in the reserved host range:
//! guest FB address X is host VRAM address X. The fake FB stays the backing store
//! until the host channels come (E5b part 2) and counts every access outside the
//! reservation.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use crate::guestmem::GuestMemory;

const PAGE: u64 = 4096;
const MIB: u64 = 1 << 20;

/// The device-owned top of a reservation: the BAR2 and BAR1 root page directories
/// (`bar2PdeBase`, `bar1PdeBase` of the static info), kept 2 MiB so the usable range
/// stays 2 MiB aligned for big pages.
pub const CARVE_OUT: u64 = 2 * MIB;

/// Which FB addresses the guest is given (docs/caraxes/memory-model.md §1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FbLayout {
    /// The FB address space: `NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE`, from which the guest
    /// computes its WPR layout top-down; the fake FB spans it.
    pub addr_space: u64,
    /// The profile's FB size: `GspStaticConfigInfo.fb_length`, what `nvidia-smi` reports.
    pub profile: u64,
    /// The host VRAM reservation `[base, end)`, when there is one.
    pub reservation: Option<(u64, u64)>,
}

impl FbLayout {
    /// Milestone 1 to E5a: FB `[0, profile)`, the fake GSP's WPR at its top.
    pub fn plain(profile: u64) -> Self {
        FbLayout {
            addr_space: profile,
            profile,
            reservation: None,
        }
    }

    /// Guest FB addresses equal to host VRAM addresses: the address space is the host
    /// card's (`addr_space`, its whole FB), the usable FB is the reservation
    /// `[base, end)` minus the device's carve-out at its top, and everything else is
    /// reserved. The guest computes its (fake) WPR below `addr_space`, above `end`.
    pub fn reserved(profile: u64, addr_space: u64, base: u64, end: u64) -> Result<Self, String> {
        if base % (2 * MIB) != 0 || end % (2 * MIB) != 0 || end <= base + CARVE_OUT {
            return Err(format!(
                "reservation {base:#x}..{end:#x} is not a 2 MiB aligned range above the carve-out"
            ));
        }
        if end - base < profile {
            return Err(format!(
                "reservation {base:#x}..{end:#x} is smaller than the profile ({profile:#x})"
            ));
        }
        if end > addr_space {
            return Err(format!(
                "reservation end {end:#x} beyond the FB address space {addr_space:#x}"
            ));
        }
        Ok(FbLayout {
            addr_space,
            profile,
            reservation: Some((base, end)),
        })
    }

    /// `(bar2PdeBase, bar1PdeBase)` in the carve-out, with a reservation.
    pub fn bar_roots(&self) -> Option<(u64, u64)> {
        self.reservation
            .map(|(_, end)| (end - CARVE_OUT, end - CARVE_OUT + PAGE))
    }

    /// The usable FB `[base, end - CARVE_OUT)` the region table gives the guest's heap,
    /// with a reservation.
    pub fn usable(&self) -> Option<(u64, u64)> {
        self.reservation.map(|(base, end)| (base, end - CARVE_OUT))
    }

    /// Whether `[pa, pa + len)` lies inside the reservation (always, without one).
    pub fn inside(&self, pa: u64, len: u64) -> bool {
        match self.reservation {
            None => true,
            Some((base, end)) => pa >= base && pa.saturating_add(len) <= end,
        }
    }
}

/// Accesses of the fake FB outside the reservation (E5b check: none may happen).
#[derive(Default)]
pub struct Outside {
    pub reads: Cell<u64>,
    pub writes: Cell<u64>,
    pub first: RefCell<Vec<(char, u64, usize)>>,
}

/// Where a BAR access lands after the page-table walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Video memory (the fake FB) at this address.
    Fb(u64),
    /// Guest system memory at this physical address.
    Sys(u64),
    /// Sparse mapping: reads give 0, writes are dropped.
    Sparse,
    /// Not mapped (invalid entry at `level`).
    Unmapped { level: u8 },
}

/// Where the bytes of the FB live. E5b part 1: host RAM ([`HostRamStore`]); part 2
/// puts the real reserved VRAM behind the same two calls (windows of the reservation
/// mapped through the host's BAR1, or forwarded writes). Addresses are FB addresses,
/// already checked against the address space.
pub trait FbStore: Send {
    /// Fill `buf` from FB address `pa`.
    fn read(&self, pa: u64, buf: &mut [u8]);
    /// Store `data` at FB address `pa`.
    fn write(&mut self, pa: u64, data: &[u8]);
    /// Pages holding data (for the tests and the summary).
    fn pages_touched(&self) -> usize {
        0
    }
    /// The counters of a VRAM-backed store, if this is one.
    fn vram_stats(&self) -> Option<VramStats> {
        None
    }
    /// Drain any write-combining buffers to real VRAM (VRAM store only; no-op otherwise).
    fn flush(&self) {}
}

/// Sparse host RAM, zero until written (fb-model.md §4a).
#[derive(Default)]
pub struct HostRamStore {
    pages: HashMap<u64, Box<[u8; PAGE as usize]>>,
}

impl FbStore for HostRamStore {
    fn read(&self, pa: u64, buf: &mut [u8]) {
        let mut done = 0usize;
        while done < buf.len() {
            let o = pa + done as u64;
            let in_page = (o % PAGE) as usize;
            let n = (PAGE as usize - in_page).min(buf.len() - done);
            match self.pages.get(&(o / PAGE)) {
                Some(p) => buf[done..done + n].copy_from_slice(&p[in_page..in_page + n]),
                None => buf[done..done + n].fill(0),
            }
            done += n;
        }
    }

    fn write(&mut self, pa: u64, data: &[u8]) {
        let mut done = 0usize;
        while done < data.len() {
            let o = pa + done as u64;
            let in_page = (o % PAGE) as usize;
            let n = (PAGE as usize - in_page).min(data.len() - done);
            let p = self
                .pages
                .entry(o / PAGE)
                .or_insert_with(|| Box::new([0u8; PAGE as usize]));
            p[in_page..in_page + n].copy_from_slice(&data[done..done + n]);
            done += n;
        }
    }

    fn pages_touched(&self) -> usize {
        self.pages.len()
    }
}

/// A hook that sees every FB write before the store takes it (docs/caraxes/dma-routes.md
/// §4: the place where a page-table write filter will sit; today a no-op).
pub trait WriteFilter: Send {
    fn before_write(&mut self, _pa: u64, _data: &[u8]) {}
}

/// The default filter: nothing is rewritten.
pub struct NoFilter;

impl WriteFilter for NoFilter {}

/// Something that gives CPU windows onto the reservation ([`VramStore`]): the host
/// memory client in the device, a RAM fake in the tests.
pub trait WindowSource: Send {
    /// Map `len` bytes at reservation offset `offset`; the error is an RM status.
    fn map(&mut self, offset: u64, len: usize) -> Result<Box<dyn WindowBacking>, u32>;
}

/// One mapped window: byte access at reservation offsets.
pub trait WindowBacking: Send {
    fn offset(&self) -> u64;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn flush(&self) {}
    fn read(&self, off: u64, buf: &mut [u8]);
    fn write(&self, off: u64, data: &[u8]);
}

impl WindowBacking for crate::hostmem::Window {
    fn offset(&self) -> u64 {
        self.offset
    }
    fn len(&self) -> usize {
        self.len
    }
    fn read(&self, off: u64, buf: &mut [u8]) {
        crate::hostmem::Window::read(self, off, buf)
    }
    fn write(&self, off: u64, data: &[u8]) {
        crate::hostmem::Window::write(self, off, data)
    }
    fn flush(&self) {
        crate::hostmem::Window::flush(self)
    }
}

impl WindowSource for crate::hostmem::ReservationMapper {
    fn map(&mut self, offset: u64, len: usize) -> Result<Box<dyn WindowBacking>, u32> {
        // Never through the `HostMemory` lock: the FB is walked while it is held.
        let w = crate::hostmem::ReservationMapper::map(self, offset, len)?;
        Ok(Box::new(w))
    }
}

/// Counters of the VRAM store, for the exit summary.
#[derive(Default, Debug, Clone)]
pub struct VramStats {
    pub maps: u64,
    pub map_failures: u64,
    /// Retries of an access for which the host granted no window at all (stalled, never
    /// dropped).
    pub stalls: u64,
    pub evictions: u64,
    pub reads: u64,
    pub writes: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
}

struct Slot {
    win: Box<dyn WindowBacking>,
    last_use: u64,
}

/// The reserved host VRAM as the FB backing (E5b part 2, option C): every guest FB
/// access inside the reservation goes to the real VRAM through a BAR1 window of the
/// reservation, so the GPFIFO/USERD pages, page tables and context buffers the guest
/// writes are where the physical GPU reads them. Windows (64 MiB by default, smaller
/// when BAR1 has no room) are mapped on demand and kept LRU. Addresses outside the
/// reservation (the fake WPR above it) fall back to host RAM and are counted by
/// [`FakeFb`] as before.
pub struct VramStore {
    src: RefCell<Box<dyn WindowSource>>,
    base: u64,
    end: u64,
    windows: RefCell<Vec<Slot>>,
    tick: Cell<u64>,
    max_windows: usize,
    window_len: Cell<usize>,
    /// Tests only: give up after this many stalls (0: never).
    stall_limit: Cell<u64>,
    outside: RefCell<HostRamStore>,
    stats: RefCell<VramStats>,
}

/// The default window: 32 MiB. BAR1 is a shared host resource (256 MiB on this card,
/// no resizable BAR): the desktop's own mappings and every device's windows come out of
/// it. 2 x 32 MiB per device leaves the desktop about 40 MiB of its measured ~232 MiB
/// free even with three devices up (P20, docs/caraxes/parallel.md §2; three devices at
/// 3 x 64 MiB exhausted it). `CARAXES_BAR1_WINDOW_MIB` (a power of two, 2..=64) and
/// `CARAXES_BAR1_WINDOWS` (1..=8) set another budget.
pub const VRAM_WINDOW: usize = 32 << 20;
/// Windows held at once by default.
pub const VRAM_WINDOWS: usize = 2;

/// The per-device BAR1 budget: (window bytes, windows), from the environment.
pub fn bar1_budget() -> (usize, usize) {
    let len = std::env::var("CARAXES_BAR1_WINDOW_MIB")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|m| m.is_power_of_two() && (2..=64).contains(m))
        .map_or(VRAM_WINDOW, |m| m << 20);
    let n = std::env::var("CARAXES_BAR1_WINDOWS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| (1..=8).contains(n))
        .unwrap_or(VRAM_WINDOWS);
    (len, n)
}

impl VramStore {
    pub fn new(src: Box<dyn WindowSource>, base: u64, end: u64) -> Self {
        let (len, n) = bar1_budget();
        Self::with_budget(src, base, end, len, n)
    }

    /// A store holding at most `max_windows` BAR1 windows of `window_len` bytes.
    pub fn with_budget(
        src: Box<dyn WindowSource>,
        base: u64,
        end: u64,
        window_len: usize,
        max_windows: usize,
    ) -> Self {
        VramStore {
            src: RefCell::new(src),
            base,
            end,
            windows: RefCell::new(Vec::new()),
            tick: Cell::new(0),
            max_windows,
            window_len: Cell::new(window_len),
            stall_limit: Cell::new(0),
            outside: RefCell::new(HostRamStore::default()),
            stats: RefCell::new(VramStats::default()),
        }
    }

    pub fn stats(&self) -> VramStats {
        self.stats.borrow().clone()
    }

    pub fn windows_held(&self) -> Vec<(u64, usize)> {
        self.windows
            .borrow()
            .iter()
            .map(|s| (s.win.offset(), s.win.len()))
            .collect()
    }

    /// The window holding reservation offset `off`, mapped if need be (LRU eviction);
    /// its index in `windows`. `None` when the host refused every window size.
    fn window_for(&self, off: u64) -> Option<usize> {
        let tick = self.tick.get() + 1;
        self.tick.set(tick);
        {
            let mut ws = self.windows.borrow_mut();
            if let Some(i) = ws
                .iter()
                .position(|s| off >= s.win.offset() && off < s.win.offset() + s.win.len() as u64)
            {
                ws[i].last_use = tick;
                return Some(i);
            }
        }
        // Not mapped: try the current window size, halving on refusal down to 2 MiB.
        loop {
            let wlen = self.window_len.get();
            let start = off & !(wlen as u64 - 1);
            let len = (wlen as u64).min(self.end - self.base - start) as usize;
            let r = self.src.borrow_mut().map(start, len);
            let mut st = self.stats.borrow_mut();
            match r {
                Ok(w) => {
                    st.maps += 1;
                    drop(st);
                    let mut ws = self.windows.borrow_mut();
                    if ws.len() >= self.max_windows {
                        let (i, _) = ws
                            .iter()
                            .enumerate()
                            .min_by_key(|(_, s)| s.last_use)
                            .expect("non-empty");
                        // Drain the window's write-combining buffers to VRAM before it is
                        // unmapped: an evicted window may hold guest writes the GPU has not
                        // read yet (coordinator's flush-rule extension, exec-formats.md §3).
                        ws[i].win.flush();
                        ws.remove(i);
                        self.stats.borrow_mut().evictions += 1;
                    }
                    ws.push(Slot {
                        win: w,
                        last_use: tick,
                    });
                    return Some(ws.len() - 1);
                }
                Err(_) => {
                    st.map_failures += 1;
                    drop(st);
                    // BAR1 is shared with the host desktop and the other devices: give
                    // back our own least recently used window and retry at this size
                    // before asking for a smaller one, so a device never grows its BAR1
                    // share under pressure and never loops on refused maps (P20: two
                    // devices at 3 x 64 MiB drove the host into ~690k refused BAR1 maps).
                    let mut ws = self.windows.borrow_mut();
                    if !ws.is_empty() {
                        let (i, _) = ws
                            .iter()
                            .enumerate()
                            .min_by_key(|(_, s)| s.last_use)
                            .expect("non-empty");
                        ws[i].win.flush();
                        ws.remove(i);
                        self.stats.borrow_mut().evictions += 1;
                        continue;
                    }
                    drop(ws);
                    if wlen <= 2 << 20 {
                        // No window at all, not even 2 MiB: stall the access and retry,
                        // never drop a guest write or invent a read (before P20 this
                        // path zero-filled reads and dropped writes silently).
                        let mut st = self.stats.borrow_mut();
                        st.stalls += 1;
                        let n = st.stalls;
                        drop(st);
                        if n == 1 || n % 1000 == 0 {
                            eprintln!(
                                "vfio-user-nvgpu: ERROR: fb: no BAR1 window of the reservation can be mapped (offset {off:#x}); stalling the access and retrying ({n} stall(s) so far)"
                            );
                        }
                        if self.stall_limit.get() != 0 && n >= self.stall_limit.get() {
                            return None;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        continue;
                    }
                    self.window_len.set(wlen / 2);
                }
            }
        }
    }

    /// Split `[pa, pa + len)` into pieces inside one window each (or outside the
    /// reservation) and apply `f(piece, fb_address, piece_len, done)`.
    fn pieces(&self, pa: u64, len: usize, mut f: impl FnMut(Piece, u64, usize, usize)) {
        let mut done = 0usize;
        while done < len {
            let a = pa + done as u64;
            if a < self.base || a >= self.end {
                let stop = if a < self.base {
                    (self.base - a).min((len - done) as u64) as usize
                } else {
                    len - done
                };
                f(Piece::Outside, a, stop, done);
                done += stop;
                continue;
            }
            let off = a - self.base;
            let wlen = self.window_len.get() as u64;
            let in_win = (wlen - (off & (wlen - 1))).min((len - done) as u64) as usize;
            let in_win = in_win.min((self.end - a) as usize);
            let piece = match self.window_for(off) {
                Some(i) => Piece::Window(i),
                None => Piece::Unmapped,
            };
            f(piece, a, in_win, done);
            done += in_win;
        }
    }
}

/// Where one piece of an FB access lands.
#[derive(Clone, Copy)]
enum Piece {
    /// Inside the reservation, in the window at this index.
    Window(usize),
    /// Inside the reservation, but the host granted no window.
    Unmapped,
    /// Outside the reservation: host RAM.
    Outside,
}

impl FbStore for VramStore {
    fn read(&self, pa: u64, buf: &mut [u8]) {
        {
            let mut st = self.stats.borrow_mut();
            st.reads += 1;
            st.bytes_read += buf.len() as u64;
        }
        let len = buf.len();
        self.pieces(pa, len, |w, a, n, done| match w {
            Piece::Window(i) => self.windows.borrow()[i]
                .win
                .read(a - self.base, &mut buf[done..done + n]),
            Piece::Outside => self.outside.borrow().read(a, &mut buf[done..done + n]),
            Piece::Unmapped => buf[done..done + n].fill(0),
        });
    }

    fn write(&mut self, pa: u64, data: &[u8]) {
        {
            let mut st = self.stats.borrow_mut();
            st.writes += 1;
            st.bytes_written += data.len() as u64;
        }
        let len = data.len();
        self.pieces(pa, len, |w, a, n, done| match w {
            Piece::Window(i) => self.windows.borrow()[i]
                .win
                .write(a - self.base, &data[done..done + n]),
            Piece::Outside => self.outside.borrow_mut().write(a, &data[done..done + n]),
            Piece::Unmapped => {}
        });
    }

    fn pages_touched(&self) -> usize {
        self.outside.borrow().pages_touched()
    }

    fn vram_stats(&self) -> Option<VramStats> {
        Some(self.stats())
    }

    fn flush(&self) {
        for slot in self.windows.borrow().iter() {
            slot.win.flush();
        }
    }
}

pub struct FakeFb {
    size: u64,
    store: Box<dyn FbStore>,
    filter: Box<dyn WriteFilter>,
    /// The reservation; accesses outside it are counted in `outside`.
    bounds: Option<(u64, u64)>,
    pub outside: Outside,
    /// Distinct `(kind, comptagline)` of the vidmem PTEs the walker resolved, capped, for
    /// the comptag safety check (execution.md §7): every vidmem PTE must be kind 0x06/0x00
    /// with comptagline 0 before any mirrored execution. `(kind, comptagline) -> count`.
    pte_kinds: RefCell<std::collections::BTreeMap<(u8, u32), u64>>,
    /// The cached walk of every guest root the shadow mirrors (ptcache.rs), keyed by the
    /// root's FB address (a sysmem root with bit 63): a guest write to one of its table
    /// pages marks that page dirty, and the next update re-reads only the dirty pages.
    walks: RefCell<std::collections::BTreeMap<u64, crate::ptcache::WalkCache>>,
    /// The BAR aperture TLB (docs/caraxes/mapping.md §3): `(root, va page) -> the page's
    /// target`, filled by the walk of a trapped BAR1/BAR2 access. Every BAR access used to
    /// walk five table levels through uncached BAR1 reads; a context creation makes about
    /// 100k of them. Flushed whole when a guest write reaches a table page any entry was
    /// walked through (`bar_tlb_pages`); `CARAXES_BAR_TLB=0` walks every access.
    bar_tlb: RefCell<HashMap<(u64, u64), Target>>,
    bar_tlb_pages: RefCell<std::collections::HashSet<u64>>,
    bar_tlb_on: bool,
    pub bar_tlb_stats: Cell<(u64, u64, u64)>,
    /// Watched dwords: `pa -> last value written or read`, kept current by `note_write`
    /// (every guest write to VRAM passes it), so that a hot reader such as the token-less
    /// doorbell scan (channel.rs `Registry::pending`, USERD GPPut of every channel) costs a
    /// map lookup instead of an uncached BAR1 read (measured 1.6 us per read, 2.5 ms on a
    /// window miss). `watched_pages` is the page set for the fast check in `note_write`.
    watched: RefCell<HashMap<u64, u32>>,
    watched_pages: RefCell<std::collections::HashSet<u64>>,
    /// The guest's page tables are in the GMMU v3 format (Hopper and Blackwell,
    /// `gmmu-v3-57bit`): the walks below hand over to [`crate::gmmu_v3`]; false keeps the
    /// v2 walker of this file.
    mmu_v3: bool,
}

impl Default for FakeFb {
    fn default() -> Self {
        FakeFb::new(0)
    }
}

impl FakeFb {
    pub fn new(size: u64) -> Self {
        FakeFb {
            size,
            store: Box::new(HostRamStore::default()),
            filter: Box::new(NoFilter),
            bounds: None,
            outside: Outside::default(),
            pte_kinds: RefCell::new(std::collections::BTreeMap::new()),
            walks: RefCell::new(std::collections::BTreeMap::new()),
            bar_tlb: RefCell::new(HashMap::new()),
            bar_tlb_pages: RefCell::new(std::collections::HashSet::new()),
            bar_tlb_on: std::env::var("CARAXES_BAR_TLB").as_deref() != Ok("0"),
            bar_tlb_stats: Cell::new((0, 0, 0)),
            watched: RefCell::new(HashMap::new()),
            watched_pages: RefCell::new(std::collections::HashSet::new()),
            mmu_v3: false,
        }
    }

    /// Select the page-table format of the chip (`ChipData::gmmu.mmu_format`):
    /// `gmmu-v3-57bit` walks with [`crate::gmmu_v3`], anything else with the v2 walker.
    pub fn with_mmu_format(mut self, format: &str) -> Self {
        self.mmu_v3 = format == "gmmu-v3-57bit";
        self
    }

    /// The current value of the dword at `pa`, watched from now on: the first call reads
    /// it from the store, later calls answer from the copy `note_write` keeps current.
    /// `None` when `pa` is outside the store.
    pub fn watched_dword(&self, pa: u64) -> Option<u32> {
        if let Some(v) = self.watched.borrow().get(&pa) {
            return Some(*v);
        }
        let mut b = [0u8; 4];
        if !self.read(pa, &mut b) {
            return None;
        }
        let v = u32::from_le_bytes(b);
        self.watched.borrow_mut().insert(pa, v);
        self.watched_pages.borrow_mut().insert(pa >> 12);
        Some(v)
    }

    /// Forget the watch on `pa` (the next `watched_dword` reads the store again).
    pub fn unwatch_dword(&self, pa: u64) {
        self.watched.borrow_mut().remove(&pa);
    }

    fn watch_key(root: Target) -> u64 {
        match root {
            Target::Fb(pa) => pa,
            Target::Sys(pa) => pa | 1 << 63,
            _ => u64::MAX,
        }
    }

    /// Whether the cached walk of `root` has work to do (never built, a table page was
    /// written since the last update, or a table lives in sysmem).
    pub fn tables_dirty(&self, root: Target) -> bool {
        self.walks
            .borrow()
            .get(&Self::watch_key(root))
            .is_none_or(|c| c.is_dirty())
    }

    /// Stop watching `root` (drops its cached walk).
    pub fn unwatch_tables(&self, root: Target) {
        self.walks.borrow_mut().remove(&Self::watch_key(root));
    }

    /// Bring the cached walk of `root` up to date (ptcache.rs): only the dirty table pages
    /// are read again, or everything when `full` (the always-resync reference), on the first
    /// call, or when a table lives in guest sysmem. Returns the units whose leaves may have
    /// changed.
    pub fn walk_update(
        &self,
        sys: Option<&GuestMemory>,
        root: Target,
        full: bool,
        max_leaves: usize,
    ) -> crate::ptcache::Update {
        let reader = FbReader { fb: self, sys };
        let mut walks = self.walks.borrow_mut();
        let c = walks
            .entry(Self::watch_key(root))
            .or_insert_with(|| {
                let mut c = crate::ptcache::WalkCache::new(root, max_leaves);
                c.reread = std::env::var("CARAXES_SHADOW_REREAD").as_deref() == Ok("1");
                c
            });
        c.update(&reader, full)
    }

    /// Run `f` on the cached walk of `root` (after [`Self::walk_update`]).
    pub fn with_walk<R>(
        &self,
        root: Target,
        f: impl FnOnce(&crate::ptcache::WalkCache) -> R,
    ) -> Option<R> {
        self.walks.borrow().get(&Self::watch_key(root)).map(f)
    }

    fn note_write(&self, pa: u64, data: &[u8]) {
        let last = (pa + data.len().max(1) as u64 - 1) >> 12;
        {
            let wp = self.watched_pages.borrow();
            if !wp.is_empty() && (pa >> 12..=last).any(|p| wp.contains(&p)) {
                let end = pa + data.len() as u64;
                let mut w = self.watched.borrow_mut();
                let hit: Vec<u64> = w
                    .keys()
                    .copied()
                    .filter(|&k| k < end && k + 4 > pa)
                    .collect();
                for k in hit {
                    if k >= pa && k + 4 <= end {
                        let o = (k - pa) as usize;
                        w.insert(k, u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]));
                    } else {
                        // A partial overwrite: re-read from the store on the next use.
                        w.remove(&k);
                    }
                }
            }
        }
        {
            let mut tp = self.bar_tlb_pages.borrow_mut();
            if !tp.is_empty() && (pa >> 12..=last).any(|p| tp.contains(&p)) {
                tp.clear();
                self.bar_tlb.borrow_mut().clear();
                let (h, m, f) = self.bar_tlb_stats.get();
                self.bar_tlb_stats.set((h, m, f + 1));
            }
        }
        let mut walks = self.walks.borrow_mut();
        if walks.is_empty() {
            return;
        }
        for page in pa >> 12..=last {
            for c in walks.values_mut() {
                if c.watches(page) {
                    c.note_write(page, pa, data);
                }
            }
        }
    }

    /// Record the KIND (bits 63:56) and COMPTAGLINE (bits 55:36) of a vidmem PTE the
    /// walker resolved (Turing v2, exec-formats.md §2.6): distinct pairs, capped.
    fn observe_pte(&self, pte: u64) {
        if pte & 1 == 0 || (pte >> 1) & 3 != 0 {
            return; // not a valid vidmem PTE
        }
        let key = ((pte >> 56) as u8, ((pte >> 36) & 0xF_FFFF) as u32);
        let mut m = self.pte_kinds.borrow_mut();
        if m.len() < 64 || m.contains_key(&key) {
            *m.entry(key).or_insert(0) += 1;
        }
    }

    /// The distinct `(kind, comptagline, count)` of the vidmem PTEs walked, for the
    /// exit summary. Kind 0x06/0x00 with comptagline 0 is the safe (uncompressed) set.
    pub fn pte_kinds(&self) -> Vec<(u8, u32, u64)> {
        self.pte_kinds
            .borrow()
            .iter()
            .map(|(&(k, c), &n)| (k, c, n))
            .collect()
    }

    /// Put a write filter in front of the store (dma-routes.md §4; none today).
    pub fn set_write_filter(&mut self, filter: Box<dyn WriteFilter>) {
        self.filter = filter;
    }

    /// The VRAM store's counters, when the backing is the reserved VRAM.
    pub fn vram_stats(&self) -> Option<VramStats> {
        self.store.vram_stats()
    }

    /// Drain the store's write-combining buffers to VRAM before the GPU reads what the
    /// guest wrote (before a host doorbell or TLB invalidate).
    pub fn flush(&self) {
        self.store.flush();
    }

    /// The fake FB of `layout`: its address space, counting accesses outside the
    /// reservation.
    pub fn for_layout(layout: &FbLayout) -> Self {
        let mut fb = FakeFb::new(layout.addr_space);
        fb.bounds = layout.reservation;
        fb
    }

    /// Replace the backing store (E5b part 2: the reserved VRAM).
    pub fn with_store(mut self, store: Box<dyn FbStore>) -> Self {
        self.store = store;
        self
    }

    fn check(&self, kind: char, pa: u64, len: usize) {
        if let Some((base, end)) = self.bounds {
            if pa < base || pa + len as u64 > end {
                let c = if kind == 'w' {
                    &self.outside.writes
                } else {
                    &self.outside.reads
                };
                c.set(c.get() + 1);
                let mut f = self.outside.first.borrow_mut();
                if f.len() < 16 {
                    f.push((kind, pa, len));
                }
            }
        }
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn pages_touched(&self) -> usize {
        self.store.pages_touched()
    }

    pub fn read(&self, pa: u64, buf: &mut [u8]) -> bool {
        self.check('r', pa, buf.len());
        if pa + buf.len() as u64 > self.size {
            return false;
        }
        self.store.read(pa, buf);
        true
    }

    pub fn write(&mut self, pa: u64, data: &[u8]) -> bool {
        self.check('w', pa, data.len());
        if pa + data.len() as u64 > self.size {
            return false;
        }
        self.filter.before_write(pa, data);
        self.note_write(pa, data);
        self.store.write(pa, data);
        true
    }

    pub fn read_u64(&self, pa: u64) -> Option<u64> {
        let mut b = [0u8; 8];
        self.read(pa, &mut b).then(|| u64::from_le_bytes(b))
    }

    pub fn write_u64(&mut self, pa: u64, v: u64) -> bool {
        self.write(pa, &v.to_le_bytes())
    }

    /// Follow a PDE (PD3/PD2/PD1): aperture bits 2:1 (1 video, 2/3 sysmem); the
    /// video address is bits 32:8, the sysmem address bits 53:8, both `<< 12`
    /// (`pascal/gp100/dev_mmu.h:113-119`; docs/caraxes/fb-model.md §1.5).
    fn next_table(entry: u64) -> Option<Target> {
        match (entry >> 1) & 3 {
            1 => Some(Target::Fb(((entry >> 8) & 0x1FF_FFFF) << 12)),
            2 | 3 => Some(Target::Sys(((entry >> 8) & 0x3FFF_FFFF_FFFF) << 12)),
            _ => None,
        }
    }

    /// The big-page table of a dual PDE's low qword: aperture 2:1, address bits
    /// 32:4 (video) or 53:4 (sysmem) `<< 8` (`pascal/gp100/dev_mmu.h:98-103`).
    fn big_table(lo: u64) -> Option<Target> {
        match (lo >> 1) & 3 {
            1 => Some(Target::Fb(((lo >> 4) & 0x1FFF_FFFF) << 8)),
            2 | 3 => Some(Target::Sys(((lo >> 4) & 0x3_FFFF_FFFF_FFFF) << 8)),
            _ => None,
        }
    }

    fn decode_pte(pte: u64, offset: u64) -> Target {
        if pte & 1 == 0 {
            return if pte & 8 != 0 {
                Target::Sparse
            } else {
                Target::Unmapped { level: 0 }
            };
        }
        match (pte >> 1) & 3 {
            0 => Target::Fb((((pte >> 8) & 0x1FF_FFFF) << 12) + offset),
            2 | 3 => Target::Sys((((pte >> 8) & 0x3FFF_FFFF_FFFF) << 12) + offset),
            _ => Target::Unmapped { level: 0 },
        }
    }

    /// One qword of a page table that lives in the fake FB or, when `sys` is
    /// given, in guest RAM.
    fn table_u64(&self, sys: Option<&GuestMemory>, table: Target, off: u64) -> Option<u64> {
        match table {
            Target::Fb(pa) => self.read_u64(pa + off),
            Target::Sys(pa) => sys?.read_u64(pa + off),
            _ => None,
        }
    }

    /// Translate `va` through the 49-bit GMMU format rooted at `root` (a PD3 in
    /// the fake FB): fb-model.md §4b. Page tables in sysmem are not followed,
    /// which is right for BAR1/BAR2 on TU (fb-model.md §4b).
    pub fn translate(&self, root: u64, va: u64) -> Target {
        self.translate_in(None, Target::Fb(root), va)
    }

    /// The same walk for a GPU virtual address space whose tables may also live
    /// in guest RAM (`sys`), as UVM's and the channels' address spaces can.
    pub fn translate_in(&self, sys: Option<&GuestMemory>, root: Target, va: u64) -> Target {
        self.walk_in(sys, root, va).0
    }

    /// Enumerate every present leaf mapping of the GMMU tree rooted at `root` (the guest
    /// root, in the reservation): each is `(va, target, size, kind)` where `size` is 2 MiB,
    /// 64 KiB or 4 KiB. Used to mirror the guest's set-A mappings into a host-managed
    /// shadow VA space by fixed-VA `RM_MAP_MEMORY_DMA` (D17 stage 1, `shadow.rs`). Capped
    /// at `max` leaves. Big/small precedence follows §2.3 (big PTE first, zero big -> small).
    pub fn enumerate_leaves(
        &self,
        sys: Option<&GuestMemory>,
        root: Target,
        max: usize,
    ) -> Vec<(u64, Target, u64, u8)> {
        if self.mmu_v3 {
            return crate::gmmu_v3::enumerate_leaves(&FbReader { fb: self, sys }, root, max);
        }
        let mut out = Vec::new();
        let leaf = |pte: u64, va: u64, size: u64| -> Option<(u64, Target, u64, u8)> {
            if pte & 1 == 0 {
                return None; // invalid or sparse: nothing physical to mirror
            }
            let kind = (pte >> 56) as u8;
            let t = match (pte >> 1) & 3 {
                0 => Target::Fb(((pte >> 8) & 0x1FF_FFFF) << 12),
                2 | 3 => Target::Sys(((pte >> 8) & 0x3FFF_FFFF_FFFF) << 12),
                _ => return None,
            };
            Some((va, t, size, kind))
        };
        // PD3 (VA 48:47) -> PD2 (46:38) -> PD1 (37:29) -> PD0 (28:21) -> PT. Each table
        // is read in one block (BAR1 reads are uncached bus round trips; per-entry reads
        // and a re-read of the big PTE for every small slot made this walk ~250 ms).
        let Some(pd3) = self.table_block(sys, root, 4) else {
            return out;
        };
        for (i3, e3) in pd3.iter().enumerate() {
            if out.len() >= max {
                break;
            }
            let Some(pd2t) = Self::next_table(*e3) else {
                continue;
            };
            let Some(pd2) = self.table_block(sys, pd2t, 512) else {
                continue;
            };
            for (i2, e2) in pd2.iter().enumerate() {
                if out.len() >= max {
                    break;
                }
                let Some(pd1t) = Self::next_table(*e2) else {
                    continue;
                };
                let Some(pd1) = self.table_block(sys, pd1t, 512) else {
                    continue;
                };
                for (i1, e1) in pd1.iter().enumerate() {
                    if out.len() >= max {
                        break;
                    }
                    let Some(pd0t) = Self::next_table(*e1) else {
                        continue;
                    };
                    let Some(pd0) = self.table_block(sys, pd0t, 512) else {
                        continue;
                    };
                    for i0 in 0..256usize {
                        if out.len() >= max {
                            break;
                        }
                        let base = ((i3 as u64) << 47)
                            | ((i2 as u64) << 38)
                            | ((i1 as u64) << 29)
                            | ((i0 as u64) << 21);
                        let (lo, hi) = (pd0[2 * i0], pd0[2 * i0 + 1]);
                        if lo & 1 == 1 {
                            // 2 MiB page (dual PDE low qword is a PTE).
                            if let Some(l) = leaf(lo, base, 0x20_0000) {
                                out.push(l);
                            }
                            continue;
                        }
                        // 64 KiB big-page table (VA 20:16, 32 entries).
                        let big = Self::big_table(lo).and_then(|t| self.table_block(sys, t, 32));
                        if let Some(bt) = &big {
                            for (b, pte) in bt.iter().enumerate() {
                                if out.len() >= max {
                                    break;
                                }
                                if let Some(l) = leaf(*pte, base | ((b as u64) << 16), 0x1_0000) {
                                    out.push(l);
                                }
                            }
                        }
                        // 4 KiB small-page table (VA 20:12, 512 entries); skip a 4 KiB slot
                        // already covered by a valid 64 KiB big PTE.
                        let Some(pt) =
                            Self::next_table(hi).and_then(|t| self.table_block(sys, t, 512))
                        else {
                            continue;
                        };
                        for (sidx, pte) in pt.iter().enumerate() {
                            if out.len() >= max {
                                break;
                            }
                            if big.as_ref().is_some_and(|bt| bt[sidx >> 4] & 1 == 1) {
                                continue;
                            }
                            if let Some(l) = leaf(*pte, base | ((sidx as u64) << 12), 0x1000) {
                                out.push(l);
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// `n` consecutive qwords of a page-table page (in the FB store, or in guest RAM).
    fn table_block(&self, sys: Option<&GuestMemory>, table: Target, n: usize) -> Option<Vec<u64>> {
        let mut b = vec![0u8; n * 8];
        let ok = match table {
            Target::Fb(pa) => self.read(pa, &mut b),
            Target::Sys(pa) => sys?.read(pa, &mut b),
            _ => false,
        };
        ok.then(|| {
            b.chunks_exact(8)
                .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
                .collect()
        })
    }

    /// `translate_in` plus the raw last-level entry the walk ended on (the PTE, or the
    /// 2 MiB PDE-as-PTE; 0 when a directory level was missing), for the mirror's
    /// read-only diagnostics of what the physical GMMU will see (exec-formats.md §2.2).
    pub fn walk_in(&self, sys: Option<&GuestMemory>, root: Target, va: u64) -> (Target, u64) {
        if self.mmu_v3 {
            return crate::gmmu_v3::walk(&FbReader { fb: self, sys }, root, va, &mut Vec::new());
        }
        let Some(e3) = self.table_u64(sys, root, 8 * ((va >> 47) & 3)) else {
            return (Target::Unmapped { level: 3 }, 0);
        };
        let Some(pd2) = Self::next_table(e3) else {
            return (Target::Unmapped { level: 3 }, e3);
        };
        let Some(e2) = self.table_u64(sys, pd2, 8 * ((va >> 38) & 0x1FF)) else {
            return (Target::Unmapped { level: 2 }, 0);
        };
        let Some(pd1) = Self::next_table(e2) else {
            return (Target::Unmapped { level: 2 }, e2);
        };
        let Some(e1) = self.table_u64(sys, pd1, 8 * ((va >> 29) & 0x1FF)) else {
            return (Target::Unmapped { level: 1 }, 0);
        };
        let Some(pd0) = Self::next_table(e1) else {
            return (Target::Unmapped { level: 1 }, e1);
        };
        let idx0 = (va >> 21) & 0xFF;
        let (Some(lo), Some(hi)) = (
            self.table_u64(sys, pd0, 16 * idx0),
            self.table_u64(sys, pd0, 16 * idx0 + 8),
        ) else {
            return (Target::Unmapped { level: 0 }, 0);
        };
        if lo & 1 == 1 {
            // A 2 MiB page: the low qword is a PTE.
            self.observe_pte(lo);
            return (Self::decode_pte(lo, va & 0x1F_FFFF), lo);
        }
        let small = Self::next_table(hi);
        let big = Self::big_table(lo);
        if let Some(pt) = small {
            if let Some(pte) = self.table_u64(sys, pt, 8 * ((va >> 12) & 0x1FF)) {
                let t = Self::decode_pte(pte, va & 0xFFF);
                if !matches!(t, Target::Unmapped { .. }) || big.is_none() {
                    self.observe_pte(pte);
                    return (t, pte);
                }
            }
        }
        if let Some(pt) = big {
            if let Some(pte) = self.table_u64(sys, pt, 8 * ((va >> 16) & 0x1F)) {
                self.observe_pte(pte);
                return (Self::decode_pte(pte, va & 0xFFFF), pte);
            }
        }
        (Target::Unmapped { level: 0 }, 0)
    }

    /// [`Self::translate`] through the BAR TLB: a hit costs no table read; a miss walks
    /// and records the table pages it read, so that a later write to one flushes the TLB.
    pub fn bar_translate(&self, root: u64, va: u64) -> Target {
        if !self.bar_tlb_on {
            return self.translate(root, va);
        }
        let key = (root, va >> 12);
        let off = va & 0xFFF;
        let (h, m, f) = self.bar_tlb_stats.get();
        if let Some(t) = self.bar_tlb.borrow().get(&key) {
            self.bar_tlb_stats.set((h + 1, m, f));
            return match *t {
                Target::Fb(pa) => Target::Fb(pa + off),
                Target::Sys(pa) => Target::Sys(pa + off),
                other => other,
            };
        }
        self.bar_tlb_stats.set((h, m + 1, f));
        let (t, pages) = self.walk_pages(root, va & !0xFFF);
        if matches!(t, Target::Fb(_) | Target::Sys(_)) {
            self.bar_tlb_pages.borrow_mut().extend(pages);
            self.bar_tlb.borrow_mut().insert(key, t);
        }
        match t {
            Target::Fb(pa) => Target::Fb(pa + off),
            Target::Sys(pa) => Target::Sys(pa + off),
            other => other,
        }
    }

    /// The walk of `translate` for a page-aligned `va`, and the FB table pages it read.
    fn walk_pages(&self, root: u64, va: u64) -> (Target, Vec<u64>) {
        if self.mmu_v3 {
            let mut pages = Vec::new();
            let r = FbReader {
                fb: self,
                sys: None,
            };
            let (t, _) = crate::gmmu_v3::walk(&r, Target::Fb(root), va, &mut pages);
            return (t, pages);
        }
        let mut pages = vec![root >> 12];
        let at = |pages: &mut Vec<u64>, t: Target| {
            if let Target::Fb(pa) = t {
                pages.push(pa >> 12);
            }
        };
        let Some(e3) = self.read_u64(root + 8 * ((va >> 47) & 3)) else {
            return (Target::Unmapped { level: 3 }, pages);
        };
        let Some(pd2) = Self::next_table(e3) else {
            return (Target::Unmapped { level: 3 }, pages);
        };
        at(&mut pages, pd2);
        let Some(e2) = self.table_u64(None, pd2, 8 * ((va >> 38) & 0x1FF)) else {
            return (Target::Unmapped { level: 2 }, pages);
        };
        let Some(pd1) = Self::next_table(e2) else {
            return (Target::Unmapped { level: 2 }, pages);
        };
        at(&mut pages, pd1);
        let Some(e1) = self.table_u64(None, pd1, 8 * ((va >> 29) & 0x1FF)) else {
            return (Target::Unmapped { level: 1 }, pages);
        };
        let Some(pd0) = Self::next_table(e1) else {
            return (Target::Unmapped { level: 1 }, pages);
        };
        at(&mut pages, pd0);
        let (t, _) = self.walk_in(None, Target::Fb(root), va);
        // The last level: the small and big tables the dual PDE names.
        let idx0 = (va >> 21) & 0xFF;
        if let (Some(lo), Some(hi)) = (
            self.table_u64(None, pd0, 16 * idx0),
            self.table_u64(None, pd0, 16 * idx0 + 8),
        ) {
            if lo & 1 == 0 {
                for tt in [Self::big_table(lo), Self::next_table(hi)].into_iter().flatten() {
                    if let Target::Fb(pa) = tt {
                        pages.push(pa >> 12);
                    }
                }
            }
        }
        (t, pages)
    }

    /// Perform a BAR access at `va`: read into or write from `data`.
    /// Returns the target for the trace.
    pub fn bar_access(
        &mut self,
        mem: &GuestMemory,
        root: u64,
        va: u64,
        data: &mut [u8],
        write: bool,
    ) -> Target {
        let t = self.bar_translate(root, va);
        match t {
            Target::Fb(pa) => {
                if write {
                    self.write(pa, data);
                } else if !self.read(pa, data) {
                    data.fill(0);
                }
            }
            Target::Sys(pa) => {
                if write {
                    mem.write(pa, data);
                } else if !mem.read(pa, data) {
                    data.fill(0);
                }
            }
            Target::Sparse | Target::Unmapped { .. } => {
                if !write {
                    data.fill(0);
                }
            }
        }
        t
    }
}

/// The FB store (and guest RAM for sysmem tables) as the incremental walker's reader.
struct FbReader<'a> {
    fb: &'a FakeFb,
    sys: Option<&'a GuestMemory>,
}

impl crate::gmmu_v3::Tables for FbReader<'_> {
    fn block(&self, table: Target, n: usize) -> Option<Vec<u64>> {
        self.fb.table_block(self.sys, table, n)
    }
}

impl crate::ptcache::TableReader for FbReader<'_> {
    fn block(&self, table: Target, n: usize) -> Option<Vec<u64>> {
        self.fb.table_block(self.sys, table, n)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn watched_dword_follows_writes() {
        let mut fb = super::FakeFb::new(1 << 20);
        fb.write(0x1000, &[1, 0, 0, 0, 2, 0, 0, 0]);
        // First read comes from the store, later ones from the copy.
        assert_eq!(fb.watched_dword(0x1004), Some(2));
        fb.write(0x1004, &7u32.to_le_bytes());
        assert_eq!(fb.watched_dword(0x1004), Some(7));
        // A write elsewhere on the page leaves it alone; a partial overwrite drops the
        // copy and the next read goes to the store again.
        fb.write(0x1000, &9u32.to_le_bytes());
        assert_eq!(fb.watched_dword(0x1004), Some(7));
        fb.write(0x1006, &[0xAA, 0xBB]);
        assert_eq!(fb.watched_dword(0x1004), Some(0xBBAA_0007));
        assert_eq!(fb.watched_dword(1 << 20), None);
    }

    use super::*;

    #[test]
    fn walk_small_and_big_pages() {
        let mut fb = FakeFb::new(64 << 20);
        let root = 0x10000;
        let pd2 = 0x11000;
        let pd1 = 0x12000;
        let pd0 = 0x13000;
        let pt_small = 0x14000;
        let pt_big = 0x15000;
        let vid = |addr: u64| ((addr >> 12) << 8) | 0b010;
        fb.write_u64(root, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        // Dual PDE 0: big PT in the low qword (address << 8), small PT in the high qword.
        fb.write_u64(pd0, ((pt_big >> 8) << 4) | 0b010);
        fb.write_u64(pd0 + 8, vid(pt_small));
        // Small PTE for VA 0x3000 -> FB 0x20000 (valid, video).
        fb.write_u64(pt_small + 8 * 3, ((0x20000u64 >> 12) << 8) | 1);
        // Sparse PTE for VA 0x4000.
        fb.write_u64(pt_small + 8 * 4, 0x8);
        // Big PTE for VA 0x10000..0x1FFFF -> sysmem 0x8000_0000 (aperture 2).
        fb.write_u64(pt_big + 8, ((0x8000_0000u64 >> 12) << 8) | 0b101);
        assert_eq!(fb.translate(root, 0x3010), Target::Fb(0x20010));
        assert_eq!(fb.translate(root, 0x4000), Target::Sparse);
        assert_eq!(fb.translate(root, 0x5000), Target::Unmapped { level: 0 });
        assert_eq!(fb.translate(root, 0x10ABC), Target::Sys(0x8000_0ABC));
        assert_eq!(fb.translate(root, 0x20_0000), Target::Unmapped { level: 0 });
        assert_eq!(fb.translate(root, 1 << 47), Target::Unmapped { level: 3 });

        let mem = GuestMemory::new();
        let mut d = 0xABCD_ABCDu32.to_le_bytes();
        assert_eq!(
            fb.bar_access(&mem, root, 0x3010, &mut d, true),
            Target::Fb(0x20010)
        );
        let mut r = [0u8; 4];
        fb.read(0x20010, &mut r);
        assert_eq!(r, d);
        let mut z = [0xFFu8; 4];
        fb.bar_access(&mem, root, 0x4000, &mut z, false);
        assert_eq!(z, [0; 4]);
        assert!(!fb.read(64 << 20, &mut z));
        assert_eq!(fb.pages_touched(), 7);
        // The BAR TLB: a hit needs no walk; a write to a table page on the path flushes it.
        let (_, walks0, _) = fb.bar_tlb_stats.get();
        assert_eq!(fb.bar_translate(root, 0x3020), Target::Fb(0x20020));
        assert_eq!(fb.bar_tlb_stats.get().1, walks0);
        fb.write_u64(pt_small + 8 * 3, ((0x28000u64 >> 12) << 8) | 1);
        assert_eq!(fb.bar_translate(root, 0x3020), Target::Fb(0x28020));
        assert_eq!(fb.bar_tlb_stats.get().1, walks0 + 1);
    }

    #[test]
    fn enumerate_leaves_sees_every_page_size_once() {
        let mut fb = FakeFb::new(64 << 20);
        let (root, pd2, pd1, pd0) = (0x10000u64, 0x11000u64, 0x12000u64, 0x13000u64);
        let (pt_small, pt_big) = (0x14000u64, 0x15000u64);
        let vid = |addr: u64| ((addr >> 12) << 8) | 0b010;
        fb.write_u64(root, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        fb.write_u64(pd0, ((pt_big >> 8) << 4) | 0b010);
        fb.write_u64(pd0 + 8, vid(pt_small));
        // Small PTEs at VA 0x3000 and at 0x11000 (the latter under a valid big PTE: hidden).
        fb.write_u64(pt_small + 8 * 3, ((0x20000u64 >> 12) << 8) | 1);
        fb.write_u64(pt_small + 8 * 0x11, ((0x30000u64 >> 12) << 8) | 1);
        fb.write_u64(pt_big + 8, ((0x8000_0000u64 >> 12) << 8) | 0b101);
        // A 2 MiB PTE in PD0 entry 1 (VA 0x20_0000), kind 0x06.
        fb.write_u64(pd0 + 16, ((0x40_0000u64 >> 12) << 8) | (0x06u64 << 56) | 1);
        let mem = GuestMemory::new();
        let l = fb.enumerate_leaves(Some(&mem), Target::Fb(root), 1 << 20);
        assert_eq!(
            l,
            vec![
                (0x1_0000, Target::Sys(0x8000_0000), 0x1_0000, 0),
                (0x3000, Target::Fb(0x20000), 0x1000, 0),
                (0x20_0000, Target::Fb(0x40_0000), 0x20_0000, 0x06),
            ]
        );
        // The cached walk: the same leaves, clean until a table page it read is written.
        assert!(fb.tables_dirty(Target::Fb(root)));
        let u = fb.walk_update(Some(&mem), Target::Fb(root), false, 1 << 20);
        assert!(u.full);
        let mut cached = fb.with_walk(Target::Fb(root), |c| c.all_leaves()).unwrap();
        cached.sort_by_key(|l| l.0);
        let mut walked = l.clone();
        walked.sort_by_key(|l| l.0);
        assert_eq!(cached, walked);
        assert!(!fb.tables_dirty(Target::Fb(root)));
        fb.write_u64(0x20000, 1); // data page, not a table
        assert!(!fb.tables_dirty(Target::Fb(root)));
        fb.write_u64(pt_small + 8 * 7, ((0x21000u64 >> 12) << 8) | 1);
        assert!(fb.tables_dirty(Target::Fb(root)));
        let u = fb.walk_update(Some(&mem), Target::Fb(root), false, 1 << 20);
        assert!(!u.full);
        assert_eq!(u.changed, vec![0]);
        assert!(!fb.tables_dirty(Target::Fb(root)));
        assert_eq!(fb.with_walk(Target::Fb(root), |c| c.leaf_count()), Some(4));
    }

    #[test]
    fn reserved_layout_and_outside_accounting() {
        // The 4 GiB profile on the RTX 2070 host (docs/caraxes/memory-model.md §1).
        let l = FbLayout::reserved(4 << 30, 8 << 30, 0xE900_0000, 0x1_E900_0000).unwrap();
        assert_eq!(l.bar_roots(), Some((0x1_E8E0_0000, 0x1_E8E0_1000)));
        assert_eq!(l.usable(), Some((0xE900_0000, 0x1_E8E0_0000)));
        assert!(l.inside(0xE900_0000, 8));
        assert!(!l.inside(0x1_E900_0000, 1));
        assert!(!l.inside(0xE8FF_FFFC, 8));
        assert!(FbLayout::reserved(4 << 30, 8 << 30, 0xE910_0000, 0x1_E900_0000).is_err());
        assert!(FbLayout::reserved(4 << 30, 4 << 30, 0xE900_0000, 0x1_E900_0000).is_err());
        let mut fb = FakeFb::for_layout(&l);
        assert!(fb.write_u64(0x1_E8E0_0000, 1));
        assert_eq!(fb.outside.writes.get(), 0);
        // The fake WPR above the reservation: still backed, but counted.
        assert!(fb.write_u64(0x1_FFF0_0000, 2));
        assert_eq!(fb.read_u64(0x1_FFF0_0000), Some(2));
        assert_eq!(fb.outside.writes.get(), 1);
        assert_eq!(fb.outside.reads.get(), 1);
        assert_eq!(FbLayout::plain(4 << 30).bar_roots(), None);
    }

    /// A window source over host RAM that refuses windows above `max_len` and counts
    /// the mappings, standing in for the BAR1 windows of the reservation.
    struct RamWindows {
        max_len: usize,
        pages: std::sync::Arc<std::sync::Mutex<HostRamStore>>,
        maps: std::sync::Arc<std::sync::atomic::AtomicU64>,
    }

    struct RamWindow {
        offset: u64,
        len: usize,
        pages: std::sync::Arc<std::sync::Mutex<HostRamStore>>,
    }

    impl WindowBacking for RamWindow {
        fn offset(&self) -> u64 {
            self.offset
        }
        fn len(&self) -> usize {
            self.len
        }
        fn read(&self, off: u64, buf: &mut [u8]) {
            assert!(off >= self.offset && off + buf.len() as u64 <= self.offset + self.len as u64);
            self.pages.lock().unwrap().read(off, buf)
        }
        fn write(&self, off: u64, data: &[u8]) {
            assert!(off >= self.offset && off + data.len() as u64 <= self.offset + self.len as u64);
            self.pages.lock().unwrap().write(off, data)
        }
    }

    impl WindowSource for RamWindows {
        fn map(&mut self, offset: u64, len: usize) -> Result<Box<dyn WindowBacking>, u32> {
            if len > self.max_len {
                return Err(0x51);
            }
            self.maps.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(Box::new(RamWindow {
                offset,
                len,
                pages: self.pages.clone(),
            }))
        }
    }

    #[test]
    fn vram_store_windows_are_lru_and_size_adapts() {
        let (base, end) = (0xE900_0000u64, 0x1_E900_0000u64);
        let pages = std::sync::Arc::new(std::sync::Mutex::new(HostRamStore::default()));
        let maps = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        // The host grants at most 16 MiB per window: 64 and 32 are refused first.
        let src = RamWindows {
            max_len: 16 << 20,
            pages: pages.clone(),
            maps: maps.clone(),
        };
        let mut s = VramStore::with_budget(Box::new(src), base, end, 64 << 20, 3);
        s.write(base + 0x300000, &0xA5A5_1234u32.to_le_bytes());
        let mut b = [0u8; 4];
        s.read(base + 0x300000, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0xA5A5_1234);
        assert_eq!(s.stats().map_failures, 2);
        assert_eq!(s.windows_held(), vec![(0, 16 << 20)]);
        // A write straddling two windows.
        let straddle = base + (16 << 20) - 2;
        s.write(straddle, &[1, 2, 3, 4]);
        s.read(straddle, &mut b);
        assert_eq!(b, [1, 2, 3, 4]);
        assert_eq!(s.windows_held().len(), 2);
        // Three more windows: the least recently used (window 0) goes first.
        for k in 2..5u64 {
            s.write(base + k * (16 << 20), &[k as u8]);
        }
        assert_eq!(s.windows_held().len(), 3);
        assert_eq!(s.stats().evictions, 2);
        assert!(!s.windows_held().iter().any(|(o, _)| *o == 0));
        // Window 0 comes back on demand with its data intact (it is the same VRAM).
        s.read(base + 0x300000, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0xA5A5_1234);
        // Outside the reservation: host RAM, no window.
        s.write(0x1_FFF0_0000, &[9, 9]);
        s.read(0x1_FFF0_0000, &mut b[..2]);
        assert_eq!(&b[..2], &[9, 9]);
        assert_eq!(s.pages_touched(), 1);
        // The last byte of the reservation, and a range that crosses its end.
        s.write(end - 4, &[7, 7, 7, 7, 8, 8]);
        s.read(end - 4, &mut [0u8; 6][..]);
        let mut six = [0u8; 6];
        s.read(end - 4, &mut six);
        assert_eq!(six, [7, 7, 7, 7, 8, 8]);
        let st = s.stats();
        assert!(st.maps >= 5 && st.writes == 7 && st.reads == 6, "{st:?}");

        // Through FakeFb, with the filter hook seeing every write.
        struct Count(std::sync::Arc<std::sync::atomic::AtomicU64>);
        impl WriteFilter for Count {
            fn before_write(&mut self, _pa: u64, data: &[u8]) {
                self.0
                    .fetch_add(data.len() as u64, std::sync::atomic::Ordering::Relaxed);
            }
        }
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let l = FbLayout::reserved(4 << 30, 8 << 30, base, end).unwrap();
        let mut fb = FakeFb::for_layout(&l).with_store(Box::new(s));
        fb.set_write_filter(Box::new(Count(seen.clone())));
        assert!(fb.write_u64(base + 0x1000, 0x1122_3344_5566_7788));
        assert_eq!(fb.read_u64(base + 0x1000), Some(0x1122_3344_5566_7788));
        assert_eq!(seen.load(std::sync::atomic::Ordering::Relaxed), 8);
        assert!(fb.vram_stats().is_some());
    }

    /// A host BAR1 with room for `room` windows at a time: a map beyond that is refused
    /// until a window is dropped (the shared-BAR1 case of P20).
    struct TightBar1 {
        live: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        room: usize,
        pages: std::sync::Arc<std::sync::Mutex<HostRamStore>>,
    }

    struct TightWindow {
        inner: RamWindow,
        live: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Drop for TightWindow {
        fn drop(&mut self) {
            self.live.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl WindowBacking for TightWindow {
        fn offset(&self) -> u64 {
            self.inner.offset()
        }
        fn len(&self) -> usize {
            self.inner.len()
        }
        fn read(&self, off: u64, buf: &mut [u8]) {
            self.inner.read(off, buf)
        }
        fn write(&self, off: u64, data: &[u8]) {
            self.inner.write(off, data)
        }
    }

    impl WindowSource for TightBar1 {
        fn map(&mut self, offset: u64, len: usize) -> Result<Box<dyn WindowBacking>, u32> {
            use std::sync::atomic::Ordering::SeqCst;
            if self.live.load(SeqCst) >= self.room {
                return Err(0x51);
            }
            self.live.fetch_add(1, SeqCst);
            Ok(Box::new(TightWindow {
                inner: RamWindow {
                    offset,
                    len,
                    pages: self.pages.clone(),
                },
                live: self.live.clone(),
            }))
        }
    }

    #[test]
    fn a_refused_window_evicts_our_own_and_never_drops_a_write() {
        use std::sync::atomic::Ordering::SeqCst;
        let (base, end) = (0xE900_0000u64, 0x1_6900_0000u64);
        let live = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let pages = std::sync::Arc::new(std::sync::Mutex::new(HostRamStore::default()));
        // Budget 3 windows, but the host has room for only 1 (the others' windows).
        let src = TightBar1 {
            live: live.clone(),
            room: 1,
            pages: pages.clone(),
        };
        let mut s = VramStore::with_budget(Box::new(src), base, end, 32 << 20, 3);
        for k in 0..6u64 {
            s.write(base + k * (32 << 20), &[k as u8 + 1]);
        }
        let mut b = [0u8; 1];
        for k in 0..6u64 {
            s.read(base + k * (32 << 20), &mut b);
            assert_eq!(b[0], k as u8 + 1, "window {k}");
        }
        // The window size never shrank and the store never held more than the host allows.
        assert_eq!(s.windows_held().len(), 1);
        assert_eq!(s.windows_held()[0].1, 32 << 20);
        assert!(live.load(SeqCst) <= 1);
        assert_eq!(s.stats().stalls, 0);
        // No room at all: the access stalls (bounded here by the test limit) and is
        // reported, instead of silently succeeding.
        let src = TightBar1 {
            live: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            room: 0,
            pages,
        };
        let mut s = VramStore::with_budget(Box::new(src), base, end, 2 << 20, 2);
        s.stall_limit.set(3);
        s.write(base, &[1]);
        assert_eq!(s.stats().stalls, 3);
    }
}

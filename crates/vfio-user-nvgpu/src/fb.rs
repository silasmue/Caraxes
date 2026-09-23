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

impl WindowSource for std::sync::Arc<std::sync::Mutex<crate::hostmem::HostMemory>> {
    fn map(&mut self, offset: u64, len: usize) -> Result<Box<dyn WindowBacking>, u32> {
        let w = self.lock().unwrap().map_reservation(offset, len)?;
        Ok(Box::new(w))
    }
}

/// Counters of the VRAM store, for the exit summary.
#[derive(Default, Debug, Clone)]
pub struct VramStats {
    pub maps: u64,
    pub map_failures: u64,
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
    outside: RefCell<HostRamStore>,
    stats: RefCell<VramStats>,
}

/// The default window: 64 MiB, the largest RM granted in host-exec.md §1 (stage A).
pub const VRAM_WINDOW: usize = 64 << 20;
/// Windows held at once (3 x 64 MiB of the 256 MiB host BAR1, ~232 MiB free, H2).
pub const VRAM_WINDOWS: usize = 3;

impl VramStore {
    pub fn new(src: Box<dyn WindowSource>, base: u64, end: u64) -> Self {
        VramStore {
            src: RefCell::new(src),
            base,
            end,
            windows: RefCell::new(Vec::new()),
            tick: Cell::new(0),
            max_windows: VRAM_WINDOWS,
            window_len: Cell::new(VRAM_WINDOW),
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
                    if wlen <= 2 << 20 {
                        return None;
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
        // PD3 (VA 48:47) -> PD2 (46:38) -> PD1 (37:29) -> PD0 (28:21) -> PT.
        for i3 in 0..4u64 {
            if out.len() >= max {
                break;
            }
            let Some(e3) = self.table_u64(sys, root, 8 * i3) else {
                continue;
            };
            let Some(pd2) = Self::next_table(e3) else {
                continue;
            };
            for i2 in 0..512u64 {
                if out.len() >= max {
                    break;
                }
                let Some(e2) = self.table_u64(sys, pd2, 8 * i2) else {
                    continue;
                };
                let Some(pd1) = Self::next_table(e2) else {
                    continue;
                };
                for i1 in 0..512u64 {
                    if out.len() >= max {
                        break;
                    }
                    let Some(e1) = self.table_u64(sys, pd1, 8 * i1) else {
                        continue;
                    };
                    let Some(pd0) = Self::next_table(e1) else {
                        continue;
                    };
                    for i0 in 0..256u64 {
                        if out.len() >= max {
                            break;
                        }
                        let base = (i3 << 47) | (i2 << 38) | (i1 << 29) | (i0 << 21);
                        let (Some(lo), Some(hi)) = (
                            self.table_u64(sys, pd0, 16 * i0),
                            self.table_u64(sys, pd0, 16 * i0 + 8),
                        ) else {
                            continue;
                        };
                        if lo & 1 == 1 {
                            // 2 MiB page (dual PDE low qword is a PTE).
                            if let Some(l) = leaf(lo, base, 0x20_0000) {
                                out.push(l);
                            }
                            continue;
                        }
                        let big = Self::big_table(lo);
                        let small = Self::next_table(hi);
                        // 64 KiB big-page table (VA 20:16, 32 entries).
                        if let Some(pt) = big {
                            for b in 0..32u64 {
                                if out.len() >= max {
                                    break;
                                }
                                if let Some(pte) = self.table_u64(sys, pt, 8 * b) {
                                    if let Some(l) = leaf(pte, base | (b << 16), 0x1_0000) {
                                        out.push(l);
                                    }
                                }
                            }
                        }
                        // 4 KiB small-page table (VA 20:12, 512 entries); skip a 4 KiB slot
                        // already covered by a valid 64 KiB big PTE.
                        if let Some(pt) = small {
                            for sidx in 0..512u64 {
                                if out.len() >= max {
                                    break;
                                }
                                let covered = big
                                    .and_then(|bt| self.table_u64(sys, bt, 8 * (sidx >> 4)))
                                    .is_some_and(|bpte| bpte & 1 == 1);
                                if covered {
                                    continue;
                                }
                                if let Some(pte) = self.table_u64(sys, pt, 8 * sidx) {
                                    if let Some(l) = leaf(pte, base | (sidx << 12), 0x1000) {
                                        out.push(l);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// `translate_in` plus the raw last-level entry the walk ended on (the PTE, or the
    /// 2 MiB PDE-as-PTE; 0 when a directory level was missing), for the mirror's
    /// read-only diagnostics of what the physical GMMU will see (exec-formats.md §2.2).
    pub fn walk_in(&self, sys: Option<&GuestMemory>, root: Target, va: u64) -> (Target, u64) {
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
        let t = self.translate(root, va);
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

#[cfg(test)]
mod tests {
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
        let mut s = VramStore::new(Box::new(src), base, end);
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
        assert_eq!(s.windows_held().len(), VRAM_WINDOWS);
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
}

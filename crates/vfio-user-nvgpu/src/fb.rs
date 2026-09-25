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
use std::collections::{HashMap, HashSet};

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
    /// P21 design H: bytes of the reservation below the carve-out held back as the BAR1
    /// pool (docs/caraxes/bar1-pool.md); 0 without it. Taken out of the usable FB.
    pub pool: u64,
}

impl FbLayout {
    /// Milestone 1 to E5a: FB `[0, profile)`, the fake GSP's WPR at its top.
    pub fn plain(profile: u64) -> Self {
        FbLayout {
            addr_space: profile,
            profile,
            reservation: None,
            pool: 0,
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
            pool: 0,
        })
    }

    /// The same layout with a BAR1 pool of `len` bytes (a multiple of 2 MiB) carved from
    /// the top of the usable FB, right below the carve-out: the guest's region table then
    /// ends its usable region below the pool and reports the pool as reserved
    /// (docs/caraxes/bar1-pool.md).
    pub fn with_pool(self, len: u64) -> Result<Self, String> {
        let (lo, hi) = self
            .usable()
            .ok_or("the BAR1 pool needs a VRAM reservation (--reserve-vram)")?;
        if len == 0 || len % (2 * MIB) != 0 || len.saturating_add(2 * MIB) > hi - lo {
            return Err(format!(
                "BAR1 pool of {len:#x} bytes does not fit the usable FB [{lo:#x}, {hi:#x}) in 2 MiB units"
            ));
        }
        Ok(FbLayout {
            pool: self.pool + len,
            ..self
        })
    }

    /// The BAR1 pool `[start, end)`, right below the carve-out, when there is one.
    pub fn pool_range(&self) -> Option<(u64, u64)> {
        match self.reservation {
            Some((_, end)) if self.pool > 0 => Some((end - CARVE_OUT - self.pool, end - CARVE_OUT)),
            _ => None,
        }
    }

    /// `(bar2PdeBase, bar1PdeBase)` in the carve-out, with a reservation.
    pub fn bar_roots(&self) -> Option<(u64, u64)> {
        self.reservation
            .map(|(_, end)| (end - CARVE_OUT, end - CARVE_OUT + PAGE))
    }

    /// The usable FB `[base, end - CARVE_OUT - pool)` the region table gives the guest's
    /// heap, with a reservation.
    pub fn usable(&self) -> Option<(u64, u64)> {
        self.reservation
            .map(|(base, end)| (base, end - CARVE_OUT - self.pool))
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

/// P21 switch (docs/caraxes/bar1-passthrough.md §6): `CARAXES_BAR1_PASSTHROUGH=1` maps
/// guest BAR1 `[0, len)` straight onto the reservation piece at `base + fb_offset`.
/// `CARAXES_BAR1_PT_MIB` (a power of two, 1..=64, default 1) sets `len`,
/// `CARAXES_BAR1_PT_FB_OFFSET` (hex or decimal, 64 KiB aligned, default 0x980000, the
/// layout every traced run showed) the offset from the reservation base.
/// Returns `(fb_offset, len)` when on.
pub fn bar1_passthrough_config() -> Option<(u64, u64)> {
    if std::env::var("CARAXES_BAR1_PASSTHROUGH").as_deref() != Ok("1") {
        return None;
    }
    // Prototype G assumes the traced layout (guest BAR1 offset X at reservation base +
    // 0x980000 + X). On 2026-09-24, on the 615 host, the guest laid its BAR1 out 2 MiB away
    // and 128 pages of the area reached the wrong page of the VM's own reservation; the check
    // can only log that after the fact, because the area is mapped once by the client. The
    // fixed layout is therefore refused unless the operator asks for it by name; the pool
    // (`CARAXES_BAR1_PASSTHROUGH=pool`, bar1pool.rs) remaps any layout and fails closed.
    if std::env::var("CARAXES_BAR1_PASSTHROUGH_UNSAFE_LAYOUT").as_deref() != Ok("1") {
        eprintln!(
            "vfio-user-nvgpu: ERROR: CARAXES_BAR1_PASSTHROUGH=1 (the fixed-layout prototype) is refused: it mismatched 128 pages on 2026-09-24; use CARAXES_BAR1_PASSTHROUGH=pool, or set CARAXES_BAR1_PASSTHROUGH_UNSAFE_LAYOUT=1 for an experiment; starting without the area"
        );
        return None;
    }
    let len = std::env::var("CARAXES_BAR1_PT_MIB")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|m| m.is_power_of_two() && (1..=64).contains(m))
        .map_or(MIB, |m| m * MIB);
    let off = std::env::var("CARAXES_BAR1_PT_FB_OFFSET")
        .ok()
        .and_then(|v| parse_u64(&v))
        .unwrap_or(BAR1_PT_FB_OFFSET);
    Some((off, len))
}

/// Where libcuda's channel buffers (GPFIFO and USERD, the only guest BAR1 traffic in
/// every trace) sat relative to the reservation base in every traced run: guest BAR1
/// offset X maps FB `base + 0x980000 + X` (docs/caraxes/bar1-passthrough.md §2).
pub const BAR1_PT_FB_OFFSET: u64 = 0x98_0000;

fn parse_u64(v: &str) -> Option<u64> {
    let v = v.trim();
    match v.strip_prefix("0x").or_else(|| v.strip_prefix("0X")) {
        Some(h) => u64::from_str_radix(h, 16).ok(),
        None => v.parse().ok(),
    }
}

/// The FB range `[fb0, fb0 + len)` the passthrough area maps, checked: inside the
/// usable part of the reservation (never the device-owned carve-out with the BAR root
/// page directories), 64 KiB aligned, and no larger than the guest's BAR1 (`bar1`).
pub fn bar1_passthrough_area(
    layout: &FbLayout,
    fb_offset: u64,
    len: u64,
    bar1: u64,
) -> Result<(u64, u64), String> {
    let (lo, hi) = layout
        .usable()
        .ok_or("BAR1 passthrough needs a VRAM reservation (--reserve-vram)")?;
    if len == 0 || len > bar1 || len % (64 << 10) != 0 || fb_offset % (64 << 10) != 0 {
        return Err(format!(
            "BAR1 passthrough: area {len:#x} at reservation offset {fb_offset:#x} must be 64 KiB aligned and at most the BAR1 size {bar1:#x}"
        ));
    }
    let fb0 = lo
        .checked_add(fb_offset)
        .filter(|&a| a.checked_add(len).is_some_and(|e| e <= hi))
        .ok_or_else(|| {
            format!(
                "BAR1 passthrough: [{:#x}, +{len:#x}) is not inside the usable reservation [{lo:#x}, {hi:#x})",
                lo.wrapping_add(fb_offset)
            )
        })?;
    Ok((fb0, len))
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

/// Counters of the BAR1 passthrough check (P21, docs/caraxes/bar1-passthrough.md §6).
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Bar1PtStats {
    /// Checks run (each one walks the guest's BAR1 tables over the passthrough area).
    pub checks: u64,
    /// Pages the guest maps inside the area, at the last check.
    pub mapped: u64,
    /// Of those, pages whose guest translation is the area's own (`fb0 + offset`).
    pub matched: u64,
    /// Of those, pages the guest maps elsewhere (a guest CPU access reaches the wrong
    /// page of its own reservation), at the last check.
    pub mismatched: u64,
    /// Distinct mismatched pages seen over the device's life.
    pub mismatch_events: u64,
    /// Distinct pages reachable through the area that are guest page-table pages the
    /// shadow watches (a write there bypasses [`FakeFb::write`]).
    pub table_hazards: u64,
    /// The first few findings, for the log.
    pub first: Vec<String>,
}

/// The state of the P21 BAR1 passthrough: the guest BAR1 range `[0, len)` is mapped by
/// the VMM straight onto the reservation piece `[fb0, fb0 + len)` (a dma-buf of it,
/// hostmem.rs `export_bar1_passthrough`), so guest accesses there never reach the
/// device. That is correct only while every page the guest maps in the range points at
/// `fb0 + offset`, and safe for the shadow only while no page reachable through it is a
/// watched page-table page. Both are checked whenever the guest changes the BAR1 tables
/// over the range or the shadow's set of table pages changes.
struct Bar1Pt {
    root: u64,
    fb0: u64,
    len: u64,
    /// The BAR1 table pages the last area walk read (a write to one re-arms the walk).
    table_pages: HashSet<u64>,
    /// `(guest BAR1 offset, target)` of every mapped 4 KiB page of the area.
    mapped: Vec<(u64, Target)>,
    reported: HashSet<u64>,
    hazards: HashSet<u64>,
    walk_dirty: bool,
    hazard_dirty: bool,
    stats: Bar1PtStats,
}

/// Where the bytes of an FB page live right now ([`FakeFb::backing`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backing {
    /// The FB store: the reserved VRAM (or host RAM without a reservation).
    Store,
    /// The BAR2 memfd at this offset: the page is mapped through guest BAR2 while
    /// `CARAXES_BAR2_MEMFD=1` (docs/caraxes/bar2-memfd.md), so the guest's current
    /// content is there; VRAM catches up at the next sync.
    Bar2Memfd(u64),
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
    /// P21: the BAR1 passthrough area and its check, when on.
    bar1_pt: RefCell<Option<Bar1Pt>>,
    /// P21 design H: the BAR1 pool and its remap table, when on (bar1pool.rs). Every FB
    /// access of this model goes through [`Self::remap`].
    pool: RefCell<Option<crate::bar1pool::Bar1Pool>>,
    /// `CARAXES_BAR2_MEMFD=1`: guest BAR2 as a memfd (bar2area.rs); the FB pages mapped
    /// through BAR2 live there ([`Self::backing`]).
    bar2: Option<Box<crate::bar2area::Bar2Area>>,
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
            bar1_pt: RefCell::new(None),
            pool: RefCell::new(None),
            bar2: None,
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
        if self.is_remapped(pa) || matches!(self.backing(pa), Backing::Bar2Memfd(_)) {
            // A BAR1 pool page or a page behind the BAR2 memfd: the guest writes it without
            // the device seeing it (`note_write` is bypassed), so a copy would go stale.
            // Read the store (or the memfd, which is RAM) every time.
            let mut b = [0u8; 4];
            return self.read(pa, &mut b).then(|| u32::from_le_bytes(b));
        }
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
        let before = c.table_pages();
        let u = c.update(&reader, full);
        if c.table_pages() != before || u.full {
            if let Some(pt) = self.bar1_pt.borrow_mut().as_mut() {
                pt.hazard_dirty = true;
            }
            if let Some(p) = self.pool.borrow_mut().as_mut() {
                p.hazard_dirty = true;
            }
        }
        u
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
        if let Some(pt) = self.bar1_pt.borrow_mut().as_mut() {
            if !pt.walk_dirty && (pa >> 12..=last).any(|p| pt.table_pages.contains(&p)) {
                pt.walk_dirty = true;
            }
        }
        if let Some(p) = self.pool.borrow_mut().as_mut() {
            p.note_table_write(pa >> 12, last);
        }
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
        if let Some(a) = self.bar2.as_deref() {
            a.note_table_write(pa, data);
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

    /// Where FB address `pa` lives right now: the one translation every FB access of the
    /// device goes through ([`Self::read`], [`Self::write`]). Today the only other home
    /// is the BAR2 memfd (`CARAXES_BAR2_MEMFD=1`, bar2area.rs).
    pub fn backing(&self, pa: u64) -> Backing {
        match self.bar2.as_deref().and_then(|a| a.backing(pa)) {
            Some(off) => Backing::Bar2Memfd(off),
            None => Backing::Store,
        }
    }

    /// FB pages (4 KiB page numbers) whose content changed behind the device: guest
    /// writes through the BAR2 memfd that a sync found. Everything a trapped write of the
    /// same page would have told `note_write`, but of unknown content: the shadow's walk
    /// caches re-read the page (from the memfd, RAM speed), the BAR TLB and the watched
    /// dwords drop what they held for it, the P21 check walks again.
    pub fn note_pages_changed(&self, pages: &[u64]) {
        if pages.is_empty() {
            return;
        }
        if let Some(pt) = self.bar1_pt.borrow_mut().as_mut() {
            if pages.iter().any(|p| pt.table_pages.contains(p)) {
                pt.walk_dirty = true;
            }
        }
        // The BAR1 pool follows the guest's BAR1 tables over its area as `note_write` tells
        // it; with the memfd the guest RM writes those tables through BAR2, so the sync is
        // the only one that sees them. Without this the pool never moved a page and the
        // first cuCtxCreate hung (optimization-backlog.md §3.1, bar2-memfd.md §8).
        if let Some(p) = self.pool.borrow_mut().as_mut() {
            for &page in pages {
                p.note_table_write(page, page);
            }
        }
        {
            let wp = self.watched_pages.borrow();
            if pages.iter().any(|p| wp.contains(p)) {
                self.watched
                    .borrow_mut()
                    .retain(|k, _| !pages.contains(&(k >> 12)));
            }
        }
        {
            let mut tp = self.bar_tlb_pages.borrow_mut();
            if pages.iter().any(|p| tp.contains(p)) {
                tp.clear();
                self.bar_tlb.borrow_mut().clear();
                let (h, m, f) = self.bar_tlb_stats.get();
                self.bar_tlb_stats.set((h, m, f + 1));
            }
        }
        for c in self.walks.borrow_mut().values_mut() {
            for &p in pages {
                if c.watches(p) {
                    c.mark_dirty(p);
                }
            }
        }
    }

    /// Put guest BAR2 in the memfd `area` (docs/caraxes/bar2-memfd.md). Refused for the
    /// GMMU v3 format, whose tables the BAR2 walk (ptcache.rs, v2 geometry) cannot read.
    pub fn set_bar2_area(&mut self, area: crate::bar2area::Bar2Area) -> Result<(), String> {
        if self.mmu_v3 {
            return Err("the BAR2 memfd follows GMMU v2 tables only (this chip uses v3)".into());
        }
        self.bar2 = Some(Box::new(area));
        Ok(())
    }

    /// The BAR2 memfd, when on.
    pub fn bar2_area(&self) -> Option<&crate::bar2area::Bar2Area> {
        self.bar2.as_deref()
    }

    /// A sync of the BAR2 memfd (bar2area.rs `Bar2Area::sync`): guest writes found by the
    /// dirty scan are written back to VRAM and reported to the caches, then the BAR2
    /// tables under `root` (the BAR2 page directory) are followed. Returns false when the
    /// memfd is off.
    pub fn bar2_sync(&mut self, root: Option<u64>, sys: Option<&GuestMemory>) -> bool {
        let size = self.size;
        let Some(a) = self.bar2.as_deref_mut() else {
            return false;
        };
        // Write-backs and loads go where the page lives now: its pool page when the BAR1
        // pool holds it (bar2-memfd.md §8).
        let pool = self.pool.borrow();
        let table = pool.as_ref().map(|p| &p.table).filter(|t| !t.is_empty());
        let tr = |pa: u64| table.map_or(pa, |t| t.translate(pa));
        let r = a.sync(&mut *self.store, size, root, sys, &tr);
        drop(pool);
        if !r.dirty.is_empty() {
            // The written-back bytes must reach VRAM before the GPU can be told to read
            // them (the RPC and invalidate paths that follow a sync).
            self.store.flush();
        }
        self.note_pages_changed(&r.dirty);
        true
    }

    /// A read of the trapped flush page: with `CARAXES_BAR2_FLUSH_DEFER_US` the sync is
    /// owed (bar2area.rs [`crate::bar2area::Owed`]), else it runs now. Returns false when
    /// the memfd is off.
    pub fn bar2_flush(&mut self, root: Option<u64>, sys: Option<&GuestMemory>) -> bool {
        let Some(a) = self.bar2.as_deref() else {
            return false;
        };
        match &a.owed {
            Some(o) => {
                o.owe(crate::bar2area::mono_ns());
                true
            }
            None => self.bar2_sync(root, sys),
        }
    }

    /// Pay a sync the flush reads owe, before something may consume BAR2 data (`by`
    /// says who). Returns whether a sync ran.
    pub fn bar2_pay(
        &mut self,
        root: Option<u64>,
        sys: Option<&GuestMemory>,
        by: crate::bar2area::Payer,
    ) -> bool {
        let Some(a) = self.bar2.as_deref() else {
            return false;
        };
        // Standing mode (the flush page as memory) always owes: every payer syncs.
        if !a.owed.as_ref().is_some_and(|o| o.owed()) {
            return false;
        }
        a.next_payer.set(Some(by));
        self.bar2_sync(root, sys)
    }

    /// The deferral state the reactor watches, when the deferral is on.
    pub fn bar2_owed(&self) -> Option<std::sync::Arc<crate::bar2area::Owed>> {
        self.bar2.as_deref().and_then(|a| a.owed.clone())
    }

    /// Count an access of a trapped BAR2 page.
    pub fn bar2_note_trapped(&self) {
        if let Some(a) = self.bar2.as_deref() {
            a.note_trapped();
        }
    }

    /// Move the BAR2 memfd to `to` (the fresh model of a function-level reset): the VMM
    /// keeps the memfd mapped across the reset. What the guest wrote is written back
    /// first; the fresh model starts with nothing mapped and walks the BAR2 tables again
    /// at its first sync.
    pub fn carry_bar2_area(&mut self, to: &mut FakeFb) {
        let size = self.size;
        if let Some(mut a) = self.bar2.take() {
            let pool = self.pool.borrow();
            let table = pool.as_ref().map(|p| &p.table).filter(|t| !t.is_empty());
            let tr = |pa: u64| table.map_or(pa, |t| t.translate(pa));
            a.sync(&mut *self.store, size, None, None, &tr);
            drop(pool);
            self.store.flush();
            a.reset_mapping();
            to.bar2 = Some(a);
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
        // The BAR2 memfd (bar2area.rs) is keyed by the guest FB address and holds the
        // guest's newest bytes of a page behind BAR2: it is asked first, with `pa`. Every
        // other page is in the store where design H (bar1pool.rs) puts it: its pool page
        // when remapped, else `pa` (bar2-memfd.md §8).
        let pool = self.pool.borrow();
        let table = pool.as_ref().map(|p| &p.table).filter(|t| !t.is_empty());
        match (self.bar2.as_deref().filter(|a| a.has_live()), table) {
            (Some(a), t) => {
                let tr = |x: u64| t.map_or(x, |t| t.translate(x));
                a.read_through(&*self.store, &tr, pa, buf)
            }
            (None, None) => self.store.read(pa, buf),
            (None, Some(t)) => t.for_each_piece(pa, buf.len(), |hpa, off, n| {
                self.store.read(hpa, &mut buf[off..off + n])
            }),
        }
        true
    }

    pub fn write(&mut self, pa: u64, data: &[u8]) -> bool {
        self.check('w', pa, data.len());
        if pa + data.len() as u64 > self.size {
            return false;
        }
        self.filter.before_write(pa, data);
        self.note_write(pa, data);
        // The same order as `read`: VRAM for the GPU at the address the BAR1 pool's remap
        // gives, and, when the BAR2 memfd is on, the memfd for the guest's BAR2 view by the
        // guest address, its key (write-through).
        let pool = self.pool.borrow();
        let table = pool.as_ref().map(|p| &p.table).filter(|t| !t.is_empty());
        let store = &mut self.store;
        match table {
            None => store.write(pa, data),
            Some(t) => t.for_each_piece(pa, data.len(), |hpa, off, n| {
                store.write(hpa, &data[off..off + n])
            }),
        }
        if let Some(a) = self.bar2.as_deref_mut() {
            a.device_write(pa, data);
        }
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

    /// Turn on the P21 check for the passthrough area: guest BAR1 `[0, len)` is backed
    /// by the reservation piece `[fb0, fb0 + len)`, the guest's BAR1 tables are rooted at
    /// `root`. The first [`Self::poll_bar1_passthrough`] walks the area.
    pub fn set_bar1_passthrough(&mut self, root: u64, fb0: u64, len: u64) {
        *self.bar1_pt.borrow_mut() = Some(Bar1Pt {
            root,
            fb0,
            len,
            table_pages: HashSet::new(),
            mapped: Vec::new(),
            reported: HashSet::new(),
            hazards: HashSet::new(),
            walk_dirty: true,
            hazard_dirty: true,
            stats: Bar1PtStats::default(),
        });
    }

    /// Move the passthrough state to `to` (the fresh model of a function-level reset):
    /// the VMM keeps the area mapped across the reset, so the check must go on. The
    /// counters carry over; the next poll walks the area again.
    pub fn carry_bar1_passthrough(&self, to: &FakeFb) {
        if let Some(mut pt) = self.bar1_pt.borrow_mut().take() {
            pt.table_pages.clear();
            pt.mapped.clear();
            pt.walk_dirty = true;
            pt.hazard_dirty = true;
            *to.bar1_pt.borrow_mut() = Some(pt);
        }
        // The pool area stays mapped by the VMM across the reset as well; the remaps go
        // with the old model (its host channels and shadows are freed with it) and the
        // fresh model walks the guest's new BAR1 tables from scratch. No data is copied
        // back: the guest re-initializes its FB after a function-level reset.
        if let Some(p) = self.pool.borrow_mut().take() {
            *to.pool.borrow_mut() = Some(p.after_reset());
        }
    }

    /// `(fb0, len)` of the passthrough area, when on.
    pub fn bar1_passthrough(&self) -> Option<(u64, u64)> {
        self.bar1_pt.borrow().as_ref().map(|p| (p.fb0, p.len))
    }

    /// The check's counters, when on.
    pub fn bar1_passthrough_stats(&self) -> Option<Bar1PtStats> {
        self.bar1_pt.borrow().as_ref().map(|p| p.stats.clone())
    }

    /// Turn on the BAR1 pool (P21 design H, docs/caraxes/bar1-pool.md): guest BAR1
    /// `[0, len)` is backed by the pool `[pool0, pool0 + len)`, the guest's BAR1 tables are
    /// rooted at `root`, and only pages of `usable` may be remapped into the pool. The first
    /// [`crate::bar1pool::poll`] walks the whole area.
    pub fn set_bar1_pool(&mut self, root: u64, pool0: u64, len: u64, usable: (u64, u64)) {
        *self.pool.borrow_mut() = Some(crate::bar1pool::Bar1Pool::new(root, pool0, len, usable));
    }

    /// `(pool0, len)` of the BAR1 pool, when on.
    pub fn bar1_pool(&self) -> Option<(u64, u64)> {
        self.pool.borrow().as_ref().map(|p| p.table.pool())
    }

    /// The pool state (bar1pool.rs).
    pub(crate) fn pool_mut(&self) -> std::cell::RefMut<'_, Option<crate::bar1pool::Bar1Pool>> {
        self.pool.borrow_mut()
    }

    /// The one translation every device path applies to a guest FB address (P21 design
    /// H): a page the guest maps into the BAR1 pool area lives at its pool page, every
    /// other address at itself. The FB store (and with it BAR2, PRAMIN, the BAR aperture
    /// TLB's targets, the table walks and the software executor) applies it inside
    /// [`Self::read`] and [`Self::write`]; the shadow (its leaves) and the mirror (USERD
    /// handed to a host channel) call it for the addresses they give the host RM.
    pub fn remap(&self, pa: u64) -> u64 {
        match self.pool.borrow().as_ref() {
            Some(p) => p.table.translate(pa),
            None => pa,
        }
    }

    /// Whether the page of `pa` is remapped into the pool now.
    pub fn is_remapped(&self, pa: u64) -> bool {
        self.pool
            .borrow()
            .as_ref()
            .is_some_and(|p| p.table.is_remapped(pa))
    }

    /// Guest GMMU leaves `(va, target, size, kind)` as the physical GPU must see them: a
    /// leaf on a page that lives in the pool now points at its pool page; a 64 KiB or
    /// 2 MiB leaf with a remapped page inside is split into 4 KiB leaves (the shadow maps
    /// them one by one and coalesces the contiguous rest). Unchanged without a pool.
    pub fn remap_leaves(&self, leaves: Vec<(u64, Target, u64, u8)>) -> Vec<(u64, Target, u64, u8)> {
        match self.pool.borrow().as_ref().map(|p| &p.table).filter(|t| !t.is_empty()) {
            None => leaves,
            Some(t) => leaves.into_iter().flat_map(|l| t.split_leaf(l)).collect(),
        }
    }

    /// Whether a remap changed a page the shadow of guest root `root` maps (the shadow must
    /// re-diff every unit of that root at its next sync).
    pub fn pool_rediff_pending(&self, root: u64) -> bool {
        self.pool
            .borrow()
            .as_ref()
            .is_some_and(|p| p.rediff.contains(&root))
    }

    /// [`Self::pool_rediff_pending`], and clear it (the shadow's plan is taking it).
    pub fn take_pool_rediff(&self, root: u64) -> bool {
        self.pool
            .borrow_mut()
            .as_mut()
            .is_some_and(|p| p.rediff.remove(&root))
    }

    /// Copy one 4 KiB page from FB address `from` to `to` in the store itself, past the
    /// remap (the pool's copy-in and copy-back).
    pub(crate) fn copy_page_raw(&mut self, from: u64, to: u64) {
        let mut b = vec![0u8; PAGE as usize];
        self.store.read(from, &mut b);
        self.store.write(to, &b);
    }

    /// Drop the watched copies of the dwords on page `page` (its bytes moved).
    pub(crate) fn forget_watched_page(&self, page: u64) {
        self.watched.borrow_mut().retain(|k, _| k >> 12 != page);
    }

    /// Whether page `page` is a guest page-table page one of the cached walks watches.
    pub(crate) fn watches_table_page(&self, page: u64) -> bool {
        self.walks.borrow().values().any(|c| c.watches(page))
    }

    /// The pool's exit-summary line, when on.
    pub fn bar1_pool_summary(&self) -> Option<String> {
        self.pool.borrow().as_ref().map(|p| p.summary())
    }

    /// Every mapped 4 KiB page of the BAR1 range `[0, len)` under `root`, and the table
    /// pages the walk read. Reads each table once per 2 MiB unit (the area is one or a
    /// few PD0 entries), not five qwords per page: BAR1 reads of VRAM are uncached.
    fn area_walk(&self, root: u64, len: u64) -> (Vec<(u64, Target)>, HashSet<u64>) {
        let mut out = Vec::new();
        let mut pages = HashSet::from([root >> 12]);
        let mut va = 0u64;
        while va < len {
            let n = ((len - va).min(0x20_0000) >> 12) as usize;
            let (m, p) = self.area_walk_unit(root, va, n);
            out.extend(m);
            pages.extend(p);
            va += 0x20_0000;
        }
        (out, pages)
    }

    /// The area walk of one 2 MiB unit at BAR1 `va` (2 MiB aligned), its first `n` 4 KiB
    /// pages: every mapped page `(guest BAR1 offset, page target)` and the FB table pages
    /// read (the root included).
    pub(crate) fn area_walk_unit(
        &self,
        root: u64,
        va: u64,
        n: usize,
    ) -> (Vec<(u64, Target)>, Vec<u64>) {
        let mut out = Vec::new();
        let mut pages = vec![root >> 12];
        let note = |pages: &mut Vec<u64>, t: Target| {
            if let Target::Fb(pa) = t {
                pages.push(pa >> 12);
            }
        };
        let Some(e3) = self.read_u64(root + 8 * ((va >> 47) & 3)) else {
            return (out, pages);
        };
        let Some(pd2) = Self::next_table(e3) else {
            return (out, pages);
        };
        note(&mut pages, pd2);
        let Some(e2) = self.table_u64(None, pd2, 8 * ((va >> 38) & 0x1FF)) else {
            return (out, pages);
        };
        let Some(pd1) = Self::next_table(e2) else {
            return (out, pages);
        };
        note(&mut pages, pd1);
        let Some(e1) = self.table_u64(None, pd1, 8 * ((va >> 29) & 0x1FF)) else {
            return (out, pages);
        };
        let Some(pd0) = Self::next_table(e1) else {
            return (out, pages);
        };
        note(&mut pages, pd0);
        let idx0 = (va >> 21) & 0xFF;
        let (Some(lo), Some(hi)) = (
            self.table_u64(None, pd0, 16 * idx0),
            self.table_u64(None, pd0, 16 * idx0 + 8),
        ) else {
            return (out, pages);
        };
        if lo & 1 == 1 {
            // A 2 MiB page: every 4 KiB page of the unit at its offset.
            for i in 0..n as u64 {
                let t = Self::decode_pte(lo, i << 12);
                if matches!(t, Target::Fb(_) | Target::Sys(_)) {
                    out.push((va + (i << 12), t));
                }
            }
            return (out, pages);
        }
        let big_t = Self::big_table(lo);
        let small_t = Self::next_table(hi);
        if let Some(t) = big_t {
            note(&mut pages, t);
        }
        if let Some(t) = small_t {
            note(&mut pages, t);
        }
        let big = big_t.and_then(|t| self.table_block(None, t, 32));
        let small = small_t.and_then(|t| self.table_block(None, t, 512));
        for i in 0..n {
            let g = va + ((i as u64) << 12);
            // walk_in's precedence: the small PTE unless it is invalid and a big
            // table exists, then the big PTE.
            let from_small = small
                .as_ref()
                .map(|pt| Self::decode_pte(pt[i], 0))
                .filter(|t| !matches!(t, Target::Unmapped { .. }) || big.is_none());
            let t = match from_small {
                Some(t) => t,
                None => match &big {
                    Some(bt) => Self::decode_pte(bt[i >> 4], (g & 0xFFFF) & !0xFFF),
                    None => Target::Unmapped { level: 0 },
                },
            };
            if matches!(t, Target::Fb(_) | Target::Sys(_)) {
                out.push((g, t));
            }
        }
        (out, pages)
    }

    /// Run the P21 check if the guest changed the BAR1 tables over the area or the
    /// shadow's table pages changed since the last run. Cheap when nothing changed (two
    /// flags); called after trapped BAR0 writes, which is where the guest flushes and
    /// invalidates after a BAR1 mapping change and before it uses the mapping. Findings
    /// are logged (the first 16) and counted; the area itself cannot be withdrawn from
    /// the VMM (vfio-user has no revocation), so a mismatch is reported, not repaired.
    /// Returns whether a check ran.
    pub fn poll_bar1_passthrough(&self) -> bool {
        let (root, len, walk) = match self.bar1_pt.borrow().as_ref() {
            Some(p) if p.walk_dirty || p.hazard_dirty => (p.root, p.len, p.walk_dirty),
            _ => return false,
        };
        let walked = walk.then(|| self.area_walk(root, len));
        let mut guard = self.bar1_pt.borrow_mut();
        let Some(pt) = guard.as_mut() else {
            return false;
        };
        pt.stats.checks += 1;
        let mut log = Vec::new();
        if let Some((mapped, pages)) = walked {
            pt.table_pages = pages;
            pt.mapped = mapped;
            pt.walk_dirty = false;
            let (mut ok, mut bad) = (0u64, 0u64);
            for &(g, t) in &pt.mapped {
                let want = pt.fb0 + g;
                if t == Target::Fb(want) {
                    ok += 1;
                    continue;
                }
                bad += 1;
                if pt.reported.insert(g) {
                    pt.stats.mismatch_events += 1;
                    log.push(format!(
                        "guest BAR1 {g:#x} maps {t:?}, the passthrough area gives FB {want:#x}: guest CPU accesses of this page reach the wrong page of its own reservation"
                    ));
                }
            }
            let changed = pt.stats.mapped != pt.mapped.len() as u64 || pt.stats.mismatched != bad;
            pt.stats.mapped = pt.mapped.len() as u64;
            pt.stats.matched = ok;
            pt.stats.mismatched = bad;
            if changed {
                eprintln!(
                    "vfio-user-nvgpu: bar1 passthrough: check {}: {} page(s) of guest BAR1 [0, {:#x}) mapped, {} at the area's FB {:#x} + offset, {} elsewhere",
                    pt.stats.checks, pt.stats.mapped, pt.len, ok, pt.fb0, bad
                );
            }
        }
        {
            let walks = self.walks.borrow();
            for &(g, t) in &pt.mapped {
                let Target::Fb(pa) = t else { continue };
                let page = pa >> 12;
                if walks.values().any(|c| c.watches(page)) && pt.hazards.insert(page) {
                    pt.stats.table_hazards += 1;
                    log.push(format!(
                        "guest BAR1 {g:#x} reaches FB {pa:#x}, a page-table page the shadow watches: guest writes there bypass the shadow's write tracking (CARAXES_SHADOW_ALWAYS_RESYNC=1 is the safe setting while this persists)"
                    ));
                }
            }
        }
        pt.hazard_dirty = false;
        for l in log {
            if pt.stats.first.len() < 16 {
                eprintln!("vfio-user-nvgpu: ERROR: bar1 passthrough: {l}");
                pt.stats.first.push(l);
            }
        }
        true
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
    fn bar1_passthrough_area_stays_in_the_usable_reservation() {
        // The 2 GiB profile at slot 0 on the RTX 2070 host: reservation [0x169000000,
        // 0x1e9000000), usable up to the 2 MiB carve-out.
        let l = FbLayout::reserved(2 << 30, 8 << 30, 0x1_6900_0000, 0x1_E900_0000).unwrap();
        let bar1 = 256 << 20;
        assert_eq!(
            bar1_passthrough_area(&l, BAR1_PT_FB_OFFSET, 2 << 20, bar1),
            Ok((0x1_6998_0000, 2 << 20))
        );
        // Unaligned, too large for the BAR, or reaching into the carve-out: refused.
        assert!(bar1_passthrough_area(&l, 0x98_1000, 2 << 20, bar1).is_err());
        assert!(bar1_passthrough_area(&l, 0, 512 << 20, bar1).is_err());
        let usable = l.usable().unwrap();
        let last = usable.1 - usable.0 - (2 << 20);
        assert!(bar1_passthrough_area(&l, last, 2 << 20, bar1).is_ok());
        assert!(bar1_passthrough_area(&l, last + (64 << 10), 2 << 20, bar1).is_err());
        assert!(bar1_passthrough_area(&l, u64::MAX & !0xFFFF, 2 << 20, bar1).is_err());
        // No reservation, no passthrough.
        assert!(bar1_passthrough_area(&FbLayout::plain(2 << 30), 0, 2 << 20, bar1).is_err());
        assert_eq!(parse_u64("0x980000"), Some(0x98_0000));
        assert_eq!(parse_u64("4096"), Some(4096));
        assert_eq!(parse_u64("zz"), None);
    }

    #[test]
    fn bar1_passthrough_check_follows_the_guest_bar1_tables() {
        let mut fb = FakeFb::new(64 << 20);
        let (root, pd2, pd1, pd0) = (0x10000u64, 0x11000u64, 0x12000u64, 0x13000u64);
        let (pt_small, pt_big) = (0x14000u64, 0x15000u64);
        let vid = |addr: u64| ((addr >> 12) << 8) | 0b010;
        let pte = |pa: u64| ((pa >> 12) << 8) | 1;
        fb.write_u64(root, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        fb.write_u64(pd0, ((pt_big >> 8) << 4) | 0b010);
        fb.write_u64(pd0 + 8, vid(pt_small));
        let fb0 = 0x100_0000u64;
        // BAR1 0x80000 and 0x81000 on the area's own pages: correct.
        fb.write_u64(pt_small + 8 * 0x80, pte(fb0 + 0x8_0000));
        fb.write_u64(pt_small + 8 * 0x81, pte(fb0 + 0x8_1000));
        fb.set_bar1_passthrough(root, fb0, 2 << 20);
        assert_eq!(fb.bar1_passthrough(), Some((fb0, 2 << 20)));
        assert!(fb.poll_bar1_passthrough());
        let st = fb.bar1_passthrough_stats().unwrap();
        assert_eq!(
            (st.checks, st.mapped, st.matched, st.mismatched),
            (1, 2, 2, 0)
        );
        // Nothing changed: no second walk.
        assert!(!fb.poll_bar1_passthrough());
        // A data write elsewhere does not re-arm it.
        fb.write_u64(fb0 + 0x8_0000, 0xdead);
        assert!(!fb.poll_bar1_passthrough());
        // The guest maps BAR1 0x82000 somewhere else: a mismatch, counted once.
        fb.write_u64(pt_small + 8 * 0x82, pte(0x30_0000));
        assert!(fb.poll_bar1_passthrough());
        let st = fb.bar1_passthrough_stats().unwrap();
        assert_eq!(
            (st.mapped, st.matched, st.mismatched, st.mismatch_events),
            (3, 2, 1, 1)
        );
        fb.write_u64(pt_small + 8 * 0x83, pte(fb0 + 0x8_3000));
        assert!(fb.poll_bar1_passthrough());
        let st = fb.bar1_passthrough_stats().unwrap();
        assert_eq!((st.mapped, st.mismatched, st.mismatch_events), (4, 1, 1));
        // A 64 KiB big page at BAR1 0x10000 on the area's own page: all 16 small pages match.
        fb.write_u64(pt_big + 8, pte(fb0 + 0x1_0000));
        assert!(fb.poll_bar1_passthrough());
        let st = fb.bar1_passthrough_stats().unwrap();
        assert_eq!((st.mapped, st.matched), (20, 19));
        // A page-table page the shadow watches becomes reachable through the area: the
        // guest maps its own table page (a watched root's PT) at BAR1 0x84000.
        let r2 = fb0 + 0x8_4000;
        let (r2pd2, r2pd1, r2pd0, r2pt) = (0x21000u64, 0x22000u64, 0x23000u64, 0x24000u64);
        fb.write_u64(r2, vid(r2pd2));
        fb.write_u64(r2pd2, vid(r2pd1));
        fb.write_u64(r2pd1, vid(r2pd0));
        fb.write_u64(r2pd0 + 8, vid(r2pt));
        fb.write_u64(r2pt, pte(0x40_0000));
        let mem = GuestMemory::new();
        fb.walk_update(Some(&mem), Target::Fb(r2), false, 1 << 20);
        fb.write_u64(pt_small + 8 * 0x84, pte(r2));
        assert!(fb.poll_bar1_passthrough());
        let st = fb.bar1_passthrough_stats().unwrap();
        assert_eq!(st.table_hazards, 1);
        assert_eq!(st.first.len(), 2);
        // The state survives a model reset and walks again on the fresh model.
        let fresh = FakeFb::new(64 << 20);
        fb.carry_bar1_passthrough(&fresh);
        assert_eq!(fb.bar1_passthrough(), None);
        assert_eq!(fresh.bar1_passthrough(), Some((fb0, 2 << 20)));
        assert!(fresh.poll_bar1_passthrough());
        let st = fresh.bar1_passthrough_stats().unwrap();
        assert_eq!((st.mapped, st.mismatch_events, st.table_hazards), (0, 1, 1));
    }

    /// The BAR2 memfd end to end on the fake FB: the remap follows the guest's BAR2 tables
    /// (written through their own self-mapping in the memfd), the dirty scan feeds the
    /// shadow's walk cache, pages leaving BAR2 are back in the store, device writes reach
    /// the guest's view, and the trapped page is never mapped.
    #[test]
    fn bar2_memfd_follows_the_bar2_tables_and_feeds_the_walk_cache() {
        use crate::bar2area::{Bar2Area, Config};
        let vid = |a: u64| ((a >> 12) << 8) | 0b010;
        let pte = |a: u64| ((a >> 12) << 8) | (0x06 << 56) | 1;
        let mut fb = FakeFb::new(64 << 20);
        // BAR2 tables: root 0x10000 -> 0x11000 -> 0x12000 -> PD0 0x13000 -> PT 0x14000.
        let root = 0x10000;
        fb.write_u64(root, vid(0x11000));
        fb.write_u64(0x11000, vid(0x12000));
        fb.write_u64(0x12000, vid(0x13000));
        fb.write_u64(0x13008, vid(0x14000));
        let bar2_pte = |fb: &mut FakeFb, page: u64, pa: u64| fb.write_u64(0x14000 + 8 * page, pte(pa));
        // BAR2 page 1 maps the PT itself; pages 2..=6 a channel's tables at 0x30000..;
        // the trapped page 0x1f maps 0x70000.
        bar2_pte(&mut fb, 1, 0x14000);
        for i in 0..5 {
            bar2_pte(&mut fb, 2 + i, 0x30000 + i * 0x1000);
        }
        bar2_pte(&mut fb, 0x1f, 0x70000);
        // The channel's tree: 0x30000 -> 0x31000 -> 0x32000 -> 0x33000 -> PT 0x34000,
        // one leaf at VA 0x3000.
        fb.write_u64(0x30000, vid(0x31000));
        fb.write_u64(0x31000, vid(0x32000));
        fb.write_u64(0x32000, vid(0x33000));
        fb.write_u64(0x33008, vid(0x34000));
        fb.write_u64(0x34000 + 8 * 3, pte(0x50000));
        fb.write_u64(0x60008, 0x6060);

        let cfg = Config {
            traps: vec![0x1f000],
            sync_doorbell: false,
            flush_defer_us: None,
            flush_page_memory: false,
        };
        let area = Bar2Area::create(0x20000, &cfg).unwrap();
        let guest = area.mem.map_again().unwrap();
        fb.set_bar2_area(area).unwrap();
        assert!(fb.bar2_sync(Some(root), None));
        // Live: the PT at BAR2 page 1, the channel tables at 2..=6; not the trapped page.
        assert_eq!(fb.backing(0x14000), Backing::Bar2Memfd(0x1000));
        assert_eq!(fb.backing(0x34018), Backing::Bar2Memfd(0x6018));
        assert_eq!(fb.backing(0x70000), Backing::Store);
        assert_eq!(guest.load_word(6, 3), pte(0x50000));

        let croot = Target::Fb(0x30000);
        let u = fb.walk_update(None, croot, false, 1 << 20);
        assert!(u.full);
        assert_eq!(fb.with_walk(croot, |c| c.leaf_count()), Some(1));
        // The guest writes a PTE through BAR2: the device does not see it until a sync.
        guest.store_word(6, 5, pte(0x51000));
        assert!(!fb.tables_dirty(croot));
        fb.bar2_sync(Some(root), None);
        assert!(fb.tables_dirty(croot));
        let u = fb.walk_update(None, croot, false, 1 << 20);
        assert!(!u.full);
        assert_eq!(u.changed, vec![0]);
        assert_eq!(u.reads, 1); // the dirty page, read again from the memfd
        assert_eq!(fb.with_walk(croot, |c| c.leaf_count()), Some(2));

        // The guest re-points BAR2 page 6 at 0x60000 through the self-mapped PT.
        guest.store_word(1, 6, pte(0x60000));
        fb.bar2_sync(Some(root), None);
        assert_eq!(fb.backing(0x34000), Backing::Store);
        assert_eq!(fb.backing(0x60000), Backing::Bar2Memfd(0x6000));
        // 0x60000 was loaded into the memfd; 0x34000 is back in the store with the write.
        assert_eq!(guest.load_word(6, 1), 0x6060);
        assert_eq!(fb.read_u64(0x34000 + 8 * 5), Some(pte(0x51000)));
        // A device write (PRAMIN, an RPC) reaches the guest's BAR2 view and the store,
        // and is not taken for a guest write.
        fb.write_u64(0x60010, 0xABC);
        assert_eq!(guest.load_word(6, 2), 0xABC);
        let before = fb.bar2_area().unwrap().stats().dirty;
        fb.bar2_sync(Some(root), None);
        assert_eq!(fb.bar2_area().unwrap().stats().dirty, before);

        // A function-level reset carries the area over with nothing mapped; the next sync
        // maps again from the tables, which are in the store.
        let mut fresh = FakeFb::new(64 << 20);
        fb.carry_bar2_area(&mut fresh);
        assert!(fb.bar2_area().is_none());
        assert!(!fresh.bar2_area().unwrap().has_live());
        assert_eq!(fb.read_u64(0x14000 + 8 * 6), Some(pte(0x60000)));
    }

    /// The flush deferral (`CARAXES_BAR2_FLUSH_DEFER_US`, docs/caraxes/intr-page.md
    /// §11.2): a flush read owes the sync instead of running it, a payer runs it once for
    /// any number of flush reads, and a sync that runs anyway (the MMU invalidate) clears
    /// the debt.
    #[test]
    fn bar2_flush_deferral_owes_and_pays_one_sync() {
        use crate::bar2area::{Bar2Area, Config, Payer};
        let vid = |a: u64| ((a >> 12) << 8) | 0b010;
        let pte = |a: u64| ((a >> 12) << 8) | (0x06 << 56) | 1;
        let mut fb = FakeFb::new(64 << 20);
        let root = 0x10000;
        fb.write_u64(root, vid(0x11000));
        fb.write_u64(0x11000, vid(0x12000));
        fb.write_u64(0x12000, vid(0x13000));
        fb.write_u64(0x13008, vid(0x14000));
        // BAR2 page 1 maps the PT, page 2 a channel's PT at 0x34000.
        fb.write_u64(0x14000 + 8, pte(0x14000));
        fb.write_u64(0x14000 + 16, pte(0x34000));
        let cfg = Config {
            traps: vec![0x1f000],
            sync_doorbell: false,
            flush_defer_us: Some(200),
            flush_page_memory: false,
        };
        let area = Bar2Area::create(0x20000, &cfg).unwrap();
        let guest = area.mem.map_again().unwrap();
        fb.set_bar2_area(area).unwrap();
        assert!(fb.bar2_sync(Some(root), None));
        let owed = fb.bar2_owed().expect("the deferral is on");
        let syncs = |fb: &FakeFb| fb.bar2_area().unwrap().stats().syncs;
        // What VRAM (the store) holds, not the memfd's view.
        let raw = |fb: &FakeFb, pa: u64| {
            let mut b = [0u8; 8];
            fb.store.read(pa, &mut b);
            Some(u64::from_le_bytes(b))
        };
        let s0 = syncs(&fb);

        // Two PTE writes, each followed by a flush read: nothing reaches the store yet.
        guest.store_word(2, 3, pte(0x51000));
        assert!(fb.bar2_flush(Some(root), None));
        guest.store_word(2, 4, pte(0x52000));
        assert!(fb.bar2_flush(Some(root), None));
        assert_eq!(syncs(&fb), s0);
        assert!(owed.owed());
        assert_eq!(raw(&fb, 0x34000 + 8 * 3), Some(0));
        // Not due before its bound, due after it.
        let since = crate::bar2area::mono_ns();
        assert!(!owed.due(since));
        assert!(owed.due(since + 200_000 + 1_000_000));
        // A payer (a trapped write, a doorbell, the reactor) runs one sync for both.
        assert!(fb.bar2_pay(Some(root), None, Payer::Access));
        assert_eq!(syncs(&fb), s0 + 1);
        assert!(!owed.owed());
        assert_eq!(raw(&fb, 0x34000 + 8 * 3), Some(pte(0x51000)));
        assert_eq!(raw(&fb, 0x34000 + 8 * 4), Some(pte(0x52000)));
        // Nothing owed: paying is free.
        assert!(!fb.bar2_pay(Some(root), None, Payer::Reactor));
        assert_eq!(syncs(&fb), s0 + 1);
        // A sync that runs anyway (the MMU invalidate) pays what is owed.
        guest.store_word(2, 5, pte(0x53000));
        fb.bar2_flush(Some(root), None);
        fb.bar2_sync(Some(root), None);
        assert!(!owed.owed());
        assert_eq!(raw(&fb, 0x34000 + 8 * 5), Some(pte(0x53000)));
        use std::sync::atomic::Ordering::Relaxed;
        assert_eq!(owed.deferred.load(Relaxed), 3);
        assert_eq!(owed.paid_access.load(Relaxed), 1);
        assert_eq!(owed.paid_reactor.load(Relaxed), 0);
        assert_eq!(owed.paid_sync.load(Relaxed), 1);
        assert!(owed.summary().contains("3 flush read(s) deferred"));
    }

    /// The flush page as memory (`CARAXES_BAR2_FLUSH_PAGE=memory`, intr-page.md §12.1): no
    /// page of BAR2 traps, no read starts a debt, and still no GPU use of a page-table
    /// update can come before its sync. The guest RM makes a PTE update usable only
    /// through an MMU invalidate (D17: kbusUpdateRmAperture_GM107 and every
    /// dmaUpdateVASpace path end in one), and the invalidate syncs before it walks; a
    /// doorbell or trapped write pays as well, and the reactor at the bound.
    #[test]
    fn bar2_flush_page_as_memory_syncs_at_the_consumers() {
        use crate::bar2area::{config_from, Bar2Area, Payer};
        use std::sync::atomic::Ordering::Relaxed;
        let vid = |a: u64| ((a >> 12) << 8) | 0b010;
        let pte = |a: u64| ((a >> 12) << 8) | (0x06 << 56) | 1;
        let mut fb = FakeFb::new(64 << 20);
        let root = 0x10000;
        fb.write_u64(root, vid(0x11000));
        fb.write_u64(0x11000, vid(0x12000));
        fb.write_u64(0x12000, vid(0x13000));
        fb.write_u64(0x13008, vid(0x14000));
        fb.write_u64(0x14000 + 8, pte(0x14000));
        fb.write_u64(0x14000 + 16, pte(0x34000));
        let cfg = config_from(None, false, Some("200"), Some("memory")).unwrap();
        assert!(cfg.traps.is_empty());
        let area = Bar2Area::create(0x20000, &cfg).unwrap();
        // The whole aperture is one sparse area: the flush page no longer traps.
        assert_eq!(area.sparse_areas(), vec![(0, 0x20000)]);
        let guest = area.mem.map_again().unwrap();
        fb.set_bar2_area(area).unwrap();
        assert!(fb.bar2_sync(Some(root), None));
        let owed = fb.bar2_owed().expect("standing debt");
        assert!(owed.standing && owed.owed());
        let raw = |fb: &FakeFb, pa: u64| {
            let mut b = [0u8; 8];
            fb.store.read(pa, &mut b);
            u64::from_le_bytes(b)
        };
        // A PTE update, no flush read seen: VRAM does not have it yet.
        guest.store_word(2, 3, pte(0x51000));
        assert_eq!(raw(&fb, 0x34000 + 8 * 3), 0);
        // The MMU invalidate (a sync that runs anyway) brings it before any walk.
        fb.bar2_sync(Some(root), None);
        assert_eq!(raw(&fb, 0x34000 + 8 * 3), pte(0x51000));
        // Still owed after the sync: the next write is not seen either.
        assert!(owed.owed());
        // A doorbell pays without any flush read before it.
        guest.store_word(2, 4, pte(0x52000));
        assert!(fb.bar2_pay(Some(root), None, Payer::Access));
        assert_eq!(raw(&fb, 0x34000 + 8 * 4), pte(0x52000));
        // The reactor's bound: a sync that wrote back bytes is counted.
        guest.store_word(2, 5, pte(0x53000));
        assert!(fb.bar2_pay(Some(root), None, Payer::Reactor));
        assert_eq!(raw(&fb, 0x34000 + 8 * 5), pte(0x53000));
        assert!(fb.bar2_pay(Some(root), None, Payer::Reactor));
        assert_eq!(owed.paid_access.load(Relaxed), 1);
        assert_eq!(owed.paid_reactor.load(Relaxed), 2);
        assert_eq!(owed.bound_found.load(Relaxed), 1);
        assert!(owed.paid_sync.load(Relaxed) >= 2);
        assert_eq!(owed.deferred.load(Relaxed), 0);
        assert!(owed.summary().starts_with("bar2 flush page as memory: bound 200 us"));
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

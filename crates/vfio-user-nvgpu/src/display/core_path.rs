// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The display core's path without the device mutex (docs/caraxes/display-own-clock.md §2).
//!
//! The card's display engine latches, writes its records and raises its interrupt on its
//! own clock; an interrupt handler's status read never waits for another engine's work.
//! Here the engine sits behind the display lock (`Display::engine`), and this path reaches
//! its records without the device mutex:
//!
//! - guest memory through the published view (guestmem.rs `MemView`): Windows' flip
//!   notifiers;
//! - FB records through [`RecordPins`]: 2 MiB windows of VRAM mapped for good when a window
//!   `UPDATE` resolves a semaphore or notifier there (under the device mutex, where the FB
//!   store lives). Windows' acquire and release semaphores are in VRAM.
//!
//! A record neither reaches waits for the device path ([`crate::display::engine::RecordIo::can`]).
//! Lock order: device mutex, display lock, the pins' lock.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock, RwLockReadGuard};

use super::engine::{self, Engine, MemRef, RecordIo};
use crate::fb::{FakeFb, SharedWindow};
use crate::guestmem::GuestMemory;

/// One FB record window: FB `[pa, pa + len)` through `win` (reservation offsets from
/// `win.offset()`).
pub struct RecordWindow {
    pub pa: u64,
    pub len: u64,
    pub win: Arc<dyn SharedWindow>,
}

impl RecordWindow {
    fn covers(&self, pa: u64, len: u64) -> bool {
        pa >= self.pa && pa.checked_add(len).is_some_and(|e| e <= self.pa + self.len)
    }

    fn at(&self, pa: u64) -> u64 {
        self.win.offset() + (pa - self.pa)
    }
}

/// The FB record windows of one display (shared by the engine and the display clock).
#[derive(Default)]
pub struct RecordPins {
    windows: RwLock<Vec<RecordWindow>>,
    /// Regions the FB store could not share (a page pooled or behind the BAR2 memfd, fixed
    /// BAR1 passthrough, no VRAM) or took back.
    refused: AtomicU64,
    refused_regions: Mutex<Vec<u64>>,
    /// Windows taken back because a page of theirs entered the BAR2 memfd or the BAR1 pool.
    revoked: AtomicU64,
}

impl crate::fb::RecordRevoke for RecordPins {
    /// Under the device lock, before a page of the region at `start` enters the BAR2 memfd
    /// or the BAR1 pool: the window goes once no tick uses it (the write lock waits for the
    /// ticks' read guards), and the region stays on the device path from then on.
    fn revoke(&self, start: u64) {
        let mut r = self.refused_regions.lock().unwrap_or_else(PoisonError::into_inner);
        let mut w = self.windows.write().unwrap_or_else(PoisonError::into_inner);
        let before = w.len();
        w.retain(|x| x.pa != start);
        if w.len() != before {
            self.revoked.fetch_add(1, Ordering::Relaxed);
        }
        if !r.contains(&start) {
            r.push(start);
        }
    }
}

impl RecordPins {
    /// The window size and alignment.
    pub const REGION: u64 = 2 << 20;
    /// At most this many windows per display (host BAR1 is shared).
    pub const MAX: usize = 4;

    /// Under the device mutex: make the FB record `[pa, pa + len)` reachable without it,
    /// mapping its 2 MiB region once. Returns a note for the trace when a window was mapped
    /// or a region refused for the first time.
    pub fn ensure(self: &Arc<Self>, fb: &mut FakeFb, pa: u64, len: u64) -> Option<String> {
        let start = pa & !(Self::REGION - 1);
        if (pa + len).div_ceil(Self::REGION) * Self::REGION - start > Self::REGION {
            return None; // a record across two regions stays on the device path
        }
        if self.read().iter().any(|w| w.covers(pa, len)) {
            return None;
        }
        {
            let mut r = self.refused_regions.lock().unwrap_or_else(PoisonError::into_inner);
            if r.contains(&start) {
                return None;
            }
            let mut w = self.windows.write().unwrap_or_else(PoisonError::into_inner);
            if w.len() < Self::MAX {
                let owner: Arc<dyn crate::fb::RecordRevoke> = self.clone();
                if let Some(win) = fb.share_record_window(start, Self::REGION as usize, &owner) {
                    w.push(RecordWindow { pa: start, len: Self::REGION, win });
                    return Some(format!(
                        "display: FB records at {start:#x} (+2 MiB) mapped for the display clock ({} window(s))",
                        w.len()
                    ));
                }
            }
            r.push(start);
        }
        self.refused.fetch_add(1, Ordering::Relaxed);
        Some(format!(
            "display: FB records at {start:#x} stay on the device path (no shared window: a page of the region in the BAR1 pool or behind the BAR2 memfd, fixed BAR1 passthrough, no VRAM store, or {} windows)",
            Self::MAX
        ))
    }

    /// Drop every window (before the FB store's shutdown, or a reset).
    pub fn clear(&self) {
        self.windows.write().unwrap_or_else(PoisonError::into_inner).clear();
        self.refused_regions.lock().unwrap_or_else(PoisonError::into_inner).clear();
    }

    pub fn read(&self) -> RwLockReadGuard<'_, Vec<RecordWindow>> {
        self.windows.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// Windows mapped and regions refused, for the summary.
    pub fn counts(&self) -> (usize, u64) {
        (self.read().len(), self.refused.load(Ordering::Relaxed))
    }

    /// Windows taken back (a page entered the BAR2 memfd or the BAR1 pool).
    pub fn revoked(&self) -> u64 {
        self.revoked.load(Ordering::Relaxed)
    }
}

/// Record I/O without the device mutex: guest memory through the view, FB records through
/// the pinned windows.
pub struct CoreIo<'a> {
    pub mem: &'a GuestMemory,
    pub pins: &'a [RecordWindow],
}

impl CoreIo<'_> {
    fn pin(&self, pa: u64, len: u64) -> Option<&RecordWindow> {
        self.pins.iter().find(|w| w.covers(pa, len))
    }
}

impl RecordIo for CoreIo<'_> {
    fn path(&self) -> &'static str {
        "clock"
    }

    fn can(&self, m: MemRef, off: u64, len: u64) -> bool {
        m.target != 1 || self.pin(m.addr + off, len).is_some()
    }

    fn read(&self, m: MemRef, off: u64, buf: &mut [u8]) -> bool {
        let pa = m.addr + off;
        if m.target != 1 {
            return self.mem.read(pa, buf);
        }
        match self.pin(pa, buf.len() as u64) {
            Some(w) => {
                w.win.read(w.at(pa), buf);
                true
            }
            None => false,
        }
    }

    fn write(&mut self, m: MemRef, off: u64, data: &[u8]) -> bool {
        let pa = m.addr + off;
        if m.target != 1 {
            return self.mem.write(pa, data);
        }
        match self.pin(pa, data.len() as u64) {
            Some(w) => {
                w.win.write(w.at(pa), data);
                // Write-combined: drained before the guest can look (its reads go through
                // other mappings of the same VRAM).
                w.win.flush();
                true
            }
            None => false,
        }
    }
}

/// The display clock's tick without the device mutex: advance the engine to now through
/// guest memory `mem` and the pinned FB record windows. Latches whose records neither
/// reaches wait for the device path. Returns whether the display interrupt line is asserted
/// afterwards.
pub fn tick(engine: &Mutex<Engine>, mem: &GuestMemory) -> bool {
    let t0 = engine::now_ns();
    let asserted = tick_at(engine, mem, t0);
    engine::ticker().note_catch_up(engine::now_ns().saturating_sub(t0) / 1000);
    asserted
}

/// [`tick`] at display-clock time `now`.
pub(crate) fn tick_at(engine: &Mutex<Engine>, mem: &GuestMemory, now: u64) -> bool {
    let mut e = engine::lock(engine);
    let pins = e.pins.clone();
    let windows = pins.read();
    e.catch_up_io(now, &mut CoreIo { mem, pins: &windows });
    e.core_ticks += 1;
    e.pending()
}

/// A guest read of an engine register (`Engine::handles`) without the device mutex: the
/// clock catches up first, as the trapped path does (`Display::read32_io`).
/// `None` when the register is not the engine's, or its status lives in the display page
/// (fe_page.rs): that read stays on the device path as before.
pub fn read32(engine: &Mutex<Engine>, mem: &GuestMemory, off: u64) -> Option<u32> {
    if !Engine::handles(off) {
        return None;
    }
    let mut e = engine::lock(engine);
    if e.paged() {
        return None;
    }
    e.routed_reads += 1;
    let pins = e.pins.clone();
    let windows = pins.read();
    let now = engine::now_ns();
    e.catch_up_io(now, &mut CoreIo { mem, pins: &windows });
    let v = e.read32(off, now);
    if let Some(v) = v {
        e.recent_reg(now, false, off, v);
    }
    v
}

/// A guest write of an engine register without the device mutex (`Display::write32`); the
/// note goes to the engine's notes for the trace.
pub fn write32(engine: &Mutex<Engine>, mem: &GuestMemory, off: u64, v: u32) -> bool {
    if !Engine::handles(off) {
        return false;
    }
    let mut e = engine::lock(engine);
    if e.paged() {
        return false;
    }
    e.routed_writes += 1;
    let pins = e.pins.clone();
    let windows = pins.read();
    let now = engine::now_ns();
    e.catch_up_io(now, &mut CoreIo { mem, pins: &windows });
    e.recent_reg(now, true, off, v);
    match e.write32(off, v) {
        Some(n) => {
            if !n.is_empty() {
                e.note(n);
            }
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A granted record window is taken back when a page of its region is about to be
    /// pooled or enter the BAR2 memfd (fb.rs `revoke_records`, bar2area.rs `apply_unit`):
    /// the display no longer reaches it, the region stays on the device path, and a page
    /// outside the region revokes nothing.
    #[test]
    fn a_record_window_is_taken_back_before_its_page_leaves_home() {
        let pins = Arc::new(RecordPins::default());
        let mut fb = crate::fb::testing::shared_ram_fb(16 << 20);
        assert!(pins.ensure(&mut fb, 0x40_0010, 16).unwrap().contains("1 window(s)"));
        fb.revoke_records(0x80_0000 / 4096, 0x80_0000 / 4096 + 1);
        assert_eq!((pins.read().len(), pins.revoked(), fb.records_revoked()), (1, 0, 0));
        // The pool is about to move page 0x41_2000 (inside [0x40_0000, 0x60_0000)).
        fb.revoke_records(0x41_2000 / 4096, 0x41_2000 / 4096 + 1);
        assert_eq!((pins.read().len(), pins.revoked(), fb.records_revoked()), (0, 1, 1));
        let w = pins.read();
        let io = CoreIo { mem: &GuestMemory::new(), pins: &w };
        assert!(!io.can(MemRef { target: 1, addr: 0x40_0010 }, 0, 16), "the device path now");
        drop(w);
        assert!(pins.ensure(&mut fb, 0x40_0010, 16).is_none(), "stays on the device path");
        fb.revoke_records(0x41_2000 / 4096, 0x41_2000 / 4096 + 1);
        assert_eq!(fb.records_revoked(), 1, "granted once, revoked once");
    }

    /// The grants over a page range are revoked and dropped, the others kept.
    #[test]
    fn revoke_grants_takes_only_the_overlapping_ones() {
        let pins = Arc::new(RecordPins::default());
        let owner: Arc<dyn crate::fb::RecordRevoke> = pins.clone();
        let g = |first: u64| crate::fb::RecordGrant {
            first,
            end: first + 512,
            owner: Arc::downgrade(&owner),
        };
        let mut grants = vec![g(0x400), g(0x600)];
        assert_eq!(crate::fb::revoke_grants(&mut grants, 0x5FF, 0x600), vec![(0x400, 0x600)]);
        assert_eq!(grants.len(), 1);
        assert_eq!(crate::fb::revoke_grants(&mut grants, 0x800, 0x900), vec![]);
        assert_eq!(grants.len(), 1);
    }

    #[test]
    fn a_record_window_maps_its_2_mib_region_once() {
        let pins = Arc::new(RecordPins::default());
        let mut fb = crate::fb::testing::shared_ram_fb(16 << 20);
        let n = pins.ensure(&mut fb, 0x50_0010, 16).unwrap();
        assert!(n.contains("0x400000") && n.contains("1 window(s)"), "{n}");
        assert!(pins.ensure(&mut fb, 0x5F_FFF0, 16).is_none(), "the same region");
        assert!(pins.ensure(&mut fb, 0x5F_FFFC, 16).is_none(), "across two regions: device path");
        let w = pins.read();
        let io = CoreIo { mem: &GuestMemory::new(), pins: &w };
        let m = MemRef { target: 1, addr: 0x50_0010 };
        assert!(io.can(m, 0, 16));
        assert!(!io.can(MemRef { target: 1, addr: 0x60_0000 }, 0, 4));
        assert!(io.can(MemRef { target: 2, addr: 0x1000 }, 0, 4), "sysmem goes through the view");
        // A write through the window is the store's: the device path reads it.
        let mut io = CoreIo { mem: io.mem, pins: &w };
        assert!(io.write(m, 4, &0xABCDu32.to_le_bytes()));
        let mut b = [0u8; 4];
        assert!(fb.read(0x50_0014, &mut b));
        assert_eq!(u32::from_le_bytes(b), 0xABCD);
        drop(w);
        // Without a shareable store the region stays on the device path, noted once.
        let mut plain = FakeFb::new(16 << 20);
        let n = pins.ensure(&mut plain, 0x80_0000, 16).unwrap();
        assert!(n.contains("stay on the device path"), "{n}");
        assert!(pins.ensure(&mut plain, 0x80_0100, 16).is_none());
        assert_eq!(pins.counts(), (1, 1));
        pins.clear();
        assert_eq!(pins.counts().0, 0);
    }
}

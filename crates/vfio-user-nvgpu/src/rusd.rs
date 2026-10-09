// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Opt-in RM user shared data (`CARAXES_RUSD=1`). This is virtual model state,
//! not a physical GPU sample: modeled clocks 1410/7000/1245/1410 MHz, P0, 40 C.
//! Only populated clock, pstate and GPU/board temperature sections are polled.
//! Utilization, ECC, power, remapping, PCI counters, fans and processes stay invalid.
//! BAR1/PMA are non-polled model facts. The sleeping publisher uses PTIMER's clock
//! and the current DMA view, with no Device lock, GPU handle or cached pointer.

use crate::guestmem::{GuestMemory, MemView};
use crate::ver::GuestAbi;
use std::sync::{
    atomic::{fence, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};

pub const ENV: &str = "CARAXES_RUSD";
pub const SEQ_START: u64 = 0xFF00_0000_0000_0000;
const MIN_INTERVAL_MS: u32 = 100;
const CLOCK: u64 = 1;
const PERF: u64 = 2;
const THERMAL: u64 = 0x10;
const SUPPORTED_MASK: u64 = CLOCK | PERF | THERMAL;

/// x86-64 cl00de.h layouts: 595/610 headers are identical; 615 adds a 32-byte
/// bank-remap section after row remap. 570 has smaller clock/throttle/ECC sections.
#[derive(Clone, Copy, Debug)]
struct Layout {
    size: usize,
    clock_stride: usize,
    pstate: usize,
    temps: usize,
    board: Option<usize>,
}

impl Layout {
    fn for_abi(abi: GuestAbi) -> Self {
        match abi {
            GuestAbi::V570 => Self {
                size: 464,
                clock_stride: 4,
                pstate: 264,
                temps: 296,
                board: None,
            },
            GuestAbi::V595 | GuestAbi::V610 => Self {
                size: 3152,
                clock_stride: 8,
                pstate: 2280,
                temps: 2312,
                board: Some(2344),
            },
            GuestAbi::V615 => Self {
                size: 3184,
                clock_stride: 8,
                pstate: 2280,
                temps: 2312,
                board: Some(2344),
            },
        }
    }
}

/// Non-polled memory facts of the virtual card, in the header's units.
#[derive(Clone, Copy, Debug)]
pub struct Facts {
    pub bar1_kib: u32,
    pub bar1_avail_kib: u32,
    pub pma_total: u64,
    pub pma_free: u64,
}

struct Page {
    pa: u64,
    gpa: u64,
    layout: Layout,
    mem: MemView,
}

struct State {
    page: Option<Page>,
    mask: u64,
    interval: Duration,
    next: Option<Instant>,
    stop: bool,
    publications: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            page: None,
            mask: 0,
            interval: Duration::from_millis(250),
            next: None,
            stop: false,
            publications: 0,
        }
    }
}

impl State {
    fn schedule(&mut self, now: Instant) {
        self.next =
            (self.page.is_some() && self.mask & SUPPORTED_MASK != 0).then(|| now + self.interval);
    }

    /// At most one publication per deadline; delays never cause catch-up bursts.
    fn due(&mut self, now: Instant, now_ns: u64) {
        if !self.next.is_some_and(|deadline| now >= deadline) {
            return;
        }
        let ok = self.page.as_ref().is_some_and(|p| {
            // Hold the CURRENT view lock through the writes: DMA republishing
            // cannot pass a publication to a mapping that has been retired.
            let current = p
                .mem
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            current.translate(p.pa, p.layout.size as u64) == Some(p.gpa)
                && current.covers(p.pa, p.layout.size as u64)
                && publish(&current, p.pa, p.layout, self.mask, now_ns)
        });
        if ok {
            self.publications = self.publications.saturating_add(1);
        } else {
            self.page = None;
        }
        self.schedule(now);
    }
}

pub struct Rusd {
    enabled: bool,
    shared: Arc<(Mutex<State>, Condvar)>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Default for Rusd {
    fn default() -> Self {
        Self {
            enabled: std::env::var(ENV).is_ok_and(|v| v == "1"),
            shared: Arc::new((Mutex::new(State::default()), Condvar::new())),
            worker: None,
        }
    }
}

impl Rusd {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn pa(&self) -> Option<u64> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .page
            .as_ref()
            .map(|p| p.pa)
    }

    /// 570 carries only physAddr (0 detaches); newer ABIs also carry bInit.
    pub fn decode_init(abi: GuestAbi, params: &[u8]) -> Result<(bool, u64), String> {
        let off = if abi == GuestAbi::V570 { 0 } else { 8 };
        let bytes = params
            .get(off..off + 8)
            .ok_or("truncated RUSD init parameters")?;
        let pa = u64::from_le_bytes(bytes.try_into().expect("eight bytes"));
        let init = if abi == GuestAbi::V570 {
            pa != 0
        } else {
            params[0] != 0
        };
        Ok((init, pa))
    }

    pub fn init(
        &mut self,
        mem: &GuestMemory,
        abi: GuestAbi,
        init: bool,
        pa: u64,
        f: &Facts,
    ) -> Result<String, String> {
        let (lock, changed) = &*self.shared;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !init {
            state.page = None;
            state.next = None;
            state.mask = 0;
            changed.notify_all();
            return Ok(format!(
                "rusd: stopped after {} publications",
                state.publications
            ));
        }
        let layout = Layout::for_abi(abi);
        if pa == 0
            || pa & 7 != 0
            || pa.checked_add(layout.size as u64).is_none()
            || !mem.covers(pa, layout.size as u64)
        {
            return Err("RUSD page is unaligned, overflows, or is not fully mapped".into());
        }
        let view = mem
            .published_view()
            .ok_or("RUSD current memory view was not prepared")?;
        let gpa = mem
            .translate(pa, layout.size as u64)
            .ok_or("RUSD page has no DMA translation")?;
        // Serializing replacement against the publisher retires the old page first.
        state.page = None;
        state.next = None;
        if !mem.write(pa, &vec![0u8; layout.size]) {
            return Err("RUSD initialization lost its mapping".into());
        }
        let mut bar1 = Vec::with_capacity(8);
        bar1.extend_from_slice(&f.bar1_kib.to_le_bytes());
        bar1.extend_from_slice(&f.bar1_avail_kib.to_le_bytes());
        let mut pma = Vec::with_capacity(16);
        pma.extend_from_slice(&f.pma_total.to_le_bytes());
        pma.extend_from_slice(&f.pma_free.to_le_bytes());
        if !section(mem, pa, &bar1, SEQ_START + 2) || !section(mem, pa + 16, &pma, SEQ_START + 2) {
            return Err("RUSD memory facts could not be published".into());
        }
        state.page = Some(Page {
            pa,
            gpa,
            layout,
            mem: view,
        });
        state.schedule(Instant::now());
        changed.notify_all();
        drop(state);
        if self.worker.is_none() {
            let shared = self.shared.clone();
            match std::thread::Builder::new()
                .name("rusd-publisher".into())
                .spawn(move || run(shared))
            {
                Ok(worker) => self.worker = Some(worker),
                Err(error) => {
                    self.stop_page();
                    return Err(format!("RUSD publisher could not start: {error}"));
                }
            }
        }
        Ok(format!(
            "rusd: virtual model page at {pa:#x} ({} bytes, ABI {})",
            layout.size,
            abi.number()
        ))
    }

    pub fn set_poll(&mut self, mask: u64, interval_ms: u32) -> Result<(), String> {
        // Kernel RM resolves client-facing 0-to-default before this internal call.
        if interval_ms < MIN_INTERVAL_MS {
            return Err(format!(
                "RUSD internal interval {interval_ms} ms is below {MIN_INTERVAL_MS} ms"
            ));
        }
        let (lock, changed) = &*self.shared;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.mask = mask;
        state.interval = Duration::from_millis(interval_ms.into());
        state.schedule(Instant::now());
        changed.notify_all();
        Ok(())
    }

    fn stop_page(&mut self) {
        let (lock, changed) = &*self.shared;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.page = None;
        state.next = None;
        changed.notify_all();
    }

    /// Device calls this BEFORE removing RAM or translation. Later remapping at
    /// the same GPA must not revive the old registration.
    pub fn retire_unmap(&mut self, mem: &GuestMemory, address: u64, size: u64) {
        let (lock, changed) = &*self.shared;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let overlaps = state.page.as_ref().is_some_and(|p| {
            mem.translate(p.pa, p.layout.size as u64) != Some(p.gpa)
                || (p.gpa < address.saturating_add(size)
                    && address < p.gpa.saturating_add(p.layout.size as u64))
        });
        if overlaps {
            state.page = None;
            state.next = None;
            changed.notify_all();
        }
    }
}

impl Drop for Rusd {
    fn drop(&mut self) {
        {
            let (lock, changed) = &*self.shared;
            let mut state = lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.stop = true;
            state.page = None;
            state.next = None;
            changed.notify_all();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(shared: Arc<(Mutex<State>, Condvar)>) {
    let (lock, changed) = &*shared;
    let mut state = lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    while !state.stop {
        match state.next {
            None => {
                state = changed
                    .wait(state)
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
            }
            Some(deadline) => {
                let now = Instant::now();
                if now >= deadline {
                    state.due(now, crate::channel::device_now_ns());
                } else {
                    state = changed
                        .wait_timeout(state, deadline - now)
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .0;
                }
            }
        }
    }
}

fn section(mem: &GuestMemory, pa: u64, data: &[u8], marker: u64) -> bool {
    if !mem.write_u64_release(pa, u64::MAX) {
        return false;
    }
    // Publish invalidation before any payload change; release publishes the result.
    fence(Ordering::SeqCst);
    mem.write(pa + 8, data) && mem.write_u64_release(pa, marker)
}

fn publish(mem: &GuestMemory, pa: u64, layout: Layout, mask: u64, now_ns: u64) -> bool {
    let ts = now_ns.clamp(1, SEQ_START - 1);
    if mask & CLOCK != 0 {
        let mut clocks = [0u8; 32];
        for (i, mhz) in [1410u32, 7000, 1245, 1410].into_iter().enumerate() {
            let off = i * layout.clock_stride;
            clocks[off..off + 4].copy_from_slice(&mhz.to_le_bytes());
            if layout.clock_stride == 8 {
                clocks[off + 4..off + 8].copy_from_slice(&(mhz * 1000).to_le_bytes());
            }
        }
        if !section(mem, pa + 72, &clocks[..4 * layout.clock_stride], ts) {
            return false;
        }
    }
    // PERF_GET_CURRENT_PSTATE uses NV2080_CTRL_PERF_PSTATES_P0 = 1 (a bitmask).
    if mask & PERF != 0 && !section(mem, pa + layout.pstate as u64, &1u32.to_le_bytes(), ts) {
        return false;
    }
    if mask & THERMAL != 0 {
        let temp = (40u32 << 8).to_le_bytes();
        if !section(mem, pa + layout.temps as u64, &temp, ts) {
            return false;
        }
        if let Some(board) = layout.board {
            if !section(mem, pa + board as u64, &temp, ts) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn memory() -> GuestMemory {
        let mut mem = GuestMemory::new();
        mem.view();
        map_memory(&mut mem);
        mem
    }

    fn map_memory(mem: &mut GuestMemory) {
        let path = std::env::temp_dir().join(format!(
            "nvgpu-rusd-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(&[0u8; 0x2000]).unwrap();
        mem.map(0x10_0000, 0x2000, 0, &file).unwrap();
        std::fs::remove_file(path).unwrap();
    }

    fn facts() -> Facts {
        Facts {
            bar1_kib: 0x4_0000,
            bar1_avail_kib: 0x3_9f80,
            pma_total: 1 << 30,
            pma_free: 1 << 29,
        }
    }

    #[test]
    fn abi_layout_and_init_frames_match_headers() {
        for (abi, size, pstate, temps) in [
            (GuestAbi::V570, 464, 264, 296),
            (GuestAbi::V595, 3152, 2280, 2312),
            (GuestAbi::V610, 3152, 2280, 2312),
            (GuestAbi::V615, 3184, 2280, 2312),
        ] {
            let l = Layout::for_abi(abi);
            assert_eq!((l.size, l.pstate, l.temps), (size, pstate, temps));
            let mut p = vec![0u8; if abi == GuestAbi::V570 { 8 } else { 16 }];
            let off = if abi == GuestAbi::V570 { 0 } else { 8 };
            p[off..off + 8].copy_from_slice(&0x10_0000u64.to_le_bytes());
            if abi != GuestAbi::V570 {
                p[0] = 1;
            }
            assert_eq!(Rusd::decode_init(abi, &p).unwrap(), (true, 0x10_0000));
            if abi != GuestAbi::V570 {
                p[0] = 0;
                assert_eq!(Rusd::decode_init(abi, &p).unwrap(), (false, 0x10_0000));
            }
            assert!(Rusd::decode_init(abi, &p[..off + 7]).is_err());
        }
    }

    #[test]
    fn requested_sections_and_deadlines_are_bounded() {
        let mem = memory();
        let base = 0x10_0000;
        for abi in GuestAbi::ALL {
            let mut r = Rusd::default();
            r.init(&mem, abi, true, base, &facts()).unwrap();
            let l = Layout::for_abi(abi);
            let now = Instant::now();
            let mut s = r.shared.0.lock().unwrap();
            s.mask = CLOCK;
            s.schedule(now);
            s.due(now + Duration::from_millis(249), 5_000_000);
            assert_eq!(mem.read_u64(base + 72), Some(0));
            s.due(now + Duration::from_secs(5), 7_000_000);
            assert_eq!(s.publications, 1, "no catch-up burst");
            assert_eq!(s.next, Some(now + Duration::from_millis(5250)));
            assert_eq!(mem.read_u64(base + 72), Some(7_000_000));
            assert_eq!(mem.read_u64(base + l.pstate as u64), Some(0));
            assert_eq!(mem.read_u64(base + l.temps as u64), Some(0));
            s.mask = PERF | THERMAL;
            s.schedule(now);
            s.due(now + Duration::from_millis(250), 9_000_000);
            assert_eq!(mem.read_u32(base + l.pstate as u64 + 8), Some(1));
            assert_eq!(mem.read_u32(base + l.temps as u64 + 8), Some(40 << 8));
            assert_eq!(mem.read_u64(base + 72), Some(7_000_000));
            assert_eq!(
                mem.read_u64(base + if abi == GuestAbi::V570 { 96 } else { 112 }),
                Some(0),
                "unsupported throttle"
            );
            s.mask = 0;
            s.schedule(now);
            assert_eq!(s.next, None);
            s.mask = 4 | 8 | 0x20 | 0x40 | 0x80;
            s.schedule(now);
            assert_eq!(s.next, None, "unsupported-only masks need no timer");
        }
    }

    #[test]
    fn invalid_requests_preserve_page_and_schedule() {
        let mem = memory();
        let mut r = Rusd::default();
        r.init(&mem, GuestAbi::V615, true, 0x10_0000, &facts())
            .unwrap();
        r.set_poll(CLOCK, 100).unwrap();
        let deadline = r.shared.0.lock().unwrap().next;
        for interval in [0, 1, 99] {
            assert!(r.set_poll(0, interval).is_err());
        }
        assert_eq!(r.shared.0.lock().unwrap().next, deadline);
        for pa in [0, 0x10_0001, u64::MAX - 7, 0x30_0000] {
            assert!(r.init(&mem, GuestAbi::V615, true, pa, &facts()).is_err());
            assert_eq!(r.pa(), Some(0x10_0000));
        }
        r.set_poll(u64::MAX, u32::MAX).unwrap();
        r.set_poll(0, 100).unwrap();
        assert_eq!(r.shared.0.lock().unwrap().next, None);
    }

    #[test]
    fn independent_publisher_replaces_and_stops_pages() {
        let mem = memory();
        let mut r = Rusd::default();
        let shared = r.shared.clone();
        let read = |pa: u64| {
            let _state = shared.0.lock().unwrap();
            mem.read_u64(pa + 72).unwrap()
        };
        let before = crate::channel::device_now_ns();
        r.init(&mem, GuestAbi::V615, true, 0x10_0000, &facts())
            .unwrap();
        r.set_poll(CLOCK, 100).unwrap();
        let limit = Instant::now() + Duration::from_secs(2);
        while read(0x10_0000) == 0 && Instant::now() < limit {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(read(0x10_0000) > 0, "no RPC or Device lock needed");
        assert!(read(0x10_0000) >= before && read(0x10_0000) <= crate::channel::device_now_ns());
        r.set_poll(0, 100).unwrap();
        let paused = read(0x10_0000);
        std::thread::sleep(Duration::from_millis(120));
        assert_eq!(read(0x10_0000), paused);
        r.set_poll(CLOCK, 100).unwrap();
        r.init(&mem, GuestAbi::V615, true, 0x10_1000, &facts())
            .unwrap();
        let old = read(0x10_0000);
        let limit = Instant::now() + Duration::from_secs(2);
        while read(0x10_1000) == 0 && Instant::now() < limit {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(read(0x10_1000) > 0);
        assert_eq!(read(0x10_0000), old);
        r.init(&mem, GuestAbi::V615, false, 0x10_1000, &facts())
            .unwrap();
        let stopped = read(0x10_1000);
        std::thread::sleep(Duration::from_millis(120));
        assert_eq!(read(0x10_1000), stopped);
        assert_eq!(r.pa(), None);
        r.init(&mem, GuestAbi::V615, true, 0x10_1000, &facts())
            .unwrap();
        r.set_poll(CLOCK, 100).unwrap();
        drop(r);
        let dropped = read(0x10_1000);
        std::thread::sleep(Duration::from_millis(120));
        assert_eq!(
            read(0x10_1000),
            dropped,
            "reset/Drop joins the old publisher"
        );
    }

    #[test]
    fn unmap_retirement_prevents_same_address_revival() {
        let mut mem = memory();
        let mut r = Rusd::default();
        r.init(&mem, GuestAbi::V615, true, 0x10_0000, &facts())
            .unwrap();
        r.set_poll(CLOCK, 100).unwrap();
        r.retire_unmap(&mem, 0x20_0000, 0x1000);
        assert_eq!(r.pa(), Some(0x10_0000));
        r.retire_unmap(&mem, 0x10_0000, 0x2000);
        mem.unmap(0x10_0000, 0x2000);
        map_memory(&mut mem);
        let mut s = r.shared.0.lock().unwrap();
        s.due(Instant::now() + Duration::from_secs(1), 12_000_000);
        assert!(s.page.is_none());
        assert_eq!(mem.read_u64(0x10_0000 + 72), Some(0));
    }

    #[test]
    fn failed_current_coverage_retires_page() {
        let mut mem = memory();
        let mut r = Rusd::default();
        r.init(&mem, GuestAbi::V615, true, 0x10_0000, &facts())
            .unwrap();
        r.set_poll(CLOCK, 100).unwrap();
        mem.unmap(0x10_0000, 0x2000);
        let mut s = r.shared.0.lock().unwrap();
        s.due(Instant::now() + Duration::from_secs(1), 12_000_000);
        assert!(s.page.is_none());
        assert_eq!(s.next, None);
    }

    #[test]
    fn changed_dma_translation_cannot_redirect_publication() {
        use crate::dmamap::{DmaRange, DmaTable};
        let mut mem = memory();
        let mut r = Rusd::default();
        r.init(&mem, GuestAbi::V615, true, 0x10_0000, &facts())
            .unwrap();
        r.set_poll(CLOCK, 100).unwrap();
        mem.set_table(Some(
            DmaTable::for_gpa_guest(vec![DmaRange {
                gpa: 0x10_1000,
                iova: 0x10_0000,
                size: 0x1000,
            }])
            .unwrap(),
        ));
        let mut s = r.shared.0.lock().unwrap();
        s.due(Instant::now() + Duration::from_secs(1), 12_000_000);
        assert!(s.page.is_none());
        assert_eq!(s.next, None);
        assert_eq!(
            mem.read_u64(0x10_0000 + 72),
            Some(0),
            "new translation's RAM stays untouched"
        );
    }
}

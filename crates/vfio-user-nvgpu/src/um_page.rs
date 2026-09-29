// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The usermode page as memory (`CARAXES_USERMODE_PAGE=1`, docs/caraxes/intr-page.md §12.2).
//!
//! BAR0 page 0xBB0000 of TU106 holds the usermode window the guest maps into its clients
//! (class VOLTA_USERMODE_A): `NV_VIRTUAL_FUNCTION_TIME_0/1` (0xBB0080/84), the PTIMER
//! nanoseconds the GL/Vulkan stack reads (about 925 reads per second under Mutter, series
//! D to I), and `NOTIFY_CHANNEL_PENDING` (0xBB0090), the doorbell. The page cannot be split,
//! so with the switch the whole page is a sparse area of the interrupt page's memfd and the
//! reactor owns it:
//!
//! - The clock: the reactor stores TIME_0 and TIME_1 as one aligned 64-bit store, at most
//!   once per [`TIME_EVERY`] ([`UmPage::publish_bounded`]: a spinning reactor would otherwise
//!   store every iteration and bounce the cache line with every guest reader). The guest reads TIME_1, TIME_0, TIME_1 and
//!   retries while TIME_1 changed (`ptimerReadTimer`, the GPU's own protocol); two 32-bit
//!   stores would let it see a torn pair at a TIME_0 wrap (4.3 s apart), the one 64-bit
//!   store cannot. While the reactor blocks (at most its sleep bound, 1 ms) the clock
//!   stands: a reader sees time that does not advance for up to that long, never time
//!   that goes back.
//! - The doorbell: a guest write lands in memory. The reactor takes the word
//!   ([`UmPage::take_doorbell`]: compare-and-swap to [`SENTINEL`]) and rings the channel its
//!   value names at once, as a trapped write of that value would (boot.rs
//!   `doorbell_from_page`): O(1) on the critical path, not a read of every channel's GPPut.
//!   The value is still only a hint: several writes before the reactor looks leave only the
//!   last. So [`FOLLOW_UP`] later the reactor scans the GPPut of every channel the device
//!   knows (ctrl.rs `doorbell_pending`) and rings each that advanced unrung; the guest writes
//!   GPPut before the doorbell and the scan runs after the swap, so a channel is rung only
//!   with a GPPut that is visible. A watchdog scan every [`WATCHDOG`] catches a doorbell
//!   whose write the swap could not tell apart (the guest storing the sentinel itself). Both
//!   scans count each channel they find pending.
//!
//! Fail closed: the three registers must share the page with TIME_0/TIME_1 as one aligned
//! qword, no other register model may answer in the page ([`UmPage::check`]), and the
//! switch needs the interrupt page (its memfd and reactor) and the boot model. A store to
//! TIME_0/1 is overwritten at the next publish and counted as an inconsistency.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::intr_page::{IntrPage, Mapping, PAGE};

/// The switch.
pub const USERMODE_PAGE_ENV: &str = "CARAXES_USERMODE_PAGE";
/// The doorbell word between guest writes: no `NV_CTRL_VF_DOORBELL` value (VECTOR 11:0,
/// RUNLIST_ID 22:16, tu102/dev_ctrl.h:36-37) and no pool token (tokens.rs) has all bits
/// set.
pub const SENTINEL: u32 = u32::MAX;
/// The watchdog scan's period.
pub const WATCHDOG: Duration = Duration::from_millis(10);
/// The scan after a doorbell, for the writes its hint did not name (coalesced).
pub const FOLLOW_UP: Duration = Duration::from_micros(100);
/// The clock is published at most this often: readers see the device clock within it.
pub const TIME_EVERY: Duration = Duration::from_micros(1);

/// `CARAXES_USERMODE_PAGE`: off, or on with the interrupt page and the boot model.
pub fn requested(intr_page: bool, boot_model: bool) -> Result<bool, String> {
    parse(
        std::env::var(USERMODE_PAGE_ENV).ok().as_deref(),
        intr_page,
        boot_model,
    )
}

pub fn parse(v: Option<&str>, intr_page: bool, boot_model: bool) -> Result<bool, String> {
    let on = match v.map(str::trim).filter(|v| !v.is_empty()).unwrap_or("0") {
        "0" | "off" => false,
        "1" | "on" => true,
        v => return Err(format!("{USERMODE_PAGE_ENV}={v:?}: 0 or 1")),
    };
    if on && !intr_page {
        return Err(format!(
            "{USERMODE_PAGE_ENV}=1 needs CARAXES_INTR_PAGE=1 (its memfd and its reactor)"
        ));
    }
    if on && !boot_model {
        return Err(format!(
            "{USERMODE_PAGE_ENV}=1 needs the boot model (it rings the channels)"
        ));
    }
    Ok(on)
}

/// Where the words are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub base: u64,
    pub time_0: u64,
    pub doorbell: u64,
}

impl Layout {
    /// From the chip's registers; refused unless TIME_0/TIME_1 form one aligned qword and
    /// the doorbell shares their page.
    pub fn of(r: &crate::chip::Registers) -> Result<Self, String> {
        let base = r.usermode_doorbell & !(PAGE - 1);
        if r.usermode_time_0 % 8 != 0 || r.usermode_time_1 != r.usermode_time_0 + 4 {
            return Err(format!(
                "TIME_0 {:#x} and TIME_1 {:#x} are not one aligned qword",
                r.usermode_time_0, r.usermode_time_1
            ));
        }
        if r.usermode_time_0 & !(PAGE - 1) != base {
            return Err(format!(
                "TIME_0 {:#x} and the doorbell {:#x} do not share one page",
                r.usermode_time_0, r.usermode_doorbell
            ));
        }
        Ok(Layout {
            base,
            time_0: r.usermode_time_0,
            doorbell: r.usermode_doorbell,
        })
    }
}

#[derive(Default, Debug)]
pub struct Stats {
    pub publishes: AtomicU64,
    /// Doorbell words taken by the reactor.
    pub doorbells: AtomicU64,
    /// Scans run for them and channels they rang.
    pub scans: AtomicU64,
    pub rung: AtomicU64,
    /// From the doorbell word's change being seen to the scan done, microseconds.
    pub scan_us_sum: AtomicU64,
    pub scan_us_max: AtomicU64,
    /// Watchdog scans, and channels they found pending (a doorbell nobody saw).
    pub watchdog_scans: AtomicU64,
    pub watchdog_rung: AtomicU64,
    /// Follow-up scans after a doorbell, and channels they found pending (a doorbell write
    /// the hint did not name).
    pub follow_ups: AtomicU64,
    pub follow_up_rung: AtomicU64,
    /// Guest stores to TIME_0/1 (overwritten) and of the sentinel to the doorbell.
    pub time_stores: AtomicU64,
    pub sentinel_stores: AtomicU64,
}

pub struct UmPage {
    layout: Layout,
    map: Mapping,
    start: Instant,
    pub stats: Stats,
    last_watchdog: std::sync::Mutex<Instant>,
    /// When the follow-up scan of the last doorbell is due (`None`: none owed).
    follow_up: std::sync::Mutex<Option<Instant>>,
    /// Nanoseconds since `start` of the last clock publish (the bound's reference).
    last_publish_ns: AtomicU64,
}

/// TIME_0 NSEC 31:5, TIME_1 NSEC 28:0 (uvm.rs `VfRegs::read32`), as the qword at TIME_0.
pub fn time_qword(ns: u64) -> u64 {
    u64::from(ns as u32 & !0x1F) | (u64::from((ns >> 32) as u32 & 0x1FFF_FFFF) << 32)
}

impl UmPage {
    /// Map the page from the interrupt page's memfd. `start` is the device clock's epoch
    /// (channel.rs `Registry::now_ns`), so the page shows the time the trapped path showed.
    pub fn create(intr: &IntrPage, layout: Layout, start: Instant) -> Result<Arc<Self>, String> {
        let map = intr.map_page(layout.base)?;
        let p = UmPage {
            layout,
            map,
            start,
            stats: Stats::default(),
            last_watchdog: std::sync::Mutex::new(Instant::now()),
            follow_up: std::sync::Mutex::new(None),
            last_publish_ns: AtomicU64::new(0),
        };
        p.reset();
        Ok(Arc::new(p))
    }

    /// Every register model answering a dword of the page must be one of the three words.
    pub fn check(&self, owns: &dyn Fn(u64) -> bool) -> Result<(), String> {
        let l = self.layout;
        for off in (l.base..l.base + PAGE).step_by(4) {
            let ours = off == l.time_0 || off == l.time_0 + 4 || off == l.doorbell;
            if owns(off) && !ours {
                return Err(format!(
                    "BAR0 {off:#x} has a register model the usermode page does not take over"
                ));
            }
        }
        Ok(())
    }

    pub fn area(&self) -> (u64, u64) {
        (self.layout.base, PAGE)
    }

    pub fn covers(&self, off: u64) -> bool {
        (self.layout.base..self.layout.base + PAGE).contains(&off)
    }

    fn word(&self, off: u64) -> &AtomicU32 {
        self.map.word((off & !3) - self.layout.base)
    }

    fn time(&self) -> &AtomicU64 {
        self.map.qword(self.layout.time_0 - self.layout.base)
    }

    pub fn reset(&self) {
        for off in (0..PAGE).step_by(4) {
            self.map.word(off).store(0, Ordering::Relaxed);
        }
        self.word(self.layout.doorbell)
            .store(SENTINEL, Ordering::Relaxed);
        self.publish_time(Instant::now());
        std::sync::atomic::fence(Ordering::SeqCst);
    }

    /// Store the clock at `now` as one qword. A guest store to either word is found here
    /// (the page held something other than the last publish) and counted.
    pub fn publish_time(&self, now: Instant) {
        let v = time_qword(now.saturating_duration_since(self.start).as_nanos() as u64);
        let t = self.time();
        let last = self.stats.publishes.load(Ordering::Relaxed);
        let old = t.swap(v, Ordering::Release);
        // A value above the new one (the clock never goes back) or with the low five
        // bits set was not ours.
        if last > 0 && (old > v || old & 0x1F != 0) {
            self.stats.time_stores.fetch_add(1, Ordering::Relaxed);
        }
        self.stats.publishes.fetch_add(1, Ordering::Relaxed);
    }

    /// [`Self::publish_time`] unless the last publish is younger than [`TIME_EVERY`].
    /// Returns whether it published.
    pub fn publish_bounded(&self, now: Instant) -> bool {
        let ns = now.saturating_duration_since(self.start).as_nanos() as u64;
        let last = self.last_publish_ns.load(Ordering::Relaxed);
        if last != 0 && ns.saturating_sub(last) < TIME_EVERY.as_nanos() as u64 {
            return false;
        }
        self.last_publish_ns.store(ns.max(1), Ordering::Relaxed);
        self.publish_time(now);
        true
    }

    /// A trapped read (a client that did not map the page): the clock as of now.
    pub fn read32(&self, off: u64) -> u32 {
        if off & !7 == self.layout.time_0 {
            self.publish_time(Instant::now());
        }
        self.word(off).load(Ordering::Acquire)
    }

    /// A trapped write: stored as the guest would store it, found by the next scan.
    pub fn write(&self, off: u64, data: &[u8]) {
        for (i, &b) in data.iter().enumerate() {
            let o = off + i as u64;
            let shift = (o & 3) * 8;
            let w = self.word(o);
            let mut cur = w.load(Ordering::Acquire);
            loop {
                let new = (cur & !(0xFF << shift)) | (u32::from(b) << shift);
                match w.compare_exchange(cur, new, Ordering::AcqRel, Ordering::Acquire) {
                    Ok(_) => break,
                    Err(c) => cur = c,
                }
            }
        }
        if off & !7 == self.layout.time_0 {
            self.stats.time_stores.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Take a doorbell the guest wrote since the last take: its value, the word back at
    /// [`SENTINEL`]. The scan that follows must run after this returns.
    pub fn take_doorbell(&self) -> Option<u32> {
        let w = self.word(self.layout.doorbell);
        let mut v = w.load(Ordering::Acquire);
        while v != SENTINEL {
            match w.compare_exchange(v, SENTINEL, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => {
                    self.stats.doorbells.fetch_add(1, Ordering::Relaxed);
                    return Some(v);
                }
                // The guest wrote again meanwhile: take the newer value.
                Err(c) => v = c,
            }
        }
        None
    }

    /// Whether the watchdog's period ran out (and restart it).
    fn watchdog_due(&self, now: Instant) -> bool {
        let mut last = self.last_watchdog.lock().unwrap_or_else(|p| p.into_inner());
        if now.saturating_duration_since(*last) >= WATCHDOG {
            *last = now;
            true
        } else {
            false
        }
    }

    pub fn summary(&self) -> String {
        let s = &self.stats;
        let l = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let mut odd = Vec::new();
        if l(&s.time_stores) > 0 {
            odd.push(format!(
                "{} guest store(s) to TIME_0/1 overwritten",
                l(&s.time_stores)
            ));
        }
        if l(&s.sentinel_stores) > 0 {
            odd.push(format!(
                "{} doorbell(s) found only by the watchdog",
                l(&s.sentinel_stores)
            ));
        }
        let scans = l(&s.scans);
        format!(
            "usermode page: BAR0 [{:#x}, {:#x}); {} clock publish(es); {} doorbell word(s) taken, {} ring(s) of the hinted channel ({} rung), ring mean {} us, max {} us; follow-up scans: {} ({} channel(s) a hint did not name); watchdog: {} scan(s), {} channel(s) found pending without a doorbell seen; inconsistencies: {}\n",
            self.layout.base,
            self.layout.base + PAGE,
            l(&s.publishes),
            l(&s.doorbells),
            scans,
            l(&s.rung),
            l(&s.scan_us_sum).checked_div(scans).unwrap_or(0),
            l(&s.scan_us_max),
            l(&s.follow_ups),
            l(&s.follow_up_rung),
            l(&s.watchdog_scans),
            l(&s.watchdog_rung),
            if odd.is_empty() { "none".into() } else { odd.join(", ") }
        )
    }
}

/// The channels a token-less scan rang, from its first note (ctrl.rs `doorbell_pending`:
/// "doorbell (ioeventfd, no token): N of M recorded channel(s) pending ...").
pub fn pending_count(note: &str) -> usize {
    note.split(": ")
        .nth(1)
        .and_then(|t| t.split(' ').next())
        .and_then(|t| t.parse().ok())
        .unwrap_or(0)
}

/// What the reactor runs (main.rs: lock the device, `Device::doorbell_from_page`):
/// `Some(value)` rings the channel the doorbell value names, `None` scans every channel
/// and rings those pending. Returns the number of channels it rang.
pub type Driver = dyn Fn(Option<u32>) -> usize + Send + Sync;

/// The reactor's side, every iteration: publish the clock (bounded), take a doorbell and
/// ring its hint, the follow-up scan when due, and the watchdog's scan. Returns whether a
/// doorbell was served (work).
pub fn poll(page: &UmPage, driver: &dyn Fn(Option<u32>) -> usize) -> bool {
    let now = Instant::now();
    page.publish_bounded(now);
    if let Some(v) = page.take_doorbell() {
        let rung = driver(Some(v)) as u64;
        let us = now.elapsed().as_micros() as u64;
        let s = &page.stats;
        s.scans.fetch_add(1, Ordering::Relaxed);
        s.rung.fetch_add(rung, Ordering::Relaxed);
        s.scan_us_sum.fetch_add(us, Ordering::Relaxed);
        s.scan_us_max.fetch_max(us, Ordering::Relaxed);
        // The writes this hint did not name are found by the follow-up scan.
        *page.follow_up.lock().unwrap_or_else(|p| p.into_inner()) =
            Some(Instant::now() + FOLLOW_UP);
        return true;
    }
    let follow = {
        let mut f = page.follow_up.lock().unwrap_or_else(|p| p.into_inner());
        match *f {
            Some(at) if now >= at => {
                *f = None;
                true
            }
            _ => false,
        }
    };
    if follow {
        let rung = driver(None) as u64;
        page.stats.follow_ups.fetch_add(1, Ordering::Relaxed);
        page.stats.follow_up_rung.fetch_add(rung, Ordering::Relaxed);
        // Restart the watchdog: a scan just ran.
        *page.last_watchdog.lock().unwrap_or_else(|p| p.into_inner()) = Instant::now();
        return rung > 0;
    }
    if page.watchdog_due(now) {
        let rung = driver(None) as u64;
        page.stats.watchdog_scans.fetch_add(1, Ordering::Relaxed);
        if rung > 0 {
            page.stats.watchdog_rung.fetch_add(rung, Ordering::Relaxed);
            page.stats.sentinel_stores.fetch_add(1, Ordering::Relaxed);
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intr::IntrShared;

    fn intr() -> Arc<IntrPage> {
        let chip = crate::fixture::rtx2070().chip;
        let d = chip.data();
        IntrPage::create(
            16 << 20,
            d.registers.intr_tree,
            d.identity,
            IntrShared::new(),
            crate::intr_page::Config::default(),
        )
        .unwrap()
    }

    fn layout() -> Layout {
        Layout::of(&crate::fixture::rtx2070().chip.data().registers).unwrap()
    }

    #[test]
    fn switch_needs_the_interrupt_page_and_the_boot_model() {
        assert_eq!(parse(None, false, false), Ok(false));
        assert_eq!(parse(Some("1"), true, true), Ok(true));
        assert!(parse(Some("1"), false, true).is_err());
        assert!(parse(Some("1"), true, false).is_err());
        assert!(parse(Some("fast"), true, true).is_err());
    }

    /// TU106: the page is 0xBB0000 with TIME at 0x80 and the doorbell at 0x90; the boot
    /// model owns nothing else in it; a qword that is not aligned is refused.
    #[test]
    fn tu106_layout_and_check() {
        let l = layout();
        assert_eq!(
            l,
            Layout {
                base: 0xBB_0000,
                time_0: 0xBB_0080,
                doorbell: 0xBB_0090
            }
        );
        let p = UmPage::create(&intr(), l, Instant::now()).unwrap();
        let f = crate::fixture::rtx2070();
        let model = crate::boot::BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(4 << 30),
            f.identity(4 << 30),
        );
        assert_eq!(p.check(&|off| model.owns_register(off)), Ok(()));
        assert!(p.check(&|off| off == 0xBB_0100).is_err());
        let mut r = f.chip.data().registers;
        r.usermode_time_0 = 0xBB_0084;
        r.usermode_time_1 = 0xBB_0088;
        assert!(Layout::of(&r).is_err());
        assert_eq!(p.area(), (0xBB_0000, PAGE));
    }

    /// The clock: one qword, the trapped model's format, never back; the trapped read
    /// shows the time of now.
    #[test]
    fn the_clock_is_one_qword_and_never_goes_back() {
        assert_eq!(time_qword(0x1_2345_6789), 0x1_2345_6780);
        assert_eq!(time_qword(0xFFFF_FFFF_FFFF_FFFF), 0x1FFF_FFFF_FFFF_FFE0);
        let start = Instant::now() - Duration::from_secs(5);
        let p = UmPage::create(&intr(), layout(), start).unwrap();
        let a = p.time().load(Ordering::Acquire);
        // Five seconds past the epoch: TIME_1 is 1 (the low word wrapped once).
        assert_eq!(a >> 32, 1);
        std::thread::sleep(Duration::from_millis(2));
        let lo = p.read32(0xBB_0080);
        let hi = p.read32(0xBB_0084);
        let b = (u64::from(hi) << 32) | u64::from(lo);
        assert!(b > a && b & 0x1F == 0);
        assert!(p.summary().contains("inconsistencies: none"));
        // A guest store to TIME is overwritten and counted.
        p.write(0xBB_0080, &7u32.to_le_bytes());
        p.publish_time(Instant::now());
        assert_eq!(p.read32(0xBB_0080) & 0x1F, 0);
        assert!(p
            .summary()
            .contains("guest store(s) to TIME_0/1 overwritten"));
    }

    /// The doorbell: any number of writes before a take is one take, the value is only
    /// the last one and rings its channel at once, the same value twice after a take is a
    /// new doorbell; the follow-up scan [`FOLLOW_UP`] later finds what the hint did not
    /// name; a channel the watchdog finds pending is counted.
    #[test]
    fn a_doorbell_rings_its_hint_and_the_follow_up_scan_finds_the_rest() {
        let p = UmPage::create(&intr(), layout(), Instant::now()).unwrap();
        let calls = std::sync::Mutex::new(Vec::new());
        let driver = |v: Option<u32>| {
            calls.lock().unwrap().push(v);
            2usize
        };
        assert!(!poll(&p, &driver));
        p.write(0xBB_0090, &5u32.to_le_bytes());
        p.write(0xBB_0090, &7u32.to_le_bytes());
        assert!(poll(&p, &driver));
        assert_eq!(calls.lock().unwrap().as_slice(), &[Some(7)]);
        assert_eq!(p.read32(0xBB_0090), SENTINEL);
        // The same value again is seen: the take left the sentinel.
        p.write(0xBB_0090, &7u32.to_le_bytes());
        assert!(poll(&p, &driver));
        assert_eq!(calls.lock().unwrap().len(), 2);
        assert_eq!(p.stats.doorbells.load(Ordering::Relaxed), 2);
        assert_eq!(p.stats.rung.load(Ordering::Relaxed), 4);
        // The follow-up scan: not before FOLLOW_UP, once after it.
        poll(&p, &driver);
        assert_eq!(calls.lock().unwrap().len(), 2);
        std::thread::sleep(FOLLOW_UP + Duration::from_micros(50));
        assert!(poll(&p, &driver));
        assert_eq!(calls.lock().unwrap().as_slice(), &[Some(7), Some(7), None]);
        assert_eq!(p.stats.follow_up_rung.load(Ordering::Relaxed), 2);
        poll(&p, &driver);
        assert_eq!(calls.lock().unwrap().len(), 3);
        // The watchdog: a scan after its period with nothing taken; a channel it finds
        // pending is a doorbell nobody saw.
        *p.last_watchdog.lock().unwrap() = Instant::now() - WATCHDOG;
        assert!(poll(&p, &driver));
        assert_eq!(calls.lock().unwrap().last(), Some(&None));
        assert_eq!(p.stats.watchdog_rung.load(Ordering::Relaxed), 2);
        assert!(p
            .summary()
            .contains("1 doorbell(s) found only by the watchdog"));
    }

    /// The clock's bound: a publish younger than TIME_EVERY is skipped, an older one not.
    #[test]
    fn the_clock_is_published_at_most_once_per_bound() {
        let p = UmPage::create(&intr(), layout(), Instant::now()).unwrap();
        let t = Instant::now() + Duration::from_millis(1);
        assert!(p.publish_bounded(t));
        assert!(!p.publish_bounded(t + TIME_EVERY / 2));
        assert!(p.publish_bounded(t + TIME_EVERY * 2));
        assert!(p.summary().contains("inconsistencies: none"));
    }
}

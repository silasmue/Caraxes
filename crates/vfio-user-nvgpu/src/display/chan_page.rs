// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The window channels' USERD pages as memory (`CARAXES_DISPLAY_KICK_PAGE=1`,
//! docs/caraxes/intr-page.md §12.3).
//!
//! A window channel (NVC57E, instance 0..31) and a window-immediate channel (NVC57B) each
//! have one 4 KiB USERD page in BAR0 from 0x690000 (disp/v03_00/dev_disp.h
//! `NV_UDISP_FE_CHN_ASSY_BASEADR*`, display/mod.rs `userd_base`): `PUT` at +0 and `GET` at
//! +4. The guest (nvkms) writes the method pushbuffer in guest RAM, then `PUT`; it reads
//! `GET` to know how much space is free. Trapped, the `PUT` write is the display kick: the
//! device walks the methods from `GET` to `PUT` (latching the window at `UPDATE`) and
//! moves `GET` to `PUT`. That is 1.33 traps per Mutter frame (series D to P1).
//!
//! With the switch the 64 pages are one sparse area of the interrupt page's memfd. The
//! reactor compares each page's `PUT` with the last one it saw ([`ChanPage::scan`]); a
//! change runs the same kick under the device lock (`Device::display_kick_from_page`) and
//! stores the resulting `GET` ([`ChanPage::complete`]). A `PUT` written twice before the
//! reactor looks is one kick to the last value, which is what the trapped path does for
//! the second write (the walk goes from `GET` to the newest `PUT`). A `PUT` written back to
//! the value the reactor saw last is no change: the guest never does that (a kick always
//! adds methods; the pushbuffer wraps with a JUMP, display/mod.rs `kick`).
//!
//! Fail closed: a store to `GET` is restored and counted; a `PUT` in a page without a
//! channel is counted and left for the channel's allocation, which sets `PUT` and `GET`
//! ([`ChanPage::set_channel`]). The watchdog counts a `GET` still behind a `PUT` the
//! reactor saw more than [`WATCHDOG`] ago (a kick that did not complete).

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::intr_page::{IntrPage, Mapping, PAGE};

/// The switch.
pub const KICK_PAGE_ENV: &str = "CARAXES_DISPLAY_KICK_PAGE";
/// The first window channel's USERD page (display/mod.rs `WIN_USERD`).
pub const BASE: u64 = super::WIN_USERD;
/// Window and window-immediate channels, 32 each.
pub const PAGES: usize = 64;
/// A kick not completed after this long is counted.
pub const WATCHDOG: Duration = Duration::from_millis(20);

/// `CARAXES_DISPLAY_KICK_PAGE`: on by default with the display page (since 2026-09-29), `0`
/// turns it off. The night of 2026-09-28 measured it twice against its control in fresh
/// Linux B guests: vkmark +2-23 %, glmark2 +11-26 %, 0 inconsistencies, the smoke suite and a
/// Moonlight stream verified (caraxes/runs/night-2026-09-28/README.md).
pub fn requested(display_page: bool) -> Result<bool, String> {
    parse(std::env::var(KICK_PAGE_ENV).ok().as_deref(), display_page)
}

pub fn parse(v: Option<&str>, display_page: bool) -> Result<bool, String> {
    let Some(v) = v.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(display_page);
    };
    let on = match v {
        "0" | "off" => false,
        "1" | "on" => true,
        v => return Err(format!("{KICK_PAGE_ENV}={v:?}: 0 or 1")),
    };
    if on && !display_page {
        return Err(format!(
            "{KICK_PAGE_ENV}=1 needs CARAXES_DISPLAY_PAGE=1 (the display clock drives the engine the kicks feed)"
        ));
    }
    Ok(on)
}

#[derive(Default, Debug)]
pub struct Stats {
    pub kicks: AtomicU64,
    /// Kicks of a page with no channel (left until the channel is allocated).
    pub no_channel: AtomicU64,
    pub get_stores: AtomicU64,
    pub stuck: AtomicU64,
    pub kick_us_sum: AtomicU64,
    pub kick_us_max: AtomicU64,
}

pub struct ChanPage {
    maps: Vec<Mapping>,
    /// The `PUT` the reactor saw last per page.
    seen: Vec<AtomicU32>,
    /// The `GET` the device set last per page.
    get: Vec<AtomicU32>,
    /// When a kick was seen and not completed (the watchdog).
    pending_since: Mutex<Vec<Option<Instant>>>,
    pub stats: Stats,
}

impl ChanPage {
    pub fn create(intr: &IntrPage) -> Result<Arc<Self>, String> {
        let maps = (0..PAGES)
            .map(|i| intr.map_page(BASE + i as u64 * PAGE))
            .collect::<Result<Vec<_>, _>>()?;
        let p = ChanPage {
            maps,
            seen: (0..PAGES).map(|_| AtomicU32::new(0)).collect(),
            get: (0..PAGES).map(|_| AtomicU32::new(0)).collect(),
            pending_since: Mutex::new(vec![None; PAGES]),
            stats: Stats::default(),
        };
        p.reset();
        Ok(Arc::new(p))
    }

    /// The sparse area of region 0: all 64 pages.
    pub fn area(&self) -> (u64, u64) {
        (BASE, PAGES as u64 * PAGE)
    }

    pub fn covers(&self, off: u64) -> bool {
        (BASE..BASE + PAGES as u64 * PAGE).contains(&off)
    }

    fn page(off: u64) -> (usize, u64) {
        (((off - BASE) / PAGE) as usize, (off - BASE) % PAGE)
    }

    pub fn reset(&self) {
        for m in &self.maps {
            for off in (0..PAGE).step_by(4) {
                m.word(off).store(0, Ordering::Relaxed);
            }
        }
        for i in 0..PAGES {
            self.seen[i].store(0, Ordering::Relaxed);
            self.get[i].store(0, Ordering::Relaxed);
        }
        std::sync::atomic::fence(Ordering::SeqCst);
    }

    /// A channel was allocated or given its pushbuffer (display/mod.rs): its page shows
    /// `put` and `get` from now on.
    pub fn set_channel(&self, base: u64, put: u32, get: u32) {
        if !self.covers(base) {
            return;
        }
        let (i, _) = Self::page(base);
        self.seen[i].store(put, Ordering::Release);
        self.get[i].store(get, Ordering::Release);
        self.maps[i].word(4).store(get, Ordering::Release);
        self.maps[i].word(0).store(put, Ordering::Release);
    }

    /// A method-state word of a channel's page (its assembly half from 0x200, its ARMED
    /// half from 0x800, display/mod.rs `ChanState`), stored for the guest's direct reads.
    /// PUT and GET (below 0x200) are not touched.
    pub fn publish(&self, base: u64, rel: u64, v: u32) {
        if !self.covers(base) || !(super::CHN_METHODS..PAGE).contains(&rel) || rel % 4 != 0 {
            return;
        }
        let (i, _) = Self::page(base);
        self.maps[i].word(rel).store(v, Ordering::Release);
    }

    pub fn read32(&self, off: u64) -> u32 {
        let (i, rel) = Self::page(off & !3);
        self.maps[i].word(rel).load(Ordering::Acquire)
    }

    /// A trapped write: stored, found by the next scan.
    pub fn write(&self, off: u64, data: &[u8]) {
        let (i, rel) = Self::page(off & !3);
        if data.len() == 4 && off % 4 == 0 {
            let v = u32::from_le_bytes(data.try_into().expect("4 bytes"));
            self.maps[i].word(rel).store(v, Ordering::Release);
        }
    }

    /// The kicks the guest wrote since the last scan: `(USERD base, PUT)`. A `GET` the
    /// guest stored is restored and counted.
    pub fn scan(&self) -> Vec<(u64, u32)> {
        let mut out = Vec::new();
        for i in 0..PAGES {
            let m = &self.maps[i];
            let g = self.get[i].load(Ordering::Acquire);
            if m.word(4).load(Ordering::Acquire) != g {
                m.word(4).store(g, Ordering::Release);
                self.stats.get_stores.fetch_add(1, Ordering::Relaxed);
            }
            let put = m.word(0).load(Ordering::Acquire);
            if put != self.seen[i].load(Ordering::Acquire) {
                self.seen[i].store(put, Ordering::Release);
                out.push((BASE + i as u64 * PAGE, put));
            }
        }
        out
    }

    /// The device ran the kick of `base`: `get` is the channel's `GET` now (`None`: the
    /// page has no channel).
    pub fn complete(&self, base: u64, get: Option<u32>, us: u64) {
        let (i, _) = Self::page(base);
        match get {
            Some(g) => {
                self.get[i].store(g, Ordering::Release);
                self.maps[i].word(4).store(g, Ordering::Release);
                self.stats.kicks.fetch_add(1, Ordering::Relaxed);
                self.stats.kick_us_sum.fetch_add(us, Ordering::Relaxed);
                self.stats.kick_us_max.fetch_max(us, Ordering::Relaxed);
            }
            None => {
                self.stats.no_channel.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// The watchdog, about every millisecond: a page whose `GET` stays behind the `PUT`
    /// the reactor saw for longer than [`WATCHDOG`] is counted once per episode.
    pub fn watchdog(&self, now: Instant) {
        let mut p = self.pending_since.lock().unwrap_or_else(|e| e.into_inner());
        for i in 0..PAGES {
            let behind =
                self.seen[i].load(Ordering::Acquire) != self.get[i].load(Ordering::Acquire);
            match (behind, p[i]) {
                (true, None) => p[i] = Some(now),
                (true, Some(t)) if now.saturating_duration_since(t) >= WATCHDOG => {
                    self.stats.stuck.fetch_add(1, Ordering::Relaxed);
                    p[i] = Some(now + Duration::from_secs(3600));
                }
                (false, _) => p[i] = None,
                _ => {}
            }
        }
    }

    pub fn summary(&self) -> String {
        let s = &self.stats;
        let l = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let mut odd = Vec::new();
        if l(&s.get_stores) > 0 {
            odd.push(format!(
                "{} guest store(s) to GET restored",
                l(&s.get_stores)
            ));
        }
        if l(&s.stuck) > 0 {
            odd.push(format!(
                "{} kick(s) not completed within {} ms",
                l(&s.stuck),
                WATCHDOG.as_millis()
            ));
        }
        let k = l(&s.kicks);
        format!(
            "display kick page: BAR0 [{:#x}, {:#x}); {} kick(s) run by the reactor, mean {} us, max {} us; {} PUT write(s) in a page without a channel; inconsistencies: {}\n",
            BASE,
            BASE + PAGES as u64 * PAGE,
            k,
            l(&s.kick_us_sum).checked_div(k).unwrap_or(0),
            l(&s.kick_us_max),
            l(&s.no_channel),
            if odd.is_empty() { "none".into() } else { odd.join(", ") }
        )
    }
}

/// What the reactor runs for a kick (main.rs: lock the device,
/// `Device::display_kick_from_page`): the channel's `GET` after it.
pub type Driver = dyn Fn(u64, u32) -> Option<u32> + Send + Sync;

/// The reactor's side: run every kick found. Returns whether one ran.
pub fn poll(page: &ChanPage, driver: &dyn Fn(u64, u32) -> Option<u32>) -> bool {
    let kicks = page.scan();
    for &(base, put) in &kicks {
        let t0 = Instant::now();
        let get = driver(base, put);
        page.complete(base, get, t0.elapsed().as_micros() as u64);
    }
    !kicks.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intr::IntrShared;

    fn page() -> Arc<ChanPage> {
        let chip = crate::fixture::rtx2070().chip;
        let d = chip.data();
        let intr = IntrPage::create(
            16 << 20,
            d.registers.intr_tree,
            d.identity,
            IntrShared::new(),
            crate::intr_page::Config::default(),
        )
        .unwrap();
        ChanPage::create(&intr).unwrap()
    }

    #[test]
    fn switch_needs_the_display_page() {
        assert_eq!(parse(None, false), Ok(false));
        assert_eq!(parse(None, true), Ok(true));
        assert_eq!(parse(Some("0"), true), Ok(false));
        assert_eq!(parse(Some("1"), true), Ok(true));
        assert!(parse(Some("1"), false).is_err());
        assert!(parse(Some("x"), true).is_err());
    }

    /// Window 6's page: a PUT store is one kick to the newest PUT, GET follows when the
    /// device is done, a GET store is restored, and a page without a channel is counted.
    #[test]
    fn a_put_store_is_one_kick_and_get_follows() {
        let p = page();
        assert_eq!(p.area(), (0x69_0000, 64 * PAGE));
        let w6 = 0x69_6000;
        p.set_channel(w6, 0x100, 0x100);
        assert_eq!(p.read32(w6 + 4), 0x100);
        let seen = Mutex::new(Vec::new());
        let driver = |b: u64, put: u32| {
            seen.lock().unwrap().push((b, put));
            (b == w6).then_some(put)
        };
        assert!(!poll(&p, &driver));
        p.write(w6, &0x140u32.to_le_bytes());
        p.write(w6, &0x180u32.to_le_bytes());
        assert!(poll(&p, &driver));
        assert_eq!(seen.lock().unwrap().as_slice(), &[(w6, 0x180)]);
        assert_eq!(p.read32(w6 + 4), 0x180);
        assert!(!poll(&p, &driver));
        // A GET store is restored.
        p.write(w6 + 4, &0u32.to_le_bytes());
        assert!(!poll(&p, &driver));
        assert_eq!(p.read32(w6 + 4), 0x180);
        // A page without a channel.
        p.write(0x69_7000, &0x40u32.to_le_bytes());
        assert!(poll(&p, &driver));
        assert_eq!(p.stats.no_channel.load(Ordering::Relaxed), 1);
        assert!(p.summary().contains("1 guest store(s) to GET restored"));
        assert!(p.summary().contains("2 kick(s) run") || p.summary().contains("1 kick(s) run"));
    }

    /// The device's method-state words land in the page for direct reads; PUT, GET and
    /// words outside the page are never touched.
    #[test]
    fn published_state_words_show_in_the_page() {
        let p = page();
        let w0 = 0x69_0000;
        p.set_channel(w0, 0x40, 0x40);
        p.publish(w0, 0xA2C, 0xCF);
        p.publish(w0, 0x22C, 0xE6);
        p.publish(w0, 0, 7);
        p.publish(w0, 4, 7);
        p.publish(w0, PAGE, 7);
        p.publish(0x6D_8000, 0xA2C, 7);
        assert_eq!(p.read32(w0 + 0xA2C), 0xCF);
        assert_eq!(p.read32(w0 + 0x22C), 0xE6);
        assert_eq!((p.read32(w0), p.read32(w0 + 4)), (0x40, 0x40));
        assert_eq!(p.read32(w0 + PAGE), 0);
    }

    /// The watchdog: a kick seen and not completed within the period is counted once.
    #[test]
    fn a_kick_left_behind_is_counted() {
        let p = page();
        let w6 = 0x69_6000;
        p.set_channel(w6, 0, 0);
        p.write(w6, &0x40u32.to_le_bytes());
        let _ = p.scan();
        let t = Instant::now();
        p.watchdog(t);
        p.watchdog(t + WATCHDOG);
        p.watchdog(t + WATCHDOG * 2);
        assert_eq!(p.stats.stuck.load(Ordering::Relaxed), 1);
        p.complete(w6, Some(0x40), 5);
        p.watchdog(t + WATCHDOG * 3);
        assert!(p.summary().contains("1 kick(s) not completed within 20 ms"));
    }
}

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The display front-end interrupt page (`CARAXES_DISPLAY_PAGE=1`, docs/caraxes/
//! intr-page.md §10): BAR0 page 0x611000 as a third sparse mmap area of the interrupt
//! page's memfd (intr_page.rs).
//!
//! The page holds the display's interrupt status (disp/v03_00/dev_disp.h:217-286):
//! `FE_EVT_STAT_HEAD_TIMING(h)`, `FE_EVT_STAT_AWAKEN_WIN/OTHER`, `FE_RM_INTR_STAT_HEAD_TIMING(h)`,
//! `FE_RM_INTR_STAT_CTRL_DISP`, `FE_RM_INTR_EN_HEAD_TIMING(h)` and `FE_RM_INTR_DISPATCH`.
//! The status lives here ([`FeIrq`], under the page's own lock, not the device's); the
//! display engine (engine.rs) sets bits through [`FePage::update`] from the server thread
//! and the display clock thread, and the reactor applies the guest's stores through
//! [`FePage::scan`] as the interrupt page does for the tree. With a read-only guest
//! mapping, trapped writes instead apply synchronously through [`FePage::write`].
//!
//! `exact-ro` instead publishes ordinary register values and applies every guest write
//! synchronously through a read-only VMM slot. It never scans shared bytes, inserts
//! tags/markers, or retriggers a watchdog; current-engine ownership protects reset.
//! The tagged mode below remains the behavior of `1`/`on`.
//!
//! What a memory word cannot report: a store of the value it already holds. So
//! - `FE_EVT_STAT_HEAD_TIMING(h)` shows `TAG` (bit 31, no field) beside the pending bits;
//!   the guest writes constants (`LAST_DATA_RESET`, the RG line and VBLANK resets), never
//!   `TAG`, so every ack changes the word;
//! - `FE_EVT_STAT_AWAKEN_OTHER` shows `TAG` too (the guest writes `CORE_RESET`, 0x1);
//! - `FE_EVT_STAT_AWAKEN_WIN` cannot: all 32 bits are channels, and the guest writes back
//!   exactly the mask it read (kern_disp_0300.c:542-590 and :597-624), so its ack is an
//!   echo the page never sees. The window awakens are therefore published with a marker:
//!   whenever the page shows window bits it also shows `AWAKEN_OTHER_CORE`, and the
//!   guest's `CORE_RESET` store, which follows its window ack in the same call
//!   (`kdispServiceAwakenIntr_v03_00`, kern_disp_0300.c:715-747), acknowledges exactly the
//!   window bits shown. Until then the shown bits are frozen; new awakens wait.
//!   The core channel has no awaken event of its own (NVKMS registers its
//!   completion-notifier callbacks on the layer channels, nvkms-evo.c:5001-5049), so the
//!   extra core awaken finds an empty event list (kern_disp_0300.c:686-694).

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use super::engine::{
    CTRL_DISP_AWAKEN, FE_EVT_STAT_AWAKEN_OTHER, FE_EVT_STAT_AWAKEN_WIN, FE_EVT_STAT_HEAD_TIMING,
    FE_RM_INTR_DISPATCH, FE_RM_INTR_EN_HEAD_TIMING, FE_RM_INTR_STAT_CTRL_DISP,
    FE_RM_INTR_STAT_HEAD_TIMING, LAST_DATA, MAX_HEADS,
};
use crate::intr_page::{IntrPage, Mapping, PAGE};

/// The switch.
pub const DISPLAY_PAGE_ENV: &str = "CARAXES_DISPLAY_PAGE";
/// The page (`NV_PDISP_FE_*` interrupt registers, dev_disp.h:217-286).
pub const FE_PAGE: u64 = 0x0061_1000;
/// Shown in `FE_EVT_STAT_HEAD_TIMING(h)` and `FE_EVT_STAT_AWAKEN_OTHER`; no field there.
pub const TAG: u32 = 1 << 31;
/// `FE_EVT_STAT_AWAKEN_OTHER_CORE` 0:0.
pub const AWAKEN_CORE: u32 = 1;
/// How long the reactor keeps spinning while a marker is outstanding (the window in which
/// a second display pass could read the shown bits again is one reactor iteration).
pub const MARKER_SPIN: Duration = Duration::from_millis(20);

/// Immutable FE mapping contract; neither mode is enabled by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    LegacyTagged,
    ExactReadOnly,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Self::LegacyTagged => "1",
            Self::ExactReadOnly => "exact-ro",
        }
    }

    pub fn automatic_kicks(self) -> bool {
        self == Self::LegacyTagged
    }
}

pub fn requested(intr_page: bool, display: bool) -> Result<bool, String> {
    requested_mode(intr_page, display).map(|mode| mode.is_some())
}

pub fn requested_mode(intr_page: bool, display: bool) -> Result<Option<Mode>, String> {
    parse_mode(std::env::var(DISPLAY_PAGE_ENV).ok().as_deref(), intr_page, display)
}

pub fn parse(v: Option<&str>, intr_page: bool, display: bool) -> Result<bool, String> {
    parse_mode(v, intr_page, display).map(|mode| mode.is_some())
}

pub fn parse_mode(v: Option<&str>, intr_page: bool, display: bool) -> Result<Option<Mode>, String> {
    let mode = match v.map(str::trim).filter(|v| !v.is_empty()).unwrap_or("0") {
        "0" | "off" => None,
        "1" | "on" => Some(Mode::LegacyTagged),
        "exact-ro" if v == Some("exact-ro") => Some(Mode::ExactReadOnly),
        "exact-ro" => return Err(format!("{DISPLAY_PAGE_ENV}: exact-ro requires canonical spelling without whitespace")),
        v => return Err(format!("{DISPLAY_PAGE_ENV}={v:?}: 0, 1 or exact-ro")),
    };
    if mode.is_some() && !intr_page {
        return Err(format!("{DISPLAY_PAGE_ENV} needs the interrupt page (host execution)"));
    }
    if mode.is_some() && !display {
        return Err(format!("{DISPLAY_PAGE_ENV} needs --display (CARAXES_DISPLAY=1)"));
    }
    Ok(mode)
}

/// The display's interrupt status, as the trapped path models it (engine.rs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeIrq {
    /// Heads that exist (`DISPATCH` covers these).
    pub heads: usize,
    /// `FE_EVT_STAT_HEAD_TIMING(h)`.
    pub evt: [u32; MAX_HEADS],
    /// `FE_RM_INTR_EN_HEAD_TIMING(h)`.
    pub en: [u32; MAX_HEADS],
    /// `FE_EVT_STAT_AWAKEN_WIN`: every window awaken not yet acknowledged.
    pub awaken_win: u32,
    /// `FE_EVT_STAT_AWAKEN_OTHER`.
    pub awaken_other: u32,
}

impl FeIrq {
    pub fn new(heads: usize) -> Self {
        FeIrq {
            heads: heads.min(MAX_HEADS),
            ..Default::default()
        }
    }

    /// `FE_RM_INTR_STAT_HEAD_TIMING(h)`.
    pub fn rm_stat(&self, h: usize) -> u32 {
        self.evt[h] & self.en[h]
    }

    /// `FE_RM_INTR_DISPATCH`: a head whose enabled status is pending.
    pub fn dispatch(&self) -> u32 {
        (0..self.heads)
            .filter(|&h| self.rm_stat(h) != 0)
            .fold(0, |m, h| m | 1 << h)
    }

    /// `FE_RM_INTR_STAT_CTRL_DISP` of the trapped path.
    pub fn ctrl_disp(&self) -> u32 {
        if self.awaken_win | self.awaken_other != 0 {
            CTRL_DISP_AWAKEN
        } else {
            0
        }
    }

    /// Whether the display interrupt line is asserted.
    pub fn pending(&self) -> bool {
        self.dispatch() != 0 || self.ctrl_disp() != 0
    }
}

// The watched words, details before summaries: the device writes them in this order, so
// a guest that reads a summary set also reads its details set (x86 keeps stores in
// order, and loads too).
const W_EVT: usize = 0;
const W_AWAKEN_WIN: usize = MAX_HEADS;
const W_AWAKEN_OTHER: usize = W_AWAKEN_WIN + 1;
const W_EN: usize = W_AWAKEN_OTHER + 1;
const W_STAT: usize = W_EN + MAX_HEADS;
const W_CTRL: usize = W_STAT + MAX_HEADS;
const W_DISPATCH: usize = W_CTRL + 1;
const WATCHED: usize = W_DISPATCH + 1;

/// BAR0 offset of watched word `i`.
fn watched_offset(i: usize) -> u64 {
    match i {
        i if i < W_AWAKEN_WIN => FE_EVT_STAT_HEAD_TIMING + 4 * (i - W_EVT) as u64,
        W_AWAKEN_WIN => FE_EVT_STAT_AWAKEN_WIN,
        W_AWAKEN_OTHER => FE_EVT_STAT_AWAKEN_OTHER,
        i if i < W_STAT => FE_RM_INTR_EN_HEAD_TIMING + 4 * (i - W_EN) as u64,
        i if i < W_CTRL => FE_RM_INTR_STAT_HEAD_TIMING + 4 * (i - W_STAT) as u64,
        W_CTRL => FE_RM_INTR_STAT_CTRL_DISP,
        _ => FE_RM_INTR_DISPATCH,
    }
}

/// Inconsistencies, each logged once.
#[derive(Clone, Copy, Debug)]
#[repr(usize)]
enum Odd {
    ReadOnlyStore,
    TagStore,
    StaleEcho,
    StaleCoreAck,
    OtherStore,
}

const ODD_NAMES: [&str; 5] = [
    "a store to a read-only status register (DISPATCH, RM_INTR_STAT, STAT_CTRL_DISP; restored)",
    "a store with the tag bit 31 (bit ignored)",
    "an AWAKEN_WIN store that is not the echo of the bits shown (a second pass acked window bits of an earlier marker; restored)",
    "a CORE_RESET store while no marker was outstanding (ignored)",
    "an AWAKEN_OTHER store without CORE_RESET (restored)",
];

struct State {
    irq: FeIrq,
    /// The window bits the page shows under the outstanding marker.
    shown: u32,
    marker: Option<Instant>,
    /// The watchdog's last raise for the outstanding marker.
    retriggered: Option<Instant>,
    logged: u32,
    notes: Vec<String>,
}

/// Counters for the exit summary.
#[derive(Default)]
pub struct Stats {
    pub scans: AtomicU64,
    pub evt_acks: AtomicU64,
    pub en_stores: AtomicU64,
    pub markers: AtomicU64,
    pub marker_acks: AtomicU64,
    pub raises: AtomicU64,
    pub watchdog: AtomicU64,
    pub cas_retries: AtomicU64,
    /// Trapped accesses of the page (a VMM that maps it read-only, or not at all) served
    /// without the device mutex (device.rs `Routes`).
    pub routed: AtomicU64,
    pub odd: [AtomicU64; 5],
    pub marker_us_max: AtomicU64,
    /// From the display clock's raise to the reactor applying the guest's EVT_STAT ack:
    /// the whole delivery (reactor wake-up, MSI, vCPU, handler), docs/caraxes/intr-page.md
    /// §11.4.
    pub service: Mutex<super::engine::Hist>,
    /// From a marker's publication to the guest's CORE_RESET.
    pub marker_ack: Mutex<super::engine::Hist>,
}

fn bump(a: &AtomicU64) {
    a.fetch_add(1, Ordering::Relaxed);
}

type Raise = Box<dyn Fn() + Send + Sync>;

/// The page, the display's interrupt status and the counters.
pub struct FePage {
    mode: Mode,
    map: Mapping,
    state: Mutex<State>,
    expected: [AtomicU32; WATCHED],
    retrigger: Option<Duration>,
    /// When the outstanding marker opened (nanoseconds after `epoch`, plus one; 0: none),
    /// for the reactor's lock-free `busy` check.
    marker_ns: AtomicU64,
    epoch: Instant,
    /// Raises the display vector (the display clock's `Ticker::raise`; tests count).
    raise: Raise,
    pub stats: Stats,
}

impl FePage {
    /// Map [`FE_PAGE`] from the interrupt page's memfd; raises go through the display
    /// clock (engine.rs `Ticker::raise`).
    pub fn create(intr: &IntrPage) -> Result<Arc<Self>, String> {
        Self::create_mode(intr, Mode::LegacyTagged)
    }

    pub fn create_mode(intr: &IntrPage, mode: Mode) -> Result<Arc<Self>, String> {
        Self::create_with_mode(intr, mode, Box::new(|| super::engine::ticker().raise()))
    }

    pub fn create_with(intr: &IntrPage, raise: Raise) -> Result<Arc<Self>, String> {
        Self::create_with_mode(intr, Mode::LegacyTagged, raise)
    }

    pub fn create_with_mode(intr: &IntrPage, mode: Mode, raise: Raise) -> Result<Arc<Self>, String> {
        let map = intr.map_page(FE_PAGE)?;
        let page = FePage {
            mode,
            map,
            state: Mutex::new(State {
                irq: FeIrq::default(),
                shown: 0,
                marker: None,
                retriggered: None,
                logged: 0,
                notes: Vec::new(),
            }),
            expected: std::array::from_fn(|_| AtomicU32::new(0)),
            retrigger: intr.config().retrigger,
            marker_ns: AtomicU64::new(0),
            epoch: Instant::now(),
            raise,
            stats: Stats::default(),
        };
        page.attach(FeIrq::default());
        Ok(Arc::new(page))
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn exact_readonly(&self) -> bool {
        self.mode == Mode::ExactReadOnly
    }

    pub fn covers_range(&self, off: u64, len: usize) -> bool {
        len > 0 && off >= FE_PAGE
            && off.checked_add(len as u64).is_some_and(|end| end <= FE_PAGE + PAGE)
    }

    /// The sparse area of region 0: (offset, size).
    pub fn area(&self) -> (u64, u64) {
        (FE_PAGE, PAGE)
    }

    /// Whether BAR0 offset `off` lies in the page.
    pub fn covers(&self, off: u64) -> bool {
        (FE_PAGE..FE_PAGE + PAGE).contains(&off)
    }

    /// The start-up check: `engine(off)` says whether the display engine models the dword
    /// (engine.rs `Engine::handles`), `other(off)` whether any other model does (the boot
    /// model's registers, the display's other pages). The engine must model exactly the
    /// watched words of the page, and nothing else may model any word of it: the rest of
    /// the page is latched memory, as the trapped path latches it.
    pub fn check(engine: &dyn Fn(u64) -> bool, other: &dyn Fn(u64) -> bool) -> Result<(), String> {
        for off in (FE_PAGE..FE_PAGE + PAGE).step_by(4) {
            let watched = (0..WATCHED).any(|i| watched_offset(i) == off);
            if engine(off) != watched {
                return Err(format!(
                    "BAR0 {off:#x}: the display engine {} it, the display page {}",
                    if engine(off) {
                        "models"
                    } else {
                        "does not model"
                    },
                    if watched { "watches it" } else { "does not" }
                ));
            }
            if other(off) {
                return Err(format!(
                    "BAR0 {off:#x} has a register model outside the display engine and lies in the display page"
                ));
            }
        }
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn word(&self, off: u64) -> &AtomicU32 {
        self.map.word((off & !3) - FE_PAGE)
    }

    fn watched(&self, i: usize) -> &AtomicU32 {
        self.word(watched_offset(i))
    }

    /// A new display engine takes the page (start, device reset): its status becomes the
    /// page's, the page is zeroed and shows it.
    pub fn attach(&self, irq: FeIrq) {
        let mut st = self.lock();
        st.irq = irq;
        st.shown = 0;
        st.marker = None;
        st.retriggered = None;
        self.marker_ns.store(0, Ordering::Relaxed);
        for off in (0..PAGE).step_by(4) {
            self.map.word(off).store(0, Ordering::Relaxed);
        }
        for i in 0..WATCHED {
            self.expected[i].store(0, Ordering::Relaxed);
        }
        self.publish(&mut st, &[0; WATCHED]);
        std::sync::atomic::fence(Ordering::SeqCst);
    }

    /// The status as the page last applied it.
    pub fn irq(&self) -> FeIrq {
        self.lock().irq.clone()
    }

    /// A trapped read of the page (a client that did not map it): the memory.
    pub fn read32(&self, off: u64) -> u32 {
        if self.exact_readonly() && !self.covers(off & !3) { return 0; }
        self.word(off).load(Ordering::Acquire)
    }

    /// Apply a trapped write before the guest resumes. Unlike a shared-memory store,
    /// the transaction exposes an AWAKEN_WIN echo even when its value is unchanged.
    ///
    /// Only the page's status changes here, under the page's own lock: no FB, BAR1 or guest
    /// memory is touched, so the write needs neither the device mutex nor the BAR1 pool's
    /// poll (device.rs `Routes`, docs/research/vblank-path-2026-09-29.md). The cost is the
    /// lock, a scan of the 28 watched words and at most one raise (an eventfd write).
    pub fn write(&self, off: u64, data: &[u8]) {
        if self.exact_readonly() {
            if !self.covers_range(off, data.len()) { return; }
            self.write_exact(off, data);
            return;
        }
        let raise = {
            let mut st = self.lock();
            let (mut signal, seen) = self.sync(&mut st);
            let mut i = 0;
            while i < data.len() {
                let o = off + i as u64;
                let byte = (o & 3) as usize;
                let n = (4 - byte).min(data.len() - i);
                let mut value = 0u32;
                let mut mask = 0u32;
                for j in 0..n {
                    value |= u32::from(data[i + j]) << ((byte + j) * 8);
                    mask |= 0xff << ((byte + j) * 8);
                }
                match (0..WATCHED).find(|&w| watched_offset(w) == o & !3) {
                    Some(w) if w < W_AWAKEN_WIN => {
                        st.irq.evt[w] &= !(value & !TAG);
                        self.note_evt_ack();
                        signal = true;
                    }
                    Some(W_AWAKEN_WIN) => {
                        st.irq.awaken_win &= !value;
                        st.shown &= !value;
                        signal = true;
                    }
                    Some(W_AWAKEN_OTHER) if value & AWAKEN_CORE != 0 => {
                        if let Some(t) = st.marker.take() {
                            self.marker_ns.store(0, Ordering::Relaxed);
                            st.irq.awaken_win &= !st.shown;
                            st.irq.awaken_other &= !AWAKEN_CORE;
                            st.shown = 0;
                            st.retriggered = None;
                            self.note_marker_ack(t);
                            signal = true;
                        }
                    }
                    Some(W_AWAKEN_OTHER) => {}
                    Some(w) if (W_EN..W_STAT).contains(&w) => {
                        let h = w - W_EN;
                        let was = st.irq.en[h];
                        let v = (was & !mask) | value;
                        st.irq.en[h] = v;
                        super::engine::ticker().set_vblank(h, v & LAST_DATA != 0);
                        bump(&self.stats.en_stores);
                        signal = true;
                        if (was ^ v) & LAST_DATA != 0 {
                            st.notes.push(format!(
                                "display: head {h} vblank (LAST_DATA) interrupt {} [guest, display page]",
                                if v & LAST_DATA != 0 { "enabled" } else { "disabled" },
                            ));
                        }
                    }
                    Some(_) => self.odd(&mut st, Odd::ReadOnlyStore),
                    None => {
                        let word = self.word(o);
                        let _ = word.fetch_update(Ordering::AcqRel, Ordering::Acquire,
                            |old| Some((old & !mask) | value));
                    }
                }
                i += n;
            }
            self.publish(&mut st, &seen);
            signal && st.irq.pending()
        };
        if raise {
            self.raise_now();
        }
    }

    /// Exact trapped lanes, under the current-engine guard held by Routes. Publication
    /// completes before the reply/raise; the callback runs after releasing FE state.
    fn write_exact(&self, off: u64, data: &[u8]) {
        let raise = {
            let mut st = self.lock();
            let mut signal = false;
            let mut i = 0;
            while i < data.len() {
                let o = off + i as u64;
                let byte = (o & 3) as usize;
                let n = (4 - byte).min(data.len() - i);
                let mut value = 0u32;
                let mut mask = 0u32;
                for j in 0..n {
                    value |= u32::from(data[i + j]) << ((byte + j) * 8);
                    mask |= 0xff << ((byte + j) * 8);
                }
                match (0..WATCHED).find(|&w| watched_offset(w) == o & !3) {
                    Some(w) if w < W_AWAKEN_WIN => {
                        st.irq.evt[w] &= !value;
                        self.note_evt_ack();
                        signal = true;
                    }
                    Some(W_AWAKEN_WIN) => { st.irq.awaken_win &= !value; signal = true; }
                    Some(W_AWAKEN_OTHER) => { st.irq.awaken_other &= !value; signal = true; }
                    Some(w) if (W_EN..W_STAT).contains(&w) => {
                        let h = w - W_EN;
                        st.irq.en[h] = (st.irq.en[h] & !mask) | value;
                        super::engine::ticker().set_vblank(h, st.irq.en[h] & LAST_DATA != 0);
                        bump(&self.stats.en_stores);
                        signal = true;
                    }
                    Some(_) => { self.odd(&mut st, Odd::ReadOnlyStore); signal = true; }
                    None => {
                        self.word(o).fetch_update(Ordering::AcqRel, Ordering::Acquire,
                            |old| Some((old & !mask) | value)).expect("unconditional lane update");
                    }
                }
                i += n;
            }
            self.publish_exact(&st);
            signal && st.irq.pending()
        };
        if raise { self.raise_now(); }
    }

    /// The fast check of one reactor iteration.
    pub fn dirty(&self) -> bool {
        if self.exact_readonly() { return false; }
        (0..WATCHED).any(|i| {
            self.watched(i).load(Ordering::Relaxed) != self.expected[i].load(Ordering::Relaxed)
        })
    }

    /// Whether the reactor should keep spinning: a marker is outstanding and recent, so
    /// the guest's `CORE_RESET` is seen within one iteration.
    pub fn busy(&self, now: Instant) -> bool {
        if self.exact_readonly() { return false; }
        let m = self.marker_ns.load(Ordering::Relaxed);
        m != 0
            && now.saturating_duration_since(self.epoch + Duration::from_nanos(m - 1)) < MARKER_SPIN
    }

    /// A guest `EVT_STAT` ack, from memory (`sync`) or trapped (`write`): counted, and the
    /// delivery from the clock's raise to it (docs/caraxes/intr-page.md §11.4) goes into
    /// the histogram. With the read-only page every ack is trapped, so this is the
    /// device's own measure of the vblank handler's latency.
    fn note_evt_ack(&self) {
        bump(&self.stats.evt_acks);
        if let Some(t) = super::engine::ticker().take_raise_for_ack() {
            let us = super::engine::now_ns().saturating_sub(t) / 1000;
            self.stats
                .service
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .add(us);
        }
    }

    /// A guest `CORE_RESET` acknowledged the marker opened at `t`.
    fn note_marker_ack(&self, t: Instant) {
        bump(&self.stats.marker_acks);
        let us = t.elapsed().as_micros() as u64;
        self.stats.marker_us_max.fetch_max(us, Ordering::Relaxed);
        self.stats
            .marker_ack
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .add(us);
    }

    fn odd(&self, st: &mut State, what: Odd) {
        bump(&self.stats.odd[what as usize]);
        let bit = 1 << what as usize;
        if st.logged & bit == 0 {
            st.logged |= bit;
            st.notes.push(format!(
                "display page: {} (logged once)",
                ODD_NAMES[what as usize]
            ));
        }
    }

    /// The display engine changes the status (engine.rs, server thread or display clock):
    /// the guest's stores first, then `f`, then the page. A level still pending after
    /// guest acks raises the vector again (the trapped path's `Engine::write32`).
    pub fn update<R>(&self, f: impl FnOnce(&mut FeIrq) -> R) -> R {
        if self.exact_readonly() {
            let mut st = self.lock();
            let result = f(&mut st.irq);
            self.publish_exact(&st);
            return result;
        }
        let (r, raise) = {
            let mut st = self.lock();
            let (acked, seen) = self.sync(&mut st);
            let r = f(&mut st.irq);
            self.publish(&mut st, &seen);
            (r, acked && st.irq.pending())
        };
        if raise {
            self.raise_now();
        }
        r
    }

    /// The reactor's scan: apply the guest's stores, rewrite the page, raise again when
    /// the line is still asserted after an ack. Returns whether there was work.
    pub fn scan(&self) -> bool {
        if self.exact_readonly() { return false; }
        self.scan_racing(&mut || {})
    }

    fn scan_racing(&self, between: &mut dyn FnMut()) -> bool {
        if self.exact_readonly() { return false; }
        if !self.dirty() {
            return false;
        }
        let raise = {
            let mut st = self.lock();
            bump(&self.stats.scans);
            let (acked, seen) = self.sync(&mut st);
            between();
            self.publish(&mut st, &seen);
            acked && st.irq.pending()
        };
        if raise {
            self.raise_now();
        }
        true
    }

    fn raise_now(&self) {
        bump(&self.stats.raises);
        (self.raise)();
    }

    /// Apply the guest's stores found in the page: whether an ack was among them, and the
    /// words as read (the write-back swaps from these).
    fn sync(&self, st: &mut State) -> (bool, [u32; WATCHED]) {
        let w: [u32; WATCHED] = std::array::from_fn(|i| self.watched(i).load(Ordering::Acquire));
        let changed = |i: usize| w[i] != self.expected[i].load(Ordering::Relaxed);
        let mut acked = false;
        for h in 0..MAX_HEADS {
            if changed(W_EN + h) {
                let v = w[W_EN + h];
                let was = st.irq.en[h];
                st.irq.en[h] = v;
                bump(&self.stats.en_stores);
                super::engine::ticker().set_vblank(h, v & LAST_DATA != 0);
                if (was ^ v) & LAST_DATA != 0 {
                    st.notes.push(format!(
                        "display: head {h} vblank (LAST_DATA) interrupt {} [guest, display page]",
                        if v & LAST_DATA != 0 {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    ));
                }
            }
        }
        for h in 0..MAX_HEADS {
            if changed(W_EVT + h) {
                let v = w[W_EVT + h];
                if v & TAG != 0 {
                    self.odd(st, Odd::TagStore);
                }
                st.irq.evt[h] &= !(v & !TAG);
                acked = true;
                self.note_evt_ack();
            }
        }
        if changed(W_AWAKEN_WIN) {
            // The echo of the shown bits is invisible; anything else is a pass that read
            // an earlier marker's bits (the one window of the marker rule, §10).
            self.odd(st, Odd::StaleEcho);
        }
        if changed(W_AWAKEN_OTHER) {
            let v = w[W_AWAKEN_OTHER];
            if v & TAG != 0 {
                self.odd(st, Odd::TagStore);
            }
            if v & AWAKEN_CORE == 0 {
                self.odd(st, Odd::OtherStore);
            } else if let Some(t) = st.marker.take() {
                self.marker_ns.store(0, Ordering::Relaxed);
                // The guest read the shown window bits before this store and acked them.
                st.irq.awaken_win &= !st.shown;
                st.irq.awaken_other &= !AWAKEN_CORE;
                st.shown = 0;
                st.retriggered = None;
                acked = true;
                self.note_marker_ack(t);
            } else {
                self.odd(st, Odd::StaleCoreAck);
            }
        }
        if (W_STAT..W_CTRL).any(changed) || changed(W_CTRL) || changed(W_DISPATCH) {
            self.odd(st, Odd::ReadOnlyStore);
        }
        (acked, w)
    }

    /// The word the page shows for watched word `i`.
    fn display(st: &State, i: usize) -> u32 {
        let marker = st.marker.is_some();
        match i {
            i if i < W_AWAKEN_WIN => st.irq.evt[i - W_EVT] | TAG,
            W_AWAKEN_WIN => {
                if marker {
                    st.shown
                } else {
                    0
                }
            }
            W_AWAKEN_OTHER => (if marker { AWAKEN_CORE } else { 0 }) | TAG,
            i if i < W_STAT => st.irq.en[i - W_EN],
            i if i < W_CTRL => st.irq.rm_stat(i - W_STAT),
            W_CTRL => {
                if marker {
                    CTRL_DISP_AWAKEN
                } else {
                    0
                }
            }
            _ => st.irq.dispatch(),
        }
    }

    /// Open a marker when awakens wait and none is outstanding, then write every watched
    /// word whose display differs, details first, with a compare-and-swap from `seen`
    /// (what `sync` read and applied): a guest store that landed since is kept for the
    /// next scan.
    fn publish(&self, st: &mut State, seen: &[u32; WATCHED]) {
        if self.exact_readonly() { self.publish_exact(st); return; }
        if st.marker.is_none() && (st.irq.awaken_win != 0 || st.irq.awaken_other != 0) {
            st.shown = st.irq.awaken_win;
            let now = Instant::now();
            st.marker = Some(now);
            let ns = now.saturating_duration_since(self.epoch).as_nanos() as u64;
            self.marker_ns.store(ns + 1, Ordering::Relaxed);
            st.retriggered = None;
            bump(&self.stats.markers);
        }
        for (i, &was) in seen.iter().enumerate() {
            let d = Self::display(st, i);
            if was != d
                && self
                    .watched(i)
                    .compare_exchange(was, d, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
            {
                bump(&self.stats.cas_retries);
            }
            self.expected[i].store(d, Ordering::Relaxed);
        }
    }

    /// Ordinary Engine::read32 values, details before summaries and DISPATCH last.
    /// A mapped reader gets ordered publication, not an atomic multi-register snapshot.
    /// The guest cannot write this mapping: skip unchanged words to avoid dirtying all
    /// watched cache lines on each head ack, while reading the actual page so a stray
    /// change is repaired even when the model itself is unchanged.
    fn publish_exact(&self, st: &State) -> usize {
        let mut changed = 0;
        for i in 0..WATCHED {
            let value = match i {
                i if i < W_AWAKEN_WIN => st.irq.evt[i - W_EVT],
                W_AWAKEN_WIN => st.irq.awaken_win,
                W_AWAKEN_OTHER => st.irq.awaken_other,
                i if i < W_STAT => st.irq.en[i - W_EN],
                i if i < W_CTRL => st.irq.rm_stat(i - W_STAT),
                W_CTRL => st.irq.ctrl_disp(),
                _ => st.irq.dispatch(),
            };
            if self.watched(i).load(Ordering::Relaxed) != value {
                self.watched(i).store(value, Ordering::Release);
                self.expected[i].store(value, Ordering::Relaxed);
                changed += 1;
            }
        }
        std::sync::atomic::fence(Ordering::SeqCst);
        changed
    }

    /// The watchdog, about every millisecond: a marker outstanding for the retrigger
    /// period raises the vector again (late, not lost), at most once per period.
    pub fn tick(&self, now: Instant) -> bool {
        if self.exact_readonly() { return false; }
        let Some(period) = self.retrigger else {
            return false;
        };
        let fire = {
            let mut st = self.lock();
            match st.marker {
                Some(t) if now.saturating_duration_since(st.retriggered.unwrap_or(t)) >= period => {
                    st.retriggered = Some(now);
                    true
                }
                _ => false,
            }
        };
        if fire {
            bump(&self.stats.watchdog);
            self.raise_now();
        }
        fire
    }

    /// Lines for the trace (drained by the display clock's tick, device.rs).
    pub fn take_notes(&self) -> Vec<String> {
        std::mem::take(&mut self.lock().notes)
    }

    /// One line for the exit summary.
    pub fn summary(&self) -> String {
        let s = &self.stats;
        let l = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let odd: Vec<String> = ODD_NAMES
            .iter()
            .zip(&s.odd)
            .filter(|(_, n)| l(n) > 0)
            .map(|(what, n)| format!("{} x {what}", l(n)))
            .collect();
        let mode_note = if self.exact_readonly() {
            "display page mode=exact-ro: ordinary values; no markers, shared-byte scanning or watchdog\n"
        } else { "" };
        mode_note.to_owned() + &format!(
            "display page: {} scan(s) with work; guest stores applied: {} EVT_STAT ack(s), {} RM_INTR_EN store(s); {} awaken marker(s), {} acked by CORE_RESET (longest {:.1} ms); {} raise(s) after an ack, {} by the watchdog; {} compare-and-swap(s) lost to a guest store; {} trapped access(es) served without the device mutex; inconsistencies: {}\n",
            l(&s.scans),
            l(&s.evt_acks),
            l(&s.en_stores),
            l(&s.markers),
            l(&s.marker_acks),
            l(&s.marker_us_max) as f64 / 1000.0,
            l(&s.raises),
            l(&s.watchdog),
            l(&s.cas_retries),
            l(&s.routed),
            if odd.is_empty() { "none".into() } else { odd.join(", ") }
        ) + &format!(
            "display page delivery: raise to the guest's EVT_STAT ack {}; marker to CORE_RESET {}\n",
            s.service.lock().unwrap_or_else(PoisonError::into_inner).describe(),
            s.marker_ack.lock().unwrap_or_else(PoisonError::into_inner).describe()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intr::IntrShared;
    use crate::intr_page::Config;

    fn page() -> (Arc<IntrPage>, Arc<FePage>, Arc<AtomicU64>) {
        page_mode(Mode::LegacyTagged)
    }

    fn page_mode(mode: Mode) -> (Arc<IntrPage>, Arc<FePage>, Arc<AtomicU64>) {
        let intr = IntrPage::create(
            16 << 20,
            0x00B8_1000,
            crate::fixture::rtx2070().chip.data().identity,
            IntrShared::new(),
            Config::default(),
        )
        .unwrap();
        let n = Arc::new(AtomicU64::new(0));
        let c = n.clone();
        let fe = FePage::create_with_mode(
            &intr,
            mode,
            Box::new(move || {
                c.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();
        fe.attach(FeIrq::new(4));
        (intr, fe, n)
    }

    #[test]
    fn exact_ro_mode_requires_dependencies_and_never_adds_kicks() {
        assert_eq!(parse_mode(None, false, false).unwrap(), None);
        assert_eq!(parse_mode(Some("off"), true, true).unwrap(), None);
        assert_eq!(parse_mode(Some("exact-ro"), true, true).unwrap(), Some(Mode::ExactReadOnly));
        assert!(parse_mode(Some(" exact-ro "), true, true).is_err());
        assert!(parse_mode(Some("\u{a0}exact-ro\u{a0}"), true, true).is_err());
        assert!(parse_mode(Some("exact-ro"), false, true).is_err());
        assert!(parse_mode(Some("exact-ro"), true, false).is_err());
        assert!(parse_mode(Some("exact"), true, true).is_err());
        assert!(!Mode::ExactReadOnly.automatic_kicks());
        assert!(Mode::LegacyTagged.automatic_kicks());
        assert_eq!(parse_mode(Some("1"), true, true).unwrap(), Some(Mode::LegacyTagged));
    }

    #[test]
    fn exact_ro_narrow_writes_preserve_independent_causes_and_latched_lanes() {
        let (_intr, p, raised) = page_mode(Mode::ExactReadOnly);
        p.update(|irq| {
            irq.evt[0] = 0x8001_0203;
            irq.evt[1] = LAST_DATA;
            irq.en[0] = 0x8001_ffff;
            irq.en[1] = LAST_DATA;
            irq.awaken_win = 3;
            irq.awaken_other = AWAKEN_CORE | 2;
        });
        // A byte W1C clears only that supplied lane, including the ordinary bit 31.
        p.write(FE_EVT_STAT_HEAD_TIMING + 1, &[2]);
        assert_eq!(p.irq().evt[0], 0x8001_0003);
        p.write(FE_EVT_STAT_HEAD_TIMING + 3, &[0x80]);
        assert_eq!(p.irq().evt[0], 0x0001_0003);
        p.write(FE_RM_INTR_EN_HEAD_TIMING + 1, &[0x12]);
        assert_eq!(p.irq().en[0], 0x8001_12ff);
        p.write(FE_EVT_STAT_AWAKEN_OTHER, &AWAKEN_CORE.to_le_bytes());
        assert_eq!((p.irq().awaken_win, p.irq().awaken_other), (3, 2));
        p.write(FE_EVT_STAT_AWAKEN_WIN, &1u32.to_le_bytes());
        p.update(|irq| irq.awaken_win |= 4);
        p.write(FE_EVT_STAT_AWAKEN_WIN, &1u32.to_le_bytes());
        assert_eq!(p.irq().awaken_win, 6, "old echoes cannot acknowledge a new cause");
        let before = p.irq();
        p.write(FE_RM_INTR_DISPATCH, &u32::MAX.to_le_bytes());
        p.write(FE_RM_INTR_STAT_HEAD_TIMING, &u32::MAX.to_le_bytes());
        assert_eq!(p.irq(), before, "derived words are read-only");
        assert_eq!(p.read32(FE_RM_INTR_DISPATCH), 3);
        p.write(FE_PAGE + 0x21, &[0xaa, 0xbb, 0xcc, 0xdd]);
        assert_eq!(p.read32(FE_PAGE + 0x20), 0xccbb_aa00);
        assert_eq!(p.read32(FE_PAGE + 0x24), 0xdd);
        assert!(raised.load(Ordering::Relaxed) > 0);
        let before = p.irq();
        for (off, bytes) in [(FE_PAGE - 1, &[1, 2][..]), (FE_PAGE + PAGE - 1, &[3, 4][..]), (u64::MAX, &[5][..]), (FE_PAGE, &[][..])] {
            assert!(!p.covers_range(off, bytes.len()));
            p.write(off, bytes);
        }
        assert_eq!(p.irq(), before);
        assert_eq!(p.read32(FE_PAGE + PAGE - 4), 0);
        // A single transaction spanning adjacent head words applies every supplied lane.
        p.write(FE_EVT_STAT_HEAD_TIMING, &[3, 0, 1, 0, LAST_DATA as u8]);
        assert_eq!(p.irq().evt[0], 0);
        assert_eq!(p.irq().evt[1], 0);
        assert_eq!(p.read32(FE_RM_INTR_DISPATCH), 0);
    }

    #[test]
    fn exact_ro_scans_cannot_interpret_shared_bytes_and_reset_clears_page() {
        let (_intr, p, raised) = page_mode(Mode::ExactReadOnly);
        p.update(|irq| { irq.evt[0] = LAST_DATA; irq.en[0] = LAST_DATA; irq.awaken_win = 5; });
        let before = p.irq();
        store(&p, FE_EVT_STAT_HEAD_TIMING, 0);
        store(&p, FE_EVT_STAT_AWAKEN_OTHER, TAG | AWAKEN_CORE);
        assert!(!p.scan());
        assert!(!p.scan_racing(&mut || panic!("exact scan must not enter legacy synchronization")));
        assert!(!p.dirty());
        assert!(!p.busy(Instant::now()));
        assert!(!p.tick(Instant::now() + Duration::from_secs(60)));
        assert_eq!(p.irq(), before);
        assert_eq!(raised.load(Ordering::Relaxed), 0);
        p.update(|_| {});
        assert_eq!(load(&p, FE_EVT_STAT_HEAD_TIMING), LAST_DATA);
        assert_eq!(load(&p, FE_EVT_STAT_AWAKEN_OTHER), 0);
        assert_eq!(load(&p, FE_EVT_STAT_AWAKEN_WIN), 5);
        p.write(FE_PAGE + 0x20, &[0x55; 4]);
        p.attach(FeIrq::new(4));
        for off in (FE_PAGE..FE_PAGE + PAGE).step_by(4) { assert_eq!(p.read32(off), 0, "{off:#x}"); }
        assert_eq!(p.stats.scans.load(Ordering::Relaxed), 0);
        assert_eq!(p.stats.markers.load(Ordering::Relaxed), 0);
        assert_eq!(p.stats.watchdog.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn exact_ro_publication_is_complete_before_raise() {
        let (intr, _unused, _) = page();
        let current = Arc::new(Mutex::new(None::<std::sync::Weak<FePage>>));
        let view = current.clone();
        let raised = Arc::new(AtomicU64::new(0));
        let calls = raised.clone();
        let p = FePage::create_with_mode(&intr, Mode::ExactReadOnly, Box::new(move || {
            let p = view.lock().unwrap().as_ref().unwrap().upgrade().unwrap();
            // irq() also proves the callback is outside FE state ownership.
            let irq = p.irq();
            for i in 0..MAX_HEADS {
                assert_eq!(p.read32(FE_EVT_STAT_HEAD_TIMING + 4 * i as u64), irq.evt[i]);
                assert_eq!(p.read32(FE_RM_INTR_EN_HEAD_TIMING + 4 * i as u64), irq.en[i]);
                assert_eq!(p.read32(FE_RM_INTR_STAT_HEAD_TIMING + 4 * i as u64), irq.rm_stat(i));
            }
            assert_eq!(p.read32(FE_EVT_STAT_AWAKEN_WIN), irq.awaken_win);
            assert_eq!(p.read32(FE_EVT_STAT_AWAKEN_OTHER), irq.awaken_other);
            assert_eq!(p.read32(FE_RM_INTR_STAT_CTRL_DISP), irq.ctrl_disp());
            assert_eq!(p.read32(FE_RM_INTR_DISPATCH), irq.dispatch());
            calls.fetch_add(1, Ordering::Relaxed);
        })).unwrap();
        *current.lock().unwrap() = Some(Arc::downgrade(&p));
        p.attach(FeIrq::new(4));
        p.update(|irq| { irq.evt[0] = LAST_DATA; irq.evt[1] = LAST_DATA; irq.awaken_win = 4; });
        p.write(FE_RM_INTR_EN_HEAD_TIMING, &LAST_DATA.to_le_bytes());
        p.write(FE_RM_INTR_EN_HEAD_TIMING + 4, &LAST_DATA.to_le_bytes());
        p.write(FE_EVT_STAT_HEAD_TIMING, &LAST_DATA.to_le_bytes());
        assert_eq!(raised.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn exact_ro_sparse_publication_keeps_full_image_and_reset() {
        let (_intr, p, _) = page_mode(Mode::ExactReadOnly);
        {
            let mut st = p.lock();
            st.irq.evt[0] = LAST_DATA;
            st.irq.en[0] = LAST_DATA;
            st.irq.awaken_win = 4;
            assert_eq!(p.publish_exact(&st), 6);
            assert_eq!(p.publish_exact(&st), 0, "unchanged status dirties no mapped words");
            p.word(FE_EVT_STAT_HEAD_TIMING).store(0, Ordering::Relaxed);
            assert_eq!(p.publish_exact(&st), 1, "publication repairs the page itself");
        }
        for off in (FE_PAGE..FE_PAGE + PAGE).step_by(4) {
            let expected = match off {
                FE_EVT_STAT_HEAD_TIMING | FE_RM_INTR_EN_HEAD_TIMING
                | FE_RM_INTR_STAT_HEAD_TIMING => LAST_DATA,
                FE_RM_INTR_DISPATCH => 1,
                FE_RM_INTR_STAT_CTRL_DISP => CTRL_DISP_AWAKEN,
                FE_EVT_STAT_AWAKEN_WIN => 4,
                _ => 0,
            };
            assert_eq!(p.read32(off), expected, "BAR0 {off:#x}");
        }
        p.write(FE_EVT_STAT_HEAD_TIMING, &LAST_DATA.to_le_bytes());
        assert_eq!(p.read32(FE_EVT_STAT_HEAD_TIMING), 0);
        assert_eq!(p.read32(FE_RM_INTR_STAT_HEAD_TIMING), 0);
        assert_eq!(p.read32(FE_RM_INTR_DISPATCH), 0);
        p.attach(FeIrq::new(4));
        for off in (FE_PAGE..FE_PAGE + PAGE).step_by(4) {
            assert_eq!(p.read32(off), 0, "reset BAR0 {off:#x}");
        }
    }

    fn store(p: &FePage, off: u64, v: u32) {
        p.word(off).store(v, Ordering::Release);
    }

    fn load(p: &FePage, off: u64) -> u32 {
        p.word(off).load(Ordering::Acquire)
    }

    const H3: u64 = 12;

    /// A VMM that maps the page read-only traps the guest's stores: the server applies them
    /// under the page's own lock, without the device mutex (device.rs `Routes`; Linux SotTR
    /// spent 42.6 s of its load in 41k EVT_STAT acks waiting for the device mutex).
    #[test]
    fn routed_trapped_acks_do_not_need_the_device_mutex() {
        let (_intr, fe, _) = page();
        fe.update(|i| i.evt[3] = super::super::engine::LAST_DATA);
        let routes = crate::device::Routes { fe_page: Some(fe.clone()), ..Default::default() };
        let device = std::sync::Mutex::new(());
        let _held = device.lock().unwrap();
        let ack = FE_EVT_STAT_HEAD_TIMING + H3;
        let mut b = [0u8; 4];
        assert!(routes.read(0, ack, &mut b));
        assert_eq!(u32::from_le_bytes(b) & !TAG, super::super::engine::LAST_DATA);
        assert!(routes.write(0, ack, &super::super::engine::LAST_DATA.to_le_bytes()));
        assert_eq!(fe.irq().evt[3], 0, "the ack applied");
        assert_eq!(fe.stats.routed.load(Ordering::Relaxed), 2);
        assert!(!routes.write(1, ack, &[0; 4]), "BAR0 only");
        assert!(fe.summary().contains("2 trapped access(es) served without the device mutex"));
    }

    #[test]
    fn trapped_echo_clears_before_another_guest_pass_without_a_scan() {
        let (_intr, fe, _) = page();
        fe.update(|i| i.awaken_win = 0x40);
        // A different window completes while the first marker is still outstanding.
        fe.update(|i| i.awaken_win |= 0x2);
        let echo = fe.read32(FE_EVT_STAT_AWAKEN_WIN);
        fe.write(FE_EVT_STAT_AWAKEN_WIN, &echo.to_le_bytes());
        assert_eq!(fe.read32(FE_EVT_STAT_AWAKEN_WIN), 0);
        assert_eq!(fe.irq().awaken_win, 0x2);
        // A new completion on the just-acknowledged window must also survive CORE_RESET.
        fe.update(|i| i.awaken_win |= 0x40);
        assert_eq!(fe.read32(FE_EVT_STAT_AWAKEN_WIN), 0);
        // The next marker becomes visible only after this pass's core acknowledgement.
        fe.write(FE_EVT_STAT_AWAKEN_OTHER, &AWAKEN_CORE.to_le_bytes());
        assert_eq!(fe.read32(FE_EVT_STAT_AWAKEN_WIN), 0x42);
        fe.write(FE_EVT_STAT_AWAKEN_WIN, &0x42u32.to_le_bytes());
        fe.write(FE_EVT_STAT_AWAKEN_OTHER, &AWAKEN_CORE.to_le_bytes());
        assert_eq!(fe.read32(FE_RM_INTR_STAT_CTRL_DISP), 0);
        assert!(!fe.dirty());
        assert!(!fe.scan());
        assert!(fe.summary().contains("inconsistencies: none"));
    }

    /// With the read-only page every ack is trapped: `write` counts them and feeds the
    /// delivery histograms as the reactor's scan does (the summary's `display page
    /// delivery` line, docs/research/vblank-path-2026-09-29.md).
    #[test]
    fn trapped_acks_feed_the_delivery_histograms() {
        let (_intr, fe, raises) = page();
        fe.update(|i| {
            i.en[3] = LAST_DATA;
            i.evt[3] = LAST_DATA;
            i.awaken_win = 0x40;
        });
        // kdispServiceAwakenIntr, then the vblank's LAST_DATA reset: the level stays up
        // after each awaken ack (vblank pending), so each raises once, as the scan would.
        fe.write(FE_EVT_STAT_AWAKEN_WIN, &0x40u32.to_le_bytes());
        fe.write(FE_EVT_STAT_AWAKEN_OTHER, &AWAKEN_CORE.to_le_bytes());
        fe.write(FE_EVT_STAT_HEAD_TIMING + H3, &LAST_DATA.to_le_bytes());
        assert_eq!(raises.load(Ordering::Relaxed), 2);
        assert!(!fe.irq().pending());
        assert_eq!(fe.stats.evt_acks.load(Ordering::Relaxed), 1);
        assert_eq!(fe.stats.marker_acks.load(Ordering::Relaxed), 1);
        assert_eq!(fe.stats.marker_ack.lock().unwrap().n, 1);
        let s = fe.summary();
        assert!(s.contains("marker to CORE_RESET p50 <= "), "{s}");
        assert!(s.contains("of 1 at 1 ms or more\n"), "{s}");
        assert!(!fe.dirty());
    }

    #[test]
    fn trapped_enable_raises_an_already_pending_event() {
        let (_intr, fe, raises) = page();
        fe.update(|i| i.evt[0] = LAST_DATA);
        assert_eq!(fe.read32(FE_RM_INTR_DISPATCH), 0);
        assert_eq!(raises.load(Ordering::Relaxed), 0);
        fe.write(FE_RM_INTR_EN_HEAD_TIMING, &LAST_DATA.to_le_bytes());
        assert_eq!(fe.read32(FE_RM_INTR_DISPATCH), 1);
        assert_eq!(raises.load(Ordering::Relaxed), 1);
        assert!(!fe.dirty());
    }

    #[test]
    fn trapped_partial_ack_preserves_unwritten_pending_bits() {
        let (_intr, fe, _) = page();
        fe.update(|i| {
            i.awaken_win = 0x1040;
            i.evt[3] = LAST_DATA | 0x100;
        });
        fe.write(FE_EVT_STAT_AWAKEN_WIN, &[0x40]);
        assert_eq!(fe.read32(FE_EVT_STAT_AWAKEN_WIN), 0x1000);
        fe.write(FE_EVT_STAT_HEAD_TIMING + H3, &[LAST_DATA as u8]);
        assert_eq!(fe.irq().evt[3], 0x100);
        assert_eq!(fe.read32(FE_EVT_STAT_HEAD_TIMING + H3), 0x100 | TAG);
        assert!(!fe.dirty());
    }

    #[test]
    fn switch_needs_the_interrupt_page_and_the_display() {
        assert_eq!(parse(None, false, false), Ok(false));
        assert_eq!(parse(Some("1"), true, true), Ok(true));
        assert!(parse(Some("1"), false, true)
            .unwrap_err()
            .contains("interrupt page"));
        assert!(parse(Some("1"), true, false)
            .unwrap_err()
            .contains("--display"));
        assert!(parse(Some("yes"), true, true).is_err());
    }

    #[test]
    fn initial_page_and_the_area() {
        let (intr, fe, _) = page();
        for h in 0..MAX_HEADS as u64 {
            assert_eq!(load(&fe, FE_EVT_STAT_HEAD_TIMING + 4 * h), TAG);
            assert_eq!(load(&fe, FE_RM_INTR_EN_HEAD_TIMING + 4 * h), 0);
            assert_eq!(load(&fe, FE_RM_INTR_STAT_HEAD_TIMING + 4 * h), 0);
        }
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_WIN), 0);
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_OTHER), TAG);
        assert_eq!(load(&fe, FE_RM_INTR_STAT_CTRL_DISP), 0);
        assert_eq!(load(&fe, FE_RM_INTR_DISPATCH), 0);
        assert_eq!(fe.area(), (0x61_1000, 0x1000));
        assert!(fe.covers(0x61_1EC0) && !fe.covers(0x61_2000) && !fe.covers(0x61_0FFC));
        assert!(!fe.dirty());
        // The rest of the page is latched memory, as when trapped.
        fe.write(0x61_1D00, &0x1234u32.to_le_bytes());
        assert_eq!(fe.read32(0x61_1D00), 0x1234);
        assert!(!fe.dirty());
        // The interrupt page refuses a page it already covers or one outside BAR0.
        assert!(intr.map_page(0).is_err());
        assert!(intr.map_page(0x00B8_1000).is_err());
        assert!(intr.map_page(16 << 20).is_err());
        assert!(intr.map_page(0x61_1004).is_err());
    }

    /// The clock thread's stores: a vblank tick with the enable on shows the head in
    /// RM_INTR_STAT and DISPATCH before the raise; the guest's LAST_DATA_RESET through the
    /// page clears it; a still-pending line raises again.
    #[test]
    fn vblank_through_the_page() {
        let (_intr, fe, raises) = page();
        // The guest enables vblank on head 3 (kheadWriteVblankIntrEnable): a store.
        store(&fe, FE_RM_INTR_EN_HEAD_TIMING + H3, LAST_DATA);
        assert!(fe.scan());
        assert_eq!(fe.irq().en[3], LAST_DATA);
        assert!(fe.take_notes()[0].contains("head 3 vblank"));
        // The tick (engine.rs catch_up through `update`).
        fe.update(|i| i.evt[3] |= LAST_DATA);
        assert_eq!(load(&fe, FE_EVT_STAT_HEAD_TIMING + H3), LAST_DATA | TAG);
        assert_eq!(load(&fe, FE_RM_INTR_STAT_HEAD_TIMING + H3), LAST_DATA);
        assert_eq!(load(&fe, FE_RM_INTR_DISPATCH), 1 << 3);
        assert!(!fe.dirty());
        // kheadResetPendingLastData: reads EVT_STAT, writes LAST_DATA_RESET (0x2).
        assert_eq!(
            load(&fe, FE_EVT_STAT_HEAD_TIMING + H3) & LAST_DATA,
            LAST_DATA
        );
        store(&fe, FE_EVT_STAT_HEAD_TIMING + H3, LAST_DATA);
        assert!(fe.dirty(), "the tag makes the ack visible");
        assert!(fe.scan());
        assert_eq!(load(&fe, FE_EVT_STAT_HEAD_TIMING + H3), TAG);
        assert_eq!(load(&fe, FE_RM_INTR_DISPATCH), 0);
        assert_eq!(raises.load(Ordering::Relaxed), 0, "nothing left pending");
        // Two acks before one scan: idempotent. A tick between the guest's store and the
        // scan is cleared by that store, as hardware merges it.
        fe.update(|i| i.evt[3] |= LAST_DATA);
        store(&fe, FE_EVT_STAT_HEAD_TIMING + H3, LAST_DATA);
        store(&fe, FE_EVT_STAT_HEAD_TIMING + H3, LAST_DATA);
        fe.update(|i| i.evt[3] |= LAST_DATA);
        assert_eq!(fe.irq().evt[3], LAST_DATA);
        assert_eq!(fe.stats.evt_acks.load(Ordering::Relaxed), 2);
        assert_eq!(
            raises.load(Ordering::Relaxed),
            1,
            "asserted again after the ack: raised"
        );
        // Head 0 pending too: the ack of head 3 leaves the line asserted, one raise.
        fe.update(|i| {
            i.en[0] = LAST_DATA;
            i.evt[0] = LAST_DATA;
        });
        store(&fe, FE_EVT_STAT_HEAD_TIMING + H3, LAST_DATA);
        fe.scan();
        assert_eq!(load(&fe, FE_RM_INTR_DISPATCH), 1);
        assert_eq!(raises.load(Ordering::Relaxed), 2);
    }

    /// The marker rule: window awakens show with CORE; the guest's echo of the window bits
    /// is invisible and its CORE_RESET acks exactly the shown bits; awakens that come
    /// while a marker is outstanding wait for the next one.
    #[test]
    fn awaken_marker() {
        let (_intr, fe, raises) = page();
        fe.update(|i| i.awaken_win |= 1 << 6);
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_WIN), 0x40);
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_OTHER), AWAKEN_CORE | TAG);
        assert_eq!(load(&fe, FE_RM_INTR_STAT_CTRL_DISP), CTRL_DISP_AWAKEN);
        assert!(fe.busy(Instant::now()));
        // Window 1 flips while the marker is out: not shown yet.
        fe.update(|i| i.awaken_win |= 1 << 1);
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_WIN), 0x40);
        // kdispServiceAwakenIntr: read CTRL_DISP, read AWAKEN_WIN, echo it, read
        // AWAKEN_OTHER, write CORE_RESET.
        let seen = load(&fe, FE_EVT_STAT_AWAKEN_WIN);
        store(&fe, FE_EVT_STAT_AWAKEN_WIN, seen);
        assert!(!fe.dirty(), "the echo is invisible");
        assert_ne!(load(&fe, FE_EVT_STAT_AWAKEN_OTHER) & AWAKEN_CORE, 0);
        store(&fe, FE_EVT_STAT_AWAKEN_OTHER, AWAKEN_CORE);
        assert!(fe.dirty());
        assert!(fe.scan());
        // Window 6 is done; window 1 is shown under a new marker, with a raise.
        assert_eq!(fe.irq().awaken_win, 1 << 1);
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_WIN), 0x2);
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_OTHER), AWAKEN_CORE | TAG);
        assert_eq!(raises.load(Ordering::Relaxed), 1);
        assert_eq!(fe.stats.markers.load(Ordering::Relaxed), 2);
        // Its pass: echo, CORE_RESET; nothing left.
        store(&fe, FE_EVT_STAT_AWAKEN_WIN, 0x2);
        store(&fe, FE_EVT_STAT_AWAKEN_OTHER, AWAKEN_CORE);
        fe.scan();
        assert_eq!(fe.irq().awaken_win, 0);
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_WIN), 0);
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_OTHER), TAG);
        assert_eq!(load(&fe, FE_RM_INTR_STAT_CTRL_DISP), 0);
        assert!(!fe.busy(Instant::now()));
        assert_eq!(fe.stats.marker_acks.load(Ordering::Relaxed), 2);
        assert_eq!(raises.load(Ordering::Relaxed), 1);
        // The core notifier's own awaken (WRITE_AWAKEN) is the same bit: one marker.
        fe.update(|i| i.awaken_other |= AWAKEN_CORE);
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_WIN), 0);
        assert_eq!(load(&fe, FE_RM_INTR_STAT_CTRL_DISP), CTRL_DISP_AWAKEN);
        store(&fe, FE_EVT_STAT_AWAKEN_OTHER, AWAKEN_CORE);
        fe.scan();
        assert_eq!(fe.irq().awaken_other, 0);
        assert_eq!(fe.summary().matches("inconsistencies: none").count(), 1);
    }

    /// The residual window of the marker rule and the other fail-closed cases: each is
    /// counted and restored, none changes the status.
    #[test]
    fn fail_closed_cases() {
        let (_intr, fe, _) = page();
        // A second pass echoes an earlier marker's bits after a new marker is shown.
        fe.update(|i| i.awaken_win |= 1 << 6);
        store(&fe, FE_EVT_STAT_AWAKEN_OTHER, AWAKEN_CORE);
        fe.scan();
        fe.update(|i| i.awaken_win |= 1 << 2);
        store(&fe, FE_EVT_STAT_AWAKEN_WIN, 0x40);
        fe.scan();
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_WIN), 0x4, "restored");
        assert_eq!(fe.irq().awaken_win, 0x4);
        // Read-only registers restored.
        store(&fe, FE_RM_INTR_DISPATCH, 0xFF);
        store(&fe, FE_RM_INTR_STAT_CTRL_DISP, 0);
        store(&fe, FE_RM_INTR_STAT_HEAD_TIMING, 0x2);
        fe.scan();
        assert_eq!(load(&fe, FE_RM_INTR_DISPATCH), 0);
        assert_eq!(load(&fe, FE_RM_INTR_STAT_CTRL_DISP), CTRL_DISP_AWAKEN);
        assert_eq!(load(&fe, FE_RM_INTR_STAT_HEAD_TIMING), 0);
        // An AWAKEN_OTHER store without CORE_RESET: restored, the marker stays.
        store(&fe, FE_EVT_STAT_AWAKEN_OTHER, 0);
        fe.scan();
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_OTHER), AWAKEN_CORE | TAG);
        assert_eq!(fe.irq().awaken_win, 0x4);
        // A tag in an ack: the rest applies.
        fe.update(|i| i.evt[1] |= LAST_DATA | 0x4);
        store(&fe, FE_EVT_STAT_HEAD_TIMING + 4, LAST_DATA | TAG);
        fe.scan();
        assert_eq!(fe.irq().evt[1], 0x4, "VBLANK stays");
        // CORE_RESET with no marker: ignored.
        store(&fe, FE_EVT_STAT_AWAKEN_OTHER, AWAKEN_CORE);
        fe.scan();
        store(&fe, FE_EVT_STAT_AWAKEN_OTHER, AWAKEN_CORE);
        fe.scan();
        let odd: Vec<u64> = fe
            .stats
            .odd
            .iter()
            .map(|a| a.load(Ordering::Relaxed))
            .collect();
        assert_eq!(odd, vec![1, 1, 1, 1, 1]);
        assert_eq!(fe.take_notes().len(), 5, "each logged once");
        // A new engine (device reset) takes the page back to its status.
        fe.attach(FeIrq::new(4));
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_OTHER), TAG);
        assert_eq!(load(&fe, FE_EVT_STAT_AWAKEN_WIN), 0);
        assert!(!fe.dirty());
    }

    /// A guest store between the scan's snapshot and its write-back is kept and applied
    /// by the next scan; the watchdog raises a marker nobody acks, once per period.
    #[test]
    fn racing_store_and_watchdog() {
        let (_intr, fe, raises) = page();
        fe.update(|i| {
            i.en[3] = LAST_DATA;
            i.evt[3] = LAST_DATA;
        });
        // A store to head 2's enable starts the scan; the ack lands during it.
        store(&fe, FE_RM_INTR_EN_HEAD_TIMING + 8, LAST_DATA);
        fe.scan_racing(&mut || store(&fe, FE_EVT_STAT_HEAD_TIMING + H3, LAST_DATA));
        assert_eq!(fe.irq().en[2], LAST_DATA);
        assert_eq!(fe.irq().evt[3], LAST_DATA, "not applied yet");
        assert_eq!(load(&fe, FE_EVT_STAT_HEAD_TIMING + H3), LAST_DATA, "kept");
        assert!(fe.dirty());
        fe.scan();
        assert_eq!(fe.irq().evt[3], 0);
        // Watchdog.
        fe.update(|i| i.awaken_win |= 1);
        let t0 = Instant::now();
        assert!(!fe.tick(t0));
        assert!(fe.tick(t0 + Duration::from_millis(10)));
        assert!(!fe.tick(t0 + Duration::from_millis(15)));
        assert!(fe.tick(t0 + Duration::from_millis(20)));
        assert_eq!(raises.load(Ordering::Relaxed), 2);
        assert!(!fe.busy(t0 + MARKER_SPIN + Duration::from_millis(1)));
        store(&fe, FE_EVT_STAT_AWAKEN_OTHER, AWAKEN_CORE);
        fe.scan();
        assert!(!fe.tick(t0 + Duration::from_secs(1)));
    }

    /// TU106's engine models exactly the watched words of the page; nothing else there.
    #[test]
    fn tu106_page_check() {
        let f = crate::fixture::rtx2070();
        let boot = crate::boot::BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(4 << 30),
            f.identity(4 << 30),
        );
        FePage::check(&super::super::engine::Engine::handles, &|off| {
            boot.owns_register(off)
        })
        .unwrap();
        assert!(FePage::check(&|off| off == FE_RM_INTR_DISPATCH, &|_| false).is_err());
        assert!(
            FePage::check(&super::super::engine::Engine::handles, &|off| off
                == 0x61_1004)
            .is_err()
        );
    }
}

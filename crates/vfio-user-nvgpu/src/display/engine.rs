// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Phase C of docs/caraxes/display-connector.md: what the display engine does with the
//! methods, and the per-head clock (docs/caraxes/display-cd.md).
//!
//! - The core channel's `UPDATE` applies the ARMED state: window owners, per-head raster
//!   and pixel clock (the tick period), SOR owners (head active), and writes the core
//!   notifier FINISHED at once (NVKMS polls it in sysmem), raising the core AWAKEN when
//!   the notifier mode asks for it. `FE_CORE_HEAD_STATE` follows the head.
//! - A window channel's `UPDATE` queues a presentation: the context DMAs of its notifier,
//!   semaphore and surface are resolved through RAMHT in the display instance memory
//!   (`disp_inst_mem_0300.c`), so later writes need no guest state. The per-head clock
//!   latches it at the next tick (display-connector.md section 2.2): acquire semaphore
//!   satisfied, `MIN_PRESENT_INTERVAL` ticks since the last latch, PTIMER at or past the
//!   `SET_UPDATE_TIMESTAMP` when `TIMESTAMP_MODE` is enabled; then the previous
//!   presentation's release semaphore (flip-away), the notifier BEGUN with a PTIMER
//!   timestamp, and `AWAKEN_WIN`. `BEGIN_MODE_IMMEDIATE` latches at the `UPDATE`.
//! - Every tick advances `RG_DPCA` and the LOADV counter and sets `LAST_DATA`; with the
//!   RM's enable that is the vblank interrupt (dev_disp.h v03/v04, kern_disp_0300.c).
//! - The clock is the device clock (`channel::device_epoch`, the PTIMER's base), with
//!   absolute deadlines `t0 + n * period`. A
//!   ticker thread only raises the DISP vector at the deadlines while a head has work
//!   (a pending presentation, or vblank enabled); the state advances on the server
//!   thread when the guest's interrupt handler reads the display registers
//!   ([`Engine::catch_up`]), so notifier writes and interrupt bits appear in order.
//!
//! Nothing scans out: the latched surface descriptor is kept for the evidence and a
//! later sink (phase E).

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use super::fe_page::{FeIrq, FePage};
use crate::fb::FakeFb;
use crate::guestmem::GuestMemory;
use crate::intr::IntrShared;

/// Where the display's interrupt status lives.
enum IrqHome {
    /// In the engine: the guest's accesses trap (the default).
    Local(FeIrq),
    /// In the display page, shared with the reactor (`CARAXES_DISPLAY_PAGE=1`).
    Page(Arc<FePage>),
}

/// Heads the register file covers (`__SIZE_1` of the head-indexed registers).
pub const MAX_HEADS: usize = 8;
/// Windows the AWAKEN register covers.
pub const MAX_WINDOWS: usize = 32;

// BAR0 (disp/v03_00/dev_disp.h, v04_00/dev_disp.h).
pub const FE_EVT_STAT_HEAD_TIMING: u64 = 0x0061_1800;
pub const FE_RM_INTR_STAT_HEAD_TIMING: u64 = 0x0061_1C00;
pub const FE_RM_INTR_EN_HEAD_TIMING: u64 = 0x0061_1D80;
pub const FE_RM_INTR_DISPATCH: u64 = 0x0061_1EC0;
pub const FE_RM_INTR_STAT_CTRL_DISP: u64 = 0x0061_1C30;
pub const FE_EVT_STAT_AWAKEN_WIN: u64 = 0x0061_1858;
pub const FE_EVT_STAT_AWAKEN_OTHER: u64 = 0x0061_185C;
pub const FE_CORE_HEAD_STATE: u64 = 0x0061_2078;
pub const RG_DPCA: u64 = 0x0061_6330;
pub const POSTCOMP_HEAD_LOADV_COUNTER: u64 = 0x0061_611C;
const HEAD_STRIDE: u64 = 0x800;
/// `_LAST_DATA` 1:1 in EVT_STAT, RM_INTR_STAT and RM_INTR_EN of HEAD_TIMING.
pub const LAST_DATA: u32 = 1 << 1;
/// `FE_RM_INTR_STAT_CTRL_DISP_AWAKEN` 8:8.
pub const CTRL_DISP_AWAKEN: u32 = 1 << 8;
/// `FE_CORE_HEAD_STATE_OPERATING_MODE` 9:8 = AWAKE.
pub const HEAD_STATE_AWAKE: u32 = 2 << 8;

// NV_DISP_NOTIFIER (clc57d.h:34-54): 16 bytes; word 0 PRESENT_COUNT 7:0, FIELD 8:8,
// FLIP_TYPE 9:9, STATUS 31:30 (NOT_BEGUN 0, BEGUN 1, FINISHED 2); TIMESTAMP_LO/HI in
// words 2-3. The same four-word record is the window semaphore's (FOUR_WORD_NVDISPLAY,
// nvkms-api-types.h:174-177): payload in word 0 (nvkms-sync.c:190-193).
//
// A core notifier completes FINISHED (NVKMS waits for it, nvkms-evo3.c:6330-6344). A
// window flip's notifier goes BEGUN when the flip latches and FINISHED when the first
// frame from the new surface has been scanned out (the next vblank). The class gives the
// states, not their timing; both drivers fix it:
// - NVKMS resets the notifier NOT_BEGUN and "when EVO performs the flip, it changes the
//   notifier to BEGUN", and tests the newest flip for BEGUN at the following vblank
//   callback (nvkms-headsurface.c:1896-1922); its client-notifier emulation writes BEGUN
//   with the timestamp (1996-2013, nvkms-sync.c:77-91);
// - the Windows 617 KMD pre-sets its records FINISHED, resets one to 0 before a flip and
//   waits for FINISHED: after a modeset's single flip it unblanks the head once FINISHED is
//   written (C49-C52), and never with BEGUN alone (C53, C54: TDR).
// PRESENT_COUNT and FIELD stay 0 (progressive scanout, as NVKMS's emulation writes).
const NOTIFIER_BEGUN: u32 = 1 << 30;
const NOTIFIER_FINISHED: u32 = 2 << 30;
const NOTIFIER_FLIP_TYPE_IMMEDIATE: u32 = 1 << 9;

/// A display timestamp: the PTIMER value at `ns` of the device clock. `NV_PTIMER_TIME_0`
/// counts in NSEC 31:5 (gm107/dev_timer.h), so the low five bits are 0 and the low word
/// is never 1, the value NVKMS resets a notifier's TIMESTAMP_LO to as "not yet written"
/// (nvkms-sync.c:29-37, 62-66).
pub fn ptimer_stamp(ns: u64) -> u64 {
    ns & !0x1F
}

// Channel numbers (NV_PDISP_CHN_NUM_*), the RAMHT key.
pub const CHN_CORE: u32 = 0;
pub fn chn_win(w: u32) -> u32 {
    1 + w
}

// Core methods (clc57d.h).
const C_SET_CONTEXT_DMA_NOTIFIER: u32 = 0x208;
const C_SET_NOTIFIER_CONTROL: u32 = 0x20C;
const C_SOR_SET_CONTROL: u32 = 0x300;
const C_WINDOW_SET_CONTROL: u32 = 0x1000;
const C_HEAD: u32 = 0x2000;
const C_HEAD_STRIDE: u32 = 0x400;
const C_HEAD_PIXEL_CLOCK: u32 = 0x0C;
const C_HEAD_VIEWPORT_SIZE_IN: u32 = 0x4C;
const C_HEAD_RASTER_SIZE: u32 = 0x64;
const C_HEAD_RASTER_BLANK_END: u32 = 0x6C;
const C_HEAD_RASTER_BLANK_START: u32 = 0x70;
const C_HEAD_OLUT_CONTROL: u32 = 0x280;
const C_HEAD_CONTEXT_DMA_OLUT: u32 = 0x288;

// Window methods (clc57e.h).
const W_SET_SEMAPHORE_CONTROL: u32 = 0x20C;
const W_SET_SEMAPHORE_ACQUIRE: u32 = 0x210;
const W_SET_SEMAPHORE_RELEASE: u32 = 0x214;
const W_SET_CONTEXT_DMA_SEMAPHORE: u32 = 0x218;
const W_SET_CONTEXT_DMA_NOTIFIER: u32 = 0x21C;
const W_SET_NOTIFIER_CONTROL: u32 = 0x220;
const W_SET_SIZE: u32 = 0x224;
const W_SET_STORAGE: u32 = 0x228;
const W_SET_PARAMS: u32 = 0x22C;
const W_SET_PLANAR_STORAGE: u32 = 0x230;
const W_SET_CONTEXT_DMA_ISO: u32 = 0x240;
const W_SET_OFFSET: u32 = 0x260;
const W_SET_PRESENT_CONTROL: u32 = 0x308;
const W_SET_UPDATE_TIMESTAMP_LO: u32 = 0x348;
const W_SET_UPDATE_TIMESTAMP_HI: u32 = 0x34C;
/// `SET_PRESENT_CONTROL_TIMESTAMP_MODE` 8:8 (clc57e.h:270-272): ENABLE holds the flip
/// until PTIMER >= `SET_UPDATE_TIMESTAMP` ("When non-zero, block the flip until PTIMER >=
/// timeStamp", nvkms-api.h:925-926; programmed so in nvkms-evo3.c:3772-3777, 3904-3906).
const PRESENT_TIMESTAMP_MODE_ENABLE: u32 = 1 << 8;

// RAMHT and context DMA objects (disp/v03_00/dev_disp.h NV_UDISP_*, NV_DMA_*).
const HASH_LIMIT: u64 = 0x2000;
const HASH_ENTRY: u64 = 8;
/// Recent display events kept (`Engine::recent`).
const RECENT: usize = 96;

// -------------------------------------------------------------------------------------
// The clock and the ticker thread.

/// The engine under the display lock (a poisoned lock is taken over: the engine's state
/// stays consistent between statements).
pub fn lock(e: &Mutex<Engine>) -> std::sync::MutexGuard<'_, Engine> {
    e.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Nanoseconds of the display clock: the device clock, PTIMER's base
/// ([`crate::channel::device_now_ns`]), so notifier timestamps and the guest's PTIMER
/// reads compare.
pub fn now_ns() -> u64 {
    crate::channel::device_now_ns()
}

#[derive(Default)]
struct TickHead {
    period_ns: AtomicU64,
    t0_ns: AtomicU64,
    /// The head has work at its ticks: a pending presentation or vblank enabled.
    want: AtomicBool,
    /// Vblank enabled through the display page (fe_page.rs), where the guest's enable
    /// store reaches the reactor, not the engine.
    vblank: AtomicBool,
}

/// State shared with the ticker thread; the server thread writes it.
#[derive(Default)]
pub struct Ticker {
    heads: [TickHead; MAX_HEADS],
    vector: AtomicU32,
    intr: Mutex<Option<Arc<IntrShared>>>,
    thread: Mutex<Option<std::thread::Thread>>,
    /// Host-side connector commands (phase D): a file the ticker polls, the command
    /// read from it, and the vector that makes the guest read the GSP queue.
    control: Mutex<Option<std::path::PathBuf>>,
    command: Mutex<Option<String>>,
    command_pending: AtomicBool,
    gsp_vector: AtomicU32,
    /// Vectors the ticker raised, and MSIs written for them.
    pub raised: AtomicU64,
    pub msis: AtomicU64,
    /// The display page (`CARAXES_DISPLAY_PAGE=1`, fe_page.rs): the guest's reads no
    /// longer trap, so the clock advances the engine itself at every tick through this
    /// (main.rs: lock the device, `Device::display_tick`) before it raises the vector.
    driver: Mutex<Option<Arc<Driver>>>,
    /// Ticks handed to the driver.
    pub driven: AtomicU64,
    /// When the ticks fired against when they were due, and how long the driver took
    /// (the device lock and the engine's catch-up): docs/caraxes/intr-page.md §11.
    timing: Mutex<TickTiming>,
    /// [`now_ns`] of the last raise not yet seen by the guest's ack (display page) and by
    /// the reactor's wake-up (0: none), for the delivery histograms (fe_page.rs,
    /// intr_page.rs), and the next tick the clock will fire (0: none due).
    raise_for_ack: AtomicU64,
    raise_for_wake: AtomicU64,
    next_due: AtomicU64,
}

/// A log2 histogram of microseconds: bucket `b` holds values in `[2^(b-1), 2^b)`, bucket 0
/// the value 0.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hist {
    pub counts: [u64; 32],
    pub n: u64,
    pub max_us: u64,
    /// Values of a millisecond or more.
    pub over_ms: u64,
}

impl Hist {
    pub fn add(&mut self, us: u64) {
        let b = (64 - us.leading_zeros() as usize).min(31);
        self.counts[b] += 1;
        self.n += 1;
        self.max_us = self.max_us.max(us);
        if us >= 1000 {
            self.over_ms += 1;
        }
    }

    /// The upper bound of the bucket that holds quantile `q` (0 to 1), in microseconds,
    /// capped at the largest value seen.
    pub fn quantile(&self, q: f64) -> u64 {
        if self.n == 0 {
            return 0;
        }
        let want = ((self.n as f64) * q).ceil().max(1.0) as u64;
        let mut seen = 0;
        for (b, &c) in self.counts.iter().enumerate() {
            seen += c;
            if seen >= want {
                let upper = if b == 0 { 0 } else { (1u64 << b) - 1 };
                return upper.min(self.max_us);
            }
        }
        self.max_us
    }

    pub fn describe(&self) -> String {
        format!(
            "p50 <= {} us, p99 <= {} us, max {} us, {} of {} at 1 ms or more",
            self.quantile(0.5),
            self.quantile(0.99),
            self.max_us,
            self.over_ms,
            self.n
        )
    }
}

/// The display clock's own timing: how late each firing tick ran against the tick it
/// served, and how long the driver held it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TickTiming {
    pub late: Hist,
    pub driver: Hist,
    /// The engine's catch-up inside the driver (display/mod.rs `Display::tick`); the
    /// rest of the driver's time is the wait for the device lock and the trace.
    pub catch_up: Hist,
}

/// What the display clock runs at a tick when the engine does not advance on reads.
/// The display clock's driver (main.rs: the device's display tick): advances the engine
/// to now and returns whether the display interrupt line is asserted afterwards.
pub type Driver = dyn Fn() -> bool + Send + Sync;

static TICKER: OnceLock<Arc<Ticker>> = OnceLock::new();

pub fn ticker() -> &'static Arc<Ticker> {
    TICKER.get_or_init(|| Arc::new(Ticker::default()))
}

impl Ticker {
    /// Attach the interrupt path and start the thread (once).
    pub fn attach(self: &Arc<Self>, intr: Arc<IntrShared>, vector: u32) {
        self.vector.store(vector, Ordering::Relaxed);
        *self.intr.lock().unwrap() = Some(intr);
        let mut t = self.thread.lock().unwrap();
        if t.is_none() {
            let me = self.clone();
            let h = std::thread::Builder::new()
                .name("display-ticker".into())
                .spawn(move || me.run())
                .expect("spawn the display ticker");
            *t = Some(h.thread().clone());
        }
    }

    /// Poll `path` for connector commands (`edid-alt`, `edid-main`, `unplug`, `plug`);
    /// `gsp_vector` is raised so that the guest services the GSP queue.
    pub fn set_control(&self, path: std::path::PathBuf, gsp_vector: u32) {
        let _ = std::fs::remove_file(&path);
        *self.control.lock().unwrap() = Some(path);
        self.gsp_vector.store(gsp_vector, Ordering::Relaxed);
    }

    /// Advance the engine at every tick from now on (the display page, fe_page.rs).
    pub fn set_driver(&self, driver: Arc<Driver>) {
        *self.driver.lock().unwrap() = Some(driver);
    }

    /// Vblank enabled or disabled on head `h` by a guest store in the display page.
    pub fn set_vblank(&self, h: usize, on: bool) {
        if let Some(t) = self.heads.get(h) {
            if t.vblank.swap(on, Ordering::SeqCst) != on && on {
                self.wake();
            }
        }
    }

    /// The tick's work. With a driver it advances the engine (the page is written) and the
    /// vector is raised while the display's interrupt line is asserted, as the card's FE
    /// raises its tree leaf: a `LAST_DATA` the guest left pending (vblank callbacks still
    /// queued) is raised again at the next frame; a tick with nothing pending raises
    /// nothing (a leaf pending without an FE cause is one the guest's display handler never
    /// acknowledges: C64, a re-arm storm after an FLR). Without a driver the tick raises and
    /// the guest's trapped status read advances the engine (`Display::read32_io`).
    fn fire(&self) {
        let d = self.driver.lock().unwrap().clone();
        let asserted = match d {
            Some(d) => {
                self.driven.fetch_add(1, Ordering::Relaxed);
                let t0 = now_ns();
                let asserted = d();
                let us = now_ns().saturating_sub(t0) / 1000;
                self.timing.lock().unwrap().driver.add(us);
                asserted
            }
            None => true,
        };
        if asserted {
            self.raise();
        }
    }

    /// The first raise the guest has not acknowledged yet ([`now_ns`]), taken.
    pub fn take_raise_for_ack(&self) -> Option<u64> {
        Some(self.raise_for_ack.swap(0, Ordering::AcqRel)).filter(|&t| t != 0)
    }

    /// The first raise since the reactor last woke for one ([`now_ns`]), taken.
    pub fn take_raise_for_wake(&self) -> Option<u64> {
        Some(self.raise_for_wake.swap(0, Ordering::AcqRel)).filter(|&t| t != 0)
    }

    /// The next tick the clock will fire ([`now_ns`]), if a head has work.
    pub fn next_due(&self) -> Option<u64> {
        Some(self.next_due.load(Ordering::Acquire)).filter(|&t| t != 0)
    }

    /// One catch-up of the engine run by the driver took `us` microseconds.
    pub fn note_catch_up(&self, us: u64) {
        self.timing.lock().unwrap().catch_up.add(us);
    }

    /// The clock's timing so far (for the summary and the tests).
    pub fn timing(&self) -> TickTiming {
        self.timing.lock().unwrap().clone()
    }

    /// The summary of the clock's timing, empty before the first tick.
    pub fn timing_summary(&self) -> String {
        let t = self.timing();
        if t.late.n == 0 {
            return String::new();
        }
        let mut s = format!(
            "display clock: ticks fired late against their due time: {}",
            t.late.describe()
        );
        if t.driver.n > 0 {
            s.push_str(&format!(
                "; driver (device lock and catch-up): {}",
                t.driver.describe()
            ));
        }
        if t.catch_up.n > 0 {
            s.push_str(&format!("; catch-up alone: {}", t.catch_up.describe()));
        }
        s
    }

    /// Whether a connector command waits for the server thread (cheap).
    pub fn command_pending(&self) -> bool {
        self.command_pending.load(Ordering::Acquire)
    }

    /// The directory of the control file (the device's state directory).
    pub fn control_dir(&self) -> Option<std::path::PathBuf> {
        self.control.lock().unwrap().as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()))
    }

    pub fn take_command(&self) -> Option<String> {
        self.command_pending.store(false, Ordering::Release);
        self.command.lock().unwrap().take()
    }

    /// Raise the GSP vector (the device posted an event on the status queue).
    pub fn raise_gsp(&self) -> bool {
        self.intr()
            .is_some_and(|i| i.raise(self.gsp_vector.load(Ordering::Relaxed)))
    }

    fn poll_control(&self) {
        let path = self.control.lock().unwrap().clone();
        let Some(path) = path else { return };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let _ = std::fs::remove_file(&path);
        *self.command.lock().unwrap() = Some(text);
        self.command_pending.store(true, Ordering::Release);
        // Any register read of the guest makes the server thread act on it; the display
        // vector gets the guest's attention even when nothing else runs. With the display
        // page those reads do not trap: the driver acts on it.
        self.fire();
    }

    pub fn intr(&self) -> Option<Arc<IntrShared>> {
        self.intr.lock().unwrap().clone()
    }

    fn set_head(&self, h: usize, period_ns: u64, t0_ns: u64) {
        if let Some(t) = self.heads.get(h) {
            t.t0_ns.store(t0_ns, Ordering::SeqCst);
            t.period_ns.store(period_ns, Ordering::SeqCst);
        }
        self.wake();
    }

    fn set_want(&self, h: usize, want: bool) {
        if let Some(t) = self.heads.get(h) {
            if t.want.swap(want, Ordering::SeqCst) != want && want {
                self.wake();
            }
        }
    }

    fn wake(&self) {
        if let Some(t) = self.thread.lock().unwrap().as_ref() {
            t.unpark();
        }
    }

    /// Raise the DISP vector (from any thread).
    pub fn raise(&self) {
        let now = now_ns().max(1);
        let _ = self
            .raise_for_ack
            .compare_exchange(0, now, Ordering::AcqRel, Ordering::Acquire);
        let _ = self
            .raise_for_wake
            .compare_exchange(0, now, Ordering::AcqRel, Ordering::Acquire);
        if let Some(i) = self.intr() {
            self.raised.fetch_add(1, Ordering::Relaxed);
            if i.raise(self.vector.load(Ordering::Relaxed)) {
                self.msis.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn run(&self) {
        let mut last = [u64::MAX; MAX_HEADS];
        let mut polled = 0u64;
        loop {
            let now = now_ns();
            if now >= polled + 100_000_000 {
                polled = now;
                self.poll_control();
            }
            let mut next: Option<u64> = None;
            let mut fire = false;
            let mut due = 0u64;
            for (h, t) in self.heads.iter().enumerate() {
                let p = t.period_ns.load(Ordering::SeqCst);
                let work = t.want.load(Ordering::SeqCst) || t.vblank.load(Ordering::SeqCst);
                if p == 0 || !work {
                    continue;
                }
                let t0 = t.t0_ns.load(Ordering::SeqCst);
                if now < t0 {
                    next = Some(next.map_or(t0, |n: u64| n.min(t0)));
                    continue;
                }
                let n = (now - t0) / p;
                if n != last[h] && n > 0 {
                    // A tick passed since the last raise for this head.
                    if last[h] != u64::MAX || n > 0 {
                        fire = true;
                        due = due.max(t0 + n * p);
                    }
                    last[h] = n;
                }
                let d = t0 + (n + 1) * p;
                next = Some(next.map_or(d, |x: u64| x.min(d)));
            }
            if fire {
                let late = now.saturating_sub(due) / 1000;
                self.timing.lock().unwrap().late.add(late);
                self.fire();
            }
            let now = now_ns();
            self.next_due.store(next.unwrap_or(0), Ordering::Release);
            match next {
                Some(d) if d > now => std::thread::park_timeout(Duration::from_nanos(d - now)),
                Some(_) => {}
                None => std::thread::park_timeout(Duration::from_millis(50)),
            }
        }
    }
}

/// `off` is the per-head register `base + HEAD_STRIDE * h` of some head h.
fn on_head_page(off: u64, base: u64) -> bool {
    off >= base && off < base + HEAD_STRIDE * MAX_HEADS as u64 && (off - base) % HEAD_STRIDE == 0
}

// -------------------------------------------------------------------------------------
// Memory the engine writes: FB (context DMA target NVM) or guest sysmem (PCI).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemRef {
    /// `NV_DMA_TARGET_NODE`: 1 physical NVM (FB), 2/3 physical PCI (sysmem).
    pub target: u32,
    pub addr: u64,
}

/// Where the engine reads and writes its records (context DMA targets: FB or guest
/// sysmem). The device path goes through the FB store and guest memory under the device
/// mutex ([`DeviceIo`]); the display clock's own path through a guest memory view and
/// pinned FB windows without it (`display::CoreIo`, docs/caraxes/display-own-clock.md §2).
pub trait RecordIo {
    /// Whether `len` bytes at `m` + `off` can be read and written now; a latch or a
    /// FINISHED whose records cannot waits for a path that can.
    fn can(&self, _m: MemRef, _off: u64, _len: u64) -> bool {
        true
    }
    fn read(&self, m: MemRef, off: u64, buf: &mut [u8]) -> bool;
    fn write(&mut self, m: MemRef, off: u64, data: &[u8]) -> bool;
    /// The path's name for the engine's recent events.
    fn path(&self) -> &'static str {
        "device"
    }
}

/// The device path: the FB store and guest memory, under the device mutex.
pub struct DeviceIo<'a> {
    pub fb: &'a mut FakeFb,
    pub mem: &'a GuestMemory,
}

impl RecordIo for DeviceIo<'_> {
    fn read(&self, m: MemRef, off: u64, buf: &mut [u8]) -> bool {
        if m.target == 1 {
            self.fb.read(m.addr + off, buf)
        } else {
            self.mem.read(m.addr + off, buf)
        }
    }

    fn write(&mut self, m: MemRef, off: u64, data: &[u8]) -> bool {
        if m.target == 1 {
            self.fb.write(m.addr + off, data)
        } else {
            self.mem.write(m.addr + off, data)
        }
    }
}

impl MemRef {
    /// The four words of the 16-byte record (NV_DISP_NOTIFIER layout), for the trace.
    fn record(&self, io: &dyn RecordIo) -> Option<[u32; 4]> {
        let mut b = [0u8; 16];
        io.read(*self, 0, &mut b)
            .then(|| std::array::from_fn(|i| u32::from_le_bytes(b[4 * i..4 * i + 4].try_into().unwrap())))
    }

    fn read32(&self, io: &dyn RecordIo) -> Option<u32> {
        let mut b = [0u8; 4];
        io.read(*self, 0, &mut b).then(|| u32::from_le_bytes(b))
    }

    fn write(&self, io: &mut dyn RecordIo, off: u64, data: &[u8]) -> bool {
        io.write(*self, off, data)
    }

    pub fn describe(&self) -> String {
        format!(
            "{} {:#x}",
            if self.target == 1 { "fb" } else { "sys" },
            self.addr
        )
    }
}

/// A context DMA object resolved from the instance memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CtxDma {
    pub target: u32,
    pub base: u64,
    pub limit: u64,
    pub block_linear: bool,
}

impl CtxDma {
    fn at(&self, off: u64) -> Option<MemRef> {
        (self.base + off <= self.limit).then_some(MemRef {
            target: self.target,
            addr: self.base + off,
        })
    }
}

/// Look `handle` up for channel number `chn` in the RAMHT at `inst` (FB), and decode
/// the object it points to. The hash (`instmemHashFunc_v03_00`) needs the client,
/// which the device does not know; the table is small (1024 entries), so it is
/// scanned for the handle with this channel.
pub fn resolve(fb: &FakeFb, inst: u64, handle: u32, chn: u32) -> Option<CtxDma> {
    if handle == 0 {
        return None;
    }
    let mut table = vec![0u8; HASH_LIMIT as usize];
    if !fb.read(inst, &mut table) {
        return None;
    }
    for e in 0..(HASH_LIMIT / HASH_ENTRY) as usize {
        let o = e * HASH_ENTRY as usize;
        let h = u32::from_le_bytes(table[o..o + 4].try_into().unwrap());
        let ctx = u32::from_le_bytes(table[o + 4..o + 8].try_into().unwrap());
        let instance = (ctx >> 14) & 0x7FF;
        if h != handle || (ctx >> 25) != chn || instance == 0 {
            continue;
        }
        let mut obj = [0u8; 20];
        if !fb.read(inst + ((instance as u64) << 5), &mut obj) {
            return None;
        }
        let w = |i: usize| u32::from_le_bytes(obj[4 * i..4 * i + 4].try_into().unwrap()) as u64;
        let base = (((w(2) & 0x7F) << 32) | w(1)) << 8;
        let limit = ((((w(4) & 0x7F) << 32) | w(3)) << 8) | 0xFF;
        return Some(CtxDma {
            target: (w(0) & 3) as u32,
            base,
            limit,
            block_linear: (w(0) >> 20) & 1 == 1,
        });
    }
    None
}

// -------------------------------------------------------------------------------------
// Heads, windows, presentations.

#[derive(Clone, Debug, Default)]
pub struct Head {
    pub active: bool,
    pub pclk_hz: u64,
    pub htotal: u32,
    pub vtotal: u32,
    pub width: u32,
    pub height: u32,
    pub period_ns: u64,
    pub t0_ns: u64,
    /// Ticks processed since t0.
    pub ticks: u64,
    /// `RG_DPCA` frame count and the LOADV counter.
    pub frames: u32,
    pub loadv: u32,
    pub olut: (u32, u32),
    /// Modesets (activations or period changes) of this head.
    pub modesets: u32,
    /// The raster's vertical blank: the line where the active region ends
    /// (`HEAD_SET_RASTER_BLANK_START` Y) and where it begins (`HEAD_SET_RASTER_BLANK_END` Y),
    /// clc57d.h; 0 when unknown.
    pub vblank_start: u32,
    pub vblank_end: u32,
    /// `SET_GET_BLANKING_CTRL`: the head's output is blanked (core channel PIO register,
    /// clc57d.h:291-297).
    pub blanked: bool,
    /// Tick-rate report: ticks and time at the last report.
    report: (u64, u64),
}

impl Head {
    pub fn hz(&self) -> f64 {
        if self.period_ns == 0 {
            0.0
        } else {
            1e9 / self.period_ns as f64
        }
    }
}

/// What a window scans out (for the evidence and a later sink).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Surface {
    pub mem: Option<MemRef>,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    /// `SET_PLANAR_STORAGE` pitch: 64-byte units (pitch) or blocks (block-linear).
    pub pitch: u32,
    pub pitch_layout: bool,
    pub block_height_log2: u32,
}

#[derive(Clone, Debug)]
pub struct Flip {
    pub queued_ns: u64,
    pub notifier: Option<(MemRef, bool)>,
    pub acquire: Option<(MemRef, u32)>,
    pub release: Option<(MemRef, u32)>,
    pub min_interval: u32,
    pub immediate: bool,
    /// `TIMESTAMP_MODE` enabled: the PTIMER time the flip waits for.
    pub not_before_ns: Option<u64>,
    pub surface: Surface,
}

#[derive(Clone, Debug, Default)]
pub struct Window {
    pub head: Option<usize>,
    pub asm: BTreeMap<u32, u32>,
    pub pending: VecDeque<Flip>,
    pub last_latch_tick: Option<u64>,
    pub prev_release: Option<(MemRef, u32)>,
    /// The latched flip's notifier and word 0, BEGUN until its first frame has been
    /// scanned out: FINISHED at the next tick.
    pub begun: Option<(MemRef, u32)>,
    pub latched: Surface,
    pub updates: u64,
    pub flips: u64,
    pub acquire_waits: u64,
    /// Ticks a flip waited for its `SET_UPDATE_TIMESTAMP`.
    pub timestamp_waits: u64,
    /// The tick the front flip's acquire first failed at (the trace names long waits).
    pub acquire_since: Option<u64>,
}

/// The display engine's method and timing state.
pub struct Engine {
    pub heads: Vec<Head>,
    pub windows: Vec<Window>,
    /// The interrupt status: here, or in the display page (fe_page.rs).
    irq: IrqHome,
    /// Display instance memory base in FB (from `WRITE_INST_MEM`).
    pub inst: Option<u64>,
    /// The ARMED `SOR_SET_CONTROL(s)` OWNER_MASK (7:0, clc57d.h) of SORs 0-7: the heads
    /// each SOR is driven by since the last core `UPDATE`.
    pub sor_owners: [u32; 8],
    pub core_updates: u64,
    pub core_notifier_writes: u64,
    pub vblank_raised: u64,
    pub unresolved: u64,
    pub notes: Vec<String>,
    /// FB record windows for the path without the device mutex (display/core_path.rs).
    pub pins: Arc<super::core_path::RecordPins>,
    /// Display clock ticks and guest register reads and writes served without the device
    /// mutex (display/core_path.rs).
    pub core_ticks: u64,
    pub routed_reads: u64,
    pub routed_writes: u64,
    /// The last latches, FINISHED writes, acquire waits and failed record writes with the
    /// path that did them, for the trace at a reset and at exit (what the display did
    /// before a TDR, whatever the trace's latch rule left out).
    pub recent: VecDeque<String>,
    pub record_write_failures: u64,
}

impl Engine {
    pub fn new(heads: usize, windows: usize) -> Self {
        let e = Engine {
            heads: vec![Head::default(); heads.min(MAX_HEADS)],
            windows: vec![Window::default(); windows.min(MAX_WINDOWS)],
            irq: IrqHome::Local(FeIrq::new(heads)),
            inst: None,
            sor_owners: [0; 8],
            core_updates: 0,
            core_notifier_writes: 0,
            vblank_raised: 0,
            unresolved: 0,
            notes: Vec::new(),
            pins: Arc::default(),
            core_ticks: 0,
            routed_reads: 0,
            routed_writes: 0,
            recent: VecDeque::new(),
            record_write_failures: 0,
        };
        for h in 0..MAX_HEADS {
            ticker().set_head(h, 0, 0);
            ticker().set_want(h, false);
        }
        e
    }

    /// The SOR that drives head `h`: the one whose ARMED owner mask names the head, while
    /// the head is active (a SOR owner and a raster, `core_update`). `None` for an idle or
    /// unknown head.
    pub fn head_sor(&self, h: usize) -> Option<u32> {
        if !self.heads.get(h)?.active {
            return None;
        }
        (0..8u32).find(|&s| self.sor_owners[s as usize] & (1 << h) != 0)
    }

    /// One of the recent display events (`recent`), at display-clock time `ns`.
    fn recent(&mut self, ns: u64, s: String) {
        if self.recent.len() >= RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back(format!("{:.4} {s}", ns as f64 / 1e9));
    }

    /// Write a record on `io`; a failed write is counted and kept among the recent events.
    fn put(&mut self, io: &mut dyn RecordIo, m: MemRef, off: u64, data: &[u8], ns: u64, what: &str) {
        if !m.write(io, off, data) {
            self.record_write_failures += 1;
            self.recent(ns, format!("FAILED {what} write at {}+{off:#x} [{}]", m.describe(), io.path()));
        }
    }

    pub(crate) fn note(&mut self, s: String) {
        if self.notes.len() < 4096 {
            self.notes.push(s);
        }
    }

    // ---- interrupt state

    /// Hand the interrupt status to the display page (`CARAXES_DISPLAY_PAGE=1`): from now
    /// on it lives there, the guest reads it from memory and the reactor applies its
    /// stores (fe_page.rs).
    pub fn attach_page(&mut self, page: Arc<FePage>) {
        page.attach(self.irq());
        self.irq = IrqHome::Page(page);
        self.sync_ticker();
    }

    /// Whether the status lives in the display page.
    pub fn paged(&self) -> bool {
        matches!(self.irq, IrqHome::Page(_))
    }

    /// The interrupt status (a copy).
    pub fn irq(&self) -> FeIrq {
        match &self.irq {
            IrqHome::Local(i) => i.clone(),
            IrqHome::Page(p) => p.irq(),
        }
    }

    /// Change the status; with the page, the guest's stores are applied first and the
    /// page is written after (fe_page.rs `FePage::update`).
    fn with_irq<R>(&mut self, f: impl FnOnce(&mut FeIrq) -> R) -> R {
        match &mut self.irq {
            IrqHome::Local(i) => f(i),
            IrqHome::Page(p) => p.update(f),
        }
    }

    /// Whether the display interrupt line is asserted.
    pub fn pending(&self) -> bool {
        self.irq().pending()
    }

    fn signal(&mut self) {
        if self.pending() {
            ticker().raise();
        }
    }

    fn sync_ticker(&self) {
        let irq = self.irq();
        for (h, head) in self.heads.iter().enumerate() {
            // A pending presentation, or a latched one whose FINISHED is owed at the next
            // tick (`finish`): the clock must reach that tick without the guest's help.
            let flips = self
                .windows
                .iter()
                .any(|w| w.head == Some(h) && (!w.pending.is_empty() || w.begun.is_some()));
            let want = head.active && (flips || irq.en[h] & LAST_DATA != 0);
            ticker().set_want(h, want);
            if self.paged() {
                ticker().set_vblank(h, irq.en[h] & LAST_DATA != 0);
            }
        }
    }

    /// Whether the window channel has an `UPDATE` not yet latched (GET_CHANNEL_INFO).
    pub fn window_busy(&self, w: usize) -> bool {
        self.windows.get(w).is_some_and(|w| !w.pending.is_empty())
    }

    /// The current scan line of head `h` (SYSTEM_GET_SCANLINE, RG_DPCA).
    pub fn scanline(&self, h: usize, now: u64) -> u32 {
        let Some(head) = self.heads.get(h) else {
            return 0;
        };
        if !head.active || head.period_ns == 0 || head.vtotal == 0 {
            return 0;
        }
        let into = (now.saturating_sub(head.t0_ns)) % head.period_ns;
        (into * head.vtotal as u64 / head.period_ns) as u32
    }

    /// Whether `line` of head `h` lies in the vertical blank: before the raster's
    /// `BLANK_END` Y or from its `BLANK_START` Y on (clc57d.h `HEAD_SET_RASTER_BLANK_*`);
    /// without them, from the viewport's height on. A head without a raster scans out no
    /// active line, so it counts as in the blank (a wait for it ends).
    pub fn in_vblank(&self, h: usize, line: u32) -> bool {
        let Some(hd) = self.heads.get(h) else {
            return false;
        };
        if !hd.active {
            return true;
        }
        if hd.vblank_start > hd.vblank_end {
            line < hd.vblank_end || line >= hd.vblank_start
        } else {
            line >= hd.height
        }
    }

    /// `GET_RG_SCAN_LINE(b)` (core channel PIO, clc57d.h:285-289): LINE 15:0 the raster
    /// generator's current line, VBLANK 16:16 whether it lies in the vertical blank.
    pub fn rg_scan_line(&self, h: usize, now: u64) -> u32 {
        let line = self.scanline(h, now);
        (line & 0xFFFF) | ((self.in_vblank(h, line) as u32) << 16)
    }

    /// A write to `SET_GET_BLANKING_CTRL(b)` (core channel PIO, clc57d.h:291-297): BLANK
    /// 0:0 and UNBLANK 1:1, each NO_CHANGE (0) or ENABLE (1). The model carries a request
    /// out at once (there are no pixels to blank), BLANK before UNBLANK; a read gives the
    /// state reached ([`Self::blanking_ctrl`]). Returns the trace note.
    pub fn set_blanking_ctrl(&mut self, h: usize, v: u32) -> String {
        let Some(hd) = self.heads.get_mut(h) else {
            return format!("display: SET_GET_BLANKING_CTRL({h}) {v:#x}: no such head, dropped");
        };
        let was = hd.blanked;
        if v & 1 != 0 {
            hd.blanked = true;
        }
        if v & 2 != 0 {
            hd.blanked = false;
        }
        format!(
            "display: head {h} SET_GET_BLANKING_CTRL {v:#x}: {} [guest]",
            match (was, hd.blanked) {
                (false, true) => "blanked",
                (true, false) => "unblanked",
                (true, true) => "stays blanked",
                (false, false) => "stays unblanked",
            }
        )
    }

    /// A read of `SET_GET_BLANKING_CTRL(b)`: the head's blanking state in the register's
    /// own fields, BLANK (0x1) while blanked, UNBLANK (0x2) otherwise. The Windows KMD writes
    /// BLANK and waits until the read shows it: C47 (the write latched, read back 0x1) went
    /// on after one read, C48 (a read of 0: "nothing pending") polled 3599 times.
    pub fn blanking_ctrl(&self, h: usize) -> u32 {
        match self.heads.get(h) {
            Some(hd) if hd.blanked => 0x1,
            _ => 0x2,
        }
    }

    // ---- registers

    pub fn handles(off: u64) -> bool {
        let head_reg = |base: u64, stride: u64| {
            off >= base && off < base + stride * MAX_HEADS as u64 && (off - base) % stride == 0
        };
        head_reg(FE_EVT_STAT_HEAD_TIMING, 4)
            || head_reg(FE_RM_INTR_STAT_HEAD_TIMING, 4)
            || head_reg(FE_RM_INTR_EN_HEAD_TIMING, 4)
            || off == FE_RM_INTR_DISPATCH
            || off == FE_RM_INTR_STAT_CTRL_DISP
            || off == FE_EVT_STAT_AWAKEN_WIN
            || off == FE_EVT_STAT_AWAKEN_OTHER
            || head_reg(FE_CORE_HEAD_STATE, HEAD_STRIDE)
            || head_reg(RG_DPCA, HEAD_STRIDE)
            || head_reg(POSTCOMP_HEAD_LOADV_COUNTER, HEAD_STRIDE)
    }

    pub fn read32(&self, off: u64, now: u64) -> Option<u32> {
        let idx = |base: u64, stride: u64| ((off - base) / stride) as usize;
        let head = |h: usize| self.heads.get(h);
        let irq = self.irq();
        Some(match off {
            o if (FE_EVT_STAT_HEAD_TIMING..FE_EVT_STAT_HEAD_TIMING + 32).contains(&o) => {
                irq.evt[idx(FE_EVT_STAT_HEAD_TIMING, 4)]
            }
            o if (FE_RM_INTR_STAT_HEAD_TIMING..FE_RM_INTR_STAT_HEAD_TIMING + 32).contains(&o) => {
                irq.rm_stat(idx(FE_RM_INTR_STAT_HEAD_TIMING, 4))
            }
            o if (FE_RM_INTR_EN_HEAD_TIMING..FE_RM_INTR_EN_HEAD_TIMING + 32).contains(&o) => {
                irq.en[idx(FE_RM_INTR_EN_HEAD_TIMING, 4)]
            }
            FE_RM_INTR_DISPATCH => irq.dispatch(),
            FE_RM_INTR_STAT_CTRL_DISP => irq.ctrl_disp(),
            FE_EVT_STAT_AWAKEN_WIN => irq.awaken_win,
            FE_EVT_STAT_AWAKEN_OTHER => irq.awaken_other,
            o if on_head_page(o, FE_CORE_HEAD_STATE) => {
                let h = idx(FE_CORE_HEAD_STATE, HEAD_STRIDE);
                if head(h).is_some_and(|h| h.active) {
                    HEAD_STATE_AWAKE
                } else {
                    0
                }
            }
            o if on_head_page(o, RG_DPCA) => {
                let h = idx(RG_DPCA, HEAD_STRIDE);
                head(h).map_or(0, |hd| (hd.frames << 16) | (self.scanline(h, now) & 0xFFFF))
            }
            o if on_head_page(o, POSTCOMP_HEAD_LOADV_COUNTER) => {
                head(idx(POSTCOMP_HEAD_LOADV_COUNTER, HEAD_STRIDE)).map_or(0, |h| h.loadv)
            }
            _ => return None,
        })
    }

    /// A guest write; `None` when the register is not the engine's.
    pub fn write32(&mut self, off: u64, v: u32) -> Option<String> {
        let idx = |base: u64| ((off - base) / 4) as usize;
        let note = match off {
            o if (FE_EVT_STAT_HEAD_TIMING..FE_EVT_STAT_HEAD_TIMING + 32).contains(&o) => {
                let h = idx(FE_EVT_STAT_HEAD_TIMING);
                self.with_irq(|i| i.evt[h] &= !v);
                None
            }
            o if (FE_RM_INTR_EN_HEAD_TIMING..FE_RM_INTR_EN_HEAD_TIMING + 32).contains(&o) => {
                let h = idx(FE_RM_INTR_EN_HEAD_TIMING);
                let was = self.with_irq(|i| std::mem::replace(&mut i.en[h], v));
                self.sync_ticker();
                (was != v).then(|| {
                    // Only LAST_DATA is raised; other enables (RG_LINE_A 5:5, RG_LINE_B
                    // 6:6, RG_SEM 16+i, dev_disp.h v03_00/v04_01) are named so a guest
                    // that waits on one shows it.
                    format!(
                        "display: head {h} vblank (LAST_DATA) interrupt {} [guest]; RM_INTR_EN_HEAD_TIMING {v:#x}{}",
                        if v & LAST_DATA != 0 {
                            "enabled"
                        } else {
                            "disabled"
                        },
                        if v & !LAST_DATA != 0 {
                            format!(" (also {:#x}: not raised by this model)", v & !LAST_DATA)
                        } else {
                            String::new()
                        }
                    )
                })
            }
            FE_EVT_STAT_AWAKEN_WIN => {
                self.with_irq(|i| i.awaken_win &= !v);
                None
            }
            FE_EVT_STAT_AWAKEN_OTHER => {
                self.with_irq(|i| i.awaken_other &= !v);
                None
            }
            o if Self::handles(o) => {
                Some(format!("display: write {v:#x} to read-only {o:#x} dropped"))
            }
            _ => return None,
        };
        // Level-triggered: still pending after the handler's clears means another MSI.
        self.signal();
        Some(note.unwrap_or_default())
    }

    // ---- the clock

    /// Advance every active head to `now`: latch presentations, count frames, set
    /// `LAST_DATA`, and raise the vector when something became pending.
    pub fn catch_up(&mut self, now: u64, fb: &mut FakeFb, mem: &GuestMemory) {
        self.catch_up_io(now, &mut DeviceIo { fb, mem });
    }

    /// [`Engine::catch_up`] through `io`: the display clock's path without the device mutex
    /// passes its own (display-own-clock.md §2.5).
    pub fn catch_up_io(&mut self, now: u64, io: &mut dyn RecordIo) {
        let snap = |e: &Self| {
            let i = e.irq();
            (i.dispatch(), i.awaken_win, i.awaken_other)
        };
        let before = snap(self);
        for h in 0..self.heads.len() {
            let (active, p, t0) = {
                let hd = &self.heads[h];
                (hd.active, hd.period_ns, hd.t0_ns)
            };
            if !active || p == 0 || now < t0 {
                continue;
            }
            let cur = (now - t0) / p;
            if cur > self.heads[h].ticks + 10_000 {
                // A long stall (the guest was paused): do not replay hours of frames.
                let skip = cur - 1 - self.heads[h].ticks;
                let hd = &mut self.heads[h];
                hd.ticks = cur - 1;
                hd.frames = hd.frames.wrapping_add(skip as u32);
                hd.loadv = hd.loadv.wrapping_add(skip as u32);
            }
            while self.heads[h].ticks < cur {
                self.heads[h].ticks += 1;
                let tick = self.heads[h].ticks;
                let tick_ns = t0 + tick * p;
                {
                    let hd = &mut self.heads[h];
                    hd.frames = hd.frames.wrapping_add(1);
                    hd.loadv = hd.loadv.wrapping_add(1);
                }
                for w in 0..self.windows.len() {
                    if self.windows[w].head == Some(h) {
                        self.finish(w, io);
                        self.try_latch(w, tick, tick_ns, false, io);
                    }
                }
                let newly = self.with_irq(|i| {
                    let newly = i.en[h] & LAST_DATA != 0 && i.evt[h] & LAST_DATA == 0;
                    i.evt[h] |= LAST_DATA;
                    newly
                });
                if newly {
                    self.vblank_raised += 1;
                }
            }
            // Rate report every 600 ticks (10 s at 60 Hz).
            let hd = &mut self.heads[h];
            if hd.ticks >= hd.report.0 + 600 {
                let dt = now.saturating_sub(hd.report.1);
                let rate = (hd.ticks - hd.report.0) as f64 * 1e9 / dt.max(1) as f64;
                let line = format!(
                    "display: head {h} {} ticks, {:.3} Hz over the last {} ({:.3} s) [clock]",
                    hd.ticks,
                    rate,
                    hd.ticks - hd.report.0,
                    dt as f64 / 1e9
                );
                hd.report = (hd.ticks, now);
                self.note(line);
            }
        }
        self.sync_ticker();
        if snap(self) != before {
            self.signal();
        }
    }

    /// Latch the first pending presentation of window `w` if it is eligible at tick
    /// `tick` (time `tick_ns`); `force` latches regardless of the clock (head inactive,
    /// immediate flips).
    fn try_latch(
        &mut self,
        w: usize,
        tick: u64,
        tick_ns: u64,
        force: bool,
        io: &mut dyn RecordIo,
    ) -> bool {
        let win = &self.windows[w];
        let Some(f) = win.pending.front() else {
            return false;
        };
        if !force {
            if f.queued_ns >= tick_ns {
                return false;
            }
            if let Some(last) = win.last_latch_tick {
                if tick < last + f.min_interval.max(1) as u64 {
                    return false;
                }
            }
        }
        // TIMESTAMP_MODE: not before PTIMER reaches the flip's timestamp, on a head that
        // scans out (a window without an active head completes at once).
        let live = win.head.is_some_and(|h| self.heads.get(h).is_some_and(|hd| hd.active));
        if let Some(ts) = f.not_before_ns {
            if live && tick_ns < ts {
                self.windows[w].timestamp_waits += 1;
                return false;
            }
        }
        let (queued_ns, acquire) = (f.queued_ns, f.acquire);
        // Every record this latch touches must be reachable on this path, or none is
        // written: the latch waits for a path that reaches them all.
        let reach = |m: Option<MemRef>, len: u64| m.is_none_or(|m| io.can(m, 0, len));
        if !reach(acquire.map(|(m, _)| m), 4)
            || !reach(self.windows[w].prev_release.map(|(m, _)| m), 16)
            || !reach(f.notifier.map(|(m, _)| m), 16)
            || !reach(self.windows[w].begun.map(|(m, _)| m), 4)
        {
            return false;
        }
        if let Some((m, want)) = acquire {
            let have = m.read32(io);
            if have != Some(want) {
                let win = &mut self.windows[w];
                win.acquire_waits += 1;
                let since = *win.acquire_since.get_or_insert(tick);
                // Half a second, then every 2 s: the trace names a flip stuck on its acquire.
                let waited = tick.saturating_sub(since);
                if waited == 0 {
                    let s = format!(
                        "window {w}: flip waits for its acquire {} == {want:#x}, reads {} [{}]",
                        m.describe(),
                        have.map_or("nothing".into(), |v| format!("{v:#x}")),
                        io.path()
                    );
                    self.recent(tick_ns, s);
                }
                if !force && (waited == 30 || (waited > 0 && waited % 120 == 0)) {
                    let line = format!(
                        "display: window {w}: flip queued {} ms ago waits {waited} ticks for its acquire semaphore {} == {want:#x}, reads {} [guest]",
                        tick_ns.saturating_sub(queued_ns) / 1_000_000,
                        m.describe(),
                        have.map_or("nothing".into(), |v| format!("{v:#x}"))
                    );
                    self.note(line);
                }
                return false;
            }
        }
        self.windows[w].acquire_since = None;
        let f = self.windows[w].pending.pop_front().unwrap();
        let stamp = ptimer_stamp(tick_ns).to_le_bytes();
        // What the guest left in the records before the device writes them (the trace of
        // the first latches): how its driver resets a notifier and a semaphore.
        let before = |m: Option<MemRef>| {
            m.and_then(|m| m.record(io))
                .map_or("-".to_string(), |r| format!("[{:#x} {:#x} {:#x} {:#x}]", r[0], r[1], r[2], r[3]))
        };
        let seen = if self.windows[w].flips < 32 {
            format!(
                "; before: notifier {}, previous release {}",
                before(f.notifier.map(|(m, _)| m)),
                before(self.windows[w].prev_release.map(|(m, _)| m))
            )
        } else {
            String::new()
        };
        let win = &mut self.windows[w];
        // The previous presentation's release: its surface is displaced now ("To match
        // display hardware behavior, we release the semaphore when we flip *away*",
        // nvkms-displayless.c:262-270; nvkms-api.h:677-690). The four-word record gets
        // the timestamp too (nvkms-headsurface.c:598-604), the payload last.
        if let Some((m, v)) = win.prev_release.take() {
            self.put(io, m, 8, &stamp, tick_ns, "release timestamp");
            self.put(io, m, 0, &v.to_le_bytes(), tick_ns, "release");
        }
        self.windows[w].prev_release = f.release;
        if let Some((m, _)) = f.notifier {
            let w0 = NOTIFIER_BEGUN
                | if f.immediate {
                    NOTIFIER_FLIP_TYPE_IMMEDIATE
                } else {
                    0
                };
            // A flip still BEGUN (two latches before a tick: immediate flips) is finished
            // now; this one finishes at the next tick, or at once without scanout.
            self.finish(w, io);
            // Timestamp first, status last: a reader that sees BEGUN finds the timestamp
            // (NVKMS polls for it after the status, nvkms-sync.c:55-58).
            self.put(io, m, 8, &stamp, tick_ns, "notifier timestamp");
            self.put(io, m, 0, &w0.to_le_bytes(), tick_ns, "notifier BEGUN");
            self.windows[w].begun = Some((m, w0));
            if !live {
                self.finish(w, io);
            }
        }
        // AWAKEN follows the rule nvidia-drm models the hardware by
        // (`__will_generate_flip_event`, nvidia-drm-modeset.c:96-140): a flip event only
        // for a window that was scanning out a surface before this presentation. So a
        // window being enabled raises none (its notifier is still written), and a window
        // being disabled (no notifier, no surface) raises one.
        let was_active = self.windows[w].latched.mem.is_some();
        if was_active && f.notifier.is_none_or(|(_, awaken)| awaken) {
            self.with_irq(|i| i.awaken_win |= 1 << w);
        }
        let win = &mut self.windows[w];
        // The trace names the first flips, every change of what is scanned out other than
        // the buffer (a flip chain alternates buffers every frame), and every 600th flip.
        let geometry = |s: &Surface| {
            (s.mem.is_some(), s.width, s.height, s.format, s.pitch, s.pitch_layout, s.block_height_log2)
        };
        let changed = geometry(&win.latched) != geometry(&f.surface);
        win.latched = f.surface.clone();
        win.last_latch_tick = Some(tick);
        win.flips += 1;
        let n = win.flips;
        let short = format!(
            "window {w} latch {n} at tick {tick}{} [{}]: surface {}, notifier {}, acquire {}",
            if force { " (immediate)" } else { "" },
            io.path(),
            f.surface.mem.map_or("none".into(), |m| m.describe()),
            f.notifier.map_or("none".into(), |(m, _)| m.describe()),
            f.acquire.map_or("none".into(), |(m, v)| format!("{} == {v:#x}", m.describe())),
        );
        self.recent(tick_ns, short);
        if changed || n <= 32 || n % 600 == 0 {
            let s = &f.surface;
            let line = format!(
                "display: window {w} latch {n} at tick {tick}{}{}: surface {} {}x{} format {:#x} {} pitch {} (block height 2^{}), notifier {}, acquire {}, release {} [guest]",
                if force { " (immediate)" } else { "" },
                f.not_before_ns.map_or(String::new(), |ts| format!(" (timestamp {ts}, now {tick_ns})")),
                s.mem.map_or("none".into(), |m| m.describe()),
                s.width,
                s.height,
                s.format,
                if s.pitch_layout { "pitch" } else { "block-linear" },
                s.pitch,
                s.block_height_log2,
                f.notifier.map_or("none".into(), |(m, a)| format!("{}{}", m.describe(), if a { " awaken" } else { "" })),
                f.acquire.map_or("none".into(), |(m, v)| format!("{} == {v:#x}", m.describe())),
                f.release.map_or("none".into(), |(m, v)| format!("{} := {v:#x}", m.describe())),
            ) + &seen;
            self.note(line);
        }
        true
    }

    /// The flip window `w` latched last has had its first frame: its notifier FINISHED.
    fn finish(&mut self, w: usize, io: &mut dyn RecordIo) {
        let Some((m, w0)) = self.windows[w].begun else {
            return;
        };
        // Unreachable on this path: owed until a path that reaches it.
        if !io.can(m, 0, 4) {
            return;
        }
        self.windows[w].begun = None;
        let w0 = (w0 & !(3 << 30)) | NOTIFIER_FINISHED;
        let now = now_ns();
        self.put(io, m, 0, &w0.to_le_bytes(), now, "notifier FINISHED");
        let n = self.windows[w].flips;
        self.recent(now, format!("window {w} flip {n} FINISHED at {} [{}]", m.describe(), io.path()));
    }

    fn resolve_at(
        &mut self,
        fb: &FakeFb,
        handle: u32,
        chn: u32,
        off: u64,
        what: &str,
    ) -> Option<MemRef> {
        if handle == 0 {
            return None;
        }
        let r = self
            .inst
            .and_then(|inst| resolve(fb, inst, handle, chn))
            .and_then(|c| c.at(off));
        if r.is_none() {
            self.unresolved += 1;
            if self.unresolved <= 16 {
                self.note(format!(
                    "display: {what} context DMA {handle:#x} (channel {chn}, offset {off:#x}) not found in the instance memory at {:?} [guest]",
                    self.inst
                ));
            }
        }
        r
    }

    // ---- methods

    /// A core `UPDATE` with the ARMED state `armed`.
    pub fn core_update(
        &mut self,
        armed: &BTreeMap<u32, u32>,
        now: u64,
        fb: &mut FakeFb,
        mem: &GuestMemory,
    ) {
        self.core_updates += 1;
        let get = |m: u32| armed.get(&m).copied().unwrap_or(0);
        // Window owners.
        for w in 0..self.windows.len() {
            let owner = get(C_WINDOW_SET_CONTROL + 0x80 * w as u32) & 0xF;
            self.windows[w].head = ((owner as usize) < self.heads.len()).then_some(owner as usize);
        }
        // Heads.
        let mut sor_heads = 0u32;
        for s in 0..8 {
            self.sor_owners[s as usize] = get(C_SOR_SET_CONTROL + 0x20 * s) & 0xFF;
            sor_heads |= self.sor_owners[s as usize];
        }
        for h in 0..self.heads.len() {
            let base = C_HEAD + C_HEAD_STRIDE * h as u32;
            let raster = get(base + C_HEAD_RASTER_SIZE);
            let (htotal, vtotal) = (raster & 0x7FFF, (raster >> 16) & 0x7FFF);
            let pclk = get(base + C_HEAD_PIXEL_CLOCK);
            let mut pclk_hz = (pclk & 0x7FFF_FFFF) as u64;
            if pclk >> 31 == 1 {
                pclk_hz = pclk_hz * 1000 / 1001;
            }
            let vp = get(base + C_HEAD_VIEWPORT_SIZE_IN);
            let active = sor_heads & (1 << h) != 0 && htotal > 0 && vtotal > 0 && pclk_hz > 0;
            let period_ns = if active {
                (htotal as u64 * vtotal as u64 * 1_000_000_000).div_ceil(pclk_hz)
            } else {
                0
            };
            let hd = &mut self.heads[h];
            hd.olut = (
                get(base + C_HEAD_OLUT_CONTROL),
                get(base + C_HEAD_CONTEXT_DMA_OLUT),
            );
            if hd.active != active || hd.period_ns != period_ns {
                hd.active = active;
                hd.period_ns = period_ns;
                hd.pclk_hz = pclk_hz;
                hd.htotal = htotal;
                hd.vtotal = vtotal;
                hd.width = vp & 0x7FFF;
                hd.height = (vp >> 16) & 0x7FFF;
                hd.vblank_end = (get(base + C_HEAD_RASTER_BLANK_END) >> 16) & 0x7FFF;
                hd.vblank_start = (get(base + C_HEAD_RASTER_BLANK_START) >> 16) & 0x7FFF;
                hd.t0_ns = now;
                hd.ticks = 0;
                hd.report = (0, now);
                hd.modesets += 1;
                ticker().set_head(h, period_ns, now);
                let line = if active {
                    format!(
                        "display: head {h} active: {}x{} viewport, raster {htotal}x{vtotal}, pixel clock {pclk_hz} Hz, tick period {period_ns} ns ({:.3} Hz), OLUT control {:#x} ctxdma {:#x} [guest]",
                        hd.width, hd.height, 1e9 / period_ns as f64, hd.olut.0, hd.olut.1
                    )
                } else {
                    format!("display: head {h} inactive (SOR owners {sor_heads:#x}, raster {raster:#x}, pclk {pclk:#x}) [guest]")
                };
                self.note(line);
            }
        }
        // Presentations on windows without an active head complete at once.
        for w in 0..self.windows.len() {
            let live = self.windows[w].head.is_some_and(|h| self.heads[h].active);
            if !live {
                let io = &mut DeviceIo { fb: &mut *fb, mem };
                self.finish(w, io);
                while self.try_latch(w, 0, now, true, io) {}
            }
        }
        // The core notifier.
        let handle = get(C_SET_CONTEXT_DMA_NOTIFIER);
        let ctrl = get(C_SET_NOTIFIER_CONTROL);
        if handle != 0 {
            let off = (((ctrl >> 4) & 0xFF) * 16) as u64;
            if let Some(m) = self.resolve_at(fb, handle, CHN_CORE, off, "core notifier") {
                let io = &mut DeviceIo { fb: &mut *fb, mem };
                m.write(io, 8, &ptimer_stamp(now).to_le_bytes());
                m.write(io, 0, &NOTIFIER_FINISHED.to_le_bytes());
                self.core_notifier_writes += 1;
                if self.core_notifier_writes <= 3 {
                    let line = format!(
                        "display: core notifier FINISHED at {} (record {}), mode {} [guest]",
                        m.describe(),
                        off / 16,
                        if ctrl & 1 == 1 {
                            "WRITE_AWAKEN"
                        } else {
                            "WRITE"
                        }
                    );
                    self.note(line);
                }
                if ctrl & 1 == 1 {
                    self.with_irq(|i| i.awaken_other |= 1);
                }
            }
        }
        self.sync_ticker();
        self.signal();
    }

    /// A window method (the assembly state of the window).
    pub fn window_method(&mut self, w: usize, m: u32, v: u32) {
        if let Some(win) = self.windows.get_mut(w) {
            win.asm.insert(m, v);
        }
    }

    /// A window `UPDATE`: queue a presentation from the assembly state.
    pub fn window_update(&mut self, w: usize, now: u64, fb: &mut FakeFb, mem: &GuestMemory) {
        if w >= self.windows.len() {
            return;
        }
        let asm = self.windows[w].asm.clone();
        let get = |m: u32| asm.get(&m).copied().unwrap_or(0);
        let chn = chn_win(w as u32);
        let nctl = get(W_SET_NOTIFIER_CONTROL);
        let notifier = self
            .resolve_at(
                fb,
                get(W_SET_CONTEXT_DMA_NOTIFIER),
                chn,
                (((nctl >> 4) & 0xFF) * 16) as u64,
                "window notifier",
            )
            .map(|m| (m, nctl & 1 == 1));
        let sem_off = ((get(W_SET_SEMAPHORE_CONTROL) & 0xFF) * 16) as u64;
        let sem = self.resolve_at(
            fb,
            get(W_SET_CONTEXT_DMA_SEMAPHORE),
            chn,
            sem_off,
            "window semaphore",
        );
        // The flip's FB records reachable by the display clock without the device mutex.
        for m in [notifier.map(|(m, _)| m), sem].into_iter().flatten() {
            if m.target == 1 {
                if let Some(n) = self.pins.ensure(fb, m.addr, 16) {
                    self.note(n);
                }
            }
        }
        let iso = get(W_SET_CONTEXT_DMA_ISO);
        let size = get(W_SET_SIZE);
        let storage = get(W_SET_STORAGE);
        let surface = Surface {
            mem: self.resolve_at(
                fb,
                iso,
                chn,
                (get(W_SET_OFFSET) as u64) << 8,
                "window surface",
            ),
            width: size & 0xFFFF,
            height: size >> 16,
            format: get(W_SET_PARAMS) & 0xFF,
            pitch: get(W_SET_PLANAR_STORAGE) & 0x1FFF,
            pitch_layout: (storage >> 4) & 1 == 1,
            block_height_log2: storage & 0xF,
        };
        let present = get(W_SET_PRESENT_CONTROL);
        let not_before_ns = (present & PRESENT_TIMESTAMP_MODE_ENABLE != 0).then(|| {
            u64::from(get(W_SET_UPDATE_TIMESTAMP_LO))
                | u64::from(get(W_SET_UPDATE_TIMESTAMP_HI)) << 32
        });
        let flip = Flip {
            queued_ns: now,
            notifier,
            acquire: sem.map(|m| (m, get(W_SET_SEMAPHORE_ACQUIRE))),
            release: sem.map(|m| (m, get(W_SET_SEMAPHORE_RELEASE))),
            min_interval: present & 0xF,
            immediate: (present >> 4) & 7 == 1,
            not_before_ns,
            surface: if iso == 0 {
                Surface::default()
            } else {
                surface
            },
        };
        let immediate = flip.immediate;
        let win = &mut self.windows[w];
        win.updates += 1;
        win.pending.push_back(flip);
        let live = win.head.is_some_and(|h| self.heads[h].active);
        if immediate || !live {
            let tick = win.head.map_or(0, |h| self.heads[h].ticks);
            let io = &mut DeviceIo { fb: &mut *fb, mem };
            while self.try_latch(w, tick, now, true, io) {}
        }
        self.sync_ticker();
        self.signal();
    }

    /// The recent display events, one line each (the trace at a reset and at exit).
    pub fn recent_lines(&self) -> Vec<String> {
        let mut v = vec![format!(
            "display recent: the last {} latches, FINISHED writes and acquire waits ({} record write(s) failed in all):",
            self.recent.len(),
            self.record_write_failures
        )];
        v.extend(self.recent.iter().map(|s| format!("display recent: {s}")));
        v
    }

    /// Counters for the device summary and the trace.
    pub fn summary(&self) -> String {
        let heads: Vec<String> = self
            .heads
            .iter()
            .enumerate()
            .filter(|(_, h)| h.active || h.modesets > 0)
            .map(|(i, h)| {
                format!(
                    "head {i}: {} {}x{} {:.3} Hz, {} ticks, {} modesets{}",
                    if h.active { "active" } else { "off" },
                    h.width,
                    h.height,
                    h.hz(),
                    h.ticks,
                    h.modesets,
                    if h.blanked { ", blanked" } else { "" }
                )
            })
            .collect();
        let wins: Vec<String> = self
            .windows
            .iter()
            .enumerate()
            .filter(|(_, w)| w.updates > 0)
            .map(|(i, w)| {
                format!(
                    "window {i}: {} updates, {} latched, {} pending, {} acquire waits, {} timestamp waits",
                    w.updates,
                    w.flips,
                    w.pending.len(),
                    w.acquire_waits,
                    w.timestamp_waits
                )
            })
            .collect();
        let clock = ticker().timing_summary();
        let (pinned, refused) = self.pins.counts();
        format!(
            "display engine: {} core UPDATEs, {} core notifiers written, {} vblank interrupts, {} unresolved context DMAs, ticker raised {} ({} MSIs); {}; {}; without the device mutex: {} clock ticks, {} register reads, {} register writes, {} FB record window(s), {} region(s) left to the device path{}{}",
            self.core_updates,
            self.core_notifier_writes,
            self.vblank_raised,
            self.unresolved,
            ticker().raised.load(Ordering::Relaxed),
            ticker().msis.load(Ordering::Relaxed),
            heads.join("; "),
            wins.join("; "),
            self.core_ticks,
            self.routed_reads,
            self.routed_writes,
            pinned,
            refused,
            if clock.is_empty() { "" } else { "; " },
            clock
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    const INST: u64 = 0x10_0000;

    #[test]
    fn tick_timing_histogram() {
        let mut h = Hist::default();
        assert_eq!(h.quantile(0.5), 0);
        for us in [0, 3, 40, 45, 50, 60, 70, 80, 90, 2500] {
            h.add(us);
        }
        assert_eq!(h.n, 10);
        assert_eq!(h.max_us, 2500);
        assert_eq!(h.over_ms, 1);
        // The 5th of 10 values is 50 us, in bucket [32, 64).
        assert_eq!(h.quantile(0.5), 63);
        // The 99th percentile is the largest value's bucket, capped at the maximum.
        assert_eq!(h.quantile(0.99), 2500);
        assert!(h.describe().contains("1 of 10 at 1 ms or more"));
    }

    /// Bind a context DMA the way `_instmemAddHashEntry` and `instmemCommitContextDma`
    /// do (hash entry at the hash slot, object at `instance << 5`).
    #[allow(clippy::too_many_arguments)]
    fn bind(
        fb: &mut FakeFb,
        slot: u64,
        handle: u32,
        chn: u32,
        instance: u32,
        target: u32,
        base: u64,
        limit: u64,
    ) {
        let ctx = 0x12 | (instance << 14) | (chn << 25);
        fb.write(INST + slot * 8, &handle.to_le_bytes());
        fb.write(INST + slot * 8 + 4, &ctx.to_le_bytes());
        let o = INST + ((instance as u64) << 5);
        let words = [
            target | (1 << 2),
            (base >> 8) as u32,
            (base >> 40) as u32,
            (limit >> 8) as u32,
            (limit >> 40) as u32,
        ];
        for (i, v) in words.iter().enumerate() {
            fb.write(o + 4 * i as u64, &v.to_le_bytes());
        }
    }

    #[test]
    fn ramht_lookup_decodes_the_object() {
        let mut fb = FakeFb::new(16 << 20);
        bind(
            &mut fb,
            5,
            0xCAFE_0001,
            CHN_CORE,
            0x100,
            1,
            0x20_0000,
            0x20_0FFF,
        );
        bind(
            &mut fb,
            6,
            0xCAFE_0001,
            chn_win(0),
            0x101,
            1,
            0x30_0000,
            0x30_0FFF,
        );
        let c = resolve(&fb, INST, 0xCAFE_0001, CHN_CORE).unwrap();
        assert_eq!((c.target, c.base, c.limit), (1, 0x20_0000, 0x20_0FFF));
        let c = resolve(&fb, INST, 0xCAFE_0001, chn_win(0)).unwrap();
        assert_eq!(c.base, 0x30_0000);
        assert!(resolve(&fb, INST, 0xCAFE_0002, CHN_CORE).is_none());
        assert_eq!(c.at(0x1000), None);
        assert_eq!(c.at(0xFF0).unwrap().addr, 0x30_0FF0);
    }

    fn engine_with_head(fb: &mut FakeFb) -> Engine {
        let mut e = Engine::new(4, 8);
        e.inst = Some(INST);
        bind(fb, 1, 0x10, CHN_CORE, 0x100, 1, 0x40_0000, 0x40_0FFF);
        bind(fb, 2, 0x20, chn_win(0), 0x101, 1, 0x50_0000, 0x50_0FFF);
        bind(fb, 3, 0x30, chn_win(0), 0x102, 1, 0x60_0000, 0x60_0FFF);
        bind(fb, 4, 0x40, chn_win(0), 0x103, 1, 0x80_0000, 0xFF_FFFF);
        let mut armed = BTreeMap::new();
        armed.insert(C_SOR_SET_CONTROL, 0x1 | (8 << 8));
        armed.insert(C_WINDOW_SET_CONTROL, 0);
        armed.insert(C_WINDOW_SET_CONTROL + 0x80, 0);
        armed.insert(C_HEAD + C_HEAD_RASTER_SIZE, 2200 | (1125 << 16));
        armed.insert(C_HEAD + C_HEAD_PIXEL_CLOCK, 148_500_000);
        armed.insert(C_HEAD + C_HEAD_VIEWPORT_SIZE_IN, 1920 | (1080 << 16));
        armed.insert(C_SET_CONTEXT_DMA_NOTIFIER, 0x10);
        armed.insert(C_SET_NOTIFIER_CONTROL, 1 | (2 << 4) | (1 << 12));
        let mem = GuestMemory::new();
        e.core_update(&armed, 1_000, fb, &mem);
        e
    }

    #[test]
    fn core_update_programs_the_head_and_writes_the_notifier() {
        let mut fb = FakeFb::new(16 << 20);
        let e = engine_with_head(&mut fb);
        let h = &e.heads[0];
        assert!(h.active);
        assert_eq!(h.period_ns, 16_666_667);
        assert_eq!((h.width, h.height), (1920, 1080));
        assert_eq!(e.windows[0].head, Some(0));
        let mut b = [0u8; 4];
        fb.read(0x40_0000 + 32, &mut b);
        assert_eq!(u32::from_le_bytes(b) >> 30, 2, "record 2 FINISHED");
        assert_eq!(e.read32(FE_CORE_HEAD_STATE, 0), Some(HEAD_STATE_AWAKE));
        assert_eq!(e.read32(FE_CORE_HEAD_STATE + HEAD_STRIDE, 0), Some(0));
        assert_eq!(e.read32(FE_CORE_HEAD_STATE + 3 * HEAD_STRIDE, 0), Some(0));
        assert_eq!(e.read32(FE_CORE_HEAD_STATE + 7 * HEAD_STRIDE, 0), Some(0));
        assert_eq!(e.read32(FE_EVT_STAT_AWAKEN_OTHER, 0), Some(1));
        assert_eq!(
            e.read32(FE_RM_INTR_STAT_CTRL_DISP, 0),
            Some(CTRL_DISP_AWAKEN)
        );
    }

    /// Queue the flip of `a_flip_completes_at_the_next_tick_not_before`: notifier record 1
    /// of 0x50_0000, semaphore record 2 of 0x60_0000 (acquire 7, release 8).
    fn queue_flip(e: &mut Engine, fb: &mut FakeFb, mem: &GuestMemory, now: u64) {
        for (m, v) in [
            (W_SET_CONTEXT_DMA_NOTIFIER, 0x20),
            (W_SET_NOTIFIER_CONTROL, 1 | (1 << 4)),
            (W_SET_CONTEXT_DMA_SEMAPHORE, 0x30),
            (W_SET_SEMAPHORE_CONTROL, 2),
            (W_SET_SEMAPHORE_ACQUIRE, 7),
            (W_SET_SEMAPHORE_RELEASE, 8),
            (W_SET_CONTEXT_DMA_ISO, 0x40),
            (W_SET_OFFSET, 0x100),
            (W_SET_SIZE, 1920 | (1080 << 16)),
            (W_SET_STORAGE, 1 << 4),
            (W_SET_PARAMS, 0xE6),
            (W_SET_PLANAR_STORAGE, 120),
            (W_SET_PRESENT_CONTROL, 1),
        ] {
            e.window_method(0, m, v);
        }
        e.window_update(0, now, fb, mem);
    }

    /// The display clock's path without the device mutex (display/core_path.rs): with the
    /// flip's FB records in shared windows it latches (BEGUN), finishes at the next tick
    /// and raises vblank, all under the display lock only; the device path sees the same
    /// records.
    #[test]
    fn the_clock_latches_and_finishes_without_the_device_mutex() {
        let mut fb = crate::fb::testing::shared_ram_fb(16 << 20);
        let mut e = engine_with_head(&mut fb);
        let mem = GuestMemory::new();
        e.write32(FE_EVT_STAT_AWAKEN_OTHER, 1);
        e.write32(FE_RM_INTR_EN_HEAD_TIMING, LAST_DATA);
        let (p, t0) = (e.heads[0].period_ns, e.heads[0].t0_ns);
        queue_flip(&mut e, &mut fb, &mem, t0 + 1_000_000);
        assert_eq!(e.pins.counts(), (2, 0), "the notifier's and the semaphore's regions");
        assert!(e.notes.iter().any(|n| n.contains("mapped for the display clock")));
        fb.write(0x60_0000 + 32, &7u32.to_le_bytes());
        let engine = Arc::new(Mutex::new(e));
        // The device mutex is someone else's now: the clock runs on another thread with the
        // engine alone.
        let device = Mutex::new(());
        let _held = device.lock().unwrap();
        let (en, m2) = (engine.clone(), GuestMemory::new());
        let asserted = std::thread::spawn(move || super::super::core_path::tick_at(&en, &m2, t0 + p))
            .join()
            .unwrap();
        assert!(asserted, "LAST_DATA at the tick");
        assert!(!lock(&engine).window_busy(0), "latched without the device mutex");
        let mut b = [0u8; 4];
        fb.read(0x50_0000 + 16, &mut b);
        assert_eq!(u32::from_le_bytes(b), NOTIFIER_BEGUN);
        let (en, m2) = (engine.clone(), GuestMemory::new());
        std::thread::spawn(move || super::super::core_path::tick_at(&en, &m2, t0 + 2 * p))
            .join()
            .unwrap();
        fb.read(0x50_0000 + 16, &mut b);
        assert_eq!(u32::from_le_bytes(b), NOTIFIER_FINISHED);
        let e = lock(&engine);
        assert_eq!(e.core_ticks, 2);
        // The recent events name the path.
        assert!(e.recent.iter().any(|s| s.contains("latch") && s.ends_with("[clock]: surface fb 0x810000, notifier fb 0x500010, acquire fb 0x600020 == 0x7")), "{:?}", e.recent);
        assert!(e.recent.iter().any(|s| s.contains("FINISHED at fb 0x500010 [clock]")), "{:?}", e.recent);
        assert_eq!(e.record_write_failures, 0);
    }

    /// FB records the store cannot share wait for the device path: the clock without the
    /// device mutex writes none of them and the latch happens at the next device tick.
    #[test]
    fn records_the_clock_cannot_reach_wait_for_the_device_path() {
        let mut fb = FakeFb::new(16 << 20);
        let mut e = engine_with_head(&mut fb);
        let mem = GuestMemory::new();
        let (p, t0) = (e.heads[0].period_ns, e.heads[0].t0_ns);
        queue_flip(&mut e, &mut fb, &mem, t0 + 1_000_000);
        assert_eq!(e.pins.counts(), (0, 2));
        fb.write(0x60_0000 + 32, &7u32.to_le_bytes());
        let engine = Mutex::new(e);
        super::super::core_path::tick_at(&engine, &mem, t0 + p);
        let mut e = engine.into_inner().unwrap();
        assert!(e.window_busy(0), "no latch without its records");
        let mut b = [0u8; 4];
        fb.read(0x50_0000 + 16, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0, "nothing written");
        e.catch_up(t0 + 2 * p, &mut fb, &mem);
        assert!(!e.window_busy(0));
        fb.read(0x50_0000 + 16, &mut b);
        assert_eq!(u32::from_le_bytes(b), NOTIFIER_BEGUN);
    }

    #[test]
    fn a_flip_completes_at_the_next_tick_not_before() {
        let mut fb = FakeFb::new(16 << 20);
        let mut e = engine_with_head(&mut fb);
        let mem = GuestMemory::new();
        e.write32(FE_EVT_STAT_AWAKEN_OTHER, 1);
        let p = e.heads[0].period_ns;
        let t0 = e.heads[0].t0_ns;
        // Queue a flip 1 ms after t0 with notifier (awaken), semaphore and surface.
        for (m, v) in [
            (W_SET_CONTEXT_DMA_NOTIFIER, 0x20),
            (W_SET_NOTIFIER_CONTROL, 1 | (1 << 4)),
            (W_SET_CONTEXT_DMA_SEMAPHORE, 0x30),
            (W_SET_SEMAPHORE_CONTROL, 2),
            (W_SET_SEMAPHORE_ACQUIRE, 7),
            (W_SET_SEMAPHORE_RELEASE, 8),
            (W_SET_CONTEXT_DMA_ISO, 0x40),
            (W_SET_OFFSET, 0x100),
            (W_SET_SIZE, 1920 | (1080 << 16)),
            (W_SET_STORAGE, 1 << 4),
            (W_SET_PARAMS, 0xE6),
            (W_SET_PLANAR_STORAGE, 120),
            (W_SET_PRESENT_CONTROL, 1),
        ] {
            e.window_method(0, m, v);
        }
        e.window_update(0, t0 + 1_000_000, &mut fb, &mem);
        assert!(e.window_busy(0));
        // Before the tick: nothing.
        e.catch_up(t0 + p - 1, &mut fb, &mem);
        assert!(e.window_busy(0));
        assert_eq!(e.read32(FE_EVT_STAT_AWAKEN_WIN, 0), Some(0));
        // At the tick, the acquire (7) does not hold yet: still pending.
        e.catch_up(t0 + p, &mut fb, &mem);
        assert!(e.window_busy(0));
        assert_eq!(e.windows[0].acquire_waits, 1);
        // The value arrives; the next tick latches.
        fb.write(0x60_0000 + 32, &7u32.to_le_bytes());
        e.catch_up(t0 + 2 * p + 5, &mut fb, &mem);
        assert!(!e.window_busy(0));
        // The window notifier: BEGUN at the latch, non-tearing, PRESENT_COUNT 0, and the
        // PTIMER time of the tick; FINISHED at the next tick, the timestamp kept.
        let mut b = [0u8; 4];
        fb.read(0x50_0000 + 16, &mut b);
        assert_eq!(u32::from_le_bytes(b), NOTIFIER_BEGUN);
        let mut ts = [0u8; 8];
        fb.read(0x50_0000 + 24, &mut ts);
        assert_eq!(u64::from_le_bytes(ts), ptimer_stamp(t0 + 2 * p));
        assert_eq!(u64::from_le_bytes(ts) & 0x1F, 0);
        e.catch_up(t0 + 3 * p - 1, &mut fb, &mem);
        fb.read(0x50_0000 + 16, &mut b);
        assert_eq!(u32::from_le_bytes(b), NOTIFIER_BEGUN);
        e.catch_up(t0 + 3 * p, &mut fb, &mem);
        fb.read(0x50_0000 + 16, &mut b);
        assert_eq!(u32::from_le_bytes(b), NOTIFIER_FINISHED);
        fb.read(0x50_0000 + 24, &mut ts);
        assert_eq!(u64::from_le_bytes(ts), ptimer_stamp(t0 + 2 * p));
        // The window was off before: the notifier is written, but no AWAKEN (the
        // nvidia-drm model of the hardware).
        assert_eq!(e.read32(FE_EVT_STAT_AWAKEN_WIN, 0), Some(0));
        assert_eq!(e.windows[0].latched.mem.unwrap().addr, 0x80_0000 + 0x10000);
        // Release is written at flip-away, not at latch.
        fb.read(0x60_0000 + 32, &mut b);
        assert_eq!(u32::from_le_bytes(b), 7);
        e.window_update(0, t0 + 3 * p + 10, &mut fb, &mem);
        fb.write(0x60_0000 + 32, &7u32.to_le_bytes());
        e.catch_up(t0 + 4 * p, &mut fb, &mem);
        fb.read(0x60_0000 + 32, &mut b);
        assert_eq!(u32::from_le_bytes(b), 8);
        // The release record carries the flip-away time; the new flip's notifier is BEGUN.
        fb.read(0x60_0000 + 40, &mut ts);
        assert_eq!(u64::from_le_bytes(ts), ptimer_stamp(t0 + 4 * p));
        fb.read(0x50_0000 + 16, &mut b);
        assert_eq!(u32::from_le_bytes(b), NOTIFIER_BEGUN);
        // Flipping away from a scanned-out surface raises AWAKEN.
        assert_eq!(e.read32(FE_EVT_STAT_AWAKEN_WIN, 0), Some(1));
        assert_eq!(
            e.read32(FE_RM_INTR_STAT_CTRL_DISP, 0),
            Some(CTRL_DISP_AWAKEN)
        );
        // The guest clears AWAKEN; RG_DPCA counts frames.
        e.write32(FE_EVT_STAT_AWAKEN_WIN, 1);
        assert_eq!(e.read32(FE_RM_INTR_STAT_CTRL_DISP, 0), Some(0));
        assert_eq!(e.read32(RG_DPCA, 0).unwrap() >> 16, 4);
        assert_eq!(e.read32(POSTCOMP_HEAD_LOADV_COUNTER, 0), Some(4));
    }

    /// `TIMESTAMP_MODE` ENABLE holds a flip until PTIMER reaches `SET_UPDATE_TIMESTAMP`
    /// (nvkms-api.h:925), at the first tick at or past it; without the mode the timestamp
    /// methods are ignored.
    #[test]
    fn a_timestamped_flip_waits_for_its_time() {
        let mut fb = FakeFb::new(16 << 20);
        let mut e = engine_with_head(&mut fb);
        let mem = GuestMemory::new();
        let (t0, p) = (e.heads[0].t0_ns, e.heads[0].period_ns);
        let at = t0 + 3 * p - 10;
        for (m, v) in [
            (W_SET_CONTEXT_DMA_NOTIFIER, 0x20),
            (W_SET_NOTIFIER_CONTROL, 0),
            (W_SET_CONTEXT_DMA_ISO, 0x40),
            (W_SET_SIZE, 1920 | (1080 << 16)),
            (W_SET_UPDATE_TIMESTAMP_LO, at as u32),
            (W_SET_UPDATE_TIMESTAMP_HI, (at >> 32) as u32),
            (W_SET_PRESENT_CONTROL, 1 | PRESENT_TIMESTAMP_MODE_ENABLE),
        ] {
            e.window_method(0, m, v);
        }
        e.window_update(0, t0 + 1, &mut fb, &mem);
        e.catch_up(t0 + 2 * p, &mut fb, &mem);
        assert!(e.window_busy(0));
        assert_eq!(e.windows[0].timestamp_waits, 2);
        let mut b = [0u8; 4];
        fb.read(0x50_0000, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0, "not begun");
        e.catch_up(t0 + 3 * p, &mut fb, &mem);
        assert!(!e.window_busy(0));
        fb.read(0x50_0000, &mut b);
        assert_eq!(u32::from_le_bytes(b), NOTIFIER_BEGUN);
        // An immediate flip with a timestamp in the future waits for the tick too.
        e.window_method(0, W_SET_UPDATE_TIMESTAMP_LO, (t0 + 4 * p) as u32);
        e.window_method(0, W_SET_PRESENT_CONTROL, (1 << 4) | PRESENT_TIMESTAMP_MODE_ENABLE);
        e.window_update(0, t0 + 3 * p + 1, &mut fb, &mem);
        assert!(e.window_busy(0));
        e.catch_up(t0 + 4 * p, &mut fb, &mem);
        assert!(!e.window_busy(0));
        // Mode disabled: the timestamp methods do not hold the flip.
        e.window_method(0, W_SET_UPDATE_TIMESTAMP_LO, u32::MAX);
        e.window_method(0, W_SET_PRESENT_CONTROL, 1);
        e.window_update(0, t0 + 4 * p + 1, &mut fb, &mem);
        e.catch_up(t0 + 5 * p, &mut fb, &mem);
        assert!(!e.window_busy(0));
    }

    /// The display clock is the device clock: PTIMER (the channel registry's clock) and
    /// the display engine's notifier timestamps are on one base.
    #[test]
    fn the_display_clock_is_the_ptimer_base() {
        let reg = crate::channel::Registry::new(crate::fixture::rtx2070().chip.data().channel);
        let a = reg.now_ns();
        let d = now_ns();
        let b = reg.now_ns();
        assert!(a <= d && d <= b, "{a} {d} {b}");
        assert_eq!(reg.epoch(), Some(crate::channel::device_epoch()));
        assert_eq!(ptimer_stamp(0x1_0000_003F), 0x1_0000_0020);
    }

    #[test]
    fn vblank_is_raised_only_when_enabled_and_cleared_by_the_guest() {
        let mut fb = FakeFb::new(16 << 20);
        let mut e = engine_with_head(&mut fb);
        let mem = GuestMemory::new();
        let (t0, p) = (e.heads[0].t0_ns, e.heads[0].period_ns);
        e.catch_up(t0 + p, &mut fb, &mem);
        assert_eq!(e.read32(FE_EVT_STAT_HEAD_TIMING, 0), Some(LAST_DATA));
        assert_eq!(e.read32(FE_RM_INTR_DISPATCH, 0), Some(0));
        e.write32(FE_RM_INTR_EN_HEAD_TIMING, LAST_DATA);
        assert_eq!(e.read32(FE_RM_INTR_STAT_HEAD_TIMING, 0), Some(LAST_DATA));
        assert_eq!(e.read32(FE_RM_INTR_DISPATCH, 0), Some(1));
        e.write32(FE_EVT_STAT_HEAD_TIMING, LAST_DATA);
        assert_eq!(e.read32(FE_RM_INTR_DISPATCH, 0), Some(0));
        e.catch_up(t0 + 2 * p, &mut fb, &mem);
        assert_eq!(e.read32(FE_RM_INTR_DISPATCH, 0), Some(1));
        assert_eq!(e.vblank_raised, 1, "the first tick came before the enable");
    }

    #[test]
    fn a_flip_on_an_inactive_head_completes_at_once() {
        let mut fb = FakeFb::new(16 << 20);
        let mut e = Engine::new(4, 8);
        e.inst = Some(INST);
        bind(&mut fb, 2, 0x20, chn_win(3), 0x101, 1, 0x50_0000, 0x50_0FFF);
        let mem = GuestMemory::new();
        e.window_method(3, W_SET_CONTEXT_DMA_NOTIFIER, 0x20);
        e.window_method(3, W_SET_NOTIFIER_CONTROL, 1);
        e.window_update(3, 5, &mut fb, &mem);
        assert!(!e.window_busy(3));
        let mut b = [0u8; 4];
        fb.read(0x50_0000, &mut b);
        assert_eq!(u32::from_le_bytes(b), NOTIFIER_FINISHED, "no scanout: finished at once");
        assert_eq!(
            e.read32(FE_EVT_STAT_AWAKEN_WIN, 0),
            Some(0),
            "no surface before"
        );
    }

    #[test]
    fn the_period_follows_the_mode() {
        let mut fb = FakeFb::new(16 << 20);
        let mut e = engine_with_head(&mut fb);
        let mem = GuestMemory::new();
        let mut armed = BTreeMap::new();
        armed.insert(C_SOR_SET_CONTROL, 1);
        armed.insert(C_HEAD + C_HEAD_RASTER_SIZE, 2640 | (1125 << 16));
        armed.insert(C_HEAD + C_HEAD_PIXEL_CLOCK, 148_500_000);
        e.core_update(&armed, 50, &mut fb, &mem);
        assert_eq!(e.heads[0].period_ns, 20_000_000);
        armed.insert(C_HEAD + C_HEAD_RASTER_SIZE, 1650 | (750 << 16));
        armed.insert(C_HEAD + C_HEAD_PIXEL_CLOCK, 74_250_000);
        e.core_update(&armed, 60, &mut fb, &mem);
        assert_eq!(e.heads[0].period_ns, 16_666_667);
        assert_eq!(e.heads[0].modesets, 3);
        armed.insert(C_SOR_SET_CONTROL, 0);
        e.core_update(&armed, 70, &mut fb, &mem);
        assert!(!e.heads[0].active);
        assert_eq!(e.read32(FE_CORE_HEAD_STATE, 0), Some(0));
    }

    /// The display page (fe_page.rs): the clock's catch-up writes the vblank and the
    /// AWAKEN marker into the page itself (the guest's reads no longer trap and no
    /// longer advance it), and the guest's stores in the page reach the engine's status.
    #[test]
    fn page_mode_the_clock_writes_the_page() {
        use crate::display::fe_page::{FePage, AWAKEN_CORE, TAG};
        use std::sync::atomic::AtomicU64;
        let intr = crate::intr_page::IntrPage::create(
            16 << 20,
            0x00B8_1000,
            crate::fixture::rtx2070().chip.data().identity,
            IntrShared::new(),
            crate::intr_page::Config::default(),
        )
        .unwrap();
        let raised = Arc::new(AtomicU64::new(0));
        let r = raised.clone();
        let fe = FePage::create_with(
            &intr,
            Box::new(move || {
                r.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();
        let mut fb = FakeFb::new(16 << 20);
        let mut e = engine_with_head(&mut fb);
        let mem = GuestMemory::new();
        // The core notifier's WRITE_AWAKEN of the modeset moves into the page as a marker.
        e.attach_page(fe.clone());
        assert!(e.paged());
        assert_eq!(fe.read32(FE_EVT_STAT_AWAKEN_OTHER), AWAKEN_CORE | TAG);
        assert_eq!(fe.read32(FE_RM_INTR_STAT_CTRL_DISP), CTRL_DISP_AWAKEN);
        fe.write(FE_EVT_STAT_AWAKEN_OTHER, &AWAKEN_CORE.to_le_bytes());
        assert_eq!(e.irq().awaken_other, 0);
        assert!(!fe.scan(), "the trapped acknowledgement was applied synchronously");
        // Trapped enable writes are visible to the engine before the reactor scans.
        fe.write(FE_RM_INTR_EN_HEAD_TIMING, &LAST_DATA.to_le_bytes());
        assert_eq!(e.read32(FE_RM_INTR_EN_HEAD_TIMING, 0), Some(LAST_DATA));
        let (t0, p) = (e.heads[0].t0_ns, e.heads[0].period_ns);
        // The clock's tick: LAST_DATA and DISPATCH in the page, no guest read needed.
        e.catch_up(t0 + p, &mut fb, &mem);
        assert_eq!(fe.read32(FE_EVT_STAT_HEAD_TIMING), LAST_DATA | TAG);
        assert_eq!(fe.read32(FE_RM_INTR_STAT_HEAD_TIMING), LAST_DATA);
        assert_eq!(fe.read32(FE_RM_INTR_DISPATCH), 1);
        assert_eq!(e.vblank_raised, 1);
        // A flip away from a scanned-out surface: the window's AWAKEN under a marker.
        for (m, v) in [
            (W_SET_CONTEXT_DMA_NOTIFIER, 0x20),
            (W_SET_NOTIFIER_CONTROL, 1 | (1 << 4)),
            (W_SET_CONTEXT_DMA_ISO, 0x40),
            (W_SET_SIZE, 1920 | (1080 << 16)),
            (W_SET_PRESENT_CONTROL, 1),
        ] {
            e.window_method(0, m, v);
        }
        e.window_update(0, t0 + p + 1, &mut fb, &mem);
        e.catch_up(t0 + 2 * p, &mut fb, &mem);
        e.window_update(0, t0 + 2 * p + 1, &mut fb, &mem);
        e.catch_up(t0 + 3 * p, &mut fb, &mem);
        assert_eq!(fe.read32(FE_EVT_STAT_AWAKEN_WIN), 1);
        assert_eq!(fe.read32(FE_EVT_STAT_AWAKEN_OTHER), AWAKEN_CORE | TAG);
        // The guest's pass: vblank ack, the window echo, CORE_RESET.
        fe.write(FE_EVT_STAT_HEAD_TIMING, &LAST_DATA.to_le_bytes());
        fe.write(FE_EVT_STAT_AWAKEN_WIN, &1u32.to_le_bytes());
        fe.write(FE_EVT_STAT_AWAKEN_OTHER, &AWAKEN_CORE.to_le_bytes());
        assert!(!e.pending());
        assert!(!fe.scan());
        assert_eq!(fe.read32(FE_RM_INTR_DISPATCH), 0);
        assert_eq!(fe.read32(FE_RM_INTR_STAT_CTRL_DISP), 0);
        assert_eq!(raised.load(Ordering::Relaxed), 1, "the vblank ack leaves a window awaken pending");
    }

    /// With a driver the clock runs it at every tick before the raise; a head with only
    /// vblank enabled through the page ticks too.
    #[test]
    fn the_clock_drives_the_engine_with_the_page() {
        use std::sync::atomic::AtomicU64;
        let t = Arc::new(Ticker::default());
        let n = Arc::new(AtomicU64::new(0));
        let c = n.clone();
        t.set_driver(Arc::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
            true
        }));
        t.set_head(0, 1_000_000, now_ns());
        t.set_vblank(0, true);
        let me = t.clone();
        std::thread::spawn(move || me.run());
        let deadline = Instant::now() + Duration::from_secs(5);
        while n.load(Ordering::Relaxed) < 5 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(n.load(Ordering::Relaxed) >= 5);
        assert_eq!(t.driven.load(Ordering::Relaxed), n.load(Ordering::Relaxed));
    }

    /// With a driver, a tick raises the vector only while the display's interrupt line is
    /// asserted after it (C64: a leaf raised with nothing pending in the FE was never
    /// acknowledged, and every MSI re-arm sent it again).
    #[test]
    fn the_clock_raises_only_an_asserted_line() {
        use std::sync::atomic::AtomicU64;
        let intr = IntrShared::new();
        let t = Arc::new(Ticker::default());
        let asserted = Arc::new(AtomicBool::new(false));
        let n = Arc::new(AtomicU64::new(0));
        let (a, c) = (asserted.clone(), n.clone());
        t.set_driver(Arc::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
            a.load(Ordering::Relaxed)
        }));
        t.set_head(0, 1_000_000, now_ns());
        t.set_want(0, true);
        t.attach(intr.clone(), 76);
        let wait_for = |k: u64| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while n.load(Ordering::Relaxed) < k && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(n.load(Ordering::Relaxed) >= k, "the clock did not tick");
        };
        wait_for(5);
        assert_eq!(t.raised.load(Ordering::Relaxed), 0, "ticks with the line low raise nothing");
        asserted.store(true, Ordering::Relaxed);
        let k = n.load(Ordering::Relaxed);
        wait_for(k + 3);
        assert!(t.raised.load(Ordering::Relaxed) > 0, "an asserted line is raised");
        t.set_want(0, false);
    }
}

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
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
//!   satisfied, `MIN_PRESENT_INTERVAL` ticks since the last latch, then the previous
//!   presentation's release semaphore, the notifier FINISHED with a timestamp, and
//!   `AWAKEN_WIN`. `BEGIN_MODE_IMMEDIATE` latches at the `UPDATE`.
//! - Every tick advances `RG_DPCA` and the LOADV counter and sets `LAST_DATA`; with the
//!   RM's enable that is the vblank interrupt (dev_disp.h v03/v04, kern_disp_0300.c).
//! - The clock is `CLOCK_MONOTONIC` with absolute deadlines `t0 + n * period`. A
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
use std::time::{Duration, Instant};

use crate::fb::FakeFb;
use crate::guestmem::GuestMemory;
use crate::intr::IntrShared;

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

// NV_DISP_NOTIFIER (clc57d.h): 16 bytes, STATUS 31:30 of word 0, timestamp in words 2-3.
const NOTIFIER_FINISHED: u32 = 2 << 30;
const NOTIFIER_FLIP_TYPE_IMMEDIATE: u32 = 1 << 9;

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

// RAMHT and context DMA objects (disp/v03_00/dev_disp.h NV_UDISP_*, NV_DMA_*).
const HASH_LIMIT: u64 = 0x2000;
const HASH_ENTRY: u64 = 8;

// -------------------------------------------------------------------------------------
// The clock and the ticker thread.

static EPOCH: OnceLock<Instant> = OnceLock::new();

/// Nanoseconds of the display clock (CLOCK_MONOTONIC through `Instant`).
pub fn now_ns() -> u64 {
    EPOCH.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

#[derive(Default)]
struct TickHead {
    period_ns: AtomicU64,
    t0_ns: AtomicU64,
    /// The head has work at its ticks: a pending presentation or vblank enabled.
    want: AtomicBool,
}

/// State shared with the ticker thread; the server thread writes it.
#[derive(Default)]
pub struct Ticker {
    heads: [TickHead; MAX_HEADS],
    vector: AtomicU32,
    intr: Mutex<Option<Arc<IntrShared>>>,
    thread: Mutex<Option<std::thread::Thread>>,
    /// Vectors the ticker raised, and MSIs written for them.
    pub raised: AtomicU64,
    pub msis: AtomicU64,
}

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
        if let Some(i) = self.intr() {
            self.raised.fetch_add(1, Ordering::Relaxed);
            if i.raise(self.vector.load(Ordering::Relaxed)) {
                self.msis.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn run(&self) {
        let mut last = [u64::MAX; MAX_HEADS];
        loop {
            let now = now_ns();
            let mut next: Option<u64> = None;
            let mut fire = false;
            for (h, t) in self.heads.iter().enumerate() {
                let p = t.period_ns.load(Ordering::SeqCst);
                if p == 0 || !t.want.load(Ordering::SeqCst) {
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
                    }
                    last[h] = n;
                }
                let d = t0 + (n + 1) * p;
                next = Some(next.map_or(d, |x: u64| x.min(d)));
            }
            if fire {
                self.raise();
            }
            let now = now_ns();
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

impl MemRef {
    fn read32(&self, fb: &FakeFb, mem: &GuestMemory) -> Option<u32> {
        let mut b = [0u8; 4];
        let ok = if self.target == 1 {
            fb.read(self.addr, &mut b)
        } else {
            mem.read(self.addr, &mut b)
        };
        ok.then(|| u32::from_le_bytes(b))
    }

    fn write(&self, fb: &mut FakeFb, mem: &GuestMemory, off: u64, data: &[u8]) -> bool {
        if self.target == 1 {
            fb.write(self.addr + off, data)
        } else {
            mem.write(self.addr + off, data)
        }
    }

    fn describe(&self) -> String {
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
    pub surface: Surface,
}

#[derive(Clone, Debug, Default)]
pub struct Window {
    pub head: Option<usize>,
    pub asm: BTreeMap<u32, u32>,
    pub pending: VecDeque<Flip>,
    pub last_latch_tick: Option<u64>,
    pub prev_release: Option<(MemRef, u32)>,
    pub latched: Surface,
    pub updates: u64,
    pub flips: u64,
    pub acquire_waits: u64,
}

/// The display engine's method and timing state.
pub struct Engine {
    pub heads: Vec<Head>,
    pub windows: Vec<Window>,
    evt_timing: [u32; MAX_HEADS],
    en_timing: [u32; MAX_HEADS],
    awaken_win: u32,
    awaken_other: u32,
    /// Display instance memory base in FB (from `WRITE_INST_MEM`).
    pub inst: Option<u64>,
    pub core_updates: u64,
    pub core_notifier_writes: u64,
    pub vblank_raised: u64,
    pub unresolved: u64,
    pub notes: Vec<String>,
}

impl Engine {
    pub fn new(heads: usize, windows: usize) -> Self {
        let e = Engine {
            heads: vec![Head::default(); heads.min(MAX_HEADS)],
            windows: vec![Window::default(); windows.min(MAX_WINDOWS)],
            evt_timing: [0; MAX_HEADS],
            en_timing: [0; MAX_HEADS],
            awaken_win: 0,
            awaken_other: 0,
            inst: None,
            core_updates: 0,
            core_notifier_writes: 0,
            vblank_raised: 0,
            unresolved: 0,
            notes: Vec::new(),
        };
        for h in 0..MAX_HEADS {
            ticker().set_head(h, 0, 0);
            ticker().set_want(h, false);
        }
        e
    }

    fn note(&mut self, s: String) {
        if self.notes.len() < 4096 {
            self.notes.push(s);
        }
    }

    // ---- interrupt state

    fn rm_stat_timing(&self, h: usize) -> u32 {
        self.evt_timing[h] & self.en_timing[h]
    }

    fn dispatch(&self) -> u32 {
        (0..self.heads.len())
            .filter(|&h| self.rm_stat_timing(h) != 0)
            .fold(0, |m, h| m | 1 << h)
    }

    fn ctrl_disp(&self) -> u32 {
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

    fn signal(&mut self) {
        if self.pending() {
            ticker().raise();
        }
    }

    fn sync_ticker(&self) {
        for (h, head) in self.heads.iter().enumerate() {
            let flips = self
                .windows
                .iter()
                .any(|w| w.head == Some(h) && !w.pending.is_empty());
            let want = head.active && (flips || self.en_timing[h] & LAST_DATA != 0);
            ticker().set_want(h, want);
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
        Some(match off {
            o if (FE_EVT_STAT_HEAD_TIMING..FE_EVT_STAT_HEAD_TIMING + 32).contains(&o) => {
                self.evt_timing[idx(FE_EVT_STAT_HEAD_TIMING, 4)]
            }
            o if (FE_RM_INTR_STAT_HEAD_TIMING..FE_RM_INTR_STAT_HEAD_TIMING + 32).contains(&o) => {
                self.rm_stat_timing(idx(FE_RM_INTR_STAT_HEAD_TIMING, 4))
            }
            o if (FE_RM_INTR_EN_HEAD_TIMING..FE_RM_INTR_EN_HEAD_TIMING + 32).contains(&o) => {
                self.en_timing[idx(FE_RM_INTR_EN_HEAD_TIMING, 4)]
            }
            FE_RM_INTR_DISPATCH => self.dispatch(),
            FE_RM_INTR_STAT_CTRL_DISP => self.ctrl_disp(),
            FE_EVT_STAT_AWAKEN_WIN => self.awaken_win,
            FE_EVT_STAT_AWAKEN_OTHER => self.awaken_other,
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
                self.evt_timing[idx(FE_EVT_STAT_HEAD_TIMING)] &= !v;
                None
            }
            o if (FE_RM_INTR_EN_HEAD_TIMING..FE_RM_INTR_EN_HEAD_TIMING + 32).contains(&o) => {
                let h = idx(FE_RM_INTR_EN_HEAD_TIMING);
                let was = self.en_timing[h];
                self.en_timing[h] = v;
                self.sync_ticker();
                (was != v).then(|| {
                    format!(
                        "display: head {h} vblank (LAST_DATA) interrupt {} [guest]",
                        if v & LAST_DATA != 0 {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    )
                })
            }
            FE_EVT_STAT_AWAKEN_WIN => {
                self.awaken_win &= !v;
                None
            }
            FE_EVT_STAT_AWAKEN_OTHER => {
                self.awaken_other &= !v;
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
        let before = (self.dispatch(), self.awaken_win, self.awaken_other);
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
                        self.try_latch(w, tick, tick_ns, false, fb, mem);
                    }
                }
                if self.en_timing[h] & LAST_DATA != 0 && self.evt_timing[h] & LAST_DATA == 0 {
                    self.vblank_raised += 1;
                }
                self.evt_timing[h] |= LAST_DATA;
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
        if (self.dispatch(), self.awaken_win, self.awaken_other) != before {
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
        fb: &mut FakeFb,
        mem: &GuestMemory,
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
        if let Some((m, want)) = f.acquire {
            let have = m.read32(fb, mem);
            if have != Some(want) {
                self.windows[w].acquire_waits += 1;
                return false;
            }
        }
        let f = self.windows[w].pending.pop_front().unwrap();
        let win = &mut self.windows[w];
        if let Some((m, v)) = win.prev_release.take() {
            m.write(fb, mem, 0, &v.to_le_bytes());
        }
        win.prev_release = f.release;
        if let Some((m, _)) = f.notifier {
            let mut rec = [0u8; 16];
            let w0 = NOTIFIER_FINISHED
                | if f.immediate {
                    NOTIFIER_FLIP_TYPE_IMMEDIATE
                } else {
                    0
                };
            rec[0..4].copy_from_slice(&w0.to_le_bytes());
            rec[8..16].copy_from_slice(&tick_ns.to_le_bytes());
            // Timestamp first, status last, as the hardware orders its writes.
            m.write(fb, mem, 8, &rec[8..16]);
            m.write(fb, mem, 0, &rec[0..4]);
        }
        if f.notifier.is_some_and(|(_, awaken)| awaken) {
            self.awaken_win |= 1 << w;
        }
        let changed = win.latched != f.surface;
        win.latched = f.surface.clone();
        win.last_latch_tick = Some(tick);
        win.flips += 1;
        let n = win.flips;
        if changed || n <= 3 {
            let s = &f.surface;
            let line = format!(
                "display: window {w} latch {n} at tick {tick}{}: surface {} {}x{} format {:#x} {} pitch {} (block height 2^{}), notifier {}, acquire {}, release {} [guest]",
                if force { " (immediate)" } else { "" },
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
            );
            self.note(line);
        }
        true
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
            sor_heads |= get(C_SOR_SET_CONTROL + 0x20 * s) & 0xFF;
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
                while self.try_latch(w, 0, now, true, fb, mem) {}
            }
        }
        // The core notifier.
        let handle = get(C_SET_CONTEXT_DMA_NOTIFIER);
        let ctrl = get(C_SET_NOTIFIER_CONTROL);
        if handle != 0 {
            let off = (((ctrl >> 4) & 0xFF) * 16) as u64;
            if let Some(m) = self.resolve_at(fb, handle, CHN_CORE, off, "core notifier") {
                m.write(fb, mem, 8, &now.to_le_bytes());
                m.write(fb, mem, 0, &NOTIFIER_FINISHED.to_le_bytes());
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
                    self.awaken_other |= 1;
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
        let flip = Flip {
            queued_ns: now,
            notifier,
            acquire: sem.map(|m| (m, get(W_SET_SEMAPHORE_ACQUIRE))),
            release: sem.map(|m| (m, get(W_SET_SEMAPHORE_RELEASE))),
            min_interval: present & 0xF,
            immediate: (present >> 4) & 7 == 1,
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
            while self.try_latch(w, tick, now, true, fb, mem) {}
        }
        self.sync_ticker();
        self.signal();
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
                    "head {i}: {} {}x{} {:.3} Hz, {} ticks, {} modesets",
                    if h.active { "active" } else { "off" },
                    h.width,
                    h.height,
                    h.hz(),
                    h.ticks,
                    h.modesets
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
                    "window {i}: {} updates, {} latched, {} pending, {} acquire waits",
                    w.updates,
                    w.flips,
                    w.pending.len(),
                    w.acquire_waits
                )
            })
            .collect();
        format!(
            "display engine: {} core UPDATEs, {} core notifiers written, {} vblank interrupts, {} unresolved context DMAs, ticker raised {} ({} MSIs); {}; {}",
            self.core_updates,
            self.core_notifier_writes,
            self.vblank_raised,
            self.unresolved,
            ticker().raised.load(Ordering::Relaxed),
            ticker().msis.load(Ordering::Relaxed),
            heads.join("; "),
            wins.join("; ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INST: u64 = 0x10_0000;

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
        let mut b = [0u8; 4];
        fb.read(0x50_0000 + 16, &mut b);
        assert_eq!(u32::from_le_bytes(b) >> 30, 2);
        let mut ts = [0u8; 8];
        fb.read(0x50_0000 + 24, &mut ts);
        assert_eq!(u64::from_le_bytes(ts), t0 + 2 * p);
        assert_eq!(e.read32(FE_EVT_STAT_AWAKEN_WIN, 0), Some(1));
        assert_eq!(
            e.read32(FE_RM_INTR_STAT_CTRL_DISP, 0),
            Some(CTRL_DISP_AWAKEN)
        );
        assert_eq!(e.windows[0].latched.mem.unwrap().addr, 0x80_0000 + 0x10000);
        // Release is written at flip-away, not at latch.
        fb.read(0x60_0000 + 32, &mut b);
        assert_eq!(u32::from_le_bytes(b), 7);
        e.window_update(0, t0 + 2 * p + 10, &mut fb, &mem);
        fb.write(0x60_0000 + 32, &7u32.to_le_bytes());
        e.catch_up(t0 + 3 * p, &mut fb, &mem);
        fb.read(0x60_0000 + 32, &mut b);
        assert_eq!(u32::from_le_bytes(b), 8);
        // The guest clears AWAKEN; RG_DPCA counts frames.
        e.write32(FE_EVT_STAT_AWAKEN_WIN, 1);
        assert_eq!(e.read32(FE_RM_INTR_STAT_CTRL_DISP, 0), Some(0));
        assert_eq!(e.read32(RG_DPCA, 0).unwrap() >> 16, 3);
        assert_eq!(e.read32(POSTCOMP_HEAD_LOADV_COUNTER, 0), Some(3));
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
        assert_eq!(e.read32(FE_EVT_STAT_AWAKEN_WIN, 0), Some(1 << 3));
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
}

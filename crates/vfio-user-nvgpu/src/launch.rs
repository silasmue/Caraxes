// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The launch path's latency chain (`CARAXES_LAUNCH_TIMING=1`, docs/caraxes/launch-path.md).
//!
//! A kernel launch of a guest CUDA process crosses the device twice: the submission (the
//! guest's doorbell write reaches the doorbell reactor through a KVM ioeventfd, poll.rs; the
//! reactor rings the host doorbell of the mirror channel, ctrl.rs and mirror.rs) and the
//! completion (the host RM's non-stall notifier reaches the host event thread, which raises
//! the guest vector; the interrupt page's reactor folds it into the page and writes the MSI,
//! intr_page.rs). With the switch on, each step is timed into a fixed histogram and the exit
//! summary prints one line per step. Off (the default), every hook is one relaxed load.
//!
//! The submission steps are timed on the thread that handles the ring: [`begin`] starts a
//! ring at the instant `epoll_wait` returned, each [`mark`] records the time since the
//! previous mark (or the start) into its step, [`end`] closes the ring. A trapped doorbell
//! (the vfio-user server thread, every Windows doorbell under QEMU) starts its ring with
//! [`begin_trapped`] when its message arrived (device.rs `SharedDevice`). The completion
//! steps cross threads and are timed from shared timestamps: the oldest host doorbell no
//! host event answered yet ([`host_event`]), the last host event and the last raise.
//!
//! Nothing here allocates after start or takes a lock; the counters are relaxed atomics.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The switch: `1` times the launch path.
pub const TIMING_ENV: &str = "CARAXES_LAUNCH_TIMING";

/// One step of the chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// `epoll_wait` returned to the Device mutex held (device.rs).
    Lock,
    /// Mutex held to the mirror's doorbell entry: the token's channel, the BAR2 memfd
    /// sync, `note_rung`, the scheduling check, the notes (boot.rs, ctrl.rs).
    Dispatch,
    /// The ran-ahead evidence (mirror.rs `note_ran_ahead`, one USERD read).
    RanAhead,
    /// Planning what the guest mapped since the last ring (lazy flush, shadow re-sync).
    Plan,
    /// Waiting for the worker to apply every plan submitted so far (the ordering barrier).
    Wait,
    /// The BAR1 write-combine flush of the FB store.
    Flush,
    /// Reading GPPut for the forward record (mirror.rs `userd_ring`).
    Userd,
    /// The host doorbell write itself and its bookkeeping.
    HostRing,
    /// After the host doorbell to the end of the ring (error scan, notes, trace).
    Tail,
    /// `epoll_wait` returned to the host doorbell write: the device's share per ring.
    ToHost,
    /// The last host doorbell to the next ring's `epoll_wait` return: the GPU, the
    /// completion as the guest sees it, the guest's next submission and its exit.
    HostToNext,
    /// The oldest host doorbell no event answered yet to the next host non-stall event.
    HostEvent,
    /// A host event reaching the event thread to its raise done (the wake of the reactor).
    Raise,
    /// A raise to the MSI eventfd write by the interrupt page's reactor.
    Msi,
}

const STEPS: [Step; 14] = [
    Step::Lock,
    Step::Dispatch,
    Step::RanAhead,
    Step::Plan,
    Step::Wait,
    Step::Flush,
    Step::Userd,
    Step::HostRing,
    Step::Tail,
    Step::ToHost,
    Step::HostToNext,
    Step::HostEvent,
    Step::Raise,
    Step::Msi,
];

impl Step {
    fn name(self) -> &'static str {
        match self {
            Step::Lock => "lock",
            Step::Dispatch => "dispatch",
            Step::RanAhead => "ran-ahead check",
            Step::Plan => "plan",
            Step::Wait => "worker wait",
            Step::Flush => "BAR1 flush",
            Step::Userd => "USERD read",
            Step::HostRing => "host doorbell write",
            Step::Tail => "tail after the host doorbell",
            Step::ToHost => "ring to host doorbell (sum)",
            Step::HostToNext => "host doorbell to the next ring",
            Step::HostEvent => "host doorbell to the next host event",
            Step::Raise => "host event to raise",
            Step::Msi => "raise to MSI",
        }
    }
}

/// Bucket edges: 100 ns up to 100 us (1000 buckets), then 10 us up to 10 ms (990), then
/// one overflow bucket.
const FINE: u64 = 100;
const FINE_N: u64 = 1000;
const COARSE: u64 = 10_000;
const COARSE_N: u64 = 990;
const BUCKETS: usize = (FINE_N + COARSE_N) as usize + 1;

fn bucket(ns: u64) -> usize {
    if ns < FINE * FINE_N {
        (ns / FINE) as usize
    } else {
        let c = (ns - FINE * FINE_N) / COARSE;
        (FINE_N + c.min(COARSE_N)) as usize
    }
}

/// The upper edge of bucket `i` in nanoseconds (the overflow bucket has none).
fn upper_ns(i: usize) -> Option<u64> {
    let i = i as u64;
    if i < FINE_N {
        Some((i + 1) * FINE)
    } else if i < FINE_N + COARSE_N {
        Some(FINE * FINE_N + (i - FINE_N + 1) * COARSE)
    } else {
        None
    }
}

/// A histogram several threads may write (fetch_add).
pub struct Hist {
    buckets: Box<[AtomicU64]>,
    count: AtomicU64,
    sum_ns: AtomicU64,
    max_ns: AtomicU64,
}

impl Default for Hist {
    fn default() -> Self {
        Hist {
            buckets: (0..BUCKETS).map(|_| AtomicU64::new(0)).collect(),
            count: AtomicU64::new(0),
            sum_ns: AtomicU64::new(0),
            max_ns: AtomicU64::new(0),
        }
    }
}

impl Hist {
    pub fn record_ns(&self, ns: u64) {
        self.buckets[bucket(ns)].fetch_add(1, Relaxed);
        self.count.fetch_add(1, Relaxed);
        self.sum_ns.fetch_add(ns, Relaxed);
        self.max_ns.fetch_max(ns, Relaxed);
    }

    pub fn count(&self) -> u64 {
        self.count.load(Relaxed)
    }

    pub fn mean_us(&self) -> f64 {
        let n = self.count();
        if n == 0 {
            return 0.0;
        }
        self.sum_ns.load(Relaxed) as f64 / n as f64 / 1000.0
    }

    /// The upper edge of the bucket holding quantile `q`, in microseconds; the maximum
    /// when it lies in the overflow bucket.
    pub fn quantile_us(&self, q: f64) -> f64 {
        let n = self.count();
        if n == 0 {
            return 0.0;
        }
        let want = ((n as f64) * q).ceil().max(1.0) as u64;
        let mut seen = 0;
        for (i, b) in self.buckets.iter().enumerate() {
            seen += b.load(Relaxed);
            if seen >= want {
                return match upper_ns(i) {
                    Some(ns) => ns as f64 / 1000.0,
                    None => self.max_ns.load(Relaxed) as f64 / 1000.0,
                };
            }
        }
        self.max_ns.load(Relaxed) as f64 / 1000.0
    }

    fn line(&self) -> String {
        format!(
            "n {}, mean {:.2} us, p10 {:.1} us, p50 {:.1} us, p90 {:.1} us, p99 {:.1} us, max {:.1} us",
            self.count(),
            self.mean_us(),
            self.quantile_us(0.1),
            self.quantile_us(0.5),
            self.quantile_us(0.9),
            self.quantile_us(0.99),
            self.max_ns.load(Relaxed) as f64 / 1000.0
        )
    }
}

/// The chain's histograms and counters.
pub struct Chain {
    hist: Vec<Hist>,
    /// Rings handled by the reactor, and those whose `epoll_wait` had blocked (the
    /// reactor was asleep: the scheduler wake-up lies before `seen`).
    pub rings: AtomicU64,
    pub rings_after_block: AtomicU64,
    /// Doorbells the vfio-user server thread handled (trapped, [`begin_trapped`]).
    pub trapped_rings: AtomicU64,
    /// Host events with no host doorbell waiting for one (another channel's work, or a
    /// channel that fetched its GPPut without a doorbell): not in [`Step::HostEvent`].
    pub events_unarmed: AtomicU64,
    /// Host doorbell writes (any thread).
    pub host_doorbells: AtomicU64,
    /// Host non-stall events, by the age of the last host doorbell of this device at
    /// their arrival: none yet, below 1 ms, below 100 ms, older.
    pub events_no_doorbell: AtomicU64,
    pub events_1ms: AtomicU64,
    pub events_100ms: AtomicU64,
    pub events_older: AtomicU64,
    /// Nanoseconds since [`epoch`] of the last host doorbell, host event and raise; 0: none.
    last_host_ns: AtomicU64,
    /// The oldest host doorbell since the last host event (0: none waiting).
    armed_ns: AtomicU64,
    last_event_ns: AtomicU64,
    last_raise_ns: AtomicU64,
}

impl Default for Chain {
    fn default() -> Self {
        Chain {
            hist: STEPS.iter().map(|_| Hist::default()).collect(),
            rings: AtomicU64::new(0),
            rings_after_block: AtomicU64::new(0),
            trapped_rings: AtomicU64::new(0),
            events_unarmed: AtomicU64::new(0),
            host_doorbells: AtomicU64::new(0),
            events_no_doorbell: AtomicU64::new(0),
            events_1ms: AtomicU64::new(0),
            events_100ms: AtomicU64::new(0),
            events_older: AtomicU64::new(0),
            last_host_ns: AtomicU64::new(0),
            armed_ns: AtomicU64::new(0),
            last_event_ns: AtomicU64::new(0),
            last_raise_ns: AtomicU64::new(0),
        }
    }
}

impl Chain {
    pub fn hist(&self, s: Step) -> &Hist {
        &self.hist[STEPS
            .iter()
            .position(|x| *x == s)
            .expect("every step is listed")]
    }

    /// The exit summary: one header line and one line per step that recorded anything.
    pub fn summary(&self) -> String {
        let mut s = format!(
            "launch timing: {} ring(s) by the reactor ({} after a blocking wait), {} trapped, {} host doorbell(s); host events: {} with no host doorbell waiting; by the age of the last host doorbell: {} none yet, {} < 1 ms, {} < 100 ms, {} older ({TIMING_ENV}=1, docs/caraxes/launch-path.md)\n",
            self.rings.load(Relaxed),
            self.rings_after_block.load(Relaxed),
            self.trapped_rings.load(Relaxed),
            self.host_doorbells.load(Relaxed),
            self.events_unarmed.load(Relaxed),
            self.events_no_doorbell.load(Relaxed),
            self.events_1ms.load(Relaxed),
            self.events_100ms.load(Relaxed),
            self.events_older.load(Relaxed),
        );
        for st in STEPS {
            let h = self.hist(st);
            if h.count() > 0 {
                s.push_str(&format!("launch timing: {}: {}\n", st.name(), h.line()));
            }
        }
        s
    }
}

static ON: AtomicBool = AtomicBool::new(false);
/// Nanoseconds since [`epoch`] of the last host doorbell while the interrupt gate is on
/// (0: none yet); the gate's measure of this device's own work.
static GATE_LAST_DOORBELL_NS: AtomicU64 = AtomicU64::new(0);
/// The gate's hold in milliseconds (0: off).
static GATE_HOLD_MS: AtomicU64 = AtomicU64::new(0);
static FAST: AtomicBool = AtomicBool::new(false);
/// The lean evidence ([`evidence_needed`]).
static LEAN: AtomicBool = AtomicBool::new(false);
/// Ran-ahead checks skipped by the lean evidence (no plan pending).
pub static EVIDENCE_SKIPPED: AtomicU64 = AtomicU64::new(0);
/// BAR1 flushes done and skipped (nothing stored since the last one) under the fast switch.
pub static FLUSH_DONE: AtomicU64 = AtomicU64::new(0);
pub static FLUSH_SKIPPED: AtomicU64 = AtomicU64::new(0);
static CHAIN: OnceLock<Chain> = OnceLock::new();
static EPOCH: OnceLock<Instant> = OnceLock::new();

thread_local! {
    /// The previous mark of the ring this thread handles; `None` outside a ring.
    static AT: Cell<Option<Instant>> = const { Cell::new(None) };
    /// When this thread's ring started (`epoll_wait` returned).
    static SEEN: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Read the switch once at start: `1` turns timing on, unset or `0` leaves it off,
/// anything else is refused.
pub fn configure() -> Result<bool, String> {
    let on = match std::env::var(TIMING_ENV).ok().as_deref().map(str::trim) {
        None | Some("") | Some("0") => false,
        Some("1") => true,
        Some(v) => return Err(format!("{TIMING_ENV}={v:?}: 1 or 0")),
    };
    set(on);
    Ok(on)
}

/// The device-side cuts of the ring path at start (docs/caraxes/launch-path.md §3): USERD's
/// GPGet and GPPut read as one qword and GPPut alone for the forward record, no BAR1
/// write-combine flush when the device stored nothing through its windows since the last
/// one, no `note_rung` USERD read under the token pool, a ring's trace notes written with
/// one flush; and the lean evidence, the ran-ahead check's USERD read only at a doorbell
/// with plans pending, the one case it logs ([`evidence_needed`]). The order of a ring
/// (plans applied, flush, GPPut read, host doorbell) is kept. The default since 2026-09-29
/// (`CARAXES_LAUNCH_FAST=2`, its variable gone on 2026-09-30): it ran every Linux and
/// Windows gate without an Xid and took the short kernel from 20 to 13 us
/// (caraxes/runs/night-2026-09-28/README.md).
pub fn configure_fast() {
    set_fast(true);
    LEAN.store(true, Relaxed);
}

/// Whether the lean evidence is on.
pub fn lean() -> bool {
    LEAN.load(Relaxed)
}

/// Whether a doorbell reads USERD for the ran-ahead evidence (mirror.rs `note_ran_ahead`).
/// The check only counts a doorbell whose host channel ran ahead and logs the ones with
/// plans pending (`pending`: deferred, being applied, or submitted without a wait since the
/// last forward). The lean evidence skips the uncached read when nothing is pending: the
/// logged lines and the pending count stay exact, the total count covers only the pending
/// doorbells. It never changes a ring's order or content; the forward record is read after
/// the barrier and the flush as before.
pub fn evidence_needed(pending: bool) -> bool {
    if pending || !(lean() || lean_here()) {
        return true;
    }
    EVIDENCE_SKIPPED.fetch_add(1, Relaxed);
    false
}

/// Turn the fast ring path on or off (tests, `configure_fast`).
pub fn set_fast(on: bool) {
    FAST.store(on, Relaxed);
}

/// Whether the fast ring path is on.
#[inline]
pub fn fast() -> bool {
    FAST.load(Relaxed) || fast_here()
}

#[cfg(test)]
thread_local! {
    static FAST_HERE: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
fn fast_here() -> bool {
    FAST_HERE.get()
}

#[cfg(test)]
thread_local! {
    static LEAN_HERE: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
fn lean_here() -> bool {
    LEAN_HERE.get()
}

#[cfg(not(test))]
#[inline]
fn lean_here() -> bool {
    false
}

/// Tests: the lean evidence for the calling thread only.
#[cfg(test)]
pub fn set_lean_here(on: bool) {
    LEAN_HERE.set(on);
}

#[cfg(not(test))]
#[inline]
fn fast_here() -> bool {
    false
}

/// Tests: the fast switch for the calling thread only.
#[cfg(test)]
pub fn set_fast_here(on: bool) {
    FAST_HERE.set(on);
}

/// The fast switch's line for the exit summary.
pub fn fast_summary() -> String {
    format!(
        "launch fast: BAR1 flushes {} done, {} skipped (no device store since the last one); ran-ahead checks skipped with no plan pending: {} (docs/caraxes/launch-path.md)\n",
        FLUSH_DONE.load(Relaxed),
        FLUSH_SKIPPED.load(Relaxed),
        EVIDENCE_SKIPPED.load(Relaxed),
    )
}

/// Whether a BAR1 write-combine flush is needed: the device stored through its windows
/// since the last flush (`dirty`), or the fast switch is off (every flush runs). The
/// ordering rule: a store of the device is never left in a write-combine buffer when the
/// host doorbell that makes the GPU read it is written; only a flush with nothing to drain
/// is skipped.
pub fn flush_needed(dirty: bool) -> bool {
    if !fast() {
        return true;
    }
    if dirty {
        FLUSH_DONE.fetch_add(1, Relaxed);
    } else {
        FLUSH_SKIPPED.fetch_add(1, Relaxed);
    }
    dirty
}

/// Turn timing on or off (tests, `configure`).
pub fn set(on: bool) {
    if on {
        let _ = EPOCH.get_or_init(Instant::now);
        let _ = CHAIN.get_or_init(Chain::default);
    }
    ON.store(on, Relaxed);
}

/// Whether timing is on.
#[inline]
pub fn on() -> bool {
    ON.load(Relaxed)
}

/// The chain, when timing was ever turned on.
pub fn chain() -> Option<&'static Chain> {
    CHAIN.get()
}

fn ns_of(t: Instant) -> u64 {
    // At least 1, so that 0 keeps meaning "none".
    (t.saturating_duration_since(*EPOCH.get_or_init(Instant::now))
        .as_nanos() as u64)
        .max(1)
}

/// The host interrupt gate (`CARAXES_INTR_GATE_MS=<hold>`, backlog item 5,
/// docs/caraxes/launch-path.md §4): the host's non-stall notifiers are engine-wide, so every
/// other client's work on the engine (the desktop, other VMs) raises this guest's vector
/// too. With the gate, an event that comes while this device wrote no host doorbell for
/// [`HOT_FACTOR`] times the hold is held, and all events held are delivered together once
/// the hold has passed since the first of them; an event within the hot window delivers at
/// once, with everything held. Rules: nothing is delivered before the host reported it
/// (the gate only delays), nothing held is lost (delivered at most one hold late), and RC
/// events are never gated (the event thread passes only non-stall vectors here).
pub const GATE_ENV: &str = "CARAXES_INTR_GATE_MS";
/// The hot window is this many holds.
pub const HOT_FACTOR: u32 = 10;

/// Read the gate's hold once at start: unset or 0 off, a number of milliseconds (1..=1000).
pub fn configure_gate() -> Result<Option<Duration>, String> {
    let ms = match std::env::var(GATE_ENV).ok().as_deref().map(str::trim) {
        None | Some("") | Some("0") => 0,
        Some(v) => match v.parse::<u64>() {
            Ok(n) if (1..=1000).contains(&n) => n,
            _ => return Err(format!("{GATE_ENV}={v:?}: milliseconds, 1 to 1000, or 0 for off")),
        },
    };
    GATE_HOLD_MS.store(ms, Relaxed);
    Ok((ms != 0).then(|| Duration::from_millis(ms)))
}

/// The gate's hold, when on.
pub fn gate_hold() -> Option<Duration> {
    match GATE_HOLD_MS.load(Relaxed) {
        0 => None,
        ms => Some(Duration::from_millis(ms)),
    }
}

/// The last host doorbell of this device as the gate saw it.
pub fn gate_last_doorbell() -> Option<Instant> {
    match GATE_LAST_DOORBELL_NS.load(Relaxed) {
        0 => None,
        ns => Some(*EPOCH.get_or_init(Instant::now) + Duration::from_nanos(ns)),
    }
}

/// The gate's state in the event thread: the vectors held (a bit per vector below 64; the
/// non-stall vectors are, mirror.rs `engine_vectors_are_in_the_chip_table`) and since when.
#[derive(Debug, Default)]
pub struct Gate {
    hold: Duration,
    held: u64,
    since: Option<Instant>,
    /// Events delivered at once, events held, deliveries of held events.
    pub passed: u64,
    pub held_events: u64,
    pub released: u64,
}

impl Gate {
    pub fn new(hold: Duration) -> Self {
        Gate {
            hold,
            ..Default::default()
        }
    }

    /// A host event for `vector` at `now`, the device's last host doorbell `last`: `None`
    /// when it is held, else the held vectors to raise together with it (the event's own
    /// vector is the caller's to raise).
    pub fn event(&mut self, now: Instant, vector: u32, last: Option<Instant>) -> Option<u64> {
        let hot = last.is_some_and(|t| now.saturating_duration_since(t) < self.hold * HOT_FACTOR);
        if hot || vector >= 64 {
            // A vector outside the mask is never held.
            self.passed += 1;
            if self.held != 0 {
                self.released += 1;
            }
            self.since = None;
            return Some(std::mem::take(&mut self.held));
        }
        self.held_events += 1;
        self.held |= 1u64 << vector;
        self.since.get_or_insert(now);
        None
    }

    /// The vectors whose hold has passed at `now` (released together), or none.
    pub fn due(&mut self, now: Instant) -> u64 {
        match self.since {
            Some(t) if now.saturating_duration_since(t) >= self.hold && self.held != 0 => {
                self.released += 1;
                self.since = None;
                std::mem::take(&mut self.held)
            }
            _ => 0,
        }
    }

    /// How long the event thread may wait before [`Gate::due`] has work: `max` when nothing
    /// is held.
    pub fn wait(&self, now: Instant, max: Duration) -> Duration {
        match self.since {
            Some(t) => (t + self.hold).saturating_duration_since(now).min(max),
            None => max,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "intr gate: hold {} ms, hot {} ms after a host doorbell; {} event(s) delivered at once, {} held, {} delivery(ies) of held events ({GATE_ENV}, docs/caraxes/launch-path.md)",
            self.hold.as_millis(),
            (self.hold * HOT_FACTOR).as_millis(),
            self.passed,
            self.held_events,
            self.released
        )
    }
}

/// USERD's `(GPGet, GPPut)` from one 8-byte read at GPGet (0x88; GPPut is the next dword,
/// 0x8C): `read` fills the buffer and says whether it could.
pub fn userd_pair(read: impl FnOnce(&mut [u8]) -> Option<bool>) -> Option<(u32, u32)> {
    let mut b = [0u8; 8];
    if !read(&mut b)? {
        return None;
    }
    Some((
        u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
    ))
}

/// A ring of the reactor starts: `seen` is when `epoll_wait` returned, `blocked` whether
/// that wait had blocked.
pub fn begin(seen: Instant, blocked: bool) {
    if !on() {
        return;
    }
    let Some(c) = chain() else { return };
    c.rings.fetch_add(1, Relaxed);
    if blocked {
        c.rings_after_block.fetch_add(1, Relaxed);
    }
    let last = c.last_host_ns.load(Relaxed);
    let now = ns_of(seen);
    if last != 0 && now > last {
        c.hist(Step::HostToNext).record_ns(now - last);
    }
    AT.set(Some(seen));
    SEEN.set(Some(seen));
}

/// A trapped doorbell's ring starts: `seen` is when its vfio-user message arrived, before
/// the Device mutex (the [`Step::Lock`] wait follows). Ended by [`end`].
pub fn begin_trapped(seen: Instant) {
    if !on() {
        return;
    }
    let Some(c) = chain() else { return };
    c.trapped_rings.fetch_add(1, Relaxed);
    let last = c.last_host_ns.load(Relaxed);
    let now = ns_of(seen);
    if last != 0 && now > last {
        c.hist(Step::HostToNext).record_ns(now - last);
    }
    AT.set(Some(seen));
    SEEN.set(Some(seen));
}

/// The step `s` of this thread's ring ended now.
#[inline]
pub fn mark(s: Step) {
    if !on() {
        return;
    }
    mark_slow(s);
}

fn mark_slow(s: Step) {
    let (Some(c), Some(t)) = (chain(), AT.get()) else {
        return;
    };
    let now = Instant::now();
    c.hist(s)
        .record_ns(now.saturating_duration_since(t).as_nanos() as u64);
    AT.set(Some(now));
}

/// The host doorbell was written now (any thread): the end of [`Step::HostRing`] and of
/// [`Step::ToHost`] for a reactor ring, and the start of the completion steps; with the
/// interrupt gate on, the gate's last sign of this device's own work.
#[inline]
pub fn host_doorbell() {
    if GATE_HOLD_MS.load(Relaxed) != 0 {
        GATE_LAST_DOORBELL_NS.store(ns_of(Instant::now()), Relaxed);
    }
    if !on() {
        return;
    }
    host_doorbell_slow();
}

fn host_doorbell_slow() {
    let Some(c) = chain() else { return };
    let now = Instant::now();
    c.host_doorbells.fetch_add(1, Relaxed);
    c.last_host_ns.store(ns_of(now), Relaxed);
    // The oldest doorbell waiting for an event stays; a later one does not move it.
    let _ = c.armed_ns.compare_exchange(0, ns_of(now), Relaxed, Relaxed);
    if let Some(t) = AT.get() {
        c.hist(Step::HostRing)
            .record_ns(now.saturating_duration_since(t).as_nanos() as u64);
        AT.set(Some(now));
    }
    if let Some(seen) = SEEN.get() {
        c.hist(Step::ToHost)
            .record_ns(now.saturating_duration_since(seen).as_nanos() as u64);
        // One sum per ring, even when a ring writes several host doorbells.
        SEEN.set(None);
    }
}

/// The ring ends: [`Step::Tail`] when it wrote a host doorbell.
pub fn end() {
    if !on() {
        return;
    }
    if AT.get().is_some() && SEEN.get().is_none() {
        mark_slow(Step::Tail);
    }
    AT.set(None);
    SEEN.set(None);
}

/// A host non-stall event reached the event thread now.
pub fn host_event() {
    if !on() {
        return;
    }
    let Some(c) = chain() else { return };
    let now = ns_of(Instant::now());
    c.last_event_ns.store(now, Relaxed);
    let last = c.last_host_ns.load(Relaxed);
    if last == 0 {
        c.events_no_doorbell.fetch_add(1, Relaxed);
        return;
    }
    // The completion time: from the oldest host doorbell this event is the first answer
    // to. An event with none waiting is not a completion of a doorbell (on Windows most
    // are: channels that fetched GPPut from USERD without one, the KMD's own events).
    match c.armed_ns.swap(0, Relaxed) {
        0 => {
            c.events_unarmed.fetch_add(1, Relaxed);
        }
        armed => c.hist(Step::HostEvent).record_ns(now.saturating_sub(armed)),
    }
    let age = now.saturating_sub(last);
    let bin = if age < 1_000_000 {
        &c.events_1ms
    } else if age < 100_000_000 {
        &c.events_100ms
    } else {
        &c.events_older
    };
    bin.fetch_add(1, Relaxed);
}

/// The event thread raised the vector for the event of [`host_event`].
pub fn raised() {
    if !on() {
        return;
    }
    let Some(c) = chain() else { return };
    let now = ns_of(Instant::now());
    let ev = c.last_event_ns.load(Relaxed);
    if ev != 0 && now >= ev {
        c.hist(Step::Raise).record_ns(now - ev);
    }
    c.last_raise_ns.store(now, Relaxed);
}

/// The interrupt page's reactor wrote the MSI for a raise.
pub fn msi_for_raise() {
    if !on() {
        return;
    }
    let Some(c) = chain() else { return };
    let r = c.last_raise_ns.swap(0, Relaxed);
    let now = ns_of(Instant::now());
    if r != 0 && now >= r {
        c.hist(Step::Msi).record_ns(now - r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lean evidence skips the ran-ahead read only with nothing pending and the switch
    /// on; a doorbell with plans pending always reads.
    #[test]
    fn lean_evidence_reads_whenever_plans_are_pending() {
        set_lean_here(false);
        assert!(evidence_needed(false), "off: every doorbell reads");
        assert!(evidence_needed(true));
        set_lean_here(true);
        let before = EVIDENCE_SKIPPED.load(Relaxed);
        assert!(evidence_needed(true), "pending: read");
        assert!(!evidence_needed(false), "nothing pending: skipped");
        assert!(EVIDENCE_SKIPPED.load(Relaxed) > before);
        set_lean_here(false);
    }

    #[test]
    fn buckets_are_monotonic_and_bounded() {
        assert_eq!(bucket(0), 0);
        assert_eq!(bucket(99), 0);
        assert_eq!(bucket(100), 1);
        assert_eq!(bucket(99_999), 999);
        assert_eq!(bucket(100_000), 1000);
        assert_eq!(bucket(109_999), 1000);
        assert_eq!(bucket(10_000_000 - 1), BUCKETS - 2);
        assert_eq!(bucket(u64::MAX), BUCKETS - 1);
        let mut last = 0;
        for i in 0..BUCKETS - 1 {
            let u = upper_ns(i).unwrap();
            assert!(u > last);
            assert_eq!(bucket(u - 1), i);
            last = u;
        }
        assert_eq!(upper_ns(BUCKETS - 1), None);
    }

    #[test]
    fn quantiles() {
        let h = Hist::default();
        for us in 1..=100 {
            h.record_ns(us * 1000);
        }
        assert_eq!(h.count(), 100);
        assert!((h.quantile_us(0.5) - 50.1).abs() < 0.2);
        assert!((h.quantile_us(0.99) - 99.1).abs() < 0.2);
        h.record_ns(50_000_000);
        assert_eq!(h.quantile_us(1.0), 50_000.0);
    }

    #[test]
    fn userd_pair_splits_the_qword() {
        // GPGet 0x11 at 0x88, GPPut 0x22 at 0x8C, little endian.
        let q = [0x11u8, 0, 0, 0, 0x22, 0, 0, 0];
        let pair = userd_pair(|b| {
            b.copy_from_slice(&q);
            Some(true)
        });
        assert_eq!(pair, Some((0x11, 0x22)));
        assert_eq!(userd_pair(|_| Some(false)), None);
        assert_eq!(userd_pair(|_| None), None);
    }

    #[test]
    fn flush_gate_skips_only_an_empty_flush() {
        // Off: every flush runs, dirty or not.
        set_fast_here(false);
        assert!(flush_needed(false));
        assert!(flush_needed(true));
        // On: a flush with nothing stored since the last one is skipped, never one after a
        // store.
        set_fast_here(true);
        assert!(flush_needed(true));
        assert!(!flush_needed(false));
        set_fast_here(false);
    }

    /// The gate never delivers an event before the host reported it, never loses one, and
    /// bounds every hold.
    #[test]
    fn gate_delays_foreign_events_and_never_loses_one() {
        let hold = Duration::from_millis(50);
        let mut g = Gate::new(hold);
        let t0 = Instant::now();
        let ms = |n: u64| Duration::from_millis(n);
        // No doorbell yet: held; nothing is due before the hold has passed.
        assert_eq!(g.event(t0, 1, None), None);
        assert_eq!(g.due(t0 + ms(49)), 0);
        assert_eq!(g.wait(t0 + ms(10), ms(250)), ms(40));
        // A second vector joins the hold without restarting it.
        assert_eq!(g.event(t0 + ms(20), 2, None), None);
        assert_eq!(g.due(t0 + ms(50)), 0b110);
        assert_eq!(g.due(t0 + ms(51)), 0, "delivered once");
        assert_eq!(g.wait(t0 + ms(51), ms(250)), ms(250));
        // Within the hot window (10 holds) of a doorbell: at once.
        let d = t0 + ms(100);
        assert_eq!(g.event(d + ms(1), 1, Some(d)), Some(0));
        assert_eq!(g.event(d + ms(499), 1, Some(d)), Some(0));
        // Past it: held again, and a hot event releases what is held with it.
        assert_eq!(g.event(d + ms(500), 3, Some(d)), None);
        let d2 = d + ms(510);
        assert_eq!(g.event(d2, 1, Some(d2)), Some(0b1000));
        assert_eq!(g.due(d2 + ms(100)), 0);
        // A vector outside the mask is never held, even cold.
        assert_eq!(g.event(d2 + ms(10_000), 70, Some(d2)), Some(0));
        assert_eq!(g.held_events, 3);
        assert_eq!(g.released, 2);
        assert!(g.summary().contains("hold 50 ms, hot 500 ms"));
    }

    /// One test drives the process-wide switch, so the steps run in order here.
    #[test]
    fn a_ring_records_its_steps_in_order() {
        set(true);
        let c = chain().unwrap();
        let before = c.hist(Step::ToHost).count();
        let seen = Instant::now();
        begin(seen, true);
        std::thread::sleep(Duration::from_micros(200));
        mark(Step::Lock);
        mark(Step::Dispatch);
        host_doorbell();
        end();
        assert_eq!(c.hist(Step::ToHost).count(), before + 1);
        assert!(c.hist(Step::Lock).quantile_us(1.0) >= 200.0);
        assert!(c.rings_after_block.load(Relaxed) >= 1);
        // Outside a ring (a trapped doorbell) marks record nothing.
        let n = c.hist(Step::Lock).count();
        mark(Step::Lock);
        assert_eq!(c.hist(Step::Lock).count(), n);
        // The completion side: event, raise, MSI.
        let events = c.hist(Step::HostEvent).count();
        host_event();
        raised();
        msi_for_raise();
        assert_eq!(c.hist(Step::HostEvent).count(), events + 1);
        // A second event with no host doorbell since is not a completion.
        let unarmed = c.events_unarmed.load(Relaxed);
        host_event();
        assert_eq!(c.hist(Step::HostEvent).count(), events + 1);
        assert_eq!(c.events_unarmed.load(Relaxed), unarmed + 1);
        // Two doorbells, one event: timed from the older one.
        host_doorbell();
        std::thread::sleep(Duration::from_micros(300));
        host_doorbell();
        host_event();
        assert!(c.hist(Step::HostEvent).quantile_us(1.0) >= 300.0);
        // A trapped doorbell times its ring from its message.
        let (trapped, to_host) = (c.trapped_rings.load(Relaxed), c.hist(Step::ToHost).count());
        begin_trapped(Instant::now());
        mark(Step::Lock);
        host_doorbell();
        end();
        assert_eq!(c.trapped_rings.load(Relaxed), trapped + 1);
        assert_eq!(c.hist(Step::ToHost).count(), to_host + 1);
        assert!(c.hist(Step::Raise).count() >= 1);
        assert!(c.hist(Step::Msi).count() >= 1);
        // A second MSI without a raise records nothing.
        let m = c.hist(Step::Msi).count();
        msi_for_raise();
        assert_eq!(c.hist(Step::Msi).count(), m);
        assert!(c.summary().contains("ring to host doorbell (sum)"));
    }
}

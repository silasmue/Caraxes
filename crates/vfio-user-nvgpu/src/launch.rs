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
//! (the vfio-user server thread) never calls [`begin`], so its marks record nothing. The
//! completion steps cross threads and are timed from shared timestamps: the last host
//! doorbell ([`Step::HostRing`]), the last host event ([`host_event`]) and the last raise.
//!
//! Nothing here allocates after start or takes a lock; the counters are relaxed atomics.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::OnceLock;
use std::time::Instant;

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
    /// The last host doorbell to a host non-stall event reaching the event thread.
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
            Step::HostEvent => "host doorbell to host event",
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
    last_event_ns: AtomicU64,
    last_raise_ns: AtomicU64,
}

impl Default for Chain {
    fn default() -> Self {
        Chain {
            hist: STEPS.iter().map(|_| Hist::default()).collect(),
            rings: AtomicU64::new(0),
            rings_after_block: AtomicU64::new(0),
            host_doorbells: AtomicU64::new(0),
            events_no_doorbell: AtomicU64::new(0),
            events_1ms: AtomicU64::new(0),
            events_100ms: AtomicU64::new(0),
            events_older: AtomicU64::new(0),
            last_host_ns: AtomicU64::new(0),
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
            "launch timing: {} ring(s) by the reactor ({} after a blocking wait), {} host doorbell(s); host events by the age of the last host doorbell: {} none yet, {} < 1 ms, {} < 100 ms, {} older ({TIMING_ENV}=1, docs/caraxes/launch-path.md)\n",
            self.rings.load(Relaxed),
            self.rings_after_block.load(Relaxed),
            self.host_doorbells.load(Relaxed),
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
/// [`Step::ToHost`] for a reactor ring, and the start of the completion steps.
#[inline]
pub fn host_doorbell() {
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
    let age = now.saturating_sub(last);
    c.hist(Step::HostEvent).record_ns(age);
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
    use std::time::Duration;

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
        host_event();
        raised();
        msi_for_raise();
        assert!(c.hist(Step::HostEvent).count() >= 1);
        assert!(c.hist(Step::Raise).count() >= 1);
        assert!(c.hist(Step::Msi).count() >= 1);
        // A second MSI without a raise records nothing.
        let m = c.hist(Step::Msi).count();
        msi_for_raise();
        assert_eq!(c.hist(Step::Msi).count(), m);
        assert!(c.summary().contains("ring to host doorbell (sum)"));
    }
}

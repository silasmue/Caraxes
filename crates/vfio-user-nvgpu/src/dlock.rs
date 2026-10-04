// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The device lock's holds by holder (docs/caraxes/execution.md §9).
//!
//! Every thread that takes the device lock (D) takes it through [`lock`] with a label: the
//! vfio-user server loop by region and direction, the doorbell and re-arm readers, the
//! usermode page's reactor, the display clock, the BAR2 flush clock, the held-ring completer,
//! the exit. The guard records how long D was held when it is dropped: a log2 histogram per
//! label, and the longest holds with their label and detail (the register of a trapped
//! access), for the exit summary. Waiting for D is not counted here (the readers' lock-wait
//! lines count that); a hold is from the acquisition to the release.
//!
//! What a hold did: the work under it tags the hold ([`tag`]: an RPC, a mirror build, a
//! channel's doorbell), and a long hold keeps its tags as detail. The GSP RPCs are also timed
//! by function and command or class ([`work`]), for the exit summary's work table.
//! `CARAXES_DLOCK_STATS=0` skips this diagnostic work and software-part timers; it does
//! not change the mutex, its guard, or functional submission/reply ordering.

use std::cell::{Cell, RefCell};
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

/// Lock diagnostics remain on unless explicitly disabled for a scored run.
pub const STATS_ENV: &str = "CARAXES_DLOCK_STATS";

fn stats_requested(value: Option<&str>) -> bool {
    !matches!(value.map(str::trim), Some("0"))
}

fn stats_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| stats_requested(std::env::var(STATS_ENV).ok().as_deref()))
}

/// Log2 microsecond buckets: bucket k holds holds in [2^(k-1), 2^k) us (bucket 0: < 1 us).
const BUCKETS: usize = 32;
/// Longest holds kept.
const LONGEST: usize = 8;
/// Holds at least this long keep their detail.
const DETAIL_AT: Duration = Duration::from_millis(5);
/// Tags kept per hold.
const TAGS: usize = 12;
/// Rows of the work table in the summary.
const WORK_ROWS: usize = 16;

thread_local! {
    /// The tags of the hold this thread is in, and how deep (a guard inside a guard).
    static HOLD_TAGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Tag the device-lock hold this thread is in with what it does now (computed only inside a
/// hold, and only for the first [`TAGS`] tags).
pub fn tag(f: impl FnOnce() -> String) {
    tag_with_stats(stats_enabled(), f);
}

fn tag_with_stats(on: bool, f: impl FnOnce() -> String) {
    if !on {
        return;
    }
    if DEPTH.with(|d| d.get()) == 0 {
        return;
    }
    HOLD_TAGS.with(|t| {
        let mut t = t.borrow_mut();
        if t.len() < TAGS {
            t.push(f());
        } else if t.len() == TAGS {
            t.push("...".into());
        }
    });
}

/// The parts of a software channel's doorbell timed apart ([`Spend`]).
#[derive(Clone, Copy, Debug)]
pub enum Part {
    /// Resolving guest VAs through the page tables (channel.rs `GpuMem::pieces`).
    Translate = 0,
    /// The software CE's CPU reads, writes and fills.
    Copy = 1,
    /// Reading GPFIFO entries and pushbuffer segments (their translations included).
    Fetch = 2,
    /// A software `MEM_OP` TLB invalidate's shadow plans, waited for until applied.
    Invalidate = 3,
    /// The GPGet store after every entry.
    GpGet = 4,
}

pub const PARTS: usize = 5;

impl Part {
    fn name(self) -> &'static str {
        match self {
            Part::Translate => "translate",
            Part::Copy => "CPU copy",
            Part::Fetch => "fetch",
            Part::Invalidate => "invalidate wait",
            Part::GpGet => "GPGet store",
        }
    }
}

thread_local! {
    /// Time spent per part on this thread since the last [`take_spent`], and whether a
    /// guard of the part is running (a nested one does not count twice).
    static SPENT: RefCell<([u64; PARTS], [bool; PARTS])> = const { RefCell::new(([0; PARTS], [false; PARTS])) };
}

/// Times one [`Part`] until dropped: into this thread's per-doorbell sums
/// ([`take_spent`]) and the work table (`software` / the part).
pub struct Spend {
    part: Part,
    t0: Option<Instant>,
}

impl Spend {
    pub fn new(part: Part) -> Self {
        Self::new_with_stats(part, stats_enabled())
    }

    fn new_with_stats(part: Part, on: bool) -> Self {
        if !on {
            return Spend { part, t0: None };
        }
        let outer = SPENT.with(|s| {
            let mut s = s.borrow_mut();
            !std::mem::replace(&mut s.1[part as usize], true)
        });
        Spend {
            part,
            t0: outer.then(Instant::now),
        }
    }
}

impl Drop for Spend {
    fn drop(&mut self) {
        let Some(t0) = self.t0 else {
            return;
        };
        let d = t0.elapsed();
        SPENT.with(|s| {
            let mut s = s.borrow_mut();
            s.0[self.part as usize] += d.as_nanos() as u64;
            s.1[self.part as usize] = false;
        });
        // An active timer is diagnostic-enabled even in explicit-mode tests.
        work_with_stats(true, "software", self.part.name().to_string(), d);
    }
}

/// This thread's time per [`Part`] since the last call, reset.
pub fn take_spent() -> [Duration; PARTS] {
    SPENT.with(|s| {
        let v = std::mem::take(&mut s.borrow_mut().0);
        v.map(Duration::from_nanos)
    })
}

#[derive(Clone, Debug, Default)]
struct WorkRow {
    count: u64,
    total_ns: u64,
    max_ns: u64,
}

type WorkTable = std::collections::HashMap<(&'static str, String), WorkRow>;

static WORK: Mutex<Option<WorkTable>> = Mutex::new(None);

/// Record `d` of work `kind`/`key` under the device lock (a GSP RPC by function and command).
pub fn work(kind: &'static str, key: String, d: Duration) {
    work_with_stats(stats_enabled(), kind, key, d);
}

fn work_with_stats(on: bool, kind: &'static str, key: String, d: Duration) {
    if !on {
        return;
    }
    let ns = d.as_nanos() as u64;
    let mut w = WORK.lock().unwrap_or_else(PoisonError::into_inner);
    let row = w.get_or_insert_with(Default::default).entry((kind, key)).or_default();
    row.count += 1;
    row.total_ns += ns;
    row.max_ns = row.max_ns.max(ns);
}

#[derive(Clone, Debug, Default)]
struct Row {
    count: u64,
    total_ns: u64,
    max_ns: u64,
    buckets: [u64; BUCKETS],
}

#[derive(Default)]
struct Stats {
    rows: Vec<(&'static str, Row)>,
    longest: Vec<(u64, &'static str, String)>,
}

static STATS: Mutex<Stats> = Mutex::new(Stats {
    rows: Vec::new(),
    longest: Vec::new(),
});

fn bucket(d: Duration) -> usize {
    let us = d.as_micros() as u64;
    if us == 0 {
        0
    } else {
        ((64 - us.leading_zeros()) as usize).min(BUCKETS - 1)
    }
}

/// Record one hold of `d` by `label` (`detail` computed only for a long one).
pub fn record(label: &'static str, d: Duration, detail: impl FnOnce() -> String) {
    record_with_stats(stats_enabled(), label, d, detail);
}

fn record_with_stats(on: bool, label: &'static str, d: Duration, detail: impl FnOnce() -> String) {
    if !on {
        return;
    }
    let ns = d.as_nanos() as u64;
    let mut st = STATS.lock().unwrap_or_else(PoisonError::into_inner);
    let row = match st.rows.iter().position(|(l, _)| *l == label) {
        Some(i) => &mut st.rows[i].1,
        None => {
            st.rows.push((label, Row::default()));
            &mut st.rows.last_mut().expect("pushed").1
        }
    };
    row.count += 1;
    row.total_ns += ns;
    row.max_ns = row.max_ns.max(ns);
    row.buckets[bucket(d)] += 1;
    if d >= DETAIL_AT {
        let least = st.longest.iter().map(|(n, _, _)| *n).min().unwrap_or(0);
        if st.longest.len() < LONGEST || ns > least {
            st.longest.push((ns, label, detail()));
            st.longest.sort_by(|a, b| b.0.cmp(&a.0));
            st.longest.truncate(LONGEST);
        }
    }
}

/// The hold at quantile `q` of a row, from its buckets (the bucket's upper bound).
fn quantile(r: &Row, q: f64) -> Duration {
    if r.count == 0 {
        return Duration::ZERO;
    }
    let want = ((r.count as f64) * q).ceil() as u64;
    let mut seen = 0;
    for (k, n) in r.buckets.iter().enumerate() {
        seen += n;
        if seen >= want {
            return Duration::from_micros(if k == 0 { 1 } else { 1u64 << k });
        }
    }
    Duration::from_nanos(r.max_ns)
}

/// The exit summary's lines: holds by label (count, total, p50/p99 bucket bounds, max), most
/// total time first, and the longest holds.
pub fn summary() -> Vec<String> {
    summary_with_stats(stats_enabled())
}

fn summary_with_stats(on: bool) -> Vec<String> {
    if !on {
        return vec![format!("device lock statistics disabled ({STATS_ENV}=0)")];
    }
    let st = STATS.lock().unwrap_or_else(PoisonError::into_inner);
    let mut rows = st.rows.clone();
    rows.sort_by(|a, b| b.1.total_ns.cmp(&a.1.total_ns));
    let mut out = vec![format!(
        "device lock holds by holder (docs/caraxes/execution.md §9): {}",
        rows.iter()
            .map(|(l, r)| format!(
                "{l} {} hold(s) {} ms, p50 <= {} us, p99 <= {} us, max {} us",
                r.count,
                r.total_ns / 1_000_000,
                quantile(r, 0.5).as_micros(),
                quantile(r, 0.99).as_micros(),
                r.max_ns / 1000
            ))
            .collect::<Vec<_>>()
            .join("; ")
    )];
    let mut work: Vec<((&'static str, String), WorkRow)> = WORK
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|w| w.iter().map(|(k, r)| (k.clone(), r.clone())).collect())
        .unwrap_or_default();
    if !work.is_empty() {
        work.sort_by(|a, b| b.1.total_ns.cmp(&a.1.total_ns));
        out.push(format!(
            "device lock work by kind (top {WORK_ROWS} by total of {}): {}",
            work.len(),
            work.iter()
                .take(WORK_ROWS)
                .map(|((kind, key), r)| format!(
                    "{kind} {key} {} x {} ms (max {} us)",
                    r.count,
                    r.total_ns / 1_000_000,
                    r.max_ns / 1000
                ))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    if !st.longest.is_empty() {
        out.push(format!(
            "device lock: longest holds: {}",
            st.longest
                .iter()
                .map(|(ns, l, d)| format!("{} us {l}{}", ns / 1000, if d.is_empty() { String::new() } else { format!(" ({d})") }))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    out
}

/// The device lock taken by `label`; the hold is recorded when the guard is dropped.
pub struct Held<'a, T> {
    guard: Option<MutexGuard<'a, T>>,
    label: &'static str,
    detail: Option<Box<dyn FnOnce() -> String + 'a>>,
    t0: Instant,
    stats_on: bool,
}

/// Take `m` for `label`.
pub fn lock<'a, T>(m: &'a Mutex<T>, label: &'static str) -> Held<'a, T> {
    lock_with_stats(m, label, stats_enabled())
}

fn lock_with_stats<'a, T>(m: &'a Mutex<T>, label: &'static str, on: bool) -> Held<'a, T> {
    let guard = m.lock().unwrap_or_else(PoisonError::into_inner);
    Held::new(guard, label, on)
}

/// Take `m` for `label` if nobody holds it; `None` while it is held (the display clock
/// then ticks without it, display/core_path.rs).
pub fn try_lock<'a, T>(m: &'a Mutex<T>, label: &'static str) -> Option<Held<'a, T>> {
    try_lock_with_stats(m, label, stats_enabled())
}

fn try_lock_with_stats<'a, T>(m: &'a Mutex<T>, label: &'static str, on: bool) -> Option<Held<'a, T>> {
    let guard = match m.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return None,
    };
    Some(Held::new(guard, label, on))
}

impl<'a, T> Held<'a, T> {
    fn new(guard: MutexGuard<'a, T>, label: &'static str, on: bool) -> Self {
        if on && DEPTH.with(|d| d.replace(d.get() + 1)) == 0 {
            HOLD_TAGS.with(|t| t.borrow_mut().clear());
        }
        Held {
            guard: Some(guard),
            label,
            detail: None,
            // Retain the public held_for duration even when aggregation is disabled.
            t0: Instant::now(),
            stats_on: on,
        }
    }

    /// The detail a long hold keeps (the register of a trapped access), computed only then.
    pub fn detail(&mut self, f: impl FnOnce() -> String + 'a) {
        if self.stats_on {
            self.detail = Some(Box::new(f));
        }
    }

    /// How long the lock has been held so far.
    pub fn held_for(&self) -> Duration {
        self.t0.elapsed()
    }
}

impl<T> Deref for Held<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.guard.as_ref().expect("held")
    }
}

impl<T> DerefMut for Held<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard.as_mut().expect("held")
    }
}

impl<T> Drop for Held<'_, T> {
    fn drop(&mut self) {
        if !self.stats_on {
            drop(self.guard.take());
            return;
        }
        let d = self.t0.elapsed();
        drop(self.guard.take());
        let outer = DEPTH.with(|n| n.replace(n.get().saturating_sub(1))) == 1;
        let tags = if outer && d >= DETAIL_AT {
            HOLD_TAGS.with(|t| std::mem::take(&mut *t.borrow_mut()))
        } else {
            Vec::new()
        };
        let detail = self.detail.take();
        record_with_stats(true, self.label, d, || {
            let mut s = detail.map_or_else(String::new, |f| f());
            if !tags.is_empty() {
                if !s.is_empty() {
                    s.push_str(": ");
                }
                s.push_str(&tags.join(", "));
            }
            s
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_default_on_and_only_explicit_zero_disables_them() {
        for value in [None, Some(""), Some("1"), Some("unexpected")] {
            assert!(stats_requested(value), "{value:?}");
        }
        assert!(!stats_requested(Some("0")));
        assert!(!stats_requested(Some(" 0 ")));
    }

    #[test]
    fn disabled_guards_still_lock_mutate_try_lock_and_unlock() {
        let m = Mutex::new(7u32);
        let depth = DEPTH.with(Cell::get);
        {
            let mut g = lock_with_stats(&m, "test: disabled hold", false);
            *g += 3;
            g.detail(|| unreachable!("disabled detail must not run"));
            tag_with_stats(false, || unreachable!("disabled tag must not run"));
            assert!(try_lock_with_stats(&m, "test: disabled busy", false).is_none());
            assert!(matches!(m.try_lock(), Err(std::sync::TryLockError::WouldBlock)));
            assert_eq!(DEPTH.with(Cell::get), depth);
            assert!(!g.stats_on);
        }
        {
            let mut g = try_lock_with_stats(&m, "test: disabled try", false).unwrap();
            assert_eq!(*g, 10);
            *g = 12;
        }
        assert_eq!(*m.lock().unwrap(), 12);
        assert_eq!(DEPTH.with(Cell::get), depth);
        assert!(STATS.lock().unwrap().rows.iter().all(|(l, _)| !l.starts_with("test: disabled")));
    }

    #[test]
    fn disabled_guards_preserve_poison_recovery() {
        let m = Mutex::new(0u32);
        assert!(std::panic::catch_unwind(|| {
            let mut g = lock_with_stats(&m, "test: disabled poison", false);
            *g = 4;
            panic!("poison the real mutex");
        }).is_err());
        assert!(m.is_poisoned());
        {
            let mut g = lock_with_stats(&m, "test: disabled recovery", false);
            assert_eq!(*g, 4);
            *g += 1;
        }
        let g = try_lock_with_stats(&m, "test: disabled poisoned try", false).unwrap();
        assert_eq!(*g, 5);
    }

    #[test]
    fn disabled_record_and_work_do_not_aggregate_or_build_details() {
        record_with_stats(false, "test: disabled record", Duration::from_secs(1), || {
            unreachable!("disabled record detail must not run")
        });
        work_with_stats(false, "test-disabled-work", "key".into(), Duration::from_secs(1));
        assert!(STATS.lock().unwrap().rows.iter().all(|(l, _)| *l != "test: disabled record"));
        let work = WORK.lock().unwrap();
        assert!(work.as_ref().is_none_or(|w| !w.contains_key(&("test-disabled-work", "key".into()))));
        assert_eq!(summary_with_stats(false), vec!["device lock statistics disabled (CARAXES_DLOCK_STATS=0)"]);
    }

    #[test]
    fn disabled_spend_has_no_timer_or_part_state() {
        take_spent();
        {
            let g = Spend::new_with_stats(Part::Copy, false);
            let nested = Spend::new_with_stats(Part::Copy, false);
            assert!(g.t0.is_none() && nested.t0.is_none());
            SPENT.with(|s| assert_eq!(s.borrow().1, [false; PARTS]));
        }
        assert_eq!(take_spent(), [Duration::ZERO; PARTS]);
        // A disabled nested scope must neither reserve nor clear an enabled timer's part.
        {
            let outer = Spend::new_with_stats(Part::Copy, true);
            {
                let disabled = Spend::new_with_stats(Part::Copy, false);
                assert!(disabled.t0.is_none());
            }
            SPENT.with(|s| assert!(s.borrow().1[Part::Copy as usize]));
            let nested = Spend::new_with_stats(Part::Copy, true);
            assert!(outer.t0.is_some() && nested.t0.is_none());
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(take_spent()[Part::Copy as usize] >= Duration::from_millis(1));
        SPENT.with(|s| assert_eq!(s.borrow().1, [false; PARTS]));
    }

    #[test]
    fn holds_are_recorded_by_label_with_the_longest_kept() {
        let m = Mutex::new(0u32);
        {
            let mut g = lock_with_stats(&m, "test: short", true);
            *g += 1;
        }
        {
            let mut g = lock_with_stats(&m, "test: long", true);
            g.detail(|| "bar0 W 0xb830b0".into());
            std::thread::sleep(Duration::from_millis(6));
        }
        assert_eq!(*m.lock().unwrap(), 1);
        let s = summary_with_stats(true);
        assert!(s[0].contains("test: long 1 hold(s)"), "{s:?}");
        assert!(s[0].contains("test: short 1 hold(s)"), "{s:?}");
        let longest = s.iter().find(|l| l.starts_with("device lock: longest")).unwrap();
        assert!(longest.contains("test: long (bar0 W 0xb830b0)"), "{s:?}");
        assert_eq!(bucket(Duration::from_micros(3)), 2);
        assert_eq!(bucket(Duration::ZERO), 0);
        let mut r = Row::default();
        for us in [1u64, 2, 3, 900] {
            r.count += 1;
            r.buckets[bucket(Duration::from_micros(us))] += 1;
            r.max_ns = r.max_ns.max(us * 1000);
        }
        assert_eq!(quantile(&r, 0.5), Duration::from_micros(4), "2 and 3 us are in [2, 4)");
        assert_eq!(quantile(&r, 0.99), Duration::from_micros(1024));
    }

    /// A part's time is summed per thread until taken, and a nested guard of the same part
    /// does not count twice.
    #[test]
    fn a_doorbell_s_parts_are_summed_once_and_taken() {
        take_spent();
        // Once, not twice: the nested part's time is at most the block's own wall time (a
        // double count would be about twice it). Measured against the block, not against a
        // fixed bound, so a thread descheduled under parallel load does not fail the test.
        let t = std::time::Instant::now();
        {
            let _outer = Spend::new_with_stats(Part::Copy, true);
            let _inner = Spend::new_with_stats(Part::Copy, true);
            std::thread::sleep(Duration::from_millis(3));
        }
        let block = t.elapsed();
        {
            let _t = Spend::new_with_stats(Part::Translate, true);
            std::thread::sleep(Duration::from_millis(1));
        }
        let [translate, copy, ..] = take_spent();
        assert!(copy >= Duration::from_millis(3) && copy <= block, "{copy:?} in a block of {block:?}");
        assert!(translate >= Duration::from_millis(1), "{translate:?}");
        assert_eq!(take_spent(), [Duration::ZERO; PARTS]);
    }

    /// A long hold keeps what the work under it tagged; outside a hold a tag is dropped; the
    /// work table sums by kind and key.
    #[test]
    fn a_long_hold_keeps_its_tags_and_work_is_summed_by_key() {
        tag_with_stats(true, || unreachable!("no hold on this thread"));
        let m = Mutex::new(0u32);
        {
            let _g = lock_with_stats(&m, "test: tagged", true);
            tag_with_stats(true, || "rpc GSP_RM_CONTROL 0x90f10106".into());
            tag_with_stats(true, || "build chid 4".into());
            std::thread::sleep(Duration::from_millis(6));
        }
        work_with_stats(true, "test-rpc", "GSP_RM_ALLOC class 0xc46f".into(), Duration::from_millis(3));
        work_with_stats(true, "test-rpc", "GSP_RM_ALLOC class 0xc46f".into(), Duration::from_millis(5));
        let s = summary_with_stats(true);
        let longest = s.iter().find(|l| l.starts_with("device lock: longest")).unwrap();
        assert!(
            longest.contains("test: tagged (rpc GSP_RM_CONTROL 0x90f10106, build chid 4)"),
            "{longest}"
        );
        let work = s.iter().find(|l| l.starts_with("device lock work")).unwrap();
        assert!(work.contains("test-rpc GSP_RM_ALLOC class 0xc46f 2 x 8 ms (max 5000 us)"), "{work}");
    }
}

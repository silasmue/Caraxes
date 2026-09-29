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

use std::cell::{Cell, RefCell};
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

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
}

const PARTS: usize = 2;

impl Part {
    fn name(self) -> &'static str {
        match self {
            Part::Translate => "translate",
            Part::Copy => "CPU copy",
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
        work("software", self.part.name().to_string(), d);
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
}

/// Take `m` for `label`.
pub fn lock<'a, T>(m: &'a Mutex<T>, label: &'static str) -> Held<'a, T> {
    let guard = m.lock().unwrap_or_else(PoisonError::into_inner);
    if DEPTH.with(|d| d.replace(d.get() + 1)) == 0 {
        HOLD_TAGS.with(|t| t.borrow_mut().clear());
    }
    Held {
        guard: Some(guard),
        label,
        detail: None,
        t0: Instant::now(),
    }
}

impl<'a, T> Held<'a, T> {
    /// The detail a long hold keeps (the register of a trapped access), computed only then.
    pub fn detail(&mut self, f: impl FnOnce() -> String + 'a) {
        self.detail = Some(Box::new(f));
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
        let d = self.t0.elapsed();
        drop(self.guard.take());
        let outer = DEPTH.with(|n| n.replace(n.get().saturating_sub(1))) == 1;
        let tags = if outer && d >= DETAIL_AT {
            HOLD_TAGS.with(|t| std::mem::take(&mut *t.borrow_mut()))
        } else {
            Vec::new()
        };
        let detail = self.detail.take();
        record(self.label, d, || {
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
    fn holds_are_recorded_by_label_with_the_longest_kept() {
        let m = Mutex::new(0u32);
        {
            let mut g = lock(&m, "test: short");
            *g += 1;
        }
        {
            let mut g = lock(&m, "test: long");
            g.detail(|| "bar0 W 0xb830b0".into());
            std::thread::sleep(Duration::from_millis(6));
        }
        assert_eq!(*m.lock().unwrap(), 1);
        let s = summary();
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
        {
            let _outer = Spend::new(Part::Copy);
            let _inner = Spend::new(Part::Copy);
            std::thread::sleep(Duration::from_millis(3));
        }
        {
            let _t = Spend::new(Part::Translate);
            std::thread::sleep(Duration::from_millis(1));
        }
        let [translate, copy] = take_spent();
        assert!(copy >= Duration::from_millis(3) && copy < Duration::from_millis(6), "{copy:?}");
        assert!(translate >= Duration::from_millis(1), "{translate:?}");
        assert_eq!(take_spent(), [Duration::ZERO; 2]);
    }

    /// A long hold keeps what the work under it tagged; outside a hold a tag is dropped; the
    /// work table sums by kind and key.
    #[test]
    fn a_long_hold_keeps_its_tags_and_work_is_summed_by_key() {
        tag(|| unreachable!("no hold on this thread"));
        let m = Mutex::new(0u32);
        {
            let _g = lock(&m, "test: tagged");
            tag(|| "rpc GSP_RM_CONTROL 0x90f10106".into());
            tag(|| "build chid 4".into());
            std::thread::sleep(Duration::from_millis(6));
        }
        work("test-rpc", "GSP_RM_ALLOC class 0xc46f".into(), Duration::from_millis(3));
        work("test-rpc", "GSP_RM_ALLOC class 0xc46f".into(), Duration::from_millis(5));
        let s = summary();
        let longest = s.iter().find(|l| l.starts_with("device lock: longest")).unwrap();
        assert!(
            longest.contains("test: tagged (rpc GSP_RM_CONTROL 0x90f10106, build chid 4)"),
            "{longest}"
        );
        let work = s.iter().find(|l| l.starts_with("device lock work")).unwrap();
        assert!(work.contains("test-rpc GSP_RM_ALLOC class 0xc46f 2 x 8 ms (max 5000 us)"), "{work}");
    }
}

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

use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Log2 microsecond buckets: bucket k holds holds in [2^(k-1), 2^k) us (bucket 0: < 1 us).
const BUCKETS: usize = 32;
/// Longest holds kept.
const LONGEST: usize = 8;
/// Holds at least this long keep their detail.
const DETAIL_AT: Duration = Duration::from_millis(5);

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
        let detail = self.detail.take();
        record(self.label, d, || detail.map_or_else(String::new, |f| f()));
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
        assert!(s[1].contains("test: long (bar0 W 0xb830b0)"), "{s:?}");
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
}

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Held rings (docs/caraxes/execution.md §9.1): a doorbell of a shadow channel whose root
//! still has jobs in the shadow worker no longer waits for them under the device lock. The
//! doorbell records a held ring and returns; the card only promises that the GPU fetches the
//! new entries eventually. The ring is forwarded once the root's jobs up to the doorbell are
//! applied and no hold names the root.
//!
//! The [`RingGate`] is shared by the device (under the device lock: it holds and completes
//! rings), the shadow worker (it pokes the gate after every job and releases holds it was
//! given) and the completer thread (it waits for a poke while rings are held, then takes the
//! device lock briefly to complete the ready ones). The gate never takes the device lock
//! itself, so the order of execution.md §9 holds: the worker and the gate's users that hold
//! the gate's mutex never wait for D.
//!
//! Holds (`hold`, `release`): the BAR1 pool (bar1-pool.md) holds the rings of roots while
//! it moves pages and has the worker release the hold after a host call it runs in order
//! with the plans (a preempt), so no ring passes that call.

use std::collections::BTreeMap;
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;

#[derive(Default, Debug)]
struct State {
    holds: BTreeMap<u64, Vec<u64>>,
    next_token: u64,
    /// Rings held in the device now (the rings themselves are the mirror's).
    waiting: usize,
    /// Moves at every event that can make a held ring ready: a job finished, a hold released.
    generation: u64,
    stop: bool,
}

/// See the module docs.
#[derive(Default, Debug)]
pub struct RingGate {
    st: Mutex<State>,
    cv: Condvar,
}

impl RingGate {
    fn st(&self) -> std::sync::MutexGuard<'_, State> {
        self.st.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hold the rings of `roots` until [`Self::release`] of the returned token.
    pub fn hold(&self, roots: Vec<u64>) -> u64 {
        let mut st = self.st();
        st.next_token += 1;
        let t = st.next_token;
        st.holds.insert(t, roots);
        t
    }

    /// Release hold `token` (unknown tokens are ignored).
    pub fn release(&self, token: u64) {
        let mut st = self.st();
        if st.holds.remove(&token).is_some() {
            st.generation += 1;
            self.cv.notify_all();
        }
    }

    /// Whether a hold names `root`.
    pub fn held(&self, root: u64) -> bool {
        self.st().holds.values().any(|rs| rs.contains(&root))
    }

    /// Something that can make a held ring ready happened (the worker finished a job).
    pub fn poke(&self) {
        let mut st = self.st();
        st.generation += 1;
        if st.waiting > 0 {
            self.cv.notify_all();
        }
    }

    /// The number of rings held in the device now.
    pub fn set_waiting(&self, n: usize) {
        let mut st = self.st();
        st.waiting = n;
        if n > 0 {
            self.cv.notify_all();
        }
    }

    pub fn waiting(&self) -> usize {
        self.st().waiting
    }

    /// The completer's wait: until rings are held and the generation moved past `seen` (or
    /// at most `max`, a safety tick). Returns the generation to pass next time, `None` once
    /// stopped.
    pub fn wait_ready(&self, seen: u64, max: Duration) -> Option<u64> {
        let mut st = self.st();
        let end = std::time::Instant::now() + max;
        loop {
            if st.stop {
                return None;
            }
            if st.waiting > 0 && st.generation != seen {
                return Some(st.generation);
            }
            let now = std::time::Instant::now();
            if now >= end {
                return Some(st.generation);
            }
            st = self
                .cv
                .wait_timeout(st, end - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// End the completer (process exit).
    pub fn stop(&self) {
        self.st().stop = true;
        self.cv.notify_all();
    }
}

/// The process's gate: one device per process, and a model rebuilt at a function-level
/// reset keeps it, so the completer thread started once waits on the right one.
pub fn global() -> std::sync::Arc<RingGate> {
    static G: std::sync::OnceLock<std::sync::Arc<RingGate>> = std::sync::OnceLock::new();
    G.get_or_init(|| std::sync::Arc::new(RingGate::default())).clone()
}

/// A ring the device holds: the shadow root the channel translates through (`None`: a
/// mirror without a shadow, which waits for every job), the worker target that must be
/// applied first, since when, and how many doorbells it covers.
#[derive(Clone, Debug)]
pub struct HeldRing {
    pub root: Option<u64>,
    pub target: crate::invgate::Target,
    pub since: std::time::Instant,
    pub doorbells: u64,
}

/// Whether a held ring may be forwarded now: its target is applied and no hold names its
/// root.
pub fn ready(ring: &HeldRing, progress: &crate::invgate::Progress, gate: &RingGate) -> bool {
    progress.is_done(&ring.target) && ring.root.is_none_or(|r| !gate.held(r))
}

/// Counters for the exit summary.
#[derive(Clone, Debug, Default)]
pub struct RingStats {
    /// Doorbells that found their root's jobs applied and rang at once.
    pub direct: u64,
    /// Doorbells held, those that joined a ring already held, and the rings completed.
    pub held: u64,
    pub coalesced: u64,
    pub completed: u64,
    /// Held rings dropped (their channel freed, a reset).
    pub dropped: u64,
    pub held_ns: u64,
    pub held_max_ns: u64,
}

impl RingStats {
    pub fn line(&self, now_held: usize) -> String {
        format!(
            "held rings (execution.md §9.1): {} doorbell(s) rang at once, {} held ({} joined a held ring), {} completed by the completer ({} ms held in total, longest {} us), {} dropped, {} held now",
            self.direct,
            self.held,
            self.coalesced,
            self.completed,
            self.held_ns / 1_000_000,
            self.held_max_ns / 1000,
            self.dropped,
            now_held
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invgate::{Progress, Target};
    use std::sync::Arc;
    use std::time::Instant;

    fn ring(root: u64, seq: u64) -> HeldRing {
        HeldRing {
            root: Some(root),
            target: Target::Roots(vec![(root, seq)]),
            since: Instant::now(),
            doorbells: 1,
        }
    }

    /// A ring is ready only when its root's jobs up to it are applied and no hold names the
    /// root; another root's job or hold does not matter.
    #[test]
    fn a_ring_waits_for_its_roots_jobs_and_holds_only() {
        let (p, g) = (Progress::default(), RingGate::default());
        p.begin(1, 0xA000);
        p.begin(2, 0xB000);
        let r = ring(0xA000, 1);
        assert!(!ready(&r, &p, &g));
        p.finish(1);
        assert!(ready(&r, &p, &g), "B's job in flight does not hold A's ring");
        let t = g.hold(vec![0xA000]);
        let other = g.hold(vec![0xB000]);
        assert!(!ready(&r, &p, &g), "a hold on A holds it");
        g.release(t);
        assert!(ready(&r, &p, &g));
        g.release(other);
        let all = HeldRing {
            root: None,
            target: Target::All(2),
            since: Instant::now(),
            doorbells: 1,
        };
        assert!(!ready(&all, &p, &g), "a mirror without a shadow waits for every job");
        p.finish(2);
        assert!(ready(&all, &p, &g));
    }

    /// The completer's wait: it returns when rings are held and something moved, at the
    /// safety tick otherwise, and not at all once stopped.
    #[test]
    fn the_completer_wakes_on_a_poke_while_rings_are_held() {
        let g = Arc::new(RingGate::default());
        let seen = g.wait_ready(u64::MAX, Duration::from_millis(1)).unwrap();
        g.set_waiting(1);
        let g2 = g.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            g2.poke();
        });
        let t0 = Instant::now();
        let next = g.wait_ready(seen, Duration::from_secs(5)).unwrap();
        assert!(next != seen);
        assert!(t0.elapsed() < Duration::from_secs(1));
        t.join().unwrap();
        g.stop();
        assert_eq!(g.wait_ready(next, Duration::from_secs(5)), None);
    }
}

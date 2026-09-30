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
//!
//! Posted doorbells (execution.md §9.9): the vfio-user loop queues a trapped write of the
//! usermode doorbell here without the device lock ([`RingGate::post`]) and answers the
//! write at once; the completer rings the queue under the lock, and every other trapped
//! access rings it first ([`RingGate::take_posted`]), so no access passes a doorbell the
//! guest wrote before it.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

#[derive(Default, Debug)]
struct State {
    holds: BTreeMap<u64, Vec<u64>>,
    next_token: u64,
    /// Rings held in the device now (the rings themselves are the mirror's).
    waiting: usize,
    /// Moves at every event that can make a held ring ready (a job finished, a hold
    /// released) and at every posted doorbell.
    generation: u64,
    /// Doorbell writes posted and not rung yet, in the guest's order, one per token.
    posted: Vec<Posted>,
    post: PostStats,
    /// Channels the USERD poller found with entries the device has not forwarded
    /// (execution.md §9.11), one entry per channel; and how many were posted and joined.
    userd_rings: Vec<u32>,
    userd_posted: u64,
    userd_joined: u64,
    stop: bool,
}

/// See the module docs.
#[derive(Default, Debug)]
pub struct RingGate {
    st: Mutex<State>,
    cv: Condvar,
    /// `posted.len()`, for the check every trapped access makes without the gate's mutex.
    posted_n: AtomicUsize,
    /// `userd_rings.len()`, for the completer's check.
    userd_n: AtomicUsize,
}

/// A trapped doorbell write the vfio-user loop posted: the value written and when.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Posted {
    pub token: u32,
    pub at: Instant,
}

/// Counters of the posted doorbells for the exit summary.
#[derive(Clone, Copy, Debug, Default)]
pub struct PostStats {
    /// Trapped doorbell writes posted, and those that joined a queued write of the same
    /// token (one ring covers both: a doorbell only makes the engine look at GPPut).
    pub writes: u64,
    pub coalesced: u64,
    /// Queued writes rung by the completer, and by a later guest access (trapped, an
    /// eventfd, a shared page's reactor) before its own work.
    pub by_completer: u64,
    pub by_access: u64,
    /// Time from the post to the start of its ring, in total and the longest.
    pub queued_ns: u64,
    pub queued_max_ns: u64,
    pub max_depth: usize,
}

impl PostStats {
    pub fn line(&self) -> String {
        let rung = self.by_completer + self.by_access;
        format!(
            "posted doorbells (execution.md §9.9): {} trapped write(s) answered without the device lock, {} joined a queued write of the same token; {} rung by the completer, {} by a later guest access first (trapped, an eventfd or a page); queued {} us on average, longest {} us; deepest queue {}",
            self.writes,
            self.coalesced,
            self.by_completer,
            self.by_access,
            self.queued_ns / rung.max(1) / 1000,
            self.queued_max_ns / 1000,
            self.max_depth
        )
    }
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

    /// Drop every hold (a device reset: the BAR1 pool's batch, whose barrier held them, is
    /// gone with the model).
    pub fn clear_holds(&self) {
        let mut st = self.st();
        if !st.holds.is_empty() {
            st.holds.clear();
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

    /// Post a trapped doorbell write of `token` at `at`. Returns false when a write of the
    /// same token is queued already (its ring covers this one).
    pub fn post(&self, token: u32, at: Instant) -> bool {
        let mut st = self.st();
        st.post.writes += 1;
        if st.posted.iter().any(|p| p.token == token) {
            st.post.coalesced += 1;
            return false;
        }
        st.posted.push(Posted { token, at });
        st.post.max_depth = st.post.max_depth.max(st.posted.len());
        self.posted_n.store(st.posted.len(), Ordering::Release);
        st.generation += 1;
        self.cv.notify_all();
        true
    }

    /// The USERD poller found channel `chid` with entries not yet forwarded: the completer
    /// rings it under the device lock (a channel already queued joins its entry).
    pub fn post_userd(&self, chid: u32) {
        let mut st = self.st();
        st.userd_posted += 1;
        if st.userd_rings.contains(&chid) {
            st.userd_joined += 1;
            return;
        }
        st.userd_rings.push(chid);
        self.userd_n.store(st.userd_rings.len(), Ordering::Release);
        st.generation += 1;
        self.cv.notify_all();
    }

    pub fn has_userd(&self) -> bool {
        self.userd_n.load(Ordering::Acquire) > 0
    }

    pub fn take_userd(&self) -> Vec<u32> {
        let mut st = self.st();
        self.userd_n.store(0, Ordering::Release);
        std::mem::take(&mut st.userd_rings)
    }

    /// The USERD poller's posts and those that joined a queued one.
    pub fn userd_stats(&self) -> (u64, u64) {
        let st = self.st();
        (st.userd_posted, st.userd_joined)
    }

    /// Whether doorbell writes are posted and not rung yet (no mutex).
    pub fn has_posted(&self) -> bool {
        self.posted_n.load(Ordering::Acquire) > 0
    }

    /// Take the posted doorbell writes to ring them, in the guest's order.
    pub fn take_posted(&self) -> Vec<Posted> {
        let mut st = self.st();
        self.posted_n.store(0, Ordering::Release);
        std::mem::take(&mut st.posted)
    }

    /// `rings` taken with [`Self::take_posted`] were rung, the first starting at `start`,
    /// by a trapped access before its own work (`by_access`) or by the completer.
    pub fn note_rung(&self, rings: &[Posted], start: Instant, by_access: bool) {
        let mut st = self.st();
        let n = rings.len() as u64;
        if by_access {
            st.post.by_access += n;
        } else {
            st.post.by_completer += n;
        }
        for p in rings {
            let ns = start.saturating_duration_since(p.at).as_nanos() as u64;
            st.post.queued_ns += ns;
            st.post.queued_max_ns = st.post.queued_max_ns.max(ns);
        }
    }

    pub fn post_stats(&self) -> PostStats {
        self.st().post
    }

    /// The completer's wait: until rings are held or doorbells posted and the generation
    /// moved past `seen` (or at most `max`, a safety tick). Returns the generation to pass
    /// next time, `None` once stopped.
    pub fn wait_ready(&self, seen: u64, max: Duration) -> Option<u64> {
        let mut st = self.st();
        let end = std::time::Instant::now() + max;
        loop {
            if st.stop {
                return None;
            }
            if (st.waiting > 0 || !st.posted.is_empty() || !st.userd_rings.is_empty())
                && st.generation != seen
            {
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
    global_ref().clone()
}

/// [`global`] without a reference count (the posted-doorbell check of every guest access).
pub fn global_ref() -> &'static std::sync::Arc<RingGate> {
    static G: std::sync::OnceLock<std::sync::Arc<RingGate>> = std::sync::OnceLock::new();
    G.get_or_init(|| std::sync::Arc::new(RingGate::default()))
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
        // A reset drops the holds the BAR1 pool left.
        g.hold(vec![0xA000]);
        assert!(!ready(&r, &p, &g));
        g.clear_holds();
        assert!(ready(&r, &p, &g));
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

    /// A posted doorbell wakes the completer without a held ring; a second write of a queued
    /// token joins it; the queue comes back in the guest's order and empties.
    #[test]
    fn a_posted_doorbell_wakes_the_completer_and_keeps_the_order() {
        let g = Arc::new(RingGate::default());
        let seen = g.wait_ready(u64::MAX, Duration::from_millis(1)).unwrap();
        assert!(!g.has_posted());
        let g2 = g.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            let at = Instant::now();
            assert!(g2.post(0x10009, at));
            assert!(g2.post(0x1000a, at));
            assert!(!g2.post(0x10009, at), "joins the queued write");
        });
        let t0 = Instant::now();
        let next = g.wait_ready(seen, Duration::from_secs(5)).unwrap();
        assert!(next != seen);
        assert!(t0.elapsed() < Duration::from_secs(1));
        t.join().unwrap();
        assert!(g.has_posted());
        let rings = g.take_posted();
        assert_eq!(rings.iter().map(|p| p.token).collect::<Vec<_>>(), [0x10009, 0x1000a]);
        assert!(!g.has_posted());
        assert!(g.take_posted().is_empty());
        g.note_rung(&rings, Instant::now(), false);
        let s = g.post_stats();
        assert_eq!((s.writes, s.coalesced, s.by_completer, s.by_access, s.max_depth), (3, 1, 2, 0, 2));
        // After the take, a new write of the same token queues again.
        assert!(g.post(0x10009, Instant::now()));
    }
}

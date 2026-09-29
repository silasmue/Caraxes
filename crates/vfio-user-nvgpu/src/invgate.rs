// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The held invalidate without the device lock (docs/caraxes/execution.md §8).
//!
//! The untrapped-GPPut rule (untrapped.rs, docs/caraxes/bar1-pool.md §9) makes a guest MMU
//! invalidate of a root whose channel has an untrapped USERD wait until the shadow worker
//! applied its plan. That wait ran inside the trapped trigger write, under the Device mutex:
//! 78-115 holds per second while Sunshine streams, 17-22 % of wall time, single holds up to
//! 130 ms, and the display clock, the reactor and every other trapped access queued behind
//! it (caraxes/runs/nvenc-2026-09-25/vm4-615-latency/NOTES.md).
//!
//! Hardware completes an invalidate asynchronously: the trigger bit (TRIGGER, bit 31 of
//! `NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE`) reads TRUE until the invalidate is done, and
//! the guest RM polls it before it returns (`kgmmuCommitTlbInvalidate_TU102` →
//! `kgmmuCheckPendingInvalidates_TU102`, 610 and 615 alike). The device now does the same:
//! the trigger write plans and submits, arms an [`InvalidateGate`] at the worker's sequence
//! number, and returns; a read of the trigger reports busy until the worker's [`Progress`]
//! reached it. Each busy read waits at most [`POLL_WAIT`] under the lock; one that finds the
//! gate armed for longer than [`FALLBACK`] waits for the worker to the end (the old hold),
//! so the guest's 4 s poll timeout can never let it continue before the apply.
//!
//! [`Seqs`] keeps the last job per shadow root, so a doorbell (or a trapped GPPut store)
//! waits only for its own root's plans instead of for every job in the worker.
//!
//! `CARAXES_SHADOW_INVALIDATE_ASYNC=0` restores the hold under the lock (A/B only).

use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The switch (default on).
pub const ENV: &str = "CARAXES_SHADOW_INVALIDATE_ASYNC";
/// A busy read of the trigger waits for the worker at most this long under the lock, so the
/// guest's poll does not become a storm of exits while other waiters still get the lock.
pub const POLL_WAIT: Duration = Duration::from_micros(50);
/// A gate armed longer than this makes the next trigger read wait for the worker to the end
/// (fail closed well inside the guest RM's 4 s `GPU_TIMEOUT_DEFAULT`).
pub const FALLBACK: Duration = Duration::from_secs(1);

/// `CARAXES_SHADOW_INVALIDATE_ASYNC`: unset, empty, `1`/`on` on; `0`/`off` off; anything
/// else an error (the caller then keeps the old hold: fail closed).
pub fn parse_switch(v: Option<&str>) -> Result<bool, String> {
    match v.map(str::trim) {
        None | Some("") | Some("1") | Some("on") => Ok(true),
        Some("0") | Some("off") => Ok(false),
        Some(x) => Err(format!("{ENV}={x:?}: expected 1, on, 0 or off")),
    }
}

/// What a waiter waits for: every job up to a sequence number, or the jobs of one shadow
/// root up to a sequence number (that root's last job when the wait began).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    All(u64),
    Roots(Vec<(u64, u64)>),
}

/// The jobs in flight (submitted, not finished), by sequence number and by shadow root,
/// and the roots someone waits for.
#[derive(Default, Debug)]
struct Inflight {
    jobs: BTreeMap<u64, u64>,
    by_root: BTreeMap<u64, std::collections::BTreeSet<u64>>,
    /// Roots with a waiter (a doorbell barrier, a held invalidate's gate), and how many:
    /// the worker applies their jobs before other roots' (`ShadowWorker`'s pick).
    urgent: BTreeMap<u64, usize>,
    /// The highest sequence number begun.
    submitted: u64,
}

impl Inflight {
    fn done(&self, t: &Target) -> bool {
        match t {
            Target::All(seq) => self.jobs.range(..=seq).next().is_none(),
            Target::Roots(rs) => rs.iter().all(|(root, seq)| {
                self.by_root
                    .get(root)
                    .is_none_or(|s| s.range(..=seq).next().is_none())
            }),
        }
    }

    fn roots(t: &Target) -> Vec<u64> {
        match t {
            Target::All(_) => Vec::new(),
            Target::Roots(rs) => rs.iter().map(|(r, _)| *r).collect(),
        }
    }
}

/// The worker's side and the waiters' (docs/caraxes/execution.md §8): which jobs are in
/// flight. A job is begun when it is submitted (the server thread) and finished when the
/// worker applied it; the worker may apply another root's job before an older one (an
/// urgent root first), so "done" is per job, not a single counter.
#[derive(Default)]
pub struct Progress {
    st: Mutex<Inflight>,
    cv: Condvar,
}

impl Progress {
    /// Job `seq` of shadow root `root` was submitted.
    pub fn begin(&self, seq: u64, root: u64) {
        let mut st = self.st.lock().unwrap();
        st.jobs.insert(seq, root);
        st.by_root.entry(root).or_default().insert(seq);
        st.submitted = st.submitted.max(seq);
    }

    /// The worker finished job `seq`.
    pub fn finish(&self, seq: u64) {
        let mut st = self.st.lock().unwrap();
        if let Some(root) = st.jobs.remove(&seq) {
            if let Some(s) = st.by_root.get_mut(&root) {
                s.remove(&seq);
                if s.is_empty() {
                    st.by_root.remove(&root);
                }
            }
        }
        self.cv.notify_all();
    }

    /// Every job below this sequence number is finished (the oldest job in flight, less
    /// one; the last one begun when none is in flight).
    pub fn done(&self) -> u64 {
        let st = self.st.lock().unwrap();
        match st.jobs.keys().next() {
            Some(first) => first - 1,
            None => st.submitted,
        }
    }

    /// No job in flight.
    pub fn idle(&self) -> bool {
        self.st.lock().unwrap().jobs.is_empty()
    }

    /// Whether what `t` names is finished.
    pub fn is_done(&self, t: &Target) -> bool {
        self.st.lock().unwrap().done(t)
    }

    /// Block until job `seq` and every job before it is finished.
    pub fn wait_for(&self, seq: u64) {
        self.wait(&Target::All(seq));
    }

    /// Block until what `t` names is finished; its roots are urgent meanwhile.
    pub fn wait(&self, t: &Target) {
        let mut st = self.st.lock().unwrap();
        if st.done(t) {
            return;
        }
        let roots = Inflight::roots(t);
        for r in &roots {
            *st.urgent.entry(*r).or_default() += 1;
        }
        while !st.done(t) {
            st = self.cv.wait(st).unwrap();
        }
        for r in &roots {
            unmark(&mut st, *r);
        }
    }

    /// Block until job `seq` (and every earlier one) is finished or `max` passed.
    pub fn wait_for_at_most(&self, seq: u64, max: Duration) -> bool {
        self.wait_at_most(&Target::All(seq), max)
    }

    /// Block until what `t` names is finished or `max` passed; whether it is finished.
    pub fn wait_at_most(&self, t: &Target, max: Duration) -> bool {
        let end = Instant::now() + max;
        let mut st = self.st.lock().unwrap();
        while !st.done(t) {
            let now = Instant::now();
            if now >= end {
                return false;
            }
            st = self.cv.wait_timeout(st, end - now).unwrap().0;
        }
        true
    }

    /// Root `root` has a waiter that does not block here (a held invalidate's gate, which
    /// the guest polls): its jobs go first until [`Self::unmark_urgent`].
    pub fn mark_urgent(&self, root: u64) {
        *self.st.lock().unwrap().urgent.entry(root).or_default() += 1;
    }

    pub fn unmark_urgent(&self, root: u64) {
        unmark(&mut self.st.lock().unwrap(), root);
    }

    /// The roots with a waiter now.
    pub fn urgent_roots(&self) -> Vec<u64> {
        self.st.lock().unwrap().urgent.keys().copied().collect()
    }
}

fn unmark(st: &mut Inflight, root: u64) {
    if let Some(n) = st.urgent.get_mut(&root) {
        *n -= 1;
        if *n == 0 {
            st.urgent.remove(&root);
        }
    }
}

/// The server thread's side: sequence numbers of submitted jobs, and the last one per
/// shadow root.
#[derive(Default, Debug)]
pub struct Seqs {
    submitted: u64,
    last: BTreeMap<u64, u64>,
}

impl Seqs {
    /// The number of a new job for shadow root `root`.
    pub fn next(&mut self, root: u64) -> u64 {
        self.submitted += 1;
        self.last.insert(root, self.submitted);
        self.submitted
    }

    pub fn submitted(&self) -> u64 {
        self.submitted
    }

    /// The last job submitted for `root` (0: none).
    pub fn last_of(&self, root: u64) -> u64 {
        self.last.get(&root).copied().unwrap_or(0)
    }

    /// Drop the entries the worker has finished (they can no longer make anyone wait).
    pub fn forget_done(&mut self, done: u64) {
        self.last.retain(|_, s| *s > done);
    }
}

/// What a read of the trigger reports.
#[derive(Debug, PartialEq, Eq)]
pub enum Trigger {
    /// The worker has not applied the invalidate's plans yet: TRIGGER reads TRUE.
    Busy,
    /// Applied: TRIGGER reads FALSE. `Some(d)` for the first read that saw it (the gate
    /// was armed `d` before), `None` when no gate was armed.
    Done(Option<Duration>),
}

/// A held invalidate off the device lock ([`InvalidateGate::hand_off`]): TRIGGER stays busy
/// until the worker applied the jobs of its roots, which [`Hold::wait`] waits for without the
/// lock (docs/caraxes/execution.md §9.8).
pub struct Hold {
    progress: Arc<Progress>,
    targets: Vec<(u64, u64)>,
    since: Instant,
}

impl Hold {
    /// A hold of `targets` (root, its last job), armed at `since`.
    pub fn new(progress: Arc<Progress>, targets: Vec<(u64, u64)>, since: Instant) -> Self {
        for (r, _) in &targets {
            progress.mark_urgent(*r);
        }
        Hold {
            progress,
            targets,
            since,
        }
    }

    pub fn is_done(&self) -> bool {
        self.progress.is_done(&Target::Roots(self.targets.clone()))
    }

    /// When the invalidate was armed (a hold's identity for fastread.rs).
    pub fn since(&self) -> Instant {
        self.since
    }

    /// The worker's progress, the targets and when it was armed, for a wait without the
    /// hold (fastread.rs: a read waits without the lock that keeps the holds).
    pub fn parts(&self) -> (Arc<Progress>, Vec<(u64, u64)>, Instant) {
        (self.progress.clone(), self.targets.clone(), self.since)
    }

    /// Wait until the worker applied the targets' jobs; the roots stop being urgent. Returns
    /// the time since the invalidate was armed.
    pub fn wait(self) -> Duration {
        self.progress.wait(&Target::Roots(self.targets.clone()));
        for (r, _) in &self.targets {
            self.progress.unmark_urgent(*r);
        }
        self.since.elapsed()
    }
}

/// The invalidate in flight: TRIGGER reads busy until the worker applied the jobs of the
/// roots it names (the invalidated scope, not every root: the card's invalidate completes
/// when the TLB holds no stale translation for its target).
#[derive(Default, Debug)]
pub struct InvalidateGate {
    pending: Option<(Vec<(u64, u64)>, Instant)>,
    /// Trigger reads that reported busy.
    pub busy_reads: u64,
    /// Reads that fell back to waiting for the worker to the end ([`FALLBACK`]).
    pub fallbacks: u64,
}

impl InvalidateGate {
    /// A held invalidate submitted the jobs `targets` (root, its last job) at `now`; its
    /// roots are urgent for the worker until the gate opens. A second one while the first
    /// is pending extends the gate (the guest polls before it writes the next, so this is
    /// the defensive case).
    pub fn arm(&mut self, targets: Vec<(u64, u64)>, now: Instant, progress: &Progress) {
        for (r, _) in &targets {
            progress.mark_urgent(*r);
        }
        self.pending = Some(match self.pending.take() {
            Some((mut t, since)) => {
                t.extend(targets);
                (t, since)
            }
            None => (targets, now),
        });
    }

    pub fn armed(&self) -> bool {
        self.pending.is_some()
    }

    /// Hand the armed invalidate off the device lock (the invalidate page, inv_page.rs): its
    /// targets and the worker's progress go to whoever completes TRIGGER; the gate is empty
    /// again. `None` when nothing is held.
    pub fn hand_off(&mut self, progress: &Arc<Progress>) -> Option<Hold> {
        let (targets, since) = self.pending.take()?;
        Some(Hold {
            progress: progress.clone(),
            targets,
            since,
        })
    }

    /// A read of the trigger: wait at most [`POLL_WAIT`] (to the end past [`FALLBACK`]) for
    /// the worker, then report.
    pub fn read(&mut self, progress: &Progress) -> Trigger {
        let Some((targets, since)) = self.pending.as_ref() else {
            return Trigger::Done(None);
        };
        let t = Target::Roots(targets.clone());
        let done = if since.elapsed() >= FALLBACK {
            self.fallbacks += 1;
            progress.wait(&t);
            true
        } else {
            progress.wait_at_most(&t, POLL_WAIT)
        };
        if done {
            let (targets, since) = self.pending.take().expect("checked");
            for (r, _) in targets {
                progress.unmark_urgent(r);
            }
            Trigger::Done(Some(since.elapsed()))
        } else {
            self.busy_reads += 1;
            Trigger::Busy
        }
    }

    /// A device reset: the worker was drained, nothing is in flight.
    pub fn clear(&mut self, progress: &Progress) {
        if let Some((targets, _)) = self.pending.take() {
            for (r, _) in targets {
                progress.unmark_urgent(r);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn the_switch_is_on_unless_turned_off() {
        assert_eq!(parse_switch(None), Ok(true));
        assert_eq!(parse_switch(Some("on")), Ok(true));
        assert_eq!(parse_switch(Some("0")), Ok(false));
        assert!(parse_switch(Some("x")).is_err());
    }

    /// A stand-in for the shadow worker: finishes job `seq` after `delay`.
    fn slow_worker(p: &Arc<Progress>, seq: u64, delay: Duration) -> std::thread::JoinHandle<()> {
        let p = p.clone();
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            p.finish(seq);
        })
    }

    /// The device's shape: one mutex (the Device) around the gate and the submission side;
    /// the worker outside it. An invalidate of root A whose walk takes 300 ms returns at
    /// once; a trapped access (here: the display tick) takes the lock while the walk runs;
    /// a doorbell of root B does not wait; the trigger reads busy and a doorbell of root A
    /// waits until the walk is done.
    #[test]
    fn a_slow_walk_blocks_neither_the_lock_nor_another_root() {
        const A: u64 = 0x1_2980_0000;
        const B: u64 = 0x1_2990_0000;
        let progress = Arc::new(Progress::default());
        let dev = Arc::new(Mutex::new((Seqs::default(), InvalidateGate::default())));
        // Root B had a plan earlier, already applied.
        {
            let mut d = dev.lock().unwrap();
            let s = d.0.next(B);
            progress.begin(s, B);
            progress.finish(s);
        }
        // The trigger write: plan A, submit, arm, return.
        let t0 = Instant::now();
        let worker = {
            let mut d = dev.lock().unwrap();
            let s = d.0.next(A);
            progress.begin(s, A);
            let targets = vec![(A, d.0.last_of(A))];
            d.1.arm(targets, Instant::now(), &progress);
            slow_worker(&progress, s, Duration::from_millis(300))
        };
        assert!(t0.elapsed() < Duration::from_millis(50), "the write returned at once");
        assert_eq!(progress.urgent_roots(), vec![A], "the gate makes A urgent");

        // The display tick on another thread gets the device lock while the walk runs.
        let tick = {
            let dev = dev.clone();
            let progress = progress.clone();
            std::thread::spawn(move || {
                let t = Instant::now();
                let d = dev.lock().unwrap();
                let waited = t.elapsed();
                let walk_pending = !progress.is_done(&Target::Roots(vec![(A, d.0.last_of(A))]));
                (waited, walk_pending)
            })
        };
        let (waited, walk_pending) = tick.join().unwrap();
        assert!(waited < Duration::from_millis(50), "tick waited {waited:?}");
        assert!(walk_pending);

        // A doorbell of root B: its own plans are applied, no wait.
        let t = Instant::now();
        {
            let d = dev.lock().unwrap();
            progress.wait(&Target::Roots(vec![(B, d.0.last_of(B))]));
        }
        assert!(t.elapsed() < Duration::from_millis(50));

        // The guest polls the trigger; each read takes the lock only briefly.
        let mut busy = 0;
        let mut longest = Duration::ZERO;
        let held = loop {
            let t = Instant::now();
            let r = dev.lock().unwrap().1.read(&progress);
            longest = longest.max(t.elapsed());
            match r {
                Trigger::Busy => {
                    busy += 1;
                    std::thread::sleep(Duration::from_micros(200));
                }
                Trigger::Done(d) => break d,
            }
        };
        assert!(busy > 0, "the trigger read busy while the walk ran");
        assert!(progress.idle(), "done only after the worker finished");
        assert!(held.unwrap() >= Duration::from_millis(250));
        assert!(longest < Duration::from_millis(50), "a read held the lock {longest:?}");
        assert_eq!(dev.lock().unwrap().1.busy_reads, busy);
        assert!(progress.urgent_roots().is_empty(), "the open gate unmarks A");
        // The next read with nothing armed is simply done.
        assert_eq!(dev.lock().unwrap().1.read(&progress), Trigger::Done(None));
        worker.join().unwrap();
    }

    /// A doorbell on the gated root waits for the walk (the per-root barrier), and its
    /// root is urgent while it waits.
    #[test]
    fn a_doorbell_on_the_gated_root_waits_for_the_walk() {
        const A: u64 = 0x1_2980_0000;
        let progress = Arc::new(Progress::default());
        let mut seqs = Seqs::default();
        let s = seqs.next(A);
        progress.begin(s, A);
        let t0 = Instant::now();
        let p2 = progress.clone();
        let w = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let urgent = p2.urgent_roots();
            std::thread::sleep(Duration::from_millis(100));
            p2.finish(s);
            urgent
        });
        progress.wait(&Target::Roots(vec![(A, seqs.last_of(A))]));
        assert!(t0.elapsed() >= Duration::from_millis(140));
        assert_eq!(w.join().unwrap(), vec![A], "urgent while the doorbell waits");
        assert!(progress.urgent_roots().is_empty());
        assert_eq!(progress.done(), s);
        seqs.forget_done(progress.done());
        assert_eq!(seqs.last_of(A), 0);
    }

    /// Jobs finish out of order (an urgent root first): a root's wait is satisfied by its
    /// own jobs; a wait for everything up to a number, and `done`, only when every job up to
    /// it is finished.
    #[test]
    fn jobs_of_another_root_may_finish_first() {
        const A: u64 = 0xA000;
        const B: u64 = 0xB000;
        let p = Progress::default();
        p.begin(1, A);
        p.begin(2, B);
        p.begin(3, A);
        p.finish(2);
        assert!(p.is_done(&Target::Roots(vec![(B, 2)])));
        assert!(!p.is_done(&Target::Roots(vec![(A, 1)])));
        assert!(!p.is_done(&Target::All(2)));
        assert_eq!(p.done(), 0);
        p.finish(1);
        assert!(p.is_done(&Target::All(2)));
        assert!(p.is_done(&Target::Roots(vec![(A, 1)])));
        assert!(!p.is_done(&Target::Roots(vec![(A, 3)])));
        assert_eq!(p.done(), 2);
        assert!(!p.idle());
        p.finish(3);
        assert!(p.idle());
        assert_eq!(p.done(), 3);
        assert!(p.is_done(&Target::Roots(vec![(0xC000, 9)])), "a root with nothing in flight");
    }

    /// Past [`FALLBACK`] a read waits for the worker to the end: the guest never sees
    /// TRIGGER FALSE before the apply, whatever its own timeout.
    #[test]
    fn a_read_past_the_fallback_waits_to_the_end() {
        let progress = Arc::new(Progress::default());
        progress.begin(1, 0xA000);
        let mut g = InvalidateGate::default();
        g.arm(vec![(0xA000, 1)], Instant::now() - FALLBACK, &progress);
        let w = slow_worker(&progress, 1, Duration::from_millis(100));
        assert!(matches!(g.read(&progress), Trigger::Done(Some(_))));
        assert!(progress.idle());
        assert_eq!(g.fallbacks, 1);
        w.join().unwrap();
    }

    /// A second held invalidate extends the gate by its roots.
    #[test]
    fn a_second_invalidate_extends_the_gate() {
        let progress = Progress::default();
        progress.begin(3, 0xA000);
        progress.begin(5, 0xB000);
        let mut g = InvalidateGate::default();
        let t = Instant::now();
        g.arm(vec![(0xA000, 3)], t, &progress);
        g.arm(vec![(0xB000, 5)], t + Duration::from_millis(1), &progress);
        progress.finish(3);
        assert_eq!(g.read(&progress), Trigger::Busy);
        progress.finish(5);
        assert!(matches!(g.read(&progress), Trigger::Done(Some(_))));
        assert!(!g.armed());
        assert!(progress.urgent_roots().is_empty());
    }

    /// A gate whose invalidate submitted nothing (the shadow was already current) opens at
    /// the first read.
    #[test]
    fn a_gate_with_nothing_in_flight_opens_at_once() {
        let progress = Progress::default();
        let mut g = InvalidateGate::default();
        g.arm(vec![(0xA000, 0)], Instant::now(), &progress);
        assert!(matches!(g.read(&progress), Trigger::Done(Some(_))));
    }
}

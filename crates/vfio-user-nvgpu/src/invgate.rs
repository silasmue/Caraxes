// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
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
use std::sync::{Condvar, Mutex};
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

/// The worker's side: the sequence number of the last job it finished. Jobs finish in
/// submission order, so `done() >= s` means every job up to `s` is applied.
#[derive(Default)]
pub struct Progress {
    done: Mutex<u64>,
    cv: Condvar,
}

impl Progress {
    /// The worker finished job `seq` (and with it every earlier one).
    pub fn finish(&self, seq: u64) {
        let mut d = self.done.lock().unwrap();
        *d = (*d).max(seq);
        self.cv.notify_all();
    }

    pub fn done(&self) -> u64 {
        *self.done.lock().unwrap()
    }

    /// Block until job `seq` is finished.
    pub fn wait_for(&self, seq: u64) {
        let mut d = self.done.lock().unwrap();
        while *d < seq {
            d = self.cv.wait(d).unwrap();
        }
    }

    /// Block until job `seq` is finished or `max` passed; whether it is finished.
    pub fn wait_for_at_most(&self, seq: u64, max: Duration) -> bool {
        let end = Instant::now() + max;
        let mut d = self.done.lock().unwrap();
        while *d < seq {
            let now = Instant::now();
            if now >= end {
                return false;
            }
            d = self.cv.wait_timeout(d, end - now).unwrap().0;
        }
        true
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

/// The invalidate in flight: TRIGGER reads busy until the worker reached `until`.
#[derive(Default, Debug)]
pub struct InvalidateGate {
    pending: Option<(u64, Instant)>,
    /// Trigger reads that reported busy.
    pub busy_reads: u64,
    /// Reads that fell back to waiting for the worker to the end ([`FALLBACK`]).
    pub fallbacks: u64,
}

impl InvalidateGate {
    /// A held invalidate submitted its plans up to job `until` at `now`. A second one while
    /// the first is pending extends the gate (the guest polls before it writes the next,
    /// so this is the defensive case).
    pub fn arm(&mut self, until: u64, now: Instant) {
        self.pending = Some(match self.pending {
            Some((u, t)) => (u.max(until), t),
            None => (until, now),
        });
    }

    pub fn armed(&self) -> bool {
        self.pending.is_some()
    }

    /// A read of the trigger: wait at most [`POLL_WAIT`] (to the end past [`FALLBACK`]) for
    /// the worker, then report.
    pub fn read(&mut self, progress: &Progress) -> Trigger {
        let Some((until, since)) = self.pending else {
            return Trigger::Done(None);
        };
        let done = if since.elapsed() >= FALLBACK {
            self.fallbacks += 1;
            progress.wait_for(until);
            true
        } else {
            progress.wait_for_at_most(until, POLL_WAIT)
        };
        if done {
            self.pending = None;
            Trigger::Done(Some(since.elapsed()))
        } else {
            self.busy_reads += 1;
            Trigger::Busy
        }
    }

    /// A device reset: the worker was drained, nothing is in flight.
    pub fn clear(&mut self) {
        self.pending = None;
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
            progress.finish(s);
        }
        // The trigger write: plan A, submit, arm, return.
        let t0 = Instant::now();
        let worker = {
            let mut d = dev.lock().unwrap();
            let s = d.0.next(A);
            let until = d.0.submitted();
            d.1.arm(until, Instant::now());
            slow_worker(&progress, s, Duration::from_millis(300))
        };
        assert!(t0.elapsed() < Duration::from_millis(50), "the write returned at once");

        // The display tick on another thread gets the device lock while the walk runs.
        let tick = {
            let dev = dev.clone();
            let progress = progress.clone();
            std::thread::spawn(move || {
                let t = Instant::now();
                let d = dev.lock().unwrap();
                let waited = t.elapsed();
                // The walk is still running when the tick has the lock.
                let walk_pending = progress.done() < d.0.last_of(A);
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
            progress.wait_for(d.0.last_of(B));
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
        assert!(progress.done() >= 2, "done only after the worker finished");
        assert!(held.unwrap() >= Duration::from_millis(250));
        assert!(longest < Duration::from_millis(50), "a read held the lock {longest:?}");
        assert_eq!(dev.lock().unwrap().1.busy_reads, busy);
        // The next read with nothing armed is simply done.
        assert_eq!(dev.lock().unwrap().1.read(&progress), Trigger::Done(None));
        worker.join().unwrap();
    }

    /// A doorbell on the gated root waits for the walk (the per-root barrier).
    #[test]
    fn a_doorbell_on_the_gated_root_waits_for_the_walk() {
        const A: u64 = 0x1_2980_0000;
        let progress = Arc::new(Progress::default());
        let mut seqs = Seqs::default();
        let s = seqs.next(A);
        let t0 = Instant::now();
        let w = slow_worker(&progress, s, Duration::from_millis(150));
        progress.wait_for(seqs.last_of(A));
        assert!(t0.elapsed() >= Duration::from_millis(140));
        assert!(progress.done() >= s);
        w.join().unwrap();
        seqs.forget_done(progress.done());
        assert_eq!(seqs.last_of(A), 0);
    }

    /// Past [`FALLBACK`] a read waits for the worker to the end: the guest never sees
    /// TRIGGER FALSE before the apply, whatever its own timeout.
    #[test]
    fn a_read_past_the_fallback_waits_to_the_end() {
        let progress = Arc::new(Progress::default());
        let mut g = InvalidateGate::default();
        g.arm(1, Instant::now() - FALLBACK);
        let w = slow_worker(&progress, 1, Duration::from_millis(100));
        assert!(matches!(g.read(&progress), Trigger::Done(Some(_))));
        assert_eq!(progress.done(), 1);
        assert_eq!(g.fallbacks, 1);
        w.join().unwrap();
    }

    #[test]
    fn a_second_invalidate_extends_the_gate() {
        let progress = Progress::default();
        let mut g = InvalidateGate::default();
        let t = Instant::now();
        g.arm(3, t);
        g.arm(5, t + Duration::from_millis(1));
        progress.finish(3);
        assert_eq!(g.read(&progress), Trigger::Busy);
        progress.finish(5);
        assert!(matches!(g.read(&progress), Trigger::Done(Some(_))));
        assert!(!g.armed());
    }
}

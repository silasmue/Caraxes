// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Bounded backend submission spans on the host timeline's clock. No hot-path I/O.

use super::host_timeline::Clock;
use std::cell::Cell;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError, TryLockError};
use std::time::Instant;

// Startup polling exceeds 20,000 rows/s. A 20 MiB ring allows snapshot overlap;
// retained sequence IDs still determine actual coverage.
pub const CAPACITY: usize = 262144;
static ON: AtomicBool = AtomicBool::new(false);
static TIMELINE: OnceLock<Timeline> = OnceLock::new();
thread_local! {
    static TID: Cell<u32> = const { Cell::new(0) };
}

#[inline]
fn native_tid() -> u32 {
    TID.with(|cached| {
        let tid = cached.get();
        if tid != 0 {
            return tid;
        }
        // SAFETY: gettid has no arguments or pointer side effects. Linux TIDs are positive.
        let tid = unsafe { libc::syscall(libc::SYS_gettid) as u32 };
        cached.set(tid);
        tid
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    DeviceLock,
    PrivateUserdScan,
    ShadowPlan,
    Bar2Sync,
    Bar1ReplicaSync,
    FbFlush,
    UserdPublish,
    UserdHarvest,
    RingHandler,
    HeldRingWait,
    PhysicalDoorbell,
}

impl Stage {
    fn label(self) -> &'static str {
        match self {
            Self::DeviceLock => "device_lock",
            Self::PrivateUserdScan => "private_userd_scan",
            Self::ShadowPlan => "shadow_plan",
            Self::Bar2Sync => "bar2_sync",
            Self::Bar1ReplicaSync => "bar1_replica_sync",
            Self::FbFlush => "fb_flush",
            Self::UserdPublish => "userd_publish",
            Self::UserdHarvest => "userd_harvest",
            Self::RingHandler => "ring_handler",
            Self::HeldRingWait => "held_ring_wait",
            Self::PhysicalDoorbell => "physical_doorbell",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    TrappedWrite,
    Eventfd,
    UsermodePage,
    PostedDoorbell,
    PolledUserd,
    HeldRingCompleter,
    Mirror,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Self::TrappedWrite => "trapped_write",
            Self::Eventfd => "eventfd",
            Self::UsermodePage => "usermode_page",
            Self::PostedDoorbell => "posted_doorbell",
            Self::PolledUserd => "polled_userd",
            Self::HeldRingCompleter => "held_ring_completer",
            Self::Mirror => "mirror",
        }
    }
}

/// Identifiers known at the callsite. An absent value must not be inferred from
/// neighboring spans or a physical doorbell on another channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ids {
    pub chid: Option<u32>,
    pub generation: Option<u64>,
}

impl Ids {
    pub const NONE: Self = Self {
        chid: None,
        generation: None,
    };
}

/// A scope timer that records on every return path. It owns no heap storage.
pub struct Span {
    stage: Stage,
    source: Source,
    ids: Ids,
    begin: Option<Instant>,
}

impl Span {
    #[inline]
    pub fn new(stage: Stage, source: Source, ids: Ids) -> Self {
        Self::new_with_ids(stage, source, || ids)
    }

    /// Build identifiers only while submission profiling is active.
    #[inline]
    pub fn new_with_ids(stage: Stage, source: Source, ids: impl FnOnce() -> Ids) -> Self {
        let begin = start();
        Self {
            stage,
            source,
            ids: if begin.is_some() { ids() } else { Ids::NONE },
            begin,
        }
    }

    /// Reuse the identifiers captured for this scope in nested stages.
    #[inline]
    pub fn ids(&self) -> Ids {
        self.ids
    }
}

impl Drop for Span {
    #[inline]
    fn drop(&mut self) {
        if let Some(begin) = self.begin {
            record(self.stage, self.source, self.ids, begin);
        }
    }
}

#[derive(Clone, Copy)]
struct Row {
    sequence: u64,
    tid: u32,
    stage: Stage,
    source: Source,
    ids: Ids,
    begin_ns: u128,
    end_ns: u128,
}

struct State {
    rows: VecDeque<Row>,
    overflow: u64,
    invalid: u64,
}

struct Timeline {
    anchor: Clock,
    capacity: usize,
    state: Mutex<State>,
    seen: AtomicU64,
    contention: AtomicU64,
    poisoned: AtomicU64,
}

impl Timeline {
    fn new(anchor: Clock, capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            anchor,
            capacity,
            state: Mutex::new(State {
                rows: VecDeque::with_capacity(capacity),
                overflow: 0,
                invalid: 0,
            }),
            seen: AtomicU64::new(0),
            contention: AtomicU64::new(0),
            poisoned: AtomicU64::new(0),
        }
    }

    fn record(&self, stage: Stage, source: Source, ids: Ids, begin: Instant, end: Instant) {
        let sequence = self.seen.fetch_add(1, Ordering::Relaxed) + 1;
        let mut state = match self.state.try_lock() {
            Ok(state) => state,
            Err(TryLockError::WouldBlock) => {
                self.contention.fetch_add(1, Ordering::Relaxed);
                return;
            }
            Err(TryLockError::Poisoned(_)) => {
                self.poisoned.fetch_add(1, Ordering::Relaxed);
                return;
            }
        };
        let (Some(begin), Some(end)) = (
            begin.checked_duration_since(self.anchor.mono),
            end.checked_duration_since(self.anchor.mono),
        ) else {
            state.invalid += 1;
            return;
        };
        if end < begin {
            state.invalid += 1;
            return;
        }
        if state.rows.len() == self.capacity {
            state.rows.pop_front();
            state.overflow += 1;
        }
        state.rows.push_back(Row {
            sequence,
            tid: native_tid(),
            stage,
            source,
            ids,
            begin_ns: begin.as_nanos(),
            end_ns: end.as_nanos(),
        });
    }

    fn tsv(&self, reason: &str) -> String {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut out = String::new();
        let _ = writeln!(out, "# SPDX-License-Identifier: MIT\n# backend submission timeline schema=1 pid={} reason={reason}", std::process::id());
        self.anchor.header(&mut out);
        let _ = writeln!(
            out,
            "# capacity={} seen={} retained={} overflow={} contention={} poisoned={} invalid={}",
            self.capacity,
            self.seen.load(Ordering::Relaxed),
            state.rows.len(),
            state.overflow,
            self.contention.load(Ordering::Relaxed),
            self.poisoned.load(Ordering::Relaxed),
            state.invalid
        );
        let _ = writeln!(
            out,
            "# record_bytes={} storage_capacity_bytes={}",
            std::mem::size_of::<Row>(),
            state.rows.capacity() * std::mem::size_of::<Row>()
        );
        out.push_str("# Backend spans only. They do not establish GPU execution or completion. Nested durations overlap and must not be summed.\n");
        out.push_str("# sequence is attempted writer order; rows are lock acquisition order. tid is the cached native Linux thread ID, useful for distinguishing nested from concurrent spans. Snapshots overlap and do not consume; writers that contend with a snapshot are dropped and counted. Missing identifiers are '-'. Times are elapsed from the shared host-profile monotonic anchor, not UTC or an Nsight clock.\n");
        out.push_str(
            "sequence\ttid\tstage\tsource\tbegin_ns\tend_ns\tduration_ns\tchid\tgeneration\n",
        );
        for row in &state.rows {
            let _ = write!(
                out,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                row.sequence,
                row.tid,
                row.stage.label(),
                row.source.label(),
                row.begin_ns,
                row.end_ns,
                row.end_ns - row.begin_ns
            );
            match row.ids.chid {
                Some(v) => {
                    let _ = write!(out, "\t{v}");
                }
                None => out.push_str("\t-"),
            }
            match row.ids.generation {
                Some(v) => {
                    let _ = write!(out, "\t{v}");
                }
                None => out.push_str("\t-"),
            }
            out.push('\n');
        }
        out
    }
}

pub(super) fn configure(anchor: Clock) {
    assert!(TIMELINE.set(Timeline::new(anchor, CAPACITY)).is_ok());
    ON.store(true, Ordering::Release);
}

/// Disabled cost is one relaxed atomic load, with no timestamp or allocation.
#[inline]
pub fn start() -> Option<Instant> {
    ON.load(Ordering::Relaxed).then(Instant::now)
}

/// Finish an observed backend span. A contended writer drops the row rather than
/// waiting on the timeline's mutex.
#[inline]
pub fn record(stage: Stage, source: Source, ids: Ids, begin: Instant) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let end = Instant::now();
    if let Some(timeline) = TIMELINE.get() {
        timeline.record(stage, source, ids, begin, end);
    }
}

pub(super) fn snapshot(reason: &str) -> Option<String> {
    TIMELINE.get().map(|timeline| timeline.tsv(reason))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn disabled_start_and_record_leave_storage_untouched() {
        assert!(start().is_none());
        let mut ids_built = false;
        {
            let _scope = Span::new_with_ids(Stage::RingHandler, Source::Mirror, || {
                ids_built = true;
                Ids::NONE
            });
        }
        assert!(!ids_built);
        let t = Timeline::new(Clock::capture(), 2);
        assert_eq!(t.seen.load(Ordering::Relaxed), 0);
        assert!(t.state.lock().unwrap().rows.is_empty());
    }

    #[test]
    fn bounds_contention_and_bad_clock_order_are_explicit() {
        let t = Timeline::new(Clock::capture(), 2);
        let at = t.anchor.mono;
        for i in 0..3 {
            let begin = at + Duration::from_nanos(10 + i);
            t.record(
                Stage::RingHandler,
                Source::Mirror,
                Ids::NONE,
                begin,
                begin + Duration::from_nanos(2),
            );
        }
        let guard = t.state.lock().unwrap();
        t.record(Stage::DeviceLock, Source::TrappedWrite, Ids::NONE, at, at);
        drop(guard);
        t.record(
            Stage::FbFlush,
            Source::Mirror,
            Ids::NONE,
            at - Duration::from_nanos(1),
            at,
        );
        t.record(
            Stage::FbFlush,
            Source::Mirror,
            Ids::NONE,
            at + Duration::from_nanos(2),
            at,
        );
        assert!(t
            .tsv("bounds")
            .contains("seen=6 retained=2 overflow=1 contention=1 poisoned=0 invalid=2"));
        assert_eq!(t.state.lock().unwrap().rows.front().unwrap().sequence, 2);
    }

    #[test]
    fn dump_shares_anchor_and_preserves_source_ids_and_order() {
        let host = super::super::host_timeline::Timeline::new(2);
        let t = Timeline::new(host.anchor(), 2);
        let at = host.anchor().mono;
        t.record(
            Stage::PhysicalDoorbell,
            Source::HeldRingCompleter,
            Ids {
                chid: Some(7),
                generation: Some(9),
            },
            at + Duration::from_nanos(123),
            at + Duration::from_nanos(130),
        );
        let a = t.tsv("first");
        let b = t.tsv("second");
        let anchor = |s: &str| {
            s.lines()
                .find(|l| l.starts_with("# anchor_unix_before_ns="))
                .unwrap()
                .split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(anchor(&a), anchor(&host.tsv("test")));
        assert_eq!(
            a.lines()
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<_>>(),
            b.lines()
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<_>>()
        );
        assert!(a.contains("\tphysical_doorbell\theld_ring_completer\t123\t130\t7\t7\t9"));
        assert_eq!(t.state.lock().unwrap().rows[0].tid, native_tid());
    }
}

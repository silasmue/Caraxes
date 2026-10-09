// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Backend observations, sharing the host ioctl timeline's exact clock anchor.
//! Storage is preallocated at startup; writers never wait or allocate.

use super::host_timeline::Clock;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError, TryLockError};
use std::time::Instant;

pub(super) const CAPACITY: usize = 65536;
static ON: AtomicBool = AtomicBool::new(false);
static MARKERS: OnceLock<Markers> = OnceLock::new();

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    PhysicalDoorbellWritten,
    HostNotifierObserved,
    VectorRaiseRequested,
    LocalIrqCausePublished,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::PhysicalDoorbellWritten => "physical_doorbell_written",
            Self::HostNotifierObserved => "host_notifier_observed",
            Self::VectorRaiseRequested => "vector_raise_requested",
            Self::LocalIrqCausePublished => "local_irq_cause_published",
        }
    }
}

/// Only identifiers known at the observation point. An absent channel means that
/// an engine event or folded cause cannot be attributed to a single channel.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ids {
    pub chid: Option<u32>,
    pub channel_generation: Option<u64>,
    pub producer_generation: Option<u64>,
    pub vector: Option<u32>,
    pub notifier: Option<u32>,
    pub epoch: Option<u64>,
    pub ticket: Option<u64>,
}

struct Marker {
    sequence: u64,
    kind: Kind,
    at_ns: u128,
    ids: Ids,
}

struct State {
    rows: VecDeque<Marker>,
    overflow: u64,
    invalid: u64,
}

struct Markers {
    anchor: Clock,
    capacity: usize,
    state: Mutex<State>,
    seen: AtomicU64,
    contention: AtomicU64,
    poisoned: AtomicU64,
}

impl Markers {
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

    fn record(&self, enabled: bool, kind: Kind, ids: Ids, at: Instant) {
        if !enabled {
            return;
        }
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
        if at < self.anchor.mono || ids.vector.is_some_and(|v| v >= 256) {
            state.invalid += 1;
            return;
        }
        if state.rows.len() == self.capacity {
            state.rows.pop_front();
            state.overflow += 1;
        }
        state.rows.push_back(Marker {
            sequence,
            kind,
            at_ns: at.duration_since(self.anchor.mono).as_nanos(),
            ids,
        });
    }

    fn tsv(&self, reason: &str) -> String {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut out = String::new();
        let _ = writeln!(out, "# SPDX-License-Identifier: MIT\n# backend observation markers schema=1 pid={} reason={reason}", std::process::id());
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
            std::mem::size_of::<Marker>(),
            state.rows.capacity() * std::mem::size_of::<Marker>()
        );
        out.push_str("# Observations only: no GPU completion, guest IRQ delivery or guest ACK. Raise requested precedes the call and does not imply generation acceptance or MSI. Publication follows successful Fold publish and wake; vectors identify folded causes, not channel ownership.\n");
        out.push_str("# sequence is attempted writer order; rows are lock acquisition order; use at_ns for observation order. Snapshots overlap and do not consume; drop counters may advance during formatting. Missing identifiers are '-'.\n");
        out.push_str("sequence\tkind\tat_ns\tchid\tchannel_generation\tproducer_generation\tvector\tnotifier\tepoch\tticket\n");
        for row in &state.rows {
            let _ = write!(out, "{}\t{}\t{}", row.sequence, row.kind.label(), row.at_ns);
            for id in [
                row.ids.chid.map(u64::from),
                row.ids.channel_generation,
                row.ids.producer_generation,
                row.ids.vector.map(u64::from),
                row.ids.notifier.map(u64::from),
                row.ids.epoch,
                row.ids.ticket,
            ] {
                match id {
                    Some(id) => {
                        let _ = write!(out, "\t{id}");
                    }
                    None => out.push_str("\t-"),
                }
            }
            out.push('\n');
        }
        out
    }
}

pub(super) fn start(anchor: Clock) {
    // The process-wide ioctl observer permits only one successful timeline start.
    assert!(MARKERS.set(Markers::new(anchor, CAPACITY)).is_ok());
    ON.store(true, Ordering::Release);
}

/// The disabled path is one relaxed load, before timestamp capture or lookup.
#[inline]
pub fn record(kind: Kind, ids: Ids) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let at = Instant::now();
    if let Some(markers) = MARKERS.get() {
        markers.record(true, kind, ids, at);
    }
}

/// A successful Fold publication may contain several vectors and cannot identify
/// their channels. Give every cause the same post-publication observation time.
/// Read the owner's generation only after the disabled fast path.
pub fn cause_published(
    vectors: &[u32; crate::intr::LEAVES],
    epoch: u64,
    ticket: u64,
    producer_generation: impl FnOnce() -> u64,
) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let at = Instant::now();
    if let Some(markers) = MARKERS.get() {
        let ids = Ids {
            producer_generation: Some(producer_generation()),
            epoch: Some(epoch),
            ticket: Some(ticket),
            ..Ids::default()
        };
        markers.record_causes(vectors, ids, at);
    }
}

impl Markers {
    fn record_causes(&self, vectors: &[u32; crate::intr::LEAVES], ids: Ids, at: Instant) {
        let mut any = false;
        for (leaf, &bits) in vectors.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let bit = bits.trailing_zeros();
                bits &= bits - 1;
                any = true;
                self.record(
                    true,
                    Kind::LocalIrqCausePublished,
                    Ids {
                        vector: Some(leaf as u32 * 32 + bit),
                        ..ids
                    },
                    at,
                );
            }
        }
        if !any {
            self.record(true, Kind::LocalIrqCausePublished, ids, at);
        }
    }
}

pub(super) fn snapshot(reason: &str) -> Option<String> {
    MARKERS.get().map(|markers| markers.tsv(reason))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn bounds_drops_and_snapshots_are_explicit() {
        let m = Markers::new(Clock::capture(), 2);
        let at = m.anchor.mono;
        for i in 0..3 {
            m.record(
                true,
                Kind::PhysicalDoorbellWritten,
                Ids {
                    chid: Some(i),
                    channel_generation: Some(5),
                    ..Ids::default()
                },
                at,
            );
        }
        let guard = m.state.lock().unwrap();
        m.record(true, Kind::VectorRaiseRequested, Ids::default(), at);
        drop(guard);
        m.record(
            true,
            Kind::VectorRaiseRequested,
            Ids::default(),
            at - Duration::from_nanos(1),
        );
        m.record(
            true,
            Kind::VectorRaiseRequested,
            Ids {
                vector: Some(256),
                ..Ids::default()
            },
            at,
        );
        let a = m.tsv("first");
        let b = m.tsv("second");
        assert!(a.contains("seen=6 retained=2 overflow=1 contention=1 poisoned=0 invalid=2"));
        assert_eq!(
            a.lines()
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<_>>(),
            b.lines()
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<_>>()
        );
        assert_eq!(m.state.lock().unwrap().rows.front().unwrap().sequence, 2);
    }

    #[test]
    fn disabled_does_not_change_storage_or_counts() {
        let m = Markers::new(Clock::capture(), 2);
        m.record(
            false,
            Kind::VectorRaiseRequested,
            Ids::default(),
            m.anchor.mono,
        );
        assert_eq!(m.seen.load(Ordering::Relaxed), 0);
        assert!(m.state.lock().unwrap().rows.is_empty());
    }

    #[test]
    fn poisoned_writers_drop_and_snapshot_retains_old_rows() {
        let m = Markers::new(Clock::capture(), 2);
        m.record(
            true,
            Kind::VectorRaiseRequested,
            Ids::default(),
            m.anchor.mono,
        );
        let _ = std::panic::catch_unwind(|| {
            let _guard = m.state.lock().unwrap();
            panic!("poison marker storage");
        });
        m.record(
            true,
            Kind::VectorRaiseRequested,
            Ids::default(),
            m.anchor.mono,
        );
        assert!(m
            .tsv("poisoned")
            .contains("seen=2 retained=1 overflow=0 contention=0 poisoned=1 invalid=0"));
    }

    #[test]
    fn host_spans_and_markers_share_the_exact_clock_and_preserve_status() {
        let mut t = super::super::host_timeline::Timeline::new(2);
        let m = Markers::new(t.anchor(), 2);
        let at = t.anchor().mono + Duration::from_nanos(123);
        t.record(0x2a, 0x2080_a612, 0x56, at, at + Duration::from_nanos(7));
        m.record(
            true,
            Kind::HostNotifierObserved,
            Ids {
                vector: Some(6),
                producer_generation: Some(2),
                ..Ids::default()
            },
            at,
        );
        let spans = t.tsv("test");
        let markers = m.tsv("test");
        let anchor = |s: &str| {
            s.lines()
                .find(|l| l.starts_with("# anchor_unix_before_ns="))
                .unwrap()
                .split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(anchor(&spans), anchor(&markers));
        assert!(spans.contains("0x2080a612\t0x56\t123\t130\t7"));
        assert!(markers.contains("host_notifier_observed\t123\t-\t-\t2\t6\t-\t-\t-"));
    }

    #[test]
    fn folded_causes_share_publication_time_and_owned_identifiers() {
        let m = Markers::new(Clock::capture(), 4);
        let mut vectors = [0; crate::intr::LEAVES];
        vectors[0] = 1 << 6;
        vectors[2] = 1 << 3;
        m.record_causes(
            &vectors,
            Ids {
                producer_generation: Some(7),
                epoch: Some(11),
                ticket: Some(13),
                ..Ids::default()
            },
            m.anchor.mono,
        );
        let st = m.state.lock().unwrap();
        assert_eq!(st.rows.len(), 2);
        assert_eq!(st.rows[0].at_ns, st.rows[1].at_ns);
        assert_eq!(st.rows[0].ids.vector, Some(6));
        assert_eq!(st.rows[1].ids.vector, Some(67));
        for row in &st.rows {
            assert_eq!(row.ids.producer_generation, Some(7));
            assert_eq!(row.ids.epoch, Some(11));
            assert_eq!(row.ids.ticket, Some(13));
            assert_eq!(row.ids.chid, None);
            assert_eq!(row.ids.channel_generation, None);
        }
    }
}

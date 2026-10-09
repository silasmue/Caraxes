// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Bounded timestamp history for selected host controls. No payloads or file I/O.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const CAPACITY: usize = 8192;

struct Clock {
    mono: Instant,
    unix_before_ns: u128,
    unix_after_ns: u128,
}

impl Clock {
    fn capture() -> Self {
        let unix = || {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        };
        let unix_before_ns = unix();
        let mono = Instant::now();
        let unix_after_ns = unix();
        Self {
            mono,
            unix_before_ns,
            unix_after_ns,
        }
    }
}

#[derive(Debug)]
struct Span {
    sequence: u64,
    cmd: u32,
    status: u32,
    begin_ns: u128,
    end_ns: u128,
}

pub struct Timeline {
    anchor: Clock,
    spans: VecDeque<Span>,
    capacity: usize,
    seen: u64,
    dropped: u64,
    invalid: u64,
}

impl Timeline {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            anchor: Clock::capture(),
            spans: VecDeque::with_capacity(capacity),
            capacity,
            seen: 0,
            dropped: 0,
            invalid: 0,
        }
    }

    pub fn record(&mut self, nr: u32, cmd: u32, status: u32, begin: Instant, end: Instant) {
        if nr != 0x2a || !matches!(cmd, 0x2080_a612 | 0x2080_a613) {
            return;
        }
        self.seen += 1;
        if begin < self.anchor.mono || end < begin {
            self.invalid += 1;
            return;
        }
        if self.spans.len() == self.capacity {
            self.spans.pop_front();
            self.dropped += 1;
        }
        self.spans.push_back(Span {
            sequence: self.seen,
            cmd,
            status,
            begin_ns: begin.duration_since(self.anchor.mono).as_nanos(),
            end_ns: end.duration_since(self.anchor.mono).as_nanos(),
        });
    }

    pub fn tsv(&self, reason: &str) -> String {
        let snapshot = Clock::capture();
        let mut out = String::new();
        let _ = writeln!(
            out,
            "# SPDX-License-Identifier: MIT\n# host ioctl timeline schema=1 pid={} reason={reason}",
            std::process::id()
        );
        let _ = writeln!(out, "# anchor_unix_before_ns={} anchor_unix_after_ns={} snapshot_mono_ns={} snapshot_unix_before_ns={} snapshot_unix_after_ns={}",
            self.anchor.unix_before_ns, self.anchor.unix_after_ns,
            snapshot.mono.duration_since(self.anchor.mono).as_nanos(),
            snapshot.unix_before_ns, snapshot.unix_after_ns);
        let _ = writeln!(
            out,
            "# capacity={} seen={} retained={} dropped={} invalid={}",
            self.capacity,
            self.seen,
            self.spans.len(),
            self.dropped,
            self.invalid
        );
        out.push_str("# Completed host calls only; callback recording order; use end_ns for observed completion order; snapshots overlap. No guest reply, GPU completion or firmware/lock attribution.\n");
        out.push_str("sequence\tcmd\tstatus\tbegin_ns\tend_ns\tduration_ns\n");
        for s in &self.spans {
            let _ = writeln!(
                out,
                "{}\t{:#010x}\t{:#x}\t{}\t{}\t{}",
                s.sequence,
                s.cmd,
                s.status,
                s.begin_ns,
                s.end_ns,
                s.end_ns - s.begin_ns
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn capacity_eviction_is_explicit_and_snapshots_do_not_consume() {
        let mut t = Timeline::new(2);
        for i in 0..3 {
            let begin = t.anchor.mono + Duration::from_nanos(10 + i);
            t.record(0x2a, 0x2080_a613, 0, begin, begin + Duration::from_nanos(7));
        }
        assert_eq!((t.seen, t.dropped, t.invalid, t.spans.len()), (3, 1, 0, 2));
        assert_eq!(t.spans[0].sequence, 2);
        let a = t.tsv("first");
        let b = t.tsv("second");
        assert!(a.contains("seen=3 retained=2 dropped=1 invalid=0"));
        assert_eq!(
            a.lines()
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<_>>(),
            b.lines()
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn unrelated_controls_are_not_recorded() {
        let mut t = Timeline::new(2);
        let begin = t.anchor.mono;
        t.record(0x29, 0x2080_a613, 0, begin, begin);
        t.record(0x2a, 0x2080_a614, 0, begin, begin);
        assert_eq!(t.seen, 0);
    }

    #[test]
    fn invalid_intervals_are_counted_without_fabricated_zero_times() {
        let mut t = Timeline::new(2);
        let begin = t.anchor.mono;
        t.record(0x2a, 0x2080_a613, 0, begin - Duration::from_nanos(1), begin);
        t.record(0x2a, 0x2080_a612, 0, begin, begin - Duration::from_nanos(1));
        assert_eq!((t.seen, t.invalid, t.spans.len()), (2, 2, 0));
    }

    #[test]
    fn recording_order_does_not_imply_start_or_completion_order() {
        let mut t = Timeline::new(2);
        let at = t.anchor.mono;
        t.record(
            0x2a,
            0x2080_a612,
            0x56,
            at + Duration::from_nanos(20),
            at + Duration::from_nanos(40),
        );
        t.record(
            0x2a,
            0x2080_a613,
            u32::MAX,
            at,
            at + Duration::from_nanos(30),
        );
        assert_eq!((t.spans[0].begin_ns, t.spans[1].begin_ns), (20, 0));
        assert_eq!((t.spans[0].end_ns, t.spans[1].end_ns), (40, 30));
        assert_eq!(t.spans[0].status, 0x56);
        assert_eq!(t.spans[1].status, u32::MAX);
        assert!(t.tsv("test").contains("duration_ns"));
    }
}

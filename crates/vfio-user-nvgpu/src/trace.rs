// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The access log: one line per region access, DMA map, interrupt setup or
//! reset, with a monotonic timestamp, so that a run can be read against
//! docs/caraxes/init-map.md. The summary counts accesses per region and lists
//! BAR0 offsets that have no name in the register map.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};

pub const REGION_NAMES: [&str; 9] = [
    "bar0", "bar1", "bar2", "bar3", "bar4", "bar5", "rom", "cfg", "vga",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Read,
    Write,
}

/// Which region accesses get a trace line (`CARAXES_TRACE_ACCESSES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accesses {
    None,
    All,
    /// BAR0 offsets in [lo, hi).
    Bar0(u64, u64),
}

impl Accesses {
    /// Unset or `1`: all; `0`: none; `bar0:<lo>-<hi>` (hex with 0x, or decimal): that
    /// BAR0 range. Anything else is refused (fail closed: a typo must not trace nothing).
    pub fn parse(v: Option<&str>) -> Result<Self> {
        let num = |t: &str| match t.strip_prefix("0x") {
            Some(h) => u64::from_str_radix(h, 16).ok(),
            None => t.parse().ok(),
        };
        match v {
            None | Some("1") => Ok(Accesses::All),
            Some("0") => Ok(Accesses::None),
            Some(r) => {
                let range = r.strip_prefix("bar0:").and_then(|x| x.split_once('-'));
                match range.and_then(|(lo, hi)| Some((num(lo)?, num(hi)?))) {
                    Some((lo, hi)) if lo < hi => Ok(Accesses::Bar0(lo, hi)),
                    _ => anyhow::bail!("CARAXES_TRACE_ACCESSES={r:?}: 0, 1 or bar0:<lo>-<hi>"),
                }
            }
        }
    }

    fn wants(self, region: u32, offset: u64) -> bool {
        match self {
            Accesses::None => false,
            Accesses::All => true,
            Accesses::Bar0(lo, hi) => region == 0 && (lo..hi).contains(&offset),
        }
    }
}

pub struct Trace {
    out: Option<BufWriter<File>>,
    start: Instant,
    reads: [u64; 9],
    writes: [u64; 9],
    unnamed: BTreeMap<u64, u64>,
    /// Every trapped BAR0 access by (dword offset, write): the count and the register's
    /// name, for the exit summary's busiest offsets (what a guest polls).
    bar0: std::collections::HashMap<(u64, bool), (u64, String)>,
    /// Trapped BAR1 and BAR2 (region 3) accesses per MiB of the BAR, by (region, MiB,
    /// write): where a guest driver's CPU traffic through the apertures lands (the BAR1
    /// pool covers only its first MiBs).
    by_mib: BTreeMap<(u32, u64, bool), u64>,
    /// `CARAXES_TRACE_ACCESSES=0`: count the region accesses but write no line per
    /// access (events are still written). Every trapped access otherwise formats and
    /// writes one line on the guest's MMIO round trip (docs/caraxes/mapping.md §6).
    /// `CARAXES_TRACE_ACCESSES=bar0:<lo>-<hi>` writes the lines of BAR0 accesses in
    /// [lo, hi) only (a diagnostic, e.g. the display block 0x610000-0x700000).
    accesses: Accesses,
    last_flush: Instant,
    /// The free-space guard ([`TRACE_MIN_FREE`]): when the trace's filesystem was last
    /// checked.
    last_space_check: Instant,
}

/// The trace stops below this much free space on its filesystem (said once, on stderr and as
/// the trace's last line): the guest's disk image and the VM state share it, and a full one
/// stopped C92's trace mid-line for 4 minutes (2026-09-29).
pub const TRACE_MIN_FREE: u64 = 10 << 30;

/// Free bytes on the filesystem of `f` (fstatvfs), if the call succeeds.
fn free_bytes(f: &File) -> Option<u64> {
    use std::os::fd::AsRawFd;
    // SAFETY: a zeroed statvfs is a valid out-parameter; the descriptor is open.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: fstatvfs writes only into `s`.
    (unsafe { libc::fstatvfs(f.as_raw_fd(), &mut s) } == 0)
        .then(|| s.f_bavail as u64 * s.f_frsize as u64)
}

/// Render an access value: little-endian integer up to 8 bytes, hex bytes beyond.
pub fn value(data: &[u8]) -> String {
    if data.len() <= 8 {
        let mut v = 0u64;
        for (i, b) in data.iter().enumerate() {
            v |= (*b as u64) << (8 * i);
        }
        format!("{v:#x}")
    } else {
        let head: Vec<String> = data.iter().take(16).map(|b| format!("{b:02x}")).collect();
        format!("{}B:{}", data.len(), head.join(""))
    }
}

impl Trace {
    pub fn new(path: Option<&Path>) -> Result<Self> {
        let out = match path {
            Some(p) => {
                Some(BufWriter::new(File::create(p).with_context(|| {
                    format!("create trace file {}", p.display())
                })?))
            }
            None => None,
        };
        Ok(Trace {
            out,
            start: Instant::now(),
            reads: [0; 9],
            writes: [0; 9],
            unnamed: BTreeMap::new(),
            bar0: std::collections::HashMap::new(),
            by_mib: BTreeMap::new(),
            accesses: Accesses::parse(std::env::var("CARAXES_TRACE_ACCESSES").ok().as_deref())?,
            last_flush: Instant::now(),
            last_space_check: Instant::now(),
        })
    }

    /// The free-space guard, at most once a second: below [`TRACE_MIN_FREE`] the trace ends
    /// with one line that says so, and nothing more is written.
    fn space_guard(&mut self) {
        if self.last_space_check.elapsed().as_secs() < 1 {
            return;
        }
        self.last_space_check = Instant::now();
        let Some(out) = &mut self.out else { return };
        let Some(free) = free_bytes(out.get_ref()) else { return };
        if free >= TRACE_MIN_FREE {
            return;
        }
        let m = format!(
            "trace stopped: {} MiB free on its filesystem, below {} MiB (the guest's disk and the VM state share it)",
            free >> 20,
            TRACE_MIN_FREE >> 20
        );
        let _ = writeln!(out, "{:>12} E {m}", self.start.elapsed().as_micros());
        let _ = out.flush();
        eprintln!("vfio-user-nvgpu: {m}");
        self.out = None;
    }

    fn line(&mut self, text: &str) {
        let t = self.start.elapsed().as_micros();
        self.space_guard();
        if let Some(out) = &mut self.out {
            let _ = writeln!(out, "{t:>12} {text}");
            // Flushed at event lines and at most every 100 ms, not after every line (one
            // write syscall per trapped access; CARAXES_TRACE_FLUSH=0 until 2026-09-30).
            if text.starts_with('E') || self.last_flush.elapsed().as_millis() >= 100 {
                let _ = out.flush();
                self.last_flush = Instant::now();
            }
        }
    }

    /// Whether this trace currently has an event sink, independently of access filters.
    pub fn records_events(&self) -> bool {
        self.out.is_some()
    }

    /// Whether a trapped access of `region` at `offset` is written to the trace (the reads
    /// answered without the device lock bypass it, so they are off for such registers;
    /// fastread.rs).
    pub fn records_access(&self, region: u32, offset: u64) -> bool {
        self.out.is_some() && self.accesses.wants(region, offset)
    }

    pub fn access(&mut self, dir: Dir, region: u32, offset: u64, data: &[u8], note: Option<&str>) {
        let r = region as usize;
        if r < 9 {
            match dir {
                Dir::Read => self.reads[r] += 1,
                Dir::Write => self.writes[r] += 1,
            }
        }
        if region == 0 && note.is_none() {
            *self.unnamed.entry(offset & !3).or_insert(0) += 1;
        }
        if region == 0 {
            let e = self.bar0.entry((offset & !3, dir == Dir::Write)).or_insert_with(|| {
                // The register's name (the note's first word), kept once per offset.
                (0, note.and_then(|n| n.split_whitespace().next()).unwrap_or("").to_string())
            });
            e.0 += 1;
        }
        if region == 1 || region == 3 {
            *self.by_mib.entry((region, offset >> 20, dir == Dir::Write)).or_insert(0) += 1;
        }
        if !self.accesses.wants(region, offset) || self.out.is_none() {
            return;
        }
        let name = REGION_NAMES.get(r).copied().unwrap_or("?");
        let d = match dir {
            Dir::Read => 'R',
            Dir::Write => 'W',
        };
        let text = format!(
            "{d} {name:<4} {offset:#010x} {:<2} {:<12} {}",
            data.len(),
            value(data),
            note.unwrap_or("")
        );
        self.line(text.trim_end());
    }

    pub fn event(&mut self, text: &str) {
        if !self.records_events() {
            return;
        }
        self.line(&format!("E {text}"));
    }

    /// Several events with one flush at the end (the fast ring path, launch.rs).
    pub fn events(&mut self, texts: &[String]) {
        if !self.records_events() {
            return;
        }
        let t = self.start.elapsed().as_micros();
        self.space_guard();
        if let Some(out) = &mut self.out {
            for text in texts {
                let _ = writeln!(out, "{t:>12} E {text}");
            }
            if !texts.is_empty() {
                let _ = out.flush();
                self.last_flush = Instant::now();
            }
        }
    }

    pub fn summary(&self) -> String {
        let mut s = String::new();
        for (i, n) in REGION_NAMES.iter().enumerate() {
            if self.reads[i] + self.writes[i] > 0 {
                s.push_str(&format!(
                    "{n}: {} reads, {} writes\n",
                    self.reads[i], self.writes[i]
                ));
            }
        }
        for (region, name) in [(1u32, "bar1"), (3, "bar3")] {
            let mut v: Vec<((u64, bool), u64)> = self
                .by_mib
                .iter()
                .filter(|((r, _, _), _)| *r == region)
                .map(|((_, m, w), n)| ((*m, *w), *n))
                .collect();
            if v.is_empty() {
                continue;
            }
            v.sort_by(|a, b| b.1.cmp(&a.1));
            let top: Vec<String> = v
                .iter()
                .take(16)
                .map(|((m, w), n)| format!("{m} MiB {}:{n}", if *w { "W" } else { "R" }))
                .collect();
            s.push_str(&format!(
                "{name} trapped accesses by MiB of the BAR (busiest first, of {} MiB-direction pairs): {}\n",
                v.len(),
                top.join(" ")
            ));
        }
        if !self.bar0.is_empty() {
            let mut v: Vec<_> = self.bar0.iter().collect();
            v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
            let top: Vec<String> = v
                .iter()
                .take(24)
                .map(|((off, w), (n, name))| {
                    format!("{off:#x} {}{}{}:{n}", if *w { "W" } else { "R" }, if name.is_empty() { "" } else { " " }, name)
                })
                .collect();
            s.push_str(&format!(
                "bar0 trapped accesses by offset (busiest first, of {} offset-direction pairs): {}\n",
                v.len(),
                top.join(", ")
            ));
        }
        if !self.unnamed.is_empty() {
            s.push_str("bar0 offsets without a name in the register map (offset: accesses):\n");
            for (off, n) in self.unnamed.iter().take(64) {
                s.push_str(&format!("  {off:#010x}: {n}\n"));
            }
            if self.unnamed.len() > 64 {
                s.push_str(&format!("  ... {} more\n", self.unnamed.len() - 64));
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::Accesses;

    /// The free-space guard reads the trace file's filesystem and keeps tracing while there is
    /// room (the stop below `TRACE_MIN_FREE` needs a nearly full filesystem to show).
    #[test]
    fn the_space_guard_keeps_tracing_with_room() {
        let p = std::env::temp_dir().join(format!("caraxes-trace-guard-{}", std::process::id()));
        let mut t = super::Trace::new(Some(&p)).unwrap();
        let free = super::free_bytes(t.out.as_ref().unwrap().get_ref());
        assert!(free.is_some());
        t.last_space_check = std::time::Instant::now() - std::time::Duration::from_secs(2);
        t.event("one");
        assert_eq!(t.out.is_some(), free.unwrap() >= super::TRACE_MIN_FREE);
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn event_sink_is_independent_of_access_filter_and_tracks_stop() {
        let p = std::env::temp_dir().join(format!(
            "caraxes-trace-events-{}",
            std::process::id()
        ));
        let mut trace = super::Trace::new(Some(&p)).unwrap();
        trace.accesses = Accesses::parse(Some("0")).unwrap();
        assert!(trace.records_events());
        assert!(!trace.records_access(0, 0x110008));
        trace.out = None;
        assert!(!trace.records_events());
        assert!(!super::Trace::new(None).unwrap().records_events());
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn events_keep_order_and_flush_when_accesses_are_filtered() {
        let p = std::env::temp_dir().join(format!(
            "caraxes-trace-event-order-{}", std::process::id()
        ));
        let mut trace = super::Trace::new(Some(&p)).unwrap();
        trace.accesses = Accesses::None;
        trace.access(super::Dir::Write, 0, 0x110008, &[0; 4], None);
        trace.event("first");
        assert_eq!(std::fs::read_to_string(&p).unwrap().trim_start()
            .split_once(' ').unwrap().1, "E first\n");
        trace.events(&["second".into(), "ERROR third".into()]);
        // Read before dropping the writer: both event methods must flush.
        let contents = std::fs::read_to_string(&p).unwrap();
        let lines: Vec<_> = contents.lines()
            .map(|line| line.trim_start().split_once(' ').unwrap().1)
            .collect();
        assert_eq!(lines, ["E first", "E second", "E ERROR third"]);
        assert!(trace.summary().contains("bar0: 0 reads, 1 writes"));
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn absent_or_stopped_event_sink_preserves_access_counts() {
        let p = std::env::temp_dir().join(format!(
            "caraxes-trace-event-stop-{}", std::process::id()
        ));
        for stopped in [false, true] {
            let mut trace = super::Trace::new(if stopped { Some(&p) } else { None }).unwrap();
            trace.out = None;
            trace.access(super::Dir::Read, 1, 0x20_0000, &[0; 4], None);
            trace.access(super::Dir::Write, 0, 0x9400, &[0; 4], None);
            let before = trace.summary();
            trace.last_space_check = std::time::Instant::now()
                - std::time::Duration::from_secs(2);
            let check = trace.last_space_check;
            let flush = trace.last_flush;
            trace.event("unused");
            trace.events(&["unused batch".into(), "ERROR unused".into()]);
            assert_eq!(trace.summary(), before);
            assert_eq!(trace.last_space_check, check);
            assert_eq!(trace.last_flush, flush);
            assert!(!trace.records_events());
        }
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "");
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn bar_accesses_are_counted_per_mib() {
        let mut t = super::Trace::new(None).unwrap();
        for _ in 0..3 {
            t.access(super::Dir::Write, 1, 0x0230_0004, &[0], None);
        }
        t.access(super::Dir::Read, 3, 0x10, &[0; 4], None);
        t.access(super::Dir::Read, 0, 0x0011_0008, &[0; 4], Some("GSP_IRQSTAT captured"));
        t.access(super::Dir::Read, 0, 0x0011_0008, &[0; 4], Some("GSP_IRQSTAT"));
        t.access(super::Dir::Write, 0, 0x9402, &[0; 2], None);
        let s = t.summary();
        assert!(
            s.contains("bar0 trapped accesses by offset (busiest first, of 2 offset-direction pairs): 0x110008 R GSP_IRQSTAT:2, 0x9400 W:1"),
            "{s}"
        );
        assert!(s.contains("bar1 trapped accesses by MiB of the BAR (busiest first, of 1 MiB-direction pairs): 35 MiB W:3"), "{s}");
        assert!(s.contains("bar3 trapped accesses by MiB of the BAR (busiest first, of 1 MiB-direction pairs): 0 MiB R:1"), "{s}");
    }

    #[test]
    fn access_filter_parses_and_selects() {
        assert_eq!(Accesses::parse(None).unwrap(), Accesses::All);
        assert_eq!(Accesses::parse(Some("0")).unwrap(), Accesses::None);
        let a = Accesses::parse(Some("bar0:0x610000-0x700000")).unwrap();
        assert_eq!(a, Accesses::Bar0(0x610000, 0x700000));
        assert!(a.wants(0, 0x611d80) && !a.wants(0, 0x700000) && !a.wants(1, 0x611d80));
        assert!(Accesses::parse(Some("bar0:0x10-0x10")).is_err());
        assert!(Accesses::parse(Some("yes")).is_err());
    }
}

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
    /// `CARAXES_TRACE_ACCESSES=0`: count the region accesses but write no line per
    /// access (events are still written). Every trapped access otherwise formats and
    /// writes one line on the guest's MMIO round trip (docs/caraxes/mapping.md §6).
    /// `CARAXES_TRACE_ACCESSES=bar0:<lo>-<hi>` writes the lines of BAR0 accesses in
    /// [lo, hi) only (a diagnostic, e.g. the display block 0x610000-0x700000).
    accesses: Accesses,
    /// `CARAXES_TRACE_FLUSH=0`: flush at event lines and at most every 100 ms instead of
    /// after every line (one write syscall per trapped access).
    flush_each: bool,
    last_flush: Instant,
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
            accesses: Accesses::parse(std::env::var("CARAXES_TRACE_ACCESSES").ok().as_deref())?,
            flush_each: std::env::var("CARAXES_TRACE_FLUSH").as_deref() != Ok("0"),
            last_flush: Instant::now(),
        })
    }

    fn line(&mut self, text: &str) {
        let t = self.start.elapsed().as_micros();
        if let Some(out) = &mut self.out {
            let _ = writeln!(out, "{t:>12} {text}");
            if self.flush_each || text.starts_with('E') || self.last_flush.elapsed().as_millis() >= 100 {
                let _ = out.flush();
                self.last_flush = Instant::now();
            }
        }
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
        self.line(&format!("E {text}"));
    }

    /// Several events with one flush at the end (the fast ring path, launch.rs).
    pub fn events(&mut self, texts: &[String]) {
        let t = self.start.elapsed().as_micros();
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

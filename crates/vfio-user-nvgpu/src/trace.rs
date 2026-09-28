// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
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

pub struct Trace {
    out: Option<BufWriter<File>>,
    start: Instant,
    reads: [u64; 9],
    writes: [u64; 9],
    unnamed: BTreeMap<u64, u64>,
    /// `CARAXES_TRACE_ACCESSES=0`: count the region accesses but write no line per
    /// access (events are still written). Every trapped access otherwise formats and
    /// writes one line on the guest's MMIO round trip (docs/caraxes/mapping.md §6).
    accesses: bool,
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
            accesses: std::env::var("CARAXES_TRACE_ACCESSES").as_deref() != Ok("0"),
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
        if !self.accesses || self.out.is_none() {
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

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The profile (`CARAXES_PROFILE=1`, docs/caraxes/coverage-map.md §1): what the device did,
//! aggregated in memory instead of written line by line as the trace does (trace.rs).
//!
//! One row per key, with a count, the device's wall time, the largest single time, a status
//! histogram and the answer's label:
//! - trapped accesses by region, direction, page and register (BAR1 and BAR2 by page only),
//!   with the device time from the vfio-user message to its answer (the lock wait included)
//!   and the guest-seen cost as rpc-cost.py attributes it: the time since the previous
//!   trapped access ended, when shorter than [`IDLE_GAP`];
//! - GSP RPCs by function, controls by command, allocations by class (device time from the
//!   request to the answer), and the unsolicited messages by function;
//! - host RM calls by escape, with the control command or allocated class (every
//!   `NvDevice::ioctl_raw` of the process, nvrm-abi's observer), and the shadow's calls by
//!   rpc-cost.py's kinds (shadow.rs `Call`);
//! - window hits: the token pool's rings, the doorbell and re-arm eventfds, the BAR2 memfd's
//!   syncs and walks, the BAR1 pool's moves and barriers, and the interrupt and display
//!   pages' scans and applied stores (read from their counters, `register_source`).
//!
//! Scopes: the device lifetime, and each guest RM client from its root allocation
//! (`GSP_RM_ALLOC` of class `NV01_ROOT`) to its free (`FREE` of the client handle) — in the
//! stock driver one per process that opens the GPU. A client's rows are the device rows
//! at its free minus those at its allocation, so they hold everything the device did while
//! the client lived, other clients' work included.
//!
//! Dumped as TSV on SIGUSR2 (`<prefix>.<n>.tsv`) and at exit (`<prefix>.exit.tsv`);
//! `caraxes/tools/profile-rank.py` ranks and joins the rows. With the profile off every
//! hook is one relaxed atomic load.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The switch.
pub const PROFILE_ENV: &str = "CARAXES_PROFILE";
/// The dump prefix (default: `gpu-profile` beside the trace file, else in the working
/// directory).
pub const PROFILE_OUT_ENV: &str = "CARAXES_PROFILE_OUT";
/// A gap between two trapped accesses longer than this is idle time, not the cost of the
/// later access (rpc-cost.py `--idle-ms 2`).
pub const IDLE_GAP: Duration = Duration::from_millis(2);
/// Closed client lifetimes kept; older ones are dropped and counted.
pub const MAX_CLOSED: usize = 512;
/// Distinct statuses kept per row; further ones are counted under [`STATUS_OTHER`].
pub const MAX_STATUS: usize = 8;
/// The histogram bucket of statuses beyond [`MAX_STATUS`].
pub const STATUS_OTHER: u32 = 0xFFFF_FFFE;
/// The register of a page-level row (BAR1 and BAR2 accesses, and a page's registers beyond
/// [`MAX_REGS_PER_PAGE`]).
pub const PAGE_ROW: u64 = u64::MAX;
/// Distinct register rows kept per BAR0 or config page and direction.
pub const MAX_REGS_PER_PAGE: u32 = 64;

/// What a row counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    /// A trapped access: vfio region index, write or read, 4 KiB page, and the register
    /// (the dword offset; [`PAGE_ROW`] for BAR1 and BAR2).
    Access {
        region: u32,
        write: bool,
        page: u64,
        reg: u64,
    },
    /// A GSP RPC by function number, from its receipt to its reply.
    Rpc(u32),
    /// A `GSP_RM_CONTROL` by command.
    Control(u32),
    /// A `GSP_RM_ALLOC` by class.
    Alloc(u32),
    /// An unsolicited message (event or GMCAPI request) by function or id.
    Event(u32),
    /// A host RM ioctl: escape number and key (control command, allocated class, or 0).
    Host { nr: u32, key: u32 },
    /// A host RM call of the shadow, by rpc-cost.py's kind (shadow.rs `CALL_NAMES`).
    Shadow(&'static str),
    /// A window hit or a window's own work.
    Window(&'static str),
}

impl Key {
    fn kind(&self) -> &'static str {
        match self {
            Key::Access { .. } => "access",
            Key::Rpc(_) => "rpc",
            Key::Control(_) => "control",
            Key::Alloc(_) => "alloc",
            Key::Event(_) => "event",
            Key::Host { .. } => "host",
            Key::Shadow(_) => "shadow",
            Key::Window(_) => "window",
        }
    }

    /// The key column of the TSV.
    fn column(&self) -> String {
        match *self {
            Key::Access {
                region,
                write,
                page,
                reg,
            } => {
                let name = crate::trace::REGION_NAMES
                    .get(region as usize)
                    .copied()
                    .unwrap_or("?");
                let reg = if reg == PAGE_ROW {
                    "-".to_string()
                } else {
                    format!("{reg:#x}")
                };
                format!("{name}:{}:{page:#x}:{reg}", if write { 'W' } else { 'R' })
            }
            Key::Rpc(f) | Key::Event(f) => format!("{f:#x}"),
            Key::Control(c) => format!("{c:#010x}"),
            Key::Alloc(c) => format!("{c:#x}"),
            Key::Host { nr, key } => format!("{nr:#x}:{key:#x}"),
            Key::Shadow(s) | Key::Window(s) => s.to_string(),
        }
    }
}

/// One row's aggregate.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub count: u64,
    /// Device wall time, nanoseconds.
    pub ns: u64,
    pub max_ns: u64,
    /// Accesses: the guest-seen cost (time since the previous trapped access ended, when
    /// under [`IDLE_GAP`]) and how many accesses it covers.
    pub gap_ns: u64,
    pub gaps: u64,
    /// A row's own unit (guest writes of an eventfd ring, pages of a sync, bytes moved).
    pub units: u64,
    /// Status histogram (status, count), at most [`MAX_STATUS`] entries.
    pub status: Vec<(u32, u64)>,
    /// The register name or the answer's label, as first seen.
    pub label: &'static str,
}

impl Cell {
    fn add(&mut self, ns: u64, units: u64, status: Option<u32>, label: &'static str) {
        self.count += 1;
        self.ns += ns;
        self.max_ns = self.max_ns.max(ns);
        self.units += units;
        if self.label.is_empty() {
            self.label = label;
        }
        if let Some(st) = status {
            let at = |v: &Vec<(u32, u64)>, s: u32| v.iter().position(|e| e.0 == s);
            if let Some(i) = at(&self.status, st) {
                self.status[i].1 += 1;
            } else if self.status.len() < MAX_STATUS {
                self.status.push((st, 1));
            } else if let Some(i) = at(&self.status, STATUS_OTHER) {
                self.status[i].1 += 1;
            } else {
                // Fold the last bucket into the overflow bucket.
                let last = self.status.pop().map_or(0, |e| e.1);
                self.status.push((STATUS_OTHER, last + 1));
            }
        }
    }

    /// `self - base` (the counts since `base`); `max_ns` is the whole lifetime's.
    fn since(&self, base: &Cell) -> Cell {
        let status = self
            .status
            .iter()
            .filter_map(|&(s, n)| {
                let b = base.status.iter().find(|e| e.0 == s).map_or(0, |e| e.1);
                (n > b).then_some((s, n - b))
            })
            .collect();
        Cell {
            count: self.count.saturating_sub(base.count),
            ns: self.ns.saturating_sub(base.ns),
            max_ns: self.max_ns,
            gap_ns: self.gap_ns.saturating_sub(base.gap_ns),
            gaps: self.gaps.saturating_sub(base.gaps),
            units: self.units.saturating_sub(base.units),
            status,
            label: self.label,
        }
    }

    fn is_empty(&self) -> bool {
        self.count == 0 && self.units == 0
    }
}

/// A guest RM client's lifetime and what the device did during it.
#[derive(Clone, Debug)]
pub struct Lifetime {
    pub h_client: u32,
    pub from_us: u64,
    pub to_us: u64,
    pub rows: Vec<(Key, Cell)>,
}

/// The aggregate (the global one behind [`record`], and one per test).
pub struct Profile {
    start: Instant,
    rows: HashMap<Key, Cell>,
    last_access: Option<Instant>,
    regs_per_page: HashMap<(u32, bool, u64), u32>,
    open: BTreeMap<u32, (u64, HashMap<Key, Cell>)>,
    closed: VecDeque<Lifetime>,
    dropped: u64,
}

impl Default for Profile {
    fn default() -> Self {
        Self::new()
    }
}

impl Profile {
    pub fn new() -> Self {
        Profile {
            start: Instant::now(),
            rows: HashMap::new(),
            last_access: None,
            regs_per_page: HashMap::new(),
            open: BTreeMap::new(),
            closed: VecDeque::new(),
            dropped: 0,
        }
    }

    fn us(&self, at: Instant) -> u64 {
        at.saturating_duration_since(self.start).as_micros() as u64
    }

    /// Count one `key` with `dur` of device time.
    pub fn record(
        &mut self,
        key: Key,
        dur: Duration,
        units: u64,
        status: Option<u32>,
        label: &'static str,
    ) {
        self.rows
            .entry(key)
            .or_default()
            .add(dur.as_nanos() as u64, units, status, label);
    }

    /// Set a counter row to an absolute value (a window's own counter, `register_source`).
    pub fn set(&mut self, key: Key, count: u64) {
        self.rows.entry(key).or_default().count = count;
    }

    /// A trapped access that arrived at `t0` and was answered at `now`.
    pub fn access(
        &mut self,
        region: u32,
        write: bool,
        offset: u64,
        name: Option<&'static str>,
        t0: Instant,
        now: Instant,
    ) {
        let page = offset & !0xFFF;
        let mut reg = if region == 1 || region == 3 {
            PAGE_ROW
        } else {
            offset & !3
        };
        if reg != PAGE_ROW {
            // A page walked register by register (the PROM window at boot, 33k BAR0 dwords)
            // folds into one page row after MAX_REGS_PER_PAGE registers, so that the rows,
            // and the client snapshots that copy them, stay small.
            let known = self.rows.contains_key(&Key::Access {
                region,
                write,
                page,
                reg,
            });
            if !known {
                let n = self.regs_per_page.entry((region, write, page)).or_insert(0);
                if *n >= MAX_REGS_PER_PAGE {
                    reg = PAGE_ROW;
                } else {
                    *n += 1;
                }
            }
        }
        let gap = self
            .last_access
            .map(|l| now.saturating_duration_since(l))
            .filter(|g| *g <= IDLE_GAP);
        self.last_access = Some(now);
        let c = self
            .rows
            .entry(Key::Access {
                region,
                write,
                page,
                reg,
            })
            .or_default();
        c.add(
            now.saturating_duration_since(t0).as_nanos() as u64,
            0,
            None,
            name.unwrap_or(""),
        );
        if let Some(g) = gap {
            c.gap_ns += g.as_nanos() as u64;
            c.gaps += 1;
        }
    }

    /// A guest client was allocated (its root object): its lifetime starts.
    pub fn client_open(&mut self, h_client: u32, now: Instant) {
        let snap = self.rows.clone();
        self.open.insert(h_client, (self.us(now), snap));
    }

    /// The guest freed a client: its lifetime ends. A client never opened is ignored.
    pub fn client_close(&mut self, h_client: u32, now: Instant) {
        let Some((from_us, base)) = self.open.remove(&h_client) else {
            return;
        };
        let rows = diff(&self.rows, &base);
        self.closed.push_back(Lifetime {
            h_client,
            from_us,
            to_us: self.us(now),
            rows,
        });
        while self.closed.len() > MAX_CLOSED {
            self.closed.pop_front();
            self.dropped += 1;
        }
    }

    /// The device rows, sorted.
    pub fn rows(&self) -> Vec<(Key, Cell)> {
        let mut v: Vec<(Key, Cell)> = self
            .rows
            .iter()
            .filter(|(_, c)| !c.is_empty())
            .map(|(k, c)| (*k, c.clone()))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    pub fn closed(&self) -> impl Iterator<Item = &Lifetime> {
        self.closed.iter()
    }

    /// The TSV: a comment header, the column line, the device scope, the closed client
    /// lifetimes, then the open ones up to `now`.
    pub fn tsv(&self, reason: &str, now: Instant) -> String {
        let mut s = String::new();
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let _ = writeln!(
            s,
            "# SPDX-License-Identifier: MIT\n# vfio-user-nvgpu profile ({PROFILE_ENV}=1, src/profile.rs): {reason}; device time {} us; unix time {wall}; pid {}; closed client lifetimes {} (dropped {}), open {}",
            self.us(now),
            std::process::id(),
            self.closed.len(),
            self.dropped,
            self.open.len()
        );
        s.push_str("scope\tfrom_us\tto_us\tkind\tkey\tname\tcount\ttotal_ns\tmax_ns\tgap_ns\tgaps\tunits\tstatus\tlabel\n");
        let to = self.us(now);
        push_rows(&mut s, "device", 0, to, &self.rows());
        for l in &self.closed {
            push_rows(
                &mut s,
                &format!("client:{:#x}", l.h_client),
                l.from_us,
                l.to_us,
                &l.rows,
            );
        }
        for (h, (from, base)) in &self.open {
            push_rows(
                &mut s,
                &format!("client:{h:#x}:open"),
                *from,
                to,
                &diff(&self.rows, base),
            );
        }
        s
    }
}

fn diff(now: &HashMap<Key, Cell>, base: &HashMap<Key, Cell>) -> Vec<(Key, Cell)> {
    let empty = Cell::default();
    let mut v: Vec<(Key, Cell)> = now
        .iter()
        .map(|(k, c)| (*k, c.since(base.get(k).unwrap_or(&empty))))
        .filter(|(_, c)| !c.is_empty())
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn push_rows(s: &mut String, scope: &str, from: u64, to: u64, rows: &[(Key, Cell)]) {
    for (k, c) in rows {
        let status = if c.status.is_empty() {
            "-".to_string()
        } else {
            c.status
                .iter()
                .map(|(st, n)| format!("{st:#x}={n}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        let _ = writeln!(
            s,
            "{scope}\t{from}\t{to}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{status}\t{}",
            k.kind(),
            k.column(),
            name_of(k),
            c.count,
            c.ns,
            c.max_ns,
            c.gap_ns,
            c.gaps,
            c.units,
            if c.label.is_empty() { "-" } else { c.label }
        );
    }
}

/// A readable name for a key where the device knows one.
fn name_of(k: &Key) -> String {
    match *k {
        Key::Rpc(f) | Key::Event(f) => crate::rpc::function_name(f).to_string(),
        Key::Host { nr, .. } => escape_name(nr).to_string(),
        _ => "-".to_string(),
    }
}

/// The name of a host RM escape number.
pub fn escape_name(nr: u32) -> &'static str {
    use nvrm_abi::nvgpu as g;
    use nvrm_abi::sys as s;
    match nr {
        s::NV_ESC_RM_ALLOC_MEMORY => "RM_ALLOC_MEMORY",
        s::NV_ESC_RM_ALLOC_OBJECT => "RM_ALLOC_OBJECT",
        s::NV_ESC_RM_FREE => "RM_FREE",
        s::NV_ESC_RM_CONTROL => "RM_CONTROL",
        s::NV_ESC_RM_ALLOC => "RM_ALLOC",
        s::NV_ESC_RM_DUP_OBJECT => "RM_DUP_OBJECT",
        s::NV_ESC_RM_SHARE => "RM_SHARE",
        s::NV_ESC_RM_VID_HEAP_CONTROL => "RM_VID_HEAP_CONTROL",
        s::NV_ESC_RM_MAP_MEMORY => "RM_MAP_MEMORY",
        s::NV_ESC_RM_UNMAP_MEMORY => "RM_UNMAP_MEMORY",
        s::NV_ESC_RM_MAP_MEMORY_DMA => "RM_MAP_MEMORY_DMA",
        s::NV_ESC_RM_UNMAP_MEMORY_DMA => "RM_UNMAP_MEMORY_DMA",
        s::NV_ESC_RM_BIND_CONTEXT_DMA => "RM_BIND_CONTEXT_DMA",
        s::NV_ESC_RM_GET_EVENT_DATA => "RM_GET_EVENT_DATA",
        g::NV_ESC_CARD_INFO => "CARD_INFO",
        g::NV_ESC_REGISTER_FD => "REGISTER_FD",
        g::NV_ESC_ALLOC_OS_EVENT => "ALLOC_OS_EVENT",
        g::NV_ESC_FREE_OS_EVENT => "FREE_OS_EVENT",
        g::NV_ESC_CHECK_VERSION_STR => "CHECK_VERSION_STR",
        g::NV_ESC_ATTACH_GPUS_TO_FD => "ATTACH_GPUS_TO_FD",
        g::NV_ESC_SYS_PARAMS => "SYS_PARAMS",
        _ => "other",
    }
}

// The global profile.

static ON: AtomicBool = AtomicBool::new(false);
static PROFILE: Mutex<Option<Profile>> = Mutex::new(None);
static OUT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// A window counter read at every snapshot and dump: (row name, current value) pairs from
/// atomics only (it runs with the device lock held).
pub type Source = Box<dyn Fn() -> Vec<(&'static str, u64)> + Send>;
static SOURCES: Mutex<Vec<Source>> = Mutex::new(Vec::new());

/// Whether the profile is on (one relaxed load).
#[inline]
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

fn global() -> MutexGuard<'static, Option<Profile>> {
    PROFILE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whether `CARAXES_PROFILE=1` is set.
pub fn requested() -> bool {
    std::env::var(PROFILE_ENV).is_ok_and(|v| v.trim() == "1")
}

/// Turn the profile on with dumps at `prefix`, install the host ioctl observer, and block
/// SIGUSR2 in the calling thread (call it before any other thread exists, so that every
/// thread inherits the mask and only [`spawn_dump_thread`]'s `sigwait` takes the signal).
pub fn start(prefix: PathBuf) -> std::io::Result<()> {
    // SAFETY: sigset manipulation on a local set; pthread_sigmask on the calling thread.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGUSR2);
        if libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) != 0 {
            return Err(std::io::Error::other("pthread_sigmask(SIGUSR2)"));
        }
    }
    *global() = Some(Profile::new());
    *OUT.lock().unwrap_or_else(PoisonError::into_inner) = Some(prefix);
    nvrm_abi::set_ioctl_observer(host_ioctl);
    ON.store(true, Ordering::Relaxed);
    Ok(())
}

/// The dump prefix: `CARAXES_PROFILE_OUT`, else `gpu-profile` beside `trace`, else in the
/// working directory.
pub fn prefix(trace: Option<&Path>) -> PathBuf {
    if let Ok(p) = std::env::var(PROFILE_OUT_ENV) {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    match trace.and_then(Path::parent) {
        Some(d) if !d.as_os_str().is_empty() => d.join("gpu-profile"),
        _ => PathBuf::from("gpu-profile"),
    }
}

/// The thread that dumps on every SIGUSR2 (`<prefix>.<n>.tsv`, n from 1).
pub fn spawn_dump_thread() -> std::io::Result<()> {
    // SAFETY: a local set; the signal is blocked in every thread (`start`).
    let set = unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGUSR2);
        set
    };
    std::thread::Builder::new()
        .name("profile".into())
        .spawn(move || {
            let mut n = 0u32;
            loop {
                let mut sig = 0;
                // SAFETY: sigwait on the set blocked in `start`.
                if unsafe { libc::sigwait(&set, &mut sig) } != 0 {
                    continue;
                }
                n += 1;
                match dump(&n.to_string(), "SIGUSR2") {
                    Ok(p) => eprintln!("vfio-user-nvgpu: profile: dump {n} to {}", p.display()),
                    Err(e) => eprintln!("vfio-user-nvgpu: profile: dump {n}: {e}"),
                }
            }
        })?;
    Ok(())
}

/// Write `<prefix>.<tag>.tsv` (to a temporary name first, then renamed, so a reader never
/// sees half a dump).
pub fn dump(tag: &str, reason: &str) -> std::io::Result<PathBuf> {
    let prefix = OUT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or_else(|| std::io::Error::other("profile off"))?;
    refresh_sources();
    let text = match global().as_ref() {
        Some(p) => p.tsv(reason, Instant::now()),
        None => return Err(std::io::Error::other("profile off")),
    };
    let path = PathBuf::from(format!("{}.{tag}.tsv", prefix.display()));
    let tmp = PathBuf::from(format!("{}.{tag}.tsv.tmp", prefix.display()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// The exit dump (`<prefix>.exit.tsv`), if the profile is on.
pub fn dump_at_exit() {
    if !on() {
        return;
    }
    match dump("exit", "exit") {
        Ok(p) => eprintln!("vfio-user-nvgpu: profile: exit dump to {}", p.display()),
        Err(e) => eprintln!("vfio-user-nvgpu: profile: exit dump: {e}"),
    }
}

/// Add a window counter source (see [`Source`]).
pub fn register_source(s: Source) {
    SOURCES.lock().unwrap_or_else(PoisonError::into_inner).push(s);
}

fn refresh_sources() {
    let values: Vec<(&'static str, u64)> = SOURCES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .flat_map(|s| s())
        .collect();
    if let Some(p) = global().as_mut() {
        for (name, v) in values {
            p.set(Key::Window(name), v);
        }
    }
}

/// Count `key` in the global profile.
pub fn record(key: Key, dur: Duration, units: u64, status: Option<u32>, label: &'static str) {
    if !on() {
        return;
    }
    if let Some(p) = global().as_mut() {
        p.record(key, dur, units, status, label);
    }
}

/// A trapped access (device.rs `SharedDevice`).
pub fn access(region: u32, write: bool, offset: u64, name: Option<&'static str>, t0: Instant) {
    if let Some(p) = global().as_mut() {
        p.access(region, write, offset, name, t0, Instant::now());
    }
}

/// A window hit with no time of its own.
#[inline]
pub fn window(name: &'static str, units: u64) {
    if on() {
        record(Key::Window(name), Duration::ZERO, units, None, "");
    }
}

/// A guest client's root allocation (boot.rs, `GSP_RM_ALLOC` of class `NV01_ROOT`).
pub fn client_open(h_client: u32) {
    if !on() {
        return;
    }
    refresh_sources();
    if let Some(p) = global().as_mut() {
        p.client_open(h_client, Instant::now());
    }
}

/// A guest client's free (boot.rs, `FREE` of the client handle).
pub fn client_close(h_client: u32) {
    if !on() {
        return;
    }
    refresh_sources();
    if let Some(p) = global().as_mut() {
        p.client_close(h_client, Instant::now());
    }
}

/// nvrm-abi's observer: every host RM ioctl of the process.
fn host_ioctl(nr: u32, key: u32, status: u32, dur: Duration) {
    record(Key::Host { nr, key }, dur, 0, Some(status), "");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn record_counts_time_max_status_and_first_label() {
        let mut p = Profile::new();
        p.record(Key::Control(0x2080_0101), ms(2), 0, Some(0), "synthesized");
        p.record(Key::Control(0x2080_0101), ms(3), 0, Some(0x56), "unanswered");
        p.record(Key::Control(0x2080_0101), ms(1), 0, Some(0), "x");
        let rows = p.rows();
        assert_eq!(rows.len(), 1);
        let c = &rows[0].1;
        assert_eq!(c.count, 3);
        assert_eq!(c.ns, 6_000_000);
        assert_eq!(c.max_ns, 3_000_000);
        assert_eq!(c.status, vec![(0, 2), (0x56, 1)]);
        assert_eq!(c.label, "synthesized");
    }

    #[test]
    fn status_histogram_is_bounded() {
        let mut p = Profile::new();
        for st in 0..20u32 {
            p.record(Key::Rpc(10), ms(0), 0, Some(st), "");
        }
        let c = &p.rows()[0].1;
        assert!(c.status.len() <= MAX_STATUS);
        assert_eq!(c.status.iter().map(|e| e.1).sum::<u64>(), 20);
        assert!(c.status.iter().any(|e| e.0 == STATUS_OTHER));
    }

    #[test]
    fn accesses_key_bar0_by_register_and_bar2_by_page_with_gaps() {
        let mut p = Profile::new();
        let t = Instant::now();
        p.access(0, true, 0xb830a0, Some("PRIV_MMU_INVALIDATE"), t, t + ms(0));
        p.access(0, true, 0xb830a2, None, t, t + Duration::from_micros(30));
        p.access(3, true, 0x10_0010, None, t, t + Duration::from_micros(60));
        p.access(3, true, 0x10_0ff8, None, t, t + Duration::from_micros(90));
        // After an idle stretch: no gap attributed.
        p.access(3, false, 0xfea000, None, t, t + ms(50));
        let rows = p.rows();
        let bar0 = rows
            .iter()
            .find(|(k, _)| matches!(k, Key::Access { region: 0, .. }))
            .unwrap();
        assert_eq!(bar0.1.count, 2, "0xb830a0 and 0xb830a2 are one dword");
        assert_eq!(bar0.1.label, "PRIV_MMU_INVALIDATE");
        assert_eq!(bar0.1.gaps, 1, "the first access has no predecessor");
        assert_eq!(bar0.1.gap_ns, 30_000);
        let bar2w = rows
            .iter()
            .find(|(k, _)| {
                *k == Key::Access {
                    region: 3,
                    write: true,
                    page: 0x10_0000,
                    reg: PAGE_ROW,
                }
            })
            .unwrap();
        assert_eq!(bar2w.1.count, 2);
        assert_eq!(bar2w.1.gap_ns, 60_000);
        let flush = rows
            .iter()
            .find(|(k, _)| matches!(k, Key::Access { region: 3, write: false, .. }))
            .unwrap();
        assert_eq!((flush.1.count, flush.1.gaps), (1, 0));
    }

    #[test]
    fn a_page_walked_register_by_register_folds_into_a_page_row() {
        let mut p = Profile::new();
        let t = Instant::now();
        for i in 0..1000u64 {
            p.access(0, false, 0x30_0000 + 4 * i, None, t, t);
        }
        // Registers already known keep their rows.
        p.access(0, false, 0x30_0000, None, t, t);
        let rows = p.rows();
        assert_eq!(rows.len(), MAX_REGS_PER_PAGE as usize + 1);
        let page = rows
            .iter()
            .find(|(k, _)| matches!(k, Key::Access { reg: PAGE_ROW, .. }))
            .unwrap();
        assert_eq!(page.1.count, 1000 - MAX_REGS_PER_PAGE as u64);
        assert_eq!(rows.iter().map(|(_, c)| c.count).sum::<u64>(), 1001);
    }

    #[test]
    fn client_lifetime_is_the_difference_between_free_and_alloc() {
        let mut p = Profile::new();
        let t = Instant::now();
        p.record(Key::Control(1), ms(1), 0, Some(0), "");
        p.client_open(0xc1d0_0009, t);
        p.record(Key::Control(1), ms(2), 0, Some(0x56), "");
        p.record(Key::Alloc(0xc46f), ms(4), 0, Some(0), "");
        p.set(Key::Window("tokens: eventfd rings"), 7);
        p.client_close(0xc1d0_0009, t + ms(10));
        // A free of a client never opened (the kernel's own) is ignored.
        p.client_close(0x1234, t + ms(11));
        let l: Vec<&Lifetime> = p.closed().collect();
        assert_eq!(l.len(), 1);
        let rows = &l[0].rows;
        let ctl = rows.iter().find(|(k, _)| *k == Key::Control(1)).unwrap();
        assert_eq!((ctl.1.count, ctl.1.ns), (1, 2_000_000));
        assert_eq!(ctl.1.status, vec![(0x56, 1)]);
        assert!(rows.iter().any(|(k, c)| *k == Key::Alloc(0xc46f) && c.count == 1));
        assert!(rows.iter().any(|(k, c)| *k == Key::Window("tokens: eventfd rings") && c.count == 7));
        // The device scope still holds everything.
        let dev = p.rows();
        assert_eq!(dev.iter().find(|(k, _)| *k == Key::Control(1)).unwrap().1.count, 2);
    }

    #[test]
    fn closed_lifetimes_are_bounded() {
        let mut p = Profile::new();
        let t = Instant::now();
        for h in 0..(MAX_CLOSED as u32 + 5) {
            p.client_open(h, t);
            p.record(Key::Rpc(103), ms(0), 0, None, "");
            p.client_close(h, t);
        }
        assert_eq!(p.closed().count(), MAX_CLOSED);
        assert_eq!(p.dropped, 5);
    }

    #[test]
    fn tsv_has_the_columns_and_one_line_per_row_and_scope() {
        let mut p = Profile::new();
        let t = Instant::now();
        p.access(0, false, 0x110094, Some("NV_PGSP_FALCON_MAILBOX0"), t, t);
        p.client_open(5, t);
        p.record(
            Key::Host {
                nr: nvrm_abi::sys::NV_ESC_RM_CONTROL,
                key: 0x2080_0102,
            },
            ms(1),
            0,
            Some(0),
            "",
        );
        let s = p.tsv("test", t + ms(1));
        let lines: Vec<&str> = s.lines().filter(|l| !l.starts_with('#')).collect();
        let cols = lines[0].split('\t').count();
        assert_eq!(cols, 14);
        assert!(lines.iter().all(|l| l.split('\t').count() == cols));
        assert!(lines.iter().any(|l| l.starts_with("device\t") && l.contains("bar0:R:0x110000:0x110094")));
        assert!(lines
            .iter()
            .any(|l| l.starts_with("client:0x5:open\t") && l.contains("RM_CONTROL") && l.contains("0x2a:0x20800102")));
    }
}

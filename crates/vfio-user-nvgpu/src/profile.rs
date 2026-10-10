// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub mod backend_markers;
mod host_timeline;
pub mod submission_timeline;

/// The switch.
pub const PROFILE_ENV: &str = "CARAXES_PROFILE";
/// The dump prefix (default: `gpu-profile` beside the trace file, else in the working
/// directory).
pub const PROFILE_OUT_ENV: &str = "CARAXES_PROFILE_OUT";
/// Retain selected host ioctl timestamps and backend observation markers while the
/// aggregate profile is enabled.
pub const HOST_TIMELINE_ENV: &str = "CARAXES_PROFILE_HOST_TIMELINE";
/// Retain bounded backend submission-stage spans on the host timeline clock.
pub const SUBMISSION_TIMELINE_ENV: &str = "CARAXES_PROFILE_SUBMISSION_TIMELINE";
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
    host_timeline: Option<host_timeline::Timeline>,
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
            host_timeline: None,
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

/// Whether profiling or its submission diagnostic is requested. `start` validates dependencies.
pub fn requested() -> bool {
    std::env::var(PROFILE_ENV).is_ok_and(|v| v.trim() == "1")
        || std::env::var(SUBMISSION_TIMELINE_ENV).is_ok_and(|v| v.trim() == "1")
}

/// Turn the profile on with dumps at `prefix`, install the host ioctl observer, and block
/// SIGUSR2 in the calling thread (call it before any other thread exists, so that every
/// thread inherits the mask and only [`spawn_dump_thread`]'s `sigwait` takes the signal).
pub fn start(prefix: PathBuf) -> std::io::Result<()> {
    let submission = std::env::var(SUBMISSION_TIMELINE_ENV).is_ok_and(|v| v.trim() == "1");
    let timeline = std::env::var(HOST_TIMELINE_ENV).is_ok_and(|v| v == "1");
    if submission && (!std::env::var(PROFILE_ENV).is_ok_and(|v| v.trim() == "1") || !timeline) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "CARAXES_PROFILE_SUBMISSION_TIMELINE=1 requires CARAXES_PROFILE=1 and CARAXES_PROFILE_HOST_TIMELINE=1",
        ));
    }
    // SAFETY: sigset manipulation on a local set; pthread_sigmask on the calling thread.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGUSR2);
        if libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) != 0 {
            return Err(std::io::Error::other("pthread_sigmask(SIGUSR2)"));
        }
    }
    let mut p = Profile::new();
    if timeline {
        p.host_timeline = Some(host_timeline::Timeline::new(host_timeline::CAPACITY));
        if !nvrm_abi::set_ioctl_span_observer(host_ioctl_span) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "host timeline requires the process-wide ioctl observer slot",
            ));
        }
    } else {
        nvrm_abi::set_ioctl_observer(host_ioctl);
    }
    if let Some(t) = &p.host_timeline {
        backend_markers::start(t.anchor());
        if submission {
            submission_timeline::configure(t.anchor());
        }
    }
    *global() = Some(p);
    *OUT.lock().unwrap_or_else(PoisonError::into_inner) = Some(prefix);
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
    let (text, timeline) = match global().as_ref() {
        Some(p) => (
            p.tsv(reason, Instant::now()),
            p.host_timeline.as_ref().map(|t| t.tsv(reason)),
        ),
        None => return Err(std::io::Error::other("profile off")),
    };
    // Marker writers never acquire PROFILE. Format this independent ring after
    // dropping the aggregate guard, avoiding a new nested lock order.
    let markers = backend_markers::snapshot(reason);
    let submission = submission_timeline::snapshot(reason);
    let path = PathBuf::from(format!("{}.{tag}.tsv", prefix.display()));
    let tmp = PathBuf::from(format!("{}.{tag}.tsv.tmp", prefix.display()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &path)?;
    if let Some(timeline) = timeline {
        let timeline_path = PathBuf::from(format!("{}.{tag}.host.tsv", prefix.display()));
        let timeline_tmp = PathBuf::from(format!("{}.{tag}.host.tsv.tmp", prefix.display()));
        std::fs::write(&timeline_tmp, timeline)?;
        std::fs::rename(&timeline_tmp, timeline_path)?;
    }
    if let Some(markers) = markers {
        let markers_path = PathBuf::from(format!("{}.{tag}.markers.tsv", prefix.display()));
        let markers_tmp = PathBuf::from(format!("{}.{tag}.markers.tsv.tmp", prefix.display()));
        std::fs::write(&markers_tmp, markers)?;
        std::fs::rename(&markers_tmp, markers_path)?;
    }
    if let Some(submission) = submission {
        let submission_path = PathBuf::from(format!("{}.{tag}.submission.tsv", prefix.display()));
        let submission_tmp = PathBuf::from(format!("{}.{tag}.submission.tsv.tmp", prefix.display()));
        std::fs::write(&submission_tmp, submission)?;
        std::fs::rename(&submission_tmp, submission_path)?;
    }
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

/// The trapped accesses the routes serve without the device mutex (device.rs `Routes`: the
/// interrupt page's stores in exact mode, the MSI re-arm, the read-only display page, the
/// display status registers), which the access rows do not see. Counted per route and
/// direction with their device time, lock-free, while the profile is on.
pub const ROUTES: [&str; 5] = [
    "intr page 0 (PMC)",
    "intr tree page",
    "MSI re-arm (NV_PCFG)",
    "display FE page",
    "display status",
];
// Engine::handles: three eight-head FE groups, four FE scalars and three
// eight-head status groups. The private Engine HEAD_STRIDE is 0x800; the coverage
// regression below checks the complete address set against Engine::handles.
const DISPLAY_GROUPS: [(u64, u64); 6] = {
    use crate::display::engine::*;
    [
        (FE_EVT_STAT_HEAD_TIMING, 4),
        (FE_RM_INTR_STAT_HEAD_TIMING, 4),
        (FE_RM_INTR_EN_HEAD_TIMING, 4),
        (FE_CORE_HEAD_STATE, 0x800),
        (RG_DPCA, 0x800),
        (POSTCOMP_HEAD_LOADV_COUNTER, 0x800),
    ]
};
const DISPLAY_SCALARS: [u64; 4] = {
    use crate::display::engine::*;
    [FE_RM_INTR_DISPATCH, FE_RM_INTR_STAT_CTRL_DISP, FE_EVT_STAT_AWAKEN_WIN, FE_EVT_STAT_AWAKEN_OTHER]
};
const DISPLAY_HEADS: usize = crate::display::engine::MAX_HEADS;
const DISPLAY_REGS: usize = 6 * DISPLAY_HEADS + 4;
const DISPLAY_SLOTS: usize = DISPLAY_REGS + 1; // Last: unsupported address/width.
const DISPLAY_COUNTERS: usize = 2 * DISPLAY_SLOTS * 2; // FE/status parents, R/W.

fn display_register(slot: usize) -> u64 {
    if slot < 6 * DISPLAY_HEADS {
        let (base, stride) = DISPLAY_GROUPS[slot / DISPLAY_HEADS];
        base + stride * (slot % DISPLAY_HEADS) as u64
    } else {
        DISPLAY_SCALARS[slot - 6 * DISPLAY_HEADS]
    }
}

fn display_slot(offset: u64, width: usize) -> usize {
    if width == 4 {
        for (group, &(base, stride)) in DISPLAY_GROUPS.iter().enumerate() {
            if let Some(delta) = offset.checked_sub(base) {
                if delta < stride * DISPLAY_HEADS as u64 && delta % stride == 0 {
                    return group * DISPLAY_HEADS + (delta / stride) as usize;
                }
            }
        }
        if let Some(slot) = DISPLAY_SCALARS.iter().position(|&reg| reg == offset) {
            return 6 * DISPLAY_HEADS + slot;
        }
    }
    DISPLAY_REGS
}

/// Fixed storage only. A successful route's existing duration is shared by its parent
/// and display subrow; neither is an ordinary Access/gap observation. Relaxed loads at
/// dump are not a transactional snapshot across concurrent writers.
struct RoutedCounters {
    count: [AtomicU64; 2 * ROUTES.len()],
    ns: [AtomicU64; 2 * ROUTES.len()],
    display_count: [AtomicU64; DISPLAY_COUNTERS],
    display_ns: [AtomicU64; DISPLAY_COUNTERS],
}

impl RoutedCounters {
    const fn new() -> Self {
        Self {
            count: [DB_ZERO; 2 * ROUTES.len()],
            ns: [DB_ZERO; 2 * ROUTES.len()],
            display_count: [DB_ZERO; DISPLAY_COUNTERS],
            display_ns: [DB_ZERO; DISPLAY_COUNTERS],
        }
    }

    fn record(&self, enabled: bool, route: usize, write: bool, offset: u64, width: usize, dur: Duration) {
        if !enabled {
            return;
        }
        let route = route.min(ROUTES.len() - 1);
        let i = route * 2 + write as usize;
        let ns = dur.as_nanos() as u64;
        self.count[i].fetch_add(1, Ordering::Relaxed);
        self.ns[i].fetch_add(ns, Ordering::Relaxed);
        if route == 3 || route == 4 {
            let slot = display_slot(offset, width);
            let i = ((route - 3) * DISPLAY_SLOTS + slot) * 2 + write as usize;
            self.display_count[i].fetch_add(1, Ordering::Relaxed);
            self.display_ns[i].fetch_add(ns, Ordering::Relaxed);
        }
    }

    fn rows(&self) -> Vec<(&'static str, u64)> {
        static NAMES: std::sync::OnceLock<Vec<(&'static str, &'static str)>> = std::sync::OnceLock::new();
        let names = NAMES.get_or_init(|| {
            let mut names = Vec::new();
            for i in 0..2 * ROUTES.len() {
                let (r, d) = (ROUTES[i / 2], if i % 2 == 1 { 'W' } else { 'R' });
                names.push((
                    &*Box::leak(format!("routed trap: {r} {d}").into_boxed_str()),
                    &*Box::leak(format!("routed trap ns: {r} {d}").into_boxed_str()),
                ));
            }
            for i in 0..DISPLAY_COUNTERS {
                let route = ROUTES[3 + i / (DISPLAY_SLOTS * 2)];
                let slot = (i / 2) % DISPLAY_SLOTS;
                let direction = if i % 2 == 1 { 'W' } else { 'R' };
                let register = if slot == DISPLAY_REGS {
                    "overflow (unsupported address/width)".to_string()
                } else {
                    format!("{:#08x}", display_register(slot))
                };
                names.push((
                    &*Box::leak(format!("routed display register (nonadditive to parent): {route} {direction} {register}").into_boxed_str()),
                    &*Box::leak(format!("routed display register ns (nonadditive to parent): {route} {direction} {register}").into_boxed_str()),
                ));
            }
            names
        });
        let mut rows = Vec::new();
        for (i, (n, ns)) in names.iter().take(2 * ROUTES.len()).enumerate() {
            let count = self.count[i].load(Ordering::Relaxed);
            if count > 0 {
                rows.push((*n, count));
                rows.push((*ns, self.ns[i].load(Ordering::Relaxed)));
            }
        }
        for (i, (n, ns)) in names.iter().skip(2 * ROUTES.len()).enumerate() {
            let count = self.display_count[i].load(Ordering::Relaxed);
            if count > 0 {
                rows.push((*n, count));
                rows.push((*ns, self.display_ns[i].load(Ordering::Relaxed)));
            }
        }
        rows
    }
}

static ROUTED: RoutedCounters = RoutedCounters::new();

/// Count one successful route with its existing timer. Width/address classify display
/// observations only; no payload, lock, allocation or additional timer is involved.
#[inline]
pub fn routed(route: usize, write: bool, offset: u64, width: usize, dur: Duration) {
    ROUTED.record(on(), route, write, offset, width, dur);
}

/// Original aggregate rows plus bounded display subrows explicitly nonadditive to their
/// parents. As with aggregate "routed trap ns", subrow ns live in the count column.
pub fn routed_rows() -> Vec<(&'static str, u64)> {
    ROUTED.rows()
}

/// Runlists and ChIDs of the trapped-doorbell rows ([`doorbell_token`]); a ChID at or above
/// [`DB_CHIDS`] is counted in its runlist's overflow row.
pub const DB_RUNLISTS: usize = 8;
pub const DB_CHIDS: usize = 64;
const DB_SLOTS: usize = DB_RUNLISTS * (DB_CHIDS + 1) + 1;
#[allow(clippy::declare_interior_mutable_const)]
const DB_ZERO: AtomicU64 = AtomicU64::new(0);
static DB_TOKENS: [AtomicU64; DB_SLOTS] = [DB_ZERO; DB_SLOTS];

/// The counter of a doorbell token: a work-submit token is `RUNLIST_ID` (bits 22:16) and
/// `VECTOR`, the ChID (bits 11:0) (tu102/dev_ctrl.h:36-37); any other value (a pool token of
/// tokens.rs among them) is the last slot.
fn doorbell_slot(token: u32) -> usize {
    let runlist = ((token >> 16) & 0x7F) as usize;
    if token & !0x007F_0FFF != 0 || runlist >= DB_RUNLISTS {
        return DB_SLOTS - 1;
    }
    runlist * (DB_CHIDS + 1) + ((token & 0xFFF) as usize).min(DB_CHIDS)
}

/// Count one trapped usermode-doorbell write of `token` (device.rs `post_doorbell`), for the
/// trap census: which rings miss the token pool's and the guest tokens' ioeventfds. One
/// relaxed add, no lock; the caller calls it only while the profile is on.
#[inline]
pub fn doorbell_token(token: u32) {
    DB_TOKENS[doorbell_slot(token)].fetch_add(1, Ordering::Relaxed);
}

/// The window rows of [`doorbell_token`] (a [`Source`]): one per nonzero counter.
pub fn doorbell_token_rows() -> Vec<(&'static str, u64)> {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    let names = NAMES.get_or_init(|| {
        (0..DB_SLOTS)
            .map(|i| {
                let (rl, c) = (i / (DB_CHIDS + 1), i % (DB_CHIDS + 1));
                let s = if i == DB_SLOTS - 1 {
                    "trapped doorbell: other value".to_string()
                } else if c == DB_CHIDS {
                    format!("trapped doorbell: runlist {rl} chid >= {DB_CHIDS}")
                } else {
                    format!("trapped doorbell: runlist {rl} chid {c}")
                };
                &*Box::leak(s.into_boxed_str())
            })
            .collect()
    });
    DB_TOKENS
        .iter()
        .zip(names)
        .filter_map(|(c, n)| {
            let v = c.load(Ordering::Relaxed);
            (v > 0).then_some((*n, v))
        })
        .collect()
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

fn host_ioctl_span(nr: u32, key: u32, status: u32, begin: Instant, end: Instant) {
    if !on() {
        return;
    }
    if let Some(p) = global().as_mut() {
        p.record(
            Key::Host { nr, key },
            end.duration_since(begin),
            0,
            Some(status),
            "",
        );
        if let Some(t) = &mut p.host_timeline {
            t.record(nr, key, status, begin, end);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submission_timeline_requires_profile_before_observer_setup() {
        const MODE: &str = "NVRM_PROFILE_TEST_SUBMISSION_DEPENDENCIES";
        if matches!(std::env::var(MODE).as_deref(), Ok("missing-profile" | "missing-host")) {
            assert!(requested());
            assert_eq!(
                start(PathBuf::from("unused-submission-profile")).unwrap_err().kind(),
                std::io::ErrorKind::InvalidInput
            );
            assert!(!on());
            assert!(global().is_none());
            assert!(backend_markers::snapshot("rejected").is_none());
            assert!(submission_timeline::snapshot("rejected").is_none());
            assert!(nvrm_abi::set_ioctl_observer(|_, _, _, _| {}));
            return;
        }
        for (mode, profile, host) in [("missing-profile", false, true), ("missing-host", true, false)] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child.args(["--exact", "profile::tests::submission_timeline_requires_profile_before_observer_setup"])
                .env(MODE, mode)
                .env(SUBMISSION_TIMELINE_ENV, "1");
            if profile { child.env(PROFILE_ENV, "1"); } else { child.env_remove(PROFILE_ENV); }
            if host { child.env(HOST_TIMELINE_ENV, "1"); } else { child.env_remove(HOST_TIMELINE_ENV); }
            let output = child.output().unwrap();
            assert!(output.status.success(), "{mode}: stdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr));
        }
    }

    #[test]
    fn host_timeline_opt_in_dump_and_observer_conflict() {
        const MODE: &str = "NVRM_PROFILE_TEST_HOST_TIMELINE_MODE";
        let mode = match std::env::var(MODE) {
            Ok(mode) => mode,
            Err(_) => {
                // The observer and profile are process-wide. Each case gets fresh globals.
                for mode in ["normal", "timeline", "submission", "conflict"] {
                    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
                    child
                        .args([
                            "--exact",
                            "profile::tests::host_timeline_opt_in_dump_and_observer_conflict",
                        ])
                        .env(MODE, mode)
                        .env(PROFILE_ENV, "1")
                        .env_remove(HOST_TIMELINE_ENV)
                        .env_remove(SUBMISSION_TIMELINE_ENV);
                    if mode != "normal" {
                        child.env(HOST_TIMELINE_ENV, "1");
                    }
                    if mode == "submission" {
                        child.env(SUBMISSION_TIMELINE_ENV, "1");
                    }
                    let output = child.output().unwrap();
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    assert!(
                        output.status.success() && stdout.contains("1 passed;"),
                        "{mode}: stdout: {stdout}\nstderr: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                return;
            }
        };
        let root = std::env::temp_dir().join(format!(
            "nvgpu-host-timeline-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let prefix = root.join("profile");
        if mode == "conflict" {
            assert!(nvrm_abi::set_ioctl_observer(|_, _, _, _| {}));
            assert_eq!(
                start(prefix).unwrap_err().kind(),
                std::io::ErrorKind::AlreadyExists
            );
            assert!(!on());
            assert!(global().is_none());
            assert!(backend_markers::snapshot("conflict").is_none());
        } else {
            assert!(mode == "normal" || mode == "timeline" || mode == "submission");
            start(prefix.clone()).unwrap();
            let device = nvrm_abi::NvDevice::open("/dev/null").unwrap();
            for cmd in [0x2080_a612, 0x2080_a613, 0x2080_0101] {
                let mut params = nvrm_abi::sys::NVOS54_PARAMETERS::default();
                params.cmd = cmd;
                // SAFETY: initialized control parameters. /dev/null rejects the ioctl;
                // no NVIDIA device, argument pointers or privileged operation is used.
                assert!(
                    unsafe { device.ioctl_raw(nvrm_abi::sys::NV_ESC_RM_CONTROL, &mut params) }.is_err()
                );
            }
            {
                let guard = global();
                let profile = guard.as_ref().unwrap();
                for cmd in [0x2080_a612, 0x2080_a613, 0x2080_0101] {
                    let cell = &profile.rows[&Key::Host { nr: 0x2a, key: cmd }];
                    assert_eq!(cell.count, 1);
                    assert_eq!(cell.status, [(u32::MAX, 1)]);
                }
                assert_eq!(profile.host_timeline.is_some(), mode != "normal");
            }
            if mode == "submission" {
                let begin = submission_timeline::start().unwrap();
                submission_timeline::record(
                    submission_timeline::Stage::PhysicalDoorbell,
                    submission_timeline::Source::Mirror,
                    submission_timeline::Ids { chid: Some(7), generation: Some(3) },
                    begin,
                );
            }
            backend_markers::record(backend_markers::Kind::PhysicalDoorbellWritten, backend_markers::Ids {
                chid: Some(7), channel_generation: Some(3), ..backend_markers::Ids::default()
            });
            assert!(dump("one", "CPU-only test").unwrap().is_file());
            assert!(dump("two", "CPU-only test").unwrap().is_file());
            let markers_path = PathBuf::from(format!("{}.one.markers.tsv", prefix.display()));
            assert_eq!(markers_path.exists(), mode != "normal");
            if mode != "normal" {
                let markers = std::fs::read_to_string(markers_path).unwrap();
                let second = std::fs::read_to_string(format!("{}.two.markers.tsv", prefix.display())).unwrap();
                assert!(markers.contains("seen=1 retained=1 overflow=0 contention=0 poisoned=0 invalid=0"));
                assert_eq!(markers.lines().filter(|l| !l.starts_with('#')).collect::<Vec<_>>(), second.lines().filter(|l| !l.starts_with('#')).collect::<Vec<_>>());
            } else {
                assert!(backend_markers::snapshot("disabled").is_none());
            }
            let path = PathBuf::from(format!("{}.one.host.tsv", prefix.display()));
            assert_eq!(path.exists(), mode != "normal");
            if mode != "normal" {
                let text = std::fs::read_to_string(path).unwrap();
                assert!(text.contains("seen=2 retained=2 dropped=0 invalid=0"));
                assert_eq!(text.lines().filter(|l| !l.starts_with('#')).count(), 3);
            }
            let submission_path = PathBuf::from(format!("{}.one.submission.tsv", prefix.display()));
            assert_eq!(submission_path.exists(), mode == "submission");
            if mode == "submission" {
                let text = std::fs::read_to_string(submission_path).unwrap();
                assert!(text.contains("seen=1 retained=1 overflow=0 contention=0 poisoned=0 invalid=0"));
                assert!(text.contains("physical_doorbell\tmirror\t"));
                assert!(text.contains("\t7\t3\n"));
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }

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
    fn routed_display_covers_engine_handles_and_rejects_partial_registers() {
        assert_eq!(DISPLAY_REGS, 52);
        let registers: std::collections::BTreeSet<_> = (0..DISPLAY_REGS).map(display_register).collect();
        assert_eq!(registers.len(), 52);
        // Independently use the engine's predicate for every byte in its register span,
        // including holes and unaligned neighbors, rather than duplicating the mapper.
        for offset in 0x610000..0x61a000 {
            assert_eq!(display_slot(offset, 4) < DISPLAY_REGS, crate::display::engine::Engine::handles(offset), "{offset:#x}");
        }
        for offset in registers {
            for width in [0, 1, 2, 3, 8] {
                assert_eq!(display_slot(offset, width), DISPLAY_REGS);
            }
        }
        assert_eq!(display_slot(u64::MAX, 4), DISPLAY_REGS);
        assert_eq!(display_slot(0, 4), DISPLAY_REGS);
    }

    #[test]
    fn routed_display_reconciles_parent_counts_and_shared_duration_with_overflow() {
        let counters = RoutedCounters::new();
        for slot in 0..DISPLAY_REGS {
            let offset = display_register(slot);
            let route = if (0x611000..0x612000).contains(&offset) { 3 } else { 4 };
            for write in [false, true] {
                counters.record(true, route, write, offset, 4, Duration::from_nanos((slot + 1) as u64));
            }
        }
        // The FE page can route partial/unknown words; these must not be mislabeled
        // as one of the modeled dwords, even when the starting address is modeled.
        counters.record(true, 3, false, display_register(0), 1, Duration::from_nanos(71));
        counters.record(true, 3, false, display_register(0) + 1, 4, Duration::from_nanos(73));
        counters.record(true, 4, true, 0xdead, 4, Duration::from_nanos(79));
        // Preserve the aggregate hook's past-table clamp to the last route.
        counters.record(true, 99, false, 0, 4, Duration::from_nanos(10));
        counters.record(true, 1, true, 0xb81200, 4, Duration::from_micros(3));
        counters.record(true, 1, true, 0xb81200, 4, Duration::from_micros(5));
        let rows = counters.rows();
        let get = |n: &str| rows.iter().find(|r| r.0 == n).map(|r| r.1);
        assert_eq!(get("routed trap: intr tree page W"), Some(2));
        assert_eq!(get("routed trap ns: intr tree page W"), Some(8000));
        assert_eq!(get("routed trap: MSI re-arm (NV_PCFG) W"), None);
        for route in 3..5 {
            for write in 0..2 {
                let first = (route - 3) * DISPLAY_SLOTS * 2 + write;
                let count: u64 = (first..first + DISPLAY_SLOTS * 2).step_by(2)
                    .map(|i| counters.display_count[i].load(Ordering::Relaxed)).sum();
                let ns: u64 = (first..first + DISPLAY_SLOTS * 2).step_by(2)
                    .map(|i| counters.display_ns[i].load(Ordering::Relaxed)).sum();
                assert_eq!(count, counters.count[route * 2 + write].load(Ordering::Relaxed));
                assert_eq!(ns, counters.ns[route * 2 + write].load(Ordering::Relaxed));
            }
        }
        assert_eq!(get("routed trap: display FE page R"), Some(30));
        assert_eq!(get("routed trap: display status W"), Some(25));
        assert_eq!(get("routed trap: display status R"), Some(25));
        assert_eq!(get("routed display register (nonadditive to parent): display FE page R overflow (unsupported address/width)"), Some(2));
        assert_eq!(get("routed display register ns (nonadditive to parent): display FE page R overflow (unsupported address/width)"), Some(144));
        assert_eq!(get("routed display register (nonadditive to parent): display status W overflow (unsupported address/width)"), Some(1));
    }

    #[test]
    fn routed_profile_disabled_changes_neither_parent_nor_display_storage() {
        let counters = RoutedCounters::new();
        for route in 0..ROUTES.len() {
            for write in [false, true] {
                counters.record(false, route, write, display_register(0), 4, Duration::from_micros(5));
                counters.record(false, route, write, 0xdead, 1, Duration::from_micros(7));
            }
        }
        assert!(counters.rows().is_empty());
        // Same local hook becomes observable when enabled; no global env mutation.
        counters.record(true, 4, false, display_register(0), 4, Duration::from_nanos(11));
        assert_eq!(counters.count[8].load(Ordering::Relaxed), 1);
        assert_eq!(counters.ns[8].load(Ordering::Relaxed), 11);
        assert_eq!(counters.display_count[DISPLAY_SLOTS * 2].load(Ordering::Relaxed), 1);
        assert_eq!(counters.display_ns[DISPLAY_SLOTS * 2].load(Ordering::Relaxed), 11);
    }

    #[test]
    fn doorbell_tokens_by_runlist_and_chid() {
        assert_eq!(doorbell_slot(0x0000_0000), 0);
        assert_eq!(doorbell_slot(0x0001_0005), DB_CHIDS + 1 + 5);
        assert_eq!(doorbell_slot(0x0007_0033), 7 * (DB_CHIDS + 1) + 0x33);
        // A ChID past the rows goes to its runlist's overflow row.
        assert_eq!(doorbell_slot(0x0002_0040), 2 * (DB_CHIDS + 1) + DB_CHIDS);
        // A pool token (tokens.rs TOKEN_BASE), a runlist past the rows, bits outside the
        // token's fields: the other-value row.
        assert_eq!(doorbell_slot(0xCA70_0003), DB_SLOTS - 1);
        assert_eq!(doorbell_slot(0x0008_0001), DB_SLOTS - 1);
        assert_eq!(doorbell_slot(0x0000_1001), DB_SLOTS - 1);
        doorbell_token(0x0003_0011);
        doorbell_token(0x0003_0011);
        let rows = doorbell_token_rows();
        let row = rows.iter().find(|r| r.0 == "trapped doorbell: runlist 3 chid 17");
        assert!(row.is_some_and(|r| r.1 >= 2), "{rows:?}");
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

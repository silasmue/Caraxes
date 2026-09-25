// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The doorbell reactor (`CARAXES_DOORBELL_POLL=off|spin|hybrid`, docs/caraxes/doorbell-poll.md).
//!
//! The doorbell eventfds (the token pool's, tokens.rs, or the token-less one) are read by
//! one thread over one epoll set. `off` blocks in `epoll_wait` until KVM signals an
//! eventfd, so every ring pays the scheduler wake-up of that thread. `spin` calls
//! `epoll_wait` with timeout 0 in a loop, as an SPDK reactor polls its queues: one
//! syscall per iteration over all eventfds, the thread never sleeps. `hybrid` spins for
//! `CARAXES_DOORBELL_POLL_IDLE_US` after the last event, then blocks until the next one
//! and spins again. A spinning reader pins itself to one core
//! (`CARAXES_DOORBELL_POLL_CPU`, or one chosen at start), since a spinning thread the
//! scheduler moves or preempts loses what it gains.
//!
//! The loop allocates nothing and logs nothing: the event and ring buffers are sized at
//! start, the counters are atomics only this thread writes, the latency histograms are
//! fixed arrays. The Device mutex is taken only when an eventfd fired, as before.

use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering::Relaxed};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The switch: `off` (default), `spin` or `hybrid`.
pub const POLL_ENV: &str = "CARAXES_DOORBELL_POLL";
/// The reader's core in spin and hybrid mode: a CPU number, `auto` (the default) or `none`.
pub const POLL_CPU_ENV: &str = "CARAXES_DOORBELL_POLL_CPU";
/// Hybrid mode: microseconds to keep spinning after the last event.
pub const POLL_IDLE_ENV: &str = "CARAXES_DOORBELL_POLL_IDLE_US";
/// The hybrid idle time when `CARAXES_DOORBELL_POLL_IDLE_US` is unset.
pub const DEFAULT_IDLE_US: u64 = 200;
/// Hybrid mode: the adaptive spin window's cap in microseconds (docs/caraxes/launch-path.md).
/// Unset or 0: the window is the fixed idle time. Set: after each event the reactor keeps
/// spinning for 1.5 times the longest of the last [`ADAPT_GAPS`] gaps between events that
/// were shorter than the cap, at least the idle time and at most the cap, so a workload
/// whose submissions come every millisecond or two does not pay the scheduler wake-up at
/// each of them, and an idle VM still sleeps after one cap.
pub const POLL_ADAPT_ENV: &str = "CARAXES_DOORBELL_POLL_ADAPT_US";
/// The gaps the adaptive window remembers.
pub const ADAPT_GAPS: usize = 8;

/// The adaptive window's cap the environment asks for, for `mode` (`None`: off). Refused
/// for a mode other than hybrid, or a cap below the idle time.
pub fn adapt_requested(mode: Mode) -> Result<Option<Duration>, String> {
    parse_adapt(mode, std::env::var(POLL_ADAPT_ENV).ok().as_deref())
}

/// [`adapt_requested`] on a given value.
pub fn parse_adapt(mode: Mode, v: Option<&str>) -> Result<Option<Duration>, String> {
    let us = match v.map(str::trim).filter(|v| !v.is_empty()) {
        None | Some("0") => return Ok(None),
        Some(v) => v
            .parse::<u64>()
            .map_err(|_| format!("{POLL_ADAPT_ENV}={v:?}: microseconds, a whole number"))?,
    };
    let Mode::Hybrid { idle } = mode else {
        return Err(format!(
            "{POLL_ADAPT_ENV} is set but {POLL_ENV}={}: the adaptive window applies to hybrid only",
            mode.name()
        ));
    };
    let cap = Duration::from_micros(us);
    if cap < idle {
        return Err(format!(
            "{POLL_ADAPT_ENV}={us}: below the idle time of {} us",
            idle.as_micros()
        ));
    }
    Ok(Some(cap))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Block in `epoll_wait` (timeout -1): the reader as before.
    Off,
    /// `epoll_wait` with timeout 0 in a loop.
    Spin,
    /// Spin for `idle` after the last event, then block until the next one.
    Hybrid { idle: Duration },
}

impl Mode {
    pub fn name(&self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Spin => "spin",
            Mode::Hybrid { .. } => "hybrid",
        }
    }
}

/// Where a polling reader runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pin {
    /// Chosen at start (`choose_cpu`).
    Auto,
    /// Not pinned (`CARAXES_DOORBELL_POLL_CPU=none`): for comparison only.
    None,
    Cpu(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub mode: Mode,
    pub pin: Pin,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mode: Mode::Off,
            pin: Pin::None,
        }
    }
}

/// The reactor configuration the environment asks for.
pub fn requested() -> Result<Config, String> {
    let get = |k: &str| std::env::var(k).ok();
    parse(
        get(POLL_ENV).as_deref(),
        get(POLL_CPU_ENV).as_deref(),
        get(POLL_IDLE_ENV).as_deref(),
    )
}

/// `mode`, `cpu` and `idle_us` as the three variables give them (`None`: unset). A
/// setting that the mode would ignore is refused rather than dropped silently.
pub fn parse(
    mode: Option<&str>,
    cpu: Option<&str>,
    idle_us: Option<&str>,
) -> Result<Config, String> {
    fn set(v: Option<&str>) -> Option<&str> {
        v.map(str::trim).filter(|v| !v.is_empty())
    }
    let (mode_s, cpu_s, idle_s) = (set(mode), set(cpu), set(idle_us));
    let mode = match mode_s.unwrap_or("off") {
        "off" | "0" => Mode::Off,
        "spin" => Mode::Spin,
        "hybrid" => {
            let us = match idle_s {
                None => DEFAULT_IDLE_US,
                Some(v) => v
                    .parse::<u64>()
                    .map_err(|_| format!("{POLL_IDLE_ENV}={v:?}: microseconds, a whole number"))?,
            };
            Mode::Hybrid {
                idle: Duration::from_micros(us),
            }
        }
        other => return Err(format!("{POLL_ENV}={other:?}: off, spin or hybrid")),
    };
    if idle_s.is_some() && !matches!(mode, Mode::Hybrid { .. }) {
        return Err(format!(
            "{POLL_IDLE_ENV} is set but {POLL_ENV}={}: the idle time applies to hybrid only",
            mode.name()
        ));
    }
    let pin = match (mode, cpu_s) {
        (Mode::Off, None) => Pin::None,
        (Mode::Off, Some(_)) => {
            return Err(format!(
                "{POLL_CPU_ENV} is set but {POLL_ENV}=off: the blocking reader is not pinned; use spin or hybrid"
            ))
        }
        (_, None | Some("auto")) => Pin::Auto,
        (_, Some("none")) => Pin::None,
        (_, Some(v)) => Pin::Cpu(v.parse().map_err(|_| {
            format!("{POLL_CPU_ENV}={v:?}: a CPU number, auto or none")
        })?),
    };
    Ok(Config { mode, pin })
}

/// The wait of one loop iteration: the `epoll_wait` timeout in milliseconds, 0 to poll
/// and -1 to block. Hybrid mode is a two-state machine: polling while the last event is
/// younger than `idle`, blocked otherwise; any event returns it to polling. It starts
/// blocked (no event yet).
#[derive(Clone, Copy, Debug)]
pub struct Wait {
    mode: Mode,
    last_event: Option<Instant>,
    /// The adaptive window's cap (`CARAXES_DOORBELL_POLL_ADAPT_US`), hybrid only.
    adapt: Option<Duration>,
    /// The last gaps between events below the cap, a ring; zero entries are empty.
    gaps: [Duration; ADAPT_GAPS],
    next_gap: usize,
    /// The spin window after the last event.
    window: Duration,
}

impl Wait {
    pub fn new(mode: Mode) -> Self {
        let window = match mode {
            Mode::Hybrid { idle } => idle,
            _ => Duration::ZERO,
        };
        Wait {
            mode,
            last_event: None,
            adapt: None,
            gaps: [Duration::ZERO; ADAPT_GAPS],
            next_gap: 0,
            window,
        }
    }

    /// With the adaptive window capped at `cap` (hybrid mode only; `None` keeps it fixed).
    pub fn with_adapt(mut self, cap: Option<Duration>) -> Self {
        if matches!(self.mode, Mode::Hybrid { .. }) {
            self.adapt = cap;
        }
        self
    }

    /// The current spin window after an event.
    pub fn window(&self) -> Duration {
        self.window
    }

    /// The timeout for the next `epoll_wait`. `now` is called only in hybrid mode, so
    /// spin and off read no clock per iteration.
    pub fn timeout(&self, now: impl FnOnce() -> Instant) -> i32 {
        match self.mode {
            Mode::Off => -1,
            Mode::Spin => 0,
            Mode::Hybrid { .. } => match self.last_event {
                Some(t) if now().saturating_duration_since(t) < self.window => 0,
                _ => -1,
            },
        }
    }

    /// `epoll_wait` returned events at `at`.
    pub fn saw_events(&mut self, at: Instant) {
        if let (Some(cap), Some(last), Mode::Hybrid { idle }) =
            (self.adapt, self.last_event, self.mode)
        {
            let gap = at.saturating_duration_since(last);
            if gap > Duration::ZERO && gap < cap {
                self.gaps[self.next_gap] = gap;
                self.next_gap = (self.next_gap + 1) % ADAPT_GAPS;
            }
            let longest = self.gaps.iter().copied().max().unwrap_or_default();
            self.window = (longest * 3 / 2).clamp(idle, cap);
        }
        self.last_event = Some(at);
    }
}

/// A histogram of durations in 100 ns buckets up to 200 us, and one overflow bucket;
/// written by the reader only, read at exit.
pub struct Histogram {
    buckets: Box<[AtomicU64]>,
    count: AtomicU64,
    sum_ns: AtomicU64,
    max_ns: AtomicU64,
}

const BUCKET_NS: u64 = 100;
const BUCKETS: usize = 2000;

impl Default for Histogram {
    fn default() -> Self {
        Histogram {
            buckets: (0..=BUCKETS).map(|_| AtomicU64::new(0)).collect(),
            count: AtomicU64::new(0),
            sum_ns: AtomicU64::new(0),
            max_ns: AtomicU64::new(0),
        }
    }
}

/// One writer: a load and a store, no locked instruction.
fn bump(a: &AtomicU64, by: u64) {
    a.store(a.load(Relaxed).wrapping_add(by), Relaxed);
}

impl Histogram {
    pub fn record(&self, d: Duration) {
        let ns = d.as_nanos().min(u128::from(u64::MAX)) as u64;
        let b = ((ns / BUCKET_NS) as usize).min(BUCKETS);
        bump(&self.buckets[b], 1);
        bump(&self.count, 1);
        bump(&self.sum_ns, ns);
        if ns > self.max_ns.load(Relaxed) {
            self.max_ns.store(ns, Relaxed);
        }
    }

    pub fn count(&self) -> u64 {
        self.count.load(Relaxed)
    }

    pub fn mean_us(&self) -> f64 {
        let n = self.count();
        if n == 0 {
            return 0.0;
        }
        self.sum_ns.load(Relaxed) as f64 / n as f64 / 1000.0
    }

    pub fn max_us(&self) -> f64 {
        self.max_ns.load(Relaxed) as f64 / 1000.0
    }

    /// The upper edge of the bucket holding quantile `q` (0..1), in microseconds; the
    /// maximum when it lies in the overflow bucket.
    pub fn quantile_us(&self, q: f64) -> f64 {
        let n = self.count();
        if n == 0 {
            return 0.0;
        }
        let want = ((n as f64) * q).ceil().max(1.0) as u64;
        let mut seen = 0;
        for (i, b) in self.buckets.iter().enumerate() {
            seen += b.load(Relaxed);
            if seen >= want {
                if i == BUCKETS {
                    return self.max_us();
                }
                return ((i as u64 + 1) * BUCKET_NS) as f64 / 1000.0;
            }
        }
        self.max_us()
    }

    fn line(&self) -> String {
        format!(
            "mean {:.2} us, p50 {:.1} us, p99 {:.1} us, max {:.1} us",
            self.mean_us(),
            self.quantile_us(0.5),
            self.quantile_us(0.99),
            self.max_us()
        )
    }
}

/// The reader's counters for the exit summary.
pub struct ReaderStats {
    pub mode: Mode,
    /// The core the reader pinned itself to, -1 when not pinned (yet).
    pub cpu: AtomicI64,
    /// The reader thread's kernel tid, for its CPU time.
    pub tid: AtomicI64,
    started: Instant,
    /// Loop iterations (`epoll_wait` calls), those that returned events, and those that
    /// blocked (timeout -1).
    pub iterations: AtomicU64,
    pub wakeups: AtomicU64,
    pub blocking_waits: AtomicU64,
    /// Eventfds read and the guest writes they counted.
    pub events: AtomicU64,
    pub writes: AtomicU64,
    /// Rings (one per eventfd read) that ended in a host doorbell write, and those that
    /// did not (software executor, stale token, no pending channel).
    pub without_host_doorbell: AtomicU64,
    /// From `epoll_wait` returning to the host doorbell write, per ring.
    pub ring_to_host: Histogram,
    /// From `epoll_wait` returning to holding the Device mutex, per wakeup: the wait for
    /// the vfio-user server thread.
    pub lock_wait: Histogram,
    /// Set to end the loop at its next iteration (tests; the device's reader runs until
    /// the process exits).
    pub stop: AtomicBool,
    /// The adaptive window's cap (0: off) and its value after the last event, in us.
    pub adapt_cap_us: AtomicU64,
    pub window_us: AtomicU64,
}

impl ReaderStats {
    pub fn new(mode: Mode) -> Self {
        ReaderStats {
            mode,
            cpu: AtomicI64::new(-1),
            tid: AtomicI64::new(0),
            started: Instant::now(),
            iterations: AtomicU64::new(0),
            wakeups: AtomicU64::new(0),
            blocking_waits: AtomicU64::new(0),
            events: AtomicU64::new(0),
            writes: AtomicU64::new(0),
            without_host_doorbell: AtomicU64::new(0),
            ring_to_host: Histogram::default(),
            lock_wait: Histogram::default(),
            stop: AtomicBool::new(false),
            adapt_cap_us: AtomicU64::new(0),
            window_us: AtomicU64::new(0),
        }
    }

    /// A ring handled: `host` is when the host doorbell was written, if it was.
    pub fn ring(&self, seen: Instant, host: Option<Instant>) {
        match host {
            Some(t) => self.ring_to_host.record(t.saturating_duration_since(seen)),
            None => bump(&self.without_host_doorbell, 1),
        }
    }

    /// The reader thread's CPU time (`/proc/self/task/<tid>/schedstat`, first field:
    /// nanoseconds on a CPU).
    pub fn cpu_time(&self) -> Option<Duration> {
        let tid = self.tid.load(Relaxed);
        if tid <= 0 {
            return None;
        }
        let s = std::fs::read_to_string(format!("/proc/self/task/{tid}/schedstat")).ok()?;
        let ns: u64 = s.split_whitespace().next()?.parse().ok()?;
        Some(Duration::from_nanos(ns))
    }

    /// One line for the exit summary.
    pub fn summary(&self, name: &str) -> String {
        let cpu = self.cpu.load(Relaxed);
        let wall = self.started.elapsed();
        let mode = match self.mode {
            Mode::Hybrid { idle } => match self.adapt_cap_us.load(Relaxed) {
                0 => format!("hybrid ({} us idle)", idle.as_micros()),
                cap => format!(
                    "hybrid ({} us idle, adaptive up to {cap} us, last window {} us)",
                    idle.as_micros(),
                    self.window_us.load(Relaxed)
                ),
            },
            m => m.name().into(),
        };
        format!(
            "doorbell poll ({name}): {mode}, {}; {} loop iteration(s), {} with events, {} blocking wait(s); {} eventfd event(s) for {} guest write(s); ring to host doorbell: {} ring(s), {} ({} ring(s) without a host doorbell); Device lock wait per wakeup: {}; reader CPU {} in {:.1} s\n",
            if cpu >= 0 { format!("pinned to CPU {cpu}") } else { "not pinned".into() },
            self.iterations.load(Relaxed),
            self.wakeups.load(Relaxed),
            self.blocking_waits.load(Relaxed),
            self.events.load(Relaxed),
            self.writes.load(Relaxed),
            self.ring_to_host.count(),
            self.ring_to_host.line(),
            self.without_host_doorbell.load(Relaxed),
            self.lock_wait.line(),
            match self.cpu_time() {
                Some(c) => format!(
                    "{:.3} s ({:.1} %)",
                    c.as_secs_f64(),
                    100.0 * c.as_secs_f64() / wall.as_secs_f64().max(1e-9)
                ),
                None => "unknown".into(),
            },
            wall.as_secs_f64()
        )
    }
}

/// The CPUs this process may run on (`sched_getaffinity`).
pub fn allowed_cpus() -> std::io::Result<Vec<usize>> {
    // SAFETY: a zeroed cpu_set_t is a valid empty set; the size passed is its own.
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    if unsafe { libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut set) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((0..libc::CPU_SETSIZE as usize)
        // SAFETY: `i` is below CPU_SETSIZE.
        .filter(|&i| unsafe { libc::CPU_ISSET(i, &set) })
        .collect())
}

/// Pin the calling thread to `cpu`.
pub fn pin_current_thread(cpu: usize) -> std::io::Result<()> {
    // SAFETY: as in `allowed_cpus`; `cpu` is checked against CPU_SETSIZE.
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    if cpu >= libc::CPU_SETSIZE as usize {
        return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
    }
    unsafe { libc::CPU_SET(cpu, &mut set) };
    // SAFETY: pid 0 is the calling thread.
    if unsafe { libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// The cores a pinned reader may take: the allowed ones except CPU 0 (the host's
/// housekeeping and most interrupts land there). Checked at start so that a poll mode
/// refuses to start when no core can be chosen.
pub fn candidates(pin: Pin) -> Result<Vec<usize>, String> {
    let allowed = allowed_cpus().map_err(|e| format!("sched_getaffinity: {e}"))?;
    match pin {
        Pin::None => Ok(Vec::new()),
        Pin::Cpu(c) if allowed.contains(&c) => Ok(vec![c]),
        Pin::Cpu(c) => Err(format!(
            "{POLL_CPU_ENV}={c}: not among the CPUs this process may run on {allowed:?}"
        )),
        Pin::Auto => {
            let c: Vec<usize> = allowed.into_iter().filter(|&c| c != 0).collect();
            if c.is_empty() {
                Err(format!(
                    "{POLL_CPU_ENV}=auto: no CPU other than 0 allowed for this process; set {POLL_CPU_ENV}=<n> or none"
                ))
            } else {
                Ok(c)
            }
        }
    }
}

/// Per-CPU (idle, total) jiffies from `/proc/stat` (`cpuN user nice system idle iowait
/// irq softirq steal ...`, proc_stat(5)); idle counts idle and iowait.
pub fn cpu_times(stat: &str) -> Vec<(usize, u64, u64)> {
    stat.lines()
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let cpu: usize = f.next()?.strip_prefix("cpu")?.parse().ok()?;
            let v: Vec<u64> = f.take(8).filter_map(|x| x.parse().ok()).collect();
            if v.len() < 5 {
                return None;
            }
            Some((cpu, v[3] + v[4], v.iter().sum()))
        })
        .collect()
}

/// The idle fraction of each CPU between two `/proc/stat` samples.
pub fn idle_fractions(before: &str, after: &str) -> Vec<(usize, f64)> {
    let a = cpu_times(before);
    cpu_times(after)
        .into_iter()
        .filter_map(|(c, idle, total)| {
            let &(_, i0, t0) = a.iter().find(|x| x.0 == c)?;
            let dt = total.saturating_sub(t0);
            Some((
                c,
                if dt == 0 {
                    1.0
                } else {
                    idle.saturating_sub(i0) as f64 / dt as f64
                },
            ))
        })
        .collect()
}

/// The core for the reader among `candidates`: not one a vCPU thread of the VMM runs on
/// (`busy`), unless that leaves none; among the rest the highest-numbered one at least
/// 90 % idle, else the most idle one (the highest-numbered on a tie).
pub fn choose_cpu(candidates: &[usize], busy: &[usize], idle: &[(usize, f64)]) -> Option<usize> {
    let free: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|c| !busy.contains(c))
        .collect();
    let pool = if free.is_empty() {
        candidates.to_vec()
    } else {
        free
    };
    let idle_of = |c: usize| idle.iter().find(|x| x.0 == c).map_or(0.0, |x| x.1);
    if let Some(&c) = pool.iter().filter(|&&c| idle_of(c) >= 0.9).max() {
        return Some(c);
    }
    pool.iter()
        .copied()
        .max_by(|&a, &b| idle_of(a).total_cmp(&idle_of(b)).then(a.cmp(&b)))
}

/// The cores the vCPU threads (`vcpuN`) of the VMM serving `socket` run on now: the
/// process whose command line names the socket, `/proc/<pid>/task/*/stat` field 39.
pub fn vcpu_cores(socket: &Path) -> Option<(u32, Vec<usize>)> {
    let want = socket.to_string_lossy();
    for e in std::fs::read_dir("/proc").ok()?.flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if pid == std::process::id() {
            continue;
        }
        let Ok(cmd) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
            continue;
        };
        if !String::from_utf8_lossy(&cmd).contains(want.as_ref()) {
            continue;
        }
        let mut cores = Vec::new();
        for t in std::fs::read_dir(format!("/proc/{pid}/task"))
            .ok()?
            .flatten()
        {
            let p = t.path();
            let comm = std::fs::read_to_string(p.join("comm")).unwrap_or_default();
            if !comm.starts_with("vcpu") {
                continue;
            }
            let stat = std::fs::read_to_string(p.join("stat")).unwrap_or_default();
            // Field 2 (comm) may hold spaces; count from the closing parenthesis.
            let rest = stat.rsplit_once(')').map_or("", |x| x.1);
            if let Some(c) = rest.split_whitespace().nth(36).and_then(|v| v.parse().ok()) {
                cores.push(c);
            }
        }
        if !cores.is_empty() {
            cores.sort_unstable();
            cores.dedup();
            return Some((pid, cores));
        }
    }
    None
}

/// Pick and pin the reader's core at the start of the reader thread (before the loop):
/// wait up to 20 s for the VMM's vCPU threads, sample `/proc/stat` over 1 s, choose,
/// pin, log the choice. Returns the core.
fn auto_pin(candidates: &[usize], socket: Option<&Path>) -> Result<usize, String> {
    let mut vcpus = None;
    if let Some(s) = socket {
        for _ in 0..200 {
            vcpus = vcpu_cores(s);
            if vcpus.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let before = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    std::thread::sleep(Duration::from_secs(1));
    let after = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let idle = idle_fractions(&before, &after);
    let busy = vcpus.as_ref().map(|v| v.1.clone()).unwrap_or_default();
    let cpu = choose_cpu(candidates, &busy, &idle).ok_or("no candidate core")?;
    pin_current_thread(cpu).map_err(|e| format!("sched_setaffinity CPU {cpu}: {e}"))?;
    let idle_of = idle.iter().find(|x| x.0 == cpu).map_or(0.0, |x| x.1);
    eprintln!(
        "vfio-user-nvgpu: doorbell poll: reader pinned to CPU {cpu} (auto: {:.0} % idle over 1 s; not CPU 0; VMM vCPU threads {})",
        idle_of * 100.0,
        match &vcpus {
            Some((pid, c)) => format!("of pid {pid} on CPU(s) {c:?} at the sample, avoided"),
            None => "not found within 20 s".into(),
        }
    );
    Ok(cpu)
}

/// An epoll set over `readers`, each registered with its index as the event's data.
pub fn epoll_over(readers: &[File]) -> std::io::Result<OwnedFd> {
    use std::os::fd::FromRawFd;
    // SAFETY: a plain syscall; the result is checked before use.
    let ep = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
    if ep < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: a new descriptor that nothing else owns.
    let ep = unsafe { OwnedFd::from_raw_fd(ep) };
    for (i, r) in readers.iter().enumerate() {
        let mut ev = libc::epoll_event {
            events: libc::EPOLLIN as u32,
            u64: i as u64,
        };
        // SAFETY: both descriptors are open; `ev` lives across the call.
        if unsafe { libc::epoll_ctl(ep.as_raw_fd(), libc::EPOLL_CTL_ADD, r.as_raw_fd(), &mut ev) }
            < 0
        {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(ep)
}

/// Where the reactor runs and what it is called in the log.
pub struct Reactor {
    pub name: &'static str,
    pub config: Config,
    /// The adaptive hybrid window's cap (`CARAXES_DOORBELL_POLL_ADAPT_US`), `None`: fixed.
    pub adapt: Option<Duration>,
    /// The candidate cores (`candidates`), checked before the thread starts.
    pub cpus: Vec<usize>,
    /// The vfio-user socket, to find the VMM's vCPU threads for the automatic choice.
    pub socket: Option<PathBuf>,
}

impl Reactor {
    /// Check the pinning before any thread starts: a poll mode refuses to start when no
    /// core can be chosen.
    pub fn new(
        name: &'static str,
        config: Config,
        socket: Option<PathBuf>,
    ) -> Result<Self, String> {
        let cpus = if config.mode == Mode::Off {
            Vec::new()
        } else {
            candidates(config.pin)?
        };
        let adapt = adapt_requested(config.mode)?;
        Ok(Reactor {
            name,
            config,
            adapt,
            cpus,
            socket,
        })
    }

    /// Start the reader thread: `ring(rings, seen, stats)` is called with the eventfds
    /// read in one wakeup, `(index, guest writes)` each, and the instant `epoll_wait`
    /// returned them. The thread ends only when an OS call fails.
    pub fn spawn<F>(self, readers: Vec<File>, ring: F) -> std::io::Result<Arc<ReaderStats>>
    where
        F: FnMut(&[(usize, u64)], Instant, &ReaderStats) + Send + 'static,
    {
        self.spawn_hooked(readers, ring, None)
    }

    /// As [`Reactor::spawn`], with a [`Hook`] the loop runs on every iteration (the
    /// interrupt page, intr_page.rs): its wake eventfd joins the epoll set, what it finds
    /// counts as an event for hybrid mode, and a blocking wait lasts at most
    /// [`Hook::max_block_ms`]. A hook needs a polling mode.
    pub fn spawn_hooked<F>(
        self,
        readers: Vec<File>,
        mut ring: F,
        mut hook: Option<Box<dyn Hook>>,
    ) -> std::io::Result<Arc<ReaderStats>>
    where
        F: FnMut(&[(usize, u64)], Instant, &ReaderStats) + Send + 'static,
    {
        if hook.is_some() && self.config.mode == Mode::Off {
            return Err(std::io::Error::other(
                "a reactor hook needs a polling mode (spin or hybrid)",
            ));
        }
        let ep = epoll_over(&readers)?;
        if let Some(fd) = hook.as_ref().and_then(|h| h.wake_fd()) {
            let mut ev = libc::epoll_event {
                events: libc::EPOLLIN as u32,
                u64: WAKE,
            };
            // SAFETY: both descriptors are open; `ev` lives across the call.
            if unsafe { libc::epoll_ctl(ep.as_raw_fd(), libc::EPOLL_CTL_ADD, fd, &mut ev) } < 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        if self.config.mode != Mode::Off {
            // Non-blocking reads: a spinning reader must never sleep in read(2).
            for r in &readers {
                // SAFETY: plain fcntl on an open descriptor.
                unsafe {
                    let fl = libc::fcntl(r.as_raw_fd(), libc::F_GETFL);
                    if fl < 0
                        || libc::fcntl(r.as_raw_fd(), libc::F_SETFL, fl | libc::O_NONBLOCK) < 0
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                }
            }
        }
        let stats = Arc::new(ReaderStats::new(self.config.mode));
        if let Some(cap) = self.adapt {
            stats
                .adapt_cap_us
                .store(cap.as_micros().max(1) as u64, Relaxed);
        }
        let st = stats.clone();
        let name = self.name;
        std::thread::Builder::new()
            .name(format!("doorbell-{name}"))
            .spawn(move || {
                // SAFETY: gettid has no preconditions.
                st.tid.store(i64::from(unsafe { libc::gettid() }), Relaxed);
                if self.config.mode != Mode::Off {
                    let pinned = match self.config.pin {
                        Pin::None => {
                            eprintln!("vfio-user-nvgpu: doorbell poll ({name}): {} reader NOT pinned ({POLL_CPU_ENV}=none)", self.config.mode.name());
                            None
                        }
                        Pin::Cpu(c) => match pin_current_thread(c) {
                            Ok(()) => {
                                eprintln!("vfio-user-nvgpu: doorbell poll ({name}): {} reader pinned to CPU {c} ({POLL_CPU_ENV}={c})", self.config.mode.name());
                                Some(c)
                            }
                            Err(e) => {
                                eprintln!("vfio-user-nvgpu: doorbell poll ({name}): sched_setaffinity CPU {c}: {e}; reader runs unpinned");
                                None
                            }
                        },
                        Pin::Auto => match auto_pin(&self.cpus, self.socket.as_deref()) {
                            Ok(c) => Some(c),
                            Err(e) => {
                                eprintln!("vfio-user-nvgpu: doorbell poll ({name}): automatic pinning failed: {e}; reader runs unpinned");
                                None
                            }
                        },
                    };
                    if let Some(c) = pinned {
                        st.cpu.store(c as i64, Relaxed);
                    }
                }
                let wait = Wait::new(self.config.mode).with_adapt(self.adapt);
                if let Err(e) = run(&ep, &readers, wait, &st, &mut ring, &mut hook) {
                    eprintln!("vfio-user-nvgpu: doorbell {name}: {e}; reader ends, the doorbell eventfds are no longer read");
                }
            })?;
        Ok(stats)
    }
}

/// Work a reactor does on every loop iteration besides reading its eventfds: the
/// interrupt page (intr_page.rs) scans the guest's stores there.
pub trait Hook: Send {
    /// One iteration; `blocked` says the iteration followed a blocking wait. Returns
    /// whether it found work, which counts as an event for hybrid mode.
    fn poll(&mut self, blocked: bool) -> bool;
    /// The longest blocking wait in milliseconds while the hook runs.
    fn max_block_ms(&self) -> i32;
    /// An eventfd other threads write to wake a blocked reactor for the hook.
    fn wake_fd(&self) -> Option<std::os::fd::RawFd>;
    /// Read the wake eventfd after it fired.
    fn drain_wake(&mut self);
}

/// The epoll data of a hook's wake eventfd (the readers use their index).
const WAKE: u64 = u64::MAX;

/// The loop. Allocates only before it starts.
fn run<F>(
    ep: &OwnedFd,
    readers: &[File],
    mut wait: Wait,
    st: &ReaderStats,
    ring: &mut F,
    hook: &mut Option<Box<dyn Hook>>,
) -> std::io::Result<()>
where
    F: FnMut(&[(usize, u64)], Instant, &ReaderStats),
{
    let mut events = vec![libc::epoll_event { events: 0, u64: 0 }; readers.len() + 1];
    let mut rings: Vec<(usize, u64)> = Vec::with_capacity(readers.len().max(1));
    let mut iterations = 0u64;
    loop {
        if st.stop.load(Relaxed) {
            return Ok(());
        }
        let mut timeout = wait.timeout(Instant::now);
        iterations += 1;
        st.iterations.store(iterations, Relaxed);
        if timeout < 0 {
            bump(&st.blocking_waits, 1);
            if let Some(h) = hook.as_ref() {
                timeout = h.max_block_ms().max(1);
            }
        }
        // SAFETY: `events` has room for the count passed.
        let n = unsafe {
            libc::epoll_wait(
                ep.as_raw_fd(),
                events.as_mut_ptr(),
                events.len() as i32,
                timeout,
            )
        };
        if n < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        if n == 0 {
            if let Some(h) = hook.as_mut() {
                if h.poll(timeout != 0) {
                    wait.saw_events(Instant::now());
                }
            }
            continue;
        }
        let seen = Instant::now();
        wait.saw_events(seen);
        st.window_us
            .store(wait.window().as_micros() as u64, Relaxed);
        bump(&st.wakeups, 1);
        rings.clear();
        let mut woken = false;
        for ev in &events[..n as usize] {
            if ev.u64 == WAKE {
                woken = true;
                continue;
            }
            let i = ev.u64 as usize;
            let mut count = [0u8; 8];
            match (&readers[i]).read(&mut count) {
                Ok(8) => {
                    let w = u64::from_ne_bytes(count);
                    bump(&st.events, 1);
                    bump(&st.writes, w);
                    rings.push((i, w));
                }
                Ok(_) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
                    ) => {}
                Err(e) => return Err(e),
            }
        }
        if !rings.is_empty() {
            crate::launch::begin(seen, timeout != 0);
            ring(&rings, seen, st);
            crate::launch::end();
        }
        if let Some(h) = hook.as_mut() {
            if woken {
                h.drain_wake();
            }
            h.poll(woken || timeout != 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_selection() {
        let off = Config {
            mode: Mode::Off,
            pin: Pin::None,
        };
        assert_eq!(parse(None, None, None), Ok(off));
        assert_eq!(parse(Some(""), None, None), Ok(off));
        assert_eq!(parse(Some("off"), None, None), Ok(off));
        assert_eq!(
            parse(Some("spin"), None, None),
            Ok(Config {
                mode: Mode::Spin,
                pin: Pin::Auto
            })
        );
        assert_eq!(
            parse(Some(" spin "), Some("7"), None).unwrap().pin,
            Pin::Cpu(7)
        );
        assert_eq!(
            parse(Some("spin"), Some("auto"), None).unwrap().pin,
            Pin::Auto
        );
        assert_eq!(
            parse(Some("spin"), Some("none"), None).unwrap().pin,
            Pin::None
        );
        assert_eq!(
            parse(Some("hybrid"), None, None).unwrap().mode,
            Mode::Hybrid {
                idle: Duration::from_micros(DEFAULT_IDLE_US)
            }
        );
        assert_eq!(
            parse(Some("hybrid"), Some("3"), Some("50")),
            Ok(Config {
                mode: Mode::Hybrid {
                    idle: Duration::from_micros(50)
                },
                pin: Pin::Cpu(3)
            })
        );
        // Refused: unknown mode, bad numbers, settings the mode would ignore.
        assert!(parse(Some("busy"), None, None)
            .unwrap_err()
            .contains("off, spin or hybrid"));
        assert!(parse(Some("spin"), Some("x"), None).is_err());
        assert!(parse(Some("hybrid"), None, Some("-1")).is_err());
        assert!(parse(Some("spin"), None, Some("100"))
            .unwrap_err()
            .contains("hybrid only"));
        assert!(parse(None, Some("3"), None)
            .unwrap_err()
            .contains("not pinned"));
    }

    #[test]
    fn spin_and_off_never_read_the_clock() {
        let clock = || -> Instant { panic!("no clock in spin or off") };
        assert_eq!(Wait::new(Mode::Spin).timeout(clock), 0);
        assert_eq!(Wait::new(Mode::Off).timeout(clock), -1);
        let mut w = Wait::new(Mode::Spin);
        w.saw_events(Instant::now());
        assert_eq!(w.timeout(clock), 0);
    }

    #[test]
    fn hybrid_state_machine() {
        let idle = Duration::from_micros(200);
        let mut w = Wait::new(Mode::Hybrid { idle });
        let t0 = Instant::now();
        // No event yet: blocked.
        assert_eq!(w.timeout(|| t0), -1);
        // An event: spin until `idle` has passed since it.
        w.saw_events(t0);
        assert_eq!(w.timeout(|| t0), 0);
        assert_eq!(w.timeout(|| t0 + Duration::from_micros(199)), 0);
        assert_eq!(w.timeout(|| t0 + idle), -1);
        assert_eq!(w.timeout(|| t0 + Duration::from_millis(5)), -1);
        // The next event (after a blocking wait) starts a new spin window.
        let t1 = t0 + Duration::from_millis(5);
        w.saw_events(t1);
        assert_eq!(w.timeout(|| t1 + Duration::from_micros(10)), 0);
        // Events inside the window extend it.
        let t2 = t1 + Duration::from_micros(150);
        w.saw_events(t2);
        assert_eq!(w.timeout(|| t1 + Duration::from_micros(300)), 0);
        assert_eq!(w.timeout(|| t2 + idle), -1);
        // A clock that reads earlier than the last event (not monotonic across calls in
        // a test) keeps spinning rather than panicking.
        assert_eq!(w.timeout(|| t0), 0);
        // Zero idle: hybrid degenerates to blocking.
        let mut z = Wait::new(Mode::Hybrid {
            idle: Duration::ZERO,
        });
        z.saw_events(t0);
        assert_eq!(z.timeout(|| t0), -1);
    }

    #[test]
    fn adaptive_window_settings() {
        let hy = Mode::Hybrid {
            idle: Duration::from_micros(200),
        };
        assert_eq!(parse_adapt(hy, None), Ok(None));
        assert_eq!(parse_adapt(hy, Some(" ")), Ok(None));
        assert_eq!(parse_adapt(hy, Some("0")), Ok(None));
        assert_eq!(
            parse_adapt(hy, Some("3000")),
            Ok(Some(Duration::from_millis(3)))
        );
        assert!(parse_adapt(hy, Some("100")).unwrap_err().contains("below"));
        assert!(parse_adapt(hy, Some("x")).is_err());
        assert!(parse_adapt(Mode::Spin, Some("3000"))
            .unwrap_err()
            .contains("hybrid only"));
        assert!(parse_adapt(Mode::Off, Some("3000")).is_err());
        // Spin and off ignore a cap handed to them.
        let w = Wait::new(Mode::Spin).with_adapt(Some(Duration::from_millis(3)));
        assert_eq!(w.timeout(|| panic!("no clock")), 0);
    }

    #[test]
    fn adaptive_window_follows_the_gaps_and_stays_bounded() {
        let idle = Duration::from_micros(200);
        let cap = Duration::from_millis(3);
        let mut w = Wait::new(Mode::Hybrid { idle }).with_adapt(Some(cap));
        let t0 = Instant::now();
        let us = |n: u64| Duration::from_micros(n);
        w.saw_events(t0);
        assert_eq!(w.window(), idle);
        // Submissions every 1.5 ms: the window grows to 2.25 ms, so the next one is met
        // spinning.
        let mut t = t0;
        for _ in 0..4 {
            t += us(1500);
            w.saw_events(t);
        }
        assert_eq!(w.window(), us(2250));
        assert_eq!(w.timeout(|| t + us(1600)), 0);
        assert_eq!(w.timeout(|| t + us(2250)), -1);
        // A gap above the cap (the VM went idle) is not learned; the window never exceeds
        // the cap.
        t += Duration::from_secs(1);
        w.saw_events(t);
        assert_eq!(w.window(), us(2250));
        for _ in 0..3 {
            t += us(2900);
            w.saw_events(t);
        }
        assert_eq!(w.window(), cap);
        // Short gaps only: after ADAPT_GAPS of them the long ones are forgotten and the
        // window falls back to the idle time.
        for _ in 0..ADAPT_GAPS {
            t += us(20);
            w.saw_events(t);
        }
        assert_eq!(w.window(), idle);
        // Without a cap the window is the fixed idle time whatever the gaps.
        let mut f = Wait::new(Mode::Hybrid { idle });
        f.saw_events(t0);
        f.saw_events(t0 + us(1500));
        assert_eq!(f.window(), idle);
    }

    #[test]
    fn histogram_quantiles() {
        let h = Histogram::default();
        assert_eq!(h.quantile_us(0.99), 0.0);
        for _ in 0..99 {
            h.record(Duration::from_nanos(1_050));
        }
        h.record(Duration::from_micros(500));
        assert_eq!(h.count(), 100);
        assert_eq!(h.quantile_us(0.5), 1.1);
        assert_eq!(h.quantile_us(0.99), 1.1);
        // The overflow bucket reports the maximum.
        assert_eq!(h.quantile_us(1.0), 500.0);
        assert!((h.mean_us() - (99.0 * 1.05 + 500.0) / 100.0).abs() < 1e-9);
    }

    #[test]
    fn cpu_choice() {
        let before = "cpu  1 2 3 4 5 6 7 8 0 0\ncpu0 0 0 0 0 0 0 0 0\ncpu1 0 0 0 0 0 0 0 0\ncpu2 0 0 0 0 0 0 0 0\ncpu3 0 0 0 0 0 0 0 0\nintr 1\n";
        // cpu1 50 % busy, cpu2 and cpu3 idle (idle + iowait).
        let after = "cpu  9 9 9 9 9 9 9 9 0 0\ncpu0 100 0 0 0 0 0 0 0\ncpu1 50 0 0 50 0 0 0 0\ncpu2 0 0 0 90 10 0 0 0\ncpu3 0 0 0 100 0 0 0 0\n";
        let idle = idle_fractions(before, after);
        assert_eq!(idle.len(), 4, "the aggregate line is not a CPU");
        assert_eq!(idle[1], (1, 0.5));
        assert_eq!(idle[2], (2, 1.0));
        // Highest-numbered idle core.
        assert_eq!(choose_cpu(&[1, 2, 3], &[], &idle), Some(3));
        // Not where a vCPU runs.
        assert_eq!(choose_cpu(&[1, 2, 3], &[3], &idle), Some(2));
        // Only busy ones left: the most idle.
        assert_eq!(choose_cpu(&[1, 2], &[2], &idle), Some(1));
        // Every candidate hosts a vCPU: choose among them anyway.
        assert_eq!(choose_cpu(&[2, 3], &[2, 3], &idle), Some(3));
        assert_eq!(choose_cpu(&[], &[], &idle), None);
    }

    #[test]
    fn candidates_exclude_cpu0_and_check_the_mask() {
        let allowed = allowed_cpus().unwrap();
        assert!(!allowed.is_empty());
        assert_eq!(candidates(Pin::None), Ok(Vec::new()));
        let c = candidates(Pin::Cpu(allowed[0])).unwrap();
        assert_eq!(c, vec![allowed[0]]);
        assert!(candidates(Pin::Cpu(libc::CPU_SETSIZE as usize + 5)).is_err());
        match candidates(Pin::Auto) {
            Ok(c) => assert!(!c.contains(&0)),
            Err(e) => assert_eq!(allowed, vec![0], "{e}"),
        }
        // Off never pins, whatever the pin says.
        let r = Reactor::new(
            "t",
            Config {
                mode: Mode::Off,
                pin: Pin::Cpu(libc::CPU_SETSIZE as usize + 5),
            },
            None,
        )
        .unwrap();
        assert!(r.cpus.is_empty());
    }

    #[test]
    fn reactor_rings_in_every_mode() {
        use std::io::Write;
        use std::os::fd::FromRawFd;
        use std::sync::mpsc;
        for mode in [
            Mode::Off,
            Mode::Spin,
            Mode::Hybrid {
                idle: Duration::from_micros(50),
            },
        ] {
            let mut writers = Vec::new();
            let mut readers = Vec::new();
            for _ in 0..3 {
                // SAFETY: a plain syscall; checked.
                let raw = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC) };
                assert!(raw >= 0);
                // SAFETY: a new descriptor.
                let f = unsafe { File::from_raw_fd(raw) };
                writers.push(f.try_clone().unwrap());
                readers.push(f);
            }
            let (tx, rx) = mpsc::channel();
            let r = Reactor::new(
                "test",
                Config {
                    mode,
                    pin: Pin::None,
                },
                None,
            )
            .unwrap();
            let stats = r
                .spawn(readers, move |rings, seen, st| {
                    for &(i, w) in rings {
                        st.ring(seen, Some(Instant::now()));
                        tx.send((i, w)).unwrap();
                    }
                })
                .unwrap();
            (&writers[2]).write_all(&1u64.to_ne_bytes()).unwrap();
            assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), (2, 1));
            (&writers[0]).write_all(&3u64.to_ne_bytes()).unwrap();
            assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), (0, 3));
            assert_eq!(stats.events.load(Relaxed), 2);
            assert_eq!(stats.writes.load(Relaxed), 4);
            assert_eq!(stats.ring_to_host.count(), 2);
            assert!(stats.summary("test").contains(mode.name()));
            if mode == Mode::Spin {
                assert_eq!(stats.blocking_waits.load(Relaxed), 0);
            }
            if mode == Mode::Off {
                assert_eq!(
                    stats.iterations.load(Relaxed),
                    stats.blocking_waits.load(Relaxed)
                );
            }
            // End the loop; one more write wakes a blocked one.
            stats.stop.store(true, Relaxed);
            (&writers[1]).write_all(&1u64.to_ne_bytes()).unwrap();
        }
    }
}

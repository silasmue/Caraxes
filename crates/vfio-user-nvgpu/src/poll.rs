// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The doorbell reactor (`CARAXES_DOORBELL_POLL=off|spin|hybrid`, docs/caraxes/doorbell-poll.md).
//!
//! The doorbell eventfds (the token pool's, tokens.rs, or the token-less one) are read by
//! one thread over one epoll set. `off` blocks in `epoll_wait` until KVM signals an
//! eventfd, so every ring pays the scheduler wake-up of that thread. `spin` calls
//! `epoll_wait` with timeout 0 in a loop, as an SPDK reactor polls its queues: one
//! syscall per iteration over all eventfds, the thread never sleeps. `hybrid` spins for
//! [`DEFAULT_IDLE_US`] after the last event, then blocks until the next one and spins
//! again. A spinning reader pins itself to a core chosen at start, since a spinning
//! thread the scheduler moves or preempts loses what it gains. With host execution the
//! default is `hybrid` (every production launcher's setting until 2026-09-30); `spin`
//! trades a core for the last microseconds.
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

/// The knob: `off`, `spin` or `hybrid`; unset, `hybrid` with host execution, else `off`.
pub const POLL_ENV: &str = "CARAXES_DOORBELL_POLL";
/// How long hybrid mode keeps spinning after the last event.
pub const DEFAULT_IDLE_US: u64 = 200;

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
    /// Not pinned: the blocking reader of `off` (and the unit tests' readers).
    None,
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

/// The reactor configuration: [`POLL_ENV`], or unset the default for the mode (`hybrid`
/// with host execution, `off` without).
pub fn requested(host_exec: bool) -> Result<Config, String> {
    parse(std::env::var(POLL_ENV).ok().as_deref(), host_exec)
}

/// The mode as [`POLL_ENV`] gives it (`None`: unset, the mode's default). A polling mode
/// pins its reader to a core chosen at start.
pub fn parse(mode: Option<&str>, host_exec: bool) -> Result<Config, String> {
    let default = if host_exec { "hybrid" } else { "off" };
    let mode = match mode.map(str::trim).filter(|v| !v.is_empty()).unwrap_or(default) {
        "off" | "0" => Mode::Off,
        "spin" => Mode::Spin,
        "hybrid" => Mode::Hybrid {
            idle: Duration::from_micros(DEFAULT_IDLE_US),
        },
        other => return Err(format!("{POLL_ENV}={other:?}: off, spin or hybrid")),
    };
    let pin = if mode == Mode::Off { Pin::None } else { Pin::Auto };
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
            window,
        }
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
        self.last_event = Some(at);
    }
}

/// Durations in 100 ns buckets up to 200 us, then 64 buckets per doubling.
/// The tail covers all u64 nanoseconds so millisecond stalls do not collapse
/// into an overflow bucket whose quantiles would misleadingly equal the maximum.
/// Written by the reader only, read at exit.
pub struct Histogram {
    buckets: Box<[AtomicU64]>,
    count: AtomicU64,
    sum_ns: AtomicU64,
    max_ns: AtomicU64,
}

const BUCKET_NS: u64 = 100;
const BUCKETS: usize = 2000;
const TAIL_START_NS: u64 = BUCKET_NS * BUCKETS as u64;
const TAIL_SUB_BUCKETS: usize = 64;
const TAIL_GROUPS: usize = (u64::BITS - (u64::MAX / TAIL_START_NS).leading_zeros()) as usize;

fn duration_bucket(ns: u64) -> usize {
    if ns < TAIL_START_NS {
        return (ns / BUCKET_NS) as usize;
    }
    let group = (u64::BITS - 1 - (ns / TAIL_START_NS).leading_zeros()) as usize;
    let base = TAIL_START_NS << group;
    let width = base / TAIL_SUB_BUCKETS as u64;
    BUCKETS + group * TAIL_SUB_BUCKETS + ((ns - base) / width) as usize
}

fn duration_bucket_upper_ns(bucket: usize) -> u128 {
    if bucket < BUCKETS {
        return (bucket as u128 + 1) * u128::from(BUCKET_NS);
    }
    let tail = bucket - BUCKETS;
    let base = u128::from(TAIL_START_NS) << (tail / TAIL_SUB_BUCKETS);
    base + (tail % TAIL_SUB_BUCKETS + 1) as u128 * (base / TAIL_SUB_BUCKETS as u128)
}

impl Default for Histogram {
    fn default() -> Self {
        Histogram {
            buckets: (0..BUCKETS + TAIL_GROUPS * TAIL_SUB_BUCKETS)
                .map(|_| AtomicU64::new(0)).collect(),
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
        let b = duration_bucket(ns);
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

    /// The upper edge of the bucket holding quantile `q` (0..1), in microseconds,
    /// capped by the observed maximum for the logarithmic tail.
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
                let upper = duration_bucket_upper_ns(i) as f64 / 1000.0;
                if i >= BUCKETS {
                    return upper.min(self.max_us());
                }
                return upper;
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
            Mode::Hybrid { idle } => format!("hybrid ({} us idle)", idle.as_micros()),
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
        Pin::Auto => {
            let c: Vec<usize> = allowed.into_iter().filter(|&c| c != 0).collect();
            if c.is_empty() {
                Err("no CPU other than 0 allowed for this process: a polling reader has no core to pin to; run it with more CPUs or CARAXES_DOORBELL_POLL=off (without host execution)".into())
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
        Ok(Reactor {
            name,
            config,
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
        let st = stats.clone();
        let name = self.name;
        std::thread::Builder::new()
            .name(format!("doorbell-{name}"))
            .spawn(move || {
                // SAFETY: gettid has no preconditions.
                st.tid.store(i64::from(unsafe { libc::gettid() }), Relaxed);
                if self.config.mode != Mode::Off {
                    let pinned = match self.config.pin {
                        Pin::None => None,
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
                let wait = Wait::new(self.config.mode);
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

    /// Unset: hybrid with host execution, off without; a polling mode pins at start.
    #[test]
    fn mode_selection() {
        let off = Config {
            mode: Mode::Off,
            pin: Pin::None,
        };
        let hybrid = Config {
            mode: Mode::Hybrid {
                idle: Duration::from_micros(DEFAULT_IDLE_US),
            },
            pin: Pin::Auto,
        };
        assert_eq!(parse(None, false), Ok(off));
        assert_eq!(parse(Some(""), false), Ok(off));
        assert_eq!(parse(None, true), Ok(hybrid));
        assert_eq!(parse(Some(" "), true), Ok(hybrid));
        assert_eq!(parse(Some("off"), true), Ok(off));
        assert_eq!(parse(Some("0"), false), Ok(off));
        assert_eq!(parse(Some("hybrid"), false), Ok(hybrid));
        assert_eq!(
            parse(Some(" spin "), true),
            Ok(Config {
                mode: Mode::Spin,
                pin: Pin::Auto
            })
        );
        assert!(parse(Some("busy"), true)
            .unwrap_err()
            .contains("off, spin or hybrid"));
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
        // The tail's upper bound is capped by the observed maximum.
        assert_eq!(h.quantile_us(1.0), 500.0);
        assert!((h.mean_us() - (99.0 * 1.05 + 500.0) / 100.0).abs() < 1e-9);
    }

    #[test]
    fn histogram_millisecond_quantiles_do_not_report_the_maximum_outlier() {
        let h = Histogram::default();
        for _ in 0..99 {
            h.record(Duration::from_micros(7_000));
        }
        h.record(Duration::from_micros(60_014));
        for q in [0.5, 0.99] {
            assert!((7_000.0..=7_100.0).contains(&h.quantile_us(q)));
        }
        assert_eq!(h.max_us(), 60_014.0);
        assert_eq!(h.quantile_us(1.0), 60_014.0);
    }

    #[test]
    fn histogram_tail_bounds_cover_every_duration_scale() {
        for group in 0..TAIL_GROUPS {
            let base = TAIL_START_NS << group;
            for ns in [base - 1, base, base.saturating_add(base / 2), base.saturating_mul(2) - 1] {
                let bucket = duration_bucket(ns);
                assert!(bucket < BUCKETS + TAIL_GROUPS * TAIL_SUB_BUCKETS);
                assert!(duration_bucket_upper_ns(bucket) > u128::from(ns));
                if bucket > 0 {
                    assert!(duration_bucket_upper_ns(bucket - 1) <= u128::from(ns));
                }
            }
        }
        let h = Histogram::default();
        h.record(Duration::from_nanos(u64::MAX));
        assert_eq!(h.quantile_us(0.5), h.max_us());
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
        match candidates(Pin::Auto) {
            Ok(c) => assert!(!c.contains(&0)),
            Err(e) => assert_eq!(allowed, vec![0], "{e}"),
        }
        // Off never pins, whatever the pin says.
        let r = Reactor::new(
            "t",
            Config {
                mode: Mode::Off,
                pin: Pin::Auto,
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

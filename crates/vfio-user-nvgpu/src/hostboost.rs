// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The host clock boost (docs/research/host-boost-2026-09-28.md).
//!
//! Natively every CUDA context sends `NV2080_CTRL_CMD_PERF_BOOST` {BOOST_TO_MAX | CUDA,
//! duration INFINITE}, and the card leaves its idle P-state. In a guest that request
//! reaches the device as `INTERNAL_PERF_BOOST_SET_2X` (0x20800a9a, cuda.rs), and the host
//! card ran the guest's work at P8 (video 540 MHz, memory 405 MHz, PCIe gen 1;
//! caraxes/runs/night-2026-09-28/clocks-*.csv).
//!
//! What the device does:
//!
//! - With host execution, `CARAXES_HOST_BOOST_POLICY=vm` (default) requests the
//!   highest P-state immediately and throughout the backend process's lifetime.
//!   `activity` retains the earlier doorbell/event-driven policy. Every request is
//!   finite ([`DURATION_S`] seconds), renewed at most every [`RENEW`], with refusal
//!   backoff. Process termination stops renewals; the last accepted hint is requested
//!   to expire after at most three seconds, subject to the host driver's semantics.
//! - `PERF_BOOST` requests a P-state, not a frequency. Other host limits and dynamic
//!   frequency policy still apply; NV_OK is not proof of achieved clocks.
//! - VM policy always sends BOOST_TO_MAX without optional guest flags. Activity policy
//!   keeps the guest's command (and permitted MULTIMEDIA). Both drop CUDA, priority,
//!   ASYNC and guest duration, avoiding the shared reference-counted CUDA hint.
//! - Activity comes from guest doorbells, mirrored non-stall events and guest boost
//!   requests. [`activity`] retains its existing bounded atomic hot path. The RM call
//!   runs on the separate `host-boost` worker, never under the device mutex.
//!
//! See docs/research/host-boost-vm-policy.md for the policy/lifecycle contract and
//! docs/research/host-boost-2026-09-28.md for the historical activity-policy research.

use core::mem::{offset_of, size_of};
use std::sync::atomic::{
    AtomicU32, AtomicU8,
    Ordering::{AcqRel, Relaxed, Release},
};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::ctrl::HostRm;
use crate::ver::common::{self as v, NV20_SUBDEVICE_0, NV_OK};


/// `NV2080_CTRL_CMD_PERF_BOOST` (615 ctrl2080perf.h:99, 610 :95). NON_PRIVILEGED: export
/// flags 0x10318 (615 g_subdevice_nvoc.c:11198, 610 :11032).
pub const CMD_PERF_BOOST: u32 = 0x2080_200A;

/// `NV2080_CTRL_PERF_BOOST_FLAGS_*` (615 ctrl2080perf.h:75-94; 610 :75-90 has no MULTIMEDIA).
pub const FLAGS_CMD: u32 = 0x3;
pub const CMD_CLEAR: u32 = 0x0;
pub const CMD_BOOST_1LEVEL: u32 = 0x1;
pub const CMD_BOOST_TO_MAX: u32 = 0x2;
pub const FLAG_CUDA: u32 = 1 << 4;
pub const FLAG_ASYNC: u32 = 1 << 5;
pub const FLAG_CUDA_PRIORITY: u32 = 1 << 6;
pub const FLAG_MULTIMEDIA: u32 = 1 << 7;
// Every defined bit survives the guest kernel's copy into the NvBool of
// `INTERNAL_PERF_BOOST_SET_2X` (kern_perf_boost.c:112).
const DEFINED_FLAGS: u32 =
    FLAGS_CMD | FLAG_CUDA | FLAG_ASYNC | FLAG_CUDA_PRIORITY | FLAG_MULTIMEDIA;
const _: () = assert!(DEFINED_FLAGS >> 8 == 0);
/// `NV2080_CTRL_PERF_BOOST_DURATION_MAX` and `_INFINITE`; durations are seconds
/// (615 ctrl2080perf.h:66-67, :96-97).
pub const DURATION_MAX: u32 = 3600;
pub const DURATION_INFINITE: u32 = 0xFFFF_FFFF;

/// The duration of every boost the device sends, in seconds.
pub const DURATION_S: u32 = 3;
/// Renewal cadence. VM policy keeps renewing while the backend lives; activity
/// policy stops after a quiet interval.
pub const RENEW: Duration = Duration::from_secs(1);
/// After a refusal: no call for this long, doubling per refusal up to [`BACKOFF_MAX`].
pub const BACKOFF_FIRST: Duration = Duration::from_secs(5);
pub const BACKOFF_MAX: Duration = Duration::from_secs(300);
/// The flags of an activity renewal: to the highest P-state, not a CUDA boost.
pub const DEFAULT_FLAGS: u32 = CMD_BOOST_TO_MAX;

// A boost outlives two renewal intervals: one late renewal (a busy host) never lets the
// finite hint expire between those active intervals; this is not a MHz guarantee.
const _: () = assert!(DURATION_S as u64 > 2 * RENEW.as_secs());
const _: () = assert!(DURATION_S < DURATION_MAX);

/// `NV2080_CTRL_PERF_BOOST_PARAMS` {NvU32 flags; NvU32 duration}: the same in 610
/// (ctrl2080perf.h:97-102) and 615 (:103-106). `nvrm-sys` does not bind ctrl2080perf.h.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PerfBoostParams {
    pub flags: u32,
    pub duration: u32,
}

pub const PARAMS_SIZE: usize = size_of::<PerfBoostParams>();

impl PerfBoostParams {
    pub fn bytes(self) -> [u8; PARAMS_SIZE] {
        let mut b = [0u8; PARAMS_SIZE];
        b[offset_of!(PerfBoostParams, flags)..][..4].copy_from_slice(&self.flags.to_le_bytes());
        b[offset_of!(PerfBoostParams, duration)..][..4]
            .copy_from_slice(&self.duration.to_le_bytes());
        b
    }
}

/// The flags the device sends for a guest request with `guest`, on a host that defines
/// the bits `host_flags` (`HostLayouts::perf_boost_flags`); `None` for a clear. The
/// command stays (an undefined command 3 is BOOST_TO_MAX); CUDA, CUDA_PRIORITY and ASYNC
/// never pass, whatever the host defines.
pub fn forwarded(guest: u32, host_flags: u32) -> Option<u32> {
    let cmd = match guest & FLAGS_CMD {
        CMD_CLEAR => return None,
        CMD_BOOST_1LEVEL => CMD_BOOST_1LEVEL,
        _ => CMD_BOOST_TO_MAX,
    };
    let keep = host_flags & !(FLAGS_CMD | FLAG_CUDA | FLAG_CUDA_PRIORITY | FLAG_ASYNC);
    Some(cmd | (guest & keep))
}

/// Explicit renewal policy; the environment is validated before host allocation.
pub const POLICY_ENV: &str = "CARAXES_HOST_BOOST_POLICY";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    Vm,
    Activity,
}

impl Policy {
    pub fn name(self) -> &'static str {
        match self {
            Self::Vm => "vm",
            Self::Activity => "activity",
        }
    }
}

pub fn parse_policy(value: Option<&str>) -> Result<Policy, String> {
    match value {
        None | Some("vm") => Ok(Policy::Vm),
        Some("activity") => Ok(Policy::Activity),
        Some(value) => Err(format!("{POLICY_ENV}={value:?}: vm or activity (canonical spelling)")),
    }
}

pub fn requested_policy() -> Result<Policy, String> {
    match std::env::var(POLICY_ENV) {
        Ok(value) => parse_policy(Some(&value)),
        Err(std::env::VarError::NotPresent) => parse_policy(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{POLICY_ENV} must be UTF-8: vm or activity")),
    }
}

// ---------------------------------------------------------------------------------
// The signal between the hot path and the worker.

const OFF: u8 = 0;
/// The worker runs or sleeps until its next renewal; activity only marks itself.
const ARMED: u8 = 1;
/// Activity since the worker last looked.
const PENDING: u8 = 2;
/// The worker waits for the next activity, which must wake it.
const PARKED: u8 = 3;

const GUEST_CLEAR: u32 = 1 << 30;
const GUEST_BOOST: u32 = 1 << 31;

/// What the guest asked since the worker last looked (the latest request wins).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuestNews {
    None,
    /// A boost with these (low-byte) guest flags.
    Boost(u32),
    Clear,
}

/// One state word, so the worker's "park unless activity arrived" and the hot path's
/// "mark, and wake if parked" are ordered by the word alone.
pub struct Signal {
    state: AtomicU8,
    policy: AtomicU8,
    guest: AtomicU32,
}

impl Default for Signal {
    fn default() -> Self {
        Self::new()
    }
}

impl Signal {
    pub const fn new() -> Self {
        Signal {
            state: AtomicU8::new(OFF),
            policy: AtomicU8::new(Policy::Activity as u8),
            guest: AtomicU32::new(0),
        }
    }

    /// Whether a worker takes the activity.
    pub fn on(&self) -> bool {
        self.state.load(std::sync::atomic::Ordering::Acquire) != OFF
    }

    fn arm(&self, policy: Policy) {
        self.policy.store(policy as u8, Release);
        self.state.store(ARMED, Release);
    }

    fn policy(&self) -> Policy {
        if self.policy.load(std::sync::atomic::Ordering::Acquire) == Policy::Vm as u8 {
            Policy::Vm
        } else {
            Policy::Activity
        }
    }

    /// Mark activity; true when the worker is parked and must be woken. Off, or with a
    /// mark already set, this is one relaxed load.
    #[inline]
    pub fn note(&self) -> bool {
        let s = self.state.load(Relaxed);
        if s == OFF || s == PENDING {
            return false;
        }
        self.note_slow(s)
    }

    #[cold]
    #[inline(never)]
    fn note_slow(&self, mut s: u8) -> bool {
        loop {
            match self
                .state
                .compare_exchange_weak(s, PENDING, AcqRel, Relaxed)
            {
                Ok(_) => return s == PARKED,
                Err(OFF) | Err(PENDING) => return false,
                Err(now) => s = now,
            }
        }
    }

    /// The worker: whether activity was marked since the last call.
    fn take(&self) -> bool {
        self.state.swap(ARMED, AcqRel) == PENDING
    }

    /// The worker, before it parks: false when activity arrived since [`Self::take`].
    fn park_if_quiet(&self) -> bool {
        self.state
            .compare_exchange(ARMED, PARKED, AcqRel, Relaxed)
            .is_ok()
    }

    fn post_guest(&self, flags: u32) {
        let news = if flags & FLAGS_CMD == CMD_CLEAR {
            GUEST_CLEAR
        } else {
            GUEST_BOOST | (flags & 0xFF)
        };
        self.guest.store(news, Release);
    }

    fn take_guest(&self) -> GuestNews {
        match self.guest.swap(0, AcqRel) {
            0 => GuestNews::None,
            GUEST_CLEAR => GuestNews::Clear,
            n => GuestNews::Boost(n & 0xFF),
        }
    }
}

// ---------------------------------------------------------------------------------
// The renewal logic, on the worker, with the time passed in.

/// What the worker counts; the exit summary prints it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub renewals: u64,
    pub from_guest: u64,
    pub refused: u64,
    pub first: Option<Instant>,
    pub calls: u64,
    pub call_ns_sum: u64,
    pub call_ns_max: u64,
}

impl Counts {
    /// "host boost: N renewals (M from guest requests), K refused, first at T; ...".
    pub fn line(&self, epoch: Instant) -> String {
        let first = self.first.map_or("never".into(), |t| {
            format!("+{:.3} s", t.saturating_duration_since(epoch).as_secs_f64())
        });
        let mean = self.call_ns_sum.checked_div(self.calls).unwrap_or(0) / 1000;
        format!(
            "host boost: {} renewals ({} from guest requests), {} refused, first at {first}; RM call mean {mean} us, max {} us over {} call(s)\n",
            self.renewals,
            self.from_guest,
            self.refused,
            self.call_ns_max / 1000,
            self.calls
        )
    }
}

/// The worker's next move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tick {
    /// Send `PERF_BOOST` with these params now.
    Boost(PerfBoostParams),
    /// Look again after this long (or at the next wake).
    Wait(Duration),
    /// No activity for a whole interval: wait for the next one.
    Idle,
}

pub struct Renewer {
    epoch: Instant,
    policy: Policy,
    host_flags: u32,
    /// VM uses only DEFAULT_FLAGS; activity uses the last permitted guest request.
    flags: u32,
    /// Activity since the last renewal.
    owed: bool,
    /// That activity includes a guest request.
    owed_guest: bool,
    /// The last renewal the host took, and its flags.
    last: Option<Instant>,
    sent_flags: u32,
    retry_at: Option<Instant>,
    backoff: Duration,
    /// The call in flight: (flags, carries a guest request).
    inflight: Option<(u32, bool)>,
    pub counts: Counts,
}

impl Renewer {
    pub fn new(host_flags: u32, epoch: Instant, policy: Policy) -> Self {
        Renewer {
            epoch,
            policy,
            host_flags,
            flags: DEFAULT_FLAGS,
            owed: false,
            owed_guest: false,
            last: None,
            sent_flags: DEFAULT_FLAGS,
            retry_at: None,
            backoff: BACKOFF_FIRST,
            inflight: None,
            counts: Counts::default(),
        }
    }

    /// Fold in what happened since the last tick and decide. A renewal is due for owed
    /// activity when none was sent, the last is [`RENEW`] old, or the flags changed.
    /// VM policy is always owed, including its very first quiet worker step.
    pub fn tick(&mut self, now: Instant, active: bool, guest: GuestNews) -> Tick {
        match self.policy {
            Policy::Vm => {
                // Backend lifetime is the lease owner, regardless of activity or a
                // guest's downgrade/clear. Optional guest flags are not forwarded.
                self.flags = DEFAULT_FLAGS;
                self.owed = true;
                self.owed_guest |= matches!(guest, GuestNews::Boost(_));
            }
            Policy::Activity => {
                match guest {
                    GuestNews::Boost(f) => {
                        if let Some(flags) = forwarded(f, self.host_flags) {
                            self.flags = flags;
                            self.owed = true;
                            self.owed_guest = true;
                        }
                    }
                    GuestNews::Clear => self.flags = DEFAULT_FLAGS,
                    GuestNews::None => {}
                }
                self.owed |= active;
            }
        }
        if let Some(t) = self.retry_at {
            if now < t {
                return Tick::Wait(t - now);
            }
            self.retry_at = None;
        }
        let since = self.last.map(|t| now.saturating_duration_since(t));
        let due = since.is_none_or(|d| d >= RENEW) || self.flags != self.sent_flags;
        if self.owed && due {
            self.inflight = Some((self.flags, self.owed_guest));
            return Tick::Boost(PerfBoostParams {
                flags: self.flags,
                duration: DURATION_S,
            });
        }
        match since {
            Some(d) if d < RENEW => Tick::Wait(RENEW - d),
            _ => Tick::Idle,
        }
    }

    /// The host's answer to the [`Tick::Boost`] issued at `now`; a line to log for the
    /// first renewal and the first refusal, nothing after.
    pub fn done(&mut self, now: Instant, status: u32, took: Duration, completed: Instant) -> Option<String> {
        let (flags, guest) = self.inflight.take()?;
        let ns = took.as_nanos() as u64;
        self.counts.calls += 1;
        self.counts.call_ns_sum += ns;
        self.counts.call_ns_max = self.counts.call_ns_max.max(ns);
        self.owed = false;
        self.owed_guest = false;
        let what = format!("PERF_BOOST {{flags {flags:#x}, {DURATION_S} s}}");
        if status == NV_OK {
            self.last = Some(now);
            self.sent_flags = flags;
            self.backoff = BACKOFF_FIRST;
            self.counts.renewals += 1;
            self.counts.from_guest += guest as u64;
            if self.counts.first.is_some() {
                return None;
            }
            self.counts.first = Some(now);
            return Some(format!(
                "host boost: first renewal at +{:.3} s: {what} NV_OK in {} us",
                now.saturating_duration_since(self.epoch).as_secs_f64(),
                ns / 1000
            ));
        }
        self.counts.refused += 1;
        // Use actual completion, including any scheduling delay before control().
        // A slow refused ioctl must not consume its own bounded retry pause.
        self.retry_at = Some(completed + self.backoff);
        let line = (self.counts.refused == 1).then(|| {
            format!(
                "host boost: {what} refused with status {status:#x}; no call for {} s, the pause doubling up to {} s; later refusals are only counted",
                self.backoff.as_secs(),
                BACKOFF_MAX.as_secs()
            )
        });
        self.backoff = (self.backoff * 2).min(BACKOFF_MAX);
        line
    }
}

/// One worker step at `now`: take the marks, decide, and issue the boost if due.
pub fn step(
    sig: &Signal,
    r: &mut Renewer,
    host: &mut dyn HostRm,
    now: Instant,
) -> (Tick, Option<String>) {
    let active = sig.take();
    let guest = sig.take_guest();
    let tick = r.tick(now, active, guest);
    let mut line = None;
    if let Tick::Boost(p) = tick {
        let mut b = p.bytes();
        let t0 = Instant::now();
        let status = host.control(NV20_SUBDEVICE_0, CMD_PERF_BOOST, &mut b);
        let completed = Instant::now();
        line = r.done(now, status, completed.saturating_duration_since(t0), completed);
    }
    (tick, line)
}

// ---------------------------------------------------------------------------------
// The device's one booster.

static SIGNAL: Signal = Signal::new();
static WORKER: OnceLock<std::thread::Thread> = OnceLock::new();

#[derive(Clone)]
enum Report {
    Off { policy: Policy, why: String },
    On { policy: Policy, epoch: Instant, counts: Counts },
}

impl Report {
    fn line(&self) -> String {
        match self {
            Self::Off { policy, why } => format!("host boost policy={}: off ({why})\n", policy.name()),
            Self::On { policy, epoch, counts } => format!(
                "host boost policy={}: {}; P-state hints only, no MHz guarantee; finite {DURATION_S} s lease\n{}",
                policy.name(),
                match policy {
                    Policy::Vm => "continuous BOOST_TO_MAX",
                    Policy::Activity => "activity-driven guest command/permitted MULTIMEDIA",
                },
                counts.line(*epoch)
            ),
        }
    }
}

static REPORT: Mutex<Option<Report>> = Mutex::new(None);

/// The hot path: the VM did something (a doorbell, a host event of its channels).
#[inline]
pub fn activity() {
    if SIGNAL.note() {
        wake();
    }
}

fn wake() {
    if let Some(t) = WORKER.get() {
        t.unpark();
    }
}

/// The guest's `INTERNAL_PERF_BOOST_SET_2X` (cuda.rs): hand it to the worker; the note
/// for the device log.
pub fn guest_request(params: &[u8]) -> String {
    let (flags, what) = describe_guest(params);
    let note = request(&SIGNAL, flags, &what);
    wake();
    note
}

fn describe_guest(params: &[u8]) -> (u32, String) {
    let flags = params.get(v::PERF_BOOST_2X_FLAGS).map_or(0, |&b| b as u32);
    let duration = params
        .get(v::PERF_BOOST_2X_DURATION..v::PERF_BOOST_2X_DURATION + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let duration = match duration {
        Some(DURATION_INFINITE) => "infinite".to_string(),
        Some(d) => format!("{d} s"),
        None => "missing".to_string(),
    };
    (
        flags,
        format!("PERF_BOOST_SET_2X {{flags {flags:#x}, duration {duration}}}"),
    )
}

fn request(sig: &Signal, flags: u32, what: &str) -> String {
    if !sig.on() {
        return format!("{what}: nothing boosted (host boost off: no host execution, or it failed to start)");
    }
    sig.post_guest(flags);
    if sig.policy() == Policy::Vm {
        return format!("{what}: policy vm keeps BOOST_TO_MAX while the backend lives ({DURATION_S} s per renewal; guest command, flags and duration are not forwarded)");
    }
    if flags & FLAGS_CMD == CMD_CLEAR {
        // Not activity: the renewals go back to the default flags, and the boost lapses
        // once the VM is idle.
        return format!(
            "{what}: clear, not forwarded; the host boost lapses by itself once the VM is idle"
        );
    }
    sig.note();
    format!("{what}: to the host boost (the guest's command for {DURATION_S} s per renewal; its CUDA bits and duration are not forwarded)")
}

/// Start the worker with its own host RM client; `host_flags` is the host's
/// `HostLayouts::perf_boost_flags`.
pub fn start(host_flags: u32, policy: Policy) -> Result<(), String> {
    let host = crate::hostrm::HostRmClient::open().map_err(|e| format!("host RM client: {e:#}"))?;
    spawn(Box::new(host), host_flags, policy).map_err(|e| format!("spawn the host-boost thread: {e}"))
}

/// Start the worker on `host`.
pub fn spawn(host: Box<dyn HostRm>, host_flags: u32, policy: Policy) -> std::io::Result<()> {
    let epoch = Instant::now();
    *REPORT.lock().unwrap_or_else(|p| p.into_inner()) = Some(Report::On {
        policy,
        epoch,
        counts: Counts::default(),
    });
    // Armed before the thread runs: arming after it could overwrite the worker's PARKED
    // and leave it asleep while activity only marks itself.
    SIGNAL.arm(policy);
    let spawned = std::thread::Builder::new()
        .name("host-boost".into())
        .spawn(move || run(host, host_flags, epoch, policy));
    if spawned.is_err() {
        SIGNAL.state.store(OFF, Release);
    }
    spawned.map(|_| ())
}

/// Record why the booster does not run, for the exit summary.
pub fn set_off(policy: Policy, why: &str) {
    *REPORT.lock().unwrap_or_else(|p| p.into_inner()) = Some(Report::Off { policy, why: why.to_string() });
}

fn run(mut host: Box<dyn HostRm>, host_flags: u32, epoch: Instant, policy: Policy) {
    let _ = WORKER.set(std::thread::current());
    let mut r = Renewer::new(host_flags, epoch, policy);
    loop {
        let (tick, line) = step(&SIGNAL, &mut r, host.as_mut(), Instant::now());
        if let Some(l) = line {
            eprintln!("vfio-user-nvgpu: {l}");
        }
        match tick {
            Tick::Boost(_) => {
                if let Ok(mut rep) = REPORT.lock() {
                    *rep = Some(Report::On {
                        policy,
                        epoch,
                        counts: r.counts,
                    });
                }
            }
            Tick::Wait(d) => std::thread::park_timeout(d),
            Tick::Idle => {
                if SIGNAL.park_if_quiet() {
                    std::thread::park();
                }
            }
        }
    }
}

/// The exit summary line, `None` when the device never decided about a host boost.
pub fn summary() -> Option<String> {
    REPORT.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map(Report::line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct FakeHost {
        calls: Vec<(u32, u32, Vec<u8>)>,
        answers: VecDeque<u32>,
    }

    impl HostRm for FakeHost {
        fn control(&mut self, class: u32, cmd: u32, p: &mut [u8]) -> u32 {
            self.calls.push((class, cmd, p.to_vec()));
            self.answers.pop_front().unwrap_or(NV_OK)
        }
    }

    fn armed() -> Signal {
        let s = Signal::new();
        s.arm(Policy::Activity);
        s
    }

    /// Steps every `every` ms in [from, to), marking activity before each step when
    /// `active`; the ms of each boost sent.
    #[allow(clippy::too_many_arguments)]
    fn drive(
        sig: &Signal,
        r: &mut Renewer,
        host: &mut FakeHost,
        t0: Instant,
        from: u64,
        to: u64,
        every: u64,
        active: bool,
    ) -> (Vec<u64>, Tick) {
        let mut sent = Vec::new();
        let mut last = Tick::Idle;
        let mut ms = from;
        while ms < to {
            if active {
                sig.note();
            }
            let (tick, _) = step(sig, r, host, t0 + Duration::from_millis(ms));
            if matches!(tick, Tick::Boost(_)) {
                sent.push(ms);
            }
            last = tick;
            ms += every;
        }
        (sent, last)
    }

    const P: PerfBoostParams = PerfBoostParams {
        flags: CMD_BOOST_TO_MAX,
        duration: DURATION_S,
    };

    #[test]
    fn policy_parse_is_strict_and_defaults_to_vm() {
        assert_eq!(parse_policy(None).unwrap(), Policy::Vm);
        assert_eq!(parse_policy(Some("vm")).unwrap(), Policy::Vm);
        assert_eq!(parse_policy(Some("activity")).unwrap(), Policy::Activity);
        for value in ["", "off", "1", "VM", " vm", "vm ", "activity\n", "\u{a0}vm", "always"] {
            assert!(parse_policy(Some(value)).is_err(), "{value:?}");
        }
    }

    #[test]
    fn vm_policy_starts_quiet_and_renews_without_activity() {
        let (sig, t0) = (Signal::new(), Instant::now());
        sig.arm(Policy::Vm);
        let mut host = FakeHost::default();
        let mut r = Renewer::new(FLAGS_CMD, t0, Policy::Vm);
        let (sent, last) = drive(&sig, &mut r, &mut host, t0, 0, 5000, 10, false);
        assert_eq!(sent, [0, 1000, 2000, 3000, 4000]);
        assert!(matches!(last, Tick::Wait(_)), "VM policy must not idle/park between leases");
        assert_eq!((r.counts.renewals, r.counts.from_guest), (5, 0));
        assert!(host.calls.iter().all(|call| *call == (NV20_SUBDEVICE_0, CMD_PERF_BOOST, P.bytes().to_vec())));
        // Identical quiet input retains the prior activity-policy idle behavior.
        let sig = armed();
        let mut host = FakeHost::default();
        let mut r = Renewer::new(FLAGS_CMD, t0, Policy::Activity);
        let (sent, last) = drive(&sig, &mut r, &mut host, t0, 0, 5000, 10, false);
        assert!(sent.is_empty());
        assert_eq!(last, Tick::Idle);
        sig.note();
        assert_eq!(step(&sig, &mut r, &mut host, t0 + Duration::from_secs(5)).0, Tick::Boost(P));
    }

    #[test]
    fn vm_policy_guest_downgrade_clear_and_flags_cannot_break_cadence() {
        let (sig, t0) = (Signal::new(), Instant::now());
        sig.arm(Policy::Vm);
        let mut host = FakeHost::default();
        let mut r = Renewer::new(u32::MAX, t0, Policy::Vm);
        assert_eq!(step(&sig, &mut r, &mut host, t0).0, Tick::Boost(P));
        for (ms, flags) in [
            (100, CMD_BOOST_1LEVEL | FLAG_MULTIMEDIA | FLAG_CUDA | FLAG_ASYNC | FLAG_CUDA_PRIORITY),
            (200, CMD_CLEAR),
            (300, 0xff),
            (400, FLAG_CUDA),
        ] {
            let note = request(&sig, flags, "guest");
            assert!(note.contains("policy vm keeps BOOST_TO_MAX"));
            assert!(!note.contains("once the VM is idle"));
            assert_eq!(
                step(&sig, &mut r, &mut host, t0 + Duration::from_millis(ms)).0,
                Tick::Wait(Duration::from_millis(1000 - ms)),
                "guest updates must not cause extra within-interval requests"
            );
        }
        assert_eq!(step(&sig, &mut r, &mut host, t0 + RENEW).0, Tick::Boost(P));
        // Even a quiet clear after a successful renewal cannot stop the next lease.
        request(&sig, CMD_CLEAR, "clear");
        assert_eq!(step(&sig, &mut r, &mut host, t0 + RENEW * 2).0, Tick::Boost(P));
        assert_eq!(host.calls.len(), 3);
        assert!(host.calls.iter().all(|call| call.2 == P.bytes().to_vec()));
        assert_eq!(r.counts.from_guest, 1, "the included request is counted, not forwarded as flags");
    }

    #[test]
    fn vm_policy_quiet_refusal_backoff_and_clear_preserve_retry_then_reset() {
        let t0 = Instant::now();
        let mut r = Renewer::new(FLAGS_CMD, t0, Policy::Vm);
        assert_eq!(r.tick(t0, false, GuestNews::None), Tick::Boost(P));
        assert!(r.done(t0, 0x56, Duration::ZERO, t0).unwrap().contains("5 s"));
        assert_eq!(r.tick(t0 + Duration::from_secs(1), false, GuestNews::Clear), Tick::Wait(Duration::from_secs(4)));
        let t = t0 + Duration::from_secs(5);
        assert_eq!(r.tick(t, false, GuestNews::None), Tick::Boost(P));
        assert!(r.done(t, 0x56, Duration::ZERO, t).is_none());
        assert_eq!(r.tick(t0 + Duration::from_secs(14), false, GuestNews::None), Tick::Wait(Duration::from_secs(1)));
        let t = t0 + Duration::from_secs(15);
        assert_eq!(r.tick(t, false, GuestNews::None), Tick::Boost(P));
        assert!(r.done(t, NV_OK, Duration::ZERO, t).unwrap().contains("first renewal"));
        let t = t0 + Duration::from_secs(16);
        assert_eq!(r.tick(t, false, GuestNews::None), Tick::Boost(P));
        r.done(t, 0x56, Duration::ZERO, t);
        assert_eq!((r.counts.renewals, r.counts.refused, r.counts.calls), (1, 3, 4));
        assert_eq!(r.tick(t0 + Duration::from_secs(20), false, GuestNews::None), Tick::Wait(Duration::from_secs(1)));
        assert_eq!(r.tick(t0 + Duration::from_secs(21), false, GuestNews::None), Tick::Boost(P));
        r.done(t0 + Duration::from_secs(21), NV_OK, Duration::ZERO, t0 + Duration::from_secs(21));
        assert_eq!(r.backoff, BACKOFF_FIRST);
    }

    #[test]
    fn vm_policy_slow_refusal_gets_full_completion_pause_and_caps_backoff() {
        let t0 = Instant::now();
        let mut r = Renewer::new(FLAGS_CMD, t0, Policy::Vm);
        assert_eq!(r.tick(t0, false, GuestNews::None), Tick::Boost(P));
        r.done(t0, 0x56, Duration::from_secs(7), t0 + Duration::from_secs(9));
        assert_eq!(r.tick(t0 + Duration::from_secs(9), false, GuestNews::Clear), Tick::Wait(BACKOFF_FIRST));
        let mut due = t0 + Duration::from_secs(14);
        for pause in [10, 20, 40, 80, 160, 300, 300] {
            assert_eq!(r.tick(due, false, GuestNews::None), Tick::Boost(P));
            r.done(due, 0x56, Duration::from_secs(2), due + Duration::from_secs(2));
            let completed = due + Duration::from_secs(2);
            assert_eq!(r.tick(completed, false, GuestNews::None), Tick::Wait(Duration::from_secs(pause)));
            due = completed + Duration::from_secs(pause);
        }
        assert_eq!(r.tick(due, false, GuestNews::None), Tick::Boost(P));
        r.done(due, NV_OK, Duration::ZERO, due);
        assert_eq!(r.backoff, BACKOFF_FIRST);
        // Activity also gets a full completion-based pause; its guest flags remain.
        let mut old = Renewer::new(FLAGS_CMD, t0, Policy::Activity);
        let one = PerfBoostParams { flags: CMD_BOOST_1LEVEL, duration: DURATION_S };
        assert_eq!(old.tick(t0, false, GuestNews::Boost(CMD_BOOST_1LEVEL)), Tick::Boost(one));
        old.done(t0, 0x56, Duration::from_secs(7), t0 + Duration::from_secs(7));
        assert_eq!(old.tick(t0 + Duration::from_secs(7), true, GuestNews::None), Tick::Wait(BACKOFF_FIRST));
        assert_eq!(old.tick(t0 + Duration::from_secs(12), true, GuestNews::None), Tick::Boost(one));
    }

    #[test]
    fn vm_policy_step_serializes_finite_request_and_records_real_refusal_status() {
        let sig = Signal::new();
        sig.arm(Policy::Vm);
        let t0 = Instant::now();
        let mut r = Renewer::new(FLAGS_CMD, t0, Policy::Vm);
        let mut host = FakeHost { answers: [0x56].into(), ..Default::default() };
        let (tick, line) = step(&sig, &mut r, &mut host, t0);
        assert_eq!(tick, Tick::Boost(P));
        assert_eq!(host.calls, [(NV20_SUBDEVICE_0, CMD_PERF_BOOST, P.bytes().to_vec())]);
        assert!(line.unwrap().contains("refused with status 0x56"));
        assert_eq!((r.counts.calls, r.counts.refused, r.counts.renewals), (1, 1, 0));
        assert!(r.retry_at.is_some());
    }

    #[test]
    fn policy_reports_name_and_finite_pstate_contract_without_global_state() {
        let t0 = Instant::now();
        for policy in [Policy::Vm, Policy::Activity] {
            let line = Report::On { policy, epoch: t0, counts: Counts::default() }.line();
            assert!(line.contains(&format!("policy={}", policy.name())));
            assert!(line.contains("no MHz guarantee; finite 3 s lease"));
            assert!(line.contains("first at never"));
            let off = Report::Off { policy, why: "startup refused".into() }.line();
            assert!(off.contains(&format!("policy={}", policy.name())) && off.contains("startup refused"));
        }
    }

    /// Continuous activity renews once per interval, each on the host's subdevice with
    /// {BOOST_TO_MAX, 3 s}.
    #[test]
    fn active_renews_at_most_once_per_interval() {
        let (sig, mut host, t0) = (armed(), FakeHost::default(), Instant::now());
        let mut r = Renewer::new(FLAGS_CMD, t0, Policy::Activity);
        let (sent, _) = drive(&sig, &mut r, &mut host, t0, 0, 5000, 10, true);
        assert_eq!(sent, [0, 1000, 2000, 3000, 4000]);
        assert_eq!(r.counts.renewals, 5);
        assert!(host
            .calls
            .iter()
            .all(|c| *c == (NV20_SUBDEVICE_0, CMD_PERF_BOOST, P.bytes().to_vec())));
    }

    /// Activity ends: the activity of the last interval is renewed once, then the
    /// worker goes idle and nothing more is sent; the first activity after that boosts
    /// at once, and it wakes a parked worker.
    #[test]
    fn idle_stops_and_first_activity_boosts_at_once() {
        let (sig, mut host, t0) = (armed(), FakeHost::default(), Instant::now());
        let mut r = Renewer::new(FLAGS_CMD, t0, Policy::Activity);
        let (sent, _) = drive(&sig, &mut r, &mut host, t0, 0, 2500, 10, true);
        assert_eq!(sent, [0, 1000, 2000]);
        let (sent, last) = drive(&sig, &mut r, &mut host, t0, 2500, 10_000, 10, false);
        assert_eq!(sent, [3000]);
        assert_eq!(last, Tick::Idle);
        // The worker parks; the next activity must wake it, once.
        assert!(sig.park_if_quiet());
        assert!(sig.note());
        assert!(!sig.note());
        let (tick, _) = step(&sig, &mut r, &mut host, t0 + Duration::from_millis(12_345));
        assert_eq!(tick, Tick::Boost(P));
        assert_eq!(r.counts.renewals, 5);
        // Right after a renewal the worker waits out the interval instead of parking, so
        // a steady stream never pays the wake.
        let (tick, _) = step(&sig, &mut r, &mut host, t0 + Duration::from_millis(12_346));
        assert_eq!(tick, Tick::Wait(Duration::from_millis(999)));
        assert!(!sig.note() && sig.take());
        // Activity lands between the worker's take and its park: no park, no lost mark.
        sig.note();
        assert!(!sig.park_if_quiet());
    }

    /// Off (no worker), the hot path marks nothing.
    #[test]
    fn off_signal_marks_nothing() {
        let sig = Signal::new();
        assert!(!sig.on() && !sig.note());
        assert!(request(&sig, 0x12, "req").contains("nothing boosted"));
        assert_eq!(sig.take_guest(), GuestNews::None);
        // Only a worker's first look arms the signal.
        assert!(!sig.take());
        assert!(sig.on());
    }

    /// The guest's cuCtxCreate request {BOOST_TO_MAX | CUDA, infinite} reaches the host as
    /// {BOOST_TO_MAX, 3 s}; its clear sends nothing; MULTIMEDIA passes on a 615 host only.
    #[test]
    fn guest_infinite_request_is_clamped() {
        let (sig, mut host, t0) = (armed(), FakeHost::default(), Instant::now());
        let mut r = Renewer::new(crate::hostabi::HOST_615.perf_boost_flags, t0, Policy::Activity);
        let mut g = vec![0u8; v::PERF_BOOST_2X_SIZE];
        g[v::PERF_BOOST_2X_FLAGS] = (CMD_BOOST_TO_MAX | FLAG_CUDA) as u8;
        g[v::PERF_BOOST_2X_DURATION..][..4].copy_from_slice(&DURATION_INFINITE.to_le_bytes());
        let (flags, what) = describe_guest(&g);
        assert_eq!(what, "PERF_BOOST_SET_2X {flags 0x12, duration infinite}");
        assert!(request(&sig, flags, &what).contains("to the host boost"));
        let (tick, _) = step(&sig, &mut r, &mut host, t0);
        assert_eq!(tick, Tick::Boost(P));
        assert_eq!(host.calls[0].2, [2, 0, 0, 0, 3, 0, 0, 0]);
        assert_eq!((r.counts.renewals, r.counts.from_guest), (1, 1));
        // A MULTIMEDIA request with every other bit set, inside the interval: command and
        // MULTIMEDIA only, sent at once because the flags changed.
        request(&sig, 0xFF, "mm");
        let (tick, _) = step(&sig, &mut r, &mut host, t0 + Duration::from_millis(500));
        assert_eq!(
            tick,
            Tick::Boost(PerfBoostParams {
                flags: CMD_BOOST_TO_MAX | FLAG_MULTIMEDIA,
                duration: DURATION_S
            })
        );
        assert_eq!((r.counts.renewals, r.counts.from_guest), (2, 2));
        // The clear at context teardown: nothing sent, no activity marked, the flags go
        // back to the default for the next renewal.
        assert!(request(&sig, FLAG_CUDA, "clear").contains("not forwarded"));
        let (tick, _) = step(&sig, &mut r, &mut host, t0 + Duration::from_secs(2));
        assert_eq!(tick, Tick::Idle);
        assert_eq!(host.calls.len(), 2);
        sig.note();
        let (tick, _) = step(&sig, &mut r, &mut host, t0 + Duration::from_millis(2100));
        assert_eq!(tick, Tick::Boost(P));
        use crate::hostabi::{HOST_610, HOST_615};
        assert_eq!(
            forwarded(0xFF, HOST_610.perf_boost_flags),
            Some(CMD_BOOST_TO_MAX)
        );
        assert_eq!(forwarded(0x91, HOST_615.perf_boost_flags), Some(0x81));
        assert_eq!(forwarded(0x12, HOST_615.perf_boost_flags), Some(0x02));
        assert_eq!(forwarded(0x70, HOST_615.perf_boost_flags), None);
        // Short params: the flags byte alone, or nothing.
        assert_eq!(
            describe_guest(&[2]).1,
            "PERF_BOOST_SET_2X {flags 0x2, duration missing}"
        );
        assert_eq!(describe_guest(&[]).0, 0);
    }

    /// A refusal is logged once and backs off 5 s, then 10 s; a success resets the pause.
    #[test]
    fn a_refusal_backs_off() {
        // Explicit fake elapsed time keeps deadlines independent of control() wall time.
        let t0 = Instant::now();
        let mut r = Renewer::new(FLAGS_CMD, t0, Policy::Activity);
        assert_eq!(r.tick(t0, true, GuestNews::None), Tick::Boost(P));
        let line = r.done(t0, 0x56, Duration::from_secs(2), t0 + Duration::from_secs(2)).unwrap();
        assert!(line.contains("refused with status 0x56") && line.contains("5 s"), "{line}");
        assert_eq!(r.tick(t0 + Duration::from_secs(6), true, GuestNews::None), Tick::Wait(Duration::from_secs(1)));
        let t = t0 + Duration::from_secs(7);
        assert_eq!(r.tick(t, true, GuestNews::None), Tick::Boost(P));
        assert!(r.done(t, 0x56, Duration::ZERO, t).is_none());
        let t = t0 + Duration::from_secs(17);
        assert_eq!(r.tick(t, true, GuestNews::None), Tick::Boost(P));
        let first = r.done(t, NV_OK, Duration::ZERO, t).unwrap();
        assert!(first.starts_with("host boost: first renewal at +17.000 s"), "{first}");
        let t = t0 + Duration::from_secs(18);
        assert_eq!(r.tick(t, true, GuestNews::None), Tick::Boost(P));
        assert!(r.done(t, 0x56, Duration::ZERO, t).is_none());
        assert_eq!(r.tick(t0 + Duration::from_secs(22), true, GuestNews::None), Tick::Wait(Duration::from_secs(1)));
        assert_eq!((r.counts.renewals, r.counts.refused, r.counts.calls), (1, 3, 4));
        let line = r.counts.line(t0);
        assert!(line.starts_with("host boost: 1 renewals (0 from guest requests), 3 refused, first at +17.000 s; RM call mean"), "{line}");
        assert!(Counts::default().line(t0).contains("first at never"));
    }

    /// The host's `NV2080_CTRL_PERF_BOOST_PARAMS` (hand-laid: nvrm-sys binds no
    /// ctrl2080perf.h) and the guest's `NV2080_CTRL_INTERNAL_PERF_BOOST_SET_PARAMS_2X`
    /// from both bindings; the forwardable flag bits per host driver.
    #[test]
    fn params_layout_v610_and_v615() {
        assert_eq!(PARAMS_SIZE, 8);
        assert_eq!(offset_of!(PerfBoostParams, flags), 0);
        assert_eq!(offset_of!(PerfBoostParams, duration), 4);
        assert_eq!(
            PerfBoostParams {
                flags: 0x0000_0082,
                duration: 0x0102_0304
            }
            .bytes(),
            [0x82, 0, 0, 0, 4, 3, 2, 1]
        );
        use nvrm_sys::{v610 as a, v615 as b};
        assert_eq!(
            size_of::<a::NV2080_CTRL_INTERNAL_PERF_BOOST_SET_PARAMS_2X>(),
            8
        );
        assert_eq!(
            size_of::<b::NV2080_CTRL_INTERNAL_PERF_BOOST_SET_PARAMS_2X>(),
            8
        );
        assert_eq!(
            offset_of!(a::NV2080_CTRL_INTERNAL_PERF_BOOST_SET_PARAMS_2X, flags),
            0
        );
        assert_eq!(
            offset_of!(b::NV2080_CTRL_INTERNAL_PERF_BOOST_SET_PARAMS_2X, flags),
            0
        );
        assert_eq!(
            offset_of!(a::NV2080_CTRL_INTERNAL_PERF_BOOST_SET_PARAMS_2X, duration),
            4
        );
        assert_eq!(
            offset_of!(b::NV2080_CTRL_INTERNAL_PERF_BOOST_SET_PARAMS_2X, duration),
            4
        );
        assert_eq!(
            (
                v::PERF_BOOST_2X_SIZE,
                v::PERF_BOOST_2X_FLAGS,
                v::PERF_BOOST_2X_DURATION
            ),
            (8, 0, 4)
        );
        // The 2X flags are an NvBool: the guest kernel keeps the low byte of the public
        // flags (kern_perf_boost.c:112), where every defined bit lies (asserted at the
        // flag constants).
        assert_eq!(size_of::<a::NvBool>(), 1);
        assert_eq!(
            (CMD_PERF_BOOST, v::CMD_INTERNAL_PERF_BOOST_SET_2X),
            (0x2080_200A, 0x2080_0A9A)
        );
        use crate::hostabi::{HOST_610, HOST_615};
        assert_eq!(HOST_610.perf_boost_flags, FLAGS_CMD);
        assert_eq!(HOST_615.perf_boost_flags, FLAGS_CMD | FLAG_MULTIMEDIA);
    }
}

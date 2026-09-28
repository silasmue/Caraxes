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
//! - It holds a short boost of its own: `PERF_BOOST` {BOOST_TO_MAX, [`DURATION_S`] s} on
//!   the subdevice of a second unprivileged host RM client (hostrm.rs), renewed at most
//!   every [`RENEW`] while the VM is active. One renewal interval without activity ends the
//!   renewals, and the boost lapses on the host by itself: no cleanup, and a crashed device
//!   leaves nothing behind.
//! - Activity is every guest doorbell (ctrl.rs `doorbell_chid`: trapped, token pool,
//!   token-less eventfd, usermode page), every host non-stall event of the VM's mirrored
//!   channels (mirror.rs `event_loop`) and the guest's own PERF_BOOST (cuda.rs).
//! - The guest's request keeps its command (and MULTIMEDIA on a 615 host). Its CUDA,
//!   CUDA_PRIORITY and ASYNC bits are dropped, and its duration is never forwarded, so a
//!   guest cannot pin the host clocks. CUDA boosts share one reference-counted hint in the
//!   host RM's model (615 tree: src/nvidia/inc/kernel/gpu/perf/kern_perf_boost.h:37-40,
//!   :69-73); the device's short renewals stay out of it.
//! - [`activity`] is the whole hot path: one relaxed load when off or while a mark is set
//!   already, one compare-exchange for the first activity after each look of the worker
//!   (two looks per renewal interval), and an unpark (one futex wake) for the first
//!   activity after the worker went idle. The RM call runs on the `host-boost` thread,
//!   never under the device mutex.
//! - `CARAXES_HOST_BOOST=0` turns it off (A/B). The exit summary prints one line.

use core::mem::{offset_of, size_of};
use std::sync::atomic::{
    AtomicU32, AtomicU8,
    Ordering::{AcqRel, Relaxed, Release},
};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::ctrl::HostRm;
use crate::ver::common::{self as v, NV20_SUBDEVICE_0, NV_OK};

/// The switch: `0` (or `off`) turns the host boost off; anything else, or unset, leaves it on.
pub const ENV: &str = "CARAXES_HOST_BOOST";

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
/// A renewal at most this often while the VM is active; one interval without activity
/// and the renewals stop.
pub const RENEW: Duration = Duration::from_secs(1);
/// After a refusal: no call for this long, doubling per refusal up to [`BACKOFF_MAX`].
pub const BACKOFF_FIRST: Duration = Duration::from_secs(5);
pub const BACKOFF_MAX: Duration = Duration::from_secs(300);
/// The flags of an activity renewal: to the highest P-state, not a CUDA boost.
pub const DEFAULT_FLAGS: u32 = CMD_BOOST_TO_MAX;

// A boost outlives two renewal intervals: one late renewal (a busy host) never lets the
// clocks drop between two active intervals.
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

/// The switch's value: on unless `0` or `off`.
pub fn enabled(env: Option<&str>) -> bool {
    !matches!(env.map(str::trim), Some("0") | Some("off"))
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
            guest: AtomicU32::new(0),
        }
    }

    /// Whether a worker takes the activity.
    pub fn on(&self) -> bool {
        self.state.load(Relaxed) != OFF
    }

    fn arm(&self) {
        self.state.store(ARMED, Release);
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
    host_flags: u32,
    /// The flags of every renewal: the guest's last request, or [`DEFAULT_FLAGS`].
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
    pub fn new(host_flags: u32, epoch: Instant) -> Self {
        Renewer {
            epoch,
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
    pub fn tick(&mut self, now: Instant, active: bool, guest: GuestNews) -> Tick {
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
    pub fn done(&mut self, now: Instant, status: u32, took: Duration) -> Option<String> {
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
        self.retry_at = Some(now + self.backoff);
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
        line = r.done(now, status, t0.elapsed());
    }
    (tick, line)
}

// ---------------------------------------------------------------------------------
// The device's one booster.

static SIGNAL: Signal = Signal::new();
static WORKER: OnceLock<std::thread::Thread> = OnceLock::new();

#[derive(Clone)]
enum Report {
    Off(String),
    On { epoch: Instant, counts: Counts },
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
        return format!("{what}: nothing boosted (host boost off: {ENV}=0 or no host execution)");
    }
    sig.post_guest(flags);
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
pub fn start(host_flags: u32) -> Result<(), String> {
    let host = crate::hostrm::HostRmClient::open().map_err(|e| format!("host RM client: {e:#}"))?;
    spawn(Box::new(host), host_flags).map_err(|e| format!("spawn the host-boost thread: {e}"))
}

/// Start the worker on `host`.
pub fn spawn(host: Box<dyn HostRm>, host_flags: u32) -> std::io::Result<()> {
    let epoch = Instant::now();
    *REPORT.lock().unwrap_or_else(|p| p.into_inner()) = Some(Report::On {
        epoch,
        counts: Counts::default(),
    });
    // Armed before the thread runs: arming after it could overwrite the worker's PARKED
    // and leave it asleep while activity only marks itself.
    SIGNAL.arm();
    let spawned = std::thread::Builder::new()
        .name("host-boost".into())
        .spawn(move || run(host, host_flags, epoch));
    if spawned.is_err() {
        SIGNAL.state.store(OFF, Release);
    }
    spawned.map(|_| ())
}

/// Record why the booster does not run, for the exit summary.
pub fn set_off(why: &str) {
    *REPORT.lock().unwrap_or_else(|p| p.into_inner()) = Some(Report::Off(why.to_string()));
}

fn run(mut host: Box<dyn HostRm>, host_flags: u32, epoch: Instant) {
    let _ = WORKER.set(std::thread::current());
    let mut r = Renewer::new(host_flags, epoch);
    loop {
        let (tick, line) = step(&SIGNAL, &mut r, host.as_mut(), Instant::now());
        if let Some(l) = line {
            eprintln!("vfio-user-nvgpu: {l}");
        }
        match tick {
            Tick::Boost(_) => {
                if let Ok(mut rep) = REPORT.lock() {
                    *rep = Some(Report::On {
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
    let rep = REPORT.lock().unwrap_or_else(|p| p.into_inner()).clone()?;
    Some(match rep {
        Report::Off(why) => format!("host boost: off ({why})\n"),
        Report::On { epoch, counts } => counts.line(epoch),
    })
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
        s.arm();
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

    /// Continuous activity renews once per interval, each on the host's subdevice with
    /// {BOOST_TO_MAX, 3 s}.
    #[test]
    fn active_renews_at_most_once_per_interval() {
        let (sig, mut host, t0) = (armed(), FakeHost::default(), Instant::now());
        let mut r = Renewer::new(FLAGS_CMD, t0);
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
        let mut r = Renewer::new(FLAGS_CMD, t0);
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
        assert!(enabled(None) && enabled(Some("1")) && enabled(Some("")));
        assert!(!enabled(Some("0")) && !enabled(Some(" 0 ")) && !enabled(Some("off")));
    }

    /// The guest's cuCtxCreate request {BOOST_TO_MAX | CUDA, infinite} reaches the host as
    /// {BOOST_TO_MAX, 3 s}; its clear sends nothing; MULTIMEDIA passes on a 615 host only.
    #[test]
    fn guest_infinite_request_is_clamped() {
        let (sig, mut host, t0) = (armed(), FakeHost::default(), Instant::now());
        let mut r = Renewer::new(crate::hostabi::HOST_615.perf_boost_flags, t0);
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
        let (sig, t0) = (armed(), Instant::now());
        let mut host = FakeHost {
            answers: [0x56, 0x56, NV_OK, 0x56].into(),
            ..Default::default()
        };
        let mut r = Renewer::new(FLAGS_CMD, t0);
        sig.note();
        let (tick, line) = step(&sig, &mut r, &mut host, t0);
        assert_eq!(tick, Tick::Boost(P));
        let line = line.unwrap();
        assert!(
            line.contains("refused with status 0x56") && line.contains("5 s"),
            "{line}"
        );
        let (sent, _) = drive(&sig, &mut r, &mut host, t0, 10, 20_000, 10, true);
        // 5 s after the first refusal (refused again), 10 s after that (taken), then the
        // interval again, where the fourth answer refuses once more (5 s, reset).
        assert_eq!(sent, [5000, 15_000, 16_000]);
        assert_eq!((r.counts.renewals, r.counts.refused), (1, 3));
        assert_eq!(r.counts.calls, 4);
        // Only the first refusal logged; the first renewal logs once.
        let mut quiet = FakeHost {
            answers: [0x56, 0x56].into(),
            ..Default::default()
        };
        let mut r = Renewer::new(FLAGS_CMD, t0);
        sig.note();
        assert!(step(&sig, &mut r, &mut quiet, t0).1.is_some());
        sig.note();
        assert!(step(&sig, &mut r, &mut quiet, t0 + BACKOFF_FIRST)
            .1
            .is_none());
        sig.note();
        let first = step(&sig, &mut r, &mut quiet, t0 + BACKOFF_FIRST * 3)
            .1
            .unwrap();
        assert!(
            first.starts_with("host boost: first renewal at +15.000 s"),
            "{first}"
        );
        let line = r.counts.line(t0);
        assert!(
            line.starts_with("host boost: 1 renewals (0 from guest requests), 2 refused, first at +15.000 s; RM call mean"),
            "{line}"
        );
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

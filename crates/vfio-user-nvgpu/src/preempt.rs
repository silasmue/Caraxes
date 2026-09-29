// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! GR context-switch preemption of the host contexts
//! (docs/research/gr-preemption-2026-09-29.md, steps 0 and 1).
//!
//! - Step 0, read only: the host modes of every host GR TSG, read once right after its
//!   build (before its first `GPFIFO_SCHEDULE`) and once at its teardown, by
//!   `GR_GET_CTXSW_MODES` on a host channel of the TSG (mirror.rs).
//! - Step 1, compute parity: the guest's `GR_SET_CTXSW_PREEMPTION_MODE` is decoded and
//!   recorded per guest TSG ([`Ledger`], kept by the registry), and its compute mode (CTA
//!   or CILP) is bound on the host TSG at the build, before the first schedule, by the
//!   pointer-free `GR_CTXSW_PREEMPTION_BIND` {CILP_SET, mode} and checked by a GET
//!   ([`apply_compute`]). A SET that arrives after the build is bound then, on the idled
//!   TSG (mirror.rs `guest_preemption_set`).
//! - The compute mode only rises (WFI < CTA < CILP, [`compute_action`]): a CILP request the
//!   host refuses is never bound as CTA instead (the P22 decision), and a CTA request leaves
//!   a context that is already in CILP alone. A refusal keeps the host default, leaves one
//!   WARN line and is counted; the guest's SET is answered NV_OK as before.
//! - Graphics requests (GFXP) are logged and never bound: owned GFXP buffers in the VM's
//!   slice are step 2, and a pointer-free GFXP SET is accepted by the host without buffers
//!   (run P14), so it is never sent.

use std::collections::BTreeMap;

const NV_OK: u32 = 0;

/// `NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE` and its params {flags @0, hChannel @4,
/// gfxpPreemptMode @8, cilpPreemptMode @12, grRouteInfo @16}, 32 bytes, the same in 610 and
/// 615 (615 tree: src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080gr.h:831-857).
pub const CMD_SET_CTXSW_PREEMPTION_MODE: u32 = 0x2080_1210;
pub const SET_FLAGS: usize = 0;
pub const SET_H_CHANNEL: usize = 4;
pub const SET_GFXP: usize = 8;
pub const SET_CILP: usize = 12;
/// `_FLAGS_CILP` 0:0 and `_FLAGS_GFXP` 1:1 (ctrl2080gr.h:842-847).
pub const FLAG_CILP_SET: u32 = 1 << 0;
pub const FLAG_GFXP_SET: u32 = 1 << 1;
/// Graphics modes (ctrl2080gr.h:850-852).
pub const GFX_WFI: u32 = 0;
pub const GFX_GFXP: u32 = 1;
pub const GFX_GFXP_POOL: u32 = 2;
/// Compute modes (ctrl2080gr.h:855-857).
pub const COMPUTE_WFI: u32 = 0;
pub const COMPUTE_CTA: u32 = 1;
pub const COMPUTE_CILP: u32 = 2;
/// `NV2080_CTRL_CMD_GR_GET_CTXSW_MODES` {hChannel @0, zcullMode @4, pmMode @8, smpcMode
/// @12, cilpPreemptMode @16, gfxpPreemptMode @20}, 24 bytes (ctrl2080gr.h:1852-1863).
pub const CMD_GET_CTXSW_MODES: u32 = 0x2080_1235;
pub const MODES_SIZE: usize = 24;

fn word(p: &[u8], off: usize) -> u32 {
    p.get(off..off + 4)
        .map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()))
}

pub fn compute_name(mode: u32) -> &'static str {
    match mode {
        COMPUTE_WFI => "WFI",
        COMPUTE_CTA => "CTA",
        COMPUTE_CILP => "CILP",
        _ => "unknown",
    }
}

pub fn graphics_name(mode: u32) -> &'static str {
    match mode {
        GFX_WFI => "WFI",
        GFX_GFXP => "GFXP",
        GFX_GFXP_POOL => "GFXP_POOL",
        _ => "unknown",
    }
}

/// One guest `GR_SET_CTXSW_PREEMPTION_MODE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetRequest {
    pub flags: u32,
    pub h_channel: u32,
    pub gfxp: u32,
    pub cilp: u32,
}

impl SetRequest {
    /// `None` for a payload shorter than its four words (the guest sends 32 bytes).
    pub fn decode(params: &[u8]) -> Option<Self> {
        (params.len() >= SET_CILP + 4).then(|| SetRequest {
            flags: word(params, SET_FLAGS),
            h_channel: word(params, SET_H_CHANNEL),
            gfxp: word(params, SET_GFXP),
            cilp: word(params, SET_CILP),
        })
    }

    /// The compute mode asked for (flag CILP_SET), else `None`.
    pub fn compute(&self) -> Option<u32> {
        (self.flags & FLAG_CILP_SET != 0).then_some(self.cilp)
    }

    /// The graphics mode asked for (flag GFXP_SET), else `None`.
    pub fn graphics(&self) -> Option<u32> {
        (self.flags & FLAG_GFXP_SET != 0).then_some(self.gfxp)
    }

    pub fn describe(&self) -> String {
        let mut names = Vec::new();
        if self.flags & FLAG_CILP_SET != 0 {
            names.push("CILP_SET");
        }
        if self.flags & FLAG_GFXP_SET != 0 {
            names.push("GFXP_SET");
        }
        if self.flags & !(FLAG_CILP_SET | FLAG_GFXP_SET) != 0 {
            names.push("unknown bits");
        }
        format!(
            "flags {:#x} ({}), hChannel {:#x}, graphics {}, compute {}",
            self.flags,
            if names.is_empty() { "none".into() } else { names.join("|") },
            self.h_channel,
            match self.graphics() {
                Some(m) => format!("{} ({m})", graphics_name(m)),
                None => format!("not set (field {})", self.gfxp),
            },
            match self.compute() {
                Some(m) => format!("{} ({m})", compute_name(m)),
                None => format!("not set (field {})", self.cilp),
            },
        )
    }
}

/// What the guest asked for one guest TSG (or bare channel) so far: per flag, the mode of
/// the last SET that carried it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Recorded {
    pub compute: Option<u32>,
    pub graphics: Option<u32>,
    pub sets: u32,
}

impl Recorded {
    pub fn merge(&mut self, r: &SetRequest) {
        self.compute = r.compute().or(self.compute);
        self.graphics = r.graphics().or(self.graphics);
        self.sets = self.sets.saturating_add(1);
    }

    pub fn describe(&self) -> String {
        format!(
            "guest SET compute {}, graphics {} ({} SET(s))",
            self.compute.map_or("not set".into(), |m| format!("{} ({m})", compute_name(m))),
            self.graphics.map_or("not set".into(), |m| format!("{} ({m})", graphics_name(m))),
            self.sets
        )
    }
}

/// The guest's SETs by guest object (the TSG, or a bare channel that has its own TSG), and
/// how often each payload shape came (for the exit summary; the first of each shape is
/// logged). Owned by the registry, which drops an entry with its guest object.
#[derive(Debug, Default)]
pub struct Ledger {
    by_guest: BTreeMap<(u32, u32), Recorded>,
    /// (flags, gfxpPreemptMode, cilpPreemptMode) → count.
    shapes: BTreeMap<(u32, u32, u32), u64>,
    /// SETs whose hChannel named no TSG or channel of this device.
    unresolved: u64,
    /// SETs too short to decode.
    short: u64,
}

impl Ledger {
    /// Record `r` for guest object `guest` (`None`: unresolved). Returns the merged record
    /// and whether this payload shape is new.
    pub fn record(&mut self, guest: Option<(u32, u32)>, r: &SetRequest) -> (Option<Recorded>, bool) {
        let count = self.shapes.entry((r.flags, r.gfxp, r.cilp)).or_default();
        *count += 1;
        let new_shape = *count == 1;
        let Some(g) = guest else {
            self.unresolved += 1;
            return (None, new_shape);
        };
        let e = self.by_guest.entry(g).or_default();
        e.merge(r);
        (Some(*e), new_shape)
    }

    pub fn note_short(&mut self) {
        self.short += 1;
    }

    pub fn get(&self, guest: (u32, u32)) -> Option<Recorded> {
        self.by_guest.get(&guest).copied()
    }

    /// Keep the records of the guest objects `keep` accepts (the registry's free).
    pub fn retain(&mut self, mut keep: impl FnMut(&(u32, u32)) -> bool) {
        self.by_guest.retain(|k, _| keep(k));
    }

    pub fn len(&self) -> usize {
        self.by_guest.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_guest.is_empty()
    }

    /// `None` when the guest sent no SET.
    pub fn summary(&self) -> Option<String> {
        if self.shapes.is_empty() && self.short == 0 {
            return None;
        }
        let shapes: Vec<String> = self
            .shapes
            .iter()
            .map(|((f, g, c), n)| format!("{n}x {{flags {f:#x}, gfxp {g}, cilp {c}}}"))
            .collect();
        Some(format!(
            "preemption, guest SETs: {} ({} unresolved, {} too short); {} guest TSG record(s) live at exit",
            if shapes.is_empty() { "none".into() } else { shapes.join(", ") },
            self.unresolved,
            self.short,
            self.by_guest.len()
        ))
    }
}

/// A host `GR_GET_CTXSW_MODES` answer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modes {
    pub status: u32,
    pub zcull: u32,
    pub pm: u32,
    pub smpc: u32,
    pub cilp: u32,
    pub gfxp: u32,
}

impl Modes {
    pub fn decode(status: u32, p: &[u8]) -> Self {
        Modes {
            status,
            zcull: word(p, 4),
            pm: word(p, 8),
            smpc: word(p, 12),
            cilp: word(p, 16),
            gfxp: word(p, 20),
        }
    }

    pub fn ok(&self) -> bool {
        self.status == NV_OK
    }

    pub fn describe(&self) -> String {
        if !self.ok() {
            return format!("GET refused {:#x}", self.status);
        }
        format!(
            "gfxp {} ({}) cilp {} ({}) zcull {:#x}",
            self.gfxp,
            graphics_name(self.gfxp),
            self.cilp,
            compute_name(self.cilp),
            self.zcull
        )
    }
}

/// The two host calls of step 0 and 1, on the mirror's own unprivileged host client
/// (mirror.rs implements it for `HostMemory`; the tests use a recording fake).
pub trait CtxswHost {
    /// `GR_GET_CTXSW_MODES` on host channel `channel`.
    fn modes(&self, channel: u32) -> Modes;
    /// The pointer-free `GR_CTXSW_PREEMPTION_BIND` {CILP_SET, `mode`} on host TSG `tsg`
    /// (the group handle, run P14); the NV status.
    fn bind_compute(&self, tsg: u32, mode: u32) -> u32;
}

/// What to do with a compute request against the host context's current compute mode
/// (`None`: unknown, the GET was refused).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// No compute request recorded: the host default stays.
    NoRequest,
    /// Not a mode this device binds (WFI, or unknown): the host default stays.
    Ignored(u32),
    /// The host context is at the request or above it (CILP for a CTA request): nothing is
    /// bound, never lowered.
    Already(u32),
    /// Bind this mode, once.
    Bind(u32),
}

pub fn compute_action(requested: Option<u32>, host: Option<u32>) -> Action {
    let Some(want) = requested else {
        return Action::NoRequest;
    };
    if want != COMPUTE_CTA && want != COMPUTE_CILP {
        return Action::Ignored(want);
    }
    match host {
        Some(now) if (want..=COMPUTE_CILP).contains(&now) => Action::Already(now),
        _ => Action::Bind(want),
    }
}

/// How one application ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    NoRequest,
    Ignored(u32),
    Already(u32),
    /// Bound and confirmed by the GET (graphics mode unchanged).
    Bound(u32),
    /// The BIND was refused, or the GET after it does not show the mode: the host default
    /// stays (no other mode is tried).
    Refused { mode: u32, bind: u32 },
    /// A late SET whose built TSG did not idle: nothing bound, the host default stays.
    NotIdle(u32),
}

impl Outcome {
    pub fn describe(&self) -> String {
        match self {
            Outcome::NoRequest => "no guest compute request: host default kept".into(),
            Outcome::Ignored(m) => format!(
                "guest compute {} ({m}) is not bound (only CTA and CILP): host default kept",
                compute_name(*m)
            ),
            Outcome::Already(m) => format!(
                "host already in compute {} ({m}): nothing bound (never lowered)",
                compute_name(*m)
            ),
            Outcome::Bound(m) => format!("compute {} ({m}) bound", compute_name(*m)),
            Outcome::Refused { mode, bind } => format!(
                "compute {} ({mode}) REFUSED by the host (BIND {bind:#x}): host default kept, no other mode tried",
                compute_name(*mode)
            ),
            Outcome::NotIdle(m) => format!(
                "compute {} ({m}) NOT BOUND: the TSG did not idle, host default kept",
                compute_name(*m)
            ),
        }
    }
}

/// One application: the modes before, what was done, the modes after a BIND.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Applied {
    pub before: Modes,
    pub outcome: Outcome,
    pub after: Option<Modes>,
}

impl Applied {
    /// Nothing was bound although the guest asked for a mode above the host's.
    pub fn refused(&self) -> bool {
        matches!(self.outcome, Outcome::Refused { .. } | Outcome::NotIdle(_))
    }

    pub fn describe(&self) -> String {
        format!(
            "host {}; {}{}",
            self.before.describe(),
            self.outcome.describe(),
            self.after.map_or(String::new(), |a| format!(", GET {}", a.describe()))
        )
    }
}

/// Step 0 and 1 on host TSG `tsg` through its host channel `channel`: read the modes, bind
/// the compute request if it raises the context's mode, and confirm by a second read. At
/// most one BIND: a refused CILP is not retried as CTA. At the build nothing runs on the TSG
/// yet ([`apply_compute_with`] idles a built one first).
pub fn apply_compute(host: &dyn CtxswHost, tsg: u32, channel: u32, requested: Option<u32>) -> Applied {
    apply_compute_with(host, tsg, channel, requested, || true)
}

/// [`apply_compute`] with `prepare` run only when a BIND follows (the idle of a TSG that is
/// built and may run); `false` from it binds nothing.
pub fn apply_compute_with(
    host: &dyn CtxswHost,
    tsg: u32,
    channel: u32,
    requested: Option<u32>,
    prepare: impl FnOnce() -> bool,
) -> Applied {
    let before = host.modes(channel);
    let known = before.ok().then_some(before.cilp);
    let mode = match compute_action(requested, known) {
        Action::NoRequest => return Applied { before, outcome: Outcome::NoRequest, after: None },
        Action::Ignored(m) => return Applied { before, outcome: Outcome::Ignored(m), after: None },
        Action::Already(m) => return Applied { before, outcome: Outcome::Already(m), after: None },
        Action::Bind(m) => m,
    };
    if !prepare() {
        return Applied { before, outcome: Outcome::NotIdle(mode), after: None };
    }
    let bind = host.bind_compute(tsg, mode);
    let after = host.modes(channel);
    let graphics_kept = !before.ok() || after.gfxp == before.gfxp;
    let outcome = if bind == NV_OK && after.ok() && after.cilp == mode && graphics_kept {
        Outcome::Bound(mode)
    } else {
        Outcome::Refused { mode, bind }
    };
    Applied { before, outcome, after: Some(after) }
}

/// Counters for the exit summary.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    /// Host GR TSGs read at their build.
    pub tsgs: u64,
    pub get_refused: u64,
    pub no_request: u64,
    pub ignored: u64,
    pub already: u64,
    pub bound_cta: u64,
    pub bound_cilp: u64,
    /// Of the bound and refused ones: bound after the build (a late SET).
    pub late: u64,
    pub refused: u64,
    /// Late SETs whose TSG did not idle: nothing bound.
    pub not_idle: u64,
    /// Host GR TSGs whose guest had asked for a graphics mode other than WFI by their build
    /// (not bound: step 2).
    pub gfxp_asked: u64,
    pub teardown_reads: u64,
}

impl Stats {
    pub fn count(&mut self, a: &Applied, late: bool) {
        if !late {
            self.tsgs += 1;
            if !a.before.ok() {
                self.get_refused += 1;
            }
        }
        match a.outcome {
            Outcome::NoRequest => self.no_request += 1,
            Outcome::Ignored(_) => self.ignored += 1,
            Outcome::Already(_) => self.already += 1,
            Outcome::Bound(COMPUTE_CTA) => self.bound_cta += 1,
            Outcome::Bound(_) => self.bound_cilp += 1,
            Outcome::Refused { .. } => self.refused += 1,
            Outcome::NotIdle(_) => self.not_idle += 1,
        }
        if late && matches!(a.outcome, Outcome::Bound(_) | Outcome::Refused { .. }) {
            self.late += 1;
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "preemption, host GR TSGs: {} read at build ({} GET refused), {} at teardown; compute bound CTA {} CILP {} ({} of them after the build), already at or above the request {}, no guest request {}, not bound (WFI/unknown asked) {}, REFUSED {} (host default kept), late SET not idle {} (host default kept); graphics asked by {} TSG(s) by their build, not bound (step 2)",
            self.tsgs,
            self.get_refused,
            self.teardown_reads,
            self.bound_cta,
            self.bound_cilp,
            self.late,
            self.already,
            self.no_request,
            self.ignored,
            self.refused,
            self.not_idle,
            self.gfxp_asked
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Call {
        Get(u32),
        Bind(u32, u32),
    }

    /// A host context: its modes, which compute modes a BIND is accepted for, and the calls.
    struct Fake {
        modes: RefCell<Modes>,
        accepts: Vec<u32>,
        calls: RefCell<Vec<Call>>,
    }

    impl Fake {
        fn new(cilp: u32, gfxp: u32, accepts: &[u32]) -> Self {
            Fake {
                modes: RefCell::new(Modes { cilp, gfxp, ..Modes::default() }),
                accepts: accepts.to_vec(),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl CtxswHost for Fake {
        fn modes(&self, channel: u32) -> Modes {
            self.calls.borrow_mut().push(Call::Get(channel));
            *self.modes.borrow()
        }
        fn bind_compute(&self, tsg: u32, mode: u32) -> u32 {
            self.calls.borrow_mut().push(Call::Bind(tsg, mode));
            if self.accepts.contains(&mode) {
                self.modes.borrow_mut().cilp = mode;
                NV_OK
            } else {
                0x1f
            }
        }
    }

    fn set(flags: u32, h: u32, gfxp: u32, cilp: u32) -> Vec<u8> {
        let mut p = vec![0u8; 32];
        for (off, v) in [(0, flags), (4, h), (8, gfxp), (12, cilp)] {
            p[off..off + 4].copy_from_slice(&v.to_le_bytes());
        }
        p
    }

    /// The payloads of the bare-metal traces (matrix/traces/610.57.04): graphics clients
    /// {flags 1, cilp 1} on their TSG, CUDA {flags 1, cilp 2}; none sets GFXP.
    #[test]
    fn the_set_payload_decodes_per_flag() {
        let r = SetRequest::decode(&set(1, 0x5c00_0012, 0, 2)).unwrap();
        assert_eq!((r.compute(), r.graphics()), (Some(COMPUTE_CILP), None));
        assert_eq!(
            r.describe(),
            "flags 0x1 (CILP_SET), hChannel 0x5c000012, graphics not set (field 0), compute CILP (2)"
        );
        let r = SetRequest::decode(&set(1, 0xcef8_0000, 0, 1)).unwrap();
        assert_eq!(r.compute(), Some(COMPUTE_CTA));
        // GFXP_SET only: the compute field is not a request.
        let r = SetRequest::decode(&set(2, 7, 1, 2)).unwrap();
        assert_eq!((r.compute(), r.graphics()), (None, Some(GFX_GFXP)));
        assert!(r.describe().contains("graphics GFXP (1), compute not set (field 2)"), "{}", r.describe());
        let r = SetRequest::decode(&set(3, 7, 2, 1)).unwrap();
        assert!(r.describe().starts_with("flags 0x3 (CILP_SET|GFXP_SET)"));
        assert!(SetRequest::decode(&[1, 0, 0, 0, 7, 0, 0, 0]).is_none());
        assert!(SetRequest::decode(&set(4, 7, 0, 0)).unwrap().describe().contains("unknown bits"));
    }

    #[test]
    fn a_record_keeps_the_last_mode_of_each_flag() {
        let mut l = Ledger::default();
        let g = (1, 0x30);
        let (r, new) = l.record(Some(g), &SetRequest::decode(&set(1, 0x30, 0, 1)).unwrap());
        assert!(new);
        assert_eq!(r.unwrap().compute, Some(COMPUTE_CTA));
        // A graphics-only SET keeps the compute request.
        let (r, new) = l.record(Some(g), &SetRequest::decode(&set(2, 0x30, 1, 0)).unwrap());
        assert!(new);
        assert_eq!(r.unwrap(), Recorded { compute: Some(COMPUTE_CTA), graphics: Some(GFX_GFXP), sets: 2 });
        // A later compute SET replaces the compute request.
        let (r, new) = l.record(Some(g), &SetRequest::decode(&set(1, 0x30, 0, 2)).unwrap());
        assert!(new);
        assert_eq!(r.unwrap().compute, Some(COMPUTE_CILP));
        let (_, new) = l.record(Some((1, 0x31)), &SetRequest::decode(&set(1, 0x31, 0, 2)).unwrap());
        assert!(!new, "the shape was seen already");
        assert_eq!(l.record(None, &SetRequest::decode(&set(1, 0x99, 0, 2)).unwrap()).0, None);
        let s = l.summary().unwrap();
        assert!(s.contains("3x {flags 0x1, gfxp 0, cilp 2}"), "{s}");
        assert!(s.contains("1 unresolved"), "{s}");
        l.retain(|k| *k != g);
        assert_eq!(l.get(g), None);
        assert_eq!(l.len(), 1);
        assert!(Ledger::default().summary().is_none());
    }

    /// Record, then apply at the build: GET, one BIND on the TSG handle, GET on the channel.
    #[test]
    fn a_recorded_request_is_bound_at_the_build_and_confirmed() {
        let mut l = Ledger::default();
        let g = (1, 0x5c00_0012);
        l.record(Some(g), &SetRequest::decode(&set(1, g.1, 0, 2)).unwrap());
        let host = Fake::new(COMPUTE_WFI, GFX_WFI, &[COMPUTE_CTA, COMPUTE_CILP]);
        let a = apply_compute(&host, 0x8000_0010, 0x8000_0020, l.get(g).and_then(|r| r.compute));
        assert_eq!(a.outcome, Outcome::Bound(COMPUTE_CILP));
        assert_eq!(
            host.calls.borrow().as_slice(),
            &[Call::Get(0x8000_0020), Call::Bind(0x8000_0010, 2), Call::Get(0x8000_0020)]
        );
        assert_eq!(a.after.unwrap().cilp, COMPUTE_CILP);
        assert!(a.describe().contains("compute CILP (2) bound, GET gfxp 0 (WFI) cilp 2 (CILP)"), "{}", a.describe());
        let mut s = Stats::default();
        s.count(&a, false);
        assert_eq!((s.tsgs, s.bound_cilp, s.late), (1, 1, 0));
    }

    /// No request: read only (step 0), nothing bound.
    #[test]
    fn without_a_request_only_the_modes_are_read() {
        let host = Fake::new(COMPUTE_CTA, GFX_WFI, &[COMPUTE_CTA, COMPUTE_CILP]);
        let a = apply_compute(&host, 1, 2, None);
        assert_eq!(a.outcome, Outcome::NoRequest);
        assert_eq!(host.calls.borrow().as_slice(), &[Call::Get(2)]);
        assert!(a.describe().starts_with("host gfxp 0 (WFI) cilp 1 (CTA) zcull 0x0; no guest compute request"));
    }

    /// The P22 decision: a refused CILP is never bound as CTA; the host default stays.
    #[test]
    fn a_refused_cilp_is_not_downgraded_to_cta() {
        let host = Fake::new(COMPUTE_WFI, GFX_WFI, &[COMPUTE_CTA]);
        let a = apply_compute(&host, 1, 2, Some(COMPUTE_CILP));
        assert_eq!(a.outcome, Outcome::Refused { mode: COMPUTE_CILP, bind: 0x1f });
        assert_eq!(
            host.calls.borrow().as_slice(),
            &[Call::Get(2), Call::Bind(1, COMPUTE_CILP), Call::Get(2)],
            "one BIND, no CTA fallback"
        );
        assert_eq!(host.modes.borrow().cilp, COMPUTE_WFI, "host default kept");
        assert!(a.outcome.describe().contains("REFUSED"));
        let mut s = Stats::default();
        s.count(&a, true);
        assert_eq!((s.refused, s.late, s.tsgs), (1, 1, 0));
    }

    /// A late SET: the idle runs only when a BIND follows; a TSG that does not idle is left
    /// at the host default.
    #[test]
    fn a_built_tsg_is_idled_before_the_bind_and_left_alone_if_it_does_not_idle() {
        let host = Fake::new(COMPUTE_WFI, GFX_WFI, &[COMPUTE_CTA, COMPUTE_CILP]);
        let idles = RefCell::new(0);
        let a = apply_compute_with(&host, 1, 2, Some(COMPUTE_CILP), || {
            *idles.borrow_mut() += 1;
            false
        });
        assert_eq!(a.outcome, Outcome::NotIdle(COMPUTE_CILP));
        assert!(a.refused());
        assert_eq!(host.calls.borrow().as_slice(), &[Call::Get(2)], "no BIND without the idle");
        let a = apply_compute_with(&host, 1, 2, Some(COMPUTE_CILP), || {
            *idles.borrow_mut() += 1;
            true
        });
        assert_eq!(a.outcome, Outcome::Bound(COMPUTE_CILP));
        // Already there: no idle at all.
        let a = apply_compute_with(&host, 1, 2, Some(COMPUTE_CTA), || {
            *idles.borrow_mut() += 1;
            true
        });
        assert_eq!(a.outcome, Outcome::Already(COMPUTE_CILP));
        assert_eq!(*idles.borrow(), 2);
        let mut s = Stats::default();
        s.count(&apply_compute_with(&Fake::new(0, 0, &[]), 1, 2, Some(COMPUTE_CTA), || false), true);
        assert_eq!((s.not_idle, s.refused, s.late), (1, 0, 0));
    }

    /// The mode only rises: CTA leaves a CILP context alone; the same mode is not re-bound.
    #[test]
    fn a_cilp_context_is_never_lowered_to_cta() {
        let host = Fake::new(COMPUTE_CILP, GFX_WFI, &[COMPUTE_CTA, COMPUTE_CILP]);
        let a = apply_compute(&host, 1, 2, Some(COMPUTE_CTA));
        assert_eq!(a.outcome, Outcome::Already(COMPUTE_CILP));
        assert_eq!(host.calls.borrow().as_slice(), &[Call::Get(2)]);
        assert_eq!(compute_action(Some(COMPUTE_CTA), Some(COMPUTE_CTA)), Action::Already(COMPUTE_CTA));
        assert_eq!(compute_action(Some(COMPUTE_CILP), Some(COMPUTE_CTA)), Action::Bind(COMPUTE_CILP));
        assert_eq!(compute_action(Some(COMPUTE_CTA), Some(COMPUTE_WFI)), Action::Bind(COMPUTE_CTA));
        // WFI or an unknown mode is never bound; an unknown host mode (GET refused, or a
        // value outside the enum) does not count as "already".
        assert_eq!(compute_action(Some(COMPUTE_WFI), Some(COMPUTE_CILP)), Action::Ignored(COMPUTE_WFI));
        assert_eq!(compute_action(Some(7), None), Action::Ignored(7));
        assert_eq!(compute_action(Some(COMPUTE_CTA), None), Action::Bind(COMPUTE_CTA));
        assert_eq!(compute_action(Some(COMPUTE_CTA), Some(9)), Action::Bind(COMPUTE_CTA));
        assert_eq!(compute_action(None, Some(COMPUTE_WFI)), Action::NoRequest);
    }

    /// A BIND answered NV_OK whose GET does not show the mode, or that changed the graphics
    /// mode, is not taken as bound.
    #[test]
    fn a_bind_is_confirmed_by_the_get() {
        struct Lies(RefCell<u32>);
        impl CtxswHost for Lies {
            fn modes(&self, _: u32) -> Modes {
                *self.0.borrow_mut() += 1;
                Modes { gfxp: *self.0.borrow() - 1, ..Modes::default() }
            }
            fn bind_compute(&self, _: u32, _: u32) -> u32 {
                NV_OK
            }
        }
        let a = apply_compute(&Lies(RefCell::new(0)), 1, 2, Some(COMPUTE_CTA));
        assert_eq!(a.outcome, Outcome::Refused { mode: COMPUTE_CTA, bind: NV_OK });
        // A refused first GET: the BIND is still tried and the second GET decides.
        struct Late(RefCell<bool>);
        impl CtxswHost for Late {
            fn modes(&self, _: u32) -> Modes {
                let first = !*self.0.borrow();
                *self.0.borrow_mut() = true;
                if first {
                    Modes { status: 0x1f, ..Modes::default() }
                } else {
                    Modes { cilp: COMPUTE_CTA, gfxp: GFX_GFXP, ..Modes::default() }
                }
            }
            fn bind_compute(&self, _: u32, _: u32) -> u32 {
                NV_OK
            }
        }
        let a = apply_compute(&Late(RefCell::new(false)), 1, 2, Some(COMPUTE_CTA));
        assert_eq!(a.outcome, Outcome::Bound(COMPUTE_CTA));
        assert!(a.describe().starts_with("host GET refused 0x1f;"));
    }
}

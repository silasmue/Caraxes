// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The table-driven public controls of the GSP plan (docs/caraxes/gsp-plan/
//! b-host-forwardable.md, c-guest-state.md): the rows of `ver::<version>::public::PUBLIC`.
//! Plain forwards stay in the `FORWARD` list (nvml.rs); this module answers the rest.
//!
//! * **Filtered forwards.** The control goes to the device's own host subdevice, and the
//!   answer is cut to what the guest may see before it leaves the device. The FB address
//!   filters keep only addresses inside the VM's VRAM reservation `[B, E)`; guest FB
//!   address X is host VRAM address X (docs/caraxes/memory-model.md §1), so a kept address
//!   is already the guest's FB offset and needs no rebasing. Without a reservation no
//!   host FB address is a guest address, and nothing address-shaped is passed on.
//!
//! * **Device answers.** Controls whose host answer is wrong for the guest are answered
//!   from the device's own model and never forwarded.
//! * **Refusals.** NOT_SUPPORTED with a label, the request echoed, nothing sent anywhere.
//! * **Guest objects** (the C plan). Controls on the guest's own channels and TSGs are
//!   recorded per object ([`GuestAttrs`]); the only host path is the object's own mirror
//!   ([`MirrorObjects`]), never the host RM client and never another handle.
//!
//! The filtered forwards call `HostRm::control` with the subdevice class, so a row can
//! never reach another host object (hostrm.rs).

use crate::ctrl::{Answer, HostRm};
use std::collections::BTreeMap;

use crate::ver::active::public::{self as p, Dev, Filter, Guest, PublicControl, Rule};
use crate::ver::active::{NV20_SUBDEVICE_0, NV_ERR_NOT_SUPPORTED, NV_OK};

/// `NV_ERR_INVALID_PARAM_STRUCT` (nvstatuscodes.h:87): a guest buffer of another size
/// than the version's struct is refused, never forwarded.
pub const NV_ERR_INVALID_PARAM_STRUCT: u32 = 0x3A;

/// What the rows need from the dispatcher.
pub struct Ctx<'a> {
    pub host: Option<&'a mut (dyn HostRm + 'static)>,
    /// The VM's VRAM reservation `[B, E)`, when the device holds one.
    pub reservation: Option<(u64, u64)>,
    /// The engines the device presents (the GPUMON filter zeroes the others).
    pub chip: &'static crate::chip::ChipData,
    /// The device clock, in ns (`channel::Registry::now_ns`).
    pub now_ns: u64,
    /// The guest object the control names (hClient, hObject), and its key in the
    /// attribute record (duplicates resolved, `channel::Registry::resolve`).
    pub object: (u32, u32),
    pub key: (u32, u32),
    /// The host mirrors of the guest's channels and TSGs, when the device executes on the
    /// physical GPU.
    pub mirror: Option<&'a mut dyn MirrorObjects>,
    /// What the guest set on its objects.
    pub attrs: &'a mut GuestAttrs,
}

/// The host mirror of a guest channel or TSG, looked up by the guest's own handles.
pub trait MirrorObjects {
    /// Issue `cmd` on the host object that mirrors the guest object `(h_client,
    /// h_object)`: (host status, note), or `None` when the object has no mirror.
    fn forward(&mut self, h_client: u32, h_object: u32, cmd: u32, params: &[u8])
        -> Option<(u32, String)>;
}

/// [`MirrorObjects`] over the device's mirror (`Mirror::forward_control`: the TSG by its
/// guest key, the channel through the registry by its guest handle, never another handle).
pub struct MirrorFwd<'a> {
    pub mirror: &'a mut crate::mirror::Mirror,
    pub reg: &'a crate::channel::Registry,
}

impl MirrorObjects for MirrorFwd<'_> {
    fn forward(
        &mut self,
        h_client: u32,
        h_object: u32,
        cmd: u32,
        params: &[u8],
    ) -> Option<(u32, String)> {
        self.mirror
            .forward_control(self.reg, h_client, h_object, cmd, params)
    }
}

/// Per guest object: what its controls recorded (c-guest-state.md §4 `ObjAttrs`).
#[derive(Default, Debug)]
pub struct GuestAttrs {
    objects: BTreeMap<(u32, u32), Attrs>,
}

#[derive(Default, Debug, Clone)]
pub struct Attrs {
    /// The last parameter block of each setter, by control id.
    pub recorded: BTreeMap<u32, Vec<u8>>,
    /// Resets of the channel the device answered or forwarded.
    pub resets: u32,
    /// The last MMU fault of the channel as `GET_MMU_FAULT_INFO` returns it; nothing
    /// records one yet (the RC events are the RPC plan's).
    pub last_fault: Option<Vec<u8>>,
}

impl GuestAttrs {
    pub fn get(&self, key: (u32, u32)) -> Option<&Attrs> {
        self.objects.get(&key)
    }

    /// `FREE` of an object (and of everything a freed client owned).
    pub fn free(&mut self, h_client: u32, h_object: u32) {
        self.objects.remove(&(h_client, h_object));
        if h_client == h_object {
            self.objects.retain(|(c, _), _| *c != h_client);
        }
    }
}

fn get32(b: &[u8], off: usize) -> u32 {
    b.get(off..off + 4)
        .map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

fn get64(b: &[u8], off: usize) -> u64 {
    b.get(off..off + 8)
        .map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap()))
}

fn put32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// The answer of the table to `cmd`, or `None` when the table has no row for it.
pub fn answer(ctx: &mut Ctx<'_>, cmd: u32, params: &[u8]) -> Option<Answer> {
    let row = p::public_control(cmd)?;
    if row.size != 0 && params.len() != row.size {
        return Some(Answer {
            status: NV_ERR_INVALID_PARAM_STRUCT,
            params: params.to_vec(),
            label: "refused (params size is not the driver's struct; not forwarded)",
            note: format!(
                "{} ({cmd:#x}): {} bytes, the struct has {}",
                row.name,
                params.len(),
                row.size
            ),
        });
    }
    Some(match row.rule {
        Rule::Filtered(f) => filtered(ctx, row, f, params),
        Rule::Device(d) => device(ctx, row, d, params),
        Rule::Guest(g) => guest(ctx, row, g, params),
        Rule::Refuse(label) => Answer {
            status: NV_ERR_NOT_SUPPORTED,
            params: params.to_vec(),
            label,
            note: format!("{} ({cmd:#x}): not supported, never forwarded", row.name),
        },
    })
}

/// A control on one of the guest's channels or TSGs. Nothing here uses the host RM
/// client; the only host path is the object's own mirror.
fn guest(ctx: &mut Ctx<'_>, row: &PublicControl, g: Guest, params: &[u8]) -> Answer {
    let (h_client, h_object) = ctx.object;
    let n = params.len();
    let ok = |b: Vec<u8>, label: &'static str, what: String| Answer {
        status: NV_OK,
        params: b,
        label,
        note: format!(
            "{} ({:#x}) on {h_client:#x}/{h_object:#x}: {what}",
            row.name, row.id
        ),
    };
    match g {
        Guest::Reset { host_cmd } => {
            let a = ctx.attrs.objects.entry(ctx.key).or_default();
            a.resets += 1;
            a.last_fault = None;
            let resets = a.resets;
            match ctx
                .mirror
                .as_deref_mut()
                .and_then(|m| m.forward(h_client, h_object, host_cmd, params))
            {
                Some((st, note)) => ok(
                    params.to_vec(),
                    "host (reset on the guest's own mirror channel; the guest sees NV_OK)",
                    format!("reset {resets}, as {host_cmd:#x}: {note}; host status {st:#x}"),
                ),
                None => ok(
                    params.to_vec(),
                    "guest (no mirror: reset recorded by the device; the software executor keeps no fault state)",
                    format!("reset {resets}, no host object"),
                ),
            }
        }
        Guest::MmuFaultInfo => {
            let fault = ctx.attrs.get(ctx.key).and_then(|a| a.last_fault.clone());
            let mut b = vec![0u8; n];
            let what = match fault {
                Some(f) => {
                    let k = f.len().min(n);
                    b[..k].copy_from_slice(&f[..k]);
                    "the recorded fault".to_string()
                }
                None => "no fault recorded".to_string(),
            };
            ok(b, "guest (the device's fault state of the channel)", what)
        }
        Guest::DeferRcState => ok(
            vec![0u8; n],
            "guest (the device defers no RC: bDeferRCPending 0)",
            "no deferred RC".into(),
        ),
        Guest::RcErrorCount => ok(
            vec![0u8; n],
            "guest (the device's RC journal for this guest: no RC error reported)",
            "error count 0".into(),
        ),
        Guest::RcErrorV2 => {
            let mut b = vec![0u8; n];
            b[..4].copy_from_slice(&params[..4]);
            put32(&mut b, p::RC_ERROR_V2_OUTPUT_SIZE, 0);
            ok(
                b,
                "guest (the device's RC journal for this guest: no record)",
                format!("record {}: none", get32(params, 0)),
            )
        }
        Guest::Record { max_level } => {
            ctx.attrs
                .objects
                .entry(ctx.key)
                .or_default()
                .recorded
                .insert(row.id, params.to_vec());
            let level = get32(params, 0);
            if max_level.is_some_and(|m| level > m) {
                return ok(
                    params.to_vec(),
                    "guest (recorded; a level above MEDIUM is not forwarded: a guest must not outrank the host desktop)",
                    format!("level {level} recorded only"),
                );
            }
            match ctx
                .mirror
                .as_deref_mut()
                .and_then(|m| m.forward(h_client, h_object, row.id, params))
            {
                Some((st, note)) => ok(
                    params.to_vec(),
                    "host (recorded; forwarded to the guest object's own mirror; the guest sees NV_OK)",
                    format!("{note}; host status {st:#x}"),
                ),
                None => ok(
                    params.to_vec(),
                    "guest (recorded; no mirror object)",
                    "recorded".into(),
                ),
            }
        }
        Guest::Get { set } => {
            let rec = ctx.attrs.get(ctx.key).and_then(|a| a.recorded.get(&set));
            let mut b = vec![0u8; n];
            if let Some(r) = rec {
                let k = r.len().min(n);
                b[..k].copy_from_slice(&r[..k]);
            }
            ok(
                b,
                "guest (the value the guest set; NONE before)",
                format!("recorded: {}", rec.is_some()),
            )
        }
        Guest::RecordOnly => {
            ctx.attrs
                .objects
                .entry(ctx.key)
                .or_default()
                .recorded
                .insert(row.id, params.to_vec());
            ok(
                params.to_vec(),
                "guest (recorded, never forwarded: decision Q-C7, no guest priority over the host desktop, no restart of a shared runlist)",
                "recorded only".into(),
            )
        }
    }
}

fn device(ctx: &Ctx<'_>, row: &PublicControl, d: Dev, params: &[u8]) -> Answer {
    let n = params.len();
    let (status, b, label, what): (u32, Vec<u8>, &'static str, String) = match d {
        Dev::RmHeapStats => (
            NV_OK,
            vec![0u8; n],
            "synthesized (the device's own GSP heap view: no RM heap, all 0; the host GSP is never asked, decision D9/Q-B6)",
            "rm heap stats: zeros".into(),
        ),
        Dev::TimeCorrelation => {
            // Every requested sample from the one device clock: the emulated GSP's OS
            // time and the device PTIMER (usermode TIME_0/1, uvm.rs) are the same base,
            // so cpuTime = gpuTime. The host's GSP clock and PTIMER are never read.
            let mut b = params.to_vec();
            let count = (b[p::TIME_CORR_SAMPLE_COUNT] as usize).min(p::TIME_CORR_MAX_SAMPLES);
            b[p::TIME_CORR_SAMPLES..].fill(0);
            for i in 0..count {
                let o = p::TIME_CORR_SAMPLES + 16 * i;
                put64(&mut b, o, ctx.now_ns);
                put64(&mut b, o + 8, ctx.now_ns);
            }
            (
                NV_OK,
                b,
                "synthesized (the device's own clock: GSP OS time and PTIMER are one time base)",
                format!("cpuClkId {:#x}, {count} sample(s) at {} ns", params[0], ctx.now_ns),
            )
        }
        Dev::NbsiNotFound => {
            let mut b = params.to_vec();
            b[p::NBSI_V2_RET_SIZE - p::NBSI_V2_RET_BUF_LEN..p::NBSI_V2_RET_SIZE].fill(0);
            put32(&mut b, p::NBSI_V2_RET_SIZE, 0);
            put32(&mut b, p::NBSI_V2_ERROR_CODE, p::NBSI_NOT_FOUND);
            (
                NV_OK,
                b,
                "synthesized (the synthesized VBIOS has no NBSI: NOT_FOUND, retSize 0)",
                "nbsi v2: not found".into(),
            )
        }
        Dev::I2cNoPorts => (
            NV_OK,
            vec![0u8; n],
            "synthesized (no I2C ports, as I2C_GET_PORT_INFO answers)",
            "i2c port speed: no ports".into(),
        ),
        Dev::RcRecoveryEnabled => {
            let mut b = vec![0u8; n];
            put32(&mut b, 0, p::RC_RECOVERY_ENABLED);
            (
                NV_OK,
                b,
                "synthesized (the device's RC policy: recovery enabled)",
                "rc recovery: enabled".into(),
            )
        }
        Dev::NoCompTags => (
            NV_OK,
            vec![0u8; n],
            "synthesized (no compression, D16: numCompTags 0)",
            "ctags for cbc eviction: none".into(),
        ),
        Dev::NoEcc => (
            NV_ERR_NOT_SUPPORTED,
            params.to_vec(),
            "synthesized (NOT_SUPPORTED: no ECC on this board, as the host answers the ECC getters, Q-B1)",
            "sram unique uncorrected counts: no ECC".into(),
        ),
    };
    Answer {
        status,
        params: b,
        label,
        note: format!("{} ({:#x}): {what}", row.name, row.id),
    }
}

fn filtered(ctx: &mut Ctx<'_>, row: &PublicControl, f: Filter, params: &[u8]) -> Answer {
    let Some(host) = ctx.host.as_deref_mut() else {
        return Answer {
            status: NV_ERR_NOT_SUPPORTED,
            params: params.to_vec(),
            label: "synthesized fallback (no host: NOT_SUPPORTED)",
            note: format!("{} ({:#x}): no host", row.name, row.id),
        };
    };
    let mut b = params.to_vec();
    let status = host.control(NV20_SUBDEVICE_0, row.id, &mut b);
    // The filter runs on every answer, whatever the status: a host that wrote the buffer
    // and failed must not leak what it wrote either.
    let (label, what) = match f {
        Filter::OfflinedPages => offlined_pages(&mut b, ctx.reservation),
        Filter::RemappedRows => remapped_rows(&mut b),
        Filter::ScrubberRange => scrubber_range(&mut b, ctx.reservation),
        Filter::DynamicOfflinedPages => dynamic_offlined_pages(&mut b, ctx.reservation),
        Filter::GpumonProcess => gpumon_process(&mut b, ctx.chip),
    };
    Answer {
        status,
        params: b,
        label,
        note: format!("{} ({:#x}): host status {status:#x}; {what}", row.name, row.id),
    }
}

/// Whether the 4 KiB page `page` lies in `[base, end)`.
fn page_inside(page: u64, (base, end): (u64, u64)) -> bool {
    page.checked_shl(p::RM_PAGE_SHIFT)
        .filter(|a| a >> p::RM_PAGE_SHIFT == page)
        .is_some_and(|a| base <= a && a < end)
}

/// `FB_GET_OFFLINED_PAGES`: keep the entries whose pages lie in the reservation, compacted
/// to the front; zero every other byte of the list. An entry is kept only if each of its
/// two addresses is either the invalid marker or inside, and at least one is inside. The
/// retirement-pending flags stay only when a kept entry is itself pending retirement.
fn offlined_pages(b: &mut [u8], res: Option<(u64, u64)>) -> (&'static str, String) {
    let n = (get32(b, p::OFFLINED_VALID_ENTRIES) as usize).min(p::OFFLINED_MAX);
    let mut kept: Vec<Vec<u8>> = Vec::new();
    let mut pending = false;
    for i in 0..n {
        let o = i * p::OFFLINED_ENTRY_SIZE;
        let e = &b[o..o + p::OFFLINED_ENTRY_SIZE];
        let addrs = [get64(e, p::OFFLINED_ECC_ON), get64(e, p::OFFLINED_ECC_OFF)];
        let inside = |a: u64| res.is_some_and(|r| page_inside(a, r));
        let keep = addrs.iter().any(|&a| inside(a))
            && addrs
                .iter()
                .all(|&a| a == p::OFFLINED_INVALID_ADDRESS || inside(a));
        if keep {
            pending |= get32(e, p::OFFLINED_STATUS) == p::OFFLINED_STATUS_PENDING_RETIREMENT;
            kept.push(e.to_vec());
        }
    }
    b[..p::OFFLINED_VALID_ENTRIES].fill(0);
    for (i, e) in kept.iter().enumerate() {
        let o = i * p::OFFLINED_ENTRY_SIZE;
        b[o..o + p::OFFLINED_ENTRY_SIZE].copy_from_slice(e);
    }
    put32(b, p::OFFLINED_VALID_ENTRIES, kept.len() as u32);
    if !pending {
        b[p::OFFLINED_B_RETIREMENT_PENDING] = 0;
        b[p::OFFLINED_RETIREMENT_PENDING] = 0;
    }
    let label = match res {
        Some(_) => "host (forwarded, filtered: offlined pages inside the VM's reservation; guest FB address = host VRAM address)",
        None => "host (forwarded, filtered: no reservation, so no host page is a guest page; empty list)",
    };
    (
        label,
        format!("{} of {n} host entries inside the reservation {res:x?}", kept.len()),
    )
}

/// `FB_GET_REMAPPED_ROWS`: a remapped row is a DRAM location (`remapRegVal`, FBPA,
/// sublocation), not an FB address, and the device has no map from one to the other, so
/// no entry can be shown to lie in the reservation: the list is emptied. The flags
/// (pending, failure) are the card's state, as the forwarded reset status is.
fn remapped_rows(b: &mut [u8]) -> (&'static str, String) {
    let n = get32(b, p::REMAPPED_ROWS_ENTRY_COUNT);
    put32(b, p::REMAPPED_ROWS_ENTRY_COUNT, 0);
    b[p::REMAPPED_ROWS_ENTRIES..].fill(0);
    (
        "host (forwarded, filtered: remapped rows cannot be placed in the reservation, empty list; card flags kept)",
        format!("{n} host entries withheld, flags {:#x}", b[p::REMAPPED_ROWS_FLAGS]),
    )
}

fn put64(b: &mut [u8], off: usize, v: u64) {
    b[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

/// `GPU_QUERY_SCRUBBER_STATUS`: the scrub range `[start, end)` clamped to the
/// reservation; 0 and 0 when they do not meet or there is no reservation. The status
/// and the remaining time are the card's.
fn scrubber_range(b: &mut [u8], res: Option<(u64, u64)>) -> (&'static str, String) {
    let (s, e) = (get64(b, p::SCRUBBER_START), get64(b, p::SCRUBBER_END));
    let (ns, ne) = match res {
        Some((base, end)) if s.max(base) < e.min(end) => (s.max(base), e.min(end)),
        _ => (0, 0),
    };
    put64(b, p::SCRUBBER_START, ns);
    put64(b, p::SCRUBBER_END, ne);
    (
        "host (forwarded, filtered: scrub range clamped to the VM's reservation)",
        format!("host range [{s:#x}, {e:#x}) -> [{ns:#x}, {ne:#x})"),
    )
}

/// `FB_GET_DYNAMIC_OFFLINED_PAGES`: keep the entries whose page lies in the reservation,
/// compacted, zero the rest and recount `validEntries`; `baseIndex` and `bMore` pass.
fn dynamic_offlined_pages(b: &mut [u8], res: Option<(u64, u64)>) -> (&'static str, String) {
    let n = (get32(b, p::DYN_OFFLINED_VALID_ENTRIES) as usize).min(p::DYN_OFFLINED_MAX);
    let es = p::DYN_OFFLINED_ENTRY_SIZE;
    let kept: Vec<Vec<u8>> = (0..n)
        .map(|i| b[i * es..(i + 1) * es].to_vec())
        .filter(|e| res.is_some_and(|r| page_inside(get64(e, 0), r)))
        .collect();
    b[..p::DYN_OFFLINED_VALID_ENTRIES].fill(0);
    for (i, e) in kept.iter().enumerate() {
        b[i * es..(i + 1) * es].copy_from_slice(e);
    }
    put32(b, p::DYN_OFFLINED_VALID_ENTRIES, kept.len() as u32);
    (
        "host (forwarded, filtered: dynamically offlined pages inside the VM's reservation)",
        format!("{} of {n} host entries inside the reservation {res:x?}", kept.len()),
    )
}

/// `PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2`: in every sample slot (the ring holds 72;
/// `count` says how many are valid and `tracker` where the oldest is, so all slots are
/// filtered) the host process fields of the six engine samples are zeroed, and so is the
/// utilization of an engine the device does not present (nvjpg and nvofa on TU106). The
/// utilization of the others is the card's, the host desktop's load included (P7,
/// accepted for telemetry under D9).
fn gpumon_process(b: &mut [u8], chip: &crate::chip::ChipData) -> (&'static str, String) {
    let presents = |engine: &str| {
        engine == "fb"
            || chip
                .device_info
                .engines
                .iter()
                .any(|e| e.name.trim_end_matches(|c: char| c.is_ascii_digit()) == engine)
    };
    let absent: Vec<&str> = p::GPUMON_ENGINES
        .iter()
        .map(|(e, _)| *e)
        .filter(|e| !presents(e))
        .collect();
    for i in 0..p::GPUMON_SAMPLE_COUNT {
        let s = p::GPUMON_SAMPLES + i * p::GPUMON_SAMPLE_SIZE;
        for (engine, off) in p::GPUMON_ENGINES {
            let e = s + off;
            b[e + p::GPUMON_ENGINE_PROC.start..e + p::GPUMON_ENGINE_PROC.end].fill(0);
            if absent.contains(engine) {
                put32(b, e + p::GPUMON_ENGINE_UTIL, 0);
            }
        }
    }
    (
        "host (forwarded, filtered: host process fields zeroed, engines the device lacks zeroed)",
        format!(
            "{} samples; process fields zeroed; absent engines {absent:?}",
            get32(b, p::GPUMON_COUNT)
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ver::active::NV_OK;

    /// A host that answers every control with a fixed parameter block.
    struct Canned(Vec<u8>, std::sync::Arc<std::sync::Mutex<Vec<(u32, u32)>>>);
    impl HostRm for Canned {
        fn control(&mut self, class: u32, cmd: u32, b: &mut [u8]) -> u32 {
            self.1.lock().unwrap().push((class, cmd));
            b.copy_from_slice(&self.0[..b.len()]);
            NV_OK
        }
    }

    const RES: (u64, u64) = (0xE900_0000, 0x1_E900_0000);

    fn tu106() -> &'static crate::chip::ChipData {
        crate::chip::by_name("tu106").unwrap().data()
    }

    fn offlined_answer(entries: &[(u64, u64, u32)], pending: bool) -> Vec<u8> {
        let mut b = vec![0u8; p::OFFLINED_PAGES_SIZE];
        for (i, &(on, off, status)) in entries.iter().enumerate() {
            let o = i * p::OFFLINED_ENTRY_SIZE;
            b[o..o + 8].copy_from_slice(&on.to_le_bytes());
            b[o + 8..o + 16].copy_from_slice(&off.to_le_bytes());
            b[o + 16] = 0x77; // rbcAddress, kept with its entry
            put32(&mut b, o + p::OFFLINED_STATUS, status);
        }
        put32(&mut b, p::OFFLINED_VALID_ENTRIES, entries.len() as u32);
        b[p::OFFLINED_B_RETIREMENT_PENDING] = pending as u8;
        b[p::OFFLINED_RETIREMENT_PENDING] = if pending { 3 } else { 0 };
        b
    }

    fn run(host: Vec<u8>, cmd: u32, res: Option<(u64, u64)>) -> (Answer, Vec<(u32, u32)>) {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = Canned(host, seen.clone());
        let mut ctx = Ctx {
            host: Some(&mut h),
            reservation: res,
            chip: tu106(),
            now_ns: 0,
            object: (0, 0),
            key: (0, 0),
            mirror: None,
            attrs: &mut GuestAttrs::default(),
        };
        let size = p::public_control(cmd).unwrap().size;
        let a = answer(&mut ctx, cmd, &vec![0u8; size]).unwrap();
        let s = seen.lock().unwrap().clone();
        (a, s)
    }

    /// Every address the guest gets back from FB_GET_OFFLINED_PAGES lies in the
    /// reservation; host pages below and above it never reach the guest, nor does a
    /// pending flag that belongs to a withheld page.
    #[test]
    fn offlined_pages_outside_the_reservation_never_reach_the_guest() {
        let pg = |a: u64| a >> 12;
        let inv = p::OFFLINED_INVALID_ADDRESS;
        let host = offlined_answer(
            &[
                (inv, pg(0x0360_0000), 1),                 // below B: pending, withheld
                (inv, pg(RES.0), 0),                       // first page inside
                (pg(RES.1 - 0x1000), pg(RES.1 - 0x1000), 0), // last page inside
                (inv, pg(RES.1), 0),                       // E itself: outside
                (pg(RES.0 + 0x2000), pg(0x1F76_0000), 0), // one address outside: withheld
                (inv, inv, 0),                             // no address: withheld
                (inv, u64::MAX >> 4, 0),                   // overflows when shifted
            ],
            true,
        );
        let (a, seen) = run(host, 0x2080_1322, Some(RES));
        assert_eq!(seen, [(NV20_SUBDEVICE_0, 0x2080_1322)]);
        assert_eq!(a.status, NV_OK);
        assert_eq!(get32(&a.params, p::OFFLINED_VALID_ENTRIES), 2);
        let entry = |i: usize| {
            let o = i * p::OFFLINED_ENTRY_SIZE;
            (get64(&a.params, o), get64(&a.params, o + 8), a.params[o + 16])
        };
        assert_eq!(entry(0), (inv, pg(RES.0), 0x77));
        assert_eq!(entry(1), (pg(RES.1 - 0x1000), pg(RES.1 - 0x1000), 0x77));
        assert!(a.params[2 * p::OFFLINED_ENTRY_SIZE..p::OFFLINED_VALID_ENTRIES]
            .iter()
            .all(|&x| x == 0));
        assert_eq!(a.params[p::OFFLINED_B_RETIREMENT_PENDING], 0);
        assert_eq!(a.params[p::OFFLINED_RETIREMENT_PENDING], 0);
        // Every address the guest sees, over all 64 slots: invalid or inside.
        for i in 0..p::OFFLINED_MAX {
            for off in [p::OFFLINED_ECC_ON, p::OFFLINED_ECC_OFF] {
                let x = get64(&a.params, i * p::OFFLINED_ENTRY_SIZE + off);
                assert!(
                    x == 0 || x == inv || page_inside(x, RES),
                    "slot {i}: {x:#x}"
                );
            }
        }
        // A kept page pending retirement keeps the flags.
        let host = offlined_answer(&[(inv, pg(RES.0 + 0x5000), 1)], true);
        let (a, _) = run(host, 0x2080_1322, Some(RES));
        assert_eq!(get32(&a.params, p::OFFLINED_VALID_ENTRIES), 1);
        assert_eq!(a.params[p::OFFLINED_B_RETIREMENT_PENDING], 1);
    }

    /// Through the dispatcher: the rows are not plain forwards any more, and the answer
    /// the guest kernel gets holds only pages inside the reservation.
    #[test]
    fn the_dispatcher_filters() {
        use crate::ctrl::{Dispatcher, INTERNAL_CLIENT, INTERNAL_SUBDEVICE};
        for id in [0x2080_1322u32, 0x2080_1344] {
            assert!(!crate::ver::active::FORWARD.iter().any(|(c, _)| *c == id));
        }
        let inv = p::OFFLINED_INVALID_ADDRESS;
        let host = offlined_answer(&[(inv, 0x3600, 0), (inv, (RES.0 >> 12) + 1, 0)], false);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let f = crate::fixture::rtx2070();
        let mut d = Dispatcher::new(
            f.chip.data(),
            &f.identity(4 << 30),
            Some(Box::new(Canned(host, seen))),
        )
        .with_reservation(Some(RES));
        let a = d.control(
            INTERNAL_CLIENT,
            INTERNAL_SUBDEVICE,
            0x2080_1322,
            &[0u8; p::OFFLINED_PAGES_SIZE],
        );
        assert_eq!(get32(&a.params, p::OFFLINED_VALID_ENTRIES), 1);
        assert_eq!(get64(&a.params, p::OFFLINED_ECC_OFF), (RES.0 >> 12) + 1);
    }

    /// Without a reservation no host page is a guest page: the list is empty.
    #[test]
    fn offlined_pages_without_reservation_are_empty() {
        let host = offlined_answer(&[(u64::MAX, 0x1000, 0), (0x2000, 0x2000, 0)], false);
        let (a, _) = run(host, 0x2080_1322, None);
        assert_eq!(get32(&a.params, p::OFFLINED_VALID_ENTRIES), 0);
        assert!(a.params.iter().all(|&x| x == 0));
        assert!(a.label.contains("no reservation"), "{}", a.label);
    }

    /// Remapped rows never reach the guest; the card flags do.
    #[test]
    fn remapped_rows_are_withheld() {
        let mut host = vec![0xABu8; p::REMAPPED_ROWS_SIZE];
        put32(&mut host, p::REMAPPED_ROWS_ENTRY_COUNT, 3);
        host[p::REMAPPED_ROWS_FLAGS] = 1;
        let (a, _) = run(host, 0x2080_1344, Some(RES));
        assert_eq!(get32(&a.params, p::REMAPPED_ROWS_ENTRY_COUNT), 0);
        assert_eq!(a.params[p::REMAPPED_ROWS_FLAGS], 1);
        assert!(a.params[p::REMAPPED_ROWS_ENTRIES..].iter().all(|&x| x == 0));
    }

    /// A buffer of another size is refused before any host call.
    #[test]
    fn wrong_size_is_not_forwarded() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = Canned(vec![0; 8192], seen.clone());
        let mut ctx = Ctx {
            host: Some(&mut h),
            reservation: Some(RES),
            chip: tu106(),
            now_ns: 0,
            object: (0, 0),
            key: (0, 0),
            mirror: None,
            attrs: &mut GuestAttrs::default(),
        };
        let a = answer(&mut ctx, 0x2080_1322, &[0u8; 2048]).unwrap();
        assert_eq!(a.status, NV_ERR_INVALID_PARAM_STRUCT);
        assert!(seen.lock().unwrap().is_empty());
    }

    /// The scrub range the guest sees lies in the reservation, or is empty.
    #[test]
    fn scrubber_range_is_clamped() {
        let mk = |s: u64, e: u64| {
            let mut b = vec![0u8; p::SCRUBBER_SIZE];
            put32(&mut b, 0, 1);
            put64(&mut b, p::SCRUBBER_START, s);
            put64(&mut b, p::SCRUBBER_END, e);
            b
        };
        let range = |a: &Answer| {
            (
                get64(&a.params, p::SCRUBBER_START),
                get64(&a.params, p::SCRUBBER_END),
            )
        };
        let (a, _) = run(mk(0, 0x2_0000_0000), 0x2080_015F, Some(RES));
        assert_eq!(range(&a), RES);
        assert_eq!(get32(&a.params, 0), 1, "status passes");
        let (a, _) = run(mk(0x1000, 0x2000), 0x2080_015F, Some(RES));
        assert_eq!(range(&a), (0, 0));
        let (a, _) = run(mk(RES.0 + 0x1000, 0x2_0000_0000), 0x2080_015F, Some(RES));
        assert_eq!(range(&a), (RES.0 + 0x1000, RES.1));
        let (a, _) = run(mk(0, 0x2_0000_0000), 0x2080_015F, None);
        assert_eq!(range(&a), (0, 0));
    }

    /// Dynamic offlined pages: three host pages (below, inside, above), one reaches the guest.
    #[test]
    fn dynamic_offlined_pages_keep_the_reservation() {
        let mut b = vec![0u8; p::DYN_OFFLINED_SIZE];
        for (i, a) in [0x1000u64, RES.0 + 0x3000, RES.1 + 0x1000]
            .into_iter()
            .enumerate()
        {
            put64(&mut b, i * 16, a >> 12);
            b[i * 16 + 8] = 2;
        }
        put32(&mut b, p::DYN_OFFLINED_VALID_ENTRIES, 3);
        b[1032] = 1; // bMore
        let (a, seen) = run(b, 0x2080_1348, Some(RES));
        assert_eq!(seen, [(NV20_SUBDEVICE_0, 0x2080_1348)]);
        assert_eq!(get32(&a.params, p::DYN_OFFLINED_VALID_ENTRIES), 1);
        assert_eq!(get64(&a.params, 0), (RES.0 + 0x3000) >> 12);
        assert_eq!(a.params[8], 2);
        assert!(a.params[16..p::DYN_OFFLINED_VALID_ENTRIES]
            .iter()
            .all(|&x| x == 0));
        assert_eq!(a.params[1032], 1);
    }

    /// A host PID and process name in a GPUMON sample come back as 0; nvjpg and nvofa,
    /// which TU106 lacks, carry no utilization; the others keep the card's.
    #[test]
    fn gpumon_host_processes_are_zeroed() {
        let mut b = vec![0u8; p::GPUMON_SIZE];
        put32(&mut b, p::GPUMON_COUNT, 72);
        for i in 0..p::GPUMON_SAMPLE_COUNT {
            let s = p::GPUMON_SAMPLES + i * p::GPUMON_SAMPLE_SIZE;
            for (_, off) in p::GPUMON_ENGINES {
                let e = s + off;
                put32(&mut b, e, 4200); // util
                put32(&mut b, e + 8, 1234); // procId: a host PID
                put32(&mut b, e + 12, 99);
                b[e + 16..e + 24].copy_from_slice(b"Xwayland");
                put64(&mut b, e + 120, 0xFFFF_8880_1234_5678);
            }
        }
        let (a, _) = run(b, 0x2080_2096, Some(RES));
        for i in 0..p::GPUMON_SAMPLE_COUNT {
            let s = p::GPUMON_SAMPLES + i * p::GPUMON_SAMPLE_SIZE;
            for (engine, off) in p::GPUMON_ENGINES {
                let e = s + off;
                assert!(a.params[e + 8..e + 128].iter().all(|&x| x == 0), "{engine}");
                let want = if ["nvjpg", "nvofa"].contains(engine) {
                    0
                } else {
                    4200
                };
                assert_eq!(get32(&a.params, e), want, "{engine}");
            }
        }
        assert_eq!(get32(&a.params, p::GPUMON_COUNT), 72);
    }

    /// The RM heap stats are the device's: zeros, and the host is not asked.
    #[test]
    fn rm_heap_stats_are_the_devices() {
        let mut b = vec![0xEEu8; p::RM_HEAP_STATS_SIZE];
        put32(&mut b, 0, 3); // gfid of a VF
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = Canned(vec![0x11; 8192], seen.clone());
        let mut ctx = Ctx {
            host: Some(&mut h),
            reservation: Some(RES),
            chip: tu106(),
            now_ns: 0,
            object: (0, 0),
            key: (0, 0),
            mirror: None,
            attrs: &mut GuestAttrs::default(),
        };
        let a = answer(&mut ctx, 0x2080_3602, &b).unwrap();
        assert!(seen.lock().unwrap().is_empty());
        assert_eq!(a.status, NV_OK);
        assert!(a.params.iter().all(|&x| x == 0));
    }

    /// Guard (hostrm.rs:84 used to send any other class to the host root client): every
    /// FORWARD row and every filtered row of the table reaches the host only on the
    /// subdevice, device or BINAPI object, the one its id addresses, whatever object the
    /// guest named; and the two lists never overlap.
    #[test]
    fn every_forward_targets_subdevice_device_or_binapi() {
        use crate::ctrl::{Dispatcher, INTERNAL_CLIENT, INTERNAL_SUBDEVICE};
        use crate::hostrm::{class_of_control, host_object};
        use crate::ver::active::FORWARD;
        type Seen = std::sync::Arc<std::sync::Mutex<Vec<(u32, u32)>>>;
        struct Rec(Seen);
        impl HostRm for Rec {
            fn control(&mut self, class: u32, cmd: u32, _: &mut [u8]) -> u32 {
                self.0.lock().unwrap().push((class, cmd));
                NV_OK
            }
        }
        let forwarded: Vec<(u32, &str)> = FORWARD
            .iter()
            .copied()
            .chain(
                p::PUBLIC
                    .iter()
                    .filter(|c| matches!(c.rule, Rule::Filtered(_)))
                    .map(|c| (c.id, c.name)),
            )
            .collect();
        for &(id, name) in &forwarded {
            let class = class_of_control(id).unwrap_or_else(|| panic!("{name}: no host object"));
            assert!(host_object(class).is_some(), "{name}");
            assert!(
                !(FORWARD.iter().any(|(c, _)| *c == id) && p::public_control(id).is_some()),
                "{name} is in FORWARD and in the table"
            );
        }
        let seen: Seen = Default::default();
        let f = crate::fixture::rtx2070();
        let mut d = Dispatcher::new(
            f.chip.data(),
            &f.identity(4 << 30),
            Some(Box::new(Rec(seen.clone()))),
        )
        .with_reservation(Some(RES));
        // The guest names objects of other classes too: a diag object and a ZBC object.
        d.alloc(0xA, 0xA, 0, crate::ver::active::NV01_ROOT, &[]);
        d.alloc(0xA, 0x20, 0xA, 0x208F, &[]);
        d.alloc(0xA, 0x21, 0xA, 0x9096, &[]);
        for &(id, name) in &forwarded {
            let size = p::public_control(id).map_or(64, |c| c.size);
            for (c, o) in [(INTERNAL_CLIENT, INTERNAL_SUBDEVICE), (0xA, 0x20), (0xA, 0x21)] {
                seen.lock().unwrap().clear();
                d.control(c, o, id, &vec![0u8; size]);
                assert_eq!(
                    seen.lock().unwrap().as_slice(),
                    &[(class_of_control(id).unwrap(), id)],
                    "{name} on {o:#x}"
                );
            }
        }
    }

    /// The 26 F1 rows of b-host-forwardable.md are plain forwards, among them the two
    /// illumination controls seen in the run logs.
    #[test]
    fn f1_rows_are_forwarded() {
        use crate::ver::active::FORWARD;
        let f1 = [
            0x2080_0133u32, 0x2080_0153, 0x2080_0154, 0x2080_0173, 0x2080_01AB, 0x2080_01AE,
            0x2080_0809, 0x2080_1037, 0x2080_1120, 0x2080_130C, 0x2080_1328, 0x2080_1347,
            0x2080_1355, 0x2080_1358, 0x2080_1816, 0x2080_1817, 0x2080_200B, 0x2080_205A,
            0x2080_206E, 0x2080_3101, 0x2080_3400, 0x2080_3401, 0x2080_3404, 0x2080_3406,
            0x2080_3604, 0x2080_81F3,
        ];
        assert_eq!(f1.len(), 26);
        for id in f1 {
            assert!(FORWARD.iter().any(|(c, _)| *c == id), "{id:#x}");
        }
    }

    fn ask(cmd: u32, params: &[u8], now_ns: u64) -> (Answer, Vec<(u32, u32)>) {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = Canned(vec![0x5A; 65536], seen.clone());
        let mut ctx = Ctx {
            host: Some(&mut h),
            reservation: Some(RES),
            chip: tu106(),
            now_ns,
            object: (0, 0),
            key: (0, 0),
            mirror: None,
            attrs: &mut GuestAttrs::default(),
        };
        let a = answer(&mut ctx, cmd, params).unwrap();
        let s = seen.lock().unwrap().clone();
        (a, s)
    }

    /// The S rows: answered by the device, never sent to the host.
    #[test]
    fn device_rows_never_reach_the_host() {
        // Time correlation: every requested sample is the device clock.
        let mut t = vec![0u8; p::TIME_CORR_SIZE];
        t[0] = 0x14; // _GSP_OS on _PROCESSOR_GSP
        t[1] = 3;
        let (a, seen) = ask(0x2080_0406, &t, 123_456);
        assert!(seen.is_empty());
        assert_eq!(a.status, NV_OK);
        for i in 0..3 {
            let o = p::TIME_CORR_SAMPLES + 16 * i;
            assert_eq!((get64(&a.params, o), get64(&a.params, o + 8)), (123_456, 123_456));
        }
        assert_eq!(get64(&a.params, p::TIME_CORR_SAMPLES + 48), 0);
        // NBSI: not found, nothing returned.
        let (a, seen) = ask(0x2080_080E, &vec![0x77u8; p::NBSI_V2_SIZE], 0);
        assert!(seen.is_empty());
        assert_eq!(get32(&a.params, p::NBSI_V2_ERROR_CODE), p::NBSI_NOT_FOUND);
        assert_eq!(get32(&a.params, p::NBSI_V2_RET_SIZE), 0);
        let buf = p::NBSI_V2_RET_SIZE - p::NBSI_V2_RET_BUF_LEN;
        assert!(a.params[buf..p::NBSI_V2_RET_SIZE].iter().all(|&x| x == 0));
        assert!(a.params[..buf].iter().all(|&x| x == 0x77), "the request is kept");
        // RC recovery, comptags, I2C port speed, SRAM ECC.
        let (a, _) = ask(0x2080_220E, &[0u8; 4], 0);
        assert_eq!(get32(&a.params, 0), p::RC_RECOVERY_ENABLED);
        let (a, _) = ask(0x2080_1338, &vec![0xFFu8; p::CTAGS_SIZE], 0);
        assert!(a.params.iter().all(|&x| x == 0));
        let (a, _) = ask(0x402C_0103, &[0xFFu8; 64], 0);
        assert!(a.params.iter().all(|&x| x == 0));
        let (a, seen) = ask(0x2080_3402, &[0u8; 16], 0);
        assert!(seen.is_empty());
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
    }

    /// The 13 R rows and the 4 GPUACCT rows: NOT_SUPPORTED with a label, request echoed,
    /// no host call.
    #[test]
    fn refusals_are_labelled_and_local() {
        let refused: Vec<&PublicControl> = p::PUBLIC
            .iter()
            .filter(|c| matches!(c.rule, Rule::Refuse(_)))
            .collect();
        assert_eq!(refused.len(), 13 + 4);
        for c in refused {
            let (a, seen) = ask(c.id, &[0xA5; 24], 0);
            assert!(seen.is_empty(), "{}", c.name);
            assert_eq!(a.status, NV_ERR_NOT_SUPPORTED, "{}", c.name);
            assert!(a.label.starts_with("refused ("), "{}", c.name);
            assert_eq!(a.params, [0xA5; 24], "{}", c.name);
        }
    }

    /// GPU_QUERY_ILLUM_SUPPORT and GPU_GET_ILLUM, the rows seen in the run logs: plain
    /// forwards whose host status and bytes reach the guest unchanged.
    #[test]
    fn illum_rows_are_forwarded_as_is() {
        use crate::ctrl::{Dispatcher, INTERNAL_CLIENT, INTERNAL_SUBDEVICE};
        struct Illum;
        impl HostRm for Illum {
            fn control(&mut self, class: u32, cmd: u32, b: &mut [u8]) -> u32 {
                assert_eq!(class, NV20_SUBDEVICE_0);
                match cmd {
                    0x2080_0153 => NV_OK, // the host card: attribute 0, not supported (Q-B1)
                    0x2080_0154 => NV_ERR_NOT_SUPPORTED,
                    _ => {
                        b.fill(0xEE);
                        0xFFFF
                    }
                }
            }
        }
        let f = crate::fixture::rtx2070();
        let mut d = Dispatcher::new(f.chip.data(), &f.identity(4 << 30), Some(Box::new(Illum)));
        let a = d.control(INTERNAL_CLIENT, INTERNAL_SUBDEVICE, 0x2080_0153, &[0u8; 8]);
        assert_eq!((a.status, &a.params[..]), (NV_OK, &[0u8; 8][..]));
        assert_eq!(a.label, "host (forwarded as-is, public control)");
        let a = d.control(INTERNAL_CLIENT, INTERNAL_SUBDEVICE, 0x2080_0154, &[0u8; 8]);
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
        assert_eq!(a.label, "host (forwarded as-is, public control)");
    }

    type Calls = std::sync::Arc<std::sync::Mutex<Vec<(u32, u32, u32)>>>;

    /// A mirror that holds host twins of the guest objects in `mirrored` only.
    struct FakeMirror {
        mirrored: Vec<(u32, u32)>,
        calls: Calls,
    }
    impl MirrorObjects for FakeMirror {
        fn forward(&mut self, c: u32, o: u32, cmd: u32, _: &[u8]) -> Option<(u32, String)> {
            if !self.mirrored.contains(&(c, o)) {
                return None;
            }
            self.calls.lock().unwrap().push((c, o, cmd));
            Some((NV_OK, format!("host object of {o:#x}")))
        }
    }

    /// One control of the C table on guest object `obj`, with a host RM that must never be
    /// called and a mirror of `mirrored`.
    fn guest_call(
        attrs: &mut GuestAttrs,
        mirrored: &[(u32, u32)],
        obj: (u32, u32),
        cmd: u32,
        params: &[u8],
    ) -> (Answer, Vec<(u32, u32, u32)>) {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = Canned(vec![0; 8192], seen.clone());
        let calls: Calls = Default::default();
        let mut m = FakeMirror {
            mirrored: mirrored.to_vec(),
            calls: calls.clone(),
        };
        let mut ctx = Ctx {
            host: Some(&mut h),
            reservation: Some(RES),
            chip: tu106(),
            now_ns: 0,
            object: obj,
            key: obj,
            mirror: Some(&mut m),
            attrs,
        };
        let a = answer(&mut ctx, cmd, params).unwrap();
        assert!(
            seen.lock().unwrap().is_empty(),
            "{cmd:#x} reached the host RM client"
        );
        let c = calls.lock().unwrap().clone();
        (a, c)
    }

    const CH: (u32, u32) = (0xC1, 0x5C00_0010);
    const OTHER: (u32, u32) = (0xC1, 0x5C00_0020);

    /// The resets act on the guest's own mirror channel only, as the public control; a
    /// channel without a mirror is answered by the device.
    #[test]
    fn resets_go_to_the_own_mirror_channel_only() {
        let mut attrs = GuestAttrs::default();
        for (cmd, host_cmd, size) in [
            (0x906F_0102u32, 0x906F_0102u32, 16usize),
            (0x506F_0105, 0x506F_0105, 8),
            (0x506F_0106, 0x506F_0105, 8),
        ] {
            let (a, calls) = guest_call(&mut attrs, &[CH, OTHER], CH, cmd, &vec![1u8; size]);
            assert_eq!(a.status, NV_OK);
            assert_eq!(calls, [(CH.0, CH.1, host_cmd)], "{cmd:#x}");
            assert!(a.label.starts_with("host (reset"), "{}", a.label);
            // No mirror: the device answers, nothing goes anywhere.
            let (a, calls) = guest_call(&mut attrs, &[OTHER], CH, cmd, &vec![1u8; size]);
            assert_eq!(a.status, NV_OK);
            assert!(calls.is_empty());
            assert!(a.label.starts_with("guest (no mirror"), "{}", a.label);
        }
        assert_eq!(attrs.get(CH).unwrap().resets, 6);
    }

    /// The fault state and the RC journal are the device's: zeros, no host call.
    #[test]
    fn fault_and_rc_state_come_from_the_device() {
        let mut attrs = GuestAttrs::default();
        for (cmd, size) in [
            (0x906F_0106u32, p::MMU_FAULT_INFO_SIZE),
            (0x906F_0105, 1),
            (0x2080_2205, 4),
        ] {
            let (a, calls) = guest_call(&mut attrs, &[CH], CH, cmd, &vec![0xEEu8; size]);
            assert!(calls.is_empty(), "{cmd:#x}");
            assert_eq!(a.status, NV_OK);
            assert!(a.params.iter().all(|&x| x == 0), "{cmd:#x}");
        }
        let mut q = vec![0xEEu8; p::RC_ERROR_V2_SIZE];
        put32(&mut q, 0, 2);
        let (a, _) = guest_call(&mut attrs, &[CH], CH, 0x2080_2213, &q);
        assert_eq!(get32(&a.params, 0), 2, "whichBuffer kept");
        assert_eq!(get32(&a.params, p::RC_ERROR_V2_OUTPUT_SIZE), 0);
        assert!(a.params[8..].iter().all(|&x| x == 0));
    }

    /// Group M records and forwards to the object's own mirror; a level above MEDIUM stays
    /// recorded; the getter reads the record back.
    #[test]
    fn group_m_records_and_forwards() {
        let mut attrs = GuestAttrs::default();
        let tsg = (0xC1, 0x5C00_0001);
        let (a, calls) = guest_call(&mut attrs, &[tsg], tsg, 0xA06C_0107, &1u32.to_le_bytes());
        assert_eq!(a.status, NV_OK);
        assert_eq!(calls, [(tsg.0, tsg.1, 0xA06C_0107)]);
        let (a, calls) = guest_call(&mut attrs, &[tsg], tsg, 0xA06C_0107, &2u32.to_le_bytes());
        assert_eq!(a.status, NV_OK);
        assert!(calls.is_empty(), "HIGH is not forwarded");
        let (_, calls) = guest_call(&mut attrs, &[CH], CH, 0xA06F_0109, &2u32.to_le_bytes());
        assert!(calls.is_empty(), "HIGH is not forwarded");
        // TPC partition mode: set (forwarded), then read back without a host call.
        let mut set = [0u8; 8];
        set[0] = 1; // STATIC
        set[4] = 1;
        let (_, calls) = guest_call(&mut attrs, &[tsg], tsg, 0x0090_0101, &set);
        assert_eq!(calls, [(tsg.0, tsg.1, 0x0090_0101)]);
        let (a, calls) = guest_call(&mut attrs, &[tsg], tsg, 0x0090_0103, &[0u8; 8]);
        assert!(calls.is_empty());
        assert_eq!(a.params, set);
        // Unset on another TSG: NONE.
        let (a, _) = guest_call(&mut attrs, &[], (0xC1, 0x5C00_0002), 0x0090_0103, &[9u8; 8]);
        assert_eq!(a.params, [0u8; 8]);
        // Without a mirror: recorded only.
        let (a, calls) = guest_call(&mut attrs, &[], CH, 0xA06C_0109, &[3u8; 8]);
        assert!(calls.is_empty());
        assert_eq!(a.label, "guest (recorded; no mirror object)");
        // FREE forgets the object's record.
        attrs.free(tsg.0, tsg.1);
        assert!(attrs.get(tsg).is_none());
        attrs.free(0xC1, 0xC1);
        assert!(attrs.get(CH).is_none());
    }

    /// Q-C7: MAKE_REALTIME and RESTART_RUNLIST are recorded and never leave the device, not
    /// even on a mirrored object.
    #[test]
    fn realtime_and_runlist_restart_are_never_forwarded() {
        let mut attrs = GuestAttrs::default();
        for (cmd, size) in [(0xA06C_0110u32, 1usize), (0xA06F_0111, 2)] {
            let (a, calls) = guest_call(&mut attrs, &[CH], CH, cmd, &vec![1u8; size]);
            assert_eq!(a.status, NV_OK);
            assert!(calls.is_empty(), "{cmd:#x}");
            assert!(a.label.contains("Q-C7"), "{}", a.label);
            assert!(attrs.get(CH).unwrap().recorded.contains_key(&cmd));
        }
    }

    /// Groups V, K and X are refused without any host or mirror call.
    #[test]
    fn c_refusals() {
        let mut attrs = GuestAttrs::default();
        let refused: Vec<&PublicControl> = p::PUBLIC_C
            .iter()
            .filter(|c| matches!(c.rule, Rule::Refuse(_)))
            .collect();
        assert_eq!(refused.len(), 8 + 5 + 2);
        for c in refused {
            let (a, calls) = guest_call(&mut attrs, &[CH], CH, c.id, &[0x3C; 16]);
            assert!(calls.is_empty(), "{}", c.name);
            assert_eq!(a.status, NV_ERR_NOT_SUPPORTED, "{}", c.name);
            assert!(a.label.starts_with("refused ("), "{}", c.name);
        }
    }

    /// NV_EVENT_BUFFER UPDATE_GET is left to the event-buffer model (ALLOC_MEMORY).
    #[test]
    fn event_buffer_update_get_is_not_in_the_table() {
        assert!(p::public_control(0x90CD_0102).is_none());
    }

    /// Through the dispatcher (no mirror, no host): every C row reaches this table ahead of
    /// the other handlers, and a record dies with its object.
    #[test]
    fn the_dispatcher_takes_the_c_rows() {
        use crate::ctrl::Dispatcher;
        let f = crate::fixture::rtx2070();
        let mut d = Dispatcher::new(f.chip.data(), &f.identity(4 << 30), None);
        for c in p::PUBLIC_C {
            let size = if c.size == 0 { 16 } else { c.size };
            let a = d.control(CH.0, CH.1, c.id, &vec![0u8; size]);
            assert_ne!(a.label, "unanswered", "{}", c.name);
            assert!(
                a.label.starts_with("guest (") || a.label.starts_with("refused ("),
                "{}: {}",
                c.name,
                a.label
            );
        }
        assert!(d.guest_attrs.get(CH).is_some());
        d.free(CH.0, CH.1);
        assert!(d.guest_attrs.get(CH).is_none());
    }
}

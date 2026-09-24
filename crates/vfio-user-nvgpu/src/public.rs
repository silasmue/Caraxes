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
//!
//! Every host call goes through `HostRm::control` with the subdevice class, so a row can
//! never reach another host object (hostrm.rs).

use crate::ctrl::{Answer, HostRm};
use crate::ver::active::public::{self as p, Dev, Filter, PublicControl, Rule};
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
        Rule::Device(d) => device(row, d, params),
    })
}

fn device(row: &PublicControl, d: Dev, params: &[u8]) -> Answer {
    let n = params.len();
    let (status, b, label, what): (u32, Vec<u8>, &'static str, String) = match d {
        Dev::RmHeapStats => (
            NV_OK,
            vec![0u8; n],
            "synthesized (the device's own GSP heap view: no RM heap, all 0; the host GSP is never asked, decision D9/Q-B6)",
            "rm heap stats: zeros".into(),
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
}

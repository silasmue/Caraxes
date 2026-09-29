// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Rows the CUDA path needs beyond nvidia-smi and nvidia-uvm (experiment E5c,
//! docs/caraxes/experiments.md; the ordered call list is docs/caraxes/testing.md §8).
//! Each answer carries its provenance label (decision D9). Host-forwarded CUDA rows
//! that need no translation are in the `FORWARD` and `LEGACY` lists of `src/ver` and
//! answered by nvml.rs; this module holds the rows that are synthesized or derived.

use crate::channel::Registry;
use crate::chip::ChipData;
use crate::ctrl::{Answer, HostRm};
use crate::ver::common::{self as v, NV_OK};

fn answer(params: Vec<u8>, label: &'static str, note: String) -> Answer {
    Answer {
        status: NV_OK,
        params,
        label,
        note,
    }
}

/// The controls of the context path that real GSP acts on (clock limits, exception masks,
/// preemption modes, timeslices, preemption, channel stop, eviction) and that the device
/// acknowledges without effect, because no host channel exists yet (part 2 maps them to
/// the host objects; cuda-lookahead.md §6.3). Native status for each is NV_OK
/// (matrix/traces/610.57.04/cuda-core.tsv and cuda-launch.tsv). The clock boost
/// `PERF_BOOST_SET_2X` is acknowledged the same way and handed to the host boost
/// (hostboost.rs).
const ACKNOWLEDGED: &[(u32, &str)] = &[
    (
        v::CMD_KGR_INTERNAL_INIT_BUG4208224_WAR,
        "KGR_INIT_BUG4208224_WAR (no WAR channel run)",
    ),
    (
        v::CMD_INTERNAL_PERF_CUDA_LIMIT_SET_CONTROL,
        "PERF_CUDA_LIMIT_SET_CONTROL (no clock limit)",
    ),
    (
        v::CMD_INTERNAL_PERF_CUDA_LIMIT_DISABLE,
        "PERF_CUDA_LIMIT_DISABLE (no clock limit)",
    ),
    (
        v::CMD_DEBUG_SET_EXCEPTION_MASK,
        "DEBUG_SET_EXCEPTION_MASK (no SM to trap)",
    ),
    (v::CMD_TSG_SET_TIMESLICE, "TSG SET_TIMESLICE (no scheduler)"),
    (
        v::CMD_TSG_PREEMPT,
        "TSG PREEMPT (the emulated channels run to completion at the doorbell)",
    ),
    (
        v::CMD_CHANNEL_STOP,
        "CHANNEL STOP (the emulated channel is idle between doorbells)",
    ),
    (v::CMD_GPU_EVICT_CTX, "GPU_EVICT_CTX (no context resident)"),
    (
        v::CMD_MC_SERVICE_INTERRUPTS,
        "MC_SERVICE_INTERRUPTS (no interrupt pending)",
    ),
];

/// Answer a CUDA-path control, or `None` when `cmd` is not one of them.
pub fn answer_control(
    chip: &'static ChipData,
    cmd: u32,
    params: &[u8],
    channels: &mut Registry,
    host: Option<&mut Box<dyn HostRm>>,
) -> Option<Answer> {
    let n = params.len();
    if let Some((_, what)) = ACKNOWLEDGED.iter().find(|(c, _)| *c == cmd) {
        return Some(answer(
            params.to_vec(),
            "synthesized (NV_OK echo, nothing acted on: no host channel before part 2)",
            format!("{what}: ok"),
        ));
    }
    // A 570.144 guest names the same WAR on the subdevice (ver::v570; 610 reused the id).
    if cmd == crate::ver::v570::CMD_INTERNAL_KGR_INIT_BUG4208224_WAR
        && crate::ver::guest() == crate::ver::GuestAbi::V570
    {
        return Some(answer(
            params.to_vec(),
            "synthesized (NV_OK echo, nothing acted on: no host channel before part 2)",
            "KGR_INIT_BUG4208224_WAR of 570.144 (no WAR channel run): ok".into(),
        ));
    }
    Some(match cmd {
        // The guest kernel's copy of its client's PERF_BOOST (cuCtxCreate: BOOST_TO_MAX |
        // CUDA, infinite; the clear at teardown; kern_perf_boost.c:100-122). NV_OK echo as
        // native; the device's host boost renews with the guest's command and its own
        // finite duration (hostboost.rs; docs/research/host-boost-2026-09-28.md).
        v::CMD_INTERNAL_PERF_BOOST_SET_2X => answer(
            params.to_vec(),
            "synthesized (NV_OK echo; the host boost acts on it with its own duration)",
            crate::hostboost::guest_request(params),
        ),
        // GPU_PROMOTE_CTX (PRIVILEGED, so not forwardable unprivileged): the golden and
        // WAR channels at module load, then per CUDA channel the kernel's PAs at the
        // compute-object alloc and UVM's VAs at channel binding. Echo (the kernel re-reads
        // bInitialize, kernel_graphics_object.c:140-156) and record for part 2.
        v::CMD_GPU_PROMOTE_CTX => {
            let note = channels.record_promote_ctx(params);
            answer(
                params.to_vec(),
                "guest (promote entries recorded; echo)",
                note,
            )
        }
        // cuInit: GPU_GET_SKYLINE_INFO in the guest kernel asks for the internal table
        // when its MIG static info is empty (subdevice_ctrl_gpu_smc.c:59-70). A chip
        // without MIG has no skylines: validEntries 0, as the real card's public
        // GPU_GET_SKYLINE_INFO answers (all zeros, matrix/traces/610.57.04/cuda-core.tsv:219).
        v::CMD_INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO if !chip.gr.mig => {
            let b = vec![0u8; n];
            answer(
                b,
                "synthesized (no MIG on this chip: no skylines, validEntries 0)",
                format!("grmgr skyline info: 0 entries ({n} bytes)"),
            )
        }
        v::CMD_CE_GET_LCE_SHIM_INFO => {
            lce_shim_info(chip, params, host, crate::hostabi::current())
        }
        _ => return None,
    })
}

/// `GR_SET_CTXSW_PREEMPTION_MODE` (0x20801210) of client `h_client`: NV_OK echo, as native
/// and as before (matrix/traces/610.57.04: graphics clients ask compute CTA on their TSG,
/// CUDA-class clients CILP). The payload is decoded and recorded under the guest TSG its
/// hChannel stands for (channel.rs `record_preemption`); the mirror binds its compute mode
/// on the host TSG at the build (preempt.rs, mirror.rs). The first SET of each payload
/// shape goes to the device log. Returns the answer and the guest object recorded.
pub fn set_ctxsw_preemption_mode(
    h_client: u32,
    params: &[u8],
    channels: &mut Registry,
) -> (Answer, Option<(u32, u32)>) {
    use crate::preempt::SetRequest;
    let Some(r) = SetRequest::decode(params) else {
        channels.note_short_preemption();
        return (
            answer(
                params.to_vec(),
                "synthesized (NV_OK echo; too short to decode, nothing recorded)",
                format!("GR_SET_CTXSW_PREEMPTION_MODE: {} bytes, not decoded", params.len()),
            ),
            None,
        );
    };
    let (guest, rec, new_shape) = channels.record_preemption(h_client, &r);
    let whom = match (guest, rec) {
        (Some(g), Some(rec)) => format!(
            "recorded for guest {} {:#x}/{:#x}{}: {}; the compute mode is bound on its host TSG at the build (step 1), graphics stays at the host default (step 2)",
            if channels.tsg(g).is_some() { "TSG" } else { "bare channel" },
            g.0,
            g.1,
            if g.1 == r.h_channel { String::new() } else { format!(" (the TSG of channel {:#x})", r.h_channel) },
            rec.describe()
        ),
        _ => format!(
            "hChannel names no TSG or channel of client {h_client:#x}: not recorded"
        ),
    };
    let note = format!("GR_SET_CTXSW_PREEMPTION_MODE {{{}}}: {whom}", r.describe());
    if new_shape {
        eprintln!(
            "vfio-user-nvgpu: preemption: guest SET of a new shape (client {h_client:#x}): {note} (every SET is counted in the exit summary)"
        );
    }
    (
        answer(
            params.to_vec(),
            "guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs)",
            note,
        ),
        guest,
    )
}

fn get32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
}

/// `CE_GET_LCE_SHIM_INFO` (v610.rs `CMD_CE_GET_LCE_SHIM_INFO`), asked by the 615 libcuda
/// at cuInit for every CE; refusing it makes cuInit fail with 100 (no CUDA-capable device;
/// caraxes/runs/cutover-615-2026-09-24/v2-615guest/gate-cuda.txt:1-3). The shim layout is
/// the silicon's: forwarded to the host's subdevice when the host driver has the control
/// NON_PRIVILEGED (615, `HostLayouts::lce_shim_info_public`), answered from the chip's
/// shim table (`[ce] lce_shim`) otherwise. The note decodes the answer and names the
/// chip table's answer beside a host one, so a disagreement shows in the device log.
fn lce_shim_info(
    chip: &'static ChipData,
    params: &[u8],
    host: Option<&mut Box<dyn HostRm>>,
    layouts: &crate::hostabi::HostLayouts,
) -> Answer {
    let ce = get32(params, v::LCE_SHIM_CE_ENGINE_TYPE);
    let lce = ce.and_then(v::copy_engine_index);
    let table = chip.ce.lce_shim.as_ref();
    let from_table = lce.zip(table).map(|(i, t)| t.of(i));
    let what = format!(
        "ce lce shim info: ceEngineType {} (LCE {})",
        ce.map_or("missing".into(), |c| format!("{c:#x}")),
        lce.map_or("none".into(), |i| i.to_string())
    );
    if params.len() < v::LCE_SHIM_INFO_SIZE {
        return Answer {
            status: v::NV_ERR_INVALID_ARGUMENT,
            params: params.to_vec(),
            label: "synthesized (params shorter than NV2080_CTRL_CE_GET_LCE_SHIM_INFO_PARAMS)",
            note: format!("{what}: {} bytes, expected {}", params.len(), v::LCE_SHIM_INFO_SIZE),
        };
    }
    if let (Some(host), true) = (host, layouts.lce_shim_info_public) {
        let mut b = params.to_vec();
        let status = host.control(v::NV20_SUBDEVICE_0, v::CMD_CE_GET_LCE_SHIM_INFO, &mut b);
        let got = (
            get32(&b, v::LCE_SHIM_INSTANCE).unwrap_or(0),
            get32(&b, v::LCE_SHIM_LOCAL_LCE_IDX).unwrap_or(0),
        );
        let agrees = match from_table {
            Some(t) if status == NV_OK && t != got => format!("; chip table says {t:?}: DISAGREES"),
            Some(_) if status == NV_OK => "; chip table agrees".into(),
            _ => String::new(),
        };
        return Answer {
            status,
            params: b,
            label: "host (forwarded as-is; NON_PRIVILEGED on the 615 host)",
            note: format!(
                "{what} -> host status {status:#x}, shimInstance {} shimLocalLceIdx {}{agrees}",
                got.0, got.1
            ),
        };
    }
    match (lce, table) {
        (Some(_), Some(t)) => {
            let (shim, local) = from_table.unwrap();
            let mut b = params.to_vec();
            b[v::LCE_SHIM_INSTANCE..][..4].copy_from_slice(&shim.to_le_bytes());
            b[v::LCE_SHIM_LOCAL_LCE_IDX..][..4].copy_from_slice(&local.to_le_bytes());
            answer(
                b,
                &t.label,
                format!("{what} -> shimInstance {shim} shimLocalLceIdx {local}"),
            )
        }
        (None, _) => Answer {
            status: v::NV_ERR_INVALID_ARGUMENT,
            params: params.to_vec(),
            label: "synthesized (not a copy engine type)",
            note: format!("{what}: not a copy engine"),
        },
        (Some(_), None) => Answer {
            status: v::NV_ERR_NOT_SUPPORTED,
            params: params.to_vec(),
            label: "synthesized (no shim table for this chip and no host that may answer)",
            note: format!("{what}: not supported"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skyline_is_empty_without_mig() {
        let f = crate::fixture::rtx2070();
        let mut p = vec![0xAAu8; v::SKYLINE_INFO_SIZE];
        p[0] = 1;
        let mut reg = Registry::new(f.chip.data().channel);
        let a = answer_control(
            f.chip.data(),
            v::CMD_INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO,
            &p,
            &mut reg,
            None,
        )
        .unwrap();
        assert_eq!(a.status, NV_OK);
        assert_eq!(a.params.len(), v::SKYLINE_INFO_SIZE);
        assert_eq!(v::SKYLINE_INFO_SIZE, 724);
        assert!(a.params.iter().all(|&b| b == 0));
        assert_eq!(v::SKYLINE_VALID_ENTRIES, 720);
        assert!(answer_control(f.chip.data(), 0x2080_0BAD, &p, &mut reg, None).is_none());
    }

    #[test]
    fn promote_ctx_is_recorded_and_merged() {
        let f = crate::fixture::rtx2070();
        let mut reg = Registry::new(f.chip.data().channel);
        let entry = |p: &mut [u8], i: usize, pa: u64, va: u64, id: u16, init: bool| {
            let o = v::PROMOTE_ENTRIES + v::PROMOTE_ENTRY_SIZE * i;
            p[o..o + 8].copy_from_slice(&pa.to_le_bytes());
            p[o + 8..o + 16].copy_from_slice(&va.to_le_bytes());
            p[o + 16..o + 24].copy_from_slice(&0x1000u64.to_le_bytes());
            p[o + 24..o + 28].copy_from_slice(&1u32.to_le_bytes());
            p[o + 28..o + 30].copy_from_slice(&id.to_le_bytes());
            p[o + 30] = init as u8;
        };
        let mut p = vec![0u8; v::PROMOTE_PARAMS_SIZE];
        p[v::PROMOTE_H_CHAN_CLIENT..][..4].copy_from_slice(&7u32.to_le_bytes());
        p[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&0x40u32.to_le_bytes());
        p[v::PROMOTE_ENTRY_COUNT..][..4].copy_from_slice(&2u32.to_le_bytes());
        entry(&mut p, 0, 0xe900_0000, 0, 0, true);
        entry(&mut p, 1, 0xe901_0000, 0, 2, true);
        let a = answer_control(f.chip.data(), v::CMD_GPU_PROMOTE_CTX, &p, &mut reg, None).unwrap();
        assert_eq!((a.status, a.params.len()), (NV_OK, 560));
        assert_eq!(a.params, p);
        // UVM's bind: VA only for buffer 0.
        let mut q = p.clone();
        q[v::PROMOTE_ENTRY_COUNT..][..4].copy_from_slice(&1u32.to_le_bytes());
        entry(&mut q, 0, 0, 0x2_0000_0000, 0, false);
        answer_control(f.chip.data(), v::CMD_GPU_PROMOTE_CTX, &q, &mut reg, None).unwrap();
        let m = reg.promoted((7, 0x40)).unwrap();
        assert_eq!(m[&0].phys, Some((0xe900_0000, 1)));
        assert_eq!(m[&0].virt, Some(0x2_0000_0000));
        assert!(m[&0].initialize);
        assert_eq!(m[&2].virt, None);
        reg.record_free(7, 7);
        assert!(reg.promoted((7, 0x40)).is_none());
        // The acknowledged rows echo with NV_OK.
        for (cmd, _) in ACKNOWLEDGED {
            let a = answer_control(f.chip.data(), *cmd, &[1, 2, 3, 4], &mut reg, None).unwrap();
            assert_eq!(
                (a.status, a.params.as_slice()),
                (NV_OK, &[1u8, 2, 3, 4][..])
            );
        }
    }

    /// 0x20800A46 is the Bug 4208224 WAR for a 570.144 guest only (610 reused the id for
    /// CHECK_CTS_ID_VALID, which the D rules refuse).
    #[test]
    fn the_570_war_id_is_acknowledged_for_a_570_guest_only() {
        use crate::ver::{GuestAbi, TestGuest};
        let f = crate::fixture::rtx2070();
        let mut reg = Registry::new(f.chip.data().channel);
        let war = crate::ver::v570::CMD_INTERNAL_KGR_INIT_BUG4208224_WAR;
        {
            let _g = TestGuest::set(GuestAbi::V570);
            let a = answer_control(f.chip.data(), war, &[0], &mut reg, None).unwrap();
            assert_eq!((a.status, a.params.as_slice()), (NV_OK, &[0u8][..]));
            assert!(a.note.contains("570.144"), "{}", a.note);
        }
        for g in [GuestAbi::V610, GuestAbi::V615] {
            let _g = TestGuest::set(g);
            let a = answer_control(f.chip.data(), war, &[0], &mut reg, None);
            assert!(a.is_none_or(|a| !a.note.contains("WAR")), "{g:?}: not the WAR");
        }
    }

    /// `NV2080_CTRL_CE_GET_LCE_SHIM_INFO_PARAMS` is three NvU32 in both drivers
    /// (ctrl2080ce.h:370-374 in 610.57.04 and 615.71.09); the 615 libcuda sends 12 bytes.
    /// The ECC status the same libcuda asks is forwarded unchanged (`FORWARD`), which is
    /// sound on either host because its struct is the same size in both bindings.
    #[test]
    fn lce_shim_info_and_ecc_layouts() {
        use core::mem::size_of;
        assert_eq!(v::CMD_CE_GET_LCE_SHIM_INFO, 0x2080_2A0C);
        assert_eq!(v::LCE_SHIM_INFO_SIZE, 3 * 4);
        assert_eq!(
            (v::LCE_SHIM_CE_ENGINE_TYPE, v::LCE_SHIM_INSTANCE, v::LCE_SHIM_LOCAL_LCE_IDX),
            (0, 4, 8)
        );
        assert_eq!(
            size_of::<nvrm_sys::v610::NV2080_CTRL_GPU_QUERY_ECC_STATUS_PARAMS>(),
            size_of::<nvrm_sys::v615::NV2080_CTRL_GPU_QUERY_ECC_STATUS_PARAMS>()
        );
        assert!(crate::ver::v610::FORWARD.iter().any(|(c, _)| *c == 0x2080_012F));
        assert_eq!(v::copy_engine_index(0x09), Some(0));
        assert_eq!(v::copy_engine_index(0x12), Some(9));
        assert_eq!(v::copy_engine_index(0x34), Some(10));
        assert_eq!(v::copy_engine_index(0x3D), Some(19));
        assert_eq!(v::copy_engine_index(0x01), None);
        use crate::hostabi::{HOST_610, HOST_615};
        assert!(!HOST_610.lce_shim_info_public && HOST_615.lce_shim_info_public);
    }

    /// The guest's boost: NV_OK and the params echoed; the note names the request (no
    /// host boost runs in the unit tests, so nothing is boosted).
    #[test]
    fn perf_boost_2x_is_acknowledged_and_noted() {
        let f = crate::fixture::rtx2070();
        let mut reg = Registry::new(f.chip.data().channel);
        let mut p = vec![0u8; v::PERF_BOOST_2X_SIZE];
        p[0] = 0x12;
        p[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let a = answer_control(f.chip.data(), v::CMD_INTERNAL_PERF_BOOST_SET_2X, &p, &mut reg, None)
            .unwrap();
        assert_eq!((a.status, a.params.as_slice()), (NV_OK, p.as_slice()));
        assert!(a.label.contains("host boost"), "{}", a.label);
        assert!(
            a.note.starts_with("PERF_BOOST_SET_2X {flags 0x12, duration infinite}"),
            "{}",
            a.note
        );
        assert!(!ACKNOWLEDGED.iter().any(|(c, _)| *c == v::CMD_INTERNAL_PERF_BOOST_SET_2X));
    }

    /// The preemption SET is decoded and recorded (preempt.rs), no longer a blind echo; the
    /// answer stays NV_OK with the params echoed, as native.
    #[test]
    fn the_preemption_set_is_decoded_not_blindly_acknowledged() {
        assert!(!ACKNOWLEDGED.iter().any(|(c, _)| *c == v::CMD_GR_SET_CTXSW_PREEMPTION_MODE));
        assert_eq!(v::CMD_GR_SET_CTXSW_PREEMPTION_MODE, crate::preempt::CMD_SET_CTXSW_PREEMPTION_MODE);
        let f = crate::fixture::rtx2070();
        let mut reg = Registry::new(f.chip.data().channel);
        let mut p = vec![0u8; 32];
        p[0] = 1;
        p[4..8].copy_from_slice(&0x5c00_0012u32.to_le_bytes());
        p[12] = 2;
        let (a, g) = set_ctxsw_preemption_mode(0xc1d0_0001, &p, &mut reg);
        assert_eq!((a.status, a.params.as_slice(), g), (NV_OK, p.as_slice(), None));
        assert!(
            a.note.starts_with("GR_SET_CTXSW_PREEMPTION_MODE {flags 0x1 (CILP_SET), hChannel 0x5c000012, graphics not set (field 0), compute CILP (2)}"),
            "{}",
            a.note
        );
    }

    /// Forwarded on a 615 host, the chip's shim table on a 610 host (the control is
    /// PRIVILEGED there) or without a host; a non-copy engine is an invalid argument.
    #[test]
    fn lce_shim_info_answer() {
        use crate::hostabi::{HOST_610, HOST_615};
        type Seen = std::sync::Arc<std::sync::Mutex<Vec<(u32, u32, usize)>>>;
        struct Rec(Seen);
        impl HostRm for Rec {
            fn control(&mut self, class: u32, cmd: u32, p: &mut [u8]) -> u32 {
                self.0.lock().unwrap().push((class, cmd, p.len()));
                let lce = u32::from_le_bytes(p[0..4].try_into().unwrap()) - 0x09;
                p[4..8].copy_from_slice(&0u32.to_le_bytes());
                p[8..12].copy_from_slice(&lce.to_le_bytes());
                NV_OK
            }
        }
        let f = crate::fixture::rtx2070();
        let chip = f.chip.data();
        let req = |engine: u32| {
            let mut p = vec![0xEEu8; v::LCE_SHIM_INFO_SIZE];
            p[0..4].copy_from_slice(&engine.to_le_bytes());
            p
        };
        let out = |a: &Answer| (a.status, get32(&a.params, 4), get32(&a.params, 8));
        let seen: Seen = Default::default();
        let mut host: Box<dyn HostRm> = Box::new(Rec(seen.clone()));
        // 615 host: forwarded to the subdevice, the chip table agrees.
        let a = lce_shim_info(chip, &req(0x09 + 3), Some(&mut host), &HOST_615);
        assert_eq!(out(&a), (NV_OK, Some(0), Some(3)));
        assert!(a.label.starts_with("host"), "{}", a.label);
        assert!(a.note.ends_with("chip table agrees"), "{}", a.note);
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &[(v::NV20_SUBDEVICE_0, 0x2080_2A0C, 12)]
        );
        // 610 host: not sent; the chip table (one shim on Turing).
        let a = lce_shim_info(chip, &req(0x09 + 2), Some(&mut host), &HOST_610);
        assert_eq!(out(&a), (NV_OK, Some(0), Some(2)));
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert!(a.label.contains("one CE shim"), "{}", a.label);
        // No host, COPY10.
        let a = lce_shim_info(chip, &req(0x34), None, &HOST_615);
        assert_eq!(out(&a), (NV_OK, Some(0), Some(10)));
        // Not a copy engine; too short.
        assert_eq!(lce_shim_info(chip, &req(0x01), None, &HOST_615).status, v::NV_ERR_INVALID_ARGUMENT);
        assert_eq!(lce_shim_info(chip, &[9, 0, 0, 0], None, &HOST_615).status, v::NV_ERR_INVALID_ARGUMENT);
        // Through the dispatcher's CUDA rows.
        let mut reg = Registry::new(chip.channel);
        let a = answer_control(chip, v::CMD_CE_GET_LCE_SHIM_INFO, &req(0x0A), &mut reg, None).unwrap();
        assert_eq!(out(&a), (NV_OK, Some(0), Some(1)));
    }
}

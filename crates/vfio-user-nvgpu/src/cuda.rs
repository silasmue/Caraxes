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
use crate::ver::active::{self as v, NV_OK};

fn answer(params: Vec<u8>, label: &'static str, note: String) -> Answer {
    Answer {
        status: NV_OK,
        params,
        label,
        note,
    }
}

/// The controls of the context path that real GSP acts on (clocks, exception masks,
/// preemption modes, timeslices, preemption, channel stop, eviction) and that the device
/// acknowledges without effect, because no host channel exists yet (part 2 maps them to
/// the host objects; cuda-lookahead.md §6.3). Native status for each is NV_OK
/// (matrix/traces/610.57.04/cuda-core.tsv and cuda-launch.tsv).
const ACKNOWLEDGED: &[(u32, &str)] = &[
    (
        v::CMD_INTERNAL_PERF_BOOST_SET_2X,
        "PERF_BOOST_SET_2X (no clocks boosted)",
    ),
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
    (
        v::CMD_GR_SET_CTXSW_PREEMPTION_MODE,
        "GR_SET_CTXSW_PREEMPTION_MODE (no context switch)",
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
    _host: Option<&mut Box<dyn HostRm>>,
) -> Option<Answer> {
    let n = params.len();
    if let Some((_, what)) = ACKNOWLEDGED.iter().find(|(c, _)| *c == cmd) {
        return Some(answer(
            params.to_vec(),
            "synthesized (NV_OK echo, nothing acted on: no host channel before part 2)",
            format!("{what}: ok"),
        ));
    }
    Some(match cmd {
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
        _ => return None,
    })
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
}

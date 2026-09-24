// SPDX-License-Identifier: MIT
//! Physical-RM controls that only GA100-and-later kernels send and the Turing dispatcher
//! (ctrl.rs) therefore never needed. Answered from the chip data, ahead of the generic
//! dispatcher (boot.rs asks here first).
//!
//! `NV2080_CTRL_CMD_INTERNAL_GET_DEVICE_INFO_TABLE` (0x20800A40,
//! src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080internal.h:747-775): the parsed
//! DEVICE_INFO2 table of the physical side. The kernel asks for it in
//! `gpuConstructDeviceInfoTable_FWCLIENT` (src/nvidia/src/kernel/gpu/gpu_gspclient.c:224-280),
//! the HAL for GA100 and later (generated/g_gpu_nvoc.c:864-881), from
//! `kfifoConstructEngineList_KERNEL` (src/kernel/gpu/fifo/kernel_fifo.c:2199-2202): a refusal
//! leaves the FIFO without an engine list, and the GB206 guest then dereferences a
//! missing KernelGraphics in memmgrCalcReservedFbSpaceHal_GM107 (N gate row b1).

use crate::chip::ChipData;
use crate::ctrl::Answer;

pub const CMD_INTERNAL_GET_DEVICE_INFO_TABLE: u32 = 0x2080_0A40;
/// `NV2080_CTRL_INTERNAL_GET_DEVICE_INFO_TABLE_PARAMS`: numEntries, then 512 entries of
/// twelve NvU32 (ctrl2080internal.h:751-775).
const ENTRY_WORDS: usize = 12;
const MAX_ENTRIES: usize = 512;
pub const PARAMS_SIZE: usize = 4 + MAX_ENTRIES * ENTRY_WORDS * 4;

// DEVICE_INFO2 type enums: GRAPHICS 0 (the DEV_TYPE_ENUM default of every family),
// LCE 0x13 (published/blackwell/gb202/dev_top_zb.h), TMR 0x1f, PBUS 0x33, HUBMMU 0x35
// (published/blackwell/gb100/dev_top_zb.h:27-29).
const TYPE_GRAPHICS: u32 = 0x00;
const TYPE_LCE: u32 = 0x13;
const TYPE_TMR: u32 = 0x1F;
const TYPE_PBUS: u32 = 0x33;
const TYPE_HUBMMU: u32 = 0x35;

/// One `NV2080_CTRL_INTERNAL_DEVICE_INFO` in field order: faultId, instanceId, typeEnum,
/// resetId, devicePriBase, isEngine, rlEngId, runlistPriBase, groupId, ginTargetId,
/// deviceBroadcastPriBase, groupLocalInstanceId.
type Entry = [u32; ENTRY_WORDS];

/// The table for `chip`: its GR and copy engines from the device-info table (fault id,
/// instance), and the non-engine units whose PRI base the GB20x HALs look up:
/// PTIMER 0x9000 (tmrGetTmrBaseAddr_GB100, timer_gb100.c:44-56; NV_PTIMER0_PRI_BASE,
/// published/blackwell/gb100/hwproject.h:38), PBUS 0x1000 (kfspFrtsSysmemLocation*_GB100,
/// kern_fsp_gb100.c:421-470; NV_PBUS0_PRI_BASE, hwproject.h:35) and HUBMMU 0x880000
/// (kern_bus_gb100.c:243-262; NV_HUBMMU0_PRI_BASE, hwproject.h:36).
///
/// Columns left 0, and why (checked against the 610.57.04 source, 2026-09-24):
/// - `rlEngId`, `runlistPriBase`, `ginTargetId`, `resetId`, `deviceBroadcastPriBase`: the
///   GB206 kernel never reads them from this table; `gpuGetOneDeviceEntry` matches only
///   `typeEnum`, `groupId` and `groupLocalInstanceId` (src/nvidia/src/kernel/gpu/gpu.c:7779-7845),
///   and the rest is only copied for vGPU (src/kernel/vgpu/rpcstructurecopy.c:3725-3818).
///   The runlist and PBDMA columns the kernel uses come from `FIFO_GET_DEVICE_INFO_TABLE`
///   (ctrl.rs, from `chips/<chip>/device-info.toml`).
/// - LCE `devicePriBase`: the source has no value for it (it is hardware DEVICE_INFO2 data,
///   not in any published header); the one reader is `kceGetGrceMaskReg_GB202`, which reads
///   `devicePriBase + NV_CE_BASE_GRCE_MASK` 0xD8 (kernel_ce_gb202.c:302-323,
///   published/blackwell/gb202/dev_ce_base.h:32-34). With base 0 that is BAR0 0xD8, which
///   reads 0 (no GRCE in the mask); the N gate b2 trace shows those six reads.
pub fn device_info2(chip: &ChipData) -> Vec<Entry> {
    let mut t: Vec<Entry> = Vec::new();
    for e in &chip.device_info.engines {
        let ty = match e.dev_type {
            0 if e.name.starts_with("gr") => TYPE_GRAPHICS,
            19 => TYPE_LCE,
            _ => continue,
        };
        t.push([
            e.mmu_fault_id,
            e.instance,
            ty,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            e.instance,
        ]);
    }
    for (ty, base) in [
        (TYPE_TMR, 0x9000),
        (TYPE_PBUS, 0x1000),
        (TYPE_HUBMMU, 0x88_0000),
    ] {
        t.push([0, 0, ty, 0, base, 0, 0, 0, 0, 0, 0, 0]);
    }
    t
}

/// `NV2080_CTRL_CMD_CE_GET_PHYSICAL_CAPS` (ctrl2080ce.h:276-287): `ceEngineType`
/// (NV2080_ENGINE_TYPE) and `capsTbl[2]` (NV2080_CTRL_CE_CAPS_TBL_SIZE, :68).
pub const CMD_CE_GET_PHYSICAL_CAPS: u32 = 0x2080_2A07;

/// The GB206 copy-engine caps of the lab capture, a chip fact (CE_GET_ALL_CAPS,
/// caraxes/reference/gb206/rm-tools/host-facts.txt:33-36): `(NV2080_ENGINE_TYPE, caps0,
/// caps1)` for CE0 (GRCE, SHARED), CE4 and CE7 (SHARED). Without them the pure-emulation
/// device answers NOT_SUPPORTED, and UVM's channel manager fails in queryCopyEngines
/// (kernel_ce.c:564; N gate row b5).
const GB206_CE_CAPS: [(u32, u8, u8); 3] =
    [(0x09, 0xE3, 0x03), (0x0D, 0xE4, 0x03), (0x10, 0xEA, 0x03)];

fn ce_physical_caps(chip: &ChipData, params: &[u8]) -> Option<Answer> {
    if chip.name != "gb206" || params.len() < 6 {
        return None;
    }
    let ty = u32::from_le_bytes(params[0..4].try_into().unwrap());
    let &(_, c0, c1) = GB206_CE_CAPS.iter().find(|c| c.0 == ty)?;
    let mut out = params.to_vec();
    out[4] = c0;
    out[5] = c1;
    Some(Answer {
        status: 0,
        params: out,
        label: "host (GB206 CE_GET_ALL_CAPS of the lab capture, rm-tools/host-facts.txt:33-36)",
        note: format!("ce physical caps for engine type {ty:#x}: {c0:#04x} {c1:#04x}"),
    })
}

/// `NV2080_CTRL_CMD_CE_GET_CE_PCE_MASK` (ctrl2080ce.h:154-178): `ceEngineType`, `pceMask`.
pub const CMD_CE_GET_CE_PCE_MASK: u32 = 0x2080_2A02;

/// The GB206 PCE masks of the lab capture (rm-tools/fifo-host-query.txt:6-8): CE0 0x2,
/// CE4 0x1, CE7 0x2 (N gate row b6: UVM's queryCopyEngines asks for them).
const GB206_PCE_MASKS: [(u32, u32); 3] = [(0x09, 0x2), (0x0D, 0x1), (0x10, 0x2)];

fn ce_pce_mask(chip: &ChipData, params: &[u8]) -> Option<Answer> {
    if chip.name != "gb206" || params.len() < 8 {
        return None;
    }
    let ty = u32::from_le_bytes(params[0..4].try_into().unwrap());
    let &(_, mask) = GB206_PCE_MASKS.iter().find(|c| c.0 == ty)?;
    let mut out = params.to_vec();
    out[4..8].copy_from_slice(&mask.to_le_bytes());
    Some(Answer {
        status: 0,
        params: out,
        label:
            "host (GB206 CE_GET_CE_PCE_MASK of the lab capture, rm-tools/fifo-host-query.txt:6-8)",
        note: format!("ce pce mask for engine type {ty:#x}: {mask:#x}"),
    })
}

/// GR info for a GB206 without a host: `NV2080_CTRL_INTERNAL_STATIC_GR_GET_INFO_PARAMS`,
/// engine 0, `{index, data}` pairs for the 63 `NV0080_CTRL_GR_INFO_INDEX_*`
/// (ctrl0080gr.h:102-160; the layout kgr.rs fills from the host on Turing). Without it
/// `pGrInfo` stays NULL and `kfifoGetMaxSubcontextFromGr_KERNEL` returns 0
/// (kernel_fifo.c:2760-2775), so the first channel group fails `numMax != 0`
/// (kernel_channel_group_api.c:914; N gate row b7). Values: MAX_SUBCONTEXT_COUNT 64
/// (the host's FIFO_GET_INFO, rm-tools/fifo-host-query.txt:22), LITTER_NUM_GPCS 3 and
/// LITTER_NUM_TPC_PER_GPC 6 (host-facts.txt:28-32), SM_PER_TPC 2 and SM_VERSION 0x1200
/// (compute capability 12.0, gpu-0000_01_00.0/nvidia-smi-query-gpu.txt:67); the rest 0.
fn gr_info(chip: &ChipData, params: &[u8]) -> Option<Answer> {
    use crate::ver::active as v;
    if chip.name != "gb206" || params.len() < v::INFO_PARAMS {
        return None;
    }
    let set: [(u32, u32); 5] = [(0x2C, 64), (0x14, 3), (0x17, 6), (0x20, 2), (0x0C, 0x1200)];
    let mut out = vec![0u8; v::INFO_PARAMS];
    for i in 0..v::GR_INFO_COUNT {
        let d = set.iter().find(|e| e.0 as usize == i).map_or(0, |e| e.1);
        out[8 * i..8 * i + 4].copy_from_slice(&(i as u32).to_le_bytes());
        out[8 * i + 4..8 * i + 8].copy_from_slice(&d.to_le_bytes());
    }
    Some(Answer {
        status: 0,
        params: out,
        label: "host (GB206 capture: subcontexts, GPC/TPC counts, CC 12.0) + synthesized (rest 0)",
        note: "kgr info: 63 entries, GB206 capture values".into(),
    })
}

/// The answer to `cmd`, or `None` to leave it to the generic dispatcher.
pub fn answer(chip: &ChipData, cmd: u32, params: &[u8]) -> Option<Answer> {
    if cmd == CMD_CE_GET_PHYSICAL_CAPS {
        return ce_physical_caps(chip, params);
    }
    if cmd == CMD_CE_GET_CE_PCE_MASK {
        return ce_pce_mask(chip, params);
    }
    if cmd == crate::ver::active::CMD_INTERNAL_STATIC_KGR_GET_INFO {
        return gr_info(chip, params);
    }
    if cmd != CMD_INTERNAL_GET_DEVICE_INFO_TABLE || params.len() < PARAMS_SIZE {
        return None;
    }
    let t = device_info2(chip);
    let mut out = vec![0u8; PARAMS_SIZE];
    out[0..4].copy_from_slice(&(t.len() as u32).to_le_bytes());
    for (i, e) in t.iter().enumerate() {
        for (k, w) in e.iter().enumerate() {
            let at = 4 + (i * ENTRY_WORDS + k) * 4;
            out[at..at + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
    Some(Answer {
        status: 0,
        params: out,
        label: "synthesized (DEVICE_INFO2 from chips/<chip>/device-info.toml + TMR/PBUS/HUBMMU PRI bases)",
        note: format!("internal device info table: {} entries", t.len()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gb206_device_info2() {
        let c = crate::chip::by_name("gb206").unwrap().data();
        let a = answer(
            c,
            CMD_INTERNAL_GET_DEVICE_INFO_TABLE,
            &vec![0u8; PARAMS_SIZE],
        )
        .unwrap();
        assert_eq!(a.params.len(), 24580);
        let w = |i: usize| u32::from_le_bytes(a.params[4 * i..4 * i + 4].try_into().unwrap());
        // GR0 + CE0/4/7 + TMR, PBUS, HUBMMU.
        assert_eq!(w(0), 7);
        assert_eq!((w(1), w(2), w(3)), (384, 0, TYPE_GRAPHICS));
        assert_eq!((w(1 + 12), w(2 + 12), w(3 + 12)), (65, 0, TYPE_LCE));
        let tmr = 1 + 4 * 12;
        assert_eq!((w(tmr + 2), w(tmr + 4)), (TYPE_TMR, 0x9000));
        assert!(answer(c, 0x2080_0A41, &[]).is_none());
        let mut p = vec![0u8; 8];
        p[0] = 0x10; // COPY7
        let a = answer(c, CMD_CE_GET_PHYSICAL_CAPS, &p).unwrap();
        assert_eq!(&a.params[4..6], &[0xEA, 0x03]);
        let m = answer(c, CMD_CE_GET_CE_PCE_MASK, &p).unwrap();
        assert_eq!(&m.params[4..8], &2u32.to_le_bytes());
        p[0] = 0x0A; // COPY1: not on GB206
        assert!(answer(c, CMD_CE_GET_PHYSICAL_CAPS, &p).is_none());
        let tu = crate::chip::by_name("tu106").unwrap().data();
        assert!(answer(tu, CMD_CE_GET_PHYSICAL_CAPS, &[9, 0, 0, 0, 0, 0, 0, 0]).is_none());
        assert!(answer(c, CMD_INTERNAL_GET_DEVICE_INFO_TABLE, &[0u8; 8]).is_none());
    }
}

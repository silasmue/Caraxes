// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The public controls of the GSP plan (docs/caraxes/gsp-plan/b-host-forwardable.md and
//! c-guest-state.md) that are not a plain forward: forwards whose answer is filtered
//! before it reaches the guest, answers the device owns, controls on the guest's own
//! channels and TSGs, and refusals. One row per control, the handler is `crate::public`.
//! Plain forwards stay in the `FORWARD` list of the version module (nvml.rs).
//!
//! Every parameter struct named here has the same `sizeof` and field offsets in
//! 610.57.04 and 615.71.09 (measured with `cc -std=gnu11 -DNV_LINUX` against each tree's
//! SDK headers, 2026-09-24; b-host-forwardable.md §2 found the same for all 70 B rows),
//! so the 615 module takes this one unchanged. Where `nvrm-sys` binds a struct, the
//! tests pin the numbers to the binding.

/// How the device answers one control of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// Forward to the host subdevice, then filter the answer before the guest sees it.
    Filtered(Filter),
    /// Answered from the device's own model; never forwarded.
    Device(Dev),
    /// NOT_SUPPORTED with the label, request echoed; never forwarded.
    Refuse(&'static str),
}

/// Answers the device owns (b-host-forwardable.md §3 S, and the decisions recorded there).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dev {
    /// `GSP_GET_RM_HEAP_STATS`: the device's own GSP heap view (decision under D9, Q-B6):
    /// the emulated GSP has no RM heap, so every number is 0; the host GSP is not asked.
    RmHeapStats,
    /// `TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO`: samples of the device's own clock (the
    /// emulated GSP's OS time and the device PTIMER are one time base).
    TimeCorrelation,
    /// `BIOS_GET_NBSI_V2`: the synthesized VBIOS has no NBSI: `errorCode` NOT_FOUND.
    NbsiNotFound,
    /// `I2C_GET_PORT_SPEED`: no I2C ports (as `I2C_GET_PORT_INFO`, ctrl.rs): all 0.
    I2cNoPorts,
    /// `GET_RC_RECOVERY`: the device's policy, recovery enabled.
    RcRecoveryEnabled,
    /// `FB_GET_CTAGS_FOR_CBC_EVICTION`: no comptags (D16: no compression reaches the host).
    NoCompTags,
    /// `ECC_GET_SRAM_UNIQUE_UNCORR_COUNTS`: this board has no ECC; NOT_SUPPORTED as the
    /// host answers the other ECC getters (Q-B1).
    NoEcc,
}

/// The output filters of forwarded controls (b-host-forwardable.md §3 F2 and §6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    /// `FB_GET_OFFLINED_PAGES`: keep the entries whose pages lie in the reservation.
    OfflinedPages,
    /// `FB_GET_REMAPPED_ROWS`: DRAM rows cannot be placed in the reservation; no entry
    /// reaches the guest, the card-level flags do.
    RemappedRows,
    /// `GPU_QUERY_SCRUBBER_STATUS`: the scrub range clamped to the reservation (0 and 0
    /// when they do not meet).
    ScrubberRange,
    /// `FB_GET_DYNAMIC_OFFLINED_PAGES`: keep the entries whose pages lie in the
    /// reservation, recount `validEntries`.
    DynamicOfflinedPages,
    /// `PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2`: zero the host process fields of every
    /// engine sample, and the utilization of engines the device does not present.
    GpumonProcess,
}

/// One control of the table.
#[derive(Clone, Copy, Debug)]
pub struct PublicControl {
    pub id: u32,
    pub name: &'static str,
    /// `sizeof` of the params struct; 0 where the rule does not read the params.
    pub size: usize,
    pub rule: Rule,
}

const fn row(id: u32, name: &'static str, size: usize, rule: Rule) -> PublicControl {
    PublicControl { id, name, size, rule }
}

use Filter as F;

pub const PUBLIC: &[PublicControl] = &[
    // Forwarded as-is until 2026-09-24 (the FORWARD list): the host answer carries host FB
    // page numbers, possibly outside the reservation (b-host-forwardable.md §6).
    PublicControl {
        id: 0x2080_1322,
        name: "FB_GET_OFFLINED_PAGES",
        size: OFFLINED_PAGES_SIZE,
        rule: Rule::Filtered(Filter::OfflinedPages),
    },
    PublicControl {
        id: 0x2080_1344,
        name: "FB_GET_REMAPPED_ROWS",
        size: REMAPPED_ROWS_SIZE,
        rule: Rule::Filtered(Filter::RemappedRows),
    },
    // Group F2 of b-host-forwardable.md: forwards with an output filter.
    row(0x2080_015F, "GPU_QUERY_SCRUBBER_STATUS", SCRUBBER_SIZE, Rule::Filtered(F::ScrubberRange)),
    row(0x2080_1348, "FB_GET_DYNAMIC_OFFLINED_PAGES", DYN_OFFLINED_SIZE, Rule::Filtered(F::DynamicOfflinedPages)),
    row(0x2080_2096, "PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2", GPUMON_SIZE, Rule::Filtered(F::GpumonProcess)),
    // The fourth F2 row of the plan (gfid pinned to 0, then forwarded) became a device
    // answer: the guest's GSP is the device, not the host's (decision recorded in
    // b-host-forwardable.md, Q-B6).
    row(0x2080_3602, "GSP_GET_RM_HEAP_STATS", RM_HEAP_STATS_SIZE, Rule::Device(Dev::RmHeapStats)),
    // Group S: device-owned answers (the other five are chip data in chips/tu106/controls.toml).
    row(0x2080_0406, "TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO", TIME_CORR_SIZE, Rule::Device(Dev::TimeCorrelation)),
    row(0x2080_080E, "BIOS_GET_NBSI_V2", NBSI_V2_SIZE, Rule::Device(Dev::NbsiNotFound)),
    row(0x402C_0103, "I2C_GET_PORT_SPEED", 64, Rule::Device(Dev::I2cNoPorts)),
    row(0x2080_220E, "GET_RC_RECOVERY", 4, Rule::Device(Dev::RcRecoveryEnabled)),
    row(0x2080_1338, "FB_GET_CTAGS_FOR_CBC_EVICTION", CTAGS_SIZE, Rule::Device(Dev::NoCompTags)),
    row(0x2080_3402, "ECC_GET_SRAM_UNIQUE_UNCORR_COUNTS", 0, Rule::Device(Dev::NoEcc)),
    // Group R: refused (b-host-forwardable.md §3 R).
    row(0x2080_110E, "FIFO_OBJSCHED_SW_GET_LOG", 0, Rule::Refuse(REFUSED_SCHED_LOG)),
    row(0x2080_1827, "BUS_GET_UPHY_DLN_CFG_SPACE", 0, Rule::Refuse("refused (raw PHY register read)")),
    row(0x2080_1828, "BUS_GET_EOM_STATUS", 0, Rule::Refuse("refused (eye measurement: acts on the physical link)")),
    row(0x208F_1101, "NV208F GPU_GET_RAM_SVOP_VALUES", 0, Rule::Refuse("refused (diag class, not opened on the host)")),
    row(0xB0CC_0109, "NVB0CC PMA_STREAM_UPDATE_GET_PUT", 0, Rule::Refuse(REFUSED_PROFILER)),
    row(0xB0CC_010D, "NVB0CC GET_TOTAL_HS_CREDITS", 0, Rule::Refuse(REFUSED_PROFILER)),
    row(0xB0CC_010F, "NVB0CC GET_HS_CREDITS", 0, Rule::Refuse(REFUSED_PROFILER)),
    row(0xB0CC_0115, "NVB0CC GET_CHIPLET_HS_CREDIT_POOL", 0, Rule::Refuse(REFUSED_PROFILER)),
    row(0xB0CC_0116, "NVB0CC GET_HS_CREDITS_MAPPING", 0, Rule::Refuse(REFUSED_PROFILER)),
    row(0xB0CC_0118, "NVB0CC GET_DYNAMIC_MMA_BOOST_STATUS", 0, Rule::Refuse(REFUSED_PROFILER)),
    row(0x2080_0181, "GPU_GET_PARTITION_CAPACITY", 0, Rule::Refuse("refused (MIG absent on this chip)")),
    row(0x2080_2069, "PERF_GET_TEGRA_PERFMON_SAMPLE", 0, Rule::Refuse("refused (Tegra only)")),
    row(0x2080_81F5, "GPU_GET_UNIT_FS_INFO_FROM_DIELET", 0, Rule::Refuse("refused (no dielets on this chip)")),
    // Group N: the guest kernel answers GPU accounting itself on a GSP client (SR-IOV
    // only sends them, gpu_registry.c:133); should one arrive, the host's accounting is
    // on the D9 deny list.
    row(0x0000_0B02, "NV0000 GPUACCT_GET_ACCOUNTING_STATE", 0, Rule::Refuse(REFUSED_ACCOUNTING)),
    row(0x0000_0B03, "NV0000 GPUACCT_GET_PROC_ACCOUNTING_INFO", 0, Rule::Refuse(REFUSED_ACCOUNTING)),
    row(0x0000_0B04, "NV0000 GPUACCT_GET_ACCOUNTING_PIDS", 0, Rule::Refuse(REFUSED_ACCOUNTING)),
    row(0x0000_0B06, "NV0000 GPUACCT_GET_PROC_ACCOUNTING_INFO_V2", 0, Rule::Refuse(REFUSED_ACCOUNTING)),
];

pub const REFUSED_PROFILER: &str = "refused (profiler: profiling is refused, gsp-surface.md §3 D rule 4)";
pub const REFUSED_SCHED_LOG: &str = "refused (host scheduler log: every tenant's scheduling)";
pub const REFUSED_ACCOUNTING: &str = "refused (host accounting: host PIDs, D9 deny list)";

/// The row of `id`, if the table has one.
pub fn public_control(id: u32) -> Option<&'static PublicControl> {
    PUBLIC.iter().find(|c| c.id == id)
}

// ---------------------------------------------------------------------------------
// Layouts (sdk/ctrl/ctrl2080/ctrl2080fb.h; line numbers of 615.71.09).

/// `NV2080_CTRL_FB_GET_OFFLINED_PAGES_PARAMS` (ctrl2080fb.h:1083-1088): `offlined[64]` of
/// `NV2080_CTRL_FB_OFFLINED_ADDRESS_INFO` (:999-1006: `pageAddressWithEccOn` u64 @0,
/// `pageAddressWithEccOff` u64 @8, `rbcAddress` @16, `source` @20, `status` @24,
/// `timestamp` @28; 32 bytes), `validEntries` @2048, `bRetirementPending` @2052,
/// `retirementPending` @2053; 2056 bytes. The addresses are 4 KiB page numbers
/// (mem_mgr_gm107.c:1711-1716 shifts them by `RM_PAGE_SHIFT`).
pub const OFFLINED_PAGES_SIZE: usize = 2056;
pub const OFFLINED_ENTRY_SIZE: usize = 32;
pub const OFFLINED_MAX: usize = 64;
pub const OFFLINED_ECC_ON: usize = 0;
pub const OFFLINED_ECC_OFF: usize = 8;
pub const OFFLINED_STATUS: usize = 24;
pub const OFFLINED_VALID_ENTRIES: usize = 2048;
pub const OFFLINED_B_RETIREMENT_PENDING: usize = 2052;
pub const OFFLINED_RETIREMENT_PENDING: usize = 2053;
/// `NV2080_CTRL_FB_OFFLINED_PAGES_INVALID_ADDRESS` (ctrl2080fb.h:960).
pub const OFFLINED_INVALID_ADDRESS: u64 = u64::MAX;
/// `NV2080_CTRL_FB_OFFLINED_PAGES_STATUS_PENDING_RETIREMENT` (ctrl2080fb.h:1016).
pub const OFFLINED_STATUS_PENDING_RETIREMENT: u32 = 1;
/// `RM_PAGE_SHIFT`: the page numbers count 4 KiB pages.
pub const RM_PAGE_SHIFT: u32 = 12;

/// `NV2080_CTRL_FB_GET_REMAPPED_ROWS_PARAMS` (ctrl2080fb.h:1973-1977): `entryCount` @0,
/// `flags` @4 (PENDING 0:0, FAILURE 1:1), `entries[512]` of 12-byte
/// `NV2080_CTRL_FB_ROW_REMAP_ENTRY` @8 (`remapRegVal`, `timestamp`, `fbpa`,
/// `sublocation`, `source`, `flags`: a DRAM row, no FB address); 6152 bytes.
pub const REMAPPED_ROWS_SIZE: usize = 6152;
pub const REMAPPED_ROWS_ENTRY_COUNT: usize = 0;
pub const REMAPPED_ROWS_FLAGS: usize = 4;
pub const REMAPPED_ROWS_ENTRIES: usize = 8;

/// `NV2080_CTRL_GPU_QUERY_SCRUBBER_STATUS_PARAMS` (ctrl2080gpu.h:2128-2133):
/// `scrubberStatus` @0, `remainingTimeMs` @4, `scrubStartAddr` u64 @8, `scrubEndAddr` u64
/// @16 (host FB addresses); 24 bytes.
pub const SCRUBBER_SIZE: usize = 24;
pub const SCRUBBER_START: usize = 8;
pub const SCRUBBER_END: usize = 16;

/// `NV2080_CTRL_FB_GET_DYNAMIC_OFFLINED_PAGES_PARAMS` (ctrl2080fb.h:2459-2464): `offlined[64]`
/// of 16-byte `{pageNumber u64 @0, source u8 @8}` (4 KiB pages, mem_mgr_ga100.c:416),
/// `validEntries` @1024, `baseIndex` @1028, `bMore` @1032; 1040 bytes.
pub const DYN_OFFLINED_SIZE: usize = 1040;
pub const DYN_OFFLINED_ENTRY_SIZE: usize = 16;
pub const DYN_OFFLINED_MAX: usize = 64;
pub const DYN_OFFLINED_VALID_ENTRIES: usize = 1024;

/// `NV2080_CTRL_PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2_PARAMS` (ctrl2080perf.h:562-600):
/// `type` @0, `bufSize` @4, `count` @8, `tracker` @12, `samples[72]` @16 of 776 bytes:
/// `base` (8), then the engine samples fb @8, gr @136, nvenc @264, nvdec @392, nvjpg @520,
/// nvofa @648, each 128 bytes `{util @0, vgpuScale @4, procId @8, subProcessID @12,
/// subProcessName[100] @16, pOsPidInfo u64 @120}`; 55888 bytes.
pub const GPUMON_SIZE: usize = 55888;
pub const GPUMON_COUNT: usize = 8;
pub const GPUMON_SAMPLES: usize = 16;
pub const GPUMON_SAMPLE_SIZE: usize = 776;
pub const GPUMON_SAMPLE_COUNT: usize = 72;
/// (engine, offset in a sample): the six engine samples.
pub const GPUMON_ENGINES: &[(&str, usize)] = &[
    ("fb", 8),
    ("gr", 136),
    ("nvenc", 264),
    ("nvdec", 392),
    ("nvjpg", 520),
    ("nvofa", 648),
];
pub const GPUMON_ENGINE_UTIL: usize = 0;
/// `procId` @8 up to the end of `pOsPidInfo` @120+8: every host process field.
pub const GPUMON_ENGINE_PROC: std::ops::Range<usize> = 8..128;

/// `NV2080_CTRL_GSP_GET_RM_HEAP_STATS_PARAMS` (ctrl2080gsp.h:131-137): `gfid` @0 (input),
/// then the managed size, largest free chunk and two snapshots; 88 bytes.
pub const RM_HEAP_STATS_SIZE: usize = 88;

/// `NV2080_CTRL_TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO_PARAMS` (ctrl2080tmr.h:201-205):
/// `cpuClkId` u8 @0, `sampleCount` u8 @1, `samples[16]` of `{cpuTime u64, gpuTime u64}` @8;
/// 264 bytes.
pub const TIME_CORR_SIZE: usize = 264;
pub const TIME_CORR_SAMPLE_COUNT: usize = 1;
pub const TIME_CORR_SAMPLES: usize = 8;
pub const TIME_CORR_MAX_SAMPLES: usize = 16;

/// `NV2080_CTRL_BIOS_GET_NBSI_V2_PARAMS` (ctrl2080bios.h:258-265): `module`, `path`,
/// `valueName`, `retBuf`, then `retSize` @1300 and `errorCode` @1304; 1308 bytes.
pub const NBSI_V2_SIZE: usize = 1308;
/// `retBuf[NV2080_BIOS_GET_NBSI_MAX_RET_SIZE]` (0x100, ctrl2080bios.h:254) ends at `retSize`.
pub const NBSI_V2_RET_BUF_LEN: usize = 0x100;
pub const NBSI_V2_RET_SIZE: usize = 1300;
pub const NBSI_V2_ERROR_CODE: usize = 1304;
/// `NV2080_CTRL_BIOS_GET_NBSI_NOT_FOUND` (ctrl2080bios.h:237).
pub const NBSI_NOT_FOUND: u32 = 0xFFFF_FFFF;

/// `NV2080_CTRL_CMD_RC_RECOVERY_ENABLED` (ctrl2080rc.h:272).
pub const RC_RECOVERY_ENABLED: u32 = 1;

/// `NV2080_CTRL_FB_GET_CTAGS_FOR_CBC_EVICTION_PARAMS` (ctrl2080fb.h:1559-1562):
/// `pCompTags[127]`, `numCompTags` @508; 512 bytes.
pub const CTAGS_SIZE: usize = 512;
pub const CTAGS_NUM: usize = 508;

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{offset_of, size_of};

    /// The layouts against the `nvrm-sys` bindings (the same in every version, `stable`).
    #[test]
    fn layouts_match_the_bindings() {
        type P = nvrm_sys::NV2080_CTRL_FB_GET_OFFLINED_PAGES_PARAMS;
        type E = nvrm_sys::NV2080_CTRL_FB_OFFLINED_ADDRESS_INFO;
        assert_eq!(size_of::<P>(), OFFLINED_PAGES_SIZE);
        assert_eq!(size_of::<E>(), OFFLINED_ENTRY_SIZE);
        assert_eq!(offset_of!(P, validEntries), OFFLINED_VALID_ENTRIES);
        assert_eq!(offset_of!(P, bRetirementPending), OFFLINED_B_RETIREMENT_PENDING);
        assert_eq!(offset_of!(P, retirementPending), OFFLINED_RETIREMENT_PENDING);
        assert_eq!(offset_of!(E, pageAddressWithEccOn), OFFLINED_ECC_ON);
        assert_eq!(offset_of!(E, pageAddressWithEccOff), OFFLINED_ECC_OFF);
        assert_eq!(offset_of!(E, status), OFFLINED_STATUS);
        assert_eq!(OFFLINED_MAX * OFFLINED_ENTRY_SIZE, OFFLINED_VALID_ENTRIES);
        // Bound per version (the entry struct differs by name only).
        type R610 = nvrm_sys::v610::NV2080_CTRL_FB_GET_REMAPPED_ROWS_PARAMS;
        type R615 = nvrm_sys::v615::NV2080_CTRL_FB_GET_REMAPPED_ROWS_PARAMS;
        assert_eq!(size_of::<R610>(), REMAPPED_ROWS_SIZE);
        assert_eq!(size_of::<R615>(), REMAPPED_ROWS_SIZE);
        assert_eq!(offset_of!(R610, flags), REMAPPED_ROWS_FLAGS);
        assert_eq!(offset_of!(R615, flags), REMAPPED_ROWS_FLAGS);
        assert_eq!(offset_of!(R610, entries), REMAPPED_ROWS_ENTRIES);
        assert_eq!(offset_of!(R615, entries), REMAPPED_ROWS_ENTRIES);
        assert_eq!(offset_of!(R615, entryCount), REMAPPED_ROWS_ENTRY_COUNT);
    }

    #[test]
    fn b_f2_layouts() {
        type S = nvrm_sys::NV2080_CTRL_GPU_QUERY_SCRUBBER_STATUS_PARAMS;
        assert_eq!(size_of::<S>(), SCRUBBER_SIZE);
        assert_eq!(offset_of!(S, scrubStartAddr), SCRUBBER_START);
        assert_eq!(offset_of!(S, scrubEndAddr), SCRUBBER_END);
        type D = nvrm_sys::NV2080_CTRL_FB_GET_DYNAMIC_OFFLINED_PAGES_PARAMS;
        assert_eq!(size_of::<D>(), DYN_OFFLINED_SIZE);
        assert_eq!(offset_of!(D, validEntries), DYN_OFFLINED_VALID_ENTRIES);
        assert_eq!(DYN_OFFLINED_MAX * DYN_OFFLINED_ENTRY_SIZE, DYN_OFFLINED_VALID_ENTRIES);
        assert_eq!(
            GPUMON_SAMPLES + GPUMON_SAMPLE_COUNT * GPUMON_SAMPLE_SIZE,
            GPUMON_SIZE
        );
        assert_eq!(GPUMON_ENGINES.last().unwrap().1 + 128, GPUMON_SAMPLE_SIZE);
    }

    #[test]
    fn b_s_layouts() {
        // Not bound by nvrm-sys: the cc measurement of the module header, restated.
        assert_eq!(TIME_CORR_SAMPLES + 16 * TIME_CORR_MAX_SAMPLES, TIME_CORR_SIZE);
        assert_eq!(NBSI_V2_ERROR_CODE + 4, NBSI_V2_SIZE);
        assert_eq!(CTAGS_NUM + 4, CTAGS_SIZE);
    }

    #[test]
    fn one_row_per_id() {
        let mut ids: Vec<u32> = PUBLIC.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PUBLIC.len());
    }
}

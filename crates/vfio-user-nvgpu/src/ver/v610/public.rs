// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
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
    /// A control on one of the guest's own channels or TSGs (c-guest-state.md).
    Guest(Guest),
}

/// Controls on the guest's objects (docs/caraxes/gsp-plan/c-guest-state.md). The host side
/// is only ever the guest object's own mirror (`mirror::Mirror::forward_control`, which
/// looks the object up by the guest's handle), never another host handle, and never the
/// device's host RM client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guest {
    /// Channel reset: on the channel's own host mirror as `host_cmd` (an INTERNAL control
    /// cannot be issued from host user space, so its public form); a channel without a
    /// mirror is answered by the device.
    Reset { host_cmd: u32 },
    /// `GET_MMU_FAULT_INFO`: the device's fault state of the channel.
    MmuFaultInfo,
    /// `GET_DEFER_RC_STATE`: the device defers no RC.
    DeferRcState,
    /// `RC_GET_ERROR_COUNT`: the number of records in the device's RC journal (rc.rs).
    RcErrorCount,
    /// `RC_GET_ERROR_V2`: record `whichBuffer` of the device's RC journal (rc.rs).
    RcErrorV2,
    /// Record the setting, and forward it to the object's mirror when there is one. A u32
    /// level @0 above `max_level` is recorded only.
    Record { max_level: Option<u32> },
    /// The value the setter `set` recorded; zeros (the default) before.
    Get { set: u32 },
    /// Record only, never forwarded (policy decision Q-C7).
    RecordOnly,
    /// Record the setting on the guest channel the params name (`hChannel` at
    /// `h_channel`, `hClient` at `h_client`, else the caller's client); never forwarded.
    ChannelRecord { h_client: Option<usize>, h_channel: usize },
    /// `NV0080 FIFO_IDLE_CHANNELS`: the listed guest channels; software channels are idle,
    /// mirrored ones go to the host RM's idle on the mirror's own channels.
    IdleChannels,
    /// `NV2080 FIFO_DISABLE_USERMODE_CHANNELS`: every guest channel of a TSG (the kernel's
    /// own bare channels stay), mirrored ones on their own host twins; the flag is the
    /// device's one set of scheduling flags (rc.rs `Sched`).
    DisableUsermode,
    /// `INTERNAL_FIFO_TOGGLE_ACTIVE_CHANNEL_SCHEDULING`: the device's scheduling flag over
    /// the guest channel set, mirrored ones on their own host twins.
    ToggleScheduling,
    /// `NV208F FIFO_CHECK_ENGINE_CONTEXT`: whether the guest promoted a GR context for the
    /// channel (or its TSG).
    CheckEngineContext,
    /// `NV0090 GET_MMU_DEBUG_MODE`: the TSG's MMU debug mode (the device never enables
    /// one).
    MmuDebugMode,
    /// `GR_GET_CTXSW_STATS`: the named channel's counters: its mirror's, zeros in software.
    CtxswStats,
    /// `GR_GET_CTXSW_MODES`: the named channel's modes: its mirror's, the recorded ones in
    /// software.
    CtxswModes,
    /// `GR_GET_CURRENT_RESIDENT_CHANNEL`: a guest ChID only when one of this VM's mirrors
    /// is resident.
    ResidentChannel,
    /// `NV208F FIFO_GET_CHANNEL_STATE`: the named channel's enabled and scheduled state.
    ChannelState,
    /// `NV83DE DEBUG_GET_MODE_ERRBAR_DEBUG`: disabled (its setter is refused).
    ErrbarDebugMode,
    /// `NVA06C GET_INFO`: the kernel's TSG id the device recorded.
    TsgInfo,
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
    /// `NV208F BIF_INFO`: the ASPM enables of the emulated PCIe Link Control register.
    BifInfo,
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

/// `NVA06C_CTRL_INTERLEAVE_LEVEL_MEDIUM` (ctrla06c.h:275): the highest level a guest TSG
/// or channel gets on the host runlist.
pub const INTERLEAVE_LEVEL_MEDIUM: u32 = 1;

pub const PUBLIC_C: &[PublicControl] = &[
    // RC group (c-guest-state.md §2 rows 6, 7, 11-13; §3 B6, B7).
    row(0x906F_0102, "NV906F RESET_CHANNEL", 16, Rule::Guest(Guest::Reset { host_cmd: 0x906F_0102 })),
    row(0x506F_0105, "NV506F RESET_ISOLATED_CHANNEL", 8, Rule::Guest(Guest::Reset { host_cmd: 0x506F_0105 })),
    row(0x506F_0106, "NV506F INTERNAL_RESET_ISOLATED_CHANNEL", 8, Rule::Guest(Guest::Reset { host_cmd: 0x506F_0105 })),
    row(0x906F_0106, "NV906F GET_MMU_FAULT_INFO", MMU_FAULT_INFO_SIZE, Rule::Guest(Guest::MmuFaultInfo)),
    row(0x906F_0105, "NV906F GET_DEFER_RC_STATE", 1, Rule::Guest(Guest::DeferRcState)),
    row(0x2080_2205, "RC_GET_ERROR_COUNT", 4, Rule::Guest(Guest::RcErrorCount)),
    row(0x2080_2213, "RC_GET_ERROR_V2", RC_ERROR_V2_SIZE, Rule::Guest(Guest::RcErrorV2)),
    // Group M: record, forward to the host mirror (rows 1-5, 15, 16, 20).
    row(0x0090_0101, "NV0090 SET_TPC_PARTITION_MODE", 8, Rule::Guest(Guest::Record { max_level: None })),
    row(0x0090_0103, "NV0090 GET_TPC_PARTITION_MODE", 8, Rule::Guest(Guest::Get { set: 0x0090_0101 })),
    row(0x0090_0107, "NV0090 PROGRAM_VIDMEM_PROMOTE", 8, Rule::Guest(Guest::Record { max_level: None })),
    row(0x0090_010B, "NV0090 SET_LG_SECTOR_PROMOTION", 4, Rule::Guest(Guest::Record { max_level: None })),
    row(0x0090_010E, "NV0090 PROGRAM_SYSMEM_PROMOTE", 4, Rule::Guest(Guest::Record { max_level: None })),
    row(0xA06C_0107, "NVA06C SET_INTERLEAVE_LEVEL", 4, Rule::Guest(Guest::Record { max_level: Some(INTERLEAVE_LEVEL_MEDIUM) })),
    row(0xA06C_0109, "NVA06C PROGRAM_VIDMEM_PROMOTE", 8, Rule::Guest(Guest::Record { max_level: None })),
    row(0xA06F_0109, "NVA06F SET_INTERLEAVE_LEVEL", 4, Rule::Guest(Guest::Record { max_level: Some(INTERLEAVE_LEVEL_MEDIUM) })),
    // Group L: recorded, never forwarded (Q-C7, decided: a guest must not outrank the host
    // desktop, nor restart a runlist the host shares).
    row(0xA06C_0110, "NVA06C MAKE_REALTIME", 1, Rule::Guest(Guest::RecordOnly)),
    row(0xA06F_0111, "NVA06F RESTART_RUNLIST", 2, Rule::Guest(Guest::RecordOnly)),
    // Group V: live migration, refused as a family.
    row(0xB06F_010B, "NVB06F GET_ENGINE_CTX_SIZE", 0, Rule::Refuse(REFUSED_MIGRATION)),
    row(0xB06F_010C, "NVB06F GET_ENGINE_CTX_DATA", 0, Rule::Refuse(REFUSED_MIGRATION)),
    row(0xB06F_010D, "NVB06F MIGRATE_ENGINE_CTX_DATA", 0, Rule::Refuse(REFUSED_MIGRATION)),
    row(0xB06F_010E, "NVB06F GET_ENGINE_CTX_STATE", 0, Rule::Refuse(REFUSED_MIGRATION)),
    row(0xB06F_010F, "NVB06F GET_CHANNEL_HW_STATE", 0, Rule::Refuse(REFUSED_MIGRATION)),
    row(0xB06F_0110, "NVB06F SET_CHANNEL_HW_STATE", 0, Rule::Refuse(REFUSED_MIGRATION)),
    row(0xB06F_0111, "NVB06F SAVE_ENGINE_CTX_DATA", 0, Rule::Refuse(REFUSED_MIGRATION)),
    row(0xB06F_0112, "NVB06F RESTORE_ENGINE_CTX_DATA", 0, Rule::Refuse(REFUSED_MIGRATION)),
    // Group K: kernel-privileged or vGPU-plugin controls without a caller on a GSP client.
    row(0x9067_0102, "NV9067 SET_TPC_PARTITION_TABLE", 0, Rule::Refuse(REFUSED_KERNEL)),
    row(0x9067_0201, "NV9067 GET_CWD_WATERMARK", 0, Rule::Refuse(REFUSED_KERNEL)),
    row(0x9067_0202, "NV9067 SET_CWD_WATERMARK", 0, Rule::Refuse(REFUSED_KERNEL)),
    row(0xC365_0108, "NVC365 ACCESS_CNTR_BUFFER_RESET_COUNTERS", 0, Rule::Refuse(REFUSED_KERNEL)),
    row(0xC36F_0109, "NVC36F GPFIFO_UPDATE_FAULT_METHOD_BUFFER", 0, Rule::Refuse(REFUSED_KERNEL)),
    // Group X: the INTERNAL TSG controls; a GSP client sends the public forms.
    row(0xA06C_0201, "NVA06C INTERNAL_GPFIFO_SCHEDULE", 0, Rule::Refuse(REFUSED_INTERNAL_TSG)),
    row(0xA06C_0202, "NVA06C INTERNAL_SET_TIMESLICE", 0, Rule::Refuse(REFUSED_INTERNAL_TSG)),
    // The category C rows of the D plan (d-device-model.md §10, "Rows for the category C
    // work"): device state and the guest's own mirror objects only, never a host-global
    // action. Five of the 17 are the D models' (rc.rs, dmodel.rs; the reconciliation of
    // 2026-09-25): FIFO_DISABLE_CHANNELS, GPU_REPORT_NON_REPLAYABLE_FAULT,
    // INTERNAL_RC_WATCHDOG_TIMEOUT, RC_SET_CLEAN_ERROR_HISTORY and
    // GPU_VALIDATE_MEM_MAP_REQUEST have no row here.
    row(0x0080_1714, "NV0080 FIFO_IDLE_CHANNELS", IDLE_CHANNELS_SIZE, Rule::Guest(Guest::IdleChannels)),
    row(0x2080_1117, "FIFO_DISABLE_USERMODE_CHANNELS", 1, Rule::Guest(Guest::DisableUsermode)),
    row(0x2080_0AC3, "INTERNAL_FIFO_TOGGLE_ACTIVE_CHANNEL_SCHEDULING", 1, Rule::Guest(Guest::ToggleScheduling)),
    row(0x208F_0401, "NV208F FIFO_CHECK_ENGINE_CONTEXT", 12, Rule::Guest(Guest::CheckEngineContext)),
    row(0x2080_1205, "GR_CTXSW_ZCULL_MODE", 16, Rule::Guest(Guest::ChannelRecord { h_client: None, h_channel: 0 })),
    row(0x2080_1211, "GR_CTXSW_PREEMPTION_BIND", PREEMPTION_BIND_SIZE, Rule::Guest(Guest::ChannelRecord { h_client: Some(4), h_channel: 8 })),
    row(0x2080_123A, "GR_CTXSW_SETUP_BIND", 0, Rule::Refuse(REFUSED_SETUP_BIND)),
    row(0x208F_0704, "NV208F BIF_INFO", 8, Rule::Device(Dev::BifInfo)),
    row(0x0073_137E, "NV0073 DP_GET_GENERIC_INFOFRAME", 0, Rule::Refuse(REFUSED_DISPLAY_CAPTURE)),
    row(0x0073_137F, "NV0073 DP_GET_MSA_ATTRIBUTES", 0, Rule::Refuse(REFUSED_DISPLAY_CAPTURE)),
    row(0x0080_1108, "NV0080 GR_SET_TPC_PARTITION_MODE", 0, Rule::Refuse(REFUSED_TPC_PARTITION)),
    row(0x2080_122C, "GR_SET_TPC_PARTITION_MODE", 0, Rule::Refuse(REFUSED_TPC_PARTITION)),
    // The open category B rows moved here (c-guest-state.md §3 B2-B5, B8, B10-B12; B1, the
    // ZBC table entry, belongs to the ZBC model, B9 `NV83DE DEBUG_GET_MODE_MMU_DEBUG` to the
    // debugger-session record of dmodel.rs, which its setter fills).
    row(0x0090_0105, "NV0090 GET_MMU_DEBUG_MODE", 1, Rule::Guest(Guest::MmuDebugMode)),
    row(0x2080_1215, "GR_GET_CTXSW_STATS", 32, Rule::Guest(Guest::CtxswStats)),
    row(0x2080_1235, "GR_GET_CTXSW_MODES", 24, Rule::Guest(Guest::CtxswModes)),
    row(0x2080_121C, "GR_GET_CURRENT_RESIDENT_CHANNEL", 24, Rule::Guest(Guest::ResidentChannel)),
    row(0x208F_0403, "NV208F FIFO_GET_CHANNEL_STATE", 16, Rule::Guest(Guest::ChannelState)),
    row(0x83DE_0320, "NV83DE DEBUG_GET_MODE_ERRBAR_DEBUG", 4, Rule::Guest(Guest::ErrbarDebugMode)),
    row(0x83DE_0325, "NV83DE DEBUG_GET_SINGLE_SM_DEBUGGER_STATUS", 0, Rule::Refuse(REFUSED_SM_DEBUGGER_STATUS)),
    row(0x83DE_032B, "NV83DE DEBUG_GET_MODE_MMU_GCC_DEBUG", 0, Rule::Refuse(REFUSED_MMU_GCC)),
    // The TSG's kernel id (classes.md §8, TSG): a GSP client answers GET_INFO itself
    // (kernel_channel_group_api.c:1386-1401, flags 0x8), so this row only keeps the answer
    // consistent should one arrive.
    row(0xA06C_0106, "NVA06C GET_INFO", 4, Rule::Guest(Guest::TsgInfo)),
];

pub const REFUSED_SETUP_BIND: &str =
    "refused (the Setup context-switch buffer bind: not modelled; per guest channel once TU106's GRAPHICS_SETUP context property is measured, d-device-model.md 6.2)";
pub const REFUSED_DISPLAY_CAPTURE: &str =
    "refused (readback of the guest's own head: the display model keeps no SDP or MSA capture yet; display-engine state, d-device-model.md 6.5)";
pub const REFUSED_TPC_PARTITION: &str =
    "refused (TPC partition mode of a guest TSG: unmeasured on the host, never forwarded; d-device-model.md 6.11)";
pub const REFUSED_SM_DEBUGGER_STATUS: &str =
    "refused (per-SM debugger status needs a debugger session with SM run control; the SM debugger is refused, d-device-model.md 6.16 and R5)";
pub const REFUSED_MMU_GCC: &str =
    "refused (MMU GCC debug mode: Blackwell onwards, absent on this chip)";

/// `NV0080_CTRL_FIFO_IDLE_CHANNELS_PARAMS` (ctrl0080fifo.h:396-401): `numChannels` @0,
/// `hChannels[4096]` @4, `flags` @16388, `timeout` @16392; 16396 bytes.
pub const IDLE_CHANNELS_SIZE: usize = 16396;
pub const IDLE_CHANNELS_MAX: usize = 4096;
pub const IDLE_CHANNELS_FLAGS: usize = 4 + 4 * IDLE_CHANNELS_MAX;
pub const IDLE_CHANNELS_TIMEOUT: usize = IDLE_CHANNELS_FLAGS + 4;
/// `NV2080_CTRL_GR_CTXSW_PREEMPTION_BIND_PARAMS` (ctrl2080gr.h:921-929): `flags` @0,
/// `hClient` @4, `hChannel` @8, `vMemPtrs[9]` @16, `gfxpPreemptMode` @88,
/// `cilpPreemptMode` @92, `grRouteInfo` @96; 112 bytes.
pub const PREEMPTION_BIND_SIZE: usize = 112;
pub const PREEMPTION_BIND_GFXP: usize = 88;
pub const PREEMPTION_BIND_CILP: usize = 92;
/// `NV2080_CTRL_GR_CTXSW_ZCULL_MODE_PARAMS` (ctrl2080gr.h:457-462): `zcullMode` @12.
pub const ZCULL_MODE_MODE: usize = 12;
/// `NV2080_CTRL_GR_GET_CTXSW_MODES_PARAMS` (ctrl2080gr.h:1856-1863): `hChannel` @0,
/// `zcullMode` @4, `pmMode` @8, `smpcMode` @12, `cilpPreemptMode` @16,
/// `gfxpPreemptMode` @20.
pub const CTXSW_MODES_ZCULL: usize = 4;
pub const CTXSW_MODES_CILP: usize = 16;
pub const CTXSW_MODES_GFXP: usize = 20;
/// `NV208F_CTRL_FIFO_GET_CHANNEL_STATE_PARAMS` (ctrl208ffifo.h:99-108): `hChannel` @0,
/// `hClient` @4, `bEnabled` @8, `bScheduled` @9, `bCpuMap` @10, `bContention` @11,
/// `bRunlistSet` @12, `bDeferRC` @13.
pub const CHANNEL_STATE_ENABLED: usize = 8;
pub const CHANNEL_STATE_SCHEDULED: usize = 9;
/// `NV208F_CTRL_FIFO_CHECK_ENGINE_CONTEXT_PARAMS` (ctrl208ffifo.h:61-65): `hChannel` @0,
/// `engine` @4, `exists` @8; `NV2080_ENGINE_TYPE_GRAPHICS` is the only valid engine.
pub const CHECK_CTX_ENGINE: usize = 4;
pub const CHECK_CTX_EXISTS: usize = 8;
pub const NV2080_ENGINE_TYPE_GRAPHICS: u32 = 1;
/// `NV208F_CTRL_BIF_INFO_PARAMS` (ctrl208fbif.h:124-133): `index` @0 (`L0S_ENABLED` 0,
/// `L1_ENABLED` 1), `data` @4.
pub const BIF_INFO_L0S: u32 = 0;
pub const BIF_INFO_L1: u32 = 1;
/// `NV83DE_CTRL_CMD_DEBUG_GET_MODE_ERRBAR_DEBUG_DISABLED` (ctrl83dedebug.h:915).
pub const ERRBAR_DEBUG_DISABLED: u32 = 0;

pub const REFUSED_MIGRATION: &str = "refused (no live migration)";
pub const REFUSED_KERNEL: &str =
    "refused (kernel-privileged or vGPU-plugin control; no caller on a GSP client)";
pub const REFUSED_INTERNAL_TSG: &str =
    "refused (INTERNAL TSG control; a GSP client sends the public form)";

/// `NV906F_CTRL_GET_MMU_FAULT_INFO_PARAMS` (ctrl906f.h:214-220): 104 bytes.
pub const MMU_FAULT_INFO_SIZE: usize = 104;
/// `NV2080_CTRL_RC_GET_ERROR_V2_PARAMS` (ctrl2080rc.h:124-128): `whichBuffer` @0 (input),
/// `outputRecordSize` @4, `recordBuffer` @8; 8200 bytes.
pub const RC_ERROR_V2_SIZE: usize = 8200;
pub const RC_ERROR_V2_OUTPUT_SIZE: usize = 4;
pub const RC_ERROR_V2_RECORD: usize = 8;
/// `RmProtoBuf_RECORD` (rmcd.h:108-111): the `NVCD_RECORD` header (nvcd.h:119-124:
/// `cRecordGroup` @0, `cRecordType` @1, `wRecordSize` u16 @2) and `dwSize` @4, the length
/// of the protobuf message after it; 8 bytes. `RmGroup` 1 (nvcd.h:105), `RmProtoBuf` 131
/// (rmcd.h:84).
pub const RC_PROTOBUF_RECORD_SIZE: usize = 8;
pub const RC_RECORD_GROUP_RM: u8 = 1;
pub const RC_RECORD_TYPE_PROTOBUF: u8 = 131;

pub const REFUSED_PROFILER: &str = "refused (profiler: profiling is refused, gsp-surface.md §3 D rule 4)";
pub const REFUSED_SCHED_LOG: &str = "refused (host scheduler log: every tenant's scheduling)";
pub const REFUSED_ACCOUNTING: &str = "refused (host accounting: host PIDs, D9 deny list)";

/// The row of `id`, if the table has one (category B rows, then category C rows).
pub fn public_control(id: u32) -> Option<&'static PublicControl> {
    PUBLIC.iter().chain(PUBLIC_C).find(|c| c.id == id)
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
        let mut ids: Vec<u32> = PUBLIC.iter().chain(PUBLIC_C).map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PUBLIC.len() + PUBLIC_C.len());
    }
}

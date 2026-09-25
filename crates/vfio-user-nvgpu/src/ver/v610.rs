// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Driver 610.57.04: the GSP RPC vocabulary, message layouts, RM classes, control
//! numbers and control parameter layouts the device speaks (docs/caraxes/versatility.md,
//! "Driver-version axis").
//!
//! Paths are relative to `vendor/open-gpu-kernel-modules` at tag 610.57.04; `sdk/`
//! is `src/common/sdk/nvidia/inc`. GSP message offsets are the compiler-measured ones
//! of docs/caraxes/gsp-abi-610.md (a), checked in the tests below. Control layouts
//! come from the typed bindings in `nvrm_sys::v610` wherever those exist.

use core::mem::{offset_of, size_of};

// The 610.57.04 bindings, named explicitly: this module is single-version by design
// (the `RmAbi` mediation of `nvrm-sys` is for code that serves several versions).
use nvrm_sys::v610 as ffi;

/// `NV_VERSION_STRING` of this driver: what the guest announces in
/// `SET_GUEST_SYSTEM_INFO` and what `GSP_GET_FEATURES` reports as firmware version.
pub const DRIVER_VERSION: &str = "610.57.04";

/// The `ChannelAllocParams` layout of this version (channel.rs), through `nvrm-sys`.
pub type RmAbi = nvrm_sys::V610;

/// The display control vocabulary (src/ver/v610/display.rs).
pub mod display;

/// The public controls of the GSP plan that are not a plain forward: filtered forwards,
/// device-owned answers, guest-object controls and refusals (src/ver/v610/public.rs).
pub mod public;

/// The CPU-GSP transport of this version: message queue v0, checksummed elements, the
/// four init RPCs (docs/caraxes/gsp-rpc-protocol.md).
pub const GSP_TRANSPORT: super::GspTransport = super::GspTransport::MsgqV1;

// ---------------------------------------------------------------------------------
// NV_STATUS values (src/common/sdk/nvidia/inc/nvstatuscodes.h).

pub const NV_OK: u32 = 0;
pub const NV_ERR_NOT_SUPPORTED: u32 = 0x56;
/// nvstatuscodes.h:60.
pub const NV_ERR_INVALID_ARGUMENT: u32 = 0x1F;

// ---------------------------------------------------------------------------------
// GSP RPC function numbers (src/nvidia/inc/kernel/vgpu/rpc_global_enums.h).

pub const FN_SET_GUEST_SYSTEM_INFO: u32 = 1;
pub const FN_FREE: u32 = 10;
pub const FN_DUP_OBJECT: u32 = 21;
pub const FN_GET_GSP_STATIC_INFO: u32 = 65;
pub const FN_UPDATE_BAR_PDE: u32 = 70;
pub const FN_CONTINUATION_RECORD: u32 = 71;
pub const FN_GSP_SET_SYSTEM_INFO: u32 = 72;
pub const FN_SET_REGISTRY: u32 = 73;
pub const FN_GSP_RM_CONTROL: u32 = 76;
pub const FN_GSP_RM_ALLOC: u32 = 103;
pub const FN_INIT_GSP_TRACE_CRASH_BUFFER: u32 = 228;
pub const EV_GSP_INIT_DONE: u32 = 0x1001;

// ---------------------------------------------------------------------------------
// Message-queue transport (gsp-abi-610.md (a), "Queue element and RPC header").

/// `GSP_MSG_QUEUE_ELEMENT` header: mctpHeader, nvdmHeader, checkSum, seqNum.
pub const ELEMENT_HEADER_SIZE: usize = 16;
/// `rpc_message_header_v` (= `rpc_message_header_v03_00`).
pub const RPC_HEADER_SIZE: usize = 32;
/// `mctpCreateTransportHeader` and `mctpCreateNvdmHeader` (mctp_format.h:40-54).
pub const MCTP_HEADER: u32 = 0xC000_0001;
pub const NVDM_HEADER: u32 = 0x2510_DE7E;
/// `RPC_HEADER_VERSION` 3.0 (g_rpc-message-header.h).
pub const RPC_HEADER_VERSION: u32 = 0x0300_0000;
/// "VRPC".
pub const RPC_SIGNATURE: u32 = 0x4350_5256;
pub const MSGQ_FLAGS_SWAP_RX: u32 = 1;
/// `maxRpcSize`: the largest element a queue slot run carries (rpc.c:2082-2092).
pub const MAX_RPC_SIZE: usize = 65520;

// ---------------------------------------------------------------------------------
// RPC payloads (gsp-abi-610.md (a)).

/// `rpc_gsp_rm_control_v03_00`.
pub const RM_CONTROL_H_CLIENT: usize = 0;
pub const RM_CONTROL_H_OBJECT: usize = 4;
pub const RM_CONTROL_CMD: usize = 8;
pub const RM_CONTROL_STATUS: usize = 12;
pub const RM_CONTROL_PARAMS_SIZE: usize = 16;
pub const RM_CONTROL_RPC_FLAGS: usize = 20;
pub const RM_CONTROL_RMCTRL_FLAGS: usize = 24;
pub const RM_CONTROL_PARAMS: usize = 40;
/// GSS legacy controls (bit 15 of the command): the guest caches the reply unless
/// `rmctrlFlags` comes back 0 (rpc.c:10902-10907, phase5-rpcs.md §2).
pub const CMD_GSS_LEGACY_BIT: u32 = 0x8000;

/// `rpc_gsp_rm_alloc_v03_00`.
pub const RM_ALLOC_H_CLIENT: usize = 0;
pub const RM_ALLOC_H_PARENT: usize = 4;
pub const RM_ALLOC_H_OBJECT: usize = 8;
pub const RM_ALLOC_H_CLASS: usize = 12;
pub const RM_ALLOC_STATUS: usize = 16;
pub const RM_ALLOC_PARAMS_SIZE: usize = 20;
pub const RM_ALLOC_FLAGS: usize = 24;
pub const RM_ALLOC_PARAMS: usize = 32;

/// `rpc_free_v03_00` (an NVOS00 inside).
pub const FREE_H_ROOT: usize = 0;
pub const FREE_H_PARENT: usize = 4;
pub const FREE_H_OBJECT: usize = 8;
pub const FREE_STATUS: usize = 12;
pub const FREE_SIZE: usize = 16;

/// `rpc_dup_object_v03_00`.
pub const DUP_STATUS: usize = 24;
pub const DUP_SIZE: usize = 28;

/// `rpc_init_gsp_trace_crash_buffer`: the buffer's physical address and size
/// (not in the measured set of gsp-abi-610.md (a); rpc.c).
pub const CRASH_BUFFER_PA: usize = 0;
pub const CRASH_BUFFER_SIZE: usize = 8;

/// `rpc_update_bar_pde_v15_00`: barType @0, entryValue u64 @8, entryLevelShift @16
/// (not in the measured set; g_rpc-structures.h).
pub const BAR_PDE_BAR_TYPE: usize = 0;
pub const BAR_PDE_ENTRY_VALUE: usize = 8;
pub const BAR_PDE_LEVEL_SHIFT: usize = 16;
/// `NV_RPC_UPDATE_PDE_BAR_2`: the value is PDE3\[0\] of the root at `bar2PdeBase`.
pub const BAR_PDE_BAR_TYPE_BAR2: u32 = 1;

/// `rpc_set_guest_system_info_v03_00`.
pub const GUEST_INFO_VGX_MAJOR: usize = 0;
pub const GUEST_INFO_VGX_MINOR: usize = 4;
pub const GUEST_INFO_DRIVER_VERSION: usize = 24;

/// `PACKED_REGISTRY_TABLE` (the SET_REGISTRY payload).
pub const REGISTRY_SIZE: usize = 0;
pub const REGISTRY_NUM_ENTRIES: usize = 4;
pub const REGISTRY_ENTRIES: usize = 8;
/// `PACKED_REGISTRY_ENTRY` (generated/g_os_nvoc.h:222-228): `nameOffset` counts from the
/// table's first byte; `type` is `REGISTRY_TABLE_ENTRY_TYPE_*` (g_os_nvoc.h:240-243),
/// and a DWORD entry carries its value in `data` (registry.c:585-589).
pub const REGISTRY_ENTRY_SIZE: usize = 16;
pub const REGISTRY_ENTRY_NAME_OFFSET: usize = 0;
pub const REGISTRY_ENTRY_TYPE: usize = 4;
pub const REGISTRY_ENTRY_DATA: usize = 8;
pub const REGISTRY_ENTRY_LENGTH: usize = 12;
pub const REGISTRY_TABLE_ENTRY_TYPE_DWORD: u8 = 1;

/// `GspSystemInfo`, the GSP_SET_SYSTEM_INFO payload. The first four fields are the
/// BAR0, BAR1 (FB), BAR3 (instance) and I/O addresses, one u64 each.
pub const SYSINFO_GPU_PHYS_ADDR: usize = 0;
pub const SYSINFO_GPU_PHYS_FB_ADDR: usize = 8;
pub const SYSINFO_GPU_PHYS_INST_ADDR: usize = 16;
pub const SYSINFO_GPU_PHYS_IO_ADDR: usize = 24;
pub const SYSINFO_BDF: usize = 32;
pub const SYSINFO_MAX_USER_VA: usize = 72;
pub const SYSINFO_PCI_CONFIG_MIRROR_BASE: usize = 80;
pub const SYSINFO_PCI_CONFIG_MIRROR_SIZE: usize = 84;
pub const SYSINFO_PCI_DEVICE_ID: usize = 88;
pub const SYSINFO_PCI_SUB_DEVICE_ID: usize = 92;
pub const SYSINFO_PCI_REVISION_ID: usize = 96;
pub const SYSINFO_HOST_PAGE_SIZE: usize = 920;

/// `GspStaticConfigInfo`, the GET_GSP_STATIC_INFO reply.
pub const STATIC_INFO_SIZE: usize = 1600;
pub const SI_GID_LENGTH: usize = 0x08;
pub const SI_GID_DATA: usize = 0x0C;
pub const SI_NUM_FB_REGIONS: usize = 0x140;
pub const SI_FB_REGIONS: usize = 0x148;
/// `NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO`, one element of `fbRegion`.
pub const FB_REGION_SIZE: usize = 56;
pub const FB_REGION_BASE: usize = 0;
pub const FB_REGION_LIMIT: usize = 8;
pub const FB_REGION_RESERVED: usize = 16;
pub const FB_REGION_PERFORMANCE: usize = 24;
pub const FB_REGION_SUPPORT_COMPRESSED: usize = 28;
pub const FB_REGION_SUPPORT_ISO: usize = 29;
pub const SI_ENGINE_CAPS: usize = 0x52C;
/// `engineCaps[NVGPU_ENGINE_CAPS_MASK_ARRAY_MAX]`.
pub const ENGINE_CAPS_WORDS: usize = 3;
pub const SI_FB_LENGTH: usize = 0x540;
pub const SI_GPU_NAME: usize = 0x548;
pub const SI_GPU_SHORT_NAME: usize = 0x588;
/// Both name strings are 64 bytes, NUL-terminated.
pub const SI_NAME_LEN: usize = 64;
pub const SI_BAR1_PDE_BASE: usize = 0x5E0;
pub const SI_BAR2_PDE_BASE: usize = 0x5E8;
pub const SI_VBIOS_VALID: usize = 0x5F0;
pub const SI_VBIOS_SUB_VENDOR: usize = 0x5F4;
pub const SI_VBIOS_SUB_DEVICE: usize = 0x5F8;
pub const SI_VBIOS_REVISION: usize = 0x5FC;
pub const SI_VBIOS_OEM_REVISION: usize = 0x600;
pub const SI_H_INTERNAL_CLIENT: usize = 0x608;
pub const SI_H_INTERNAL_DEVICE: usize = 0x60C;
pub const SI_H_INTERNAL_SUBDEVICE: usize = 0x610;
pub const SI_BUS_RESET_REQUIRED: usize = 0x638;
/// `fwWprLayoutOffset.nonWprHeapOffset`/`.frtsOffset`: read by the kernel RM only with
/// `PDB_PROP_GPU_FW_WPR_OFFSET_SET_BY_ACR` (GH100 and GB20x; kernel_gsp.c:4712-4728,
/// generated/g_gpu_nvoc.c:365-375), where ACR, not the CPU, lays out the WPR.
pub const SI_FW_WPR_NON_WPR_HEAP_OFFSET: usize = 0x628;
pub const SI_FW_WPR_FRTS_OFFSET: usize = 0x630;

/// `GspFwWprMeta` (gsp_fw_wpr_meta.h:211-212 for the magic).
pub const WPR_META_SIZE: usize = 256;
pub const WPR_META_MAGIC: u64 = 0xdc3a_ae21_371a_60b3;
pub const WPR_META_MAGIC_OFF: usize = 0x00;
pub const WPR_META_GSP_FW_RSVD_START: usize = 0x58;
pub const WPR_META_NON_WPR_HEAP_OFFSET: usize = 0x60;
pub const WPR_META_NON_WPR_HEAP_SIZE: usize = 0x68;
pub const WPR_META_GSP_FW_WPR_START: usize = 0x70;
pub const WPR_META_FRTS_OFFSET: usize = 0x98;
pub const WPR_META_GSP_FW_WPR_END: usize = 0xA8;
pub const WPR_META_FB_SIZE: usize = 0xB0;
pub const WPR_META_VGA_WORKSPACE_OFFSET: usize = 0xB8;
// The fields the CPU fills as size requests on GH100 and later and ACR completes
// (kgspPopulateWprMeta_GH100, kernel_gsp_gh100.c:303-397); the FSP boot model plays ACR.
pub const WPR_META_SIZE_OF_RADIX3_ELF: usize = 0x18;
pub const WPR_META_SIZE_OF_BOOTLOADER: usize = 0x28;
pub const WPR_META_GSP_FW_HEAP_OFFSET: usize = 0x78;
pub const WPR_META_GSP_FW_HEAP_SIZE: usize = 0x80;
pub const WPR_META_GSP_FW_OFFSET: usize = 0x88;
pub const WPR_META_BOOT_BIN_OFFSET: usize = 0x90;
pub const WPR_META_FRTS_SIZE: usize = 0xA0;
pub const WPR_META_VGA_WORKSPACE_SIZE: usize = 0xC0;
pub const WPR_META_PMU_RESERVED_SIZE: usize = 0xF4;

/// `GSP_FMC_BOOT_PARAMS` (arch/nvalloc/common/inc/gsp/gspifpub.h:115-122), the GSP-FMC
/// arguments whose address the COT message carries (kfspGetGspBootArgs,
/// kern_fsp_gh100.c:919-940; filled by kgspSetupGspFmcArgs_GH100, kernel_gsp_gh100.c:419-497).
pub const FMC_PARAMS_SIZE: usize = 88;
pub const FMC_GSP_RM_DESC_SIZE: usize = 0x0C;
pub const FMC_GSP_RM_DESC_OFFSET: usize = 0x10;
pub const FMC_BOOT_ARGS_OFFSET: usize = 0x30;

/// `LibosMemoryRegionInitArgument`, the array the libos args page holds.
pub const LIBOS_ARG_SIZE: u64 = 32;
pub const LIBOS_ARG_ID8: u64 = 0;
pub const LIBOS_ARG_PA: u64 = 8;
pub const LIBOS_ARG_REGION_SIZE: u64 = 16;
/// `id8` of the `RMARGS` libos region: "RMARGS" packed big-endian (`kernel_gsp.c:3219-3237`).
pub const RMARGS_ID8: u64 = 0x0000_524D_4152_4753;

/// `MESSAGE_QUEUE_INIT_ARGUMENTS`, the first member of `GSP_ARGUMENTS_CACHED` (RMARGS).
pub const MQ_SHARED_MEM_PA: u64 = 0;
pub const MQ_PAGE_TABLE_ENTRY_COUNT: u64 = 8;
pub const MQ_CMD_QUEUE_OFFSET: u64 = 16;
pub const MQ_STAT_QUEUE_OFFSET: u64 = 24;

// ---------------------------------------------------------------------------------
// Driver enums the chip tables name symbolically (src/chip; chips/<chip>/*.toml). A
// chip file says `COPY0`, this table says what number 610.57.04 gives it.

/// `NV2080_ENGINE_TYPE_*` (sdk/class/cl2080_notification.h:284-340), the bits of
/// `engineCaps`. NVJPEG0 and OFA0 are the GB206 additions (:330-331, :339-340).
pub const NV2080_ENGINE_TYPES: &[(&str, u32)] = &[
    ("GR0", 0x01),
    ("COPY0", 0x09),
    ("COPY1", 0x0A),
    ("COPY2", 0x0B),
    ("COPY3", 0x0C),
    ("COPY4", 0x0D),
    ("COPY5", 0x0E),
    ("COPY6", 0x0F),
    ("COPY7", 0x10),
    ("COPY8", 0x11),
    ("COPY9", 0x12),
    ("NVDEC0", 0x13),
    ("NVDEC1", 0x14),
    ("NVDEC2", 0x15),
    ("NVENC0", 0x1B),
    ("SW", 0x22),
    ("SEC2", 0x26),
    ("HOST", 0x27),
    ("NVJPEG0", 0x2B),
    ("OFA0", 0x33),
];

/// `RM_ENGINE_TYPE_*` (src/nvidia/inc/kernel/gpu/gpu_engine_type.h:35-96), the
/// `ENGINE_INFO_TYPE_RM_ENGINE_TYPE` column of the FIFO device-info table. COPY7,
/// NVJPEG0 and OFA0 are the GB206 additions (:50, :88, :96).
pub const RM_ENGINE_TYPES: &[(&str, u32)] = &[
    ("GR0", 0x01),
    ("COPY0", 0x09),
    ("COPY1", 0x0A),
    ("COPY2", 0x0B),
    ("COPY3", 0x0C),
    ("COPY4", 0x0D),
    ("COPY7", 0x10),
    ("NVDEC0", 0x1D),
    ("NVDEC1", 0x1E),
    ("NVDEC2", 0x1F),
    ("NVENC0", 0x25),
    ("SW", 0x2D),
    ("SEC2", 0x31),
    ("NVJPEG0", 0x36),
    ("OFA0", 0x3E),
];

/// NVOC class ids of the physical engines (src/nvidia/generated/g_eng_desc_nvoc.h);
/// `ENG_DESC` = class id << 8 | instance (`MKENGDESC`, :55-59). OBJNVJPG and OBJOFA
/// are the GB206 additions (:825, :865).
pub const ENG_DESC_CLASSES: &[(&str, u32)] = &[
    ("Graphics", 0xd3_34df),
    ("OBJCE", 0x79_3ceb),
    ("OBJBSP", 0x8f_99e1),
    ("OBJMSENC", 0xe9_7b6c),
    ("OBJSEC2", 0x28_c408),
    ("OBJSWENG", 0x95_a6f5),
    ("OBJNVJPG", 0x2b_3a54),
    ("OBJOFA", 0xdd_7bab),
];

/// `MC_ENGINE_IDX_*` (src/nvidia/inc/kernel/gpu/intr/engine_idx.h:39-172), an array
/// index in the guest. CE7, NVJPEG0 (= MC_ENGINE_IDX_NVJPG) and OFA0 are the GB206
/// additions (:61, :91-93, :125).
pub const MC_ENGINE_IDX: &[(&str, u32)] = &[
    ("NULL", 0),
    ("DISP", 2),
    ("TMR", 1),
    ("FIFO", 4),
    ("CE0", 15),
    ("CE1", 16),
    ("CE2", 17),
    ("CE3", 18),
    ("CE4", 19),
    ("CE7", 22),
    ("NVENC0", 38),
    ("SEC2", 47),
    ("GSP", 50),
    ("NVJPEG0", 51),
    ("REPLAYABLE_FAULT", 59),
    ("ACCESS_CNTR", 60),
    ("NON_REPLAYABLE_FAULT", 61),
    ("NVDEC0", 65),
    ("NVDEC1", 66),
    ("NVDEC2", 67),
    ("OFA0", 81),
    ("GR0", 84),
];

/// `NV2080_INTR_CATEGORY` (sdk/ctrl/ctrl2080/ctrl2080mc.h:337-345), the index into
/// the kernel interrupt table's `subtreeMap`.
pub const INTR_CATEGORIES: &[(&str, u32)] = &[
    ("DEFAULT", 0),
    ("ESCHED_DRIVEN_ENGINE", 1),
    ("ESCHED_DRIVEN_ENGINE_NOTIFICATION", 2),
    ("RUNLIST", 3),
    ("RUNLIST_NOTIFICATION", 4),
    ("UVM_OWNED", 5),
    ("UVM_SHARED", 6),
];

/// `NV2080_CTRL_FB_INFO_RAM_TYPE_*` (sdk/ctrl/ctrl2080/ctrl2080fb.h:400-423; GDDR7 for
/// GB206 at :423).
pub const RAM_TYPES: &[(&str, u32)] = &[
    ("GDDR5", 0x08),
    ("GDDR5X", 0x10),
    ("GDDR6", 0x11),
    ("GDDR7", 0x15),
];

/// The number `name` has in `table`, or `None`.
pub fn lookup(table: &[(&str, u32)], name: &str) -> Option<u32> {
    table.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

// ---------------------------------------------------------------------------------
// RM classes (sdk/class/*.h). The chip's own channel and copy classes are chip data.

pub const NV01_ROOT: u32 = 0x0;
pub const NV01_DEVICE_0: u32 = 0x80;
pub const NV20_SUBDEVICE_0: u32 = 0x2080;
pub const NV01_EVENT_KERNEL_CALLBACK_EX: u32 = 0x7E;
pub const NV2081_BINAPI: u32 = 0x2081;
pub const NV20_SUBDEVICE_DIAG: u32 = 0x208F;
pub const GF100_SUBDEVICE_INFOROM: u32 = 0x90E7;
pub const FERMI_VASPACE_A: u32 = 0x90F1;
pub const KEPLER_CHANNEL_GROUP_A: u32 = 0xA06C;
/// `FERMI_CONTEXT_SHARE_A` (cl9067.h): {hVASpace @0, flags @4, subctxId @8}
/// (nvos.h:3223-3234, `NV_CTXSHARE_ALLOCATION_PARAMETERS`).
pub const FERMI_CONTEXT_SHARE_A: u32 = 0x9067;
pub const CTXSHARE_HVASPACE: usize = 0;

// ---------------------------------------------------------------------------------
// Controls of the phase-5 dispatcher (ctrl.rs; sdk/ctrl/ctrl2080/*.h).

/// 0x20800A36 is `INTERNAL_GPU_GET_CHIP_INFO` in this version (ctrl2080internal.h:574-592);
/// 615 reuses the id (`ver::v615`), so ctrl.rs answers it by [`CTRL_0A36`].
pub const CMD_INTERNAL_GPU_GET_CHIP_INFO: u32 = 0x2080_0A36;
/// What 0x20800A36 names in this version.
pub const CTRL_0A36: super::Id0A36 = super::Id0A36::GetChipInfo;
pub const CMD_INTERNAL_GPU_GET_USER_REGISTER_ACCESS_MAP: u32 = 0x2080_0A41;
pub const CMD_GPU_GET_CONSTRUCTED_FALCON_INFO: u32 = 0x2080_01B0;
pub const CMD_INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO: u32 = 0x2080_0A87;
pub const CMD_FIFO_GET_DEVICE_INFO_TABLE: u32 = 0x2080_1112;
pub const CMD_INTERNAL_INTR_GET_KERNEL_TABLE: u32 = 0x2080_0A5C;
pub const CMD_INTERNAL_MEMSYS_GET_STATIC_CONFIG: u32 = 0x2080_0A1C;
pub const CMD_BUS_GET_PCI_BAR_INFO: u32 = 0x2080_1803;
pub const CMD_INTERNAL_BIF_GET_STATIC_INFO: u32 = 0x2080_0AAC;
pub const CMD_INTERNAL_INIT_USER_SHARED_DATA: u32 = 0x2080_0AFE;
pub const CMD_INTERNAL_USER_SHARED_DATA_SET_DATA_POLL: u32 = 0x2080_0AFF;
pub const CMD_EVENT_SET_NOTIFICATION: u32 = 0x2080_0301;
pub const CMD_INTERNAL_GMMU_GET_STATIC_INFO: u32 = 0x2080_0A59;
pub const CMD_INTERNAL_MEMSYS_L2_INVALIDATE_EVICT: u32 = 0x2080_0A6C;
pub const CMD_INTERNAL_PERF_GPU_BOOST_SYNC_GET_INFO: u32 = 0x2080_0A80;
pub const CMD_INTERNAL_FIFO_GET_NUM_CHANNELS: u32 = 0x2080_0A61;
pub const CMD_GPU_GET_VMMU_SEGMENT_SIZE: u32 = 0x2080_017E;
/// `NV2080_CTRL_CMD_FB_GET_GPU_CACHE_INFO` (sdk/ctrl/ctrl2080/ctrl2080fb.h:822-846), export
/// flags 0x40148 (NON_PRIVILEGED): `{powerState, writeMode, bypassMode, rcmState}`. The
/// NVIDIA EGL/GL driver asks it at `eglInitialize` and gives up on an error (display-e.md).
pub const CMD_FB_GET_GPU_CACHE_INFO: u32 = 0x2080_1315;
/// `NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES` (sdk/ctrl/ctrl0080/ctrl0080fifo.h:119),
/// export flags 0x50148 (NON_PRIVILEGED): `{engineId, alignment, size}` of an engine's
/// context buffer. The GL driver sizes its 3D context buffers with it (display-e.md).
pub const CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES: u32 = 0x0080_1707;
/// `NV2080_CTRL_CMD_GR_CTXSW_ZCULL_BIND` (sdk/ctrl/ctrl2080/ctrl2080gr.h:600-621), export flags
/// 0x90348: `{hClient, hChannel, vMemPtr (u64 at 8), zcullMode (at 16)}`; the GL driver binds
/// a zcull buffer to its 3D channel (display-e.md).
pub const CMD_GR_CTXSW_ZCULL_BIND: u32 = 0x2080_1208;
/// `WRITE_MODE_WRITEBACK` (ctrl2080fb.h:838); the other fields' 0 is ENABLED, DISABLED, FULL.
pub const GPU_CACHE_INFO_WRITE_MODE_WRITEBACK: u32 = 1;
pub const CMD_INTERNAL_BUS_FLUSH_WITH_SYSMEMBAR: u32 = 0x2080_0A70;
pub const CMD_INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_TYPE: u32 = 0x2080_2A0F;
pub const CMD_CE_UPDATE_CLASS_DB: u32 = 0x2080_2A06;
pub const CMD_CE_UPDATE_PCE_LCE_MAPPINGS_V2: u32 = 0x2080_2A0D;
pub const CMD_GPU_GET_OEM_BOARD_INFO: u32 = 0x2080_013F;
pub const CMD_INTERNAL_GET_PCIE_P2P_CAPS: u32 = 0x2080_0AB8;
pub const CMD_GSP_GET_FEATURES: u32 = 0x2080_3601;
pub const CMD_BIOS_GET_INFO_V2: u32 = 0x2080_0810;
pub const CMD_I2C_GET_PORT_INFO: u32 = 0x402C_0101;
pub const CMD_CE_GET_FAULT_METHOD_BUFFER_SIZE: u32 = 0x2080_2A08;

/// `NV2080_CTRL_INTERNAL_GPU_GET_CHIP_INFO_PARAMS` (ctrl2080internal.h:574-592).
pub const CHIP_INFO_SIZE: usize = size_of::<ffi::NV2080_CTRL_INTERNAL_GPU_GET_CHIP_INFO_PARAMS>();
pub const CHIP_INFO_PCI_DEVICE_ID: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GPU_GET_CHIP_INFO_PARAMS,
    pciDeviceId
);
pub const CHIP_INFO_PCI_SUB_DEVICE_ID: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GPU_GET_CHIP_INFO_PARAMS,
    pciSubDeviceId
);
pub const CHIP_INFO_PCI_REVISION_ID: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GPU_GET_CHIP_INFO_PARAMS,
    pciRevisionId
);
pub const CHIP_INFO_REG_BASES: usize =
    offset_of!(ffi::NV2080_CTRL_INTERNAL_GPU_GET_CHIP_INFO_PARAMS, regBases);
pub const CHIP_INFO_REG_BASE_COUNT: usize = 16;
/// `NV_REG_BASE_TIMER` and `NV_REG_BASE_USERMODE`, indices into `regBases`
/// (`gpuGetRegBaseOffset`, kern_gpu_tu102.c:275-306).
pub const REG_BASE_TIMER: usize = 2;
pub const REG_BASE_USERMODE: usize = 4;
/// Unsupported base (`NV_REG_BASE_...` not present).
pub const REG_BASE_NONE: u32 = 0xFFFF_FFFF;

/// `NV2080_CTRL_INTERNAL_GPU_GET_USER_REGISTER_ACCESS_MAP_PARAMS` (ctrl2080internal.h:775-788):
/// with `compressedSize` 0 the map allows every register.
pub const USER_REG_MAP_SIZE: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GPU_GET_USER_REGISTER_ACCESS_MAP_PARAMS,
    userRegisterAccessMapSize
);

// `NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE` is a HOST call whose flag bits moved in 615:
// crate::hostabi (`HostLayouts::flush_gpu_cache`), not the guest ABI.

/// `NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS` (ctrl2080fifo.h:535-576).
pub const DEVINFO_PARAMS_SIZE: usize =
    size_of::<ffi::NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS>();
pub const DEVINFO_NUM_ENTRIES: usize = offset_of!(
    ffi::NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS,
    numEntries
);
pub const DEVINFO_B_MORE: usize =
    offset_of!(ffi::NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS, bMore);
pub const DEVINFO_ENTRIES: usize =
    offset_of!(ffi::NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS, entries);
pub const DEVINFO_ENTRY_SIZE: usize = size_of::<ffi::NV2080_CTRL_FIFO_DEVICE_ENTRY>();
pub const DEVINFO_MAX_ENTRIES: usize = 32;
pub const DEVINFO_ENGINE_DATA: usize = offset_of!(ffi::NV2080_CTRL_FIFO_DEVICE_ENTRY, engineData);
pub const DEVINFO_PBDMA_IDS: usize = offset_of!(ffi::NV2080_CTRL_FIFO_DEVICE_ENTRY, pbdmaIds);
pub const DEVINFO_PBDMA_FAULT_IDS: usize =
    offset_of!(ffi::NV2080_CTRL_FIFO_DEVICE_ENTRY, pbdmaFaultIds);
pub const DEVINFO_NUM_PBDMAS: usize = offset_of!(ffi::NV2080_CTRL_FIFO_DEVICE_ENTRY, numPbdmas);
pub const DEVINFO_ENGINE_NAME: usize = offset_of!(ffi::NV2080_CTRL_FIFO_DEVICE_ENTRY, engineName);
/// `ENGINE_INFO_TYPE` indices of `engineData` (src/nvidia/inc/kernel/gpu/fifo/engine_info.h).
pub const ENGINE_INFO_ENG_DESC: usize = 0;
pub const ENGINE_INFO_RM_ENGINE_TYPE: usize = 2;
pub const ENGINE_INFO_RUNLIST: usize = 3;
pub const ENGINE_INFO_MMU_FAULT_ID: usize = 4;
pub const ENGINE_INFO_MC: usize = 8;
pub const ENGINE_INFO_DEV_TYPE_ENUM: usize = 9;
pub const ENGINE_INFO_INSTANCE_ID: usize = 10;
pub const ENGINE_INFO_IS_HOST_DRIVEN_ENGINE: usize = 12;

/// `NV2080_CTRL_INTERNAL_INTR_GET_KERNEL_TABLE_PARAMS` (ctrl2080internal.h:1463-1480).
pub const INTR_TABLE_SIZE: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_INTR_GET_KERNEL_TABLE_PARAMS>();
pub const INTR_TABLE_LEN: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_INTR_GET_KERNEL_TABLE_PARAMS,
    tableLen
);
pub const INTR_TABLE: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_INTR_GET_KERNEL_TABLE_PARAMS,
    table
);
pub const INTR_ENTRY_SIZE: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_INTR_GET_KERNEL_TABLE_ENTRY>();
pub const INTR_ENTRY_ENGINE_IDX: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_INTR_GET_KERNEL_TABLE_ENTRY,
    engineIdx
);
pub const INTR_ENTRY_VECTOR_STALL: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_INTR_GET_KERNEL_TABLE_ENTRY,
    vectorStall
);
pub const INTR_ENTRY_VECTOR_NONSTALL: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_INTR_GET_KERNEL_TABLE_ENTRY,
    vectorNonStall
);
pub const INTR_SUBTREE_MAP: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_INTR_GET_KERNEL_TABLE_PARAMS,
    subtreeMap
);
/// One `NV2080_INTR_CATEGORY_SUBTREE_MAP` (a u64 subtree mask).
pub const INTR_SUBTREE_MAP_ENTRY: usize = size_of::<ffi::NV2080_INTR_CATEGORY_SUBTREE_MAP>();

/// `NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS` (ctrl2080internal.h:94-135).
pub const MEMSYS_SIZE: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS>();
pub const MEMSYS_DISABLE_COMPBIT_BACKING: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS,
    bDisableCompbitBacking
);
pub const MEMSYS_L2_CACHE_SIZE: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS,
    l2CacheSize
);
pub const MEMSYS_FBPA_PRESENT: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS,
    bFbpaPresent
);
pub const MEMSYS_COMPR_PAGE_SIZE: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS,
    comprPageSize
);
pub const MEMSYS_COMPR_PAGE_SHIFT: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS,
    comprPageShift
);
pub const MEMSYS_RAM_TYPE: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS,
    ramType
);
pub const MEMSYS_LTC_COUNT: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS,
    ltcCount
);
pub const MEMSYS_LTS_PER_LTC_COUNT: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS,
    ltsPerLtcCount
);

/// `NV2080_CTRL_BUS_GET_PCI_BAR_INFO_PARAMS` (ctrl2080bus.h:612-646).
pub const BAR_INFO_SIZE: usize = size_of::<ffi::NV2080_CTRL_BUS_GET_PCI_BAR_INFO_PARAMS>();
pub const BAR_INFO_COUNT: usize =
    offset_of!(ffi::NV2080_CTRL_BUS_GET_PCI_BAR_INFO_PARAMS, pciBarCount);
pub const BAR_INFO_ENTRIES: usize =
    offset_of!(ffi::NV2080_CTRL_BUS_GET_PCI_BAR_INFO_PARAMS, pciBarInfo);
pub const BAR_INFO_ENTRY_SIZE: usize = size_of::<ffi::NV2080_CTRL_BUS_PCI_BAR_INFO>();
pub const BAR_INFO_BAR_SIZE_MIB: usize = offset_of!(ffi::NV2080_CTRL_BUS_PCI_BAR_INFO, barSize);
pub const BAR_INFO_BAR_SIZE_BYTES: usize =
    offset_of!(ffi::NV2080_CTRL_BUS_PCI_BAR_INFO, barSizeBytes);
pub const BAR_INFO_BAR_OFFSET: usize = offset_of!(ffi::NV2080_CTRL_BUS_PCI_BAR_INFO, barOffset);

/// `NV2080_CTRL_INTERNAL_INIT_USER_SHARED_DATA_PARAMS`.
pub const RUSD_PHYS_ADDR: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_INIT_USER_SHARED_DATA_PARAMS,
    physAddr
);

/// `NV2080_CTRL_INTERNAL_GMMU_GET_STATIC_INFO_PARAMS` (ctrl2080internal.h:1417-1426).
pub const GMMU_STATIC_SIZE: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_GMMU_GET_STATIC_INFO_PARAMS>();
pub const GMMU_REPLAYABLE_FAULT_BUFFER_SIZE: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GMMU_GET_STATIC_INFO_PARAMS,
    replayableFaultBufferSize
);
pub const GMMU_NON_REPLAYABLE_FAULT_BUFFER_SIZE: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GMMU_GET_STATIC_INFO_PARAMS,
    nonReplayableFaultBufferSize
);

/// `NV2080_CTRL_INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_TYPE_PARAMS` (ctrl2080ce.h:469-478):
/// the four output masks after `lceType` and the metadata (no typed layout).
pub const PCE_CONFIG_OUTPUTS: [usize; 4] = [8, 12, 16, 20];

/// `NV2080_CTRL_GSP_GET_FEATURES_PARAMS` (ctrl2080gsp.h:64-75; no typed layout):
/// gspFeatures @0, bValid @4, bDefaultGspRmGpu @5, firmwareVersion\[64\] @6.
pub const GSP_FEATURES_VALID: usize = 4;
pub const GSP_FEATURES_DEFAULT_GSP_RM_GPU: usize = 5;
pub const GSP_FEATURES_FIRMWARE_VERSION: usize = 6;
pub const GSP_FEATURES_SIZE: usize = 70;

/// `NV2080_CTRL_BIOS_GET_INFO_V2_PARAMS` (ctrl2080bios.h:44-45, 97-104).
pub const BIOS_INFO_LIST_SIZE: usize =
    offset_of!(ffi::NV2080_CTRL_BIOS_GET_INFO_V2_PARAMS, biosInfoListSize);
pub const BIOS_INFO_LIST: usize =
    offset_of!(ffi::NV2080_CTRL_BIOS_GET_INFO_V2_PARAMS, biosInfoList);
pub const BIOS_INFO_ENTRY_SIZE: usize = size_of::<ffi::NV2080_CTRL_BIOS_INFO>();
/// `NV2080_CTRL_BIOS_INFO_INDEX_REVISION` and `_OEM_REVISION`.
pub const BIOS_INFO_INDEX_REVISION: u32 = 0;
pub const BIOS_INFO_INDEX_OEM_REVISION: u32 = 1;

/// `NV2080_CTRL_INTERNAL_GET_PCIE_P2P_CAPS_PARAMS` (ctrl2080internal.h:2576-2580).
pub const P2P_READ_CAPS_STATUS: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GET_PCIE_P2P_CAPS_PARAMS,
    p2pReadCapsStatus
);
pub const P2P_WRITE_CAPS_STATUS: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GET_PCIE_P2P_CAPS_PARAMS,
    p2pWriteCapsStatus
);
/// `NV0000_P2P_CAPS_STATUS_NOT_SUPPORTED`.
pub const P2P_CAPS_STATUS_NOT_SUPPORTED: u8 = 5;

/// `NV2080_CTRL_INTERNAL_FIFO_GET_NUM_CHANNELS_PARAMS` (ctrl2080internal.h:1579-1586).
pub const NUM_CHANNELS_SIZE: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_FIFO_GET_NUM_CHANNELS_PARAMS>();
pub const NUM_CHANNELS: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_FIFO_GET_NUM_CHANNELS_PARAMS,
    numChannels
);

/// `NV2080_CTRL_CE_GET_FAULT_METHOD_BUFFER_SIZE_PARAMS` {size} (ctrl2080ce.h:295).
pub const CE_FAULT_METHOD_BUFFER_SIZE: usize = 0;

/// `NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO` (sdk/ctrl/ctrl2080/ctrl2080ce.h:351-374, the same
/// text in 610.57.04 and 615.71.09): which CE shim an LCE belongs to and its index inside
/// the shim. ROUTE_TO_PHYSICAL in both; PRIVILEGED in 610 (export flags 0x145,
/// src/nvidia/generated/g_subdevice_nvoc.c:11827), NON_PRIVILEGED in 615 (0x149, 615
/// g_subdevice_nvoc.c:12008), and the 615 libcuda asks it at cuInit for every CE
/// (caraxes/runs/cutover-615-2026-09-24/v1/cuda/guest/v1-615/cuda-managed.rm.tsv:214-228).
pub const CMD_CE_GET_LCE_SHIM_INFO: u32 = 0x2080_2A0C;
/// `NV2080_CTRL_CE_GET_LCE_SHIM_INFO_PARAMS` {ceEngineType (in), shimInstance (out),
/// shimLocalLceIdx (out)}, three NvU32 (ctrl2080ce.h:370-374). Not in the generated
/// bindings; measured by hand from the header, as the 615 libcuda sends it (len=12 in
/// the trace above).
pub const LCE_SHIM_INFO_SIZE: usize = 12;
pub const LCE_SHIM_CE_ENGINE_TYPE: usize = 0;
pub const LCE_SHIM_INSTANCE: usize = 4;
pub const LCE_SHIM_LOCAL_LCE_IDX: usize = 8;
/// `NV2080_ENGINE_TYPE_COPY0`..`COPY9` and `COPY10`..`COPY19`
/// (sdk/class/cl2080_notification.h:293-302, :342-351; `NV2080_ENGINE_TYPE_COPY_IDX`
/// :401-402).
pub fn copy_engine_index(nv2080: u32) -> Option<u32> {
    match nv2080 {
        0x09..=0x12 => Some(nv2080 - 0x09),
        0x34..=0x3D => Some(nv2080 - 0x34 + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------
// KernelGraphics static info (kgr.rs; ctrl2080internal.h, ctrl2080gr.h, ctrl0080gr.h).

pub const CMD_INTERNAL_GMMU_COPY_RESERVED_SPLIT_GVASPACE_PDES_TO_SERVER: u32 = 0x2080_0A9F;
pub const CMD_INTERNAL_STATIC_KGR_GET_CAPS: u32 = 0x2080_0A1F;
pub const CMD_INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER: u32 = 0x2080_0A22;
pub const CMD_INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS: u32 = 0x2080_0A26;
pub const CMD_INTERNAL_STATIC_KGR_GET_INFO: u32 = 0x2080_0A2A;
pub const CMD_INTERNAL_STATIC_KGR_GET_ZCULL_INFO: u32 = 0x2080_0A2C;
pub const CMD_INTERNAL_STATIC_KGR_GET_ROP_INFO: u32 = 0x2080_0A2E;
pub const CMD_INTERNAL_STATIC_KGR_GET_PPC_MASKS: u32 = 0x2080_0A30;
pub const CMD_INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO: u32 = 0x2080_0A32;
pub const CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER: u32 = 0x2080_0A34;
pub const CMD_INTERNAL_GR_GET_FECS_TRACE_HW_ENABLE: u32 = 0x2080_0A38;
pub const CMD_INTERNAL_STATIC_KGR_GET_FECS_RECORD_SIZE: u32 = 0x2080_0A3D;
pub const CMD_INTERNAL_STATIC_KGR_GET_FECS_TRACE_DEFINES: u32 = 0x2080_0A3F;
pub const CMD_INTERNAL_STATIC_KGR_GET_PDB_PROPERTIES: u32 = 0x2080_0A48;
pub const CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER_V2: u32 = 0x2080_0B03;
pub const CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_THROTTLE_CTRL: u32 = 0x2080_0B05;

pub const CMD_GR_GET_ZCULL_INFO: u32 = 0x2080_1206;
pub const CMD_GR_GET_ROP_INFO: u32 = 0x2080_1213;
pub const CMD_GR_GET_GLOBAL_SM_ORDER: u32 = 0x2080_121B;
pub const CMD_GR_GET_CAPS_V2: u32 = 0x2080_1227;
pub const CMD_GR_GET_INFO_V2: u32 = 0x2080_1228;
pub const CMD_GR_GET_ENGINE_CONTEXT_PROPERTIES: u32 = 0x2080_122D;
pub const CMD_GR_GET_SM_ISSUE_RATE_MODIFIER: u32 = 0x2080_1230;
pub const CMD_GR_GET_SM_ISSUE_RATE_MODIFIER_V2: u32 = 0x2080_123C;
pub const CMD_GR_GET_SM_ISSUE_THROTTLE_CTRL: u32 = 0x2080_123D;

/// NV2080_CTRL_INTERNAL_GR_MAX_ENGINES (ctrl2080internal.h:190).
pub const GR_MAX_ENGINES: usize = 8;

// Byte layouts. `*_ELEM` is one engine's element of the internal array, `*_PARAMS`
// the whole internal params struct, `PUB_*` the public one. Computed from the typed
// bindings; the tests in kgr.rs pin the numbers the handlers were written against.

/// NV2080_CTRL_INTERNAL_STATIC_GR_CAPS { NvU8 capsTbl\[23\] } (ctrl2080internal.h:198-200).
pub const CAPS_ELEM: usize = size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_CAPS>();
pub const CAPS_PARAMS: usize = size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_CAPS_PARAMS>();
/// NV0080_CTRL_GR_GET_CAPS_V2_PARAMS (ctrl0080gr.h:283-287).
pub const PUB_CAPS_V2: usize = size_of::<ffi::NV2080_CTRL_GR_GET_CAPS_V2_PARAMS>();

/// NV0080_CTRL_GR_INFO_MAX_SIZE = 0x3f (ctrl0080gr.h:176).
pub const GR_INFO_COUNT: usize = 0x3f;
/// 63 x NV2080_CTRL_INTERNAL_GR_INFO {index u32, data u32} (ctrl2080internal.h:411-414).
pub const INFO_ELEM: usize = size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_INFO>();
pub const INFO_PARAMS: usize = size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_INFO_PARAMS>();
/// NV0080_CTRL_GR_GET_INFO_V2_PARAMS (ctrl0080gr.h:293-297).
pub const PUB_INFO_V2: usize = size_of::<ffi::NV2080_CTRL_GR_GET_INFO_V2_PARAMS>();
pub const PUB_INFO_V2_LIST: usize = offset_of!(ffi::NV2080_CTRL_GR_GET_INFO_V2_PARAMS, grInfoList);

/// NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS (ctrl2080internal.h:300-326).
pub const FS_ELEM: usize = size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS>();
pub const FS_PARAMS: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_FLOORSWEEPING_MASKS_PARAMS>();
/// Fields of element 0 (E5c: assembled from the host's public GR controls, kgr.rs).
pub const FS_MAX_GPC: usize = 64;
pub const FS_GPC_MASK: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS,
    gpcMask
);
pub const FS_TPC_MASK: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS,
    tpcMask
);
pub const FS_TPC_COUNT: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS,
    tpcCount
);
pub const FS_PHYS_GPC_MASK: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS,
    physGpcMask
);
pub const FS_TPC_TO_PES_MAP: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS,
    tpcToPesMap
);
pub const FS_NUM_PES_PER_GPC: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS,
    numPesPerGpc
);
pub const FS_ZCULL_MASK: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS,
    zcullMask
);
pub const FS_PHYS_GFX_GPC_MASK: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS,
    physGfxGpcMask
);
pub const FS_NUM_GFX_TPC: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS,
    numGfxTpc
);
/// The public GR controls the floorsweeping masks are assembled from, all NON_PRIVILEGED
/// (g_subdevice_nvoc.c flags 0x10118, 0x10118, 0x8, 0x10018, 0x8, 0x8). Layouts from
/// ctrl2080gr.h (no typed bindings): `NV2080_CTRL_GR_ROUTE_INFO` {flags u32, route u64}
/// is 16 bytes, so GPC_MASK is {route, gpcMask @16} (24 bytes), TPC_MASK {route,
/// gpcId @16, tpcMask @20}, GFX_GPC_AND_TPC_INFO {route, physGfxGpcMask @16,
/// numGfxTpc @20}; NUM_TPCS_FOR_GPC and ZCULL_MASK are {gpcId @0, value @4}.
pub const CMD_GR_GET_GPC_MASK: u32 = 0x2080_122A;
pub const CMD_GR_GET_TPC_MASK: u32 = 0x2080_122B;
pub const CMD_GR_GET_NUM_TPCS_FOR_GPC: u32 = 0x2080_1234;
pub const CMD_GR_GET_ZCULL_MASK: u32 = 0x2080_1237;
pub const CMD_GR_GET_GFX_GPC_AND_TPC_INFO: u32 = 0x2080_1239;
pub const CMD_GPU_GET_PES_INFO: u32 = 0x2080_0168;
pub const GR_ROUTED_SIZE: usize = 24;
pub const GR_ROUTED_ARG: usize = 16;
pub const GR_ROUTED_OUT: usize = 20;
// `GPU_GET_PES_INFO` and the public `GR_GET_GLOBAL_SM_ORDER` are host calls: their
// layouts are the HOST driver's and live in hostabi.rs (`HostLayouts`), chosen at run
// time. What follows is the guest's side.

/// Entries of the guest's `tpcToPesMap` in `NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS`
/// (ten in 610; the tests pin it to the struct).
pub const FS_TPC_TO_PES_MAP_LEN: usize = 10;
const _: () = assert!(FS_TPC_TO_PES_MAP + 4 * FS_TPC_TO_PES_MAP_LEN == FS_NUM_PES_PER_GPC);

/// NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER (ctrl2080internal.h:223-238): 512 SMs
/// of nine u16, then numSm and numTpc. The host's public struct is translated into it
/// field by name (hostabi.rs `translate_sm_order`, kgr.rs `global_sm_order`).
pub const SM_ORDER_GUEST: crate::hostabi::SmOrderLayout = crate::hostabi::sm_order_layout!(
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER,
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER__bindgen_ty_1;
    gpcId, localTpcId, localSmId, globalTpcId, virtualGpcId, migratableTpcId, ugpuId,
    physicalCpcId, virtualTpcId
);
pub const SM_ENTRY: usize = SM_ORDER_GUEST.entry_size;
pub const SM_ORDER_ELEM: usize = SM_ORDER_GUEST.size;
pub const SM_ORDER_NUM_SM: usize = SM_ORDER_GUEST.num_sm;
pub const SM_ORDER_NUM_TPC: usize = SM_ORDER_GUEST.num_tpc;
const _: () = assert!(SM_ORDER_GUEST.fields.len() * 2 == SM_ENTRY);
pub const SM_ORDER_PARAMS: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_GLOBAL_SM_ORDER_PARAMS>();

/// NV2080_CTRL_INTERNAL_STATIC_GR_ZCULL_INFO; the public ZCULL_INFO is the same 10 x u32.
pub const ZCULL_ELEM: usize = size_of::<ffi::NV2080_CTRL_GR_GET_ZCULL_INFO_PARAMS>();
pub const ZCULL_PARAMS: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_ZCULL_INFO_PARAMS>();

/// NV2080_CTRL_INTERNAL_STATIC_GR_ROP_INFO; the public ROP_INFO is the same 3 x u32.
pub const ROP_ELEM: usize = size_of::<ffi::NV2080_CTRL_GR_GET_ROP_INFO_PARAMS>();
pub const ROP_PARAMS: usize = size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_ROP_INFO_PARAMS>();

/// NV2080_CTRL_INTERNAL_STATIC_GR_PPC_MASKS { NvU32 mask\[64\] } (ctrl2080internal.h:492-494).
pub const PPC_PARAMS: usize = size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_PPC_MASKS_PARAMS>();

/// NV2080_CTRL_INTERNAL_ENGINE_CONTEXT_BUFFER_INFO {size, alignment} x 0x1a per engine
/// (ctrl2080internal.h:517-524).
pub const CTX_ENGINE_COUNT: usize = 0x1a;
pub const CTX_ELEM: usize = CTX_ENGINE_COUNT * 8;
pub const CTX_PARAMS: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_CONTEXT_BUFFERS_INFO_PARAMS>();
/// NV2080_CTRL_GR_GET_ENGINE_CONTEXT_PROPERTIES_PARAMS (ctrl2080gr.h:1519-1525).
pub const PUB_CTX_PROPS: usize =
    size_of::<ffi::NV2080_CTRL_GR_GET_ENGINE_CONTEXT_PROPERTIES_PARAMS>();
pub const PUB_CTX_PROPS_ENGINE_ID: usize = offset_of!(
    ffi::NV2080_CTRL_GR_GET_ENGINE_CONTEXT_PROPERTIES_PARAMS,
    engineId
);
pub const PUB_CTX_PROPS_ALIGNMENT: usize = offset_of!(
    ffi::NV2080_CTRL_GR_GET_ENGINE_CONTEXT_PROPERTIES_PARAMS,
    alignment
);
pub const PUB_CTX_PROPS_SIZE: usize = offset_of!(
    ffi::NV2080_CTRL_GR_GET_ENGINE_CONTEXT_PROPERTIES_PARAMS,
    size
);

/// NV2080_CTRL_INTERNAL_STATIC_GR_SM_ISSUE_RATE_MODIFIER: 9 x u8 (ctrl2080internal.h:544-554).
pub const SMIR_ELEM: usize = 9;
pub const SMIR_PARAMS: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_SM_ISSUE_RATE_MODIFIER_PARAMS>();
/// NV2080_CTRL_GR_GET_SM_ISSUE_RATE_MODIFIER_PARAMS (ctrl2080gr.h:1634-1645):
/// grRouteInfo, then the same nine u8.
pub const PUB_SMIR: usize = size_of::<ffi::NV2080_CTRL_GR_GET_SM_ISSUE_RATE_MODIFIER_PARAMS>();
pub const PUB_SMIR_DATA: usize =
    offset_of!(ffi::NV2080_CTRL_GR_GET_SM_ISSUE_RATE_MODIFIER_PARAMS, imla0);

/// listSize u32 + 255 x {index, data} (ctrl2080internal.h:5401-5404, 5429-5432); the
/// public V2 and THROTTLE_CTRL structs are the same 2044 bytes.
pub const LIST_ELEM: usize = size_of::<ffi::NV2080_CTRL_GR_GET_SM_ISSUE_RATE_MODIFIER_V2_PARAMS>();
pub const LIST_PARAMS: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_SM_ISSUE_RATE_MODIFIER_V2_PARAMS>();

/// NV2080_CTRL_INTERNAL_STATIC_GR_GET_FECS_RECORD_SIZE (ctrl2080internal.h:688-696).
pub const FECS_RECORD_SIZE_PARAMS: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_FECS_RECORD_SIZE_PARAMS>();
/// NV2080_CTRL_INTERNAL_STATIC_GR_GET_FECS_TRACE_DEFINES (ctrl2080internal.h:725-731).
pub const FECS_TRACE_PARAMS: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_FECS_TRACE_DEFINES_PARAMS>();
/// NV2080_CTRL_INTERNAL_STATIC_GR_PDB_PROPERTIES { NvBool } x 8 (ctrl2080internal.h:830-832).
pub const PDB_PARAMS: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_PDB_PROPERTIES_PARAMS>();
/// NV2080_CTRL_INTERNAL_GR_FECS_TRACE_HW_ENABLE_PARAMS (ctrl2080internal.h:614-617).
pub const FECS_HW_ENABLE_PARAMS: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_GR_FECS_TRACE_HW_ENABLE_PARAMS>();
pub const FECS_HW_ENABLE_BENABLE: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GR_FECS_TRACE_HW_ENABLE_PARAMS,
    bEnable
);

/// sizeof(FECS_EVENT_RECORD), src/nvidia/src/kernel/gpu/gr/fecs_event_list.c:58-69:
/// six NvU32 (magic_lo, magic_hi, context_id, context_ptr, new_context_id,
/// new_context_ptr) = 24 bytes @0; NvU64 ts\[5\] (NV_FECS_TRACE_MAX_TIMESTAMPS, :53)
/// needs 8-byte alignment and 24 is aligned, so 40 bytes @24; NvU32 reserved\[13\] =
/// 52 bytes @64; NvU32 seqno @116; end 120, already a multiple of the struct
/// alignment 8, so no tail padding.
pub const FECS_EVENT_RECORD_SIZE: u32 = 6 * 4 + 5 * 8 + 13 * 4 + 4;

/// NV0080_CTRL_FIFO_GET_ENGINE_CONTEXT_PROPERTIES_ENGINE_ID_GRAPHICS_FECS_EVENT
/// (sdk/ctrl/ctrl0080/ctrl0080fifo.h:145).
pub const CTX_ENGINE_FECS_EVENT: usize = 0x17;

// ---------------------------------------------------------------------------------
// nvidia-smi controls (nvml.rs; docs/caraxes/nvml.md).

pub const CMD_GPU_GET_INFO_V2: u32 = 0x2080_0102;
pub const CMD_INTERNAL_GPU_GET_SMC_MODE: u32 = 0x2080_0A4C;
pub const CMD_THERMAL_SYSTEM_EXECUTE_V2: u32 = 0x2080_0513;
pub const CMD_THERMAL_SYSTEM_EXECUTE_V2_PHYSICAL: u32 = 0x2080_8513;
pub const CMD_GPU_GET_CHIP_DETAILS: u32 = 0x2080_01A4;
pub const CMD_NVLINK_GET_PLATFORM_INFO: u32 = 0x2080_3083;
pub const CMD_FB_GET_INFO_V2: u32 = 0x2080_1303;
pub const CMD_BINAPI_0108: u32 = 0x2081_0108;
pub const CMD_DIAG_GPU_VERIFY_INFOROM: u32 = 0x208F_1105;

/// NV2081_BINAPI commands (the class forwards any command to GSP, binary_api.c:60-118;
/// none is named in the open headers). 0x0108 (992 bytes) is in the host's own
/// `nvidia-smi -q` trace; 0x0110 (1 byte) and 0x0111 (4 bytes) are what the guest's
/// `nvidia-smi` adds; 0x0107 (2 bytes) is what the NVIDIA GL/EGL driver asks at context
/// creation (display-e.md).
pub const BINAPI: &[u32] = &[0x2081_0107, CMD_BINAPI_0108, 0x2081_0110, 0x2081_0111];

/// `INDEX_FORWARD_TO_PHYSICAL` (subdevice_ctrl_gpu_kernel.c:86): the guest kernel marks
/// the GET_INFO_V2 indices it cannot serve; the physical side clears the bit.
pub const INDEX_FORWARD_TO_PHYSICAL: u32 = 0x8000_0000;
/// Unnamed GET_INFO_V2 indices whose host values are chip-unique (the host returned
/// 0xbe1523a4 and 0x79f23fe1 in the 610.57.04 trace; `nvidia-smi -q` prints them as
/// the GPU PDI). Identity, so derived from the device's own GID.
pub const INFO_CHIP_UNIQUE_LO: u32 = 0x23;
pub const INFO_CHIP_UNIQUE_HI: u32 = 0x24;
/// NV2080_CTRL_GPU_INFO_GPU_SMC_MODE_UNSUPPORTED (ctrl2080gpu.h:168).
pub const SMC_MODE_UNSUPPORTED: u32 = 0;
/// GET_INFO_V2 lists: listSize u32 @0, then {index u32, data u32} entries @4.
pub const INFO_LIST: usize = offset_of!(ffi::NV2080_CTRL_GPU_GET_INFO_V2_PARAMS, gpuInfoList);
/// NV2080_CTRL_GPU_GET_CHIP_DETAILS_PARAMS.pciDevId (ctrl2080gpu.h:4289-4294).
pub const CHIP_DETAILS_PCI_DEV_ID: usize =
    offset_of!(ffi::NV2080_CTRL_GPU_GET_CHIP_DETAILS_PARAMS, pciDevId);
/// THERMAL_SYSTEM_EXECUTE_V2 (ctrl2080thermal.h:397-415): six u32 header words
/// (clientAPIVersion, clientAPIRevision, clientInstructionSizeOf, executeFlags,
/// successfulInstructions, instructionListSize), then 44-byte instructions
/// {result, executed, opcode, operands}.
pub const THERMAL_API_VERSION: usize = 0;
pub const THERMAL_API_REVISION: usize = 4;
pub const THERMAL_INSN_SIZE: usize = 8;
pub const THERMAL_SUCCESSFUL_INSNS: usize = 16;
pub const THERMAL_INSN_COUNT: usize = 20;
pub const THERMAL_INSNS: usize = 24;
pub const THERMAL_INSN_STRIDE: usize = 44;
pub const THERMAL_INSN_OPCODE: usize = 8;
pub const THERMAL_MAX_INSNS: usize = 32;
/// FB_GET_INFO_V2 indices that describe the FB size, in KiB: RAM_SIZE 7,
/// TOTAL_RAM_SIZE 8, HEAP_SIZE 9, MAPPABLE_HEAP_SIZE 0xA, HEAP_FREE 0x16,
/// USABLE_RAM_SIZE 0x20 (ctrl2080fb.h:322-354).
pub const FB_INFO_SIZE_INDICES: &[u32] = &[0x7, 0x8, 0x9, 0xA, 0x16, 0x20];

// ---------------------------------------------------------------------------------
// Host memory (hostmem.rs, docs/caraxes/memory-model.md): the VRAM reservation and the
// guest-RAM OS descriptors on the host RM. The host driver is this version as well
// (`nvrm_sys::assert_driver_version` when the host RM client opens).

/// `NV01_MEMORY_LOCAL_USER` (class/cl0040.h:34): the VRAM reservation.
pub const NV01_MEMORY_LOCAL_USER: u32 = 0x40;
/// `NV2080_CTRL_CMD_FB_GET_FB_REGION_INFO` (ctrl2080fb.h:890), NON_PRIVILEGED.
pub const CMD_FB_GET_FB_REGION_INFO: u32 = 0x2080_1320;
pub type FbRegionInfoParams = ffi::NV2080_CTRL_CMD_FB_GET_FB_REGION_INFO_PARAMS;
/// `NV2080_CTRL_FB_INFO_INDEX_*` (ctrl2080fb.h:322-354), in KiB.
pub const FB_INFO_TOTAL_RAM_SIZE: u32 = 0x08;
pub const FB_INFO_HEAP_BASE_KB: u32 = 0x11;
pub const FB_INFO_LARGEST_FREE_REGION_SIZE_KB: u32 = 0x12;
pub const FB_INFO_LARGEST_FREE_REGION_BASE_KB: u32 = 0x13;
pub const FB_INFO_HEAP_FREE: u32 = 0x16;
/// `NVOS32_ATTR2_PAGE_SIZE_HUGE` 21:20 = `_2MB` (nvos.h:1266-1268).
pub const ATTR2_PAGE_SIZE_HUGE_2MB: u32 = 1 << 20;
/// `NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR` (ctrl0041.h:113), PRIVILEGED (flags 0x4,
/// `src/nvidia/generated/g_mem_nvoc.c:166`): the IOVA of an OS descriptor's page.
pub const CMD_GET_SURFACE_PHYS_ATTR: u32 = 0x0041_0103;
/// `NV0041_CTRL_GET_SURFACE_PHYS_ATTR_PARAMS` (ctrl0041.h:117-127): memOffset u64 @0
/// (in: offset into the surface, out: its address), memAperture u32 @20 (1 = SYSMEM),
/// contigSegmentSize u64 @40; 48 bytes. Not in the generated bindings; measured by hand
/// from the header (natural alignment, NV_DECLARE_ALIGNED 8).
pub const PHYS_ATTR_SIZE: usize = 48;
pub const PHYS_ATTR_MEM_OFFSET: usize = 0;
pub const PHYS_ATTR_MEM_APERTURE: usize = 20;
pub const PHYS_ATTR_CONTIG_SEGMENT_SIZE: usize = 40;

/// Public, NON_PRIVILEGED, ROUTE_TO_PHYSICAL controls whose params are plain data of
/// the same layout on guest and host (same driver build), forwarded unchanged to the
/// host object of the same class. None of them carries an identity field.
pub const FORWARD: &[(u32, &str)] = &[
    (0x0080_0294, "GPU_GET_BRAND_CAPS"),
    (0x2080_012F, "GPU_QUERY_ECC_STATUS"),
    (0x2080_014B, "GPU_GET_INFOROM_OBJECT_VERSION"),
    (0x2080_0156, "GPU_GET_INFOROM_IMAGE_VERSION"),
    (0x2080_0157, "GPU_QUERY_INFOROM_ECC_SUPPORT"),
    // FB_GET_OFFLINED_PAGES 0x20801322 and FB_GET_REMAPPED_ROWS 0x20801344 carry host FB
    // pages and DRAM rows: filtered forwards of `public::PUBLIC` since 2026-09-24.
    (0x2080_1357, "FB_QUERY_DRAM_ENCRYPTION_INFOROM_SUPPORT"),
    // Sent by the 615 guest's nvidia-smi (caraxes/runs/transport-615-2026-09-24/boot/
    // device-events.txt:1114, 8208 bytes). Not new in 615: 610.57.04 has the same control
    // and layout (sdk/ctrl/ctrl2080/ctrl2080fb.h:2990-3014; `nvrm_sys::v610` and `v615`
    // both 8208 bytes, hostabi.rs tests), NON_PRIVILEGED and ROUTE_TO_PHYSICAL (flags 0x58,
    // g_subdevice_nvoc.c), so both host drivers answer it.
    (0x2080_1361, "FB_GET_REMAPPED_BANKS"),
    (0x2080_1813, "BUS_GET_PEX_COUNTERS"),
    (0x2080_1819, "BUS_GET_PEX_UTIL_COUNTERS"),
    (0x2080_1823, "BUS_GET_INFO_V2"),
    (0x2080_1829, "BUS_GET_PCIE_REQ_ATOMICS_CAPS"),
    (0x2080_1830, "BUS_GET_PCIE_CPL_ATOMICS_CAPS"),
    // cuInit (the CUDA gate, 2026-09-23) dies on NOT_SUPPORTED here; the real card answers
    // OK. All three are NON_PRIVILEGED, ROUTE_TO_PHYSICAL, plain-data params
    // (g_subdevice_nvoc.c flags 0x40048, 0x50048, 0x10248).
    (0x2080_182A, "BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS"),
    (0x2080_182B, "BUS_GET_C2C_INFO"),
    (0x2080_3801, "GRMGR_GET_GR_FS_INFO"),
    (0x2080_2068, "PERF_GET_CURRENT_PSTATE"),
    (0x2080_2087, "PERF_GET_VID_ENG_PERFMON_SAMPLE"),
    // Group F1 of docs/caraxes/gsp-plan/b-host-forwardable.md (§2, §3): NON_PRIVILEGED
    // ROUTE_TO_PHYSICAL subdevice getters whose answer is card truth, with no handle,
    // address, PID or other client's state; the params have the same sizeof and text in
    // 610.57.04 and 615.71.09 (§2). NOT_SUPPORTED from a GeForce host is passed on: it is
    // what the real card says.
    (0x2080_0133, "GPU_QUERY_ECC_CONFIGURATION"),
    // Seen in the 595 guest's nvidia-smi run (caraxes/runs/guest-driver-2026-09-24/
    // 595-static/device-events.txt:12, unanswered until 2026-09-24): the board LED.
    (0x2080_0153, "GPU_QUERY_ILLUM_SUPPORT"),
    (0x2080_0154, "GPU_GET_ILLUM"),
    (0x2080_0173, "GPU_QUERY_FUNCTION_STATUS"),
    (0x2080_01AB, "GPU_GET_RESET_STATUS"),
    (0x2080_01AE, "GPU_GET_DRAIN_AND_RESET_STATUS"),
    (0x2080_0809, "BIOS_GET_POST_TIME"),
    (0x2080_1037, "CLK_PMUMON_CLK_DOMAINS_GET_SAMPLES"),
    (0x2080_1120, "FIFO_OBJSCHED_GET_STATE"),
    (0x2080_130C, "FB_GET_CALIBRATION_LOCK_FAILED"),
    (0x2080_1328, "FB_GET_LTC_INFO_FOR_FBP"),
    (0x2080_1347, "FB_GET_ROW_REMAPPER_HISTOGRAM"),
    (0x2080_1355, "FB_QUERY_DRAM_ENCRYPTION_PENDING_CONFIGURATION"),
    (0x2080_1358, "FB_QUERY_DRAM_ENCRYPTION_STATUS"),
    (0x2080_1816, "BUS_GET_PEX_LANE_COUNTERS"),
    (0x2080_1817, "BUS_GET_PCIE_LTR_LATENCY"),
    (0x2080_200B, "PERF_GET_LEVEL_INFO_V2"),
    (0x2080_205A, "PERF_GET_POWERSTATE"),
    (0x2080_206E, "PERF_RATED_TDP_GET_CONTROL"),
    (0x2080_3101, "FLCN_GET_DMEM_USAGE"),
    (0x2080_3400, "ECC_GET_CLIENT_EXPOSED_COUNTERS"),
    (0x2080_3401, "ECC_GET_VOLATILE_COUNTS"),
    (0x2080_3404, "ECC_GET_REPAIR_STATUS"),
    (0x2080_3406, "ECC_GET_UNREPAIRABLE_MEMORY_FLAG"),
    (0x2080_3604, "GSP_GET_LIBOS_HEAP_STATS"),
    (0x2080_81F3, "GPU_IS_RESET_COUPLED"),
    // Unanswered in the guest control sweep of 2026-09-25 (caraxes/runs/coverage-2026-09-25/
    // extra/work/ctlsweep-table-out.txt; docs/caraxes/coverage-map.md §3). NON_PRIVILEGED
    // ROUTE_TO_PHYSICAL getters of the card's own configuration (flags 0x48 to 0x50048,
    // caraxes/abi/gsp-6*/controls.tsv), with the same layout in 610.57.04 and 615.71.09, and no
    // handle, address, PID or other client's state in the answer. The other fourteen of the
    // sweep are chip-table rows (chips/tu106/controls.toml).
    //
    // gpEntries/pbEntries of an engine's latency buffer (ctrl0080fifo.h:189-215), on the
    // host device object.
    (0x0080_170E, "FIFO_GET_LATENCY_BUFFER_SIZE"),
    // The IP_VER register of a unit: display, HDA codec, PMGR, PMU, display falcon
    // (ctrl2080gpu.h:1836-1868).
    (0x2080_014D, "GPU_GET_IP_VERSION"),
    // The supported scheduling policies and timeslice limits of an engine
    // (ctrl2080fifo.h:991-1048); FIFO_OBJSCHED_GET_STATE is forwarded above.
    (0x2080_1122, "FIFO_OBJSCHED_GET_CAPS"),
    // The GPC/TPC tile map of the card's floorswept GR (ctrl2080gr.h:1865-1884); grRouteInfo
    // is 0 without MIG.
    (0x2080_1236, "GR_GET_GPC_TILE_MAP"),
    // Floorsweeping masks of FBPs, LTCs and L2 slices (ctrl2080fb.h:2340-2362), as
    // GRMGR_GET_GR_FS_INFO above is for GR.
    (0x2080_1346, "FB_GET_FS_INFO"),
    // Board module id and NVSwitch support (ctrl2080pmgr.h:39-66).
    (0x2080_2609, "PMGR_GET_MODULE_INFO"),
    // Falcon or RISC-V per engine (ctrl2080flcn.h:128-154).
    (0x2080_3118, "FLCN_GET_ENGINE_ARCH"),
];

/// GSS-legacy controls (NV2080 categories 0x85 THERMAL, 0x90 CLK, 0xA0 PERF, 0xA6
/// PMGR, all `_LEGACY_NON_PRIVILEGED`, ctrl2080base.h:37-96). Their numbers are not
/// defined in the open headers; the set is what the host's `nvidia-smi -q` sends
/// (matrix/traces/610.57.04/nvml.tsv), with the params size seen there, plus the two
/// GPU_LEGACY_NON_PRIVILEGED (0x81) commands of cuInit.
pub const LEGACY: &[(u32, usize, &str)] = &[
    (0x2080_852A, 1784, "thermal 0x2a"),
    (0x2080_852C, 1440, "thermal 0x2c"),
    // 0x2e: output only (the request is all zeros); the host fills 1432 bytes, the
    // size of THERMAL_SYSTEM_EXECUTE_V2 but not its instruction list.
    (0x2080_852E, 1432, "thermal 0x2e"),
    (0x2080_852F, 844, "thermal 0x2f"),
    // thermal 0x36 (0x20808536), pmgr 0x18 (0x2080A618) and perf 0xf1 (0x2080A0F1) have a
    // size that depends on the host driver: hostabi.rs `HostLayouts::legacy`.
    (0x2080_853A, 15352, "thermal 0x3a"),
    (0x2080_8542, 504, "thermal 0x42"),
    (0x2080_8546, 24, "thermal 0x46"),
    (0x2080_9004, 1544, "clk 0x04"),
    (0x2080_9009, 8, "clk 0x09"),
    (0x2080_9019, 12336, "clk 0x19"),
    (0x2080_9038, 4, "clk 0x38"),
    (0x2080_9064, 520, "clk 0x64"),
    (0x2080_A014, 12, "perf 0x14"),
    (0x2080_A060, 28, "perf 0x60"),
    (0x2080_A06E, 12, "perf 0x6e"),
    (0x2080_A079, 83972, "perf 0x79"),
    (0x2080_A080, 52, "perf 0x80"),
    (0x2080_A081, 1872, "perf 0x81"),
    (0x2080_A097, 1168, "perf 0x97"),
    (0x2080_A0A4, 67396, "perf 0xa4"),
    (0x2080_A0A7, 17676, "perf 0xa7"),
    (0x2080_A0A8, 32908, "perf 0xa8"),
    (0x2080_A0F2, 96, "perf 0xf2"),
    (0x2080_A612, 54352, "pmgr 0x12"),
    (0x2080_A613, 79936, "pmgr 0x13"),
    (0x2080_A61A, 13840, "pmgr 0x1a"),
    (0x2080_A61F, 8, "pmgr 0x1f"),
    (0x2080_A63C, 28, "pmgr 0x3c"),
    // GPU_LEGACY_NON_PRIVILEGED (0x81) commands libcuda's cuInit sends after
    // GPU_QUERY_COMPUTE_MODE_RULES and after SYSTEM_GET_P2P_CAPS_MATRIX; the native
    // 610.57.04 trace (matrix/traces/610.57.04/cuda-core.tsv:288, :301) has them with
    // these sizes and NV_OK. cuInit returns 100 when 0x59 fails (E5c).
    (0x2080_8159, 332, "gpu 0x59 (cuInit)"),
    (0x2080_8162, 1, "gpu 0x62 (cuInit)"),
    // Telemetry libcuda sends under torch (matrix/traces/610.57.04/cuda-torch.tsv:2207-2219,
    // NV_OK natively; cuda-lookahead.md §4.10).
    (0x2080_9001, 8, "clk 0x01 (torch)"),
    (0x2080_A026, 532, "perf 0x26 (torch)"),
    (0x2080_A084, 4, "perf 0x84 (torch)"),
];

// ---------------------------------------------------------------------------------
// nvidia-uvm controls and the channel emulation (uvm.rs, channel.rs; docs/caraxes/uvm.md).

pub const CMD_INTERNAL_UVM_REGISTER_ACCESS_CNTR_BUFFER: u32 = 0x2080_0A1D;
pub const CMD_INTERNAL_UVM_UNREGISTER_ACCESS_CNTR_BUFFER: u32 = 0x2080_0A1E;
pub const CMD_INTERNAL_GMMU_REGISTER_FAULT_BUFFER: u32 = 0x2080_0A9B;
pub const CMD_INTERNAL_GMMU_UNREGISTER_FAULT_BUFFER: u32 = 0x2080_0A9C;
pub const CMD_INTERNAL_GMMU_REGISTER_CLIENT_SHADOW_FAULT_BUFFER: u32 = 0x2080_0A9D;
pub const CMD_INTERNAL_GMMU_UNREGISTER_CLIENT_SHADOW_FAULT_BUFFER: u32 = 0x2080_0A9E;
pub const CMD_CE_GET_CE_PCE_MASK: u32 = 0x2080_2A02;
pub const CMD_CE_GET_CAPS_V2: u32 = 0x2080_2A03;
pub const CMD_CE_GET_PHYSICAL_CAPS: u32 = 0x2080_2A07;
/// `NV2080_CTRL_CMD_CE_GET_ALL_CAPS` (public) and its INTERNAL twin (flags 0x101d0), the
/// same `NV2080_CTRL_CE_GET_ALL_CAPS_PARAMS` layout (ctrl2080ce.h:333-348).
pub const CMD_CE_GET_ALL_CAPS: u32 = 0x2080_2A0A;
pub const CMD_CE_GET_ALL_PHYSICAL_CAPS: u32 = 0x2080_2A0B;
pub const CMD_CE_IS_DECOMP_LCE_ENABLED: u32 = 0x2080_2A12;
pub const CMD_DMA_SET_PAGE_DIRECTORY: u32 = 0x0080_1813;
pub const CMD_DMA_UNSET_PAGE_DIRECTORY: u32 = 0x0080_1814;
pub const CMD_VASPACE_COPY_SERVER_RESERVED_PDES: u32 = 0x90F1_0106;
pub const CMD_CHANNEL_GPFIFO_SCHEDULE: u32 = 0xA06F_0103;
/// `NVA06F_CTRL_CMD_BIND` (sdk/ctrl/ctrla06f/ctrla06fgpfifo.h), `{engineType}`: the NVIDIA
/// GL/EGL driver binds its channel to GR before allocating the 3D object (display-e.md).
pub const CMD_CHANNEL_BIND: u32 = 0xA06F_0104;
pub const CMD_TSG_GPFIFO_SCHEDULE: u32 = 0xA06C_0101;
pub const CMD_TSG_INTERNAL_PROMOTE_FAULT_METHOD_BUFFERS: u32 = 0xA06C_010A;

/// NV2080_CTRL_INTERNAL_GMMU_REGISTER_FAULT_BUFFER_PARAMS (ctrl2080internal.h:1813-1830).
pub const FAULT_BUFFER_SIZE: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GMMU_REGISTER_FAULT_BUFFER_PARAMS,
    faultBufferSize
);
pub const FAULT_BUFFER_PTE_ARRAY: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GMMU_REGISTER_FAULT_BUFFER_PARAMS,
    faultBufferPteArray
);
/// NV2080_CTRL_INTERNAL_GMMU_REGISTER_CLIENT_SHADOW_FAULT_BUFFER_PARAMS (ctrl2080internal.h:1863-1880).
pub const SHADOW_FAULT_BUFFER_QUEUE_PA: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GMMU_REGISTER_CLIENT_SHADOW_FAULT_BUFFER_PARAMS,
    shadowFaultBufferQueuePhysAddr
);
pub const SHADOW_FAULT_BUFFER_SIZE: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GMMU_REGISTER_CLIENT_SHADOW_FAULT_BUFFER_PARAMS,
    shadowFaultBufferSize
);
/// NV2080_CTRL_INTERNAL_UVM_REGISTER_ACCESS_CNTR_BUFFER_PARAMS (ctrl2080internal.h:160-176).
pub const ACCESS_CNTR_INDEX: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_UVM_REGISTER_ACCESS_CNTR_BUFFER_PARAMS,
    accessCounterIndex
);
pub const ACCESS_CNTR_BUFFER_SIZE: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_UVM_REGISTER_ACCESS_CNTR_BUFFER_PARAMS,
    bufferSize
);
pub const ACCESS_CNTR_PTE_ARRAY: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_UVM_REGISTER_ACCESS_CNTR_BUFFER_PARAMS,
    bufferPteArray
);
/// NV2080_CTRL_CE_GET_CAPS_V2_PARAMS {ceEngineType u32, capsTbl\[2\]} (ctrl2080ce.h:82-85;
/// no typed layout). CE_GET_PHYSICAL_CAPS has the same layout.
pub const CE_CAPS_ENGINE_TYPE: usize = 0;
pub const CE_CAPS_TBL: usize = 4;
/// NV2080_CTRL_CE_IS_DECOMP_LCE_ENABLED_PARAMS {lceIndex u32, bDecompEnabled u8 @4}
/// (ctrl2080ce.h:513-520; no typed layout).
pub const DECOMP_LCE_INDEX: usize = 0;
pub const DECOMP_ENABLED: usize = 4;

/// NV90F1_CTRL_VASPACE_COPY_SERVER_RESERVED_PDES_PARAMS (ctrl90f1.h:272-330).
pub const PDES_VIRT_ADDR_LO: usize = offset_of!(
    ffi::NV90F1_CTRL_VASPACE_COPY_SERVER_RESERVED_PDES_PARAMS,
    virtAddrLo
);
pub const PDES_VIRT_ADDR_HI: usize = offset_of!(
    ffi::NV90F1_CTRL_VASPACE_COPY_SERVER_RESERVED_PDES_PARAMS,
    virtAddrHi
);
pub const PDES_NUM_LEVELS: usize = offset_of!(
    ffi::NV90F1_CTRL_VASPACE_COPY_SERVER_RESERVED_PDES_PARAMS,
    numLevelsToCopy
);
pub const PDES_LEVELS: usize = offset_of!(
    ffi::NV90F1_CTRL_VASPACE_COPY_SERVER_RESERVED_PDES_PARAMS,
    levels
);
pub const PDES_LEVEL_SIZE: usize =
    size_of::<ffi::NV90F1_CTRL_VASPACE_COPY_SERVER_RESERVED_PDES_PARAMS__bindgen_ty_1>();
pub const PDES_MAX_LEVELS: usize = 6;
pub const PDES_LEVEL_PHYS_ADDRESS: usize = offset_of!(
    ffi::NV90F1_CTRL_VASPACE_COPY_SERVER_RESERVED_PDES_PARAMS__bindgen_ty_1,
    physAddress
);
pub const PDES_LEVEL_SIZE_FIELD: usize = offset_of!(
    ffi::NV90F1_CTRL_VASPACE_COPY_SERVER_RESERVED_PDES_PARAMS__bindgen_ty_1,
    size
);
pub const PDES_LEVEL_APERTURE: usize = offset_of!(
    ffi::NV90F1_CTRL_VASPACE_COPY_SERVER_RESERVED_PDES_PARAMS__bindgen_ty_1,
    aperture
);
pub const PDES_LEVEL_PAGE_SHIFT: usize = offset_of!(
    ffi::NV90F1_CTRL_VASPACE_COPY_SERVER_RESERVED_PDES_PARAMS__bindgen_ty_1,
    pageShift
);

/// NV0080_CTRL_DMA_SET_PAGE_DIRECTORY_PARAMS (ctrl0080dma.h:798-818; no typed layout):
/// physAddress u64 @0, numEntries @8, flags @12, hVASpace @16. FLAGS_APERTURE 1:0 is
/// 0 vidmem, 1 sysmem coherent, 2 sysmem non-coherent.
pub const SET_PD_PHYS_ADDRESS: usize = 0;
pub const SET_PD_NUM_ENTRIES: usize = 8;
pub const SET_PD_FLAGS: usize = 12;
pub const SET_PD_H_VASPACE: usize = 16;
/// NV0080_CTRL_DMA_UNSET_PAGE_DIRECTORY_PARAMS {hVASpace @0, subDeviceId @4}.
pub const UNSET_PD_H_VASPACE: usize = 0;

/// NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS (nvos.h:2900-2907).
pub const TSG_HVASPACE: usize = offset_of!(ffi::NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS, hVASpace);
pub const TSG_ENGINE_TYPE: usize =
    offset_of!(ffi::NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS, engineType);

/// NV_MEMORY_DESC_PARAMS {base u64, size u64, addressSpace u32, cacheAttrib u32}
/// (alloc_channel.h:51-56); `NV_ADDR_SYSMEM` 1 / `NV_ADDR_FBMEM` 2
/// (src/nvidia/inc/kernel/vgpu/rm_plugin_shared_code.h:66-67).
pub const MEM_DESC_BASE: usize = offset_of!(ffi::NV_MEMORY_DESC_PARAMS, base);
pub const MEM_DESC_ADDRESS_SPACE: usize = offset_of!(ffi::NV_MEMORY_DESC_PARAMS, addressSpace);
pub const ADDR_SYSMEM: u32 = 1;
pub const ADDR_FBMEM: u32 = 2;

// ---------------------------------------------------------------------------------
// The CUDA path (cuda.rs; docs/caraxes/testing.md §8, experiments.md E5c).

/// `NV2080_CTRL_CMD_INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO` (ctrl2080internal.h:1982),
/// INTERNAL: the guest kernel asks for it from `GPU_GET_SKYLINE_INFO` when its MIG
/// static info is empty (subdevice_ctrl_gpu_smc.c:59-70).
pub const CMD_INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO: u32 = 0x2080_0AA2;
pub const SKYLINE_INFO_SIZE: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO_PARAMS>();
pub const SKYLINE_VALID_ENTRIES: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO_PARAMS,
    validEntries
);

/// `NV2080_CTRL_CMD_GPU_PROMOTE_CTX` (ctrl2080gpu.h:950-971), PRIVILEGED (0x10244): the
/// guest kernel promotes the GR context buffers of a channel (PAs, at compute-object
/// alloc) and UVM its VAs (bind). No typed binding; measured from the header (natural
/// alignment): engineType @0, hClient @4, ChID @8, hChanClient @12, hObject @16,
/// hVirtMemory @20, virtAddress @24, size @32, entryCount @40, promoteEntry\[16\] @48 of
/// {gpuPhysAddr @0, gpuVirtAddr @8, size @16, physAttr @24, bufferId u16 @28,
/// bInitialize @30, bNonmapped @31} (32 bytes); 560 bytes in all.
pub const CMD_GPU_PROMOTE_CTX: u32 = 0x2080_012B;
pub const PROMOTE_ENGINE_TYPE: usize = 0;
pub const PROMOTE_H_CHAN_CLIENT: usize = 12;
pub const PROMOTE_H_OBJECT: usize = 16;
pub const PROMOTE_ENTRY_COUNT: usize = 40;
pub const PROMOTE_ENTRIES: usize = 48;
pub const PROMOTE_ENTRY_SIZE: usize = 32;
pub const PROMOTE_MAX_ENTRIES: usize = 16;
pub const PROMOTE_PARAMS_SIZE: usize = PROMOTE_ENTRIES + PROMOTE_ENTRY_SIZE * PROMOTE_MAX_ENTRIES;
/// Controls of the context path that real GSP acts on and that the device, without a
/// host channel, only acknowledges (docs/caraxes/cuda-lookahead.md C2, C25-C27, C29,
/// X3, X6, I-10; flags from g_*_nvoc.c).
pub const CMD_INTERNAL_PERF_BOOST_SET_2X: u32 = 0x2080_0A9A;
pub const CMD_KGR_INTERNAL_INIT_BUG4208224_WAR: u32 = 0x0080_2046;
pub const CMD_INTERNAL_PERF_CUDA_LIMIT_SET_CONTROL: u32 = 0x0080_2009;
/// `NV0080_CTRL_CMD_INTERNAL_PERF_CUDA_LIMIT_DISABLE` (ctrl0080internal.h:82), sent on the
/// internal device when a device with a CUDA limit is freed (kern_cuda_limit.c:62-73).
pub const CMD_INTERNAL_PERF_CUDA_LIMIT_DISABLE: u32 = 0x0080_2004;
pub const CMD_DEBUG_SET_EXCEPTION_MASK: u32 = 0x83DE_0309;
pub const CMD_GR_SET_CTXSW_PREEMPTION_MODE: u32 = 0x2080_1210;
pub const CMD_TSG_SET_TIMESLICE: u32 = 0xA06C_0103;
pub const CMD_TSG_PREEMPT: u32 = 0xA06C_0105;
pub const CMD_CHANNEL_STOP: u32 = 0xA06F_0112;
pub const CMD_GPU_EVICT_CTX: u32 = 0x2080_012C;
/// `NV2080_CTRL_CMD_MC_SERVICE_INTERRUPTS` (ctrl2080mc.h:183), re-sent to GSP by the
/// guest kernel (intr.c:203-220). libcuda calls it only while it waits for GR work that
/// does not complete (never in the native traces).
pub const CMD_MC_SERVICE_INTERRUPTS: u32 = 0x2080_1702;
/// `NVOS55_PARAMETERS_v03_00` inside `rpc_dup_object_v03_00` (g_sdk-structures.h:368-377).
pub const DUP_H_CLIENT: usize = 0;
pub const DUP_H_PARENT: usize = 4;
pub const DUP_H_OBJECT: usize = 8;
pub const DUP_H_CLIENT_SRC: usize = 12;
pub const DUP_H_OBJECT_SRC: usize = 16;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ver::measured;

    const GSP_ABI: &str = include_str!("../../../../docs/caraxes/gsp-abi-610.md");

    /// Every hand-written GSP message offset against the compiler-measured tables.
    #[test]
    fn gsp_offsets_match_the_measured_abi() {
        let t = measured::parse(GSP_ABI);
        let field = |s: &str, f: &str| -> usize {
            *t.fields
                .get(&(s.to_string(), f.to_string()))
                .unwrap_or_else(|| panic!("{s}.{f} not in gsp-abi-610.md (a)"))
        };
        let size = |s: &str| -> usize {
            *t.sizes
                .get(s)
                .unwrap_or_else(|| panic!("{s} not in gsp-abi-610.md (a)"))
        };
        assert_eq!(size("GSP_MSG_QUEUE_ELEMENT"), ELEMENT_HEADER_SIZE);
        assert_eq!(size("rpc_message_header_v"), RPC_HEADER_SIZE);

        let c = "rpc_gsp_rm_control_v03_00";
        assert_eq!(field(c, "hClient"), RM_CONTROL_H_CLIENT);
        assert_eq!(field(c, "hObject"), RM_CONTROL_H_OBJECT);
        assert_eq!(field(c, "cmd"), RM_CONTROL_CMD);
        assert_eq!(field(c, "status"), RM_CONTROL_STATUS);
        assert_eq!(field(c, "paramsSize"), RM_CONTROL_PARAMS_SIZE);
        assert_eq!(field(c, "rmapiRpcFlags"), RM_CONTROL_RPC_FLAGS);
        assert_eq!(field(c, "rmctrlFlags"), RM_CONTROL_RMCTRL_FLAGS);
        assert_eq!(field(c, "params"), RM_CONTROL_PARAMS);

        let a = "rpc_gsp_rm_alloc_v03_00";
        assert_eq!(field(a, "hClient"), RM_ALLOC_H_CLIENT);
        assert_eq!(field(a, "hParent"), RM_ALLOC_H_PARENT);
        assert_eq!(field(a, "hObject"), RM_ALLOC_H_OBJECT);
        assert_eq!(field(a, "hClass"), RM_ALLOC_H_CLASS);
        assert_eq!(field(a, "status"), RM_ALLOC_STATUS);
        assert_eq!(field(a, "paramsSize"), RM_ALLOC_PARAMS_SIZE);
        assert_eq!(field(a, "flags"), RM_ALLOC_FLAGS);
        assert_eq!(field(a, "params"), RM_ALLOC_PARAMS);

        let f = "rpc_free_v03_00";
        assert_eq!(field(f, "params.hRoot"), FREE_H_ROOT);
        assert_eq!(field(f, "params.hObjectParent"), FREE_H_PARENT);
        assert_eq!(field(f, "params.hObjectOld"), FREE_H_OBJECT);
        assert_eq!(field(f, "params.status"), FREE_STATUS);
        assert_eq!(size(f), FREE_SIZE);
        assert_eq!(field("rpc_dup_object_v03_00", "params.status"), DUP_STATUS);
        assert_eq!(size("rpc_dup_object_v03_00"), DUP_SIZE);

        let g = "rpc_set_guest_system_info_v03_00";
        assert_eq!(field(g, "vgxVersionMajorNum"), GUEST_INFO_VGX_MAJOR);
        assert_eq!(field(g, "vgxVersionMinorNum"), GUEST_INFO_VGX_MINOR);
        assert_eq!(field(g, "guestDriverVersion"), GUEST_INFO_DRIVER_VERSION);

        assert_eq!(field("PACKED_REGISTRY_TABLE", "size"), REGISTRY_SIZE);
        assert_eq!(
            field("PACKED_REGISTRY_TABLE", "numEntries"),
            REGISTRY_NUM_ENTRIES
        );
        assert_eq!(field("PACKED_REGISTRY_TABLE", "entries"), REGISTRY_ENTRIES);
        let e = "PACKED_REGISTRY_ENTRY";
        assert_eq!(size(e), REGISTRY_ENTRY_SIZE);
        assert_eq!(field(e, "nameOffset"), REGISTRY_ENTRY_NAME_OFFSET);
        assert_eq!(field(e, "type"), REGISTRY_ENTRY_TYPE);
        assert_eq!(field(e, "data"), REGISTRY_ENTRY_DATA);
        assert_eq!(field(e, "length"), REGISTRY_ENTRY_LENGTH);

        let s = "GspSystemInfo";
        assert_eq!(field(s, "gpuPhysAddr"), SYSINFO_GPU_PHYS_ADDR);
        assert_eq!(field(s, "gpuPhysFbAddr"), SYSINFO_GPU_PHYS_FB_ADDR);
        assert_eq!(field(s, "gpuPhysInstAddr"), SYSINFO_GPU_PHYS_INST_ADDR);
        assert_eq!(field(s, "gpuPhysIoAddr"), SYSINFO_GPU_PHYS_IO_ADDR);
        assert_eq!(field(s, "nvDomainBusDeviceFunc"), SYSINFO_BDF);
        assert_eq!(field(s, "maxUserVa"), SYSINFO_MAX_USER_VA);
        assert_eq!(
            field(s, "pciConfigMirrorBase"),
            SYSINFO_PCI_CONFIG_MIRROR_BASE
        );
        assert_eq!(
            field(s, "pciConfigMirrorSize"),
            SYSINFO_PCI_CONFIG_MIRROR_SIZE
        );
        assert_eq!(field(s, "PCIDeviceID"), SYSINFO_PCI_DEVICE_ID);
        assert_eq!(field(s, "PCISubDeviceID"), SYSINFO_PCI_SUB_DEVICE_ID);
        assert_eq!(field(s, "PCIRevisionID"), SYSINFO_PCI_REVISION_ID);
        assert_eq!(field(s, "hostPageSize"), SYSINFO_HOST_PAGE_SIZE);

        let i = "GspStaticConfigInfo";
        assert_eq!(size(i), STATIC_INFO_SIZE);
        assert_eq!(field(i, "gidInfo.length"), SI_GID_LENGTH);
        assert_eq!(field(i, "gidInfo.data"), SI_GID_DATA);
        assert_eq!(
            field(i, "fbRegionInfoParams.numFBRegions"),
            SI_NUM_FB_REGIONS
        );
        assert_eq!(field(i, "fbRegionInfoParams.fbRegion"), SI_FB_REGIONS);
        assert_eq!(field(i, "engineCaps"), SI_ENGINE_CAPS);
        assert_eq!(field(i, "fb_length"), SI_FB_LENGTH);
        assert_eq!(field(i, "gpuNameString"), SI_GPU_NAME);
        assert_eq!(field(i, "gpuShortNameString"), SI_GPU_SHORT_NAME);
        assert_eq!(field(i, "bar1PdeBase"), SI_BAR1_PDE_BASE);
        assert_eq!(field(i, "bar2PdeBase"), SI_BAR2_PDE_BASE);
        assert_eq!(field(i, "bVbiosValid"), SI_VBIOS_VALID);
        assert_eq!(field(i, "vbiosSubVendor"), SI_VBIOS_SUB_VENDOR);
        assert_eq!(field(i, "vbiosSubDevice"), SI_VBIOS_SUB_DEVICE);
        assert_eq!(field(i, "vbiosRevision"), SI_VBIOS_REVISION);
        assert_eq!(field(i, "vbiosOemRevision"), SI_VBIOS_OEM_REVISION);
        assert_eq!(field(i, "hInternalClient"), SI_H_INTERNAL_CLIENT);
        assert_eq!(field(i, "hInternalDevice"), SI_H_INTERNAL_DEVICE);
        assert_eq!(field(i, "hInternalSubdevice"), SI_H_INTERNAL_SUBDEVICE);
        assert_eq!(field(i, "bBusResetRequired"), SI_BUS_RESET_REQUIRED);

        let r = "NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO";
        assert_eq!(size(r), FB_REGION_SIZE);
        assert_eq!(field(r, "base"), FB_REGION_BASE);
        assert_eq!(field(r, "limit"), FB_REGION_LIMIT);
        assert_eq!(field(r, "reserved"), FB_REGION_RESERVED);
        assert_eq!(field(r, "performance"), FB_REGION_PERFORMANCE);
        assert_eq!(field(r, "supportCompressed"), FB_REGION_SUPPORT_COMPRESSED);
        assert_eq!(field(r, "supportISO"), FB_REGION_SUPPORT_ISO);

        let w = "GspFwWprMeta";
        assert_eq!(size(w), WPR_META_SIZE);
        assert_eq!(field(w, "magic"), WPR_META_MAGIC_OFF);
        assert_eq!(field(w, "gspFwRsvdStart"), WPR_META_GSP_FW_RSVD_START);
        assert_eq!(field(w, "nonWprHeapOffset"), WPR_META_NON_WPR_HEAP_OFFSET);
        assert_eq!(field(w, "nonWprHeapSize"), WPR_META_NON_WPR_HEAP_SIZE);
        assert_eq!(field(w, "gspFwWprStart"), WPR_META_GSP_FW_WPR_START);
        assert_eq!(field(w, "frtsOffset"), WPR_META_FRTS_OFFSET);
        assert_eq!(field(w, "gspFwWprEnd"), WPR_META_GSP_FW_WPR_END);
        assert_eq!(field(w, "fbSize"), WPR_META_FB_SIZE);
        assert_eq!(
            field(w, "vgaWorkspaceOffset"),
            WPR_META_VGA_WORKSPACE_OFFSET
        );

        for (f, off) in [
            ("sizeOfRadix3Elf", WPR_META_SIZE_OF_RADIX3_ELF),
            ("sizeOfBootloader", WPR_META_SIZE_OF_BOOTLOADER),
            ("gspFwHeapOffset", WPR_META_GSP_FW_HEAP_OFFSET),
            ("gspFwHeapSize", WPR_META_GSP_FW_HEAP_SIZE),
            ("gspFwOffset", WPR_META_GSP_FW_OFFSET),
            ("bootBinOffset", WPR_META_BOOT_BIN_OFFSET),
            ("frtsSize", WPR_META_FRTS_SIZE),
            ("vgaWorkspaceSize", WPR_META_VGA_WORKSPACE_SIZE),
            ("pmuReservedSize", WPR_META_PMU_RESERVED_SIZE),
        ] {
            assert_eq!(field(w, f), off, "{f}");
        }
        let i = "GspStaticConfigInfo";
        assert_eq!(
            field(i, "fwWprLayoutOffset.nonWprHeapOffset"),
            SI_FW_WPR_NON_WPR_HEAP_OFFSET
        );
        assert_eq!(
            field(i, "fwWprLayoutOffset.frtsOffset"),
            SI_FW_WPR_FRTS_OFFSET
        );
        let fmc = "GSP_FMC_BOOT_PARAMS";
        assert_eq!(size(fmc), FMC_PARAMS_SIZE);
        assert_eq!(
            field(fmc, "bootGspRmParams.gspRmDescSize"),
            FMC_GSP_RM_DESC_SIZE
        );
        assert_eq!(
            field(fmc, "bootGspRmParams.gspRmDescOffset"),
            FMC_GSP_RM_DESC_OFFSET
        );
        assert_eq!(
            field(fmc, "gspRmParams.bootArgsOffset"),
            FMC_BOOT_ARGS_OFFSET
        );

        let l = "LibosMemoryRegionInitArgument";
        assert_eq!(size(l) as u64, LIBOS_ARG_SIZE);
        assert_eq!(field(l, "id8") as u64, LIBOS_ARG_ID8);
        assert_eq!(field(l, "pa") as u64, LIBOS_ARG_PA);
        assert_eq!(field(l, "size") as u64, LIBOS_ARG_REGION_SIZE);
        let m = "MESSAGE_QUEUE_INIT_ARGUMENTS";
        assert_eq!(field(m, "sharedMemPhysAddr") as u64, MQ_SHARED_MEM_PA);
        assert_eq!(
            field(m, "pageTableEntryCount") as u64,
            MQ_PAGE_TABLE_ENTRY_COUNT
        );
        assert_eq!(field(m, "cmdQueueOffset") as u64, MQ_CMD_QUEUE_OFFSET);
        assert_eq!(field(m, "statQueueOffset") as u64, MQ_STAT_QUEUE_OFFSET);
    }

    /// The control layouts computed from `nvrm_sys::v610`, pinned to the numbers the
    /// handlers were written against (a regeneration that moves one fails here).
    #[test]
    fn control_layouts_are_the_ones_the_handlers_use() {
        assert_eq!(
            (
                CHIP_INFO_SIZE,
                CHIP_INFO_PCI_DEVICE_ID,
                CHIP_INFO_PCI_SUB_DEVICE_ID
            ),
            (88, 12, 16)
        );
        assert_eq!((CHIP_INFO_PCI_REVISION_ID, CHIP_INFO_REG_BASES), (20, 24));
        assert_eq!(USER_REG_MAP_SIZE, 0);
        assert_eq!(
            (
                DEVINFO_PARAMS_SIZE,
                DEVINFO_NUM_ENTRIES,
                DEVINFO_B_MORE,
                DEVINFO_ENTRIES
            ),
            (12 + 32 * 100, 4, 8, 12)
        );
        assert_eq!(
            (DEVINFO_ENTRY_SIZE, DEVINFO_ENGINE_DATA, DEVINFO_PBDMA_IDS),
            (100, 0, 64)
        );
        assert_eq!(
            (
                DEVINFO_PBDMA_FAULT_IDS,
                DEVINFO_NUM_PBDMAS,
                DEVINFO_ENGINE_NAME
            ),
            (72, 80, 84)
        );
        assert_eq!((INTR_TABLE_SIZE, INTR_TABLE_LEN, INTR_TABLE), (2112, 0, 4));
        assert_eq!(
            (
                INTR_ENTRY_SIZE,
                INTR_ENTRY_ENGINE_IDX,
                INTR_ENTRY_VECTOR_STALL
            ),
            (16, 0, 8)
        );
        assert_eq!(
            (
                INTR_ENTRY_VECTOR_NONSTALL,
                INTR_SUBTREE_MAP,
                INTR_SUBTREE_MAP_ENTRY
            ),
            (12, 2056, 8)
        );
        assert_eq!(
            (
                MEMSYS_SIZE,
                MEMSYS_DISABLE_COMPBIT_BACKING,
                MEMSYS_L2_CACHE_SIZE
            ),
            (40, 3, 8)
        );
        assert_eq!(
            (
                MEMSYS_FBPA_PRESENT,
                MEMSYS_COMPR_PAGE_SIZE,
                MEMSYS_COMPR_PAGE_SHIFT
            ),
            (16, 20, 24)
        );
        assert_eq!(
            (MEMSYS_RAM_TYPE, MEMSYS_LTC_COUNT, MEMSYS_LTS_PER_LTC_COUNT),
            (28, 32, 36)
        );
        assert_eq!(
            (
                BAR_INFO_SIZE,
                BAR_INFO_COUNT,
                BAR_INFO_ENTRIES,
                BAR_INFO_ENTRY_SIZE
            ),
            (8 + 8 * 24, 0, 8, 24)
        );
        assert_eq!(
            (
                BAR_INFO_BAR_SIZE_MIB,
                BAR_INFO_BAR_SIZE_BYTES,
                BAR_INFO_BAR_OFFSET
            ),
            (4, 8, 16)
        );
        assert_eq!(RUSD_PHYS_ADDR, 8);
        assert_eq!(
            (
                GMMU_STATIC_SIZE,
                GMMU_REPLAYABLE_FAULT_BUFFER_SIZE,
                GMMU_NON_REPLAYABLE_FAULT_BUFFER_SIZE
            ),
            (16, 0, 8)
        );
        assert_eq!(
            (BIOS_INFO_LIST_SIZE, BIOS_INFO_LIST, BIOS_INFO_ENTRY_SIZE),
            (0, 4, 8)
        );
        assert_eq!((P2P_READ_CAPS_STATUS, P2P_WRITE_CAPS_STATUS), (1, 2));
        assert_eq!((NUM_CHANNELS_SIZE, NUM_CHANNELS), (8, 4));
        assert_eq!((INFO_LIST, CHIP_DETAILS_PCI_DEV_ID), (4, 0));
        assert_eq!(
            (
                FAULT_BUFFER_SIZE,
                FAULT_BUFFER_PTE_ARRAY,
                SHADOW_FAULT_BUFFER_QUEUE_PA
            ),
            (8, 16, 0)
        );
        assert_eq!(
            (
                SHADOW_FAULT_BUFFER_SIZE,
                ACCESS_CNTR_INDEX,
                ACCESS_CNTR_BUFFER_SIZE,
                ACCESS_CNTR_PTE_ARRAY
            ),
            (8, 0, 4, 8)
        );
        assert_eq!(
            (
                PDES_VIRT_ADDR_LO,
                PDES_VIRT_ADDR_HI,
                PDES_NUM_LEVELS,
                PDES_LEVELS,
                PDES_LEVEL_SIZE
            ),
            (16, 24, 32, 40, 24)
        );
        assert_eq!(
            (
                PDES_LEVEL_PHYS_ADDRESS,
                PDES_LEVEL_SIZE_FIELD,
                PDES_LEVEL_APERTURE,
                PDES_LEVEL_PAGE_SHIFT
            ),
            (0, 8, 16, 20)
        );
        assert_eq!((TSG_HVASPACE, TSG_ENGINE_TYPE), (8, 12));
        assert_eq!((MEM_DESC_BASE, MEM_DESC_ADDRESS_SPACE), (0, 16));
        assert_eq!(
            size_of::<ffi::NV2080_CTRL_GPU_GET_INFO_V2_PARAMS>(),
            580,
            "nvml.rs GET_INFO_V2"
        );
    }

    /// Class numbers against the bindings that have them.
    #[test]
    fn classes_match_the_bindings() {
        assert_eq!(NV01_ROOT, ffi::NV01_ROOT);
        assert_eq!(NV01_DEVICE_0, ffi::NV01_DEVICE_0);
        assert_eq!(NV20_SUBDEVICE_0, ffi::NV20_SUBDEVICE_0);
        assert_eq!(
            CMD_BUS_GET_PCI_BAR_INFO,
            ffi::NV2080_CTRL_CMD_BUS_GET_PCI_BAR_INFO
        );
        assert_eq!(nvrm_sys::DRIVER_VERSION, DRIVER_VERSION);
    }
}

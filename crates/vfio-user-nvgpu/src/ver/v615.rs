// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Driver 615.71.09 as the guest's driver (docs/caraxes/gsp-transport-615.md).
//!
//! Paths are relative to the open-gpu-kernel-modules tree at tag 615.71.09 (commit
//! 61dcc93); `sdk/` is `src/common/sdk/nvidia/inc`.
//!
//! The module is the 610.57.04 module (`pub use super::v610::*`) with the items that
//! differ overridden here; an item defined in this file shadows the glob import. Three
//! kinds of difference exist between the two versions for what the device speaks:
//!
//! 1. The CPU-GSP transport and the init handshake are new code, not a layout change:
//!    `GSP_TRANSPORT` selects `msgq::v2` and the GMCAPI init path (`gmcapi`) in the
//!    boot model. The transport and boot structures are `NV_ABI_STABLE` from 615 on
//!    (sdk/nvtypes.h:520-533), so their layouts live with the transport code, not
//!    here; the numbers below that repeat them are the ones generic code reads
//!    through `crate::ver::active` (the continuation cap, the RMARGS offsets).
//! 2. Seven control parameter structs changed (docs/caraxes/blackwell-gb206.md §3.4).
//!    The guest-facing ones are recomputed from `nvrm_sys::v615`; the test
//!    `versioned_layouts_are_overridden_where_they_differ` fails when a regenerated
//!    binding moves a layout this file does not override.
//! 3. The layouts of controls the device sends to the HOST are not this module's: they
//!    follow the running host driver (610.57.04 or 615.71.09), chosen at start-up by
//!    `crate::hostabi` (`GPU_GET_PES_INFO`, the public `GR_GET_GLOBAL_SM_ORDER`, the TSG
//!    allocation, the host-sized GSS legacy controls, the `FB_FLUSH_GPU_CACHE` flag
//!    bits). The rest of what the device sends to the host (the GR routed queries) is
//!    the same in both.
//!
//! `GspStaticConfigInfo` and `GspSystemInfo` are no longer on the wire in 615
//! (kernel_gsp.c:5314-5361). The device still builds its static info as the 610 struct
//! image (`rpc::build_static_info`, offsets `SI_*` from the 610 module) and encodes that
//! image as NVGMC_SC keys (`rpc::static_info_nvkv`), so one builder keeps the values and
//! their provenance labels for both versions.

use core::mem::{offset_of, size_of};

// The 615.71.09 bindings, named explicitly as in v610.rs.
use nvrm_sys::v615 as ffi;

pub use super::v610::*;

/// The display control vocabulary: the 610 one with the 615 changes of the NV0073 headers
/// (src/ver/v615/display.rs, docs/caraxes/display-615.md §3). Shadows the glob's `display`.
pub mod display;

/// `NV_VERSION_STRING` of this driver: what the guest announces in `SET_GUEST_SYSTEM_INFO`
/// and in the NVKV key `NVGMC_SI_KERNEL_DRIVER_VERSION_STRING` of `GMCAPI_CMD_GSP_INIT`.
pub const DRIVER_VERSION: &str = "615.71.09";

/// The guest's `ChannelAllocParams` layout (channel.rs), through `nvrm-sys`.
pub type RmAbi = nvrm_sys::V615;

/// Message queue v2, MCTP framing, GMCAPI init (docs/caraxes/gsp-transport-615.md §2-5).
pub const GSP_TRANSPORT: super::GspTransport = super::GspTransport::MsgqV2;

// ---------------------------------------------------------------------------------
// Transport numbers generic code reads (message_queue_priv.h:90-128).

/// `GSP_MSG_QUEUE_ELEMENT_SIZE_NO_ENCRYPTION`: mctpMagic, mctpPayloadSize, mctpHeader,
/// nvdmHeader, nvdmPayloadSize, reserved (message_queue_priv.h:90-128).
pub const ELEMENT_HEADER_SIZE: usize = crate::msgq::v2::ELEMENT_HEADER_SIZE;
/// `maxRpcSize` = queueElementSizeMax - queueElementHdrSize (message_queue_priv.h:180-184,
/// message_queue_cpu.c:85-86): 0x10000 - 24.
pub const MAX_RPC_SIZE: usize = crate::msgq::v2::MAX_RPC_SIZE;

/// `MESSAGE_QUEUE_INIT_ARGUMENTS` inside `GSP_ARGUMENTS_CACHED` (the RMARGS region), as
/// offsets from the start of RMARGS: the queue arguments moved behind a 16-byte header
/// (magic "GSP ", size, flags; gsp_init_args.h:33-113).
pub const MQ_SHARED_MEM_PA: u64 = crate::msgq::v2::ARGS_MQ_SHARED_MEM_PA;
pub const MQ_PAGE_TABLE_ENTRY_COUNT: u64 = crate::msgq::v2::ARGS_MQ_PAGE_TABLE_ENTRY_COUNT;
pub const MQ_CMD_QUEUE_OFFSET: u64 = crate::msgq::v2::ARGS_MQ_CMD_QUEUE_OFFSET;
pub const MQ_STAT_QUEUE_OFFSET: u64 = crate::msgq::v2::ARGS_MQ_STAT_QUEUE_OFFSET;

// ---------------------------------------------------------------------------------
// Controls.

/// `NV2080_CTRL_CMD_INTERNAL_GPU_GET_CHIP_INFO` does not exist in 615: its id 0x20800A36
/// is `NV2080_CTRL_CMD_INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO` now
/// (sdk/ctrl/ctrl2080/ctrl2080internal.h:572-585), and the chip sub-revision travels as
/// `NVGMC_SC_CHIP_SUB_REV` (interface/gmcapi/gmcapi_gsp_config.h:33). ctrl.rs dispatches
/// the id by [`CTRL_0A36`], so a fabric-probe call is never answered with chip info.
pub const CTRL_0A36: super::Id0A36 = super::Id0A36::SyncFabricProbeInfo;
/// The id 0x20800A36 carries in 615 (the same number as 610's
/// `CMD_INTERNAL_GPU_GET_CHIP_INFO`, which this module inherits).
pub const CMD_INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO: u32 = 0x2080_0A36;
/// `NV2080_CTRL_CMD_INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO_PARAMS` {linkMaskToBeReduced}: the
/// kernel pushes the fabric manager's NVLink mask to GSP (src/kernel/gpu/gpu_fabric_probe.c:
/// 1912-1943) and only logs a failure.
pub const FABRIC_PROBE_INFO_SIZE: usize =
    size_of::<ffi::NV2080_CTRL_CMD_INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO_PARAMS>();

/// `NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS` (sdk/ctrl/ctrl2080/ctrl2080fifo.h):
/// `engineData` grew from 16 to 17 words (`ENGINE_INFO_TYPE_IS_PREEMPTABLE`,
/// src/nvidia/inc/kernel/gpu/fifo/engine_info.h:98-103, valid only for esched-driven
/// engines; 0 on Turing). The indices of the other data types are unchanged.
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
pub const DEVINFO_ENGINE_DATA: usize = offset_of!(ffi::NV2080_CTRL_FIFO_DEVICE_ENTRY, engineData);
pub const DEVINFO_PBDMA_IDS: usize = offset_of!(ffi::NV2080_CTRL_FIFO_DEVICE_ENTRY, pbdmaIds);
pub const DEVINFO_PBDMA_FAULT_IDS: usize =
    offset_of!(ffi::NV2080_CTRL_FIFO_DEVICE_ENTRY, pbdmaFaultIds);
pub const DEVINFO_NUM_PBDMAS: usize = offset_of!(ffi::NV2080_CTRL_FIFO_DEVICE_ENTRY, numPbdmas);
pub const DEVINFO_ENGINE_NAME: usize = offset_of!(ffi::NV2080_CTRL_FIFO_DEVICE_ENTRY, engineName);

/// `NV2080_CTRL_INTERNAL_GET_PCIE_P2P_CAPS_PARAMS`: `bSkipChipsetCheck` appended (3 to 4
/// bytes); the two status bytes the handler writes stay where they were.
pub const P2P_READ_CAPS_STATUS: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GET_PCIE_P2P_CAPS_PARAMS,
    p2pReadCapsStatus
);
pub const P2P_WRITE_CAPS_STATUS: usize = offset_of!(
    ffi::NV2080_CTRL_INTERNAL_GET_PCIE_P2P_CAPS_PARAMS,
    p2pWriteCapsStatus
);

/// NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS (sdk/ctrl/ctrl2080/ctrl2080internal.h):
/// grew from 1336 to 1356 bytes per engine; the fields move.
pub const FS_ELEM: usize = size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS>();
pub const FS_PARAMS: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_FLOORSWEEPING_MASKS_PARAMS>();
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
/// `tpcToPesMap[15]` in 615 (ten in 610).
pub const FS_TPC_TO_PES_MAP_LEN: usize = 15;
const _: () = assert!(FS_TPC_TO_PES_MAP + 4 * FS_TPC_TO_PES_MAP_LEN == FS_NUM_PES_PER_GPC);
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

/// NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER: `virtualDpcId` inserted after
/// `virtualGpcId` in every SM entry (ten u16 instead of nine). kgr.rs translates the host's
/// public struct into it field by name, whichever host driver runs (hostabi.rs
/// `translate_sm_order`); on a 610 host `virtualDpcId` stays 0 (no DPC before Blackwell).
pub const SM_ORDER_GUEST: crate::hostabi::SmOrderLayout = crate::hostabi::sm_order_layout!(
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER,
    ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER__bindgen_ty_1;
    gpcId, localTpcId, localSmId, globalTpcId, virtualGpcId, virtualDpcId,
    migratableTpcId, ugpuId, physicalCpcId, virtualTpcId
);
pub const SM_ENTRY: usize = SM_ORDER_GUEST.entry_size;
pub const SM_ORDER_ELEM: usize = SM_ORDER_GUEST.size;
pub const SM_ORDER_NUM_SM: usize = SM_ORDER_GUEST.num_sm;
pub const SM_ORDER_NUM_TPC: usize = SM_ORDER_GUEST.num_tpc;
const _: () = assert!(SM_ORDER_GUEST.fields.len() * 2 == SM_ENTRY);
pub const SM_ORDER_PARAMS: usize =
    size_of::<ffi::NV2080_CTRL_INTERNAL_STATIC_GR_GET_GLOBAL_SM_ORDER_PARAMS>();

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ver::measured;

    const TRANSPORT_DOC: &str = include_str!("../../../../docs/caraxes/gsp-transport-615.md");

    /// The RPC payload layouts generic code shares with 610 are the 615 ones too, and the
    /// RMARGS offsets are the measured `GSP_ARGUMENTS_CACHED` ones (the compiler tables
    /// in appendix A of docs/caraxes/gsp-transport-615.md).
    #[test]
    fn gsp_offsets_match_the_measured_615_abi() {
        let t = measured::parse(TRANSPORT_DOC);
        let field = |s: &str, f: &str| -> usize {
            *t.fields
                .get(&(s.to_string(), f.to_string()))
                .unwrap_or_else(|| panic!("{s}.{f} not in gsp-transport-615.md appendix A"))
        };
        let size = |s: &str| -> usize {
            *t.sizes
                .get(s)
                .unwrap_or_else(|| panic!("{s} not in gsp-transport-615.md appendix A"))
        };
        assert_eq!(
            field("GSP_MSG_QUEUE_ELEMENT", "noEncryption.payload"),
            ELEMENT_HEADER_SIZE
        );
        assert_eq!(size("rpc_message_header_v"), RPC_HEADER_SIZE);
        let c = "rpc_gsp_rm_control_v03_00";
        assert_eq!(field(c, "hClient"), RM_CONTROL_H_CLIENT);
        assert_eq!(field(c, "hObject"), RM_CONTROL_H_OBJECT);
        assert_eq!(field(c, "cmd"), RM_CONTROL_CMD);
        assert_eq!(field(c, "status"), RM_CONTROL_STATUS);
        assert_eq!(field(c, "paramsSize"), RM_CONTROL_PARAMS_SIZE);
        assert_eq!(field(c, "rmctrlFlags"), RM_CONTROL_RMCTRL_FLAGS);
        assert_eq!(field(c, "params"), RM_CONTROL_PARAMS);
        let a = "rpc_gsp_rm_alloc_v03_00";
        assert_eq!(field(a, "hClient"), RM_ALLOC_H_CLIENT);
        assert_eq!(field(a, "hParent"), RM_ALLOC_H_PARENT);
        assert_eq!(field(a, "hObject"), RM_ALLOC_H_OBJECT);
        assert_eq!(field(a, "hClass"), RM_ALLOC_H_CLASS);
        assert_eq!(field(a, "status"), RM_ALLOC_STATUS);
        assert_eq!(field(a, "paramsSize"), RM_ALLOC_PARAMS_SIZE);
        assert_eq!(field(a, "params"), RM_ALLOC_PARAMS);
        assert_eq!(size("rpc_free_v03_00"), FREE_SIZE);
        assert_eq!(field("rpc_free_v03_00", "params.status"), FREE_STATUS);
        assert_eq!(size("rpc_dup_object_v03_00"), DUP_SIZE);
        assert_eq!(field("rpc_dup_object_v03_00", "params.status"), DUP_STATUS);
        let g = "rpc_set_guest_system_info_v03_00";
        assert_eq!(field(g, "guestDriverVersion"), GUEST_INFO_DRIVER_VERSION);
        let w = "GspFwWprMeta";
        assert_eq!(size(w), WPR_META_SIZE);
        assert_eq!(field(w, "gspFwRsvdStart"), WPR_META_GSP_FW_RSVD_START);
        assert_eq!(field(w, "nonWprHeapOffset"), WPR_META_NON_WPR_HEAP_OFFSET);
        assert_eq!(field(w, "frtsOffset"), WPR_META_FRTS_OFFSET);
        assert_eq!(field(w, "fbSize"), WPR_META_FB_SIZE);
        assert_eq!(
            field(w, "vgaWorkspaceOffset"),
            WPR_META_VGA_WORKSPACE_OFFSET
        );
        let l = "LibosMemoryRegionInitArgument";
        assert_eq!(size(l) as u64, LIBOS_ARG_SIZE);
        assert_eq!(field(l, "pa") as u64, LIBOS_ARG_PA);
        let r = "GSP_ARGUMENTS_CACHED";
        assert_eq!(
            field(r, "messageQueueInitArguments.sharedMemPhysAddr") as u64,
            MQ_SHARED_MEM_PA
        );
        assert_eq!(
            field(r, "messageQueueInitArguments.pageTableEntryCount") as u64,
            MQ_PAGE_TABLE_ENTRY_COUNT
        );
        assert_eq!(
            field(r, "messageQueueInitArguments.cmdQueueOffset") as u64,
            MQ_CMD_QUEUE_OFFSET
        );
        assert_eq!(
            field(r, "messageQueueInitArguments.statQueueOffset") as u64,
            MQ_STAT_QUEUE_OFFSET
        );
    }

    /// Every layout the 610 module computes from a versioned binding is either the same
    /// in 615 or overridden here. `stable` types are identical in every version by
    /// construction (nvrm-sys/src/v615.rs:4-12), so only the per-version ones are listed.
    #[test]
    fn versioned_layouts_are_overridden_where_they_differ() {
        use core::mem::size_of as s;
        use nvrm_sys::{v610 as a, v615 as b};
        macro_rules! same {
            ($($t:ident),* $(,)?) => {$(
                assert_eq!(s::<a::$t>(), s::<b::$t>(), concat!(stringify!($t), " changed in 615: override it"));
            )*};
        }
        same!(
            NV2080_CTRL_GPU_GET_INFO_V2_PARAMS,
            NV2080_CTRL_GR_GET_INFO_V2_PARAMS,
            NV2080_CTRL_INTERNAL_STATIC_GR_INFO,
            NV2080_CTRL_INTERNAL_STATIC_GR_GET_INFO_PARAMS,
            NV2080_CTRL_INTERNAL_STATIC_GR_GET_PPC_MASKS_PARAMS,
            NV2080_CTRL_CMD_FB_GET_FB_REGION_INFO_PARAMS,
        );
        // The TSG allocation grew at its end (`reserved` and `internalFlags` appended, 20
        // to 28 bytes);
        // the two fields the device reads stay where they were.
        assert_eq!(
            (
                s::<a::NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS>(),
                s::<b::NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS>()
            ),
            (20, 28)
        );
        assert_eq!(
            (TSG_HVASPACE, TSG_ENGINE_TYPE),
            (
                offset_of!(b::NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS, hVASpace),
                offset_of!(b::NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS, engineType)
            )
        );
        // The seven of docs/caraxes/blackwell-gb206.md §3.4 that the device answers or
        // queries, each either overridden above or deliberately kept as the host's.
        assert_eq!(
            (
                s::<a::NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS>(),
                DEVINFO_PARAMS_SIZE
            ),
            (3212, 3340)
        );
        assert_eq!(
            (DEVINFO_ENTRY_SIZE, DEVINFO_ENGINE_DATA, DEVINFO_PBDMA_IDS),
            (104, 0, 68)
        );
        assert_eq!((DEVINFO_NUM_PBDMAS, DEVINFO_ENGINE_NAME), (84, 88));
        // The two host queries (PES info 56/76, public SM order 9240/10264) follow the
        // host driver: hostabi.rs pins both versions.
        assert_eq!(SM_ENTRY, 20);
        assert_eq!(SM_ORDER_GUEST.fields[5], ("virtualDpcId", 10));
        assert_eq!(
            (SM_ORDER_ELEM, SM_ORDER_NUM_SM, SM_ORDER_NUM_TPC, SM_ORDER_PARAMS),
            (10244, 10240, 10242, 81952)
        );
        assert_eq!(
            SM_ORDER_GUEST,
            crate::hostabi::guest_layouts::GUEST_SM_ORDER_615
        );
        assert_eq!(FS_PARAMS, 10848);
        assert_eq!((FS_TPC_TO_PES_MAP_LEN, FS_NUM_PES_PER_GPC - FS_TPC_TO_PES_MAP), (15, 60));
        assert_eq!(s::<b::NV2080_CTRL_INTERNAL_GET_PCIE_P2P_CAPS_PARAMS>(), 4);
        assert_eq!((P2P_READ_CAPS_STATUS, P2P_WRITE_CAPS_STATUS), (1, 2));
        assert_eq!(
            s::<b::NV2080_CTRL_INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO_PARAMS>(),
            1072
        );
    }

    /// The reused id is not answered as chip info (docs/caraxes/blackwell-gb206.md §3.4).
    #[test]
    fn the_reused_chip_info_id_is_not_chip_info() {
        assert_eq!(CTRL_0A36, crate::ver::Id0A36::SyncFabricProbeInfo);
        assert_eq!(super::super::v610::CTRL_0A36, crate::ver::Id0A36::GetChipInfo);
        assert_eq!(
            super::super::v610::CMD_INTERNAL_GPU_GET_CHIP_INFO,
            CMD_INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO
        );
        assert_eq!(FABRIC_PROBE_INFO_SIZE, 16);
        assert_eq!(DRIVER_VERSION, nvrm_sys::DriverVersion::V615.as_str());
        assert_eq!(GSP_TRANSPORT.guest_abi(), 615);
    }
}

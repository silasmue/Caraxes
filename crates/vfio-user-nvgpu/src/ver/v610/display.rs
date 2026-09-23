// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Driver 610.57.04: the display control vocabulary (docs/caraxes/display-adapter.md
//! section 1.3, docs/caraxes/display-ab.md). Control numbers, parameter sizes and field
//! offsets of the NV0073, NV5070, NVC370 and NVC372 controls and of the internal NV2080
//! display controls the device answers.
//!
//! `nvrm-sys` carries none of these structs except `GET_STATIC_INFO` and
//! `INIT_BRIGHTC_STATE_LOAD`, so the offsets are hand-written from the SDK headers
//! (`sdk/ctrl/ctrl0073/*.h`, `ctrl5070/*.h`, `ctrlc370/*.h`, `ctrlc372/*.h`,
//! `ctrl2080/ctrl2080internal.h`, `class/cl5070.h`) and checked in the tests against
//! the table `display-layouts.txt`, which gcc measured from those headers (x86_64, the
//! guest ABI; the generator is `caraxes/runs/display-ab-2026-09-23/abi/gen.py`). Every
//! display control travels as its flat C struct (no FINN serialization,
//! display-adapter.md section 1.1).

// ---------------------------------------------------------------------------------
// Classes (sdk/class/*.h).

pub const NV04_DISPLAY_COMMON: u32 = 0x0073;
pub const NVC570_DISPLAY: u32 = 0xC570;
pub const NVC57A_CURSOR_IMM_CHANNEL_PIO: u32 = 0xC57A;
pub const NVC57B_WINDOW_IMM_CHANNEL_DMA: u32 = 0xC57B;
pub const NVC57D_CORE_CHANNEL_DMA: u32 = 0xC57D;
pub const NVC57E_WINDOW_CHANNEL_DMA: u32 = 0xC57E;
pub const NVC573_DISP_CAPABILITIES: u32 = 0xC573;
pub const NVC372_DISPLAY_SW: u32 = 0xC372;
pub const NVC371_DISP_SF_USER: u32 = 0xC371;

// ---------------------------------------------------------------------------------
// Internal NV2080 display controls (ctrl2080internal.h), kernel RM to GSP.

pub const CMD_INTERNAL_DISPLAY_GET_STATIC_INFO: u32 = 0x2080_0A01;
pub const CMD_INTERNAL_DISPLAY_WRITE_INST_MEM: u32 = 0x2080_0A49;
pub const CMD_INTERNAL_DISPLAY_GET_IP_VERSION: u32 = 0x2080_0A4B;
pub const CMD_INTERNAL_DISPLAY_SETUP_RG_LINE_INTR: u32 = 0x2080_0A4D;
pub const CMD_INTERNAL_DISPLAY_SET_IMP_INIT_INFO: u32 = 0x2080_0A54;
pub const CMD_INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER: u32 = 0x2080_0A58;
pub const CMD_INTERNAL_DISPLAY_GET_ACTIVE_DISPLAY_DEVICES: u32 = 0x2080_0A5D;
pub const CMD_INTERNAL_DISPLAY_PRE_UNIX_CONSOLE: u32 = 0x2080_0A76;
pub const CMD_INTERNAL_DISPLAY_POST_UNIX_CONSOLE: u32 = 0x2080_0A77;
pub const CMD_INTERNAL_INIT_BRIGHTC_STATE_LOAD: u32 = 0x2080_0AC6;
pub const CMD_INTERNAL_SET_STATIC_EDID_DATA: u32 = 0x2080_0ADF;
pub const CMD_INTERNAL_DISPLAY_ACPI_SUBSYSTEM_ACTIVATED: u32 = 0x2080_0AF0;
pub const CMD_INTERNAL_DISPLAY_PRE_MODESET: u32 = 0x2080_0AF1;
pub const CMD_INTERNAL_DISPLAY_POST_MODESET: u32 = 0x2080_0AF2;

/// `NV2080_CTRL_INTERNAL_DISPLAY_GET_STATIC_INFO_PARAMS` (32 bytes).
pub const STATIC_INFO_SIZE: usize = 32;
pub const STATIC_INFO_FE_HW_SYS_CAP: usize = 0;
pub const STATIC_INFO_WINDOW_PRESENT_MASK: usize = 4;
pub const STATIC_INFO_NUM_HEADS: usize = 8;
pub const STATIC_INFO_I2C_PORT: usize = 12;
pub const STATIC_INFO_INTERNAL_DISP_ACTIVE_MASK: usize = 16;
pub const STATIC_INFO_EMBEDDED_DP_MASK: usize = 20;
pub const STATIC_INFO_NUM_DISP_CHANNELS: usize = 28;
/// `..._WRITE_INST_MEM_PARAMS` (24 bytes).
pub const WRITE_INST_MEM_SIZE: usize = 24;
pub const WRITE_INST_MEM_PHYS_ADDR: usize = 0;
pub const WRITE_INST_MEM_SIZE_FIELD: usize = 8;
pub const WRITE_INST_MEM_ADDR_SPACE: usize = 16;
/// `..._GET_IP_VERSION_PARAMS` (4 bytes).
pub const IP_VERSION_SIZE: usize = 4;
/// `..._CHANNEL_PUSHBUFFER_PARAMS` (56 bytes).
pub const PUSHBUFFER_SIZE: usize = 56;
pub const PUSHBUFFER_ADDRESS_SPACE: usize = 0;
pub const PUSHBUFFER_PHYSICAL_ADDR: usize = 8;
pub const PUSHBUFFER_LIMIT: usize = 16;
pub const PUSHBUFFER_HCLASS: usize = 28;
pub const PUSHBUFFER_CHANNEL_INSTANCE: usize = 32;
pub const PUSHBUFFER_VALID: usize = 36;
pub const PUSHBUFFER_CHANNEL_PB_SIZE: usize = 44;
pub const SET_IMP_INIT_INFO_SIZE: usize = 492;
pub const BRIGHTC_STATE_LOAD_SIZE: usize = 4104;
pub const SET_STATIC_EDID_DATA_SIZE: usize = 8388;

// ---------------------------------------------------------------------------------
// NV0073 (NV04_DISPLAY_COMMON): ctrl0073system.h, ctrl0073specific.h, ctrl0073dfp.h,
// ctrl0073dp.h, ctrl0073internal.h.

pub const CMD_SYSTEM_GET_CAPS_V2: u32 = 0x73_0101;
pub const CMD_SYSTEM_GET_NUM_HEADS: u32 = 0x73_0102;
pub const CMD_SYSTEM_GET_SCANLINE: u32 = 0x73_0104;
pub const CMD_SYSTEM_GET_SUPPORTED: u32 = 0x73_0107;
pub const CMD_SYSTEM_GET_CONNECT_STATE: u32 = 0x73_0108;
pub const CMD_SYSTEM_GET_HEAD_ROUTING_MAP: u32 = 0x73_010B;
pub const CMD_SYSTEM_GET_ACTIVE: u32 = 0x73_010C;
pub const CMD_SYSTEM_GET_INTERNAL_DISPLAYS: u32 = 0x73_0116;
pub const CMD_SYSTEM_GET_BOOT_DISPLAYS: u32 = 0x73_011E;
pub const CMD_SYSTEM_INTERNAL_ALLOCATE_DISPLAY_BANDWIDTH: u32 = 0x73_0157;
pub const CMD_SYSTEM_MAP_SHARED_DATA: u32 = 0x73_0151;
pub const CMD_SPECIFIC_GET_I2C_PORTID: u32 = 0x73_0211;
pub const CMD_SPECIFIC_GET_TYPE: u32 = 0x73_0240;
pub const CMD_SPECIFIC_GET_EDID_V2: u32 = 0x73_0245;
pub const CMD_SPECIFIC_SET_EDID_V2: u32 = 0x73_0246;
pub const CMD_SPECIFIC_GET_CONNECTOR_DATA: u32 = 0x73_0250;
pub const CMD_SPECIFIC_GET_HDCP_STATE: u32 = 0x73_0280;
pub const CMD_SPECIFIC_GET_ALL_HEAD_MASK: u32 = 0x73_0287;
pub const CMD_SPECIFIC_GET_PCLK_LIMIT: u32 = 0x73_028A;
pub const CMD_SPECIFIC_OR_GET_INFO: u32 = 0x73_028B;
pub const CMD_SPECIFIC_SET_MONITOR_POWER: u32 = 0x73_0295;
pub const CMD_SPECIFIC_APPLY_EDID_OVERRIDE_V2: u32 = 0x73_02A1;
pub const CMD_SPECIFIC_DISPLAY_CHANGE: u32 = 0x73_02A4;
pub const CMD_SPECIFIC_IS_DIRECTMODE_DISPLAY: u32 = 0x73_02A7;
pub const CMD_SPECIFIC_GET_VALID_HEAD_WINDOW_ASSIGNMENT: u32 = 0x73_02AD;
pub const CMD_INTERNAL_GET_HOTPLUG_UNPLUG_STATE: u32 = 0x73_0401;
pub const CMD_DFP_GET_INFO: u32 = 0x73_1140;
pub const CMD_DFP_SET_ELD_AUDIO_CAPS: u32 = 0x73_1144;
pub const CMD_DFP_UPDATE_DYNAMIC_DFP_CACHE: u32 = 0x73_114E;
pub const CMD_DFP_SET_AUDIO_ENABLE: u32 = 0x73_1150;
pub const CMD_DFP_ASSIGN_SOR: u32 = 0x73_1152;
pub const CMD_DP_AUXCH_CTRL: u32 = 0x73_1341;
pub const CMD_DP_CTRL: u32 = 0x73_1343;
pub const CMD_DP_MAIN_LINK_CTRL: u32 = 0x73_1356;
pub const CMD_DP_SET_AUDIO_MUTESTREAM: u32 = 0x73_1359;
pub const CMD_DP_GET_LINK_CONFIG: u32 = 0x73_1360;
pub const CMD_DP_CONFIG_STREAM: u32 = 0x73_1362;
pub const CMD_DP_SET_MANUAL_DISPLAYPORT: u32 = 0x73_1365;
pub const CMD_DP_GET_CAPS: u32 = 0x73_1369;
pub const CMD_DP_NOTIFY_LT: u32 = 0x73_138F;
pub const CMD_DP_SET_STEREO_MSA_PROPERTIES: u32 = 0x73_1378;
pub const CMD_DP_SET_MSA_PROPERTIES_V2: u32 = 0x73_1381;
pub const CMD_SPECIFIC_SET_OD_PACKET: u32 = 0x73_0288;
pub const CMD_SYSTEM_VRR_DISPLAY_INFO: u32 = 0x73_012C;
pub const CMD_SPECIFIC_GET_BACKLIGHT_BRIGHTNESS: u32 = 0x73_0291;
pub const CMD_SYSTEM_QUERY_DISPLAY_IDS_WITH_MUX: u32 = 0x73_013D;
/// `NV2080_CTRL_CMD_LPWR_DIFR_CTRL` (ctrl2080lpwr.h:42): display idle frame refresh.
pub const CMD_LPWR_DIFR_CTRL: u32 = 0x2080_2801;

/// `NV0073_CTRL_SYSTEM_GET_CAPS_V2_PARAMS`: `capsTbl[2]`.
pub const SYSTEM_CAPS_TBL_SIZE: usize = 2;
/// Byte 1 bit 3 (`ctrl0073system.h:61`).
pub const SYSTEM_CAPS_CROSS_BAR_SUPPORTED: (usize, u8) = (1, 0x08);
pub const GET_NUM_HEADS_NUM_HEADS: usize = 8;
pub const GET_SUPPORTED_DISPLAY_MASK: usize = 4;
pub const GET_SUPPORTED_DISPLAY_MASK_DDC: usize = 8;
pub const CONNECT_STATE_DISPLAY_MASK: usize = 8;
pub const CONNECT_STATE_RETRY_TIME_MS: usize = 12;
pub const HEAD_ROUTING_DISPLAY_MASK: usize = 4;
pub const HEAD_ROUTING_MAP: usize = 16;
pub const GET_ACTIVE_HEAD: usize = 4;
pub const GET_ACTIVE_DISPLAY_ID: usize = 12;
pub const GET_SCANLINE_CURRENT: usize = 8;
pub const HOTPLUG_UNPLUG_PLUG_MASK: usize = 8;
pub const HOTPLUG_UNPLUG_UNPLUG_MASK: usize = 12;
/// `NV0073_CTRL_SPECIFIC_*`: `displayId` sits at 4 in every per-display struct.
pub const DISPLAY_ID: usize = 4;
pub const I2C_PORTID_COMM_PORT_ID: usize = 8;
pub const I2C_PORTID_DDC_PORT_ID: usize = 12;
pub const GET_TYPE_DISPLAY_TYPE: usize = 8;
/// `NV0073_CTRL_SPECIFIC_DISPLAY_TYPE_DFP`.
pub const DISPLAY_TYPE_DFP: u32 = 2;
/// `..._GET_EDID_V2_PARAMS` (2064) and `..._SET_EDID_V2_PARAMS` (2060).
pub const GET_EDID_V2_SIZE: usize = 2064;
pub const GET_EDID_BUFFER_SIZE: usize = 8;
pub const GET_EDID_FLAGS: usize = 12;
pub const GET_EDID_BUFFER: usize = 16;
pub const SET_EDID_V2_SIZE: usize = 2060;
pub const SET_EDID_BUFFER_SIZE: usize = 8;
pub const SET_EDID_BUFFER: usize = 12;
/// `NV0073_CTRL_SPECIFIC_GET_EDID_MAX_EDID_BYTES`.
pub const EDID_MAX_BYTES: usize = 2048;
/// `GET_EDID_V2` flags: `COPY_CACHE` bit 0, `READ_MODE` bit 1 (1 = raw).
pub const GET_EDID_FLAGS_COPY_CACHE_YES: u32 = 1;
pub const GET_EDID_FLAGS_READ_MODE_RAW: u32 = 2;
/// `..._GET_CONNECTOR_DATA_PARAMS` (72): flags 8, DDCPartners 12, count 16, `data[4]` of
/// {index, type, location} at 20, platform 68.
pub const CONNECTOR_DATA_DDC_PARTNERS: usize = 12;
pub const CONNECTOR_DATA_COUNT: usize = 16;
pub const CONNECTOR_DATA_DATA: usize = 20;
pub const CONNECTOR_DATA_ENTRY_SIZE: usize = 12;
pub const CONNECTOR_DATA_PLATFORM: usize = 68;
/// `NV0073_CTRL_SPECIFIC_CONNECTOR_DATA_TYPE_DP_EXT`.
pub const CONNECTOR_TYPE_DP_EXT: u32 = 0x46;
pub const HDCP_STATE_FLAGS: usize = 8;
pub const ALL_HEAD_MASK_HEAD_MASK: usize = 4;
pub const PCLK_LIMIT_PCLK: usize = 8;
pub const PCLK_LIMIT_OR_PCLK: usize = 12;
pub const PCLK_LIMIT_VB_PCLK: usize = 16;
/// `..._OR_GET_INFO_PARAMS` (56).
pub const OR_INFO_INDEX: usize = 8;
pub const OR_INFO_TYPE: usize = 12;
pub const OR_INFO_PROTOCOL: usize = 16;
pub const OR_INFO_LOCATION: usize = 28;
pub const OR_INFO_DCB_INDEX: usize = 36;
pub const OR_INFO_IS_LIT_BY_VBIOS: usize = 48;
pub const OR_INFO_IS_DISP_DYNAMIC: usize = 49;
pub const OR_TYPE_SOR: u32 = 2;
pub const OR_PROTOCOL_SOR_DP_A: u32 = 8;
pub const OR_LOCATION_CHIP: u32 = 0;
pub const DIRECTMODE_IS_DIRECTMODE: usize = 4;
/// `..._GET_VALID_HEAD_WINDOW_ASSIGNMENT_PARAMS` (36): `windowHeadMask[32]` of NvU8.
pub const WINDOW_HEAD_MASK: usize = 4;
pub const WINDOW_HEAD_MASK_COUNT: usize = 32;
/// `NV0073_CTRL_DFP_GET_INFO_PARAMS` (16): flags 8, UHBRSupportedByDfp 12.
pub const DFP_INFO_FLAGS: usize = 8;
/// `NV0073_CTRL_DFP_FLAGS_*` (ctrl0073dfp.h:132-201).
pub const DFP_FLAGS_SIGNAL_DISPLAYPORT: u32 = 3;
pub const DFP_FLAGS_LANE_SHIFT: u32 = 3;
pub const DFP_FLAGS_LANE_QUAD: u32 = 3;
pub const DFP_FLAGS_DP_LINK_BW_SHIFT: u32 = 17;
pub const DFP_FLAGS_LINK_SHIFT: u32 = 20;
pub const DFP_FLAGS_LINK_SINGLE: u32 = 1;
/// `NV0073_CTRL_DFP_ASSIGN_SOR_PARAMS` (80): `sorAssignList[4]` at 24 (u32 display masks),
/// `sorAssignListWithTag[4]` of {displayMask, sorType} at 40.
pub const ASSIGN_SOR_SOR_EXCLUDE_MASK: usize = 8;
pub const ASSIGN_SOR_LIST: usize = 24;
pub const ASSIGN_SOR_LIST_WITH_TAG: usize = 40;
pub const ASSIGN_SOR_TAG_SIZE: usize = 8;
/// `NV0073_CTRL_DFP_SOR_TYPE_SINGLE`.
pub const SOR_TYPE_SINGLE: u32 = 1;
/// `NV0073_CTRL_DP_AUXCH_CTRL_PARAMS` (48).
pub const AUX_SIZE: usize = 48;
pub const AUX_ADDR_ONLY: usize = 8;
pub const AUX_CMD: usize = 12;
pub const AUX_ADDR: usize = 16;
pub const AUX_DATA: usize = 20;
pub const AUX_DATA_LEN: usize = 16;
pub const AUX_SIZE_FIELD: usize = 36;
pub const AUX_REPLY_TYPE: usize = 40;
pub const AUX_RETRY_TIME_MS: usize = 44;
/// `NV0073_CTRL_DP_AUXCH_REPLYTYPE_INVALID_ARGUMENT`.
pub const AUX_REPLYTYPE_INVALID_ARGUMENT: u32 = 0xFFFF_FFFF;
/// `NV0073_CTRL_DP_CTRL_PARAMS` (28): cmd 8, data 12, err 16, retryTimeMs 20.
pub const DP_CTRL_CMD: usize = 8;
pub const DP_CTRL_DATA: usize = 12;
pub const DP_CTRL_ERR: usize = 16;
/// `NV0073_CTRL_DP_DATA_SET_LANE_COUNT` 4:0, `_SET_LINK_BW` 15:8.
pub const DP_DATA_LANE_COUNT_MASK: u32 = 0x1F;
pub const DP_DATA_LINK_BW_SHIFT: u32 = 8;
/// `NV0073_CTRL_DP_GET_LINK_CONFIG_PARAMS` (24).
pub const LINK_CONFIG_LANE_COUNT: usize = 8;
pub const LINK_CONFIG_LINK_BW: usize = 12;
pub const LINK_CONFIG_DP2_LINK_BW: usize = 16;
pub const LINK_CONFIG_FEC_ENABLED: usize = 20;
/// `NV0073_CTRL_CMD_DP_GET_CAPS_PARAMS` (64).
pub const DP_CAPS_SIZE: usize = 64;
pub const DP_CAPS_MAX_LINK_RATE: usize = 8;
pub const DP_CAPS_DP_VERSIONS: usize = 12;
pub const DP_CAPS_MULTISTREAM: usize = 20;
pub const DP_CAPS_FEC: usize = 25;
pub const DP_CAPS_DOWNSPREAD: usize = 29;
pub const DP_CAPS_DSC_SUPPORTED: usize = 36;
pub const DP_CAPS_DSC_COLOR_FORMAT_MASK: usize = 40;
pub const DP_CAPS_DSC_LINE_BUFFER_KB: usize = 44;
pub const DP_CAPS_DSC_RATE_BUFFER_KB: usize = 48;
pub const DP_CAPS_DSC_BPP_PRECISION: usize = 52;
pub const DP_CAPS_DSC_MAX_HZT_SLICES: usize = 56;
pub const DP_CAPS_DSC_LINE_BUFFER_BIT_DEPTH: usize = 60;
/// `NV0073_CTRL_CMD_DP_GET_CAPS_MAX_LINK_RATE_*` (2:0): 1 RBR .. 4 HBR3.
pub const DP_CAPS_MAX_LINK_RATE_8_10: u32 = 4;
/// `DP_VERSIONS_SUPPORTED`: DP1_2 bit 0, DP1_4 bit 1.
pub const DP_VERSIONS_DP1_2_DP1_4: u32 = 0x3;

// ---------------------------------------------------------------------------------
// NV5070 / NVC370 / NVC372 on the NVC570 and NVC372 objects (ctrl5070*.h, ctrlc370*.h,
// ctrlc372*.h). Each struct starts with an `NVxx70_CTRL_CMD_BASE_PARAMS` (subdeviceIndex).

pub const CMD_5070_SYSTEM_GET_CAPS_V2: u32 = 0x5070_0709;
pub const CMD_5070_SET_SOR_FLUSH_MODE: u32 = 0x5070_0457;
pub const CMD_5070_SET_RMFREE_FLAGS: u32 = 0x5070_0117;
pub const CMD_C370_IDLE_CHANNEL: u32 = 0xC370_0101;
pub const CMD_C370_SET_ACCL: u32 = 0xC370_0102;
pub const CMD_C370_GET_ACCL: u32 = 0xC370_0103;
pub const CMD_C370_GET_CHANNEL_INFO: u32 = 0xC370_0104;
pub const CMD_C370_GET_LOCKPINS_CAPS: u32 = 0xC370_0201;
pub const CMD_C372_IS_MODE_POSSIBLE: u32 = 0xC372_0101;
pub const CMD_C372_GET_ACTIVE_VIEWPORT_POINT_IN: u32 = 0xC372_0104;
/// `NV5070_CTRL_SYSTEM_GET_CAPS_V2_PARAMS` (8): base, `capsTbl[1]` at 4.
pub const CAPS_5070_TBL: usize = 4;
/// `NVC370_CTRL_CMD_GET_CHANNEL_INFO_PARAMS` (20): channelState at 16.
pub const CHANNEL_INFO_STATE: usize = 16;
/// `NVC370_CTRL_CMD_CHANNEL_STATE_IDLE` = NVBIT(0).
pub const CHANNEL_STATE_IDLE: u32 = 1;
/// `NVC370_CTRL_GET_LOCKPINS_CAPS_PARAMS` (36): stereoPin at 20; NONE is 0xFFFFFFFF.
pub const LOCKPINS_STEREO_PIN: usize = 20;
pub const LOCKPIN_NONE: u32 = 0xFFFF_FFFF;
/// `NVC372_CTRL_IS_MODE_POSSIBLE_PARAMS` (2048): numHeads 4, numWindows 5,
/// bIsPossible 1904.
pub const IMP_SIZE: usize = 2048;
pub const IMP_IS_POSSIBLE: usize = 1904;
/// `numHeads` (NvU8), `head[]` of `NVC372_CTRL_IMP_HEAD` (92 bytes: `headIndex` NvU8 at
/// 0, `maxPixelClkKHz` at 4); gcc-measured, display-layouts.txt.
pub const IMP_NUM_HEADS: usize = 4;
pub const IMP_HEAD: usize = 8;
pub const IMP_HEAD_SIZE: usize = 92;
pub const IMP_HEAD_INDEX: usize = 0;
pub const IMP_HEAD_MAX_PCLK_KHZ: usize = 4;

// ---------------------------------------------------------------------------------
// Channel allocation parameters (cl5070.h / alloc).

/// `NV50VAIO_CHANNELDMA_ALLOCATION_PARAMETERS` (40): channelInstance 0, hObjectBuffer 4,
/// hObjectNotify 8, offset 12.
pub const CHANNELDMA_ALLOC_SIZE: usize = 40;
/// `NV50VAIO_CHANNELPIO_ALLOCATION_PARAMETERS` (16): channelInstance 0.
pub const CHANNELPIO_ALLOC_SIZE: usize = 16;
pub const CHANNEL_ALLOC_INSTANCE: usize = 0;

#[cfg(test)]
mod tests {
    use super::*;

    /// The gcc-measured table: `STRUCT sizeof N` and `STRUCT.field OFFSET` lines.
    const MEASURED: &str = include_str!("display-layouts.txt");

    fn measured(key: &str) -> usize {
        MEASURED
            .lines()
            .find_map(|l| {
                let (k, v) = l.rsplit_once(' ')?;
                (k == key).then(|| v.parse().ok())?
            })
            .unwrap_or_else(|| panic!("{key} not measured"))
    }

    #[test]
    fn display_layouts_match_the_headers() {
        let s = |n: &str| measured(&format!("{n} sizeof"));
        let f = |n: &str, field: &str| measured(&format!("{n}.{field}"));
        assert_eq!(
            s("NV2080_CTRL_INTERNAL_DISPLAY_GET_STATIC_INFO_PARAMS"),
            STATIC_INFO_SIZE
        );
        assert_eq!(
            f(
                "NV2080_CTRL_INTERNAL_DISPLAY_GET_STATIC_INFO_PARAMS",
                "numDispChannels"
            ),
            STATIC_INFO_NUM_DISP_CHANNELS
        );
        assert_eq!(
            std::mem::size_of::<nvrm_sys::v610::NV2080_CTRL_INTERNAL_DISPLAY_GET_STATIC_INFO_PARAMS>(
            ),
            STATIC_INFO_SIZE
        );
        assert_eq!(
            s("NV2080_CTRL_INTERNAL_DISPLAY_WRITE_INST_MEM_PARAMS"),
            WRITE_INST_MEM_SIZE
        );
        assert_eq!(
            s("NV2080_CTRL_INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER_PARAMS"),
            PUSHBUFFER_SIZE
        );
        let pb = "NV2080_CTRL_INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER_PARAMS";
        assert_eq!(f(pb, "physicalAddr"), PUSHBUFFER_PHYSICAL_ADDR);
        assert_eq!(f(pb, "limit"), PUSHBUFFER_LIMIT);
        assert_eq!(f(pb, "hclass"), PUSHBUFFER_HCLASS);
        assert_eq!(f(pb, "channelInstance"), PUSHBUFFER_CHANNEL_INSTANCE);
        assert_eq!(f(pb, "valid"), PUSHBUFFER_VALID);
        assert_eq!(f(pb, "channelPBSize"), PUSHBUFFER_CHANNEL_PB_SIZE);
        assert_eq!(
            s("NV2080_CTRL_INTERNAL_DISPLAY_SET_IMP_INIT_INFO_PARAMS"),
            SET_IMP_INIT_INFO_SIZE
        );
        assert_eq!(
            s("NV2080_CTRL_CMD_INTERNAL_SET_STATIC_EDID_DATA_PARAMS"),
            SET_STATIC_EDID_DATA_SIZE
        );
        let aux = "NV0073_CTRL_DP_AUXCH_CTRL_PARAMS";
        assert_eq!(s(aux), AUX_SIZE);
        assert_eq!(f(aux, "bAddrOnly"), AUX_ADDR_ONLY);
        assert_eq!(f(aux, "cmd"), AUX_CMD);
        assert_eq!(f(aux, "addr"), AUX_ADDR);
        assert_eq!(f(aux, "data"), AUX_DATA);
        assert_eq!(f(aux, "size"), AUX_SIZE_FIELD);
        assert_eq!(f(aux, "replyType"), AUX_REPLY_TYPE);
        assert_eq!(f(aux, "retryTimeMs"), AUX_RETRY_TIME_MS);
        assert_eq!(
            s("NV0073_CTRL_SPECIFIC_GET_EDID_V2_PARAMS"),
            GET_EDID_V2_SIZE
        );
        assert_eq!(
            f("NV0073_CTRL_SPECIFIC_GET_EDID_V2_PARAMS", "edidBuffer"),
            GET_EDID_BUFFER
        );
        assert_eq!(
            s("NV0073_CTRL_SPECIFIC_SET_EDID_V2_PARAMS"),
            SET_EDID_V2_SIZE
        );
        assert_eq!(
            f("NV0073_CTRL_SPECIFIC_SET_EDID_V2_PARAMS", "edidBuffer"),
            SET_EDID_BUFFER
        );
        let cd = "NV0073_CTRL_SPECIFIC_GET_CONNECTOR_DATA_PARAMS";
        assert_eq!(f(cd, "count"), CONNECTOR_DATA_COUNT);
        assert_eq!(f(cd, "data"), CONNECTOR_DATA_DATA);
        assert_eq!(f(cd, "platform"), CONNECTOR_DATA_PLATFORM);
        let or = "NV0073_CTRL_SPECIFIC_OR_GET_INFO_PARAMS";
        assert_eq!(f(or, "protocol"), OR_INFO_PROTOCOL);
        assert_eq!(f(or, "location"), OR_INFO_LOCATION);
        assert_eq!(f(or, "dcbIndex"), OR_INFO_DCB_INDEX);
        assert_eq!(f(or, "bIsDispDynamic"), OR_INFO_IS_DISP_DYNAMIC);
        let caps = "NV0073_CTRL_CMD_DP_GET_CAPS_PARAMS";
        assert_eq!(s(caps), DP_CAPS_SIZE);
        assert_eq!(f(caps, "maxLinkRate"), DP_CAPS_MAX_LINK_RATE);
        assert_eq!(f(caps, "bFECSupported"), DP_CAPS_FEC);
        assert_eq!(f(caps, "bSupportDPDownSpread"), DP_CAPS_DOWNSPREAD);
        assert_eq!(f(caps, "DSC"), DP_CAPS_DSC_SUPPORTED);
        let a = "NV0073_CTRL_DFP_ASSIGN_SOR_PARAMS";
        assert_eq!(f(a, "sorAssignList"), ASSIGN_SOR_LIST);
        assert_eq!(f(a, "sorAssignListWithTag"), ASSIGN_SOR_LIST_WITH_TAG);
        let lc = "NV0073_CTRL_DP_GET_LINK_CONFIG_PARAMS";
        assert_eq!(f(lc, "linkBW"), LINK_CONFIG_LINK_BW);
        assert_eq!(f(lc, "bFECEnabled"), LINK_CONFIG_FEC_ENABLED);
        assert_eq!(f("NV0073_CTRL_DP_CTRL_PARAMS", "err"), DP_CTRL_ERR);
        assert_eq!(
            f("NVC370_CTRL_CMD_GET_CHANNEL_INFO_PARAMS", "channelState"),
            CHANNEL_INFO_STATE
        );
        assert_eq!(s("NVC372_CTRL_IS_MODE_POSSIBLE_PARAMS"), IMP_SIZE);
        assert_eq!(
            f("NVC372_CTRL_IS_MODE_POSSIBLE_PARAMS", "numHeads"),
            IMP_NUM_HEADS
        );
        assert_eq!(f("NVC372_CTRL_IS_MODE_POSSIBLE_PARAMS", "head"), IMP_HEAD);
        assert_eq!(s("NVC372_CTRL_IMP_HEAD"), IMP_HEAD_SIZE);
        assert_eq!(f("NVC372_CTRL_IMP_HEAD", "headIndex"), IMP_HEAD_INDEX);
        assert_eq!(
            f("NVC372_CTRL_IMP_HEAD", "maxPixelClkKHz"),
            IMP_HEAD_MAX_PCLK_KHZ
        );
        assert_eq!(
            f("NVC372_CTRL_IS_MODE_POSSIBLE_PARAMS", "bIsPossible"),
            IMP_IS_POSSIBLE
        );
        assert_eq!(
            f("NVC370_CTRL_GET_LOCKPINS_CAPS_PARAMS", "stereoPin"),
            LOCKPINS_STEREO_PIN
        );
        assert_eq!(
            s("NV50VAIO_CHANNELDMA_ALLOCATION_PARAMETERS"),
            CHANNELDMA_ALLOC_SIZE
        );
        assert_eq!(
            f(
                "NV0073_CTRL_SPECIFIC_GET_VALID_HEAD_WINDOW_ASSIGNMENT_PARAMS",
                "windowHeadMask"
            ),
            WINDOW_HEAD_MASK
        );
    }
}

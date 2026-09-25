// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! GMCAPI and NVKV, the init vocabulary of the 615 transport
//! (docs/caraxes/gsp-transport-615.md §4-5).
//!
//! From driver 615 the kernel RM sends one `GMCAPI_CMD_GSP_INIT` (0x10001) in place of
//! GSP_SET_SYSTEM_INFO, SET_REGISTRY and GET_GSP_STATIC_INFO, and no longer waits for
//! GSP_INIT_DONE (kernel_gsp.c:5314-5361, bootstrap in kernel_gsp_tu102.c:463-474). The
//! request carries the system information and the registry as NVKV words
//! (`_kgspEncodeSystemInfo`, kernel_gsp.c:4977-5217, `_kgspEncodePackedRegistryKeys`,
//! :4893-4958); the response carries the static configuration as NVKV words, decoded by
//! `_kgspStaticInfoKeyHandler` (kernel_gsp.c:7729-7862) into the kernel's
//! `GspStaticConfigInfo`.
//!
//! Paths are relative to the open-gpu-kernel-modules tree at tag 615.71.09. The
//! GMCAPI header is `NV_ABI_STABLE` (interface/gmcapi/gmcapi_base.h:27-45); the NVKV
//! format is src/nvidia/inc/libraries/nvkv/nvkv.h:36-118 and the decoder
//! src/nvidia/src/libraries/nvkv/nvkv.c:27-207, which [`decode`] follows word for word.

use std::fmt;

// ---------------------------------------------------------------------------------
// GMCAPI_HEADER (interface/gmcapi/gmcapi_base.h:27-60).

/// `sizeof(GMCAPI_HEADER)`, `ct_assert`ed in src/nvidia/src/kernel/core/gmcapi_impl.c:191.
pub const HEADER_SIZE: usize = 40;
pub const HEADER_COMMAND: usize = 0;
pub const HEADER_SIZE_FIELD: usize = 4;
pub const HEADER_SEQUENCE: usize = 8;
/// `request.max_response_size` in a request, `response.status` in a response.
pub const HEADER_REQUEST_OR_STATUS: usize = 16;
pub const HEADER_RESERVED: usize = 20;
pub const HEADER_DATA: usize = 40;

/// `GMCAPI_HEADER_COMMAND_ID_MASK` (gmcapi_base.h:47).
pub const COMMAND_ID_MASK: u32 = 0x00FF_FFFF;
/// `GMCAPI_HEADER_COMMAND_FLAGS_RESPONSE` (gmcapi_base.h:59).
pub const COMMAND_FLAGS_RESPONSE: u32 = 0x0100_0000;
/// Command ids up to 0xFFFF are reserved "to ensure no collisions with the 'function'
/// constant of the VGPU style RPC" (gmcapi_base.h:76-80): one namespace for both.
pub const COMMAND_RESERVED_LAST: u32 = 0xFFFF;

/// `GMCAPI_ID(GSP_MGMT, n)` = 0x01 << 16 | n (interface/gmcapi/gmcapi_table.h:74-124).
pub const CMD_GSP_INIT: u32 = 0x0001_0001;
pub const CMD_EXEC_GENERIC_BOOTLOADER: u32 = 0x0001_0002;
pub const CMD_EXEC_HS_BINARY: u32 = 0x0001_0003;
pub const CMD_GSP_SUSPEND: u32 = 0x0001_0004;
pub const CMD_GSP_RESUME_DONE: u32 = 0x0001_0005;

/// The largest static-info response the kernel accepts: `maxStaticInfoDataSize`
/// (kernel_gsp.c:5321), passed as `request.max_response_size`.
pub const GSP_INIT_MAX_RESPONSE: usize = 48 * 1024;

/// The name of a GMCAPI command id, for the trace.
pub fn command_name(id: u32) -> &'static str {
    match id & COMMAND_ID_MASK {
        CMD_GSP_INIT => "GMCAPI_GSP_INIT",
        CMD_EXEC_GENERIC_BOOTLOADER => "GMCAPI_EXEC_GENERIC_BOOTLOADER",
        CMD_EXEC_HS_BINARY => "GMCAPI_EXEC_HS_BINARY",
        CMD_GSP_SUSPEND => "GMCAPI_GSP_SUSPEND",
        CMD_GSP_RESUME_DONE => "GMCAPI_GSP_RESUME_DONE",
        _ => "GMCAPI_?",
    }
}

/// `GMCAPI_HEADER`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Header {
    pub command: u32,
    /// Bytes of payload after the header.
    pub size: u32,
    pub sequence: u64,
    /// `request.max_response_size` or `response.status` (a union, gmcapi_base.h:32-42).
    pub request_or_status: u32,
}

impl Header {
    pub fn parse(b: &[u8]) -> Option<Self> {
        if b.len() < HEADER_SIZE {
            return None;
        }
        let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        Some(Header {
            command: u32_at(HEADER_COMMAND),
            size: u32_at(HEADER_SIZE_FIELD),
            sequence: u64::from_le_bytes(
                b[HEADER_SEQUENCE..HEADER_SEQUENCE + 8].try_into().unwrap(),
            ),
            request_or_status: u32_at(HEADER_REQUEST_OR_STATUS),
        })
    }

    pub fn encode(&self) -> [u8; HEADER_SIZE] {
        let mut b = [0u8; HEADER_SIZE];
        b[HEADER_COMMAND..HEADER_COMMAND + 4].copy_from_slice(&self.command.to_le_bytes());
        b[HEADER_SIZE_FIELD..HEADER_SIZE_FIELD + 4].copy_from_slice(&self.size.to_le_bytes());
        b[HEADER_SEQUENCE..HEADER_SEQUENCE + 8].copy_from_slice(&self.sequence.to_le_bytes());
        b[HEADER_REQUEST_OR_STATUS..HEADER_REQUEST_OR_STATUS + 4]
            .copy_from_slice(&self.request_or_status.to_le_bytes());
        b
    }

    pub fn id(&self) -> u32 {
        self.command & COMMAND_ID_MASK
    }

    pub fn is_response(&self) -> bool {
        self.command & COMMAND_FLAGS_RESPONSE != 0
    }

    /// The response to a request: the kernel matches the command id and the sequence
    /// (kernel_gsp.c:1989-2002) and asserts the response flag; it reads `size` bytes of
    /// response and `response.status` (gpu.c:7887-7893).
    pub fn response(id: u32, sequence: u64, status: u32, size: usize) -> Self {
        Header {
            command: (id & COMMAND_ID_MASK) | COMMAND_FLAGS_RESPONSE,
            size: size as u32,
            sequence,
            request_or_status: status,
        }
    }
}

// ---------------------------------------------------------------------------------
// NVKV (src/nvidia/inc/libraries/nvkv/nvkv.h:36-118).

/// Opcodes, bits 31:28 of an instruction word (nvkv.h:88-104).
pub const OP_IMM32: u64 = 0x0;
pub const OP_SEQ32: u64 = 0x1;
pub const OP_SEQ64: u64 = 0x2;
pub const OP_ARRAY8: u64 = 0x3;
pub const OP_ARRAY32: u64 = 0x4;
pub const OP_ARRAY64: u64 = 0x5;

/// One instruction word: key 15:0, index 27:16, opcode 31:28, immediate or count 63:32
/// (nvkv.h:81-86).
pub fn instruction(op: u64, index: u16, key: u16, imm_or_count: u32) -> u64 {
    (key as u64) | (((index & 0xFFF) as u64) << 16) | (op << 28) | ((imm_or_count as u64) << 32)
}

/// A decoded value, typed as the decoder types it (nvkv.h:410-436): `U32` and `U64` for
/// the IMM32/SEQ32 and SEQ64 opcodes (one value per key), arrays for the ARRAY opcodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    U32(u32),
    U64(u64),
    U8s(Vec<u8>),
    U32s(Vec<u32>),
    U64s(Vec<u64>),
}

impl Value {
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            Value::U32(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::U64(v) => Some(*v),
            _ => None,
        }
    }

    /// A string sent with `nvkvSetString8` (the NUL counted, nvkv.h:270-286).
    pub fn as_str(&self) -> Option<String> {
        match self {
            Value::U8s(b) => {
                let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
                Some(String::from_utf8_lossy(&b[..n]).into_owned())
            }
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::U32(v) => write!(f, "{v:#x}"),
            Value::U64(v) => write!(f, "{v:#x}"),
            Value::U8s(b) => match self.as_str() {
                Some(s) if b.last() == Some(&0) && s.chars().all(|c| !c.is_control()) => {
                    write!(f, "{s:?}")
                }
                _ => write!(f, "{} bytes", b.len()),
            },
            Value::U32s(v) => write!(f, "{} x u32", v.len()),
            Value::U64s(v) => {
                let items: Vec<String> = v.iter().map(|x| format!("{x:#x}")).collect();
                write!(f, "[{}]", items.join(", "))
            }
        }
    }
}

/// One key/value pair as the handler sees it (`NVKVKeyHandler`, nvkv.h:438-449).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: u16,
    pub index: u16,
    pub value: Value,
}

/// Decode NVKV words the way `nvkvDecode` does (nvkv.c:27-207): SEQ32 and SEQ64 give one
/// entry per key with the key incremented, arrays one entry; padding to the next
/// 8-byte boundary is skipped. `Err` for an unknown opcode (NV_ERR_INVALID_COMMAND) or
/// an instruction that reads past the end (NV_ERR_OUT_OF_RANGE).
pub fn decode(words: &[u64]) -> Result<Vec<Entry>, String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < words.len() {
        let w = words[i];
        let at = i;
        i += 1;
        let mut key = (w & 0xFFFF) as u16;
        let index = ((w >> 16) & 0xFFF) as u16;
        let op = (w >> 28) & 0xF;
        let count = (w >> 32) as usize;
        let take = |n: usize, i: usize| -> Result<&[u64], String> {
            words.get(i..i + n).ok_or_else(|| {
                format!(
                    "NVKV word {at}: {n} value words past the end ({} words)",
                    words.len()
                )
            })
        };
        match op {
            OP_IMM32 => out.push(Entry {
                key,
                index,
                value: Value::U32((w >> 32) as u32),
            }),
            OP_SEQ32 => {
                let n64 = (count * 4).div_ceil(8);
                let v = take(n64, i)?;
                for k in 0..count {
                    let word = v[k / 2];
                    let x = if k % 2 == 0 {
                        word as u32
                    } else {
                        (word >> 32) as u32
                    };
                    out.push(Entry {
                        key,
                        index,
                        value: Value::U32(x),
                    });
                    key = key.wrapping_add(1);
                }
                i += n64;
            }
            OP_SEQ64 => {
                let v = take(count, i)?;
                for &x in v {
                    out.push(Entry {
                        key,
                        index,
                        value: Value::U64(x),
                    });
                    key = key.wrapping_add(1);
                }
                i += count;
            }
            OP_ARRAY8 => {
                let n64 = count.div_ceil(8);
                let v = take(n64, i)?;
                let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).take(count).collect();
                out.push(Entry {
                    key,
                    index,
                    value: Value::U8s(bytes),
                });
                i += n64;
            }
            OP_ARRAY32 => {
                let n64 = (count * 4).div_ceil(8);
                let v = take(n64, i)?;
                let vals: Vec<u32> = v
                    .iter()
                    .flat_map(|x| [*x as u32, (*x >> 32) as u32])
                    .take(count)
                    .collect();
                out.push(Entry {
                    key,
                    index,
                    value: Value::U32s(vals),
                });
                i += n64;
            }
            OP_ARRAY64 => {
                let v = take(count, i)?;
                out.push(Entry {
                    key,
                    index,
                    value: Value::U64s(v.to_vec()),
                });
                i += count;
            }
            _ => return Err(format!("NVKV word {at}: invalid instruction {w:#018x}")),
        }
    }
    Ok(out)
}

/// NVKV words from little-endian bytes (a GMCAPI payload); a trailing partial word is
/// ignored, as the kernel sends whole words (`sizeof(NvU64) * count`, kernel_gsp.c:5328-5331).
pub fn words(bytes: &[u8]) -> Vec<u64> {
    bytes
        .chunks_exact(8)
        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// An NVKV encoder with the kernel's helpers (nvkv.h:147-286).
#[derive(Clone, Debug, Default)]
pub struct Encoder {
    words: Vec<u64>,
}

impl Encoder {
    pub fn new() -> Self {
        Encoder::default()
    }

    /// `nvkvSetImm32`.
    pub fn imm32(&mut self, index: u16, key: u16, v: u32) -> &mut Self {
        self.words.push(instruction(OP_IMM32, index, key, v));
        self
    }

    /// `nvkvSetSeq32_*`: consecutive keys from `key`, two values per word.
    pub fn seq32(&mut self, index: u16, key: u16, v: &[u32]) -> &mut Self {
        self.words
            .push(instruction(OP_SEQ32, index, key, v.len() as u32));
        for pair in v.chunks(2) {
            let hi = pair.get(1).copied().unwrap_or(0);
            self.words.push(pair[0] as u64 | ((hi as u64) << 32));
        }
        self
    }

    /// `nvkvSetSeq64_*`: consecutive keys from `key`.
    pub fn seq64(&mut self, index: u16, key: u16, v: &[u64]) -> &mut Self {
        self.words
            .push(instruction(OP_SEQ64, index, key, v.len() as u32));
        self.words.extend_from_slice(v);
        self
    }

    /// `nvkvSetArray8`, zero-padded to a whole word.
    pub fn array8(&mut self, index: u16, key: u16, v: &[u8]) -> &mut Self {
        self.words
            .push(instruction(OP_ARRAY8, index, key, v.len() as u32));
        for c in v.chunks(8) {
            let mut b = [0u8; 8];
            b[..c.len()].copy_from_slice(c);
            self.words.push(u64::from_le_bytes(b));
        }
        self
    }

    /// `nvkvSetString8`: the bytes plus the NUL, counted (nvkv.h:270-286).
    pub fn string8(&mut self, index: u16, key: u16, s: &str) -> &mut Self {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        self.array8(index, key, &v)
    }

    /// `nvkvSetArray64`.
    pub fn array64(&mut self, index: u16, key: u16, v: &[u64]) -> &mut Self {
        self.words
            .push(instruction(OP_ARRAY64, index, key, v.len() as u32));
        self.words.extend_from_slice(v);
        self
    }

    pub fn words(&self) -> &[u64] {
        &self.words
    }

    /// The encoded words as little-endian bytes (the GMCAPI payload).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }
}

// ---------------------------------------------------------------------------------
// Keys.

/// `NVGMC_SI_*`: the system information of the GSP_INIT request
/// (interface/gmcapi/gmcapi_system_info.h:26-216, gmcapi_system_info_unstable.h:29).
pub mod si {
    pub const PCI_DEVICE_ID: u16 = 0x0001;
    pub const PCI_SUB_DEVICE_ID: u16 = 0x0002;
    pub const PCI_REVISION_ID: u16 = 0x0003;
    pub const PCI_CONFIG_MIRROR_BASE: u16 = 0x0010;
    pub const PCI_CONFIG_MIRROR_SIZE: u16 = 0x0011;
    pub const PCIE_ATOMICS_CPL_DEV_CAP_MASK: u16 = 0x0020;
    pub const CHIPSET: u16 = 0x0030;
    pub const CHIPSET_L1SS_ENABLE: u16 = 0x0031;
    pub const HYPERVISOR_TYPE: u16 = 0x0040;
    pub const VIRTUAL_CONFIG_BITS: u16 = 0x0041;
    pub const PCIE_POWER_CONTROL_VALUE: u16 = 0x0050;
    pub const PF0_DEVICE_CONTROL2_REG: u16 = 0x0051;
    pub const PCIE_LINK_CAP: u16 = 0x0052;
    pub const GRID_BUILD_CSP: u16 = 0x0060;
    pub const HOST_ARCH: u16 = 0x0070;
    pub const HOST_OS_TYPE: u16 = 0x0071;
    pub const HOST_PAGE_SIZE: u16 = 0x0078;
    pub const VF_TOTAL_VFS: u16 = 0x0080;
    pub const VF_FIRST_VF_OFFSET: u16 = 0x0081;
    pub const FHB_BUSINFO_REVISION_ID: u16 = 0x0090;
    pub const CHIPSET_BUSINFO_REVISION_ID: u16 = 0x0091;
    pub const ACPI_DOD_STATUS: u16 = 0x0200;
    pub const ACPI_CAPS_OPTIMUS_CAPS: u16 = 0x0271;
    pub const PCIE_FLAGS: u16 = 0x1000;
    pub const PLATFORM_FLAGS: u16 = 0x1001;
    pub const FEATURE_FLAGS: u16 = 0x1002;
    pub const VF_FLAGS: u16 = 0x1003;
    pub const GPU_PHYS_ADDR: u16 = 0x1010;
    pub const GPU_PHYS_FB_ADDR: u16 = 0x1011;
    pub const GPU_PHYS_INST_ADDR: u16 = 0x1012;
    pub const GPU_PHYS_IO_ADDR: u16 = 0x1013;
    pub const NV_DOMAIN_BUS_DEVICE_FUNC: u16 = 0x1020;
    pub const SIM_ACCESS_BUF_PHYS_ADDR: u16 = 0x1021;
    pub const NOTIFY_OP_SHARED_SURF_PHYS_ADDR: u16 = 0x1022;
    pub const PCIE_ATOMICS_OP_MASK: u16 = 0x1030;
    pub const CONSOLE_MEM_SIZE: u16 = 0x1031;
    pub const MAX_USER_VA: u16 = 0x1032;
    /// gmcapi_system_info_unstable.h:29.
    pub const CL_FLAGS: u16 = 0x1033;
    pub const SYS_TIMER_OFFSET_NS: u16 = 0x1040;
    pub const VF_FIRST_BAR0_ADDRESS: u16 = 0x1050;
    pub const FHB_BUSINFO: u16 = 0x1090;
    pub const CHIPSET_BUSINFO: u16 = 0x1091;
    pub const KERNEL_DRIVER_NAME: u16 = 0x1100;
    pub const KERNEL_DRIVER_VERSION_STRING: u16 = 0x1101;
    pub const KERNEL_DRIVER_CHANGELIST: u16 = 0x1102;
    pub const UNSTABLE_INTERFACE_SUPPORT: u16 = 0x1110;
    pub const REGKEY_NAME: u16 = 0x3070;
    pub const REGKEY_VALUE_U32: u16 = 0x3071;
    pub const REGKEY_VALUE_STRING: u16 = 0x3072;
    pub const REGKEY_VALUE_BINARY: u16 = 0x3073;
}

/// `NVGMC_SC_*`: the static configuration of the GSP_INIT response
/// (interface/gmcapi/gmcapi_gsp_config.h:30-148). The value type each key must carry is
/// the one of its `NVKV_CASE_*` in `_kgspStaticInfoKeyHandler` (kernel_gsp.c:7741-7848):
/// a key with the wrong type, index or count ends the decode with
/// NV_ERR_INVALID_ARGUMENT (the `continue` of `_NVKV_VALIDATE_*`, nvkv.h:507-519, leaves
/// the switch to kernel_gsp.c:7859-7861), and GSP_INIT fails.
pub mod sc {
    /// U32 keys (IMM32 or SEQ32).
    pub const SKU_BOARD_ID: u16 = 0x0001;
    pub const CHIP_SUB_REV: u16 = 0x0004;
    pub const EMULATION_REV1: u16 = 0x0005;
    pub const GC6_RTD3_TOTAL_BOARD_POWER: u16 = 0x0030;
    pub const GC6_RTD3_PERST_DELAY: u16 = 0x0031;
    pub const SRIOV_MAX_GFID: u16 = 0x0200;
    pub const FB_REGION_COUNT: u16 = 0x0010;
    pub const FB_REGION_PERFORMANCE: u16 = 0x0011;
    pub const FB_REGION_FLAGS: u16 = 0x0012;
    pub const FB_REGION_FLAGS_IS_COMPRESSION_SUPPORTED: u32 = 1 << 0;
    pub const FB_REGION_FLAGS_IS_ISO_SUPPORTED: u32 = 1 << 1;
    pub const FB_REGION_FLAGS_IS_PROTECTED: u32 = 1 << 2;
    pub const FB_REGION_TAG: u16 = 0x0013;
    pub const FB_REGION_TAG_NONE: u32 = 0;
    pub const VBIOS_SUB_VENDOR: u16 = 0x0050;
    pub const VBIOS_SUB_DEVICE: u16 = 0x0051;
    pub const VBIOS_REVISION: u16 = 0x0052;
    pub const VBIOS_OEM_REVISION: u16 = 0x0053;
    pub const VGX_VERSION: u16 = 0x0210;
    pub const INTERNAL_CLIENT_HANDLE: u16 = 0x0FF0;
    pub const INTERNAL_DEVICE_HANDLE: u16 = 0x0FF1;
    pub const INTERNAL_SUBDEVICE_HANDLE: u16 = 0x0FF2;
    /// U64 keys (SEQ64).
    pub const BRANDING: u16 = 0x1000;
    pub const MISC_FEATURES: u16 = 0x1001;
    pub const POWER_FEATURES: u16 = 0x1002;
    pub const FB_LENGTH: u16 = 0x1010;
    pub const FB_REGION_BASE: u16 = 0x1011;
    pub const FB_REGION_LIMIT: u16 = 0x1012;
    pub const FB_REGION_RESERVED: u16 = 0x1013;
    pub const BAR1_PDE_BASE: u16 = 0x1020;
    pub const BAR2_PDE_BASE: u16 = 0x1021;
    pub const WPR_LAYOUT_NON_WPR_HEAP_OFFSET: u16 = 0x1080;
    pub const WPR_LAYOUT_FRTS_OFFSET: u16 = 0x1081;
    /// SRIOV_BAR0_SIZE, then BAR1 and BAR2 at the next keys.
    pub const SRIOV_BAR0_SIZE: u16 = 0x1200;
    /// U64 array, index = the first NVGMC_ENGINE_TYPE.
    pub const ENGINE_MASK: u16 = 0x1100;
    /// Byte arrays (ARRAY8).
    pub const GPU_NAME_STRING: u16 = 0x2000;
    pub const GPU_SHORT_NAME_STRING: u16 = 0x2001;
    pub const GPU_GID_SHA1: u16 = 0x2002;
    pub const CHIP_SKU: u16 = 0x2003;
    pub const CHIP_SKU_MODIFIER: u16 = 0x2004;
    /// `NV2080_CTRL_CMD_FB_GET_FB_REGION_INFO_MAX_ENTRIES`, the bound of FB_REGION_COUNT
    /// (kernel_gsp.c:7762, sdk/ctrl/ctrl2080/ctrl2080fb.h).
    pub const FB_REGION_MAX: u32 = 16;
}

/// The name of a key for the trace (`None` for a key this table does not name).
pub fn key_name(request: bool, key: u16) -> Option<&'static str> {
    Some(if request {
        match key {
            si::PCI_DEVICE_ID => "PCI_DEVICE_ID",
            si::PCI_SUB_DEVICE_ID => "PCI_SUB_DEVICE_ID",
            si::PCI_REVISION_ID => "PCI_REVISION_ID",
            si::PCI_CONFIG_MIRROR_BASE => "PCI_CONFIG_MIRROR_BASE",
            si::PCI_CONFIG_MIRROR_SIZE => "PCI_CONFIG_MIRROR_SIZE",
            si::HOST_ARCH => "HOST_ARCH",
            si::HOST_OS_TYPE => "HOST_OS_TYPE",
            si::HOST_PAGE_SIZE => "HOST_PAGE_SIZE",
            si::GPU_PHYS_ADDR => "GPU_PHYS_ADDR",
            si::GPU_PHYS_FB_ADDR => "GPU_PHYS_FB_ADDR",
            si::GPU_PHYS_INST_ADDR => "GPU_PHYS_INST_ADDR",
            si::GPU_PHYS_IO_ADDR => "GPU_PHYS_IO_ADDR",
            si::NV_DOMAIN_BUS_DEVICE_FUNC => "NV_DOMAIN_BUS_DEVICE_FUNC",
            si::MAX_USER_VA => "MAX_USER_VA",
            si::KERNEL_DRIVER_NAME => "KERNEL_DRIVER_NAME",
            si::KERNEL_DRIVER_VERSION_STRING => "KERNEL_DRIVER_VERSION_STRING",
            si::KERNEL_DRIVER_CHANGELIST => "KERNEL_DRIVER_CHANGELIST",
            si::REGKEY_NAME => "REGKEY_NAME",
            si::REGKEY_VALUE_U32 => "REGKEY_VALUE_U32",
            si::REGKEY_VALUE_STRING => "REGKEY_VALUE_STRING",
            si::REGKEY_VALUE_BINARY => "REGKEY_VALUE_BINARY",
            _ => return None,
        }
    } else {
        match key {
            sc::FB_REGION_COUNT => "FB_REGION_COUNT",
            sc::FB_REGION_PERFORMANCE => "FB_REGION_PERFORMANCE",
            sc::FB_REGION_FLAGS => "FB_REGION_FLAGS",
            sc::FB_REGION_TAG => "FB_REGION_TAG",
            sc::VBIOS_SUB_VENDOR => "VBIOS_SUB_VENDOR",
            sc::VBIOS_SUB_DEVICE => "VBIOS_SUB_DEVICE",
            sc::VBIOS_REVISION => "VBIOS_REVISION",
            sc::VBIOS_OEM_REVISION => "VBIOS_OEM_REVISION",
            sc::INTERNAL_CLIENT_HANDLE => "INTERNAL_CLIENT_HANDLE",
            sc::INTERNAL_DEVICE_HANDLE => "INTERNAL_DEVICE_HANDLE",
            sc::INTERNAL_SUBDEVICE_HANDLE => "INTERNAL_SUBDEVICE_HANDLE",
            sc::FB_LENGTH => "FB_LENGTH",
            sc::FB_REGION_BASE => "FB_REGION_BASE",
            sc::FB_REGION_LIMIT => "FB_REGION_LIMIT",
            sc::FB_REGION_RESERVED => "FB_REGION_RESERVED",
            sc::BAR1_PDE_BASE => "BAR1_PDE_BASE",
            sc::BAR2_PDE_BASE => "BAR2_PDE_BASE",
            sc::ENGINE_MASK => "ENGINE_MASK",
            sc::GPU_NAME_STRING => "GPU_NAME_STRING",
            sc::GPU_SHORT_NAME_STRING => "GPU_SHORT_NAME_STRING",
            sc::GPU_GID_SHA1 => "GPU_GID_SHA1",
            _ => return None,
        }
    })
}

// ---------------------------------------------------------------------------------
// The GSP_INIT request.

/// One registry entry of the request (`_kgspEncodePackedRegistryKeys`,
/// kernel_gsp.c:4906-4953): a REGKEY_NAME string followed by its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegValue {
    U32(u32),
    String(String),
    Binary(Vec<u8>),
}

/// What the device reads from the GSP_INIT request (`_kgspEncodeSystemInfo`,
/// kernel_gsp.c:4977-5217): the identity the CPU saw, the BAR addresses for
/// `BUS_GET_PCI_BAR_INFO` (the four `GPU_PHYS_*` of kernel_gsp.c:5003-5006), the
/// driver version, the registry. Keys the device does not use are counted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemInfo {
    pub pci_device_id: Option<u32>,
    pub pci_sub_device_id: Option<u32>,
    pub pci_revision_id: Option<u32>,
    /// BAR0, BAR1 (FB), BAR3 (instance) and I/O addresses.
    pub gpu_phys: [Option<u64>; 4],
    pub bdf: Option<u64>,
    pub pci_config_mirror: (Option<u32>, Option<u32>),
    pub max_user_va: Option<u64>,
    pub host_page_size: Option<u32>,
    pub driver_name: Option<String>,
    pub driver_version: Option<String>,
    pub changelist: Option<u32>,
    /// `NVGMC_SI_UNSTABLE_INTERFACE_SUPPORT`: RPC 0:0, EVENTS 1:1 (gspboot.rs).
    pub unstable: Option<u32>,
    pub registry: Vec<(String, RegValue)>,
    /// Keys in the request, SEQ runs counted per key.
    pub keys: usize,
    /// Keys this parser does not interpret.
    pub other_keys: usize,
}

impl SystemInfo {
    /// Decode the request payload. A REGKEY value without a preceding name is an error:
    /// the kernel always sends the name first (kernel_gsp.c:4923-4942).
    pub fn parse(payload: &[u8]) -> Result<Self, String> {
        if payload.len() % 8 != 0 {
            return Err(format!(
                "GSP_INIT request of {} bytes is not whole NVKV words",
                payload.len()
            ));
        }
        let mut s = SystemInfo::default();
        let mut name: Option<String> = None;
        for e in decode(&words(payload))? {
            s.keys += 1;
            let v = &e.value;
            match e.key {
                si::PCI_DEVICE_ID => s.pci_device_id = v.as_u32(),
                si::PCI_SUB_DEVICE_ID => s.pci_sub_device_id = v.as_u32(),
                si::PCI_REVISION_ID => s.pci_revision_id = v.as_u32(),
                si::GPU_PHYS_ADDR..=si::GPU_PHYS_IO_ADDR => {
                    s.gpu_phys[(e.key - si::GPU_PHYS_ADDR) as usize] = v.as_u64()
                }
                si::NV_DOMAIN_BUS_DEVICE_FUNC => s.bdf = v.as_u64(),
                si::PCI_CONFIG_MIRROR_BASE => s.pci_config_mirror.0 = v.as_u32(),
                si::PCI_CONFIG_MIRROR_SIZE => s.pci_config_mirror.1 = v.as_u32(),
                si::MAX_USER_VA => s.max_user_va = v.as_u64(),
                si::HOST_PAGE_SIZE => s.host_page_size = v.as_u32(),
                si::KERNEL_DRIVER_NAME => s.driver_name = v.as_str(),
                si::KERNEL_DRIVER_VERSION_STRING => s.driver_version = v.as_str(),
                si::KERNEL_DRIVER_CHANGELIST => s.changelist = v.as_u32(),
                si::UNSTABLE_INTERFACE_SUPPORT => s.unstable = v.as_u32(),
                si::REGKEY_NAME => name = v.as_str(),
                si::REGKEY_VALUE_U32 | si::REGKEY_VALUE_STRING | si::REGKEY_VALUE_BINARY => {
                    let n = name.take().ok_or_else(|| {
                        format!("registry value key {:#06x} without REGKEY_NAME", e.key)
                    })?;
                    let r = match (e.key, v) {
                        (si::REGKEY_VALUE_U32, Value::U32(x)) => RegValue::U32(*x),
                        (si::REGKEY_VALUE_STRING, _) => {
                            RegValue::String(v.as_str().unwrap_or_default())
                        }
                        (si::REGKEY_VALUE_BINARY, Value::U8s(b)) => RegValue::Binary(b.clone()),
                        _ => {
                            return Err(format!("registry key {n:?}: value {v} has the wrong type"))
                        }
                    };
                    s.registry.push((n, r));
                }
                _ => s.other_keys += 1,
            }
        }
        Ok(s)
    }

    /// The four BAR addresses once all four keys came.
    pub fn bar_addresses(&self) -> Option<[u64; 4]> {
        Some([
            self.gpu_phys[0]?,
            self.gpu_phys[1]?,
            self.gpu_phys[2]?,
            self.gpu_phys[3]?,
        ])
    }

    /// One line for the trace.
    pub fn describe(&self) -> String {
        let hex = |v: Option<u64>| v.map_or("-".into(), |x| format!("{x:#x}"));
        format!(
            "{} keys: driver {:?} {:?} cl {}, PCI {:?}/{:?} rev {:?}, bars {}/{}/{}/{}, bdf {}, mirror {:?}, maxUserVa {}, page {:?}, {} registry entr{}, {} other keys",
            self.keys,
            self.driver_name.as_deref().unwrap_or("-"),
            self.driver_version.as_deref().unwrap_or("-"),
            self.changelist.unwrap_or(0),
            self.pci_device_id,
            self.pci_sub_device_id,
            self.pci_revision_id,
            hex(self.gpu_phys[0]),
            hex(self.gpu_phys[1]),
            self.gpu_phys[2].map_or("-".into(), |x| format!("{x:#x}")),
            hex(self.gpu_phys[3]),
            hex(self.bdf),
            self.pci_config_mirror,
            hex(self.max_user_va),
            self.host_page_size,
            self.registry.len(),
            if self.registry.len() == 1 { "y" } else { "ies" },
            self.other_keys
        )
    }
}

// ---------------------------------------------------------------------------------
// Engines (interface/gmcapi/gmcapi_engine_types.h:33-62, gpu_engine_type.c:36-120).

/// `NVGMC_ENGINE_TYPE_*`.
pub const ENGINE_TYPE_GR: u16 = 0x01;
pub const ENGINE_TYPE_COPY: u16 = 0x02;
pub const ENGINE_TYPE_NVDEC: u16 = 0x03;
pub const ENGINE_TYPE_NVENC: u16 = 0x04;
pub const ENGINE_TYPE_SW: u16 = 0x09;
pub const ENGINE_TYPE_SEC2: u16 = 0x0D;
pub const ENGINE_TYPE_HOST: u16 = 0x0E;
pub const ENGINE_TYPE_COUNT: u16 = 0x14;

/// The NVGMC engine id (type 15:0, index 31:16) of an `NV2080_ENGINE_TYPE` number, from
/// the one-to-one table `_NV_ENGINE_MAPPINGS` (gpu_engine_type.c:36-120) with the
/// NV2080 numbers of sdk/class/cl2080_notification.h:285-350. `None` for an engine
/// the table does not map (or that the device never presents).
pub fn gmc_engine_id(nv2080: u32) -> Option<(u16, u16)> {
    Some(match nv2080 {
        0x01..=0x08 => (ENGINE_TYPE_GR, (nv2080 - 0x01) as u16),
        0x09..=0x12 => (ENGINE_TYPE_COPY, (nv2080 - 0x09) as u16),
        0x34..=0x3D => (ENGINE_TYPE_COPY, (nv2080 - 0x34 + 10) as u16),
        0x13..=0x1A => (ENGINE_TYPE_NVDEC, (nv2080 - 0x13) as u16),
        0x1B..=0x1D => (ENGINE_TYPE_NVENC, (nv2080 - 0x1B) as u16),
        0x3F => (ENGINE_TYPE_NVENC, 3),
        0x22 => (ENGINE_TYPE_SW, 0),
        0x26 => (ENGINE_TYPE_SEC2, 0),
        0x27 => (ENGINE_TYPE_HOST, 0),
        _ => return None,
    })
}

/// The NVGMC engine id of an `RM_ENGINE_TYPE` number (gpu_engine_type.h:33-110, the RM
/// column of `_NV_ENGINE_MAPPINGS`). `None` for a type the device never presents.
pub fn gmc_engine_id_of_rm_type(rm: u32) -> Option<(u16, u16)> {
    Some(match rm {
        0x01..=0x08 => (ENGINE_TYPE_GR, (rm - 0x01) as u16),
        0x09..=0x1C => (ENGINE_TYPE_COPY, (rm - 0x09) as u16),
        0x1D..=0x24 => (ENGINE_TYPE_NVDEC, (rm - 0x1D) as u16),
        0x25..=0x28 => (ENGINE_TYPE_NVENC, (rm - 0x25) as u16),
        0x2D => (ENGINE_TYPE_SW, 0),
        0x31 => (ENGINE_TYPE_SEC2, 0),
        0x32 => (ENGINE_TYPE_HOST, 0),
        _ => return None,
    })
}

/// The `NVGMC_SC_ENGINE_MASK` value for a list of `NV2080_ENGINE_TYPE` numbers: one u64
/// per engine type from `ENGINE_TYPE_GR` on, bit n for engine index n. Returns the first
/// type (the key index) and the masks, and the numbers that have no GMC id.
///
/// The 615 kernel decodes the key into `GspStaticConfigInfo.engineCaps` as RM engine
/// types (`gpuGetRmEngineTypeCapsFromGMCEngineIdMasks`, kernel_gsp.c:7837-7843), but its
/// one reader treats that array as `NV2080_ENGINE_TYPE` caps and converts it once more
/// (`gpuCheckEngineWithOrderList_KERNEL`, gpu.c:6733-6749, through
/// `gpuGetRmEngineTypeCapMask`). So the bit the kernel must see for an engine is its
/// NV2080 number, and each NV2080 number `t` is encoded here as the GMC id of the RM type
/// `t`. For GR0 and CE0-9 both numberings coincide, which is why GR and CE worked with the
/// plain mapping; NVDEC and NVENC did not: the 615 guest dropped NVC4B0 and NVC4B7 from
/// its class DB and listed no video engine (rmlog-615, NVENC refused as an unsupported
/// device).
pub fn engine_masks(nv2080_types: &[u32]) -> (u16, Vec<u64>, Vec<u32>) {
    let first = ENGINE_TYPE_GR;
    let mut masks = vec![0u64; (ENGINE_TYPE_COUNT - first) as usize];
    let mut unmapped = Vec::new();
    for &t in nv2080_types {
        match gmc_engine_id_of_rm_type(t) {
            Some((ty, idx)) if idx < 64 => masks[(ty - first) as usize] |= 1 << idx,
            _ => unmapped.push(t),
        }
    }
    while masks.last() == Some(&0) {
        masks.pop();
    }
    (first, masks, unmapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The kernel's own encoders, for the request side: `NVKV_SET_SEQ32_3U` (a SEQ32 of
    /// three packed into two words, nvkv.h:348-353), `NVKV_SET_SEQ64_4U`, a string, a
    /// registry DWORD entry, as `_kgspEncodeSystemInfo` and
    /// `_kgspEncodePackedRegistryKeys` write them.
    fn request() -> Vec<u8> {
        let mut e = Encoder::new();
        e.seq32(0, si::PCI_DEVICE_ID, &[0x1F02_10DE, 0x4011_1458, 0xA1]);
        e.seq64(
            0,
            si::GPU_PHYS_ADDR,
            &[0xF500_0000, 0xE000_0000, 0xF000_0000, 0xE000],
        );
        e.seq64(0, si::NV_DOMAIN_BUS_DEVICE_FUNC, &[0x0000_0400]);
        e.imm32(0, si::HOST_ARCH, 1);
        e.seq32(0, si::PCI_CONFIG_MIRROR_BASE, &[0x88000, 0x1000]);
        e.imm32(0, si::HOST_PAGE_SIZE, 4096);
        e.string8(0, si::KERNEL_DRIVER_NAME, "NVRM");
        e.string8(0, si::KERNEL_DRIVER_VERSION_STRING, "615.71.09");
        e.imm32(0, si::KERNEL_DRIVER_CHANGELIST, 0);
        e.string8(0, si::REGKEY_NAME, "RmRcWatchdog");
        e.imm32(0, si::REGKEY_VALUE_U32, 0);
        e.string8(0, si::REGKEY_NAME, "RmMsg");
        e.string8(0, si::REGKEY_VALUE_STRING, ":");
        e.to_bytes()
    }

    #[test]
    fn instruction_layout_is_the_header_one() {
        // key 15:0, index 27:16, opcode 31:28, count 63:32 (nvkv.h:81-86).
        assert_eq!(instruction(OP_SEQ64, 3, 0x1011, 2), 0x0000_0002_2003_1011);
        assert_eq!(instruction(OP_IMM32, 0, 0x0010, 7), 0x0000_0007_0000_0010);
        // The 12-bit index field does not spill into the opcode.
        assert_eq!(instruction(OP_IMM32, 0xFFFF, 0, 0) >> 28, 0);
    }

    #[test]
    fn the_request_decodes_as_the_kernel_encodes_it() {
        let s = SystemInfo::parse(&request()).unwrap();
        assert_eq!(
            (s.pci_device_id, s.pci_sub_device_id, s.pci_revision_id),
            (Some(0x1F02_10DE), Some(0x4011_1458), Some(0xA1))
        );
        assert_eq!(
            s.bar_addresses(),
            Some([0xF500_0000, 0xE000_0000, 0xF000_0000, 0xE000])
        );
        assert_eq!(s.bdf, Some(0x400));
        assert_eq!(s.pci_config_mirror, (Some(0x88000), Some(0x1000)));
        assert_eq!(s.driver_name.as_deref(), Some("NVRM"));
        assert_eq!(s.driver_version.as_deref(), Some("615.71.09"));
        assert_eq!(
            s.registry,
            vec![
                ("RmRcWatchdog".into(), RegValue::U32(0)),
                ("RmMsg".into(), RegValue::String(":".into()))
            ]
        );
        // HOST_ARCH is a key the device does not interpret.
        assert_eq!(s.other_keys, 1);
        assert!(s.describe().contains("\"615.71.09\""), "{}", s.describe());
    }

    #[test]
    fn decode_refuses_what_nvkv_decode_refuses() {
        // Unknown opcode 6 (nvkv.c:197-202).
        assert!(decode(&[instruction(6, 0, 1, 0)])
            .unwrap_err()
            .contains("invalid instruction"));
        // A SEQ64 of two with one value word (nvkv.c:95-102).
        assert!(decode(&[instruction(OP_SEQ64, 0, 1, 2), 5])
            .unwrap_err()
            .contains("past the end"));
        // A registry value before its name.
        let mut e = Encoder::new();
        e.imm32(0, si::REGKEY_VALUE_U32, 1);
        assert!(SystemInfo::parse(&e.to_bytes()).is_err());
        assert!(SystemInfo::parse(&[0u8; 5]).is_err());
    }

    #[test]
    fn arrays_and_strings_are_padded_and_counted() {
        let mut e = Encoder::new();
        e.string8(0, sc::GPU_SHORT_NAME_STRING, "TU106");
        e.array8(0, sc::GPU_GID_SHA1, &[1u8; 16]);
        e.array64(1, sc::ENGINE_MASK, &[1, 0x1FF]);
        // "TU106\0" is 6 bytes counted, one padded word (nvkv.h:270-286).
        assert_eq!(e.words()[0] >> 32, 6);
        assert_eq!(e.words().len(), 2 + 3 + 3);
        let d = decode(e.words()).unwrap();
        assert_eq!(d[0].value.as_str().as_deref(), Some("TU106"));
        assert_eq!(d[1].value, Value::U8s(vec![1u8; 16]));
        assert_eq!(
            (d[2].index, d[2].value.clone()),
            (1, Value::U64s(vec![1, 0x1FF]))
        );
    }

    #[test]
    fn gmcapi_header_round_trip() {
        let h = Header {
            command: CMD_GSP_INIT,
            size: 0x280,
            sequence: 7,
            request_or_status: GSP_INIT_MAX_RESPONSE as u32,
        };
        let b = h.encode();
        assert_eq!(Header::parse(&b), Some(h));
        assert_eq!(&b[HEADER_RESERVED..HEADER_SIZE], &[0u8; 20]);
        let r = Header::response(h.command, h.sequence, 0, 64);
        assert!(r.is_response() && r.id() == CMD_GSP_INIT);
        assert_eq!(r.command, 0x0101_0001);
        assert_eq!(command_name(r.command), "GMCAPI_GSP_INIT");
        const { assert!(CMD_GSP_INIT > COMMAND_RESERVED_LAST) };
    }

    #[test]
    fn engines_map_to_gmc_masks() {
        // The TU106 list of chips/tu106/chip.toml: GR0, CE0-8, NVDEC0 (NVDEC1 in the test list: the encoding, not the chip), NVENC0, SW, SEC2, HOST.
        let tu106 = [
            0x01, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x13, 0x14, 0x1B, 0x22,
            0x26, 0x27,
        ];
        let (first, masks, unmapped) = engine_masks(&tu106);
        assert_eq!(first, ENGINE_TYPE_GR);
        assert!(unmapped.is_empty());
        // The 615 kernel's two steps: the key to RM-type bits (GMC id -> RM type), then that
        // array read as NV2080 bits (gpu.c:6733-6749). What it ends up checking must be the
        // NV2080 list itself, video engines included.
        let mut seen: Vec<u32> = Vec::new();
        for (i, m) in masks.iter().enumerate() {
            for j in 0..64u16 {
                if m & (1 << j) != 0 {
                    let id = (first + i as u16, j);
                    let rm = (1..0x40).find(|r| gmc_engine_id_of_rm_type(*r) == Some(id)).unwrap();
                    seen.push(rm);
                }
            }
        }
        seen.sort();
        assert_eq!(seen, tu106.to_vec());
        // As the key goes out: NVDEC0/1 and NVENC0 ride as COPY10, COPY11 and COPY18,
        // SW as NVDEC5, SEC2 and HOST as NVENC1 and NVENC2.
        let m = |t: u16| masks[(t - first) as usize];
        assert_eq!(m(ENGINE_TYPE_GR), 0b1);
        assert_eq!(m(ENGINE_TYPE_COPY), 0x1FF | 1 << 10 | 1 << 11 | 1 << 18);
        assert_eq!(m(ENGINE_TYPE_NVDEC), 1 << 5);
        assert_eq!(m(ENGINE_TYPE_NVENC), 0b110);
        assert_eq!(masks.len(), (ENGINE_TYPE_NVENC - first + 1) as usize);
        assert_eq!(gmc_engine_id(0x34), Some((ENGINE_TYPE_COPY, 10)));
        assert_eq!(gmc_engine_id(0x28), None);
    }
}

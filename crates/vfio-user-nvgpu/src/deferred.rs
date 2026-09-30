// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>

use std::fmt;

pub const CMD_DEFERRED_API: u32 = 0x5080_0101;
/// `NV5080_CTRL_CMD_REMOVE_API` (ctrl5080.h:188-212).
pub const CMD_REMOVE_API: u32 = 0x5080_0102;
/// `NV5080_CTRL_CMD_DEFERRED_API_V2`: the legacy layout with more bundles
/// (ctrl5080.h:125-180).
pub const CMD_DEFERRED_API_V2: u32 = 0x5080_0103;
/// `NV5080_CTRL_CMD_DEFERRED_API_INTERNAL`: V2 as the kernel RM forwards it to GSP-RM,
/// parameters unchanged (ctrl5080.h:182-186; deferred_api.c `defapiCtrlCmdDeferredApiV2`).
pub const CMD_DEFERRED_API_INTERNAL: u32 = 0x5080_0104;
/// `NV2080_CTRL_CMD_FIFO_UPDATE_CHANNEL_INFO`: a channel moves to a new USERD and GPFIFO
/// (ctrl2080fifo.h:693-725), directly or as a deferred API.
pub const CMD_UPDATE_CHANNEL_INFO: u32 = 0x2080_1116;
/// `NV2080_CTRL_FIFO_UPDATE_CHANNEL_INFO_PARAMS`: three handles, the entry count, then the
/// 8-aligned GPFIFO offset and USERD offset.
pub const UPDATE_CHANNEL_INFO_SIZE: usize = 32;
pub const CMD_INITIALIZE_CTX: u32 = 0x2080_012d;
pub const CMD_PROMOTE_CTX: u32 = 0x2080_012b;
pub const CMD_EVICT_CTX: u32 = 0x2080_012c;

pub const PARAMS_SIZE: usize = 584;
pub const INITIALIZE_CTX_SIZE: usize = 56;
pub const PROMOTE_CTX_SIZE: usize = 560;
pub const EVICT_CTX_SIZE: usize = 20;

const BUNDLE: usize = 24;
const ENGINE_GR0: u32 = 1;
const INITIALIZE_CACHE_BIT: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextTarget {
    pub engine_type: u32,
    pub h_client: u32,
    pub channel_id: u32,
    pub h_channel_client: u32,
    pub h_object: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Initialize {
        api_handle: u32,
        target: ContextTarget,
        h_virt_memory: u32,
        phys_address: u64,
        phys_attr: u32,
        h_dma_handle: u32,
        index: u32,
        size: u64,
    },
    Promote {
        api_handle: u32,
        target: ContextTarget,
        h_virt_memory: u32,
        virt_address: u64,
        size: u64,
    },
    Evict {
        api_handle: u32,
        target: ContextTarget,
    },
}

impl Request {
    pub fn api_handle(self) -> u32 {
        match self {
            Self::Initialize { api_handle, .. }
            | Self::Promote { api_handle, .. }
            | Self::Evict { api_handle, .. } => api_handle,
        }
    }

    pub fn target(self) -> ContextTarget {
        match self {
            Self::Initialize { target, .. }
            | Self::Promote { target, .. }
            | Self::Evict { target, .. } => target,
        }
    }

    pub fn command(self) -> u32 {
        match self {
            Self::Initialize { .. } => CMD_INITIALIZE_CTX,
            Self::Promote { .. } => CMD_PROMOTE_CTX,
            Self::Evict { .. } => CMD_EVICT_CTX,
        }
    }

    pub fn encode(self) -> [u8; PARAMS_SIZE] {
        let mut out = [0; PARAMS_SIZE];
        put_u32(&mut out, 0, self.api_handle());
        put_u32(&mut out, 4, self.command());
        let target = self.target();
        put_target(&mut out, target);
        match self {
            Self::Initialize {
                h_virt_memory,
                phys_address,
                phys_attr,
                h_dma_handle,
                index,
                size,
                ..
            } => {
                put_u32(&mut out, BUNDLE + 20, h_virt_memory);
                put_u64(&mut out, BUNDLE + 24, phys_address);
                put_u32(&mut out, BUNDLE + 32, phys_attr);
                put_u32(&mut out, BUNDLE + 36, h_dma_handle);
                put_u32(&mut out, BUNDLE + 40, index);
                put_u64(&mut out, BUNDLE + 48, size);
            }
            Self::Promote {
                h_virt_memory,
                virt_address,
                size,
                ..
            } => {
                put_u32(&mut out, BUNDLE + 20, h_virt_memory);
                put_u64(&mut out, BUNDLE + 24, virt_address);
                put_u64(&mut out, BUNDLE + 32, size);
            }
            Self::Evict { .. } => {}
        }
        out
    }
}

/// Whether `cmd` registers a deferred API (legacy, V2, or V2 forwarded as INTERNAL): one
/// parameter layout, `NV5080_CTRL_DEFERRED_API[_V2]_PARAMS` (ctrl5080.h:91-116, 152-180).
pub fn is_registration(cmd: u32) -> bool {
    matches!(cmd, CMD_DEFERRED_API | CMD_DEFERRED_API_V2 | CMD_DEFERRED_API_INTERNAL)
}

/// The nested command of a registration's parameters.
pub fn nested_command(bytes: &[u8]) -> Option<u32> {
    bytes.get(4..8).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

/// `NV2080_CTRL_FIFO_UPDATE_CHANNEL_INFO_PARAMS` (ctrl2080fifo.h:717-724).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelInfo {
    pub h_client: u32,
    pub h_channel: u32,
    pub h_userd_memory: u32,
    pub gp_fifo_entries: u32,
    pub gp_fifo_offset: u64,
    pub userd_offset: u64,
}

impl ChannelInfo {
    /// The control's own parameters (the direct call, or a deferred API's bundle).
    pub fn parse(bytes: &[u8]) -> Result<Self, ParseError> {
        if bytes.len() < UPDATE_CHANNEL_INFO_SIZE {
            return Err(ParseError::Length {
                expected: UPDATE_CHANNEL_INFO_SIZE,
                actual: bytes.len(),
            });
        }
        let info = Self {
            h_client: u32_at(bytes, 0),
            h_channel: u32_at(bytes, 4),
            h_userd_memory: u32_at(bytes, 8),
            gp_fifo_entries: u32_at(bytes, 12),
            gp_fifo_offset: u64_at(bytes, 16),
            userd_offset: u64_at(bytes, 24),
        };
        // kernel_fifo_ctrl.c: no USERD handle is NV_ERR_INVALID_ARGUMENT. A GPFIFO holds a
        // power-of-two number of 8-byte entries at an 8-byte aligned address (clc46f.h).
        if info.h_userd_memory == 0 {
            return Err(ParseError::InvalidField { field: "hUserdMemory", value: 0 });
        }
        if info.gp_fifo_entries < 2 || !info.gp_fifo_entries.is_power_of_two() {
            return Err(ParseError::InvalidField {
                field: "gpFifoEntries",
                value: u64::from(info.gp_fifo_entries),
            });
        }
        if info.gp_fifo_offset % 8 != 0 || info.userd_offset % 512 != 0 {
            return Err(ParseError::InvalidField {
                field: "gpFifoOffset/userdOffset alignment",
                value: info.gp_fifo_offset | info.userd_offset,
            });
        }
        checked_end("GPFIFO", info.gp_fifo_offset, 8 * u64::from(info.gp_fifo_entries))?;
        Ok(info)
    }

    pub fn encode(&self) -> [u8; UPDATE_CHANNEL_INFO_SIZE] {
        let mut out = [0; UPDATE_CHANNEL_INFO_SIZE];
        put_u32(&mut out, 0, self.h_client);
        put_u32(&mut out, 4, self.h_channel);
        put_u32(&mut out, 8, self.h_userd_memory);
        put_u32(&mut out, 12, self.gp_fifo_entries);
        put_u64(&mut out, 16, self.gp_fifo_offset);
        put_u64(&mut out, 24, self.userd_offset);
        out
    }
}

/// The UpdateChannelInfo a control carries: directly, or as the bundle of a deferred API
/// registration. `None` for any other control or a malformed one (its handler reports it).
pub fn migration_bundle(cmd: u32, params: &[u8]) -> Option<ChannelInfo> {
    if cmd == CMD_UPDATE_CHANNEL_INFO {
        return ChannelInfo::parse(params).ok();
    }
    if is_registration(cmd) && params.len() == PARAMS_SIZE && nested_command(params) == Some(CMD_UPDATE_CHANNEL_INFO) {
        return ChannelInfo::parse(&params[BUNDLE..BUNDLE + UPDATE_CHANNEL_INFO_SIZE]).ok();
    }
    None
}

/// A deferred UpdateChannelInfo: `hApiHandle`, `flags` (DELETE and WAIT_FOR_TLB_FLUSH,
/// ctrl5080.h:118-123) and the bundle, from registration parameters of any of the three
/// registration controls.
pub fn parse_channel_info_request(bytes: &[u8]) -> Result<(u32, u32, ChannelInfo), ParseError> {
    if bytes.len() != PARAMS_SIZE {
        return Err(ParseError::Length {
            expected: PARAMS_SIZE,
            actual: bytes.len(),
        });
    }
    let command = u32_at(bytes, 4);
    if command != CMD_UPDATE_CHANNEL_INFO {
        return Err(ParseError::UnsupportedCommand(command));
    }
    require_zero(bytes, BUNDLE + UPDATE_CHANNEL_INFO_SIZE, PARAMS_SIZE)?;
    let info = ChannelInfo::parse(&bytes[BUNDLE..BUNDLE + UPDATE_CHANNEL_INFO_SIZE])?;
    Ok((u32_at(bytes, 0), u32_at(bytes, 8), info))
}

/// The registration parameters of a deferred UpdateChannelInfo (V2 layout, no VA owner).
pub fn encode_channel_info_request(api_handle: u32, flags: u32, info: &ChannelInfo) -> [u8; PARAMS_SIZE] {
    let mut out = [0; PARAMS_SIZE];
    put_u32(&mut out, 0, api_handle);
    put_u32(&mut out, 4, CMD_UPDATE_CHANNEL_INFO);
    put_u32(&mut out, 8, flags);
    out[BUNDLE..BUNDLE + UPDATE_CHANNEL_INFO_SIZE].copy_from_slice(&info.encode());
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Length { expected: usize, actual: usize },
    UnsupportedCommand(u32),
    InvalidField { field: &'static str, value: u64 },
    NonZeroReserved { offset: usize },
    AddressOverflow { field: &'static str },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { expected, actual } => {
                write!(
                    f,
                    "deferred parameters are {actual} bytes, expected {expected}"
                )
            }
            Self::UnsupportedCommand(cmd) => write!(f, "unsupported deferred command {cmd:#x}"),
            Self::InvalidField { field, value } => write!(f, "invalid {field} value {value:#x}"),
            Self::NonZeroReserved { offset } => {
                write!(f, "nonzero reserved byte at offset {offset}")
            }
            Self::AddressOverflow { field } => write!(f, "{field} range overflows u64"),
        }
    }
}

impl std::error::Error for ParseError {}

pub fn parse(bytes: &[u8]) -> Result<Request, ParseError> {
    if bytes.len() != PARAMS_SIZE {
        return Err(ParseError::Length {
            expected: PARAMS_SIZE,
            actual: bytes.len(),
        });
    }
    require_zero(bytes, 8, 24)?;
    let api_handle = u32_at(bytes, 0);
    let command = u32_at(bytes, 4);
    let target = ContextTarget {
        engine_type: u32_at(bytes, BUNDLE),
        h_client: u32_at(bytes, BUNDLE + 4),
        channel_id: u32_at(bytes, BUNDLE + 8),
        h_channel_client: u32_at(bytes, BUNDLE + 12),
        h_object: u32_at(bytes, BUNDLE + 16),
    };
    if target.engine_type != ENGINE_GR0 {
        return Err(ParseError::InvalidField {
            field: "engineType",
            value: u64::from(target.engine_type),
        });
    }

    match command {
        CMD_INITIALIZE_CTX => {
            require_zero(bytes, BUNDLE + INITIALIZE_CTX_SIZE, PARAMS_SIZE)?;
            require_zero(bytes, BUNDLE + 44, BUNDLE + 48)?;
            let h_virt_memory = u32_at(bytes, BUNDLE + 20);
            require_value("hVirtMemory", h_virt_memory, 0)?;
            let phys_address = u64_at(bytes, BUNDLE + 24);
            let phys_attr = u32_at(bytes, BUNDLE + 32);
            if !matches!(phys_attr, 0 | INITIALIZE_CACHE_BIT) {
                return Err(ParseError::InvalidField {
                    field: "physAttr",
                    value: u64::from(phys_attr),
                });
            }
            let h_dma_handle = u32_at(bytes, BUNDLE + 36);
            require_value("hDmaHandle", h_dma_handle, 0)?;
            let index = u32_at(bytes, BUNDLE + 40);
            let size = u64_at(bytes, BUNDLE + 48);
            checked_end("physical context", phys_address, size)?;
            Ok(Request::Initialize {
                api_handle,
                target,
                h_virt_memory,
                phys_address,
                phys_attr,
                h_dma_handle,
                index,
                size,
            })
        }
        CMD_PROMOTE_CTX => {
            let h_virt_memory = u32_at(bytes, BUNDLE + 20);
            require_value("hVirtMemory", h_virt_memory, 0)?;
            let virt_address = u64_at(bytes, BUNDLE + 24);
            let size = u64_at(bytes, BUNDLE + 32);
            let entry_count = u32_at(bytes, BUNDLE + 40);
            require_value("entryCount", entry_count, 0)?;
            require_zero(bytes, BUNDLE + 44, PARAMS_SIZE)?;
            checked_end("virtual context", virt_address, size)?;
            Ok(Request::Promote {
                api_handle,
                target,
                h_virt_memory,
                virt_address,
                size,
            })
        }
        CMD_EVICT_CTX => {
            require_zero(bytes, BUNDLE + EVICT_CTX_SIZE, PARAMS_SIZE)?;
            Ok(Request::Evict { api_handle, target })
        }
        other => Err(ParseError::UnsupportedCommand(other)),
    }
}

fn checked_end(field: &'static str, address: u64, size: u64) -> Result<(), ParseError> {
    address
        .checked_add(size)
        .ok_or(ParseError::AddressOverflow { field })?;
    Ok(())
}

fn require_value(field: &'static str, value: u32, expected: u32) -> Result<(), ParseError> {
    if value == expected {
        Ok(())
    } else {
        Err(ParseError::InvalidField {
            field,
            value: u64::from(value),
        })
    }
}

fn require_zero(bytes: &[u8], start: usize, end: usize) -> Result<(), ParseError> {
    match bytes[start..end].iter().position(|&byte| byte != 0) {
        Some(offset) => Err(ParseError::NonZeroReserved {
            offset: start + offset,
        }),
        None => Ok(()),
    }
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn put_target(bytes: &mut [u8], target: ContextTarget) {
    put_u32(bytes, BUNDLE, target.engine_type);
    put_u32(bytes, BUNDLE + 4, target.h_client);
    put_u32(bytes, BUNDLE + 8, target.channel_id);
    put_u32(bytes, BUNDLE + 12, target.h_channel_client);
    put_u32(bytes, BUNDLE + 16, target.h_object);
}

#[cfg(test)]
mod tests {
    use super::*;

    // The nonzero prefixes of the eight P4 Windows captures. Their remaining union
    // storage is zero, so keeping the prefixes here retains the exact 584-byte payloads.
    const CAPTURE_PREFIXES: [&str; 8] = [
        "020000402d01802000000000000000000000000000000000010000002a00d0c1110000002c00d0c100000eff0000000000200e00000000000000000000000000110000000000000000d40a0000000000",
        "030000402b01802000000000000000000000000000000000010000002a00d0c1110000002c00d0c100000eff00000000007001000000000000d40a000000000000",
        "040000402c01802000000000000000000000000000000000010000002a00d0c1110000002c00d0c100000eff",
        "050000402d01802000000000000000000000000000000000010000005200d0c1110000005400d0c100000eff0000000000200e00000000000000000000000000110000000000000000d40a0000000000",
        "060000402b01802000000000000000000000000000000000010000005200d0c1110000005400d0c100000eff00000000001001000000000000d40a000000000000",
        "070000402d01802000000000000000000000000000000000010000005600d0c1120000005800d0c100000eff0000000000001900000000000000000000000000120000000000000000d40a0000000000",
        "080000402b01802000000000000000000000000000000000010000005600d0c1120000005800d0c100000eff00000000001001000000000000d40a000000000000",
        "090000402c01802000000000000000000000000000000000010000005200d0c1110000005400d0c100000eff",
    ];

    fn captures() -> Vec<Vec<u8>> {
        CAPTURE_PREFIXES
            .iter()
            .map(|prefix| {
                let mut bytes = decode_hex(prefix);
                bytes.resize(PARAMS_SIZE, 0);
                bytes
            })
            .collect()
    }

    fn decode_hex(text: &str) -> Vec<u8> {
        assert_eq!(text.len() % 2, 0);
        text.as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let digit = |byte: u8| match byte {
                    b'0'..=b'9' => byte - b'0',
                    b'a'..=b'f' => byte - b'a' + 10,
                    _ => panic!("non-hex capture byte"),
                };
                digit(pair[0]) << 4 | digit(pair[1])
            })
            .collect()
    }

    /// C115 (Windows, Steam beside Heaven): the request the kernel forwarded as
    /// DEFERRED_API_INTERNAL, which moved chid 22 to a 64K-entry GPFIFO and a new USERD.
    #[test]
    fn the_captured_channel_migration_parses_and_round_trips() {
        let mut bytes = decode_hex(
            "7e0000401611802000000000000000000000000000000000b500d0c1130004ff02a000ff00000100000038250200000000040000000000000000",
        );
        bytes.resize(PARAMS_SIZE, 0);
        assert!(is_registration(CMD_DEFERRED_API_INTERNAL));
        assert_eq!(nested_command(&bytes), Some(CMD_UPDATE_CHANNEL_INFO));
        let (api, flags, info) = parse_channel_info_request(&bytes).unwrap();
        assert_eq!((api, flags), (0x4000_007e, 0));
        assert_eq!(
            info,
            ChannelInfo {
                h_client: 0xc1d0_00b5,
                h_channel: 0xff04_0013,
                h_userd_memory: 0xff00_a002,
                gp_fifo_entries: 0x1_0000,
                gp_fifo_offset: 0x2_2538_0000,
                userd_offset: 0x400,
            }
        );
        assert_eq!(encode_channel_info_request(api, flags, &info).as_slice(), bytes);
        // The context parser refuses it: it is not a context request.
        assert!(parse(&bytes).is_err());
        assert_eq!(migration_bundle(CMD_DEFERRED_API_INTERNAL, &bytes), Some(info));
        assert_eq!(migration_bundle(CMD_UPDATE_CHANNEL_INFO, &info.encode()), Some(info));
        assert_eq!(migration_bundle(0x2080_110b, &info.encode()), None);
    }

    #[test]
    fn a_channel_migration_needs_a_userd_and_a_power_of_two_ring() {
        let good = ChannelInfo {
            h_client: 1,
            h_channel: 2,
            h_userd_memory: 3,
            gp_fifo_entries: 8,
            gp_fifo_offset: 0x1000,
            userd_offset: 0,
        };
        assert_eq!(ChannelInfo::parse(&good.encode()), Ok(good));
        let mut no_userd = good;
        no_userd.h_userd_memory = 0;
        assert!(ChannelInfo::parse(&no_userd.encode()).is_err());
        let mut odd = good;
        odd.gp_fifo_entries = 12;
        assert!(ChannelInfo::parse(&odd.encode()).is_err());
        let mut unaligned = good;
        unaligned.userd_offset = 0x10;
        assert!(ChannelInfo::parse(&unaligned.encode()).is_err());
        assert!(ChannelInfo::parse(&good.encode()[..16]).is_err());
    }

    #[test]
    fn captured_windows_requests_round_trip() {
        let captured = captures();
        assert_eq!(captured.len(), 8);
        for bytes in captured {
            assert_eq!(bytes.len(), PARAMS_SIZE);
            let request = parse(&bytes).unwrap();
            assert_eq!(request.encode().as_slice(), bytes);
        }
    }

    #[test]
    fn preserves_index_channel_and_client_identity() {
        let requests: Vec<_> = captures()
            .iter()
            .map(|bytes| parse(bytes).unwrap())
            .collect();
        let Request::Initialize { index, target, .. } = requests[0] else {
            panic!("first request is initialize")
        };
        assert_eq!(index, 0x11);
        assert_eq!(target.channel_id, 0x11);
        assert_ne!(requests[0].target().h_client, requests[3].target().h_client);
        assert_ne!(
            requests[3].target().h_channel_client,
            requests[5].target().h_channel_client
        );
    }

    #[test]
    fn rejects_bad_length_flags_and_engine() {
        let mut bytes = captures().remove(0);
        assert!(matches!(
            parse(&bytes[..583]),
            Err(ParseError::Length { .. })
        ));
        bytes[8] = 1;
        assert!(matches!(
            parse(&bytes),
            Err(ParseError::NonZeroReserved { offset: 8 })
        ));
        bytes[8] = 0;
        put_u32(&mut bytes, BUNDLE, 2);
        assert!(matches!(
            parse(&bytes),
            Err(ParseError::InvalidField {
                field: "engineType",
                ..
            })
        ));
    }

    #[test]
    fn rejects_unknown_commands_entries_and_unused_union_data() {
        let mut initialize = captures().remove(0);
        put_u32(&mut initialize, 4, 0xdead_beef);
        assert_eq!(
            parse(&initialize),
            Err(ParseError::UnsupportedCommand(0xdead_beef))
        );

        let mut promote = captures().remove(1);
        put_u32(&mut promote, BUNDLE + 40, 1);
        assert!(matches!(
            parse(&promote),
            Err(ParseError::InvalidField {
                field: "entryCount",
                ..
            })
        ));
        put_u32(&mut promote, BUNDLE + 40, 0);
        promote[BUNDLE + 48] = 1;
        assert!(matches!(
            parse(&promote),
            Err(ParseError::NonZeroReserved { .. })
        ));
    }

    #[test]
    fn rejects_invalid_attributes_handles_and_overflow() {
        let mut initialize = captures().remove(0);
        put_u32(&mut initialize, BUNDLE + 32, 1);
        assert!(matches!(
            parse(&initialize),
            Err(ParseError::InvalidField {
                field: "physAttr",
                ..
            })
        ));
        put_u32(&mut initialize, BUNDLE + 32, 0);
        put_u32(&mut initialize, BUNDLE + 36, 1);
        assert!(matches!(
            parse(&initialize),
            Err(ParseError::InvalidField {
                field: "hDmaHandle",
                ..
            })
        ));
        put_u32(&mut initialize, BUNDLE + 36, 0);
        put_u64(&mut initialize, BUNDLE + 24, u64::MAX);
        assert!(matches!(
            parse(&initialize),
            Err(ParseError::AddressOverflow { .. })
        ));
    }
}

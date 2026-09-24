// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The driver-version axis (decision D14): everything the device speaks that the
//! vendored driver release fixes and that no chip changes. That is the GSP RPC
//! vocabulary and its message layouts (`rpc_*_v03_00`, `rpc_*_v17_00`,
//! `GspStaticConfigInfo`, `GspSystemInfo`, `GspFwWprMeta`, the libos arguments),
//! the RM class and control numbers, and the control parameter layouts the
//! handlers in `ctrl.rs`, `kgr.rs`, `nvml.rs`, `uvm.rs` and `channel.rs` read and
//! write (docs/caraxes/other-architectures.md §6: one ABI per driver release, shared
//! by every chip).
//!
//! One module per vendored driver version, each behind a feature of the same name,
//! mirroring `crates/nvrm-sys`. The modules export the same names; generic code
//! reaches them through [`active`], the version this build emulates. A build
//! emulates exactly one driver version, because the guest accepts only the GSP
//! firmware whose `.fwversion` equals its own `NV_VERSION_STRING`
//! (docs/caraxes/gsp-abi-610.md (e), "Version selection"); the device notes a guest
//! that announces another version in `SET_GUEST_SYSTEM_INFO`.
//!
//! Where `nvrm-sys` has a typed layout for a parameter struct, the offsets and sizes
//! here are computed from it (`offset_of!`, `size_of`) and the tests pin them to the
//! numbers the handlers were written against. The GSP message structs are not in
//! `nvrm-sys` (gsp-abi-610.md (e) proposes a generated `nvgsp-sys` for them); their
//! hand-written offsets are checked against the compiler-measured tables of
//! gsp-abi-610.md (a) instead. How to add a version: docs/caraxes/versatility.md.

#[cfg(feature = "v610")]
pub mod v610;

/// The driver version this build emulates.
#[cfg(feature = "v610")]
pub use v610 as active;

#[cfg(not(feature = "v610"))]
compile_error!("vfio-user-nvgpu needs a driver-version feature (v610)");

/// How the CPU and the emulated GSP exchange messages: what a version module names in
/// `GSP_TRANSPORT`, what the boot model links and serves (boot.rs, msgq.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GspTransport {
    /// Up to 610: message queue v0 with the read and write pointers in the shared queue
    /// headers, checksummed and sequence-numbered elements, and the init RPCs
    /// GSP_SET_SYSTEM_INFO, SET_REGISTRY, GET_GSP_STATIC_INFO and the GSP_INIT_DONE event
    /// (docs/caraxes/gsp-rpc-protocol.md).
    MsgqV1,
    /// From 615: message queue v2 with the pointers in `NV_PGSP_QUEUE_HEAD/TAIL(0)` and
    /// `NV_PGSP_MSGQ_HEAD/TAIL(0)`, MCTP-framed elements without checksum or sequence
    /// number, and one `GMCAPI_CMD_GSP_INIT` with NVKV-encoded request and response
    /// (docs/caraxes/gsp-transport-615.md; `msgq::v2`, `gmcapi`).
    MsgqV2,
}

impl GspTransport {
    /// The guest ABI a transport stands for, as `--guest-abi` and `CARAXES_GUEST_ABI` name it.
    pub fn guest_abi(self) -> u32 {
        match self {
            GspTransport::MsgqV1 => 610,
            GspTransport::MsgqV2 => 615,
        }
    }
}

/// The compiler-measured layout tables of docs/caraxes/gsp-abi-610.md (a), parsed
/// for the tests: `(struct, field) -> offset`, and `struct -> sizeof`.
#[cfg(test)]
pub(crate) mod measured {
    use std::collections::BTreeMap;

    pub struct Tables {
        pub sizes: BTreeMap<String, usize>,
        pub fields: BTreeMap<(String, String), usize>,
    }

    /// Parse the `#### \`name\`: sizeof N` headings and the `| offset | hex | size |
    /// field |` rows below them.
    pub fn parse(md: &str) -> Tables {
        let mut sizes = BTreeMap::new();
        let mut fields = BTreeMap::new();
        let mut current: Option<String> = None;
        for line in md.lines() {
            if let Some(rest) = line.strip_prefix("#### `") {
                let Some((name, tail)) = rest.split_once('`') else {
                    current = None;
                    continue;
                };
                current = Some(name.to_string());
                if let Some(n) = tail
                    .strip_prefix(": sizeof ")
                    .and_then(|t| t.split_whitespace().next())
                    .and_then(|t| t.parse().ok())
                {
                    sizes.insert(name.to_string(), n);
                }
                continue;
            }
            if line.starts_with("## ") || line.starts_with("### ") {
                current = None;
                continue;
            }
            let Some(s) = &current else { continue };
            let cols: Vec<&str> = line.split('|').map(str::trim).collect();
            if cols.len() >= 5 {
                if let Ok(off) = cols[1].parse::<usize>() {
                    let field = cols[4].trim_matches('`');
                    let field = field.split('`').next().unwrap_or(field);
                    fields.insert((s.clone(), field.to_string()), off);
                }
            }
        }
        Tables { sizes, fields }
    }
}

/// Queue-element layout of the guest's driver. The build emulates one version, but
/// the element header is the first thing a guest of another version disagrees on
/// (docs/caraxes/guest-driver-versions.md): before 610 it is `authTagBuffer[16]`,
/// `aadBuffer[16]`, `checkSum`, `seqNum`, `elemCount` and the RPC header at 0x30
/// (595.91.07 `message_queue_priv.h:43-51`); from 610 on `mctpHeader`,
/// `nvdmHeader`, `checkSum`, `seqNum` and the payload at 0x10 (610.57.04
/// `message_queue_priv.h:52-67`). Experiment only: `CARAXES_GUEST_ABI=595` selects
/// the older layout; unset, the device uses the emulated version's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElementLayout {
    /// Bytes before the RPC header (`queueElementHdrSize`).
    pub header_size: usize,
    pub checksum_off: usize,
    pub seq_off: usize,
    /// `elemCount`, which the pre-610 receiver reads instead of deriving it.
    pub elem_count_off: Option<usize>,
    /// Whether the element carries `mctpHeader`/`nvdmHeader` at 0 and 4.
    pub mctp: bool,
}

impl ElementLayout {
    /// `maxRpcSize` = `queueElementSizeMax` (16 pages) minus the element header
    /// (message_queue_priv.h `gspMsgQueueGetMaxRpcSize`): 65520 from 610 on, 65488
    /// before. The first record of a large RPC is exactly this long.
    pub fn max_rpc_size(&self) -> usize {
        0x10000 - self.header_size
    }

    pub const EMULATED: ElementLayout = ElementLayout {
        header_size: active::ELEMENT_HEADER_SIZE,
        checksum_off: 8,
        seq_off: 12,
        elem_count_off: None,
        mctp: true,
    };
    pub const PRE_610: ElementLayout = ElementLayout {
        header_size: 0x30,
        checksum_off: 0x20,
        seq_off: 0x24,
        elem_count_off: Some(0x28),
        mctp: false,
    };
}

/// The layout selected by `CARAXES_GUEST_ABI` (read once).
pub fn element_layout() -> ElementLayout {
    static L: std::sync::OnceLock<ElementLayout> = std::sync::OnceLock::new();
    *L.get_or_init(|| match std::env::var("CARAXES_GUEST_ABI").as_deref() {
        Ok("595") => {
            log::warn!("CARAXES_GUEST_ABI=595: pre-610 GSP queue elements (experiment)");
            ElementLayout::PRE_610
        }
        _ => ElementLayout::EMULATED,
    })
}

/// `GspStaticConfigInfo` as a pre-610 guest reads it (`CARAXES_GUEST_ABI=595`):
/// 595.91.07 lacks `vbiosRevision`/`vbiosOemRevision` (610: 0x5fc, 0x600),
/// `bSysL2CacheCoherentMode` (610: 0x616) and `bBusResetRequired` (610: 0x638), so
/// its tail sits 8 bytes lower and the struct is 0x638 bytes, not 0x640
/// (gsp-abi-sizes of both trees, docs/caraxes/guest-driver-versions.md). Everything
/// before 0x5fc is identical. Unchanged for the emulated version.
pub fn static_info_for_guest(info: Vec<u8>) -> Vec<u8> {
    if element_layout() != ElementLayout::PRE_610 || info.len() != 0x640 {
        return info;
    }
    let mut out = vec![0u8; 0x638];
    // (610 offset, 595 offset, length)
    for (from, to, len) in [
        (0x000, 0x000, 0x5fc), // everything up to vbiosRevision
        (0x604, 0x5fc, 3),     // bPageRetirementSupported .. bClRootportNeedsNosnoopWAR
        (0x608, 0x600, 12),    // hInternalClient/Device/Subdevice
        (0x614, 0x60c, 2),     // bSelfHostedMode, bAtsSupported
        (0x617, 0x60e, 2),     // bIsGpuUefi, bIsEfiInit
        (0x61c, 0x610, 12),    // ecidInfo
        (0x628, 0x620, 16),    // fwWprLayoutOffset
    ] {
        out[to..to + len].copy_from_slice(&info[from..from + len]);
    }
    out
}

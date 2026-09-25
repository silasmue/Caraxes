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
/// Driver 615.71.09 as the guest's driver: the 615 control layouts over the 610 module
/// (docs/caraxes/gsp-transport-615.md). The feature implies `v610`, whose module keeps
/// the host-side layouts: the host driver stays 610.57.04.
#[cfg(feature = "v615")]
pub mod v615;

/// The driver version this build emulates. Features are additive (as in `nvrm-sys`),
/// so a build with both picks the newer one.
#[cfg(feature = "v615")]
pub use v615 as active;
#[cfg(all(feature = "v610", not(feature = "v615")))]
pub use v610 as active;

/// Driver 570.144's GSP boot vocabulary, the firmware nova-core speaks (Linux 7.2.7,
/// docs/caraxes/nova-guest.md): data only, served by a 610 build under
/// `CARAXES_GUEST_ABI=570` ([`older_guest`]).
pub mod v570;

#[cfg(not(feature = "v610"))]
compile_error!("vfio-user-nvgpu needs a driver-version feature (v610)");

// The generated control tables of every guest ABI (caraxes/tools/gsp-tables.py and
// caraxes/tools/ctrl-layouts.py; docs/caraxes/gsp-surface.md). They are data, not code,
// and compiled into every build, so the tests check a chip's tables against each ABI;
// the running device uses the build's own ([`Abi::active`]).
#[path = "v610/surface.rs"]
pub mod surface610;
#[path = "v610/ctrl_layouts.rs"]
pub mod layouts610;
#[path = "v615/surface.rs"]
pub mod surface615;
#[path = "v615/ctrl_layouts.rs"]
pub mod layouts615;
#[path = "v610/dmodel.rs"]
pub mod dmodel610;
#[path = "v615/dmodel.rs"]
pub mod dmodel615;

/// One feature family of category E controls ("never expected from a stock client on
/// this chip", docs/caraxes/gsp-surface.md §3.E) of one driver version.
#[derive(Debug)]
pub struct Family {
    pub name: &'static str,
    pub what: &'static str,
    /// The SDK header blocks that define the family's controls (`file:first-last`).
    pub headers: &'static [&'static str],
    /// (control id, control name).
    pub controls: &'static [(u32, &'static str)],
}

/// One control that reaches GSP, as the surface inventory of a driver version reads it.
#[derive(Debug)]
pub struct Routed {
    pub id: u32,
    pub name: &'static str,
    /// The inventory's heuristic category, 'A' to 'E' (gsp-surface.py `classify`).
    pub category: char,
    /// A handler other than the table rules of chipctl.rs answered it when the table was
    /// generated (display, kgr, cuda, uvm, nvml or a match arm of ctrl.rs).
    pub other: bool,
}

/// The answer kind of a category D control (docs/caraxes/gsp-plan/d-device-model.md §2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DAnswer {
    /// NV_ERR_NOT_SUPPORTED (or the status the header names): feature absent on this board.
    Refuse,
    /// Refused although a client may want it: the effect would reach shared host hardware.
    Deny,
    /// NV_OK with the request echoed: a setter no caller reads back.
    Ack,
    /// Static chip or board data (category A).
    Table,
    /// Answered from a device model's state (`model` names it).
    Model,
    /// Forwarded to the host mirror object of the guest's own channel or TSG.
    Mirror,
}

/// One category D control and the plan's answer to it (generated from d-controls.tsv by
/// caraxes/tools/gsp-dtable.py).
#[derive(Debug)]
pub struct DRule {
    pub id: u32,
    pub name: &'static str,
    /// The §6 group (`fifo`, `disp`, `prof`, ...).
    pub group: &'static str,
    pub answer: DAnswer,
    /// The model of a [`DAnswer::Model`] row (`display-connector`, `channel-registry`, ...).
    pub model: &'static str,
    /// The NV status of a Refuse, Deny or Ack row (from the tree's nvstatuscodes.h).
    pub status: u32,
    /// The category the row moves to: 'A', 'C', 'E', or '-' (stays D).
    pub reclass: char,
    /// 1 = on the CUDA or Wayland path now ... 5 = never on this chip (§5).
    pub rank: u8,
    /// `sizeof` of the parameter struct in this driver version.
    pub size: usize,
    /// The plan's reason where the answer names one (a status or a table value).
    pub why: &'static str,
}

/// A control parameter struct as the compiler lays it out for one driver version.
#[derive(Debug)]
pub struct CtrlLayout {
    pub name: &'static str,
    /// The SDK header that defines it (relative to `sdk/nvidia/inc/ctrl/`).
    pub header: &'static str,
    pub size: usize,
    /// Every scalar leaf.
    pub fields: &'static [CtrlField],
}

/// A scalar leaf of a [`CtrlLayout`]: `path` has `[]` for each array level, whose
/// (stride, count) are in `dims`, outermost first.
#[derive(Debug)]
pub struct CtrlField {
    pub path: &'static str,
    pub offset: usize,
    pub size: usize,
    pub dims: &'static [(usize, usize)],
}

impl CtrlLayout {
    /// The (offset, size) of the scalar `path` names, with array indices
    /// (`entries[2].nv2080EngineType`, `capsTbl[4]`).
    pub fn resolve(&self, path: &str) -> Result<(usize, usize), String> {
        let mut shape = String::new();
        let mut idx = Vec::new();
        let mut rest = path;
        while let Some(open) = rest.find('[') {
            let close = rest[open..]
                .find(']')
                .map(|c| open + c)
                .ok_or_else(|| format!("{}: unbalanced [ in {path:?}", self.name))?;
            shape.push_str(&rest[..open]);
            shape.push_str("[]");
            let i = rest[open + 1..close]
                .trim()
                .parse::<usize>()
                .map_err(|_| format!("{}: index in {path:?} is not a number", self.name))?;
            idx.push(i);
            rest = &rest[close + 1..];
        }
        shape.push_str(rest);
        let f = self
            .fields
            .iter()
            .find(|f| f.path == shape)
            .ok_or_else(|| format!("{} has no scalar field {path:?}", self.name))?;
        if f.dims.len() != idx.len() {
            return Err(format!(
                "{}: {path:?} needs {} index(es)",
                self.name,
                f.dims.len()
            ));
        }
        let mut off = f.offset;
        for (&i, &(stride, count)) in idx.iter().zip(f.dims) {
            if i >= count {
                return Err(format!("{}: index {i} of {path:?} is out of 0..{count}", self.name));
            }
            off += i * stride;
        }
        Ok((off, f.size))
    }
}

/// A guest ABI's generated tables.
#[derive(Debug)]
pub struct Abi {
    /// "610" or "615", as `--guest-abi` names it.
    pub name: &'static str,
    pub driver: &'static str,
    pub routed: &'static [Routed],
    pub families: &'static [Family],
    /// Category E and A controls no other handler answered at generation time.
    pub e_inventory: usize,
    pub a_inventory: usize,
    pub layouts: &'static [CtrlLayout],
    /// The category D rule table (docs/caraxes/gsp-plan/d-device-model.md, dmodel.rs).
    pub d_rules: &'static [DRule],
    /// The D rows that belong to E, as feature families beside `families`.
    pub d_families: &'static [Family],
    /// The ids of `d_families` answered NV_OK (echo) where the family is absent.
    pub d_e_ack: &'static [u32],
    /// The category D controls no other handler answered at generation time.
    pub d_inventory: usize,
    /// The profiler classes whose allocation is refused (6.16).
    pub profiler_classes: &'static [(u32, &'static str)],
    /// SDK enum values the modelled D controls use.
    pub d_enums: &'static [(&'static str, u32)],
}

/// Every guest ABI the tables exist for.
pub const ABIS: &[Abi] = &[
    Abi {
        name: "610",
        driver: surface610::SURFACE_DRIVER,
        routed: surface610::ROUTED,
        families: surface610::E_FAMILIES,
        e_inventory: surface610::E_INVENTORY,
        a_inventory: surface610::A_INVENTORY,
        layouts: layouts610::CTRL_LAYOUTS,
        d_rules: dmodel610::D_RULES,
        d_families: dmodel610::D_FAMILIES,
        d_e_ack: dmodel610::D_E_ACK,
        d_inventory: dmodel610::D_INVENTORY,
        profiler_classes: dmodel610::PROFILER_CLASSES,
        d_enums: dmodel610::D_ENUMS,
    },
    Abi {
        name: "615",
        driver: surface615::SURFACE_DRIVER,
        routed: surface615::ROUTED,
        families: surface615::E_FAMILIES,
        e_inventory: surface615::E_INVENTORY,
        a_inventory: surface615::A_INVENTORY,
        layouts: layouts615::CTRL_LAYOUTS,
        d_rules: dmodel615::D_RULES,
        d_families: dmodel615::D_FAMILIES,
        d_e_ack: dmodel615::D_E_ACK,
        d_inventory: dmodel615::D_INVENTORY,
        profiler_classes: dmodel615::PROFILER_CLASSES,
        d_enums: dmodel615::D_ENUMS,
    },
];

impl Abi {
    /// The tables of the driver version this build emulates.
    pub fn active() -> &'static Abi {
        ABIS.iter()
            .find(|a| a.driver == active::DRIVER_VERSION)
            .expect("generated control tables for the emulated driver version")
    }

    pub fn routed(&self, name: &str) -> Option<&'static Routed> {
        self.routed.iter().find(|r| r.name == name)
    }

    pub fn layout(&self, name: &str) -> Option<&'static CtrlLayout> {
        self.layouts.iter().find(|l| l.name == name)
    }

    /// The category D rule of `cmd`.
    pub fn d_rule(&self, cmd: u32) -> Option<&'static DRule> {
        self.d_rules.iter().find(|r| r.id == cmd)
    }

    /// An SDK enum value of a modelled D control.
    pub fn d_enum(&self, name: &str) -> Option<u32> {
        self.d_enums.iter().find(|e| e.0 == name).map(|e| e.1)
    }
}

/// What the control id 0x20800A36 names in the guest's driver: 610.57.04
/// `NV2080_CTRL_CMD_INTERNAL_GPU_GET_CHIP_INFO` (88-byte params), 615.71.09
/// `NV2080_CTRL_CMD_INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO` (16-byte params; 615
/// sdk/ctrl/ctrl2080/ctrl2080internal.h:572-585). Each version module names its own as
/// `CTRL_0A36`, and ctrl.rs dispatches the id by it, so a 615 guest is never answered
/// with 610 chip info.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Id0A36 {
    GetChipInfo,
    SyncFabricProbeInfo,
}

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

/// The guest ABI the device serves, checked against this build (fail closed): what
/// `--guest-abi` and `CARAXES_GUEST_ABI` ask for must be the build's own
/// (`active::GSP_TRANSPORT`: 610 in a default build, 615 in a `--features v615` build), or
/// 595 or 570 in a 610 build together with the environment switch that selects the pre-610
/// element layout ([`element_layout`], [`older_guest`]; 570 is nova-core's firmware,
/// docs/caraxes/nova-guest.md). Neither given: the build's. Returns the ABI.
pub fn check_guest_abi(flag: Option<u32>, env: Option<&str>) -> Result<u32, String> {
    let build = active::GSP_TRANSPORT.guest_abi();
    let env = match env {
        None | Some("") => None,
        Some(v) => Some(
            v.parse::<u32>()
                .map_err(|_| format!("CARAXES_GUEST_ABI={v:?} is not 570, 595, 610 or 615"))?,
        ),
    };
    if let (Some(f), Some(e)) = (flag, env) {
        if f != e {
            return Err(format!("--guest-abi {f} but CARAXES_GUEST_ABI={e}"));
        }
    }
    let want = flag.or(env).unwrap_or(build);
    let how = |abi: u32| match abi {
        615 => "cargo build --release -p vfio-user-nvgpu --features v615 --target-dir target/v615".to_string(),
        _ => "cargo build --release -p vfio-user-nvgpu".to_string(),
    };
    match want {
        570 | 595 if build == 610 && env == Some(want) => Ok(want),
        570 | 595 => Err(format!(
            "guest ABI {want} needs a 610 build and CARAXES_GUEST_ABI={want} in the device's environment"
        )),
        610 | 615 if want == build => Ok(want),
        610 | 615 => Err(format!(
            "guest ABI {want}, but this binary emulates {} with the {build} transport ({:?}); use the binary of `{}`",
            active::DRIVER_VERSION,
            active::GSP_TRANSPORT,
            how(want)
        )),
        other => Err(format!("guest ABI {other} is none of 570, 595, 610, 615")),
    }
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

#[cfg(test)]
mod abi_tests {
    use super::*;

    #[test]
    fn the_guest_abi_must_be_the_builds() {
        let build = active::GSP_TRANSPORT.guest_abi();
        let other = if build == 610 { 615 } else { 610 };
        assert_eq!(check_guest_abi(None, None), Ok(build));
        assert_eq!(check_guest_abi(Some(build), Some(&build.to_string())), Ok(build));
        assert!(check_guest_abi(Some(other), None).unwrap_err().contains("use the binary"));
        assert!(check_guest_abi(None, Some(&other.to_string())).is_err());
        assert!(check_guest_abi(Some(610), Some("615")).is_err());
        assert!(check_guest_abi(None, Some("6x5")).is_err());
        assert!(check_guest_abi(Some(595), None).is_err(), "595 needs the environment switch");
        assert_eq!(check_guest_abi(None, Some("595")).is_ok(), build == 610);
        assert_eq!(check_guest_abi(Some(570), Some("570")).is_ok(), build == 610);
        assert!(check_guest_abi(Some(570), None).is_err(), "570 needs the environment switch");
        assert!(check_guest_abi(Some(570), Some("595")).is_err());
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

#[cfg(test)]
thread_local! {
    /// A test's stand-in for `CARAXES_GUEST_ABI` (per test thread): `Some(guest)` overrides
    /// [`older_guest`].
    pub(crate) static TEST_GUEST: std::cell::Cell<Option<Option<u32>>> = const { std::cell::Cell::new(None) };
}

/// A guest older than the emulated version that a 610 build serves on request
/// (`CARAXES_GUEST_ABI`, read once): 595 (the Ubuntu driver of
/// docs/caraxes/guest-driver-versions.md) or 570 (570.144, the firmware Linux 7.2.7's
/// nova-core loads, docs/caraxes/nova-guest.md). None: the emulated version.
pub fn older_guest() -> Option<u32> {
    #[cfg(test)]
    if let Some(g) = TEST_GUEST.with(|c| c.get()) {
        return g;
    }
    static G: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    *G.get_or_init(|| match std::env::var("CARAXES_GUEST_ABI").as_deref() {
        Ok("595") => {
            log::warn!("CARAXES_GUEST_ABI=595: pre-610 GSP queue elements (experiment)");
            Some(595)
        }
        Ok("570") => {
            log::warn!(
                "CARAXES_GUEST_ABI=570: r570.144 GSP boot for nova-core (pre-610 elements, CPU sequencer, 570 static info)"
            );
            Some(570)
        }
        _ => None,
    })
}

/// The layout selected by `CARAXES_GUEST_ABI`: 570.144 frames its elements as 595 does
/// (`GSP_MSG_QUEUE_ELEMENT` with the RPC header at 0x30, 570.144
/// `src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h`; nova-core
/// `gsp/fw/r570_144/bindings.rs:868-876` of Linux 7.2.7).
pub fn element_layout() -> ElementLayout {
    match older_guest() {
        Some(_) => ElementLayout::PRE_610,
        None => ElementLayout::EMULATED,
    }
}

/// The events the device sends when it links the queues, in order: GSP_INIT_DONE for the
/// emulated version (`rpc_init_done_v17_00`, 1 byte in 610); for a 570 guest first an empty
/// GSP_RUN_CPU_SEQUENCER and then GSP_INIT_DONE, because nova-core waits for the sequencer
/// first and consumes and drops every message it is not waiting for (Linux 7.2.7 nova-core
/// `gsp/hal/tu102.rs:335-356`, then `gsp/boot.rs:153-154`; `gsp/cmdq.rs:813-848`).
pub fn link_events(guest: Option<u32>) -> Vec<(u32, Vec<u8>)> {
    match guest {
        Some(570) => vec![
            (v570::EV_GSP_RUN_CPU_SEQUENCER, v570::empty_sequence()),
            (active::EV_GSP_INIT_DONE, vec![0u8; v570::INIT_DONE_SIZE]),
        ],
        _ => vec![(active::EV_GSP_INIT_DONE, vec![0u8])],
    }
}

/// `GspStaticConfigInfo` as a pre-610 guest reads it (`CARAXES_GUEST_ABI=595`):
/// 595.91.07 lacks `vbiosRevision`/`vbiosOemRevision` (610: 0x5fc, 0x600),
/// `bSysL2CacheCoherentMode` (610: 0x616) and `bBusResetRequired` (610: 0x638), so
/// its tail sits 8 bytes lower and the struct is 0x638 bytes, not 0x640
/// (gsp-abi-sizes of both trees, docs/caraxes/guest-driver-versions.md). Everything
/// before 0x5fc is identical. Unchanged for the emulated version.
pub fn static_info_for_guest(info: Vec<u8>) -> Vec<u8> {
    static_info_for(older_guest(), info)
}

/// [`static_info_for_guest`] for a given older guest (the tests pass it; the device reads
/// `CARAXES_GUEST_ABI`). A 570 guest gets the 570.144 layout, remapped field by field by the
/// table generated from the two snapshots ([`v570::static_info`]).
pub fn static_info_for(guest: Option<u32>, info: Vec<u8>) -> Vec<u8> {
    if guest == Some(570) {
        return v570::static_info(&info);
    }
    if guest != Some(595) || info.len() != 0x640 {
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

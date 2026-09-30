// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The driver-version axis (decision D14): everything the device speaks that the
//! vendored driver release fixes and that no chip changes. That is the GSP RPC
//! vocabulary and its message layouts (`rpc_*_v03_00`, `rpc_*_v17_00`,
//! `GspStaticConfigInfo`, `GspSystemInfo`, `GspFwWprMeta`, the libos arguments),
//! the RM class and control numbers, and the control parameter layouts the
//! handlers in `ctrl.rs`, `kgr.rs`, `nvml.rs`, `uvm.rs` and `channel.rs` read and
//! write (docs/caraxes/other-architectures.md §6: one ABI per driver release, shared
//! by every chip).
//!
//! One module per vendored driver version, mirroring `crates/nvrm-sys`, and every one is
//! compiled into every build (docs/research/one-abi-binary-2026-09-28.md). The guest
//! chooses (decision D14): the device serves the ABI of the guest in front of it, read at
//! run time from what the guest prepared for its GSP boot ([`guest`], detected by
//! `BootModel`), or the one `--guest-abi` / `CARAXES_GUEST_ABI` pinned. Generic code reads
//! what all guest ABIs share through [`common`] and what differs through [`layouts`]; the
//! transport follows [`GuestAbi::transport`]. The guest accepts only the GSP firmware whose
//! `.fwversion` equals its own `NV_VERSION_STRING` (docs/caraxes/gsp-abi-610.md (e),
//! "Version selection"); the device notes a guest that announces a version other than the
//! served one in `SET_GUEST_SYSTEM_INFO` or the GMCAPI init.
//!
//! Where `nvrm-sys` has a typed layout for a parameter struct, the offsets and sizes
//! here are computed from it (`offset_of!`, `size_of`) and the tests pin them to the
//! numbers the handlers were written against. The GSP message structs are not in
//! `nvrm-sys` (gsp-abi-610.md (e) proposes a generated `nvgsp-sys` for them); their
//! hand-written offsets are checked against the compiler-measured tables of
//! gsp-abi-610.md (a) instead. How to add a version: docs/caraxes/versatility.md.

pub mod v610;
/// Driver 615.71.09 as the guest's driver: the 615 control layouts over the 610 module
/// (docs/caraxes/gsp-transport-615.md). It also serves the 617.14 Windows guest, whose
/// model is 615's.
pub mod v615;

/// Driver 570.144's GSP boot vocabulary, the firmware nova-core speaks (Linux 7.2.7,
/// docs/caraxes/nova-guest.md): data only, over the 610 control vocabulary
/// ([`GuestAbi::V570`], [`older_guest`]).
pub mod v570;

/// The vocabulary every guest ABI shares: the 610 module without the items that differ,
/// which are private there and read through [`layouts`] (src/ver/common.rs).
pub mod common;

// The generated control tables of every guest ABI (caraxes/tools/gsp-tables.py and
// caraxes/tools/ctrl-layouts.py; docs/caraxes/gsp-surface.md). They are data, not code,
// and compiled into every build, so the tests check a chip's tables against each ABI;
// the running device uses the guest's ([`Abi::active`]).
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
    /// The tables of the guest ABI in force ([`guest`]; a 570 or 595 guest's controls are
    /// 610's).
    pub fn active() -> &'static Abi {
        Self::of(guest())
    }

    /// The tables of `g`.
    pub fn of(g: GuestAbi) -> &'static Abi {
        let driver = g.layouts().driver;
        ABIS.iter()
            .find(|a| a.driver == driver)
            .expect("generated control tables for every guest ABI")
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

/// A value resolved once for every guest ABI's generated tables ([`ABIS`]: 610, 615) when
/// the data is loaded, and read for the guest ABI in force ([`Abi::active`]): the chip data
/// resolved against control ids and layouts (chipctl.rs), which a chip loads before any
/// guest shows its ABI.
#[derive(Clone, Debug, Default)]
pub struct PerAbi<T> {
    by_abi: Vec<T>,
}

impl<T> PerAbi<T> {
    /// `f` for every entry of [`ABIS`], in order; the first error ends it.
    pub fn resolve<E>(f: impl FnMut(&'static Abi) -> Result<T, E>) -> Result<PerAbi<T>, E> {
        Ok(PerAbi { by_abi: ABIS.iter().map(f).collect::<Result<_, _>>()? })
    }

    /// The value of the guest ABI in force.
    pub fn get(&self) -> &T {
        self.of(Abi::active())
    }

    /// The value of `abi`'s tables.
    pub fn of(&self, abi: &Abi) -> &T {
        // By name: `ABIS` is a const, so its entries have no one address.
        let i = ABIS
            .iter()
            .position(|a| a.name == abi.name)
            .expect("an entry of ABIS");
        &self.by_abi[i]
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

impl GspTransport {
    /// The guest ABI a transport stands for, as `--guest-abi` and `CARAXES_GUEST_ABI` name it.
    pub fn guest_abi(self) -> u32 {
        match self {
            GspTransport::MsgqV1 => 610,
            GspTransport::MsgqV2 => 615,
        }
    }
}

/// A guest ABI the device serves: the GSP boot, transport and control layouts of one
/// guest driver generation. Every build serves all of them; the guest in front of the
/// device chooses ([`guest`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuestAbi {
    /// 570.144, the firmware nova-core loads (Linux 7.2.7, docs/caraxes/nova-guest.md):
    /// pre-610 queue elements, the GSP_RUN_CPU_SEQUENCER event before GSP_INIT_DONE, the
    /// 570 static info; controls as 610 ([`v570`]).
    V570,
    /// 595.91.07, the Ubuntu driver experiment of docs/caraxes/guest-driver-versions.md:
    /// pre-610 elements and the 595 static info. Only by pin: its elements look like 570's.
    V595,
    /// 610.57.04: message queue v0 with 610 elements and the four init RPCs.
    V610,
    /// 615.71.09: message queue v2, the GMCAPI init and the 615 control layouts. The
    /// 617.14 Windows guest runs on this model.
    V615,
}

impl GuestAbi {
    pub const ALL: [GuestAbi; 4] = [GuestAbi::V570, GuestAbi::V595, GuestAbi::V610, GuestAbi::V615];

    /// As `--guest-abi` and `CARAXES_GUEST_ABI` name it.
    pub fn number(self) -> u32 {
        match self {
            GuestAbi::V570 => 570,
            GuestAbi::V595 => 595,
            GuestAbi::V610 => 610,
            GuestAbi::V615 => 615,
        }
    }

    pub fn from_number(n: u32) -> Option<GuestAbi> {
        GuestAbi::ALL.into_iter().find(|g| g.number() == n)
    }

    /// The CPU-GSP transport the guest speaks.
    pub fn transport(self) -> GspTransport {
        match self {
            GuestAbi::V615 => GspTransport::MsgqV2,
            _ => GspTransport::MsgqV1,
        }
    }

    /// The layouts the device answers the guest with (a 570 or 595 guest's controls are
    /// 610's: nova-core sends few, and the 595 experiment was a 610 build).
    pub fn layouts(self) -> &'static GuestLayouts {
        match self {
            GuestAbi::V615 => &L615,
            _ => &L610,
        }
    }

    /// 570 or 595 for a guest older than 610 (the pre-610 queue elements), else `None`.
    pub fn older(self) -> Option<u32> {
        match self {
            GuestAbi::V570 | GuestAbi::V595 => Some(self.number()),
            _ => None,
        }
    }

    fn encode(self) -> u8 {
        match self {
            GuestAbi::V570 => 1,
            GuestAbi::V595 => 2,
            GuestAbi::V610 => 3,
            GuestAbi::V615 => 4,
        }
    }

    fn decode(v: u8) -> Option<GuestAbi> {
        GuestAbi::ALL.into_iter().find(|g| g.encode() == v)
    }
}

impl std::fmt::Display for GuestAbi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            GuestAbi::V570 => "570 (r570.144, nova-core)",
            GuestAbi::V595 => "595 (595.91.07, experiment)",
            GuestAbi::V610 => "610 (610.57.04)",
            GuestAbi::V615 => "615 (615.71.09; 617 Windows on its model)",
        })
    }
}

/// `NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS` and its entry (`engineData` grew from 16
/// to 17 words in 615).
#[derive(Debug)]
pub struct DevInfoLayout {
    pub params_size: usize,
    pub num_entries: usize,
    pub b_more: usize,
    pub entries: usize,
    pub entry_size: usize,
    pub engine_data: usize,
    pub pbdma_ids: usize,
    pub pbdma_fault_ids: usize,
    pub num_pbdmas: usize,
    pub engine_name: usize,
}

/// `NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS` (1336 bytes per engine in 610,
/// 1356 in 615) and its params.
#[derive(Debug)]
pub struct FsLayout {
    pub elem: usize,
    pub params: usize,
    pub gpc_mask: usize,
    pub tpc_mask: usize,
    pub tpc_count: usize,
    pub phys_gpc_mask: usize,
    pub tpc_to_pes_map: usize,
    /// `tpcToPesMap[]` entries: 10 in 610, 15 in 615.
    pub tpc_to_pes_map_len: usize,
    pub num_pes_per_gpc: usize,
    pub zcull_mask: usize,
    pub phys_gfx_gpc_mask: usize,
    pub num_gfx_tpc: usize,
}

/// The NV0073 items `v615/display.rs` overrides that the display model reads.
#[derive(Debug)]
pub struct DisplayLayouts {
    pub get_active_size: usize,
    pub get_active_stale_sor_index: Option<usize>,
    pub get_active_flags_gop_stale_head: u32,
    pub dp_caps_internal_dp_tunneling: Option<usize>,
    pub cmd_dp_set_war_flags: Option<u32>,
}

/// What differs between the guest ABIs for the code that answers the guest: the items
/// `v615.rs` overrides, one table per version module, built from its constants at compile
/// time ([`L610`], [`L615`]) and chosen at run time ([`layouts`]). The shared rest is
/// [`common`], where these names are private.
#[derive(Debug)]
pub struct GuestLayouts {
    /// `NV_VERSION_STRING` of the version module.
    pub driver: &'static str,
    pub transport: GspTransport,
    /// What 0x20800A36 names in this version.
    pub ctrl_0a36: Id0A36,
    pub devinfo: DevInfoLayout,
    pub p2p_read_caps_status: usize,
    pub p2p_write_caps_status: usize,
    pub fs: FsLayout,
    /// The internal SM-order element (nine u16 per SM in 610, ten in 615).
    pub sm_order: crate::hostabi::SmOrderLayout,
    pub sm_order_params: usize,
    /// `NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS` (20 bytes in 610, 28 in 615).
    pub tsg_size: usize,
    /// `internalFlags` of the TSG allocation, where the struct has it (615).
    pub tsg_internal_flags: Option<usize>,
    pub display: DisplayLayouts,
}

macro_rules! guest_layouts {
    ($m:ident) => {{
        const TSG_SIZE: usize = <$m::RmAbi as nvrm_sys::RmAbi>::TSG_PARAMS_SIZE as usize;
        const TSG_FLAGS: usize = <$m::RmAbi as nvrm_sys::RmAbi>::TSG_PARAMS_OFF_internalFlags;
        GuestLayouts {
            driver: $m::DRIVER_VERSION,
            transport: $m::GSP_TRANSPORT,
            ctrl_0a36: $m::CTRL_0A36,
            devinfo: DevInfoLayout {
                params_size: $m::DEVINFO_PARAMS_SIZE,
                num_entries: $m::DEVINFO_NUM_ENTRIES,
                b_more: $m::DEVINFO_B_MORE,
                entries: $m::DEVINFO_ENTRIES,
                entry_size: $m::DEVINFO_ENTRY_SIZE,
                engine_data: $m::DEVINFO_ENGINE_DATA,
                pbdma_ids: $m::DEVINFO_PBDMA_IDS,
                pbdma_fault_ids: $m::DEVINFO_PBDMA_FAULT_IDS,
                num_pbdmas: $m::DEVINFO_NUM_PBDMAS,
                engine_name: $m::DEVINFO_ENGINE_NAME,
            },
            p2p_read_caps_status: $m::P2P_READ_CAPS_STATUS,
            p2p_write_caps_status: $m::P2P_WRITE_CAPS_STATUS,
            fs: FsLayout {
                elem: $m::FS_ELEM,
                params: $m::FS_PARAMS,
                gpc_mask: $m::FS_GPC_MASK,
                tpc_mask: $m::FS_TPC_MASK,
                tpc_count: $m::FS_TPC_COUNT,
                phys_gpc_mask: $m::FS_PHYS_GPC_MASK,
                tpc_to_pes_map: $m::FS_TPC_TO_PES_MAP,
                tpc_to_pes_map_len: $m::FS_TPC_TO_PES_MAP_LEN,
                num_pes_per_gpc: $m::FS_NUM_PES_PER_GPC,
                zcull_mask: $m::FS_ZCULL_MASK,
                phys_gfx_gpc_mask: $m::FS_PHYS_GFX_GPC_MASK,
                num_gfx_tpc: $m::FS_NUM_GFX_TPC,
            },
            sm_order: $m::SM_ORDER_GUEST,
            sm_order_params: $m::SM_ORDER_PARAMS,
            tsg_size: TSG_SIZE,
            tsg_internal_flags: if TSG_SIZE >= TSG_FLAGS + 4 { Some(TSG_FLAGS) } else { None },
            display: DisplayLayouts {
                get_active_size: $m::display::GET_ACTIVE_SIZE,
                get_active_stale_sor_index: $m::display::GET_ACTIVE_STALE_SOR_INDEX,
                get_active_flags_gop_stale_head: $m::display::GET_ACTIVE_FLAGS_GOP_STALE_HEAD,
                dp_caps_internal_dp_tunneling: $m::display::DP_CAPS_INTERNAL_DP_TUNNELING,
                cmd_dp_set_war_flags: $m::display::CMD_DP_SET_WAR_FLAGS,
            },
        }
    }};
}

/// The 610.57.04 layouts (also the 570 and 595 guests').
pub static L610: GuestLayouts = guest_layouts!(v610);
/// The 615.71.09 layouts (also the 617.14 Windows guest's).
pub static L615: GuestLayouts = guest_layouts!(v615);

/// The guest ABI `--guest-abi` pinned for this process ([`pin_guest`], main.rs at
/// start-up). Unset: none, the guest chooses.
static PIN: std::sync::OnceLock<Option<GuestAbi>> = std::sync::OnceLock::new();
/// The guest ABI the boot model detected last ([`set_guest`]; 0: none yet).
#[cfg_attr(test, allow(dead_code))]
static DETECTED: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

#[cfg(test)]
thread_local! {
    /// A test's guest ABI in force (per test thread), set by [`TestGuest`] or by a boot
    /// model's detection within the test ([`set_guest`]).
    static TEST_GUEST: std::cell::Cell<Option<GuestAbi>> = const { std::cell::Cell::new(None) };
    /// A test's stand-in for `CARAXES_GUEST_ABI` (per test thread): `Some(pin)`. Unset, a
    /// test sees no pin: the environment of the shell that runs the tests does not count.
    static TEST_PIN: std::cell::Cell<Option<Option<GuestAbi>>> = const { std::cell::Cell::new(None) };
}

/// A test's guest ABI and pin for as long as it lives (restored on drop).
#[cfg(test)]
pub(crate) struct TestGuest {
    guest: Option<GuestAbi>,
    pin: Option<Option<GuestAbi>>,
}

#[cfg(test)]
impl TestGuest {
    /// Serve `g` as if the boot model had detected it.
    pub(crate) fn set(g: GuestAbi) -> TestGuest {
        TestGuest {
            guest: TEST_GUEST.with(|c| c.replace(Some(g))),
            pin: TEST_PIN.with(|c| c.get()),
        }
    }

    /// Serve `g` as if `--guest-abi` pinned it (`None`: no pin), nothing detected.
    pub(crate) fn pin(g: Option<GuestAbi>) -> TestGuest {
        TestGuest {
            guest: TEST_GUEST.with(|c| c.replace(None)),
            pin: TEST_PIN.with(|c| c.replace(Some(g))),
        }
    }
}

#[cfg(test)]
impl Drop for TestGuest {
    fn drop(&mut self) {
        TEST_GUEST.with(|c| c.set(self.guest));
        TEST_PIN.with(|c| c.set(self.pin));
    }
}

/// Install the pin of `--guest-abi` ([`check_guest_abi`]); `None`
/// lets the guest choose. Once per process; a second, different pin is an error.
pub fn pin_guest(p: Option<GuestAbi>) -> Result<(), String> {
    let got = *PIN.get_or_init(|| p);
    if got == p {
        Ok(())
    } else {
        Err(format!("guest ABI already pinned as {got:?}, not {p:?}"))
    }
}

/// The pinned guest ABI, `None` when the guest chooses.
pub fn pinned() -> Option<GuestAbi> {
    #[cfg(test)]
    {
        TEST_PIN.with(|c| c.get()).flatten()
    }
    #[cfg(not(test))]
    {
        *PIN.get_or_init(|| None)
    }
}

/// The guest ABI in force: the one the boot model detected, else the pinned one, else 610
/// (the default until a guest shows its own; nothing version-dependent runs before the
/// guest's GSP boot).
pub fn guest() -> GuestAbi {
    #[cfg(test)]
    let detected = TEST_GUEST.with(|c| c.get());
    #[cfg(not(test))]
    let detected = GuestAbi::decode(DETECTED.load(std::sync::atomic::Ordering::Acquire));
    detected.or_else(pinned).unwrap_or(GuestAbi::V610)
}

/// The boot model found guest `g` (`BootModel::detect_guest`, `link_queues`); returns the
/// one in force before.
pub fn set_guest(g: GuestAbi) -> GuestAbi {
    let before = guest();
    #[cfg(test)]
    TEST_GUEST.with(|c| c.set(Some(g)));
    #[cfg(not(test))]
    DETECTED.store(g.encode(), std::sync::atomic::Ordering::Release);
    before
}

/// The layouts of the guest ABI in force.
pub fn layouts() -> &'static GuestLayouts {
    guest().layouts()
}

/// `--guest-abi` (fail closed): it may pin one of 570, 595, 610 and 615. Unset: `None`, the
/// guest chooses (every build serves every guest ABI; the variable `CARAXES_GUEST_ABI` went
/// on 2026-09-30, the launcher passes the flag).
pub fn check_guest_abi(flag: Option<u32>) -> Result<Option<GuestAbi>, String> {
    match flag {
        None => Ok(None),
        Some(n) => GuestAbi::from_number(n)
            .map(Some)
            .ok_or_else(|| format!("guest ABI {n} is none of 570, 595, 610, 615 (617 runs on 615)")),
    }
}

/// The frame of the first command-queue element of message queue v0: where the RPC header
/// ("VRPC", version 3.0) sits tells the generation (docs/caraxes/guest-driver-versions.md
/// §6, detection (b)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementFrame {
    /// `mctpHeader`, `nvdmHeader`, `checkSum`, `seqNum`, the RPC header at 0x10 (610).
    V610,
    /// `authTagBuffer[16]`, `aadBuffer[16]`, `checkSum`, `seqNum`, `elemCount`, the RPC
    /// header at 0x30 (before 610: 570.144, 595.91.07).
    Pre610,
}

/// The frame of the element `raw` (at least 0x38 bytes), `None` when neither place holds
/// an RPC header.
pub fn element_frame(raw: &[u8]) -> Option<ElementFrame> {
    let rpc_at = |o: usize| {
        raw.len() >= o + 8
            && u32::from_le_bytes(raw[o..o + 4].try_into().unwrap()) == common::RPC_HEADER_VERSION
            && u32::from_le_bytes(raw[o + 4..o + 8].try_into().unwrap()) == common::RPC_SIGNATURE
    };
    match (rpc_at(ElementLayout::V610.header_size), rpc_at(ElementLayout::PRE_610.header_size)) {
        (true, false) => Some(ElementFrame::V610),
        (false, true) => Some(ElementFrame::Pre610),
        _ => None,
    }
}

#[cfg(test)]
mod abi_tests {
    use super::*;

    #[test]
    fn the_guest_abi_is_pinned_or_chosen_by_the_guest() {
        assert_eq!(check_guest_abi(None), Ok(None));
        for g in GuestAbi::ALL {
            assert_eq!(check_guest_abi(Some(g.number())), Ok(Some(g)));
        }
        assert!(check_guest_abi(Some(617)).is_err());
        assert!(check_guest_abi(Some(580)).is_err());
    }

    #[test]
    fn the_guest_in_force_is_the_detected_then_the_pinned_then_610() {
        let _g = TestGuest::pin(None);
        assert_eq!(guest(), GuestAbi::V610);
        {
            let _p = TestGuest::pin(Some(GuestAbi::V615));
            assert_eq!((guest(), layouts().driver), (GuestAbi::V615, "615.71.09"));
            assert_eq!(set_guest(GuestAbi::V570), GuestAbi::V615);
            assert_eq!((guest(), older_guest()), (GuestAbi::V570, Some(570)));
        }
        assert_eq!(guest(), GuestAbi::V610, "the guard restores");
        let _s = TestGuest::set(GuestAbi::V615);
        assert_eq!(Abi::active().name, "615");
        assert_eq!(element_layout().max_rpc_size(), crate::msgq::v2::MAX_RPC_SIZE);
    }

    #[test]
    fn every_guest_abi_has_its_transport_and_tables() {
        use GspTransport::{MsgqV1, MsgqV2};
        let want = [(570, MsgqV1, "610"), (595, MsgqV1, "610"), (610, MsgqV1, "610"), (615, MsgqV2, "615")];
        for (g, (n, t, tables)) in GuestAbi::ALL.into_iter().zip(want) {
            assert_eq!((g.number(), g.transport(), Abi::of(g).name), (n, t, tables), "{g:?}");
            assert_eq!(g.layouts().transport, t);
            assert_eq!(GuestAbi::decode(g.encode()), Some(g));
            assert_eq!(GuestAbi::from_number(n), Some(g));
        }
        assert_eq!(L610.driver, v610::DRIVER_VERSION);
        assert_eq!(L615.driver, v615::DRIVER_VERSION);
    }

    /// The per-guest table carries each version module's own numbers, and every number
    /// that differs between the two is one of the table's (the rest is `common`).
    #[test]
    fn the_layout_tables_are_the_version_modules() {
        assert_eq!((L610.fs.params, L615.fs.params), (10688, 10848));
        assert_eq!((L610.fs.tpc_to_pes_map_len, L615.fs.tpc_to_pes_map_len), (10, 15));
        assert_eq!((L610.sm_order_params, L615.sm_order_params), (73760, 81952));
        assert_eq!((L610.sm_order.entry_size, L615.sm_order.entry_size), (18, 20));
        assert_eq!((L610.devinfo.params_size, L615.devinfo.params_size), (3212, 3340));
        assert_eq!((L610.tsg_size, L615.tsg_size), (20, 28));
        assert_eq!((L610.tsg_internal_flags, L615.tsg_internal_flags), (None, Some(24)));
        assert_eq!((L610.ctrl_0a36, L615.ctrl_0a36), (Id0A36::GetChipInfo, Id0A36::SyncFabricProbeInfo));
        assert_eq!((L610.display.get_active_size, L615.display.get_active_size), (16, 20));
        assert_eq!((L610.display.cmd_dp_set_war_flags, L615.display.cmd_dp_set_war_flags), (None, Some(0x73_1391)));
        assert_eq!(L610.sm_order, crate::hostabi::guest_layouts::GUEST_SM_ORDER_610);
        assert_eq!(L615.sm_order, crate::hostabi::guest_layouts::GUEST_SM_ORDER_615);
    }

    /// `common` hides exactly what `v615.rs` and `v615/display.rs` override (common.rs):
    /// a new override without a private stand-in there would let generic code read the 610
    /// number for a 615 guest.
    #[test]
    fn the_shared_vocabulary_hides_every_615_override() {
        fn items(src: &str) -> Vec<&str> {
            src.lines()
                .filter_map(|l| {
                    let l = l.trim_start();
                    ["pub const ", "pub type ", "pub static "]
                        .iter()
                        .find_map(|p| l.strip_prefix(p))
                        .map(|r| r.split(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap_or(""))
                })
                .collect()
        }
        let common = include_str!("common.rs");
        let (display_part, top_part) = {
            let at = common.find("pub mod display").unwrap();
            (&common[at..], &common[..at])
        };
        let hidden = |part: &str, n: &str| {
            part.lines().any(|l| {
                let t = l.trim().trim_end_matches(',');
                t == n || l.trim_start().starts_with(&format!("type {n} "))
            })
        };
        // New in 615: no 610 item of that name to hide.
        let new_in_615 = [
            "CMD_INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO",
            "FABRIC_PROBE_INFO_SIZE",
            "WAR_FLAGS_SIZE",
        ];
        let (top, display) = (items(include_str!("v615.rs")), items(include_str!("v615/display.rs")));
        for n in ["DRIVER_VERSION", "RmAbi", "FS_PARAMS", "SM_ORDER_GUEST", "DEVINFO_ENGINE_NAME"] {
            assert!(top.contains(&n), "the scan finds {n} in v615.rs");
        }
        assert!(display.contains(&"GET_ACTIVE_SIZE") && display.contains(&"CMD_DP_SET_WAR_FLAGS"));
        let mut missing = Vec::new();
        for n in top {
            if !new_in_615.contains(&n) && !hidden(top_part, n) {
                missing.push(n.to_string());
            }
        }
        for n in display {
            if !new_in_615.contains(&n) && !hidden(display_part, n) {
                missing.push(format!("display::{n}"));
            }
        }
        assert!(missing.is_empty(), "v615 overrides not hidden in common.rs: {missing:?}");
    }

    #[test]
    fn the_element_frame_tells_610_from_older() {
        let rpc = |at: usize| {
            let mut raw = vec![0u8; 0x60];
            raw[at..at + 4].copy_from_slice(&common::RPC_HEADER_VERSION.to_le_bytes());
            raw[at + 4..at + 8].copy_from_slice(&common::RPC_SIGNATURE.to_le_bytes());
            raw
        };
        assert_eq!(element_frame(&rpc(0x10)), Some(ElementFrame::V610));
        assert_eq!(element_frame(&rpc(0x30)), Some(ElementFrame::Pre610));
        assert_eq!(element_frame(&[0u8; 0x60]), None);
        assert_eq!(element_frame(&rpc(0x10)[..0x20]), Some(ElementFrame::V610));
        assert_eq!(element_frame(&rpc(0x30)[..0x20]), None, "too short to tell");
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

/// Queue-element layout of the guest's driver. The element header is the first thing
/// guests of different versions disagree on (docs/caraxes/guest-driver-versions.md):
/// before 610 it is `authTagBuffer[16]`, `aadBuffer[16]`, `checkSum`, `seqNum`,
/// `elemCount` and the RPC header at 0x30 (595.91.07 `message_queue_priv.h:43-51`); from
/// 610 on `mctpHeader`, `nvdmHeader`, `checkSum`, `seqNum` and the payload at 0x10
/// (610.57.04 `message_queue_priv.h:52-67`); from 615 on the 24-byte message queue v2
/// header (`msgq::v2`). [`element_layout`] gives the guest's.
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

    /// 610: `mctpHeader`, `nvdmHeader`, `checkSum`, `seqNum`.
    pub const V610: ElementLayout = ElementLayout {
        header_size: v610::ELEMENT_HEADER_SIZE,
        checksum_off: 8,
        seq_off: 12,
        elem_count_off: None,
        mctp: true,
    };
    /// 615 (message queue v2): only `header_size` is read here (`max_rpc_size`); the v2
    /// element has no checksum or sequence number and is parsed by `msgq::v2`.
    pub const V2: ElementLayout = ElementLayout {
        header_size: crate::msgq::v2::ELEMENT_HEADER_SIZE,
        ..ElementLayout::V610
    };
    pub const PRE_610: ElementLayout = ElementLayout {
        header_size: 0x30,
        checksum_off: 0x20,
        seq_off: 0x24,
        elem_count_off: Some(0x28),
        mctp: false,
    };
}

/// The guest older than 610 in force ([`guest`]): 570 (570.144, the firmware Linux
/// 7.2.7's nova-core loads, docs/caraxes/nova-guest.md; detected from its pre-610 queue
/// elements) or 595 (the Ubuntu driver experiment of docs/caraxes/guest-driver-versions.md;
/// by pin only). `None`: 610 or 615.
pub fn older_guest() -> Option<u32> {
    guest().older()
}

/// The element layout of the guest in force: 570.144 frames its elements as 595 does
/// (`GSP_MSG_QUEUE_ELEMENT` with the RPC header at 0x30, 570.144
/// `src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h`; nova-core
/// `gsp/fw/r570_144/bindings.rs:868-876` of Linux 7.2.7).
pub fn element_layout() -> ElementLayout {
    match guest() {
        GuestAbi::V570 | GuestAbi::V595 => ElementLayout::PRE_610,
        GuestAbi::V610 => ElementLayout::V610,
        GuestAbi::V615 => ElementLayout::V2,
    }
}

/// `rpc_gsp_rm_control_v03_00` as a guest ABI frames it. `hClient`, `hObject`, `cmd`,
/// `status`, `paramsSize` and the RPC flags word sit at 0 to 20 in every version; what
/// follows differs:
/// - 570.144: nothing, the params start at 24 (`flags` at 20; 570.144
///   `src/nvidia/generated/g_rpc-structures.h:1388-1397`; nouveau's
///   `nvkm/subdev/gsp/rm/r535/nvrm/ctrl.h:11-20` of Linux 7.2.7, used by its r570 path);
/// - 595.91.07 and later: `rmctrlFlags` at 24, `rmctrlAccessRight` at 28, 8 reserved bytes,
///   the params at 40 (595.91.07 `g_rpc-structures.h:1545-1557`, [`v610::RM_CONTROL_PARAMS`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RmControlFrame {
    /// Where the params start: the size of the fixed part.
    pub params: usize,
    /// `rmctrlFlags` (GSS legacy caching), where the frame has it.
    pub rmctrl_flags: Option<usize>,
    /// `rmctrlAccessRight`, where the frame has it.
    pub access_right: Option<usize>,
}

impl RmControlFrame {
    pub const V570: RmControlFrame = RmControlFrame {
        params: v570::RM_CONTROL_PARAMS,
        rmctrl_flags: None,
        access_right: None,
    };
    pub const V610: RmControlFrame = RmControlFrame {
        params: v610::RM_CONTROL_PARAMS,
        rmctrl_flags: Some(v610::RM_CONTROL_RMCTRL_FLAGS),
        access_right: Some(v610::RM_CONTROL_RMCTRL_ACCESS_RIGHT),
    };
}

impl GuestAbi {
    /// The `GSP_RM_CONTROL` frame this guest sends and reads back.
    pub fn rm_control(self) -> RmControlFrame {
        match self {
            GuestAbi::V570 => RmControlFrame::V570,
            GuestAbi::V595 | GuestAbi::V610 | GuestAbi::V615 => RmControlFrame::V610,
        }
    }
}

/// The `GSP_RM_CONTROL` frame of the guest in force ([`guest`]).
pub fn rm_control() -> RmControlFrame {
    guest().rm_control()
}

/// RM alloc parameters of the guest in force in the layout the device handles (610.57.04),
/// or `None` when they need no translation: a 570.144 guest's channel and VA space
/// parameters ([`v570::alloc_params_up`]); every other guest ABI and class as they are.
pub fn alloc_params_to_device(h_class: u32, params: &[u8]) -> Option<Vec<u8>> {
    match guest() {
        GuestAbi::V570 => v570::alloc_params_up(h_class, params),
        _ => None,
    }
}

/// The device's answer to an alloc (610.57.04 layout) in the layout the guest in force
/// reads, or `None` when it needs no translation ([`v570::alloc_params_down`]).
pub fn alloc_params_to_guest(h_class: u32, params: &[u8]) -> Option<Vec<u8>> {
    match guest() {
        GuestAbi::V570 => v570::alloc_params_down(h_class, params),
        _ => None,
    }
}

/// The events the device sends when it links message queue v0, in order: GSP_INIT_DONE
/// for a 610 guest (`rpc_init_done_v17_00`, 1 byte in 610); for a 570 guest first an empty
/// GSP_RUN_CPU_SEQUENCER and then GSP_INIT_DONE, because nova-core waits for the sequencer
/// first and consumes and drops every message it is not waiting for (Linux 7.2.7 nova-core
/// `gsp/hal/tu102.rs:335-356`, then `gsp/boot.rs:153-154`; `gsp/cmdq.rs:813-848`).
pub fn link_events(guest: Option<u32>) -> Vec<(u32, Vec<u8>)> {
    match guest {
        Some(570) => vec![
            (v570::EV_GSP_RUN_CPU_SEQUENCER, v570::empty_sequence()),
            (common::EV_GSP_INIT_DONE, vec![0u8; v570::INIT_DONE_SIZE]),
        ],
        _ => vec![(common::EV_GSP_INIT_DONE, vec![0u8])],
    }
}

/// `GspStaticConfigInfo` as a pre-610 guest reads it (595 by pin, 570 detected):
/// 595.91.07 lacks `vbiosRevision`/`vbiosOemRevision` (610: 0x5fc, 0x600),
/// `bSysL2CacheCoherentMode` (610: 0x616) and `bBusResetRequired` (610: 0x638), so
/// its tail sits 8 bytes lower and the struct is 0x638 bytes, not 0x640
/// (gsp-abi-sizes of both trees, docs/caraxes/guest-driver-versions.md). Everything
/// before 0x5fc is identical. Unchanged for a 610 guest.
pub fn static_info_for_guest(info: Vec<u8>) -> Vec<u8> {
    static_info_for(older_guest(), info)
}

/// [`static_info_for_guest`] for a given older guest (the tests pass it; the device uses
/// [`older_guest`]). A 570 guest gets the 570.144 layout, remapped field by field by the
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

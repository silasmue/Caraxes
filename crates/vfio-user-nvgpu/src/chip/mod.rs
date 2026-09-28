// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The silicon axis (decision D14): what differs between chips, behind the [`Chip`]
//! trait. A chip supplies its tables as data ([`ChipData`], loaded once from
//! `chips/<chip>/*.toml`) and its firmware-boot behaviour as code
//! ([`FirmwareBoot`]). Generic code (the BAR0 window, the queue and RPC service, the
//! phase-5 dispatcher) asks the chip and never names a register, engine or class of
//! its own. TU106 is the first implementation ([`tu106`]); how to add another chip:
//! docs/caraxes/versatility.md.
//!
//! Columns that are driver enums (NV2080_ENGINE_TYPE, RM_ENGINE_TYPE, ENG_DESC,
//! MC_ENGINE_IDX, NV2080_INTR_CATEGORY, the RAM type) are written by name in the
//! chip files and resolved to numbers through the driver-version module
//! ([`crate::ver`]) when the chip is loaded; an unknown name refuses the chip.

use serde::Deserialize;

use crate::guestmem::GuestMemory;

pub mod boot_fsp;
pub mod derived;
pub mod physical;
pub mod gb206;
pub mod tu106;
pub mod turing;

/// Every chip this build can present, by `--chip` name.
/// `tu104`, `ga106`, `ad106` and `gh100` are the costume ladder's derived chips
/// ([`derived`], docs/caraxes/costume-ladder.md).
pub const CHIPS: &[&str] = &["tu106", "gb206", "tu104", "ga106", "ad106", "gh100", "tu106vf"];

/// A chip the device can present.
pub trait Chip: Sync {
    /// The tables: identity, register map, engines, interrupts, FIFO device info.
    fn data(&self) -> &ChipData;
    /// A fresh firmware-boot model in its post-GFW state, as a function-level reset
    /// leaves the card (GSP/FWSEC/Booter side effects for Turing).
    fn firmware_boot(&'static self, fb_bytes: u64) -> Box<dyn FirmwareBoot>;
}

/// The chip named `name` (`--chip`), loaded and validated once.
pub fn by_name(name: &str) -> Result<&'static dyn Chip, String> {
    match name {
        "tu106" => tu106::get(),
        "gb206" => gb206::get(),
        other if derived::get(other).is_some() => derived::get(other).unwrap(),
        other => Err(format!(
            "unknown chip {other:?}; this build knows {}",
            CHIPS.join(", ")
        )),
    }
}

/// The CPU-visible side effects of a firmware boot that never runs firmware.
pub trait FirmwareBoot: Send {
    /// True when the dword-aligned BAR0 offset belongs to this model.
    fn handles(&self, off: u64) -> bool;
    /// The value of an owned register (`None` when not owned).
    fn read32(&mut self, off: u64) -> Option<u32>;
    /// Write an owned register (`None` when not owned). Firmware side effects that
    /// the generic model has to act on come back as a [`BootEvent`].
    fn write32(&mut self, off: u64, v: u32, mem: &GuestMemory) -> Option<BootWrite>;
    /// Where the CPU put the libos init arguments for the GSP-RM it boots (on Turing
    /// the GSP falcon's MAILBOX0/1, `kgspProgramLibosBootArgsAddr_TU102`). The 615
    /// transport links its queues when GSP-RM starts, before any doorbell
    /// (docs/caraxes/gsp-transport-615.md §3). `None` when the model does not know.
    fn libos_args_pa(&self) -> Option<u64> {
        None
    }
    /// The four registers that hold the message-queue pointers from driver 615 on
    /// (message queue v2), when the chip has them.
    fn queue_registers(&self) -> Option<QueueRegisters> {
        None
    }
    /// A write to a register the model watches but does not own (it latches in BAR0 and
    /// reads back as written, or from the capture before): `NV_PMC_ENABLE`, whose rising
    /// engine bits reset falcons. `old` is what the register read before. Notes for the trace.
    fn observe_write(&mut self, _off: u64, _old: u32, _new: u32) -> Vec<String> {
        Vec::new()
    }
}

/// `NV_PGSP_QUEUE_HEAD/TAIL(0)` and `NV_PGSP_MSGQ_HEAD/TAIL(0)`: the CPU writes the
/// command queue's write pointer to `cmd_head` and the status queue's read pointer to
/// `msg_tail`, and reads the GSP's pointers from `cmd_tail` and `msg_head`
/// (msgq_priv.h:55-63 and kern_gpu_tu102.c:596-605 of 615).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueRegisters {
    pub cmd_head: u64,
    pub cmd_tail: u64,
    pub msg_head: u64,
    pub msg_tail: u64,
}

/// What a firmware-boot register write did.
#[derive(Debug, Default)]
pub struct BootWrite {
    /// Trace notes, in order.
    pub notes: Vec<String>,
    pub event: Option<BootEvent>,
}

/// Firmware-boot milestones the generic model reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootEvent {
    /// The loader handed over the `GspFwWprMeta` at `pa` and GSP-RM is "running".
    WprMeta { pa: u64 },
    /// The CPU rang the GSP command-queue doorbell; the libos init arguments are at `pa`.
    CmdQueueDoorbell { libos_args_pa: u64 },
    /// Booter Unload ran with `arg` in the SEC2 mailboxes: 0, or the `GspFwSRMeta` of a
    /// suspend (`_kgspGetBooterUnloadArgs`, kernel_gsp_tu102.c:491-515 of 615).
    BooterUnload { arg: u64 },
}

/// The three boot registers that select the chip and the runtime variant
/// (docs/caraxes/driver-modes.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct BootIdentity {
    pub boot_0: u32,
    pub boot_1: u32,
    pub boot_42: u32,
}

/// `chip.toml` plus the table files, as one value.
#[derive(Debug)]
pub struct ChipData {
    pub name: String,
    pub identity: BootIdentity,
    /// `gpuShortNameString` (GspStaticConfigInfo) and the log name.
    pub short_name: String,
    pub bar0: Bar0Windows,
    pub registers: Registers,
    pub engine_caps: EngineCaps,
    pub chip_info: ChipInfo,
    pub user_register_access_map: Labelled<u32>,
    pub memsys: Memsys,
    pub fifo: Fifo,
    pub gmmu: Gmmu,
    pub ce: Ce,
    pub gr: Gr,
    pub channel: ChannelClasses,
    /// The classes the kernel RM allows on this chip (`[classes]` of chip.toml,
    /// classes.rs); `None` for a chip without the table: every class is acknowledged.
    pub classes: Option<ClassDb>,
    pub device_info: DeviceInfo,
    pub interrupts: Interrupts,
    /// BAR0 names for the trace, first match wins.
    pub regmap: Vec<RegName>,
    /// The feature families the chip lacks (category E, chipctl.rs).
    pub features: crate::chipctl::Features,
    /// The chip-data answers of category A (`controls.toml`, chipctl.rs).
    pub controls: crate::chipctl::ControlTable,
    /// `[ecc]`: the ECC facts of a board with ECC (families/ecc.rs); `None` without.
    pub ecc: Option<crate::families::ecc::EccData>,
    /// `[mig]`: the MIG profile tables (families/mig.rs); `None` without MIG.
    pub mig: Option<crate::families::mig::MigData>,
    /// `[compression]`: the card's compression facts (compression.rs); `None` for a chip
    /// whose card was never read: its guest is told no compression, whatever the switch.
    pub compression: Option<crate::compression::ComprData>,
}

impl ChipData {
    /// Name of the register or block at `offset`, from the chip's register map.
    /// `None` means the offset is not in the map yet: the trace summary lists those.
    pub fn register_name(&self, offset: u64) -> Option<&str> {
        self.regmap
            .iter()
            .find(|r| (r.lo..r.hi).contains(&offset))
            .map(|r| r.name.as_str())
    }

    /// The `NV2080_ENGINE_TYPE` bitmask of `engineCaps`, `words` u32 wide.
    pub fn engine_caps_mask(&self, words: usize) -> Vec<u32> {
        let mut caps = vec![0u32; words];
        for &e in &self.engine_caps.types {
            if let Some(w) = caps.get_mut((e / 32) as usize) {
                *w |= 1 << (e % 32);
            }
        }
        caps
    }
}

/// The windows every BAR0 access passes before any model.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bar0Windows {
    /// `NV_PCFG`: the PCI config-space mirror the kernel RM reads.
    pub pcfg: u64,
    pub pcfg_size: u64,
    /// `NV_PROM_DATA`: the VBIOS window.
    pub prom: u64,
    pub prom_size: u64,
}

/// The kernel-RM hardware surface outside the firmware boot (docs/caraxes/
/// kernel-rm-hw-surface.md): windows, BAR blocks, TLB invalidate, the interrupt tree,
/// the VF fault-buffer pointers and the usermode window.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registers {
    pub bar0_window: u64,
    pub bar1_block: u64,
    pub bar2_block: u64,
    pub pramin: u64,
    pub pramin_size: u64,
    pub mmu_invalidate_pdb: u64,
    pub mmu_invalidate_upper_pdb: u64,
    pub mmu_invalidate: u64,
    pub intr_tree: u64,
    pub intr_tree_size: u64,
    pub fault_buffer_get: [u64; 2],
    pub fault_buffer_put: [u64; 2],
    pub access_counter_notify_buffer_size: u64,
    pub access_counter_get: u64,
    pub access_counter_put: u64,
    pub access_counter_info: u64,
    pub usermode_time_0: u64,
    pub usermode_time_1: u64,
    pub usermode_doorbell: u64,
    /// `NV_UFLUSH_L2_FLUSH_DIRTY`.
    pub l2_flush_dirty: u64,
    /// `NV_UFLUSH_L2_SYSMEM_INVALIDATE` and its VF-window alias.
    pub l2_sysmem_invalidate: [u64; 2],
    /// The other cache-operation registers, completed without a host operation.
    pub l2_ops_local: [u64; 4],
}

/// A value with the provenance label its answer carries (decision D9).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Labelled<T> {
    pub value: T,
    pub label: String,
}

/// `engineCaps` of the static info: the chip's engines as `NV2080_ENGINE_TYPE` numbers.
#[derive(Clone, Debug)]
pub struct EngineCaps {
    pub types: Vec<u32>,
    /// The engine list in words, for the trace.
    pub summary: String,
    pub source: String,
}

/// `INTERNAL_GPU_GET_CHIP_INFO`: the register bases the chip reports.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChipInfo {
    pub timer_base: u32,
    pub usermode_base: u32,
    pub label: String,
}

/// `INTERNAL_MEMSYS_GET_STATIC_CONFIG`.
#[derive(Clone, Debug)]
pub struct Memsys {
    pub disable_compbit_backing: bool,
    pub l2_cache_size: u64,
    pub fbpa_present: bool,
    pub compr_page_size: u32,
    pub compr_page_shift: u32,
    /// `NV2080_CTRL_FB_INFO_RAM_TYPE_*`.
    pub ram_type: u32,
    pub ltc_count: u32,
    pub lts_per_ltc_count: u32,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fifo {
    pub num_channels: Labelled<u32>,
    pub fault_method_buffer_size: Labelled<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gmmu {
    /// The page-table format `fb.rs` walks; the device refuses a chip whose format it
    /// does not implement ([`MMU_FORMATS`]).
    pub mmu_format: String,
    /// Entries of the replayable and non-replayable fault buffers (GMMU static info)
    /// and of the access-counter notify buffer.
    pub fault_buffer_entries: u32,
    /// Bytes per fault-buffer entry.
    pub fault_entry_size: u32,
    pub label: String,
}

/// The GMMU formats `fb.rs` implements.
pub const MMU_FORMATS: &[&str] = &["gmmu-v2-49bit"];

/// GMMU formats fb.rs's own walker does not know. `gmmu-v3-57bit` (Hopper and Blackwell)
/// is walked by [`crate::gmmu_v3`] for the BAR1/BAR2 apertures and the leaf enumeration
/// (fb.rs `with_mmu_format`); the incremental shadow walker (ptcache.rs), used only with
/// host execution, stays v2 (versatility.md, "GMMU format").
pub const MMU_FORMATS_UNWALKED: &[&str] = &["gmmu-v3-57bit"];

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ce {
    /// A decompression LCE exists (Ampere GA100 and later).
    pub decomp_lce: bool,
    pub decomp_label: String,
    /// How LCEs group into CE shims (`CE_GET_LCE_SHIM_INFO`), when the device answers it
    /// without the host (cuda.rs). `None`: not known for this chip (the control is then
    /// NOT_SUPPORTED without a host that may answer it).
    #[serde(default)]
    pub lce_shim: Option<LceShim>,
}

/// LCE `i` is in shim `i / lces_per_shim` at local index `i % lces_per_shim`: the
/// `kceSetShimInstance_GB100` rule (publicID / group-id stride, 615
/// src/kernel/gpu/ce/arch/blackwell/kernel_ce_gb100.c:105-112). Chips before Blackwell
/// keep every KernelCE in shim 0 (kernel_ce.c:53-56, the no-op HAL of
/// g_kernel_ce_nvoc.c:352-362), which a stride of 20 (every LCE an NV2080 engine type can
/// name) expresses.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LceShim {
    pub lces_per_shim: u32,
    pub label: String,
}

impl LceShim {
    /// (shimInstance, shimLocalLceIdx) of LCE `lce`.
    pub fn of(&self, lce: u32) -> (u32, u32) {
        let s = self.lces_per_shim.max(1);
        (lce / s, lce % s)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gr {
    pub per_subctx_header: Labelled<bool>,
    /// SMC/MIG partitioning exists.
    pub mig: bool,
    pub smc_label: String,
    /// The `NV0080_CTRL_GR_INFO_INDEX_*` values the device answers
    /// `INTERNAL_STATIC_KGR_GET_INFO` with when no host GR answers it (pure emulation):
    /// `[index, data]` pairs, every other index 0 (kgr.rs).
    #[serde(default)]
    pub emulated_info: Option<GrInfoTable>,
}

/// `[gr.emulated_info]`: GR info entries with their provenance label.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrInfoTable {
    pub entries: Vec<[u32; 2]>,
    pub label: String,
}

/// The chip's own channel classes, which the channel emulation executes.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelClasses {
    pub gpfifo: u32,
    pub copy: u32,
    pub uvm_sw: u32,
    pub compute: u32,
}

/// The chip's class database: the generic list plus the chip's no-engine and engine
/// lists of the generated `g_gpu_class_list.c` (classes.rs).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassDb {
    pub source: String,
    pub ids: Vec<u32>,
}

/// The FIFO device-info table (`FIFO_GET_DEVICE_INFO_TABLE`).
#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub label: String,
    pub engines: Vec<DeviceInfoEntry>,
}

/// One engine of the device-info table, with the driver enums resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfoEntry {
    pub name: String,
    pub eng_desc: u32,
    pub rm_engine_type: u32,
    pub runlist: u32,
    pub mmu_fault_id: u32,
    pub mc_idx: u32,
    pub instance: u32,
    pub host_driven: u32,
    pub dev_type: u32,
    /// (PBDMA id, PBDMA fault id) pairs.
    pub pbdma: Vec<[u32; 2]>,
}

/// The kernel interrupt table (`INTERNAL_INTR_GET_KERNEL_TABLE`).
#[derive(Clone, Debug)]
pub struct Interrupts {
    pub label: String,
    pub entries: Vec<IntrEntry>,
    /// (category index, subtree mask).
    pub subtrees: Vec<(usize, u64)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntrEntry {
    pub mc_idx: u16,
    pub vector_stall: u32,
    pub vector_nonstall: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegName {
    pub lo: u64,
    pub hi: u64,
    pub name: String,
}

/// The source texts of one chip's table files.
pub struct ChipFiles<'a> {
    pub chip: &'a str,
    pub regmap: &'a str,
    pub device_info: &'a str,
    pub interrupts: &'a str,
    pub controls: &'a str,
}

// The file formats. Driver enums are strings here and numbers in [`ChipData`].

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChipFile {
    name: String,
    short_name: String,
    identity: BootIdentity,
    bar0: Bar0Windows,
    registers: Registers,
    engine_caps: EngineCapsFile,
    chip_info: ChipInfo,
    user_register_access_map: Labelled<u32>,
    memsys: MemsysFile,
    fifo: Fifo,
    gmmu: Gmmu,
    ce: Ce,
    gr: Gr,
    channel: ChannelClasses,
    #[serde(default)]
    classes: Option<ClassDb>,
    features: crate::chipctl::FeaturesFile,
    #[serde(default)]
    ecc: Option<crate::families::ecc::EccData>,
    #[serde(default)]
    mig: Option<crate::families::mig::MigData>,
    #[serde(default)]
    compression: Option<crate::compression::ComprData>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EngineCapsFile {
    types: Vec<String>,
    summary: String,
    source: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemsysFile {
    disable_compbit_backing: bool,
    l2_cache_size: u64,
    fbpa_present: bool,
    compr_page_size: u32,
    compr_page_shift: u32,
    ram_type: String,
    ltc_count: u32,
    lts_per_ltc_count: u32,
    label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegmapFile {
    register: Vec<RegName>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceInfoFile {
    label: String,
    engine: Vec<DeviceInfoEntryFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceInfoEntryFile {
    name: String,
    /// [NVOC class name, instance].
    eng_desc: (String, u32),
    rm_engine_type: String,
    runlist: u32,
    mmu_fault_id: u32,
    mc: String,
    instance: u32,
    host_driven: u32,
    dev_type: u32,
    pbdma: Vec<[u32; 2]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InterruptsFile {
    label: String,
    entry: Vec<IntrEntryFile>,
    subtree: Vec<IntrSubtreeFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntrEntryFile {
    mc: String,
    vector_stall: u32,
    vector_nonstall: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntrSubtreeFile {
    category: String,
    mask: u64,
}

fn parse<T: serde::de::DeserializeOwned>(file: &str, text: &str) -> Result<T, String> {
    toml::from_str(text).map_err(|e| format!("{file}: {e}"))
}

/// The number the emulated driver version gives `name` in `table` (`what` names the
/// enum for the error).
fn resolve(table: &[(&str, u32)], what: &str, name: &str) -> Result<u32, String> {
    crate::ver::active::lookup(table, name).ok_or_else(|| {
        format!(
            "{what} {name:?} has no number in driver {}",
            crate::ver::active::DRIVER_VERSION
        )
    })
}

impl ChipData {
    /// Parse and resolve one chip's files (`chips/<chip>/*.toml`).
    pub fn load(f: &ChipFiles) -> Result<Self, String> {
        use crate::ver::active as v;
        let c: ChipFile = parse("chip.toml", f.chip)?;
        let regmap: RegmapFile = parse("regmap.toml", f.regmap)?;
        let di: DeviceInfoFile = parse("device-info.toml", f.device_info)?;
        let intr: InterruptsFile = parse("interrupts.toml", f.interrupts)?;
        let format = c.gmmu.mmu_format.as_str();
        if !MMU_FORMATS.contains(&format) && !MMU_FORMATS_UNWALKED.contains(&format) {
            return Err(format!(
                "chip {}: GMMU format {:?} is not implemented (fb.rs knows {})",
                c.name,
                c.gmmu.mmu_format,
                MMU_FORMATS.join(", ")
            ));
        }
        let types = c
            .engine_caps
            .types
            .iter()
            .map(|t| resolve(v::NV2080_ENGINE_TYPES, "NV2080_ENGINE_TYPE", t))
            .collect::<Result<Vec<_>, _>>()?;
        let m = c.memsys;
        let memsys = Memsys {
            disable_compbit_backing: m.disable_compbit_backing,
            l2_cache_size: m.l2_cache_size,
            fbpa_present: m.fbpa_present,
            compr_page_size: m.compr_page_size,
            compr_page_shift: m.compr_page_shift,
            ram_type: resolve(v::RAM_TYPES, "RAM type", &m.ram_type)?,
            ltc_count: m.ltc_count,
            lts_per_ltc_count: m.lts_per_ltc_count,
            label: m.label,
        };
        let engines = di
            .engine
            .into_iter()
            .map(|e| {
                let class = resolve(v::ENG_DESC_CLASSES, "engine class", &e.eng_desc.0)?;
                Ok(DeviceInfoEntry {
                    eng_desc: (class << 8) | e.eng_desc.1,
                    rm_engine_type: resolve(
                        v::RM_ENGINE_TYPES,
                        "RM_ENGINE_TYPE",
                        &e.rm_engine_type,
                    )?,
                    mc_idx: resolve(v::MC_ENGINE_IDX, "MC_ENGINE_IDX", &e.mc)?,
                    name: e.name,
                    runlist: e.runlist,
                    mmu_fault_id: e.mmu_fault_id,
                    instance: e.instance,
                    host_driven: e.host_driven,
                    dev_type: e.dev_type,
                    pbdma: e.pbdma,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if engines.len() > v::DEVINFO_MAX_ENTRIES {
            return Err(format!(
                "device-info.toml: {} engines, at most {}",
                engines.len(),
                v::DEVINFO_MAX_ENTRIES
            ));
        }
        let entries = intr
            .entry
            .iter()
            .map(|e| {
                Ok(IntrEntry {
                    mc_idx: resolve(v::MC_ENGINE_IDX, "MC_ENGINE_IDX", &e.mc)? as u16,
                    vector_stall: e.vector_stall,
                    vector_nonstall: e.vector_nonstall,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let subtrees = intr
            .subtree
            .iter()
            .map(|s| {
                Ok((
                    resolve(v::INTR_CATEGORIES, "NV2080_INTR_CATEGORY", &s.category)? as usize,
                    s.mask,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        // A family table needs its family present (families/mod.rs).
        for (table, fams, given) in [
            ("ecc", ["ecc", "row_remap"], c.ecc.is_some()),
            ("mig", ["mig", "mig_partitioning"], c.mig.is_some()),
        ] {
            for fam in fams {
                if given && c.features.absent.iter().any(|a| a == fam) {
                    return Err(format!(
                        "chip {}: chip.toml has [{table}], but names family {fam:?} absent",
                        c.name
                    ));
                }
            }
        }
        Ok(ChipData {
            ecc: c.ecc,
            mig: c.mig,
            compression: c.compression,
            name: c.name,
            identity: c.identity,
            short_name: c.short_name,
            bar0: c.bar0,
            registers: c.registers,
            engine_caps: EngineCaps {
                types,
                summary: c.engine_caps.summary,
                source: c.engine_caps.source,
            },
            chip_info: c.chip_info,
            user_register_access_map: c.user_register_access_map,
            memsys,
            fifo: c.fifo,
            gmmu: c.gmmu,
            ce: c.ce,
            gr: c.gr,
            channel: c.channel,
            classes: c.classes,
            device_info: DeviceInfo {
                label: di.label,
                engines,
            },
            interrupts: Interrupts {
                label: intr.label,
                entries,
                subtrees,
            },
            regmap: regmap.register,
            // Category E families and the category A table, for the emulated driver
            // version's control ids and layouts (chipctl.rs).
            features: crate::chipctl::Features::resolve(crate::ver::Abi::active(), &c.features)
                .map_err(|e| format!("chip.toml {e}"))?,
            controls: crate::chipctl::ControlTable::resolve(
                crate::ver::Abi::active(),
                f.controls,
            )?,
        })
    }
}

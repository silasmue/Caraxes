// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The feature-family models (docs/caraxes/gsp-plan/e-families.md §4, "the change";
//! docs/caraxes/limits-mig-ecc-vgpu.md): a family the chip names present in `[features]`
//! and describes in its own chip-file table is answered here, ahead of the host forwards of
//! nvml.rs and public.rs and ahead of the refusal rules of chipctl.rs. A family without its
//! table stays as before: refused when absent, unanswered when present.
//!
//! * `ecc` and `row_remap` ([`ecc`]): the ECC state of a data-center board, the mode
//!   current and pending, the counters (zero), the empty row-remapper tables.
//! * `mig` and `mig_partitioning` ([`mig`]): the MIG mode and the GPU and compute
//!   instances as per-VM bookkeeping.
//!
//! Nothing here reaches the host: in pure emulation there is none, and with a host the
//! executing card (the RTX 2070) has neither feature, so a host answer would be the wrong
//! card's. The settings a real card keeps in its InfoROM across boots (the ECC mode and the
//! MIG mode, both "pending until the next reset") are the [`Persist`] record: carried
//! across a function-level reset of the device (`BootModel::reset`), and across a restart of
//! the device process when the device was started with `--persist <file>`.

pub mod ecc;
pub mod mig;

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::chip::ChipData;
use crate::ctrl::Answer;
use crate::ver::Abi;

/// A mode a real card stores in its InfoROM: what the next reset applies (`pending`) and
/// what is in force now (`current`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mode {
    pub current: bool,
    pub pending: bool,
}

impl Mode {
    fn new(on: bool) -> Self {
        Mode {
            current: on,
            pending: on,
        }
    }

    /// A GPU reset or a power-on: the pending setting takes effect.
    fn reset(&mut self) {
        self.current = self.pending;
    }
}

/// The persistent settings of one VM's device (the InfoROM analogue).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Persist {
    /// The ECC mode; `None` until the chip's default is taken.
    #[serde(default)]
    pub ecc: Option<Mode>,
    /// The MIG mode; `None` until the chip's default (off) is taken.
    #[serde(default)]
    pub mig: Option<Mode>,
}

static PERSIST_PATH: OnceLock<PathBuf> = OnceLock::new();

/// `--persist <file>`: where the device keeps [`Persist`] across its restarts. Set once,
/// before the first dispatcher is made.
pub fn set_persist_path(p: PathBuf) -> Result<(), String> {
    PERSIST_PATH
        .set(p)
        .map_err(|_| "the persist file is set twice".to_string())
}

fn load_persist() -> Persist {
    let Some(p) = PERSIST_PATH.get() else {
        return Persist::default();
    };
    match std::fs::read_to_string(p) {
        Ok(t) => toml::from_str(&t).unwrap_or_else(|e| {
            eprintln!(
                "vfio-user-nvgpu: {}: {e}; the persistent settings start from the chip's defaults",
                p.display()
            );
            Persist::default()
        }),
        Err(_) => Persist::default(),
    }
}

/// The devinit scratch bit of a chip without resetless MIG, as the next power-on sets it:
/// `(register, mask)` when the stored MIG mode is on, `None` otherwise. On GA100 the
/// kernel learns that MIG is on at load only from this bit (kmigmgrStateInitLocked,
/// kernel_mig_manager.c:1203-1204 of 615.71.09; the other chips' HAL answers FALSE,
/// g_kernel_mig_manager_nvoc.c:440-455), which a real card's devinit sets from the InfoROM
/// setting. BAR0 latches it at power-on and at every reset (bar0.rs).
pub fn devinit_scratch(chip: &ChipData) -> Option<(u64, u32)> {
    let m = chip.mig.as_ref()?;
    let [reg, bit] = m.devinit_scratch?;
    if m.resetless || !modelled(chip, "mig") {
        return None;
    }
    // A power-on applies the pending setting (Families::power_on).
    load_persist().mig?.pending.then_some((reg as u64, 1u32 << bit))
}

/// The feature-family state of one VM.
pub struct Families {
    pub(crate) chip: &'static ChipData,
    /// The framebuffer size the guest is told (the MIG memory fallback).
    pub fb_bytes: u64,
    pub persist: Persist,
    pub mig: mig::MigState,
}

/// Whether `chip` answers `family` with a model: named present and described.
pub fn modelled(chip: &ChipData, family: &str) -> bool {
    if chip.features().absent.contains(&family) {
        return false;
    }
    match family {
        "ecc" | "row_remap" => chip.ecc.is_some(),
        "mig" | "mig_partitioning" => chip.mig.is_some(),
        _ => false,
    }
}

/// The id of `name` in the guest ABI in force, when it reaches GSP there.
pub(crate) fn id(name: &str) -> Option<u32> {
    Abi::active().routed(name).map(|r| r.id).or_else(|| {
        // Sent to GSP, but not in the generated inventory: GET_GPU_INSTANCES goes out on the
        // user's handles from GPU_GET_PARTITIONS (KMM:7994-8000; ctrl2080internal.h:2055 of
        // 610.57.04, the same id in 615.71.09), and the 615 inventory lacks the VMMU segment
        // size (ctrl2080gpu.h:3135 of 610.57.04, unchanged in 615.71.09).
        UNLISTED.iter().find(|(n, _)| *n == name).map(|(_, i)| *i)
    })
}

/// Controls the family models answer that the version's surface table does not route.
const UNLISTED: &[(&str, u32)] = &[
    ("NV2080_CTRL_CMD_INTERNAL_MIGMGR_GET_GPU_INSTANCES", 0x2080_0AA6),
    ("NV2080_CTRL_CMD_GPU_GET_VMMU_SEGMENT_SIZE", 0x2080_017E),
];

/// Offset of `path` in the version's generated layout of `st` (caraxes/tools/
/// ctrl-layouts.py, `FAMILY_STRUCTS`).
pub(crate) fn off(st: &str, path: &str) -> usize {
    Abi::active()
        .layout(st)
        .unwrap_or_else(|| panic!("{st}: no generated layout"))
        .resolve(path)
        .unwrap_or_else(|e| panic!("{e}"))
        .0
}

/// The size of `st` in the guest ABI in force.
pub(crate) fn size(st: &str) -> usize {
    Abi::active()
        .layout(st)
        .unwrap_or_else(|| panic!("{st}: no generated layout"))
        .size
}

pub(crate) fn rd32(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4)
        .map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

pub(crate) fn wr32(b: &mut [u8], o: usize, v: u32) {
    if let Some(s) = b.get_mut(o..o + 4) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

pub(crate) fn wr64(b: &mut [u8], o: usize, v: u64) {
    if let Some(s) = b.get_mut(o..o + 8) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

pub(crate) fn wr8(b: &mut [u8], o: usize, v: u8) {
    if let Some(s) = b.get_mut(o) {
        *s = v;
    }
}

pub(crate) fn reply(status: u32, params: Vec<u8>, label: &'static str, note: String) -> Answer {
    Answer {
        status,
        params,
        label,
        note,
    }
}

impl Families {
    /// The state at power-on: the persistent record (from `--persist` when given) with
    /// its pending settings applied, the chip's defaults where it has none.
    pub fn new(chip: &'static ChipData, fb_bytes: u64) -> Self {
        let mut f = Families {
            chip,
            fb_bytes,
            persist: load_persist(),
            mig: mig::MigState::default(),
        };
        f.power_on();
        f
    }

    /// Take the defaults the chip names, then apply the pending settings (a power-on or a
    /// GPU reset: both are where a real card applies them).
    fn power_on(&mut self) {
        if let Some(e) = &self.chip.ecc {
            self.persist
                .ecc
                .get_or_insert_with(|| Mode::new(e.default_enabled))
                .reset();
        }
        if let Some(m) = &self.chip.mig {
            if m.resetless {
                // Resetless MIG (GH100 and later): the mode is GSP-RM runtime state, off at
                // every GSP boot. The kernel turns it on at load only when told to
                // (gpumgrIsSystemMIGEnabled, the RmSetMIGAutoOnlineMode regkey or a boot
                // config, kernel_mig_manager.c:1009-1027 of 615.71.09); its devinit
                // scratch check is FALSE on every chip but GA100
                // (g_kernel_mig_manager_nvoc.c:440-455). A mode kept across the boot would
                // leave GSP-RM enabled and the kernel disabled: GPU_SET_PARTITIONS then
                // fails NOT_SUPPORTED (kernel_mig_manager.c:7984 of 615.71.09;
                // caraxes/runs/limits-2026-09-25/mig/gh100-6).
                self.persist.mig = Some(Mode::new(false));
            } else {
                self.persist.mig.get_or_insert_with(|| Mode::new(false)).reset();
            }
        }
        self.mig = mig::MigState::default();
    }

    /// A function-level reset of the device (`BootModel::reset`): the fresh dispatcher's
    /// families take this record over and apply what was pending. GPU and compute
    /// instances do not survive a reset (they are GSP-RM state).
    pub fn after_reset(&mut self, old: &Families) {
        self.persist = old.persist.clone();
        self.fb_bytes = old.fb_bytes;
        self.power_on();
        self.save();
    }

    /// Write the record to the `--persist` file, if one was given.
    pub fn save(&self) {
        let Some(p) = PERSIST_PATH.get() else {
            return;
        };
        match toml::to_string(&self.persist) {
            Ok(t) => {
                let text = format!(
                    "# The persistent settings of this VM's synthetic GPU (vfio-user-nvgpu --persist):\n\
                     # what a real card keeps in its InfoROM. src/families/mod.rs.\n{t}"
                );
                if let Err(e) = std::fs::write(p, text) {
                    eprintln!("vfio-user-nvgpu: {}: {e}", p.display());
                }
            }
            Err(e) => eprintln!("vfio-user-nvgpu: persist: {e}"),
        }
    }

    /// ECC in force now (the memsys static config's `bEnabledEccFBPA`).
    pub fn ecc_enabled(&self) -> bool {
        modelled(self.chip, "ecc") && self.persist.ecc.is_some_and(|m| m.current)
    }

    /// The answer of a family model to `cmd` on `(h_client, h_object)`, or `None`.
    pub fn control(&mut self, h_client: u32, h_object: u32, cmd: u32, params: &[u8]) -> Option<Answer> {
        let chip = self.chip;
        if let Some(a) = ecc::answer(self, chip, cmd, params) {
            return Some(a);
        }
        mig::answer(self, chip, (h_client, h_object), cmd, params)
    }

    /// `GSP_RM_ALLOC` of a family object (the MIG instance references 0xC637/0xC638).
    pub fn alloc(
        &mut self,
        h_client: u32,
        h_object: u32,
        h_parent: u32,
        class: u32,
        params: &[u8],
    ) -> Option<Answer> {
        mig::alloc(self, (h_client, h_object), h_parent, class, params)
    }

    /// `DUP_OBJECT`: a duplicated subscription subscribes to the same instance.
    pub fn dup(&mut self, dst: (u32, u32), src: (u32, u32)) {
        mig::dup(self, dst, src);
    }

    /// Whether the chip's static info says MIG is supported (`bIsMigSupported`,
    /// gsp_static_config.h:113; read by gpuValidateMIGSupport_KERNEL, gpu.c:7706-7716).
    pub fn mig_supported(chip: &ChipData) -> bool {
        modelled(chip, "mig")
    }

    /// `FREE`: forget a family object (freeing a client forgets all of its objects).
    pub fn free(&mut self, h_client: u32, h_object: u32) {
        self.mig.free(h_client, h_object);
    }
}

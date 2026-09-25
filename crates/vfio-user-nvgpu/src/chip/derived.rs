// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Chips of the costume ladder (docs/caraxes/costume-ladder.md): a presented chip whose
//! tables are a **base chip's tables with an overlay**. `chips/<chip>/chip.toml` names
//! the base (`base = "tu106"`) and holds only what the presented chip changes (identity,
//! short name, class database, channel classes, feature families, ...); the tables the
//! directory does not carry (regmap, device info, interrupts, controls) and the
//! firmware-boot model are the base's, and an optional `boot.toml` overlay changes the
//! boot model's data. Merging is by TOML table: a table in the overlay merges key by key
//! into the base's table, any other value (numbers, strings, arrays) replaces it.
//!
//! The overlay is how a rung says what it lies about: everything absent from it is the
//! base chip's truth.

use std::sync::OnceLock;

use super::boot_fsp::{FspBoot, FspBootData};
use super::turing::{TuringBoot, TuringBootData};
use super::{Chip, ChipData, ChipFiles, FirmwareBoot};

/// One derived chip's source texts.
pub struct DerivedFiles {
    pub name: &'static str,
    /// `chip.toml` of the derived chip: `base = "<chip>"` plus the overlay.
    pub chip: &'static str,
    /// An overlay on the base's `boot.toml`, when the boot data differ.
    pub boot: Option<&'static str>,
}

/// The chips of the ladder, by `--chip` name.
pub const DERIVED: &[DerivedFiles] = &[
    DerivedFiles {
        name: "tu104",
        chip: include_str!("../../chips/tu104/chip.toml"),
        boot: None,
    },
    DerivedFiles {
        name: "ga106",
        chip: include_str!("../../chips/ga106/chip.toml"),
        boot: Some(include_str!("../../chips/ga106/boot.toml")),
    },
    DerivedFiles {
        name: "ad106",
        chip: include_str!("../../chips/ad106/chip.toml"),
        boot: Some(include_str!("../../chips/ad106/boot.toml")),
    },
    // The GA100 rung of the limits runs (docs/caraxes/limits-mig-ecc-vgpu.md): A100
    // identity, ECC and MIG without resetless MIG.
    DerivedFiles {
        name: "ga100",
        chip: include_str!("../../chips/ga100/chip.toml"),
        boot: None,
    },
    // The vGPU-guest experiment (docs/caraxes/limits-mig-ecc-vgpu.md §4): BOOT_1 says VF.
    DerivedFiles {
        name: "tu106vf",
        chip: include_str!("../../chips/tu106vf/chip.toml"),
        boot: None,
    },
    DerivedFiles {
        name: "gh100",
        chip: include_str!("../../chips/gh100/chip.toml"),
        boot: Some(include_str!("../../chips/gh100/boot.toml")),
    },
];

enum Boot {
    Turing(TuringBootData),
    Fsp(FspBootData),
}

pub struct Derived {
    data: ChipData,
    boot: Boot,
    base: &'static str,
}

impl Derived {
    /// The base chip whose tables and boot model the overlay starts from.
    pub fn base(&self) -> &'static str {
        self.base
    }
}

impl Chip for Derived {
    fn data(&self) -> &ChipData {
        &self.data
    }

    fn firmware_boot(&'static self, fb_bytes: u64) -> Box<dyn FirmwareBoot> {
        match &self.boot {
            Boot::Turing(d) => Box::new(TuringBoot::new(d, fb_bytes)),
            Boot::Fsp(d) => Box::new(FspBoot::new(d, fb_bytes)),
        }
    }
}

/// Merge `over` into `base`: tables key by key, everything else replaced.
pub fn merge(base: &mut toml::Value, over: toml::Value) {
    match (base, over) {
        (toml::Value::Table(b), toml::Value::Table(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(bv) if bv.is_table() && v.is_table() => merge(bv, v),
                    _ => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (b, o) => *b = o,
    }
}

/// `base` with `over` merged in, as TOML text; `strip` keys are removed from the top.
pub fn overlay(base: &str, over: &str, strip: &[&str]) -> Result<(String, toml::Table), String> {
    let mut b: toml::Value = toml::from_str(base).map_err(|e| format!("base: {e}"))?;
    let o: toml::Table = toml::from_str(over).map_err(|e| format!("overlay: {e}"))?;
    let mut o2 = o.clone();
    for k in strip {
        o2.remove(*k);
    }
    merge(&mut b, toml::Value::Table(o2));
    Ok((toml::to_string(&b).map_err(|e| e.to_string())?, o))
}

fn load(f: &DerivedFiles) -> Result<Derived, String> {
    let head: toml::Table = toml::from_str(f.chip).map_err(|e| format!("chip.toml: {e}"))?;
    let base = head
        .get("base")
        .and_then(|v| v.as_str())
        .ok_or("chip.toml: a derived chip names its `base`")?;
    let (files, boot_text, turing): (&ChipFiles<'static>, &str, bool) = match base {
        "tu106" => (&super::tu106::FILES, super::tu106::BOOT, true),
        "gb206" => (&super::gb206::FILES, super::gb206::BOOT, false),
        other => return Err(format!("chip.toml: unknown base {other:?}")),
    };
    let (chip, _) = overlay(files.chip, f.chip, &["base"]).map_err(|e| format!("chip.toml {e}"))?;
    let data = ChipData::load(&ChipFiles {
        chip: &chip,
        ..*files
    })?;
    let boot_text = match f.boot {
        Some(o) => overlay(boot_text, o, &[]).map_err(|e| format!("boot.toml {e}"))?.0,
        None => boot_text.to_string(),
    };
    let boot = if turing {
        Boot::Turing(TuringBootData::parse(&boot_text)?)
    } else {
        Boot::Fsp(FspBootData::parse(&boot_text)?)
    };
    let base: &'static str = if turing { "tu106" } else { "gb206" };
    Ok(Derived { data, boot, base })
}

static LOADED: OnceLock<Vec<Result<Derived, String>>> = OnceLock::new();

fn all() -> &'static [Result<Derived, String>] {
    LOADED.get_or_init(|| {
        DERIVED
            .iter()
            .map(|f| load(f).map_err(|e| format!("chips/{}/{e}", f.name)))
            .collect()
    })
}

/// The derived chip `name`, or `None` when no such chip exists.
pub fn get(name: &str) -> Option<Result<&'static dyn Chip, String>> {
    let i = DERIVED.iter().position(|f| f.name == name)?;
    Some(match &all()[i] {
        Ok(c) => Ok(c as &'static dyn Chip),
        Err(e) => Err(e.clone()),
    })
}

/// The base chip of the derived chip `name`.
pub fn base_of(name: &str) -> Option<&'static str> {
    let i = DERIVED.iter().position(|f| f.name == name)?;
    all()[i].as_ref().ok().map(|d| d.base())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ladder_chip_loads() {
        for f in DERIVED {
            let c = get(f.name).unwrap().unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(c.data().name, f.name);
        }
    }

    #[test]
    fn merge_is_by_table() {
        let (t, o) = overlay(
            "a = 1\n[t]\nx = 1\ny = [1, 2]\n",
            "base = \"q\"\n[t]\ny = [3]\n",
            &["base"],
        )
        .unwrap();
        let v: toml::Table = toml::from_str(&t).unwrap();
        assert_eq!(v["a"].as_integer(), Some(1));
        assert_eq!(v["t"]["x"].as_integer(), Some(1));
        assert_eq!(v["t"]["y"].as_array().unwrap().len(), 1);
        assert!(v.get("base").is_none());
        assert!(o.contains_key("base"));
    }

    #[test]
    fn tu104_is_tu106_with_the_tu104_identity() {
        let a = get("tu104").unwrap().unwrap().data();
        let b = crate::chip::by_name("tu106").unwrap().data();
        assert_eq!((a.identity.boot_0, a.identity.boot_42), (0x1640_00A1, 0x164A_1000));
        assert_eq!(a.short_name, "TU104");
        assert_eq!(a.classes.as_ref().unwrap().ids, b.classes.as_ref().unwrap().ids);
        assert_eq!(a.device_info.engines, b.device_info.engines);
        assert_eq!(a.gmmu.mmu_format, b.gmmu.mmu_format);
        assert_eq!(base_of("tu104"), Some("tu106"));
    }

    /// The vGPU-guest experiment: TU106 with NV_PMC_BOOT_1_VGPU 17:16 = _VF (2) and
    /// nothing else changed.
    #[test]
    fn tu106vf_is_tu106_with_boot_1_vf() {
        let a = get("tu106vf").unwrap().unwrap().data();
        let b = crate::chip::by_name("tu106").unwrap().data();
        assert_eq!((a.identity.boot_1 >> 16) & 3, 2);
        assert_eq!((a.identity.boot_0, a.identity.boot_42), (b.identity.boot_0, b.identity.boot_42));
        assert_eq!(a.features.refused.len(), b.features.refused.len());
    }

    /// The Ampere, Ada and Hopper costumes: the architecture/implementation fields of
    /// BOOT_42 that halmgr decodes (nv_ref.h), the guest's channel classes, and the base's
    /// truth underneath (device info, GMMU format).
    #[test]
    fn later_architectures_keep_the_base_tables() {
        let tu = crate::chip::by_name("tu106").unwrap().data();
        let gb = crate::chip::by_name("gb206").unwrap().data();
        for (name, chip_id, gpfifo, compute, base) in [
            ("ga106", 0x176, 0xC56F, 0xC7C0, tu),
            ("ad106", 0x196, 0xC56F, 0xC9C0, tu),
            ("gh100", 0x180, 0xC86F, 0xCBC0, gb),
        ] {
            let c = get(name).unwrap().unwrap().data();
            assert_eq!((c.identity.boot_42 >> 20) & 0x1FF, chip_id, "{name}");
            assert_eq!((c.channel.gpfifo, c.channel.compute), (gpfifo, compute), "{name}");
            let ids = &c.classes.as_ref().unwrap().ids;
            assert!(ids.contains(&gpfifo) && ids.contains(&compute), "{name}");
            assert_eq!(c.device_info.engines, base.device_info.engines, "{name}");
            assert_eq!(c.gmmu.mmu_format, base.gmmu.mmu_format, "{name}");
        }
        let gh = get("gh100").unwrap().unwrap().data();
        assert!(gh.gr.mig);
        assert!(!gh.features.absent.contains(&"mig"));
        assert!(gh.features.absent.contains(&"frl"));
    }
}

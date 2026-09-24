// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The two table-driven rules of the GSP control surface (docs/caraxes/gsp-surface.md §3):
//!
//! * **Category E, one refusal per feature family.** The controls a stock client never
//!   sends to this chip (NVLink, vGPU, MIG, Confidential Computing, ...) are listed per
//!   family in the driver version's generated surface table (`src/ver/v<major>/surface.rs`,
//!   caraxes/tools/gsp-tables.py); the chip file names the families the chip lacks
//!   (`chips/<chip>/chip.toml` `[features]`, decision D14), and their controls are answered
//!   NOT_SUPPORTED with the D9 label [`REFUSED`]. Where the real card answers one of them
//!   NV_OK with an empty result instead, the chip file lists it under `empty_ok` and the
//!   answer is a zeroed block with the chip file's label.
//! * **Category A, pure chip data.** `chips/<chip>/controls.toml` lists controls whose
//!   answer is a fixed parameter block; fields are named, and resolved to offsets through
//!   the version's compiler-measured layout table (`src/ver/v<major>/ctrl_layouts.rs`,
//!   caraxes/tools/ctrl-layouts.py), never typed as numbers.
//!
//! Both rules run in `Dispatcher::control` (ctrl.rs) after the display, kgr, cuda, uvm and
//! nvml handlers and before the match of ctrl.rs; the tables are checked at load against
//! the generated surface, so neither rule takes a control another handler answers.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::ctrl::Answer;
use crate::ver::{active as v, Abi};

/// The D9 label of a category E refusal.
pub const REFUSED: &str = "refused (feature absent on this chip)";

// ---------------------------------------------------------------------------------
// Category E: the feature families.

/// `[features]` of chip.toml.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeaturesFile {
    /// Families the chip lacks: their controls are refused.
    pub absent: Vec<String>,
    /// Families the chip has: their controls go to the other handlers (and today end
    /// unanswered). Every family of the driver version is in exactly one of the lists.
    #[serde(default)]
    pub present: Vec<String>,
    /// Controls of absent families the real card answers NV_OK with an empty result.
    #[serde(default)]
    pub empty_ok: Vec<EmptyOkFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyOkFile {
    pub name: String,
    pub label: String,
}

/// The resolved feature families of a chip for the build's driver version.
#[derive(Clone, Debug, Default)]
pub struct Features {
    pub absent: Vec<&'static str>,
    /// Control id -> (family, control name), for the controls answered [`REFUSED`].
    pub refused: BTreeMap<u32, (&'static str, &'static str)>,
    /// Control id -> (family, control name, label), answered NV_OK with zeros.
    pub empty_ok: BTreeMap<u32, (&'static str, &'static str, String)>,
}

impl Features {
    /// Resolve `f` against the families of `abi`: every family must be named absent or
    /// present, no unknown name, and every `empty_ok` control must belong to an absent
    /// family.
    pub fn resolve(abi: &'static Abi, f: &FeaturesFile) -> Result<Features, String> {
        for n in f.absent.iter().chain(&f.present) {
            if !abi.families.iter().any(|fam| fam.name == n) {
                return Err(format!(
                    "[features]: {n:?} is no feature family of driver {} ({})",
                    abi.driver,
                    abi.families
                        .iter()
                        .map(|f| f.name)
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        let mut out = Features::default();
        for fam in abi.families {
            let a = f.absent.iter().any(|n| n == fam.name);
            let p = f.present.iter().any(|n| n == fam.name);
            if a == p {
                return Err(format!(
                    "[features]: family {:?} must be named in exactly one of absent and present",
                    fam.name
                ));
            }
            if a {
                out.absent.push(fam.name);
                for &(id, name) in fam.controls {
                    out.refused.insert(id, (fam.name, name));
                }
            }
        }
        for e in &f.empty_ok {
            let (id, (fam, name)) = out
                .refused
                .iter()
                .find(|(_, (_, n))| *n == e.name)
                .map(|(id, x)| (*id, *x))
                .ok_or_else(|| {
                    format!("[features] empty_ok: {:?} is in no absent family", e.name)
                })?;
            out.refused.remove(&id);
            out.empty_ok.insert(id, (fam, name, e.label.clone()));
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------------
// Category A: the chip-data table.

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlsFile {
    #[serde(default)]
    control: Vec<ControlFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlFile {
    name: String,
    params: String,
    label: String,
    #[serde(default)]
    inputs: Vec<String>,
    #[serde(default)]
    fields: BTreeMap<String, Value>,
    #[serde(default)]
    case: Vec<CaseFile>,
    /// "NOT_SUPPORTED" where the chip's RM refuses the control; default NV_OK.
    status: Option<String>,
    /// The guest ABIs the entry applies to; default all.
    abi: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseFile {
    when: BTreeMap<String, u64>,
    #[serde(default)]
    fields: BTreeMap<String, Value>,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Value {
    Int(u64),
    /// "ENUM:NAME" of a driver enum the version module knows.
    Name(String),
}

/// A scalar in the parameter block: (offset, size).
type Slot = (usize, usize);

#[derive(Clone, Debug)]
struct Case {
    when: Vec<(Slot, u64)>,
    fields: Vec<(Slot, u64)>,
    label: Option<String>,
}

/// One resolved table entry.
#[derive(Clone, Debug)]
pub struct ControlEntry {
    pub id: u32,
    pub name: &'static str,
    pub params: &'static str,
    pub size: usize,
    inputs: Vec<Slot>,
    fields: Vec<(Slot, u64)>,
    cases: Vec<Case>,
    status: u32,
    label: String,
}

/// A chip's category A table for one guest ABI.
#[derive(Clone, Debug, Default)]
pub struct ControlTable {
    pub entries: BTreeMap<u32, ControlEntry>,
}

fn enum_value(name: &str) -> Result<u64, String> {
    let (table, key) = name
        .split_once(':')
        .ok_or_else(|| format!("value {name:?} is neither a number nor ENUM:NAME"))?;
    let t = match table {
        "NV2080_ENGINE_TYPE" => v::NV2080_ENGINE_TYPES,
        "RM_ENGINE_TYPE" => v::RM_ENGINE_TYPES,
        "MC_ENGINE_IDX" => v::MC_ENGINE_IDX,
        "NV2080_INTR_CATEGORY" => v::INTR_CATEGORIES,
        other => {
            return Err(format!(
                "{other:?} is no driver enum the version module knows"
            ))
        }
    };
    v::lookup(t, key)
        .map(u64::from)
        .ok_or_else(|| format!("{table} has no {key:?} in driver {}", v::DRIVER_VERSION))
}

fn slot_value(
    layout: &crate::ver::CtrlLayout,
    path: &str,
    val: &Value,
) -> Result<(Slot, u64), String> {
    let slot = layout.resolve(path)?;
    let x = match val {
        Value::Int(i) => *i,
        Value::Name(n) => enum_value(n)?,
    };
    if slot.1 < 8 && x >> (8 * slot.1) != 0 {
        return Err(format!(
            "{}: {x:#x} does not fit the {} bytes of {path}",
            layout.name, slot.1
        ));
    }
    Ok((slot, x))
}

impl ControlTable {
    /// Parse and resolve a chip's controls.toml against the tables of `abi`. Every entry
    /// must name a routed category A control no other handler answers, a parameter struct
    /// the ABI lays out, and only fields that struct has.
    pub fn resolve(abi: &'static Abi, text: &str) -> Result<ControlTable, String> {
        let file: ControlsFile = toml::from_str(text).map_err(|e| format!("controls.toml: {e}"))?;
        let mut out = ControlTable::default();
        for c in file.control {
            if c.abi
                .as_ref()
                .is_some_and(|a| !a.iter().any(|x| x == abi.name))
            {
                continue;
            }
            let err = |e: String| format!("controls.toml {} (guest ABI {}): {e}", c.name, abi.name);
            let r = abi
                .routed(&c.name)
                .ok_or_else(|| err("not a control that reaches GSP".into()))?;
            if r.category != 'A' || r.other {
                return Err(err(format!(
                    "category {}{}, not an unanswered category A control",
                    r.category,
                    if r.other {
                        ", answered by another handler"
                    } else {
                        ""
                    }
                )));
            }
            let layout = abi
                .layout(&c.params)
                .ok_or_else(|| err(format!("no layout of {} (rerun ctrl-layouts.py)", c.params)))?;
            let fields = |m: &BTreeMap<String, Value>| {
                m.iter()
                    .map(|(p, x)| slot_value(layout, p, x))
                    .collect::<Result<Vec<_>, String>>()
            };
            let cases = c
                .case
                .iter()
                .map(|k| {
                    Ok(Case {
                        when: k
                            .when
                            .iter()
                            .map(|(p, x)| slot_value(layout, p, &Value::Int(*x)))
                            .collect::<Result<_, String>>()?,
                        fields: fields(&k.fields)?,
                        label: k.label.clone(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()
                .map_err(err)?;
            let status = match c.status.as_deref() {
                None | Some("OK") => v::NV_OK,
                Some("NOT_SUPPORTED") => v::NV_ERR_NOT_SUPPORTED,
                Some(s) => {
                    return Err(err(format!("status {s:?} is neither OK nor NOT_SUPPORTED")))
                }
            };
            let entry = ControlEntry {
                id: r.id,
                name: r.name,
                params: layout.name,
                size: layout.size,
                inputs: c
                    .inputs
                    .iter()
                    .map(|p| layout.resolve(p))
                    .collect::<Result<_, _>>()
                    .map_err(err)?,
                fields: fields(&c.fields).map_err(err)?,
                cases,
                status,
                label: c.label,
            };
            if out.entries.insert(r.id, entry).is_some() {
                return Err(err("listed twice".into()));
            }
        }
        Ok(out)
    }

    /// The table's answer to `cmd`: (status, parameter block of the request's size,
    /// label), or `None` when the table has no entry or no case matches the request.
    pub fn answer(&self, cmd: u32, params: &[u8]) -> Option<(u32, Vec<u8>, &str)> {
        let e = self.entries.get(&cmd)?;
        let n = params.len();
        if e.status != v::NV_OK {
            return Some((e.status, params.to_vec(), &e.label));
        }
        let read = |(off, size): Slot| -> Option<u64> {
            let b = params.get(off..off + size)?;
            let mut w = [0u8; 8];
            w[..size].copy_from_slice(b);
            Some(u64::from_le_bytes(w))
        };
        let case = if e.cases.is_empty() {
            None
        } else {
            Some(
                e.cases
                    .iter()
                    .find(|k| k.when.iter().all(|&(s, x)| read(s) == Some(x)))?,
            )
        };
        let mut b = vec![0u8; n.max(e.size)];
        for &(off, size) in &e.inputs {
            if let Some(src) = params.get(off..off + size) {
                b[off..off + size].copy_from_slice(src);
            }
        }
        for &((off, size), x) in e.fields.iter().chain(case.iter().flat_map(|k| &k.fields)) {
            b[off..off + size].copy_from_slice(&x.to_le_bytes()[..size]);
        }
        b.truncate(n);
        let label = case.and_then(|k| k.label.as_deref()).unwrap_or(&e.label);
        Some((v::NV_OK, b, label))
    }
}

// ---------------------------------------------------------------------------------
// The dispatch hook.

/// The answer of the two rules to `cmd`, or `None` when neither takes it.
pub fn answer(chip: &'static crate::chip::ChipData, cmd: u32, params: &[u8]) -> Option<Answer> {
    if let Some(&(family, name)) = chip.features.refused.get(&cmd) {
        return Some(Answer {
            status: v::NV_ERR_NOT_SUPPORTED,
            params: params.to_vec(),
            label: REFUSED,
            note: format!(
                "{name} ({cmd:#x}): feature family {family} absent on {}",
                chip.name
            ),
        });
    }
    if let Some((family, name, label)) = chip.features.empty_ok.get(&cmd) {
        return Some(Answer {
            status: v::NV_OK,
            params: vec![0u8; params.len()],
            label: label.as_str(),
            note: format!(
                "{name} ({cmd:#x}): family {family} absent, NV_OK empty as the card answers"
            ),
        });
    }
    let (status, b, label) = chip.controls.answer(cmd, params)?;
    let e = &chip.controls.entries[&cmd];
    Some(Answer {
        status,
        params: b,
        label,
        note: format!("{} ({cmd:#x}): chip table, {}", e.name, e.params),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrl::{Dispatcher, HostRm, INTERNAL_CLIENT, INTERNAL_SUBDEVICE};
    use crate::ver::ABIS;

    const TU106_CONTROLS: &str = include_str!("../chips/tu106/controls.toml");

    fn tu106() -> &'static crate::chip::ChipData {
        crate::chip::by_name("tu106").unwrap().data()
    }

    fn features_file() -> FeaturesFile {
        let t: toml::Table = toml::from_str(include_str!("../chips/tu106/chip.toml")).unwrap();
        t["features"].clone().try_into().unwrap()
    }

    /// Records every control that reaches the host.
    struct Rec(std::sync::Arc<std::sync::Mutex<Vec<u32>>>);
    impl HostRm for Rec {
        fn control(&mut self, _: u32, cmd: u32, _: &mut [u8]) -> u32 {
            self.0.lock().unwrap().push(cmd);
            v::NV_OK
        }
    }

    fn dispatcher(
        chip: &'static crate::chip::ChipData,
        host: Option<Box<dyn HostRm>>,
    ) -> Dispatcher {
        let f = crate::fixture::rtx2070();
        Dispatcher::new(chip, &f.identity(4 << 30), host)
    }

    /// The family table covers exactly the inventory's category E count of the tree it
    /// was generated from, without an id twice, for every guest ABI.
    #[test]
    fn families_cover_the_inventory_e_count() {
        for abi in ABIS {
            let ids: Vec<u32> = abi
                .families
                .iter()
                .flat_map(|f| f.controls.iter().map(|c| c.0))
                .collect();
            let set: std::collections::BTreeSet<u32> = ids.iter().copied().collect();
            assert_eq!(set.len(), ids.len(), "{}: an id in two families", abi.name);
            assert_eq!(ids.len(), abi.e_inventory, "{}", abi.name);
            for f in abi.families {
                assert!(
                    !f.headers.is_empty(),
                    "{}: {} cites no header",
                    abi.name,
                    f.name
                );
                for &(id, name) in f.controls {
                    let r = abi.routed(name).unwrap();
                    assert_eq!((r.id, r.category, r.other), (id, 'E', false), "{name}");
                }
            }
        }
        assert_eq!((ABIS[0].e_inventory, ABIS[1].e_inventory), (189, 200));
        // TU106 lacks every family, so the chip's rule covers the whole E count.
        let f = &tu106().features;
        assert_eq!(
            f.refused.len() + f.empty_ok.len(),
            Abi::active().e_inventory
        );
    }

    /// Every id of the family list resolves to the rule through the dispatcher, and none
    /// of them reaches the host.
    #[test]
    fn every_family_id_is_refused() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut d = dispatcher(tu106(), Some(Box::new(Rec(seen.clone()))));
        let f = &tu106().features;
        for fam in Abi::active().families {
            for &(id, name) in fam.controls {
                let a = d.control(INTERNAL_CLIENT, INTERNAL_SUBDEVICE, id, &[0xA5; 24]);
                if let Some((_, _, label)) = f.empty_ok.get(&id) {
                    assert_eq!(
                        (a.status, a.label, &a.params[..]),
                        (v::NV_OK, label.as_str(), &[0u8; 24][..]),
                        "{name}"
                    );
                } else {
                    assert_eq!(
                        (a.status, a.label),
                        (v::NV_ERR_NOT_SUPPORTED, REFUSED),
                        "{name}"
                    );
                    assert_eq!(a.params, [0xA5; 24], "{name}: a refusal echoes the request");
                }
            }
        }
        assert!(
            seen.lock().unwrap().is_empty(),
            "{:x?}",
            seen.lock().unwrap()
        );
    }

    /// No handled or host-forwarded control is caught by either rule: the rules take no
    /// id another handler answers, and with every family present each family id falls
    /// through every other handler unanswered (and never to the host).
    #[test]
    fn rules_take_nothing_another_handler_answers() {
        let chip = tu106();
        for r in Abi::active().routed.iter().filter(|r| r.other) {
            assert!(
                answer(chip, r.id, &[0u8; 16]).is_none(),
                "{} is answered elsewhere",
                r.name
            );
        }
        for (id, name) in crate::ver::active::FORWARD {
            assert!(
                answer(chip, *id, &[0u8; 16]).is_none(),
                "{name} is host-forwarded"
            );
        }
        let all: Vec<toml::Value> = Abi::active()
            .families
            .iter()
            .map(|f| toml::Value::String(f.name.into()))
            .collect();
        let mut t: toml::Table = toml::from_str(include_str!("../chips/tu106/chip.toml")).unwrap();
        let feats = t.get_mut("features").unwrap().as_table_mut().unwrap();
        feats.insert("absent".into(), toml::Value::Array(vec![]));
        feats.insert("present".into(), toml::Value::Array(all));
        feats.insert("empty_ok".into(), toml::Value::Array(vec![]));
        let text = toml::to_string(&t).unwrap();
        let files = crate::chip::ChipFiles {
            chip: &text,
            ..crate::chip::tu106::FILES
        };
        let present: &'static crate::chip::ChipData =
            Box::leak(Box::new(crate::chip::ChipData::load(&files).unwrap()));
        assert!(present.features.refused.is_empty() && present.features.empty_ok.is_empty());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut d = dispatcher(present, Some(Box::new(Rec(seen.clone()))));
        for fam in Abi::active().families {
            for &(id, name) in fam.controls {
                let a = d.control(INTERNAL_CLIENT, INTERNAL_SUBDEVICE, id, &[0u8; 24]);
                assert_eq!(
                    a.label, "unanswered",
                    "{name} ({id:#x}) has another handler"
                );
            }
        }
        assert!(
            seen.lock().unwrap().is_empty(),
            "{:x?}",
            seen.lock().unwrap()
        );
        // The A table answers its controls ahead of the rest (a zeroed request matches
        // every entry without cases).
        for e in chip
            .controls
            .entries
            .values()
            .filter(|e| e.cases.is_empty())
        {
            let a = d.control(
                INTERNAL_CLIENT,
                INTERNAL_SUBDEVICE,
                e.id,
                &vec![0u8; e.size],
            );
            assert_ne!(a.label, "unanswered", "{}", e.name);
        }
    }

    #[test]
    fn features_are_checked() {
        let abi = Abi::active();
        let mut f = features_file();
        f.absent.push("warp_drive".into());
        assert!(Features::resolve(abi, &f)
            .unwrap_err()
            .contains("warp_drive"));
        let mut f = features_file();
        f.absent.retain(|n| n != "nvlink");
        assert!(Features::resolve(abi, &f).unwrap_err().contains("nvlink"));
        let mut f = features_file();
        f.present.push("mig".into());
        assert!(Features::resolve(abi, &f).is_err());
        let mut f = features_file();
        f.empty_ok.push(EmptyOkFile {
            name: "NV2080_CTRL_CMD_GR_CTXSW_ZCULL_BIND".into(),
            label: "card".into(),
        });
        assert!(Features::resolve(abi, &f).is_err());
        // A chip with NVLink (TU102) turns the family on: its controls are not refused.
        let mut f = features_file();
        f.absent.retain(|n| n != "nvlink");
        f.present.push("nvlink".into());
        let r = Features::resolve(abi, &f).unwrap();
        assert!(!r.refused.values().any(|(fam, _)| *fam == "nvlink"));
        assert!(r.refused.values().any(|(fam, _)| *fam == "mig"));
    }

    /// The real card answers NVENC_SW_SESSION_STATS NV_OK with zeros (native nvml trace);
    /// NVLINK_GET_PLATFORM_INFO, which it also answers NV_OK, stays with nvml.rs.
    #[test]
    fn the_cards_empty_answers_are_kept() {
        let f = &tu106().features;
        assert!(f.empty_ok.values().any(|(_, n, l)| *n
            == "NV2080_CTRL_GPU_GET_NVENC_SW_SESSION_STATS"
            && l.starts_with("card")));
        let mut d = dispatcher(tu106(), None);
        let a = d.control(
            INTERNAL_CLIENT,
            INTERNAL_SUBDEVICE,
            v::CMD_NVLINK_GET_PLATFORM_INFO,
            &[0u8; 64],
        );
        assert_eq!(a.status, v::NV_OK);
        assert_ne!(a.label, REFUSED);
    }

    /// Every entry of the table names only fields that exist in each ABI it targets.
    #[test]
    fn table_fields_exist_in_every_abi() {
        for abi in ABIS {
            let t = ControlTable::resolve(abi, TU106_CONTROLS).unwrap();
            assert_eq!(t.entries.len(), 8, "{}", abi.name);
        }
        let abi = Abi::active();
        let bad = TU106_CONTROLS.replace("firstAsyncCEIdx", "firstAsyncCeIndex");
        assert!(ControlTable::resolve(abi, &bad)
            .unwrap_err()
            .contains("firstAsyncCeIndex"));
        let bad = TU106_CONTROLS.replace("\"capsTbl[4]\"", "\"capsTbl[8]\"");
        assert!(ControlTable::resolve(abi, &bad)
            .unwrap_err()
            .contains("out of"));
        let bad = TU106_CONTROLS.replace("NV2080_ENGINE_TYPE:COPY4", "NV2080_ENGINE_TYPE:COPY99");
        assert!(ControlTable::resolve(abi, &bad).is_err());
        // Handled elsewhere (GR_CTXSW_ZCULL_BIND is a match arm of ctrl.rs).
        let bad = TU106_CONTROLS.replace(
            "name = \"NV2080_CTRL_CMD_GPU_GET_FIRST_ASYNC_CE_IDX\"",
            "name = \"NV2080_CTRL_CMD_GR_CTXSW_ZCULL_BIND\"",
        );
        assert!(ControlTable::resolve(abi, &bad).is_err());
        let limited = TU106_CONTROLS.replace(
            "name = \"NV2080_CTRL_CMD_GPU_GET_FIRST_ASYNC_CE_IDX\"",
            "abi = [\"999\"]\nname = \"NV2080_CTRL_CMD_GPU_GET_FIRST_ASYNC_CE_IDX\"",
        );
        assert_eq!(
            ControlTable::resolve(abi, &limited).unwrap().entries.len(),
            7
        );
    }

    fn put32(b: &mut [u8], off: usize, x: u32) {
        b[off..off + 4].copy_from_slice(&x.to_le_bytes());
    }

    /// The handler against hand-built expected blocks for three controls, on both ABIs:
    /// NVDEC caps (a byte table and an input kept), the engine notification vectors (an
    /// array of structs with enum values) and the ZBC table size (cases on an input).
    #[test]
    fn three_controls_on_both_abis() {
        use core::mem::{offset_of, size_of};
        type Nvdec610 = nvrm_sys::v610::NV0080_CTRL_NVDEC_GET_CAPS_PARAMS_V2;
        type Nvdec615 = nvrm_sys::v615::NV0080_CTRL_NVDEC_GET_CAPS_PARAMS_V2;
        type Vectors = nvrm_sys::NV2080_CTRL_MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS_PARAMS;
        type Entry = nvrm_sys::NV2080_CTRL_MC_ENGINE_NOTIFICATION_INTR_VECTOR_ENTRY;
        for abi in ABIS {
            let t = ControlTable::resolve(abi, TU106_CONTROLS).unwrap();
            let id = |n: &str| abi.routed(n).unwrap().id;

            // NVDEC caps: capsTbl[0] = 0x01, capsTbl[4] = 0x16, instanceId echoed.
            let (size, caps, inst) = match abi.name {
                "610" => (
                    size_of::<Nvdec610>(),
                    offset_of!(Nvdec610, capsTbl),
                    offset_of!(Nvdec610, instanceId),
                ),
                _ => (
                    size_of::<Nvdec615>(),
                    offset_of!(Nvdec615, capsTbl),
                    offset_of!(Nvdec615, instanceId),
                ),
            };
            let mut req = vec![0xEEu8; size];
            put32(&mut req, inst, 1);
            let mut want = vec![0u8; size];
            want[caps] = 0x01;
            want[caps + 4] = 0x16;
            put32(&mut want, inst, 1);
            let (st, got, label) = t
                .answer(id("NV0080_CTRL_CMD_NVDEC_GET_CAPS_V2"), &req)
                .unwrap();
            assert_eq!(
                (st, &got, label.starts_with("card")),
                (v::NV_OK, &want, true),
                "{}",
                abi.name
            );

            // Engine notification vectors: GR0 4, COPY2-4 5-7.
            let size = size_of::<Vectors>();
            let e0 = offset_of!(Vectors, entries);
            let es = size_of::<Entry>();
            let mut want = vec![0u8; size];
            put32(&mut want, offset_of!(Vectors, numEntries), 4);
            for (i, (eng, vec)) in [(1u32, 4u32), (0x0B, 5), (0x0C, 6), (0x0D, 7)]
                .into_iter()
                .enumerate()
            {
                put32(
                    &mut want,
                    e0 + i * es + offset_of!(Entry, nv2080EngineType),
                    eng,
                );
                put32(
                    &mut want,
                    e0 + i * es + offset_of!(Entry, notificationIntrVector),
                    vec,
                );
            }
            let (st, got, _) = t
                .answer(
                    id("NV2080_CTRL_CMD_MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS"),
                    &vec![0u8; size],
                )
                .unwrap();
            assert_eq!((st, &got), (v::NV_OK, &want), "{}", abi.name);

            // ZBC table size {indexStart, indexEnd, tableType} (ctrl9096.h): 1..15 for
            // colour, depth and stencil, as the card answered; no answer for type 0.
            let zbc = id("NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_SIZE");
            for ty in 1u8..=3 {
                let req = [0, 0, 0, 0, 0, 0, 0, 0, ty, 0, 0, 0];
                let want = [1, 0, 0, 0, 15, 0, 0, 0, ty, 0, 0, 0];
                let (st, got, _) = t.answer(zbc, &req).unwrap();
                assert_eq!(
                    (st, &got[..]),
                    (v::NV_OK, &want[..]),
                    "{} type {ty}",
                    abi.name
                );
            }
            assert!(t.answer(zbc, &[0u8; 12]).is_none());
        }
    }

    /// The MC entries repeat chips/tu106/interrupts.toml; they must agree with it.
    #[test]
    fn mc_entries_agree_with_the_interrupt_table() {
        let chip = tu106();
        let id = |n: &str| Abi::active().routed(n).unwrap().id;
        let (_, b, _) = chip
            .controls
            .answer(
                id("NV2080_CTRL_CMD_MC_GET_INTR_CATEGORY_SUBTREE_MAP"),
                &[0u8; 56],
            )
            .unwrap();
        for c in 0..7usize {
            let got = u64::from_le_bytes(b[8 * c..8 * c + 8].try_into().unwrap());
            let want = chip
                .interrupts
                .subtrees
                .iter()
                .find(|s| s.0 == c)
                .map_or(0, |s| s.1);
            assert_eq!(got, want, "category {c}");
        }
        let (_, b, _) = chip
            .controls
            .answer(
                id("NV2080_CTRL_CMD_MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS"),
                &[0u8; 2052],
            )
            .unwrap();
        let w = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let mc = |n: &str| v::lookup(v::MC_ENGINE_IDX, n).unwrap() as u16;
        for (i, name) in ["GR0", "CE2", "CE3", "CE4"].into_iter().enumerate() {
            let e = chip
                .interrupts
                .entries
                .iter()
                .find(|e| e.mc_idx == mc(name))
                .unwrap();
            assert_eq!(w(4 + 8 * i + 4), e.vector_nonstall, "{name}");
        }
        let (_, b, _) = chip
            .controls
            .answer(id("NV2080_CTRL_CMD_MC_GET_STATIC_INTR_TABLE"), &[0u8; 516])
            .unwrap();
        let w = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        for (i, name) in [
            "NON_REPLAYABLE_FAULT",
            "REPLAYABLE_FAULT",
            "ACCESS_CNTR",
            "TMR",
        ]
        .into_iter()
        .enumerate()
        {
            let e = chip
                .interrupts
                .entries
                .iter()
                .find(|e| e.mc_idx == mc(name))
                .unwrap();
            assert_eq!(
                (w(4 + 16 * i + 8), w(4 + 16 * i + 12)),
                (e.vector_stall, e.vector_nonstall),
                "{name}"
            );
        }
    }
}

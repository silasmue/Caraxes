// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The guest-local ZBC clear table (docs/caraxes/gsp-plan/d-device-model.md 6.2, roadmap R3,
//! pulled forward: the display gate sends `SET_ZBC_COLOR_CLEAR`,
//! caraxes/runs/gsp-d-2026-09-24/after/gate-dscan.txt).
//!
//! On the real GPU the ZBC table is one per chip, shared by every client: GSP-RM seeds it at
//! boot and every `SET_ZBC_*` of a client adds an entry (ctrl9096.h:84-123), until
//! `NV_ERR_INSUFFICIENT_RESOURCES`. The device keeps one table per VM instead, seeded from
//! the chip's defaults (`chips/<chip>/zbc.toml`, read once from the host, Measurement Q5) and
//! written only by this guest. Nothing here reaches the host: forwarding a setter would take
//! slots of the table the host desktop's clients use and let the guest steer the clear
//! values of host surfaces, and a ZBC clear needs a compressible kind, which D16 never lets
//! reach the host anyway. The getters (`GET_ZBC_CLEAR_TABLE_ENTRY`, the deprecated
//! `GET_ZBC_CLEAR_TABLE`) answer from the same table, so a guest reads back what it set;
//! `GET_ZBC_CLEAR_TABLE_SIZE` stays the chip-data row of chipctl.rs (the same ranges).
//!
//! Layouts come from the version's generated table (`ctrl_layouts.rs`), the table types
//! from the generated D enums (`NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_*`, the values of
//! `NV9096_CTRL_ZBC_CLEAR_TABLE_TYPE`, ctrl9096.h:371-377).

use serde::Deserialize;

use crate::ctrl::Answer;
use crate::ver::Abi;

/// D9 label of a setter.
pub const RECORDED: &str = "guest (ZBC table of this VM; never written to the host's table)";
/// D9 label of a getter.
pub const READ: &str = "guest (ZBC table of this VM: the chip defaults read once from the host, then this guest's entries)";

/// One table entry: the value words (colour: colorFB[4] then colorDS[4]; depth and stencil:
/// one word) and the format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub value: Vec<u32>,
    pub format: u32,
}

/// One of the three tables: slots `start..=end`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub start: u32,
    pub end: u32,
    slots: Vec<Option<Entry>>,
}

impl Table {
    fn new(start: u32, end: u32) -> Self {
        Table {
            start,
            end,
            slots: vec![None; (end.saturating_sub(start) + 1) as usize],
        }
    }

    pub fn get(&self, index: u32) -> Option<&Entry> {
        if index < self.start || index > self.end {
            return None;
        }
        self.slots[(index - self.start) as usize].as_ref()
    }

    fn set(&mut self, index: u32, e: Option<Entry>) -> bool {
        if index < self.start || index > self.end {
            return false;
        }
        self.slots[(index - self.start) as usize] = e;
        true
    }

    /// Add `e`: the index of an equal entry (the real table references it again), else the
    /// first free slot, else `None` (table full).
    fn add(&mut self, e: Entry) -> Option<u32> {
        if let Some(i) = self.slots.iter().position(|s| s.as_ref() == Some(&e)) {
            return Some(self.start + i as u32);
        }
        let i = self.slots.iter().position(|s| s.is_none())?;
        self.slots[i] = Some(e);
        Some(self.start + i as u32)
    }

    pub fn used(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }
}

/// The three tables of one VM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZbcTable {
    pub color: Table,
    pub depth: Table,
    pub stencil: Table,
    /// Where the seed came from (the chip file's label, or why there is none).
    pub seed: String,
}

#[derive(Deserialize)]
struct SeedFile {
    label: String,
    color: SeedTable<ColorEntry>,
    depth: SeedTable<ValueEntry>,
    stencil: SeedTable<ValueEntry>,
}

#[derive(Deserialize)]
struct SeedTable<E> {
    index_start: u32,
    index_end: u32,
    #[serde(default = "Vec::new")]
    entry: Vec<E>,
}

#[derive(Deserialize)]
struct ColorEntry {
    index: u32,
    format: u32,
    color_fb: [u32; 4],
    color_ds: [u32; 4],
}

#[derive(Deserialize)]
struct ValueEntry {
    index: u32,
    format: u32,
    value: u32,
}

/// The chip file's seed text (`chips/<chip>/zbc.toml`); `None` for a chip without a sweep.
fn seed_text(chip: &str) -> Option<&'static str> {
    match chip {
        "tu106" => Some(include_str!("../chips/tu106/zbc.toml")),
        _ => None,
    }
}

impl ZbcTable {
    /// The table of a new VM on `chip`: the chip's defaults, or three empty 1..15 tables
    /// (the ranges of `GET_ZBC_CLEAR_TABLE_SIZE`) where no sweep of the chip exists.
    pub fn for_chip(chip: &str) -> Self {
        match seed_text(chip) {
            Some(t) => Self::from_seed(t).unwrap_or_else(|e| panic!("chips/{chip}/zbc.toml: {e}")),
            None => ZbcTable {
                color: Table::new(1, 15),
                depth: Table::new(1, 15),
                stencil: Table::new(1, 15),
                seed: format!("open: no ZBC sweep of {chip} (Measurement Q5 on that chip); the table starts empty"),
            },
        }
    }

    fn from_seed(text: &str) -> Result<Self, String> {
        let f: SeedFile = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut t = ZbcTable {
            color: Table::new(f.color.index_start, f.color.index_end),
            depth: Table::new(f.depth.index_start, f.depth.index_end),
            stencil: Table::new(f.stencil.index_start, f.stencil.index_end),
            seed: f.label,
        };
        for e in f.color.entry {
            let value = e
                .color_fb
                .iter()
                .chain(e.color_ds.iter())
                .copied()
                .collect();
            if !t.color.set(
                e.index,
                Some(Entry {
                    value,
                    format: e.format,
                }),
            ) {
                return Err(format!("colour index {} outside the range", e.index));
            }
        }
        for (tab, entries, what) in [
            (&mut t.depth, f.depth.entry, "depth"),
            (&mut t.stencil, f.stencil.entry, "stencil"),
        ] {
            for e in entries {
                if !tab.set(
                    e.index,
                    Some(Entry {
                        value: vec![e.value],
                        format: e.format,
                    }),
                ) {
                    return Err(format!("{what} index {} outside the range", e.index));
                }
            }
        }
        Ok(t)
    }

    fn table(&self, ty: u32) -> Option<&Table> {
        let e = |n: &str| Abi::active().d_enum(n);
        if Some(ty) == e("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_COLOR") {
            Some(&self.color)
        } else if Some(ty) == e("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_DEPTH") {
            Some(&self.depth)
        } else if Some(ty) == e("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_STENCIL") {
            Some(&self.stencil)
        } else {
            None
        }
    }
}

/// Field offset of `path` in the version's layout of `st`.
fn off(st: &str, path: &str) -> usize {
    Abi::active()
        .layout(st)
        .unwrap_or_else(|| panic!("{st}: no generated layout"))
        .resolve(path)
        .unwrap_or_else(|e| panic!("{e}"))
        .0
}

fn rd(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4)
        .map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

fn wr(b: &mut [u8], o: usize, v: u32) {
    if let Some(s) = b.get_mut(o..o + 4) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

fn status(name: &str) -> u32 {
    crate::dmodel::status(name)
}

fn answer(status: u32, params: Vec<u8>, label: &'static str, note: String) -> Answer {
    Answer {
        status,
        params,
        label,
        note,
    }
}

const COLOR: &str = "NV9096_CTRL_SET_ZBC_COLOR_CLEAR_PARAMS";
const DEPTH: &str = "NV9096_CTRL_SET_ZBC_DEPTH_CLEAR_PARAMS";
const STENCIL: &str = "NV9096_CTRL_SET_ZBC_STENCIL_CLEAR_PARAMS";
const TABLE: &str = "NV9096_CTRL_SET_ZBC_CLEAR_TABLE_PARAMS";
const GET_ENTRY: &str = "NV9096_CTRL_GET_ZBC_CLEAR_TABLE_ENTRY_PARAMS";
const GET_TABLE: &str = "NV9096_CTRL_GET_ZBC_CLEAR_TABLE_PARAMS";

/// The ZBC controls by name: the four setters (D rows) and the two getters.
pub fn control(t: &mut ZbcTable, name: &str, params: &[u8]) -> Option<Answer> {
    let mut b = params.to_vec();
    let full = status("NV_ERR_INSUFFICIENT_RESOURCES");
    let bad = status("NV_ERR_INVALID_ARGUMENT");
    let words = |st: &str, field: &str, n: usize| -> Vec<u32> {
        (0..n)
            .map(|i| rd(params, off(st, &format!("{field}[{i}]"))))
            .collect()
    };
    let added = |r: Option<u32>, what: &str, b: Vec<u8>| match r {
        Some(i) => answer(
            0,
            b,
            RECORDED,
            format!("ZBC {what} entry at index {i} of this VM's table"),
        ),
        None => answer(
            full,
            b,
            RECORDED,
            format!("ZBC {what} table of this VM is full"),
        ),
    };
    let a = match name {
        "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR" => {
            let mut value = words(COLOR, "colorFB", 4);
            value.extend(words(COLOR, "colorDS", 4));
            let format = rd(params, off(COLOR, "format"));
            added(t.color.add(Entry { value, format }), "colour", b)
        }
        "NV9096_CTRL_CMD_SET_ZBC_DEPTH_CLEAR" => {
            let e = Entry {
                value: vec![rd(params, off(DEPTH, "depth"))],
                format: rd(params, off(DEPTH, "format")),
            };
            added(t.depth.add(e), "depth", b)
        }
        "NV9096_CTRL_CMD_SET_ZBC_STENCIL_CLEAR" => {
            let e = Entry {
                value: vec![rd(params, off(STENCIL, "stencil"))],
                format: rd(params, off(STENCIL, "format")),
            };
            added(t.stencil.add(e), "stencil", b)
        }
        // The deprecated indexed form (ctrl9096.h:249-309): all three tables at `index`; a
        // table whose format is INVALID (0) is left as it is.
        "NV9096_CTRL_CMD_SET_ZBC_CLEAR_TABLE" => {
            let index = rd(params, off(TABLE, "index"));
            if index < t.color.start || index > t.color.end {
                return Some(answer(
                    bad,
                    b,
                    RECORDED,
                    format!(
                        "ZBC clear table index {index} outside {}..={}",
                        t.color.start, t.color.end
                    ),
                ));
            }
            let mut value = words(TABLE, "colorFB", 4);
            value.extend(words(TABLE, "colorDS", 4));
            let (cf, df, sf) = (
                rd(params, off(TABLE, "colorFormat")),
                rd(params, off(TABLE, "depthFormat")),
                rd(params, off(TABLE, "stencilFormat")),
            );
            let mut set = Vec::new();
            if cf != 0 && t.color.set(index, Some(Entry { value, format: cf })) {
                set.push("colour");
            }
            if df != 0
                && t.depth.set(
                    index,
                    Some(Entry {
                        value: vec![rd(params, off(TABLE, "depth"))],
                        format: df,
                    }),
                )
            {
                set.push("depth");
            }
            if sf != 0
                && t.stencil.set(
                    index,
                    Some(Entry {
                        value: vec![rd(params, off(TABLE, "stencil"))],
                        format: sf,
                    }),
                )
            {
                set.push("stencil");
            }
            answer(
                0,
                b,
                RECORDED,
                format!(
                    "ZBC clear table index {index}: {} set in this VM's table",
                    set.join(", ")
                ),
            )
        }
        "NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY" => {
            let (ty, index) = (
                rd(params, off(GET_ENTRY, "tableType")),
                rd(params, off(GET_ENTRY, "index")),
            );
            let Some(tab) = t.table(ty) else {
                return Some(answer(
                    bad,
                    b,
                    READ,
                    format!("ZBC entry: table type {ty} unknown"),
                ));
            };
            if index < tab.start || index > tab.end {
                return Some(answer(
                    bad,
                    b,
                    READ,
                    format!(
                        "ZBC entry: index {index} outside {}..={}",
                        tab.start, tab.end
                    ),
                ));
            }
            let e = tab.get(index).cloned();
            fill_value(&mut b, GET_ENTRY, ty, e.as_ref());
            wr(
                &mut b,
                off(GET_ENTRY, "format"),
                e.as_ref().map_or(0, |e| e.format),
            );
            let valid = off(GET_ENTRY, "bIndexValid");
            if let Some(v) = b.get_mut(valid) {
                *v = e.is_some() as u8;
            }
            answer(
                0,
                b,
                READ,
                format!(
                    "ZBC entry type {ty} index {index}: {}",
                    if e.is_some() { "valid" } else { "empty" }
                ),
            )
        }
        // The deprecated combined getter (ctrl9096.h:173-247): valType INVALID returns the
        // colour range; otherwise indexSize names the index to read.
        "NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE" => {
            let ty = rd(params, off(GET_TABLE, "valType"));
            if ty == 0 {
                wr(&mut b, off(GET_TABLE, "indexSize"), t.color.end);
                wr(&mut b, off(GET_TABLE, "indexStart"), t.color.start);
                wr(&mut b, off(GET_TABLE, "indexEnd"), t.color.end);
                return Some(answer(0, b, READ, "ZBC clear table: index range".into()));
            }
            let index = rd(params, off(GET_TABLE, "indexSize"));
            let Some(tab) = t.table(ty) else {
                return Some(answer(
                    bad,
                    b,
                    READ,
                    format!("ZBC clear table: value type {ty} unknown"),
                ));
            };
            if index < tab.start || index > tab.end {
                return Some(answer(
                    bad,
                    b,
                    READ,
                    format!(
                        "ZBC clear table: index {index} outside {}..={}",
                        tab.start, tab.end
                    ),
                ));
            }
            let e = tab.get(index).cloned();
            fill_value(&mut b, GET_TABLE, ty, e.as_ref());
            wr(
                &mut b,
                off(GET_TABLE, "format"),
                e.as_ref().map_or(0, |e| e.format),
            );
            wr(&mut b, off(GET_TABLE, "indexUsed"), e.is_some() as u32);
            answer(
                0,
                b,
                READ,
                format!("ZBC clear table type {ty} index {index}"),
            )
        }
        _ => return None,
    };
    Some(a)
}

/// Write `value.colorFB`, `colorDS`, `depth`, `stencil` of a getter from `e` (zero when empty).
fn fill_value(b: &mut [u8], st: &str, ty: u32, e: Option<&Entry>) {
    let color = Abi::active().d_enum("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_COLOR");
    let depth = Abi::active().d_enum("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_DEPTH");
    for i in 0..4 {
        wr(b, off(st, &format!("value.colorFB[{i}]")), 0);
        wr(b, off(st, &format!("value.colorDS[{i}]")), 0);
    }
    wr(b, off(st, "value.depth"), 0);
    wr(b, off(st, "value.stencil"), 0);
    let Some(e) = e else { return };
    if Some(ty) == color {
        for i in 0..4 {
            wr(b, off(st, &format!("value.colorFB[{i}]")), e.value[i]);
            wr(b, off(st, &format!("value.colorDS[{i}]")), e.value[4 + i]);
        }
    } else if Some(ty) == depth {
        wr(b, off(st, "value.depth"), e.value[0]);
    } else {
        wr(b, off(st, "value.stencil"), e.value[0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(t: &mut ZbcTable, ty: u32, index: u32) -> (u32, Vec<u8>) {
        let mut p = vec![0u8; 56];
        wr(&mut p, off(GET_ENTRY, "tableType"), ty);
        wr(&mut p, off(GET_ENTRY, "index"), index);
        let a = control(t, "NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY", &p).unwrap();
        (a.status, a.params)
    }

    /// The seed is the chip file's defaults (Q5): colour 1-3, depth 1-2, stencil 1-3, the
    /// ranges 1..15, nothing of the host desktop's entries.
    #[test]
    fn seeded_with_the_chip_defaults() {
        let mut t = ZbcTable::for_chip("tu106");
        assert_eq!(
            (t.color.used(), t.depth.used(), t.stencil.used()),
            (3, 2, 3)
        );
        assert_eq!((t.color.start, t.color.end), (1, 15));
        let (st, b) = entry(&mut t, 1, 2);
        assert_eq!(st, 0);
        assert_eq!(rd(&b, off(GET_ENTRY, "value.colorFB[0]")), 0xFFFF_FFFF);
        assert_eq!(rd(&b, off(GET_ENTRY, "value.colorDS[3]")), 0x3F80_0000);
        assert_eq!(rd(&b, off(GET_ENTRY, "format")), 2);
        assert_eq!(b[off(GET_ENTRY, "bIndexValid")], 1);
        let (_, b) = entry(&mut t, 1, 4);
        assert_eq!(b[off(GET_ENTRY, "bIndexValid")], 0, "no host desktop entry");
        let (_, b) = entry(&mut t, 3, 3);
        assert_eq!(rd(&b, off(GET_ENTRY, "value.stencil")), 0xFF);
        assert_eq!(entry(&mut t, 4, 1).0, status("NV_ERR_INVALID_ARGUMENT"));
        assert_eq!(entry(&mut t, 2, 16).0, status("NV_ERR_INVALID_ARGUMENT"));
        assert!(ZbcTable::for_chip("gb206").seed.starts_with("open"));
    }

    /// A guest's colour goes into the first free slot, an equal one reuses its slot, the
    /// getter hands it back, and the sixteenth distinct entry finds the table full.
    #[test]
    fn set_add_readback_full() {
        let mut t = ZbcTable::for_chip("tu106");
        let mut p = vec![0u8; 40];
        let color = |p: &mut Vec<u8>, w: u32| {
            for i in 0..4 {
                wr(p, off(COLOR, &format!("colorFB[{i}]")), w + i as u32);
                wr(p, off(COLOR, &format!("colorDS[{i}]")), w);
            }
            wr(p, off(COLOR, "format"), 0x28);
        };
        color(&mut p, 0x1234_0000);
        let a = control(&mut t, "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR", &p).unwrap();
        assert_eq!((a.status, a.label, &a.params), (0, RECORDED, &p));
        assert!(a.note.contains("index 4"), "{}", a.note);
        let a = control(&mut t, "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR", &p).unwrap();
        assert!(
            a.note.contains("index 4"),
            "an equal entry is referenced again: {}",
            a.note
        );
        let (_, b) = entry(&mut t, 1, 4);
        assert_eq!(rd(&b, off(GET_ENTRY, "value.colorFB[3]")), 0x1234_0003);
        assert_eq!(rd(&b, off(GET_ENTRY, "format")), 0x28);
        for k in 1..=11u32 {
            color(&mut p, 0x2000_0000 + (k << 8));
            assert_eq!(
                control(&mut t, "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR", &p)
                    .unwrap()
                    .status,
                0
            );
        }
        assert_eq!(t.color.used(), 15);
        color(&mut p, 0x3000_0000);
        let a = control(&mut t, "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR", &p).unwrap();
        assert_eq!(a.status, status("NV_ERR_INSUFFICIENT_RESOURCES"));
        // Depth and stencil through their own setters.
        let mut d = vec![0u8; 12];
        wr(&mut d, off(DEPTH, "depth"), 0x3E80_0000);
        wr(&mut d, off(DEPTH, "format"), 1);
        assert_eq!(
            control(&mut t, "NV9096_CTRL_CMD_SET_ZBC_DEPTH_CLEAR", &d)
                .unwrap()
                .status,
            0
        );
        let (_, b) = entry(&mut t, 2, 3);
        assert_eq!(rd(&b, off(GET_ENTRY, "value.depth")), 0x3E80_0000);
        let mut s = vec![0u8; 12];
        wr(&mut s, off(STENCIL, "stencil"), 0x5A);
        wr(&mut s, off(STENCIL, "format"), 1);
        assert_eq!(
            control(&mut t, "NV9096_CTRL_CMD_SET_ZBC_STENCIL_CLEAR", &s)
                .unwrap()
                .status,
            0
        );
        let (_, b) = entry(&mut t, 3, 4);
        assert_eq!(rd(&b, off(GET_ENTRY, "value.stencil")), 0x5A);
    }

    /// Each VM has its own table: a second VM does not see the first one's entries.
    #[test]
    fn tables_are_per_vm() {
        let mut a = ZbcTable::for_chip("tu106");
        let b = ZbcTable::for_chip("tu106");
        let mut p = vec![0u8; 12];
        wr(&mut p, off(STENCIL, "stencil"), 7);
        wr(&mut p, off(STENCIL, "format"), 1);
        control(&mut a, "NV9096_CTRL_CMD_SET_ZBC_STENCIL_CLEAR", &p).unwrap();
        assert_eq!((a.stencil.used(), b.stencil.used()), (4, 3));
    }

    /// The indexed setter and the combined getter (both deprecated) over the same table.
    #[test]
    fn indexed_setter_and_combined_getter() {
        let mut t = ZbcTable::for_chip("tu106");
        let mut p = vec![0u8; 60];
        wr(&mut p, off(TABLE, "index"), 9);
        wr(&mut p, off(TABLE, "depth"), 0x3F00_0000);
        wr(&mut p, off(TABLE, "depthFormat"), 1);
        assert_eq!(
            control(&mut t, "NV9096_CTRL_CMD_SET_ZBC_CLEAR_TABLE", &p)
                .unwrap()
                .status,
            0
        );
        assert_eq!(
            t.color.used(),
            3,
            "colour format INVALID leaves the colour table"
        );
        let mut g = vec![0u8; 64];
        wr(&mut g, off(GET_TABLE, "valType"), 2);
        wr(&mut g, off(GET_TABLE, "indexSize"), 9);
        let a = control(&mut t, "NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE", &g).unwrap();
        assert_eq!(rd(&a.params, off(GET_TABLE, "value.depth")), 0x3F00_0000);
        assert_eq!(rd(&a.params, off(GET_TABLE, "indexUsed")), 1);
        wr(&mut p, off(TABLE, "index"), 16);
        assert_eq!(
            control(&mut t, "NV9096_CTRL_CMD_SET_ZBC_CLEAR_TABLE", &p)
                .unwrap()
                .status,
            status("NV_ERR_INVALID_ARGUMENT")
        );
    }
}

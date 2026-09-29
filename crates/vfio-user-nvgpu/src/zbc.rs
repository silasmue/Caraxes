// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The guest-local ZBC clear table (docs/caraxes/gsp-plan/d-device-model.md 6.2, roadmap R3,
//! pulled forward: the display gate sends `SET_ZBC_COLOR_CLEAR`,
//! caraxes/runs/gsp-d-2026-09-24/after/gate-dscan.txt).
//!
//! On the real GPU the ZBC table is one per chip, shared by every client: GSP-RM seeds it at
//! boot and every `SET_ZBC_*` of a client adds an entry (ctrl9096.h:84-123), until
//! `NV_ERR_INSUFFICIENT_RESOURCES`. The device keeps one table per VM instead, seeded from
//! the chip's defaults (`chips/<chip>/zbc.toml`, read once from the host, Measurement Q5) and
//! written only by this guest. Without real compression nothing here reaches the host:
//! forwarding a setter would take slots of the table the host desktop's clients use, and a
//! ZBC clear needs a compressible kind, which D16 never lets reach the host anyway. The
//! getters (`GET_ZBC_CLEAR_TABLE_ENTRY`, the deprecated `GET_ZBC_CLEAR_TABLE`) answer from
//! the same table, so a guest reads back what it set;
//! `GET_ZBC_CLEAR_TABLE_SIZE` stays the chip-data row of chipctl.rs (the same ranges).
//!
//! Layouts come from the version's generated table (`ctrl_layouts.rs`), the table types
//! from the generated D enums (`NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_*`, the values of
//! `NV9096_CTRL_ZBC_CLEAR_TABLE_TYPE`, ctrl9096.h:371-377).
//!
//! With real compression per VM slice (`CARAXES_COMPRESSION=slice`, [`SliceZbc`]) the guest's
//! fast clears run on the host GPU with compressible kinds, which on Turing are exactly the
//! ZBC kinds (mem_mgr_tu102_base.c:52-53), so the hardware resolves their ZBC indices through
//! the host's one table. Then the adding setters go to the host's table on the device's own
//! client, capped per VM (`CARAXES_ZBC_MAX`), the indexed setter is refused, and the getters
//! show the defaults and this VM's entries at their host indices. Without the switch nothing
//! changes (docs/research/zbc-host-table-2026-09-28.md).

use serde::Deserialize;

use crate::ctrl::{Answer, HostRm};
use crate::hostrm::{
    CMD_GET_ZBC_CLEAR_TABLE_ENTRY, CMD_SET_ZBC_COLOR_CLEAR, CMD_SET_ZBC_DEPTH_CLEAR,
    CMD_SET_ZBC_STENCIL_CLEAR, GF100_ZBC_CLEAR,
};
use crate::ver::Abi;

/// D9 label of a setter.
pub const RECORDED: &str = "guest (ZBC table of this VM; never written to the host's table)";
/// D9 label of a getter.
pub const READ: &str = "guest (ZBC table of this VM: the chip defaults read once from the host, then this guest's entries)";

/// One table entry: the value words (colour: `colorFB[4]` then `colorDS[4]`; depth and stencil:
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

    fn table_mut(&mut self, ty: u32) -> Option<&mut Table> {
        let e = |n: &str| Abi::active().d_enum(n);
        if Some(ty) == e("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_COLOR") {
            Some(&mut self.color)
        } else if Some(ty) == e("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_DEPTH") {
            Some(&mut self.depth)
        } else if Some(ty) == e("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_STENCIL") {
            Some(&mut self.stencil)
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
    control_with(t, name, params, READ)
}

/// [`control`] with the getters labelled `read`.
fn control_with(t: &mut ZbcTable, name: &str, params: &[u8], read: &'static str) -> Option<Answer> {
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
                    read,
                    format!("ZBC entry: table type {ty} unknown"),
                ));
            };
            if index < tab.start || index > tab.end {
                return Some(answer(
                    bad,
                    b,
                    read,
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
                read,
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
                return Some(answer(0, b, read, "ZBC clear table: index range".into()));
            }
            let index = rd(params, off(GET_TABLE, "indexSize"));
            let Some(tab) = t.table(ty) else {
                return Some(answer(
                    bad,
                    b,
                    read,
                    format!("ZBC clear table: value type {ty} unknown"),
                ));
            };
            if index < tab.start || index > tab.end {
                return Some(answer(
                    bad,
                    b,
                    read,
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
                read,
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

// ---------------------------------------------------------------------------------
// Real compression per VM slice: the host's table.

/// `CARAXES_COMPRESSION=slice`: real compression per VM slice
/// (docs/research/compression-isolation-2026-09-28.md). A ZBC fast clear on a compressible
/// kind stores a table index in the compbits, and the hardware resolves it through the one
/// table of the host GPU (mem_mgr_gm107.c:516-523 of 615.71.09), so the guest's entries go
/// to the host's table ([`SliceZbc`]). Anything else keeps this VM's local table. The one
/// parser of the switch is compslice.rs [`requested`](crate::compslice::requested).
pub const COMPRESSION_ENV: &str = crate::compslice::ENV;
/// `CARAXES_ZBC_MAX`: how many free slots of each host table (colour, depth, stencil) one VM
/// may fill, 0-15; an equal entry the host already holds costs nothing.
pub const ZBC_MAX_ENV: &str = "CARAXES_ZBC_MAX";
/// The default cap: each host table has 15 slots, 2-3 of them the chip defaults; at most
/// two VMs run beside the desktop, so two full shares still leave the desktop four or more.
pub const DEFAULT_MAX: u32 = 4;

/// `NV_ERR_INSUFFICIENT_ZBC_ENTRY` (nvstatuscodes.h:157 of 610.57.04 and 615.71.09, "No more
/// ZBC entry for the client"): what the host answered `SET_ZBC_COLOR_CLEAR` of a new colour on
/// its full colour table (caraxes/runs/comp-zbc-2026-09-28/zbc-host-probe-full.txt), so the
/// cap answers it too. The local table's full answer (NV_ERR_INSUFFICIENT_RESOURCES, the
/// header's text, ctrl9096.h:109-111) stays as it was.
pub const NV_ERR_INSUFFICIENT_ZBC_ENTRY: u32 = 0x80;

/// D9 label of a forwarded setter.
pub const HOST_SET: &str =
    "host (forwarded to the host's ZBC table on the device's own client; the host dedups and places the entry)";
/// D9 label of a getter with real compression.
pub const HOST_VIEW: &str = "host (the host's ZBC table as this VM sees it: the chip defaults and this VM's entries at their host indices; other tenants' entries read as empty)";
/// D9 label of a setter refused at the cap.
pub const CAPPED: &str =
    "refused (this VM filled CARAXES_ZBC_MAX slots of the host table: the answer of a full table)";
/// D9 label of a setter refused here, never forwarded.
pub const REFUSED_HERE: &str =
    "refused (isolation: never forwarded to the host's shared ZBC table)";
/// D9 label of the guest's ZBC-surface flag.
pub const REFERENCED: &str =
    "guest (this guest's ZBC-kind surfaces recorded; not forwarded: kernel-privileged on the host)";

/// Whether `CARAXES_COMPRESSION=slice`, by the switch's one parser (compslice.rs; a value
/// it refuses ends start-up in main.rs before this is read).
pub fn slice_from_env() -> bool {
    crate::compslice::requested() == Ok(crate::compslice::Mode::Slice)
}

/// `CARAXES_ZBC_MAX`, or [`DEFAULT_MAX`].
pub fn max_from_env() -> u32 {
    parse_max(std::env::var(ZBC_MAX_ENV).ok().as_deref())
}

fn parse_max(v: Option<&str>) -> u32 {
    match v.map(str::trim) {
        None | Some("") => DEFAULT_MAX,
        Some(s) => s.parse::<u32>().map(|n| n.min(15)).unwrap_or_else(|_| {
            log::warn!("{ZBC_MAX_ENV}={s:?} is no number; the cap stays {DEFAULT_MAX}");
            DEFAULT_MAX
        }),
    }
}

/// A host slot this VM's guest set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostRef {
    /// `NV9096_CTRL_ZBC_CLEAR_TABLE_TYPE`: 1 colour, 2 depth, 3 stencil.
    pub ty: u32,
    /// The host's index: the one the hardware stores in the compbits of this VM's clears.
    pub index: u32,
    pub entry: Entry,
    /// This VM's set filled a free slot (counts against the cap); else the host already
    /// held an equal entry (a default, the desktop's or another VM's).
    pub filled: bool,
}

/// Counters for the exit log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SliceStats {
    /// Setters forwarded to the host.
    pub forwarded: u64,
    /// ... that met an equal entry, and that filled a free slot.
    pub equal: u64,
    pub filled: u64,
    /// ... that the host answered [`NV_ERR_INSUFFICIENT_ZBC_ENTRY`] (its table full).
    pub full: u64,
    /// Setters answered [`NV_ERR_INSUFFICIENT_ZBC_ENTRY`] at the cap, not forwarded.
    pub capped: u64,
    /// Setters refused here (the indexed setter, an INVALID format), not forwarded.
    pub refused: u64,
    /// Entries of this VM the host table no longer held at a later scan (a host flush).
    pub lost: u64,
}

/// The ZBC state of one VM with real compression.
///
/// Host side (the RM source of 615.71.09, K = src/nvidia/src/kernel):
/// * any client may add: `GF100_ZBC_CLEAR` is allocated NON_PRIVILEGED under a subdevice
///   (K/rmapi/resource_list.h:874-884), and the three setters and GET_ZBC_CLEAR_TABLE_ENTRY
///   are NON_PRIVILEGED and ROUTE_TO_PHYSICAL (flags 0x10248, generated/g_zbc_api_nvoc.c:181,
///   196, 241, 271): GSP-RM keeps the table and places the entry; the setters return no
///   index (ctrl9096.h:118-123, :167-171, :343-347), so the device reads the table back;
/// * nothing is counted per entry or per client: the kernel's ZbcApi has an empty
///   constructor and destructor (generated/g_zbc_api_nvoc.h:416-422), so freeing the object
///   or the client releases no entry;
/// * the only count is of ZBC-kind surfaces (`zbcSurfaces`, mem_mgr_gm107.c:539-563 at
///   allocation, :611-634 at free); on 0 <-> 1 the kernel tells GSP-RM with
///   `INTERNAL_MEMSYS_SET_ZBC_REFERENCED` (mem_mgr_gm107.c:265-320), and GSP-RM may flush
///   the table only when the PF and every VF report none (ctrl0080internal.h:117-126). That
///   control needs kernel privilege or a VF (kern_mem_sys_ctrl.c:1316); a compressed
///   `NV01_MEMORY_LOCAL_PHYSICAL` skips the count (K/mem_mgr/phys_mem.c:119-120).
///
/// So a VM's entries live as long as the host table does, and what keeps them from a flush
/// while the guest still has ZBC-cleared surfaces is a ZBC-kind allocation on the host (the
/// slice's own, or an anchor held while [`SliceZbc::referenced`] is true). The cap bounds
/// the slots a VM can take for the rest of the host session.
///
/// The guest's view ([`SliceZbc::view`]) is the chip's defaults (the host seeds the same,
/// chips/tu106/zbc.toml) and this VM's entries at their host indices. The desktop's and other
/// VMs' entries read as empty: the hardware matches clear values against the whole table
/// anyway, and nothing in the guest needs another tenant's clear colours.
#[derive(Clone, Debug)]
pub struct SliceZbc {
    /// What the guest's getters read.
    pub view: ZbcTable,
    seed: ZbcTable,
    /// `CARAXES_ZBC_MAX`.
    pub max: u32,
    /// The host slots this VM's guest set, in order.
    pub refs: Vec<HostRef>,
    /// The guest kernel's last `INTERNAL_MEMSYS_SET_ZBC_REFERENCED`: whether it has
    /// ZBC-kind surfaces (`None` before the first).
    pub referenced: Option<bool>,
    pub stats: SliceStats,
}

impl SliceZbc {
    pub fn new(chip: &str, max: u32) -> Self {
        let seed = ZbcTable::for_chip(chip);
        SliceZbc {
            view: seed.clone(),
            seed,
            max,
            refs: Vec::new(),
            referenced: None,
            stats: SliceStats::default(),
        }
    }

    /// Host slots this VM filled in table `ty`.
    pub fn filled(&self, ty: u32) -> usize {
        self.refs.iter().filter(|r| r.ty == ty && r.filled).count()
    }

    /// Drop this VM's entries of table `ty` that the host no longer holds at their index
    /// (the host flushed its table); their slots show the seed again. Returns how many.
    fn reconcile(&mut self, ty: u32, host: &[(u32, Option<Entry>)]) -> usize {
        let gone: Vec<HostRef> = self
            .refs
            .iter()
            .filter(|r| {
                r.ty == ty
                    && host
                        .iter()
                        .find(|(i, _)| *i == r.index)
                        .is_some_and(|(_, e)| e.as_ref() != Some(&r.entry))
            })
            .cloned()
            .collect();
        for r in &gone {
            self.refs.retain(|x| x != r);
            let seed = self.seed.table(ty).and_then(|t| t.get(r.index)).cloned();
            if let Some(t) = self.view.table_mut(ty) {
                t.set(r.index, seed);
            }
        }
        self.stats.lost += gone.len() as u64;
        gone.len()
    }

    fn record(&mut self, ty: u32, index: u32, entry: Entry, filled: bool) {
        match self
            .refs
            .iter_mut()
            .find(|r| r.ty == ty && r.index == index)
        {
            Some(r) if r.entry == entry => r.filled |= filled,
            Some(r) => {
                *r = HostRef {
                    ty,
                    index,
                    entry: entry.clone(),
                    filled,
                }
            }
            None => self.refs.push(HostRef {
                ty,
                index,
                entry: entry.clone(),
                filled,
            }),
        }
        if let Some(t) = self.view.table_mut(ty) {
            t.set(index, Some(entry));
        }
    }

    /// VM exit: what this VM leaves in the host's table, then the VM's state is dropped.
    /// No entry can be released on the host (no control removes one, and the client's
    /// objects hold none); the VM's hold on the table is its ZBC-kind allocation, which
    /// the slice's free drops.
    pub fn release(&mut self) -> Vec<String> {
        let list = |filled: bool| -> String {
            let per: Vec<String> = [(1, "colour"), (2, "depth"), (3, "stencil")]
                .iter()
                .filter_map(|&(ty, n)| {
                    let ix: Vec<String> = self
                        .refs
                        .iter()
                        .filter(|r| r.ty == ty && (!filled || r.filled))
                        .map(|r| r.index.to_string())
                        .collect();
                    (!ix.is_empty()).then(|| format!("{n} {}", ix.join(",")))
                })
                .collect();
            if per.is_empty() {
                "none".into()
            } else {
                per.join("; ")
            }
        };
        let st = &self.stats;
        let out = vec![
            format!(
                "ZBC (real compression): host entries this VM used: {}; slots it filled: {} (cap {} per table); they stay in the host table until GSP-RM flushes it, which it may only once no ZBC-kind surface is left on the GPU (ctrl0080internal.h:117-126)",
                list(false),
                list(true),
                self.max
            ),
            format!(
                "ZBC (real compression): {} setter(s) forwarded ({} equal, {} filled, {} host table full), {} at the cap, {} refused here, {} entr(ies) lost to a host flush; guest ZBC-kind surfaces at exit: {}",
                st.forwarded,
                st.equal,
                st.filled,
                st.full,
                st.capped,
                st.refused,
                st.lost,
                match self.referenced {
                    Some(true) => "yes",
                    Some(false) => "none",
                    None => "never reported",
                }
            ),
        ];
        self.refs.clear();
        self.view = self.seed.clone();
        self.referenced = None;
        out
    }
}

/// The host's table `ty` over `start..=end`: each index with its entry, `None` where the host
/// reports it unused; `Err` with the host status of a failed read. The layout is the guest
/// ABI's, identical in 610.57.04 and 615.71.09 (ctrl9096.h:431-442).
fn host_table(
    host: &mut dyn HostRm,
    ty: u32,
    (start, end): (u32, u32),
) -> Result<Vec<(u32, Option<Entry>)>, u32> {
    let size = Abi::active().layout(GET_ENTRY).map_or(56, |l| l.size);
    let mut out = Vec::new();
    for index in start..=end {
        let mut b = vec![0u8; size];
        wr(&mut b, off(GET_ENTRY, "tableType"), ty);
        wr(&mut b, off(GET_ENTRY, "index"), index);
        let st = host.control(GF100_ZBC_CLEAR, CMD_GET_ZBC_CLEAR_TABLE_ENTRY, &mut b);
        if st != 0 {
            return Err(st);
        }
        let valid = b
            .get(off(GET_ENTRY, "bIndexValid"))
            .is_some_and(|&v| v != 0);
        out.push((
            index,
            valid.then(|| Entry {
                value: entry_value(&b, GET_ENTRY, ty),
                format: rd(&b, off(GET_ENTRY, "format")),
            }),
        ));
    }
    Ok(out)
}

/// The value words of a getter's `value` for table `ty`, laid out as [`Entry::value`].
fn entry_value(b: &[u8], st: &str, ty: u32) -> Vec<u32> {
    let e = |n: &str| Abi::active().d_enum(n);
    if Some(ty) == e("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_COLOR") {
        (0..4)
            .map(|i| rd(b, off(st, &format!("value.colorFB[{i}]"))))
            .chain((0..4).map(|i| rd(b, off(st, &format!("value.colorDS[{i}]")))))
            .collect()
    } else if Some(ty) == e("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_DEPTH") {
        vec![rd(b, off(st, "value.depth"))]
    } else {
        vec![rd(b, off(st, "value.stencil"))]
    }
}

/// Where the host put `e`: the index holding an equal entry after the set, else the one slot
/// that went from unused to used (the host may store a value differently); the entry as the
/// host holds it.
fn placed(
    before: &[(u32, Option<Entry>)],
    after: &[(u32, Option<Entry>)],
    e: &Entry,
) -> Option<(u32, Entry)> {
    if let Some((i, _)) = after.iter().find(|(_, x)| x.as_ref() == Some(e)) {
        return Some((*i, e.clone()));
    }
    let new: Vec<(u32, Entry)> = after
        .iter()
        .filter(|(i, x)| x.is_some() && before.iter().any(|(j, y)| j == i && y.is_none()))
        .map(|(i, x)| (*i, x.clone().unwrap()))
        .collect();
    (new.len() == 1).then(|| new[0].clone())
}

/// A guest setter with real compression: the cap, then the host's table.
#[allow(clippy::too_many_arguments)]
fn slice_set(
    s: &mut SliceZbc,
    host: &mut dyn HostRm,
    cmd: u32,
    st: &str,
    ty: u32,
    e: Entry,
    params: &[u8],
    what: &str,
) -> Answer {
    let b = params.to_vec();
    let full = [
        NV_ERR_INSUFFICIENT_ZBC_ENTRY,
        status("NV_ERR_INSUFFICIENT_RESOURCES"),
    ];
    // ctrl9096.h:62, :139, :317: format INVALID (0) names no entry; the depth header keeps
    // it "to disable/remove an entry" (:133-136), which must never reach the shared table.
    if e.format == 0 {
        s.stats.refused += 1;
        return answer(
            status("NV_ERR_INVALID_ARGUMENT"),
            b,
            REFUSED_HERE,
            format!("ZBC {what}: format INVALID, not forwarded"),
        );
    }
    let Some(range) = s.seed.table(ty).map(|t| (t.start, t.end)) else {
        return answer(
            status("NV_ERR_INVALID_ARGUMENT"),
            b,
            REFUSED_HERE,
            format!("ZBC {what}: no table"),
        );
    };
    let before = match host_table(host, ty, range) {
        Ok(t) => t,
        Err(st) => {
            return answer(
                st,
                b,
                HOST_SET,
                format!("ZBC {what}: host table unreadable (GET_ZBC_CLEAR_TABLE_ENTRY status {st:#x}), not forwarded"),
            )
        }
    };
    let lost = s.reconcile(ty, &before);
    let lost = if lost > 0 {
        format!("; {lost} earlier entr(ies) of this VM gone from the host table (flushed)")
    } else {
        String::new()
    };
    let equal = before
        .iter()
        .find(|(_, x)| x.as_ref() == Some(&e))
        .map(|(i, _)| *i);
    if equal.is_none() && s.filled(ty) >= s.max as usize {
        s.stats.capped += 1;
        return answer(
            NV_ERR_INSUFFICIENT_ZBC_ENTRY,
            b,
            CAPPED,
            format!(
                "ZBC {what}: this VM filled {} host slot(s), its cap is {}; not forwarded{lost}",
                s.filled(ty),
                s.max
            ),
        );
    }
    // bSkipL2Table is for verification platforms (ctrl9096.h:101-107): a DS entry without its
    // L2 value would make a matching clear read back whatever the L2 slot held.
    let mut h = b.clone();
    if let Some(x) = h.get_mut(off(st, "bSkipL2Table")) {
        *x = 0;
    }
    let hs = host.control(GF100_ZBC_CLEAR, cmd, &mut h);
    s.stats.forwarded += 1;
    if hs != 0 {
        let why = if full.contains(&hs) {
            s.stats.full += 1;
            " (the host table is full: this VM's clears of the value stay non-ZBC, as on a full card)"
        } else {
            ""
        };
        return answer(
            hs,
            b,
            HOST_SET,
            format!("ZBC {what}: host status {hs:#x}{why}{lost}"),
        );
    }
    let at = match equal {
        Some(i) => Some((i, e.clone(), false)),
        None => host_table(host, ty, range)
            .ok()
            .and_then(|after| placed(&before, &after, &e))
            .map(|(i, x)| (i, x, true)),
    };
    let Some((index, stored, filled)) = at else {
        return answer(
            0,
            b,
            HOST_SET,
            format!("ZBC {what}: the host took it, but its table shows no such entry; not in this VM's view{lost}"),
        );
    };
    if filled {
        s.stats.filled += 1;
    } else {
        s.stats.equal += 1;
    }
    s.record(ty, index, stored, filled);
    answer(
        0,
        b,
        HOST_SET,
        format!(
            "ZBC {what} at host index {index} ({}){lost}",
            if filled {
                "filled a free slot"
            } else {
                "an equal entry the host held"
            }
        ),
    )
}

/// The ZBC controls with real compression. The three adding setters go to the host's table
/// through the device's own client, the indexed one is refused as a vGPU guest's is, and the
/// getters read [`SliceZbc::view`]. Without a host client the VM's view answers as the local
/// table does.
pub fn slice_control(
    s: &mut SliceZbc,
    host: Option<&mut dyn HostRm>,
    name: &str,
    params: &[u8],
) -> Option<Answer> {
    let words = |st: &str, field: &str, n: usize| -> Vec<u32> {
        (0..n)
            .map(|i| rd(params, off(st, &format!("{field}[{i}]"))))
            .collect()
    };
    let e = |n: &str| Abi::active().d_enum(n).unwrap_or(0);
    let set = match name {
        "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR" => {
            let mut value = words(COLOR, "colorFB", 4);
            value.extend(words(COLOR, "colorDS", 4));
            let format = rd(params, off(COLOR, "format"));
            let ty = e("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_COLOR");
            Some((
                CMD_SET_ZBC_COLOR_CLEAR,
                COLOR,
                ty,
                Entry { value, format },
                "colour",
            ))
        }
        "NV9096_CTRL_CMD_SET_ZBC_DEPTH_CLEAR" => {
            let x = Entry {
                value: vec![rd(params, off(DEPTH, "depth"))],
                format: rd(params, off(DEPTH, "format")),
            };
            let ty = e("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_DEPTH");
            Some((CMD_SET_ZBC_DEPTH_CLEAR, DEPTH, ty, x, "depth"))
        }
        "NV9096_CTRL_CMD_SET_ZBC_STENCIL_CLEAR" => {
            let x = Entry {
                value: vec![rd(params, off(STENCIL, "stencil"))],
                format: rd(params, off(STENCIL, "format")),
            };
            let ty = e("NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_STENCIL");
            Some((CMD_SET_ZBC_STENCIL_CLEAR, STENCIL, ty, x, "stencil"))
        }
        _ => None,
    };
    let Some(host) = host else {
        return control(&mut s.view, name, params);
    };
    if let Some((cmd, st, ty, entry, what)) = set {
        return Some(slice_set(s, host, cmd, st, ty, entry, params, what));
    }
    match name {
        // The deprecated indexed setter (flags 0x48: ROUTE_TO_PHYSICAL without
        // ROUTE_TO_VGPU_HOST, g_zbc_api_nvoc.c:226) is NOT_SUPPORTED on a vGPU guest
        // (K/rmapi/control.c:779-789), and forwarded it would overwrite a slot another
        // tenant's surfaces resolve through.
        "NV9096_CTRL_CMD_SET_ZBC_CLEAR_TABLE" => {
            s.stats.refused += 1;
            Some(answer(
                crate::ver::common::NV_ERR_NOT_SUPPORTED,
                params.to_vec(),
                REFUSED_HERE,
                "ZBC clear table (indexed setter): not supported, as on a vGPU guest".into(),
            ))
        }
        _ => control_with(&mut s.view, name, params, HOST_VIEW),
    }
}

/// `NV0080_CTRL_CMD_INTERNAL_MEMSYS_SET_ZBC_REFERENCED` of the guest kernel: its ZBC-kind
/// surfaces exist, or the last one is gone (mem_mgr_gm107.c:556-557, 623-624). A vGPU guest
/// sends it to the host plugin (K/vgpu/rpc.c:4812-4816), which reports per VF to GSP-RM; the
/// device cannot (kernel privilege, kern_mem_sys_ctrl.c:1316), so it records the flag for
/// the VM's hold on the host table ([`SliceZbc`]) and acknowledges.
pub fn referenced(s: &mut SliceZbc, params: &[u8]) -> Answer {
    // ctrl0080internal.h:131-133: { NvBool bZbcSurfacesExist }.
    let on = params.first().is_some_and(|&v| v != 0);
    let was = s.referenced.replace(on);
    answer(
        0,
        params.to_vec(),
        REFERENCED,
        format!(
            "ZBC referenced: guest ZBC-kind surfaces {} (was {})",
            if on { "exist" } else { "none" },
            match was {
                Some(true) => "exist",
                Some(false) => "none",
                None => "unreported",
            }
        ),
    )
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

    // -----------------------------------------------------------------------------
    // Real compression: the host's table (SliceZbc) against a fake host.

    use crate::hostrm::zbc_control_allowed;
    use std::sync::{Arc, Mutex};

    /// A host ZBC table as the RM source and the Q5 sweep show it: GSP-RM's defaults, an
    /// equal entry referenced again, else the first free slot, else
    /// NV_ERR_INSUFFICIENT_ZBC_ENTRY (as the host answered, zbc-host-probe-full.txt); the
    /// getters by type and index. Every control is
    /// recorded; a control the device must never send to this object panics.
    #[derive(Clone, Default)]
    struct FakeZbcHost(Arc<Mutex<Fake>>);

    #[derive(Default)]
    struct Fake {
        /// Colour, depth, stencil: slot i is index i + 1.
        tables: [Vec<Option<Entry>>; 3],
        calls: Vec<(u32, u32)>,
        /// bSkipL2Table of every forwarded setter.
        skip: Vec<u8>,
    }

    impl FakeZbcHost {
        /// The chip defaults, then `desktop` entries (type, value, format) in the next slots.
        fn new(desktop: &[(u32, Vec<u32>, u32)]) -> Self {
            let seed = ZbcTable::for_chip("tu106");
            let mut f = Fake::default();
            for (k, t) in [&seed.color, &seed.depth, &seed.stencil]
                .into_iter()
                .enumerate()
            {
                f.tables[k] = (t.start..=t.end).map(|i| t.get(i).cloned()).collect();
            }
            let h = FakeZbcHost(Arc::new(Mutex::new(f)));
            for (ty, value, format) in desktop {
                h.add(
                    *ty,
                    Entry {
                        value: value.clone(),
                        format: *format,
                    },
                );
            }
            h
        }

        fn add(&self, ty: u32, e: Entry) -> Option<u32> {
            let mut f = self.0.lock().unwrap();
            let t = &mut f.tables[ty as usize - 1];
            if let Some(i) = t.iter().position(|x| x.as_ref() == Some(&e)) {
                return Some(i as u32 + 1);
            }
            let i = t.iter().position(|x| x.is_none())?;
            t[i] = Some(e);
            Some(i as u32 + 1)
        }

        /// GSP-RM's flush: back to the defaults.
        fn flush(&self) {
            let seed = FakeZbcHost::new(&[]);
            let tables = seed.0.lock().unwrap().tables.clone();
            self.0.lock().unwrap().tables = tables;
        }

        fn sets(&self) -> usize {
            self.0
                .lock()
                .unwrap()
                .calls
                .iter()
                .filter(|c| {
                    [
                        CMD_SET_ZBC_COLOR_CLEAR,
                        CMD_SET_ZBC_DEPTH_CLEAR,
                        CMD_SET_ZBC_STENCIL_CLEAR,
                    ]
                    .contains(&c.1)
                })
                .count()
        }

        fn calls(&self) -> usize {
            self.0.lock().unwrap().calls.len()
        }

        fn at(&self, ty: u32, index: u32) -> Option<Entry> {
            self.0.lock().unwrap().tables[ty as usize - 1][index as usize - 1].clone()
        }
    }

    impl HostRm for FakeZbcHost {
        fn control(&mut self, class: u32, cmd: u32, p: &mut [u8]) -> u32 {
            self.0.lock().unwrap().calls.push((class, cmd));
            if class != GF100_ZBC_CLEAR {
                return crate::ver::common::NV_ERR_NOT_SUPPORTED;
            }
            assert!(
                zbc_control_allowed(cmd),
                "{cmd:#x} sent to the host ZBC object"
            );
            let setter = |st: &str, ty: u32, value: Vec<u32>, p: &[u8]| {
                self.0.lock().unwrap().skip.push(p[off(st, "bSkipL2Table")]);
                let e = Entry {
                    value,
                    format: rd(p, off(st, "format")),
                };
                match self.add(ty, e) {
                    Some(_) => 0,
                    None => NV_ERR_INSUFFICIENT_ZBC_ENTRY,
                }
            };
            match cmd {
                CMD_SET_ZBC_COLOR_CLEAR => {
                    let mut v: Vec<u32> = (0..4)
                        .map(|i| rd(p, off(COLOR, &format!("colorFB[{i}]"))))
                        .collect();
                    v.extend((0..4).map(|i| rd(p, off(COLOR, &format!("colorDS[{i}]")))));
                    setter(COLOR, 1, v, p)
                }
                CMD_SET_ZBC_DEPTH_CLEAR => setter(DEPTH, 2, vec![rd(p, off(DEPTH, "depth"))], p),
                CMD_SET_ZBC_STENCIL_CLEAR => {
                    setter(STENCIL, 3, vec![rd(p, off(STENCIL, "stencil"))], p)
                }
                CMD_GET_ZBC_CLEAR_TABLE_ENTRY => {
                    let (ty, index) = (
                        rd(p, off(GET_ENTRY, "tableType")),
                        rd(p, off(GET_ENTRY, "index")),
                    );
                    if !(1..=3).contains(&ty) || !(1..=15).contains(&index) {
                        return status("NV_ERR_INVALID_ARGUMENT");
                    }
                    let e = self.at(ty, index);
                    fill_value(p, GET_ENTRY, ty, e.as_ref());
                    wr(
                        p,
                        off(GET_ENTRY, "format"),
                        e.as_ref().map_or(0, |e| e.format),
                    );
                    p[off(GET_ENTRY, "bIndexValid")] = e.is_some() as u8;
                    0
                }
                _ => 0,
            }
        }
    }

    fn color_params(w: u32, format: u32) -> Vec<u8> {
        let mut p = vec![0u8; 40];
        for i in 0..4 {
            wr(&mut p, off(COLOR, &format!("colorFB[{i}]")), w + i as u32);
            wr(&mut p, off(COLOR, &format!("colorDS[{i}]")), w);
        }
        wr(&mut p, off(COLOR, "format"), format);
        p
    }

    fn color_entry(w: u32, format: u32) -> Entry {
        let mut value: Vec<u32> = (0..4).map(|i| w + i as u32).collect();
        value.extend([w; 4]);
        Entry { value, format }
    }

    fn slice_set_color(s: &mut SliceZbc, h: &mut FakeZbcHost, w: u32) -> Answer {
        let p = color_params(w, 0x28);
        slice_control(
            s,
            Some(h as &mut dyn HostRm),
            "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR",
            &p,
        )
        .unwrap()
    }

    fn slice_entry(s: &mut SliceZbc, h: &mut FakeZbcHost, ty: u32, index: u32) -> Answer {
        let mut p = vec![0u8; 56];
        wr(&mut p, off(GET_ENTRY, "tableType"), ty);
        wr(&mut p, off(GET_ENTRY, "index"), index);
        slice_control(
            s,
            Some(h as &mut dyn HostRm),
            "NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY",
            &p,
        )
        .unwrap()
    }

    /// A new colour goes to the host (bSkipL2Table cleared) and lands in the host's first
    /// free slot, after the desktop's two; the guest reads it back there, reads the
    /// defaults, and reads the desktop's slots as empty.
    #[test]
    fn slice_forwards_and_learns_the_host_index() {
        let mut h = FakeZbcHost::new(&[
            (1, color_entry(0xFF11_0000, 0x28).value, 0x28),
            (1, color_entry(0xFF22_0000, 0x18).value, 0x18),
        ]);
        let mut s = SliceZbc::new("tu106", DEFAULT_MAX);
        let mut p = color_params(0x3F00_0000, 0x28);
        p[off(COLOR, "bSkipL2Table")] = 1;
        let a = slice_control(
            &mut s,
            Some(&mut h as &mut dyn HostRm),
            "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR",
            &p,
        )
        .unwrap();
        assert_eq!((a.status, a.label, &a.params), (0, HOST_SET, &p));
        assert!(
            a.note.contains("host index 6 (filled a free slot)"),
            "{}",
            a.note
        );
        assert_eq!(h.at(1, 6), Some(color_entry(0x3F00_0000, 0x28)));
        assert_eq!(
            h.0.lock().unwrap().skip,
            [0],
            "bSkipL2Table never reaches the host"
        );
        // 15 reads before, the set, 15 reads after.
        assert_eq!((h.calls(), h.sets()), (31, 1));
        assert_eq!(
            s.refs,
            [HostRef {
                ty: 1,
                index: 6,
                entry: color_entry(0x3F00_0000, 0x28),
                filled: true
            }]
        );
        let a = slice_entry(&mut s, &mut h, 1, 6);
        assert_eq!((a.status, a.label), (0, HOST_VIEW));
        assert_eq!(
            rd(&a.params, off(GET_ENTRY, "value.colorFB[1]")),
            0x3F00_0001
        );
        assert_eq!(a.params[off(GET_ENTRY, "bIndexValid")], 1);
        let a = slice_entry(&mut s, &mut h, 1, 4);
        assert_eq!(
            a.params[off(GET_ENTRY, "bIndexValid")],
            0,
            "the desktop's entry is hidden"
        );
        assert_eq!(rd(&a.params, off(GET_ENTRY, "value.colorFB[0]")), 0);
        let a = slice_entry(&mut s, &mut h, 1, 2);
        assert_eq!(
            rd(&a.params, off(GET_ENTRY, "value.colorFB[0]")),
            0xFFFF_FFFF,
            "a default"
        );
        assert_eq!(
            h.calls(),
            31,
            "the getters read this VM's view, not the host"
        );
    }

    /// An entry the host already holds (the desktop's) costs no slot: it is referenced at
    /// the desktop's index and becomes visible; with a cap of 0 only such entries pass.
    #[test]
    fn an_equal_entry_costs_no_slot() {
        let desk = color_entry(0xFF11_0000, 0x28);
        let mut h = FakeZbcHost::new(&[(1, desk.value.clone(), 0x28)]);
        let mut s = SliceZbc::new("tu106", 0);
        let a = slice_set_color(&mut s, &mut h, 0xFF11_0000);
        assert_eq!(a.status, 0);
        assert!(
            a.note
                .contains("host index 4 (an equal entry the host held)"),
            "{}",
            a.note
        );
        assert_eq!(
            (h.calls(), h.sets()),
            (16, 1),
            "no scan after an equal entry"
        );
        assert_eq!(s.filled(1), 0);
        assert_eq!(
            slice_entry(&mut s, &mut h, 1, 4).params[off(GET_ENTRY, "bIndexValid")],
            1
        );
        let a = slice_set_color(&mut s, &mut h, 0x1234_0000);
        assert_eq!((a.status, a.label), (NV_ERR_INSUFFICIENT_ZBC_ENTRY, CAPPED));
        assert_eq!(h.sets(), 1, "the capped setter is not forwarded");
        assert_eq!(s.stats.equal, 1);
        assert_eq!(s.stats.capped, 1);
    }

    /// The cap counts the host slots a VM filled, per table: the third new colour is refused
    /// without reaching the host; its own entries and a depth value still pass.
    #[test]
    fn the_cap_bounds_the_slots_a_vm_fills() {
        let mut h = FakeZbcHost::new(&[]);
        let mut s = SliceZbc::new("tu106", 2);
        assert_eq!(slice_set_color(&mut s, &mut h, 0x1000_0000).status, 0);
        assert_eq!(slice_set_color(&mut s, &mut h, 0x2000_0000).status, 0);
        assert_eq!(s.filled(1), 2);
        let a = slice_set_color(&mut s, &mut h, 0x3000_0000);
        assert_eq!((a.status, a.label), (NV_ERR_INSUFFICIENT_ZBC_ENTRY, CAPPED));
        assert_eq!(h.sets(), 2);
        assert_eq!(h.at(1, 6), None, "no third slot on the host");
        let a = slice_set_color(&mut s, &mut h, 0x1000_0000);
        assert!(
            a.note.contains("host index 4 (an equal entry"),
            "{}",
            a.note
        );
        let mut d = vec![0u8; 12];
        wr(&mut d, off(DEPTH, "depth"), 0x3F00_0000);
        wr(&mut d, off(DEPTH, "format"), 1);
        let a = slice_control(
            &mut s,
            Some(&mut h as &mut dyn HostRm),
            "NV9096_CTRL_CMD_SET_ZBC_DEPTH_CLEAR",
            &d,
        )
        .unwrap();
        assert!(a.note.contains("host index 3 (filled"), "{}", a.note);
        assert_eq!((s.filled(1), s.filled(2)), (2, 1));
        assert_eq!(s.refs.len(), 3);
    }

    /// A full host table answers NV_ERR_INSUFFICIENT_ZBC_ENTRY, which the guest gets as a
    /// real full card gives it; nothing is recorded.
    #[test]
    fn a_full_host_table_answers_as_a_full_card() {
        let desk: Vec<(u32, Vec<u32>, u32)> = (0..12)
            .map(|k| (1, color_entry(0xFF00_0000 + (k << 8), 0x28).value, 0x28))
            .collect();
        let mut h = FakeZbcHost::new(&desk);
        let mut s = SliceZbc::new("tu106", DEFAULT_MAX);
        let a = slice_set_color(&mut s, &mut h, 0x4000_0000);
        assert_eq!(
            (a.status, a.label),
            (NV_ERR_INSUFFICIENT_ZBC_ENTRY, HOST_SET)
        );
        assert!(a.note.contains("host table is full"), "{}", a.note);
        assert!(s.refs.is_empty());
        assert_eq!(s.view, ZbcTable::for_chip("tu106"));
        assert_eq!((s.stats.forwarded, s.stats.full), (1, 1));
    }

    /// The indexed setter and a setter with format INVALID never reach the host.
    #[test]
    fn the_indexed_setter_and_invalid_formats_stay_here() {
        let mut h = FakeZbcHost::new(&[]);
        let mut s = SliceZbc::new("tu106", DEFAULT_MAX);
        let mut p = vec![0u8; 60];
        wr(&mut p, off(TABLE, "index"), 5);
        wr(&mut p, off(TABLE, "colorFormat"), 0x28);
        let a = slice_control(
            &mut s,
            Some(&mut h as &mut dyn HostRm),
            "NV9096_CTRL_CMD_SET_ZBC_CLEAR_TABLE",
            &p,
        )
        .unwrap();
        assert_eq!(
            (a.status, a.label),
            (crate::ver::common::NV_ERR_NOT_SUPPORTED, REFUSED_HERE)
        );
        let a = slice_control(
            &mut s,
            Some(&mut h as &mut dyn HostRm),
            "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR",
            &color_params(0x5000_0000, 0),
        )
        .unwrap();
        assert_eq!(
            (a.status, a.label),
            (status("NV_ERR_INVALID_ARGUMENT"), REFUSED_HERE)
        );
        assert_eq!(h.calls(), 0);
        assert_eq!(s.stats.refused, 2);
    }

    /// A host flush empties the VM's entries: the next setter's scan drops them from the
    /// view and from the cap, and the new entry lands in the freed slot.
    #[test]
    fn a_host_flush_drops_the_vms_entries() {
        let mut h = FakeZbcHost::new(&[]);
        let mut s = SliceZbc::new("tu106", 1);
        assert_eq!(slice_set_color(&mut s, &mut h, 0x1000_0000).status, 0);
        h.flush();
        let a = slice_set_color(&mut s, &mut h, 0x2000_0000);
        assert_eq!(
            a.status, 0,
            "the flushed slot no longer counts against the cap"
        );
        assert!(
            a.note.contains("host index 4") && a.note.contains("1 earlier entr(ies)"),
            "{}",
            a.note
        );
        assert_eq!(
            s.refs,
            [HostRef {
                ty: 1,
                index: 4,
                entry: color_entry(0x2000_0000, 0x28),
                filled: true
            }]
        );
        assert_eq!(s.stats.lost, 1);
    }

    /// VM exit: the log names the host slots the VM used and filled; the state is dropped.
    #[test]
    fn release_at_exit() {
        let desk = color_entry(0xFF11_0000, 0x28);
        let mut h = FakeZbcHost::new(&[(1, desk.value.clone(), 0x28)]);
        let mut s = SliceZbc::new("tu106", DEFAULT_MAX);
        slice_set_color(&mut s, &mut h, 0xFF11_0000);
        slice_set_color(&mut s, &mut h, 0x1000_0000);
        referenced(&mut s, &[1]);
        let log = s.release();
        assert!(
            log[0].contains("used: colour 4,5; slots it filled: colour 5 (cap 4 per table)"),
            "{}",
            log[0]
        );
        assert!(
            log[1].contains("2 setter(s) forwarded (1 equal, 1 filled, 0 host table full)"),
            "{}",
            log[1]
        );
        assert!(log[1].ends_with("surfaces at exit: yes"), "{}", log[1]);
        assert!(s.refs.is_empty() && s.referenced.is_none());
        assert_eq!(s.view, ZbcTable::for_chip("tu106"));
        assert_eq!(
            h.at(1, 5),
            Some(color_entry(0x1000_0000, 0x28)),
            "the host keeps its entry"
        );
    }

    /// Without a host client the VM's view answers as the local table.
    #[test]
    fn without_a_host_the_view_is_local() {
        let mut s = SliceZbc::new("tu106", DEFAULT_MAX);
        let a = slice_control(
            &mut s,
            None,
            "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR",
            &color_params(0x1000_0000, 0x28),
        )
        .unwrap();
        assert_eq!((a.status, a.label), (0, RECORDED));
        assert_eq!(s.view.color.used(), 4);
    }

    #[test]
    fn the_cap_from_the_environment_value() {
        assert_eq!(parse_max(None), DEFAULT_MAX);
        assert_eq!(parse_max(Some(" 2 ")), 2);
        assert_eq!(parse_max(Some("0")), 0);
        assert_eq!(parse_max(Some("99")), 15);
        assert_eq!(parse_max(Some("many")), DEFAULT_MAX);
    }

    fn dispatcher(host: FakeZbcHost) -> crate::ctrl::Dispatcher {
        let f = crate::fixture::rtx2070();
        let chip = crate::chip::by_name("tu106").unwrap().data();
        crate::ctrl::Dispatcher::new(chip, &f.identity(4 << 30), Some(Box::new(host)))
    }

    fn id(name: &str) -> u32 {
        Abi::active().routed(name).unwrap().id
    }

    /// Through the dispatcher with the switch: the setter reaches the host's table, the
    /// guest kernel's ZBC-surface flag is recorded and not forwarded.
    #[test]
    fn the_dispatcher_with_real_compression() {
        let h = FakeZbcHost::new(&[]);
        let mut d = dispatcher(h.clone());
        d.dstate.zbc_slice = Some(SliceZbc::new("tu106", DEFAULT_MAX));
        let a = d.control(
            0xC1,
            0x9096,
            id("NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR"),
            &color_params(0x1000_0000, 0x28),
        );
        assert_eq!((a.status, a.label), (0, HOST_SET));
        assert_eq!(h.sets(), 1);
        let calls = h.calls();
        let referenced = id("NV0080_CTRL_CMD_INTERNAL_MEMSYS_SET_ZBC_REFERENCED");
        let a = d.control(0xC1, 0xC2, referenced, &[1]);
        assert_eq!((a.status, a.label), (0, REFERENCED));
        assert_eq!(d.dstate.zbc_slice.as_ref().unwrap().referenced, Some(true));
        assert_eq!(h.calls(), calls, "SET_ZBC_REFERENCED is not forwarded");
        let log = d.dstate.release_zbc();
        assert!(log[0].contains("slots it filled: colour 4"), "{}", log[0]);
    }

    /// Without the switch nothing changes: the dispatcher answers the ZBC controls exactly as
    /// the local table does, byte for byte, acknowledges SET_ZBC_REFERENCED in chipctl.rs and
    /// issues no host control.
    #[test]
    fn the_switch_off_keeps_the_local_table() {
        let h = FakeZbcHost::new(&[]);
        let mut d = dispatcher(h.clone());
        d.dstate.zbc_slice = None;
        let mut local = ZbcTable::for_chip("tu106");
        let mut g = vec![0u8; 56];
        wr(&mut g, off(GET_ENTRY, "tableType"), 1);
        wr(&mut g, off(GET_ENTRY, "index"), 4);
        let mut t = vec![0u8; 60];
        wr(&mut t, off(TABLE, "index"), 9);
        wr(&mut t, off(TABLE, "colorFormat"), 0x28);
        for (name, p) in [
            (
                "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR",
                color_params(0x1000_0000, 0x28),
            ),
            (
                "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR",
                color_params(0x1000_0000, 0),
            ),
            ("NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY", g.clone()),
            ("NV9096_CTRL_CMD_SET_ZBC_CLEAR_TABLE", t.clone()),
        ] {
            let a = d.control(0xC1, 0x9096, id(name), &p);
            let b = control(&mut local, name, &p).unwrap();
            assert_eq!(
                (a.status, &a.params, a.label, &a.note),
                (b.status, &b.params, b.label, &b.note),
                "{name}"
            );
        }
        let a = d.control(
            0xC1,
            0xC2,
            id("NV0080_CTRL_CMD_INTERNAL_MEMSYS_SET_ZBC_REFERENCED"),
            &[1],
        );
        assert_eq!((a.status, a.label), (0, crate::chipctl::ACKED));
        assert_eq!(h.calls(), 0);
        assert!(d.dstate.release_zbc().is_empty());
    }
}

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Pure-emulation answers for the controls a host RM would answer: the RTX 2070's own
//! answers from the native control traces (matrix/traces/610.57.04/control, extracted by
//! caraxes/tools/recorded-controls.py into chips/tu106/recorded-controls.tsv), and chip
//! facts for the few the traces do not cover.
//!
//! The Windows driver 617.14 in GSP-RM mode (the `EnableGpuFirmware` registry key) sends
//! these from its adapter start where the Linux driver does not
//! (caraxes/runs/win11-2026-09-25/night-2026-09-25/WINDOWS-LOOP.md): without a host the
//! dispatcher answered them NOT_SUPPORTED and the driver unloaded GSP-RM right after the
//! last of them. They only fill answers the dispatcher had no model for (an `unanswered`
//! control or a `no host` fallback); every modelled answer stays as it is.
//!
//! Matching, in order: the exact request bytes of a recorded pair; for the index-list
//! controls (`FB_GET_INFO_V2`, `BUS_GET_INFO_V2`: a count and `{index, data}` pairs) every
//! requested index from all recorded answers, the sizes rewritten to the emulated FB as the
//! host path does (nvml.rs `fb_get_info_v2`); `GRMGR_GET_GR_FS_INFO` per query from the GPC
//! and TPC counts; two controls with an all-zero answer (`GPU_QUERY_FUNCTION_STATUS`: no
//! USB-C or PPC function; `INTERNAL_GET_ENABLED_SEC2_CLASSES`: none of the three SEC2
//! classes, the card's class list has none of them).

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use crate::ctrl::Answer;
use crate::ver::active as v;

const TSV: &str = include_str!("../chips/tu106/recorded-controls.tsv");

pub const LABEL: &str =
    "card (native control trace of the RTX 2070 on 610.57.04, chips/tu106/recorded-controls.tsv)";
pub const LABEL_CHIP: &str = "chip (pure emulation: TU106 facts, recorded.rs)";

pub const FB_GET_INFO_V2: u32 = 0x2080_1303;
pub const BUS_GET_INFO_V2: u32 = 0x2080_1823;
pub const GRMGR_GET_GR_FS_INFO: u32 = 0x2080_3801;
pub const GPU_QUERY_FUNCTION_STATUS: u32 = 0x2080_0173;
pub const INTERNAL_GET_ENABLED_SEC2_CLASSES: u32 = 0x2080_0AAF;

const NV_ERR_INVALID_ARGUMENT: u32 = 0x1F;

/// FB info indices (ctrl2080fb.h:318-390) the device answers from the emulated FB.
const FB_LARGEST_FREE_REGION_SIZE_KB: u32 = 0x12;
const FB_ZERO_BASES: [u32; 3] = [0x11, 0x13, 0x1E];
/// BUS info indices of the host's PCI topology (ctrl2080bus.h:348-350): not the guest's.
const BUS_TOPOLOGY: [u32; 3] = [0x0F, 0x10, 0x11];

struct Entry {
    status: u32,
    answer: Vec<u8>,
    source: String,
}

/// The parsed table.
pub struct Recorded {
    exact: HashMap<(u32, Vec<u8>), Entry>,
    /// index → data of the index-list controls, from every recorded answer.
    index: HashMap<u32, BTreeMap<u32, u32>>,
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

fn num(s: &str) -> Option<u32> {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).ok()
}

fn get32(b: &[u8], off: usize) -> u32 {
    b.get(off..off + 4)
        .map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

fn put32(b: &mut [u8], off: usize, v: u32) {
    if let Some(s) = b.get_mut(off..off + 4) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

/// `{count u32, {index u32, data u32}[]}`: the requested entries.
fn list_entries(b: &[u8]) -> usize {
    (get32(b, 0) as usize).min(b.len().saturating_sub(4) / 8)
}

impl Recorded {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut r = Recorded {
            exact: HashMap::new(),
            index: HashMap::new(),
        };
        for (n, line) in text.lines().enumerate() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            let bad = || format!("recorded-controls.tsv:{}: malformed", n + 1);
            if f.len() < 5 {
                return Err(bad());
            }
            let (cmd, status) = (num(f[0]).ok_or_else(bad)?, num(f[1]).ok_or_else(bad)?);
            let (req, ans) = (unhex(f[2]).ok_or_else(bad)?, unhex(f[3]).ok_or_else(bad)?);
            if matches!(cmd, FB_GET_INFO_V2 | BUS_GET_INFO_V2) && status == 0 {
                let m = r.index.entry(cmd).or_default();
                for i in 0..list_entries(&ans) {
                    m.entry(get32(&ans, 4 + 8 * i))
                        .or_insert(get32(&ans, 8 + 8 * i));
                }
            }
            r.exact.entry((cmd, req)).or_insert(Entry {
                status,
                answer: ans,
                source: f[4].to_string(),
            });
        }
        Ok(r)
    }

    /// The embedded table (a malformed file fails the unit test, not the device).
    pub fn get() -> &'static Recorded {
        static T: OnceLock<Recorded> = OnceLock::new();
        T.get_or_init(|| {
            Recorded::parse(TSV).unwrap_or(Recorded {
                exact: HashMap::new(),
                index: HashMap::new(),
            })
        })
    }
}

/// What the answers need from the emulated card.
#[derive(Clone, Copy, Debug)]
pub struct Facts {
    pub fb_bytes: u64,
    pub gpcs: u32,
    pub tpcs_per_gpc: u32,
}

fn ok(params: Vec<u8>, label: &'static str, note: String) -> Answer {
    Answer {
        status: v::NV_OK,
        params,
        label,
        note,
    }
}

/// The answer to `cmd` with request `params`, or `None` when nothing recorded or known fits.
pub fn answer(cmd: u32, params: &[u8], f: &Facts) -> Option<Answer> {
    let t = Recorded::get();
    let n = params.len();
    if let Some(e) = t.exact.get(&(cmd, params.to_vec())) {
        let mut b = e.answer.clone();
        b.resize(n, 0);
        // The FB sizes are the emulated FB's, whatever the card had.
        if cmd == FB_GET_INFO_V2 {
            return Some(list(t, cmd, params, f));
        }
        return Some(Answer {
            status: e.status,
            params: b,
            label: LABEL,
            note: format!("recorded answer, exact request ({})", e.source),
        });
    }
    match cmd {
        FB_GET_INFO_V2 | BUS_GET_INFO_V2 => Some(list(t, cmd, params, f)),
        GRMGR_GET_GR_FS_INFO => Some(gr_fs_info(params, f)),
        GPU_QUERY_FUNCTION_STATUS => Some(ok(
            vec![0; n],
            LABEL_CHIP,
            "gpu query function status: no XUSB or PPC function (statusMask 0)".into(),
        )),
        INTERNAL_GET_ENABLED_SEC2_CLASSES => Some(ok(
            vec![0; n],
            LABEL_CHIP,
            "enabled SEC2 classes: none (the card's class list has no SEC2 class)".into(),
        )),
        _ => None,
    }
}

/// An index-list control: every requested index from the recordings, sizes from `f`.
fn list(t: &Recorded, cmd: u32, params: &[u8], f: &Facts) -> Answer {
    let mut b = params.to_vec();
    let empty = BTreeMap::new();
    let m = t.index.get(&cmd).unwrap_or(&empty);
    let kib = (f.fb_bytes / 1024).min(u32::MAX as u64) as u32;
    let mut unknown = Vec::new();
    for i in 0..list_entries(params) {
        let o = 4 + 8 * i;
        let idx = get32(params, o);
        let data = match cmd {
            FB_GET_INFO_V2 if v::FB_INFO_SIZE_INDICES.contains(&idx) => kib,
            FB_GET_INFO_V2 if idx == FB_LARGEST_FREE_REGION_SIZE_KB => kib,
            FB_GET_INFO_V2 if FB_ZERO_BASES.contains(&idx) => 0,
            BUS_GET_INFO_V2 if BUS_TOPOLOGY.contains(&idx) => 0,
            _ => match m.get(&idx) {
                Some(&d) => d,
                None => {
                    unknown.push(format!("{idx:#x}"));
                    0
                }
            },
        };
        put32(&mut b, o + 4, data);
    }
    let what = if cmd == FB_GET_INFO_V2 { "fb info v2" } else { "bus info v2" };
    ok(
        b,
        LABEL,
        format!(
            "{what}: {} entries from the recordings (sizes: {} MiB FB){}",
            list_entries(params),
            f.fb_bytes >> 20,
            if unknown.is_empty() {
                String::new()
            } else {
                format!("; not recorded, answered 0: [{}]", unknown.join(" "))
            }
        ),
    )
}

/// `GRMGR_GET_GR_FS_INFO` (ctrl2080grmgr.h:257-297): `numQueries` u16 at 0, the queries at
/// 8, 20 bytes each (`queryType` u16, `status` u32 at +4, the 12-byte union at +8; the
/// recorded answer is laid out so: 96 queries in 1928 bytes).
fn gr_fs_info(params: &[u8], f: &Facts) -> Answer {
    const Q: usize = 8;
    const STRIDE: usize = 20;
    let mut b = params.to_vec();
    let count = (u16::from_le_bytes([
        params.first().copied().unwrap_or(0),
        params.get(1).copied().unwrap_or(0),
    ]) as usize)
        .min(params.len().saturating_sub(Q) / STRIDE);
    let gpc_mask = (1u32 << f.gpcs) - 1;
    let tpc_mask = (1u32 << f.tpcs_per_gpc) - 1;
    let mut types = Vec::new();
    for i in 0..count {
        let q = Q + STRIDE * i;
        let ty = u16::from_le_bytes([b[q], b[q + 1]]);
        let d = q + 8;
        let gpc = get32(&b, d);
        let status = match ty {
            1 => {
                put32(&mut b, d, f.gpcs);
                v::NV_OK
            }
            // CHIPLET_GPC_MAP, TPC_MASK, PPC_MASK: {gpcId in, value out}; no floorsweeping.
            2 | 3 | 4 if gpc >= f.gpcs => NV_ERR_INVALID_ARGUMENT,
            2 => {
                put32(&mut b, d + 4, gpc);
                v::NV_OK
            }
            3 => {
                put32(&mut b, d + 4, tpc_mask);
                v::NV_OK
            }
            // Three PES per GPC of six TPCs (GV100/TU10x GPC layout).
            4 => {
                put32(&mut b, d + 4, 0x7);
                v::NV_OK
            }
            // CHIPLET_SYSPIPE_MASK, CHIPLET_GRAPHICS_SYSPIPE_MASK: the one GR.
            6 | 11 => {
                put32(&mut b, d, 1);
                v::NV_OK
            }
            // PARTITION_CHIPLET_SYSPIPE_IDS: {swizzId u16, count u16, ids u8[8]}.
            7 => {
                b[d + 2..d + 4].copy_from_slice(&1u16.to_le_bytes());
                b[d + 4] = 0;
                v::NV_OK
            }
            // PROFILER_MON_GPC_MASK, GFX_CAPABLE_GPC_MASK: {swizzId, grIdx, mask out}.
            8 | 12 => {
                put32(&mut b, d + 8, gpc_mask);
                v::NV_OK
            }
            // PARTITION_SYSPIPE_ID: 0.
            9 => {
                put32(&mut b, d, 0);
                v::NV_OK
            }
            // ROP_MASK (GPC ROPs are GA10x on; Turing's are in the FBPs) and the rest.
            _ => v::NV_ERR_NOT_SUPPORTED,
        };
        put32(&mut b, q + 4, status);
        types.push(format!("{ty}"));
    }
    ok(
        b,
        LABEL_CHIP,
        format!(
            "gr fs info: {count} queries (types {}): {} GPCs, TPC mask {tpc_mask:#x}",
            types.join(","),
            f.gpcs
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: Facts = Facts {
        fb_bytes: 1 << 30,
        gpcs: 3,
        tpcs_per_gpc: 6,
    };

    #[test]
    fn the_embedded_table_parses() {
        let t = Recorded::parse(TSV).unwrap();
        assert!(t.exact.len() >= 15);
        // The card's bus type is PCIe (3) and its RAM type GDDR6 (0x11 recorded).
        assert_eq!(t.index[&BUS_GET_INFO_V2].get(&0), Some(&3));
        assert_eq!(t.index[&FB_GET_INFO_V2].get(&0xD), Some(&0x11));
    }

    /// The class list (0x800292, 804 bytes, all-zero request) is the card's: 95 classes,
    /// among them TURING_A (0xc597) and TURING_DMA_COPY_A (0xc5b5).
    #[test]
    fn classlist_v2_is_the_cards() {
        let a = answer(0x0080_0292, &[0u8; 804], &F).unwrap();
        assert_eq!((a.status, a.params.len(), a.label), (0, 804, LABEL));
        assert_eq!(get32(&a.params, 0), 95);
        let classes: Vec<u32> = (0..95).map(|i| get32(&a.params, 4 + 4 * i)).collect();
        assert!(classes.contains(&0xC597) && classes.contains(&0xC5B5));
        // A request that matches no recording is not answered here.
        assert!(answer(0x0080_0292, &[1u8; 804], &F).is_none());
    }

    #[test]
    fn fb_info_sizes_are_the_emulated_fb() {
        // RAM_SIZE 7, BUS_WIDTH 0xb, RAM_TYPE 0xd, an unrecorded 0x2f, NUMA 0x3b.
        let mut p = vec![0u8; 1028];
        put32(&mut p, 0, 5);
        for (i, idx) in [7u32, 0xB, 0xD, 0x2F, 0x3B].into_iter().enumerate() {
            put32(&mut p, 4 + 8 * i, idx);
            put32(&mut p, 8 + 8 * i, 0xAAAA);
        }
        let a = answer(FB_GET_INFO_V2, &p, &F).unwrap();
        assert_eq!(a.status, 0);
        let data = |i: usize| get32(&a.params, 8 + 8 * i);
        assert_eq!(data(0), 0x10_0000, "1 GiB in KiB");
        assert_eq!(data(1), 0x100);
        assert_eq!(data(2), 0x11);
        assert_eq!(data(3), 0);
        assert_eq!(data(4), 0xFFFF_FFFF);
        assert!(a.note.contains("[0x2f]"), "{}", a.note);
    }

    #[test]
    fn bus_info_is_pcie_without_the_hosts_topology() {
        let mut p = vec![0u8; 420];
        put32(&mut p, 0, 3);
        for (i, idx) in [0u32, 0xF, 0xB].into_iter().enumerate() {
            put32(&mut p, 4 + 8 * i, idx);
        }
        let a = answer(BUS_GET_INFO_V2, &p, &F).unwrap();
        assert_eq!(
            (get32(&a.params, 8), get32(&a.params, 16), get32(&a.params, 24)),
            (3, 0, 5)
        );
    }

    #[test]
    fn gr_fs_info_answers_every_query() {
        let mut p = vec![0u8; 1928];
        p[0] = 5;
        for (i, (ty, gpc)) in [(1u16, 0u32), (2, 2), (3, 1), (3, 3), (10, 0)]
            .into_iter()
            .enumerate()
        {
            let q = 8 + 20 * i;
            p[q..q + 2].copy_from_slice(&ty.to_le_bytes());
            put32(&mut p, q + 8, gpc);
        }
        let a = answer(GRMGR_GET_GR_FS_INFO, &p, &F).unwrap();
        let q = |i: usize, o: usize| get32(&a.params, 8 + 20 * i + o);
        assert_eq!((q(0, 4), q(0, 8)), (0, 3));
        assert_eq!((q(1, 4), q(1, 12)), (0, 2));
        assert_eq!((q(2, 4), q(2, 12)), (0, 0x3F));
        assert_eq!(q(3, 4), NV_ERR_INVALID_ARGUMENT);
        assert_eq!(q(4, 4), v::NV_ERR_NOT_SUPPORTED);
        // The recorded CUDA request (three chiplet GPC maps) gives the card's answer.
        let t = Recorded::get();
        let (req, e) = t
            .exact
            .iter()
            .find(|((c, _), _)| *c == GRMGR_GET_GR_FS_INFO)
            .map(|((_, r), e)| (r.clone(), e))
            .unwrap();
        let a = answer(GRMGR_GET_GR_FS_INFO, &req, &F).unwrap();
        assert_eq!(a.params, e.answer);
        let s = gr_fs_info(&req, &F);
        assert_eq!(s.params, e.answer, "the synthesized answer equals the card's");
    }
}

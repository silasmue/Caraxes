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
pub const FIFO_GET_ENGINE_CONTEXT_PROPERTIES: u32 = 0x0080_1707;
/// GSS-legacy clock commands (no public layout): `{flags u32, count u32, entries[]}`, each
/// entry keyed by its clock domain. The recordings have one domain per call, the Windows
/// KMD asks several in one; [`ENTRY_LISTS`] answers each entry from its recorded twin.
pub const GSS_LEGACY_CLK_64: u32 = 0x2080_9064;
pub const GSS_LEGACY_CLK_04: u32 = 0x2080_9004;

/// An entry-list layout: `count` u32 at 4, entries from 8, `stride` bytes, the key at `key`.
pub struct EntryList {
    pub cmd: u32,
    pub stride: usize,
    pub key: usize,
}

impl EntryList {
    fn offsets(&self, b: &[u8]) -> Vec<usize> {
        let n = (get32(b, 4) as usize).min(b.len().saturating_sub(8) / self.stride);
        (0..n).map(|i| 8 + self.stride * i).collect()
    }
}

/// clk 0x64: 16-byte entries {domain, three words}; clk 0x04: 48-byte entries {0, domain,
/// frequencies} (1544 bytes = 8 + 32 entries).
pub const ENTRY_LISTS: [EntryList; 2] = [
    EntryList { cmd: GSS_LEGACY_CLK_64, stride: 16, key: 0 },
    EntryList { cmd: GSS_LEGACY_CLK_04, stride: 48, key: 4 },
];
/// `GR_GFX_POOL_QUERY_SIZE` (ctrl2080gr.h:1272-1307): the Windows KMD sizes its graphics
/// preemption (GfxP) pool with it. The card refuses it to a user client, so no capture has
/// its numbers: a slot is the card's graphics-preemption buffer (engine 0xb, 0x10f300) in
/// 4 KiB pages, a control structure of 64 bytes plus 16 per slot in 4 KiB pages, 64 KiB
/// pool alignment.
pub const GR_GFX_POOL_QUERY_SIZE: u32 = 0x2080_121F;
const GFXP_SLOT_STRIDE: u32 = 0x11_0000;
/// `GR_GFX_POOL_INITIALIZE`, `_ADD_SLOTS`, `_REMOVE_SLOTS` (ctrl2080gr.h:1310-1400): the pool
/// and its control structure are the KMD's memory; nothing executes on the emulated GR, so
/// the device acknowledges them (NV_OK, request unchanged).
pub const GR_GFX_POOL_MANAGE: [u32; 3] = [0x2080_1220, 0x2080_1221, 0x2080_1222];

/// A FIFO control of the Windows build only (0x20801111, between CONFIG_CTXSW_TIMEOUT and
/// GET_DEVICE_INFO_TABLE; no public header): the KMD sends it with two handles of its class
/// 0xB297 objects and the handle of an 8-page sysmem allocation (40 bytes) for each pair it
/// creates, and unloads GSP-RM when it is refused (Y9). Acknowledged: nothing executes.
pub const FIFO_WINDOWS_1111: u32 = 0x2080_1111;

/// `NV0080_CTRL_CMD_FB_GET_COMPBIT_STORE_INFO`: refused by design on the Linux path (no
/// compression, D16); with `CARAXES_WIN_COMPBIT=1` the card's recorded answer (the KMD-block
/// capture) is given instead, to test whether the Windows KMD's start needs it.
pub const FB_GET_COMPBIT_STORE_INFO: u32 = 0x0080_1306;
pub const WIN_COMPBIT_ENV: &str = "CARAXES_WIN_COMPBIT";

/// `CARAXES_WIN_ACKS=1`: three controls the Linux path refuses on purpose (isolation or
/// host FB addresses) acknowledged for the Windows KMD, which is the kernel and would get
/// NV_OK from GSP-RM: FIFO_CONFIG_CTXSW_TIMEOUT (0x20801110, recorded, nothing applied),
/// BUS_SERVICE_GPU_MULTIFUNC_STATE (0x20801812, the HDA function: nothing to service) and
/// GPU_GET_VPR_CAPS (0x20800160: minStart/maxEnd 0, no VPR in the emulated FB), and
/// SET_RC_RECOVERY (0x2080220D: the KMD sets the robust-channel policy right after creating its
/// channels and unloads when it is refused, Y25; recorded, the device's RC policy stays).
pub const WIN_ACKS_ENV: &str = "CARAXES_WIN_ACKS";
pub const WIN_ACKS: [u32; 4] = [0x2080_1110, 0x2080_1812, 0x2080_0160, 0x2080_220D];

/// Controls whose pure-emulation fallback answers NV_OK with nothing in it: the card's recorded
/// answer (an exact request match) is preferred. GET_CONSTRUCTED_FALCON_INFO: the fallback
/// lists no falcons, so the kernel RM has no NVDEC/NVENC engine for the KMD's video channels.
/// Only in pure emulation and with `CARAXES_PREFER_RECORDED=1` (the Linux emu-boot keeps its
/// fallbacks).
pub const PREFER_RECORDED: [u32; 1] = [0x2080_01B0];
pub const PREFER_RECORDED_ENV: &str = "CARAXES_PREFER_RECORDED";

/// Whether `cmd` is answered here ahead of the dispatcher's tables.
pub fn overrides(cmd: u32) -> bool {
    if PREFER_RECORDED.contains(&cmd) {
        return std::env::var(PREFER_RECORDED_ENV).is_ok_and(|v| v == "1");
    }
    (WIN_ACKS.contains(&cmd) && std::env::var(WIN_ACKS_ENV).is_ok_and(|v| v == "1"))
        || OVERRIDES.contains(&cmd)
        || (cmd == FB_GET_COMPBIT_STORE_INFO
            && std::env::var(WIN_COMPBIT_ENV).is_ok_and(|v| v == "1"))
}

/// Controls answered here before the dispatcher's own tables, which refuse them for the
/// Linux path (no caller there): the Windows KMD's graphics-preemption pool and its FIFO
/// control 0x20801111.
pub const OVERRIDES: [u32; 5] = [
    GR_GFX_POOL_QUERY_SIZE,
    GR_GFX_POOL_MANAGE[0],
    GR_GFX_POOL_MANAGE[1],
    GR_GFX_POOL_MANAGE[2],
    FIFO_WINDOWS_1111,
];
/// A GSS-legacy GPU command (no public layout): the card answered the recorded one (cuInit,
/// subcommand 0x25) NV_OK with the request unchanged.
pub const GSS_LEGACY_GPU_59: u32 = 0x2080_8159;
/// An LPWR control (category 0x28, message 6) absent from every public header; the Windows
/// KMD sends it last before it gives up (4 bytes, zero). Answered NV_OK only with
/// `CARAXES_LPWR_2806_OK=1`, to test whether its refusal is the gate.
pub const LPWR_2806: u32 = 0x2080_2806;
pub const LPWR_2806_ENV: &str = "CARAXES_LPWR_2806_OK";
/// The main GR context size the device reports (no capture has it; generous, the
/// zcull buffer alone is 0x78600 on the card): 2 MiB, 4 KiB aligned.
const GR_MAIN_CTX_SIZE: u32 = 0x20_0000;

const NV_ERR_INVALID_ARGUMENT: u32 = 0x1F;
const NV_ERR_INSUFFICIENT_PERMISSIONS: u32 = 0x1B;

/// FB info indices (ctrl2080fb.h:318-390) the device answers from the emulated FB.
const FB_LARGEST_FREE_REGION_SIZE_KB: u32 = 0x12;
const FB_ZERO_BASES: [u32; 3] = [0x11, 0x13, 0x1E];
/// SUSPEND_RESUME_RSVD_SIZE (bytes, not KiB): the card reserves 512 MiB of its 8 GiB for the
/// FB save/restore; the emulated FB gets the same sixteenth, not the card's absolute size.
const FB_SUSPEND_RESUME_RSVD: u32 = 0x29;
/// PARTITION_COUNT, PARTITION_MASK and LTC_MASK, which no recording has: the card's four
/// FBPs (FBP_COUNT 4, FBP_MASK 0xf recorded) and eight LTCs (LTC_COUNT 8 recorded).
const FB_TOPOLOGY: [(u32, u32); 3] = [(0x04, 4), (0x14, 0xF), (0x2B, 0xFF)];
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
    /// The entry-list controls ([`ENTRY_LISTS`]): cmd → key → the whole recorded entry.
    entries: HashMap<u32, BTreeMap<u32, Vec<u8>>>,
}

/// A hex buffer, or `<len>:<hex>` (the hex, then zeros up to `len` bytes).
fn unhex(s: &str) -> Option<Vec<u8>> {
    if let Some((len, hex)) = s.split_once(':') {
        let mut b = unhex(hex)?;
        let len: usize = len.parse().ok()?;
        (b.len() <= len).then_some(())?;
        b.resize(len, 0);
        return Some(b);
    }
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
            entries: HashMap::new(),
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
            // The KMD-block capture ran as an unprivileged user client: its permission
            // refusals (0x1b) and its answers to internal controls (0x20800Axx/Bxx) are not
            // what the kernel caller gets; the device's own answers stay.
            let device_run = f[4].contains("device run");
            if status == NV_ERR_INSUFFICIENT_PERMISSIONS
                || ((cmd & 0xFFFF_FE00) == 0x2080_0A00 && !device_run)
            {
                continue;
            }
            if matches!(cmd, FB_GET_INFO_V2 | BUS_GET_INFO_V2) && status == 0 {
                let m = r.index.entry(cmd).or_default();
                for i in 0..list_entries(&ans) {
                    m.entry(get32(&ans, 4 + 8 * i))
                        .or_insert(get32(&ans, 8 + 8 * i));
                }
            }
            if let Some(l) = ENTRY_LISTS.iter().find(|l| l.cmd == cmd) {
                if status == 0 {
                    for e in l.offsets(&ans) {
                        r.entries
                            .entry(cmd)
                            .or_default()
                            .entry(get32(&ans, e + l.key))
                            .or_insert_with(|| ans[e..e + l.stride].to_vec());
                    }
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
                entries: HashMap::new(),
            })
        })
    }
}

/// `CARAXES_RECORD_HOST=<file>`: every answer the host RM gave (label `host...`) is appended
/// to `<file>` as a recorded-controls.tsv line (request and answer with zero tails
/// compressed), so a host-RM run yields the card's answers for the pure-emulation table.
pub const RECORD_HOST_ENV: &str = "CARAXES_RECORD_HOST";

fn hex_trim(b: &[u8]) -> String {
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let t = h.trim_end_matches('0');
    let t = if t.len() % 2 == 1 { format!("{t}0") } else { t.to_string() };
    if t.len() == h.len() { h } else { format!("{}:{t}", b.len()) }
}

pub fn record_host(cmd: u32, status: u32, req: &[u8], ans: &[u8]) {
    use std::io::Write;
    let Ok(path) = std::env::var(RECORD_HOST_ENV) else { return };
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            f,
            "{cmd:#x}\t{status:#x}\t{}\t{}\thost RM (device run, CARAXES_RECORD_HOST)",
            hex_trim(req),
            hex_trim(ans)
        );
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
        FIFO_GET_ENGINE_CONTEXT_PROPERTIES => Some(engine_ctx(params)),
        0x2080_0160 => Some(ok(
            vec![0; n],
            LABEL_CHIP,
            "vpr caps: no VPR (minStart/maxEnd 0) in the emulated FB".into(),
        )),
        0x2080_1110 | 0x2080_1812 | 0x2080_220D => Some(ok(
            params.to_vec(),
            LABEL_CHIP,
            format!("{cmd:#x}: acknowledged for the Windows KMD (nothing applied)"),
        )),
        c if ENTRY_LISTS.iter().any(|l| l.cmd == c) => entry_list(t, c, params),
        GR_GFX_POOL_QUERY_SIZE => Some(gfx_pool_query_size(params)),
        FIFO_WINDOWS_1111 => Some(ok(
            params.to_vec(),
            LABEL_CHIP,
            format!(
                "fifo 0x1111 (Windows): acknowledged for objects {:#x}/{:#x}, memory {:#x}",
                get32(params, 0),
                get32(params, 4),
                get32(params, 8)
            ),
        )),
        c if GR_GFX_POOL_MANAGE.contains(&c) => Some(ok(
            params.to_vec(),
            LABEL_CHIP,
            format!("gfx pool {c:#x}: acknowledged (the pool is the KMD's memory; no GR executes)"),
        )),
        GSS_LEGACY_GPU_59 => Some(ok(
            params.to_vec(),
            LABEL,
            "gss legacy gpu 0x59: NV_OK, request unchanged (as the card answered the recorded cuInit one)".into(),
        )),
        LPWR_2806 if std::env::var(LPWR_2806_ENV).is_ok_and(|v| v == "1") => Some(ok(
            vec![0; n],
            LABEL_CHIP,
            format!("lpwr 0x2806: NV_OK, zeros ({LPWR_2806_ENV}=1)"),
        )),
        _ => None,
    }
}

/// The GR context buffer sizes by `NV0080_CTRL_FIFO_GET_ENGINE_CONTEXT_PROPERTIES_ENGINE_ID_*`
/// (ctrl0080fifo.h:120-146), 4 KiB aligned. Only zcull (8) is the card's (recorded); the
/// rest is chosen generously (no capture has them), the same for the public FIFO and GR
/// controls and the kernel's context buffer info. Engines 1-7 are not GR buffers.
pub fn gr_ctx_size(id: u32) -> Option<u32> {
    if let Some((_, sz)) = recorded_ctx(id) {
        return sz;
    }
    Some(match id {
        0 => GR_MAIN_CTX_SIZE,
        8 => 0x7_8600,
        9 => 0x2_0000,
        0xA | 0xB | 0x13 | 0x15 => GFXP_SLOT_STRIDE,
        0xC | 0xF | 0x10 | 0x14 | 0x17 | 0x18 | 0x19 => 0x1_0000,
        0xD | 0xE | 0x11 | 0x12 => 0x8_0000,
        0x16 => 0x1000,
        _ => return None,
    })
}

/// The card's own FIFO_GET_ENGINE_CONTEXT_PROPERTIES answer for `id` (the KMD-block capture
/// has engines 0-0x10): `Some((alignment, Some(size)))`, or `Some((0, None))` where the card
/// refused it.
fn recorded_ctx(id: u32) -> Option<(u32, Option<u32>)> {
    let mut req = vec![0u8; 12];
    put32(&mut req, 0, id);
    let e = Recorded::get().exact.get(&(FIFO_GET_ENGINE_CONTEXT_PROPERTIES, req))?;
    Some(if e.status == 0 {
        (get32(&e.answer, 4), Some(get32(&e.answer, 8)))
    } else {
        (0, None)
    })
}

fn gr_ctx_align(id: u32) -> u32 {
    match recorded_ctx(id) {
        Some((a, Some(_))) if a != 0 => a,
        _ => 0x1000,
    }
}

/// `FIFO_GET_ENGINE_CONTEXT_PROPERTIES` ({engineId, alignment, size}, ctrl0080fifo.h:92-135)
/// for an engine the recordings do not have: [`gr_ctx_size`]; any other engine
/// INVALID_ARGUMENT, one of the documented statuses.
fn engine_ctx(params: &[u8]) -> Answer {
    let mut b = params.to_vec();
    b.resize(params.len().max(12), 0);
    let id = get32(&b, 0) & 0x1F;
    let size = gr_ctx_size(id);
    let st = match size {
        Some(sz) => {
            put32(&mut b, 4, gr_ctx_align(id));
            put32(&mut b, 8, sz);
            v::NV_OK
        }
        None => NV_ERR_INVALID_ARGUMENT,
    };
    b.truncate(params.len());
    Answer {
        status: st,
        params: b,
        label: LABEL_CHIP,
        note: format!("engine context properties: engine {id:#x}"),
    }
}

/// An entry-list control, each entry from the recorded one of its key; `None` if a key was
/// never recorded.
fn entry_list(t: &Recorded, cmd: u32, params: &[u8]) -> Option<Answer> {
    let l = ENTRY_LISTS.iter().find(|l| l.cmd == cmd)?;
    let m = t.entries.get(&cmd)?;
    let mut b = params.to_vec();
    let mut keys = Vec::new();
    for e in l.offsets(params) {
        let k = get32(params, e + l.key);
        b[e..e + l.stride].copy_from_slice(m.get(&k)?);
        keys.push(format!("{k:#x}"));
    }
    Some(ok(
        b,
        LABEL,
        format!("entry list {cmd:#x}: keys [{}] from the recordings", keys.join(" ")),
    ))
}

/// `{maxSlots u32, slotStride u32, ctrlStructSize u64, ctrlStructAlign u64, poolSize u64,
/// poolAlign u64}`; `maxSlots` must be non-zero.
fn gfx_pool_query_size(params: &[u8]) -> Answer {
    let mut b = params.to_vec();
    b.resize(params.len().max(40), 0);
    let slots = get32(&b, 0);
    let st = if slots == 0 {
        NV_ERR_INVALID_ARGUMENT
    } else {
        let ctrl = (64 + 16 * slots as u64).div_ceil(0x1000) * 0x1000;
        put32(&mut b, 4, GFXP_SLOT_STRIDE);
        b[8..16].copy_from_slice(&ctrl.to_le_bytes());
        b[16..24].copy_from_slice(&0x1000u64.to_le_bytes());
        b[24..32].copy_from_slice(&(slots as u64 * GFXP_SLOT_STRIDE as u64).to_le_bytes());
        b[32..40].copy_from_slice(&0x1_0000u64.to_le_bytes());
        v::NV_OK
    };
    b.truncate(params.len());
    Answer {
        status: st,
        params: b,
        label: LABEL_CHIP,
        note: format!("gfx pool query size: maxSlots {slots}, slot stride {GFXP_SLOT_STRIDE:#x}"),
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
            FB_GET_INFO_V2 if idx == FB_SUSPEND_RESUME_RSVD => (f.fb_bytes / 16).min(u32::MAX as u64) as u32,
            FB_GET_INFO_V2 if FB_TOPOLOGY.iter().any(|t| t.0 == idx) => {
                FB_TOPOLOGY.iter().find(|t| t.0 == idx).unwrap().1
            }
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
    let t = Recorded::get();
    for i in 0..count {
        let q = Q + STRIDE * i;
        let ty = u16::from_le_bytes([b[q], b[q + 1]]);
        let d = q + 8;
        let gpc = get32(&b, d);
        // The card's answer to this query alone (the KMD-block capture asks each type and
        // GPC in a one-query request), else the chip facts below.
        let mut one = vec![0u8; params.len()];
        one[0] = 1;
        one[Q..Q + STRIDE].copy_from_slice(&params[q..q + STRIDE]);
        if let Some(e) = t.exact.get(&(GRMGR_GET_GR_FS_INFO, one)) {
            if e.status == 0 && e.answer.len() >= Q + STRIDE {
                b[q..q + STRIDE].copy_from_slice(&e.answer[Q..Q + STRIDE]);
                types.push(format!("{ty}*"));
                continue;
            }
        }
        let status = match ty {
            1 => {
                put32(&mut b, d, f.gpcs);
                v::NV_OK
            }
            // CHIPLET_GPC_MAP, TPC_MASK, PPC_MASK: {gpcId in, value out}; no floorsweeping.
            2..=4 if gpc >= f.gpcs => NV_ERR_INVALID_ARGUMENT,
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
            "gr fs info: {count} queries (types {}; * the card's): {} GPCs, TPC mask {tpc_mask:#x}",
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
    fn host_answers_are_recorded_in_the_table_format() {
        assert_eq!(hex_trim(&[1, 2, 0, 0]), "4:0102");
        assert_eq!(hex_trim(&[1, 2]), "0102");
        assert_eq!(unhex(&hex_trim(&[0x10, 0, 0])).unwrap(), vec![0x10, 0, 0]);
    }

    #[test]
    fn the_embedded_table_parses() {
        let t = Recorded::parse(TSV).unwrap();
        assert!(t.exact.len() >= 15);
        // The card's bus type is PCIe (3) and its RAM type GDDR6 (0x11 recorded).
        assert_eq!(t.index[&BUS_GET_INFO_V2].get(&0), Some(&3));
        assert_eq!(t.index[&FB_GET_INFO_V2].get(&0xD), Some(&0x11));
    }

    /// The class list (0x800292, 804 bytes, all-zero request) is the card's (host RM 615: 96
    /// classes; the 610 traces have 95),
    /// among them TURING_A (0xc597) and TURING_DMA_COPY_A (0xc5b5).
    #[test]
    fn classlist_v2_is_the_cards() {
        let a = answer(0x0080_0292, &[0u8; 804], &F).unwrap();
        assert_eq!((a.status, a.params.len(), a.label), (0, 804, LABEL));
        assert_eq!(get32(&a.params, 0), 96);
        let classes: Vec<u32> = (0..96).map(|i| get32(&a.params, 4 + 4 * i)).collect();
        assert!(classes.contains(&0xC597) && classes.contains(&0xC5B5));
        // A request that matches no recording is not answered here.
        assert!(answer(0x0080_0292, &[1u8; 804], &F).is_none());
    }

    /// The Windows acknowledgements: only with CARAXES_WIN_ACKS=1 ahead of the tables; the
    /// VPR caps answer is an empty range.
    #[test]
    fn windows_acks() {
        assert_eq!(overrides(0x2080_0160), std::env::var(WIN_ACKS_ENV).is_ok_and(|v| v == "1"));
        let a = answer(0x2080_0160, &[0xAAu8; 16], &F).unwrap();
        assert_eq!((a.status, a.params), (0, vec![0u8; 16]));
        let a = answer(0x2080_1110, &[7u8; 8], &F).unwrap();
        assert_eq!((a.status, a.params), (0, vec![7u8; 8]));
    }

    /// The capture's user-client refusals do not replace the device's answers.
    #[test]
    fn permission_refusals_are_not_the_cards_answer() {
        let mut q = vec![0u8; 40];
        q[0] = 4;
        let a = answer(GR_GFX_POOL_QUERY_SIZE, &q, &F).unwrap();
        assert_eq!((a.status, a.label), (0, LABEL_CHIP));
        let a = answer(INTERNAL_GET_ENABLED_SEC2_CLASSES, &[0u8; 3], &F).unwrap();
        assert_eq!((a.status, a.label), (0, LABEL_CHIP));
    }

    #[test]
    fn engine_context_properties() {
        // The recorded zcull request (engine 8) is the card's answer.
        let mut p = vec![0u8; 12];
        p[0] = 8;
        let a = answer(FIFO_GET_ENGINE_CONTEXT_PROPERTIES, &p, &F).unwrap();
        assert_eq!((a.status, get32(&a.params, 4), get32(&a.params, 8)), (0, 0x1000, 0x78600));
        // The main GR context (what the Windows KMD asks).
        let a = answer(FIFO_GET_ENGINE_CONTEXT_PROPERTIES, &[0u8; 12], &F).unwrap();
        assert_eq!((a.status, get32(&a.params, 4), get32(&a.params, 8)), (0, 0x1000, 0xA_D400));
        p[0] = 0xB;
        let a = answer(FIFO_GET_ENGINE_CONTEXT_PROPERTIES, &p, &F).unwrap();
        assert_eq!((a.status, get32(&a.params, 8)), (0, 0x10_F300));
        p[0] = 3;
        let a = answer(FIFO_GET_ENGINE_CONTEXT_PROPERTIES, &p, &F).unwrap();
        assert!(gr_ctx_size(0x1A).is_none());
        assert_eq!(a.status, v::NV_ERR_NOT_SUPPORTED, "the card refuses engine 3");
        p[0] = 0x11;
        let a = answer(FIFO_GET_ENGINE_CONTEXT_PROPERTIES, &p, &F).unwrap();
        assert_eq!((a.status, get32(&a.params, 4), get32(&a.params, 8)), (0, 0x100, 0x3000), "the card's bundle CB");
        let mut q = vec![0u8; 40];
        q[0] = 8;
        let a = answer(GR_GFX_POOL_QUERY_SIZE, &q, &F).unwrap();
        assert_eq!(a.status, 0);
        assert_eq!(get32(&a.params, 4), GFXP_SLOT_STRIDE);
        assert_eq!(u64::from_le_bytes(a.params[24..32].try_into().unwrap()), 8 * GFXP_SLOT_STRIDE as u64);
        assert_eq!(answer(GR_GFX_POOL_QUERY_SIZE, &[0u8; 40], &F).unwrap().status, NV_ERR_INVALID_ARGUMENT);
        let mut g = vec![0u8; 332];
        g[0] = 1;
        g[4] = 0xA;
        let a = answer(GSS_LEGACY_GPU_59, &g, &F).unwrap();
        assert_eq!((a.status, a.params), (0, g));
        if std::env::var(LPWR_2806_ENV).is_err() {
            assert!(answer(LPWR_2806, &[0u8; 4], &F).is_none());
        }
    }

    /// The Windows KMD's three-domain request (Y0 trace) from the one-domain recordings.
    #[test]
    fn gss_clk64_combines_the_recorded_domains() {
        let mut p = vec![0u8; 520];
        put32(&mut p, 4, 3);
        for (i, d) in [1u32, 4, 8].into_iter().enumerate() {
            put32(&mut p, 8 + 16 * i, d);
        }
        let a = answer(GSS_LEGACY_CLK_64, &p, &F).unwrap();
        assert_eq!(a.status, 0);
        let w = |i: usize, k: usize| get32(&a.params, 8 + 16 * i + 4 + 4 * k);
        assert_eq!([w(0, 0), w(1, 0), w(2, 0)], [1, 0x10, 0x10_0000]);
        assert_eq!([w(0, 1), w(0, 2)], [1, 0x64]);
        put32(&mut p, 8, 2);
        assert!(answer(GSS_LEGACY_CLK_64, &p, &F).is_none());
        // clk 0x04: 48-byte entries keyed by the domain at +4.
        let mut p = vec![0u8; 1544];
        put32(&mut p, 4, 2);
        put32(&mut p, 12, 0x10);
        put32(&mut p, 60, 1);
        let a = answer(GSS_LEGACY_CLK_04, &p, &F).unwrap();
        // The frequencies are the card's at recording time (the first recording wins).
        assert_eq!(get32(&a.params, 12), 0x10);
        assert_ne!(get32(&a.params, 16), 0);
        assert_eq!(get32(&a.params, 60), 1);
        assert_ne!(get32(&a.params, 64), 0);
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
        assert_eq!(data(3), 1, "FORCED_BAR1_64KB_MAPPING_ENABLED, the card's");
        assert_eq!(data(4), 0xFFFF_FFFF);
        assert!(!a.note.contains("not recorded"), "{}", a.note);
        put32(&mut p, 0, 1);
        put32(&mut p, 4, FB_SUSPEND_RESUME_RSVD);
        let a = answer(FB_GET_INFO_V2, &p, &F).unwrap();
        assert_eq!(get32(&a.params, 8), 64 << 20, "a sixteenth of 1 GiB");
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
            .find(|((c, r), _)| *c == GRMGR_GET_GR_FS_INFO && r[0] == 3)
            .map(|((_, r), e)| (r.clone(), e))
            .unwrap();
        let a = answer(GRMGR_GET_GR_FS_INFO, &req, &F).unwrap();
        assert_eq!(a.params, e.answer);
        let s = gr_fs_info(&req, &F);
        assert_eq!(s.params, e.answer, "the synthesized answer equals the card's");
    }
}

/// `CARAXES_EMU_GR=1`: in pure emulation the kernel's GR static info (kgr.rs) comes from
/// [`EmuGrHost`] instead of the empty-GPU fallbacks (gpcMask 0). A kernel RM that allocates a
/// GR object needs `numGpcs > 0` (`_kgrAlloc`, kernel_graphics_object.c:192: otherwise
/// NV_ERR_INSUFFICIENT_RESOURCES, which is the Windows adapter start's
/// STATUS_INSUFFICIENT_RESOURCES). Off by default: with GPCs the Linux RM builds its golden
/// context channel at load, which pure emulation does not execute.
pub const EMU_GR_ENV: &str = "CARAXES_EMU_GR";

pub fn emu_gr() -> bool {
    std::env::var(EMU_GR_ENV).is_ok_and(|v| v == "1")
}

pub const LABEL_EMU_GR: &str =
    "card+chip (pure emulation: kgr from the recorded GR controls and TU106 facts, recorded.rs EmuGrHost)";

/// A stand-in for the host RM that kgr.rs asks for the public GR controls: the card's
/// recorded answers (GPC and TPC masks, global SM order, caps), the chip facts for the rest
/// (TPCs per GPC, zcull mask 0xf, three PES per GPC, the graphics GPC/TPC info), and
/// [`gr_ctx_size`] for the context buffers. Anything else is NOT_SUPPORTED, which kgr.rs
/// treats as it treats a host that lacks the control.
pub struct EmuGrHost {
    pub facts: Facts,
}

impl crate::ctrl::HostRm for EmuGrHost {
    fn control(&mut self, _class: u32, cmd: u32, params: &mut [u8]) -> u32 {
        use crate::ver::active as v;
        let t = Recorded::get();
        if let Some(e) = t.exact.get(&(cmd, params.to_vec())) {
            let n = params.len().min(e.answer.len());
            params[..n].copy_from_slice(&e.answer[..n]);
            return e.status;
        }
        let f = &self.facts;
        let gpc = get32(params, 0);
        match cmd {
            v::CMD_GR_GET_NUM_TPCS_FOR_GPC if gpc < f.gpcs => put32(params, 4, f.tpcs_per_gpc),
            v::CMD_GR_GET_ZCULL_MASK if gpc < f.gpcs => put32(params, 4, 0xF),
            v::CMD_GPU_GET_PES_INFO if gpc < f.gpcs => {
                let hl = crate::hostabi::current();
                put32(params, hl.pes_info_num_pes, 3);
                // Two TPCs per PES (the host capture: numPesInGpc 3, activePesMask 0x7).
                for t in 0..(f.tpcs_per_gpc as usize).min(hl.pes_info_map_len) {
                    put32(params, hl.pes_info_tpc_to_pes_map + 4 * t, (t / 2) as u32);
                }
            }
            v::CMD_GR_GET_GFX_GPC_AND_TPC_INFO => {
                put32(params, v::GR_ROUTED_ARG, (1 << f.gpcs) - 1);
                put32(params, v::GR_ROUTED_OUT, f.gpcs * f.tpcs_per_gpc);
            }
            v::CMD_GR_GET_ENGINE_CONTEXT_PROPERTIES => {
                match gr_ctx_size(get32(params, v::PUB_CTX_PROPS_ENGINE_ID)) {
                    Some(sz) => {
                        put32(params, v::PUB_CTX_PROPS_SIZE, sz);
                        put32(params, v::PUB_CTX_PROPS_ALIGNMENT, gr_ctx_align(get32(params, v::PUB_CTX_PROPS_ENGINE_ID)));
                    }
                    None => return v::NV_ERR_NOT_SUPPORTED,
                }
            }
            _ => return v::NV_ERR_NOT_SUPPORTED,
        }
        v::NV_OK
    }
}

#[cfg(test)]
mod emu_gr_tests {
    use super::*;
    use crate::ver::active as v;

    /// kgr.rs through the stand-in: 3 GPCs of 6 TPCs, 36 SMs in the card's order, context
    /// buffer sizes, and FECS_EVENT not NV_U32_MAX.
    #[test]
    fn kgr_static_info_is_a_whole_tu106() {
        let chip = crate::fixture::rtx2070().chip.data();
        let facts = Facts { fb_bytes: 1 << 30, gpcs: 3, tpcs_per_gpc: 6 };
        let mut h: Box<dyn crate::ctrl::HostRm> = Box::new(EmuGrHost { facts });
        let a = crate::kgr::answer(chip, v::CMD_INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS, &vec![0; v::FS_PARAMS], Some(&mut h)).unwrap();
        assert_eq!(a.status, 0);
        assert_eq!(get32(&a.params, v::FS_GPC_MASK), 7, "{}", a.note);
        assert_eq!(get32(&a.params, v::FS_TPC_MASK + 8), 0x3F);
        assert_eq!(get32(&a.params, v::FS_TPC_COUNT + 4), 6);
        assert_eq!(get32(&a.params, v::FS_NUM_PES_PER_GPC), 3);
        assert_eq!(get32(&a.params, v::FS_NUM_GFX_TPC), 18);
        let a = crate::kgr::answer(chip, v::CMD_INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER, &vec![0; v::SM_ORDER_PARAMS], Some(&mut h)).unwrap();
        assert!(a.note.contains("36 SMs"), "{}", a.note);
        let a = crate::kgr::answer(chip, v::CMD_INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO, &vec![0; v::CTX_PARAMS], Some(&mut h)).unwrap();
        assert_eq!(get32(&a.params, 0), 0xA_D400, "the card's main context, {}", a.note);
        assert_eq!(get32(&a.params, 8 * v::CTX_ENGINE_FECS_EVENT), 0x1_0000);
    }
}

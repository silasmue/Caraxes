// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Real compression per VM slice, the guest's side (operator's decision of 2026-09-28,
//! replacing the "no compression" of D16; docs/research/compression-guest-view-2026-09-28.md).
//! With `CARAXES_COMPRESSION=slice` every answer the guest driver decides compression by is
//! a real TU106's with compression; without it (the default) every answer stays as before,
//! byte for byte. Paths are relative to open-gpu-kernel-modules 615.71.09, the RM sources
//! to `src/nvidia/src/kernel/`.
//!
//! The card runs 1:1 comptag lines ([`ComprData`], chips/tu106/chip.toml `[compression]`):
//! the line of a 64 KiB page is `1 + SPA[35:16]` (`memmgrDetermineComptag_TU102`,
//! gpu/mem_mgr/arch/turing/mem_mgr_tu102.c:505-512), so a page's line follows from its
//! physical address and a VM's lines are those of its own slice. The guest computes the
//! same from its FB addresses for every compressible PTE it writes
//! (`kgmmuFieldSetKindCompTags`, gpu/mmu/kern_gmmu.c:974-991; `ctagId` -1 makes the
//! surface "physically based", gpu/mem_mgr/mem_mgr.c:2601-2614) and for UVM's external
//! mappings (rmapi/nv_gpu_ops.c:4349-4389). Those guest PTEs never reach the GPU: the
//! shadow tables are rebuilt by the host RM, which writes the lines of the host pages
//! itself. On the identity-addressed layout (guest FB address = host VRAM address,
//! docs/caraxes/memory-model.md §1) the two agree line for line.
//!
//! The answers (the whole decision chain is in the research note):
//! * `INTERNAL_MEMSYS_GET_STATIC_CONFIG` ([`memsys_static_config`]):
//!   `bOneToOneComptagLineAllocation` 1 and `bDisableCompbitBacking` 0. Without one of
//!   1:1 or raw mode the guest refuses every compressible allocation
//!   (`kmemsysAllocComprResources_KERNEL`, gpu/mem_sys/kern_mem_sys.c:473-474), and with
//!   the backing disabled it never asks (gpu/mem_mgr/arch/maxwell/mem_mgr_gm107.c:428-433).
//! * The FB region the guest's heap lives in says `supportCompressed` (rpc.rs
//!   `build_static_info`); otherwise every compressible PTE is written with the
//!   uncompressed kind at map time (virt_mem_allocator_gm107.c:1866-1881, 1931-1943).
//! * `FB_GET_INFO_V2` `COMPRESSION_SIZE` ([`finish`]): the comptag lines of this VM's FB in
//!   bytes, as the card derives its own; `1TO1_COMPTAG_ENABLED` needs no answer, the guest
//!   reads it from the static config (gpu/mem_sys/kern_mem_sys_ctrl.c:660-665).
//! * `FB_GET_COMPBIT_STORE_INFO` ([`answer`]): the card's policy and cache geometry, the
//!   comptag lines of this VM, and no backing-store location: the store is the host's and
//!   no host address reaches the guest.
//! * `FB_GET_CTAGS_FOR_CBC_EVICTION`: no lines; `FB_GET_COMPBITCOPY_CONSTRUCT_INFO`:
//!   refused (the compbits live in the host's store).
//!
//! From the host side (workstream A, hostmem.rs) the device needs one fact: that the
//! reservation is compressible backing. It reaches this module as
//! [`FbLayout::compressible`] (`FbLayout::with_compression`, set by main.rs).

use serde::Deserialize;

use crate::chip::ChipData;
use crate::ctrl::Answer;
use crate::fb::FbLayout;
use crate::gmcapi::RegValue;
use crate::ver::common::{NV_ERR_NOT_SUPPORTED, NV_OK};
use crate::ver::Abi;

/// The switch: `off` (default) or `slice`.
pub const ENV: &str = "CARAXES_COMPRESSION";

/// `NV0080_CTRL_CMD_FB_GET_COMPBIT_STORE_INFO` (ctrl0080fb.h:148).
pub const FB_GET_COMPBIT_STORE_INFO: u32 = 0x0080_1306;
/// `NV2080_CTRL_CMD_FB_GET_CTAGS_FOR_CBC_EVICTION` (ctrl2080fb.h:1548).
pub const FB_GET_CTAGS_FOR_CBC_EVICTION: u32 = 0x2080_1338;
/// `NV2080_CTRL_CMD_FB_GET_COMPBITCOPY_CONSTRUCT_INFO` (ctrl2080fb.h:1712-1772).
pub const FB_GET_COMPBITCOPY_CONSTRUCT_INFO: u32 = 0x2080_133D;
/// `NV2080_CTRL_CMD_FB_GET_INFO_V2` and its `COMPRESSION_SIZE` index (ctrl2080fb.h:53-56,
/// 319): "the amount of compression (in bytes) ... 0 indicates the GPU does not support
/// compression".
pub const FB_GET_INFO_V2: u32 = 0x2080_1303;
pub const FB_INFO_INDEX_COMPRESSION_SIZE: u32 = 0x01;
/// `NV_MMU_VER2_PTE_COMPTAGLINE` (20+35):36 (turing/tu102/dev_mmu.h:117): 20 bits.
pub const PTE_COMPTAGLINE_BITS: u32 = 20;

const MEMSYS: &str = "NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS";
const STORE: &str = "NV0080_CTRL_FB_GET_COMPBIT_STORE_INFO_PARAMS";

/// D9 labels.
pub const LABEL_STORE: &str = "card+guest (compression slice: the card's 1:1 policy and cache geometry, MaxCompbitLine of this VM's FB; the backing store is the host's, no address)";
pub const LABEL_CTAGS: &str =
    "guest (compression slice, 1:1: comptag lines follow the page address; none reserved for CBC eviction)";
pub const LABEL_COMPBITCOPY: &str =
    "refused (compression slice: the compbits live in the host's backing store; no compbit copy)";

/// `CARAXES_COMPRESSION`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// D16: the guest is told there is no compression.
    #[default]
    Off,
    /// Real compression, each VM's slice with its own comptag lines.
    Slice,
}

impl Mode {
    /// Unset, empty, `off` or `0` is [`Mode::Off`]; anything but `slice` is refused.
    pub fn parse(value: Option<&str>) -> Result<Mode, String> {
        match value {
            None | Some("" | "off" | "0") => Ok(Mode::Off),
            Some("slice") => Ok(Mode::Slice),
            Some(x) => Err(format!("{ENV}={x:?}: expected \"off\" or \"slice\"")),
        }
    }

    pub fn from_env() -> Result<Mode, String> {
        Mode::parse(std::env::var(ENV).ok().as_deref())
    }
}

/// The comptag line allocation policy of a card
/// (`NV0080_CTRL_CMD_FB_GET_COMPBIT_STORE_INFO_POLICY_*`, ctrl0080fb.h:174-203).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Policy {
    /// `POLICY_1TO1`: one line per 64 KiB page, computed from the physical address.
    #[serde(rename = "1to1")]
    OneToOne,
}

impl Policy {
    /// The `comptaglineAllocationPolicy` value.
    pub fn value(self) -> u32 {
        match self {
            Policy::OneToOne => 1,
        }
    }
}

/// `[compression]` of chip.toml: the card's compression facts.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComprData {
    /// The short name of the chip the facts were read on; a costume presenting another
    /// chip inherits the table (chip/derived.rs), not the facts ([`GuestView::of`]).
    pub chip: String,
    pub policy: Policy,
    /// `[hi, lo]`: the line of a page is `1 + SPA[hi:lo]`.
    pub comptagline_from_spa: [u32; 2],
    /// `bDisablePostL2Compression` (PLC off on the whole card).
    pub disable_post_l2_compression: bool,
    /// The cache geometry of `FB_GET_COMPBIT_STORE_INFO`, as the card answered it.
    pub comptags_per_cache_line: u32,
    pub cache_line_size: u32,
    pub cache_line_size_per_slice: u32,
    pub cache_line_fetch_alignment: u32,
    pub gobs_per_comptag_per_slice: u32,
    pub cbc_coverage_per_slice: u32,
    pub label: String,
}

/// What this VM's guest is told about compression: the chip's facts over the guest's
/// compressible FB. `None` ([`GuestView::of`]) is D16, today's answers.
#[derive(Clone, Copy, Debug)]
pub struct GuestView {
    pub facts: &'static ComprData,
    /// The guest's compressible FB `[lo, hi)` in guest coordinates: the usable region,
    /// which is where its heap lives (rpc.rs region 0). The carve-out and the BAR1 pool
    /// above it are reserved and stay uncompressed.
    pub fb: (u64, u64),
}

impl GuestView {
    /// The view of a layout the host holds as compressible backing on a chip whose card
    /// was read; `None` otherwise.
    pub fn of(layout: &FbLayout, chip: &'static ChipData) -> Option<GuestView> {
        if !layout.compressible {
            return None;
        }
        let facts = chip
            .compression
            .as_ref()
            .filter(|c| c.chip == chip.short_name)?;
        let fb = layout.usable()?;
        (fb.0 < fb.1).then_some(GuestView { facts, fb })
    }

    /// The comptag line the guest writes into a compressible PTE of FB address `pa`
    /// (`memmgrDetermineComptag_TU102`, mem_mgr_tu102.c:505-512).
    pub fn comptag_line(&self, pa: u64) -> u32 {
        let [hi, lo] = self.facts.comptagline_from_spa;
        1 + ((pa >> lo) & ((1u64 << (hi - lo + 1)) - 1)) as u32
    }

    /// The first and the last comptag line of the guest's compressible FB.
    pub fn lines(&self) -> (u32, u32) {
        (self.comptag_line(self.fb.0), self.comptag_line(self.fb.1 - 1))
    }

    /// `MaxCompbitLine`: the highest line a guest page carries. The card's 0x1f7d0 is the
    /// line of its compressible top, 0x1_f7d0_0000 (recorded-controls.tsv:76).
    pub fn max_compbit_line(&self) -> u32 {
        self.lines().1
    }

    /// `COMPRESSION_SIZE`: `MaxCompbitLine` pages in bytes, cut to 32 bits as the card cuts
    /// its own (0x1f7d0 lines: 0xf7d0_0000, recorded-controls.tsv:10).
    pub fn compression_size(&self) -> u32 {
        (u64::from(self.max_compbit_line()) << self.facts.comptagline_from_spa[1]) as u32
    }

    pub fn describe(&self) -> String {
        let (first, last) = self.lines();
        format!(
            "compression slice: FB [{:#x}, {:#x}) compressible, comptag lines {first:#x}..={last:#x} (1:1, 1 + SPA[{}:{}])",
            self.fb.0, self.fb.1, self.facts.comptagline_from_spa[0], self.facts.comptagline_from_spa[1]
        )
    }
}

/// Offset of `path` in the guest ABI's generated layout of `st`
/// (src/ver/v6xx/ctrl_layouts.rs).
fn off(st: &str, path: &str) -> usize {
    Abi::active()
        .layout(st)
        .unwrap_or_else(|| panic!("{st}: no generated layout"))
        .resolve(path)
        .unwrap_or_else(|e| panic!("{e}"))
        .0
}

fn size(st: &str) -> usize {
    Abi::active()
        .layout(st)
        .unwrap_or_else(|| panic!("{st}: no generated layout"))
        .size
}

fn put(b: &mut [u8], o: usize, v: u64, n: usize) {
    if let Some(s) = b.get_mut(o..o + n) {
        s.copy_from_slice(&v.to_le_bytes()[..n]);
    }
}

fn get32(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4)
        .map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

/// `INTERNAL_MEMSYS_GET_STATIC_CONFIG` (ctrl2080internal.h:96-137) over the chip's answer
/// `b`: the comptag policy flags of the card and the backing on. The guest caches the
/// struct once (`kmemsysInitStaticConfig_KERNEL`, kern_mem_sys.c:517-529).
pub fn memsys_static_config(b: &mut [u8], view: &GuestView) {
    let f = view.facts;
    for (field, value) in [
        ("bOneToOneComptagLineAllocation", f.policy == Policy::OneToOne),
        ("bUseOneToFourComptagLineAllocation", false),
        ("bUseRawModeComptaglineAllocation", false),
        ("bDisableCompbitBacking", false),
        ("bDisablePostL2Compression", f.disable_post_l2_compression),
    ] {
        put(b, off(MEMSYS, field), value as u64, 1);
    }
}

/// The compbit-store controls of a VM with compression, ahead of the chip table
/// (chips/tu106/controls.toml refuses them for D16) and of the recorded
/// `CARAXES_WIN_COMPBIT` answer, which carries the host's store addresses.
pub fn answer(view: &GuestView, cmd: u32, params: &[u8]) -> Option<Answer> {
    let n = params.len();
    match cmd {
        FB_GET_COMPBIT_STORE_INFO => {
            let f = view.facts;
            let mut b = vec![0u8; n.max(size(STORE))];
            // Size, Address, AddressSpace (0 = UNKNOWN, ctrl0080fb.h:170), backingStoreBase,
            // backingStoreCbcBase, privRegionStartOffset and cbcRegionSize stay 0: the
            // store is in the host's FB, outside every reservation. The guest kernel reads
            // the store only to save it over a suspend, which TU106 never asks for
            // (bPreserveComptagBackingStoreOnSuspend is FALSE on every chip,
            // generated/g_kern_mem_sys_nvoc.c:289-290; gpu/mem_mgr/mem_mgr_pwr_mgmt.c:365-380).
            for (field, value) in [
                ("MaxCompbitLine", view.max_compbit_line()),
                ("comptagsPerCacheLine", f.comptags_per_cache_line),
                ("cacheLineSize", f.cache_line_size),
                ("cacheLineSizePerSlice", f.cache_line_size_per_slice),
                ("cacheLineFetchAlignment", f.cache_line_fetch_alignment),
                ("gobsPerComptagPerSlice", f.gobs_per_comptag_per_slice),
                ("comptaglineAllocationPolicy", f.policy.value()),
                ("cbcCoveragePerSlice", f.cbc_coverage_per_slice),
            ] {
                put(&mut b, off(STORE, field), value.into(), 4);
            }
            b.truncate(n);
            let (first, last) = view.lines();
            Some(Answer {
                status: NV_OK,
                params: b,
                label: LABEL_STORE,
                note: format!(
                    "compbit store info: policy 1TO1, MaxCompbitLine {:#x} (this VM's lines {first:#x}..={last:#x}), no backing-store address",
                    view.max_compbit_line()
                ),
            })
        }
        FB_GET_CTAGS_FOR_CBC_EVICTION => {
            // `pCompTags[127]`, `numCompTags` (ctrl2080fb.h:1559-1562): no entries.
            let mut b = vec![0u8; n];
            put(&mut b, crate::ver::common::public::CTAGS_NUM, 0, 4);
            Some(Answer {
                status: NV_OK,
                params: b,
                label: LABEL_CTAGS,
                note: "ctags for cbc eviction: none (numCompTags 0)".into(),
            })
        }
        FB_GET_COMPBITCOPY_CONSTRUCT_INFO => Some(Answer {
            status: NV_ERR_NOT_SUPPORTED,
            params: params.to_vec(),
            label: LABEL_COMPBITCOPY,
            note: "compbit copy construct info: refused".into(),
        }),
        _ => None,
    }
}

/// The answers the device forwards or replays, finished for a VM with compression:
/// `FB_GET_INFO_V2` `COMPRESSION_SIZE` is this VM's, not the host card's (nvml.rs forwards
/// the index list, recorded.rs replays it in pure emulation).
pub fn finish(view: &GuestView, cmd: u32, a: &mut Answer) {
    if cmd != FB_GET_INFO_V2 || a.status != NV_OK {
        return;
    }
    let count = (get32(&a.params, 0) as usize).min(a.params.len().saturating_sub(4) / 8);
    let mut done = false;
    for i in 0..count {
        let o = 4 + 8 * i;
        if get32(&a.params, o) == FB_INFO_INDEX_COMPRESSION_SIZE {
            put(&mut a.params, o + 4, view.compression_size().into(), 4);
            done = true;
        }
    }
    if done {
        a.note.push_str(&format!(
            "; COMPRESSION_SIZE {:#x} (compression slice: comptag lines up to {:#x})",
            view.compression_size(),
            view.max_compbit_line()
        ));
    }
}

/// `NV2080_CTRL_FB_INFO_INDEX_1TO1_COMPTAG_ENABLED` (ctrl2080fb.h:362).
pub const FB_INFO_INDEX_1TO1_COMPTAG_ENABLED: u32 = 0x28;

/// The same answers for a VM without compression (D16): `FB_GET_INFO_V2` says "0 indicates
/// the GPU does not support compression" (ctrl2080fb.h:53-56) and no 1:1 comptag lines,
/// as the memsys static config already does (`bOneToOneComptagLineAllocation` 0,
/// `bDisableCompbitBacking` 1). The forwarded or replayed card's figures made the Windows
/// KMD, which chooses its kinds itself, allocate kind 0x09 surfaces that the shadow then
/// refused (C87/C88: 1.5 MiB per D3D device at VA 0x9e400000).
pub fn finish_off(cmd: u32, a: &mut Answer) {
    if cmd != FB_GET_INFO_V2 || a.status != NV_OK {
        return;
    }
    let count = (get32(&a.params, 0) as usize).min(a.params.len().saturating_sub(4) / 8);
    let mut cleared = Vec::new();
    for i in 0..count {
        let o = 4 + 8 * i;
        let index = get32(&a.params, o);
        if (index == FB_INFO_INDEX_COMPRESSION_SIZE || index == FB_INFO_INDEX_1TO1_COMPTAG_ENABLED)
            && get32(&a.params, o + 4) != 0
        {
            put(&mut a.params, o + 4, 0, 4);
            cleared.push(format!("{index:#x}"));
        }
    }
    if !cleared.is_empty() {
        a.note.push_str(&format!(
            "; index {} answered 0 (no compression, D16)",
            cleared.join(", ")
        ));
    }
}

/// The guest registry under compression. A Linux guest with `RMDisableScrubOnFree` set
/// gets no compressible surface at all: without scrub on free and without the Windows
/// (WDDM), vGPU-host or VF exemptions `kmemsysAllocComprResources_KERNEL` returns
/// NV_ERR_INVALID_STATE (kern_mem_sys.c:480-495), and the allocation fails instead of
/// falling back. Scrub on free in turn runs the scrubber's CeUtils channel in virtual
/// mode over a compressible alias of the whole heap (1:1 sets `bUseVasForCeMemoryOps`,
/// mem_mgr_gm107.c:1523-1527; gpu/mem_mgr/mem_scrub.c:152-155;
/// arch/maxwell/mem_utils_gm107.c:456-480, 831-850), which the device must execute.
pub fn guest_key_warnings(entries: &[(String, RegValue)]) -> Vec<String> {
    let scrub_off = entries.iter().any(|(n, v)| {
        n == "RMDisableScrubOnFree" && matches!(v, RegValue::U32(x) if *x != 0)
    });
    let mut out = Vec::new();
    if scrub_off {
        out.push(format!(
            "WARNING guest registry RMDisableScrubOnFree is set while {ENV}=slice: a Linux guest's compressible allocations fail with NV_ERR_INVALID_STATE (kmemsysAllocComprResources_KERNEL, kern_mem_sys.c:480-495); drop the key (its scrubber then runs a virtual-mode CE channel)"
        ));
    }
    out
}

/// The verdict on the vidmem PTEs the walker saw, `(kind, comptagline, count)` (fb.rs
/// `pte_kinds`, the device's exit summary). D16 wants every kind 0x06/0x00 with line 0; the
/// slice lets the guest's compressible kinds (0x08-0x0E, tu102/kind_macros.h:29) through
/// when their line is one of this VM's, every other kind with line 0.
pub fn pte_kinds_verdict(view: Option<&GuestView>, kinds: &[(u8, u32, u64)]) -> &'static str {
    match view {
        None if kinds.iter().any(|&(k, c, _)| !(k == 0x06 || k == 0x00) || c != 0) => {
            " -- COMPRESSIBLE KIND OR NON-ZERO COMPTAGLINE PRESENT (comptag hold, execution.md §7)"
        }
        None => " -- all uncompressed (kind 0x06/0x00, comptagline 0): comptag hold satisfied",
        Some(v) => {
            let (first, last) = v.lines();
            let ours = kinds.iter().all(|&(k, c, _)| match k {
                0x00..=0x06 => c == 0,
                0x08..=0x0E => (first..=last).contains(&c),
                _ => false,
            });
            if ours {
                " -- compression slice: compressible kinds carry comptag lines of this VM's FB only"
            } else {
                " -- compression slice: A KIND OR COMPTAG LINE OUTSIDE THIS VM'S FB (guest PTE; the host PTEs are the host RM's)"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrl::{Dispatcher, HostRm, INTERNAL_CLIENT, INTERNAL_DEVICE, INTERNAL_SUBDEVICE};
    use crate::ver::common as v;

    const GIB: u64 = 1 << 30;
    /// The 4 GiB profile on this host (docs/caraxes/memory-model.md §1).
    const BASE: u64 = 0xE900_0000;
    const END: u64 = 0x1_E900_0000;
    const FACTS: crate::recorded::Facts = crate::recorded::Facts {
        fb_bytes: 4 * GIB,
        gpcs: 3,
        tpcs_per_gpc: 6,
    };

    fn chip() -> &'static ChipData {
        crate::fixture::rtx2070().chip.data()
    }

    fn layout(on: bool) -> FbLayout {
        FbLayout::reserved(4 * GIB, 8 * GIB, BASE, END)
            .unwrap()
            .with_compression(on)
            .unwrap()
    }

    fn view() -> GuestView {
        GuestView::of(&layout(true), chip()).unwrap()
    }

    fn dispatcher(l: Option<FbLayout>, host: Option<Box<dyn HostRm>>) -> Dispatcher {
        let f = crate::fixture::rtx2070();
        let d = Dispatcher::new(f.chip.data(), &f.identity(4 * GIB), host);
        match l {
            Some(l) => d.with_layout(l),
            None => d,
        }
    }

    fn ask(d: &mut Dispatcher, object: u32, cmd: u32, params: &[u8]) -> Answer {
        d.control(INTERNAL_CLIENT, object, cmd, params)
    }

    fn memsys(l: Option<FbLayout>) -> Answer {
        let a = ask(
            &mut dispatcher(l, None),
            INTERNAL_SUBDEVICE,
            v::CMD_INTERNAL_MEMSYS_GET_STATIC_CONFIG,
            &[0u8; v::MEMSYS_SIZE],
        );
        assert_eq!(a.status, NV_OK, "{}", a.note);
        a
    }

    fn field(b: &[u8], st: &str, path: &str) -> u64 {
        let (o, n) = Abi::active().layout(st).unwrap().resolve(path).unwrap();
        let mut w = [0u8; 8];
        w[..n].copy_from_slice(&b[o..o + n]);
        u64::from_le_bytes(w)
    }

    /// `{count, {index, data}[]}` of FB_GET_INFO_V2 (1028 bytes).
    fn fb_info_request(indices: &[u32]) -> Vec<u8> {
        let mut p = vec![0u8; 1028];
        put(&mut p, 0, indices.len() as u64, 4);
        for (i, &x) in indices.iter().enumerate() {
            put(&mut p, 4 + 8 * i, x.into(), 4);
            put(&mut p, 8 + 8 * i, 0xAAAA, 4);
        }
        p
    }

    #[test]
    fn the_switch() {
        assert_eq!(Mode::parse(None), Ok(Mode::Off));
        assert_eq!(Mode::parse(Some("off")), Ok(Mode::Off));
        assert_eq!(Mode::parse(Some("")), Ok(Mode::Off));
        assert_eq!(Mode::parse(Some("slice")), Ok(Mode::Slice));
        assert!(Mode::parse(Some("on")).unwrap_err().contains(ENV));
        // Only a reservation can be compressible: its pages carry the lines.
        assert!(FbLayout::plain(GIB).with_compression(true).is_err());
        assert!(!FbLayout::plain(GIB).with_compression(false).unwrap().compressible);
        assert!(layout(true).compressible && !layout(false).compressible);
        assert!(GuestView::of(&layout(false), chip()).is_none());
        // A zero-based guest FB keeps its host reservation, so it can be compressible.
        let zero = FbLayout::reserved(4 * GIB, 8 * GIB, BASE, END)
            .unwrap()
            .zero_based()
            .unwrap()
            .with_compression(true)
            .unwrap();
        assert_eq!(GuestView::of(&zero, chip()).unwrap().lines(), (1, 0xFFE0));
    }

    /// The control ids are the version's (src/ver/v6xx/surface.rs).
    #[test]
    fn control_ids_are_the_versions() {
        let abi = Abi::active();
        for (name, id) in [
            ("NV0080_CTRL_CMD_FB_GET_COMPBIT_STORE_INFO", FB_GET_COMPBIT_STORE_INFO),
            ("NV2080_CTRL_CMD_FB_GET_COMPBITCOPY_CONSTRUCT_INFO", FB_GET_COMPBITCOPY_CONSTRUCT_INFO),
        ] {
            assert_eq!(abi.routed(name).unwrap().id, id, "{name}");
        }
        assert_eq!(crate::recorded::FB_GET_INFO_V2, FB_GET_INFO_V2);
        assert_eq!(crate::recorded::FB_GET_COMPBIT_STORE_INFO, FB_GET_COMPBIT_STORE_INFO);
        assert_eq!(v::public::CTAGS_SIZE, v::public::CTAGS_NUM + 4);
    }

    /// The chip facts are the card's own answers (chips/tu106/recorded-controls.tsv).
    #[test]
    fn chip_facts_are_the_recorded_cards() {
        let f = chip().compression.as_ref().unwrap();
        assert_eq!((f.chip.as_str(), f.policy), ("TU106", Policy::OneToOne));
        let r = crate::recorded::answer(FB_GET_INFO_V2, &fb_info_request(&[0x28, 0x01]), &FACTS).unwrap();
        assert_eq!(get32(&r.params, 8), 1, "1TO1_COMPTAG_ENABLED of the card");
        // The recorded compbit-store answer (the Windows KMD-block capture, the replay of
        // CARAXES_WIN_COMPBIT).
        let (status, card) = crate::recorded::Recorded::get()
            .exact_answer(FB_GET_COMPBIT_STORE_INFO, &[0u8; 88])
            .unwrap();
        assert_eq!(status, NV_OK);
        let g = |p: &str| field(card, STORE, p);
        assert_eq!(g("comptaglineAllocationPolicy"), f.policy.value() as u64);
        assert_eq!(g("comptagsPerCacheLine"), f.comptags_per_cache_line as u64);
        assert_eq!(g("cacheLineSize"), f.cache_line_size as u64);
        assert_eq!(g("cacheLineSizePerSlice"), f.cache_line_size_per_slice as u64);
        assert_eq!(g("cacheLineFetchAlignment"), f.cache_line_fetch_alignment as u64);
        assert_eq!(g("gobsPerComptagPerSlice"), f.gobs_per_comptag_per_slice as u64);
        assert_eq!(g("cbcCoveragePerSlice"), f.cbc_coverage_per_slice as u64);
        // The card's MaxCompbitLine and COMPRESSION_SIZE are one number: its compressible
        // top 0x1_f7d0_0000, in lines and in bytes cut to 32 bits. The device derives the
        // guest's the same way.
        assert_eq!(g("MaxCompbitLine"), 0x1_F7D0);
        assert_eq!(get32(&r.params, 16), (0x1_F7D0u64 << 16) as u32);
        let card_view = GuestView { facts: f, fb: (0, 0x1_F7D0_0000) };
        assert_eq!(card_view.max_compbit_line() as u64, g("MaxCompbitLine"));
        assert_eq!(card_view.compression_size(), get32(&r.params, 16));
        // Its backing store sits in the host's FB below every reservation.
        assert_eq!((g("AddressSpace"), g("Address")), (2, 0x16A_0000));
    }

    /// A costume presenting another chip inherits `[compression]` from its base but gets
    /// no compression: its HAL expects another policy.
    #[test]
    fn costumes_get_no_compression() {
        for name in ["ga106", "ad106", "gh100", "tu104"] {
            let c = crate::chip::by_name(name).unwrap().data();
            assert!(GuestView::of(&layout(true), c).is_none(), "{name}");
        }
        assert!(crate::chip::by_name("gb206").unwrap().data().compression.is_none());
    }

    /// 1:1 lines of the guest's FB: 1 + PA[35:16], in the 20-bit PTE field, and the lines
    /// of two VMs' reservations are disjoint because their pages are.
    #[test]
    fn comptag_lines_follow_the_page() {
        let v = view();
        assert_eq!(v.comptag_line(0), 1);
        assert_eq!(v.comptag_line(0xE900_FFFF), 0xE901);
        assert_eq!(v.comptag_line(0xE901_0000), 0xE902);
        // SPA bits above 35 are not the line's (dev_fb_addendum.h:27).
        assert_eq!(v.comptag_line(1 << 36), 1);
        let (first, last) = v.lines();
        assert_eq!(v.fb, (BASE, END - crate::fb::CARVE_OUT));
        assert_eq!((first, last), (0xE901, 0x1_E8E0));
        assert!(last < 1 << PTE_COMPTAGLINE_BITS);
        assert_eq!(v.max_compbit_line(), last);
        assert_eq!(v.compression_size(), 0xE8E0_0000);
        // A second VM's slice right below: no line in common.
        let other = FbLayout::reserved(GIB, 8 * GIB, BASE - GIB, BASE)
            .unwrap()
            .with_compression(true)
            .unwrap();
        let o = GuestView::of(&other, chip()).unwrap().lines();
        assert!(o.1 < first, "{o:x?} vs {first:#x}");
        // A BAR1 pool comes out of the compressible FB with the usable region.
        let pooled = FbLayout::reserved(4 * GIB, 8 * GIB, BASE, END)
            .unwrap()
            .with_pool(64 << 20)
            .unwrap()
            .with_compression(true)
            .unwrap();
        let p = GuestView::of(&pooled, chip()).unwrap();
        assert_eq!(p.fb.1, END - crate::fb::CARVE_OUT - (64 << 20));
        assert_eq!(p.max_compbit_line(), last - (64 << 20 >> 16));
    }

    /// INTERNAL_MEMSYS_GET_STATIC_CONFIG: off is the chip's D16 answer, byte for byte with
    /// and without a reservation; the slice changes the five compression flags only.
    #[test]
    fn memsys_static_config_off_and_slice() {
        let bare = memsys(None);
        let d16 = memsys(Some(layout(false)));
        assert_eq!(d16.params, bare.params);
        assert_eq!((d16.label, d16.note.as_str()), (bare.label, "memsys static config"));
        let off_ = |f: &str| off(MEMSYS, f);
        let flag = |b: &[u8], f: &str| b[off_(f)];
        assert_eq!(flag(&d16.params, "bDisableCompbitBacking"), 1);
        assert_eq!(flag(&d16.params, "bOneToOneComptagLineAllocation"), 0);
        let on = memsys(Some(layout(true)));
        assert_eq!(on.label, d16.label, "the memsys values stay the chip's");
        assert!(on.note.contains("compression slice") && on.note.contains("recorded-controls.tsv"), "{}", on.note);
        let want = [
            ("bOneToOneComptagLineAllocation", 1),
            ("bUseOneToFourComptagLineAllocation", 0),
            ("bUseRawModeComptaglineAllocation", 0),
            ("bDisableCompbitBacking", 0),
            ("bDisablePostL2Compression", 0),
        ];
        for (f, x) in want {
            assert_eq!(flag(&on.params, f), x, "{f}");
        }
        let changed: Vec<usize> = (0..on.params.len()).filter(|&i| on.params[i] != d16.params[i]).collect();
        assert_eq!(changed, vec![off_("bOneToOneComptagLineAllocation"), off_("bDisableCompbitBacking")]);
        // Pinned to the generated layout of both versions (ctrl2080internal.h:96-137).
        assert_eq!((off_("bOneToOneComptagLineAllocation"), off_("bUseRawModeComptaglineAllocation")), (0, 2));
        assert_eq!((off_("bDisableCompbitBacking"), off_("bDisablePostL2Compression")), (3, 4));
        assert_eq!(off_("bDisableCompbitBacking"), v::MEMSYS_DISABLE_COMPBIT_BACKING);
    }

    /// FB_GET_COMPBIT_STORE_INFO: off is the chip table's D16 refusal; the slice answers
    /// the card's policy and geometry with this VM's lines and no host address.
    #[test]
    fn compbit_store_info_off_and_slice() {
        let req = vec![0u8; size(STORE)];
        let off = ask(&mut dispatcher(Some(layout(false)), None), INTERNAL_DEVICE, FB_GET_COMPBIT_STORE_INFO, &req);
        assert_eq!((off.status, off.label), (NV_ERR_NOT_SUPPORTED, "refused (no compression, D16: no compbit backing store)"));
        assert_eq!(off.params, req);
        let on = ask(&mut dispatcher(Some(layout(true)), None), INTERNAL_DEVICE, FB_GET_COMPBIT_STORE_INFO, &req);
        assert_eq!((on.status, on.label, on.params.len()), (NV_OK, LABEL_STORE, 88));
        let f = chip().compression.as_ref().unwrap();
        let g = |p: &str| field(&on.params, STORE, p);
        assert_eq!(g("comptaglineAllocationPolicy"), 1, "POLICY_1TO1");
        assert_eq!(g("MaxCompbitLine"), 0x1_E8E0);
        assert_eq!(g("comptagsPerCacheLine"), f.comptags_per_cache_line as u64);
        assert_eq!(g("cacheLineSize"), 0x1_0000);
        assert_eq!(g("gobsPerComptagPerSlice"), 8);
        assert_eq!(g("cbcCoveragePerSlice"), 0xD8_0000);
        for p in ["Size", "Address", "AddressSpace", "backingStoreBase", "backingStoreCbcBase", "privRegionStartOffset", "cbcRegionSize"] {
            assert_eq!(g(p), 0, "{p}: no backing-store location reaches the guest");
        }
        // Not a byte of the card's store addresses (0x16a0000, CBC base 0x5a8).
        let (_, card) = crate::recorded::Recorded::get().exact_answer(FB_GET_COMPBIT_STORE_INFO, &[0u8; 88]).unwrap();
        assert_ne!(field(card, STORE, "backingStoreBase"), 0);
    }

    /// The CBC-eviction and compbit-copy controls: the same bytes and status as today,
    /// labelled for the slice.
    #[test]
    fn cbc_eviction_and_compbit_copy() {
        let ctags = vec![0xFFu8; v::public::CTAGS_SIZE];
        let copy = vec![0x11u8; size("NV2080_CTRL_CMD_FB_GET_COMPBITCOPY_CONSTRUCT_INFO_PARAMS")];
        let mut d_off = dispatcher(Some(layout(false)), None);
        let mut d_on = dispatcher(Some(layout(true)), None);
        let (a, b) = (
            ask(&mut d_off, INTERNAL_SUBDEVICE, FB_GET_CTAGS_FOR_CBC_EVICTION, &ctags),
            ask(&mut d_on, INTERNAL_SUBDEVICE, FB_GET_CTAGS_FOR_CBC_EVICTION, &ctags),
        );
        assert_eq!((a.status, &a.params), (b.status, &b.params));
        assert_eq!((b.status, get32(&b.params, v::public::CTAGS_NUM)), (NV_OK, 0));
        assert!(a.label.contains("D16") && b.label == LABEL_CTAGS, "{} / {}", a.label, b.label);
        let (a, b) = (
            ask(&mut d_off, INTERNAL_SUBDEVICE, FB_GET_COMPBITCOPY_CONSTRUCT_INFO, &copy),
            ask(&mut d_on, INTERNAL_SUBDEVICE, FB_GET_COMPBITCOPY_CONSTRUCT_INFO, &copy),
        );
        assert_eq!((a.status, &a.params), (b.status, &b.params));
        assert_eq!(b.status, NV_ERR_NOT_SUPPORTED);
        assert_eq!(b.label, LABEL_COMPBITCOPY);
    }

    /// FB_GET_INFO_V2 COMPRESSION_SIZE: off says no compression (0, and no 1:1 comptag
    /// lines, D16) whatever the recording in pure emulation or the host RM answers; the
    /// slice's is this VM's, every other index stays.
    #[test]
    fn fb_info_compression_size_off_and_slice() {
        let req = fb_info_request(&[0x07, 0x01, 0x28, 0x0D]);
        let data = |a: &Answer, i: usize| get32(&a.params, 8 + 8 * i);
        // Pure emulation: the recorded table.
        let off = ask(&mut dispatcher(Some(layout(false)), None), INTERNAL_SUBDEVICE, FB_GET_INFO_V2, &req);
        let on = ask(&mut dispatcher(Some(layout(true)), None), INTERNAL_SUBDEVICE, FB_GET_INFO_V2, &req);
        assert_eq!((off.status, on.status), (NV_OK, NV_OK));
        assert_eq!(data(&off, 1), 0, "no compression (D16)");
        assert_eq!(data(&on, 1), 0xE8E0_0000, "this VM's lines");
        assert_eq!((data(&off, 2), data(&on, 2)), (0, 1), "1TO1_COMPTAG_ENABLED");
        for i in [0, 3] {
            assert_eq!(data(&off, i), data(&on, i), "index {i}");
        }
        assert!(on.note.contains("COMPRESSION_SIZE 0xe8e00000"), "{}", on.note);
        assert!(off.note.contains("no compression, D16"), "{}", off.note);
        // With a host RM (nvml.rs forwards the list): the host card's value, and the slice's.
        struct Host;
        impl HostRm for Host {
            fn control(&mut self, _: u32, cmd: u32, p: &mut [u8]) -> u32 {
                assert_eq!(cmd, FB_GET_INFO_V2);
                for i in 0..get32(p, 0) as usize {
                    let x = if get32(p, 4 + 8 * i) == 1 { 0xF7D0_0000 } else { 0x1234 };
                    put(p, 8 + 8 * i, x, 4);
                }
                NV_OK
            }
        }
        let off = ask(&mut dispatcher(Some(layout(false)), Some(Box::new(Host))), INTERNAL_SUBDEVICE, FB_GET_INFO_V2, &req);
        let on = ask(&mut dispatcher(Some(layout(true)), Some(Box::new(Host))), INTERNAL_SUBDEVICE, FB_GET_INFO_V2, &req);
        assert!(off.label.starts_with("host") && on.label == off.label, "{}", on.label);
        assert_eq!((data(&off, 1), data(&on, 1)), (0, 0xE8E0_0000));
        assert_eq!((data(&off, 2), data(&on, 2)), (0, 0x1234));
        assert_eq!(data(&on, 0), ((4 * GIB) >> 10) as u32, "sizes stay the profile's");
    }

    /// The exit summary's PTE audit: D16's two verdicts unchanged, the slice's by line range.
    #[test]
    fn pte_audit() {
        let safe = [(0x06, 0, 10), (0x00, 0, 2)];
        assert!(pte_kinds_verdict(None, &safe).contains("comptag hold satisfied"));
        assert!(pte_kinds_verdict(None, &[(0x08, 0xE901, 1)]).contains("PRESENT"));
        let v = view();
        let ours = [(0x06, 0, 3), (0x0C, 0xE901, 4), (0x08, 0x1_E8E0, 1)];
        assert!(pte_kinds_verdict(Some(&v), &ours).contains("of this VM's FB only"));
        for bad in [(0x08, 0xE900, 1), (0x08, 0x1_E8E1, 1), (0x06, 5, 1), (0x0F, 0, 1)] {
            assert!(pte_kinds_verdict(Some(&v), &[bad]).contains("OUTSIDE"), "{bad:x?}");
        }
    }

    /// The guest registry: under the slice a Linux guest must not carry RMDisableScrubOnFree.
    #[test]
    fn guest_keys_under_compression() {
        let quickstart: Vec<(String, RegValue)> = vec![
            ("RmDisableGlobalCeUtils".into(), RegValue::U32(1)),
            ("RMDisableScrubOnFree".into(), RegValue::U32(1)),
            ("RmRcWatchdog".into(), RegValue::U32(0)),
        ];
        let w = guest_key_warnings(&quickstart);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("kern_mem_sys.c:480-495"), "{}", w[0]);
        assert!(guest_key_warnings(&quickstart[..1]).is_empty());
        let zero = vec![("RMDisableScrubOnFree".to_string(), RegValue::U32(0))];
        assert!(guest_key_warnings(&zero).is_empty());
    }

    /// The 615 guest's compression decisions, transcribed from its source
    /// (src/nvidia/src/kernel/ of 615.71.09) over what the device answers.
    mod guest615 {
        use super::super::GuestView;

        pub const NV_ERR_INVALID_STATE: u32 = 0x40;
        /// tu102/dev_mmu.h:97-111.
        pub const KIND_GENERIC_MEMORY: u8 = 0x06;
        pub const KIND_S8Z24: u8 = 0x03;
        pub const KIND_GENERIC_MEMORY_COMPRESSIBLE: u8 = 0x08;
        pub const KIND_S8Z24_COMPRESSIBLE_DISABLE_PLC: u8 = 0x0C;

        /// The memsys static config the guest caches (kern_mem_sys.c:517-529).
        #[derive(Debug)]
        pub struct Config {
            pub one_to_one: bool,
            pub one_to_four: bool,
            pub raw: bool,
            pub disable_backing: bool,
            pub disable_plc: bool,
        }

        /// The guest's platform and registry.
        #[derive(Clone, Copy)]
        pub struct Os {
            /// Windows WDDM (not TCC).
            pub wddm: bool,
            /// `RMDisableScrubOnFree` nonzero.
            pub scrub_key: bool,
        }

        /// `PTEKIND_COMPRESSIBLE` (tu102/kind_macros.h:29).
        pub fn compressible(kind: u8) -> bool {
            (0x08..=0x0E).contains(&kind)
        }

        /// `bScrubOnFreeEnabled`: on for TU106 (generated/g_mem_mgr_nvoc.c:411-416), off with
        /// the key (gpu/mem_mgr/mem_mgr.c:221-225) and on Windows WDDM
        /// (arch/maxwell/mem_mgr_gm107.c:1506-1515).
        pub fn scrub_on_free(os: Os) -> bool {
            !os.scrub_key && !os.wddm
        }

        /// `bUseVasForCeMemoryOps` (mem_mgr_gm107.c:1523-1527).
        pub fn vas_for_ce(c: &Config) -> bool {
            c.one_to_one
        }

        /// `memmgrAllocHal_GM107` (mem_mgr_gm107.c:428-447) then `memmgrChooseKind_TU102`
        /// (arch/turing/mem_mgr_tu102.c:245-272) for a surface that asks for compression
        /// (`NVOS32_ATTR_COMPR_ANY`) on FB, where `memmgrComprSupported` holds (big pages,
        /// mem_mgr.c:1559-1590): the kind and whether the comptag path runs.
        pub fn choose(c: &Config, depth: bool) -> (u8, bool) {
            let compr = !c.disable_backing;
            let kind = match (depth, compr) {
                (false, false) => KIND_GENERIC_MEMORY,
                (false, true) if c.disable_plc => 0x09,
                (false, true) => KIND_GENERIC_MEMORY_COMPRESSIBLE,
                (true, false) => KIND_S8Z24,
                (true, true) => KIND_S8Z24_COMPRESSIBLE_DISABLE_PLC,
            };
            (kind, compr && compressible(kind))
        }

        /// `kmemsysAllocComprResources_KERNEL` (gpu/mem_sys/kern_mem_sys.c:456-505).
        pub fn alloc_compr_resources(c: &Config, os: Os) -> Result<(), u32> {
            // :473-474
            if !(c.one_to_one || c.raw) {
                return Err(NV_ERR_INVALID_STATE);
            }
            // :480-495: no simulation, zero-FB, vGPU host, VF or SLI here.
            if !scrub_on_free(os) {
                if !os.wddm {
                    return Err(NV_ERR_INVALID_STATE);
                }
            } else if c.one_to_one && !vas_for_ce(c) {
                // :496-499
                return Err(NV_ERR_INVALID_STATE);
            }
            Ok(())
        }

        /// One PTE of a compressible surface at FB address `pa`: `dmaUpdateVASpace`
        /// (arch/maxwell/virt_mem_allocator_gm107.c:1866-1943; two regions, so the region
        /// of `pa` decides) and `kgmmuFieldSetKindCompTags` (gpu/mmu/kern_gmmu.c:952-1003:
        /// `bPhysBasedComptags`, not SR-IOV, not raw: `memmgrDetermineComptag_TU102`).
        pub fn pte(c: &Config, regions: &[(u64, u64, bool)], kind: u8, pa: u64, line: impl Fn(u64) -> u32) -> (u8, u32) {
            let region = regions.iter().find(|r| pa >= r.0 && pa <= r.1);
            if !compressible(kind) || !region.is_some_and(|r| r.2) {
                let uncompressed = match kind {
                    0x08 | 0x09 => KIND_GENERIC_MEMORY,
                    0x0C => KIND_S8Z24,
                    k => k,
                };
                return (uncompressed, 0);
            }
            assert!(c.one_to_one || c.raw, "kern_gmmu.c:979");
            (kind, if c.raw { 1 } else { line(pa) })
        }

        pub fn config(b: &[u8]) -> Config {
            let f = |p: &str| b[super::off(super::MEMSYS, p)] != 0;
            Config {
                one_to_one: f("bOneToOneComptagLineAllocation"),
                one_to_four: f("bUseOneToFourComptagLineAllocation"),
                raw: f("bUseRawModeComptaglineAllocation"),
                disable_backing: f("bDisableCompbitBacking"),
                disable_plc: f("bDisablePostL2Compression"),
            }
        }

        /// The regions of the static info: `(base, limit, supportCompressed)`.
        pub fn regions(img: &[u8]) -> Vec<(u64, u64, bool)> {
            use crate::ver::common as v;
            let n = u32::from_le_bytes(img[v::SI_NUM_FB_REGIONS..v::SI_NUM_FB_REGIONS + 4].try_into().unwrap());
            (0..n as usize)
                .map(|i| {
                    let o = v::SI_FB_REGIONS + i * v::FB_REGION_SIZE;
                    let q = |x: usize| u64::from_le_bytes(img[o + x..o + x + 8].try_into().unwrap());
                    (q(v::FB_REGION_BASE), q(v::FB_REGION_LIMIT), img[o + v::FB_REGION_SUPPORT_COMPRESSED] != 0)
                })
                .collect()
        }

        pub fn line_of(view: Option<GuestView>) -> impl Fn(u64) -> u32 {
            move |pa| view.map_or(0, |v| v.comptag_line(pa))
        }
    }

    /// The 615 guest over the device's answers: with the slice a Linux guest with scrub on
    /// free and a Windows guest get their compressed depth buffer (S8Z24 compressible,
    /// the kind whose absence made SotTR's 2D fill fault) with the 1:1 lines of their own
    /// pages; a Linux guest that keeps RMDisableScrubOnFree gets NV_ERR_INVALID_STATE; with
    /// the switch off nothing asks for comptags and the kind stays S8Z24 (D16, as today).
    #[test]
    fn guest615_alloc_compr_resources_preconditions_hold() {
        use guest615::*;
        let f = crate::fixture::rtx2070();
        let linux = Os { wddm: false, scrub_key: false };
        let quickstart = Os { wddm: false, scrub_key: true };
        let windows = Os { wddm: true, scrub_key: false };
        for on in [false, true] {
            let l = layout(on);
            let c = config(&memsys(Some(l)).params);
            let (img, _) = crate::rpc::build_static_info(f.chip.data(), &f.identity(4 * GIB), None, &l);
            let regions = regions(&img);
            let view = GuestView::of(&l, chip());
            assert_eq!(regions.len(), 2);
            assert!(!c.one_to_four);
            for depth in [false, true] {
                let (kind, comptags) = choose(&c, depth);
                if !on {
                    assert!(!comptags && !compressible(kind), "D16: {kind:#x}");
                    assert_eq!(kind, if depth { KIND_S8Z24 } else { KIND_GENERIC_MEMORY });
                    continue;
                }
                assert!(comptags);
                assert_eq!(kind, if depth { KIND_S8Z24_COMPRESSIBLE_DISABLE_PLC } else { KIND_GENERIC_MEMORY_COMPRESSIBLE });
                assert_eq!(alloc_compr_resources(&c, linux), Ok(()));
                assert_eq!(alloc_compr_resources(&c, windows), Ok(()));
                assert_eq!(alloc_compr_resources(&c, quickstart), Err(NV_ERR_INVALID_STATE));
                // Every page of the heap keeps its compressible kind and gets its own line.
                let v = view.unwrap();
                for pa in [v.fb.0, v.fb.0 + 0x1_0000, (v.fb.0 + v.fb.1) / 2, v.fb.1 - 0x1_0000] {
                    let (k, line) = pte(&c, &regions, kind, pa, line_of(view));
                    assert_eq!((k, line), (kind, 1 + (pa >> 16) as u32), "{pa:#x}");
                    assert!(line <= v.max_compbit_line() && line < 1 << PTE_COMPTAGLINE_BITS);
                }
                // The reserved carve-out is never compressible.
                let top = regions[1].0;
                assert!(!regions[1].2);
                assert_eq!(pte(&c, &regions, kind, top, line_of(view)).1, 0);
            }
            // Without the 1:1 flag the guest would refuse every compressible allocation.
            let mut no_policy = config(&memsys(Some(l)).params);
            no_policy.one_to_one = false;
            assert_eq!(alloc_compr_resources(&no_policy, linux), Err(NV_ERR_INVALID_STATE));
        }
    }

    /// Without compression the FB info says so (D16): the card's COMPRESSION_SIZE and
    /// 1TO1_COMPTAG_ENABLED, forwarded or replayed, are answered 0, other indices keep
    /// their answer, and the compbit store is left to the chip table's refusal.
    #[test]
    fn without_compression_the_fb_info_says_none() {
        let mut a = crate::recorded::answer(
            FB_GET_INFO_V2,
            &fb_info_request(&[0x28, 0x01, 0x02]),
            &FACTS,
        )
        .unwrap();
        assert_eq!(get32(&a.params, 8), 1, "the card's 1TO1_COMPTAG_ENABLED");
        assert_ne!(get32(&a.params, 16), 0, "the card's COMPRESSION_SIZE");
        let other = get32(&a.params, 24);
        finish_off(FB_GET_INFO_V2, &mut a);
        assert_eq!((get32(&a.params, 8), get32(&a.params, 16)), (0, 0));
        assert_eq!(get32(&a.params, 24), other, "index 0x02 changed");
        assert!(a.note.contains("no compression, D16"), "{}", a.note);
        // Other controls pass untouched.
        let mut b =
            crate::recorded::answer(FB_GET_INFO_V2, &fb_info_request(&[0x01]), &FACTS).unwrap();
        let before = b.params.clone();
        finish_off(FB_GET_COMPBIT_STORE_INFO, &mut b);
        assert_eq!(b.params, before);
        // CARAXES_WIN_COMPBIT is retired: the store is never a recorded override.
        assert!(!crate::recorded::overrides(FB_GET_COMPBIT_STORE_INFO));
    }
}

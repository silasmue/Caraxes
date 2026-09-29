// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Real compression per VM slice (decision D16b, docs/caraxes/README.md; the research in
//! docs/research/compression-isolation-2026-09-28.md). `CARAXES_COMPRESSION=slice` allocates
//! the VRAM reservation compressed and lets the shadow map the guest's compressible kinds
//! into it. Off (the default) keeps D16: no compressible kind reaches the host.
//!
//! **Why the slice is isolated.** The host RM runs Turing in 1:1 comptag mode (`FB_GET_INFO`
//! index `1TO1_COMPTAG_ENABLED` is 1 on the RTX 2070,
//! caraxes/reference/rtx2070/kmd-controls-2026-09-26.md:42). A compressed allocation gets no
//! comptag range there, only the mark "physically based" (`ctagId` -1,
//! `kmemsysAllocComprResources_KERNEL`, kern_mem_sys.c:456-503). Every compressible PTE the
//! host RM writes then carries COMPTAGLINE = 1 + SPA[35:16] of its own physical page
//! (`memmgrGetKindComprForGpu_KERNEL`, mem_mgr.c:2598-2616; `kgmmuFieldSetKindCompTags`,
//! kern_gmmu.c:975-996; `memmgrDetermineComptag_TU102`, mem_mgr_tu102.c:505-513;
//! `_BITS_FROM_SPA` 35:16, tu102/dev_fb_addendum.h:27). A comptag line belongs to whoever
//! owns the 64 KiB page: the slice's lines are `[1 + base>>16, 1 + (end-1)>>16]`, and no
//! other allocation has any of them.
//!
//! **The invariants and where they hold.**
//! - (a) No host PTE carries a comptag line outside the slice. The filter maps a
//!   compressible kind only on covered, 64 KiB-aligned slice pages of at least 64 KiB
//!   ([`Policy::kind_use`], shadow.rs `resolve_leaf`); everything else is downgraded to
//!   its uncompressed twin or refused. The shadow samples the host PTEs it made with
//!   `NV0080_CTRL_CMD_DMA_GET_PTE_INFO` ([`Policy::check_pte`]); a violation is an ERROR
//!   and stops every compressible map ([`Policy::violation`]).
//! - (b) Shadow-only execution: every host channel runs on a shadow VA space (mirror.rs
//!   `is_shadow`), and the mirror refuses every raw path (a host channel on the guest's
//!   own tables would use the guest's COMPTAGLINE).
//! - (c) Guest comptag values never pass: the walk reads kind and address only
//!   (ptcache.rs), and `RM_MAP_MEMORY_DMA` has no comptag argument; the host RM computes it.
//! - (d) Comptags are cleared at start and at free: the host grants compression only with
//!   scrub-on-free (kern_mem_sys.c:480-496), in 1:1 mode its scrubber writes every freed page
//!   through a compressible (kind 0x09) alias of the FB (mem_mgr_gm107.c:1523-1527,
//!   mem_scrub.c:152-155, mem_utils_gm107.c:843, channel_utils.c:1096-1099), and PMA hands
//!   out scrubbed pages only (phys_mem_allocator.c:686-693). The device also writes the
//!   whole slice through its own compressible BAR1 windows after the allocation and before
//!   the free (hostmem.rs `clear_slice`; `CARAXES_COMPRESSION_CLEAR=0` skips it).
//!
//! The slice also counts as a ZBC-kind surface on the host (`ZBC_SKIP_ZBCREFCOUNT` left
//! `_NO`, checked in [`Grant::verdict`]), so the host keeps the VM's ZBC entries (zbc.rs)
//! while its compressed surfaces may reference them.
//!
//! Line numbers are those of the host driver's open source, 615.71.09.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;

/// The knob: `slice` or `off` (the default).
pub const ENV: &str = "CARAXES_COMPRESSION";

/// One comptag line per 64 KiB on TU10x (`comprPageShift` 16).
pub const COMPR_PAGE: u64 = 0x1_0000;
/// `NV_MMU_PTE_KIND_GENERIC_MEMORY_COMPRESSIBLE_DISABLE_PLC` (tu102/dev_mmu.h:106): the
/// reservation's own kind (`NVOS32_ATTR_COMPR_DISABLE_PLC_ANY`, `memmgrChooseKind_TU102`,
/// mem_mgr_tu102.c:250-258), the kind the host scrubber writes with. Without post-L2
/// compression, a write through the device's own BAR1 windows lands in DRAM as written,
/// so physical readers (page walks, USERD, instance blocks) see what the device wrote.
pub const KIND_RESERVATION: u32 = 0x09;
/// `NVOS32_ATTR_COMPR` (nvos.h:988-994), bits 13:12 of `NV_MEMORY_ALLOCATION_PARAMS::attr`.
pub const ATTR_COMPR_SHIFT: u32 = 12;
pub const ATTR_COMPR_DISABLE_PLC_ANY: u32 = 3;
/// `NVOS32_ATTR2_ZBC` (nvos.h:1108-1113), bits 1:0 of `attr2`: the host answers
/// `PREFER_ZBC` exactly when it counted the allocation as a ZBC surface
/// (`memmgrAllocHal_GM107`, mem_mgr_gm107.c:539-563).
pub const ATTR2_ZBC_MASK: u32 = 3;
pub const ATTR2_ZBC_PREFER_ZBC: u32 = 2;
/// `NVOS32_ATTR2_ZBC_SKIP_ZBCREFCOUNT` (nvos.h:1233-1235), bit 17 of `attr2`: left `_NO`,
/// so the slice counts as a ZBC-kind surface for its whole life.
pub const ATTR2_ZBC_SKIP_ZBCREFCOUNT: u32 = 1 << 17;
/// `NV2080_CTRL_FB_INFO_INDEX_*` (ctrl2080fb.h:319, 362).
pub const FB_INFO_COMPRESSION_SIZE: u32 = 0x01;
pub const FB_INFO_1TO1_COMPTAG_ENABLED: u32 = 0x28;
/// `NV0041_CTRL_CMD_GET_SURFACE_INFO` (ctrl0041.h:279, NON_PRIVILEGED): indices and flag.
pub const CMD_GET_SURFACE_INFO: u32 = 0x0041_0110;
pub const SURFACE_INFO_ATTRS: u32 = 0x01;
pub const SURFACE_INFO_COMPR_COVERAGE: u32 = 0x05;
pub const SURFACE_INFO_ATTRS_COMPR: u32 = 0x02;
/// `NV0041_CTRL_CMD_GET_SURFACE_COMPRESSION_COVERAGE` (ctrl0041.h:289-310, NON_PRIVILEGED):
/// `{hSubDevice, lineMin, lineMax, format}`.
pub const CMD_GET_SURFACE_COMPRESSION_COVERAGE: u32 = 0x0041_0112;
/// `NV0080_CTRL_CMD_DMA_GET_PTE_INFO` (ctrl0080dma.h:171-184). Flagged
/// `RMCTRL_FLAGS_RM_TEST_ONLY_CODE` (g_device_nvoc.c:1084): refused with
/// `NV_ERR_TEST_ONLY_CODE_NOT_ENABLED` unless the host sets `EnableRmTestOnlyCode`
/// (control.c:858-864).
pub const CMD_DMA_GET_PTE_INFO: u32 = 0x0080_1801;
pub const PTE_INFO_SIZE: usize = 184;
const PTE_INFO_BLOCKS: usize = 5;
pub const NV_ERR_TEST_ONLY_CODE_NOT_ENABLED: u32 = 0x7E;

/// Whether a Turing kind is compressible (0x08-0x0E, tu102/dev_mmu.h:105-111).
pub fn compressible(kind: u8) -> bool {
    (0x08..=0x0E).contains(&kind)
}

/// The D16b allowlist: the uncompressed kinds 0x00-0x06 and the compressible 0x08-0x0E.
/// INVALID (0x07) and anything above 0x0F are never mapped; SMSKED_MESSAGE (0x0F) maps as GENERIC_MEMORY.
pub fn allowlisted(kind: u8) -> bool {
    kind <= 0x06 || compressible(kind)
}

/// The uncompressed twin of a kind: `memmgrGetUncompressedKind_TU102` with
/// `bReleaseReacquire` false (mem_mgr_tu102.c:305-351). `None` outside the allowlist.
pub fn uncompressed(kind: u8) -> Option<u8> {
    match kind {
        0x00..=0x06 => Some(kind),
        0x08 | 0x09 => Some(0x06),
        0x0A => Some(0x02),
        0x0B => Some(0x01),
        0x0C => Some(0x03),
        0x0D => Some(0x04),
        0x0E => Some(0x05),
        _ => None,
    }
}

/// The comptag line the host RM writes for a compressible PTE of physical page `spa`:
/// 1 + SPA[35:16] (`memmgrDetermineComptag_TU102`, mem_mgr_tu102.c:505-513).
pub fn comptag_line(spa: u64) -> u32 {
    1 + ((spa >> 16) & 0xF_FFFF) as u32
}

/// The switch's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Off,
    Slice,
}

/// Parse [`ENV`]: unset, empty, `0` or `off` is off; `slice` is on; anything else is an error.
pub fn parse(v: Option<&str>) -> Result<Mode, String> {
    match v.map(str::trim) {
        None | Some("") | Some("0") | Some("off") => Ok(Mode::Off),
        Some("slice") => Ok(Mode::Slice),
        Some(o) => Err(format!(
            "{ENV}={o:?} is not a compression mode: expected slice or off"
        )),
    }
}

/// The switch from the environment.
pub fn requested() -> Result<Mode, String> {
    parse(std::env::var(ENV).ok().as_deref())
}

/// What the host granted for the reservation, read back after the allocation (hostmem.rs
/// `query_grant`). Every figure is a host answer; a control that failed leaves its zero.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Grant {
    /// The reservation's host physical range.
    pub base: u64,
    pub size: u64,
    /// `NV_MEMORY_ALLOCATION_PARAMS::attr`, `attr2` and `format` (the PTE kind) the
    /// allocation returned (`memmgrAllocResources`, mem_utils.c:1581-1583).
    pub attr: u32,
    pub attr2: u32,
    pub kind: u32,
    /// `GET_SURFACE_INFO`: `ATTRS` and `COMPR_COVERAGE` in 64 KiB units (mem_ctrl.c:155-187).
    pub surface_attrs: u32,
    pub coverage_64k: u32,
    /// `GET_SURFACE_COMPRESSION_COVERAGE`: 1/1/1 is the signature of physically based
    /// comptags (`compTagLineMin` 1, `compPageIndexLo`/`Hi` 0: mem_mgr.c:2607-2614,
    /// mem_ctrl.c:79-87).
    pub line_min: u32,
    pub line_max: u32,
    pub format: u32,
    /// `FB_GET_INFO_V2` `1TO1_COMPTAG_ENABLED` and `COMPRESSION_SIZE` (raw).
    pub one_to_one: u64,
    pub compression_size: u64,
    /// Statuses of the three read-back controls (surface info, coverage, FB info).
    pub statuses: [u32; 3],
}

impl Grant {
    /// `NVOS32_ATTR_COMPR` of the returned attributes.
    pub fn attr_compr(&self) -> u32 {
        (self.attr >> ATTR_COMPR_SHIFT) & 3
    }

    /// Bytes of the reservation, from its start, that carry comptags.
    pub fn covered(&self) -> u64 {
        (u64::from(self.coverage_64k) * COMPR_PAGE).min(self.size)
    }

    /// The comptag lines of the covered part, inclusive.
    pub fn lines(&self) -> (u32, u32) {
        let c = self.covered().max(1);
        (comptag_line(self.base), comptag_line(self.base + c - 1))
    }

    /// Whether the grant is the one the isolation argument needs: 1:1 mode, a
    /// compressed allocation of kind 0x09 whose comptags are physically based, with coverage
    /// from its first page, 64 KiB aligned, and counted as a ZBC surface, so the host keeps
    /// the ZBC entries the VM's compressed surfaces reference while the slice lives (zbc.rs;
    /// the host flushes its table only with no ZBC surface left, mem_mgr_gm107.c:611-634).
    /// `Err` lists every reason.
    pub fn verdict(&self) -> Result<(), String> {
        let mut why: Vec<String> = Vec::new();
        if self.statuses.iter().any(|&s| s != 0) {
            why.push(format!("a read-back control failed (statuses {:#x?})", self.statuses));
        }
        if self.one_to_one != 1 {
            why.push(format!(
                "the host is not in 1:1 comptag mode (1TO1_COMPTAG_ENABLED {}): a comptag line would not follow its page",
                self.one_to_one
            ));
        }
        if self.attr_compr() == 0 {
            why.push(format!("the allocation came back COMPR_NONE (attr {:#x})", self.attr));
        }
        if self.attr2 & ATTR2_ZBC_MASK != ATTR2_ZBC_PREFER_ZBC {
            why.push(format!(
                "the host did not count the slice as a ZBC surface (attr2 {:#x}, ZBC not PREFER_ZBC): it could drop the VM's ZBC entries while compressed surfaces still use them",
                self.attr2
            ));
        }
        if self.kind != KIND_RESERVATION {
            why.push(format!(
                "the allocation's kind is {:#x}, not {KIND_RESERVATION:#x} (compressible without post-L2 compression)",
                self.kind
            ));
        }
        if self.surface_attrs & SURFACE_INFO_ATTRS_COMPR == 0 {
            why.push(format!("no compression resources bound (surface ATTRS {:#x})", self.surface_attrs));
        }
        if self.covered() == 0 {
            why.push("no comptag coverage".into());
        }
        if (self.line_min, self.line_max, self.format) != (1, 1, 1) {
            why.push(format!(
                "comptags are not physically based (coverage lineMin/lineMax/format {}/{}/{}, want 1/1/1)",
                self.line_min, self.line_max, self.format
            ));
        }
        if self.base % COMPR_PAGE != 0 || self.size % COMPR_PAGE != 0 {
            why.push(format!(
                "[{:#x}, +{:#x}) is not 64 KiB aligned",
                self.base, self.size
            ));
        }
        if why.is_empty() {
            Ok(())
        } else {
            Err(why.join("; "))
        }
    }

    /// The log line of what the host granted.
    pub fn describe(&self) -> String {
        let (lo, hi) = self.lines();
        format!(
            "compression granted for [{:#x}, {:#x}): attr {:#x} (COMPR {}), attr2 {:#x} (ZBC {}: {}), kind {:#x}, surface ATTRS {:#x}, coverage {} x 64 KiB = {} MiB of {} MiB, coverage lines {}/{} format {} (1/1/1: physically based), 1TO1_COMPTAG {}, COMPRESSION_SIZE {:#x}; the slice's comptag lines [{lo:#x}, {hi:#x}] ({} lines)",
            self.base,
            self.base + self.size,
            self.attr,
            self.attr_compr(),
            self.attr2,
            self.attr2 & ATTR2_ZBC_MASK,
            if self.attr2 & ATTR2_ZBC_MASK == ATTR2_ZBC_PREFER_ZBC {
                "counted as a ZBC surface"
            } else {
                "NOT counted as a ZBC surface"
            },
            self.kind,
            self.surface_attrs,
            self.coverage_64k,
            self.covered() >> 20,
            self.size >> 20,
            self.line_min,
            self.line_max,
            self.format,
            self.one_to_one,
            self.compression_size,
            hi - lo + 1
        )
    }
}

/// `NV_MMU_PTE_KIND_SMSKED_MESSAGE` and `_GENERIC_MEMORY` (turing/tu102/dev_mmu.h).
const SMSKED_MESSAGE: u8 = 0x0F;
const GENERIC_MEMORY: u8 = 0x06;

/// What the filter does with a guest kind on its way into a shadow mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KindUse {
    /// Mapped with this kind as the override.
    Map(u8),
    /// A compressible kind mapped with its uncompressed twin, and why.
    Downgrade(u8, &'static str),
    /// Never mapped.
    Refuse(&'static str),
}

/// One block of `NV0080_CTRL_DMA_GET_PTE_INFO`: the PTE of one page size at a VA
/// (`NV0080_CTRL_DMA_PTE_INFO_PTE_BLOCK`, ctrl0080dma.h:94-100; filled by
/// `kgmmuExtractPteInfo`, kern_gmmu.c:2502-2680).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PteBlock {
    pub page_size: u64,
    pub comptag_line: u32,
    pub kind: u32,
    pub flags: u32,
}

impl PteBlock {
    /// `_PARAMS_FLAGS_VALID` (bit 0).
    pub fn valid(&self) -> bool {
        self.flags & 1 != 0
    }

    /// `_PARAMS_FLAGS_APERTURE` (bits 6:3): 0 video, 1 peer, 2/3 system.
    pub fn aperture(&self) -> u32 {
        (self.flags >> 3) & 0xF
    }
}

/// The parameter block of `DMA_GET_PTE_INFO` for `va` in the VA space `vas`: gpuAddr u64 @0,
/// subDeviceId u32 @8, skipVASpaceInit u8 @12, five 32-byte blocks @16, hVASpace u32 @176.
pub fn pte_info_params(vas: u32, va: u64) -> [u8; PTE_INFO_SIZE] {
    let mut p = [0u8; PTE_INFO_SIZE];
    p[0..8].copy_from_slice(&va.to_le_bytes());
    p[176..180].copy_from_slice(&vas.to_le_bytes());
    p
}

/// The five blocks of an answered `DMA_GET_PTE_INFO` (block i at 16 + 32 i: pageSize u64,
/// pteEntrySize u64, comptagLine u32, kind u32, pteFlags u32).
pub fn parse_pte_info(p: &[u8; PTE_INFO_SIZE]) -> [PteBlock; PTE_INFO_BLOCKS] {
    let u32at = |o: usize| u32::from_le_bytes(p[o..o + 4].try_into().unwrap());
    let u64at = |o: usize| u64::from_le_bytes(p[o..o + 8].try_into().unwrap());
    std::array::from_fn(|i| {
        let b = 16 + 32 * i;
        PteBlock {
            page_size: u64at(b),
            comptag_line: u32at(b + 16),
            kind: u32at(b + 20),
            flags: u32at(b + 24),
        }
    })
}

/// Audit state of [`Policy`]: whether the host answers `DMA_GET_PTE_INFO`.
const AUDIT_UNKNOWN: u8 = 0;
const AUDIT_AVAILABLE: u8 = 1;
const AUDIT_UNAVAILABLE: u8 = 2;

/// The device-wide compression policy the shadow filters by: the covered part of the
/// reservation, where it lies on the host, and the kill switch the PTE audit throws. Shared
/// by every shadow VAS (the server thread plans, the shadow worker maps and audits).
#[derive(Debug)]
pub struct Policy {
    /// The host physical base of the reservation and the covered bytes from it.
    host_base: u64,
    covered: u64,
    lines: (u32, u32),
    /// Set by an audit violation: every compressible leaf is downgraded from then on.
    disabled: AtomicBool,
    /// Bumped whenever the policy changes, so every shadow re-plans its units.
    generation: AtomicU64,
    audit: AtomicU8,
    pub audited: AtomicU64,
    pub violations: AtomicU64,
    pub kind_mismatches: AtomicU64,
    first_violation: Mutex<Option<String>>,
}

impl Policy {
    /// The policy of a grant that passed [`Grant::verdict`].
    pub fn new(g: &Grant) -> Result<Self, String> {
        g.verdict()?;
        Ok(Policy {
            host_base: g.base,
            covered: g.covered(),
            lines: g.lines(),
            disabled: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            audit: AtomicU8::new(AUDIT_UNKNOWN),
            audited: AtomicU64::new(0),
            violations: AtomicU64::new(0),
            kind_mismatches: AtomicU64::new(0),
            first_violation: Mutex::new(None),
        })
    }

    /// The slice's comptag lines, inclusive.
    pub fn lines(&self) -> (u32, u32) {
        self.lines
    }

    pub fn disabled(&self) -> bool {
        self.disabled.load(Ordering::Relaxed)
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// The D16b allowlist for one guest leaf of `size` bytes that resolved to reservation
    /// offset `vid_off` (vidmem) or to guest RAM (`None`). Uncompressed kinds map as they
    /// are. A compressible kind maps as it is only on covered slice pages, 64 KiB aligned,
    /// in pages of at least 64 KiB; otherwise it is downgraded to its uncompressed twin (as
    /// the host RM itself falls back per PTE, virt_mem_allocator_gm107.c:1926-1941): a 4 KiB
    /// compressible map the host widens to the physical page (2 MiB here,
    /// virt_mem_allocator_gm107.c:743-756), sysmem has no compression on Turing.
    pub fn kind_use(&self, kind: u8, size: u64, vid_off: Option<u64>) -> KindUse {
        // SMSKED_MESSAGE (0x0F, the compute object's SM scheduling messages): mapped with
        // GENERIC_MEMORY (0x06), the uncompressed kind CUDA ran with before D16b (the leaf
        // then took the uncompressed reservation's own kind). Without an override it would
        // now take the slice's compressible 0x09.
        if kind == SMSKED_MESSAGE {
            return KindUse::Map(GENERIC_MEMORY);
        }
        if !allowlisted(kind) {
            return KindUse::Refuse("kind outside the Turing allowlist (0x00-0x06, 0x08-0x0E)");
        }
        if !compressible(kind) {
            return KindUse::Map(kind);
        }
        let down = uncompressed(kind).expect("allowlisted");
        let why = match vid_off {
            None => Some("guest RAM (no sysmem compression on Turing)"),
            Some(_) if self.disabled() => Some("compressible maps are stopped (PTE audit violation)"),
            Some(_) if size < COMPR_PAGE => {
                Some("a 4 KiB page (the host widens a compressible 4 KiB map to the physical page)")
            }
            Some(off) if off % COMPR_PAGE != 0 => Some("not 64 KiB aligned in the slice"),
            Some(off) if off.checked_add(size).is_none_or(|e| e > self.covered) => {
                Some("outside the comptag-covered part of the slice")
            }
            Some(_) => None,
        };
        match why {
            None => KindUse::Map(kind),
            Some(w) => KindUse::Downgrade(down, w),
        }
    }

    /// The comptag line the host must write for a compressible PTE of `page` bytes that
    /// maps reservation offset `off`: the line of the page's first 64 KiB.
    pub fn expected_line(&self, off: u64, page: u64) -> u32 {
        comptag_line(self.host_base + (off & !(page.max(COMPR_PAGE) - 1)))
    }

    /// Invariant (a) for one sampled host PTE: the valid block of `page` bytes at a VA the
    /// shadow mapped from reservation offset `off` with override `kind`. A compressible
    /// PTE must carry exactly the line of its own physical page, inside the slice's lines;
    /// an uncompressed video PTE must carry line 0. `Ok(true)`: checked and clean;
    /// `Ok(false)`: clean, but the kind is not the one asked for (the host downgraded it);
    /// `Err`: a violation.
    pub fn check_pte(&self, blocks: &[PteBlock], page: u64, off: u64, kind: u8) -> Result<bool, String> {
        let b = blocks
            .iter()
            .find(|b| b.page_size == page && b.valid())
            .ok_or_else(|| format!("no valid {page:#x}-byte PTE at the mapped VA ({blocks:x?})"))?;
        if b.aperture() != 0 {
            return Err(format!("the PTE is not in video memory (flags {:#x})", b.flags));
        }
        let pte_kind = b.kind as u8;
        if b.kind > 0xFF || !allowlisted(pte_kind) {
            return Err(format!("the PTE carries kind {:#x}, outside the allowlist", b.kind));
        }
        if compressible(pte_kind) {
            let want = self.expected_line(off, page);
            let (lo, hi) = self.lines;
            if b.comptag_line != want || b.comptag_line < lo || b.comptag_line > hi {
                return Err(format!(
                    "compressible kind {:#x} with comptag line {:#x}; the page's own line is {want:#x}, the slice's [{lo:#x}, {hi:#x}]",
                    b.kind, b.comptag_line
                ));
            }
        } else if b.comptag_line != 0 {
            return Err(format!(
                "uncompressed kind {:#x} with comptag line {:#x} (must be 0)",
                b.kind, b.comptag_line
            ));
        }
        Ok(b.kind == u32::from(kind))
    }

    /// Whether the shadow should sample a PTE now: the host answers the control, or has
    /// not been asked yet.
    pub fn audit_wanted(&self) -> bool {
        self.audit.load(Ordering::Relaxed) != AUDIT_UNAVAILABLE
    }

    /// Record an answered sample.
    pub fn audit_answered(&self) {
        self.audit.store(AUDIT_AVAILABLE, Ordering::Relaxed);
        self.audited.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a refused sample: `Some(line)` the first time the host refuses the control
    /// (the audit is off from then on; the filter's construction still holds (a)).
    pub fn audit_refused(&self, status: u32) -> Option<String> {
        let prev = self.audit.swap(AUDIT_UNAVAILABLE, Ordering::Relaxed);
        (prev != AUDIT_UNAVAILABLE).then(|| {
            let why = if status == NV_ERR_TEST_ONLY_CODE_NOT_ENABLED {
                "NV_ERR_TEST_ONLY_CODE_NOT_ENABLED: the control is test-only and the host does not set EnableRmTestOnlyCode"
            } else {
                "refused"
            };
            format!(
                "WARN compression: the host PTE audit (DMA_GET_PTE_INFO) is unavailable, status {status:#x} ({why}); invariant (a) rests on the filter's construction (compressible kinds only on covered slice pages, the host writes 1 + SPA[35:16]) [D16b]"
            )
        })
    }

    /// Record a violation of (a): compressible maps stop and every shadow re-plans (its
    /// compressible leaves are mapped again downgraded). `Some(line)` for the first one.
    pub fn violation(&self, what: &str) -> Option<String> {
        self.violations.fetch_add(1, Ordering::Relaxed);
        let first = !self.disabled.swap(true, Ordering::Relaxed);
        if first {
            self.generation.fetch_add(1, Ordering::Relaxed);
            let line = format!(
                "ERROR compression: host PTE audit violation: {what}; compressible kinds are no longer mapped, every shadow re-plans with its compressible leaves downgraded [D16b invariant (a)]"
            );
            *self.first_violation.lock().unwrap() = Some(line.clone());
            Some(line)
        } else {
            None
        }
    }

    /// A clean sample whose kind is not the one asked for.
    pub fn kind_mismatch(&self) -> bool {
        self.kind_mismatches.fetch_add(1, Ordering::Relaxed) == 0
    }

    /// `compression: lines [lo, hi], N PTEs audited, V violations, ...`.
    pub fn summary(&self) -> String {
        let audit = match self.audit.load(Ordering::Relaxed) {
            AUDIT_AVAILABLE => "available",
            AUDIT_UNAVAILABLE => "unavailable",
            _ => "not asked yet",
        };
        format!(
            "compression: slice lines [{:#x}, {:#x}], covered {} MiB; PTE audit {audit}, {} sampled, {} violation(s), {} kind mismatch(es); compressible maps {}",
            self.lines.0,
            self.lines.1,
            self.covered >> 20,
            self.audited.load(Ordering::Relaxed),
            self.violations.load(Ordering::Relaxed),
            self.kind_mismatches.load(Ordering::Relaxed),
            if self.disabled() { "STOPPED" } else { "on" }
        )
    }
}

/// A grant for `[base, base + size)` as the RTX 2070 host answered the probe (compressed,
/// kind 0x09, fully covered, physically based, 1:1); it passes [`Grant::verdict`].
#[cfg(test)]
pub(crate) fn test_grant(base: u64, size: u64) -> Grant {
    Grant {
        base,
        size,
        attr: 0x1180_1000,
        attr2: 0x10_0006,
        kind: KIND_RESERVATION,
        surface_attrs: SURFACE_INFO_ATTRS_COMPR,
        coverage_64k: (size / COMPR_PAGE) as u32,
        line_min: 1,
        line_max: 1,
        format: 1,
        one_to_one: 1,
        compression_size: 0xf7d0_0000,
        statuses: [0; 3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: u64 = 0x1_8000_0000;
    const SIZE: u64 = 1 << 30;

    fn grant() -> Grant {
        test_grant(BASE, SIZE)
    }

    #[test]
    fn the_switch_is_off_unless_slice() {
        assert_eq!(parse(None), Ok(Mode::Off));
        assert_eq!(parse(Some("off")), Ok(Mode::Off));
        assert_eq!(parse(Some("0")), Ok(Mode::Off));
        assert_eq!(parse(Some("slice")), Ok(Mode::Slice));
        assert!(parse(Some("1")).is_err());
        assert!(parse(Some("on")).is_err());
    }

    #[test]
    fn a_comptag_line_follows_the_physical_64k_page() {
        assert_eq!(comptag_line(0), 1);
        assert_eq!(comptag_line(0xFFFF), 1);
        assert_eq!(comptag_line(0x1_0000), 2);
        assert_eq!(comptag_line(0x1_E918_FFFF), 1 + 0x1_E918);
        // SPA[35:16]: bit 36 and above do not reach the line.
        assert_eq!(comptag_line(0x10_0000_0000), 1);
    }

    #[test]
    fn the_uncompressed_twins_are_the_host_rms() {
        for k in 0..=6u8 {
            assert_eq!(uncompressed(k), Some(k));
        }
        let t = [(0x08, 0x06), (0x09, 0x06), (0x0A, 0x02), (0x0B, 0x01), (0x0C, 0x03), (0x0D, 0x04), (0x0E, 0x05)];
        for (c, u) in t {
            assert_eq!(uncompressed(c), Some(u), "{c:#x}");
        }
        for k in [0x07u8, 0x0F, 0x10, 0xFF] {
            assert_eq!(uncompressed(k), None);
            assert!(!allowlisted(k));
        }
    }

    #[test]
    fn only_a_physically_based_1to1_grant_passes() {
        assert_eq!(grant().verdict(), Ok(()));
        type Spoil = fn(&mut Grant);
        let bad: [(&str, Spoil); 9] = [
            ("ZBC surface", |g| g.attr2 = 0x10_0005),
            ("1:1", |g| g.one_to_one = 0),
            ("COMPR_NONE", |g| g.attr = 0x1180_0000),
            ("kind", |g| g.kind = 0x08),
            ("ATTRS", |g| g.surface_attrs = 0),
            ("no comptag coverage", |g| g.coverage_64k = 0),
            ("physically based", |g| g.line_max = 7),
            ("aligned", |g| g.base += 0x1000),
            ("read-back", |g| g.statuses[1] = 0x1f),
        ];
        for (what, f) in bad {
            let mut g = grant();
            f(&mut g);
            let e = g.verdict().unwrap_err();
            assert!(e.contains(what), "{what}: {e}");
            assert!(Policy::new(&g).is_err());
        }
        let g = grant();
        assert_eq!(g.lines(), (comptag_line(BASE), comptag_line(BASE + SIZE - 1)));
        assert_eq!(g.lines().1 - g.lines().0 + 1, (SIZE / COMPR_PAGE) as u32);
    }

    /// Invariant (a), the filter's side: compressible kinds only on covered, aligned slice
    /// pages of 64 KiB or more; everything else downgraded or refused.
    #[test]
    fn compressible_kinds_map_only_on_covered_slice_pages() {
        let mut g = grant();
        g.coverage_64k = 64; // 4 MiB covered
        let p = Policy::new(&g).unwrap();
        for k in 0..=6u8 {
            assert_eq!(p.kind_use(k, 0x1000, Some(0)), KindUse::Map(k));
            assert_eq!(p.kind_use(k, 0x1000, None), KindUse::Map(k));
        }
        assert_eq!(p.kind_use(0x0C, 0x1_0000, Some(0x1_0000)), KindUse::Map(0x0C));
        assert_eq!(p.kind_use(0x08, 0x20_0000, Some(0x20_0000)), KindUse::Map(0x08));
        let down = |k, size, off| match p.kind_use(k, size, off) {
            KindUse::Downgrade(d, _) => d,
            other => panic!("{k:#x} {size:#x} {off:?}: {other:?}"),
        };
        assert_eq!(down(0x0C, 0x1000, Some(0)), 0x03); // 4 KiB
        assert_eq!(down(0x0E, 0x1_0000, None), 0x05); // sysmem
        assert_eq!(down(0x08, 0x1_0000, Some(0x8000)), 0x06); // unaligned
        assert_eq!(down(0x0A, 0x20_0000, Some(0x30_0000)), 0x02); // past the coverage
        assert_eq!(down(0x0B, 0x1_0000, Some(0x40_0000)), 0x01); // just past
        assert_eq!(p.kind_use(0x0D, 0x1_0000, Some(0x3F_0000)), KindUse::Map(0x0D)); // last page
        for k in [0x07u8, 0x10, 0xFE] {
            assert!(matches!(p.kind_use(k, 0x1_0000, Some(0)), KindUse::Refuse(_)), "{k:#x}");
        }
        // SMSKED_MESSAGE maps uncompressed generic, on slice pages and in guest RAM.
        assert_eq!(p.kind_use(0x0F, 0x1_0000, Some(0)), KindUse::Map(0x06));
        assert_eq!(p.kind_use(0x0F, 0x1000, None), KindUse::Map(0x06));
    }

    fn block(page: u64, line: u32, kind: u32) -> PteBlock {
        PteBlock { page_size: page, comptag_line: line, kind, flags: 1 }
    }

    #[test]
    fn the_audit_accepts_only_the_pages_own_line() {
        let p = Policy::new(&grant()).unwrap();
        let off = 0x23_0000;
        let own = comptag_line(BASE + off);
        let blocks = [block(0x1000, 0, 0), block(0x1_0000, own, 0x0C)];
        assert_eq!(p.check_pte(&blocks, 0x1_0000, off, 0x0C), Ok(true));
        // A 2 MiB PTE carries the line of its first 64 KiB.
        let b2 = [block(0x20_0000, comptag_line(BASE + 0x20_0000), 0x08)];
        assert_eq!(p.check_pte(&b2, 0x20_0000, 0x20_0000, 0x08), Ok(true));
        // Another page's line, a line outside the slice, line 0: violations.
        for line in [own + 1, comptag_line(BASE + SIZE), 0, 1] {
            let b = [block(0x1_0000, line, 0x0C)];
            assert!(p.check_pte(&b, 0x1_0000, off, 0x0C).is_err(), "line {line:#x}");
        }
        // An uncompressed PTE must carry line 0.
        assert_eq!(p.check_pte(&[block(0x1_0000, 0, 0x03)], 0x1_0000, off, 0x03), Ok(true));
        assert!(p.check_pte(&[block(0x1_0000, own, 0x03)], 0x1_0000, off, 0x03).is_err());
        // The host downgraded the kind: clean, but reported.
        assert_eq!(p.check_pte(&[block(0x1_0000, 0, 0x03)], 0x1_0000, off, 0x0C), Ok(false));
        // Not video, not valid, a kind outside the allowlist.
        let mut sys = block(0x1_0000, 0, 0x06);
        sys.flags |= 2 << 3;
        assert!(p.check_pte(&[sys], 0x1_0000, off, 0x06).is_err());
        let mut inval = block(0x1_0000, own, 0x0C);
        inval.flags = 0;
        assert!(p.check_pte(&[inval], 0x1_0000, off, 0x0C).is_err());
        assert!(p.check_pte(&[block(0x1_0000, 0, 0x0F)], 0x1_0000, off, 0x06).is_err());
    }

    #[test]
    fn a_violation_stops_compressible_maps_once() {
        let p = Policy::new(&grant()).unwrap();
        assert_eq!(p.kind_use(0x0C, 0x1_0000, Some(0)), KindUse::Map(0x0C));
        let g0 = p.generation();
        assert!(p.violation("test").is_some());
        assert!(p.violation("again").is_none());
        assert_eq!(p.generation(), g0 + 1);
        assert_eq!(p.violations.load(Ordering::Relaxed), 2);
        assert!(matches!(p.kind_use(0x0C, 0x1_0000, Some(0)), KindUse::Downgrade(0x03, _)));
        assert!(p.summary().contains("STOPPED"));
    }

    #[test]
    fn a_refused_audit_is_reported_once_and_not_asked_again() {
        let p = Policy::new(&grant()).unwrap();
        assert!(p.audit_wanted());
        let l = p.audit_refused(NV_ERR_TEST_ONLY_CODE_NOT_ENABLED).unwrap();
        assert!(l.contains("EnableRmTestOnlyCode"));
        assert!(!p.audit_wanted());
        assert!(p.audit_refused(NV_ERR_TEST_ONLY_CODE_NOT_ENABLED).is_none());
    }

    #[test]
    fn the_pte_info_block_layout() {
        let p = pte_info_params(0xcafe, 0x1_2345_0000);
        assert_eq!(&p[0..8], &0x1_2345_0000u64.to_le_bytes());
        assert_eq!(&p[176..180], &0xcafeu32.to_le_bytes());
        let mut a = [0u8; PTE_INFO_SIZE];
        let b = 16 + 32;
        a[b..b + 8].copy_from_slice(&0x1_0000u64.to_le_bytes());
        a[b + 16..b + 20].copy_from_slice(&0x1_8023u32.to_le_bytes());
        a[b + 20..b + 24].copy_from_slice(&0x0Cu32.to_le_bytes());
        a[b + 24..b + 28].copy_from_slice(&1u32.to_le_bytes());
        let blocks = parse_pte_info(&a);
        assert_eq!(blocks[1], block(0x1_0000, 0x1_8023, 0x0C));
        assert_eq!(blocks[0], PteBlock::default());
    }
}

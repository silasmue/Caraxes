// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Execution on the physical GPU (E5b part 2, docs/caraxes/execution.md): for each guest
//! channel that can run on the host, a **mirror** of the guest's RM objects on the
//! device's own host RM client, wired as docs/caraxes/host-exec.md §2 specifies.
//!
//! - per guest VA space a host `FERMI_VASPACE_A`, externally owned, whose page directory
//!   is the guest's root (a real VRAM address by identity, memory-model.md §1) through
//!   `DMA_SET_PAGE_DIRECTORY` (PRIVILEGED; CAP_SYS_ADMIN is raised around that one call);
//! - per guest TSG a host `KEPLER_CHANNEL_GROUP_A` on that VA space;
//! - per guest channel a host `TURING_CHANNEL_GPFIFO_A` with the guest's GPFIFO VA and
//!   entry count, the guest's USERD (a vidmem USERD is the reservation at `PA - base`, a
//!   sysmem USERD an OS descriptor over that guest page), the guest's error notifier page
//!   as the channel's error notifier, then the engine objects the guest allocated under
//!   the channel (`TURING_DMA_COPY_A`, `TURING_COMPUTE_A`, ...) with the guest's own
//!   parameters (the guest's engine numbering is the host's: the CE caps were forwarded),
//!   `GPFIFO_SCHEDULE`, and the work-submit token;
//! - the guest's doorbell write becomes a write of the host token to the host doorbell
//!   page; the GPFIFO is never interpreted for a mirrored channel;
//! - the guest kernel's TLB invalidates (`NV_PFB_PRI_MMU_INVALIDATE` by register) become
//!   `DMA_INVALIDATE_TLB` on the mirror VA space with that page directory;
//! - host OS events with `NV01_EVENT_NONSTALL_INTR` on the GR0 and CE2..CE4 notifiers
//!   raise the guest's non-stall vectors of those engines (`chips/<chip>/interrupts.toml`)
//!   and the MSI from an event thread; the RC notifier is watched and counted.
//!
//! Everything is created lazily at the first doorbell of a channel, when the guest has
//! finished what the host needs first (host-exec.md §1: the page directory before the
//! channel, an engine object before scheduling). Channels that are not children of a
//! TSG (the kernel's golden-context and WAR channels at module load) stay with the
//! software executor of `channel.rs`. `GPU_PROMOTE_CTX` entries stay recorded (variant
//! c1 of cuda-path.md §4.1): the host RM builds the GR context of a mirror channel from
//! its own golden image.
//!
//! Provenance: every host object is `host` (created here on the guest's behalf); every
//! address handed to the host is the guest's, unchanged (`guest`), after the check that
//! it lies inside the reservation.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use nvrm_abi::sys;
use nvrm_client::mem::CpuMap;

use crate::channel::{Channel, GpuMem, Registry};
use crate::chip::ChipData;
use crate::fb::{FakeFb, FbLayout};
use crate::fb::Target;
use crate::guestmem::GuestMemory;
use crate::hostmem::{HostMemory, HostUserd, OsEvent};
use crate::intr::IntrShared;
use crate::invalidate::Invalidator;
use crate::ver::common as v;

/// `NV2080_NOTIFIERS_*` (class/cl2080_notification.h): the engine notifiers the mirrored
/// channels run on, and the RC notifier.
pub const NOTIFIER_GRAPHICS: u32 = 12;
pub const NOTIFIER_CE2: u32 = 25;
pub const NOTIFIER_CE3: u32 = 26;
pub const NOTIFIER_CE4: u32 = 27;
pub const NOTIFIER_RC_ERROR: u32 = 37;
/// `NV2080_NOTIFIERS_NVDEC0` (= `_VLD` 14) and `NV2080_NOTIFIERS_NVENC0` (= `_MSENC` 38),
/// cl2080_notification.h:50-51, :75-76.
pub const NOTIFIER_NVDEC0: u32 = 14;
pub const NOTIFIER_NVENC0: u32 = 38;
/// `NV01_EVENT_NONSTALL_INTR` (nvos.h:435).
pub const EVENT_NONSTALL: u32 = 0x0800_0000;
/// The engines whose non-stall interrupts are forwarded: (MC_ENGINE_IDX name, notifier).
pub const ENGINES: &[(&str, u32)] = &[
    ("GR0", NOTIFIER_GRAPHICS),
    ("CE2", NOTIFIER_CE2),
    ("CE3", NOTIFIER_CE3),
    ("CE4", NOTIFIER_CE4),
    // The video engines (docs/caraxes/video-engines.md): the kernel RM services their
    // non-stall interrupts itself (gkflcnRegisterIntrService, kernel_falcon.c:362-400) and
    // notifies these subdevice notifiers, on the host as in the guest.
    ("NVDEC0", NOTIFIER_NVDEC0),
    ("NVENC0", NOTIFIER_NVENC0),
];

/// The guest non-stall vector of the forwarded engine `name` (an `MC_ENGINE_IDX` name of
/// [`ENGINES`]) in the chip's interrupt table, if it has one.
fn engine_vector(chip: &ChipData, name: &str) -> Option<u32> {
    let mc = v::lookup(v::MC_ENGINE_IDX, name)?;
    chip.interrupts
        .entries
        .iter()
        .find(|e| e.mc_idx as u32 == mc)
        .map(|e| e.vector_nonstall)
        .filter(|&vector| vector != 0)
}

/// The guest vectors the host OS events of [`ENGINES`] raise: notifications of the host
/// engines, which run every client of the host GPU (intr_page.rs `set_hint_vectors`).
pub fn hint_vectors(chip: &ChipData) -> Vec<u32> {
    ENGINES.iter().filter_map(|(name, _)| engine_vector(chip, name)).collect()
}

/// The video engine an `NV2080_ENGINE_TYPE` names (`NVENC0`.., `NVDEC0`..), if any.
pub fn video_engine(engine_type: u32) -> Option<&'static str> {
    v::NV2080_ENGINE_TYPES
        .iter()
        .find(|(n, t)| *t == engine_type && (n.starts_with("NVENC") || n.starts_with("NVDEC")))
        .map(|(n, _)| *n)
}

/// The chip's non-stall vector for an `NV2080_ENGINE_TYPE_*`. Copy-engine type names use
/// `COPYn`, while the MC interrupt table names the same engines `CEn`.
fn nonstall_vector(chip: &ChipData, engine_type: u32) -> Option<u32> {
    let engine = v::NV2080_ENGINE_TYPES
        .iter()
        .find(|(_, value)| *value == engine_type)?
        .0;
    let mc_name = engine
        .strip_prefix("COPY")
        .map_or_else(|| engine.to_string(), |n| format!("CE{n}"));
    let mc = v::lookup(v::MC_ENGINE_IDX, &mc_name)?;
    chip.interrupts
        .entries
        .iter()
        .find(|entry| entry.mc_idx as u32 == mc)
        .map(|entry| entry.vector_nonstall)
        .filter(|vector| *vector != 0)
}

/// Whether `class` is a video class the mirror serves on `chip`: an NVDEC (`..B0`) or
/// NVENC (`..B7`) class of the chip's class database (TU106: NVC4B0, NVC4B7).
pub fn video_class_ok(chip: &ChipData, class: u32) -> bool {
    matches!(class & 0xFF, 0xB0 | 0xB7)
        && chip.classes.as_ref().is_some_and(|db| db.ids.contains(&class))
}

/// `NV50_DEFERRED_API_CLASS`: the software object the Windows KMD allocates on every channel.
/// Its deferred calls run only on the KMD's management channel, which the device executes
/// itself (ctrl/deferred_context.rs); on any other channel it is an inert object, which the
/// host channel gets too (as a GR channel's) so that the guest's SET_OBJECT of it resolves.
pub const DEFERRED_API_CLASS: u32 = 0x5080;

/// The first engine object of a video channel that is neither a video class of `chip` nor
/// the KMD's deferred-API object (fail closed: such a channel stays in software). Run
/// P29b: Windows' NVENC/NVDEC channels carry 0x5080 beside NVC4B7/NVC4B0; refusing them
/// left every encode and decode session in software (ffmpeg EINVAL, then a TDR).
pub fn video_channel_foreign_class(chip: &ChipData, classes: &[(u32, Vec<u8>)]) -> Option<u32> {
    classes
        .iter()
        .map(|(c, _)| *c)
        .find(|c| *c != DEFERRED_API_CLASS && !video_class_ok(chip, *c))
}

/// The engine a video channel with no video object yet is bound to at its build
/// (`gr_shadow_doorbell`): its own engine type, when that is a video engine.
pub fn video_bind(chip: &ChipData, ch: &Channel) -> Option<u32> {
    video_engine(ch.engine_type)?;
    (!ch.engine_objects.iter().any(|(c, _)| video_class_ok(chip, *c))).then_some(ch.engine_type)
}

/// The engine a copy channel with no engine object yet is bound to at its build: its own
/// engine type, when that is a copy engine (`NV2080_ENGINE_TYPE_COPY0..9` 0x09-0x12,
/// `COPY10..19` 0x34-0x3D). The NVENC library's bare CE channel rings before its copy
/// object exists, as a video channel does; without a bind its build was abandoned ("no
/// engine object; not scheduling"), its first push never ran, and `nvEncInitializeEncoder`
/// gave up after 10 s with NV_ENC_ERR_OUT_OF_MEMORY (vm4-615 with
/// `CARAXES_MIRROR_BARE_CE=1`, 2026-09-25).
pub fn copy_bind(ch: &Channel) -> Option<u32> {
    let t = ch.engine_type;
    ((0x09..=0x12).contains(&t) || (0x34..=0x3D).contains(&t)).then_some(t)
}

/// The allocation flags of every guest channel's host mirror:
/// `NVOS04_FLAGS_CHANNEL_DENY_PHYSICAL_MODE_CE` (bit 7). With it the host RM makes the
/// channel unprivileged whatever the host client's privilege (alloc_channel.h:159-170), so
/// a physical-mode CE method in the guest's push faults that channel instead of reaching
/// host memory. Not for the device's own invalidator, which needs ADMIN (invalidate.rs).
const HOST_MIRROR_CHANNEL_FLAGS: u32 = 1 << 7;

/// A channel which must not inherit the unprivileged host mirror client's execution
/// context: a privileged non-GR channel (`NVOS04_FLAGS_PRIVILEGED_CHANNEL`, bit 5) may
/// rely on physical-mode work, which its host mirror would refuse, so it remains in the
/// bounded software executor. A bare COPY channel without bit 7 is mirrored: its host
/// channel denies physical-mode CE itself ([`HOST_MIRROR_CHANNEL_FLAGS`]); the Vulkan
/// driver's X11 present channel is one (SotTR, vkcube on XWayland, 2026-09-28).
fn needs_software_physical_ce(alloc_flags: u32, is_gr: bool) -> bool {
    let privileged = alloc_flags & (1 << 5) != 0;
    privileged && !is_gr
}

/// Whether a guest channel stays in the software executor for its privilege: a privileged
/// non-GR copy channel that is not an application's ([`application_copy_channel`]). A video
/// channel never does: NVENC/NVDEC have no physical mode, and the RM sets
/// PRIVILEGED_CHANNEL on every channel a kernel-level client allocates (kernel_channel.c
/// 273-283), so on Windows the bit says nothing about the KMD's intent (run P31: the KMD's own
/// NVENC channel ran in software, hit an NVENC method the CE model lacks, Xid 45 and a TDR).
/// A privileged-only method on a mirrored video channel faults that channel only.
fn privileged_stays_in_software(reg: &Registry, ch: &Channel, gr0: u32, is_gr: bool) -> bool {
    needs_software_physical_ce(ch.alloc_flags, is_gr)
        && video_engine(ch.engine_type).is_none()
        && !application_copy_channel(reg, ch, gr0)
}

/// `NVOS04_FLAGS_SKIP_CTXBUFFER_ALLOC` (bit 31): the Windows KMD allocates an application's GR
/// context itself (the deferred context, ctrl/deferred_context.rs).
const SKIP_CTXBUFFER_ALLOC: u32 = 1 << 31;

/// Whether a privileged non-GR channel is an application's copy (or video) channel: its VA
/// space is also that of an application GR context, a skip-context GR channel on the same
/// root. The Windows KMD makes every channel privileged; CUDA's COPY TSG channels share the
/// CUDA context's address space, the KMD's own paging and scrub channels (the physical-mode
/// page-table writers) run in the system address space with only the KMD's management GR
/// channel. Such a channel is mirrored like any other, unprivileged: its host channel
/// denies physical-mode CE ([`HOST_MIRROR_CHANNEL_FLAGS`]), so a physical-mode method there
/// faults that channel only (run P29: CUDA's copies in software ran at 10-200 MB/s and a
/// 64 MiB read-back held the device 6.7 s). Linux never sets bit 31: unchanged there.
pub fn application_copy_channel(reg: &Registry, ch: &Channel, gr0: u32) -> bool {
    let Some(root) = reg.vas_key_of(ch).and_then(|k| reg.vas_root(k)) else {
        return false;
    };
    reg.channels().any(|c| {
        c.chid != ch.chid
            && c.engine_type == gr0
            && c.alloc_flags & SKIP_CTXBUFFER_ALLOC != 0
            && reg.vas_key_of(c).and_then(|k| reg.vas_root(k)) == Some(root)
    })
}

/// `NVA06F_CTRL_CMD_BIND` {engineType} (ctrla06f.h): a host channel needs an engine before
/// `GPFIFO_SCHEDULE` (docs/caraxes/host-exec.md §1: NV_ERR_INVALID_STATE otherwise).
const CMD_CHANNEL_BIND: u32 = 0xA06F_0104;

/// The promoted context ranges `[va, va + size)` of a channel (GR set B, a falcon
/// engine's context), which the shadow keeps out of the guest's set A. A global GR
/// buffer promoted by VA alone has size 0; `global_size(buffer id, VA)` gives its extent
/// at that VA ([`global_exclusion`]), `Some(0)` none (the host's own copy is elsewhere).
fn promoted_ranges(
    reg: &Registry,
    ch: &Channel,
    global_size: impl Fn(u16, u64) -> Option<u64>,
) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    if let Some(m) = reg.promoted((ch.h_client, ch.h_object)) {
        for e in m.values() {
            if let Some(v) = e.virt {
                let size = if e.size == 0 {
                    match global_size(e.buffer_id, v) {
                        // The host's own buffer is not at this VA (a fenced shadow, F2b).
                        Some(0) => continue,
                        Some(s) => s,
                        None => 0x1000,
                    }
                } else {
                    e.size
                };
                out.push((v, v + size.max(0x1000)));
            }
        }
    }
    out
}

/// The host's first fit (the probe before the object), when it lies in the host-only area.
pub fn host_first_fit(layout: &crate::shadow::Layout, before: Result<u64, u32>) -> Result<u64, String> {
    let (lo, hi) = layout.host_area;
    let b = before.map_err(|st| format!("the placement probe before the object was refused {st:#x}"))?;
    if !layout.in_host_area(b) {
        return Err(format!(
            "the host's first fit is {b:#x}, outside the host-only area [{lo:#x}, {hi:#x}) (a VA the guest can map)"
        ));
    }
    Ok(b)
}

/// The verdict of the two first-fit probes around a host video object's allocation
/// ([`Mirror::place_video_object`]): `before` is where a 4 KiB non-fixed allocation went
/// just before the object, `after` just after it. The host RM maps the object's context
/// first fit, so the context took `before` exactly when `after` moved on. Kept only when
/// that VA lies in the layout's host-only area, outside everything the guest can map.
pub fn context_verdict(
    layout: &crate::shadow::Layout,
    before: Result<u64, u32>,
    after: Result<u64, u32>,
) -> Result<String, String> {
    let (lo, hi) = layout.host_area;
    let b = host_first_fit(layout, before)?;
    match after {
        Ok(a) if a == b => Err(format!(
            "the host did not map its context at its first fit {b:#x} (still free after the object), so its VA is unknown"
        )),
        Ok(a) => Ok(format!(
            "its context at {b:#x}, in the host-only area [{lo:#x}, {hi:#x}) (first fit {a:#x} after it)"
        )),
        Err(st) => Err(format!(
            "the placement probe after the object was refused {st:#x}, so the host context's VA is unknown"
        )),
    }
}

/// The host RM's VA footprint of a global GR buffer of `raw` bytes: RM maps an internal
/// pitch vidmem allocation with 4 KiB pages below 64 KiB, 64 KiB pages below 2 MiB and 2 MiB
/// pages above (mem_mgr.c `_memmgrPickDefaultGpuPageSize`), so this is `raw` rounded up to
/// that page. The guest's RM, the same code, reserves the same span (run P19: the guest's
/// BUNDLE_CB of 0x3000 at 0x1200e9000, its next page at 0x1200ec000).
pub fn host_footprint(raw: u64) -> u64 {
    let page = if raw < 1 << 16 {
        1 << 12
    } else if raw < 1 << 21 {
        1 << 16
    } else {
        1 << 21
    };
    raw.div_ceil(page) * page
}

/// How a size-0 global buffer's exclusion ended ([`global_exclusion`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalClip {
    /// No guest page past the host's footprint within the 64 KiB extent: the full extent.
    Full,
    /// A guest page at `at`, past the host's footprint: excluded up to it.
    Cut { at: u64 },
}

/// The exclusion `[va, end)` of a global GR buffer promoted by VA alone (size 0), with the
/// host's size `raw`: at least the host's footprint ([`host_footprint`]), at most `raw`
/// rounded up to 64 KiB (the extent since Display E, d5987e97), and cut at the first page
/// the guest maps past the footprint (`guest_first(lo, hi)`: the lowest guest-mapped VA in
/// `[lo, hi)`). Run P19: the 64 KiB extent of the 12 KiB BUNDLE_CB covered the guest's
/// semaphore page at +0x3000, and the GR engine's write there faulted (Xid 31 FAULT_PTE).
/// Pages inside the footprint are the guest's own copy of the buffer (its RM allocates and
/// maps the global buffers itself, run P20), which the host's replaces in the shadow.
pub fn global_exclusion(
    va: u64,
    raw: u64,
    guest_first: impl Fn(u64, u64) -> Option<u64>,
) -> (u64, GlobalClip) {
    let floor = va + host_footprint(raw);
    let cap = floor.max(va + ((raw + 0xFFFF) & !0xFFFF));
    match guest_first(floor, cap) {
        Some(at) => (at, GlobalClip::Cut { at }),
        None => (cap, GlobalClip::Full),
    }
}

/// The set-B ranges the shadow of guest root `root` keeps out of set A: the promoted
/// contexts of every channel the guest still has on that root, sorted and deduplicated.
/// Recomputed, never accumulated: a context whose channel the guest freed gives its VAs
/// back to set A. Before (2026-09-25, vm4-615) the exclusions of freed GR channels stayed:
/// GNOME recreated its GL contexts, the guest reused the old attribute-CB VA 0x121400000 for
/// ordinary buffers, the shadow still left it unmapped, and a copy engine read there
/// faulted (Xid 31 FAULT_PDE on CE0) with the stream going black. A freed channel whose
/// host TSG still stands keeps its ranges out until that host free ([`held_at_free`]).
pub fn exclusions_of(
    reg: &Registry,
    root: u64,
    global_size: impl Fn(u16, u64) -> Option<u64>,
) -> Vec<(u64, u64)> {
    exclusions_among(
        reg,
        |ch| reg.vas_key_of(ch).and_then(|k| reg.vas_root(k)) == Some(Target::Fb(root)),
        global_size,
    )
}

/// [`exclusions_of`] over the channels `on_root` selects.
fn exclusions_among(
    reg: &Registry,
    on_root: impl Fn(&Channel) -> bool,
    global_size: impl Fn(u16, u64) -> Option<u64>,
) -> Vec<(u64, u64)> {
    let mut out: Vec<(u64, u64)> = reg
        .channels()
        .filter(|ch| on_root(ch))
        .flat_map(|ch| promoted_ranges(reg, ch, &global_size))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The set-B ranges of a guest channel the guest is freeing, on shadow root `root`,
/// with the host TSG key that stands for its GR context ([`held_at_free`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldSetB {
    pub chid: u32,
    pub root: u64,
    pub key: TsgKey,
    pub ranges: Vec<(u64, u64)>,
}

/// The promoted ranges of the guest channels `freed` selects, taken while the registry
/// still knows them: the shadow keeps them out of set A until the host TSG under `key`
/// (the owner of the host's GR context at those VAs) is freed too. Before (2026-09-25,
/// vm4-615 18:12:42 and 20:21:50), the recompute at the next invalidate released them
/// the moment the guest freed its GR channels, the shadow mapped the guest's pages at
/// 0x120000000 and up with fixed addresses, and the host RM, whose GR context of the
/// still-live host TSG sat at the same VAs, refused all 23 (virtmemAllocResources
/// NO_MEMORY). `root_of` names the shadow root of a channel (`None`: no shadow);
/// `global_size(root, buffer id, VA)` the extent of a global buffer there.
pub fn held_at_free(
    reg: &Registry,
    freed: impl Fn(&Channel) -> bool,
    root_of: impl Fn(&Channel) -> Option<u64>,
    global_size: impl Fn(u64, u16, u64) -> Option<u64>,
) -> Vec<HeldSetB> {
    reg.channels()
        .filter(|ch| freed(ch))
        .filter_map(|ch| {
            let root = root_of(ch)?;
            let ranges = promoted_ranges(reg, ch, |id, va| global_size(root, id, va));
            (!ranges.is_empty()).then(|| HeldSetB {
                chid: ch.chid,
                root,
                key: tsg_key_of(reg, ch).0,
                ranges,
            })
        })
        .collect()
}

/// Set B of a shadow root: the live channels' exclusions ([`exclusions_of`]) and the
/// ranges held for host TSGs not yet freed ([`held_at_free`]), sorted and deduplicated.
pub fn set_b_with_held(
    live: Vec<(u64, u64)>,
    held: &BTreeMap<TsgKey, Vec<(u64, u64)>>,
) -> Vec<(u64, u64)> {
    let mut out = live;
    out.extend(held.values().flatten().copied());
    out.sort_unstable();
    out.dedup();
    out
}

/// The lowest VA in `[lo, hi)` the guest's cached walk of `root` maps (a leaf overlapping
/// it), `None` without a cached walk or without such a leaf.
fn guest_first_mapped(fb: &FakeFb, root: u64, lo: u64, hi: u64) -> Option<u64> {
    if lo >= hi {
        return None;
    }
    fb.with_walk(Target::Fb(root), |c| {
        let mut best: Option<u64> = None;
        let mut unit = lo & !(crate::ptcache::UNIT - 1);
        while unit < hi {
            for &(va, _, size, _) in c.unit_leaves(unit) {
                if va < hi && va.saturating_add(size) > lo {
                    let at = va.max(lo);
                    best = Some(best.map_or(at, |b| b.min(at)));
                }
            }
            let Some(next) = unit.checked_add(crate::ptcache::UNIT) else {
                break;
            };
            unit = next;
        }
        best
    })
    .flatten()
}

fn guest_span_coverage(fb: &FakeFb, root: u64, start: u64, end: u64) -> String {
    let leaves = fb.with_walk(Target::Fb(root), |c| {
        let mut out = Vec::new();
        let mut unit = start & !(crate::ptcache::UNIT - 1);
        while unit < end {
            out.extend_from_slice(c.unit_leaves(unit));
            if end - unit <= crate::ptcache::UNIT {
                break;
            }
            let Some(next) = unit.checked_add(crate::ptcache::UNIT) else { break };
            unit = next;
        }
        out
    });
    let Some(leaves) = leaves else { return "no cached guest walk".into() };
    let mut at = start;
    let mut used = 0usize;
    let mut first = None;
    let mut last = None;
    while at < end {
        let Some((va, target, size, kind)) = leaves
            .iter()
            .copied()
            .find(|(va, _, size, _)| *va <= at && at < va.saturating_add(*size))
        else {
            return format!("gap at {at:#x} after {used} leaf/leaves");
        };
        let next = va.saturating_add(size);
        if next <= at {
            return format!("invalid leaf at {va:#x} -> {target:?} size {size:#x} kind {kind:#x}");
        }
        first.get_or_insert((va, target, size, kind));
        last = Some((va, target, size, kind));
        used += 1;
        at = next.min(end);
    }
    format!(
        "covered [{start:#x}, {end:#x}) by {used} leaf/leaves; first {first:?}; last {last:?}"
    )
}

/// `NV906F_CTRL_GET_MMU_FAULT_INFO_PARAMS` (ctrl906f.h:208-214): the fault VA {addrHi @0,
/// addrLo @4}, faultType @8 and faultString[32] @12.
fn decode_fault_info(p: &[u8]) -> (u64, u32, String) {
    let word = |o: usize| {
        p.get(o..o + 4)
            .map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()))
    };
    let text = p.get(12..44).map_or(String::new(), |b| {
        let end = b.iter().position(|c| *c == 0).unwrap_or(b.len());
        String::from_utf8_lossy(&b[..end]).into_owned()
    });
    (
        (u64::from(word(0)) << 32) | u64::from(word(4)),
        word(8),
        text,
    )
}

/// Whether a `GET_MMU_FAULT_INFO` answer `(status, record)` holds no fault: a failed read,
/// or a record with neither a fault address nor a fault string.
fn fault_record_empty(st: u32, p: &[u8]) -> bool {
    if st != 0 {
        return true;
    }
    let (va, _, text) = decode_fault_info(p);
    va == 0 && text.is_empty()
}

/// An RC'd channel's MMU fault record ([`Mirror::mmu_fault_info_in_tsg`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TsgFault {
    /// The status of the read on the channel's own mirror.
    pub own_status: u32,
    /// The ChID whose record this is (the channel's own when no TSG member held one).
    pub from: u32,
    pub status: u32,
    pub record: Vec<u8>,
}

/// The first of `own` (the answer for `chid`) and the `peers`' answers that holds a fault;
/// `own` when none does. The peers are read only while needed.
fn first_fault(
    chid: u32,
    own: (u32, Vec<u8>),
    peers: impl Iterator<Item = (u32, (u32, Vec<u8>))>,
) -> TsgFault {
    let own_status = own.0;
    if !fault_record_empty(own.0, &own.1) {
        return TsgFault { own_status, from: chid, status: own.0, record: own.1 };
    }
    for (c, (st, p)) in peers {
        if !fault_record_empty(st, &p) {
            return TsgFault { own_status, from: c, status: st, record: p };
        }
    }
    TsgFault { own_status, from: chid, status: own.0, record: own.1 }
}

/// `NV_VASPACE_ALLOCATION_FLAGS_IS_EXTERNALLY_OWNED` (nvos.h:3162).
const VAS_IS_EXTERNALLY_OWNED: u32 = 1 << 3;
/// The VA size of a mirror VA space: 1 TiB, as host-exec.rs (the root covers 2^47 anyway).
const VAS_SIZE: u64 = 1 << 40;
const BIG_PAGE_64K: u32 = 64 * 1024;
/// `NV0080_CTRL_DMA_SET_PAGE_DIRECTORY_FLAGS_ALL_CHANNELS_TRUE` (ctrl0080dma.h).
const SETPD_ALL_CHANNELS: u32 = 1 << 3;
const SETPD_SIZE: usize = 32;
const CMD_GPFIFO_SCHEDULE: u32 = 0xA06F_0103;
const CMD_GET_WORK_SUBMIT_TOKEN: u32 = 0xC36F_0108;
/// `NV0080_CTRL_CMD_DMA_INVALIDATE_TLB` (ctrl0080dma.h:484), NON_PRIVILEGED (0x10008):
/// `{hVASpace, flags}`.
const CMD_DMA_INVALIDATE_TLB: u32 = 0x0080_180C;
/// `NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS` and `NV0080_CTRL_FIFO_IDLE_CHANNELS_PARAMS`
/// {numChannels @0, hChannels\[4096\] @4, flags @16388, timeout @16392}, 16396 bytes
/// (ctrl0080fifo.h:391-401, the same in 610 and 615).
pub const CMD_FIFO_IDLE_CHANNELS: u32 = 0x0080_1714;
pub const IDLE_PARAMS_SIZE: usize = 16396;
pub const IDLE_MAX_CHANNELS: usize = 4096;
pub const IDLE_PARAMS_FLAGS: usize = 4 + 4 * IDLE_MAX_CHANNELS;
pub const IDLE_PARAMS_TIMEOUT: usize = IDLE_PARAMS_FLAGS + 4;
/// `NV2080_CTRL_CMD_FIFO_DISABLE_CHANNELS` and its params {bDisable @0, numChannels @4,
/// bOnlyDisableScheduling @8, bRewindGpPut @9, pRunlistPreemptEvent @16,
/// hClientList\[64\] @24, hChannelList\[64\] @280}, 536 bytes (ctrl2080fifo.h:235-251).
pub const CMD_FIFO_DISABLE_CHANNELS: u32 = 0x2080_110B;
pub const DISABLE_PARAMS_SIZE: usize = 536;
pub const DISABLE_MAX_ENTRIES: usize = 64;
pub const DISABLE_CLIENT_LIST: usize = 24;
pub const DISABLE_CHANNEL_LIST: usize = DISABLE_CLIENT_LIST + 4 * DISABLE_MAX_ENTRIES;
/// `NV2080_CTRL_CMD_GR_GET_CURRENT_RESIDENT_CHANNEL` (ctrl2080gr.h:1246).
pub const CMD_GR_GET_CURRENT_RESIDENT_CHANNEL: u32 = 0x2080_121C;
/// `NV906F_CTRL_CMD_GET_MMU_FAULT_INFO`, 104 bytes (ctrl906f.h:175-220).
pub const CMD_GET_MMU_FAULT_INFO: u32 = 0x906F_0106;
pub const MMU_FAULT_INFO_SIZE: usize = 104;
/// `NVC361_NOTIFY_CHANNEL_PENDING` inside the usermode page (clc361.h:33).
const DOORBELL_OFFSET: usize = 0x90;
/// `NV90F1_CTRL_VASPACE_COPY_SERVER_RESERVED_PDES_PARAMS`: hSubDevice @0, subDeviceId @4
/// (ctrl90f1.h:65-72); the rest is the guest's (`ver::PDES_*`).
const PDES_H_SUBDEVICE: usize = 0;
const PDES_SUBDEVICE_ID: usize = 4;
/// Levels handed to the host: root, PD2, PD1 (not the guest's PD0 of the reserved range).
const PDES_LEVELS_HANDED_OVER: usize = 3;
const NV_ERR_INVALID_ADDRESS: u32 = 0x1E;
const PAGE: u64 = 4096;
/// USERD offsets of GPGet and GPPut (`clc46f.h:61-62`).
const USERD_GP_GET: u64 = 0x88;
const USERD_GP_PUT: u64 = 0x8C;
const CMD_BIND_CTXSW_PREEMPTION_MODE: u32 = 0x2080_1211;
const CMD_GET_CTXSW_PREEMPTION_MODE: u32 = 0x2080_1235;
/// The host's 3D class (the executing chip is a Turing, board.rs `host_class_of`): a host TSG
/// with an object of it gets owned GFXP at its build.
const HOST_3D_CLASS: u32 = 0xC597;

/// One buffer of an owned-GFXP set: a fixed VA of the slot window, mapped through the
/// reservation's handle at the slot's offset (gfxpslot.rs).
struct OwnedGfxpBuffer {
    virtual_handle: Option<u32>,
    va: u64,
    len: u64,
    mapped: bool,
}

/// The owned-GFXP set of one host TSG: its slot in the VM's slice, bound at the build.
struct OwnedGfxp {
    slot: usize,
    bind_attempted: bool,
    bound: bool,
    buffers: Vec<OwnedGfxpBuffer>,
}

/// Unmap and free what `owned` holds, newest first, and give its slot back. A step that
/// fails keeps the set (fail closed); returns the error.
fn release_gfxp(
    h: &mut HostMemory,
    slots: Option<&mut crate::gfxpslot::Slots>,
    reservation: u32,
    owned: &mut OwnedGfxp,
) -> Result<(), String> {
    for b in owned.buffers.iter_mut().rev() {
        if b.mapped {
            let st = h.unmap_dma(b.virtual_handle.unwrap(), reservation, b.va, b.len, false);
            if st != sys::NV_OK {
                return Err(format!(
                    "unmapping GFXP VA {:#x}: {}",
                    b.va,
                    status_name(st)
                ));
            }
            b.mapped = false;
        }
        if let Some(vh) = b.virtual_handle {
            h.free_object(vh)
                .map_err(|st| format!("freeing GFXP virtual {vh:#x}: {}", status_name(st)))?;
            b.virtual_handle = None;
        }
    }
    owned.buffers.clear();
    if let Some(s) = slots {
        s.free(owned.slot);
    }
    Ok(())
}

/// Bind owned GFXP to host TSG `tsg` at its build, before its first schedule: slot `slot` of
/// the VM's slice (reserved by the caller), its five `buffers` mapped at the slot's fixed VAs
/// in VA space `vas` through the reservation's handle, then `BIND {GFXP_SET, gfxp 1}` on the
/// TSG and a GET on `channel` that must show 1. On an error what was made is released again
/// (or kept, if that fails); an `Err` without a set leaves the slot to the caller to free.
/// Runs on the shadow worker (execution.md §9.2), which cannot touch the slot map.
#[allow(clippy::too_many_arguments)]
fn attach_gfxp_slot(
    h: &mut HostMemory,
    slot: usize,
    buffers: &[(u64, u64, u64); 5],
    vas: u32,
    tsg: u32,
    channel: u32,
    reservation: u32,
    guest_base: u64,
) -> Result<OwnedGfxp, (Option<OwnedGfxp>, String)> {
    let mut owned = OwnedGfxp {
        slot,
        bind_attempted: false,
        bound: false,
        buffers: Vec::new(),
    };
    let result = (|| -> Result<(), String> {
        let mut addresses = [0u64; 5];
        for (k, &(pa, va, len)) in buffers.iter().enumerate() {
            if len == 0 {
                continue;
            }
            let vh = h
                .alloc_gfxp_virtual(vas, va, len)
                .map_err(|st| format!("GFXP VA {va:#x}: {}", status_name(st)))?;
            owned.buffers.push(OwnedGfxpBuffer {
                virtual_handle: Some(vh),
                va,
                len,
                mapped: false,
            });
            let st = h.map_dma_fixed(
                vh,
                reservation,
                pa - guest_base,
                len,
                va,
                nvrm_abi::nvgpu::nvos46_flags::PAGE_SIZE_4KB,
                false,
                Some(0x06),
                false,
            );
            if st != sys::NV_OK {
                return Err(format!(
                    "GFXP map of slot {slot} buffer {:#x}: {}",
                    crate::gfxpslot::ENGINES[k],
                    status_name(st)
                ));
            }
            owned.buffers.last_mut().unwrap().mapped = true;
            addresses[k] = va;
        }
        owned.bind_attempted = true;
        let st = bind_gfxp(h, tsg, 1, &addresses);
        let (get, mode) = get_gfxp_mode(h, channel);
        if st != sys::NV_OK || get != sys::NV_OK || mode != 1 {
            return Err(format!(
                "GFXP BIND/GET: bind {}, get {}, mode {mode:#x}",
                status_name(st),
                status_name(get)
            ));
        }
        owned.bound = true;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(owned),
        Err(why) => {
            // The context is not scheduled yet: back to WFI, then release.
            if owned.bind_attempted {
                let _ = bind_gfxp(h, tsg, 0, &[0; 5]);
            }
            match release_gfxp(h, None, reservation, &mut owned) {
                Ok(()) => Err((None, why)),
                Err(e) => Err((Some(owned), format!("{why}; release: {e}"))),
            }
        }
    }
}

fn get_preemption_modes(host: &HostMemory, channel: u32) -> (u32, u32, u32) {
    let mut p = [0u8; 24];
    p[..4].copy_from_slice(&channel.to_le_bytes());
    let status = host.control(host.subdevice(), CMD_GET_CTXSW_PREEMPTION_MODE, &mut p);
    (
        status,
        u32::from_le_bytes(p[20..24].try_into().unwrap()),
        u32::from_le_bytes(p[16..20].try_into().unwrap()),
    )
}

fn get_gfxp_mode(host: &HostMemory, channel: u32) -> (u32, u32) {
    let (status, gfxp, _) = get_preemption_modes(host, channel);
    (status, gfxp)
}

fn gfxp_bind_params(root: u32, tsg: u32, mode: u32, addresses: &[u64; 5]) -> [u8; 112] {
    let mut p = [0u8; 112];
    p[0..4].copy_from_slice(&2u32.to_le_bytes());
    p[4..8].copy_from_slice(&root.to_le_bytes());
    p[8..12].copy_from_slice(&tsg.to_le_bytes());
    for (i, address) in addresses.iter().enumerate() {
        p[16 + i * 8..24 + i * 8].copy_from_slice(&address.to_le_bytes());
    }
    p[88..92].copy_from_slice(&mode.to_le_bytes());
    p
}

fn bind_gfxp(host: &HostMemory, tsg: u32, mode: u32, addresses: &[u64; 5]) -> u32 {
    let mut p = gfxp_bind_params(host.root(), tsg, mode, addresses);
    host.control(host.subdevice(), CMD_BIND_CTXSW_PREEMPTION_MODE, &mut p)
}

/// `NV2080_CTRL_SET_CTXSW_PREEMPTION_MODE_COMPUTE_CTA`/`_CILP` (ctrl2080gr.h:856-857).
pub const COMPUTE_CTA: u32 = crate::preempt::COMPUTE_CTA;
pub const COMPUTE_CILP: u32 = crate::preempt::COMPUTE_CILP;

/// `NV2080_CTRL_GR_CTXSW_PREEMPTION_BIND_PARAMS` for compute mode `mode` alone: flags
/// CILP_SET (0:0), no buffer pointers, graphics ignored (ctrl2080gr.h:842-857, 917).
fn compute_bind_params(root: u32, tsg: u32, mode: u32) -> [u8; 112] {
    let mut p = [0u8; 112];
    p[0..4].copy_from_slice(&1u32.to_le_bytes());
    p[4..8].copy_from_slice(&root.to_le_bytes());
    p[8..12].copy_from_slice(&tsg.to_le_bytes());
    p[92..96].copy_from_slice(&mode.to_le_bytes());
    p
}

fn bind_compute(host: &HostMemory, tsg: u32, mode: u32) -> u32 {
    let mut p = compute_bind_params(host.root(), tsg, mode);
    host.control(host.subdevice(), CMD_BIND_CTXSW_PREEMPTION_MODE, &mut p)
}

/// Steps 0 and 1 (preempt.rs) on the mirror's own unprivileged host client: the GET on a
/// host channel, the pointer-free compute BIND on a host TSG.
impl crate::preempt::CtxswHost for HostMemory {
    fn modes(&self, channel: u32) -> crate::preempt::Modes {
        let mut p = [0u8; crate::preempt::MODES_SIZE];
        p[..4].copy_from_slice(&channel.to_le_bytes());
        let status = self.control(self.subdevice(), CMD_GET_CTXSW_PREEMPTION_MODE, &mut p);
        crate::preempt::Modes::decode(status, &p)
    }

    fn bind_compute(&self, tsg: u32, mode: u32) -> u32 {
        bind_compute(self, tsg, mode)
    }
}

/// [`crate::preempt::CtxswHost`] taking the host lock per call, so an idle between the
/// calls (`Mirror::idle_host_channels`, which locks itself) does not deadlock.
struct LockedHost<'a>(&'a Mutex<HostMemory>);

impl crate::preempt::CtxswHost for LockedHost<'_> {
    fn modes(&self, channel: u32) -> crate::preempt::Modes {
        crate::preempt::CtxswHost::modes(&*self.0.lock().unwrap(), channel)
    }

    fn bind_compute(&self, tsg: u32, mode: u32) -> u32 {
        bind_compute(&self.0.lock().unwrap(), tsg, mode)
    }
}

struct HostVas {
    handle: u32,
    root: u64,
    /// Built by replaying the guest's own VA-space controls (alloc parameters and
    /// `COPY_SERVER_RESERVED_PDES`): the host RM then owns the server-reserved range
    /// inside the guest's page-directory chain and can bind GR contexts. `false`: the
    /// externally-owned form of host-exec.md (copy engines only).
    replayed: bool,
}

/// The shadow VAS of one guest root, the set-B ranges excluded from it, and the guest
/// channels and TSGs placed on it.
struct ShadowSpace {
    /// The walk and the filter (this thread); the host side lives in the worker.
    front: crate::shadow::ShadowFront,
    vas: u32,
    exclude: Vec<(u64, u64)>,
    /// A fresh stamp whenever `exclude` is assigned ([`Mirror::shadow_exclusions`]).
    exclude_gen: u64,
    /// The inputs `exclude` was last computed from ([`GlobalExtents::refresh`]); `None`:
    /// compute it again at the next refresh.
    set_b_stamp: Option<SetBStamp>,
    /// Set-B ranges of guest channels already freed, held by the host TSG that still
    /// stands for their GR context ([`held_at_free`]); released at that TSG's host free.
    /// Changed only through [`ShadowSpace::hold`] and [`ShadowSpace::release`].
    held: BTreeMap<TsgKey, Vec<(u64, u64)>>,
    chids: std::collections::BTreeSet<u32>,
    tsgs: std::collections::BTreeSet<TsgKey>,
}

impl ShadowSpace {
    fn new(front: crate::shadow::ShadowFront, vas: u32) -> Self {
        ShadowSpace {
            front,
            vas,
            exclude: Vec::new(),
            exclude_gen: crate::ptcache::fresh_generation(),
            set_b_stamp: None,
            held: BTreeMap::new(),
            chids: Default::default(),
            tsgs: Default::default(),
        }
    }

    /// Hold `ranges` out of set A for host TSG `key` ([`held_at_free`]).
    fn hold(&mut self, key: TsgKey, ranges: Vec<(u64, u64)>) {
        let e = self.held.entry(key).or_default();
        e.extend(ranges);
        e.sort_unstable();
        e.dedup();
        self.set_b_stamp = None;
    }

    /// The ranges held for host TSG `key`, released back to set A.
    fn release(&mut self, key: TsgKey) -> Option<Vec<(u64, u64)>> {
        let r = self.held.remove(&key);
        if r.is_some() {
            self.set_b_stamp = None;
        }
        r
    }
}

struct HostTsg {
    handle: u32,
    vas: (u32, u32),
    engine: u32,
    /// The guest object the host TSG stands for: the guest TSG `(hClient, hTsg)`, or the
    /// channel itself for a bare channel's own TSG.
    guest: (u32, u32),
    gfxp: Option<OwnedGfxp>,
    /// Steps 0 and 1 of the GR preemption fix (preempt.rs) on a GR TSG.
    preempt: TsgPreempt,
}

/// What the build check of a host GR TSG found and did (preempt.rs).
#[derive(Clone, Copy, Debug, Default)]
struct TsgPreempt {
    /// The modes were read (and the guest's compute request applied) after the TSG's first
    /// GR object, before that channel's first schedule.
    checked: bool,
    /// The last application: at the build, or at a SET after it.
    outcome: Option<crate::preempt::Outcome>,
    /// The modes were read at the teardown.
    teardown_read: bool,
}

/// The key of the host TSG map (docs/caraxes/gsp-plan/classes.md §8, TSG): the kernel's
/// TSG id where the kernel assigns one and GSP-RM takes it over, the guest handle
/// otherwise.
///
/// * `Grp(id)`: a 615 kernel assigns every GSP-client TSG its grpID in `internalFlags`
///   (`TSG_ID` 15:0 with `TSG_ID_VALID` 16, g_kernel_channel_group_nvoc.h:123-129, set at
///   kernel_channel_group_api.c:442-455), and the GSP build creates the TSG under that
///   fixed id (`kchangrpInit(..., bFixedTsgID, requestedTsgID, ...)`, :273-285). A bare
///   channel's internal TSG is named the same way by the channel's `INTERNAL_TSG_ID` 23:8
///   with `_VALID` 24 (g_kernel_channel_nvoc.h:199-202, kernel_channel.c:2786-2799). The id
///   is what GSP-RM knows the TSG by, so every guest handle of one TSG (the kernel's and a
///   duplicate) and a bare channel's internal TSG meet at one host TSG.
/// * `Handle(hClient, hObject)`: a 610 kernel sends no id (its TSG parameters end before
///   `internalFlags`), so the guest TSG handle, or a bare channel's own handle, is the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TsgKey {
    Grp(u32),
    Handle(u32, u32),
}

impl std::fmt::Display for TsgKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TsgKey::Grp(id) => write!(f, "grpID {id}"),
            TsgKey::Handle(c, o) => write!(f, "{c:#x}/{o:#x}"),
        }
    }
}

/// The host TSG key of guest channel `ch`, whether it is a bare channel (no guest TSG
/// parent), and the guest object the host TSG stands for (the TSG, or the bare channel).
pub fn tsg_key_of(reg: &Registry, ch: &Channel) -> (TsgKey, bool, (u32, u32)) {
    let parent = (ch.h_client, ch.h_parent);
    if reg.tsg(parent).is_some() {
        let key = reg
            .tsg_id(parent)
            .map_or(TsgKey::Handle(parent.0, parent.1), TsgKey::Grp);
        return (key, false, parent);
    }
    let own = (ch.h_client, ch.h_object);
    let key = ch
        .internal_tsg_id
        .map_or(TsgKey::Handle(own.0, own.1), TsgKey::Grp);
    (key, true, own)
}

/// The host TSG key of a guest TSG handle (or a bare channel's handle), when the registry
/// knows the object.
pub fn tsg_key_of_guest(reg: &Registry, obj: (u32, u32)) -> Option<TsgKey> {
    if reg.tsg(obj).is_some() {
        return Some(reg.tsg_id(obj).map_or(TsgKey::Handle(obj.0, obj.1), TsgKey::Grp));
    }
    let ch = reg
        .channels()
        .find(|c| (c.h_client, c.h_object) == obj)?;
    match tsg_key_of(reg, ch) {
        (k, true, _) => Some(k),
        _ => None,
    }
}

/// How a deferred eviction releases the target's host context ([`Mirror::evict_deferred_context`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EvictPath {
    /// Idle the target's host channel, harvest its GPGet, free its host TSG.
    HostChannel,
    /// A target that never ran on the host, alone in its TSG: free the host TSG (its context).
    FreeTsg,
    /// A target that never ran, with no host TSG left: nothing on the host.
    Nothing,
    /// A target without a host channel while other channels of its TSG are hosted: refused
    /// (the context is theirs to release).
    Refuse,
}

fn evict_path(hosted: bool, other_members_hosted: bool, host_tsg: bool) -> EvictPath {
    match (hosted, other_members_hosted, host_tsg) {
        (true, _, _) => EvictPath::HostChannel,
        (false, true, _) => EvictPath::Refuse,
        (false, false, true) => EvictPath::FreeTsg,
        (false, false, false) => EvictPath::Nothing,
    }
}

/// What to do with an existing host TSG under the key a new guest TSG maps to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TsgReuse {
    /// The same guest TSG (or a channel of it): join the host TSG.
    Join,
    /// The key's host TSG stands for a guest TSG the registry no longer knows (its FREE
    /// did not reach the mirror): free it, then build a new one.
    Stale,
    /// Two live guest TSGs claim one kernel id: the kernel never does this (its grpID
    /// allocator is a bitmap); refused, the channel stays in software.
    Collision,
}

/// The decision for an existing host TSG that stands for `existing` when guest object
/// `wanted` maps to the same key.
pub fn tsg_reuse(reg: &Registry, existing: (u32, u32), wanted: (u32, u32)) -> TsgReuse {
    if existing == wanted {
        return TsgReuse::Join;
    }
    let live = reg.tsg(existing).is_some()
        || reg.channels().any(|c| (c.h_client, c.h_object) == existing);
    if live {
        TsgReuse::Collision
    } else {
        TsgReuse::Stale
    }
}

/// What one channel build yields: the host channel, its engine objects, the token.
type Built = (u32, Vec<(u32, u32)>, BTreeMap<u32, u32>, u32);

struct ShadowUserd {
    storage: HostUserd,
    guest: u64,
    entries: u32,
    published_put: Option<u32>,
    /// Get and GPGet of the host USERD at the last harvest (`GpuState::read_if_moved`).
    harvested: Option<(u32, u32)>,
}

/// A host channel's private USERD, shared with the USERD poller that runs without the
/// device lock (docs/caraxes/execution.md §9.11). The cell's mutex is a leaf: the device
/// lock's holders (publish, harvest, the takeovers, the free) and the poller take it
/// briefly; nothing under it takes another lock. `userd` is `None` once the channel is freed
/// (its host USERD goes after the cell is emptied, so the poller never reads an unmapped
/// window); `ready` mirrors `HostChannel::context_ready`.
pub struct UserdSlot {
    cell: Mutex<UserdCell>,
}

struct UserdCell {
    userd: Option<ShadowUserd>,
    ready: bool,
}

impl UserdSlot {
    fn new(u: ShadowUserd, ready: bool) -> Arc<Self> {
        Arc::new(UserdSlot {
            cell: Mutex::new(UserdCell { userd: Some(u), ready }),
        })
    }

    fn cell(&self) -> std::sync::MutexGuard<'_, UserdCell> {
        self.cell.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn with<R>(&self, f: impl FnOnce(&mut ShadowUserd) -> R) -> Option<R> {
        self.cell().userd.as_mut().map(f)
    }

    fn set_ready(&self, ready: bool) {
        self.cell().ready = ready;
    }

    /// Empty the cell (the channel's free): the poller no longer touches the USERD.
    fn take(&self) -> Option<ShadowUserd> {
        self.cell().userd.take()
    }

    /// The poll of one channel without the device lock: the GPU-owned words back to the
    /// guest's USERD if the host's moved (as the locked harvest), and whether the guest
    /// published entries the device has not forwarded (GPPut in the ring and not the one
    /// last published). A channel not ready (or freed) is skipped: false.
    pub fn poll(&self, mem: &GuestMemory) -> bool {
        let mut c = self.cell();
        if !c.ready {
            return false;
        }
        let Some(u) = c.userd.as_mut() else {
            return false;
        };
        u.harvest_sys(mem);
        u.pending(mem)
    }
}

/// The private USERDs the poller watches, by mirror (one per device model; a model rebuilt
/// at a reset publishes under a new id and the old one's entries go when it is dropped).
type UserdWatch = Vec<(u64, u32, Arc<UserdSlot>)>;

fn userd_watch_list() -> &'static Mutex<UserdWatch> {
    static W: std::sync::OnceLock<Mutex<UserdWatch>> = std::sync::OnceLock::new();
    W.get_or_init(|| Mutex::new(Vec::new()))
}

/// The private USERDs to poll now: `(chid, slot)` of every live mirror.
pub fn userd_watch() -> Vec<(u32, Arc<UserdSlot>)> {
    userd_watch_list()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .map(|(_, c, s)| (*c, s.clone()))
        .collect()
}

impl ShadowUserd {
    /// [`Self::harvest`] with guest RAM only (the words go to the guest's sysmem USERD).
    fn harvest_sys(&mut self, mem: &GuestMemory) {
        let storage = &self.storage;
        if let Some(state) = crate::userd::GpuState::read_if_moved(&mut self.harvested, |off| {
            Some(storage.read_u32(off))
        }) {
            state.publish(|off, value| {
                if let Some(at) = self.guest.checked_add(off) {
                    mem.write(at, &value.to_le_bytes());
                }
            });
        }
    }

    /// The guest published entries the device has not forwarded: its GPPut (NV_RAMUSERD
    /// GP_PUT, +0x8c) inside the ring and not the one last published.
    fn pending(&self, mem: &GuestMemory) -> bool {
        mem.read_u32(self.guest + 0x8c)
            .is_some_and(|put| put < self.entries && Some(put) != self.published_put)
    }

    fn publish(&mut self, gm: &GpuMem) -> Result<u32, String> {
        let cpu = crate::userd::CpuState::read(self.entries, |off| {
            gm.read_u32(Target::Sys(self.guest.checked_add(off)?))
        }).ok_or("unreadable or invalid guest USERD submission")?;
        cpu.publish(|off, value| self.storage.write_u32(off, value));
        self.storage.flush();
        self.published_put = Some(cpu.gp_put());
        Ok(cpu.gp_put())
    }

    fn harvest(&mut self, gm: &mut GpuMem) {
        self.harvest_sys(gm.sys);
    }
}

/// A mirror build in flight on the shadow worker (docs/caraxes/execution.md §9.2): what the
/// commit needs from the first doorbell, the doorbells that joined it, and the slot the job
/// fills ([`build_on_host`]).
struct PendingBuild {
    /// Commit order: oldest first.
    id: u64,
    root: u64,
    /// The build job's number (a preparation waits for it).
    seq: u64,
    generation: u64,
    /// The guest channel's `(hClient, hObject)`.
    h_client: u32,
    h_channel: u32,
    tsg_key: TsgKey,
    tsg_guest: (u32, u32),
    vas_key: (u32, u32),
    /// The host TSG's engine (for a TSG the build makes).
    engine_type: u32,
    /// A GFXP slot reserved for the build (the first build of a guest TSG with a 3D object).
    slot: Option<usize>,
    out: Arc<Mutex<Option<BuildOut>>>,
    /// The guest's USERD GPGet/GPPut addresses and values at the first doorbell.
    saved_userd: Option<(Target, Target, Option<u32>, Option<u32>)>,
    /// The newest nonzero GPPut read at a doorbell of the build (mbuild.rs `put_to_restore`).
    newest_put: Option<u32>,
    /// The root's last job at the newest doorbell: the ring waits for it after the commit.
    ring_seq: u64,
    doorbells: u64,
    since: Instant,
    submit: bool,
    preempt_request: Option<crate::preempt::Recorded>,
    /// Engine objects the guest allocated during the build, added at the commit.
    late_objects: Vec<(u32, u32, Vec<u8>)>,
    channel_engine: u32,
    alloc_flags: u32,
}

/// The USERD of a build's host channel, from the guest's state under the lock.
enum UserdPlan {
    /// A vidmem page of the reservation: `(reservation handle, offset)`.
    Fb(u32, u64),
    /// Guest sysmem shadowed by a private host USERD (`CARAXES_SHADOW_USERD=1`).
    Private { iova: u64, entries: u32 },
    /// A guest sysmem page: the device's host pointer to it, the USERD's offset in it.
    GuestPage { ptr: usize, offset: u64, iova: u64 },
}

/// What the worker needs for a build ([`build_on_host`]): no guest memory, no device state.
struct BuildInput {
    chid: u32,
    vas_handle: u32,
    tsg_key: TsgKey,
    tsg_guest: (u32, u32),
    cell: Arc<Mutex<crate::mbuild::TsgCell>>,
    engine_type: u32,
    userd: UserdPlan,
    /// The device's host pointer to the guest's error-notifier page, if it has one.
    error_page: Option<usize>,
    gpfifo_va: u64,
    gpfifo_entries: u32,
    channel_engine: u32,
    alloc_flags: u32,
    /// `(guest handle, class, params)` of the guest's engine objects.
    objects: Vec<(u32, u32, Vec<u8>)>,
    /// The engine a channel without an object is bound to (a video or copy channel).
    bind_engine: Option<u32>,
    gpfifo_class: u32,
    /// The GFXP slot and its buffers, or why there is none (every slot in use).
    gfxp_slot: Option<GfxpSlot>,
    reservation: u32,
    base: u64,
    /// On a GR TSG: the guest's compute preemption request (steps 0 and 1, preempt.rs).
    preempt: Option<Option<u32>>,
    sw_classes: [u32; 2],
    diag: Option<GpfifoDiag>,
}

/// A reserved GFXP slot and its five buffers `(pa, va, len)`, or why there is none.
type GfxpSlot = Result<(usize, [(u64, u64, u64); 5]), String>;
/// `CARAXES_SHADOW_CHANNEL_DIAG=1`: the GPFIFO span's bytes and end, its guest and host
/// coverage, and the set-B ranges it meets.
type GpfifoDiag = (u64, Option<u64>, String, String, Vec<(u64, u64)>);
/// The host channel, its engine objects and their map; or why not, with the RM status.
type HostSteps = Result<(u32, Vec<(u32, u32)>, BTreeMap<u32, u32>), (String, Option<u32>)>;

/// What the worker made for a build.
struct BuildOut {
    /// The host TSG (made by this build, or joined), also when a later step failed.
    tsg: Option<(u32, bool)>,
    result: Result<HostBuilt, (String, Option<u32>)>,
    gfxp: Option<Result<OwnedGfxp, (Option<OwnedGfxp>, String)>>,
    preempt: Option<crate::preempt::Applied>,
    notes: Vec<String>,
    host: std::time::Duration,
}

/// A host channel the worker made, not scheduled yet.
struct HostBuilt {
    channel: u32,
    objects: Vec<(u32, u32)>,
    engine_map: BTreeMap<u32, u32>,
    /// Everything the build made on the host except the TSG, oldest first (the USERD and
    /// error-notifier objects, the channel).
    owned: Vec<u32>,
    shadow_userd: Option<ShadowUserd>,
    rc_page: Option<crate::hostmem::DevicePage>,
}

/// The host side of a mirror build (execution.md §9.2), on the shadow worker after the
/// root's plans: the host TSG (made or joined), the USERD and error-notifier objects, the
/// channel, the engine objects (the GR object makes the host RM build the context), the
/// owned GFXP bind and the preemption bind. Not the scheduling: the channel cannot run
/// before the commit. The host lock is taken per RM call, so the device's own host calls
/// interleave. On a refusal what the build made is freed again, except a TSG, which stands
/// for the guest TSG as at the synchronous build.
fn build_on_host(b: BuildInput, hm: &Mutex<HostMemory>) -> BuildOut {
    let t0 = Instant::now();
    let lock = || hm.lock().unwrap_or_else(PoisonError::into_inner);
    let chid = b.chid;
    let mut notes = Vec::new();
    let tsg = crate::mbuild::tsg_for_build(&b.cell, || {
        // The host driver's layout (hostabi.rs).
        let mut tp = crate::hostabi::current().tsg_params(b.vas_handle, b.engine_type);
        let mut h = lock();
        let device = h.device();
        h.alloc_object(device, sys::KEPLER_CHANNEL_GROUP_A, &mut tp, "shadow-tsg")
    });
    let (tsg, made_tsg) = match tsg {
        Ok((t, made)) => {
            notes.push(if made {
                format!(
                    "shadow: host TSG {t:#x} for guest TSG {:#x} ({}) on VAS {:#x} [host]",
                    b.tsg_guest.1, b.tsg_key, b.vas_handle
                )
            } else {
                format!(
                    "shadow: chid {chid} joins host TSG {t:#x} of guest TSG {:#x} ({}) [host]",
                    b.tsg_guest.1, b.tsg_key
                )
            });
            (t, made)
        }
        Err(st) => {
            return BuildOut {
                tsg: None,
                result: Err((format!("shadow: TSG refused: {}", status_name(st)), Some(st))),
                gfxp: None,
                preempt: None,
                notes,
                host: t0.elapsed(),
            }
        }
    };
    let mut owned: Vec<u32> = Vec::new();
    let mut shadow_userd: Option<ShadowUserd> = None;
    let mut rc_page: Option<crate::hostmem::DevicePage> = None;
    let mut gfxp = None;
    let mut preempt = None;
    let built = (|| -> HostSteps {
        let userd = match b.userd {
            UserdPlan::Fb(handle, offset) => (handle, offset),
            UserdPlan::Private { iova, entries } => {
                let page = lock().alloc_host_userd().map_err(|st| {
                    (format!("shadow USERD allocation: {}", status_name(st)), Some(st))
                })?;
                let handle = page.handle();
                notes.push(format!("shadow USERD chid {chid}: guest sysmem {iova:#x} -> private host vidmem {:#x}, object {handle:#x} [host]", page.gpu_addr()));
                shadow_userd = Some(ShadowUserd {
                    storage: page,
                    guest: iova,
                    entries,
                    published_put: None,
                    harvested: None,
                });
                (handle, 0)
            }
            UserdPlan::GuestPage { ptr, offset, iova } => {
                match lock().guest_page_object(ptr as *mut u8, PAGE, "shadow-userd-page") {
                    Ok(d) => {
                        owned.push(d);
                        (d, offset)
                    }
                    Err(e) => {
                        return Err((
                            format!("shadow: chid {chid} USERD IOVA {iova:#x}: no host object over the guest page ({e:#})"),
                            None,
                        ))
                    }
                }
            }
        };
        // The error notifier: the guest's page, else a page of the device's own.
        let guest_error = b
            .error_page
            .and_then(|ptr| lock().guest_page_object(ptr as *mut u8, PAGE, "shadow-error-notifier").ok());
        let h_error = match guest_error {
            Some(d) => {
                owned.push(d);
                d
            }
            None => match lock().device_page_object("rc-notifier") {
                Ok((d, page)) => {
                    owned.push(d);
                    rc_page = Some(page);
                    d
                }
                Err(e) => {
                    notes.push(format!(
                        "shadow: chid {chid}: no host error notifier ({e:#}); an RC of this channel reaches the guest only as a fence timeout [host]"
                    ));
                    0
                }
            },
        };
        let mut cp = crate::hostabi::ChannelAllocParams::default();
        cp.flags = HOST_MIRROR_CHANNEL_FLAGS;
        cp.hObjectError = h_error;
        cp.gpFifoOffset = b.gpfifo_va;
        cp.gpFifoEntries = b.gpfifo_entries;
        cp.hUserdMemory[0] = userd.0;
        cp.userdOffset[0] = userd.1;
        cp.engineType = b.channel_engine;
        if let Some((bytes, end, guest, host, hits)) = &b.diag {
            notes.push(format!(
                "shadow diagnostic: chid {chid} GPFIFO start {:#x} end {end:#x?} {} entries/{bytes:#x} bytes: guest {guest}; host {host}; set-B intersections {hits:x?}; USERD object {:#x}+{:#x}; engineType {:#x}; guest alloc_flags {:#x}; host flags {:#x}/internalFlags {:#x}; host ABI {} [host]",
                b.gpfifo_va, b.gpfifo_entries, userd.0, userd.1, b.channel_engine,
                b.alloc_flags, cp.flags, cp.internalFlags, crate::hostabi::current().driver(),
            ));
        }
        let channel = lock()
            .alloc_object(tsg, b.gpfifo_class, as_bytes(&mut cp), "shadow-channel")
            .map_err(|st| {
                (
                    format!(
                        "shadow: channel refused: {} (gpfifo va {:#x})",
                        status_name(st),
                        b.gpfifo_va
                    ),
                    Some(st),
                )
            })?;
        owned.push(channel);
        notes.push(format!(
            "shadow: host GR channel {channel:#x} in TSG {tsg:#x} on VAS {:#x}, gpfifo va {:#x} [host]",
            b.vas_handle, b.gpfifo_va
        ));
        // The guest's engine objects; the compute object makes the host RM build set B.
        let mut objects = Vec::new();
        let mut engine_map = BTreeMap::new();
        let mut refused = None;
        for (guest_handle, class, params) in &b.objects {
            let mut p = params.clone();
            let r = lock().alloc_object(channel, crate::board::host_class_of(*class), &mut p, "shadow-engine");
            match r {
                Ok(oh) => {
                    notes.push(format!(
                        "shadow: chid {chid} host object class {class:#x}{} {oh:#x} [host]",
                        crate::board::mapped_note(*class)
                    ));
                    objects.push((*class, oh));
                    engine_map.insert(*guest_handle, oh);
                }
                Err(e) => {
                    refused = refused.or(Some(e));
                    notes.push(format!(
                        "shadow: chid {chid} object class {class:#x} refused: {}",
                        status_name(e)
                    ))
                }
            }
        }
        if objects.is_empty() {
            // A video channel rings before its engine object exists, and the guest
            // waits for that first push before it allocates the object (a held doorbell
            // deadlocked for 10 s, 2026-09-25 probe-hold). The host channel is bound to
            // the engine instead; the object follows at the guest's allocation
            // (`place_video_object`). A copy channel likewise (`copy_bind`); its object
            // is added on the host at the guest's allocation (`add_engine_object`).
            let Some(engine) = b.bind_engine else {
                return Err(("shadow: no engine object; not scheduling".into(), refused));
            };
            let mut p = engine.to_le_bytes();
            let st = lock().control(channel, CMD_CHANNEL_BIND, &mut p);
            if st != sys::NV_OK {
                return Err((
                    format!(
                        "shadow: chid {chid} BIND engineType {engine:#x}: {}; not scheduling",
                        status_name(st)
                    ),
                    None,
                ));
            }
            notes.push(format!(
                "shadow: chid {chid} host channel bound to engineType {engine:#x} ({}; its object follows the guest's) [host]",
                video_engine(engine).unwrap_or("copy engine")
            ));
        }
        // Owned GFXP (gfxpslot.rs): a new host TSG with a 3D object gets a slot of the VM's
        // slice bound before its first schedule, for every guest (NVIDIA's vGPU binds GFXP
        // for every 3D object of a Unix guest, rpc.c:3951-3956 of 615). No slot: the
        // context stays in graphics WFI.
        let three_d = objects
            .iter()
            .any(|(class, _)| crate::board::host_class_of(*class) == HOST_3D_CLASS);
        if made_tsg && three_d {
            gfxp = match &b.gfxp_slot {
                Some(Ok((slot, buffers))) => Some(attach_gfxp_slot(
                    &mut lock(),
                    *slot,
                    buffers,
                    b.vas_handle,
                    tsg,
                    channel,
                    b.reservation,
                    b.base,
                )),
                Some(Err(why)) => Some(Err((None, why.clone()))),
                None => None,
            };
        }
        // The GR context exists once an object of the GR engine does (not the UVM or
        // deferred-API software classes); the channel is not scheduled yet.
        if let Some(requested) = b.preempt {
            if objects.iter().any(|(class, _)| !b.sw_classes.contains(class))
                && crate::mbuild::claim_preempt_check(&b.cell)
            {
                preempt = Some(crate::preempt::apply_compute(&*lock(), tsg, channel, requested));
            }
        }
        Ok((channel, objects, engine_map))
    })();
    let result = match built {
        Ok((channel, objects, engine_map)) => Ok(HostBuilt {
            channel,
            objects,
            engine_map,
            owned,
            shadow_userd,
            rc_page,
        }),
        Err(e) => {
            // Newest first: the channel (with its engine objects), then the guest pages.
            let mut h = lock();
            let mut freed = true;
            for d in owned.into_iter().rev() {
                freed &= h.free_object(d).is_ok();
            }
            if let Some(u) = shadow_userd.take() {
                // If a channel free failed, retain its tracked memory object until
                // RM client teardown; the CPU mapping may still be dropped safely.
                if freed {
                    let _ = h.free_host_userd(u.storage);
                }
            }
            Err(e)
        }
    };
    BuildOut {
        tsg: Some((tsg, made_tsg)),
        result,
        gfxp,
        preempt,
        notes,
        host: t0.elapsed(),
    }
}

/// The host side of one guest channel.
pub struct HostChannel {
    generation: u64,
    pub handle: u32,
    pub tsg: TsgKey,
    pub token: u32,
    /// `(class, host handle)` of the replayed engine objects.
    pub objects: Vec<(u32, u32)>,
    /// Exact guest engine handle to host handle, scoped by this channel generation.
    engine_map: BTreeMap<u32, u32>,
    /// The host memory objects over the guest's own RAM pages this channel uses (its USERD
    /// and error notifier in sysmem, `guest_page_object`): children of the device, not of
    /// the channel, so they are freed with the channel explicitly (before, they lived until
    /// the device exited and kept the guest pages pinned; docs/caraxes/host-resources.md).
    pub pages: Vec<u32>,
    pub created: Instant,
    pub doorbells: u64,
    /// The guest root of the shadow VAS a shadow GR channel runs on (`None`: CE mirror).
    pub shadow_root: Option<u64>,
    /// The guest's GPPut when the device last forwarded this channel's doorbell (untrapped.rs
    /// `ran_ahead`).
    pub fwd_put: Option<u32>,
    /// `Mirror::unwaited` (the count of plans submitted without a wait) at that forward.
    pub fwd_unwaited: u64,
    /// Private low-address USERD for a guest sysmem USERD on a high-IOVA host, shared with
    /// the USERD poller ([`UserdSlot`]).
    userd: Option<Arc<UserdSlot>>,
    context_ready: bool,
    /// The host error notifier when the guest's could not be mirrored (a guest notifier in
    /// VRAM): the device's own page, read by the RC scan (`Mirror::rc_notifier`).
    rc_page: Option<crate::hostmem::DevicePage>,
}

impl HostChannel {
    /// `f` on the private USERD, if the channel has one.
    fn with_userd<R>(&self, f: impl FnOnce(&mut ShadowUserd) -> R) -> Option<R> {
        self.userd.as_ref().and_then(|s| s.with(f))
    }

    fn set_context_ready(&mut self, ready: bool) {
        self.context_ready = ready;
        if let Some(s) = &self.userd {
            s.set_ready(ready);
        }
    }
}

/// Counters for the exit summary.
#[derive(Default, Debug, Clone)]
pub struct MirrorStats {
    pub channels_created: u64,
    pub channels_freed: u64,
    pub doorbells: u64,
    pub invalidates: u64,
    pub refusals: u64,
    pub max_create_us: u128,
}

/// Sizes of the host GPU's global GR context buffers, by the host FIFO engine id used by
/// `NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES`. These are properties of the
/// physical GPU, not of a guest channel or context. A cache belongs to one [`Mirror`], and
/// therefore to the one host RM device/client that mirror was built with; recreating the
/// mirror for another device starts with an empty cache.
#[derive(Default)]
struct GlobalCtxSizeCache {
    by_engine: BTreeMap<u32, Option<u64>>,
    queries: u64,
    hits: u64,
    /// Answers not cached (a possibly transient error): asked again at the next use.
    retries: u64,
}

impl GlobalCtxSizeCache {
    /// The FIFO engine property which gives the size of a zero-sized
    /// `NV2080_CTRL_GPU_PROMOTE_CTX_BUFFER_ID_*` entry.
    fn engine_id(buffer_id: u16) -> Option<u32> {
        // ctrl0080fifo.h ENGINE_ID_GRAPHICS_*: BUNDLE_CB 0x11, PAGEPOOL 0x0d,
        // ATTRIBUTE_CB 0x13, RTV_CB_GLOBAL 0x14, GFXP_POOL 0x15,
        // GFXP_CTRL_BLK 0x16, FECS_EVENT 0x17, PRIV_ACCESS_MAP 0x18.
        Some(match buffer_id {
            3 => 0x11,
            4 => 0x0d,
            5 => 0x13,
            6 => 0x14,
            7 => 0x15,
            8 => 0x16,
            9 => 0x17,
            10..=12 => 0x18,
            _ => return None,
        })
    }

    /// Return a cached size, or query it once. Successful answers (including a zero size)
    /// and NOT_SUPPORTED are immutable properties of this host-device generation and are
    /// cached. Other errors may be transient (for example across recovery), so they are
    /// returned but retried on the next use.
    fn get_or_query(
        &mut self,
        buffer_id: u16,
        query: impl FnOnce(u32) -> (u32, u32),
    ) -> Option<u64> {
        let engine_id = Self::engine_id(buffer_id)?;
        if let Some(size) = self.by_engine.get(&engine_id) {
            self.hits += 1;
            return *size;
        }
        self.queries += 1;
        let (status, raw) = query(engine_id);
        let size = (status == sys::NV_OK && raw != 0).then(|| u64::from(raw));
        if status == sys::NV_OK || status == sys::NV_ERR_NOT_SUPPORTED {
            self.by_engine.insert(engine_id, size);
        } else {
            self.retries += 1;
        }
        size
    }

    /// Engine results cached so far: a new one may change a size (part of [`SetBStamp`]).
    fn cached(&self) -> usize {
        self.by_engine.len()
    }

    fn summary(&self) -> String {
        format!(
            "global context sizes: {} host query(s), {} cache hit(s), {} cached engine result(s)",
            self.queries,
            self.hits,
            self.by_engine.len()
        )
    }
}

/// The inputs of a shadow root's set B by generation ([`GlobalExtents::refresh`]), besides
/// the root's held and private ranges (whose change clears [`ShadowSpace::set_b_stamp`]):
/// while they stand, so does set B, and a doorbell reuses it instead of computing it again.
/// The computation walks every channel's promoted buffers and the guest's walk around each
/// global buffer: 10 µs, twice per doorbell, for a CUDA context whose eight GR channels
/// promote ten global buffers each (the 2026-09-28 latency run: the doorbell's plan step at
/// 20.6 µs p50, 0.2 µs on 2026-09-25; `bench_set_b_clean_doorbell`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SetBStamp {
    /// The registry's channels, their VA-space resolution and their promoted buffers
    /// (channel.rs `Registry::set_b_generation`).
    registry: u64,
    /// The guest's cached walk of the root (`None`: none), which clips a global buffer's
    /// exclusion ([`global_exclusion`], ptcache.rs `WalkCache::generation`).
    walk: Option<u64>,
    /// The host's global context sizes cached so far ([`GlobalCtxSizeCache::cached`]).
    sizes: usize,
}

/// The host side of the set-B computation: the global context sizes and the clip of each
/// size-0 global buffer (run P19) with its one-time WARN. Interior mutability keeps the
/// exclusion walkers read-only; `Mirror` itself is serialized by the Device mutex.
#[derive(Default)]
struct GlobalExtents {
    /// Immutable global-context sizes of this mirror's physical host GPU. A new host RM
    /// device gets a new `Mirror` and an empty cache.
    sizes: RefCell<GlobalCtxSizeCache>,
    /// The exclusion end of each size-0 global buffer by (shadow root, VA), as last computed
    /// with the guest's walk ([`global_exclusion`]); used where no walk is at hand (a free).
    clips: RefCell<BTreeMap<(u64, u64), u64>>,
    /// Clips already reported by (root, VA, end): one WARN each.
    clip_seen: RefCell<std::collections::BTreeSet<(u64, u64, u64)>>,
    /// WARN lines not yet in the trace ([`Self::refresh`] hands them out).
    clip_notes: RefCell<Vec<String>>,
    /// Refreshes that reused set B (no input changed), and those that computed it.
    reused: std::cell::Cell<u64>,
    computed: std::cell::Cell<u64>,
}

/// How the host answers a global context size: `(buffer id, FIFO engine id)` to
/// `(status, size)` (`Mirror::host_ctx_size`).
type CtxSizeQuery<'a> = &'a dyn Fn(u16, u32) -> (u32, u32);

impl GlobalExtents {
    /// The size of global GR buffer `buffer_id` on the host ([`GlobalCtxSizeCache`]).
    fn size(&self, buffer_id: u16, query: CtxSizeQuery) -> Option<u64> {
        self.sizes
            .borrow_mut()
            .get_or_query(buffer_id, |engine| query(buffer_id, engine))
    }

    /// The exclusion size of size-0 global buffer `buffer_id` at `va` on shadow root `root`
    /// ([`global_exclusion`]): with the guest's cached walk when `fb` is given, else the
    /// last such result (or the unclipped extent). A clip is logged once as a WARN.
    fn extent(
        &self,
        fb: Option<&FakeFb>,
        root: u64,
        buffer_id: u16,
        va: u64,
        query: CtxSizeQuery,
    ) -> Option<u64> {
        let raw = self.size(buffer_id, query)?;
        let Some(fb) = fb else {
            let end = self.clips.borrow().get(&(root, va)).copied();
            return Some(end.unwrap_or_else(|| global_exclusion(va, raw, |_, _| None).0) - va);
        };
        let (end, clip) = global_exclusion(va, raw, |lo, hi| guest_first_mapped(fb, root, lo, hi));
        self.clips.borrow_mut().insert((root, va), end);
        if clip != GlobalClip::Full && self.clip_seen.borrow_mut().insert((root, va, end)) {
            let line = match clip {
                GlobalClip::Cut { at } => format!(
                    "WARN: shadow root {root:#x}: global buffer {buffer_id} at {va:#x}: set-B exclusion cut short at {at:#x} by a guest-mapped page (host size {raw:#x}, footprint to {:#x}, 64 KiB extent to {:#x}) [set B]",
                    va + host_footprint(raw),
                    va + ((raw + 0xFFFF) & !0xFFFF)
                ),
                GlobalClip::Full => unreachable!(),
            };
            eprintln!("vfio-user-nvgpu: {line}");
            self.clip_notes.borrow_mut().push(line);
        }
        Some(end - va)
    }

    /// The set-B exclusions of shadow root `root` now: the live channels'
    /// ([`exclusions_of`]), and with the root's shadow `space` the ranges held for a host
    /// TSG not yet freed ([`set_b_with_held`]) and its private ones. Every GR channel of a
    /// context promotes the same global buffers (a CUDA context: eight channels, ten
    /// buffers), so each `(buffer id, VA)` is sized and clipped once per computation.
    /// A size-0 global inside the root's host `fence` (F2b) holds no host buffer: the host
    /// maps its own first fit above the fence, so the guest's pages there are plain set A.
    fn set_b(
        &self,
        reg: &Registry,
        root: u64,
        fb: Option<&FakeFb>,
        space: Option<&ShadowSpace>,
        fence: Option<(u64, u64)>,
        query: CtxSizeQuery,
    ) -> Vec<(u64, u64)> {
        let once: RefCell<BTreeMap<(u16, u64), Option<u64>>> = RefCell::new(BTreeMap::new());
        let live = exclusions_of(reg, root, |id, va| {
            if fence.is_some_and(|(lo, hi)| va >= lo && va < hi) {
                return Some(0);
            }
            if let Some(e) = once.borrow().get(&(id, va)) {
                return *e;
            }
            let e = self.extent(fb, root, id, va, query);
            once.borrow_mut().insert((id, va), e);
            e
        });
        match space {
            Some(sp) => {
                let mut out = set_b_with_held(live, &sp.held);
                out.sort_unstable();
                out.dedup();
                out
            }
            None => live,
        }
    }

    /// The stamp of set B of `root` from the inputs as they are now.
    fn stamp(&self, reg: &Registry, root: u64, fb: &FakeFb) -> SetBStamp {
        SetBStamp {
            registry: reg.set_b_generation(),
            walk: fb.walk_generation(Target::Fb(root)),
            sizes: self.sizes.borrow().cached(),
        }
    }

    /// Recompute set B of shadow root `root` (its shadow `sp`, with the guest's cached walk
    /// from `fb`, [`global_exclusion`]) unless no input changed since the last time
    /// ([`SetBStamp`]: then O(1), the doorbell of a root where nothing changed); the notes,
    /// and whether set B changed.
    fn refresh(
        &self,
        reg: &Registry,
        root: u64,
        fb: &FakeFb,
        sp: &mut ShadowSpace,
        query: CtxSizeQuery,
    ) -> (Vec<String>, bool) {
        if sp.set_b_stamp.is_some_and(|s| s == self.stamp(reg, root, fb)) {
            self.reused.set(self.reused.get() + 1);
            return (self.take_notes(), false);
        }
        self.computed.set(self.computed.get() + 1);
        let retries = self.sizes.borrow().retries;
        let now = self.set_b(reg, root, Some(fb), Some(sp), sp.front.layout.fence, query);
        // A size the host did not answer for good is asked again at the next refresh.
        sp.set_b_stamp = (self.sizes.borrow().retries == retries).then(|| self.stamp(reg, root, fb));
        let mut notes = self.take_notes();
        if sp.exclude == now {
            return (notes, false);
        }
        let added = now.iter().filter(|x| !sp.exclude.contains(x)).count();
        let released = sp.exclude.iter().filter(|x| !now.contains(x)).count();
        sp.exclude = now;
        sp.exclude_gen = crate::ptcache::fresh_generation();
        notes.push(format!(
            "shadow: root {root:#x} set-B exclusions recomputed: {added} added, {released} released back to set A [guest]"
        ));
        (notes, true)
    }

    /// The WARN lines not yet in the trace.
    fn take_notes(&self) -> Vec<String> {
        std::mem::take(&mut *self.clip_notes.borrow_mut())
    }

    fn summary(&self) -> String {
        format!(
            "{}; set B: {} refresh(es) reused it, {} computed it",
            self.sizes.borrow().summary(),
            self.reused.get(),
            self.computed.get()
        )
    }
}

/// The event thread: one OS event per engine notifier plus the RC notifier.
struct Events {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    handles: Vec<u32>,
}

pub struct Mirror {
    hm: Arc<Mutex<HostMemory>>,
    intr: Arc<IntrShared>,
    chip: &'static ChipData,
    /// The reservation `[base, end)`: guest FB addresses handed to the host must lie in it.
    base: u64,
    end: u64,
    /// Whether guest FB addresses are offsets into, rather than identities with, the host
    /// reservation. Such addresses are safe only through host-managed shadow VA spaces.
    translated: bool,
    /// Real compression per slice (D16b, compslice.rs): the policy every shadow VAS filters
    /// and audits by. Like a translated FB it allows shadow-only execution: a host channel
    /// on the guest's own tables would use the guest's COMPTAGLINE.
    comp: Option<Arc<crate::compslice::Policy>>,
    reservation: u32,
    /// The host's global context sizes and the clips of global buffers (set B).
    extents: GlobalExtents,
    vas: BTreeMap<(u32, u32), HostVas>,
    /// By [`TsgKey`]: the kernel's TSG id on a 615 guest, the guest handle on 610.
    tsgs: BTreeMap<TsgKey, HostTsg>,
    /// By guest ChID.
    channels: BTreeMap<u32, HostChannel>,
    userd_cursor: u32,
    /// This mirror's id among the USERD poller's watch list (`publish_userd_watch`).
    watch_id: u64,
    capture_deferred: bool,
    /// Replay the guest's VA-space controls to make GR channels schedulable on the host
    /// (off while the GR-context wall stands: GR runs in software, only CE on the host).
    replay_vas: bool,
    /// Run GR channels on the host and try the c2 promote (exec-formats.md §4.5).
    mirror_gr: bool,
    /// Run CUDA's copy-engine channels on the host (default); `CARAXES_MIRROR_CE=0`
    /// keeps them on the software executor so a GR experiment is not blocked by the
    /// CE first-launch stall (execution.md §6).
    mirror_ce: bool,
    /// Forward doorbells to host channels (real execution). Off by default: the CE path
    /// hits the first-submission PBENTRY fault and GR the context wall (execution.md), so
    /// channel work stays on the software executor over the VRAM store until those are
    /// resolved; the VA-space, event and doorbell scaffolding is built either way so the
    /// mirror is exercised. `CARAXES_MIRROR_CHANNELS=1` turns it on for measurements.
    channels_enabled: bool,
    doorbell: Option<(u32, CpuMap)>,
    events: Option<Events>,
    /// The device's own privileged channel for `MEM_OP MMU_TLB_INVALIDATE` of the guest's
    /// page directories (invalidate.rs). This is the invalidation path that reaches the
    /// physical GPU: `DMA_INVALIDATE_TLB` on an externally-owned mirror VA space is a
    /// no-op (the RM walker has no root there), so the guest's register invalidate was
    /// never replayed and a fresh mapping faulted with FAULT_PDE (Xid 31, measured
    /// 2026-09-23 `d-ce-token`). `CARAXES_MEMOP_INVALIDATE=0` falls back to the (no-op)
    /// `DMA_INVALIDATE_TLB` for A/B comparison.
    invalidator: Option<Invalidator>,
    /// The device-owned host copy engine of the software CE's large transfers (devce.rs),
    /// shared with the FB store; built once at the first doorbell.
    device_ce: Option<Arc<Mutex<crate::devce::DeviceCe>>>,
    device_ce_tried: bool,
    memop_invalidate: bool,
    /// `CARAXES_MIRROR_GR_SHADOW=1`: run GR channels on a host-managed **shadow VAS**
    /// (`shadow.rs`, D17 route 4 stage 1) — the host RM owns the GR context there and the
    /// guest's set-A is mirrored in at its own VAs. The GR launch experiment.
    shadow_probe: bool,
    /// `CARAXES_MIRROR_BARE_CE=1` (display-e.md): a bare copy-engine channel (no guest TSG,
    /// the NVIDIA GL/EGL driver's CE channels) is mirrored in a host TSG of its own, keyed
    /// by the channel's handle. Bare GR channels (the kernel's golden-context and WAR
    /// channels) stay in software either way.
    bare_ce: bool,
    /// CE channels on the shadow VAS too (default with the GR shadow; `CARAXES_CE_SHADOW=0`).
    ce_shadow: bool,
    /// One shadow VAS per guest root (per CUDA context: two processes use the same VAs
    /// for different pages, measured in the gpu gate's robustness stage), built at the
    /// first GR channel of that root and freed when its last channel and TSG are gone.
    shadows: BTreeMap<u64, ShadowSpace>,
    /// The device's budget of live RM mappings over all shadow VA spaces
    /// (`CARAXES_MAP_BUDGET`, docs/caraxes/mapping.md §2).
    map_budget: crate::shadow::MapBudget,
    /// The shadow worker (docs/caraxes/mapping.md §4): applies the plans (the RM map and
    /// unmap calls) and the teardowns off the server thread, in order.
    worker: ShadowWorker,
    /// Held rings (ringgate.rs, execution.md §9.1): shared with the worker and the completer.
    ring_gate: Arc<crate::ringgate::RingGate>,
    held_rings: BTreeMap<u32, crate::ringgate::HeldRing>,
    ring_stats: crate::ringgate::RingStats,
    /// Mirror builds in flight on the worker, by guest ChID (execution.md §9.2), the host
    /// TSG cells their jobs share, the counters, and the channels committed since the
    /// dispatcher last pinned their tokens ([`Mirror::take_committed`]).
    builds: BTreeMap<u32, PendingBuild>,
    /// A deferred context's preparation hands its build to the worker instead of waiting
    /// for it under the device lock ([`Self::start_deferred_context`]).
    prepare_async: bool,
    tsg_cells: BTreeMap<TsgKey, Arc<Mutex<crate::mbuild::TsgCell>>>,
    build_stats: crate::mbuild::BuildStats,
    next_build: u64,
    committed: Vec<(u32, u32)>,
    /// `CARAXES_ALLOC_LAZY=1` (lazy.rs, docs/caraxes/alloc-path.md): a guest MMU invalidate
    /// defers its plan to the next shadow doorbell or channel creation.
    lazy: crate::lazy::LazyPlans,
    /// The rule for a mirrored channel whose GPPut store does not trap (untrapped.rs,
    /// `CARAXES_SHADOW_UNTRAPPED_HOLD`, default on) and its counters.
    hold_on: bool,
    pub hold: crate::untrapped::HoldStats,
    /// `CARAXES_SHADOW_INVALIDATE_ASYNC` (invgate.rs, default on): a held invalidate returns
    /// at once and its trigger reads busy until the worker applied the plan, instead of
    /// waiting inside the trapped write under the device lock.
    inv_async: bool,
    inv_gate: crate::invgate::InvalidateGate,
    /// Held invalidates by scope: one root, `ALL_PDB` (its untrapped roots), and the roots
    /// the `ALL_PDB` ones held in total.
    held_scope: (u64, u64, u64),
    /// Invalidates whose plans were submitted without a wait before the guest continued
    /// (eager or at the lazy deadline, not held): a host channel that runs ahead of its
    /// doorbell after one of them may run on a mirror the worker is still applying.
    unwaited: u64,
    /// The registered guest RAM descriptors `(handle, iova, size)`, read once.
    descs: Option<Vec<(u32, u64, u64)>>,
    /// Video objects the guest allocated on a mirrored shadow channel, not yet placed on
    /// the host: `(guest handle, class, params)` by ChID ([`Mirror::place_video_object`];
    /// docs/caraxes/video-engines.md §3).
    video_pending: BTreeMap<u32, (u32, u32, Vec<u8>)>,
    /// Place video objects on the host ([`Mirror::place_video_object`]);
    /// `CARAXES_MIRROR_VIDEO_OBJECT=0` refuses every one to the guest.
    video_objects: bool,
    /// `CARAXES_SHADOW_FENCE=1` (shadow.rs [`crate::shadow::Layout::fenced`]): GR and CE shadow
    /// VA spaces fence the guest RM's kernel range, and set B moves above it.
    shadow_fence: bool,
    next_event_id: u32,
    pub stats: MirrorStats,
    /// The owned-GFXP slots in the VM's slice (gfxpslot.rs); `None`: every host GR context
    /// stays in graphics WFI.
    slots: Option<crate::gfxpslot::Slots>,
    /// The end of the guest's usable FB (fb.rs `FbLayout::usable`): the device CE's window.
    usable_end: u64,
    /// GFXP binds done at a host TSG's build, and TSGs left in WFI for want of a slot.
    pub gfxp_bound: u64,
    pub gfxp_no_slot: u64,
    /// Steps 0 and 1 of the GR preemption fix (preempt.rs): the counters of the exit summary.
    preempt: crate::preempt::Stats,
    /// Guest roots whose shadow VAS was handed to the teardown: the FB store drops their
    /// cached walks ([`Mirror::take_reaped_roots`]; before, a walk cache of up to 4M leaves
    /// stayed for every context that ever ran).
    reaped_roots: Vec<u64>,
    /// Guest roots whose held set-B ranges were released at a host TSG free
    /// ([`Mirror::take_released_roots`]).
    released_roots: Vec<u64>,
    /// Guest channels whose shadow build the host refused for memory (NO_MEMORY or the
    /// host VRAM floor): the caller RCs them once ([`Mirror::take_exhausted`]) so the guest
    /// sees an error instead of a channel that never runs.
    exhausted: std::collections::BTreeSet<u32>,
    exhausted_raised: std::collections::BTreeSet<u32>,
}

// SAFETY: the doorbell mapping (`CpuMap`, a raw pointer) is only ever written from the
// server thread that owns the device; the event thread holds no reference to it.
unsafe impl Send for Mirror {}

fn status_name(st: u32) -> String {
    match nvrm_abi::status_name(st) {
        Some(n) => format!("{st:#x} {n}"),
        None => format!("{st:#x}"),
    }
}

fn as_bytes<T>(p: &mut T) -> &mut [u8] {
    // SAFETY: the callers pass plain-data RM parameter structs, viewed as bytes for the
    // raw escapes; the view lives as long as the borrow.
    unsafe { std::slice::from_raw_parts_mut(p as *mut T as *mut u8, std::mem::size_of::<T>()) }
}

impl Mirror {
    pub fn new(
        hm: Arc<Mutex<HostMemory>>,
        intr: Arc<IntrShared>,
        chip: &'static ChipData,
        layout: &FbLayout,
    ) -> Result<Self, String> {
        let (host_base, host_end, reservation, grant) = {
            let h = hm.lock().unwrap();
            let r = h
                .reservation()
                .ok_or("the mirror needs the VRAM reservation (--reserve-vram)")?;
            (r.base, r.end(), r.handle, r.compression.clone())
        };
        let (layout_host_base, layout_host_end) = layout.host_reservation
            .ok_or("the mirror needs an FB layout backed by a host VRAM reservation")?;
        if (layout_host_base, layout_host_end) != (host_base, host_end) {
            return Err(format!("mirror host reservation [{host_base:#x}, {host_end:#x}) does not match the FB layout [{layout_host_base:#x}, {layout_host_end:#x})"));
        }
        let (base, end) = layout.reservation
            .ok_or("the mirror needs a guest-visible FB reservation")?;
        if end <= base || end - base != host_end - host_base {
            return Err(format!("mirror guest reservation [{base:#x}, {end:#x}) is not the size of the host reservation [{host_base:#x}, {host_end:#x})"));
        }
        let translated = (base, end) != (host_base, host_end);
        let shadow_probe = std::env::var("CARAXES_MIRROR_GR_SHADOW").as_deref() == Ok("1");
        let ce_shadow = std::env::var("CARAXES_CE_SHADOW").as_deref() != Ok("0");
        let mirror_gr = std::env::var("CARAXES_MIRROR_GR").as_deref() == Ok("1");
        if translated && (!shadow_probe || !ce_shadow || mirror_gr) {
            return Err("a translated FB reservation needs shadow-only host execution: CARAXES_MIRROR_GR_SHADOW=1, CARAXES_CE_SHADOW!=0, and CARAXES_MIRROR_GR!=1".into());
        }
        if translated && crate::device::doorbell_ioeventfd_requested() {
            return Err("a translated FB reservation refuses the token-less doorbell ioeventfd; use trapped or token-pool doorbells so shadow routing is enforced".into());
        }
        // D16b invariant (b), again where the host channels are made (main.rs checks it at
        // start): a compressed reservation allows shadow-only execution.
        let comp = match grant {
            Some(g) => {
                crate::compslice::check_shadow_only(
                    crate::compslice::Mode::Slice,
                    shadow_probe.then_some("1"),
                    (!ce_shadow).then_some("0"),
                    mirror_gr.then_some("1"),
                    crate::device::doorbell_ioeventfd_requested(),
                )?;
                if base.abs_diff(host_base) % crate::compslice::COMPR_PAGE != 0 {
                    return Err(format!("a compressed reservation needs guest and host FB addresses 64 KiB aligned to each other (guest {base:#x}, host {host_base:#x})"));
                }
                Some(Arc::new(crate::compslice::Policy::new(&g)?))
            }
            None => None,
        };
        let ring_gate = crate::ringgate::global();
        let worker = ShadowWorker::new(hm.clone(), intr.clone(), ring_gate.clone());
        // Owned GFXP (gfxpslot.rs): the layout's slots, with the host's buffer geometry.
        let slots = layout.gfxp_range().and_then(|range| {
            match crate::gfxpslot::host_geometry(&hm.lock().unwrap()) {
                Ok(g) if g.slot <= range.1 - range.0 => Some(crate::gfxpslot::Slots::new(range, g)),
                Ok(g) => {
                    eprintln!("vfio-user-nvgpu: WARNING gfxp slots: slot of {:#x} bytes above the carve-out {:#x}; none", g.slot, range.1 - range.0);
                    None
                }
                Err(e) => {
                    eprintln!("vfio-user-nvgpu: WARNING gfxp slots: none ({e})");
                    None
                }
            }
        });
        let usable_end = layout.usable().map_or(end, |(_, e)| e);
        Ok(Mirror {
            hm,
            intr,
            chip,
            base,
            end,
            translated,
            comp,
            reservation,
            extents: GlobalExtents::default(),
            vas: BTreeMap::new(),
            tsgs: BTreeMap::new(),
            channels: BTreeMap::new(),
            userd_cursor: 0,
            watch_id: crate::ptcache::fresh_generation(),
            capture_deferred: std::env::var("CARAXES_DEFERRED_API_CAPTURE").as_deref() == Ok("1"),
            // GR channels on the host (the c2 experiment, exec-formats.md §4.5): the
            // replayed shared-management VAS plus a second `GPU_PROMOTE_CTX` on the host
            // channel carrying the guest's own recorded entries. Off by default (the GR
            // context wall); `CARAXES_MIRROR_GR=1` turns it and the VAS replay on.
            replay_vas: mirror_gr,
            mirror_gr,
            mirror_ce: std::env::var("CARAXES_MIRROR_CE").as_deref() != Ok("0"),
            channels_enabled: std::env::var("CARAXES_MIRROR_CHANNELS").as_deref() == Ok("1"),
            doorbell: None,
            events: None,
            invalidator: None,
            device_ce: None,
            device_ce_tried: false,
            memop_invalidate: std::env::var("CARAXES_MEMOP_INVALIDATE").as_deref() != Ok("0"),
            shadow_probe,
            bare_ce: std::env::var("CARAXES_MIRROR_BARE_CE").as_deref() == Ok("1"),
            ce_shadow,
            shadows: BTreeMap::new(),
            map_budget: crate::shadow::MapBudget::from_env(),
            worker,
            ring_gate,
            held_rings: BTreeMap::new(),
            ring_stats: Default::default(),
            builds: BTreeMap::new(),
            prepare_async: false,
            tsg_cells: BTreeMap::new(),
            build_stats: Default::default(),
            next_build: 0,
            committed: Vec::new(),
            lazy: match crate::lazy::LazyPlans::from_env() {
                Ok(l) => {
                    if l.enabled() {
                        eprintln!(
                            "vfio-user-nvgpu: alloc lazy: on, shadow plans at the next shadow doorbell or channel creation, deadline {} [{}]",
                            l.deadline().map_or("none".into(), |d| format!("{} ms", d.as_millis())),
                            crate::lazy::ENV
                        );
                    }
                    l
                }
                Err(e) => {
                    eprintln!("vfio-user-nvgpu: ERROR alloc lazy: {e}; shadow plans stay at every invalidate");
                    crate::lazy::LazyPlans::new(false, None)
                }
            },
            hold_on: match crate::untrapped::parse_switch(
                std::env::var(crate::untrapped::ENV).ok().as_deref(),
            ) {
                Ok(on) => {
                    if !on {
                        eprintln!(
                            "vfio-user-nvgpu: WARN untrapped GPPut: rule OFF ({}=0): an invalidate of a root whose channel has an untrapped USERD is not held (A/B only; docs/caraxes/bar1-pool.md §9)",
                            crate::untrapped::ENV
                        );
                    }
                    on
                }
                Err(e) => {
                    eprintln!("vfio-user-nvgpu: ERROR untrapped GPPut: {e}; the rule stays on");
                    true
                }
            },
            hold: crate::untrapped::HoldStats::default(),
            held_scope: (0, 0, 0),
            inv_async: match crate::invgate::parse_switch(
                std::env::var(crate::invgate::ENV).ok().as_deref(),
            ) {
                Ok(on) => {
                    if !on {
                        eprintln!(
                            "vfio-user-nvgpu: WARN held invalidates wait under the device lock ({}=0, A/B only; docs/caraxes/execution.md §8)",
                            crate::invgate::ENV
                        );
                    }
                    on
                }
                Err(e) => {
                    eprintln!("vfio-user-nvgpu: ERROR held invalidate: {e}; it waits under the device lock");
                    false
                }
            },
            inv_gate: Default::default(),
            unwaited: 0,
            descs: None,
            video_pending: BTreeMap::new(),
            video_objects: std::env::var("CARAXES_MIRROR_VIDEO_OBJECT").as_deref() != Ok("0"),
            shadow_fence: std::env::var(crate::shadow::FENCE_ENV).as_deref() == Ok("1"),
            next_event_id: 0x1000,
            stats: MirrorStats::default(),
            slots,
            usable_end,
            gfxp_bound: 0,
            gfxp_no_slot: 0,
            preempt: crate::preempt::Stats::default(),
            reaped_roots: Vec::new(),
            released_roots: Vec::new(),
            exhausted: Default::default(),
            exhausted_raised: Default::default(),
        })
    }

    /// Guest roots whose shadow VAS went to the teardown since the last call: the caller
    /// drops their cached walks (`FakeFb::unwatch_tables`).
    pub fn take_reaped_roots(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.reaped_roots)
    }

    /// Guest channels to RC because the host refused their shadow build for memory; each
    /// channel once until the guest frees it.
    pub fn take_exhausted(&mut self) -> Vec<u32> {
        let out: Vec<u32> = std::mem::take(&mut self.exhausted)
            .into_iter()
            .filter(|c| self.exhausted_raised.insert(*c))
            .collect();
        out
    }

    /// Counts for the soak line: shadow VA spaces (front side), CE mirror VA spaces, host
    /// TSGs, host channels.
    pub fn soak_counts(&self) -> (usize, usize, usize, usize) {
        (self.shadows.len(), self.vas.len(), self.tsgs.len(), self.channels.len())
    }

    /// A shadow build of `chid` on `root` that did not finish: the channel leaves the
    /// shadow space's set, and so does its TSG key when no host TSG stands under it, so an
    /// empty shadow VAS is torn down instead of held until the device exits.
    fn abandon_shadow_build(&mut self, root: u64, chid: u32, tsg_key: TsgKey, notes: &mut Vec<String>) {
        let has_tsg = self.tsgs.contains_key(&tsg_key)
            || self.builds.values().any(|b| b.tsg_key == tsg_key);
        if let Some(sp) = self.shadows.get_mut(&root) {
            sp.chids.remove(&chid);
            if !has_tsg {
                sp.tsgs.remove(&tsg_key);
            }
        }
        self.reap_shadows(None, notes);
    }

    pub fn is_mirrored(&self, chid: u32) -> bool {
        self.channels.contains_key(&chid)
    }

    /// The size of a global GR context buffer (by `NV2080_CTRL_GPU_PROMOTE_CTX_BUFFER_ID_*`)
    /// on the host, in bytes: `NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES`
    /// (NON_PRIVILEGED) {engineId @0, alignment @4, size @8} on the device's own host device,
    /// as `(status, size)`. Each host answer is logged once (the query is cached per engine,
    /// [`GlobalCtxSizeCache`]).
    fn host_ctx_size(hm: &Mutex<HostMemory>, buffer_id: u16, engine_id: u32) -> (u32, u32) {
        let h = hm.lock().unwrap();
        let mut p = [0u8; 12];
        p[..4].copy_from_slice(&engine_id.to_le_bytes());
        let status = h.control(h.device(), 0x0080_1707, &mut p);
        let align = u32::from_le_bytes(p[4..8].try_into().unwrap());
        let size = u32::from_le_bytes(p[8..12].try_into().unwrap());
        eprintln!(
            "vfio-user-nvgpu: global context size: buffer {buffer_id} host engine {engine_id:#x} (FIFO_GET_ENGINE_CONTEXT_PROPERTIES): status {status:#x}, size {size:#x}, alignment {align:#x} [host read]"
        );
        (status, size)
    }

    /// Why the raw paths are refused (a host VAS or channel on the guest's own tables, raw
    /// PDE replay and forwarding, the raw physical invalidator): a translated FB
    /// reservation, whose guest addresses are not host addresses, or a compressed one
    /// (D16b), whose guest COMPTAGLINE must never reach a host walker. `None`: allowed.
    fn raw_refusal(&self) -> Option<&'static str> {
        if self.translated {
            Some("a translated FB reservation")
        } else if self.comp.is_some() {
            Some("a compressed FB reservation (D16b)")
        } else {
            None
        }
    }

    /// The exclusion size of size-0 global buffer `buffer_id` at `va` on shadow root `root`
    /// ([`GlobalExtents::extent`]).
    fn global_extent(
        &self,
        fb: Option<&FakeFb>,
        root: u64,
        buffer_id: u16,
        va: u64,
    ) -> Option<u64> {
        // A fenced shadow (F2b): a size-0 global inside the fence is plain set A.
        if self.fence_of(root).is_some_and(|(lo, hi)| va >= lo && va < hi) {
            return Some(0);
        }
        let hm = &self.hm;
        self.extents
            .extent(fb, root, buffer_id, va, &|b, e| Self::host_ctx_size(hm, b, e))
    }

    /// Whether `chid` is a GR channel the shadow path takes (`CARAXES_MIRROR_GR_SHADOW=1`).
    pub fn is_gr_shadow(&self, reg: &Registry, chid: u32) -> bool {
        if !self.shadow_probe {
            return false;
        }
        let Some(ch) = reg.channel(chid) else {
            return false;
        };
        let gr = v::lookup(v::NV2080_ENGINE_TYPES, "GR0").unwrap_or(1);
        reg.tsg((ch.h_client, ch.h_parent))
            .is_some_and(|(_, e)| e == gr)
            || ch
                .engine_objects
                .iter()
                .any(|(c, _)| *c == self.chip.channel.compute)
    }

    /// Whether `chid` runs on the shadow VAS of its root: every GR channel under
    /// `CARAXES_MIRROR_GR_SHADOW=1`, and every other channel the mirror takes (CUDA's copy
    /// engines) too, so that no host channel walks the guest's unfiltered tables
    /// (parallel.md §8.2, D17 stage 2; docs/caraxes/mapping.md §5).
    /// `CARAXES_CE_SHADOW=0` keeps CE channels on the externally-owned VAS (the guest root).
    pub fn is_shadow(&self, reg: &Registry, chid: u32) -> bool {
        if self.is_gr_shadow(reg, chid) {
            return true;
        }
        if let Some(c) = self.channels.get(&chid) {
            return c.shadow_root.is_some();
        }
        self.shadow_probe && self.ce_shadow && self.eligible(reg, chid).is_ok()
    }

    /// Harvest private USERDs and find new submissions without requiring a trapped RAM
    /// write. At most 32 channels per tick, round-robin. The caller routes each submission
    /// through its normal disabled-channel checks and the shadow mapping barrier.
    /// Publish this mirror's private USERDs to the poller's watch list (after a channel
    /// was added or removed): the poller then polls them without the device lock.
    fn publish_userd_watch(&self) {
        let mut w = userd_watch_list().lock().unwrap_or_else(PoisonError::into_inner);
        w.retain(|(id, _, _)| *id != self.watch_id);
        w.extend(
            self.channels
                .iter()
                .filter_map(|(&chid, hc)| hc.userd.as_ref().map(|s| (self.watch_id, chid, s.clone()))),
        );
    }

    /// Whether channel `chid`'s private USERD has entries the device has not forwarded,
    /// checked as the locked poll checks it (ready, the guest's channel unchanged), with the
    /// GPU-owned words harvested first: the USERD poller's ring under the device lock.
    pub fn userd_pending(&mut self, reg: &Registry, gm: &mut GpuMem, chid: u32) -> bool {
        let Some(hc) = self.channels.get(&chid) else {
            return false;
        };
        if !hc.context_ready || reg.channel(chid).is_none_or(|ch| ch.generation != hc.generation) {
            return false;
        }
        let sys = gm.sys;
        hc.with_userd(|u| {
            u.harvest_sys(sys);
            u.pending(sys)
        }) == Some(true)
    }

    pub fn poll_userds(&mut self, reg: &Registry, gm: &mut GpuMem) -> Vec<u32> {
        use std::ops::Bound::{Excluded, Unbounded};
        let ids: Vec<u32> = self.channels.range((Excluded(self.userd_cursor), Unbounded))
            .chain(self.channels.range(..=self.userd_cursor))
            .filter(|(_, hc)| hc.userd.is_some())
            .take(32).map(|(&chid, _)| chid).collect();
        let mut pending = Vec::new();
        for chid in ids {
            self.userd_cursor = chid;
            let hc = self.channels.get(&chid).unwrap();
            if !hc.context_ready || reg.channel(chid).is_none_or(|ch| ch.generation != hc.generation) { continue }
            let sys = gm.sys;
            if hc.with_userd(|u| {
                u.harvest_sys(sys);
                u.pending(sys)
            }) == Some(true)
            {
                pending.push(chid);
            }
        }
        pending
    }

    /// A guest may replace a ChID without FREE, even with identical handle values.
    /// Retire the previous host allocation before answering the replacement allocation.
    pub fn retire_replaced_channels(&mut self, reg: &Registry) -> Vec<String> {
        let stale: Vec<_> = self.channels.iter().filter(|(chid, hc)| {
            reg.channel(**chid).is_none_or(|ch| ch.generation != hc.generation)
        }).map(|(&chid, _)| chid).collect();
        let mut notes = Vec::new();
        for chid in stale {
            self.free_channel(chid, &mut notes);
        }
        notes
    }

    /// Re-sync the shadow VAS of guest root `root` (`None`: every shadow VAS) from the
    /// guest's tables (called at a guest TLB invalidate and before a shadow doorbell): map
    /// what the guest newly mapped, unmap what it changed or removed.
    pub fn shadow_resync(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        root: Option<u64>,
    ) -> Vec<String> {
        let (base, end, reservation) = (self.base, self.leaf_end(), self.reservation);
        let descs = self.descs();
        let mut out = Vec::new();
        // The guest's page-table writes through the BAR2 memfd reach the walks first
        // (bar2-memfd.md §3 rule 2): a guest that updates its tables through BAR2 and
        // invalidates in a pushbuffer (the Windows KMD's paging channel, or a MEM_OP a
        // host channel runs where the device cannot see it) has no register invalidate that
        // would have synced them. The live table pages are compared and the changed ones
        // marked dirty in the walks, so the plan below re-reads them (C80: without this,
        // host Xid 31 at the KMD's GPFIFO and USERD VAs).
        gm.fb.bar2_sync_tables(None, Some(gm.sys));
        // The exclusions follow the guest's live channels (`exclusions_of`).
        let roots: Vec<u64> = self.shadows.keys().copied().filter(|r| root.is_none_or(|x| x == *r)).collect();
        for r in &roots {
            out.extend(self.refresh_exclusions(reg, *r, gm.fb).0);
        }
        for (r, sp) in self.shadows.iter_mut() {
            if root.is_some_and(|x| x != *r) {
                continue;
            }
            if let Some(plan) =
                sp.front
                    .plan(gm.fb, gm.sys, base, end, reservation, &sp.exclude, &descs)
            {
                if !plan.is_empty() {
                    self.worker.submit(Job::Apply(plan, Instant::now()));
                }
            }
        }
        // The plans brought the cached walks up to date. A guest page they walked for the
        // first time can cut a global buffer's exclusion short (`global_exclusion`): set B
        // changes, and the root is planned again with it (run P19).
        for r in roots {
            let (notes, changed) = self.refresh_exclusions(reg, r, gm.fb);
            out.extend(notes);
            let Some(sp) = self.shadows.get_mut(&r).filter(|_| changed) else {
                continue;
            };
            if let Some(plan) =
                sp.front
                    .plan(gm.fb, gm.sys, base, end, reservation, &sp.exclude, &descs)
            {
                if !plan.is_empty() {
                    self.worker.submit(Job::Apply(plan, Instant::now()));
                }
            }
        }
        if !self.worker.enabled {
            self.worker.wait_all();
        }
        out.append(&mut Vec::new());
        out
    }

    /// A guest MMU invalidate of `pdb` (`None`: `ALL_PDB`): re-sync the shadows now, or,
    /// with `CARAXES_ALLOC_LAZY=1`, record the root for the next shadow doorbell or channel
    /// creation (lazy.rs); past the deadline, plan everything pending now.
    pub fn shadow_invalidated(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        pdb: Option<u64>,
    ) -> Vec<String> {
        // A root whose channel's GPPut store does not trap (untrapped.rs): the invalidate is
        // the last point before the host channel may run the guest's next work, so the plan
        // is made and applied before the guest continues, with everything deferred so far.
        let untrapped = if self.hold_on {
            self.untrapped_roots(reg, gm.fb)
        } else {
            Default::default()
        };
        if self.hold_on && crate::untrapped::must_hold(pdb, &untrapped) {
            // The invalidate's scope (docs/caraxes/execution.md §8): the card completes it
            // when the TLB holds no stale translation for its target. The target root, or
            // for ALL_PDB every root whose host channel may fetch without a trapped
            // doorbell, is planned now and TRIGGER reads busy until the worker applied
            // those roots' jobs, which it takes before other roots' (invgate.rs). Other
            // roots are not in the hold: their host channels run only after a doorbell,
            // whose barrier plans and waits for them, so they follow the lazy rule below.
            let t0 = Instant::now();
            let held = crate::untrapped::held_roots(pdb, &untrapped);
            match pdb {
                Some(_) => self.held_scope.0 += 1,
                None => {
                    self.held_scope.1 += 1;
                    self.held_scope.2 += held.len() as u64;
                }
            }
            let mut notes = Vec::new();
            for r in &held {
                notes.extend(self.shadow_resync(reg, gm, Some(*r)));
            }
            let targets = self.worker.targets(&held);
            if self.inv_async {
                // The guest polls TRIGGER until it reads FALSE before its map call returns
                // (`kgmmuCommitTlbInvalidate_TU102`) ([`Mirror::invalidate_busy`]). The
                // device lock is free meanwhile.
                self.inv_gate.arm(targets, t0, &self.worker.progress);
            } else {
                self.worker
                    .progress
                    .wait(&crate::invgate::Target::Roots(targets));
                self.hold.note_held(t0.elapsed());
            }
            if pdb.is_none() {
                notes.extend(self.invalidate_unheld(reg, gm, None));
            }
            return notes;
        }
        self.invalidate_unheld(reg, gm, pdb)
    }

    /// An invalidate outside the hold (`shadow_invalidated`): planned now without a wait,
    /// or deferred (lazy.rs).
    fn invalidate_unheld(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        pdb: Option<u64>,
    ) -> Vec<String> {
        use crate::lazy::OnInvalidate;
        // A plan submitted here is not waited for before the guest continues (the evidence
        // counter of `note_ran_ahead`).
        match self.lazy.on_invalidate(pdb, Instant::now()) {
            OnInvalidate::Plan(p) => {
                self.unwaited += 1;
                self.shadow_resync(reg, gm, p)
            }
            OnInvalidate::Defer => Vec::new(),
            OnInvalidate::Flush(p) => {
                self.unwaited += 1;
                let mut notes = vec![format!(
                    "alloc lazy: deadline passed, planning {} deferred invalidate(s) now",
                    p.invalidates
                )];
                notes.extend(self.resync_pending(reg, gm, &p));
                notes
            }
        }
    }

    /// Plan every root a lazy batch names (lazy.rs).
    fn resync_pending(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        p: &crate::lazy::Pending,
    ) -> Vec<String> {
        match p.roots() {
            None => self.shadow_resync(reg, gm, None),
            Some(roots) => roots
                .into_iter()
                .flat_map(|r| self.shadow_resync(reg, gm, Some(r)))
                .collect(),
        }
    }

    /// Before a shadow channel runs (its doorbell or creation): plan what lazy mode
    /// deferred, for every root, so no host channel runs on a mirror older than the
    /// guest's last invalidate (the ordering rule of `gr_shadow_doorbell`).
    fn flush_lazy(&mut self, reg: &Registry, gm: &mut GpuMem) -> Vec<String> {
        match self.lazy.take() {
            Some(p) => self.resync_pending(reg, gm, &p),
            None => Vec::new(),
        }
    }

    /// The shadow roots whose software `MEM_OP` TLB invalidate stops its push until the
    /// shadow is applied (channel.rs `doorbell_holding`): every shadow root while the rule
    /// is on (untrapped.rs `memop_holds`), `None` otherwise.
    pub fn memop_hold_roots(&self) -> Option<std::collections::BTreeSet<u64>> {
        (self.hold_on && !self.shadows.is_empty()).then(|| self.shadows.keys().copied().collect())
    }

    /// `MEM_OP` TLB invalidates the software executor ran (`inv`: the page directory each
    /// names; nvidia-uvm's page-table updates of a CUDA VA space end in one): plan every
    /// shadow root they name now and wait until the worker applied it. The channel that ran
    /// them is stopped right after the invalidate meanwhile (channel.rs `doorbell_holding`),
    /// so UVM's semaphore release after the update reaches the guest only once the host
    /// mappings exist, and no host channel can run the guest's next work on an older shadow
    /// (docs/research/shadow-fault-race-2026-09-28.md). Before, nothing planned a root at
    /// such an invalidate: its next doorbell did, while a host channel with an untrapped
    /// USERD could already run the new work.
    pub fn software_invalidated(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        inv: &[Option<Target>],
    ) -> Vec<String> {
        let Some(held) = self.memop_hold_roots() else {
            return Vec::new();
        };
        let roots = crate::untrapped::memop_roots(inv, &held);
        if roots.is_empty() {
            return Vec::new();
        }
        let t0 = Instant::now();
        let mut notes = Vec::new();
        for r in &roots {
            notes.extend(self.shadow_resync(reg, gm, Some(*r)));
        }
        for r in &roots {
            self.worker.wait_root(*r);
        }
        let d = t0.elapsed();
        self.hold.note_memop_held(d);
        notes.push(format!(
            "shadow: {} software TLB invalidate(s) of shadow root(s) {} planned and applied in {} us before the channel continues [host]",
            inv.len(),
            roots.iter().map(|r| format!("{r:#x}")).collect::<Vec<_>>().join(", "),
            d.as_micros()
        ));
        notes
    }

    /// The untrapped-GPPut counters (untrapped.rs), for the pool's per-context line.
    pub fn hold_summary(&self) -> String {
        format!(
            "{}; held invalidates {} (trigger read busy {} time(s), {} waited past {} ms) [{}], by scope: {} of one root, {} ALL_PDB holding {} untrapped root(s) in total; {}; {}",
            self.hold.summary(self.hold_on),
            if self.inv_async { "complete through the trigger's busy bit" } else { "wait under the device lock" },
            self.inv_gate.busy_reads,
            self.inv_gate.fallbacks,
            crate::invgate::FALLBACK.as_millis(),
            crate::invgate::ENV,
            self.held_scope.0,
            self.held_scope.1,
            self.held_scope.2,
            self.worker.summary(),
            self.ring_summary()
        ) + "; " + &self.build_summary()
    }

    /// A read of the invalidate trigger: whether the held invalidate in flight is still
    /// being applied (TRIGGER reads TRUE). The first read that finds it applied counts the
    /// hold with its time from the trigger write (invgate.rs).
    /// The held invalidate in flight, handed off the device lock (the invalidate page,
    /// inv_page.rs, or the trapped trigger write for the reads without the lock, fastread.rs):
    /// whoever completes TRIGGER waits for the worker.
    pub fn hand_off_invalidate(&mut self) -> Option<crate::invgate::Hold> {
        self.inv_gate.hand_off(&self.worker.progress)
    }

    pub fn invalidate_busy(&mut self) -> bool {
        match self.inv_gate.read(&self.worker.progress) {
            crate::invgate::Trigger::Busy => true,
            crate::invgate::Trigger::Done(Some(d)) => {
                self.hold.note_held(d);
                crate::profile::record(
                    crate::profile::Key::Window("held invalidate: armed to TRIGGER read done"),
                    d,
                    1,
                    None,
                    "",
                );
                false
            }
            crate::invgate::Trigger::Done(None) => false,
        }
    }

    /// Whether plans are pending: deferred by lazy mode, or submitted and not yet applied.
    pub fn plans_pending(&self) -> bool {
        self.lazy.is_pending() || self.worker.pending()
    }

    /// The trapped GPPut store's barrier (boot.rs `gp_put_barrier`): plan what lazy mode
    /// deferred and wait until the worker applied every plan of shadow root `root` (`None`:
    /// every plan; a channel without a shadow root, or a store that hits several roots).
    pub fn plans_barrier(&mut self, reg: &Registry, gm: &mut GpuMem, root: Option<u64>) -> Vec<String> {
        let notes = self.flush_lazy(reg, gm);
        match root {
            Some(r) => self.worker.wait_root(r),
            None => self.worker.wait_all(),
        }
        notes
    }

    /// The one shadow root of the mirrored channels whose USERD GPPut a guest store of
    /// `len` bytes at FB address `pa` hits; `None` when it hits none, a channel without a
    /// shadow root, or channels of several roots (the caller then waits for everything).
    pub fn gp_put_root(&self, reg: &Registry, pa: u64, len: usize) -> Option<u64> {
        let mut roots = self.channels.iter().filter_map(|(chid, c)| {
            match reg.channel(*chid).and_then(|ch| ch.userd) {
                Some(Target::Fb(u)) if crate::boot::stores_gp_put(&[u], pa, len) => Some(c.shadow_root),
                _ => None,
            }
        });
        let first = roots.next()??;
        roots.all(|r| r == Some(first)).then_some(first)
    }

    /// The shadow roots of mirrored channels whose USERD the guest writes without a trap
    /// (untrapped.rs): a USERD page that lives in the BAR1 pool (the guest CPU reaches it
    /// through the pool area), or a USERD in guest RAM.
    pub fn untrapped_roots(&self, reg: &Registry, fb: &FakeFb) -> std::collections::BTreeSet<u64> {
        self.channels
            .iter()
            .filter_map(|(chid, c)| {
                let r = c.shadow_root?;
                match reg.channel(*chid).and_then(|ch| ch.userd)? {
                    Target::Fb(u) if fb.remap(u) != u => Some(r),
                    Target::Sys(_) => Some(r),
                    _ => None,
                }
            })
            .collect()
    }

    /// `(GPGet, GPPut, GPFIFO entries)` of guest channel `chid`, read from its USERD where
    /// the device finds it (the pool page for a USERD in the pool: the host channel's own).
    fn userd_ring(reg: &Registry, gm: &GpuMem, chid: u32) -> Option<(u32, u32, u32)> {
        let ch = reg.channel(chid)?;
        let at = |o: u64| match ch.userd {
            Some(Target::Fb(pa)) => Some(Target::Fb(pa + o)),
            Some(Target::Sys(pa)) => Some(Target::Sys(pa + o)),
            _ => None,
        };
        if crate::launch::fast() {
            // GPGet and GPPut are adjacent dwords of one aligned qword: one uncached read.
            let (get, put) = crate::launch::userd_pair(|b| Some(gm.read(at(USERD_GP_GET)?, b)))?;
            return Some((get, put, ch.gpfifo_entries));
        }
        Some((
            gm.read_u32(at(USERD_GP_GET)?)?,
            gm.read_u32(at(USERD_GP_PUT)?)?,
            ch.gpfifo_entries,
        ))
    }

    /// The guest's USERD GPPut of `chid` alone (one read; the forward record of the fast
    /// ring path, launch.rs `FAST_ENV`).
    fn userd_put(reg: &Registry, gm: &GpuMem, chid: u32) -> Option<u32> {
        match reg.channel(chid)?.userd? {
            Target::Fb(pa) => gm.read_u32(Target::Fb(pa + USERD_GP_PUT)),
            Target::Sys(pa) => gm.read_u32(Target::Sys(pa + USERD_GP_PUT)),
            _ => None,
        }
    }

    /// At a doorbell of a built shadow channel, before anything is planned: count it when
    /// the host channel already fetched entries past the last forward (untrapped.rs
    /// `ran_ahead`), and log it with the wall time when plans were pending.
    fn note_ran_ahead(&mut self, reg: &Registry, gm: &GpuMem, chid: u32) -> Option<String> {
        let hc = self.channels.get(&chid)?;
        let (fwd, since) = (hc.fwd_put?, self.unwaited - hc.fwd_unwaited);
        // Plans deferred now, being applied now, or submitted without a wait since the last
        // forward: the entries the host channel fetched may have met an older mirror.
        let pending = self.plans_pending() || since > 0;
        // The lean evidence (launch.rs `evidence_needed`): no uncached read when nothing is
        // pending, the case that is only counted.
        if !crate::launch::evidence_needed(pending) {
            return None;
        }
        let (get, put, n) = Self::userd_ring(reg, gm, chid)?;
        if !crate::untrapped::ran_ahead(fwd, get, put, n) {
            return None;
        }
        let (lazy, worker) = (self.lazy.is_pending(), self.worker.pending());
        let root = self.channels.get(&chid).and_then(|c| c.shadow_root);
        // Plans of the channel's own root: the only ones its entries translate through
        // (a channel of root r uses r's shadow VAS only).
        let (own_lazy, own_worker) = root.map_or((lazy, worker), |r| {
            (self.lazy.is_pending_for(r), self.worker.pending_root(r))
        });
        let own = own_lazy || own_worker;
        let untrapped = root.is_some_and(|r| self.untrapped_roots(reg, gm.fb).contains(&r));
        let line = self.hold.note_ahead(pending, own, || {
            format!(
                "untrapped GPPut: at {} chid {chid}'s host channel ran ahead of its doorbell (GPGet {get}, forwarded up to {fwd}, GPPut {put}) with plans pending (lazy {lazy}, worker {worker}, {since} unwaited invalidate plan(s) since the forward; of its own root {:#x}: lazy {own_lazy}, worker {own_worker}); USERD untrapped {untrapped}",
                crate::untrapped::wall(),
                root.unwrap_or(0)
            )
        })?;
        eprintln!(
            "vfio-user-nvgpu: {} {line}",
            if own { "ERROR" } else { "WARN (other roots' plans only)" }
        );
        Some(line)
    }

    /// The registered guest RAM descriptors, read once (they do not change while the
    /// guest runs).
    fn descs(&mut self) -> Vec<(u32, u64, u64)> {
        if self.descs.is_none() {
            // Guest RAM answered at DMA_MAP and still being registered finishes first
            // (registrar.rs); its worker takes hostmem's lock, so wait without it.
            let registrar = self.hm.lock().unwrap().registrar();
            if let Some(r) = registrar {
                r.wait_idle();
            }
            let mut h = self.hm.lock().unwrap();
            let d: Vec<(u32, u64, u64)> = h
                .descriptors()
                .iter()
                .map(|d| (d.handle, d.range.iova, d.range.size))
                .collect();
            if !d.is_empty() {
                h.freeze_gpa_mappings();
                self.descs = Some(d);
            } else {
                return d;
            }
        }
        self.descs.clone().unwrap_or_default()
    }

    /// P21 design H (bar1pool.rs): every mirrored channel as `(guest chid, the guest root of
    /// its shadow VAS or None, its host TSG handle)`. A channel whose mirror build is in
    /// flight counts too (execution.md §9.2): its host channel takes the guest's USERD page
    /// as the snapshot found it, so the pool must not move that page before the commit; its
    /// TSG handle is known once the worker made it (and not waited for when the worker is
    /// making it).
    pub fn pool_channels(&self) -> Vec<(u32, Option<u64>, Option<u32>)> {
        let building = self.builds.iter().map(|(chid, b)| {
            let tsg = self
                .tsgs
                .get(&b.tsg_key)
                .map(|t| t.handle)
                .or_else(|| {
                    self.tsg_cells
                        .get(&b.tsg_key)
                        .and_then(|c| c.try_lock().ok().and_then(|c| c.handle))
                });
            (*chid, Some(b.root), tsg)
        });
        self.channels
            .iter()
            .map(|(chid, c)| (*chid, c.shadow_root, self.tsgs.get(&c.tsg).map(|t| t.handle)))
            .chain(building)
            .collect()
    }

    /// The set-B VA ranges of shadow root `root` (the promoted context buffers the host
    /// builds itself: the shadow VAS never maps the guest's pages there) and a stamp that
    /// changes whenever they do; `None` for a root without a shadow VAS.
    pub fn shadow_exclusions(&self, root: u64) -> Option<(u64, &[(u64, u64)])> {
        self.shadows
            .get(&root)
            .map(|sp| (sp.exclude_gen, sp.exclude.as_slice()))
    }

    /// P21 design H: the guest roots that have a shadow VAS.
    pub fn shadow_roots(&self) -> Vec<u64> {
        self.shadows.keys().copied().collect()
    }

    /// P21 design H: `NVA06C_CTRL_CMD_PREEMPT` with wait on host TSG `handle` (ctrla06c.h:
    /// 203-211, NON_PRIVILEGED): its channels are off the engines when this returns.
    pub fn preempt_tsg(&self, handle: u32) -> u32 {
        // bWait, bManualTimeout, pad, timeoutUs.
        let mut p = [1u8, 0, 0, 0, 0, 0, 0, 0];
        self.hm.lock().unwrap().control(handle, 0xA06C_0105, &mut p)
    }

    /// The BAR1 pool's barrier without the device lock (bar1-pool.md §15): the preempt of
    /// host TSG `handle` as a worker job in order with `root`'s plans. Returns the job
    /// (root, sequence number) and the slot its status lands in.
    pub fn submit_preempt(&mut self, root: u64, handle: u32) -> ((u64, u64), Arc<Mutex<Option<u32>>>) {
        let slot = Arc::new(Mutex::new(None));
        let out = slot.clone();
        let call: HostCall = Box::new(move |hm: &Mutex<HostMemory>| {
            // bWait, bManualTimeout, pad, timeoutUs (as `preempt_tsg`).
            let mut p = [1u8, 0, 0, 0, 0, 0, 0, 0];
            let st = hm.lock().unwrap().control(handle, 0xA06C_0105, &mut p);
            *out.lock().unwrap() = Some(st);
            Vec::new()
        });
        self.submit_call(root, call, None);
        ((root, self.worker.seqs.last_of(root)), slot)
    }

    /// Release ring hold `token` once `root`'s jobs submitted so far are applied (a job
    /// that does nothing but release it, in order with the root's plans).
    pub fn release_rings_after(&mut self, root: u64, token: u64) -> (u64, u64) {
        self.submit_call(root, Box::new(|_: &Mutex<HostMemory>| Vec::new()), Some(token));
        (root, self.worker.seqs.last_of(root))
    }

    /// Whether shadow jobs `jobs` ((root, sequence number): each root's jobs up to it) are
    /// applied, waiting at most `max` (`None`: to the end). True without a worker thread.
    pub fn shadow_jobs_applied(&self, jobs: &[(u64, u64)], max: Option<Duration>) -> bool {
        if self.worker.thread.is_none() || jobs.is_empty() {
            return true;
        }
        let t = crate::invgate::Target::Roots(jobs.to_vec());
        match max {
            None => {
                self.worker.progress.wait(&t);
                true
            }
            Some(m) => self.worker.progress.wait_at_most(&t, m),
        }
    }

    /// Whether the shadow worker runs (jobs submitted are applied at all).
    pub fn shadow_worker_running(&self) -> bool {
        self.worker.thread.is_some()
    }

    /// Wait until the worker applied everything submitted so far: the mirror of every
    /// root is then consistent with the guest's tables as of the last resync (the
    /// ordering guarantee a doorbell needs).
    pub fn shadow_barrier(&self) {
        self.worker.wait_all();
    }

    /// The BAR1 pool's question without a wait (docs/caraxes/bar1-pool.md §14): the newest
    /// shadow job not yet applied that may still map a page of `pages` on the host beyond
    /// what the cached walks show (a leaf its plan took out of the walk, a teardown).
    /// `None`: the cached walks show every host mapping of these pages.
    pub fn shadow_in_flight(&self, pages: &BTreeSet<u64>) -> Option<u64> {
        self.worker.in_flight(pages)
    }

    /// Wait until shadow job `seq` is applied, at most `max` (`None`: to the end).
    pub fn shadow_wait_applied(&self, seq: u64, max: Option<Duration>) -> bool {
        self.worker.wait_applied(seq, max)
    }

    /// The number of shadow jobs submitted so far. The cached walks of the shadow roots
    /// change only in a plan that is submitted (or with a root created or freed): while it
    /// stays the same, an answer of [`Self::shadow_in_flight`] and the walks stays valid.
    pub fn shadow_submitted(&self) -> u64 {
        self.worker.seqs.submitted()
    }

    /// Whether the worker has plans submitted and not yet applied (the GPPut barrier,
    /// boot.rs `bar_access`).
    pub fn shadow_pending(&self) -> bool {
        self.worker.pending()
    }

    /// The FB addresses of every mirrored channel's USERD. The host channel reads its
    /// GPPut there (the restore in `gr_shadow_doorbell`), so a guest GPPut store there must
    /// wait for the worker as the doorbell does: a loaded host channel can fetch the new
    /// GPPut without a doorbell (docs/caraxes/display-e.md §9, the Xid 31 of f2 and B1).
    pub fn mirrored_userds(&self, reg: &Registry) -> Vec<u64> {
        self.channels
            .keys()
            .filter_map(|chid| match reg.channel(*chid).and_then(|c| c.userd) {
                Some(Target::Fb(u)) => Some(u),
                _ => None,
            })
            .collect()
    }

    /// Free a shadow VAS whose last channel and TSG are gone.
    fn reap_shadows(&mut self, fb: Option<&crate::fb::FakeFb>, notes: &mut Vec<String>) {
        let empty: Vec<u64> = self
            .shadows
            .iter()
            .filter(|(_, sp)| sp.chids.is_empty() && sp.tsgs.is_empty())
            .map(|(r, _)| *r)
            .collect();
        for r in empty {
            if let Some(sp) = self.shadows.remove(&r) {
                // Freed by the worker in bounded batches, after everything queued before.
                // Until then the host maps every leaf of the root (bar1-pool.md §14).
                let leaves = fb.and_then(|fb| {
                    fb.with_walk(Target::Fb(r), |c| {
                        c.all_leaves()
                            .iter()
                            .filter_map(|l| match l.1 {
                                Target::Fb(pa) => Some((pa, pa.saturating_add(l.2))),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                    })
                });
                self.worker.submit_with(Job::Free(r), leaves);
                notes.push(format!(
                    "shadow: VAS {:#x} of guest root {r:#x} handed to the worker for an incremental teardown ({} resync(s) skipped clean on this thread) [host]",
                    sp.vas, sp.front.skipped
                ));
                match fb {
                    Some(fb) => fb.unwatch_tables(Target::Fb(r)),
                    None => self.reaped_roots.push(r),
                }
            }
        }
    }

    /// Build (once) and ring a GR channel on the host-managed shadow VAS: mirror the
    /// guest's set-A there, allocate the channel + its engine objects (the host RM builds
    /// the GR context, set B, in the shadow VAS), schedule, and forward the doorbell. The
    /// GR launch experiment (c1s, exec-formats.md §4.5). Returns the notes.
    pub fn gr_shadow_doorbell(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        chid: u32,
    ) -> Vec<String> {
        self.shadow_channel(reg, gm, chid, true)
    }

    fn shadow_channel(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        chid: u32,
        submit: bool,
    ) -> Vec<String> {
        if self.capture_deferred && reg.channel(chid).is_some_and(|ch| {
            ch.engine_objects.iter().any(|(class, _)| *class == 0x5080)
        }) {
            return vec![format!("deferred API capture: chid {chid} held before host submission (0x5080 object present)")];
        }
        let mut replacement_notes = Vec::new();
        if self.channels.get(&chid).is_some_and(|hc| {
            reg.channel(chid).is_none_or(|ch| ch.generation != hc.generation)
                || reg.channel(chid).and_then(|ch| reg.vas_key_of(ch))
                .and_then(|k| reg.vas_root(k)) != hc.shadow_root.map(Target::Fb)
        }) && !self.free_channel(chid, &mut replacement_notes)
        {
            return replacement_notes;
        }
        if self.channels.get(&chid).is_some_and(|hc| !hc.context_ready) && submit {
            return vec![format!("shadow: chid {chid} held until deferred context promotion")];
        }
        use crate::launch::{mark, Step};
        mark(Step::Dispatch);
        // Evidence (untrapped.rs): did the host channel fetch entries this doorbell has not
        // forwarded yet, and were plans pending then?
        let ahead_line = self.note_ran_ahead(reg, gm, chid);
        mark(Step::RanAhead);
        // CARAXES_ALLOC_LAZY: the invalidates deferred since the last doorbell are planned
        // here, before the barrier below or the creation's own wait.
        let mut lazy_notes = replacement_notes;
        lazy_notes.extend(self.flush_lazy(reg, gm));
        // Its mirror build in flight (execution.md §9.2): the doorbell joins the held ring.
        if self.builds.contains_key(&chid) {
            let mut notes = lazy_notes;
            notes.extend(ahead_line);
            notes.extend(self.build_doorbell(reg, gm, chid));
            return notes;
        }
        // Already built: re-sync the shadow VAS from the guest root FIRST (the guest maps
        // more set-A between doorbells), then flush and ring.
        if self.channels.contains_key(&chid) {
            let root = self.channels.get(&chid).and_then(|c| c.shadow_root);
            let mut notes = lazy_notes;
            notes.extend(ahead_line);
            // `shadow_resync` recomputes the set-B ranges of the root's live channels once:
            // a context promoted since the build leaves set A, and one whose channel the
            // guest freed comes back to it.
            notes.extend(self.shadow_resync(reg, gm, root));
            mark(Step::Plan);
            if !submit {
                match root {
                    Some(r) => self.worker.wait_root(r),
                    None => self.worker.wait_all(),
                }
                gm.fb.flush();
                notes.push(format!("shadow: chid {chid} context already prepared; no submission"));
                return notes;
            }
            // Ordering: the doorbell is forwarded only after the worker applied every plan
            // submitted so far for this root (the mirror is consistent for it; a held
            // invalidate of this root still in flight is among them, invgate.rs) and no hold
            // names the root. Not waited for under the device lock (execution.md §9.1): the
            // ring is held and the completer forwards it (ringgate.rs).
            let ring = crate::ringgate::HeldRing {
                root,
                target: match root {
                    Some(r) => crate::invgate::Target::Roots(vec![(r, self.worker.seqs.last_of(r))]),
                    None => crate::invgate::Target::All(self.worker.seqs.submitted()),
                },
                since: Instant::now(),
                doorbells: 1,
            };
            if !crate::ringgate::ready(&ring, &self.worker.progress, &self.ring_gate) {
                self.ring_stats.held += 1;
                match self.held_rings.get_mut(&chid) {
                    Some(h) => {
                        h.target = ring.target;
                        h.doorbells += 1;
                        self.ring_stats.coalesced += 1;
                    }
                    None => {
                        self.held_rings.insert(chid, ring);
                    }
                }
                self.update_waiting();
                notes.push(format!(
                    "shadow: chid {chid} ring held until root {:#x}'s jobs are applied [host]",
                    root.unwrap_or(0)
                ));
                return notes;
            }
            if let Some(h) = self.held_rings.remove(&chid) {
                // A ring held before is covered by this one (its jobs are older).
                self.note_ring_completed(&h);
                self.update_waiting();
            } else {
                self.ring_stats.direct += 1;
            }
            notes.extend(self.ring_now(reg, gm, chid));
            return notes;
        }
        let Some(ch) = reg.channel(chid).cloned() else {
            return vec![format!("shadow: chid {chid} not recorded")];
        };
        drop(ahead_line);
        self.shadow_build(reg, gm, chid, ch, submit, lazy_notes)
    }

    /// A held ring left the gate (`complete_held_rings`, or a later doorbell covered it).
    fn note_ring_completed(&mut self, h: &crate::ringgate::HeldRing) {
        let ns = h.since.elapsed().as_nanos() as u64;
        self.ring_stats.completed += 1;
        self.ring_stats.held_ns += ns;
        self.ring_stats.held_max_ns = self.ring_stats.held_max_ns.max(ns);
    }

    /// Forward the held rings whose root's jobs are applied and whose root no hold names
    /// (the completer thread, under the device lock briefly; ringgate.rs).
    pub fn complete_held_rings(&mut self, reg: &Registry, gm: &mut GpuMem) -> Vec<String> {
        // Builds the worker finished first: a commit may ring at once or hold its ring.
        let mut notes = self.commit_finished_builds(reg, gm);
        let ready: Vec<u32> = self
            .held_rings
            .iter()
            .filter(|(_, r)| crate::ringgate::ready(r, &self.worker.progress, &self.ring_gate))
            .map(|(c, _)| *c)
            .collect();
        for chid in ready {
            let Some(h) = self.held_rings.remove(&chid) else { continue };
            if !self.channels.contains_key(&chid) {
                self.ring_stats.dropped += 1;
                continue;
            }
            self.note_ring_completed(&h);
            notes.push(format!(
                "shadow: chid {chid} held ring forwarded after {} us ({} doorbell(s)) [host]",
                h.since.elapsed().as_micros(),
                h.doorbells
            ));
            notes.extend(self.ring_now(reg, gm, chid));
        }
        self.update_waiting();
        notes
    }

    /// The ring gate, for the completer thread (main.rs).
    pub fn ring_gate(&self) -> Arc<crate::ringgate::RingGate> {
        self.ring_gate.clone()
    }

    /// Hold the rings of `roots` until [`Self::release_rings`] or a [`Job::Call`] releases the
    /// token (the BAR1 pool, ringgate.rs).
    pub fn hold_rings(&self, roots: Vec<u64>) -> u64 {
        self.ring_gate.hold(roots)
    }

    pub fn release_rings(&self, token: u64) {
        self.ring_gate.release(token);
    }

    /// Run `call` on the worker in order with `root`'s plans, then release `release`.
    pub fn submit_call(&mut self, root: u64, call: HostCall, release: Option<u64>) {
        self.worker.submit(Job::Call(root, call, release));
    }

    /// The held-ring line of the exit summary.
    pub fn ring_summary(&self) -> String {
        self.ring_stats.line(self.held_rings.len())
    }

    /// Forward the doorbell of built channel `chid` now: flush the FB store, publish a
    /// shadowed USERD, record the forwarded GPPut, ring the host doorbell, harvest.
    fn ring_now(&mut self, reg: &Registry, gm: &mut GpuMem, chid: u32) -> Vec<String> {
        use crate::launch::{mark, Step};
        let mut notes = Vec::new();
        gm.fb.flush();
        mark(Step::Flush);
        {
            let published = self
                .channels
                .get(&chid)
                .and_then(|hc| hc.with_userd(|u| u.publish(gm)));
            if let Some(Err(e)) = published {
                notes.push(format!("shadow USERD chid {chid}: {e}; submission refused"));
                return notes;
            }
            let put = if crate::launch::fast() {
                Self::userd_put(reg, gm, chid)
            } else {
                Self::userd_ring(reg, gm, chid).map(|(_, p, _)| p)
            };
            mark(Step::Userd);
            let unwaited = self.unwaited;
            if let Some(hc) = self.channels.get_mut(&chid) {
                hc.fwd_put = put;
                hc.fwd_unwaited = unwaited;
            }
            match self.doorbell(chid) {
                Ok(n) => notes.push(n),
                Err(e) => notes.push(format!("shadow: doorbell chid {chid}: {e}")),
            }
            // The GPU-owned words back to the guest's USERD after the ring, off the
            // submission path (uncached reads of the host USERD; P37: 9.6 us before it).
            if let Some(hc) = self.channels.get(&chid) {
                hc.with_userd(|u| u.harvest(gm));
            }
        }
        notes
    }

    /// The first doorbell of channel `chid` (or its preparation, `submit` false): build its
    /// mirror on the host (VAS, TSG, channel, USERD, engine objects, scheduling).
    ///
    /// Off the device lock (execution.md §9.2): under the lock this takes the build's inputs
    /// from the guest's state (the VAS and its set-A plan, the USERD and error-notifier pages,
    /// the engine objects, the preemption request), and hands the host RM calls to the shadow
    /// worker as one job after the root's plans ([`build_on_host`]). The channel's ring is
    /// held until [`Self::commit_build`] schedules the channel and rings it, under the lock,
    /// at the completer's next pass (or at the channel's next doorbell). A preparation
    /// (`submit` false) waits for its build: its caller needs the host context at once.
    fn shadow_build(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        chid: u32,
        ch: crate::channel::Channel,
        submit: bool,
        lazy_notes: Vec<String>,
    ) -> Vec<String> {
        let since = Instant::now();
        let Some(vas_key) = reg.vas_key_of(&ch) else {
            return vec![format!("shadow: chid {chid} has no VA space")];
        };
        let root = match reg.vas_root(vas_key) {
            Some(Target::Fb(pa)) if pa >= self.base && pa + PAGE <= self.end => pa,
            other => {
                return vec![format!(
                    "shadow: chid {chid} root {other:?} not in the reservation"
                )]
            }
        };
        // The promoted-context (set-B) VA ranges of every live channel of this root
        // (`root_exclusions`): a GR channel that records no promote entries of its own must
        // still not have set A mirrored over its TSG's (measured 2026-09-23: chids 3-9
        // collided at 0x120010000 etc.). A global buffer (the 3D class's bundle/attribute/RTV
        // CBs, the pagepool) is promoted by VA only, size 0; its extent is the host's
        // context-buffer size for the same silicon (display-e.md: the guest's attribute CB at
        // 0x120400000 was mirrored as set A and the host's own collided with it, Xid 31
        // PRIV_VIOLATION).
        // The snapshot's phases, for the long-hold tag (dlock.rs).
        let mut phases: Vec<(&str, u128)> = Vec::new();
        let mut phase_t = Instant::now();
        let mut phase = |name: &'static str, phases: &mut Vec<(&str, u128)>| {
            phases.push((name, phase_t.elapsed().as_micros()));
            phase_t = Instant::now();
        };
        let new_b = self.root_exclusions(reg, root, Some(&*gm.fb));
        phase("set-B exclusions", &mut phases);
        // A bare CE channel (`bare_ce`, the GL driver's) gets a host TSG of its own, keyed
        // by its internal TSG id (615) or its handle, on the channel's own engine.
        let (tsg_key, bare, tsg_guest) = tsg_key_of(reg, &ch);
        // A sysmem USERD is zeroed by the host RM when it allocates the channel (mbuild.rs
        // `put_to_restore`); the guest has already written its GPFIFO entry and GPPut for the
        // submission that rang this doorbell. Save them now and write them back at the
        // commit, or the GR PBDMA sees GPPut 0 and fetches nothing (measured 2026-09-23:
        // GPGet 0 / GPPut 0, the launch hung with no Xid).
        let saved_userd = ch.userd.map(|u| {
            let at = |o: u64| match u {
                Target::Fb(pa) => Target::Fb(pa + o),
                Target::Sys(pa) => Target::Sys(pa + o),
                t => t,
            };
            (
                at(0x88),
                at(0x8C),
                gm.read_u32(at(0x88)),
                gm.read_u32(at(0x8C)),
            )
        });
        self.ensure_events(&mut Vec::new());
        phase("host events", &mut phases);
        let mut notes = lazy_notes;
        notes.push(format!(
            "shadow: GR chid {chid} on the host-managed shadow VAS of guest root {root:#x}"
        ));
        let descs = self.descs();
        let mut h = self.hm.lock().unwrap();
        // The shadow VAS of this guest root, built once per root.
        // A root first built for a video channel gets the video layout (shadow.rs `Layout`):
        // a host-only area below the guest's RM base, where the host RM's own context lands.
        // GR and CE roots keep the default layout, as before.
        let layout = if self.video_objects && video_bind(self.chip, &ch).is_some() {
            crate::shadow::Layout::video()
        } else if self.shadow_fence {
            crate::shadow::Layout::fenced()
        } else {
            crate::shadow::Layout::default()
        }
        // The owned-GFXP slot VA window is never mirrored (gfxpslot.rs).
        .with_slot_window(self.slots.as_ref().map(|s| s.va_window()));
        if let std::collections::btree_map::Entry::Vacant(slot) = self.shadows.entry(root) {
            let t0 = Instant::now();
            match crate::shadow::ShadowVas::new(&mut h, self.map_budget.clone(), &layout) {
                Ok(mut sv) => {
                    sv.comp = self.comp.clone();
                    notes.push(format!(
                        "shadow: host-managed VAS {:#x} for guest root {root:#x}, VAS start {:#x}{}{}{} in {} us [host]",
                        sv.vas,
                        sv.va_start,
                        if layout.va_base == 0 && sv.va_start != 0 {
                            " (ALLOW_ZERO_ADDRESS not honoured: guest VAs below it cannot be mirrored)"
                        } else {
                            ""
                        },
                        match layout.fence {
                            Some((lo, hi)) => format!(", fence [{lo:#x}, {hi:#x}) held by the host (its own buffers land above it)"),
                            None => String::new(),
                        },
                        if layout.va_base != 0 {
                            format!(
                                ", video layout: base {:#x}, host area [{:#x}, {:#x}), host server-RM range [{:#x}, {:#x}) not mirrored",
                                layout.va_base,
                                layout.host_area.0,
                                layout.host_area.1,
                                layout.host_server.0,
                                layout.host_server.1
                            )
                        } else {
                            String::new()
                        },
                        t0.elapsed().as_micros()
                    ));
                    let vas = sv.vas;
                    self.worker.submit(Job::Create(root, Box::new(sv)));
                    let mut front = crate::shadow::ShadowFront::new(root);
                    front.layout = layout;
                    front.comp = self.comp.clone();
                    slot.insert(ShadowSpace::new(front, vas));
                }
                Err(st) => {
                    notes.push(format!("shadow: VAS refused: {}", status_name(st)));
                    if crate::hostfloor::is_exhaustion(st) {
                        self.exhausted.insert(chid);
                    }
                    return notes;
                }
            }
        }
        phase("VAS", &mut phases);
        let leaf_end = self.leaf_end();
        let sp = self.shadows.get_mut(&root).expect("built");
        sp.exclude = new_b;
        sp.exclude_gen = crate::ptcache::fresh_generation();
        sp.set_b_stamp = None;
        sp.chids.insert(chid);
        sp.tsgs.insert(tsg_key);
        let exclude = sp.exclude.clone();
        // Mirror the guest's set-A before the channel/GPFIFO VA is validated against the
        // VAS: plan here; the worker applies it before the build job below (a root's jobs
        // never pass each other), so nothing waits for it here.
        let t_plan = Instant::now();
        // The guest's table writes through the BAR2 memfd first, as at every re-plan
        // (`shadow_resync`).
        gm.fb.bar2_sync_tables(None, Some(gm.sys));
        let plan = sp.front.plan(
            gm.fb,
            gm.sys,
            self.base,
            leaf_end,
            self.reservation,
            &exclude,
            &descs,
        );
        let vas_handle = sp.vas;
        drop(h);
        let planned = t_plan.elapsed();
        crate::dlock::work("mirror", "build walk (the root's set-A plan)".into(), planned);
        if planned >= std::time::Duration::from_millis(1) {
            crate::dlock::tag(|| format!("build chid {chid} walk {} ms", planned.as_millis()));
        }
        if let Some(plan) = plan {
            self.worker.submit(Job::Apply(plan, Instant::now()));
        }
        // Diagnostics only (`CARAXES_SHADOW_CHANNEL_DIAG=1`): the coverage query waits for
        // the worker.
        let diag_gpfifo = (std::env::var("CARAXES_SHADOW_CHANNEL_DIAG").as_deref() == Ok("1")).then(|| {
            let bytes = u64::from(ch.gpfifo_entries).saturating_mul(8);
            let Some(end) = ch.gpfifo_va.checked_add(bytes) else {
                return (bytes, None, "skipped: GPFIFO span overflows u64".into(),
                    "skipped: GPFIFO span overflows u64".into(), Vec::new());
            };
            if bytes > 1 << 20 {
                return (bytes, Some(end), format!("skipped: GPFIFO span {bytes:#x} exceeds 1 MiB"),
                    format!("skipped: GPFIFO span {bytes:#x} exceeds 1 MiB"), Vec::new());
            }
            let guest = guest_span_coverage(gm.fb, root, ch.gpfifo_va, end);
            let host = self.worker.coverage(root, ch.gpfifo_va, end);
            let hits: Vec<_> = exclude.iter().copied()
                .filter(|(lo, hi)| ch.gpfifo_va < *hi && end > *lo).collect();
            (bytes, Some(end), guest, host, hits)
        });
        phase("walk and diagnostics", &mut phases);
        if let Err(e) = self.check_tsg_reuse(reg, tsg_key, tsg_guest, &mut notes) {
            notes.push(format!("shadow: chid {chid}: {e}"));
            self.abandon_shadow_build(root, chid, tsg_key, &mut notes);
            return notes;
        }
        // A stale host TSG under the key was dropped from the shadow space's set too.
        if let Some(sp) = self.shadows.get_mut(&root) {
            sp.tsgs.insert(tsg_key);
        }
        // The USERD the host channel uses, from the guest's state now.
        phase("TSG reuse", &mut phases);
        let userd = match self.userd_plan(gm, chid, &ch) {
            Ok(u) => u,
            Err(why) => {
                notes.push(why);
                self.abandon_shadow_build(root, chid, tsg_key, &mut notes);
                return notes;
            }
        };
        // The error notifier: the guest's page, so the host RM writes an RC there and
        // the guest sees the fault (returns 719) instead of hanging to the 30 s
        // watchdog (the CE path does this; without it launch5 hung on the MMU fault).
        let error_page = match ch.error_notifier {
            Some(Target::Sys(iova)) if iova % PAGE == 0 => gm
                .sys
                .translate(iova, PAGE)
                .and_then(|gpa| gm.sys.host_ptr(gpa, PAGE))
                .map(|p| p as usize),
            _ => None,
        };
        // One host TSG per guest TSG, on the shadow VAS (GR0): the guest's GR channels
        // of one group (CUDA's chids 2-9, one TSG, one context share) share one GR
        // context on the host too, so state a push sets on one channel is what a launch
        // on another sees (the guest's layout; before, every channel got its own TSG and
        // context, and `self.tsgs` kept only the last handle). The builds of one guest TSG
        // in flight share its cell: the first one the worker runs makes the host TSG.
        let gr_engine = v::lookup(v::NV2080_ENGINE_TYPES, "GR0").unwrap_or(1);
        let engine_type = match self.tsgs.get(&tsg_key) {
            Some(t) => t.engine,
            None => match reg.tsg(tsg_guest) {
                // The guest TSG's engine: GR0 for a compute channel, COPYn for a CE one.
                Some((_, e)) if e != 0 => e,
                _ if bare && ch.engine_type != 0 => ch.engine_type,
                _ => gr_engine,
            },
        };
        let first_of_tsg =
            !self.tsgs.contains_key(&tsg_key) && !self.builds.values().any(|b| b.tsg_key == tsg_key);
        let cell = self.tsg_cell(tsg_key);
        let objects: Vec<(u32, u32, Vec<u8>)> = ch
            .engine_objects_with_handles()
            .map(|(handle, class, params)| (handle, class, params.to_vec()))
            .collect();
        // Owned GFXP (gfxpslot.rs): a slot for the new host TSG of a channel with a 3D object,
        // bound on the worker before its first schedule; given back at the commit if unused.
        let gfxp_slot = (first_of_tsg
            && objects
                .iter()
                .any(|(_, class, _)| crate::board::host_class_of(*class) == HOST_3D_CLASS))
        .then(|| {
            self.slots.as_mut().map(|s| {
                s.alloc()
                    .map(|i| (i, s.buffers(i)))
                    .ok_or_else(|| format!("all {} slots in use", s.count()))
            })
        })
        .flatten();
        // Steps 0 and 1 of the GR preemption fix (preempt.rs), once per host GR TSG: after
        // the first GR object made its context and before this channel's first
        // GPFIFO_SCHEDULE, read the host modes and bind the guest's recorded compute request.
        let preempt_request = reg.preemption(tsg_guest);
        let out = Arc::new(Mutex::new(None));
        let input = BuildInput {
            chid,
            vas_handle,
            tsg_key,
            tsg_guest,
            cell: cell.clone(),
            engine_type,
            userd,
            error_page,
            gpfifo_va: ch.gpfifo_va,
            gpfifo_entries: ch.gpfifo_entries,
            channel_engine: ch.engine_type,
            alloc_flags: ch.alloc_flags,
            objects,
            bind_engine: video_bind(self.chip, &ch).or_else(|| copy_bind(&ch)),
            gpfifo_class: crate::board::host_class_of(self.chip.channel.gpfifo),
            gfxp_slot: gfxp_slot.clone(),
            reservation: self.reservation,
            base: self.base,
            preempt: (engine_type == gr_engine).then_some(preempt_request.and_then(|r| r.compute)),
            sw_classes: [self.chip.channel.uvm_sw, 0x5080],
            diag: diag_gpfifo,
        };
        let slot2 = out.clone();
        let job: HostCall = Box::new(move |hm| {
            let o = build_on_host(input, hm);
            *slot2.lock().unwrap_or_else(PoisonError::into_inner) = Some(o);
            Vec::new()
        });
        if self.worker.thread.is_some() {
            self.worker.submit(Job::Call(root, job, None));
        } else {
            job(&self.hm);
        }
        let seq = self.worker.seqs.last_of(root);
        self.next_build += 1;
        self.builds.insert(
            chid,
            PendingBuild {
                id: self.next_build,
                root,
                seq,
                generation: ch.generation,
                h_client: ch.h_client,
                h_channel: ch.h_object,
                tsg_key,
                tsg_guest,
                vas_key,
                engine_type,
                slot: gfxp_slot.and_then(|r| r.ok()).map(|(i, _)| i),
                out,
                saved_userd,
                newest_put: saved_userd.and_then(|(_, _, _, put)| put),
                ring_seq: seq,
                doorbells: 1,
                since,
                submit,
                preempt_request,
                late_objects: Vec::new(),
                channel_engine: ch.engine_type,
                alloc_flags: ch.alloc_flags,
            },
        );
        self.build_stats.started += 1;
        self.update_waiting();
        if (!submit && !self.prepare_async) || self.build_finished(chid) {
            if !self.build_finished(chid) {
                self.wait_build(chid);
            } else {
                self.build_stats.inline += 1;
            }
            notes.extend(self.commit_build(reg, gm, chid));
            return notes;
        }
        notes.push(format!(
            "shadow: chid {chid} mirror build handed to the worker (job {seq}, {} us under the lock); its ring is held until the build is committed [host]",
            since.elapsed().as_micros()
        ));
        phase("inputs and hand-off", &mut phases);
        crate::dlock::tag(|| {
            format!(
                "build chid {chid} snapshot {} us ({})",
                since.elapsed().as_micros(),
                phases
                    .iter()
                    .filter(|(_, us)| *us >= 100)
                    .map(|(n, us)| format!("{n} {us} us"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        });
        for (n, us) in &phases {
            crate::dlock::work("mirror build snapshot", n.to_string(), std::time::Duration::from_micros(*us as u64));
        }
        crate::dlock::work("mirror", "build snapshot".into(), since.elapsed());
        notes
    }

    /// The USERD a build's host channel uses (from the guest's state under the lock).
    fn userd_plan(&self, gm: &mut GpuMem, chid: u32, ch: &crate::channel::Channel) -> Result<UserdPlan, String> {
        // P21 design H: a USERD page the guest maps into the BAR1 pool area lives at
        // its pool page (fb.rs `remap`).
        match ch.userd.map(|t| match t {
            Target::Fb(pa) => Target::Fb(gm.fb.remap(pa)),
            t => t,
        }) {
            Some(Target::Fb(pa)) if pa >= self.base && pa + PAGE <= self.end => {
                Ok(UserdPlan::Fb(self.reservation, pa - self.base))
            }
            // USERD in guest sysmem (the guest's RMInstLoc USERD=COH,
            // docs/caraxes/mapping.md §6): the host memory object over the guest's own
            // page, as for a CE channel.
            Some(Target::Sys(iova)) if std::env::var("CARAXES_SHADOW_USERD").as_deref() == Ok("1") => {
                let mut snapshot = [0; crate::userd::USERD_BYTES];
                if iova % 512 != 0 || !gm.read(Target::Sys(iova), &mut snapshot) {
                    return Err("shadow USERD: guest extent is unaligned or unreadable".into());
                }
                Ok(UserdPlan::Private {
                    iova,
                    entries: ch.gpfifo_entries,
                })
            }
            Some(Target::Sys(iova)) => gm
                .sys
                .translate(iova & !(PAGE - 1), PAGE)
                .and_then(|gpa| gm.sys.host_ptr(gpa, PAGE))
                .map(|ptr| UserdPlan::GuestPage {
                    ptr: ptr as usize,
                    offset: iova & (PAGE - 1),
                    iova,
                })
                .ok_or_else(|| format!("shadow: chid {chid} USERD IOVA {iova:#x} not registered guest RAM")),
            other => Err(format!(
                "shadow: chid {chid} USERD {other:?} not a vidmem reservation page"
            )),
        }
    }

    /// The host TSG cell of guest TSG `key` for a build (mbuild.rs `TsgCell`): shared with the
    /// builds in flight, else fresh (from the committed host TSG, if there is one).
    fn tsg_cell(&mut self, key: TsgKey) -> Arc<Mutex<crate::mbuild::TsgCell>> {
        let committed = self
            .tsgs
            .get(&key)
            .map(|t| crate::mbuild::TsgCell::committed(t.handle, t.preempt.checked));
        self.tsg_cells
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(committed.unwrap_or_default())))
            .clone()
    }

    /// Whether the worker has finished the build of `chid`.
    fn build_finished(&self, chid: u32) -> bool {
        self.builds
            .get(&chid)
            .is_some_and(|b| b.out.lock().unwrap_or_else(PoisonError::into_inner).is_some())
    }

    /// Wait under the lock for the build of `chid` (a preparation; a free of its objects).
    fn wait_build(&mut self, chid: u32) {
        let Some(b) = self.builds.get(&chid) else {
            return;
        };
        if self.worker.thread.is_none() || self.build_finished(chid) {
            return;
        }
        let t0 = Instant::now();
        self.worker
            .progress
            .wait(&crate::invgate::Target::Roots(vec![(b.root, b.seq)]));
        self.build_stats.waited += 1;
        self.build_stats.waited_ns += t0.elapsed().as_nanos() as u64;
    }

    /// Rings the completer waits for: held rings and builds in flight.
    fn update_waiting(&self) {
        self.ring_gate
            .set_waiting(self.held_rings.len() + self.builds.len());
    }

    /// A doorbell of channel `chid` whose mirror build is in flight (execution.md §9.2): the
    /// root is re-synced as at a built channel's doorbell, the newest GPPut and the root's
    /// last job are noted for the commit, and the build is committed now if the worker has
    /// finished it.
    fn build_doorbell(&mut self, reg: &Registry, gm: &mut GpuMem, chid: u32) -> Vec<String> {
        let Some((root, put_at)) = self
            .builds
            .get(&chid)
            .map(|b| (b.root, b.saved_userd.map(|(_, put, _, _)| put)))
        else {
            return Vec::new();
        };
        let mut notes = self.shadow_resync(reg, gm, Some(root));
        // A zero here is the host RM's clear, not a store: the guest cannot wrap its GPPut
        // back to 0 while nothing it submitted is fetched (GPGet stays 0).
        let put = put_at.and_then(|at| gm.read_u32(at)).filter(|p| *p != 0);
        let last = self.worker.seqs.last_of(root);
        if let Some(b) = self.builds.get_mut(&chid) {
            b.newest_put = put.or(b.newest_put);
            b.ring_seq = last;
            b.doorbells += 1;
        }
        self.build_stats.joined += 1;
        if self.build_finished(chid) {
            notes.extend(self.commit_build(reg, gm, chid));
        } else {
            notes.push(format!(
                "shadow: chid {chid} doorbell joined its mirror build in flight [host]"
            ));
        }
        notes
    }

    /// Commit every build the worker has finished, oldest first (the completer; ringgate.rs).
    fn commit_finished_builds(&mut self, reg: &Registry, gm: &mut GpuMem) -> Vec<String> {
        let mut done: Vec<(u64, u32)> = self
            .builds
            .iter()
            .filter(|(_, b)| b.out.lock().unwrap_or_else(PoisonError::into_inner).is_some())
            .map(|(c, b)| (b.id, *c))
            .collect();
        done.sort_unstable();
        let mut notes = Vec::new();
        for (_, chid) in done {
            notes.extend(self.commit_build(reg, gm, chid));
        }
        notes
    }

    /// Wait for and commit the builds in flight that a guest free of `(h_client, h_object)`
    /// concerns (the channel, one of its engine objects, its TSG, its VA space, or the client
    /// of any of them), so the free finds them built (boot.rs, before `Dispatcher::free`).
    /// Any other free leaves them in flight. Rare: a build lives about a second.
    pub fn settle_builds_for_free(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        h_client: u32,
        h_object: u32,
    ) -> Vec<String> {
        if self.builds.is_empty() {
            return Vec::new();
        }
        let key = reg.resolve((h_client, h_object));
        let object_of = reg.engine_object(h_client, h_object).map(|(chid, ..)| chid);
        let mut hit: Vec<(u64, u32)> = self
            .builds
            .iter()
            .filter(|(chid, b)| {
                let client = h_object == h_client
                    && (b.h_client == h_client || b.vas_key.0 == h_client || b.tsg_guest.0 == h_client);
                client
                    || (b.h_client, b.h_channel) == (h_client, h_object)
                    || object_of == Some(**chid)
                    || b.tsg_guest == key
                    || b.vas_key == key
            })
            .map(|(c, b)| (b.id, *c))
            .collect();
        hit.sort_unstable();
        let mut notes = Vec::new();
        for (_, chid) in hit {
            self.wait_build(chid);
            notes.push(format!(
                "shadow: chid {chid}: mirror build committed before the free of {h_client:#x}/{h_object:#x} [host]"
            ));
            notes.extend(self.commit_build(reg, gm, chid));
        }
        notes
    }

    /// A build in flight whose channel, TSG or VA space goes (a free the boot model did not
    /// settle, a reset): wait for it, then put what it made in the maps unscheduled (token 0,
    /// context not ready, so nothing rings it), for the caller's teardown to free as any host
    /// channel. Its doorbells are dropped with the channel.
    fn settle_build(&mut self, chid: u32, notes: &mut Vec<String>) {
        if !self.builds.contains_key(&chid) {
            return;
        }
        self.wait_build(chid);
        let Some(pb) = self.builds.remove(&chid) else {
            return;
        };
        self.update_waiting();
        self.build_stats.dropped += 1;
        let Some(built) = self.absorb_build(&pb, chid, notes) else {
            return;
        };
        notes.push(format!(
            "shadow: chid {chid}: mirror build taken down before its commit (host channel {:#x} never scheduled) [host]",
            built.channel
        ));
        let pages: Vec<u32> = built.owned.iter().copied().filter(|d| *d != built.channel).collect();
        self.channels.insert(
            chid,
            HostChannel {
                generation: pb.generation,
                handle: built.channel,
                tsg: pb.tsg_key,
                token: 0,
                objects: built.objects,
                engine_map: built.engine_map,
                pages,
                created: Instant::now(),
                doorbells: 0,
                shadow_root: Some(pb.root),
                fwd_put: None,
                fwd_unwaited: 0,
                userd: built.shadow_userd.map(|u| UserdSlot::new(u, false)),
                context_ready: false,
                rc_page: built.rc_page,
            },
        );
        self.publish_userd_watch();
    }

    /// Take a finished build's output into the maps (the commit and the teardown): the host
    /// TSG it made or joined, its GFXP slot and set, the preemption check. `None`: nothing
    /// to schedule (the host refused the build, or the job never ran), with the build
    /// abandoned.
    fn absorb_build(&mut self, pb: &PendingBuild, chid: u32, notes: &mut Vec<String>) -> Option<HostBuilt> {
        let key = pb.tsg_key;
        let out = pb.out.lock().unwrap_or_else(PoisonError::into_inner).take();
        if !self.builds.values().any(|b| b.tsg_key == key) {
            self.tsg_cells.remove(&key);
        }
        let Some(out) = out else {
            notes.push(format!("shadow: chid {chid}: mirror build lost (the shadow worker is gone)"));
            if let (Some(s), Some(slots)) = (pb.slot, self.slots.as_mut()) {
                slots.free(s);
            }
            self.abandon_shadow_build(pb.root, chid, key, notes);
            return None;
        };
        self.build_stats.host(out.host);
        notes.extend(out.notes);
        if let Some((handle, made)) = out.tsg {
            let t = self.tsgs.entry(key).or_insert(HostTsg {
                handle,
                vas: pb.vas_key,
                engine: pb.engine_type,
                guest: pb.tsg_guest,
                gfxp: None,
                preempt: TsgPreempt::default(),
            });
            if made && t.handle != handle {
                notes.push(format!(
                    "ERROR shadow: tsg {key}: host TSG {handle:#x} made by the build of chid {chid} while {:#x} stands for the guest TSG [host]",
                    t.handle
                ));
            }
        }
        // The reserved GFXP slot goes back unless the set holds it.
        let slot_held = matches!(&out.gfxp, Some(Ok(_)) | Some(Err((Some(_), _))));
        if let (Some(s), false, Some(slots)) = (pb.slot, slot_held, self.slots.as_mut()) {
            slots.free(s);
        }
        let tsg = out.tsg.map(|(t, _)| t);
        match out.gfxp {
            Some(Ok(set)) => {
                let (lo, hi) = self
                    .slots
                    .as_ref()
                    .map(|s| s.range(set.slot))
                    .unwrap_or_default();
                notes.push(format!(
                    "shadow: tsg {key}: owned GFXP bound at build (slot {} of the VM's slice [{lo:#x}, {hi:#x}), graphics mode 1 verified) [host]",
                    set.slot
                ));
                self.gfxp_bound += 1;
                if let Some(t) = self.tsgs.get_mut(&key) {
                    t.gfxp = Some(set);
                }
            }
            Some(Err((kept, why))) => {
                let line = format!(
                    "WARNING gfxp: tsg {key} (guest chid {chid}) stays in graphics WFI: {why}"
                );
                eprintln!("vfio-user-nvgpu: {line}");
                notes.push(line);
                self.gfxp_no_slot += 1;
                if let (Some(set), Some(t)) = (kept, self.tsgs.get_mut(&key)) {
                    t.gfxp = Some(set);
                }
            }
            None => {}
        }
        match out.result {
            Ok(built) => {
                if let (Some(a), Some(tsg)) = (out.preempt, tsg) {
                    self.note_preempt_build(key, tsg, chid, pb.preempt_request, &a, notes);
                }
                Some(built)
            }
            Err((why, st)) => {
                notes.push(why);
                self.build_stats.failed += 1;
                if st.is_some_and(crate::hostfloor::is_exhaustion) {
                    self.exhausted.insert(chid);
                }
                self.abandon_shadow_build(pb.root, chid, key, notes);
                None
            }
        }
    }

    /// Commit the finished build of `chid` under the lock (execution.md §9.2): take its
    /// output into the maps, schedule the channel and get its work-submit token (the host
    /// channel runs from here on), write back the GPPut the host RM cleared, and ring it once
    /// the root's jobs up to its newest doorbell are applied (a held ring otherwise).
    fn commit_build(&mut self, reg: &Registry, gm: &mut GpuMem, chid: u32) -> Vec<String> {
        let t0 = Instant::now();
        let Some(pb) = self.builds.remove(&chid) else {
            return Vec::new();
        };
        let mut notes = Vec::new();
        let built = self.absorb_build(&pb, chid, &mut notes);
        let committed = self.schedule_build(reg, gm, &pb, chid, built, &mut notes);
        crate::dlock::tag(|| format!("commit chid {chid} {} us", t0.elapsed().as_micros()));
        crate::dlock::work("mirror", "build commit".into(), t0.elapsed());
        self.update_waiting();
        self.build_stats.committed += 1;
        self.build_stats.commit(t0.elapsed(), pb.since.elapsed());
        self.stats.max_create_us = self.stats.max_create_us.max(pb.since.elapsed().as_micros());
        if !committed {
            return notes;
        }
        // The guest freed or replaced the channel during the build (a free the boot model
        // did not settle): the host channel goes again at once.
        if reg.channel(chid).is_none_or(|c| c.generation != pb.generation) {
            notes.push(format!(
                "shadow: chid {chid}: the guest replaced the channel during its mirror build; host channel freed [host]"
            ));
            self.committed.retain(|(c, _)| *c != chid);
            self.free_channel(chid, &mut notes);
            return notes;
        }
        if !pb.submit {
            notes.push(format!(
                "shadow: chid {chid} context prepared with an empty private USERD; no submission"
            ));
            return notes;
        }
        // The ring: after the root's jobs up to its newest doorbell (plans submitted during
        // the build) and while no hold names the root, as for a built channel.
        let ring = crate::ringgate::HeldRing {
            root: Some(pb.root),
            target: crate::invgate::Target::Roots(vec![(pb.root, pb.ring_seq)]),
            since: Instant::now(),
            doorbells: pb.doorbells,
        };
        if crate::ringgate::ready(&ring, &self.worker.progress, &self.ring_gate) {
            notes.extend(self.ring_now(reg, gm, chid));
        } else {
            self.ring_stats.held += 1;
            self.held_rings.insert(chid, ring);
            self.update_waiting();
            notes.push(format!(
                "shadow: chid {chid} ring held until root {:#x}'s jobs are applied [host]",
                pb.root
            ));
        }
        // One-shot USERD read (no poll): GPGet advancing past GPPut's start means the GR
        // PBDMA fetched the guest's first push; GPGet stuck at 0 means it never fetched
        // (scheduling/token). Distinguishes a completion-delivery hang from a fetch hang.
        // Diagnostic only (`CARAXES_MIRROR_DIAG=1`): its 5 ms sleep ran at every shadow
        // channel's creation, about half a second per CUDA context (mapping.md §3).
        if let Some((get_at, put_at, _, _)) = pb
            .saved_userd
            .filter(|_| std::env::var("CARAXES_MIRROR_DIAG").as_deref() == Ok("1"))
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
            notes.push(format!(
                "shadow: chid {chid} USERD after doorbell: GPGet {:?} GPPut {:?} [host GPU]",
                gm.read_u32(get_at),
                gm.read_u32(put_at)
            ));
        }
        notes
    }

    /// The scheduling half of a commit: the private USERD's first state, GPFIFO_SCHEDULE,
    /// the work-submit token (the host RM writes it into the guest's notifier page, from
    /// which the guest rings it from its next submission on), the channel in the maps, the
    /// engine objects the guest allocated during the build, and the GPPut write-back.
    /// Whether the channel is scheduled; a refusal frees what the build made.
    fn schedule_build(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        pb: &PendingBuild,
        chid: u32,
        built: Option<HostBuilt>,
        notes: &mut Vec<String>,
    ) -> bool {
        let Some(mut built) = built else {
            return false;
        };
        let key = pb.tsg_key;
        let tsg = self.tsgs.get(&key).map_or(0, |t| t.handle);
        let scheduled = (|| -> Result<u32, String> {
            if let Some(u) = built.shadow_userd.as_mut() {
                // Initialize an unscheduled, empty ring once. Later copies never
                // overwrite GPU-owned state; new GPPut is published before the doorbell.
                let get = gm
                    .read_u32(Target::Sys(u.guest + 0x88))
                    .filter(|get| *get < u.entries)
                    .ok_or("shadow USERD: invalid initial GPGet")?;
                u.storage.write_u32(0x88, get);
                // Hold the ring empty until token acquisition and ownership insertion.
                u.storage.write_u32(0x8c, get);
                u.storage.flush();
            }
            let h = self.hm.lock().unwrap();
            let mut sched = [1u8, 0, 0];
            let st = h.control(built.channel, CMD_GPFIFO_SCHEDULE, &mut sched);
            if st != sys::NV_OK {
                return Err(format!("shadow: GPFIFO_SCHEDULE: {}", status_name(st)));
            }
            let mut tok = [0u8; 4];
            let st = h.control(built.channel, CMD_GET_WORK_SUBMIT_TOKEN, &mut tok);
            if st != sys::NV_OK {
                return Err(format!("shadow: GET_WORK_SUBMIT_TOKEN: {}", status_name(st)));
            }
            Ok(u32::from_le_bytes(tok))
        })();
        let token = match scheduled {
            Ok(t) => t,
            Err(why) => {
                notes.push(why);
                self.build_stats.failed += 1;
                let mut h = self.hm.lock().unwrap();
                // An owned GFXP set bound in this build goes back first (WFI, unmap, slot):
                // a TSG this build made holds no other channel.
                if let Some(t) = self.tsgs.get_mut(&key).filter(|_| pb.slot.is_some()) {
                    if let Some(mut set) = t.gfxp.take() {
                        let _ = bind_gfxp(&h, tsg, 0, &[0; 5]);
                        if let Err(e) = release_gfxp(&mut h, self.slots.as_mut(), self.reservation, &mut set) {
                            notes.push(format!("ERROR gfxp: tsg {key}: {e}; set retained"));
                            t.gfxp = Some(set);
                        }
                    }
                }
                // Newest first: the channel (with its engine objects), then the guest pages.
                let mut freed = true;
                for d in built.owned.iter().rev() {
                    freed &= h.free_object(*d).is_ok();
                }
                if let Some(u) = built.shadow_userd.take() {
                    // If a channel free failed, retain its tracked memory object until
                    // RM client teardown; the CPU mapping may still be dropped safely.
                    if freed {
                        let _ = h.free_host_userd(u.storage);
                    }
                }
                drop(h);
                self.abandon_shadow_build(pb.root, chid, key, notes);
                return false;
            }
        };
        let pages: Vec<u32> = built.owned.iter().copied().filter(|d| *d != built.channel).collect();
        self.stats.channels_created += 1;
        let private_userd = built.shadow_userd.is_some();
        self.channels.insert(
            chid,
            HostChannel {
                generation: pb.generation,
                handle: built.channel,
                tsg: key,
                token,
                objects: built.objects,
                engine_map: built.engine_map,
                pages,
                created: Instant::now(),
                doorbells: 0,
                shadow_root: Some(pb.root),
                fwd_put: None,
                fwd_unwaited: 0,
                userd: built.shadow_userd.map(|u| UserdSlot::new(u, pb.submit)),
                context_ready: pb.submit,
                rc_page: built.rc_page,
            },
        );
        self.publish_userd_watch();
        self.committed.push((chid, token));
        notes.push(format!(
            "shadow: chid {chid} scheduled; host token {token:#x} (host chid {}, runlist {}), {} ms after its first doorbell [host]",
            token & 0xFFF,
            (token >> 16) & 0x7F,
            pb.since.elapsed().as_millis()
        ));
        if pb.alloc_flags & (1 << 5) != 0
            && pb.channel_engine != v::lookup(v::NV2080_ENGINE_TYPES, "GR0").unwrap_or(1)
        {
            notes.push(format!(
                "shadow: chid {chid}: the guest's privileged engine {:#x} channel ({}) runs unprivileged on the host (DENY_PHYSICAL_MODE_CE; a privileged or physical-mode method faults this channel only) [host]",
                pb.channel_engine,
                if video_engine(pb.channel_engine).is_some() { "a video engine" } else { "an application's VA space" }
            ));
        }
        // A preemption SET of the guest during the build: bound now, as a SET after a build.
        if self.tsgs.get(&key).is_some_and(|t| t.preempt.checked)
            && reg.preemption(pb.tsg_guest) != pb.preempt_request
        {
            notes.extend(self.guest_preemption_set(reg, pb.tsg_guest));
        }
        // Engine objects the guest allocated during the build.
        for (handle, class, params) in &pb.late_objects {
            notes.extend(self.add_engine_object(chid, *handle, *class, params));
        }
        // The GPPut the host RM's clear took (mbuild.rs `put_to_restore`).
        if let Some((get_at, put_at, Some(get), _)) = pb.saved_userd.filter(|_| !private_userd) {
            if let Some(put) = crate::mbuild::put_to_restore(pb.newest_put, gm.read_u32(put_at)) {
                gm.write(get_at, &get.to_le_bytes());
                gm.write(put_at, &put.to_le_bytes());
                notes.push(format!(
                    "shadow: chid {chid} restored guest GPGet {get} / GPPut {put} (host RM had cleared the USERD) [guest]"
                ));
            }
        }
        gm.fb.flush();
        true
    }

    /// Channels committed since the last call, with their host tokens (ctrl.rs pins a pool
    /// token over the host token the RM wrote into the guest's notifier).
    pub fn take_committed(&mut self) -> Vec<(u32, u32)> {
        std::mem::take(&mut self.committed)
    }

    /// The exit summary's line of the builds off the lock.
    pub fn build_summary(&self) -> String {
        self.build_stats.line(self.builds.len())
    }

    /// The guest channel whose host mirror owns work-submit token `token`. The host RM
    /// writes the host token into the channel's notifier memory when the mirror asks for
    /// it (`kchannelNotifyWorkSubmitToken`, kernel_channel.c:4023-4030), and that memory
    /// is the guest's own notifier page, from which libcuda re-reads the token before
    /// every submission: from the second doorbell on, the guest rings the host token
    /// (measured 2026-09-23: `token 0x8003a chid 58`, "no such channel", the push never
    /// ran; that was the whole "CE first-launch stall"). Host tokens carry host ChIDs
    /// (44 and up) and runlists, so an exact match cannot be a guest token.
    pub fn chid_of_host_token(&self, token: u32) -> Option<u32> {
        self.channels
            .iter()
            .find(|(_, c)| c.token == token)
            .map(|(chid, _)| *chid)
    }

    /// Read the fetched prefix boundary without retiring the host channel.
    pub fn management_cursor(&self, reg: &Registry, chid: u32) -> Result<u32, String> {
        let ch = reg.channel(chid).ok_or("management channel is gone")?;
        let hc = self.channels.get(&chid).ok_or("management has no host channel")?;
        if hc.generation != ch.generation { return Err("management channel generation changed".into()); }
        let (get, published) = hc
            .with_userd(|u| (u.storage.read_u32(0x88), u.published_put))
            .ok_or("management takeover requires private USERD")?;
        if published != Some(get) { return Err("management host channel still has unfetched work".into()); }
        Ok(get)
    }

    /// Wait for engine completion, then retire the exact validated management channel.
    pub fn retire_for_management(&mut self, reg: &Registry, gm: &mut GpuMem, chid: u32, expected_get: u32) -> Result<Vec<String>, String> {
        let ch = reg.channel(chid).ok_or("management channel is gone")?;
        let get = self.management_cursor(reg, chid)?;
        if get != expected_get { return Err("management prefix changed after validation".into()); }
        let key = self.channels.get(&chid).ok_or("management host channel is gone")?.tsg;
        if reg.channels().filter(|c| tsg_key_of(reg, c).0 == key).count() != 1 {
            return Err("management takeover of a shared TSG is not implemented".into());
        }
        // nvos.h: idle the pushbuffer, FIFO cache and all engines of this channel.
        let flags = ((sys::NVOS30_FLAGS_IDLE_PUSH_BUFFER | sys::NVOS30_FLAGS_IDLE_CACHE1
            | sys::NVOS30_FLAGS_IDLE_ALL_ENGINES) << 8) | sys::NVOS30_FLAGS_BEHAVIOR_SLEEP;
        let (status, idle_note) = self.idle_channels(reg, &[(ch.h_client, ch.h_object)], flags, 100_000)
            .ok_or("management idle has no host target")?;
        if status != sys::NV_OK { return Err(idle_note); }
        if self.management_cursor(reg, chid)? != expected_get { return Err("management prefix changed while idling".into()); }
        self.channels
            .get(&chid)
            .and_then(|hc| hc.with_userd(|u| u.harvest(gm)))
            .ok_or("private USERD disappeared")?;
        // The root's plans applied before the host TSG goes; the shadow VAS's teardown then runs
        // on the worker in bounded steps, not waited for under the device lock: no channel or
        // TSG is left on it, so nothing can run through it (Windows C70: 300-345 ms holds).
        let root = self.channels.get(&chid).and_then(|hc| hc.shadow_root);
        self.wait_root_or_all(root);
        let mut notes = vec![idle_note];
        if !self.free_tsg(key, &mut notes) { return Err(notes.join("; ")); }
        Ok(notes)
    }

    /// Wait under the device lock for the plans of shadow root `root` only (all plans without
    /// one), not for other roots' jobs or teardowns.
    fn wait_root_or_all(&self, root: Option<u64>) {
        match root {
            Some(r) => self.worker.wait_root(r),
            None => self.worker.wait_all(),
        }
    }

    /// Prepare a host-owned GR context without publishing guest work to its USERD.
    pub fn prepare_deferred_context(&mut self, reg: &Registry, gm: &mut GpuMem, chid: u32) -> Result<Vec<String>, String> {
        let ch = reg.channel(chid).ok_or("deferred target channel is gone")?;
        if !self.is_shadow(reg, chid) || !matches!(ch.userd, Some(Target::Sys(_)))
            || std::env::var("CARAXES_SHADOW_USERD").as_deref() != Ok("1") {
            return Err("deferred context requires a shadow GR channel and private sysmem USERD".into());
        }
        if !ch.engine_objects.iter().any(|(class, _)| *class == self.chip.channel.compute) {
            return Err("deferred target has no GR compute object".into());
        }
        if self.channels.get(&chid).is_some_and(|hc| hc.doorbells != 0) {
            return Err("deferred target already submitted work before initialization".into());
        }
        let notes = self.shadow_channel(reg, gm, chid, false);
        let hc = self.channels.get(&chid).filter(|hc| hc.generation == ch.generation)
            .ok_or_else(|| format!("host context preparation failed: {}", notes.join("; ")))?;
        if hc.userd.is_none() || !hc.objects.iter().any(|(class, _)| *class == self.chip.channel.compute) {
            return Err("host context preparation lacks private USERD or GR object".into());
        }
        Ok(notes)
    }

    /// [`Self::prepare_deferred_context`] without waiting under the device lock
    /// (docs/caraxes/execution.md §9.12): the target's first mirror build goes to the
    /// worker, and each call answers whether the context is ready: its build finished and
    /// committed. `Ok((false, _))`: ask again later (the deferred method stays
    /// pending, its management channel held). A context built before is prepared as before.
    pub fn start_deferred_context(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        chid: u32,
    ) -> Result<(bool, Vec<String>), String> {
        let ch = reg.channel(chid).ok_or("deferred target channel is gone")?;
        let mut notes = Vec::new();
        if self.builds.contains_key(&chid) {
            if !self.build_finished(chid) {
                return Ok((false, notes));
            }
            notes.extend(self.commit_build(reg, gm, chid));
        } else if !self.channels.contains_key(&chid) {
            if !self.is_shadow(reg, chid) || !matches!(ch.userd, Some(Target::Sys(_)))
                || std::env::var("CARAXES_SHADOW_USERD").as_deref() != Ok("1") {
                return Err("deferred context requires a shadow GR channel and private sysmem USERD".into());
            }
            if !ch.engine_objects.iter().any(|(class, _)| *class == self.chip.channel.compute) {
                return Err("deferred target has no GR compute object".into());
            }
            self.prepare_async = true;
            notes.extend(self.shadow_channel(reg, gm, chid, false));
            self.prepare_async = false;
            if self.builds.contains_key(&chid) {
                return Ok((false, notes));
            }
        } else {
            return self.prepare_deferred_context(reg, gm, chid).map(|n| (true, n));
        }
        let hc = self.channels.get(&chid).filter(|hc| hc.generation == ch.generation)
            .ok_or_else(|| format!("host context preparation failed: {}", notes.join("; ")))?;
        if hc.userd.is_none() || !hc.objects.iter().any(|(class, _)| *class == self.chip.channel.compute) {
            return Err("host context preparation lacks private USERD or GR object".into());
        }
        // Committed: the worker ran the build after the root's plans before it, as the
        // synchronous preparation's wait for the build did.
        gm.fb.flush();
        Ok((true, notes))
    }

    pub fn promote_deferred_context(&mut self, reg: &Registry, chid: u32) -> Result<(), String> {
        let ch = reg.channel(chid).ok_or("deferred target channel is gone")?;
        let root = reg.vas_key_of(ch).and_then(|key| reg.vas_root(key));
        let hc = self.channels.get_mut(&chid).filter(|hc| hc.generation == ch.generation)
            .ok_or("deferred target host context is gone")?;
        if root != hc.shadow_root.map(Target::Fb) { return Err("deferred target VAS changed".into()); }
        hc.set_context_ready(true);
        Ok(())
    }

    /// Read the actual mode of an existing private context. No guest buffer pointers
    /// are passed to RM, and no context or scheduling state is changed.
    pub fn deferred_context_modes(&self, reg: &Registry, client: u32, object: u32) -> Result<String, String> {
        let key = reg.resolve((client, object));
        if reg.tsg(key).is_none_or(|(_, engine)| engine != 1) {
            return Err("preemption target is not a live GR TSG".into());
        }
        let mut members = reg.channels().filter(|ch| (ch.h_client, ch.h_parent) == key);
        let ch = members.next().ok_or("preemption TSG has no channel")?;
        if members.next().is_some() {
            return Err("preemption query requires a single-channel TSG".into());
        }
        let hc = self.channels.get(&ch.chid)
            .filter(|hc| hc.generation == ch.generation && hc.context_ready)
            .ok_or("preemption target has no prepared and promoted host context")?;
        if reg.vas_key_of(ch).and_then(|vas| reg.vas_root(vas)) != hc.shadow_root.map(Target::Fb) {
            return Err("preemption target VAS changed".into());
        }
        let mut params = [0u8; 24];
        params[..4].copy_from_slice(&hc.handle.to_le_bytes());
        let host = self.hm.lock().unwrap();
        let status = host.control(host.subdevice(), 0x2080_1235, &mut params);
        let word = |offset| u32::from_le_bytes(params[offset..offset + 4].try_into().unwrap());
        Ok(format!(
            "host context modes chid {} generation {}: status {status:#x}, gfxp {:#x}, cilp {:#x}, zcull {:#x}, pm {:#x}, smpc {:#x}",
            ch.chid, ch.generation, word(20), word(16), word(4), word(8), word(12)
        ))
    }

    /// Steps 0 and 1 at a host GR TSG's build (preempt.rs, `shadow_channel`): mark it
    /// checked, count, and leave one line in the device log (plus a WARN on a refusal).
    fn note_preempt_build(
        &mut self,
        key: TsgKey,
        host_tsg: u32,
        chid: u32,
        request: Option<crate::preempt::Recorded>,
        a: &crate::preempt::Applied,
        notes: &mut Vec<String>,
    ) {
        let Some(t) = self.tsgs.get_mut(&key) else {
            return;
        };
        t.preempt.checked = true;
        t.preempt.outcome = Some(a.outcome);
        let guest = t.guest;
        self.preempt.count(a, false);
        if request
            .and_then(|r| r.graphics)
            .is_some_and(|g| g != crate::preempt::GFX_WFI)
        {
            self.preempt.gfxp_asked += 1;
        }
        let line = format!(
            "preemption: tsg {key} (guest {:#x}/{:#x}, host TSG {host_tsg:#x}, chid {chid}) at build, before the first schedule: {}; {} [host]",
            guest.0,
            guest.1,
            request.map_or("no guest SET".into(), |r| r.describe()),
            a.describe()
        );
        eprintln!("vfio-user-nvgpu: {line}");
        if a.refused() {
            eprintln!(
                "vfio-user-nvgpu: WARN preemption: tsg {key}: {}; the guest's SET was answered NV_OK [host]",
                a.outcome.describe()
            );
        }
        notes.push(line);
    }

    /// Step 0 at teardown: the modes of a checked host GR TSG, read once on its last hosted
    /// channel right before that channel's free (read only).
    fn preempt_teardown_read(&mut self, chid: u32, notes: &mut Vec<String>) {
        let Some(hc) = self.channels.get(&chid) else {
            return;
        };
        let (key, handle) = (hc.tsg, hc.handle);
        if self
            .channels
            .iter()
            .any(|(c, other)| *c != chid && other.tsg == key)
        {
            return;
        }
        let Some(t) = self.tsgs.get_mut(&key) else {
            return;
        };
        if !t.preempt.checked || t.preempt.teardown_read {
            return;
        }
        t.preempt.teardown_read = true;
        let (guest, outcome) = (t.guest, t.preempt.outcome);
        let m = crate::preempt::CtxswHost::modes(&*self.hm.lock().unwrap(), handle);
        self.preempt.teardown_reads += 1;
        let line = format!(
            "preemption: tsg {key} (guest {:#x}/{:#x}, chid {chid}, its last host channel) at teardown: host {}{} [host read]",
            guest.0,
            guest.1,
            m.describe(),
            outcome.map_or(String::new(), |o| format!("; earlier: {}", o.describe()))
        );
        eprintln!("vfio-user-nvgpu: {line}");
        notes.push(line);
    }

    /// Step 1 for a guest SET on guest TSG (or bare channel) `guest` whose host TSG is built
    /// and checked already: the recorded compute request is bound now, on the TSG idled
    /// first (`FIFO_IDLE_CHANNELS` over its host channels). `None` when no host TSG stands
    /// for it yet: its build binds the request.
    pub fn guest_preemption_set(&mut self, reg: &Registry, guest: (u32, u32)) -> Option<String> {
        let key = self.tsg_standing_for(guest)?;
        let gr = v::lookup(v::NV2080_ENGINE_TYPES, "GR0").unwrap_or(1);
        let t = self.tsgs.get(&key)?;
        if t.engine != gr {
            return Some(format!("host TSG ({key}) is not on GR: nothing bound"));
        }
        if !t.preempt.checked {
            return Some(format!(
                "host TSG ({key}) has no GR context yet: bound at its next channel's build"
            ));
        }
        let host_tsg = t.handle;
        let hosted: Vec<(u32, u32)> = self
            .channels
            .iter()
            .filter(|(_, hc)| hc.tsg == key)
            .map(|(c, hc)| (*c, hc.handle))
            .collect();
        let Some(&(probe_chid, probe)) = hosted.first() else {
            return Some(format!(
                "host TSG ({key}) has no host channel left: nothing bound"
            ));
        };
        let requested = reg.preemption(guest).and_then(|r| r.compute);
        let flags = ((sys::NVOS30_FLAGS_IDLE_PUSH_BUFFER
            | sys::NVOS30_FLAGS_IDLE_CACHE1
            | sys::NVOS30_FLAGS_IDLE_ALL_ENGINES)
            << 8)
            | sys::NVOS30_FLAGS_BEHAVIOR_SLEEP;
        let mut idle_note = None;
        let a = {
            let host = LockedHost(&self.hm);
            crate::preempt::apply_compute_with(&host, host_tsg, probe, requested, || {
                let (st, note) = self.idle_host_channels(&hosted, flags, 100_000);
                idle_note = Some(note);
                st == sys::NV_OK
            })
        };
        self.preempt.count(&a, true);
        if let Some(t) = self.tsgs.get_mut(&key) {
            if !matches!(a.outcome, crate::preempt::Outcome::NoRequest) {
                t.preempt.outcome = Some(a.outcome);
            }
        }
        let line = format!(
            "preemption: tsg {key} (guest {:#x}/{:#x}, host TSG {host_tsg:#x}, probe chid {probe_chid}, {} host channel(s)) SET after the build{}: {} [host]",
            guest.0,
            guest.1,
            hosted.len(),
            idle_note.map_or(String::new(), |n| format!(" ({n})")),
            a.describe()
        );
        // A late SET that changes nothing stays in the trace; one that binds or is refused
        // goes to the device log.
        if !matches!(
            a.outcome,
            crate::preempt::Outcome::NoRequest | crate::preempt::Outcome::Already(_)
        ) {
            eprintln!("vfio-user-nvgpu: {line}");
        }
        if a.refused() {
            eprintln!(
                "vfio-user-nvgpu: WARN preemption: tsg {key}: {}; the guest's SET was answered NV_OK [host]",
                a.outcome.describe()
            );
        }
        Some(line)
    }

    /// The exit summary's line of steps 0 and 1 (preempt.rs).
    pub fn preemption_summary(&self) -> String {
        self.preempt.summary()
    }

    /// RM teardown idles the exact mirrored target, harvests its final GPGet and releases
    /// its host-owned context. Once eviction starts, an error leaves the context present
    /// but not ready, so no new guest work can be published to it.
    pub fn evict_deferred_context(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        chid: u32,
    ) -> Result<(Vec<String>, u32), String> {
        let ch = reg.channel(chid).ok_or("deferred target channel is gone")?;
        let generation = ch.generation;
        let client = ch.h_client;
        let object = ch.h_object;
        let root = reg.vas_key_of(ch).and_then(|vas| reg.vas_root(vas));
        let key = tsg_key_of(reg, ch).0;
        let hosted = self.channels.get(&chid).is_some_and(|hc| hc.generation == generation);
        let others = self.channels.iter().any(|(c, hc)| *c != chid && hc.tsg == key);
        match evict_path(hosted, others, self.tsgs.contains_key(&key)) {
            EvictPath::HostChannel => {}
            EvictPath::Refuse => {
                return Err("deferred target host context is gone while other members of its TSG are hosted".into());
            }
            // Run P30: the TSG's context was moved to its last channel, which never rang (no
            // host channel was built for it). Nothing of it runs on the host: the TSG's host
            // context goes with the host TSG, and the guest's GPGet is final.
            EvictPath::FreeTsg | EvictPath::Nothing => {
                let mut notes = vec![format!(
                    "deferred eviction of chid {chid}: a TSG member that never ran on the host; its TSG's host context is released with the host TSG [host]"
                )];
                self.wait_root_or_all(root.and_then(|t| match t {
                    Target::Fb(pa) => Some(pa),
                    _ => None,
                }));
                if !self.free_tsg(key, &mut notes) {
                    return Err(notes.join("; "));
                }
                return Ok((notes, reg.channel_gp_get(chid).unwrap_or(0)));
            }
        }
        let hc = self.channels.get_mut(&chid).filter(|hc| hc.generation == generation)
            .ok_or("deferred target host context is gone")?;
        if root != hc.shadow_root.map(Target::Fb) {
            return Err("deferred target VAS changed".into());
        }
        if hc.tsg != key {
            return Err("deferred target host channel is in another host TSG".into());
        }
        if reg.channels().filter(|c| tsg_key_of(reg, c).0 == key).count() != 1 {
            return Err("deferred eviction of a multi-channel TSG is not implemented".into());
        }
        hc.set_context_ready(false);

        // `FIFO_IDLE_CHANNELS` deschedules the target and waits for its pending engine work,
        // unlike GPGet (which proves only that PBDMA fetched the entries). Match management
        // takeover's bounded wait and require a successful completion before freeing RM state.
        let flags = ((sys::NVOS30_FLAGS_IDLE_PUSH_BUFFER | sys::NVOS30_FLAGS_IDLE_CACHE1
            | sys::NVOS30_FLAGS_IDLE_ALL_ENGINES) << 8) | sys::NVOS30_FLAGS_BEHAVIOR_SLEEP;
        let (status, idle_note) = self.idle_channels(reg, &[(client, object)], flags, 100_000)
            .ok_or("deferred target idle has no host target")?;
        if status != sys::NV_OK {
            return Err(idle_note);
        }

        let ch = reg.channel(chid).filter(|ch| ch.generation == generation)
            .ok_or("deferred target channel changed while idling")?;
        if reg.vas_key_of(ch).and_then(|vas| reg.vas_root(vas)) != root
            || reg.channels().filter(|c| tsg_key_of(reg, c).0 == key).count() != 1 {
            return Err("deferred target VAS or TSG changed while idling".into());
        }
        let hc = self.channels.get_mut(&chid)
            .filter(|hc| hc.generation == generation && hc.tsg == key)
            .ok_or("deferred target host context changed while idling")?;
        let doorbells = hc.doorbells;
        let get = hc.with_userd(|userd| {
            let get = userd.storage.read_u32(USERD_GP_GET);
            if get >= userd.entries {
                return Err(format!("deferred target final GPGet {get} is outside its {}-entry ring", userd.entries));
            }
            let put = userd.storage.read_u32(USERD_GP_PUT);
            let all_published_work_fetched = userd.published_put == Some(get);
            let untouched_empty_ring = userd.published_put.is_none()
                && doorbells == 0
                && put == get;
            if !all_published_work_fetched && !untouched_empty_ring {
                return Err("deferred target still has published work after idling".to_string());
            }
            userd.harvest(gm);
            Ok(get)
        })
        .ok_or("deferred target has no private USERD")??;

        // As at the management retirement: the root's plans, not the teardown.
        self.wait_root_or_all(root.and_then(|t| match t {
            Target::Fb(pa) => Some(pa),
            _ => None,
        }));
        let mut notes = vec![idle_note];
        if !self.free_tsg(key, &mut notes) { return Err(notes.join("; ")); }
        Ok((notes, get))
    }

    pub fn channel(&self, chid: u32) -> Option<&HostChannel> {
        self.channels.get(&chid)
    }

    /// The host error notifier of `chid`'s mirror when it is the device's own page (the
    /// guest's could not be mirrored): what the host RM wrote there. `None` otherwise.
    pub fn rc_notifier(&self, chid: u32) -> Option<[u8; crate::events::NOTIFIER_SIZE]> {
        let page = self.channels.get(&chid)?.rc_page.as_ref()?;
        let mut n = [0u8; crate::events::NOTIFIER_SIZE];
        page.read(&mut n);
        Some(n)
    }

    pub fn mirrored_channels(&self) -> usize {
        self.channels.len()
    }

    /// Raise the guest non-stall interrupt for software-executed work on `engine_type`.
    /// The same [`IntrShared`] path serves host-mirrored engine events, including interrupt
    /// page mode and MSI coalescing. Returns the vector, or `None` when this chip exposes no
    /// non-stall vector for the engine.
    pub fn raise_software_nonstall(&self, engine_type: u32) -> Option<u32> {
        let vector = nonstall_vector(self.chip, engine_type)?;
        self.intr.raise(vector);
        Some(vector)
    }

    /// Whether the registry's channel `chid` is one the mirror takes: a child of a
    /// guest TSG whose VA space has a known root inside the reservation.
    pub fn eligible(&self, reg: &Registry, chid: u32) -> Result<(u64, (u32, u32)), String> {
        if !self.channels_enabled {
            return Err("host channel execution off (CARAXES_MIRROR_CHANNELS=1 to enable); channel work runs on the software executor over the VRAM store".into());
        }
        let ch = reg.channel(chid).ok_or("no such channel")?;
        let gr0 = v::lookup(v::NV2080_ENGINE_TYPES, "GR0").unwrap_or(1);
        let bare = reg.tsg((ch.h_client, ch.h_parent)).is_none();
        // A bare video channel (libnvidia-encode's and libnvcuvid's, under the device with
        // no TSG and no VAS of its own) is mirrored in a host TSG of its own, as a bare CE
        // channel is under CARAXES_MIRROR_BARE_CE.
        let video = video_engine(ch.engine_type);
        if bare
            && video.is_none()
            && !(self.bare_ce && ch.engine_type != gr0 && ch.engine_type != 0)
        {
            return Err("bare channel (no TSG): stays with the software executor".into());
        }
        // Fail closed: a video channel carries only the chip's own video classes.
        if let Some(engine) = video {
            if let Some(c) = video_channel_foreign_class(self.chip, &ch.engine_objects) {
                return Err(format!(
                    "{engine} channel carries class {c:#x}, not a video class of this chip: refused, stays with the software executor"
                ));
            }
        }
        // nvidia-uvm's own channels bind GP100_UVM_SW on subchannel 5; the host refuses
        // that class to an unprivileged client (RS_FLAGS_ALLOC_PRIVILEGED), and a
        // SET_OBJECT of a class the host channel lacks is a PBDMA method error: Xid 32
        // and an RC, measured 2026-09-23 (execution.md). Those channels stay in software
        // on the VRAM store; their TLB invalidates are forwarded (Dispatcher::doorbell).
        if ch
            .engine_objects
            .iter()
            .any(|(c, _)| *c == self.chip.channel.uvm_sw)
        {
            return Err(format!(
                "carries GP100_UVM_SW {:#x} (kernel-privileged on the host): stays with the software executor",
                self.chip.channel.uvm_sw
            ));
        }
        // A GR channel needs a host GR context that agrees with the guest's promoted
        // context; binding the host's own golden context (c1) executes the guest's push
        // and faults (Xid 38, measured 2026-09-23), and promoting the guest's buffers
        // (c2) is unestablished (cuda-path.md §4.1). Until that wall is resolved, GR
        // channels stay in software on the VRAM store, so the guest's compute pushes are
        // decoded there (the kernel launch is refused as in E5c) and never RC the card.
        // CUDA's copy engines (COPY2-4 TSGs) run on the host. `TURING_COMPUTE_A` on the
        // channel, or a GRAPHICS-engine TSG, marks a GR channel.
        let (_, engine) = reg.tsg((ch.h_client, ch.h_parent)).unwrap_or((0, 0));
        let is_gr = engine == v::lookup(v::NV2080_ENGINE_TYPES, "GR0").unwrap_or(1)
            || ch
                .engine_objects
                .iter()
                .any(|(c, _)| *c == self.chip.channel.compute);
        // A guest privileged non-GR channel may issue physical-mode work (page-table
        // updates and scrubbing, alloc_channel.h NVOS04_FLAGS_PRIVILEGED_CHANNEL). The host
        // mirror is unprivileged (HOST_MIRROR_CHANNEL_FLAGS), so replaying it there would
        // silently drop its privilege and can fault the physical GPU: it stays in the
        // bounded software executor. Every other CE and video channel, bare or in a TSG,
        // remains eligible; its host channel denies physical-mode CE methods.
        if privileged_stays_in_software(reg, ch, gr0, is_gr) {
            return Err(format!(
                "channel may issue guest physical-mode work (engine {:#x}, allocation flags {:#x}, bare {bare}): host privilege is not inherited; stays with the bounded software executor",
                ch.engine_type, ch.alloc_flags
            ));
        }
        if is_gr && !self.mirror_gr {
            return Err(
                "GR channel: a host GR context that agrees with the guest's promoted context is not established (cuda-path.md §4.1); stays with the software executor (the kernel-launch wall)".into(),
            );
        }
        if !is_gr && !self.mirror_ce {
            return Err(
                "CE channel kept on the software executor (CARAXES_MIRROR_CE=0) for the GR experiment".into(),
            );
        }
        let key = reg
            .vas_key_of(ch)
            .ok_or("the channel's VA space has no root yet")?;
        let root = match reg.vas_root(key) {
            Some(Target::Fb(pa)) => pa,
            Some(other) => return Err(format!("VA space root {other:?} is not in vidmem")),
            None => return Err("the channel's VA space has no root yet".into()),
        };
        if root < self.base || root + PAGE > self.end {
            return Err(format!(
                "VA space root {root:#x} outside the reservation [{:#x}, {:#x}): refused",
                self.base, self.end
            ));
        }
        Ok((root, key))
    }

    fn ensure_events(&mut self, notes: &mut Vec<String>) {
        if self.events.is_some() {
            return;
        }
        let mut events: Vec<(OsEvent, Option<u32>)> = Vec::new();
        let mut handles = Vec::new();
        let mut want: Vec<(&str, u32, u32)> = Vec::new();
        for (name, notifier) in ENGINES {
            match engine_vector(self.chip, name) {
                Some(vec) => want.push((name, *notifier | EVENT_NONSTALL, vec)),
                None => notes.push(format!(
                    "events: {name} has no non-stall vector in the chip's interrupt table; its completions are not forwarded"
                )),
            }
        }
        {
            let mut h = self.hm.lock().unwrap();
            for (name, notifier, vector) in &want {
                let id = self.next_event_id;
                self.next_event_id += 1;
                match h.os_event(*notifier, id) {
                    Ok(ev) => {
                        notes.push(format!(
                            "events: host OS event on {name} notifier {:#x} (non-stall) -> guest vector {vector}, event {:#x} [host]",
                            notifier, ev.handle
                        ));
                        handles.push(ev.handle);
                        events.push((ev, Some(*vector)));
                    }
                    Err(e) => notes.push(format!("events: {name}: {e:#}")),
                }
            }
            let id = self.next_event_id;
            self.next_event_id += 1;
            match h.os_event(NOTIFIER_RC_ERROR, id) {
                Ok(ev) => {
                    notes.push(format!(
                        "events: host OS event on the RC notifier {NOTIFIER_RC_ERROR}, event {:#x} [host]",
                        ev.handle
                    ));
                    handles.push(ev.handle);
                    events.push((ev, None));
                }
                Err(e) => notes.push(format!("events: RC notifier: {e:#}")),
            }
        }
        let stop = Arc::new(AtomicBool::new(false));
        let intr = self.intr.clone();
        let stop2 = stop.clone();
        let thread = std::thread::Builder::new()
            .name("host-events".into())
            .spawn(move || event_loop(events, intr, stop2))
            .ok();
        self.events = Some(Events {
            stop,
            thread,
            handles,
        });
    }

    fn ensure_vas(
        &mut self,
        key: (u32, u32),
        root: u64,
        entries: u32,
        notes: &mut Vec<String>,
    ) -> Result<u32, String> {
        if let Some(why) = self.raw_refusal() {
            return Err(format!("raw externally-owned VAS refused for {why}; use the shadow VAS path"));
        }
        if let Some(hv) = self.vas.get(&key) {
            if hv.root == root || hv.replayed {
                return Ok(hv.handle);
            }
            notes.push(format!(
                "vaspace {:#x}/{:#x}: root moved {:#x} -> {root:#x}; the host mirror (and its channels) are rebuilt",
                key.0, key.1, hv.root
            ));
            self.drop_vas(key, notes);
        }
        let mut h = self.hm.lock().unwrap();
        let mut vp = sys::NV_VASPACE_ALLOCATION_PARAMETERS::default();
        vp.index = 0;
        vp.flags = VAS_IS_EXTERNALLY_OWNED;
        vp.vaSize = VAS_SIZE;
        vp.bigPageSize = BIG_PAGE_64K;
        let device = h.device();
        let handle = h
            .alloc_object(device, sys::FERMI_VASPACE_A, as_bytes(&mut vp), "vaspace")
            .map_err(|st| format!("host FERMI_VASPACE_A: {}", status_name(st)))?;
        // NV0080_CTRL_DMA_SET_PAGE_DIRECTORY_PARAMS: physAddress @0, numEntries @8,
        // flags @12 (APERTURE 1:0 = VIDMEM 0, ALL_CHANNELS bit 3), hVASpace @16, chId @20,
        // subDeviceId @24 (instance + 1), pasid @28.
        let mut sp = [0u8; SETPD_SIZE];
        sp[0..8].copy_from_slice(&root.to_le_bytes());
        sp[8..12].copy_from_slice(&entries.to_le_bytes());
        sp[12..16].copy_from_slice(&SETPD_ALL_CHANNELS.to_le_bytes());
        sp[16..20].copy_from_slice(&handle.to_le_bytes());
        sp[24..28].copy_from_slice(&1u32.to_le_bytes());
        let t = Instant::now();
        let st = h
            .control_admin(device, v::CMD_DMA_SET_PAGE_DIRECTORY, &mut sp)
            .map_err(|e| format!("DMA_SET_PAGE_DIRECTORY: {e:#}"))?;
        if st != sys::NV_OK {
            let _ = h.free_object(handle);
            return Err(format!(
                "DMA_SET_PAGE_DIRECTORY to {root:#x} ({entries} entries): {}",
                status_name(st)
            ));
        }
        notes.push(format!(
            "vaspace {:#x}/{:#x}: host FERMI_VASPACE_A {handle:#x} externally owned, page directory {root:#x} x {entries} (VIDMEM, all channels) set in {} us [host; guest root unchanged]",
            key.0,
            key.1,
            t.elapsed().as_micros()
        ));
        drop(h);
        self.vas.insert(
            key,
            HostVas {
                handle,
                root,
                replayed: false,
            },
        );
        Ok(handle)
    }

    /// The guest sent `COPY_SERVER_RESERVED_PDES` on a fresh VA space: build the host
    /// mirror now, the way the guest kernel builds it on GSP, by replaying the guest's
    /// own controls with the guest's own addresses (all inside the reservation): the
    /// `FERMI_VASPACE_A` parameters as sent, then `COPY_SERVER_RESERVED_PDES` with the
    /// guest's PDE chain pages (root, PD2, PD1, PD0 of the server-reserved range).
    /// The host RM adopts that chain as its own page levels (gpu_vaspace.c
    /// `gvaspaceCopyServerReservedPdes`), so it maps its GR context buffers into the
    /// reserved range through the guest's tables and `kchannelIsSchedulable` holds for
    /// GR channels (a bare externally-owned VAS can never schedule GR from user space:
    /// `bIsContextBound` is set only by the kernel-internal nv_gpu_ops path).
    pub fn on_server_reserved_pdes(&mut self, reg: &Registry, key: (u32, u32)) -> Vec<String> {
        if let Some(why) = self.raw_refusal() {
            self.stats.refusals += 1;
            return vec![format!("vaspace {:#x}/{:#x}: raw server-reserved PDE replay refused for {why}", key.0, key.1)];
        }
        // The replayed (shared-management) VA space is only needed to schedule GR
        // channels on the host; while GR stays in software (the launch wall), the CE
        // channels bind to an externally-owned mirror built at their first doorbell
        // (host-exec.md §1), so nothing is replayed here.
        let _ = reg;
        if !self.replay_vas {
            return Vec::new();
        }
        let mut notes = Vec::new();
        let key = reg.resolve(key);
        if let Some(hv) = self.vas.get(&key) {
            if hv.replayed {
                notes.push(format!(
                    "vaspace {:#x}/{:#x}: host mirror {:#x} already replayed",
                    key.0, key.1, hv.handle
                ));
                return notes;
            }
            self.drop_vas(key, &mut notes);
        }
        let (Some(vp), Some(pdes)) = reg.vas_replay(key) else {
            notes.push(format!(
                "vaspace {:#x}/{:#x}: no alloc parameters or PDE levels recorded; not mirrored",
                key.0, key.1
            ));
            return notes;
        };
        let root = match reg.vas_root(key) {
            Some(Target::Fb(pa)) if pa >= self.base && pa + PAGE <= self.end => pa,
            other => {
                notes.push(format!(
                    "vaspace {:#x}/{:#x}: root {other:?} is not in the reservation; not mirrored",
                    key.0, key.1
                ));
                return notes;
            }
        };
        let rd32 = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let rd64 = |b: &[u8], o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        // Every level page must lie in the reservation.
        let n = (rd32(pdes, v::PDES_NUM_LEVELS) as usize).min(v::PDES_MAX_LEVELS);
        for i in 0..n {
            let pa = rd64(
                pdes,
                v::PDES_LEVELS + v::PDES_LEVEL_SIZE * i + v::PDES_LEVEL_PHYS_ADDRESS,
            );
            if pa < self.base || pa + PAGE > self.end {
                notes.push(format!(
                    "vaspace {:#x}/{:#x}: PDE level {i} page {pa:#x} outside the reservation; refused",
                    key.0, key.1
                ));
                self.stats.refusals += 1;
                return notes;
            }
        }
        // The host kernel RM is itself a GSP client, so it reserves the same
        // `[4 GiB, 4.5 GiB)` for its server as the guest kernel did
        // (`gvaspaceReserveSplitVaSpace_IMPL`, gpu_vaspace.c:410-420) and hands the host
        // GSP its own levels; this replay then hands the GSP the guest's root, PD2 and
        // PD1 instead (the GSP adopts them, `gvaspaceCopyServerReservedPdes_IMPL`
        // :4296-4333, and the channel's PDB becomes the guest root: b2 measured the
        // GPFIFO fetched at a guest VA). The alloc parameters are the guest's, unchanged:
        // adding `VA_INTERNAL_LIMIT` (b2b, run 10) makes the RM-internal range equal the
        // server-reserved range (vaspace_api.c:370-375, :642-644), which leaves the host
        // kernel no VA for the context buffers it maps at `TURING_COMPUTE_A` allocation
        // (`NV_ERR_NO_MEMORY`, measured 2026-09-23). Only root/PD2/PD1 are handed over
        // (exec-formats.md §4.5 set C); the host GSP brings its own PD0 for the range.
        let (lo, hi) = (
            rd64(pdes, v::PDES_VIRT_ADDR_LO),
            rd64(pdes, v::PDES_VIRT_ADDR_HI),
        );
        let t = Instant::now();
        let mut h = self.hm.lock().unwrap();
        let device = h.device();
        let mut vpb = vp.to_vec();
        vpb.resize(vpb.len().max(40), 0);
        let handle = match h.alloc_object(device, sys::FERMI_VASPACE_A, &mut vpb, "vaspace") {
            Ok(x) => x,
            Err(st) => {
                notes.push(format!(
                    "vaspace {:#x}/{:#x}: host FERMI_VASPACE_A with the guest's parameters (flags {:#x}): {}; not mirrored",
                    key.0,
                    key.1,
                    rd32(vp, 4),
                    status_name(st)
                ));
                self.stats.refusals += 1;
                return notes;
            }
        };
        let mut pb = pdes.to_vec();
        pb[PDES_H_SUBDEVICE..PDES_H_SUBDEVICE + 4].copy_from_slice(&0u32.to_le_bytes());
        pb[PDES_SUBDEVICE_ID..PDES_SUBDEVICE_ID + 4].copy_from_slice(&0u32.to_le_bytes());
        let n = n.min(PDES_LEVELS_HANDED_OVER);
        pb[v::PDES_NUM_LEVELS..v::PDES_NUM_LEVELS + 4].copy_from_slice(&(n as u32).to_le_bytes());
        // PRIVILEGED (flags 0x14004, g_vaspace_api_nvoc.c), like DMA_SET_PAGE_DIRECTORY:
        // one CAP_SYS_ADMIN window (D12; the fourth control under it, execution.md).
        let st = h
            .control_admin(handle, v::CMD_VASPACE_COPY_SERVER_RESERVED_PDES, &mut pb)
            .unwrap_or(0xFFFF_FFFF);
        if st != sys::NV_OK {
            notes.push(format!(
                "vaspace {:#x}/{:#x}: host COPY_SERVER_RESERVED_PDES ({n} levels, root {root:#x}): {}; the mirror falls back to the externally-owned form at the first doorbell",
                key.0,
                key.1,
                status_name(st)
            ));
            let _ = h.free_object(handle);
            self.stats.refusals += 1;
            return notes;
        }
        drop(h);
        notes.push(format!(
            "vaspace {:#x}/{:#x}: host FERMI_VASPACE_A {handle:#x} with the guest's parameters unchanged (flags {:#x}, vaSize {:#x}), COPY_SERVER_RESERVED_PDES replayed ({n} levels: root {root:#x}, PD2, PD1; server range {lo:#x}..{hi:#x}) in {} us: the host GSP owns the server-reserved range in the guest's page-directory chain [host; guest addresses unchanged]",
            key.0,
            key.1,
            rd32(vp, 4),
            rd64(vp, 8),
            t.elapsed().as_micros()
        ));
        self.vas.insert(
            key,
            HostVas {
                handle,
                root,
                replayed: true,
            },
        );
        notes
    }

    /// Before a channel joins or builds the host TSG under `key`: an entry that stands for
    /// a guest TSG the registry no longer knows is freed (its FREE never reached the
    /// mirror, and the kernel reuses its id); two live guest TSGs under one kernel id are
    /// refused.
    fn check_tsg_reuse(
        &mut self,
        reg: &Registry,
        key: TsgKey,
        guest: (u32, u32),
        notes: &mut Vec<String>,
    ) -> Result<(), String> {
        let Some(existing) = self.tsgs.get(&key).map(|t| t.guest) else {
            return Ok(());
        };
        match tsg_reuse(reg, existing, guest) {
            TsgReuse::Join => Ok(()),
            TsgReuse::Stale => {
                notes.push(format!(
                    "tsg {key}: host TSG of the freed guest TSG {:#x}/{:#x} dropped before guest TSG {:#x}/{:#x} takes the id [host]",
                    existing.0, existing.1, guest.0, guest.1
                ));
                self.free_tsg(key, notes);
                Ok(())
            }
            TsgReuse::Collision => Err(format!(
                "tsg {key}: guest TSGs {:#x}/{:#x} and {:#x}/{:#x} are both live under one kernel TSG id; refused, stays with the software executor",
                existing.0, existing.1, guest.0, guest.1
            )),
        }
    }

    fn ensure_tsg(
        &mut self,
        key: TsgKey,
        guest: (u32, u32),
        vas_key: (u32, u32),
        vas_handle: u32,
        engine: u32,
        notes: &mut Vec<String>,
    ) -> Result<u32, String> {
        if let Some(t) = self.tsgs.get(&key) {
            return Ok(t.handle);
        }
        let mut h = self.hm.lock().unwrap();
        // The host driver's layout (hostabi.rs).
        let mut tp = crate::hostabi::current().tsg_params(vas_handle, engine);
        let device = h.device();
        let handle = h
            .alloc_object(
                device,
                sys::KEPLER_CHANNEL_GROUP_A,
                &mut tp,
                "tsg",
            )
            .map_err(|st| {
                format!(
                    "host KEPLER_CHANNEL_GROUP_A engineType {engine:#x}: {}",
                    status_name(st)
                )
            })?;
        notes.push(format!(
            "tsg {:#x}/{:#x} ({key}): host TSG {handle:#x} engineType {engine:#x} on vaspace {vas_handle:#x} [host]",
            guest.0, guest.1
        ));
        drop(h);
        self.tsgs.insert(
            key,
            HostTsg {
                handle,
                vas: vas_key,
                engine,
                guest,
                gfxp: None,
                preempt: TsgPreempt::default(),
            },
        );
        Ok(handle)
    }

    /// The host mirror of guest channel `chid`, created now if it does not exist.
    /// Returns the host token and the notes of what was done.
    pub fn ensure_channel(
        &mut self,
        reg: &Registry,
        gm: &mut GpuMem,
        chid: u32,
    ) -> Result<(u32, Vec<String>), String> {
        if let Some(why) = self.raw_refusal() {
            return Err(format!("raw host channel construction refused for {why}; use the shadow channel path"));
        }
        let mut notes = Vec::new();
        if let Some(c) = self.channels.get(&chid) {
            return Ok((c.token, notes));
        }
        // The host RM clears the USERD it is given when it creates the channel, and the
        // guest has already written GPPut for the submission that rang this doorbell
        // (measured 2026-09-23: GPPut 1 -> 0, the push never ran). Keep the guest's
        // GPGet/GPPut and put them back after the alloc.
        let userd_ptrs = reg.channel(chid).and_then(|c| c.userd).map(|u| {
            let at = |off: u64| match u {
                Target::Fb(pa) => Target::Fb(pa + off),
                Target::Sys(pa) => Target::Sys(pa + off),
                t => t,
            };
            (
                at(USERD_GP_GET),
                at(USERD_GP_PUT),
                gm.read_u32(at(USERD_GP_GET)),
                gm.read_u32(at(USERD_GP_PUT)),
            )
        });
        let mem: &GuestMemory = gm.sys;
        let t0 = Instant::now();
        let (root, vas_key) = self.eligible(reg, chid)?;
        let ch: Channel = reg.channel(chid).ok_or("no such channel")?.clone();
        // A bare CE channel (`bare_ce`) gets a host TSG of its own, keyed by its internal
        // TSG id (615) or its handle.
        let (tsg_key, bare, tsg_guest) = tsg_key_of(reg, &ch);
        // The guest's recorded promote entries for this channel (c2 replay input).
        let promote_entries: Vec<crate::channel::PromoteEntry> = reg
            .promoted((ch.h_client, ch.h_object))
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        let engine = if bare {
            ch.engine_type
        } else {
            reg.tsg(tsg_guest).ok_or("no TSG")?.1
        };
        self.ensure_events(&mut notes);
        self.ensure_invalidator(gm.fb, &mut notes);
        let entries = reg.root_entries(vas_key);
        let vas_handle = self.ensure_vas(vas_key, root, entries, &mut notes)?;
        self.check_tsg_reuse(reg, tsg_key, tsg_guest, &mut notes)?;
        let tsg_handle =
            self.ensure_tsg(tsg_key, tsg_guest, vas_key, vas_handle, engine, &mut notes)?;

        let mut h = self.hm.lock().unwrap();
        let mut owned: Vec<u32> = Vec::new();
        let mut rc_page: Option<crate::hostmem::DevicePage> = None;
        let r = (|| -> Result<Built, String> {
            // USERD: the guest's page, as the host memory object over the same bytes.
            // P21 design H: the USERD's pool page when it lives in the pool (fb.rs `remap`).
            let (userd_mem, userd_off) = match ch.userd.map(|t| match t {
                Target::Fb(pa) => Target::Fb(gm.fb.remap(pa)),
                t => t,
            }) {
                Some(Target::Fb(pa)) => {
                    if pa < self.base || pa + PAGE > self.end {
                        return Err(format!("USERD {pa:#x} outside the reservation: refused"));
                    }
                    (self.reservation, pa - self.base)
                }
                Some(Target::Sys(iova)) => {
                    let gpa = mem
                        .translate(iova, PAGE)
                        .ok_or(format!("USERD IOVA {iova:#x} is not registered guest RAM"))?;
                    let ptr = mem
                        .host_ptr(gpa & !(PAGE - 1), PAGE)
                        .ok_or(format!("USERD GPA {gpa:#x} not mapped"))?;
                    let d = h
                        .guest_page_object(ptr, PAGE, "userd-page")
                        .map_err(|e| format!("USERD OS descriptor: {e:#}"))?;
                    owned.push(d);
                    (d, iova & (PAGE - 1))
                }
                _ => return Err("channel without USERD".into()),
            };
            // The error notifier: the guest's page (sysmem, page aligned), written by
            // the host RM on an RC of this channel, read by the guest's user space.
            let h_error = match ch.error_notifier {
                Some(Target::Sys(iova)) if iova % PAGE == 0 => {
                    match mem
                        .translate(iova, PAGE)
                        .and_then(|gpa| mem.host_ptr(gpa, PAGE))
                    {
                        Some(ptr) => match h.guest_page_object(ptr, PAGE, "error-notifier") {
                            Ok(d) => {
                                owned.push(d);
                                d
                            }
                            Err(e) => {
                                notes.push(format!(
                                    "chid {chid}: error notifier not mirrored: {e:#}"
                                ));
                                0
                            }
                        },
                        None => 0,
                    }
                }
                _ => 0,
            };
            let h_error = if h_error != 0 {
                h_error
            } else {
                match h.device_page_object("rc-notifier") {
                    Ok((d, page)) => {
                        owned.push(d);
                        rc_page = Some(page);
                        d
                    }
                    Err(e) => {
                        notes.push(format!(
                            "chid {chid}: no host error notifier ({e:#}); an RC of this channel reaches the guest only as a fence timeout [host]"
                        ));
                        0
                    }
                }
            };
            let mut cp = crate::hostabi::ChannelAllocParams::default();
            cp.flags = HOST_MIRROR_CHANNEL_FLAGS;
            cp.hObjectError = h_error;
            cp.gpFifoOffset = ch.gpfifo_va;
            cp.gpFifoEntries = ch.gpfifo_entries;
            cp.hUserdMemory[0] = userd_mem;
            cp.userdOffset[0] = userd_off;
            cp.engineType = ch.engine_type;
            let mut st = NV_ERR_INVALID_ADDRESS;
            let mut handle = 0;
            for attempt in 0..4 {
                match h.alloc_object(
                    tsg_handle,
                    crate::board::host_class_of(self.chip.channel.gpfifo),
                    as_bytes(&mut cp),
                    "channel",
                ) {
                    Ok(hh) => {
                        handle = hh;
                        owned.push(hh);
                        st = sys::NV_OK;
                        break;
                    }
                    Err(e) => {
                        st = e;
                        if e != NV_ERR_INVALID_ADDRESS {
                            break;
                        }
                        notes.push(format!(
                            "chid {chid}: channel alloc attempt {attempt}: INVALID_ADDRESS (page-table read raced the BAR1 write), retrying"
                        ));
                    }
                }
            }
            if st != sys::NV_OK {
                return Err(format!(
                    "host TURING_CHANNEL_GPFIFO_A (gpfifo va {:#x} x {}, userd {:?}): {}",
                    ch.gpfifo_va,
                    ch.gpfifo_entries,
                    ch.userd,
                    status_name(st)
                ));
            }
            notes.push(format!(
                "chid {chid}: host channel {handle:#x} in TSG {tsg_handle:#x}: gpfifo va {:#x} x {} [guest], userd {:?} -> host object {userd_mem:#x}+{userd_off:#x}, error notifier {} [host]",
                ch.gpfifo_va,
                ch.gpfifo_entries,
                ch.userd,
                if h_error != 0 { "the guest's page" } else { "none" }
            ));
            // The engine objects, with the guest's own parameters.
            let mut objects = Vec::new();
            let mut engine_map = BTreeMap::new();
            for (guest_handle, class, params) in ch.engine_objects_with_handles() {
                let mut p = params.to_vec();
                match h.alloc_object(handle, crate::board::host_class_of(class), &mut p, "engine-object") {
                    Ok(oh) => {
                        notes.push(format!(
                            "chid {chid}: host object class {class:#x}{} {oh:#x} ({} bytes of the guest's params) [host]",
                            crate::board::mapped_note(class),
                            params.len()
                        ));
                        objects.push((class, oh));
                        engine_map.insert(guest_handle, oh);
                    }
                    Err(e) => notes.push(format!(
                        "chid {chid}: host object class {class:#x} refused: {} (continuing)",
                        status_name(e)
                    )),
                }
            }
            if objects.is_empty() {
                return Err("no engine object could be created on the host channel; GPFIFO_SCHEDULE needs one".into());
            }
            // c2 (exec-formats.md §4.5): a GR channel gets, after its compute object and
            // before it is scheduled (the order the guest RM uses, `_kgrAlloc` ->
            // `kgrobjPromoteContext`), a `GPU_PROMOTE_CTX` for the host channel with the
            // guest's recorded entries. A refusal keeps the channel off the host: it would
            // otherwise run with a context the guest never sees (b2: Xid 38).
            if self.mirror_gr && objects.iter().any(|(c, _)| *c == self.chip.channel.compute) {
                let (st, note) =
                    promote_gr(&mut h, handle, &promote_entries, self.base, self.end);
                notes.push(note);
                if st != sys::NV_OK {
                    return Err(format!(
                        "c2 GPU_PROMOTE_CTX refused ({}): the GR channel stays with the software executor",
                        status_name(st)
                    ));
                }
            }
            let mut sched = [1u8, 0, 0];
            let st = h.control(handle, CMD_GPFIFO_SCHEDULE, &mut sched);
            if st != sys::NV_OK {
                return Err(format!("host GPFIFO_SCHEDULE: {}", status_name(st)));
            }
            let mut tok = [0u8; 4];
            let st = h.control(handle, CMD_GET_WORK_SUBMIT_TOKEN, &mut tok);
            if st != sys::NV_OK {
                return Err(format!("host GET_WORK_SUBMIT_TOKEN: {}", status_name(st)));
            }
            Ok((handle, objects, engine_map, u32::from_le_bytes(tok)))
        })();
        // The host RM clears the USERD it is given; the guest's GPGet/GPPut go back
        // whether the mirror was built or the software executor keeps the channel.
        let restore_userd = |gm: &mut GpuMem, notes: &mut Vec<String>| {
            if let Some((get_at, put_at, Some(get), Some(put))) = userd_ptrs {
                let (now_get, now_put) = (gm.read_u32(get_at), gm.read_u32(put_at));
                if now_get != Some(get) || now_put != Some(put) {
                    gm.write(get_at, &get.to_le_bytes());
                    gm.write(put_at, &put.to_le_bytes());
                    notes.push(format!(
                        "chid {chid}: the host RM cleared the USERD at channel creation (GPGet {:?} GPPut {:?}); the guest's GPGet {get} / GPPut {put} restored [guest]",
                        now_get, now_put
                    ));
                }
            }
        };
        match r {
            Ok((handle, objects, engine_map, token)) => {
                drop(h);
                restore_userd(gm, &mut notes);
                let us = t0.elapsed().as_micros();
                self.stats.channels_created += 1;
                self.stats.max_create_us = self.stats.max_create_us.max(us);
                notes.push(format!(
                    "chid {chid}: scheduled; host token {token:#x} (host chid {}, runlist {}) for guest token chid {chid}; mirror built in {us} us [host]",
                    token & 0xFFF,
                    (token >> 16) & 0x7F
                ));
                let pages = owned.iter().copied().filter(|d| *d != handle).collect();
                self.channels.insert(
                    chid,
                    HostChannel {
                        generation: ch.generation,
                        handle,
                        tsg: tsg_key,
                        token,
                        objects,
                        engine_map,
                        pages,
                        created: t0,
                        doorbells: 0,
                        shadow_root: None,
                        fwd_put: None,
                        fwd_unwaited: 0,
                        userd: None,
                        context_ready: true,
                        rc_page,
                    },
                );
                Ok((token, notes))
            }
            Err(e) => {
                // Free what this attempt made, newest first (the channel frees its
                // objects), and give the guest its USERD back for the software executor.
                for d in owned.into_iter().rev() {
                    let _ = h.free_object(d);
                }
                drop(h);
                restore_userd(gm, &mut notes);
                self.stats.refusals += 1;
                Err(format!("{e}; notes so far: {}", notes.join(" | ")))
            }
        }
    }

    /// [`Self::root_exclusions`] for diagnostics (ctrl.rs `gr_exception_postmortem`).
    pub fn set_b_of(&self, reg: &Registry, root: u64) -> Vec<(u64, u64)> {
        self.root_exclusions(reg, root, None)
    }

    /// The set-B exclusions of shadow root `root` now: the live channels' ([`exclusions_of`])
    /// and those held for a host TSG not yet freed ([`set_b_with_held`]).
    fn root_exclusions(&self, reg: &Registry, root: u64, fb: Option<&FakeFb>) -> Vec<(u64, u64)> {
        let hm = &self.hm;
        let fence = self.fence_of(root);
        self.extents
            .set_b(reg, root, fb, self.shadows.get(&root), fence, &|b, e| {
                Self::host_ctx_size(hm, b, e)
            })
    }

    /// The host fence (F2b) of shadow root `root`: its shadow's, or for a shadow not built
    /// yet the one its GR channel will get (`CARAXES_SHADOW_FENCE`).
    fn fence_of(&self, root: u64) -> Option<(u64, u64)> {
        self.shadows.get(&root).map_or_else(
            || {
                self.shadow_fence
                    .then(crate::shadow::Layout::fenced)
                    .and_then(|l| l.fence)
            },
            |sp| sp.front.layout.fence,
        )
    }

    /// Guest roots whose held set-B ranges went back to set A at a host TSG free since the
    /// last call (only those whose shadow still stands): the caller re-syncs them, so the
    /// guest's pages there are mapped after the host's context is gone.
    pub fn take_released_roots(&mut self) -> Vec<u64> {
        let mut out: Vec<u64> = std::mem::take(&mut self.released_roots)
            .into_iter()
            .filter(|r| self.shadows.contains_key(r))
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// A line for the device trace from a caller whose own notes are not traced (the BAR1
    /// pool's re-sync), through the interrupt tree's note queue.
    pub fn trace_note(&self, line: String) {
        self.intr.note(line);
    }

    /// Recompute set B of `root` (with the guest's cached walk from `fb`, [`global_exclusion`])
    /// when one of its inputs changed ([`GlobalExtents::refresh`]); the notes, and whether
    /// set B changed.
    fn refresh_exclusions(
        &mut self,
        reg: &Registry,
        root: u64,
        fb: &FakeFb,
    ) -> (Vec<String>, bool) {
        let hm = &self.hm;
        let query = |b, e| Self::host_ctx_size(hm, b, e);
        match self.shadows.get_mut(&root) {
            Some(sp) => self.extents.refresh(reg, root, fb, sp, &query),
            None => {
                let fence = self.fence_of(root);
                self.extents.set_b(reg, root, Some(fb), None, fence, &query);
                (self.extents.take_notes(), false)
            }
        }
    }

    /// Whether a video object on mirrored channel `chid` is refused to the guest before it
    /// is recorded (fail closed): with `CARAXES_MIRROR_VIDEO_OBJECT=0`, or when the channel's
    /// shadow VAS has the default layout (built for a GR or CE channel first), where the
    /// host's context would land on the guest's RM base. Returns the refusal line.
    pub fn refuse_video_object(&self, chid: u32, class: u32) -> Option<String> {
        if !video_class_ok(self.chip, class) {
            return None;
        }
        let root = self.channels.get(&chid)?.shadow_root?;
        let why = if !self.video_objects {
            "CARAXES_MIRROR_VIDEO_OBJECT=0".to_string()
        } else if self.shadows.get(&root).is_none_or(|sp| sp.front.layout.va_base == 0) {
            format!("the shadow VAS of guest root {root:#x} has the default layout (built for a GR or CE channel first): no host-only area for the host's context")
        } else {
            return None;
        };
        Some(format!(
            "chid {chid}: video object {class:#x} refused (fail closed): {why} [host]"
        ))
    }

    /// A failed host FREE keeps the old engine handle occupied until channel teardown.
    pub fn engine_handle_in_use(&self, reg: &Registry, client: u32, object: u32) -> bool {
        self.channels.iter().any(|(chid, hc)| {
            reg.channel(*chid).is_some_and(|ch| ch.h_client == client && ch.generation == hc.generation)
                && hc.engine_map.contains_key(&object)
        })
    }

    /// An engine object the guest allocated on an already mirrored channel.
    pub fn add_engine_object(&mut self, chid: u32, guest_handle: u32, class: u32, params: &[u8]) -> Option<String> {
        // A channel whose mirror build is in flight gets the object at the commit.
        if let Some(b) = self.builds.get_mut(&chid) {
            b.late_objects.push((guest_handle, class, params.to_vec()));
            return Some(format!(
                "chid {chid}: engine object {class:#x} added at its mirror build's commit [host]"
            ));
        }
        let hc = self.channels.get_mut(&chid)?;
        if hc.engine_map.contains_key(&guest_handle) {
            return Some(format!("chid {chid}: duplicate live engine handle {guest_handle:#x} refused"));
        }
        // A video object on a shadow channel: placed by `place_video_object` once the
        // guest's own context page has left the shadow.
        if video_class_ok(self.chip, class) && hc.shadow_root.is_some() {
            if hc.objects.iter().any(|(c, _)| video_class_ok(self.chip, *c))
                || self.video_pending.contains_key(&chid)
            {
                return Some(format!(
                    "chid {chid}: a second video object {class:#x} on a mirrored channel: refused"
                ));
            }
            self.video_pending.insert(chid, (guest_handle, class, params.to_vec()));
            return Some(format!(
                "chid {chid}: video object {class:#x} deferred until the guest's context page has left the shadow [host]"
            ));
        }
        let mut p = params.to_vec();
        let r = self
            .hm
            .lock()
            .unwrap()
            .alloc_object(hc.handle, crate::board::host_class_of(class), &mut p, "engine-object");
        Some(match r {
            Ok(oh) => {
                hc.objects.push((class, oh));
                hc.engine_map.insert(guest_handle, oh);
                format!("chid {chid}: host object class {class:#x} {oh:#x} added [host]")
            }
            Err(e) => format!(
                "chid {chid}: host object class {class:#x} refused: {}",
                status_name(e)
            ),
        })
    }

    /// The deferred video object of shadow channel `chid` (`add_engine_object`), placed on
    /// the host, or refused.
    ///
    /// The host kernel RM allocates the channel's falcon context itself at the object's
    /// allocation (`kflcnAllocContext`, kernel_falcon.c:279-298), maps it first fit
    /// (`kchannelMapEngineCtxBuf`, kernel_channel.c:3848-3870) and binds it to the engine at
    /// that VA; the guest's own context VA (its `GPU_PROMOTE_CTX` reaches only this device)
    /// does not matter to the host engine. The channel's shadow VAS has the video layout
    /// (shadow.rs `Layout`), so the first fit is the host-only area the guest never maps. Two
    /// 4 KiB probes check it ([`context_verdict`]): if the host's context is not in the host
    /// area, its object is freed before the guest can map anything more and the guest's
    /// allocation is refused (fail closed). The object is a child of the mirrored channel,
    /// in the TSG the mirror made for it (no implicit channel group), so the channel's free
    /// (`free_channel`) frees the object and the RM frees its context with it
    /// (`kflcnFreeContext`). Returns the notes and whether the guest's allocation is refused.
    pub fn place_video_object(&mut self, chid: u32) -> (Vec<String>, bool) {
        let Some((guest_handle, class, params)) = self.video_pending.remove(&chid) else {
            return (Vec::new(), false);
        };
        let Some(hc) = self.channels.get(&chid) else {
            return (vec![format!("chid {chid}: video object {class:#x}: the channel went away")], true);
        };
        let (handle, root) = (hc.handle, hc.shadow_root);
        let Some(sp) = root.and_then(|r| self.shadows.get(&r)) else {
            return (vec![format!("chid {chid}: video object {class:#x} refused: no shadow VAS [host]")], true);
        };
        let (vas, layout) = (sp.vas, sp.front.layout);
        // Every plan submitted so far is applied: the probes see the shadow as the guest left it.
        self.worker.wait_all();
        let mut h = self.hm.lock().unwrap();
        let probe = |h: &mut HostMemory| match h.alloc_virtual_probe(vas, 0x1000) {
            Ok((ph, va)) => {
                let _ = h.free_object(ph);
                Ok(va)
            }
            Err(st) => Err(st),
        };
        let before = probe(&mut h);
        // The first fit outside the host area already: refused without an object.
        if let Err(line) = host_first_fit(&layout, before) {
            return (
                vec![format!("chid {chid}: video object {class:#x} refused (fail closed): {line} [host]")],
                true,
            );
        }
        let mut p = params;
        let oh = match h.alloc_object(handle, crate::board::host_class_of(class), &mut p, "video-object") {
            Ok(oh) => oh,
            Err(e) => {
                return (
                    vec![format!(
                        "chid {chid}: host video object class {class:#x} refused: {}; the guest's is refused too [host]",
                        status_name(e)
                    )],
                    true,
                );
            }
        };
        let after = probe(&mut h);
        match context_verdict(&layout, before, after) {
            Ok(line) => {
                drop(h);
                if let Some(hc) = self.channels.get_mut(&chid) {
                    hc.objects.push((class, oh));
                    hc.engine_map.insert(guest_handle, oh);
                }
                (vec![format!("chid {chid}: host video object class {class:#x} {oh:#x}: {line} [host]")], false)
            }
            Err(line) => {
                let _ = h.free_object(oh);
                (
                    vec![format!(
                        "chid {chid}: video object {class:#x} refused (fail closed): {line}; host object {oh:#x} freed before the guest maps more [host]"
                    )],
                    true,
                )
            }
        }
    }

    /// The guest rang the doorbell of a mirrored channel: ring the host's.
    pub fn doorbell(&mut self, chid: u32) -> Result<String, String> {
        let token = self.channels.get(&chid).ok_or("not mirrored")?.token;
        if self.doorbell.is_none() {
            let (h, map) = self
                .hm
                .lock()
                .unwrap()
                .map_doorbell()
                .map_err(|e| format!("host doorbell: {e:#}"))?;
            self.doorbell = Some((h, map));
        }
        let (_, map) = self.doorbell.as_ref().expect("mapped");
        map.write_u32(DOORBELL_OFFSET, token);
        crate::launch::host_doorbell();
        self.stats.doorbells += 1;
        let hc = self.channels.get_mut(&chid).expect("present");
        hc.doorbells += 1;
        Ok(format!(
            "doorbell: guest chid {chid} -> host token {token:#x} written to the host doorbell page ({} on this channel) [host]",
            hc.doorbells
        ))
    }

    /// The end of the guest FB a guest leaf may map: the device's private pages at the top
    /// of the reservation (the device CE's and the invalidator's page tables and rings,
    /// devce.rs `PRIVATE_AT`) are no guest memory.
    fn leaf_end(&self) -> u64 {
        self.end - crate::devce::PRIVATE_AT
    }

    /// Build the device CE once (devce.rs) and hand it to the FB store. A refusal leaves the
    /// software CE on the CPU, with one line.
    pub fn ensure_device_ce(&mut self, fb: &mut FakeFb) {
        if self.device_ce_tried {
            return;
        }
        self.device_ce_tried = true;
        let mut h = self.hm.lock().unwrap();
        let Some((host_base, host_end)) = h.reservation().map(|r| (r.base, r.end())) else {
            return;
        };
        let built = crate::devce::DeviceCe::new(
            &mut h,
            fb,
            (self.base, self.end),
            self.usable_end,
            (host_base, host_end),
            self.reservation,
            crate::board::host_class_of(self.chip.channel.gpfifo),
            crate::board::host_class_of(self.chip.channel.copy),
        );
        drop(h);
        match built {
            Ok((ce, notes)) => {
                for n in notes {
                    eprintln!("vfio-user-nvgpu: {n}");
                    self.intr.note(n);
                }
                let ce = Arc::new(Mutex::new(ce));
                let shared: crate::devce::SharedEngine = ce.clone();
                fb.set_device_ce(Some(shared));
                self.device_ce = Some(ce);
            }
            Err(e) => {
                let line = format!("device CE: not built ({e}); the software CE stays on the CPU");
                eprintln!("vfio-user-nvgpu: {line}");
                self.intr.note(line);
            }
        }
    }

    fn ensure_invalidator(&mut self, fb: &mut FakeFb, notes: &mut Vec<String>) {
        if let Some(why) = self.raw_refusal() {
            notes.push(format!("invalidator: raw physical invalidator refused for {why}; shadow VAS invalidation remains active"));
            return;
        }
        if self.invalidator.is_some() || !self.memop_invalidate {
            return;
        }
        let mut h = self.hm.lock().unwrap();
        match Invalidator::new(
            &mut h,
            fb,
            self.base,
            self.end,
            self.reservation,
            crate::board::host_class_of(self.chip.channel.gpfifo),
            crate::board::host_class_of(self.chip.channel.copy),
        ) {
            Ok((inv, n)) => {
                notes.extend(n);
                self.invalidator = Some(inv);
            }
            Err(e) => notes.push(format!(
                "invalidator: NOT built ({e}); the guest's TLB invalidates cannot reach the physical GPU (stale PDEs: FAULT_PDE on fresh mappings)"
            )),
        }
    }

    /// The guest invalidated the TLB for page directory `pdb` (by register, or by a
    /// `MEM_OP` on a software channel; `None`: every page directory): the same on every
    /// mirror VA space rooted there. Default: `DMA_INVALIDATE_TLB` on the mirror VA
    /// space (for an externally-owned one the RM's walker root *is* the guest root after
    /// `DMA_SET_PAGE_DIRECTORY`; for a replayed one it is the host kernel's own root,
    /// exec-formats.md §4.4). Opt-in: a `MEM_OP` `MMU_TLB_INVALIDATE` of each distinct
    /// guest root from the invalidator channel (see `invalidator`). Returns how many.
    /// Whether a host VA space walks the guest's own page tables (a mirror VAS forwarded
    /// the guest's page directory): then the host MMU reads them from VRAM, and the BAR2
    /// memfd's invalidate syncs every live page, not only the tables the device walks
    /// (boot.rs `mmu_invalidate_write`).
    pub fn walks_guest_tables(&self) -> bool {
        !self.vas.is_empty()
    }

    pub fn invalidate_tlb(&mut self, fb: &mut FakeFb, pdb: Option<u64>) -> usize {
        if let Some(inv) = self.invalidator.as_mut() {
            let mut roots: Vec<u64> = self
                .vas
                .values()
                .filter(|v| pdb.is_none_or(|p| v.root == p))
                .map(|v| v.root)
                .collect();
            roots.sort_unstable();
            roots.dedup();
            for root in &roots {
                match inv.invalidate(fb, *root) {
                    Ok(n) => {
                        if inv.pushes <= 3 || inv.pushes % 256 == 0 {
                            self.intr.note(n);
                        }
                    }
                    Err(e) => self.intr.note(format!("mirror: {e}")),
                }
            }
            self.stats.invalidates += roots.len() as u64;
            return roots.len();
        }
        let targets: Vec<u32> = self
            .vas
            .values()
            .filter(|v| pdb.is_none_or(|p| v.root == p))
            .map(|v| v.handle)
            .collect();
        if targets.is_empty() {
            return 0;
        }
        let h = self.hm.lock().unwrap();
        let device = h.device();
        for t in &targets {
            let mut p = [0u8; 8];
            p[0..4].copy_from_slice(&t.to_le_bytes());
            let t0 = Instant::now();
            let st = h.control(device, CMD_DMA_INVALIDATE_TLB, &mut p);
            let us = t0.elapsed().as_micros();
            if st != sys::NV_OK || self.stats.invalidates < 3 {
                self.intr.note(format!(
                    "mirror: DMA_INVALIDATE_TLB on vaspace {t:#x}: {} in {us} us [host]",
                    status_name(st)
                ));
            }
        }
        self.stats.invalidates += targets.len() as u64;
        targets.len()
    }

    /// A control the guest sent to one of its channels or TSGs, forwarded to the host
    /// mirror when there is one (`STOP_CHANNEL`, `GPFIFO_SCHEDULE`, TSG `PREEMPT`,
    /// `SET_TIMESLICE`). `None` when the object has no mirror.
    pub fn forward_control(
        &mut self,
        reg: &Registry,
        h_client: u32,
        h_object: u32,
        cmd: u32,
        params: &[u8],
    ) -> Option<(u32, String)> {
        // A TSG (or a bare channel's own TSG) by its key: the kernel's id on 615.
        let tsg = reg
            .tsg(reg.resolve((h_client, h_object)))
            .and_then(|_| tsg_key_of_guest(reg, reg.resolve((h_client, h_object))))
            .and_then(|k| self.tsgs.get(&k));
        let target = if let Some(t) = tsg {
            Some(("tsg", t.handle))
        } else {
            reg.channels()
                .find(|c| c.h_client == h_client && c.h_object == h_object)
                .and_then(|c| self.channels.get(&c.chid))
                .map(|hc| ("channel", hc.handle))
        };
        let (kind, handle) = target?;
        let mut p = params.to_vec();
        let st = self.hm.lock().unwrap().control(handle, cmd, &mut p);
        Some((
            st,
            format!(
                "control {cmd:#x} forwarded to the host {kind} {handle:#x}: {} [host]",
                status_name(st)
            ),
        ))
    }

    /// The host channel handle of guest channel `(hClient, hChannel)`: the mirror's own
    /// host object, found through the registry by the guest's handle; `None` when the
    /// channel has no mirror.
    pub fn host_channel_of(&self, reg: &Registry, h_client: u32, h_channel: u32) -> Option<(u32, u32)> {
        let c = reg
            .channels()
            .find(|c| c.h_client == h_client && c.h_object == h_channel)?;
        self.channels.get(&c.chid).map(|hc| (c.chid, hc.handle))
    }

    /// `IDLE_CHANNELS` for the guest channels `list` that run on a host mirror (gsprpc.rs,
    /// public.rs): `NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS` (ctrl0080fifo.h:367-401; export
    /// flags 0x109: NON_PRIVILEGED, g_device_nvoc.c) on the mirror's own host device, over
    /// the mirror's own host channel handles of the same host client only; the host RM
    /// deschedules them and waits for their pending work (kernel_fifo_ctrl.c:131-186
    /// forwards it to the host GSP). Channels of the list without a mirror are left out;
    /// `None` when none has one. `timeout_us` is the caller's, already capped.
    pub fn idle_channels(
        &self,
        reg: &Registry,
        list: &[(u32, u32)],
        flags: u32,
        timeout_us: u32,
    ) -> Option<(u32, String)> {
        let hosts: Vec<(u32, u32)> = list
            .iter()
            .filter_map(|&(c, h)| self.host_channel_of(reg, c, h))
            .take(IDLE_MAX_CHANNELS)
            .collect();
        if hosts.is_empty() {
            return None;
        }
        Some(self.idle_host_channels(&hosts, flags, timeout_us))
    }

    /// Idle exact host handles already owned by this mirror. Teardown uses this after the
    /// guest registry entry may have disappeared, so it must not rediscover a target by a
    /// reused guest handle.
    fn idle_host_channels(
        &self,
        hosts: &[(u32, u32)],
        flags: u32,
        timeout_us: u32,
    ) -> (u32, String) {
        let mut p = vec![0u8; IDLE_PARAMS_SIZE];
        p[0..4].copy_from_slice(&(hosts.len() as u32).to_le_bytes());
        for (i, (_, handle)) in hosts.iter().enumerate() {
            p[4 + 4 * i..8 + 4 * i].copy_from_slice(&handle.to_le_bytes());
        }
        p[IDLE_PARAMS_FLAGS..IDLE_PARAMS_FLAGS + 4].copy_from_slice(&flags.to_le_bytes());
        p[IDLE_PARAMS_TIMEOUT..IDLE_PARAMS_TIMEOUT + 4].copy_from_slice(&timeout_us.to_le_bytes());
        let h = self.hm.lock().unwrap();
        let t0 = Instant::now();
        let st = h.control(h.device(), CMD_FIFO_IDLE_CHANNELS, &mut p);
        let chids: Vec<String> = hosts
            .iter()
            .map(|(c, handle)| format!("chid {c} (host {handle:#x})"))
            .collect();
        (
            st,
            format!(
                "NV0080 FIFO_IDLE_CHANNELS on the mirror's host device for {}: {} in {} us [host]",
                chids.join(", "),
                status_name(st),
                t0.elapsed().as_micros()
            ),
        )
    }

    /// `NV2080_CTRL_CMD_FIFO_DISABLE_CHANNELS` (ctrl2080fifo.h:235-261; export flags
    /// 0x10108: NON_PRIVILEGED) on the mirror's own host subdevice for the guest channels of
    /// `list` that have a mirror: `hClientList` is the mirror's own host client,
    /// `hChannelList` its own host channels, in batches of 64. `pRunlistPreemptEvent` is a
    /// kernel pointer and never passed (NULL); `bRewindGpPut` is never set (the host would
    /// rewrite the guest's USERD). `None` when no channel of the list has a mirror.
    pub fn set_scheduling(
        &self,
        reg: &Registry,
        list: &[(u32, u32)],
        disable: bool,
        only_scheduling: bool,
    ) -> Option<(u32, String)> {
        let hosts: Vec<(u32, u32)> = list
            .iter()
            .filter_map(|&(c, h)| self.host_channel_of(reg, c, h))
            .collect();
        if hosts.is_empty() {
            return None;
        }
        let h = self.hm.lock().unwrap();
        let mut worst = sys::NV_OK;
        let mut parts = Vec::new();
        for batch in hosts.chunks(DISABLE_MAX_ENTRIES) {
            let mut p = vec![0u8; DISABLE_PARAMS_SIZE];
            p[0] = disable as u8;
            p[4..8].copy_from_slice(&(batch.len() as u32).to_le_bytes());
            p[8] = only_scheduling as u8;
            for (i, (_, handle)) in batch.iter().enumerate() {
                let c = DISABLE_CLIENT_LIST + 4 * i;
                let o = DISABLE_CHANNEL_LIST + 4 * i;
                p[c..c + 4].copy_from_slice(&h.root().to_le_bytes());
                p[o..o + 4].copy_from_slice(&handle.to_le_bytes());
            }
            let st = h.control(h.subdevice(), CMD_FIFO_DISABLE_CHANNELS, &mut p);
            if st != sys::NV_OK {
                worst = st;
            }
            let chids: Vec<String> = batch.iter().map(|(c, _)| c.to_string()).collect();
            parts.push(format!("chids {}: {}", chids.join(","), status_name(st)));
        }
        Some((
            worst,
            format!(
                "NV2080 FIFO_DISABLE_CHANNELS bDisable {disable} bOnlyDisableScheduling {only_scheduling} on the mirror's own host channels ({}) [host]",
                parts.join("; ")
            ),
        ))
    }

    /// A subdevice control that names guest channel `(h_client, h_channel)` in its
    /// parameters (`hChannel` at `channel_off`, `hClient` at `client_off` when the struct
    /// has one), issued on the mirror's own host subdevice with the mirror's own host
    /// channel and client handles in their place; the guest's handles are put back in the
    /// answer. `None` when the channel has no mirror. Used for the getters of
    /// gsp-plan/c-guest-state.md §3 (B3, B4) on the device's own channel only.
    pub fn channel_query(
        &self,
        reg: &Registry,
        h_client: u32,
        h_channel: u32,
        cmd: u32,
        params: &[u8],
        (channel_off, client_off): (usize, Option<usize>),
    ) -> Option<(u32, Vec<u8>, String)> {
        let (chid, handle) = self.host_channel_of(reg, h_client, h_channel)?;
        let mut p = params.to_vec();
        let h = self.hm.lock().unwrap();
        p.get_mut(channel_off..channel_off + 4)?
            .copy_from_slice(&handle.to_le_bytes());
        if let Some(o) = client_off {
            p.get_mut(o..o + 4)?.copy_from_slice(&h.root().to_le_bytes());
        }
        let st = h.control(h.subdevice(), cmd, &mut p);
        p[channel_off..channel_off + 4].copy_from_slice(&h_channel.to_le_bytes());
        if let Some(o) = client_off {
            p[o..o + 4].copy_from_slice(&h_client.to_le_bytes());
        }
        Some((
            st,
            p,
            format!(
                "control {cmd:#x} on the mirror's host subdevice for chid {chid} (host channel {handle:#x}): {} [host]",
                status_name(st)
            ),
        ))
    }

    /// `NV2080_CTRL_CMD_GR_GET_CURRENT_RESIDENT_CHANNEL` (ctrl2080gr.h:1236-1253; export
    /// flags 0x48: NON_PRIVILEGED) on the mirror's own host subdevice: the host ChID
    /// resident on GR, mapped to the guest ChID of this device's mirror channel whose host
    /// work-submit token names it (VECTOR 11:0 is the host ChID,
    /// kernel_fifo_tu102.c:60-104). `(status, Some(guest chid))` only for one of this VM's
    /// mirrors; another tenant's ChID never leaves this function. A read of host
    /// scheduling state, reported in the note. `None` without a mirror channel.
    pub fn resident_guest_chid(&self, params: &[u8]) -> Option<(u32, Option<u32>, String)> {
        if self.channels.is_empty() {
            return None;
        }
        let mut p = params.to_vec();
        let st = {
            let h = self.hm.lock().unwrap();
            h.control(h.subdevice(), CMD_GR_GET_CURRENT_RESIDENT_CHANNEL, &mut p)
        };
        let host = u32::from_le_bytes(p.get(0..4)?.try_into().ok()?);
        let guest = (st == sys::NV_OK)
            .then(|| {
                self.channels
                    .iter()
                    .find(|(_, c)| c.token & 0xFFF == host & 0xFFF)
                    .map(|(chid, _)| *chid)
            })
            .flatten();
        Some((
            st,
            guest,
            format!(
                "GR_GET_CURRENT_RESIDENT_CHANNEL on the mirror's host subdevice: {}; {} [host read]",
                status_name(st),
                match guest {
                    Some(c) => format!("one of this VM's mirrors, guest chid {c}"),
                    None => "no mirror of this VM resident".into(),
                }
            ),
        ))
    }

    /// `NV906F_CTRL_CMD_GET_MMU_FAULT_INFO` (ctrl906f.h:175-220; export flags 0x10048:
    /// NON_PRIVILEGED) on the mirror's own host channel of guest chid `chid`: the fault the
    /// host RM recorded for that channel. The mirror runs in the guest's own VA space, so
    /// the address is the guest's VA (c-guest-state.md Q-C3). `None` without a mirror.
    pub fn mmu_fault_info(&self, chid: u32) -> Option<(u32, Vec<u8>)> {
        let hc = self.channels.get(&chid)?;
        let mut p = vec![0u8; MMU_FAULT_INFO_SIZE];
        let st = self.hm.lock().unwrap().control(hc.handle, CMD_GET_MMU_FAULT_INFO, &mut p);
        Some((st, p))
    }

    /// [`Self::mmu_fault_info`] of `chid`; when that holds no fault, of every other
    /// mirrored channel of its host TSG, the first that does. The host records an MMU
    /// fault on the TSG member that faulted and RCs the whole TSG, so the others read back
    /// an empty record (run of 2026-09-28: chids 15-22 RC'd, only chid 20's record held
    /// the fault). `None` without a mirror of `chid`.
    pub fn mmu_fault_info_in_tsg(&self, chid: u32) -> Option<TsgFault> {
        let own = self.mmu_fault_info(chid)?;
        let tsg = self.channels.get(&chid)?.tsg;
        let peers = self
            .channels
            .iter()
            .filter(|(c, h)| **c != chid && h.tsg == tsg)
            .map(|(c, _)| *c);
        Some(first_fault(chid, own, peers.filter_map(|c| {
            self.mmu_fault_info(c).map(|r| (c, r))
        })))
    }

    /// The fault of a shadow channel's MMU-fault RC against the guest's tables and the
    /// shadow (run P18): the page's guest leaf (cached walk) or gap, the shadow's mirrored
    /// leaf or gap, and the set-B and host-private ranges it hits. `p` is the
    /// `GET_MMU_FAULT_INFO` answer {addrHi @0, addrLo @4, faultType @8, faultString @12}.
    /// `None` for a channel without a shadow VAS.
    pub fn fault_diagnostic(&mut self, fb: &FakeFb, chid: u32, p: &[u8]) -> Option<String> {
        let (va, fault_type, what) = decode_fault_info(p);
        let root = self.channels.get(&chid)?.shadow_root?;
        let page = va & !(PAGE - 1);
        let guest = guest_span_coverage(fb, root, page, page + PAGE);
        let host = self.worker.coverage(root, page, page + PAGE);
        let sp = self.shadows.get(&root)?;
        let hit = |r: &[(u64, u64)]| r.iter().any(|(lo, hi)| page < *hi && page + PAGE > *lo);
        let set_b = hit(&sp.exclude) || sp.held.values().any(|r| hit(r));
        let slot_window = sp.front.layout.slot_window.is_some_and(|w| hit(&[w]));
        Some(format!(
            "shadow fault diagnostic: chid {chid} fault {va:#x} type {fault_type:#x} ({what}) on shadow root {root:#x}: guest {guest}; shadow {host}; set B {set_b}; GFXP slot window {slot_window}{} [host read]",
            sp.front.layout.unmirrorable(page, page + PAGE).map_or(String::new(), |why| format!("; unmirrorable: {why}"))
        ))
    }

    /// Detach and release an owned GFXP set while the exact host TSG and channel still
    /// exist. Every failure retains the remaining objects and all private VA exclusions.
    fn free_owned_gfxp(&mut self, key: TsgKey, notes: &mut Vec<String>) -> bool {
        let Some(mut owned) = self.tsgs.get_mut(&key).and_then(|t| t.gfxp.take()) else {
            return true;
        };
        let host_tsg = self.tsgs.get(&key).map(|t| t.handle);
        // The TSG's host channels, idled before the BIND to WFI (the context must not run).
        let hosted: Vec<(u32, u32)> = self
            .channels
            .iter()
            .filter(|(_, hc)| hc.tsg == key)
            .map(|(c, hc)| (*c, hc.handle))
            .collect();
        if !hosted.is_empty() {
            let flags = ((sys::NVOS30_FLAGS_IDLE_PUSH_BUFFER
                | sys::NVOS30_FLAGS_IDLE_CACHE1
                | sys::NVOS30_FLAGS_IDLE_ALL_ENGINES)
                << 8)
                | sys::NVOS30_FLAGS_BEHAVIOR_SLEEP;
            let (status, note) = self.idle_host_channels(&hosted, flags, 100_000);
            notes.push(note);
            if status != sys::NV_OK {
                self.tsgs.get_mut(&key).unwrap().gfxp = Some(owned);
                return false;
            }
        }
        if owned.bind_attempted {
            let host = self.hm.lock().unwrap();
            let bind = host_tsg.map_or(sys::NV_OK, |t| bind_gfxp(&host, t, 0, &[0; 5]));
            let (get, mode) = hosted
                .first()
                .map_or((sys::NV_OK, 0), |(_, hc)| get_gfxp_mode(&host, *hc));
            if bind != sys::NV_OK || get != sys::NV_OK || mode != 0 {
                notes.push(format!("tsg {key}: ERROR GFXP BIND WFI/GET: bind {}, get {}, mode {mode:#x}; retained",
                    status_name(bind), status_name(get)));
                drop(host);
                self.tsgs.get_mut(&key).unwrap().gfxp = Some(owned);
                return false;
            }
            owned.bound = false;
        }
        self.worker.wait_all();
        self.worker.wait_idle();
        let mut host = self.hm.lock().unwrap();
        if let Err(e) = release_gfxp(&mut host, self.slots.as_mut(), self.reservation, &mut owned) {
            notes.push(format!("tsg {key}: ERROR {e}; retained"));
            drop(host);
            self.tsgs.get_mut(&key).unwrap().gfxp = Some(owned);
            return false;
        }
        drop(host);
        notes.push(format!(
            "tsg {key}: owned GFXP back to WFI, slot {} freed [host]",
            owned.slot
        ));
        true
    }

    fn free_channel(&mut self, chid: u32, notes: &mut Vec<String>) -> bool {
        // A build in flight is waited for and freed as a built channel (execution.md §9.2).
        self.settle_build(chid, notes);
        // Step 0 at teardown (preempt.rs): the live modes, before an owned GFXP set unbinds.
        self.preempt_teardown_read(chid, notes);
        // The owned-GFXP set belongs to the TSG's context: it goes with the TSG's last host
        // channel (or its 3D object, `free_host_objects`).
        if let Some(key) = self.channels.get(&chid).map(|hc| hc.tsg) {
            let last = self.channels.values().filter(|hc| hc.tsg == key).count() == 1;
            if last && !self.free_owned_gfxp(key, notes) {
                return false;
            }
        }
        self.exhausted.remove(&chid);
        self.exhausted_raised.remove(&chid);
        if self.held_rings.remove(&chid).is_some() {
            self.ring_stats.dropped += 1;
            self.update_waiting();
        }
        if let Some(mut hc) = self.channels.remove(&chid) {
            // Out of the poller's list before the USERD goes (the cell is emptied below).
            self.publish_userd_watch();
            let r = {
                let mut h = self.hm.lock().unwrap();
                let r = h.free_object(hc.handle);
                // The guest pages the channel used, after the channel (it references them);
                // kept when the channel free failed (fail closed, hostmem.rs `run_teardown`).
                for d in hc.pages.iter().filter(|_| r.is_ok()) {
                    if let Err(st) = h.free_object(*d) {
                        notes.push(format!(
                            "chid {chid}: ERROR freeing the host object {d:#x} over a guest page: {}",
                            status_name(st)
                        ));
                    }
                }
                if r.is_ok() {
                    // The cell is emptied first: the poller no longer reads the window.
                    if let Some(u) = hc.userd.take().and_then(|s| s.take()) {
                        if let Err(st) = h.free_host_userd(u.storage) {
                            notes.push(format!("chid {chid}: ERROR freeing private USERD: {}", status_name(st)));
                        }
                    }
                }
                r
            };
            if r.is_ok() {
                self.stats.channels_freed += 1;
            }
            notes.push(match r {
                Ok(()) => format!(
                    "chid {chid}: host channel {:#x} freed after {} doorbell(s), {} ms of life [host]",
                    hc.handle,
                    hc.doorbells,
                    hc.created.elapsed().as_millis()
                ),
                Err(st) => format!("chid {chid}: ERROR freeing host channel {:#x}: {}", hc.handle, status_name(st)),
            });
            if r.is_err() {
                hc.set_context_ready(false);
                self.channels.insert(chid, hc);
                self.publish_userd_watch();
                return false;
            }
        }
        for sp in self.shadows.values_mut() {
            sp.chids.remove(&chid);
        }
        true
    }

    fn free_tsg(&mut self, key: TsgKey, notes: &mut Vec<String>) -> bool {
        let building: Vec<u32> = self
            .builds
            .iter()
            .filter(|(_, b)| b.tsg_key == key)
            .map(|(c, _)| *c)
            .collect();
        for c in building {
            self.settle_build(c, notes);
        }
        let chids: Vec<u32> = self
            .channels
            .iter()
            .filter(|(_, c)| c.tsg == key)
            .map(|(c, _)| *c)
            .collect();
        let mut freed = true;
        for c in chids {
            freed &= self.free_channel(c, notes);
        }
        if !freed {
            return false;
        }
        if let Some(t) = self.tsgs.remove(&key) {
            let r = self.hm.lock().unwrap().free_object(t.handle);
            notes.push(match r {
                Ok(()) => format!(
                    "tsg {:#x}/{:#x} ({key}): host TSG {:#x} (engineType {:#x}) freed [host]",
                    t.guest.0, t.guest.1, t.handle, t.engine
                ),
                Err(st) => format!(
                    "tsg {:#x}/{:#x} ({key}): ERROR freeing host TSG {:#x}: {}",
                    t.guest.0,
                    t.guest.1,
                    t.handle,
                    status_name(st)
                ),
            });
            // The host's GR context at the held set-B VAs is gone with its TSG: the ranges
            // go back to set A and their roots are re-synced ([`Mirror::take_released_roots`]).
            // A refused free keeps them (fail closed: the host context may still be there).
            if r.is_ok() {
                self.release_held(key, notes);
            } else {
                self.tsgs.insert(key, t);
                return false;
            }
        } else {
            // No host TSG under the key (never built, or freed before): nothing owns the VAs.
            self.release_held(key, notes);
        }
        for sp in self.shadows.values_mut() {
            sp.tsgs.remove(&key);
        }
        self.reap_shadows(None, notes);
        true
    }

    fn drop_vas(&mut self, key: (u32, u32), notes: &mut Vec<String>) {
        let building: Vec<u32> = self
            .builds
            .iter()
            .filter(|(_, b)| b.vas_key == key)
            .map(|(c, _)| *c)
            .collect();
        for c in building {
            self.settle_build(c, notes);
        }
        let tsgs: Vec<TsgKey> = self
            .tsgs
            .iter()
            .filter(|(_, t)| t.vas == key)
            .map(|(k, _)| *k)
            .collect();
        let mut dependent_failed = false;
        for t in tsgs {
            dependent_failed |= !self.free_tsg(t, notes);
        }
        if dependent_failed || self.tsgs.values().any(|t| t.vas == key) {
            notes.push(format!("vaspace {:#x}/{:#x}: retained because a dependent host TSG remains", key.0, key.1));
            return;
        }
        if let Some(hv) = self.vas.remove(&key) {
            let r = self.hm.lock().unwrap().free_object(hv.handle);
            notes.push(match r {
                Ok(()) => format!(
                    "vaspace {:#x}/{:#x}: host VA space {:#x} freed [host]",
                    key.0, key.1, hv.handle
                ),
                Err(st) => format!(
                    "vaspace {:#x}/{:#x}: ERROR freeing host VA space {:#x}: {}",
                    key.0,
                    key.1,
                    hv.handle,
                    status_name(st)
                ),
            });
            if r.is_err() {
                self.vas.insert(key, hv);
            }
        }
    }

    /// The guest's `DMA_SET_PAGE_DIRECTORY` (`params` as sent) or `UNSET` (`None`) on
    /// `key` (resolved). A replayed mirror gets the same control on the host with the
    /// guest's root (PRIVILEGED: one CAP_SYS_ADMIN window), as GSP would; an
    /// externally-owned mirror rooted elsewhere is rebuilt at the next doorbell.
    pub fn on_page_directory(
        &mut self,
        key: (u32, u32),
        root: Option<u64>,
        params: Option<&[u8]>,
    ) -> Vec<String> {
        if let Some(why) = self.raw_refusal() {
            return vec![format!("vaspace {:#x}/{:#x}: raw page-directory forwarding refused for {why}", key.0, key.1)];
        }
        let mut notes = Vec::new();
        let Some(hv) = self.vas.get(&key) else {
            return notes;
        };
        if !hv.replayed {
            if !matches!(root, Some(r) if r == hv.root) {
                self.drop_vas(key, &mut notes);
            }
            return notes;
        }
        let handle = hv.handle;
        let h = self.hm.lock().unwrap();
        let device = h.device();
        match (root, params) {
            (Some(r), Some(p)) => {
                if r < self.base || r + PAGE > self.end {
                    notes.push(format!(
                        "vaspace {:#x}/{:#x}: root {r:#x} outside the reservation; not forwarded",
                        key.0, key.1
                    ));
                    return notes;
                }
                let mut sp = p.to_vec();
                sp.resize(SETPD_SIZE, 0);
                sp[16..20].copy_from_slice(&handle.to_le_bytes());
                sp[24..28].copy_from_slice(&1u32.to_le_bytes());
                let st = h
                    .control_admin(device, v::CMD_DMA_SET_PAGE_DIRECTORY, &mut sp)
                    .unwrap_or(0xFFFF_FFFF);
                notes.push(format!(
                    "vaspace {:#x}/{:#x}: host DMA_SET_PAGE_DIRECTORY {r:#x} (guest's flags {:#x}) on mirror {handle:#x}: {} [host]",
                    key.0,
                    key.1,
                    u32::from_le_bytes(p[12..16].try_into().unwrap_or([0; 4])),
                    status_name(st)
                ));
                drop(h);
                if st == sys::NV_OK {
                    if let Some(hv) = self.vas.get_mut(&key) {
                        hv.root = r;
                    }
                }
            }
            _ => {
                let mut up = [0u8; 8];
                up[0..4].copy_from_slice(&handle.to_le_bytes());
                up[4..8].copy_from_slice(&1u32.to_le_bytes());
                let st = h
                    .control_admin(device, v::CMD_DMA_UNSET_PAGE_DIRECTORY, &mut up)
                    .unwrap_or(0xFFFF_FFFF);
                notes.push(format!(
                    "vaspace {:#x}/{:#x}: host DMA_UNSET_PAGE_DIRECTORY on mirror {handle:#x}: {} [host]",
                    key.0,
                    key.1,
                    status_name(st)
                ));
            }
        }
        notes
    }

    /// The key of the host TSG that stands for guest object `obj` (a guest TSG, or a bare
    /// channel with its own host TSG). A FREE names the guest's handle; a duplicate handle
    /// of the TSG is not the TSG and frees nothing.
    fn tsg_standing_for(&self, obj: (u32, u32)) -> Option<TsgKey> {
        self.tsgs
            .iter()
            .find(|(_, t)| t.guest == obj)
            .map(|(k, _)| *k)
    }

    /// Release the set-B ranges held for host TSG `key` on every shadow root.
    fn release_held(&mut self, key: TsgKey, notes: &mut Vec<String>) {
        for (root, sp) in self.shadows.iter_mut() {
            if let Some(ranges) = sp.release(key) {
                notes.push(format!(
                    "shadow: root {root:#x}: {} held set-B range(s) of host TSG ({key}) back to set A after its host free [host]",
                    ranges.len()
                ));
                self.released_roots.push(*root);
            }
        }
    }

    /// The guest freed `h_object` of `h_client` (or the client itself): free the host
    /// mirrors that stood for it, channels before TSGs before VA spaces. The set-B ranges
    /// of a freed guest channel whose host TSG outlives this free stay out of set A until
    /// that TSG's host free ([`held_at_free`]).
    pub fn free_object(&mut self, reg: &Registry, h_client: u32, h_object: u32) -> Vec<String> {
        let candidates = held_at_free(
            reg,
            |c| c.h_client == h_client && (h_client == h_object || c.h_object == h_object),
            |c| match reg.vas_key_of(c).and_then(|k| reg.vas_root(k)) {
                Some(Target::Fb(r)) if self.shadows.contains_key(&r) => Some(r),
                _ => None,
            },
            |root, id, va| self.global_extent(None, root, id, va),
        );
        let mut notes = self.free_host_objects(reg, h_client, h_object);
        for h in candidates {
            if !self.tsgs.contains_key(&h.key) {
                continue;
            }
            let Some(sp) = self.shadows.get_mut(&h.root) else {
                continue;
            };
            notes.push(format!(
                "shadow: chid {}: {} set-B range(s) of the freed guest channel held out of set A on root {:#x} until host TSG ({}) is freed [guest]",
                h.chid,
                h.ranges.len(),
                h.root,
                h.key
            ));
            sp.hold(h.key, h.ranges);
        }
        notes
    }

    /// [`Mirror::free_object`] without the set-B holds.
    fn free_host_objects(&mut self, reg: &Registry, h_client: u32, h_object: u32) -> Vec<String> {
        let mut notes = Vec::new();
        if let Some((chid, generation, class, _)) = reg.engine_object(h_client, h_object) {
            if self.video_pending.get(&chid).is_some_and(|(handle, _, _)| *handle == h_object) {
                self.video_pending.remove(&chid);
                notes.push(format!("chid {chid}: pending engine {h_object:#x} dropped [guest]"));
            }
            if let Some(key) = self.channels.get(&chid)
                .filter(|hc| hc.generation == generation)
                .map(|hc| hc.tsg)
            {
                // The 3D object's GFXP set goes before it (NVIDIA's vGPU BINDs WFI before it
                // frees the buffers, rpc.c:3051-3110 of 615).
                if crate::board::host_class_of(class) == HOST_3D_CLASS
                    && self.tsgs.get(&key).is_some_and(|t| t.gfxp.is_some())
                    && !self.free_owned_gfxp(key, &mut notes)
                {
                    notes.push(format!("chid {chid}: engine {h_object:#x} retained because owned GFXP cleanup failed"));
                    return notes;
                }
            }
            if let Some(hc) = self.channels.get_mut(&chid).filter(|hc| hc.generation == generation) {
                if let Some(host) = hc.engine_map.get(&h_object).copied() {
                    match self.hm.lock().unwrap().free_object(host) {
                        Ok(()) => {
                            hc.engine_map.remove(&h_object);
                            hc.objects.retain(|(_, handle)| *handle != host);
                            notes.push(format!("chid {chid}: engine {h_object:#x} host object {host:#x} freed [host]"));
                        }
                        Err(st) => notes.push(format!("chid {chid}: ERROR freeing engine {h_object:#x} host object {host:#x}: {}", status_name(st))),
                    }
                }
            }
            return notes;
        }
        // A deferred video object of a channel that goes away is never placed.
        let gone: Vec<u32> = reg
            .channels()
            .filter(|c| c.h_client == h_client && (h_client == h_object || c.h_object == h_object))
            .map(|c| c.chid)
            .filter(|c| self.video_pending.contains_key(c))
            .collect();
        for c in gone {
            self.video_pending.remove(&c);
            notes.push(format!("chid {c}: deferred video object dropped with its channel [guest]"));
        }
        if h_client == h_object {
            let keys: Vec<(u32, u32)> = self
                .vas
                .keys()
                .filter(|k| k.0 == h_client)
                .copied()
                .collect();
            for k in keys {
                self.drop_vas(k, &mut notes);
            }
            let tsgs: Vec<TsgKey> = self
                .tsgs
                .iter()
                .filter(|(_, t)| t.guest.0 == h_client)
                .map(|(k, _)| *k)
                .collect();
            for k in tsgs {
                self.free_tsg(k, &mut notes);
            }
            let chids: Vec<u32> = reg
                .channels()
                .filter(|c| c.h_client == h_client)
                .map(|c| c.chid)
                .collect();
            for c in chids {
                self.free_channel(c, &mut notes);
            }
            return notes;
        }
        if let Some(c) = reg
            .channels()
            .find(|c| c.h_client == h_client && c.h_object == h_object)
        {
            let chid = c.chid;
            self.free_channel(chid, &mut notes);
            // A bare CE channel's own host TSG goes with it.
            if let Some(k) = self.tsg_standing_for((h_client, h_object)) {
                self.free_tsg(k, &mut notes);
            }
            return notes;
        }
        if let Some(k) = self.tsg_standing_for((h_client, h_object)) {
            self.free_tsg(k, &mut notes);
            return notes;
        }
        let key = reg.resolve((h_client, h_object));
        if self.vas.contains_key(&key) && key == (h_client, h_object) {
            self.drop_vas(key, &mut notes);
        }
        notes
    }

    /// Free every mirror object (channels, TSGs, VA spaces, the doorbell page, the
    /// events) and stop the event thread. Idempotent; the mirror stays usable.
    pub fn reset(&mut self) -> Vec<String> {
        let mut notes = Vec::new();
        let building: Vec<u32> = self.builds.keys().copied().collect();
        for c in building {
            self.settle_build(c, &mut notes);
        }
        let chids: Vec<u32> = self.channels.keys().copied().collect();
        for c in chids {
            self.free_channel(c, &mut notes);
        }
        let tsgs: Vec<TsgKey> = self.tsgs.keys().copied().collect();
        for t in tsgs {
            self.free_tsg(t, &mut notes);
        }
        let vas: Vec<(u32, u32)> = self.vas.keys().copied().collect();
        for k in vas {
            self.drop_vas(k, &mut notes);
        }
        self.reap_shadows(None, &mut notes);
        // Every teardown queued above finishes before the descriptors can be freed.
        self.worker.wait_idle();
        notes.extend(self.worker.take_notes());
        if !self.channels.is_empty() || !self.tsgs.is_empty() || !self.vas.is_empty()
            || !self.shadows.is_empty()
        {
            notes.push("mirror reset retained shared infrastructure because host objects remain after a teardown refusal".into());
            return notes;
        }
        self.inv_gate.clear(&self.worker.progress);
        self.ring_stats.dropped += self.held_rings.len() as u64;
        self.held_rings.clear();
        self.tsg_cells.clear();
        self.committed.clear();
        self.ring_gate.clear_holds();
        self.update_waiting();
        if let Some(inv) = self.invalidator.take() {
            let mut h = self.hm.lock().unwrap();
            notes.push(inv.free(&mut h));
        }
        if let Some(ce) = self.device_ce.take() {
            let mut h = self.hm.lock().unwrap();
            notes.push(ce.lock().unwrap_or_else(|e| e.into_inner()).free(&mut h));
        }
        if let Some((h, map)) = self.doorbell.take() {
            drop(map);
            let r = self.hm.lock().unwrap().free_object(h);
            notes.push(format!(
                "host doorbell object {h:#x} freed: {:?}",
                r.map_err(status_name)
            ));
        }
        if let Some(mut ev) = self.events.take() {
            ev.stop.store(true, Ordering::SeqCst);
            if let Some(t) = ev.thread.take() {
                let _ = t.join();
            }
            let mut h = self.hm.lock().unwrap();
            for e in ev.handles {
                let _ = h.free_object(e);
            }
            notes.push("host OS events freed, event thread stopped".into());
        }
        notes
    }

    pub fn summary(&self) -> String {
        let s = &self.stats;
        format!(
            "mirror: {} channel(s) created, {} freed, {} live; {} doorbell(s) forwarded, {} TLB invalidate(s) ({} MEM_OP push(es), {} slow), {} refusal(s), longest mirror build {} us; events raised {}, MSIs {} (coalesced {}), host RC events {}{}; {}; {}",
            s.channels_created,
            s.channels_freed,
            self.channels.len(),
            s.doorbells,
            s.invalidates,
            self.invalidator.as_ref().map_or(0, |i| i.pushes),
            self.invalidator.as_ref().map_or(0, |i| i.slow),
            s.refusals,
            s.max_create_us,
            self.intr.raised.load(Ordering::Relaxed),
            self.intr.msis.load(Ordering::Relaxed),
            self.intr.coalesced.load(Ordering::Relaxed),
            self.intr.rc_events.load(Ordering::Relaxed),
            if self.lazy.enabled() {
                format!("; {}", self.lazy.summary())
            } else {
                String::new()
            },
            self.hold_summary(),
            self.extents.summary()
        ) + &self
            .device_ce
            .as_ref()
            .map_or(String::new(), |c| format!("; {}", c.lock().unwrap_or_else(|e| e.into_inner()).summary()))
            + &self.slots.as_ref().map_or("; gfxp slots: none (every host GR context in graphics WFI)".to_string(), |sl| format!(
                "; gfxp slots: {} of {} in use, {} TSG(s) bound GFXP at build, {} left in WFI",
                sl.in_use(), sl.count(), self.gfxp_bound, self.gfxp_no_slot
            ))
    }
}

/// Work for the shadow worker, applied in submission order.
enum Job {
    /// A fresh shadow VAS of a guest root.
    Create(u64, Box<crate::shadow::ShadowVas>),
    /// A plan of the server thread, and when it was submitted.
    Apply(crate::shadow::Plan, Instant),
    /// Tear the shadow VAS of a root down (its channels and TSGs are already freed).
    Free(u64),
    /// Return exact post-apply mapping coverage to a diagnostic caller.
    Coverage(u64, u64, u64, std::sync::mpsc::Sender<String>),
    /// A host RM call in order with root's plans, then the release of a ring hold
    /// (ringgate.rs `RingGate::hold`).
    Call(u64, HostCall, Option<u64>),
}

/// A host RM call the worker runs ([`Job::Call`]): its trace lines.
pub type HostCall = Box<dyn FnOnce(&Mutex<HostMemory>) -> Vec<String> + Send>;

impl Job {
    /// The shadow root the job belongs to.
    fn root(&self) -> u64 {
        match self {
            Job::Create(r, _) | Job::Free(r) | Job::Coverage(r, _, _, _) | Job::Call(r, _, _) => *r,
            Job::Apply(plan, _) => plan.root,
        }
    }
}

/// The worker's queues: jobs per shadow root in submission order.
#[derive(Default)]
struct JobQueues {
    roots: BTreeMap<u64, std::collections::VecDeque<(u64, Job)>>,
    len: usize,
}

impl JobQueues {
    fn push(&mut self, seq: u64, job: Job) {
        self.roots.entry(job.root()).or_default().push_back((seq, job));
        self.len += 1;
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The next job: the oldest job of a root in `urgent` that has one queued, else the
    /// oldest job of all. Jobs of one root never pass each other.
    fn pop(&mut self, urgent: &[u64], stats: &WorkerStats) -> Option<(u64, Job)> {
        let front = |r: &u64, q: &std::collections::VecDeque<(u64, Job)>| q.front().map(|(s, _)| (*s, *r));
        let oldest = self.roots.iter().filter_map(|(r, q)| front(r, q)).min()?;
        let pick = urgent
            .iter()
            .filter_map(|r| self.roots.get(r).and_then(|q| front(r, q)))
            .min()
            .unwrap_or(oldest);
        if pick != oldest {
            stats.promoted.fetch_add(1, Ordering::Relaxed);
        }
        let q = self.roots.get_mut(&pick.1).expect("picked");
        let job = q.pop_front();
        if q.is_empty() {
            self.roots.remove(&pick.1);
        }
        self.len -= 1;
        job
    }
}

/// Adds the time since its start to the worker's busy time when dropped.
struct BusyTimer<'a>(&'a WorkerStats, Instant);

impl Drop for BusyTimer<'_> {
    fn drop(&mut self) {
        self.0
            .busy_us
            .fetch_add(self.1.elapsed().as_micros() as u64, Ordering::Relaxed);
    }
}

/// The worker's counters for the exit summary: time applying plans, the time plans
/// waited in the queue, and the jobs of a waited-for root applied before older ones.
#[derive(Default)]
struct WorkerStats {
    busy_us: std::sync::atomic::AtomicU64,
    queued_us: std::sync::atomic::AtomicU64,
    queued_max_us: std::sync::atomic::AtomicU64,
    applies: std::sync::atomic::AtomicU64,
    promoted: std::sync::atomic::AtomicU64,
}

/// What a shadow job in flight may still map on the host beyond the cached walks
/// (docs/caraxes/bar1-pool.md §14): the host keeps it until the job is applied.
enum Ghost {
    /// FB ranges `[start, end)`, sorted and merged: the leaves a plan's walk update took out
    /// of the walk, or every leaf of a root being torn down.
    Ranges(Vec<(u64, u64)>),
    /// A teardown of a root whose walk was not at hand: any page.
    All,
}

impl Ghost {
    fn covers_any(&self, pages: &BTreeSet<u64>) -> bool {
        match self {
            Ghost::All => !pages.is_empty(),
            Ghost::Ranges(r) => pages.iter().any(|p| {
                let i = r.partition_point(|(s, _)| *s <= *p);
                i > 0 && r[i - 1].1 > *p
            }),
        }
    }
}

/// The shadow worker (docs/caraxes/mapping.md §4): one thread that owns the host side of
/// every shadow VA space and applies the server thread's plans, so a guest MMU invalidate
/// returns after the walk (tens of microseconds) instead of after the RM calls, and a
/// context's teardown runs in the background in bounded batches. Ordering: every job gets
/// a sequence number; a doorbell of a shadow channel waits until the worker finished every
/// job submitted before it (`wait_all`), so no host channel runs before its root's mirror is
/// consistent with the guest's last invalidate. `CARAXES_SHADOW_WORKER=0` waits after every
/// submission (the synchronous reference).
struct ShadowWorker {
    tx: Option<std::sync::mpsc::Sender<(u64, Job)>>,
    stats: Arc<WorkerStats>,
    /// The last job finished (invgate.rs), shared with the worker thread.
    progress: Arc<crate::invgate::Progress>,
    /// Submitted jobs and the last one per shadow root.
    seqs: crate::invgate::Seqs,
    enabled: bool,
    notes: Arc<Mutex<Vec<String>>>,
    /// No teardown in progress.
    idle: Arc<(Mutex<bool>, std::sync::Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// Jobs not yet applied, ascending, with their root and what they may still map beyond
    /// the cached walks ([`Ghost`]); the ones below the oldest job in flight are dropped at
    /// the next submit (a root's jobs may finish before older jobs of other roots).
    ghosts: std::collections::VecDeque<(u64, u64, Ghost)>,
}

impl ShadowWorker {
    fn new(hm: Arc<Mutex<HostMemory>>, intr: Arc<IntrShared>, gate: Arc<crate::ringgate::RingGate>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<(u64, Job)>();
        let progress = Arc::new(crate::invgate::Progress::default());
        let notes = Arc::new(Mutex::new(Vec::new()));
        let (progress2, notes2) = (progress.clone(), notes.clone());
        let stats = Arc::new(WorkerStats::default());
        let worker_stats2 = stats.clone();
        let gate2 = gate;
        let idle = Arc::new((Mutex::new(true), std::sync::Condvar::new()));
        let idle2 = idle.clone();
        let thread = std::thread::Builder::new()
            .name("shadow-worker".into())
            .spawn(move || {
                let mut spaces: BTreeMap<u64, crate::shadow::ShadowVas> = BTreeMap::new();
                // Shadow VASes being torn down, a bounded step at a time whenever no job
                // waits: a teardown never delays a plan (the doorbell barrier waits only
                // for plans), and never holds the host RM lock for more than one step.
                let mut graveyard: std::collections::VecDeque<(u64, crate::shadow::ShadowVas, Instant, usize, String)> =
                    std::collections::VecDeque::new();
                // Jobs received and not yet applied, per shadow root in submission order: a
                // root's jobs stay in order, and a root someone waits for (a doorbell
                // barrier, a held invalidate) goes before the others (`pick_job`).
                let mut queues = JobQueues::default();
                loop {
                    if queues.is_empty() {
                        let first = if graveyard.is_empty() {
                            match rx.recv() {
                                Ok(j) => Some(j),
                                Err(_) => break,
                            }
                        } else {
                            match rx.recv_timeout(crate::shadow::TEARDOWN_PAUSE) {
                                Ok(j) => Some(j),
                                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                    // Shutting down: finish every teardown.
                                    while let Some((root, mut sv, t0, mut steps, summary)) = graveyard.pop_front() {
                                        while !sv.teardown_step(&hm) {
                                            steps += 1;
                                        }
                                        intr.note(format!("shadow: VAS of guest root {root:#x} freed: {summary}; {} [host]", sv.teardown_line(steps + 1, t0.elapsed().as_millis())));
                                    }
                                    break;
                                }
                            }
                        };
                        if let Some((seq, job)) = first {
                            queues.push(seq, job);
                        }
                    }
                    while let Ok((seq, job)) = rx.try_recv() {
                        queues.push(seq, job);
                    }
                    let next = queues.pop(&progress2.urgent_roots(), &worker_stats2);
                    let Some((seq, job)) = next else {
                        if let Some(g) = graveyard.front_mut() {
                            g.3 += 1;
                            if g.1.teardown_step(&hm) {
                                let (root, sv, t0, steps, summary) = graveyard.pop_front().expect("front");
                                let line = format!(
                                    "shadow: VAS of guest root {root:#x} freed: {summary}; {} [host]",
                                    sv.teardown_line(steps, t0.elapsed().as_millis())
                                );
                                notes2.lock().unwrap().push(line.clone());
                                intr.note(line);
                            }
                        }
                        if graveyard.is_empty() {
                            let (m, cv) = &*idle2;
                            *m.lock().unwrap() = true;
                            cv.notify_all();
                        }
                        continue;
                    };
                    let lines: Vec<String> = match job {
                        Job::Create(root, sv) => {
                            spaces.insert(root, *sv);
                            Vec::new()
                        }
                        Job::Apply(mut plan, queued) => {
                            plan.queued_us = queued.elapsed().as_micros();
                            let q = plan.queued_us as u64;
                            worker_stats2.queued_us.fetch_add(q, Ordering::Relaxed);
                            worker_stats2.queued_max_us.fetch_max(q, Ordering::Relaxed);
                            worker_stats2.applies.fetch_add(1, Ordering::Relaxed);
                            crate::profile::record(
                                crate::profile::Key::Window("shadow worker: plan queued before apply"),
                                std::time::Duration::from_micros(q),
                                1,
                                None,
                                "",
                            );
                            let busy0 = Instant::now();
                            let _busy = BusyTimer(&worker_stats2, busy0);
                            let descs: Vec<(u32, u64, u64)> = hm
                                .lock()
                                .unwrap()
                                .descriptors()
                                .iter()
                                .map(|d| (d.handle, d.range.iova, d.range.size))
                                .collect();
                            match spaces.get_mut(&plan.root) {
                                Some(sv) => {
                                    let (changed, n) = sv.apply(&hm, &plan, &descs);
                                    if changed { n } else { Vec::new() }
                                }
                                None => vec![format!("shadow: plan for unknown root {:#x} dropped", plan.root)],
                            }
                        }
                        Job::Free(root) => {
                            if let Some(sv) = spaces.remove(&root) {
                                let summary = sv.summary();
                                {
                                    let (m, _) = &*idle2;
                                    *m.lock().unwrap() = false;
                                }
                                graveyard.push_back((root, sv, Instant::now(), 0, summary));
                            }
                            Vec::new()
                        }
                        Job::Call(_, call, release) => {
                            // A host RM call in order with the root's plans (the pool's
                            // preempt, ringgate.rs); then the rings it held may go.
                            let lines = call(&hm);
                            if let Some(t) = release {
                                gate2.release(t);
                            }
                            lines
                        }
                        Job::Coverage(root, start, end, reply) => {
                            let answer = spaces.get(&root).map_or_else(
                                || format!("unknown shadow root {root:#x}"),
                                |sv| sv.coverage(start, end),
                            );
                            let _ = reply.send(answer);
                            Vec::new()
                        }
                    };
                    for l in lines {
                        intr.note(l);
                    }
                    progress2.finish(seq);
                    // A held ring of this root may be ready now (ringgate.rs).
                    gate2.poke();
                }
            })
            .ok();
        ShadowWorker {
            tx: Some(tx),
            stats,
            progress,
            idle,
            seqs: Default::default(),
            enabled: std::env::var("CARAXES_SHADOW_WORKER").as_deref() != Ok("0"),
            notes,
            thread,
            ghosts: Default::default(),
        }
    }

    fn submit(&mut self, job: Job) {
        self.submit_with(job, None);
    }

    /// Submit `job`; for a teardown, `leaves` are the FB ranges its root maps (`None`: not
    /// known, any page). What the job may still map beyond the cached walks is kept until
    /// it is applied ([`Self::in_flight`]).
    fn submit_with(&mut self, mut job: Job, leaves: Option<Vec<(u64, u64)>>) {
        let root = job.root();
        if self.thread.is_some() {
            let done = self.progress.done();
            self.seqs.forget_done(done);
            while self.ghosts.front().is_some_and(|(s, _, _)| *s <= done) {
                self.ghosts.pop_front();
            }
        }
        let seq = self.seqs.next(root);
        if self.thread.is_some() {
            self.progress.begin(seq, root);
        }
        let ghost = match &mut job {
            Job::Apply(plan, _) if !plan.ghosts.is_empty() => {
                Some(Ghost::Ranges(std::mem::take(&mut plan.ghosts)))
            }
            Job::Free(_) => Some(match leaves {
                Some(r) => Ghost::Ranges(crate::ptcache::merge_ranges(r)),
                None => Ghost::All,
            }),
            _ => None,
        };
        if let (Some(g), true) = (ghost, self.thread.is_some()) {
            self.ghosts.push_back((seq, root, g));
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send((seq, job));
        }
        if !self.enabled {
            self.wait_all();
        }
    }

    /// The newest job not yet applied that may still map a page of `pages` (4 KiB-aligned
    /// FB addresses) on the host beyond the cached walks; `None` without one.
    fn in_flight(&self, pages: &BTreeSet<u64>) -> Option<u64> {
        if self.thread.is_none() || pages.is_empty() {
            return None;
        }
        let done = self.progress.done();
        self.ghosts
            .iter()
            .rev()
            .take_while(|(s, _, _)| *s > done)
            .filter(|(_, _, g)| g.covers_any(pages))
            .find(|(s, r, _)| !self.progress.is_done(&crate::invgate::Target::Roots(vec![(*r, *s)])))
            .map(|(s, _, _)| *s)
    }

    /// Wait until job `seq` is applied, at most `max` (`None`: to the end); whether it is.
    /// Only that job's root is waited for (its jobs finish in order), not older jobs of
    /// other roots.
    fn wait_applied(&self, seq: u64, max: Option<std::time::Duration>) -> bool {
        if self.thread.is_none() {
            return true;
        }
        let t = match self.ghosts.iter().find(|(s, _, _)| *s == seq) {
            Some((s, r, _)) => crate::invgate::Target::Roots(vec![(*r, *s)]),
            None => crate::invgate::Target::All(seq),
        };
        match max {
            None => {
                self.progress.wait(&t);
                true
            }
            Some(m) => self.progress.wait_at_most(&t, m),
        }
    }

    fn coverage(&mut self, root: u64, start: u64, end: u64) -> String {
        if self.thread.is_none() {
            return "shadow worker unavailable".into();
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.submit(Job::Coverage(root, start, end, tx));
        rx.recv().unwrap_or_else(|_| "shadow worker unavailable".into())
    }

    /// Jobs submitted and not yet done.
    fn pending(&self) -> bool {
        self.thread.is_some() && !self.progress.idle()
    }

    /// Jobs of shadow root `root` submitted and not yet done.
    fn pending_root(&self, root: u64) -> bool {
        self.thread.is_some()
            && !self
                .progress
                .is_done(&crate::invgate::Target::Roots(vec![(root, self.seqs.last_of(root))]))
    }

    /// The exit summary's worker part.
    fn summary(&self) -> String {
        let s = &self.stats;
        let n = s.applies.load(Ordering::Relaxed);
        format!(
            "shadow worker: {} plan(s) applied in {} ms, queued {} ms in total ({} us max), {} job(s) of a waited-for root applied before older ones",
            n,
            s.busy_us.load(Ordering::Relaxed) / 1000,
            s.queued_us.load(Ordering::Relaxed) / 1000,
            s.queued_max_us.load(Ordering::Relaxed),
            s.promoted.load(Ordering::Relaxed),
        )
    }

    /// Block until every job submitted so far is done.
    fn wait_all(&self) {
        if self.thread.is_none() {
            return;
        }
        self.progress.wait_for(self.seqs.submitted());
    }

    /// Block until every job submitted so far for shadow root `root` is done: the root's
    /// mirror is then what its last plan made it. The root is urgent meanwhile, so its jobs
    /// go before other roots' queued jobs, which are not waited for (invgate.rs).
    fn wait_root(&self, root: u64) {
        if self.thread.is_none() {
            return;
        }
        self.progress
            .wait(&crate::invgate::Target::Roots(vec![(root, self.seqs.last_of(root))]));
    }

    /// The gate targets of a held invalidate of `roots`: each root and its last job.
    fn targets(&self, roots: &[u64]) -> Vec<(u64, u64)> {
        if self.thread.is_none() {
            return Vec::new();
        }
        roots.iter().map(|r| (*r, self.seqs.last_of(*r))).collect()
    }

    /// Block until every job is done and every teardown finished (before the host
    /// descriptors may be freed).
    fn wait_idle(&self) {
        self.wait_all();
        if self.thread.is_none() {
            return;
        }
        let (m, cv) = &*self.idle;
        let mut i = m.lock().unwrap();
        while !*i {
            i = cv.wait(i).unwrap();
        }
    }

    fn take_notes(&self) -> Vec<String> {
        std::mem::take(&mut *self.notes.lock().unwrap())
    }
}

impl Drop for ShadowWorker {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Validate the guest-owned physical half of one `GPU_PROMOTE_CTX` entry before the
/// privileged host control can see it. `physAttr` uses the public
/// `NV2080_CTRL_GPU_INITIALIZE_CTX_APERTURE` bits 1:0 and GPU-cacheable bit 2. The mirror
/// currently owns only the reservation's contiguous VIDMEM aperture; system-memory
/// physical addresses require an explicit registered-descriptor translation and are not
/// forwarded by this path.
fn validate_promote_phys(
    entry: &crate::channel::PromoteEntry,
    base: u64,
    end: u64,
) -> Result<(), String> {
    if entry.nonmapped && !entry.initialize {
        return Err("bNonmapped without bInitialize is illegal".into());
    }
    let Some((pa, attr)) = entry.phys else {
        return if entry.initialize {
            Err("bInitialize has no physical address".into())
        } else {
            Ok(())
        };
    };
    if !entry.initialize {
        return Err(format!("physical address {pa:#x} without bInitialize"));
    }
    if entry.size == 0 {
        return Err(format!("physical address {pa:#x} has zero size"));
    }
    if attr & !0x7 != 0 {
        return Err(format!("physical attributes {attr:#x} contain unsupported bits"));
    }
    match attr & 0x3 {
        0 => {
            let limit = pa
                .checked_add(entry.size)
                .ok_or_else(|| format!("VIDMEM range {pa:#x}+{:#x} wraps", entry.size))?;
            if pa < base || limit > end {
                return Err(format!(
                    "VIDMEM range [{pa:#x}, {limit:#x}) is outside the reservation [{base:#x}, {end:#x})"
                ));
            }
            Ok(())
        }
        1 => Err(format!(
            "COHERENT_SYSMEM physical range {pa:#x}+{:#x} has no registered-descriptor translation",
            entry.size
        )),
        2 => Err(format!(
            "NONCOHERENT_SYSMEM physical range {pa:#x}+{:#x} has no registered-descriptor translation",
            entry.size
        )),
        _ => Err(format!("physical attributes {attr:#x} name an invalid aperture")),
    }
}

/// c2 (exec-formats.md §4.5): `GPU_PROMOTE_CTX` for the host GR channel with the
/// guest's recorded entries (VAs of every buffer, the reservation PAs with the
/// initialize flag for MAIN and PATCH), so the host GSP initializes the guest's
/// MAIN/PATCH at their PAs from its golden image and binds the guest's context VAs to
/// the host channel, which walks the guest root. PRIVILEGED (flags 0x10244): one
/// CAP_SYS_ADMIN window (D12; the fifth control under it, execution.md). Returns the
/// status and its note; every PA handed over was checked against the reservation.
fn promote_gr(
    h: &mut HostMemory,
    host_channel: u32,
    entries: &[crate::channel::PromoteEntry],
    base: u64,
    end: u64,
) -> (u32, String) {
    if entries.is_empty() {
        return (
            0xFFFF_FFFF,
            format!("chid mirror {host_channel:#x}: no recorded promote entries; GR context not promoted (c2)"),
        );
    }
    for e in entries {
        if let Err(why) = validate_promote_phys(e, base, end) {
            return (
                sys::NV_ERR_INVALID_ARGUMENT,
                format!(
                    "chid mirror {host_channel:#x}: c2 GPU_PROMOTE_CTX entry {} refused before the privileged host control: {why} [guest, isolation]",
                    e.buffer_id
                ),
            );
        }
    }
    let root = h.root();
    let n = entries.len().min(v::PROMOTE_MAX_ENTRIES);
    let mut p = vec![0u8; v::PROMOTE_PARAMS_SIZE];
    let put32 = |b: &mut [u8], o: usize, v: u32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    let put64 = |b: &mut [u8], o: usize, v: u64| b[o..o + 8].copy_from_slice(&v.to_le_bytes());
    // engineType GR0, hClient/hChanClient = host root, hObject = host channel.
    put32(
        &mut p,
        v::PROMOTE_ENGINE_TYPE,
        v::lookup(v::NV2080_ENGINE_TYPES, "GR0").unwrap_or(1),
    );
    put32(&mut p, 4, root);
    put32(&mut p, v::PROMOTE_H_CHAN_CLIENT, root);
    put32(&mut p, v::PROMOTE_H_OBJECT, host_channel);
    put32(&mut p, v::PROMOTE_ENTRY_COUNT, n as u32);
    let mut desc = Vec::new();
    for (i, e) in entries.iter().take(n).enumerate() {
        let o = v::PROMOTE_ENTRIES + v::PROMOTE_ENTRY_SIZE * i;
        let (pa, attr) = e.phys.unwrap_or((0, 0));
        put64(&mut p, o, pa);
        put64(&mut p, o + 8, e.virt.unwrap_or(0));
        put64(&mut p, o + 16, e.size);
        put32(&mut p, o + 24, attr);
        p[o + 28..o + 30].copy_from_slice(&e.buffer_id.to_le_bytes());
        p[o + 30] = e.initialize as u8;
        p[o + 31] = e.nonmapped as u8;
        desc.push(format!(
            "{}: pa {pa:#x} va {:#x} size {:#x}{}",
            e.buffer_id,
            e.virt.unwrap_or(0),
            e.size,
            if e.initialize { " init" } else { "" }
        ));
    }
    let sub = h.subdevice();
    let st = h
        .control_admin(sub, v::CMD_GPU_PROMOTE_CTX, &mut p)
        .unwrap_or(0xFFFF_FFFF);
    (
        st,
        format!(
            "chid mirror {host_channel:#x}: c2 GPU_PROMOTE_CTX(admin) engineType GR0 hObject = the host channel, {n} entries [{}] -> {} [host]",
            desc.join("; "),
            status_name(st)
        ),
    )
}

impl Drop for Mirror {
    fn drop(&mut self) {
        // The poller forgets this mirror's USERDs.
        userd_watch_list()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|(id, _, _)| *id != self.watch_id);
        for n in self.reset() {
            eprintln!("vfio-user-nvgpu: mirror: {n}");
        }
    }
}

/// The event thread: wait on the event ctl fds; drain each record; raise the guest
/// vector of the engine (non-stall) or count the RC.
fn event_loop(events: Vec<(OsEvent, Option<u32>)>, intr: Arc<IntrShared>, stop: Arc<AtomicBool>) {
    use std::os::fd::AsRawFd;
    let mut fds: Vec<libc::pollfd> = events
        .iter()
        .map(|(e, _)| libc::pollfd {
            fd: e.ctl.as_raw_fd(),
            events: libc::POLLIN | libc::POLLPRI,
            revents: 0,
        })
        .collect();
    // The host interrupt gate (launch.rs `GATE_ENV`, backlog item 5): off unless set.
    let mut gate = crate::launch::gate_hold().map(crate::launch::Gate::new);
    let raise_mask = |mask: u64| {
        for v in (0..64).filter(|v| mask & (1u64 << v) != 0) {
            intr.raise(v);
        }
    };
    while !stop.load(Ordering::SeqCst) {
        for f in fds.iter_mut() {
            f.revents = 0;
        }
        let wait_ms = gate.as_ref().map_or(250, |g| {
            g.wait(std::time::Instant::now(), std::time::Duration::from_millis(250))
                .as_millis()
                .max(1) as i32
        });
        // SAFETY: a pollfd array over fds this thread owns.
        let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, wait_ms) };
        if let Some(g) = gate.as_mut() {
            let due = g.due(std::time::Instant::now());
            if due != 0 {
                raise_mask(due);
                intr.note(format!("event: intr gate released held vector(s) {due:#x} [host]"));
            }
        }
        if n <= 0 {
            continue;
        }
        for (i, f) in fds.iter().enumerate() {
            if f.revents & (libc::POLLIN | libc::POLLPRI) == 0 {
                continue;
            }
            let (ev, vector) = &events[i];
            let recs = ev.drain();
            match vector {
                Some(vec) => {
                    crate::launch::host_event();
                    // A host engine of the VM finished work: activity for the host boost.
                    crate::hostboost::activity();
                    if let Some(g) = gate.as_mut() {
                        let now = std::time::Instant::now();
                        match g.event(now, *vec, crate::launch::gate_last_doorbell()) {
                            // Held: delivered with the next hot event or when the hold ends.
                            None => continue,
                            Some(held) => raise_mask(held),
                        }
                    }
                    let msi = intr.raise(*vec);
                    crate::launch::raised();
                    intr.note(format!(
                        "event: host notifier {:#x} fired ({} record(s)) -> guest vector {vec}{} [host]",
                        ev.notifier,
                        recs.len(),
                        if msi { ", MSI" } else { ", no MSI (disabled or no fd)" }
                    ));
                }
                None => {
                    intr.rc_events.fetch_add(1, Ordering::Relaxed);
                    let what: Vec<String> = recs
                        .iter()
                        .map(|(_, idx, info32, info16)| {
                            format!("notify {idx:#x} exceptType {info32:#x} info16 {info16:#x}")
                        })
                        .collect();
                    let line = format!(
                        "HOST RC EVENT on a mirrored engine: {} [host]",
                        if what.is_empty() {
                            "no record".into()
                        } else {
                            what.join("; ")
                        }
                    );
                    eprintln!("vfio-user-nvgpu: mirror: {line}");
                    intr.note(line);
                }
            }
        }
    }
    if let Some(g) = gate {
        eprintln!("vfio-user-nvgpu: {}", g.summary());
    }
}

#[cfg(test)]
mod tests {
    /// The worker's pick (docs/caraxes/execution.md §8): a root someone waits for goes first,
    /// a root's jobs never pass each other, and without a waiter the oldest job goes.
    #[test]
    fn the_worker_takes_a_waited_for_root_first_and_keeps_each_roots_order() {
        const A: u64 = 0xA000;
        const B: u64 = 0xB000;
        let stats = WorkerStats::default();
        let mut q = JobQueues::default();
        for (seq, root) in [(1, A), (2, B), (3, A), (4, B)] {
            q.push(seq, Job::Free(root));
        }
        let seq = |j: Option<(u64, Job)>| j.map(|(s, _)| s);
        assert_eq!(seq(q.pop(&[], &stats)), Some(1));
        assert_eq!(seq(q.pop(&[B], &stats)), Some(2));
        assert_eq!(seq(q.pop(&[B], &stats)), Some(4), "B's next, before A's older job");
        assert_eq!(seq(q.pop(&[B], &stats)), Some(3), "B has nothing queued: the oldest");
        assert!(q.is_empty());
        assert_eq!(seq(q.pop(&[], &stats)), None);
        assert_eq!(stats.promoted.load(Ordering::Relaxed), 1);
    }

    use super::*;

    /// The USERD poller's view of one channel without the device lock (execution.md §9.11):
    /// the GPU-owned words go to the guest's USERD when the host's moved, a guest GPPut not
    /// yet published is pending, and a channel not ready or freed is left alone.
    #[test]
    fn the_userd_poller_harvests_and_finds_unforwarded_entries_off_the_lock() {
        let (mem, _, base, path) = crate::msgq::tests::queue_fixture("userd-poll");
        let host: &'static mut [u8; 4096] = Box::leak(Box::new([0u8; 4096]));
        let host_ptr = host.as_mut_ptr();
        let guest = base + 0x2000;
        let slot = UserdSlot::new(
            ShadowUserd {
                storage: crate::hostmem::HostUserd::for_test(host),
                guest,
                entries: 16,
                published_put: Some(2),
                harvested: None,
            },
            true,
        );
        // The host fetched to 1; the guest's GPPut is the published 2.
        // SAFETY: the page is leaked and only this test touches it.
        unsafe { std::ptr::write_volatile(host_ptr.add(0x88) as *mut u32, 1) };
        assert!(mem.write(guest + 0x8c, &2u32.to_le_bytes()));
        assert!(!slot.poll(&mem), "nothing new from the guest");
        assert_eq!(mem.read_u32(guest + 0x88), Some(1), "GPGet harvested to the guest");
        // The guest publishes entry 2: pending.
        assert!(mem.write(guest + 0x8c, &3u32.to_le_bytes()));
        assert!(slot.poll(&mem));
        // Out of the ring: not a submission.
        assert!(mem.write(guest + 0x8c, &16u32.to_le_bytes()));
        assert!(!slot.poll(&mem));
        assert!(mem.write(guest + 0x8c, &3u32.to_le_bytes()));
        slot.set_ready(false);
        assert!(!slot.poll(&mem), "not ready: left alone");
        slot.set_ready(true);
        let u = slot.take().expect("the channel's USERD");
        assert!(!slot.poll(&mem), "freed: left alone");
        assert_eq!(u.entries, 16);
        std::mem::forget(u);
        let _ = std::fs::remove_file(&path);
    }

    /// What a shadow job in flight may still map beyond the cached walks (bar1-pool.md
    /// §14): a page overlaps a range when it starts inside it; a teardown without its walk
    /// covers every page.
    #[test]
    fn a_ghost_covers_the_pages_that_start_inside_its_ranges() {
        let g = Ghost::Ranges(vec![(0x1000, 0x3000), (0x1_0000, 0x2_0000)]);
        let set = |v: &[u64]| v.iter().copied().collect::<BTreeSet<u64>>();
        assert!(!g.covers_any(&set(&[0x0, 0x3000, 0x2_0000])));
        assert!(g.covers_any(&set(&[0x3000, 0x2000])));
        assert!(g.covers_any(&set(&[0x1_f000])));
        assert!(!Ghost::All.covers_any(&set(&[])));
        assert!(Ghost::All.covers_any(&set(&[0x5000])));
    }
    use std::cell::Cell;

    /// The engines the mirror forwards are the chip's, with non-stall vectors in the
    /// notification subtree (vectors 0..63, cuda-path.md §5.1).
    #[test]
    fn engine_vectors_are_in_the_chip_table() {
        let chip = crate::fixture::rtx2070().chip.data();
        for (name, notifier) in ENGINES {
            let mc = v::lookup(v::MC_ENGINE_IDX, name).unwrap();
            let e = chip
                .interrupts
                .entries
                .iter()
                .find(|e| e.mc_idx as u32 == mc)
                .unwrap_or_else(|| panic!("{name} missing from interrupts.toml"));
            assert!(e.vector_nonstall != 0 && e.vector_nonstall < 64, "{name}");
            assert!(*notifier < 200);
        }
        assert_eq!(NOTIFIER_GRAPHICS | EVENT_NONSTALL, 0x0800_000C);
        assert_eq!(SETPD_SIZE, 32);
    }

    /// Software-executed completions use the same chip-table mapping as mirrored host
    /// notifications. COPY engine names deliberately map to the MC table's CE names.
    #[test]
    fn software_nonstall_vectors_follow_the_chip_table() {
        let chip = crate::fixture::rtx2070().chip.data();
        let engine = |name| v::lookup(v::NV2080_ENGINE_TYPES, name).unwrap();
        assert_eq!(nonstall_vector(chip, engine("GR0")), Some(4));
        assert_eq!(nonstall_vector(chip, engine("COPY2")), Some(5));
        assert_eq!(nonstall_vector(chip, engine("COPY3")), Some(6));
        assert_eq!(nonstall_vector(chip, engine("COPY4")), Some(7));
        assert_eq!(nonstall_vector(chip, engine("NVDEC0")), Some(8));
        assert_eq!(nonstall_vector(chip, engine("NVENC0")), Some(9));
        assert_eq!(nonstall_vector(chip, engine("COPY0")), None);
        assert_eq!(nonstall_vector(chip, 0xFFFF_FFFF), None);
    }

    /// Host privilege is never inherited: a privileged non-GR channel stays in software.
    /// Every other channel may be mirrored, because its host channel is allocated with
    /// DENY_PHYSICAL_MODE_CE (a bare CE channel with bit 7 clear included, the Vulkan X11
    /// present channel of 2026-09-28).
    #[test]
    fn physical_ce_gate_keeps_privileged_work_and_the_host_denies_physical_ce() {
        const PRIVILEGED: u32 = 1 << 5;
        const DENY_PHYSICAL_CE: u32 = 1 << 7;
        assert_eq!(HOST_MIRROR_CHANNEL_FLAGS, DENY_PHYSICAL_CE);
        assert_eq!(HOST_MIRROR_CHANNEL_FLAGS & PRIVILEGED, 0);
        assert!(needs_software_physical_ce(PRIVILEGED, false));
        assert!(!needs_software_physical_ce(PRIVILEGED, true));
        assert!(!needs_software_physical_ce(0, false));
        assert!(!needs_software_physical_ce(DENY_PHYSICAL_CE, false));
        // The guest's X11 present CE channel of SotTR (allocation flags 0x202000).
        assert!(!needs_software_physical_ce(0x20_2000, false));
    }

    /// Run P29 (Windows): every channel is privileged. A copy channel on the VA space of an
    /// application GR context (a skip-context GR channel on the same root: CUDA's COPY TSG
    /// channels) is an application channel and may be mirrored; the KMD's paging copy
    /// channel on the system VA space, where only the privileged management GR channel
    /// runs, stays in software; so does any channel without a root.
    #[test]
    fn a_privileged_copy_channel_of_an_application_vas_is_mirrored() {
        let chip = crate::fixture::rtx2070().chip.data();
        let gr0 = 1u32;
        let mut reg = registry();
        let vas = |reg: &mut Registry, client: u32, root: u64| {
            reg.record_alloc(client, 0x1, 0x3, v::FERMI_VASPACE_A, &[0u8; 56]);
            let mut pd = vec![0u8; 32];
            pd[v::SET_PD_PHYS_ADDRESS..][..8].copy_from_slice(&root.to_le_bytes());
            pd[v::SET_PD_NUM_ENTRIES..][..4].copy_from_slice(&4u32.to_le_bytes());
            pd[v::SET_PD_H_VASPACE..][..4].copy_from_slice(&0x3u32.to_le_bytes());
            reg.record_set_page_directory(client, &pd);
        };
        let channel =
            |reg: &mut Registry, client: u32, h: u32, chid: u32, engine: u32, flags: u32| {
                let mut cp = channel_params(chid, None);
                let put = |cp: &mut Vec<u8>, o: usize, v: u32| {
                    cp[o..o + 4].copy_from_slice(&v.to_le_bytes())
                };
                put(
                    &mut cp,
                    <Abi as nvrm_sys::RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_hVASpace,
                    0x3,
                );
                put(
                    &mut cp,
                    <Abi as nvrm_sys::RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_engineType,
                    engine,
                );
                let o = <Abi as nvrm_sys::RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_flags;
                let f = u32::from_le_bytes(cp[o..o + 4].try_into().unwrap());
                put(&mut cp, o, f | flags);
                reg.record_alloc(client, 0x1, h, chip.channel.gpfifo, &cp);
            };
        const PRIV: u32 = 1 << 5;
        // CUDA's process: GR channel with SKIP_CTXBUFFER_ALLOC and two privileged CE channels.
        let (app, system) = (0x1_0000_0000u64, 0x1_0800_0000u64);
        vas(&mut reg, 0xc1d0_006f, app);
        channel(
            &mut reg,
            0xc1d0_006f,
            0x40,
            22,
            gr0,
            PRIV | SKIP_CTXBUFFER_ALLOC,
        );
        vas(&mut reg, 0xc1d0_008e, app);
        channel(&mut reg, 0xc1d0_008e, 0x41, 32, 0x0c, PRIV);
        // The system address space: the management GR channel and the paging CE channel.
        vas(&mut reg, 0xc1d0_0002, system);
        channel(&mut reg, 0xc1d0_0002, 0x42, 4, gr0, PRIV);
        vas(&mut reg, 0xc1d0_0025, system);
        channel(&mut reg, 0xc1d0_0025, 0x43, 14, 0x0b, PRIV);
        let ch = |c: u32| reg.channel(c).unwrap();
        assert!(application_copy_channel(&reg, ch(32), gr0));
        assert!(!application_copy_channel(&reg, ch(14), gr0));
        // The rule the mirror applies: privileged non-GR stays in software unless it is an
        // application copy channel or a video channel (the KMD's own NVENC, run P31).
        let software = |reg: &Registry, c: u32| {
            privileged_stays_in_software(reg, reg.channel(c).unwrap(), gr0, false)
        };
        assert!(!software(&reg, 32));
        assert!(software(&reg, 14));
        vas(&mut reg, 0xc1d0_0128, system);
        channel(&mut reg, 0xc1d0_0128, 0x44, 8, 0x1b, PRIV);
        assert!(!application_copy_channel(
            &reg,
            reg.channel(8).unwrap(),
            gr0
        ));
        assert!(!software(&reg, 8), "a privileged NVENC channel is mirrored");
        // The application's GR channel gone: its copy channel is no application's any more.
        reg.record_free(0xc1d0_006f, 0x40);
        assert!(!application_copy_channel(
            &reg,
            reg.channel(32).unwrap(),
            gr0
        ));
    }

    /// The privileged context-promotion control receives only initialized VIDMEM ranges
    /// wholly owned by this mirror. VA-only entries remain valid; system apertures need a
    /// separate registered-descriptor translation and are refused by this path.
    #[test]
    fn promote_physical_entries_are_bounded_to_the_reservation() {
        const BASE: u64 = 0x1_0000_0000;
        const END: u64 = BASE + 0x20_0000;
        let physical = |pa, size, attr| crate::channel::PromoteEntry {
            buffer_id: 0,
            phys: Some((pa, attr)),
            size,
            initialize: true,
            ..Default::default()
        };

        assert!(validate_promote_phys(&physical(BASE, END - BASE, 0x4), BASE, END).is_ok());
        assert!(validate_promote_phys(&physical(BASE + 0x1000, 0x8c00, 0), BASE, END).is_ok());
        assert!(validate_promote_phys(
            &crate::channel::PromoteEntry {
                buffer_id: 9,
                virt: Some(0x1_2000_0000),
                ..Default::default()
            },
            BASE,
            END
        )
        .is_ok());

        for bad in [
            physical(BASE - 1, 1, 0x4),
            physical(END - 1, 2, 0x4),
            physical(u64::MAX - 1, 4, 0x4),
            physical(BASE, 0, 0x4),
            physical(BASE, 0x1000, 0x5),
            physical(BASE, 0x1000, 0x6),
            physical(BASE, 0x1000, 0x3),
            physical(BASE, 0x1000, 0x8),
        ] {
            assert!(validate_promote_phys(&bad, BASE, END).is_err(), "{bad:?}");
        }

        let mut missing = crate::channel::PromoteEntry {
            buffer_id: 2,
            initialize: true,
            ..Default::default()
        };
        assert!(validate_promote_phys(&missing, BASE, END).is_err());
        missing.initialize = false;
        missing.nonmapped = true;
        assert!(validate_promote_phys(&missing, BASE, END).is_err());
        let mut uninitialized = physical(BASE, 0x1000, 0x4);
        uninitialized.initialize = false;
        assert!(validate_promote_phys(&uninitialized, BASE, END).is_err());
    }

    /// Global GR context sizes belong to host engine properties, not to a guest channel:
    /// buffer ids 10..12 share PRIV_ACCESS_MAP and must cause only one host query. Unknown
    /// buffer ids never reach the host. The host's byte size is kept as answered
    /// (`global_exclusion` rounds it).
    #[test]
    fn global_context_sizes_are_cached_by_host_engine() {
        let calls = Cell::new(0u32);
        let mut cache = GlobalCtxSizeCache::default();
        assert_eq!(
            cache.get_or_query(2, |_| {
                calls.set(calls.get() + 1);
                (sys::NV_OK, 1)
            }),
            None
        );
        assert_eq!(calls.get(), 0, "an unknown buffer id has no host engine property");

        assert_eq!(
            cache.get_or_query(10, |engine| {
                calls.set(calls.get() + 1);
                assert_eq!(engine, 0x18);
                (sys::NV_OK, 0x1_0001)
            }),
            Some(0x1_0001)
        );
        for id in [11, 12] {
            assert_eq!(
                cache.get_or_query(id, |_| panic!("the shared host engine was queried twice")),
                Some(0x1_0001)
            );
        }
        assert_eq!(calls.get(), 1);

        assert_eq!(
            cache.get_or_query(5, |engine| {
                calls.set(calls.get() + 1);
                assert_eq!(engine, 0x13);
                (sys::NV_OK, 0x20_0000)
            }),
            Some(0x20_0000)
        );
        assert_eq!(calls.get(), 2, "a distinct host engine has its own property");
        assert_eq!(cache.queries, 2);
        assert_eq!(cache.hits, 2);
        assert_eq!(cache.by_engine.len(), 2);
    }

    /// NOT_SUPPORTED and an NV_OK zero answer are stable for a host-device generation and
    /// stay cached as `None`. Other failures are not cached: a later query can recover.
    #[test]
    fn global_context_size_cache_retries_only_nonstable_failures() {
        let calls = Cell::new(0u32);
        let mut cache = GlobalCtxSizeCache::default();

        assert_eq!(
            cache.get_or_query(7, |_| {
                calls.set(calls.get() + 1);
                (sys::NV_ERR_NOT_SUPPORTED, 0)
            }),
            None
        );
        assert_eq!(cache.get_or_query(7, |_| panic!("NOT_SUPPORTED was not cached")), None);

        assert_eq!(
            cache.get_or_query(8, |_| {
                calls.set(calls.get() + 1);
                (sys::NV_OK, 0)
            }),
            None
        );
        assert_eq!(cache.get_or_query(8, |_| panic!("the zero answer was not cached")), None);

        assert_eq!(
            cache.get_or_query(5, |_| {
                calls.set(calls.get() + 1);
                (sys::NV_ERR_INVALID_ARGUMENT, 0)
            }),
            None
        );
        assert_eq!(
            cache.get_or_query(5, |_| {
                calls.set(calls.get() + 1);
                (sys::NV_OK, 0x30_0000)
            }),
            Some(0x30_0000),
            "a transient/nonstable failure must not poison the device cache"
        );
        assert_eq!(
            cache.get_or_query(5, |_| panic!("the recovered success was not cached")),
            Some(0x30_0000)
        );
        assert_eq!(calls.get(), 4);
        assert_eq!(cache.queries, 4);
        assert_eq!(cache.hits, 3);
        assert_eq!(cache.by_engine.len(), 3);
    }

    /// The guest's channel allocation layout (the same in every guest ABI; the TSG's is
    /// `ver::layouts()`).
    type Abi = nvrm_sys::V610;

    fn registry() -> Registry {
        Registry::new(crate::fixture::rtx2070().chip.data().channel)
    }

    /// A TSG with the kernel's id `grp` in internalFlags (a 615 layout; the 610 struct has
    /// no such field, so the id is dropped there, as a 610 kernel sends none).
    fn tsg_params(grp: Option<u32>) -> Vec<u8> {
        let l = crate::ver::layouts();
        let mut b = vec![0u8; l.tsg_size];
        if let (Some(id), Some(off)) = (grp, l.tsg_internal_flags) {
            b[off..off + 4].copy_from_slice(&(id | 1 << 16 | 1 << 17).to_le_bytes());
        }
        b
    }

    /// A channel with `chid` in its flags and, for a bare channel, the kernel's internal TSG
    /// id `itsg` in internalFlags.
    fn channel_params(chid: u32, itsg: Option<u32>) -> Vec<u8> {
        let mut b = vec![0u8; <Abi as nvrm_sys::RmAbi>::ALLOC_CHANNEL_PARAMS_SIZE as usize];
        let flags = ((chid / 8) << 12) | ((chid % 8) << 8);
        let o = <Abi as nvrm_sys::RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_flags;
        b[o..o + 4].copy_from_slice(&flags.to_le_bytes());
        if let Some(id) = itsg {
            let f = <Abi as nvrm_sys::RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_internalFlags;
            b[f..f + 4].copy_from_slice(&(id << 8 | 1 << 24).to_le_bytes());
        }
        b
    }

    fn with_615_id() -> bool {
        crate::ver::layouts().tsg_internal_flags.is_some()
    }

    /// Item 3 of the GSP plan round 2 (classes.md §8, TSG): the host TSG map is keyed by the
    /// kernel's TSG id where the kernel sends one (615), by the guest handle otherwise
    /// (610); the key does not depend on the order of channel and TSG allocations, and a
    /// bare channel's internal TSG is keyed by its own id.
    #[test]
    fn host_tsg_key_is_the_kernel_tsg_id() {
        for g in [crate::ver::GuestAbi::V610, crate::ver::GuestAbi::V615] {
            let _g = crate::ver::TestGuest::set(g);
            assert_eq!(with_615_id(), g == crate::ver::GuestAbi::V615);
            host_tsg_key_case();
        }
    }

    fn host_tsg_key_case() {
        let gpfifo = crate::fixture::rtx2070().chip.data().channel.gpfifo;
        let grp = |id: u32, c: u32, h: u32| {
            if with_615_id() {
                TsgKey::Grp(id)
            } else {
                TsgKey::Handle(c, h)
            }
        };
        // Order 1: TSG 0x30 (grpID 7), then its channels 2 and 3.
        let mut reg = registry();
        reg.record_alloc(1, 0x10, 0x30, v::KEPLER_CHANNEL_GROUP_A, &tsg_params(Some(7)));
        reg.record_alloc(1, 0x30, 0x40, gpfifo, &channel_params(2, None));
        reg.record_alloc(1, 0x30, 0x41, gpfifo, &channel_params(3, None));
        let k2 = tsg_key_of(&reg, reg.channel(2).unwrap());
        let k3 = tsg_key_of(&reg, reg.channel(3).unwrap());
        assert_eq!(k2, (grp(7, 1, 0x30), false, (1, 0x30)));
        assert_eq!(k2, k3, "the channels of one TSG meet at one host TSG");
        assert_eq!(tsg_key_of_guest(&reg, (1, 0x30)), Some(grp(7, 1, 0x30)));
        // Order 2: a second TSG (grpID 8) allocated after a bare channel with internal TSG 9,
        // then a channel of the second TSG; the keys follow the ids, not the order.
        reg.record_alloc(1, 0x10, 0x50, gpfifo, &channel_params(4, Some(9)));
        reg.record_alloc(1, 0x10, 0x31, v::KEPLER_CHANNEL_GROUP_A, &tsg_params(Some(8)));
        reg.record_alloc(1, 0x31, 0x42, gpfifo, &channel_params(5, None));
        let bare = tsg_key_of(&reg, reg.channel(4).unwrap());
        assert!(bare.1);
        assert_eq!(bare.2, (1, 0x50));
        if with_615_id() {
            assert_eq!(bare.0, TsgKey::Grp(9), "a bare channel's internal TSG id");
        }
        assert_eq!(tsg_key_of(&reg, reg.channel(5).unwrap()).0, grp(8, 1, 0x31));
        assert_ne!(k2.0, tsg_key_of(&reg, reg.channel(5).unwrap()).0);
        // A channel of a TSG under another client with the same handle is another TSG.
        reg.record_alloc(2, 0x10, 0x30, v::KEPLER_CHANNEL_GROUP_A, &tsg_params(Some(10)));
        reg.record_alloc(2, 0x30, 0x40, gpfifo, &channel_params(6, None));
        assert_eq!(tsg_key_of(&reg, reg.channel(6).unwrap()).0, grp(10, 2, 0x30));
        // Reuse: the same guest TSG joins; a freed TSG's id taken by a new TSG is stale; two
        // live TSGs under one id collide.
        assert_eq!(tsg_reuse(&reg, (1, 0x30), (1, 0x30)), TsgReuse::Join);
        assert_eq!(tsg_reuse(&reg, (1, 0x30), (1, 0x31)), TsgReuse::Collision);
        reg.record_free(1, 0x30);
        reg.record_alloc(1, 0x10, 0x32, v::KEPLER_CHANNEL_GROUP_A, &tsg_params(Some(7)));
        assert_eq!(tsg_reuse(&reg, (1, 0x30), (1, 0x32)), TsgReuse::Stale);
        assert_eq!(tsg_key_of_guest(&reg, (1, 0x32)), Some(grp(7, 1, 0x32)));
        assert_eq!(tsg_key_of_guest(&reg, (1, 0x30)), None);
        // Keys sort and print.
        assert!(TsgKey::Grp(1) < TsgKey::Handle(0, 0));
        assert_eq!(TsgKey::Grp(7).to_string(), "grpID 7");
        assert_eq!(TsgKey::Handle(1, 0x30).to_string(), "0x1/0x30");
    }

    /// Run P30: the eviction of a TSG's context through its last channel, which never rang,
    /// frees the host TSG instead of refusing ("host context is gone" was an Xid 45 and a
    /// TDR after every D3D12 process); with other members still hosted it is refused.
    #[test]
    fn a_deferred_eviction_through_a_member_that_never_ran_frees_the_tsg() {
        assert_eq!(evict_path(true, false, true), EvictPath::HostChannel);
        assert_eq!(evict_path(true, true, true), EvictPath::HostChannel);
        assert_eq!(evict_path(false, false, true), EvictPath::FreeTsg);
        assert_eq!(evict_path(false, false, false), EvictPath::Nothing);
        assert_eq!(evict_path(false, true, true), EvictPath::Refuse);
    }

    /// Step 1's record-then-apply (preempt.rs): a guest SET is recorded under the guest
    /// object the host TSG will stand for (`tsg_key_of`), whether it names the TSG (CUDA,
    /// GL, Vulkan), a member channel, or a bare channel (Vulkan's 0xc46f SETs), so the build
    /// finds it; it goes with its guest object.
    #[test]
    fn a_guest_set_is_recorded_for_the_host_tsg_of_its_guest_object() {
        let gpfifo = crate::fixture::rtx2070().chip.data().channel.gpfifo;
        let set = |h: u32, cilp: u32| {
            let mut p = vec![0u8; 32];
            p[0..4].copy_from_slice(&crate::preempt::FLAG_CILP_SET.to_le_bytes());
            p[4..8].copy_from_slice(&h.to_le_bytes());
            p[12..16].copy_from_slice(&cilp.to_le_bytes());
            p
        };
        let mut reg = registry();
        reg.record_alloc(
            1,
            0x10,
            0x30,
            v::KEPLER_CHANNEL_GROUP_A,
            &tsg_params(Some(22)),
        );
        reg.record_alloc(1, 0x30, 0x40, gpfifo, &channel_params(24, None));
        reg.record_alloc(1, 0x30, 0x41, gpfifo, &channel_params(25, None));
        reg.record_alloc(1, 0x10, 0x50, gpfifo, &channel_params(4, Some(7))); // bare
                                                                              // Before any SET the build finds nothing (the host default stays).
        let (_, _, guest) = tsg_key_of(&reg, reg.channel(24).unwrap());
        assert_eq!(guest, (1, 0x30));
        assert_eq!(reg.preemption(guest), None);
        // CUDA names the TSG.
        let (a, g) = crate::cuda::set_ctxsw_preemption_mode(1, &set(0x30, COMPUTE_CILP), &mut reg);
        assert_eq!((a.status, a.params.len()), (sys::NV_OK, 32));
        assert_eq!(g, Some(guest));
        assert!(a.note.contains("compute CILP (2)"), "{}", a.note);
        assert!(
            a.note.contains("recorded for guest TSG 0x1/0x30"),
            "{}",
            a.note
        );
        assert_eq!(reg.preemption(guest).unwrap().compute, Some(COMPUTE_CILP));
        // A member channel names its TSG: the same record, the later mode.
        let (a, g) = crate::cuda::set_ctxsw_preemption_mode(1, &set(0x41, COMPUTE_CTA), &mut reg);
        assert_eq!(g, Some(guest));
        assert!(a.note.contains("(the TSG of channel 0x41)"), "{}", a.note);
        let (_, _, g25) = tsg_key_of(&reg, reg.channel(25).unwrap());
        assert_eq!(
            reg.preemption(g25).unwrap(),
            crate::preempt::Recorded {
                compute: Some(COMPUTE_CTA),
                graphics: None,
                sets: 2
            }
        );
        // A bare channel stands for its own TSG.
        let (_, _, bare) = tsg_key_of(&reg, reg.channel(4).unwrap());
        let (a, g) = crate::cuda::set_ctxsw_preemption_mode(1, &set(0x50, COMPUTE_CTA), &mut reg);
        assert_eq!(g, Some(bare));
        assert!(a.note.contains("bare channel 0x1/0x50"), "{}", a.note);
        // Unknown handles and short payloads: NV_OK as before, nothing recorded.
        let (a, g) = crate::cuda::set_ctxsw_preemption_mode(1, &set(0x99, COMPUTE_CTA), &mut reg);
        assert_eq!((a.status, g), (sys::NV_OK, None));
        assert!(a.note.contains("not recorded"), "{}", a.note);
        let (a, g) = crate::cuda::set_ctxsw_preemption_mode(1, &[1, 0, 0, 0], &mut reg);
        assert_eq!(
            (a.status, a.params.as_slice(), g),
            (sys::NV_OK, &[1u8, 0, 0, 0][..], None)
        );
        let s = reg.preemption_summary().unwrap();
        assert!(s.contains("1 unresolved, 1 too short"), "{s}");
        // Freed with the guest objects.
        reg.record_free(1, 0x30);
        assert_eq!(reg.preemption(guest), None);
        assert!(reg.preemption(bare).is_some());
        reg.record_free(1, 0x50);
        assert_eq!(reg.preemption(bare), None);
    }

    /// The video engines by `NV2080_ENGINE_TYPE` (NVDEC0 0x13, NVENC0 0x1b on 610 and
    /// 615) and the chip's video classes: TU106 has NVC4B0 and NVC4B7, and the copy and
    /// compute classes are no video classes (fail closed on the channel).
    #[test]
    fn video_engines_and_classes() {
        let chip = crate::fixture::rtx2070().chip.data();
        assert_eq!(video_engine(0x1B), Some("NVENC0"));
        assert_eq!(video_engine(0x13), Some("NVDEC0"));
        assert_eq!(video_engine(0x01), None);
        assert_eq!(video_engine(0x0B), None);
        assert!(video_class_ok(chip, 0xC4B7));
        assert!(video_class_ok(chip, 0xC4B0));
        // Ampere's NVENC and NVDEC are not in TU106's database; CE and compute never are.
        assert!(!video_class_ok(chip, 0xC7B7));
        assert!(!video_class_ok(chip, 0xC6B0));
        assert!(!video_class_ok(chip, 0xC5B5));
        assert!(!video_class_ok(chip, 0xC5C0));
        assert_eq!(NOTIFIER_NVENC0 | EVENT_NONSTALL, 0x0800_0026);
        assert_eq!(NOTIFIER_NVDEC0 | EVENT_NONSTALL, 0x0800_000E);
    }

    /// Run P29b: a Windows video channel carries the KMD's deferred-API object beside its
    /// video class, and is mirrored; any other foreign class still keeps it in software.
    #[test]
    fn a_video_channel_may_carry_the_deferred_api_object() {
        let chip = crate::fixture::rtx2070().chip.data();
        let obj = |c: u32| (c, Vec::new());
        assert_eq!(video_channel_foreign_class(chip, &[obj(0xC4B7)]), None);
        assert_eq!(
            video_channel_foreign_class(chip, &[obj(DEFERRED_API_CLASS), obj(0xC4B7)]),
            None
        );
        assert_eq!(
            video_channel_foreign_class(chip, &[obj(DEFERRED_API_CLASS), obj(0xC4B0)]),
            None
        );
        assert_eq!(
            video_channel_foreign_class(chip, &[obj(DEFERRED_API_CLASS)]),
            None
        );
        assert_eq!(
            video_channel_foreign_class(chip, &[obj(DEFERRED_API_CLASS), obj(0xC5B5)]),
            Some(0xC5B5)
        );
        assert_eq!(
            video_channel_foreign_class(chip, &[obj(0xC4B7), obj(0xC5C0)]),
            Some(0xC5C0)
        );
    }

    /// A video channel with no video object yet is bound to its own engine at its build
    /// (the guest waits for the first push before it allocates the object); once the
    /// chip's video object exists, or for a GR or CE channel, nothing is bound.
    #[test]
    fn a_video_channel_without_its_object_is_bound_to_its_engine() {
        let chip = crate::fixture::rtx2070().chip.data();
        let mut reg = registry();
        let mut cp = channel_params(22, Some(3));
        let o = <Abi as nvrm_sys::RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_engineType;
        cp[o..o + 4].copy_from_slice(&0x1Bu32.to_le_bytes());
        reg.record_alloc(1, 0x80, 0x14, chip.channel.gpfifo, &cp);
        assert_eq!(video_bind(chip, reg.channel(22).unwrap()), Some(0x1B));
        // A foreign class on it is no video object of this chip.
        reg.record_alloc(1, 0x14, 0x31, 0xC7B7, &[]);
        assert_eq!(video_bind(chip, reg.channel(22).unwrap()), Some(0x1B));
        reg.record_alloc(1, 0x14, 0x32, 0xC4B7, &[]);
        assert_eq!(video_bind(chip, reg.channel(22).unwrap()), None);
        // A CE channel (engineType COPY2 0x0b) is not a video channel.
        let mut ce = channel_params(23, Some(4));
        ce[o..o + 4].copy_from_slice(&0x0Bu32.to_le_bytes());
        reg.record_alloc(1, 0x80, 0x15, chip.channel.gpfifo, &ce);
        assert_eq!(video_bind(chip, reg.channel(23).unwrap()), None);
        // The copy bind takes it (the NVENC library's CE channel rings before its object),
        // and neither the video nor the GR channel.
        assert_eq!(copy_bind(reg.channel(23).unwrap()), Some(0x0B));
        assert_eq!(copy_bind(reg.channel(22).unwrap()), None);
        let mut gr = channel_params(24, Some(5));
        gr[o..o + 4].copy_from_slice(&0x01u32.to_le_bytes());
        reg.record_alloc(1, 0x80, 0x16, chip.channel.gpfifo, &gr);
        assert_eq!(copy_bind(reg.channel(24).unwrap()), None);
    }

    /// The ranges the shadow keeps out of set A: GR buffers with their size, a global GR
    /// buffer promoted by VA alone with the host's size, a falcon context by its VA.
    #[test]
    fn promoted_ranges_cover_gr_and_falcon_contexts() {
        let chip = crate::fixture::rtx2070().chip.data();
        let mut reg = registry();
        reg.record_alloc(1, 0x80, 0x14, chip.channel.gpfifo, &channel_params(22, Some(3)));
        let mut p = vec![0u8; v::PROMOTE_PARAMS_SIZE];
        p[v::PROMOTE_H_CHAN_CLIENT..][..4].copy_from_slice(&1u32.to_le_bytes());
        p[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&0x14u32.to_le_bytes());
        p[v::PROMOTE_VIRT_ADDRESS..][..8].copy_from_slice(&0x1_2002_d000u64.to_le_bytes());
        p[v::PROMOTE_SIZE..][..8].copy_from_slice(&0x800u64.to_le_bytes());
        reg.record_promote_ctx(&p);
        let ch = reg.channel(22).unwrap().clone();
        assert_eq!(
            promoted_ranges(&reg, &ch, |_, _| None),
            vec![(0x1_2002_d000, 0x1_2002_e000)],
            "a context smaller than a page covers its page"
        );
        let mut g = vec![0u8; v::PROMOTE_PARAMS_SIZE];
        g[v::PROMOTE_H_CHAN_CLIENT..][..4].copy_from_slice(&1u32.to_le_bytes());
        g[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&0x14u32.to_le_bytes());
        g[v::PROMOTE_ENTRY_COUNT..][..4].copy_from_slice(&1u32.to_le_bytes());
        g[v::PROMOTE_ENTRIES + 8..][..8].copy_from_slice(&0x1_2040_0000u64.to_le_bytes());
        g[v::PROMOTE_ENTRIES + 28..][..2].copy_from_slice(&5u16.to_le_bytes());
        reg.record_promote_ctx(&g);
        let r = promoted_ranges(&reg, &ch, |id, _| (id == 5).then_some(0x20_0000));
        assert!(r.contains(&(0x1_2040_0000, 0x1_2060_0000)), "{r:x?}");
        assert!(r.contains(&(0x1_2002_d000, 0x1_2002_e000)), "{r:x?}");
    }

    /// The exclusions follow the guest's live channels: a GR context promoted by one
    /// channel keeps its VAs out of set A while that channel lives (for every channel of
    /// the root), and gives them back once the guest frees it (vm4-615, 2026-09-25: a stale
    /// attribute-CB exclusion at 0x121400000 left a reused VA unmapped, Xid 31 on CE0).
    #[test]
    fn exclusions_are_released_when_the_channel_goes() {
        let chip = crate::fixture::rtx2070().chip.data();
        let mut reg = registry();
        reg.record_alloc(1, 0x80, 0x14, chip.channel.gpfifo, &channel_params(22, Some(3)));
        reg.record_alloc(1, 0x80, 0x15, chip.channel.gpfifo, &channel_params(23, Some(4)));
        let mut g = vec![0u8; v::PROMOTE_PARAMS_SIZE];
        g[v::PROMOTE_H_CHAN_CLIENT..][..4].copy_from_slice(&1u32.to_le_bytes());
        g[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&0x14u32.to_le_bytes());
        g[v::PROMOTE_ENTRY_COUNT..][..4].copy_from_slice(&1u32.to_le_bytes());
        g[v::PROMOTE_ENTRIES + 8..][..8].copy_from_slice(&0x1_2140_0000u64.to_le_bytes());
        g[v::PROMOTE_ENTRIES + 28..][..2].copy_from_slice(&5u16.to_le_bytes());
        reg.record_promote_ctx(&g);
        let size = |id: u16, _: u64| (id == 5).then_some(0x20_0000u64);
        let all = |_: &Channel| true;
        assert_eq!(exclusions_among(&reg, all, size), vec![(0x1_2140_0000, 0x1_2160_0000)]);
        // Only the root's channels count.
        assert!(exclusions_among(&reg, |c: &Channel| c.chid == 23, size).is_empty());
        // The guest frees the channel that promoted it: the range is set A again.
        reg.record_free(1, 0x14);
        assert!(exclusions_among(&reg, all, size).is_empty());
    }

    /// The order of a GR channel's teardown (vm4-615 18:12:42 and 20:21:50, 23 refused fixed
    /// mappings at 0x120000000 and up): the guest frees its channel, a guest invalidate
    /// re-syncs the shadow, and only later does the guest free the TSG whose host TSG holds
    /// the host's GR context at the same VAs. The re-sync between the two must not map the
    /// promoted VAs; after the host TSG free it maps them. A channel whose host TSG is
    /// already gone releases at once (4a6ba83's behaviour).
    #[test]
    fn set_b_is_released_only_after_the_host_tsg_free() {
        let chip = crate::fixture::rtx2070().chip.data();
        // The guest's tables: four 4 KiB leaves at VA 0x1000..0x5000 of root BASE (the small-page
        // half of PD0 entry 0).
        const BASE: u64 = 0x1_0000_0000;
        const END: u64 = 0x2_0000_0000;
        let mut fb = FakeFb::new(END + (64 << 20));
        let vid = |a: u64| ((a >> 12) << 8) | 0b010;
        let pte = |a: u64| ((a >> 12) << 8) | 1 | (0x06u64 << 56);
        let (root, pd2, pd1, pd0, pt) = (BASE, BASE + 0x1000, BASE + 0x2000, BASE + 0x3000, BASE + 0x4000);
        fb.write_u64(root, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        fb.write_u64(pd0 + 8, vid(pt));
        let x = BASE + 0x10_0000;
        for i in 0..4u64 {
            fb.write_u64(pt + 8 * (i + 1), pte(x + i * 0x1000));
        }
        let mem = GuestMemory::new();
        // A guest TSG with a GR channel that promoted a context at 0x2000 (two pages).
        let mut reg = registry();
        reg.record_alloc(1, 0x10, 0x30, v::KEPLER_CHANNEL_GROUP_A, &tsg_params(Some(26)));
        reg.record_alloc(1, 0x30, 0x40, chip.channel.gpfifo, &channel_params(2, None));
        let mut g = vec![0u8; v::PROMOTE_PARAMS_SIZE];
        g[v::PROMOTE_H_CHAN_CLIENT..][..4].copy_from_slice(&1u32.to_le_bytes());
        g[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&0x40u32.to_le_bytes());
        g[v::PROMOTE_ENTRY_COUNT..][..4].copy_from_slice(&1u32.to_le_bytes());
        g[v::PROMOTE_ENTRIES + 8..][..8].copy_from_slice(&0x2000u64.to_le_bytes());
        g[v::PROMOTE_ENTRIES + 16..][..8].copy_from_slice(&0x2000u64.to_le_bytes());
        reg.record_promote_ctx(&g);
        let key = tsg_key_of(&reg, reg.channel(2).unwrap()).0;
        let size = |_: u16, _: u64| None;
        let all = |_: &Channel| true;
        let mut held: BTreeMap<TsgKey, Vec<(u64, u64)>> = BTreeMap::new();
        // The host TSGs standing (the mirror's `tsgs`): the one of the guest TSG.
        let mut host_tsgs = std::collections::BTreeSet::from([key]);
        let mut front = crate::shadow::ShadowFront::new(root);
        let set_b = |reg: &Registry, held: &BTreeMap<TsgKey, Vec<(u64, u64)>>| {
            set_b_with_held(exclusions_among(reg, all, size), held)
        };
        let b = set_b(&reg, &held);
        assert_eq!(b, vec![(0x2000, 0x4000)]);
        let plan = front.plan(&fb, &mem, BASE, END, 1, &b, &[]).unwrap();
        assert_eq!((plan.want, plan.excluded), (2, 2), "set B stays unmapped while the channel lives");
        // The guest frees the channel; its host TSG (the GR context) is still alive.
        for h in held_at_free(
            &reg,
            |c| c.h_object == 0x40,
            |_| Some(root),
            |_, id, va| size(id, va),
        ) {
            assert_eq!((h.chid, h.root, h.key), (2, root, key));
            if host_tsgs.contains(&h.key) {
                held.entry(h.key).or_default().extend(h.ranges);
            }
        }
        reg.record_free(1, 0x40);
        assert!(exclusions_among(&reg, all, size).is_empty(), "the live channels hold nothing now");
        // The guest's invalidate between the two frees: set B is unchanged, nothing is mapped.
        let b = set_b(&reg, &held);
        assert_eq!(b, vec![(0x2000, 0x4000)], "held by the live host TSG");
        assert!(front.plan(&fb, &mem, BASE, END, 1, &b, &[]).is_none(), "the promoted VAs are not mapped");
        // The host TSG is freed (the guest's TSG FREE): the held ranges go back to set A and
        // the re-sync after it maps the two pages.
        assert!(host_tsgs.remove(&key));
        assert!(held.remove(&key).is_some());
        let b = set_b(&reg, &held);
        assert!(b.is_empty());
        let plan = front.plan(&fb, &mem, BASE, END, 1, &b, &[]).unwrap();
        assert_eq!((plan.want, plan.excluded), (4, 0), "after the host free the pages are mapped");
        // A channel whose host TSG is already gone (never built, or a bare channel's freed
        // with it) holds nothing: released at once.
        reg.record_alloc(1, 0x30, 0x41, chip.channel.gpfifo, &channel_params(3, None));
        g[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&0x41u32.to_le_bytes());
        reg.record_promote_ctx(&g);
        for h in held_at_free(
            &reg,
            |c| c.h_object == 0x41,
            |_| Some(root),
            |_, id, va| size(id, va),
        ) {
            if host_tsgs.contains(&h.key) {
                held.entry(h.key).or_default().extend(h.ranges);
            }
        }
        reg.record_free(1, 0x41);
        assert!(set_b(&reg, &held).is_empty());
        // A channel without a shadow root holds nothing either.
        reg.record_alloc(1, 0x30, 0x42, chip.channel.gpfifo, &channel_params(4, None));
        g[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&0x42u32.to_le_bytes());
        reg.record_promote_ctx(&g);
        assert!(held_at_free(
            &reg,
            |c| c.h_object == 0x42,
            |_| None,
            |_, id, va| size(id, va)
        )
        .is_empty());
    }

    /// A Vulkan client's VAS as the 615 guest builds it (vm4-615, 2026-09-26): client 0xb1
    /// allocates VAS 0x3 and sets its root; 0xb3 and 0xb4 duplicate it; 0xb4's GR channel
    /// runs on its alias 0xbeef0300 and promoted a context at 0x2000 (two pages). Returns the
    /// registry and the channel's chid.
    fn dup_vas_world(root: u64) -> (Registry, u32) {
        let chip = crate::fixture::rtx2070().chip.data();
        let mut reg = registry();
        reg.record_alloc(0xb1, 0x1, 0x3, v::FERMI_VASPACE_A, &[0u8; 56]);
        let mut pd = vec![0u8; 32];
        pd[v::SET_PD_PHYS_ADDRESS..][..8].copy_from_slice(&root.to_le_bytes());
        pd[v::SET_PD_NUM_ENTRIES..][..4].copy_from_slice(&4u32.to_le_bytes());
        pd[v::SET_PD_H_VASPACE..][..4].copy_from_slice(&0x3u32.to_le_bytes());
        reg.record_set_page_directory(0xb1, &pd);
        assert!(reg.record_dup((0xb3, 0xbfef_0001), (0xb1, 0x3)).is_some());
        assert!(reg.record_dup((0xb4, 0xbeef_0300), (0xb1, 0x3)).is_some());
        reg.record_alloc(0xb4, 0x10, 0x30, v::KEPLER_CHANNEL_GROUP_A, &tsg_params(Some(10)));
        let mut cp = channel_params(8, None);
        let o = <Abi as nvrm_sys::RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_hVASpace;
        cp[o..o + 4].copy_from_slice(&0xbeef_0300u32.to_le_bytes());
        reg.record_alloc(0xb4, 0x30, 0x40, chip.channel.gpfifo, &cp);
        let mut g = vec![0u8; v::PROMOTE_PARAMS_SIZE];
        g[v::PROMOTE_H_CHAN_CLIENT..][..4].copy_from_slice(&0xb4u32.to_le_bytes());
        g[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&0x40u32.to_le_bytes());
        g[v::PROMOTE_ENTRY_COUNT..][..4].copy_from_slice(&1u32.to_le_bytes());
        g[v::PROMOTE_ENTRIES + 8..][..8].copy_from_slice(&0x2000u64.to_le_bytes());
        g[v::PROMOTE_ENTRIES + 16..][..8].copy_from_slice(&0x2000u64.to_le_bytes());
        reg.record_promote_ctx(&g);
        (reg, 8)
    }

    /// The shadow root of a channel, as `Mirror::free_object` names it.
    fn fb_root(reg: &Registry, c: &Channel) -> Option<u64> {
        match reg.vas_key_of(c).and_then(|k| reg.vas_root(k)) {
            Some(Target::Fb(r)) => Some(r),
            _ => None,
        }
    }

    /// The five bursts of vm4-615 on 2026-09-26 (04:37-04:48, 31 refused fixed VA
    /// allocations each): the guest freed the client that owns a dup'ed VAS before the
    /// client of the GR channels on it. The registry dropped the VAS, the channels resolved
    /// to no root, set B of the root was empty while the host TSGs stood, and the next
    /// re-sync mapped over the host's GR context. The VAS now stays under a surviving alias:
    /// root, root entries, set B and the hold at the channel's free are unchanged.
    #[test]
    fn set_b_survives_the_vas_owner_client_free() {
        const ROOT: u64 = 0x1_3567_7000;
        let (mut reg, chid) = dup_vas_world(ROOT);
        let size = |_: u16, _: u64| None;
        let ch = reg.channel(chid).unwrap().clone();
        assert_eq!(reg.vas_key_of(&ch), Some((0xb1, 0x3)));
        assert_eq!(exclusions_of(&reg, ROOT, size), vec![(0x2000, 0x4000)]);
        // The owner goes first: kept under the first surviving alias, the other repointed.
        let notes = reg.record_free(0xb1, 0xb1);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("kept under 0xb3/0xbfef0001"), "{notes:?}");
        assert_eq!(reg.vas_key_of(&ch), Some((0xb3, 0xbfef_0001)));
        assert_eq!(reg.vas_root(reg.vas_key_of(&ch).unwrap()), Some(Target::Fb(ROOT)));
        assert_eq!(reg.root_entries((0xb4, 0xbeef_0300)), 4);
        assert!(reg.vas_replay((0xb4, 0xbeef_0300)).0.is_some(), "the alloc parameters moved too");
        assert_eq!(exclusions_of(&reg, ROOT, size), vec![(0x2000, 0x4000)], "set B unchanged");
        // The second duplicate goes (the trace's order): the channel's own alias holds it.
        let notes = reg.record_free(0xb3, 0xb3);
        assert!(notes.len() == 1 && notes[0].contains("kept under 0xb4/0xbeef0300"), "{notes:?}");
        assert_eq!(reg.vas_key_of(&ch), Some((0xb4, 0xbeef_0300)));
        assert_eq!(exclusions_of(&reg, ROOT, size), vec![(0x2000, 0x4000)]);
        // The channel's free still finds its root: its ranges are held for the host TSG.
        let held = held_at_free(
            &reg,
            |c| c.h_object == 0x40,
            |c| fb_root(&reg, c),
            |_, id, va| size(id, va),
        );
        assert_eq!(held.len(), 1);
        assert_eq!((held[0].chid, held[0].root, held[0].ranges.clone()), (chid, ROOT, vec![(0x2000, 0x4000)]));
        // The last client goes: nothing is left, and no alias to keep it under.
        assert!(reg.record_free(0xb4, 0xb4).is_empty());
        assert_eq!(reg.vas_root((0xb4, 0xbeef_0300)), None);
        assert!(exclusions_of(&reg, ROOT, size).is_empty());
    }

    /// The owner freed last (the channels' client first): the behaviour before the re-home.
    /// The channel's free holds its ranges on the root, the duplicates go without a note,
    /// and the owner's free takes the VAS with it.
    #[test]
    fn a_vas_owner_freed_last_keeps_the_old_behaviour() {
        const ROOT: u64 = 0x1_3567_7000;
        let (mut reg, chid) = dup_vas_world(ROOT);
        let size = |_: u16, _: u64| None;
        let held = held_at_free(
            &reg,
            |c| c.h_client == 0xb4,
            |c| fb_root(&reg, c),
            |_, id, va| size(id, va),
        );
        assert_eq!((held.len(), held[0].chid, held[0].root), (1, chid, ROOT));
        assert!(reg.record_free(0xb4, 0xb4).is_empty());
        assert!(exclusions_of(&reg, ROOT, size).is_empty(), "the channel is gone");
        assert_eq!(reg.vas_root((0xb1, 0x3)), Some(Target::Fb(ROOT)), "the owner keeps it");
        assert!(reg.record_free(0xb3, 0xb3).is_empty());
        assert_eq!(reg.vas_root((0xb1, 0x3)), Some(Target::Fb(ROOT)));
        assert!(reg.record_free(0xb1, 0xb1).is_empty(), "no surviving alias: nothing kept");
        assert_eq!(reg.vas_root((0xb1, 0x3)), None);
        assert_eq!(reg.vas_root((0xb3, 0xbfef_0001)), None);
    }

    /// The video layout: the host-only area sits below the guest's RM base, inside the
    /// guest's server-RM range; guest leaves below the base, in the host area or in the
    /// host's moved server-RM range are unmirrorable, everything else is mirrored.
    #[test]
    fn the_video_layout_keeps_the_host_area_out_of_the_guest_range() {
        use crate::shadow::{Layout, GUEST_RM_VA_BASE, SERVER_RM_VA_SIZE, SERVER_RM_VA_START};
        let l = Layout::video();
        assert_eq!(l.va_base, 0x1_1C00_0000);
        assert_eq!(l.host_area, (0x1_1C00_0000, 0x1_2000_0000));
        assert_eq!(l.host_server, (0x2_1C00_0000, 0x2_3C00_0000));
        // The host area lies inside the guest's own server-RM range: never a guest VA.
        assert!(l.host_area.0 >= SERVER_RM_VA_START && l.host_area.1 <= SERVER_RM_VA_START + SERVER_RM_VA_SIZE);
        assert_eq!(l.host_area.1, GUEST_RM_VA_BASE);
        // The guest's kernel-RM buffers (probe-merged610) are mirrored.
        for va in [0x1_2002_d000u64, 0x1_2007_0000, 0x1_2040_0000, 0x1_205f_e000] {
            assert_eq!(l.unmirrorable(va, va + 0x1000), None, "{va:#x}");
        }
        assert!(l.unmirrorable(0x10_0000, 0x10_1000).unwrap().contains("below"));
        assert!(l.unmirrorable(0x1_1C00_0000, 0x1_1C00_1000).unwrap().contains("host-only area"));
        // A 2 MiB leaf straddling the guest base touches the host area.
        assert!(l.unmirrorable(0x1_1FF0_0000, 0x1_2010_0000).is_some());
        assert!(l.unmirrorable(0x2_1C00_0000, 0x2_1C20_0000).unwrap().contains("server-RM"));
        assert_eq!(l.unmirrorable(0x2_3C00_0000, 0x2_3C20_0000), None);
        // The default layout (GR, CE) refuses nothing, as before.
        assert_eq!(Layout::default().unmirrorable(0x10_0000, 0x10_1000), None);
    }

    /// F2b: the fenced layout holds [0x1_2000_0000, +512 MiB); without a fence (Linux's default,
    /// the video layout) nothing changes. A size-0 global buffer answered `Some(0)` (inside a
    /// fence) is no set-B range; a promoted buffer with its own size stays set B (P24: the
    /// host maps the guest's PATCH at the guest's VA).
    #[test]
    fn a_fenced_global_buffer_is_no_set_b_range() {
        use crate::shadow::{Layout, FENCE_SIZE, GUEST_RM_VA_BASE};
        let f = Layout::fenced();
        assert_eq!(
            f.fence,
            Some((GUEST_RM_VA_BASE, GUEST_RM_VA_BASE + FENCE_SIZE))
        );
        assert_eq!(
            (f.va_base, f.alloc_flags()),
            (0, crate::shadow::VAS_ALLOW_ZERO_ADDRESS)
        );
        assert_eq!(
            (Layout::default().fence, Layout::video().fence),
            (None, None)
        );
        let chip = crate::fixture::rtx2070().chip.data();
        let mut reg = registry();
        reg.record_alloc(
            1,
            0x80,
            0x15,
            chip.channel.gpfifo,
            &channel_params(23, Some(4)),
        );
        let mut g = vec![0u8; v::PROMOTE_PARAMS_SIZE];
        g[v::PROMOTE_H_CHAN_CLIENT..][..4].copy_from_slice(&1u32.to_le_bytes());
        g[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&0x15u32.to_le_bytes());
        g[v::PROMOTE_ENTRY_COUNT..][..4].copy_from_slice(&2u32.to_le_bytes());
        // Entry 0: the PATCH buffer with its own size; entry 1: BUNDLE_CB by VA alone.
        let e0 = v::PROMOTE_ENTRIES;
        g[e0 + 8..][..8].copy_from_slice(&0x1_200e_4000u64.to_le_bytes());
        g[e0 + 16..][..8].copy_from_slice(&0x5000u64.to_le_bytes());
        g[e0 + 28..][..2].copy_from_slice(&2u16.to_le_bytes());
        let e1 = e0 + v::PROMOTE_ENTRY_SIZE;
        g[e1 + 8..][..8].copy_from_slice(&0x1_200e_9000u64.to_le_bytes());
        g[e1 + 28..][..2].copy_from_slice(&3u16.to_le_bytes());
        reg.record_promote_ctx(&g);
        let all = |_: &Channel| true;
        let fenced = |_: u16, _: u64| Some(0);
        assert_eq!(
            exclusions_among(&reg, all, fenced),
            vec![(0x1_200e_4000, 0x1_200e_9000)]
        );
        let unfenced = |_: u16, _: u64| Some(0x3000);
        assert_eq!(
            exclusions_among(&reg, all, unfenced),
            vec![
                (0x1_200e_4000, 0x1_200e_9000),
                (0x1_200e_9000, 0x1_200e_c000)
            ]
        );
    }

    /// Run P18: Windows maps pushbuffers below 1 MiB (the first Xid 31 was a host fetch at
    /// 0x13000). The default layout asks the RM to start its VAS at 0 (`ALLOW_ZERO_ADDRESS`,
    /// nvos.h BIT(8)) instead of the reserved 1 MiB; the video layout keeps its own base and
    /// asks for nothing, so its host area and server-RM range stay where they were.
    #[test]
    fn the_default_layout_starts_at_zero_and_the_video_layout_is_unchanged() {
        use crate::shadow::{Layout, RM_RESERVED_VA_BASE, VAS_ALLOW_ZERO_ADDRESS};
        assert_eq!(VAS_ALLOW_ZERO_ADDRESS, 0x100);
        assert_eq!(RM_RESERVED_VA_BASE, 0x10_0000);
        let d = Layout::default();
        assert_eq!((d.va_base, d.alloc_flags()), (0, VAS_ALLOW_ZERO_ADDRESS));
        for va in [0x0u64, 0x1_0000, 0x1_3000, 0xb_f000, 0xc_e000] {
            assert_eq!(d.unmirrorable(va, va + 0x1000), None, "{va:#x}");
        }
        assert_eq!(Layout::video().alloc_flags(), 0);
    }

    /// Run P18's first FE fault: `GET_MMU_FAULT_INFO` {addrHi, addrLo, faultType,
    /// faultString[32]} decoded for the fault diagnostic.
    #[test]
    fn the_mmu_fault_info_decodes_va_type_and_text() {
        let mut p = vec![0u8; MMU_FAULT_INFO_SIZE];
        p[0..4].copy_from_slice(&1u32.to_le_bytes());
        p[4..8].copy_from_slice(&0x200e_c000u32.to_le_bytes());
        p[8..12].copy_from_slice(&3u32.to_le_bytes());
        p[12..21].copy_from_slice(b"FAULT_PTE");
        assert_eq!(
            decode_fault_info(&p),
            (0x1_200e_c000, 3, "FAULT_PTE".into())
        );
        // A short answer decodes to zeros, never a panic.
        assert_eq!(decode_fault_info(&p[..3]), (0, 0, String::new()));
    }

    /// The RC of 2026-09-28: the host RC'd the eight channels of a TSG, and only the member
    /// that faulted (chid 20) held the fault record; the others read back empty. An empty
    /// record is looked up on the other members, the first that holds a fault is used, and
    /// the peers are read only while needed.
    #[test]
    fn an_empty_fault_record_is_taken_from_the_tsg_member_that_faulted() {
        let empty = vec![0u8; MMU_FAULT_INFO_SIZE];
        let mut fault = empty.clone();
        fault[0..4].copy_from_slice(&0x7794u32.to_le_bytes());
        fault[4..8].copy_from_slice(&0xb5c0_3000u32.to_le_bytes());
        fault[12..21].copy_from_slice(b"FAULT_PDE");
        assert!(fault_record_empty(0, &empty));
        assert!(!fault_record_empty(0, &fault));
        assert!(fault_record_empty(0x1f, &fault), "a failed read holds nothing");
        // FAULT_PDE is fault type 0: the string or the address tells a record from none.
        let mut typed = empty.clone();
        typed[12..21].copy_from_slice(b"FAULT_PDE");
        assert!(!fault_record_empty(0, &typed));

        let read = std::cell::Cell::new(0);
        let peers = |list: Vec<(u32, (u32, Vec<u8>))>| {
            let read = &read;
            list.into_iter().inspect(move |_| read.set(read.get() + 1))
        };
        // chid 15 empty; 16 empty, 20 holds it, 21 is never read.
        let f = first_fault(
            15,
            (0, empty.clone()),
            peers(vec![(16, (0, empty.clone())), (20, (0, fault.clone())), (21, (0, empty.clone()))]),
        );
        assert_eq!(f, TsgFault { own_status: 0, from: 20, status: 0, record: fault.clone() });
        assert_eq!(read.get(), 2);
        assert_eq!(decode_fault_info(&f.record).0, 0x7794_b5c0_3000);
        // The faulting member's own record: no peer is read.
        read.set(0);
        let f = first_fault(20, (0, fault.clone()), peers(vec![(15, (0, empty.clone()))]));
        assert_eq!((f.from, read.get()), (20, 0));
        // No member holds one: the own answer, as before.
        let f = first_fault(15, (0, empty.clone()), peers(vec![(16, (0, empty.clone()))]));
        assert_eq!(f, TsgFault { own_status: 0, from: 15, status: 0, record: empty.clone() });
        // A failed own read: a member's record, with the own status kept for the trace.
        let f = first_fault(15, (0x1f, empty.clone()), peers(vec![(20, (0, fault.clone()))]));
        assert_eq!((f.own_status, f.from, f.status), (0x1f, 20, 0));
    }

    /// The host RM's VA footprint of its global buffers (run P19, the sizes the host answered
    /// in the static info): 4 KiB pages below 64 KiB, 64 KiB pages below 2 MiB, 2 MiB pages
    /// above (mem_mgr.c `_memmgrPickDefaultGpuPageSize`).
    #[test]
    fn the_host_footprint_follows_the_rm_page_size() {
        assert_eq!(host_footprint(0x3000), 0x3000, "BUNDLE_CB");
        assert_eq!(host_footprint(0x464), 0x1000, "GFXP_CTRL_BLK");
        assert_eq!(host_footprint(0x1_0000), 0x1_0000, "FECS_EVENT");
        assert_eq!(host_footprint(0x2_0000), 0x2_0000, "PAGEPOOL_GLOBAL");
        assert_eq!(
            host_footprint(0x8_0000),
            0x8_0000,
            "RTV_CB_GLOBAL, PRIV_ACCESS_MAP"
        );
        assert_eq!(host_footprint(0x3f_3a00), 0x40_0000, "ATTRIBUTE_CB");
        assert_eq!(host_footprint(0x1_0001), 0x2_0000);
    }

    /// Run P19: the guest's BUNDLE_CB (the host's 0x3000) at 0x1200e9000, its semaphore page at
    /// 0x1200ec000. The exclusion stops there instead of at the 64 KiB extent; the guest's own
    /// pages inside the host's footprint never shrink it.
    #[test]
    fn a_global_exclusion_stops_at_the_guests_next_page() {
        const BASE: u64 = 0x1_200e_9000;
        let at = |pages: &'static [u64]| {
            move |lo: u64, hi: u64| pages.iter().copied().filter(|p| *p >= lo && *p < hi).min()
        };
        assert_eq!(
            global_exclusion(BASE, 0x3000, at(&[0x1_200e_c000, 0x1_200f_0000])),
            (0x1_200e_c000, GlobalClip::Cut { at: 0x1_200e_c000 })
        );
        assert_eq!(
            global_exclusion(BASE, 0x3000, at(&[])),
            (0x1_200f_9000, GlobalClip::Full),
            "no guest page: the 64 KiB extent, as before"
        );
        // Run P20: the guest maps its own copy of the buffer at the promoted VA; it never
        // shrinks the footprint, and the next page still cuts the extent.
        assert_eq!(
            global_exclusion(BASE, 0x3000, at(&[BASE, 0x1_200e_a000, 0x1_200e_c000])),
            (0x1_200e_c000, GlobalClip::Cut { at: 0x1_200e_c000 })
        );
        assert_eq!(
            global_exclusion(BASE, 0x3000, at(&[BASE])),
            (0x1_200f_9000, GlobalClip::Full)
        );
        // Linux's layout (display-e.md): the attribute CB at 0x120400000, 2 MiB pages; the
        // exclusion is the footprint whatever the guest maps past it.
        assert_eq!(
            global_exclusion(0x1_2040_0000, 0x3f_3a00, at(&[0x1_2080_0000])),
            (0x1_2080_0000, GlobalClip::Full)
        );
        // A huge-page footprint larger than the 64 KiB extent is never cut below it.
        assert_eq!(
            global_exclusion(0, 0x21_0000, at(&[])),
            (0x40_0000, GlobalClip::Full)
        );
    }

    /// The set B of a channel whose BUNDLE_CB the guest promoted by VA alone: with the clip
    /// the guest's page right after it is set A, the host's footprint stays set B.
    #[test]
    fn exclusions_use_the_clipped_global_extent() {
        let chip = crate::fixture::rtx2070().chip.data();
        let mut reg = registry();
        reg.record_alloc(
            1,
            0x80,
            0x15,
            chip.channel.gpfifo,
            &channel_params(23, Some(4)),
        );
        let mut g = vec![0u8; v::PROMOTE_PARAMS_SIZE];
        g[v::PROMOTE_H_CHAN_CLIENT..][..4].copy_from_slice(&1u32.to_le_bytes());
        g[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&0x15u32.to_le_bytes());
        g[v::PROMOTE_ENTRY_COUNT..][..4].copy_from_slice(&1u32.to_le_bytes());
        g[v::PROMOTE_ENTRIES + 8..][..8].copy_from_slice(&0x1_200e_9000u64.to_le_bytes());
        g[v::PROMOTE_ENTRIES + 28..][..2].copy_from_slice(&3u16.to_le_bytes());
        reg.record_promote_ctx(&g);
        let all = |_: &Channel| true;
        let guest =
            |lo: u64, hi: u64| (lo <= 0x1_200e_c000 && 0x1_200e_c000 < hi).then_some(0x1_200e_c000);
        let clipped =
            |id: u16, va: u64| (id == 3).then(|| global_exclusion(va, 0x3000, guest).0 - va);
        assert_eq!(
            exclusions_among(&reg, all, clipped),
            vec![(0x1_200e_9000, 0x1_200e_c000)]
        );
    }

    /// The host's global context sizes by FIFO engine (the RTX 2070's answers, the set-B
    /// WARN lines of the 2026-09-28 latency run).
    fn rtx2070_ctx_size(engine: u32) -> (u32, u32) {
        let size = match engine {
            0x11 => 0x3000,
            0x0d => 0x2_0000,
            0x13 => 0x3f_3a00,
            0x14 => 0x8_0000,
            0x17 => 0x1_0000,
            0x18 => 0x8_0000,
            _ => return (sys::NV_ERR_NOT_SUPPORTED, 0),
        };
        (sys::NV_OK, size)
    }

    /// A guest GMMU v2 tree in an FB store (ptcache.rs geometry), mapped page by page.
    struct GuestTree {
        fb: FakeFb,
        root: u64,
        next_table: u64,
        next_pa: u64,
        tables: BTreeMap<(u8, u64), u64>,
    }

    impl GuestTree {
        fn new(root: u64) -> Self {
            GuestTree {
                fb: FakeFb::new(0x2_0000_0000 + (64 << 20)),
                root,
                next_table: root + 0x1000,
                next_pa: 0x1_4000_0000,
                tables: BTreeMap::new(),
            }
        }

        /// The table under `key` (level 2..0 by VA, 8 a unit's big-page table, 9 its
        /// small-page table), and whether it is new.
        fn table(&mut self, key: (u8, u64)) -> (u64, bool) {
            if let Some(t) = self.tables.get(&key) {
                return (*t, false);
            }
            let t = self.next_table;
            self.next_table += 0x1000;
            self.tables.insert(key, t);
            (t, true)
        }

        /// Map `n` pages of `size` (4 KiB, 64 KiB or 2 MiB) from `va` on fresh memory.
        fn map(&mut self, va: u64, size: u64, n: u64) {
            for i in 0..n {
                self.next_pa = self.next_pa.next_multiple_of(size);
                let pa = self.next_pa;
                self.next_pa += size;
                self.map_page(va + i * size, pa, size);
            }
        }

        fn map_page(&mut self, va: u64, pa: u64, size: u64) {
            let pde = |a: u64| ((a >> 12) << 8) | 0b010;
            let pte = |a: u64| ((a >> 12) << 8) | 1 | (0x06u64 << 56);
            let mut table = self.root;
            for (level, shift) in [(3u8, 47u32), (2, 38), (1, 29)] {
                let i = (va >> shift) & if level == 3 { 3 } else { 0x1ff };
                let (child, fresh) = self.table((level - 1, va & !((1u64 << shift) - 1)));
                if fresh {
                    self.fb.write_u64(table + 8 * i, pde(child));
                }
                table = child;
            }
            let dual = table + 16 * ((va >> 21) & 0xff);
            let unit = va & !(crate::ptcache::UNIT - 1);
            match size {
                0x20_0000 => {
                    self.fb.write_u64(dual, pte(pa));
                }
                0x1_0000 => {
                    let (bt, fresh) = self.table((8, unit));
                    if fresh {
                        self.fb.write_u64(dual, ((bt >> 8) << 4) | 0b010);
                    }
                    self.fb.write_u64(bt + 8 * ((va >> 16) & 0x1f), pte(pa));
                }
                _ => {
                    let (st, fresh) = self.table((9, unit));
                    if fresh {
                        self.fb.write_u64(dual + 8, pde(st));
                    }
                    self.fb.write_u64(st + 8 * ((va >> 12) & 0x1ff), pte(pa));
                }
            }
        }
    }

    /// A CUDA context's global GR buffers as nvprobe 4 promotes them on every one of its
    /// eight GR channels (the 2026-09-28 latency trace): `(buffer id, VA)`, all of size 0.
    const CUDA_GLOBALS: [(u16, u64); 10] = [
        (5, 0x2_03c0_0000),
        (11, 0x2_0400_0000),
        (9, 0x2_0408_0000),
        (10, 0x2_0409_0000),
        (4, 0x2_0411_0000),
        (6, 0x2_0413_0000),
        (0, 0x2_041b_0000),
        (3, 0x2_0429_e000),
        (1, 0x2_042a_1000),
        (2, 0x2_042a_a000),
    ];

    /// The guest's registry, FB store and cached walk of a lean compute VM running
    /// nvprobe 4, with a desktop beside it: CUDA's context (root `root`) with one TSG of
    /// eight GR channels, each promoting [`CUDA_GLOBALS`], and six copy channels; UVM with
    /// ten copy channels on its own root and a duplicate of CUDA's VA space; three desktop
    /// clients with a GR and six copy channels each on their own roots. The guest maps the
    /// global buffers at their VAs (2 MiB, 64 KiB and 4 KiB pages) and about 2300 pages of
    /// CUDA's own memory: about 2600 leaves, as the latency run's shadow held.
    struct DoorbellWorld {
        reg: Registry,
        tree: GuestTree,
        root: u64,
    }

    fn promote_params(h_client: u32, h_object: u32, entries: &[(u16, u64, u64, u64, bool)]) -> Vec<u8> {
        let mut g = vec![0u8; v::PROMOTE_PARAMS_SIZE];
        g[v::PROMOTE_H_CHAN_CLIENT..][..4].copy_from_slice(&h_client.to_le_bytes());
        g[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&h_object.to_le_bytes());
        g[v::PROMOTE_ENTRY_COUNT..][..4].copy_from_slice(&(entries.len() as u32).to_le_bytes());
        for (i, &(id, pa, va, size, nonmapped)) in entries.iter().enumerate() {
            let o = v::PROMOTE_ENTRIES + v::PROMOTE_ENTRY_SIZE * i;
            g[o..][..8].copy_from_slice(&pa.to_le_bytes());
            g[o + 8..][..8].copy_from_slice(&va.to_le_bytes());
            g[o + 16..][..8].copy_from_slice(&size.to_le_bytes());
            g[o + 28..][..2].copy_from_slice(&id.to_le_bytes());
            g[o + 30] = u8::from(nonmapped);
            g[o + 31] = u8::from(nonmapped);
        }
        g
    }

    fn doorbell_world() -> DoorbellWorld {
        let chip = crate::fixture::rtx2070().chip.data();
        let root = 0x1_0000_0000u64;
        let mut reg = registry();
        let vas = |reg: &mut Registry, h_client: u32, h_vas: u32, root: u64| {
            reg.record_alloc(h_client, 0x1, h_vas, v::FERMI_VASPACE_A, &[0u8; 56]);
            let mut pd = vec![0u8; 32];
            pd[v::SET_PD_PHYS_ADDRESS..][..8].copy_from_slice(&root.to_le_bytes());
            pd[v::SET_PD_NUM_ENTRIES..][..4].copy_from_slice(&4u32.to_le_bytes());
            pd[v::SET_PD_H_VASPACE..][..4].copy_from_slice(&h_vas.to_le_bytes());
            reg.record_set_page_directory(h_client, &pd);
        };
        let mut chid = 0u32;
        let mut channel = |reg: &mut Registry, h_client: u32, h_parent: u32, h_object: u32, h_vas: u32| {
            let mut cp = channel_params(chid, None);
            let o = <Abi as nvrm_sys::RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_hVASpace;
            cp[o..o + 4].copy_from_slice(&h_vas.to_le_bytes());
            reg.record_alloc(h_client, h_parent, h_object, chip.channel.gpfifo, &cp);
            chid += 1;
        };
        let globals = |base: u64| -> Vec<(u16, u64, u64, u64, bool)> {
            CUDA_GLOBALS.iter().map(|&(id, va)| (id, 0, va - 0x2_0000_0000 + base, 0, false)).collect()
        };
        // CUDA: one TSG of eight GR channels on its VAS, six copy channels.
        let cuda = 0xc1d0_0373u32;
        vas(&mut reg, cuda, 0x5c00_0003, root);
        reg.record_alloc(cuda, 0x1, 0x5c00_0010, v::KEPLER_CHANNEL_GROUP_A, &tsg_params(Some(26)));
        for i in 0..8u32 {
            let h = 0x5c00_0019 + 4 * i;
            channel(&mut reg, cuda, 0x5c00_0010, h, 0x5c00_0003);
            if i == 0 {
                reg.record_promote_ctx(&promote_params(cuda, h, &[
                    (0, 0x1_e6b3_6000, 0, 0xe_e000, true),
                    (1, 0x1_e727_1000, 0, 0x8c00, true),
                    (2, 0x1_e727_a000, 0, 0x5000, true),
                ]));
            }
            reg.record_promote_ctx(&promote_params(cuda, h, &globals(0x2_0000_0000)));
        }
        for i in 0..6u32 {
            channel(&mut reg, cuda, 0x1, 0x5c00_0100 + i, 0x5c00_0003);
        }
        // UVM: ten copy channels on its own root, and CUDA's VA space duplicated.
        let uvm = 0xc1e0_0001u32;
        vas(&mut reg, uvm, 0x3, 0x1_0800_0000);
        assert!(reg.record_dup((uvm, 0xbeef_0001), (cuda, 0x5c00_0003)).is_some());
        for i in 0..10u32 {
            channel(&mut reg, uvm, 0x1, 0x100 + i, 0x3);
        }
        // The desktop: three clients with a GR channel (its own globals) and six copy channels.
        for d in 0..3u32 {
            let c = 0xc1f0_0001 + d;
            vas(&mut reg, c, 0x3, 0x1_0900_0000 + u64::from(d) * 0x10_0000);
            channel(&mut reg, c, 0x1, 0x200, 0x3);
            reg.record_promote_ctx(&promote_params(c, 0x200, &globals(0x1_2000_0000)));
            for i in 0..6u32 {
                channel(&mut reg, c, 0x1, 0x201 + i, 0x3);
            }
        }
        assert_eq!(reg.channels().count(), 45);
        // The guest's pages: the global buffers at their VAs, and CUDA's own memory.
        let mut tree = GuestTree::new(root);
        let g = |id: u16| CUDA_GLOBALS.iter().find(|(i, _)| *i == id).unwrap().1;
        tree.map(g(5), 0x20_0000, 2);
        tree.map(g(11), 0x1_0000, 8);
        tree.map(g(9), 0x1_0000, 1);
        tree.map(g(10), 0x1_0000, 8);
        tree.map(g(4), 0x1_0000, 2);
        tree.map(g(6), 0x1_0000, 8);
        tree.map(g(0), 0x1000, 238);
        tree.map(g(3), 0x1000, 3);
        tree.map(g(1), 0x1000, 9);
        tree.map(g(2), 0x1000, 5);
        tree.map(0x2_0020_0000, 0x1000, 2300);
        DoorbellWorld { reg, tree, root }
    }

    /// Mean nanoseconds of `f` over `n` calls, the best of five rounds.
    fn ns_per_call(n: u32, mut f: impl FnMut()) -> f64 {
        (0..5)
            .map(|_| {
                let t = Instant::now();
                for _ in 0..n {
                    f();
                }
                t.elapsed().as_nanos() as f64 / f64::from(n)
            })
            .fold(f64::INFINITY, f64::min)
    }

    /// `shadow_resync` of one root without the worker: refresh set B, plan, and when the plan
    /// changed set B (a first walk clipping a global buffer), plan again. `force` computes set
    /// B at both refreshes, as before the stamp. Whether anything was planned.
    fn resync_root(
        ext: &GlobalExtents,
        sp: &mut ShadowSpace,
        reg: &Registry,
        fb: &FakeFb,
        mem: &GuestMemory,
        descs: &[(u32, u64, u64)],
        force: bool,
    ) -> bool {
        let root = sp.front.root;
        let query = |_: u16, engine: u32| rtx2070_ctx_size(engine);
        let mut planned = false;
        if force {
            sp.set_b_stamp = None;
        }
        ext.refresh(reg, root, fb, sp, &query);
        planned |= sp.front.plan(fb, mem, 0x1_0000_0000, 0x2_0000_0000, 1, &sp.exclude, descs).is_some();
        if force {
            sp.set_b_stamp = None;
        }
        if ext.refresh(reg, root, fb, sp, &query).1 {
            planned |= sp.front.plan(fb, mem, 0x1_0000_0000, 0x2_0000_0000, 1, &sp.exclude, descs).is_some();
        }
        planned
    }

    /// The set-B stamp ([`SetBStamp`]): a doorbell on a root where nothing changed reuses set
    /// B and computes nothing; every input that can change set B makes the next refresh
    /// compute it: a promotion, a channel free, a guest page mapped inside a global
    /// buffer's extent (P19's clip, found by the plan's walk), a hold and its release at the
    /// host TSG free, a private range, a host size not answered for good. A free of an
    /// object set B does not depend on changes nothing.
    #[test]
    fn set_b_is_reused_until_an_input_changes() {
        let DoorbellWorld { mut reg, mut tree, root } = doorbell_world();
        let mem = GuestMemory::new();
        let ext = GlobalExtents::default();
        let mut sp = ShadowSpace::new(crate::shadow::ShadowFront::new(root), 0);
        let cuda = 0xc1d0_0373u32;
        // The build: the first refresh has no walk yet, the plan's walk clips the global
        // buffers, the second refresh computes again with it.
        assert!(resync_root(&ext, &mut sp, &reg, &tree.fb, &mem, &[], false));
        assert_eq!(ext.computed.get(), 2);
        let computed = |n: u64| n + 2;
        // Clean doorbells: nothing computed.
        for _ in 0..3 {
            assert!(!resync_root(&ext, &mut sp, &reg, &tree.fb, &mem, &[], false));
        }
        assert_eq!((ext.computed.get(), ext.reused.get()), (computed(0), 6));
        let set_b = sp.exclude.clone();
        // A free of an object set B does not depend on (guest memory) keeps the stamp.
        let gen = reg.set_b_generation();
        reg.record_free(cuda, 0x5c00_9999);
        reg.channel_mut(0).unwrap().mirrored = true;
        assert_eq!(reg.set_b_generation(), gen);
        assert!(!resync_root(&ext, &mut sp, &reg, &tree.fb, &mem, &[], false));
        assert_eq!(ext.computed.get(), computed(0));
        // A promotion: one of CUDA's copy channels promotes a context of two pages.
        let ctx = (0x2_1000_0000, 0x2_1000_2000);
        reg.record_promote_ctx(&promote_params(cuda, 0x5c00_0100, &[(13, 0, ctx.0, 0x2000, false)]));
        resync_root(&ext, &mut sp, &reg, &tree.fb, &mem, &[], false);
        assert_eq!(ext.computed.get(), computed(1));
        assert!(sp.exclude.contains(&ctx), "{:x?}", sp.exclude);
        // Its channel's free gives it back to set A.
        reg.record_free(cuda, 0x5c00_0100);
        resync_root(&ext, &mut sp, &reg, &tree.fb, &mem, &[], false);
        assert_eq!(ext.computed.get(), computed(2));
        assert_eq!(sp.exclude, set_b);
        // A global buffer promoted by VA alone where the guest maps nothing: the whole
        // 64 KiB extent of the 12 KiB BUNDLE_CB (run P19's numbers).
        let bundle = 0x2_2000_9000u64;
        reg.record_promote_ctx(&promote_params(cuda, 0x5c00_0101, &[(3, 0, bundle, 0, false)]));
        resync_root(&ext, &mut sp, &reg, &tree.fb, &mem, &[], false);
        assert_eq!(ext.computed.get(), computed(3));
        assert!(sp.exclude.contains(&(bundle, bundle + 0x1_0000)), "{:x?}", sp.exclude);
        assert!(!resync_root(&ext, &mut sp, &reg, &tree.fb, &mem, &[], false));
        assert_eq!(ext.computed.get(), computed(3));
        // The guest maps its semaphore page right after the footprint: the first refresh
        // still finds the old walk, the plan walks the new page and the second refresh cuts
        // the exclusion there; the root is planned again with it, and the WARN comes once.
        tree.map(bundle + 0x3000, 0x1000, 1);
        let gen = reg.set_b_generation();
        assert!(resync_root(&ext, &mut sp, &reg, &tree.fb, &mem, &[], false));
        assert_eq!(reg.set_b_generation(), gen, "no registry change");
        assert_eq!(ext.computed.get(), computed(4));
        assert!(sp.exclude.contains(&(bundle, bundle + 0x3000)), "{:x?}", sp.exclude);
        assert!(!sp.exclude.contains(&(bundle, bundle + 0x1_0000)));
        assert!(ext.take_notes().is_empty(), "handed out with the refresh");
        assert!(!resync_root(&ext, &mut sp, &reg, &tree.fb, &mem, &[], false));
        assert_eq!(ext.computed.get(), computed(4));
        let set_b = sp.exclude.clone();
        // A hold for a host TSG not yet freed, and its release at that free.
        let key = TsgKey::Handle(cuda, 0x5c00_0010);
        let held = (0x2_3000_0000, 0x2_3000_4000);
        sp.hold(key, vec![held]);
        resync_root(&ext, &mut sp, &reg, &tree.fb, &mem, &[], false);
        assert_eq!(ext.computed.get(), computed(5));
        assert!(sp.exclude.contains(&held));
        assert!(sp.release(key).is_some());
        assert!(sp.release(key).is_none());
        resync_root(&ext, &mut sp, &reg, &tree.fb, &mem, &[], false);
        assert_eq!(ext.computed.get(), computed(6));
        assert_eq!(sp.exclude, set_b);
        // A channel of another root changes the registry: set B is computed again (and
        // stays the same).
        reg.record_free(0xc1f0_0001, 0x206);
        assert!(!resync_root(&ext, &mut sp, &reg, &tree.fb, &mem, &[], false));
        assert_eq!(ext.computed.get(), computed(7));
        assert_eq!(sp.exclude, set_b);
        // A host size not answered for good (a transient error) is asked again at the next
        // refresh, and once answered the stamp holds.
        let fresh = GlobalExtents::default();
        let mut sp2 = ShadowSpace::new(crate::shadow::ShadowFront::new(root), 0);
        let (asked, failed) = (Cell::new(0u32), Cell::new(false));
        let flaky = |_: u16, engine: u32| {
            asked.set(asked.get() + 1);
            if engine == 0x11 && !failed.replace(true) {
                (sys::NV_ERR_TIMEOUT, 0)
            } else {
                rtx2070_ctx_size(engine)
            }
        };
        fresh.refresh(&reg, root, &tree.fb, &mut sp2, &flaky);
        assert!(sp2.set_b_stamp.is_none(), "not stamped after a transient failure");
        fresh.refresh(&reg, root, &tree.fb, &mut sp2, &flaky);
        assert!(sp2.set_b_stamp.is_some());
        assert_eq!(sp2.exclude, sp.exclude);
        let n = asked.get();
        fresh.refresh(&reg, root, &tree.fb, &mut sp2, &flaky);
        assert_eq!((asked.get(), fresh.computed.get(), fresh.reused.get()), (n, 2, 1));
    }

    /// Run P28 (the merge with planfast): set B is computed by [`GlobalExtents`], and the F2b
    /// rule must hold there too. A size-0 global inside a fenced root's fence is plain set A;
    /// a PA-promoted buffer inside it and a size-0 global outside it stay set B. Without it
    /// the shadow left the guest's pages in the fence unmapped (46 refusals, FE
    /// FAULT_PRIV_VIOLATION writes at 0x1_200f_c000 and up).
    #[test]
    fn a_fenced_root_computes_set_b_without_fenced_globals() {
        let DoorbellWorld { mut reg, tree, root } = doorbell_world();
        let cuda = 0xc1d0_0373u32;
        let (fenced_global, patch, outside) =
            (0x1_200e_9000u64, 0x1_200e_4000u64, 0x2_2000_9000u64);
        reg.record_promote_ctx(&promote_params(
            cuda,
            0x5c00_0101,
            &[
                (3, 0, fenced_global, 0, false),
                (2, 0x40_0000, patch, 0x5000, false),
                (4, 0, outside, 0, false),
            ],
        ));
        let query = |_: u16, engine: u32| rtx2070_ctx_size(engine);
        let set_b = |layout: crate::shadow::Layout| {
            let ext = GlobalExtents::default();
            let mut front = crate::shadow::ShadowFront::new(root);
            front.layout = layout;
            let mut sp = ShadowSpace::new(front, 0);
            ext.refresh(&reg, root, &tree.fb, &mut sp, &query);
            let fence = sp.front.layout.fence;
            let direct = ext.set_b(&reg, root, Some(&tree.fb), Some(&sp), fence, &query);
            assert_eq!(direct, sp.exclude, "refresh and set_b agree");
            sp.exclude
        };
        let starts = |x: &[(u64, u64)]| x.iter().map(|r| r.0).collect::<Vec<_>>();
        let fenced = set_b(crate::shadow::Layout::fenced());
        assert!(!starts(&fenced).contains(&fenced_global), "{fenced:x?}");
        assert!(fenced.contains(&(patch, patch + 0x5000)), "{fenced:x?}");
        assert!(starts(&fenced).contains(&outside), "{fenced:x?}");
        let plain = set_b(crate::shadow::Layout::default());
        assert!(starts(&plain).contains(&fenced_global), "{plain:x?}");
    }

    /// Micro-benchmark (`cargo test --release -p vfio-user-nvgpu --lib bench_set_b --
    /// --ignored --nocapture`): the plan step of a clean doorbell on the CUDA root of
    /// [`doorbell_world`] (`shadow_resync` without the worker), before and with the set-B
    /// stamp, and where the time goes.
    #[test]
    #[ignore]
    fn bench_set_b_clean_doorbell() {
        let w = doorbell_world();
        let (reg, fb, root) = (&w.reg, &w.tree.fb, w.root);
        let mem = GuestMemory::new();
        let descs: Vec<(u32, u64, u64)> =
            (0..53).map(|i| (i, 0x7ffa_c000_0000 + u64::from(i) * (1 << 30), 1 << 30)).collect();
        let ext = GlobalExtents::default();
        let mut sp = ShadowSpace::new(crate::shadow::ShadowFront::new(root), 0);
        // The build and the first doorbell: the first plan's walk clips the global buffers.
        assert!(resync_root(&ext, &mut sp, reg, fb, &mem, &descs, false));
        assert!(!resync_root(&ext, &mut sp, reg, fb, &mem, &descs, false), "a clean doorbell");
        eprintln!("cached walk: {} leaves", fb.with_walk(Target::Fb(root), |c| c.leaf_count()).unwrap());
        eprintln!("set B of the CUDA root: {} range(s)", sp.exclude.len());
        let n = 2000;
        // cb4974fc: every refresh walks every channel and sizes and clips all 80 promoted
        // global buffers of the eight GR channels (`Mirror::global_extent` per entry).
        let old_ext = GlobalExtents::default();
        let query = |_: u16, engine: u32| rtx2070_ctx_size(engine);
        let old_set_b = || {
            let live = exclusions_of(reg, root, |id, va| old_ext.extent(Some(fb), root, id, va, &query));
            let mut out = set_b_with_held(live, &sp.held);
            out.sort_unstable();
            out.dedup();
            out
        };
        assert_eq!(old_set_b(), sp.exclude, "the same set B");
        let mut front = crate::shadow::ShadowFront::new(root);
        front.plan(fb, &mem, 0x1_0000_0000, 0x2_0000_0000, 1, &sp.exclude, &descs).unwrap();
        let mut exclude = sp.exclude.clone();
        let old = ns_per_call(n, || {
            let descs = descs.clone();
            for _ in 0..2 {
                let now = old_set_b();
                if exclude != now {
                    exclude = now;
                }
                assert!(front.plan(fb, &mem, 0x1_0000_0000, 0x2_0000_0000, 1, &exclude, &descs).is_none());
            }
        });
        let old_one = ns_per_call(n, || {
            std::hint::black_box(old_set_b());
        });
        let forced = ns_per_call(n, || {
            let descs = descs.clone();
            assert!(!resync_root(&ext, &mut sp, reg, fb, &mem, &descs, true));
        });
        let clean = ns_per_call(n, || {
            let descs = descs.clone();
            assert!(!resync_root(&ext, &mut sp, reg, fb, &mem, &descs, false));
        });
        let reg_walk = ns_per_call(n, || {
            std::hint::black_box(exclusions_of(reg, root, |_, _| Some(0x1000)));
        });
        let filter_only = ns_per_call(n, || {
            std::hint::black_box(
                reg.channels()
                    .filter(|ch| reg.vas_key_of(ch).and_then(|k| reg.vas_root(k)) == Some(Target::Fb(root)))
                    .count(),
            );
        });
        let extents = ns_per_call(n, || {
            for _ in 0..8 {
                for &(id, va) in &CUDA_GLOBALS {
                    std::hint::black_box(old_ext.extent(Some(fb), root, id, va, &query));
                }
            }
        });
        let walk_queries = ns_per_call(n, || {
            for _ in 0..8 {
                for &(id, va) in &CUDA_GLOBALS {
                    if let Some(raw) = old_ext.size(id, &query) {
                        std::hint::black_box(global_exclusion(va, raw, |lo, hi| guest_first_mapped(fb, root, lo, hi)));
                    }
                }
            }
        });
        let plan_clean = ns_per_call(n, || {
            assert!(sp.front.plan(fb, &mem, 0x1_0000_0000, 0x2_0000_0000, 1, &sp.exclude, &descs).is_none());
        });
        let stamp = ns_per_call(n, || {
            std::hint::black_box(ext.stamp(reg, root, fb));
        });
        eprintln!("clean doorbell, plan step without the lazy flush (2 set-B refreshes + plan):");
        eprintln!("  cb4974fc:                          {old:>7.0} ns");
        eprintln!("  set B computed at both refreshes:  {forced:>7.0} ns (each (buffer, VA) sized and clipped once)");
        eprintln!("  with the stamp (clean root):       {clean:>7.0} ns (stamp {stamp:.0} ns, plan's clean check {plan_clean:.0} ns)");
        eprintln!("where cb4974fc's time goes, per refresh ({old_one:.0} ns):");
        eprintln!("  registry walk, 45 channels, trivial extents: {reg_walk:.0} ns (root filter alone {filter_only:.0} ns)");
        eprintln!("  80 global extents (8 GR channels x 10 buffers): {extents:.0} ns, of which the walk queries {walk_queries:.0} ns");
    }

    /// The fallback: the host object is kept only when its context took the host's first
    /// fit and that first fit lies in the host-only area.
    #[test]
    fn a_host_context_outside_the_host_area_is_refused() {
        let l = crate::shadow::Layout::video();
        let ok = context_verdict(&l, Ok(0x1_1C00_0000), Ok(0x1_1C00_1000)).unwrap();
        assert!(ok.contains("its context at 0x11c000000"));
        // The first fit in the guest's range (the default layout's 0x120400000).
        let e = context_verdict(&l, Ok(0x1_2040_0000), Ok(0x1_2040_1000)).unwrap_err();
        assert!(e.contains("outside the host-only area"));
        // The first fit still free after the object: the context went elsewhere.
        let e = context_verdict(&l, Ok(0x1_1C00_0000), Ok(0x1_1C00_0000)).unwrap_err();
        assert!(e.contains("did not map its context"));
        assert!(context_verdict(&l, Err(0x51), Ok(1)).unwrap_err().contains("before the object was refused 0x51"));
        assert!(context_verdict(&l, Ok(0x1_1C00_0000), Err(0x51)).unwrap_err().contains("after the object"));
        // The default layout has no host area: always refused.
        assert!(context_verdict(&crate::shadow::Layout::default(), Ok(0x1_2002_d000), Ok(0x1_2002_e000)).is_err());
    }

    #[test]
    fn owned_gfxp_bind_contains_only_host_identity_and_private_addresses() {
        let addresses = [0x1000, 0x22000, 0x333000, 0x4444_0000, 0x5555_5000];
        let p = gfxp_bind_params(0x1234, 0x5678, 1, &addresses);
        let word = |o| u32::from_le_bytes(p[o..o + 4].try_into().unwrap());
        assert_eq!((word(0), word(4), word(8), word(88)), (2, 0x1234, 0x5678, 1));
        for (i, address) in addresses.iter().enumerate() {
            assert_eq!(u64::from_le_bytes(p[16 + i * 8..24 + i * 8].try_into().unwrap()), *address);
        }
        assert!(p[56..88].iter().all(|byte| *byte == 0), "unused and CPU-pointer fields stay zero");
        assert!(p[92..].iter().all(|byte| *byte == 0), "CILP and routing fields stay zero");
    }

    #[test]
    fn owned_compute_cta_bind_is_pointer_free_and_compute_only() {
        let p = compute_bind_params(0x1234, 0x5678, COMPUTE_CTA);
        let word = |o| u32::from_le_bytes(p[o..o + 4].try_into().unwrap());
        assert_eq!((word(0), word(4), word(8)), (1, 0x1234, 0x5678));
        assert_eq!((word(88), word(92)), (0, 1), "graphics is ignored and compute selects CTA");
        assert!(p[12..16].iter().all(|byte| *byte == 0), "ABI padding stays zero");
        assert!(p[16..88].iter().all(|byte| *byte == 0), "all nine guest pointer positions stay zero");
        assert!(p[96..112].iter().all(|byte| *byte == 0), "route stays zero");
        // Run P22: CUDA's context asks for CILP (flags 1, cilpPreemptMode 2, no pointers).
        let p = compute_bind_params(0x1234, 0x5678, COMPUTE_CILP);
        assert_eq!((word_of(&p, 0), word_of(&p, 88), word_of(&p, 92)), (1, 0, 2));
    }

    fn word_of(p: &[u8], o: usize) -> u32 {
        u32::from_le_bytes(p[o..o + 4].try_into().unwrap())
    }
}

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
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
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use nvrm_abi::sys;
use nvrm_client::mem::CpuMap;

use crate::channel::{Channel, GpuMem, Registry};
use crate::chip::ChipData;
use crate::fb::{FakeFb, FbLayout};
use crate::fb::Target;
use crate::guestmem::GuestMemory;
use crate::hostmem::{HostGfxp, HostMemory, HostUserd, OsEvent};
use crate::intr::IntrShared;
use crate::invalidate::Invalidator;
use crate::ver::active as v;

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

/// `NVA06F_CTRL_CMD_BIND` {engineType} (ctrla06f.h): a host channel needs an engine before
/// `GPFIFO_SCHEDULE` (docs/caraxes/host-exec.md §1: NV_ERR_INVALID_STATE otherwise).
const CMD_CHANNEL_BIND: u32 = 0xA06F_0104;

/// The promoted context ranges `[va, va + size)` of a channel (GR set B, a falcon
/// engine's context), which the shadow keeps out of the guest's set A. A global GR
/// buffer promoted by VA alone has size 0; `global_size` gives its extent.
fn promoted_ranges(
    reg: &Registry,
    ch: &Channel,
    global_size: impl Fn(u16) -> Option<u64>,
) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    if let Some(m) = reg.promoted((ch.h_client, ch.h_object)) {
        for e in m.values() {
            if let Some(v) = e.virt {
                let size = if e.size == 0 {
                    global_size(e.buffer_id).unwrap_or(0x1000)
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
    global_size: impl Fn(u16) -> Option<u64>,
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
    global_size: impl Fn(u16) -> Option<u64>,
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
/// NO_MEMORY). `root_of` names the shadow root of a channel (`None`: no shadow).
pub fn held_at_free(
    reg: &Registry,
    freed: impl Fn(&Channel) -> bool,
    root_of: impl Fn(&Channel) -> Option<u64>,
    global_size: impl Fn(u16) -> Option<u64>,
) -> Vec<HeldSetB> {
    reg.channels()
        .filter(|ch| freed(ch))
        .filter_map(|ch| {
            let root = root_of(ch)?;
            let ranges = promoted_ranges(reg, ch, &global_size);
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
const CMD_GET_ENGINE_CONTEXT_PROPERTIES: u32 = 0x0080_1707;
const CMD_BIND_CTXSW_PREEMPTION_MODE: u32 = 0x2080_1211;
const CMD_GET_CTXSW_PREEMPTION_MODE: u32 = 0x2080_1235;
const GFXP_BUFFER_CAP: u64 = 64 << 20;
const GFXP_TSG_CAP: usize = 8;

struct OwnedGfxpBuffer {
    memory: Option<HostGfxp>,
    virtual_handle: Option<u32>,
    va: u64,
    len: u64,
    mapped: bool,
}

struct OwnedGfxp {
    chid: u32,
    generation: u64,
    host_channel: u32,
    root: u64,
    bind_attempted: bool,
    bound: bool,
    buffers: Vec<OwnedGfxpBuffer>,
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

fn compute_cta_bind_params(root: u32, tsg: u32) -> [u8; 112] {
    let mut p = [0u8; 112];
    p[0..4].copy_from_slice(&1u32.to_le_bytes());
    p[4..8].copy_from_slice(&root.to_le_bytes());
    p[8..12].copy_from_slice(&tsg.to_le_bytes());
    p[92..96].copy_from_slice(&1u32.to_le_bytes());
    p
}

fn bind_compute_cta(host: &HostMemory, tsg: u32) -> u32 {
    let mut p = compute_cta_bind_params(host.root(), tsg);
    host.control(host.subdevice(), CMD_BIND_CTXSW_PREEMPTION_MODE, &mut p)
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
    /// Set-B ranges of guest channels already freed, held by the host TSG that still
    /// stands for their GR context ([`held_at_free`]); released at that TSG's host free.
    held: BTreeMap<TsgKey, Vec<(u64, u64)>>,
    /// Host-private ordinary-GFXP buffers mapped in this VAS. These ranges stay out of
    /// the guest shadow even after a partial allocation or cleanup failure.
    private: BTreeMap<TsgKey, Vec<(u64, u64)>>,
    chids: std::collections::BTreeSet<u32>,
    tsgs: std::collections::BTreeSet<TsgKey>,
}

struct HostTsg {
    handle: u32,
    vas: (u32, u32),
    engine: u32,
    /// The guest object the host TSG stands for: the guest TSG `(hClient, hTsg)`, or the
    /// channel itself for a bare channel's own TSG.
    guest: (u32, u32),
    gfxp: Option<OwnedGfxp>,
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
}

impl ShadowUserd {
    fn publish(&mut self, gm: &GpuMem) -> Result<u32, String> {
        let cpu = crate::userd::CpuState::read(self.entries, |off| {
            gm.read_u32(Target::Sys(self.guest.checked_add(off)?))
        }).ok_or("unreadable or invalid guest USERD submission")?;
        cpu.publish(|off, value| self.storage.write_u32(off, value));
        self.storage.flush();
        self.published_put = Some(cpu.gp_put());
        Ok(cpu.gp_put())
    }

    fn harvest(&self, gm: &mut GpuMem) {
        if let Some(state) = crate::userd::GpuState::read(|off| Some(self.storage.read_u32(off))) {
            state.publish(|off, value| {
                if let Some(at) = self.guest.checked_add(off) {
                    gm.write(Target::Sys(at), &value.to_le_bytes());
                }
            });
        }
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
    /// Private low-address USERD for a guest sysmem USERD on a high-IOVA host.
    userd: Option<ShadowUserd>,
    context_ready: bool,
    /// The host error notifier when the guest's could not be mirrored (a guest notifier in
    /// VRAM): the device's own page, read by the RC scan (`Mirror::rc_notifier`).
    rc_page: Option<crate::hostmem::DevicePage>,
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
        let size = (status == sys::NV_OK && raw != 0).then(|| (u64::from(raw) + 0xFFFF) & !0xFFFF);
        if status == sys::NV_OK || status == sys::NV_ERR_NOT_SUPPORTED {
            self.by_engine.insert(engine_id, size);
        }
        size
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
    /// Immutable global-context sizes of this mirror's physical host GPU. Interior
    /// mutability keeps the exclusion walkers read-only; `Mirror` itself is serialized by
    /// the Device mutex. A new host RM device gets a new `Mirror` and an empty cache.
    global_ctx_sizes: RefCell<GlobalCtxSizeCache>,
    vas: BTreeMap<(u32, u32), HostVas>,
    /// By [`TsgKey`]: the kernel's TSG id on a 615 guest, the guest handle on 610.
    tsgs: BTreeMap<TsgKey, HostTsg>,
    /// By guest ChID.
    channels: BTreeMap<u32, HostChannel>,
    userd_cursor: u32,
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
    next_event_id: u32,
    pub stats: MirrorStats,
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
        let worker = ShadowWorker::new(hm.clone(), intr.clone());
        Ok(Mirror {
            hm,
            intr,
            chip,
            base,
            end,
            translated,
            comp,
            reservation,
            global_ctx_sizes: RefCell::new(GlobalCtxSizeCache::default()),
            vas: BTreeMap::new(),
            tsgs: BTreeMap::new(),
            channels: BTreeMap::new(),
            userd_cursor: 0,
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
            memop_invalidate: std::env::var("CARAXES_MEMOP_INVALIDATE").as_deref() != Ok("0"),
            shadow_probe,
            bare_ce: std::env::var("CARAXES_MIRROR_BARE_CE").as_deref() == Ok("1"),
            ce_shadow,
            shadows: BTreeMap::new(),
            map_budget: crate::shadow::MapBudget::from_env(),
            worker,
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
            next_event_id: 0x1000,
            stats: MirrorStats::default(),
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
        let has_tsg = self.tsgs.contains_key(&tsg_key);
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
    /// on the host: `NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES` (NON_PRIVILEGED)
    /// on the device's own host device, rounded up to 64 KiB.
    fn global_ctx_size(&self, buffer_id: u16) -> Option<u64> {
        self.global_ctx_sizes
            .borrow_mut()
            .get_or_query(buffer_id, |engine_id| {
                let h = self.hm.lock().unwrap();
                let mut p = [0u8; 12];
                p[..4].copy_from_slice(&engine_id.to_le_bytes());
                let status = h.control(h.device(), 0x0080_1707, &mut p);
                let size = u32::from_le_bytes(p[8..12].try_into().unwrap());
                (status, size)
            })
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
            let u = hc.userd.as_ref().unwrap();
            u.harvest(gm);
            if gm.read_u32(Target::Sys(u.guest + 0x8c))
                .is_some_and(|put| put < u.entries && Some(put) != u.published_put)
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
        let (base, end, reservation) = (self.base, self.end, self.reservation);
        let descs = self.descs();
        let mut out = Vec::new();
        // The exclusions follow the guest's live channels (`exclusions_of`).
        let roots: Vec<u64> = self.shadows.keys().copied().filter(|r| root.is_none_or(|x| x == *r)).collect();
        for r in roots {
            out.extend(self.refresh_exclusions(reg, r));
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
        use crate::lazy::OnInvalidate;
        // A root whose channel's GPPut store does not trap (untrapped.rs): the invalidate is
        // the last point before the host channel may run the guest's next work, so the plan
        // is made and applied before the guest continues, with everything deferred so far.
        if self.hold_on && crate::untrapped::must_hold(pdb, &self.untrapped_roots(reg, gm.fb)) {
            let t0 = Instant::now();
            let mut notes = self.flush_lazy(reg, gm);
            notes.extend(self.shadow_resync(reg, gm, pdb));
            if self.inv_async {
                // The guest polls TRIGGER until it reads FALSE before its map call returns
                // (`kgmmuCommitTlbInvalidate_TU102`); it reads TRUE until the worker applied
                // every job submitted up to here ([`Mirror::invalidate_busy`], invgate.rs).
                // The device lock is free meanwhile.
                self.inv_gate.arm(self.worker.submitted(), t0);
            } else {
                self.worker.wait_all();
                self.hold.note_held(t0.elapsed());
            }
            return notes;
        }
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

    /// The untrapped-GPPut counters (untrapped.rs), for the pool's per-context line.
    pub fn hold_summary(&self) -> String {
        format!(
            "{}; held invalidates {} (trigger read busy {} time(s), {} waited past {} ms) [{}]",
            self.hold.summary(self.hold_on),
            if self.inv_async { "complete through the trigger's busy bit" } else { "wait under the device lock" },
            self.inv_gate.busy_reads,
            self.inv_gate.fallbacks,
            crate::invgate::FALLBACK.as_millis(),
            crate::invgate::ENV
        )
    }

    /// A read of the invalidate trigger: whether the held invalidate in flight is still
    /// being applied (TRIGGER reads TRUE). The first read that finds it applied counts the
    /// hold with its time from the trigger write (invgate.rs).
    pub fn invalidate_busy(&mut self) -> bool {
        match self.inv_gate.read(&self.worker.progress) {
            crate::invgate::Trigger::Busy => true,
            crate::invgate::Trigger::Done(Some(d)) => {
                self.hold.note_held(d);
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
        let untrapped = self
            .channels
            .get(&chid)
            .and_then(|c| c.shadow_root)
            .is_some_and(|r| self.untrapped_roots(reg, gm.fb).contains(&r));
        let line = self.hold.note_ahead(pending, || {
            format!(
                "untrapped GPPut: at {} chid {chid}'s host channel ran ahead of its doorbell (GPGet {get}, forwarded up to {fwd}, GPPut {put}) with plans pending (lazy {lazy}, worker {worker}, {since} unwaited invalidate plan(s) since the forward); USERD untrapped {untrapped}",
                crate::untrapped::wall()
            )
        })?;
        eprintln!("vfio-user-nvgpu: ERROR {line}");
        Some(line)
    }

    /// The registered guest RAM descriptors, read once (they do not change while the
    /// guest runs).
    fn descs(&mut self) -> Vec<(u32, u64, u64)> {
        if self.descs.is_none() {
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
    /// its shadow VAS or None, its host TSG handle)`.
    pub fn pool_channels(&self) -> Vec<(u32, Option<u64>, Option<u32>)> {
        self.channels
            .iter()
            .map(|(chid, c)| (*chid, c.shadow_root, self.tsgs.get(&c.tsg).map(|t| t.handle)))
            .collect()
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

    /// Wait until the worker applied everything submitted so far: the mirror of every
    /// root is then consistent with the guest's tables as of the last resync (the
    /// ordering guarantee a doorbell needs).
    pub fn shadow_barrier(&self) {
        self.worker.wait_all();
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
            .filter(|(_, sp)| sp.chids.is_empty() && sp.tsgs.is_empty() && sp.private.is_empty())
            .map(|(r, _)| *r)
            .collect();
        for r in empty {
            if let Some(sp) = self.shadows.remove(&r) {
                // Freed by the worker in bounded batches, after everything queued before.
                self.worker.submit(Job::Free(r));
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
            // Ordering: the doorbell is forwarded only after the worker applied every plan
            // submitted so far for this root (the mirror is consistent for it). A held
            // invalidate of this root still in flight is among them (invgate.rs); another
            // root's plans are not waited for.
            match root {
                Some(r) => self.worker.wait_root(r),
                None => self.worker.wait_all(),
            }
            mark(Step::Wait);
            gm.fb.flush();
            mark(Step::Flush);
            if !submit {
                notes.push(format!("shadow: chid {chid} context already prepared; no submission"));
                return notes;
            }
            if let Some(u) = self.channels.get_mut(&chid).and_then(|hc| hc.userd.as_mut()) {
                u.harvest(gm);
                if let Err(e) = u.publish(gm) {
                    notes.push(format!("shadow USERD chid {chid}: {e}; submission refused"));
                    return notes;
                }
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
            return notes;
        }
        let Some(ch) = reg.channel(chid).cloned() else {
            return vec![format!("shadow: chid {chid} not recorded")];
        };
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
        let new_b = self.root_exclusions(reg, root);
        // A bare CE channel (`bare_ce`, the GL driver's) gets a host TSG of its own, keyed
        // by its internal TSG id (615) or its handle, on the channel's own engine.
        let (tsg_key, bare, tsg_guest) = tsg_key_of(reg, &ch);
        // The host RM clears the channel's USERD when it creates the mirror; the guest has
        // already written its GPFIFO entry and GPPut for the submission that rang this
        // doorbell. Save them now and restore them after the alloc, or the GR PBDMA sees
        // GPPut 0 and fetches nothing (measured 2026-09-23: GPGet 0 / GPPut 0, the launch
        // hung with no Xid).
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
        } else {
            crate::shadow::Layout::default()
        };
        if let std::collections::btree_map::Entry::Vacant(slot) = self.shadows.entry(root) {
            match crate::shadow::ShadowVas::new(&mut h, self.map_budget.clone(), &layout) {
                Ok(mut sv) => {
                    sv.comp = self.comp.clone();
                    notes.push(format!(
                        "shadow: host-managed VAS {:#x} for guest root {root:#x}{} [host]",
                        sv.vas,
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
                        }
                    ));
                    let vas = sv.vas;
                    self.worker.submit(Job::Create(root, Box::new(sv)));
                    let mut front = crate::shadow::ShadowFront::new(root);
                    front.layout = layout;
                    front.comp = self.comp.clone();
                    slot.insert(ShadowSpace {
                        front,
                        vas,
                        exclude: Vec::new(),
                        held: BTreeMap::new(),
                        private: BTreeMap::new(),
                        chids: Default::default(),
                        tsgs: Default::default(),
                    });
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
        let sp = self.shadows.get_mut(&root).expect("built");
        sp.exclude = new_b;
        sp.chids.insert(chid);
        sp.tsgs.insert(tsg_key);
        let exclude = sp.exclude.clone();
        // Mirror the guest's set-A before the channel/GPFIFO VA is validated against the
        // VAS: plan here, apply in the worker, and wait for it.
        let plan = sp.front.plan(
            gm.fb,
            gm.sys,
            self.base,
            self.end,
            self.reservation,
            &exclude,
            &descs,
        );
        let vas_handle = sp.vas;
        drop(h);
        if let Some(plan) = plan {
            self.worker.submit(Job::Apply(plan, Instant::now()));
        }
        self.worker.wait_all();
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
        if let Err(e) = self.check_tsg_reuse(reg, tsg_key, tsg_guest, &mut notes) {
            notes.push(format!("shadow: chid {chid}: {e}"));
            self.abandon_shadow_build(root, chid, tsg_key, &mut notes);
            return notes;
        }
        // A stale host TSG under the key was dropped from the shadow space's set too.
        if let Some(sp) = self.shadows.get_mut(&root) {
            sp.tsgs.insert(tsg_key);
        }
        let mut h = self.hm.lock().unwrap();
        // One host TSG per guest TSG, on the shadow VAS (GR0): the guest's GR channels
        // of one group (CUDA's chids 2-9, one TSG, one context share) share one GR
        // context on the host too, so state a push sets on one channel is what a launch
        // on another sees (the guest's layout; before, every channel got its own TSG and
        // context, and `self.tsgs` kept only the last handle).
        let tsg = match self.tsgs.get(&tsg_key) {
            Some(t) => {
                notes.push(format!(
                    "shadow: chid {chid} joins host TSG {:#x} of guest TSG {:#x} ({tsg_key}) [host]",
                    t.handle, tsg_guest.1
                ));
                t.handle
            }
            None => {
                let gr = v::lookup(v::NV2080_ENGINE_TYPES, "GR0").unwrap_or(1);
                // The guest TSG's engine: GR0 for a compute channel, COPYn for a CE one.
                let engine_type = match reg.tsg(tsg_guest) {
                    Some((_, e)) if e != 0 => e,
                    _ if bare && ch.engine_type != 0 => ch.engine_type,
                    _ => gr,
                };
                // The host driver's layout (hostabi.rs).
                let mut tp = crate::hostabi::current().tsg_params(vas_handle, engine_type);
                let device = h.device();
                let tsg = match h.alloc_object(
                    device,
                    sys::KEPLER_CHANNEL_GROUP_A,
                    &mut tp,
                    "shadow-tsg",
                ) {
                    Ok(x) => x,
                    Err(st) => {
                        notes.push(format!("shadow: TSG refused: {}", status_name(st)));
                        if crate::hostfloor::is_exhaustion(st) {
                            self.exhausted.insert(chid);
                        }
                        drop(h);
                        self.abandon_shadow_build(root, chid, tsg_key, &mut notes);
                        return notes;
                    }
                };
                notes.push(format!(
                    "shadow: host TSG {tsg:#x} for guest TSG {:#x} ({tsg_key}) on VAS {vas_handle:#x} [host]",
                    tsg_guest.1
                ));
                self.tsgs.insert(
                    tsg_key,
                    HostTsg {
                        handle: tsg,
                        vas: vas_key,
                        engine: engine_type,
                        guest: tsg_guest,
                        gfxp: None,
                    },
                );
                tsg
            }
        };
        // Everything this build makes on the host, freed newest first when it fails (a
        // channel refused after its guest pages were wrapped, or built but never scheduled,
        // used to stay until the device exited: one leak per doorbell of a refused channel).
        let mut owned: Vec<u32> = Vec::new();
        let mut shadow_userd: Option<ShadowUserd> = None;
        let mut rc_page: Option<crate::hostmem::DevicePage> = None;
        let built = (|| -> Result<Built, (String, Option<u32>)> {
            // P21 design H: a USERD page the guest maps into the BAR1 pool area lives at
            // its pool page (fb.rs `remap`).
            let userd = match ch.userd.map(|t| match t {
                Target::Fb(pa) => Target::Fb(gm.fb.remap(pa)),
                t => t,
            }) {
                Some(Target::Fb(pa)) if pa >= self.base && pa + PAGE <= self.end => {
                    (self.reservation, pa - self.base)
                }
                // USERD in guest sysmem (the guest's RMInstLoc USERD=COH,
                // docs/caraxes/mapping.md §6): the host memory object over the guest's own
                // page, as for a CE channel.
                Some(Target::Sys(iova)) if std::env::var("CARAXES_SHADOW_USERD").as_deref() == Ok("1") => {
                    let mut snapshot = [0; crate::userd::USERD_BYTES];
                    if iova % 512 != 0 || !gm.read(Target::Sys(iova), &mut snapshot) {
                        return Err(("shadow USERD: guest extent is unaligned or unreadable".into(), None));
                    }
                    let page = h.alloc_host_userd().map_err(|st| (
                        format!("shadow USERD allocation: {}", status_name(st)), Some(st)
                    ))?;
                    let handle = page.handle();
                    notes.push(format!("shadow USERD chid {chid}: guest sysmem {iova:#x} -> private host vidmem {:#x}, object {handle:#x} [host]", page.gpu_addr()));
                    shadow_userd = Some(ShadowUserd {
                        storage: page, guest: iova, entries: ch.gpfifo_entries, published_put: None,
                    });
                    (handle, 0)
                }
                Some(Target::Sys(iova)) => {
                    match gm
                        .sys
                        .translate(iova & !(PAGE - 1), PAGE)
                        .and_then(|gpa| gm.sys.host_ptr(gpa, PAGE))
                        .map(|ptr| h.guest_page_object(ptr, PAGE, "shadow-userd-page"))
                    {
                        Some(Ok(d)) => {
                            owned.push(d);
                            (d, iova & (PAGE - 1))
                        }
                        other => {
                            return Err((
                                format!(
                                    "shadow: chid {chid} USERD IOVA {iova:#x} not registered guest RAM ({:?})",
                                    other.map(|r| r.map_err(|e| e.to_string()))
                                ),
                                None,
                            ))
                        }
                    }
                }
                other => {
                    return Err((
                        format!("shadow: chid {chid} USERD {other:?} not a vidmem reservation page"),
                        None,
                    ))
                }
            };
            // The error notifier: the guest's page, so the host RM writes an RC there and
            // the guest sees the fault (returns 719) instead of hanging to the 30 s
            // watchdog (the CE path does this; without it launch5 hung on the MMU fault).
            let h_error = match ch.error_notifier {
                Some(Target::Sys(iova)) if iova % PAGE == 0 => gm
                    .sys
                    .translate(iova, PAGE)
                    .and_then(|gpa| gm.sys.host_ptr(gpa, PAGE))
                    .and_then(|ptr| h.guest_page_object(ptr, PAGE, "shadow-error-notifier").ok())
                    .inspect(|d| owned.push(*d))
                    .unwrap_or(0),
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
                            "shadow: chid {chid}: no host error notifier ({e:#}); an RC of this channel reaches the guest only as a fence timeout [host]"
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
            cp.hUserdMemory[0] = userd.0;
            cp.userdOffset[0] = userd.1;
            cp.engineType = ch.engine_type;
            if let Some((bytes, end, guest, host, hits)) = &diag_gpfifo {
                notes.push(format!(
                    "shadow diagnostic: chid {chid} GPFIFO start {:#x} end {end:#x?} {} entries/{bytes:#x} bytes: guest {guest}; host {host}; set-B intersections {hits:x?}; USERD object {:#x}+{:#x}; engineType {:#x}; guest alloc_flags {:#x}; host flags {:#x}/internalFlags {:#x}; host ABI {} [host]",
                    ch.gpfifo_va, ch.gpfifo_entries, userd.0, userd.1, ch.engine_type,
                    ch.alloc_flags, cp.flags, cp.internalFlags, crate::hostabi::current().driver(),
                ));
            }
            let channel = match h.alloc_object(
                tsg,
                crate::board::host_class_of(self.chip.channel.gpfifo),
                as_bytes(&mut cp),
                "shadow-channel",
            ) {
                Ok(x) => x,
                Err(st) => {
                    return Err((
                        format!(
                            "shadow: channel refused: {} (gpfifo va {:#x})",
                            status_name(st),
                            ch.gpfifo_va
                        ),
                        Some(st),
                    ))
                }
            };
            owned.push(channel);
            notes.push(format!("shadow: host GR channel {channel:#x} in TSG {tsg:#x} on VAS {vas_handle:#x}, gpfifo va {:#x} [host]", ch.gpfifo_va));
            // The guest's engine objects; the compute object makes the host RM build set B.
            let mut objects = Vec::new();
            let mut engine_map = BTreeMap::new();
            let mut refused = None;
            for (guest_handle, class, params) in ch.engine_objects_with_handles() {
                let mut p = params.to_vec();
                match h.alloc_object(channel, crate::board::host_class_of(class), &mut p, "shadow-engine") {
                    Ok(oh) => {
                        notes.push(format!(
                            "shadow: chid {chid} host object class {class:#x}{} {oh:#x} [host]",
                            crate::board::mapped_note(class)
                        ));
                        objects.push((class, oh));
                        engine_map.insert(guest_handle, oh);
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
                let Some(engine) = video_bind(self.chip, &ch).or_else(|| copy_bind(&ch)) else {
                    return Err(("shadow: no engine object; not scheduling".into(), refused));
                };
                let mut p = engine.to_le_bytes();
                let st = h.control(channel, CMD_CHANNEL_BIND, &mut p);
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
            if let Some(u) = shadow_userd.as_mut() {
                // Initialize an unscheduled, empty ring once. Later copies never
                // overwrite GPU-owned state; new GPPut is published before the doorbell.
                let get = gm.read_u32(Target::Sys(u.guest + 0x88))
                    .filter(|get| *get < u.entries)
                    .ok_or_else(|| ("shadow USERD: invalid initial GPGet".into(), None))?;
                u.storage.write_u32(0x88, get);
                // Hold the ring empty until token acquisition and ownership insertion.
                u.storage.write_u32(0x8c, get);
                u.storage.flush();
            }
            let mut sched = [1u8, 0, 0];
            let st = h.control(channel, CMD_GPFIFO_SCHEDULE, &mut sched);
            if st != sys::NV_OK {
                return Err((format!("shadow: GPFIFO_SCHEDULE: {}", status_name(st)), None));
            }
            let mut tok = [0u8; 4];
            let st = h.control(channel, CMD_GET_WORK_SUBMIT_TOKEN, &mut tok);
            if st != sys::NV_OK {
                return Err((
                    format!("shadow: GET_WORK_SUBMIT_TOKEN: {}", status_name(st)),
                    None,
                ));
            }
            Ok((channel, objects, engine_map, u32::from_le_bytes(tok)))
        })();
        let (channel, objects, engine_map, token) = match built {
            Ok(b) => b,
            Err((why, st)) => {
                notes.push(why);
                // Newest first: the channel (with its engine objects), then the guest pages.
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
                drop(h);
                if st.is_some_and(crate::hostfloor::is_exhaustion) {
                    self.exhausted.insert(chid);
                }
                self.abandon_shadow_build(root, chid, tsg_key, &mut notes);
                return notes;
            }
        };
        let pages: Vec<u32> = owned.into_iter().filter(|d| *d != channel).collect();
        self.stats.channels_created += 1;
        self.channels.insert(
            chid,
            HostChannel {
                generation: ch.generation,
                handle: channel,
                tsg: tsg_key,
                token,
                objects,
                engine_map,
                pages,
                created: Instant::now(),
                doorbells: 0,
                shadow_root: Some(root),
                fwd_put: None,
                fwd_unwaited: 0,
                userd: shadow_userd,
                context_ready: submit,
                rc_page,
            },
        );
        notes.push(format!(
            "shadow: chid {chid} scheduled; host token {token:#x} (host chid {}, runlist {}) [host]",
            token & 0xFFF,
            (token >> 16) & 0x7F
        ));
        drop(h);
        // Restore the guest's GPGet/GPPut the host RM cleared at channel creation.
        if let Some((get_at, put_at, Some(get), Some(put))) = saved_userd
            .filter(|_| self.channels.get(&chid).is_some_and(|hc| hc.userd.is_none()))
        {
            if gm.read_u32(put_at) != Some(put) {
                gm.write(get_at, &get.to_le_bytes());
                gm.write(put_at, &put.to_le_bytes());
                notes.push(format!(
                    "shadow: chid {chid} restored guest GPGet {get} / GPPut {put} (host RM had cleared the USERD) [guest]"
                ));
            }
        }
        gm.fb.flush();
        if !submit {
            notes.push(format!("shadow: chid {chid} context prepared with an empty private USERD; no submission"));
            return notes;
        }
        if let Some(u) = self.channels.get_mut(&chid).and_then(|hc| hc.userd.as_mut()) {
            if let Err(e) = u.publish(gm) {
                notes.push(format!("shadow USERD chid {chid}: {e}; submission refused"));
                return notes;
            }
        }
        match self.doorbell(chid) {
            Ok(n) => notes.push(n),
            Err(e) => notes.push(format!("shadow: doorbell chid {chid}: {e}")),
        }
        // One-shot USERD read (no poll): GPGet advancing past GPPut's start means the GR
        // PBDMA fetched the guest's first push; GPGet stuck at 0 means it never fetched
        // (scheduling/token). Distinguishes a completion-delivery hang from a fetch hang.
        // Diagnostic only (`CARAXES_MIRROR_DIAG=1`): its 5 ms sleep ran at every shadow
        // channel's creation, about half a second per CUDA context (mapping.md §3).
        let diag = std::env::var("CARAXES_MIRROR_DIAG").as_deref() == Ok("1");
        if let Some(userd) = ch.userd.filter(|_| diag) {
            let at = |o: u64| match userd {
                Target::Fb(pa) => Target::Fb(pa + o),
                Target::Sys(pa) => Target::Sys(pa + o),
                t => t,
            };
            std::thread::sleep(std::time::Duration::from_millis(5));
            notes.push(format!(
                "shadow: chid {chid} USERD after doorbell: GPGet {:?} GPPut {:?} [host GPU]",
                gm.read_u32(at(0x88)),
                gm.read_u32(at(0x8C))
            ));
        }
        notes
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
        let u = hc.userd.as_ref().ok_or("management takeover requires private USERD")?;
        let get = u.storage.read_u32(0x88);
        if u.published_put != Some(get) { return Err("management host channel still has unfetched work".into()); }
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
        self.channels.get(&chid).and_then(|hc| hc.userd.as_ref()).ok_or("private USERD disappeared")?.harvest(gm);
        self.worker.wait_all();
        let mut notes = vec![idle_note];
        if !self.free_tsg(key, &mut notes) { return Err(notes.join("; ")); }
        self.worker.wait_idle();
        Ok(notes)
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

    pub fn promote_deferred_context(&mut self, reg: &Registry, chid: u32) -> Result<(), String> {
        let ch = reg.channel(chid).ok_or("deferred target channel is gone")?;
        let root = reg.vas_key_of(ch).and_then(|key| reg.vas_root(key));
        let hc = self.channels.get_mut(&chid).filter(|hc| hc.generation == ch.generation)
            .ok_or("deferred target host context is gone")?;
        if root != hc.shadow_root.map(Target::Fb) { return Err("deferred target VAS changed".into()); }
        hc.context_ready = true;
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

    /// Bind mirror-owned ordinary-GFXP buffers to one prepared GR TSG. Every address in
    /// the BIND is selected by host RM in the private shadow VAS; guest pointers are not
    /// forwarded.
    pub fn bind_owned_gfxp(&mut self, reg: &Registry, client: u32, guest_tsg: u32) -> Result<Vec<String>, String> {
        let guest = reg.resolve((client, guest_tsg));
        if reg.tsg(guest).is_none_or(|(_, engine)| engine != 1) {
            return Err("owned GFXP target is not a live GR TSG".into());
        }
        let key = tsg_key_of_guest(reg, guest).ok_or("owned GFXP target has no TSG key")?;
        let mut members = reg.channels().filter(|ch| tsg_key_of(reg, ch).0 == key);
        let ch = members.next().ok_or("owned GFXP TSG has no channel")?;
        if members.next().is_some() {
            return Err("owned GFXP currently requires a single-channel TSG".into());
        }
        let (chid, generation) = (ch.chid, ch.generation);
        let vas_key = reg.vas_key_of(ch).ok_or("owned GFXP target has no VAS")?;
        let root = match reg.vas_root(vas_key) {
            Some(Target::Fb(root)) => root,
            _ => return Err("owned GFXP target has no shadow FB root".into()),
        };
        let (host_channel, host_tsg, vas) = {
            let hc = self.channels.get(&chid)
                .filter(|hc| hc.generation == generation && hc.context_ready && hc.tsg == key)
                .ok_or("owned GFXP target has no prepared and promoted host context")?;
            let ht = self.tsgs.get(&key).ok_or("owned GFXP host TSG is gone")?;
            if ht.guest != guest || ht.engine != 1 || ht.vas != vas_key || hc.shadow_root != Some(root) {
                return Err("owned GFXP host TSG identity changed".into());
            }
            if let Some(gfxp) = ht.gfxp.as_ref() {
                if gfxp.chid == chid && gfxp.generation == generation && gfxp.host_channel == hc.handle
                    && gfxp.root == root && gfxp.bound {
                    return Ok(vec![format!("tsg {key}: owned ordinary-GFXP buffers already bound to chid {chid} generation {generation} [host]")]);
                }
                return Err("owned GFXP TSG has retained partial or mismatched state".into());
            }
            let userd = hc.userd.as_ref().ok_or("owned GFXP target has no private USERD")?;
            if hc.doorbells != 0 || userd.published_put.is_some()
                || userd.storage.read_u32(USERD_GP_GET) != 0 || userd.storage.read_u32(USERD_GP_PUT) != 0 {
                return Err("owned GFXP target is not an empty first-use context at cursor zero".into());
            }
            (hc.handle, ht.handle, self.shadows.get(&root).ok_or("owned GFXP shadow VAS is gone")?.vas)
        };
        if self.tsgs.values().filter(|t| t.gfxp.is_some()).count() >= GFXP_TSG_CAP {
            return Err(format!("owned GFXP cap of {GFXP_TSG_CAP} TSGs reached"));
        }

        self.channels.get_mut(&chid).unwrap().context_ready = false;
        let flags = ((sys::NVOS30_FLAGS_IDLE_PUSH_BUFFER | sys::NVOS30_FLAGS_IDLE_CACHE1
            | sys::NVOS30_FLAGS_IDLE_ALL_ENGINES) << 8) | sys::NVOS30_FLAGS_BEHAVIOR_SLEEP;
        let (status, idle_note) = self.idle_host_channels(&[(chid, host_channel)], flags, 100_000);
        if status != sys::NV_OK { return Err(idle_note); }
        let hc = self.channels.get(&chid).filter(|hc| hc.generation == generation && hc.handle == host_channel
            && hc.tsg == key && hc.shadow_root == Some(root)).ok_or("owned GFXP target changed while idling")?;
        let userd = hc.userd.as_ref().ok_or("owned GFXP private USERD disappeared")?;
        if hc.doorbells != 0 || userd.published_put.is_some()
            || userd.storage.read_u32(USERD_GP_GET) != 0 || userd.storage.read_u32(USERD_GP_PUT) != 0 {
            return Err("owned GFXP target changed from its empty initial state while idling".into());
        }
        self.worker.wait_all();
        self.worker.wait_idle();

        let mut owned = OwnedGfxp { chid, generation, host_channel, root, bind_attempted: false, bound: false, buffers: Vec::new() };
        let mut notes = vec![idle_note];
        let attempt = (|| -> Result<(), String> {
            let mut addresses = [0u64; 5];
            let mut total = 0u64;
            let mut host = self.hm.lock().unwrap();
            for (slot, engine) in (0xbu32..=0xf).enumerate() {
                let mut p = [0u8; 12];
                p[..4].copy_from_slice(&engine.to_le_bytes());
                let st = host.control(host.device(), CMD_GET_ENGINE_CONTEXT_PROPERTIES, &mut p);
                let alignment = u32::from_le_bytes(p[4..8].try_into().unwrap()) as u64;
                let size = u32::from_le_bytes(p[8..12].try_into().unwrap()) as u64;
                if st != sys::NV_OK || (size != 0 && !alignment.is_power_of_two()) {
                    return Err(format!("GFXP property {engine:#x} refused or invalid: {} size {size:#x} alignment {alignment:#x}", status_name(st)));
                }
                if size == 0 {
                    notes.push(format!("GFXP buffer {engine:#x}: host reports no allocation required [host]"));
                    continue;
                }
                let alignment = alignment.max(PAGE);
                let len = size.checked_add(alignment - 1).map(|n| n & !(alignment - 1)).ok_or("GFXP buffer size overflow")?;
                total = total.checked_add(len).ok_or("GFXP total size overflow")?;
                if total > GFXP_BUFFER_CAP { return Err(format!("GFXP buffers exceed the {GFXP_BUFFER_CAP:#x}-byte cap")); }
                let memory = host.alloc_host_gfxp(len, alignment)
                    .map_err(|st| format!("GFXP physical {engine:#x}: {}", status_name(st)))?;
                owned.buffers.push(OwnedGfxpBuffer { memory: Some(memory), virtual_handle: None, va: 0, len, mapped: false });
                let buffer = owned.buffers.last_mut().unwrap();
                let (virtual_handle, va) = host.alloc_virtual_aligned(vas, len, alignment)
                    .map_err(|st| format!("GFXP virtual {engine:#x}: {}", status_name(st)))?;
                buffer.virtual_handle = Some(virtual_handle);
                buffer.va = va;
                if !crate::hostmem::valid_aligned_virtual(len, alignment, va) {
                    return Err("GFXP host virtual allocation returned an invalid range; handle retained".into());
                }
                let end = va.checked_add(len).ok_or("GFXP private VA overflow")?;
                let sp = self.shadows.get_mut(&root).ok_or("GFXP shadow disappeared")?;
                sp.private.entry(key).or_default().push((va, end));
                sp.exclude.push((va, end));
                sp.exclude.sort_unstable();
                sp.exclude.dedup();
                let st = host.map_dma_fixed(virtual_handle, buffer.memory.as_ref().unwrap().handle(), 0, len, va,
                    nvrm_abi::nvgpu::nvos46_flags::PAGE_SIZE_4KB, false, None, false);
                if st != sys::NV_OK { return Err(format!("GFXP map {engine:#x}: {}", status_name(st))); }
                buffer.mapped = true;
                addresses[slot] = va;
                notes.push(format!("GFXP buffer {engine:#x}: size {size:#x}, mapped span {len:#x} at private VA {va:#x} [host]"));
            }
            owned.bind_attempted = true;
            let st = bind_gfxp(&host, host_tsg, 1, &addresses);
            let (get_st, mode) = get_gfxp_mode(&host, host_channel);
            if st != sys::NV_OK || get_st != sys::NV_OK || mode != 1 {
                return Err(format!("GFXP BIND1/GET refused: bind {}, get {}, mode {mode:#x}", status_name(st), status_name(get_st)));
            }
            owned.bound = true;
            notes.push(format!("tsg {key}: host-owned ordinary-GFXP mode 1 bound and verified ({total} bytes) [host]"));
            Ok(())
        })();
        self.tsgs.get_mut(&key).unwrap().gfxp = Some(owned);
        if attempt.is_ok() {
            self.channels.get_mut(&chid).unwrap().context_ready = true;
            Ok(notes)
        } else {
            Err(format!("{}; resources retained and target blocked", attempt.unwrap_err()))
        }
    }

    /// Set compute CTA preemption on an exact host-owned ordinary-GFXP context. This
    /// translates only the captured pointer-free shape: flags COMPUTE, CTA mode, every
    /// buffer and route field zero. The guest's control payload is never consumed.
    pub fn bind_owned_compute_cta(
        &mut self,
        reg: &Registry,
        client: u32,
        guest_tsg: u32,
    ) -> Result<Vec<String>, String> {
        let guest = reg.resolve((client, guest_tsg));
        if reg.tsg(guest).is_none_or(|(_, engine)| engine != 1) {
            return Err("owned compute-CTA target is not a live GR TSG".into());
        }
        let key = tsg_key_of_guest(reg, guest).ok_or("owned compute-CTA target has no TSG key")?;
        let mut members = reg.channels().filter(|ch| tsg_key_of(reg, ch).0 == key);
        let ch = members.next().ok_or("owned compute-CTA TSG has no channel")?;
        if members.next().is_some() {
            return Err("owned compute-CTA currently requires a single-channel TSG".into());
        }
        let vas_key = reg.vas_key_of(ch).ok_or("owned compute-CTA target has no VAS")?;
        let root = match reg.vas_root(vas_key) {
            Some(Target::Fb(root)) => root,
            _ => return Err("owned compute-CTA target has no shadow FB root".into()),
        };
        let (chid, generation) = (ch.chid, ch.generation);
        let (host_channel, host_tsg) = {
            let hc = self.channels.get(&chid)
                .filter(|hc| hc.generation == generation && hc.context_ready && hc.tsg == key
                    && hc.shadow_root == Some(root))
                .ok_or("owned compute-CTA target has no exact host context")?;
            let ht = self.tsgs.get(&key).ok_or("owned compute-CTA host TSG is gone")?;
            ht.gfxp.as_ref().filter(|owned| owned.bound
                && owned.chid == chid && owned.generation == generation
                && owned.host_channel == hc.handle && owned.root == root)
                .ok_or("owned compute-CTA requires the exact bound ordinary-GFXP context")?;
            if ht.guest != guest || ht.engine != 1 || ht.vas != vas_key {
                return Err("owned compute-CTA host TSG identity changed".into());
            }
            (hc.handle, ht.handle)
        };

        self.channels.get_mut(&chid).unwrap().context_ready = false;
        let flags = ((sys::NVOS30_FLAGS_IDLE_PUSH_BUFFER | sys::NVOS30_FLAGS_IDLE_CACHE1
            | sys::NVOS30_FLAGS_IDLE_ALL_ENGINES) << 8) | sys::NVOS30_FLAGS_BEHAVIOR_SLEEP;
        let (idle_status, idle_note) = self.idle_host_channels(&[(chid, host_channel)], flags, 100_000);
        if idle_status != sys::NV_OK { return Err(idle_note); }
        let ch = reg.channel(chid).filter(|ch| ch.generation == generation)
            .ok_or("owned compute-CTA target changed while idling")?;
        if reg.vas_key_of(ch) != Some(vas_key)
            || reg.vas_root(vas_key) != Some(Target::Fb(root))
            || reg.channels().filter(|member| tsg_key_of(reg, member).0 == key).count() != 1
        {
            return Err("owned compute-CTA target VAS or TSG changed while idling".into());
        }
        let hc = self.channels.get(&chid).filter(|hc| hc.generation == generation
            && hc.handle == host_channel && hc.tsg == key && hc.shadow_root == Some(root))
            .ok_or("owned compute-CTA host channel changed while idling")?;
        let ht = self.tsgs.get(&key).ok_or("owned compute-CTA host TSG disappeared while idling")?;
        if ht.handle != host_tsg || ht.guest != guest || ht.vas != vas_key
            || !ht.gfxp.as_ref().is_some_and(|owned| owned.bound
                && owned.chid == chid && owned.generation == generation
                && owned.host_channel == hc.handle && owned.root == root)
        {
            return Err("owned compute-CTA ownership changed while idling".into());
        }

        let host = self.hm.lock().unwrap();
        let bind = bind_compute_cta(&host, host_tsg);
        let (get, gfxp, cilp) = get_preemption_modes(&host, host_channel);
        if bind != sys::NV_OK || get != sys::NV_OK || gfxp != 1 || cilp != 1 {
            return Err(format!(
                "compute-CTA BIND/GET refused: bind {}, get {}, gfxp {gfxp:#x}, cilp {cilp:#x}; target remains blocked",
                status_name(bind), status_name(get)
            ));
        }
        drop(host);
        self.channels.get_mut(&chid).unwrap().context_ready = true;
        Ok(vec![idle_note, format!(
            "tsg {key}: host-owned compute CTA mode 1 bound; ordinary GFXP mode 1 preserved [host]"
        )])
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
        let hc = self.channels.get_mut(&chid).filter(|hc| hc.generation == generation)
            .ok_or("deferred target host context is gone")?;
        if root != hc.shadow_root.map(Target::Fb) {
            return Err("deferred target VAS changed".into());
        }
        let key = hc.tsg;
        if reg.channels().filter(|c| tsg_key_of(reg, c).0 == key).count() != 1 {
            return Err("deferred eviction of a multi-channel TSG is not implemented".into());
        }
        hc.context_ready = false;

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
        let hc = self.channels.get(&chid)
            .filter(|hc| hc.generation == generation && hc.tsg == key)
            .ok_or("deferred target host context changed while idling")?;
        let userd = hc.userd.as_ref().ok_or("deferred target has no private USERD")?;
        let get = userd.storage.read_u32(USERD_GP_GET);
        if get >= userd.entries {
            return Err(format!("deferred target final GPGet {get} is outside its {}-entry ring", userd.entries));
        }
        let put = userd.storage.read_u32(USERD_GP_PUT);
        let all_published_work_fetched = userd.published_put == Some(get);
        let untouched_empty_ring = userd.published_put.is_none()
            && hc.doorbells == 0
            && put == get;
        if !all_published_work_fetched && !untouched_empty_ring {
            return Err("deferred target still has published work after idling".into());
        }
        userd.harvest(gm);

        self.worker.wait_all();
        let mut notes = vec![idle_note];
        if !self.free_tsg(key, &mut notes) { return Err(notes.join("; ")); }
        self.worker.wait_idle();
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
            if let Some((c, _)) = ch
                .engine_objects
                .iter()
                .find(|(c, _)| !video_class_ok(self.chip, *c))
            {
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
        if needs_software_physical_ce(ch.alloc_flags, is_gr) {
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
            let mc = v::lookup(v::MC_ENGINE_IDX, name);
            let vector = mc.and_then(|m| {
                self.chip
                    .interrupts
                    .entries
                    .iter()
                    .find(|e| e.mc_idx as u32 == m)
                    .map(|e| e.vector_nonstall)
            });
            match vector {
                Some(vec) if vec != 0 => want.push((name, *notifier | EVENT_NONSTALL, vec)),
                _ => notes.push(format!(
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
        self.root_exclusions(reg, root)
    }

    /// The set-B exclusions of shadow root `root` now: the live channels' ([`exclusions_of`])
    /// and those held for a host TSG not yet freed ([`set_b_with_held`]).
    fn root_exclusions(&self, reg: &Registry, root: u64) -> Vec<(u64, u64)> {
        let live = exclusions_of(reg, root, |id| self.global_ctx_size(id));
        match self.shadows.get(&root) {
            Some(sp) => {
                let mut out = set_b_with_held(live, &sp.held);
                out.extend(sp.private.values().flatten().copied());
                out.sort_unstable();
                out.dedup();
                out
            }
            None => live,
        }
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

    /// Recompute the exclusions of shadow root `root`; one note when they changed.
    fn refresh_exclusions(&mut self, reg: &Registry, root: u64) -> Vec<String> {
        let now = self.root_exclusions(reg, root);
        let Some(sp) = self.shadows.get_mut(&root) else {
            return Vec::new();
        };
        if sp.exclude == now {
            return Vec::new();
        }
        let added = now.iter().filter(|x| !sp.exclude.contains(x)).count();
        let released = sp.exclude.iter().filter(|x| !now.contains(x)).count();
        sp.exclude = now;
        vec![format!(
            "shadow: root {root:#x} set-B exclusions recomputed: {added} added, {released} released back to set A [guest]"
        )]
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

    /// Detach and release an owned GFXP set while the exact host TSG and channel still
    /// exist. Every failure retains the remaining objects and all private VA exclusions.
    fn free_owned_gfxp(&mut self, key: TsgKey, notes: &mut Vec<String>) -> bool {
        let Some(mut owned) = self.tsgs.get_mut(&key).and_then(|t| t.gfxp.take()) else {
            return true;
        };
        let Some((host_tsg, host_channel)) = self.tsgs.get(&key).and_then(|t| {
            self.channels.get(&owned.chid)
                .filter(|hc| hc.generation == owned.generation && hc.tsg == key
                    && hc.handle == owned.host_channel && hc.shadow_root == Some(owned.root))
                .map(|hc| (t.handle, hc.handle))
        }) else {
            self.tsgs.get_mut(&key).unwrap().gfxp = Some(owned);
            notes.push(format!("tsg {key}: ERROR owned GFXP channel identity changed; retained"));
            return false;
        };
        self.channels.get_mut(&owned.chid).unwrap().context_ready = false;
        let flags = ((sys::NVOS30_FLAGS_IDLE_PUSH_BUFFER | sys::NVOS30_FLAGS_IDLE_CACHE1
            | sys::NVOS30_FLAGS_IDLE_ALL_ENGINES) << 8) | sys::NVOS30_FLAGS_BEHAVIOR_SLEEP;
        let (status, note) = self.idle_host_channels(&[(owned.chid, host_channel)], flags, 100_000);
        notes.push(note);
        if status != sys::NV_OK {
            self.tsgs.get_mut(&key).unwrap().gfxp = Some(owned);
            return false;
        }
        if owned.bind_attempted {
            let host = self.hm.lock().unwrap();
            let bind = bind_gfxp(&host, host_tsg, 0, &[0; 5]);
            let (get, mode) = get_gfxp_mode(&host, host_channel);
            if bind != sys::NV_OK || get != sys::NV_OK || mode != 0 {
                notes.push(format!("tsg {key}: ERROR GFXP BIND WFI/GET: bind {}, get {}, mode {mode:#x}; retained",
                    status_name(bind), status_name(get)));
                self.tsgs.get_mut(&key).unwrap().gfxp = Some(owned);
                return false;
            }
            owned.bound = false;
        }
        self.worker.wait_all();
        self.worker.wait_idle();
        let mut host = self.hm.lock().unwrap();
        for buffer in owned.buffers.iter_mut().rev() {
            if buffer.mapped {
                let st = host.unmap_dma(buffer.virtual_handle.unwrap(), buffer.memory.as_ref().unwrap().handle(), buffer.va, buffer.len, false);
                if st != sys::NV_OK {
                    notes.push(format!("tsg {key}: ERROR unmapping private GFXP VA {:#x}: {}; retained", buffer.va, status_name(st)));
                    drop(host);
                    self.tsgs.get_mut(&key).unwrap().gfxp = Some(owned);
                    return false;
                }
                buffer.mapped = false;
            }
            if let Some(handle) = buffer.virtual_handle {
                if let Err(st) = host.free_object(handle) {
                    notes.push(format!("tsg {key}: ERROR freeing private GFXP virtual {handle:#x}: {}; retained", status_name(st)));
                    drop(host);
                    self.tsgs.get_mut(&key).unwrap().gfxp = Some(owned);
                    return false;
                }
                buffer.virtual_handle = None;
            }
            if let Some(memory) = buffer.memory.take() {
                if let Err((st, memory)) = host.free_host_gfxp(memory) {
                    buffer.memory = Some(memory);
                    notes.push(format!("tsg {key}: ERROR freeing private GFXP memory: {}; retained", status_name(st)));
                    drop(host);
                    self.tsgs.get_mut(&key).unwrap().gfxp = Some(owned);
                    return false;
                }
            }
        }
        drop(host);
        if let Some(sp) = self.shadows.get_mut(&owned.root) {
            sp.private.remove(&key);
            // `exclude` is the merged live + held + private set. Do not subtract exact
            // tuples here: another owner may have the same range. The normal root re-sync
            // recomputes it from all owners after `released_roots` is consumed.
            self.released_roots.push(owned.root);
        }
        notes.push(format!("tsg {key}: owned ordinary-GFXP buffers detached and freed [host]"));
        true
    }

    fn free_channel(&mut self, chid: u32, notes: &mut Vec<String>) -> bool {
        if let Some(key) = self.channels.get(&chid).map(|hc| hc.tsg) {
            if !self.free_owned_gfxp(key, notes) { return false; }
        }
        self.exhausted.remove(&chid);
        self.exhausted_raised.remove(&chid);
        if let Some(mut hc) = self.channels.remove(&chid) {
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
                    if let Some(u) = hc.userd.take() {
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
                hc.context_ready = false;
                self.channels.insert(chid, hc);
                return false;
            }
        }
        for sp in self.shadows.values_mut() {
            sp.chids.remove(&chid);
        }
        true
    }

    fn free_tsg(&mut self, key: TsgKey, notes: &mut Vec<String>) -> bool {
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
        if !freed { return false; }
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
            if let Some(ranges) = sp.held.remove(&key) {
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
            |id| self.global_ctx_size(id),
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
            let e = sp.held.entry(h.key).or_default();
            e.extend(h.ranges);
            e.sort_unstable();
            e.dedup();
        }
        notes
    }

    /// [`Mirror::free_object`] without the set-B holds.
    fn free_host_objects(&mut self, reg: &Registry, h_client: u32, h_object: u32) -> Vec<String> {
        let mut notes = Vec::new();
        if let Some((chid, generation, _, _)) = reg.engine_object(h_client, h_object) {
            if self.video_pending.get(&chid).is_some_and(|(handle, _, _)| *handle == h_object) {
                self.video_pending.remove(&chid);
                notes.push(format!("chid {chid}: pending engine {h_object:#x} dropped [guest]"));
            }
            if let Some(key) = self.channels.get(&chid)
                .filter(|hc| hc.generation == generation)
                .map(|hc| hc.tsg)
            {
                if self.tsgs.get(&key).is_some_and(|t| t.gfxp.is_some())
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
        self.inv_gate.clear();
        if let Some(inv) = self.invalidator.take() {
            let mut h = self.hm.lock().unwrap();
            notes.push(inv.free(&mut h));
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
            self.global_ctx_sizes.borrow().summary()
        )
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
    /// The last job finished (invgate.rs), shared with the worker thread.
    progress: Arc<crate::invgate::Progress>,
    /// Submitted jobs and the last one per shadow root.
    seqs: crate::invgate::Seqs,
    enabled: bool,
    notes: Arc<Mutex<Vec<String>>>,
    /// No teardown in progress.
    idle: Arc<(Mutex<bool>, std::sync::Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ShadowWorker {
    fn new(hm: Arc<Mutex<HostMemory>>, intr: Arc<IntrShared>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<(u64, Job)>();
        let progress = Arc::new(crate::invgate::Progress::default());
        let notes = Arc::new(Mutex::new(Vec::new()));
        let (progress2, notes2) = (progress.clone(), notes.clone());
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
                loop {
                    let next = if graveyard.is_empty() {
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
                }
            })
            .ok();
        ShadowWorker {
            tx: Some(tx),
            progress,
            idle,
            seqs: Default::default(),
            enabled: std::env::var("CARAXES_SHADOW_WORKER").as_deref() != Ok("0"),
            notes,
            thread,
        }
    }

    fn submit(&mut self, job: Job) {
        let root = match &job {
            Job::Create(r, _) | Job::Free(r) | Job::Coverage(r, _, _, _) => *r,
            Job::Apply(plan, _) => plan.root,
        };
        if self.thread.is_some() {
            self.seqs.forget_done(self.progress.done());
        }
        let seq = self.seqs.next(root);
        if let Some(tx) = &self.tx {
            let _ = tx.send((seq, job));
        }
        if !self.enabled {
            self.wait_all();
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
        self.thread.is_some() && self.progress.done() < self.seqs.submitted()
    }

    /// Block until every job submitted so far is done.
    fn wait_all(&self) {
        if self.thread.is_none() {
            return;
        }
        self.progress.wait_for(self.seqs.submitted());
    }

    /// Block until every job submitted so far for shadow root `root` is done. Jobs finish
    /// in submission order, so the root's mirror is then what its last plan made it; jobs
    /// of other roots queued after it are not waited for (invgate.rs).
    fn wait_root(&self, root: u64) {
        if self.thread.is_none() {
            return;
        }
        self.progress.wait_for(self.seqs.last_of(root));
    }

    /// The sequence number of the last job submitted (an invalidate gate's target), 0
    /// without a worker thread (nothing is ever in flight then).
    fn submitted(&self) -> u64 {
        if self.thread.is_none() {
            return 0;
        }
        self.seqs.submitted()
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
    use super::*;
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
    /// buffer ids never reach the host. The host's byte size is rounded up to 64 KiB once.
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
            Some(0x2_0000)
        );
        for id in [11, 12] {
            assert_eq!(
                cache.get_or_query(id, |_| panic!("the shared host engine was queried twice")),
                Some(0x2_0000)
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

    type Abi = v::RmAbi;

    fn registry() -> Registry {
        Registry::new(crate::fixture::rtx2070().chip.data().channel)
    }

    /// A TSG with the kernel's id `grp` in internalFlags (a 615 layout; the 610 struct has
    /// no such field, so the id is dropped there, as a 610 kernel sends none).
    fn tsg_params(grp: Option<u32>) -> Vec<u8> {
        let mut b = vec![0u8; <Abi as nvrm_sys::RmAbi>::TSG_PARAMS_SIZE as usize];
        let off = <Abi as nvrm_sys::RmAbi>::TSG_PARAMS_OFF_internalFlags;
        if let (Some(id), true) = (grp, b.len() >= off + 4) {
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
        <Abi as nvrm_sys::RmAbi>::TSG_PARAMS_SIZE as usize
            >= <Abi as nvrm_sys::RmAbi>::TSG_PARAMS_OFF_internalFlags + 4
    }

    /// Item 3 of the GSP plan round 2 (classes.md §8, TSG): the host TSG map is keyed by the
    /// kernel's TSG id where the kernel sends one (615), by the guest handle otherwise
    /// (610); the key does not depend on the order of channel and TSG allocations, and a
    /// bare channel's internal TSG is keyed by its own id.
    #[test]
    fn host_tsg_key_is_the_kernel_tsg_id() {
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
            promoted_ranges(&reg, &ch, |_| None),
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
        let r = promoted_ranges(&reg, &ch, |id| (id == 5).then_some(0x20_0000));
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
        let size = |id: u16| (id == 5).then_some(0x20_0000u64);
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
        let size = |_: u16| None;
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
        for h in held_at_free(&reg, |c| c.h_object == 0x40, |_| Some(root), size) {
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
        for h in held_at_free(&reg, |c| c.h_object == 0x41, |_| Some(root), size) {
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
        assert!(held_at_free(&reg, |c| c.h_object == 0x42, |_| None, size).is_empty());
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
        let size = |_: u16| None;
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
        let held = held_at_free(&reg, |c| c.h_object == 0x40, |c| fb_root(&reg, c), size);
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
        let size = |_: u16| None;
        let held = held_at_free(&reg, |c| c.h_client == 0xb4, |c| fb_root(&reg, c), size);
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
        let p = compute_cta_bind_params(0x1234, 0x5678);
        let word = |o| u32::from_le_bytes(p[o..o + 4].try_into().unwrap());
        assert_eq!((word(0), word(4), word(8)), (1, 0x1234, 0x5678));
        assert_eq!((word(88), word(92)), (0, 1), "graphics is ignored and compute selects CTA");
        assert!(p[12..16].iter().all(|byte| *byte == 0), "ABI padding stays zero");
        assert!(p[16..88].iter().all(|byte| *byte == 0), "all nine guest pointer positions stay zero");
        assert!(p[96..112].iter().all(|byte| *byte == 0), "route stays zero");
    }
}

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

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use nvrm_abi::sys;
use nvrm_client::mem::CpuMap;

use crate::channel::{Channel, GpuMem, Registry};
use crate::chip::ChipData;
use crate::fb::FakeFb;
use crate::fb::Target;
use crate::guestmem::GuestMemory;
use crate::hostmem::{HostMemory, OsEvent};
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
type Built = (u32, Vec<(u32, u32)>, u32);

/// The host side of one guest channel.
pub struct HostChannel {
    pub handle: u32,
    pub tsg: TsgKey,
    pub token: u32,
    /// `(class, host handle)` of the replayed engine objects.
    pub objects: Vec<(u32, u32)>,
    pub created: Instant,
    pub doorbells: u64,
    /// The guest root of the shadow VAS a shadow GR channel runs on (`None`: CE mirror).
    pub shadow_root: Option<u64>,
    /// The guest's GPPut when the device last forwarded this channel's doorbell (untrapped.rs
    /// `ran_ahead`).
    pub fwd_put: Option<u32>,
    /// `Mirror::unwaited` (the count of plans submitted without a wait) at that forward.
    pub fwd_unwaited: u64,
    /// The host memory objects made for this channel over guest pages (the USERD and
    /// error-notifier OS descriptors), freed after the channel.
    pub descriptors: Vec<u32>,
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
    reservation: u32,
    vas: BTreeMap<(u32, u32), HostVas>,
    /// By [`TsgKey`]: the kernel's TSG id on a 615 guest, the guest handle on 610.
    tsgs: BTreeMap<TsgKey, HostTsg>,
    /// By guest ChID.
    channels: BTreeMap<u32, HostChannel>,
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
    /// Invalidates whose plans were submitted without a wait before the guest continued
    /// (eager or at the lazy deadline, not held): a host channel that runs ahead of its
    /// doorbell after one of them may run on a mirror the worker is still applying.
    unwaited: u64,
    /// The registered guest RAM descriptors `(handle, iova, size)`, read once.
    descs: Option<Vec<(u32, u64, u64)>>,
    /// Video objects the guest allocated on a mirrored shadow channel, not yet placed on
    /// the host: `(class, params)` by ChID ([`Mirror::place_video_object`];
    /// docs/caraxes/video-engines.md §3).
    video_pending: BTreeMap<u32, (u32, Vec<u8>)>,
    next_event_id: u32,
    pub stats: MirrorStats,
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
    ) -> Result<Self, String> {
        let (base, end, reservation) = {
            let h = hm.lock().unwrap();
            let r = h
                .reservation()
                .ok_or("the mirror needs the VRAM reservation (--reserve-vram)")?;
            (r.base, r.end(), r.handle)
        };
        let worker = ShadowWorker::new(hm.clone(), intr.clone());
        Ok(Mirror {
            hm,
            intr,
            chip,
            base,
            end,
            reservation,
            vas: BTreeMap::new(),
            tsgs: BTreeMap::new(),
            channels: BTreeMap::new(),
            // GR channels on the host (the c2 experiment, exec-formats.md §4.5): the
            // replayed shared-management VAS plus a second `GPU_PROMOTE_CTX` on the host
            // channel carrying the guest's own recorded entries. Off by default (the GR
            // context wall); `CARAXES_MIRROR_GR=1` turns it and the VAS replay on.
            replay_vas: std::env::var("CARAXES_MIRROR_GR").as_deref() == Ok("1"),
            mirror_gr: std::env::var("CARAXES_MIRROR_GR").as_deref() == Ok("1"),
            mirror_ce: std::env::var("CARAXES_MIRROR_CE").as_deref() != Ok("0"),
            channels_enabled: std::env::var("CARAXES_MIRROR_CHANNELS").as_deref() == Ok("1"),
            doorbell: None,
            events: None,
            invalidator: None,
            memop_invalidate: std::env::var("CARAXES_MEMOP_INVALIDATE").as_deref() != Ok("0"),
            shadow_probe: std::env::var("CARAXES_MIRROR_GR_SHADOW").as_deref() == Ok("1"),
            bare_ce: std::env::var("CARAXES_MIRROR_BARE_CE").as_deref() == Ok("1"),
            ce_shadow: std::env::var("CARAXES_CE_SHADOW").as_deref() != Ok("0"),
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
            unwaited: 0,
            descs: None,
            video_pending: BTreeMap::new(),
            next_event_id: 0x1000,
            stats: MirrorStats::default(),
        })
    }

    pub fn is_mirrored(&self, chid: u32) -> bool {
        self.channels.contains_key(&chid)
    }

    /// The size of a global GR context buffer (by `NV2080_CTRL_GPU_PROMOTE_CTX_BUFFER_ID_*`)
    /// on the host: `NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES` (NON_PRIVILEGED)
    /// on the device's own host device, rounded up to 64 KiB.
    fn global_ctx_size(&self, buffer_id: u16) -> Option<u64> {
        // ctrl0080fifo.h ENGINE_ID_GRAPHICS_*: BUNDLE_CB 0x11, PAGEPOOL 0x0d, ATTRIBUTE_CB
        // 0x13, RTV_CB_GLOBAL 0x14, GFXP_POOL 0x15, GFXP_CTRL_BLK 0x16, FECS_EVENT 0x17,
        // PRIV_ACCESS_MAP 0x18.
        let engine_id: u32 = match buffer_id {
            3 => 0x11,
            4 => 0x0d,
            5 => 0x13,
            6 => 0x14,
            7 => 0x15,
            8 => 0x16,
            9 => 0x17,
            10..=12 => 0x18,
            _ => return None,
        };
        let h = self.hm.lock().unwrap();
        let mut p = [0u8; 12];
        p[..4].copy_from_slice(&engine_id.to_le_bytes());
        let st = h.control(h.device(), 0x0080_1707, &mut p);
        let size = u32::from_le_bytes(p[8..12].try_into().unwrap()) as u64;
        (st == sys::NV_OK && size != 0).then(|| (size + 0xFFFF) & !0xFFFF)
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

    /// Re-sync the shadow VAS of guest root `root` (`None`: every shadow VAS) from the
    /// guest's tables (called at a guest TLB invalidate and before a shadow doorbell): map
    /// what the guest newly mapped, unmap what it changed or removed.
    pub fn shadow_resync(
        &mut self,
        _reg: &Registry,
        gm: &mut GpuMem,
        root: Option<u64>,
    ) -> Vec<String> {
        let (base, end, reservation) = (self.base, self.end, self.reservation);
        let descs = self.descs();
        let mut out = Vec::new();
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
            self.worker.wait_all();
            self.hold.note_held(t0.elapsed());
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
        self.hold.summary(self.hold_on)
    }

    /// Whether plans are pending: deferred by lazy mode, or submitted and not yet applied.
    pub fn plans_pending(&self) -> bool {
        self.lazy.is_pending() || self.worker.pending()
    }

    /// The trapped GPPut store's barrier (boot.rs `gp_put_barrier`): plan what lazy mode
    /// deferred and wait until the worker applied every plan.
    pub fn plans_barrier(&mut self, reg: &Registry, gm: &mut GpuMem) -> Vec<String> {
        let notes = self.flush_lazy(reg, gm);
        self.worker.wait_all();
        notes
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
        Some((
            gm.read_u32(at(USERD_GP_GET)?)?,
            gm.read_u32(at(USERD_GP_PUT)?)?,
            ch.gpfifo_entries,
        ))
    }

    /// At a doorbell of a built shadow channel, before anything is planned: count it when
    /// the host channel already fetched entries past the last forward (untrapped.rs
    /// `ran_ahead`), and log it with the wall time when plans were pending.
    fn note_ran_ahead(&mut self, reg: &Registry, gm: &GpuMem, chid: u32) -> Option<String> {
        let hc = self.channels.get(&chid)?;
        let (fwd, since) = (hc.fwd_put?, self.unwaited - hc.fwd_unwaited);
        let (get, put, n) = Self::userd_ring(reg, gm, chid)?;
        if !crate::untrapped::ran_ahead(fwd, get, put, n) {
            return None;
        }
        // Plans deferred now, being applied now, or submitted without a wait since the last
        // forward: the entries the host channel fetched may have met an older mirror.
        let pending = self.plans_pending() || since > 0;
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
            let h = self.hm.lock().unwrap();
            let d: Vec<(u32, u64, u64)> = h
                .descriptors()
                .iter()
                .map(|d| (d.handle, d.range.iova, d.range.size))
                .collect();
            if !d.is_empty() {
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
            .filter(|(_, sp)| sp.chids.is_empty() && sp.tsgs.is_empty())
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
                if let Some(fb) = fb {
                    fb.unwatch_tables(Target::Fb(r));
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
        // Evidence (untrapped.rs): did the host channel fetch entries this doorbell has not
        // forwarded yet, and were plans pending then?
        let ahead_line = self.note_ran_ahead(reg, gm, chid);
        // CARAXES_ALLOC_LAZY: the invalidates deferred since the last doorbell are planned
        // here, before the barrier below or the creation's own wait.
        let lazy_notes = self.flush_lazy(reg, gm);
        // Already built: re-sync the shadow VAS from the guest root FIRST (the guest maps
        // more set-A between doorbells), then flush and ring.
        if self.channels.contains_key(&chid) {
            let root = self.channels.get(&chid).and_then(|c| c.shadow_root);
            let mut notes = lazy_notes;
            notes.extend(ahead_line);
            // Context ranges promoted since the build (a video engine object the guest
            // allocated after the channel's first doorbell) leave set A before the resync.
            if let (Some(r), Some(ch)) = (root, reg.channel(chid)) {
                let ranges = promoted_ranges(reg, ch, |id| self.global_ctx_size(id));
                if let Some(sp) = self.shadows.get_mut(&r) {
                    for x in ranges {
                        if !sp.exclude.contains(&x) {
                            notes.push(format!(
                                "shadow: chid {chid} promoted context [{:#x}, {:#x}) kept out of set A [guest]",
                                x.0, x.1
                            ));
                            sp.exclude.push(x);
                        }
                    }
                }
            }
            notes.extend(self.shadow_resync(reg, gm, root));
            // Ordering: the doorbell is forwarded only after the worker applied every plan
            // submitted so far (the mirror is consistent for this root).
            self.worker.wait_all();
            gm.fb.flush();
            let put = Self::userd_ring(reg, gm, chid).map(|(_, p, _)| p);
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
        let root = match reg.vas_key_of(&ch).and_then(|k| reg.vas_root(k)) {
            Some(Target::Fb(pa)) if pa >= self.base && pa + PAGE <= self.end => pa,
            other => {
                return vec![format!(
                    "shadow: chid {chid} root {other:?} not in the reservation"
                )]
            }
        };
        // Accumulate the promoted-context (set-B) VA ranges across every GR channel: the
        // host RM builds set B in the shadow VAS at these VAs, so a later GR channel that
        // records no promote entries of its own must still not have set A mirrored over
        // them (measured 2026-09-23: chids 3-9 collided at 0x120010000 etc.).
        // A global buffer (the 3D class's bundle/attribute/RTV CBs, the pagepool) is
        // promoted by VA only, size 0; its extent is the host's context-buffer size for the
        // same silicon (display-e.md: the guest's attribute CB at 0x120400000 was mirrored
        // as set A and the host's own collided with it, Xid 31 PRIV_VIOLATION).
        let new_b = promoted_ranges(reg, &ch, |id| self.global_ctx_size(id));
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
        if let std::collections::btree_map::Entry::Vacant(slot) = self.shadows.entry(root) {
            match crate::shadow::ShadowVas::new(&mut h, self.map_budget.clone()) {
                Ok(sv) => {
                    notes.push(format!(
                        "shadow: host-managed VAS {:#x} for guest root {root:#x} [host]",
                        sv.vas
                    ));
                    let vas = sv.vas;
                    self.worker.submit(Job::Create(root, Box::new(sv)));
                    slot.insert(ShadowSpace {
                        front: crate::shadow::ShadowFront::new(root),
                        vas,
                        exclude: Vec::new(),
                        chids: Default::default(),
                        tsgs: Default::default(),
                    });
                }
                Err(st) => {
                    notes.push(format!("shadow: VAS refused: {}", status_name(st)));
                    return notes;
                }
            }
        }
        let sp = self.shadows.get_mut(&root).expect("built");
        for r in new_b {
            if !sp.exclude.contains(&r) {
                sp.exclude.push(r);
            }
        }
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
        if let Err(e) = self.check_tsg_reuse(reg, tsg_key, tsg_guest, &mut notes) {
            notes.push(format!("shadow: chid {chid}: {e}"));
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
                        vas: tsg_guest,
                        engine: engine_type,
                        guest: tsg_guest,
                    },
                );
                tsg
            }
        };
        // P21 design H: a USERD page the guest maps into the BAR1 pool area lives at its
        // pool page (fb.rs `remap`).
        let mut descriptors: Vec<u32> = Vec::new();
        let userd = match ch.userd.map(|t| match t {
            Target::Fb(pa) => Target::Fb(gm.fb.remap(pa)),
            t => t,
        }) {
            Some(Target::Fb(pa)) if pa >= self.base && pa + PAGE <= self.end => {
                (self.reservation, pa - self.base)
            }
            // USERD in guest sysmem (the guest's RMInstLoc USERD=COH, docs/caraxes/mapping.md
            // §6): the host memory object over the guest's own page, as for a CE channel.
            Some(Target::Sys(iova)) => {
                match gm
                    .sys
                    .translate(iova & !(PAGE - 1), PAGE)
                    .and_then(|gpa| gm.sys.host_ptr(gpa, PAGE))
                    .map(|ptr| h.guest_page_object(ptr, PAGE, "shadow-userd-page"))
                {
                    Some(Ok(d)) => {
                        descriptors.push(d);
                        (d, iova & (PAGE - 1))
                    }
                    other => {
                        notes.push(format!(
                            "shadow: chid {chid} USERD IOVA {iova:#x} not registered guest RAM ({:?})",
                            other.map(|r| r.map_err(|e| e.to_string()))
                        ));
                        return notes;
                    }
                }
            }
            other => {
                notes.push(format!(
                    "shadow: chid {chid} USERD {other:?} not a vidmem reservation page"
                ));
                return notes;
            }
        };
        // The error notifier: the guest's page, so the host RM writes an RC there and the
        // guest sees the fault (returns 719) instead of hanging to the 30 s watchdog (the
        // CE path does this; without it launch5 hung on the MMU fault).
        let h_error = match ch.error_notifier {
            Some(Target::Sys(iova)) if iova % PAGE == 0 => gm
                .sys
                .translate(iova, PAGE)
                .and_then(|gpa| gm.sys.host_ptr(gpa, PAGE))
                .and_then(|ptr| h.guest_page_object(ptr, PAGE, "shadow-error-notifier").ok())
                .inspect(|d| descriptors.push(*d))
                .unwrap_or(0),
            _ => 0,
        };
        let mut cp = crate::hostabi::ChannelAllocParams::default();
        cp.hObjectError = h_error;
        cp.gpFifoOffset = ch.gpfifo_va;
        cp.gpFifoEntries = ch.gpfifo_entries;
        cp.hUserdMemory[0] = userd.0;
        cp.userdOffset[0] = userd.1;
        cp.engineType = ch.engine_type;
        let channel = match h.alloc_object(
            tsg,
            crate::board::host_class_of(self.chip.channel.gpfifo),
            as_bytes(&mut cp),
            "shadow-channel",
        ) {
            Ok(x) => x,
            Err(st) => {
                notes.push(format!(
                    "shadow: channel refused: {} (gpfifo va {:#x})",
                    status_name(st),
                    ch.gpfifo_va
                ));
                for d in descriptors {
                    let _ = h.free_object(d);
                }
                return notes;
            }
        };
        notes.push(format!("shadow: host GR channel {channel:#x} in TSG {tsg:#x} on VAS {vas_handle:#x}, gpfifo va {:#x} [host]", ch.gpfifo_va));
        // The guest's engine objects; the compute object makes the host RM build set B,
        // a video object the host's video context (kchannelMapEngineCtxBuf,
        // kernel_channel.c:3860-3870: first fit in the shadow VAS, where the guest's own
        // context page is already unmapped, `promoted_ranges`).
        let mut objects = Vec::new();
        for (class, params) in &ch.engine_objects {
            let mut p = params.clone();
            match h.alloc_object(channel, crate::board::host_class_of(*class), &mut p, "shadow-engine") {
                Ok(oh) => {
                    notes.push(format!(
                        "shadow: chid {chid} host object class {class:#x}{} {oh:#x} [host]",
                        crate::board::mapped_note(*class)
                    ));
                    objects.push((*class, oh));
                }
                Err(e) => notes.push(format!(
                    "shadow: chid {chid} object class {class:#x} refused: {}",
                    status_name(e)
                )),
            }
        }
        if objects.is_empty() {
            // A video channel rings before its engine object exists, and the guest waits
            // for that first push before it allocates the object (a held doorbell
            // deadlocked for 10 s, 2026-09-25 probe-hold). The host channel is bound to
            // the engine instead; the object follows at the guest's allocation
            // (`place_video_object`).
            let Some(engine) = video_bind(self.chip, &ch) else {
                notes.push("shadow: no engine object; not scheduling".into());
                return notes;
            };
            let mut p = engine.to_le_bytes();
            let st = h.control(channel, CMD_CHANNEL_BIND, &mut p);
            if st != sys::NV_OK {
                notes.push(format!(
                    "shadow: chid {chid} BIND engineType {engine:#x}: {}; not scheduling",
                    status_name(st)
                ));
                return notes;
            }
            notes.push(format!(
                "shadow: chid {chid} host channel bound to engineType {engine:#x} ({}; its object follows the guest's) [host]",
                video_engine(engine).unwrap_or("engine")
            ));
        }
        let mut sched = [1u8, 0, 0];
        let st = h.control(channel, CMD_GPFIFO_SCHEDULE, &mut sched);
        if st != sys::NV_OK {
            notes.push(format!("shadow: GPFIFO_SCHEDULE: {}", status_name(st)));
            return notes;
        }
        let mut tok = [0u8; 4];
        let st = h.control(channel, CMD_GET_WORK_SUBMIT_TOKEN, &mut tok);
        if st != sys::NV_OK {
            notes.push(format!(
                "shadow: GET_WORK_SUBMIT_TOKEN: {}",
                status_name(st)
            ));
            return notes;
        }
        let token = u32::from_le_bytes(tok);
        self.stats.channels_created += 1;
        self.channels.insert(
            chid,
            HostChannel {
                handle: channel,
                tsg: tsg_key,
                token,
                objects,
                created: Instant::now(),
                doorbells: 0,
                shadow_root: Some(root),
                fwd_put: None,
                fwd_unwaited: 0,
                descriptors: std::mem::take(&mut descriptors),
            },
        );
        notes.push(format!(
            "shadow: chid {chid} scheduled; host token {token:#x} (host chid {}, runlist {}) [host]",
            token & 0xFFF,
            (token >> 16) & 0x7F
        ));
        drop(h);
        // Restore the guest's GPGet/GPPut the host RM cleared at channel creation.
        if let Some((get_at, put_at, Some(get), Some(put))) = saved_userd {
            if gm.read_u32(put_at) != Some(put) {
                gm.write(get_at, &get.to_le_bytes());
                gm.write(put_at, &put.to_le_bytes());
                notes.push(format!(
                    "shadow: chid {chid} restored guest GPGet {get} / GPPut {put} (host RM had cleared the USERD) [guest]"
                ));
            }
        }
        gm.fb.flush();
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

    pub fn channel(&self, chid: u32) -> Option<&HostChannel> {
        self.channels.get(&chid)
    }

    pub fn mirrored_channels(&self) -> usize {
        self.channels.len()
    }

    /// Whether the registry's channel `chid` is one the mirror takes: a child of a
    /// guest TSG whose VA space has a known root inside the reservation.
    pub fn eligible(&self, reg: &Registry, chid: u32) -> Result<(u64, (u32, u32)), String> {
        if !self.channels_enabled {
            return Err("host channel execution off (CARAXES_MIRROR_CHANNELS=1 to enable); channel work runs on the software executor over the VRAM store".into());
        }
        let ch = reg.channel(chid).ok_or("no such channel")?;
        let gr0 = v::lookup(v::NV2080_ENGINE_TYPES, "GR0").unwrap_or(1);
        // A bare video channel (libnvidia-encode's and libnvcuvid's, under the device with
        // no TSG and no VAS of its own) is mirrored in a host TSG of its own, as a bare CE
        // channel is under CARAXES_MIRROR_BARE_CE.
        let video = video_engine(ch.engine_type);
        if reg.tsg((ch.h_client, ch.h_parent)).is_none()
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
            let mut cp = crate::hostabi::ChannelAllocParams::default();
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
            for (class, params) in &ch.engine_objects {
                let mut p = params.clone();
                match h.alloc_object(handle, crate::board::host_class_of(*class), &mut p, "engine-object") {
                    Ok(oh) => {
                        notes.push(format!(
                            "chid {chid}: host object class {class:#x}{} {oh:#x} ({} bytes of the guest's params) [host]",
                            crate::board::mapped_note(*class),
                            params.len()
                        ));
                        objects.push((*class, oh));
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
                let (st, note) = promote_gr(&mut h, handle, &promote_entries);
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
            Ok((handle, objects, u32::from_le_bytes(tok)))
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
            Ok((handle, objects, token)) => {
                drop(h);
                let descriptors: Vec<u32> = owned.iter().copied().filter(|d| *d != handle).collect();
                restore_userd(gm, &mut notes);
                let us = t0.elapsed().as_micros();
                self.stats.channels_created += 1;
                self.stats.max_create_us = self.stats.max_create_us.max(us);
                notes.push(format!(
                    "chid {chid}: scheduled; host token {token:#x} (host chid {}, runlist {}) for guest token chid {chid}; mirror built in {us} us [host]",
                    token & 0xFFF,
                    (token >> 16) & 0x7F
                ));
                self.channels.insert(
                    chid,
                    HostChannel {
                        handle,
                        tsg: tsg_key,
                        token,
                        objects,
                        created: t0,
                        doorbells: 0,
                        shadow_root: None,
                        fwd_put: None,
                        fwd_unwaited: 0,
                        descriptors,
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

    /// An engine object the guest allocated on an already mirrored channel.
    pub fn add_engine_object(&mut self, chid: u32, class: u32, params: &[u8]) -> Option<String> {
        let hc = self.channels.get_mut(&chid)?;
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
            self.video_pending.insert(chid, (class, params.to_vec()));
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
                format!("chid {chid}: host object class {class:#x} {oh:#x} added [host]")
            }
            Err(e) => format!(
                "chid {chid}: host object class {class:#x} refused: {}",
                status_name(e)
            ),
        })
    }

    /// The deferred video object of shadow channel `chid` (`add_engine_object`): the
    /// guest's context range (its `GPU_PROMOTE_CTX`, sent before the object's allocation,
    /// falcon/kernel_falcon.c:294-298) leaves set A first, the worker unmaps it and is
    /// waited for, and only then the host object is allocated. The host RM maps its video
    /// context first fit (kchannelMapEngineCtxBuf, kernel_channel.c:3860-3870), so it lands
    /// in the hole the guest's own context left instead of a VA the guest fills later
    /// (Xid 31 FAULT_PTE on NVENC0 at 0x1205fe000, probe-mirror1).
    pub fn place_video_object(&mut self, reg: &Registry, gm: &mut GpuMem, chid: u32) -> Vec<String> {
        let Some((class, params)) = self.video_pending.remove(&chid) else {
            return Vec::new();
        };
        let mut notes = Vec::new();
        let (Some(hc), Some(ch)) = (self.channels.get(&chid), reg.channel(chid)) else {
            return vec![format!("chid {chid}: video object {class:#x}: the channel went away")];
        };
        let (handle, root) = (hc.handle, hc.shadow_root);
        let ranges = promoted_ranges(reg, ch, |id| self.global_ctx_size(id));
        if ranges.is_empty() {
            return vec![format!(
                "chid {chid}: video object {class:#x} refused: the guest promoted no context for it, so the host context has no hole to land in"
            )];
        }
        if let Some(sp) = root.and_then(|r| self.shadows.get_mut(&r)) {
            for x in ranges {
                if !sp.exclude.contains(&x) {
                    notes.push(format!(
                        "shadow: chid {chid} guest context [{:#x}, {:#x}) leaves set A before the host's video object [guest]",
                        x.0, x.1
                    ));
                    sp.exclude.push(x);
                }
            }
        }
        notes.extend(self.shadow_resync(reg, gm, root));
        self.worker.wait_all();
        let mut p = params;
        let r = self
            .hm
            .lock()
            .unwrap()
            .alloc_object(handle, crate::board::host_class_of(class), &mut p, "video-object");
        notes.push(match r {
            Ok(oh) => {
                if let Some(hc) = self.channels.get_mut(&chid) {
                    hc.objects.push((class, oh));
                }
                format!("chid {chid}: host video object class {class:#x} {oh:#x} [host]")
            }
            Err(e) => format!(
                "chid {chid}: host video object class {class:#x} refused: {}",
                status_name(e)
            ),
        });
        notes
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
        self.stats.doorbells += 1;
        let hc = self.channels.get_mut(&chid).expect("present");
        hc.doorbells += 1;
        Ok(format!(
            "doorbell: guest chid {chid} -> host token {token:#x} written to the host doorbell page ({} on this channel) [host]",
            hc.doorbells
        ))
    }

    fn ensure_invalidator(&mut self, fb: &mut FakeFb, notes: &mut Vec<String>) {
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
        Some((
            st,
            format!(
                "NV0080 FIFO_IDLE_CHANNELS on the mirror's host device for {}: {} in {} us [host]",
                chids.join(", "),
                status_name(st),
                t0.elapsed().as_micros()
            ),
        ))
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

    fn free_channel(&mut self, chid: u32, notes: &mut Vec<String>) {
        for sp in self.shadows.values_mut() {
            sp.chids.remove(&chid);
        }
        if let Some(hc) = self.channels.remove(&chid) {
            let mut h = self.hm.lock().unwrap();
            let r = h.free_object(hc.handle);
            // The descriptors over guest pages the channel used, after the channel.
            let desc_failed = hc
                .descriptors
                .iter()
                .filter(|d| h.free_object(**d).is_err())
                .count();
            drop(h);
            if desc_failed != 0 {
                notes.push(format!(
                    "chid {chid}: ERROR {desc_failed} of {} guest-page descriptor(s) not freed",
                    hc.descriptors.len()
                ));
            }
            self.stats.channels_freed += 1;
            notes.push(match r {
                Ok(()) => format!(
                    "chid {chid}: host channel {:#x} freed after {} doorbell(s), {} ms of life [host]",
                    hc.handle,
                    hc.doorbells,
                    hc.created.elapsed().as_millis()
                ),
                Err(st) => format!("chid {chid}: ERROR freeing host channel {:#x}: {}", hc.handle, status_name(st)),
            });
        }
    }

    fn free_tsg(&mut self, key: TsgKey, notes: &mut Vec<String>) {
        let chids: Vec<u32> = self
            .channels
            .iter()
            .filter(|(_, c)| c.tsg == key)
            .map(|(c, _)| *c)
            .collect();
        for c in chids {
            self.free_channel(c, notes);
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
        }
        for sp in self.shadows.values_mut() {
            sp.tsgs.remove(&key);
        }
        self.reap_shadows(None, notes);
    }

    fn drop_vas(&mut self, key: (u32, u32), notes: &mut Vec<String>) {
        let tsgs: Vec<TsgKey> = self
            .tsgs
            .iter()
            .filter(|(_, t)| t.vas == key)
            .map(|(k, _)| *k)
            .collect();
        for t in tsgs {
            self.free_tsg(t, notes);
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

    /// The guest freed `h_object` of `h_client` (or the client itself): free the host
    /// mirrors that stood for it, channels before TSGs before VA spaces.
    pub fn free_object(&mut self, reg: &Registry, h_client: u32, h_object: u32) -> Vec<String> {
        let mut notes = Vec::new();
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
        for sp in self.shadows.values_mut() {
            sp.chids.clear();
            sp.tsgs.clear();
        }
        self.reap_shadows(None, &mut notes);
        // Every teardown queued above finishes before the descriptors can be freed.
        self.worker.wait_idle();
        notes.extend(self.worker.take_notes());
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
            "mirror: {} channel(s) created, {} freed, {} live; {} doorbell(s) forwarded, {} TLB invalidate(s) ({} MEM_OP push(es), {} slow), {} refusal(s), longest mirror build {} us; events raised {}, MSIs {} (coalesced {}), host RC events {}{}; {}",
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
            self.hold.summary(self.hold_on)
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
    done: Arc<(Mutex<u64>, std::sync::Condvar)>,
    submitted: u64,
    enabled: bool,
    notes: Arc<Mutex<Vec<String>>>,
    /// No teardown in progress.
    idle: Arc<(Mutex<bool>, std::sync::Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ShadowWorker {
    fn new(hm: Arc<Mutex<HostMemory>>, intr: Arc<IntrShared>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<(u64, Job)>();
        let done = Arc::new((Mutex::new(0u64), std::sync::Condvar::new()));
        let notes = Arc::new(Mutex::new(Vec::new()));
        let (done2, notes2) = (done.clone(), notes.clone());
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
                    };
                    for l in lines {
                        intr.note(l);
                    }
                    let (m, cv) = &*done2;
                    *m.lock().unwrap() = seq;
                    cv.notify_all();
                }
            })
            .ok();
        ShadowWorker {
            tx: Some(tx),
            done,
            idle,
            submitted: 0,
            enabled: std::env::var("CARAXES_SHADOW_WORKER").as_deref() != Ok("0"),
            notes,
            thread,
        }
    }

    fn submit(&mut self, job: Job) {
        self.submitted += 1;
        if let Some(tx) = &self.tx {
            let _ = tx.send((self.submitted, job));
        }
        if !self.enabled {
            self.wait_all();
        }
    }

    /// Jobs submitted and not yet done.
    fn pending(&self) -> bool {
        self.thread.is_some() && *self.done.0.lock().unwrap() < self.submitted
    }

    /// Block until every job submitted so far is done.
    fn wait_all(&self) {
        if self.thread.is_none() {
            return;
        }
        let (m, cv) = &*self.done;
        let mut d = m.lock().unwrap();
        while *d < self.submitted {
            d = cv.wait(d).unwrap();
        }
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
) -> (u32, String) {
    if entries.is_empty() {
        return (
            0xFFFF_FFFF,
            format!("chid mirror {host_channel:#x}: no recorded promote entries; GR context not promoted (c2)"),
        );
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
    while !stop.load(Ordering::SeqCst) {
        for f in fds.iter_mut() {
            f.revents = 0;
        }
        // SAFETY: a pollfd array over fds this thread owns.
        let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 250) };
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
                    let msi = intr.raise(*vec);
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

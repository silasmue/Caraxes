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
/// `NV01_EVENT_NONSTALL_INTR` (nvos.h:435).
pub const EVENT_NONSTALL: u32 = 0x0800_0000;
/// The engines whose non-stall interrupts are forwarded: (MC_ENGINE_IDX name, notifier).
pub const ENGINES: &[(&str, u32)] = &[
    ("GR0", NOTIFIER_GRAPHICS),
    ("CE2", NOTIFIER_CE2),
    ("CE3", NOTIFIER_CE3),
    ("CE4", NOTIFIER_CE4),
];

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
    sv: crate::shadow::ShadowVas,
    exclude: Vec<(u64, u64)>,
    chids: std::collections::BTreeSet<u32>,
    tsgs: std::collections::BTreeSet<(u32, u32)>,
}

struct HostTsg {
    handle: u32,
    vas: (u32, u32),
    engine: u32,
}

/// What one channel build yields: the host channel, its engine objects, the token.
type Built = (u32, Vec<(u32, u32)>, u32);

/// The host side of one guest channel.
pub struct HostChannel {
    pub handle: u32,
    pub tsg: (u32, u32),
    pub token: u32,
    /// `(class, host handle)` of the replayed engine objects.
    pub objects: Vec<(u32, u32)>,
    pub created: Instant,
    pub doorbells: u64,
    /// The guest root of the shadow VAS a shadow GR channel runs on (`None`: CE mirror).
    pub shadow_root: Option<u64>,
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
    tsgs: BTreeMap<(u32, u32), HostTsg>,
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
    /// One shadow VAS per guest root (per CUDA context: two processes use the same VAs
    /// for different pages, measured in the gpu gate's robustness stage), built at the
    /// first GR channel of that root and freed when its last channel and TSG are gone.
    shadows: BTreeMap<u64, ShadowSpace>,
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
            shadows: BTreeMap::new(),
            next_event_id: 0x1000,
            stats: MirrorStats::default(),
        })
    }

    pub fn is_mirrored(&self, chid: u32) -> bool {
        self.channels.contains_key(&chid)
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
        let mut h = self.hm.lock().unwrap();
        let mut out = Vec::new();
        for (r, sp) in self.shadows.iter_mut() {
            if root.is_some_and(|x| x != *r) {
                continue;
            }
            let (changed, notes) = sp.sv.sync(
                &mut h,
                gm.fb,
                gm.sys,
                *r,
                base,
                end,
                reservation,
                &sp.exclude,
            );
            if changed {
                out.extend(notes);
            }
        }
        out
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
                let mut h = self.hm.lock().unwrap();
                notes.push(format!(
                    "shadow: VAS {:#x} of guest root {r:#x} freed: {} [host]",
                    sp.sv.vas,
                    sp.sv.summary()
                ));
                sp.sv.free(&mut h);
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
        // Already built: re-sync the shadow VAS from the guest root FIRST (the guest maps
        // more set-A between doorbells), then flush and ring.
        if self.channels.contains_key(&chid) {
            let root = self.channels.get(&chid).and_then(|c| c.shadow_root);
            let mut notes = self.shadow_resync(reg, gm, root);
            gm.fb.flush();
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
        let mut new_b = Vec::new();
        if let Some(m) = reg.promoted((ch.h_client, ch.h_object)) {
            for e in m.values() {
                if let Some(v) = e.virt {
                    new_b.push((v, v + e.size.max(0x1000)));
                }
            }
        }
        let tsg_key = (ch.h_client, ch.h_parent);
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
        let mut notes = vec![format!(
            "shadow: GR chid {chid} on the host-managed shadow VAS of guest root {root:#x}"
        )];
        let mut h = self.hm.lock().unwrap();
        // The shadow VAS of this guest root, built once per root.
        if let std::collections::btree_map::Entry::Vacant(slot) = self.shadows.entry(root) {
            match crate::shadow::ShadowVas::new(&mut h) {
                Ok(sv) => {
                    notes.push(format!(
                        "shadow: host-managed VAS {:#x} for guest root {root:#x} [host]",
                        sv.vas
                    ));
                    slot.insert(ShadowSpace {
                        sv,
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
        let sv = &mut sp.sv;
        // Mirror the guest's set-A before the channel/GPFIFO VA is validated against the VAS.
        let (_, sync_notes) = sv.sync(
            &mut h,
            gm.fb,
            gm.sys,
            root,
            self.base,
            self.end,
            self.reservation,
            &exclude,
        );
        notes.extend(sync_notes);
        let vas_handle = sv.vas;
        // One host TSG per guest TSG, on the shadow VAS (GR0): the guest's GR channels
        // of one group (CUDA's chids 2-9, one TSG, one context share) share one GR
        // context on the host too, so state a push sets on one channel is what a launch
        // on another sees (the guest's layout; before, every channel got its own TSG and
        // context, and `self.tsgs` kept only the last handle).
        let tsg = match self.tsgs.get(&tsg_key) {
            Some(t) => {
                notes.push(format!(
                    "shadow: chid {chid} joins host TSG {:#x} of guest TSG {:#x} [host]",
                    t.handle, tsg_key.1
                ));
                t.handle
            }
            None => {
                let mut tp = sys::v610::NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS::default();
                tp.hVASpace = vas_handle;
                tp.engineType = v::lookup(v::NV2080_ENGINE_TYPES, "GR0").unwrap_or(1);
                let device = h.device();
                let tsg = match h.alloc_object(
                    device,
                    sys::KEPLER_CHANNEL_GROUP_A,
                    as_bytes(&mut tp),
                    "shadow-tsg",
                ) {
                    Ok(x) => x,
                    Err(st) => {
                        notes.push(format!("shadow: TSG refused: {}", status_name(st)));
                        return notes;
                    }
                };
                notes.push(format!(
                    "shadow: host TSG {tsg:#x} for guest TSG {:#x} on VAS {vas_handle:#x} [host]",
                    tsg_key.1
                ));
                self.tsgs.insert(
                    tsg_key,
                    HostTsg {
                        handle: tsg,
                        vas: tsg_key,
                        engine: tp.engineType,
                    },
                );
                tsg
            }
        };
        let userd = match ch.userd {
            Some(Target::Fb(pa)) if pa >= self.base && pa + PAGE <= self.end => {
                (self.reservation, pa - self.base)
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
                .unwrap_or(0),
            _ => 0,
        };
        let mut cp = sys::v610::NV_CHANNEL_ALLOC_PARAMS::default();
        cp.hObjectError = h_error;
        cp.gpFifoOffset = ch.gpfifo_va;
        cp.gpFifoEntries = ch.gpfifo_entries;
        cp.hUserdMemory[0] = userd.0;
        cp.userdOffset[0] = userd.1;
        cp.engineType = ch.engine_type;
        let channel = match h.alloc_object(
            tsg,
            self.chip.channel.gpfifo,
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
                return notes;
            }
        };
        notes.push(format!("shadow: host GR channel {channel:#x} in TSG {tsg:#x} on VAS {vas_handle:#x}, gpfifo va {:#x} [host]", ch.gpfifo_va));
        // The guest's engine objects; the compute object makes the host RM build set B.
        let mut objects = Vec::new();
        for (class, params) in &ch.engine_objects {
            let mut p = params.clone();
            match h.alloc_object(channel, *class, &mut p, "shadow-engine") {
                Ok(oh) => {
                    notes.push(format!(
                        "shadow: chid {chid} host object class {class:#x} {oh:#x} [host]"
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
            notes.push("shadow: no engine object; not scheduling".into());
            return notes;
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
        if let Some(userd) = ch.userd {
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
        if reg.tsg((ch.h_client, ch.h_parent)).is_none()
            && !(self.bare_ce && ch.engine_type != gr0 && ch.engine_type != 0)
        {
            return Err("bare channel (no TSG): stays with the software executor".into());
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

    fn ensure_tsg(
        &mut self,
        key: (u32, u32),
        vas_key: (u32, u32),
        vas_handle: u32,
        engine: u32,
        notes: &mut Vec<String>,
    ) -> Result<u32, String> {
        if let Some(t) = self.tsgs.get(&key) {
            return Ok(t.handle);
        }
        let mut h = self.hm.lock().unwrap();
        let mut tp = sys::v610::NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS::default();
        tp.hVASpace = vas_handle;
        tp.engineType = engine;
        let device = h.device();
        let handle = h
            .alloc_object(
                device,
                sys::KEPLER_CHANNEL_GROUP_A,
                as_bytes(&mut tp),
                "tsg",
            )
            .map_err(|st| {
                format!(
                    "host KEPLER_CHANNEL_GROUP_A engineType {engine:#x}: {}",
                    status_name(st)
                )
            })?;
        notes.push(format!(
            "tsg {:#x}/{:#x}: host TSG {handle:#x} engineType {engine:#x} on vaspace {vas_handle:#x} [host]",
            key.0, key.1
        ));
        drop(h);
        self.tsgs.insert(
            key,
            HostTsg {
                handle,
                vas: vas_key,
                engine,
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
        // A bare CE channel (`bare_ce`) gets a host TSG of its own, keyed by its handle.
        let bare = reg.tsg((ch.h_client, ch.h_parent)).is_none();
        let tsg_key = if bare {
            (ch.h_client, ch.h_object)
        } else {
            (ch.h_client, ch.h_parent)
        };
        // The guest's recorded promote entries for this channel (c2 replay input).
        let promote_entries: Vec<crate::channel::PromoteEntry> = reg
            .promoted((ch.h_client, ch.h_object))
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        let engine = if bare {
            ch.engine_type
        } else {
            reg.tsg(tsg_key).ok_or("no TSG")?.1
        };
        self.ensure_events(&mut notes);
        self.ensure_invalidator(gm.fb, &mut notes);
        let entries = reg.root_entries(vas_key);
        let vas_handle = self.ensure_vas(vas_key, root, entries, &mut notes)?;
        let tsg_handle = self.ensure_tsg(tsg_key, vas_key, vas_handle, engine, &mut notes)?;

        let mut h = self.hm.lock().unwrap();
        let mut owned: Vec<u32> = Vec::new();
        let r = (|| -> Result<Built, String> {
            // USERD: the guest's page, as the host memory object over the same bytes.
            let (userd_mem, userd_off) = match ch.userd {
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
            let mut cp = sys::v610::NV_CHANNEL_ALLOC_PARAMS::default();
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
                    self.chip.channel.gpfifo,
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
                match h.alloc_object(handle, *class, &mut p, "engine-object") {
                    Ok(oh) => {
                        notes.push(format!(
                            "chid {chid}: host object class {class:#x} {oh:#x} ({} bytes of the guest's params) [host]",
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
        let mut p = params.to_vec();
        let r = self
            .hm
            .lock()
            .unwrap()
            .alloc_object(hc.handle, class, &mut p, "engine-object");
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
            self.chip.channel.gpfifo,
            self.chip.channel.copy,
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
        let target = if let Some(t) = self.tsgs.get(&(h_client, h_object)) {
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

    fn free_channel(&mut self, chid: u32, notes: &mut Vec<String>) {
        for sp in self.shadows.values_mut() {
            sp.chids.remove(&chid);
        }
        if let Some(hc) = self.channels.remove(&chid) {
            let r = self.hm.lock().unwrap().free_object(hc.handle);
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

    fn free_tsg(&mut self, key: (u32, u32), notes: &mut Vec<String>) {
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
                    "tsg {:#x}/{:#x}: host TSG {:#x} (engineType {:#x}) freed [host]",
                    key.0, key.1, t.handle, t.engine
                ),
                Err(st) => format!(
                    "tsg {:#x}/{:#x}: ERROR freeing host TSG {:#x}: {}",
                    key.0,
                    key.1,
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
        let tsgs: Vec<(u32, u32)> = self
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

    /// The guest freed `h_object` of `h_client` (or the client itself): free the host
    /// mirrors that stood for it, channels before TSGs before VA spaces.
    pub fn free_object(&mut self, reg: &Registry, h_client: u32, h_object: u32) -> Vec<String> {
        let mut notes = Vec::new();
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
            let tsgs: Vec<(u32, u32)> = self
                .tsgs
                .keys()
                .filter(|k| k.0 == h_client)
                .copied()
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
            if self.tsgs.contains_key(&(h_client, h_object)) {
                self.free_tsg((h_client, h_object), &mut notes);
            }
            return notes;
        }
        if self.tsgs.contains_key(&(h_client, h_object)) {
            self.free_tsg((h_client, h_object), &mut notes);
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
        let tsgs: Vec<(u32, u32)> = self.tsgs.keys().copied().collect();
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
            "mirror: {} channel(s) created, {} freed, {} live; {} doorbell(s) forwarded, {} TLB invalidate(s) ({} MEM_OP push(es), {} slow), {} refusal(s), longest mirror build {} us; events raised {}, MSIs {} (coalesced {}), host RC events {}",
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
            self.intr.rc_events.load(Ordering::Relaxed)
        )
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
}

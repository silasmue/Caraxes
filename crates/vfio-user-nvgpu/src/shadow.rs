// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Shadow VA space for the GR kernel launch (variant c1s, exec-formats.md §4.5,
//! decision D17 route 4 stage 1). A GR channel cannot run through the guest's own root
//! from user space: an externally-owned VAS cannot schedule GR, and a replayed VAS makes
//! the host RM build its GR context in the host kernel's own tables, unreachable through
//! the guest root (Xid 38, experiments.md d-gr-c2). The route that stays on the decided
//! design is to run the GR channel on a **normal host-managed VAS** (so the host RM's
//! own context buffers, set B, allocate and resolve as usual) and mirror the guest's
//! own work (set A: GPFIFO, pushbuffers, QMDs, constants, shader/local memory,
//! semaphores) into that VAS at the **same VAs**, through the RM API, by walking the
//! guest root and issuing fixed-VA `RM_MAP_MEMORY_DMA` calls.
//!
//! Every mapped PA is validated first (D17 stage 1's filter): a vidmem leaf must lie
//! wholly in the reservation and carry an uncompressible kind; a sysmem leaf must lie
//! wholly in a registered guest-RAM descriptor. Anything else is refused, counted and
//! logged with VA, PA, aperture and reason, and never mapped (D16/D17 isolation).
//!
//! **Dynamic mirroring (2026-09-24).** The guest keeps mapping set-A while it works, so
//! the mirror is a diff: [`ShadowVas::apply`] compares the guest's leaves with what is
//! mirrored, **unmaps** (NVOS47) every leaf that vanished or changed, and maps what is
//! new. Mapped VAs live in fixed-granularity virtual objects that never grow: one
//! `NV50_MEMORY_VIRTUAL` per 2 MiB-aligned region and page class, created at first use
//! and freed when the region empties or changes class. A region whose object the host
//! refuses (a VA collision with the host RM's own set-B buffers) or that touches an
//! excluded set-B range falls back to one exactly-sized virtual object per leaf.
//!
//! **Mapping package (docs/caraxes/mapping.md, 2026-09-24).**
//! - *Incremental.* The walk is the cached walk of ptcache.rs: only the table pages the
//!   guest wrote are read again, and only the 2 MiB units whose leaves changed are
//!   diffed. `CARAXES_SHADOW_ALWAYS_RESYNC=1` walks and diffs everything at every
//!   request (the reference).
//! - *Coalesced.* New leaves that are contiguous in VA and in the backing (same region
//!   object, memory handle, page size and kind, adjacent offsets) are mapped with one
//!   `RM_MAP_MEMORY_DMA`. A later change of part of such a run unmaps only that part (the
//!   RM splits the mapping, `virtmemUnmapFrom_IMPL`), so the rest of the run, which a
//!   running kernel may use, never goes away.
//! - *Budget.* The device counts its live RM mappings over all its shadow VA spaces; a
//!   run past the budget (`--map-budget`, `CARAXES_MAP_BUDGET`) is refused with an error
//!   line and never mapped (the guest's access then faults on its own channel).
//! - *Incremental teardown.* A shadow VA space is freed by unmapping its runs in bounded
//!   batches with a pause between them, then freeing its objects the same way, so the
//!   host RM never holds its GPU lock over a teardown of thousands of mappings (parallel.md
//!   §2.4: half a million mappings in one free held it for minutes).
//!
//! **Fewer host RM calls (docs/caraxes/memory-model.md §7, 2026-09-25).**
//! - *Gather descriptors.* A VA-contiguous stretch of 4 KiB sysmem leaves whose guest pages
//!   are scattered (the guest driver's page pool hands them out one by one, mapping.md §2)
//!   is mapped from one gather descriptor (hostmem.rs `gather`): the stretch's pages, each
//!   the filter's answer, back to back in one OS descriptor, so one `RM_MAP_MEMORY_DMA`
//!   maps the whole stretch. The descriptor is freed when its last mapping is gone.
//!   `CARAXES_SHADOW_GATHER=0` maps such stretches run by run.
//! - *Whole-region frees.* When the guest empties a 2 MiB region that holds at least
//!   [`BULK_MIN_RUNS`] runs, the shadow frees the region object with its mappings in one
//!   `RM_FREE` (the RM unmaps them itself) instead of one NVOS47 per run; the teardown does
//!   the same, about [`TEARDOWN_BATCH`] mappings per step. `CARAXES_SHADOW_BULK_FREE=0`
//!   unmaps run by run.
//! - *Deferred TLB invalidation* (`CARAXES_SHADOW_DEFER_TLB=1`, off by default: it measured
//!   no gain, the per-call cost is not the invalidate). Every map and unmap of a plan or a
//!   teardown step then carries the RM's `DEFER_TLB_INVALIDATION` flag, and the shadow owes
//!   the VAS one `NV0080_CTRL_CMD_DMA_INVALIDATE_TLB`, paid ([`ShadowVas::settle`]) before
//!   any virtual object is freed, at the end of every plan (so before the doorbell barrier
//!   releases a channel) and at the end of every teardown step.
//! - *Region free list.* A region object whose 2 MiB region the guest emptied, with every
//!   run in it unmapped by the RM, goes on a bounded per-VAS free list instead of being
//!   freed, and the next need of the same region and page class takes it back: two RM
//!   calls fewer per churned region. `CARAXES_SHADOW_REGION_POOL=0` frees at once.
//! - *Counted.* Every RM call of the shadow is counted by kind with its time ([`RmCalls`]),
//!   per sync line and in the summary.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use nvrm_abi::sys;

use crate::fb::{FakeFb, Target};
use crate::guestmem::GuestMemory;
use crate::hostmem::HostMemory;

/// The granularity of the region virtual objects: one PD0 entry (2 MiB).
pub const REGION: u64 = 2 << 20;
/// Refusal lines emitted per sync (the counters are always complete).
const LOG_PER_SYNC: usize = 24;
/// Leaves the cached walk holds per root at most (about 40 bytes each).
const MAX_LEAVES: usize = 4 << 20;
/// Teardown: RM calls per batch, and the pause after each batch.
pub const TEARDOWN_BATCH: usize = 64;
pub const TEARDOWN_PAUSE: Duration = Duration::from_millis(2);

/// The default per-device budget of live RM mappings (`--map-budget`): sized so that a
/// teardown at the budget, in batches, stays in the seconds and never holds the host GPU
/// lock for more than one batch (docs/caraxes/mapping.md §2).
pub const DEFAULT_MAP_BUDGET: usize = 32768;

/// Empty region objects a shadow VAS keeps for reuse at most (each is a 2 MiB VA reservation
/// with no mapping and no backing, freed with the VAS).
pub const REGION_POOL_MAX: usize = 64;

/// An emptied region with at least this many runs is freed whole (one `RM_FREE`, the RM
/// unmaps its mappings itself) instead of one unmap per run.
pub const BULK_MIN_RUNS: usize = 4;

/// A VA-contiguous stretch of 4 KiB sysmem leaves is mapped through a gather descriptor
/// when it has at least this many leaves and coalescing would still need this many runs.
pub const GATHER_MIN_LEAVES: usize = 16;
pub const GATHER_MIN_RUNS: usize = 8;

/// The host RM calls the shadow makes, behind a trait so the plan logic runs against a fake
/// host in the unit tests. [`HostMemory`] implements it with the RM escapes.
pub trait ShadowHost {
    /// `NV50_MEMORY_VIRTUAL` fixed at `va` (hostmem.rs `alloc_virtual_fixed`).
    fn alloc_virtual_fixed(
        &mut self,
        vas: u32,
        va: u64,
        size: u64,
        page_bytes: Option<u64>,
        force_align: bool,
    ) -> Result<u32, u32>;
    /// `RM_MAP_MEMORY_DMA` (NVOS46) at a fixed VA; `defer` skips the RM's TLB invalidate.
    #[allow(clippy::too_many_arguments)]
    fn map_dma_fixed(
        &self,
        h_dma: u32,
        h_mem: u32,
        offset: u64,
        length: u64,
        va: u64,
        page_size: u32,
        snoop: bool,
        kind: Option<u8>,
        defer: bool,
    ) -> u32;
    /// `RM_UNMAP_MEMORY_DMA` (NVOS47); `defer` skips the RM's TLB invalidate.
    fn unmap_dma(&self, h_dma: u32, h_mem: u32, va: u64, length: u64, defer: bool) -> u32;
    /// `RM_FREE`.
    fn free_object(&mut self, handle: u32) -> Result<(), u32>;
    /// `NV0080_CTRL_CMD_DMA_INVALIDATE_TLB` of the VA space.
    fn invalidate_vas(&self, vas: u32) -> u32;
    /// A gather descriptor over the pages `(registered descriptor, offset)`, in this order
    /// (hostmem.rs `gather`).
    fn gather(&mut self, pages: &[(u32, u64)]) -> Result<u32, String>;
    /// Free a gather descriptor.
    fn free_gather(&mut self, handle: u32) -> u32;
}

impl ShadowHost for HostMemory {
    fn alloc_virtual_fixed(
        &mut self,
        vas: u32,
        va: u64,
        size: u64,
        page_bytes: Option<u64>,
        force_align: bool,
    ) -> Result<u32, u32> {
        HostMemory::alloc_virtual_fixed(self, vas, va, size, page_bytes, force_align)
    }
    fn map_dma_fixed(
        &self,
        h_dma: u32,
        h_mem: u32,
        offset: u64,
        length: u64,
        va: u64,
        page_size: u32,
        snoop: bool,
        kind: Option<u8>,
        defer: bool,
    ) -> u32 {
        HostMemory::map_dma_fixed(self, h_dma, h_mem, offset, length, va, page_size, snoop, kind, defer)
    }
    fn unmap_dma(&self, h_dma: u32, h_mem: u32, va: u64, length: u64, defer: bool) -> u32 {
        HostMemory::unmap_dma(self, h_dma, h_mem, va, length, defer)
    }
    fn free_object(&mut self, handle: u32) -> Result<(), u32> {
        HostMemory::free_object(self, handle)
    }
    fn invalidate_vas(&self, vas: u32) -> u32 {
        HostMemory::invalidate_vas(self, vas)
    }
    fn gather(&mut self, pages: &[(u32, u64)]) -> Result<u32, String> {
        HostMemory::gather(self, pages)
    }
    fn free_gather(&mut self, handle: u32) -> u32 {
        HostMemory::free_gather(self, handle)
    }
}

/// The kinds of host RM call the shadow makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Map,
    Unmap,
    ObjectAlloc,
    ObjectFree,
    Invalidate,
    Gather,
    GatherFree,
}

const KINDS: usize = 7;
const CALL_NAMES: [&str; KINDS] = [
    "map",
    "unmap",
    "object alloc",
    "object free",
    "invalidate",
    "gather alloc",
    "gather free",
];

/// Host RM calls by kind, with the time spent in them, and what the region free list saved.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RmCalls {
    pub n: [u64; KINDS],
    pub us: [u64; KINDS],
    /// Region objects taken from the free list instead of allocated.
    pub reused: u64,
    /// Region objects put on the free list instead of freed.
    pub pooled: u64,
}

impl RmCalls {
    fn add(&mut self, c: Call, t0: Instant) {
        let dt = t0.elapsed();
        self.n[c as usize] += 1;
        self.us[c as usize] += dt.as_micros() as u64;
        crate::profile::record(
            crate::profile::Key::Shadow(CALL_NAMES[c as usize]),
            dt,
            0,
            None,
            "",
        );
    }

    pub fn count(&self, c: Call) -> u64 {
        self.n[c as usize]
    }

    pub fn total(&self) -> u64 {
        self.n.iter().sum()
    }

    fn since(&self, before: &RmCalls) -> RmCalls {
        let mut d = RmCalls::default();
        for i in 0..KINDS {
            d.n[i] = self.n[i] - before.n[i];
            d.us[i] = self.us[i] - before.us[i];
        }
        d.reused = self.reused - before.reused;
        d.pooled = self.pooled - before.pooled;
        d
    }

    /// `RM calls N: map N (U us), unmap N (U us), ...; regions reused R, pooled P`.
    pub fn line(&self) -> String {
        let parts: Vec<String> = (0..KINDS)
            .map(|i| format!("{} {} ({} us)", CALL_NAMES[i], self.n[i], self.us[i]))
            .collect();
        format!(
            "RM calls {}: {}; regions reused {}, pooled {}",
            self.total(),
            parts.join(", "),
            self.reused,
            self.pooled
        )
    }
}

/// `CARAXES_SHADOW_DEFER_TLB` and `CARAXES_SHADOW_REGION_POOL`: on unless set to 0.
fn switch_on(var: &str) -> bool {
    std::env::var(var).as_deref() != Ok("0")
}

/// `nvos46_flags::PAGE_SIZE_*` for a guest page size.
fn page_size_flag(size: u64) -> u32 {
    use nvrm_abi::nvgpu::nvos46_flags as f;
    match size {
        0x1000 => f::PAGE_SIZE_4KB,
        0x1_0000 => f::PAGE_SIZE_BIG,
        _ => f::PAGE_SIZE_HUGE,
    }
}

/// A leaf resolved against the reservation and the registered sysmem descriptors.
#[derive(Debug, PartialEq, Eq)]
enum Resolved {
    Vidmem { offset: u64 },
    Sysmem { h_mem: u32, offset: u64 },
    Refused(&'static str),
}

/// The D16/D17 filter: the **whole** leaf `[pa, pa + size)` must lie in the reservation
/// `[base, end)` (vidmem) or in one registered descriptor `(handle, iova, size)`
/// (sysmem), and its kind must not be compressible.
fn resolve_leaf(
    t: &Target,
    size: u64,
    kind: u8,
    base: u64,
    end: u64,
    descs: &[(u32, u64, u64)],
) -> Resolved {
    if (0x08..=0x0E).contains(&kind) {
        return Resolved::Refused("compressible kind (D16)");
    }
    match t {
        Target::Fb(pa) => {
            if *pa >= base && pa.saturating_add(size) <= end {
                Resolved::Vidmem { offset: pa - base }
            } else {
                Resolved::Refused("vidmem outside the reservation")
            }
        }
        Target::Sys(iova) => {
            for (h, ib, isz) in descs {
                if *iova >= *ib && iova.saturating_add(size) <= *ib + *isz {
                    return Resolved::Sysmem {
                        h_mem: *h,
                        offset: iova - ib,
                    };
                }
            }
            Resolved::Refused("sysmem outside the registered guest-RAM ranges")
        }
        _ => Resolved::Refused("no physical target"),
    }
}

/// The target a guest leaf resolves by: a sysmem leaf at the guest's DMA address becomes
/// the registered host IOVA of the same guest pages. The two differ only for the swiotlb
/// alias of the guest's table (guestmem.rs `host_iova`, memory-model.md §2), whose rounding
/// also covers ordinary guest RAM; a leaf nothing covers keeps its address and is refused
/// by [`resolve_leaf`]. The leaf's identity for the diff stays the guest's address.
fn host_target(sys_mem: &GuestMemory, t: &Target, size: u64) -> Target {
    match t {
        Target::Sys(iova) => sys_mem
            .host_iova(*iova, size)
            .map_or(*t, Target::Sys),
        _ => *t,
    }
}

/// What a guest leaf points at: the identity the mirror diffs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leaf {
    pub target: Target,
    pub size: u64,
    /// The guest PTE's kind. A depth/stencil kind (Z16..Z24S8, 0x01-0x05, uncompressed) is
    /// carried into the shadow mapping as a kind override (`kind_override`): the 3D engine
    /// checks the kind of a Z surface (Xid 13 "3D-Z KIND Violation" with the reservation's
    /// generic kind, display-e.md).
    pub kind: u8,
}

/// The kind a shadow mapping overrides to: the guest's, for the uncompressed depth and
/// stencil kinds only; every other leaf keeps the backing object's kind.
pub fn kind_override(kind: u8) -> Option<u8> {
    (0x01..=0x05).contains(&kind).then_some(kind)
}

/// The page class of a region object: 2 MiB pages, or 4 KiB/64 KiB pages under the
/// default page-size attribute. A PD0 entry is either a 2 MiB PTE or a dual PDE, so a
/// region never needs both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Huge,
    Small,
}

fn class_of(size: u64) -> Class {
    if size >= REGION {
        Class::Huge
    } else {
        Class::Small
    }
}

#[derive(Clone, Copy, Debug)]
struct Region {
    /// The region's virtual object; `None`: per-leaf objects (the region object was
    /// refused, or the region touches an excluded set-B range).
    handle: Option<u32>,
    class: Class,
}

/// One live RM mapping `[va, va + size)`: one or more leaves mapped by one call.
#[derive(Clone, Copy, Debug)]
struct Run {
    size: u64,
    h_dma: u32,
    h_mem: u32,
    /// A per-leaf virtual object, freed with the mapping.
    own_object: bool,
}

#[derive(Clone, Copy, Debug)]
struct MappedLeaf {
    leaf: Leaf,
    /// The VA of the run holding it.
    run: u64,
}

/// A leaf to map: VA, leaf, backing handle, offset in it, snoop (sysmem).
#[derive(Clone, Copy, Debug)]
struct ToMap {
    va: u64,
    leaf: Leaf,
    h_mem: u32,
    off: u64,
    snoop: bool,
}

/// Group sorted leaves into runs one RM call can map: contiguous VA, same page size and
/// kind, same backing handle and snoop, adjacent offsets, and inside one region (a run
/// never crosses its 2 MiB virtual object). Returns index ranges into `items`.
fn coalesce(items: &[ToMap]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let mut j = i + 1;
        while j < items.len() {
            let (p, n) = (&items[j - 1], &items[j]);
            let ok = n.va == p.va + p.leaf.size
                && n.leaf.size == p.leaf.size
                && n.leaf.kind == p.leaf.kind
                && n.h_mem == p.h_mem
                && n.snoop == p.snoop
                && n.off == p.off + p.leaf.size
                && n.va & !(REGION - 1) == items[i].va & !(REGION - 1);
            if !ok {
                break;
            }
            j += 1;
        }
        out.push(i..j);
        i = j;
    }
    out
}

/// Counters for the sync lines and the exit summary.
#[derive(Default, Debug, Clone)]
pub struct ShadowStats {
    pub syncs: u64,
    /// Resync requests answered without a walk (no page-table write since the last).
    pub skipped: u64,
    /// Syncs that walked the whole tree (first sync, sysmem tables, always-resync).
    pub full_walks: u64,
    /// Table blocks read over all syncs, and 2 MiB units diffed.
    pub table_reads: u64,
    pub units_diffed: u64,
    /// RM map calls (each one run) and the leaves they mapped.
    pub maps: u64,
    pub leaves_mapped: u64,
    pub unmaps: u64,
    pub partial_unmaps: u64,
    pub unmap_failed: u64,
    /// Leaves whose map (or per-leaf object) the host refused, counted once per leaf.
    pub map_failed: u64,
    /// Leaves refused because the device's mapping budget was reached.
    pub over_budget: u64,
    /// Leaves the D16/D17 filter refused, counted once per leaf.
    pub refused: u64,
    pub region_objects: u64,
    pub regions_freed: u64,
    pub regions_refused: u64,
    pub leaf_objects: u64,
    /// Region objects freed whole with their mappings, and the runs they held.
    pub bulk_frees: u64,
    pub bulk_runs: u64,
    /// Gather descriptors made, the leaves they carry, and the ones the host refused.
    pub gathers: u64,
    pub gathered_leaves: u64,
    pub gathers_refused: u64,
    /// `DMA_INVALIDATE_TLB` calls the RM refused (each turns the deferral off).
    pub invalidate_failed: u64,
    pub max_sync_us: u128,
    pub total_sync_us: u128,
    /// Every physical address ever mapped (isolation evidence, P20 item 3a): the vidmem
    /// PA extent `[lo, hi)` of all maps, and the maps per registered sysmem chunk (its
    /// IOVA base). Everything here passed the filter, so it lies in this device's own
    /// reservation and registered guest RAM by construction; the log lets a checker
    /// compare it with the other VM's ranges.
    pub vid_extent: Option<(u64, u64)>,
    pub sys_chunks: BTreeMap<u64, u64>,
}

impl ShadowStats {
    fn note_mapped(&mut self, t: &Target, size: u64, h_mem: u32, descs: &[(u32, u64, u64)]) {
        match t {
            Target::Fb(pa) => {
                let (lo, hi) = self.vid_extent.unwrap_or((*pa, *pa + size));
                self.vid_extent = Some((lo.min(*pa), hi.max(*pa + size)));
            }
            Target::Sys(iova) => {
                // An alias leaf (host_target) is counted under the descriptor it maps from.
                if let Some((_, base, _)) = descs
                    .iter()
                    .find(|(_, b, sz)| *iova >= *b && *iova < *b + *sz)
                    .or_else(|| descs.iter().find(|(h, _, _)| *h == h_mem))
                {
                    *self.sys_chunks.entry(*base).or_insert(0) += 1;
                }
            }
            _ => {}
        }
    }

    /// `mapped PAs: vidmem [lo, hi); sysmem chunks base:n ...` (every map so far).
    pub fn extents(&self) -> String {
        format!(
            "mapped PAs so far: vidmem {}; sysmem chunks {}",
            self.vid_extent
                .map(|(lo, hi)| format!("[{lo:#x}, {hi:#x})"))
                .unwrap_or_else(|| "none".into()),
            if self.sys_chunks.is_empty() {
                "none".into()
            } else {
                self.sys_chunks
                    .iter()
                    .map(|(b, n)| format!("{b:#x}:{n}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        )
    }
}

/// The device-wide budget of live RM mappings, shared by its shadow VA spaces.
#[derive(Clone, Debug)]
pub struct MapBudget {
    pub live: Arc<AtomicUsize>,
    pub limit: usize,
}

impl MapBudget {
    pub fn new(limit: usize) -> Self {
        MapBudget {
            live: Arc::new(AtomicUsize::new(0)),
            limit,
        }
    }

    /// `CARAXES_MAP_BUDGET` (set by the launcher from `--map-budget`), else the default.
    pub fn from_env() -> Self {
        let limit = std::env::var("CARAXES_MAP_BUDGET")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|n: &usize| *n > 0)
            .unwrap_or(DEFAULT_MAP_BUDGET);
        MapBudget::new(limit)
    }

    pub fn live(&self) -> usize {
        self.live.load(Ordering::Relaxed)
    }

    fn add(&self, n: isize) {
        if n >= 0 {
            self.live.fetch_add(n as usize, Ordering::Relaxed);
        } else {
            self.live.fetch_sub((-n) as usize, Ordering::Relaxed);
        }
    }
}

/// The part of a sync that reads the guest (the server thread, which owns the FB
/// store): which 2 MiB units changed and what each should hold after the filter.
#[derive(Debug, Default, Clone)]
pub struct Plan {
    pub root: u64,
    /// Per unit: every leaf that passed the filter, with its backing.
    units: Vec<(u64, Vec<ToMap>)>,
    pub exclude: Vec<(u64, u64)>,
    /// Refusal lines of this walk (each leaf once).
    pub notes: Vec<String>,
    pub full: bool,
    pub reads: usize,
    pub truncated: bool,
    pub walked: usize,
    pub want: usize,
    pub excluded: usize,
    pub refused_now: usize,
    pub refused_total: u64,
    pub walk_us: u128,
    /// Set by the worker: how long the plan waited in its queue.
    pub queued_us: u128,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.units.is_empty() && self.notes.is_empty()
    }
}

/// The server-thread side of a shadow VA space: the walk, the filter and the refusal log.
pub struct ShadowFront {
    pub root: u64,
    exclude: Vec<(u64, u64)>,
    refused: BTreeMap<u64, Leaf>,
    refused_total: u64,
    /// Units whose last plan wanted a leaf (for a sweep when the exclusions change).
    live_units: BTreeSet<u64>,
    always: bool,
    pub skipped: u64,
}

impl ShadowFront {
    pub fn new(root: u64) -> Self {
        ShadowFront {
            root,
            exclude: vec![(u64::MAX, u64::MAX)],
            refused: BTreeMap::new(),
            refused_total: 0,
            live_units: BTreeSet::new(),
            always: std::env::var("CARAXES_SHADOW_ALWAYS_RESYNC").as_deref() == Ok("1"),
            skipped: 0,
        }
    }

    /// Whether the next sync has work: the guest wrote a table page of this root, or the
    /// exclusions changed, or always-resync is on.
    pub fn needs_sync(&self, fb: &FakeFb, exclude: &[(u64, u64)]) -> bool {
        self.always
            || exclude != self.exclude.as_slice()
            || fb.tables_dirty(Target::Fb(self.root))
            || fb.pool_rediff_pending(self.root)
    }

    /// Walk what changed (ptcache.rs) and filter it (D16/D17): the plan for [`ShadowVas::apply`].
    /// `None` when nothing needs doing (no table written since the last plan).
    #[allow(clippy::too_many_arguments)]
    pub fn plan(
        &mut self,
        fb: &FakeFb,
        sys_mem: &GuestMemory,
        base: u64,
        end: u64,
        reservation: u32,
        exclude: &[(u64, u64)],
        descs: &[(u32, u64, u64)],
    ) -> Option<Plan> {
        let t0 = Instant::now();
        if !self.needs_sync(fb, exclude) {
            self.skipped += 1;
            return None;
        }
        let exclude_changed = exclude != self.exclude.as_slice();
        self.exclude = exclude.to_vec();
        let root = Target::Fb(self.root);
        let upd = fb.walk_update(Some(sys_mem), root, self.always, MAX_LEAVES);
        let mut units: BTreeSet<u64> = upd.changed.iter().copied().collect();
        // P21 design H: a page this root maps moved into or out of the BAR1 pool.
        let pool_moved = fb.take_pool_rediff(self.root);
        if exclude_changed || upd.full || pool_moved {
            if let Some(all) = fb.with_walk(root, |c| c.units_with_leaves()) {
                units.extend(all);
            }
            units.extend(self.live_units.iter().copied());
            units.extend(self.refused.keys().map(|va| va & !(REGION - 1)));
        }
        let mut plan = Plan {
            root: self.root,
            exclude: exclude.to_vec(),
            full: upd.full,
            reads: upd.reads,
            truncated: fb.with_walk(root, |c| c.truncated) == Some(true),
            ..Default::default()
        };
        let mut logged = 0usize;
        for &u in &units {
            // P21 design H: leaves on pool pages point there (fb.rs `remap`).
            let leaves = fb.remap_leaves(
                fb.with_walk(root, |c| c.unit_leaves(u).to_vec())
                    .unwrap_or_default(),
            );
            plan.walked += leaves.len();
            let mut want_u: Vec<ToMap> = Vec::new();
            let old: Vec<u64> = self.refused.range(u..u + REGION).map(|(v, _)| *v).collect();
            let mut refused_u: BTreeMap<u64, Leaf> = BTreeMap::new();
            for (va, t, size, kind) in &leaves {
                if exclude.iter().any(|(lo, hi)| va < hi && va + size > *lo) {
                    plan.excluded += 1;
                    continue;
                }
                let leaf = Leaf {
                    target: *t,
                    size: *size,
                    kind: *kind,
                };
                match resolve_leaf(&host_target(sys_mem, t, *size), *size, *kind, base, end, descs) {
                    Resolved::Vidmem { offset } => want_u.push(ToMap {
                        va: *va,
                        leaf,
                        h_mem: reservation,
                        off: offset,
                        snoop: false,
                    }),
                    Resolved::Sysmem { h_mem, offset } => want_u.push(ToMap {
                        va: *va,
                        leaf,
                        h_mem,
                        off: offset,
                        snoop: true,
                    }),
                    Resolved::Refused(why) => {
                        if self.refused.get(va) != Some(&leaf) {
                            self.refused_total += 1;
                            if logged < LOG_PER_SYNC {
                                logged += 1;
                                plan.notes.push(format!(
                                    "shadow: REFUSED leaf va {va:#x} -> {t:x?} size {size:#x} kind {kind:#x}: {why} (never mapped) [D16/D17]"
                                ));
                            }
                        }
                        refused_u.insert(*va, leaf);
                    }
                }
            }
            for v in old {
                self.refused.remove(&v);
            }
            self.refused.extend(refused_u);
            if want_u.is_empty() {
                self.live_units.remove(&u);
            } else {
                self.live_units.insert(u);
            }
            want_u.sort_by_key(|m| m.va);
            plan.want += want_u.len();
            plan.units.push((u, want_u));
        }
        plan.refused_now = self.refused.len();
        plan.refused_total = self.refused_total;
        plan.walk_us = t0.elapsed().as_micros();
        Some(plan)
    }
}

/// A host-managed shadow VA space (c1s): the guest's set-A mappings mirrored at their
/// own VAs by fixed `RM_MAP_MEMORY_DMA` into per-region `NV50_MEMORY_VIRTUAL` objects,
/// so a GR channel placed on this VAS resolves the guest's work while the host RM owns
/// the GR context (set B) in the same VAS. The guest's promoted-context VA ranges are
/// excluded (the host builds those).
pub struct ShadowVas {
    pub vas: u32,
    regions: BTreeMap<u64, Region>,
    /// Mirrored leaves by VA.
    mapped: BTreeMap<u64, MappedLeaf>,
    /// Live RM mappings by VA.
    runs: BTreeMap<u64, Run>,
    /// Leaves the host refused to map (or the budget refused), not retried until the
    /// guest changes them.
    failed: BTreeMap<u64, Leaf>,
    pub stats: ShadowStats,
    budget: MapBudget,
    /// The budget refusal was logged since the budget last had room.
    budget_logged: bool,
    /// Incremental teardown progress.
    teardown_objects: Vec<u32>,
    teardown_unmapped: usize,
    teardown_freed: usize,
    /// Coalesce contiguous leaves into one RM mapping (`CARAXES_SHADOW_COALESCE=0`: one
    /// mapping per leaf, the earlier behaviour, for A/B measurements).
    coalesce: bool,
    /// Map and unmap with the RM's TLB invalidate deferred to [`Self::settle`]
    /// (`CARAXES_SHADOW_DEFER_TLB=1`; off by default: measured without gain, memory-model.md
    /// §7.5). Turned off for good when an invalidate fails.
    defer_tlb: bool,
    /// Free an emptied region's object with its mappings in one call
    /// (`CARAXES_SHADOW_BULK_FREE=0`: one unmap per run).
    bulk_on: bool,
    /// A deferred map or unmap was made since the last invalidate of the VAS.
    tlb_owed: bool,
    /// Empty region objects kept for reuse, by region VA (`CARAXES_SHADOW_REGION_POOL=0`:
    /// none). Every one of them had all its runs unmapped by the RM without a failure.
    pool: BTreeMap<u64, Region>,
    pool_on: bool,
    /// Host RM calls by kind, since the VAS was created.
    pub calls: RmCalls,
    /// Lines of [`Self::settle`] for the next sync line.
    settle_notes: Vec<String>,
    /// The call counters when the teardown started (the teardown line reports its own).
    teardown_calls0: Option<RmCalls>,
    /// Mappings the teardown removed with their region object.
    teardown_bulk: usize,
    /// Map scattered sysmem stretches through gather descriptors
    /// (`CARAXES_SHADOW_GATHER=0`: run by run from the registered descriptors).
    gather_on: bool,
    /// Live gather descriptors and the runs mapped from each.
    gathers: BTreeMap<u32, usize>,
    /// Gather descriptors no run uses any more, freed at the next safe point.
    gathers_idle: Vec<u32>,
}

impl ShadowVas {
    /// A fresh normal (host-managed) `FERMI_VASPACE_A`.
    pub fn new(hm: &mut HostMemory, budget: MapBudget) -> Result<Self, u32> {
        let device = hm.device();
        let mut vp = sys::NV_VASPACE_ALLOCATION_PARAMETERS::default();
        vp.vaSize = 1 << 47;
        vp.bigPageSize = 64 * 1024;
        // SAFETY: plain-data alloc params viewed as bytes.
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                &mut vp as *mut _ as *mut u8,
                std::mem::size_of::<sys::NV_VASPACE_ALLOCATION_PARAMETERS>(),
            )
        };
        let vas = hm.alloc_object(device, sys::FERMI_VASPACE_A, bytes, "shadow-vaspace")?;
        Ok(Self::with_vas(vas, budget))
    }

    /// The shadow of an existing VA space handle, with the switches from the environment.
    pub fn with_vas(vas: u32, budget: MapBudget) -> Self {
        ShadowVas {
            vas,
            regions: BTreeMap::new(),
            mapped: BTreeMap::new(),
            runs: BTreeMap::new(),
            failed: BTreeMap::new(),
            stats: ShadowStats::default(),
            budget,
            budget_logged: false,
            teardown_objects: Vec::new(),
            teardown_unmapped: 0,
            teardown_freed: 0,
            coalesce: switch_on("CARAXES_SHADOW_COALESCE"),
            defer_tlb: std::env::var("CARAXES_SHADOW_DEFER_TLB").as_deref() == Ok("1"),
            bulk_on: switch_on("CARAXES_SHADOW_BULK_FREE"),
            tlb_owed: false,
            pool: BTreeMap::new(),
            pool_on: switch_on("CARAXES_SHADOW_REGION_POOL"),
            calls: RmCalls::default(),
            settle_notes: Vec::new(),
            teardown_calls0: None,
            teardown_bulk: 0,
            gather_on: switch_on("CARAXES_SHADOW_GATHER"),
            gathers: BTreeMap::new(),
            gathers_idle: Vec::new(),
        }
    }

    /// Whether deferred TLB invalidation is on (the switch, and no invalidate failed yet).
    pub fn defers_tlb(&self) -> bool {
        self.defer_tlb
    }

    /// Pay the TLB invalidate that deferred maps and unmaps owe the VAS: one
    /// `DMA_INVALIDATE_TLB` of the whole VAS (hostmem.rs `invalidate_vas`). Called before
    /// any virtual object is freed (the RM frees its page tables with it), at the end of
    /// every plan (before the worker reports it done, so before the doorbell barrier lets a
    /// channel run) and at the end of every teardown step. A refused invalidate turns the
    /// deferral off for this VAS, so every later map and unmap invalidates the whole VAS
    /// itself again, and is logged as an ERROR.
    pub fn settle<H: ShadowHost>(&mut self, h: &H) {
        if !self.tlb_owed {
            return;
        }
        self.tlb_owed = false;
        let t0 = Instant::now();
        let st = h.invalidate_vas(self.vas);
        self.calls.add(Call::Invalidate, t0);
        if st != sys::NV_OK {
            self.stats.invalidate_failed += 1;
            self.defer_tlb = false;
            let line = format!(
                "ERROR shadow: DMA_INVALIDATE_TLB of shadow VAS {:#x} refused {st:#x}; deferred TLB invalidation is off for this VAS from now on (every map and unmap invalidates the VAS itself) [host]",
                self.vas
            );
            eprintln!("vfio-user-nvgpu: {line}");
            self.settle_notes.push(line);
        }
    }

    /// Free a virtual object: the owed invalidate first, then `RM_FREE`.
    fn free_virtual<H: ShadowHost>(&mut self, h: &mut H, handle: u32) {
        self.settle(&*h);
        let t0 = Instant::now();
        let _ = h.free_object(handle);
        self.calls.add(Call::ObjectFree, t0);
    }

    /// Free a region object together with its mappings: one `RM_FREE`, in which the RM first
    /// unmaps every mapping made into the object (the resource server's auto-unmap of the
    /// context's back-references, `_clientUnmapInterBackRefMappings`, rs_client.c:1347-1400,
    /// each through `virtmemUnmapFrom` with its own TLB invalidate) and then frees the
    /// object's VA range. The shadow forgets the region's runs and leaves. The owed
    /// invalidate of deferred calls is paid first ([`Self::free_virtual`]).
    fn free_region_whole<H: ShadowHost>(&mut self, h: &mut H, u: u64, hd: u32) -> usize {
        let runs: Vec<u64> = self.runs.range(u..u + REGION).map(|(v, _)| *v).collect();
        for v in &runs {
            if let Some(r) = self.runs.remove(v) {
                self.run_gone(r.h_mem);
            }
        }
        self.budget.add(-(runs.len() as isize));
        let gone: Vec<u64> = self.mapped.range(u..u + REGION).map(|(v, _)| *v).collect();
        for v in gone {
            self.mapped.remove(&v);
        }
        self.regions.remove(&u);
        self.free_virtual(h, hd);
        self.stats.regions_freed += 1;
        self.stats.bulk_frees += 1;
        self.stats.bulk_runs += runs.len() as u64;
        // The gathers only this region used, after the object (and its mappings) is gone.
        self.free_idle_gathers(h);
        runs.len()
    }

    /// Free the pooled region objects that overlap an excluded (set-B) range: the host RM
    /// builds its own buffers there.
    fn purge_pool<H: ShadowHost>(&mut self, h: &mut H, exclude: &[(u64, u64)]) {
        let hit: Vec<u64> = self
            .pool
            .keys()
            .copied()
            .filter(|rv| exclude.iter().any(|(lo, hi)| *rv < *hi && rv + REGION > *lo))
            .collect();
        for rv in hit {
            if let Some(hd) = self.pool.remove(&rv).and_then(|r| r.handle) {
                self.free_virtual(h, hd);
                self.stats.regions_freed += 1;
            }
        }
    }

    /// Leaves mirrored now.
    pub fn mapped_leaves(&self) -> usize {
        self.mapped.len()
    }

    /// Live RM mappings of this VAS.
    pub fn live_maps(&self) -> usize {
        self.runs.len()
    }

    /// The mirrored leaf covering `va`, if any: `(leaf va, leaf)`.
    pub fn lookup(&self, va: u64) -> Option<(u64, Leaf)> {
        self.mapped
            .range(..=va)
            .next_back()
            .filter(|(lva, m)| va < **lva + m.leaf.size)
            .map(|(lva, m)| (*lva, m.leaf))
    }

    /// Unmap the mirrored leaves at `vas` (ascending): one NVOS47 per contiguous piece of
    /// a run; a piece that is not the whole run splits it (the RM keeps the rest mapped).
    /// A partial unmap the RM refuses unmaps the whole run; its other leaves are then no
    /// longer mirrored and the caller maps them again.
    fn unmap_leaves<H: ShadowHost>(&mut self, hm: &mut H, vas: &[u64]) {
        let mut i = 0;
        while i < vas.len() {
            let Some(first) = self.mapped.get(&vas[i]).copied() else {
                i += 1;
                continue;
            };
            let run_va = first.run;
            let s = vas[i];
            let mut e = s + first.leaf.size;
            let mut j = i + 1;
            while j < vas.len() {
                match self.mapped.get(&vas[j]) {
                    Some(m) if m.run == run_va && vas[j] == e => {
                        e += m.leaf.size;
                        j += 1;
                    }
                    _ => break,
                }
            }
            self.unmap_piece(hm, run_va, s, e);
            i = j;
        }
    }

    /// One `RM_UNMAP_MEMORY_DMA`, deferred when the switch is on (the VAS then owes an
    /// invalidate).
    fn rm_unmap<H: ShadowHost>(&mut self, hm: &H, run: &Run, va: u64, len: u64) -> u32 {
        let t0 = Instant::now();
        let st = hm.unmap_dma(run.h_dma, run.h_mem, va, len, self.defer_tlb);
        self.calls.add(Call::Unmap, t0);
        self.tlb_owed |= self.defer_tlb;
        self.stats.unmaps += 1;
        st
    }

    fn unmap_piece<H: ShadowHost>(&mut self, hm: &mut H, run_va: u64, s: u64, e: u64) {
        let Some(run) = self.runs.get(&run_va).copied() else {
            return;
        };
        let run_end = run_va + run.size;
        let whole = s == run_va && e == run_end;
        let st = self.rm_unmap(&*hm, &run, s, e - s);
        if st == sys::NV_OK && whole {
            self.drop_run(hm, run_va, run);
        } else if st == sys::NV_OK {
            self.stats.partial_unmaps += 1;
            self.runs.remove(&run_va);
            let mut n: isize = -1;
            if s > run_va {
                self.runs.insert(
                    run_va,
                    Run {
                        size: s - run_va,
                        ..run
                    },
                );
                n += 1;
            }
            if e < run_end {
                self.runs.insert(
                    e,
                    Run {
                        size: run_end - e,
                        ..run
                    },
                );
                n += 1;
                for (_, m) in self.mapped.range_mut(e..run_end) {
                    m.run = e;
                }
            }
            self.budget.add(n);
            if let Some(c) = self.gathers.get_mut(&run.h_mem) {
                *c = c.saturating_add_signed(n);
            }
            let gone: Vec<u64> = self.mapped.range(s..e).map(|(va, _)| *va).collect();
            for va in gone {
                self.mapped.remove(&va);
            }
        } else {
            self.stats.unmap_failed += 1;
            if !whole {
                // The RM refused the split: take the whole run down; the caller maps its
                // other leaves again.
                let st2 = self.rm_unmap(&*hm, &run, run_va, run.size);
                if st2 != sys::NV_OK {
                    self.stats.unmap_failed += 1;
                }
            }
            self.drop_run(hm, run_va, run);
        }
    }

    /// Forget a run whose mapping is gone (and free its per-leaf object).
    fn drop_run<H: ShadowHost>(&mut self, hm: &mut H, run_va: u64, run: Run) {
        self.runs.remove(&run_va);
        self.budget.add(-1);
        let gone: Vec<u64> = self
            .mapped
            .range(run_va..run_va + run.size)
            .map(|(va, _)| *va)
            .collect();
        for va in gone {
            self.mapped.remove(&va);
        }
        if run.own_object {
            self.free_virtual(hm, run.h_dma);
        }
        self.run_gone(run.h_mem);
        self.free_idle_gathers(hm);
    }

    /// A run of `h_mem` is gone: a gather descriptor no run uses any more becomes idle.
    fn run_gone(&mut self, h_mem: u32) {
        if let Some(c) = self.gathers.get_mut(&h_mem) {
            *c = c.saturating_sub(1);
            if *c == 0 {
                self.gathers.remove(&h_mem);
                self.gathers_idle.push(h_mem);
            }
        }
    }

    /// Free the idle gather descriptors, after the owed invalidate: their mappings are gone.
    fn free_idle_gathers<H: ShadowHost>(&mut self, h: &mut H) {
        if self.gathers_idle.is_empty() {
            return;
        }
        self.settle(&*h);
        for g in std::mem::take(&mut self.gathers_idle) {
            let t0 = Instant::now();
            let _ = h.free_gather(g);
            self.calls.add(Call::GatherFree, t0);
        }
    }

    /// Map scattered sysmem stretches through gather descriptors: every maximal
    /// VA-contiguous stretch of 4 KiB sysmem leaves in `todo` (sorted by VA; one unit) with
    /// one kind and snoop setting, at least [`GATHER_MIN_LEAVES`] leaves and at least
    /// [`GATHER_MIN_RUNS`] coalesced runs, gets one gather descriptor of its pages in VA
    /// order, and its leaves are rewritten to that backing at offsets 0, 4 KiB, ...: they
    /// then coalesce into one run. The pages are the ones the filter resolved (each leaf's
    /// registered descriptor and offset); a stretch the host refuses keeps its leaves as
    /// they were. Returns the gathers made.
    fn gather_stretches<H: ShadowHost>(&mut self, h: &mut H, todo: &mut [ToMap], notes: &mut Vec<String>) -> Vec<u32> {
        const PAGE: u64 = 0x1000;
        let eligible = |m: &ToMap| m.snoop && m.leaf.size == PAGE && matches!(m.leaf.target, Target::Sys(_));
        let mut made = Vec::new();
        let mut i = 0;
        while i < todo.len() {
            if !eligible(&todo[i]) {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < todo.len()
                && eligible(&todo[j])
                && todo[j].va == todo[j - 1].va + PAGE
                && todo[j].leaf.kind == todo[i].leaf.kind
            {
                j += 1;
            }
            if j - i >= GATHER_MIN_LEAVES && coalesce(&todo[i..j]).len() >= GATHER_MIN_RUNS {
                let pages: Vec<(u32, u64)> = todo[i..j].iter().map(|m| (m.h_mem, m.off)).collect();
                let t0 = Instant::now();
                let r = h.gather(&pages);
                self.calls.add(Call::Gather, t0);
                match r {
                    Ok(g) => {
                        self.gathers.insert(g, 0);
                        self.stats.gathers += 1;
                        self.stats.gathered_leaves += (j - i) as u64;
                        for (k, m) in todo[i..j].iter_mut().enumerate() {
                            m.h_mem = g;
                            m.off = k as u64 * PAGE;
                        }
                        made.push(g);
                    }
                    Err(e) => {
                        self.stats.gathers_refused += 1;
                        if self.stats.gathers_refused <= 3 {
                            notes.push(format!(
                                "shadow: gather of {} leaves from va {:#x} refused ({e}); mapped run by run [host]",
                                j - i,
                                todo[i].va
                            ));
                        }
                    }
                }
            }
            i = j;
        }
        made
    }

    /// Apply a [`Plan`]: per unit, unmap what the guest removed or changed, drop a region
    /// object of the wrong class, and map what is new, coalesced into runs. The host RM lock
    /// is taken per unit, so a long plan never holds it for more than one unit's RM calls.
    /// Returns `(changed, notes)`, the first note the one-line summary of the sync.
    pub fn apply<H: ShadowHost>(
        &mut self,
        hm: &std::sync::Mutex<H>,
        plan: &Plan,
        descs: &[(u32, u64, u64)],
    ) -> (bool, Vec<String>) {
        let t0 = Instant::now();
        let calls0 = self.calls;
        let exclude = &plan.exclude;
        let mut notes = Vec::new();
        let mut logged = 0usize;
        let live0 = self.runs.len();
        let (mut n_unmap, mut n_map, mut n_leaves, mut n_fail, mut n_budget) =
            (0usize, 0usize, 0usize, 0usize, 0usize);
        if !self.pool.is_empty() {
            let mut h = hm.lock().unwrap();
            self.purge_pool(&mut *h, exclude);
        }
        for (u, want_v) in &plan.units {
            let u = *u;
            let want: BTreeMap<u64, ToMap> = want_v.iter().map(|m| (m.va, *m)).collect();
            // A failed leaf is retried once the guest changes it.
            let failed_u: Vec<u64> = self.failed.range(u..u + REGION).map(|(v, _)| *v).collect();
            for v in failed_u {
                if want.get(&v).map(|m| m.leaf) != self.failed.get(&v).copied() {
                    self.failed.remove(&v);
                }
            }
            let unmap: Vec<u64> = self
                .mapped
                .range(u..u + REGION)
                .filter(|(va, m)| want.get(va).map(|w| w.leaf) != Some(m.leaf))
                .map(|(va, _)| *va)
                .collect();
            let need = want.values().next().map(|m| class_of(m.leaf.size));
            let stale = self.regions.get(&u).is_some_and(|reg| Some(reg.class) != need);
            let todo_any = want
                .values()
                .any(|m| !self.mapped.contains_key(&m.va) || unmap.contains(&m.va));
            if unmap.is_empty() && !stale && !todo_any {
                continue;
            }
            let mut h = hm.lock().unwrap();
            // The guest emptied a region with many runs: one RM_FREE of its object instead
            // of one unmap per run (nothing is left to map in it).
            if need.is_none() && self.bulk_on {
                let hd = self.regions.get(&u).and_then(|r| r.handle);
                if let Some(hd) = hd.filter(|_| self.runs.range(u..u + REGION).count() >= BULK_MIN_RUNS) {
                    n_unmap += unmap.len();
                    self.free_region_whole(&mut *h, u, hd);
                    continue;
                }
            }
            let unmap_failed0 = self.stats.unmap_failed;
            n_unmap += unmap.len();
            self.unmap_leaves(&mut *h, &unmap);
            if stale {
                let inside: Vec<u64> = self.mapped.range(u..u + REGION).map(|(v, _)| *v).collect();
                self.unmap_leaves(&mut *h, &inside);
                if let Some(reg) = self.regions.remove(&u) {
                    if let Some(hd) = reg.handle {
                        // The free list takes a region only when the guest emptied it and
                        // the RM unmapped every run in it: a pooled object holds no leaf.
                        let clean = need.is_none()
                            && self.stats.unmap_failed == unmap_failed0
                            && self.runs.range(u..u + REGION).next().is_none()
                            && self.mapped.range(u..u + REGION).next().is_none()
                            && !exclude.iter().any(|(lo, hi)| u < *hi && u + REGION > *lo);
                        if self.pool_on && clean && self.pool.len() < REGION_POOL_MAX {
                            self.pool.insert(u, reg);
                            self.calls.pooled += 1;
                        } else {
                            self.free_virtual(&mut *h, hd);
                            self.stats.regions_freed += 1;
                        }
                    }
                }
            }
            // Map what is wanted and not mirrored (a refused split may have taken more down).
            let mut todo: Vec<ToMap> = want
                .values()
                .filter(|m| !self.mapped.contains_key(&m.va) && self.failed.get(&m.va) != Some(&m.leaf))
                .copied()
                .collect();
            let made = if self.gather_on {
                self.gather_stretches(&mut *h, &mut todo, &mut notes)
            } else {
                Vec::new()
            };
            for r in coalesce(&todo) {
                let items = &todo[r];
                let first = items[0];
                let rv = first.va & !(REGION - 1);
                let class = class_of(first.leaf.size);
                if !self.regions.contains_key(&rv) {
                    let touches_b = exclude.iter().any(|(lo, hi)| rv < *hi && rv + REGION > *lo);
                    // A pooled object of this region: taken back when its class fits,
                    // freed otherwise.
                    let mut reuse = None;
                    if let Some(reg) = self.pool.remove(&rv) {
                        match reg.handle {
                            Some(hd) if !touches_b && reg.class == class => {
                                self.calls.reused += 1;
                                reuse = Some(hd);
                            }
                            Some(hd) => {
                                self.free_virtual(&mut *h, hd);
                                self.stats.regions_freed += 1;
                            }
                            None => {}
                        }
                    }
                    let handle = if touches_b {
                        None
                    } else if reuse.is_some() {
                        reuse
                    } else {
                        let page = match class {
                            Class::Huge => Some(REGION),
                            Class::Small => None,
                        };
                        let ta = Instant::now();
                        let r = h.alloc_virtual_fixed(self.vas, rv, REGION, page, false);
                        self.calls.add(Call::ObjectAlloc, ta);
                        match r {
                            Ok(hd) => {
                                self.stats.region_objects += 1;
                                Some(hd)
                            }
                            Err(st) => {
                                self.stats.regions_refused += 1;
                                if logged < LOG_PER_SYNC {
                                    logged += 1;
                                    notes.push(format!(
                                        "shadow: region {rv:#x} ({class:?}) object refused {st:#x}; per-leaf objects there [host]"
                                    ));
                                }
                                None
                            }
                        }
                    };
                    self.regions.insert(rv, Region { handle, class });
                }
                let region = self.regions[&rv].handle;
                // Without a region object every leaf gets its own object and mapping.
                let pieces: Vec<&[ToMap]> = match region {
                    Some(_) if self.coalesce => vec![items],
                    _ => items.chunks(1).collect(),
                };
                for piece in pieces {
                    if self.budget.live() >= self.budget.limit {
                        n_budget += piece.len();
                        self.stats.over_budget += piece.len() as u64;
                        for m in piece {
                            self.failed.insert(m.va, m.leaf);
                        }
                        if !self.budget_logged {
                            self.budget_logged = true;
                            let line = format!(
                                "ERROR shadow: the device's mapping budget of {} live RM mappings is reached; set-A leaves from va {:#x} on are refused and never mapped (the guest's access faults on its own channel) [host protection, CARAXES_MAP_BUDGET]",
                                self.budget.limit, piece[0].va
                            );
                            eprintln!("vfio-user-nvgpu: {line}");
                            notes.push(line);
                        }
                        continue;
                    }
                    let p0 = piece[0];
                    let len: u64 = piece.iter().map(|m| m.leaf.size).sum();
                    let (h_dma, own) = match region {
                        Some(hd) => (hd, false),
                        None => {
                            let ta = Instant::now();
                            let r = h.alloc_virtual_fixed(self.vas, p0.va, len, Some(p0.leaf.size), true);
                            self.calls.add(Call::ObjectAlloc, ta);
                            match r {
                                Ok(hd) => {
                                    self.stats.leaf_objects += 1;
                                    (hd, true)
                                }
                                Err(st) => {
                                    self.stats.map_failed += 1;
                                    n_fail += 1;
                                    self.failed.insert(p0.va, p0.leaf);
                                    if logged < LOG_PER_SYNC {
                                        logged += 1;
                                        notes.push(format!(
                                            "shadow: map va {:#x} -> {:x?} size {:#x} refused: per-leaf VA object {st:#x} (VA collision with set B?) [host]",
                                            p0.va, p0.leaf.target, p0.leaf.size
                                        ));
                                    }
                                    continue;
                                }
                            }
                        }
                    };
                    let mut st = self.rm_map(&*h, h_dma, &p0, len);
                    if st == sys::NV_OK {
                        self.record_run(p0.va, len, h_dma, p0.h_mem, own, piece, descs);
                        n_map += 1;
                        n_leaves += piece.len();
                        continue;
                    }
                    if own {
                        self.free_virtual(&mut *h, h_dma);
                    }
                    if piece.len() == 1 {
                        self.stats.map_failed += 1;
                        n_fail += 1;
                        self.failed.insert(p0.va, p0.leaf);
                        if logged < LOG_PER_SYNC {
                            logged += 1;
                            notes.push(format!(
                                "shadow: map va {:#x} -> {:x?} size {:#x} refused {st:#x} [host]",
                                p0.va, p0.leaf.target, p0.leaf.size
                            ));
                        }
                        continue;
                    }
                    // A coalesced map the host refused: leaf by leaf, as before coalescing.
                    for m in piece {
                        st = self.rm_map(&*h, h_dma, m, m.leaf.size);
                        if st == sys::NV_OK {
                            self.record_run(m.va, m.leaf.size, h_dma, m.h_mem, false, std::slice::from_ref(m), descs);
                            n_map += 1;
                            n_leaves += 1;
                        } else {
                            self.stats.map_failed += 1;
                            n_fail += 1;
                            self.failed.insert(m.va, m.leaf);
                            if logged < LOG_PER_SYNC {
                                logged += 1;
                                notes.push(format!(
                                    "shadow: map va {:#x} -> {:x?} size {:#x} refused {st:#x} [host]",
                                    m.va, m.leaf.target, m.leaf.size
                                ));
                            }
                        }
                    }
                }
            }
            // A gather no map took (the host refused them all) is freed again.
            for g in made {
                if self.gathers.get(&g) == Some(&0) {
                    self.gathers.remove(&g);
                    self.gathers_idle.push(g);
                }
            }
            self.free_idle_gathers(&mut *h);
        }
        // The invalidate the deferred calls owe, before the worker reports the plan done.
        if self.tlb_owed {
            let h = hm.lock().unwrap();
            self.settle(&*h);
        }
        if self.budget.live() < self.budget.limit {
            self.budget_logged = false;
        }
        let us_apply = t0.elapsed().as_micros();
        let us = plan.walk_us + us_apply;
        self.stats.syncs += 1;
        if plan.full {
            self.stats.full_walks += 1;
        }
        self.stats.table_reads += plan.reads as u64;
        self.stats.units_diffed += plan.units.len() as u64;
        self.stats.refused = plan.refused_total;
        self.stats.max_sync_us = self.stats.max_sync_us.max(us);
        self.stats.total_sync_us += us;
        let settle_notes = std::mem::take(&mut self.settle_notes);
        let changed = n_unmap > 0
            || n_map > 0
            || n_fail > 0
            || n_budget > 0
            || logged > 0
            || !plan.notes.is_empty()
            || !settle_notes.is_empty();
        let mut out = vec![format!(
            "shadow: sync {} leaves walked{} in {} unit(s), {} table read(s){} ({} set-A, {} set-B excluded, {} refused now) -> {n_unmap} unmapped, {n_leaves} mapped in {n_map} call(s), {n_fail} failed, {n_budget} over budget; mirrored {} leaves in {} RM mapping(s) ({} before), {} region object(s) + {} leaf object(s) + {} pooled; device live maps {}/{}; totals: {} maps ({} leaves), {} unmaps ({} partial, {} failed), {} map-failed, {} over budget, {} refused, {} region(s) refused; walk {} us, apply {us_apply} us{}, sync {us} us; {}{}; {}",
            plan.walked,
            if plan.full { " [full walk]" } else { "" },
            plan.units.len(),
            plan.reads,
            if plan.truncated { " [TRUNCATED at the leaf cap]" } else { "" },
            plan.want,
            plan.excluded,
            plan.refused_now,
            self.mapped.len(),
            self.runs.len(),
            live0,
            self.regions.values().filter(|r| r.handle.is_some()).count(),
            self.runs.values().filter(|r| r.own_object).count(),
            self.pool.len(),
            self.budget.live(),
            self.budget.limit,
            self.stats.maps,
            self.stats.leaves_mapped,
            self.stats.unmaps,
            self.stats.partial_unmaps,
            self.stats.unmap_failed,
            self.stats.map_failed,
            self.stats.over_budget,
            self.stats.refused,
            self.stats.regions_refused,
            plan.walk_us,
            if plan.queued_us > 0 { format!(" after {} us in the worker's queue", plan.queued_us) } else { String::new() },
            self.calls.since(&calls0).line(),
            if self.defer_tlb { " [TLB deferred]" } else { "" },
            self.stats.extents(),
        )];
        out.extend(plan.notes.iter().cloned());
        out.extend(settle_notes);
        out.extend(notes);
        (changed, out)
    }

    /// One `RM_MAP_MEMORY_DMA` of `len` bytes from `m`'s backing at `m.va`, deferred when
    /// the switch is on (the VAS then owes an invalidate).
    fn rm_map<H: ShadowHost>(&mut self, h: &H, h_dma: u32, m: &ToMap, len: u64) -> u32 {
        let t0 = Instant::now();
        let st = h.map_dma_fixed(
            h_dma,
            m.h_mem,
            m.off,
            len,
            m.va,
            page_size_flag(m.leaf.size),
            m.snoop,
            kind_override(m.leaf.kind),
            self.defer_tlb,
        );
        self.calls.add(Call::Map, t0);
        self.tlb_owed |= self.defer_tlb;
        st
    }

    #[allow(clippy::too_many_arguments)]
    fn record_run(
        &mut self,
        va: u64,
        len: u64,
        h_dma: u32,
        h_mem: u32,
        own: bool,
        piece: &[ToMap],
        descs: &[(u32, u64, u64)],
    ) {
        self.runs.insert(
            va,
            Run {
                size: len,
                h_dma,
                h_mem,
                own_object: own,
            },
        );
        self.budget.add(1);
        if let Some(c) = self.gathers.get_mut(&h_mem) {
            *c += 1;
        }
        for m in piece {
            self.mapped.insert(m.va, MappedLeaf { leaf: m.leaf, run: va });
        }
        self.stats.maps += 1;
        self.stats.leaves_mapped += piece.len() as u64;
        let t = piece[0].leaf.target;
        self.stats.note_mapped(&t, len, h_mem, descs);
    }

    pub fn summary(&self) -> String {
        let s = &self.stats;
        format!(
            "{} syncs ({} skipped clean, {} full walks, {} table reads, {} units diffed; max {} us, total {} ms), {} maps ({} leaves), {} unmaps ({} partial, {} failed), {} map-failed, {} over budget, {} refused by the filter, {} region objects ({} freed, {} of them whole with {} mappings, {} refused, {} pooled), {} per-leaf objects, {} gather descriptor(s) for {} leaves ({} refused, {} live); {} leaves in {} RM mappings at the end; {}; TLB invalidation {}{}; {}",
            s.syncs,
            s.skipped,
            s.full_walks,
            s.table_reads,
            s.units_diffed,
            s.max_sync_us,
            s.total_sync_us / 1000,
            s.maps,
            s.leaves_mapped,
            s.unmaps,
            s.partial_unmaps,
            s.unmap_failed,
            s.map_failed,
            s.over_budget,
            s.refused,
            s.region_objects,
            s.regions_freed,
            s.bulk_frees,
            s.bulk_runs,
            s.regions_refused,
            self.pool.len(),
            s.leaf_objects,
            s.gathers,
            s.gathered_leaves,
            s.gathers_refused,
            self.gathers.len(),
            self.mapped.len(),
            self.runs.len(),
            self.calls.line(),
            if self.defer_tlb { "deferred" } else { "per call" },
            if s.invalidate_failed > 0 {
                format!(" ({} invalidate(s) FAILED)", s.invalidate_failed)
            } else {
                String::new()
            },
            s.extents()
        )
    }

    /// Free the shadow VAS incrementally: unmap its runs `TEARDOWN_BATCH` at a time with
    /// a pause after each batch (every NVOS47 takes the host GPU lock only for itself),
    /// then free its virtual objects the same way, then the VAS. Returns one line.
    pub fn free<H: ShadowHost>(mut self, hm: &std::sync::Mutex<H>) -> String {
        let t0 = Instant::now();
        let mut steps = 0usize;
        while !self.teardown_step(hm) {
            steps += 1;
            std::thread::sleep(TEARDOWN_PAUSE);
        }
        self.teardown_line(steps + 1, t0.elapsed().as_millis())
    }

    /// One bounded step of the incremental teardown: unmap up to `TEARDOWN_BATCH` runs
    /// (deferred, then one invalidate), or free up to `TEARDOWN_BATCH` virtual objects (the
    /// regions, the pooled regions and the per-leaf objects), or finally the VAS. The host
    /// RM lock is held for the step only. Returns true when everything is freed.
    pub fn teardown_step<H: ShadowHost>(&mut self, hm: &std::sync::Mutex<H>) -> bool {
        let mut h = hm.lock().unwrap();
        if self.teardown_calls0.is_none() {
            self.teardown_calls0 = Some(self.calls);
        }
        if !self.runs.is_empty() && self.bulk_on {
            // Region objects whole, with their mappings: about TEARDOWN_BATCH mappings per
            // step, at least one object.
            let regs: Vec<(u64, u32)> = self
                .regions
                .iter()
                .filter_map(|(u, r)| r.handle.map(|hd| (*u, hd)))
                .filter(|(u, _)| self.runs.range(*u..*u + REGION).next().is_some())
                .collect();
            let mut taken = 0usize;
            for (u, hd) in regs {
                let n = self.runs.range(u..u + REGION).count();
                if taken > 0 && taken + n > TEARDOWN_BATCH {
                    break;
                }
                self.free_region_whole(&mut *h, u, hd);
                self.teardown_unmapped += n;
                self.teardown_bulk += n;
                self.teardown_freed += 1;
                taken += n;
            }
            if taken > 0 {
                return false;
            }
        }
        if !self.runs.is_empty() {
            let batch: Vec<(u64, Run)> = self.runs.iter().take(TEARDOWN_BATCH).map(|(v, r)| (*v, *r)).collect();
            for (va, run) in batch {
                if self.rm_unmap(&*h, &run, va, run.size) != sys::NV_OK {
                    self.stats.unmap_failed += 1;
                }
                if run.own_object {
                    self.teardown_objects.push(run.h_dma);
                }
                self.runs.remove(&va);
                self.run_gone(run.h_mem);
                self.budget.add(-1);
                self.teardown_unmapped += 1;
            }
            self.settle(&*h);
            self.free_idle_gathers(&mut *h);
            return false;
        }
        self.mapped.clear();
        if !self.regions.is_empty() || !self.pool.is_empty() {
            let rs: Vec<u64> = self.regions.keys().copied().collect();
            for r in rs {
                if let Some(hd) = self.regions.remove(&r).and_then(|reg| reg.handle) {
                    self.teardown_objects.push(hd);
                }
            }
            let ps: Vec<u64> = self.pool.keys().copied().collect();
            for r in ps {
                if let Some(hd) = self.pool.remove(&r).and_then(|reg| reg.handle) {
                    self.teardown_objects.push(hd);
                }
            }
        }
        if !self.teardown_objects.is_empty() {
            let n = self.teardown_objects.len().min(TEARDOWN_BATCH);
            for o in self.teardown_objects.drain(..n).collect::<Vec<_>>() {
                self.free_virtual(&mut *h, o);
                self.teardown_freed += 1;
            }
            return false;
        }
        let left: Vec<u32> = std::mem::take(&mut self.gathers).into_keys().collect();
        self.gathers_idle.extend(left);
        self.free_idle_gathers(&mut *h);
        if self.vas != 0 {
            self.settle(&*h);
            let t0 = Instant::now();
            let _ = h.free_object(self.vas);
            self.calls.add(Call::ObjectFree, t0);
            self.vas = 0;
        }
        true
    }

    pub fn teardown_line(&self, steps: usize, ms: u128) -> String {
        format!(
            "incremental teardown: {} RM mapping(s) unmapped ({} with their region object, {} failed) and {} virtual object(s) freed in {steps} step(s) of at most {TEARDOWN_BATCH} mappings, the host RM lock released between steps, TLB invalidation {}, {ms} ms in total; {}",
            self.teardown_unmapped,
            self.teardown_bulk,
            self.stats.unmap_failed,
            self.teardown_freed,
            if self.defer_tlb { "deferred to one per step" } else { "per call" },
            self.calls.since(&self.teardown_calls0.unwrap_or_default()).line()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: u64 = 0x1_0000_0000;
    const END: u64 = 0x2_0000_0000;

    #[test]
    fn the_filter_needs_the_whole_leaf_inside() {
        let descs = [(7u32, 0x7fff_0000_0000u64, 0x4000_0000u64)];
        assert_eq!(
            resolve_leaf(&Target::Fb(BASE), 0x1000, 0, BASE, END, &descs),
            Resolved::Vidmem { offset: 0 }
        );
        // A 2 MiB page that starts inside and ends past the reservation is refused.
        assert!(matches!(
            resolve_leaf(&Target::Fb(END - 0x1000), REGION, 0, BASE, END, &descs),
            Resolved::Refused(_)
        ));
        assert!(matches!(
            resolve_leaf(&Target::Fb(BASE - 0x1000), 0x1000, 0, BASE, END, &descs),
            Resolved::Refused(_)
        ));
        assert_eq!(
            resolve_leaf(&Target::Sys(0x7fff_0000_1000), 0x1000, 0, BASE, END, &descs),
            Resolved::Sysmem {
                h_mem: 7,
                offset: 0x1000
            }
        );
        assert!(matches!(
            resolve_leaf(&Target::Sys(0x7fff_3fff_f000), 0x1_0000, 0, BASE, END, &descs),
            Resolved::Refused(_)
        ));
        // D16: a compressible kind never passes, whatever the address.
        assert!(matches!(
            resolve_leaf(&Target::Fb(BASE), 0x1000, 0x0A, BASE, END, &descs),
            Resolved::Refused(_)
        ));
    }

    fn tm(va: u64, pa: u64, size: u64, kind: u8, h: u32) -> ToMap {
        ToMap {
            va,
            leaf: Leaf {
                target: Target::Fb(pa),
                size,
                kind,
            },
            h_mem: h,
            off: pa - BASE,
            snoop: false,
        }
    }

    #[test]
    fn contiguous_leaves_coalesce_inside_a_region_only() {
        let k = 0x06;
        let items = vec![
            tm(0x1000, BASE + 0x5000, 0x1000, k, 1),
            tm(0x2000, BASE + 0x6000, 0x1000, k, 1), // contiguous: same run
            tm(0x3000, BASE + 0x9000, 0x1000, k, 1), // PA jumps: new run
            tm(0x4000, BASE + 0xA000, 0x1000, 0x00, 1), // kind differs: new run
            tm(0x5000, BASE + 0xB000, 0x1000, 0x00, 2), // backing differs: new run
            tm(REGION - 0x1000, BASE + 0x10_0000, 0x1000, k, 1),
            tm(REGION, BASE + 0x10_1000, 0x1000, k, 1), // crosses the region: new run
            tm(REGION + 0x1000, BASE + 0x10_2000, 0x1_0000, k, 1), // page size differs
        ];
        let runs = coalesce(&items);
        assert_eq!(runs, vec![0..2, 2..3, 3..4, 4..5, 5..6, 6..7, 7..8]);
        // 512 contiguous 4 KiB leaves of one region are one run.
        let many: Vec<ToMap> = (0..512u64)
            .map(|i| tm(0x40_0000 + i * 0x1000, BASE + 0x80_0000 + i * 0x1000, 0x1000, k, 1))
            .collect();
        assert_eq!(coalesce(&many), vec![0..512]);
    }

    #[test]
    fn regions_are_fixed_2mib_and_never_mix_classes() {
        assert_eq!(class_of(0x1000), Class::Small);
        assert_eq!(class_of(0x1_0000), Class::Small);
        assert_eq!(class_of(REGION), Class::Huge);
        assert_eq!(0x100_04e0_1000u64 & !(REGION - 1), 0x100_04e0_0000);
    }

    #[test]
    fn a_leaf_on_a_bar1_pool_page_is_mirrored_to_the_pool_page() {
        // P21 design H (bar1pool.rs): the plan maps a leaf on a remapped page at its pool
        // page, splits a big leaf around it, and re-diffs when only the remap changed.
        let mut fb = FakeFb::new(END + (64 << 20));
        let vid = |a: u64| ((a >> 12) << 8) | 0b010;
        let pte = |a: u64| ((a >> 12) << 8) | 1 | (6u64 << 56);
        let (root, pd2, pd1, pd0, pt, bt) = (
            BASE,
            BASE + 0x1000,
            BASE + 0x2000,
            BASE + 0x3000,
            BASE + 0x4000,
            BASE + 0x5000,
        );
        fb.write_u64(root, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        fb.write_u64(pd0, ((bt >> 8) << 4) | 0b010);
        fb.write_u64(pd0 + 8, vid(pt));
        let x = BASE + 0x10_0000;
        fb.write_u64(pt + 8, pte(x)); // VA 0x1000 -> X (4 KiB)
        fb.write_u64(bt + 8, pte(BASE + 0x20_0000)); // VA 0x10000 -> 64 KiB
        let pool0 = END - (8 << 20);
        fb.set_bar1_pool(0, pool0, 4 << 20, (BASE, pool0));
        let mem = GuestMemory::new();
        let mut front = ShadowFront::new(root);
        let want = |plan: &Plan| -> Vec<(u64, Target, u64, u64)> {
            plan.units
                .iter()
                .flat_map(|(_, w)| w.iter().map(|m| (m.va, m.leaf.target, m.leaf.size, m.off)))
                .collect()
        };
        let p = front.plan(&fb, &mem, BASE, END, 1, &[], &[]).unwrap();
        assert_eq!(want(&p)[0], (0x1000, Target::Fb(x), 0x1000, x - BASE));
        assert!(front.plan(&fb, &mem, BASE, END, 1, &[], &[]).is_none());
        // X and one page of the big leaf move into the pool; the pool poll asks the root
        // to re-diff.
        {
            let mut g = fb.pool_mut();
            let pool = g.as_mut().unwrap();
            pool.table.install(0x3000, x, (BASE, pool0)).unwrap();
            pool.table.install(0x4000, BASE + 0x20_2000, (BASE, pool0)).unwrap();
            pool.rediff.insert(root);
        }
        assert!(front.needs_sync(&fb, &[]));
        let p = front.plan(&fb, &mem, BASE, END, 1, &[], &[]).unwrap();
        let w = want(&p);
        assert_eq!(w.len(), 17);
        assert_eq!(w[0], (0x1000, Target::Fb(pool0 + 0x3000), 0x1000, pool0 + 0x3000 - BASE));
        assert_eq!(w[1], (0x10000, Target::Fb(BASE + 0x20_0000), 0x1000, 0x20_0000));
        assert_eq!(w[3], (0x12000, Target::Fb(pool0 + 0x4000), 0x1000, pool0 + 0x4000 - BASE));
        // The split leaves coalesce around the pool page.
        let items: Vec<ToMap> = p.units.iter().flat_map(|(_, w)| w.iter().copied()).collect();
        assert_eq!(coalesce(&items[1..]).len(), 3);
        assert!(!front.needs_sync(&fb, &[]));
    }

    /// The Xid 31 of 2026-09-24 (docs/caraxes/memory-model.md §2, "The alias in the
    /// device"): a pinned host buffer whose pages the guest placed in the 2 MiB rounding of
    /// its swiotlb alias carries PTEs at alias IOVAs (0xf0_xxxx_x000). Without the guest's
    /// aliases the filter refuses those leaves, the shadow has no PTE there and the copy
    /// engine faults (FAULT_PTE); with them the leaf maps from the registered descriptor
    /// at the page's own offset, and a leaf outside every range stays refused.
    #[test]
    fn a_leaf_at_the_guest_alias_maps_from_the_registered_descriptor() {
        let mut fb = FakeFb::new(END + (64 << 20));
        let vid = |a: u64| ((a >> 12) << 8) | 0b010;
        let sys = |a: u64| ((a >> 12) << 8) | 0b101 | (6u64 << 56);
        let (root, pd2, pd1, pd0, pt) = (BASE, BASE + 0x1000, BASE + 0x2000, BASE + 0x3000, BASE + 0x4000);
        fb.write_u64(root, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        fb.write_u64(pd0 + 8, vid(pt));
        fb.write_u64(pt + 8, sys(0x7fff_0000_1000)); // VA 0x1000: a registered IOVA
        fb.write_u64(pt + 16, sys(0xF0_0000_1000)); // VA 0x2000: the alias of GPA 0x20_1000
        fb.write_u64(pt + 24, sys(0xF0_0040_0000)); // VA 0x3000: past the alias entry
        let dev = crate::dmamap::DmaTable::parse("0:7fff00000000:40000000").unwrap();
        let guest = dev.with_alias(0x20_0000, 0x1000, crate::dmamap::LOW_ALIAS_IOVA).unwrap();
        let descs = [(7u32, 0x7fff_0000_0000u64, 0x4000_0000u64)];
        let want = |plan: &Plan| -> Vec<(u64, u32, u64, bool)> {
            plan.units
                .iter()
                .flat_map(|(_, w)| w.iter().map(|m| (m.va, m.h_mem, m.off, m.snoop)))
                .collect()
        };
        let mut mem = GuestMemory::new();
        mem.set_table(Some(dev.clone()));
        // Before: only the registered IOVA maps.
        let p = ShadowFront::new(root)
            .plan(&fb, &mem, BASE, END, 1, &[], &descs)
            .unwrap();
        assert_eq!(want(&p), vec![(0x1000, 7, 0x1000, true)]);
        assert_eq!(p.refused_now, 2);
        // With the guest's aliases: the alias leaf maps at GPA 0x20_1000's offset.
        mem.set_aliases(dev.aliases_of(&guest).unwrap());
        let p = ShadowFront::new(root)
            .plan(&fb, &mem, BASE, END, 1, &[], &descs)
            .unwrap();
        assert_eq!(want(&p), vec![(0x1000, 7, 0x1000, true), (0x2000, 7, 0x20_1000, true)]);
        assert_eq!(p.refused_now, 1);
        // The leaf keeps the guest's address as its identity for the diff.
        let t = p.units[0].1[1].leaf.target;
        assert_eq!(t, Target::Sys(0xF0_0000_1000));
        let mut st = ShadowStats::default();
        st.note_mapped(&t, 0x1000, 7, &descs);
        assert_eq!(st.sys_chunks.get(&0x7fff_0000_0000), Some(&1));
    }

    #[test]
    fn the_budget_counts_up_and_down() {
        let b = MapBudget::new(3);
        b.add(2);
        b.add(1);
        assert_eq!(b.live(), 3);
        b.add(-2);
        assert_eq!(b.live(), 1);
    }

    // ---- The host RM call batching (memory-model.md §7), against a fake host. ----

    use std::cell::RefCell;
    use std::sync::Mutex;

    /// One host RM call the fake saw: kind, VA (or handle for a free), length, deferred.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Seen {
        call: Call,
        va: u64,
        len: u64,
        defer: bool,
    }

    #[derive(Default)]
    struct FakeHost {
        seen: RefCell<Vec<Seen>>,
        next: u32,
        /// Unmaps at these VAs fail.
        fail_unmap: Vec<u64>,
        /// The invalidate fails.
        fail_invalidate: bool,
        /// Live mappings `(h_dma, va) -> len`: a free of an object with live mappings is
        /// what the RM would auto-unmap, which the tests assert never happens behind the
        /// shadow's back for a pooled object.
        live: RefCell<BTreeMap<(u32, u64), (u64, u32)>>,
        freed: Vec<u32>,
        /// The gathers made: handle and pages.
        gathered: BTreeMap<u32, Vec<(u32, u64)>>,
        fail_gather: bool,
    }

    impl FakeHost {
        fn calls(&self, c: Call) -> Vec<Seen> {
            self.seen.borrow().iter().filter(|s| s.call == c).cloned().collect()
        }
        fn kinds(&self) -> Vec<Call> {
            self.seen.borrow().iter().map(|s| s.call).collect()
        }
    }

    impl ShadowHost for FakeHost {
        fn alloc_virtual_fixed(&mut self, _vas: u32, va: u64, size: u64, _p: Option<u64>, _f: bool) -> Result<u32, u32> {
            self.next += 1;
            self.seen.borrow_mut().push(Seen { call: Call::ObjectAlloc, va, len: size, defer: false });
            Ok(0x100 + self.next)
        }
        fn map_dma_fixed(&self, h_dma: u32, h_mem: u32, _o: u64, len: u64, va: u64, _ps: u32, _s: bool, _k: Option<u8>, defer: bool) -> u32 {
            self.seen.borrow_mut().push(Seen { call: Call::Map, va, len, defer });
            self.live.borrow_mut().insert((h_dma, va), (len, h_mem));
            sys::NV_OK
        }
        fn unmap_dma(&self, h_dma: u32, _m: u32, va: u64, len: u64, defer: bool) -> u32 {
            self.seen.borrow_mut().push(Seen { call: Call::Unmap, va, len, defer });
            if self.fail_unmap.contains(&va) {
                return 0x1f;
            }
            self.live.borrow_mut().remove(&(h_dma, va));
            sys::NV_OK
        }
        fn free_object(&mut self, handle: u32) -> Result<(), u32> {
            self.seen.borrow_mut().push(Seen { call: Call::ObjectFree, va: handle as u64, len: 0, defer: false });
            self.live.borrow_mut().retain(|(h, _), _| *h != handle);
            self.freed.push(handle);
            Ok(())
        }
        fn invalidate_vas(&self, _vas: u32) -> u32 {
            self.seen.borrow_mut().push(Seen { call: Call::Invalidate, va: 0, len: 0, defer: false });
            if self.fail_invalidate {
                0x1f
            } else {
                sys::NV_OK
            }
        }
        fn gather(&mut self, pages: &[(u32, u64)]) -> Result<u32, String> {
            self.seen.borrow_mut().push(Seen { call: Call::Gather, va: 0, len: pages.len() as u64, defer: false });
            if self.fail_gather {
                return Err("refused".into());
            }
            self.next += 1;
            let g = 0x900 + self.next;
            self.gathered.insert(g, pages.to_vec());
            Ok(g)
        }
        fn free_gather(&mut self, handle: u32) -> u32 {
            self.seen.borrow_mut().push(Seen { call: Call::GatherFree, va: handle as u64, len: 0, defer: false });
            assert!(
                self.live.borrow().values().all(|(_, m)| *m != handle),
                "gather {handle:#x} freed while still mapped"
            );
            assert!(self.gathered.remove(&handle).is_some());
            sys::NV_OK
        }
    }

    fn sv(defer: bool, pool: bool) -> ShadowVas {
        let mut s = ShadowVas::with_vas(0x77, MapBudget::new(1 << 20));
        s.coalesce = true;
        s.defer_tlb = defer;
        s.pool_on = pool;
        s.bulk_on = false;
        s.gather_on = false;
        s
    }

    /// A plan holding `leaves` (VA, FB PA, size, kind) in their 2 MiB units.
    fn plan_of(leaves: &[(u64, u64, u64, u8)], units: &[u64]) -> Plan {
        let mut by: BTreeMap<u64, Vec<ToMap>> = units.iter().map(|u| (*u, Vec::new())).collect();
        for &(va, pa, size, kind) in leaves {
            by.entry(va & !(REGION - 1)).or_default().push(tm(va, pa, size, kind, 1));
        }
        Plan {
            root: BASE,
            units: by.into_iter().collect(),
            ..Default::default()
        }
    }

    #[test]
    fn coalescing_keeps_holes_permissions_and_order_apart() {
        let k = 0x06;
        let sys = |va: u64, iova: u64, snoop: bool| ToMap {
            va,
            leaf: Leaf { target: Target::Sys(iova), size: 0x1000, kind: k },
            h_mem: 7,
            off: iova,
            snoop,
        };
        let items = vec![
            tm(0x1000, BASE + 0x1000, 0x1000, k, 1),
            tm(0x3000, BASE + 0x3000, 0x1000, k, 1), // a VA hole (0x2000 unmapped): new run
            tm(0x4000, BASE + 0x4000, 0x1000, k, 1), // contiguous with the one before
            sys(0x5000, 0x5000, true),                // other backing (sysmem descriptor)
            sys(0x6000, 0x6000, false),               // same descriptor, snoop differs
            sys(0x7000, 0x7000, false),               // contiguous: joins
            sys(0x8000, 0x6000, false),               // PA goes backwards: new run
            sys(0x9000, 0x5000, false),               // descending pages never merge
        ];
        assert_eq!(coalesce(&items), vec![0..1, 1..3, 3..4, 4..6, 6..7, 7..8]);
        // Every run is exactly the leaves it covers: VA and backing advance together.
        for r in coalesce(&items) {
            let s = &items[r];
            for w in s.windows(2) {
                assert_eq!(w[1].va - w[0].va, w[1].off - w[0].off);
                assert_eq!((w[1].h_mem, w[1].snoop, w[1].leaf.kind), (w[0].h_mem, w[0].snoop, w[0].leaf.kind));
            }
        }
    }

    /// D16: a compressible leaf inside an otherwise contiguous stretch is refused by the
    /// filter, never reaches a map call, and splits the run around it.
    #[test]
    fn a_compressible_leaf_splits_the_run_and_is_never_mapped() {
        let mut fb = FakeFb::new(END + (64 << 20));
        let vid = |a: u64| ((a >> 12) << 8) | 0b010;
        let pte = |a: u64, kind: u64| ((a >> 12) << 8) | 1 | (kind << 56);
        let (root, pd2, pd1, pd0, pt) = (BASE, BASE + 0x1000, BASE + 0x2000, BASE + 0x3000, BASE + 0x4000);
        fb.write_u64(root, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        fb.write_u64(pd0 + 8, vid(pt));
        let x = BASE + 0x10_0000;
        for i in 0..6u64 {
            let kind = if i == 3 { 0x0A } else { 0x06 };
            fb.write_u64(pt + 8 * (i + 1), pte(x + i * 0x1000, kind));
        }
        let mem = GuestMemory::new();
        let plan = ShadowFront::new(root).plan(&fb, &mem, BASE, END, 1, &[], &[]).unwrap();
        assert_eq!(plan.want, 5);
        assert_eq!(plan.refused_now, 1);
        let host = Mutex::new(FakeHost::default());
        let mut s = sv(true, true);
        let (changed, lines) = s.apply(&host, &plan, &[]);
        assert!(changed);
        let h = host.lock().unwrap();
        let maps = h.calls(Call::Map);
        assert_eq!(
            maps.iter().map(|m| (m.va, m.len)).collect::<Vec<_>>(),
            vec![(0x1000, 0x3000), (0x5000, 0x2000)]
        );
        assert!(maps.iter().all(|m| !(m.va..m.va + m.len).contains(&0x4000)));
        assert!(lines.iter().any(|l| l.contains("REFUSED leaf va 0x4000") && l.contains("D16")));
    }

    #[test]
    fn deferred_calls_owe_one_invalidate_paid_at_the_end_of_the_plan() {
        let host = Mutex::new(FakeHost::default());
        let mut s = sv(true, true);
        // Three runs in two regions: 2 region objects, 3 maps, 1 invalidate, in that order.
        let p = plan_of(
            &[
                (0x1000, BASE + 0x1000, 0x1000, 6),
                (0x2000, BASE + 0x2000, 0x1000, 6),
                (0x8000, BASE + 0x9000, 0x1000, 6),
                (REGION + 0x1000, BASE + 0x20_0000, 0x1000, 6),
            ],
            &[],
        );
        let (_, lines) = s.apply(&host, &p, &[]);
        let h = host.lock().unwrap();
        assert!(h.calls(Call::Map).iter().all(|m| m.defer));
        assert_eq!(h.calls(Call::Map).len(), 3);
        assert_eq!(*h.kinds().last().unwrap(), Call::Invalidate);
        assert_eq!(h.calls(Call::Invalidate).len(), 1);
        assert_eq!(s.calls.count(Call::ObjectAlloc), 2);
        assert!(lines[0].contains("RM calls 6: map 3") && lines[0].contains("[TLB deferred]"));
        // A plan with nothing to change owes nothing.
        drop(h);
        s.apply(&host, &p, &[]);
        assert_eq!(host.lock().unwrap().calls(Call::Invalidate).len(), 1);
    }

    #[test]
    fn no_object_is_freed_before_the_owed_invalidate() {
        let host = Mutex::new(FakeHost::default());
        let mut s = sv(true, false);
        let full = plan_of(&[(0x1000, BASE + 0x1000, 0x1000, 6), (0x3000, BASE + 0x5000, 0x1000, 6)], &[]);
        s.apply(&host, &full, &[]);
        host.lock().unwrap().seen.borrow_mut().clear();
        // The guest empties the region: two deferred unmaps, the invalidate, then the free.
        s.apply(&host, &plan_of(&[], &[0]), &[]);
        let h = host.lock().unwrap();
        assert_eq!(h.kinds(), vec![Call::Unmap, Call::Unmap, Call::Invalidate, Call::ObjectFree]);
        assert!(h.calls(Call::Unmap).iter().all(|u| u.defer));
        assert_eq!(s.mapped_leaves(), 0);
        assert_eq!(s.budget.live(), 0);
    }

    /// The BAR1 pool's re-diff (bar1pool.rs: a page moved behind a successful barrier)
    /// re-plans every unit of the root, but the host sees calls only in the region of the
    /// moved page. A region whose leaves did not change keeps its object, its mapping and
    /// so its PDE, and a re-diff in which nothing changed makes no call at all
    /// (docs/caraxes/bar1-pool.md §9).
    #[test]
    fn a_pool_rediff_touches_only_the_region_of_the_moved_page() {
        let host = Mutex::new(FakeHost::default());
        let mut s = sv(false, true);
        let (a, b) = (0x20_0000u64, 0x40_0000u64);
        let (pa, pb) = (BASE + 0x20_0000, BASE + 0x40_0000);
        s.apply(&host, &plan_of(&[(a, pa, REGION, 6), (b, pb, REGION, 6)], &[]), &[]);
        let hb = s.regions[&b].handle.unwrap();
        host.lock().unwrap().seen.borrow_mut().clear();
        // A re-diff with nothing moved: no call.
        s.apply(&host, &plan_of(&[(a, pa, REGION, 6), (b, pb, REGION, 6)], &[a, b]), &[]);
        assert!(host.lock().unwrap().seen.borrow().is_empty());
        // Page 3 of region A moved to its pool page: A's 2 MiB leaf becomes 4 KiB leaves
        // (bar1pool.rs `split_leaf`), region B is unchanged.
        let p = BASE + 0x100_0000;
        let mut leaves: Vec<(u64, u64, u64, u8)> = (0..512u64)
            .map(|i| (a + i * 0x1000, if i == 3 { p } else { pa + i * 0x1000 }, 0x1000, 6))
            .collect();
        leaves.push((b, pb, REGION, 6));
        s.apply(&host, &plan_of(&leaves, &[a, b]), &[]);
        let h = host.lock().unwrap();
        assert!(!h.seen.borrow().is_empty());
        for c in h.seen.borrow().iter() {
            match c.call {
                Call::ObjectFree => assert_ne!(c.va, hb as u64, "region B's object freed"),
                Call::Invalidate | Call::GatherFree | Call::Gather => {}
                _ => assert!(c.va >= a && c.va < a + REGION, "a call outside region A: {c:?}"),
            }
        }
        assert_eq!(h.live.borrow().get(&(hb, b)).map(|m| m.0), Some(REGION));
    }

    #[test]
    fn an_emptied_region_is_pooled_and_taken_back_without_new_objects() {
        let host = Mutex::new(FakeHost::default());
        let mut s = sv(true, true);
        let a = plan_of(&[(0x1000, BASE + 0x1000, 0x1000, 6)], &[]);
        s.apply(&host, &a, &[]);
        s.apply(&host, &plan_of(&[], &[0]), &[]);
        {
            let h = host.lock().unwrap();
            assert!(h.calls(Call::ObjectFree).is_empty(), "pooled, not freed");
            assert!(h.live.borrow().is_empty(), "a pooled object holds no mapping");
        }
        assert_eq!((s.calls.pooled, s.pool.len()), (1, 1));
        // The guest maps the region again, elsewhere in it: no new object.
        s.apply(&host, &plan_of(&[(0x9000, BASE + 0x40_0000, 0x1000, 6)], &[]), &[]);
        assert_eq!(s.calls.count(Call::ObjectAlloc), 1);
        assert_eq!((s.calls.reused, s.pool.len()), (1, 0));
        // Emptied again and then needed with 2 MiB pages: the pooled small-page object is
        // freed (after the owed invalidate) and a huge-page one allocated.
        s.apply(&host, &plan_of(&[], &[0]), &[]);
        host.lock().unwrap().seen.borrow_mut().clear();
        s.apply(&host, &plan_of(&[(0, BASE + 0x60_0000, REGION, 6)], &[]), &[]);
        let h = host.lock().unwrap();
        assert_eq!(h.kinds(), vec![Call::ObjectFree, Call::ObjectAlloc, Call::Map, Call::Invalidate]);
    }

    /// Fail closed: a region in which an unmap failed may still hold a mapping on the host,
    /// so it is freed (the RM takes the mapping with the object), never pooled.
    #[test]
    fn a_region_with_a_failed_unmap_is_freed_not_pooled() {
        let host = Mutex::new(FakeHost {
            fail_unmap: vec![0x3000],
            ..Default::default()
        });
        let mut s = sv(true, true);
        s.apply(&host, &plan_of(&[(0x1000, BASE + 0x1000, 0x1000, 6), (0x3000, BASE + 0x5000, 0x1000, 6)], &[]), &[]);
        s.apply(&host, &plan_of(&[], &[0]), &[]);
        let h = host.lock().unwrap();
        assert_eq!(s.pool.len(), 0);
        assert_eq!(h.freed.len(), 1);
        assert!(h.live.borrow().is_empty());
        assert_eq!(s.stats.unmap_failed, 1);
    }

    /// A pooled object in a range that becomes set B is freed before anything else.
    #[test]
    fn a_pooled_region_under_a_new_exclusion_is_freed() {
        let host = Mutex::new(FakeHost::default());
        let mut s = sv(true, true);
        s.apply(&host, &plan_of(&[(0x1000, BASE + 0x1000, 0x1000, 6)], &[]), &[]);
        s.apply(&host, &plan_of(&[], &[0]), &[]);
        assert_eq!(s.pool.len(), 1);
        let mut p = plan_of(&[], &[]);
        p.exclude = vec![(0x10_0000, 0x20_0000)];
        s.apply(&host, &p, &[]);
        assert_eq!(s.pool.len(), 0);
        assert_eq!(host.lock().unwrap().freed.len(), 1);
    }

    #[test]
    fn a_refused_invalidate_turns_the_deferral_off() {
        let host = Mutex::new(FakeHost {
            fail_invalidate: true,
            ..Default::default()
        });
        let mut s = sv(true, true);
        let (_, lines) = s.apply(&host, &plan_of(&[(0x1000, BASE + 0x1000, 0x1000, 6)], &[]), &[]);
        assert!(!s.defers_tlb());
        assert_eq!(s.stats.invalidate_failed, 1);
        assert!(lines.iter().any(|l| l.starts_with("ERROR shadow: DMA_INVALIDATE_TLB")));
        host.lock().unwrap().seen.borrow_mut().clear();
        s.apply(&host, &plan_of(&[(0x1000, BASE + 0x1000, 0x1000, 6), (0x3000, BASE + 0x3000, 0x1000, 6)], &[]), &[]);
        let h = host.lock().unwrap();
        assert_eq!(h.kinds(), vec![Call::Map]);
        assert!(!h.calls(Call::Map)[0].defer);
    }

    /// With both switches off the calls are the earlier sequence: no deferral, no
    /// invalidate, every emptied region freed.
    #[test]
    fn switches_off_make_the_earlier_calls() {
        let host = Mutex::new(FakeHost::default());
        let mut s = sv(false, false);
        s.apply(&host, &plan_of(&[(0x1000, BASE + 0x1000, 0x1000, 6), (0x3000, BASE + 0x5000, 0x1000, 6)], &[]), &[]);
        s.apply(&host, &plan_of(&[], &[0]), &[]);
        let h = host.lock().unwrap();
        assert_eq!(
            h.kinds(),
            vec![Call::ObjectAlloc, Call::Map, Call::Map, Call::Unmap, Call::Unmap, Call::ObjectFree]
        );
        assert!(h.seen.borrow().iter().all(|c| !c.defer));
    }

    #[test]
    fn the_teardown_defers_per_step_and_frees_the_pool() {
        let host = Mutex::new(FakeHost::default());
        let mut s = sv(true, true);
        // 70 separate runs in one region (every other page), plus one pooled region.
        let leaves: Vec<(u64, u64, u64, u8)> =
            (0..70u64).map(|i| (0x1000 + 2 * i * 0x1000, BASE + 0x1000 + 2 * i * 0x1000, 0x1000, 6)).collect();
        s.apply(&host, &plan_of(&leaves, &[]), &[]);
        s.apply(&host, &plan_of(&[(REGION + 0x1000, BASE + 0x40_0000, 0x1000, 6)], &[]), &[]);
        s.apply(&host, &plan_of(&[], &[REGION]), &[]);
        assert_eq!((s.live_maps(), s.pool.len()), (70, 1));
        host.lock().unwrap().seen.borrow_mut().clear();
        let line = s.free(&host);
        let h = host.lock().unwrap();
        let k = h.kinds();
        // Step 1: 64 deferred unmaps and an invalidate; step 2: 6 and an invalidate; then
        // the two objects and the VAS.
        assert_eq!(k.iter().filter(|c| **c == Call::Unmap).count(), 70);
        assert_eq!(k[64], Call::Invalidate);
        assert_eq!(k[71], Call::Invalidate);
        assert_eq!(&k[72..], &[Call::ObjectFree, Call::ObjectFree, Call::ObjectFree]);
        assert!(h.live.borrow().is_empty());
        assert!(line.contains("deferred to one per step"));
    }

    /// An emptied region with many runs is freed whole: one RM_FREE, no unmap, and the
    /// fake RM (like the real one) takes the mappings with the object. Few runs still go run
    /// by run (and then to the free list).
    #[test]
    fn an_emptied_region_with_many_runs_is_freed_whole() {
        let host = Mutex::new(FakeHost::default());
        let mut s = sv(false, true);
        s.bulk_on = true;
        let many: Vec<(u64, u64, u64, u8)> =
            (0..10u64).map(|i| (0x1000 + 2 * i * 0x1000, BASE + 0x1000 + 2 * i * 0x1000, 0x1000, 6)).collect();
        s.apply(&host, &plan_of(&many, &[]), &[]);
        s.apply(&host, &plan_of(&[(REGION + 0x1000, BASE + 0x40_0000, 0x1000, 6)], &[]), &[]);
        assert_eq!((s.live_maps(), s.budget.live()), (11, 11));
        host.lock().unwrap().seen.borrow_mut().clear();
        let (_, lines) = s.apply(&host, &plan_of(&[], &[0, REGION]), &[]);
        let h = host.lock().unwrap();
        // Region 0 (10 runs): one free. Region 1 (1 run): an unmap, then the free list.
        assert_eq!(h.kinds(), vec![Call::ObjectFree, Call::Unmap]);
        assert!(h.live.borrow().is_empty());
        assert_eq!((s.live_maps(), s.mapped_leaves(), s.budget.live()), (0, 0, 0));
        assert_eq!((s.stats.bulk_frees, s.stats.bulk_runs, s.pool.len()), (1, 10, 1));
        assert!(lines[0].contains("-> 11 unmapped"));
        // The region is mapped again later: a fresh object, nothing stale in it.
        drop(h);
        s.apply(&host, &plan_of(&[(0x5000, BASE + 0x50_0000, 0x1000, 6)], &[]), &[]);
        assert_eq!(host.lock().unwrap().calls(Call::ObjectAlloc).len(), 1);
        assert_eq!(s.lookup(0x5000).map(|(_, l)| l.target), Some(Target::Fb(BASE + 0x50_0000)));
        assert_eq!(s.lookup(0x1000), None);
    }

    /// With deferral on, a whole-region free pays the owed invalidate first.
    #[test]
    fn a_whole_region_free_pays_the_owed_invalidate_first() {
        let host = Mutex::new(FakeHost::default());
        let mut s = sv(true, false);
        s.bulk_on = true;
        let many: Vec<(u64, u64, u64, u8)> =
            (0..6u64).map(|i| (0x1000 + 2 * i * 0x1000, BASE + 0x1000 + 2 * i * 0x1000, 0x1000, 6)).collect();
        s.apply(&host, &plan_of(&many, &[]), &[]);
        host.lock().unwrap().seen.borrow_mut().clear();
        // One plan: a deferred map in region 1 (listed first), then region 0 emptied: the
        // invalidate comes before the free, and nothing is owed after it.
        let mut p = plan_of(&[(REGION + 0x1000, BASE + 0x40_0000, 0x1000, 6)], &[0]);
        p.units.sort_by_key(|(u, _)| std::cmp::Reverse(*u));
        s.apply(&host, &p, &[]);
        let h = host.lock().unwrap();
        assert_eq!(
            h.kinds(),
            vec![Call::ObjectAlloc, Call::Map, Call::Invalidate, Call::ObjectFree]
        );
    }

    #[test]
    fn the_teardown_frees_whole_regions_about_a_batch_per_step() {
        let host = Mutex::new(FakeHost::default());
        let mut s = sv(false, false);
        s.bulk_on = true;
        // Region 0: 40 runs, region 1: 40 runs, region 2: 1 run.
        let mut leaves = Vec::new();
        for r in 0..2u64 {
            for i in 0..40u64 {
                let va = r * REGION + 0x1000 + 2 * i * 0x1000;
                leaves.push((va, BASE + va, 0x1000, 6));
            }
        }
        leaves.push((2 * REGION + 0x1000, BASE + 2 * REGION + 0x1000, 0x1000, 6));
        s.apply(&host, &plan_of(&leaves, &[]), &[]);
        assert_eq!(s.live_maps(), 81);
        host.lock().unwrap().seen.borrow_mut().clear();
        // Step 1: region 0 (40; region 1 would pass 64). Step 2: region 1 and region 2.
        assert!(!s.teardown_step(&host));
        assert_eq!(s.live_maps(), 41);
        assert!(!s.teardown_step(&host));
        assert_eq!(s.live_maps(), 0);
        let budget = s.budget.clone();
        let line = s.free(&host);
        let h = host.lock().unwrap();
        assert!(h.calls(Call::Unmap).is_empty());
        assert_eq!(h.calls(Call::ObjectFree).len(), 4); // three regions and the VAS
        assert!(h.live.borrow().is_empty());
        assert_eq!(budget.live(), 0);
        assert!(line.contains("81 RM mapping(s) unmapped (81 with their region object"), "{line}");
    }

    // ---- Gather descriptors for scattered sysmem stretches. ----

    const SYS: u64 = 0x7fff_0000_0000;

    /// Sysmem 4 KiB leaves `(va, iova)` of registered descriptor 7 (IOVA base `SYS`).
    fn splan_of(leaves: &[(u64, u64)], units: &[u64]) -> Plan {
        let mut by: BTreeMap<u64, Vec<ToMap>> = units.iter().map(|u| (*u, Vec::new())).collect();
        for &(va, iova) in leaves {
            by.entry(va & !(REGION - 1)).or_default().push(ToMap {
                va,
                leaf: Leaf { target: Target::Sys(iova), size: 0x1000, kind: 6 },
                h_mem: 7,
                off: iova - SYS,
                snoop: true,
            });
        }
        Plan {
            root: BASE,
            units: by.into_iter().collect(),
            ..Default::default()
        }
    }

    /// `n` leaves from `va0` whose pages run backwards (the order a LIFO page pool hands out).
    fn scattered(va0: u64, n: u64) -> Vec<(u64, u64)> {
        (0..n).map(|i| (va0 + i * 0x1000, SYS + 0x10_0000 + (n - 1 - i) * 0x1000)).collect()
    }

    fn gsv() -> ShadowVas {
        let mut s = sv(false, true);
        s.gather_on = true;
        s
    }

    #[test]
    fn a_scattered_sysmem_stretch_is_mapped_through_one_gather() {
        let host = Mutex::new(FakeHost::default());
        let mut s = gsv();
        let leaves = scattered(0x10000, 32);
        let (_, lines) = s.apply(&host, &splan_of(&leaves, &[]), &[]);
        let h = host.lock().unwrap();
        assert_eq!(h.kinds(), vec![Call::Gather, Call::ObjectAlloc, Call::Map]);
        let (g, pages) = h.gathered.iter().next().unwrap();
        // The pages in VA order, each the filter's descriptor and offset.
        assert_eq!(pages.len(), 32);
        assert_eq!(pages[0], (7, 0x10_0000 + 31 * 0x1000));
        assert_eq!(pages[31], (7, 0x10_0000));
        let m = &h.calls(Call::Map)[0];
        assert_eq!((m.va, m.len), (0x10000, 32 * 0x1000));
        assert_eq!(h.live.borrow().values().next().unwrap().1, *g);
        // The leaves keep the guest's addresses as their identity.
        assert_eq!(s.lookup(0x10000).map(|(_, l)| l.target), Some(Target::Sys(SYS + 0x10_0000 + 31 * 0x1000)));
        assert_eq!((s.stats.gathers, s.stats.gathered_leaves, s.live_maps()), (1, 32, 1));
        assert!(lines[0].contains("gather alloc 1"));
    }

    #[test]
    fn contiguous_short_or_mixed_stretches_are_not_gathered() {
        let host = Mutex::new(FakeHost::default());
        let mut s = gsv();
        // 32 ascending pages coalesce by themselves; 10 scattered are below the minimum.
        let mut leaves: Vec<(u64, u64)> = (0..32u64).map(|i| (0x10000 + i * 0x1000, SYS + 0x20_0000 + i * 0x1000)).collect();
        leaves.extend(scattered(0x80000, 10));
        s.apply(&host, &splan_of(&leaves, &[]), &[]);
        let h = host.lock().unwrap();
        assert!(h.calls(Call::Gather).is_empty());
        assert_eq!(h.calls(Call::Map).len(), 1 + 10);
        drop(h);
        // A kind change and a VA hole end a stretch: 16 + 16 scattered leaves around a hole
        // make two gathers, never one across the hole.
        let host = Mutex::new(FakeHost::default());
        let mut s = gsv();
        let mut leaves = scattered(0x10000, 16);
        leaves.extend(scattered(0x21000, 16)); // 0x20000 is a hole
        s.apply(&host, &splan_of(&leaves, &[]), &[]);
        let h = host.lock().unwrap();
        assert_eq!(h.calls(Call::Gather).len(), 2);
        assert_eq!(
            h.calls(Call::Map).iter().map(|m| (m.va, m.len)).collect::<Vec<_>>(),
            vec![(0x10000, 0x10000), (0x21000, 0x10000)]
        );
    }

    #[test]
    fn a_refused_gather_maps_run_by_run_from_the_registered_descriptor() {
        let host = Mutex::new(FakeHost {
            fail_gather: true,
            ..Default::default()
        });
        let mut s = gsv();
        let (_, lines) = s.apply(&host, &splan_of(&scattered(0x10000, 20), &[]), &[]);
        let h = host.lock().unwrap();
        assert_eq!(h.calls(Call::Map).len(), 20);
        assert!(h.live.borrow().values().all(|(_, m)| *m == 7));
        assert_eq!(s.stats.gathers_refused, 1);
        assert!(lines.iter().any(|l| l.contains("gather of 20 leaves") && l.contains("refused")));
    }

    #[test]
    fn a_gather_lives_until_its_last_mapping_is_gone() {
        for bulk in [false, true] {
            let host = Mutex::new(FakeHost::default());
            let mut s = gsv();
            s.bulk_on = bulk;
            let leaves = scattered(0x10000, 32);
            s.apply(&host, &splan_of(&leaves, &[]), &[]);
            // The guest moves page 5: the gather run is split around it, the gather stays.
            let mut moved = leaves.clone();
            moved[5].1 = SYS + 0x40_0000;
            s.apply(&host, &splan_of(&moved, &[]), &[]);
            {
                let h = host.lock().unwrap();
                assert!(h.calls(Call::GatherFree).is_empty());
                assert_eq!(h.gathered.len(), 1);
            }
            assert_eq!(s.live_maps(), 3);
            assert_eq!(s.lookup(0x15000).map(|(_, l)| l.target), Some(Target::Sys(SYS + 0x40_0000)));
            // The guest empties the region: the gather is freed only after its mappings
            // (the fake RM asserts that), by unmaps or with the region object.
            s.apply(&host, &splan_of(&[], &[0]), &[]);
            let h = host.lock().unwrap();
            assert_eq!(h.calls(Call::GatherFree).len(), 1, "bulk {bulk}");
            assert!(h.gathered.is_empty() && h.live.borrow().is_empty());
            assert_eq!((s.live_maps(), s.budget.live()), (0, 0));
        }
    }

    #[test]
    fn the_teardown_frees_gathers_after_their_mappings() {
        for bulk in [false, true] {
            let host = Mutex::new(FakeHost::default());
            let mut s = gsv();
            s.bulk_on = bulk;
            s.apply(&host, &splan_of(&scattered(0x10000, 32), &[]), &[]);
            s.apply(&host, &splan_of(&scattered(REGION + 0x10000, 32), &[]), &[]);
            s.free(&host);
            let h = host.lock().unwrap();
            assert_eq!(h.calls(Call::GatherFree).len(), 2);
            assert!(h.gathered.is_empty() && h.live.borrow().is_empty());
            assert_eq!(*h.kinds().last().unwrap(), Call::ObjectFree);
        }
    }
}

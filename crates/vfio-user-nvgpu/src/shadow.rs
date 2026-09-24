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

/// What a guest leaf points at: the identity the mirror diffs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leaf {
    pub target: Target,
    pub size: u64,
    pub kind: u8,
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
    fn note_mapped(&mut self, t: &Target, size: u64, descs: &[(u32, u64, u64)]) {
        match t {
            Target::Fb(pa) => {
                let (lo, hi) = self.vid_extent.unwrap_or((*pa, *pa + size));
                self.vid_extent = Some((lo.min(*pa), hi.max(*pa + size)));
            }
            Target::Sys(iova) => {
                if let Some((_, base, _)) = descs
                    .iter()
                    .find(|(_, b, sz)| *iova >= *b && *iova < *b + *sz)
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
        self.always || exclude != self.exclude.as_slice() || fb.tables_dirty(Target::Fb(self.root))
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
        if exclude_changed || upd.full {
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
            let leaves = fb
                .with_walk(root, |c| c.unit_leaves(u).to_vec())
                .unwrap_or_default();
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
                match resolve_leaf(t, *size, *kind, base, end, descs) {
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
        Ok(ShadowVas {
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
            coalesce: std::env::var("CARAXES_SHADOW_COALESCE").as_deref() != Ok("0"),
        })
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
    fn unmap_leaves(&mut self, hm: &mut HostMemory, vas: &[u64]) {
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

    fn unmap_piece(&mut self, hm: &mut HostMemory, run_va: u64, s: u64, e: u64) {
        let Some(run) = self.runs.get(&run_va).copied() else {
            return;
        };
        let run_end = run_va + run.size;
        let whole = s == run_va && e == run_end;
        let st = hm.unmap_dma(run.h_dma, run.h_mem, s, e - s);
        self.stats.unmaps += 1;
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
            let gone: Vec<u64> = self.mapped.range(s..e).map(|(va, _)| *va).collect();
            for va in gone {
                self.mapped.remove(&va);
            }
        } else {
            self.stats.unmap_failed += 1;
            if !whole {
                // The RM refused the split: take the whole run down; the caller maps its
                // other leaves again.
                let st2 = hm.unmap_dma(run.h_dma, run.h_mem, run_va, run.size);
                self.stats.unmaps += 1;
                if st2 != sys::NV_OK {
                    self.stats.unmap_failed += 1;
                }
            }
            self.drop_run(hm, run_va, run);
        }
    }

    /// Forget a run whose mapping is gone (and free its per-leaf object).
    fn drop_run(&mut self, hm: &mut HostMemory, run_va: u64, run: Run) {
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
            let _ = hm.free_object(run.h_dma);
        }
    }

    /// Apply a [`Plan`]: per unit, unmap what the guest removed or changed, drop a region
    /// object of the wrong class, and map what is new, coalesced into runs. The host RM lock
    /// is taken per unit, so a long plan never holds it for more than one unit's RM calls.
    /// Returns `(changed, notes)`, the first note the one-line summary of the sync.
    pub fn apply(
        &mut self,
        hm: &std::sync::Mutex<HostMemory>,
        plan: &Plan,
        descs: &[(u32, u64, u64)],
    ) -> (bool, Vec<String>) {
        let t0 = Instant::now();
        let exclude = &plan.exclude;
        let mut notes = Vec::new();
        let mut logged = 0usize;
        let live0 = self.runs.len();
        let (mut n_unmap, mut n_map, mut n_leaves, mut n_fail, mut n_budget) =
            (0usize, 0usize, 0usize, 0usize, 0usize);
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
            n_unmap += unmap.len();
            self.unmap_leaves(&mut h, &unmap);
            if stale {
                let inside: Vec<u64> = self.mapped.range(u..u + REGION).map(|(v, _)| *v).collect();
                self.unmap_leaves(&mut h, &inside);
                if let Some(reg) = self.regions.remove(&u) {
                    if let Some(hd) = reg.handle {
                        let _ = h.free_object(hd);
                        self.stats.regions_freed += 1;
                    }
                }
            }
            // Map what is wanted and not mirrored (a refused split may have taken more down).
            let todo: Vec<ToMap> = want
                .values()
                .filter(|m| !self.mapped.contains_key(&m.va) && self.failed.get(&m.va) != Some(&m.leaf))
                .copied()
                .collect();
            for r in coalesce(&todo) {
                let items = &todo[r];
                let first = items[0];
                let rv = first.va & !(REGION - 1);
                let class = class_of(first.leaf.size);
                if !self.regions.contains_key(&rv) {
                    let touches_b = exclude.iter().any(|(lo, hi)| rv < *hi && rv + REGION > *lo);
                    let handle = if touches_b {
                        None
                    } else {
                        let page = match class {
                            Class::Huge => Some(REGION),
                            Class::Small => None,
                        };
                        match h.alloc_virtual_fixed(self.vas, rv, REGION, page, false) {
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
                        None => match h.alloc_virtual_fixed(self.vas, p0.va, len, Some(p0.leaf.size), true) {
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
                        },
                    };
                    let mut st = h.map_dma_fixed(
                        h_dma,
                        p0.h_mem,
                        p0.off,
                        len,
                        p0.va,
                        page_size_flag(p0.leaf.size),
                        p0.snoop,
                    );
                    if st == sys::NV_OK {
                        self.record_run(p0.va, len, h_dma, p0.h_mem, own, piece, descs);
                        n_map += 1;
                        n_leaves += piece.len();
                        continue;
                    }
                    if own {
                        let _ = h.free_object(h_dma);
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
                        st = h.map_dma_fixed(
                            h_dma,
                            m.h_mem,
                            m.off,
                            m.leaf.size,
                            m.va,
                            page_size_flag(m.leaf.size),
                            m.snoop,
                        );
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
        let changed = n_unmap > 0 || n_map > 0 || n_fail > 0 || n_budget > 0 || logged > 0 || !plan.notes.is_empty();
        let mut out = vec![format!(
            "shadow: sync {} leaves walked{} in {} unit(s), {} table read(s){} ({} set-A, {} set-B excluded, {} refused now) -> {n_unmap} unmapped, {n_leaves} mapped in {n_map} call(s), {n_fail} failed, {n_budget} over budget; mirrored {} leaves in {} RM mapping(s) ({} before), {} region object(s) + {} leaf object(s); device live maps {}/{}; totals: {} maps ({} leaves), {} unmaps ({} partial, {} failed), {} map-failed, {} over budget, {} refused, {} region(s) refused; walk {} us, apply {us_apply} us{}, sync {us} us; {}",
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
            self.stats.extents(),
        )];
        out.extend(plan.notes.iter().cloned());
        out.extend(notes);
        (changed, out)
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
        for m in piece {
            self.mapped.insert(m.va, MappedLeaf { leaf: m.leaf, run: va });
        }
        self.stats.maps += 1;
        self.stats.leaves_mapped += piece.len() as u64;
        let t = piece[0].leaf.target;
        self.stats.note_mapped(&t, len, descs);
    }

    pub fn summary(&self) -> String {
        let s = &self.stats;
        format!(
            "{} syncs ({} skipped clean, {} full walks, {} table reads, {} units diffed; max {} us, total {} ms), {} maps ({} leaves), {} unmaps ({} partial, {} failed), {} map-failed, {} over budget, {} refused by the filter, {} region objects ({} freed, {} refused), {} per-leaf objects; {} leaves in {} RM mappings at the end; {}",
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
            s.regions_refused,
            s.leaf_objects,
            self.mapped.len(),
            self.runs.len(),
            s.extents()
        )
    }

    /// Free the shadow VAS incrementally: unmap its runs `TEARDOWN_BATCH` at a time with
    /// a pause after each batch (every NVOS47 takes the host GPU lock only for itself),
    /// then free its virtual objects the same way, then the VAS. Returns one line.
    pub fn free(mut self, hm: &std::sync::Mutex<HostMemory>) -> String {
        let t0 = Instant::now();
        let mut steps = 0usize;
        while !self.teardown_step(hm) {
            steps += 1;
            std::thread::sleep(TEARDOWN_PAUSE);
        }
        self.teardown_line(steps + 1, t0.elapsed().as_millis())
    }

    /// One bounded step of the incremental teardown: unmap up to `TEARDOWN_BATCH` runs, or
    /// free up to `TEARDOWN_BATCH` virtual objects, or finally the VAS. The host RM lock is
    /// held for the step only. Returns true when everything is freed.
    pub fn teardown_step(&mut self, hm: &std::sync::Mutex<HostMemory>) -> bool {
        let mut h = hm.lock().unwrap();
        if !self.runs.is_empty() {
            let batch: Vec<(u64, Run)> = self.runs.iter().take(TEARDOWN_BATCH).map(|(v, r)| (*v, *r)).collect();
            for (va, run) in batch {
                if h.unmap_dma(run.h_dma, run.h_mem, va, run.size) != sys::NV_OK {
                    self.stats.unmap_failed += 1;
                }
                if run.own_object {
                    self.teardown_objects.push(run.h_dma);
                }
                self.runs.remove(&va);
                self.budget.add(-1);
                self.teardown_unmapped += 1;
            }
            return false;
        }
        self.mapped.clear();
        if !self.regions.is_empty() {
            let rs: Vec<u64> = self.regions.keys().copied().collect();
            for r in rs {
                if let Some(hd) = self.regions.remove(&r).and_then(|reg| reg.handle) {
                    self.teardown_objects.push(hd);
                }
            }
        }
        if !self.teardown_objects.is_empty() {
            let n = self.teardown_objects.len().min(TEARDOWN_BATCH);
            for o in self.teardown_objects.drain(..n).collect::<Vec<_>>() {
                let _ = h.free_object(o);
                self.teardown_freed += 1;
            }
            return false;
        }
        if self.vas != 0 {
            let _ = h.free_object(self.vas);
            self.vas = 0;
        }
        true
    }

    pub fn teardown_line(&self, steps: usize, ms: u128) -> String {
        format!(
            "incremental teardown: {} RM mapping(s) unmapped ({} failed) and {} virtual object(s) freed in {steps} step(s) of at most {TEARDOWN_BATCH}, the host RM lock released between steps, {ms} ms in total",
            self.teardown_unmapped, self.stats.unmap_failed, self.teardown_freed
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
    fn the_budget_counts_up_and_down() {
        let b = MapBudget::new(3);
        b.add(2);
        b.add(1);
        assert_eq!(b.live(), 3);
        b.add(-2);
        assert_eq!(b.live(), 1);
    }
}

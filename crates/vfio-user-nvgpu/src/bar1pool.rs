// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! P21 design H, the BAR1 pool (docs/caraxes/bar1-pool.md, bar1-passthrough.md §4 H):
//! `CARAXES_BAR1_PASSTHROUGH=pool`.
//!
//! A private pool of `len` bytes is carved from the top of the guest's usable FB (right
//! below the device's carve-out; the region table reports it as reserved) and exported as
//! the dma-buf that backs one sparse mmap area of the guest's BAR1, `[0, len)`. The guest
//! CPU reaches area page `g` at pool page `pool0 + g` without a trap, whatever the guest
//! maps there. When the guest maps area page `g` to its FB page `X` (its BAR1 page tables,
//! walked here as the prototype's check walks them), the device gives `X` the pool page
//! `P = pool0 + g`: it copies `X` to `P` and, from then on, every translation it performs
//! treats `X` as living at `P` ([`crate::fb::FakeFb::remap`]): the FB store, the shadow's
//! leaves and the USERD a host channel is given. When the guest unmaps `g` (or maps it
//! elsewhere) the device copies `P` back to `X` and drops the remap.
//!
//! A page the physical GPU may be using when its remap changes (a shadow leaf maps it) is
//! moved only behind a barrier: every host channel on the affected shadow roots idle
//! (GPGet equal to GPPut) and preempted with wait, the copy, the re-sync of those roots'
//! shadows and the wait for the shadow worker (mirror.rs `shadow_barrier`, the rule the
//! doorbell path follows). A page that is a mirrored channel's USERD, or whose barrier
//! cannot be had, is refused and retried later: the device's view of it stays where it
//! was (fail closed), and the guest's CPU accesses of that area page reach only the pool
//! page, inside its own reservation.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::fb::{FakeFb, FbLayout, Target};
use crate::guestmem::GuestMemory;

const PAGE: u64 = 4096;
const MIB: u64 = 1 << 20;
/// The area is walked and tracked in 2 MiB units (one PD0 entry each).
const UNIT: u64 = 2 * MIB;

/// `CARAXES_BAR1_POOL_MIB` default.
pub const DEFAULT_POOL_MIB: u64 = 64;
/// The largest pool: the dma-buf maps all of it into the host's shared BAR1 (256 MiB on
/// this card, docs/caraxes/parallel.md §2).
pub const MAX_POOL_MIB: u64 = 128;
/// Host BAR1 one device may hold with a pool: the pool plus its window budget
/// (`CARAXES_BAR1_WINDOWS` x `CARAXES_BAR1_WINDOW_MIB`, 64 MiB by default).
pub const MAX_HOST_BAR1: u64 = 192 * MIB;
/// Refused remaps are retried at most this often (at the next trapped BAR0 write after).
pub const RETRY_EVERY: Duration = Duration::from_millis(1);
/// Findings logged at most (the counters are complete).
const LOG_FIRST: usize = 32;

/// The switch: `CARAXES_BAR1_PASSTHROUGH=pool`, the pool size from `CARAXES_BAR1_POOL_MIB`
/// (an even number of MiB, 2..=128, default 64). `Ok(Some(bytes))` when on; an invalid
/// size is an error, never a silent default.
pub fn config() -> Result<Option<u64>, String> {
    if std::env::var("CARAXES_BAR1_PASSTHROUGH").as_deref() != Ok("pool") {
        return Ok(None);
    }
    let mib = match std::env::var("CARAXES_BAR1_POOL_MIB") {
        Err(_) => DEFAULT_POOL_MIB,
        Ok(v) => v
            .trim()
            .parse::<u64>()
            .map_err(|_| format!("CARAXES_BAR1_POOL_MIB={v}: not a number of MiB"))?,
    };
    if !(2..=MAX_POOL_MIB).contains(&mib) || mib % 2 != 0 {
        return Err(format!(
            "CARAXES_BAR1_POOL_MIB={mib}: the pool must be an even number of MiB from 2 to {MAX_POOL_MIB}"
        ));
    }
    Ok(Some(mib * MIB))
}

/// The bounds of a pool of `pool` bytes for `layout` (without a pool yet), a guest BAR1 of
/// `bar1` bytes and a host BAR1 window budget of `windows` bytes: 2 MiB units; at most
/// half the guest's BAR1 (the guest RM keeps the upper half for its own mappings, the
/// RM-owned USERD is mapped top-down there); the pool plus the windows at most
/// [`MAX_HOST_BAR1`] of the host's shared BAR1; and at most a quarter of the usable FB,
/// which shrinks by the pool.
pub fn check_size(layout: &FbLayout, pool: u64, bar1: u64, windows: u64) -> Result<(), String> {
    let (lo, hi) = layout
        .usable()
        .ok_or("the BAR1 pool needs a VRAM reservation (--reserve-vram)")?;
    if layout.pool_range().is_some() {
        return Err("the layout has a BAR1 pool already".into());
    }
    if pool == 0 || pool % UNIT != 0 || pool > MAX_POOL_MIB * MIB {
        return Err(format!(
            "BAR1 pool {pool:#x}: a multiple of 2 MiB up to {MAX_POOL_MIB} MiB"
        ));
    }
    if pool > bar1 / 2 {
        return Err(format!(
            "BAR1 pool {} MiB: more than half the guest's BAR1 ({} MiB)",
            pool / MIB,
            bar1 / MIB
        ));
    }
    if pool + windows > MAX_HOST_BAR1 {
        return Err(format!(
            "BAR1 pool {} MiB plus the window budget {} MiB exceed the {} MiB of host BAR1 one device may hold (CARAXES_BAR1_POOL_MIB, CARAXES_BAR1_WINDOWS, CARAXES_BAR1_WINDOW_MIB)",
            pool / MIB,
            windows / MIB,
            MAX_HOST_BAR1 / MIB
        ));
    }
    if pool > (hi - lo) / 4 {
        return Err(format!(
            "BAR1 pool {} MiB: more than a quarter of the usable FB ({} MiB)",
            pool / MIB,
            (hi - lo) / MIB
        ));
    }
    Ok(())
}

/// Why a remap cannot be installed (static reasons; the GPU reasons are strings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The area offset is outside the area or not page aligned.
    BadSlot,
    /// The area page holds another page's remap still (its release was refused).
    SlotBusy,
    /// The FB page lives in the pool already, through another area page (aliasing).
    Aliased,
    /// The FB page is not in the guest's usable FB (the pool itself, the carve-out with
    /// the BAR roots and the invalidator, or outside the reservation).
    NotUsable,
}

impl Refusal {
    pub fn why(self) -> &'static str {
        match self {
            Refusal::BadSlot => "the area offset is not a page of the area",
            Refusal::SlotBusy => "the area page still holds another page whose move back was refused",
            Refusal::Aliased => "the page lives in the pool already through another area page (the guest maps it twice)",
            Refusal::NotUsable => "the page is not in the guest's usable FB (pool, carve-out or outside the reservation)",
        }
    }
}

/// The remap table: guest FB page `X` lives at pool page `pool0 + g` while the guest maps
/// area page `g` to it. Pure bookkeeping, no FB access.
#[derive(Debug, Clone)]
pub struct RemapTable {
    pool0: u64,
    len: u64,
    /// Guest FB page address `X` -> area offset `g`.
    fwd: BTreeMap<u64, u64>,
    /// Area offset `g` -> guest FB page address `X`.
    slots: BTreeMap<u64, u64>,
}

impl RemapTable {
    pub fn new(pool0: u64, len: u64) -> Self {
        RemapTable {
            pool0,
            len,
            fwd: BTreeMap::new(),
            slots: BTreeMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.fwd.is_empty()
    }

    pub fn len(&self) -> usize {
        self.fwd.len()
    }

    /// `(pool0, len)`.
    pub fn pool(&self) -> (u64, u64) {
        (self.pool0, self.len)
    }

    /// The pool page of area offset `g`.
    pub fn pool_page(&self, g: u64) -> u64 {
        self.pool0 + g
    }

    /// Where the device finds guest FB address `pa`.
    pub fn translate(&self, pa: u64) -> u64 {
        match self.fwd.get(&(pa & !(PAGE - 1))) {
            Some(g) => self.pool0 + g + (pa & (PAGE - 1)),
            None => pa,
        }
    }

    pub fn is_remapped(&self, pa: u64) -> bool {
        self.fwd.contains_key(&(pa & !(PAGE - 1)))
    }

    /// The guest page area page `g` holds, if any.
    pub fn holder(&self, g: u64) -> Option<u64> {
        self.slots.get(&g).copied()
    }

    /// Every `(g, X)` held now, ascending by `g`.
    pub fn held(&self) -> Vec<(u64, u64)> {
        self.slots.iter().map(|(g, x)| (*g, *x)).collect()
    }

    /// Whether `[pa, pa + len)` has a remapped page.
    pub fn any_in(&self, pa: u64, len: u64) -> bool {
        let lo = pa & !(PAGE - 1);
        self.fwd.range(lo..pa.saturating_add(len)).next().is_some()
    }

    /// Check that `X` may move to area page `g` now; `usable` is the guest's usable FB.
    pub fn check(&self, g: u64, x: u64, usable: (u64, u64)) -> Result<(), Refusal> {
        if g % PAGE != 0 || g >= self.len {
            return Err(Refusal::BadSlot);
        }
        if x % PAGE != 0 || x < usable.0 || x.saturating_add(PAGE) > usable.1 {
            return Err(Refusal::NotUsable);
        }
        if self.slots.contains_key(&g) {
            return Err(Refusal::SlotBusy);
        }
        if self.fwd.contains_key(&x) {
            return Err(Refusal::Aliased);
        }
        Ok(())
    }

    /// Record that `X` lives at area page `g` (after [`Self::check`] and the copy).
    pub fn install(&mut self, g: u64, x: u64, usable: (u64, u64)) -> Result<(), Refusal> {
        self.check(g, x, usable)?;
        self.slots.insert(g, x);
        self.fwd.insert(x, g);
        Ok(())
    }

    /// Drop the remap of area page `g`; the page it held.
    pub fn release(&mut self, g: u64) -> Option<u64> {
        let x = self.slots.remove(&g)?;
        self.fwd.remove(&x);
        Some(x)
    }

    /// Split the access `[pa, pa + len)` into pieces that are contiguous where the device
    /// finds them: `f(device address, offset in the access, length)`.
    pub fn for_each_piece(&self, pa: u64, len: usize, mut f: impl FnMut(u64, usize, usize)) {
        let mut done = 0usize;
        let mut run: Option<(u64, usize, usize)> = None;
        while done < len {
            let a = pa + done as u64;
            let n = ((PAGE - (a & (PAGE - 1))) as usize).min(len - done);
            let h = self.translate(a);
            run = match run {
                Some((rh, ro, rn)) if rh + rn as u64 == h => Some((rh, ro, rn + n)),
                Some((rh, ro, rn)) => {
                    f(rh, ro, rn);
                    Some((h, done, n))
                }
                None => Some((h, done, n)),
            };
            done += n;
        }
        if let Some((rh, ro, rn)) = run {
            f(rh, ro, rn);
        }
    }

    /// A guest leaf `(va, target, size, kind)` as the GPU must map it: unchanged when no
    /// page of it is remapped, else split into 4 KiB leaves on the pages' device addresses.
    pub fn split_leaf(&self, l: (u64, Target, u64, u8)) -> Vec<(u64, Target, u64, u8)> {
        let (va, t, size, kind) = l;
        let Target::Fb(pa) = t else {
            return vec![l];
        };
        if !self.any_in(pa, size) {
            return vec![l];
        }
        (0..size / PAGE)
            .map(|i| {
                let o = i * PAGE;
                (va + o, Target::Fb(self.translate(pa + o)), PAGE, kind)
            })
            .collect()
    }
}

/// Counters for the log and the exit summary.
#[derive(Default, Debug, Clone)]
pub struct PoolStats {
    /// Polls that did work (a walk or a retry).
    pub polls: u64,
    pub units_walked: u64,
    /// Pages moved into the pool: no GPU reference (cold), behind the barrier (warm).
    pub installs_cold: u64,
    pub installs_warm: u64,
    /// Pages moved back.
    pub releases_cold: u64,
    pub releases_warm: u64,
    /// Remaps refused (each distinct area page and target once).
    pub refused: u64,
    /// Area pages the guest maps to guest RAM (the area serves VRAM only).
    pub sysmem: u64,
    /// Barriers taken and refused, and the TSGs preempted by them.
    pub barriers: u64,
    pub barriers_refused: u64,
    pub preempts: u64,
    /// Bytes copied into and out of the pool, and the time the copies took.
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub copy_us: u128,
    pub max_poll_us: u128,
    /// Remapped pages that are guest page-table pages the shadow watches (guest writes
    /// through the area bypass its write tracking).
    pub table_hazards: u64,
    /// Pages moved into the pool while guest BAR2 maps them too (the BAR2 memfd holds
    /// them by guest address): the device's reads and writes and the memfd's write-back
    /// agree (bar2-memfd.md §8), but a guest CPU write through the area reaches the guest's
    /// BAR2 view of the page only after a device write of the same bytes.
    pub bar2_aliased: u64,
    pub first: Vec<String>,
}

/// One 2 MiB unit of the area as last walked.
#[derive(Default, Clone)]
struct UnitState {
    tables: Vec<u64>,
    mapped: Vec<(u64, Target)>,
}

/// The pool state inside [`FakeFb`].
pub struct Bar1Pool {
    pub root: u64,
    pub table: RemapTable,
    usable: (u64, u64),
    units: Vec<UnitState>,
    /// Table page -> units whose walk read it.
    page_units: HashMap<u64, BTreeSet<usize>>,
    dirty: BTreeSet<usize>,
    /// Area pages whose wanted remap is refused now: `g -> (target, reason)`.
    pending: BTreeMap<u64, (Target, String)>,
    /// A release was refused and waits for its retry.
    release_waits: bool,
    /// `(g, key(target))` of every refusal logged.
    reported: HashSet<(u64, u64)>,
    reported_release: HashSet<(u64, u64)>,
    /// Shadow roots that must re-diff every unit at their next sync (a page they map
    /// moved).
    pub rediff: BTreeSet<u64>,
    hazards: HashSet<u64>,
    pub hazard_dirty: bool,
    last_retry: Option<Instant>,
    pub stats: PoolStats,
}

impl Bar1Pool {
    pub fn new(root: u64, pool0: u64, len: u64, usable: (u64, u64)) -> Self {
        let n = len.div_ceil(UNIT) as usize;
        Bar1Pool {
            root,
            table: RemapTable::new(pool0, len),
            usable,
            units: vec![UnitState::default(); n],
            page_units: HashMap::new(),
            dirty: (0..n).collect(),
            pending: BTreeMap::new(),
            release_waits: false,
            reported: HashSet::new(),
            reported_release: HashSet::new(),
            rediff: BTreeSet::new(),
            hazards: HashSet::new(),
            hazard_dirty: true,
            last_retry: None,
            stats: PoolStats::default(),
        }
    }

    /// The state for the fresh model of a function-level reset: same pool and counters,
    /// no remaps, everything to walk again.
    pub fn after_reset(self) -> Self {
        let (pool0, len) = self.table.pool();
        let mut p = Bar1Pool::new(self.root, pool0, len, self.usable);
        p.stats = self.stats;
        p.reported = self.reported;
        p.reported_release = self.reported_release;
        p.hazards = self.hazards;
        p
    }

    /// A guest write reached FB pages `first..=last`: re-walk the units whose tables
    /// they hold.
    pub fn note_table_write(&mut self, first: u64, last: u64) {
        if self.page_units.is_empty() {
            return;
        }
        for p in first..=last {
            if let Some(us) = self.page_units.get(&p) {
                self.dirty.extend(us.iter().copied());
            }
        }
    }

    fn unit_pages(&self, u: usize) -> usize {
        let (_, len) = self.table.pool();
        ((len - u as u64 * UNIT).min(UNIT) / PAGE) as usize
    }

    fn set_unit(&mut self, u: usize, mapped: Vec<(u64, Target)>, tables: Vec<u64>) {
        for p in &self.units[u].tables {
            if let Some(s) = self.page_units.get_mut(p) {
                s.remove(&u);
                if s.is_empty() {
                    self.page_units.remove(p);
                }
            }
        }
        for p in &tables {
            self.page_units.entry(*p).or_default().insert(u);
        }
        self.units[u] = UnitState { tables, mapped };
    }

    fn finding(&mut self, error: bool, line: String) {
        if self.stats.first.len() < LOG_FIRST {
            eprintln!(
                "vfio-user-nvgpu: {}bar1 pool: {line}",
                if error { "ERROR: " } else { "" }
            );
            self.stats.first.push(line);
        }
    }

    pub fn summary(&self) -> String {
        let s = &self.stats;
        let (pool0, len) = self.table.pool();
        format!(
            "bar1 pool: guest BAR1 [0, {len:#x}) on the pool FB [{pool0:#x}, {:#x}); {} poll(s), {} unit walk(s); {} page(s) in the pool now; moved in {} cold + {} behind the barrier, back {} cold + {} behind the barrier; {} barrier(s) ({} refused, {} TSG preempt(s)); {} refused, {} pending now, {} sysmem page(s) in the area; copied {} KiB in, {} KiB back in {} us; {} page-table hazard(s); {} page(s) also behind the BAR2 memfd; slowest poll {} us (docs/caraxes/bar1-pool.md)",
            pool0 + len,
            s.polls,
            s.units_walked,
            self.table.len(),
            s.installs_cold,
            s.installs_warm,
            s.releases_cold,
            s.releases_warm,
            s.barriers,
            s.barriers_refused,
            s.preempts,
            s.refused,
            self.pending.len(),
            s.sysmem,
            s.bytes_in >> 10,
            s.bytes_out >> 10,
            s.copy_us,
            s.table_hazards,
            s.bar2_aliased,
            s.max_poll_us
        )
    }
}

/// What the GPU side holds of an FB page (from [`PoolGpu::refs`]).
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Refs {
    /// Shadow roots with a leaf on the page (the GPU can reach it through them).
    pub roots: BTreeSet<u64>,
    /// A mirrored channel whose USERD is on the page (the host channel holds its address).
    pub userd: Option<u32>,
}

/// The GPU side of the pool: who may be using a page, and the barrier.
pub trait PoolGpu {
    /// A reason no remap may change at all now (a host channel walks the guest's own
    /// tables, which the remap cannot reach).
    fn blocked(&self) -> Option<String>;
    /// The GPU references of each FB page in `pages`.
    fn refs(&self, fb: &FakeFb, pages: &BTreeSet<u64>) -> BTreeMap<u64, Refs>;
    /// Make sure no GPU work runs on the shadow VA spaces of `roots` until the next
    /// doorbell: every host channel there idle, its TSG preempted with wait.
    /// `Ok((channels, TSGs preempted))`, or why not.
    fn quiesce(&mut self, fb: &mut FakeFb, roots: &BTreeSet<u64>) -> Result<(usize, usize), String>;
    /// Re-sync the shadows of `roots` (the remap is in place) and wait for the worker.
    fn resync(&mut self, fb: &mut FakeFb, roots: &BTreeSet<u64>) -> Vec<String>;
}

/// No GPU side (no host execution, and the tests): nothing is ever in use.
pub struct NoGpu;

impl PoolGpu for NoGpu {
    fn blocked(&self) -> Option<String> {
        None
    }
    fn refs(&self, _fb: &FakeFb, _pages: &BTreeSet<u64>) -> BTreeMap<u64, Refs> {
        BTreeMap::new()
    }
    fn quiesce(&mut self, _fb: &mut FakeFb, _roots: &BTreeSet<u64>) -> Result<(usize, usize), String> {
        Ok((0, 0))
    }
    fn resync(&mut self, _fb: &mut FakeFb, _roots: &BTreeSet<u64>) -> Vec<String> {
        Vec::new()
    }
}

/// The host mirror as the pool's GPU side.
pub struct MirrorGpu<'a> {
    pub mirror: &'a mut crate::mirror::Mirror,
    pub reg: &'a crate::channel::Registry,
    pub sys: &'a GuestMemory,
}

/// USERD offsets of GPGet and GPPut (`clc46f.h:61-62`).
const USERD_GP_GET: u64 = 0x88;
const USERD_GP_PUT: u64 = 0x8C;

impl PoolGpu for MirrorGpu<'_> {
    fn blocked(&self) -> Option<String> {
        let direct: Vec<u32> = self
            .mirror
            .pool_channels()
            .iter()
            .filter(|c| c.1.is_none())
            .map(|c| c.0)
            .collect();
        (!direct.is_empty()).then(|| {
            format!(
                "host channel(s) of guest chid {direct:?} run on an externally-owned VA space and walk the guest's own tables, which the remap cannot reach"
            )
        })
    }

    fn refs(&self, fb: &FakeFb, pages: &BTreeSet<u64>) -> BTreeMap<u64, Refs> {
        let mut out: BTreeMap<u64, Refs> = BTreeMap::new();
        if pages.is_empty() {
            return out;
        }
        // The cached walks are what the shadows hold only once the worker applied every
        // plan (a leaf the guest dropped may still be mapped until then).
        self.mirror.shadow_barrier();
        for r in self.mirror.shadow_roots() {
            // Fail closed: a shadow root without a cached walk may map any page.
            let Some(leaves) = fb.with_walk(Target::Fb(r), |c| c.all_leaves()) else {
                for p in pages {
                    out.entry(*p).or_default().roots.insert(r);
                }
                continue;
            };
            for (_, t, size, _) in leaves {
                let Target::Fb(pa) = t else { continue };
                for p in pages.range(pa & !(PAGE - 1)..pa.saturating_add(size)) {
                    out.entry(*p).or_default().roots.insert(r);
                }
            }
        }
        for (chid, _, _) in self.mirror.pool_channels() {
            if let Some(Target::Fb(u)) = self.reg.channel(chid).and_then(|c| c.userd) {
                let page = u & !(PAGE - 1);
                if pages.contains(&page) {
                    out.entry(page).or_default().userd = Some(chid);
                }
            }
        }
        out
    }

    fn quiesce(&mut self, fb: &mut FakeFb, roots: &BTreeSet<u64>) -> Result<(usize, usize), String> {
        if std::env::var("CARAXES_BAR1_POOL_WARM").as_deref() == Ok("refuse") {
            return Err("CARAXES_BAR1_POOL_WARM=refuse: no page the GPU maps is moved".into());
        }
        let chans: Vec<(u32, Option<u64>, Option<u32>)> = self
            .mirror
            .pool_channels()
            .into_iter()
            .filter(|c| c.1.is_some_and(|r| roots.contains(&r)))
            .collect();
        let gm = crate::channel::GpuMem { fb, sys: self.sys };
        let idle = |chid: u32| -> Result<(), String> {
            let Some(u) = self.reg.channel(chid).and_then(|c| c.userd) else {
                return Err(format!("chid {chid} has no USERD to read"));
            };
            let at = |o: u64| match u {
                Target::Fb(pa) => Target::Fb(pa + o),
                Target::Sys(pa) => Target::Sys(pa + o),
                t => t,
            };
            match (gm.read_u32(at(USERD_GP_GET)), gm.read_u32(at(USERD_GP_PUT))) {
                (Some(g), Some(p)) if g == p => Ok(()),
                (g, p) => Err(format!("chid {chid} busy (GPGet {g:?}, GPPut {p:?})")),
            }
        };
        for c in &chans {
            idle(c.0)?;
        }
        let tsgs: BTreeSet<u32> = chans.iter().filter_map(|c| c.2).collect();
        for t in &tsgs {
            let st = self.mirror.preempt_tsg(*t);
            if st != nvrm_abi::sys::NV_OK {
                return Err(format!("preempt of host TSG {t:#x}: status {st:#x}"));
            }
        }
        // GPPut may have moved while the preempt ran (a guest CPU write through the area):
        // read again, after the TSGs are off the engines.
        for c in &chans {
            idle(c.0)?;
        }
        Ok((chans.len(), tsgs.len()))
    }

    fn resync(&mut self, fb: &mut FakeFb, roots: &BTreeSet<u64>) -> Vec<String> {
        let mut gm = crate::channel::GpuMem { fb, sys: self.sys };
        let mut notes = Vec::new();
        for r in roots {
            notes.extend(self.mirror.shadow_resync(self.reg, &mut gm, Some(*r)));
        }
        // The doorbell ordering rule (mirror.rs `gr_shadow_doorbell`): the worker has
        // applied every plan before anything else runs on the GPU.
        self.mirror.shadow_barrier();
        gm.fb.flush();
        notes
    }
}

/// Poll the pool of the boot model after a trapped BAR0 write (device.rs): the guest
/// flushes and invalidates through BAR0 after it changes a BAR1 mapping and before it uses
/// the mapping. Returns whether a poll ran.
pub fn poll_model(b: &mut crate::boot::BootModel, mem: &GuestMemory) -> bool {
    if b.fb.bar1_pool().is_none() {
        return false;
    }
    let held = |fb: &FakeFb| fb.pool_mut().as_ref().map_or(0, |p| p.table.len());
    let before = held(&b.fb);
    let reg = &b.ctrl.channels;
    let ran = match b.ctrl.mirror.as_mut() {
        Some(m) => {
            let mut gpu = MirrorGpu {
                mirror: m,
                reg,
                sys: mem,
            };
            poll(&mut b.fb, &mut gpu)
        }
        None => poll(&mut b.fb, &mut NoGpu),
    };
    // The pool emptied (a guest context went away): the counters so far, once per context,
    // for a loop of workloads to read per iteration (docs/caraxes/bar1-pool.md §9).
    if before > 0 && held(&b.fb) == 0 {
        if let Some(s) = b.fb.bar1_pool_summary() {
            let hold = b.ctrl.mirror.as_ref().map(|m| m.hold_summary()).unwrap_or_default();
            eprintln!("vfio-user-nvgpu: at {} pool empty: {s}; {hold}", crate::untrapped::wall());
        }
    }
    ran
}

/// One unit's walk: its mapped pages and the table pages read.
type Walked = (Vec<(u64, Target)>, Vec<u64>);

/// One remap change the poll decided on.
#[derive(Clone, Copy, Debug)]
struct Op {
    install: bool,
    g: u64,
    x: u64,
}

/// Bring the remap table in line with the guest's BAR1 tables over the area: walk the
/// units whose tables changed, release what the guest unmapped (copy back), install what it
/// newly mapped (copy in), each page cold or behind the barrier, and retry what was refused
/// (at most every [`RETRY_EVERY`]). Returns whether anything ran.
pub fn poll(fb: &mut FakeFb, gpu: &mut dyn PoolGpu) -> bool {
    let t0 = Instant::now();
    let (root, work, retry, hazard) = {
        let mut guard = fb.pool_mut();
        let Some(p) = guard.as_mut() else {
            return false;
        };
        let retry = (!p.pending.is_empty() || p.release_waits)
            && p.last_retry.is_none_or(|t| t.elapsed() >= RETRY_EVERY);
        if p.dirty.is_empty() && !retry && !p.hazard_dirty {
            return false;
        }
        let work: Vec<(usize, usize)> = std::mem::take(&mut p.dirty)
            .into_iter()
            .map(|u| (u, p.unit_pages(u)))
            .collect();
        (p.root, work, retry, p.hazard_dirty)
    };
    // The walk reads through the FB (and so through the pool borrow): outside it.
    let walked: Vec<(usize, Walked)> = work
        .iter()
        .map(|&(u, n)| {
            (u, fb.area_walk_unit(root, u as u64 * UNIT, n))
        })
        .collect();
    let (releases, installs, usable) = {
        let mut guard = fb.pool_mut();
        let p = guard.as_mut().expect("checked above");
        p.stats.polls += 1;
        p.stats.units_walked += walked.len() as u64;
        if retry {
            p.last_retry = Some(Instant::now());
        }
        for (u, (m, t)) in walked {
            p.set_unit(u, m, t);
        }
        let want: BTreeMap<u64, Target> = p
            .units
            .iter()
            .flat_map(|u| u.mapped.iter().copied())
            .collect();
        let releases: Vec<Op> = p
            .table
            .held()
            .into_iter()
            .filter(|(g, x)| want.get(g) != Some(&Target::Fb(*x)))
            .map(|(g, x)| Op {
                install: false,
                g,
                x,
            })
            .collect();
        let mut installs = Vec::new();
        let mut sys_lines = Vec::new();
        for (g, t) in &want {
            match *t {
                Target::Fb(x) if p.table.holder(*g) != Some(x) => installs.push(Op {
                    install: true,
                    g: *g,
                    x,
                }),
                Target::Sys(_) => {
                    if p.reported.insert((*g, key(*t))) {
                        p.stats.sysmem += 1;
                        sys_lines.push(format!(
                            "guest BAR1 {g:#x} maps {t:x?}: the area serves VRAM only; guest CPU accesses of this page reach its pool page {:#x}, not guest RAM",
                            p.table.pool_page(*g)
                        ));
                    }
                }
                _ => {}
            }
        }
        // Pending entries the guest no longer wants are dropped.
        p.pending.retain(|g, (t, _)| want.get(g) == Some(t));
        for l in sys_lines {
            p.finding(true, l);
        }
        (releases, installs, p.usable)
    };
    if releases.is_empty() && installs.is_empty() {
        if hazard {
            check_hazards(fb);
        }
        note_time(fb, t0);
        return true;
    }
    // Who holds each page on the GPU.
    let pages: BTreeSet<u64> = releases.iter().chain(&installs).map(|o| o.x).collect();
    let blocked = gpu.blocked();
    let refs = gpu.refs(fb, &pages);
    let mut refused: Vec<(Op, String)> = Vec::new();
    let mut cold: Vec<Op> = Vec::new();
    let mut warm: Vec<Op> = Vec::new();
    let mut roots: BTreeSet<u64> = BTreeSet::new();
    // The static checks run against the table as it will be after the releases (each
    // install is checked again when it runs).
    let after = {
        let mut t = fb.pool_mut().as_ref().expect("on").table.clone();
        for r in &releases {
            t.release(r.g);
        }
        t
    };
    for op in releases.iter().chain(&installs) {
        if op.install {
            if let Err(e) = after.check(op.g, op.x, usable) {
                refused.push((*op, e.why().into()));
                continue;
            }
        }
        if let Some(why) = &blocked {
            refused.push((*op, why.clone()));
            continue;
        }
        let r = refs.get(&op.x).cloned().unwrap_or_default();
        if let Some(chid) = r.userd {
            refused.push((
                *op,
                format!("the page is the USERD of mirrored channel chid {chid}, whose host channel holds its address until it is freed"),
            ));
            continue;
        }
        if r.roots.is_empty() {
            cold.push(*op);
        } else {
            roots.extend(r.roots.iter().copied());
            warm.push(*op);
        }
    }
    let mut notes = Vec::new();
    let barrier = if warm.is_empty() {
        None
    } else {
        Some(gpu.quiesce(fb, &roots))
    };
    let warm_ok = matches!(barrier, Some(Ok(_)));
    if let Some(Err(e)) = &barrier {
        for op in &warm {
            refused.push((*op, format!("the GPU may be using it and the barrier failed: {e}")));
        }
    }
    // Releases first (they free area pages and pages for the installs), then installs.
    let mut order: Vec<(Op, bool)> = cold.iter().map(|o| (*o, false)).collect();
    if warm_ok {
        order.extend(warm.iter().map(|o| (*o, true)));
    }
    order.sort_by_key(|(o, _)| o.install);
    let mut moved_warm = 0usize;
    for (op, is_warm) in order {
        let (pool0, _) = fb.bar1_pool().expect("on");
        let p = pool0 + op.g;
        let tc = Instant::now();
        if op.install {
            let ok = {
                let mut guard = fb.pool_mut();
                let pool = guard.as_mut().expect("on");
                pool.table.check(op.g, op.x, usable)
            };
            if let Err(e) = ok {
                refused.push((op, e.why().into()));
                continue;
            }
            fb.copy_page_raw(op.x, p);
            let mut guard = fb.pool_mut();
            let pool = guard.as_mut().expect("on");
            pool.table
                .install(op.g, op.x, usable)
                .expect("checked just before");
            pool.stats.bytes_in += PAGE;
            if is_warm {
                pool.stats.installs_warm += 1;
            } else {
                pool.stats.installs_cold += 1;
            }
            pool.stats.copy_us += tc.elapsed().as_micros();
            crate::profile::record(
                crate::profile::Key::Window(if is_warm {
                    "bar1 pool: page moved in, warm (units: bytes)"
                } else {
                    "bar1 pool: page moved in, cold (units: bytes)"
                }),
                tc.elapsed(),
                PAGE,
                None,
                "",
            );
            pool.pending.remove(&op.g);
        } else {
            fb.copy_page_raw(p, op.x);
            let mut guard = fb.pool_mut();
            let pool = guard.as_mut().expect("on");
            pool.table.release(op.g);
            pool.stats.bytes_out += PAGE;
            if is_warm {
                pool.stats.releases_warm += 1;
            } else {
                pool.stats.releases_cold += 1;
            }
            pool.stats.copy_us += tc.elapsed().as_micros();
            crate::profile::record(
                crate::profile::Key::Window(if is_warm {
                    "bar1 pool: page moved out, warm (units: bytes)"
                } else {
                    "bar1 pool: page moved out, cold (units: bytes)"
                }),
                tc.elapsed(),
                PAGE,
                None,
                "",
            );
        }
        fb.forget_watched_page(op.x >> 12);
        if op.install && matches!(fb.backing(op.x), crate::fb::Backing::Bar2Memfd(_)) {
            let mut guard = fb.pool_mut();
            let pool = guard.as_mut().expect("on");
            pool.stats.bar2_aliased += 1;
            pool.finding(
                false,
                format!(
                    "FB {:#x} (guest BAR1 {:#x}) is behind the BAR2 memfd as well: device accesses and write-backs go to its pool page, guest BAR2 reads see guest area writes only after a device write",
                    op.x, op.g
                ),
            );
        }
        if is_warm {
            moved_warm += 1;
        }
    }
    fb.flush();
    if warm_ok && moved_warm > 0 {
        {
            let mut guard = fb.pool_mut();
            let pool = guard.as_mut().expect("on");
            pool.rediff.extend(roots.iter().copied());
            pool.stats.barriers += 1;
        }
        crate::profile::window("bar1 pool: warm-move barriers (units: pages)", moved_warm as u64);
        notes.extend(gpu.resync(fb, &roots));
        let (nc, nt) = match &barrier {
            Some(Ok(q)) => *q,
            _ => (0, 0),
        };
        let line = format!(
            "{moved_warm} page(s) the GPU maps moved behind the barrier ({nc} host channel(s) idle, {nt} TSG(s) preempted); shadow(s) of root(s) {roots:x?} re-synced and the worker drained before the guest continues"
        );
        let mut guard = fb.pool_mut();
        guard.as_mut().expect("on").finding(false, line);
    }
    {
        let mut guard = fb.pool_mut();
        let pool = guard.as_mut().expect("on");
        if let Some(Err(e)) = &barrier {
            pool.stats.barriers_refused += 1;
            crate::profile::window("bar1 pool: barriers refused", 0);
            // Every refusal, with the wall time, for the reader of the host kernel log (a
            // refused barrier moves nothing; the pages stay and are retried).
            eprintln!(
                "vfio-user-nvgpu: bar1 pool: at {} barrier {} refused for {} page(s) of root(s) {roots:x?} (first g {:#x} FB {:#x}): {e}; nothing moved, retried",
                crate::untrapped::wall(),
                pool.stats.barriers_refused,
                warm.len(),
                warm[0].g,
                warm[0].x
            );
        }
        if let Some(Ok((_, nt))) = &barrier {
            pool.stats.preempts += *nt as u64;
        }
        pool.release_waits = false;
        for (op, why) in refused {
            if !op.install {
                // A release that cannot happen keeps the remap: the page stays consistent
                // at its pool page; only the area page cannot take a new page yet.
                pool.release_waits = true;
                if pool.reported_release.insert((op.g, op.x)) {
                    pool.stats.refused += 1;
                    let line = format!(
                        "guest BAR1 {:#x} no longer maps FB {:#x}, but the page cannot move back from its pool page {:#x} yet: {why}; it stays in the pool (consistent) and is retried",
                        op.g, op.x, pool.table.pool_page(op.g)
                    );
                    pool.finding(true, line);
                }
                continue;
            }
            let t = Target::Fb(op.x);
            pool.pending.insert(op.g, (t, why.clone()));
            if pool.reported.insert((op.g, key(t))) {
                pool.stats.refused += 1;
                let line = format!(
                    "guest BAR1 {:#x} maps FB {:#x}: remap refused ({why}); the device keeps the page where it is (fail closed), guest CPU accesses of this area page reach only its pool page {:#x}; retried",
                    op.g, op.x, pool.table.pool_page(op.g)
                );
                pool.finding(true, line);
            }
        }
    }
    for n in notes {
        if n.contains("REFUSED") || n.contains("ERROR") {
            eprintln!("vfio-user-nvgpu: bar1 pool: {n}");
        }
    }
    check_hazards(fb);
    note_time(fb, t0);
    true
}

/// A hashable key of a page target (`Target` has no `Hash`): sysmem with bit 63.
fn key(t: Target) -> u64 {
    match t {
        Target::Fb(pa) => pa,
        Target::Sys(pa) => pa | 1 << 63,
        _ => u64::MAX,
    }
}

fn note_time(fb: &FakeFb, t0: Instant) {
    if let Some(p) = fb.pool_mut().as_mut() {
        p.stats.max_poll_us = p.stats.max_poll_us.max(t0.elapsed().as_micros());
    }
}

/// Count remapped pages that are guest page-table pages a cached walk watches.
fn check_hazards(fb: &FakeFb) {
    let held = match fb.pool_mut().as_mut() {
        Some(p) => {
            p.hazard_dirty = false;
            p.table.held()
        }
        None => return,
    };
    let hits: Vec<(u64, u64)> = held
        .into_iter()
        .filter(|(_, x)| fb.watches_table_page(x >> 12))
        .collect();
    let mut guard = fb.pool_mut();
    let Some(p) = guard.as_mut() else { return };
    for (g, x) in hits {
        if p.hazards.insert(x >> 12) {
            p.stats.table_hazards += 1;
            let line = format!(
                "guest BAR1 {g:#x} maps FB {x:#x}, a page-table page the shadow watches: guest writes through the area bypass the shadow's write tracking (CARAXES_SHADOW_ALWAYS_RESYNC=1 is the safe setting while this persists)"
            );
            p.finding(true, line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A 64 MiB reservation [64 MiB, 128 MiB) in a 256 MiB FB with a 4 MiB pool: the pool is
    // [122 MiB, 126 MiB), the carve-out [126 MiB, 128 MiB), usable [64 MiB, 122 MiB).
    const BASE: u64 = 0x400_0000;
    const END: u64 = 0x800_0000;
    const POOL: u64 = 4 * MIB;

    fn layout() -> FbLayout {
        FbLayout::reserved(32 * MIB, 256 * MIB, BASE, END)
            .unwrap()
            .with_pool(POOL)
            .unwrap()
    }

    fn vid(a: u64) -> u64 {
        ((a >> 12) << 8) | 0b010
    }

    fn pte(a: u64) -> u64 {
        ((a >> 12) << 8) | 1
    }

    /// A FakeFb with the pool on and the guest's BAR1 tables for the first 2 MiB unit of
    /// the area: the FB, the small page table's address and the pool base.
    fn setup() -> (FakeFb, u64, u64) {
        let l = layout();
        let mut fb = FakeFb::for_layout(&l);
        let (_, root) = l.bar_roots().unwrap();
        let (pd2, pd1, pd0, pt) = (BASE + 0x1000, BASE + 0x2000, BASE + 0x3000, BASE + 0x4000);
        fb.write_u64(root, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        fb.write_u64(pd0 + 8, vid(pt));
        let (pool0, pend) = l.pool_range().unwrap();
        assert_eq!((pool0, pend), (END - 2 * MIB - POOL, END - 2 * MIB));
        fb.set_bar1_pool(root, pool0, POOL, l.usable().unwrap());
        (fb, pt, pool0)
    }

    /// A GPU side that says every page in `used` is mapped by shadow root `root` and every
    /// page in `userd` is a mirrored channel's USERD, and whose barrier holds when `ok`.
    struct FakeGpu {
        used: BTreeSet<u64>,
        userd: BTreeSet<u64>,
        root: u64,
        ok: bool,
        blocked: bool,
        resynced: Vec<u64>,
    }

    impl FakeGpu {
        fn new(root: u64) -> Self {
            FakeGpu {
                used: BTreeSet::new(),
                userd: BTreeSet::new(),
                root,
                ok: true,
                blocked: false,
                resynced: Vec::new(),
            }
        }
    }

    impl PoolGpu for FakeGpu {
        fn blocked(&self) -> Option<String> {
            self.blocked.then(|| "a direct walker".into())
        }
        fn refs(&self, _fb: &FakeFb, pages: &BTreeSet<u64>) -> BTreeMap<u64, Refs> {
            let mut m = BTreeMap::new();
            for p in pages {
                let mut r = Refs::default();
                if self.used.contains(p) {
                    r.roots.insert(self.root);
                }
                if self.userd.contains(p) {
                    r.userd = Some(7);
                }
                m.insert(*p, r);
            }
            m
        }
        fn quiesce(&mut self, _fb: &mut FakeFb, _roots: &BTreeSet<u64>) -> Result<(usize, usize), String> {
            if self.ok {
                Ok((2, 1))
            } else {
                Err("chid 3 busy".into())
            }
        }
        fn resync(&mut self, fb: &mut FakeFb, roots: &BTreeSet<u64>) -> Vec<String> {
            for r in roots {
                // What the shadow's plan does: it takes the re-diff.
                assert!(fb.pool_rediff_pending(*r));
                assert!(fb.take_pool_rediff(*r));
                self.resynced.push(*r);
            }
            Vec::new()
        }
    }

    fn stats(fb: &FakeFb) -> PoolStats {
        fb.pool_mut().as_ref().unwrap().stats.clone()
    }

    fn pending(fb: &FakeFb) -> usize {
        fb.pool_mut().as_ref().unwrap().pending.len()
    }

    fn expire_retry(fb: &FakeFb) {
        fb.pool_mut().as_mut().unwrap().last_retry = None;
    }

    #[test]
    fn the_remap_table_installs_translates_and_releases() {
        let usable = (BASE, END - 6 * MIB);
        let pool0 = END - 6 * MIB;
        let mut t = RemapTable::new(pool0, POOL);
        assert!(t.is_empty());
        let x = BASE + 0x10_0000;
        t.install(0x3000, x, usable).unwrap();
        assert_eq!(t.translate(x + 0x18), pool0 + 0x3018);
        assert_eq!(t.translate(x + 0x1000), x + 0x1000);
        assert!(t.is_remapped(x + 0xFFF) && !t.is_remapped(x - 1));
        assert_eq!(t.holder(0x3000), Some(x));
        assert_eq!(t.held(), vec![(0x3000, x)]);
        // Refusals: the slot is busy, the page is there already, not usable, bad slot.
        assert_eq!(t.check(0x3000, x + 0x1000, usable), Err(Refusal::SlotBusy));
        assert_eq!(t.check(0x4000, x, usable), Err(Refusal::Aliased));
        assert_eq!(t.check(0x4000, pool0, usable), Err(Refusal::NotUsable));
        assert_eq!(t.check(0x4000, END - 2 * MIB, usable), Err(Refusal::NotUsable));
        assert_eq!(t.check(0x4000, BASE - PAGE, usable), Err(Refusal::NotUsable));
        assert_eq!(t.check(0x4000, x + 0x10, usable), Err(Refusal::NotUsable));
        assert_eq!(t.check(POOL, x + 0x1000, usable), Err(Refusal::BadSlot));
        assert_eq!(t.check(0x4010, x + 0x1000, usable), Err(Refusal::BadSlot));
        assert!(t.install(0x4000, x, usable).is_err());
        // An access across the remapped page is split where the device finds it, and the
        // contiguous rest stays one piece.
        let mut pieces = Vec::new();
        t.for_each_piece(x - 0x10, 0x2020, |h, o, n| pieces.push((h, o, n)));
        assert_eq!(
            pieces,
            vec![(x - 0x10, 0, 0x10), (pool0 + 0x3000, 0x10, 0x1000), (x + 0x1000, 0x1010, 0x1010)]
        );
        // A 64 KiB leaf over the page splits into 16 small leaves, one of them on the pool;
        // a leaf elsewhere and a sysmem leaf stay whole.
        let big = (0x20_0000u64, Target::Fb(x), 0x1_0000u64, 6u8);
        let s = t.split_leaf(big);
        assert_eq!(s.len(), 16);
        assert_eq!(s[0], (0x20_0000, Target::Fb(pool0 + 0x3000), PAGE, 6));
        assert_eq!(s[1], (0x20_1000, Target::Fb(x + 0x1000), PAGE, 6));
        let other = (0x40_0000u64, Target::Fb(x + 0x1_0000), 0x1_0000u64, 6u8);
        assert_eq!(t.split_leaf(other), vec![other]);
        let sys = (0x40_0000u64, Target::Sys(x), 0x1000u64, 0u8);
        assert_eq!(t.split_leaf(sys), vec![sys]);
        assert_eq!(t.release(0x3000), Some(x));
        assert_eq!(t.release(0x3000), None);
        assert_eq!(t.translate(x), x);
        assert!(t.is_empty());
    }

    #[test]
    fn size_and_layout_bounds() {
        let plain = FbLayout::reserved(32 * MIB, 256 * MIB, BASE, END).unwrap();
        assert_eq!(plain.usable(), Some((BASE, END - 2 * MIB)));
        let l = layout();
        // The guest is told a usable FB that ends below the pool; the pool sits right
        // below the carve-out, whose BAR roots stay where they were.
        assert_eq!(l.usable(), Some((BASE, END - 6 * MIB)));
        assert_eq!(l.pool_range(), Some((END - 6 * MIB, END - 2 * MIB)));
        assert_eq!(l.bar_roots(), plain.bar_roots());
        assert_eq!(plain.pool_range(), None);
        assert!(plain.with_pool(3 * MIB).is_err());
        assert!(plain.with_pool(62 * MIB).is_err());
        assert!(FbLayout::plain(64 * MIB).with_pool(2 * MIB).is_err());
        let bar1 = 256 * MIB;
        let big = FbLayout::reserved(2048 * MIB, 8192 * MIB, 0x1_6900_0000, 0x1_E900_0000).unwrap();
        assert!(check_size(&big, 64 * MIB, bar1, 64 * MIB).is_ok());
        assert!(check_size(&big, 128 * MIB, bar1, 64 * MIB).is_ok());
        assert!(check_size(&big, 128 * MIB, bar1, 128 * MIB).is_err()); // host BAR1
        assert!(check_size(&big, 130 * MIB, bar1, 0).is_err()); // above the maximum
        assert!(check_size(&big, 3 * MIB, bar1, 0).is_err()); // not in 2 MiB units
        assert!(check_size(&big, 64 * MIB, 64 * MIB, 0).is_err()); // half the guest BAR1
        assert!(check_size(&plain, 16 * MIB, bar1, 0).is_err()); // a quarter of the FB
        assert!(check_size(&l, 2 * MIB, bar1, 0).is_err()); // a pool already
        assert!(check_size(&FbLayout::plain(64 * MIB), 2 * MIB, bar1, 0).is_err());
    }

    #[test]
    fn a_mapped_page_moves_into_the_pool_for_every_path_and_back_on_unmap() {
        let (mut fb, pt, pool0) = setup();
        let x = BASE + 0x10_0000;
        fb.write(x, &[1, 2, 3, 4]);
        assert_eq!(fb.watched_dword(x), Some(0x0403_0201));
        // Nothing mapped yet: the first poll walks every unit and moves nothing.
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(stats(&fb).units_walked, 2);
        assert!(!poll(&mut fb, &mut NoGpu));
        // The guest maps area page 0x5000 to X (BAR1 PTE through BAR2), then invalidates.
        fb.write_u64(pt + 8 * 5, pte(x));
        assert!(poll(&mut fb, &mut NoGpu));
        let st = stats(&fb);
        assert_eq!((st.installs_cold, st.bytes_in, st.units_walked), (1, PAGE, 3));
        let p = pool0 + 0x5000;
        assert_eq!(fb.remap(x + 4), p + 4);
        // The copy-in: the pool page holds X's bytes.
        let mut b = [0u8; 4];
        fb.read(p, &mut b);
        assert_eq!(b, [1, 2, 3, 4]);
        // The guest CPU writes through the area (straight to the pool page): every device
        // read of X sees it, the watched copy included (no stale copy is kept).
        fb.write(p, &[9, 9, 9, 9]);
        fb.read(x, &mut b);
        assert_eq!(b, [9, 9, 9, 9]);
        assert_eq!(fb.watched_dword(x), Some(0x0909_0909));
        // A device write of X (BAR2, PRAMIN, the software executor) lands in the pool page.
        fb.write(x + 8, &[5; 4]);
        fb.read(p + 8, &mut b);
        assert_eq!(b, [5; 4]);
        // The BAR aperture TLB holds guest targets; the remap applies after it: a BAR2
        // mapping of X reaches the pool page.
        let mem = GuestMemory::new();
        let l = layout();
        let (bar2, _) = l.bar_roots().unwrap();
        let (pd2, pd1, pd0, pt2) = (BASE + 0x6000, BASE + 0x7000, BASE + 0x8000, BASE + 0x9000);
        fb.write_u64(bar2, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        fb.write_u64(pd0 + 8, vid(pt2));
        fb.write_u64(pt2 + 8 * 3, pte(x));
        let mut d = 0xAABB_CCDDu32.to_le_bytes();
        assert_eq!(fb.bar_access(&mem, bar2, 0x3010, &mut d, true), Target::Fb(x + 0x10));
        fb.read(p + 0x10, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0xAABB_CCDD);
        // Shadow leaves on X point at the pool page.
        let leaves = fb.remap_leaves(vec![
            (0x7000_0000, Target::Fb(x), PAGE, 6),
            (0x7010_0000, Target::Fb(x + PAGE), PAGE, 6),
        ]);
        assert_eq!(leaves[0].1, Target::Fb(p));
        assert_eq!(leaves[1].1, Target::Fb(x + PAGE));
        // The guest unmaps the page: the pool page is copied back to X, the remap goes.
        fb.write_u64(pt + 8 * 5, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        let st = stats(&fb);
        assert_eq!((st.releases_cold, st.bytes_out), (1, PAGE));
        assert_eq!(fb.remap(x), x);
        fb.read(x, &mut b);
        assert_eq!(b, [9, 9, 9, 9]);
        fb.read(x + 0x10, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0xAABB_CCDD);
        assert_eq!(fb.watched_dword(x), Some(0x0909_0909));
        // Mapped again at another area page: the page moves to that one.
        fb.write_u64(pt + 8 * 9, pte(x));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(fb.remap(x), pool0 + 0x9000);
        // Moved from 0x9000 to 0xA000 in one change: released and installed in one poll.
        fb.write_u64(pt + 8 * 9, 0);
        fb.write_u64(pt + 8 * 10, pte(x));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(fb.remap(x), pool0 + 0xA000);
        let st = stats(&fb);
        assert_eq!((st.installs_cold, st.releases_cold), (3, 2));
        assert!(fb.bar1_pool_summary().unwrap().contains("1 page(s) in the pool now"));
        // A function-level reset: the pool and its counters carry over, the remaps go.
        let fresh = FakeFb::for_layout(&layout());
        fb.carry_bar1_passthrough(&fresh);
        assert_eq!(fresh.bar1_pool(), Some((pool0, POOL)));
        assert_eq!(fresh.remap(x), x);
        assert_eq!(stats(&fresh).installs_cold, 3);
        assert!(fb.bar1_pool().is_none());
    }

    #[test]
    fn big_pages_move_page_by_page_and_refusals_fail_closed() {
        let (mut fb, pt, pool0) = setup();
        let l = layout();
        let (_, root) = l.bar_roots().unwrap();
        let pd0 = BASE + 0x3000;
        let big = BASE + 0xA000;
        fb.write_u64(pd0, ((big >> 8) << 4) | 0b010);
        let x = BASE + 0x20_0000;
        // A 64 KiB page at area 0x10000: 16 pages move.
        fb.write_u64(big + 8, pte(x));
        // Aliasing: area 0x2000 maps a page the big page maps too.
        fb.write_u64(pt + 8 * 2, pte(x + 0x3000));
        // Not usable: the pool itself, the carve-out (the BAR1 root); and guest RAM.
        fb.write_u64(pt + 8 * 3, pte(pool0));
        fb.write_u64(pt + 8 * 4, pte(root));
        fb.write_u64(pt + 8 * 6, ((0x8000_0000u64 >> 12) << 8) | 0b101);
        assert!(poll(&mut fb, &mut NoGpu));
        let st = stats(&fb);
        assert_eq!(st.installs_cold, 16, "{st:?}");
        assert_eq!((st.refused, st.sysmem), (3, 1));
        // Area pages are served in ascending order: the page mapped twice went to 0x2000,
        // the big page's 0x13000 is refused.
        assert_eq!(fb.remap(x + 0x3000), pool0 + 0x2000);
        assert_eq!(fb.remap(x + 0x4000), pool0 + 0x1_4000);
        assert_eq!(fb.remap(root), root);
        assert_eq!(fb.remap(pool0), pool0);
        assert_eq!(pending(&fb), 3);
        // The refusals are logged once; the retry changes nothing and logs nothing new.
        expire_retry(&fb);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(stats(&fb).refused, 3);
        assert_eq!(fb.remap(x + 0x3000), pool0 + 0x2000);
        // Once the guest drops the small mapping, the big page's area page takes it.
        fb.write_u64(pt + 8 * 2, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(fb.remap(x + 0x3000), pool0 + 0x1_3000);
        assert_eq!(pending(&fb), 2);
        // The big page goes: all 16 pages move back.
        fb.write_u64(big + 8, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(fb.remap(x), x);
        assert_eq!(stats(&fb).releases_cold, 17);
    }

    #[test]
    fn a_page_the_gpu_maps_moves_only_behind_the_barrier() {
        let (mut fb, pt, pool0) = setup();
        let x = BASE + 0x30_0000;
        fb.write(x, &[7; 8]);
        assert!(poll(&mut fb, &mut NoGpu));
        let mut gpu = FakeGpu::new(0x1234_0000);
        gpu.used.insert(x);
        // The barrier fails (a busy channel): refused, the page stays where it is.
        gpu.ok = false;
        fb.write_u64(pt + 8, pte(x));
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(x), x);
        let st = stats(&fb);
        assert_eq!((st.barriers_refused, st.refused, st.installs_warm), (1, 1, 0));
        assert_eq!(pending(&fb), 1);
        assert!(!fb.pool_rediff_pending(0x1234_0000));
        // No retry before RETRY_EVERY; then the barrier holds and the page moves: copied,
        // the shadow of the root told to re-diff and re-synced.
        fb.pool_mut().as_mut().unwrap().last_retry = Some(Instant::now());
        assert!(!poll(&mut fb, &mut gpu));
        gpu.ok = true;
        expire_retry(&fb);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(x), pool0 + 0x1000);
        let st = stats(&fb);
        assert_eq!((st.installs_warm, st.barriers, st.preempts), (1, 1, 1));
        assert_eq!(gpu.resynced, vec![0x1234_0000]);
        assert_eq!(pending(&fb), 0);
        let mut b = [0u8; 8];
        fb.read(pool0 + 0x1000, &mut b);
        assert_eq!(b, [7; 8]);
        // The page becomes a mirrored channel's USERD (its host channel holds the pool
        // address): the unmap cannot move it back; it stays consistent in the pool.
        gpu.userd.insert(x);
        fb.write_u64(pt + 8, 0);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(x), pool0 + 0x1000);
        // Meanwhile the guest maps another page at the same area page: it must wait.
        let y = BASE + 0x31_0000;
        fb.write_u64(pt + 8, pte(y));
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(y), y);
        assert_eq!(pending(&fb), 1);
        // The channel is freed: the release and the waiting install go through.
        gpu.userd.clear();
        expire_retry(&fb);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(x), x);
        assert_eq!(fb.remap(y), pool0 + 0x1000);
        let st = stats(&fb);
        assert_eq!((st.releases_warm, st.installs_cold), (1, 1));
        fb.read(x, &mut b);
        assert_eq!(b, [7; 8]);
        // A host channel that walks the guest's own tables blocks every change.
        gpu.blocked = true;
        fb.write_u64(pt + 8 * 2, pte(x));
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(x), x);
        assert_eq!(pending(&fb), 1);
    }

    /// The morning matrix's refused barriers (docs/caraxes/bar1-pool.md §9): a page in the
    /// pool that the GPU maps, whose move back meets a busy channel again and again, never
    /// moves while the GPU maps it. The remap stays (the GPU and every device path keep
    /// finding it at its pool page), no shadow is told to re-diff (no region object or PDE
    /// of the root changes), and the guest's next page for that area page waits. Only once
    /// no shadow maps the page any more does it move back, cold, with its bytes.
    #[test]
    fn a_page_whose_barrier_is_refused_never_moves_while_the_gpu_maps_it() {
        let (mut fb, pt, pool0) = setup();
        let x = BASE + 0x30_0000;
        fb.write_u64(pt + 8, pte(x));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(fb.remap(x), pool0 + 0x1000);
        // The GPU writes the page through the shadow, at its pool page.
        fb.write(x, &[9; 8]);
        let root = 0x1234_0000;
        let mut gpu = FakeGpu::new(root);
        gpu.used.insert(x);
        gpu.ok = false;
        // The guest unmaps the area page, then maps another page there.
        fb.write_u64(pt + 8, 0);
        for i in 0..14 {
            expire_retry(&fb);
            assert!(poll(&mut fb, &mut gpu));
            assert_eq!(fb.remap(x), pool0 + 0x1000, "moved at refusal {i}");
            assert!(!fb.pool_rediff_pending(root));
        }
        let y = BASE + 0x31_0000;
        fb.write_u64(pt + 8, pte(y));
        expire_retry(&fb);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!((fb.remap(x), fb.remap(y)), (pool0 + 0x1000, y));
        let st = stats(&fb);
        assert_eq!((st.barriers_refused, st.releases_cold, st.releases_warm), (15, 0, 0));
        assert_eq!(pending(&fb), 1, "the install at the busy area page waits");
        assert!(gpu.resynced.is_empty());
        let mut b = [0u8; 8];
        fb.read(x, &mut b);
        assert_eq!(b, [9; 8]);
        // The guest drops the GPU mapping (the shadow no longer maps the page): cold now.
        gpu.used.clear();
        expire_retry(&fb);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!((fb.remap(x), fb.remap(y)), (x, pool0 + 0x1000));
        let st = stats(&fb);
        assert_eq!((st.releases_cold, st.installs_cold, st.barriers), (1, 2, 0));
        fb.read(x, &mut b);
        assert_eq!(b, [9; 8]);
        assert!(gpu.resynced.is_empty());
    }

    #[test]
    fn only_units_whose_tables_changed_are_walked_again() {
        let (mut fb, pt, _) = setup();
        assert!(poll(&mut fb, &mut NoGpu));
        let w0 = stats(&fb).units_walked;
        // A data write does not re-arm the walk.
        fb.write(BASE + 0x40_0000, &[1]);
        assert!(!poll(&mut fb, &mut NoGpu));
        // A write to unit 0's small table re-walks unit 0 only.
        fb.write_u64(pt + 8 * 7, pte(BASE + 0x40_0000));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(stats(&fb).units_walked, w0 + 1);
    }

    /// The BAR2 memfd beside the pool (docs/caraxes/bar2-memfd.md §8): `setup()` plus guest
    /// BAR2 tables in the carve-out whose small table maps BAR2 page 1 to the BAR1 small
    /// table `pt`, page 2 to `x` and page 3 to itself; then the memfd on and synced. Returns
    /// the FB, `pt`, the pool base, the BAR2 root and the guest's view of the memfd.
    fn setup_with_memfd(x: u64) -> (FakeFb, u64, u64, u64, crate::bar2area::SharedView) {
        use crate::bar2area::{Bar2Area, Config};
        let (mut fb, pt, pool0) = setup();
        let (bar2, _) = layout().bar_roots().unwrap();
        let (pd2, pd1, pd0, pt2) = (BASE + 0x6000, BASE + 0x7000, BASE + 0x8000, BASE + 0x9000);
        fb.write_u64(bar2, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        fb.write_u64(pd0 + 8, vid(pt2));
        fb.write_u64(pt2 + 8, pte(pt));
        fb.write_u64(pt2 + 8 * 2, pte(x));
        fb.write_u64(pt2 + 8 * 3, pte(pt2));
        let cfg = Config {
            traps: vec![0x1f000],
            sync_doorbell: false,
            flush_defer_us: None,
        };
        let area = Bar2Area::create(0x20000, &cfg).unwrap();
        let guest = area.mem.map_again().unwrap();
        fb.set_bar2_area(area).unwrap();
        assert!(fb.bar2_sync(Some(bar2), None));
        assert!(matches!(fb.backing(pt), crate::fb::Backing::Bar2Memfd(0x1000)));
        assert!(matches!(fb.backing(x), crate::fb::Backing::Bar2Memfd(0x2000)));
        // The first poll walks every unit of the area (nothing mapped yet).
        assert!(poll(&mut fb, &mut NoGpu));
        (fb, pt, pool0, bar2, guest)
    }

    /// The census's hang (optimization-backlog.md §3.1): with the memfd, the guest RM
    /// writes the BAR1 PTEs of the area through BAR2 into the memfd. The sync finds them,
    /// and the pool must see them as it sees a trapped write, or it never moves the page:
    /// the guest's CPU writes then reach only the pool page while the GPU reads the guest
    /// page, and the shadow maps the 2 MiB leaf over `x` whole (1 leaf) instead of split
    /// around its pool page (512 leaves: the 511 the census's first walk missed).
    #[test]
    fn with_the_bar2_memfd_the_pool_sees_bar1_ptes_written_through_bar2() {
        let x = BASE + 0x10_0000;
        let (mut fb, _pt, pool0, bar2, guest) = setup_with_memfd(x);
        // A context's GMMU tree with one 2 MiB leaf over [BASE, BASE + 2 MiB), x inside.
        let (croot, cpd2, cpd1, cpd0) = (BASE + 0xA000, BASE + 0xB000, BASE + 0xC000, BASE + 0xD000);
        fb.write_u64(croot, vid(cpd2));
        fb.write_u64(cpd2, vid(cpd1));
        fb.write_u64(cpd1, vid(cpd0));
        fb.write_u64(cpd0, pte(BASE) | (0x06 << 56));
        let root = Target::Fb(croot);
        fb.walk_update(None, root, false, 1 << 20);
        let shadow_sees = |fb: &FakeFb| fb.remap_leaves(fb.with_walk(root, |c| c.unit_leaves(0).to_vec()).unwrap()).len();
        assert_eq!(shadow_sees(&fb), 1);
        // The guest maps area page 5 to x through BAR2 (the memfd page of the BAR1 table),
        // then flushes: the sync writes it back, and the next trapped BAR0 write polls.
        guest.store_word(1, 5, pte(x));
        fb.bar2_sync(Some(bar2), None);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(stats(&fb).installs_cold, 1);
        assert_eq!(fb.remap(x), pool0 + 0x5000);
        assert_eq!(shadow_sees(&fb), 512);
        // Unmapped the same way: the page moves back.
        guest.store_word(1, 5, 0);
        fb.bar2_sync(Some(bar2), None);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(stats(&fb).releases_cold, 1);
        assert_eq!(fb.remap(x), x);
        assert_eq!(shadow_sees(&fb), 1);
    }

    /// A page both in the pool and behind BAR2: the memfd holds the guest's newest bytes
    /// of the guest page `x` (its key is the guest FB address, the one the guest's BAR2
    /// tables name), the store holds them at the pool page. Reads look in the memfd by the
    /// guest address first; the sync writes back to where the page lives now; a device
    /// write reaches the guest's BAR2 view by the guest address and VRAM at the pool page.
    #[test]
    fn a_page_in_the_pool_and_behind_bar2_reads_and_writes_through_one_order() {
        let x = BASE + 0x10_0000;
        let (mut fb, pt, pool0, bar2, guest) = setup_with_memfd(x);
        fb.write_u64(x, 0x1111);
        assert_eq!(guest.load_word(2, 0), 0x1111);
        // A trapped write of the BAR1 PTE (not through the memfd): the pool takes x.
        fb.write_u64(pt + 8 * 5, pte(x));
        assert!(poll(&mut fb, &mut NoGpu));
        let p = pool0 + 0x5000;
        assert_eq!(fb.remap(x), p);
        assert_eq!(fb.read_u64(p), Some(0x1111));
        // The guest writes x through BAR2: before the sync, device reads of x see the
        // memfd; after it, the pool page (what the GPU reads) has it too.
        guest.store_word(2, 1, 0x2222);
        assert_eq!(fb.read_u64(x + 8), Some(0x2222));
        fb.bar2_sync(Some(bar2), None);
        assert_eq!(fb.read_u64(p + 8), Some(0x2222));
        // A device write of x reaches the guest's BAR2 view and the pool page.
        fb.write_u64(x + 16, 0x3333);
        assert_eq!(guest.load_word(2, 2), 0x3333);
        assert_eq!(fb.read_u64(p + 16), Some(0x3333));
        // Back out of the pool: x in the store has everything.
        fb.write_u64(pt + 8 * 5, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(fb.remap(x), x);
        guest.store_word(2, 3, 0x4444);
        fb.bar2_sync(Some(bar2), None);
        // BAR2 re-pointed away from x: x leaves the memfd and the store is read.
        guest.store_word(3, 2, 0);
        fb.bar2_sync(Some(bar2), None);
        assert_eq!(fb.backing(x), crate::fb::Backing::Store);
        for (i, v) in [0x1111u64, 0x2222, 0x3333, 0x4444].into_iter().enumerate() {
            assert_eq!(fb.read_u64(x + 8 * i as u64), Some(v));
        }
    }
}

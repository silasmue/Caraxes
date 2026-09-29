// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
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
//!
//! The poll runs under the device lock after every trapped BAR0 write, so it is
//! incremental (docs/caraxes/bar1-pool.md §11): it walks only the units whose tables were
//! written, decides only the area pages whose mapping changed (plus the paced retries and
//! the replicas), learns of pooled pages that became table pages from the walks' new
//! pages, refuses a USERD page before it waits for the shadow worker, and merges the
//! replicas only after a flush point ([`poll_at`]).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use crate::fb::{FakeFb, FbLayout, Target};
use crate::guestmem::GuestMemory;

pub const PAGE: u64 = 4096;
const MIB: u64 = 1 << 20;
/// The area is walked and tracked in 2 MiB units (one PD0 entry each).
const UNIT: u64 = 2 * MIB;

/// The pool with host execution when `CARAXES_BAR1_POOL_MIB` is unset: what every production
/// launcher ran until 2026-09-30 (Linux B 615, nouveau+NVK, the Windows kit's
/// `WIN_BAR1_POOL_MIB=32`).
pub const DEFAULT_POOL_MIB: u64 = 32;
/// The knob: the pool in MiB (0: no pool).
pub const POOL_MIB_ENV: &str = "CARAXES_BAR1_POOL_MIB";
/// The largest pool: the dma-buf maps all of it into the host's shared BAR1 (256 MiB on
/// this card, docs/caraxes/parallel.md §2).
pub const MAX_POOL_MIB: u64 = 128;
/// Host BAR1 one device may hold with a pool: the pool plus its window budget
/// (`fb::bar1_budget`, 2 x 16 MiB).
pub const MAX_HOST_BAR1: u64 = 192 * MIB;
/// Host BAR1 a new pool must leave free for the host's own clients (the desktop's Vulkan
/// fails at vkCreateDevice without it, 2026-09-25: a game VM's 128 MiB pool plus a second
/// device's pool exhausted the 256 MiB); `CARAXES_HOST_BAR1_FLOOR_MIB` overrides it, 0
/// turns the check off.
pub const HOST_BAR1_FLOOR_MIB: u64 = 96;
pub const HOST_BAR1_FLOOR_ENV: &str = "CARAXES_HOST_BAR1_FLOOR_MIB";
/// Refused remaps are retried at most this often (at the next sync point after).
pub const RETRY_EVERY: Duration = Duration::from_millis(1);
/// The retry interval doubles after every retry that moved nothing, up to this bound, and
/// drops back to [`RETRY_EVERY`] at the first retry that moves a page. A retry runs under
/// the device lock and may wait for the shadow worker (`PoolGpu::refs`) and read every
/// mirrored channel's USERD (`PoolGpu::quiesce`): at 1 ms it ran up to 1000 times a second
/// against a channel that stays busy (docs/caraxes/sottr-stage.md §11).
pub const RETRY_MAX: Duration = Duration::from_millis(64);

/// How long a page the guest unmapped from the area stays in the pool, retired (lazy move
/// back, docs/caraxes/bar1-pool.md §13): mapped again at the same area page within this time
/// it costs nothing, as a BAR1 remap costs no copy on the real card; older, or when another
/// page wants its area page, it moves home (one copy, as every unmap did before).
pub const RETIRE_AGE: Duration = Duration::from_secs(2);
/// Retired pages are checked for [`RETIRE_AGE`] at most this often.
pub const RETIRE_SCAN: Duration = Duration::from_millis(100);
/// The churn line (the trace, once per second with activity; `Bar1Pool::take_churn`).
pub const CHURN_EVERY: Duration = Duration::from_secs(1);
/// Findings logged at most (the counters are complete).
const LOG_FIRST: usize = 32;

/// The pool: with host execution [`DEFAULT_POOL_MIB`] unless [`POOL_MIB_ENV`] sets another
/// size (an even number of MiB, 2..=128) or 0 (no pool); without host execution none.
/// `Ok(Some(bytes))` when on; an invalid size is an error, never a silent default.
pub fn config(host_exec: bool) -> Result<Option<u64>, String> {
    parse_pool(std::env::var(POOL_MIB_ENV).ok().as_deref(), host_exec)
}

/// [`config`] on a given value (`None`: unset).
pub fn parse_pool(v: Option<&str>, host_exec: bool) -> Result<Option<u64>, String> {
    let mib = match v.map(str::trim).filter(|v| !v.is_empty()) {
        None if host_exec => DEFAULT_POOL_MIB,
        None | Some("0") => return Ok(None),
        Some(v) => v
            .parse::<u64>()
            .map_err(|_| format!("{POOL_MIB_ENV}={v}: not a number of MiB"))?,
    };
    if !host_exec {
        return Err(format!(
            "{POOL_MIB_ENV}={mib}: the BAR1 pool needs host execution (--host-exec)"
        ));
    }
    if !(2..=MAX_POOL_MIB).contains(&mib) || mib % 2 != 0 {
        return Err(format!(
            "{POOL_MIB_ENV}={mib}: the pool must be 0 (none) or an even number of MiB from 2 to {MAX_POOL_MIB}"
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

/// `CARAXES_HOST_BAR1_FLOOR_MIB` (unset: [`HOST_BAR1_FLOOR_MIB`]), in bytes.
pub fn host_bar1_floor(v: Option<&str>) -> Result<u64, String> {
    match v.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(HOST_BAR1_FLOOR_MIB * MIB),
        Some(v) => v
            .parse::<u64>()
            .map(|m| m * MIB)
            .map_err(|_| format!("{HOST_BAR1_FLOOR_ENV}={v:?}: a number of MiB")),
    }
}

/// `(total, used, free)` in bytes from the "BAR1 Memory Usage" block of
/// `nvidia-smi -q -d MEMORY` (the first GPU's).
pub fn parse_smi_bar1(out: &str) -> Option<(u64, u64, u64)> {
    let mut lines = out.lines().skip_while(|l| l.trim() != "BAR1 Memory Usage").skip(1);
    let mut field = |name: &str| -> Option<u64> {
        let l = lines.next()?;
        let (k, v) = l.split_once(':')?;
        if k.trim() != name {
            return None;
        }
        let v = v.trim().strip_suffix("MiB")?.trim().parse::<u64>().ok()?;
        Some(v * MIB)
    };
    Some((field("Total")?, field("Used")?, field("Free")?))
}

/// The host's free BAR1, read with `nvidia-smi -q -d MEMORY` (read-only; the host
/// driver's own accounting of its BAR1 heap).
pub fn host_bar1_free() -> Result<u64, String> {
    let out = std::process::Command::new("nvidia-smi")
        .args(["-q", "-d", "MEMORY"])
        .output()
        .map_err(|e| format!("nvidia-smi: {e}"))?;
    if !out.status.success() {
        return Err(format!("nvidia-smi -q -d MEMORY: {}", out.status));
    }
    parse_smi_bar1(&String::from_utf8_lossy(&out.stdout))
        .map(|(_, _, free)| free)
        .ok_or_else(|| "nvidia-smi -q -d MEMORY: no BAR1 Memory Usage block".into())
}

/// The host BAR1 floor (D11's rule for VRAM, applied to BAR1): a pool of `need` bytes
/// (the pool and its window budget) is refused unless the host keeps `floor` free after
/// it. Fail closed: an unknown free size (`Err`) refuses too; a floor of 0 skips the check.
pub fn check_host_bar1(need: u64, free: Result<u64, String>, floor: u64) -> Result<(), String> {
    if floor == 0 {
        return Ok(());
    }
    let free = free.map_err(|e| {
        format!("host BAR1 floor: the host's free BAR1 is unknown ({e}); refused ({HOST_BAR1_FLOOR_ENV}=0 skips the check)")
    })?;
    if free < need + floor {
        return Err(format!(
            "host BAR1 floor: the pool and its windows need {} MiB, the host has {} MiB free and must keep {} MiB ({HOST_BAR1_FLOOR_ENV})",
            need / MIB,
            free / MIB,
            floor / MIB
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
    /// The FB page is a guest page-table page the device tracks the writes of (a shadow's
    /// cached walk or the pool's own BAR1 walk reads it): guest writes through the area
    /// would bypass that tracking.
    TablePage,
}

impl Refusal {
    pub fn why(self) -> &'static str {
        match self {
            Refusal::BadSlot => "the area offset is not a page of the area",
            Refusal::SlotBusy => "the area page still holds another page whose move back was refused",
            Refusal::Aliased => "the page lives in the pool already through another area page (the guest maps it twice)",
            Refusal::NotUsable => "the page is not in the guest's usable FB (pool, carve-out or outside the reservation)",
            Refusal::TablePage => "the page is a guest page-table page the device tracks (a shadow's walk or the pool's BAR1 walk reads it)",
        }
    }

    /// Whether the area page serves the refused page as a replica (a copy the device
    /// merges both ways at every sync point, [`sync_untracked`]) instead of leaving the
    /// guest's accesses on a pool page nobody reads.
    pub fn replicates(self) -> bool {
        matches!(self, Refusal::Aliased | Refusal::TablePage)
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

    /// [`Self::check`] against the table as it will be once the area pages `released`
    /// gave up their pages (the releases of the same poll run first), without a copy of
    /// the table: the poll's static check before it moves anything.
    pub fn check_after(
        &self,
        g: u64,
        x: u64,
        usable: (u64, u64),
        released: &BTreeSet<u64>,
    ) -> Result<(), Refusal> {
        if g % PAGE != 0 || g >= self.len {
            return Err(Refusal::BadSlot);
        }
        if x % PAGE != 0 || x < usable.0 || x.saturating_add(PAGE) > usable.1 {
            return Err(Refusal::NotUsable);
        }
        if self.slots.contains_key(&g) && !released.contains(&g) {
            return Err(Refusal::SlotBusy);
        }
        if self.fwd.get(&x).is_some_and(|h| !released.contains(h)) {
            return Err(Refusal::Aliased);
        }
        Ok(())
    }

    /// The area page that holds FB page `x` now, if any.
    pub fn slot_of(&self, x: u64) -> Option<u64> {
        self.fwd.get(&(x & !(PAGE - 1))).copied()
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
    /// Polls that retried refused remaps, and those after which the interval grew.
    pub retries: u64,
    pub retry_backoffs: u64,
    /// Distinct guest page-table pages (a shadow's walk or the pool's BAR1 walk reads
    /// them) the guest mapped into the area.
    pub table_hazards: u64,
    /// Of those, pages that were in the pool and were moved back out to the trapped path
    /// (evicted), and installs refused because the page was a table page already.
    pub table_evictions: u64,
    pub table_refusals: u64,
    /// Times a pooled table page was marked changed at a sync point while its eviction
    /// was still due (the shadow re-reads it from its pool page).
    pub table_resyncs: u64,
    /// Replicas made (area pages that mirror a refused page, [`sync_untracked`]), sync
    /// points that merged them, pages whose guest area writes reached the page's home
    /// (tracked writes), pages whose home changes were copied to the area, and 8-byte
    /// words changed on both sides between two syncs (the area's value wins).
    pub replicas: u64,
    pub replica_syncs: u64,
    pub replica_to_home: u64,
    pub replica_to_area: u64,
    pub replica_conflicts: u64,
    /// Replica merges that read the home page (the others found it unchanged: no
    /// device-visible write since the last merge, docs/caraxes/bar1-pool.md §12).
    pub replica_home_reads: u64,
    /// Pages moved into the pool while guest BAR2 maps them too (the BAR2 memfd holds
    /// them by guest address): the device's reads and writes and the memfd's write-back
    /// agree (bar2-memfd.md §8), but a guest CPU write through the area reaches the guest's
    /// BAR2 view of the page only after a device write of the same bytes.
    pub bar2_aliased: u64,
    /// Units whose directory page was written but whose path (directory pages and PD0
    /// entry) was unchanged: their page tables were not read again.
    pub units_path_only: u64,
    /// Polls that ran without the replica merge (the BAR0 write was no flush point; the
    /// handler of the invalidate and of the GSP queue doorbell merges itself).
    pub polls_unmerged: u64,
    /// The lazy move back (§13): pages the guest unmapped that stayed in the pool (no copy),
    /// retired pages mapped again at the same area page (no copy), retired pages the guest
    /// mapped at another area page (home first, then in), retired pages that went home
    /// because another page wanted their area page, and because they aged out.
    pub retires: u64,
    pub relives: u64,
    pub relocations: u64,
    pub slot_evictions: u64,
    pub aged_out: u64,
    /// FB store window maps the moves needed (a page's home outside the store's windows).
    pub move_window_maps: u64,
    /// Device-CE transfers kept on the CPU because a pooled page lies in them (fb.rs
    /// `plain_vram`), and of those the ones with a retired page.
    pub devce_blocked: std::cell::Cell<u64>,
    pub devce_blocked_retired: std::cell::Cell<u64>,
    /// §14: batches that took more than one hold of the device lock, holds that left a
    /// batch for the next poll, and the longest a batch was pending; waits for a shadow job
    /// in flight that may still map a page to move, those left to the next poll, and their
    /// time; the time of the GPU check, the barriers, the moves and the re-syncs in all.
    pub sliced_batches: u64,
    pub slices_parked: u64,
    pub batch_max_us: u128,
    pub in_flight_waits: u64,
    pub in_flight_parked: u64,
    pub in_flight_wait_us: u128,
    /// Holds that left a barrier's preempts or re-sync to the worker and the next poll (§15).
    pub barrier_parked: u64,
    /// Polls after a trapped BAR0 write skipped: nothing due, and the write no merge point
    /// (§16).
    pub polls_skipped: u64,
    /// Doorbells that merged the replicas the rung channels see home (§16), the replica
    /// merges they did, and those that carried area writes home.
    pub ring_merges: u64,
    pub ring_replicas: u64,
    pub ring_to_home: u64,
    pub refs_us: u128,
    pub barrier_us: u128,
    pub move_us: u128,
    pub resync_us: u128,
    pub first: Vec<String>,
}

/// Where one poll spent its time (the slow-poll trace event prints it, device.rs
/// `after_bar0_write`, docs/caraxes/bar1-pool.md §11).
#[derive(Default, Debug, Clone)]
pub struct PollBreakdown {
    /// The poll merged the replicas (a flush point) or only marked the pooled table pages.
    pub merge: bool,
    pub total_us: u128,
    /// `sync_untracked`: the replica merges (two page reads each) and the marks.
    pub sync_us: u128,
    pub replicas_merged: usize,
    pub tables_marked: usize,
    /// Unit walks: full ones, and directory-only checks that found the path unchanged.
    pub walk_us: u128,
    pub units_walked: usize,
    pub units_path_only: usize,
    /// Table pages the walk had no copy of and read from the FB.
    pub pages_read: usize,
    /// Replica merges that read the home page.
    pub homes_read: u64,
    /// The decision: area pages whose wanted mapping changed, plus the paced retries and
    /// the replicas.
    pub decide_us: u128,
    pub candidates: usize,
    pub ops: usize,
    /// The USERD check (no worker wait) and the GPU references (shadow worker wait and
    /// the leaf scan), for the pages left after the USERD check.
    pub userd_us: u128,
    pub refs_us: u128,
    pub refs_pages: usize,
    pub barrier_us: u128,
    pub move_us: u128,
    pub moved: usize,
    pub resync_us: u128,
    pub retry: bool,
    pub hazard: bool,
    /// The lazy move back (§13): area pages retired and re-lived in this poll, and the FB
    /// store window maps its moves needed.
    pub retired: usize,
    pub relived: usize,
    pub window_maps: u64,
    /// §14: the time spent waiting for a shadow job in flight that may still map a page to
    /// move, and the holds of the device lock the batch took so far.
    pub in_flight_us: u128,
    pub slices: u32,
}

impl std::fmt::Display for PollBreakdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "sync {} us ({} replica(s) merged, {} home(s) read{}, {} pooled table page(s) marked); walk {} us ({} unit(s) walked, {} path-only, {} table page(s) read); decide {} us ({} candidate(s), {} op(s){}{}); userd {} us; refs {} us ({} page(s)); barrier {} us; moves {} us ({} page(s), {} window map(s)); resync {} us; in-flight wait {} us; {} retired, {} re-lived; slice {} of the batch; total {} us",
            self.sync_us,
            self.replicas_merged,
            self.homes_read,
            if self.merge { "" } else { ", merge skipped: no flush point" },
            self.tables_marked,
            self.walk_us,
            self.units_walked,
            self.units_path_only,
            self.pages_read,
            self.decide_us,
            self.candidates,
            self.ops,
            if self.retry { ", retry" } else { "" },
            if self.hazard { ", table-page eviction" } else { "" },
            self.userd_us,
            self.refs_us,
            self.refs_pages,
            self.barrier_us,
            self.move_us,
            self.moved,
            self.window_maps,
            self.resync_us,
            self.in_flight_us,
            self.retired,
            self.relived,
            self.slices.max(1),
            self.total_us
        )
    }
}

/// One 2 MiB unit of the area as last walked.
#[derive(Default, Clone)]
struct UnitState {
    /// Every table page the walk read: `dirs` then `leaf_tables`.
    tables: Vec<u64>,
    dirs: Vec<u64>,
    leaf_tables: Vec<u64>,
    pde: Option<(u64, u64)>,
    mapped: Vec<(u64, Target)>,
}

/// An area page that serves a guest FB page the pool refused to move there (aliased, or
/// a table page): the pool page holds a copy of the page, and every sync point merges the
/// two (guest writes through the area go to the page's home as tracked writes, device
/// and other-mapping changes of the home go to the area).
struct Replica {
    x: u64,
    /// The page as both sides held it after the last merge.
    snap: Vec<u8>,
    /// The home changed through a device-visible write since the last merge
    /// (`FakeFb::write`, a BAR2 memfd sync, a raw copy): the merge reads it.
    home_dirty: bool,
    /// Merges since the home was last read (the sweep reads it every [`HOME_SWEEP`]).
    quiet: u32,
    /// For the exit report (§16): merges, those that carried area writes home and home
    /// changes to the area, and home reads.
    merges: u64,
    to_home: u64,
    to_area: u64,
    home_reads: u64,
}

impl Replica {
    fn new(x: u64, snap: Vec<u8>) -> Self {
        Replica {
            x,
            snap,
            home_dirty: false,
            quiet: 0,
            merges: 0,
            to_home: 0,
            to_area: 0,
            home_reads: 0,
        }
    }
}

/// A replica whose home lives outside the pool and saw no device-visible write is merged
/// without reading the home, except every this many merges (a sweep that bounds any
/// change the device did not see, e.g. a GPU write to a guest page table; the shadow's
/// walk caches assume none, ptcache.rs).
pub const HOME_SWEEP: u32 = 64;

/// The pool state inside [`FakeFb`].
pub struct Bar1Pool {
    pub root: u64,
    pub table: RemapTable,
    usable: (u64, u64),
    units: Vec<UnitState>,
    /// Table page -> units whose walk read it.
    page_units: HashMap<u64, BTreeSet<usize>>,
    /// Units one of whose page tables was written (walked again in full) and units only
    /// one of whose directory pages was written (their path is read first).
    dirty: BTreeSet<usize>,
    dir_dirty: BTreeSet<usize>,
    /// What the guest maps over the area now: the union of the units' `mapped`, kept up
    /// to date unit by unit.
    want: BTreeMap<u64, Target>,
    /// Area pages whose wanted remap is refused now: `g -> (target, reason)`.
    pending: BTreeMap<u64, (Target, String)>,
    /// Area pages whose release (or eviction) was refused and waits for its retry.
    waiting: BTreeSet<u64>,
    /// Pooled pages that became table pages the device tracks, `g -> X`: evicted at the
    /// next poll, marked changed at every sync point until then. Filled from the pages a
    /// walk starts to read ([`Bar1Pool::note_new_table_pages`], `set_unit`).
    pooled_tables: BTreeMap<u64, u64>,
    /// A walk stopped reading a table page: a replica's page may move in again.
    recheck_replicas: bool,
    /// The last poll's breakdown (the device traces it when the poll was slow).
    pub last: PollBreakdown,
    /// Write-through copies of the table pages the pool's walk reads (`page_units`):
    /// patched by every tracked write, dropped on a write of unknown content. A unit
    /// walk reads VRAM only for a page it has not seen (docs/caraxes/bar1-pool.md §12).
    pt_raw: crate::fb::TablePageCache,
    /// The home pages (4 KiB page numbers) of the replicas.
    replica_homes: HashSet<u64>,
    /// Moves whenever the set of replicas or a replica's home changes (a doorbell's cached
    /// set of the replicas its channel sees, boot.rs `pool_observed`).
    replica_gen: u64,
    /// `(g, key(target))` of every refusal logged.
    reported: HashSet<(u64, u64)>,
    reported_release: HashSet<(u64, u64)>,
    /// Shadow roots that must re-diff every unit at their next sync (a page they map
    /// moved).
    pub rediff: BTreeSet<u64>,
    hazards: HashSet<u64>,
    pub hazard_dirty: bool,
    /// Area page `g` -> the replica it serves.
    replicas: BTreeMap<u64, Replica>,
    last_retry: Option<Instant>,
    /// The current retry interval ([`RETRY_EVERY`] up to [`RETRY_MAX`]).
    retry_every: Duration,
    /// Area pages whose page the guest unmapped, kept in the pool (§13): `g -> since`. The
    /// remap stays, so every device path and the GPU still find the page at its pool page;
    /// it goes home when another page wants `g`, when the guest maps it elsewhere, when it
    /// becomes a table page, or after [`RETIRE_AGE`].
    retired: BTreeMap<u64, Instant>,
    /// When retired pages were last checked for their age.
    last_age_scan: Option<Instant>,
    /// The churn since the last churn line (`take_churn`).
    churn: Churn,
    /// The remap changes a poll decided and did not finish in its hold of the device lock
    /// (§14).
    batch: Option<Box<Batch>>,
    pub stats: PoolStats,
}

/// What the pool did since the last churn line: the pattern of the guest's BAR1 mapping
/// changes (docs/caraxes/bar1-pool.md §13).
#[derive(Default, Debug, Clone)]
pub struct Churn {
    since: Option<Instant>,
    polls: u64,
    installs: u64,
    releases: u64,
    retires: u64,
    relives: u64,
    relocations: u64,
    slot_evictions: u64,
    aged_out: u64,
    window_maps: u64,
    move_us: u128,
    max_poll_us: u128,
    /// Runs of consecutive area pages installed in one poll, by length: 1, 2-15, 16, 17-511,
    /// 512 and more pages (4 KiB, small, 64 KiB, medium, 2 MiB guest BAR1 mappings).
    runs: [u64; 5],
}

impl Churn {
    fn is_empty(&self) -> bool {
        self.installs + self.releases + self.retires + self.relives + self.aged_out == 0
    }

    fn note_runs(&mut self, gs: &mut [u64]) {
        gs.sort_unstable();
        let mut i = 0;
        while i < gs.len() {
            let mut j = i + 1;
            while j < gs.len() && gs[j] == gs[j - 1] + PAGE {
                j += 1;
            }
            let n = j - i;
            let b = match n {
                1 => 0,
                2..=15 => 1,
                16 => 2,
                17..=511 => 3,
                _ => 4,
            };
            self.runs[b] += 1;
            i = j;
        }
    }

    fn line(&self, secs: f64) -> String {
        format!(
            "bar1 pool churn in {secs:.1} s: {} poll(s) with changes (slowest {} us); moved in {} and home {} page(s) ({} us of moves, {} window map(s)); retired {} (no copy), re-lived {} (no copy), relocated {}, home for another page {}, aged out {}; install runs of 1 / 2-15 / 16 / 17-511 / 512+ pages: {} / {} / {} / {} / {} (docs/caraxes/bar1-pool.md §13)",
            self.polls,
            self.max_poll_us,
            self.installs,
            self.releases,
            self.move_us,
            self.window_maps,
            self.retires,
            self.relives,
            self.relocations,
            self.slot_evictions,
            self.aged_out,
            self.runs[0],
            self.runs[1],
            self.runs[2],
            self.runs[3],
            self.runs[4]
        )
    }
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
            dir_dirty: BTreeSet::new(),
            want: BTreeMap::new(),
            pending: BTreeMap::new(),
            waiting: BTreeSet::new(),
            pooled_tables: BTreeMap::new(),
            recheck_replicas: false,
            last: PollBreakdown::default(),
            pt_raw: HashMap::new(),
            replica_homes: HashSet::new(),
            replica_gen: 0,
            reported: HashSet::new(),
            reported_release: HashSet::new(),
            rediff: BTreeSet::new(),
            hazards: HashSet::new(),
            hazard_dirty: true,
            replicas: BTreeMap::new(),
            last_retry: None,
            retry_every: RETRY_EVERY,
            retired: BTreeMap::new(),
            last_age_scan: None,
            churn: Churn::default(),
            batch: None,
            stats: PoolStats::default(),
        }
    }

    /// Whether remap changes a poll decided are still running (§14): the invalidate
    /// trigger reads busy until they are done.
    pub fn batch_pending(&self) -> bool {
        self.batch.is_some()
    }

    /// Pages in the pool that the guest maps through the area now (the others are retired).
    pub fn live(&self) -> usize {
        self.table.len().saturating_sub(self.retired.len())
    }

    /// Area pages retired now (§13).
    pub fn retired_now(&self) -> usize {
        self.retired.len()
    }

    /// Whether area page `g` holds a retired page (§13).
    pub fn is_retired(&self, g: u64) -> bool {
        self.retired.contains_key(&g)
    }

    /// The churn line, at most once per [`CHURN_EVERY`] and only after activity (the
    /// device traces it after the poll, device.rs `after_bar0_write`).
    pub fn take_churn(&mut self) -> Option<String> {
        let since = self.churn.since?;
        let d = since.elapsed();
        if d < CHURN_EVERY || self.churn.is_empty() {
            return None;
        }
        let line = self.churn.line(d.as_secs_f64());
        self.churn = Churn::default();
        Some(line)
    }

    /// The retired area pages older than [`RETIRE_AGE`] at `now`, checked at most every
    /// [`RETIRE_SCAN`].
    /// Whether a poll has anything to do besides a replica merge (docs/caraxes/bar1-pool.md
    /// §16): a unit to walk (the guest wrote a BAR1 table page through a tracked path), a
    /// paced retry, a table-page hazard, a replica to check again, a retired page to age, a
    /// pooled table page to mark, or a batch to continue. A trapped BAR0 write that is no
    /// merge point skips the poll without it (device.rs `after_bar0_write_in`).
    pub fn work_due(&self, now: Instant) -> bool {
        !self.dirty.is_empty()
            || !self.dir_dirty.is_empty()
            || self.hazard_dirty
            || self.recheck_replicas
            || self.batch.is_some()
            || !self.pooled_tables.is_empty()
            || ((!self.pending.is_empty() || !self.waiting.is_empty())
                && self.last_retry.is_none_or(|t| now.duration_since(t) >= self.retry_every))
            || (!self.retired.is_empty()
                && self.last_age_scan.is_none_or(|t| now.duration_since(t) >= RETIRE_SCAN))
    }

    /// A poll skipped: nothing due and the BAR0 write no merge point.
    pub fn note_poll_skipped(&mut self) {
        self.stats.polls_skipped += 1;
    }

    fn aged(&mut self, now: Instant) -> Vec<u64> {
        if self.retired.is_empty() || self.last_age_scan.is_some_and(|t| now.duration_since(t) < RETIRE_SCAN) {
            return Vec::new();
        }
        self.last_age_scan = Some(now);
        self.retired
            .iter()
            .filter(|(_, t)| now.duration_since(**t) >= RETIRE_AGE)
            .map(|(g, _)| *g)
            .collect()
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

    /// Whether the pool follows page `page` (4 KiB page number): a table page of its area
    /// walk, or the home of a replica. The BAR2 memfd's table sync must report a change
    /// of such a page (fb.rs `FakeFb::device_table_page`).
    /// Whether FB page `page` is a table page of the pool's own BAR1 walk (not a replica home).
    pub fn follows_table(&self, page: u64) -> bool {
        self.page_units.contains_key(&page)
    }

    /// Whether any FB page in `[first, end)` (page numbers) is in the pool (its bytes at a
    /// pool page) or a replica's home: a display record window over it would miss them
    /// (fb.rs `share_record_window`).
    pub fn touches_home(&self, first: u64, end: u64) -> bool {
        self.table.fwd.range(first * PAGE..end * PAGE).next().is_some()
            || (first..end).any(|p| self.replica_homes.contains(&p))
    }

    pub fn tracks_page(&self, page: u64) -> bool {
        self.page_units.contains_key(&page) || self.replica_homes.contains(&page)
    }

    /// Every page [`Self::tracks_page`] holds true for.
    pub fn tracked_pages(&self) -> impl Iterator<Item = u64> + '_ {
        self.page_units.keys().chain(self.replica_homes.iter()).copied()
    }

    /// A guest write reached FB pages `first..=last`: re-walk the units whose tables
    /// they hold. A unit's page table written: the whole unit; only a directory page (the
    /// root, PD2, PD1 and PD0 are shared by every unit of the area): its path first.
    pub fn note_table_write(&mut self, first: u64, last: u64) {
        if self.page_units.is_empty() {
            return;
        }
        for p in first..=last {
            if let Some(us) = self.page_units.get(&p) {
                for &u in us {
                    if self.units[u].leaf_tables.contains(&p) {
                        self.dirty.insert(u);
                    } else {
                        self.dir_dirty.insert(u);
                    }
                }
            }
        }
    }

    /// A tracked write of `data` at FB address `pa` (`FakeFb::note_write`): the copies of
    /// the pool's table pages are patched, a replica whose home it hit reads its home at
    /// the next merge. Two emptiness checks when there is neither.
    pub fn note_write_data(&mut self, pa: u64, data: &[u8]) {
        if (self.pt_raw.is_empty() && self.replica_homes.is_empty()) || data.is_empty() {
            return;
        }
        let end = pa + data.len() as u64;
        for page in pa >> 12..=(end - 1) >> 12 {
            if let Some(c) = self.pt_raw.get_mut(&page) {
                let lo = pa.max(page << 12);
                let hi = end.min((page + 1) << 12);
                c[(lo - (page << 12)) as usize..(hi - (page << 12)) as usize]
                    .copy_from_slice(&data[(lo - pa) as usize..(hi - pa) as usize]);
            }
            if self.replica_homes.contains(&page) {
                self.mark_home(page);
            }
        }
    }

    /// Pages whose content changed without the bytes (a BAR2 memfd sync, a raw copy):
    /// their copies go, a replica home among them is read at the next merge.
    pub fn note_pages_unknown(&mut self, pages: &[u64]) {
        if self.pt_raw.is_empty() && self.replica_homes.is_empty() {
            return;
        }
        for p in pages {
            self.pt_raw.remove(p);
            if self.replica_homes.contains(p) {
                self.mark_home(*p);
            }
        }
    }

    /// A raw store write of `len` bytes at `pa` (a pool page, past the remap).
    pub fn note_raw_write(&mut self, pa: u64, len: usize) {
        if self.pt_raw.is_empty() || len == 0 {
            return;
        }
        for page in pa >> 12..=(pa + len as u64 - 1) >> 12 {
            self.pt_raw.remove(&page);
        }
    }

    fn mark_home(&mut self, page: u64) {
        for r in self.replicas.values_mut() {
            if r.x >> 12 == page {
                r.home_dirty = true;
            }
        }
    }

    fn index_homes(&mut self) {
        self.replica_homes = self.replicas.values().map(|r| r.x >> 12).collect();
        self.replica_gen += 1;
    }

    /// The replicas now, (area page, home), and the generation of that set (§16).
    pub fn replica_set(&self) -> (u64, Vec<(u64, u64)>) {
        (self.replica_gen, self.replicas.iter().map(|(g, r)| (*g, r.x)).collect())
    }

    /// Whether there are replicas.
    pub fn has_replicas(&self) -> bool {
        !self.replicas.is_empty()
    }

    /// A walk of the shadow started to read table pages `added` (4 KiB page numbers), or
    /// stopped reading one (`dropped`): a pooled page among `added` is now a table page
    /// the device tracks and is evicted at the next poll; a dropped page may let a
    /// replica's page move in again. Cost: one lookup per added page.
    pub fn note_new_table_pages(&mut self, added: &[u64], dropped: bool) {
        if dropped && !self.replicas.is_empty() {
            self.recheck_replicas = true;
        }
        if self.table.is_empty() {
            return;
        }
        for &pg in added {
            if let Some(g) = self.table.slot_of(pg << 12) {
                self.pooled_tables.insert(g, pg << 12);
                self.hazard_dirty = true;
            }
        }
    }

    fn unit_pages(&self, u: usize) -> usize {
        let (_, len) = self.table.pool();
        ((len - u as u64 * UNIT).min(UNIT) / PAGE) as usize
    }

    /// Take unit `u`'s new walk: the table-page index, `want`, and the area pages whose
    /// wanted mapping changed (added to `changed`).
    fn set_unit(&mut self, u: usize, w: crate::fb::AreaUnitWalk, changed: &mut BTreeSet<u64>) {
        let mut dropped = false;
        for p in &self.units[u].tables {
            if let Some(s) = self.page_units.get_mut(p) {
                s.remove(&u);
                if s.is_empty() {
                    self.page_units.remove(p);
                    dropped = true;
                }
            }
        }
        let mut tables = w.dirs.clone();
        tables.extend(w.leaf_tables.iter().copied());
        let mut added = Vec::new();
        for p in &tables {
            let s = self.page_units.entry(*p).or_default();
            if s.is_empty() {
                added.push(*p);
            }
            s.insert(u);
        }
        // The pages the unit mapped before and maps now, compared (both ascending).
        let old = std::mem::take(&mut self.units[u].mapped);
        let new: HashMap<u64, Target> = w.mapped.iter().copied().collect();
        for (g, t) in &old {
            if new.get(g) != Some(t) {
                changed.insert(*g);
                self.want.remove(g);
            }
        }
        let before: HashMap<u64, Target> = old.into_iter().collect();
        for (g, t) in &w.mapped {
            if before.get(g) != Some(t) {
                changed.insert(*g);
            }
            self.want.insert(*g, *t);
        }
        self.units[u] = UnitState {
            tables,
            dirs: w.dirs,
            leaf_tables: w.leaf_tables,
            pde: w.pde,
            mapped: w.mapped,
        };
        self.note_new_table_pages(&added, dropped);
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
            "bar1 pool: guest BAR1 [0, {len:#x}) on the pool FB [{pool0:#x}, {:#x}); {} poll(s), {} unit walk(s); {} page(s) in the pool now; moved in {} cold + {} behind the barrier, back {} cold + {} behind the barrier; {} barrier(s) ({} refused, {} TSG preempt(s)); {} refused, {} pending now, {} sysmem page(s) in the area; copied {} KiB in, {} KiB back in {} us; {} page-table hazard(s) ({} evicted, {} refused, {} re-read(s) while pooled); {} replica(s) made, {} now, {} merge(s) at sync points, {} page merge(s) to home, {} to the area, {} conflicting word(s); {} page(s) also behind the BAR2 memfd; {} retry poll(s), {} of them moved nothing (interval now {} ms, at most {} ms); slowest poll {} us (docs/caraxes/bar1-pool.md §10); lazy move back: {} retired now, {} retired in all (no copy), {} re-lived (no copy), home {} for another page, {} relocated, {} aged out after {} ms; {} FB store window map(s) for moves; {} device-CE transfer(s) kept on the CPU by a pooled page ({} by a retired one) (§13); lock holds: {} batch(es) in more than one hold ({} hold(s) left the rest for the next poll, longest batch {} us), {} wait(s) for a shadow job in flight ({} left to the next poll, {} us of worker waits in all), {} hold(s) left a barrier to the worker, GPU checks {} ms, barriers {} ms, moves {} ms, re-syncs {} ms in all (§14, §15); {} poll(s) skipped after a BAR0 write with nothing due; {} doorbell(s) merged {} replica(s) their channels see, {} carried area writes home (§16)",
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
            s.table_evictions,
            s.table_refusals,
            s.table_resyncs,
            s.replicas,
            self.replicas.len(),
            s.replica_syncs,
            s.replica_to_home,
            s.replica_to_area,
            s.replica_conflicts,
            s.bar2_aliased,
            s.retries,
            s.retry_backoffs,
            self.retry_every.as_millis(),
            RETRY_MAX.as_millis(),
            s.max_poll_us,
            self.retired.len(),
            s.retires,
            s.relives,
            s.slot_evictions,
            s.relocations,
            s.aged_out,
            RETIRE_AGE.as_millis(),
            s.move_window_maps,
            s.devce_blocked.get(),
            s.devce_blocked_retired.get(),
            s.sliced_batches,
            s.slices_parked,
            s.batch_max_us,
            s.in_flight_waits,
            s.in_flight_parked,
            s.in_flight_wait_us,
            s.barrier_parked,
            s.refs_us / 1000,
            s.barrier_us / 1000,
            s.move_us / 1000,
            s.resync_us / 1000,
            s.polls_skipped,
            s.ring_merges,
            s.ring_replicas,
            s.ring_to_home
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

/// A barrier in progress (§15): the roots whose rings are held and whose TSGs the worker
/// preempts, and what its end waits for.
#[derive(Default)]
pub struct Barrier {
    pub roots: BTreeSet<u64>,
    /// A ring hold per root, released by the worker after the root's re-sync.
    pub tokens: Vec<(u64, u64)>,
    /// The preempt jobs (root, sequence number) and each TSG's status slot.
    pub jobs: Vec<(u64, u64)>,
    pub status: Vec<(u32, std::sync::Arc<std::sync::Mutex<Option<u32>>>)>,
    /// The host channels found idle, checked again once the preempts ran.
    pub channels: Vec<u32>,
    pub nchannels: usize,
    pub ntsgs: usize,
}

/// The GPU side of the pool: who may be using a page, and the barrier.
pub trait PoolGpu {
    /// A reason no remap may change at all now (a host channel walks the guest's own
    /// tables, which the remap cannot reach).
    fn blocked(&self) -> Option<String>;
    /// The GPU references of each FB page in `pages`, as the shadows' cached walks show
    /// them: no wait for the shadow worker. The poll asks [`Self::in_flight`] first and
    /// waits for its answer (§14).
    fn refs(&self, fb: &FakeFb, pages: &BTreeSet<u64>) -> BTreeMap<u64, Refs>;
    /// The newest shadow job not yet applied that may still map a page of `pages` on the
    /// host beyond what [`Self::refs`] sees (a leaf its plan took out of the walk, a
    /// teardown); `None`: nothing to wait for (§14).
    fn in_flight(&self, _fb: &FakeFb, _pages: &BTreeSet<u64>) -> Option<u64> {
        None
    }
    /// Wait until shadow job `seq` is applied, at most `max` (`None`: to the end); whether
    /// it is.
    fn wait_applied(&self, _seq: u64, _max: Option<Duration>) -> bool {
        true
    }
    /// A stamp that changes whenever the answers of [`Self::refs`] and [`Self::in_flight`]
    /// may change (a shadow job submitted); a batch asks again then.
    fn stamp(&self) -> u64 {
        0
    }
    /// The pages of `pages` that are a mirrored channel's USERD, with the channel: the
    /// part of [`Self::refs`] that needs no wait for the shadow worker and no scan of the
    /// shadows' leaves. The poll refuses these pages before it asks for the rest
    /// (a release refused for a USERD is retried until the channel goes).
    fn userd_refs(&self, _pages: &BTreeSet<u64>) -> BTreeMap<u64, u32> {
        BTreeMap::new()
    }
    /// Make sure no GPU work runs on the shadow VA spaces of `roots` until the next
    /// doorbell: every host channel there idle, its TSG preempted with wait.
    /// `Ok((channels, TSGs preempted))`, or why not.
    fn quiesce(&mut self, fb: &mut FakeFb, roots: &BTreeSet<u64>) -> Result<(usize, usize), String>;
    /// Re-sync the shadows of `roots` (the remap is in place) and wait for the worker.
    fn resync(&mut self, fb: &mut FakeFb, roots: &BTreeSet<u64>) -> Vec<String>;
    /// Start the barrier for `roots` without waiting (§15); `Err`: it cannot hold (nothing is
    /// held then). Default: the synchronous [`Self::quiesce`].
    fn begin_barrier(&mut self, fb: &mut FakeFb, roots: &BTreeSet<u64>) -> Result<Barrier, String> {
        let (nchannels, ntsgs) = self.quiesce(fb, roots)?;
        Ok(Barrier {
            roots: roots.clone(),
            nchannels,
            ntsgs,
            ..Default::default()
        })
    }
    /// Whether barrier `b` holds (its preempts ran, every channel still idle), waiting at
    /// most `max` (`None`: to the end); `Err`: it failed (the caller cancels it).
    fn barrier_ready(&mut self, _fb: &mut FakeFb, _b: &Barrier, _max: Option<Duration>) -> Result<bool, String> {
        Ok(true)
    }
    /// After the moves: re-sync the shadows of the barrier's roots. Returns the notes and the
    /// jobs the guest's trigger waits for (the worker releases the rings after them).
    /// Default: the synchronous [`Self::resync`].
    fn end_barrier(&mut self, fb: &mut FakeFb, b: Barrier) -> (Vec<String>, Vec<(u64, u64)>) {
        (self.resync(fb, &b.roots), Vec::new())
    }
    /// Drop a barrier that moved nothing, or failed: its rings released.
    fn cancel_barrier(&mut self, _b: Barrier) {}
    /// Whether shadow jobs (root, sequence number) are applied, waiting at most `max`.
    fn jobs_applied(&self, _jobs: &[(u64, u64)], _max: Option<Duration>) -> bool {
        true
    }
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

impl MirrorGpu<'_> {
    /// Whether mirrored channel `chid` has no work left: GPGet equals GPPut in its USERD.
    fn channel_idle(&self, fb: &mut FakeFb, chid: u32) -> Result<(), String> {
        let gm = crate::channel::GpuMem { fb, sys: self.sys };
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
    }
}

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
        // The cached walks show every leaf a shadow maps or will map once the worker
        // applied its plans; a leaf the guest dropped may still be mapped until then, which
        // `in_flight` answers (§14, the poll waited for it). No wait for the worker here.
        for r in self.mirror.shadow_roots() {
            // Fail closed: a shadow root without a cached walk may map any page.
            let hit = fb.with_walk(Target::Fb(r), |c| {
                let mut hit: Vec<u64> = Vec::new();
                for (_, t, size, _) in c.leaves() {
                    let Target::Fb(pa) = t else { continue };
                    hit.extend(pages.range(pa & !(PAGE - 1)..pa.saturating_add(*size)));
                }
                hit
            });
            let Some(hit) = hit else {
                for p in pages {
                    out.entry(*p).or_default().roots.insert(r);
                }
                continue;
            };
            for p in hit {
                out.entry(p).or_default().roots.insert(r);
            }
        }
        for (page, chid) in self.userd_refs(pages) {
            out.entry(page).or_default().userd = Some(chid);
        }
        out
    }

    fn in_flight(&self, _fb: &FakeFb, pages: &BTreeSet<u64>) -> Option<u64> {
        self.mirror.shadow_in_flight(pages)
    }

    fn wait_applied(&self, seq: u64, max: Option<Duration>) -> bool {
        self.mirror.shadow_wait_applied(seq, max)
    }

    fn stamp(&self) -> u64 {
        self.mirror.shadow_submitted()
    }

    fn userd_refs(&self, pages: &BTreeSet<u64>) -> BTreeMap<u64, u32> {
        let mut out = BTreeMap::new();
        for (chid, _, _) in self.mirror.pool_channels() {
            if let Some(Target::Fb(u)) = self.reg.channel(chid).and_then(|c| c.userd) {
                let page = u & !(PAGE - 1);
                if pages.contains(&page) {
                    out.insert(page, chid);
                }
            }
        }
        out
    }

    fn quiesce(&mut self, fb: &mut FakeFb, roots: &BTreeSet<u64>) -> Result<(usize, usize), String> {
        let chans: Vec<(u32, Option<u64>, Option<u32>)> = self
            .mirror
            .pool_channels()
            .into_iter()
            .filter(|c| c.1.is_some_and(|r| roots.contains(&r)))
            .collect();
        for c in &chans {
            self.channel_idle(fb, c.0)?;
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
            self.channel_idle(fb, c.0)?;
        }
        Ok((chans.len(), tsgs.len()))
    }

    fn resync(&mut self, fb: &mut FakeFb, roots: &BTreeSet<u64>) -> Vec<String> {
        let mut gm = crate::channel::GpuMem { fb, sys: self.sys };
        let mut notes = Vec::new();
        for r in roots {
            notes.extend(self.mirror.shadow_resync(self.reg, &mut gm, Some(*r)));
        }
        // A set-B change this re-sync made goes to the trace: `poll_at` keeps only REFUSED
        // and ERROR notes, so the recompute that emptied a root's exclusions here never
        // showed (vm4-615, 2026-09-26, docs/research/va-alloc-bursts-2026-09-26.md).
        for n in notes.iter().filter(|n| n.contains("set-B")) {
            self.mirror.trace_note(format!("bar1 pool re-sync: {n}"));
        }
        // The doorbell ordering rule (mirror.rs `gr_shadow_doorbell`): the worker has
        // applied every plan before anything else runs on the GPU.
        self.mirror.shadow_barrier();
        gm.fb.flush();
        notes
    }

    /// §15: no wait under the device lock. The channels on `roots` must be idle; their
    /// rings are held (a doorbell is not forwarded to the host meanwhile) and their TSGs
    /// preempted by worker jobs in order with the roots' plans.
    fn begin_barrier(&mut self, fb: &mut FakeFb, roots: &BTreeSet<u64>) -> Result<Barrier, String> {
        if !self.mirror.shadow_worker_running() {
            let (nchannels, ntsgs) = self.quiesce(fb, roots)?;
            return Ok(Barrier {
                roots: roots.clone(),
                nchannels,
                ntsgs,
                ..Default::default()
            });
        }
        let chans: Vec<(u32, Option<u64>, Option<u32>)> = self
            .mirror
            .pool_channels()
            .into_iter()
            .filter(|c| c.1.is_some_and(|r| roots.contains(&r)))
            .collect();
        for c in &chans {
            self.channel_idle(fb, c.0)?;
        }
        let tokens: Vec<(u64, u64)> = roots.iter().map(|r| (*r, self.mirror.hold_rings(vec![*r]))).collect();
        let tsgs: BTreeMap<u32, u64> = chans
            .iter()
            .filter_map(|c| Some((c.2?, c.1?)))
            .collect();
        let mut jobs = Vec::new();
        let mut status = Vec::new();
        for (tsg, root) in &tsgs {
            let (job, slot) = self.mirror.submit_preempt(*root, *tsg);
            jobs.push(job);
            status.push((*tsg, slot));
        }
        Ok(Barrier {
            roots: roots.clone(),
            tokens,
            jobs,
            status,
            channels: chans.iter().map(|c| c.0).collect(),
            nchannels: chans.len(),
            ntsgs: tsgs.len(),
        })
    }

    fn barrier_ready(&mut self, fb: &mut FakeFb, b: &Barrier, max: Option<Duration>) -> Result<bool, String> {
        if !self.mirror.shadow_jobs_applied(&b.jobs, max) {
            return Ok(false);
        }
        for (tsg, slot) in &b.status {
            match *slot.lock().unwrap() {
                Some(st) if st == nvrm_abi::sys::NV_OK => {}
                Some(st) => return Err(format!("preempt of host TSG {tsg:#x}: status {st:#x}")),
                None => return Err(format!("preempt of host TSG {tsg:#x} did not run")),
            }
        }
        // GPPut may have moved while the preempts ran (a guest CPU write through the area):
        // read again, now that the TSGs are off the engines.
        for chid in &b.channels {
            self.channel_idle(fb, *chid)?;
        }
        Ok(true)
    }

    fn end_barrier(&mut self, fb: &mut FakeFb, b: Barrier) -> (Vec<String>, Vec<(u64, u64)>) {
        if b.tokens.is_empty() {
            return (self.resync(fb, &b.roots), Vec::new());
        }
        let mut gm = crate::channel::GpuMem { fb, sys: self.sys };
        let mut notes = Vec::new();
        for r in &b.roots {
            notes.extend(self.mirror.shadow_resync(self.reg, &mut gm, Some(*r)));
        }
        for n in notes.iter().filter(|n| n.contains("set-B")) {
            self.mirror.trace_note(format!("bar1 pool re-sync: {n}"));
        }
        gm.fb.flush();
        // The worker releases each root's rings after its re-sync (ringgate.rs): no wait here.
        let jobs = b
            .tokens
            .iter()
            .map(|(r, t)| self.mirror.release_rings_after(*r, *t))
            .collect();
        (notes, jobs)
    }

    fn cancel_barrier(&mut self, b: Barrier) {
        for (_, t) in b.tokens {
            self.mirror.release_rings(t);
        }
    }

    fn jobs_applied(&self, jobs: &[(u64, u64)], max: Option<Duration>) -> bool {
        self.mirror.shadow_jobs_applied(jobs, max)
    }
}

/// Poll the pool of the boot model after a trapped BAR0 write (device.rs): the guest
/// flushes and invalidates through BAR0 after it changes a BAR1 mapping and before it uses
/// the mapping. `merge`: the write is a flush point, the replicas are merged
/// ([`poll_at`], `BootModel::pool_merge_after`). Returns whether a poll ran.
pub fn poll_model(b: &mut crate::boot::BootModel, mem: &GuestMemory, merge: bool, pace: Pace) -> bool {
    if b.fb.bar1_pool().is_none() {
        return false;
    }
    // The pages the guest maps through the area now: retired pages (§13) do not count.
    let held = |fb: &FakeFb| fb.pool_mut().as_ref().map_or(0, |p| p.live());
    let before = held(&b.fb);
    let reg = &b.ctrl.channels;
    let ran = match b.ctrl.mirror.as_mut() {
        Some(m) => {
            let mut gpu = MirrorGpu {
                mirror: m,
                reg,
                sys: mem,
            };
            poll_sliced(&mut b.fb, &mut gpu, merge, pace)
        }
        None => poll_sliced(&mut b.fb, &mut NoGpu, merge, pace),
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

/// A read of the invalidate trigger with a batch pending (§14, device.rs): its next chunk,
/// and once it is done, what the guest changed meanwhile. Returns whether a batch was
/// pending.
pub fn continue_model(b: &mut crate::boot::BootModel, mem: &GuestMemory, pace: Pace) -> bool {
    if !b.fb.pool_mut().as_ref().is_some_and(|p| p.batch_pending()) {
        return false;
    }
    poll_model(b, mem, false, pace);
    true
}

/// One remap change the poll decided on.
#[derive(Clone, Copy, Debug)]
struct Op {
    install: bool,
    g: u64,
    x: u64,
    /// A release the guest did not ask for: `x` became a table page the device tracks
    /// while it lived in the pool, so it goes back to the trapped path.
    evict: bool,
    /// A release of a retired page (§13), and why it goes home now.
    home: Option<Home>,
}

/// Why a retired page goes home (§13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Home {
    /// Another page (or guest RAM) is wanted at its area page.
    Slot,
    /// The guest maps it at another area page (it moves there after its move home).
    Relocate,
    /// It stayed retired for [`RETIRE_AGE`].
    Aged,
}

/// Whether FB page `page` (a 4 KiB page number) is a guest page-table page whose writes
/// the device tracks: a shadow's cached walk reads it, or the pool's own walk of the
/// guest's BAR1 tables over the area does.
fn tracked_table_page(fb: &FakeFb, p: &Bar1Pool, page: u64) -> bool {
    p.page_units.contains_key(&page) || fb.watches_table_page(page)
}

/// Whether `X` may move to area page `g`: the table's static checks, then the table-page
/// rule (a page whose writes the device tracks never lives where the guest writes it
/// untracked).
fn admit(fb: &FakeFb, p: &Bar1Pool, g: u64, x: u64) -> Result<(), Refusal> {
    p.table.check(g, x, p.usable)?;
    if tracked_table_page(fb, p, x >> 12) {
        return Err(Refusal::TablePage);
    }
    Ok(())
}

/// [`admit`] against the table as it will be after the releases of `released`.
fn admit_after(
    fb: &FakeFb,
    p: &Bar1Pool,
    g: u64,
    x: u64,
    released: &BTreeSet<u64>,
) -> Result<(), Refusal> {
    p.table.check_after(g, x, p.usable, released)?;
    if tracked_table_page(fb, p, x >> 12) {
        return Err(Refusal::TablePage);
    }
    Ok(())
}

/// Merge the replica of area page `g` both ways: 8-byte words the guest changed through
/// the area since the last merge are written to the page's home through
/// [`FakeFb::write`] (so the shadow's walk caches, the BAR TLB, the pool's walk and the
/// BAR2 memfd see them as any trapped write), words that changed at home (a device write,
/// the guest's BAR2 or other BAR1 mapping) are copied to the area. A word changed on both
/// sides takes the area's value. With `last`, the replica is dropped after the merge.
/// Returns whether anything was written.
fn merge_replica(fb: &mut FakeFb, g: u64, last: bool) -> bool {
    let (p, x, mut snap, read_home) = {
        let mut guard = fb.pool_mut();
        let Some(pool) = guard.as_mut() else { return false };
        let pool0 = pool.table.pool().0;
        // A home in the pool (an aliased page) is written by the guest through its own
        // area page and by the GPU without the device seeing it: always read.
        let pooled = |x: u64| pool.table.is_remapped(x);
        let Some(r) = pool.replicas.get(&g) else { return false };
        let read_home = last || r.home_dirty || r.quiet + 1 >= HOME_SWEEP || pooled(r.x);
        let r = pool.replicas.get_mut(&g).expect("just found");
        (pool0 + g, r.x, std::mem::take(&mut r.snap), read_home)
    };
    let n = PAGE as usize;
    let mut area = vec![0u8; n];
    fb.read_raw(p, &mut area);
    let home = if read_home {
        let mut home = vec![0u8; n];
        fb.read(x, &mut home);
        home
    } else {
        // No device-visible write reached the home since the last merge: it holds the
        // snapshot.
        snap.clone()
    };
    let (mut to_home, mut to_area) = (Vec::new(), Vec::new());
    let mut conflicts = 0u64;
    let push = |runs: &mut Vec<(usize, usize)>, o: usize| match runs.last_mut() {
        Some((s, l)) if *s + *l == o => *l += 8,
        _ => runs.push((o, 8)),
    };
    for o in (0..n).step_by(8) {
        let (a, h, s) = (&area[o..o + 8], &home[o..o + 8], &snap[o..o + 8]);
        if a != s {
            if h != s && h != a {
                conflicts += 1;
            }
            push(&mut to_home, o);
        } else if h != s {
            push(&mut to_area, o);
        }
    }
    for &(o, l) in &to_home {
        fb.write(x + o as u64, &area[o..o + l]);
        snap[o..o + l].copy_from_slice(&area[o..o + l]);
    }
    for &(o, l) in &to_area {
        fb.write_raw(p + o as u64, &home[o..o + l]);
        snap[o..o + l].copy_from_slice(&home[o..o + l]);
    }
    let mut guard = fb.pool_mut();
    let pool = guard.as_mut().expect("on");
    pool.stats.replica_to_home += u64::from(!to_home.is_empty());
    pool.stats.replica_to_area += u64::from(!to_area.is_empty());
    pool.stats.replica_conflicts += conflicts;
    pool.stats.replica_home_reads += u64::from(read_home);
    if last {
        pool.replicas.remove(&g);
        pool.index_homes();
    } else if let Some(r) = pool.replicas.get_mut(&g) {
        r.snap = snap;
        // Its own writes home (just merged) are in the snapshot.
        r.home_dirty = false;
        r.quiet = if read_home { 0 } else { r.quiet + 1 };
        r.merges += 1;
        r.to_home += u64::from(!to_home.is_empty());
        r.to_area += u64::from(!to_area.is_empty());
        r.home_reads += u64::from(read_home);
    }
    !to_home.is_empty() || !to_area.is_empty()
}

/// A doorbell's merge (§16): the area writes of the replicas `gs` go home before the rung
/// channels' engines can see the home (on the card both BAR1 mappings reach the same VRAM).
/// One direction only: the area is read and compared with the snapshot, the changed words
/// are written home; changes at home reach the area at the next full merge (a flush point).
/// Returns whether anything was written.
pub fn merge_to_home(fb: &mut FakeFb, gs: &[u64]) -> bool {
    let mut wrote = false;
    let mut merged = 0u64;
    let mut carried = 0u64;
    for g in gs {
        let (p, x, mut snap, dirty_before) = {
            let mut guard = fb.pool_mut();
            let Some(pool) = guard.as_mut() else { return false };
            let pool0 = pool.table.pool().0;
            let Some(r) = pool.replicas.get_mut(g) else { continue };
            (pool0 + g, r.x, std::mem::take(&mut r.snap), r.home_dirty)
        };
        let n = PAGE as usize;
        let mut area = vec![0u8; n];
        fb.read_raw(p, &mut area);
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for o in (0..n).step_by(8) {
            if area[o..o + 8] != snap[o..o + 8] {
                match runs.last_mut() {
                    Some((s, l)) if *s + *l == o => *l += 8,
                    _ => runs.push((o, 8)),
                }
            }
        }
        for &(o, l) in &runs {
            fb.write(x + o as u64, &area[o..o + l]);
            snap[o..o + l].copy_from_slice(&area[o..o + l]);
        }
        merged += 1;
        carried += u64::from(!runs.is_empty());
        wrote |= !runs.is_empty();
        let mut guard = fb.pool_mut();
        let pool = guard.as_mut().expect("on");
        if let Some(r) = pool.replicas.get_mut(g) {
            r.snap = snap;
            // Its own writes home are in the snapshot; a device write before them still
            // makes the next full merge read the home.
            r.home_dirty = dirty_before;
            r.to_home += u64::from(!runs.is_empty());
        }
    }
    if let Some(pool) = fb.pool_mut().as_mut() {
        pool.stats.ring_merges += u64::from(merged > 0);
        pool.stats.ring_replicas += merged;
        pool.stats.ring_to_home += carried;
    }
    if wrote {
        fb.flush();
    }
    wrote
}

/// The order in which a sync point merges the replicas: by home page, then area page.
/// A merge reads the area's pool page (one region of VRAM) and the home page (anywhere in
/// the reservation) through the device's few BAR1 windows (fb.rs `VramStore`, two of 32 MiB
/// by default). In area order the homes can alternate between regions, and every merge
/// can then cost a window remap (host RM calls; 2.5 ms measured for a window miss, fb.rs
/// `FakeFb::watched`); in home order a sync point maps each region once at most.
fn merge_order(replicas: &BTreeMap<u64, Replica>) -> Vec<u64> {
    let mut by_home: Vec<(u64, u64)> = replicas.iter().map(|(g, r)| (r.x, *g)).collect();
    by_home.sort_unstable();
    by_home.into_iter().map(|(_, g)| g).collect()
}

/// Make area page `g` a replica of FB page `x`: the pool page gets the page's bytes, and
/// from now on the sync points merge the two ([`sync_untracked`]).
fn make_replica(fb: &mut FakeFb, g: u64, x: u64) {
    // A display record window over the home goes first: the replica mirrors what the device
    // writes to the home, and the window's writes bypass that.
    fb.revoke_records(x / PAGE, x / PAGE + 1);
    let mut b = vec![0u8; PAGE as usize];
    fb.read(x, &mut b);
    let p = fb.bar1_pool().expect("on").0 + g;
    fb.write_raw(p, &b);
    let mut guard = fb.pool_mut();
    let pool = guard.as_mut().expect("on");
    pool.replicas.insert(g, Replica::new(x, b));
    pool.index_homes();
    pool.stats.replicas += 1;
}

/// The replicas now, one line each, for the exit summary (docs/caraxes/bar1-pool.md §16):
/// the area page, the home and where it lives (an alias: another area page holds it), the
/// merges and how many carried area writes home or home changes to the area, the home
/// reads, and what uses the home: a mirrored channel's USERD, the shadow roots whose walks
/// map it and at which VA, a channel's GPFIFO among those VAs.
pub fn replica_lines(b: &crate::boot::BootModel) -> Vec<String> {
    let fb = &b.fb;
    // (area page, home, the home's own area page, merges, to home, to area, home reads)
    type Row = (u64, u64, Option<u64>, u64, u64, u64, u64);
    let rows: Vec<Row> = {
        let guard = fb.pool_mut();
        let Some(p) = guard.as_ref() else { return Vec::new() };
        p.replicas
            .iter()
            .map(|(g, r)| {
                let home_g = p.table.slot_of(r.x);
                (*g, r.x, home_g, r.merges, r.to_home, r.to_area, r.home_reads)
            })
            .collect()
    };
    let roots: Vec<u64> = b.ctrl.mirror.as_ref().map(|m| m.shadow_roots()).unwrap_or_default();
    rows.into_iter()
        .map(|(g, x, home_g, merges, to_home, to_area, home_reads)| {
            let mut uses: Vec<String> = Vec::new();
            for ch in b.ctrl.channels.channels() {
                if let Some(Target::Fb(u)) = ch.userd {
                    if u & !(PAGE - 1) == x {
                        uses.push(format!("USERD of chid {}", ch.chid));
                    }
                }
            }
            for r in &roots {
                let vas: Vec<u64> = fb
                    .with_walk(Target::Fb(*r), |c| {
                        c.leaves()
                            .filter_map(|(va, t, size, _)| match t {
                                Target::Fb(pa) if *pa <= x && x < pa + size => Some(va + (x - pa)),
                                _ => None,
                            })
                            .take(4)
                            .collect()
                    })
                    .unwrap_or_default();
                for va in vas {
                    let gpfifo = b.ctrl.channels.channels().find(|ch| {
                        let len = u64::from(ch.gpfifo_entries) * 8;
                        va >= ch.gpfifo_va && va < ch.gpfifo_va.saturating_add(len)
                    });
                    uses.push(match gpfifo {
                        Some(ch) => format!("root {r:#x} VA {va:#x} (GPFIFO of chid {})", ch.chid),
                        None => format!("root {r:#x} VA {va:#x}"),
                    });
                }
            }
            if uses.is_empty() {
                uses.push("no shadow maps it, no channel's USERD".into());
            }
            format!(
                "bar1 pool replica: area page {g:#x} mirrors FB {x:#x}{}; {merges} merge(s), {to_home} carried area writes home, {to_area} carried home changes to the area, {home_reads} home read(s); {}",
                match home_g {
                    Some(h) => format!(" (an alias: the page lives in the pool at area page {h:#x})"),
                    None => " (its home is outside the pool)".into(),
                },
                uses.join(", ")
            )
        })
        .collect()
}

/// The sync points of the pool's untracked pages: called where the guest orders its CPU
/// writes before the device or the GPU uses them (the MMU invalidate, the GSP doorbell,
/// the trapped BAR2 flush read, and the polls after the BAR0 writes that are flush points,
/// [`poll_at`]).
/// - Every replica is merged both ways (`merge_replica`).
/// - A table page the device tracks that still lives in the pool (its eviction waits for
///   the next poll, or a barrier refused it) is marked changed, so the shadow's next walk
///   re-reads it from its pool page instead of trusting its write-through copy, and the
///   poll is told to evict it.
/// - Design G's twin: [`FakeFb::bar1_pt_resync`].
///
/// Returns whether anything was merged or marked. Two borrow checks when there is nothing.
pub fn sync_untracked(fb: &mut FakeFb) -> bool {
    sync_inner(fb, true, &mut PollBreakdown::default())
}

/// [`sync_untracked`] at an MMU invalidate (docs/caraxes/bar2-memfd.md §3 rules 2 and 3):
/// the marks, and the merges of the replicas whose home is a page table the device follows
/// (the guest's PTE writes through the area reach the walks before the invalidate plans);
/// a data replica waits for the next engine start, whose flush point merges every replica.
/// SotTR run E: the full merge at every invalidate cost 675 us each, 144 s over the run.
pub fn sync_untracked_tables(fb: &mut FakeFb) -> bool {
    sync_inner_where(fb, true, &mut PollBreakdown::default(), true)
}

/// [`sync_untracked`]; without `merge` only the marks (design G's and the pooled table
/// pages'), which cost a few lookups, not the two page reads per replica.
fn sync_inner(fb: &mut FakeFb, merge: bool, bd: &mut PollBreakdown) -> bool {
    sync_inner_where(fb, merge, bd, false)
}

/// [`sync_inner`]; with `tables_only` only the replicas whose home is a followed table page
/// are merged.
fn sync_inner_where(fb: &mut FakeFb, merge: bool, bd: &mut PollBreakdown, tables_only: bool) -> bool {
    let g_marked = fb.bar1_pt_resync() > 0;
    let (gs, pooled) = {
        let guard = fb.pool_mut();
        let Some(p) = guard.as_ref() else {
            return g_marked;
        };
        if p.replicas.is_empty() && p.pooled_tables.is_empty() {
            return g_marked;
        }
        let gs: Vec<(u64, u64)> = if merge {
            merge_order(&p.replicas)
                .into_iter()
                .filter_map(|g| p.replicas.get(&g).map(|r| (g, r.x)))
                .collect()
        } else {
            Vec::new()
        };
        let pooled: Vec<(u64, u64)> = p.pooled_tables.iter().map(|(g, x)| (*g, *x)).collect();
        (gs, pooled)
    };
    let gs: Vec<u64> = gs
        .into_iter()
        .filter(|(_, x)| !tables_only || fb.followed_table_page(x >> 12))
        .map(|(g, _)| g)
        .collect();
    let homes0 = fb.pool_mut().as_ref().map_or(0, |p| p.stats.replica_home_reads);
    let mut wrote = false;
    for g in &gs {
        wrote |= merge_replica(fb, *g, false);
    }
    bd.replicas_merged = gs.len();
    bd.homes_read = fb.pool_mut().as_ref().map_or(0, |p| p.stats.replica_home_reads) - homes0;
    bd.tables_marked = pooled.len();
    if !pooled.is_empty() {
        let pages: Vec<u64> = pooled.iter().map(|(_, x)| x >> 12).collect();
        fb.note_pages_changed(&pages);
    }
    if let Some(p) = fb.pool_mut().as_mut() {
        p.stats.replica_syncs += u64::from(!gs.is_empty());
        if !pooled.is_empty() {
            p.stats.table_resyncs += pooled.len() as u64;
            // A page not seen before: the next poll evicts it (a page seen before waits for
            // the paced retry of its refused eviction).
            if pooled.iter().any(|(_, x)| !p.hazards.contains(&(x >> 12))) {
                p.hazard_dirty = true;
            }
        }
    }
    if wrote {
        fb.flush();
    }
    g_marked || wrote || !pooled.is_empty()
}

/// [`poll_at`] with the replica merge (every sync point merges, the tests' default).
pub fn poll(fb: &mut FakeFb, gpu: &mut dyn PoolGpu) -> bool {
    poll_at(fb, gpu, true)
}

/// Bring the remap table in line with the guest's BAR1 tables over the area: walk the
/// units whose tables changed, release what the guest unmapped (copy back), install what it
/// newly mapped (copy in), each page cold or behind the barrier, and retry what was refused
/// (at most every [`RETRY_EVERY`], backing off to [`RETRY_MAX`]). A page whose writes the
/// device tracks (a guest page table) never lives in the pool: it is evicted when it
/// becomes one, refused when it is one already; such a page, and a page the guest maps
/// twice, is served by its area page as a replica ([`sync_untracked`], merged when
/// `merge`). Returns whether anything ran.
///
/// Incremental (docs/caraxes/bar1-pool.md §11): the cost follows what changed, not the
/// pool's size or the guest's page tables.
/// - Only the units whose tables were written are walked; a unit whose directory page was
///   written (shared by every unit) is walked only when its path changed.
/// - Only the area pages whose wanted mapping changed are decided, plus the paced retries
///   and the replicas; `want` is kept unit by unit.
/// - A pooled page that became a table page is known from the walks' new pages; nothing
///   scans the walks.
/// - A page that is a mirrored channel's USERD is refused before the GPU references are
///   taken (no wait for the shadow worker, no leaf scan of every shadow root).
pub fn poll_at(fb: &mut FakeFb, gpu: &mut dyn PoolGpu, merge: bool) -> bool {
    poll_sliced(fb, gpu, merge, Pace::ALL)
}

/// How a poll may spend its hold of the device lock (§14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pace {
    /// The moves run until this is spent (a chunk at least), the rest at the next polls;
    /// `None`: all now.
    pub budget: Option<Duration>,
    /// A shadow job in flight that may still map a page to move is waited for at most this
    /// long per poll, then the batch waits for the next poll; `None`: to the end.
    pub wait: Option<Duration>,
}

impl Pace {
    /// Everything now, waits included: [`poll_at`] and the invalidate page, whose trigger
    /// the reactor clears when the poll returns.
    pub const ALL: Pace = Pace { budget: None, wait: None };
    /// A trapped sync point outside an invalidate: every move now, no wait for the worker
    /// beyond [`WAIT_SLICE`].
    pub const NO_WAIT: Pace = Pace {
        budget: None,
        wait: Some(WAIT_SLICE),
    };
    /// A trapped write of the invalidate registers and a read of its trigger.
    pub const SLICED: Pace = Pace {
        budget: Some(POOL_SLICE),
        wait: Some(WAIT_SLICE),
    };
}

/// [`poll_at`] with a pace (§14): the moves run in chunks until the budget is spent, the rest
/// at the next polls ([`continue_model`] at the guest's reads of the invalidate trigger,
/// which report busy meanwhile); a shadow job in flight that may still map a page to move is
/// waited for as long as the pace allows, then at the next polls. A batch still pending is
/// continued first; nothing new is decided before it is done.
pub fn poll_sliced(fb: &mut FakeFb, gpu: &mut dyn PoolGpu, merge: bool, pace: Pace) -> bool {
    let t0 = Instant::now();
    let mut bd = PollBreakdown {
        merge,
        ..Default::default()
    };
    let mut synced = sync_inner(fb, merge, &mut bd);
    bd.sync_us = t0.elapsed().as_micros();
    // A batch an earlier poll decided: its next chunks first.
    let parked = fb.pool_mut().as_mut().and_then(|p| p.batch.take());
    if let Some(mut b) = parked {
        if !run_batch(fb, gpu, &mut b, pace) {
            park(fb, b, t0);
            return true;
        }
        settle(fb, *b, t0);
        synced = true;
    }
    let aged: Vec<u64>;
    let (root, full, path_only, retry, hazard) = {
        let mut guard = fb.pool_mut();
        let Some(p) = guard.as_mut() else {
            return synced;
        };
        p.stats.polls_unmerged += u64::from(!merge);
        let every = p.retry_every;
        let retry = (!p.pending.is_empty() || !p.waiting.is_empty())
            && p.last_retry.is_none_or(|t| t.elapsed() >= every);
        aged = p.aged(t0);
        if p.dirty.is_empty()
            && p.dir_dirty.is_empty()
            && !retry
            && !p.hazard_dirty
            && !p.recheck_replicas
            && aged.is_empty()
        {
            bd.total_us = t0.elapsed().as_micros();
            p.last = bd;
            return synced;
        }
        let dirty = std::mem::take(&mut p.dirty);
        let path_only: Vec<(usize, usize)> = std::mem::take(&mut p.dir_dirty)
            .into_iter()
            .filter(|u| !dirty.contains(u))
            .map(|u| (u, p.unit_pages(u)))
            .collect();
        let full: Vec<(usize, usize)> = dirty.into_iter().map(|u| (u, p.unit_pages(u))).collect();
        (p.root, full, path_only, retry, p.hazard_dirty)
    };
    bd.retry = retry;
    bd.hazard = hazard;
    let before = {
        let guard = fb.pool_mut();
        let p = guard.as_ref().expect("checked above");
        (p.pending.len(), p.waiting.len())
    };
    // The walk reads through the FB (and so through the pool borrow): outside it.
    let tw = Instant::now();
    // The copies of the table pages come out of the pool for the walk (the walk reads
    // through the FB, which borrows the pool) and go back after it.
    let mut cache = std::mem::take(&mut fb.pool_mut().as_mut().expect("on").pt_raw);
    let cached = cache.len();
    let mut walked: Vec<(usize, crate::fb::AreaUnitWalk)> = full
        .iter()
        .map(|&(u, n)| (u, fb.area_walk_unit_parts(root, u as u64 * UNIT, n, Some(&mut cache))))
        .collect();
    for &(u, n) in &path_only {
        let (dirs, pde) = fb.area_walk_path(root, u as u64 * UNIT, Some(&mut cache));
        let same = {
            let guard = fb.pool_mut();
            let s = &guard.as_ref().expect("on").units[u];
            s.dirs == dirs && s.pde == pde
        };
        if same {
            bd.units_path_only += 1;
            continue;
        }
        walked.push((u, fb.area_walk_unit_parts(root, u as u64 * UNIT, n, Some(&mut cache))));
    }
    bd.units_walked = walked.len();
    bd.pages_read = cache.len().saturating_sub(cached);
    bd.walk_us = tw.elapsed().as_micros();
    let td = Instant::now();
    let (releases, installs, stale_replicas) = {
        let mut guard = fb.pool_mut();
        let p = guard.as_mut().expect("checked above");
        p.stats.polls += 1;
        p.stats.units_walked += walked.len() as u64;
        p.stats.units_path_only += bd.units_path_only as u64;
        if retry {
            p.last_retry = Some(Instant::now());
            p.stats.retries += 1;
        }
        // The area pages to decide: those whose wanted mapping changed, the paced retries,
        // and the replicas (a replica's page may move in once it is no longer a table page
        // or its alias went; its admission check costs a few lookups).
        let mut cand: BTreeSet<u64> = BTreeSet::new();
        for (u, w) in walked {
            p.set_unit(u, w, &mut cand);
        }
        // Keep the copies of the pages the walk still reads.
        p.pt_raw = cache;
        let pu = &p.page_units;
        p.pt_raw.retain(|pg, _| pu.contains_key(pg));
        if retry {
            cand.extend(p.pending.keys().copied());
            cand.extend(p.waiting.iter().copied());
        }
        cand.extend(p.replicas.keys().copied());
        p.recheck_replicas = false;
        bd.candidates = cand.len();
        // The lazy move back (§13). An area page the guest unmapped keeps its page in the
        // pool, retired: the remap stays, so the device and the GPU still find the page at
        // its pool page, and nothing is copied (a BAR1 unmap on the real card is a PTE
        // write, not a copy). Mapped again at the same area page, it is simply live again.
        // It goes home (one copy, as every unmap did before) when another page or guest
        // RAM is wanted at its area page, when the guest maps it at another area page, or
        // after RETIRE_AGE.
        let now = Instant::now();
        let mut releases: Vec<Op> = Vec::new();
        for g in &cand {
            let Some(x) = p.table.holder(*g) else { continue };
            match p.want.get(g) {
                Some(Target::Fb(w)) if *w == x => {
                    if p.retired.remove(g).is_some() {
                        bd.relived += 1;
                    }
                }
                None => match p.retired.get(g) {
                    None => {
                        p.retired.insert(*g, now);
                        bd.retired += 1;
                    }
                    // A retired page whose move home was refused (a USERD, a busy barrier):
                    // the paced retry asks again once it is due.
                    Some(t) if p.waiting.contains(g) && now.duration_since(*t) >= RETIRE_AGE => {
                        releases.push(Op {
                            install: false,
                            g: *g,
                            x,
                            evict: false,
                            home: Some(Home::Aged),
                        })
                    }
                    Some(_) => {}
                },
                Some(_) => releases.push(Op {
                    install: false,
                    g: *g,
                    x,
                    evict: false,
                    home: p.retired.contains_key(g).then_some(Home::Slot),
                }),
            }
        }
        for g in &aged {
            if let Some(x) = p.table.holder(*g).filter(|_| p.retired.contains_key(g)) {
                if !releases.iter().any(|r| r.g == *g) {
                    releases.push(Op {
                        install: false,
                        g: *g,
                        x,
                        evict: false,
                        home: Some(Home::Aged),
                    });
                }
            }
        }
        p.stats.retires += bd.retired as u64;
        p.stats.relives += bd.relived as u64;
        p.churn.retires += bd.retired as u64;
        p.churn.relives += bd.relived as u64;
        // A table page the device tracks that lives in the pool goes back out (the rule
        // of `admit`, for the other order: the shadow started watching it after it moved).
        if hazard || retry {
            p.hazard_dirty = false;
            let pooled: Vec<(u64, u64)> = p.pooled_tables.iter().map(|(g, x)| (*g, *x)).collect();
            for (g, x) in pooled {
                if p.table.holder(g) != Some(x) || !tracked_table_page(fb, p, x >> 12) {
                    // Moved out already, or no walk reads it any more: an ordinary pooled
                    // page again.
                    p.pooled_tables.remove(&g);
                    continue;
                }
                if !releases.iter().any(|r| r.g == g) {
                    releases.push(Op {
                        install: false,
                        g,
                        x,
                        evict: true,
                        home: None,
                    });
                }
            }
        }
        let mut installs = Vec::new();
        let mut sys_lines = Vec::new();
        for g in &cand {
            match p.want.get(g).copied() {
                Some(Target::Fb(x)) if p.table.holder(*g) != Some(x) => {
                    // Retired at another area page: home first (in this poll), then here.
                    if let Some(g2) = p.table.slot_of(x).filter(|g2| *g2 != *g && p.retired.contains_key(g2)) {
                        if let Some(r) = releases.iter_mut().find(|r| r.g == g2) {
                            r.home = Some(Home::Relocate);
                        } else {
                            releases.push(Op {
                                install: false,
                                g: g2,
                                x,
                                evict: false,
                                home: Some(Home::Relocate),
                            });
                        }
                    }
                    installs.push(Op {
                        install: true,
                        g: *g,
                        x,
                        evict: false,
                        home: None,
                    })
                }
                Some(t @ Target::Sys(_)) => {
                    if p.reported.insert((*g, key(t))) {
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
        // Pending entries the guest no longer wants are dropped, and so are replicas; a
        // waiting release the guest took back (it maps the same page there again, so no
        // release or eviction is due) too. The releases due are settled after the moves.
        let want = &p.want;
        p.pending
            .retain(|g, (t, _)| !cand.contains(g) || want.get(g) == Some(t));
        let due: BTreeSet<u64> = releases.iter().map(|r| r.g).collect();
        p.waiting.retain(|g| !cand.contains(g) || due.contains(g));
        let stale_replicas: Vec<u64> = cand
            .iter()
            .filter(|g| {
                p.replicas
                    .get(g)
                    .is_some_and(|r| want.get(g) != Some(&Target::Fb(r.x)))
            })
            .copied()
            .collect();
        for l in sys_lines {
            p.finding(true, l);
        }
        (releases, installs, stale_replicas)
    };
    // A replica the guest no longer maps: the area's last writes go home, then it goes.
    for g in &stale_replicas {
        merge_replica(fb, *g, true);
    }
    bd.ops = releases.len() + installs.len();
    if releases.is_empty() && installs.is_empty() {
        bd.decide_us = td.elapsed().as_micros();
        pace_retry(fb, retry, before, 0);
        finish(fb, t0, bd);
        return true;
    }
    let mut refused: Vec<(Op, String)> = Vec::new();
    let mut replicate: Vec<(u64, u64, Refusal)> = Vec::new();
    let mut ops: Vec<Op> = Vec::new();
    // The static checks run against the table as it will be after the releases (each
    // install is checked again when it runs).
    {
        let guard = fb.pool_mut();
        let pool = guard.as_ref().expect("on");
        let released: BTreeSet<u64> = releases.iter().map(|r| r.g).collect();
        for op in releases.iter().chain(&installs) {
            if op.install {
                match admit_after(fb, pool, op.g, op.x, &released) {
                    Ok(()) => {}
                    // Served by its area page as a replica (made after the moves), or
                    // already.
                    Err(e) if e.replicates() => {
                        if pool.replicas.get(&op.g).is_none_or(|r| r.x != op.x) {
                            replicate.push((op.g, op.x, e));
                        }
                        continue;
                    }
                    Err(e) => {
                        refused.push((*op, e.why().into()));
                        continue;
                    }
                }
            }
            ops.push(*op);
        }
    }
    bd.decide_us = td.elapsed().as_micros();
    // Releases first (they free area pages and pages for the installs), then installs; by
    // home page within each half: pages of one FB store window move together (a move of a
    // page outside the store's windows maps one, §13).
    ops.sort_by_key(|o| (o.install, o.x));
    let mut b = Box::new(Batch {
        ops: ops.into(),
        releases,
        refused,
        replicate,
        notes: Vec::new(),
        retry,
        before,
        moved: 0,
        held: HashMap::new(),
        stamp: None,
        blocked: None,
        wait: None,
        stage: None,
        since: t0,
        slices: 0,
        bd,
    });
    if run_batch(fb, gpu, &mut b, pace) {
        settle(fb, *b, t0);
    } else {
        park(fb, b, t0);
    }
    true
}

/// A poll inside a trapped MMU invalidate (device.rs `after_bar0_write`) moves pages for
/// about this long per hold of the device lock; the rest follows at the guest's reads of
/// the trigger, which report busy until the last page moved (§14).
pub const POOL_SLICE: Duration = Duration::from_micros(500);
/// Pages per chunk of a sliced batch: one GPU check (when a shadow job was submitted since
/// the last), one barrier if a page of it is mapped by a shadow, the moves.
const CHUNK: usize = 16;
/// A batch pending this long runs all its moves at its next continuation, whatever the
/// budget, well inside the guest's poll of the trigger (4 s in RM, 2 s in nouveau). A wait
/// for a shadow job in flight stays bounded by the pace: the device lock is never held
/// waiting for the worker (execution.md §9).
pub const BATCH_FALLBACK: Duration = crate::invgate::FALLBACK;
/// How long one poll waits under the lock for a shadow job the batch waits for.
pub const WAIT_SLICE: Duration = crate::invgate::POLL_WAIT;

/// What holds a page of a batch on the GPU side (from [`PoolGpu::userd_refs`] and
/// [`PoolGpu::refs`]).
#[derive(Clone, Debug)]
enum Held {
    /// Nothing: the page moves without a barrier.
    Cold,
    /// Shadow roots that map it: it moves behind their barrier.
    Warm(BTreeSet<u64>),
    /// A mirrored channel's USERD: it does not move.
    Userd(u32),
}

/// The remap changes one poll decided, run in chunks (§14): with a budget, a chunk at a
/// time until the budget is spent, the rest at the next continuations; without one, all at
/// once. Every chunk leaves the pool consistent (each move is a copy and a remap switch
/// under the lock), so the device lock is free between two continuations.
pub struct Batch {
    /// The ops still to run, releases first, each half by home page.
    ops: VecDeque<Op>,
    /// Every release decided (a waiting release is settled at the end).
    releases: Vec<Op>,
    refused: Vec<(Op, String)>,
    /// Replicas to make at the end, when the releases freed their area pages.
    replicate: Vec<(u64, u64, Refusal)>,
    notes: Vec<String>,
    retry: bool,
    before: (usize, usize),
    moved: usize,
    /// The GPU side's answers for the pages of `ops`, valid while [`PoolGpu::stamp`] stays
    /// `stamp`.
    held: HashMap<u64, Held>,
    stamp: Option<u64>,
    blocked: Option<String>,
    /// What the batch waits for before its next chunk: a shadow job in flight that may
    /// still map a page of it, or a barrier's re-sync.
    wait: Option<Wait>,
    /// A chunk behind its barrier (§15).
    stage: Option<Stage>,
    /// The poll that decided the batch.
    since: Instant,
    slices: u32,
    bd: PollBreakdown,
}

/// What a batch waits for before its next chunk (never longer than the pace allows per poll).
#[derive(Clone, Debug)]
enum Wait {
    /// A shadow job in flight that may still map a page of the batch.
    InFlight(u64),
    /// A barrier's re-sync jobs; the worker releases the roots' rings after them.
    Resync(Vec<(u64, u64)>),
}

/// A chunk with pages a shadow maps, waiting for its barrier (§15).
struct Stage {
    /// The chunk's ops in order, and the ones a shadow maps.
    run: Vec<Op>,
    warm: Vec<Op>,
    barrier: Barrier,
}

/// Run what is left of batch `b` at `pace`: with a budget, until it is spent (a chunk at
/// least); whether the batch is done.
fn run_batch(fb: &mut FakeFb, gpu: &mut dyn PoolGpu, b: &mut Batch, pace: Pace) -> bool {
    let t = Instant::now();
    let budget = pace.budget.filter(|_| b.since.elapsed() < BATCH_FALLBACK);
    b.slices += 1;
    loop {
        if let Some(st) = b.stage.take() {
            // A chunk behind its barrier: its moves once the barrier holds.
            let tb = Instant::now();
            let ready = gpu.barrier_ready(fb, &st.barrier, pace.wait);
            b.bd.barrier_us += tb.elapsed().as_micros();
            match ready {
                Ok(false) => {
                    b.stage = Some(st);
                    return false;
                }
                Ok(true) => run_moves(fb, gpu, b, st.run, st.warm, Some(st.barrier)),
                Err(e) => refuse_barrier(fb, gpu, b, st, e),
            }
        } else if let Some(w) = b.wait.clone() {
            let tw = Instant::now();
            let applied = match &w {
                Wait::InFlight(s) => gpu.wait_applied(*s, pace.wait),
                Wait::Resync(jobs) => gpu.jobs_applied(jobs, pace.wait),
            };
            b.bd.in_flight_us += tw.elapsed().as_micros();
            if !applied {
                return false;
            }
            b.wait = None;
            continue;
        } else {
            if b.ops.is_empty() {
                return true;
            }
            // The GPU side's answers: again whenever a shadow job was submitted since (a
            // plan may have mapped a page of the batch, or taken one out of its walk).
            let stamp = gpu.stamp();
            if b.stamp != Some(stamp) {
                if let Err(s) = classify(fb, gpu, b) {
                    b.wait = Some(Wait::InFlight(s));
                    if let Some(p) = fb.pool_mut().as_mut() {
                        p.stats.in_flight_waits += 1;
                    }
                    continue;
                }
                b.stamp = Some(stamp);
            }
            let n = if budget.is_some() { CHUNK.min(b.ops.len()) } else { b.ops.len() };
            let chunk: Vec<Op> = b.ops.drain(..n).collect();
            run_chunk(fb, gpu, b, chunk);
            if b.stage.is_some() {
                // A barrier begun: see at once whether it holds (a bounded wait).
                continue;
            }
        }
        let left = !b.ops.is_empty() || b.stage.is_some() || b.wait.is_some();
        if left && budget.is_some_and(|bud| t.elapsed() >= bud) {
            return false;
        }
    }
}

/// Ask the GPU side who holds the pages of the batch: first the USERDs (a host channel
/// holds its address), then, when no shadow job in flight may still map one of the others
/// beyond the cached walks, the shadow leaves. `Err(seq)`: that job must be applied first.
fn classify(fb: &FakeFb, gpu: &mut dyn PoolGpu, b: &mut Batch) -> Result<(), u64> {
    b.held.clear();
    b.blocked = gpu.blocked();
    if b.blocked.is_some() {
        return Ok(());
    }
    let tu = Instant::now();
    let pages: BTreeSet<u64> = b.ops.iter().map(|o| o.x).collect();
    let userd = gpu.userd_refs(&pages);
    b.bd.userd_us += tu.elapsed().as_micros();
    let rest: BTreeSet<u64> = pages.iter().filter(|p| !userd.contains_key(p)).copied().collect();
    if let Some(s) = gpu.in_flight(fb, &rest) {
        return Err(s);
    }
    let tr = Instant::now();
    let refs = if rest.is_empty() {
        BTreeMap::new()
    } else {
        gpu.refs(fb, &rest)
    };
    b.bd.refs_us += tr.elapsed().as_micros();
    b.bd.refs_pages += rest.len();
    for p in pages {
        let r = refs.get(&p).cloned().unwrap_or_default();
        let h = match userd.get(&p).copied().or(r.userd) {
            Some(chid) => Held::Userd(chid),
            None if r.roots.is_empty() => Held::Cold,
            None => Held::Warm(r.roots),
        };
        b.held.insert(p, h);
    }
    Ok(())
}

/// One chunk of a batch: the pages nothing on the GPU maps move now; a chunk with pages a
/// shadow maps starts their barrier and waits for it as the batch's stage (§15).
fn run_chunk(fb: &mut FakeFb, gpu: &mut dyn PoolGpu, b: &mut Batch, chunk: Vec<Op>) {
    let mut warm: Vec<Op> = Vec::new();
    let mut roots: BTreeSet<u64> = BTreeSet::new();
    let mut run: Vec<Op> = Vec::new();
    for op in chunk {
        if let Some(why) = &b.blocked {
            b.refused.push((op, why.clone()));
            continue;
        }
        match b.held.get(&op.x) {
            Some(Held::Cold) => run.push(op),
            Some(Held::Warm(r)) => {
                roots.extend(r.iter().copied());
                warm.push(op);
                run.push(op);
            }
            Some(Held::Userd(chid)) => b.refused.push((
                op,
                format!("the page is the USERD of mirrored channel chid {chid}, whose host channel holds its address until it is freed"),
            )),
            None => b.refused.push((op, "the GPU side was not asked about the page (fail closed)".into())),
        }
    }
    if warm.is_empty() {
        run_moves(fb, gpu, b, run, warm, None);
        return;
    }
    let tb = Instant::now();
    let begun = gpu.begin_barrier(fb, &roots);
    b.bd.barrier_us += tb.elapsed().as_micros();
    match begun {
        Ok(barrier) => {
            b.stage = Some(Stage {
                run,
                warm,
                barrier,
            })
        }
        Err(e) => {
            let st = Stage {
                run,
                warm,
                barrier: Barrier {
                    roots,
                    ..Default::default()
                },
            };
            refuse_barrier(fb, gpu, b, st, e);
        }
    }
}

/// The moves of a chunk (`run` in order; `warm` the ones a shadow maps, whose barrier holds
/// when `barrier` is given), the replicas whose area page is free now, then the barrier's
/// end: the re-sync of its roots, whose rings the worker releases after it (§15).
fn run_moves(fb: &mut FakeFb, gpu: &mut dyn PoolGpu, b: &mut Batch, run: Vec<Op>, warm: Vec<Op>, barrier: Option<Barrier>) {
    let warm_op = |o: &Op| warm.iter().any(|w| w.g == o.g && w.x == o.x && w.install == o.install);
    let mut moved_warm = 0usize;
    let maps0 = fb.vram_stats().map_or(0, |s| s.maps);
    let mut installed_gs: Vec<u64> = Vec::new();
    let tm = Instant::now();
    for op in run {
        let is_warm = warm_op(&op);
        let (pool0, _) = fb.bar1_pool().expect("on");
        let p = pool0 + op.g;
        let tc = Instant::now();
        if op.install {
            let ok = {
                let guard = fb.pool_mut();
                let pool = guard.as_ref().expect("on");
                // The guest took the mapping back while the batch ran: nothing to install.
                if pool.want.get(&op.g) != Some(&Target::Fb(op.x)) {
                    continue;
                }
                admit(fb, pool, op.g, op.x)
            };
            if let Err(e) = ok {
                if e.replicates() {
                    b.replicate.push((op.g, op.x, e));
                } else {
                    b.refused.push((op, e.why().into()));
                }
                continue;
            }
            // A replica at this area page (the page was refused before and may move now,
            // or the area page served another page): its last area writes go home first.
            merge_replica(fb, op.g, true);
            // A display record window over the page goes before the page leaves home.
            fb.revoke_records(op.x / PAGE, op.x / PAGE + 1);
            fb.copy_page_raw(op.x, p);
            let mut guard = fb.pool_mut();
            let pool = guard.as_mut().expect("on");
            pool.table
                .install(op.g, op.x, pool.usable)
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
            installed_gs.push(op.g);
            b.moved += 1;
        } else {
            {
                let guard = fb.pool_mut();
                let pool = guard.as_ref().expect("on");
                // Moved out already, or the guest maps the page there again (re-lived
                // while the batch ran): nothing to release.
                if pool.table.holder(op.g) != Some(op.x)
                    || (!op.evict && pool.want.get(&op.g) == Some(&Target::Fb(op.x)))
                {
                    continue;
                }
            }
            b.moved += 1;
            fb.copy_page_raw(p, op.x);
            {
                let mut guard = fb.pool_mut();
                let pool = guard.as_mut().expect("on");
                pool.table.release(op.g);
                pool.pooled_tables.remove(&op.g);
                pool.waiting.remove(&op.g);
                pool.retired.remove(&op.g);
                match op.home {
                    Some(Home::Slot) => pool.stats.slot_evictions += 1,
                    Some(Home::Relocate) => pool.stats.relocations += 1,
                    Some(Home::Aged) => pool.stats.aged_out += 1,
                    None => {}
                }
                match op.home {
                    Some(Home::Slot) => pool.churn.slot_evictions += 1,
                    Some(Home::Relocate) => pool.churn.relocations += 1,
                    Some(Home::Aged) => pool.churn.aged_out += 1,
                    None => {}
                }
                pool.churn.releases += 1;
                pool.stats.bytes_out += PAGE;
                if is_warm {
                    pool.stats.releases_warm += 1;
                } else {
                    pool.stats.releases_cold += 1;
                }
                pool.stats.copy_us += tc.elapsed().as_micros();
            }
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
            if op.evict {
                // Back on the trapped path: the walk caches re-read the page (the guest may
                // have written it through the area since they read it), and the area page
                // becomes its replica.
                fb.note_pages_changed(&[op.x >> 12]);
                b.replicate.push((op.g, op.x, Refusal::TablePage));
                let mut guard = fb.pool_mut();
                let pool = guard.as_mut().expect("on");
                pool.stats.table_evictions += 1;
                let first = pool.hazards.insert(op.x >> 12);
                pool.stats.table_hazards += u64::from(first);
                pool.reported.insert((op.g, key(Target::Fb(op.x))));
                pool.finding(
                    false,
                    format!(
                        "guest BAR1 {:#x} maps FB {:#x}, a page-table page the shadow watches: moved out of the pool back to the trapped path{} (fail closed); the shadow re-reads the page, and area page {:#x} mirrors it as a replica merged at every sync point",
                        op.g,
                        op.x,
                        if is_warm { " behind the barrier" } else { "" },
                        op.g
                    ),
                );
            }
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
    let move_us = tm.elapsed().as_micros();
    b.bd.move_us += move_us;
    b.bd.moved = b.moved;
    let maps = fb.vram_stats().map_or(0, |s| s.maps).saturating_sub(maps0);
    b.bd.window_maps += maps;
    {
        let mut guard = fb.pool_mut();
        let pool = guard.as_mut().expect("on");
        pool.stats.move_window_maps += maps;
        pool.churn.window_maps += maps;
        pool.churn.move_us += move_us;
        pool.churn.installs += installed_gs.len() as u64;
        pool.churn.note_runs(&mut installed_gs);
    }
    // The replicas whose area page is free now (an evicted table page's among them: its
    // area page mirrors it from this hold on, not only at the end of the batch).
    let list = std::mem::take(&mut b.replicate);
    b.replicate = make_replicas(fb, list);
    fb.flush();
    let Some(bar) = barrier else { return };
    let ntsgs = bar.ntsgs;
    let nchannels = bar.nchannels;
    let roots = bar.roots.clone();
    fb.pool_mut().as_mut().expect("on").stats.preempts += ntsgs as u64;
    if moved_warm == 0 {
        gpu.cancel_barrier(bar);
        return;
    }
    {
        let mut guard = fb.pool_mut();
        let pool = guard.as_mut().expect("on");
        pool.rediff.extend(roots.iter().copied());
        pool.stats.barriers += 1;
    }
    crate::profile::window("bar1 pool: warm-move barriers (units: pages)", moved_warm as u64);
    let ts = Instant::now();
    let (notes, jobs) = gpu.end_barrier(fb, bar);
    b.notes.extend(notes);
    b.bd.resync_us += ts.elapsed().as_micros();
    let after = if jobs.is_empty() {
        "the worker drained"
    } else {
        "their rings held until the worker applied the re-sync"
    };
    if !jobs.is_empty() {
        b.wait = Some(Wait::Resync(jobs));
    }
    let line = format!(
        "{moved_warm} page(s) the GPU maps moved behind the barrier ({nchannels} host channel(s) idle, {ntsgs} TSG(s) preempted); shadow(s) of root(s) {roots:x?} re-synced, {after}, before the guest continues"
    );
    let mut guard = fb.pool_mut();
    guard.as_mut().expect("on").finding(false, line);
}

/// A barrier that failed: its rings released, the pages the GPU maps refused (retried), the
/// others of the chunk moved.
fn refuse_barrier(fb: &mut FakeFb, gpu: &mut dyn PoolGpu, b: &mut Batch, st: Stage, e: String) {
    let roots = st.barrier.roots.clone();
    gpu.cancel_barrier(st.barrier);
    for op in &st.warm {
        b.refused.push((*op, format!("the GPU may be using it and the barrier failed: {e}")));
    }
    {
        let mut guard = fb.pool_mut();
        let pool = guard.as_mut().expect("on");
        pool.stats.barriers_refused += 1;
        crate::profile::window("bar1 pool: barriers refused", 0);
        // Every refusal, with the wall time, for the reader of the host kernel log (a
        // refused barrier moves nothing; the pages stay and are retried).
        eprintln!(
            "vfio-user-nvgpu: bar1 pool: at {} barrier {} refused for {} page(s) of root(s) {roots:x?} (first g {:#x} FB {:#x}): {e}; nothing moved, retried",
            crate::untrapped::wall(),
            pool.stats.barriers_refused,
            st.warm.len(),
            st.warm[0].g,
            st.warm[0].x
        );
    }
    let is_warm = |o: &Op| st.warm.iter().any(|w| w.g == o.g && w.x == o.x && w.install == o.install);
    let cold: Vec<Op> = st.run.iter().filter(|o| !is_warm(o)).copied().collect();
    run_moves(fb, gpu, b, cold, Vec::new(), None);
}

/// Make the replicas of `list` whose area page is free now; the others are returned (an
/// area page a later chunk's release frees).
fn make_replicas(fb: &mut FakeFb, list: Vec<(u64, u64, Refusal)>) -> Vec<(u64, u64, Refusal)> {
    let mut later = Vec::new();
    for (g, x, why) in list {
        let (held, free, first) = {
            let guard = fb.pool_mut();
            let pool = guard.as_ref().expect("on");
            (
                pool.table.holder(g).is_some(),
                pool.replicas.get(&g).is_none_or(|r| r.x != x),
                !pool.reported.contains(&(g, key(Target::Fb(x)))),
            )
        };
        if held {
            later.push((g, x, why));
            continue;
        }
        if !free {
            continue;
        }
        merge_replica(fb, g, true);
        make_replica(fb, g, x);
        let mut guard = fb.pool_mut();
        let pool = guard.as_mut().expect("on");
        pool.pending.remove(&g);
        if !first {
            continue;
        }
        pool.reported.insert((g, key(Target::Fb(x))));
        pool.stats.refused += 1;
        let line = match why {
            Refusal::TablePage => {
                pool.stats.table_refusals += 1;
                pool.stats.table_hazards += u64::from(pool.hazards.insert(x >> 12));
                format!(
                    "guest BAR1 {g:#x} maps FB {x:#x}: remap refused ({}); the page stays on the trapped path (fail closed) and area page {g:#x} mirrors it as a replica merged at every sync point",
                    why.why()
                )
            }
            _ => format!(
                "guest BAR1 {g:#x} maps FB {x:#x}: remap refused ({}); the page stays where it is (fail closed) and area page {g:#x} mirrors it as a replica merged at every sync point, so guest writes through either mapping reach the page",
                why.why()
            ),
        };
        pool.finding(true, line);
    }
    later
}

/// A batch left for the next continuation: the hold so far counts as a poll's.
fn park(fb: &mut FakeFb, mut b: Box<Batch>, t_hold: Instant) {
    let mut guard = fb.pool_mut();
    let Some(p) = guard.as_mut() else { return };
    let hold = t_hold.elapsed().as_micros();
    p.stats.max_poll_us = p.stats.max_poll_us.max(hold);
    p.stats.slices_parked += 1;
    if matches!(b.wait, Some(Wait::InFlight(_))) {
        p.stats.in_flight_parked += 1;
    }
    if b.stage.is_some() || matches!(b.wait, Some(Wait::Resync(_))) {
        p.stats.barrier_parked += 1;
    }
    b.bd.total_us = hold;
    b.bd.slices = b.slices;
    p.last = b.bd.clone();
    p.batch = Some(b);
}

/// The end of a batch: the replicas still due (an area page that is still held keeps none),
/// the releases and refusals settled, the retry paced, the poll counted.
fn settle(fb: &mut FakeFb, b: Batch, t_hold: Instant) {
    let Batch {
        releases,
        refused,
        replicate,
        notes,
        retry,
        before,
        moved,
        since,
        slices,
        mut bd,
        ..
    } = b;
    make_replicas(fb, replicate);
    fb.flush();
    {
        let mut guard = fb.pool_mut();
        let pool = guard.as_mut().expect("on");
        pool.stats.refs_us += bd.refs_us;
        pool.stats.barrier_us += bd.barrier_us;
        pool.stats.move_us += bd.move_us;
        pool.stats.resync_us += bd.resync_us;
        pool.stats.in_flight_wait_us += bd.in_flight_us;
        if slices > 1 {
            pool.stats.sliced_batches += 1;
            pool.stats.batch_max_us = pool.stats.batch_max_us.max(since.elapsed().as_micros());
        }
        // Every release tried in this batch is settled here: done, or refused again.
        for r in &releases {
            pool.waiting.remove(&r.g);
        }
        for (op, why) in refused {
            if !op.install {
                // A release that cannot happen keeps the remap: the page stays consistent
                // at its pool page; only the area page cannot take a new page yet. An
                // eviction that cannot happen keeps the page marked changed at every sync
                // point (`sync_untracked`) until it can.
                pool.waiting.insert(op.g);
                if op.evict {
                    // Seen: the sync points stop asking for an immediate eviction, the
                    // paced retry takes it (also when the release of the same page was
                    // refused and logged before, e.g. for a USERD).
                    pool.stats.table_hazards += u64::from(pool.hazards.insert(op.x >> 12));
                }
                if pool.reported_release.insert((op.g, op.x)) {
                    pool.stats.refused += 1;
                    let line = if op.evict {
                        format!(
                            "guest BAR1 {:#x} maps FB {:#x}, a page-table page the shadow watches, but the page cannot move back out of its pool page {:#x} yet: {why}; until it can, the device marks it changed at every sync point (the shadow re-reads it from the pool page) and retries",
                            op.g, op.x, pool.table.pool_page(op.g)
                        )
                    } else {
                        format!(
                            "guest BAR1 {:#x} no longer maps FB {:#x}, but the page cannot move back from its pool page {:#x} yet: {why}; it stays in the pool (consistent) and is retried",
                            op.g, op.x, pool.table.pool_page(op.g)
                        )
                    };
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
    pace_retry(fb, retry, before, moved);
    bd.slices = slices;
    finish(fb, t_hold, bd);
}

/// After a poll that retried refused remaps: back to [`RETRY_EVERY`] when it made progress
/// (a page moved, a pending remap or a waiting release went away), else the interval
/// doubles up to [`RETRY_MAX`]. `before` is `(pending remaps, waiting releases)` at the
/// start of the poll. A new dirty unit is walked at the next poll whatever the interval.
fn pace_retry(fb: &FakeFb, retry: bool, before: (usize, usize), moved: usize) {
    if !retry {
        return;
    }
    let mut guard = fb.pool_mut();
    let Some(p) = guard.as_mut() else { return };
    let progress = moved > 0 || p.pending.len() < before.0 || p.waiting.len() < before.1;
    let waiting = !p.pending.is_empty() || !p.waiting.is_empty();
    if progress || !waiting {
        p.retry_every = RETRY_EVERY;
    } else {
        p.retry_every = (p.retry_every * 2).min(RETRY_MAX);
        p.stats.retry_backoffs += 1;
    }
}

/// A hashable key of a page target (`Target` has no `Hash`): sysmem with bit 63.
fn key(t: Target) -> u64 {
    match t {
        Target::Fb(pa) => pa,
        Target::Sys(pa) => pa | 1 << 63,
        _ => u64::MAX,
    }
}

/// The end of a poll that did work: the slowest-poll counter and the breakdown.
fn finish(fb: &FakeFb, t0: Instant, mut bd: PollBreakdown) {
    if let Some(p) = fb.pool_mut().as_mut() {
        bd.total_us = t0.elapsed().as_micros();
        p.stats.max_poll_us = p.stats.max_poll_us.max(bd.total_us);
        if bd.ops > 0 || bd.retired > 0 || bd.relived > 0 {
            p.churn.since.get_or_insert(t0);
            p.churn.polls += 1;
            p.churn.max_poll_us = p.churn.max_poll_us.max(bd.total_us);
        }
        p.last = bd;
    }
}

#[cfg(test)]
mod floor_tests {
    use super::*;

    /// The host BAR1 floor: parsed from nvidia-smi's MEMORY block, refused below the
    /// floor and when the free size is unknown, skipped at floor 0.
    #[test]
    fn host_bar1_floor_fails_closed() {
        let out = "GPU 00000000:2D:00.0\n    FB Memory Usage\n        Total : 8192 MiB\n        Used : 4223 MiB\n        Free : 3556 MiB\n    BAR1 Memory Usage\n        Total                                          : 256 MiB\n        Used                                           : 165 MiB\n        Free                                           : 91 MiB\n";
        assert_eq!(parse_smi_bar1(out), Some((256 * MIB, 165 * MIB, 91 * MIB)));
        assert_eq!(parse_smi_bar1("no such block"), None);
        assert_eq!(host_bar1_floor(None), Ok(96 * MIB));
        assert_eq!(host_bar1_floor(Some("0")), Ok(0));
        assert!(host_bar1_floor(Some("lots")).is_err());
        // The morning's case: 91 MiB free, a 32 MiB pool with 64 MiB of windows.
        let e = check_host_bar1(96 * MIB, Ok(91 * MIB), 96 * MIB).unwrap_err();
        assert!(e.contains("need 96 MiB, the host has 91 MiB free and must keep 96 MiB"), "{e}");
        assert!(check_host_bar1(96 * MIB, Ok(192 * MIB), 96 * MIB).is_ok());
        assert!(check_host_bar1(96 * MIB, Err("x".into()), 96 * MIB).is_err());
        assert!(check_host_bar1(96 * MIB, Err("x".into()), 0).is_ok());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The knob: 32 MiB with host execution unless set, 0 for none, nothing without host
    /// execution; a malformed or out-of-range size is refused.
    #[test]
    fn the_pool_knob() {
        assert_eq!(parse_pool(None, true), Ok(Some(32 * MIB)));
        assert_eq!(parse_pool(Some(" "), true), Ok(Some(32 * MIB)));
        assert_eq!(parse_pool(None, false), Ok(None));
        assert_eq!(parse_pool(Some("0"), true), Ok(None));
        assert_eq!(parse_pool(Some("0"), false), Ok(None));
        assert_eq!(parse_pool(Some("64"), true), Ok(Some(64 * MIB)));
        assert!(parse_pool(Some("64"), false).unwrap_err().contains("host execution"));
        assert!(parse_pool(Some("3"), true).is_err());
        assert!(parse_pool(Some("130"), true).is_err());
        assert!(parse_pool(Some("pool"), true).is_err());
    }

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
        /// Calls of `refs` (the leaf scan) and of `userd_refs`.
        refs_calls: std::cell::Cell<usize>,
        userd_calls: std::cell::Cell<usize>,
        /// Shadow jobs in flight that took a page out of a walk (page -> job), the last job
        /// applied, the bounded waits that time out before a job is applied, every wait,
        /// and the stamp (a shadow job submitted).
        in_flight: BTreeMap<u64, u64>,
        done: std::cell::Cell<u64>,
        slow_waits: std::cell::Cell<u32>,
        waits: std::cell::RefCell<Vec<(u64, Option<Duration>)>>,
        stamp: std::cell::Cell<u64>,
        /// §15: the barrier as held rings and worker jobs: the roots held now, the polls
        /// its preempts and its re-sync stay unapplied (a bounded wait), a failure, and the
        /// barriers cancelled.
        async_barrier: bool,
        holds: std::cell::RefCell<BTreeSet<u64>>,
        barrier_polls: std::cell::Cell<u32>,
        resync_polls: std::cell::Cell<u32>,
        barrier_fail: Option<String>,
        cancelled: usize,
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
                refs_calls: std::cell::Cell::new(0),
                userd_calls: std::cell::Cell::new(0),
                in_flight: BTreeMap::new(),
                done: std::cell::Cell::new(0),
                slow_waits: std::cell::Cell::new(0),
                waits: std::cell::RefCell::new(Vec::new()),
                stamp: std::cell::Cell::new(0),
                async_barrier: false,
                holds: std::cell::RefCell::new(BTreeSet::new()),
                barrier_polls: std::cell::Cell::new(0),
                resync_polls: std::cell::Cell::new(0),
                barrier_fail: None,
                cancelled: 0,
            }
        }
    }

    impl PoolGpu for FakeGpu {
        fn blocked(&self) -> Option<String> {
            self.blocked.then(|| "a direct walker".into())
        }
        fn userd_refs(&self, pages: &BTreeSet<u64>) -> BTreeMap<u64, u32> {
            self.userd_calls.set(self.userd_calls.get() + 1);
            pages.iter().filter(|p| self.userd.contains(p)).map(|p| (*p, 7)).collect()
        }
        fn refs(&self, _fb: &FakeFb, pages: &BTreeSet<u64>) -> BTreeMap<u64, Refs> {
            self.refs_calls.set(self.refs_calls.get() + 1);
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
        fn in_flight(&self, _fb: &FakeFb, pages: &BTreeSet<u64>) -> Option<u64> {
            let done = self.done.get();
            self.in_flight
                .iter()
                .filter(|(p, s)| **s > done && pages.contains(p))
                .map(|(_, s)| *s)
                .max()
        }
        fn wait_applied(&self, seq: u64, max: Option<Duration>) -> bool {
            self.waits.borrow_mut().push((seq, max));
            if max.is_some() && self.slow_waits.get() > 0 {
                self.slow_waits.set(self.slow_waits.get() - 1);
                return false;
            }
            self.done.set(self.done.get().max(seq));
            true
        }
        fn stamp(&self) -> u64 {
            self.stamp.get()
        }
        fn begin_barrier(&mut self, fb: &mut FakeFb, roots: &BTreeSet<u64>) -> Result<Barrier, String> {
            let (nchannels, ntsgs) = self.quiesce(fb, roots)?;
            let mut b = Barrier {
                roots: roots.clone(),
                nchannels,
                ntsgs,
                ..Default::default()
            };
            if self.async_barrier {
                self.holds.borrow_mut().extend(roots.iter().copied());
                b.tokens = roots.iter().map(|r| (*r, 1)).collect();
                b.jobs = vec![(self.root, 50)];
            }
            Ok(b)
        }
        fn barrier_ready(&mut self, _fb: &mut FakeFb, b: &Barrier, max: Option<Duration>) -> Result<bool, String> {
            if b.tokens.is_empty() {
                return Ok(true);
            }
            if let Some(e) = &self.barrier_fail {
                return Err(e.clone());
            }
            if max.is_some() && self.barrier_polls.get() > 0 {
                self.barrier_polls.set(self.barrier_polls.get() - 1);
                return Ok(false);
            }
            Ok(true)
        }
        fn end_barrier(&mut self, fb: &mut FakeFb, b: Barrier) -> (Vec<String>, Vec<(u64, u64)>) {
            let notes = self.resync(fb, &b.roots);
            if b.tokens.is_empty() {
                return (notes, Vec::new());
            }
            (notes, vec![(self.root, 60)])
        }
        fn cancel_barrier(&mut self, b: Barrier) {
            if !b.tokens.is_empty() {
                self.cancelled += 1;
                self.holds.borrow_mut().clear();
            }
        }
        fn jobs_applied(&self, _jobs: &[(u64, u64)], max: Option<Duration>) -> bool {
            if max.is_some() && self.resync_polls.get() > 0 {
                self.resync_polls.set(self.resync_polls.get() - 1);
                return false;
            }
            // The worker released the rings after the re-sync.
            self.holds.borrow_mut().clear();
            true
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

    /// Make every retired page due for its move home at the next poll (§13).
    fn age_retired(fb: &FakeFb) {
        let mut g = fb.pool_mut();
        let p = g.as_mut().unwrap();
        let old = Instant::now() - RETIRE_AGE - Duration::from_millis(1);
        for t in p.retired.values_mut() {
            *t = old;
        }
        p.last_age_scan = None;
    }

    fn retired(fb: &FakeFb) -> usize {
        fb.pool_mut().as_ref().unwrap().retired_now()
    }

    fn expire_retry(fb: &FakeFb) {
        fb.pool_mut().as_mut().unwrap().last_retry = None;
    }

    fn replicas(fb: &FakeFb) -> usize {
        fb.pool_mut().as_ref().unwrap().replicas.len()
    }

    /// The retry is not due for a while (the interval at its bound, just retried).
    fn hold_retry(fb: &FakeFb) {
        let mut g = fb.pool_mut();
        let p = g.as_mut().unwrap();
        p.last_retry = Some(Instant::now());
        p.retry_every = RETRY_MAX;
    }

    fn last(fb: &FakeFb) -> PollBreakdown {
        fb.pool_mut().as_ref().unwrap().last.clone()
    }

    /// A guest GMMU tree (a CUDA context's) at `BASE + 1 MiB` whose small page table is
    /// `BASE + 1 MiB + 16 KiB`, leaf 3 on `BASE + 4 MiB`, and the shadow's cached walk of
    /// it: the root and the page table.
    fn context_tree(fb: &mut FakeFb) -> (Target, u64) {
        let (croot, cpd2, cpd1, cpd0, cpt) = (
            BASE + 0x10_0000,
            BASE + 0x10_1000,
            BASE + 0x10_2000,
            BASE + 0x10_3000,
            BASE + 0x10_4000,
        );
        fb.write_u64(croot, vid(cpd2));
        fb.write_u64(cpd2, vid(cpd1));
        fb.write_u64(cpd1, vid(cpd0));
        fb.write_u64(cpd0 + 8, vid(cpt));
        fb.write_u64(cpt + 8 * 3, pte(BASE + 0x40_0000));
        let root = Target::Fb(croot);
        fb.walk_update(None, root, false, 1 << 20);
        (root, cpt)
    }

    fn leaf_at(fb: &FakeFb, root: Target, va: u64) -> Option<Target> {
        fb.with_walk(root, |c| c.all_leaves())
            .unwrap()
            .into_iter()
            .find(|l| l.0 == va)
            .map(|l| l.1)
    }

    /// The vm4-615 black stream (docs/caraxes/bar1-pool.md §10), first order: the shadow
    /// watches a guest page table, then the guest maps it into the area. The page is never
    /// moved into the pool (it stays on the trapped path); the area page becomes its
    /// replica, and the sync points carry the guest's area writes to the page as tracked
    /// writes (the shadow's walk sees them) and the device's writes back to the area.
    #[test]
    fn a_table_page_the_shadow_watches_is_refused_and_served_by_a_replica() {
        let (mut fb, pt, pool0) = setup();
        assert!(poll(&mut fb, &mut NoGpu));
        let (root, cpt) = context_tree(&mut fb);
        assert!(fb.watches_table_page(cpt >> 12));
        fb.write_u64(pt + 8 * 5, pte(cpt));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(fb.remap(cpt), cpt, "a watched table page never moves into the pool");
        let st = stats(&fb);
        assert_eq!(
            (st.table_refusals, st.table_hazards, st.table_evictions, st.replicas, st.refused),
            (1, 1, 0, 1, 1)
        );
        assert_eq!((pending(&fb), replicas(&fb)), (0, 1));
        assert!(st.first.iter().any(|l| l.contains("stays on the trapped path")));
        // The area page holds the page's bytes.
        let p = pool0 + 0x5000;
        let mut w = [0u8; 8];
        fb.read_raw(p + 8 * 3, &mut w);
        assert_eq!(u64::from_le_bytes(w), pte(BASE + 0x40_0000));
        // The guest writes a PTE through the area (no trap), then invalidates: the sync
        // point writes it to the page as a tracked write, and the walk sees the leaf.
        fb.write_raw(p + 8 * 4, &pte(BASE + 0x41_0000).to_le_bytes());
        assert!(!fb.tables_dirty(root));
        assert!(sync_untracked(&mut fb));
        assert_eq!(fb.read_u64(cpt + 8 * 4), Some(pte(BASE + 0x41_0000)));
        assert!(fb.tables_dirty(root));
        fb.walk_update(None, root, false, 1 << 20);
        assert_eq!(leaf_at(&fb, root, 0x4000), Some(Target::Fb(BASE + 0x41_0000)));
        // A device write of the page (BAR2, PRAMIN) reaches the area at the next sync.
        fb.write_u64(cpt + 8 * 6, pte(BASE + 0x42_0000));
        assert!(sync_untracked(&mut fb));
        fb.read_raw(p + 8 * 6, &mut w);
        assert_eq!(u64::from_le_bytes(w), pte(BASE + 0x42_0000));
        // Nothing changed: a sync merges and writes nothing.
        assert!(!sync_untracked(&mut fb));
        let st = stats(&fb);
        assert_eq!((st.replica_to_home, st.replica_to_area, st.replica_conflicts), (1, 1, 0));
        // Later polls leave the replica alone and log nothing new.
        fb.write_u64(pt + 8 * 7, pte(BASE + 0x40_0000));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!((stats(&fb).refused, stats(&fb).replicas, replicas(&fb)), (1, 1, 1));
        // The guest unmaps the area page: its last area write goes to the page, the
        // replica goes.
        fb.write_raw(p + 8 * 7, &pte(BASE + 0x43_0000).to_le_bytes());
        fb.write_u64(pt + 8 * 5, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(replicas(&fb), 0);
        assert_eq!(fb.read_u64(cpt + 8 * 7), Some(pte(BASE + 0x43_0000)));
        fb.walk_update(None, root, false, 1 << 20);
        assert_eq!(leaf_at(&fb, root, 0x7000), Some(Target::Fb(BASE + 0x43_0000)));
    }

    /// The other order: the page is in the pool (the guest mapped it into the area as
    /// data), then it becomes a page table the shadow walks. Until the next poll, every
    /// sync point marks it changed, so the walk re-reads it from its pool page; the poll
    /// evicts it (behind the barrier when the GPU maps it, refused and retried while the
    /// barrier fails), the walk re-reads it, and the area page becomes its replica. Once
    /// no walk reads it any more, it moves back into the pool.
    #[test]
    fn a_pooled_page_that_becomes_a_table_page_is_evicted_to_the_trapped_path() {
        let (mut fb, pt, pool0) = setup();
        assert!(poll(&mut fb, &mut NoGpu));
        let cpt = BASE + 0x10_4000;
        fb.write_u64(cpt + 8 * 3, pte(BASE + 0x40_0000));
        fb.write_u64(pt + 8 * 5, pte(cpt));
        assert!(poll(&mut fb, &mut NoGpu));
        let p = pool0 + 0x5000;
        assert_eq!(fb.remap(cpt), p);
        // The guest RM makes the page a page table; the shadow walks it at its pool page.
        let (root, _) = context_tree(&mut fb);
        assert_eq!(leaf_at(&fb, root, 0x3000), Some(Target::Fb(BASE + 0x40_0000)));
        // Before any poll the guest writes a PTE through the area and invalidates: the
        // sync point marks the page changed and the walk re-reads it from the pool page.
        fb.write_raw(p + 8 * 4, &pte(BASE + 0x41_0000).to_le_bytes());
        assert!(!fb.tables_dirty(root));
        assert!(sync_untracked(&mut fb));
        assert!(fb.tables_dirty(root));
        assert_eq!(stats(&fb).table_resyncs, 1);
        fb.walk_update(None, root, false, 1 << 20);
        assert_eq!(leaf_at(&fb, root, 0x4000), Some(Target::Fb(BASE + 0x41_0000)));
        // The GPU maps the page and the barrier fails: the eviction is refused, the page
        // stays in the pool (consistent), and the sync points keep marking it.
        let mut gpu = FakeGpu::new(0x1234_0000);
        gpu.used.insert(cpt);
        gpu.ok = false;
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(cpt), p);
        let st = stats(&fb);
        assert_eq!((st.barriers_refused, st.table_evictions, st.table_hazards), (1, 0, 1));
        assert!(st.first.iter().any(|l| l.contains("cannot move back out of its pool page")));
        let before = stats(&fb).table_resyncs;
        assert!(sync_untracked(&mut fb));
        assert_eq!(stats(&fb).table_resyncs, before + 1);
        // The barrier holds at the retry: evicted behind it, the root re-synced.
        gpu.ok = true;
        expire_retry(&fb);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(cpt), cpt);
        let st = stats(&fb);
        assert_eq!(
            (st.table_evictions, st.releases_warm, st.barriers, st.replicas, st.table_hazards),
            (1, 1, 1, 1, 1)
        );
        assert_eq!(gpu.resynced, vec![0x1234_0000]);
        assert!(st
            .first
            .iter()
            .any(|l| l.contains("moved out of the pool back to the trapped path behind the barrier")));
        // The page has every byte, and the walk re-reads it.
        assert_eq!(fb.read_u64(cpt + 8 * 4), Some(pte(BASE + 0x41_0000)));
        assert!(fb.tables_dirty(root));
        // From now on the area page is its replica: area writes reach the walk.
        fb.write_raw(p + 8 * 5, &pte(BASE + 0x44_0000).to_le_bytes());
        let before = stats(&fb).table_resyncs;
        assert!(sync_untracked(&mut fb));
        assert_eq!(stats(&fb).table_resyncs, before, "nothing pooled to mark any more");
        fb.walk_update(None, root, false, 1 << 20);
        assert_eq!(leaf_at(&fb, root, 0x5000), Some(Target::Fb(BASE + 0x44_0000)));
        // The context goes (no walk reads the page): it moves back into the pool.
        gpu.used.clear();
        fb.unwatch_tables(root);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(cpt), p);
        assert_eq!(replicas(&fb), 0);
        let mut w = [0u8; 8];
        fb.read_raw(p + 8 * 5, &mut w);
        assert_eq!(u64::from_le_bytes(w), pte(BASE + 0x44_0000));
    }

    /// The lazy move back (§13, Windows' BAR1 churn): a page the guest unmaps stays in the
    /// pool, retired, and mapped again at the same area page it is live again: no copy
    /// either way, as a BAR1 remap costs no copy on the real card. Every path keeps finding
    /// it at its pool page meanwhile.
    #[test]
    fn a_page_mapped_again_at_its_area_page_costs_no_copy() {
        let (mut fb, pt, pool0) = setup();
        let x = BASE + 0x10_0000;
        fb.write(x, &[1; 8]);
        assert!(poll(&mut fb, &mut NoGpu));
        let p = pool0 + 0x5000;
        for round in 0..5 {
            fb.write_u64(pt + 8 * 5, pte(x));
            assert!(poll(&mut fb, &mut NoGpu));
            assert_eq!((fb.remap(x), retired(&fb)), (p, 0), "round {round}");
            fb.write_u64(pt + 8 * 5, 0);
            assert!(poll(&mut fb, &mut NoGpu));
            assert_eq!((fb.remap(x), retired(&fb)), (p, 1), "round {round}");
            // The device and the GPU still find the page at its pool page.
            fb.write(x, &[round as u8 + 2; 8]);
            let mut b = [0u8; 8];
            fb.read_raw(p, &mut b);
            assert_eq!(b, [round as u8 + 2; 8]);
        }
        let st = stats(&fb);
        assert_eq!((st.installs_cold, st.bytes_in, st.releases_cold, st.bytes_out), (1, PAGE, 0, 0));
        assert_eq!((st.retires, st.relives), (5, 4));
        assert!(fb.bar1_pool_summary().unwrap().contains("1 retired now, 5 retired in all"));
        // The pool counts the page as not mapped by the guest ("pool empty" is about the
        // guest's mappings).
        assert_eq!(fb.pool_mut().as_ref().unwrap().live(), 0);
        // The churn line (the trace, once per second with activity).
        {
            let mut g = fb.pool_mut();
            let pl = g.as_mut().unwrap();
            assert!(pl.take_churn().is_none(), "not a second yet");
            pl.churn.since = Some(Instant::now() - CHURN_EVERY);
            let line = pl.take_churn().unwrap();
            assert!(line.contains("retired 5 (no copy), re-lived 4 (no copy)"), "{line}");
            assert!(line.contains("moved in 1 and home 0 page(s)"), "{line}");
            assert!(pl.take_churn().is_none(), "reset after the line");
        }
    }

    /// Another page wanted at a retired page's area page: the retired page goes home first
    /// with every byte written meanwhile (one copy, as every unmap did before), then the new
    /// page moves in. A retired page the guest maps at another area page goes home and in
    /// there; one that ages out goes home.
    #[test]
    fn a_retired_page_goes_home_when_its_area_page_is_needed_or_it_moves_or_ages() {
        let (mut fb, pt, pool0) = setup();
        assert!(poll(&mut fb, &mut NoGpu));
        let (x, y, z) = (BASE + 0x10_0000, BASE + 0x11_0000, BASE + 0x12_0000);
        fb.write_u64(pt + 8 * 5, pte(x));
        assert!(poll(&mut fb, &mut NoGpu));
        fb.write_u64(pt + 8 * 5, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        fb.write_u64(x + 8, 0x7777);
        // y at the same area page: x home with its bytes, y in.
        fb.write_u64(pt + 8 * 5, pte(y));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!((fb.remap(x), fb.remap(y), retired(&fb)), (x, pool0 + 0x5000, 0));
        let mut w = [0u8; 8];
        fb.read_raw(x + 8, &mut w);
        assert_eq!(u64::from_le_bytes(w), 0x7777, "the retired page's bytes went home");
        assert_eq!(stats(&fb).slot_evictions, 1);
        // y unmapped (retired), then mapped at another area page: home first, then in there,
        // in the same poll.
        fb.write_u64(pt + 8 * 5, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        fb.write_u64(y, 0x8888);
        fb.write_u64(pt + 8 * 7, pte(y));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!((fb.remap(y), retired(&fb)), (pool0 + 0x7000, 0));
        assert_eq!(fb.read_u64(y), Some(0x8888));
        fb.read_raw(pool0 + 0x7000, &mut w);
        assert_eq!(u64::from_le_bytes(w), 0x8888);
        assert_eq!(stats(&fb).relocations, 1);
        // z mapped and unmapped: it goes home only once it is RETIRE_AGE old.
        fb.write_u64(pt + 8 * 9, pte(z));
        assert!(poll(&mut fb, &mut NoGpu));
        fb.write_u64(pt + 8 * 9, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        assert!(!poll(&mut fb, &mut NoGpu), "not due yet: the poll stays idle");
        assert_eq!(fb.remap(z), pool0 + 0x9000);
        age_retired(&fb);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!((fb.remap(z), stats(&fb).aged_out), (z, 1));
    }

    /// A retired page the GPU maps goes home behind the barrier only when it must (another
    /// page wanted at its area page); its retirement itself asks the GPU nothing.
    #[test]
    fn a_retired_page_the_gpu_maps_goes_home_behind_the_barrier() {
        let (mut fb, pt, pool0) = setup();
        let x = BASE + 0x30_0000;
        assert!(poll(&mut fb, &mut NoGpu));
        let mut gpu = FakeGpu::new(0x1234_0000);
        fb.write_u64(pt + 8, pte(x));
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(x), pool0 + 0x1000);
        gpu.used.insert(x);
        let r0 = gpu.refs_calls.get();
        fb.write_u64(pt + 8, 0);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!((gpu.refs_calls.get(), stats(&fb).barriers), (r0, 0), "retiring asks nothing");
        let y = BASE + 0x31_0000;
        fb.write_u64(pt + 8, pte(y));
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!((fb.remap(x), fb.remap(y)), (x, pool0 + 0x1000));
        let st = stats(&fb);
        assert_eq!((st.barriers, st.releases_warm, st.slot_evictions), (1, 1, 1));
        assert_eq!(gpu.resynced, vec![0x1234_0000]);
    }

    /// A retired page that becomes a guest page table goes home like a pooled one (the
    /// table-page rule, §10): the walk must see every write the guest makes.
    #[test]
    fn a_retired_page_that_becomes_a_table_page_goes_home() {
        let (mut fb, pt, pool0) = setup();
        assert!(poll(&mut fb, &mut NoGpu));
        let cpt = BASE + 0x10_4000;
        fb.write_u64(cpt + 8 * 3, pte(BASE + 0x40_0000));
        fb.write_u64(pt + 8 * 5, pte(cpt));
        assert!(poll(&mut fb, &mut NoGpu));
        fb.write_u64(pt + 8 * 5, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!((fb.remap(cpt), retired(&fb)), (pool0 + 0x5000, 1));
        let (root, _) = context_tree(&mut fb);
        assert_eq!(leaf_at(&fb, root, 0x3000), Some(Target::Fb(BASE + 0x40_0000)));
        assert!(sync_untracked(&mut fb));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!((fb.remap(cpt), retired(&fb)), (cpt, 0));
        assert_eq!(stats(&fb).table_evictions, 1);
        assert_eq!(fb.read_u64(cpt + 8 * 3), Some(pte(BASE + 0x40_0000)));
    }

    /// The device CE is kept off pooled pages (fb.rs `plain_vram`), retired ones included:
    /// counted for the summary (§13).
    #[test]
    fn a_device_ce_transfer_over_a_pooled_page_stays_on_the_cpu_and_is_counted() {
        let (mut fb, pt, _pool0) = setup();
        assert!(poll(&mut fb, &mut NoGpu));
        let x = BASE + 0x10_0000;
        assert!(fb.plain_vram(x, 0x10000));
        fb.write_u64(pt + 8 * 5, pte(x + 0x2000));
        assert!(poll(&mut fb, &mut NoGpu));
        assert!(!fb.plain_vram(x, 0x10000));
        fb.write_u64(pt + 8 * 5, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        assert!(!fb.plain_vram(x, 0x10000));
        assert!(fb.plain_vram(x + 0x3000, 0x1000));
        let st = stats(&fb);
        assert_eq!((st.devce_blocked.get(), st.devce_blocked_retired.get()), (2, 1));
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
        // The guest unmaps the page: it stays in the pool, retired, no copy (§13); every
        // path still finds it at its pool page.
        fb.write_u64(pt + 8 * 5, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!((stats(&fb).releases_cold, stats(&fb).bytes_out, stats(&fb).retires), (0, 0, 1));
        assert_eq!(fb.remap(x), p);
        assert_eq!(retired(&fb), 1);
        // After RETIRE_AGE it goes home: the pool page is copied back to X, the remap goes.
        age_retired(&fb);
        assert!(poll(&mut fb, &mut NoGpu));
        let st = stats(&fb);
        assert_eq!((st.releases_cold, st.bytes_out, st.aged_out), (1, PAGE, 1));
        assert_eq!(retired(&fb), 0);
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
        // 0x9000 was retired, then relocated: home first, then in at 0xA000.
        assert_eq!((st.installs_cold, st.releases_cold, st.relocations), (3, 2, 1));
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
        // The aliased page is not pending: its second area page mirrors it (a replica),
        // so a guest write through either mapping reaches the page at the next sync point.
        assert_eq!(pending(&fb), 2);
        assert_eq!((stats(&fb).replicas, replicas(&fb)), (1, 1));
        fb.write_raw(pool0 + 0x1_3000 + 8, &0x5a5a_u64.to_le_bytes());
        assert!(sync_untracked(&mut fb));
        assert_eq!(fb.read_u64(x + 0x3008), Some(0x5a5a));
        let mut w = [0u8; 8];
        fb.read_raw(pool0 + 0x2008, &mut w);
        assert_eq!(u64::from_le_bytes(w), 0x5a5a, "the other mapping's pool page has it");
        fb.write_raw(pool0 + 0x2010, &0x6b6b_u64.to_le_bytes());
        assert!(sync_untracked(&mut fb));
        fb.read_raw(pool0 + 0x1_3010, &mut w);
        assert_eq!(u64::from_le_bytes(w), 0x6b6b, "and the replica has the other's");
        // The refusals are logged once; the retry changes nothing and logs nothing new.
        expire_retry(&fb);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(stats(&fb).refused, 3);
        assert_eq!(fb.remap(x + 0x3000), pool0 + 0x2000);
        // Once the guest drops the small mapping, the big page's area page takes it: the
        // replica goes, the page moves in with every byte either mapping wrote.
        fb.write_u64(pt + 8 * 2, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(fb.remap(x + 0x3000), pool0 + 0x1_3000);
        assert_eq!(pending(&fb), 2);
        assert_eq!(replicas(&fb), 0);
        assert_eq!(fb.read_u64(x + 0x3008), Some(0x5a5a));
        assert_eq!(fb.read_u64(x + 0x3010), Some(0x6b6b));
        // The big page goes: its 16 pages retire, then move back when they age out.
        fb.write_u64(big + 8, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(retired(&fb), 16);
        assert_eq!(fb.remap(x), pool0 + 0x1_0000);
        age_retired(&fb);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(fb.remap(x), x);
        assert_eq!(stats(&fb).releases_cold, 17);
    }

    fn batch_pending(fb: &FakeFb) -> bool {
        fb.pool_mut().as_ref().unwrap().batch_pending()
    }

    /// A chunk per hold.
    const TINY: Pace = Pace {
        budget: Some(Duration::ZERO),
        wait: Some(WAIT_SLICE),
    };

    /// The pages of `xs` that live in the pool now.
    fn pooled(fb: &FakeFb, xs: &[u64]) -> usize {
        xs.iter().filter(|x| fb.remap(**x) != **x).count()
    }

    /// §14: the GPU check waits for the shadow worker only when a job in flight took a page
    /// to move out of a walk (the host still maps it until the job is applied), and only
    /// for that job. Outside an invalidate the wait runs to the end; inside one it is left
    /// to the trigger reads, each waiting at most WAIT_SLICE, and the page moves once the
    /// job is applied.
    #[test]
    fn a_page_a_plan_in_flight_took_out_of_a_walk_waits_for_that_plan_only() {
        let (mut fb, pt, pool0) = setup();
        let (x, y, z) = (BASE + 0x30_0000, BASE + 0x31_0000, BASE + 0x32_0000);
        fb.write(x, &[7; 8]);
        let mut gpu = FakeGpu::new(0x1234_0000);
        // Nothing in flight touches the pages: no wait at all.
        fb.write_u64(pt + 8, pte(y));
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(y), pool0 + 0x1000);
        assert!(gpu.waits.borrow().is_empty());
        // Job 5 took X out of a shadow's walk and is not applied yet.
        gpu.in_flight.insert(x, 5);
        gpu.stamp.set(1);
        fb.write_u64(pt + 16, pte(x));
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(*gpu.waits.borrow(), vec![(5, None)]);
        assert_eq!(fb.remap(x), pool0 + 0x2000);
        let mut b = [0u8; 8];
        fb.read(pool0 + 0x2000, &mut b);
        assert_eq!(b, [7; 8]);
        let st = stats(&fb);
        assert_eq!((st.installs_cold, st.installs_warm, st.in_flight_waits), (2, 0, 1));
        // Inside an invalidate: job 9 took Z out of a walk and needs two more trigger reads.
        gpu.in_flight.insert(z, 9);
        gpu.stamp.set(2);
        gpu.slow_waits.set(2);
        fb.write_u64(pt + 24, pte(z));
        assert!(poll_sliced(&mut fb, &mut gpu, true, Pace::SLICED));
        assert!(batch_pending(&fb));
        assert_eq!(fb.remap(z), z);
        // A sync point outside the invalidate: no wait beyond WAIT_SLICE, the batch waits.
        assert!(poll_sliced(&mut fb, &mut gpu, false, Pace::NO_WAIT));
        assert!(batch_pending(&fb));
        assert_eq!(gpu.waits.borrow().last(), Some(&(9, Some(WAIT_SLICE))));
        // The invalidate page (its trigger is cleared when the poll returns) waits to the end.
        assert!(poll_sliced(&mut fb, &mut gpu, false, Pace::ALL));
        assert_eq!(gpu.waits.borrow().last(), Some(&(9, None)));
        assert_eq!(fb.remap(z), pool0 + 0x3000);
        assert!(!batch_pending(&fb));
        // The same with the trigger reads only.
        let w = BASE + 0x33_0000;
        gpu.in_flight.insert(w, 12);
        gpu.stamp.set(3);
        gpu.slow_waits.set(2);
        fb.write_u64(pt + 32, pte(w));
        assert!(poll_sliced(&mut fb, &mut gpu, true, Pace::SLICED));
        assert!(batch_pending(&fb));
        assert!(poll_sliced(&mut fb, &mut gpu, false, Pace::SLICED));
        assert!(batch_pending(&fb), "the job is not applied yet");
        assert!(poll_sliced(&mut fb, &mut gpu, false, Pace::SLICED));
        assert!(!batch_pending(&fb));
        assert_eq!(fb.remap(w), pool0 + 0x4000);
        assert!(gpu.waits.borrow().iter().rev().take(3).all(|w| *w == (12, Some(WAIT_SLICE))));
        let st = stats(&fb);
        assert_eq!((st.in_flight_waits, st.in_flight_parked, st.sliced_batches), (3, 4, 2));
        assert_eq!(st.installs_cold, 4);
    }

    /// §14: inside an invalidate the moves run a chunk per hold of the device lock; between
    /// two holds every page is either home or in the pool with its bytes, and the rest
    /// follows at the trigger reads. A mapping the guest makes meanwhile is decided once
    /// the batch is done, in the same hold. A sync point outside an invalidate finishes a
    /// batch at once.
    #[test]
    fn a_sliced_batch_moves_a_chunk_per_hold_and_the_pool_is_consistent_between_holds() {
        let (mut fb, pt, pool0) = setup();
        let xs: Vec<u64> = (0..40u64).map(|i| BASE + 0x30_0000 + i * 0x1000).collect();
        for (i, x) in xs.iter().enumerate() {
            fb.write(*x, &[i as u8; 8]);
            fb.write_u64(pt + 8 * (i as u64 + 1), pte(*x));
        }
        assert!(poll_sliced(&mut fb, &mut NoGpu, true, TINY));
        assert!(batch_pending(&fb));
        assert_eq!(pooled(&fb, &xs), CHUNK);
        let check = |fb: &FakeFb| {
            for (i, x) in xs.iter().enumerate() {
                let mut b = [0u8; 8];
                fb.read(*x, &mut b);
                assert_eq!(b, [i as u8; 8], "page {i} reads its bytes wherever it lives");
            }
        };
        check(&fb);
        // A new mapping while the batch runs.
        let w = BASE + 0x40_0000;
        fb.write(w, &[0xAA; 8]);
        fb.write_u64(pt + 8 * 45, pte(w));
        assert!(poll_sliced(&mut fb, &mut NoGpu, false, TINY));
        assert_eq!(pooled(&fb, &xs), 2 * CHUNK);
        assert_eq!(fb.remap(w), w, "decided after the batch");
        check(&fb);
        assert!(poll_sliced(&mut fb, &mut NoGpu, false, TINY));
        assert!(!batch_pending(&fb));
        assert_eq!(pooled(&fb, &xs), 40);
        assert_eq!(fb.remap(w), pool0 + 45 * 0x1000, "the batch done, the new mapping follows");
        check(&fb);
        let st = stats(&fb);
        assert_eq!((st.installs_cold, st.sliced_batches, st.slices_parked), (41, 1, 2));
        // Other pages at the same area pages: 40 moves home and 40 in, a chunk per hold;
        // then a sync point outside an invalidate (no budget) finishes the batch.
        let ys: Vec<u64> = (0..40u64).map(|i| BASE + 0x50_0000 + i * 0x1000).collect();
        for (i, y) in ys.iter().enumerate() {
            fb.write(*y, &[0x80 | i as u8; 8]);
            fb.write_u64(pt + 8 * (i as u64 + 1), pte(*y));
        }
        assert!(poll_sliced(&mut fb, &mut NoGpu, true, TINY));
        assert!(batch_pending(&fb));
        assert_eq!(pooled(&fb, &xs), 40 - CHUNK, "the releases run first");
        assert_eq!(pooled(&fb, &ys), 0);
        check(&fb);
        assert!(poll(&mut fb, &mut NoGpu));
        assert!(!batch_pending(&fb));
        assert_eq!((pooled(&fb, &xs), pooled(&fb, &ys)), (0, 40));
        check(&fb);
        for (i, y) in ys.iter().enumerate() {
            let mut b = [0u8; 8];
            fb.read(pool0 + (i as u64 + 1) * 0x1000, &mut b);
            assert_eq!(b, [0x80 | i as u8; 8]);
            assert_eq!(fb.remap(*y), pool0 + (i as u64 + 1) * 0x1000);
        }
    }

    /// §14: a batch asks the GPU side again when a shadow job was submitted between two
    /// holds (a plan may have mapped a page it still has to move): that page moves behind
    /// the barrier. Without a new job the answers stand.
    #[test]
    fn a_batch_asks_the_gpu_again_when_a_shadow_job_was_submitted_between_holds() {
        let (mut fb, pt, pool0) = setup();
        let root = 0x1234_0000;
        let mut gpu = FakeGpu::new(root);
        let xs: Vec<u64> = (0..20u64).map(|i| BASE + 0x30_0000 + i * 0x1000).collect();
        for (i, x) in xs.iter().enumerate() {
            fb.write_u64(pt + 8 * (i as u64 + 1), pte(*x));
        }
        assert!(poll_sliced(&mut fb, &mut gpu, true, TINY));
        assert_eq!(pooled(&fb, &xs), CHUNK);
        assert_eq!(gpu.refs_calls.get(), 1);
        // A plan maps page 18 while the lock is free.
        gpu.used.insert(xs[18]);
        gpu.stamp.set(1);
        assert!(poll_sliced(&mut fb, &mut gpu, false, TINY));
        assert!(!batch_pending(&fb));
        assert_eq!(gpu.refs_calls.get(), 2);
        assert_eq!(fb.remap(xs[18]), pool0 + 19 * 0x1000);
        assert_eq!(gpu.resynced, vec![root]);
        let st = stats(&fb);
        assert_eq!((st.installs_cold, st.installs_warm, st.barriers), (19, 1, 1));
        // The same stamp: no new question.
        let ys: Vec<u64> = (0..20u64).map(|i| BASE + 0x50_0000 + i * 0x1000).collect();
        for (i, y) in ys.iter().enumerate() {
            fb.write_u64(pt + 8 * (i as u64 + 30), pte(*y));
        }
        assert!(poll_sliced(&mut fb, &mut gpu, true, TINY));
        assert!(poll_sliced(&mut fb, &mut gpu, false, TINY));
        assert!(!batch_pending(&fb));
        assert_eq!(gpu.refs_calls.get(), 3);
        assert_eq!(pooled(&fb, &ys), 20);
    }

    /// §14: a batch pending past BATCH_FALLBACK runs all its moves at its next hold, well
    /// inside the guest's poll of the trigger; a wait for a job in flight stays bounded (the
    /// device lock is never held waiting for the worker), so the trigger reads busy until
    /// the job is applied.
    #[test]
    fn a_batch_pending_past_the_fallback_moves_everything_but_never_waits_to_the_end() {
        let (mut fb, pt, _) = setup();
        let mut gpu = FakeGpu::new(0x1234_0000);
        let xs: Vec<u64> = (0..40u64).map(|i| BASE + 0x30_0000 + i * 0x1000).collect();
        for (i, x) in xs.iter().enumerate() {
            fb.write_u64(pt + 8 * (i as u64 + 1), pte(*x));
        }
        gpu.in_flight.insert(xs[39], 3);
        gpu.slow_waits.set(100);
        assert!(poll_sliced(&mut fb, &mut gpu, true, TINY));
        assert!(batch_pending(&fb));
        assert_eq!(pooled(&fb, &xs), 0, "the whole batch waits for job 3");
        fb.pool_mut().as_mut().unwrap().batch.as_mut().unwrap().since =
            Instant::now() - BATCH_FALLBACK;
        assert!(poll_sliced(&mut fb, &mut gpu, false, TINY));
        assert!(batch_pending(&fb));
        assert!(gpu.waits.borrow().iter().all(|w| w.1 == Some(WAIT_SLICE)));
        gpu.slow_waits.set(0);
        assert!(poll_sliced(&mut fb, &mut gpu, false, TINY));
        assert!(!batch_pending(&fb));
        assert_eq!(pooled(&fb, &xs), 40, "all at once past the fallback");
    }

    /// §15: a page a shadow maps moves behind held rings and worker jobs, with no wait for
    /// the worker under the device lock: the poll holds the root's rings and queues the
    /// preempt; the trigger reads (bounded waits) find it done, move the page and queue the
    /// re-sync; the worker releases the rings after it and the trigger clears. A barrier whose
    /// preempt fails releases the rings and refuses the page (retried).
    #[test]
    fn a_page_the_gpu_maps_moves_behind_held_rings_without_a_wait_under_the_lock() {
        let (mut fb, pt, pool0) = setup();
        let root = 0x1234_0000;
        let x = BASE + 0x30_0000;
        fb.write(x, &[7; 8]);
        assert!(poll(&mut fb, &mut NoGpu));
        let mut gpu = FakeGpu::new(root);
        gpu.async_barrier = true;
        gpu.used.insert(x);
        gpu.barrier_polls.set(2);
        gpu.resync_polls.set(1);
        fb.write_u64(pt + 8, pte(x));
        assert!(poll_sliced(&mut fb, &mut gpu, true, Pace::SLICED));
        assert!(batch_pending(&fb));
        assert_eq!(fb.remap(x), x, "nothing moves before the preempt ran");
        assert_eq!(*gpu.holds.borrow(), BTreeSet::from([root]));
        // Trigger reads: the preempt is not done yet.
        assert!(poll_sliced(&mut fb, &mut gpu, false, Pace::SLICED));
        assert_eq!(fb.remap(x), x);
        // The preempt ran: the page moves, the re-sync is queued, the rings stay held.
        assert!(poll_sliced(&mut fb, &mut gpu, false, Pace::SLICED));
        assert_eq!(fb.remap(x), pool0 + 0x1000);
        assert_eq!(gpu.resynced, vec![root]);
        assert!(batch_pending(&fb), "the trigger waits for the re-sync");
        assert_eq!(gpu.holds.borrow().len(), 1);
        assert!(poll_sliced(&mut fb, &mut gpu, false, Pace::SLICED));
        assert!(!batch_pending(&fb));
        assert!(gpu.holds.borrow().is_empty(), "the worker released the rings");
        let mut b = [0u8; 8];
        fb.read(pool0 + 0x1000, &mut b);
        assert_eq!(b, [7; 8]);
        let st = stats(&fb);
        assert_eq!((st.installs_warm, st.barriers, st.barriers_refused, st.preempts), (1, 1, 0, 1));
        assert_eq!(st.barrier_parked, 3);
        // A preempt that fails: the rings go, the page is refused and waits.
        let y = BASE + 0x31_0000;
        gpu.used.insert(y);
        gpu.barrier_fail = Some("preempt of host TSG 0x5: status 0x1f".into());
        gpu.stamp.set(1);
        fb.write_u64(pt + 16, pte(y));
        assert!(poll_sliced(&mut fb, &mut gpu, true, Pace::SLICED));
        assert!(!batch_pending(&fb));
        assert_eq!(fb.remap(y), y);
        assert_eq!(gpu.cancelled, 1);
        assert!(gpu.holds.borrow().is_empty());
        assert_eq!(pending(&fb), 1);
        assert_eq!(stats(&fb).barriers_refused, 1);
        // The invalidate page's pace waits to the end: one poll does it all.
        gpu.barrier_fail = None;
        gpu.barrier_polls.set(5);
        gpu.resync_polls.set(5);
        expire_retry(&fb);
        assert!(poll_sliced(&mut fb, &mut gpu, false, Pace::ALL));
        assert!(!batch_pending(&fb));
        assert_eq!(fb.remap(y), pool0 + 0x2000);
        assert!(gpu.holds.borrow().is_empty());
    }

    /// §16: a trapped BAR0 write that is no merge point polls only when the pool has work
    /// due: a BAR1 table page written through a tracked path, a retired page to age (at most
    /// every RETIRE_SCAN), a paced retry; the replica report names each replica.
    #[test]
    fn the_pool_has_work_due_only_after_a_tracked_change_or_on_its_clocks() {
        let (mut fb, pt, _) = setup();
        let due = |fb: &FakeFb| fb.pool_mut().as_ref().unwrap().work_due(Instant::now());
        assert!(poll(&mut fb, &mut NoGpu));
        assert!(!due(&fb), "a quiet pool");
        let x = BASE + 0x30_0000;
        fb.write_u64(pt + 8, pte(x));
        assert!(due(&fb), "a BAR1 PTE written through a tracked path");
        assert!(poll(&mut fb, &mut NoGpu));
        assert!(!due(&fb));
        // Unmapped: retired; the age scan is due once RETIRE_SCAN passed since the last.
        fb.write_u64(pt + 8, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(retired(&fb), 1);
        assert!(due(&fb), "never scanned");
        assert!(!poll(&mut fb, &mut NoGpu), "the scan finds nothing aged");
        assert!(!due(&fb), "scanned just now");
        fb.pool_mut().as_mut().unwrap().last_age_scan = Some(Instant::now() - RETIRE_SCAN);
        assert!(due(&fb));
        // An alias makes a replica; the report names it.
        fb.write_u64(pt + 8, pte(x));
        fb.write_u64(pt + 16, pte(x));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(replicas(&fb), 1);
        sync_untracked(&mut fb);
        let r = fb.pool_mut().as_ref().unwrap().replicas.values().next().map(|r| (r.merges, r.x)).unwrap();
        assert_eq!(r.1, x);
        assert!(r.0 >= 1);
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
        // The guest unmaps the area page: retired, nothing moves and no barrier is asked
        // for (§13).
        fb.write_u64(pt + 8, 0);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!((fb.remap(x), retired(&fb), stats(&fb).barriers_refused), (pool0 + 0x1000, 1, 0));
        // Then it maps another page there: the retired page must go home, behind the
        // barrier, which is refused again and again.
        let y = BASE + 0x31_0000;
        fb.write_u64(pt + 8, pte(y));
        assert!(poll(&mut fb, &mut gpu));
        for i in 0..14 {
            expire_retry(&fb);
            assert!(poll(&mut fb, &mut gpu));
            assert_eq!(fb.remap(x), pool0 + 0x1000, "moved at refusal {i}");
            assert!(!fb.pool_rediff_pending(root));
        }
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
        assert_eq!((st.releases_cold, st.installs_cold, st.barriers, st.slot_evictions), (1, 2, 0, 1));
        fb.read(x, &mut b);
        assert_eq!(b, [9; 8]);
        assert!(gpu.resynced.is_empty());
    }

    /// A refused remap whose barrier keeps failing (a busy channel, Sunshine's during the
    /// capture) is retried less and less often: the interval doubles after every retry
    /// that moved nothing, up to RETRY_MAX, and is back at RETRY_EVERY once the page moves
    /// (docs/caraxes/sottr-stage.md §11).
    #[test]
    fn a_fruitless_retry_backs_off_and_a_move_resets_it() {
        let (mut fb, pt, pool0) = setup();
        let x = BASE + 0x30_0000;
        assert!(poll(&mut fb, &mut NoGpu));
        let mut gpu = FakeGpu::new(0x1234_0000);
        gpu.used.insert(x);
        gpu.ok = false;
        fb.write_u64(pt + 8, pte(x));
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(pending(&fb), 1);
        let every = |fb: &FakeFb| fb.pool_mut().as_ref().unwrap().retry_every;
        assert_eq!(every(&fb), RETRY_EVERY, "the refusal itself was no retry");
        let mut want = RETRY_EVERY;
        for _ in 0..10 {
            expire_retry(&fb);
            assert!(poll(&mut fb, &mut gpu));
            want = (want * 2).min(RETRY_MAX);
            assert_eq!(every(&fb), want);
        }
        assert_eq!(want, RETRY_MAX);
        let st = stats(&fb);
        assert_eq!((st.retries, st.retry_backoffs), (10, 10));
        // Inside the interval nothing is retried, even long after RETRY_EVERY.
        fb.pool_mut().as_mut().unwrap().last_retry =
            Some(Instant::now() - RETRY_EVERY * 8);
        assert!(!poll(&mut fb, &mut gpu));
        assert_eq!(stats(&fb).retries, 10);
        // A new mapping is walked at once whatever the interval.
        let y = BASE + 0x31_0000;
        fb.write_u64(pt + 8 * 2, pte(y));
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(y), pool0 + 0x2000);
        // The channel goes idle: the next retry moves the page and resets the interval.
        gpu.ok = true;
        expire_retry(&fb);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(x), pool0 + 0x1000);
        assert_eq!((pending(&fb), every(&fb)), (0, RETRY_EVERY));
        assert!(fb.bar1_pool_summary().unwrap().contains("11 retry poll(s), 10 of them moved nothing"));
    }

    /// A sync point merges the replicas grouped by home page, so each VRAM region behind the
    /// device's BAR1 windows is visited once, not once per replica.
    #[test]
    fn replicas_merge_in_home_order() {
        let r = |x: u64| Replica::new(x, Vec::new());
        let m: BTreeMap<u64, Replica> = [
            (0x1000, r(0x9_0000_0000)),
            (0x2000, r(0x1_0000_0000)),
            (0x3000, r(0x9_0000_1000)),
            (0x4000, r(0x1_0000_1000)),
        ]
        .into_iter()
        .collect();
        assert_eq!(merge_order(&m), vec![0x2000, 0x4000, 0x1000, 0x3000]);
    }

    /// The vm4-615 idle desktop (docs/caraxes/bar1-pool.md §11): a USERD page whose
    /// release is refused, and shadow walks that keep adding and dropping table pages
    /// (Sunshine maps and unmaps its capture buffer every frame). Before, every such walk
    /// re-armed the whole poll, and each poll waited for the shadow worker and scanned
    /// every shadow leaf to refuse the same USERD again (2 to 60 ms under the device
    /// lock). Now a walk that adds no pooled page leaves the poll idle, and the paced retry
    /// refuses the USERD without the worker wait or the leaf scan.
    #[test]
    fn a_waiting_userd_release_and_growing_walks_keep_the_poll_cheap() {
        let (mut fb, pt, pool0) = setup();
        let mut gpu = FakeGpu::new(0x1234_0000);
        assert!(poll(&mut fb, &mut gpu));
        let x = BASE + 0x40_0000;
        fb.write_u64(pt + 8 * 5, pte(x));
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(x), pool0 + 0x5000);
        let (r0, u0) = (gpu.refs_calls.get(), gpu.userd_calls.get());
        assert_eq!((r0, u0), (1, 1));
        // The page becomes a channel's USERD; the guest unmaps it: retired, no GPU check
        // (§13). Once it is due to go home, its move is refused: waiting.
        gpu.userd.insert(x);
        fb.write_u64(pt + 8 * 5, 0);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!((fb.remap(x), retired(&fb)), (pool0 + 0x5000, 1));
        assert_eq!((gpu.refs_calls.get(), gpu.userd_calls.get()), (r0, u0));
        age_retired(&fb);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(x), pool0 + 0x5000);
        assert_eq!((gpu.refs_calls.get(), gpu.userd_calls.get()), (r0, u0 + 1));
        assert!(stats(&fb).first.iter().any(|l| l.contains("USERD of mirrored channel")));
        let lb = last(&fb);
        assert_eq!((lb.candidates, lb.ops, lb.refs_pages), (0, 1, 0));
        assert!(lb.to_string().contains("(0 page(s)); barrier"), "{lb}");
        // Shadow walks that add table pages outside the pool: the poll stays idle.
        hold_retry(&fb);
        let (root, _) = context_tree(&mut fb);
        fb.write_u64(BASE + 0x10_3000 + 16, vid(BASE + 0x10_5000));
        fb.walk_update(None, root, false, 1 << 20);
        let polls = stats(&fb).polls;
        assert!(!poll(&mut fb, &mut gpu));
        assert_eq!(stats(&fb).polls, polls, "no work");
        assert_eq!((gpu.refs_calls.get(), gpu.userd_calls.get()), (r0, u0 + 1));
        // The paced retry: the USERD check only.
        expire_retry(&fb);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!((gpu.refs_calls.get(), gpu.userd_calls.get()), (r0, u0 + 2));
        assert_eq!(fb.remap(x), pool0 + 0x5000);
        assert!(last(&fb).retry);
        // The channel goes: the retry moves the page back (cold, the leaf check runs).
        gpu.userd.clear();
        expire_retry(&fb);
        assert!(poll(&mut fb, &mut gpu));
        assert_eq!(fb.remap(x), x);
        assert_eq!(gpu.refs_calls.get(), r0 + 1);
        assert!(fb.pool_mut().as_ref().unwrap().waiting.is_empty());
        // Nothing waits any more: no retry, the poll is idle.
        expire_retry(&fb);
        assert!(!poll(&mut fb, &mut gpu));
    }

    /// A write to a directory page (the root, PD2, PD1 or PD0 are shared by every unit of
    /// the area) reads each unit's path (five qwords) and walks only the units whose path
    /// changed; a write to a unit's page table walks that unit.
    #[test]
    fn a_directory_write_walks_only_the_units_whose_path_changed() {
        let (mut fb, _pt, pool0) = setup();
        assert!(poll(&mut fb, &mut NoGpu));
        let w0 = stats(&fb).units_walked;
        let pd0 = BASE + 0x3000;
        // A PD0 entry far above the area (the RM's own BAR1 mappings): no unit walked.
        fb.write_u64(pd0 + 16 * 100 + 8, vid(BASE + 0xB000));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(stats(&fb).units_walked, w0);
        assert_eq!((stats(&fb).units_path_only, last(&fb).units_path_only), (2, 2));
        // Unit 1 gets a page table: walked, its page moves into the pool.
        let (pt1, x) = (BASE + 0xA000, BASE + 0x40_0000);
        fb.write_u64(pt1 + 8 * 2, pte(x));
        fb.write_u64(pd0 + 16 + 8, vid(pt1));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(stats(&fb).units_walked, w0 + 1);
        assert_eq!(stats(&fb).units_path_only, 3);
        assert_eq!(fb.remap(x), pool0 + 2 * MIB + 0x2000);
        // A write to unit 1's page table walks unit 1 only (no path check either).
        fb.write_u64(pt1 + 8 * 2, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!((stats(&fb).units_walked, stats(&fb).units_path_only), (w0 + 2, 3));
        assert_eq!(last(&fb).candidates, 1);
        // Retired (§13): it stays at its pool page until it ages out.
        assert_eq!(fb.remap(x), pool0 + 2 * MIB + 0x2000);
        age_retired(&fb);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(fb.remap(x), x);
    }

    /// A poll after a BAR0 write that is no flush point (`poll_at` without `merge`) leaves
    /// the replicas alone; the next flush point merges them.
    #[test]
    fn a_poll_without_merge_leaves_the_replicas_to_the_next_flush_point() {
        let (mut fb, pt, pool0) = setup();
        assert!(poll(&mut fb, &mut NoGpu));
        let x = BASE + 0x40_0000;
        // The guest maps x twice: area page 5 holds it, area page 6 is its replica.
        fb.write_u64(pt + 8 * 5, pte(x));
        fb.write_u64(pt + 8 * 6, pte(x));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!((fb.remap(x), replicas(&fb)), (pool0 + 0x5000, 1));
        // A guest write through the replica's area page.
        fb.write_raw(pool0 + 0x6000 + 16, &0xabcdu64.to_le_bytes());
        assert!(!poll_at(&mut fb, &mut NoGpu, false));
        assert_eq!(fb.read_u64(x + 16), Some(0));
        let lb = last(&fb);
        assert_eq!((lb.merge, lb.replicas_merged), (false, 0));
        assert!(lb.to_string().contains("merge skipped"), "{lb}");
        assert!(stats(&fb).polls_unmerged >= 1);
        assert!(poll_at(&mut fb, &mut NoGpu, true));
        assert_eq!(fb.read_u64(x + 16), Some(0xabcd));
        assert_eq!(last(&fb).replicas_merged, 1);
    }

    /// At an MMU invalidate only the replicas whose home is a page table the device follows
    /// are merged ([`sync_untracked_tables`]): the guest's PTE write through the area reaches
    /// the walk, while a data replica's write waits for the next engine start, whose flush
    /// point ([`sync_untracked`]) merges it.
    #[test]
    fn an_invalidate_merges_the_table_replicas_and_leaves_the_data_ones() {
        let (mut fb, pt, pool0) = setup();
        assert!(poll(&mut fb, &mut NoGpu));
        // A table replica: the shadow watches cpt, the guest maps it into area page 5.
        let (root, cpt) = context_tree(&mut fb);
        fb.write_u64(pt + 8 * 5, pte(cpt));
        // A data replica: x mapped twice, area page 7 holds it, area page 8 is its replica.
        let x = BASE + 0x40_0000;
        fb.write_u64(pt + 8 * 7, pte(x));
        fb.write_u64(pt + 8 * 8, pte(x));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(replicas(&fb), 2);
        // The guest writes both through the area, then invalidates.
        fb.write_raw(pool0 + 0x5000 + 8 * 4, &pte(BASE + 0x41_0000).to_le_bytes());
        fb.write_raw(pool0 + 0x8000 + 16, &0xabcdu64.to_le_bytes());
        assert!(sync_untracked_tables(&mut fb));
        assert_eq!(fb.read_u64(cpt + 8 * 4), Some(pte(BASE + 0x41_0000)), "the PTE reached the table");
        assert!(fb.tables_dirty(root));
        assert_eq!(fb.read_u64(x + 16), Some(0), "the data waits for the engine start");
        // The engine start's flush point merges the data replica too.
        assert!(sync_untracked(&mut fb));
        assert_eq!(fb.read_u64(x + 16), Some(0xabcd));
    }

    /// The pool's walk reads each table page from the FB once and keeps a write-through
    /// copy: a tracked write patches it (the next walk sees the new PTE without a read), a
    /// write of unknown content (a BAR2 memfd sync) drops it, and a page no walk reads
    /// any more is dropped too (bar1-pool.md §12).
    #[test]
    fn unit_walks_read_table_pages_from_their_write_through_copies() {
        let (mut fb, pt, pool0) = setup();
        assert!(poll(&mut fb, &mut NoGpu));
        let cached = |fb: &FakeFb| fb.pool_mut().as_ref().unwrap().pt_raw.len();
        // Root, PD2, PD1, PD0 and unit 0's page table.
        assert_eq!(cached(&fb), 5);
        let x = BASE + 0x40_0000;
        fb.write_u64(pt + 8 * 5, pte(x));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(last(&fb).pages_read, 0, "patched copy, no FB read");
        assert_eq!(fb.remap(x), pool0 + 0x5000);
        // Unknown content: the copy goes and the next walk reads the page again.
        fb.note_pages_changed(&[pt >> 12]);
        assert_eq!(cached(&fb), 4);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(last(&fb).pages_read, 1);
        assert_eq!(fb.remap(x), pool0 + 0x5000);
        // Unit 0 moves to another page table: the old one's copy goes.
        let pt2 = BASE + 0xC000;
        fb.write_u64(pt2 + 8 * 5, pte(x));
        fb.write_u64(BASE + 0x3000 + 8, vid(pt2));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(last(&fb).pages_read, 1);
        let g = fb.pool_mut();
        let p = g.as_ref().unwrap();
        assert!(p.pt_raw.contains_key(&(pt2 >> 12)) && !p.pt_raw.contains_key(&(pt >> 12)));
    }

    /// A replica whose home is outside the pool reads its home only after a device-visible
    /// write to it (and at the sweep); an aliased replica, whose home is a pool page,
    /// always reads it (bar1-pool.md §12).
    #[test]
    fn a_replica_merge_reads_its_home_only_when_it_may_have_changed() {
        let (mut fb, pt, pool0) = setup();
        assert!(poll(&mut fb, &mut NoGpu));
        let (root, cpt) = context_tree(&mut fb);
        fb.write_u64(pt + 8 * 5, pte(cpt));
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!(replicas(&fb), 1);
        let reads = |fb: &FakeFb| stats(fb).replica_home_reads;
        let r0 = reads(&fb);
        // Nothing changed: the merge reads only the area page.
        assert!(!sync_untracked(&mut fb));
        assert_eq!(reads(&fb), r0);
        // A guest write through the area still reaches the page and the walk.
        let p = pool0 + 0x5000;
        fb.write_raw(p + 8 * 4, &pte(BASE + 0x41_0000).to_le_bytes());
        assert!(sync_untracked(&mut fb));
        assert_eq!(reads(&fb), r0);
        assert_eq!(fb.read_u64(cpt + 8 * 4), Some(pte(BASE + 0x41_0000)));
        fb.walk_update(None, root, false, 1 << 20);
        assert_eq!(leaf_at(&fb, root, 0x4000), Some(Target::Fb(BASE + 0x41_0000)));
        // A device write of the home: read at the next merge, and copied to the area.
        fb.write_u64(cpt + 8 * 6, pte(BASE + 0x42_0000));
        assert!(sync_untracked(&mut fb));
        assert_eq!(reads(&fb), r0 + 1);
        let mut w = [0u8; 8];
        fb.read_raw(p + 8 * 6, &mut w);
        assert_eq!(u64::from_le_bytes(w), pte(BASE + 0x42_0000));
        // A change the device did not see (written straight to the store) is picked up
        // by the sweep at the latest.
        let mut hits = 0;
        fb.copy_page_raw(cpt, cpt); // a raw copy counts as unknown content
        assert!(!sync_untracked(&mut fb));
        assert_eq!(reads(&fb), r0 + 2);
        for _ in 0..HOME_SWEEP {
            sync_untracked(&mut fb);
            hits += 1;
        }
        assert_eq!(reads(&fb), r0 + 3, "one sweep read in {hits} merges");
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
            defer_flush: false,
            flush_page_memory: false,
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
        // Unmapped the same way: retired (§13), then back home when it ages out.
        guest.store_word(1, 5, 0);
        fb.bar2_sync(Some(bar2), None);
        assert!(poll(&mut fb, &mut NoGpu));
        assert_eq!((fb.remap(x), retired(&fb)), (pool0 + 0x5000, 1));
        age_retired(&fb);
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
        // Back out of the pool (unmapped, retired, aged out): x in the store has everything.
        fb.write_u64(pt + 8 * 5, 0);
        assert!(poll(&mut fb, &mut NoGpu));
        age_retired(&fb);
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

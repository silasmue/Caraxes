// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Lazy shadow plans (`CARAXES_ALLOC_LAZY=1`, docs/caraxes/alloc-path.md §3).
//!
//! A guest `cuMemAlloc` or `cuMemFree` ends in the guest RM's MMU invalidate of the context's
//! page directory. By default the device walks the changed table pages and hands a plan to
//! the shadow worker at that invalidate (mapping.md §4), and the worker makes the host RM
//! calls (`NV50_MEMORY_VIRTUAL`, `RM_MAP_MEMORY_DMA`, NVOS47) in the background. The only
//! point at which the mirror must match the guest's tables is before a host channel runs
//! work: the doorbell of a shadow channel, and its creation, both of which already plan
//! their root and wait for the worker (`Mirror::gr_shadow_doorbell`). D18's line applies:
//! the guest's page tables are the guest's own bookkeeping, and the physical GPU only reads
//! the shadow, which validates every leaf when it is planned (`shadow::resolve_leaf`).
//!
//! With the switch on, an invalidate only records its root as pending. The walk cache
//! keeps collecting the dirty table pages (ptcache.rs), so the plan made later is the
//! difference between what is mirrored and the guest's tables at that moment: an
//! allocation freed before the next doorbell is never mapped on the host, a region freed
//! and allocated again at the same place costs nothing, and many allocations become one
//! plan with coalesced runs. The pending roots are planned
//! - at the next doorbell or creation of any shadow channel, before its barrier;
//! - at an invalidate once the oldest deferred one is older than the deadline
//!   (`CARAXES_ALLOC_LAZY_MS`, default 20 ms, 0: no deadline), which bounds the work a
//!   doorbell inherits and how long a mapping the guest dropped stays mapped on the host.
//!
//! Isolation is unchanged: a deferred plan is filtered like an eager one, and a mapping
//! that stays on the host longer than before was validated when it was made, so it still
//! points into this VM's reservation or its registered guest RAM.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

/// The switch.
pub const ENV: &str = "CARAXES_ALLOC_LAZY";
/// The deadline in milliseconds (0: plan only at doorbells and channel creations).
pub const DEADLINE_ENV: &str = "CARAXES_ALLOC_LAZY_MS";
pub const DEFAULT_DEADLINE: Duration = Duration::from_millis(20);

/// Deferred invalidates: the roots they named, or every root (`ALL_PDB`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Pending {
    pub all: bool,
    pub roots: BTreeSet<u64>,
    /// How many invalidates this batch covers.
    pub invalidates: u64,
    /// With `all`: roots planned since (the untrapped-GPPut rule's roots), which the batch
    /// no longer owes anything.
    pub exempt: BTreeSet<u64>,
}

impl Pending {
    /// The roots to plan: `None` for every root.
    pub fn roots(&self) -> Option<Vec<u64>> {
        (!self.all).then(|| self.roots.iter().copied().collect())
    }

    fn is_empty(&self) -> bool {
        self.invalidates == 0
    }

    /// Whether the batch owes root `r` a plan.
    fn owes(&self, r: u64) -> bool {
        !self.is_empty() && (self.roots.contains(&r) || (self.all && !self.exempt.contains(&r)))
    }
}

/// What the device does at a guest MMU invalidate.
#[derive(Debug, PartialEq, Eq)]
pub enum OnInvalidate {
    /// Plan now, as without the switch: this root, or every root (`None`).
    Plan(Option<u64>),
    /// Deferred to the next doorbell, channel creation or deadline.
    Defer,
    /// The deadline passed: plan everything pending now, this invalidate included.
    Flush(Pending),
}

/// Why a batch was planned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushAt {
    Doorbell,
    Deadline,
}

#[derive(Debug, Default, Clone)]
pub struct LazyStats {
    pub deferred: u64,
    pub batches_at_doorbell: u64,
    pub batches_at_deadline: u64,
    /// Invalidates in the largest batch.
    pub max_batch: u64,
}

/// The policy (no FB or host access here; `Mirror` does the planning).
#[derive(Debug)]
pub struct LazyPlans {
    enabled: bool,
    deadline: Option<Duration>,
    pending: Pending,
    since: Option<Instant>,
    pub stats: LazyStats,
}

/// `CARAXES_ALLOC_LAZY`: `1`/`on` on; unset, empty, `0`/`off` off; anything else an error.
pub fn parse_switch(v: Option<&str>) -> Result<bool, String> {
    match v.map(str::trim) {
        None | Some("") | Some("0") | Some("off") => Ok(false),
        Some("1") | Some("on") => Ok(true),
        Some(x) => Err(format!("{ENV}={x:?}: expected 1, on, 0 or off")),
    }
}

/// `CARAXES_ALLOC_LAZY_MS`: unset or empty is the default, 0 no deadline.
pub fn parse_deadline(v: Option<&str>) -> Result<Option<Duration>, String> {
    match v.map(str::trim) {
        None | Some("") => Ok(Some(DEFAULT_DEADLINE)),
        Some(x) => match x.parse::<u64>() {
            Ok(0) => Ok(None),
            Ok(ms) => Ok(Some(Duration::from_millis(ms))),
            Err(_) => Err(format!("{DEADLINE_ENV}={x:?}: expected milliseconds")),
        },
    }
}

impl LazyPlans {
    pub fn new(enabled: bool, deadline: Option<Duration>) -> Self {
        LazyPlans {
            enabled,
            deadline,
            pending: Pending::default(),
            since: None,
            stats: LazyStats::default(),
        }
    }

    /// From the environment. A malformed value is an error; the caller then stays eager.
    pub fn from_env() -> Result<Self, String> {
        let on = parse_switch(std::env::var(ENV).ok().as_deref())?;
        let deadline = parse_deadline(std::env::var(DEADLINE_ENV).ok().as_deref())?;
        Ok(LazyPlans::new(on, deadline))
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn deadline(&self) -> Option<Duration> {
        self.deadline
    }

    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// A guest MMU invalidate of `pdb` (`None`: `ALL_PDB`) at `now`.
    pub fn on_invalidate(&mut self, pdb: Option<u64>, now: Instant) -> OnInvalidate {
        if !self.enabled {
            return OnInvalidate::Plan(pdb);
        }
        match pdb {
            Some(r) => {
                self.pending.roots.insert(r);
                self.pending.exempt.remove(&r);
            }
            None => {
                self.pending.all = true;
                self.pending.exempt.clear();
            }
        }
        self.pending.invalidates += 1;
        let since = *self.since.get_or_insert(now);
        if self.deadline.is_some_and(|d| now.duration_since(since) >= d) {
            return OnInvalidate::Flush(self.take_batch(FlushAt::Deadline).expect("pending"));
        }
        self.stats.deferred += 1;
        OnInvalidate::Defer
    }

    /// At an invalidate the untrapped-GPPut rule plans now (untrapped.rs): take the pending
    /// roots of `roots` out of the batch and return them; the rest stays deferred. A batch
    /// that names every root (`ALL_PDB`) keeps it: the roots returned are planned now and
    /// planned again (with nothing left to do) with the rest.
    pub fn take_roots(&mut self, roots: &BTreeSet<u64>) -> Vec<u64> {
        if self.pending.is_empty() {
            return Vec::new();
        }
        if self.pending.all {
            self.exempt(roots);
            return roots.iter().copied().collect();
        }
        let taken: Vec<u64> = self.pending.roots.intersection(roots).copied().collect();
        for r in &taken {
            self.pending.roots.remove(r);
        }
        if self.pending.roots.is_empty() {
            self.pending = Pending::default();
            self.since = None;
        }
        taken
    }

    /// `roots` were planned just now: a pending batch no longer owes them anything (the
    /// ran-ahead evidence, `owes_plan`).
    pub fn exempt(&mut self, roots: &BTreeSet<u64>) {
        if self.pending.is_empty() {
            return;
        }
        for r in roots {
            self.pending.roots.remove(r);
        }
        if self.pending.all {
            self.pending.exempt.extend(roots.iter().copied());
        } else if self.pending.roots.is_empty() {
            self.pending = Pending::default();
            self.since = None;
        }
    }

    /// Whether a deferred invalidate still owes root `r` a plan.
    pub fn owes_plan(&self, r: u64) -> bool {
        self.pending.owes(r)
    }

    /// At a doorbell or creation of a shadow channel: the pending batch, if any.
    pub fn take(&mut self) -> Option<Pending> {
        self.take_batch(FlushAt::Doorbell)
    }

    fn take_batch(&mut self, at: FlushAt) -> Option<Pending> {
        if self.pending.is_empty() {
            return None;
        }
        self.since = None;
        let p = std::mem::take(&mut self.pending);
        match at {
            FlushAt::Doorbell => self.stats.batches_at_doorbell += 1,
            FlushAt::Deadline => self.stats.batches_at_deadline += 1,
        }
        self.stats.max_batch = self.stats.max_batch.max(p.invalidates);
        Some(p)
    }

    /// The exit-summary part.
    pub fn summary(&self) -> String {
        let s = &self.stats;
        format!(
            "alloc lazy: {} invalidate(s) deferred, planned in {} batch(es) at a doorbell and {} at the deadline ({}), largest batch {} invalidate(s){}",
            s.deferred,
            s.batches_at_doorbell,
            s.batches_at_deadline,
            self.deadline
                .map_or("none".into(), |d| format!("{} ms", d.as_millis())),
            s.max_batch,
            if self.is_pending() {
                format!(", {} still pending", self.pending.invalidates)
            } else {
                String::new()
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fb::{FakeFb, Target};
    use crate::guestmem::GuestMemory;
    use crate::shadow::ShadowFront;

    const A: u64 = 0x1_6900_0000;
    const B: u64 = 0x1_6900_4000;

    #[test]
    fn off_plans_at_every_invalidate() {
        let mut l = LazyPlans::new(false, Some(DEFAULT_DEADLINE));
        let t = Instant::now();
        assert_eq!(l.on_invalidate(Some(A), t), OnInvalidate::Plan(Some(A)));
        assert_eq!(l.on_invalidate(None, t), OnInvalidate::Plan(None));
        assert!(!l.is_pending());
        assert_eq!(l.take(), None);
    }

    #[test]
    fn on_defers_until_the_doorbell_and_collects_the_roots() {
        let mut l = LazyPlans::new(true, None);
        let t = Instant::now();
        assert_eq!(l.on_invalidate(Some(A), t), OnInvalidate::Defer);
        assert_eq!(l.on_invalidate(Some(B), t), OnInvalidate::Defer);
        assert_eq!(l.on_invalidate(Some(A), t + Duration::from_secs(60)), OnInvalidate::Defer);
        assert!(l.is_pending());
        let p = l.take().unwrap();
        assert_eq!(p.roots(), Some(vec![A, B]));
        assert_eq!(p.invalidates, 3);
        assert_eq!(l.take(), None, "a batch is planned once");
        assert_eq!(l.stats.deferred, 3);
        assert_eq!(l.stats.batches_at_doorbell, 1);
        assert_eq!(l.stats.max_batch, 3);
    }

    #[test]
    fn all_pdb_plans_every_root() {
        let mut l = LazyPlans::new(true, None);
        let t = Instant::now();
        l.on_invalidate(Some(A), t);
        l.on_invalidate(None, t);
        assert_eq!(l.take().unwrap().roots(), None);
    }

    #[test]
    fn the_deadline_flushes_at_the_next_invalidate() {
        let d = Duration::from_millis(20);
        let mut l = LazyPlans::new(true, Some(d));
        let t0 = Instant::now();
        assert_eq!(l.on_invalidate(Some(A), t0), OnInvalidate::Defer);
        assert_eq!(l.on_invalidate(Some(B), t0 + d - Duration::from_millis(1)), OnInvalidate::Defer);
        match l.on_invalidate(Some(A), t0 + d) {
            OnInvalidate::Flush(p) => {
                assert_eq!(p.roots(), Some(vec![A, B]));
                assert_eq!(p.invalidates, 3, "the batch includes the invalidate that flushed it");
            }
            other => panic!("expected a flush, got {other:?}"),
        }
        assert!(!l.is_pending());
        // The clock starts again at the next deferred invalidate.
        assert_eq!(l.on_invalidate(Some(A), t0 + 2 * d), OnInvalidate::Defer);
        assert_eq!(l.stats.batches_at_deadline, 1);
        assert!(l.summary().contains("1 still pending"));
    }

    #[test]
    fn the_switches_parse_strictly() {
        assert_eq!(parse_switch(None), Ok(false));
        assert_eq!(parse_switch(Some("0")), Ok(false));
        assert_eq!(parse_switch(Some("off")), Ok(false));
        assert_eq!(parse_switch(Some(" 1 ")), Ok(true));
        assert_eq!(parse_switch(Some("on")), Ok(true));
        assert!(parse_switch(Some("yes")).is_err());
        assert_eq!(parse_deadline(None), Ok(Some(DEFAULT_DEADLINE)));
        assert_eq!(parse_deadline(Some("0")), Ok(None));
        assert_eq!(parse_deadline(Some("5")), Ok(Some(Duration::from_millis(5))));
        assert!(parse_deadline(Some("5ms")).is_err());
    }

    /// The property the switch relies on: the walk cache keeps every table write between
    /// two plans, so one plan made late sees the guest's final tables, and an allocation
    /// freed before it is not in the plan at all.
    #[test]
    fn a_late_plan_sees_the_final_tables_and_skips_what_was_freed() {
        const BASE: u64 = 0x1_0000_0000;
        const END: u64 = 0x2_0000_0000;
        let mut fb = FakeFb::new(END + (64 << 20));
        let vid = |a: u64| ((a >> 12) << 8) | 0b010;
        let pte = |a: u64| ((a >> 12) << 8) | 1 | (6u64 << 56);
        let (root, pd2, pd1, pd0, pt) = (BASE, BASE + 0x1000, BASE + 0x2000, BASE + 0x3000, BASE + 0x4000);
        fb.write_u64(root, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        fb.write_u64(pd0 + 8, vid(pt));
        let (x, y) = (BASE + 0x10_0000, BASE + 0x20_0000);
        fb.write_u64(pt + 8, pte(x)); // VA 0x1000 -> X
        let mem = GuestMemory::new();
        let mut front = ShadowFront::new(root);
        let leaves = |fb: &FakeFb| {
            fb.with_walk(Target::Fb(root), |c| c.all_leaves())
                .unwrap_or_default()
                .into_iter()
                .map(|(va, t, size, _)| (va, t, size))
                .collect::<Vec<_>>()
        };
        let p = front.plan(&fb, &mem, BASE, END, 1, &[], &[]).unwrap();
        assert_eq!(p.want, 1);
        assert_eq!(leaves(&fb), vec![(0x1000, Target::Fb(x), 0x1000)]);

        // Deferred: VA 0x2000 allocated and freed again, VA 0x1000 freed and allocated at
        // Y, all before the next plan.
        fb.write_u64(pt + 16, pte(y));
        fb.write_u64(pt + 16, 0);
        fb.write_u64(pt + 8, 0);
        fb.write_u64(pt + 8, pte(y));
        let p = front.plan(&fb, &mem, BASE, END, 1, &[], &[]).unwrap();
        assert_eq!(p.want, 1, "only the final leaf is wanted");
        assert_eq!(leaves(&fb), vec![(0x1000, Target::Fb(y), 0x1000)]);

        // An allocation and its free between two plans leave nothing to map: the table
        // page is back to what the cache holds, so no unit is even diffed.
        fb.write_u64(pt + 24, pte(x));
        fb.write_u64(pt + 24, 0);
        if let Some(p) = front.plan(&fb, &mem, BASE, END, 1, &[], &[]) {
            assert_eq!((p.want, p.walked), (0, 0), "no unit changed");
        }
        assert_eq!(leaves(&fb), vec![(0x1000, Target::Fb(y), 0x1000)]);
        assert!(front.plan(&fb, &mem, BASE, END, 1, &[], &[]).is_none(), "nothing left to plan");
    }

    #[test]
    fn the_rule_takes_only_its_roots_out_of_the_batch() {
        let mut l = LazyPlans::new(true, None);
        let t = Instant::now();
        let set: BTreeSet<u64> = [A].into();
        assert!(l.take_roots(&set).is_empty(), "nothing pending");
        l.on_invalidate(Some(A), t);
        l.on_invalidate(Some(B), t);
        assert_eq!(l.take_roots(&set), vec![A]);
        assert_eq!(l.take().unwrap().roots(), Some(vec![B]), "B stays deferred");
        // Only A pending: taking it empties the batch.
        l.on_invalidate(Some(A), t);
        assert_eq!(l.take_roots(&set), vec![A]);
        assert!(!l.is_pending());
        // ALL_PDB pending: the roots are planned now and the batch stays, owing them nothing.
        l.on_invalidate(None, t);
        assert!(l.owes_plan(A) && l.owes_plan(B));
        assert_eq!(l.take_roots(&set), vec![A]);
        assert!(l.is_pending());
        assert!(!l.owes_plan(A) && l.owes_plan(B));
        // A later invalidate of A, or another ALL_PDB, owes A again.
        l.on_invalidate(Some(A), t);
        assert!(l.owes_plan(A));
        l.exempt(&set);
        assert!(!l.owes_plan(A));
        l.on_invalidate(None, t);
        assert!(l.owes_plan(A));
        assert_eq!(l.take().unwrap().roots(), None);
        assert!(!l.owes_plan(B), "nothing pending");
    }
}

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The GPPut store that does not trap (docs/caraxes/bar1-pool.md §9, the Xid 31 of the
//! morning matrix).
//!
//! The mirror's ordering rule is that nothing reaches the GPU before the shadow worker has
//! applied every plan of the guest's tables (mirror.rs `gr_shadow_doorbell`). The device
//! enforces it at the doorbell, and at a trapped GPPut store (boot.rs `gp_put_barrier`),
//! because a host channel that is still loaded fetches a new GPPut from its USERD without a
//! doorbell (display-e.md §9). A mirrored channel whose USERD the guest writes **without a
//! trap** escapes both: its USERD page lives in the BAR1 pool (the guest mapped the page into
//! the pool area before it created the channel, so the host channel was created on the pool
//! page, bar1pool.rs), or it lives in guest RAM. The guest's order is page tables,
//! invalidate, GPFIFO entry, GPPut, doorbell. A loaded host channel then runs the new work
//! while the worker still applies the plan of that invalidate (or, with
//! `CARAXES_ALLOC_LAZY=1`, before the plan is even made), and a kernel that writes a freshly
//! allocated 2 MiB region meets a shadow without a PDE there: `FAULT_PDE` on a GR write.
//!
//! The rule for such a channel moves the barrier to the last point the device sees: the
//! guest's invalidate. An invalidate that names the shadow root of a channel with an
//! untrapped USERD (or every root) plans that root now, together with everything lazy mode
//! deferred, and returns to the guest only after the worker applied it ([`must_hold`]). The
//! return is the trigger's busy bit reading clear, which the guest polls (invgate.rs); the
//! device lock is free while the worker applies. The counters say how often and how long.
//! `CARAXES_SHADOW_UNTRAPPED_HOLD=0` turns the rule off, for the A/B reproduction only.
//!
//! nvidia-uvm's page-table updates of a CUDA VA space never reach that register: they end
//! in a `MEM_OP` TLB invalidate on UVM's own CE channel, which the software executor runs,
//! followed by the semaphore release UVM waits for. Such an invalidate of any shadow root
//! stops its push until the worker applied the root's plan ([`memop_holds`], channel.rs
//! `doorbell_holding`, docs/research/shadow-fault-race-2026-09-28.md). The same switch
//! turns it off.
//!
//! The same module counts the evidence: a doorbell of a shadow channel whose host channel
//! already fetched entries past the GPPut of the device's last forward ran ahead of the
//! doorbell ([`ran_ahead`]); with plans pending at that moment it may have run on a mirror
//! older than the guest's tables.

use std::collections::BTreeSet;
use std::time::Duration;

/// The switch (default on).
pub const ENV: &str = "CARAXES_SHADOW_UNTRAPPED_HOLD";
/// Findings logged at most (the counters are complete).
const LOG_FIRST: usize = 16;

/// `CARAXES_SHADOW_UNTRAPPED_HOLD`: unset, empty, `1`/`on` on; `0`/`off` off; anything else
/// an error (the caller then keeps the rule on: fail closed).
pub fn parse_switch(v: Option<&str>) -> Result<bool, String> {
    match v.map(str::trim) {
        None | Some("") | Some("1") | Some("on") => Ok(true),
        Some("0") | Some("off") => Ok(false),
        Some(x) => Err(format!("{ENV}={x:?}: expected 1, on, 0 or off")),
    }
}

/// Whether a guest MMU invalidate of `pdb` (`None`: `ALL_PDB`) must be planned and applied
/// before it returns: it names the shadow root of a mirrored channel whose USERD the guest
/// writes without a trap.
pub fn must_hold(pdb: Option<u64>, untrapped: &BTreeSet<u64>) -> bool {
    match pdb {
        Some(r) => untrapped.contains(&r),
        None => !untrapped.is_empty(),
    }
}

/// The roots a held invalidate of `pdb` (`None`: `ALL_PDB`) waits for, when it holds
/// ([`must_hold`]): the root it names, or for `ALL_PDB` every root whose GPPut store does not
/// trap. Only those host channels can fetch the guest's next work without a doorbell; every
/// other root's channel runs after a doorbell barrier that plans and waits for its root
/// (mirror.rs `gr_shadow_doorbell`), so the invalidate's completion does not wait for it.
pub fn held_roots(pdb: Option<u64>, untrapped: &BTreeSet<u64>) -> Vec<u64> {
    match pdb {
        Some(r) => vec![r],
        None => untrapped.iter().copied().collect(),
    }
}

/// Whether a `MEM_OP` TLB invalidate of page directory `pdb` (`None`: every one) that the
/// software executor runs must stop its push until the shadow is applied: it names one of
/// the shadow `roots`. nvidia-uvm updates the page tables of a CUDA VA space from its own
/// CE channels (they carry `GP100_UVM_SW` and stay in software) and never through the
/// invalidate register, so this is the last point the device sees before the guest
/// submits work on the new mappings (docs/research/shadow-fault-race-2026-09-28.md). Unlike
/// [`must_hold`] it holds every shadow root: a trapped GPPut store waits only for plans
/// already made, and nothing else plans a root at a `MEM_OP` invalidate.
pub fn memop_holds(pdb: Option<crate::fb::Target>, roots: &BTreeSet<u64>) -> bool {
    match pdb {
        Some(crate::fb::Target::Fb(r)) => roots.contains(&r),
        Some(_) => false,
        None => !roots.is_empty(),
    }
}

/// The shadow `roots` that the `MEM_OP` invalidates `inv` name ([`memop_holds`]).
pub fn memop_roots(inv: &[Option<crate::fb::Target>], roots: &BTreeSet<u64>) -> BTreeSet<u64> {
    let mut out = BTreeSet::new();
    for pdb in inv {
        match pdb {
            Some(crate::fb::Target::Fb(r)) if roots.contains(r) => {
                out.insert(*r);
            }
            None => out.extend(roots.iter().copied()),
            _ => {}
        }
    }
    out
}

/// Whether a host channel fetched GPFIFO entries the device never forwarded: at a doorbell,
/// its GPGet `get` has moved past `fwd`, the GPPut the device saw at its last forward,
/// towards the guest's current GPPut `put`, on a ring of `entries` entries.
pub fn ran_ahead(fwd: u32, get: u32, put: u32, entries: u32) -> bool {
    if entries == 0 || fwd >= entries || get >= entries || put >= entries {
        return false;
    }
    let dist = |a: u32, b: u32| (b + entries - a) % entries;
    put != fwd && get != fwd && dist(fwd, get) <= dist(fwd, put)
}

/// Counters for the exit summary.
#[derive(Default, Debug, Clone)]
pub struct HoldStats {
    /// Invalidates held until the worker applied their plan, and the time.
    pub held: u64,
    pub held_total: Duration,
    pub held_max: Duration,
    /// Software `MEM_OP` invalidates of a shadow root whose push waited until the worker
    /// applied the root's plan ([`memop_holds`]), and the time.
    pub memop_held: u64,
    pub memop_total: Duration,
    pub memop_max: Duration,
    /// Doorbells whose host channel had run ahead of them, and of those the ones that found
    /// plans pending (lazy or in the worker).
    pub ahead: u64,
    pub ahead_pending: u64,
    pub first: Vec<String>,
}

impl HoldStats {
    pub fn note_held(&mut self, d: Duration) {
        self.held += 1;
        self.held_total += d;
        self.held_max = self.held_max.max(d);
    }

    pub fn note_memop_held(&mut self, d: Duration) {
        self.memop_held += 1;
        self.memop_total += d;
        self.memop_max = self.memop_max.max(d);
    }

    /// A doorbell that found its host channel ahead; `pending` whether plans were pending.
    /// Returns the log line of the first few with plans pending.
    pub fn note_ahead(&mut self, pending: bool, line: impl FnOnce() -> String) -> Option<String> {
        self.ahead += 1;
        if !pending {
            return None;
        }
        self.ahead_pending += 1;
        (self.first.len() < LOG_FIRST).then(|| {
            let l = line();
            self.first.push(l.clone());
            l
        })
    }

    pub fn summary(&self, on: bool) -> String {
        format!(
            "untrapped GPPut: rule {} ({ENV}); {} invalidate(s) held until applied ({} ms in total, longest {} us); {} software MEM_OP invalidate(s) of a shadow root held until applied ({} ms in total, longest {} us); {} doorbell(s) found the host channel ahead of them, {} of them with plans pending (docs/caraxes/bar1-pool.md §9)",
            if on { "on" } else { "OFF" },
            self.held,
            self.held_total.as_millis(),
            self.held_max.as_micros(),
            self.memop_held,
            self.memop_total.as_millis(),
            self.memop_max.as_micros(),
            self.ahead,
            self.ahead_pending
        )
    }
}

/// Seconds since the epoch with milliseconds, for lines a reader correlates with the host
/// kernel log (`journalctl -o short-unix`).
pub fn wall() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}", t.as_secs(), t.subsec_millis())
}

#[cfg(test)]
mod tests {
    /// A held invalidate waits for its own root, or for ALL_PDB for the untrapped roots
    /// only (not every shadow root).
    #[test]
    fn a_held_invalidate_waits_for_its_scope_only() {
        let untrapped: BTreeSet<u64> = [0xA000, 0xB000].into_iter().collect();
        assert!(must_hold(Some(0xA000), &untrapped));
        assert_eq!(held_roots(Some(0xA000), &untrapped), vec![0xA000]);
        assert!(must_hold(None, &untrapped));
        assert_eq!(held_roots(None, &untrapped), vec![0xA000, 0xB000]);
        assert!(!must_hold(Some(0xC000), &untrapped), "a trapped root is not held");
        assert!(!must_hold(None, &BTreeSet::new()));
    }

    use super::*;

    #[test]
    fn the_switch_is_on_unless_turned_off() {
        assert_eq!(parse_switch(None), Ok(true));
        assert_eq!(parse_switch(Some("")), Ok(true));
        assert_eq!(parse_switch(Some("1")), Ok(true));
        assert_eq!(parse_switch(Some("0")), Ok(false));
        assert_eq!(parse_switch(Some("off")), Ok(false));
        assert!(parse_switch(Some("maybe")).is_err());
    }

    /// An invalidate is held when it names a root with an untrapped USERD, or every root
    /// while there is one; never when no mirrored channel has an untrapped USERD.
    #[test]
    fn an_invalidate_of_a_root_with_an_untrapped_userd_is_held() {
        let none = BTreeSet::new();
        let one: BTreeSet<u64> = [0x1_2980_0000].into();
        assert!(!must_hold(Some(0x1_2980_0000), &none));
        assert!(!must_hold(None, &none));
        assert!(must_hold(Some(0x1_2980_0000), &one));
        assert!(!must_hold(Some(0x1_2990_0000), &one));
        assert!(must_hold(None, &one));
    }

    /// A software `MEM_OP` invalidate holds every shadow root it names, untrapped or not;
    /// one of every page directory holds them all; a sysmem or unknown directory none.
    #[test]
    fn a_memop_invalidate_holds_the_shadow_roots_it_names() {
        use crate::fb::Target;
        let none = BTreeSet::new();
        let roots: BTreeSet<u64> = [0xef20_1000, 0x1_2980_0000].into();
        assert!(memop_holds(Some(Target::Fb(0xef20_1000)), &roots));
        assert!(!memop_holds(Some(Target::Fb(0xef20_0000)), &roots));
        assert!(!memop_holds(Some(Target::Sys(0xef20_1000)), &roots));
        assert!(memop_holds(None, &roots));
        assert!(!memop_holds(None, &none));
        assert!(!memop_holds(Some(Target::Fb(0xef20_1000)), &none));
        // The run of 2026-09-28: UVM's invalidates of root 0xef201000, and its fault-replay
        // invalidates that name page directory 0.
        let inv = [Some(Target::Fb(0xef20_1000)), Some(Target::Fb(0)), Some(Target::Fb(0xef20_1000))];
        assert_eq!(memop_roots(&inv, &roots), [0xef20_1000].into());
        assert_eq!(memop_roots(&[None], &roots), roots);
        assert!(memop_roots(&[Some(Target::Fb(0))], &roots).is_empty());
        assert!(memop_roots(&[None], &none).is_empty());
    }

    #[test]
    fn a_channel_ran_ahead_when_gpget_passed_the_last_forward() {
        // Forwarded at 10, the guest published up to 14 since.
        assert!(!ran_ahead(10, 10, 14, 1024), "still at the forward");
        assert!(ran_ahead(10, 12, 14, 1024));
        assert!(ran_ahead(10, 14, 14, 1024), "consumed all before the doorbell");
        assert!(!ran_ahead(10, 9, 14, 1024), "behind the forward: older work");
        assert!(!ran_ahead(10, 10, 10, 1024), "nothing new");
        // Across the wrap of the ring.
        assert!(ran_ahead(1022, 1, 3, 1024));
        assert!(!ran_ahead(1022, 5, 3, 1024));
        // Values outside the ring are never evidence.
        assert!(!ran_ahead(10, 2000, 14, 1024));
        assert!(!ran_ahead(10, 12, 14, 0));
    }

    #[test]
    fn only_the_first_pending_cases_are_logged() {
        let mut s = HoldStats::default();
        assert!(s.note_ahead(false, || "x".into()).is_none());
        for _ in 0..LOG_FIRST {
            assert!(s.note_ahead(true, || "y".into()).is_some());
        }
        assert!(s.note_ahead(true, || "z".into()).is_none());
        assert_eq!((s.ahead, s.ahead_pending), (LOG_FIRST as u64 + 2, LOG_FIRST as u64 + 1));
        s.note_held(Duration::from_micros(300));
        s.note_held(Duration::from_micros(100));
        assert_eq!((s.held, s.held_max), (2, Duration::from_micros(300)));
        assert!(s.summary(true).contains("2 invalidate(s) held"));
        s.note_memop_held(Duration::from_micros(3123));
        assert_eq!((s.memop_held, s.memop_max), (1, Duration::from_micros(3123)));
        assert!(s
            .summary(true)
            .contains("1 software MEM_OP invalidate(s) of a shadow root held until applied (3 ms in total, longest 3123 us)"));
    }
}

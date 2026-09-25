// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
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
//! cost is the apply time at those invalidates; the counters say how often and how long.
//! `CARAXES_SHADOW_UNTRAPPED_HOLD=0` turns the rule off, for the A/B reproduction only.
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

/// Whether an invalidate the rule plans now must also wait for the worker's apply: only
/// when a plan it submitted may add or change a mapping (`Plan::adds`). A plan that only
/// removes mappings cannot make a host channel that runs ahead meet a VA the guest maps
/// and the shadow does not (the FAULT_PDE of bar1-pool.md §9): until the worker applies it,
/// the shadow holds every mapping of the guest's new tables with the same target, and
/// more. The extra mappings are ones the guest dropped: the guest issues no work on them,
/// and each was validated into this VM's reservation or registered RAM when it was made
/// (D16/D17), so a stale one reaches only this VM's memory, as under lazy mode's deferral.
pub fn must_wait(adds: bool) -> bool {
    adds
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
    /// Invalidates of such a root whose plans only removed mappings: planned now and not
    /// waited for ([`must_wait`]), and the planning time.
    pub passed: u64,
    pub passed_total: Duration,
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

    pub fn note_passed(&mut self, d: Duration) {
        self.passed += 1;
        self.passed_total += d;
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
            "untrapped GPPut: rule {} ({ENV}); {} invalidate(s) held until applied ({} ms in total, longest {} us); {} only removed mappings and were not held ({} ms planning); {} doorbell(s) found the host channel ahead of them, {} of them with plans pending (docs/caraxes/bar1-pool.md §9)",
            if on { "on" } else { "OFF" },
            self.held,
            self.held_total.as_millis(),
            self.held_max.as_micros(),
            self.passed,
            self.passed_total.as_millis(),
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
        s.note_passed(Duration::from_micros(40));
        assert_eq!((s.held, s.passed), (2, 1));
        assert!(s.summary(true).contains("1 only removed mappings and were not held"));
    }

    /// The scenario of bar1-pool.md §9 with the refinement. A shadow root whose channel's
    /// USERD is untrapped holds a buffer; the guest frees it (an invalidate that only
    /// removes leaves), then allocates a fresh 2 MiB region and launches a kernel that
    /// writes it (an invalidate that adds a PDE). A host channel that runs ahead meets the
    /// shadow as it was before the invalidate's plan was applied: after the free that
    /// shadow still holds every leaf of the guest's tables with the same target (no fault
    /// possible, so no wait), after the allocation it lacks the fresh region's PDE (the
    /// FAULT_PDE on a GR write, so the invalidate is held).
    #[test]
    fn a_plan_that_only_removes_mappings_cannot_fault_a_run_ahead_channel() {
        use crate::fb::{FakeFb, Target};
        use crate::guestmem::GuestMemory;
        use crate::shadow::ShadowFront;
        const BASE: u64 = 0x1_0000_0000;
        const END: u64 = 0x2_0000_0000;
        let mut fb = FakeFb::new(END + (64 << 20));
        let vid = |a: u64| ((a >> 12) << 8) | 0b010;
        let pte = |a: u64| ((a >> 12) << 8) | 1 | (6u64 << 56);
        let (root, pd2, pd1, pd0) = (BASE, BASE + 0x1000, BASE + 0x2000, BASE + 0x3000);
        let (pt_a, pt_c) = (BASE + 0x4000, BASE + 0x5000);
        fb.write_u64(root, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        // Region 0 (VA 0 .. 2 MiB): the pushbuffer at VA 0x1000 and a buffer at 0x2000.
        fb.write_u64(pd0 + 8, vid(pt_a));
        fb.write_u64(pt_a + 8, pte(BASE + 0x10_0000));
        fb.write_u64(pt_a + 16, pte(BASE + 0x11_0000));
        let mem = GuestMemory::new();
        let mut front = ShadowFront::new(root);
        let leaves = |fb: &FakeFb| -> Vec<(u64, Target, u64)> {
            fb.with_walk(Target::Fb(root), |c| c.all_leaves())
                .unwrap_or_default()
                .into_iter()
                .map(|(va, t, size, _)| (va, t, size))
                .collect()
        };
        // Would a GPU write at `va` resolve in `shadow` to what the guest's tables say?
        let resolves = |shadow: &[(u64, Target, u64)], guest: &[(u64, Target, u64)], va: u64| {
            let g = guest.iter().find(|(v, _, s)| va >= *v && va < v + s);
            let m = shadow.iter().find(|(v, _, s)| va >= *v && va < v + s);
            g.is_some() && g == m
        };
        let first = front.plan(&fb, &mem, BASE, END, 1, &[], &[]).unwrap();
        assert!(first.adds, "the first plan maps everything");
        let mirrored = leaves(&fb); // what the worker holds once `first` is applied

        // cuMemFree of the buffer: its PTE cleared, the invalidate follows.
        fb.write_u64(pt_a + 16, 0);
        let free = front.plan(&fb, &mem, BASE, END, 1, &[], &[]).unwrap();
        assert!(!free.adds && !must_wait(free.adds), "a free is not held");
        let guest = leaves(&fb);
        // Every VA the guest still maps resolves in the shadow before the free is applied.
        for (va, _, _) in &guest {
            assert!(resolves(&mirrored, &guest, *va), "{va:#x} would fault");
        }
        let mirrored = guest.clone(); // the free applied

        // cuMemAlloc of a fresh 2 MiB region (VA 2 MiB .. 4 MiB, a new PD0 entry) and a
        // kernel that writes it: the §9 case.
        fb.write_u64(pd0 + 16 + 8, vid(pt_c));
        fb.write_u64(pt_c, pte(BASE + 0x40_0000));
        let alloc = front.plan(&fb, &mem, BASE, END, 1, &[], &[]).unwrap();
        assert!(alloc.adds && must_wait(alloc.adds), "an allocation is held");
        let guest = leaves(&fb);
        assert!(!resolves(&mirrored, &guest, 0x20_0000), "unheld: FAULT_PDE at the fresh region");

        // A remap of the same VA to another page is a change, and held too.
        fb.write_u64(pt_a + 8, pte(BASE + 0x12_0000));
        let remap = front.plan(&fb, &mem, BASE, END, 1, &[], &[]).unwrap();
        assert!(remap.adds);
    }
}

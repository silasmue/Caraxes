// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The guest's TLB invalidates against the VAS roots the device walks (fix 2 of run P17,
//! alpha-2026-09-27/windows-plain/P17-owned-compute-cta/ANALYSIS.md).
//!
//! Every guest TLB invalidate names the page directory the guest really uses: `MEM_OP_C/D`
//! in a pushbuffer (clc46f.h:134-170) and the `MMU_INVALIDATE_PDB` register pair
//! (dev_vm.h:120-129). In P17 a second `COPY_SERVER_RESERVED_PDES` replaced a Windows VAS
//! root with a PD1-level table. The guest went on invalidating its old root, the shadow
//! walked the new one, the GPFIFOs were never mapped, and every host GR channel took Xid 31
//! `FAULT_PDE` at its first fetch. Fix 1 keeps the root in that case (channel.rs
//! `record_server_reserved_pdes`); this watch catches the class of bug.
//!
//! The rule: a VAS whose guest, **after** the device recorded its current root, named one of
//! the VAS's former roots and has not named the current root since, is walked from a root
//! the guest does not use. The dispatcher then logs an ERROR and holds the host doorbells of
//! the VAS's channels (ctrl.rs `doorbell_chid_inner`) until the guest names the current root
//! again. No evidence, no hold: a VAS whose root never changed, a guest that named no former
//! root since the change, and an `ALL_PDB` invalidate never hold anything.

use std::collections::{BTreeMap, BTreeSet};

use crate::fb::Target;

/// Former roots kept per VAS (the oldest drop out first).
const HISTORY: usize = 8;

/// A root as a map key: aperture (0 vidmem, 1 sysmem) and address.
type RootKey = (u8, u64);

fn key_of(t: Target) -> Option<RootKey> {
    match t {
        Target::Fb(a) => Some((0, a)),
        Target::Sys(a) => Some((1, a)),
        _ => None,
    }
}

/// The page directory of a `MEM_OP` TLB invalidate (`MEM_OP_D` OPERATION 9 or 0xa):
/// `MEM_OP_C` TLB_INVALIDATE_PDB 0:0 ONE (0) or ALL (1), PDB_APERTURE 11:10 (0 vidmem, 2 and
/// 3 sysmem), PDB_ADDR_LO 31:12; `MEM_OP_D` PDB_ADDR_HI 26:0 (clc46f.h:134-170). `None`: every
/// page directory, or an aperture no GPU walks.
pub fn memop_pdb(c: u32, d: u32) -> Option<Target> {
    if c & 1 != 0 {
        return None;
    }
    let pa = (c as u64 & 0xFFFF_F000) | ((d as u64 & 0x7FF_FFFF) << 32);
    match (c >> 10) & 3 {
        0 => Some(Target::Fb(pa)),
        2 | 3 => Some(Target::Sys(pa)),
        _ => None,
    }
}

/// The page directory of a register TLB invalidate: `MMU_INVALIDATE_PDB` ADDR 31:4 in 4 KiB
/// units, APERTURE 1:1 (0 vidmem, 1 sysmem); `MMU_INVALIDATE_UPPER_PDB` ADDR 19:0 are bits
/// 51:32 (dev_vm.h:120-129).
pub fn register_pdb(pdb: u32, upper: u32) -> Target {
    let pa = ((pdb as u64 >> 4) << 12) | ((upper as u64 & 0xF_FFFF) << 32);
    if pdb & 2 != 0 {
        Target::Sys(pa)
    } else {
        Target::Fb(pa)
    }
}

/// The roots one VAS key was given.
#[derive(Clone, Debug, Default)]
struct History {
    /// Distinct roots, oldest first; the last is the current one.
    roots: Vec<Target>,
    /// The invalidate sequence number when the current root was recorded: only a later
    /// naming of a former root is evidence.
    since: u64,
}

/// A VAS walked from a root the guest's invalidates no longer name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mismatch {
    /// The former root the guest named last.
    pub named: Target,
    /// The current root: what the device (and a shadow built from it) walks.
    pub walked: Target,
    /// Guest invalidates of the former root since the current one was recorded.
    pub count: u64,
}

#[derive(Debug, Default)]
pub struct PdbWatch {
    vas: BTreeMap<(u32, u32), History>,
    /// Per named root: the sequence number of the last guest invalidate naming it.
    named: BTreeMap<RootKey, u64>,
    /// Per named root: invalidates naming it since the watch last saw its VAS move away.
    counts: BTreeMap<RootKey, u64>,
    seq: u64,
    /// Whether any VAS root ever changed (the doorbell's fast path skips the check before).
    moved: bool,
    /// Mismatches already reported, one ERROR per (VAS, former root, current root).
    reported: BTreeSet<((u32, u32), RootKey, RootKey)>,
    /// Page-directory invalidates seen, `ALL_PDB` ones among them, doorbells held.
    pub invalidates: u64,
    pub all_pdb: u64,
    pub holds: u64,
}

impl PdbWatch {
    /// The device recorded `root` for VAS key `vas` (`None`: unknown again, history kept).
    pub fn set_root(&mut self, vas: (u32, u32), root: Option<Target>) {
        let Some(r) = root.filter(|r| key_of(*r).is_some()) else {
            return;
        };
        let h = self.vas.entry(vas).or_default();
        if h.roots.last() == Some(&r) {
            return;
        }
        if !h.roots.is_empty() {
            self.moved = true;
        }
        h.roots.retain(|x| *x != r);
        h.roots.push(r);
        if h.roots.len() > HISTORY {
            h.roots.remove(0);
        }
        h.since = self.seq;
        // Counts start at the change: only namings after it are evidence.
        for f in &h.roots[..h.roots.len() - 1] {
            if let Some(k) = key_of(*f) {
                self.counts.remove(&k);
            }
        }
    }

    /// The VAS moved to another key (a DUP alias inherited it, channel.rs
    /// `rehome_duplicated_vas`).
    pub fn rekey(&mut self, from: (u32, u32), to: (u32, u32)) {
        if let Some(h) = self.vas.remove(&from) {
            self.vas.insert(to, h);
        }
    }

    /// Key `to` holds the same VAS object as `from` (a device keeps its default VAS after
    /// the handle's FREE, channel.rs `retain_device_default`).
    pub fn copy(&mut self, from: (u32, u32), to: (u32, u32)) {
        if let Some(h) = self.vas.get(&from).cloned() {
            self.vas.insert(to, h);
        }
    }

    /// Forget the VAS keys `live` refuses (FREE) and the namings of roots no VAS remembers.
    pub fn retain(&mut self, live: impl Fn(&(u32, u32)) -> bool) {
        self.vas.retain(|k, _| live(k));
        self.reported.retain(|(k, _, _)| live(k));
        let known: BTreeSet<RootKey> = self
            .vas
            .values()
            .flat_map(|h| h.roots.iter().filter_map(|r| key_of(*r)))
            .collect();
        self.named.retain(|k, _| known.contains(k));
        self.counts.retain(|k, _| known.contains(k));
    }

    /// A guest TLB invalidate of `pdb` (`None`: every page directory, which names none).
    pub fn note_invalidate(&mut self, pdb: Option<Target>) {
        self.invalidates += 1;
        let Some(k) = pdb.and_then(key_of) else {
            self.all_pdb += 1;
            return;
        };
        self.seq += 1;
        self.named.insert(k, self.seq);
        *self.counts.entry(k).or_default() += 1;
    }

    /// Whether VAS key `vas`, whose current root is `walked`, is walked from a root the
    /// guest's invalidates no longer name (see the module text).
    pub fn check(&self, vas: (u32, u32), walked: Target) -> Option<Mismatch> {
        if !self.moved {
            return None;
        }
        let h = self.vas.get(&vas)?;
        // A root the watch did not see recorded: no evidence either way.
        if h.roots.last() != Some(&walked) || h.roots.len() < 2 {
            return None;
        }
        let current = key_of(walked)
            .and_then(|k| self.named.get(&k))
            .copied()
            .unwrap_or(0);
        h.roots[..h.roots.len() - 1]
            .iter()
            .filter_map(|f| {
                let k = key_of(*f)?;
                let at = *self.named.get(&k)?;
                (at > h.since && at > current).then_some((at, *f, k))
            })
            .max_by_key(|(at, _, _)| *at)
            .map(|(_, named, k)| Mismatch {
                named,
                walked,
                count: self.counts.get(&k).copied().unwrap_or(0),
            })
    }

    /// Whether mismatch `m` of VAS `vas` is reported for the first time (one ERROR each).
    pub fn first_report(&mut self, vas: (u32, u32), m: &Mismatch) -> bool {
        match (key_of(m.named), key_of(m.walked)) {
            (Some(n), Some(w)) => self.reported.insert((vas, n, w)),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VAS: (u32, u32) = (0xc1d0_002d, 0xff00_0850);
    const OLD: Target = Target::Fb(0x3b48_5000);
    const PD1: Target = Target::Fb(0x0d3f_0000);

    #[test]
    fn memop_c_bit_0_is_one_pdb_not_all() {
        // P17's Windows paging channel and a Linux UVM channel: PDB ONE, vidmem.
        assert_eq!(memop_pdb(0x3b48_52a0, 0x4800_0000), Some(OLD));
        assert_eq!(
            memop_pdb(0x7023_f100, 0x4800_0000),
            Some(Target::Fb(0x7023_f000))
        );
        // ADDR_HI in MEM_OP_D 26:0 (the OPERATION field 31:27 does not leak in).
        assert_eq!(
            memop_pdb(0x3b48_5000, 0x4800_0001),
            Some(Target::Fb(0x1_3b48_5000))
        );
        // ALL, and the sysmem apertures.
        assert_eq!(memop_pdb(0x3b48_5001, 0), None);
        assert_eq!(memop_pdb(0x0012_3800, 0), Some(Target::Sys(0x12_3000)));
        assert_eq!(memop_pdb(0x0012_3c00, 0), Some(Target::Sys(0x12_3000)));
        assert_eq!(memop_pdb(0x0012_3400, 0), None);
    }

    #[test]
    fn register_pdb_is_addr_31_4_in_pages_and_aperture_1_1() {
        // P17: `mmu invalidate: pdb 0x3b485000`, written as 0x3b485 << 4.
        assert_eq!(register_pdb(0x003b_4850, 0), OLD);
        assert_eq!(register_pdb(0x003b_4852, 1), Target::Sys(0x1_3b48_5000));
    }

    /// The P17 shape: the root moves to a PD1 table, and the guest goes on invalidating the
    /// old root. Held until the guest names the root the device walks.
    #[test]
    fn a_former_root_named_after_the_change_is_a_mismatch() {
        let mut w = PdbWatch::default();
        w.set_root(VAS, Some(OLD));
        w.note_invalidate(Some(OLD));
        assert_eq!(w.check(VAS, OLD), None);
        w.set_root(VAS, Some(PD1));
        // Namings before the change are no evidence.
        assert_eq!(w.check(VAS, PD1), None);
        w.note_invalidate(Some(OLD));
        w.note_invalidate(Some(OLD));
        let m = w.check(VAS, PD1).expect("mismatch");
        assert_eq!((m.named, m.walked, m.count), (OLD, PD1, 2));
        assert!(w.first_report(VAS, &m));
        assert!(!w.first_report(VAS, &m));
        // Other roots and ALL_PDB change nothing.
        w.note_invalidate(Some(Target::Fb(0x3fe0_1000)));
        w.note_invalidate(None);
        assert!(w.check(VAS, PD1).is_some());
        assert_eq!((w.invalidates, w.all_pdb), (5, 1));
        // The guest names the walked root: released.
        w.note_invalidate(Some(PD1));
        assert_eq!(w.check(VAS, PD1), None);
        // ...and the old one again, later: held again.
        w.note_invalidate(Some(OLD));
        assert!(w.check(VAS, PD1).is_some());
    }

    /// A legitimate move (SET_PAGE_DIRECTORY): the guest names the new root from then on.
    #[test]
    fn a_moved_root_the_guest_follows_passes() {
        let mut w = PdbWatch::default();
        let new = Target::Fb(0x3b49_9000);
        w.set_root(VAS, Some(OLD));
        assert!(!w.moved);
        w.note_invalidate(Some(OLD));
        w.set_root(VAS, Some(new));
        assert!(w.moved);
        w.note_invalidate(Some(new));
        assert_eq!(w.check(VAS, new), None);
        // A root the watch never saw recorded (another path set it): no evidence.
        w.note_invalidate(Some(OLD));
        assert_eq!(w.check(VAS, Target::Fb(0x1000)), None);
        // Unknown again (UNSET_PAGE_DIRECTORY) keeps the history: the naming of OLD above,
        // after the move, still counts against `new`.
        w.set_root(VAS, None);
        assert!(w.check(VAS, new).is_some());
    }

    /// A root that moves back (A -> B -> A) keeps B as the only former root.
    #[test]
    fn a_root_that_moves_back_is_current_again() {
        let mut w = PdbWatch::default();
        w.set_root(VAS, Some(OLD));
        w.set_root(VAS, Some(PD1));
        w.set_root(VAS, Some(OLD));
        w.note_invalidate(Some(OLD));
        assert_eq!(w.check(VAS, OLD), None);
        w.note_invalidate(Some(PD1));
        assert_eq!(w.check(VAS, OLD).map(|m| m.named), Some(PD1));
    }

    /// Keys follow the registry: a DUP heir inherits the history, a device default copies
    /// it, a FREE forgets it and the namings of roots no VAS remembers.
    #[test]
    fn keys_follow_rekey_copy_and_free() {
        let mut w = PdbWatch::default();
        let heir = (0xc1d0_0032, 0x99);
        let dev = (0xc1d0_0032, 0x80);
        w.set_root(VAS, Some(OLD));
        w.set_root(VAS, Some(PD1));
        w.note_invalidate(Some(OLD));
        w.rekey(VAS, heir);
        assert_eq!(w.check(VAS, PD1), None);
        assert!(w.check(heir, PD1).is_some());
        w.copy(heir, dev);
        assert!(w.check(dev, PD1).is_some());
        w.retain(|k| *k != heir && *k != dev);
        assert_eq!(w.check(heir, PD1), None);
        assert!(w.named.is_empty() && w.counts.is_empty());
        // A new VAS on a recycled root page starts clean.
        w.set_root(VAS, Some(PD1));
        w.note_invalidate(Some(OLD));
        assert_eq!(w.check(VAS, PD1), None);
    }

    #[test]
    fn history_is_bounded() {
        let mut w = PdbWatch::default();
        for i in 0..20u64 {
            w.set_root(VAS, Some(Target::Fb(0x1000 * (i + 1))));
        }
        assert_eq!(w.vas[&VAS].roots.len(), HISTORY);
        // Not an FB or sysmem root: ignored.
        w.set_root(VAS, Some(Target::Sparse));
        assert_eq!(w.vas[&VAS].roots.last(), Some(&Target::Fb(0x14000)));
    }
}

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Incremental walk of one guest GMMU tree (docs/caraxes/mapping.md §1).
//!
//! The shadow VA space (shadow.rs) mirrors the leaves of a guest root. Before this module
//! every mapping change re-walked the whole tree through uncached BAR1 reads (about 30 ms
//! for a CUDA context, 0.5 s for half a million leaves). The cache keeps, per root, a copy
//! of every directory table it read, the leaves of every 2 MiB unit (one PD0 entry), and
//! for every table page in VRAM the nodes that read it. The FB store sees every guest write
//! to VRAM (BAR1/BAR2/PRAMIN are trapped), so `FakeFb::note_write` marks the written table
//! pages dirty, and [`WalkCache::update`] re-reads only those: a changed directory entry
//! drops the old subtree and walks the new one, a changed page table recomputes its unit.
//! The result is the set of units whose leaves may have changed, so the mirror diffs only
//! those. A table in guest sysmem cannot be watched (its writes never reach the device); a
//! tree with one is re-walked in full every time, as before.
//!
//! Geometry (Turing GMMU v2, 49-bit VA, fb-model.md §1.5): PD3 (the root, 4 entries, VA
//! 48:47) -> PD2 (512, 46:38) -> PD1 (512, 37:29) -> PD0 (256 dual entries of 16 bytes,
//! 28:21) -> a 64 KiB big-page table (32 entries, 20:16, 256 bytes) and a 4 KiB small-page
//! table (512 entries, 20:12). A valid PD0 low qword is a 2 MiB PTE.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::fb::Target;

/// One present leaf: `(va, target, size, kind)`.
pub type Leaf = (u64, Target, u64, u8);

/// A 2 MiB unit: one PD0 entry.
pub const UNIT: u64 = 2 << 20;

/// Reads `n` consecutive qwords of a page table.
pub trait TableReader {
    fn block(&self, table: Target, n: usize) -> Option<Vec<u64>>;
}

/// Who read a table page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Use {
    /// A directory table of `level` (3 = root .. 0 = PD0) covering VAs from `va`.
    Dir(u8, u64),
    /// The page tables of the 2 MiB unit at `va`.
    Unit(u64),
}

struct Dir {
    at: Target,
    entries: Vec<u64>,
}

struct Unit {
    /// The PD0 dual entry the unit was built from.
    lo: u64,
    hi: u64,
    /// The FB pages of its big and small page tables (for the use map).
    pages: Vec<u64>,
    leaves: Vec<Leaf>,
}

/// Qwords of a directory table of `level`.
fn entries_of(level: u8) -> usize {
    if level == 3 {
        4
    } else {
        512
    }
}

/// VA span of one entry of a directory table of `level` (PD0: one dual entry).
fn entry_span(level: u8) -> u64 {
    match level {
        3 => 1 << 47,
        2 => 1 << 38,
        1 => 1 << 29,
        _ => UNIT,
    }
}

/// VA span of a whole directory table of `level`.
fn table_span(level: u8) -> u64 {
    match level {
        3 => 1 << 49,
        0 => UNIT * 256,
        l => entry_span(l) * 512,
    }
}

/// Follow a PDE (PD3/PD2/PD1 entry or the small-table half of a dual PDE).
pub fn next_table(entry: u64) -> Option<Target> {
    match (entry >> 1) & 3 {
        1 => Some(Target::Fb(((entry >> 8) & 0x1FF_FFFF) << 12)),
        2 | 3 => Some(Target::Sys(((entry >> 8) & 0x3FFF_FFFF_FFFF) << 12)),
        _ => None,
    }
}

/// The big-page table of a dual PDE's low qword.
pub fn big_table(lo: u64) -> Option<Target> {
    match (lo >> 1) & 3 {
        1 => Some(Target::Fb(((lo >> 4) & 0x1FFF_FFFF) << 8)),
        2 | 3 => Some(Target::Sys(((lo >> 4) & 0x3_FFFF_FFFF_FFFF) << 8)),
        _ => None,
    }
}

/// A valid PTE as a leaf (invalid and sparse entries have nothing physical to mirror).
fn leaf(pte: u64, va: u64, size: u64) -> Option<Leaf> {
    if pte & 1 == 0 {
        return None;
    }
    let kind = (pte >> 56) as u8;
    let t = match (pte >> 1) & 3 {
        0 => Target::Fb(((pte >> 8) & 0x1FF_FFFF) << 12),
        2 | 3 => Target::Sys(((pte >> 8) & 0x3FFF_FFFF_FFFF) << 12),
        _ => return None,
    };
    Some((va, t, size, kind))
}

/// The 4096 bytes of a table page copy (little-endian qwords on x86).
fn bytemuck_bytes(a: &mut [u64; 512]) -> &mut [u8; 4096] {
    // SAFETY: [u64; 512] and [u8; 4096] have the same size, u8 has no alignment
    // requirement, and the borrow is carried over.
    unsafe { &mut *(a as *mut [u64; 512] as *mut [u8; 4096]) }
}

/// What an update did.
#[derive(Debug, Default, Clone)]
pub struct Update {
    /// The whole tree was (re)walked.
    pub full: bool,
    /// Units whose leaves may have changed (ascending).
    pub changed: Vec<u64>,
    /// Table blocks read from the store in this update.
    pub reads: usize,
    /// Dirty table pages the update started from.
    pub dirty_pages: usize,
}

/// The cached walk of one guest root.
pub struct WalkCache {
    root: Target,
    dirs: BTreeMap<(u8, u64), Dir>,
    units: BTreeMap<u64, Unit>,
    /// FB table page (4 KiB page number) -> the nodes that read it.
    uses: HashMap<u64, BTreeSet<Use>>,
    dirty: BTreeSet<u64>,
    built: bool,
    /// A table of this tree lives in guest sysmem: it cannot be watched.
    pub sys_tables: bool,
    leaves: usize,
    /// Leaves cached at most (a guest cannot make the device hold unbounded state);
    /// units past the cap are not cached and `truncated` is set.
    pub max_leaves: usize,
    pub truncated: bool,
    reads: usize,
    /// Write-through copies of the watched VRAM table pages: filled by the first read of a
    /// page and patched with every guest write to it (`note_write`), so an update reads the
    /// store (an uncached BAR1 read, about 0.8 ms per 4 KiB page) only for pages it has not
    /// seen. `CARAXES_SHADOW_REREAD=1` drops a dirty page's copy instead (re-read from VRAM).
    raw: HashMap<u64, Box<[u64; 512]>>,
    pub reread: bool,
}

impl WalkCache {
    pub fn new(root: Target, max_leaves: usize) -> Self {
        WalkCache {
            root,
            dirs: BTreeMap::new(),
            units: BTreeMap::new(),
            uses: HashMap::new(),
            dirty: BTreeSet::new(),
            built: false,
            sys_tables: false,
            leaves: 0,
            max_leaves,
            truncated: false,
            reads: 0,
            raw: HashMap::new(),
            reread: false,
        }
    }

    /// A guest write of `data` at FB address `pa` (inside watched page `page`): patch the
    /// page's copy and mark it dirty.
    pub fn note_write(&mut self, page: u64, pa: u64, data: &[u8]) {
        self.dirty.insert(page);
        if self.reread {
            self.raw.remove(&page);
            return;
        }
        if let Some(r) = self.raw.get_mut(&page) {
            let base = page << 12;
            let bytes: &mut [u8; 4096] = bytemuck_bytes(r);
            let lo = pa.max(base);
            let hi = (pa + data.len() as u64).min(base + 4096);
            if lo < hi {
                let src = &data[(lo - pa) as usize..(hi - pa) as usize];
                bytes[(lo - base) as usize..(hi - base) as usize].copy_from_slice(src);
            }
        }
    }

    /// Whether `page` (4 KiB page number) is a table page this cache watches.
    pub fn watches(&self, page: u64) -> bool {
        self.uses.contains_key(&page)
    }

    /// A write of unknown content reached table page `page`: re-read it at the next update.
    pub fn mark_dirty(&mut self, page: u64) {
        self.dirty.insert(page);
        self.raw.remove(&page);
    }

    /// Whether the next update has work (never built, a dirty page, or unwatchable).
    pub fn is_dirty(&self) -> bool {
        !self.built || self.sys_tables || !self.dirty.is_empty()
    }

    pub fn leaf_count(&self) -> usize {
        self.leaves
    }

    pub fn table_pages(&self) -> usize {
        self.uses.len()
    }

    /// The cached leaves of the unit at `va` (2 MiB aligned).
    pub fn unit_leaves(&self, va: u64) -> &[Leaf] {
        self.units.get(&va).map_or(&[], |u| &u.leaves)
    }

    /// Every unit with at least one leaf.
    pub fn units_with_leaves(&self) -> Vec<u64> {
        self.units
            .iter()
            .filter(|(_, u)| !u.leaves.is_empty())
            .map(|(va, _)| *va)
            .collect()
    }

    fn read(&mut self, r: &dyn TableReader, t: Target, n: usize) -> Option<Vec<u64>> {
        match t {
            Target::Fb(pa) => {
                let page = pa >> 12;
                let off = ((pa & 0xFFF) / 8) as usize;
                if off + n > 512 {
                    self.reads += 1;
                    return r.block(t, n);
                }
                if !self.raw.contains_key(&page) {
                    self.reads += 1;
                    let b = r.block(Target::Fb(page << 12), 512)?;
                    let mut a = Box::new([0u64; 512]);
                    a.copy_from_slice(&b);
                    self.raw.insert(page, a);
                }
                Some(self.raw[&page][off..off + n].to_vec())
            }
            Target::Sys(_) => {
                self.reads += 1;
                self.sys_tables = true;
                r.block(t, n)
            }
            _ => None,
        }
    }

    fn add_use(&mut self, t: Target, u: Use) {
        if let Target::Fb(pa) = t {
            self.uses.entry(pa >> 12).or_default().insert(u);
        }
    }

    fn drop_use(&mut self, page: u64, u: Use) {
        if let Some(s) = self.uses.get_mut(&page) {
            s.remove(&u);
            if s.is_empty() {
                self.uses.remove(&page);
            }
        }
    }

    /// Bring the cache up to date: a full walk when it was never built, when a table is in
    /// sysmem or when `full` is asked for (the always-resync reference), otherwise only the
    /// dirty table pages. Returns the units whose leaves may have changed.
    pub fn update(&mut self, r: &dyn TableReader, full: bool) -> Update {
        self.reads = 0;
        let dirty_pages = self.dirty.len();
        let mut changed = BTreeSet::new();
        let full = full || !self.built || self.sys_tables;
        if full {
            changed.extend(self.units.keys().copied());
            self.dirs.clear();
            self.units.clear();
            self.uses.clear();
            self.dirty.clear();
            self.raw.clear();
            self.leaves = 0;
            self.truncated = false;
            self.sys_tables = false;
            let root = self.root;
            self.walk_dir(r, 3, 0, root, &mut changed);
            self.built = true;
        } else {
            let pages = std::mem::take(&mut self.dirty);
            let mut work: BTreeSet<Use> = BTreeSet::new();
            for p in &pages {
                if let Some(s) = self.uses.get(p) {
                    work.extend(s.iter().copied());
                }
            }
            // Directories top-down (level 3 first), then the page tables of units.
            let mut dirs: Vec<(u8, u64)> = work
                .iter()
                .filter_map(|u| match u {
                    Use::Dir(l, va) => Some((*l, *va)),
                    Use::Unit(_) => None,
                })
                .collect();
            dirs.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            let mut rebuilt = BTreeSet::new();
            for (level, va) in dirs {
                self.refresh_dir(r, level, va, &mut changed, &mut rebuilt);
            }
            for u in work {
                if let Use::Unit(va) = u {
                    if rebuilt.contains(&va) {
                        continue;
                    }
                    if let Some(unit) = self.units.get(&va) {
                        let (lo, hi) = (unit.lo, unit.hi);
                        self.rebuild_unit(r, va, lo, hi, &mut changed);
                        rebuilt.insert(va);
                    }
                }
            }
        }
        let uses = &self.uses;
        self.raw.retain(|p, _| uses.contains_key(p));
        Update {
            full,
            changed: changed.into_iter().collect(),
            reads: self.reads,
            dirty_pages,
        }
    }

    /// Walk the directory table of `level` at `at` covering VAs from `va`, and below it.
    fn walk_dir(
        &mut self,
        r: &dyn TableReader,
        level: u8,
        va: u64,
        at: Target,
        changed: &mut BTreeSet<u64>,
    ) {
        let Some(entries) = self.read(r, at, entries_of(level)) else {
            return;
        };
        self.add_use(at, Use::Dir(level, va));
        self.dirs.insert(
            (level, va),
            Dir {
                at,
                entries: entries.clone(),
            },
        );
        self.walk_children(r, level, va, &entries, None, changed);
    }

    /// The children of directory `(level, va)`, all of them or only entry `only`.
    fn walk_children(
        &mut self,
        r: &dyn TableReader,
        level: u8,
        va: u64,
        entries: &[u64],
        only: Option<usize>,
        changed: &mut BTreeSet<u64>,
    ) {
        if level == 0 {
            for i in 0..256usize {
                if only.is_some_and(|o| o != i) {
                    continue;
                }
                let uva = va + i as u64 * UNIT;
                let (lo, hi) = (entries[2 * i], entries[2 * i + 1]);
                self.build_unit(r, uva, lo, hi, changed);
            }
            return;
        }
        for (i, e) in entries.iter().enumerate() {
            if only.is_some_and(|o| o != i) {
                continue;
            }
            if let Some(t) = next_table(*e) {
                let cva = va + i as u64 * entry_span(level);
                self.walk_dir(r, level - 1, cva, t, changed);
            }
        }
    }

    /// Build the unit at `va` from its PD0 dual entry.
    fn build_unit(
        &mut self,
        r: &dyn TableReader,
        va: u64,
        lo: u64,
        hi: u64,
        changed: &mut BTreeSet<u64>,
    ) {
        let mut leaves = Vec::new();
        let mut pages = Vec::new();
        if lo & 1 == 1 {
            // A 2 MiB page: the low qword is a PTE.
            leaves.extend(leaf(lo, va, UNIT));
        } else {
            let bt = big_table(lo);
            let st = next_table(hi);
            if bt.is_none() && st.is_none() {
                return;
            }
            let big = bt.and_then(|t| {
                if let Target::Fb(pa) = t {
                    pages.push(pa >> 12);
                }
                self.read(r, t, 32)
            });
            if let Some(b) = &big {
                for (i, pte) in b.iter().enumerate() {
                    leaves.extend(leaf(*pte, va | ((i as u64) << 16), 0x1_0000));
                }
            }
            if let Some(t) = st {
                if let Target::Fb(pa) = t {
                    pages.push(pa >> 12);
                }
                if let Some(s) = self.read(r, t, 512) {
                    for (i, pte) in s.iter().enumerate() {
                        // A 4 KiB slot under a valid 64 KiB big PTE is hidden.
                        if big.as_ref().is_some_and(|b| b[i >> 4] & 1 == 1) {
                            continue;
                        }
                        leaves.extend(leaf(*pte, va | ((i as u64) << 12), 0x1000));
                    }
                }
            }
        }
        if self.leaves + leaves.len() > self.max_leaves {
            self.truncated = true;
            leaves.clear();
        }
        pages.sort_unstable();
        pages.dedup();
        for p in &pages {
            self.uses.entry(*p).or_default().insert(Use::Unit(va));
        }
        if !leaves.is_empty() {
            changed.insert(va);
        }
        self.leaves += leaves.len();
        self.units.insert(
            va,
            Unit {
                lo,
                hi,
                pages,
                leaves,
            },
        );
    }

    fn drop_unit(&mut self, va: u64, changed: &mut BTreeSet<u64>) {
        if let Some(u) = self.units.remove(&va) {
            for p in &u.pages {
                self.drop_use(*p, Use::Unit(va));
            }
            if !u.leaves.is_empty() {
                changed.insert(va);
            }
            self.leaves -= u.leaves.len();
        }
    }

    fn rebuild_unit(
        &mut self,
        r: &dyn TableReader,
        va: u64,
        lo: u64,
        hi: u64,
        changed: &mut BTreeSet<u64>,
    ) {
        let old: Vec<Leaf> = self.unit_leaves(va).to_vec();
        let mut scratch = BTreeSet::new();
        self.drop_unit(va, &mut scratch);
        self.build_unit(r, va, lo, hi, &mut scratch);
        if self.unit_leaves(va) != old.as_slice() {
            changed.insert(va);
        }
    }

    /// Drop the directory `(level, va)` and everything under it.
    fn drop_subtree(&mut self, level: u8, va: u64, changed: &mut BTreeSet<u64>) {
        let end = va.saturating_add(table_span(level));
        for l in (0..=level).rev() {
            let keys: Vec<(u8, u64)> = self
                .dirs
                .range((l, va)..(l, end))
                .map(|(k, _)| *k)
                .collect();
            for k in keys {
                if let Some(d) = self.dirs.remove(&k) {
                    if let Target::Fb(pa) = d.at {
                        self.drop_use(pa >> 12, Use::Dir(k.0, k.1));
                    }
                }
            }
        }
        let units: Vec<u64> = self.units.range(va..end).map(|(k, _)| *k).collect();
        for u in units {
            self.drop_unit(u, changed);
        }
    }

    /// Re-read directory `(level, va)` and follow every entry that changed.
    fn refresh_dir(
        &mut self,
        r: &dyn TableReader,
        level: u8,
        va: u64,
        changed: &mut BTreeSet<u64>,
        rebuilt: &mut BTreeSet<u64>,
    ) {
        let Some(d) = self.dirs.get(&(level, va)) else {
            return; // dropped with a parent that changed
        };
        let at = d.at;
        let old = d.entries.clone();
        let Some(new) = self.read(r, at, entries_of(level)) else {
            return;
        };
        if new == old {
            return;
        }
        if let Some(d) = self.dirs.get_mut(&(level, va)) {
            d.entries = new.clone();
        }
        if level == 0 {
            for i in 0..256usize {
                if new[2 * i] == old[2 * i] && new[2 * i + 1] == old[2 * i + 1] {
                    continue;
                }
                let uva = va + i as u64 * UNIT;
                self.rebuild_unit(r, uva, new[2 * i], new[2 * i + 1], changed);
                rebuilt.insert(uva);
            }
            return;
        }
        for i in 0..new.len() {
            if new[i] == old[i] {
                continue;
            }
            let cva = va + i as u64 * entry_span(level);
            self.drop_subtree(level - 1, cva, changed);
            self.walk_children(r, level, va, &new, Some(i), changed);
        }
    }

    /// Every cached leaf, ascending by unit (tests and the full-walk comparison).
    pub fn all_leaves(&self) -> Vec<Leaf> {
        self.units.values().flat_map(|u| u.leaves.iter().copied()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A sparse qword store standing in for VRAM.
    struct Mem(RefCell<BTreeMap<u64, u64>>);

    impl Mem {
        fn new() -> Self {
            Mem(RefCell::new(BTreeMap::new()))
        }
        fn w(&self, pa: u64, v: u64) {
            self.0.borrow_mut().insert(pa, v);
        }
    }

    impl TableReader for Mem {
        fn block(&self, t: Target, n: usize) -> Option<Vec<u64>> {
            let Target::Fb(pa) = t else { return None };
            let m = self.0.borrow();
            Some((0..n as u64).map(|i| *m.get(&(pa + 8 * i)).unwrap_or(&0)).collect())
        }
    }

    fn vid(addr: u64) -> u64 {
        ((addr >> 12) << 8) | 0b010
    }
    fn pte(pa: u64) -> u64 {
        ((pa >> 12) << 8) | (0x06 << 56) | 1
    }

    /// root 0x10000 -> PD2 0x11000 -> PD1 0x12000 -> PD0 0x13000; unit 0 has a small PT
    /// at 0x14000 and a big PT at 0x15000.
    fn tree() -> Mem {
        let m = Mem::new();
        m.w(0x10000, vid(0x11000));
        m.w(0x11000, vid(0x12000));
        m.w(0x12000, vid(0x13000));
        m.w(0x13000, ((0x15000u64 >> 8) << 4) | 0b010);
        m.w(0x13008, vid(0x14000));
        m.w(0x14000 + 8 * 3, pte(0x20000));
        m.w(0x15000 + 8, pte(0x40000));
        m
    }

    #[test]
    fn an_incremental_update_equals_a_full_walk_and_names_the_unit() {
        let m = tree();
        let mut c = WalkCache::new(Target::Fb(0x10000), 1 << 20);
        let u = c.update(&m, false);
        assert!(u.full);
        assert_eq!(u.changed, vec![0]);
        assert_eq!(c.all_leaves().len(), 2);
        assert!(!c.is_dirty());
        // A new small PTE in unit 0: only that table page is re-read.
        m.w(0x14000 + 8 * 5, pte(0x21000));
        assert!(c.watches(0x14));
        c.mark_dirty(0x14);
        let u = c.update(&m, false);
        assert!(!u.full);
        assert_eq!(u.changed, vec![0]);
        assert_eq!(u.reads, 1); // the small table's page (the big table's copy is kept)
        let mut full = WalkCache::new(Target::Fb(0x10000), 1 << 20);
        full.update(&m, true);
        assert_eq!(c.all_leaves(), full.all_leaves());
        // A 2 MiB page in PD0 entry 3: the PD0 page is dirty, unit 3 is new.
        m.w(0x13000 + 16 * 3, pte(0x60_0000));
        c.mark_dirty(0x13);
        let u = c.update(&m, false);
        assert_eq!(u.changed, vec![3 * UNIT]);
        assert_eq!(c.unit_leaves(3 * UNIT), &[(3 * UNIT, Target::Fb(0x60_0000), UNIT, 6)]);
        // Clearing the PD1 entry drops everything below it.
        m.w(0x12000, 0);
        c.mark_dirty(0x12);
        let u = c.update(&m, false);
        assert_eq!(u.changed, vec![0, 3 * UNIT]);
        assert!(c.all_leaves().is_empty());
        assert_eq!(c.table_pages(), 3); // root, PD2, PD1 stay watched
        // And back: the subtree is walked again.
        m.w(0x12000, vid(0x13000));
        c.mark_dirty(0x12);
        let u = c.update(&m, false);
        assert_eq!(u.changed, vec![0, 3 * UNIT]);
        let mut full = WalkCache::new(Target::Fb(0x10000), 1 << 20);
        full.update(&m, true);
        assert_eq!(c.all_leaves(), full.all_leaves());
    }

    #[test]
    fn written_pages_are_patched_not_reread() {
        let m = tree();
        let mut c = WalkCache::new(Target::Fb(0x10000), 1 << 20);
        c.update(&m, false);
        // The guest writes a PTE: the copy is patched, the update reads nothing.
        let v = pte(0x22000);
        m.w(0x14000 + 8 * 9, v);
        c.note_write(0x14, 0x14000 + 8 * 9, &v.to_le_bytes());
        let u = c.update(&m, false);
        assert_eq!(u.reads, 0);
        assert_eq!(u.changed, vec![0]);
        assert_eq!(c.leaf_count(), 3);
        // A half-qword write patches only its bytes.
        m.w(0x14000 + 8 * 9, 0);
        c.note_write(0x14, 0x14000 + 8 * 9, &[0, 0, 0, 0]);
        c.note_write(0x14, 0x14000 + 8 * 9 + 4, &[0, 0, 0, 0]);
        c.update(&m, false);
        assert_eq!(c.leaf_count(), 2);
        let mut full = WalkCache::new(Target::Fb(0x10000), 1 << 20);
        full.update(&m, true);
        assert_eq!(c.all_leaves(), full.all_leaves());
    }

    #[test]
    fn an_unchanged_rewrite_reports_nothing_and_the_cap_holds() {
        let m = tree();
        let mut c = WalkCache::new(Target::Fb(0x10000), 1 << 20);
        c.update(&m, false);
        c.mark_dirty(0x14);
        assert!(c.update(&m, false).changed.is_empty());
        let mut small = WalkCache::new(Target::Fb(0x10000), 1);
        small.update(&m, false);
        assert!(small.truncated);
        assert_eq!(small.leaf_count(), 0);
    }
}

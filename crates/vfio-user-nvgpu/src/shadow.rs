// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Shadow VA space for the GR kernel launch (variant c1s, exec-formats.md §4.5,
//! decision D17 route 4 stage 1). A GR channel cannot run through the guest's own root
//! from user space: an externally-owned VAS cannot schedule GR, and a replayed VAS makes
//! the host RM build its GR context in the host kernel's own tables, unreachable through
//! the guest root (Xid 38, experiments.md d-gr-c2). The route that stays on the decided
//! design is to run the GR channel on a **normal host-managed VAS** — so the host RM's
//! own context buffers (set B) allocate and resolve as usual — and mirror the guest's
//! own work (set A: GPFIFO, pushbuffers, QMDs, constants, shader/local memory,
//! semaphores) into that VAS at the **same VAs**, through the RM API, by walking the
//! guest root and issuing one fixed-VA `RM_MAP_MEMORY_DMA` per present PTE.
//!
//! Every mapped PA is validated first (D17 stage 1's filter): a vidmem leaf must lie
//! wholly in the reservation and carry an uncompressible kind; a sysmem leaf must lie
//! wholly in a registered guest-RAM descriptor. Anything else is refused, counted and
//! logged with VA, PA, aperture and reason, and never mapped (D16/D17 isolation).
//!
//! **Dynamic mirroring (2026-09-24).** The guest keeps mapping set-A while it works (the
//! launch's I2M targets, new pushbuffer chunks, remapped VAs), so the mirror is a diff:
//! [`ShadowVas::sync`] walks the guest root, compares every leaf with the VA ->
//! (target, size) map of what is mirrored, **unmaps** (NVOS47) every leaf that vanished
//! or changed, and maps what is new. Mapped VAs live in fixed-granularity virtual
//! objects that never grow: one `NV50_MEMORY_VIRTUAL` per 2 MiB-aligned region and page
//! class (a 2 MiB-page region, or a 4 KiB/64 KiB region with the default page-size
//! attribute), created at first use and freed when the region empties or changes class.
//! A region whose object the host refuses (a VA collision with the host RM's own set-B
//! buffers) or that touches an excluded set-B range falls back to one exactly-sized
//! virtual object per leaf. The earlier coalesced-run scheme grew runs past their
//! virtual object and remapped without unmapping (d-gr-launch7: 3 -> 119976
//! `NV_ERR_INVALID_OFFSET`).

use std::collections::BTreeMap;
use std::time::Instant;

use nvrm_abi::sys;

use crate::fb::{FakeFb, Target};
use crate::guestmem::GuestMemory;
use crate::hostmem::HostMemory;

/// The granularity of the region virtual objects: one PD0 entry (2 MiB).
pub const REGION: u64 = 2 << 20;
/// Refusal lines emitted per sync (the counters are always complete).
const LOG_PER_SYNC: usize = 24;

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

struct Mapped {
    leaf: Leaf,
    h_dma: u32,
    h_mem: u32,
    /// A per-leaf virtual object, freed with the mapping.
    own_object: bool,
}

/// The diff of one sync: VAs to unmap (gone or changed) and VAs to map, both ascending.
fn plan(
    mapped: &BTreeMap<u64, Leaf>,
    want: &BTreeMap<u64, Leaf>,
    failed: &BTreeMap<u64, Leaf>,
) -> (Vec<u64>, Vec<u64>) {
    let unmap = mapped
        .iter()
        .filter(|(va, l)| want.get(va) != Some(l))
        .map(|(va, _)| *va)
        .collect();
    let map = want
        .iter()
        .filter(|(va, l)| mapped.get(va) != Some(l) && failed.get(va) != Some(l))
        .map(|(va, _)| *va)
        .collect();
    (unmap, map)
}

/// Counters for the sync lines and the exit summary.
#[derive(Default, Debug, Clone)]
pub struct ShadowStats {
    pub syncs: u64,
    /// Resync requests answered without a walk (no page-table write since the last).
    pub skipped: u64,
    pub maps: u64,
    pub unmaps: u64,
    pub unmap_failed: u64,
    /// Leaves whose map (or per-leaf object) the host refused, counted once per leaf.
    pub map_failed: u64,
    /// Leaves the D16/D17 filter refused, counted once per leaf.
    pub refused: u64,
    pub region_objects: u64,
    pub regions_freed: u64,
    pub regions_refused: u64,
    pub leaf_objects: u64,
    pub max_sync_us: u128,
    pub total_sync_us: u128,
}

/// A host-managed shadow VA space (c1s): the guest's set-A mappings mirrored at their
/// own VAs by fixed `RM_MAP_MEMORY_DMA` into per-region `NV50_MEMORY_VIRTUAL` objects,
/// so a GR channel placed on this VAS resolves the guest's work while the host RM owns
/// the GR context (set B) in the same VAS. The guest's promoted-context VA ranges are
/// excluded (the host builds those).
pub struct ShadowVas {
    pub vas: u32,
    regions: BTreeMap<u64, Region>,
    mapped: BTreeMap<u64, Mapped>,
    /// Leaves the host refused to map, not retried until the guest changes them.
    failed: BTreeMap<u64, Leaf>,
    /// Leaves the filter refused (to count and log each once).
    refused: BTreeMap<u64, Leaf>,
    pub stats: ShadowStats,
    exclude_len: usize,
    always: bool,
}

impl ShadowVas {
    /// A fresh normal (host-managed) `FERMI_VASPACE_A`.
    pub fn new(hm: &mut HostMemory) -> Result<Self, u32> {
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
            failed: BTreeMap::new(),
            refused: BTreeMap::new(),
            stats: ShadowStats::default(),
            exclude_len: usize::MAX,
            always: std::env::var("CARAXES_SHADOW_ALWAYS_RESYNC").as_deref() == Ok("1"),
        })
    }

    /// Leaves mirrored now.
    pub fn mapped_leaves(&self) -> usize {
        self.mapped.len()
    }

    /// The mirrored leaf covering `va`, if any: `(leaf va, leaf)`.
    pub fn lookup(&self, va: u64) -> Option<(u64, Leaf)> {
        self.mapped
            .range(..=va)
            .next_back()
            .filter(|(lva, m)| va < **lva + m.leaf.size)
            .map(|(lva, m)| (*lva, m.leaf))
    }

    fn unmap_one(&mut self, hm: &mut HostMemory, va: u64) {
        let Some(m) = self.mapped.remove(&va) else {
            return;
        };
        let st = hm.unmap_dma(m.h_dma, m.h_mem, va, m.leaf.size);
        self.stats.unmaps += 1;
        if st != sys::NV_OK {
            self.stats.unmap_failed += 1;
        }
        if m.own_object {
            let _ = hm.free_object(m.h_dma);
        }
    }

    /// Re-walk the guest root and bring the shadow VAS in line with it: every present
    /// set-A leaf (outside the `exclude` ranges, the guest's promoted-context buffers)
    /// that passes the filter is mapped at its own VA; a leaf that vanished or now
    /// points elsewhere is unmapped first. Returns `(changed, notes)`: the first note is
    /// the one-line summary, then one line per newly refused leaf (bounded).
    #[allow(clippy::too_many_arguments)]
    pub fn sync(
        &mut self,
        hm: &mut HostMemory,
        fb: &FakeFb,
        sys_mem: &GuestMemory,
        guest_root: u64,
        base: u64,
        end: u64,
        reservation: u32,
        exclude: &[(u64, u64)],
    ) -> (bool, Vec<String>) {
        let t0 = Instant::now();
        // Skip the walk when no write reached a page-table page of the last walk (the
        // FB store sees every guest write to VRAM: BAR1 is trapped, fb.rs `note_write`)
        // and the set-B exclusions did not change. A full walk costs ~30 ms of uncached
        // BAR1 reads, which dominated the per-launch cost (d-launch4-1: 68 ms/launch).
        // `CARAXES_SHADOW_ALWAYS_RESYNC=1` walks every time.
        let dirty = fb.take_tables_dirty(Target::Fb(guest_root));
        if !dirty && !self.always && exclude.len() == self.exclude_len {
            self.stats.skipped += 1;
            return (false, Vec::new());
        }
        self.exclude_len = exclude.len();
        self.stats.syncs += 1;
        let descs: Vec<(u32, u64, u64)> = hm
            .descriptors()
            .iter()
            .map(|d| (d.handle, d.range.iova, d.range.size))
            .collect();
        // Walk the whole guest root (a 16384 cap once cut off the guest's high-VA set-A,
        // the Xid 31 at 0x100_04e00000 of d-gr-launch5).
        const WALK_MAX: usize = 1 << 20;
        let walked = fb.enumerate_leaves_watched(Some(sys_mem), Target::Fb(guest_root), WALK_MAX);
        let truncated = walked.len() == WALK_MAX;
        let t_walk = t0.elapsed().as_micros();
        let mut notes = Vec::new();
        let mut logged = 0usize;
        let mut excluded = 0usize;
        let mut want: BTreeMap<u64, Leaf> = BTreeMap::new();
        let mut backing: BTreeMap<u64, (u32, u64, bool)> = BTreeMap::new();
        let mut refused_now: BTreeMap<u64, Leaf> = BTreeMap::new();
        for (va, t, size, kind) in &walked {
            if exclude.iter().any(|(lo, hi)| va < hi && va + size > *lo) {
                excluded += 1;
                continue;
            }
            let leaf = Leaf {
                target: *t,
                size: *size,
            };
            match resolve_leaf(t, *size, *kind, base, end, &descs) {
                Resolved::Vidmem { offset } => {
                    want.insert(*va, leaf);
                    backing.insert(*va, (reservation, offset, false));
                }
                Resolved::Sysmem { h_mem, offset } => {
                    want.insert(*va, leaf);
                    backing.insert(*va, (h_mem, offset, true));
                }
                Resolved::Refused(why) => {
                    if self.refused.get(va) != Some(&leaf) {
                        self.stats.refused += 1;
                        if logged < LOG_PER_SYNC {
                            logged += 1;
                            notes.push(format!(
                                "shadow: REFUSED leaf va {va:#x} -> {t:x?} size {size:#x} kind {kind:#x}: {why} (never mapped) [D16/D17]"
                            ));
                        }
                    }
                    refused_now.insert(*va, leaf);
                }
            }
        }
        self.refused = refused_now;
        // A failed leaf is retried once the guest changes it.
        self.failed.retain(|va, l| want.get(va) == Some(l));
        let mirrored: BTreeMap<u64, Leaf> =
            self.mapped.iter().map(|(va, m)| (*va, m.leaf)).collect();
        let (unmap, map) = plan(&mirrored, &want, &self.failed);
        let (n_unmap, mut n_map, mut n_fail) = (unmap.len(), 0usize, 0usize);
        for va in &unmap {
            self.unmap_one(hm, *va);
        }
        // Regions: free those no longer needed or of the wrong class (after unmapping
        // what they still hold), then create the missing ones.
        let mut need: BTreeMap<u64, Class> = BTreeMap::new();
        for (va, l) in &want {
            need.insert(va & !(REGION - 1), class_of(l.size));
        }
        let stale: Vec<u64> = self
            .regions
            .iter()
            .filter(|(r, reg)| need.get(r) != Some(&reg.class))
            .map(|(r, _)| *r)
            .collect();
        for r in stale {
            let inside: Vec<u64> = self
                .mapped
                .range(r..r + REGION)
                .map(|(va, _)| *va)
                .collect();
            for va in inside {
                self.unmap_one(hm, va);
            }
            if let Some(reg) = self.regions.remove(&r) {
                if let Some(h) = reg.handle {
                    let _ = hm.free_object(h);
                    self.stats.regions_freed += 1;
                }
            }
        }
        // Map what is missing (the stale-region pass may have unmapped more).
        let todo: Vec<u64> = map
            .into_iter()
            .chain(
                want.iter()
                    .filter(|(va, l)| {
                        !self.mapped.contains_key(va) && self.failed.get(va) != Some(l)
                    })
                    .map(|(va, _)| *va),
            )
            .collect::<std::collections::BTreeSet<u64>>()
            .into_iter()
            .collect();
        for va in todo {
            if self.mapped.contains_key(&va) {
                continue;
            }
            let leaf = want[&va];
            let (h_mem, off, snoop) = backing[&va];
            let r = va & !(REGION - 1);
            let class = class_of(leaf.size);
            if !self.regions.contains_key(&r) {
                let touches_b = exclude.iter().any(|(lo, hi)| r < *hi && r + REGION > *lo);
                let handle = if touches_b {
                    None
                } else {
                    let page = match class {
                        Class::Huge => Some(REGION),
                        Class::Small => None,
                    };
                    match hm.alloc_virtual_fixed(self.vas, r, REGION, page, false) {
                        Ok(h) => {
                            self.stats.region_objects += 1;
                            Some(h)
                        }
                        Err(st) => {
                            self.stats.regions_refused += 1;
                            if logged < LOG_PER_SYNC {
                                logged += 1;
                                notes.push(format!(
                                    "shadow: region {r:#x} ({class:?}) object refused {st:#x}; per-leaf objects there [host]"
                                ));
                            }
                            None
                        }
                    }
                };
                self.regions.insert(r, Region { handle, class });
            }
            let (h_dma, own) = match self.regions[&r].handle {
                Some(h) => (h, false),
                None => {
                    match hm.alloc_virtual_fixed(self.vas, va, leaf.size, Some(leaf.size), true) {
                        Ok(h) => {
                            self.stats.leaf_objects += 1;
                            (h, true)
                        }
                        Err(st) => {
                            self.stats.map_failed += 1;
                            n_fail += 1;
                            self.failed.insert(va, leaf);
                            if logged < LOG_PER_SYNC {
                                logged += 1;
                                notes.push(format!(
                                "shadow: map va {va:#x} -> {:x?} size {:#x} refused: per-leaf VA object {st:#x} (VA collision with set B?) [host]",
                                leaf.target, leaf.size
                            ));
                            }
                            continue;
                        }
                    }
                }
            };
            let st = hm.map_dma_fixed(
                h_dma,
                h_mem,
                off,
                leaf.size,
                va,
                page_size_flag(leaf.size),
                snoop,
            );
            if st == sys::NV_OK {
                self.mapped.insert(
                    va,
                    Mapped {
                        leaf,
                        h_dma,
                        h_mem,
                        own_object: own,
                    },
                );
                self.stats.maps += 1;
                n_map += 1;
            } else {
                if own {
                    let _ = hm.free_object(h_dma);
                }
                self.stats.map_failed += 1;
                n_fail += 1;
                self.failed.insert(va, leaf);
                if logged < LOG_PER_SYNC {
                    logged += 1;
                    notes.push(format!(
                        "shadow: map va {va:#x} -> {:x?} size {:#x} refused {st:#x} [host]",
                        leaf.target, leaf.size
                    ));
                }
            }
        }
        let us = t0.elapsed().as_micros();
        self.stats.max_sync_us = self.stats.max_sync_us.max(us);
        self.stats.total_sync_us += us;
        let changed = n_unmap > 0 || n_map > 0 || n_fail > 0 || logged > 0;
        notes.insert(
            0,
            format!(
                "shadow: sync {} leaves walked{} ({} set-A, {excluded} set-B excluded, {} refused now) -> {n_unmap} unmapped, {n_map} mapped, {n_fail} failed; mirrored {} leaves in {} region object(s) + {} leaf object(s); totals: {} maps, {} unmaps ({} failed), {} map-failed, {} refused, {} region(s) refused; walk {t_walk} us, sync {us} us",
                walked.len(),
                if truncated { " [TRUNCATED at the walk cap]" } else { "" },
                want.len(),
                self.refused.len(),
                self.mapped.len(),
                self.regions.values().filter(|r| r.handle.is_some()).count(),
                self.mapped.values().filter(|m| m.own_object).count(),
                self.stats.maps,
                self.stats.unmaps,
                self.stats.unmap_failed,
                self.stats.map_failed,
                self.stats.refused,
                self.stats.regions_refused,
            ),
        );
        (changed, notes)
    }

    pub fn summary(&self) -> String {
        let s = &self.stats;
        format!(
            "{} syncs ({} skipped clean; max {} us, total {} ms), {} maps, {} unmaps ({} failed), {} map-failed, {} refused by the filter, {} region objects ({} freed, {} refused), {} per-leaf objects; {} leaves mirrored at the end",
            s.syncs,
            s.skipped,
            s.max_sync_us,
            s.total_sync_us / 1000,
            s.maps,
            s.unmaps,
            s.unmap_failed,
            s.map_failed,
            s.refused,
            s.region_objects,
            s.regions_freed,
            s.regions_refused,
            s.leaf_objects,
            self.mapped.len()
        )
    }

    /// Free every virtual object (RM drops their mappings with them), then the VAS.
    pub fn free(self, hm: &mut HostMemory) {
        for (_, m) in self.mapped.iter().filter(|(_, m)| m.own_object) {
            let _ = hm.free_object(m.h_dma);
        }
        for reg in self.regions.values() {
            if let Some(h) = reg.handle {
                let _ = hm.free_object(h);
            }
        }
        let _ = hm.free_object(self.vas);
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
            resolve_leaf(
                &Target::Sys(0x7fff_3fff_f000),
                0x1_0000,
                0,
                BASE,
                END,
                &descs
            ),
            Resolved::Refused(_)
        ));
        // D16: a compressible kind never passes, whatever the address.
        assert!(matches!(
            resolve_leaf(&Target::Fb(BASE), 0x1000, 0x0A, BASE, END, &descs),
            Resolved::Refused(_)
        ));
    }

    #[test]
    fn a_changed_leaf_is_unmapped_before_it_is_mapped_again() {
        let l = |pa: u64, size: u64| Leaf {
            target: Target::Fb(pa),
            size,
        };
        let mut mapped = BTreeMap::new();
        mapped.insert(0x1000u64, l(BASE, 0x1000));
        mapped.insert(0x2000u64, l(BASE + 0x1000, 0x1000));
        mapped.insert(0x3000u64, l(BASE + 0x2000, 0x1000));
        let mut want = BTreeMap::new();
        want.insert(0x1000u64, l(BASE, 0x1000)); // unchanged
        want.insert(0x2000u64, l(BASE + 0x9000, 0x1000)); // remapped
        want.insert(0x4000u64, l(BASE + 0x3000, 0x1000)); // new
        let mut failed = BTreeMap::new();
        let (unmap, map) = plan(&mapped, &want, &failed);
        assert_eq!(unmap, vec![0x2000, 0x3000]);
        assert_eq!(map, vec![0x2000, 0x4000]);
        // A leaf the host refused is not retried until it changes.
        failed.insert(0x4000u64, l(BASE + 0x3000, 0x1000));
        let (_, map) = plan(&mapped, &want, &failed);
        assert_eq!(map, vec![0x2000]);
    }

    #[test]
    fn regions_are_fixed_2mib_and_never_mix_classes() {
        assert_eq!(class_of(0x1000), Class::Small);
        assert_eq!(class_of(0x1_0000), Class::Small);
        assert_eq!(class_of(REGION), Class::Huge);
        assert_eq!(0x100_04e0_1000u64 & !(REGION - 1), 0x100_04e0_0000);
    }
}

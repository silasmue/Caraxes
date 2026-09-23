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
//! Every mapped PA is validated first (D17 stage 1's filter): a vidmem PA must lie in the
//! reservation and carry an uncompressible kind; a sysmem IOVA must lie in a registered
//! guest-RAM descriptor. Anything else is refused and logged, never mapped.
//!
//! This module currently provides the **probe** that establishes whether the guest's
//! set-A pages can be fixed-mapped at all (the reservation is allocated with 2 MiB HUGE
//! pages, so a 4 KiB map of it may be refused by RM — the crux unknown). The channel is
//! wired onto the shadow VAS only once the probe shows the maps land.

use nvrm_abi::sys;

use crate::fb::{FakeFb, Target};
use crate::guestmem::GuestMemory;
use crate::hostmem::HostMemory;

/// `nvos46_flags::PAGE_SIZE_*` for a guest page size.
fn page_size_flag(size: u64) -> u32 {
    use nvrm_abi::nvgpu::nvos46_flags as f;
    match size {
        0x1000 => f::PAGE_SIZE_4KB,
        0x1_0000 => f::PAGE_SIZE_BIG,
        _ => f::PAGE_SIZE_HUGE,
    }
}

/// A leaf the probe would map, resolved against the reservation and the registered
/// sysmem descriptors.
enum Resolved {
    Vidmem { offset: u64 },
    Sysmem { h_mem: u32, offset: u64 },
    RefusedVidmemOutside,
    RefusedSysmemUnregistered,
    RefusedCompressible,
}

fn resolve_leaf(t: &Target, kind: u8, base: u64, end: u64, descs: &[(u32, u64, u64)]) -> Resolved {
    if (0x08..=0x0E).contains(&kind) {
        return Resolved::RefusedCompressible;
    }
    match t {
        Target::Fb(pa) => {
            if *pa >= base && *pa < end {
                Resolved::Vidmem { offset: pa - base }
            } else {
                Resolved::RefusedVidmemOutside
            }
        }
        Target::Sys(iova) => {
            for (h, ib, isz) in descs {
                if *iova >= *ib && *iova < *ib + *isz {
                    return Resolved::Sysmem {
                        h_mem: *h,
                        offset: iova - ib,
                    };
                }
            }
            Resolved::RefusedSysmemUnregistered
        }
        _ => Resolved::RefusedVidmemOutside,
    }
}

/// A host-managed shadow VA space (c1s): the guest's set-A mappings, mirrored at their
/// own VAs by fixed `RM_MAP_MEMORY_DMA` into per-run `NV50_MEMORY_VIRTUAL` objects, so a
/// GR channel placed on this VAS resolves the guest's work while the host RM owns the GR
/// context (set B) in the same VAS. The guest's promoted-context VA ranges are excluded
/// (the host builds those); a leaf whose VA falls in one is skipped.
pub struct ShadowVas {
    pub vas: u32,
    /// Every `NV50_MEMORY_VIRTUAL` object allocated (freed with the VAS at teardown).
    virt_objects: Vec<u32>,
    /// Aligned run bases already given a virtual object, and its handle: `base -> (handle, size, page)`.
    runs: std::collections::BTreeMap<u64, (u32, u64, u64)>,
    /// VAs already mapped and the backing key they point at (to diff on re-sync).
    mapped: std::collections::BTreeMap<u64, (Target, u64)>,
    pub maps_ok: u64,
    pub maps_failed: u64,
    pub runs_failed: u64,
    pub refused: u64,
    pub collisions: Vec<(u64, u64, u64, u32)>,
    failed: std::collections::BTreeSet<u64>,
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
            virt_objects: Vec::new(),
            runs: std::collections::BTreeMap::new(),
            mapped: std::collections::BTreeMap::new(),
            maps_ok: 0,
            maps_failed: 0,
            runs_failed: 0,
            refused: 0,
            collisions: Vec::new(),
            failed: std::collections::BTreeSet::new(),
        })
    }

    /// Re-walk the guest root and mirror every present set-A leaf (excluding the VA ranges
    /// in `exclude`, the guest's promoted-context buffers) into the shadow VAS. New runs
    /// get a FIXED_ADDRESS virtual object; new/changed leaves get a fixed map. Returns a
    /// one-line summary note.
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
    ) -> String {
        let descs: Vec<(u32, u64, u64)> = hm
            .descriptors()
            .iter()
            .map(|d| (d.handle, d.range.iova, d.range.size))
            .collect();
        // Walk the whole guest root: an earlier 16384 cap cut off the guest's high-VA
        // set-A (texture header/sampler pool at 0x100_0000_0000+), so the GR engine
        // MMU-faulted at 0x100_04e00000 (Xid 31). The cap is now large; if `truncated` is
        // ever true the guest exceeded it and the cap must grow again.
        const WALK_MAX: usize = 1 << 20;
        let mut leaves = fb.enumerate_leaves(Some(sys_mem), Target::Fb(guest_root), WALK_MAX);
        let truncated = leaves.len() == WALK_MAX;
        let raw = leaves.len();
        // Drop set-B (promoted context), and count the leaves the filter refuses (they are
        // never mapped — "0 map-failed" alone does not prove completeness).
        leaves.retain(|(va, t, _size, kind)| {
            if exclude.iter().any(|(lo, hi)| va >= lo && va < hi) {
                return false;
            }
            match resolve_leaf(t, *kind, base, end, &descs) {
                Resolved::Vidmem { .. } | Resolved::Sysmem { .. } => true,
                _ => {
                    self.refused += 1;
                    false
                }
            }
        });
        leaves.sort_by_key(|l| l.0);
        // Coalesce contiguous same-page-size leaves into runs.
        let mut runs: Vec<(u64, u64, u64)> = Vec::new(); // (start, len, page)
        for (va, _, size, _) in &leaves {
            if let Some(last) = runs.last_mut() {
                if last.2 == *size && last.0 + last.1 == *va {
                    last.1 += *size;
                    continue;
                }
            }
            runs.push((*va, *size, *size));
        }
        let (mut new_runs, mut new_maps) = (0u64, 0u64);
        for (start, len, page) in runs {
            if !self.runs.contains_key(&start) {
                match hm.alloc_virtual_fixed(self.vas, start, len, page) {
                    Ok(h) => {
                        self.virt_objects.push(h);
                        self.runs.insert(start, (h, len, page));
                        new_runs += 1;
                    }
                    Err(st) => {
                        // Record the VA (a collision with the host RM's own set-B context
                        // buffers, or a bad alignment) once, and mark the run so it is not
                        // retried on every resync.
                        if self.failed.insert(start) {
                            self.runs_failed += 1;
                            if self.collisions.len() < 32 {
                                self.collisions.push((start, len, page, st));
                            }
                        }
                        continue;
                    }
                }
            }
        }
        // Map each leaf into the virtual object covering its run.
        for (va, t, size, kind) in &leaves {
            if self.mapped.get(va).is_some_and(|(pt, _)| pt == t) {
                continue;
            }
            let Some((&_rs, &(h_dma, _, _))) = self.runs.range(..=va).next_back() else {
                continue;
            };
            let (h_mem, off, snoop) = match resolve_leaf(t, *kind, base, end, &descs) {
                Resolved::Vidmem { offset } => (reservation, offset, false),
                Resolved::Sysmem { h_mem, offset } => (h_mem, offset, true),
                _ => continue,
            };
            let st = hm.map_dma_fixed(h_dma, h_mem, off, *size, *va, page_size_flag(*size), snoop);
            if st == sys::NV_OK {
                self.mapped.insert(*va, (*t, *size));
                self.maps_ok += 1;
                new_maps += 1;
            } else {
                self.maps_failed += 1;
            }
        }
        format!(
            "shadow: sync {raw} leaves walked{} ({} set-A after {} excluded/refused) -> {new_runs} new run(s), {new_maps} new map(s); total mapped {}, {} map-failed, {} run/collision-failed",
            if truncated { " [TRUNCATED at the walk cap]" } else { "" },
            leaves.len(),
            self.refused,
            self.maps_ok,
            self.maps_failed,
            self.runs_failed
        )
    }

    pub fn free(self, hm: &mut HostMemory) {
        for h in self.virt_objects.into_iter().rev() {
            let _ = hm.free_object(h);
        }
        let _ = hm.free_object(self.vas);
    }
}

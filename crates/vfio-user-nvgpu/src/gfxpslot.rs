// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Owned-GFXP slots in the VM's slice (docs/research/gr-preemption-2026-09-29.md, step 2).
//!
//! A host GR context in graphics-preemption mode GFXP needs five buffers (the GR context
//! buffer ids 0xb-0xf: preempt, spill, beta CB, page pool, RTV CB). The device keeps them in
//! slots carved from the VM's own reservation (fb.rs `FbLayout::with_gfxp`, between the usable
//! FB and the BAR1 pool, reported reserved, device-private) and maps a slot's buffers through
//! the reservation's handle at fixed VAs of the slot VA window, [`SLOT_VA_BASE`] and up: above
//! every VA a guest allocator gives out (they allocate bottom-up; unified-memory VAs follow CPU
//! VAs below 2^47), and never mirrored (shadow.rs `Layout::slot_window`). No host VRAM outside
//! the slice, and a guest mapping can never land on a slot VA.

/// The GR context buffer ids of the GFXP set (`NV2080_CTRL_GPU_PROMOTE_CTX_BUFFER_ID_*`).
pub const ENGINES: [u32; 5] = [0xb, 0xc, 0xd, 0xe, 0xf];
/// Placement granule of a buffer inside its slot.
pub const GRANULE: u64 = 64 << 10;
/// The slot VA window's start: 256 TiB, the upper half of Turing's 49-bit VA space.
pub const SLOT_VA_BASE: u64 = 1 << 48;
/// The guest VA range a guest allocation stays in: below the window.
pub const GUEST_VA_END: u64 = SLOT_VA_BASE;
/// Slots per 512 MiB of profile, and the bounds.
pub const PROFILE_PER_SLOT: u64 = 512 << 20;
pub const MIN_SLOTS: u32 = 2;
pub const MAX_SLOTS: u32 = 16;
const MIB: u64 = 1 << 20;

/// Where each buffer lies in a slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    /// Offset and length of each buffer in [`ENGINES`] order; a length 0 is a buffer the host
    /// does not need.
    pub offsets: [u64; 5],
    pub lens: [u64; 5],
    /// The slot stride: a multiple of the largest alignment (at least [`GRANULE`]).
    pub slot: u64,
}

impl Geometry {
    /// From the host's `(size, alignment)` of each engine (`GET_ENGINE_CONTEXT_PROPERTIES`).
    pub fn from_properties(props: &[(u64, u64); 5]) -> Result<Self, String> {
        let mut offsets = [0u64; 5];
        let mut lens = [0u64; 5];
        let mut at = 0u64;
        let mut align_max = GRANULE;
        for (i, &(size, alignment)) in props.iter().enumerate() {
            if size == 0 {
                continue;
            }
            if alignment != 0 && !alignment.is_power_of_two() {
                return Err(format!(
                    "GFXP buffer {:#x}: alignment {alignment:#x} is no power of two",
                    ENGINES[i]
                ));
            }
            let a = alignment.max(GRANULE);
            align_max = align_max.max(a);
            at = at.checked_next_multiple_of(a).ok_or("GFXP slot overflow")?;
            let len = size
                .checked_next_multiple_of(GRANULE)
                .ok_or("GFXP slot overflow")?;
            offsets[i] = at;
            lens[i] = len;
            at = at.checked_add(len).ok_or("GFXP slot overflow")?;
        }
        if at == 0 {
            return Err("the host needs no GFXP buffer".into());
        }
        if at > 64 * MIB {
            return Err(format!("GFXP slot of {at:#x} bytes above 64 MiB"));
        }
        let slot = at.next_multiple_of(align_max);
        Ok(Geometry {
            offsets,
            lens,
            slot,
        })
    }
}

/// `NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES`: `{engineId, alignment, size}`.
const CMD_GET_ENGINE_CONTEXT_PROPERTIES: u32 = 0x0080_1707;

/// The slot geometry from the host's own sizes and alignments (read-only controls on the
/// device's host device object).
pub fn host_geometry(h: &crate::hostmem::HostMemory) -> Result<Geometry, String> {
    let mut props = [(0u64, 0u64); 5];
    for (i, engine) in ENGINES.iter().enumerate() {
        let mut p = [0u8; 12];
        p[..4].copy_from_slice(&engine.to_le_bytes());
        let st = h.control(h.device(), CMD_GET_ENGINE_CONTEXT_PROPERTIES, &mut p);
        if st != 0 {
            return Err(format!(
                "GET_ENGINE_CONTEXT_PROPERTIES {engine:#x}: status {st:#x}"
            ));
        }
        let alignment = u32::from_le_bytes(p[4..8].try_into().unwrap()) as u64;
        let size = u32::from_le_bytes(p[8..12].try_into().unwrap()) as u64;
        props[i] = (size, alignment);
    }
    Geometry::from_properties(&props)
}

/// The slot count for a profile of `profile` bytes: one per 512 MiB, 2 to 16.
pub fn count_for(profile: u64) -> u32 {
    ((profile / PROFILE_PER_SLOT) as u32).clamp(MIN_SLOTS, MAX_SLOTS)
}

/// The slice the slots take: `count` slots, rounded up to 2 MiB (the usable FB stays 2 MiB
/// aligned).
pub fn carve_bytes(g: &Geometry, count: u32) -> u64 {
    (g.slot * u64::from(count)).next_multiple_of(2 * MIB)
}

/// The slots of one device: which are bound to a host TSG.
#[derive(Debug)]
pub struct Slots {
    /// Guest FB address of slot 0 (the start of `FbLayout::gfxp_range`).
    base: u64,
    pub geometry: Geometry,
    used: Vec<bool>,
}

impl Slots {
    /// As many slots as fit `range` (the layout's GFXP range).
    pub fn new(range: (u64, u64), geometry: Geometry) -> Self {
        let n = ((range.1 - range.0) / geometry.slot) as usize;
        Slots {
            base: range.0,
            geometry,
            used: vec![false; n],
        }
    }

    pub fn count(&self) -> usize {
        self.used.len()
    }

    pub fn in_use(&self) -> usize {
        self.used.iter().filter(|u| **u).count()
    }

    /// The lowest free slot, taken.
    pub fn alloc(&mut self) -> Option<usize> {
        let i = self.used.iter().position(|u| !*u)?;
        self.used[i] = true;
        Some(i)
    }

    pub fn free(&mut self, i: usize) {
        if let Some(u) = self.used.get_mut(i) {
            *u = false;
        }
    }

    /// The guest FB range of slot `i`.
    pub fn range(&self, i: usize) -> (u64, u64) {
        let at = self.base + i as u64 * self.geometry.slot;
        (at, at + self.geometry.slot)
    }

    /// The private VA window of every slot: `[SLOT_VA_BASE, + count * slot)`.
    pub fn va_window(&self) -> (u64, u64) {
        (
            SLOT_VA_BASE,
            SLOT_VA_BASE + self.count() as u64 * self.geometry.slot,
        )
    }

    /// The buffers of slot `i` in [`ENGINES`] order: `(guest FB address, VA, length)`, length
    /// 0 for a buffer the host does not need. Slot `i` has its own VAs, so two slots never
    /// share one, in one VA space or across them.
    pub fn buffers(&self, i: usize) -> [(u64, u64, u64); 5] {
        let (pa, _) = self.range(i);
        let va = SLOT_VA_BASE + i as u64 * self.geometry.slot;
        let g = &self.geometry;
        std::array::from_fn(|k| (pa + g.offsets[k], va + g.offsets[k], g.lens[k]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The RTX 2070's sizes (astra-review-2026-09-26.md:97), 4 KiB aligned.
    const RTX2070: [(u64, u64); 5] = [
        (0x10f300, 0x1000),
        (0xe1000, 0x1000),
        (0x20000, 0x1000),
        (0x3e3a80, 0x1000),
        (0x82000, 0x1000),
    ];

    #[test]
    fn a_slot_holds_the_five_buffers_at_64_kib() {
        let g = Geometry::from_properties(&RTX2070).unwrap();
        assert_eq!(g.lens, [0x110000, 0xf0000, 0x20000, 0x3f0000, 0x90000]);
        assert_eq!(g.offsets, [0, 0x110000, 0x200000, 0x220000, 0x610000]);
        assert_eq!(g.slot, 0x6a0000);
        // A larger alignment moves the buffer and rounds the stride.
        let mut p = RTX2070;
        p[3].1 = 0x20_0000;
        let g2 = Geometry::from_properties(&p).unwrap();
        assert_eq!(g2.offsets[3] % 0x20_0000, 0);
        assert_eq!(g2.slot % 0x20_0000, 0);
        assert!(Geometry::from_properties(&[(0, 0); 5]).is_err());
        p[0].1 = 0x3000;
        assert!(Geometry::from_properties(&p).is_err());
    }

    /// The slice cost per profile (the user sees this table): one slot per 512 MiB, 2 to 16.
    #[test]
    fn the_slice_cost_per_profile() {
        let g = Geometry::from_properties(&RTX2070).unwrap();
        let cost = |gib: u64| {
            let n = count_for(gib << 30);
            (n, carve_bytes(&g, n) >> 20)
        };
        assert_eq!(cost(1), (2, 14));
        assert_eq!(cost(2), (4, 28));
        assert_eq!(cost(3), (6, 40));
        assert_eq!(cost(4), (8, 54));
        assert_eq!(cost(8), (16, 106));
        assert_eq!(cost(16), (16, 106));
        assert_eq!(count_for(256 << 20), 2);
    }

    #[test]
    fn slots_are_allocated_lowest_first_reused_and_bounded() {
        let g = Geometry::from_properties(&RTX2070).unwrap();
        let base = 0xf000_0000;
        let mut s = Slots::new((base, base + carve_bytes(&g, 3)), g);
        assert_eq!(s.count(), 3);
        assert_eq!(
            (s.alloc(), s.alloc(), s.alloc(), s.alloc()),
            (Some(0), Some(1), Some(2), None)
        );
        s.free(1);
        assert_eq!(s.in_use(), 2);
        assert_eq!(s.alloc(), Some(1));
        assert_eq!(s.range(2), (base + 2 * 0x6a0000, base + 3 * 0x6a0000));
        let b = s.buffers(2);
        assert_eq!(
            b[0],
            (base + 2 * 0x6a0000, SLOT_VA_BASE + 2 * 0x6a0000, 0x110000)
        );
        assert_eq!(b[4].0 + b[4].2, s.range(2).0 + 0x6a0000);
    }

    /// No guest mapping in the guest's VA range can land on a slot VA: the whole window lies
    /// at or above [`GUEST_VA_END`], for every profile, and a shadow layout with the window
    /// refuses a guest leaf inside it by name.
    #[test]
    fn a_guest_mapping_never_lands_on_a_slot_va() {
        let g = Geometry::from_properties(&RTX2070).unwrap();
        for gib in [1, 2, 4, 8, 16] {
            let n = count_for(gib << 30);
            let s = Slots::new((0, carve_bytes(&g, n)), g);
            let (lo, hi) = s.va_window();
            assert!(
                lo >= GUEST_VA_END && hi <= 1 << 49,
                "{gib} GiB: {lo:#x}..{hi:#x}"
            );
            for i in 0..s.count() {
                for (_, va, len) in s.buffers(i) {
                    assert!(va >= lo && va + len <= hi);
                }
            }
            let l = crate::shadow::Layout::default().with_slot_window(Some((lo, hi)));
            assert!(l
                .unmirrorable(lo, lo + 0x1000)
                .unwrap()
                .contains("slot VA window"));
            assert!(l.unmirrorable(hi - 0x1000, hi).is_some());
            assert_eq!(l.unmirrorable(GUEST_VA_END - 0x1000, GUEST_VA_END), None);
            assert_eq!(l.unmirrorable(hi, hi + 0x1000), None);
        }
    }
}

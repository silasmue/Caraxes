// SPDX-License-Identifier: MIT
//! The GMMU version-3 page-table format (Hopper and Blackwell, 57-bit VA, five
//! directory levels), for the BAR1/BAR2 aperture walk and the leaf enumeration of
//! [`crate::fb::FakeFb`] when the chip's format is `gmmu-v3-57bit` (chips/gb206/chip.toml).
//! The v2 walker in fb.rs is not touched; fb.rs hands a v3 walk here.
//!
//! Level layout for GB20x (`kgmmuFmtInitLevels_GB10X`, the GB206 HAL,
//! generated/g_kern_gmmu_nvoc.c:705-725; src/nvidia/src/kernel/gpu/mmu/arch/blackwell/
//! kern_gmmu_fmt_gb10x.c:30-60 on top of hopper/kern_gmmu_fmt_gh10x.c:30-122):
//!
//! | Level | VA bits | Entry | May be a PTE |
//! |---|---|---|---|
//! | PD4 | 56 | 8-byte PDE | no |
//! | PD3 | 55:47 | 8-byte PDE | no |
//! | PD2 | 46:38 | 8-byte PDE | 256 GiB page (GB10X) |
//! | PD1 | 37:29 | 8-byte PDE | 512 MiB page |
//! | PD0 | 28:21 | 16-byte dual PDE | 2 MiB page |
//! | PT big | 20:16 | 8-byte PTE | 64 KiB |
//! | PT small | 20:12 | 8-byte PTE | 4 KiB |
//!
//! Field layouts (src/common/inc/swref/published/hopper/gh100/dev_mmu.h, which the GB20x
//! format code includes, kern_gmmu_fmt_gh10x.c:28; the same lines in the 615.71.09 tree,
//! /mnt/vmstore/caraxes/ogkm-615.71.09, checked 2026-09-24; blackwell/gb202/dev_mmu.h
//! carries only kinds):
//!
//! - PDE: IS_PTE 0:0, APERTURE 2:1 (0 invalid, 1 video, 2/3 system), PCF 5:3, ADDRESS
//!   51:12 shifted by 12 (dev_mmu.h:52-79).
//! - Dual PDE: IS_PTE 0:0; big: APERTURE_BIG 2:1, ADDRESS_BIG 51:8 shifted by 8; small:
//!   APERTURE_SMALL 66:65, ADDRESS_SMALL 115:76 shifted by 12 (dev_mmu.h:81-128).
//! - PTE: VALID 0:0, APERTURE 2:1 (0 video, 1 peer, 2/3 system), PCF 7:3 (1 = SPARSE when
//!   invalid), KIND 11:8, ADDRESS 51:12 shifted by 12 (dev_mmu.h:130-188).
//!
//! The results are the v2 walker's [`Target`]s: a page-table level that is missing gives
//! `Unmapped { level }` with level 4 (PD4) down to 0 (the page tables).

use crate::fb::Target;

/// Address bits 51:12 of a PDE or PTE.
const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;
/// ADDRESS_BIG 51:8 of the dual PDE's low qword.
const ADDR_BIG_MASK: u64 = 0x000F_FFFF_FFFF_FF00;

/// Reads `n` consecutive qwords of a page-table page (the FB store or guest RAM).
pub trait Tables {
    fn block(&self, table: Target, n: usize) -> Option<Vec<u64>>;
    fn qword(&self, table: Target, index: u64) -> Option<u64> {
        match table {
            Target::Fb(pa) => self.block(Target::Fb(pa + 8 * index), 1).map(|v| v[0]),
            Target::Sys(pa) => self.block(Target::Sys(pa + 8 * index), 1).map(|v| v[0]),
            _ => None,
        }
    }
}

fn aperture_target(aperture: u64, addr: u64) -> Option<Target> {
    match aperture {
        1 => Some(Target::Fb(addr)),
        2 | 3 => Some(Target::Sys(addr)),
        _ => None,
    }
}

/// The table a (non-dual) PDE points to.
pub fn next_table(pde: u64) -> Option<Target> {
    if pde & 1 == 1 {
        return None; // a PTE, not a directory
    }
    aperture_target((pde >> 1) & 3, pde & ADDR_MASK)
}

/// The big-page table of a dual PDE's low qword.
pub fn big_table(lo: u64) -> Option<Target> {
    if lo & 1 == 1 {
        return None;
    }
    aperture_target((lo >> 1) & 3, lo & ADDR_BIG_MASK)
}

/// The small-page table of a dual PDE's high qword (bits 66:65 and 115:76 of the entry).
pub fn small_table(hi: u64) -> Option<Target> {
    aperture_target((hi >> 1) & 3, hi & ADDR_MASK)
}

/// A PTE (or a PDE with IS_PTE set) resolved at `offset` inside its page.
pub fn decode_pte(pte: u64, offset: u64) -> Target {
    if pte & 1 == 0 {
        return if (pte >> 3) & 0x1F == 1 {
            Target::Sparse
        } else {
            Target::Unmapped { level: 0 }
        };
    }
    match (pte >> 1) & 3 {
        0 => Target::Fb((pte & ADDR_MASK) + offset),
        2 | 3 => Target::Sys((pte & ADDR_MASK) + offset),
        _ => Target::Unmapped { level: 0 }, // peer memory: nothing behind it here
    }
}

/// KIND 11:8 of a PTE.
pub fn kind(pte: u64) -> u8 {
    ((pte >> 8) & 0xF) as u8
}

/// Directory levels above PD0: (level number, VA shift, index mask, page size when the
/// entry is a PTE).
const DIRS: [(u8, u32, u64, Option<u64>); 4] = [
    (4, 56, 0x1, None),
    (3, 47, 0x1FF, None),
    (2, 38, 0x1FF, Some(1 << 38)),
    (1, 29, 0x1FF, Some(1 << 29)),
];

/// Translate `va` from `root` (the PD4). Returns the target and the raw last entry, and
/// pushes every FB table page read into `pages` (for the BAR TLB's invalidation).
pub fn walk(t: &impl Tables, root: Target, va: u64, pages: &mut Vec<u64>) -> (Target, u64) {
    let mut table = root;
    for (level, shift, mask, page) in DIRS {
        if let Target::Fb(pa) = table {
            pages.push(pa >> 12);
        }
        let Some(e) = t.qword(table, (va >> shift) & mask) else {
            return (Target::Unmapped { level }, 0);
        };
        if e & 1 == 1 {
            if let Some(size) = page {
                return (decode_pte(e, va & (size - 1)), e);
            }
            return (Target::Unmapped { level }, e);
        }
        let Some(next) = next_table(e) else {
            return (Target::Unmapped { level }, e);
        };
        table = next;
    }
    if let Target::Fb(pa) = table {
        pages.push(pa >> 12);
    }
    let idx0 = (va >> 21) & 0xFF;
    let (Some(lo), Some(hi)) = (t.qword(table, 2 * idx0), t.qword(table, 2 * idx0 + 1)) else {
        return (Target::Unmapped { level: 0 }, 0);
    };
    if lo & 1 == 1 {
        return (decode_pte(lo, va & 0x1F_FFFF), lo);
    }
    let small = small_table(hi);
    let big = big_table(lo);
    for tt in [big, small].into_iter().flatten() {
        if let Target::Fb(pa) = tt {
            pages.push(pa >> 12);
        }
    }
    // The small PTE wins when it maps; an unmapped small PTE falls back to the big table
    // (the v2 walker's rule, fb.rs `walk_in`).
    if let Some(pt) = small {
        if let Some(pte) = t.qword(pt, (va >> 12) & 0x1FF) {
            let r = decode_pte(pte, va & 0xFFF);
            if !matches!(r, Target::Unmapped { .. }) || big.is_none() {
                return (r, pte);
            }
        }
    }
    if let Some(pt) = big {
        if let Some(pte) = t.qword(pt, (va >> 16) & 0x1F) {
            return (decode_pte(pte, va & 0xFFFF), pte);
        }
    }
    (Target::Unmapped { level: 0 }, 0)
}

/// Every present leaf under `root`: `(va, target, size, kind)` as the v2
/// `enumerate_leaves` gives them (fb.rs), capped at `max`. A valid big PTE hides the
/// sixteen small slots it covers.
pub fn enumerate_leaves(t: &impl Tables, root: Target, max: usize) -> Vec<(u64, Target, u64, u8)> {
    let mut out = Vec::new();
    dir(t, root, 0, 0, max, &mut out);
    out
}

fn leaf(pte: u64, va: u64, size: u64) -> Option<(u64, Target, u64, u8)> {
    if pte & 1 == 0 {
        return None;
    }
    match decode_pte(pte, 0) {
        tg @ (Target::Fb(_) | Target::Sys(_)) => Some((va, tg, size, kind(pte))),
        _ => None,
    }
}

fn dir(
    t: &impl Tables,
    table: Target,
    depth: usize,
    base: u64,
    max: usize,
    out: &mut Vec<(u64, Target, u64, u8)>,
) {
    if depth == DIRS.len() {
        return pd0(t, table, base, max, out);
    }
    let (_, shift, mask, page) = DIRS[depth];
    let Some(entries) = t.block(table, (mask + 1) as usize) else {
        return;
    };
    for (i, e) in entries.iter().enumerate() {
        if out.len() >= max {
            return;
        }
        let va = base | ((i as u64) << shift);
        if e & 1 == 1 {
            if let Some(size) = page {
                if let Some(l) = leaf(*e, va, size) {
                    out.push(l);
                }
            }
            continue;
        }
        if let Some(next) = next_table(*e) {
            dir(t, next, depth + 1, va, max, out);
        }
    }
}

fn pd0(
    t: &impl Tables,
    table: Target,
    base: u64,
    max: usize,
    out: &mut Vec<(u64, Target, u64, u8)>,
) {
    let Some(pd0) = t.block(table, 512) else {
        return;
    };
    for i0 in 0..256usize {
        if out.len() >= max {
            return;
        }
        let va = base | ((i0 as u64) << 21);
        let (lo, hi) = (pd0[2 * i0], pd0[2 * i0 + 1]);
        if lo & 1 == 1 {
            if let Some(l) = leaf(lo, va, 0x20_0000) {
                out.push(l);
            }
            continue;
        }
        let big = big_table(lo).and_then(|b| t.block(b, 32));
        if let Some(bt) = &big {
            for (b, pte) in bt.iter().enumerate() {
                if out.len() >= max {
                    return;
                }
                if let Some(l) = leaf(*pte, va | ((b as u64) << 16), 0x1_0000) {
                    out.push(l);
                }
            }
        }
        let Some(pt) = small_table(hi).and_then(|s| t.block(s, 512)) else {
            continue;
        };
        for (s, pte) in pt.iter().enumerate() {
            if out.len() >= max {
                return;
            }
            if big.as_ref().is_some_and(|bt| bt[s >> 4] & 1 == 1) {
                continue;
            }
            if let Some(l) = leaf(*pte, va | ((s as u64) << 12), 0x1000) {
                out.push(l);
            }
        }
    }
}

#[cfg(test)]
mod tests;

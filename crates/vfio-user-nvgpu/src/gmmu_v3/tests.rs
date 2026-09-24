// SPDX-License-Identifier: MIT
//! The v3 walker against page tables written with the field layouts of
//! hopper/gh100/dev_mmu.h (the GB20x format, see the module header), and against the v2
//! walker of fb.rs on the same logical mappings: both must give the same `Target`s.

use super::*;
use crate::fb::FakeFb;
use crate::guestmem::GuestMemory;

const MIB: u64 = 1 << 20;

// v3 encoders (dev_mmu.h:52-188).
fn pde3(pa: u64) -> u64 {
    (pa & ADDR_MASK) | (1 << 1) // APERTURE 2:1 = VIDEO_MEMORY
}
fn dual_lo3(big_pa: u64) -> u64 {
    (big_pa & ADDR_BIG_MASK) | (1 << 1) // APERTURE_BIG = VIDEO_MEMORY
}
fn dual_hi3(small_pa: u64) -> u64 {
    (small_pa & ADDR_MASK) | (1 << 1) // bits 66:65 and 115:76 of the dual PDE
}
fn pte3_fb(pa: u64, kind: u64) -> u64 {
    1 | (kind << 8) | (pa & ADDR_MASK) // VALID, APERTURE 0 = VIDEO_MEMORY
}
fn pte3_sys(pa: u64) -> u64 {
    1 | (2 << 1) | (pa & ADDR_MASK) // SYSTEM_COHERENT_MEMORY
}
const SPARSE3: u64 = 1 << 3; // VALID 0, PCF 7:3 = SPARSE (dev_mmu.h:141)

// v2 encoders, the layouts fb.rs decodes (pascal/gp100/dev_mmu.h).
fn pde2(pa: u64) -> u64 {
    ((pa >> 12) << 8) | (1 << 1)
}
fn dual_lo2(big_pa: u64) -> u64 {
    ((big_pa >> 8) << 4) | (1 << 1)
}
fn pte2_fb(pa: u64) -> u64 {
    1 | ((pa >> 12) << 8)
}
fn pte2_sys(pa: u64) -> u64 {
    1 | (2 << 1) | ((pa >> 12) << 8)
}

/// The mappings both trees carry.
const VA_SMALL: u64 = 0x10_0000_3000; // 4 KiB page
const VA_BIG: u64 = 0x10_0001_0000; // 64 KiB page (same PD0 entry, big slot 1)
const VA_2M: u64 = 0x10_0020_0000; // 2 MiB page (PD0 index 1)
const VA_SYS: u64 = 0x10_0000_5000; // 4 KiB page in guest memory
const VA_SPARSE: u64 = 0x10_0000_6000;
const PA_SMALL: u64 = 0x80_3000;
const PA_BIG: u64 = 0x90_0000;
const PA_2M: u64 = 0xA0_0000;
const PA_SYS: u64 = 0x1234_5000;

fn put(fb: &mut FakeFb, pa: u64, v: u64) {
    assert!(fb.write_u64(pa, v));
}

/// A v3 tree rooted at 0x1000 (PD4 0x1000, PD3 0x2000, PD2 0x3000, PD1 0x4000, PD0 0x5000,
/// small PT 0x6000, big PT 0x7000), plus a 512 MiB page at PD1 index 3.
fn tree_v3() -> FakeFb {
    let mut fb = FakeFb::new(64 * MIB).with_mmu_format("gmmu-v3-57bit");
    let va = VA_SMALL;
    put(&mut fb, 0x1000 + 8 * ((va >> 56) & 1), pde3(0x2000));
    put(&mut fb, 0x2000 + 8 * ((va >> 47) & 0x1FF), pde3(0x3000));
    put(&mut fb, 0x3000 + 8 * ((va >> 38) & 0x1FF), pde3(0x4000));
    put(&mut fb, 0x4000 + 8 * ((va >> 29) & 0x1FF), pde3(0x5000));
    put(&mut fb, 0x4000 + 8 * 3, pte3_fb(0x2000_0000, 6)); // 512 MiB page, VA 3 << 29
    let i0 = (va >> 21) & 0xFF;
    put(&mut fb, 0x5000 + 16 * i0, dual_lo3(0x7000));
    put(&mut fb, 0x5000 + 16 * i0 + 8, dual_hi3(0x6000));
    put(
        &mut fb,
        0x5000 + 16 * ((VA_2M >> 21) & 0xFF),
        pte3_fb(PA_2M, 6),
    );
    put(
        &mut fb,
        0x6000 + 8 * ((VA_SMALL >> 12) & 0x1FF),
        pte3_fb(PA_SMALL, 6),
    );
    put(
        &mut fb,
        0x6000 + 8 * ((VA_SYS >> 12) & 0x1FF),
        pte3_sys(PA_SYS),
    );
    put(&mut fb, 0x6000 + 8 * ((VA_SPARSE >> 12) & 0x1FF), SPARSE3);
    put(
        &mut fb,
        0x7000 + 8 * ((VA_BIG >> 16) & 0x1F),
        pte3_fb(PA_BIG, 6),
    );
    fb
}

/// The same mappings (without the 512 MiB page, which v2 has not) in the v2 format.
fn tree_v2() -> FakeFb {
    let mut fb = FakeFb::new(64 * MIB);
    let va = VA_SMALL;
    put(&mut fb, 0x1000 + 8 * ((va >> 47) & 3), pde2(0x3000));
    put(&mut fb, 0x3000 + 8 * ((va >> 38) & 0x1FF), pde2(0x4000));
    put(&mut fb, 0x4000 + 8 * ((va >> 29) & 0x1FF), pde2(0x5000));
    let i0 = (va >> 21) & 0xFF;
    put(&mut fb, 0x5000 + 16 * i0, dual_lo2(0x7000));
    put(&mut fb, 0x5000 + 16 * i0 + 8, pde2(0x6000));
    put(
        &mut fb,
        0x5000 + 16 * ((VA_2M >> 21) & 0xFF),
        pte2_fb(PA_2M),
    );
    put(
        &mut fb,
        0x6000 + 8 * ((VA_SMALL >> 12) & 0x1FF),
        pte2_fb(PA_SMALL),
    );
    put(
        &mut fb,
        0x6000 + 8 * ((VA_SYS >> 12) & 0x1FF),
        pte2_sys(PA_SYS),
    );
    put(&mut fb, 0x6000 + 8 * ((VA_SPARSE >> 12) & 0x1FF), 1 << 3);
    put(
        &mut fb,
        0x7000 + 8 * ((VA_BIG >> 16) & 0x1F),
        pte2_fb(PA_BIG),
    );
    fb
}

#[test]
fn field_decoders_follow_dev_mmu_h() {
    // PDE ADDRESS 51:12, APERTURE 2:1 (dev_mmu.h:59-77).
    assert_eq!(
        next_table(pde3(0xF_FFFF_FFFF_F000)),
        Some(Target::Fb(0xF_FFFF_FFFF_F000))
    );
    assert_eq!(next_table((0x5000) | (2 << 1)), Some(Target::Sys(0x5000)));
    assert_eq!(next_table(0x5000), None, "APERTURE_INVALID");
    assert_eq!(next_table(pde3(0x5000) | 1), None, "IS_PTE");
    // Dual PDE ADDRESS_BIG 51:8 shifted by 8 (dev_mmu.h:106, 127).
    assert_eq!(big_table(dual_lo3(0x7700)), Some(Target::Fb(0x7700)));
    // PTE KIND 11:8, ADDRESS 51:12, sparse via PCF 7:3 (dev_mmu.h:139-177).
    assert_eq!(kind(pte3_fb(0x1000, 6)), 6);
    assert_eq!(decode_pte(pte3_fb(0x8000, 6), 0x10), Target::Fb(0x8010));
    assert_eq!(decode_pte(SPARSE3, 0), Target::Sparse);
    assert_eq!(decode_pte(0, 0), Target::Unmapped { level: 0 });
    assert_eq!(
        decode_pte(1 | (1 << 1) | 0x8000, 0),
        Target::Unmapped { level: 0 },
        "peer"
    );
}

#[test]
fn v3_and_v2_trees_give_the_same_targets() {
    let (v3, v2) = (tree_v3(), tree_v2());
    let mem = GuestMemory::new();
    for (va, want) in [
        (VA_SMALL + 0x123, Target::Fb(PA_SMALL + 0x123)),
        (VA_BIG + 0x4567, Target::Fb(PA_BIG + 0x4567)),
        (VA_2M + 0x1_2345, Target::Fb(PA_2M + 0x1_2345)),
        (VA_SYS + 8, Target::Sys(PA_SYS + 8)),
        (VA_SPARSE, Target::Sparse),
    ] {
        assert_eq!(v3.translate(0x1000, va), want, "v3 {va:#x}");
        assert_eq!(v2.translate(0x1000, va), want, "v2 {va:#x}");
        assert_eq!(v3.bar_translate(0x1000, va), want, "v3 tlb {va:#x}");
        assert_eq!(v3.bar_translate(0x1000, va), want, "v3 tlb hit {va:#x}");
        let _ = &mem;
    }
    // A 4 KiB slot without a small PTE falls back to the big table, as in v2.
    let va = VA_BIG + 0x1000;
    assert_eq!(v3.translate(0x1000, va), v2.translate(0x1000, va));
    // Unmapped levels: an empty PD3 slot (another bit-47 index).
    assert_eq!(v3.translate(0x1000, 1 << 47), Target::Unmapped { level: 3 });
    // The 512 MiB page at PD1 (v3 only).
    assert_eq!(
        v3.translate(0x1000, (3 << 29) + 0x77),
        Target::Fb(0x2000_0077)
    );
}

#[test]
fn leaves_match_the_v2_enumeration() {
    let (v3, v2) = (tree_v3(), tree_v2());
    let mut l3 = v3.enumerate_leaves(None, Target::Fb(0x1000), 100);
    let mut l2 = v2.enumerate_leaves(None, Target::Fb(0x1000), 100);
    // v3's extra 512 MiB page aside, the leaves are the same; v2 takes KIND from 63:56,
    // which these v2 PTEs leave 0, so compare without it.
    let huge = (3u64 << 29, Target::Fb(0x2000_0000), 1u64 << 29, 6u8);
    assert!(l3.contains(&huge));
    l3.retain(|l| *l != huge);
    let strip = |v: &mut Vec<(u64, Target, u64, u8)>| {
        v.iter_mut().for_each(|l| l.3 = 0);
        v.sort_by_key(|l| l.0);
    };
    assert!(l3
        .iter()
        .all(|l| l.3 == if matches!(l.1, Target::Fb(_)) { 6 } else { 0 }));
    strip(&mut l3);
    strip(&mut l2);
    assert_eq!(l3, l2);
    assert_eq!(l3.len(), 4); // small, sys, big, 2 MiB (sparse is not a leaf)
    assert_eq!(
        v3.enumerate_leaves(None, Target::Fb(0x1000), 2).len(),
        2,
        "cap"
    );
}

#[test]
fn bar_tlb_flushes_on_a_v3_table_write() {
    let mut fb = tree_v3();
    assert_eq!(fb.bar_translate(0x1000, VA_SMALL), Target::Fb(PA_SMALL));
    // Remap the small page: the write lands on a table page the walk read.
    put(
        &mut fb,
        0x6000 + 8 * ((VA_SMALL >> 12) & 0x1FF),
        pte3_fb(0xC0_0000, 6),
    );
    assert_eq!(fb.bar_translate(0x1000, VA_SMALL), Target::Fb(0xC0_0000));
    assert_eq!(fb.bar_tlb_stats.get().2, 1, "one flush");
}

#[test]
fn the_default_format_is_v2() {
    // The same v3 bytes read by the v2 walker do not resolve: the switch is what selects.
    let mut fb = FakeFb::new(64 * MIB);
    let src = tree_v3();
    for pa in (0x1000..0x8000).step_by(8) {
        put(&mut fb, pa, src.read_u64(pa).unwrap());
    }
    assert_ne!(fb.translate(0x1000, VA_SMALL), Target::Fb(PA_SMALL));
    let fb = FakeFb::new(MIB).with_mmu_format("gmmu-v2-49bit");
    assert_eq!(fb.translate(0x1000, 0), Target::Unmapped { level: 3 });
}

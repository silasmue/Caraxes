// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The costume board `rtx5060ti-on-tu106` (docs/caraxes/costume-5060.md), as the tests
//! pin it: the RTX 5060 Ti's PCI identity and name, every answer a capability is derived
//! from the TU106's. The costume is board data (`boards/rtx5060ti-on-tu106.toml`, with the
//! generic `ids_from` overlay of config.rs); no code path knows it, so these tests compare
//! it with the two boards it is made of.

use crate::bar0::{Bar0, NV_PMC_BOOT_0, NV_PMC_BOOT_1, NV_PMC_BOOT_42};
use crate::board::Board;
use crate::fb::FbLayout;
use crate::fixture;
use crate::guestmem::GuestMemory;
use crate::rpc::build_static_info;
use crate::ver::active as v;

const MIB: u64 = 1 << 20;

/// The PCI ids are the 5060 Ti's (10de:2d04 rev a1, class 030000, subsystem 1043:8a08,
/// the GB206 board's bytes); every other config byte is the RTX 2070's, so the capability
/// chain and the link registers sit where the Turing kernel RM reads them (XVE 0x84/0x88).
#[test]
fn pci_ids_are_the_5060_ti_and_the_layout_the_tu106() {
    let c = fixture::rtx5060ti_on_tu106().config();
    let g = fixture::rtx5060ti().config();
    let t = fixture::rtx2070().config();
    assert_eq!(
        (c.vendor_id(), c.device_id(), c.revision()),
        (0x10DE, 0x2D04, 0xA1)
    );
    assert_eq!(c.class_code(), 0x03_0000);
    assert_eq!(c.subsystem(), (0x1043, 0x8A08));
    for o in 0..crate::config::SIZE {
        if crate::config::ID_BYTES.iter().any(|r| r.contains(&o)) {
            assert_eq!(c.byte(o), g.byte(o), "id byte {o:#x}");
        } else {
            assert_eq!(c.byte(o), t.byte(o), "config byte {o:#x}");
        }
    }
    let mut chain = Vec::new();
    let mut off = c.byte(0x34) as usize;
    while off >= 0x40 {
        chain.push((off, c.byte(off)));
        off = c.byte(off + 1) as usize;
    }
    assert_eq!(chain, [(0x60, 0x01), (0x68, 0x05), (0x78, 0x10)]);
    // NV_XVE_LINK_CAPABILITIES (0x84): max speed 3 (8 GT/s) in 3:0, max width 16 in 9:4.
    let lnkcap = c.dword(0x84);
    assert_eq!((lnkcap & 0xF, (lnkcap >> 4) & 0x3F), (3, 16));
    // The extended space is the TU106 capture's (the loop above), hidden from the config
    // region.
    let mut ext = vec![0u8; crate::config::SIZE - 0x100];
    c.region_read(0x100, &mut ext);
    assert!(ext.iter().all(|&b| b == 0));
}

/// With the whole GB206 config space the Turing RM's fixed-offset read of
/// NV_XVE_LINK_CAPABILITIES lands in Device Capabilities 2 and yields width 25 (what the
/// first GPU run showed, caraxes/runs/costume-2026-09-24/gb206-layout/): the reason the
/// costume takes only the ids.
#[test]
fn the_gb206_layout_would_break_the_turing_link_read() {
    let g = fixture::rtx5060ti().config();
    let v = g.dword(0x84);
    assert_eq!((v & 0xF, (v >> 4) & 0x3F), (3, 25));
}

/// The BARs are the 5060 Ti board's (BAR0 64 MiB), the ROM BAR 512 KiB as on both cards.
#[test]
fn bars_are_the_5060_ti_boards() {
    let b = Board::load("rtx5060ti-on-tu106").unwrap();
    let g = Board::load("rtx5060ti").unwrap();
    assert_eq!(b.bars, g.bars);
    assert_eq!(b.region_bar_sizes(), [64 * MIB, 256 * MIB, 32 * MIB, 0x80]);
    assert_eq!(b.rom_bar_size, g.rom_bar_size);
}

/// BAR0 is the TU106's: the boot registers select Turing, the config mirror carries the
/// costume's ids (it mirrors config space), the PROM window serves the image it is given.
#[test]
fn bar0_boot_registers_are_tu106() {
    let f = fixture::rtx5060ti_on_tu106();
    let t = fixture::rtx2070();
    assert_eq!(f.chip.data().name, "tu106");
    assert_eq!(f.chip.data().identity, t.chip.data().identity);
    let cfg = f.config();
    let mem = GuestMemory::new();
    let mut bar0 = Bar0::new(f.chip, vec![0x55, 0xAA, 0x7F], None);
    let mut r = |off: u64| {
        let mut b = [0u8; 4];
        bar0.read(&cfg, &mem, off, &mut b);
        u32::from_le_bytes(b)
    };
    assert_eq!(r(NV_PMC_BOOT_0), 0x1660_00A1);
    assert_eq!(r(NV_PMC_BOOT_1), 0);
    assert_eq!(r(NV_PMC_BOOT_42), 0x166A_1000);
    let pcfg = f.chip.data().bar0.pcfg;
    assert_eq!(r(pcfg), 0x2D04_10DE);
    assert_eq!(r(pcfg + 0x2C), 0x8A08_1043);
    assert_eq!(r(f.chip.data().bar0.prom) & 0xFF_FFFF, 0x7F_AA55);
}

/// The static identity: the costume's name through the naming model, the chip's short
/// name, the Turing VBIOS version of the image the guest parses.
#[test]
fn static_identity_names_the_5060_ti_and_keeps_tu106() {
    let f = fixture::rtx5060ti_on_tu106();
    let id = f.identity(1 << 30);
    assert_eq!(id.gpu_name, "Leandro VFIO RTX 5060 Ti-1G");
    assert_eq!(id.short_name, "TU106");
    assert_eq!((id.pci.device, id.pci.revision), (0x2D04, 0xA1));
    assert_eq!(
        (id.vbios_revision, id.vbios_oem_revision),
        (0x9006_3D00, 0x76)
    );
    assert_eq!(id.bar_sizes, [64 * MIB, 256 * MIB, 32 * MIB, 0x80]);
    assert_eq!(f.identity(4 << 30).gpu_name, "Leandro VFIO RTX 5060 Ti-4G");
}

/// The GSP static info differs from the RTX 2070's only in what the costume changes: the
/// name, the GID seeded by it, and the subsystem ids; engine caps, FB regions, FB length,
/// short name, VBIOS revision and everything else are byte-identical.
#[test]
fn static_info_differs_only_in_name_gid_and_subsystem() {
    let f = fixture::rtx5060ti_on_tu106();
    let t = fixture::rtx2070();
    let layout = FbLayout::plain(1 << 30);
    let (a, _) = build_static_info(f.chip.data(), &f.identity(1 << 30), None, &layout);
    let (b, _) = build_static_info(t.chip.data(), &t.identity(1 << 30), None, &layout);
    assert_eq!(a.len(), b.len());
    let allowed = [
        v::SI_GID_DATA..v::SI_GID_DATA + 16,
        v::SI_GPU_NAME..v::SI_GPU_NAME + v::SI_NAME_LEN,
        v::SI_VBIOS_SUB_VENDOR..v::SI_VBIOS_SUB_VENDOR + 4,
        v::SI_VBIOS_SUB_DEVICE..v::SI_VBIOS_SUB_DEVICE + 4,
    ];
    let differ: Vec<usize> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
    assert!(!differ.is_empty());
    for i in &differ {
        assert!(
            allowed.iter().any(|r| r.contains(i)),
            "static info byte {i:#x} differs from the RTX 2070's"
        );
    }
    let sub = |x: &[u8], o: usize| u32::from_le_bytes(x[o..o + 4].try_into().unwrap());
    assert_eq!(sub(&a, v::SI_VBIOS_SUB_VENDOR), 0x1043);
    assert_eq!(sub(&a, v::SI_VBIOS_SUB_DEVICE), 0x8A08);
}

/// The chip tables every capability answer comes from are the TU106's: GMMU v2, the
/// Turing channel classes, the engine list, GR without MIG.
#[test]
fn capability_tables_are_tu106() {
    let d = fixture::rtx5060ti_on_tu106().chip.data();
    assert_eq!(d.gmmu.mmu_format, "gmmu-v2-49bit");
    let c = d.channel;
    // TURING_CHANNEL_GPFIFO_A, TURING_DMA_COPY_A, TURING_COMPUTE_A (cl*.h of 610).
    assert_eq!((c.gpfifo, c.copy, c.compute), (0xC46F, 0xC5B5, 0xC5C0));
    let t = fixture::rtx2070().chip.data();
    assert_eq!(d.engine_caps.types, t.engine_caps.types);
    assert_eq!(d.device_info.engines, t.device_info.engines);
    assert!(!d.gr.mig);
}

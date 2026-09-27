// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Client-side check of a running server, using the same crate's `Client`:
//! region and interrupt tables, the PCI identity, BAR sizing, the boot
//! registers, the PROM window and the config mirror. Runs without a VM.

use std::path::Path;

use anyhow::{bail, ensure, Context, Result};
use vfio_bindings::bindings::vfio::{
    VFIO_PCI_BAR0_REGION_INDEX, VFIO_PCI_CONFIG_REGION_INDEX, VFIO_PCI_INTX_IRQ_INDEX,
    VFIO_PCI_MSIX_IRQ_INDEX, VFIO_PCI_MSI_IRQ_INDEX, VFIO_PCI_ROM_REGION_INDEX,
};
use vfio_user::Client;

use crate::bar0;
use crate::board::Board;
use crate::chip::Chip;
use crate::config::PciIds;
use crate::device::region_size;

fn read32(c: &mut Client, region: u32, offset: u64) -> Result<u32> {
    let mut b = [0u8; 4];
    c.region_read(region, offset, &mut b)
        .with_context(|| format!("read region {region} offset {offset:#x}"))?;
    Ok(u32::from_le_bytes(b))
}

fn write32(c: &mut Client, region: u32, offset: u64, v: u32) -> Result<()> {
    c.region_write(region, offset, &v.to_le_bytes())
        .with_context(|| format!("write region {region} offset {offset:#x}"))
}

fn expect(what: &str, got: u32, want: u32) -> Result<()> {
    ensure!(got == want, "{what}: got {got:#x}, want {want:#x}");
    Ok(())
}

/// What the server should present: the chip, the board and the ids of the captured
/// config space.
pub struct Expect<'a> {
    pub chip: &'static dyn Chip,
    pub board: &'a Board,
    pub pci: PciIds,
    /// Config dword 0x08: class code and revision.
    pub class_revision: u32,
    /// The capability pointer at 0x34.
    pub cap_ptr: u8,
}

/// The value a BAR or ROM register reads after all-ones was written: the size mask
/// with the type bits.
fn sizing_mask(bar: &crate::board::Bar, high_of: Option<u64>) -> u32 {
    use crate::board::Bar;
    match *bar {
        Bar::Mem32 { size, prefetch } => !(size as u32 - 1) | ((prefetch as u32) << 3),
        Bar::Mem64 { size, prefetch } => (!(size - 1) as u32) | 0b100 | ((prefetch as u32) << 3),
        Bar::Mem64High => high_of.map_or(0, |size| (!(size - 1) >> 32) as u32),
        Bar::Io { size } => !(size as u32 - 1) | 1,
        Bar::None => 0,
    }
}

/// Connect to `socket` and verify the E0 device model. Returns the list of checks passed.
pub fn run(socket: &Path, e: &Expect) -> Result<Vec<String>> {
    let identity = e.chip.data().identity;
    let windows = e.chip.data().bar0;
    let mut c = Client::new(socket).with_context(|| format!("connect to {}", socket.display()))?;
    let mut passed = Vec::new();

    for index in [
        VFIO_PCI_BAR0_REGION_INDEX,
        1,
        3,
        5,
        VFIO_PCI_ROM_REGION_INDEX,
        VFIO_PCI_CONFIG_REGION_INDEX,
    ] {
        let Some(r) = c.region(index) else {
            bail!("region {index} missing")
        };
        ensure!(
            r.size == region_size(e.board, index),
            "region {index}: size {:#x}, want {:#x}",
            r.size,
            region_size(e.board, index)
        );
    }
    passed.push("region table".into());

    for (index, count) in [
        (VFIO_PCI_INTX_IRQ_INDEX, 1),
        (VFIO_PCI_MSI_IRQ_INDEX, 1),
        (VFIO_PCI_MSIX_IRQ_INDEX, 0),
    ] {
        let info = c.get_irq_info(index)?;
        ensure!(
            info.count == count,
            "irq index {index}: count {}, want {count}",
            info.count
        );
    }
    passed.push("irq table".into());

    let cfg = VFIO_PCI_CONFIG_REGION_INDEX;
    expect(
        "vendor/device",
        read32(&mut c, cfg, 0x00)?,
        e.pci.device_id(),
    )?;
    expect(
        "class/revision",
        read32(&mut c, cfg, 0x08)?,
        e.class_revision,
    )?;
    expect(
        "subsystem",
        read32(&mut c, cfg, 0x2C)?,
        e.pci.sub_device_id(),
    )?;
    expect(
        "capability pointer",
        read32(&mut c, cfg, 0x34)? & 0xFF,
        e.cap_ptr as u32,
    )?;
    let mut one = [0u8; 1];
    c.region_read(cfg, 0x3D, &mut one)?;
    ensure!(one[0] == 1, "interrupt pin: got {}, want 1 (INTA)", one[0]);
    passed.push("pci identity".into());

    let ones = 0xFFFF_FFFF;
    let mut sizing: Vec<(u64, u32)> = e
        .board
        .bars
        .iter()
        .enumerate()
        .map(|(i, bar)| {
            let low = i.checked_sub(1).map(|j| e.board.bars[j]);
            let high_of = match low {
                Some(crate::board::Bar::Mem64 { size, .. }) => Some(size),
                _ => None,
            };
            (0x10 + 4 * i as u64, sizing_mask(bar, high_of))
        })
        .collect();
    sizing.push((0x30, !(e.board.rom_bar_size as u32 - 1) | 1));
    for (reg, want) in sizing {
        write32(&mut c, cfg, reg, ones)?;
        expect(
            &format!("sizing of config {reg:#x}"),
            read32(&mut c, cfg, reg)?,
            want,
        )?;
        write32(&mut c, cfg, reg, 0)?;
    }
    passed.push("bar sizing".into());

    let b0 = VFIO_PCI_BAR0_REGION_INDEX;
    expect(
        "NV_PMC_BOOT_0",
        read32(&mut c, b0, bar0::NV_PMC_BOOT_0)?,
        identity.boot_0,
    )?;
    expect(
        "NV_PMC_BOOT_1",
        read32(&mut c, b0, bar0::NV_PMC_BOOT_1)?,
        identity.boot_1,
    )?;
    expect(
        "NV_PMC_BOOT_42",
        read32(&mut c, b0, bar0::NV_PMC_BOOT_42)?,
        identity.boot_42,
    )?;
    expect(
        "config mirror",
        read32(&mut c, b0, windows.pcfg)?,
        e.pci.device_id(),
    )?;
    passed.push("boot registers".into());

    // 55 AA, or the IFR header (`NVGI`) of a card-like window (vbios.rs).
    let mut sig = [0u8; 4];
    c.region_read(b0, windows.prom, &mut sig)?;
    ensure!(
        sig[..2] == [0x55, 0xAA] || &sig == b"NVGI",
        "PROM window: signature {sig:02x?}, want 55 aa or NVGI"
    );
    let mut rom_sig = [0u8; 2];
    c.region_read(VFIO_PCI_ROM_REGION_INDEX, 0, &mut rom_sig)?;
    ensure!(
        rom_sig == [0x55, 0xAA],
        "ROM region: signature {rom_sig:02x?}, want 55 aa"
    );
    passed.push("rom".into());

    c.shutdown()?;
    Ok(passed)
}

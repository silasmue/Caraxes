// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! BAR0: the register window of the synthetic GPU.
//!
//! Experiment E0 answers only what the driver reads before it needs a GSP: the
//! three boot registers that select the chip and the runtime variant
//! (docs/caraxes/driver-modes.md), the PCI config-space mirror (`NV_PCFG`) that
//! the kernel RM uses instead of real config cycles (kernel-rm-hw-surface.md),
//! and the PROM window through which the driver reads the VBIOS
//! (gsp-boot-turing.md); both windows sit where the chip data puts them. With the
//! E2 boot model attached, the firmware-boot registers, the kernel-RM surface and
//! the queues are modelled too. Writes to any other offset are latched so that a
//! read-back returns the written value (the honest default while no register has a
//! model); everything else reads as zero. The names for the trace come from the
//! chip's register map ([`crate::chip::ChipData::register_name`]).

use std::collections::BTreeMap;

use crate::boot::BootModel;
use crate::chip::{BootIdentity, Chip};
use crate::config::ConfigSpace;
use crate::guestmem::GuestMemory;

/// BAR0 size (16 MiB).
pub const SIZE: u64 = 16 << 20;

/// The boot registers every NVIDIA GPU answers at the same offsets: the driver
/// reads them before it knows the chip (docs/caraxes/driver-modes.md).
pub const NV_PMC_BOOT_0: u64 = 0x0000_0000;
pub const NV_PMC_BOOT_1: u64 = 0x0000_0004;
pub const NV_PMC_BOOT_42: u64 = 0x0000_0A00;

/// The values of the three boot registers (the chip's).
pub type Identity = BootIdentity;

/// What a BAR0 write did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteEffect {
    /// Landed in the config-space mirror (and therefore in config space).
    Mirror,
    /// Handled by the boot model (falcons, boot status registers).
    Model,
    /// No register model behind it; the value is kept and read back as written.
    Latched,
    /// Read-only register; the value was dropped.
    Dropped,
}

pub struct Bar0 {
    chip: &'static dyn Chip,
    identity: Identity,
    pcfg: std::ops::Range<u64>,
    prom: std::ops::Range<u64>,
    /// The PCI-visible ROM chain, served through the expansion ROM region.
    rom: Vec<u8>,
    /// The image behind the PROM window (the full VBIOS when available).
    vbios: Vec<u8>,
    latched: BTreeMap<u64, u32>,
    boot: Option<BootModel>,
    notes: Vec<String>,
}

impl Bar0 {
    /// `vbios` is the image served through the PROM window; `None` serves `rom`.
    pub fn new(chip: &'static dyn Chip, rom: Vec<u8>, vbios: Option<Vec<u8>>) -> Self {
        let vbios = vbios.unwrap_or_else(|| rom.clone());
        let d = chip.data();
        Bar0 {
            chip,
            identity: d.identity,
            pcfg: d.bar0.pcfg..d.bar0.pcfg + d.bar0.pcfg_size,
            prom: d.bar0.prom..d.bar0.prom + d.bar0.prom_size,
            rom,
            vbios,
            latched: BTreeMap::new(),
            boot: None,
            notes: Vec::new(),
        }
    }

    /// Attach the E2 boot model.
    pub fn with_boot_model(mut self, boot: BootModel) -> Self {
        self.boot = Some(boot);
        self
    }

    pub fn boot(&self) -> Option<&BootModel> {
        self.boot.as_ref()
    }

    pub fn boot_mut(&mut self) -> Option<&mut BootModel> {
        self.boot.as_mut()
    }

    pub fn identity(&self) -> Identity {
        self.identity
    }

    pub fn chip(&self) -> &'static dyn Chip {
        self.chip
    }

    /// Name of the register or block at `offset` for the trace (the chip's map).
    pub fn name(&self, offset: u64) -> Option<&'static str> {
        self.chip.data().register_name(offset)
    }

    pub fn rom(&self) -> &[u8] {
        &self.rom
    }

    pub fn vbios(&self) -> &[u8] {
        &self.vbios
    }

    /// Device reset (vfio-user reset or a PCI function-level reset): latched
    /// registers and the boot model return to their initial state.
    pub fn reset(&mut self) {
        self.latched.clear();
        if let Some(b) = self.boot.as_mut() {
            b.reset();
        }
    }

    /// Notes produced by the boot model since the last call (for the trace).
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    /// The dword at the aligned offset `reg`. One fetch per dword, because
    /// falcon data ports advance on every read.
    fn dword_at(&mut self, cfg: &ConfigSpace, mem: &GuestMemory, reg: u64) -> u32 {
        if self.pcfg.contains(&reg) {
            return cfg.dword((reg - self.pcfg.start) as usize);
        }
        if self.prom.contains(&reg) {
            // Beyond the image the flash reads as erased.
            let i = (reg - self.prom.start) as usize;
            let mut b = [0xFFu8; 4];
            for (k, v) in b.iter_mut().enumerate() {
                if let Some(x) = self.vbios.get(i + k) {
                    *v = *x;
                }
            }
            return u32::from_le_bytes(b);
        }
        match reg {
            NV_PMC_BOOT_0 => self.identity.boot_0,
            NV_PMC_BOOT_1 => self.identity.boot_1,
            NV_PMC_BOOT_42 => self.identity.boot_42,
            _ => {
                // The display engine's registers and pages (display/mod.rs) come first:
                // the fuse there overrides the chip's boot constant.
                if let Some(v) = self.boot.as_mut().and_then(|b| b.display_read32(reg, mem)) {
                    return v;
                }
                if let Some(v) = self.boot.as_mut().and_then(|b| b.read32(reg)) {
                    return v;
                }
                self.latched.get(&reg).copied().unwrap_or(0)
            }
        }
    }

    /// Fill `buf` with the bytes at `offset`. Any size and alignment is accepted;
    /// the driver uses 1, 2, 4 and 8-byte accesses.
    pub fn read(&mut self, cfg: &ConfigSpace, mem: &GuestMemory, offset: u64, buf: &mut [u8]) {
        let mut i = 0usize;
        while i < buf.len() {
            let off = offset + i as u64;
            let reg = off & !3;
            let v = self.dword_at(cfg, mem, reg).to_le_bytes();
            let start = (off & 3) as usize;
            let n = (4 - start).min(buf.len() - i);
            buf[i..i + n].copy_from_slice(&v[start..start + n]);
            i += n;
        }
    }

    pub fn write(
        &mut self,
        cfg: &mut ConfigSpace,
        mem: &GuestMemory,
        offset: u64,
        data: &[u8],
    ) -> WriteEffect {
        if self.pcfg.contains(&offset) {
            cfg.write((offset - self.pcfg.start) as usize, data);
            return WriteEffect::Mirror;
        }
        if matches!(offset & !3, NV_PMC_BOOT_0 | NV_PMC_BOOT_1 | NV_PMC_BOOT_42)
            || self.prom.contains(&offset)
        {
            return WriteEffect::Dropped;
        }
        if let Some(boot) = self.boot.as_mut() {
            if data.len() == 4 && offset % 4 == 0 {
                let v = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                if let Some(notes) = boot.display_write32(offset, v, mem) {
                    self.notes.extend(notes);
                    return WriteEffect::Model;
                }
                if let Some(notes) = boot.write32(offset, v, mem) {
                    self.notes.extend(notes);
                    return WriteEffect::Model;
                }
            } else if let Some(cur) = boot.read32(offset & !3) {
                // Sub-dword write into a modelled register: read-modify-write.
                let mut b = cur.to_le_bytes();
                for (i, &d) in data.iter().enumerate() {
                    let off = offset + i as u64;
                    if off & !3 != offset & !3 {
                        break;
                    }
                    b[(off & 3) as usize] = d;
                }
                if let Some(notes) = boot.write32(offset & !3, u32::from_le_bytes(b), mem) {
                    self.notes.extend(notes);
                    return WriteEffect::Model;
                }
            }
        }
        for (i, &b) in data.iter().enumerate() {
            let off = offset + i as u64;
            let reg = off & !3;
            let shift = (off & 3) * 8;
            let v = self.latched.entry(reg).or_insert(0);
            *v = (*v & !(0xFF << shift)) | ((b as u32) << shift);
        }
        WriteEffect::Latched
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NV_PCFG: u64 = 0x0008_8000;
    const NV_PROM_DATA: u64 = 0x0030_0000;

    fn parts() -> (Bar0, ConfigSpace) {
        let f = crate::fixture::rtx2070();
        (Bar0::new(f.chip, vec![0x55, 0xAA, 0x7F], None), f.config())
    }

    #[test]
    fn boot_registers_and_windows() {
        let (mut bar0, mut cfg) = parts();
        let mem = GuestMemory::new();
        let mut b = [0u8; 4];
        bar0.read(&cfg, &GuestMemory::new(), NV_PMC_BOOT_0, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0x1660_00A1);
        bar0.read(&cfg, &GuestMemory::new(), NV_PMC_BOOT_42, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0x166A_1000);
        bar0.read(&cfg, &GuestMemory::new(), NV_PMC_BOOT_0 + 2, &mut b[..2]);
        assert_eq!(u16::from_le_bytes([b[0], b[1]]), 0x1660);
        let mut eight = [0u8; 8];
        bar0.read(&cfg, &GuestMemory::new(), NV_PMC_BOOT_0, &mut eight);
        assert_eq!(u64::from_le_bytes(eight), 0x1660_00A1);
        bar0.read(&cfg, &GuestMemory::new(), NV_PCFG, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0x1F02_10DE);
        bar0.read(&cfg, &GuestMemory::new(), NV_PROM_DATA, &mut b);
        assert_eq!(b, [0x55, 0xAA, 0x7F, 0xFF]);
        bar0.read(&cfg, &GuestMemory::new(), NV_PROM_DATA + 1, &mut b[..2]);
        assert_eq!(&b[..2], &[0xAA, 0x7F]);
        assert_eq!(
            bar0.write(&mut cfg, &mem, NV_PCFG + 0x6A, &[0x01]),
            WriteEffect::Mirror
        );
        assert!(cfg.msi_enabled());
        assert_eq!(
            bar0.write(&mut cfg, &mem, 0x100C10, &0x228370u32.to_le_bytes()),
            WriteEffect::Latched
        );
        bar0.read(&cfg, &GuestMemory::new(), 0x100C10, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0x228370);
        assert_eq!(
            bar0.write(&mut cfg, &mem, 0x100C11, &[0x11]),
            WriteEffect::Latched
        );
        bar0.read(&cfg, &GuestMemory::new(), 0x100C10, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0x221170);
        assert_eq!(
            bar0.write(&mut cfg, &mem, NV_PMC_BOOT_0, &[0xFF; 4]),
            WriteEffect::Dropped
        );
        bar0.read(&cfg, &GuestMemory::new(), NV_PMC_BOOT_0, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0x1660_00A1);
        let mut full = Bar0::new(
            bar0.chip(),
            vec![0x55, 0xAA],
            Some(vec![0x55, 0xAA, 0x01, 0x02]),
        );
        full.read(&cfg, &GuestMemory::new(), NV_PROM_DATA + 2, &mut b[..2]);
        assert_eq!(&b[..2], &[0x01, 0x02]);
        assert_eq!(full.rom().len(), 2);
        assert_eq!(bar0.name(0x1478), Some("NV_PBUS_SW_SCRATCH(i)"));
        assert_eq!(bar0.name(0xA00), Some("NV_PMC_BOOT_42"));
        assert_eq!(bar0.name(0x110C00), Some("NV_PGSP_QUEUE_HEAD/TAIL"));
        assert_eq!(bar0.name(0x12345), None);
    }

    #[test]
    fn boot_model_takes_over_its_registers() {
        let (bar0, mut cfg) = parts();
        let mem = GuestMemory::new();
        let f = crate::fixture::rtx2070();
        let mut bar0 = bar0.with_boot_model(BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(4 << 30),
            f.identity(4 << 30),
        ));
        let mut b = [0u8; 4];
        bar0.read(&cfg, &GuestMemory::new(), 0x100CE0, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0x4A);
        bar0.read(&cfg, &GuestMemory::new(), 0x110100, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0x10);
        assert_eq!(
            bar0.write(&mut cfg, &mem, 0x1103C0, &1u32.to_le_bytes()),
            WriteEffect::Model
        );
        assert_eq!(bar0.take_notes(), vec!["gsp reset 1".to_string()]);
        bar0.read(&cfg, &GuestMemory::new(), 0x110100, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0);
        // Registers the model does not own still latch.
        assert_eq!(
            bar0.write(&mut cfg, &mem, 0x9400, &7u32.to_le_bytes()),
            WriteEffect::Latched
        );
    }
}

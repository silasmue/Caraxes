// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! PCI configuration space of the synthetic device.
//!
//! Seeded from the real card's `lspci -xxxx` dump (the board's capture,
//! `caraxes/reference/rtx2070/config-space.txt` for the RTX 2070) so vendor, device,
//! subsystem, class, revision and the capability chain match the card byte for byte;
//! the PM, MSI and PCIe capabilities are found by walking that chain. The BAR layout
//! and the ROM BAR size come from the board data (`boards/<board>.toml`). The extended configuration space above 0x100 is zeroed: the
//! extended capabilities of the card are not needed until a reason appears
//! (docs/caraxes/vfio-user.md). BARs, ROM BAR, command register, MSI and the
//! usual control words are writable under PCI rules; everything else is read-only.

use anyhow::{bail, Result};

/// Size of the configuration space region (PCIe).
pub const SIZE: usize = 4096;

pub use crate::board::Bar;

const REG_COMMAND: usize = 0x04;
const REG_CACHE_LINE: usize = 0x0C;
const REG_LATENCY: usize = 0x0D;
const REG_BAR0: usize = 0x10;
const REG_ROM: usize = 0x30;
const REG_INT_LINE: usize = 0x3C;
const REG_CAP_PTR: usize = 0x34;
const CAP_ID_PM: u8 = 0x01;
const CAP_ID_MSI: u8 = 0x05;
const CAP_ID_PCIE: u8 = 0x10;
/// The MSI-X capability id (PCI Local Bus 3.0, 6.8.2).
pub const CAP_ID_MSIX: u8 = 0x11;
/// The vendor-specific capability id (PCI Local Bus 3.0, 6.8.4).
pub const CAP_ID_VNDR: u8 = 0x09;

/// Bytes a standard capability occupies: MSI-X 12, vendor-specific its length byte, other
/// ids their two header bytes.
fn cap_len(bytes: &[u8; SIZE], id: u8, off: usize) -> usize {
    match id {
        CAP_ID_MSIX => 12,
        CAP_ID_VNDR => (bytes[off + 2] as usize).max(3),
        _ => 2,
    }
    .min(0x100 - off)
}

/// Unlink every capability `id` from the standard capability list of a dump and zero
/// its bytes, so the chain the guest walks no longer reaches it and no captured value
/// of it stays readable. Returns how many were removed. The board profile uses it to
/// drop MSI-X (`msix = false`) and the vendor-specific capability.
pub fn unlink_cap(bytes: &mut [u8; SIZE], id: u8) -> usize {
    let mut removed = 0;
    // `prev` is the offset of the pointer byte that leads to `off`.
    let mut prev = REG_CAP_PTR;
    let mut off = (bytes[REG_CAP_PTR] & 0xFC) as usize;
    for _ in 0..48 {
        if off < 0x40 {
            break;
        }
        let next = bytes[off + 1] & 0xFC;
        if bytes[off] == id {
            bytes[prev] = next;
            let len = cap_len(bytes, id, off);
            bytes[off..off + len].fill(0);
            removed += 1;
        } else {
            prev = off + 1;
        }
        off = next as usize;
    }
    removed
}

/// The offset of capability `id` in the standard capability list, or `None`.
fn find_cap(bytes: &[u8; SIZE], id: u8) -> Option<usize> {
    let mut off = (bytes[REG_CAP_PTR] & 0xFC) as usize;
    for _ in 0..48 {
        if off < 0x40 {
            return None;
        }
        if bytes[off] == id {
            return Some(off);
        }
        off = (bytes[off + 1] & 0xFC) as usize;
    }
    None
}

/// The device's config space for `board` from the dump text `dump`: the board's
/// capability filter (MSI-X unlinked unless `msix`), its BAR layout and ROM BAR size.
/// The vendor-specific capability is always removed: on GB206 it is the PBI mailbox
/// (blackwell/gb100/dev_pcfg_pf0.h:32-37), and its data dwords in the capture hold part
/// of the card's UUID, a per-card value no synthetic identity may carry
/// (docs/caraxes/blackwell-gb206.md 2.5).
pub fn for_board(dump: &str, board: &crate::board::Board) -> Result<ConfigSpace> {
    let mut bytes = parse_dump(dump)?;
    unlink_cap(&mut bytes, CAP_ID_VNDR);
    if !board.msix {
        unlink_cap(&mut bytes, CAP_ID_MSIX);
    }
    ConfigSpace::from_dump(bytes, board.bars, board.rom_bar_size)
}

/// The PCI identity words of the function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciIds {
    pub vendor: u16,
    pub device: u16,
    pub revision: u8,
    pub sub_vendor: u16,
    pub sub_device: u16,
}

impl PciIds {
    /// `device << 16 | vendor`, as config dword 0 and the RM's `pciDeviceId`.
    pub fn device_id(&self) -> u32 {
        ((self.device as u32) << 16) | self.vendor as u32
    }

    /// `sub_device << 16 | sub_vendor`, as config dword 0x2C and `pciSubDeviceId`.
    pub fn sub_device_id(&self) -> u32 {
        ((self.sub_device as u32) << 16) | self.sub_vendor as u32
    }
}

/// Parse an `lspci -xxxx` hex dump. Lines that are not `<hex offset>: <bytes>`
/// are skipped, so the header line and comments are harmless. A byte spelled `xx` was
/// redacted by the capture kit (per-card values, `caraxes/tools/lab-capture.sh`
/// `redact_config`) and reads as 0.
pub fn parse_dump(text: &str) -> Result<[u8; SIZE]> {
    let mut bytes = [0u8; SIZE];
    let mut seen = 0usize;
    for line in text.lines() {
        let Some((addr, rest)) = line.split_once(':') else {
            continue;
        };
        let addr = addr.trim();
        if addr.is_empty() || addr.len() > 3 || !addr.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let tokens: Vec<&str> = rest.split_whitespace().collect();
        if tokens.is_empty()
            || !tokens
                .iter()
                .all(|t| t.len() == 2 && (*t == "xx" || t.chars().all(|c| c.is_ascii_hexdigit())))
        {
            continue;
        }
        let base = usize::from_str_radix(addr, 16)?;
        for (i, tok) in tokens.iter().enumerate() {
            let off = base + i;
            if off >= SIZE {
                bail!("config dump offset {off:#x} exceeds {SIZE:#x}");
            }
            bytes[off] = if *tok == "xx" {
                0
            } else {
                u8::from_str_radix(tok, 16)?
            };
            seen = seen.max(off + 1);
        }
    }
    if seen < 0x100 {
        bail!("config dump covers only {seen} bytes, expected at least 256");
    }
    Ok(bytes)
}

/// The live configuration space with its per-byte write mask.
pub struct ConfigSpace {
    bytes: [u8; SIZE],
    wmask: [u8; SIZE],
    flr: bool,
    bars: [Bar; 6],
    rom_size: u64,
    cap_msi: usize,
    cap_pcie: usize,
}

impl ConfigSpace {
    /// Build the device's reset state from the card's dump and the board's BAR
    /// layout: host-assigned BAR addresses, bus-master/memory enables and MSI
    /// programming are cleared; the extended space is zeroed; the write mask follows
    /// PCI rules.
    pub fn from_dump(mut bytes: [u8; SIZE], bars: [Bar; 6], rom_size: u64) -> Result<Self> {
        if u16::from_le_bytes([bytes[0], bytes[1]]) != 0x10DE {
            bail!("config dump is not an NVIDIA device");
        }
        let cap = |id, what| {
            find_cap(&bytes, id)
                .ok_or_else(|| anyhow::anyhow!("config dump has no {what} capability"))
        };
        let (cap_pm, cap_msi, cap_pcie) = (
            cap(CAP_ID_PM, "power management")?,
            cap(CAP_ID_MSI, "MSI")?,
            cap(CAP_ID_PCIE, "PCI Express")?,
        );
        for b in &mut bytes[0x100..] {
            *b = 0;
        }
        bytes[REG_COMMAND] = 0;
        bytes[REG_COMMAND + 1] = 0;
        for (i, bar) in bars.iter().enumerate() {
            let flags: u32 = match bar {
                Bar::Mem32 { prefetch, .. } => (*prefetch as u32) << 3,
                Bar::Mem64 { prefetch, .. } => 0b100 | ((*prefetch as u32) << 3),
                Bar::Mem64High | Bar::None => 0,
                Bar::Io { .. } => 1,
            };
            bytes[REG_BAR0 + 4 * i..REG_BAR0 + 4 * i + 4].copy_from_slice(&flags.to_le_bytes());
        }
        bytes[REG_ROM..REG_ROM + 4].fill(0);
        // MSI: keep 64-bit capability, clear enable and multiple-message enable,
        // clear address and data.
        bytes[cap_msi + 2] &= !0x71;
        bytes[cap_msi + 4..cap_msi + 14].fill(0);

        let mut wmask = [0u8; SIZE];
        wmask[REG_COMMAND] = 0x47; // I/O, memory, bus master, parity error response
        wmask[REG_COMMAND + 1] = 0x05; // SERR enable, INTx disable
        wmask[REG_CACHE_LINE] = 0xFF;
        wmask[REG_LATENCY] = 0xFF;
        wmask[REG_BAR0..REG_BAR0 + 24].fill(0xFF);
        wmask[REG_ROM..REG_ROM + 4].fill(0xFF);
        wmask[REG_INT_LINE] = 0xFF;
        wmask[cap_pm + 4] = 0x03; // power state
        wmask[cap_msi + 2] = 0x71; // enable, multiple message enable
        wmask[cap_msi + 4..cap_msi + 14].fill(0xFF); // address low/high, data
        for reg in [0x08usize, 0x10, 0x28, 0x30] {
            // PCIe device control, link control, device control 2, link control 2
            wmask[cap_pcie + reg] = 0xFF;
            wmask[cap_pcie + reg + 1] = 0xFF;
        }
        Ok(ConfigSpace {
            bytes,
            wmask,
            flr: false,
            bars,
            rom_size,
            cap_msi,
            cap_pcie,
        })
    }

    pub fn byte(&self, off: usize) -> u8 {
        self.bytes.get(off).copied().unwrap_or(0xFF)
    }

    pub fn word(&self, off: usize) -> u16 {
        u16::from_le_bytes([self.byte(off), self.byte(off + 1)])
    }

    pub fn dword(&self, off: usize) -> u32 {
        u32::from_le_bytes([
            self.byte(off),
            self.byte(off + 1),
            self.byte(off + 2),
            self.byte(off + 3),
        ])
    }

    fn set_dword(&mut self, off: usize, v: u32) {
        self.bytes[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }

    pub fn vendor_id(&self) -> u16 {
        self.word(0x00)
    }

    pub fn device_id(&self) -> u16 {
        self.word(0x02)
    }

    pub fn revision(&self) -> u8 {
        self.byte(0x08)
    }

    /// Class code as `base << 16 | sub << 8 | prog_if`.
    pub fn class_code(&self) -> u32 {
        self.dword(0x08) >> 8
    }

    pub fn subsystem(&self) -> (u16, u16) {
        (self.word(0x2C), self.word(0x2E))
    }

    pub fn command(&self) -> u16 {
        self.word(REG_COMMAND)
    }

    pub fn msi_enabled(&self) -> bool {
        self.byte(self.cap_msi + 2) & 1 != 0
    }

    /// The BAR layout the space was built with.
    pub fn bars(&self) -> &[Bar; 6] {
        &self.bars
    }

    /// PCI revision id, class code and the full id words, for the device's identity.
    pub fn ids(&self) -> PciIds {
        let (sub_vendor, sub_device) = self.subsystem();
        PciIds {
            vendor: self.vendor_id(),
            device: self.device_id(),
            revision: self.revision(),
            sub_vendor,
            sub_device,
        }
    }

    /// Current value of BAR `i` as the guest programmed it (flags included).
    pub fn bar(&self, i: usize) -> u32 {
        self.dword(REG_BAR0 + 4 * i)
    }

    /// Copy `buf.len()` bytes from `offset`; bytes outside the space read as 0xFF.
    pub fn read(&self, offset: usize, buf: &mut [u8]) {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = self.byte(offset + i);
        }
    }

    /// True once since the last call if a write set the Initiate Function Level
    /// Reset bit (PCIe device control bit 15); the bit itself always reads 0.
    pub fn take_flr(&mut self) -> bool {
        std::mem::take(&mut self.flr)
    }

    /// Apply a write under the mask, then re-normalize BAR and ROM registers so
    /// that sizing writes read back as the size mask, as PCI requires.
    pub fn write(&mut self, offset: usize, data: &[u8]) {
        for (i, &d) in data.iter().enumerate() {
            let off = offset + i;
            if off >= SIZE {
                break;
            }
            let m = self.wmask[off];
            self.bytes[off] = (self.bytes[off] & !m) | (d & m);
        }
        let end = offset + data.len();
        let devctl_hi = self.cap_pcie + 0x09;
        if self.bytes[devctl_hi] & 0x80 != 0 {
            self.bytes[devctl_hi] &= !0x80;
            self.flr = true;
        }
        let bars = self.bars;
        for (i, bar) in bars.iter().enumerate() {
            let reg = REG_BAR0 + 4 * i;
            if offset < reg + 4 && end > reg {
                let raw = self.dword(reg);
                let v = match bar {
                    Bar::Mem32 { size, prefetch } => {
                        (raw & !(*size as u32 - 1)) | ((*prefetch as u32) << 3)
                    }
                    Bar::Mem64 { size, prefetch } => {
                        (raw & !(*size as u32 - 1)) | 0b100 | ((*prefetch as u32) << 3)
                    }
                    Bar::Mem64High => {
                        let size = match bars[i - 1] {
                            Bar::Mem64 { size, .. } => size,
                            _ => unreachable!("Mem64High must follow Mem64"),
                        };
                        raw & !(((size - 1) >> 32) as u32)
                    }
                    Bar::Io { size } => (raw & !(*size as u32 - 1)) | 1,
                    Bar::None => 0,
                };
                self.set_dword(reg, v);
            }
        }
        if offset < REG_ROM + 4 && end > REG_ROM {
            let raw = self.dword(REG_ROM);
            self.set_dword(REG_ROM, (raw & !(self.rom_size as u32 - 1)) | (raw & 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str = include_str!("../../../caraxes/reference/rtx2070/config-space.txt");

    const CAP_PM: usize = 0x60;
    const CAP_MSI: usize = 0x68;
    const CAP_PCIE: usize = 0x78;

    fn space() -> ConfigSpace {
        let b = crate::board::Board::load("rtx2070").unwrap();
        ConfigSpace::from_dump(parse_dump(DUMP).unwrap(), b.bars, b.rom_bar_size).unwrap()
    }

    #[test]
    fn identity_matches_the_card() {
        let c = space();
        assert_eq!(c.vendor_id(), 0x10DE);
        assert_eq!(c.device_id(), 0x1F02);
        assert_eq!(c.revision(), 0xA1);
        assert_eq!(c.class_code(), 0x030000);
        assert_eq!(c.subsystem(), (0x1458, 0x4011));
        assert_eq!(c.byte(0x34), CAP_PM as u8);
        assert_eq!(c.byte(CAP_PM), 0x01);
        assert_eq!(c.byte(CAP_PM + 1), CAP_MSI as u8);
        assert_eq!(c.byte(CAP_MSI), 0x05);
        assert_eq!(c.byte(CAP_MSI + 1), CAP_PCIE as u8);
        assert_eq!(c.byte(CAP_PCIE), 0x10);
        assert_eq!(c.byte(CAP_PCIE + 1), 0);
        assert_eq!(c.command(), 0);
        assert_eq!((c.cap_msi, c.cap_pcie), (CAP_MSI, CAP_PCIE));
        assert_eq!(c.ids().device_id(), 0x1F02_10DE);
        assert_eq!(c.ids().sub_device_id(), 0x4011_1458);
        assert!(!c.msi_enabled());
        assert_eq!(c.dword(0x100), 0);
    }

    /// The GB206 capture (blackwell-gb206.md 1.2): PM 0x40, MSI 0x48, PCIe 0x60, the
    /// vendor-specific capability 0x9C and MSI-X 0xB0 on the card; the vendor-specific
    /// capability (UUID bytes in its PBI data dwords) is always removed, MSI-X by the
    /// board's `msix = false`, and the extended space (SR-IOV, ARI, the serial number at
    /// 0x2F4) is not presented.
    #[test]
    fn gb206_capture_without_msix() {
        let dump =
            include_str!("../../../caraxes/reference/gb206/gpu-0000_01_00.0/config-space.txt");
        let raw = parse_dump(dump).unwrap();
        assert_eq!(find_cap(&raw, CAP_ID_MSIX), Some(0xB0));
        // The capture kit redacted the DSN serial (0x2F8-0x2FF) to "xx": the header stays.
        assert_eq!(raw[0x2F4], 0x03);
        assert_eq!(raw[0x2F8..0x300], [0u8; 8]);
        let b = crate::board::Board::load("rtx5060ti").unwrap();
        let c = for_board(dump, &b).unwrap();
        assert_eq!(
            (c.vendor_id(), c.device_id(), c.revision()),
            (0x10DE, 0x2D04, 0xA1)
        );
        assert_eq!(c.class_code(), 0x030000);
        assert_eq!(c.subsystem(), (0x1043, 0x8A08));
        let mut chain = Vec::new();
        let mut off = c.byte(0x34) as usize;
        while off >= 0x40 {
            chain.push((off, c.byte(off)));
            off = c.byte(off + 1) as usize;
        }
        assert_eq!(chain, [(0x40, 0x01), (0x48, 0x05), (0x60, 0x10)]);
        // Nothing of the vendor-specific capability (0x9C, with UUID bytes at 0xA8) or
        // of MSI-X (0xB0) stays readable.
        assert!((0x9C..0x100).all(|o| c.byte(o) == 0));
        assert_eq!(raw[0xA8..0xAC], [0u8; 4], "redacted in the capture");
        assert!((0x100..SIZE).all(|o| c.byte(o) == 0));
        assert_eq!((c.cap_msi, c.cap_pcie), (0x48, 0x60));
        // The TU106 dump has no MSI-X: the filter changes nothing there.
        let mut t = parse_dump(DUMP).unwrap();
        let before = t;
        assert_eq!(unlink_cap(&mut t, CAP_ID_MSIX), 0);
        assert_eq!(t, before);
    }

    #[test]
    fn bar_sizing_follows_pci_rules() {
        let mut c = space();
        let ones = 0xFFFF_FFFFu32.to_le_bytes();
        c.write(0x10, &ones);
        assert_eq!(c.bar(0), 0xFF00_0000);
        c.write(0x14, &ones);
        c.write(0x18, &ones);
        assert_eq!(c.bar(1), 0xF000_000C);
        assert_eq!(c.bar(2), 0xFFFF_FFFF);
        c.write(0x1C, &ones);
        c.write(0x20, &ones);
        assert_eq!(c.bar(3), 0xFE00_000C);
        assert_eq!(c.bar(4), 0xFFFF_FFFF);
        c.write(0x24, &ones);
        assert_eq!(c.bar(5), 0xFFFF_FF81);
        c.write(REG_ROM, &ones);
        assert_eq!(c.dword(REG_ROM), 0xFFF8_0001);
        c.write(0x10, &0xFB00_0000u32.to_le_bytes());
        assert_eq!(c.bar(0), 0xFB00_0000);
        c.write(0x04, &[0x07, 0x04]);
        assert_eq!(c.command(), 0x0407);
        c.write(0x00, &[0, 0]);
        assert_eq!(c.vendor_id(), 0x10DE);
        c.write(CAP_MSI + 2, &[0x01]);
        assert!(c.msi_enabled());
        assert!(!c.take_flr());
        c.write(CAP_PCIE + 0x08, &0x8000u16.to_le_bytes());
        assert!(c.take_flr());
        assert!(!c.take_flr());
        assert_eq!(c.word(CAP_PCIE + 0x08) & 0x8000, 0);
    }
}

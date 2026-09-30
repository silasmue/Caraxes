// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! PCI configuration space of the synthetic device.
//!
//! Seeded from the real card's `lspci -xxxx` dump (the board's capture,
//! `caraxes/reference/rtx2070/config-space.txt` for the RTX 2070) so vendor, device,
//! subsystem, class, revision and the capability chain match the card byte for byte;
//! the PM, MSI and PCIe capabilities are found by walking that chain. The BAR layout
//! and the ROM BAR size come from the board data (`boards/<board>.toml`). BARs, ROM BAR,
//! command register, MSI and the usual control words are writable under PCI rules; the
//! rest of the standard header is read-only.
//!
//! The extended configuration space (0x100-0xFFF) holds the capture's bytes, because the
//! Windows driver reads them through the BAR0 mirror `NV_PCFG` before any GSP contact
//! (0x700, and the hidden SR-IOV block's `NV_XVE_SRIOV_CAP_HDR3/5/10/12` at 0xBD8-0xBFC,
//! `kbifCacheVFInfo_TU102`, kernel_bif_tu102.c:235-282) and stops with Code 43 on zeros
//! (docs/caraxes/vfio-user.md, "Extended configuration space"). SR-IOV, ARI and the Device
//! Serial Number are unlinked from the chain and zeroed ([`strip_ext`]); a hidden SR-IOV
//! block (TU106: 0xBCC, not reachable from 0x100) stays, read-only. Writes latch except
//! capability headers, the SR-IOV and Resizable BAR blocks and the AER logs (read-only)
//! and the AER status words (RW1C). The vfio-user config region shows the extended space
//! only with [`EXT_CONFIG_ENV`]`=1`; by default the guest OS reads zeros there, as before,
//! and only the BAR0 mirror serves it.

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

/// Start of the PCIe extended configuration space.
pub const EXT_START: usize = 0x100;
/// Extended capability ids (PCIe base spec 7.6-7.9): AER, Device Serial Number, ARI,
/// SR-IOV, Resizable BAR.
pub const EXT_ID_AER: u16 = 0x0001;
pub const EXT_ID_DSN: u16 = 0x0003;
pub const EXT_ID_ARI: u16 = 0x000E;
pub const EXT_ID_SRIOV: u16 = 0x0010;
pub const EXT_ID_REBAR: u16 = 0x0015;
/// `CARAXES_EXT_CONFIG=1`: the vfio-user config region (the guest OS's view) shows the
/// extended space too, not only the BAR0 mirror.
pub const EXT_CONFIG_ENV: &str = "CARAXES_EXT_CONFIG";

/// Whether `CARAXES_EXT_CONFIG=1` is set.
pub fn ext_config_requested() -> bool {
    std::env::var(EXT_CONFIG_ENV).is_ok_and(|v| v.trim() == "1")
}

fn dw(bytes: &[u8; SIZE], off: usize) -> u32 {
    u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
}

/// The linked extended capability chain from 0x100 as `(offset, id, version)`.
pub fn ext_chain(bytes: &[u8; SIZE]) -> Vec<(usize, u16, u8)> {
    let mut out: Vec<(usize, u16, u8)> = Vec::new();
    let mut off = EXT_START;
    for _ in 0..(SIZE - EXT_START) / 4 {
        let h = dw(bytes, off);
        if h == 0 || h == 0xFFFF_FFFF {
            break;
        }
        out.push((off, h as u16, ((h >> 16) & 0xF) as u8));
        let next = (h >> 20) as usize & !3;
        if next < EXT_START || out.iter().any(|&(o, _, _)| o == next) {
            break;
        }
        off = next;
    }
    out
}

/// Bytes an extended capability of `id` at `off` occupies, for the ones handled here.
fn ext_len(bytes: &[u8; SIZE], id: u16, off: usize) -> usize {
    match id {
        EXT_ID_DSN => 12,
        EXT_ID_ARI => 8,
        EXT_ID_SRIOV => 0x40,
        EXT_ID_AER => 0x48,
        // Header, then one capability and one control dword per resizable BAR (the
        // count in bits 7:5 of the first control word).
        EXT_ID_REBAR => 4 + 8 * (((bytes[off + 8] >> 5) & 7).max(1) as usize),
        _ => 4,
    }
    .min(SIZE - off)
}

/// A dword that looks like the header of extended capability `id`: version 1 and a next
/// pointer that is 0 or a dword offset in the extended space.
fn looks_like_header(bytes: &[u8; SIZE], off: usize, id: u16) -> bool {
    let h = dw(bytes, off);
    let next = (h >> 20) as usize;
    h as u16 == id && (h >> 16) & 0xF == 1 && (next == 0 || (next >= EXT_START && next % 4 == 0))
}

/// Remove what names the physical card or a host-only function from the extended space:
/// SR-IOV, ARI and the Device Serial Number are unlinked from the chain from 0x100 and
/// zeroed (GB206: ARI 0x248, SR-IOV 0x250, DSN 0x2F4); an ARI or DSN outside the chain is
/// zeroed too (TU106: the hidden ARI at 0xC14 that the hidden SR-IOV at 0xBCC leads to).
/// A hidden SR-IOV block stays: the TU106 RM reads its read-only words through `NV_PCFG`.
/// Returns the ranges zeroed as `(offset, length)`; they stay read-only zeros.
pub fn strip_ext(bytes: &mut [u8; SIZE]) -> Vec<(usize, usize)> {
    let mut zeroed = Vec::new();
    let mut prev: Option<usize> = None;
    for (off, id, _) in ext_chain(bytes) {
        if !matches!(id, EXT_ID_SRIOV | EXT_ID_ARI | EXT_ID_DSN) {
            prev = Some(off);
            continue;
        }
        let next = dw(bytes, off) >> 20;
        let len = ext_len(bytes, id, off);
        bytes[off..off + len].fill(0);
        zeroed.push((off, len));
        // The previous header skips it; at 0x100 a null capability keeps the chain.
        let at = prev.unwrap_or(EXT_START);
        let h = if prev.is_some() {
            dw(bytes, at) & 0x000F_FFFF
        } else {
            0
        };
        bytes[at..at + 4].copy_from_slice(&(h | (next << 20)).to_le_bytes());
    }
    for off in (EXT_START..SIZE).step_by(4) {
        for id in [EXT_ID_ARI, EXT_ID_DSN] {
            if !looks_like_header(bytes, off, id) {
                continue;
            }
            let next = dw(bytes, off) & 0xFFF0_0000;
            // A hidden SR-IOV that led here skips it.
            for s in (EXT_START..SIZE).step_by(4) {
                if looks_like_header(bytes, s, EXT_ID_SRIOV) && (dw(bytes, s) >> 20) as usize == off
                {
                    let h = (dw(bytes, s) & 0x000F_FFFF) | next;
                    bytes[s..s + 4].copy_from_slice(&h.to_le_bytes());
                }
            }
            let len = ext_len(bytes, id, off);
            bytes[off..off + len].fill(0);
            zeroed.push((off, len));
        }
    }
    zeroed
}

/// `NV_XVE_CYA_2` (0x704, the RM's MSI re-arm, written with the value ignored): kept
/// read-only as before the extended space was served, so the re-arm changes nothing.
const XVE_CYA_2: usize = 0x704;

/// The extended space's write rules: everything latches except capability headers, the
/// SR-IOV and Resizable BAR blocks (no VFs, no BAR resizing in the model) and the AER
/// logs, which are read-only, and the AER status words, which are RW1C; the `stripped`
/// ranges stay read-only zeros. Sets `wmask` over 0x100-0xFFF and returns the RW1C mask.
fn ext_masks(
    bytes: &[u8; SIZE],
    stripped: &[(usize, usize)],
    wmask: &mut [u8; SIZE],
) -> [u8; SIZE] {
    let mut w1c = [0u8; SIZE];
    wmask[EXT_START..].fill(0xFF);
    wmask[XVE_CYA_2..XVE_CYA_2 + 4].fill(0);
    for &(off, len) in stripped {
        wmask[off..off + len].fill(0);
    }
    for (off, id, _) in ext_chain(bytes) {
        wmask[off..off + 4].fill(0);
        let len = ext_len(bytes, id, off);
        match id {
            EXT_ID_REBAR => wmask[off..off + len].fill(0),
            EXT_ID_AER => {
                // 0x04 uncorrectable and 0x10 correctable status: RW1C; 0x08, 0x0C, 0x14
                // masks and severity latch; 0x18 capabilities and control: ECRC
                // generation and check enable (bits 6, 8) only; from 0x1C: header and
                // TLP prefix logs, read-only.
                for st in [0x04, 0x10] {
                    wmask[off + st..off + st + 4].fill(0);
                    w1c[off + st..off + st + 4].fill(0xFF);
                }
                wmask[off + 0x18..off + 0x1C].copy_from_slice(&0x140u32.to_le_bytes());
                wmask[off + 0x1C..off + len].fill(0);
            }
            _ => {}
        }
    }
    for off in (EXT_START..SIZE).step_by(4) {
        if looks_like_header(bytes, off, EXT_ID_SRIOV) {
            let len = ext_len(bytes, EXT_ID_SRIOV, off);
            wmask[off..off + len].fill(0);
        }
    }
    w1c
}

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
    for_board_with_ids(dump, None, board)
}

/// The identity words a board may take from another card's capture (`ids_from`, the
/// costume board of docs/caraxes/costume-5060.md): vendor and device id (0x00-0x03),
/// revision and class code (0x08-0x0B), subsystem vendor and id (0x2C-0x2F). Nothing
/// else: the capability chain stays where the chip's kernel RM reads it at fixed
/// offsets (Turing: `NV_XVE_LINK_CAPABILITIES` 0x84, `NV_XVE_LINK_CONTROL_STATUS` 0x88,
/// maxwell/gm107/dev_nv_xve.h:104-105, through `kbifGetBusOptionsAddr_GM107`,
/// kernel_bif_gm107.c:993-1025).
pub const ID_BYTES: [std::ops::Range<usize>; 3] = [0x00..0x04, 0x08..0x0C, 0x2C..0x30];

/// Copy the [`ID_BYTES`] of the capture `ids` over `bytes`.
pub fn overlay_ids(bytes: &mut [u8; SIZE], ids: &[u8; SIZE]) {
    for r in ID_BYTES {
        bytes[r.clone()].copy_from_slice(&ids[r]);
    }
}

/// [`for_board`] with the identity words of `ids_dump` (the board's `ids_from` capture)
/// laid over `dump` when given.
pub fn for_board_with_ids(
    dump: &str,
    ids_dump: Option<&str>,
    board: &crate::board::Board,
) -> Result<ConfigSpace> {
    let mut bytes = parse_dump(dump)?;
    if let Some(ids) = ids_dump {
        overlay_ids(&mut bytes, &parse_dump(ids)?);
    }
    if let Some(ids) = &board.ids {
        // A costume-ladder board without a capture of its card (docs/caraxes/
        // costume-ladder.md): the device id, and the subsystem ids when given.
        bytes[2..4].copy_from_slice(&ids.device.to_le_bytes());
        if let Some(v) = ids.sub_vendor {
            bytes[0x2C..0x2E].copy_from_slice(&v.to_le_bytes());
        }
        if let Some(d) = ids.sub_device {
            bytes[0x2E..0x30].copy_from_slice(&d.to_le_bytes());
        }
    }
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
#[derive(Clone)]
pub struct ConfigSpace {
    bytes: [u8; SIZE],
    wmask: [u8; SIZE],
    /// Write-1-to-clear bits (the AER status words).
    w1c: [u8; SIZE],
    /// Whether the config region (the guest OS's view) shows the extended space.
    ext_visible: bool,
    flr: bool,
    bars: [Bar; 6],
    rom_size: u64,
    cap_msi: usize,
    cap_pcie: usize,
}

impl ConfigSpace {
    /// Build the device's reset state from the card's dump and the board's BAR
    /// layout: host-assigned BAR addresses, bus-master/memory enables and MSI
    /// programming are cleared; SR-IOV, ARI and the serial number are stripped from the
    /// extended space ([`strip_ext`]); the write mask follows PCI rules.
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
        let stripped = strip_ext(&mut bytes);
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
        let w1c = ext_masks(&bytes, &stripped, &mut wmask);
        Ok(ConfigSpace {
            bytes,
            wmask,
            w1c,
            ext_visible: false,
            flr: false,
            bars,
            rom_size,
            cap_msi,
            cap_pcie,
        })
    }

    /// The PCIe capability's Link Control register (PCIe base spec 7.5.3.7: ASPM Control
    /// 1:0).
    pub fn link_control(&self) -> u16 {
        self.word(self.cap_pcie + 0x10)
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

    /// Show the extended space in the config region ([`EXT_CONFIG_ENV`]).
    pub fn set_ext_visible(&mut self, on: bool) {
        self.ext_visible = on;
    }

    /// A read of the vfio-user config region: [`Self::read`], with the extended space
    /// reading 0 unless it is visible (the BAR0 mirror always reads it).
    pub fn region_read(&self, offset: usize, buf: &mut [u8]) {
        self.read(offset, buf);
        if !self.ext_visible {
            for (i, b) in buf.iter_mut().enumerate() {
                if (EXT_START..SIZE).contains(&(offset + i)) {
                    *b = 0;
                }
            }
        }
    }

    /// A write through the vfio-user config region: [`Self::write`], with writes to the
    /// extended space dropped unless it is visible.
    pub fn region_write(&mut self, offset: usize, data: &[u8]) {
        if self.ext_visible {
            return self.write(offset, data);
        }
        let n = EXT_START.saturating_sub(offset).min(data.len());
        if n > 0 {
            self.write(offset, &data[..n]);
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
            self.bytes[off] &= !(d & self.w1c[off]);
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
        // The extended space holds the capture (VC at 0x100); the config region hides it.
        assert_eq!(c.dword(0x100), 0x2501_0002);
        let mut b = [0u8; 4];
        c.region_read(0x100, &mut b);
        assert_eq!(b, [0; 4]);
    }

    /// The capture's extended chain: VC, LTR, L1SS, Power Budgeting, AER, VSEC, Secondary
    /// PCIe, Resizable BAR; no SR-IOV, ARI or DSN is linked on TU106.
    #[test]
    fn ext_chain_of_the_rtx2070() {
        let c = space();
        assert_eq!(
            ext_chain(&c.bytes),
            [
                (0x100, 0x02, 1),
                (0x250, 0x18, 1),
                (0x258, 0x1E, 1),
                (0x128, 0x04, 1),
                (0x420, EXT_ID_AER, 2),
                (0x600, 0x0B, 1),
                (0x900, 0x19, 1),
                (0xBB0, EXT_ID_REBAR, 1),
            ]
        );
    }

    /// The dwords the Windows driver reads through `NV_PCFG` before Code 43: 0x700 and
    /// the hidden SR-IOV words 0xBD8, 0xBE0, 0xBF4, 0xBFC come from the capture.
    #[test]
    fn mirror_serves_the_capture_ext_dwords() {
        let c = space();
        for (off, v) in [
            (0x700, 0x0002_0FFE),
            (0xBD8, 0x003F_003F),
            (0xBE0, 0x0001_0004),
            (0xBF4, 0x0000_000C),
            (0xBFC, 0x0000_000C),
        ] {
            assert_eq!(c.dword(off), v, "{off:#x}");
        }
        // Through BAR0 as well.
        let f = crate::fixture::rtx2070();
        let mut bar0 = crate::bar0::Bar0::new(f.chip, vec![0x55, 0xAA], None);
        let cfg = f.config();
        let mut b = [0u8; 4];
        let pcfg = f.chip.data().bar0.pcfg;
        bar0.read(
            &cfg,
            &crate::guestmem::GuestMemory::new(),
            pcfg + 0x700,
            &mut b,
        );
        assert_eq!(u32::from_le_bytes(b), 0x0002_0FFE);
        bar0.read(
            &cfg,
            &crate::guestmem::GuestMemory::new(),
            pcfg + 0xBD8,
            &mut b,
        );
        assert_eq!(u32::from_le_bytes(b), 0x003F_003F);
    }

    /// The driver's read-modify-write at 0x700 reads back; the hidden SR-IOV block, the
    /// Resizable BAR block and capability headers stay; AER status is RW1C; NV_XVE_CYA_2
    /// stays read-only.
    #[test]
    fn ext_write_rules() {
        let mut c = space();
        c.write(0x700, &0x0002_0000u32.to_le_bytes());
        assert_eq!(c.dword(0x700), 0x0002_0000);
        c.write(0x704, &0xFFFF_FFFFu32.to_le_bytes());
        assert_eq!(c.dword(0x704), 0);
        for off in [0xBCC, 0xBD4, 0xBD8, 0xBF4, 0xBB8, 0xBC0, 0x100, 0x420] {
            let before = c.dword(off);
            c.write(off, &0x5A5A_5A5Au32.to_le_bytes());
            assert_eq!(c.dword(off), before, "{off:#x}");
        }
        // AER correctable mask latches, the status clears on 1.
        c.write(0x434, &0x0000_2000u32.to_le_bytes());
        assert_eq!(c.dword(0x434), 0x0000_2000);
        c.bytes[0x430] = 0x41;
        c.write(0x430, &0x0000_0001u32.to_le_bytes());
        assert_eq!(c.dword(0x430), 0x40);
        // The config region drops extended writes until the space is visible.
        c.region_write(0x708, &0x1234_5678u32.to_le_bytes());
        assert_eq!(c.dword(0x708), 0x0987_E1ED);
        c.set_ext_visible(true);
        c.region_write(0x708, &0x1234_5678u32.to_le_bytes());
        assert_eq!(c.dword(0x708), 0x1234_5678);
        let mut b = [0u8; 4];
        c.region_read(0x100, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0x2501_0002);
        // A write across 0x100 keeps its standard-space part when hidden.
        let mut h = space();
        h.region_write(0xFC, &[0; 8]);
        assert_eq!(h.dword(0x100), 0x2501_0002);
    }

    /// Cleared: the hidden ARI (0xC14) behind the hidden SR-IOV on TU106; on GB206 the
    /// linked ARI, SR-IOV and DSN.
    #[test]
    fn stripped_capabilities_stay_zero() {
        let mut c = space();
        assert_eq!(c.dword(0xC14), 0);
        assert_eq!(c.dword(0xC18), 0);
        assert_eq!(
            c.dword(0xBCC) >> 20,
            0,
            "hidden SR-IOV no longer leads to ARI"
        );
        c.write(0xC14, &0xFFFF_FFFFu32.to_le_bytes());
        assert_eq!(c.dword(0xC14), 0, "a stripped capability stays zero");
        let dump =
            include_str!("../../../caraxes/reference/gb206/gpu-0000_01_00.0/config-space.txt");
        let g = for_board(dump, &crate::board::Board::load("rtx5060ti").unwrap()).unwrap();
        let ids: Vec<u16> = ext_chain(&g.bytes).iter().map(|&(_, id, _)| id).collect();
        assert_eq!(
            ids,
            [0x19, 0x18, 0x15, 0x24, 0x25, 0x26, 0x2A, 0x01, 0x27, 0x0B, 0x04]
        );
        for (off, len) in [(0x248, 8), (0x250, 0x40), (0x2F4, 12)] {
            assert!((off..off + len).all(|o| g.byte(o) == 0), "{off:#x}");
        }
        // Lane Margining (0x200) now leads to the VSEC (0x2A4), Power Budgeting ends it.
        assert_eq!(g.dword(0x200) >> 20, 0x2A4);
        assert_eq!(g.dword(0x2BC) >> 20, 0);
    }

    /// The GB206 capture (blackwell-gb206.md 1.2): PM 0x40, MSI 0x48, PCIe 0x60, the
    /// vendor-specific capability 0x9C and MSI-X 0xB0 on the card; the vendor-specific
    /// capability (UUID bytes in its PBI data dwords) is always removed, MSI-X by the
    /// board's `msix = false`, and SR-IOV, ARI and the serial number at 0x2F4 are stripped
    /// from the extended space; the config region shows none of it.
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
        let mut ext = vec![0u8; SIZE - 0x100];
        c.region_read(0x100, &mut ext);
        assert!(ext.iter().all(|&b| b == 0));
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

    /// A ladder board without a capture: `[ids]` sets the device id (and the subsystem when
    /// given) over the RTX 2070 layout; nothing else of the config space changes.
    #[test]
    fn ladder_ids_over_the_rtx2070_layout() {
        let dump = include_str!("../../../caraxes/reference/rtx2070/config-space.txt");
        let plain = for_board(dump, &crate::board::Board::load("rtx2070").unwrap()).unwrap();
        let g = crate::board::Board::load("rtx3060-on-tu106").unwrap();
        let h = crate::board::Board::load("h100-on-tu106").unwrap();
        let a = for_board(dump, &g).unwrap();
        let b = for_board(dump, &h).unwrap();
        assert_eq!(
            (a.vendor_id(), a.device_id(), a.revision()),
            (0x10DE, 0x2503, 0xA1)
        );
        assert_eq!(a.subsystem(), plain.subsystem());
        assert_eq!(b.subsystem(), (0x10DE, 0x1626));
        assert!((0x08..0x100)
            .filter(|o| !(0x2C..0x30).contains(o))
            .all(|o| a.byte(o) == plain.byte(o)));
    }
}

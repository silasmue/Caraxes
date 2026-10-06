// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
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
//! model). A read of an offset no model answers and nothing latched comes from the
//! captured table ([`CapturedRegisters`], the physical card's values read once through
//! the host RM) when one is loaded; everything else reads as zero. The names for the
//! trace come from the chip's register map ([`crate::chip::ChipData::register_name`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
/// `NV_PMC_ENABLE` (tu102/dev_mc.h): latched like any unmodelled register, but its rising
/// engine bits reset falcons in the firmware model (`FirmwareBoot::observe_write`).
pub const NV_PMC_ENABLE: u64 = 0x0000_0200;

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

/// BAR0 registers of the physical card, captured once through the host RM
/// (`caraxes/tools/hostreg-read.c`, EXEC_REG_OPS READ_32) and kept in the board's reference
/// directory (`caraxes/reference/rtx2070/bar0-registers-<date>.txt`: `#` header lines, then
/// `offset value` in hex). They answer reads of offsets that no model owns and nothing
/// latched: the Windows driver's chip-identification reads before its GSP decision
/// (caraxes/runs/win11-2026-09-25/README.md, "The Windows driver's first contact"). A
/// modelled register is never overridden.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CapturedRegisters {
    regs: BTreeMap<u64, u32>,
}

impl CapturedRegisters {
    /// The file prefix in a reference directory.
    pub const PREFIX: &'static str = "bar0-registers-";

    /// Parse `offset value` lines (hex, `0x` optional); `#` starts a comment. An offset must
    /// be dword aligned and inside BAR0; an offset twice must carry the same value.
    pub fn parse(text: &str) -> Result<Self, String> {
        let hex = |t: &str| u64::from_str_radix(t.trim_start_matches("0x"), 16);
        let mut regs = BTreeMap::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let mut f = line.split_whitespace();
            let (Some(o), Some(v), None) = (f.next(), f.next(), f.next()) else {
                return Err(format!("line {}: expected `offset value`", n + 1));
            };
            let off = hex(o).map_err(|e| format!("line {}: offset {o:?}: {e}", n + 1))?;
            let val = hex(v)
                .ok()
                .and_then(|v| u32::try_from(v).ok())
                .ok_or_else(|| format!("line {}: value {v:?} is no 32-bit hex number", n + 1))?;
            if off % 4 != 0 || off >= SIZE {
                return Err(format!(
                    "line {}: offset {off:#x} unaligned or beyond BAR0",
                    n + 1
                ));
            }
            if regs.insert(off, val).is_some_and(|old| old != val) {
                return Err(format!(
                    "line {}: offset {off:#x} twice with different values",
                    n + 1
                ));
            }
        }
        Ok(CapturedRegisters { regs })
    }

    /// Load a capture; a comma-separated list of files is merged (the BAR0 capture and the
    /// PMU/SEC2 falcon capture, `falcon-pmu-sec2-<date>.txt`). An offset in two files must
    /// carry the same value.
    pub fn load(path: &Path) -> Result<Self, String> {
        let s = path.to_string_lossy();
        if s.contains(',') {
            let mut all = CapturedRegisters::default();
            for p in s.split(',').filter(|p| !p.is_empty()) {
                all.merge(Self::load(Path::new(p))?)
                    .map_err(|e| format!("{p}: {e}"))?;
            }
            return Ok(all);
        }
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Add `other`'s registers; an offset both hold must carry the same value.
    pub fn merge(&mut self, other: CapturedRegisters) -> Result<(), String> {
        for (off, v) in other.regs {
            if self.regs.insert(off, v).is_some_and(|old| old != v) {
                return Err(format!("offset {off:#x} captured twice with different values"));
            }
        }
        Ok(())
    }

    /// The newest capture in `dir` (the greatest `bar0-registers-*.txt` name; the date
    /// sorts), or `None`.
    pub fn newest_in(dir: &Path) -> Option<PathBuf> {
        std::fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(Self::PREFIX) && n.ends_with(".txt"))
            })
            .max()
    }

    /// A capture of another chip must not answer for this one: when the table holds
    /// `NV_PMC_BOOT_0`, it must be the presented identity's.
    pub fn check_identity(&self, boot_0: u32) -> Result<(), String> {
        match self.regs.get(&NV_PMC_BOOT_0) {
            Some(&v) if v != boot_0 => Err(format!(
                "captured NV_PMC_BOOT_0 {v:#010x} is not the presented chip's {boot_0:#010x}"
            )),
            _ => Ok(()),
        }
    }

    pub fn get(&self, off: u64) -> Option<u32> {
        self.regs.get(&off).copied()
    }

    pub fn len(&self) -> usize {
        self.regs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.regs.is_empty()
    }
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
    /// The physical card's captured registers for unmodelled offsets (none: they read 0).
    captured: Option<CapturedRegisters>,
    /// A dword of the last read came from `captured` (the trace's `captured` note).
    served_captured: bool,
    boot: Option<BootModel>,
    notes: Vec<String>,
    /// The interrupt page (host execution): page 0 and the interrupt tree's page are memory the guest
    /// maps (intr_page.rs); a trapped access there goes to that memory too.
    page: Option<std::sync::Arc<crate::intr_page::IntrPage>>,
    /// `CARAXES_DISPLAY_PAGE=1`: the display front-end page 0x611000 is memory too
    /// (display/fe_page.rs).
    fe_page: Option<std::sync::Arc<crate::display::fe_page::FePage>>,
    /// The MMU invalidate page (`CARAXES_INVALIDATE_PAGE=1`, inv_page.rs).
    inv_page: Option<std::sync::Arc<crate::inv_page::InvPage>>,
    /// The usermode page (`CARAXES_USERMODE_PAGE=1`, um_page.rs).
    um_page: Option<std::sync::Arc<crate::um_page::UmPage>>,
    /// The display kick page (`CARAXES_DISPLAY_KICK_PAGE=1`, display/chan_page.rs).
    chan_page: Option<std::sync::Arc<crate::display::chan_page::ChanPage>>,
    /// An optional exact read-only window USERD page. Unlike `chan_page`, every
    /// guest write to this page stays on the ordinary trapped BAR0 path.
    ro_chan_page: Option<std::sync::Arc<crate::display::ro_chan_page::RoChanPage>>,
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
            latched: crate::families::devinit_scratch(d)
                .map(|(r, v)| BTreeMap::from([(r, v)]))
                .unwrap_or_default(),
            captured: None,
            served_captured: false,
            boot: None,
            notes: Vec::new(),
            page: None,
            fe_page: None,
            inv_page: None,
            um_page: None,
            chan_page: None,
            ro_chan_page: None,
        }
    }

    /// Serve the interrupt page's two pages from its memory (intr_page.rs).
    pub fn with_intr_page(mut self, page: std::sync::Arc<crate::intr_page::IntrPage>) -> Self {
        self.page = Some(page);
        self
    }

    pub fn owns_intr_page(&self, page: &std::sync::Arc<crate::intr_page::IntrPage>) -> bool {
        self.page.as_ref().is_some_and(|owner| std::sync::Arc::ptr_eq(owner, page))
    }

    /// Serve the window channels' USERD pages from their memory (display/chan_page.rs).
    pub fn with_chan_page(
        mut self,
        page: std::sync::Arc<crate::display::chan_page::ChanPage>,
    ) -> Result<Self, String> {
        if self.ro_chan_page.is_some() {
            return Err("writable display kick page overlaps read-only window page".into());
        }
        self.chan_page = Some(page);
        Ok(self)
    }

    /// Attach a guest-specific, read-only window page after the boot model is
    /// present, before the sparse area is offered to a VMM. The legacy writable
    /// kick-page area overlaps it and is therefore incompatible.
    pub fn with_ro_chan_page(
        mut self,
        page: std::sync::Arc<crate::display::ro_chan_page::RoChanPage>,
    ) -> Result<Self, String> {
        if self.chan_page.is_some() {
            return Err("read-only window page overlaps the writable kick page".into());
        }
        if !self.page.as_ref().is_some_and(|intr| page.belongs_to(intr)) {
            return Err("read-only window page belongs to a different interrupt memfd".into());
        }
        let boot = self.boot.as_mut().ok_or("read-only window page needs the boot model")?;
        boot.ctrl.display.attach_ro_chan_page(page.clone())?;
        self.ro_chan_page = Some(page);
        Ok(self)
    }

    pub fn has_ro_chan_page(&self) -> bool {
        self.ro_chan_page.is_some()
    }

    /// Erase the old client's mapped register image before ending its backend.
    pub fn clear_ro_chan_page(&self) {
        if let Some(page) = &self.ro_chan_page {
            page.clear();
        }
    }

    /// End model activity after a terminal RO-page error. The caller keeps the
    /// device's host-memory reservation for the normal worker-join teardown.
    pub fn retire_boot_model(&mut self) {
        if let Some(mut boot) = self.boot.take() {
            self.notes.extend(boot.ctrl.dstate.release_zbc());
        }
    }

    /// Serve the usermode page from its memory (um_page.rs).
    pub fn with_um_page(mut self, page: std::sync::Arc<crate::um_page::UmPage>) -> Self {
        self.um_page = Some(page);
        self
    }

    /// Serve the MMU invalidate page from its memory (inv_page.rs).
    pub fn with_inv_page(mut self, page: std::sync::Arc<crate::inv_page::InvPage>) -> Self {
        self.inv_page = Some(page);
        self
    }

    /// Serve the display page from its memory (display/fe_page.rs).
    pub fn with_fe_page(mut self, page: std::sync::Arc<crate::display::fe_page::FePage>) -> Self {
        self.fe_page = Some(page);
        self
    }

    /// Whether a register model answers the dword at `off` (the interrupt page's start-up
    /// check): the config mirror, the PROM window, the boot constants, the boot model's
    /// registers and the display engine (NV_PDISP [0x610000, 0x700000),
    /// `turing/tu102/dev_disp.h`, and the display fuse, display/mod.rs).
    pub fn model_owns(&self, off: u64) -> bool {
        self.pcfg.contains(&off)
            || self.prom.contains(&off)
            || matches!(off, NV_PMC_BOOT_0 | NV_PMC_BOOT_1 | NV_PMC_BOOT_42)
            || (0x0061_0000..0x0070_0000).contains(&off)
            || off == 0x0002_1C04
            || self.boot.as_ref().is_some_and(|b| b.owns_register(off))
    }

    /// Answer unmodelled, unlatched reads from the captured table.
    pub fn with_captured(mut self, table: CapturedRegisters) -> Self {
        self.captured = Some(table);
        self
    }

    /// Whether a dword of the last [`Bar0::read`] came from the captured table.
    pub fn take_captured(&mut self) -> bool {
        std::mem::take(&mut self.served_captured)
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

    /// The image behind the PROM window, byte for byte at its window offsets.
    pub fn vbios(&self) -> &[u8] {
        &self.vbios
    }

    /// The first PCI image of the PROM window image, located as the RM does (at 0, through
    /// the IFR header, or by scanning; vbios.rs): what the device derives from the VBIOS
    /// for itself starts there, not at offset 0.
    pub fn vbios_image(&self) -> Option<crate::vbios::PciImage> {
        crate::vbios::locate(&self.vbios)
    }

    /// Device reset (vfio-user reset or a PCI function-level reset): latched
    /// registers and the boot model return to their initial state.
    pub fn reset(&mut self) -> Result<(), String> {
        if let Some(page) = &self.ro_chan_page {
            // Erase the previous guest's image before rebuilding its model. A
            // failed reattachment cannot leave old register values mapped.
            page.clear();
        }
        self.latched.clear();
        // The devinit scratch a real card's devinit sets at every reset (families/mod.rs).
        if let Some((r, v)) = crate::families::devinit_scratch(self.chip.data()) {
            self.latched.insert(r, v);
        }
        // The boot model first: its old interrupt sources (the mirror's host event thread,
        // joined when the old mirror drops) stop before the interrupt page forgets what was
        // raised (C65: two host notifier raises of the old mirror reached the fresh tree,
        // the guest enabled the vector and never serviced it).
        if let Some(b) = self.boot.as_mut() {
            b.reset();
            if let Some(page) = &self.ro_chan_page {
                b.ctrl.display.attach_ro_chan_page(page.clone()).map_err(|e| {
                    format!("read-only window page: reset attachment failed with erased image: {e}")
                })?;
            }
        } else if self.ro_chan_page.is_some() {
            return Err("read-only window page: boot model disappeared after erasing image".into());
        }
        if let Some(p) = &self.page {
            p.reset();
        }
        Ok(())
    }

    /// Notes produced by the boot model since the last call (for the trace).
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    /// The dword at the aligned offset `reg`. One fetch per dword, because
    /// falcon data ports advance on every read.
    fn dword_at(&mut self, cfg: &ConfigSpace, mem: &GuestMemory, reg: u64) -> u32 {
        if let Some(p) = self.page.as_ref().filter(|p| p.covers(reg)) {
            return p.read32(reg);
        }
        if let Some(p) = self.fe_page.as_ref().filter(|p| p.covers(reg)) {
            return p.read32(reg);
        }
        if let Some(p) = self.inv_page.as_ref().filter(|p| p.covers(reg)) {
            return p.read32(reg);
        }
        if let Some(p) = self.um_page.as_ref().filter(|p| p.covers(reg)) {
            return p.read32(reg);
        }
        if let Some(p) = self.chan_page.as_ref().filter(|p| p.covers(reg)) {
            return p.read32(reg);
        }
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
                if let Some(&v) = self.latched.get(&reg) {
                    return v;
                }
                match self.captured.as_ref().and_then(|c| c.get(reg)) {
                    Some(v) => {
                        self.served_captured = true;
                        v
                    }
                    None => 0,
                }
            }
        }
    }

    /// Fill `buf` with the bytes at `offset`. Any size and alignment is accepted;
    /// the driver uses 1, 2, 4 and 8-byte accesses.
    pub fn read(&mut self, cfg: &ConfigSpace, mem: &GuestMemory, offset: u64, buf: &mut [u8]) {
        self.served_captured = false;
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
        if let Some(p) = self.page.as_ref().filter(|p| p.covers(offset)) {
            // The tree page: applied at once with the register's semantics (exact mode,
            // intr_page.rs); page 0: stored as the guest would store it.
            p.write(offset, data);
            return WriteEffect::Model;
        }
        if let Some(p) = self.fe_page.as_ref().filter(|p| {
            p.covers(offset) || (p.exact_readonly() && !data.is_empty()
                && offset < p.area().0 + p.area().1
                && offset.checked_add(data.len() as u64).is_some_and(|end| end > p.area().0))
        }) {
            // Exact accesses must fit wholly inside the FE mapping. Do not partially
            // apply a crossing access or report a rejected write as a model update.
            if p.exact_readonly() && !p.covers_range(offset, data.len()) {
                return WriteEffect::Dropped;
            }
            p.write(offset, data);
            return WriteEffect::Model;
        }
        if let Some(p) = self.inv_page.as_ref().filter(|p| p.covers(offset)) {
            p.write(offset, data);
            return WriteEffect::Model;
        }
        if let Some(p) = self.um_page.as_ref().filter(|p| p.covers(offset)) {
            // A doorbell lands in the page; the reactor finds it.
            p.write(offset, data);
            return WriteEffect::Model;
        }
        if let Some(p) = self.chan_page.as_ref().filter(|p| p.covers(offset)) {
            // A kick lands in the page; the reactor runs it.
            p.write(offset, data);
            return WriteEffect::Model;
        }
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
        // NV_PMC_ENABLE latches; the firmware model watches its engine bits (the PMU reset).
        let pmc = offset & !3 == NV_PMC_ENABLE;
        let old = pmc.then(|| {
            self.latched
                .get(&NV_PMC_ENABLE)
                .copied()
                .or_else(|| self.captured.as_ref().and_then(|c| c.get(NV_PMC_ENABLE)))
                .unwrap_or(0)
        });
        for (i, &b) in data.iter().enumerate() {
            let off = offset + i as u64;
            let reg = off & !3;
            let shift = (off & 3) * 8;
            let v = self.latched.entry(reg).or_insert(0);
            *v = (*v & !(0xFF << shift)) | ((b as u32) << shift);
        }
        if let (Some(old), Some(boot)) = (old, self.boot.as_mut()) {
            let new = self.latched.get(&NV_PMC_ENABLE).copied().unwrap_or(0);
            self.notes
                .extend(boot.observe_write(NV_PMC_ENABLE, old, new));
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
    fn read_only_window_page_preserves_trapped_partial_write_semantics() {
        use crate::display::ro_chan_page::RoChanPage;
        let f = crate::fixture::rtx2070();
        let intr = crate::intr_page::IntrPage::create(
            16 << 20,
            0xb8_1000,
            f.chip.data().identity,
            crate::intr::IntrShared::new(),
            crate::intr_page::Config::default(),
        )
        .unwrap();
        let base = crate::display::WIN_USERD + 4 * crate::intr_page::PAGE;
        let ro = std::sync::Arc::new(RoChanPage::create(&intr, base).unwrap());
        let board = crate::board::Board::load("rtx2070").unwrap();
        let mut boot = BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(4 << 30),
            f.identity(4 << 30),
        );
        boot.ctrl.display = crate::display::Display::from_setup(&crate::display::Setup {
            enabled: true,
            chip: "tu106".into(),
            board: board.display,
        })
        .unwrap();
        let (plain, mut cfg) = parts();
        let mut bar0 = plain
            .with_boot_model(boot)
            .with_intr_page(intr.clone())
            .with_ro_chan_page(ro.clone())
            .unwrap();
        let mem = GuestMemory::new();
        let off = base + 0x100;
        assert_eq!(bar0.write(&mut cfg, &mem, off + 1, &[0x12, 0x34]), WriteEffect::Latched);
        let mut routed = [0; 4];
        let mut mapped = [0; 4];
        bar0.read(&cfg, &mem, off, &mut routed);
        assert!(ro.read(off, &mut mapped));
        assert_eq!(mapped, routed);
        assert_eq!(mapped, [0; 4], "partial display writes do not change Display::read32");

        assert_eq!(bar0.write(&mut cfg, &mem, off, &0x8765_4321u32.to_le_bytes()), WriteEffect::Model);
        bar0.read(&cfg, &mem, off, &mut routed);
        assert!(ro.read(off, &mut mapped));
        assert_eq!(mapped, routed);
        assert_eq!(u32::from_le_bytes(mapped), 0x8765_4321);

        // Simulate a reset that cannot rebuild its display owner. The returned
        // error and erased image must precede any attempt to reuse the memfd.
        bar0.boot = None;
        assert!(bar0.reset().unwrap_err().contains("boot model disappeared"));
        assert!(ro.read(off, &mut mapped));
        assert_eq!(mapped, [0; 4]);

        let writable = crate::display::chan_page::ChanPage::create(&intr).unwrap();
        assert!(bar0.with_chan_page(writable.clone()).err().unwrap().contains("overlaps"));
        let forward = Bar0::new(crate::fixture::rtx2070().chip, Vec::new(), None)
            .with_intr_page(intr)
            .with_chan_page(writable)
            .unwrap();
        assert!(forward.with_ro_chan_page(ro).err().unwrap().contains("overlaps"));
    }

    /// This changes the process-global display Setup, so run it alone:
    /// `cargo test -p vfio-user-nvgpu ro_window_reset_rejects_disabled_setup --lib -- --ignored`.
    #[test]
    #[ignore = "mutates process-global display setup; run alone"]
    fn ro_window_reset_rejects_disabled_setup() {
        use crate::display::ro_chan_page::RoChanPage;
        let f = crate::fixture::rtx2070();
        let board = crate::board::Board::load("rtx2070").unwrap();
        crate::display::configure(crate::display::Setup {
            enabled: true,
            chip: "tu106".into(),
            board: board.display,
        });
        let intr = crate::intr_page::IntrPage::create(
            16 << 20,
            0xb8_1000,
            f.chip.data().identity,
            crate::intr::IntrShared::new(),
            crate::intr_page::Config::default(),
        )
        .unwrap();
        let base = crate::display::WIN_USERD + 4 * crate::intr_page::PAGE;
        let ro = std::sync::Arc::new(RoChanPage::create(&intr, base).unwrap());
        let boot = BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(4 << 30),
            f.identity(4 << 30),
        );
        let (plain, mut cfg) = parts();
        let mut bar0 = plain
            .with_boot_model(boot)
            .with_intr_page(intr)
            .with_ro_chan_page(ro.clone())
            .unwrap();
        let off = base + 0x100;
        let mem = GuestMemory::new();
        assert_eq!(bar0.write(&mut cfg, &mem, off, &0x8765_4321u32.to_le_bytes()), WriteEffect::Model);
        let mut mapped = [0; 4];
        assert!(ro.read(off, &mut mapped));
        assert_eq!(u32::from_le_bytes(mapped), 0x8765_4321);

        crate::display::configure(crate::display::Setup::default());
        let error = bar0.reset().unwrap_err();
        assert!(error.contains("enabled display"), "{error}");
        assert!(ro.read(off, &mut mapped));
        assert_eq!(mapped, [0; 4], "no old-client value after failed reset");
    }

    #[test]
    fn exact_ro_fallback_drops_whole_crossing_access_without_partial_mutation() {
        use crate::display::fe_page::{FePage, Mode, FE_PAGE};
        let intr = crate::intr_page::IntrPage::create(
            16 << 20, 0xb8_1000, crate::fixture::rtx2070().chip.data().identity,
            crate::intr::IntrShared::new(), crate::intr_page::Config::default(),
        ).unwrap();
        let page = FePage::create_with_mode(&intr, Mode::ExactReadOnly, Box::new(|| {})).unwrap();
        let (bar0, mut cfg) = parts();
        let mut bar0 = bar0.with_fe_page(page.clone());
        let mem = GuestMemory::new();
        for (off, data) in [(FE_PAGE - 1, &[7, 8][..]), (FE_PAGE + crate::intr_page::PAGE - 1, &[9, 10][..]), (FE_PAGE, &[][..])] {
            assert_eq!(bar0.write(&mut cfg, &mem, off, data), WriteEffect::Dropped);
            assert_eq!(page.read32(FE_PAGE), 0);
            assert_eq!(page.read32(FE_PAGE + crate::intr_page::PAGE - 4), 0);
            assert!(!bar0.latched.contains_key(&(off & !3)), "no fragment of a crossing access may be latched");
        }
        assert_eq!(bar0.write(&mut cfg, &mem, FE_PAGE + 1, &[0x12, 0x34]), WriteEffect::Model);
        assert_eq!(page.read32(FE_PAGE), 0x0034_1200);
        assert_eq!(bar0.write(&mut cfg, &mem, FE_PAGE + crate::intr_page::PAGE, &[0x55; 4]), WriteEffect::Latched, "non-FE behavior unchanged");
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
            bar0.write(&mut cfg, &mem, 0x9428, &7u32.to_le_bytes()),
            WriteEffect::Latched
        );
    }

    /// The captured table answers unmodelled offsets only: a model, the boot constants and a
    /// latched write win; without a table (the file missing) those offsets read 0.
    #[test]
    fn captured_registers_answer_unmodelled_reads() {
        let text = "# test capture\n0x00000000 0x166000a1\n0x00021434 0x00000001\n\
                    0x001373a0 0x0000000a\n0x00610018 0x04000000\n0x00000a00 0xdeadbeef\n\
                    0x00110100 0x12345678\n";
        let t = CapturedRegisters::parse(text).unwrap();
        assert_eq!(t.len(), 6);
        assert!(t.check_identity(0x1660_00A1).is_ok());
        assert!(t.check_identity(0x1B20_00A1).is_err());
        assert!(CapturedRegisters::parse("0x3 0x1").is_err());
        assert!(CapturedRegisters::parse("0x4 0x1 0x2").is_err());
        assert!(CapturedRegisters::parse("0x4 0x1\n0x4 0x2").is_err());
        assert!(CapturedRegisters::parse("0x4 0x100000000").is_err());
        assert!(CapturedRegisters::parse("0x1000000 0x1").is_err());
        assert!(CapturedRegisters::load(Path::new("/nonexistent/bar0-registers-x.txt")).is_err());

        let (bar0, mut cfg) = parts();
        let mem = GuestMemory::new();
        let f = crate::fixture::rtx2070();
        let mut bar0 = bar0
            .with_boot_model(BootModel::new(
                f.chip,
                crate::fb::FbLayout::plain(4 << 30),
                f.identity(4 << 30),
            ))
            .with_captured(t);
        let rd = |bar0: &mut Bar0, cfg: &ConfigSpace, off: u64| {
            let mut b = [0u8; 4];
            bar0.read(cfg, &mem, off, &mut b);
            (u32::from_le_bytes(b), bar0.take_captured())
        };
        assert_eq!(rd(&mut bar0, &cfg, 0x21434), (1, true));
        assert_eq!(rd(&mut bar0, &cfg, 0x1373A0), (0xA, true));
        assert_eq!(rd(&mut bar0, &cfg, 0x610018), (0x0400_0000, true));
        // The boot constants and the boot model's registers are not overridden.
        assert_eq!(rd(&mut bar0, &cfg, NV_PMC_BOOT_42), (0x166A_1000, false));
        assert_eq!(rd(&mut bar0, &cfg, 0x110100), (0x10, false));
        // Not in the table: 0.
        assert_eq!(rd(&mut bar0, &cfg, 0x21438), (0, false));
        // A latched write reads back as written.
        assert_eq!(
            bar0.write(&mut cfg, &mem, 0x21434, &7u32.to_le_bytes()),
            WriteEffect::Latched
        );
        assert_eq!(rd(&mut bar0, &cfg, 0x21434), (7, false));
        // A reset forgets the write; the capture stays.
        bar0.reset().unwrap();
        assert_eq!(rd(&mut bar0, &cfg, 0x21434), (1, true));

        // Fail closed: no table, the same offsets read 0.
        let (mut plain, cfg) = parts();
        assert_eq!(rd(&mut plain, &cfg, 0x21434), (0, false));

        // The house card's capture parses and is the presented TU106's.
        let house = CapturedRegisters::parse(include_str!(
            "../../../caraxes/reference/rtx2070/bar0-registers-2026-09-25.txt"
        ))
        .unwrap();
        assert_eq!(house.len(), 7941);
        assert!(house.check_identity(0x1660_00A1).is_ok());
        assert_eq!(house.get(0x1373A0), Some(0xA));
    }

    /// The card's PROM window (IFR header at 0, the image chain at 0x28600) is served at
    /// its real offsets; the device locates its PCI image through the IFR.
    #[test]
    fn the_ifr_headed_prom_window_is_served_as_on_the_card() {
        let Some(window) = crate::fixture::rtx2070_prom_window() else {
            return;
        };
        let f = crate::fixture::rtx2070();
        let mut bar0 = Bar0::new(f.chip, vec![0x55, 0xAA], Some(window.to_vec()));
        let cfg = f.config();
        let mut b = [0u8; 4];
        bar0.read(&cfg, &GuestMemory::new(), NV_PROM_DATA, &mut b);
        assert_eq!(&b, b"NVGI");
        bar0.read(&cfg, &GuestMemory::new(), NV_PROM_DATA + 0x28600, &mut b);
        assert_eq!(b[..2], [0x55, 0xAA]);
        bar0.read(&cfg, &GuestMemory::new(), NV_PROM_DATA + 0x78000, &mut b);
        assert_eq!(&b, b"NVGI", "the second copy");
        bar0.read(&cfg, &GuestMemory::new(), NV_PROM_DATA + 0xFFFFC, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0xFFFF_FFFF);
        let i = bar0.vbios_image().unwrap();
        assert_eq!((i.offset, i.device, i.bit), (0x28600, 0x1F02, Some(0x1B0)));
        // An image that starts with 55 AA is found at 0, as before.
        let plain = Bar0::new(f.chip, window[0x28600..0x48200].to_vec(), None);
        assert_eq!(plain.vbios_image().unwrap().offset, 0);
    }

    /// NV_PMC_ENABLE stays a latch (read back as written, the capture before), and its
    /// bit 13 rising from the captured 0xd0fdd1c1 resets the PMU falcon.
    #[test]
    fn pmc_enable_latches_and_resets_the_pmu() {
        let house = CapturedRegisters::parse(include_str!(
            "../../../caraxes/reference/rtx2070/bar0-registers-2026-09-25.txt"
        ))
        .unwrap();
        let (bar0, mut cfg) = parts();
        let mem = GuestMemory::new();
        let f = crate::fixture::rtx2070();
        let mut bar0 = bar0
            .with_boot_model(BootModel::new(
                f.chip,
                crate::fb::FbLayout::plain(4 << 30),
                f.identity(4 << 30),
            ))
            .with_captured(house);
        let mut b = [0u8; 4];
        bar0.read(&cfg, &mem, NV_PMC_ENABLE, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0xD0FD_D1C1);
        bar0.write(
            &mut cfg,
            &mem,
            0x10A000 + crate::falcon::DMEMC0,
            &0x100_0010u32.to_le_bytes(),
        );
        assert_eq!(
            bar0.write(&mut cfg, &mem, NV_PMC_ENABLE, &0xD1FD_FFD1u32.to_le_bytes()),
            WriteEffect::Latched
        );
        assert!(bar0.take_notes().iter().any(|n| n.contains("pmu reset")));
        bar0.read(&cfg, &mem, NV_PMC_ENABLE, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0xD1FD_FFD1);
        bar0.read(&cfg, &mem, 0x10A000 + crate::falcon::DMEMC0, &mut b);
        assert_eq!(u32::from_le_bytes(b), 0);
    }

    /// A falcon-block register the model has no value for falls through to the capture
    /// (GSP 0x110ABC reads 0x20 as on the RTX 2070); the falcon offsets the Linux 615 guest
    /// reads at boot (vm4-615 gpu-trace.log) keep the model's answers, and a written
    /// register reads back as written.
    #[test]
    fn unmodelled_falcon_registers_read_the_capture() {
        let house = CapturedRegisters::parse(include_str!(
            "../../../caraxes/reference/rtx2070/bar0-registers-2026-09-25.txt"
        ))
        .unwrap();
        let (bar0, mut cfg) = parts();
        let mem = GuestMemory::new();
        let f = crate::fixture::rtx2070();
        let model = || {
            BootModel::new(
                f.chip,
                crate::fb::FbLayout::plain(4 << 30),
                f.identity(4 << 30),
            )
        };
        let mut bar0 = bar0.with_boot_model(model()).with_captured(house);
        let (plain, _) = parts();
        let mut plain = plain.with_boot_model(model());
        let rd = |bar0: &mut Bar0, cfg: &ConfigSpace, off: u64| {
            let mut b = [0u8; 4];
            bar0.read(cfg, &mem, off, &mut b);
            (u32::from_le_bytes(b), bar0.take_captured())
        };
        assert_eq!(rd(&mut bar0, &cfg, 0x110ABC), (0x20, true));
        assert_eq!(rd(&mut plain, &cfg, 0x110ABC), (0, false));
        // The Linux 615 boot's falcon reads: same values with and without the capture.
        for off in [
            0x110008, 0x110018, 0x11001C, 0x110040, 0x110044, 0x110094, 0x1100F4, 0x110100,
            0x110108, 0x11010C, 0x1103C0, 0x110610, 0x110624, 0x110C04, 0x110C80, 0x111240,
            0x840040, 0x840044, 0x840100, 0x84010C, 0x8403C0, 0x840624, 0x110AC0, 0x110AC4,
        ] {
            let with = rd(&mut bar0, &cfg, off);
            assert_eq!(with, (rd(&mut plain, &cfg, off).0, false), "{off:#x}");
        }
        bar0.write(&mut cfg, &mem, 0x110ABC, &7u32.to_le_bytes());
        assert_eq!(rd(&mut bar0, &cfg, 0x110ABC), (7, false));
    }

    /// The L2 cache-operation registers (`NV_UFLUSH_*`, gm200/dev_flush.h, and the
    /// Turing VF-window sysmem/peer invalidates, tu102/dev_vm.h:29-30) are not latched:
    /// a write of PENDING_BUSY starts the operation, and PENDING (0:0) and OUTSTANDING
    /// (1:1) read back as done, so kmemsysDoCacheOp_GM107's poll ends at once. Latched,
    /// they read back busy forever (a 30 s RM timeout per operation, bench v2-hang).
    #[test]
    fn l2_cache_operations_complete() {
        let (bar0, mut cfg) = parts();
        let mem = GuestMemory::new();
        let f = crate::fixture::rtx2070();
        let mut bar0 = bar0.with_boot_model(BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(4 << 30),
            f.identity(4 << 30),
        ));
        let mut b = [0u8; 4];
        for reg in [
            0x70000u64, 0x70004, 0x70008, 0x7000C, 0x70010, 0xB80F00, 0xB80F04,
        ] {
            bar0.read(&cfg, &mem, reg, &mut b);
            assert_eq!(u32::from_le_bytes(b), 0, "{reg:#x} before any operation");
            // PENDING_BUSY, as kmemsysCacheOp_GM200 writes it.
            assert_eq!(
                bar0.write(&mut cfg, &mem, reg, &1u32.to_le_bytes()),
                WriteEffect::Model,
                "{reg:#x}"
            );
            bar0.read(&cfg, &mem, reg, &mut b);
            assert_eq!(u32::from_le_bytes(b) & 0b11, 0, "{reg:#x} still busy");
            // Bits outside PENDING/OUTSTANDING keep what was written.
            bar0.write(&mut cfg, &mem, reg, &0x7u32.to_le_bytes());
            bar0.read(&cfg, &mem, reg, &mut b);
            assert_eq!(u32::from_le_bytes(b), 0x4, "{reg:#x}");
        }
        assert_eq!(bar0.name(0x70010), Some("NV_UFLUSH_L2_FLUSH_DIRTY"));
        assert_eq!(
            bar0.name(0xB80F00),
            Some("NV_VIRTUAL_FUNCTION_PRIV_L2_SYSMEM_INVALIDATE")
        );
        // Two starts per register (both writes set PENDING); without host execution
        // nothing is forwarded.
        let c = bar0.boot().unwrap().l2_op_counts;
        assert_eq!(c, (14, 0, 0));
        // Neighbours still latch (the default is unchanged).
        assert_eq!(
            bar0.write(&mut cfg, &mem, 0x70014, &1u32.to_le_bytes()),
            WriteEffect::Latched
        );
        bar0.read(&cfg, &mem, 0x70014, &mut b);
        assert_eq!(u32::from_le_bytes(b), 1);
    }
}

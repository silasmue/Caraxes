// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! A falcon register block, as the kernel RM drives it during the GSP boot
//! (docs/caraxes/gsp-boot-turing.md §2): reset through `FALCON_ENGINE`, IMEM and
//! DMEM port writes with auto-increment and tags, `BOOTVEC`, `CPUCTL` start and
//! halt, the mailboxes, `HWCFG`/`HWCFG2`, FBIF. Nothing executes: a started CPU
//! halts at once and the caller (the boot model) produces the effects the
//! firmware would have had. Offsets are relative to the block base (GSP at
//! 0x110000, SEC2 at 0x840000; `dev_falcon_v4.h`, `dev_fbif_v4.h`, `dev_riscv_pri.h`).

use std::collections::BTreeMap;

pub const MAILBOX0: u64 = 0x040;
pub const MAILBOX1: u64 = 0x044;
pub const OS: u64 = 0x080;
pub const RM: u64 = 0x084;
pub const DEBUGINFO: u64 = 0x094;
pub const HWCFG2: u64 = 0x0F4;
pub const CPUCTL: u64 = 0x100;
pub const BOOTVEC: u64 = 0x104;
pub const HWCFG: u64 = 0x108;
pub const DMACTL: u64 = 0x10C;
/// `NV_PFALCON_FALCON_DMATRFCMD` (ampere/ga102/dev_falcon_v4.h:57-71): `_FULL` 0:0 and
/// `_IDLE` 1:1 read-only. The GA102-family HS path (`kgspExecuteHsFalcon_GA102`,
/// kernel_gsp_falcon_ga102.c:184) DMAs the ucode itself and polls FULL false before and
/// IDLE true after each transfer (`s_dmaPoll_GA102`); the model never moves the bytes (the
/// ucode never runs) and reports every transfer done at once.
pub const DMATRFCMD: u64 = 0x118;
const DMATRFCMD_IDLE: u32 = 1 << 1;
const DMATRFCMD_FULL: u32 = 1 << 0;
/// `_IMEM` 4:4: the transfer goes to IMEM (else DMEM).
const DMATRFCMD_IMEM: u32 = 1 << 4;
/// `NV_PFALCON_FALCON_DMATRFBASE` (the source PA >> 8, low 32 bits), `_DMATRFMOFFS` (the
/// falcon-memory offset), `_DMATRFFBOFFS` (the offset added to the base) and `_DMATRFBASE1`
/// (bits 40:32 of the base >> 8) (ampere/ga102/dev_falcon_v4.h:53-78).
pub const DMATRFBASE: u64 = 0x110;
pub const DMATRFMOFFS: u64 = 0x114;
pub const DMATRFFBOFFS: u64 = 0x11C;
pub const DMATRFBASE1: u64 = 0x128;
pub const CPUCTL_ALIAS: u64 = 0x130;
pub const IMEMC0: u64 = 0x180;
pub const IMEMD0: u64 = 0x184;
pub const IMEMT0: u64 = 0x188;
pub const DMEMC0: u64 = 0x1C0;
pub const DMEMD0: u64 = 0x1C4;
pub const ENGINE: u64 = 0x3C0;
pub const FBIF_TRANSCFG0: u64 = 0x600;
pub const FBIF_CTL: u64 = 0x624;
/// `NV_PRISCV_RISCV_CPUCTL` relative to the falcon base (RISC-V block at +0x1000).
pub const RISCV_STATUS: u64 = 0x1240;
/// `NV_PRISCV_RISCV_CPUCTL` (the RISC-V block at +0x1000, ampere/ga102/dev_riscv_pri.h:32-34):
/// `ACTIVE_STAT` 7:7, which the GA102-family HAL reads for "RISC-V running"
/// (`kflcnIsRiscvActive_GA102`, kernel_falcon_ga102.c:47-56) where TU102 reads RISCV_STATUS.
pub const RISCV_CPUCTL: u64 = 0x1388;
const RISCV_CPUCTL_ACTIVE_STAT: u32 = 1 << 7;
/// Size of the block in bytes (falcon, FBIF and the RISC-V block).
pub const BLOCK_SIZE: u64 = 0x2000;

/// Block-relative ranges the model answers even when never written (reads there never
/// fall through to the capture, [`Falcon::answers`]): the falcon core registers 0x000-0x13F
/// (IRQ, mailboxes, OS/RM, DEBUGINFO, HWCFG/HWCFG2, CPUCTL, BOOTVEC, DMA), the IMEM/DMEM
/// ports, `ENGINE`, FBIF 0x600-0x6FF, the EMEM ports 0xAC0-0xADF, the queue registers
/// 0xC00-0xCFF (`NV_PGSP_QUEUE_*`/`MSGQ_*`, answered by the boot model before the falcon)
/// and the whole RISC-V block from 0x1000.
const MODELLED: [std::ops::Range<u64>; 7] = [
    0x000..0x140,
    IMEMC0..0x200,
    ENGINE..ENGINE + 4,
    0x600..0x700,
    0xAC0..0xAE0,
    0xC00..0xD00,
    0x1000..BLOCK_SIZE,
];

const CPUCTL_STARTCPU: u32 = 1 << 1;
const CPUCTL_HALTED: u32 = 1 << 4;
const ENGINE_RESET: u32 = 1;
const PORT_ADDR_MASK: u32 = 0xFFFC;
const PORT_AINCW: u32 = 1 << 24;
const PORT_AINCR: u32 = 1 << 25;
const HWCFG2_RISCV: u32 = 1 << 10;

/// What a register write did that the boot model must react to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// `FALCON_ENGINE` reset asserted; the block is back in its post-reset state.
    Reset,
    /// `CPUCTL` start: the (virtual) ucode ran and halted. `run` counts starts since creation.
    Halted { run: u32 },
}

pub struct Falcon {
    pub name: &'static str,
    riscv_capable: bool,
    imem: Vec<u8>,
    dmem: Vec<u8>,
    imem_tags: Vec<u16>,
    imemc: [u32; 4],
    dmemc: [u32; 4],
    halted: bool,
    riscv_active: bool,
    runs: u32,
    resets: u32,
    regs: BTreeMap<u64, u32>,
    /// The guest PA the first DMEM transfer since the last reset read from, minus its
    /// falcon offset: where the ucode's DMEM image starts in guest memory (the FWSEC data
    /// image of the GA102-family boot, which DMAs instead of loading through the ports).
    dmem_dma_src: Option<u64>,
}

impl Falcon {
    /// `imem_blocks`/`dmem_blocks` in 256-byte blocks, as `HWCFG` reports them.
    /// `halted` is the state before any reset (the GSP falcon is halted after GFW boot).
    pub fn new(
        name: &'static str,
        riscv_capable: bool,
        imem_blocks: u16,
        dmem_blocks: u16,
        halted: bool,
    ) -> Self {
        Falcon {
            name,
            riscv_capable,
            imem: vec![0; imem_blocks as usize * 256],
            dmem: vec![0; dmem_blocks as usize * 256],
            imem_tags: vec![0; imem_blocks as usize],
            imemc: [0; 4],
            dmemc: [0; 4],
            halted,
            riscv_active: false,
            runs: 0,
            resets: 0,
            regs: BTreeMap::new(),
            dmem_dma_src: None,
        }
    }

    /// Where the DMEM image of the last DMA-loaded ucode starts in guest memory.
    pub fn dmem_dma_src(&self) -> Option<u64> {
        self.dmem_dma_src
    }

    pub fn imem(&self) -> &[u8] {
        &self.imem
    }

    pub fn dmem(&self) -> &[u8] {
        &self.dmem
    }

    pub fn imem_tag(&self, block: usize) -> Option<u16> {
        self.imem_tags.get(block).copied()
    }

    pub fn mailbox(&self) -> (u32, u32) {
        (self.reg(MAILBOX0), self.reg(MAILBOX1))
    }

    pub fn set_mailbox(&mut self, m0: u32, m1: u32) {
        self.regs.insert(MAILBOX0, m0);
        self.regs.insert(MAILBOX1, m1);
    }

    pub fn is_halted(&self) -> bool {
        self.halted
    }

    pub fn runs(&self) -> u32 {
        self.runs
    }

    pub fn resets(&self) -> u32 {
        self.resets
    }

    pub fn riscv_active(&self) -> bool {
        self.riscv_active
    }

    pub fn set_riscv_active(&mut self, active: bool) {
        self.riscv_active = active;
    }

    fn reg(&self, off: u64) -> u32 {
        self.regs.get(&off).copied().unwrap_or(0)
    }

    /// Whether the model answers a read of the block-relative offset `off` itself: inside
    /// `MODELLED` (the registers it names and the ones the boot model and the RM path
    /// depend on), or written since the last reset. Elsewhere an unwritten register has no
    /// model value, and the caller may answer it from the physical card's capture
    /// (bar0.rs `CapturedRegisters`), e.g. 0xABC next to `NV_PGSP_EMEMC` (0x20 on the
    /// RTX 2070), which the Windows driver reads.
    pub fn answers(&self, off: u64) -> bool {
        MODELLED.iter().any(|r| r.contains(&off)) || self.regs.contains_key(&off)
    }

    fn hwcfg(&self) -> u32 {
        let imem = (self.imem.len() / 256) as u32 & 0x1FF;
        let dmem = (self.dmem.len() / 256) as u32 & 0x1FF;
        imem | (dmem << 9)
    }

    fn reset(&mut self) {
        self.imem.fill(0);
        self.dmem.fill(0);
        self.imem_tags.fill(0);
        self.imemc = [0; 4];
        self.dmemc = [0; 4];
        self.halted = false;
        self.riscv_active = false;
        self.resets += 1;
        self.dmem_dma_src = None;
        let engine = self.reg(ENGINE);
        self.regs.clear();
        self.regs.insert(ENGINE, engine);
    }

    /// Read a 32-bit register at a block-relative, dword-aligned offset.
    /// `DMEMD` reads with `AINCR` advance the port, hence `&mut self`.
    pub fn read32(&mut self, off: u64) -> u32 {
        match off {
            CPUCTL | CPUCTL_ALIAS => u32::from(self.halted) * CPUCTL_HALTED,
            HWCFG => self.hwcfg(),
            HWCFG2 => u32::from(self.riscv_capable) * HWCFG2_RISCV,
            DMACTL => 0,
            DMATRFCMD => (self.reg(DMATRFCMD) & !DMATRFCMD_FULL) | DMATRFCMD_IDLE,
            DEBUGINFO => 0,
            RISCV_STATUS => u32::from(self.riscv_active),
            RISCV_CPUCTL => u32::from(self.riscv_active) * RISCV_CPUCTL_ACTIVE_STAT,
            o if (IMEMC0..IMEMC0 + 64).contains(&o) && (o - IMEMC0) % 16 == 0 => {
                self.imemc[((o - IMEMC0) / 16) as usize]
            }
            o if (DMEMC0..DMEMC0 + 32).contains(&o) && (o - DMEMC0) % 8 == 0 => {
                self.dmemc[((o - DMEMC0) / 8) as usize]
            }
            o if (DMEMD0..DMEMD0 + 32).contains(&o) && (o - DMEMD0) % 8 == 0 => {
                let port = ((o - DMEMD0) / 8) as usize;
                let addr = (self.dmemc[port] & PORT_ADDR_MASK) as usize;
                let v = self
                    .dmem
                    .get(addr..addr + 4)
                    .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .unwrap_or(0);
                if self.dmemc[port] & PORT_AINCR != 0 {
                    self.dmemc[port] =
                        (self.dmemc[port] & !PORT_ADDR_MASK) | ((addr as u32 + 4) & PORT_ADDR_MASK);
                }
                v
            }
            o => self.reg(o),
        }
    }

    /// Write a 32-bit register; returns the event the boot model must act on.
    pub fn write32(&mut self, off: u64, v: u32) -> Option<Event> {
        match off {
            ENGINE => {
                let was = self.reg(ENGINE);
                self.regs.insert(ENGINE, v);
                if v & ENGINE_RESET != 0 && was & ENGINE_RESET == 0 {
                    self.reset();
                    return Some(Event::Reset);
                }
                None
            }
            CPUCTL | CPUCTL_ALIAS => {
                if v & CPUCTL_STARTCPU != 0 {
                    self.runs += 1;
                    self.halted = true;
                    return Some(Event::Halted { run: self.runs });
                }
                None
            }
            o if (IMEMC0..IMEMC0 + 64).contains(&o) && (o - IMEMC0) % 16 == 0 => {
                self.imemc[((o - IMEMC0) / 16) as usize] = v;
                None
            }
            o if (IMEMD0..IMEMD0 + 64).contains(&o) && (o - IMEMD0) % 16 == 0 => {
                let port = ((o - IMEMD0) / 16) as usize;
                let addr = (self.imemc[port] & PORT_ADDR_MASK) as usize;
                if let Some(b) = self.imem.get_mut(addr..addr + 4) {
                    b.copy_from_slice(&v.to_le_bytes());
                }
                if self.imemc[port] & PORT_AINCW != 0 {
                    self.imemc[port] =
                        (self.imemc[port] & !PORT_ADDR_MASK) | ((addr as u32 + 4) & PORT_ADDR_MASK);
                }
                None
            }
            o if (IMEMT0..IMEMT0 + 64).contains(&o) && (o - IMEMT0) % 16 == 0 => {
                let port = ((o - IMEMT0) / 16) as usize;
                let block = ((self.imemc[port] & PORT_ADDR_MASK) >> 8) as usize;
                if let Some(t) = self.imem_tags.get_mut(block) {
                    *t = v as u16;
                }
                None
            }
            o if (DMEMC0..DMEMC0 + 32).contains(&o) && (o - DMEMC0) % 8 == 0 => {
                self.dmemc[((o - DMEMC0) / 8) as usize] = v;
                None
            }
            o if (DMEMD0..DMEMD0 + 32).contains(&o) && (o - DMEMD0) % 8 == 0 => {
                let port = ((o - DMEMD0) / 8) as usize;
                let addr = (self.dmemc[port] & PORT_ADDR_MASK) as usize;
                if let Some(b) = self.dmem.get_mut(addr..addr + 4) {
                    b.copy_from_slice(&v.to_le_bytes());
                }
                if self.dmemc[port] & PORT_AINCW != 0 {
                    self.dmemc[port] =
                        (self.dmemc[port] & !PORT_ADDR_MASK) | ((addr as u32 + 4) & PORT_ADDR_MASK);
                }
                None
            }
            DMATRFCMD => {
                // s_dmaTransfer_GA102 (kernel_gsp_falcon_ga102.c:117-152): source
                // (BASE1:BASE << 8) + FBOFFS into falcon memory at MOFFS; the first DMEM
                // chunk locates the image. Nothing is copied: the ucode never runs.
                if v & DMATRFCMD_IMEM == 0 && self.dmem_dma_src.is_none() {
                    let base = ((self.reg(DMATRFBASE1) as u64 & 0x1FF) << 32
                        | self.reg(DMATRFBASE) as u64)
                        << 8;
                    let src = base + self.reg(DMATRFFBOFFS) as u64;
                    self.dmem_dma_src = src.checked_sub(self.reg(DMATRFMOFFS) as u64);
                }
                self.regs.insert(DMATRFCMD, v);
                None
            }
            HWCFG | HWCFG2 | DEBUGINFO | RISCV_STATUS | RISCV_CPUCTL => None,
            o => {
                self.regs.insert(o, v);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_load_start_halt() {
        let mut f = Falcon::new("gsp", true, 0x100, 0x100, true);
        assert_eq!(f.read32(CPUCTL), CPUCTL_HALTED);
        assert_eq!(f.read32(HWCFG2) & HWCFG2_RISCV, HWCFG2_RISCV);
        assert_eq!(f.read32(HWCFG), 0x100 | (0x100 << 9));

        let e = f.read32(ENGINE);
        assert_eq!(f.write32(ENGINE, e | ENGINE_RESET), Some(Event::Reset));
        assert_eq!(f.write32(ENGINE, e & !ENGINE_RESET), None);
        assert_eq!(f.read32(DMACTL), 0);
        assert_eq!(f.read32(CPUCTL), 0);
        f.write32(RM, 0x1660_00A1);
        assert_eq!(f.read32(RM), 0x1660_00A1);

        // 84-byte DMEM descriptor at 0 with auto-increment.
        f.write32(DMEMC0, PORT_AINCW);
        for i in 0..21u32 {
            f.write32(DMEMD0, 0x1000 + i);
        }
        assert_eq!(&f.dmem()[0..4], &0x1000u32.to_le_bytes());
        assert_eq!(&f.dmem()[80..84], &0x1014u32.to_le_bytes());
        assert_eq!(f.dmemc[0] & PORT_ADDR_MASK, 84);
        f.write32(DMEMC0, PORT_AINCR | 4);
        assert_eq!(f.read32(DMEMD0), 0x1001);
        assert_eq!(f.read32(DMEMD0), 0x1002);

        // Bootloader at the top two IMEM blocks, tagged 0xFD and 0xFE.
        let blk = 0x100 - 2;
        f.write32(IMEMC0, (blk << 8) | PORT_AINCW);
        f.write32(IMEMT0, 0xFD);
        for i in 0..64u32 {
            f.write32(IMEMD0, i);
        }
        f.write32(IMEMT0, 0xFE);
        for i in 64..128u32 {
            f.write32(IMEMD0, i);
        }
        assert_eq!(f.imem_tag(blk as usize), Some(0xFD));
        assert_eq!(f.imem_tag(blk as usize + 1), Some(0xFE));
        assert_eq!(
            &f.imem()[(blk as usize) * 256 + 4..(blk as usize) * 256 + 8],
            &1u32.to_le_bytes()
        );
        f.write32(BOOTVEC, 0xFD00);
        assert_eq!(f.read32(BOOTVEC), 0xFD00);

        assert_eq!(f.read32(CPUCTL), 0);
        assert_eq!(
            f.write32(CPUCTL, CPUCTL_STARTCPU),
            Some(Event::Halted { run: 1 })
        );
        assert_eq!(f.read32(CPUCTL), CPUCTL_HALTED);
        f.set_mailbox(0x1234_5000, 0x1);
        assert_eq!(f.mailbox(), (0x1234_5000, 0x1));
        assert_eq!(f.read32(RISCV_STATUS), 0);
        f.set_riscv_active(true);
        assert_eq!(f.read32(RISCV_STATUS), 1);
        assert_eq!(f.write32(ENGINE, ENGINE_RESET), Some(Event::Reset));
        assert_eq!(f.resets(), 2);
        assert_eq!(f.read32(RISCV_STATUS), 0);
        assert_eq!(f.read32(RM), 0);
    }
}

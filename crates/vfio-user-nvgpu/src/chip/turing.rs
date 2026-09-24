// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The Turing firmware-boot model (experiment E2): everything the CPU expects to see
//! from GFW, FWSEC, Booter and the GSP RISC-V core, without any firmware running
//! (docs/caraxes/gsp-boot-turing.md, init-map.md phase 4, experiments.md E2). The
//! same `_TU102` HALs boot GA10x and AD10x (docs/caraxes/other-architectures.md §1),
//! so a chip of those families reuses this with its own `boot.toml`; Hopper and
//! Blackwell boot through GSP-FMC and need a model of their own.
//!
//! The two falcon blocks delegate to [`crate::falcon::Falcon`]. After the GSP falcon
//! "runs" the FWSEC command the model reads the FRTS region offset the CPU wrote into
//! the FWSEC data buffer in guest RAM (through the bootloader descriptor it wrote
//! into DMEM) and reports it back in the WPR2 registers, which is what the CPU
//! verifies. Booter-Load hands over the `GspFwWprMeta` address and a GSP queue
//! doorbell the libos arguments: both go back to the generic boot model as
//! [`BootEvent`]s.

use serde::Deserialize;

use super::{BootEvent, BootWrite, FirmwareBoot};
use crate::falcon::{Event, Falcon};
use crate::guestmem::GuestMemory;

/// One falcon block of `boot.toml`.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FalconSpec {
    pub base: u64,
    pub imem_blocks: u16,
    pub dmem_blocks: u16,
    pub riscv: bool,
    /// Halted before any reset (the GSP falcon is halted after GFW boot).
    pub halted: bool,
}

/// A register that reads a fixed post-GFW value.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstReg {
    pub name: String,
    pub offset: u64,
    pub value: u32,
}

/// `chips/<chip>/boot.toml` for a Turing-style boot.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TuringBootData {
    pub gsp: FalconSpec,
    pub sec2: FalconSpec,
    pub falcon_block_size: u64,
    pub gsp_queue_head: u64,
    /// `NV_PGSP_QUEUE_TAIL(0)`, `NV_PGSP_MSGQ_HEAD(0)`, `NV_PGSP_MSGQ_TAIL(0)`: the
    /// pointer registers of message queue v2 (driver 615 on); absent in a chip file
    /// written for 610 only.
    #[serde(default)]
    pub gsp_queue_tail: Option<u64>,
    #[serde(default)]
    pub gsp_msgq_head: Option<u64>,
    #[serde(default)]
    pub gsp_msgq_tail: Option<u64>,
    pub local_memory_range: u64,
    pub wpr2_addr_lo: u64,
    pub wpr2_addr_hi: u64,
    pub frts_size: u64,
    pub bl_desc_data_dma_base: u64,
    pub fwsec_data_frts_offset_4k: u64,
    pub vga_workspace_size: u64,
    pub wpr_end_align: u64,
    #[serde(rename = "const")]
    pub consts: Vec<ConstReg>,
}

impl TuringBootData {
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("boot.toml: {e}"))
    }
}

/// Encode a framebuffer size for `NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE`:
/// `fbSize = LOWER_MAG[9:4] << (LOWER_SCALE[3:0] + 20)` (`kern_mem_sys_gp102.c:47-50`).
/// Whole GiB counts use scale 10 like the card (8 GiB reads 0x8A); other sizes take
/// the largest representable value not above `fb_bytes` with the largest scale.
/// Returns the register value and the size it represents.
pub fn local_memory_range(fb_bytes: u64) -> Option<(u32, u64)> {
    if fb_bytes == 0 {
        return None;
    }
    if fb_bytes % (1 << 30) == 0 && fb_bytes >> 30 < 64 {
        return Some(((((fb_bytes >> 30) as u32) << 4) | 10, fb_bytes));
    }
    // Ascending scales; max_by_key keeps the last maximum, i.e. the largest scale.
    (0u32..16)
        .filter_map(|scale| {
            let mag = (fb_bytes >> (scale + 20)).min(63);
            (mag > 0).then(|| ((mag as u32) << 4 | scale, mag << (scale + 20)))
        })
        .max_by_key(|&(_, size)| size)
}

/// The Turing boot model of one card, in its post-GFW state until written.
pub struct TuringBoot {
    d: &'static TuringBootData,
    pub gsp: Falcon,
    pub sec2: Falcon,
    fb_bytes: u64,
    wpr2: (u32, u32),
    frts_offset: Option<u64>,
    /// GSP-RM was loaded by a Booter Load and not unloaded since: what tells Booter
    /// Unload from Booter Load when the mailboxes do not (B4).
    loaded: bool,
}

impl TuringBoot {
    pub fn new(d: &'static TuringBootData, fb_bytes: u64) -> Self {
        let falcon = |name, f: &FalconSpec| {
            Falcon::new(name, f.riscv, f.imem_blocks, f.dmem_blocks, f.halted)
        };
        TuringBoot {
            gsp: falcon("gsp", &d.gsp),
            sec2: falcon("sec2", &d.sec2),
            d,
            fb_bytes,
            wpr2: (0, 0),
            frts_offset: None,
            loaded: false,
        }
    }

    fn in_block(&self, f: &FalconSpec, off: u64) -> bool {
        (f.base..f.base + self.d.falcon_block_size).contains(&off)
    }

    fn constant(&self, off: u64) -> Option<u32> {
        self.d
            .consts
            .iter()
            .find(|c| c.offset == off)
            .map(|c| c.value)
    }

    /// The FWSEC-FRTS side effects: WPR2 window as the CPU asked for it.
    fn fwsec_done(&mut self, mem: &GuestMemory) -> Vec<String> {
        let dmem = self.gsp.dmem();
        let at = self.d.bl_desc_data_dma_base as usize;
        let lo = u32::from_le_bytes(dmem[at..][..4].try_into().unwrap());
        let hi = u32::from_le_bytes(dmem[at + 4..][..4].try_into().unwrap());
        let data_pa = ((hi as u64) << 32) | lo as u64;
        let (frts_offset, how) = match mem.read_u32(data_pa + self.d.fwsec_data_frts_offset_4k) {
            Some(v) if v != 0 => ((v as u64) << 12, "from the FWSEC command in guest RAM"),
            _ => (self.fallback_frts_offset(), "fallback from the FB size"),
        };
        self.frts_offset = Some(frts_offset);
        self.wpr2 = (
            ((frts_offset >> 12) << 4) as u32,
            (((frts_offset + self.d.frts_size - 1) >> 12) << 4) as u32,
        );
        vec![format!(
            "fwsec: data buffer PA {data_pa:#x}, frtsOffset {frts_offset:#x} ({how}), WPR2 lo {:#x} hi {:#x}",
            self.wpr2.0, self.wpr2.1
        )]
    }

    /// `kgspCalculateFbLayout_TU102` with no display (see `boot.toml`).
    fn fallback_frts_offset(&self) -> u64 {
        let vga_workspace = self.fb_bytes - self.d.vga_workspace_size;
        let wpr_end = vga_workspace & !(self.d.wpr_end_align - 1);
        wpr_end - self.d.frts_size
    }
}

impl FirmwareBoot for TuringBoot {
    fn libos_args_pa(&self) -> Option<u64> {
        let (m0, m1) = self.gsp.mailbox();
        Some(((m1 as u64) << 32) | m0 as u64).filter(|&pa| pa != 0)
    }

    fn queue_registers(&self) -> Option<super::QueueRegisters> {
        Some(super::QueueRegisters {
            cmd_head: self.d.gsp_queue_head,
            cmd_tail: self.d.gsp_queue_tail?,
            msg_head: self.d.gsp_msgq_head?,
            msg_tail: self.d.gsp_msgq_tail?,
        })
    }

    fn handles(&self, off: u64) -> bool {
        off == self.d.local_memory_range
            || off == self.d.wpr2_addr_lo
            || off == self.d.wpr2_addr_hi
            || self.constant(off).is_some()
            || self.in_block(&self.d.gsp, off)
            || self.in_block(&self.d.sec2, off)
    }

    fn read32(&mut self, off: u64) -> Option<u32> {
        if !self.handles(off) {
            return None;
        }
        Some(match off {
            o if o == self.d.local_memory_range => {
                local_memory_range(self.fb_bytes).map_or(0, |(v, _)| v)
            }
            o if o == self.d.wpr2_addr_lo => self.wpr2.0,
            o if o == self.d.wpr2_addr_hi => self.wpr2.1,
            o if self.in_block(&self.d.gsp, o) => self.gsp.read32(o - self.d.gsp.base),
            o if self.in_block(&self.d.sec2, o) => self.sec2.read32(o - self.d.sec2.base),
            o => self.constant(o).unwrap_or(0),
        })
    }

    fn write32(&mut self, off: u64, v: u32, mem: &GuestMemory) -> Option<BootWrite> {
        if !self.handles(off) {
            return None;
        }
        let mut w = BootWrite::default();
        if self.in_block(&self.d.gsp, off) {
            if off == self.d.gsp_queue_head {
                // The CPU put the libos init arguments' PA into MAILBOX0/1 first.
                let (m0, m1) = self.gsp.mailbox();
                w.event = Some(BootEvent::CmdQueueDoorbell {
                    libos_args_pa: ((m1 as u64) << 32) | m0 as u64,
                });
            }
            match self.gsp.write32(off - self.d.gsp.base, v) {
                Some(Event::Reset) => w.notes.push(format!("gsp reset {}", self.gsp.resets())),
                Some(Event::Halted { run }) => {
                    w.notes.push(format!("gsp start {run}: halted"));
                    if run == 1 {
                        w.notes.extend(self.fwsec_done(mem));
                    }
                }
                None => {}
            }
        } else if self.in_block(&self.d.sec2, off) {
            match self.sec2.write32(off - self.d.sec2.base, v) {
                Some(Event::Reset) => w.notes.push(format!("sec2 reset {}", self.sec2.resets())),
                // Booter Load or Unload by its own signal, not by the SEC2 run count
                // (docs/caraxes/gsp-plan/boot-exchange.md §4): the Load argument is the
                // GspFwWprMeta (its magic, gsp_fw_wpr_meta.h:214), the Unload argument 0
                // or a GspFwSRMeta, which a resume's Load passes too
                // (kernel_gsp_tu102.c:346-366, 491-515); without the WPR meta magic the
                // state decides: an Unload needs a loaded GSP-RM. A reload without a
                // function-level reset (Load, Unload, Load) is then read right.
                Some(Event::Halted { run })
                    if self.loaded && {
                        let (m0, m1) = self.sec2.mailbox();
                        let pa = ((m1 as u64) << 32) | m0 as u64;
                        mem.read_u64(pa) != Some(crate::ver::active::WPR_META_MAGIC)
                    } =>
                {
                    // Booter Unload: WPR2 is torn down and the RISC-V core stops
                    // (`kgspExecuteBooterUnloadIfNeeded_TU102` checks WPR2 afterwards).
                    let (m0, m1) = self.sec2.mailbox();
                    let arg = ((m1 as u64) << 32) | m0 as u64;
                    self.sec2.set_mailbox(0, m1);
                    self.wpr2 = (0, 0);
                    self.gsp.set_riscv_active(false);
                    self.loaded = false;
                    w.notes.push(format!(
                        "sec2 start {run}: booter unload done (argument {arg:#x}), WPR2 down, gsp riscv stopped"
                    ));
                    w.event = Some(BootEvent::BooterUnload { arg });
                }
                Some(Event::Halted { run }) => {
                    // Booter-Load: MAILBOX0/1 carried the GspFwWprMeta PA; a zero in
                    // MAILBOX0 afterwards means success (`kernel_gsp_booter_tu102.c:72-76`),
                    // and the GSP RISC-V core is running GSP-RM from then on.
                    let (m0, m1) = self.sec2.mailbox();
                    let pa = ((m1 as u64) << 32) | m0 as u64;
                    self.sec2.set_mailbox(0, m1);
                    self.gsp.set_riscv_active(true);
                    self.loaded = true;
                    w.notes.push(format!(
                        "sec2 start {run}: booter done, wpr meta PA {pa:#x} ({}), gsp riscv active",
                        if mem.covers(pa, 256) {
                            "mapped"
                        } else {
                            "NOT MAPPED"
                        }
                    ));
                    w.event = Some(BootEvent::WprMeta { pa });
                }
                None => {}
            }
        } else {
            w.notes.push("read-only".into());
        }
        Some(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::falcon;

    fn data() -> &'static TuringBootData {
        super::super::tu106::boot_data()
    }

    #[test]
    fn fb_size_encoding() {
        assert_eq!(local_memory_range(4 << 30), Some((0x4A, 4 << 30)));
        assert_eq!(local_memory_range(8 << 30), Some((0x8A, 8 << 30)));
        assert_eq!(local_memory_range(6 << 30), Some((0x6A, 6 << 30)));
        assert_eq!(local_memory_range(512 << 20), Some((0x19, 512 << 20)));
        assert_eq!(local_memory_range(130 << 20), Some((0x17, 128 << 20)));
        assert_eq!(local_memory_range(0), None);
    }

    #[test]
    fn boot_sequence_side_effects() {
        let d = data();
        let (gsp, sec2) = (d.gsp.base, d.sec2.base);
        let mem = GuestMemory::new();
        let mut b = TuringBoot::new(d, 4 << 30);
        assert_eq!(b.read32(gsp + falcon::CPUCTL), Some(0x10));
        assert_eq!(b.read32(0x0011_8128), Some(1), "GFW scratch PLM");
        assert_eq!(b.read32(0x0011_8234), Some(0xFF), "GFW_BOOT");
        assert_eq!(b.read32(d.local_memory_range), Some(0x4A));
        assert_eq!(b.read32(d.wpr2_addr_hi), Some(0));
        assert_eq!(b.read32(0x1234), None);
        assert_eq!(
            b.write32(0x0002_174C, 0, &mem).unwrap().notes,
            ["read-only"]
        );

        // FWSEC on the GSP falcon: reset, descriptor into DMEM, start.
        b.write32(gsp + falcon::ENGINE, 1, &mem);
        b.write32(gsp + falcon::ENGINE, 0, &mem);
        b.write32(gsp + falcon::DMEMC0, 1 << 24, &mem);
        for i in 0..21u32 {
            let v = match i {
                16 => 0x0800_0000,
                17 => 0x1,
                _ => 0,
            };
            b.write32(gsp + falcon::DMEMD0, v, &mem);
        }
        let w = b.write32(gsp + falcon::CPUCTL, 2, &mem).unwrap();
        assert!(w.notes.iter().any(|n| n.contains("fallback")), "{w:?}");
        // 4 GiB - 1 MiB VGA workspace, aligned, minus 1 MiB FRTS = 0xFFE00000.
        assert_eq!(b.read32(d.wpr2_addr_lo), Some(0x00FF_E000));
        assert_ne!(b.read32(d.wpr2_addr_hi), Some(0));

        // Booter on SEC2 with the WPR meta PA in the mailboxes.
        b.write32(sec2 + falcon::MAILBOX0, 0x7654_3000, &mem);
        b.write32(sec2 + falcon::MAILBOX1, 0x1, &mem);
        let w = b.write32(sec2 + falcon::CPUCTL, 2, &mem).unwrap();
        assert!(w.notes[0].contains("wpr meta PA 0x176543000"), "{w:?}");
        assert_eq!(w.event, Some(BootEvent::WprMeta { pa: 0x1_7654_3000 }));
        assert_eq!(b.read32(sec2 + falcon::MAILBOX0), Some(0));
        assert_eq!(b.read32(gsp + falcon::RISCV_STATUS), Some(1));

        // Libos args through the GSP mailboxes, then the doorbell.
        b.write32(gsp + falcon::MAILBOX0, 0x8000_1000, &mem);
        b.write32(gsp + falcon::MAILBOX1, 0, &mem);
        let w = b.write32(d.gsp_queue_head, 0, &mem).unwrap();
        assert!(w.notes.is_empty());
        assert_eq!(
            w.event,
            Some(BootEvent::CmdQueueDoorbell {
                libos_args_pa: 0x8000_1000
            })
        );

        // Booter Unload: WPR2 down.
        let w = b.write32(sec2 + falcon::CPUCTL, 2, &mem).unwrap();
        assert!(w.notes[0].contains("booter unload done"), "{w:?}");
        assert_eq!(w.event, Some(BootEvent::BooterUnload { arg: 0x1_0000_0000 }));
        assert_eq!(b.read32(d.wpr2_addr_lo), Some(0));
        assert_eq!(b.read32(gsp + falcon::RISCV_STATUS), Some(0));

        // B4: a reload without a function-level reset. The teardown resets the GSP
        // falcon first (kgspTeardown_TU102), the next boot runs FWSEC-FRTS again and
        // then Booter Load: read as a Load, not as a third SEC2 run.
        b.write32(gsp + falcon::ENGINE, 1, &mem);
        b.write32(gsp + falcon::ENGINE, 0, &mem);
        b.write32(sec2 + falcon::MAILBOX0, 0x7654_3000, &mem);
        b.write32(sec2 + falcon::MAILBOX1, 0x1, &mem);
        let w = b.write32(sec2 + falcon::CPUCTL, 2, &mem).unwrap();
        assert!(w.notes[0].contains("sec2 start 3: booter done"), "{w:?}");
        assert_eq!(w.event, Some(BootEvent::WprMeta { pa: 0x1_7654_3000 }));
        assert_eq!(b.read32(gsp + falcon::RISCV_STATUS), Some(1));
        // And its Unload again after a falcon reset of the teardown: GSP-RM is loaded.
        b.write32(gsp + falcon::ENGINE, 1, &mem);
        b.write32(gsp + falcon::ENGINE, 0, &mem);
        let w = b.write32(sec2 + falcon::CPUCTL, 2, &mem).unwrap();
        assert!(w.notes[0].contains("sec2 start 4: booter unload done"), "{w:?}");
    }
}

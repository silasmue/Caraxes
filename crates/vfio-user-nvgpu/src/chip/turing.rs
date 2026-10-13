// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
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
    /// The host card's `HWCFG` (`None`: computed from the block counts), `HWCFG1` and
    /// `ITFEN` (read-only capture of 2026-09-25; `falcon::Ids`).
    #[serde(default)]
    pub hwcfg: Option<u32>,
    #[serde(default)]
    pub hwcfg1: u32,
    #[serde(default)]
    pub itfen: u32,
    /// FBIF offset in the block (default 0x600; the PMU's is 0xE00).
    #[serde(default)]
    pub fbif: Option<u64>,
    /// Block size when not `falcon_block_size` (the PMU has no RISC-V block: 0x1000).
    #[serde(default)]
    pub block_size: Option<u64>,
    /// A start raises `IRQSTAT` HALT (the PMU; `Falcon::with_halt_irq`).
    #[serde(default)]
    pub halt_irq: bool,
    /// The `NV_PMC_ENABLE` bit whose rising edge resets the falcon (the PMU: 13, nouveau
    /// `gk104_mc_reset`).
    #[serde(default)]
    pub pmc_enable_bit: Option<u8>,
}

impl FalconSpec {
    /// The falcon model of this block.
    pub fn falcon(&self, name: &'static str, stallack: u32) -> Falcon {
        Falcon::new(
            name,
            self.riscv,
            self.imem_blocks,
            self.dmem_blocks,
            self.halted,
        )
        .with_ids(crate::falcon::Ids {
            hwcfg: self.hwcfg,
            hwcfg1: self.hwcfg1,
            itfen: self.itfen,
        })
        .with_fbif(self.fbif.unwrap_or(crate::falcon::FBIF_DEFAULT))
        .with_halt_irq(self.halt_irq)
        .with_stallack(stallack)
    }
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
    /// The PMU falcon, which the CPU-RM path of the Windows driver drives (the ACR unload;
    /// caraxes/runs/win11-2026-09-25/night-2026-09-25/ANALYSIS-gsp-path.md); the GSP-RM
    /// path never touches it.
    #[serde(default)]
    pub pmu: Option<FalconSpec>,
    /// `NV_PMC_ENABLE`, watched (not owned) for the falcon resets of `pmc_enable_bit`.
    #[serde(default)]
    pub pmc_enable: Option<u64>,
    /// What a falcon's `ENGCTL` reads after a stall request (default 0x200; the
    /// `CARAXES_FALCON_STALLACK` environment variable wins).
    #[serde(default)]
    pub stallack: Option<u32>,
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
    /// `NV_PFB_PRI_MMU_WPR1_ADDR_LO/HI` (0x1fa81c/0x1fa820): WPR1 as ACR sets it; nothing
    /// in the model raises it yet, the ACR unload clears it.
    #[serde(default)]
    pub wpr1_addr_lo: Option<u64>,
    #[serde(default)]
    pub wpr1_addr_hi: Option<u64>,
    /// The PMU `BOOTVEC` of the ACR unload bootloader (`unload_bl.bin`, IMEM tag 0x100):
    /// its halt tears WPR1 and WPR2 down.
    #[serde(default)]
    pub acr_unload_bootvec: Option<u32>,
    pub frts_size: u64,
    pub bl_desc_data_dma_base: u64,
    pub fwsec_data_frts_offset_4k: u64,
    pub vga_workspace_size: u64,
    pub wpr_end_align: u64,
    /// `NV_USABLE_FB_SIZE_IN_MB` (ampere/ga102/dev_gc6_island.h), when the chip's memsys HAL
    /// reads the FB size there (`kmemsysReadUsableFbSize_GA102`, kern_mem_sys_ga102.c:37-52)
    /// instead of `NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE`: the GA10x and AD10x costumes.
    #[serde(default)]
    pub usable_fb_size_mb: Option<u64>,
    /// `NV_PGSP_MAILBOX(GSPRM_HEARTBEAT)` and `(LIBOS_HEARTBEAT)` (0x110804 + 4i,
    /// ampere/ga102/dev_gsp.h:35), when the chip's GSP-RM (libos 3, GA102 on) keeps
    /// heartbeats the kernel checks (`_kgspIsHeartbeatTimedOut`, kernel_gsp.c:2264-2303): the
    /// device's PTIMER time in ms, as the FSP model answers them.
    #[serde(default)]
    pub heartbeat: Option<[u64; 2]>,
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

/// `FALCON_APPLICATION_INTERFACE_DMEM_MAPPER_V3` signature "DMAP" and version.
const DMAP_SIGNATURE: u32 = 0x5041_4D44;
const DMAP_VERSION: u32 = 3;
/// `init_cmd` and `cmd_in_buffer_offset` in the DMEMMAPPER.
const DMAP_INIT_CMD: u64 = 0x2C;
const DMAP_CMD_IN: u64 = 0x08;
/// `FWSECLIC_FRTS_CMD.frtsRegion.addr`: after the 24-byte READ_VBIOS part, ver and hdr.
const FRTS_REGION_ADDR: u64 = 0x20;
pub const FWSEC_CMD_FRTS: u32 = 0x15;
pub const FWSEC_CMD_SB: u32 = 0x19;
/// How far the mapper is searched when the data size is unknown (the GA102-family DMA
/// boot) or implausible.
const FWSEC_DATA_SCAN_DEFAULT: u64 = 0x1000;
const FWSEC_DATA_SCAN_MAX: u64 = 0x10000;

/// The FWSEC command found in a data image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FwsecCmd {
    /// Offset of the DMEMMAPPER in the data image.
    pub at: u64,
    pub cmd: u32,
    /// Offset of the command input buffer in the data image.
    pub cmd_in: u32,
}

/// The DMEMMAPPER v3 in the FWSEC data image at `data_pa` (`data_size` bytes, the loader
/// descriptor's `dataSize`): the first dword-aligned "DMAP" with version 3. `None` when
/// guest RAM does not hold it.
pub fn fwsec_command(mem: &GuestMemory, data_pa: u64, data_size: u64) -> Option<FwsecCmd> {
    let limit = match data_size {
        0 => FWSEC_DATA_SCAN_DEFAULT,
        n => n.min(FWSEC_DATA_SCAN_MAX),
    };
    (0..limit.saturating_sub(DMAP_INIT_CMD + 4))
        .step_by(4)
        .find_map(|at| {
            let hdr = (mem.read_u32(data_pa + at)?, mem.read_u32(data_pa + at + 4)?);
            (hdr.0 == DMAP_SIGNATURE && hdr.1 & 0xFFFF == DMAP_VERSION).then_some(())?;
            Some(FwsecCmd {
                at,
                cmd: mem.read_u32(data_pa + at + DMAP_INIT_CMD)?,
                cmd_in: mem.read_u32(data_pa + at + DMAP_CMD_IN)?,
            })
        })
}

fn fwsec_command_name(cmd: u32) -> &'static str {
    match cmd {
        FWSEC_CMD_FRTS => "FRTS",
        FWSEC_CMD_SB => "SB",
        _ => "unknown",
    }
}

/// The Turing boot model of one card, in its post-GFW state until written.
pub struct TuringBoot {
    d: &'static TuringBootData,
    pub gsp: Falcon,
    pub sec2: Falcon,
    pub pmu: Option<Falcon>,
    fb_bytes: u64,
    wpr2: (u32, u32),
    wpr1: (u32, u32),
    frts_offset: Option<u64>,
    /// GSP-RM was loaded by a Booter Load and not unloaded since: what tells Booter
    /// Unload from Booter Load when the mailboxes do not (B4).
    loaded: bool,
    /// The time base of the heartbeats: created with the model, as the device's PTIMER
    /// (channel.rs `Registry::now_ns`, created by the same `BootModel::new`).
    clock: std::time::Instant,
}

impl TuringBoot {
    pub fn new(d: &'static TuringBootData, fb_bytes: u64) -> Self {
        let stallack = d.stallack.unwrap_or(crate::falcon::STALLACK_DEFAULT);
        TuringBoot {
            gsp: d.gsp.falcon("gsp", stallack),
            sec2: d.sec2.falcon("sec2", stallack),
            pmu: d.pmu.as_ref().map(|p| p.falcon("pmu", stallack)),
            d,
            fb_bytes,
            wpr2: (0, 0),
            wpr1: (0, 0),
            frts_offset: None,
            loaded: false,
            clock: std::time::Instant::now(),
        }
    }

    fn in_block(&self, f: &FalconSpec, off: u64) -> bool {
        (f.base..f.base + f.block_size.unwrap_or(self.d.falcon_block_size)).contains(&off)
    }

    fn in_pmu(&self, off: u64) -> bool {
        self.d.pmu.as_ref().is_some_and(|p| self.in_block(p, off))
    }

    fn constant(&self, off: u64) -> Option<u32> {
        self.d
            .consts
            .iter()
            .find(|c| c.offset == off)
            .map(|c| c.value)
    }

    /// The FWSEC side effects. The command is decoded from the FWSEC data image in guest
    /// RAM: the DMEMMAPPER (v3, "DMAP") holds `init_cmd` at +0x2C and the command input
    /// buffer's offset at +0x08 (nova-core firmware/fwsec.rs `FalconAppifDmemmapperV3`; on the
    /// RTX 2070's VBIOS the app interface table at data + 0xE0 names it at + 0x390, on the
    /// synthetic image at + 0x20). Only FRTS (0x15) creates WPR2, from the FRTS region's
    /// `addr` (4 KiB units) at command input + 0x20; SB (0x19) and the rest leave it. Without
    /// a readable mapper the start counts as FRTS, as before.
    fn fwsec_done(&mut self, mem: &GuestMemory) -> Vec<String> {
        // Boot from HS (GA102 family): the DMEM image was DMAed from guest memory; boot with
        // loader (TU10x): the loader descriptor in DMEM names the data buffer and its size.
        let (data_pa, data_size) = match self.gsp.dmem_dma_src() {
            Some(pa) => (pa, FWSEC_DATA_SCAN_DEFAULT),
            None => {
                let dmem = self.gsp.dmem();
                let rd = |at: usize| u32::from_le_bytes(dmem[at..][..4].try_into().unwrap());
                let at = self.d.bl_desc_data_dma_base as usize;
                let pa = ((rd(at + 4) as u64) << 32) | rd(at) as u64;
                // `dataSize` follows `dataDmaBase` (rmflcnbl.h:89-90).
                (pa, rd(at + 8) as u64)
            }
        };
        let decoded = fwsec_command(mem, data_pa, data_size);
        let frts_at = match decoded {
            Some(FwsecCmd { at, cmd, cmd_in }) if cmd != FWSEC_CMD_FRTS => {
                return vec![format!(
                    "fwsec: data buffer PA {data_pa:#x}, DMEMMAPPER at +{at:#x}, command {cmd:#x} ({}), command input +{cmd_in:#x}: WPR2 unchanged (lo {:#x} hi {:#x})",
                    fwsec_command_name(cmd),
                    self.wpr2.0,
                    self.wpr2.1
                )];
            }
            Some(FwsecCmd { cmd_in, .. }) => cmd_in as u64 + FRTS_REGION_ADDR,
            None => self.d.fwsec_data_frts_offset_4k,
        };
        let what = match decoded {
            Some(FwsecCmd { at, .. }) => format!("command 0x15 (FRTS), DMEMMAPPER at +{at:#x}"),
            None => "no DMEMMAPPER readable, taken as FRTS".to_string(),
        };
        let (frts_offset, how) = match mem.read_u32(data_pa + frts_at) {
            Some(v) if v != 0 => ((v as u64) << 12, "from the FWSEC command in guest RAM"),
            _ => (self.fallback_frts_offset(), "fallback from the FB size"),
        };
        self.frts_offset = Some(frts_offset);
        self.wpr2 = (
            ((frts_offset >> 12) << 4) as u32,
            (((frts_offset + self.d.frts_size - 1) >> 12) << 4) as u32,
        );
        vec![format!(
            "fwsec: data buffer PA {data_pa:#x}, {what}, frtsOffset {frts_offset:#x} ({how}), WPR2 lo {:#x} hi {:#x}",
            self.wpr2.0, self.wpr2.1
        )]
    }

    /// `kgspCalculateFbLayout_TU102` with no display (see `boot.toml`).
    fn fallback_frts_offset(&self) -> u64 {
        let vga_workspace = self.fb_bytes - self.d.vga_workspace_size;
        let wpr_end = vga_workspace & !(self.d.wpr_end_align - 1);
        wpr_end - self.d.frts_size
    }

    /// A write into the PMU block. A start halts at once (the ucode never runs); the
    /// status the CPU reads afterwards is success: `MAILBOX0` 0 (nouveau
    /// `gm200_flcn_fw_boot`).
    fn pmu_write(&mut self, off: u64, v: u32) -> Vec<String> {
        let (Some(spec), Some(pmu)) = (self.d.pmu.as_ref(), self.pmu.as_mut()) else {
            return Vec::new();
        };
        match pmu.write32(off - spec.base, v) {
            Some(Event::Reset) => vec![format!("pmu reset {}", pmu.resets())],
            Some(Event::Halted { run }) => {
                let bootvec = pmu.read32(crate::falcon::BOOTVEC);
                let (_, m1) = pmu.mailbox();
                pmu.set_mailbox(0, m1);
                if Some(bootvec) == self.d.acr_unload_bootvec {
                    // The ACR unload (nouveau `tu102_acr_unload_fwif` on the PMU): the WPR
                    // regions are torn down.
                    let before = (self.wpr1, self.wpr2);
                    self.wpr1 = (0, 0);
                    self.wpr2 = (0, 0);
                    return vec![format!(
                        "pmu start {run}: acr unload (BOOTVEC {bootvec:#x}) done, MAILBOX0 0; WPR1 lo/hi {:#x}/{:#x} and WPR2 lo/hi {:#x}/{:#x} -> 0",
                        before.0 .0, before.0 .1, before.1 .0, before.1 .1
                    )];
                }
                vec![format!(
                    "pmu start {run}: halted (BOOTVEC {bootvec:#x}), MAILBOX0 0"
                )]
            }
            None => Vec::new(),
        }
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
            || Some(off) == self.d.usable_fb_size_mb
            || self.d.heartbeat.is_some_and(|h| h.contains(&off))
            || off == self.d.wpr2_addr_lo
            || off == self.d.wpr2_addr_hi
            || Some(off) == self.d.wpr1_addr_lo
            || Some(off) == self.d.wpr1_addr_hi
            || self.constant(off).is_some()
            || self.in_block(&self.d.gsp, off)
            || self.in_block(&self.d.sec2, off)
            || self.in_pmu(off)
    }

    fn observe_write(&mut self, off: u64, old: u32, new: u32) -> Vec<String> {
        let (Some(pmc), Some(spec), Some(pmu)) =
            (self.d.pmc_enable, self.d.pmu.as_ref(), self.pmu.as_mut())
        else {
            return Vec::new();
        };
        match spec.pmc_enable_bit {
            Some(bit) if off == pmc && (new & !old) >> bit & 1 == 1 => {
                pmu.reset_block();
                vec![format!(
                    "pmu reset {} (NV_PMC_ENABLE bit {bit} rose: {old:#x} -> {new:#x})",
                    pmu.resets()
                )]
            }
            _ => Vec::new(),
        }
    }

    fn read32(&mut self, off: u64) -> Option<u32> {
        if !self.handles(off) {
            return None;
        }
        // A falcon register the model has no value for (outside `falcon::MODELLED`, never
        // written): not owned for this read, so the captured table answers it (bar0.rs).
        let (gsp, sec2) = (self.d.gsp.base, self.d.sec2.base);
        if (self.in_block(&self.d.gsp, off) && !self.gsp.answers(off - gsp))
            || (self.in_block(&self.d.sec2, off) && !self.sec2.answers(off - sec2))
            || (self.in_pmu(off)
                && self
                    .d
                    .pmu
                    .as_ref()
                    .zip(self.pmu.as_ref())
                    .is_some_and(|(s, p)| !p.answers(off - s.base)))
        {
            return self.constant(off);
        }
        Some(match off {
            o if o == self.d.local_memory_range => {
                local_memory_range(self.fb_bytes).map_or(0, |(v, _)| v)
            }
            o if Some(o) == self.d.usable_fb_size_mb => (self.fb_bytes >> 20) as u32,
            o if self.d.heartbeat.is_some_and(|h| h.contains(&o)) => {
                (self.clock.elapsed().as_millis() as u64) as u32
            }
            o if o == self.d.wpr2_addr_lo => self.wpr2.0,
            o if o == self.d.wpr2_addr_hi => self.wpr2.1,
            o if Some(o) == self.d.wpr1_addr_lo => self.wpr1.0,
            o if Some(o) == self.d.wpr1_addr_hi => self.wpr1.1,
            o if self.in_block(&self.d.gsp, o) => self.gsp.read32(o - self.d.gsp.base),
            o if self.in_block(&self.d.sec2, o) => self.sec2.read32(o - self.d.sec2.base),
            o if self.in_pmu(o) => match (self.d.pmu.as_ref(), self.pmu.as_mut()) {
                (Some(s), Some(p)) => p.read32(o - s.base),
                _ => 0,
            },
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
                    // FWSEC-FRTS is the GSP start that finds WPR2 down: the first one after
                    // GFW, and the first of every later boot without a function-level reset
                    // (nova-core re-probes that way: FWSEC-FRTS, a bare start with the libos
                    // arguments, and FWSEC-SB at unload are all GSP starts, Linux 7.2.7
                    // gsp/hal/tu102.rs:136-141, 182-219, 308-314). The other starts find
                    // WPR2 up (nova's bare start, FWSEC-SB before Booter Unload) or GSP-RM
                    // loaded.
                    if run == 1 || (self.wpr2.1 == 0 && !self.loaded) {
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
                        mem.read_u64(pa) != Some(crate::ver::common::WPR_META_MAGIC)
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
        } else if self.in_pmu(off) {
            w.notes.extend(self.pmu_write(off, v));
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
        assert_eq!(
            w.event,
            Some(BootEvent::BooterUnload { arg: 0x1_0000_0000 })
        );
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
        assert!(
            w.notes[0].contains("sec2 start 4: booter unload done"),
            "{w:?}"
        );
    }

    /// The PMU falcon as the Windows driver drives it for the ACR unload (run P,
    /// ANALYSIS-gsp-path.md §1): the host card's identity registers, the descriptor through
    /// DMEMC with auto-increment, FBIF at +0xE00, a start halts with IRQSTAT HALT and
    /// MAILBOX0 0, and NV_PMC_ENABLE bit 13 rising resets it.
    #[test]
    fn pmu_falcon_loads_starts_and_resets() {
        let d = data();
        let pmu = d.pmu.as_ref().unwrap().base;
        assert_eq!(pmu, 0x10_A000);
        let mem = GuestMemory::new();
        let mut b = TuringBoot::new(d, 4 << 30);
        assert_eq!(b.read32(pmu + falcon::HWCFG), Some(0x400E_0100));
        assert_eq!(b.read32(pmu + falcon::HWCFG1), Some(0xEA10_4136));
        assert_eq!(b.read32(pmu + falcon::ITFEN), Some(0x5));
        assert_eq!(b.read32(pmu + falcon::CPUCTL), Some(0));
        assert_eq!(b.read32(pmu + falcon::DMACTL), Some(0));
        // The other falcons' identity registers are the card's too.
        assert_eq!(b.read32(d.gsp.base + falcon::HWCFG1), Some(0xCA10_1136));
        assert_eq!(b.read32(d.gsp.base + falcon::ITFEN), Some(0x1004));
        assert_eq!(b.read32(d.sec2.base + falcon::ITFEN), Some(0x1007));
        assert_eq!(b.read32(d.sec2.base + falcon::HWCFG), Some(0x4042_0100));
        // HWCFG keeps the IMEM size the Linux RM places its loader by.
        assert_eq!(b.read32(d.gsp.base + falcon::HWCFG).unwrap() & 0x1FF, 0x100);

        b.write32(pmu + falcon::ITFEN, 0x100, &mem);
        assert_eq!(b.read32(pmu + falcon::ITFEN), Some(0x100));
        for (i, v) in [4u32, 0, 4, 5, 6].into_iter().enumerate() {
            b.write32(pmu + 0xE00 + 4 * i as u64, v, &mem);
        }
        b.write32(pmu + 0xE24, 0x80, &mem);
        assert_eq!(b.read32(pmu + 0xE0C), Some(5));
        assert_eq!(b.read32(pmu + 0xE24), Some(0x80));
        // DMEMC 0x11000000 (AINCW), 21 descriptor words: the offset advances.
        b.write32(pmu + falcon::DMEMC0, 0x1100_0000, &mem);
        for i in 0..21u32 {
            b.write32(pmu + falcon::DMEMD0, 0x100 + i, &mem);
            assert_eq!(
                b.read32(pmu + falcon::DMEMC0),
                Some(0x1100_0000 + 4 * (i + 1))
            );
        }
        assert_eq!(
            &b.pmu.as_ref().unwrap().dmem()[..4],
            &0x100u32.to_le_bytes()
        );
        b.write32(pmu + falcon::MAILBOX0, 0xDEAD, &mem);
        b.write32(pmu + falcon::BOOTVEC, 0xFD00, &mem);
        let w = b.write32(pmu + falcon::CPUCTL, 2, &mem).unwrap();
        assert!(w.notes[0].starts_with("pmu start 1: halted"), "{w:?}");
        assert_eq!(b.read32(pmu + falcon::CPUCTL), Some(0x10));
        assert_eq!(b.read32(pmu + falcon::IRQSTAT), Some(0x10));
        assert_eq!(b.read32(pmu + falcon::MAILBOX0), Some(0));
        b.write32(pmu + falcon::IRQSCLR, 0x10, &mem);
        assert_eq!(b.read32(pmu + falcon::IRQSTAT), Some(0));

        // NV_PMC_ENABLE: bit 13 rising resets the PMU; other edges do not.
        assert!(b.observe_write(0x200, 0xD0FD_D1C1, 0xD0FD_D1C1).is_empty());
        let n = b.observe_write(0x200, 0xD0FD_D1C1, 0xD1FD_FFD1);
        assert!(n[0].contains("pmu reset 1"), "{n:?}");
        assert_eq!(b.read32(pmu + falcon::CPUCTL), Some(0));
        assert_eq!(b.read32(pmu + falcon::ITFEN), Some(0x5));
        assert_eq!(b.read32(pmu + falcon::DMEMC0), Some(0));
        assert!(b.observe_write(0x200, 0xD1FD_FFD1, 0xD1FD_FFD1).is_empty());
        // The GSP's IRQSTAT keeps its latch (no HALT bit on a GSP start).
        b.write32(d.gsp.base + falcon::CPUCTL, 2, &mem);
        assert_eq!(b.read32(d.gsp.base + falcon::IRQSTAT), Some(0));
    }

    /// Guest RAM at `pa` holding the RTX 2070 VBIOS's FWSEC (app 0x85) DMEM image, 0x420
    /// bytes from the PROM window (0x66564 + IMEM 0x9800), with `cmd` patched into the
    /// DMEMMAPPER and `frts_4k` into the FRTS region, as a driver patches it.
    fn fwsec_guest(
        tag: &str,
        pa: u64,
        cmd: u32,
        frts_4k: u32,
    ) -> Option<(GuestMemory, std::path::PathBuf)> {
        use std::io::Write;
        let window = crate::fixture::rtx2070_prom_window()?;
        let mut data = window[0x6FD64..0x6FD64 + 0x420].to_vec();
        data[0x390 + 0x2C..][..4].copy_from_slice(&cmd.to_le_bytes());
        data[0x3E0 + 0x20..][..4].copy_from_slice(&frts_4k.to_le_bytes());
        let path = std::env::temp_dir().join(format!("nvgpu-fwsec-{tag}-{}", std::process::id()));
        let mut f = std::fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        f.write_all(&vec![0u8; 0x1000]).unwrap();
        let mut mem = GuestMemory::new();
        mem.map(pa, 0x1000, 0, &f).unwrap();
        assert!(mem.write(pa, &data));
        Some((mem, path))
    }

    /// The loader descriptor the Windows driver writes (run P): dataDmaBase `pa`, dataSize
    /// 0x420; then the start.
    fn fwsec_start(b: &mut TuringBoot, pa: u64, mem: &GuestMemory) -> BootWrite {
        let gsp = data().gsp.base;
        b.write32(gsp + falcon::ENGINE, 1, mem);
        b.write32(gsp + falcon::ENGINE, 0, mem);
        b.write32(gsp + falcon::DMEMC0, 1 << 24, mem);
        for i in 0..21u32 {
            let v = match i {
                16 => pa as u32,
                17 => (pa >> 32) as u32,
                18 => 0x420,
                _ => 0,
            };
            b.write32(gsp + falcon::DMEMD0, v, mem);
        }
        b.write32(gsp + falcon::CPUCTL, 2, mem).unwrap()
    }

    /// The real VBIOS's FWSEC data image: the mapper is found at +0x390 (the app interface
    /// table at +0xE0 names it). SB (0x19, what Windows most likely runs) leaves WPR2 down;
    /// FRTS (0x15) raises it from the FRTS region.
    #[test]
    fn fwsec_command_decides_wpr2() {
        let d = data();
        let pa = 0x1_7FC4_5000;
        let Some((mem, path)) = fwsec_guest("sb", pa, FWSEC_CMD_SB, 0) else {
            return;
        };
        assert_eq!(
            fwsec_command(&mem, pa, 0x420),
            Some(FwsecCmd {
                at: 0x390,
                cmd: 0x19,
                cmd_in: 0x3E0
            })
        );
        let mut b = TuringBoot::new(d, 8 << 30);
        let w = fwsec_start(&mut b, pa, &mem);
        assert!(
            w.notes
                .iter()
                .any(|n| n.contains("command 0x19 (SB)") && n.contains("WPR2 unchanged")),
            "{w:?}"
        );
        assert_eq!(
            (b.read32(d.wpr2_addr_lo), b.read32(d.wpr2_addr_hi)),
            (Some(0), Some(0))
        );
        std::fs::remove_file(path).ok();

        let Some((mem, path)) = fwsec_guest("frts", pa, FWSEC_CMD_FRTS, 0x1_FDE00) else {
            return;
        };
        let mut b = TuringBoot::new(d, 8 << 30);
        let w = fwsec_start(&mut b, pa, &mem);
        assert!(
            w.notes
                .iter()
                .any(|n| n.contains("command 0x15 (FRTS)") && n.contains("in guest RAM")),
            "{w:?}"
        );
        // frtsRegion.addr 0x1fde00 (4 KiB units): WPR2 lo = addr << 4.
        assert_eq!(b.read32(d.wpr2_addr_lo), Some(0x1FD_E000));
        assert_ne!(b.read32(d.wpr2_addr_hi), Some(0));
        std::fs::remove_file(path).ok();
    }

    /// boot.toml's identity registers are the captured ones (GSP from the BAR0 capture, PMU
    /// and SEC2 from the falcon capture), and both captures merge into one table.
    #[test]
    fn falcon_identity_registers_are_the_capture() {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../caraxes/reference/rtx2070/"
        );
        let list =
            format!("{dir}bar0-registers-2026-09-25.txt,{dir}falcon-pmu-sec2-2026-09-25.txt");
        let cap = crate::bar0::CapturedRegisters::load(std::path::Path::new(&list)).unwrap();
        let d = data();
        let mut b = TuringBoot::new(d, 4 << 30);
        for spec in [&d.gsp, &d.sec2, d.pmu.as_ref().unwrap()] {
            for off in [falcon::HWCFG, falcon::HWCFG1, falcon::ITFEN] {
                assert_eq!(
                    b.read32(spec.base + off),
                    cap.get(spec.base + off),
                    "{:#x}",
                    spec.base + off
                );
            }
        }
        // The idle ENGCTL the card reads is the model's too.
        assert_eq!(b.read32(0x10A0A4), cap.get(0x10A0A4));
        assert_eq!(b.read32(0x8400A4), cap.get(0x8400A4));
    }

    /// The ACR unload on the PMU (BOOTVEC 0x10000, run P at 115.32): after FWSEC-FRTS raised
    /// WPR2, the unload's halt tears WPR1 and WPR2 down and leaves MAILBOX0 0.
    #[test]
    fn acr_unload_clears_wpr() {
        let d = data();
        let pmu = d.pmu.as_ref().unwrap().base;
        let pa = 0x1_7FC4_5000;
        let Some((mem, path)) = fwsec_guest("unload", pa, FWSEC_CMD_FRTS, 0x1_FDE00) else {
            return;
        };
        let mut b = TuringBoot::new(d, 8 << 30);
        fwsec_start(&mut b, pa, &mem);
        assert_ne!(b.read32(d.wpr2_addr_hi), Some(0));
        assert_eq!(b.read32(0x1F_A81C), Some(0));
        assert_eq!(b.read32(0x1F_A820), Some(0));
        b.write32(pmu + falcon::MAILBOX0, 0xDEAD, &mem);
        b.write32(pmu + falcon::BOOTVEC, 0x1_0000, &mem);
        let w = b.write32(pmu + falcon::CPUCTL, 2, &mem).unwrap();
        assert!(w.notes[0].contains("acr unload"), "{w:?}");
        assert!(w.notes[0].contains("WPR2 lo/hi 0x1fde000"), "{w:?}");
        for off in [0x1F_A81C, 0x1F_A820, d.wpr2_addr_lo, d.wpr2_addr_hi] {
            assert_eq!(b.read32(off), Some(0), "{off:#x}");
        }
        assert_eq!(b.read32(pmu + falcon::MAILBOX0), Some(0));
        assert_eq!(b.read32(pmu + falcon::CPUCTL), Some(0x10));
        std::fs::remove_file(path).ok();
    }

    /// Run P's gate: `W 0x10a0a4 0x2`, then the poll reads the stall acknowledge at once.
    #[test]
    fn pmu_stall_request_is_acknowledged() {
        let d = data();
        let pmu = d.pmu.as_ref().unwrap().base;
        let mem = GuestMemory::new();
        let mut b = TuringBoot::new(d, 4 << 30);
        assert_eq!(b.read32(pmu + falcon::ENGCTL), Some(0));
        b.write32(pmu + falcon::ENGCTL, 0x2, &mem);
        let want = d.stallack.unwrap_or(falcon::STALLACK_DEFAULT);
        assert_eq!(b.read32(pmu + falcon::ENGCTL), Some(want));
    }

    /// nova-core's Turing boot and re-probe (Linux 7.2.7 gsp/hal/tu102.rs): FWSEC-FRTS, a
    /// bare GSP start with the libos arguments, Booter Load; at unbind FWSEC-SB on the GSP
    /// and Booter Unload; the next probe runs FWSEC-FRTS again as the GSP's 4th start,
    /// without a function-level reset. Only the starts that find WPR2 down create it.
    #[test]
    fn nova_reprobe_runs_fwsec_frts_again() {
        let d = data();
        let (gsp, sec2) = (d.gsp.base, d.sec2.base);
        let mem = GuestMemory::new();
        let mut b = TuringBoot::new(d, 1 << 30);
        let start = |b: &mut TuringBoot| {
            b.write32(gsp + falcon::ENGINE, 1, &mem);
            b.write32(gsp + falcon::ENGINE, 0, &mem);
            b.write32(gsp + falcon::CPUCTL, 2, &mem).unwrap()
        };
        let w = start(&mut b);
        assert!(w.notes.iter().any(|n| n.starts_with("fwsec:")), "{w:?}");
        let wpr2 = (b.read32(d.wpr2_addr_lo), b.read32(d.wpr2_addr_hi));
        assert_ne!(wpr2.1, Some(0));
        // The bare start (libos PA in the mailboxes) leaves WPR2 alone.
        let w = start(&mut b);
        assert!(!w.notes.iter().any(|n| n.starts_with("fwsec:")), "{w:?}");
        b.write32(sec2 + falcon::MAILBOX0, 0x7654_3000, &mem);
        b.write32(sec2 + falcon::CPUCTL, 2, &mem);
        // Unbind: FWSEC-SB while WPR2 is up, then Booter Unload with the 0xFF sentinel.
        let w = start(&mut b);
        assert!(!w.notes.iter().any(|n| n.starts_with("fwsec:")), "{w:?}");
        b.write32(sec2 + falcon::MAILBOX0, 0xff, &mem);
        b.write32(sec2 + falcon::MAILBOX1, 0xff, &mem);
        let w = b.write32(sec2 + falcon::CPUCTL, 2, &mem).unwrap();
        assert!(w.notes[0].contains("booter unload done"), "{w:?}");
        assert_eq!(b.read32(d.wpr2_addr_hi), Some(0));
        // Re-probe: FWSEC-FRTS as the GSP's 4th start creates WPR2 where it was.
        let w = start(&mut b);
        assert!(w.notes[0].contains("gsp start 4"), "{w:?}");
        assert!(w.notes.iter().any(|n| n.starts_with("fwsec:")), "{w:?}");
        assert_eq!((b.read32(d.wpr2_addr_lo), b.read32(d.wpr2_addr_hi)), wpr2);
    }
}

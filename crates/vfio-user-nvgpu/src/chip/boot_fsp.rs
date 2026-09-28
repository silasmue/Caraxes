// SPDX-License-Identifier: MIT
//! The FSP firmware-boot model (GH100-style Chain-of-Trust boot, used by GB20x):
//! everything the CPU expects to see from FSP, GSP-FMC/ACR and the GSP RISC-V core
//! when the 610.57.04 kernel RM boots GSP-RM on a Blackwell GB20x part, without any
//! firmware running (docs/caraxes/gsp-boot-blackwell.md).
//!
//! The sequence the model answers (`kgspBootstrap_GH100`, the GB206 HAL;
//! kernel_gsp_gh100.c:853-1037):
//!
//! 1. FSP boot complete: `NV_THERM_I2CS_SCRATCH` reads 0xFF (`kfspWaitForSecureBoot_GB202`).
//! 2. NVDM messages over the FSP EMEM channel 0 and its command/message queue pointers
//!    (`kfspSendPacket_GH100`/`kfspReadPacket_GH100`): the CAPS_QUERY for clock boost,
//!    the CLOCK_BOOST enable and the COT boot command. Every message is answered with an
//!    NVDM FSP_RESPONSE carrying `FSP_OK`.
//! 3. The COT payload carries the address of `GSP_FMC_BOOT_PARAMS`; the model reads it,
//!    finds the `GspFwWprMeta` and the libos init arguments through it, completes the WPR
//!    layout the way ACR would (the CPU only sends size requests on GH100 and later,
//!    kernel_gsp_gh100.c:283-301) and hands the meta to the generic model
//!    ([`BootEvent::WprMeta`]). From then on the GSP target mask reads released, the
//!    RISC-V lockdown reads unlocked and the core reads active.
//! 4. The CPU queues its init RPCs and rings `NV_PGSP_QUEUE_HEAD(0)`; the model reports
//!    the doorbell with the libos arguments from the FMC parameters
//!    ([`BootEvent::CmdQueueDoorbell`]), and the generic model links the queues.
//!
//! Register offsets and values come from `chips/<chip>/boot.toml`; the NVDM/MCTP framing
//! and the COT layout are fixed by the 610 headers cited below.

use serde::Deserialize;

use super::turing::{ConstReg, FalconSpec};
use super::{BootEvent, BootWrite, FirmwareBoot};
use crate::falcon::{self, Falcon};
use crate::guestmem::GuestMemory;
use crate::ver::common as v;

/// The FSP registers of `boot.toml` `[fsp]`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FspRegs {
    /// `NV_THERM_I2CS_SCRATCH` and the value FSP writes when it has booted.
    pub boot_complete: ConstReg,
    /// `NV_PFSP_EMEMC(0)`/`NV_PFSP_EMEMD(0)`: the EMEM port of the RM channel.
    pub ememc: u64,
    pub ememd: u64,
    /// `NV_PFSP_QUEUE_HEAD(0)`/`_TAIL(0)` (CPU to FSP) and `NV_PFSP_MSGQ_HEAD(0)`/`_TAIL(0)`
    /// (FSP to CPU).
    pub queue_head: u64,
    pub queue_tail: u64,
    pub msgq_head: u64,
    pub msgq_tail: u64,
    /// Bytes of EMEM the RM channel owns (`FSP_EMEM_CHANNEL_RM_SIZE`).
    pub emem_bytes: u32,
}

/// The ACR side of the WPR layout (`boot.toml` `[acr]`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcrRegs {
    /// `NV_PFB_PRI_MMU_WPR2_ADDR_LO/HI` as the GB20x HAL reads them; ADDR in 4 KiB units
    /// at bit `wpr2_addr_shift`.
    pub wpr2_addr_lo: u64,
    pub wpr2_addr_hi: u64,
    pub wpr2_addr_shift: u32,
    /// `NV_USABLE_FB_SIZE_IN_MB`: the FB size in MiB the kernel RM reads for its heap
    /// estimate (kmemsysReadUsableFbSize_GA102).
    pub usable_fb_size_mb: u64,
    /// GSP-FW heap when the CPU asks for none (bytes).
    pub default_heap_size: u64,
}

/// The secure timer and the GSP heartbeat mailboxes (`boot.toml` `[timer]`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimerRegs {
    /// `NV_PGC6_SCI_SEC_TIMER_TIME_0/1`: NSEC 31:5 and 28:0.
    pub sec_time_0: u64,
    pub sec_time_1: u64,
    /// `NV_PGC6_SCI_SYS_TIMER_OFFSET_0/1`: the offset the CPU programs (UPDATE 0:0 in _0).
    pub sys_offset_0: u64,
    pub sys_offset_1: u64,
    /// `NV_PGSP_MAILBOX(GSPRM_HEARTBEAT)` and `(LIBOS_HEARTBEAT)`.
    pub heartbeat: [u64; 2],
}

/// `chips/<chip>/boot.toml` for an FSP boot.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FspBootData {
    pub gsp: FalconSpec,
    pub falcon_block_size: u64,
    /// `NV_PGSP_QUEUE_HEAD(0)`: the command-queue doorbell.
    pub gsp_queue_head: u64,
    /// `NV_PRISCV_RISCV_CPUCTL` of the GSP (absolute).
    pub gsp_riscv_cpuctl: u64,
    pub fsp: FspRegs,
    pub acr: AcrRegs,
    pub timer: TimerRegs,
    #[serde(rename = "const")]
    pub consts: Vec<ConstReg>,
}

impl FspBootData {
    pub fn parse(text: &str) -> Result<Self, String> {
        let d: Self = toml::from_str(text).map_err(|e| format!("boot.toml: {e}"))?;
        if d.fsp.emem_bytes == 0 || d.fsp.emem_bytes % 4 != 0 || d.fsp.emem_bytes > 0x1_0000 {
            return Err(format!(
                "boot.toml: implausible emem_bytes {:#x}",
                d.fsp.emem_bytes
            ));
        }
        Ok(d)
    }
}

// NV_PFALCON_FALCON_HWCFG2 fields (hopper/gh100/dev_falcon_v4.h:70, 79-81).
const HWCFG2_RISCV: u32 = 1 << 10;
const HWCFG2_RISCV_BR_PRIV_LOCKDOWN: u32 = 1 << 13;
// NV_PRISCV_RISCV_CPUCTL fields (ampere/ga102/dev_riscv_pri.h:32-35; read by
// kflcnIsRiscvActive_GA102, kernel_falcon_ga102.c:47-56, the GB206 HAL).
const RISCV_CPUCTL_HALTED: u32 = 1 << 4;
const RISCV_CPUCTL_ACTIVE_STAT: u32 = 1 << 7;

// NV_PFSP_EMEMC fields (hopper/gh100/dev_fsp_pri.h:28-36): OFFS 7:2 in dwords within
// a 64-dword block, BLK 15:8, AINCW 24, AINCR 25 (kern_fsp_gh100.c:64, 515-560).
const EMEMC_AINCW: u32 = 1 << 24;
const EMEMC_AINCR: u32 = 1 << 25;
const DWORDS_PER_EMEM_BLOCK: u32 = 64;

// MCTP transport header (arch/nvalloc/common/inc/mctp_format.h:40-48) and the MCTP
// message header carrying the NVDM type (:51-58); NVDM types (nvdm_format.h:43-61).
const MCTP_SOM: u32 = 1 << 31;
const MCTP_EOM: u32 = 1 << 30;
const MCTP_HEADER_VERSION_1: u32 = 1;
const MCTP_MSG_TYPE_VENDOR_PCI: u32 = 0x7E;
const MCTP_VENDOR_ID_NV: u32 = 0x10DE;
pub const NVDM_TYPE_COT: u8 = 0x14;
pub const NVDM_TYPE_FSP_RESPONSE: u8 = 0x15;
pub const NVDM_TYPE_CAPS_QUERY: u8 = 0x16;
pub const NVDM_TYPE_CLOCK_BOOST: u8 = 0x20;
/// `FSP_OK` (inc/kernel/gpu/fsp/kern_fsp_retval.h:24).
const FSP_OK: u32 = 0;

/// `NVDM_PAYLOAD_COT` (inc/kernel/gpu/fsp/kern_fsp_cot_payload.h:36-55, `#pragma pack(1)`;
/// offsets derived from the packed layout): version, size, gspFmcSysmemOffset,
/// frtsSysmemOffset/Size, frtsVidmemOffset/Size, `hash384[12]`, `publicKey[96]`,
/// `signature[96]`, gspBootArgsSysmemOffset.
pub const COT_SIZE: usize = 860;
const COT_GSP_FMC_SYSMEM: usize = 4;
const COT_FRTS_SYSMEM_OFFSET: usize = 12;
const COT_FRTS_SYSMEM_SIZE: usize = 20;
const COT_FRTS_VIDMEM_OFFSET: usize = 24;
const COT_FRTS_VIDMEM_SIZE: usize = 32;
const COT_GSP_BOOT_ARGS: usize = 852;

/// The COT fields the model uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cot {
    pub version: u16,
    pub size: u16,
    pub gsp_fmc_sysmem: u64,
    pub frts_sysmem: (u64, u32),
    /// FRTS in vidmem: offset from the end of FB, size (kern_fsp_cot_payload.h:45-47).
    pub frts_vidmem: (u64, u32),
    pub gsp_boot_args: u64,
}

impl Cot {
    fn parse(p: &[u8]) -> Option<Self> {
        if p.len() < COT_SIZE {
            return None;
        }
        let u16_at = |o: usize| u16::from_le_bytes(p[o..o + 2].try_into().unwrap());
        let u32_at = |o: usize| u32::from_le_bytes(p[o..o + 4].try_into().unwrap());
        let u64_at = |o: usize| u64::from_le_bytes(p[o..o + 8].try_into().unwrap());
        Some(Cot {
            version: u16_at(0),
            size: u16_at(2),
            gsp_fmc_sysmem: u64_at(COT_GSP_FMC_SYSMEM),
            frts_sysmem: (u64_at(COT_FRTS_SYSMEM_OFFSET), u32_at(COT_FRTS_SYSMEM_SIZE)),
            frts_vidmem: (u64_at(COT_FRTS_VIDMEM_OFFSET), u32_at(COT_FRTS_VIDMEM_SIZE)),
            gsp_boot_args: u64_at(COT_GSP_BOOT_ARGS),
        })
    }
}

/// The WPR layout the model computes in place of ACR (all byte offsets into the FB).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcrLayout {
    pub fb_size: u64,
    pub vga_workspace_offset: u64,
    pub vga_workspace_size: u64,
    pub frts_offset: u64,
    pub frts_size: u64,
    pub gsp_fw_wpr_end: u64,
    pub boot_bin_offset: u64,
    pub gsp_fw_offset: u64,
    pub gsp_fw_heap_offset: u64,
    pub gsp_fw_heap_size: u64,
    pub gsp_fw_wpr_start: u64,
    pub non_wpr_heap_offset: u64,
    pub non_wpr_heap_size: u64,
}

const MIB: u64 = 1 << 20;

fn align_down(x: u64, a: u64) -> u64 {
    x & !(a - 1)
}

fn align_up(x: u64, a: u64) -> Option<u64> {
    Some(x.checked_add(a - 1)? & !(a - 1))
}

/// The CPU's size requests in the WPR meta (kgspPopulateWprMeta_GH100).
#[derive(Clone, Copy, Debug, Default)]
pub struct WprRequest {
    pub vga_workspace_size: u64,
    pub pmu_reserved_size: u64,
    pub size_of_bootloader: u64,
    pub size_of_radix3_elf: u64,
    pub non_wpr_heap_size: u64,
    pub gsp_fw_heap_size: u64,
    pub frts_size: u64,
}

impl AcrLayout {
    /// Lay the WPR out from the top of `fb_size` down, with the TU102 alignments of
    /// `kgspPopulateWprMeta_TU102` (kernel_gsp_tu102.c:755-860) in the order of the
    /// GH100 picture (kernel_gsp_gh100.c:283-301): VGA workspace at the end, FRTS where
    /// the COT puts it (`frtsVidmemOffset` below the end of FB), then boot binary, ELF,
    /// heap, meta and the non-WPR heap. Derived: the real ACR is not documented.
    pub fn compute(
        fb_size: u64,
        req: &WprRequest,
        frts_from_end: u64,
        heap_default: u64,
    ) -> Result<Self, String> {
        let under = |what: &str| format!("FB of {fb_size:#x} bytes too small for the WPR ({what})");
        let vga_size = if req.vga_workspace_size == 0 {
            128 << 10
        } else {
            req.vga_workspace_size
        };
        let vga = align_down(
            fb_size.checked_sub(vga_size).ok_or_else(|| under("VGA"))?,
            64 << 10,
        );
        let frts_size = if req.frts_size == 0 {
            MIB
        } else {
            req.frts_size
        };
        // FRTS top: frtsVidmemOffset below the end of FB, or just under the VGA workspace
        // and the PMU reservation when the COT gives none.
        let frts_top = if frts_from_end != 0 {
            fb_size.checked_sub(frts_from_end)
        } else {
            vga.checked_sub(req.pmu_reserved_size)
        }
        .map(|t| align_down(t, 128 << 10))
        .ok_or_else(|| under("FRTS"))?;
        let frts = frts_top
            .checked_sub(frts_size)
            .ok_or_else(|| under("FRTS"))?;
        let boot_bin = align_down(
            frts.checked_sub(req.size_of_bootloader)
                .ok_or_else(|| under("boot binary"))?,
            0x1000,
        );
        let gsp_fw = align_down(
            boot_bin
                .checked_sub(req.size_of_radix3_elf)
                .ok_or_else(|| under("ELF"))?,
            0x1_0000,
        );
        let heap_size = align_up(
            if req.gsp_fw_heap_size == 0 {
                heap_default
            } else {
                req.gsp_fw_heap_size
            },
            MIB,
        )
        .ok_or_else(|| under("heap"))?;
        let heap = align_down(
            gsp_fw.checked_sub(heap_size).ok_or_else(|| under("heap"))?,
            MIB,
        );
        // The meta itself, 1 MiB aligned (wprMetaSize, kernel_gsp_tu102.c:818).
        let wpr_start = heap.checked_sub(MIB).ok_or_else(|| under("meta"))?;
        let non_wpr_size =
            align_up(req.non_wpr_heap_size, MIB).ok_or_else(|| under("non-WPR heap"))?;
        let non_wpr = wpr_start
            .checked_sub(non_wpr_size)
            .ok_or_else(|| under("non-WPR heap"))?;
        Ok(AcrLayout {
            fb_size,
            vga_workspace_offset: vga,
            vga_workspace_size: fb_size - vga,
            frts_offset: frts,
            frts_size,
            gsp_fw_wpr_end: frts_top,
            boot_bin_offset: boot_bin,
            gsp_fw_offset: gsp_fw,
            gsp_fw_heap_offset: heap,
            gsp_fw_heap_size: gsp_fw - heap,
            gsp_fw_wpr_start: wpr_start,
            non_wpr_heap_offset: non_wpr,
            non_wpr_heap_size: non_wpr_size,
        })
    }

    /// Patch the layout into the meta at `pa` (the fields ACR fills on GH100 and later).
    fn write(&self, mem: &GuestMemory, pa: u64) -> bool {
        [
            (v::WPR_META_GSP_FW_RSVD_START, self.non_wpr_heap_offset),
            (v::WPR_META_NON_WPR_HEAP_OFFSET, self.non_wpr_heap_offset),
            (v::WPR_META_NON_WPR_HEAP_SIZE, self.non_wpr_heap_size),
            (v::WPR_META_GSP_FW_WPR_START, self.gsp_fw_wpr_start),
            (v::WPR_META_GSP_FW_HEAP_OFFSET, self.gsp_fw_heap_offset),
            (v::WPR_META_GSP_FW_HEAP_SIZE, self.gsp_fw_heap_size),
            (v::WPR_META_GSP_FW_OFFSET, self.gsp_fw_offset),
            (v::WPR_META_BOOT_BIN_OFFSET, self.boot_bin_offset),
            (v::WPR_META_FRTS_OFFSET, self.frts_offset),
            (v::WPR_META_FRTS_SIZE, self.frts_size),
            (v::WPR_META_GSP_FW_WPR_END, self.gsp_fw_wpr_end),
            (v::WPR_META_FB_SIZE, self.fb_size),
            (v::WPR_META_VGA_WORKSPACE_OFFSET, self.vga_workspace_offset),
            (v::WPR_META_VGA_WORKSPACE_SIZE, self.vga_workspace_size),
        ]
        .iter()
        .all(|&(off, val)| mem.write(pa + off as u64, &val.to_le_bytes()))
    }
}

/// Where the boot stands, as the CPU can observe it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Out of reset: FSP booted, GSP halted and locked down.
    Reset,
    /// COT processed: target mask released, lockdown released, GSP-RM "running".
    GspRunning,
}

/// The FSP boot model of one card.
pub struct FspBoot {
    d: &'static FspBootData,
    pub gsp: Falcon,
    fb_bytes: u64,
    phase: Phase,
    emem: Vec<u32>,
    ememc: u32,
    /// Dword position of the EMEM port.
    emem_pos: u32,
    queue: (u32, u32),
    msgq: (u32, u32),
    /// NVDM messages received, by type (for the trace and the tests).
    pub messages: Vec<u8>,
    pub cot: Option<Cot>,
    pub layout: Option<AcrLayout>,
    fmc_params_pa: Option<u64>,
    libos_args_pa: Option<u64>,
    wpr2: (u32, u32),
    /// Multi-packet message being assembled: NVDM type and payload so far.
    partial: Option<(u8, Vec<u8>)>,
    /// The secure timer's time base: created with the model, like the device's PTIMER
    /// clock (channel.rs `Registry::now_ns`, created by the same `BootModel::new`).
    clock: std::time::Instant,
    /// The system-timer offset the CPU programmed (`tmrSetCurrentTime_GH100`) and the
    /// high half written before the low one.
    sys_offset: (u64, u32),
}

impl FspBoot {
    pub fn new(d: &'static FspBootData, fb_bytes: u64) -> Self {
        let g = &d.gsp;
        FspBoot {
            gsp: Falcon::new("gsp", g.riscv, g.imem_blocks, g.dmem_blocks, g.halted),
            d,
            fb_bytes,
            phase: Phase::Reset,
            emem: vec![0; (d.fsp.emem_bytes / 4) as usize],
            ememc: 0,
            emem_pos: 0,
            queue: (0, 0),
            msgq: (0, 0),
            messages: Vec::new(),
            cot: None,
            layout: None,
            fmc_params_pa: None,
            libos_args_pa: None,
            wpr2: (0, 0),
            partial: None,
            clock: std::time::Instant::now(),
            sys_offset: (0, 0),
        }
    }

    fn in_gsp(&self, off: u64) -> bool {
        (self.d.gsp.base..self.d.gsp.base + self.d.falcon_block_size).contains(&off)
    }

    fn fsp_reg(&self, off: u64) -> bool {
        let f = &self.d.fsp;
        [
            f.boot_complete.offset,
            f.ememc,
            f.ememd,
            f.queue_head,
            f.queue_tail,
            f.msgq_head,
            f.msgq_tail,
        ]
        .contains(&off)
    }

    fn constant(&self, off: u64) -> Option<u32> {
        self.d
            .consts
            .iter()
            .find(|c| c.offset == off)
            .map(|c| c.value)
    }

    fn ememc_value(&self) -> u32 {
        let flags = self.ememc & (EMEMC_AINCW | EMEMC_AINCR);
        ((self.emem_pos % DWORDS_PER_EMEM_BLOCK) << 2)
            | ((self.emem_pos / DWORDS_PER_EMEM_BLOCK) << 8)
            | flags
    }

    /// One NVDM message from the CPU (the payload after the NVDM type byte).
    fn message(&mut self, nvdm: u8, payload: &[u8], mem: &GuestMemory) -> BootWrite {
        let mut w = BootWrite::default();
        self.messages.push(nvdm);
        match nvdm {
            NVDM_TYPE_COT => match Cot::parse(payload) {
                Some(cot) => {
                    w.notes.push(format!(
                        "fsp: COT version {} size {}: gspFmc {:#x}, frts sysmem {:#x}+{:#x}, frts vidmem {:#x} from the end +{:#x}, GSP_FMC_BOOT_PARAMS {:#x}",
                        cot.version,
                        cot.size,
                        cot.gsp_fmc_sysmem,
                        cot.frts_sysmem.0,
                        cot.frts_sysmem.1,
                        cot.frts_vidmem.0,
                        cot.frts_vidmem.1,
                        cot.gsp_boot_args
                    ));
                    self.cot = Some(cot);
                    self.boot_gsp(cot, mem, &mut w);
                }
                None => w.notes.push(format!(
                    "fsp: COT payload of {} bytes, expected {COT_SIZE}; ignored",
                    payload.len()
                )),
            },
            NVDM_TYPE_CAPS_QUERY => w.notes.push(format!(
                "fsp: CAPS_QUERY sub-message {:#x}: supported",
                payload.first().copied().unwrap_or(0)
            )),
            NVDM_TYPE_CLOCK_BOOST => w.notes.push(format!(
                "fsp: CLOCK_BOOST sub-message {:#x}: accepted",
                payload.first().copied().unwrap_or(0)
            )),
            t => w.notes.push(format!(
                "fsp: NVDM type {t:#x} ({} bytes): acknowledged",
                payload.len()
            )),
        }
        self.respond(nvdm);
        w
    }

    /// Queue an NVDM FSP_RESPONSE with `FSP_OK` for a message of type `nvdm`: one
    /// packet in EMEM from offset 0, the message queue pointers around it
    /// (kfspReadPacket_GH100, kern_fsp_gh100.c:668-725; the CPU parses it in
    /// kfspReadMessage and kfspProcessCommandResponse_GH100, kern_fsp.c:599-705,
    /// kern_fsp_gh100.c:466-510). NVDM_PAYLOAD_COMMAND_RESPONSE is taskId,
    /// commandNvdmType, errorCode (fsp/nvdm_payload_cmd_response.h).
    fn respond(&mut self, nvdm: u8) {
        let words = [
            MCTP_SOM | MCTP_EOM | MCTP_HEADER_VERSION_1,
            MCTP_MSG_TYPE_VENDOR_PCI
                | (MCTP_VENDOR_ID_NV << 8)
                | ((NVDM_TYPE_FSP_RESPONSE as u32) << 24),
            0,
            nvdm as u32,
            FSP_OK,
        ];
        self.emem[..words.len()].copy_from_slice(&words);
        // TAIL is the offset of the last dword; HEAD != TAIL means a response is there.
        self.msgq = (0, (words.len() as u32 - 1) * 4);
    }

    /// FSP and GSP-FMC with ACR: read the FMC parameters, complete the WPR meta and
    /// start GSP-RM.
    fn boot_gsp(&mut self, cot: Cot, mem: &GuestMemory, w: &mut BootWrite) {
        let fmc = cot.gsp_boot_args;
        let mut p = [0u8; v::FMC_PARAMS_SIZE];
        if fmc == u64::MAX || !mem.read(fmc, &mut p) {
            w.notes.push(format!(
                "fsp: GSP_FMC_BOOT_PARAMS at {fmc:#x} not mapped; GSP stays down"
            ));
            return;
        }
        self.fmc_params_pa = Some(fmc);
        let u64_at = |o: usize| u64::from_le_bytes(p[o..o + 8].try_into().unwrap());
        let meta = u64_at(v::FMC_GSP_RM_DESC_OFFSET);
        let meta_size = u32::from_le_bytes(
            p[v::FMC_GSP_RM_DESC_SIZE..v::FMC_GSP_RM_DESC_SIZE + 4]
                .try_into()
                .unwrap(),
        );
        let libos = u64_at(v::FMC_BOOT_ARGS_OFFSET);
        self.libos_args_pa = Some(libos);
        w.notes.push(format!(
            "fsp: GSP_FMC_BOOT_PARAMS {fmc:#x}: gspRmDesc {meta:#x} ({meta_size} bytes), libos args {libos:#x}"
        ));
        let rd = |o: usize| mem.read_u64(meta + o as u64).unwrap_or(0);
        let req = WprRequest {
            vga_workspace_size: rd(v::WPR_META_VGA_WORKSPACE_SIZE),
            pmu_reserved_size: mem
                .read_u32(meta + v::WPR_META_PMU_RESERVED_SIZE as u64)
                .unwrap_or(0) as u64,
            size_of_bootloader: rd(v::WPR_META_SIZE_OF_BOOTLOADER),
            size_of_radix3_elf: rd(v::WPR_META_SIZE_OF_RADIX3_ELF),
            non_wpr_heap_size: rd(v::WPR_META_NON_WPR_HEAP_SIZE),
            gsp_fw_heap_size: rd(v::WPR_META_GSP_FW_HEAP_SIZE),
            frts_size: rd(v::WPR_META_FRTS_SIZE),
        };
        let frts_from_end = if cot.frts_vidmem.1 != 0 {
            cot.frts_vidmem.0
        } else {
            0
        };
        match AcrLayout::compute(
            self.fb_bytes,
            &req,
            frts_from_end,
            self.d.acr.default_heap_size,
        ) {
            Ok(l) => {
                if meta_size as usize != v::WPR_META_SIZE || !l.write(mem, meta) {
                    w.notes.push(format!(
                        "acr: WPR meta at {meta:#x} ({meta_size} bytes) not writable; layout not patched"
                    ));
                }
                let enc = |a: u64| ((a >> 12) << self.d.acr.wpr2_addr_shift) as u32;
                self.wpr2 = (
                    enc(l.non_wpr_heap_offset + l.non_wpr_heap_size),
                    enc(l.gsp_fw_wpr_end - 1),
                );
                w.notes.push(format!(
                    "acr: fb {:#x}, WPR [{:#x}, {:#x}), FRTS {:#x}+{:#x}, heap {:#x}+{:#x}, non-WPR heap {:#x}+{:#x}, VGA {:#x}",
                    l.fb_size,
                    l.gsp_fw_wpr_start,
                    l.gsp_fw_wpr_end,
                    l.frts_offset,
                    l.frts_size,
                    l.gsp_fw_heap_offset,
                    l.gsp_fw_heap_size,
                    l.non_wpr_heap_offset,
                    l.non_wpr_heap_size,
                    l.vga_workspace_offset
                ));
                self.layout = Some(l);
            }
            Err(e) => w.notes.push(format!("acr: {e}")),
        }
        self.phase = Phase::GspRunning;
        self.gsp.set_riscv_active(true);
        w.notes
            .push("gsp: target mask released, lockdown released, GSP-RM running".into());
        w.event = Some(BootEvent::WprMeta { pa: meta });
    }

    fn timer_reg(&self, off: u64) -> bool {
        let t = &self.d.timer;
        [t.sec_time_0, t.sec_time_1, t.sys_offset_0, t.sys_offset_1].contains(&off)
            || t.heartbeat.contains(&off)
    }

    /// The secure timer counts from the model's creation. The heartbeats are GPU time in
    /// ms as GSP-RM and libos write them, in the device's time base: the kernel compares
    /// `heartbeat + sysTimerOffsetNs / 1e6` with its PTIMER time in ms
    /// (`_kgspIsHeartbeatTimedOut`, src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:2264-2300;
    /// `tmrGetPtimerOffsetNs` is 0 on GB206, generated/g_objtmr_nvoc.c:530-540, and
    /// `kfspRequiresBug3957833WAR` false, g_kern_fsp_nvoc.c:620-629). The device's PTIMER
    /// (the usermode TIME registers, uvm.rs) counts from the same moment without the
    /// programmed offset, so the heartbeat is `now - offset`, which the kernel's check
    /// turns back into `now`.
    fn timer_read(&self, off: u64) -> u32 {
        let t = &self.d.timer;
        let now = self.clock.elapsed().as_nanos() as u64;
        match off {
            o if o == t.sec_time_0 => now as u32 & !0x1F,
            o if o == t.sec_time_1 => (now >> 32) as u32 & 0x1FFF_FFFF,
            o if o == t.sys_offset_0 => self.sys_offset.0 as u32 & !0x1F,
            o if o == t.sys_offset_1 => (self.sys_offset.0 >> 32) as u32,
            _ => ((now / 1_000_000) as u32).wrapping_sub((self.sys_offset.0 / 1_000_000) as u32),
        }
    }

    fn timer_write(&mut self, off: u64, v: u32) -> Option<String> {
        let t = &self.d.timer;
        if off == t.sys_offset_1 {
            self.sys_offset.1 = v;
            return None;
        }
        if off == t.sys_offset_0 && v & 1 != 0 {
            // UPDATE_TRIGGER (hopper/gh100/dev_gc6_island.h:35-43).
            self.sys_offset.0 = ((self.sys_offset.1 as u64) << 32) | (v & !0x1F) as u64;
            return Some(format!(
                "timer: system-timer offset {:#x} ns",
                self.sys_offset.0
            ));
        }
        Some("read-only".into())
    }

    /// The CPU wrote `NV_PFSP_QUEUE_HEAD(0)`: a packet of `tail - head + 4` bytes sits in
    /// EMEM from `head` (kfspSendPacket_GH100, kern_fsp_gh100.c:621-665).
    fn packet(&mut self, mem: &GuestMemory) -> BootWrite {
        let (head, tail) = self.queue;
        // FSP consumed it: the CPU polls for HEAD == TAIL before the next packet
        // (kfspCanSendPacket_GH100, kern_fsp_gh100.c:249-275).
        self.queue = (tail, tail);
        if tail < head || tail >= self.d.fsp.emem_bytes {
            return BootWrite {
                notes: vec![format!(
                    "fsp: packet head {head:#x} tail {tail:#x} outside EMEM; dropped"
                )],
                event: None,
            };
        }
        let bytes: Vec<u8> = self.emem[(head / 4) as usize..=(tail / 4) as usize]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect();
        let hdr = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let (som, eom) = (hdr & MCTP_SOM != 0, hdr & MCTP_EOM != 0);
        if som {
            if bytes.len() < 8 {
                return BootWrite {
                    notes: vec!["fsp: first packet shorter than its headers; dropped".into()],
                    event: None,
                };
            }
            let msg = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
            if msg & 0x7F != MCTP_MSG_TYPE_VENDOR_PCI || (msg >> 8) & 0xFFFF != MCTP_VENDOR_ID_NV {
                return BootWrite {
                    notes: vec![format!(
                        "fsp: MCTP message header {msg:#010x} is not NVDM; dropped"
                    )],
                    event: None,
                };
            }
            self.partial = Some(((msg >> 24) as u8, bytes[8..].to_vec()));
        } else if let Some((_, buf)) = self.partial.as_mut() {
            buf.extend_from_slice(&bytes[4..]);
        } else {
            return BootWrite {
                notes: vec!["fsp: continuation packet without a first packet; dropped".into()],
                event: None,
            };
        }
        if !eom {
            return BootWrite::default();
        }
        let (nvdm, payload) = self.partial.take().expect("assembled above");
        self.message(nvdm, &payload, mem)
    }
}

impl FirmwareBoot for FspBoot {
    fn handles(&self, off: u64) -> bool {
        self.timer_reg(off)
            || self.in_gsp(off)
            || self.fsp_reg(off)
            || off == self.d.acr.wpr2_addr_lo
            || off == self.d.acr.wpr2_addr_hi
            || off == self.d.acr.usable_fb_size_mb
            || self.constant(off).is_some()
    }

    fn read32(&mut self, off: u64) -> Option<u32> {
        if !self.handles(off) {
            return None;
        }
        let f = &self.d.fsp;
        let running = self.phase == Phase::GspRunning;
        Some(match off {
            o if o == f.boot_complete.offset => f.boot_complete.value,
            o if o == f.ememc => self.ememc_value(),
            o if o == f.ememd => {
                let v = self.emem.get(self.emem_pos as usize).copied().unwrap_or(0);
                if self.ememc & EMEMC_AINCR != 0 {
                    self.emem_pos += 1;
                }
                v
            }
            o if o == f.queue_head => self.queue.0,
            o if o == f.queue_tail => self.queue.1,
            o if o == f.msgq_head => self.msgq.0,
            o if o == f.msgq_tail => self.msgq.1,
            o if o == self.d.acr.wpr2_addr_lo => self.wpr2.0,
            o if o == self.d.acr.wpr2_addr_hi => self.wpr2.1,
            o if o == self.d.acr.usable_fb_size_mb => (self.fb_bytes >> 20) as u32,
            o if self.timer_reg(o) => self.timer_read(o),
            // Target mask and lockdown (_kfspIsGspTargetMaskReleased, kern_fsp_gh100.c:1069-1090;
            // _kgspLockdownReleasedOrFmcError, kernel_gsp_gh100.c:500-547).
            o if o == self.d.gsp.base + falcon::HWCFG2 => {
                HWCFG2_RISCV
                    | if running {
                        0
                    } else {
                        HWCFG2_RISCV_BR_PRIV_LOCKDOWN
                    }
            }
            o if o == self.d.gsp_riscv_cpuctl => {
                if running {
                    RISCV_CPUCTL_ACTIVE_STAT
                } else {
                    RISCV_CPUCTL_HALTED
                }
            }
            o if self.in_gsp(o) => self.gsp.read32(o - self.d.gsp.base),
            o => self.constant(o).unwrap_or(0),
        })
    }

    fn write32(&mut self, off: u64, v: u32, mem: &GuestMemory) -> Option<BootWrite> {
        if !self.handles(off) {
            return None;
        }
        let f = &self.d.fsp;
        let mut w = BootWrite::default();
        match off {
            o if o == f.ememc => {
                self.ememc = v;
                self.emem_pos = ((v >> 2) & 0x3F) + ((v >> 8) & 0xFF) * DWORDS_PER_EMEM_BLOCK;
            }
            o if o == f.ememd => {
                if let Some(slot) = self.emem.get_mut(self.emem_pos as usize) {
                    *slot = v;
                }
                if self.ememc & EMEMC_AINCW != 0 {
                    self.emem_pos += 1;
                }
            }
            o if o == f.queue_tail => self.queue.1 = v,
            // The CPU writes HEAD last; it interrupts FSP (kern_fsp_gh100.c:96-109).
            o if o == f.queue_head => {
                self.queue.0 = v;
                w = self.packet(mem);
            }
            o if o == f.msgq_tail => self.msgq.1 = v,
            o if o == f.msgq_head => self.msgq.0 = v,
            o if self.timer_reg(o) => w.notes.extend(self.timer_write(o, v)),
            o if o == self.d.gsp_queue_head => {
                self.gsp.write32(o - self.d.gsp.base, v);
                match self.libos_args_pa {
                    Some(pa) => w.event = Some(BootEvent::CmdQueueDoorbell { libos_args_pa: pa }),
                    None => w.notes.push(
                        "gsp: queue doorbell before the COT boot command; no libos arguments known"
                            .into(),
                    ),
                }
            }
            o if o == self.d.gsp.base + falcon::HWCFG2 || o == self.d.gsp_riscv_cpuctl => {
                w.notes.push("read-only".into())
            }
            o if self.in_gsp(o) => {
                if let Some(falcon::Event::Reset) = self.gsp.write32(o - self.d.gsp.base, v) {
                    w.notes.push(format!("gsp reset {}", self.gsp.resets()));
                }
            }
            _ => w.notes.push("read-only".into()),
        }
        Some(w)
    }
}

#[cfg(test)]
mod tests;

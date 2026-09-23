// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! TLB invalidation of the guest's page directories on the physical GPU (E5b part 2).
//!
//! The host RM's `NV0080_CTRL_CMD_DMA_INVALIDATE_TLB` on a mirror VA space invalidates the
//! PDB of *its walker's* root instance (`gvaspaceInvalidateTlb_IMPL`,
//! `mmu_walk`'s `mmuWalkGetPageLevelInfo`), which for an externally-owned VAS is a root
//! the RM allocated for itself, not the guest root it was given (`pExternalPDB` is only
//! consulted by `gvaspaceGetPageDirBase_IMPL`), and for a replayed VAS the host kernel's
//! own root (exec-formats.md §4.4). So no forwarded control ever invalidated the PDB the
//! mirror channels run on; measured 2026-09-23 (`d-ce-token`): after 88 good copies, the
//! first write into a 2 MiB block the guest had just mapped faulted with `FAULT_PDE` (Xid
//! 31 on our own channel) because the GMMU still held the block's old, invalid PDE.
//!
//! What the guest's own kernel would do on real hardware is what nvidia-uvm does on any
//! channel: a `MEM_OP` `MMU_TLB_INVALIDATE` naming the PDB explicitly
//! (`uvm_hal_turing_host_tlb_invalidate_all`, `uvm_turing_host.c:238-290`). The device
//! therefore keeps one small host channel of its own, the **invalidator**, and pushes that
//! method with the guest's PDB whenever the guest invalidates (its register write, or a
//! `MEM_OP` the software executor ran). Everything the invalidator needs lives in the
//! device-owned carve-out at the top of the reservation (memory-model.md §1: the 2 MiB
//! above the guest's usable heap, where the BAR1/BAR2 roots already are), written through
//! the FB store like everything else in VRAM: a five-level page table mapping one
//! GPFIFO page, one pushbuffer page and one semaphore page at low VAs, plus the USERD.
//! The channel itself is host-exec.md stage C: externally-owned `FERMI_VASPACE_A` +
//! `DMA_SET_PAGE_DIRECTORY` (PRIVILEGED, one CAP_SYS_ADMIN window: the same control the
//! mirror VA spaces use), a TSG on COPY2, `TURING_CHANNEL_GPFIFO_A`, a `TURING_DMA_COPY_A`
//! object (something must be bound before `GPFIFO_SCHEDULE`), the work-submit token and
//! the host doorbell. Every VRAM address it uses is inside the reservation; the PTEs are
//! kind `GENERIC_MEMORY` (0x06) with COMPTAGLINE 0 (D16).

use nvrm_abi::sys;
use nvrm_client::mem::CpuMap;

use crate::fb::FakeFb;
use crate::hostmem::HostMemory;
use crate::ver::active as v;

/// The carve-out the invalidator takes: the top 256 KiB of the reservation.
pub const CARVE_OUT: u64 = 0x40000;
/// Page offsets inside the carve-out.
const PD3: u64 = 0x0000;
const PD2: u64 = 0x1000;
const PD1: u64 = 0x2000;
const PD0: u64 = 0x3000;
const PT: u64 = 0x4000;
const GPFIFO: u64 = 0x5000;
const PUSHBUF: u64 = 0x6000;
const SEMAPHORE: u64 = 0x7000;
const USERD: u64 = 0x8000;
/// The VAs those pages get (PD0 entry 1, small pages 0..2 of its table).
const GPFIFO_VA: u64 = 0x20_0000;
const PUSHBUF_VA: u64 = 0x20_1000;
const SEMAPHORE_VA: u64 = 0x20_2000;
const GPFIFO_ENTRIES: u32 = 512;
/// One push = 16 dwords (64 B) at most: a 4 KiB pushbuffer ring of 64 slots.
const PUSH_SLOT: u64 = 64;
const PUSH_SLOTS: u64 = 4096 / PUSH_SLOT;
/// `NV_MMU_VER2_PTE_KIND` GENERIC_MEMORY (dev_mmu.h; memory-model.md §6).
const KIND_GENERIC: u64 = 0x06;
const VAS_IS_EXTERNALLY_OWNED: u32 = 1 << 3;
const SETPD_ALL_CHANNELS: u32 = 1 << 3;
const CMD_GPFIFO_SCHEDULE: u32 = 0xA06F_0103;
const CMD_GET_WORK_SUBMIT_TOKEN: u32 = 0xC36F_0108;
const DOORBELL_OFFSET: usize = 0x90;
const USERD_GP_GET: u64 = 0x88;
const USERD_GP_PUT: u64 = 0x8C;
const NV_ERR_INVALID_ADDRESS: u32 = 0x1E;
/// `NVC46F_MEM_OP_A..D` (clc46f.h:119-181) and the Volta semaphore methods
/// (`SEM_ADDR_LO` 0x5c .. `SEM_EXECUTE` 0x6c, as caraxes/tools/host-exec.rs).
const MEM_OP_A: u32 = 0x28;
const SEM_ADDR_LO: u32 = 0x5C;
const MEM_OP_A_SYSMEMBAR_EN: u32 = 1 << 11;
const MEM_OP_C_ACK_GLOBALLY: u32 = 1 << 5;
const MEM_OP_D_MMU_TLB_INVALIDATE: u32 = 0x9 << 27;
const SEM_EXECUTE_RELEASE: u32 = 0x1;

fn status_name(st: u32) -> String {
    match nvrm_abi::status_name(st) {
        Some(n) => format!("{st:#x} {n}"),
        None => format!("{st:#x}"),
    }
}

fn as_bytes<T>(p: &mut T) -> &mut [u8] {
    // SAFETY: plain-data RM parameter structs viewed as bytes for the raw escape.
    unsafe { std::slice::from_raw_parts_mut(p as *mut T as *mut u8, std::mem::size_of::<T>()) }
}

/// PDE (PD3/PD2/PD1): aperture 2:1 (1 = VIDEO), address 32:8 = PA >> 12.
fn pde_vid(pa: u64) -> u64 {
    (1 << 1) | ((pa >> 12) << 8)
}

/// PTE (4 KiB, VIDEO): VALID, aperture 0, address 32:8, KIND 63:56.
fn pte_vid(pa: u64) -> u64 {
    1 | ((pa >> 12) << 8) | (KIND_GENERIC << 56)
}

fn inc_hdr(method: u32, count: u32) -> u32 {
    (1 << 29) | (count << 16) | (method >> 2)
}

/// The device's own host channel for TLB invalidates (and nothing else).
pub struct Invalidator {
    at: u64,
    vas: u32,
    tsg: u32,
    channel: u32,
    token: u32,
    doorbell: (u32, CpuMap),
    gp_put: u32,
    seq: u32,
    pub pushes: u64,
    pub slow: u64,
}

impl Invalidator {
    /// Build the invalidator in the carve-out `[end - CARVE_OUT, end)` of the reservation
    /// `[base, end)` (`reservation` is its host handle), through `fb`.
    pub fn new(
        hm: &mut HostMemory,
        fb: &mut FakeFb,
        base: u64,
        end: u64,
        reservation: u32,
        gpfifo_class: u32,
        copy_class: u32,
    ) -> Result<(Self, Vec<String>), String> {
        let mut notes = Vec::new();
        let at = end - CARVE_OUT;
        // The page tables: root -> PD2 -> PD1 -> PD0 (entry 1 = VA 0x200000, small half)
        // -> PT (entries 0..2), then the ring pages, all zeroed first.
        for off in [PD3, PD2, PD1, PD0, PT, GPFIFO, PUSHBUF, SEMAPHORE, USERD] {
            if !fb.write(at + off, &[0u8; 4096]) {
                return Err(format!("carve-out page {:#x} not writable", at + off));
            }
        }
        let w64 = |fb: &mut FakeFb, pa: u64, val: u64| fb.write(pa, &val.to_le_bytes());
        w64(fb, at + PD3, pde_vid(at + PD2));
        w64(fb, at + PD2, pde_vid(at + PD1));
        w64(fb, at + PD1, pde_vid(at + PD0));
        // Dual PDE entry 1: big half 0 (invalid), small half aperture VIDEO + PT >> 12
        // (bits 66:65 and 96:72 of the 128-bit entry: qword 1 bits 2:1 and 32:8).
        w64(fb, at + PD0 + 16, 0);
        w64(fb, at + PD0 + 16 + 8, pde_vid(at + PT));
        w64(fb, at + PT, pte_vid(at + GPFIFO));
        w64(fb, at + PT + 8, pte_vid(at + PUSHBUF));
        w64(fb, at + PT + 16, pte_vid(at + SEMAPHORE));
        fb.flush();

        let device = hm.device();
        let mut vp = sys::NV_VASPACE_ALLOCATION_PARAMETERS::default();
        vp.flags = VAS_IS_EXTERNALLY_OWNED;
        vp.vaSize = 1 << 40;
        vp.bigPageSize = 64 * 1024;
        let vas = hm
            .alloc_object(
                device,
                sys::FERMI_VASPACE_A,
                as_bytes(&mut vp),
                "inv-vaspace",
            )
            .map_err(|st| format!("invalidator FERMI_VASPACE_A: {}", status_name(st)))?;
        let mut sp = [0u8; 32];
        sp[0..8].copy_from_slice(&(at + PD3).to_le_bytes());
        sp[8..12].copy_from_slice(&1u32.to_le_bytes());
        sp[12..16].copy_from_slice(&SETPD_ALL_CHANNELS.to_le_bytes());
        sp[16..20].copy_from_slice(&vas.to_le_bytes());
        sp[24..28].copy_from_slice(&1u32.to_le_bytes());
        let st = hm
            .control_admin(device, v::CMD_DMA_SET_PAGE_DIRECTORY, &mut sp)
            .map_err(|e| format!("invalidator DMA_SET_PAGE_DIRECTORY: {e:#}"))?;
        if st != sys::NV_OK {
            let _ = hm.free_object(vas);
            return Err(format!(
                "invalidator DMA_SET_PAGE_DIRECTORY to {:#x}: {}",
                at + PD3,
                status_name(st)
            ));
        }
        let mut tp = sys::v610::NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS::default();
        tp.hVASpace = vas;
        tp.engineType = v::lookup(v::NV2080_ENGINE_TYPES, "COPY2").unwrap_or(0x0B);
        let tsg = match hm.alloc_object(
            device,
            sys::KEPLER_CHANNEL_GROUP_A,
            as_bytes(&mut tp),
            "inv-tsg",
        ) {
            Ok(h) => h,
            Err(st) => {
                let _ = hm.free_object(vas);
                return Err(format!(
                    "invalidator KEPLER_CHANNEL_GROUP_A: {}",
                    status_name(st)
                ));
            }
        };
        let mut cp = sys::v610::NV_CHANNEL_ALLOC_PARAMS::default();
        cp.gpFifoOffset = GPFIFO_VA;
        cp.gpFifoEntries = GPFIFO_ENTRIES;
        cp.hUserdMemory[0] = reservation;
        cp.userdOffset[0] = at + USERD - base;
        // Allocated under CAP_SYS_ADMIN so the channel's AUTH_LEVEL is ADMIN and the
        // privileged `MEM_OP MMU_TLB_INVALIDATE` method is accepted (hostmem.rs
        // `alloc_object_admin`; a user-level channel RCs with PBDMA METHOD, Xid 32
        // intr 0x00200000, measured 2026-09-23 `d-ce-inval`).
        let mut channel = Err(NV_ERR_INVALID_ADDRESS);
        for _ in 0..4 {
            channel = hm.alloc_object_admin(tsg, gpfifo_class, as_bytes(&mut cp), "inv-channel");
            if !matches!(channel, Err(NV_ERR_INVALID_ADDRESS)) {
                break;
            }
            fb.flush();
        }
        let channel = match channel {
            Ok(h) => h,
            Err(st) => {
                let _ = hm.free_object(tsg);
                let _ = hm.free_object(vas);
                return Err(format!(
                    "invalidator TURING_CHANNEL_GPFIFO_A: {}",
                    status_name(st)
                ));
            }
        };
        let fail = |hm: &mut HostMemory, what: String| {
            let _ = hm.free_object(channel);
            let _ = hm.free_object(tsg);
            let _ = hm.free_object(vas);
            Err(what)
        };
        // NVB0B5_ALLOCATION_PARAMETERS version 1: {version, engineType (NV2080 ordinal)}.
        let mut ce = [0u8; 8];
        ce[0..4].copy_from_slice(&1u32.to_le_bytes());
        ce[4..8].copy_from_slice(&tp.engineType.to_le_bytes());
        if let Err(st) = hm.alloc_object(channel, copy_class, &mut ce, "inv-ce") {
            return fail(
                hm,
                format!("invalidator TURING_DMA_COPY_A: {}", status_name(st)),
            );
        }
        let mut sched = [1u8, 0, 0];
        let st = hm.control(channel, CMD_GPFIFO_SCHEDULE, &mut sched);
        if st != sys::NV_OK {
            return fail(
                hm,
                format!("invalidator GPFIFO_SCHEDULE: {}", status_name(st)),
            );
        }
        let mut tok = [0u8; 4];
        let st = hm.control(channel, CMD_GET_WORK_SUBMIT_TOKEN, &mut tok);
        if st != sys::NV_OK {
            return fail(
                hm,
                format!("invalidator GET_WORK_SUBMIT_TOKEN: {}", status_name(st)),
            );
        }
        let token = u32::from_le_bytes(tok);
        let doorbell = match hm.map_doorbell() {
            Ok(d) => d,
            Err(e) => return fail(hm, format!("invalidator doorbell: {e:#}")),
        };
        // The host RM cleared the USERD at alloc; the ring starts at 0 either way.
        fb.write(at + USERD + USERD_GP_GET, &0u32.to_le_bytes());
        fb.write(at + USERD + USERD_GP_PUT, &0u32.to_le_bytes());
        fb.flush();
        notes.push(format!(
            "invalidator: device channel {channel:#x} (TSG {tsg:#x} on COPY2, VA space {vas:#x} rooted at {:#x} in the carve-out [{at:#x}, {end:#x}); gpfifo VA {GPFIFO_VA:#x} x {GPFIFO_ENTRIES}, pushbuffer VA {PUSHBUF_VA:#x}, semaphore VA {SEMAPHORE_VA:#x}, USERD {:#x}; token {token:#x} (host chid {}, runlist {}) [host; VRAM inside the reservation, kind 0x06]",
            at + PD3,
            at + USERD,
            token & 0xFFF,
            (token >> 16) & 0x7F
        ));
        Ok((
            Invalidator {
                at,
                vas,
                tsg,
                channel,
                token,
                doorbell,
                gp_put: 0,
                seq: 0,
                pushes: 0,
                slow: 0,
            },
            notes,
        ))
    }

    /// Invalidate every TLB entry of page directory `pdb` (VIDMEM) on the physical GPU:
    /// `MEM_OP` `MMU_TLB_INVALIDATE`, PDB_ONE, all page-table levels, GPC enabled,
    /// ACK GLOBALLY, with a SYSMEMBAR (the guest asked for one with its register write);
    /// then a host semaphore release the device polls so the caller knows the invalidate
    /// has been acknowledged before the next doorbell.
    pub fn invalidate(&mut self, fb: &mut FakeFb, pdb: u64) -> Result<String, String> {
        let t0 = std::time::Instant::now();
        self.seq = self.seq.wrapping_add(1);
        let seq = self.seq;
        let pdb12 = pdb >> 12;
        let push: [u32; 11] = [
            inc_hdr(MEM_OP_A, 4),
            MEM_OP_A_SYSMEMBAR_EN,
            0,
            ((pdb12 as u32) << 12) | MEM_OP_C_ACK_GLOBALLY,
            MEM_OP_D_MMU_TLB_INVALIDATE | ((pdb12 >> 20) as u32 & 0x7FF_FFFF),
            inc_hdr(SEM_ADDR_LO, 5),
            SEMAPHORE_VA as u32,
            (SEMAPHORE_VA >> 32) as u32,
            seq,
            0,
            SEM_EXECUTE_RELEASE,
        ];
        let slot = (self.gp_put as u64) % PUSH_SLOTS;
        let pb_pa = self.at + PUSHBUF + slot * PUSH_SLOT;
        let pb_va = PUSHBUF_VA + slot * PUSH_SLOT;
        let mut bytes = Vec::with_capacity(push.len() * 4);
        for d in push {
            bytes.extend_from_slice(&d.to_le_bytes());
        }
        if !fb.write(pb_pa, &bytes) {
            return Err(format!("pushbuffer slot {pb_pa:#x} not writable"));
        }
        // GP entry: GET 31:2 | GET_HI 7:0, LENGTH 30:10 in dwords (clc46f.h:264-282).
        let e0 = (pb_va as u32) & !3;
        let e1 = ((pb_va >> 32) as u32 & 0xFF) | ((push.len() as u32) << 10);
        let idx = self.gp_put % GPFIFO_ENTRIES;
        fb.write(self.at + GPFIFO + 8 * idx as u64, &e0.to_le_bytes());
        fb.write(self.at + GPFIFO + 8 * idx as u64 + 4, &e1.to_le_bytes());
        self.gp_put = (self.gp_put + 1) % GPFIFO_ENTRIES;
        fb.write(self.at + USERD + USERD_GP_PUT, &self.gp_put.to_le_bytes());
        fb.flush();
        self.doorbell.1.write_u32(DOORBELL_OFFSET, self.token);
        self.pushes += 1;
        // Wait for the release (the PBDMA executes the MEM_OP, waits for the global ack,
        // then releases): at most 200 ms, spinning through the FB store.
        let mut sem = [0u8; 4];
        loop {
            fb.read(self.at + SEMAPHORE, &mut sem);
            if u32::from_le_bytes(sem) == seq {
                break;
            }
            if t0.elapsed() > std::time::Duration::from_millis(200) {
                self.slow += 1;
                let mut get = [0u8; 4];
                fb.read(self.at + USERD + USERD_GP_GET, &mut get);
                return Err(format!(
                    "invalidate of pdb {pdb:#x} not acknowledged in 200 ms (push {seq}, GPGet {} GPPut {})",
                    u32::from_le_bytes(get),
                    self.gp_put
                ));
            }
            std::hint::spin_loop();
        }
        Ok(format!(
            "invalidator: MEM_OP MMU_TLB_INVALIDATE pdb {pdb:#x} (all levels, GPC, ack globally, sysmembar) done in {} us (push {seq}) [host GPU]",
            t0.elapsed().as_micros()
        ))
    }

    /// Free the host objects (channel, TSG, VA space, the doorbell object).
    pub fn free(self, hm: &mut HostMemory) -> String {
        let (h, map) = self.doorbell;
        drop(map);
        let mut r = Vec::new();
        for (what, handle) in [
            ("channel", self.channel),
            ("tsg", self.tsg),
            ("vaspace", self.vas),
            ("doorbell", h),
        ] {
            if let Err(st) = hm.free_object(handle) {
                r.push(format!("{what} {handle:#x}: {}", status_name(st)));
            }
        }
        format!(
            "invalidator freed after {} push(es), {} slow; {}",
            self.pushes,
            self.slow,
            if r.is_empty() {
                "all NV_OK".into()
            } else {
                r.join(", ")
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings_match_the_headers() {
        // A 4-method incrementing header on MEM_OP_A, subchannel 0.
        assert_eq!(inc_hdr(MEM_OP_A, 4), 0x2004_000A);
        // PDE/PTE fields (dev_mmu.h): aperture VIDEO, address >> 12 at bit 8.
        assert_eq!(pde_vid(0x1e8fc1000), (1 << 1) | (0x1e8fc1 << 8));
        assert_eq!(pte_vid(0x1e8fc5000) >> 56, KIND_GENERIC);
        assert_eq!(pte_vid(0x1e8fc5000) & 0xFF, 1);
        // MEM_OP_D operation field 31:27 = MMU_TLB_INVALIDATE (9).
        assert_eq!(MEM_OP_D_MMU_TLB_INVALIDATE >> 27, 9);
        const _: () = assert!(CARVE_OUT >= USERD + 4096);
    }
}

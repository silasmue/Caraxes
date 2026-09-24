// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The boot model: everything behind BAR0 once `--boot-model` is on. The chip's
//! firmware-boot model ([`crate::chip::FirmwareBoot`]; Turing: GFW, FWSEC, Booter,
//! the falcons, WPR2) produces the CPU-visible side effects of a boot that never runs
//! firmware; this generic part owns the kernel-RM surface the chip data locates (the
//! PRAMIN window, the BAR blocks, TLB invalidate, the interrupt tree, the VF pointer
//! registers and the usermode doorbell) and, once the firmware boot hands over the
//! WPR meta and rings the command-queue doorbell, the GSP message queues and the RPC
//! service (docs/caraxes/gsp-rpc-protocol.md, docs/caraxes/phase5-rpcs.md). Message
//! layouts are the emulated driver version's ([`crate::ver`]).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::chip::{BootEvent, Chip, ChipData, FirmwareBoot, Registers};
use crate::ctrl::{Dispatcher, HostRm};
use crate::fb::{FakeFb, FbLayout, Target, VramStore};
use crate::guestmem::GuestMemory;
use crate::hostmem::HostMemory;
use crate::intr::{IntrShared, IntrTree};
use crate::mirror::Mirror;
use crate::msgq::{QueueMemory, QueueState, RpcHeader};
use crate::rpc::{self, StaticIdentity, WprLayout};
use crate::ver::active as v;

/// A register of the kernel-RM surface this model owns (offsets from the chip).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reg {
    Bar0Window,
    Bar1Block,
    Bar2Block,
    MmuInvalidatePdb,
    MmuInvalidateUpperPdb,
    MmuInvalidate,
    UsermodeDoorbell,
    /// An L2 cache-operation register (`NV_UFLUSH_*` and the VF-window aliases).
    L2Op(L2Op),
}

/// What an L2 cache-operation register does when the guest starts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum L2Op {
    /// `L2_FLUSH_DIRTY`: write back every dirty line.
    FlushDirty,
    /// `L2_SYSMEM_INVALIDATE`: invalidate the lines caching system memory.
    SysmemInvalidate,
    /// FB_FLUSH, PEERMEM_INVALIDATE, CLEAN_COMPTAGS: completed without a host operation.
    Local,
}

/// `_PENDING` (0:0) and `_OUTSTANDING` (1:1) of every `NV_UFLUSH_*` cache-operation
/// register (gm200/dev_flush.h:26-54): the RM writes `PENDING_BUSY` to start the
/// operation and polls both bits until they read 0 (kmemsysDoCacheOp_GM107).
const L2_OP_BUSY: u32 = 0b11;

/// `NV_RAMUSERD` is 512 bytes (dev_ram.ref; 8 channels per 4 KiB page): what GSP-RM
/// clears when it takes over a client-provided USERD.
const USERD_SIZE: usize = 512;

pub struct BootModel {
    chip: &'static dyn Chip,
    regs: Registers,
    /// The chip's firmware-boot model (falcons, fuses, GFW, WPR2 on Turing).
    fw: Box<dyn FirmwareBoot>,
    /// Which FB addresses the guest uses (fb.rs; docs/caraxes/memory-model.md).
    layout: FbLayout,
    identity: StaticIdentity,
    wpr_meta_pa: Option<u64>,
    wpr: Option<WprLayout>,
    libos_args_pa: Option<u64>,
    doorbells: u32,
    /// The queue page table and state once the first doorbell revealed RMARGS.
    pages: Vec<u64>,
    queues: Option<QueueState>,
    /// The `GspSystemInfo` the CPU sent (kept for E4).
    pub system_info: Option<Vec<u8>>,
    /// Requests the device could not answer, for the trace summary.
    pub unanswered: Vec<String>,
    /// A request split over CONTINUATION_RECORDs that is still being assembled.
    pending: Option<(crate::msgq::Message, usize)>,
    /// Phase-5 answers (docs/caraxes/phase5-rpcs.md).
    pub ctrl: Dispatcher,
    /// The fake framebuffer (docs/caraxes/fb-model.md).
    pub fb: FakeFb,
    /// `NV_PBUS_BAR0_WINDOW`: BASE[23:0] in 64 KiB units, TARGET[25:24].
    bar0_window: u32,
    mmu_invalidate: (u32, u32, u32),
    /// Last value written to each L2 cache-operation register (busy bits masked on read).
    l2_ops: BTreeMap<u64, u32>,
    /// L2 cache operations started / forwarded to the host / host failures (trace).
    pub l2_op_counts: (u64, u64, u64),
    bar2_block: u32,
    pub bar_faults: u32,
    /// The CPU interrupt tree (MSI source).
    pub intr: IntrTree,
    /// Fault-buffer and access-counter pointers, usermode timer (E5a, uvm.rs).
    pub vf: crate::uvm::VfRegs,
    /// Execution on the physical GPU (E5b part 2): the host memory client whose
    /// reservation backs the FB, and the vectors the event thread raises. Kept across
    /// a reset, which rebuilds the store and the mirror.
    exec: Option<(Arc<Mutex<HostMemory>>, Arc<IntrShared>)>,
}

impl BootModel {
    /// The model for `layout`: the firmware boot reports its address space as the FB
    /// size (`LOCAL_MEMORY_RANGE`), the fake FB spans it, and the static info takes the
    /// region table and the BAR roots from it.
    pub fn new(chip: &'static dyn Chip, layout: FbLayout, identity: StaticIdentity) -> Self {
        let d: &'static ChipData = chip.data();
        let r = d.registers;
        let fb_bytes = layout.addr_space;
        BootModel {
            chip,
            regs: r,
            fw: chip.firmware_boot(fb_bytes),
            layout,
            ctrl: Dispatcher::new(d, &identity, None),
            identity,
            wpr_meta_pa: None,
            wpr: None,
            libos_args_pa: None,
            doorbells: 0,
            pages: Vec::new(),
            queues: None,
            system_info: None,
            unanswered: Vec::new(),
            pending: None,
            fb: FakeFb::for_layout(&layout),
            // Reset value as the VBIOS leaves it: the last MiB of FB (fb-model.md §4c).
            bar0_window: ((fb_bytes - (1 << 20)) >> 16) as u32,
            mmu_invalidate: (0, 0, 0),
            l2_ops: BTreeMap::new(),
            l2_op_counts: (0, 0, 0),
            bar2_block: 0,
            bar_faults: 0,
            intr: IntrTree::new(r.intr_tree, r.intr_tree_size),
            vf: crate::uvm::VfRegs::new(d),
            exec: None,
        }
    }

    /// Execute on the physical GPU (E5b part 2): the FB is backed by the reserved VRAM
    /// through BAR1 windows ([`VramStore`]), the host mirrors of the guest's channels
    /// live in the dispatcher ([`Mirror`]), and the vectors of the host's non-stall
    /// events reach the interrupt tree through `shared`.
    pub fn with_exec(
        mut self,
        hm: Arc<Mutex<HostMemory>>,
        shared: Arc<IntrShared>,
    ) -> Result<Self, String> {
        let (base, end) = self
            .layout
            .reservation
            .ok_or("--host-exec needs the VRAM reservation")?;
        self.fb = FakeFb::for_layout(&self.layout).with_store(Box::new(VramStore::new(
            Box::new(hm.clone()),
            base,
            end,
        )));
        let r = self.regs;
        self.intr = IntrTree::new(r.intr_tree, r.intr_tree_size).with_shared(shared.clone());
        self.ctrl.mirror = Some(Mirror::new(hm.clone(), shared.clone(), self.chip.data())?);
        self.exec = Some((hm, shared));
        Ok(self)
    }

    /// True once when the interrupt tree asserted; the device then signals MSI.
    pub fn take_msi(&mut self) -> bool {
        self.intr.take_msi()
    }

    /// What the FB store and the mirror did, for the exit summary.
    pub fn exec_summary(&self) -> Vec<String> {
        let mut s = Vec::new();
        if let Some(st) = self.fb.vram_stats() {
            s.push(format!(
                "vram store: {} window map(s), {} refused, {} eviction(s); {} read(s) of {} bytes, {} write(s) of {} bytes",
                st.maps, st.map_failures, st.evictions, st.reads, st.bytes_read, st.writes, st.bytes_written
            ));
        }
        if let Some(m) = &self.ctrl.mirror {
            s.push(m.summary());
        }
        s
    }

    /// Root page directory for a trapped BAR access: BAR3 is the BAR2 aperture
    /// rooted at `bar2PdeBase`, BAR1 at `bar1PdeBase`: in the reservation's carve-out
    /// when there is one, else in the non-WPR heap.
    fn bar_root(&self, bar: u32) -> Option<u64> {
        if let Some((bar2, bar1)) = self.layout.bar_roots() {
            return match bar {
                3 => Some(bar2),
                1 => Some(bar1),
                _ => None,
            };
        }
        let heap = self.wpr.as_ref()?.non_wpr_heap_offset;
        match bar {
            3 => Some(heap),
            1 => Some(heap + 0x1000),
            _ => None,
        }
    }

    /// A trapped BAR1/BAR3 access: walk the guest's page tables in the fake FB.
    pub fn bar_access(
        &mut self,
        mem: &GuestMemory,
        bar: u32,
        offset: u64,
        data: &mut [u8],
        write: bool,
    ) -> String {
        let Some(root) = self.bar_root(bar) else {
            if !write {
                data.fill(0);
            }
            return "no root yet".into();
        };
        let t = self.fb.bar_access(mem, root, offset, data, write);
        match t {
            Target::Fb(pa) => format!("fb {pa:#x}"),
            Target::Sys(pa) => format!("sys {pa:#x}"),
            Target::Sparse => "sparse".into(),
            Target::Unmapped { level } => {
                self.bar_faults += 1;
                format!("UNMAPPED (level {level})")
            }
        }
    }

    /// Give the device a host RM connection for the forwardable RPCs.
    pub fn with_host(mut self, host: Box<dyn HostRm>) -> Self {
        self.ctrl = Dispatcher::new(self.chip.data(), &self.identity, Some(host));
        self
    }

    pub fn chip(&self) -> &'static dyn Chip {
        self.chip
    }

    /// The FB address space the guest is told (the profile size without a reservation).
    pub fn fb_bytes(&self) -> u64 {
        self.layout.addr_space
    }

    pub fn layout(&self) -> &FbLayout {
        &self.layout
    }

    pub fn identity(&self) -> &StaticIdentity {
        &self.identity
    }

    pub fn wpr(&self) -> Option<&WprLayout> {
        self.wpr.as_ref()
    }

    /// Back to the post-GFW state, as a function-level reset would leave the card.
    /// The host RM connection survives.
    pub fn reset(&mut self) {
        let host = self.ctrl.take_host();
        let exec = self.exec.take();
        // The old mirror frees its host objects when the old model drops (Mirror::drop).
        let mut fresh = BootModel::new(self.chip, self.layout, self.identity.clone());
        if let Some(h) = host {
            fresh.ctrl = Dispatcher::new(self.chip.data(), &self.identity, Some(h));
        }
        if let Some((hm, shared)) = exec {
            // Cannot fail once it succeeded (the layout's reservation is unchanged).
            fresh = match fresh.with_exec(hm, shared) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("vfio-user-nvgpu: reset: host execution not re-attached: {e}");
                    BootModel::new(self.chip, self.layout, self.identity.clone())
                }
            };
        }
        *self = fresh;
    }

    pub fn libos_args_pa(&self) -> Option<u64> {
        self.libos_args_pa
    }

    pub fn wpr_meta_pa(&self) -> Option<u64> {
        self.wpr_meta_pa
    }

    pub fn doorbells(&self) -> u32 {
        self.doorbells
    }

    /// The kernel-RM surface register at `off`, if it is one.
    fn reg(&self, off: u64) -> Option<Reg> {
        let r = &self.regs;
        Some(match off {
            o if o == r.bar0_window => Reg::Bar0Window,
            o if o == r.bar1_block => Reg::Bar1Block,
            o if o == r.bar2_block => Reg::Bar2Block,
            o if o == r.mmu_invalidate_pdb => Reg::MmuInvalidatePdb,
            o if o == r.mmu_invalidate_upper_pdb => Reg::MmuInvalidateUpperPdb,
            o if o == r.mmu_invalidate => Reg::MmuInvalidate,
            o if o == r.usermode_doorbell => Reg::UsermodeDoorbell,
            o if o == r.l2_flush_dirty => Reg::L2Op(L2Op::FlushDirty),
            o if r.l2_sysmem_invalidate.contains(&o) => Reg::L2Op(L2Op::SysmemInvalidate),
            o if r.l2_ops_local.contains(&o) => Reg::L2Op(L2Op::Local),
            _ => return None,
        })
    }

    /// Start the L2 cache operation `op` (the guest wrote `PENDING_BUSY`). With host
    /// execution on, the guest's work runs on the physical GPU and its sysmem lines sit
    /// in the physical L2, so FLUSH_DIRTY and SYSMEM_INVALIDATE are honoured there with
    /// `FB_FLUSH_GPU_CACHE` before the register reports completion. Without host
    /// execution there is no cache behind the model and the operation completes at once.
    /// A host failure is logged and still reported complete: the guest RM cannot act on
    /// a timeout either (it logs and continues), and a wedged poll costs 30 s per op.
    fn l2_op(&mut self, op: L2Op) -> String {
        use crate::ver::active::{FLUSH_GPU_CACHE_INVALIDATE, FLUSH_GPU_CACHE_WRITE_BACK};
        self.l2_op_counts.0 += 1;
        let flags = match op {
            L2Op::FlushDirty => FLUSH_GPU_CACHE_WRITE_BACK,
            // The host RM promotes a sysmem invalidate to an evict (kmemsysCacheOp_GM200).
            L2Op::SysmemInvalidate => FLUSH_GPU_CACHE_INVALIDATE,
            L2Op::Local => return format!("l2 {op:?}: complete (no host operation)"),
        };
        if self.exec.is_none() {
            return format!("l2 {op:?}: complete (no host execution, no cache behind the model)");
        }
        let t = std::time::Instant::now();
        match self.ctrl.host_flush_gpu_cache(flags) {
            Some(0) => {
                self.l2_op_counts.1 += 1;
                format!(
                    "l2 {op:?}: FB_FLUSH_GPU_CACHE flags {:#x} NV_OK in {} us [host GPU]",
                    flags,
                    t.elapsed().as_micros()
                )
            }
            Some(st) => {
                self.l2_op_counts.2 += 1;
                let m = format!(
                    "l2 {op:?}: FB_FLUSH_GPU_CACHE flags {flags:#x} failed: status {st:#x}; reported complete"
                );
                eprintln!("vfio-user-nvgpu: {m}");
                m
            }
            None => format!("l2 {op:?}: complete (no host RM connection)"),
        }
    }

    fn in_pramin(&self, off: u64) -> bool {
        (self.regs.pramin..self.regs.pramin + self.regs.pramin_size).contains(&off)
    }

    fn handles(&self, off: u64) -> bool {
        self.reg(off).is_some()
            || self.vf.handles(off)
            || self.in_pramin(off)
            || self.intr.handles(off)
            || self.fw.handles(off)
    }

    fn pramin_pa(&self, off: u64) -> u64 {
        (((self.bar0_window & 0xFF_FFFF) as u64) << 16) + (off - self.regs.pramin)
    }

    /// Value of the dword-aligned register `off`, or `None` when the model does not own it.
    pub fn read32(&mut self, off: u64) -> Option<u32> {
        if !self.handles(off) {
            return None;
        }
        if self.intr.handles(off) {
            return Some(self.intr.read32(off));
        }
        if self.in_pramin(off) {
            let mut b = [0u8; 4];
            if (self.bar0_window >> 24) & 3 == 0 {
                self.fb.read(self.pramin_pa(off), &mut b);
            }
            return Some(u32::from_le_bytes(b));
        }
        if self.vf.handles(off) {
            return Some(self.vf.read32(off, self.ctrl.channels.now_ns()));
        }
        Some(match self.reg(off) {
            Some(Reg::Bar0Window) => self.bar0_window,
            Some(Reg::Bar1Block) => 0,
            Some(Reg::Bar2Block) => self.bar2_block,
            Some(Reg::MmuInvalidatePdb) => self.mmu_invalidate.0,
            Some(Reg::MmuInvalidateUpperPdb) => self.mmu_invalidate.1,
            // TRIGGER (bit 31) always reads back as done (fb-model.md §4d).
            Some(Reg::MmuInvalidate) => self.mmu_invalidate.2 & !(1 << 31),
            // Write-only doorbell (`clc361.h:33`); reads return 0.
            Some(Reg::UsermodeDoorbell) => 0,
            // The operation completed when the write started it: PENDING and
            // OUTSTANDING read EMPTY/FALSE (execution.md §6, "L2 cache operations").
            Some(Reg::L2Op(_)) => self.l2_ops.get(&off).copied().unwrap_or(0) & !L2_OP_BUSY,
            None => return self.fw.read32(off),
        })
    }

    /// Write the dword-aligned register `off`. Returns `None` when the model does not
    /// own it, otherwise the notes the trace should carry (empty for plain writes).
    pub fn write32(&mut self, off: u64, v: u32, mem: &GuestMemory) -> Option<Vec<String>> {
        if !self.handles(off) {
            return None;
        }
        let mut notes = Vec::new();
        if let Some((_, shared)) = &self.exec {
            notes.extend(shared.take_notes());
        }
        if self.intr.handles(off) {
            notes.extend(self.intr.write32(off, v));
            return Some(notes);
        }
        if self.in_pramin(off) {
            if (self.bar0_window >> 24) & 3 == 0 {
                let pa = self.pramin_pa(off);
                if !self.fb.write(pa, &v.to_le_bytes()) {
                    notes.push(format!("pramin write beyond FB: {pa:#x}"));
                }
            } else {
                notes.push(format!(
                    "pramin write with TARGET {} dropped",
                    (self.bar0_window >> 24) & 3
                ));
            }
            return Some(notes);
        }
        if self.vf.handles(off) {
            notes.extend(self.vf.write32(off, v));
            return Some(notes);
        }
        match self.reg(off) {
            Some(Reg::Bar0Window) => self.bar0_window = v,
            Some(Reg::Bar2Block) => self.bar2_block = v,
            Some(Reg::MmuInvalidatePdb) => self.mmu_invalidate.0 = v,
            Some(Reg::MmuInvalidateUpperPdb) => self.mmu_invalidate.1 = v,
            Some(Reg::Bar1Block) => notes.push("read-only".into()),
            Some(Reg::L2Op(op)) => {
                self.l2_ops.insert(off, v);
                if v & 1 != 0 {
                    notes.push(self.l2_op(op));
                }
            }
            Some(Reg::UsermodeDoorbell) => {
                // A channel's work submit token (kernel_fifo_tu102.c:75-130): a mirrored
                // channel rings the host doorbell (E5b part 2), the others run their
                // pending GPFIFO entries against the FB store and guest RAM (E5a).
                let mut gm = crate::channel::GpuMem {
                    fb: &mut self.fb,
                    sys: mem,
                };
                notes.extend(self.ctrl.doorbell(&mut gm, v));
            }
            Some(Reg::MmuInvalidate) => {
                self.mmu_invalidate.2 = v;
                if v & (1 << 31) != 0 {
                    // NV_PFB_PRI_MMU_INVALIDATE_PDB: ADDR 31:4 in 4 KiB units, APERTURE
                    // 1:0; UPPER_PDB adds bits 51:32 (fb-model.md §4d).
                    let pdb = ((self.mmu_invalidate.0 as u64 >> 4) << 12)
                        | ((self.mmu_invalidate.1 as u64 & 0xF_FFFF) << 32);
                    // The guest changed page tables in VRAM through BAR1; drain the WC
                    // buffers before the host TLB invalidate so RM re-walks fresh tables.
                    self.fb.flush();
                    let forwarded = self.ctrl.mirror_invalidate(&mut self.fb, Some(pdb));
                    // Re-sync the GR shadow VAS from the guest root: the guest maps set-A
                    // pages (e.g. the kernel's I2M upload targets) and then invalidates
                    // before the GR push touches them, so a page mapped since the last
                    // walk must be mirrored now or the GR engine MMU-faults (Xid 31).
                    {
                        let mut gm = crate::channel::GpuMem {
                            fb: &mut self.fb,
                            sys: mem,
                        };
                        // ALL_PDB (bit 1) invalidates every VAS: resync every shadow.
                        let which = if v & 2 != 0 { None } else { Some(pdb) };
                        notes.extend(self.ctrl.mirror_shadow_resync(&mut gm, which));
                    }
                    notes.push(format!(
                        "mmu invalidate: pdb {pdb:#x} flags {:#x}{}",
                        v & 0x7FFF_FFFF,
                        if forwarded > 0 {
                            format!(" -> invalidated on {forwarded} mirror VA space(s) [host]")
                        } else {
                            " (no mirror VA space rooted there; nothing cached here)".into()
                        }
                    ));
                }
            }
            None => {
                let w = self.fw.write32(off, v, mem)?;
                notes.extend(w.notes);
                match w.event {
                    Some(BootEvent::WprMeta { pa }) => {
                        self.wpr_meta_pa = Some(pa);
                        match WprLayout::read(mem, pa) {
                            Ok(w) => {
                                notes.push(format!(
                                    "wpr meta: fbSize {:#x} gspFwRsvdStart {:#x} nonWprHeap {:#x}+{:#x} wprStart {:#x} frtsOffset {:#x} wprEnd {:#x} vgaWorkspace {:#x}",
                                    w.fb_size, w.gsp_fw_rsvd_start, w.non_wpr_heap_offset, w.non_wpr_heap_size, w.gsp_fw_wpr_start, w.frts_offset, w.gsp_fw_wpr_end, w.vga_workspace_offset
                                ));
                                if let Some((base, end)) = self.layout.reservation {
                                    // The fake GSP's WPR must lie above the reservation:
                                    // the guest never hands those addresses to an engine,
                                    // and the host card keeps its own WPR there.
                                    notes.push(if w.gsp_fw_rsvd_start >= end && w.fb_size == self.layout.addr_space {
                                        format!("fb layout: guest WPR [{:#x}, {:#x}) above the reservation [{base:#x}, {end:#x}): ok", w.gsp_fw_rsvd_start, w.fb_size)
                                    } else {
                                        format!("fb layout: ERROR guest WPR [{:#x}, {:#x}) overlaps the reservation [{base:#x}, {end:#x}) or fbSize differs from {:#x}", w.gsp_fw_rsvd_start, w.fb_size, self.layout.addr_space)
                                    });
                                }
                                self.wpr = Some(w);
                            }
                            Err(e) => notes.push(format!("wpr meta: {e}")),
                        }
                    }
                    Some(BootEvent::CmdQueueDoorbell { libos_args_pa: pa }) => {
                        self.doorbells += 1;
                        self.libos_args_pa = Some(pa);
                        notes.push(format!(
                            "doorbell {}: libos args PA {pa:#x} ({}), wpr meta PA {}",
                            self.doorbells,
                            if mem.covers(pa, 4096) {
                                "mapped"
                            } else {
                                "NOT MAPPED"
                            },
                            self.wpr_meta_pa
                                .map_or("unknown".to_string(), |p| format!("{p:#x}")),
                        ));
                        notes.extend(self.service_queues(mem, pa));
                    }
                    None => {}
                }
            }
        }
        Some(notes)
    }

    /// Find RMARGS through the libos init arguments, link the queues on the first
    /// doorbell, then answer everything in the command queue.
    fn service_queues(&mut self, mem: &GuestMemory, libos_args_pa: u64) -> Vec<String> {
        let mut notes = Vec::new();
        if self.queues.is_none() {
            match self.link_queues(mem, libos_args_pa) {
                Ok(n) => notes.extend(n),
                Err(e) => {
                    notes.push(format!("queues: {e}"));
                    return notes;
                }
            }
        }
        let pages = std::mem::take(&mut self.pages);
        let mut queues = self.queues.take().expect("linked");
        {
            let qm = QueueMemory::new(mem, &pages);
            loop {
                let mut msg = match queues.receive(&qm) {
                    Ok(Some(m)) => m,
                    Ok(None) => break,
                    Err(e) => {
                        notes.push(format!("receive: {e}"));
                        break;
                    }
                };
                // Large RPCs arrive as a first record plus CONTINUATION_RECORDs whose
                // payloads continue the buffer (rpc.c:2058-2244); reassemble them.
                if msg.header.function == rpc::FN_CONTINUATION_RECORD {
                    if let Some((mut first, needed)) = self.pending.take() {
                        first.payload.extend_from_slice(&msg.payload);
                        if first.payload.len() < needed {
                            notes.push(format!(
                                "continuation: {}/{} bytes",
                                first.payload.len(),
                                needed
                            ));
                            self.pending = Some((first, needed));
                            continue;
                        }
                        notes.push(format!(
                            "continuation: request complete, {} bytes",
                            first.payload.len()
                        ));
                        msg = first;
                    } else {
                        notes.push("continuation record without a pending request".into());
                        continue;
                    }
                } else {
                    let size_at = |o: usize| {
                        u32::from_le_bytes(msg.payload[o..o + 4].try_into().unwrap()) as usize
                    };
                    let needed = match msg.header.function {
                        rpc::FN_GSP_RM_CONTROL
                            if msg.payload.len() >= v::RM_CONTROL_PARAMS_SIZE + 4 =>
                        {
                            v::RM_CONTROL_PARAMS + size_at(v::RM_CONTROL_PARAMS_SIZE)
                        }
                        rpc::FN_GSP_RM_ALLOC
                            if msg.payload.len() >= v::RM_ALLOC_PARAMS_SIZE + 4 =>
                        {
                            v::RM_ALLOC_PARAMS + size_at(v::RM_ALLOC_PARAMS_SIZE)
                        }
                        _ => 0,
                    };
                    // Only a first record at the maxRpcSize cap can be split (rpc.c:2082-2092);
                    // client allocations legitimately carry fewer bytes than paramsSize.
                    if msg.header.length as usize >= v::MAX_RPC_SIZE && needed > msg.payload.len() {
                        notes.push(format!(
                            "large request {}: {} of {needed} bytes, waiting for continuation records",
                            rpc::function_name(msg.header.function),
                            msg.payload.len()
                        ));
                        self.pending = Some((msg, needed));
                        continue;
                    }
                }
                let h = msg.header;
                notes.push(format!(
                    "rx seq {} {} ({}) sequence {} length {} checksum {}: {}",
                    msg.seq_num,
                    rpc::function_name(h.function),
                    h.function,
                    h.sequence,
                    h.length,
                    if msg.checksum_ok { "ok" } else { "BAD" },
                    rpc::describe(h.function, &msg.payload)
                ));
                let reply: Option<(u32, Vec<u8>)> = match h.function {
                    rpc::FN_GSP_SET_SYSTEM_INFO => {
                        // BAR addresses for BUS_GET_PCI_BAR_INFO come from here.
                        if let Some(bars) = rpc::system_info_bar_addresses(&msg.payload) {
                            self.ctrl.bar_offsets = bars;
                        }
                        self.system_info = Some(msg.payload.clone());
                        None
                    }
                    rpc::FN_SET_REGISTRY => None,
                    rpc::FN_SET_GUEST_SYSTEM_INFO => {
                        // The device emulates one driver version (gsp-abi-610.md (e)); a
                        // guest that announces another is noted, not refused, so its
                        // failure shows up where its layouts first differ.
                        let guest = rpc::guest_driver_version(&msg.payload);
                        if guest != v::DRIVER_VERSION {
                            notes.push(format!(
                                "guest driver {guest:?} is not the emulated {} (build with its ver module)",
                                v::DRIVER_VERSION
                            ));
                        }
                        Some((rpc::NV_OK, msg.payload.clone()))
                    }
                    rpc::FN_GET_GSP_STATIC_INFO => {
                        let (info, labels) = rpc::build_static_info(
                            self.chip.data(),
                            &self.identity,
                            self.wpr.as_ref(),
                            &self.layout,
                        );
                        for l in labels {
                            notes.push(format!(
                                "static-info {} = {} [{}]",
                                l.field, l.value, l.source
                            ));
                        }
                        Some((rpc::NV_OK, info))
                    }
                    rpc::FN_GSP_RM_CONTROL => {
                        // rpc_gsp_rm_control_v03_00 (docs/caraxes/gsp-abi-610.md).
                        let p = &msg.payload;
                        let w =
                            |o: usize| u32::from_le_bytes(p[o..o + 4].try_into().unwrap_or([0; 4]));
                        let (h_object, cmd, size) = (
                            w(v::RM_CONTROL_H_OBJECT),
                            w(v::RM_CONTROL_CMD),
                            w(v::RM_CONTROL_PARAMS_SIZE) as usize,
                        );
                        let at = v::RM_CONTROL_PARAMS;
                        let params = p.get(at..at + size).unwrap_or(&[]);
                        let a = self
                            .ctrl
                            .control(w(v::RM_CONTROL_H_CLIENT), h_object, cmd, params);
                        notes.push(format!(
                            "ctrl {cmd:#x}: {} -> status {:#x} [{}]",
                            a.note, a.status, a.label
                        ));
                        if a.label == "unanswered" {
                            self.unanswered.push(a.note.clone());
                        }
                        let mut reply = p[..at.min(p.len())].to_vec();
                        reply.resize(at, 0);
                        let st = v::RM_CONTROL_STATUS;
                        reply[st..st + 4].copy_from_slice(&a.status.to_le_bytes());
                        if cmd & v::CMD_GSS_LEGACY_BIT != 0 {
                            // GSS legacy: rmctrlFlags 0, or the guest caches the reply
                            // (rpc.c:10902-10907, phase5-rpcs.md §2).
                            let f = v::RM_CONTROL_RMCTRL_FLAGS;
                            reply[f..f + 4].copy_from_slice(&0u32.to_le_bytes());
                        }
                        let mut out = a.params;
                        out.resize(size, 0);
                        reply.extend_from_slice(&out);
                        Some((rpc::NV_OK, reply))
                    }
                    rpc::FN_GSP_RM_ALLOC => {
                        // rpc_gsp_rm_alloc_v03_00 (docs/caraxes/gsp-abi-610.md). The alloc
                        // status goes into rpc_result as well (gsp-rpc-protocol.md §5).
                        let p = &msg.payload;
                        let w =
                            |o: usize| u32::from_le_bytes(p[o..o + 4].try_into().unwrap_or([0; 4]));
                        let (h_parent, h_object, h_class, size) = (
                            w(v::RM_ALLOC_H_PARENT),
                            w(v::RM_ALLOC_H_OBJECT),
                            w(v::RM_ALLOC_H_CLASS),
                            w(v::RM_ALLOC_PARAMS_SIZE) as usize,
                        );
                        let at = v::RM_ALLOC_PARAMS;
                        // The parameters follow the fixed part in the slot even when
                        // `length` does not count them (msgq.rs `slot_payload`).
                        let params = p
                            .get(at..at + size)
                            .or_else(|| msg.slot_payload.get(at..at + size))
                            .unwrap_or(&[]);
                        let h_client = w(v::RM_ALLOC_H_CLIENT);
                        let a = self
                            .ctrl
                            .alloc(h_client, h_object, h_parent, h_class, params);
                        notes.push(format!(
                            "alloc: {} -> status {:#x} [{}]",
                            a.note, a.status, a.label
                        ));
                        // A new channel's USERD: GSP-RM clears the USERD it is handed at
                        // channel creation (measured on the host, execution.md §3), so the
                        // fake GSP does the same. Without it a re-created channel on the
                        // same USERD page starts with the physical GPU's last GPGet
                        // (measured 2026-09-23, run 10: GPGet 1 == GPPut 1, nothing ran).
                        let new_channel = self
                            .ctrl
                            .channels
                            .channels()
                            .find(|c| c.h_client == h_client && c.h_object == h_object)
                            .map(|c| (c.chid, c.userd));
                        if let Some((chid, Some(userd))) = new_channel {
                            let mut gm = crate::channel::GpuMem {
                                fb: &mut self.fb,
                                sys: mem,
                            };
                            let ok = gm.write(userd, &[0u8; USERD_SIZE]);
                            notes.push(format!(
                                "chid {chid}: USERD {userd:?} cleared ({USERD_SIZE} bytes, as GSP-RM does at channel creation): {ok} [synthesized]"
                            ));
                        }
                        let mut reply = p[..at.min(p.len())].to_vec();
                        reply.resize(at, 0);
                        let st = v::RM_ALLOC_STATUS;
                        reply[st..st + 4].copy_from_slice(&a.status.to_le_bytes());
                        let mut out = a.params;
                        out.resize(size, 0);
                        reply.extend_from_slice(&out);
                        Some((a.status, reply))
                    }
                    rpc::FN_FREE => {
                        let p = &msg.payload;
                        // rpc_free_v03_00: an NVOS00 {hRoot, hObjectParent, hObjectOld, status}.
                        let w =
                            |o: usize| u32::from_le_bytes(p[o..o + 4].try_into().unwrap_or([0; 4]));
                        let (h_client, h_object) = (w(v::FREE_H_ROOT), w(v::FREE_H_OBJECT));
                        let a = self.ctrl.free(h_client, h_object);
                        notes.push(format!("{} [{}]", a.note, a.label));
                        let mut reply = p.clone();
                        if reply.len() >= v::FREE_SIZE {
                            let st = v::FREE_STATUS;
                            reply[st..st + 4].copy_from_slice(&0u32.to_le_bytes());
                        }
                        Some((rpc::NV_OK, reply))
                    }
                    rpc::FN_DUP_OBJECT => {
                        let mut reply = msg.payload.clone();
                        let w = |o: usize| {
                            u32::from_le_bytes(reply[o..o + 4].try_into().unwrap_or([0; 4]))
                        };
                        let note = if reply.len() >= v::DUP_SIZE {
                            self.ctrl.dup(
                                (w(v::DUP_H_CLIENT), w(v::DUP_H_OBJECT)),
                                (w(v::DUP_H_CLIENT_SRC), w(v::DUP_H_OBJECT_SRC)),
                            )
                        } else {
                            "short payload".into()
                        };
                        if reply.len() >= v::DUP_SIZE {
                            let st = v::DUP_STATUS;
                            reply[st..st + 4].copy_from_slice(&0u32.to_le_bytes());
                        }
                        notes.push(format!("dup object: {note}: ok [guest, recorded]"));
                        Some((rpc::NV_OK, reply))
                    }
                    rpc::FN_INIT_GSP_TRACE_CRASH_BUFFER => {
                        let p = &msg.payload;
                        let (a, s) = (v::CRASH_BUFFER_PA, v::CRASH_BUFFER_SIZE);
                        let pa = u64::from_le_bytes(p[a..a + 8].try_into().unwrap_or([0; 8]));
                        let size = u32::from_le_bytes(p[s..s + 4].try_into().unwrap_or([0; 4]));
                        self.ctrl.crash_buffer = Some((pa, size));
                        notes.push(format!("init gsp trace crash buffer: pa {pa:#x} size {size:#x} [guest, recorded]"));
                        Some((rpc::NV_OK, msg.payload.clone()))
                    }
                    rpc::FN_UPDATE_BAR_PDE => {
                        // rpc_update_bar_pde_v15_00: barType, entryValue, entryLevelShift.
                        let p = &msg.payload;
                        let (b, e, s) = (
                            v::BAR_PDE_BAR_TYPE,
                            v::BAR_PDE_ENTRY_VALUE,
                            v::BAR_PDE_LEVEL_SHIFT,
                        );
                        let bar = u32::from_le_bytes(p[b..b + 4].try_into().unwrap_or([0; 4]));
                        let entry = u64::from_le_bytes(p[e..e + 8].try_into().unwrap_or([0; 8]));
                        let shift = u32::from_le_bytes(p[s..s + 4].try_into().unwrap_or([0; 4]));
                        // BAR_2: the value is PDE3[0] of the root at bar2PdeBase.
                        let bar2 = bar == v::BAR_PDE_BAR_TYPE_BAR2;
                        let root = if bar2 {
                            self.bar_root(3)
                        } else {
                            self.bar_root(1)
                        };
                        if bar2 {
                            self.ctrl.bar2_pde = Some(entry);
                        } else {
                            self.ctrl.bar1_pde = Some(entry);
                        }
                        let stored = root.map(|r| self.fb.write_u64(r, entry)).unwrap_or(false);
                        notes.push(format!(
                            "update bar pde: barType {bar} entryValue {entry:#x} levelShift {shift} -> PDE3[0] at {:?} stored {stored} [guest]",
                            root
                        ));
                        Some((rpc::NV_OK, msg.payload.clone()))
                    }
                    _ => {
                        self.unanswered.push(format!(
                            "{} {}",
                            rpc::function_name(h.function),
                            rpc::describe(h.function, &msg.payload)
                        ));
                        Some((rpc::NV_ERR_NOT_SUPPORTED, msg.payload.clone()))
                    }
                };
                if let Some((result, payload)) = reply {
                    // The reply takes the request's shape: at most 65520 bytes per record,
                    // continuation records with the following sequence numbers.
                    const MAX_RPC: usize = v::MAX_RPC_SIZE;
                    let first_len = payload.len().min(MAX_RPC - v::RPC_HEADER_SIZE);
                    let hdr = RpcHeader::reply(&h, result, first_len);
                    match queues.send(&qm, &hdr, &payload[..first_len]) {
                        Ok(seq) => notes.push(format!(
                            "tx seq {seq} reply {} sequence {} result {result:#x} length {}",
                            rpc::function_name(h.function),
                            h.sequence,
                            hdr.length
                        )),
                        Err(e) => notes.push(format!("send: {e}")),
                    }
                    let mut sent = first_len;
                    let mut sequence = h.sequence;
                    while sent < payload.len() {
                        let chunk = (payload.len() - sent).min(MAX_RPC - v::RPC_HEADER_SIZE);
                        sequence = sequence.wrapping_add(1);
                        let cont = RpcHeader {
                            function: rpc::FN_CONTINUATION_RECORD,
                            sequence,
                            ..RpcHeader::reply(&h, result, chunk)
                        };
                        match queues.send(&qm, &cont, &payload[sent..sent + chunk]) {
                            Ok(seq) => notes.push(format!(
                                "tx seq {seq} continuation sequence {sequence} length {}",
                                cont.length
                            )),
                            Err(e) => {
                                notes.push(format!("send continuation: {e}"));
                                break;
                            }
                        }
                        sent += chunk;
                    }
                }
            }
        }
        self.pages = pages;
        self.queues = Some(queues);
        notes
    }

    fn link_queues(
        &mut self,
        mem: &GuestMemory,
        libos_args_pa: u64,
    ) -> Result<Vec<String>, String> {
        let mut notes = Vec::new();
        let mut rmargs_pa = None;
        for i in 0..16u64 {
            let e = libos_args_pa + v::LIBOS_ARG_SIZE * i;
            let id8 = mem
                .read_u64(e + v::LIBOS_ARG_ID8)
                .ok_or("libos args not mapped")?;
            if id8 == 0 {
                break;
            }
            let pa = mem.read_u64(e + v::LIBOS_ARG_PA).unwrap_or(0);
            let size = mem.read_u64(e + v::LIBOS_ARG_REGION_SIZE).unwrap_or(0);
            let name: String = id8
                .to_be_bytes()
                .iter()
                .filter(|&&c| c != 0)
                .map(|&c| c as char)
                .collect();
            notes.push(format!("libos arg {i}: {name:?} pa {pa:#x} size {size:#x}"));
            if id8 == v::RMARGS_ID8 {
                rmargs_pa = Some(pa);
            }
        }
        let rmargs = rmargs_pa.ok_or("no RMARGS region in the libos init arguments")?;
        let shared = mem
            .read_u64(rmargs + v::MQ_SHARED_MEM_PA)
            .ok_or("RMARGS not mapped")?;
        let count = mem
            .read_u32(rmargs + v::MQ_PAGE_TABLE_ENTRY_COUNT)
            .unwrap_or(0);
        let cmd_off = mem.read_u64(rmargs + v::MQ_CMD_QUEUE_OFFSET).unwrap_or(0);
        let stat_off = mem.read_u64(rmargs + v::MQ_STAT_QUEUE_OFFSET).unwrap_or(0);
        notes.push(format!(
            "rmargs: sharedMemPhysAddr {shared:#x} pages {count} cmdQueueOffset {cmd_off:#x} statQueueOffset {stat_off:#x}"
        ));
        if count == 0 || count > 4096 || stat_off >= count as u64 * 4096 {
            return Err(format!(
                "implausible queue arguments: {count} pages, status at {stat_off:#x}"
            ));
        }
        let pages = QueueMemory::read_page_table(mem, shared, count)
            .ok_or("queue page table not mapped")?;
        let stat_size = (count as u64 * 4096 - stat_off) as u32;
        let mut queues = {
            let qm = QueueMemory::new(mem, &pages);
            QueueState::link(&qm, cmd_off, stat_off, stat_size)?
        };
        notes.push(format!(
            "queues linked: cmd {:?}; status header written (size {:#x}, {} slots)",
            queues.cmd, stat_size, queues.stat.msg_count
        ));
        // GSP_INIT_DONE: function 0x1001, sequence 0, rpc_result 0, bIsD3Hot = 0.
        let sent = {
            let qm = QueueMemory::new(mem, &pages);
            queues.send(&qm, &RpcHeader::event(rpc::EV_GSP_INIT_DONE, 1), &[0u8])
        };
        match sent {
            Ok(seq) => notes.push(format!("tx seq {seq} event GSP_INIT_DONE")),
            Err(e) => notes.push(format!("send GSP_INIT_DONE: {e}")),
        }
        self.pages = pages;
        self.queues = Some(queues);
        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chip::turing::local_memory_range;
    use crate::falcon;

    /// The TU106 boot model behind the generic part: the firmware-boot registers
    /// reach the chip's model, its events reach the queues.
    #[test]
    fn boot_sequence_side_effects() {
        let f = crate::fixture::rtx2070();
        let mem = GuestMemory::new();
        let mut b = BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(4 << 30),
            f.identity(4 << 30),
        );
        let (gsp, sec2, queue_head) = (0x0011_0000u64, 0x0084_0000u64, 0x0011_0C00u64);
        assert_eq!(b.read32(gsp + falcon::CPUCTL), Some(0x10));
        assert_eq!(b.read32(0x0011_8128), Some(1));
        assert_eq!(b.read32(0x0011_8234), Some(0xFF));
        assert_eq!(
            b.read32(0x0010_0CE0),
            local_memory_range(4 << 30).map(|(v, _)| v)
        );
        assert_eq!(b.read32(0x001F_A828), Some(0));
        assert_eq!(b.read32(0x1234), None);
        assert_eq!(
            b.write32(0x1704, 1, &mem),
            Some(vec!["read-only".to_string()])
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
        let notes = b.write32(gsp + falcon::CPUCTL, 2, &mem).unwrap();
        assert!(notes.iter().any(|n| n.contains("fallback")), "{notes:?}");
        // 4 GiB - 1 MiB VGA workspace, aligned, minus 1 MiB FRTS = 0xFFE00000.
        assert_eq!(b.read32(0x001F_A824), Some(0x00FF_E000));

        // Booter on SEC2 with the WPR meta PA in the mailboxes.
        b.write32(sec2 + falcon::MAILBOX0, 0x7654_3000, &mem);
        b.write32(sec2 + falcon::MAILBOX1, 0x1, &mem);
        let notes = b.write32(sec2 + falcon::CPUCTL, 2, &mem).unwrap();
        assert!(notes[0].contains("wpr meta PA 0x176543000"), "{notes:?}");
        assert!(
            notes[1].contains("wpr meta: WPR meta at 0x176543000 not mapped"),
            "{notes:?}"
        );
        assert_eq!(b.wpr_meta_pa(), Some(0x1_7654_3000));

        // Libos args through the GSP mailboxes, then the doorbell.
        b.write32(gsp + falcon::MAILBOX0, 0x8000_1000, &mem);
        b.write32(gsp + falcon::MAILBOX1, 0, &mem);
        let notes = b.write32(queue_head, 0, &mem).unwrap();
        assert!(
            notes[0].contains(
                "doorbell 1: libos args PA 0x80001000 (NOT MAPPED), wpr meta PA 0x176543000"
            ),
            "{notes:?}"
        );
        assert_eq!(b.doorbells(), 1);

        // The usermode doorbell reaches the channel emulation (E5a).
        let notes = b.write32(0x00BB_0090, 0x1_0007, &mem).unwrap();
        assert!(notes[1].contains("chid 7: no such channel"), "{notes:?}");
        assert_eq!(b.read32(0x00B8_3110), Some(256));

        // A function-level reset brings the post-GFW state back.
        b.reset();
        assert_eq!(b.read32(gsp + falcon::CPUCTL), Some(0x10));
        assert_eq!(b.read32(0x001F_A824), Some(0));
        assert_eq!(b.doorbells(), 0);
    }
}

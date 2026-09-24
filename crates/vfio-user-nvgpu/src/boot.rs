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
use crate::msgq::{QueueMemory, QueueState, ReplyTo, RpcHeader};
use crate::rpc::{self, StaticIdentity, WprLayout};
use crate::ver::active as v;
use crate::ver::GspTransport;

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
    /// GSP IRQSTAT.SWGEN0 raised for an event the device posted unasked (display hotplug).
    gsp_swgen0: bool,
    /// The CPU-GSP transport this model serves: the emulated version's
    /// (`ver::active::GSP_TRANSPORT`) unless [`BootModel::with_transport`] says otherwise.
    transport: GspTransport,
    /// The last value the CPU wrote to the command-queue head register. With message
    /// queue v2 it is the CPU's write pointer (`msgqTxCreate` writes 0 before GSP-RM
    /// boots, msgq.c:175-177 of 615); with v0 the register is a plain doorbell.
    cpu_cmd_head: u32,
}

// The GSP falcon's interrupt registers (turing/tu102/dev_falcon_v4.h at NV_PGSP 0x110000,
// dev_riscv_pri.h at NV_FALCON2_GSP_BASE 0x111000) and the SWGEN0 bit.
const GSP_IRQSCLR: u64 = 0x0011_0004;
const GSP_IRQSTAT: u64 = 0x0011_0008;
const GSP_IRQMASK: u64 = 0x0011_0018;
const GSP_IRQDEST: u64 = 0x0011_001C;
const GSP_RISCV_IRQMASK: u64 = 0x0011_12B4;
const GSP_RISCV_IRQDEST: u64 = 0x0011_12B8;
const GSP_SWGEN0: u32 = 1 << 6;
/// `NV_VGPU_MSG_EVENT_POST_EVENT` (rpc_global_enums.h).
const POST_EVENT: u32 = 0x1003;

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
            fb: FakeFb::for_layout(&layout).with_mmu_format(&d.gmmu.mmu_format),
            // Reset value as the VBIOS leaves it: the last MiB of FB (fb-model.md §4c).
            bar0_window: ((fb_bytes - (1 << 20)) >> 16) as u32,
            mmu_invalidate: (0, 0, 0),
            l2_ops: BTreeMap::new(),
            l2_op_counts: (0, 0, 0),
            bar2_block: 0,
            bar_faults: 0,
            // The display raises its vector from its clock thread (display/engine.rs).
            intr: match crate::display::intr_shared() {
                Some(s) => IntrTree::new(r.intr_tree, r.intr_tree_size).with_shared(s),
                None => IntrTree::new(r.intr_tree, r.intr_tree_size),
            },
            vf: crate::uvm::VfRegs::new(d),
            exec: None,
            gsp_swgen0: false,
            transport: v::GSP_TRANSPORT,
            cpu_cmd_head: 0,
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
        let mapper = hm
            .lock()
            .unwrap()
            .reservation_mapper()
            .map_err(|st| format!("reservation window mapper: status {st:#x}"))?;
        self.fb = FakeFb::for_layout(&self.layout).with_store(Box::new(VramStore::new(
            Box::new(mapper),
            base,
            end,
        )));
        let r = self.regs;
        self.intr = IntrTree::new(r.intr_tree, r.intr_tree_size).with_shared(shared.clone());
        self.ctrl.mirror = Some(Mirror::new(hm.clone(), shared.clone(), self.chip.data())?);
        self.exec = Some((hm, shared));
        Ok(self)
    }

    /// A BAR0 read of the display (display/mod.rs), which may advance its clock and
    /// write notifiers into the FB or guest memory. A pending connector command is acted
    /// on first: its POST_EVENTs go on the status queue and GSP IRQSTAT.SWGEN0 rises, as
    /// the GSP raises it for any message it sends unasked (kgspService_TU102).
    pub fn display_read32(&mut self, off: u64, mem: &GuestMemory) -> Option<u32> {
        if !self.ctrl.display.enabled {
            // Headless: the display model still answers the fuse (display absent).
            return self.ctrl.display.read32_io(off, &mut self.fb, mem);
        }
        if crate::display::engine::ticker().command_pending() {
            self.post_display_command(mem);
        }
        // GSP falcon IRQSTAT / IRQMASK / IRQDEST and the RISC-V IRQMASK / IRQDEST: SWGEN0
        // (bit 6) is routed to the host; IRQSTAT carries it while an event is unread.
        match off {
            GSP_IRQSTAT => {
                let v = self.read32(off).unwrap_or(0);
                return Some(v | if self.gsp_swgen0 { GSP_SWGEN0 } else { 0 });
            }
            GSP_IRQMASK | GSP_IRQDEST | GSP_RISCV_IRQMASK | GSP_RISCV_IRQDEST => {
                return Some(self.read32(off).unwrap_or(0) | GSP_SWGEN0);
            }
            _ => {}
        }
        self.ctrl.display.read32_io(off, &mut self.fb, mem)
    }

    /// A BAR0 write to the display (display/mod.rs).
    pub fn display_write32(&mut self, off: u64, v: u32, mem: &GuestMemory) -> Option<Vec<String>> {
        if off == GSP_IRQSCLR && v & GSP_SWGEN0 != 0 {
            // The kernel clears SWGEN0 before it reads the queue; the falcon model sees
            // the write too.
            self.gsp_swgen0 = false;
            return None;
        }
        self.ctrl.display.write32(off, v, mem, &mut self.fb)
    }

    /// Act on a host-side connector command (display/mod.rs `connector_command`): post
    /// one `POST_EVENT` (0x1003, `rpc_post_event_v17_00`) per registered hotplug event.
    fn post_display_command(&mut self, mem: &GuestMemory) {
        let Some(cmd) = crate::display::engine::ticker().take_command() else {
            return;
        };
        let events = self.ctrl.display.connector_command(&cmd);
        let Some(mut queues) = self.queues.take() else {
            self.ctrl
                .display
                .notes
                .push("display: connector command before the GSP queues exist; no event".into());
            return;
        };
        let mut sent = 0;
        {
            let qm = QueueMemory::new(mem, &self.pages);
            for p in &events {
                match queues.send(&qm, &RpcHeader::event(POST_EVENT, p.len()), p) {
                    Ok(seq) => {
                        sent += 1;
                        self.ctrl
                            .display
                            .notes
                            .push(format!("display: tx seq {seq} event POST_EVENT HOTPLUG"));
                    }
                    Err(e) => self
                        .ctrl
                        .display
                        .notes
                        .push(format!("display: POST_EVENT: {e}")),
                }
            }
        }
        self.queues = Some(queues);
        if sent > 0 {
            self.gsp_swgen0 = true;
            crate::display::engine::ticker().raise_gsp();
        }
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
                "vram store: {} window map(s), {} refused, {} eviction(s), {} stall(s) without any window; {} read(s) of {} bytes, {} write(s) of {} bytes",
                st.maps, st.map_failures, st.evictions, st.stalls, st.reads, st.bytes_read, st.writes, st.bytes_written
            ));
        }
        let (h, m, f) = self.fb.bar_tlb_stats.get();
        s.push(format!(
            "bar tlb: {h} hit(s), {m} walk(s), {f} flush(es) by a table write (docs/caraxes/mapping.md §3)"
        ));
        if let (Some((fb0, len)), Some(st)) =
            (self.fb.bar1_passthrough(), self.fb.bar1_passthrough_stats())
        {
            s.push(format!(
                "bar1 passthrough: guest BAR1 [0, {len:#x}) on FB {fb0:#x}; {} check(s), last: {} page(s) mapped, {} matched, {} elsewhere; {} mismatched page(s) and {} page-table hazard(s) over the run (docs/caraxes/bar1-passthrough.md)",
                st.checks, st.mapped, st.matched, st.mismatched, st.mismatch_events, st.table_hazards
            ));
        }
        if let Some(a) = self.fb.bar2_area() {
            s.push(a.summary());
        }
        if let Some(m) = &self.ctrl.mirror {
            s.push(m.summary());
        }
        if let Some(p) = &self.ctrl.channels.tokens {
            s.push(p.summary());
        }
        if self.ctrl.display.enabled {
            s.push(self.ctrl.display.engine.summary());
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
        if bar == 3 && self.fb.bar2_area().is_some() {
            // With the BAR2 memfd only the trapped pages reach here: the guest RM's flush
            // read (kbusFlushSingle_GV100), its point for "BAR2 writes visible to the GPU".
            self.fb.bar2_note_trapped();
            self.fb.bar2_sync(Some(root), Some(mem));
        }
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

    /// The doorbell token pool of `n` tokens (`CARAXES_DOORBELL_TOKENS`, tokens.rs). Call
    /// after `with_host` and `with_exec`, which build the dispatcher.
    pub fn with_doorbell_tokens(mut self, n: usize) -> Self {
        self.ctrl.channels.tokens = Some(crate::tokens::TokenPool::new(n));
        self
    }

    /// A ring of pool token `token` through its ioeventfd (device.rs `doorbell_tokens`).
    pub fn doorbell_token(&mut self, mem: &GuestMemory, token: u32) -> Vec<String> {
        self.bar2_doorbell_sync(mem);
        let mut gm = crate::channel::GpuMem {
            fb: &mut self.fb,
            sys: mem,
        };
        self.ctrl.doorbell_pool(&mut gm, token, false)
    }

    /// The first host doorbell write since the last call (ctrl.rs `host_doorbell_at`).
    pub fn take_host_doorbell(&mut self) -> Option<std::time::Instant> {
        self.ctrl.host_doorbell_at.take()
    }

    /// The doorbell token pool's counters, for the eventfd reader.
    pub fn token_pool_mut(&mut self) -> Option<&mut crate::tokens::TokenPool> {
        self.ctrl.channels.tokens.as_mut()
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
        // The pool's size and counters survive; every assignment goes with the channels.
        let tokens = self.ctrl.channels.tokens.take().map(|p| {
            let mut fresh = crate::tokens::TokenPool::new(p.len());
            fresh.stats = p.stats;
            fresh
        });
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
        self.fb.carry_bar1_passthrough(&fresh.fb);
        self.fb.carry_bar2_area(&mut fresh.fb);
        fresh.ctrl.channels.tokens = tokens;
        fresh.transport = self.transport;
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

    /// Whether the model answers the BAR0 dword `off` (the interrupt page's start-up
    /// check, intr_page.rs).
    pub fn owns_register(&self, off: u64) -> bool {
        self.handles(off)
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
        if let Some(v) = self.queue_register_read(off) {
            return Some(v);
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
            Some(Reg::Bar2Block) => {
                self.bar2_block = v;
                // BAR2 (re)bound: its tables are live from here on.
                let root = self.bar_root(3);
                self.fb.bar2_sync(root, Some(mem));
            }
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
                self.bar2_doorbell_sync(mem);
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
                    // With the BAR2 memfd: take in what the guest wrote through BAR2 (write
                    // it back to VRAM, mark the changed table pages dirty) and follow the
                    // BAR2 tables; the RM flushes and invalidates after every BAR2 PTE
                    // update (kbusUpdateRmAperture_GM107, kern_bus_gm107.c:2757-2785).
                    let bar2_root = self.bar_root(3);
                    self.fb.bar2_sync(bar2_root, Some(mem));
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
                notes.extend(self.queue_register_write(off, v, mem));
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
                        notes.extend(self.gsp_started(mem));
                    }
                    Some(BootEvent::CmdQueueDoorbell { libos_args_pa: pa }) => {
                        // An RPC may name memory the guest filled through BAR2.
                        let bar2_root = self.bar_root(3);
                        self.fb.bar2_sync(bar2_root, Some(mem));
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

    /// `CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1`: a BAR2 memfd sync before every doorbell (the
    /// reference for the rule that the guest's flush and invalidate are enough,
    /// docs/caraxes/bar2-memfd.md §3).
    fn bar2_doorbell_sync(&mut self, mem: &GuestMemory) {
        if self.fb.bar2_area().is_some_and(|a| a.sync_doorbell) {
            let root = self.bar_root(3);
            self.fb.bar2_sync(root, Some(mem));
        }
    }

    /// A token-less doorbell from the ioeventfd path (device.rs): ring every channel with
    /// newly published GPFIFO entries (`Dispatcher::doorbell_pending`).
    pub fn doorbell_pending(&mut self, mem: &GuestMemory) -> Vec<String> {
        self.bar2_doorbell_sync(mem);
        let mut gm = crate::channel::GpuMem {
            fb: &mut self.fb,
            sys: mem,
        };
        self.ctrl.doorbell_pending(&mut gm)
    }

    /// Find RMARGS through the libos init arguments, link the queues on the first
    /// doorbell, then answer everything in the command queue.
    fn service_queues(&mut self, mem: &GuestMemory, libos_args_pa: u64) -> Vec<String> {
        let mut notes = Vec::new();
        if self.transport == GspTransport::MsgqV2 && self.queues.is_none() {
            // Message queue v2 links when GSP-RM starts (`gsp_started`); a doorbell before
            // that is the CPU's msgqTxCreate at construction (msgq.c:175-177 of 615).
            notes.push("queues: not linked yet (615 transport: they link when GSP-RM starts)".into());
            return notes;
        }
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
            notes.extend(queues.retry_held(&qm));
            loop {
                let mut msg = match queues.receive(&qm) {
                    Ok(Some(m)) => m,
                    Ok(None) => break,
                    Err(e) => {
                        notes.push(format!("receive: {e}"));
                        break;
                    }
                };
                // A rejected element (msgq.rs `Rejection`) is never dispatched. Its
                // error reply goes to the request it belongs to: a torn continuation
                // record ends the request being assembled.
                if let Some(to) = msg.reply_to() {
                    let rej = msg.rejected.take().expect("rejected");
                    notes.push(format!("rx seq {} rejected: {}", msg.seq_num, rej.reason));
                    let target = match to {
                        ReplyTo::Itself => Some(msg.header),
                        ReplyTo::PendingRequest => self.pending.take().map(|(p, _)| p.header),
                        ReplyTo::Nobody => None,
                    };
                    if let Some(h) = target {
                        match queues.send(&qm, &RpcHeader::reply(&h, rej.status, 0), &[]) {
                            Ok(seq) => notes.push(format!(
                                "tx seq {seq} reply {} sequence {} result {:#x} (rejected request)",
                                rpc::function_name(h.function),
                                h.sequence,
                                rej.status
                            )),
                            Err(e) => notes.push(format!("send: {e}")),
                        }
                    }
                    continue;
                }
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
                    if msg.header.length as usize >= crate::ver::element_layout().max_rpc_size()
                        && needed > msg.payload.len()
                    {
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
                    // 615 on: one GMCAPI request in place of the four init RPCs
                    // (docs/caraxes/gsp-transport-615.md §4-5); its id lies above the RPC
                    // function numbers, so a 610 guest never reaches these arms.
                    crate::gmcapi::CMD_GSP_INIT if msg.gmcapi.is_some() => {
                        Some(self.gsp_init(&msg, &mut notes))
                    }
                    crate::gmcapi::CMD_GSP_SUSPEND if msg.gmcapi.is_some() => {
                        // Fire and forget (gmcapi_table.h:109-116 of 615): no response. The
                        // kernel then waits for the processor-suspended bit that libos
                        // sets in the GSP's MAILBOX0 (kernel_gsp.c:5889-5907,
                        // kernel_gsp_tu102.c:218-255 of 615); without it the unload
                        // waits out its timeout.
                        let m0 = self.fw.read32(GSP_MAILBOX0).unwrap_or(0);
                        self.fw.write32(GSP_MAILBOX0, m0 | GSP_PROCESSOR_SUSPENDED, mem);
                        notes.push(format!(
                            "gmcapi GSP_SUSPEND: no response (fire and forget); MAILBOX0 {m0:#x} -> {:#x} (processor suspended)",
                            m0 | GSP_PROCESSOR_SUSPENDED
                        ));
                        None
                    }
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
                        Some((rpc::NV_OK, crate::ver::static_info_for_guest(info)))
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
                        // Controls only newer chips' kernels send come from the chip first
                        // (chip/physical.rs), the rest from the dispatcher.
                        let a = match crate::chip::physical::answer(self.chip.data(), cmd, params) {
                            Some(a) => a,
                            None => {
                                self.ctrl
                                    .control(w(v::RM_CONTROL_H_CLIENT), h_object, cmd, params)
                            }
                        };
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
                    let max_rpc: usize = crate::ver::element_layout().max_rpc_size();
                    let first_len = payload.len().min(max_rpc - v::RPC_HEADER_SIZE);
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
                        let chunk = (payload.len() - sent).min(max_rpc - v::RPC_HEADER_SIZE);
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
            if queues.held() > 0 {
                notes.push(format!(
                    "status queue full: {} element(s) held for the next doorbell",
                    queues.held()
                ));
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

/// The GSP falcon's MAILBOX0 (NV_PGSP_FALCON_MAILBOX0, turing/tu102/dev_gsp.h) and the
/// `INTERRUPT_PROCESSOR_SUSPENDED_VALUE` libos reports in it (kernel_gsp_tu102.c:218-221
/// of 615).
const GSP_MAILBOX0: u64 = 0x0011_0040;
const GSP_PROCESSOR_SUSPENDED: u32 = 0x8000_0000;

/// `NV_ERR_BUFFER_TOO_SMALL` and `NV_ERR_INVALID_ARGUMENT` (sdk/nvstatuscodes.h).
const NV_ERR_BUFFER_TOO_SMALL: u32 = 0x02;
const NV_ERR_INVALID_ARGUMENT: u32 = 0x1F;

/// The 615 transport (docs/caraxes/gsp-transport-615.md): message queue v2 with the
/// pointers in the NV_PGSP_QUEUE_* and MSGQ_* registers, linked when GSP-RM starts, and
/// the GMCAPI init.
impl BootModel {
    /// Serve `transport` instead of the emulated version's (the tests of the 615 path,
    /// which run in the default 610 build).
    pub fn with_transport(mut self, transport: GspTransport) -> Self {
        self.transport = transport;
        self
    }

    pub fn transport(&self) -> GspTransport {
        self.transport
    }

    /// `NV_PGSP_QUEUE_TAIL(0)` and `NV_PGSP_MSGQ_HEAD(0)` once v2 queues are linked:
    /// the GSP's read pointer into the command queue and write pointer into the status
    /// queue (msgqTxGetFreeSpace and msgqRxGetReadAvailable read them, msgq.c:283-430 of 615).
    fn queue_register_read(&self, off: u64) -> Option<u32> {
        if self.transport != GspTransport::MsgqV2 {
            return None;
        }
        let r = self.fw.queue_registers()?;
        let q = self.queues.as_ref()?.v2()?;
        match off {
            o if o == r.cmd_tail => Some(q.read_ptr),
            o if o == r.msg_head => Some(q.write_ptr),
            _ => None,
        }
    }

    /// The CPU's pointer writes: `NV_PGSP_QUEUE_HEAD(0)` (its command write pointer; the
    /// doorbell event follows from the firmware-boot model) and `NV_PGSP_MSGQ_TAIL(0)`
    /// (its status read pointer, after which held replies may fit). The value still
    /// reaches the falcon block. A head of 0 while our tail is not is a queue created
    /// anew (a driver reload without a reset): the link is dropped until GSP-RM starts
    /// again.
    fn queue_register_write(&mut self, off: u64, v: u32, mem: &GuestMemory) -> Vec<String> {
        let mut notes = Vec::new();
        if self.transport != GspTransport::MsgqV2 {
            return notes;
        }
        let Some(r) = self.fw.queue_registers() else {
            return notes;
        };
        if off == r.cmd_head {
            self.cpu_cmd_head = v;
            let recreated = match self.queues.as_mut().and_then(|q| q.v2_mut()) {
                Some(q) if v == 0 && q.read_ptr != 0 => true,
                Some(q) => {
                    q.cpu_write_ptr = v;
                    false
                }
                None => false,
            };
            if recreated {
                self.queues = None;
                notes.push("queues: QUEUE_HEAD written 0 (command queue created anew); link dropped until GSP-RM starts again".into());
            }
        } else if off == r.msg_tail {
            let pages = std::mem::take(&mut self.pages);
            if let Some(queues) = self.queues.as_mut() {
                if let Some(q) = queues.v2_mut() {
                    q.cpu_read_ptr = v;
                }
                let qm = QueueMemory::new(mem, &pages);
                notes.extend(queues.retry_held(&qm));
            }
            self.pages = pages;
        }
        notes
    }

    /// GSP-RM "starts" (Booter-Load done). The 615 CPU polls for our status-queue header
    /// right after this (GspStatusQueueInit, kernel_gsp_tu102.c:463-469 of 615) and rings
    /// no doorbell before its first message, so the v2 queues link here, from the libos
    /// arguments the CPU put into the GSP mailboxes before Booter
    /// (kernel_gsp_tu102.c:429-437). A 610 device notes a 615 guest (RMARGS magic "GSP ")
    /// and leaves its queues alone: the guest times out in its status-queue link.
    fn gsp_started(&mut self, mem: &GuestMemory) -> Vec<String> {
        let mut notes = Vec::new();
        let Some(pa) = self.fw.libos_args_pa() else {
            if self.transport == GspTransport::MsgqV2 {
                notes.push("queues (v2): no libos arguments in the GSP mailboxes; not linked".into());
            }
            return notes;
        };
        if self.transport != GspTransport::MsgqV2 {
            let mut probe = Vec::new();
            if let Ok(rmargs) = Self::find_rmargs(mem, pa, &mut probe) {
                if crate::msgq::v2::rmargs_is_v2(mem, rmargs) == Some(true) {
                    notes.push(format!(
                        "guest ABI: ERROR the guest's RMARGS at {rmargs:#x} carry the 615 magic \"GSP \"; this device emulates {} with the 610 transport (start it with --guest-abi 615 from a v615 build, CARAXES_GUEST_ABI=615)",
                        v::DRIVER_VERSION
                    ));
                }
            }
            return notes;
        }
        self.libos_args_pa = Some(pa);
        match self.link_queues_v2(mem, pa) {
            Ok(n) => notes.extend(n),
            Err(e) => notes.push(format!("queues (v2): {e}")),
        }
        notes
    }

    /// The RMARGS region among the libos init arguments at `libos_args_pa`.
    fn find_rmargs(mem: &GuestMemory, libos_args_pa: u64, notes: &mut Vec<String>) -> Result<u64, String> {
        let mut rmargs = None;
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
                rmargs = Some(pa);
            }
        }
        rmargs.ok_or_else(|| "no RMARGS region in the libos init arguments".into())
    }

    /// Link message queue v2: check RMARGS (`GSP_ARGUMENTS_CACHED`, magic "GSP "), read
    /// the page table, check the CPU's command-queue header and write ours
    /// (`msgq::v2::QueueStateV2::link`). No GSP_INIT_DONE: the 615 kernel does not wait
    /// for it.
    fn link_queues_v2(&mut self, mem: &GuestMemory, libos_args_pa: u64) -> Result<Vec<String>, String> {
        use crate::msgq::v2::{QueueArgs, QueueStateV2};
        let mut notes = Vec::new();
        let rmargs = Self::find_rmargs(mem, libos_args_pa, &mut notes)?;
        let args = QueueArgs::read(mem, rmargs)?;
        notes.push(format!(
            "rmargs (615): size {} flags {:#x} sharedMemPhysAddr {:#x} pages {} cmdQueueOffset {:#x} statQueueOffset {:#x} element {}/{:#x}/{:#x}",
            args.args_size,
            args.args_flags,
            args.shared_mem_pa,
            args.page_table_entry_count,
            args.cmd_queue_offset,
            args.stat_queue_offset,
            args.element_hdr_size,
            args.element_size_min,
            args.element_size_max
        ));
        let pages = QueueMemory::read_page_table(mem, args.shared_mem_pa, args.page_table_entry_count)
            .ok_or("queue page table not mapped")?;
        let q = {
            let qm = QueueMemory::new(mem, &pages);
            QueueStateV2::link(&qm, &args, self.cpu_cmd_head)?
        };
        notes.push(format!(
            "queues linked (v2): cmd {:?}; status header written (size {:#x}, {} slots); pointers in QUEUE_HEAD/TAIL(0), MSGQ_HEAD/TAIL(0); no GSP_INIT_DONE",
            q.cmd, q.stat.size, q.stat.msg_count
        ));
        self.pages = pages;
        self.queues = Some(crate::msgq::QueueState::from_v2(q));
        self.pending = None;
        Ok(notes)
    }

    /// `GMCAPI_CMD_GSP_INIT`: read the NVKV system information (BAR addresses, driver
    /// version, registry), answer with the static configuration as NVKV keys
    /// (`rpc::static_info_nvkv`), bounded by the request's `max_response_size`.
    fn gsp_init(&mut self, msg: &crate::msgq::Message, notes: &mut Vec<String>) -> (u32, Vec<u8>) {
        let max = msg.gmcapi.map_or(0, |g| g.request_or_status as usize);
        let si = match crate::gmcapi::SystemInfo::parse(&msg.payload) {
            Ok(si) => si,
            Err(e) => {
                notes.push(format!("gmcapi GSP_INIT: request refused: {e}"));
                return (NV_ERR_INVALID_ARGUMENT, Vec::new());
            }
        };
        notes.push(format!("gmcapi GSP_INIT system info: {}", si.describe()));
        if let Some(bars) = si.bar_addresses() {
            self.ctrl.bar_offsets = bars;
        }
        if si.driver_version.as_deref() != Some(v::DRIVER_VERSION) {
            notes.push(format!(
                "guest driver {:?} is not the emulated {} (build with its ver module)",
                si.driver_version,
                v::DRIVER_VERSION
            ));
        }
        for (name, value) in &si.registry {
            notes.push(format!("registry {name} = {value:?}"));
        }
        self.system_info = Some(msg.payload.clone());
        let (image, labels) = rpc::build_static_info(
            self.chip.data(),
            &self.identity,
            self.wpr.as_ref(),
            &self.layout,
        );
        for l in labels {
            notes.push(format!("static-info {} = {} [{}]", l.field, l.value, l.source));
        }
        let (out, keys) = rpc::static_info_nvkv(&image, self.chip.data());
        for k in keys {
            notes.push(format!("static-info key {k}"));
        }
        if out.len() > max {
            notes.push(format!(
                "gmcapi GSP_INIT: {} bytes of static info exceed max_response_size {max}",
                out.len()
            ));
            return (NV_ERR_BUFFER_TOO_SMALL, Vec::new());
        }
        notes.push(format!("gmcapi GSP_INIT: {} bytes of NVKV static info", out.len()));
        (rpc::NV_OK, out)
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

    /// Guest memory holding what the 615.71.09 guest prepared before its GSP boot, as
    /// captured from guest RAM (caraxes/runs/guest-driver-2026-09-24/615.71.09/unlinked/):
    /// the WPR meta at 0x769c000, the libos init arguments at 0x769d000, RMARGS
    /// (`GSP_ARGUMENTS_CACHED`) at 0x769e000, and the 129-page shared queue buffer at
    /// 0x88a00000 with the page table and the v2 command-queue header.
    fn guest_615(tag: &str) -> (GuestMemory, std::path::PathBuf) {
        use std::io::Write;
        const RUN: &str = "../../caraxes/runs/guest-driver-2026-09-24/615.71.09/unlinked";
        let read = |f: &str| std::fs::read(format!("{RUN}/{f}")).unwrap();
        let path = std::env::temp_dir().join(format!("nvgpu-boot615-{tag}-{}", std::process::id()));
        let mut f = std::fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        f.write_all(&vec![0u8; 0x100000]).unwrap();
        let mut mem = GuestMemory::new();
        mem.map(0x0769_0000, 0x20000, 0, &f).unwrap();
        mem.map(0x88a0_0000, 0x81000, 0x20000, &f).unwrap();
        assert!(mem.write(0x0769_c000, &read("wpr-meta.bin")));
        assert!(mem.write(0x0769_d000, &read("libos-init-args.bin")));
        assert!(mem.write(0x0769_e000, &read("gsp-arguments-cached.bin")));
        assert!(mem.write(0x88a0_0000, &read("queue-page-table.bin")));
        assert!(mem.write(0x88a0_1000, &read("cmd-queue-first-page.bin")));
        (mem, path)
    }

    /// Booter-Load of the Turing boot model with the libos arguments in the GSP mailboxes
    /// and the WPR meta in SEC2's, as `kgspBootstrap_TU102` of 615 drives it
    /// (kernel_gsp_tu102.c:429-437).
    fn booter_load(b: &mut BootModel, mem: &GuestMemory) -> Vec<String> {
        let (gsp, sec2) = (0x0011_0000u64, 0x0084_0000u64);
        b.write32(gsp + falcon::MAILBOX0, 0x0769_d000, mem);
        b.write32(gsp + falcon::MAILBOX1, 0, mem);
        b.write32(sec2 + falcon::MAILBOX0, 0x0769_c000, mem);
        b.write32(sec2 + falcon::MAILBOX1, 0, mem);
        b.write32(sec2 + falcon::CPUCTL, 2, mem).unwrap()
    }

    /// The 615 path end to end against the captured guest: the pre-boot QUEUE_HEAD
    /// write is no doorbell, the queues link at Booter-Load with the status header
    /// msgqRxLink wants, the captured first GMCAPI_CMD_GSP_INIT is answered with NVKV
    /// static info the kernel's key handler accepts, and the pointers travel in the
    /// NV_PGSP_QUEUE_* and MSGQ_* registers.
    #[test]
    fn boot_615_link_and_gsp_init_from_the_captured_guest() {
        let (mem, path) = guest_615("init");
        let f = crate::fixture::rtx2070();
        let mut b = BootModel::new(f.chip, crate::fb::FbLayout::plain(4 << 30), f.identity(4 << 30))
            .with_transport(GspTransport::MsgqV2);
        let (cmd_head, cmd_tail, msg_head, msg_tail) =
            (0x0011_0C00u64, 0x0011_0C04u64, 0x0011_0C80u64, 0x0011_0C84u64);
        // msgqTxCreate before the boot (msgq.c:175-177): no link, no reply.
        let notes = b.write32(cmd_head, 0, &mem).unwrap();
        assert!(notes.iter().any(|n| n.contains("not linked yet")), "{notes:?}");
        let notes = booter_load(&mut b, &mem);
        assert!(notes.iter().any(|n| n.starts_with("rmargs (615): size 184 flags 0x2 sharedMemPhysAddr 0x88a00000 pages 129")), "{notes:?}");
        assert!(notes.iter().any(|n| n.starts_with("queues linked (v2)")), "{notes:?}");
        let mut hdr = [0u8; 32];
        assert!(mem.read(0x88a0_0000 + 0x41000, &mut hdr));
        // The header the linked capture wrote by hand (guest-driver-versions.md §5.3).
        assert_eq!(
            hdr.iter().map(|x| format!("{x:02x}")).collect::<String>(),
            "0200000000000400001000003f00000000100000000000000000000000000000"
        );
        assert_eq!((b.read32(cmd_tail), b.read32(msg_head)), (Some(0), Some(0)));

        // The guest links its status queue, queues GSP_INIT and rings.
        let slot0 = std::fs::read(
            "../../caraxes/runs/guest-driver-2026-09-24/615.71.09/linked/gmcapi-gsp-init-slot0.bin",
        )
        .unwrap();
        assert!(mem.write(0x88a0_0000 + 0x2000, &slot0));
        b.write32(msg_tail, 0, &mem);
        let notes = b.write32(cmd_head, 1, &mem).unwrap();
        assert!(notes.iter().any(|n| n.contains("GMCAPI_GSP_INIT (65537) sequence 0")), "{notes:?}");
        assert!(
            notes.iter().any(|n| n.contains("\"NVRM\" \"615.71.09\"") && n.contains("bars 0xe6000000/0x3fffe0000000/0x3ffffc000000/0x0")),
            "{notes:?}"
        );
        assert!(notes.iter().any(|n| n.starts_with("registry RmRcWatchdog = U32(0)")), "{notes:?}");
        assert!(notes.iter().any(|n| n.starts_with("tx seq 0 reply GMCAPI_GSP_INIT sequence 0 result 0x0")), "{notes:?}");
        assert_eq!(b.ctrl.bar_offsets, [0xe600_0000, 0x3fff_e000_0000, 0x3fff_fc00_0000, 0]);
        // QUEUE_TAIL: one element consumed; MSGQ_HEAD: one element published.
        assert_eq!((b.read32(cmd_tail), b.read32(msg_head)), (Some(1), Some(1)));
        // The CPU's view of the response (message_queue_cpu.c:634-822, gpu.c:7887-7893).
        let mut el = vec![0u8; 0x1000];
        assert!(mem.read(0x88a0_0000 + 0x42000, &mut el));
        let u32_at = |o: usize| u32::from_le_bytes(el[o..o + 4].try_into().unwrap());
        assert_eq!((u32_at(0), u32_at(8), u32_at(12)), (0x4D43_5450, 0xC000_0001, 0x2610_DE7E));
        let g = crate::gmcapi::Header::parse(&el[24..]).unwrap();
        assert_eq!((g.command, g.sequence, g.request_or_status), (0x0101_0001, 0, 0));
        assert_eq!(u32_at(4) as usize, 24 + 40 + g.size as usize);
        assert!(g.size <= 0xc000);
        let data = &el[64..64 + g.size as usize];
        let keys = crate::rpc::tests::kernel_accepts(&crate::gmcapi::words(data)).unwrap();
        assert!(keys.iter().any(|e| e.key == crate::gmcapi::sc::GPU_NAME_STRING));
        // A driver reload creates the command queue anew (QUEUE_HEAD := 0): the link
        // goes until GSP-RM starts again.
        let notes = b.write32(cmd_head, 0, &mem).unwrap();
        assert!(notes.iter().any(|n| n.contains("link dropped")), "{notes:?}");
        assert_eq!(b.read32(msg_head), Some(0), "the falcon register again");
        b.reset();
        assert_eq!(b.transport(), GspTransport::MsgqV2, "a reset keeps the transport");
        let _ = std::fs::remove_file(&path);
    }

    /// Fail closed: a 610 device facing the captured 615 guest does not link anything
    /// at Booter-Load, names the mismatch, and keeps the QUEUE_* registers the falcon's.
    #[test]
    fn a_610_device_names_a_615_guest() {
        let (mem, path) = guest_615("mismatch");
        let f = crate::fixture::rtx2070();
        let mut b = BootModel::new(f.chip, crate::fb::FbLayout::plain(4 << 30), f.identity(4 << 30))
            .with_transport(GspTransport::MsgqV1);
        let notes = booter_load(&mut b, &mem);
        assert!(notes.iter().any(|n| n.contains("guest ABI: ERROR") && n.contains("615 magic")), "{notes:?}");
        assert!(!notes.iter().any(|n| n.contains("queues linked")), "{notes:?}");
        assert_eq!(b.read32(0x0011_0C80), Some(0));
        let _ = std::fs::remove_file(&path);
    }
}

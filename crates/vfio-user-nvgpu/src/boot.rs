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
    /// `NV_PTIMER_TIME_0` / `NV_PTIMER_TIME_1` (timer base + 0x400 / + 0x410,
    /// gm107/dev_timer.h:26-27).
    PtimerTime0,
    PtimerTime1,
    /// An L2 cache-operation register (`NV_UFLUSH_*` and the VF-window aliases).
    L2Op(L2Op),
}

/// `NV_PTIMER_TIME_0` / `TIME_1` relative to the PTIMER base (gm107/dev_timer.h:26-27).
const PTIMER_TIME_0: u64 = 0x400;
const PTIMER_TIME_1: u64 = 0x410;

/// One PTIMER word at `now` ns ([`BootModel::ptimer_read`]); `pending` is the high word
/// of the last TIME_1 read not yet followed by a TIME_0 read.
fn ptimer_word(pending: &mut Option<u64>, now: u64, hi: bool) -> u32 {
    if hi {
        *pending = Some(now >> 32);
        return (now >> 32) as u32 & 0x1FFF_FFFF;
    }
    match pending.take() {
        Some(h) if now >> 32 > h => 0xFFFF_FFE0,
        _ => now as u32 & !0x1F,
    }
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
    /// The RM user shared data page GSP-RM keeps (rusd.rs, `CARAXES_RUSD=1`).
    rusd: crate::rusd::Rusd,
    /// A request split over CONTINUATION_RECORDs that is still being assembled.
    pending: Option<(crate::msgq::Message, usize)>,
    /// Memory registered with `ALLOC_MEMORY` and the GSP-side event buffers on it
    /// (gspmem.rs, docs/caraxes/gsp-plan/rpc-functions.md §4.1).
    pub gspmem: crate::gspmem::GspMemory,
    /// Phase-5 answers (docs/caraxes/phase5-rpcs.md).
    pub ctrl: Dispatcher,
    /// The fake framebuffer (docs/caraxes/fb-model.md).
    pub fb: FakeFb,
    /// `NV_PBUS_BAR0_WINDOW`: BASE[23:0] in 64 KiB units, TARGET[25:24].
    bar0_window: u32,
    mmu_invalidate: (u32, u32, u32),
    /// Last value written to each L2 cache-operation register (busy bits masked on read).
    l2_ops: BTreeMap<u64, u32>,
    /// The high word the last `NV_PTIMER_TIME_1` read returned, until the next `TIME_0`
    /// read (the torn-read guard of [`BootModel::ptimer_read`]).
    ptimer_hi: Option<u64>,
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
    /// The device posted an unsolicited message at least once: SWGEN0 reads routed to the
    /// host in the GSP falcon's IRQMASK/IRQDEST (events.rs).
    events_posted: bool,
    /// The error notifiers of the guest channels on host mirrors (events.rs
    /// `ChannelWatch`), by (hClient, hChannel).
    watch: BTreeMap<(u32, u32), crate::events::ChannelWatch>,
    /// The host RC counter at the last notifier scan, and when that scan ran.
    rc_scan: (u64, Option<std::time::Instant>),
    /// The registry keys the device honours (gspboot.rs, B3).
    registry: crate::gspboot::RegistryPolicy,
    /// What the 615 client opted in to at GSP_INIT (B7); `None` before GSP_INIT and on
    /// the 610 transport, which has no such opt-in.
    unstable: Option<crate::gspboot::Unstable>,
    /// The flags of the last GSP_SUSPEND, until the next boot or resume (B5).
    suspended: Option<u64>,
    /// The argument of the last Booter Unload: 0 or the suspend's GspFwSRMeta (B5).
    sr_meta_pa: Option<u64>,
    /// The CPU-GSP transport this model serves: the emulated version's
    /// (`ver::active::GSP_TRANSPORT`) unless [`BootModel::with_transport`] says otherwise.
    transport: GspTransport,
    /// The last value the CPU wrote to the command-queue head register. With message
    /// queue v2 it is the CPU's write pointer (`msgqTxCreate` writes 0 before GSP-RM
    /// boots, msgq.c:175-177 of 615); with v0 the register is a plain doorbell.
    cpu_cmd_head: u32,
    /// The command-queue head write as a KVM ioeventfd (`CARAXES_RPC_IOEVENTFD=1`): a
    /// non-blocking duplicate of the eventfd the reactor reads, so the model can take the
    /// count itself (at the queue link, and before a trapped access of the GSP page), and
    /// what came through it.
    rpc_eventfd: Option<std::fs::File>,
    pub rpc_eventfd_stats: RpcEventfdStats,
}

/// What the command-queue head ioeventfd delivered (`BootModel::cmd_head_from_ring`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RpcEventfdStats {
    /// Guest head writes counted.
    pub writes: u64,
    /// Writes that published an element, served as the trapped write would have been.
    pub elements: u64,
    /// Writes before the queues were linked (`msgqTxCreate`'s 0).
    pub before_link: u64,
    /// Writes that published nothing while linked: a queue created anew (the link drops).
    pub recreated: u64,
    /// Writes taken at the link or before a trapped access instead of by the reactor.
    pub taken_inline: u64,
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

/// The trace's words for a GPPut store the barrier held (device.rs makes it an event).
pub const GP_PUT_HELD: &str = "GPPut of a mirrored channel held";

/// Whether a store of `len` bytes at FB `pa` writes the GPPut dword (USERD + 0x8C,
/// `clc46f.h:61-62`) of one of the channels whose USERD is at `userds`.
pub(crate) fn stores_gp_put(userds: &[u64], pa: u64, len: usize) -> bool {
    const GP_PUT: u64 = 0x8C;
    userds
        .iter()
        .any(|&u| pa < u + GP_PUT + 4 && u + GP_PUT < pa + len as u64)
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
            ctrl: Dispatcher::new(d, &identity, None).with_layout(layout),
            identity,
            wpr_meta_pa: None,
            wpr: None,
            libos_args_pa: None,
            doorbells: 0,
            pages: Vec::new(),
            queues: None,
            system_info: None,
            unanswered: Vec::new(),
            rusd: Default::default(),
            pending: None,
            gspmem: crate::gspmem::GspMemory::new(),
            fb: FakeFb::for_layout(&layout).with_mmu_format(&d.gmmu.mmu_format),
            // Reset value as the VBIOS leaves it: the last MiB of FB (fb-model.md §4c).
            bar0_window: ((fb_bytes - (1 << 20)) >> 16) as u32,
            mmu_invalidate: (0, 0, 0),
            l2_ops: BTreeMap::new(),
            ptimer_hi: None,
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
            events_posted: false,
            watch: BTreeMap::new(),
            rc_scan: (0, None),
            registry: crate::gspboot::RegistryPolicy::default(),
            unstable: None,
            suspended: None,
            sr_meta_pa: None,
            transport: v::GSP_TRANSPORT,
            cpu_cmd_head: 0,
            rpc_eventfd: None,
            rpc_eventfd_stats: RpcEventfdStats::default(),
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
        eprintln!(
            "vfio-user-nvgpu: {} (every BAR1 window of the VRAM store is admitted against the host's free BAR1 minus the window; docs/caraxes/host-resources.md)",
            mapper.bar1_guard().line()
        );
        self.fb = FakeFb::for_layout(&self.layout).with_store(Box::new(VramStore::new(
            Box::new(mapper),
            base,
            end,
        )));
        let r = self.regs;
        self.intr = IntrTree::new(r.intr_tree, r.intr_tree_size).with_shared(shared.clone());
        self.ctrl.mirror = Some(Mirror::new(hm.clone(), shared.clone(), self.chip.data(), &self.layout)?);
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

    /// The display clock's tick with the display page (display/mod.rs `Display::tick`):
    /// a waiting connector command first, as a trapped display read would do.
    pub fn display_tick(&mut self, mem: &GuestMemory) -> Vec<String> {
        if crate::display::engine::ticker().command_pending() {
            self.post_display_command(mem);
        }
        let mut notes = self.ctrl.display.tick(&mut self.fb, mem);
        notes.extend(self.scan_channel_errors(mem, true));
        notes
    }

    pub fn poll_shadow_userds(&mut self, mem: &GuestMemory) -> Vec<String> {
        self.ctrl.poll_shadow_userds(&mut crate::channel::GpuMem {
            fb: &mut self.fb,
            sys: mem,
        })
    }

    /// Send unsolicited messages (events.rs) on the status queue and raise GSP
    /// IRQSTAT.SWGEN0 with the GSP stall vector, as the GSP does for any message it
    /// posts unasked (kgspService_TU102). Without linked queues nothing is sent.
    pub fn post_unsolicited(
        &mut self,
        mem: &GuestMemory,
        items: &[crate::events::Unsolicited],
    ) -> Vec<String> {
        use crate::events::Unsolicited;
        let mut notes = Vec::new();
        let Some(mut queues) = self.queues.take() else {
            notes.push(format!(
                "events: {} message(s) before the GSP queues exist; not sent",
                items.len()
            ));
            return notes;
        };
        let mut sent = 0;
        {
            let qm = QueueMemory::new(mem, &self.pages);
            for u in items {
                // B7: RM events only to a client that opted in to them.
                if let (Unsolicited::Event(..), Some(o)) = (u, self.unstable) {
                    if !o.events {
                        notes.push(format!("{}: not sent, the client did not opt in to events", u.name()));
                        continue;
                    }
                }
                let (r, len) = match u {
                    Unsolicited::Event(f, p) => {
                        (queues.send(&qm, &RpcHeader::event(*f, p.len()), p), p.len())
                    }
                    Unsolicited::Gmcapi(id, p) => (queues.send_gmcapi_request(&qm, *id, p), p.len()),
                };
                match r {
                    Ok(seq) => {
                        sent += 1;
                        if crate::profile::on() {
                            let (id, label) = match u {
                                Unsolicited::Event(f, _) => (*f, "rpc event"),
                                Unsolicited::Gmcapi(id, _) => (*id, "gmcapi request"),
                            };
                            crate::profile::record(
                                crate::profile::Key::Event(id),
                                std::time::Duration::ZERO,
                                len as u64,
                                None,
                                label,
                            );
                        }
                        notes.push(format!("tx seq {seq} {} ({len} bytes) [unsolicited]", u.name()));
                    }
                    Err(e) => notes.push(format!("{}: {e}", u.name())),
                }
            }
        }
        self.queues = Some(queues);
        if sent > 0 {
            self.gsp_swgen0 = true;
            self.events_posted = true;
            let raised = if self.ctrl.display.enabled {
                crate::display::engine::ticker().raise_gsp()
            } else {
                let d = self.chip.data();
                let gsp = v::MC_ENGINE_IDX
                    .iter()
                    .find(|(n, _)| *n == "GSP")
                    .map(|&(_, i)| i);
                match d
                    .interrupts
                    .entries
                    .iter()
                    .find(|e| Some(e.mc_idx as u32) == gsp)
                {
                    Some(e) => {
                        self.intr.trigger(e.vector_stall);
                        true
                    }
                    None => false,
                }
            };
            notes.push(format!("events: {sent} sent, SWGEN0 set, GSP vector raised {raised}"));
        }
        notes
    }

    /// A Booter Load that resumes a PM suspend (B5, boot-exchange.md §5): the last
    /// GSP_SUSPEND carried PM_TRANSITION, the queues are kept, and the RMARGS the libos
    /// arguments name say IN_PM_TRANSITION (kernel_gsp.c:6192-6197).
    fn resuming(&self, mem: &GuestMemory) -> bool {
        let pm = self
            .suspended
            .is_some_and(|f| f & crate::gspboot::SUSPEND_PM_TRANSITION != 0);
        if !pm || self.queues.is_none() || self.transport != GspTransport::MsgqV2 {
            return false;
        }
        let Some(pa) = self.fw.libos_args_pa() else {
            return false;
        };
        let mut probe = Vec::new();
        Self::find_rmargs(mem, pa, &mut probe)
            .ok()
            .and_then(|r| crate::gspboot::resume_args(mem, r))
            .is_some_and(|(in_pm, _, _)| in_pm)
    }

    /// The resume: no relink (the kernel does not link again, "GSP-RM will restore queue
    /// state", kernel_gsp_tu102.c:464-466), the processor-suspended bit cleared, and
    /// GSP_RESUME_DONE (flags 0: no D3hot) as a GMCAPI request, which
    /// `kgspWaitForRmResumeDone` waits for (kernel_gsp.c:7135-7160) in place of GSP_INIT.
    fn resume(&mut self, mem: &GuestMemory, pa: u64) -> Vec<String> {
        let mut notes = Vec::new();
        let flags = self.suspended.take().unwrap_or(0);
        let m0 = self.fw.read32(GSP_MAILBOX0).unwrap_or(0);
        self.fw.write32(GSP_MAILBOX0, m0 & !GSP_PROCESSOR_SUSPENDED, mem);
        let args = self
            .fw
            .libos_args_pa()
            .and_then(|l| Self::find_rmargs(mem, l, &mut Vec::new()).ok())
            .and_then(|r| crate::gspboot::resume_args(mem, r));
        notes.push(format!(
            "resume: booter load argument {pa:#x}, suspend flags {flags:#x}, RMARGS IN_PM_TRANSITION/oldLevel/flags {args:?}; queues kept, no relink"
        ));
        notes.extend(self.post_unsolicited(
            mem,
            &[crate::events::Unsolicited::Gmcapi(
                crate::events::GMCAPI_GSP_RESUME_DONE,
                crate::events::resume_done(false),
            )],
        ));
        notes
    }

    /// The 615 operational-event import for an RC the device raised: one GOE record
    /// (gspmem.rs) and its wake-up into `items`. Nothing while the guest has no ring bound.
    /// Every RC the device raises passes here once, next to its one journal record (rc.rs).
    fn goe_rc(
        &mut self,
        mem: &GuestMemory,
        xid: u32,
        text: &str,
        items: &mut Vec<crate::events::Unsolicited>,
    ) -> Vec<String> {
        if !self.gspmem.has_goe_bind() {
            return Vec::new();
        }
        let ev = crate::gspmem::OpEvent::rc(xid, self.ctrl.channels.now_ns() / 1000, text.to_string());
        let (wake, notes) = self.gspmem.produce(mem, &ev);
        for (c, e) in wake {
            items.push(crate::events::Unsolicited::Event(
                crate::events::EV_POST_EVENT,
                crate::events::post_event(c, e),
            ));
        }
        notes
    }

    /// `CARAXES_RC_SELFTEST=<seconds>` (rc.rs `RcState::selftest`): a SIMULATED RC record of
    /// no channel every so often, once the guest can take it (the GSP queues exist and, on
    /// a 615 guest, the operational-event import is bound). It goes where a device RC goes:
    /// one journal record, OS_ERROR_LOG when the guest logs RCs (chid invalid, so no
    /// channel is touched and no RC_TRIGGERED is sent), and on 615 one operational-event
    /// record.
    fn rc_selftest(&mut self, mem: &GuestMemory) -> Vec<String> {
        let ready = self.queues.is_some()
            && (self.transport.guest_abi() != 615 || self.gspmem.has_goe_bind());
        if !ready {
            return Vec::new();
        }
        let Some(i) = self.ctrl.dstate.rc.selftest.as_mut().and_then(|s| s.due()) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let mut notes = Vec::new();
        {
            let text = format!("Caraxes: SIMULATED RC {i} (CARAXES_RC_SELFTEST, no channel)");
            self.ctrl.dstate.rc.record(None, 0, text.clone());
            notes.push(format!("rc self-test: {text} journalled [synthesized]"));
            if self.registry.log_on_rc {
                items.push(crate::events::Unsolicited::Event(
                    crate::events::EV_OS_ERROR_LOG,
                    crate::events::os_error_log(0, 0, u32::MAX, &text),
                ));
            }
            let n = self.goe_rc(mem, 0, &text, &mut items);
            notes.extend(n);
        }
        if !items.is_empty() {
            notes.extend(self.post_unsolicited(mem, &items));
        }
        notes
    }

    /// The RCs the category D models raised (rc.rs: a fault UVM reported, the RC watchdog;
    /// journalled there) go to the guest on the same path as a host mirror's error:
    /// OS_ERROR_LOG when the guest logs RCs, then RC_TRIGGERED in the guest ABI's layout,
    /// and on 615 an operational-event record.
    fn post_model_rcs(&mut self, mem: &GuestMemory) -> Vec<String> {
        if self.ctrl.dstate.rc.pending.is_empty() {
            return Vec::new();
        }
        let gm = crate::channel::GpuMem { fb: &mut self.fb, sys: mem };
        let rcs = crate::rc::take(&mut self.ctrl, &gm);
        let layout = crate::events::rc_layout(self.transport.guest_abi());
        let mut items = Vec::new();
        let mut notes = Vec::new();
        for r in rcs {
            notes.push(format!("chid {}: RC Xid {} from the D models: {} [guest channel]", r.chid, r.xid, r.text));
            if self.registry.log_on_rc {
                items.push(crate::events::Unsolicited::Event(
                    crate::events::EV_OS_ERROR_LOG,
                    crate::events::os_error_log(r.xid, 0, r.chid, &r.text),
                ));
            }
            items.push(crate::events::Unsolicited::Event(
                crate::events::EV_RC_TRIGGERED,
                crate::events::rc_triggered(layout, r.engine_type, r.chid, r.xid),
            ));
            let n = self.goe_rc(mem, r.xid, &r.text, &mut items);
            notes.extend(n);
        }
        if !items.is_empty() {
            notes.extend(self.post_unsolicited(mem, &items));
        }
        notes
    }

    /// Look at the error notifiers of the guest's channels that run on host mirrors
    /// (events.rs): a write the host RM made there (it is the guest's own page, the
    /// mirror channel's `hObjectError`) becomes OS_ERROR_LOG and RC_TRIGGERED for that
    /// guest channel, once per channel. Runs when the host RC counter moved (a wake-up
    /// only: that subscription is host-global and never a source, events.rs) and at most
    /// every 100 ms otherwise when `periodic`.
    fn scan_channel_errors(&mut self, mem: &GuestMemory, periodic: bool) -> Vec<String> {
        // `CARAXES_GOE_SELFTEST`: a SIMULATED operational event for the guest probe
        // (gspmem.rs `SelfTest`, caraxes/tools/goe-probe.sh).
        let mut goe_notes = Vec::new();
        if let Some(n) = self.gspmem.selftest_due() {
            let ev = crate::gspmem::OpEvent::selftest(n, self.ctrl.channels.now_ns() / 1000);
            let (wake, n) = self.gspmem.produce(mem, &ev);
            goe_notes.extend(n);
            let items: Vec<_> = wake
                .into_iter()
                .map(|(c, e)| {
                    crate::events::Unsolicited::Event(
                        crate::events::EV_POST_EVENT,
                        crate::events::post_event(c, e),
                    )
                })
                .collect();
            if !items.is_empty() {
                goe_notes.extend(self.post_unsolicited(mem, &items));
            }
        }
        goe_notes.extend(self.rc_selftest(mem));
        let mut notes = self.scan_channel_errors_inner(mem, periodic);
        goe_notes.append(&mut notes);
        // Software channels the guest enabled again run their held doorbells (rc.rs, the
        // scheduling flags; ctrl.rs `replay_held`).
        if !self.ctrl.dstate.rc.held.is_empty() {
            let mut gm = crate::channel::GpuMem { fb: &mut self.fb, sys: mem };
            goe_notes.extend(self.ctrl.replay_held(&mut gm));
        }
        goe_notes
    }

    fn scan_channel_errors_inner(&mut self, mem: &GuestMemory, periodic: bool) -> Vec<String> {
        // The category D models' RCs (rc.rs: watchdog timeout, non-replayable fault reports)
        // go out first; the channel scan runs when there are none.
        let model_rcs = self.post_model_rcs(mem);
        if !model_rcs.is_empty() {
            return model_rcs;
        }
        let rc = self
            .exec
            .as_ref()
            .map_or(0, |(_, s)| s.rc_events.load(std::sync::atomic::Ordering::Relaxed));
        let due = periodic
            && self
                .rc_scan
                .1
                .is_none_or(|t| t.elapsed() >= std::time::Duration::from_millis(100));
        if rc == self.rc_scan.0 && !due {
            return Vec::new();
        }
        self.rc_scan = (rc, Some(std::time::Instant::now()));
        let layout = crate::events::rc_layout(self.transport.guest_abi());
        let mut items = Vec::new();
        let mut notes = Vec::new();
        let mut live = std::collections::BTreeSet::new();
        // Where each mirrored channel's RC shows: the device's own host notifier page when the
        // guest's could not be mirrored (mirror.rs `rc_notifier`), else the guest's notifier.
        let mirror = self.ctrl.mirror.as_ref();
        let chans: Vec<_> = self
            .ctrl
            .channels
            .channels()
            .filter(|c| c.mirrored)
            .filter_map(|c| {
                let host = mirror.and_then(|m| m.rc_notifier(c.chid));
                (host.is_some() || c.error_notifier.is_some()).then_some((
                    c.h_client,
                    c.h_object,
                    c.chid,
                    c.engine_type,
                    c.error_notifier,
                    host,
                    c.stopped,
                ))
            })
            .collect();
        for (hc, ho, chid, eng, t, host, stopped) in chans {
            live.insert((hc, ho));
            let mut now = [0u8; crate::events::NOTIFIER_SIZE];
            let gm = crate::channel::GpuMem {
                fb: &mut self.fb,
                sys: mem,
            };
            match (host, t) {
                (Some(h), _) => now = h,
                (None, Some(t)) if gm.read(t, &mut now) => {}
                _ => continue,
            }
            let w = self
                .watch
                .entry((hc, ho))
                .or_insert(crate::events::ChannelWatch {
                    baseline: now,
                    raised: false,
                });
            if let Some((xid, info16)) = crate::events::notifier_error(w, &now) {
                if stopped && xid == crate::gspmem::XID_PREEMPTIVE_REMOVAL {
                    // The guest kernel's own notice after its STOP_CHANNEL: the new baseline.
                    w.raised = false;
                    w.baseline = now;
                    notes.push(format!(
                        "chid {chid}: PREEMPTIVE_REMOVAL after the guest's STOP_CHANNEL, the kernel's own stop notice: no RC [guest channel]"
                    ));
                    continue;
                }
                let text = format!(
                    "Caraxes: channel {chid} (engine type {eng:#x}) error, info16 {info16:#x} (host mirror)"
                );
                notes.push(format!(
                    "chid {chid}: error notifier written (exceptType {xid}, info16 {info16:#x}){}: OS_ERROR_LOG and RC_TRIGGERED to the guest [guest channel, host mirror]",
                    if host.is_some() { " on the device's host notifier page" } else { "" }
                ));
                if xid == crate::gspmem::XID_GR_EXCEPTION {
                    notes.extend(self.ctrl.gr_exception_postmortem(&gm, chid));
                }
                // RmLogonRC = 0: no Xid line (gspboot.rs, B3).
                if self.registry.log_on_rc {
                    items.push(crate::events::Unsolicited::Event(
                        crate::events::EV_OS_ERROR_LOG,
                        crate::events::os_error_log(xid, 0, chid, &text),
                    ));
                }
                items.push(crate::events::Unsolicited::Event(
                    crate::events::EV_RC_TRIGGERED,
                    crate::events::rc_triggered(layout, eng, chid, xid),
                ));
                // The device's RC journal (rc.rs) and the channel's fault record
                // (public.rs): an MMU fault's details are the host's record of the
                // channel's own mirror (a read of the device's own host channel, NV906F
                // GET_MMU_FAULT_INFO). The host keeps the record on the TSG member that
                // faulted: an empty one is looked up on the other members of the TSG.
                let fault = (xid == crate::gspmem::XID_MMU_FAULT)
                    .then(|| self.ctrl.mirror.as_ref().and_then(|m| m.mmu_fault_info_in_tsg(chid)))
                    .flatten();
                if let Some(f) = &fault {
                    notes.push(if f.from == chid {
                        format!(
                            "chid {chid}: GET_MMU_FAULT_INFO on its own host mirror channel: status {:#x} [host read]",
                            f.status
                        )
                    } else {
                        format!(
                            "chid {chid}: GET_MMU_FAULT_INFO on its own host mirror channel: status {:#x}, no fault recorded; the record of TSG member chid {} is used (status {:#x}) [host read]",
                            f.own_status, f.from, f.status
                        )
                    });
                }
                // The fault page against the guest's tables and the shadow (run P18: was the
                // page unmapped by the guest, refused, or held by the host?).
                if let (Some(f), Some(m)) = (&fault, self.ctrl.mirror.as_mut()) {
                    if f.status == 0 {
                        notes.extend(m.fault_diagnostic(&*gm.fb, chid, &f.record));
                    }
                }
                if let Some(f) = fault.filter(|f| f.status == 0) {
                    self.ctrl.guest_attrs.set_fault((hc, ho), f.record);
                }
                self.ctrl.dstate.rc.record(Some(chid), xid, text.clone());
                // The 615 operational-event import: a GOE record and its wake-up.
                let n = self.goe_rc(mem, xid, &text, &mut items);
                notes.extend(n);
            }
        }
        self.watch.retain(|k, _| live.contains(k));
        if !items.is_empty() {
            notes.extend(self.post_unsolicited(mem, &items));
        }
        notes
    }

    /// A kick the display kick page's reactor found (display/chan_page.rs): what the
    /// trapped `PUT` write did (the walk reads the pushbuffer through the BAR2 memfd, so no
    /// owed flush needs settling first, bar2area.rs `DeferredFlush`).
    pub fn display_kick_from_page(
        &mut self,
        mem: &GuestMemory,
        base: u64,
        put: u32,
    ) -> (Option<u32>, Vec<String>) {
        match self.ctrl.display.page_kick(base, put, mem, &mut self.fb) {
            Some((get, notes)) => (Some(get), notes),
            None => (None, Vec::new()),
        }
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
                "vram store: {} window map(s), {} refused, {} eviction(s), {} stall(s) without any window, {} access(es) failed closed, {} window(s) held; {} read(s) of {} bytes, {} write(s) of {} bytes",
                st.maps, st.map_failures, st.evictions, st.stalls, st.failed, st.held, st.reads, st.bytes_read, st.writes, st.bytes_written
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
                "bar1 passthrough: guest BAR1 [0, {len:#x}) on FB {fb0:#x}; {} check(s), last: {} page(s) mapped, {} matched, {} elsewhere; {} mismatched page(s) and {} page-table hazard(s) over the run, {} table re-read(s) (docs/caraxes/bar1-passthrough.md)",
                st.checks, st.mapped, st.matched, st.mismatched, st.mismatch_events, st.table_hazards, st.table_resyncs
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
        if self.ctrl.mirror.is_some() || self.ctrl.channels.tokens.is_some() {
            s.push(self.ctrl.routing.summary());
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

    /// The work of a flush point (the flush read, the MMU invalidate, the GSP
    /// command-queue doorbell): the BAR2 memfd sync, then the BAR1 pool's untracked pages.
    /// It settles a deferred flush read, counted by `cause`.
    fn flush_point(&mut self, mem: &GuestMemory, cause: crate::bar2area::Settle) {
        let root = self.bar_root(3);
        self.fb.bar2_sync(root, Some(mem));
        crate::bar1pool::sync_untracked(&mut self.fb);
        self.fb.bar2_flush_settled(cause);
    }

    /// Settle a deferred flush read if its sync is owed (a doorbell, the flush clock);
    /// returns whether one was.
    pub fn settle_deferred_flush(
        &mut self,
        mem: &GuestMemory,
        cause: crate::bar2area::Settle,
    ) -> bool {
        if !self.fb.bar2_flush_pending() {
            return false;
        }
        self.flush_point(mem, cause);
        true
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
            // By default it only marks the sync owed; the next hand-over settles it
            // (bar2area.rs `DeferredFlush`, docs/caraxes/display-pacing.md).
            self.fb.bar2_note_trapped();
            if !self.fb.bar2_defer_flush() {
                self.flush_point(mem, crate::bar2area::Settle::SyncPoint);
            }
        }
        let held = if write && bar == 1 {
            self.gp_put_barrier(mem, root, offset, data.len())
        } else {
            String::new()
        };
        let t = self.fb.bar_access(mem, root, offset, data, write);
        match t {
            Target::Fb(pa) => format!("fb {pa:#x}{held}"),
            Target::Sys(pa) => format!("sys {pa:#x}"),
            Target::Sparse => "sparse".into(),
            Target::Unmapped { level } => {
                self.bar_faults += 1;
                format!("UNMAPPED (level {level})")
            }
        }
    }

    /// The ordering rule of the doorbell (mirror.rs `gr_shadow_doorbell`) for the store
    /// that really starts the work: a guest GPPut store into a mirrored channel's USERD
    /// reaches the host channel's USERD at once, and a loaded host channel may fetch it
    /// before the doorbell. So the store waits until the shadow worker applied every plan
    /// submitted so far (display-e.md §9). With `CARAXES_ALLOC_LAZY=1` the invalidates
    /// before the store may not even be planned yet, so the store plans them first
    /// (docs/caraxes/bar1-pool.md §9). Returns the trace's note, empty when it did not wait.
    fn gp_put_barrier(&mut self, mem: &GuestMemory, root: u64, va: u64, len: usize) -> String {
        let Some(m) = self.ctrl.mirror.as_ref() else {
            return String::new();
        };
        if !m.plans_pending() {
            return String::new();
        }
        let Target::Fb(pa) = self.fb.bar_translate(root, va) else {
            return String::new();
        };
        if !stores_gp_put(&m.mirrored_userds(&self.ctrl.channels), pa, len) {
            return String::new();
        }
        let t0 = std::time::Instant::now();
        let reg = &self.ctrl.channels;
        if let Some(m) = self.ctrl.mirror.as_mut() {
            // Only the plans of the store's own shadow root (invgate.rs): another root's
            // walk in the worker does not hold this store.
            let root = m.gp_put_root(reg, pa, len);
            let mut gm = crate::channel::GpuMem {
                fb: &mut self.fb,
                sys: mem,
            };
            m.plans_barrier(reg, &mut gm, root);
        }
        format!(
            " ({GP_PUT_HELD} {} us until the shadow worker applied its plans)",
            t0.elapsed().as_micros()
        )
    }

    /// Give the device a host RM connection for the forwardable RPCs.
    pub fn with_host(mut self, host: Box<dyn HostRm>) -> Self {
        self.ctrl = Dispatcher::new(self.chip.data(), &self.identity, Some(host))
            .with_layout(self.layout);
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
        let mut notes = self.ctrl.doorbell_pool(&mut gm, token, false);
        notes.extend(self.scan_channel_errors(mem, false));
        notes
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
            fresh.ctrl = Dispatcher::new(self.chip.data(), &self.identity, Some(h))
                .with_layout(self.layout);
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
        // The persistent settings (ECC and MIG mode) survive, and what was pending applies.
        fresh.ctrl.families.after_reset(&self.ctrl.families);
        fresh.transport = self.transport;
        fresh.rpc_eventfd = self.rpc_eventfd.take();
        fresh.rpc_eventfd_stats = self.rpc_eventfd_stats;
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
    /// The PTIMER's BAR0 base (`INTERNAL_GPU_GET_CHIP_INFO` `timer_base`, 0x9000).
    fn ptimer_base(&self) -> u64 {
        self.chip.data().chip_info.timer_base as u64
    }

    /// `NV_PTIMER_TIME_0` (`hi` false: NSEC 31:5) or `NV_PTIMER_TIME_1` (`hi` true: NSEC
    /// 28:0) from the device clock, the base of the usermode TIME_0/1 (uvm.rs) and of the
    /// semaphore timestamps, so the option ROM's GOP, the VBIOS scripts and a CPU-RM see
    /// time pass in pure emulation. Torn-read guard: the RM reads TIME_1, TIME_0, TIME_1
    /// and retries on a change (tmrGetTimeEx), but a reader that takes TIME_1 then TIME_0
    /// once would see its value go back by 2^32 ns when the low word wraps in between. So a
    /// TIME_0 read that follows a TIME_1 read whose high word has since moved on answers
    /// the last low value of that high word (0xFFFF_FFE0): the pair is then a time that
    /// already passed but never older than one read before, and the next pair catches up.
    pub fn ptimer_read(&mut self, hi: bool) -> u32 {
        ptimer_word(&mut self.ptimer_hi, self.ctrl.channels.now_ns(), hi)
    }

    /// The BAR0 offset of `_INVALIDATE` (the invalidate page's trigger word, inv_page.rs).
    pub fn mmu_invalidate_register(&self) -> u64 {
        self.regs.mmu_invalidate
    }

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
            o if o == self.ptimer_base() + PTIMER_TIME_0 => Reg::PtimerTime0,
            o if o == self.ptimer_base() + PTIMER_TIME_1 => Reg::PtimerTime1,
            o if o == r.l2_flush_dirty => Reg::L2Op(L2Op::FlushDirty),
            o if r.l2_sysmem_invalidate.contains(&o) => Reg::L2Op(L2Op::SysmemInvalidate),
            o if r.l2_ops_local.contains(&o) => Reg::L2Op(L2Op::Local),
            _ => return None,
        })
    }

    /// Whether the BAR1 pool's poll after a trapped BAR0 write at `off` merges the
    /// replicas (bar1pool.rs `poll_at`). Not after a write that only latches a value or
    /// whose own handler merged just before the poll (docs/caraxes/bar1-pool.md §11):
    /// - the MMU invalidate's PDB and upper PDB (latched; the trigger follows);
    /// - the MMU invalidate trigger (its handler runs `sync_untracked` before the re-sync);
    /// - the usermode doorbell (not a sync point, bar1-pool.md §10; its handler forwards
    ///   the doorbell before the poll, so a merge there orders nothing for it);
    /// - the PTIMER time registers (the write is dropped);
    /// - the GSP command queue's head, `NV_PGSP_QUEUE_HEAD(0)` (the command-queue doorbell:
    ///   its handler runs `sync_untracked` itself before the RPC, bar1-pool.md §12);
    /// - the GSP status queue's tail, `NV_PGSP_MSGQ_TAIL(0)` (the guest's read pointer:
    ///   it hands the device nothing to read).
    ///
    /// Every other BAR0 write (the L2 flushes and invalidates, the display, the other GSP
    /// registers, anything unknown) merges, as before.
    /// The GSP queue pointer registers of this chip's firmware model, when it has them.
    pub fn gsp_queue_registers(&self) -> Option<crate::chip::QueueRegisters> {
        self.fw.queue_registers()
    }

    pub fn pool_merge_after(&self, off: u64) -> bool {
        if self
            .fw
            .queue_registers()
            .is_some_and(|q| off == q.cmd_head || off == q.msg_tail)
        {
            return false;
        }
        !matches!(
            self.reg(off),
            Some(
                Reg::MmuInvalidatePdb
                    | Reg::MmuInvalidateUpperPdb
                    | Reg::MmuInvalidate
                    | Reg::UsermodeDoorbell
                    | Reg::PtimerTime0
                    | Reg::PtimerTime1
            )
        )
    }

    /// Start the L2 cache operation `op` (the guest wrote `PENDING_BUSY`). With host
    /// execution on, the guest's work runs on the physical GPU and its sysmem lines sit
    /// in the physical L2, so FLUSH_DIRTY and SYSMEM_INVALIDATE are honoured there with
    /// `FB_FLUSH_GPU_CACHE` before the register reports completion. Without host
    /// execution there is no cache behind the model and the operation completes at once.
    /// A host failure is logged and still reported complete: the guest RM cannot act on
    /// a timeout either (it logs and continues), and a wedged poll costs 30 s per op.
    fn l2_op(&mut self, op: L2Op) -> String {
        use crate::hostabi::L2Flush;
        self.l2_op_counts.0 += 1;
        let flush = match op {
            L2Op::FlushDirty => L2Flush::WriteBack,
            // The host RM promotes a sysmem invalidate to an evict (kmemsysCacheOp_GM200).
            L2Op::SysmemInvalidate => L2Flush::Invalidate,
            L2Op::Local => return format!("l2 {op:?}: complete (no host operation)"),
        };
        if self.exec.is_none() {
            return format!("l2 {op:?}: complete (no host execution, no cache behind the model)");
        }
        let t = std::time::Instant::now();
        match self.ctrl.host_flush_gpu_cache(flush) {
            Some((flags, 0)) => {
                self.l2_op_counts.1 += 1;
                format!(
                    "l2 {op:?}: FB_FLUSH_GPU_CACHE flags {:#x} NV_OK in {} us [host GPU]",
                    flags,
                    t.elapsed().as_micros()
                )
            }
            Some((flags, st)) => {
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
            // TRIGGER (bit 31) reads back as done (fb-model.md §4d), except while a held
            // invalidate's shadow plan is still being applied (invgate.rs, execution.md §8):
            // the guest polls it before its map call returns.
            Some(Reg::MmuInvalidate) => {
                let busy = self.ctrl.mirror.as_mut().is_some_and(|m| m.invalidate_busy());
                (self.mmu_invalidate.2 & !(1 << 31)) | if busy { 1 << 31 } else { 0 }
            }
            // Write-only doorbell (`clc361.h:33`); reads return 0.
            Some(Reg::UsermodeDoorbell) => 0,
            Some(Reg::PtimerTime0) => self.ptimer_read(false),
            Some(Reg::PtimerTime1) => self.ptimer_read(true),
            // The operation completed when the write started it: PENDING and
            // OUTSTANDING read EMPTY/FALSE (execution.md §6, "L2 cache operations").
            Some(Reg::L2Op(_)) => self.l2_ops.get(&off).copied().unwrap_or(0) & !L2_OP_BUSY,
            None => {
                // SWGEN0 of an unsolicited message (events.rs), as display_read32 shows
                // it for the display's events.
                let v = self.fw.read32(off)?;
                return Some(match off {
                    GSP_IRQSTAT if self.gsp_swgen0 => v | GSP_SWGEN0,
                    GSP_IRQMASK | GSP_IRQDEST | GSP_RISCV_IRQMASK | GSP_RISCV_IRQDEST
                        if self.events_posted =>
                    {
                        v | GSP_SWGEN0
                    }
                    _ => v,
                });
            }
        })
    }

    /// A latched write to a register the firmware model watches (`NV_PMC_ENABLE`):
    /// `old` and `new` are what it read before and after. Notes for the trace.
    pub fn observe_write(&mut self, off: u64, old: u32, new: u32) -> Vec<String> {
        self.fw.observe_write(off, old, new)
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
        if off == GSP_IRQSCLR && v & GSP_SWGEN0 != 0 {
            self.gsp_swgen0 = false;
        }
        notes.extend(self.scan_channel_errors(mem, false));
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
            // One time base with the usermode TIME_0/1 and the GSP OS time (public.rs):
            // setting it would move the one and not the others.
            Some(Reg::PtimerTime0 | Reg::PtimerTime1) => {
                notes.push("PTIMER time is the device clock: write dropped".into())
            }
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
                notes.extend(self.mmu_invalidate_write(v, mem));
            }
            None => {
                notes.extend(self.queue_register_write(off, v, mem));
                let w = self.fw.write32(off, v, mem)?;
                notes.extend(w.notes);
                match w.event {
                    Some(BootEvent::BooterUnload { arg }) => {
                        // B5: nothing to save; the device's state lives in its process.
                        self.sr_meta_pa = Some(arg);
                        notes.push(format!(
                            "booter unload: argument {arg:#x}{}",
                            if arg != 0 { " (GspFwSRMeta of a suspend: recorded, nothing to save)" } else { "" }
                        ));
                    }
                    Some(BootEvent::WprMeta { pa }) if self.resuming(mem) => {
                        notes.extend(self.resume(mem, pa));
                    }
                    Some(BootEvent::WprMeta { pa }) => {
                        self.suspended = None;
                        self.wpr_meta_pa = Some(pa);
                        // A Booter Load that is not a resume starts a new GSP-RM: the CPU
                        // built fresh libos arguments and queues (a driver re-probe without a
                        // function-level reset, as nova-core does, Linux 7.2.7 gsp.rs
                        // `Gsp::new`, gsp/cmdq.rs `Cmdq::new`), so the old link goes and the
                        // first doorbell links again. Message queue v2 relinks in
                        // `gsp_started` anyway.
                        if self.transport != GspTransport::MsgqV2 && self.queues.is_some() {
                            self.queues = None;
                            self.pending = None;
                            notes.push("queues: the previous boot's link dropped (new Booter Load); the next doorbell links again".into());
                        }
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
                        self.flush_point(mem, crate::bar2area::Settle::SyncPoint);
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

    /// A write of `_INVALIDATE` (trapped, or found by the invalidate page's reactor,
    /// inv_page.rs): with TRIGGER (bit 31) the device runs the invalidate now.
    fn mmu_invalidate_write(&mut self, v: u32, mem: &GuestMemory) -> Vec<String> {
        let mut notes = Vec::new();
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
            // The BAR1 area's untracked pages (bar1pool.rs `sync_untracked`): the
            // replicas' guest writes reach their pages as tracked writes, and a
            // table page still in the pool is re-read by the walks below. Settles
            // a deferred flush read.
            self.flush_point(mem, crate::bar2area::Settle::SyncPoint);
            // The guest changed page tables in VRAM through BAR1; drain the WC
            // buffers before the host TLB invalidate so RM re-walks fresh tables.
            self.fb.flush();
            // The root watch (pdbwatch.rs, fix 2 of run P17): the guest names the root it uses.
            let named = (v & 2 == 0).then(|| {
                crate::pdbwatch::register_pdb(self.mmu_invalidate.0, self.mmu_invalidate.1)
            });
            self.ctrl.channels.note_guest_invalidate(named);
            let forwarded = self.ctrl.mirror_invalidate(&mut self.fb, Some(pdb));
            // Shadow VA spaces (every channel's with the pool) are not host VA spaces
            // the invalidate is forwarded to: the re-sync below brings them in line.
            let shadowed = self.ctrl.mirror.as_ref().is_some_and(|m| {
                let roots = m.shadow_roots();
                if v & 2 != 0 {
                    !roots.is_empty()
                } else {
                    roots.contains(&pdb)
                }
            });
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
                } else if shadowed {
                    " -> the shadow VA space(s) re-synced from the guest's tables".into()
                } else {
                    " (no mirror or shadow VA space rooted there; nothing cached here)".into()
                }
            ));
        }
        notes
    }

    /// The invalidate page's trigger (`CARAXES_INVALIDATE_PAGE=1`, inv_page.rs): the three
    /// words as the guest stored them in memory, applied as the trapped writes are.
    pub fn mmu_invalidate_from_page(
        &mut self,
        mem: &GuestMemory,
        pdb: u32,
        upper: u32,
        v: u32,
    ) -> Vec<String> {
        self.mmu_invalidate.0 = pdb;
        self.mmu_invalidate.1 = upper;
        let notes = self.mmu_invalidate_write(v, mem);
        // A held invalidate completes through the trigger's busy bit (invgate.rs,
        // execution.md §8): the trapped trigger read reports TRIGGER until the shadow
        // worker applied the plan. The page clears TRIGGER as soon as this returns
        // (inv_page.rs `complete`), so the gate is waited out here, under the device lock
        // (each `invalidate_busy` waits at most 50 us; past 1 s it waits to the end).
        while self.ctrl.mirror.as_mut().is_some_and(|m| m.invalidate_busy()) {}
        notes
    }

    /// `CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1`: a BAR2 memfd sync before every doorbell (the
    /// reference for the rule that the guest's flush and invalidate are enough,
    /// docs/caraxes/bar2-memfd.md §3).
    ///
    /// A flush read whose sync is still owed is settled here first: the channel may fetch
    /// what the guest wrote before the doorbell (bar2-memfd.md §3, rule 3).
    fn bar2_doorbell_sync(&mut self, mem: &GuestMemory) {
        if self.settle_deferred_flush(mem, crate::bar2area::Settle::Doorbell) {
            return;
        }
        if self.fb.bar2_area().is_some_and(|a| a.sync_doorbell) {
            let root = self.bar_root(3);
            self.fb.bar2_sync(root, Some(mem));
        }
    }

    /// A doorbell the usermode page's reactor found (um_page.rs), or its watchdog
    /// (`hint` None): every channel whose GPPut advanced is rung, as the token-less
    /// ioeventfd path rings them. The watchdog first checks without the BAR2 sync that a
    /// doorbell pays, and does nothing when no channel is pending. Returns the channels
    /// rung and the notes.
    pub fn doorbell_from_page(&mut self, mem: &GuestMemory, hint: Option<u32>) -> (usize, Vec<String>) {
        if hint.is_none() {
            let gm = crate::channel::GpuMem {
                fb: &mut self.fb,
                sys: mem,
            };
            let any = self
                .ctrl
                .channels
                .channels()
                .map(|c| c.chid)
                .collect::<Vec<_>>()
                .into_iter()
                .any(|c| self.ctrl.channels.pending(&gm, c));
            if !any {
                return (0, Vec::new());
            }
        }
        let notes = self.doorbell_pending(mem);
        let rung = notes.first().map_or(0, |n| crate::um_page::pending_count(n));
        (rung, notes)
    }

    /// A token-less doorbell from the ioeventfd path (device.rs): ring every channel with
    /// newly published GPFIFO entries (`Dispatcher::doorbell_pending`).
    pub fn doorbell_pending(&mut self, mem: &GuestMemory) -> Vec<String> {
        self.bar2_doorbell_sync(mem);
        let mut gm = crate::channel::GpuMem {
            fb: &mut self.fb,
            sys: mem,
        };
        let mut notes = self.ctrl.doorbell_pending(&mut gm);
        notes.extend(self.scan_channel_errors(mem, false));
        notes
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
                        ReplyTo::Itself => Some((msg.header, msg.gmcapi)),
                        ReplyTo::PendingRequest => {
                            self.pending.take().map(|(p, _)| (p.header, p.gmcapi))
                        }
                        ReplyTo::Nobody => None,
                    };
                    if let Some((h, gmcapi)) = target {
                        let reply_sequence = gmcapi
                            .as_ref()
                            .map_or(u64::from(h.sequence), |g| g.sequence);
                        let sent = match gmcapi {
                            Some(g) => queues.send_gmcapi_response(&qm, &g, rej.status, &[]),
                            None => queues.send(&qm, &RpcHeader::reply(&h, rej.status, 0), &[]),
                        };
                        match sent {
                            Ok(seq) => notes.push(format!(
                                "tx seq {seq} reply {} sequence {} result {:#x} (rejected request)",
                                rpc::function_name(h.function),
                                reply_sequence,
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
                        // A page list above maxRpcSize arrives split too (_issuePteDescRpc,
                        // rpc.c:2334-2347 of 615): the fixed part plus pteDesc.length PFNs.
                        crate::gspmem::FN_ALLOC_MEMORY => {
                            crate::gspmem::alloc_memory_size(&msg.payload).unwrap_or(0)
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
                let sequence = msg
                    .gmcapi
                    .as_ref()
                    .map_or(u64::from(h.sequence), |g| g.sequence);
                let integrity = if self.transport == GspTransport::MsgqV2 {
                    "framing"
                } else {
                    "checksum"
                };
                let t_rpc = crate::profile::on().then(std::time::Instant::now);
                notes.push(format!(
                    "rx seq {} {} ({}) sequence {} length {} {integrity} {}: {}",
                    msg.seq_num,
                    rpc::function_name(h.function),
                    h.function,
                    sequence,
                    h.length,
                    if msg.checksum_ok { "ok" } else { "BAD" },
                    rpc::describe(h.function, &msg.payload)
                ));
                // B7: a 615 client that did not opt in to the unstable RPC interface
                // speaks GMCAPI only (gmcapi_system_info.h:200-203; boot-exchange.md
                // §3.4): its RM RPCs are refused.
                if msg.gmcapi.is_none() && self.unstable.is_some_and(|o| !o.rpc) {
                    notes.push(format!(
                        "{}: refused, the client did not opt in to RM RPCs (NVGMC_SI_UNSTABLE_INTERFACE_SUPPORT)",
                        rpc::function_name(h.function)
                    ));
                    match queues.send(&qm, &RpcHeader::reply(&h, rpc::NV_ERR_NOT_SUPPORTED, 0), &[]) {
                        Ok(seq) => notes.push(format!("tx seq {seq} reply result {:#x}", rpc::NV_ERR_NOT_SUPPORTED)),
                        Err(e) => notes.push(format!("send: {e}")),
                    }
                    continue;
                }
                let reply: Option<(u32, Vec<u8>)> = match h.function {
                    rpc::FN_GSP_SET_SYSTEM_INFO => {
                        // BAR addresses for BUS_GET_PCI_BAR_INFO come from here.
                        if let Some(bars) = rpc::system_info_bar_addresses(&msg.payload) {
                            self.ctrl.bar_offsets = bars;
                        }
                        self.system_info = Some(msg.payload.clone());
                        None
                    }
                    rpc::FN_SET_REGISTRY => {
                        let entries = rpc::packed_registry(&msg.payload);
                        for (name, value) in &entries {
                            notes.push(format!("registry {name} = {value:?}"));
                        }
                        self.guest_key_warnings(&entries, &mut notes);
                        None
                    }
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
                        let flags = crate::gspboot::suspend_flags(&msg.payload);
                        self.suspended = Some(flags);
                        let m0 = self.fw.read32(GSP_MAILBOX0).unwrap_or(0);
                        self.fw.write32(GSP_MAILBOX0, m0 | GSP_PROCESSOR_SUSPENDED, mem);
                        notes.push(format!(
                            "gmcapi GSP_SUSPEND flags {}: no response (fire and forget); MAILBOX0 {m0:#x} -> {:#x} (processor suspended)",
                            crate::gspboot::describe_suspend(flags),
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
                        let t_ctl = crate::profile::on().then(std::time::Instant::now);
                        // Controls only newer chips' kernels send come from the chip first
                        // (chip/physical.rs), the rest from the dispatcher.
                        // The event-buffer controls on GSP-side event buffers (gspmem.rs).
                        let gm = self
                            .gspmem
                            .control(w(v::RM_CONTROL_H_CLIENT), h_object, cmd, params)
                            .map(|r| crate::ctrl::Answer {
                                status: r.status,
                                params: r.params,
                                label: "guest (GSP-side event buffer, gspmem.rs)",
                                note: r.note,
                            });
                        let a = match gm
                            .or_else(|| crate::chip::physical::answer(self.chip.data(), cmd, params))
                        {
                            Some(a) => a,
                            None => {
                                self.ctrl
                                    .control(w(v::RM_CONTROL_H_CLIENT), h_object, cmd, params)
                            }
                        };
                        if let Some(t) = t_ctl {
                            crate::profile::record(
                                crate::profile::Key::Control(cmd),
                                t.elapsed(),
                                0,
                                Some(a.status),
                                a.label,
                            );
                        }
                        notes.push(format!(
                            "ctrl {cmd:#x}: {} -> status {:#x} [{}]",
                            a.note, a.status, a.label
                        ));
                        if a.label == "unanswered" {
                            self.unanswered.push(a.note.clone());
                        }
                        if crate::rusd::enabled() {
                            let now = self.ctrl.channels.now_ns();
                            if cmd == v::CMD_INTERNAL_INIT_USER_SHARED_DATA {
                                let pa = params
                                    .get(v::RUSD_PHYS_ADDR..v::RUSD_PHYS_ADDR + 8)
                                    .map_or(0, |b| u64::from_le_bytes(b.try_into().unwrap()));
                                let fb = self.ctrl.profile.fb_bytes;
                                let f = crate::rusd::Facts {
                                    bar1_kib: 0x4_0000,
                                    bar1_avail_kib: 0x3_9F80,
                                    pma_total: fb,
                                    pma_free: fb - fb / 8,
                                };
                                notes.push(self.rusd.init(mem, pa, now, &f));
                            } else if cmd == v::CMD_INTERNAL_USER_SHARED_DATA_SET_DATA_POLL {
                                let mask = params
                                    .get(..8)
                                    .map_or(0, |b| u64::from_le_bytes(b.try_into().unwrap()));
                                self.rusd.set_poll(mask);
                                notes.push(format!("rusd: poll mask {mask:#x}"));
                            }
                            self.rusd.refresh(mem, now);
                        }
                        let mut reply = p[..at.min(p.len())].to_vec();
                        reply.resize(at, 0);
                        let st = v::RM_CONTROL_STATUS;
                        reply[st..st + 4].copy_from_slice(&a.status.to_le_bytes());
                        if cmd & v::CMD_GSS_LEGACY_BIT != 0 {
                            // GSS legacy: the guest caches the reply by its rmctrlFlags
                            // (rpc.c:10899-10905 of 615): CACHEABLE for the proven static
                            // answers when CARAXES_LEGACY_CACHEABLE=1, 0 for the rest and
                            // by default (nvml.rs `legacy_reply_flags`); with the flag the
                            // access right is written 0.
                            let flags = crate::nvml::legacy_reply_flags(cmd, a.status);
                            let f = v::RM_CONTROL_RMCTRL_FLAGS;
                            reply[f..f + 4].copy_from_slice(&flags.to_le_bytes());
                            if flags != 0 {
                                let r = v::RM_CONTROL_RMCTRL_ACCESS_RIGHT;
                                reply[r..r + 4].copy_from_slice(&0u32.to_le_bytes());
                                notes.push(format!("ctrl {cmd:#x}: reply rmctrlFlags {flags:#x} (the guest caches it)"));
                            }
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
                        let t_alloc = crate::profile::on().then(std::time::Instant::now);
                        // The event-buffer classes the 615 event import allocates on GSP
                        // (gspmem.rs) are checked against the registered memory first.
                        // Classes GSP-RM would refuse are refused first (classes.rs).
                        let under_channel = self
                            .ctrl
                            .channels
                            .channels()
                            .any(|c| c.h_client == h_client && c.h_object == h_parent);
                        let refused =
                            crate::classes::refusal(self.chip.data(), h_class, under_channel);
                        let a = match refused {
                            Some((status, note)) => crate::ctrl::Answer {
                                status,
                                params: params.to_vec(),
                                label: "synthesized (refused as GSP-RM refuses it, classes.rs)",
                                note,
                            },
                            None => match self
                            .gspmem
                            .alloc(h_client, h_parent, h_object, h_class, params)
                        {
                            Some(r) => crate::ctrl::Answer {
                                status: r.status,
                                params: r.params,
                                label: "guest (GSP-side event buffer, gspmem.rs)",
                                note: r.note,
                            },
                            None => self.ctrl.alloc(h_client, h_object, h_parent, h_class, params),
                        },
                        };
                        if let Some(t) = t_alloc {
                            crate::profile::record(
                                crate::profile::Key::Alloc(h_class),
                                t.elapsed(),
                                0,
                                Some(a.status),
                                a.label,
                            );
                            // A guest client's root: one per process that opens the GPU.
                            if h_class == v::NV01_ROOT && a.status == rpc::NV_OK {
                                crate::profile::client_open(h_client);
                            }
                        }
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
                        // A video object the mirror deferred (mirror.rs
                        // `place_video_object`): placed on the host now.
                        let mut a = a;
                        if a.status == rpc::NV_OK {
                            let (n, refused) = self
                                .ctrl
                                .place_video_object(h_client, h_parent, h_object, h_class);
                            notes.extend(n);
                            if refused {
                                // Fail closed: the guest's encoder/decoder open fails cleanly.
                                a.status = crate::rpc::NV_ERR_NOT_SUPPORTED;
                            }
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
                        // Shadow VA spaces the free handed to the teardown: drop their
                        // cached walks (docs/caraxes/host-resources.md).
                        if let Some(m) = self.ctrl.mirror.as_mut() {
                            for r in m.take_reaped_roots() {
                                self.fb.unwatch_tables(crate::fb::Target::Fb(r));
                            }
                        }
                        // Set-B ranges held until a host TSG's free (mirror.rs `held_at_free`)
                        // went back to set A: map the guest's pages there now, after the host
                        // context at those VAs is gone.
                        {
                            let mut gm = crate::channel::GpuMem {
                                fb: &mut self.fb,
                                sys: mem,
                            };
                            notes.extend(self.ctrl.mirror_resync_released(&mut gm));
                        }
                        if h_object == h_client {
                            crate::profile::client_close(h_client);
                        }
                        notes.push(format!("{} [{}]", a.note, a.label));
                        if let Some(n) = self.gspmem.free(h_client, h_object) {
                            notes.push(format!("free {h_client:#x}/{h_object:#x}: {n} [guest]"));
                        }
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
                    crate::gspmem::FN_ALLOC_MEMORY => {
                        // rpc_alloc_memory_v13_01: GSP-RM creates a memory-list object under
                        // the same handles (mem.c:513-585 of 615); the kernel reads only the
                        // status (rpc.c:3486-3530). Recorded, pages checked (gspmem.rs).
                        let fb_bytes = self.fb_bytes();
                        let (st, note) = self.gspmem.alloc_memory(&msg.payload, mem, fb_bytes);
                        notes.push(note);
                        // One record back, whatever the request's length: the send is
                        // not bidirectional (rpc.c:2362), a continuation would be unread.
                        let n = msg.payload.len().min(crate::gspmem::AM_SIZE);
                        Some((st, msg.payload[..n].to_vec()))
                    }
                    crate::gsprpc::FN_IDLE_CHANNELS => {
                        // Resolved through the channel registry, idle when USERD GPGet
                        // caught up with GPPut; mirrored channels through the host RM's
                        // idle on the mirror's own host objects (gsprpc.rs; rpc-functions.md
                        // §4.2).
                        let chans = &self.ctrl.channels;
                        let mirror = self.ctrl.mirror.as_ref();
                        let fb = &mut self.fb;
                        let host_idle = |l: &[(u32, u32)], f: u32, t: u32| {
                            mirror.and_then(|m| m.idle_channels(chans, l, f, t))
                        };
                        let (st, note) = crate::gsprpc::idle_channels(&msg.payload, |c, h| {
                            let ch = chans.channels().find(|x| x.h_client == c && x.h_object == h)?;
                            let word = |off: u64| match ch.userd {
                                Some(crate::fb::Target::Fb(pa)) => Some(crate::fb::Target::Fb(pa + off)),
                                Some(crate::fb::Target::Sys(pa)) => Some(crate::fb::Target::Sys(pa + off)),
                                _ => None,
                            };
                            let gm = crate::channel::GpuMem { fb: &mut *fb, sys: mem };
                            // NV_RAMUSERD GP_GET 0x88, GP_PUT 0x8C (clc46f.h:61-62).
                            let (get, put) = match (word(0x88), word(0x8C)) {
                                (Some(g), Some(p)) => (gm.read_u32(g).unwrap_or(0), gm.read_u32(p).unwrap_or(0)),
                                _ => (0, 0),
                            };
                            Some(crate::gsprpc::ChannelIdle {
                                chid: ch.chid,
                                mirrored: ch.mirrored,
                                gp_get: get,
                                gp_put: put,
                            })
                        }, host_idle);
                        notes.push(note);
                        Some((st, msg.payload.clone()))
                    }
                    crate::gsprpc::FN_GPU_EXEC_REG_OPS => {
                        // The device's own identity registers only; never a host register.
                        let id = self.chip.data().identity;
                        let (st, out, note) = crate::gsprpc::exec_reg_ops(&msg.payload, |off| match off {
                            0x0 => Some(id.boot_0),
                            0x4 => Some(id.boot_1),
                            0xA00 => Some(id.boot_42),
                            _ => None,
                        });
                        notes.push(note);
                        Some((st, out))
                    }
                    crate::gsprpc::FN_DUMP_PROTOBUF_COMPONENT => {
                        let (st, out, note) = crate::gsprpc::dump_protobuf_component(&msg.payload);
                        notes.push(note);
                        Some((st, out))
                    }
                    crate::gsprpc::FN_ECC_NOTIFIER_WRITE_ACK => {
                        // Asynchronous, no reply (rpc.c:11166-11184 of 615): the kernel's
                        // answer to ECC_POSSIBLE_ERR_FAST_PATH, which the device never sends.
                        notes.push("ecc notifier write ack: consumed, no reply (asynchronous) [guest]".into());
                        None
                    }
                    crate::gsprpc::FN_MAP_MEMORY_DMA | crate::gsprpc::FN_UNMAP_MEMORY_DMA => {
                        let (st, note) = crate::gsprpc::map_memory_dma(h.function);
                        notes.push(note);
                        Some((st, msg.payload.clone()))
                    }
                    crate::gsprpc::FN_UNLOADING_GUEST_DRIVER => {
                        // The 610 and nova-core (r570) unload: the facts of GSP_SUSPEND as
                        // an RPC (rpc_unloading_guest_driver_v1F_07: bInPMTransition,
                        // bGc6Entering, newLevel), answered NV_OK, then the same
                        // processor-suspended bit (rpc-functions.md §4.3).
                        let p = &msg.payload;
                        let m0 = self.fw.read32(GSP_MAILBOX0).unwrap_or(0);
                        self.fw.write32(GSP_MAILBOX0, m0 | GSP_PROCESSOR_SUSPENDED, mem);
                        notes.push(format!(
                            "unloading guest driver: bInPMTransition {} bGc6Entering {} newLevel {}; MAILBOX0 {m0:#x} -> {:#x} (processor suspended) [guest]",
                            p.first().copied().unwrap_or(0),
                            p.get(1).copied().unwrap_or(0),
                            u32::from_le_bytes(p.get(4..8).and_then(|s| s.try_into().ok()).unwrap_or([0; 4])),
                            m0 | GSP_PROCESSOR_SUSPENDED
                        ));
                        Some((rpc::NV_OK, msg.payload.clone()))
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
                    if let Some(g) = msg.gmcapi {
                        // A GMCAPI response is one MCTP/NVDM element (possibly spanning
                        // several queue slots), never an RM continuation series. Preserve
                        // the request's full 64-bit sequence for the kernel's waiter.
                        match queues.send_gmcapi_response(&qm, &g, result, &payload) {
                            Ok(seq) => notes.push(format!(
                                "tx seq {seq} reply {} sequence {} result {result:#x} length {}",
                                crate::gmcapi::command_name(g.id()),
                                g.sequence,
                                crate::gmcapi::HEADER_SIZE + payload.len()
                            )),
                            Err(e) => notes.push(format!("send: {e}")),
                        }
                    } else {
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
                if let Some(t) = t_rpc {
                    crate::profile::record(
                        crate::profile::Key::Rpc(h.function),
                        t.elapsed(),
                        0,
                        None,
                        "",
                    );
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
        // GSP_INIT_DONE: function 0x1001, sequence 0, rpc_result 0, bIsD3Hot = 0; for a 570
        // guest (nova-core) an empty GSP_RUN_CPU_SEQUENCER first (ver::link_events).
        for (function, payload) in crate::ver::link_events(crate::ver::older_guest()) {
            let sent = {
                let qm = QueueMemory::new(mem, &pages);
                queues.send(&qm, &RpcHeader::event(function, payload.len()), &payload)
            };
            let name = rpc::function_name(function);
            match sent {
                Ok(seq) => notes.push(format!("tx seq {seq} event {name} ({} bytes)", payload.len())),
                Err(e) => notes.push(format!("send {name}: {e}")),
            }
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

    /// The BAR0 offset of the command-queue head register (`NV_PGSP_QUEUE_HEAD(0)`,
    /// 0x110C00 on TU10x through GB20x, kern_gpu_tu102.c:596-605 of 615).
    pub fn cmd_head_register(&self) -> Option<u64> {
        self.fw.queue_registers().map(|r| r.cmd_head)
    }

    /// Take the head writes through the ioeventfd from here as well
    /// (`CARAXES_RPC_IOEVENTFD=1`): `fd` is a duplicate of the reactor's eventfd; it is
    /// made non-blocking (the flag is the shared file description's, which the polling
    /// reactor sets as well).
    pub fn with_rpc_eventfd(mut self, fd: std::fs::File) -> std::io::Result<Self> {
        use std::os::fd::AsRawFd;
        // SAFETY: plain fcntl on an open descriptor.
        unsafe {
            let fl = libc::fcntl(fd.as_raw_fd(), libc::F_GETFL);
            if fl < 0 || libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, fl | libc::O_NONBLOCK) < 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        self.rpc_eventfd = Some(fd);
        Ok(self)
    }

    /// The head writes counted by the eventfd since it was last read (0 without one).
    pub fn take_rpc_writes(&mut self) -> u64 {
        use std::io::Read;
        let Some(f) = self.rpc_eventfd.as_mut() else {
            return 0;
        };
        let mut b = [0u8; 8];
        match f.read(&mut b) {
            Ok(8) => {
                let n = u64::from_ne_bytes(b);
                self.rpc_eventfd_stats.taken_inline += n;
                n
            }
            _ => 0,
        }
    }

    /// One command-queue head write that came through the ioeventfd, whose value the
    /// device did not see: the value the trapped write would have carried, for
    /// [`BootModel::write32`] to serve exactly as it serves a trapped one, or `None` when
    /// the write publishes nothing to serve. With it the note for the trace.
    ///
    /// - Message queue v0 (610): the register is a plain doorbell, the CPU writes 0
    ///   (kernel_gsp.c:424 of 610) and the pointer lives in shared memory: 0.
    /// - Message queue v2 (615), not linked: `msgqTxCreate`'s 0 (msgq.c:176), before
    ///   GSP-RM starts; the link starts from 0 anyway.
    /// - v2, linked: the element at our read pointer decides
    ///   ([`crate::msgq::v2::QueueStateV2::peek_published`]): read pointer plus its slots,
    ///   or 0 for a queue created anew, which drops the link as the trapped 0 does.
    pub fn cmd_head_from_ring(&mut self, mem: &GuestMemory) -> (Option<u32>, String) {
        use crate::msgq::v2::Published;
        self.rpc_eventfd_stats.writes += 1;
        if self.transport != GspTransport::MsgqV2 {
            self.rpc_eventfd_stats.elements += 1;
            return (Some(0), "610 doorbell (pointer in shared memory)".into());
        }
        let peek = {
            let pages = &self.pages;
            match self.queues.as_ref().and_then(|q| q.v2()) {
                None => None,
                Some(q) => {
                    let qm = QueueMemory::new(mem, pages);
                    Some((q.read_ptr, q.peek_published(&qm)))
                }
            }
        };
        match peek {
            None => {
                self.rpc_eventfd_stats.before_link += 1;
                self.cpu_cmd_head = 0;
                (None, "before the queue link: msgqTxCreate's QUEUE_HEAD = 0 (msgq.c:176)".into())
            }
            Some((rp, Published::Element { slots })) => {
                self.rpc_eventfd_stats.elements += 1;
                let head = rp.wrapping_add(slots);
                (Some(head), format!("element {rp}, {slots} slot(s): QUEUE_HEAD {head}"))
            }
            Some((0, Published::Stale(why))) => (None, format!("nothing published ({why}), read pointer 0: ignored")),
            Some((_, Published::Stale(why))) => {
                self.rpc_eventfd_stats.recreated += 1;
                (Some(0), format!("nothing published ({why}): msgqTxCreate's QUEUE_HEAD = 0, the command queue created anew"))
            }
            Some((_, Published::Failed)) => (None, "queue failed: not served".into()),
        }
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
        // Head writes through the ioeventfd not taken yet were made before this link:
        // msgqTxCreate's 0 (msgq.c:176); the CPU publishes nothing before it sees our
        // status header (kernel_gsp_tu102.c:463-474 of 615).
        let early = self.take_rpc_writes();
        if early > 0 {
            self.rpc_eventfd_stats.writes += early;
            self.rpc_eventfd_stats.before_link += early;
            self.cpu_cmd_head = 0;
            notes.push(format!("rpc ioeventfd: {early} head write(s) before the link (msgqTxCreate's 0)"));
        }
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

    /// The guest registry keys the device needs (gspboot.rs `missing_guest_keys`): each
    /// warning goes to the trace and to the device log, where a boot check reads it.
    fn guest_key_warnings(&self, entries: &[(String, crate::gmcapi::RegValue)], notes: &mut Vec<String>) {
        // The keys are OpenRM's module options; nova-core (a 570 guest) sends its own three
        // (Linux 7.2.7 gsp/commands.rs `SetRegistry`) and runs no CeUtils or watchdog.
        if crate::ver::older_guest() == Some(570) {
            return;
        }
        // With compression per VM slice a Linux guest needs scrub on free, the reverse of
        // the quickstart key (compression.rs `guest_key_warnings`).
        let slice = crate::compression::GuestView::of(&self.layout, self.chip.data()).is_some();
        let compression = if slice {
            crate::compression::guest_key_warnings(entries)
        } else {
            Vec::new()
        };
        for w in crate::gspboot::missing_guest_keys(entries)
            .into_iter()
            .filter(|w| !(slice && w.contains("RMDisableScrubOnFree")))
            .chain(compression)
        {
            eprintln!("vfio-user-nvgpu: {w}");
            notes.push(w);
        }
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
        let (policy, honoured) = crate::gspboot::registry_policy(&si.registry);
        notes.extend(honoured);
        self.guest_key_warnings(&si.registry, notes);
        self.registry = policy;
        let opt = crate::gspboot::Unstable::from_key(si.unstable);
        notes.push(format!(
            "unstable interfaces: key {:?}, RM RPCs {}, RM events {}",
            si.unstable,
            if opt.rpc { "opted in" } else { "NOT opted in: refused" },
            if opt.events { "opted in" } else { "NOT opted in: not sent" }
        ));
        self.unstable = Some(opt);
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
        let extra = rpc::StaticExtra {
            wpr_offsets: self.wpr.as_ref().map(|w| (w.non_wpr_heap_offset, w.frts_offset)),
            sku: self.identity.sku.as_ref(),
        };
        let (out, keys) = rpc::static_info_nvkv(&image, self.chip.data(), &extra);
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

    /// Compression per VM slice turns the quickstart's scrub key from a requirement into a
    /// warning (compression.rs `guest_key_warnings`); off keeps today's warnings.
    #[test]
    fn guest_key_warnings_under_compression() {
        use crate::gmcapi::RegValue;
        let f = crate::fixture::rtx2070();
        let l = crate::fb::FbLayout::reserved(4 << 30, 8 << 30, 0xE900_0000, 0x1_E900_0000).unwrap();
        let key = |k: &str, x: u32| (k.to_string(), RegValue::U32(x));
        let quickstart = vec![
            key("RmDisableGlobalCeUtils", 1),
            key("RMDisableScrubOnFree", 1),
            key("RmRcWatchdog", 0),
        ];
        let scrubbing = vec![key("RmDisableGlobalCeUtils", 1), key("RmRcWatchdog", 0)];
        let warn = |layout: crate::fb::FbLayout, e: &[(String, RegValue)]| {
            let b = BootModel::new(f.chip, layout, f.identity(4 << 30));
            let mut notes = Vec::new();
            b.guest_key_warnings(e, &mut notes);
            notes
        };
        assert!(warn(l, &quickstart).is_empty());
        let w = warn(l, &scrubbing);
        assert!(w.len() == 1 && w[0].contains("RMDisableScrubOnFree is absent"), "{w:?}");
        let on = l.with_compression(true).unwrap();
        let w = warn(on, &quickstart);
        assert!(w.len() == 1 && w[0].contains("kern_mem_sys.c:480-495"), "{w:?}");
        assert!(warn(on, &scrubbing).is_empty());
    }

    /// NV_PTIMER_TIME_0/1 run on the device clock, the usermode TIME_0/1's base, and a
    /// TIME_1-then-TIME_0 pair never goes back across a low-word wrap.
    #[test]
    fn ptimer_runs_on_the_device_clock() {
        let f = crate::fixture::rtx2070();
        let mut b = BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(1 << 30),
            f.identity(1 << 30),
        );
        let um = f.chip.data().registers;
        let t64 = |b: &mut BootModel| {
            let hi = b.read32(0x9410).unwrap() as u64;
            let lo = b.read32(0x9400).unwrap() as u64;
            (hi << 32) | lo
        };
        let t0 = t64(&mut b);
        std::thread::sleep(std::time::Duration::from_millis(2));
        let u0 = b.read32(um.usermode_time_0).unwrap() as u64
            | (b.read32(um.usermode_time_1).unwrap() as u64) << 32;
        let t1 = t64(&mut b);
        assert!(t1 >= t0 + 2_000_000, "{t0} {t1}");
        // The same base: the usermode time read between the two lies between them.
        assert!(t0 <= u0 && u0 <= t1, "{t0} {u0} {t1}");
        assert_eq!(t1 & 0x1F, 0);
        // Writes are dropped with a note.
        let notes = b.write32(0x9400, 5, &GuestMemory::new()).unwrap();
        assert!(notes.iter().any(|n| n.contains("PTIMER")));
        assert!(t64(&mut b) >= t1);
        // The torn-read guard, on a synthetic clock around a low-word wrap.
        let mut p = None;
        let before = (3u64 << 32) | 0xFFFF_FF00;
        let after = (4u64 << 32) | 0x40;
        assert_eq!(ptimer_word(&mut p, before, true), 3);
        assert_eq!(ptimer_word(&mut p, after, false), 0xFFFF_FFE0);
        assert_eq!(ptimer_word(&mut p, after, true), 4);
        assert_eq!(ptimer_word(&mut p, after, false), 0x40);
        // A TIME_0 read on its own and the TIME_1 field width (NSEC 28:0).
        assert_eq!(ptimer_word(&mut p, after | 0x1F, false), 0x40);
        assert_eq!(ptimer_word(&mut p, 0xE000_0001_0000_0000, true), 1);
    }

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

    /// nova-core's GSP exchange (Linux 7.2.7, firmware 570.144; docs/caraxes/nova-guest.md)
    /// against a 610 device under `CARAXES_GUEST_ABI=570`: pre-610 elements (RPC header at
    /// 0x30, checksum 0x20, seqNum 0x24, elemCount 0x28), SET_SYSTEM_INFO and SET_REGISTRY
    /// without reply, then the events GSP_RUN_CPU_SEQUENCER (empty) and GSP_INIT_DONE in
    /// that order, GET_GSP_STATIC_INFO answered with the 1656-byte 570 layout and the name
    /// at 1260, UNLOADING_GUEST_DRIVER with the processor-suspended bit, and a re-probe
    /// (new Booter Load, fresh queues) that links again. A 610 build only: the 615 build's
    /// RMARGS offsets are 615's, and `check_guest_abi` refuses 570 there.
    #[test]
    #[cfg(not(feature = "v615"))]
    fn nova_570_boot_through_the_queue() {
        use crate::msgq::tests::queue_fixture;
        crate::ver::TEST_GUEST.with(|c| c.set(Some(Some(570))));
        let f = crate::fixture::rtx2070();
        let (mem, pages, base, path) = queue_fixture("nova570");
        let mut b = BootModel::new(f.chip, crate::fb::FbLayout::plain(1 << 30), f.identity(1 << 30))
            .with_transport(GspTransport::MsgqV1);
        let (gsp, sec2, queue_head) = (0x0011_0000u64, 0x0084_0000u64, 0x0011_0C00u64);
        // Libos arguments (LOGINIT, RMARGS) and RMARGS' queue arguments behind the queue
        // buffer; 570's MESSAGE_QUEUE_INIT_ARGUMENTS keeps the four fields at 0/8/16/24.
        let (libos, rmargs) = (base + 0x90000, base + 0x91000);
        let id8 = |s: &str| s.bytes().fold(0u64, |a, c| (a << 8) | c as u64);
        assert!(mem.write(libos, &id8("LOGINIT").to_le_bytes()));
        assert!(mem.write(libos + 32, &id8("RMARGS").to_le_bytes()));
        assert!(mem.write(libos + 40, &rmargs.to_le_bytes()));
        assert!(mem.write(rmargs, &base.to_le_bytes()));
        assert!(mem.write(rmargs + 8, &129u32.to_le_bytes()));
        assert!(mem.write(rmargs + 16, &0x1000u64.to_le_bytes()));
        assert!(mem.write(rmargs + 24, &0x41000u64.to_le_bytes()));
        let mut cmd = crate::msgq::TxHeader::standard(0x40000);
        let qm = QueueMemory::new(&mem, &pages);
        assert!(qm.write(0x1000, &cmd.encode()));
        // nova's send_single_command: GSP_MSG_QUEUE_ELEMENT of r570 (48 bytes) + RPC.
        let mut seq = 0u32;
        let mut send = |cmd: &mut crate::msgq::TxHeader, function: u32, payload: &[u8]| {
            let total = 0x30 + 32 + payload.len();
            let mut raw = vec![0u8; total.div_ceil(8) * 8];
            raw[0x24..0x28].copy_from_slice(&seq.to_le_bytes());
            raw[0x28..0x2c].copy_from_slice(&(total.div_ceil(0x1000) as u32).to_le_bytes());
            let h = RpcHeader {
                header_version: 0x0300_0000,
                signature: u32::from_le_bytes(*b"VRPC"),
                length: (32 + payload.len()) as u32,
                function,
                rpc_result: 0xFFFF_FFFF,
                rpc_result_private: 0xFFFF_FFFF,
                sequence: seq,
                u: 0,
            };
            raw[0x30..0x50].copy_from_slice(&h.encode());
            raw[0x50..0x50 + payload.len()].copy_from_slice(payload);
            let sum = crate::msgq::checksum32(&raw);
            raw[0x20..0x24].copy_from_slice(&sum.to_le_bytes());
            let at = 0x1000 + cmd.entry_off as u64 + cmd.write_ptr as u64 * 0x1000;
            assert!(qm.write(at, &raw));
            cmd.write_ptr = (cmd.write_ptr + total.div_ceil(0x1000) as u32) % cmd.msg_count;
            assert!(qm.write_u32(0x1000 + 16, cmd.write_ptr));
            seq += 1;
        };
        // The status queue: element `slot`'s function, RPC length and payload.
        let status = |slot: u64| {
            let mut e = vec![0u8; 0x1000];
            assert!(qm.read(0x42000 + slot * 0x1000, &mut e));
            let h = RpcHeader::parse(&e[0x30..]);
            let len = h.length as usize;
            let padded = (0x30 + len).div_ceil(8) * 8;
            assert_eq!(crate::msgq::checksum32(&e[..padded]), 0, "slot {slot} checksum");
            (h.function, len, e[0x50..0x30 + len].to_vec())
        };
        let boot = |b: &mut BootModel, cmd: &mut crate::msgq::TxHeader, send: &mut dyn FnMut(&mut crate::msgq::TxHeader, u32, &[u8])| {
            // Bare GSP start with the libos arguments, Booter Load, then the two
            // commands without reply and the doorbell (gsp/boot.rs:146-149).
            b.write32(gsp + falcon::MAILBOX0, libos as u32, &mem);
            b.write32(gsp + falcon::MAILBOX1, (libos >> 32) as u32, &mem);
            b.write32(sec2 + falcon::MAILBOX0, 0x7654_3000, &mem);
            b.write32(sec2 + falcon::MAILBOX1, 0, &mem);
            let mut notes = b.write32(sec2 + falcon::CPUCTL, 2, &mem).unwrap();
            send(cmd, rpc::FN_GSP_SET_SYSTEM_INFO, &[0u8; 928]);
            send(cmd, rpc::FN_SET_REGISTRY, &[8, 0, 0, 0, 0, 0, 0, 0]);
            notes.extend(b.write32(queue_head, 0, &mem).unwrap());
            notes
        };
        let notes = boot(&mut b, &mut cmd, &mut send);
        assert!(notes.iter().any(|n| n.contains("queues linked")), "{notes:?}");
        assert_eq!(status(0).0, crate::ver::v570::EV_GSP_RUN_CPU_SEQUENCER);
        assert_eq!(status(0).1, 32 + 40);
        assert_eq!(status(1).0, rpc::EV_GSP_INIT_DONE);
        assert_eq!(status(1).1, 32 + 4);
        assert!(notes.iter().any(|n| n.contains("GSP_SET_SYSTEM_INFO") && n.contains("checksum ok")), "{notes:?}");
        assert!(!notes.iter().any(|n| n.contains("WARNING guest registry")), "{notes:?}");
        send(&mut cmd, rpc::FN_GET_GSP_STATIC_INFO, &[0u8; 1656]);
        b.write32(queue_head, 0, &mem).unwrap();
        let (function, len, payload) = status(2);
        assert_eq!((function, len), (rpc::FN_GET_GSP_STATIC_INFO, 32 + 1656));
        let name = &payload[1260..1260 + 64];
        let name = &name[..name.iter().position(|&c| c == 0).expect("NUL-terminated")];
        assert!(std::str::from_utf8(name).unwrap().contains("RTX 2070"), "{name:?}");
        // Unbind: UNLOADING_GUEST_DRIVER (newLevel 0), reply, MAILBOX0 bit 31.
        send(&mut cmd, crate::gsprpc::FN_UNLOADING_GUEST_DRIVER, &[0u8; 8]);
        b.write32(queue_head, 0, &mem).unwrap();
        assert_eq!(status(3).0, crate::gsprpc::FN_UNLOADING_GUEST_DRIVER);
        assert_ne!(b.read32(gsp + falcon::MAILBOX0).unwrap() & GSP_PROCESSOR_SUSPENDED, 0);
        // Booter Unload, then the re-probe: fresh queues from the CPU, a new Booter Load.
        b.write32(sec2 + falcon::MAILBOX0, 0xff, &mem);
        b.write32(sec2 + falcon::MAILBOX1, 0xff, &mem);
        b.write32(sec2 + falcon::CPUCTL, 2, &mem).unwrap();
        let zero = vec![0u8; 0x80000];
        assert!(qm.write(0x1000, &zero));
        let mut cmd = crate::msgq::TxHeader::standard(0x40000);
        assert!(qm.write(0x1000, &cmd.encode()));
        let notes = boot(&mut b, &mut cmd, &mut send);
        assert!(notes.iter().any(|n| n.contains("previous boot's link dropped")), "{notes:?}");
        assert!(notes.iter().any(|n| n.contains("queues linked")), "{notes:?}");
        assert_eq!(status(0).0, crate::ver::v570::EV_GSP_RUN_CPU_SEQUENCER);
        assert_eq!(status(1).0, rpc::EV_GSP_INIT_DONE);
        crate::ver::TEST_GUEST.with(|c| c.set(None));
        let _ = std::fs::remove_file(path);
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
        // The captured guest carries the three keys the device needs (gspboot.rs).
        assert!(!notes.iter().any(|n| n.contains("WARNING guest registry")), "{notes:?}");
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

    /// The command-queue head writes through the ioeventfd (`CARAXES_RPC_IOEVENTFD=1`):
    /// the count without the values. msgqTxCreate's write before the boot is taken at the
    /// link; each later write is served with the head reconstructed from the ring, as the
    /// trapped write with its value would be; a write that published nothing while linked
    /// is the reload's 0 and drops the link. A 610 transport gets its plain doorbell.
    #[test]
    fn head_writes_without_their_value_are_served_from_the_ring() {
        use std::io::Write;
        let (mem, path) = guest_615("rpcfd");
        let f = crate::fixture::rtx2070();
        // SAFETY: a plain syscall; checked below.
        let raw = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC) };
        assert!(raw >= 0);
        // SAFETY: a new descriptor nothing else owns.
        let efd = unsafe { <std::fs::File as std::os::fd::FromRawFd>::from_raw_fd(raw) };
        let guest_write = |n: u64| (&efd).write_all(&n.to_ne_bytes()).unwrap();
        let mut b = BootModel::new(f.chip, crate::fb::FbLayout::plain(4 << 30), f.identity(4 << 30))
            .with_transport(GspTransport::MsgqV2)
            .with_rpc_eventfd(efd.try_clone().unwrap())
            .unwrap();
        assert_eq!(b.cmd_head_register(), Some(CMD_HEAD));
        assert_eq!(b.take_rpc_writes(), 0, "nothing counted, and the read does not block");
        guest_write(1); // msgqTxCreate (msgq.c:176)
        let notes = booter_load(&mut b, &mem);
        assert!(notes.iter().any(|n| n.contains("1 head write(s) before the link")), "{notes:?}");
        assert!(notes.iter().any(|n| n.starts_with("queues linked (v2)")), "{notes:?}");
        // GSP_INIT published at slot 0: head 1, served as the trapped write.
        let slot0 = std::fs::read(
            "../../caraxes/runs/guest-driver-2026-09-24/615.71.09/linked/gmcapi-gsp-init-slot0.bin",
        )
        .unwrap();
        assert!(mem.write(0x88a0_0000 + 0x2000, &slot0));
        b.write32(MSGQ_TAIL, 0, &mem);
        let (v, note) = b.cmd_head_from_ring(&mem);
        assert_eq!(v, Some(1), "{note}");
        let notes = b.write32(CMD_HEAD, 1, &mem).unwrap();
        assert!(notes.iter().any(|n| n.starts_with("tx seq 0 reply GMCAPI_GSP_INIT")), "{notes:?}");
        // A second count before the element is written would be a create write; after
        // the RPC is written it is the RPC.
        let pages = QueueMemory::read_page_table(&mem, 0x88a0_0000, 129).unwrap();
        let mut cpu = crate::msgq::v2::tests::Cpu {
            hdr: crate::msgq::v2::TxHeaderV2::standard(0x40000),
            write_ptr: 1,
            read_ptr: 0,
        };
        let h = RpcHeader {
            header_version: v::RPC_HEADER_VERSION,
            signature: v::RPC_SIGNATURE,
            length: v::RPC_HEADER_SIZE as u32,
            function: rpc::FN_SET_GUEST_SYSTEM_INFO,
            rpc_result: 0xFFFF_FFFF,
            rpc_result_private: 0xFFFF_FFFF,
            sequence: 1,
            u: 0,
        };
        {
            let qm = QueueMemory::new(&mem, &pages);
            let mut body = h.encode().to_vec();
            body.extend_from_slice(&[0u8; 792]);
            let mut hh = h;
            hh.length = body.len() as u32;
            body[..32].copy_from_slice(&hh.encode());
            cpu.send(&qm, crate::msgq::v2::NVDM_TYPE_RM_RPC, &body);
        }
        let (v, note) = b.cmd_head_from_ring(&mem);
        assert_eq!(v, Some(2), "{note}");
        let notes = b.write32(CMD_HEAD, 2, &mem).unwrap();
        assert!(notes.iter().any(|n| n.contains("SET_GUEST_SYSTEM_INFO")), "{notes:?}");
        // A driver reload: msgqTxCreate's 0 publishes nothing; the slot at our read
        // pointer was never written (0): the link drops as for the trapped 0.
        let (v, note) = b.cmd_head_from_ring(&mem);
        assert_eq!(v, Some(0), "{note}");
        assert!(note.contains("created anew"), "{note}");
        let notes = b.write32(CMD_HEAD, 0, &mem).unwrap();
        assert!(notes.iter().any(|n| n.contains("link dropped")), "{notes:?}");
        // Unlinked now: a count is noted only.
        assert_eq!(b.cmd_head_from_ring(&mem).0, None);
        let st = b.rpc_eventfd_stats;
        assert_eq!((st.writes, st.elements, st.before_link, st.recreated), (5, 2, 2, 1));
        // The reset keeps the eventfd and the counters.
        b.reset();
        guest_write(3);
        assert_eq!(b.take_rpc_writes(), 3);
        assert_eq!(b.rpc_eventfd_stats.writes, 5);
        // The 610 transport: the register is a doorbell of value 0.
        let mut b610 = BootModel::new(f.chip, crate::fb::FbLayout::plain(4 << 30), f.identity(4 << 30))
            .with_transport(GspTransport::MsgqV1);
        assert_eq!(b610.cmd_head_from_ring(&mem).0, Some(0));
        let _ = std::fs::remove_file(&path);
    }

    /// The captured 615 guest past `GMCAPI_CMD_GSP_INIT`, with the CPU's side of both
    /// queues (msgq/v2.rs `Cpu`): what the tests of the RPCs that follow the init send
    /// their requests through.
    pub(crate) struct Guest615 {
        pub b: BootModel,
        pub mem: GuestMemory,
        pub path: std::path::PathBuf,
        pages: Vec<u64>,
        cpu: crate::msgq::v2::tests::Cpu,
        sequence: u32,
    }

    const CMD_HEAD: u64 = 0x0011_0C00;
    const MSGQ_HEAD: u64 = 0x0011_0C80;
    const MSGQ_TAIL: u64 = 0x0011_0C84;

    impl Guest615 {
        pub fn boot(tag: &str) -> Self {
            let (mem, path) = guest_615(tag);
            let f = crate::fixture::rtx2070();
            let mut b = BootModel::new(f.chip, crate::fb::FbLayout::plain(4 << 30), f.identity(4 << 30))
                .with_transport(GspTransport::MsgqV2);
            b.write32(CMD_HEAD, 0, &mem);
            booter_load(&mut b, &mem);
            let slot0 = std::fs::read(
                "../../caraxes/runs/guest-driver-2026-09-24/615.71.09/linked/gmcapi-gsp-init-slot0.bin",
            )
            .unwrap();
            assert!(mem.write(0x88a0_0000 + 0x2000, &slot0));
            b.write32(MSGQ_TAIL, 0, &mem);
            b.write32(CMD_HEAD, 1, &mem);
            let pages = QueueMemory::read_page_table(&mem, 0x88a0_0000, 129).unwrap();
            let cpu = crate::msgq::v2::tests::Cpu {
                hdr: crate::msgq::v2::TxHeaderV2::standard(0x40000),
                write_ptr: 1,
                read_ptr: 0,
            };
            let mut g = Guest615 { b, mem, path, pages, cpu, sequence: 1 };
            let init = g.drain();
            assert_eq!(init.len(), 1, "the GSP_INIT response");
            g
        }

        /// Every element the device published since the last call, as (NVDM type, body).
        pub fn drain(&mut self) -> Vec<(u32, Vec<u8>)> {
            let head = self.b.read32(MSGQ_HEAD).unwrap();
            let mut out = Vec::new();
            {
                let qm = QueueMemory::new(&self.mem, &self.pages);
                while let Some(e) = self.cpu.receive(&qm, head) {
                    out.push(e);
                }
            }
            self.b.write32(MSGQ_TAIL, self.cpu.read_ptr, &self.mem);
            out
        }

        /// Send one element of type `nvdm` and return the device's notes and every
        /// element it published.
        pub fn send(&mut self, nvdm: u32, body: &[u8]) -> (Vec<String>, Vec<(u32, Vec<u8>)>) {
            {
                let qm = QueueMemory::new(&self.mem, &self.pages);
                self.cpu.send(&qm, nvdm, body);
            }
            let notes = self.b.write32(CMD_HEAD, self.cpu.write_ptr, &self.mem).unwrap();
            (notes, self.drain())
        }

        /// Send one RPC and return the device's notes and every element it published.
        pub fn rpc(&mut self, function: u32, payload: &[u8]) -> (Vec<String>, Vec<(u32, Vec<u8>)>) {
            let h = RpcHeader {
                header_version: v::RPC_HEADER_VERSION,
                signature: v::RPC_SIGNATURE,
                length: (v::RPC_HEADER_SIZE + payload.len()) as u32,
                function,
                rpc_result: 0xFFFF_FFFF,
                rpc_result_private: 0xFFFF_FFFF,
                sequence: self.sequence,
                u: 0,
            };
            self.sequence += 1;
            let mut body = h.encode().to_vec();
            body.extend_from_slice(payload);
            self.send(crate::msgq::v2::NVDM_TYPE_RM_RPC, &body)
        }

        /// The RPC element for `function` among `elements`: (rpc_result, payload).
        pub fn reply(elements: &[(u32, Vec<u8>)], function: u32) -> Option<(u32, Vec<u8>)> {
            elements.iter().find_map(|(t, body)| {
                let h = RpcHeader::parse(body);
                (*t == crate::msgq::v2::NVDM_TYPE_RM_RPC && h.function == function)
                    .then(|| (h.rpc_result, body[v::RPC_HEADER_SIZE..h.length as usize].to_vec()))
            })
        }
    }

    impl Drop for Guest615 {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// BootModel must route both an ordinary GMCAPI reply and an answerable framing
    /// rejection through the typed response path. In both cases the kernel waiter's
    /// 64-bit sequence survives instead of being narrowed through `RpcHeader`.
    #[test]
    fn boot_615_gmcapi_replies_preserve_u64_sequence() {
        let mut guest = Guest615::boot("gmc-sequence");
        let request = crate::gmcapi::Header {
            command: crate::gmcapi::CMD_GSP_INIT,
            size: 0,
            sequence: 0x1_0000_0001,
            request_or_status: crate::gmcapi::GSP_INIT_MAX_RESPONSE as u32,
        };
        let (notes, elements) = guest.send(
            crate::msgq::v2::NVDM_TYPE_GMCAPI,
            &request.encode(),
        );
        assert!(notes.iter().any(|n| n.contains("sequence 4294967297")), "{notes:?}");
        assert_eq!(elements.len(), 1);
        let response = crate::gmcapi::Header::parse(&elements[0].1).unwrap();
        assert_eq!(
            (elements[0].0, response.id(), response.sequence, response.request_or_status),
            (
                crate::msgq::v2::NVDM_TYPE_GMCAPI,
                crate::gmcapi::CMD_GSP_INIT,
                0x1_0000_0001,
                rpc::NV_OK
            )
        );
        assert!(response.is_response());

        let malformed = crate::gmcapi::Header {
            sequence: 0x2_0000_0001,
            ..request
        };
        let body = malformed.encode().to_vec();
        let mut raw = {
            let qm = QueueMemory::new(&guest.mem, &guest.pages);
            guest.cpu.send(&qm, crate::msgq::v2::NVDM_TYPE_GMCAPI, &body)
        };
        // Rewind and overwrite the just-built slot with a bad MCTP magic. Its intact
        // GMCAPI header still makes the rejection answerable.
        guest.cpu.write_ptr -= raw.len().div_ceil(0x1000) as u32;
        raw[..4].copy_from_slice(&0xffff_ffffu32.to_le_bytes());
        {
            let qm = QueueMemory::new(&guest.mem, &guest.pages);
            guest.cpu.raw(&qm, &raw);
        }
        let notes = guest.b.write32(CMD_HEAD, guest.cpu.write_ptr, &guest.mem).unwrap();
        let elements = guest.drain();
        assert!(notes.iter().any(|n| n.contains("sequence 8589934593")), "{notes:?}");
        assert_eq!(elements.len(), 1);
        let response = crate::gmcapi::Header::parse(&elements[0].1).unwrap();
        assert_eq!(
            (response.id(), response.sequence, response.request_or_status),
            (
                crate::gmcapi::CMD_GSP_INIT,
                0x2_0000_0001,
                crate::msgq::NV_ERR_INVALID_DATA
            )
        );
        assert!(response.is_response());
    }

    /// A `GSP_RM_ALLOC` request of client `c` under parent `c`.
    pub(crate) fn alloc_request(c: u32, h: u32, class: u32, params: &[u8]) -> Vec<u8> {
        let mut p = vec![0u8; v::RM_ALLOC_PARAMS];
        for (o, x) in [
            (v::RM_ALLOC_H_CLIENT, c),
            (v::RM_ALLOC_H_PARENT, c),
            (v::RM_ALLOC_H_OBJECT, h),
            (v::RM_ALLOC_H_CLASS, class),
            (v::RM_ALLOC_PARAMS_SIZE, params.len() as u32),
        ] {
            p[o..o + 4].copy_from_slice(&x.to_le_bytes());
        }
        p.extend_from_slice(params);
        p
    }

    /// A `GSP_RM_CONTROL` request.
    pub(crate) fn control_request(c: u32, h: u32, cmd: u32, params: &[u8]) -> Vec<u8> {
        let mut p = vec![0u8; v::RM_CONTROL_PARAMS];
        for (o, x) in [
            (v::RM_CONTROL_H_CLIENT, c),
            (v::RM_CONTROL_H_OBJECT, h),
            (v::RM_CONTROL_CMD, cmd),
            (v::RM_CONTROL_PARAMS_SIZE, params.len() as u32),
        ] {
            p[o..o + 4].copy_from_slice(&x.to_le_bytes());
        }
        p.extend_from_slice(params);
        p
    }

    /// B1: the operational-event import of 615 (gpu_op_event_import.c:593-685) through
    /// the queue: the header buffer's `ALLOC_MEMORY` of the first 615 boot
    /// (transport-615-2026-09-24/boot/device-events.txt:517) is answered NV_OK, and the
    /// event buffer, its enable and its bind follow on the registered memory.
    #[test]
    fn alloc_memory_and_the_event_import_through_the_queue() {
        let mut g = Guest615::boot("allocmem");
        let c = goe_import(&mut g);
        // Teardown frees the client: every registration goes.
        let mut free = vec![0u8; v::FREE_SIZE];
        free[v::FREE_H_ROOT..][..4].copy_from_slice(&c.to_le_bytes());
        free[v::FREE_H_OBJECT..][..4].copy_from_slice(&c.to_le_bytes());
        let (notes, _) = g.rpc(rpc::FN_FREE, &free);
        assert!(notes.iter().any(|n| n.contains("GSP memory object(s) of the client")), "{notes:?}");
        assert!(g.b.gspmem.memories.is_empty() && g.b.gspmem.buffers.is_empty());
        assert!(g.b.unanswered.is_empty(), "{:?}", g.b.unanswered);
    }

    /// The RC self-test (`CARAXES_RC_SELFTEST`): nothing before the 615 import is bound,
    /// then one SIMULATED record per period, in the journal and as one GOE record.
    #[test]
    fn the_rc_selftest_journals_and_writes_one_goe_record() {
        let mut g = Guest615::boot("rcselftest");
        g.b.ctrl.dstate.rc.selftest = Some(crate::rc::SelfTest {
            period: std::time::Duration::from_secs(3600),
            last: None,
            sent: 0,
        });
        g.b.scan_channel_errors(&g.mem, false);
        assert!(g.b.ctrl.dstate.rc.journal.is_empty(), "not before the import is bound");
        goe_import(&mut g);
        let notes = g.b.scan_channel_errors(&g.mem, false);
        g.b.scan_channel_errors(&g.mem, false);
        let j = &g.b.ctrl.dstate.rc.journal;
        assert_eq!(j.len(), 1, "{notes:?}");
        assert!(j[0].chid.is_none() && j[0].text.contains("SIMULATED"), "{:?}", j[0]);
        assert_eq!(g.mem.read_u32(GOE_HEADER + 4), Some(1), "{notes:?}");
    }

    /// The guest physical address of the GOE header [`goe_import`] registers.
    const GOE_HEADER: u64 = 0x88a7_0000;

    /// The operational-event import as the first 615 boot sent it, through the queue
    /// (header 32 bytes at [`GOE_HEADER`], 256 records of 64 bytes, vardata 0x1000); returns
    /// the client.
    fn goe_import(g: &mut Guest615) -> u32 {
        let (c, d, s) = (0xc1e0_0007u32, 0xcaf0_0000u32, 0xcaf0_0001u32);
        let am = |h: u32, len: u64, pfn: u64| {
            let mut p = vec![0u8; crate::gspmem::AM_PTES + 8];
            for (o, v) in [(0, c), (4, d), (8, h), (12, 0x81), (16, 0x4c00_0000), (24, 6)] {
                p[o..o + 4].copy_from_slice(&v.to_le_bytes());
            }
            p[32..40].copy_from_slice(&len.to_le_bytes());
            p[40..44].copy_from_slice(&1u32.to_le_bytes());
            p[48..52].copy_from_slice(&(1u32 << 16).to_le_bytes());
            p[56..64].copy_from_slice(&pfn.to_le_bytes());
            p
        };
        // Header, records and vardata in the captured guest's mapped RAM.
        for (h, len, pfn) in [(0xcaf0_0002u32, 32u64, 0x88a70u64), (0xcaf0_0003, 0x4000, 0x88a71), (0xcaf0_0004, 0x1000, 0x88a75)] {
            let (notes, el) = g.rpc(crate::gspmem::FN_ALLOC_MEMORY, &am(h, len, pfn));
            let (st, reply) = Guest615::reply(&el, crate::gspmem::FN_ALLOC_MEMORY).unwrap();
            assert_eq!(st, 0, "{notes:?}");
            assert_eq!(reply.len(), crate::gspmem::AM_SIZE);
            assert!(notes.iter().any(|n| n.contains("ALLOC_MEMORY (4)")), "{notes:?}");
        }
        // A page the guest does not have: refused, nothing mapped.
        let (_, el) = g.rpc(crate::gspmem::FN_ALLOC_MEMORY, &am(0xcaf0_0009, 0x1000, 0x7_0000_0000));
        assert_eq!(
            Guest615::reply(&el, crate::gspmem::FN_ALLOC_MEMORY).unwrap().0,
            crate::gspmem::NV_ERR_INVALID_ADDRESS
        );
        // Step 5b: NV_EVENT_BUFFER on the three, allocated by GSP_RM_ALLOC.
        let mut eb = vec![0u8; crate::gspmem::EB_PARAMS_SIZE];
        for (o, x) in [(16, 64u32), (20, 256), (32, 0x1000), (52, s), (60, 0xcaf0_0002), (64, 0xcaf0_0003), (68, 0xcaf0_0004)] {
            eb[o..o + 4].copy_from_slice(&x.to_le_bytes());
        }
        let (notes, el) = g.rpc(rpc::FN_GSP_RM_ALLOC, &alloc_request(c, 0xcaf0_0006, 0x90cd, &eb));
        assert_eq!(Guest615::reply(&el, rpc::FN_GSP_RM_ALLOC).unwrap().0, 0, "{notes:?}");
        // Step 7: ENABLE_EVENTS, a GSP_RM_CONTROL on the buffer.
        let ctl = control_request(c, 0xcaf0_0006, 0x90cd_0101, &[1, 0, 0, 0, 1, 0, 0, 0]);
        let (notes, el) = g.rpc(rpc::FN_GSP_RM_CONTROL, &ctl);
        let (_, reply) = Guest615::reply(&el, rpc::FN_GSP_RM_CONTROL).unwrap();
        let st = u32::from_le_bytes(reply[v::RM_CONTROL_STATUS..][..4].try_into().unwrap());
        assert_eq!(st, 0, "{notes:?}");
        // Step 8: the bind names the buffer; bindId comes back.
        let mut bind = vec![0u8; crate::gspmem::BIND_PARAMS_SIZE];
        bind[..4].copy_from_slice(&0xcaf0_0006u32.to_le_bytes());
        bind[4] = 1;
        let (notes, el) = g.rpc(rpc::FN_GSP_RM_ALLOC, &alloc_request(c, 0xcaf0_0005, 0x90d0, &bind));
        let (st, reply) = Guest615::reply(&el, rpc::FN_GSP_RM_ALLOC).unwrap();
        assert_eq!(st, 0, "{notes:?}");
        assert_ne!(reply[v::RM_ALLOC_PARAMS + crate::gspmem::BIND_BIND_ID], 0);
        assert!(g.b.gspmem.has_goe_bind());
        c
    }

    /// The conditional RPCs through the queue (gsprpc.rs): each gets the answer of
    /// rpc-functions.md §4.2, none lands in `unanswered`, the asynchronous ECC ack gets
    /// no reply.
    #[test]
    fn conditional_rpcs_through_the_queue() {
        use crate::gsprpc as g;
        let mut q = Guest615::boot("condrpc");
        // DUMP_PROTOBUF_COMPONENT: an empty blob.
        let mut dump = vec![0u8; g::DUMP_FIXED];
        dump[g::DUMP_BUFFER_SIZE..][..4].copy_from_slice(&0xFFD0u32.to_le_bytes());
        let (_, el) = q.rpc(g::FN_DUMP_PROTOBUF_COMPONENT, &dump);
        let (st, reply) = Guest615::reply(&el, g::FN_DUMP_PROTOBUF_COMPONENT).unwrap();
        assert_eq!((st, &reply[g::DUMP_BUFFER_SIZE..g::DUMP_BUFFER_SIZE + 4]), (0, &[0u8; 4][..]));
        // ECC_NOTIFIER_WRITE_ACK: consumed, nothing sent back.
        let (notes, el) = q.rpc(g::FN_ECC_NOTIFIER_WRITE_ACK, &[]);
        assert!(el.is_empty() && notes.iter().any(|n| n.contains("no reply")), "{notes:?}");
        // GPU_EXEC_REG_OPS: BOOT_0 from the chip's identity, the rest NOACCESS.
        let mut ops = vec![0u8; g::REGOPS_OPERATIONS + 2 * g::REGOP_SIZE];
        ops[g::REGOPS_COUNT..][..4].copy_from_slice(&2u32.to_le_bytes());
        ops[g::REGOPS_OPERATIONS + g::REGOP_SIZE + g::REGOP_OFFSET..][..4]
            .copy_from_slice(&0x0010_0000u32.to_le_bytes());
        let (_, el) = q.rpc(g::FN_GPU_EXEC_REG_OPS, &ops);
        let (st, reply) = Guest615::reply(&el, g::FN_GPU_EXEC_REG_OPS).unwrap();
        assert_eq!(st, 0);
        let lo = u32::from_le_bytes(reply[g::REGOPS_OPERATIONS + g::REGOP_VALUE_LO..][..4].try_into().unwrap());
        assert_eq!(lo, q.b.chip.data().identity.boot_0);
        assert_eq!(reply[g::REGOPS_OPERATIONS + g::REGOP_SIZE + g::REGOP_STATUS], g::REG_OP_STATUS_NOACCESS);
        // IDLE_CHANNELS of a channel the device does not know.
        let mut idle = vec![0u8; g::IDLE_LIST + g::IDLE_ENTRY];
        idle[g::IDLE_NCHANNELS..][..4].copy_from_slice(&1u32.to_le_bytes());
        let (_, el) = q.rpc(g::FN_IDLE_CHANNELS, &idle);
        assert_eq!(Guest615::reply(&el, g::FN_IDLE_CHANNELS).unwrap().0, crate::gspmem::NV_ERR_INVALID_OBJECT_HANDLE);
        // MAP_MEMORY_DMA: refused, named.
        let (notes, el) = q.rpc(g::FN_MAP_MEMORY_DMA, &[0u8; 56]);
        assert_eq!(Guest615::reply(&el, g::FN_MAP_MEMORY_DMA).unwrap().0, crate::gspmem::NV_ERR_NOT_SUPPORTED);
        assert!(notes.iter().any(|n| n.contains("RMSplitVasMgmtServerClientRm")), "{notes:?}");
        // UNLOADING_GUEST_DRIVER: NV_OK and the processor-suspended bit.
        let (_, el) = q.rpc(g::FN_UNLOADING_GUEST_DRIVER, &[0u8; 8]);
        assert_eq!(Guest615::reply(&el, g::FN_UNLOADING_GUEST_DRIVER).unwrap().0, 0);
        assert_eq!(q.b.fw.read32(GSP_MAILBOX0).unwrap() & GSP_PROCESSOR_SUSPENDED, GSP_PROCESSOR_SUSPENDED);
        assert!(q.b.unanswered.is_empty(), "{:?}", q.b.unanswered);
    }

    /// Events from a host-mirrored channel's error notifier (events.rs): the host RM's
    /// write into the guest's own notifier page becomes OS_ERROR_LOG then the 64-byte
    /// 615 RC_TRIGGERED for that guest chid, once, with SWGEN0 up until the kernel
    /// clears it.
    #[test]
    fn a_mirrored_channel_error_reaches_the_guest_once() {
        use nvrm_sys::RmAbi;
        type Abi = v::RmAbi;
        let mut g = Guest615::boot("rcevent");
        let (c, notifier) = (0xc1d0_0001u32, 0x88a7_8000u64);
        let mut p = vec![0u8; <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_SIZE as usize];
        let put = |b: &mut [u8], o: usize, x: &[u8]| b[o..o + x.len()].copy_from_slice(x);
        let flags = (1u32 << 21) | (5 << 8);
        put(&mut p, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_flags, &flags.to_le_bytes());
        put(&mut p, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_gpFifoEntries, &32u32.to_le_bytes());
        put(&mut p, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_engineType, &1u32.to_le_bytes());
        let userd = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_userdMem;
        put(&mut p, userd + v::MEM_DESC_BASE, &0x20_0000u64.to_le_bytes());
        put(&mut p, userd + v::MEM_DESC_ADDRESS_SPACE, &v::ADDR_FBMEM.to_le_bytes());
        let err = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_errorNotifierMem;
        put(&mut p, err + v::MEM_DESC_BASE, &notifier.to_le_bytes());
        put(&mut p, err + v::MEM_DESC_ADDRESS_SPACE, &v::ADDR_SYSMEM.to_le_bytes());
        let (notes, el) = g.rpc(rpc::FN_GSP_RM_ALLOC, &alloc_request(c, 0xcaf1_0000, 0xc46f, &p));
        assert_eq!(Guest615::reply(&el, rpc::FN_GSP_RM_ALLOC).unwrap().0, 0, "{notes:?}");
        g.b.ctrl.channels.channel_mut(5).unwrap().mirrored = true;
        // First look: the notifier is the baseline; nothing to report.
        assert!(g.b.scan_channel_errors(&g.mem, true).is_empty());
        g.b.rc_scan.1 = None;
        // The host RM's RC write: info32 31 (MMU fault), status 0xFFFF.
        let mut n = [0u8; 16];
        n[8] = 31;
        n[14] = 0xFF;
        n[15] = 0xFF;
        assert!(g.mem.write(notifier, &n));
        let notes = g.b.scan_channel_errors(&g.mem, true);
        assert!(notes.iter().any(|s| s.contains("chid 5: error notifier written (exceptType 31")), "{notes:?}");
        assert_eq!(g.b.read32(GSP_IRQSTAT).unwrap() & GSP_SWGEN0, GSP_SWGEN0);
        let el = g.drain();
        let oel = Guest615::reply(&el, crate::events::EV_OS_ERROR_LOG).unwrap().1;
        assert_eq!(u32::from_le_bytes(oel[..4].try_into().unwrap()), 31);
        let rc = Guest615::reply(&el, crate::events::EV_RC_TRIGGERED).unwrap().1;
        assert_eq!(rc.len(), 64);
        assert_eq!(
            (u32::from_le_bytes(rc[0..4].try_into().unwrap()), u32::from_le_bytes(rc[4..8].try_into().unwrap())),
            (1, 5)
        );
        // The kernel clears SWGEN0; the guest kernel's own notifier write after the RC
        // raises nothing more.
        g.b.write32(GSP_IRQSCLR, GSP_SWGEN0, &g.mem);
        assert_eq!(g.b.read32(GSP_IRQSTAT).unwrap() & GSP_SWGEN0, 0);
        n[0] = 1;
        assert!(g.mem.write(notifier, &n));
        g.b.rc_scan.1 = None;
        assert!(g.b.scan_channel_errors(&g.mem, true).is_empty());
    }

    /// The guest's STOP_CHANNEL, then the kernel's own PREEMPTIVE_REMOVAL in the channel's
    /// error notifier (kchannelNotifyRc): no OS_ERROR_LOG, no RC_TRIGGERED (the guest Xid 45
    /// at every CUDA process exit of 2026-09-27/28). A later error of another kind still
    /// reaches the guest.
    #[test]
    fn the_notice_after_the_guests_stop_channel_is_no_rc() {
        use nvrm_sys::RmAbi;
        type Abi = v::RmAbi;
        let mut g = Guest615::boot("stopnotice");
        let (c, notifier) = (0xc1d0_0001u32, 0x88a7_8000u64);
        let mut p = vec![0u8; <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_SIZE as usize];
        let put = |b: &mut [u8], o: usize, x: &[u8]| b[o..o + x.len()].copy_from_slice(x);
        let flags = (1u32 << 21) | (5 << 8);
        put(&mut p, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_flags, &flags.to_le_bytes());
        put(&mut p, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_gpFifoEntries, &32u32.to_le_bytes());
        put(&mut p, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_engineType, &1u32.to_le_bytes());
        let userd = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_userdMem;
        put(&mut p, userd + v::MEM_DESC_BASE, &0x20_0000u64.to_le_bytes());
        put(&mut p, userd + v::MEM_DESC_ADDRESS_SPACE, &v::ADDR_FBMEM.to_le_bytes());
        let err = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_errorNotifierMem;
        put(&mut p, err + v::MEM_DESC_BASE, &notifier.to_le_bytes());
        put(&mut p, err + v::MEM_DESC_ADDRESS_SPACE, &v::ADDR_SYSMEM.to_le_bytes());
        let (notes, el) = g.rpc(rpc::FN_GSP_RM_ALLOC, &alloc_request(c, 0xcaf1_0000, 0xc46f, &p));
        assert_eq!(Guest615::reply(&el, rpc::FN_GSP_RM_ALLOC).unwrap().0, 0, "{notes:?}");
        g.b.ctrl.channels.channel_mut(5).unwrap().mirrored = true;
        assert!(g.b.scan_channel_errors(&g.mem, true).is_empty());
        g.rpc(rpc::FN_GSP_RM_CONTROL, &control_request(c, 0xcaf1_0000, v::CMD_CHANNEL_STOP, &[0u8; 1]));
        assert!(g.b.ctrl.channels.channel_mut(5).unwrap().stopped);
        g.drain();
        // The kernel's stop notice: info32 45, status 0xFFFF.
        let mut n = [0u8; 16];
        n[8] = 45;
        n[14] = 0xFF;
        n[15] = 0xFF;
        assert!(g.mem.write(notifier, &n));
        g.b.rc_scan.1 = None;
        let notes = g.b.scan_channel_errors(&g.mem, true);
        assert!(notes.iter().any(|s| s.contains("chid 5: PREEMPTIVE_REMOVAL after the guest's STOP_CHANNEL")), "{notes:?}");
        assert!(!notes.iter().any(|s| s.contains("error notifier written")), "{notes:?}");
        let el = g.drain();
        assert!(Guest615::reply(&el, crate::events::EV_OS_ERROR_LOG).is_none());
        assert!(Guest615::reply(&el, crate::events::EV_RC_TRIGGERED).is_none());
        // A later MMU fault on the same channel is an RC as before.
        n[8] = 31;
        assert!(g.mem.write(notifier, &n));
        g.b.rc_scan.1 = None;
        let notes = g.b.scan_channel_errors(&g.mem, true);
        assert!(notes.iter().any(|s| s.contains("chid 5: error notifier written (exceptType 31")), "{notes:?}");
    }

    /// The RC reflection of the D models (rc.rs): a non-replayable fault UVM reports on a
    /// guest channel, then an RC watchdog timeout with that channel's USERD behind, each
    /// reach the guest as OS_ERROR_LOG and RC_TRIGGERED for that chid on the next scan;
    /// a fault on an instance block no channel has is refused and raises nothing.
    #[test]
    fn the_d_models_rc_reaches_the_guest() {
        use nvrm_sys::RmAbi;
        type Abi = v::RmAbi;
        let mut g = Guest615::boot("dmodelrc");
        goe_import(&mut g);
        // recordPut of the GOE header: one record per RC the device raised.
        let goe_records = |g: &Guest615| g.mem.read_u32(GOE_HEADER + 4).unwrap();
        let (c, inst) = (0xc1d0_0002u32, 0x0123_4000u64);
        let mut p = vec![0u8; <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_SIZE as usize];
        let put = |b: &mut [u8], o: usize, x: &[u8]| b[o..o + x.len()].copy_from_slice(x);
        put(&mut p, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_flags, &((1u32 << 21) | (6 << 8)).to_le_bytes());
        put(&mut p, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_engineType, &1u32.to_le_bytes());
        let userd = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_userdMem;
        put(&mut p, userd + v::MEM_DESC_BASE, &0x30_0000u64.to_le_bytes());
        put(&mut p, userd + v::MEM_DESC_ADDRESS_SPACE, &v::ADDR_FBMEM.to_le_bytes());
        let im = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_instanceMem;
        put(&mut p, im + v::MEM_DESC_BASE, &inst.to_le_bytes());
        put(&mut p, im + v::MEM_DESC_ADDRESS_SPACE, &v::ADDR_FBMEM.to_le_bytes());
        let (notes, el) = g.rpc(rpc::FN_GSP_RM_ALLOC, &alloc_request(c, 0xcaf2_0000, 0xc46f, &p));
        assert_eq!(Guest615::reply(&el, rpc::FN_GSP_RM_ALLOC).unwrap().0, 0, "{notes:?}");
        let nr = crate::ver::Abi::active()
            .d_rules
            .iter()
            .find(|r| r.name == "NV2080_CTRL_CMD_GPU_REPORT_NON_REPLAYABLE_FAULT")
            .unwrap()
            .id;
        // The control's own status (rpc_gsp_rm_control `status`).
        let st = |el: &[_]| {
            let r = Guest615::reply(el, rpc::FN_GSP_RM_CONTROL).unwrap().1;
            u32::from_le_bytes(r[v::RM_CONTROL_STATUS..v::RM_CONTROL_STATUS + 4].try_into().unwrap())
        };
        let packet = |inst: u64| {
            let mut b = vec![0u8; 32];
            b[0..4].copy_from_slice(&(inst as u32 & 0xFFFF_F000).to_le_bytes());
            b[4..8].copy_from_slice(&((inst >> 32) as u32).to_le_bytes());
            b
        };
        // An instance block of no channel: refused, nothing raised.
        let (_, el) = g.rpc(rpc::FN_GSP_RM_CONTROL, &control_request(c, 0xcaf2_0001, nr, &packet(0x0999_0000)));
        assert_ne!(st(&el), 0);
        assert!(g.b.scan_channel_errors(&g.mem, false).is_empty());
        // The channel's own instance block: NV_OK, then the RC on the next scan.
        // (The RPC's own BAR0 writes run the scan: the RC may already be out.)
        let (mut notes, el) = g.rpc(rpc::FN_GSP_RM_CONTROL, &control_request(c, 0xcaf2_0001, nr, &packet(inst)));
        assert_eq!(st(&el), 0);
        notes.extend(g.b.scan_channel_errors(&g.mem, false));
        assert!(g.b.ctrl.dstate.rc.pending.is_empty(), "{notes:?}");
        assert_eq!(g.b.ctrl.dstate.rc.journal.last().map(|r| (r.chid, r.xid)), Some((Some(6), 31)));
        assert_eq!((g.b.ctrl.dstate.rc.journal.len(), goe_records(&g)), (1, 1), "{notes:?}");
        let el: Vec<_> = el.into_iter().chain(g.drain()).collect();
        let rc = Guest615::reply(&el, crate::events::EV_RC_TRIGGERED).unwrap().1;
        assert_eq!(u32::from_le_bytes(rc[4..8].try_into().unwrap()), 6);
        assert_eq!(u32::from_le_bytes(rc[16..20].try_into().unwrap()), 31);
        // The watchdog: the channel's USERD stands at GPGet 1 of GPPut 3.
        g.b.write32(GSP_IRQSCLR, GSP_SWGEN0, &g.mem);
        {
            let mut gm = crate::channel::GpuMem { fb: &mut g.b.fb, sys: &g.mem };
            assert!(gm.write(crate::fb::Target::Fb(0x30_0088), &1u32.to_le_bytes()));
            assert!(gm.write(crate::fb::Target::Fb(0x30_008C), &3u32.to_le_bytes()));
        }
        let wd = crate::ver::Abi::active()
            .d_rules
            .iter()
            .find(|r| r.name == "NV2080_CTRL_CMD_INTERNAL_RC_WATCHDOG_TIMEOUT")
            .unwrap()
            .id;
        let (mut notes, el) = g.rpc(rpc::FN_GSP_RM_CONTROL, &control_request(c, 0xcaf2_0001, wd, &[]));
        assert_eq!(st(&el), 0);
        notes.extend(g.b.scan_channel_errors(&g.mem, false));
        assert!(g.b.ctrl.dstate.rc.pending.is_empty(), "{notes:?}");
        let last = g.b.ctrl.dstate.rc.journal.last().unwrap();
        assert_eq!((last.chid, last.xid), (Some(6), 45));
        assert!(last.text.contains("GPGet 1 of GPPut 3"), "{}", last.text);
        let el: Vec<_> = el.into_iter().chain(g.drain()).collect();
        let rc = Guest615::reply(&el, crate::events::EV_RC_TRIGGERED).unwrap().1;
        assert_eq!(u32::from_le_bytes(rc[16..20].try_into().unwrap()), 45);
        assert_eq!((g.b.ctrl.dstate.rc.journal.len(), goe_records(&g)), (2, 2), "{notes:?}");
        // The guest's own Xid (LOG_OOB_XID) lands in the same journal, as no RC of the
        // device (no GOE record); RC_GET_ERROR_COUNT (the C row) counts all three.
        let abi = crate::ver::Abi::active();
        let oob = abi.d_rules.iter().find(|r| r.name == "NV2080_CTRL_CMD_INTERNAL_LOG_OOB_XID").unwrap();
        let layout = abi.layout("NV2080_CTRL_INTERNAL_LOG_OOB_XID_PARAMS").unwrap();
        let mut x = vec![0u8; layout.size];
        x[layout.resolve("xid").unwrap().0..][..4].copy_from_slice(&13u32.to_le_bytes());
        let (_, el) = g.rpc(rpc::FN_GSP_RM_CONTROL, &control_request(c, 0xcaf2_0001, oob.id, &x));
        assert_eq!(st(&el), 0);
        let (notes, el) = g.rpc(rpc::FN_GSP_RM_CONTROL, &control_request(c, 0xcaf2_0001, 0x2080_2205, &[0; 4]));
        assert_eq!(st(&el), 0);
        let r = Guest615::reply(&el, rpc::FN_GSP_RM_CONTROL).unwrap().1;
        let count = u32::from_le_bytes(r[v::RM_CONTROL_PARAMS..][..4].try_into().unwrap());
        assert_eq!((count, goe_records(&g)), (3, 2), "{notes:?}");
    }

    /// B3, B7 and B5 against the captured 615 guest: the opt-in and the registry of the
    /// captured GSP_INIT, a refusal for a client without the RPC opt-in, then a PM
    /// suspend (GSP_SUSPEND with PM_TRANSITION, Booter Unload) and the resume (Booter
    /// Load with IN_PM_TRANSITION in RMARGS): no relink, GSP_RESUME_DONE as a GMCAPI
    /// request, the suspended bit cleared.
    #[test]
    fn opt_in_registry_suspend_and_resume() {
        use crate::gspboot::{Unstable, SUSPEND_PM_TRANSITION};
        let mut g = Guest615::boot("resume");
        assert_eq!(g.b.unstable, Some(Unstable { rpc: true, events: true }));
        assert!(g.b.registry.log_on_rc, "RmLogonRC = 1 in the captured registry");
        // GSP_SUSPEND with PM_TRANSITION (kernel_gsp.c:5866-5892).
        let hdr = crate::gmcapi::Header {
            command: crate::gmcapi::CMD_GSP_SUSPEND,
            size: 8,
            sequence: 9,
            request_or_status: 0,
        };
        let mut body = hdr.encode().to_vec();
        body.extend_from_slice(&SUSPEND_PM_TRANSITION.to_le_bytes());
        let (notes, el) = g.send(crate::msgq::v2::NVDM_TYPE_GMCAPI, &body);
        assert!(el.is_empty(), "fire and forget");
        assert!(notes.iter().any(|n| n.contains("GSP_SUSPEND flags 0x1 PM_TRANSITION")), "{notes:?}");
        assert_ne!(g.b.fw.read32(GSP_MAILBOX0).unwrap() & GSP_PROCESSOR_SUSPENDED, 0);
        // Booter Unload (SEC2 run with argument 0), then the resume's Booter Load.
        let sec2 = 0x0084_0000u64;
        g.b.write32(sec2 + falcon::MAILBOX0, 0, &g.mem);
        g.b.write32(sec2 + falcon::MAILBOX1, 0, &g.mem);
        let notes = g.b.write32(sec2 + falcon::CPUCTL, 2, &g.mem).unwrap();
        assert!(notes.iter().any(|n| n.contains("booter unload done")), "{notes:?}");
        let rmargs = 0x0769_e000 + crate::msgq::v2::ARGS_FLAGS;
        let f = g.mem.read_u64(rmargs).unwrap();
        assert!(g.mem.write(rmargs, &(f | crate::gspboot::ARGS_FLAG_IN_PM_TRANSITION).to_le_bytes()));
        let head_before = g.b.read32(0x0011_0C80).unwrap();
        let notes = booter_load(&mut g.b, &g.mem);
        assert!(notes.iter().any(|n| n.starts_with("resume: booter load")), "{notes:?}");
        assert!(!notes.iter().any(|n| n.starts_with("queues linked")), "{notes:?}");
        assert_eq!(g.b.fw.read32(GSP_MAILBOX0).unwrap() & GSP_PROCESSOR_SUSPENDED, 0);
        let el = g.drain();
        assert_eq!(el.len(), 1, "GSP_RESUME_DONE");
        let rd = crate::gmcapi::Header::parse(&el[0].1).unwrap();
        assert_eq!((el[0].0, rd.command, rd.sequence, rd.size), (crate::msgq::v2::NVDM_TYPE_GMCAPI, 0x0001_0005, 0, 8));
        assert_eq!(g.b.read32(0x0011_0C80).unwrap(), head_before + 1, "the pointers continued");
        // The link survives: an RPC after the resume is answered.
        let (_, el) = g.rpc(crate::gsprpc::FN_DUMP_PROTOBUF_COMPONENT, &[0u8; 16]);
        assert_eq!(Guest615::reply(&el, crate::gsprpc::FN_DUMP_PROTOBUF_COMPONENT).unwrap().0, 0);
        // B7: a client without the RPC opt-in gets its RPCs refused.
        g.b.unstable = Some(Unstable { rpc: false, events: false });
        let (_, el) = g.rpc(crate::gsprpc::FN_DUMP_PROTOBUF_COMPONENT, &[0u8; 16]);
        assert_eq!(
            Guest615::reply(&el, crate::gsprpc::FN_DUMP_PROTOBUF_COMPONENT).unwrap().0,
            rpc::NV_ERR_NOT_SUPPORTED
        );
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

    /// A BootModel on a reserved layout with the BAR2 memfd (`defer`: flush reads defer
    /// their sync): BAR2 page 2 maps the data page `x`, the trapped page 0x1f the flush
    /// page. Returns the model, `x` and the guest's view of the memfd.
    fn memfd_model(defer: bool) -> (BootModel, u64, crate::bar2area::SharedView) {
        use crate::bar2area::{Bar2Area, Config};
        const MIB: u64 = 1 << 20;
        let (base, end) = (0x400_0000u64, 0x800_0000u64);
        let layout = crate::fb::FbLayout::reserved(32 * MIB, 256 * MIB, base, end).unwrap();
        let f = crate::fixture::rtx2070();
        let mut b = BootModel::new(f.chip, layout, f.identity(256 * MIB));
        let vid = |a: u64| ((a >> 12) << 8) | 0b010;
        let pte = |a: u64| ((a >> 12) << 8) | 1;
        let (bar2, _) = layout.bar_roots().unwrap();
        let (pd2, pd1, pd0, pt) = (base + 0x6000, base + 0x7000, base + 0x8000, base + 0x9000);
        let x = base + 0x10_0000;
        b.fb.write_u64(bar2, vid(pd2));
        b.fb.write_u64(pd2, vid(pd1));
        b.fb.write_u64(pd1, vid(pd0));
        b.fb.write_u64(pd0 + 8, vid(pt));
        b.fb.write_u64(pt + 8 * 2, pte(x));
        b.fb.write_u64(pt + 8 * 0x1f, pte(base + 0x2_0000));
        let cfg = Config {
            traps: vec![0x1f000],
            sync_doorbell: false,
            defer_flush: defer,
            flush_page_memory: false,
        };
        let area = Bar2Area::create(0x20000, &cfg).unwrap();
        let guest = area.mem.map_again().unwrap();
        b.fb.set_bar2_area(area).unwrap();
        assert!(b.fb.bar2_sync(Some(bar2), None));
        assert_eq!(b.fb.backing(x), crate::fb::Backing::Bar2Memfd(0x2000));
        (b, x, guest)
    }

    fn vram_u64(b: &BootModel, pa: u64) -> u64 {
        let mut w = [0u8; 8];
        b.fb.read_raw(pa, &mut w);
        u64::from_le_bytes(w)
    }

    /// The flush read of BAR2 0x1f000 (docs/caraxes/display-pacing.md): with the deferral
    /// it answers without the sync, the device's reads still see the guest's bytes, and
    /// the next doorbell, MMU invalidate or the flush clock writes them to VRAM first.
    #[test]
    fn a_flush_read_defers_its_sync_to_the_next_hand_over() {
        let mem = GuestMemory::new();
        let (mut b, x, guest) = memfd_model(true);
        let flush = |b: &mut BootModel| {
            let mut d = [0u8; 4];
            b.bar_access(&GuestMemory::new(), 3, 0x1f000, &mut d, false);
        };
        let stats = |b: &BootModel| b.fb.bar2_area().unwrap().stats();
        let syncs0 = stats(&b).syncs;
        // The guest writes x through BAR2 and flushes: nothing synced, the sync is owed.
        guest.store_word(2, 0, 0xABCD);
        flush(&mut b);
        flush(&mut b);
        assert_eq!(stats(&b).syncs, syncs0);
        assert!(b.fb.bar2_flush_pending());
        assert_eq!(vram_u64(&b, x), 0, "VRAM has it only after a hand-over");
        assert_eq!(b.fb.read_u64(x), Some(0xABCD), "device reads see the memfd");
        // A doorbell (the ioeventfd path) settles it before the channels are rung.
        b.doorbell_pending(&mem);
        assert_eq!(vram_u64(&b, x), 0xABCD);
        assert!(!b.fb.bar2_flush_pending());
        assert_eq!(stats(&b).syncs, syncs0 + 1);
        // Without an owed flush a doorbell does not sync (sync_doorbell is off).
        b.doorbell_pending(&mem);
        assert_eq!(stats(&b).syncs, syncs0 + 1);
        // The flush clock settles one no hand-over did.
        guest.store_word(2, 1, 0x1234);
        flush(&mut b);
        assert!(b.settle_deferred_flush(&mem, crate::bar2area::Settle::Clock));
        assert_eq!(vram_u64(&b, x + 8), 0x1234);
        assert!(!b.settle_deferred_flush(&mem, crate::bar2area::Settle::Clock));
        // The MMU invalidate settles it (the RM flushes, then invalidates).
        guest.store_word(2, 2, 0x5678);
        flush(&mut b);
        let r = b.regs;
        b.write32(r.mmu_invalidate_pdb, 0, &mem).unwrap();
        b.write32(r.mmu_invalidate_upper_pdb, 0, &mem).unwrap();
        b.write32(r.mmu_invalidate, 0x8000_0005, &mem).unwrap();
        assert_eq!(vram_u64(&b, x + 16), 0x5678);
        let s = stats(&b);
        assert_eq!(
            (s.deferred, s.settled_doorbell, s.settled_clock, s.settled_sync_point),
            (4, 1, 1, 1)
        );
        assert!(b.fb.bar2_area().unwrap().summary().contains("4 deferred"));
    }

    /// `CARAXES_BAR2_MEMFD_DEFER_FLUSH=0`: every flush read syncs, as before.
    #[test]
    fn without_the_deferral_every_flush_read_syncs() {
        let (mut b, x, guest) = memfd_model(false);
        let syncs0 = b.fb.bar2_area().unwrap().stats().syncs;
        guest.store_word(2, 0, 0xABCD);
        let mut d = [0u8; 4];
        b.bar_access(&GuestMemory::new(), 3, 0x1f000, &mut d, false);
        assert_eq!(vram_u64(&b, x), 0xABCD);
        assert!(!b.fb.bar2_flush_pending());
        assert_eq!(b.fb.bar2_area().unwrap().stats().syncs, syncs0 + 1);
        assert!(b.fb.bar2_area().unwrap().summary().contains("flush reads sync at once"));
    }

    /// The GPPut barrier's decision (display-e.md §9): a store that covers USERD + 0x8C of
    /// a mirrored channel waits for the shadow worker; GPGet, the rest of USERD and other
    /// channels' USERD do not.
    #[test]
    fn gp_put_stores_of_mirrored_channels_are_found() {
        let userds = [0xE918_FE00u64, 0x1_0000_0000];
        assert!(stores_gp_put(&userds, 0xE918_FE8C, 4), "the guest's 32-bit GPPut store");
        assert!(stores_gp_put(&userds, 0xE918_FE88, 8), "GPGet and GPPut in one store");
        assert!(stores_gp_put(&userds, 0xE918_FE8E, 1), "a byte of GPPut");
        assert!(stores_gp_put(&userds, 0x1_0000_008C, 4), "the second channel");
        assert!(!stores_gp_put(&userds, 0xE918_FE88, 4), "GPGet alone");
        assert!(!stores_gp_put(&userds, 0xE918_FE90, 4), "past GPPut");
        assert!(!stores_gp_put(&userds, 0xE918_FE40, 4), "elsewhere in USERD");
        assert!(!stores_gp_put(&userds, 0xE919_008C, 4), "a channel that is not mirrored");
        assert!(!stores_gp_put(&[], 0xE918_FE8C, 4), "no mirrored channel");
    }
}

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Channel emulation for experiment E5a (docs/caraxes/uvm.md): the host (PBDMA)
//! and copy-engine methods that `nvidia-uvm` pushes while it registers a GPU,
//! executed by the device against the fake framebuffer and guest RAM. No work
//! reaches the physical GPU.
//!
//! What is modelled:
//! - the guest's channels, TSGs and GPU virtual address spaces as the guest RM
//!   announces them through `GSP_RM_ALLOC` and two controls (the VAS root);
//! - the usermode doorbell (`NVC361_NOTIFY_CHANNEL_PENDING`, BAR0 0xBB0090 on TU106,
//!   the chip's `registers.usermode_doorbell`): the token selects a channel, whose
//!   GPPut is read from its USERD;
//! - GPFIFO entries (`clc46f.h:264-282`) and the pushbuffer method stream
//!   (`clc46f.h:285-358`);
//! - host methods `SET_OBJECT`, `NOP`, `MEM_OP_*`, `SEM_*`, `WFI`,
//!   `NON_STALL_INTERRUPT` (`clc46f.h:68-254`);
//! - the `TURING_DMA_COPY_A` subset UVM uses (`kernel-open/nvidia-uvm/uvm_turing_ce.c`):
//!   semaphore release (one and four words, reductions), physical and virtual
//!   copies and constant memsets (`clc5b5.h:35-206`);
//! - GPGet written back into USERD +0x88 (`clc46f.h:48-64`).
//!
//! Every value this module writes into guest-visible memory is either computed
//! from what the guest pushed (label `guest`) or a timestamp from the device
//! clock (label `synthesized`).
//!
//! The channel and copy classes come from the chip (`chips/<chip>/chip.toml`,
//! `[channel]`); the method decoding below is that of the Volta/Turing host class
//! (`clc46f.h`) and the Turing copy class (`clc5b5.h`), which the chip data names. The
//! allocation and control parameter layouts are the guest ABI's ([`crate::ver`]).

use std::collections::BTreeMap;

use nvrm_sys::RmAbi;

use crate::chip::ChannelClasses;
use crate::fb::{FakeFb, Target};
use crate::guestmem::GuestMemory;
use crate::ver::common::{
    self as v, ADDR_FBMEM, ADDR_SYSMEM, CTXSHARE_HVASPACE, FERMI_CONTEXT_SHARE_A, FERMI_VASPACE_A,
    KEPLER_CHANNEL_GROUP_A, TSG_ENGINE_TYPE, TSG_HVASPACE,
};

/// The guest's channel allocation offsets: `NV_CHANNEL_ALLOC_PARAMS` is the same in every
/// guest ABI (hostabi.rs asserts 610 and 615 equal field by field); the TSG allocation is
/// not, and comes from `ver::layouts()`.
type Abi = nvrm_sys::V610;

/// The page shift of a Turing (GMMU v2) VA space's root directory, PD3 (ptcache.rs).
const ROOT_PAGE_SHIFT_V2: u8 = 47;

/// USERD offsets of GPGet and GPPut (`clc46f.h:61-62`).
const USERD_GP_GET: u64 = 0x88;
const USERD_GP_PUT: u64 = 0x8C;
/// Hard diagnostic bounds: semaphore capture is optional and must not grow with runtime.
const SEM_DIAG_TUPLES: usize = 32;
const SEM_DIAG_PEERS: usize = 32;
/// `NV_VASPACE_ALLOCATION_INDEX_GPU_DEVICE` (nvos.h:3178): "acquire reference to device
/// vaspace".
const VAS_INDEX_GPU_DEVICE: u32 = 3;
/// The promote-entry id under which a falcon engine's context (NVENC, NVDEC, the
/// top-level `virtAddress` of `GPU_PROMOTE_CTX`, falcon/kernel_falcon.c:266-270) is kept:
/// GR buffer ids stop at 12 (`NV2080_CTRL_GPU_PROMOTE_CTX_BUFFER_ID_*`).
pub const PROMOTE_ID_FALCON_CTX: u16 = 0x100;

/// Where a GPU virtual address space's root page directory lives.
fn gmmu_aperture(aperture: u32, pa: u64) -> Option<Target> {
    // GMMU_APERTURE_VIDEO 1, _PEER 2, _SYS_NONCOH 3, _SYS_COH 4
    // (`src/nvidia/inc/libraries/mmu/gmmu_fmt.h:291-323`).
    match aperture {
        1 => Some(Target::Fb(pa)),
        3 | 4 => Some(Target::Sys(pa)),
        _ => None,
    }
}

/// The engines' view of memory: the fake FB and guest RAM.
pub struct GpuMem<'a> {
    pub fb: &'a mut FakeFb,
    pub sys: &'a GuestMemory,
}

impl GpuMem<'_> {
    pub fn read(&self, t: Target, buf: &mut [u8]) -> bool {
        match t {
            Target::Fb(pa) => self.fb.read(pa, buf),
            Target::Sys(pa) => self.sys.read(pa, buf),
            _ => false,
        }
    }

    pub fn write(&mut self, t: Target, data: &[u8]) -> bool {
        match t {
            Target::Fb(pa) => self.fb.write(pa, data),
            Target::Sys(pa) => self.sys.write(pa, data),
            _ => false,
        }
    }

    pub fn read_u32(&self, t: Target) -> Option<u32> {
        let mut b = [0u8; 4];
        self.read(t, &mut b).then(|| u32::from_le_bytes(b))
    }

    /// Resolve `len` bytes at `va` page by page (4 KiB is the smallest GMMU page,
    /// so every larger page splits consistently) into physical pieces. The tables are read
    /// once per 2 MiB unit (fb.rs `translate_pages`), not per page.
    fn pieces(&self, root: Target, va: u64, len: u64) -> Result<Vec<(Target, usize)>, String> {
        let _spent = crate::dlock::Spend::new(crate::dlock::Part::Translate);
        let mut out = Vec::new();
        let mut a = va;
        for (t, n) in self.fb.translate_pages(Some(self.sys), root, va, len) {
            match t {
                Target::Fb(_) | Target::Sys(_) => out.push((t, n)),
                other => return Err(format!("va {a:#x}: {other:?}")),
            }
            a += n as u64;
        }
        Ok(out)
    }

    pub fn read_va(&self, root: Target, va: u64, buf: &mut [u8]) -> Result<(), String> {
        let mut off = 0usize;
        for (t, n) in self.pieces(root, va, buf.len() as u64)? {
            if !self.read(t, &mut buf[off..off + n]) {
                return Err(format!("read {t:?}: outside FB or guest RAM"));
            }
            off += n;
        }
        Ok(())
    }

    pub fn write_va(&mut self, root: Target, va: u64, data: &[u8]) -> Result<(), String> {
        let mut off = 0usize;
        for (t, n) in self.pieces(root, va, data.len() as u64)? {
            if !self.write(t, &data[off..off + n]) {
                return Err(format!("write {t:?}: outside FB or guest RAM"));
            }
            off += n;
        }
        Ok(())
    }
}

/// Copy-engine method state (`clc5b5.h`).
#[derive(Clone, Debug, Default)]
struct CeState {
    sem_upper: u32,
    sem_lower: u32,
    sem_payload: u32,
    src_phys: u32,
    dst_phys: u32,
    offset_in: (u32, u32),
    offset_out: (u32, u32),
    pitch_in: u32,
    pitch_out: u32,
    line_length_in: u32,
    line_count: u32,
    remap_const_a: u32,
    remap_const_b: u32,
    remap_components: u32,
    /// A transfer run in slices ([`CE_SLICE`]): how far it got.
    progress: Option<CeProgress>,
    /// A transfer running on the device CE (devce.rs): the push it waits for.
    device_wait: Option<DeviceWait>,
}

/// The device-CE push a LAUNCH_DMA waits for, and the FB ranges it writes.
#[derive(Clone)]
struct DeviceWait {
    key: CeKey,
    seq: u32,
    ranges: Vec<(u64, u64)>,
    engine: crate::devce::SharedEngine,
}

impl std::fmt::Debug for DeviceWait {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DeviceWait {{ push {}, {} range(s) }}",
            self.seq,
            self.ranges.len()
        )
    }
}

/// The LAUNCH_DMA a sliced transfer belongs to: its value and geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CeKey {
    v: u32,
    src: u64,
    dst: u64,
    lines: u64,
    bytes_per_line: u64,
}

/// Where a sliced transfer continues: the row and the byte offset in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CeProgress {
    key: CeKey,
    line: u64,
    offset: u64,
}

/// What a LAUNCH_DMA did: finished (its notes), or ran one slice of a long transfer, which
/// the channel continues at the same method ([`MethodError::Blocked`], the held replay).
#[derive(Debug)]
enum CeOutcome {
    Done(String),
    Sliced(String),
}

/// Sliced software CE transfers are pending: the device's `ce-slices` clock continues them
/// ([`ce_slices_pending`]).
static CE_SLICES_PENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether a software CE transfer waits for its next slice (cleared by the check).
pub fn ce_slices_pending() -> bool {
    CE_SLICES_PENDING.swap(false, std::sync::atomic::Ordering::AcqRel)
}

/// A software channel waits on a semaphore acquire ([`AcquireWait`]): the `ce-slices` clock
/// re-reads the semaphore ([`Registry::ready_acquires`]).
static ACQUIRE_WAITS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether a software channel may wait on a semaphore acquire (cleared by the check; the
/// check sets it again while one still waits).
pub fn acquire_waits_pending() -> bool {
    ACQUIRE_WAITS.swap(false, std::sync::atomic::Ordering::AcqRel)
}

/// The semaphore acquire a software channel is blocked on: the host re-evaluates an acquire
/// until it is satisfied (clc46f.h SEM_EXECUTE ACQUIRE*, the legacy SEMAPHORED ACQUIRE
/// forms), so the executor re-reads this word at every tick of the `ce-slices` clock and
/// continues the channel as soon as the release is there. Until 2026-09-29 a blocked acquire
/// was retried only at the next doorbell or budget stop of any channel: C79's Sunshine
/// capture setup lost 1.58 s in 59 such waits of the Windows KMD's channel (up to 401 ms).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcquireWait {
    pub root: Target,
    pub va: u64,
    /// 8-byte payload (SEM_EXECUTE PAYLOAD_SIZE_64BIT), else 4.
    pub wide: bool,
    pub payload: u64,
    pub op: AcquireOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcquireOp {
    /// `== payload`.
    Eq,
    /// `>= payload`.
    Geq,
    /// `(cur - payload)` as signed `>= 0` (ACQ_CIRC_GEQ).
    CircGeq,
    /// `cur & payload != 0` (the legacy ACQUIRE_AND).
    And,
}

impl AcquireOp {
    fn ready(self, cur: u64, want: u64, wide: bool) -> bool {
        let (cur, want) = if wide { (cur, want) } else { (cur & 0xFFFF_FFFF, want & 0xFFFF_FFFF) };
        match self {
            AcquireOp::Eq => cur == want,
            AcquireOp::Geq => cur >= want,
            AcquireOp::CircGeq => {
                if wide {
                    (cur.wrapping_sub(want) as i64) >= 0
                } else {
                    ((cur as u32).wrapping_sub(want as u32) as i32) >= 0
                }
            }
            AcquireOp::And => cur & want != 0,
        }
    }
}

/// Compute-class stub state (`clc5c0.h`, E5c): report semaphores and inline-to-memory.
#[derive(Clone, Debug, Default)]
struct ComputeState {
    sem: [u32; 3],
    i2m_sem: [u32; 3],
    line_length_in: u32,
    line_count: u32,
    offset_out: (u32, u32),
    pitch_out: u32,
    /// An inline-to-memory transfer waiting for its LOAD_INLINE_DATA dwords:
    /// (LAUNCH_DMA value, bytes expected, bytes so far).
    i2m: Option<(u32, usize, Vec<u8>)>,
    pcas_a: u32,
    /// SET_INLINE_QMD_ADDRESS_A/B and the 64 LOAD_INLINE_QMD_DATA dwords.
    qmd_addr: (u32, u32),
    qmd: Vec<u32>,
}

/// One entry of `GPU_PROMOTE_CTX` (ctrl2080gpu.h:893-903) for a channel or TSG, merged
/// per buffer id over the kernel's PA promotion and UVM's VA promotion. Kept for part 2,
/// which has to give the host context the same buffers (cuda-path.md §4.3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromoteEntry {
    pub buffer_id: u16,
    pub phys: Option<(u64, u32)>,
    pub virt: Option<u64>,
    pub size: u64,
    pub initialize: bool,
    pub nonmapped: bool,
}

/// Host method state (`clc46f.h`).
#[derive(Clone, Debug, Default)]
struct HostState {
    legacy_sem: [u32; 3],
    sem_addr_lo: u32,
    sem_addr_hi: u32,
    sem_payload_lo: u32,
    sem_payload_hi: u32,
    mem_op: [u32; 3],
}

#[derive(Clone, Copy, Debug, Default)]
struct StrictReportState {
    words: [u32; 3],
    initialized: u8,
}

#[derive(Clone, Copy, Debug)]
enum DeferredState {
    Pending { object: u32, api: u32 },
    Completed { object: u32, api: u32 },
}

/// A read-only, bounded validation of the host-executed prefix of a management channel.
/// Its fields stay private so only this registry can apply it after host teardown.
#[derive(Debug)]
pub struct DeferredTakeover {
    chid: u32,
    generation: u64,
    gp_get: u32,
    bindings: [u32; 8],
}

/// One guest channel as the guest RM allocated it.
#[derive(Clone, Debug)]
pub struct Channel {
    pub h_client: u32,
    pub h_object: u32,
    /// The parent of the allocation: a TSG (`KEPLER_CHANNEL_GROUP_A`) or the device.
    pub h_parent: u32,
    /// Engine objects the guest allocated under the channel: `(class, params)` in
    /// allocation order (the mirror replays them on the host channel, part 2).
    pub engine_objects: Vec<(u32, Vec<u8>)>,
    /// Guest handles corresponding one-for-one with `engine_objects`.
    engine_object_handles: Vec<u32>,
    /// Channel-local ENG_SW class IDs assigned in engine-object allocation order.
    software_objects: BTreeMap<u16, (u32, u32)>,
    next_software_object: u16,
    software_object_model_valid: bool,
    /// Runs on a host mirror channel: the software executor leaves it alone.
    pub mirrored: bool,
    /// The guest stopped it (`NVA06F_CTRL_CMD_STOP_CHANNEL`): a PREEMPTIVE_REMOVAL in its
    /// error notifier is the guest kernel's own stop notice, not a host RC (boot.rs).
    pub stopped: bool,
    pub chid: u32,
    /// Registry-unique identity for this allocation, even when the guest reuses a ChID.
    pub generation: u64,
    pub engine_type: u32,
    /// Guest allocation flags, decoded with the selected guest ABI. These never grant
    /// the host channel additional privileges.
    pub alloc_flags: u32,
    pub gpfifo_va: u64,
    pub gpfifo_entries: u32,
    /// USERD (GPGet/GPPut page), from `userdMem`.
    pub userd: Option<Target>,
    /// The VAS the channel runs in: its own `hVASpace` or its TSG's.
    pub vas: Option<(u32, u32)>,
    pub error_notifier: Option<Target>,
    /// The memory the guest handed for the channel's own structures, which the card reads by
    /// physical address: `instanceMem` (the instance block, `NV_RAMIN`, 4 KiB,
    /// maxwell/gm107/dev_ram.h:37), `ramfcMem` (`NV_RAMIN_RAMFC`, turing/tu102/dev_ram.h:27,
    /// 52) and `mthdbufMem` (`NV_RAMIN_ENG_METHOD_BUFFER_ADDR`, :41-42), each `(start,
    /// bytes)`; USERD is [`Self::userd`]. The BAR2 memfd's doorbell rule counts them as
    /// memory the channel's work could observe (boot.rs `bar2_observed`).
    pub structures: Vec<(Target, u64)>,
    /// 615: the internal TSG the kernel created for a bare channel
    /// (`NV_KERNELCHANNEL_ALLOC_INTERNALFLAGS_INTERNAL_TSG_ID` 23:8, `_VALID` 24,
    /// src/nvidia/generated/g_kernel_channel_nvoc.h:199-202 of 615;
    /// kernel_channel.c:2786-2799), which GSP-RM creates under that id.
    pub internal_tsg_id: Option<u32>,
    gp_get: u32,
    /// USERD GPPut as read at the channel's last doorbell on any path (software, shadow,
    /// mirror): the token-less ioeventfd doorbell path rings a channel only when the
    /// guest published GPFIFO entries since ([`Registry::pending`]).
    gp_put_seen: u32,
    /// Resume point after a host semaphore acquire that was not yet satisfied:
    /// the pushbuffer dword index of the blocking method within entry `gp_get`.
    resume: Option<usize>,
    /// Class bound to each subchannel with `SET_OBJECT`.
    objects: [u32; 8],
    ce: CeState,
    compute: ComputeState,
    host: HostState,
    strict_reports: [StrictReportState; 8],
    pub entries_done: u64,
    pub methods_done: u64,
    /// Compute work (kernel launches) the device refused to run: part 2.
    pub launches_refused: u64,
    /// `MEM_OP` TLB invalidates of this channel's pushes since the registry drained them:
    /// the page directory each names (`None`: all, pdbwatch.rs `memop_pdb`).
    tlb_invalidates: Vec<Option<Target>>,
    /// A failed software method is terminal until this channel is reallocated.
    software_failed: bool,
    software_failure: Option<String>,
    software_nonstall: bool,
    software_fb_range: Option<(u64, u64)>,
    /// The software CE's slice ([`CE_SLICE`]; the registry's, copied at each run).
    ce_slice: std::time::Duration,
    /// The last run stopped at its budget ([`RUN_BUDGET`]) with entries left: the slice
    /// clock continues it at the same GPGet ([`Registry::blocked_software_channels`]).
    budget_stop: bool,
    /// Fail-closed software execution for management channels whose deferred APIs are
    /// completed by the dispatcher through a prepared host context.
    deferred_management: bool,
    deferred: Option<DeferredState>,
    /// Semaphore diagnostics already emitted as `(VA, payload)`; bounds retry logging.
    sem_diag_seen: std::collections::BTreeSet<(u64, u32)>,
    /// The acquire the channel's last run blocked on ([`AcquireWait`]).
    acquire_wait: Option<AcquireWait>,
}

impl Channel {
    /// NVOS04_FLAGS_PRIVILEGED_CHANNEL, bit 5 in alloc_channel.h.
    pub fn privileged(&self) -> bool {
        self.alloc_flags & (1 << 5) != 0
    }

    /// Engine objects in guest allocation order, including their exact guest handles.
    pub fn engine_objects_with_handles(&self) -> impl Iterator<Item = (u32, u32, &[u8])> {
        self.engine_object_handles
            .iter()
            .copied()
            .zip(self.engine_objects.iter())
            .map(|(handle, (class, params))| (handle, *class, params.as_slice()))
    }

    fn bound_class(&self, binding: u32) -> u32 {
        u16::try_from(binding)
            .ok()
            .and_then(|id| self.software_objects.get(&id))
            .map_or(binding, |(_, class)| *class)
    }

    fn validate_deferred_management(&self, classes: ChannelClasses) -> Result<(), String> {
        if !self.software_object_model_valid {
            return Err(format!(
                "chid {}: channel-local software class ID sequence is not modelled",
                self.chid
            ));
        }
        let allowed = |class| {
            matches!(class, 0x5080 | 0xa140 | 0x902d | 0xc597)
                || class == classes.uvm_sw
                || class == classes.copy
                || class == classes.compute
        };
        if let Some((class, _)) = self
            .engine_objects
            .iter()
            .find(|(class, _)| !allowed(*class))
        {
            return Err(format!(
                "chid {}: strict management has unsupported engine class {class:#x}",
                self.chid
            ));
        }
        let live = self
            .engine_objects_with_handles()
            .map(|(handle, class, _)| (handle, class))
            .collect::<BTreeMap<_, _>>();
        if self
            .software_objects
            .values()
            .any(|(handle, class)| live.get(handle) != Some(class))
        {
            return Err(format!(
                "chid {}: channel-local software class ID names a stale object",
                self.chid
            ));
        }
        let deferred = live.iter().filter(|(_, class)| **class == 0x5080);
        let mut count = 0;
        for (&handle, _) in deferred {
            count += 1;
            if !self
                .software_objects
                .values()
                .any(|entry| *entry == (handle, 0x5080))
            {
                return Err(format!(
                    "chid {}: deferred object {handle:#x} has no channel-local software class ID",
                    self.chid
                ));
            }
        }
        if count == 0 {
            return Err(format!("chid {}: no live deferred object", self.chid));
        }
        Ok(())
    }
}

/// The device clock's epoch: one per device process, the base of `NV_PTIMER_TIME_0/1`,
/// the usermode TIME_0/1, semaphore timestamps, the time correlation, and the display
/// engine's vblank clock and notifier timestamps (display/engine.rs), so every time the
/// guest reads or finds in memory is on one base. A function-level reset does not restart
/// it: the RM sets PTIMER to the wall-clock time before every GSP boot
/// (`kernel_gsp.c:5677-5682`, `tmrSetCurrentTime_GV100`, timer_gv100.c:47-72), so on the
/// card the timer never goes back across a reset either.
pub fn device_epoch() -> std::time::Instant {
    static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(std::time::Instant::now)
}

/// Nanoseconds of the device clock ([`device_epoch`]).
pub fn device_now_ns() -> u64 {
    device_epoch().elapsed().as_nanos() as u64
}

/// Completed software work or a terminal method failure, drained by the dispatcher.
pub struct SoftwareEvent {
    pub chid: u32,
    pub engine_type: u32,
    pub failure: Option<String>,
    pub nonstall: bool,
}

/// Everything the device knows about the guest's channels.
/// `NV01_DEVICE_0`.
const NV01_DEVICE_0_CLASS: u32 = 0x80;

pub struct Registry {
    /// The chip's channel, copy and UVM software classes.
    classes: ChannelClasses,
    /// (hClient, hVASpace) → root page directory, once a control revealed it.
    vaspaces: BTreeMap<(u32, u32), Option<Target>>,
    /// (hClient, hVASpace) → entries of the root page directory (the guest's
    /// `numEntries`, or the size of `levels[0]` in entries).
    root_entries: BTreeMap<(u32, u32), u32>,
    /// (hClient, hVASpace) → the guest's `NV_VASPACE_ALLOCATION_PARAMETERS`, and its
    /// `COPY_SERVER_RESERVED_PDES` parameters: what the host mirror replays (part 2).
    vas_params: BTreeMap<(u32, u32), Vec<u8>>,
    server_pdes: BTreeMap<(u32, u32), Vec<u8>>,
    /// (hClient, hTsg) → (hVASpace (0: the device's default VAS), engineType).
    tsgs: BTreeMap<(u32, u32), (u32, u32)>,
    /// 615: the TSG id (grpID) the kernel assigned and GSP-RM takes over
    /// (`NV_KERNELCHANNELGROUP_ALLOC_INTERNALFLAGS_TSG_ID` 15:0, `_VALID` 16,
    /// g_kernel_channel_group_nvoc.h:123-129; set at kernel_channel_group_api.c:442-455,
    /// honoured by the GSP build at :273-285), by (hClient, hTsg).
    tsg_ids: BTreeMap<(u32, u32), u32>,
    /// (hClient, hCtxShare) → hVASpace (`FERMI_CONTEXT_SHARE_A`): GR channels name their
    /// VAS only through their context share (cuda-lookahead.md §5 item 2).
    ctxshares: BTreeMap<(u32, u32), u32>,
    /// (hClient, hObject) of a `DUP_OBJECT` → its source, for VA spaces: UVM duplicates
    /// CUDA's VAS and sets the page directory on its own handle (§5 item 1).
    aliases: BTreeMap<(u32, u32), (u32, u32)>,
    /// (hChanClient, hObject) → promoted context buffers by buffer id.
    promoted: BTreeMap<(u32, u32), BTreeMap<u16, PromoteEntry>>,
    /// The generation of what a shadow root's set B is computed from here (mirror.rs
    /// `exclusions_of`): the channels, how their VA spaces resolve to a root (`vaspaces`,
    /// `aliases`, `default_vas`, `device_vas`, `client_device`, `client_share`) and the
    /// promoted buffers. A fresh stamp (ptcache.rs `fresh_generation`) at every change of
    /// those ([`Self::set_b_changed`]); [`Self::channel_mut`] callers change none of them.
    set_b_gen: u64,
    /// (hClient, hVASpace) of a `FERMI_VASPACE_A` with `index` GPU_DEVICE → its device:
    /// the kernel announces a device's default VAS to GSP through such a transient
    /// handle, `COPY_SERVER_RESERVED_PDES` on it and its FREE
    /// (gpu_vaspace.c:3877-3921); the root then lives on under `(hClient, hDevice)`.
    device_vas: BTreeMap<(u32, u32), u32>,
    /// (hClient, hDevice) → the private `FERMI_VASPACE_A` made that device's default by
    /// `NV0080_CTRL_DMA_SET_DEFAULT_VASPACE`. Unlike `device_vas`, this is a live object
    /// association: page-directory changes on the VAS are visible to every sharing client.
    default_vas: BTreeMap<(u32, u32), (u32, u32)>,
    /// The device allocated by each client. RM clients in this model own one GPU device;
    /// keeping its handle makes a sharing client's lookup select the corresponding default.
    client_device: BTreeMap<u32, u32>,
    /// hClient → hClientShare of its `NV01_DEVICE_0` (`NV0080_ALLOC_PARAMETERS.hClientShare`
    /// @4): a client whose device shares another client's VA space. The Windows KMD opens one
    /// client per channel with the device sharing its main client's VAS; a channel there that
    /// names no VAS runs in the share client's default VAS.
    client_share: BTreeMap<u32, u32>,
    /// ChID → channel.
    channels: BTreeMap<u32, Channel>,
    /// Last generation assigned to an accepted channel allocation.
    channel_generation: u64,
    /// Device clock base for semaphore timestamps: [`device_epoch`], shared with PTIMER
    /// and the display engine and kept across a function-level reset.
    start: Option<std::time::Instant>,
    pub doorbells: u64,
    /// TLB invalidates the software executor ran into (`MEM_OP` MMU_TLB_INVALIDATE):
    /// the page directory (`None`: all), for the host mirrors of the same page
    /// directories (part 2); drained by the dispatcher after each doorbell.
    pub tlb_invalidates: Vec<Option<Target>>,
    /// The shadow roots whose `MEM_OP` TLB invalidate stops the software executor right
    /// after the method, for one doorbell ([`Self::doorbell_holding`]); `None`: never stop.
    memop_hold: Option<std::collections::BTreeSet<u64>>,
    /// Channels stopped after such an invalidate, to continue once the shadow is applied.
    held_at_invalidate: Vec<u32>,
    /// The guest's TLB invalidates against the VAS roots recorded here (fix 2 of run P17).
    pdb_watch: crate::pdbwatch::PdbWatch,
    /// The doorbell token pool (`CARAXES_DOORBELL_TOKENS`, tokens.rs): which channel
    /// holds which pool token.
    pub tokens: Option<crate::tokens::TokenPool>,
    /// Host-backed execution confines software CE accesses to this VM's reservation.
    pub software_fb_range: Option<(u64, u64)>,
    /// The software CE's slice ([`CE_SLICE`]; shorter in tests).
    pub ce_slice: std::time::Duration,
    /// A software run's budget between GPFIFO entries ([`RUN_BUDGET`]; other in tests).
    pub run_budget: std::time::Duration,
    /// Lets the dispatcher avoid software-event scans on the mirrored hot path.
    pub software_runs: u64,
    sem_diag: bool,
    sem_diag_chid: Option<u32>,
    /// The guest's `GR_SET_CTXSW_PREEMPTION_MODE` requests by guest TSG (or bare channel),
    /// for the host TSG's build (preempt.rs, [`Self::record_preemption`]).
    preempt: crate::preempt::Ledger,
}

fn rd32(b: &[u8], off: usize) -> u32 {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .unwrap_or(0)
}

fn rd64(b: &[u8], off: usize) -> u64 {
    b.get(off..off + 8)
        .map(|s| u64::from_le_bytes(s.try_into().unwrap()))
        .unwrap_or(0)
}

/// `NV_MEMORY_DESC_PARAMS` {base u64, size u64, addressSpace u32, cacheAttrib u32}
/// (`alloc_channel.h:51-56`) at `off`.
fn mem_desc(b: &[u8], off: usize) -> Option<Target> {
    let base = rd64(b, off + v::MEM_DESC_BASE);
    match rd32(b, off + v::MEM_DESC_ADDRESS_SPACE) {
        ADDR_FBMEM => Some(Target::Fb(base)),
        ADDR_SYSMEM => Some(Target::Sys(base)),
        _ => None,
    }
}

impl Registry {
    pub fn new(classes: ChannelClasses) -> Self {
        Registry {
            classes,
            vaspaces: BTreeMap::new(),
            root_entries: BTreeMap::new(),
            vas_params: BTreeMap::new(),
            server_pdes: BTreeMap::new(),
            tsgs: BTreeMap::new(),
            tsg_ids: BTreeMap::new(),
            ctxshares: BTreeMap::new(),
            aliases: BTreeMap::new(),
            promoted: BTreeMap::new(),
            set_b_gen: crate::ptcache::fresh_generation(),
            device_vas: BTreeMap::new(),
            default_vas: BTreeMap::new(),
            client_device: BTreeMap::new(),
            client_share: BTreeMap::new(),
            channels: BTreeMap::new(),
            channel_generation: 0,
            start: Some(device_epoch()),
            doorbells: 0,
            tlb_invalidates: Vec::new(),
            memop_hold: None,
            held_at_invalidate: Vec::new(),
            pdb_watch: crate::pdbwatch::PdbWatch::default(),
            tokens: None,
            software_fb_range: None,
            ce_slice: CE_SLICE,
            // Unit tests run whole doorbells unless they test the budget.
            run_budget: if cfg!(test) { std::time::Duration::from_secs(1) } else { RUN_BUDGET },
            software_runs: 0,
            sem_diag: std::env::var("CARAXES_SEMAPHORE_DIAG").as_deref() == Ok("1"),
            sem_diag_chid: std::env::var("CARAXES_SEMAPHORE_DIAG_CHID")
                .ok()
                .and_then(|s| s.parse().ok()),
            preempt: crate::preempt::Ledger::default(),
        }
    }

    pub fn channels(&self) -> impl Iterator<Item = &Channel> {
        self.channels.values()
    }

    /// Find an engine object by its client-scoped guest handle.
    ///
    /// The ChID and generation identify its exact owning channel allocation. Callers
    /// must reject a duplicate live identity before recording a replacement.
    pub fn engine_object(&self, h_client: u32, h_object: u32) -> Option<(u32, u64, u32, &[u8])> {
        self.channels
            .values()
            .filter(|ch| ch.h_client == h_client)
            .find_map(|ch| {
                ch.engine_object_handles
                    .iter()
                    .position(|h| *h == h_object)
                    .map(|i| {
                        let (class, params) = &ch.engine_objects[i];
                        (ch.chid, ch.generation, *class, params.as_slice())
                    })
            })
    }

    pub fn engine_owner(&self, h_client: u32, h_object: u32) -> Option<(u32, u64)> {
        self.engine_object(h_client, h_object)
            .map(|(chid, generation, _, _)| (chid, generation))
    }

    pub fn enable_deferred_management(&mut self, chid: u32) -> Result<(), String> {
        let classes = self.classes;
        let ch = self
            .channels
            .get_mut(&chid)
            .ok_or_else(|| format!("chid {chid}: no such channel"))?;
        if ch.mirrored || ch.software_failed {
            return Err(format!(
                "chid {chid}: cannot enable strict management execution"
            ));
        }
        ch.validate_deferred_management(classes)?;
        ch.deferred_management = true;
        Ok(())
    }

    /// Validate the bounded prefix already consumed by an idle host channel without
    /// changing registry state. The caller may safely do this before freeing host objects.
    pub fn preview_deferred_takeover(
        &self,
        mem: &GpuMem,
        chid: u32,
        gp_get: u32,
    ) -> Result<DeferredTakeover, String> {
        let ch = self
            .channels
            .get(&chid)
            .ok_or_else(|| format!("chid {chid}: no such channel"))?;
        let entries = ch.gpfifo_entries.max(1);
        if gp_get > 32 || gp_get >= entries {
            return Err(format!(
                "chid {chid}: harvested GPGet {gp_get} is outside the bounded {entries}-entry prefix"
            ));
        }
        if ch.software_failed || ch.deferred.is_some() {
            return Err(format!(
                "chid {chid}: cannot take over active management state"
            ));
        }
        ch.validate_deferred_management(self.classes)?;
        if ch.gp_get != 0 {
            return Err(format!(
                "chid {chid}: management takeover after a software ring wrap is unsupported"
            ));
        }
        let root = self
            .root_of(ch)
            .ok_or_else(|| format!("chid {chid}: VAS root unknown"))?;
        let mut bindings = ch.objects;
        for entry in 0..gp_get {
            let mut descriptor = [0; 8];
            mem.read_va(root, ch.gpfifo_va + 8 * u64::from(entry), &mut descriptor)
                .map_err(|e| format!("chid {chid}: initial GPFIFO entry {entry}: {e}"))?;
            let e0 = rd32(&descriptor, 0);
            let e1 = rd32(&descriptor, 4);
            let dwords = ((e1 >> 10) & 0x1f_ffff) as usize;
            if dwords == 0 || dwords > 4096 {
                return Err(format!(
                    "chid {chid}: initial entry {entry} has unsupported length {dwords}"
                ));
            }
            let va = ((e1 as u64 & 0xff) << 32) | (e0 as u64 & !3);
            let mut raw = vec![0; dwords.checked_mul(4).ok_or("pushbuffer size overflow")?];
            mem.read_va(root, va, &mut raw)
                .map_err(|e| format!("chid {chid}: initial pushbuffer {entry}: {e}"))?;
            let words = raw
                .chunks_exact(4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
                .collect::<Vec<_>>();
            let diagnostic = |subch: usize, bound_class: u32, method: u32, data: u32, why: &str| {
                let shown = words.len().min(256);
                let raw = words[..shown]
                    .iter()
                    .map(|word| format!("{word:08x}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!(
                    "chid {chid}: takeover prefix rejected at entry {entry}/{gp_get}, subch {subch}, bound class {:#x}, method {method:#x}, data {data:#x}: {why}; entry {dwords} dwords, raw[0..{shown}] [{raw}]{}",
                    bound_class,
                    if shown < words.len() { " (truncated)" } else { "" }
                )
            };
            let state_setter = |binding: u32, method: u32| {
                let class = u16::try_from(binding)
                    .ok()
                    .and_then(|id| ch.software_objects.get(&id))
                    .map_or(binding, |(_, class)| *class);
                (class == 0x902d
                    && matches!(
                        method,
                        0x888 | 0x290 | 0x29c | 0x2ac | 0x804
                            | 0x840..=0x84c
                            | 0x850..=0x85c
                            | 0x8c0..=0x8cc
                    ))
                    || (matches!(class, 0xc597) || class == self.classes.compute)
                        && matches!(method, 0x104 | 0x108)
            };
            let mut i = 0;
            while i < words.len() {
                let header = words[i];
                i += 1;
                let sec_op = header >> 29;
                if sec_op == 0 && header == 0 {
                    continue;
                }
                if sec_op == 7 {
                    break;
                }
                let count = ((header >> 16) & 0x1fff) as usize;
                if sec_op == 4 {
                    let subch = ((header >> 13) & 7) as usize;
                    let method = (header & 0xfff) << 2;
                    match method {
                        0 => bindings[subch] = count as u32 & 0xffff,
                        8 => {}
                        _ if state_setter(bindings[subch], method) => {}
                        _ => {
                            return Err(diagnostic(
                                subch,
                                bindings[subch],
                                method,
                                count as u32,
                                "initial immediate method is not SET_OBJECT/NOP",
                            ))
                        }
                    }
                    continue;
                }
                if !matches!(sec_op, 1 | 3 | 5) || i + count > words.len() {
                    let subch = ((header >> 13) & 7) as usize;
                    return Err(diagnostic(
                        subch,
                        bindings[subch],
                        (header & 0xfff) << 2,
                        header,
                        "unsupported or truncated initial method header",
                    ));
                }
                let subch = ((header >> 13) & 7) as usize;
                let addr = (header & 0xfff) << 2;
                for k in 0..count {
                    let method = match sec_op {
                        1 => addr + 4 * k as u32,
                        3 => addr,
                        _ => addr + if k == 0 { 0 } else { 4 },
                    };
                    let data = words[i + k];
                    match method {
                        0 => bindings[subch] = data & 0xffff,
                        8 => {}
                        _ if state_setter(bindings[subch], method) => {}
                        _ => {
                            return Err(diagnostic(
                                subch,
                                bindings[subch],
                                method,
                                data,
                                "initial method is not SET_OBJECT/NOP",
                            ))
                        }
                    }
                }
                i += count;
            }
        }
        Ok(DeferredTakeover {
            chid,
            generation: ch.generation,
            gp_get,
            bindings,
        })
    }

    /// Apply a previously validated takeover after the host channel was retired.
    pub fn apply_deferred_takeover(&mut self, plan: DeferredTakeover) -> Result<(), String> {
        let ch = self
            .channels
            .get_mut(&plan.chid)
            .ok_or_else(|| format!("chid {}: no such channel", plan.chid))?;
        if ch.generation != plan.generation
            || !ch.mirrored
            || ch.gp_get != 0
            || ch.software_failed
            || ch.deferred.is_some()
        {
            return Err(format!(
                "chid {}: channel changed after takeover preview",
                plan.chid
            ));
        }
        ch.objects = plan.bindings;
        ch.mirrored = false;
        ch.gp_get = plan.gp_get;
        Ok(())
    }

    /// Retire a deferred target after its host FIFO is idle, its private USERD has been
    /// harvested, and its host objects were freed. The harvested GPGet becomes the exact
    /// software-side position so old host-executed entries are never replayed.
    pub fn retire_deferred_target(
        &mut self,
        chid: u32,
        generation: u64,
        gp_get: u32,
    ) -> Result<(), String> {
        let ch = self
            .channels
            .get_mut(&chid)
            .ok_or_else(|| format!("chid {chid}: deferred target is gone"))?;
        if ch.generation != generation {
            return Err(format!(
                "chid {chid}: deferred target generation changed from {generation} to {}",
                ch.generation
            ));
        }
        let entries = ch.gpfifo_entries.max(1);
        if gp_get >= entries {
            return Err(format!(
                "chid {chid}: harvested GPGet {gp_get} is outside {entries} entries"
            ));
        }
        if ch.resume.is_some()
            || ch.deferred.is_some()
            || ch.deferred_management
            || ch.software_failed
        {
            return Err(format!(
                "chid {chid}: deferred target has active software execution state"
            ));
        }
        ch.gp_get = gp_get;
        ch.gp_put_seen = gp_get;
        ch.mirrored = false;
        Ok(())
    }

    /// Validate and immediately apply a takeover. Runtime callers that tear down a host
    /// channel between the two steps use `preview_deferred_takeover` and
    /// `apply_deferred_takeover` directly.
    pub fn takeover_deferred_management(
        &mut self,
        chid: u32,
        gp_get: u32,
        mem: &mut GpuMem,
    ) -> Result<(), String> {
        let plan = self.preview_deferred_takeover(mem, chid, gp_get)?;
        self.apply_deferred_takeover(plan)
    }

    /// Run only this strict management channel. Unlike a normal guest doorbell this
    /// never retries unrelated blocked channels.
    pub fn run_deferred_one(&mut self, mem: &mut GpuMem, chid: u32) -> Vec<String> {
        self.doorbells = self.doorbells.wrapping_add(1);
        self.run(mem, chid)
    }

    pub fn pending_deferred(&self) -> Vec<(u32, u32, u32)> {
        self.channels
            .values()
            .filter_map(|ch| match ch.deferred {
                Some(DeferredState::Pending { object, api }) => Some((ch.chid, object, api)),
                _ => None,
            })
            .collect()
    }

    pub fn complete_deferred(
        &mut self,
        chid: u32,
        api: u32,
        result: Result<(), String>,
    ) -> Result<(), String> {
        let ch = self
            .channels
            .get_mut(&chid)
            .ok_or_else(|| format!("chid {chid}: no such channel"))?;
        let Some(DeferredState::Pending {
            object,
            api: pending,
        }) = ch.deferred
        else {
            return Err(format!("chid {chid}: no deferred API pending"));
        };
        if pending != api {
            return Err(format!(
                "chid {chid}: deferred API {pending:#x} pending, not {api:#x}"
            ));
        }
        match result {
            Ok(()) => ch.deferred = Some(DeferredState::Completed { object, api }),
            Err(error) => {
                let failure = format!(
                    "chid {chid}: deferred API {api:#x} on object {object:#x} failed: {error}"
                );
                ch.deferred = None;
                ch.software_failed = true;
                ch.software_failure = Some(failure);
                ch.software_nonstall = false;
            }
        }
        Ok(())
    }

    pub fn take_software_events(&mut self) -> Vec<SoftwareEvent> {
        self.channels
            .values_mut()
            .filter_map(|ch| {
                let failure = ch.software_failure.take();
                let nonstall = std::mem::take(&mut ch.software_nonstall);
                (failure.is_some() || nonstall).then_some(SoftwareEvent {
                    chid: ch.chid,
                    engine_type: ch.engine_type,
                    failure,
                    nonstall,
                })
            })
            .collect()
    }

    /// The GPGet the device last recorded for guest channel `chid` (its software or
    /// harvested position).
    pub fn channel_gp_get(&self, chid: u32) -> Option<u32> {
        self.channels.get(&chid).map(|c| c.gp_get)
    }

    /// The software channels whose blocked acquire ([`AcquireWait`]) is satisfied now, read
    /// through the same path as the acquire itself. The flag of [`acquire_waits_pending`]
    /// stays set while a channel still waits.
    pub fn ready_acquires(&self, mem: &GpuMem) -> Vec<u32> {
        let mut ready = Vec::new();
        let mut waiting = false;
        for ch in self.channels.values() {
            let Some(w) = ch.acquire_wait else { continue };
            if ch.mirrored || ch.software_failed || ch.resume.is_none() {
                continue;
            }
            let n = if w.wide { 8 } else { 4 };
            let mut b = [0u8; 8];
            let read = ce_pieces(mem, w.root, false, 0, w.va, n as u64, ch.software_fb_range)
                .and_then(|pieces| ce_read(mem, &pieces, &mut b[..n]));
            match read {
                Ok(()) if w.op.ready(u64::from_le_bytes(b), w.payload, w.wide) => ready.push(ch.chid),
                // Unreadable now: the channel's next run reports it.
                Err(_) => ready.push(ch.chid),
                Ok(()) => waiting = true,
            }
        }
        if waiting {
            ACQUIRE_WAITS.store(true, std::sync::atomic::Ordering::Release);
        }
        ready
    }

    /// Software acquires may wait for a host GPU write without another guest doorbell.
    pub fn blocked_software_channels(&self) -> Vec<u32> {
        self.channels
            .values()
            .filter(|ch| !ch.mirrored && !ch.software_failed && (ch.resume.is_some() || ch.budget_stop))
            .map(|ch| ch.chid)
            .collect()
    }

    pub fn vas_root(&self, key: (u32, u32)) -> Option<Target> {
        self.vaspaces.get(&self.resolve(key)).copied().flatten()
    }

    /// Entries of the root page directory of a VAS (4 for the 49-bit format when a
    /// control said so; 1 otherwise).
    pub fn root_entries(&self, key: (u32, u32)) -> u32 {
        self.root_entries
            .get(&self.resolve(key))
            .copied()
            .unwrap_or(1)
            .max(1)
    }

    pub fn channel(&self, chid: u32) -> Option<&Channel> {
        self.channels.get(&chid)
    }

    /// The guest's `FERMI_VASPACE_A` parameters and its `COPY_SERVER_RESERVED_PDES`
    /// parameters for a VAS (resolved through aliases).
    pub fn vas_replay(&self, key: (u32, u32)) -> (Option<&[u8]>, Option<&[u8]>) {
        let k = self.resolve(key);
        (
            self.vas_params.get(&k).map(Vec::as_slice),
            self.server_pdes.get(&k).map(Vec::as_slice),
        )
    }

    /// The channel's run state (`mirrored`, `stopped`, ...). Its identity (`h_client`,
    /// `h_object`, `h_parent`, `vas`) is a set-B input ([`Self::set_b_generation`]) that only
    /// the `record_*` calls change.
    pub fn channel_mut(&mut self, chid: u32) -> Option<&mut Channel> {
        self.channels.get_mut(&chid)
    }

    /// The channel and its USERD GPPut as the guest published it, when both are known. A
    /// USERD in VRAM is read through the FB store's watched copy (`FakeFb::watched_dword`,
    /// a map lookup after the first read); one in guest RAM is read directly.
    fn gp_put_of(&self, mem: &GpuMem, chid: u32) -> Option<(&Channel, u32)> {
        let ch = self.channels.get(&chid)?;
        let v = match ch.userd? {
            Target::Fb(pa) => mem.fb.watched_dword(pa + USERD_GP_PUT)?,
            Target::Sys(pa) => mem.read_u32(Target::Sys(pa + USERD_GP_PUT))?,
            t => mem.read_u32(t)?,
        };
        Some((ch, v))
    }

    /// Whether the guest has published GPFIFO entries the device has neither run nor
    /// forwarded: for a software channel GPGet is behind GPPut, for a mirrored one GPPut
    /// moved since its last doorbell ([`Self::note_rung`]). A channel the guest never
    /// rang has GPPut 0 on both sides and is not pending, so the token-less ioeventfd
    /// doorbell path (device.rs `doorbell_ioeventfd`) never creates a host mirror for
    /// it. One FB-store or guest-RAM read per call.
    pub fn pending(&self, mem: &GpuMem, chid: u32) -> bool {
        let Some((ch, gp_put)) = self.gp_put_of(mem, chid) else {
            return false;
        };
        if ch.mirrored {
            gp_put != ch.gp_put_seen
        } else {
            ch.gp_get != gp_put % ch.gpfifo_entries.max(1)
        }
    }

    /// Record the guest's GPPut at a doorbell of `chid`: every doorbell path calls this
    /// before it rings, and [`Self::pending`] compares against it.
    pub fn note_rung(&mut self, mem: &GpuMem, chid: u32) {
        // Only the token-less ioeventfd reader compares against `gp_put_seen` (`pending`),
        // and it excludes the token pool (main.rs): with the pool on, the fast ring path
        // (launch.rs `FAST_ENV`) spares the read, an uncached one for a USERD in the pool.
        if crate::launch::fast() && self.tokens.is_some() {
            return;
        }
        if let Some((_, gp_put)) = self.gp_put_of(mem, chid) {
            if let Some(ch) = self.channels.get_mut(&chid) {
                ch.gp_put_seen = gp_put;
            }
        }
    }

    /// `(hVASpace, engineType)` of a guest TSG.
    pub fn tsg(&self, key: (u32, u32)) -> Option<(u32, u32)> {
        self.tsgs.get(&key).copied()
    }

    /// The channel's VAS key as the registry resolves it (through DUP aliases), with
    /// the default-VAS fallback of the software executor.
    pub fn vas_key_of(&self, ch: &Channel) -> Option<(u32, u32)> {
        if let Some(k) = ch.vas {
            let r = self.resolve(k);
            if self.vaspaces.get(&r).copied().flatten().is_some() {
                return Some(r);
            }
        }
        // A channel naming no VAS runs in its device's default VAS. SET_DEFAULT_VASPACE
        // associates a private VAS object; the GPU_DEVICE path keeps an internal default
        // under the device key after its transient VAS handle is freed.
        if ch.vas.is_none() {
            if let Some(k) = self.device_default_key(ch.h_client, ch.h_parent) {
                return Some(k);
            }
            if let Some(k) = self.client_default_key(ch.h_client) {
                return Some(k);
            }
            if let Some(k) = self.shared_default_key(ch.h_client) {
                return Some(k);
            }
        }
        let mut roots = self
            .vaspaces
            .iter()
            .filter(|((c, _), r)| *c == ch.h_client && r.is_some());
        match (roots.next(), roots.next()) {
            (Some((k, _)), None) => Some(*k),
            _ => None,
        }
    }

    /// The live VAS key associated with one device, or the internal default kept under
    /// the device key after the kernel's transient GPU_DEVICE handle was freed.
    fn device_default_key(&self, h_client: u32, h_device: u32) -> Option<(u32, u32)> {
        let dev = (h_client, h_device);
        self.default_vas
            .get(&dev)
            .map(|k| self.resolve(*k))
            .or_else(|| self.vaspaces.contains_key(&dev).then_some(dev))
    }

    /// The one device default of a client. The `client_device` path is exact; the scans
    /// retain compatibility with older/internal flows that only announced a GPU_DEVICE VAS.
    fn client_default_key(&self, h_client: u32) -> Option<(u32, u32)> {
        if let Some(&dev) = self.client_device.get(&h_client) {
            if let Some(k) = self.device_default_key(h_client, dev) {
                return Some(k);
            }
        }
        let mut explicit = self
            .default_vas
            .iter()
            .filter(|((c, _), _)| *c == h_client)
            .map(|(_, k)| self.resolve(*k));
        match (explicit.next(), explicit.next()) {
            (Some(k), None) => return Some(k),
            (Some(_), Some(_)) => return None,
            _ => {}
        }
        let mut internal = self
            .device_vas
            .iter()
            .filter(|((c, _), _)| *c == h_client)
            .map(|(_, d)| (h_client, *d))
            .filter(|k| self.vaspaces.contains_key(k));
        match (internal.next(), internal.next()) {
            (Some(k), None) => Some(k),
            _ => None,
        }
    }

    /// The VAS a (possibly duplicated) handle stands for.
    pub fn resolve(&self, mut key: (u32, u32)) -> (u32, u32) {
        for _ in 0..4 {
            match self.aliases.get(&key) {
                Some(k) => key = *k,
                None => break,
            }
        }
        key
    }

    /// `DUP_OBJECT` (rpc_dup_object_v03_00): remember a duplicated VA space.
    /// Returns a note when the source is a VAS the registry knows.
    pub fn record_dup(&mut self, dst: (u32, u32), src: (u32, u32)) -> Option<String> {
        let src = self.resolve(src);
        if !self.vaspaces.contains_key(&src) {
            return None;
        }
        self.aliases.insert(dst, src);
        self.set_b_changed();
        Some(format!(
            "vaspace alias {:#x}/{:#x} -> {:#x}/{:#x} [guest]",
            dst.0, dst.1, src.0, src.1
        ))
    }

    /// The generation of what a shadow root's set B is computed from here (the channels,
    /// how their VA spaces resolve to a root, their promoted buffers): while it stands, so
    /// does set B (mirror.rs `SetBStamp`).
    pub fn set_b_generation(&self) -> u64 {
        self.set_b_gen
    }

    /// A set-B input changed ([`Self::set_b_gen`]).
    fn set_b_changed(&mut self) {
        self.set_b_gen = crate::ptcache::fresh_generation();
    }

    /// The sizes of the set-B inputs [`Self::record_free`] only removes from: a change of
    /// one of them is a removal.
    fn set_b_shape(&self) -> [usize; 8] {
        [
            self.channels.len(),
            self.vaspaces.len(),
            self.aliases.len(),
            self.promoted.len(),
            self.device_vas.len(),
            self.default_vas.len(),
            self.client_device.len(),
            self.client_share.len(),
        ]
    }

    /// The promoted context buffers of a channel or TSG (part 2 input).
    pub fn promoted(&self, key: (u32, u32)) -> Option<&BTreeMap<u16, PromoteEntry>> {
        self.promoted.get(&key)
    }

    /// `GPU_PROMOTE_CTX`: record the entries (the reply is the echo).
    pub fn record_promote_ctx(&mut self, params: &[u8]) -> String {
        let key = (
            rd32(params, v::PROMOTE_H_CHAN_CLIENT),
            rd32(params, v::PROMOTE_H_OBJECT),
        );
        let n = (rd32(params, v::PROMOTE_ENTRY_COUNT) as usize).min(v::PROMOTE_MAX_ENTRIES);
        self.set_b_changed();
        let map = self.promoted.entry(key).or_default();
        let mut desc = Vec::new();
        for i in 0..n {
            let o = v::PROMOTE_ENTRIES + v::PROMOTE_ENTRY_SIZE * i;
            if o + v::PROMOTE_ENTRY_SIZE > params.len() {
                break;
            }
            let (pa, va, size) = (rd64(params, o), rd64(params, o + 8), rd64(params, o + 16));
            let attr = rd32(params, o + 24);
            let id = u16::from_le_bytes([params[o + 28], params[o + 29]]);
            let (init, nonmapped) = (params[o + 30] != 0, params[o + 31] != 0);
            let e = map.entry(id).or_insert_with(|| PromoteEntry {
                buffer_id: id,
                ..Default::default()
            });
            if pa != 0 {
                e.phys = Some((pa, attr));
            }
            if va != 0 && !nonmapped {
                e.virt = Some(va);
            }
            e.size = e.size.max(size);
            e.initialize |= init;
            e.nonmapped |= nonmapped;
            desc.push(format!(
                "{id}: pa {pa:#x} attr {attr:#x} va {va:#x} size {size:#x}{}{}",
                if init { " init" } else { "" },
                if nonmapped { " nonmapped" } else { "" }
            ));
        }
        // A falcon engine (NVENC, NVDEC) on a VAS the kernel manages promotes its one
        // context buffer by VA alone: entryCount 0, virtAddress and size at the top level
        // (falcon/kernel_falcon.c:217-271). The shadow keeps that range out of the
        // guest's set A, as it does GR's set B (mirror.rs `gr_shadow_doorbell`).
        let top_va = rd64(params, v::PROMOTE_VIRT_ADDRESS);
        if n == 0 && top_va != 0 {
            let size = rd64(params, v::PROMOTE_SIZE);
            map.insert(
                PROMOTE_ID_FALCON_CTX,
                PromoteEntry {
                    buffer_id: PROMOTE_ID_FALCON_CTX,
                    virt: Some(top_va),
                    size,
                    ..Default::default()
                },
            );
            desc.push(format!("falcon context va {top_va:#x} size {size:#x}"));
        }
        format!(
            "promote ctx engineType {:#x} for {:#x}/{:#x}: {n} entries [{}] [guest, recorded]",
            rd32(params, v::PROMOTE_ENGINE_TYPE),
            key.0,
            key.1,
            desc.join("; ")
        )
    }

    /// The device clock's epoch (the usermode page publishes against it, um_page.rs).
    pub fn epoch(&self) -> Option<std::time::Instant> {
        self.start
    }

    /// Nanoseconds of the device clock (semaphore timestamps, usermode TIME_0/1).
    pub fn now_ns(&self) -> u64 {
        self.start.map_or(0, |s| s.elapsed().as_nanos() as u64)
    }

    /// Record an allocation of one of the classes this module cares about.
    /// Returns a note for the trace, or `None` for other classes.
    pub fn record_alloc(
        &mut self,
        h_client: u32,
        h_parent: u32,
        h_object: u32,
        h_class: u32,
        params: &[u8],
    ) -> Option<String> {
        match h_class {
            NV01_DEVICE_0_CLASS => {
                // A client handle can be reused after FREE. Every new device allocation
                // replaces its sharing relationship, including an unshared allocation.
                self.set_b_changed();
                if let Some(old) = self.client_device.insert(h_client, h_object) {
                    if old != h_object {
                        self.default_vas.remove(&(h_client, old));
                    }
                }
                self.client_share.remove(&h_client);
                let share = rd32(params, 4);
                if share != 0 && share != h_client {
                    self.client_share.insert(h_client, share);
                    return Some(format!(
                        "device {h_object:#x} shares the VA space of client {share:#x} [guest]"
                    ));
                }
                None
            }
            FERMI_VASPACE_A => {
                // NV_VASPACE_ALLOCATION_PARAMETERS.index @0; GPU_DEVICE 3 (nvos.h:3178).
                if rd32(params, 0) == VAS_INDEX_GPU_DEVICE {
                    self.device_vas.insert((h_client, h_object), h_parent);
                }
                self.vaspaces.insert((h_client, h_object), None);
                self.set_b_changed();
                self.vas_params
                    .insert((h_client, h_object), params.to_vec());
                Some(format!(
                    "vaspace {h_object:#x} recorded (flags {:#x}, vaSize {:#x}, internal {:#x}..{:#x}, bigPage {:#x}) [guest]",
                    rd32(params, 4),
                    rd64(params, 8),
                    rd64(params, 16),
                    rd64(params, 24),
                    rd32(params, 32)
                ))
            }
            FERMI_CONTEXT_SHARE_A => {
                let vas = rd32(params, CTXSHARE_HVASPACE);
                self.ctxshares.insert((h_client, h_object), vas);
                Some(format!("ctxshare {h_object:#x} vaspace {vas:#x} [guest]"))
            }
            KEPLER_CHANNEL_GROUP_A => {
                let vas = rd32(params, TSG_HVASPACE);
                let engine = rd32(params, TSG_ENGINE_TYPE);
                self.tsgs.insert((h_client, h_object), (vas, engine));
                // 615 appends internalFlags (28 bytes; 610 has 20 and no such field).
                let grp = crate::ver::layouts()
                    .tsg_internal_flags
                    .filter(|&off| params.len() >= off + 4)
                    .map(|off| rd32(params, off))
                    .filter(|f| f & (1 << 16) != 0)
                    .map(|f| f & 0xFFFF);
                if let Some(id) = grp {
                    self.tsg_ids.insert((h_client, h_object), id);
                }
                Some(format!(
                    "tsg {h_object:#x} engineType {engine:#x} vaspace {vas:#x}{} [guest]",
                    grp.map_or(String::new(), |id| format!(" grpID {id} (kernel-assigned)"))
                ))
            }
            c if c == self.classes.gpfifo => {
                self.channel_generation = self
                    .channel_generation
                    .checked_add(1)
                    .expect("channel generation exhausted");
                let flags = rd32(params, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_flags);
                // GSP clients pass their ChID as USERD_INDEX_VALUE (10:8) and
                // USERD_INDEX_PAGE_VALUE (20:12), 8 channels per page
                // (kernel_channel.c:2704-2718, alloc_channel.h:184-205).
                let chid = ((flags >> 12) & 0x1FF) * 8 + ((flags >> 8) & 7);
                let own_vas = rd32(params, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_hVASpace);
                let ctxshare = rd32(
                    params,
                    <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_hContextShare,
                );
                // The channel's own VAS, else its TSG's, else its context share's.
                let vas = if own_vas != 0 {
                    Some((h_client, own_vas))
                } else {
                    self.tsgs
                        .get(&(h_client, h_parent))
                        .map(|(v, _)| v)
                        .filter(|v| **v != 0)
                        .or_else(|| {
                            self.ctxshares
                                .get(&(h_client, ctxshare))
                                .filter(|v| **v != 0)
                        })
                        .map(|v| (h_client, *v))
                };
                let ch = Channel {
                    h_client,
                    h_object,
                    h_parent,
                    engine_objects: Vec::new(),
                    engine_object_handles: Vec::new(),
                    software_objects: BTreeMap::new(),
                    next_software_object: 1,
                    software_object_model_valid: true,
                    mirrored: false,
                    stopped: false,
                    chid,
                    generation: self.channel_generation,
                    engine_type: rd32(params, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_engineType),
                    alloc_flags: flags,
                    gpfifo_va: rd64(
                        params,
                        <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_gpFifoOffset,
                    ),
                    gpfifo_entries: rd32(
                        params,
                        <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_gpFifoEntries,
                    ),
                    userd: mem_desc(params, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_userdMem),
                    vas,
                    error_notifier: mem_desc(
                        params,
                        <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_errorNotifierMem,
                    ),
                    structures: [
                        (<Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_instanceMem, 0x1000),
                        (<Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_ramfcMem, 0x200),
                        (<Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_mthdbufMem, 0),
                    ]
                    .into_iter()
                    .filter_map(|(off, least)| {
                        let t = mem_desc(params, off)?;
                        // A guest's size is bounded (a method buffer is some pages).
                        let size = rd64(params, off + v::MEM_DESC_SIZE).clamp(least, 1 << 24);
                        (size > 0).then_some((t, size))
                    })
                    .collect(),
                    internal_tsg_id: Some(rd32(
                        params,
                        <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_internalFlags,
                    ))
                    .filter(|f| f & (1 << 24) != 0)
                    .map(|f| (f >> 8) & 0xFFFF),
                    gp_get: 0,
                    gp_put_seen: 0,
                    resume: None,
                    objects: [0; 8],
                    ce: CeState::default(),
                    compute: ComputeState::default(),
                    host: HostState::default(),
                    strict_reports: [StrictReportState::default(); 8],
                    entries_done: 0,
                    methods_done: 0,
                    launches_refused: 0,
                    tlb_invalidates: Vec::new(),
                    software_failed: false,
                    software_failure: None,
                    software_nonstall: false,
                    software_fb_range: None,
                    ce_slice: CE_SLICE,
                    budget_stop: false,
                    deferred_management: false,
                    deferred: None,
                    sem_diag_seen: std::collections::BTreeSet::new(),
                    acquire_wait: None,
                };
                let mut note = format!(
                    "channel {h_object:#x} chid {chid} engineType {:#x} gpfifo va {:#x} x {} userd {:?} vaspace {:?} errorNotifier {:?}{} alloc_flags {flags:#x} [guest]",
                    ch.engine_type,
                    ch.gpfifo_va,
                    ch.gpfifo_entries,
                    ch.userd,
                    ch.vas,
                    ch.error_notifier,
                    ch.internal_tsg_id
                        .map_or(String::new(), |id| format!(" internal TSG {id}"))
                );
                // A ChID the guest reuses without a FREE the device saw: the old
                // channel's pool token must not ring the new one.
                if let Some(t) = self.tokens.as_mut().and_then(|p| p.release(chid)) {
                    note.push_str(&format!(
                        "; doorbell token {t:#x} of the previous chid {chid} released"
                    ));
                }
                self.channels.insert(chid, ch);
                self.set_b_changed();
                Some(note)
            }
            // An engine object (copy, compute, the UVM SW class, ...) under a channel:
            // kept with its parameters for the host mirror.
            _ => {
                let classes = self.classes;
                self.channels
                    .values()
                    .find(|c| c.h_client == h_client && c.h_object == h_parent)?;
                // A duplicate live identity is invalid and should be rejected by the
                // dispatcher before mutation. Keep this registry internally coherent if
                // a caller nevertheless replaces one, including across channel parents.
                for old in self
                    .channels
                    .values_mut()
                    .filter(|c| c.h_client == h_client)
                {
                    if let Some(i) = old
                        .engine_object_handles
                        .iter()
                        .position(|h| *h == h_object)
                    {
                        let handle = old.engine_object_handles[i];
                        old.software_objects.retain(|_, (h, _)| *h != handle);
                        old.engine_object_handles.remove(i);
                        old.engine_objects.remove(i);
                    }
                }
                let ch = self
                    .channels
                    .values_mut()
                    .find(|c| c.h_client == h_client && c.h_object == h_parent)?;
                let known_hardware = matches!(h_class, 0xa140 | 0x902d | 0xc597)
                    || h_class == classes.copy
                    || h_class == classes.compute;
                if h_class == 0x5080 || h_class == classes.uvm_sw {
                    let id = ch.next_software_object;
                    if id != 0 {
                        ch.next_software_object = id.checked_add(1).unwrap_or(0);
                        ch.software_objects.insert(id, (h_object, h_class));
                    } else {
                        ch.software_object_model_valid = false;
                    }
                } else if !known_hardware {
                    ch.software_object_model_valid = false;
                }
                ch.engine_object_handles.push(h_object);
                ch.engine_objects.push((h_class, params.to_vec()));
                Some(format!(
                    "engine object class {h_class:#x} on chid {} ({} bytes of params) [guest]",
                    ch.chid,
                    params.len()
                ))
            }
        }
    }

    /// The TSG id the kernel assigned to TSG `key` (615), or the internal TSG of a bare
    /// channel `key`.
    pub fn tsg_id(&self, key: (u32, u32)) -> Option<u32> {
        self.tsg_ids.get(&key).copied().or_else(|| {
            self.channels
                .values()
                .find(|c| (c.h_client, c.h_object) == key)
                .and_then(|c| c.internal_tsg_id)
        })
    }

    /// The guest object a host TSG would stand for when a control names `(h_client,
    /// handle)`: a guest TSG itself, the TSG of a member channel, or a bare channel (its
    /// own TSG), as mirror.rs `tsg_key_of` names it. `None`: no such TSG or channel.
    pub fn tsg_guest_of(&self, h_client: u32, handle: u32) -> Option<(u32, u32)> {
        if self.tsgs.contains_key(&(h_client, handle)) {
            return Some((h_client, handle));
        }
        let ch = self
            .channels
            .values()
            .find(|c| (c.h_client, c.h_object) == (h_client, handle))?;
        Some(if self.tsgs.contains_key(&(ch.h_client, ch.h_parent)) {
            (ch.h_client, ch.h_parent)
        } else {
            (ch.h_client, ch.h_object)
        })
    }

    /// Record the guest's `GR_SET_CTXSW_PREEMPTION_MODE` of client `h_client` (preempt.rs)
    /// under the guest object its hChannel stands for. Returns that object (`None`: the
    /// handle names no TSG or channel of this device), the merged record and whether the
    /// payload shape is new.
    pub fn record_preemption(
        &mut self,
        h_client: u32,
        r: &crate::preempt::SetRequest,
    ) -> (Option<(u32, u32)>, Option<crate::preempt::Recorded>, bool) {
        let guest = self.tsg_guest_of(h_client, r.h_channel);
        let (rec, new_shape) = self.preempt.record(guest, r);
        (guest, rec, new_shape)
    }

    /// A SET too short to decode, for the exit summary.
    pub fn note_short_preemption(&mut self) {
        self.preempt.note_short();
    }

    /// What the guest asked for guest TSG (or bare channel) `guest`.
    pub fn preemption(&self, guest: (u32, u32)) -> Option<crate::preempt::Recorded> {
        self.preempt.get(guest)
    }

    /// The exit summary's line of the guest's SETs; `None` when there was none.
    pub fn preemption_summary(&self) -> Option<String> {
        self.preempt.summary()
    }

    /// Forget the last engine object of `class` recorded under channel `(h_client,
    /// h_parent)`: an allocation the device refused after recording it.
    pub fn forget_engine_object(&mut self, h_client: u32, h_parent: u32, class: u32) {
        if let Some(ch) = self
            .channels
            .values_mut()
            .find(|c| c.h_client == h_client && c.h_object == h_parent)
        {
            if let Some(i) = ch.engine_objects.iter().rposition(|(c, _)| *c == class) {
                let handle = ch.engine_object_handles[i];
                ch.software_objects.retain(|_, (h, _)| *h != handle);
                ch.engine_object_handles.remove(i);
                ch.engine_objects.remove(i);
            }
        }
    }

    /// Forget a freed object (a freed client takes its objects with it). Returns the trace
    /// notes of VA spaces kept under a surviving DUP alias (`rehome_duplicated_vas`).
    pub fn record_free(&mut self, h_client: u32, h_object: u32) -> Vec<String> {
        let shape = self.set_b_shape();
        let mut notes = self.rehome_duplicated_vas(h_client, h_object);
        let all = h_client == h_object;
        let device_gone = all || self.client_device.get(&h_client) == Some(&h_object);
        if !all {
            notes.extend(self.retain_device_default(h_client, h_object));
        }
        let parent_gone = |c: &Channel| {
            c.h_client == h_client && (all || c.h_object == h_object || c.h_parent == h_object)
        };
        // A freed channel gives its doorbell token back (docs/caraxes/doorbell-token-pool.md).
        if let Some(pool) = self.tokens.as_mut() {
            for c in self.channels.values().filter(|c| parent_gone(c)) {
                pool.release(c.chid);
            }
        }
        // Freeing a TSG owns and tears down its child channels. An engine-object FREE
        // removes only that exact client-scoped handle from its owning live channel.
        for c in self
            .channels
            .values_mut()
            .filter(|c| c.h_client == h_client)
        {
            if let Some(i) = c.engine_object_handles.iter().position(|h| *h == h_object) {
                c.software_objects.retain(|_, (h, _)| *h != h_object);
                c.engine_object_handles.remove(i);
                c.engine_objects.remove(i);
            }
        }
        self.channels.retain(|_, c| !parent_gone(c));
        self.tsgs
            .retain(|(c, h), _| *c != h_client || !(all || *h == h_object));
        self.tsg_ids
            .retain(|(c, h), _| *c != h_client || !(all || *h == h_object));
        // A preemption request lives as long as its guest TSG or bare channel.
        if !self.preempt.is_empty() {
            let (tsgs, channels) = (&self.tsgs, &self.channels);
            self.preempt.retain(|k| {
                tsgs.contains_key(k)
                    || channels.values().any(|c| (c.h_client, c.h_object) == *k)
            });
        }
        self.vaspaces
            .retain(|(c, h), _| *c != h_client || !(all || *h == h_object));
        let live = &self.vaspaces;
        self.pdb_watch.retain(|k| live.contains_key(k));
        self.root_entries
            .retain(|(c, h), _| *c != h_client || !(all || *h == h_object));
        self.vas_params
            .retain(|(c, h), _| *c != h_client || !(all || *h == h_object));
        self.server_pdes
            .retain(|(c, h), _| *c != h_client || !(all || *h == h_object));
        self.ctxshares
            .retain(|(c, h), _| *c != h_client || !(all || *h == h_object));
        self.aliases
            .retain(|(c, h), _| *c != h_client || !(all || *h == h_object));
        self.promoted
            .retain(|(c, h), _| *c != h_client || !(all || *h == h_object));
        self.device_vas
            .retain(|(c, h), d| *c != h_client || !(all || *h == h_object || *d == h_object));
        self.default_vas.retain(|(c, d), v| {
            !(*c == h_client && (all || *d == h_object || (v.0 == h_client && v.1 == h_object)))
        });
        if device_gone {
            self.client_device.remove(&h_client);
            self.client_share.remove(&h_client);
        }
        if all {
            // A sharing client cannot retain a reference to a client that no longer exists.
            self.client_share
                .retain(|c, share| *c != h_client && *share != h_client);
        }
        // Past the re-homes above (which note their changes), a free only removes: a set-B
        // input changed exactly when one of them shrank.
        if self.set_b_shape() != shape {
            self.set_b_changed();
        }
        notes
    }

    /// SET_DEFAULT takes a reference to the VAS object. Freeing its public handle therefore
    /// leaves the device's default alive; keep that otherwise-unnameable object under the
    /// device key, as the internal GPU_DEVICE flow already does. Channels continue resolving
    /// the device state rather than retaining a snapshot of its root.
    fn retain_device_default(&mut self, h_client: u32, h_vas: u32) -> Vec<String> {
        let old = (h_client, h_vas);
        let devices: Vec<(u32, u32)> = self
            .default_vas
            .iter()
            .filter_map(|(dev, vas)| (*vas == old).then_some(*dev))
            .collect();
        let Some(root) = self.vaspaces.get(&old).copied() else {
            return Vec::new();
        };
        let entries = self.root_entries.get(&old).copied();
        let alloc = self.vas_params.get(&old).cloned();
        let pdes = self.server_pdes.get(&old).cloned();
        let mut notes = Vec::new();
        for dev in devices {
            self.vaspaces.insert(dev, root);
            self.set_b_changed();
            self.pdb_watch.copy(old, dev);
            if let Some(entries) = entries {
                self.root_entries.insert(dev, entries);
            }
            if let Some(ref alloc) = alloc {
                self.vas_params.insert(dev, alloc.clone());
            }
            if let Some(ref pdes) = pdes {
                self.server_pdes.insert(dev, pdes.clone());
            }
            self.default_vas.remove(&dev);
            notes.push(format!(
                "default vaspace {h_client:#x}/{h_vas:#x} handle freed: retained by device {:#x}/{:#x}, root {root:?} [guest]",
                dev.0, dev.1
            ));
        }
        notes
    }

    /// A VA space freed by this FREE (its handle, or its owner client) while a DUP alias of
    /// another client still names it: the guest RM keeps the object alive through the
    /// duplicate, so the registry does too. Its root, root entries, alloc parameters and
    /// server-reserved PDEs move to the first surviving alias, and the other surviving
    /// aliases are repointed to it. Before (vm4-615, 2026-09-26 04:37-04:48, five bursts),
    /// a Vulkan client's VAS was allocated by one client and dup'ed into the client of its
    /// GR channels; the guest freed the owner first, the channels resolved to no root, their
    /// set-B exclusions and held ranges vanished while the host TSGs stood, and the next
    /// shadow re-sync mapped over the host's GR context (31 refused fixed VA allocations per
    /// teardown; docs/research/va-alloc-bursts-2026-09-26.md).
    fn rehome_duplicated_vas(&mut self, h_client: u32, h_object: u32) -> Vec<String> {
        let all = h_client == h_object;
        let freed = |k: &(u32, u32)| k.0 == h_client && (all || k.1 == h_object);
        let owners: Vec<(u32, u32)> = self.vaspaces.keys().copied().filter(freed).collect();
        let mut notes = Vec::new();
        for old in owners {
            // `record_dup` stores the resolved source, so an alias names its VAS directly.
            let heirs: Vec<(u32, u32)> = self
                .aliases
                .iter()
                .filter(|(d, s)| **s == old && !freed(d))
                .map(|(d, _)| *d)
                .collect();
            let Some(&heir) = heirs.first() else {
                continue;
            };
            self.set_b_changed();
            self.aliases.remove(&heir);
            for d in &heirs[1..] {
                self.aliases.insert(*d, heir);
            }
            // The device holds an object reference, not the handle spelling used when
            // SET_DEFAULT ran. Follow a surviving DUP alias when it becomes the owner.
            for vas in self.default_vas.values_mut() {
                if *vas == old {
                    *vas = heir;
                }
            }
            if let Some(r) = self.vaspaces.remove(&old) {
                self.vaspaces.insert(heir, r);
            }
            self.pdb_watch.rekey(old, heir);
            if let Some(e) = self.root_entries.remove(&old) {
                self.root_entries.insert(heir, e);
            }
            if let Some(p) = self.vas_params.remove(&old) {
                self.vas_params.insert(heir, p);
            }
            if let Some(p) = self.server_pdes.remove(&old) {
                self.server_pdes.insert(heir, p);
            }
            notes.push(format!(
                "vaspace {:#x}/{:#x} freed while {} DUP alias(es) of another client name it: kept under {:#x}/{:#x}, root {:?} [guest]",
                old.0,
                old.1,
                heirs.len(),
                heir.0,
                heir.1,
                self.vaspaces.get(&heir).copied().flatten()
            ));
        }
        notes
    }

    /// The ChID of guest channel `(h_client, h_channel)`, if recorded.
    pub fn chid_of(&self, h_client: u32, h_channel: u32) -> Option<u32> {
        self.channels
            .values()
            .find(|c| c.h_client == h_client && c.h_object == h_channel)
            .map(|c| c.chid)
    }

    /// `NV90F1_CTRL_CMD_VASPACE_COPY_SERVER_RESERVED_PDES` on a VAS: `levels[0]` is
    /// the root page directory (ctrl90f1.h:272-330; gpu_vaspace.c:5026-5031).
    pub fn record_server_reserved_pdes(
        &mut self,
        h_client: u32,
        h_vas: u32,
        params: &[u8],
    ) -> String {
        // numLevelsToCopy levels of {physAddress, size, aperture, pageShift}.
        let n = rd32(params, v::PDES_NUM_LEVELS).min(v::PDES_MAX_LEVELS as u32) as usize;
        let level = |i: usize| v::PDES_LEVELS + v::PDES_LEVEL_SIZE * i;
        let levels: Vec<String> = (0..n)
            .map(|i| {
                let o = level(i);
                format!(
                    "{:#x}+{:#x} ap {} shift {}",
                    rd64(params, o + v::PDES_LEVEL_PHYS_ADDRESS),
                    rd64(params, o + v::PDES_LEVEL_SIZE_FIELD),
                    rd32(params, o + v::PDES_LEVEL_APERTURE),
                    params
                        .get(o + v::PDES_LEVEL_PAGE_SHIFT)
                        .copied()
                        .unwrap_or(0)
                )
            })
            .collect();
        let pa = rd64(params, level(0) + v::PDES_LEVEL_PHYS_ADDRESS);
        let aperture = rd32(params, level(0) + v::PDES_LEVEL_APERTURE);
        let va_lo = rd64(params, v::PDES_VIRT_ADDR_LO);
        let va_hi = rd64(params, v::PDES_VIRT_ADDR_HI);
        // `_gvaspacePopulatePDEentries` zeroes its output first. When there is no
        // server-owned range it reports the exact sentinel seen from Windows: PA 0 and
        // the empty inclusive interval 1..0. It describes no replacement page directory;
        // keep both the last usable root and the last real server-PDE description.
        if pa == 0 && va_lo > va_hi {
            return format!(
                "vaspace {h_vas:#x}: empty server-reserved range {va_lo:#x}..{va_hi:#x}; root {:?} and prior PDE levels kept [guest]",
                self.vas_root((h_client, h_vas))
            );
        }
        // A call whose first level is a lower directory describes a subtree, not a new root.
        // The Windows KMD sends this control a second time with one PD1-level table (page
        // shift 29) and the empty range 1..0 (run P17); taking it as the root makes the shadow
        // walk a PD1 as the PD3, and the GPFIFOs are never mapped. A stated shift below the
        // root level, or below the recorded root's, keeps the root; a zero shift is unstated.
        let shift0 = params
            .get(level(0) + v::PDES_LEVEL_PAGE_SHIFT)
            .copied()
            .unwrap_or(0);
        let prior_shift = self
            .server_pdes
            .get(&(h_client, h_vas))
            .and_then(|p| p.get(level(0) + v::PDES_LEVEL_PAGE_SHIFT).copied())
            .unwrap_or(0);
        if shift0 != 0 && (shift0 < ROOT_PAGE_SHIFT_V2 || shift0 < prior_shift) {
            return format!(
                "vaspace {h_vas:#x}: level 0 at {pa:#x} is a page-shift-{shift0} directory, not a root (shift {}); root {:?} and prior PDE levels kept; va {va_lo:#x}..{va_hi:#x} levels [{}] [guest]",
                ROOT_PAGE_SHIFT_V2.max(prior_shift),
                self.vas_root((h_client, h_vas)),
                levels.join(", ")
            );
        }
        let root = gmmu_aperture(aperture, pa);
        self.put_root((h_client, h_vas), root);
        let entries = (rd64(params, level(0) + v::PDES_LEVEL_SIZE_FIELD) / 8) as u32;
        self.root_entries.insert((h_client, h_vas), entries.max(1));
        self.server_pdes.insert((h_client, h_vas), params.to_vec());
        // The device's default VAS, announced through a transient handle: kept under
        // (hClient, hDevice) after that handle's FREE, for the channels that name no VAS.
        let device_note = match self.device_vas.get(&(h_client, h_vas)).copied() {
            Some(dev) => {
                self.put_root((h_client, dev), root);
                self.root_entries.insert((h_client, dev), entries.max(1));
                self.server_pdes.insert((h_client, dev), params.to_vec());
                format!(" (the default VAS of device {dev:#x})")
            }
            None => String::new(),
        };
        format!(
            "vaspace {h_vas:#x}{device_note}: root {root:?}; va {:#x}..{:#x} levels [{}] [guest]",
            va_lo,
            va_hi,
            levels.join(", ")
        )
    }

    /// `NV0080_CTRL_DMA_SET_DEFAULT_VASPACE` associates an existing private VAS object
    /// with its parent device. The dispatcher validates the two object classes first.
    pub fn record_set_default_vaspace(
        &mut self,
        h_client: u32,
        h_device: u32,
        h_vas: u32,
    ) -> Result<String, String> {
        let dev = (h_client, h_device);
        if self.default_vas.contains_key(&dev) || self.vaspaces.contains_key(&dev) {
            return Err(format!("device {h_device:#x} already has a default VAS"));
        }
        let vas = self.resolve((h_client, h_vas));
        if !self.vaspaces.contains_key(&vas) {
            return Err(format!(
                "vaspace {h_vas:#x} is not recorded for client {h_client:#x}"
            ));
        }
        self.default_vas.insert(dev, vas);
        self.set_b_changed();
        Ok(format!(
            "device {h_device:#x} default vaspace {h_vas:#x} (live key {:#x}/{:#x}) [guest]",
            vas.0, vas.1
        ))
    }

    /// `NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY`: the VAS moves to the root the
    /// client supplies (ctrl0080dma.h:798-818).
    pub fn record_set_page_directory(&mut self, h_client: u32, params: &[u8]) -> String {
        let pa = rd64(params, v::SET_PD_PHYS_ADDRESS);
        let flags = rd32(params, v::SET_PD_FLAGS);
        let h_vas = rd32(params, v::SET_PD_H_VASPACE);
        // FLAGS_APERTURE 1:0: 0 vidmem, 1 sysmem coherent, 2 sysmem non-coherent.
        let root = match flags & 3 {
            0 => Some(Target::Fb(pa)),
            1 | 2 => Some(Target::Sys(pa)),
            _ => None,
        };
        let key = self.resolve((h_client, h_vas));
        self.put_root(key, root);
        self.root_entries
            .insert(key, rd32(params, v::SET_PD_NUM_ENTRIES).max(1));
        format!(
            "set page directory: vaspace {h_vas:#x}{} root {root:?} entries {} flags {flags:#x} [guest]",
            if key != (h_client, h_vas) {
                format!(" (alias of {:#x}/{:#x})", key.0, key.1)
            } else {
                String::new()
            },
            rd32(params, v::SET_PD_NUM_ENTRIES)
        )
    }

    /// `NV0080_CTRL_CMD_DMA_UNSET_PAGE_DIRECTORY` {hVASpace @0, subDeviceId @4}:
    /// the VAS root is unknown again until the next control names it.
    pub fn record_unset_page_directory(&mut self, h_client: u32, params: &[u8]) -> String {
        let h_vas = rd32(params, v::UNSET_PD_H_VASPACE);
        let key = self.resolve((h_client, h_vas));
        self.vaspaces.insert(key, None);
        self.set_b_changed();
        format!("unset page directory: vaspace {h_vas:#x} [guest]")
    }

    fn root_of(&self, ch: &Channel) -> Option<Target> {
        self.vas_key_of(ch).and_then(|k| self.vas_root(k))
    }

    /// Record `root` for VAS key `key`; the watch of the guest's invalidates follows.
    fn put_root(&mut self, key: (u32, u32), root: Option<Target>) {
        self.vaspaces.insert(key, root);
        self.set_b_changed();
        self.pdb_watch.set_root(key, root);
    }

    /// A guest TLB invalidate by register (boot.rs `mmu_invalidate_write`) of `pdb`
    /// (`None`: `ALL_PDB`); the `MEM_OP` ones arrive through the software executor.
    pub fn note_guest_invalidate(&mut self, pdb: Option<Target>) {
        self.pdb_watch.note_invalidate(pdb);
    }

    /// Fix 2 of run P17 (pdbwatch.rs): channel `chid`'s VAS key and mismatch when its VAS is
    /// walked from a root the guest's TLB invalidates no longer name.
    pub fn root_mismatch(&self, chid: u32) -> Option<((u32, u32), crate::pdbwatch::Mismatch)> {
        let ch = self.channels.get(&chid)?;
        let vas = self.vas_key_of(ch)?;
        let walked = self.vas_root(vas)?;
        let m = self.pdb_watch.check(self.resolve(vas), walked)?;
        Some((vas, m))
    }

    pub fn pdb_watch_mut(&mut self) -> &mut crate::pdbwatch::PdbWatch {
        &mut self.pdb_watch
    }

    pub fn pdb_watch(&self) -> &crate::pdbwatch::PdbWatch {
        &self.pdb_watch
    }

    /// The default VAS of the client a client's device shares (`client_share`): the key kept
    /// under the share client's device (announced through a GPU_DEVICE vaspace), else that
    /// client's only VAS with a root.
    fn shared_default_key(&self, h_client: u32) -> Option<(u32, u32)> {
        let share = *self.client_share.get(&h_client)?;
        // Windows uses the same device handle in the main and per-channel clients.
        if let Some(&device) = self.client_device.get(&h_client) {
            if let Some(k) = self.device_default_key(share, device) {
                return Some(k);
            }
        }
        if let Some(k) = self.client_default_key(share) {
            return Some(k);
        }
        let mut roots = self
            .vaspaces
            .iter()
            .filter(|((c, _), r)| *c == share && r.is_some());
        match (roots.next(), roots.next()) {
            (Some((k, _)), None) => Some(*k),
            _ => None,
        }
    }

    /// A doorbell write: run the channel the token names, then retry channels
    /// that were blocked on a semaphore acquire.
    pub fn doorbell(&mut self, mem: &mut GpuMem, token: u32) -> Vec<String> {
        self.doorbell_except(mem, token, &std::collections::BTreeSet::new())
    }

    /// [`Self::doorbell`], but a blocked channel in `off` (the guest disabled its
    /// scheduling, rc.rs) is not retried.
    pub fn doorbell_except(
        &mut self,
        mem: &mut GpuMem,
        token: u32,
        off: &std::collections::BTreeSet<u32>,
    ) -> Vec<String> {
        self.doorbells += 1;
        // NV_CTRL_VF_DOORBELL_VECTOR 11:0 is the ChID, RUNLIST_ID 22:16
        // (tu102/dev_ctrl.h:36-37; kernel_fifo_tu102.c:122-124).
        let chid = token & 0xFFF;
        let mut notes = vec![format!(
            "doorbell {}: token {token:#x} chid {chid} runlist {}",
            self.doorbells,
            (token >> 16) & 0x7F
        )];
        if !self.channels.contains_key(&chid) {
            notes.push(format!("chid {chid}: no such channel recorded"));
            return notes;
        }
        let ran = self.run(mem, chid);
        let diagnosed = self.sem_diag && ran.iter().any(|n| n.starts_with("SEMAPHORE DIAG"));
        notes.extend(ran);
        if diagnosed {
            notes.push(self.semaphore_diag_peers(mem, chid, off));
        }
        let blocked: Vec<u32> = self
            .channels
            .iter()
            .filter(|(c, ch)| **c != chid && ch.resume.is_some() && !off.contains(c))
            .map(|(c, _)| *c)
            .collect();
        for c in blocked {
            let ran = self.run(mem, c);
            let diagnosed = self.sem_diag && ran.iter().any(|n| n.starts_with("SEMAPHORE DIAG"));
            notes.extend(ran);
            if diagnosed {
                notes.push(self.semaphore_diag_peers(mem, c, off));
            }
        }
        notes
    }

    /// [`Self::doorbell_except`] for a dispatcher with shadow VA spaces
    /// (docs/research/shadow-fault-race-2026-09-28.md). A `MEM_OP` TLB invalidate that names
    /// one of the `hold` roots (or every root) stops its channel right after the method;
    /// `on_invalidates` then gets every invalidate run so far (the dispatcher plans the
    /// shadows it names and waits until the worker applied them), and only then do the
    /// stopped channels continue. So nothing after such an invalidate in a push, above all
    /// the semaphore release nvidia-uvm waits for before it reports a mapping done, reaches
    /// the guest before the host mappings exist. Without `hold` this is `doorbell_except`
    /// followed by one `on_invalidates` call, as before.
    pub fn doorbell_holding(
        &mut self,
        mem: &mut GpuMem,
        token: u32,
        off: &std::collections::BTreeSet<u32>,
        hold: Option<std::collections::BTreeSet<u64>>,
        on_invalidates: &mut OnInvalidates<'_>,
    ) -> Vec<String> {
        self.memop_hold = hold.filter(|h| !h.is_empty());
        self.held_at_invalidate.clear();
        let mut notes = self.doorbell_except(mem, token, off);
        let mut rounds = 0usize;
        loop {
            let inv = std::mem::take(&mut self.tlb_invalidates);
            if !inv.is_empty() {
                let _spent = crate::dlock::Spend::new(crate::dlock::Part::Invalidate);
                notes.extend(on_invalidates(self, mem, &inv));
            }
            // The invalidates of every stopped channel were handed over above: continuing
            // them is safe from here on, also later through the held-doorbell replay
            // (their resume point is kept) when the bound below ends this loop.
            if self.held_at_invalidate.is_empty() || rounds >= MAX_INVALIDATE_ROUNDS {
                break;
            }
            rounds += 1;
            for c in std::mem::take(&mut self.held_at_invalidate) {
                if !off.contains(&c) {
                    notes.extend(self.run(mem, c));
                }
            }
        }
        self.memop_hold = None;
        self.held_at_invalidate.clear();
        notes
    }

    fn semaphore_diag_peers(
        &self,
        mem: &GpuMem,
        chid: u32,
        off: &std::collections::BTreeSet<u32>,
    ) -> String {
        let peer_count = self.channels.len().saturating_sub(1);
        let mut peers = self
            .channels
            .iter()
            .filter(|(peer, _)| **peer != chid)
            .take(SEM_DIAG_PEERS)
            .map(|(&peer, p)| {
                let put = self.gp_put_of(mem, peer).map(|(_, put)| put);
                format!(
                    "chid {peer}: engine {:#x}, mirrored {}, disabled {}, gp {}/{put:?}, resume {:?}",
                    p.engine_type,
                    p.mirrored,
                    off.contains(&peer),
                    p.gp_get,
                    p.resume
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        if peer_count > SEM_DIAG_PEERS {
            peers.push_str(&format!(
                "; ... {} peer(s) truncated",
                peer_count - SEM_DIAG_PEERS
            ));
        }
        format!("SEMAPHORE DIAG peers: {peers}")
    }

    /// Consume the channel's GPFIFO from GPGet up to GPPut.
    fn run(&mut self, mem: &mut GpuMem, chid: u32) -> Vec<String> {
        let mut notes = Vec::new();
        let now = self.now_ns();
        let classes = self.classes;
        let Some(root) = self.channels.get(&chid).and_then(|c| self.root_of(c)) else {
            notes.push(format!("chid {chid}: VAS root unknown, nothing run"));
            return notes;
        };
        let ch = self.channels.get_mut(&chid).expect("checked");
        if ch.mirrored || ch.software_failed {
            return notes;
        }
        self.software_runs = self.software_runs.wrapping_add(1);
        ch.software_fb_range = self.software_fb_range;
        ch.ce_slice = self.ce_slice;
        ch.budget_stop = false;
        ch.acquire_wait = None;
        let started = std::time::Instant::now();
        let budget = self.run_budget;
        let Some(userd) = ch.userd else {
            notes.push(format!("chid {chid}: no USERD, nothing run"));
            return notes;
        };
        let userd_at = |off: u64| match userd {
            Target::Fb(pa) => Target::Fb(pa + off),
            Target::Sys(pa) => Target::Sys(pa + off),
            t => t,
        };
        let Some(gp_put) = mem.read_u32(userd_at(USERD_GP_PUT)) else {
            notes.push(format!("chid {chid}: USERD {userd:?} unreadable"));
            return notes;
        };
        let entries = ch.gpfifo_entries.max(1);
        let mut guard = 0u32;
        let mut held = false;
        while ch.gp_get != gp_put % entries && guard < entries {
            guard += 1;
            let mut e = [0u8; 8];
            let entry_va = ch.gpfifo_va + 8 * ch.gp_get as u64;
            let fetched = {
                let _spent = crate::dlock::Spend::new(crate::dlock::Part::Fetch);
                mem.read_va(root, entry_va, &mut e)
            };
            if let Err(err) = fetched {
                notes.push(format!("chid {chid}: gpfifo entry {}: {err}", ch.gp_get));
                break;
            }
            let (e0, e1) = (rd32(&e, 0), rd32(&e, 4));
            // GP_ENTRY0 GET 31:2, GP_ENTRY1 GET_HI 7:0, LENGTH 30:10 (dwords), OPCODE 7:0
            // when LENGTH is 0 (clc46f.h:264-282).
            let length = (e1 >> 10) & 0x1F_FFFF;
            if length == 0 {
                if ch.deferred_management {
                    let failure = format!(
                        "strict management chid {chid} rejects control entry opcode {}",
                        e1 & 0xff
                    );
                    ch.software_failed = true;
                    ch.software_failure = Some(failure.clone());
                    notes.push(failure);
                    break;
                }
                notes.push(format!(
                    "chid {chid} gp {}: control entry opcode {} (no pushbuffer)",
                    ch.gp_get,
                    e1 & 0xFF
                ));
            } else {
                if ch.deferred_management && length > 4096 {
                    let failure =
                        format!("strict management chid {chid} rejects {length}-dword pushbuffer");
                    ch.software_failed = true;
                    ch.software_failure = Some(failure.clone());
                    notes.push(failure);
                    break;
                }
                let pb_va = ((e1 as u64 & 0xFF) << 32) | (e0 as u64 & !3);
                let diagnose = self.sem_diag && self.sem_diag_chid.is_none_or(|c| c == chid);
                match Self::segment(
                    ch,
                    mem,
                    root,
                    pb_va,
                    length as usize,
                    now,
                    &classes,
                    diagnose,
                    self.memop_hold.as_ref(),
                ) {
                    Ok((seg_notes, stop)) => {
                        notes.push(format!(
                            "chid {chid} gp {}: pushbuffer va {pb_va:#x} {} dwords{}",
                            ch.gp_get,
                            length,
                            match stop {
                                Stop::End => "",
                                Stop::Blocked => ", blocked on acquire",
                                Stop::HeldAtInvalidate => {
                                    ", stopped after a TLB invalidate of a shadow root until the shadow is applied"
                                }
                            }
                        ));
                        notes.extend(seg_notes);
                        match stop {
                            Stop::End => {}
                            Stop::Blocked => break,
                            Stop::HeldAtInvalidate => {
                                held = true;
                                break;
                            }
                        }
                    }
                    Err(err) => {
                        notes.push(format!("chid {chid} gp {}: {err}", ch.gp_get));
                        if ch.deferred_management {
                            ch.software_failed = true;
                            ch.software_failure = Some(err);
                            ch.software_nonstall = false;
                        }
                        break;
                    }
                }
            }
            ch.gp_get = (ch.gp_get + 1) % entries;
            ch.entries_done += 1;
            let stored = {
                let _spent = crate::dlock::Spend::new(crate::dlock::Part::GpGet);
                mem.write(userd_at(USERD_GP_GET), &ch.gp_get.to_le_bytes())
            };
            if !stored {
                notes.push(format!("chid {chid}: GPGet write to {userd:?} failed"));
            }
            // The run's budget: stop between two entries, the slice clock continues.
            if ch.gp_get != gp_put % entries && started.elapsed() >= budget {
                ch.budget_stop = true;
                CE_SLICES_PENDING.store(true, std::sync::atomic::Ordering::Release);
                notes.push(format!(
                    "chid {chid}: run stopped at gp {} after {} us ({} us budget); the slice clock continues it",
                    ch.gp_get,
                    started.elapsed().as_micros(),
                    budget.as_micros()
                ));
                break;
            }
        }
        let inv = std::mem::take(&mut ch.tlb_invalidates);
        for pdb in &inv {
            self.pdb_watch.note_invalidate(*pdb);
        }
        self.tlb_invalidates.extend(inv);
        if held {
            self.held_at_invalidate.push(chid);
        }
        notes
    }

    /// Execute one pushbuffer segment. Returns the notes and where it stopped: at its end,
    /// at a host semaphore acquire that blocked (the entry is then resumed at that method),
    /// or right after a `MEM_OP` TLB invalidate of a `hold` root (resumed after it,
    /// [`Self::doorbell_holding`]).
    #[allow(clippy::too_many_arguments)]
    fn segment(
        ch: &mut Channel,
        mem: &mut GpuMem,
        root: Target,
        va: u64,
        dwords: usize,
        now: u64,
        classes: &ChannelClasses,
        diagnose: bool,
        hold: Option<&std::collections::BTreeSet<u64>>,
    ) -> Result<(Vec<String>, Stop), String> {
        let mut raw = vec![0u8; dwords * 4];
        {
            let _spent = crate::dlock::Spend::new(crate::dlock::Part::Fetch);
            mem.read_va(root, va, &mut raw)
        }
        .map_err(|e| format!("pushbuffer: {e}"))?;
        let w: Vec<u32> = raw
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        let skip_to = ch.resume.take().unwrap_or(0);
        let mut notes = Vec::new();
        let mut i = 0usize;
        while i < w.len() {
            let hdr = w[i];
            i += 1;
            // NVC46F_DMA_*: SEC_OP 31:29, COUNT/IMMD 28:16, SUBCHANNEL 15:13, ADDRESS 11:0
            // in dwords (clc46f.h:285-358).
            let sec_op = hdr >> 29;
            let count = ((hdr >> 16) & 0x1FFF) as usize;
            let subch = ((hdr >> 13) & 7) as usize;
            let addr = (hdr & 0xFFF) << 2;
            let methods: Vec<(usize, u32, u32)> = match sec_op {
                // INC_METHOD, NON_INC_METHOD, ONE_INC
                1 | 3 | 5 => {
                    if i + count > w.len() {
                        if ch.deferred_management {
                            return Err(format!(
                                "strict management method header {hdr:#010x} at dword {} runs past the segment",
                                i - 1
                            ));
                        }
                        notes.push(format!(
                            "method header {hdr:#010x} at dword {} runs past the segment",
                            i - 1
                        ));
                    }
                    let n = count.min(w.len() - i);
                    let m = (0..n)
                        .map(|k| {
                            let a = match sec_op {
                                1 => addr + 4 * k as u32,
                                3 => addr,
                                _ => addr + if k == 0 { 0 } else { 4 },
                            };
                            (i + k, a, w[i + k])
                        })
                        .collect();
                    i += n;
                    m
                }
                // IMMD_DATA_METHOD: the data is the count field.
                4 => vec![(i - 1, addr, count as u32)],
                // GRP0_USE_TERT: NOP (all zero) and the subdevice-mask opcodes.
                0 => Vec::new(),
                // END_PB_SEGMENT
                7 => break,
                _ => {
                    if ch.deferred_management {
                        return Err(format!(
                            "strict management rejects unknown method header {hdr:#010x}"
                        ));
                    }
                    notes.push(format!(
                        "unknown method header {hdr:#010x}, segment dropped"
                    ));
                    break;
                }
            };
            for (pos, a, data) in methods {
                if pos < skip_to {
                    continue;
                }
                ch.methods_done += 1;
                let invalidates_before = ch.tlb_invalidates.len();
                // Methods below 0x100 are the host's; the rest go to the class bound to
                // the subchannel. A subchannel without SET_OBJECT is taken as the copy
                // engine (UVM's CE channels, E5a).
                let binding = ch.objects[subch];
                let class = ch.bound_class(binding);
                let r = if a < 0x100 {
                    Self::host_method(ch, mem, root, subch, a, data, now, ch.deferred_management)
                } else if ch.deferred_management && class == 0x5080 {
                    Self::deferred_management_method(ch, binding, a, data)
                } else if ch.deferred_management && class == 0xc597 {
                    Self::strict_report_method(ch, mem, root, subch, a, data)
                } else if ch.deferred_management && class == classes.compute {
                    Err(MethodError::Failed(format!(
                        "strict management chid {} rejects compute method {a:#x}",
                        ch.chid
                    )))
                } else if ch.deferred_management && class == classes.uvm_sw {
                    Err(MethodError::Failed(format!(
                        "strict management chid {} rejects UVM method {a:#x}",
                        ch.chid
                    )))
                } else if class == classes.uvm_sw {
                    Ok(Some(format!(
                        "sw method subch {subch} {a:#x} = {data:#x} ignored (GP100_UVM_SW)"
                    )))
                } else if class == classes.compute {
                    Ok(Self::compute_method(ch, mem, root, a, data, now))
                } else if class == classes.copy || class == 0 {
                    Self::ce_method(ch, mem, root, a, data, now)
                } else if ch.deferred_management {
                    Err(MethodError::Failed(format!(
                        "strict management chid {} rejects class {class:#x} method {a:#x}",
                        ch.chid
                    )))
                } else {
                    Ok(Some(format!(
                        "subch {subch} class {class:#x} method {a:#x} = {data:#x} skipped (class not modelled)"
                    )))
                };
                match r {
                    Ok(Some(n)) => notes.push(n),
                    Ok(None) => {}
                    Err(MethodError::Blocked(n)) => {
                        notes.push(n);
                        if diagnose && a == 0x1C {
                            let h = &ch.host;
                            let sem_va = ((h.legacy_sem[0] as u64 & 0xFF) << 32)
                                | (h.legacy_sem[1] as u64 & !3);
                            let payload = h.legacy_sem[2];
                            if ch.sem_diag_seen.len() < SEM_DIAG_TUPLES
                                && ch.sem_diag_seen.insert((sem_va, payload))
                            {
                                let start = sem_va.saturating_sub(8);
                                let mut nearby = [0u8; 20];
                                let resolved = mem.pieces(root, start, nearby.len() as u64);
                                let words = mem
                                    .read_va(root, start, &mut nearby)
                                    .map(|()| {
                                        nearby
                                            .chunks_exact(4)
                                            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                                            .map(|v| format!("{v:#x}"))
                                            .collect::<Vec<_>>()
                                            .join(" ")
                                    })
                                    .unwrap_or_else(|e| format!("unreadable: {e}"));
                                let raw = w
                                    .iter()
                                    .take(64)
                                    .map(|v| format!("{v:08x}"))
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                notes.push(format!(
                                    "SEMAPHORE DIAG chid {} gp {} push va {va:#x} dwords {} raw[0..{}] [{raw}]; blocked decoded dword {pos}: subch {subch} method {a:#x} data {data:#x}, legacy A/B/C [{:#x} {:#x} {payload:#x}], target window {start:#x}+{:#x} -> {resolved:?}, words [{words}]",
                                    ch.chid,
                                    ch.gp_get,
                                    w.len(),
                                    w.len().min(64),
                                    h.legacy_sem[0],
                                    h.legacy_sem[1],
                                    nearby.len()
                                ));
                            }
                        }
                        ch.resume = Some(pos);
                        return Ok((notes, Stop::Blocked));
                    }
                    Err(MethodError::Failed(n)) => {
                        let n = if ch.deferred_management {
                            let raw = w
                                .iter()
                                .take(64)
                                .map(|word| format!("{word:08x}"))
                                .collect::<Vec<_>>()
                                .join(" ");
                            format!(
                                "{n}; strict failure at dword {pos}, subch {subch}, class {class:#x}, method {a:#x}, data {data:#x}; push prefix[0..{}] [{raw}]",
                                w.len().min(64)
                            )
                        } else {
                            n
                        };
                        ch.software_failed = true;
                        ch.software_failure = Some(n.clone());
                        ch.software_nonstall = false;
                        return Err(n);
                    }
                }
                // A TLB invalidate of a held shadow root (nvidia-uvm's page-table update
                // of a CUDA VA space ends in one): the rest of the push, above all the
                // semaphore release UVM waits for, runs only after the dispatcher applied
                // the shadow ([`Self::doorbell_holding`]). Resumed after this method.
                if a < 0x100
                    && ch.tlb_invalidates.len() > invalidates_before
                    && hold.is_some_and(|h| {
                        ch.tlb_invalidates[invalidates_before..]
                            .iter()
                            .any(|pdb| crate::untrapped::memop_holds(*pdb, h))
                    })
                {
                    ch.resume = Some(pos + 1);
                    return Ok((notes, Stop::HeldAtInvalidate));
                }
            }
        }
        Ok((notes, Stop::End))
    }

    #[allow(clippy::too_many_arguments)]
    fn host_method(
        ch: &mut Channel,
        mem: &mut GpuMem,
        root: Target,
        subch: usize,
        addr: u32,
        data: u32,
        now: u64,
        strict: bool,
    ) -> Result<Option<String>, MethodError> {
        let h = &mut ch.host;
        Ok(match addr {
            // SET_OBJECT: NVCLASS 15:0 (clc46f.h:68-71).
            0x00 => {
                ch.objects[subch] = data & 0xFFFF;
                Some(format!(
                    "host SET_OBJECT subch {subch} class {:#x}",
                    data & 0xFFFF
                ))
            }
            0x08 => None,
            0x10 => {
                h.legacy_sem[0] = data;
                None
            }
            0x14 => {
                h.legacy_sem[1] = data;
                None
            }
            0x18 => {
                h.legacy_sem[2] = data;
                None
            }
            // Legacy SEMAPHOREA-D is used after the Windows CE-utils transfers.
            // 0x01100002 means a four-byte RELEASE, not a no-op.
            0x1C => {
                let va = ((h.legacy_sem[0] as u64 & 0xFF) << 32) | (h.legacy_sem[1] as u64 & !3);
                let payload = h.legacy_sem[2];
                let op = data & 0x1F;
                let size = if op == 2 && data & (1 << 24) == 0 {
                    16
                } else {
                    4
                };
                let pieces = ce_pieces(mem, root, false, 0, va, size, ch.software_fb_range)
                    .map_err(MethodError::Failed)?;
                mem.fb.flush();
                if op == 2 {
                    let mut b = [0u8; 16];
                    b[..4].copy_from_slice(&payload.to_le_bytes());
                    if size == 16 {
                        b[8..].copy_from_slice(&now.to_le_bytes());
                    }
                    ce_publish(mem, &pieces, &b[..size as usize], 4)
                        .map_err(MethodError::Failed)?;
                    mem.fb.flush();
                    Some(format!(
                        "host legacy semaphore release va {va:#x} = {payload:#x} [guest]"
                    ))
                } else if matches!(op, 1 | 4 | 8) {
                    let mut b = [0u8; 4];
                    ce_read(mem, &pieces, &mut b).map_err(MethodError::Failed)?;
                    let cur = u32::from_le_bytes(b);
                    let ready = match op {
                        1 => cur == payload,
                        4 => cur >= payload,
                        _ => cur & payload != 0,
                    };
                    if !ready {
                        ch.acquire_wait = Some(AcquireWait {
                            root,
                            va,
                            wide: false,
                            payload: payload as u64,
                            op: match op {
                                1 => AcquireOp::Eq,
                                4 => AcquireOp::Geq,
                                _ => AcquireOp::And,
                            },
                        });
                        ACQUIRE_WAITS.store(true, std::sync::atomic::Ordering::Release);
                        return Err(MethodError::Blocked(format!("host legacy semaphore acquire va {va:#x}: {cur:#x} vs {payload:#x}, channel waits")));
                    }
                    None
                } else {
                    return Err(MethodError::Failed(format!(
                        "host legacy semaphore operation {op:#x} not implemented"
                    )));
                }
            }
            0x20 => {
                mem.fb.flush();
                ch.software_nonstall = true;
                Some("host NON_STALL_INTERRUPT queued after software writes".into())
            }
            0x24 => {
                mem.fb.flush();
                None
            }
            0x28 => {
                h.mem_op[0] = data;
                None
            }
            0x2C => {
                h.mem_op[1] = data;
                None
            }
            0x30 => {
                h.mem_op[2] = data;
                None
            }
            // MEM_OP_D OPERATION 31:27: 5 MEMBAR, 9 MMU_TLB_INVALIDATE, 0xa targeted,
            // others L2 maintenance (clc46f.h:176-192). Nothing is cached here; a TLB
            // invalidate is queued for the host mirrors and the root watch with the page
            // directory it names (pdbwatch.rs `memop_pdb`: MEM_OP_C PDB 0:0 is ONE when 0).
            0x34 => {
                let op = data >> 27;
                if strict && op != 5 {
                    return Err(MethodError::Failed(format!(
                        "strict management chid {} rejects MEM_OP operation {op:#x}",
                        ch.chid
                    )));
                }
                let what = match op {
                    5 => "membar",
                    9 => "tlb invalidate",
                    0xA => "targeted tlb invalidate",
                    0x16 => "access counter clear",
                    _ => "l2 or other",
                };
                let mut forwarded = "";
                let mut pdb_note = String::new();
                if op == 9 || op == 0xA {
                    let pdb = crate::pdbwatch::memop_pdb(h.mem_op[2], data);
                    ch.tlb_invalidates.push(pdb);
                    // The executor's own cached translations end here too (fb.rs `ExecTlb`).
                    mem.fb.exec_tlb_flush();
                    pdb_note = match pdb {
                        Some(p) => format!(" pdb {p:?}"),
                        None => " all pdbs".into(),
                    };
                    forwarded = "; queued for the host mirrors";
                }
                mem.fb.flush();
                Some(format!(
                    "host MEM_OP {what} (op {op:#x}, A {:#x} B {:#x} C {:#x}{pdb_note}): no cache to act on{forwarded}",
                    h.mem_op[0], h.mem_op[1], h.mem_op[2]
                ))
            }
            // SET_REFERENCE is GPU-owned USERD state. Strict management publishes it
            // synchronously; no GPU work or completion is implied.
            0x50 if strict => {
                let userd = ch.userd.ok_or_else(|| {
                    MethodError::Failed(format!(
                        "strict management chid {} has no USERD for SET_REFERENCE",
                        ch.chid
                    ))
                })?;
                let at = match userd {
                    Target::Fb(pa) => Target::Fb(pa.checked_add(0x48).ok_or_else(|| {
                        MethodError::Failed("SET_REFERENCE USERD address overflow".into())
                    })?),
                    Target::Sys(pa) => Target::Sys(pa.checked_add(0x48).ok_or_else(|| {
                        MethodError::Failed("SET_REFERENCE USERD address overflow".into())
                    })?),
                    _ => {
                        return Err(MethodError::Failed(
                            "SET_REFERENCE USERD aperture is not writable".into(),
                        ))
                    }
                };
                if !mem.write(at, &data.to_le_bytes()) {
                    return Err(MethodError::Failed(format!(
                        "strict management chid {} SET_REFERENCE USERD write failed",
                        ch.chid
                    )));
                }
                mem.fb.flush();
                Some(format!(
                    "strict management SET_REFERENCE USERD {at:?} = {data:#x} [guest]"
                ))
            }
            0x5C => {
                h.sem_addr_lo = data;
                None
            }
            0x60 => {
                h.sem_addr_hi = data;
                None
            }
            0x64 => {
                h.sem_payload_lo = data;
                None
            }
            0x68 => {
                h.sem_payload_hi = data;
                None
            }
            0x6C => {
                // SEM_EXECUTE: OPERATION 2:0, PAYLOAD_SIZE 24, RELEASE_TIMESTAMP 25
                // (clc46f.h:209-241); address SEM_ADDR_HI 7:0 : SEM_ADDR_LO 31:2.
                let va = ((h.sem_addr_hi as u64 & 0xFF) << 32) | (h.sem_addr_lo as u64 & !3);
                let wide = data & (1 << 24) != 0;
                let payload = ((h.sem_payload_hi as u64) << 32) | h.sem_payload_lo as u64;
                match data & 7 {
                    1 => {
                        let mut b = [0u8; 16];
                        let size = if data & (1 << 25) != 0 {
                            b[..8].copy_from_slice(&payload.to_le_bytes());
                            b[8..].copy_from_slice(&now.to_le_bytes());
                            16
                        } else if wide {
                            b[..8].copy_from_slice(&payload.to_le_bytes());
                            8
                        } else {
                            b[..4].copy_from_slice(&(payload as u32).to_le_bytes());
                            4
                        };
                        let pieces = ce_pieces(mem, root, false, 0, va, size, ch.software_fb_range)
                            .map_err(MethodError::Failed)?;
                        mem.fb.flush();
                        ce_publish(mem, &pieces, &b[..size as usize], if wide { 8 } else { 4 })
                            .map_err(MethodError::Failed)?;
                        mem.fb.flush();
                        Some(format!(
                            "host semaphore release va {va:#x} payload {payload:#x}{} [guest]",
                            if data & (1 << 25) != 0 {
                                " + timestamp [synthesized]"
                            } else {
                                ""
                            }
                        ))
                    }
                    op @ (0 | 2 | 3) => {
                        let mut b = [0u8; 8];
                        let n = if wide { 8 } else { 4 };
                        let pieces =
                            ce_pieces(mem, root, false, 0, va, n as u64, ch.software_fb_range)
                                .map_err(MethodError::Failed)?;
                        ce_read(mem, &pieces, &mut b[..n]).map_err(MethodError::Failed)?;
                        let cur = u64::from_le_bytes(b);
                        let (cur, want) = if wide {
                            (cur, payload)
                        } else {
                            (cur & 0xFFFF_FFFF, payload & 0xFFFF_FFFF)
                        };
                        let ok = match op {
                            0 => cur == want,
                            2 => cur >= want,
                            // ACQ_CIRC_GEQ: (cur - want) as signed >= 0.
                            _ => {
                                if wide {
                                    (cur.wrapping_sub(want) as i64) >= 0
                                } else {
                                    ((cur as u32).wrapping_sub(want as u32) as i32) >= 0
                                }
                            }
                        };
                        if !ok {
                            ch.acquire_wait = Some(AcquireWait {
                                root,
                                va,
                                wide,
                                payload,
                                op: match op {
                                    0 => AcquireOp::Eq,
                                    2 => AcquireOp::Geq,
                                    _ => AcquireOp::CircGeq,
                                },
                            });
                            ACQUIRE_WAITS.store(true, std::sync::atomic::Ordering::Release);
                            return Err(MethodError::Blocked(format!(
                                "host semaphore acquire va {va:#x} op {op}: {cur:#x} vs {want:#x}, channel waits"
                            )));
                        }
                        None
                    }
                    op => {
                        return Err(MethodError::Failed(format!(
                            "host SEM_EXECUTE op {op} not modelled"
                        )))
                    }
                }
            }
            0x78 => {
                mem.fb.flush();
                None
            }
            a if strict => {
                return Err(MethodError::Failed(format!(
                    "strict management chid {} rejects unmodelled host method {a:#x}",
                    ch.chid
                )))
            }
            a => Some(format!("host method {a:#x} = {data:#x} not modelled")),
        })
    }

    fn deferred_management_method(
        ch: &mut Channel,
        binding: u32,
        addr: u32,
        api: u32,
    ) -> Result<Option<String>, MethodError> {
        if addr != 0x200 {
            return Err(MethodError::Failed(format!(
                "strict management chid {} rejects class 0x5080 method {addr:#x}",
                ch.chid
            )));
        }
        let objects = if binding == 0x5080 {
            ch.engine_objects_with_handles()
                .filter_map(|(handle, class, _)| (class == 0x5080).then_some(handle))
                .collect::<Vec<_>>()
        } else {
            u16::try_from(binding)
                .ok()
                .and_then(|id| ch.software_objects.get(&id).copied())
                .filter(|(_, class)| *class == 0x5080)
                .map(|(handle, _)| vec![handle])
                .unwrap_or_default()
        };
        let Some(&object) = objects.first() else {
            return Err(MethodError::Failed(format!(
                "strict management chid {} has no recorded 0x5080 object",
                ch.chid
            )));
        };
        if objects.len() != 1 {
            return Err(MethodError::Failed(format!(
                "strict management chid {} has ambiguous 0x5080 objects",
                ch.chid
            )));
        }
        match ch.deferred {
            None => {
                ch.deferred = Some(DeferredState::Pending { object, api });
                Err(MethodError::Blocked(format!(
                    "strict management chid {} deferred object {object:#x} API {api:#x}",
                    ch.chid
                )))
            }
            Some(DeferredState::Pending {
                object: pending_object,
                api: pending_api,
            }) if (pending_object, pending_api) == (object, api) => {
                Err(MethodError::Blocked(format!(
                    "strict management chid {} still waits for object {object:#x} API {api:#x}",
                    ch.chid
                )))
            }
            Some(DeferredState::Completed {
                object: completed_object,
                api: completed_api,
            }) if (completed_object, completed_api) == (object, api) => {
                ch.deferred = None;
                Ok(Some(format!(
                    "strict management chid {} completed object {object:#x} API {api:#x}",
                    ch.chid
                )))
            }
            Some(state) => Err(MethodError::Failed(format!(
                "strict management chid {} deferred state {state:?} does not match object {object:#x} API {api:#x}",
                ch.chid
            ))),
        }
    }

    fn strict_report_method(
        ch: &mut Channel,
        mem: &mut GpuMem,
        root: Target,
        subch: usize,
        addr: u32,
        data: u32,
    ) -> Result<Option<String>, MethodError> {
        let report = &mut ch.strict_reports[subch];
        match addr {
            0x1b00..=0x1b08 if (addr - 0x1b00) % 4 == 0 => {
                let index = ((addr - 0x1b00) / 4) as usize;
                report.words[index] = data;
                report.initialized |= 1 << index;
                Ok(None)
            }
            0x1b0c => {
                if data != 0x1000_f010 {
                    return Err(MethodError::Failed(format!(
                        "strict C597 REPORT_SEMAPHORE_D {data:#x} is not the captured release"
                    )));
                }
                if report.initialized != 0b111 {
                    return Err(MethodError::Failed(
                        "strict C597 report semaphore A/B/C are not initialized".into(),
                    ));
                }
                let high = report.words[0];
                let low = report.words[1];
                let payload = report.words[2];
                if high & !0xff != 0 || low & 3 != 0 {
                    return Err(MethodError::Failed(format!(
                        "strict C597 report semaphore address {high:#x}:{low:#x} is invalid"
                    )));
                }
                let va = (u64::from(high) << 32) | u64::from(low);
                let pieces = ce_pieces(mem, root, false, 0, va, 4, ch.software_fb_range)
                    .map_err(MethodError::Failed)?;
                mem.fb.flush();
                ce_publish(mem, &pieces, &payload.to_le_bytes(), 4).map_err(MethodError::Failed)?;
                mem.fb.flush();
                report.initialized = 0;
                Ok(Some(format!(
                    "strict C597 report semaphore release va {va:#x} = {payload:#x} [guest]"
                )))
            }
            _ => Err(MethodError::Failed(format!(
                "strict C597 method {addr:#x} = {data:#x} not modelled"
            ))),
        }
    }

    /// The `TURING_COMPUTE_A` stub (E5c): report semaphores, inline-to-memory and its
    /// semaphore run; `SEND_PCAS_*` (a kernel launch) is refused and logged, since it
    /// needs the SMs (the execution wall, part 2); everything else is logged and skipped.
    fn compute_method(
        ch: &mut Channel,
        mem: &mut GpuMem,
        root: Target,
        addr: u32,
        data: u32,
        now: u64,
    ) -> Option<String> {
        let c = &mut ch.compute;
        match addr {
            // NO_OPERATION, WAIT_FOR_IDLE, INVALIDATE_SHADER_CACHES and the no-op spares:
            // nothing is cached or in flight here.
            0x100 | 0x110 | 0x1F0..=0x1FC | 0x21C => None,
            0x180 => {
                c.line_length_in = data;
                None
            }
            0x184 => {
                c.line_count = data;
                None
            }
            0x188 => {
                c.offset_out.0 = data & 0x1FFFF;
                None
            }
            0x18C => {
                c.offset_out.1 = data;
                None
            }
            0x190 => {
                c.pitch_out = data;
                None
            }
            // LAUNCH_DMA (clc5c0.h:141-172): the data follows as LOAD_INLINE_DATA.
            0x1B0 => {
                let lines = c.line_count.max(1) as usize;
                let bytes = (c.line_length_in as usize).saturating_mul(lines);
                if bytes > 1 << 24 {
                    return Some(format!("compute I2M of {bytes} bytes refused"));
                }
                c.i2m = Some((data, bytes, Vec::with_capacity(bytes)));
                if bytes == 0 {
                    return Self::i2m_finish(c, mem, root, now);
                }
                None
            }
            0x1B4 => {
                let Some((_, want, buf)) = c.i2m.as_mut() else {
                    return Some(format!(
                        "compute LOAD_INLINE_DATA {data:#x} without LAUNCH_DMA"
                    ));
                };
                let take = (*want - buf.len()).min(4);
                buf.extend_from_slice(&data.to_le_bytes()[..take]);
                if buf.len() >= *want {
                    return Self::i2m_finish(c, mem, root, now);
                }
                None
            }
            0x1DC => {
                c.i2m_sem[0] = data & 0xFF;
                None
            }
            0x1E0 => {
                c.i2m_sem[1] = data;
                None
            }
            0x1E4 => {
                c.i2m_sem[2] = data;
                None
            }
            0x2B4 => {
                c.pcas_a = data;
                None
            }
            // SET_SPA_VERSION: nothing executes shaders here.
            0x310 => None,
            // SET_INLINE_QMD_ADDRESS_A/B (clc5c0.h:390-394).
            0x318 => {
                c.qmd_addr.0 = data;
                None
            }
            0x31C => {
                c.qmd_addr.1 = data;
                None
            }
            // LOAD_INLINE_QMD_DATA(i) (clc5c0.h:396): the last dword launches the grid.
            0x320..=0x41C => {
                let i = ((addr - 0x320) / 4) as usize;
                c.qmd.resize(64, 0);
                c.qmd[i] = data;
                if i < 63 {
                    return None;
                }
                ch.launches_refused += 1;
                Some(format!(
                    "compute KERNEL LAUNCH NOT EXECUTED (needs the physical GPU) [execution wall]: {}",
                    describe_qmd(c.qmd_addr, &c.qmd)
                ))
            }
            0x2B8 | 0x2BC => {
                ch.launches_refused += 1;
                Some(format!(
                    "compute KERNEL LAUNCH NOT EXECUTED (needs the physical GPU): {} {data:#x}, QMD at va {:#x} [execution wall]",
                    if addr == 0x2B8 { "SEND_PCAS_B" } else { "SEND_SIGNALING_PCAS_B" },
                    (c.pcas_a as u64) << 8
                ))
            }
            0x1B00 => {
                c.sem[0] = data & 0xFF;
                None
            }
            0x1B04 => {
                c.sem[1] = data;
                None
            }
            0x1B08 => {
                c.sem[2] = data;
                None
            }
            // SET_REPORT_SEMAPHORE_D (clc5c0.h:631-660).
            0x1B0C => {
                let va = ((c.sem[0] as u64) << 32) | c.sem[1] as u64;
                let payload = c.sem[2];
                if data & 3 != 0 {
                    return Some(format!(
                        "compute report semaphore op {} at va {va:#x} not modelled",
                        data & 3
                    ));
                }
                let one_word = data & (1 << 28) != 0;
                let value = if data & (1 << 3) != 0 {
                    let mut b = [0u8; 4];
                    if let Err(e) = mem.read_va(root, va, &mut b) {
                        return Some(format!("compute report semaphore va {va:#x}: {e}"));
                    }
                    let old = u32::from_le_bytes(b);
                    // REDUCTION_OP 11:9 ADD MIN MAX INC DEC AND OR XOR, FORMAT 18:17
                    // 0 unsigned (clc5c0.h:647-660).
                    let signed = (data >> 17) & 3 == 1;
                    compute_reduce((data >> 9) & 7, signed, old, payload)
                } else {
                    payload
                };
                let r = if one_word {
                    mem.write_va(root, va, &value.to_le_bytes())
                } else {
                    let mut b = [0u8; 16];
                    b[..4].copy_from_slice(&value.to_le_bytes());
                    b[8..].copy_from_slice(&now.to_le_bytes());
                    mem.write_va(root, va, &b)
                };
                Some(match r {
                    Ok(()) => format!(
                        "compute report semaphore release va {va:#x} = {value:#x}{} [guest]{}",
                        if one_word {
                            ""
                        } else {
                            " + timestamp [synthesized]"
                        },
                        if data & (1 << 20) != 0 {
                            " (awaken requested, no event posted)"
                        } else {
                            ""
                        }
                    ),
                    Err(e) => format!("compute report semaphore release va {va:#x}: {e}"),
                })
            }
            a => Some(format!("compute method {a:#x} = {data:#x} skipped")),
        }
    }

    /// The end of an inline-to-memory transfer: the data to OFFSET_OUT (pitch layout),
    /// then the optional semaphore release.
    fn i2m_finish(
        c: &mut ComputeState,
        mem: &mut GpuMem,
        root: Target,
        now: u64,
    ) -> Option<String> {
        let (launch, _, buf) = c.i2m.take()?;
        let dst = ((c.offset_out.0 as u64) << 32) | c.offset_out.1 as u64;
        let mut parts = Vec::new();
        if launch & 1 == 0 {
            parts.push(format!(
                "compute I2M {} bytes to va {dst:#x}: block-linear layout not modelled",
                buf.len()
            ));
        } else {
            let line = (c.line_length_in as usize).max(1);
            let mut res = Ok(());
            for (i, chunk) in buf.chunks(line).enumerate() {
                res = mem.write_va(root, dst + i as u64 * c.pitch_out as u64, chunk);
                if res.is_err() {
                    break;
                }
            }
            parts.push(match res {
                Ok(()) => format!("compute I2M {} bytes -> va {dst:#x} [guest]", buf.len()),
                Err(e) => format!("compute I2M -> va {dst:#x} FAILED: {e}"),
            });
            // For a host-side replay of the guest's exact kernel bytes and parameter
            // uploads (caraxes/tools/host-kernel.rs): dump the payload as hex, up to 4 KiB,
            // behind CARAXES_DUMP_KERNEL=1 so the default trace stays quiet.
            if std::env::var("CARAXES_DUMP_KERNEL").as_deref() == Ok("1") {
                let hex: String = buf.iter().take(4096).map(|b| format!("{b:02x}")).collect();
                parts.push(format!("I2M BYTES va {dst:#x} len {}: {hex}", buf.len()));
            }
        }
        // COMPLETION_TYPE 5:4 = 2: release the I2M semaphore; STRUCT_SIZE 12 one word.
        if (launch >> 4) & 3 == 2 {
            let va = ((c.i2m_sem[0] as u64) << 32) | c.i2m_sem[1] as u64;
            let r = if launch & (1 << 12) != 0 {
                mem.write_va(root, va, &c.i2m_sem[2].to_le_bytes())
            } else {
                let mut b = [0u8; 16];
                b[..4].copy_from_slice(&c.i2m_sem[2].to_le_bytes());
                b[8..].copy_from_slice(&now.to_le_bytes());
                mem.write_va(root, va, &b)
            };
            parts.push(match r {
                Ok(()) => format!(
                    "compute I2M semaphore va {va:#x} = {:#x} [guest]",
                    c.i2m_sem[2]
                ),
                Err(e) => format!("compute I2M semaphore va {va:#x}: {e}"),
            });
        }
        if (launch >> 8) & 3 != 0 {
            parts.push("interrupt requested (not raised)".into());
        }
        Some(parts.join("; "))
    }

    fn ce_method(
        ch: &mut Channel,
        mem: &mut GpuMem,
        root: Target,
        addr: u32,
        data: u32,
        now: u64,
    ) -> Result<Option<String>, MethodError> {
        let c = &mut ch.ce;
        match addr {
            0x100 => {}
            0x240 => c.sem_upper = data & 0x1_FFFF,
            0x244 => c.sem_lower = data,
            0x248 => c.sem_payload = data,
            0x260 => c.src_phys = data,
            0x264 => c.dst_phys = data,
            0x300 => {
                let (fb_range, slice) = (ch.software_fb_range, ch.ce_slice);
                return match Self::launch_dma(&mut ch.ce, mem, root, data, now, fb_range, slice)
                    .map_err(MethodError::Failed)?
                {
                    CeOutcome::Done(note) => {
                        ch.software_nonstall |= (data >> 5) & 3 != 0;
                        Ok(Some(note))
                    }
                    CeOutcome::Sliced(note) => {
                        CE_SLICES_PENDING.store(true, std::sync::atomic::Ordering::Release);
                        Err(MethodError::Blocked(note))
                    }
                };
            }
            0x400 => c.offset_in.0 = data & 0x1_FFFF,
            0x404 => c.offset_in.1 = data,
            0x408 => c.offset_out.0 = data & 0x1_FFFF,
            0x40C => c.offset_out.1 = data,
            0x410 => c.pitch_in = data,
            0x414 => c.pitch_out = data,
            0x418 => c.line_length_in = data,
            0x41C => c.line_count = data,
            0x700 => c.remap_const_a = data,
            0x704 => c.remap_const_b = data,
            0x708 => c.remap_components = data,
            a => {
                return Err(MethodError::Failed(format!(
                    "ce method {a:#x} = {data:#x} not modelled"
                )))
            }
        }
        Ok(None)
    }

    /// Execute the bounded linear/constant-remap subset before publishing completion.
    /// Unsupported shapes and inaccessible memory leave GPGet and semaphores unchanged. A
    /// transfer runs for at most `budget` per call: a longer one returns
    /// [`CeOutcome::Sliced`] and continues where it stopped at the next call with the same
    /// LAUNCH_DMA, so the device's message loop answers between slices (run P29: a 64 MiB
    /// read-back of VRAM held the loop 6.7 s and QEMU dropped the device at its 5 s reply
    /// timeout; on Linux the scrubber's memsets stalled the desktop).
    fn launch_dma(
        c: &mut CeState,
        mem: &mut GpuMem,
        root: Target,
        v: u32,
        now: u64,
        fb_range: Option<(u64, u64)>,
        budget: std::time::Duration,
    ) -> Result<CeOutcome, String> {
        let start = std::time::Instant::now();
        let transfer = v & 3;
        let sem = (v >> 3) & 3;
        let interrupt = (v >> 5) & 3;
        let multi = v & (1 << 9) != 0;
        let remap = v & (1 << 10) != 0;
        let src_phys = v & (1 << 12) != 0;
        let dst_phys = v & (1 << 13) != 0;
        if transfer == 3 || sem == 3 || interrupt == 1 || interrupt == 3 {
            return Err(format!(
                "CE LAUNCH_DMA {v:#x}: unsupported transfer/semaphore/blocking interrupt"
            ));
        }
        if v & (3 << 22) != 0 || v >> 28 != 0 {
            return Err(format!(
                "CE LAUNCH_DMA {v:#x}: protected/reserved operation refused"
            ));
        }
        if multi && transfer != 0 && ((v & (1 << 8) == 0) || (!remap && v & (1 << 7) == 0)) {
            return Err("CE multiline block-linear transfer is not implemented".into());
        }
        let src = ((c.offset_in.0 as u64) << 32) | c.offset_in.1 as u64;
        let dst = ((c.offset_out.0 as u64) << 32) | c.offset_out.1 as u64;
        let sem_va = ((c.sem_upper as u64) << 32) | c.sem_lower as u64;
        // Resolve the completion destination before changing any transfer data.
        let sem_pieces = if sem != 0 {
            ce_pieces(
                mem,
                root,
                false,
                0,
                sem_va,
                if sem == 1 { 4 } else { 16 },
                fb_range,
            )?
        } else {
            Vec::new()
        };
        if sem == 2 && v & (1 << 19) != 0 {
            return Err("CE four-word semaphore reduction is not implemented".into());
        }
        let mut parts = Vec::new();
        if transfer != 0 {
            let lines = if multi { c.line_count as u64 } else { 1 };
            let bytes_per_line = if remap {
                (c.line_length_in as u64)
                    .checked_mul((((c.remap_components >> 16) & 3) + 1) as u64)
                    .and_then(|n| n.checked_mul((((c.remap_components >> 24) & 3) + 1) as u64))
                    .ok_or("CE remap size overflow")?
            } else {
                c.line_length_in as u64
            };
            let bytes = bytes_per_line
                .checked_mul(lines)
                .ok_or("CE transfer size overflow")?;
            // A memset (constant remap) writes the guest's own memory with no buffer the
            // size of the transfer, so it gets the larger fill budget: the guest's scrubber
            // clears freed allocations in one LAUNCH_DMA (132448256 bytes seen, 615 with
            // scrub-on-free, 2026-09-28). A copy keeps the copy budget.
            let max = if remap { CE_MAX_FILL_BYTES } else { CE_MAX_BYTES };
            if lines > CE_MAX_LINES || bytes_per_line > max || bytes > max {
                return Err(format!(
                    "CE transfer {lines} lines/{bytes} bytes exceeds software budget"
                ));
            }
            // A transfer that fits one slice is validated whole before its first write; a
            // longer one chunk by chunk (a fault then stops it part-way, as on hardware, and
            // no completion is published). Physical addresses never become host physical
            // addresses (ce_pieces checks each piece against the guest's own memory).
            let key = CeKey {
                v,
                src,
                dst,
                lines,
                bytes_per_line,
            };
            let row_at = |base: u64, pitch: u32, line: u64| {
                base.checked_add(if multi { line * pitch as u64 } else { 0 })
            };
            let what = if remap { "memset" } else { "memcopy" };
            // The device CE (devce.rs): a large transfer's bytes on the device-owned host copy
            // engine, submitted once and waited for through the held replay (the vfio-user
            // loop never waits for it). The guest's semaphore follows as on the CPU path; a
            // failed or refused device CE leaves the transfer to the CPU, never an RC.
            let mut on_device = None;
            if let Some(w) = c.device_wait.take().filter(|w| w.key == key) {
                let polled = w
                    .engine
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .poll(mem.fb, w.seq);
                match polled {
                    crate::devce::Poll::Done => {
                        for &(pa, len) in &w.ranges {
                            mem.fb.note_range_changed(pa, len);
                        }
                        on_device = Some(w.seq);
                    }
                    crate::devce::Poll::Pending => {
                        let seq = w.seq;
                        c.device_wait = Some(w);
                        return Ok(CeOutcome::Sliced(format!(
                            "ce {what} of {bytes} bytes waits for device CE push {seq} [host]"
                        )));
                    }
                    crate::devce::Poll::Failed(why) => {
                        parts.push(format!("device CE: {why}; this transfer runs on the CPU"));
                        c.progress = None;
                    }
                }
            } else if c.progress.is_none()
                && (bytes >= crate::devce::MIN_BYTES || (!remap && bytes >= DEVCE_VRAM_READ_MIN))
            {
                if let Some(engine) = mem.fb.device_ce() {
                    let mut e = engine.lock().unwrap_or_else(|e| e.into_inner());
                    if e.off().is_none() {
                        let w = e.window();
                        #[allow(clippy::type_complexity)]
                        let plan =
                            (|| -> Result<(Vec<crate::devce::Op>, Vec<(u64, u64)>), String> {
                                let (mut from, mut to) = (Vec::new(), Vec::new());
                                for line in 0..lines {
                                    let s_at = row_at(src, c.pitch_in, line).ok_or("overflow")?;
                                    let d_at = row_at(dst, c.pitch_out, line).ok_or("overflow")?;
                                    if !remap {
                                        from.extend(ce_pieces(
                                            mem,
                                            root,
                                            src_phys,
                                            c.src_phys,
                                            s_at,
                                            bytes_per_line,
                                            fb_range,
                                        )?);
                                    }
                                    to.extend(ce_pieces(
                                        mem,
                                        root,
                                        dst_phys,
                                        c.dst_phys,
                                        d_at,
                                        bytes_per_line,
                                        fb_range,
                                    )?);
                                }
                                // A small copy goes to the device CE only when it reads VRAM:
                                // the CPU reads VRAM through the BAR1 windows at about 10 MB/s
                                // (Windows C81: 63512 CPU copies, 19.9 s), while writes and guest
                                // RAM reads are fast on the CPU.
                                if !remap
                                    && bytes < crate::devce::MIN_BYTES
                                    && !from.iter().any(|(t, _)| matches!(t, Target::Fb(_)))
                                {
                                    return Err("a small copy without a VRAM source stays on the CPU".into());
                                }
                                let ops = if remap {
                                    let r = crate::devce::Remap {
                                        const_a: c.remap_const_a,
                                        const_b: c.remap_const_b,
                                        components: c.remap_components,
                                    };
                                    crate::devce::plan_fill(
                                        &w,
                                        mem.fb,
                                        &to,
                                        remap_elem(c)?.len() as u64,
                                        r,
                                    )?
                                } else {
                                    crate::devce::plan_copy(&w, mem.fb, &from, &to)?
                                };
                                // The written FB ranges, merged where contiguous.
                                let mut ranges: Vec<(u64, u64)> = Vec::new();
                                for &(t, n) in &to {
                                    if let Target::Fb(pa) = t {
                                        match ranges.last_mut() {
                                            Some((a, l)) if *a + *l == pa => *l += n as u64,
                                            _ => ranges.push((pa, n as u64)),
                                        }
                                    }
                                }
                                Ok((ops, ranges))
                            })();
                        if let Ok((ops, ranges)) = plan {
                            match e.submit(mem.fb, &ops) {
                                Ok(seq) => {
                                    drop(e);
                                    c.device_wait = Some(DeviceWait {
                                        key,
                                        seq,
                                        ranges,
                                        engine,
                                    });
                                    return Ok(CeOutcome::Sliced(format!(
                                        "ce {what} of {bytes} bytes submitted to the device CE as push {seq} ({} op(s)) [host]",
                                        ops.len()
                                    )));
                                }
                                Err(why) if why.contains("busy") => {
                                    return Ok(CeOutcome::Sliced(format!(
                                        "ce {what} of {bytes} bytes waits for the device CE (busy) [host]"
                                    )));
                                }
                                Err(_) => {}
                            }
                        }
                    }
                }
            }
            let (mut line, mut offset) = match c.progress.take() {
                // Done on the device CE: nothing left for the CPU.
                _ if on_device.is_some() => (lines, 0),
                Some(p) if p.key == key => (p.line, p.offset),
                _ => {
                    if bytes <= CE_VALIDATE_WHOLE {
                        for line in 0..lines {
                            let s_at = row_at(src, c.pitch_in, line)
                                .ok_or("CE source address overflow")?;
                            let d_at = row_at(dst, c.pitch_out, line)
                                .ok_or("CE destination address overflow")?;
                            if !remap {
                                ce_pieces(
                                    mem,
                                    root,
                                    src_phys,
                                    c.src_phys,
                                    s_at,
                                    bytes_per_line,
                                    fb_range,
                                )?;
                            }
                            ce_pieces(
                                mem,
                                root,
                                dst_phys,
                                c.dst_phys,
                                d_at,
                                bytes_per_line,
                                fb_range,
                            )?;
                        }
                    }
                    (0, 0)
                }
            };
            let elem = if remap { remap_elem(c)? } else { Vec::new() };
            // A fill chunk is written from one pattern buffer; a copy chunk is read into a
            // buffer first (VRAM reads through the BAR1 windows ran at 10 MB/s, run P29).
            let chunk = if remap {
                CE_FILL_CHUNK as u64
            } else {
                CE_COPY_CHUNK as u64
            };
            let mut data = Vec::new();
            while line < lines {
                let s_at = row_at(src, c.pitch_in, line)
                    .and_then(|a| a.checked_add(offset))
                    .ok_or("CE source address overflow")?;
                let d_at = row_at(dst, c.pitch_out, line)
                    .and_then(|a| a.checked_add(offset))
                    .ok_or("CE destination address overflow")?;
                let n = (bytes_per_line - offset).min(chunk);
                let to = ce_pieces(mem, root, dst_phys, c.dst_phys, d_at, n, fb_range)?;
                if remap {
                    // The pattern continues at this offset of the row.
                    let phase = (offset % elem.len().max(1) as u64) as usize;
                    let rotated: Vec<u8> = elem[phase..]
                        .iter()
                        .chain(&elem[..phase])
                        .copied()
                        .collect();
                    ce_fill(mem, &to, &rotated)?;
                } else {
                    let from = ce_pieces(mem, root, src_phys, c.src_phys, s_at, n, fb_range)?;
                    data.resize(n as usize, 0);
                    ce_read(mem, &from, &mut data)?;
                    ce_write(mem, &to, &data)?;
                }
                offset += n;
                if offset == bytes_per_line {
                    line += 1;
                    offset = 0;
                }
                if line < lines && start.elapsed() >= budget {
                    mem.fb.flush();
                    c.progress = Some(CeProgress { key, line, offset });
                    return Ok(CeOutcome::Sliced(format!(
                        "ce {} {} of {bytes} bytes in this slice's {} us; continued in the next slice [guest]",
                        if remap { "memset" } else { "memcopy" },
                        line * bytes_per_line + offset,
                        start.elapsed().as_micros()
                    )));
                }
            }
            let dst_desc = if dst_phys {
                format!(
                    "{:?}",
                    if c.dst_phys & 3 == 0 {
                        Target::Fb(dst)
                    } else {
                        Target::Sys(dst)
                    }
                )
            } else {
                format!("va {dst:#x}")
            };
            parts.push(format!(
                "ce {what} {bytes} bytes -> {dst_desc} [guest{}]",
                on_device.map_or(String::new(), |seq| format!(", device CE push {seq}"))
            ));
        }
        // BAR1 WC stores must be visible before a CPU/GPU waiter observes completion.
        mem.fb.flush();
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        if sem == 1 {
            let payload = if v & (1 << 19) != 0 {
                let mut b = [0u8; 4];
                ce_read(mem, &sem_pieces, &mut b)?;
                let op = (v >> 14) & 0xF;
                if op > 7 {
                    return Err(format!("CE semaphore reduction {op} not implemented"));
                }
                // The helper argument is unsigned: SIGN_UNSIGNED=1 (clc5b5.h).
                reduce(op, v & (1 << 18) != 0, u32::from_le_bytes(b), c.sem_payload)
            } else {
                c.sem_payload
            };
            ce_publish(mem, &sem_pieces, &payload.to_le_bytes(), 4)?;
            parts.push(format!(
                "ce semaphore release va {sem_va:#x} = {payload:#x} [guest]"
            ));
        } else if sem == 2 {
            let mut b = [0u8; 16];
            b[..4].copy_from_slice(&c.sem_payload.to_le_bytes());
            b[8..].copy_from_slice(&now.to_le_bytes());
            ce_publish(mem, &sem_pieces, &b, 4)?;
            parts.push(format!("ce semaphore release (four words) va {sem_va:#x} = {:#x} [guest] + timestamp {now} [synthesized]", c.sem_payload));
        }
        mem.fb.flush();
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
        if interrupt == 2 {
            parts.push("non-stall interrupt queued after completion".into());
        }
        if parts.is_empty() {
            parts.push(format!("ce LAUNCH_DMA {v:#x}: nothing to do"));
        }
        Ok(CeOutcome::Done(parts.join("; ")))
    }
}

// Explicit CPU-work and allocation limits, independent of guest-supplied dimensions.
/// A copy's bound: no buffer the size of the transfer is allocated (it runs in
/// [`CE_COPY_CHUNK`] pieces and [`CE_SLICE`]s), so a copy may cover the guest's own memory
/// as a memset may.
const CE_MAX_BYTES: u64 = 4 << 30;
/// The longest one call of the software CE works on a transfer before the rest continues in
/// the next slice: the vfio-user loop answers within that (QEMU's reply timeout is 5 s) and
/// the guest's other channels run in between. 40 ms until 2026-09-29; 4 ms since, so a
/// slice under the device lock stays within a few milliseconds (execution.md §9.7).
const CE_SLICE: std::time::Duration = std::time::Duration::from_millis(4);
/// The longest a software channel's run works through its GPFIFO under the device lock
/// before it stops between two entries; the slice clock (the device's `ce-slices` thread,
/// 1 ms) continues it at the same GPGet (docs/caraxes/execution.md §9.7). A doorbell only
/// promises that the entries run eventually; the channel's own entries, semaphores and
/// events keep their order, and other channels run in between as on the GPU. Windows C67:
/// software doorbells of the KMD's channel held the lock 516-562 ms.
pub const RUN_BUDGET: std::time::Duration = std::time::Duration::from_millis(2);
/// A copy that reads VRAM goes to the device CE from this size on (below
/// [`crate::devce::MIN_BYTES`] the rest stays on the CPU): at 10 MB/s a CPU read of 1 KiB of
/// VRAM costs about 100 us, a device CE push and its completion about as much.
const DEVCE_VRAM_READ_MIN: u64 = 1 << 10;
/// A copy's chunk between two slice checks: 64 KiB take 6.5 ms at the 10 MB/s of VRAM reads
/// through the BAR1 windows (run P29), so a slice ends within about 11 ms.
const CE_COPY_CHUNK: usize = 64 << 10;
/// Transfers up to this size are validated whole before their first write.
const CE_VALIDATE_WHOLE: u64 = 16 << 20;
/// A memset's bound: at most the guest's own memory it may address (ce_pieces checks each
/// piece against the guest FB and registered RAM), written in [`CE_FILL_CHUNK`] pieces.
const CE_MAX_FILL_BYTES: u64 = 4 << 30;
const CE_FILL_CHUNK: usize = 1 << 20;
const CE_MAX_LINES: u64 = 4096;

fn ce_pieces(
    mem: &GpuMem,
    root: Target,
    physical: bool,
    mode: u32,
    address: u64,
    len: u64,
    fb_range: Option<(u64, u64)>,
) -> Result<Vec<(Target, usize)>, String> {
    address
        .checked_add(len)
        .ok_or("CE address range overflow")?;
    if len > CE_MAX_FILL_BYTES {
        return Err("CE range exceeds software budget".into());
    }
    if physical && (mode & 3 == 3 || mode & !3 != 0) {
        return Err(format!("CE physical mode/kind {mode:#x} not implemented"));
    }
    let failed = mem.fb.vram_stats().map_or(0, |s| s.failed);
    let pieces = if physical {
        // A physical range is contiguous: one piece, checked whole below.
        let t = if mode & 3 == 0 {
            Target::Fb(address)
        } else {
            Target::Sys(address)
        };
        if len == 0 {
            Vec::new()
        } else {
            vec![(t, len as usize)]
        }
    } else {
        mem.pieces(root, address, len)?
    };
    if mem.fb.vram_stats().map_or(0, |s| s.failed) != failed {
        return Err("CE page-table read failed while resolving the address".into());
    }
    for &(target, n) in &pieces {
        let ok = match target {
            // The device's private pages (fb.rs `FbLayout::private_ranges`: the owned-GFXP
            // slots, the device CE's and the invalidator's pages) are no VM backing.
            Target::Fb(pa) => pa.checked_add(n as u64).is_some_and(|end| {
                end <= mem.fb.size()
                    && fb_range.is_none_or(|(lo, hi)| pa >= lo && end <= hi)
                    && !mem.fb.private_overlaps(pa, n as u64)
            }),
            Target::Sys(pa) => mem.sys.covers(pa, n as u64),
            _ => false,
        };
        if !ok {
            return Err(format!("CE range {target:?}+{n:#x} outside VM backing"));
        }
    }
    Ok(pieces)
}

fn ce_read(mem: &GpuMem, pieces: &[(Target, usize)], data: &mut [u8]) -> Result<(), String> {
    let _spent = crate::dlock::Spend::new(crate::dlock::Part::Copy);
    let mut off = 0;
    for &(target, n) in pieces {
        let failed = mem.fb.vram_stats().map_or(0, |s| s.failed);
        if !mem.read(target, &mut data[off..off + n])
            || mem.fb.vram_stats().map_or(0, |s| s.failed) != failed
        {
            return Err(format!("CE read {target:?}+{n:#x} failed"));
        }
        off += n;
    }
    Ok(())
}

fn ce_write(mem: &mut GpuMem, pieces: &[(Target, usize)], data: &[u8]) -> Result<(), String> {
    let _spent = crate::dlock::Spend::new(crate::dlock::Part::Copy);
    let mut off = 0;
    for &(target, n) in pieces {
        let failed = mem.fb.vram_stats().map_or(0, |s| s.failed);
        if !mem.write(target, &data[off..off + n])
            || mem.fb.vram_stats().map_or(0, |s| s.failed) != failed
        {
            return Err(format!("CE write {target:?}+{n:#x} failed"));
        }
        off += n;
    }
    Ok(())
}

/// Write completion metadata before its payload: a later-page failure cannot expose
/// a completed payload while its timestamp/body is still missing.
fn ce_publish(
    mem: &mut GpuMem,
    pieces: &[(Target, usize)],
    data: &[u8],
    payload: usize,
) -> Result<(), String> {
    let Some(&(first, first_len)) = pieces.first() else {
        return Err("empty semaphore destination".into());
    };
    let pa = match first {
        Target::Fb(pa) | Target::Sys(pa) => pa,
        _ => return Err("invalid semaphore aperture".into()),
    };
    if pa & (payload as u64 - 1) != 0 || first_len < payload {
        return Err("unaligned or fragmented semaphore payload refused".into());
    }
    if data.len() > payload {
        let mut tail = Vec::new();
        if first_len > payload {
            let at = match first {
                Target::Fb(pa) => Target::Fb(pa + payload as u64),
                Target::Sys(pa) => Target::Sys(pa + payload as u64),
                _ => unreachable!(),
            };
            tail.push((at, first_len - payload));
        }
        tail.extend_from_slice(&pieces[1..]);
        ce_write(mem, &tail, &data[payload..])?;
    }
    mem.fb.flush();
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
    ce_write(mem, &[(first, payload)], &data[..payload])?;
    mem.fb.flush();
    Ok(())
}

/// The error type of a method that has to wait.
#[derive(Debug)]
enum MethodError {
    Blocked(String),
    Failed(String),
}

/// Where a pushbuffer segment stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    /// At its end: the entry is done.
    End,
    /// At a host semaphore acquire not yet satisfied (resumed at that method).
    Blocked,
    /// Right after a `MEM_OP` TLB invalidate of a held shadow root (resumed after it,
    /// [`Registry::doorbell_holding`]).
    HeldAtInvalidate,
}

/// What [`Registry::doorbell_holding`] hands the software executor's `MEM_OP` TLB
/// invalidates to (the page directory each names, `None`: all); returns the trace notes.
pub type OnInvalidates<'a> = dyn FnMut(&Registry, &mut GpuMem, &[Option<Target>]) -> Vec<String> + 'a;

/// Rounds of invalidate, shadow apply and continue per doorbell; a channel still stopped
/// after them continues through the held-doorbell replay (its invalidates are handed over).
const MAX_INVALIDATE_ROUNDS: usize = 256;

/// One memset line from the remap constants: LINE_LENGTH_IN elements of
/// NUM_DST_COMPONENTS (25:24, +1) components of COMPONENT_SIZE (17:16, +1)
/// bytes, each component from CONST_A (4) or CONST_B (5) (clc5b5.h:155-206).
/// Write the element `elem` repeated over `pieces` (the destination rows of a memset),
/// in [`CE_FILL_CHUNK`] writes, the pattern continuing across pieces.
fn ce_fill(mem: &mut GpuMem, pieces: &[(Target, usize)], elem: &[u8]) -> Result<(), String> {
    let _spent = crate::dlock::Spend::new(crate::dlock::Part::Copy);
    let e = elem.len().max(1);
    // One chunk of whole elements plus one element of slack for the phase at a piece start.
    let reps = CE_FILL_CHUNK.div_ceil(e) + 1;
    let pattern: Vec<u8> = elem.iter().copied().cycle().take(reps * e).collect();
    let mut row_off = 0usize;
    for &(target, n) in pieces {
        let mut done = 0usize;
        while done < n {
            let len = (n - done).min(CE_FILL_CHUNK);
            let phase = (row_off + done) % e;
            let at = match target {
                Target::Fb(pa) => Target::Fb(pa + done as u64),
                Target::Sys(pa) => Target::Sys(pa + done as u64),
                t => t,
            };
            let failed = mem.fb.vram_stats().map_or(0, |s| s.failed);
            if !mem.write(at, &pattern[phase..phase + len])
                || mem.fb.vram_stats().map_or(0, |s| s.failed) != failed
            {
                return Err(format!("CE fill {at:?}+{len:#x} failed"));
            }
            done += len;
        }
        row_off += n;
    }
    Ok(())
}

/// One element of a constant remap (memset): the components the remap selects, each the
/// component size wide.
fn remap_elem(c: &CeState) -> Result<Vec<u8>, String> {
    let comp = ((c.remap_components >> 16) & 3) as usize + 1;
    let ncomp = ((c.remap_components >> 24) & 3) as usize + 1;
    let mut elem = Vec::with_capacity(comp * ncomp);
    for k in 0..ncomp {
        let sel = (c.remap_components >> (4 * k)) & 7;
        let v = match sel {
            4 => c.remap_const_a,
            5 => c.remap_const_b,
            s => {
                return Err(format!(
                    "remap component {k} selects {s}, only constants modelled"
                ))
            }
        };
        elem.extend_from_slice(&v.to_le_bytes()[..comp]);
    }
    Ok(elem)
}

/// A Turing QMD as far as the refused launch needs describing. The QMD V02_02 layout is
/// not in the vendored tree; the field positions are those of NVIDIA's public
/// `clc5c0qmd.h` (open-gpu-doc) and were checked against the first launch observed
/// (E5c: the release semaphore in dwords 23-25 is the value the CE channel then waits
/// for). The raw dwords are logged as well.
fn describe_qmd(addr: (u32, u32), q: &[u32]) -> String {
    let at = ((addr.0 as u64) << 40) | ((addr.1 as u64) << 8);
    let release = ((q[24] as u64 & 0xFF) << 32) | q[23] as u64;
    let raw: Vec<String> = q.iter().map(|d| format!("{d:08x}")).collect();
    format!(
        "QMD at va {at:#x}; grid {}x{}x{}; release0 va {release:#x} payload {:#x} (enable bit {}) is NOT written; raw [{}]",
        q[12],
        q[13] & 0xFFFF,
        q[14] & 0xFFFF,
        q[25],
        q[24] >> 31,
        raw.join(" ")
    )
}

/// A compute report-semaphore reduction (`clc5c0.h:647-660`: ADD, MIN, MAX, INC, DEC,
/// AND, OR, XOR); INC and DEC wrap at the payload as in [`reduce`].
fn compute_reduce(op: u32, signed: bool, old: u32, p: u32) -> u32 {
    match op {
        0 => old.wrapping_add(p),
        1 if signed => (old as i32).min(p as i32) as u32,
        1 => old.min(p),
        2 if signed => (old as i32).max(p as i32) as u32,
        2 => old.max(p),
        3 => reduce(6, true, old, p),
        4 => reduce(7, true, old, p),
        5 => old & p,
        6 => old | p,
        _ => old ^ p,
    }
}

/// A semaphore reduction (`clc5b5.h:109-124`); INC and DEC wrap at the payload
/// as the host class documents for its REDUCTION_INC/DEC.
fn reduce(op: u32, unsigned: bool, old: u32, p: u32) -> u32 {
    match op {
        0 if unsigned => old.min(p),
        0 => (old as i32).min(p as i32) as u32,
        1 if unsigned => old.max(p),
        1 => (old as i32).max(p as i32) as u32,
        2 => old ^ p,
        3 => old & p,
        4 => old | p,
        5 => old.wrapping_add(p),
        6 => {
            if old >= p {
                0
            } else {
                old + 1
            }
        }
        7 => {
            if old == 0 || old > p {
                p
            } else {
                old - 1
            }
        }
        _ => p,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TURING_CHANNEL_GPFIFO_A: u32 = 0xC46F;
    const TURING_DMA_COPY_A: u32 = 0xC5B5;
    const GP100_UVM_SW: u32 = 0xC076;

    /// The TU106 classes (chips/tu106/chip.toml), which the tests push by number.
    fn classes() -> ChannelClasses {
        let c = crate::fixture::rtx2070().chip.data().channel;
        assert_eq!(
            (c.gpfifo, c.copy, c.uvm_sw),
            (TURING_CHANNEL_GPFIFO_A, TURING_DMA_COPY_A, GP100_UVM_SW)
        );
        c
    }
    use crate::msgq::tests::queue_fixture;

    /// Guest RAM at 0x4000_0000 (1 MiB file), fake FB of 64 MiB. VA 0x1000_0000
    /// maps 16 small pages: VA page k → FB 0x100000 + k*4K for k < 8, and guest
    /// RAM 0x4000_0000 + (k-8)*4K for k >= 8.
    fn world(tag: &str) -> (FakeFb, GuestMemory, std::path::PathBuf, Target) {
        let (sys, _, _, path) = queue_fixture(tag);
        let mut fb = FakeFb::new(64 << 20);
        let (root, pd2, pd1, pd0, pt) = (0x10000u64, 0x11000, 0x12000, 0x13000, 0x14000);
        let vid = |a: u64| ((a >> 12) << 8) | 0b010;
        fb.write_u64(root, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        // VA 0x1000_0000: PD1 index 0 (bits 37:29), PD0 index 0x80 (bits 28:21).
        fb.write_u64(pd1, vid(pd0));
        fb.write_u64(pd0 + 16 * 0x80 + 8, vid(pt));
        for k in 0..16u64 {
            let pte = if k < 8 {
                (((0x100000 + k * 0x1000) >> 12) << 8) | 1
            } else {
                (((0x4000_0000 + (k - 8) * 0x1000) >> 12) << 8) | 0b101
            };
            fb.write_u64(pt + 8 * k, pte);
        }
        (fb, sys, path, Target::Fb(root))
    }

    fn inc(subch: u32, addr: u32, data: &[u32]) -> Vec<u32> {
        let mut v = vec![(1 << 29) | ((data.len() as u32) << 16) | (subch << 13) | (addr >> 2)];
        v.extend_from_slice(data);
        v
    }

    fn channel_params(chid: u32, gpfifo_va: u64, userd_pa: u64, vas: u32) -> Vec<u8> {
        let mut b = vec![0u8; <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_SIZE as usize];
        let put = |b: &mut [u8], o: usize, v: &[u8]| b[o..o + v.len()].copy_from_slice(v);
        put(
            &mut b,
            <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_gpFifoOffset,
            &gpfifo_va.to_le_bytes(),
        );
        put(
            &mut b,
            <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_gpFifoEntries,
            &32u32.to_le_bytes(),
        );
        let flags = (1u32 << 21) | ((chid / 8) << 12) | ((chid % 8) << 8);
        put(
            &mut b,
            <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_flags,
            &flags.to_le_bytes(),
        );
        put(
            &mut b,
            <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_hVASpace,
            &vas.to_le_bytes(),
        );
        put(
            &mut b,
            <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_userdMem,
            &userd_pa.to_le_bytes(),
        );
        put(
            &mut b,
            <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_userdMem + 16,
            &ADDR_FBMEM.to_le_bytes(),
        );
        put(
            &mut b,
            <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_engineType,
            &0x0Du32.to_le_bytes(),
        );
        b
    }

    fn software_fixture(
        tag: &str,
        pb: &[u32],
    ) -> (FakeFb, GuestMemory, std::path::PathBuf, Registry) {
        let (mut fb, sys, path, root) = world(tag);
        let mut reg = Registry::new(classes());
        reg.vaspaces.insert((1, 0x20), Some(root));
        let mut params = channel_params(9, 0x1000_0000, 0x20_0000, 0x20);
        let off = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_flags;
        let flags = rd32(&params, off) | (1 << 5);
        params[off..off + 4].copy_from_slice(&flags.to_le_bytes());
        reg.record_alloc(1, 0x10, 0x40, TURING_CHANNEL_GPFIFO_A, &params)
            .unwrap();
        let bytes: Vec<u8> = pb.iter().flat_map(|w| w.to_le_bytes()).collect();
        assert!(sys.write(0x4000_0000, &bytes));
        fb.write_u64(0x100000, 0x1000_8000 | (((pb.len() as u64) << 10) << 32));
        fb.write(0x20_0000 + USERD_GP_PUT, &1u32.to_le_bytes());
        (fb, sys, path, reg)
    }

    fn physical_fill(dst: u64, elements: u32) -> Vec<u32> {
        let mut pb = inc(0, 0, &[TURING_DMA_COPY_A]);
        pb.extend(inc(0, 0x700, &[0xAABB_CCDD, 0, 0x0003_0004]));
        pb.extend(inc(0, 0x264, &[0]));
        pb.extend(inc(0, 0x408, &[(dst >> 32) as u32, dst as u32]));
        pb.extend(inc(0, 0x418, &[elements]));
        pb.extend(inc(0, 0x240, &[0, 0x1000_9004, 1]));
        // The first Windows CE-utils launch, including physical destination, pitch,
        // constant remap, flush, one-word semaphore, and DISABLE_PLC.
        pb.extend(inc(0, 0x300, &[0x0400_258e]));
        pb
    }

    /// `MEM_OP_A..D` of a targeted TLB invalidate (OPERATION 0xa) of the vidmem page
    /// directory `pdb`, or of every page directory (`None`), as nvidia-uvm pushes it.
    fn memop_invalidate(pdb: Option<u64>) -> Vec<u32> {
        let (c, d) = match pdb {
            Some(p) => ((p as u32) & 0xFFFF_F000, (0xA << 27) | ((p >> 32) as u32 & 0x7FF_FFFF)),
            None => (1, 0xA << 27),
        };
        inc(0, 0x28, &[0, 0, c, d])
    }

    /// A 64-bit legacy semaphore release of `payload` at VA `va` (the fixture maps VA
    /// 0x1000_8000.. to guest RAM 0x4000_0000..).
    fn release(va: u32, payload: u32) -> Vec<u32> {
        inc(0, 0x10, &[0, va, payload, 0x0110_0002])
    }

    /// What `on_invalidates` saw at each call: the invalidates, the two semaphores (guest
    /// RAM 0x4000_1000 and 0x4000_1010) and the channel's GPGet.
    type Seen = Vec<(Vec<Option<Target>>, Option<u32>, Option<u32>, u32)>;

    fn doorbell_seen(
        tag: &str,
        pb: &[u32],
        hold: Option<std::collections::BTreeSet<u64>>,
    ) -> (Seen, String, FakeFb, GuestMemory, Registry) {
        let (mut fb, sys, path, mut reg) = software_fixture(tag, pb);
        let mut seen: Seen = Vec::new();
        let notes = {
            let mut mem = GpuMem {
                fb: &mut fb,
                sys: &sys,
            };
            reg.doorbell_holding(
                &mut mem,
                9,
                &std::collections::BTreeSet::new(),
                hold,
                &mut |reg, mem, inv| {
                    seen.push((
                        inv.to_vec(),
                        mem.sys.read_u32(0x4000_1000),
                        mem.sys.read_u32(0x4000_1010),
                        reg.channel(9).unwrap().gp_get,
                    ));
                    vec!["shadow applied".to_string()]
                },
            )
            .join("\n")
        };
        std::fs::remove_file(path).ok();
        (seen, notes, fb, sys, reg)
    }

    /// The Xid 31 of 2026-09-28 (docs/research/shadow-fault-race-2026-09-28.md): nvidia-uvm
    /// maps a CUDA allocation with PTE writes, a `MEM_OP` TLB invalidate of the VA space's
    /// root and the semaphore release it waits for, all on its software CE channel. With
    /// the root held the push stops right after the invalidate, so the release reaches the
    /// guest only after `on_invalidates` (the dispatcher's shadow apply) returned.
    #[test]
    fn a_memop_invalidate_of_a_held_root_releases_nothing_before_the_shadow_is_applied() {
        let mut pb = memop_invalidate(Some(0x1_0000));
        pb.extend(release(0x1000_9000, 1));
        let hold = Some([0x1_0000u64].into());
        let (seen, notes, fb, sys, reg) = doorbell_seen("memop-hold", &pb, hold);
        assert_eq!(seen, vec![(vec![Some(Target::Fb(0x1_0000))], Some(0), Some(0), 0)]);
        assert!(notes.contains("stopped after a TLB invalidate of a shadow root"), "{notes}");
        assert!(notes.contains("shadow applied"), "{notes}");
        // Continued after the apply: released, GPGet past the entry, nothing left stopped,
        // the invalidate handed over once.
        assert_eq!(sys.read_u32(0x4000_1000), Some(1));
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 1);
        assert!(reg.blocked_software_channels().is_empty());
        assert!(reg.tlb_invalidates.is_empty());
        assert!(reg.memop_hold.is_none());
    }

    /// The order before the fix, still the order without a hold: the release is in guest
    /// memory before the dispatcher sees the invalidate (the window a host channel with an
    /// untrapped USERD ran the guest's next work in).
    #[test]
    fn without_a_hold_the_release_comes_before_the_invalidate_is_handed_over() {
        let mut pb = memop_invalidate(Some(0x1_0000));
        pb.extend(release(0x1000_9000, 1));
        let (seen, notes, ..) = doorbell_seen("memop-nohold", &pb, None);
        assert_eq!(seen, vec![(vec![Some(Target::Fb(0x1_0000))], Some(1), Some(0), 1)]);
        assert!(!notes.contains("stopped after"), "{notes}");
        // Another root held: the same.
        let (seen, ..) = doorbell_seen("memop-other", &pb, Some([0x2_0000u64].into()));
        assert_eq!(seen, vec![(vec![Some(Target::Fb(0x1_0000))], Some(1), Some(0), 1)]);
    }

    /// Two invalidates in one push (the root, then every page directory): the push stops
    /// after each, every method runs once, and each release follows its invalidate's apply.
    #[test]
    fn every_held_invalidate_of_a_push_stops_it_once() {
        let mut pb = memop_invalidate(Some(0x1_0000));
        pb.extend(release(0x1000_9000, 1));
        pb.extend(memop_invalidate(None));
        pb.extend(release(0x1000_9010, 2));
        let hold = Some([0x1_0000u64].into());
        let (seen, notes, fb, sys, reg) = doorbell_seen("memop-two", &pb, hold);
        assert_eq!(
            seen,
            vec![
                (vec![Some(Target::Fb(0x1_0000))], Some(0), Some(0), 0),
                (vec![None], Some(1), Some(0), 0),
            ]
        );
        assert_eq!(notes.matches("stopped after a TLB invalidate").count(), 2, "{notes}");
        assert_eq!((sys.read_u32(0x4000_1000), sys.read_u32(0x4000_1010)), (Some(1), Some(2)));
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 1);
        assert!(reg.blocked_software_channels().is_empty());
    }

    /// A physical FB-to-FB copy of `len` bytes from `src` to `dst` (CeState only).
    fn physical_copy(src: u64, dst: u64, len: u32) -> CeState {
        CeState {
            offset_in: ((src >> 32) as u32, src as u32),
            offset_out: ((dst >> 32) as u32, dst as u32),
            line_length_in: len,
            ..Default::default()
        }
    }

    /// LAUNCH_DMA of a physical-to-physical pitch copy, no semaphore, no interrupt.
    const PHYS_COPY: u32 = 0x3182;

    /// Runs P29/P29b and the Linux scrub stalls: a 1 GiB software copy never holds the
    /// caller (the vfio-user loop) longer than the slice bound; each call ends within
    /// CE_SLICE plus one chunk, returns Sliced and continues where it stopped.
    #[test]
    fn a_1_gib_software_copy_never_holds_the_loop_longer_than_the_slice_bound() {
        let (_, sys, path, root) = world("ce-slice-1g");
        let mut fb = FakeFb::new(4 << 30);
        let mut gm = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        let mut ce = physical_copy(0, 1 << 30, 1 << 30);
        let bound = CE_SLICE + std::time::Duration::from_millis(10);
        // The copy's own time is this thread's CPU time: the slice ends by the clock, so a
        // slice never works longer than the bound, but a thread descheduled under parallel
        // load (the test harness) may take longer by the wall clock. The wall clock is only
        // checked against a loose sanity bound.
        let cpu = || {
            let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
            // SAFETY: a valid clock id and a local timespec.
            unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
            std::time::Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
        };
        let mut done = 0u64;
        for _ in 0..4 {
            let (t, c) = (std::time::Instant::now(), cpu());
            let r = Registry::launch_dma(&mut ce, &mut gm, root, PHYS_COPY, 1, None, CE_SLICE);
            let (took, worked) = (t.elapsed(), cpu() - c);
            assert!(worked <= bound, "one slice worked {worked:?}");
            assert!(took <= std::time::Duration::from_secs(1), "one slice took {took:?}");
            assert!(matches!(r, Ok(CeOutcome::Sliced(_))), "{r:?}");
            let p = ce.progress.expect("a sliced copy keeps its progress");
            let now = p.line * p.key.bytes_per_line + p.offset;
            assert!(now > done && now < 1 << 30, "progress {done} -> {now}");
            done = now;
        }
        std::fs::remove_file(path).ok();
    }

    /// A sliced copy and a sliced fill end exactly as a whole one: every byte once, the fill
    /// pattern continuing across slices (3-byte elements), the progress cleared at the end.
    #[test]
    fn sliced_transfers_complete_exactly() {
        let (mut fb, sys, path, root) = world("ce-slice-exact");
        let n = 3 * CE_COPY_CHUNK as u32 + 4099;
        let src: Vec<u8> = (0..n).map(|i| (i * 7 + i / 251) as u8).collect();
        assert!(fb.write(0x100_0000, &src));
        let mut gm = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        let zero = std::time::Duration::ZERO;
        let mut ce = physical_copy(0x100_0000, 0x200_0000, n);
        let mut slices = 0;
        loop {
            match Registry::launch_dma(&mut ce, &mut gm, root, PHYS_COPY, 1, None, zero).unwrap() {
                CeOutcome::Sliced(_) => slices += 1,
                CeOutcome::Done(note) => {
                    assert!(note.contains(&format!("memcopy {n} bytes")), "{note}");
                    break;
                }
            }
        }
        assert_eq!(slices, 3, "one chunk per slice with a zero budget");
        assert!(ce.progress.is_none());
        let mut got = vec![0u8; n as usize];
        assert!(gm.fb.read(0x200_0000, &mut got));
        assert!(got == src, "the sliced copy differs");
        // A constant-remap fill of 3-byte elements (COMPONENT_SIZE 3, one component: the
        // constant A), 2.5 chunks.
        let elements = (5 * CE_FILL_CHUNK as u32 / 2) / 3 + 1;
        let mut fill = CeState {
            offset_out: (0, 0x300_0000),
            line_length_in: elements,
            remap_const_a: 0x00C3_B2A1,
            remap_components: 0x0002_0004,
            ..Default::default()
        };
        // Physical destination, constant remap, pitch.
        let v = 0x2582;
        let mut fills = 0;
        while let CeOutcome::Sliced(_) =
            Registry::launch_dma(&mut fill, &mut gm, root, v, 1, None, zero).unwrap()
        {
            fills += 1;
        }
        assert_eq!(fills, 2);
        let mut b = vec![0u8; elements as usize * 3];
        assert!(gm.fb.read(0x300_0000, &mut b));
        assert!(
            b.chunks(3).all(|c| c == [0xA1, 0xB2, 0xC3]),
            "fill pattern broken"
        );
        std::fs::remove_file(path).ok();
    }

    /// Through the channel: a sliced LAUNCH_DMA holds the channel at that method (a blocked
    /// software channel, replayed by the held path); its semaphore is released and GPGet
    /// advances only when the last slice is done.
    #[test]
    fn a_sliced_launch_releases_its_semaphore_only_at_the_end() {
        let mut pb = physical_fill(0x30_0000, 3 * CE_FILL_CHUNK as u32);
        pb.extend(inc(0, 0x20, &[0]));
        let (mut fb, sys, path, mut reg) = software_fixture("ce-slice-chan", &pb);
        reg.software_fb_range = Some((0, 64 << 20));
        reg.ce_slice = std::time::Duration::ZERO;
        let mut runs = 0;
        loop {
            reg.doorbell(
                &mut GpuMem {
                    fb: &mut fb,
                    sys: &sys,
                },
                9,
            );
            runs += 1;
            if reg.blocked_software_channels().is_empty() {
                break;
            }
            assert_eq!(
                sys.read_u32(0x4000_1004),
                Some(0),
                "released before the end"
            );
            assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 0);
            assert!(runs < 64, "never finished");
        }
        assert!(runs > 1, "the fill was not sliced");
        assert!(ce_slices_pending());
        assert_eq!(sys.read_u32(0x4000_1004), Some(1));
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 1);
        assert_eq!(
            fb.read_u64(0x30_0000 + 12 * CE_FILL_CHUNK as u64 - 8)
                .unwrap() as u32,
            0xAABB_CCDD
        );
        std::fs::remove_file(path).ok();
    }

    /// A device-CE double (devce.rs `Engine`): a push completes after `delay` polls, when it
    /// runs its operations on the store behind the model's back, as the host engine would;
    /// with `fail` its first poll reports a failure and the engine stays off.
    struct TestEngine {
        window: crate::devce::Window,
        delay: u32,
        fail: bool,
        polls: u32,
        seq: u32,
        running: Option<(u32, Vec<crate::devce::Op>)>,
        submitted: usize,
        off: Option<String>,
        /// Whether a done push writes its bytes (off for the timing test's 1 GiB).
        execute: bool,
    }

    impl TestEngine {
        fn shared(
            len: u64,
            delay: u32,
            fail: bool,
        ) -> std::sync::Arc<std::sync::Mutex<TestEngine>> {
            std::sync::Arc::new(std::sync::Mutex::new(TestEngine {
                window: crate::devce::Window {
                    guest_base: 0,
                    len,
                    dva: 1 << 32,
                },
                delay,
                fail,
                polls: 0,
                seq: 0,
                running: None,
                submitted: 0,
                off: None,
                execute: true,
            }))
        }
    }

    impl crate::devce::Engine for TestEngine {
        fn window(&self) -> crate::devce::Window {
            self.window
        }
        fn submit(&mut self, _fb: &mut FakeFb, ops: &[crate::devce::Op]) -> Result<u32, String> {
            if let Some(w) = &self.off {
                return Err(w.clone());
            }
            if self.running.is_some() {
                return Err("device CE busy".into());
            }
            self.seq += 1;
            self.polls = 0;
            self.submitted += 1;
            self.running = Some((self.seq, ops.to_vec()));
            Ok(self.seq)
        }
        fn poll(&mut self, fb: &mut FakeFb, seq: u32) -> crate::devce::Poll {
            use crate::devce::{Op, Poll};
            if self.fail {
                self.running = None;
                self.off = Some("push timed out (test)".into());
                return Poll::Failed("push timed out (test)".into());
            }
            self.polls += 1;
            if self.polls < self.delay {
                return Poll::Pending;
            }
            let (s, ops) = self.running.take().expect("a push runs");
            assert_eq!(s, seq);
            if !self.execute {
                return Poll::Done;
            }
            let pa = |dva: u64| self.window.guest_base + (dva - self.window.dva);
            for op in ops {
                match op {
                    Op::Fill {
                        dst,
                        elements,
                        remap,
                    } => {
                        let comp = ((remap.components >> 16) & 3) as usize + 1;
                        let ncomp = ((remap.components >> 24) & 3) as usize + 1;
                        let mut elem = Vec::new();
                        for k in 0..ncomp {
                            let v = if (remap.components >> (4 * k)) & 7 == 5 {
                                remap.const_b
                            } else {
                                remap.const_a
                            };
                            elem.extend_from_slice(&v.to_le_bytes()[..comp]);
                        }
                        let bytes: Vec<u8> = elem
                            .iter()
                            .copied()
                            .cycle()
                            .take(elements as usize * elem.len())
                            .collect();
                        fb.write_behind(pa(dst), &bytes);
                    }
                    Op::Copy { src, dst, len } => {
                        let mut b = vec![0u8; len as usize];
                        assert!(fb.read(pa(src), &mut b));
                        fb.write_behind(pa(dst), &b);
                    }
                }
            }
            Poll::Done
        }
        fn off(&self) -> Option<String> {
            self.off.clone()
        }
    }

    fn fill_const(dst: u64, elements: u32, value: u32) -> Vec<u32> {
        let mut pb = physical_fill(dst, elements);
        pb[3] = value;
        pb
    }

    fn doorbell_until_done(fb: &mut FakeFb, sys: &GuestMemory, reg: &mut Registry) -> usize {
        let mut runs = 0;
        loop {
            reg.doorbell(&mut GpuMem { fb, sys }, 9);
            runs += 1;
            if reg.blocked_software_channels().is_empty() {
                return runs;
            }
            assert_eq!(
                sys.read_u32(0x4000_1004),
                Some(0),
                "released before the end"
            );
            assert!(runs < 256, "never finished");
        }
    }

    /// A large fill runs on the device CE: submitted once, waited for through the held
    /// replay, its bytes written by the engine, the semaphore released and GPGet advanced
    /// only after the engine is done.
    #[test]
    fn a_large_fill_runs_on_the_device_ce_and_completes_after_it() {
        let mut pb = physical_fill(0x30_0000, 1 << 18);
        pb.extend(inc(0, 0x20, &[0]));
        let (mut fb, sys, path, mut reg) = software_fixture("devce-fill", &pb);
        reg.software_fb_range = Some((0, 64 << 20));
        let engine = TestEngine::shared(48 << 20, 4, false);
        fb.set_device_ce(Some(engine.clone()));
        let runs = doorbell_until_done(&mut fb, &sys, &mut reg);
        assert_eq!(runs, 5, "one submit, three pending polls, the done poll");
        assert_eq!(engine.lock().unwrap().submitted, 1);
        assert_eq!(sys.read_u32(0x4000_1004), Some(1));
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 1);
        assert_eq!(
            fb.read_u64(0x30_0000 + (1 << 20) - 8).unwrap() as u32,
            0xAABB_CCDD
        );
        std::fs::remove_file(path).ok();
    }

    /// Condition 3 of the review: a page-table page inside a large device-CE fill. The
    /// cached walk of that table sees the fill (its leaves are gone), as it would after a
    /// CPU write.
    #[test]
    fn a_device_ce_fill_over_a_page_table_drops_the_cached_walk() {
        let mut pb = fill_const(0x80_0000, 1 << 16, 0);
        pb.extend(inc(0, 0x20, &[0]));
        let (mut fb, sys, path, mut reg) = software_fixture("devce-walk", &pb);
        reg.software_fb_range = Some((0, 64 << 20));
        // A second tree at 0x80_0000 whose PT page 0x80_4000 maps 8 VRAM pages.
        let (root, pd2, pd1, pd0, pt) = (0x80_0000u64, 0x80_1000, 0x80_2000, 0x80_3000, 0x80_4000);
        let vid = |a: u64| ((a >> 12) << 8) | 0b010;
        fb.write_u64(root, vid(pd2));
        fb.write_u64(pd2, vid(pd1));
        fb.write_u64(pd1, vid(pd0));
        fb.write_u64(pd0 + 16 * 0x80 + 8, vid(pt));
        for k in 0..8u64 {
            fb.write_u64(pt + 8 * k, (((0x100000 + k * 0x1000) >> 12) << 8) | 1);
        }
        fb.walk_update(None, Target::Fb(root), false, 1 << 16);
        let leaves = |fb: &FakeFb| fb.with_walk(Target::Fb(root), |c| c.leaf_count()).unwrap();
        assert_eq!(leaves(&fb), 8);
        fb.set_device_ce(Some(TestEngine::shared(48 << 20, 2, false)));
        doorbell_until_done(&mut fb, &sys, &mut reg);
        assert_eq!(fb.read_u64(pt).unwrap(), 0, "the engine cleared the table");
        fb.walk_update(None, Target::Fb(root), false, 1 << 16);
        assert_eq!(leaves(&fb), 0, "the cached walk re-read the cleared table");
        std::fs::remove_file(path).ok();
    }

    /// Condition 5: a device CE that fails leaves the transfer to the CPU, with one note,
    /// and no RC of the guest channel; the engine stays off for later transfers.
    #[test]
    fn a_failed_device_ce_falls_back_to_the_cpu_without_an_rc() {
        let mut pb = physical_fill(0x30_0000, 1 << 18);
        pb.extend(inc(0, 0x20, &[0]));
        let (mut fb, sys, path, mut reg) = software_fixture("devce-fail", &pb);
        reg.software_fb_range = Some((0, 64 << 20));
        let engine = TestEngine::shared(48 << 20, 1, true);
        fb.set_device_ce(Some(engine.clone()));
        doorbell_until_done(&mut fb, &sys, &mut reg);
        assert_eq!(sys.read_u32(0x4000_1004), Some(1));
        assert_eq!(
            fb.read_u64(0x30_0000 + (1 << 20) - 8).unwrap() as u32,
            0xAABB_CCDD
        );
        let events = reg.take_software_events();
        assert!(
            events.iter().all(|e| e.failure.is_none()),
            "no RC for a device-internal failure"
        );
        assert!(engine.lock().unwrap().off.is_some());
        std::fs::remove_file(path).ok();
    }

    /// Condition 2: with the device CE, no call of the software CE holds the loop longer
    /// than the slice bound, even for a 1 GiB fill the engine takes long to finish.
    #[test]
    fn a_device_ce_transfer_never_holds_the_loop_longer_than_the_slice_bound() {
        let (_, sys, path, root) = world("devce-bound");
        let mut fb = FakeFb::new(2 << 30);
        let engine = TestEngine::shared(2 << 30, 20, false);
        engine.lock().unwrap().execute = false;
        fb.set_device_ce(Some(engine.clone()));
        let mut gm = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        let mut ce = CeState {
            offset_out: (0, 0x1000_0000),
            line_length_in: 1 << 28,
            remap_const_a: 0x1234_5678,
            remap_components: 0x0003_0004,
            ..Default::default()
        };
        let bound = CE_SLICE + std::time::Duration::from_millis(10);
        let mut calls = 0;
        loop {
            let t = std::time::Instant::now();
            let r =
                Registry::launch_dma(&mut ce, &mut gm, root, 0x2582, 1, None, CE_SLICE).unwrap();
            assert!(t.elapsed() <= bound, "a call took {:?}", t.elapsed());
            calls += 1;
            if let CeOutcome::Done(note) = r {
                assert!(note.contains("device CE push 1"), "{note}");
                break;
            }
            assert!(calls < 64);
        }
        assert_eq!(calls, 21);
        assert_eq!(engine.lock().unwrap().submitted, 1);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn physical_ce_then_legacy_release_and_nonstall_complete_in_order() {
        let mut pb = physical_fill(0x30_0000, 1);
        pb.extend(inc(0, 0x10, &[0, 0x1000_9000, 1, 0x0110_0002]));
        pb.extend(inc(0, 0x20, &[0]));
        let (mut fb, sys, path, mut reg) = software_fixture("ce-legacy-complete", &pb);
        reg.software_fb_range = Some((0, 64 << 20));
        assert!(reg.channel(9).unwrap().privileged());
        let notes = reg
            .doorbell(
                &mut GpuMem {
                    fb: &mut fb,
                    sys: &sys,
                },
                9,
            )
            .join("\n");
        assert!(!notes.contains("not modelled"), "{notes}");
        assert_eq!(fb.read_u64(0x30_0000).unwrap() as u32, 0xAABB_CCDD);
        assert_eq!(sys.read_u32(0x4000_1004), Some(1));
        assert_eq!(sys.read_u32(0x4000_1000), Some(1));
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 1);
        let events = reg.take_software_events();
        assert_eq!(events.len(), 1);
        assert!(events[0].nonstall && events[0].failure.is_none());
        std::fs::remove_file(path).ok();
    }

    /// A software CE fill into the device's private pages (an owned-GFXP slot of the VM's
    /// slice, inside the software FB range) is refused as outside VM backing, like any other
    /// range the guest does not own; nothing is written there.
    #[test]
    fn a_software_ce_fill_into_a_gfxp_slot_is_refused() {
        let mut pb = physical_fill(0x30_0000, 1);
        pb.extend(inc(0, 0x10, &[0, 0x1000_9000, 99, 0x0110_0002]));
        pb.extend(inc(0, 0x20, &[0]));
        let (mut fb, sys, path, mut reg) = software_fixture("ce-gfxp-slot", &pb);
        reg.software_fb_range = Some((0, 0x100_0000));
        fb.set_private(vec![(0x30_0000, 0x40_0000)]);
        fb.write_u64(0x30_0000, 0x5a5a);
        reg.doorbell(
            &mut GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        );
        let events = reg.take_software_events();
        assert_eq!(events.len(), 1);
        assert!(events[0]
            .failure
            .as_ref()
            .unwrap()
            .contains("outside VM backing"));
        assert_eq!(fb.read_u64(0x30_0000), Some(0x5a5a));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn failed_physical_ce_never_releases_semaphore_or_advances_gpget() {
        let mut pb = physical_fill(0x30_0000, 1);
        pb.extend(inc(0, 0x10, &[0, 0x1000_9000, 99, 0x0110_0002]));
        pb.extend(inc(0, 0x20, &[0]));
        let (mut fb, sys, path, mut reg) = software_fixture("ce-failed-range", &pb);
        // Tables/USERD are valid, but the destination is outside the VM reservation.
        reg.software_fb_range = Some((0, 0x28_0000));
        reg.doorbell(
            &mut GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        );
        assert_eq!(sys.read_u32(0x4000_1004), Some(0));
        assert_eq!(sys.read_u32(0x4000_1000), Some(0));
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 0);
        let events = reg.take_software_events();
        assert_eq!(events.len(), 1);
        assert!(events[0]
            .failure
            .as_ref()
            .unwrap()
            .contains("outside VM backing"));
        assert!(!events[0].nonstall);
        reg.doorbell(
            &mut GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        );
        assert!(
            reg.take_software_events().is_empty(),
            "terminal failure must not repeat writes/events"
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn ce_refuses_overflow_budget_and_unknown_physical_kind_before_completion() {
        let (mut fb, sys, path, root) = world("ce-bounds");
        let mut gm = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        let mut ce = CeState {
            offset_out: (u32::MAX, u32::MAX),
            line_length_in: 1,
            remap_components: 0x0003_0004,
            sem_lower: 0x1000_9000,
            sem_payload: 7,
            ..Default::default()
        };
        assert!(
            Registry::launch_dma(&mut ce, &mut gm, root, 0x0400_258e, 1, None, CE_SLICE)
                .unwrap_err()
                .contains("overflow")
        );
        ce.offset_out = (0, 0x30_0000);
        ce.line_length_in = u32::MAX;
        assert!(
            Registry::launch_dma(&mut ce, &mut gm, root, 0x0400_258e, 1, None, CE_SLICE)
                .unwrap_err()
                .contains("budget")
        );
        // Zero rows must not hide an oversized per-row staging allocation.
        ce.line_count = 0;
        assert!(
            Registry::launch_dma(&mut ce, &mut gm, root, 0x0400_278e, 1, None, CE_SLICE)
                .unwrap_err()
                .contains("budget")
        );
        ce.line_length_in = 1;
        ce.dst_phys = 3;
        assert!(
            Registry::launch_dma(&mut ce, &mut gm, root, 0x0400_258e, 1, None, CE_SLICE)
                .unwrap_err()
                .contains("mode/kind")
        );
        assert_eq!(sys.read_u32(0x4000_1000), Some(0));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn legacy_acquire_blocks_and_resumes_without_repeating_earlier_work() {
        let pb = inc(0, 0x10, &[0, 0x1000_9000, 7, 1]);
        let (mut fb, sys, path, mut reg) = software_fixture("ce-acquire-resume", &pb);
        reg.sem_diag = true;
        reg.sem_diag_chid = Some(9);
        let first = reg.doorbell(
            &mut GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        );
        let diag = first
            .iter()
            .find(|n| n.starts_with("SEMAPHORE DIAG chid"))
            .expect("first blocked tuple has a diagnostic");
        assert!(diag.contains("raw[0..5]"), "{diag}");
        assert!(diag.contains("method 0x1c data 0x1"), "{diag}");
        assert!(diag.contains("Sys(1073745912)"), "{diag}");
        assert!(first.iter().any(|n| n.starts_with("SEMAPHORE DIAG peers:")));
        assert_eq!(reg.blocked_software_channels(), vec![9]);
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 0);
        let retry = reg.doorbell(
            &mut GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        );
        assert!(
            retry.iter().all(|n| !n.starts_with("SEMAPHORE DIAG")),
            "same blocked tuple must not repeat diagnostics: {retry:?}"
        );
        assert!(sys.write(0x4000_1000, &7u32.to_le_bytes()));
        reg.doorbell(
            &mut GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        );
        assert!(reg.blocked_software_channels().is_empty());
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 1);
        std::fs::remove_file(path).ok();
    }

    /// A blocked acquire is kept on the channel (`AcquireWait`) and re-read without a
    /// doorbell: not ready while the semaphore lags, ready once the release lands, and the
    /// wait is gone after the channel's next run (C79: the Windows KMD's channel waited for
    /// the next unrelated doorbell, up to 401 ms).
    #[test]
    fn a_blocked_acquire_is_rechecked_without_a_doorbell() {
        let pb = inc(0, 0x10, &[0, 0x1000_9000, 7, 4]);
        let (mut fb, sys, path, mut reg) = software_fixture("ce-acquire-recheck", &pb);
        reg.doorbell(&mut GpuMem { fb: &mut fb, sys: &sys }, 9);
        assert_eq!(reg.blocked_software_channels(), vec![9]);
        assert_eq!(
            reg.channel(9).and_then(|c| c.acquire_wait).map(|w| (w.va, w.payload, w.op)),
            Some((0x1000_9000, 7, AcquireOp::Geq))
        );
        assert!(acquire_waits_pending());
        assert!(reg.ready_acquires(&GpuMem { fb: &mut fb, sys: &sys }).is_empty());
        assert!(acquire_waits_pending(), "still waiting: the flag stays for the next tick");
        assert!(sys.write(0x4000_1000, &9u32.to_le_bytes()));
        assert_eq!(reg.ready_acquires(&GpuMem { fb: &mut fb, sys: &sys }), vec![9]);
        reg.doorbell(&mut GpuMem { fb: &mut fb, sys: &sys }, 9);
        assert!(reg.blocked_software_channels().is_empty());
        assert_eq!(reg.channel(9).and_then(|c| c.acquire_wait), None);
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 1);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn deferred_management_yields_then_resumes_exactly_once() {
        let mut pb = inc(5, 0, &[0x5080]);
        pb.extend(inc(5, 0x200, &[0x4000_0003]));
        pb.extend(inc(0, 0x10, &[0, 0x1000_9000, 9, 0x0110_0002]));
        let (mut fb, sys, path, mut reg) = software_fixture("deferred-management", &pb);
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[]).unwrap();
        reg.enable_deferred_management(9).unwrap();

        let first = reg.doorbell(
            &mut GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        );
        assert!(first.iter().any(|n| n.contains("deferred object 0x77")));
        assert_eq!(reg.pending_deferred(), vec![(9, 0x77, 0x4000_0003)]);
        assert_eq!(sys.read_u32(0x4000_1000), Some(0));
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 0);

        reg.complete_deferred(9, 0x4000_0003, Ok(())).unwrap();
        assert!(reg.pending_deferred().is_empty());
        assert!(reg.complete_deferred(9, 0x4000_0003, Ok(())).is_err());
        let second = reg.doorbell(
            &mut GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        );
        assert!(second.iter().any(|n| n.contains("completed object 0x77")));
        assert_eq!(sys.read_u32(0x4000_1000), Some(9));
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 1);
        assert!(reg.pending_deferred().is_empty());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn management_takeover_recovers_only_bounded_set_object_prefix() {
        let mut pb = inc(5, 0, &[0x5080]);
        pb.extend(inc(5, 8, &[0]));
        let (mut fb, sys, path, mut reg) = software_fixture("management-takeover", &pb);
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[]).unwrap();
        reg.channel_mut(9).unwrap().mirrored = true;
        let plan = reg
            .preview_deferred_takeover(
                &GpuMem {
                    fb: &mut fb,
                    sys: &sys,
                },
                9,
                1,
            )
            .unwrap();
        assert_eq!(reg.channel(9).unwrap().objects[5], 0);
        assert!(reg.channel(9).unwrap().mirrored);
        assert_eq!(reg.channel(9).unwrap().gp_get, 0);
        reg.apply_deferred_takeover(plan).unwrap();
        assert_eq!(reg.channel(9).unwrap().objects[5], 0x5080);
        assert!(!reg.channel(9).unwrap().mirrored);
        assert_eq!(reg.channel(9).unwrap().gp_get, 1);
        assert!(reg
            .preview_deferred_takeover(
                &GpuMem {
                    fb: &mut fb,
                    sys: &sys,
                },
                9,
                33,
            )
            .is_err());
        std::fs::remove_file(path).ok();

        let pb = inc(0, 0x10, &[1]);
        let (mut fb, sys, path, mut reg) = software_fixture("management-bad-prefix", &pb);
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[]).unwrap();
        reg.channel_mut(9).unwrap().mirrored = true;
        let error = reg
            .preview_deferred_takeover(
                &GpuMem {
                    fb: &mut fb,
                    sys: &sys,
                },
                9,
                1,
            )
            .unwrap_err();
        assert!(error.contains("not SET_OBJECT/NOP"), "{error}");
        assert!(error.contains("entry 0/1"), "{error}");
        assert!(error.contains("bound class 0x0"), "{error}");
        assert!(error.contains("method 0x10, data 0x1"), "{error}");
        assert!(error.contains("entry 2 dwords, raw[0..2]"), "{error}");
        assert!(reg.channel(9).unwrap().mirrored);
        assert_eq!(reg.channel(9).unwrap().gp_get, 0);
        std::fs::remove_file(path).ok();

        let pb = inc(5, 0, &[0x5080]);
        let (mut fb, sys, path, mut reg) = software_fixture("management-stale-plan", &pb);
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[]).unwrap();
        reg.channel_mut(9).unwrap().mirrored = true;
        let plan = reg
            .preview_deferred_takeover(
                &GpuMem {
                    fb: &mut fb,
                    sys: &sys,
                },
                9,
                1,
            )
            .unwrap();
        reg.record_free(1, 0x40);
        reg.record_alloc(
            1,
            0x10,
            0x40,
            TURING_CHANNEL_GPFIFO_A,
            &channel_params(9, 0x1000_0000, 0x20_0000, 0x20),
        );
        reg.channel_mut(9).unwrap().mirrored = true;
        assert!(reg.apply_deferred_takeover(plan).is_err());
        assert_eq!(reg.channel(9).unwrap().objects[5], 0);
        assert_eq!(reg.channel(9).unwrap().gp_get, 0);
        std::fs::remove_file(path).ok();

        let pb = inc(5, 0, &[1]);
        let (mut fb, sys, path, mut reg) = software_fixture("management-unknown-history", &pb);
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[]).unwrap();
        reg.record_alloc(1, 0x40, 0x78, 0xdead, &[]).unwrap();
        reg.record_free(1, 0x78);
        reg.channel_mut(9).unwrap().mirrored = true;
        let before = reg.channel(9).unwrap().objects;
        let error = reg
            .preview_deferred_takeover(
                &GpuMem {
                    fb: &mut fb,
                    sys: &sys,
                },
                9,
                1,
            )
            .unwrap_err();
        assert!(error.contains("ID sequence is not modelled"), "{error}");
        let ch = reg.channel(9).unwrap();
        assert!(ch.mirrored);
        assert_eq!(ch.gp_get, 0);
        assert_eq!(ch.objects, before);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn deferred_target_retirement_records_only_valid_harvested_position() {
        let (mut fb, sys, path, mut reg) =
            software_fixture("deferred-target-retire", &inc(0, 8, &[0]));
        let generation = reg.channel(9).unwrap().generation;
        reg.channel_mut(9).unwrap().mirrored = true;
        let before = {
            let ch = reg.channel(9).unwrap();
            (ch.gp_get, ch.gp_put_seen, ch.mirrored)
        };
        assert!(reg.retire_deferred_target(9, generation + 1, 1).is_err());
        assert_eq!(
            {
                let ch = reg.channel(9).unwrap();
                (ch.gp_get, ch.gp_put_seen, ch.mirrored)
            },
            before
        );
        assert!(reg.retire_deferred_target(9, generation, 32).is_err());
        assert_eq!(
            {
                let ch = reg.channel(9).unwrap();
                (ch.gp_get, ch.gp_put_seen, ch.mirrored)
            },
            before
        );

        reg.retire_deferred_target(9, generation, 1).unwrap();
        let ch = reg.channel(9).unwrap();
        assert_eq!((ch.gp_get, ch.gp_put_seen, ch.mirrored), (1, 1, false));
        assert!(!reg.pending(
            &GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        ));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn captured_management_prefix_is_pure_state_and_resolves_software_id() {
        let words = [
            0x20014000, 0x0000a140, 0x20016000, 0x0000902d, 0x20016222, 0x00000001, 0x200160a4,
            0x00000000, 0x200160a7, 0x00000000, 0x200160ab, 0x00000003, 0x20016201, 0x000000cf,
            0x20046210, 0x00000000, 0x00000001, 0x00000000, 0x00000001, 0x20046214, 0x00000000,
            0x00000000, 0x00000000, 0x00000000, 0x20046230, 0x00000000, 0x00000001, 0x00000000,
            0x00000001, 0x20018000, 0x0000c5b5, 0x20010000, 0x0000c597, 0x20020041, 0x00000001,
            0x20233600, 0x20012000, 0x0000c5c0, 0x20022041, 0x00000001, 0x20233600, 0x2001a000,
            0x00000001,
        ];
        let (mut fb, sys, path, mut reg) = software_fixture("captured-management-prefix", &words);
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[]).unwrap();
        reg.channel_mut(9).unwrap().mirrored = true;
        let plan = reg
            .preview_deferred_takeover(
                &GpuMem {
                    fb: &mut fb,
                    sys: &sys,
                },
                9,
                1,
            )
            .unwrap();
        reg.apply_deferred_takeover(plan).unwrap();
        let ch = reg.channel(9).unwrap();
        assert_eq!(ch.objects[3], 0x902d);
        assert_eq!(ch.objects[2], 0xa140);
        assert_eq!(ch.objects[1], 0xc5c0);
        assert_eq!(ch.objects[5], 1);
        assert_eq!(ch.bound_class(1), 0x5080);
        assert_eq!(ch.software_objects.get(&1), Some(&(0x77, 0x5080)));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn captured_deferred_sequence_publishes_report_only_after_both_completions() {
        let sequence = || {
            let mut pb = inc(0, 0x10, &[0, 0x1000_9000, 2, 0x1001]);
            pb.extend(inc(2, 0, &[0xa140]));
            pb.extend(inc(0, 0x50, &[0]));
            pb.extend(inc(5, 0x200, &[0x4000_0002]));
            pb.extend(inc(0, 0x50, &[0]));
            pb.extend(inc(5, 0x200, &[0x4000_0003]));
            pb.extend(inc(0, 0x1b00, &[0, 0x1000_9000, 3, 0x1000_f010]));
            pb
        };
        let prepare = |tag: &str| {
            let (fb, sys, path, mut reg) = software_fixture(tag, &sequence());
            reg.record_alloc(1, 0x40, 0x77, 0x5080, &[]).unwrap();
            let ch = reg.channel_mut(9).unwrap();
            ch.objects[0] = 0xc597;
            ch.objects[5] = 1;
            reg.enable_deferred_management(9).unwrap();
            (fb, sys, path, reg)
        };
        let (mut fb, sys, path, mut reg) = prepare("captured-deferred-sequence");
        assert!(sys.write(0x4000_1000, &2u32.to_le_bytes()));
        fb.write(0x20_0000 + 0x48, &0x55u32.to_le_bytes());
        let ring = |reg: &mut Registry, fb: &mut FakeFb, sys: &GuestMemory| {
            reg.run_deferred_one(&mut GpuMem { fb, sys }, 9)
        };
        ring(&mut reg, &mut fb, &sys);
        assert_eq!(reg.pending_deferred(), vec![(9, 0x77, 0x4000_0002)]);
        assert_eq!(sys.read_u32(0x4000_1000), Some(2));
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 0);
        assert_eq!(fb.read_u64(0x20_0000 + 0x48).unwrap() as u32, 0);
        reg.complete_deferred(9, 0x4000_0002, Ok(())).unwrap();
        ring(&mut reg, &mut fb, &sys);
        assert_eq!(reg.pending_deferred(), vec![(9, 0x77, 0x4000_0003)]);
        assert_eq!(sys.read_u32(0x4000_1000), Some(2));
        reg.complete_deferred(9, 0x4000_0003, Ok(())).unwrap();
        ring(&mut reg, &mut fb, &sys);
        assert_eq!(sys.read_u32(0x4000_1000), Some(3));
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 1);
        std::fs::remove_file(path).ok();

        let (mut fb, sys, path, mut reg) = prepare("captured-deferred-failure");
        assert!(sys.write(0x4000_1000, &2u32.to_le_bytes()));
        ring(&mut reg, &mut fb, &sys);
        reg.complete_deferred(9, 0x4000_0002, Err("refused".into()))
            .unwrap();
        ring(&mut reg, &mut fb, &sys);
        assert_eq!(sys.read_u32(0x4000_1000), Some(2));
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 0);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn strict_report_rejects_uncaptured_d_without_publishing() {
        let pb = inc(0, 0x1b00, &[0, 0x1000_9000, 3, 0x1000_f011]);
        let (mut fb, sys, path, mut reg) = software_fixture("strict-report-d", &pb);
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[]).unwrap();
        let ch = reg.channel_mut(9).unwrap();
        ch.objects[0] = 0xc597;
        reg.enable_deferred_management(9).unwrap();
        assert!(sys.write(0x4000_1000, &2u32.to_le_bytes()));
        reg.run_deferred_one(
            &mut GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        );
        assert_eq!(sys.read_u32(0x4000_1000), Some(2));
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 0);
        assert!(reg.take_software_events()[0]
            .failure
            .as_deref()
            .unwrap()
            .contains("not the captured release"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn strict_management_rejects_unmodelled_work_and_failed_completion_is_fatal() {
        let mut pb = inc(5, 0, &[0x5080]);
        pb.extend(inc(5, 0x200, &[7]));
        let (mut fb, sys, path, mut reg) = software_fixture("deferred-failure", &pb);
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[]).unwrap();
        reg.enable_deferred_management(9).unwrap();
        reg.doorbell(
            &mut GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        );
        reg.complete_deferred(9, 7, Err("host refused".into()))
            .unwrap();
        let events = reg.take_software_events();
        assert_eq!(events.len(), 1);
        assert!(events[0]
            .failure
            .as_deref()
            .unwrap()
            .contains("host refused"));
        assert!(reg.complete_deferred(9, 7, Ok(())).is_err());
        std::fs::remove_file(path).ok();

        let pb = inc(5, 0, &[0xc597])
            .into_iter()
            .chain(inc(5, 0x100, &[1]))
            .collect::<Vec<_>>();
        let (mut fb, sys, path, mut reg) = software_fixture("strict-unmodelled", &pb);
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[]).unwrap();
        reg.enable_deferred_management(9).unwrap();
        reg.doorbell(
            &mut GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        );
        let events = reg.take_software_events();
        assert_eq!(events.len(), 1);
        assert!(events[0]
            .failure
            .as_deref()
            .unwrap()
            .contains("strict C597 method 0x100"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn semaphore_diagnostic_tuple_cap_does_not_change_the_acquire() {
        let pb = inc(0, 0x10, &[0, 0x1000_9000, 7, 1]);
        let (mut fb, sys, path, mut reg) = software_fixture("sem-diag-cap", &pb);
        reg.sem_diag = true;
        reg.sem_diag_chid = Some(9);
        reg.channel_mut(9)
            .unwrap()
            .sem_diag_seen
            .extend((0..SEM_DIAG_TUPLES).map(|n| (n as u64, n as u32)));

        let notes = reg.doorbell(
            &mut GpuMem {
                fb: &mut fb,
                sys: &sys,
            },
            9,
        );
        assert!(notes.iter().all(|n| !n.starts_with("SEMAPHORE DIAG")));
        assert_eq!(reg.channel(9).unwrap().sem_diag_seen.len(), SEM_DIAG_TUPLES);
        assert_eq!(reg.blocked_software_channels(), vec![9]);
        assert_eq!(fb.read_u64(0x20_0000 + USERD_GP_GET).unwrap() as u32, 0);
        std::fs::remove_file(path).ok();
    }

    /// A software run stops between two GPFIFO entries once its budget is spent
    /// ([`RUN_BUDGET`], execution.md §9.7): the entries before ran with their semaphore
    /// release, GPGet stands after them, the channel waits for the slice clock, and the next
    /// run continues at the same GPGet with the rest, in order.
    #[test]
    fn a_run_past_its_budget_stops_between_entries_and_continues_in_order() {
        let (mut fb, sys, _path, _root) = world("budget");
        let mut reg = Registry::new(classes());
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        let mut pdes = vec![0u8; 40 + 24 * 6];
        pdes[40..48].copy_from_slice(&0x10000u64.to_le_bytes());
        pdes[56..60].copy_from_slice(&1u32.to_le_bytes());
        reg.record_server_reserved_pdes(1, 0x20, &pdes);
        let mut tsg = vec![0u8; 20];
        tsg[8..12].copy_from_slice(&0x20u32.to_le_bytes());
        reg.record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &tsg);
        reg.record_alloc(
            1,
            0x30,
            0x40,
            TURING_CHANNEL_GPFIFO_A,
            &channel_params(9, 0x1000_0000, 0x20_0000, 0),
        )
        .unwrap();
        // Two pushbuffers in guest RAM (VA pages 8 and 9): host semaphore releases of 1
        // and 2 into VA page 10 (GPA 0x4000_2000).
        let push = |payload: u32| -> Vec<u8> {
            inc(0, 0x5C, &[0x1000_A000, 0x0, payload, 0, 1])
                .iter()
                .flat_map(|d| d.to_le_bytes())
                .collect()
        };
        let (p1, p2) = (push(1), push(2));
        assert!(sys.write(0x4000_0000, &p1));
        assert!(sys.write(0x4000_1000, &p2));
        let entry = |va: u32, len: usize| -> Vec<u8> {
            let mut e = va.to_le_bytes().to_vec();
            e.extend_from_slice(&(((len / 4) as u32) << 10).to_le_bytes());
            e
        };
        let mut gp = entry(0x1000_8000, p1.len());
        gp.extend(entry(0x1000_9000, p2.len()));
        fb.write(0x100000, &gp);
        fb.write(0x20_0000 + USERD_GP_PUT, &2u32.to_le_bytes());
        let mut mem = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        reg.run_budget = std::time::Duration::ZERO;
        let all = reg.doorbell(&mut mem, (1 << 16) | 9).join("\n");
        assert!(all.contains("run stopped at gp 1"), "{all}");
        assert_eq!(sys.read_u32(0x4000_2000), Some(1), "the first entry's release");
        assert_eq!(mem.read_u32(Target::Fb(0x20_0000 + USERD_GP_GET)), Some(1));
        // Held for the slice clock (its flag is process-wide, so other tests share it).
        assert_eq!(reg.blocked_software_channels(), vec![9]);
        // The next run (the slice clock's held replay) continues with the second entry.
        reg.run_budget = std::time::Duration::from_secs(1);
        reg.doorbell(&mut mem, (1 << 16) | 9);
        assert_eq!(sys.read_u32(0x4000_2000), Some(2), "the second release, after the first");
        assert_eq!(mem.read_u32(Target::Fb(0x20_0000 + USERD_GP_GET)), Some(2));
        assert!(reg.blocked_software_channels().is_empty());
        assert_eq!(reg.channels().next().unwrap().entries_done, 2);
    }

    #[test]
    fn uvm_style_pushes_execute() {
        let (mut fb, sys, path, _root) = world("channel");
        let mut reg = Registry::new(classes());
        // A VAS whose root arrives by COPY_SERVER_RESERVED_PDES, a TSG, a channel.
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        let mut pdes = vec![0u8; 40 + 24 * 6];
        pdes[40..48].copy_from_slice(&0x10000u64.to_le_bytes());
        pdes[56..60].copy_from_slice(&1u32.to_le_bytes());
        reg.record_server_reserved_pdes(1, 0x20, &pdes);
        let mut tsg = vec![0u8; 20]; // sizeof NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS
        tsg[8..12].copy_from_slice(&0x20u32.to_le_bytes());
        reg.record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &tsg);
        // GPFIFO at VA page 0 (FB 0x100000), USERD at FB 0x200000, chid 9.
        let note = reg
            .record_alloc(
                1,
                0x30,
                0x40,
                TURING_CHANNEL_GPFIFO_A,
                &channel_params(9, 0x1000_0000, 0x20_0000, 0),
            )
            .unwrap();
        assert!(note.contains("chid 9"), "{note}");
        assert_eq!(reg.channels().next().unwrap().vas, Some((1, 0x20)));

        // Pushbuffer in guest RAM at VA page 8 (GPA 0x4000_0000): what
        // channel_init and uvm_page_tree_init push.
        let sem_va = 0x1000_9000u64; // VA page 9 = GPA 0x4000_1000
        let mut pb = Vec::new();
        pb.extend(inc(0, 0x0, &[TURING_DMA_COPY_A]));
        pb.extend(inc(5, 0x0, &[GP100_UVM_SW]));
        // memset_8 of 64 bytes at FB 0x300000 by physical address.
        pb.extend(inc(4, 0x700, &[0xAABB_CCDD, 0x1122_3344, 0x0103_0054]));
        pb.extend(inc(4, 0x264, &[0]));
        pb.extend(inc(4, 0x408, &[0, 0x30_0000]));
        pb.extend(inc(4, 0x418, &[8]));
        pb.extend(inc(4, 0x300, &[(1 << 13) | (1 << 10) | 2]));
        // Host WFI + membar, then the CE tracking semaphore release.
        pb.extend(inc(0, 0x78, &[0]));
        pb.extend(inc(0, 0x28, &[0, 0, 0, 5 << 27]));
        pb.extend(inc(4, 0x240, &[(sem_va >> 32) as u32, sem_va as u32, 7]));
        pb.extend(inc(4, 0x300, &[(1 << 3) | 4]));
        // A copy from the semaphore page to VA page 1 (FB 0x101000), 4 bytes.
        pb.extend(inc(4, 0x400, &[0, sem_va as u32, 0, 0x1000_1000]));
        pb.extend(inc(4, 0x418, &[4]));
        pb.extend(inc(4, 0x300, &[2]));
        // Host semaphore release with timestamp into VA page 10.
        pb.extend(inc(0, 0x5C, &[0x1000_A000, 0x0, 0x55, 0, 1 | (1 << 25)]));
        let bytes: Vec<u8> = pb.iter().flat_map(|d| d.to_le_bytes()).collect();
        assert!(sys.write(0x4000_0000, &bytes));
        // GPFIFO entry 0 → VA 0x1000_8000, length in dwords; entry 1 a NOP control entry.
        let e0 = 0x1000_8000u32;
        let e1 = (pb.len() as u32) << 10;
        let mut gp = e0.to_le_bytes().to_vec();
        gp.extend_from_slice(&e1.to_le_bytes());
        gp.extend_from_slice(&0u64.to_le_bytes());
        fb.write(0x100000, &gp);
        fb.write(0x20_0000 + USERD_GP_PUT, &2u32.to_le_bytes());

        let mut mem = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        let notes = reg.doorbell(&mut mem, (1 << 16) | 9);
        let all = notes.join("\n");
        assert!(all.contains("memset 64 bytes -> Fb(3145728)"), "{all}");
        assert!(
            all.contains("ce semaphore release va 0x10009000 = 0x7"),
            "{all}"
        );
        let mut q = [0u8; 8];
        mem.fb.read(0x30_0000 + 56, &mut q);
        assert_eq!(u64::from_le_bytes(q), 0x1122_3344_AABB_CCDD);
        assert_eq!(sys.read_u32(0x4000_1000), Some(7));
        assert_eq!(mem.read_u32(Target::Fb(0x101000)), Some(7));
        assert_eq!(sys.read_u32(0x4000_2000), Some(0x55));
        assert_ne!(sys.read_u64(0x4000_2008), Some(0));
        assert_eq!(mem.read_u32(Target::Fb(0x20_0000 + USERD_GP_GET)), Some(2));
        let ch = reg.channels().next().unwrap();
        assert_eq!(ch.entries_done, 2);
        assert_eq!(ch.objects[0], TURING_DMA_COPY_A);
        assert_eq!(ch.objects[5], GP100_UVM_SW);

        // A second doorbell with nothing new does nothing; an unknown chid is reported.
        assert_eq!(reg.doorbell(&mut mem, 9).len(), 1);
        assert!(reg.doorbell(&mut mem, 77)[1].contains("no such channel"));
        let _ = std::fs::remove_file(&path);
    }

    /// The post-mortem of a host graphics exception (ctrl.rs `gr_exception_postmortem`,
    /// the SotTR "2D KIND Violation" of 2026-09-28): a blit of the channel's last entries is
    /// reported with both surfaces' layout, VA and the guest PTE's kind.
    #[test]
    fn a_gr_exception_postmortem_names_the_2d_surfaces_and_their_kinds() {
        let (mut fb, sys, path, _root) = world("grpm");
        // The destination page VA 0x1000_2000 has kind GENERIC_16BX2 (0x06).
        fb.write_u64(0x14000 + 8 * 2, ((0x102000u64 >> 12) << 8) | 1 | (0x06 << 56));
        let f = crate::fixture::rtx2070();
        let mut d = crate::ctrl::Dispatcher::new(f.chip.data(), &f.identity(4 << 30), None);
        let reg = &mut d.channels;
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        let mut pdes = vec![0u8; 40 + 24 * 6];
        pdes[40..48].copy_from_slice(&0x10000u64.to_le_bytes());
        pdes[56..60].copy_from_slice(&1u32.to_le_bytes());
        reg.record_server_reserved_pdes(1, 0x20, &pdes);
        let mut tsg = vec![0u8; 20];
        tsg[8..12].copy_from_slice(&0x20u32.to_le_bytes());
        reg.record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &tsg);
        reg.record_alloc(1, 0x30, 0x40, TURING_CHANNEL_GPFIFO_A, &channel_params(9, 0x1000_0000, 0x20_0000, 0))
            .unwrap();
        // Pushbuffer at VA 0x1000_1000: 2D on subch 3, a pitch destination at VA 0x1000_2000
        // (1920x1080, format 0xe6), a block-linear source at VA 0x1000_3000, one blit.
        let mut pb = inc(3, 0, &[0x902D]);
        pb.extend(inc(3, 0x200, &[0xE6, 1, 0, 1, 0, 0x1E00, 1920, 1080, 0, 0x1000_2000]));
        pb.extend(inc(3, 0x230, &[0xE6, 0, 0x10, 1, 0, 0, 1920, 1080, 0, 0x1000_3000]));
        pb.extend(inc(3, 0x8DC, &[0]));
        let bytes: Vec<u8> = pb.iter().flat_map(|d| d.to_le_bytes()).collect();
        fb.write(0x101000, &bytes);
        let e0 = 0x1000_1000u32;
        let e1 = (pb.len() as u32) << 10;
        fb.write(0x100000, &[e0.to_le_bytes(), e1.to_le_bytes()].concat());
        fb.write(0x20_0000 + USERD_GP_PUT, &1u32.to_le_bytes());
        let mem = GpuMem { fb: &mut fb, sys: &sys };
        let notes = d.gr_exception_postmortem(&mem, 9);
        let blit = notes.iter().find(|n| n.contains("2D blit")).unwrap_or_else(|| panic!("{notes:#?}"));
        assert!(blit.contains("dst format 0xe6 PITCH"), "{blit}");
        assert!(blit.contains("va 0x10002000 -> Fb(1056768)") || blit.contains("va 0x10002000 -> Fb(0x102000)"), "{blit}");
        assert!(blit.contains("kind 0x06"), "{blit}");
        assert!(blit.contains("src format 0xe6 BLOCKLINEAR block 0x10"), "{blit}");
        assert!(blit.contains("0x780x0x438 va 0x10003000") || blit.contains("1920x1080 va 0x10003000"), "{blit}");
        assert!(!blit.contains("assumed"), "{blit}");
        let _ = std::fs::remove_file(&path);
    }

    /// The RC post-mortem of any GR channel (ctrl.rs `gr_rc_push_note`, the Xid 109 of
    /// gr-preemption-2026-09-29.md): the entry at GPGet-1, its last methods, and whether it
    /// dispatched compute work or called an MME macro (a draw).
    #[test]
    fn a_gr_rc_names_the_push_the_host_fetched_last() {
        let (mut fb, sys, path, _root) = world("grrc");
        let f = crate::fixture::rtx2070();
        let compute = f.chip.data().channel.compute;
        let mut d = crate::ctrl::Dispatcher::new(f.chip.data(), &f.identity(4 << 30), None);
        let reg = &mut d.channels;
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        let mut pdes = vec![0u8; 40 + 24 * 6];
        pdes[40..48].copy_from_slice(&0x10000u64.to_le_bytes());
        pdes[56..60].copy_from_slice(&1u32.to_le_bytes());
        reg.record_server_reserved_pdes(1, 0x20, &pdes);
        let mut tsg = vec![0u8; 20];
        tsg[8..12].copy_from_slice(&0x20u32.to_le_bytes());
        reg.record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &tsg);
        reg.record_alloc(1, 0x30, 0x40, TURING_CHANNEL_GPFIFO_A, &channel_params(9, 0x1000_0000, 0x20_0000, 0))
            .unwrap();
        // Entry 0: 3D on subch 0 and an MME call; entry 1: compute on subch 1, a dispatch.
        let entry = |fb: &mut FakeFb, idx: u64, va: u32, pb: &[u32]| {
            let bytes: Vec<u8> = pb.iter().flat_map(|d| d.to_le_bytes()).collect();
            fb.write(0x100000 + u64::from(va - 0x1000_0000), &bytes);
            let e1 = (pb.len() as u32) << 10;
            fb.write(0x100000 + 8 * idx, &[va.to_le_bytes(), e1.to_le_bytes()].concat());
        };
        let mut pb0 = inc(0, 0, &[0xC597]);
        pb0.extend(inc(0, 0x3800, &[0x12]));
        entry(&mut fb, 0, 0x1000_1000, &pb0);
        let mut pb1 = inc(1, 0, &[compute]);
        pb1.extend(inc(1, 0x2B4, &[0x40_0000]));
        pb1.extend(inc(1, 0x2BC, &[0x3F]));
        entry(&mut fb, 1, 0x1000_2000, &pb1);
        fb.write(0x20_0000 + USERD_GP_GET, &2u32.to_le_bytes());
        fb.write(0x20_0000 + USERD_GP_PUT, &2u32.to_le_bytes());
        let mem = GpuMem { fb: &mut fb, sys: &sys };
        let n = d.gr_rc_push_note(&mem, 9, 109).unwrap();
        assert!(n.starts_with("gr rc push: chid 9 (exceptType 109): USERD GPGet 2 GPPut 2 (0 not fetched); entry 1 (GPGet-1) pb 0x10002000, 6 dword(s), 3 method(s): compute dispatch;"), "{n}");
        assert!(n.contains(&format!("1[{compute:#x}]:0x2b4=0x400000 1[{compute:#x}]:0x2bc=0x3f")), "{n}");
        assert!(n.contains(&format!("subchannel classes seen 0=0xc597 1={compute:#x}")), "{n}");
        // The draw entry.
        fb.write(0x20_0000 + USERD_GP_GET, &1u32.to_le_bytes());
        let mem = GpuMem { fb: &mut fb, sys: &sys };
        let n = d.gr_rc_push_note(&mem, 9, 109).unwrap();
        assert!(n.contains("entry 0 (GPGet-1) pb 0x10001000") && n.contains("MME macro call"), "{n}");
        assert!(n.contains("(1 not fetched)"), "{n}");
        // Garbage in USERD is named, not decoded.
        fb.write(0x20_0000 + USERD_GP_GET, &0xFFFF_FFFFu32.to_le_bytes());
        let mem = GpuMem { fb: &mut fb, sys: &sys };
        assert!(d.gr_rc_push_note(&mem, 9, 109).unwrap().contains("outside the ring"));
        let _ = std::fs::remove_file(&path);
    }

    /// A memset (constant remap) is written in chunks with no buffer of its size, and its
    /// pattern continues across the destination's pieces (the guest scrubber's fills,
    /// 2026-09-28): a 3-byte element over pieces of odd sizes, in FB and guest RAM.
    #[test]
    fn a_memset_fill_keeps_its_pattern_across_pieces() {
        let (mut fb, sys, path, _root) = world("cefill");
        let elem = [0xA1u8, 0xB2, 0xC3];
        let pieces = [(Target::Fb(0x10_0000), 7usize), (Target::Fb(0x20_0005), 5), (Target::Sys(0x4000_0100), 11)];
        {
            let mut mem = GpuMem { fb: &mut fb, sys: &sys };
            ce_fill(&mut mem, &pieces, &elem).unwrap();
        }
        let mut all = Vec::new();
        for (t, n) in pieces {
            let mut b = vec![0u8; n];
            match t {
                Target::Fb(pa) => assert!(fb.read(pa, &mut b)),
                Target::Sys(pa) => assert!(sys.read(pa, &mut b)),
                _ => unreachable!(),
            }
            all.extend(b);
        }
        let want: Vec<u8> = elem.iter().copied().cycle().take(all.len()).collect();
        assert_eq!(all, want);
        // A fill longer than one chunk.
        let n = 3 * CE_FILL_CHUNK + 17;
        {
            let mut mem = GpuMem { fb: &mut fb, sys: &sys };
            ce_fill(&mut mem, &[(Target::Fb(0x40_0000), n)], &[0x5Au8, 0x5B]).unwrap();
        }
        let mut b = vec![0u8; n];
        assert!(fb.read(0x40_0000, &mut b));
        assert!(b.chunks(2).all(|c| c[0] == 0x5A && (c.len() == 1 || c[1] == 0x5B)));
        let _ = std::fs::remove_file(&path);
    }

    /// A client that keeps its token at another notifier index (SET_WORK_SUBMIT_TOKEN_NOTIF_
    /// INDEX, handled by the guest RM alone): the entry is found by the RM's signature and
    /// gets the pool token, both when the default entry holds nothing and when the pool
    /// token was written to the default entry before.
    #[test]
    fn a_token_at_another_notifier_index_gets_the_pool_token() {
        use crate::tokens::{TokenPool, NOTIFIER_TOKEN_OFFSET, TOKEN_BASE, TOKEN_ENTRY_STATUS};
        let (mut fb, sys, path, _root) = world("tokens-index");
        let f = crate::fixture::rtx2070();
        let mut d = crate::ctrl::Dispatcher::new(f.chip.data(), &f.identity(4 << 30), None);
        d.channels.tokens = Some(TokenPool::new(4));
        let reg = &mut d.channels;
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        let mut pdes = vec![0u8; 40 + 24 * 6];
        pdes[40..48].copy_from_slice(&0x10000u64.to_le_bytes());
        pdes[56..60].copy_from_slice(&1u32.to_le_bytes());
        reg.record_server_reserved_pdes(1, 0x20, &pdes);
        let mut tsg = vec![0u8; 20];
        tsg[8..12].copy_from_slice(&0x20u32.to_le_bytes());
        reg.record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &tsg);
        let with_notifier = |chid: u32, userd: u64, notifier: u64| {
            let mut p = channel_params(chid, 0x1000_0000, userd, 0);
            let o = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_errorNotifierMem;
            p[o + v::MEM_DESC_BASE..o + v::MEM_DESC_BASE + 8]
                .copy_from_slice(&notifier.to_le_bytes());
            p[o + v::MEM_DESC_ADDRESS_SPACE..o + v::MEM_DESC_ADDRESS_SPACE + 4]
                .copy_from_slice(&ADDR_SYSMEM.to_le_bytes());
            p
        };
        let (n9, n10) = (0x4008_0000u64, 0x4008_1000u64);
        reg.record_alloc(1, 0x30, 0x40, TURING_CHANNEL_GPFIFO_A, &with_notifier(9, 0x20_0000, n9))
            .unwrap();
        reg.record_alloc(1, 0x30, 0x41, TURING_CHANNEL_GPFIFO_A, &with_notifier(10, 0x20_1000, n10))
            .unwrap();
        let entry = |notifier: u64, index: u64, token: u32| {
            let mut e = [0u8; 16];
            e[8..12].copy_from_slice(&token.to_le_bytes());
            e[14..16].copy_from_slice(&TOKEN_ENTRY_STATUS.to_le_bytes());
            assert!(sys.write(notifier + 16 * index, &e));
        };
        // Channel 9: entry 1 holds 0, the token is at index 5. Index 6 holds its value
        // without the RM's signature (info16 set), index 8 another channel's token and
        // index 64 its token past the scanned entries: none of them is touched.
        let (t9, t10) = ((1u32 << 16) | 9, (1u32 << 16) | 10);
        entry(n9, 5, t9);
        entry(n9, 6, t9);
        assert!(sys.write(n9 + 16 * 6 + 12, &1u16.to_le_bytes()));
        entry(n9, 8, t10);
        entry(n9, 64, t9);
        fb.write(0x100000, &[0u8; 16]);
        let mut mem = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        let notes = d.doorbell(&mut mem, t9);
        assert!(notes.iter().any(|n| n.contains("chid 9 gets pool token 0xca700000")), "{notes:#?}");
        assert_eq!(sys.read_u32(n9 + 16 * 5 + 8), Some(TOKEN_BASE));
        assert_eq!(sys.read_u32(n9 + NOTIFIER_TOKEN_OFFSET), Some(0), "entry 1 left alone");
        assert_eq!(sys.read_u32(n9 + 16 * 6 + 8), Some(t9), "no signature, not the RM's entry");
        assert_eq!(sys.read_u32(n9 + 16 * 8 + 8), Some(t10), "another channel's token");
        assert_eq!(sys.read_u32(n9 + 16 * 64 + 8), Some(t9), "past the scanned entries");
        // Channel 10: the token at entries 1 and 7 (a client that moved its index after the
        // first GET): both get the pool token.
        entry(n10, 1, t10);
        entry(n10, 7, t10);
        let notes = d.doorbell(&mut mem, t10);
        assert!(notes.iter().any(|n| n.contains("chid 10 gets pool token 0xca700001")), "{notes:#?}");
        assert_eq!(sys.read_u32(n10 + NOTIFIER_TOKEN_OFFSET), Some(TOKEN_BASE + 1));
        assert_eq!(sys.read_u32(n10 + 16 * 7 + 8), Some(TOKEN_BASE + 1));
        let s = &d.channels.tokens.as_ref().unwrap().stats;
        assert_eq!((s.handed_out, s.elsewhere, s.mismatch), (2, 2, 0));
        let _ = std::fs::remove_file(&path);
    }

    /// The doorbell token pool through the dispatcher (docs/caraxes/doorbell-token-pool.md):
    /// the first ring traps with the guest token and hands out a pool token through the
    /// work-submit-token notifier entry; later rings by pool token (eventfd or trapped)
    /// reach the same channel; stale, mismatched and freed cases fail closed.
    #[test]
    fn token_pool_hands_out_and_rings_by_token() {
        use crate::tokens::{TokenPool, NOTIFIER_TOKEN_OFFSET, TOKEN_BASE};
        let (mut fb, sys, path, _root) = world("tokens");
        let f = crate::fixture::rtx2070();
        let mut d = crate::ctrl::Dispatcher::new(f.chip.data(), &f.identity(4 << 30), None);
        d.channels.tokens = Some(TokenPool::new(4));
        let reg = &mut d.channels;
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        let mut pdes = vec![0u8; 40 + 24 * 6];
        pdes[40..48].copy_from_slice(&0x10000u64.to_le_bytes());
        pdes[56..60].copy_from_slice(&1u32.to_le_bytes());
        reg.record_server_reserved_pdes(1, 0x20, &pdes);
        let mut tsg = vec![0u8; 20];
        tsg[8..12].copy_from_slice(&0x20u32.to_le_bytes());
        reg.record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &tsg);
        // Channel 9 with its error notifier in guest RAM; channel 10 with one whose
        // work-submit-token entry holds something else.
        let with_notifier = |chid: u32, userd: u64, notifier: u64| {
            let mut p = channel_params(chid, 0x1000_0000, userd, 0);
            let o = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_errorNotifierMem;
            p[o + v::MEM_DESC_BASE..o + v::MEM_DESC_BASE + 8]
                .copy_from_slice(&notifier.to_le_bytes());
            p[o + v::MEM_DESC_ADDRESS_SPACE..o + v::MEM_DESC_ADDRESS_SPACE + 4]
                .copy_from_slice(&ADDR_SYSMEM.to_le_bytes());
            p
        };
        let (n9, n10) = (0x4008_0000u64, 0x4008_1000u64);
        let c9 = with_notifier(9, 0x20_0000, n9);
        let c10 = with_notifier(10, 0x20_1000, n10);
        reg.record_alloc(1, 0x30, 0x40, TURING_CHANNEL_GPFIFO_A, &c9)
            .unwrap();
        reg.record_alloc(1, 0x30, 0x41, TURING_CHANNEL_GPFIFO_A, &c10)
            .unwrap();
        // The guest RM wrote each channel's token into entry 1 (kchannelNotifyWorkSubmitToken).
        let (t9, t10) = ((1u32 << 16) | 9, (1u32 << 16) | 10);
        assert!(sys.write(n9 + NOTIFIER_TOKEN_OFFSET, &t9.to_le_bytes()));
        assert!(sys.write(n10 + NOTIFIER_TOKEN_OFFSET, &0x1234u32.to_le_bytes()));
        // Two NOP control entries for channel 9, published one at a time.
        fb.write(0x100000, &[0u8; 16]);
        fb.write(0x20_0000 + USERD_GP_PUT, &1u32.to_le_bytes());
        let mut mem = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };

        // First ring: trapped, the guest's own token; the entry now holds pool slot 0.
        let notes = d.doorbell(&mut mem, t9);
        let handed = |n: &String| n.contains("chid 9 gets pool token 0xca700000");
        assert!(notes.iter().any(handed), "{notes:#?}");
        assert_eq!(sys.read_u32(n9 + NOTIFIER_TOKEN_OFFSET), Some(TOKEN_BASE));
        assert_eq!(mem.read_u32(Target::Fb(0x20_0000 + USERD_GP_GET)), Some(1));

        // Next ring through the eventfd: the token names chid 9, which runs entry 1.
        mem.fb.write(0x20_0000 + USERD_GP_PUT, &2u32.to_le_bytes());
        let notes = d.doorbell_pool(&mut mem, TOKEN_BASE, false);
        assert!(notes[0].contains("(eventfd) -> chid 9"), "{notes:#?}");
        assert_eq!(mem.read_u32(Target::Fb(0x20_0000 + USERD_GP_GET)), Some(2));
        // The same token as a trapped write (its ioeventfd not registered) goes the same way.
        let notes = d.doorbell(&mut mem, TOKEN_BASE);
        assert!(notes[0].contains("(trapped) -> chid 9"), "{notes:#?}");
        // The guest ringing its own token again (a ring that does not read the notifier):
        // counted, the entry is not written again.
        d.doorbell(&mut mem, t9);
        assert_eq!(sys.read_u32(n9 + NOTIFIER_TOKEN_OFFSET), Some(TOKEN_BASE));
        // Someone (the host RM for a mirror) wrote the entry again: the next ring with
        // that token gets the same pool token written back.
        assert!(sys.write(n9 + NOTIFIER_TOKEN_OFFSET, &t9.to_le_bytes()));
        let notes = d.doorbell(&mut mem, t9);
        let back = |n: &String| n.contains("chid 9 gets back its pool token 0xca700000");
        assert!(notes.iter().any(back), "{notes:#?}");
        assert_eq!(sys.read_u32(n9 + NOTIFIER_TOKEN_OFFSET), Some(TOKEN_BASE));
        // An unassigned pool token is stale and rings nothing.
        let notes = d.doorbell(&mut mem, TOKEN_BASE + 1);
        assert!(notes[0].contains("stale"), "{notes:#?}");

        // Channel 10's entry does not hold the token it rang: no pool token for it.
        let notes = d.doorbell(&mut mem, t10);
        let kept = |n: &String| n.contains("keeps its own token");
        assert!(notes.iter().any(kept), "{notes:#?}");
        assert_eq!(sys.read_u32(n10 + NOTIFIER_TOKEN_OFFSET), Some(0x1234));

        // Freeing channel 9 frees its token; a ring of it is then stale.
        d.free(1, 0x40);
        let notes = d.doorbell_pool(&mut mem, TOKEN_BASE, false);
        assert!(notes[0].contains("stale"), "{notes:#?}");
        let p = d.channels.tokens.as_ref().unwrap();
        assert_eq!(p.assigned(), 0);
        let s = &p.stats;
        assert_eq!((s.handed_out, s.trapped_pool_rings, s.stale), (1, 2, 2));
        assert_eq!(
            (s.own_token_after_handout, s.rewritten, s.mismatch),
            (1, 1, 1)
        );
        let _ = std::fs::remove_file(&path);
    }

    /// A registry with a VAS, a TSG and GR-style channels at the given ChIDs, each with its
    /// error notifier in guest RAM at 0x4008_0000 + 0x1000 * i and its own USERD.
    fn token_world(d: &mut crate::ctrl::Dispatcher, chids: &[u32]) -> Vec<u64> {
        let reg = &mut d.channels;
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        let mut pdes = vec![0u8; 40 + 24 * 6];
        pdes[40..48].copy_from_slice(&0x10000u64.to_le_bytes());
        pdes[56..60].copy_from_slice(&1u32.to_le_bytes());
        reg.record_server_reserved_pdes(1, 0x20, &pdes);
        let mut tsg = vec![0u8; 20];
        tsg[8..12].copy_from_slice(&0x20u32.to_le_bytes());
        reg.record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &tsg);
        let mut notifiers = Vec::new();
        for (i, &chid) in chids.iter().enumerate() {
            let n = 0x4008_0000u64 + 0x1000 * i as u64;
            let mut p = channel_params(chid, 0x1000_0000, 0x20_0000 + 0x1000 * i as u64, 0);
            let o = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_errorNotifierMem;
            p[o + v::MEM_DESC_BASE..o + v::MEM_DESC_BASE + 8].copy_from_slice(&n.to_le_bytes());
            p[o + v::MEM_DESC_ADDRESS_SPACE..o + v::MEM_DESC_ADDRESS_SPACE + 4]
                .copy_from_slice(&ADDR_SYSMEM.to_le_bytes());
            reg.record_alloc(1, 0x30, 0x40 + i as u32, TURING_CHANNEL_GPFIFO_A, &p)
                .unwrap();
            notifiers.push(n);
        }
        notifiers
    }

    /// The 2026-09-26 collision (vm4-615-cuda-vulkan/NOTES.md): mirrored chid 12 holds host
    /// token 0x36 (host chid 54); the guest's new GR channel has ChID 54, so its first ring
    /// is the raw value 0x36. That ring must go to guest chid 54 (the one that builds or runs
    /// it), and chid 12 is rung too; values that name no guest channel still resolve by
    /// host token, and a guest channel's own value without a host owner stays with it.
    #[test]
    fn a_guest_token_equal_to_a_host_token_rings_the_guest_channel() {
        let f = crate::fixture::rtx2070();
        let mut d = crate::ctrl::Dispatcher::new(f.chip.data(), &f.identity(4 << 30), None);
        token_world(&mut d, &[12, 54]);
        use crate::ctrl::{route_token, Route};
        // The collision: the old order returned (12, via host token) and never rang 54.
        assert_eq!(
            route_token(&d.channels, 0x36, Some(12)),
            Route {
                chid: 54,
                via_host_token: false,
                also: Some(12)
            }
        );
        // A host token naming no guest channel (host chid 0x3f, runlist 8): the owner.
        assert_eq!(
            route_token(&d.channels, 0x8003f, Some(12)),
            Route {
                chid: 12,
                via_host_token: true,
                also: None
            }
        );
        // The guest channel's own token, no host owner (the no-mirror path): unchanged.
        assert_eq!(
            route_token(&d.channels, 54, None),
            Route {
                chid: 54,
                via_host_token: false,
                also: None
            }
        );
        // A mirror whose host token has its own ChID's low bits: that channel, once.
        assert_eq!(
            route_token(&d.channels, 12, Some(12)),
            Route {
                chid: 12,
                via_host_token: true,
                also: None
            }
        );
        // A value naming nothing: as before, the ChID (reported as no such channel).
        assert_eq!(
            route_token(&d.channels, 0x77, None),
            Route {
                chid: 0x77,
                via_host_token: false,
                also: None
            }
        );
    }

    /// Through the dispatcher without a pool and without a mirror (the old behaviour):
    /// the raw ring of ChID 54 runs guest chid 54's entry on the software executor, and no
    /// notifier entry is written.
    #[test]
    fn without_a_pool_the_guest_ring_is_unchanged() {
        let (mut fb, sys, path, _root) = world("nopool");
        let f = crate::fixture::rtx2070();
        let mut d = crate::ctrl::Dispatcher::new(f.chip.data(), &f.identity(4 << 30), None);
        let n = token_world(&mut d, &[12, 54]);
        use crate::tokens::NOTIFIER_TOKEN_OFFSET as AT;
        assert!(sys.write(n[1] + AT, &0x36u32.to_le_bytes()));
        // One NOP control entry for chid 54 (USERD at FB 0x201000).
        fb.write(0x100000, &[0u8; 8]);
        fb.write(0x20_1000 + USERD_GP_PUT, &1u32.to_le_bytes());
        let mut mem = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        let notes = d.doorbell(&mut mem, 0x36);
        assert_eq!(
            mem.read_u32(Target::Fb(0x20_1000 + USERD_GP_GET)),
            Some(1),
            "{notes:#?}"
        );
        assert_eq!(sys.read_u32(n[1] + AT), Some(0x36));
        assert_eq!(d.routing.collisions_avoided, 0);
        let _ = std::fs::remove_file(&path);
    }

    /// Part 2: at the mirror's scheduling the host token in the channel's work-submit-token
    /// entry is replaced by its pool token before the guest can read it (`pin_pool_token`);
    /// an entry holding anything else is left alone, and without a pool nothing is written.
    #[test]
    fn a_scheduled_mirror_gets_its_pool_token_not_the_host_token() {
        use crate::tokens::{TokenPool, NOTIFIER_TOKEN_OFFSET as AT, TOKEN_BASE};
        let (mut fb, sys, path, _root) = world("pin");
        let f = crate::fixture::rtx2070();
        let mut d = crate::ctrl::Dispatcher::new(f.chip.data(), &f.identity(4 << 30), None);
        let n = token_world(&mut d, &[12, 13]);
        let mut mem = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        // Without a pool: the host token stays (the old behaviour).
        assert!(sys.write(n[0] + AT, &0x36u32.to_le_bytes()));
        assert!(d.pin_pool_token(&mut mem, 12, 0x36).is_empty());
        assert_eq!(sys.read_u32(n[0] + AT), Some(0x36));
        // With a pool: the host RM wrote host token 0x36 into chid 12's entry.
        d.channels.tokens = Some(TokenPool::new(4));
        let notes = d.pin_pool_token(&mut mem, 12, 0x36);
        assert!(
            notes
                .iter()
                .any(|s| s.contains("chid 12 gets pool token 0xca700000")),
            "{notes:#?}"
        );
        assert_eq!(sys.read_u32(n[0] + AT), Some(TOKEN_BASE));
        assert_ne!(sys.read_u32(n[0] + AT), Some(0x36));
        assert_eq!(d.routing.pinned_at_schedule, 1);
        // Chid 13's entry holds something that is not its host token: not overwritten.
        assert!(sys.write(n[1] + AT, &0x1234u32.to_le_bytes()));
        assert!(d.pin_pool_token(&mut mem, 13, 0x37).is_empty());
        assert_eq!(sys.read_u32(n[1] + AT), Some(0x1234));
        assert_eq!(d.routing.pinned_at_schedule, 1);
        let _ = std::fs::remove_file(&path);
    }

    /// The software executor consults the device's one set of scheduling flags (rc.rs):
    /// a channel the guest switched off (its own flag, the usermode switch for a channel
    /// of a TSG, the device-wide toggle) runs nothing at its doorbell; the doorbell is held
    /// and runs once the channel is enabled again.
    #[test]
    fn a_disabled_software_channel_holds_its_doorbell() {
        let (mut fb, sys, path, _root) = world("held");
        let f = crate::fixture::rtx2070();
        let mut d = crate::ctrl::Dispatcher::new(f.chip.data(), &f.identity(4 << 30), None);
        let reg = &mut d.channels;
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        let mut pdes = vec![0u8; 40 + 24 * 6];
        pdes[40..48].copy_from_slice(&0x10000u64.to_le_bytes());
        pdes[56..60].copy_from_slice(&1u32.to_le_bytes());
        reg.record_server_reserved_pdes(1, 0x20, &pdes);
        let mut tsg = vec![0u8; 20];
        tsg[8..12].copy_from_slice(&0x20u32.to_le_bytes());
        reg.record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &tsg);
        reg.record_alloc(
            1,
            0x30,
            0x40,
            TURING_CHANNEL_GPFIFO_A,
            &channel_params(9, 0x1000_0000, 0x20_0000, 0),
        )
        .unwrap();
        // Three NOP control entries, published one at a time.
        fb.write(0x100000, &[0u8; 24]);
        let mut mem = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        let gp_get = |mem: &GpuMem| mem.read_u32(Target::Fb(0x20_0000 + USERD_GP_GET));
        let t9 = (1u32 << 16) | 9;
        for (n, how) in [(1u32, "usermode"), (2, "toggle"), (3, "own")] {
            mem.fb.write(0x20_0000 + USERD_GP_PUT, &n.to_le_bytes());
            match how {
                "usermode" => d.dstate.rc.usermode_disabled = true,
                "toggle" => d.dstate.rc.scheduling_disabled = true,
                _ => {
                    d.dstate.rc.disabled.insert((1, 0x40));
                }
            }
            let notes = d.doorbell(&mut mem, t9);
            assert!(notes[0].contains("held until enabled"), "{how}: {notes:#?}");
            assert_eq!(gp_get(&mem), Some(n - 1), "{how}: nothing ran");
            assert!(d.replay_held(&mut mem).is_empty(), "{how}: still off");
            d.dstate.rc.usermode_disabled = false;
            d.dstate.rc.scheduling_disabled = false;
            d.dstate.rc.disabled.clear();
            let notes = d.replay_held(&mut mem);
            assert!(
                notes[0].contains("held doorbell runs now"),
                "{how}: {notes:#?}"
            );
            assert_eq!(gp_get(&mem), Some(n), "{how}: the held entry ran");
            assert!(d.dstate.rc.held.is_empty());
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pending_follows_gpput_on_both_paths() {
        let (mut fb, sys, path, _root) = world("pending");
        let mut reg = Registry::new(classes());
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        let mut pdes = vec![0u8; 40 + 24 * 6];
        pdes[40..48].copy_from_slice(&0x10000u64.to_le_bytes());
        pdes[56..60].copy_from_slice(&1u32.to_le_bytes());
        reg.record_server_reserved_pdes(1, 0x20, &pdes);
        let mut tsg = vec![0u8; 20];
        tsg[8..12].copy_from_slice(&0x20u32.to_le_bytes());
        reg.record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &tsg);
        reg.record_alloc(
            1,
            0x30,
            0x40,
            TURING_CHANNEL_GPFIFO_A,
            &channel_params(9, 0x1000_0000, 0x20_0000, 0),
        )
        .unwrap();
        // GPFIFO entry 0: a NOP control entry, so the software executor consumes it.
        fb.write(0x100000, &0u64.to_le_bytes());
        let mut mem = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        // Never rung, GPPut 0: not pending; an unknown chid is not pending either.
        assert!(!reg.pending(&mem, 9));
        assert!(!reg.pending(&mem, 77));
        // The guest publishes one entry: pending until the software executor ran it.
        mem.fb.write(0x20_0000 + USERD_GP_PUT, &1u32.to_le_bytes());
        assert!(reg.pending(&mem, 9));
        reg.note_rung(&mem, 9);
        assert!(
            reg.pending(&mem, 9),
            "a software channel is pending until GPGet catches up"
        );
        reg.doorbell(&mut mem, 9);
        assert!(!reg.pending(&mem, 9));
        // A mirrored channel: the host runs it, so GPGet stays; pending means GPPut moved
        // since the last doorbell.
        reg.channel_mut(9).unwrap().mirrored = true;
        assert!(!reg.pending(&mem, 9));
        mem.fb.write(0x20_0000 + USERD_GP_PUT, &2u32.to_le_bytes());
        assert!(reg.pending(&mem, 9));
        reg.note_rung(&mem, 9);
        assert!(!reg.pending(&mem, 9));
        // The fast ring path spares note_rung's read only with the token pool on (which
        // excludes the token-less reader, the one user of `pending`).
        crate::launch::set_fast_here(true);
        mem.fb.write(0x20_0000 + USERD_GP_PUT, &3u32.to_le_bytes());
        reg.note_rung(&mem, 9);
        assert!(!reg.pending(&mem, 9), "no pool: still recorded");
        reg.tokens = Some(crate::tokens::TokenPool::new(4));
        mem.fb.write(0x20_0000 + USERD_GP_PUT, &4u32.to_le_bytes());
        reg.note_rung(&mem, 9);
        assert!(reg.pending(&mem, 9), "pool on: the read is skipped");
        crate::launch::set_fast_here(false);
        reg.note_rung(&mem, 9);
        assert!(!reg.pending(&mem, 9), "switch off: recorded again");
        let _ = std::fs::remove_file(&path);
    }

    /// The usermode page's doorbell (um_page.rs): one scan after any number of doorbell
    /// writes rings every channel whose GPPut advanced, and only those; a channel whose
    /// GPPut is not written yet is not rung (the guest writes GPPut before the doorbell,
    /// the reactor scans after taking the doorbell word).
    #[test]
    fn one_page_doorbell_rings_every_advanced_channel() {
        let (mut fb, sys, path, _root) = world("umpage");
        let f = crate::fixture::rtx2070();
        let mut d = crate::ctrl::Dispatcher::new(f.chip.data(), &f.identity(4 << 30), None);
        let reg = &mut d.channels;
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        let mut pdes = vec![0u8; 40 + 24 * 6];
        pdes[40..48].copy_from_slice(&0x10000u64.to_le_bytes());
        pdes[56..60].copy_from_slice(&1u32.to_le_bytes());
        reg.record_server_reserved_pdes(1, 0x20, &pdes);
        let mut tsg = vec![0u8; 20];
        tsg[8..12].copy_from_slice(&0x20u32.to_le_bytes());
        reg.record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &tsg);
        for (chid, h, userd) in [(9u32, 0x40u32, 0x20_0000u64), (10, 0x41, 0x20_1000), (11, 0x42, 0x20_2000)] {
            reg.record_alloc(
                1,
                0x30,
                h,
                TURING_CHANNEL_GPFIFO_A,
                &channel_params(chid, 0x1000_0000, userd, 0),
            )
            .unwrap();
        }
        fb.write(0x100000, &[0u8; 16]);
        let mut mem = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        // Channels 9 and 10 publish one entry each and write the doorbell twice (only the
        // last value is left in the page); channel 11 publishes nothing.
        mem.fb.write(0x20_0000 + USERD_GP_PUT, &1u32.to_le_bytes());
        mem.fb.write(0x20_1000 + USERD_GP_PUT, &1u32.to_le_bytes());
        let notes = d.doorbell_pending(&mut mem);
        assert_eq!(crate::um_page::pending_count(&notes[0]), 2, "{notes:#?}");
        assert!(notes[0].contains("[9, 10]"), "{notes:#?}");
        assert_eq!(mem.read_u32(Target::Fb(0x20_0000 + USERD_GP_GET)), Some(1));
        assert_eq!(mem.read_u32(Target::Fb(0x20_1000 + USERD_GP_GET)), Some(1));
        // Nothing advanced since: a second scan (the watchdog's) rings nothing.
        let notes = d.doorbell_pending(&mut mem);
        assert_eq!(crate::um_page::pending_count(&notes[0]), 0, "{notes:#?}");
        let _ = std::fs::remove_file(&path);
    }

    /// 615: the kernel's TSG id from the TSG's internalFlags and the internal TSG of a
    /// bare channel; 610 has neither (its TSG params end before internalFlags).
    #[test]
    fn kernel_tsg_ids_are_carried() {
        let mut reg = Registry::new(classes());
        let off = <Abi as RmAbi>::TSG_PARAMS_OFF_internalFlags;
        let mut tsg = vec![0u8; <Abi as RmAbi>::TSG_PARAMS_SIZE as usize];
        let has = tsg.len() >= off + 4;
        if has {
            // TSG_ID 7, TSG_ID_VALID, GSP_OWNED (g_kernel_channel_group_nvoc.h:123-129).
            tsg[off..off + 4].copy_from_slice(&(7u32 | 1 << 16 | 1 << 17).to_le_bytes());
        }
        let note = reg
            .record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &tsg)
            .unwrap();
        assert_eq!(reg.tsg_id((1, 0x30)), has.then_some(7), "{note}");
        let mut ch = channel_params(3, 0x1000_0000, 0x20_0000, 0);
        let f = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_internalFlags;
        ch[f..f + 4].copy_from_slice(&(12u32 << 8 | 1 << 24).to_le_bytes());
        let note = reg
            .record_alloc(1, 0x10, 0x40, TURING_CHANNEL_GPFIFO_A, &ch)
            .unwrap();
        assert!(note.contains("internal TSG 12"), "{note}");
        assert_eq!(reg.tsg_id((1, 0x40)), Some(12));
        reg.record_free(1, 0x30);
        assert_eq!(reg.tsg_id((1, 0x30)), None);
    }

    #[test]
    fn a_reallocated_channel_gets_a_new_generation() {
        let mut reg = Registry::new(classes());
        let params = channel_params(3, 0x1000_0000, 0x20_0000, 0);
        reg.record_alloc(1, 0x10, 0x40, TURING_CHANNEL_GPFIFO_A, &params)
            .expect("first channel allocation");
        let first = reg.channel(3).expect("first channel").generation;

        reg.record_free(1, 0x40);
        reg.record_alloc(1, 0x10, 0x40, TURING_CHANNEL_GPFIFO_A, &params)
            .expect("identical channel reallocation");
        let second = reg.channel(3).expect("replacement channel").generation;

        assert_eq!(first, 1);
        assert_eq!(second, 2);
        assert_ne!(first, second);
        reg.record_alloc(1, 0x10, 0x40, TURING_CHANNEL_GPFIFO_A, &params)
            .expect("replacement without FREE");
        assert_eq!(reg.channel(3).unwrap().generation, 3);
    }

    #[test]
    fn engine_identity_is_client_scoped_and_tracks_channel_generation() {
        let mut reg = Registry::new(classes());
        let p3 = channel_params(3, 0x1000_0000, 0x20_0000, 0);
        let p4 = channel_params(4, 0x1000_1000, 0x20_1000, 0);
        reg.record_alloc(1, 0x10, 0x40, TURING_CHANNEL_GPFIFO_A, &p3);
        reg.record_alloc(2, 0x10, 0x41, TURING_CHANNEL_GPFIFO_A, &p4);
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[1, 2]);
        reg.record_alloc(2, 0x41, 0x77, 0xc597, &[3, 4]);

        let one = reg.engine_object(1, 0x77).unwrap();
        let two = reg.engine_object(2, 0x77).unwrap();
        assert_eq!((one.0, one.2, one.3), (3, 0x5080, &[1, 2][..]));
        assert_eq!((two.0, two.2, two.3), (4, 0xc597, &[3, 4][..]));
        assert_ne!(one.1, two.1);
        assert_eq!(reg.engine_owner(1, 0x77), Some((3, one.1)));
        let first_generation = one.1;
        assert_eq!(
            reg.channel(3)
                .unwrap()
                .engine_objects_with_handles()
                .collect::<Vec<_>>(),
            vec![(0x77, 0x5080, &[1, 2][..])]
        );

        reg.record_free(1, 0x40);
        reg.record_alloc(1, 0x10, 0x40, TURING_CHANNEL_GPFIFO_A, &p3);
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[5]);
        let replacement = reg.engine_object(1, 0x77).unwrap();
        assert_eq!(
            (replacement.0, replacement.2, replacement.3),
            (3, 0x5080, &[5][..])
        );
        assert_ne!(replacement.1, first_generation);
        assert_eq!(reg.engine_object(2, 0x77).unwrap().3, &[3, 4]);
    }

    #[test]
    fn engine_replacement_and_free_remove_exact_owned_identity() {
        let mut reg = Registry::new(classes());
        reg.record_alloc(
            1,
            0x10,
            0x40,
            TURING_CHANNEL_GPFIFO_A,
            &channel_params(3, 0x1000_0000, 0x20_0000, 0),
        );
        reg.record_alloc(
            1,
            0x10,
            0x41,
            TURING_CHANNEL_GPFIFO_A,
            &channel_params(4, 0x1000_1000, 0x20_1000, 0),
        );
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[1]);
        reg.record_alloc(1, 0x41, 0x77, 0xc597, &[2]);
        assert_eq!(reg.engine_object(1, 0x77).unwrap().0, 4);
        assert!(reg
            .channel(3)
            .unwrap()
            .engine_objects_with_handles()
            .next()
            .is_none());

        reg.record_free(1, 0x77);
        assert!(reg.engine_object(1, 0x77).is_none());
        assert!(reg.channel(4).is_some());
    }

    #[test]
    fn software_object_ids_are_exact_monotonic_and_unknown_history_fails_strict() {
        let mut reg = Registry::new(classes());
        let params = channel_params(3, 0x1000_0000, 0x20_0000, 0);
        reg.record_alloc(1, 0x10, 0x40, TURING_CHANNEL_GPFIFO_A, &params);
        reg.record_alloc(1, 0x40, 0x70, 0x5080, &[]);
        reg.record_alloc(1, 0x40, 0x71, GP100_UVM_SW, &[]);
        assert_eq!(
            reg.channel(3).unwrap().software_objects.get(&1),
            Some(&(0x70, 0x5080))
        );
        assert_eq!(
            reg.channel(3).unwrap().software_objects.get(&2),
            Some(&(0x71, GP100_UVM_SW))
        );
        reg.record_free(1, 0x70);
        reg.record_alloc(1, 0x40, 0x72, 0x5080, &[]);
        assert!(!reg.channel(3).unwrap().software_objects.contains_key(&1));
        assert_eq!(
            reg.channel(3).unwrap().software_objects.get(&3),
            Some(&(0x72, 0x5080))
        );

        reg.record_alloc(1, 0x40, 0x73, 0xdead, &[]);
        reg.record_free(1, 0x73);
        assert!(reg.enable_deferred_management(3).is_err());
        reg.record_free(1, 0x40);
        reg.record_alloc(1, 0x10, 0x40, TURING_CHANNEL_GPFIFO_A, &params);
        reg.record_alloc(1, 0x40, 0x70, 0x5080, &[]);
        assert_eq!(
            reg.channel(3).unwrap().software_objects.get(&1),
            Some(&(0x70, 0x5080))
        );
        assert!(reg.enable_deferred_management(3).is_ok());
    }

    #[test]
    fn freeing_tsg_or_client_removes_owned_engine_identities() {
        let mut reg = Registry::new(classes());
        let mut tsg = vec![0u8; 20];
        tsg[8..12].copy_from_slice(&0x20u32.to_le_bytes());
        reg.record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &tsg);
        reg.record_alloc(
            1,
            0x30,
            0x40,
            TURING_CHANNEL_GPFIFO_A,
            &channel_params(3, 0x1000_0000, 0x20_0000, 0),
        );
        reg.record_alloc(1, 0x40, 0x77, 0x5080, &[]);
        reg.record_free(1, 0x30);
        assert!(reg.channel(3).is_none());
        assert!(reg.engine_object(1, 0x77).is_none());

        reg.record_alloc(
            2,
            0x10,
            0x41,
            TURING_CHANNEL_GPFIFO_A,
            &channel_params(4, 0x1000_1000, 0x20_1000, 0),
        );
        reg.record_alloc(2, 0x41, 0x78, 0x5080, &[]);
        reg.record_free(2, 2);
        assert!(reg.channel(4).is_none());
        assert!(reg.engine_object(2, 0x78).is_none());
    }

    #[test]
    fn acquire_blocks_until_released() {
        let (mut fb, sys, path, root) = world("acquire");
        let mut reg = Registry::new(classes());
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        reg.record_set_page_directory(1, &{
            let mut p = vec![0u8; 32];
            let Target::Fb(r) = root else { unreachable!() };
            p[0..8].copy_from_slice(&r.to_le_bytes());
            p[16..20].copy_from_slice(&0x20u32.to_le_bytes());
            p
        });
        reg.record_alloc(
            1,
            0x10,
            0x40,
            TURING_CHANNEL_GPFIFO_A,
            &channel_params(1, 0x1000_0000, 0x20_0000, 0x20),
        );
        reg.record_alloc(
            1,
            0x10,
            0x41,
            TURING_CHANNEL_GPFIFO_A,
            &channel_params(2, 0x1000_1000, 0x20_1000, 0x20),
        );
        // Channel 1 acquires GPA 0x4000_3000 (VA page 11) >= 1, then releases 9 at page 12.
        let mut a = inc(0, 0x5C, &[0x1000_B000, 0, 1, 0, 3]);
        a.extend(inc(0, 0x5C, &[0x1000_C000, 0, 9, 0, 1]));
        // Channel 2 releases 1 at page 11.
        let b = inc(0, 0x5C, &[0x1000_B000, 0, 1, 0, 1]);
        let put_pb = |gpa: u64, d: &[u32]| {
            let bytes: Vec<u8> = d.iter().flat_map(|x| x.to_le_bytes()).collect();
            assert!(sys.write(gpa, &bytes));
        };
        put_pb(0x4000_0000, &a);
        put_pb(0x4000_0800, &b);
        let entry = |va: u32, len: usize| {
            let mut v = va.to_le_bytes().to_vec();
            v.extend_from_slice(&((len as u32) << 10).to_le_bytes());
            v
        };
        fb.write(0x100000, &entry(0x1000_8000, a.len()));
        fb.write(0x101000, &entry(0x1000_8800, b.len()));
        fb.write(0x20_0000 + USERD_GP_PUT, &1u32.to_le_bytes());
        fb.write(0x20_1000 + USERD_GP_PUT, &1u32.to_le_bytes());
        let mut mem = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        let n = reg.doorbell(&mut mem, 1).join("\n");
        assert!(n.contains("channel waits"), "{n}");
        assert_eq!(sys.read_u32(0x4000_4000), Some(0));
        let n = reg.doorbell(&mut mem, 2).join("\n");
        assert_eq!(sys.read_u32(0x4000_3000), Some(1), "{n}");
        assert_eq!(sys.read_u32(0x4000_4000), Some(9), "{n}");
        assert_eq!(mem.read_u32(Target::Fb(0x20_0000 + USERD_GP_GET)), Some(1));
        let _ = std::fs::remove_file(&path);
    }

    /// CUDA's GR channel: VAS through the context share, root set by UVM on its
    /// duplicate of the VAS; a compute subchannel runs the stub.
    #[test]
    fn cuda_gr_channel_with_compute_stub() {
        let (mut fb, sys, path, root) = world("compute");
        let Target::Fb(r) = root else { unreachable!() };
        let mut reg = Registry::new(classes());
        // CUDA's VAS 0x20 in client 1; UVM duplicates it as 0x99 in client 2 and sets
        // the page directory there.
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        assert!(reg.record_dup((2, 0x99), (1, 0x20)).is_some());
        assert!(reg.record_dup((2, 0x98), (1, 0x77)).is_none());
        let note = reg.record_set_page_directory(2, &{
            let mut p = vec![0u8; 32];
            p[0..8].copy_from_slice(&r.to_le_bytes());
            p[16..20].copy_from_slice(&0x99u32.to_le_bytes());
            p
        });
        assert!(note.contains("alias of 0x1/0x20"), "{note}");
        assert_eq!(reg.vas_root((1, 0x20)), Some(root));
        // GR TSG without VAS, a context share naming 0x20, a channel naming the share.
        reg.record_alloc(1, 0x10, 0x30, KEPLER_CHANNEL_GROUP_A, &[0u8; 20]);
        let mut share = vec![0u8; 12];
        share[0..4].copy_from_slice(&0x20u32.to_le_bytes());
        reg.record_alloc(1, 0x30, 0x31, FERMI_CONTEXT_SHARE_A, &share);
        let mut cp = channel_params(3, 0x1000_0000, 0x20_0000, 0);
        let o = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_hContextShare;
        cp[o..o + 4].copy_from_slice(&0x31u32.to_le_bytes());
        reg.record_alloc(1, 0x30, 0x40, TURING_CHANNEL_GPFIFO_A, &cp);
        assert_eq!(reg.channels().next().unwrap().vas, Some((1, 0x20)));

        // Compute on subchannel 1: I2M of 8 bytes into VA page 9 with its semaphore,
        // a one-word report semaphore with ADD reduction, and a launch.
        let mut pb = inc(1, 0x0, &[0xC5C0]);
        pb.extend(inc(1, 0x180, &[8, 1, 0, 0x1000_9000, 8]));
        pb.extend(inc(1, 0x1DC, &[0, 0x1000_9100, 0x42]));
        pb.extend(inc(1, 0x1B0, &[1 | (2 << 4) | (1 << 12)]));
        // LOAD_INLINE_DATA as a non-incrementing method.
        pb.push((3 << 29) | (2 << 16) | (1 << 13) | (0x1B4 >> 2));
        pb.extend([0xDEAD_BEEF, 0x0123_4567]);
        pb.extend(inc(1, 0x1B00, &[0, 0x1000_9200, 5, (1 << 28) | (1 << 3)]));
        pb.extend(inc(1, 0x244, &[0]));
        pb.extend(inc(1, 0x2B4, &[0x1234]));
        pb.extend(inc(1, 0x2BC, &[1]));
        // An inline QMD launch whose release semaphore (dwords 23-25) must stay unwritten.
        let mut qmd = [0u32; 64];
        (qmd[12], qmd[13], qmd[14]) = (16, 1, 1);
        (qmd[23], qmd[24], qmd[25]) = (0x1000_9300, 0x8000_0000, 6);
        pb.extend(inc(1, 0x318, &[0, 0x0010_0092]));
        pb.extend(inc(1, 0x320, &qmd));
        let bytes: Vec<u8> = pb.iter().flat_map(|d| d.to_le_bytes()).collect();
        assert!(sys.write(0x4000_0000, &bytes));
        assert!(sys.write(0x4000_1200, &3u32.to_le_bytes()));
        let mut gp = 0x1000_8000u32.to_le_bytes().to_vec();
        gp.extend_from_slice(&((pb.len() as u32) << 10).to_le_bytes());
        fb.write(0x100000, &gp);
        fb.write(0x20_0000 + USERD_GP_PUT, &1u32.to_le_bytes());
        let mut mem = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        let all = reg.doorbell(&mut mem, 3).join("\n");
        assert_eq!(
            sys.read_u64(0x4000_1000),
            Some(0x0123_4567_DEAD_BEEF),
            "{all}"
        );
        assert_eq!(sys.read_u32(0x4000_1100), Some(0x42), "{all}");
        assert_eq!(sys.read_u32(0x4000_1200), Some(8), "{all}");
        assert!(all.contains("KERNEL LAUNCH NOT EXECUTED"), "{all}");
        assert!(all.contains("compute method 0x244 = 0x0 skipped"), "{all}");
        assert!(
            all.contains("grid 16x1x1; release0 va 0x10009300 payload 0x6"),
            "{all}"
        );
        assert_eq!(sys.read_u32(0x4000_1300), Some(0));
        assert_eq!(reg.channels().next().unwrap().launches_refused, 2);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reductions() {
        assert_eq!(reduce(6, true, 3, 10), 4);
        assert_eq!(reduce(6, true, 10, 10), 0);
        assert_eq!(reduce(5, true, 3, 10), 13);
        assert_eq!(reduce(7, true, 0, 10), 10);
    }

    /// Run P17: the Windows KMD sends COPY_SERVER_RESERVED_PDES a second time with one
    /// PD1-level table and the empty range 1..0. That call describes a subtree, not a root.
    #[test]
    fn a_lower_level_server_pde_call_keeps_the_vas_root() {
        let mut reg = Registry::new(classes());
        reg.record_alloc(1, 0x80, 0x20, FERMI_VASPACE_A, &[0u8; 56]);
        let pdes = |pa: u64, size: u64, shift: u8, lo: u64, hi: u64| {
            let mut p = vec![0u8; v::PDES_LEVELS + v::PDES_LEVEL_SIZE * v::PDES_MAX_LEVELS];
            let put = |p: &mut Vec<u8>, o: usize, b: &[u8]| p[o..o + b.len()].copy_from_slice(b);
            let l = v::PDES_LEVELS;
            put(&mut p, v::PDES_NUM_LEVELS, &1u32.to_le_bytes());
            put(&mut p, v::PDES_VIRT_ADDR_LO, &lo.to_le_bytes());
            put(&mut p, v::PDES_VIRT_ADDR_HI, &hi.to_le_bytes());
            put(&mut p, l + v::PDES_LEVEL_PHYS_ADDRESS, &pa.to_le_bytes());
            put(&mut p, l + v::PDES_LEVEL_SIZE_FIELD, &size.to_le_bytes());
            put(&mut p, l + v::PDES_LEVEL_APERTURE, &1u32.to_le_bytes());
            p[l + v::PDES_LEVEL_PAGE_SHIFT] = shift;
            p
        };
        let root = pdes(0x3b48_5000, 0x20, 47, 0x1_0000_0000, 0x1_1fff_ffff);
        reg.record_server_reserved_pdes(1, 0x20, &root);
        assert_eq!(reg.vas_root((1, 0x20)), Some(Target::Fb(0x3b48_5000)));
        let note = reg.record_server_reserved_pdes(1, 0x20, &pdes(0x0d3f_0000, 0x1000, 29, 1, 0));
        assert!(note.contains("not a root"), "{note}");
        assert_eq!(reg.vas_root((1, 0x20)), Some(Target::Fb(0x3b48_5000)));
        // A real root replacement (shift 47) is still taken.
        let next = pdes(0x3b49_9000, 0x20, 47, 0x1_0000_0000, 0x1_1fff_ffff);
        reg.record_server_reserved_pdes(1, 0x20, &next);
        assert_eq!(reg.vas_root((1, 0x20)), Some(Target::Fb(0x3b49_9000)));
    }

    /// Fix 2 of run P17 (pdbwatch.rs): a paging channel's `MEM_OP` names the root another
    /// VAS really uses. That VAS's root moved, and the guest still invalidates the former
    /// one: its channels are walked from the wrong root until the guest names the new one.
    #[test]
    fn a_memop_naming_a_former_root_flags_that_vas() {
        let (mut fb, sys, path, root) = world("pdbwatch");
        let mut reg = Registry::new(classes());
        let set_pd = |reg: &mut Registry, vas: u32, pa: u64| {
            let mut p = vec![0u8; 32];
            p[0..8].copy_from_slice(&pa.to_le_bytes());
            p[16..20].copy_from_slice(&vas.to_le_bytes());
            reg.record_set_page_directory(1, &p)
        };
        let Target::Fb(r) = root else { unreachable!() };
        // The paging channel (chid 1) in VAS 0x20, a GR channel (chid 2) in VAS 0x21.
        reg.record_alloc(1, 0x10, 0x20, FERMI_VASPACE_A, &[]);
        set_pd(&mut reg, 0x20, r);
        reg.record_alloc(1, 0x10, 0x21, FERMI_VASPACE_A, &[]);
        let (old, new) = (Target::Fb(0x3b48_5000), Target::Fb(0x0d3f_0000));
        set_pd(&mut reg, 0x21, 0x3b48_5000);
        let paging = channel_params(1, 0x1000_0000, 0x20_0000, 0x20);
        reg.record_alloc(1, 0x10, 0x40, TURING_CHANNEL_GPFIFO_A, &paging);
        let gr = channel_params(2, 0x1000_1000, 0x20_1000, 0x21);
        reg.record_alloc(1, 0x10, 0x41, TURING_CHANNEL_GPFIFO_A, &gr);
        set_pd(&mut reg, 0x21, 0x0d3f_0000);
        assert_eq!(reg.root_mismatch(2), None);
        // MEM_OP A..D: TLB invalidate (OPERATION 9) of PDB ONE 0x3b485000 in vidmem, as the
        // Windows paging channel pushed it in P17 (C 0x3b4852a0).
        let pb = inc(0, 0x28, &[0, 0, 0x3b48_52a0, 9 << 27]);
        let bytes: Vec<u8> = pb.iter().flat_map(|d| d.to_le_bytes()).collect();
        assert!(sys.write(0x4000_0000, &bytes));
        let mut gp = 0x1000_8000u32.to_le_bytes().to_vec();
        gp.extend_from_slice(&((pb.len() as u32) << 10).to_le_bytes());
        fb.write(0x100000, &gp);
        fb.write(0x20_0000 + USERD_GP_PUT, &1u32.to_le_bytes());
        let mut mem = GpuMem {
            fb: &mut fb,
            sys: &sys,
        };
        let all = reg.doorbell(&mut mem, 1).join("\n");
        assert!(all.contains("pdb Fb(994594816)"), "{all}");
        assert_eq!(reg.tlb_invalidates, vec![Some(old)]);
        assert_eq!(reg.root_mismatch(1), None);
        let (vas, m) = reg
            .root_mismatch(2)
            .expect("chid 2 walks a root the guest left");
        assert_eq!((vas, m.named, m.walked, m.count), ((1, 0x21), old, new, 1));
        assert!(reg.pdb_watch_mut().first_report(vas, &m));
        // The guest names the walked root (a register invalidate): released.
        reg.note_guest_invalidate(Some(new));
        assert_eq!(reg.root_mismatch(2), None);
        // The former root named again: flagged again; the VAS's FREE forgets it.
        reg.note_guest_invalidate(Some(old));
        assert!(reg.root_mismatch(2).is_some());
        reg.record_free(1, 0x21);
        assert_eq!(reg.root_mismatch(2), None);
        assert_eq!(reg.pdb_watch().invalidates, 3);
        let _ = std::fs::remove_file(&path);
    }

    /// libnvidia-encode's NVENC channel: a bare channel under the device, no VAS of its
    /// own. Its VAS is the device's default VAS, which the kernel announced to GSP through
    /// a transient GPU_DEVICE-index handle (gpu_vaspace.c:3877-3921) that it then freed.
    #[test]
    fn a_bare_video_channel_runs_in_the_device_default_vas() {
        let mut reg = Registry::new(classes());
        let mut vp = vec![0u8; 56];
        vp[..4].copy_from_slice(&VAS_INDEX_GPU_DEVICE.to_le_bytes());
        reg.record_alloc(1, 0x80, 0xcaf0_0000, FERMI_VASPACE_A, &vp);
        let mut pdes = vec![0u8; 40 + 24 * 6];
        pdes[40..48].copy_from_slice(&0x1_2729_d000u64.to_le_bytes());
        pdes[56..60].copy_from_slice(&1u32.to_le_bytes());
        let note = reg.record_server_reserved_pdes(1, 0xcaf0_0000, &pdes);
        assert!(note.contains("default VAS of device 0x80"), "{note}");
        reg.record_free(1, 0xcaf0_0000);
        // NVENC0 (engineType 0x1b), parent the device, hVASpace 0.
        let mut cp = channel_params(22, 0x1_2007_0000, 0x20_0000, 0);
        let o = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_engineType;
        cp[o..o + 4].copy_from_slice(&0x1Bu32.to_le_bytes());
        reg.record_alloc(1, 0x80, 0x14, TURING_CHANNEL_GPFIFO_A, &cp)
            .unwrap();
        let ch = reg.channel(22).unwrap().clone();
        assert_eq!(ch.vas, None);
        assert_eq!(reg.vas_key_of(&ch), Some((1, 0x80)));
        assert_eq!(
            reg.vas_root(reg.vas_key_of(&ch).unwrap()),
            Some(Target::Fb(0x1_2729_d000))
        );
        assert_eq!(reg.chid_of(1, 0x14), Some(22));
        // A VAS allocated with index NEW (0) is no device default.
        reg.record_alloc(2, 0x80, 0x21, FERMI_VASPACE_A, &[0u8; 56]);
        let n = reg.record_server_reserved_pdes(2, 0x21, &pdes);
        assert!(!n.contains("default VAS"), "{n}");
        assert_eq!(reg.vas_root((2, 0x80)), None);
    }

    /// The Windows KMD opens one client per channel whose device shares the main client's VA
    /// space (hClientShare): a channel there that names no VAS runs in the share client's
    /// device default VAS.
    #[test]
    fn a_channel_of_a_sharing_client_runs_in_the_share_clients_default_vas() {
        let mut reg = Registry::new(classes());
        let mut vp = vec![0u8; 56];
        vp[..4].copy_from_slice(&VAS_INDEX_GPU_DEVICE.to_le_bytes());
        reg.record_alloc(1, 0x80, 0xcaf0_0000, FERMI_VASPACE_A, &vp);
        let mut pdes = vec![0u8; 40 + 24 * 6];
        pdes[40..48].copy_from_slice(&0x1_2729_d000u64.to_le_bytes());
        pdes[56..60].copy_from_slice(&1u32.to_le_bytes());
        reg.record_server_reserved_pdes(1, 0xcaf0_0000, &pdes);
        reg.record_free(1, 0xcaf0_0000);
        // Client 2's device shares client 1.
        let mut dp = vec![0u8; 56];
        dp[4..8].copy_from_slice(&1u32.to_le_bytes());
        let n = reg
            .record_alloc(2, 2, 0x90, NV01_DEVICE_0_CLASS, &dp)
            .unwrap();
        assert!(n.contains("shares the VA space of client 0x1"), "{n}");
        let mut cp = channel_params(5, 0x1_2007_0000, 0x20_0000, 0);
        let o = <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_engineType;
        cp[o..o + 4].copy_from_slice(&0xBu32.to_le_bytes());
        reg.record_alloc(2, 0x90, 0x14, TURING_CHANNEL_GPFIFO_A, &cp)
            .unwrap();
        let ch = reg.channel(5).unwrap().clone();
        assert_eq!(reg.root_of(&ch), Some(Target::Fb(0x1_2729_d000)));
        assert_eq!(reg.vas_key_of(&ch), Some((1, 0x80)));
        // A device without a share records nothing.
        assert!(reg
            .record_alloc(3, 3, 0x91, NV01_DEVICE_0_CLASS, &[0u8; 56])
            .is_none());
    }

    /// Windows allocates its default VAS with index NEW, selects it explicitly, then opens
    /// per-channel clients whose devices share the main client. The association must select
    /// that VAS even when another rooted VAS makes the old sole-root heuristic ambiguous.
    #[test]
    fn explicit_default_vas_is_live_for_sharing_clients_and_ignores_empty_pdes() {
        let mut reg = Registry::new(classes());
        reg.record_alloc(1, 1, 0x80, NV01_DEVICE_0_CLASS, &[0u8; 56]);

        let mut vas = vec![0u8; 56];
        vas[4..8].copy_from_slice(&5u32.to_le_bytes());
        reg.record_alloc(1, 0x80, 0x21, FERMI_VASPACE_A, &vas);
        reg.record_alloc(1, 0x80, 0x22, FERMI_VASPACE_A, &vas);
        let pdes = |root: u64| {
            let mut p = vec![0u8; v::PDES_LEVELS + v::PDES_LEVEL_SIZE * v::PDES_MAX_LEVELS];
            p[v::PDES_VIRT_ADDR_LO..][..8].copy_from_slice(&0x1_0000_0000u64.to_le_bytes());
            p[v::PDES_VIRT_ADDR_HI..][..8].copy_from_slice(&0x1_3fff_ffffu64.to_le_bytes());
            p[v::PDES_NUM_LEVELS..][..4].copy_from_slice(&1u32.to_le_bytes());
            let l = v::PDES_LEVELS;
            p[l + v::PDES_LEVEL_PHYS_ADDRESS..][..8].copy_from_slice(&root.to_le_bytes());
            p[l + v::PDES_LEVEL_SIZE_FIELD..][..8].copy_from_slice(&0x20u64.to_le_bytes());
            p[l + v::PDES_LEVEL_APERTURE..][..4].copy_from_slice(&1u32.to_le_bytes());
            p
        };
        reg.record_server_reserved_pdes(1, 0x21, &pdes(0x39da_2000));
        reg.record_server_reserved_pdes(1, 0x22, &pdes(0x7abc_0000));
        reg.record_set_default_vaspace(1, 0x80, 0x21).unwrap();

        let mut device = vec![0u8; 56];
        device[4..8].copy_from_slice(&1u32.to_le_bytes());
        reg.record_alloc(2, 2, 0x80, NV01_DEVICE_0_CLASS, &device);
        reg.record_alloc(
            2,
            0x80,
            0x14,
            TURING_CHANNEL_GPFIFO_A,
            &channel_params(14, 0x1_2007_0000, 0x20_0000, 0),
        );
        let ch = reg.channel(14).unwrap().clone();
        assert_eq!(reg.vas_key_of(&ch), Some((1, 0x21)));
        assert_eq!(reg.root_of(&ch), Some(Target::Fb(0x39da_2000)));

        // The mapping is live: replacing the selected VAS's root changes the shared client.
        let newest = pdes(0x4bcd_3000);
        reg.record_server_reserved_pdes(1, 0x21, &newest);
        assert_eq!(reg.root_of(&ch), Some(Target::Fb(0x4bcd_3000)));

        // RM reports no server-owned PDE interval as VA 1..0 with PA 0. This is a sentinel,
        // not a replacement root, and must retain the real PDE block for mirror replay.
        let mut empty = vec![0u8; v::PDES_LEVELS + v::PDES_LEVEL_SIZE * v::PDES_MAX_LEVELS];
        empty[v::PDES_VIRT_ADDR_LO..][..8].copy_from_slice(&1u64.to_le_bytes());
        empty[v::PDES_NUM_LEVELS..][..4].copy_from_slice(&1u32.to_le_bytes());
        let note = reg.record_server_reserved_pdes(1, 0x21, &empty);
        assert!(note.contains("empty server-reserved range"), "{note}");
        assert_eq!(reg.root_of(&ch), Some(Target::Fb(0x4bcd_3000)));
        assert_eq!(reg.vas_replay((1, 0x21)).1, Some(newest.as_slice()));

        // deviceSetDefaultVASpace takes a reference: freeing the public VAS handle keeps it.
        reg.record_free(1, 0x21);
        assert_eq!(reg.vas_key_of(&ch), Some((1, 0x80)));
        assert_eq!(reg.root_of(&ch), Some(Target::Fb(0x4bcd_3000)));
    }

    #[test]
    fn freeing_a_shared_device_clears_its_client_share_before_handle_reuse() {
        let mut reg = Registry::new(classes());
        let mut shared = vec![0u8; 56];
        shared[4..8].copy_from_slice(&1u32.to_le_bytes());
        reg.record_alloc(2, 2, 0x80, NV01_DEVICE_0_CLASS, &shared);
        assert_eq!(reg.client_share.get(&2), Some(&1));

        reg.record_free(2, 0x80);
        assert_eq!(reg.client_share.get(&2), None);
        reg.record_alloc(2, 2, 0x80, NV01_DEVICE_0_CLASS, &[0u8; 56]);
        assert_eq!(reg.client_share.get(&2), None);
        assert_eq!(reg.shared_default_key(2), None);
    }

    /// A falcon engine promotes its context by VA alone (entryCount 0, the top-level
    /// virtAddress and size, falcon/kernel_falcon.c:217-271): recorded, so the shadow
    /// keeps the range out of set A.
    #[test]
    fn a_falcon_context_promote_is_recorded_by_its_va() {
        let mut reg = Registry::new(classes());
        let mut p = vec![0u8; v::PROMOTE_PARAMS_SIZE];
        p[v::PROMOTE_ENGINE_TYPE..][..4].copy_from_slice(&0x1Bu32.to_le_bytes());
        p[v::PROMOTE_H_CHAN_CLIENT..][..4].copy_from_slice(&1u32.to_le_bytes());
        p[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&0x14u32.to_le_bytes());
        p[v::PROMOTE_VIRT_ADDRESS..][..8].copy_from_slice(&0x1_2002_d000u64.to_le_bytes());
        p[v::PROMOTE_SIZE..][..8].copy_from_slice(&0x1000u64.to_le_bytes());
        let note = reg.record_promote_ctx(&p);
        assert!(
            note.contains("falcon context va 0x12002d000 size 0x1000"),
            "{note}"
        );
        let e = &reg.promoted((1, 0x14)).unwrap()[&PROMOTE_ID_FALCON_CTX];
        assert_eq!((e.virt, e.size), (Some(0x1_2002_d000), 0x1000));
        // A GR promote with entries leaves the top-level VA alone.
        let mut g = vec![0u8; v::PROMOTE_PARAMS_SIZE];
        g[v::PROMOTE_H_CHAN_CLIENT..][..4].copy_from_slice(&1u32.to_le_bytes());
        g[v::PROMOTE_H_OBJECT..][..4].copy_from_slice(&0x15u32.to_le_bytes());
        g[v::PROMOTE_VIRT_ADDRESS..][..8].copy_from_slice(&0x5000u64.to_le_bytes());
        g[v::PROMOTE_ENTRY_COUNT..][..4].copy_from_slice(&1u32.to_le_bytes());
        g[v::PROMOTE_ENTRIES + 8..][..8].copy_from_slice(&0x7000u64.to_le_bytes());
        reg.record_promote_ctx(&g);
        let m = reg.promoted((1, 0x15)).unwrap();
        assert!(!m.contains_key(&PROMOTE_ID_FALCON_CTX));
        assert_eq!(m[&0].virt, Some(0x7000));
    }
}

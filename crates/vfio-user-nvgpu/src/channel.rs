// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
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
//! allocation and control parameter layouts are the emulated driver version's
//! ([`crate::ver`]).

use std::collections::BTreeMap;

use nvrm_sys::RmAbi;

use crate::chip::ChannelClasses;
use crate::fb::{FakeFb, Target};
use crate::guestmem::GuestMemory;
use crate::ver::active::{
    self as v, ADDR_FBMEM, ADDR_SYSMEM, CTXSHARE_HVASPACE, FERMI_CONTEXT_SHARE_A, FERMI_VASPACE_A,
    KEPLER_CHANNEL_GROUP_A, TSG_ENGINE_TYPE, TSG_HVASPACE,
};

/// The channel parameter offsets of the emulated driver version.
type Abi = v::RmAbi;

/// USERD offsets of GPGet and GPPut (`clc46f.h:61-62`).
const USERD_GP_GET: u64 = 0x88;
const USERD_GP_PUT: u64 = 0x8C;

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
    /// so every larger page splits consistently) into physical pieces.
    fn pieces(&self, root: Target, va: u64, len: u64) -> Result<Vec<(Target, usize)>, String> {
        let mut out = Vec::new();
        let mut done = 0u64;
        while done < len {
            let a = va + done;
            let n = (0x1000 - (a & 0xFFF)).min(len - done);
            let t = self.fb.translate_in(Some(self.sys), root, a);
            match t {
                Target::Fb(_) | Target::Sys(_) => out.push((t, n as usize)),
                other => return Err(format!("va {a:#x}: {other:?}")),
            }
            done += n;
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
    sem_addr_lo: u32,
    sem_addr_hi: u32,
    sem_payload_lo: u32,
    sem_payload_hi: u32,
    mem_op: [u32; 3],
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
    /// Runs on a host mirror channel: the software executor leaves it alone.
    pub mirrored: bool,
    pub chid: u32,
    pub engine_type: u32,
    pub gpfifo_va: u64,
    pub gpfifo_entries: u32,
    /// USERD (GPGet/GPPut page), from `userdMem`.
    pub userd: Option<Target>,
    /// The VAS the channel runs in: its own `hVASpace` or its TSG's.
    pub vas: Option<(u32, u32)>,
    pub error_notifier: Option<Target>,
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
    pub entries_done: u64,
    pub methods_done: u64,
    /// Compute work (kernel launches) the device refused to run: part 2.
    pub launches_refused: u64,
    /// `MEM_OP` TLB invalidates of this channel's pushes since the registry drained them.
    tlb_invalidates: Vec<Option<u64>>,
}

/// Everything the device knows about the guest's channels.
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
    /// (hClient, hCtxShare) → hVASpace (`FERMI_CONTEXT_SHARE_A`): GR channels name their
    /// VAS only through their context share (cuda-lookahead.md §5 item 2).
    ctxshares: BTreeMap<(u32, u32), u32>,
    /// (hClient, hObject) of a `DUP_OBJECT` → its source, for VA spaces: UVM duplicates
    /// CUDA's VAS and sets the page directory on its own handle (§5 item 1).
    aliases: BTreeMap<(u32, u32), (u32, u32)>,
    /// (hChanClient, hObject) → promoted context buffers by buffer id.
    promoted: BTreeMap<(u32, u32), BTreeMap<u16, PromoteEntry>>,
    /// ChID → channel.
    channels: BTreeMap<u32, Channel>,
    /// Device clock base for semaphore timestamps.
    start: Option<std::time::Instant>,
    pub doorbells: u64,
    /// TLB invalidates the software executor ran into (`MEM_OP` MMU_TLB_INVALIDATE):
    /// the page directory (`None`: all), for the host mirrors of the same page
    /// directories (part 2); drained by the dispatcher after each doorbell.
    pub tlb_invalidates: Vec<Option<u64>>,
    /// The doorbell token pool (`CARAXES_DOORBELL_TOKENS`, tokens.rs): which channel
    /// holds which pool token.
    pub tokens: Option<crate::tokens::TokenPool>,
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
            ctxshares: BTreeMap::new(),
            aliases: BTreeMap::new(),
            promoted: BTreeMap::new(),
            channels: BTreeMap::new(),
            start: Some(std::time::Instant::now()),
            doorbells: 0,
            tlb_invalidates: Vec::new(),
            tokens: None,
        }
    }

    pub fn channels(&self) -> impl Iterator<Item = &Channel> {
        self.channels.values()
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
        let mut roots = self
            .vaspaces
            .iter()
            .filter(|((c, _), r)| *c == ch.h_client && r.is_some());
        match (roots.next(), roots.next()) {
            (Some((k, _)), None) => Some(*k),
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
        Some(format!(
            "vaspace alias {:#x}/{:#x} -> {:#x}/{:#x} [guest]",
            dst.0, dst.1, src.0, src.1
        ))
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
        format!(
            "promote ctx engineType {:#x} for {:#x}/{:#x}: {n} entries [{}] [guest, recorded]",
            rd32(params, v::PROMOTE_ENGINE_TYPE),
            key.0,
            key.1,
            desc.join("; ")
        )
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
            FERMI_VASPACE_A => {
                self.vaspaces.insert((h_client, h_object), None);
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
                Some(format!(
                    "tsg {h_object:#x} engineType {engine:#x} vaspace {vas:#x} [guest]"
                ))
            }
            c if c == self.classes.gpfifo => {
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
                    mirrored: false,
                    chid,
                    engine_type: rd32(params, <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_engineType),
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
                    gp_get: 0,
                    gp_put_seen: 0,
                    resume: None,
                    objects: [0; 8],
                    ce: CeState::default(),
                    compute: ComputeState::default(),
                    host: HostState::default(),
                    entries_done: 0,
                    methods_done: 0,
                    launches_refused: 0,
                    tlb_invalidates: Vec::new(),
                };
                let mut note = format!(
                    "channel {h_object:#x} chid {chid} engineType {:#x} gpfifo va {:#x} x {} userd {:?} vaspace {:?} errorNotifier {:?} [guest]",
                    ch.engine_type, ch.gpfifo_va, ch.gpfifo_entries, ch.userd, ch.vas, ch.error_notifier
                );
                // A ChID the guest reuses without a FREE the device saw: the old
                // channel's pool token must not ring the new one.
                if let Some(t) = self.tokens.as_mut().and_then(|p| p.release(chid)) {
                    note.push_str(&format!(
                        "; doorbell token {t:#x} of the previous chid {chid} released"
                    ));
                }
                self.channels.insert(chid, ch);
                Some(note)
            }
            // An engine object (copy, compute, the UVM SW class, ...) under a channel:
            // kept with its parameters for the host mirror.
            _ => {
                let ch = self
                    .channels
                    .values_mut()
                    .find(|c| c.h_client == h_client && c.h_object == h_parent)?;
                ch.engine_objects.push((h_class, params.to_vec()));
                Some(format!(
                    "engine object class {h_class:#x} on chid {} ({} bytes of params) [guest]",
                    ch.chid,
                    params.len()
                ))
            }
        }
    }

    /// Forget a freed object (a freed client takes its objects with it).
    pub fn record_free(&mut self, h_client: u32, h_object: u32) {
        let all = h_client == h_object;
        let gone = |c: &Channel| c.h_client == h_client && (all || c.h_object == h_object);
        // A freed channel gives its doorbell token back (docs/caraxes/doorbell-token-pool.md).
        if let Some(pool) = self.tokens.as_mut() {
            for c in self.channels.values().filter(|c| gone(c)) {
                pool.release(c.chid);
            }
        }
        self.channels.retain(|_, c| !gone(c));
        self.tsgs
            .retain(|(c, h), _| *c != h_client || !(all || *h == h_object));
        self.vaspaces
            .retain(|(c, h), _| *c != h_client || !(all || *h == h_object));
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
        let root = gmmu_aperture(aperture, pa);
        self.vaspaces.insert((h_client, h_vas), root);
        let entries = (rd64(params, level(0) + v::PDES_LEVEL_SIZE_FIELD) / 8) as u32;
        self.root_entries.insert((h_client, h_vas), entries.max(1));
        self.server_pdes.insert((h_client, h_vas), params.to_vec());
        format!(
            "vaspace {h_vas:#x}: root {root:?}; va {:#x}..{:#x} levels [{}] [guest]",
            rd64(params, v::PDES_VIRT_ADDR_LO),
            rd64(params, v::PDES_VIRT_ADDR_HI),
            levels.join(", ")
        )
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
        self.vaspaces.insert(key, root);
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
        format!("unset page directory: vaspace {h_vas:#x} [guest]")
    }

    fn root_of(&self, ch: &Channel) -> Option<Target> {
        if let Some(k) = ch.vas {
            if let Some(r) = self.vas_root(k) {
                return Some(r);
            }
        }
        // A channel of the device's default VAS: the only root of that client.
        let mut roots = self
            .vaspaces
            .iter()
            .filter(|((c, _), r)| *c == ch.h_client && r.is_some());
        match (roots.next(), roots.next()) {
            (Some((_, r)), None) => *r,
            _ => None,
        }
    }

    /// A doorbell write: run the channel the token names, then retry channels
    /// that were blocked on a semaphore acquire.
    pub fn doorbell(&mut self, mem: &mut GpuMem, token: u32) -> Vec<String> {
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
        notes.extend(self.run(mem, chid));
        let blocked: Vec<u32> = self
            .channels
            .iter()
            .filter(|(c, ch)| **c != chid && ch.resume.is_some())
            .map(|(c, _)| *c)
            .collect();
        for c in blocked {
            notes.extend(self.run(mem, c));
        }
        notes
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
        while ch.gp_get != gp_put % entries && guard < entries {
            guard += 1;
            let mut e = [0u8; 8];
            let entry_va = ch.gpfifo_va + 8 * ch.gp_get as u64;
            if let Err(err) = mem.read_va(root, entry_va, &mut e) {
                notes.push(format!("chid {chid}: gpfifo entry {}: {err}", ch.gp_get));
                break;
            }
            let (e0, e1) = (rd32(&e, 0), rd32(&e, 4));
            // GP_ENTRY0 GET 31:2, GP_ENTRY1 GET_HI 7:0, LENGTH 30:10 (dwords), OPCODE 7:0
            // when LENGTH is 0 (clc46f.h:264-282).
            let length = (e1 >> 10) & 0x1F_FFFF;
            if length == 0 {
                notes.push(format!(
                    "chid {chid} gp {}: control entry opcode {} (no pushbuffer)",
                    ch.gp_get,
                    e1 & 0xFF
                ));
            } else {
                let pb_va = ((e1 as u64 & 0xFF) << 32) | (e0 as u64 & !3);
                match Self::segment(ch, mem, root, pb_va, length as usize, now, &classes) {
                    Ok((seg_notes, blocked)) => {
                        notes.push(format!(
                            "chid {chid} gp {}: pushbuffer va {pb_va:#x} {} dwords{}",
                            ch.gp_get,
                            length,
                            if blocked { ", blocked on acquire" } else { "" }
                        ));
                        notes.extend(seg_notes);
                        if blocked {
                            break;
                        }
                    }
                    Err(err) => {
                        notes.push(format!("chid {chid} gp {}: {err}", ch.gp_get));
                        break;
                    }
                }
            }
            ch.gp_get = (ch.gp_get + 1) % entries;
            ch.entries_done += 1;
            if !mem.write(userd_at(USERD_GP_GET), &ch.gp_get.to_le_bytes()) {
                notes.push(format!("chid {chid}: GPGet write to {userd:?} failed"));
            }
        }
        let inv = std::mem::take(&mut ch.tlb_invalidates);
        self.tlb_invalidates.extend(inv);
        notes
    }

    /// Execute one pushbuffer segment. Returns the notes and whether a host
    /// semaphore acquire blocked (the entry is then resumed at that method).
    fn segment(
        ch: &mut Channel,
        mem: &mut GpuMem,
        root: Target,
        va: u64,
        dwords: usize,
        now: u64,
        classes: &ChannelClasses,
    ) -> Result<(Vec<String>, bool), String> {
        let mut raw = vec![0u8; dwords * 4];
        mem.read_va(root, va, &mut raw)
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
                // Methods below 0x100 are the host's; the rest go to the class bound to
                // the subchannel. A subchannel without SET_OBJECT is taken as the copy
                // engine (UVM's CE channels, E5a).
                let class = ch.objects[subch];
                let r = if a < 0x100 {
                    Self::host_method(ch, mem, root, subch, a, data, now)
                } else if class == classes.uvm_sw {
                    Ok(Some(format!(
                        "sw method subch {subch} {a:#x} = {data:#x} ignored (GP100_UVM_SW)"
                    )))
                } else if class == classes.compute {
                    Ok(Self::compute_method(ch, mem, root, a, data, now))
                } else if class == classes.copy || class == 0 {
                    Self::ce_method(ch, mem, root, a, data, now)
                } else {
                    Ok(Some(format!(
                        "subch {subch} class {class:#x} method {a:#x} = {data:#x} skipped (class not modelled)"
                    )))
                };
                match r {
                    Ok(Some(n)) => notes.push(n),
                    Ok(None) => {}
                    Err(Blocked(n)) => {
                        notes.push(n);
                        ch.resume = Some(pos);
                        return Ok((notes, true));
                    }
                }
            }
        }
        Ok((notes, false))
    }

    fn host_method(
        ch: &mut Channel,
        mem: &mut GpuMem,
        root: Target,
        subch: usize,
        addr: u32,
        data: u32,
        now: u64,
    ) -> Result<Option<String>, Blocked> {
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
            0x20 => Some("host NON_STALL_INTERRUPT (no interrupt raised)".into()),
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
            // invalidate is queued for the host mirrors of the same page directory
            // (MEM_OP_C PDB bit 0 = one PDB, ADDR_LO 31:12; MEM_OP_D ADDR_HI 26:0).
            0x34 => {
                let op = data >> 27;
                let what = match op {
                    5 => "membar",
                    9 => "tlb invalidate",
                    0xA => "targeted tlb invalidate",
                    0x16 => "access counter clear",
                    _ => "l2 or other",
                };
                let mut forwarded = "";
                if op == 9 || op == 0xA {
                    let c = h.mem_op[2];
                    let pdb = (c & 1 == 1)
                        .then_some((c as u64 & 0xFFFF_F000) | ((data as u64 & 0x7FF_FFFF) << 32));
                    ch.tlb_invalidates.push(pdb);
                    forwarded = "; queued for the host mirrors";
                }
                Some(format!(
                    "host MEM_OP {what} (op {op:#x}, A {:#x} B {:#x} C {:#x}): no cache to act on{forwarded}",
                    h.mem_op[0], h.mem_op[1], h.mem_op[2]
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
                        let r = if data & (1 << 25) != 0 {
                            let mut b = [0u8; 16];
                            b[..8].copy_from_slice(&payload.to_le_bytes());
                            b[8..].copy_from_slice(&now.to_le_bytes());
                            mem.write_va(root, va, &b)
                        } else if wide {
                            mem.write_va(root, va, &payload.to_le_bytes())
                        } else {
                            mem.write_va(root, va, &(payload as u32).to_le_bytes())
                        };
                        Some(match r {
                            Ok(()) => format!(
                                "host semaphore release va {va:#x} payload {payload:#x}{} [guest]",
                                if data & (1 << 25) != 0 {
                                    " + timestamp [synthesized]"
                                } else {
                                    ""
                                }
                            ),
                            Err(e) => format!("host semaphore release va {va:#x}: {e}"),
                        })
                    }
                    op @ (0 | 2 | 3) => {
                        let mut b = [0u8; 8];
                        let n = if wide { 8 } else { 4 };
                        if let Err(e) = mem.read_va(root, va, &mut b[..n]) {
                            return Ok(Some(format!(
                                "host semaphore acquire va {va:#x}: {e}, treated as satisfied"
                            )));
                        }
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
                            return Err(Blocked(format!(
                                "host semaphore acquire va {va:#x} op {op}: {cur:#x} vs {want:#x}, channel waits"
                            )));
                        }
                        None
                    }
                    op => Some(format!("host SEM_EXECUTE op {op} not modelled")),
                }
            }
            0x78 => None,
            a => Some(format!("host method {a:#x} = {data:#x} not modelled")),
        })
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
    ) -> Result<Option<String>, Blocked> {
        let c = &mut ch.ce;
        match addr {
            0x100 => {}
            0x240 => c.sem_upper = data & 0x1_FFFF,
            0x244 => c.sem_lower = data,
            0x248 => c.sem_payload = data,
            0x260 => c.src_phys = data,
            0x264 => c.dst_phys = data,
            0x300 => return Ok(Some(Self::launch_dma(c, mem, root, data, now))),
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
            a => return Ok(Some(format!("ce method {a:#x} = {data:#x} not modelled"))),
        }
        Ok(None)
    }

    /// `LAUNCH_DMA` (clc5b5.h:68-138): the transfer first, then the semaphore.
    fn launch_dma(c: &CeState, mem: &mut GpuMem, root: Target, v: u32, now: u64) -> String {
        let mut parts = Vec::new();
        let transfer = v & 3;
        let multi = v & (1 << 9) != 0;
        let remap = v & (1 << 10) != 0;
        let src_phys = v & (1 << 12) != 0;
        let dst_phys = v & (1 << 13) != 0;
        let src = ((c.offset_in.0 as u64) << 32) | c.offset_in.1 as u64;
        let dst = ((c.offset_out.0 as u64) << 32) | c.offset_out.1 as u64;
        // SET_*_PHYS_MODE TARGET 1:0: 0 LOCAL_FB, 1 COHERENT_SYSMEM, 2 NONCOHERENT_SYSMEM.
        let phys = |mode: u32, pa: u64| {
            if mode & 3 == 0 {
                Target::Fb(pa)
            } else {
                Target::Sys(pa)
            }
        };
        if transfer != 0 {
            let lines = if multi { c.line_count.max(1) } else { 1 };
            let r: Result<u64, String> = (|| {
                let mut bytes = 0u64;
                for line in 0..lines as u64 {
                    let (s_at, d_at) = if multi {
                        (
                            src + line * c.pitch_in as u64,
                            dst + line * c.pitch_out as u64,
                        )
                    } else {
                        (src, dst)
                    };
                    let data = if remap {
                        remap_line(c)?
                    } else {
                        let mut b = vec![0u8; c.line_length_in as usize];
                        if src_phys {
                            if !mem.read(phys(c.src_phys, s_at), &mut b) {
                                return Err(format!("source pa {s_at:#x} unreadable"));
                            }
                        } else {
                            mem.read_va(root, s_at, &mut b)?;
                        }
                        b
                    };
                    if dst_phys {
                        if !mem.write(phys(c.dst_phys, d_at), &data) {
                            return Err(format!("destination pa {d_at:#x} unwritable"));
                        }
                    } else {
                        mem.write_va(root, d_at, &data)?;
                    }
                    bytes += data.len() as u64;
                }
                Ok(bytes)
            })();
            let kind = if remap { "memset" } else { "memcopy" };
            let dst_desc = if dst_phys {
                format!("{:?}", phys(c.dst_phys, dst))
            } else {
                format!("va {dst:#x}")
            };
            match r {
                Ok(n) => parts.push(format!(
                    "ce {kind} {n} bytes -> {dst_desc}{} [guest]",
                    if remap {
                        format!(" value {:#x}/{:#x}", c.remap_const_a, c.remap_const_b)
                    } else if src_phys {
                        format!(" from {:?}", phys(c.src_phys, src))
                    } else {
                        format!(" from va {src:#x}")
                    }
                )),
                Err(e) => parts.push(format!("ce {kind} -> {dst_desc} FAILED: {e}")),
            }
        }
        // SEMAPHORE_TYPE 4:3: 1 one-word, 2 four-word (payload, 0, 64-bit timestamp).
        let sem_va = ((c.sem_upper as u64) << 32) | c.sem_lower as u64;
        match (v >> 3) & 3 {
            0 => {}
            1 => {
                let payload = if v & (1 << 19) != 0 {
                    let mut b = [0u8; 4];
                    let old = match mem.read_va(root, sem_va, &mut b) {
                        Ok(()) => u32::from_le_bytes(b),
                        Err(e) => {
                            parts.push(format!("ce semaphore va {sem_va:#x}: {e}"));
                            return parts.join("; ");
                        }
                    };
                    reduce((v >> 14) & 0xF, v & (1 << 18) != 0, old, c.sem_payload)
                } else {
                    c.sem_payload
                };
                parts.push(match mem.write_va(root, sem_va, &payload.to_le_bytes()) {
                    Ok(()) => format!("ce semaphore release va {sem_va:#x} = {payload:#x} [guest]"),
                    Err(e) => format!("ce semaphore release va {sem_va:#x}: {e}"),
                });
            }
            2 => {
                let mut b = [0u8; 16];
                b[..4].copy_from_slice(&c.sem_payload.to_le_bytes());
                b[8..].copy_from_slice(&now.to_le_bytes());
                parts.push(match mem.write_va(root, sem_va, &b) {
                    Ok(()) => format!(
                        "ce semaphore release (four words) va {sem_va:#x} = {:#x} [guest] + timestamp {now} [synthesized]",
                        c.sem_payload
                    ),
                    Err(e) => format!("ce semaphore release va {sem_va:#x}: {e}"),
                });
            }
            t => parts.push(format!("ce semaphore type {t} not modelled")),
        }
        if (v >> 5) & 3 != 0 {
            parts.push("interrupt requested (not raised)".into());
        }
        if parts.is_empty() {
            parts.push(format!("ce LAUNCH_DMA {v:#x}: nothing to do"));
        }
        parts.join("; ")
    }
}

/// The error type of a method that has to wait.
struct Blocked(String);

/// One memset line from the remap constants: LINE_LENGTH_IN elements of
/// NUM_DST_COMPONENTS (25:24, +1) components of COMPONENT_SIZE (17:16, +1)
/// bytes, each component from CONST_A (4) or CONST_B (5) (clc5b5.h:155-206).
fn remap_line(c: &CeState) -> Result<Vec<u8>, String> {
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
    let n = c.line_length_in as usize;
    if n.saturating_mul(elem.len()) > 1 << 30 {
        return Err(format!("memset of {n} elements refused"));
    }
    Ok(elem.repeat(n))
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
        assert!(reg.pending(&mem, 9), "a software channel is pending until GPGet catches up");
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
        let _ = std::fs::remove_file(&path);
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
}

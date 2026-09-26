// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Answers for the RM API RPCs of phase 5 (docs/caraxes/phase5-rpcs.md), one handler
//! per row, each labelled with the provenance of its answer (decision D9): `host`
//! (forwarded to the host RM as an unprivileged client), `guest` (derived from what
//! the guest sent), `card` (captured), `synthesized` (a constant chosen here).
//! Layouts come from the SDK headers named in the inventory; sizes are asserted in
//! the tests against the typed bindings in `nvrm-sys` where those exist.

use std::collections::BTreeMap;

use crate::chip::ChipData;
use crate::rpc::StaticIdentity;
// Classes, control numbers and parameter layouts of the emulated driver (src/ver).
use crate::ver::active as v;
pub use crate::ver::active::{
    CMD_BIOS_GET_INFO_V2, CMD_BUS_GET_PCI_BAR_INFO, CMD_CE_GET_FAULT_METHOD_BUFFER_SIZE,
    CMD_CE_UPDATE_CLASS_DB, CMD_CE_UPDATE_PCE_LCE_MAPPINGS_V2, CMD_EVENT_SET_NOTIFICATION,
    CMD_FB_GET_GPU_CACHE_INFO, CMD_FIFO_GET_DEVICE_INFO_TABLE, CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES, CMD_GR_CTXSW_ZCULL_BIND, CMD_GPU_GET_CONSTRUCTED_FALCON_INFO,
    CMD_GPU_GET_OEM_BOARD_INFO, CMD_GPU_GET_VMMU_SEGMENT_SIZE, CMD_GSP_GET_FEATURES,
    CMD_I2C_GET_PORT_INFO, CMD_INTERNAL_BIF_GET_STATIC_INFO, CMD_INTERNAL_BUS_FLUSH_WITH_SYSMEMBAR,
    CMD_INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_TYPE, CMD_INTERNAL_FIFO_GET_NUM_CHANNELS,
    CMD_INTERNAL_GET_PCIE_P2P_CAPS, CMD_INTERNAL_GMMU_GET_STATIC_INFO,
    CMD_INTERNAL_GPU_GET_CHIP_INFO, CMD_INTERNAL_GPU_GET_USER_REGISTER_ACCESS_MAP,
    CMD_INTERNAL_INIT_USER_SHARED_DATA, CMD_INTERNAL_INTR_GET_KERNEL_TABLE,
    CMD_INTERNAL_MEMSYS_GET_STATIC_CONFIG, CMD_INTERNAL_MEMSYS_L2_INVALIDATE_EVICT,
    CMD_INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO, CMD_INTERNAL_PERF_GPU_BOOST_SYNC_GET_INFO,
    CMD_INTERNAL_USER_SHARED_DATA_SET_DATA_POLL, NV01_DEVICE_0, NV01_EVENT_KERNEL_CALLBACK_EX,
    NV01_ROOT, NV20_SUBDEVICE_0, NV_ERR_NOT_SUPPORTED, NV_OK,
};

/// `NV_ERR_INSUFFICIENT_PERMISSIONS` (nvstatuscodes.h).
const NV_ERR_INSUFFICIENT_PERMISSIONS: u32 = 0x1B;

/// The internal client, device and subdevice handles the device hands the guest
/// kernel in the static info (device-owned, synthesized).
pub const INTERNAL_CLIENT: u32 = 0xCA7A_0001;
pub const INTERNAL_DEVICE: u32 = 0xCA7A_0002;
pub const INTERNAL_SUBDEVICE: u32 = 0xCA7A_0003;

/// Host RM access from the device process (decision D10): a control on the host
/// object that corresponds to the guest object's class.
pub trait HostRm: Send {
    /// Issue `cmd` on the host subdevice/device/client for `class`; `params` is
    /// updated in place. Returns the NV status.
    fn control(&mut self, class: u32, cmd: u32, params: &mut [u8]) -> u32;
}

/// The answer to one request.
pub struct Answer {
    pub status: u32,
    pub params: Vec<u8>,
    pub label: &'static str,
    pub note: String,
}

impl Answer {
    fn ok(params: Vec<u8>, label: &'static str, note: impl Into<String>) -> Self {
        Answer {
            status: NV_OK,
            params,
            label,
            note: note.into(),
        }
    }
}

fn put32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn put64(b: &mut [u8], off: usize, v: u64) {
    b[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

fn get32(b: &[u8], off: usize) -> u32 {
    if off + 4 > b.len() {
        return 0;
    }
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn get64(b: &[u8], off: usize) -> u64 {
    if off + 8 > b.len() {
        return 0;
    }
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}

/// What the device knows about the guest's objects and the addresses it was given.
pub struct Dispatcher {
    host: Option<Box<dyn HostRm>>,
    /// The chip's tables (engines, interrupts, memsys, ...).
    chip: &'static ChipData,
    /// (hClient, hObject) → class, for every alloc the device acknowledged. Handles are
    /// per client in RM, so the guest kernel's clients may reuse each other's numbers.
    objects: BTreeMap<(u32, u32), u32>,
    /// PCI ids of the emulated function (config space).
    pub pci_device_id: u32,
    pub pci_sub_device_id: u32,
    pub pci_revision: u32,
    /// BAR sizes (board data) and the addresses the guest reported (GspSystemInfo).
    pub bar_sizes: Vec<u64>,
    pub bar_offsets: [u64; 4],
    /// VBIOS revision and OEM revision (board data) and their provenance label.
    vbios: (u32, u32, &'static str),
    pub crash_buffer: Option<(u64, u32)>,
    pub rusd_pa: Option<u64>,
    pub bar2_pde: Option<u64>,
    pub bar1_pde: Option<u64>,
    /// The identity the nvidia-smi controls must not take from the host (nvml.rs).
    pub profile: crate::nvml::Profile,
    /// The guest's channels, TSGs and VA spaces for the channel emulation (E5a).
    pub channels: crate::channel::Registry,
    /// The fault and access-counter buffers nvidia-uvm registered (E5a).
    pub uvm: crate::uvm::UvmBuffers,
    /// The host mirrors of the guest's channels (E5b part 2, mirror.rs), when the device
    /// executes on the physical GPU.
    pub mirror: Option<crate::mirror::Mirror>,
    /// `CARAXES_MIRROR_DIAG=1`: run the per-doorbell push diagnostic (slow; investigation).
    mirror_diag: bool,
    /// When the first host doorbell write since the last `take` happened (the mirror's
    /// doorbell counter moved): the end of the ring-to-host-doorbell time the doorbell
    /// reactor measures (poll.rs, docs/caraxes/doorbell-poll.md).
    pub host_doorbell_at: Option<std::time::Instant>,
    /// The display engine in the GSP role (display/mod.rs, docs/caraxes/display-ab.md).
    pub display: crate::display::Display,
    /// The VM's VRAM reservation `[B, E)` (fb.rs `FbLayout`): the filtered forwards of
    /// public.rs pass on only host FB addresses inside it.
    pub reservation: Option<(u64, u64)>,
    /// What the guest set on its channels and TSGs (public.rs, gsp-plan/c-guest-state.md).
    pub guest_attrs: crate::public::GuestAttrs,
    /// The category D models of this VM (dmodel.rs, rc.rs, zbc.rs; gsp-plan/d-device-model.md).
    pub dstate: crate::dmodel::DState,
    /// The emulated PCIe Link Control register as the guest last left it (device.rs sets
    /// it after every config-space write): `NV208F BIF_INFO` answers from it (public.rs).
    pub link_control: u16,
}

/// Controls on a guest channel or TSG that a host mirror can take (mirror.rs
/// `forward_control`); the guest gets NV_OK either way, the host status is logged.
const MIRROR_FORWARDED: &[u32] = &[
    v::CMD_CHANNEL_STOP,
    v::CMD_TSG_PREEMPT,
    v::CMD_TSG_SET_TIMESLICE,
    v::CMD_CHANNEL_GPFIFO_SCHEDULE,
    v::CMD_TSG_GPFIFO_SCHEDULE,
];

impl Dispatcher {
    /// A dispatcher for `chip` presenting `id` (PCI ids, BAR sizes, VBIOS, GID, FB size).
    pub fn new(
        chip: &'static ChipData,
        id: &StaticIdentity,
        host: Option<Box<dyn HostRm>>,
    ) -> Self {
        let mut objects = BTreeMap::new();
        objects.insert((INTERNAL_CLIENT, INTERNAL_CLIENT), NV01_ROOT);
        objects.insert((INTERNAL_CLIENT, INTERNAL_DEVICE), NV01_DEVICE_0);
        objects.insert((INTERNAL_CLIENT, INTERNAL_SUBDEVICE), NV20_SUBDEVICE_0);
        Dispatcher {
            host,
            chip,
            objects,
            pci_device_id: id.pci.device_id(),
            pci_sub_device_id: id.pci.sub_device_id(),
            pci_revision: id.pci.revision as u32,
            bar_sizes: id.bar_sizes.clone(),
            bar_offsets: [0; 4],
            vbios: (id.vbios_revision, id.vbios_oem_revision, id.vbios_label),
            crash_buffer: None,
            rusd_pa: None,
            bar2_pde: None,
            bar1_pde: None,
            profile: crate::nvml::Profile {
                fb_bytes: id.fb_bytes,
                gid: id.gid,
                pci_device_id: id.pci.device,
            },
            channels: crate::channel::Registry::new(chip.channel),
            uvm: crate::uvm::UvmBuffers::default(),
            mirror: None,
            mirror_diag: std::env::var("CARAXES_MIRROR_DIAG").as_deref() == Ok("1"),
            host_doorbell_at: None,
            display: crate::display::Display::new(),
            reservation: None,
            guest_attrs: Default::default(),
            dstate: crate::dmodel::DState::new(chip),
            link_control: 0,
        }
    }

    /// The dispatcher of a device holding the VRAM reservation `r` (`FbLayout::reservation`).
    pub fn with_reservation(mut self, r: Option<(u64, u64)>) -> Self {
        self.reservation = r;
        self
    }

    pub fn take_host(&mut self) -> Option<Box<dyn HostRm>> {
        self.host.take()
    }

    /// A doorbell write: a channel the mirror takes runs on the physical GPU (created
    /// on first use), every other channel on the software executor.
    pub fn doorbell(&mut self, gm: &mut crate::channel::GpuMem, token: u32) -> Vec<String> {
        // A pool token (tokens.rs) that reached the device as a trapped write: its
        // ioeventfd was not registered. Same channel, same path as through the eventfd.
        let pool_token = |p: &crate::tokens::TokenPool| p.contains(token);
        if self.channels.tokens.as_ref().is_some_and(pool_token) {
            return self.doorbell_pool(gm, token, true);
        }
        // NV_CTRL_VF_DOORBELL_VECTOR 11:0 is the ChID (tu102/dev_ctrl.h:36-37) of a guest
        // token; a mirrored channel's guest reads the *host* token back from its notifier
        // page (`Mirror::chid_of_host_token`) and rings that from its second submission on.
        let (chid, via_host_token) = match self.mirror.as_ref().and_then(|m| m.chid_of_host_token(token)) {
            Some(c) => (c, true),
            None => (token & 0xFFF, false),
        };
        let mut notes = self.doorbell_chid(gm, token, chid, via_host_token);
        notes.extend(self.hand_out_token(gm, chid, token));
        notes
    }

    /// A ring of pool token `token` (docs/caraxes/doorbell-token-pool.md): through its
    /// eventfd (`trapped` false, device.rs `doorbell_tokens`) or as a trapped write. The
    /// pool names the channel; an unassigned token is stale and rings nothing (a doorbell
    /// only makes the engine look at GPPut, so nothing the guest relies on is lost).
    pub fn doorbell_pool(
        &mut self,
        gm: &mut crate::channel::GpuMem,
        token: u32,
        trapped: bool,
    ) -> Vec<String> {
        let Some(pool) = self.channels.tokens.as_mut() else {
            return vec![format!("doorbell token {token:#x}: no pool")];
        };
        if trapped {
            pool.stats.trapped_pool_rings += 1;
        }
        let Some(chid) = pool.chid(token) else {
            pool.note_stale(token, if trapped { "trapped" } else { "eventfd" });
            return vec![format!(
                "doorbell token {token:#x}: not assigned to any channel (stale), dropped"
            )];
        };
        let mut notes = vec![format!(
            "doorbell token {token:#x} ({}) -> chid {chid}",
            if trapped { "trapped" } else { "eventfd" }
        )];
        // The software executor decodes a token as ChID 11:0, runlist 22:16; ring the
        // channel with a guest token of it (runlist 0, as the token-less path does).
        notes.extend(self.doorbell_chid(gm, chid, chid, false));
        notes
    }

    /// After a trapped doorbell of `chid` with its own token `rung` (the guest's, or the
    /// host token a mirror's host RM wrote): give the channel a pool token and write it
    /// into the channel's work-submit-token notifier entry, where libcuda reads the token
    /// it rings (kernel_channel.c:3225-3286 `kchannelCtrlCmdGpfifoGetWorkSubmitToken`,
    /// :4015-4030 `kchannelNotifyWorkSubmitToken`; execution.md §6). Only a notifier entry
    /// that holds `rung` or the mirror's host token is overwritten; otherwise the channel
    /// keeps its own token and every ring of it traps, as without the pool. A channel
    /// that holds a pool token and rings its own token again gets the pool token written
    /// again if the entry was overwritten since (the host RM rewrites it when a mirror
    /// asks for the host token), and is only counted if the entry still holds the pool
    /// token (a ring that does not read the notifier: nvidia-uvm keeps the token of its
    /// channel allocation).
    fn hand_out_token(
        &mut self,
        gm: &mut crate::channel::GpuMem,
        chid: u32,
        rung: u32,
    ) -> Vec<String> {
        // The channel's error notifier: `None` for a channel the guest never announced.
        let notifier = self.channels.channel(chid).map(|c| c.error_notifier);
        let Some(pool) = self.channels.tokens.as_mut() else {
            return Vec::new();
        };
        let held = pool.token_of(chid);
        let Some(notifier) = notifier else {
            return Vec::new();
        };
        let Some(notifier) = notifier else {
            pool.stats.no_notifier += 1;
            return vec![format!(
                "doorbell tokens: chid {chid} has no error notifier; it keeps token {rung:#x} (trapped)"
            )];
        };
        use crate::fb::Target;
        use crate::tokens::NOTIFIER_TOKEN_OFFSET as AT;
        let at = match notifier {
            Target::Fb(pa) => Target::Fb(pa + AT),
            Target::Sys(pa) => Target::Sys(pa + AT),
            other => other,
        };
        let host = self
            .mirror
            .as_ref()
            .and_then(|m| m.channel(chid))
            .map(|h| h.token);
        let current = gm.read_u32(at);
        if held.is_some() && current == held {
            pool.stats.own_token_after_handout += 1;
            return Vec::new();
        }
        if current.is_none() || (current != Some(rung) && current != host) {
            pool.stats.mismatch += 1;
            let why = format!(
                "chid {chid}: work-submit-token entry at {at:?} holds {current:x?}, not the rung token {rung:#x}{}",
                host.map_or(String::new(), |h| format!(" or the host token {h:#x}"))
            );
            pool.note_refused(why.clone());
            return vec![format!(
                "doorbell tokens: {why}; it keeps its own token (trapped)"
            )];
        }
        let Some(token) = pool.assign(chid) else {
            pool.stats.exhausted += 1;
            return vec![format!(
                "doorbell tokens: pool of {} exhausted; chid {chid} keeps token {rung:#x} (trapped)",
                pool.len()
            )];
        };
        if !gm.write(at, &token.to_le_bytes()) {
            pool.release(chid);
            pool.stats.mismatch += 1;
            return vec![format!(
                "doorbell tokens: chid {chid}: write of {token:#x} to {at:?} failed; it keeps its own token (trapped)"
            )];
        }
        if matches!(at, crate::fb::Target::Fb(_)) {
            gm.fb.flush();
        }
        if held.is_some() {
            pool.stats.rewritten += 1;
        } else {
            pool.stats.handed_out += 1;
        }
        vec![format!(
            "doorbell tokens: chid {chid} {} pool token {token:#x} (was {rung:#x}{}), written to its work-submit-token notifier entry at {at:?} [synthesized]",
            if held.is_some() { "gets back its" } else { "gets" },
            match host {
                Some(h) if current == Some(h) => format!("; the entry held the host token {h:#x}"),
                _ => String::new(),
            }
        )]
    }

    /// The token-less doorbell (the ioeventfd path, device.rs `doorbell_ioeventfd`): KVM
    /// signals that the guest wrote the doorbell register but not what, so every channel
    /// with GPFIFO entries published since its last doorbell (`Registry::pending`, one
    /// USERD read each) is rung the way its own token would ring it. A channel the guest
    /// never rang is never pending, so no host mirror is created for it. The note reports
    /// the scan so the cost per wakeup is visible in the trace.
    pub fn doorbell_pending(&mut self, gm: &mut crate::channel::GpuMem) -> Vec<String> {
        let t0 = std::time::Instant::now();
        let chids: Vec<u32> = self.channels.channels().map(|c| c.chid).collect();
        let pending: Vec<u32> = chids
            .iter()
            .copied()
            .filter(|c| self.channels.pending(gm, *c))
            .collect();
        let mut notes = vec![format!(
            "doorbell (ioeventfd, no token): {} of {} recorded channel(s) pending {pending:?}, scan {} us",
            pending.len(),
            chids.len(),
            t0.elapsed().as_micros()
        )];
        for chid in pending {
            // A guest token of this channel: ChID in 11:0, runlist 0 (the runlist id only
            // reaches the notes).
            notes.extend(self.doorbell_chid(gm, chid, chid, false));
        }
        notes
    }

    /// The software channels the guest's scheduling switches hold (rc.rs `RcState`, the
    /// device's one set of flags) whose doorbells arrived while they were off, run once
    /// they are enabled again. Called on the RC event path (boot.rs), which has guest
    /// memory at hand.
    pub fn replay_held(&mut self, gm: &mut crate::channel::GpuMem) -> Vec<String> {
        if self.dstate.rc.held.is_empty() {
            return Vec::new();
        }
        let off = self.dstate.rc.disabled_chids(&self.channels);
        let held = std::mem::take(&mut self.dstate.rc.held);
        let (run, keep): (Vec<u32>, Vec<u32>) = held.into_iter().partition(|c| !off.contains(c));
        self.dstate.rc.held = keep.into_iter().collect();
        let mut notes = Vec::new();
        for chid in run {
            if self.channels.channel(chid).is_none() {
                continue;
            }
            notes.push(format!(
                "chid {chid}: enabled again, its held doorbell runs now [guest channel]"
            ));
            notes.extend(self.doorbell_chid(gm, chid, chid, false));
        }
        notes
    }

    /// After allocation `h_object` of `h_class` under channel `(h_client, h_parent)`: a
    /// video object the mirror deferred (mirror.rs `place_video_object`) is placed on the
    /// host now. Returns the notes and whether the guest's allocation is refused after all
    /// (fail closed); a refused one is forgotten again.
    pub fn place_video_object(
        &mut self,
        h_client: u32,
        h_parent: u32,
        h_object: u32,
        h_class: u32,
    ) -> (Vec<String>, bool) {
        let Some(chid) = self
            .channels
            .channels()
            .find(|c| c.h_client == h_client && c.h_object == h_parent)
            .map(|c| c.chid)
        else {
            return (Vec::new(), false);
        };
        let Some(m) = self.mirror.as_mut() else {
            return (Vec::new(), false);
        };
        let (notes, refused) = m.place_video_object(chid);
        if refused {
            self.channels.forget_engine_object(h_client, h_parent, h_class);
            self.objects.remove(&(h_client, h_object));
        }
        (notes, refused)
    }

    fn doorbell_chid(
        &mut self,
        gm: &mut crate::channel::GpuMem,
        token: u32,
        chid: u32,
        via_host_token: bool,
    ) -> Vec<String> {
        self.channels.note_rung(gm, chid);
        // The guest's scheduling switches (rc.rs, the one set of flags): a channel that is
        // off and not on a host mirror runs nothing; the doorbell is held until the
        // channel is enabled. A mirrored channel's host twin was switched off on the host
        // by the same control, so its doorbell goes on (the host does not schedule it).
        let off = self.dstate.rc.disabled_chids(&self.channels);
        if off.contains(&chid) && !self.channels.channel(chid).is_some_and(|c| c.mirrored) {
            self.dstate.rc.held.insert(chid);
            return vec![format!(
                "doorbell chid {chid}: scheduling disabled by the guest; held until enabled (rc.rs) [guest channel]"
            )];
        }
        let Some(m) = self.mirror.as_mut() else {
            return self.channels.doorbell_except(gm, token, &off);
        };
        // A GR channel under CARAXES_MIRROR_GR_SHADOW runs on the host-managed shadow VAS
        // (D17 route 4 stage 1, shadow.rs), built at its first doorbell.
        let host_doorbells = m.stats.doorbells;
        if m.is_shadow(&self.channels, chid) {
            let mut notes = m.gr_shadow_doorbell(&self.channels, gm, chid);
            // Shadow VA spaces handed to the teardown: their cached walks go too.
            for r in m.take_reaped_roots() {
                gm.fb.unwatch_tables(crate::fb::Target::Fb(r));
            }
            // A build the host refused for memory: RC the channel once (rc.rs).
            for c in m.take_exhausted() {
                let engine = self.channels.channel(c).map_or(0, |ch| ch.engine_type);
                self.dstate.rc.host_refused(c, engine);
                notes.push(format!(
                    "shadow: chid {c} refused by the host for memory: RC queued (PREEMPTIVE_REMOVAL) [host protection]"
                ));
            }
            // The host doorbell write is the last step of a built shadow channel's ring.
            if m.stats.doorbells != host_doorbells {
                self.host_doorbell_at.get_or_insert_with(std::time::Instant::now);
            }
            if let Some(c) = self.channels.channel_mut(chid) {
                c.mirrored = true;
            }
            // Forward the guest's own MEM_OP TLB invalidates too (the shadow VAS's PDB is
            // the host root; the invalidator handles the guest root for the CE channels).
            let inv = std::mem::take(&mut self.channels.tlb_invalidates);
            if !inv.is_empty() {
                let n: usize = inv.iter().map(|pdb| m.invalidate_tlb(gm.fb, *pdb)).sum();
                notes.push(format!(
                    "mirror: {} software TLB invalidate(s) -> {n} host invalidate(s) [host]",
                    inv.len()
                ));
            }
            // The push diagnostic for GR channels too (`CARAXES_MIRROR_DIAG=1`): the costume
            // ladder records the method stream libcuda submits for a presented compute class
            // (docs/caraxes/costume-ladder.md).
            if self.mirror_diag {
                notes.extend(self.mirror_push_diag(gm, chid));
            }
            return notes;
        }
        let eligible = m.is_mirrored(chid) || m.eligible(&self.channels, chid).is_ok();
        if !eligible {
            let mut notes = self.channels.doorbell_except(gm, token, &off);
            if let Err(why) = m.eligible(&self.channels, chid) {
                notes.push(format!("mirror: chid {chid} in software: {why}"));
            }
            // The software executor's TLB invalidates reach the physical GPU's TLBs
            // through the mirror VA spaces (the same page directories).
            let inv = std::mem::take(&mut self.channels.tlb_invalidates);
            if !inv.is_empty() {
                let n: usize = inv.iter().map(|pdb| m.invalidate_tlb(gm.fb, *pdb)).sum();
                notes.push(format!(
                    "mirror: {} software TLB invalidate(s) -> invalidated on {n} mirror VA space(s) [host]",
                    inv.len()
                ));
            }
            return notes;
        }
        self.channels.doorbells += 1;
        let mut notes = vec![format!(
            "doorbell {}: token {token:#x} chid {chid} runlist {} -> host mirror{}",
            self.channels.doorbells,
            (token >> 16) & 0x7F,
            if via_host_token {
                " (the host token, read back by the guest from its notifier page)"
            } else {
                ""
            }
        )];
        // Drain the BAR1 write-combining buffers before the host RM (and, for a GR
        // channel, the host GSP's context promotion) reads what the guest wrote to VRAM.
        gm.fb.flush();
        match m.ensure_channel(&self.channels, gm, chid) {
            Ok((_, n)) => {
                notes.extend(n);
                if let Some(c) = self.channels.channel_mut(chid) {
                    c.mirrored = true;
                }
                // Drain the BAR1 write-combining buffers so the GPFIFO entry, USERD and
                // page tables the guest wrote are in VRAM before the physical PBDMA
                // fetches them (execution.md: a stale fetch is a PBENTRY fault).
                gm.fb.flush();
                match m.doorbell(chid) {
                    Ok(n) => {
                        self.host_doorbell_at.get_or_insert_with(std::time::Instant::now);
                        notes.push(n)
                    }
                    Err(e) => notes.push(format!("mirror: doorbell chid {chid}: {e}")),
                }
                // The per-doorbell push diagnostic (GPGet poll, entry decode, semaphore
                // read-back) is an investigation aid that serializes and slows the data
                // path; it is off unless `CARAXES_MIRROR_DIAG=1`.
                if self.mirror_diag {
                    notes.extend(self.mirror_push_diag(gm, chid));
                }
            }
            Err(e) => {
                notes.push(format!(
                    "mirror: REFUSED chid {chid}: {e}; this doorbell runs on the software executor"
                ));
                notes.extend(self.channels.doorbell_except(gm, token, &off));
            }
        }
        notes
    }

    /// After the host doorbell of a mirrored channel (read-only, nothing is executed
    /// here): poll the channel's USERD GPGet through the FB store for up to 20 ms, then
    /// decode the GPFIFO entry the guest just rang and report every address the physical
    /// engine dereferences with its translation through the guest's own tables (the raw
    /// PTE as the GMMU sees it, exec-formats.md §2.2), and the bytes of a CE semaphore
    /// after the poll. GPGet unchanged means the PBDMA never fetched (scheduling or
    /// doorbell); GPGet == GPPut with the semaphore unwritten means the engine consumed
    /// the entry and the work itself did not land (execution.md §6, the CE decision).
    fn mirror_push_diag(&self, gm: &crate::channel::GpuMem, chid: u32) -> Vec<String> {
        use crate::fb::Target;
        const POLL: std::time::Duration = std::time::Duration::from_millis(20);
        let mut notes = Vec::new();
        let Some(ch) = self.channels.channel(chid) else {
            return notes;
        };
        let Some(userd) = ch.userd else {
            return notes;
        };
        let at = |t: Target, off: u64| match t {
            Target::Fb(pa) => Target::Fb(pa + off),
            Target::Sys(pa) => Target::Sys(pa + off),
            t => t,
        };
        let t0 = std::time::Instant::now();
        let (get, put) = loop {
            let get = gm.read_u32(at(userd, 0x88));
            let put = gm.read_u32(at(userd, 0x8C));
            if get.is_none() || get == put || t0.elapsed() > POLL {
                break (get, put);
            }
            std::hint::spin_loop();
        };
        notes.push(format!(
            "mirror: chid {chid} USERD GPGet {get:?} GPPut {put:?} after {} us (polled up to 20 ms) [host GPU]",
            t0.elapsed().as_micros()
        ));
        let (Some(put), Some(root)) = (
            put,
            self.channels
                .vas_key_of(ch)
                .and_then(|k| self.channels.vas_root(k)),
        ) else {
            return notes;
        };
        let pte_desc = |pte: u64| -> String {
            if pte == 0 {
                return "no entry".into();
            }
            let ap = (pte >> 1) & 3;
            format!(
                "pte {pte:#018x}: valid {} aperture {} ({}) vol {} priv {} ro {} kind {:#04x} comptag {:#x} addr {:#x}",
                pte & 1,
                ap,
                ["VIDEO", "PEER", "SYS_COH", "SYS_NONCOH"][ap as usize],
                (pte >> 3) & 1,
                (pte >> 5) & 1,
                (pte >> 6) & 1,
                pte >> 56,
                (pte >> 36) & 0xF_FFFF,
                ((pte >> 8) & 0x3FFF_FFFF_FFFF) << 12
            )
        };
        let walk = |va: u64| -> String {
            let (t, pte) = gm.fb.walk_in(Some(gm.sys), root, va);
            format!("va {va:#x} -> {t:?}; {}", pte_desc(pte))
        };
        let entries = ch.gpfifo_entries.max(1);
        let idx = (put + entries - 1) % entries;
        let entry_va = ch.gpfifo_va + 8 * idx as u64;
        let mut e = [0u8; 8];
        if let Err(err) = gm.read_va(root, entry_va, &mut e) {
            notes.push(format!(
                "mirror: chid {chid} gp {idx}: entry unreadable: {err}"
            ));
            return notes;
        }
        let (e0, e1) = (
            u32::from_le_bytes(e[0..4].try_into().unwrap()),
            u32::from_le_bytes(e[4..8].try_into().unwrap()),
        );
        let length = ((e1 >> 10) & 0x1F_FFFF) as usize;
        let pb_va = ((e1 as u64 & 0xFF) << 32) | (e0 as u64 & !3);
        notes.push(format!(
            "mirror: chid {chid} gp {idx} (entry {}): pushbuffer va {pb_va:#x} {length} dwords; gpfifo {}; pushbuffer {}",
            walk(entry_va).split(';').next().unwrap_or(""),
            walk(entry_va),
            walk(pb_va)
        ));
        if length == 0 || length > 4096 {
            return notes;
        }
        let mut raw = vec![0u8; length * 4];
        if let Err(err) = gm.read_va(root, pb_va, &mut raw) {
            notes.push(format!(
                "mirror: chid {chid} gp {idx}: pushbuffer unreadable: {err}"
            ));
            return notes;
        }
        let w: Vec<u32> = raw
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        notes.push(format!(
            "mirror: chid {chid} gp {idx} dwords: {}",
            w.iter()
                .take(48)
                .map(|d| format!("{d:08x}"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
        // NVC46F headers (clc46f.h:285-358), CE methods (clc5b5.h): SET_SEMAPHORE_A/B
        // 0x240/0x244, PAYLOAD 0x248, LAUNCH_DMA 0x300, OFFSET_IN/OUT 0x400-0x40c,
        // LINE_LENGTH_IN 0x418.
        let (mut sem, mut payload, mut src, mut dst, mut len) = (0u64, 0u32, 0u64, 0u64, 0u32);
        let mut objects = [0u32; 8];
        let mut i = 0usize;
        while i < w.len() {
            let hdr = w[i];
            i += 1;
            let (sec_op, count, subch, addr) = (
                hdr >> 29,
                ((hdr >> 16) & 0x1FFF) as usize,
                ((hdr >> 13) & 7) as usize,
                (hdr & 0xFFF) << 2,
            );
            let methods: Vec<(u32, u32)> = match sec_op {
                1 | 3 | 5 => {
                    let n = count.min(w.len() - i);
                    let m = (0..n)
                        .map(|k| {
                            let a = if sec_op == 3 || (sec_op == 5 && k > 0) {
                                addr
                            } else {
                                addr + 4 * k as u32
                            };
                            (a, w[i + k])
                        })
                        .collect();
                    i += n;
                    m
                }
                4 => vec![(addr, (hdr >> 16) & 0x1FFF)],
                _ => continue,
            };
            for (a, d) in methods {
                match a {
                    0x000 => objects[subch] = d & 0xFFFF,
                    0x240 => sem = (sem & 0xFFFF_FFFF) | ((d as u64 & 0x1_FFFF) << 32),
                    0x244 => sem = (sem & !0xFFFF_FFFF) | d as u64,
                    0x248 => payload = d,
                    0x400 => src = (src & 0xFFFF_FFFF) | ((d as u64 & 0x1_FFFF) << 32),
                    0x404 => src = (src & !0xFFFF_FFFF) | d as u64,
                    0x408 => dst = (dst & 0xFFFF_FFFF) | ((d as u64 & 0x1_FFFF) << 32),
                    0x40C => dst = (dst & !0xFFFF_FFFF) | d as u64,
                    0x418 => len = d,
                    0x300 => {
                        let mut parts = vec![format!(
                            "mirror: chid {chid} LAUNCH_DMA {d:#x} on subch {subch} (class {:#x}): transfer {} sem_type {} flush {} intr {} src_phys {} dst_phys {}",
                            objects[subch],
                            d & 3,
                            (d >> 3) & 3,
                            (d >> 2) & 1,
                            (d >> 5) & 3,
                            (d >> 12) & 1,
                            (d >> 13) & 1
                        )];
                        if d & 3 != 0 {
                            parts.push(format!(
                                "  src {} ; dst {} ; {len} bytes",
                                walk(src),
                                walk(dst)
                            ));
                        }
                        if (d >> 3) & 3 != 0 {
                            let mut b = [0u8; 16];
                            let bytes = match gm.read_va(root, sem, &mut b) {
                                Ok(()) => b
                                    .iter()
                                    .map(|x| format!("{x:02x}"))
                                    .collect::<Vec<_>>()
                                    .join(""),
                                Err(e) => format!("unreadable: {e}"),
                            };
                            parts.push(format!(
                                "  semaphore payload {payload:#x} at {} ; bytes there after the poll: {bytes}",
                                walk(sem)
                            ));
                        }
                        notes.extend(parts);
                    }
                    _ => {}
                }
            }
        }
        notes
    }

    /// Re-sync the GR shadow VAS from the guest root (called at each guest MMU_INVALIDATE):
    /// the guest maps set-A pages then invalidates before the GR push touches them. With
    /// `CARAXES_ALLOC_LAZY=1` the plan waits for the next shadow doorbell (lazy.rs).
    pub fn mirror_shadow_resync(
        &mut self,
        gm: &mut crate::channel::GpuMem,
        pdb: Option<u64>,
    ) -> Vec<String> {
        let reg = &self.channels;
        self.mirror
            .as_mut()
            .map(|m| m.shadow_invalidated(reg, gm, pdb))
            .unwrap_or_default()
    }

    /// The guest's L2 cache operation, honoured on the physical GPU with
    /// `NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE` (full cache, system-memory aperture, the
    /// operation's bit) on the device's own subdevice. The flag bits are the host
    /// driver's (`hostabi::HostLayouts::flush_gpu_cache`; 615 moved them). `None`
    /// without a host RM connection, otherwise the flags word sent and the host's NV
    /// status.
    pub fn host_flush_gpu_cache(&mut self, op: crate::hostabi::L2Flush) -> Option<(u32, u32)> {
        let host = self.host.as_mut()?;
        let l = crate::hostabi::current();
        let mut p = l.flush_gpu_cache(op);
        let st = host.control(NV20_SUBDEVICE_0, crate::hostabi::CMD_FB_FLUSH_GPU_CACHE, &mut p);
        Some((l.flush_gpu_cache_flags(op), st))
    }

    /// The guest kernel's TLB invalidate for `pdb`: forwarded to the mirror VA spaces.
    pub fn mirror_invalidate(&mut self, fb: &mut crate::fb::FakeFb, pdb: Option<u64>) -> usize {
        self.mirror
            .as_mut()
            .map_or(0, |m| m.invalidate_tlb(fb, pdb))
    }

    pub fn has_host(&self) -> bool {
        self.host.is_some()
    }

    /// `GSP_RM_ALLOC`: record the object; every class the init path allocates is
    /// acknowledged (rows 14-17, 26-28, 36-38 of the inventory).
    pub fn alloc(
        &mut self,
        h_client: u32,
        h_object: u32,
        h_parent: u32,
        h_class: u32,
        params: &[u8],
    ) -> Answer {
        // Profiler classes are refused (dmodel.rs, d-device-model.md 6.16); the D models note
        // the allocation (a reused handle, a channel's instance block).
        if let Some(a) = self.dstate.alloc(h_client, h_object, h_class, params) {
            return a;
        }
        self.objects.insert((h_client, h_object), h_class);
        // Hotplug events are kept for the display's POST_EVENT (display/mod.rs).
        self.display
            .record_event(h_client, h_object, h_class, params);
        // Display classes (display/mod.rs): recorded with their channel state.
        if let Some(a) = self.display.alloc(h_class, params) {
            return a;
        }
        // Channels, TSGs and VA spaces feed the channel emulation (channel.rs); the
        // reply is the echo either way (phase5-rpcs.md §1.6 rows 59 and 61: no reply
        // field is read).
        // A video object the host cannot place on a mirrored channel is refused before it
        // is recorded (mirror.rs `refuse_video_object`): the guest's encoder or decoder
        // open fails cleanly instead of pushing to a host channel without the object.
        if let Some(m) = self.mirror.as_ref() {
            let chid = self
                .channels
                .channels()
                .find(|c| c.h_client == h_client && c.h_object == h_parent)
                .map(|c| c.chid);
            if let Some(why) = chid.and_then(|c| m.refuse_video_object(c, h_class)) {
                self.objects.remove(&(h_client, h_object));
                return Answer {
                    status: NV_ERR_NOT_SUPPORTED,
                    params: params.to_vec(),
                    label: "refused (fail closed: no host placement for the video context)",
                    note: format!("alloc {h_class:#x} object {h_object:#x} under {h_parent:#x}: {why}"),
                };
            }
        }
        if let Some(mut note) = self
            .channels
            .record_alloc(h_client, h_parent, h_object, h_class, params)
        {
            // An engine object on a channel that already runs on the host: add it there.
            if let Some(m) = self.mirror.as_mut() {
                let chid = self
                    .channels
                    .channels()
                    .find(|c| c.h_client == h_client && c.h_object == h_parent)
                    .map(|c| c.chid);
                if let Some(n) = chid.and_then(|c| m.add_engine_object(c, h_class, params)) {
                    note.push_str("; ");
                    note.push_str(&n);
                }
            }
            return Answer::ok(
                params.to_vec(),
                "guest (recorded for the channel emulation; no host object)",
                format!("alloc {h_class:#x} object {h_object:#x} under {h_parent:#x}: {note}"),
            );
        }
        let name = match h_class {
            NV01_ROOT => "NV01_ROOT",
            NV01_DEVICE_0 => "NV01_DEVICE_0",
            NV20_SUBDEVICE_0 => "NV20_SUBDEVICE_0",
            NV01_EVENT_KERNEL_CALLBACK_EX => "NV01_EVENT_KERNEL_CALLBACK_EX",
            _ => "other",
        };
        Answer::ok(
            params.to_vec(),
            "synthesized (object recorded, no host object yet)",
            format!("alloc {name} {h_class:#x} object {h_object:#x} under {h_parent:#x}"),
        )
    }

    /// `DUP_OBJECT`: the duplicate has the source's class; a duplicated VA space is
    /// an alias of the source in the channel emulation (cuda-lookahead.md §5 item 1).
    pub fn dup(&mut self, dst: (u32, u32), src: (u32, u32)) -> String {
        let class = self.objects.get(&src).copied();
        if let Some(c) = class {
            self.objects.insert(dst, c);
        }
        let alias = self.channels.record_dup(dst, src);
        format!(
            "dup {:#x}/{:#x} <- {:#x}/{:#x} class {}{}",
            dst.0,
            dst.1,
            src.0,
            src.1,
            class.map_or("unknown".into(), |c| format!("{c:#x}")),
            alias.map_or(String::new(), |a| format!("; {a}"))
        )
    }

    /// `FREE`: forget the object; freeing a client forgets everything it owned.
    pub fn free(&mut self, h_client: u32, h_object: u32) -> Answer {
        let class = self.objects.remove(&(h_client, h_object));
        // The host mirrors go first (channels, then TSGs, then VA spaces), while the
        // registry still knows what the handle stood for.
        let mirror_notes = match self.mirror.as_mut() {
            Some(m) => m.free_object(&self.channels, h_client, h_object),
            None => Vec::new(),
        };
        self.channels.record_free(h_client, h_object);
        self.guest_attrs.free(h_client, h_object);
        self.dstate.rc.forget(h_client, h_object);
        if h_object == h_client {
            self.objects.retain(|(c, _), _| *c != h_client);
        }
        Answer::ok(
            Vec::new(),
            "synthesized",
            format!(
                "free {h_object:#x} class {class:?}{}",
                if mirror_notes.is_empty() {
                    String::new()
                } else {
                    format!("; mirror: {}", mirror_notes.join("; "))
                }
            ),
        )
    }

    /// `GSP_RM_CONTROL`. An answer the device has no model for (`unanswered`, or a `no
    /// host` fallback in pure emulation) is filled from the card's recorded answers or chip
    /// facts where they fit (recorded.rs); what stays NOT_SUPPORTED carries the first
    /// request bytes in its note, so a trace shows what the guest asked.
    pub fn control(&mut self, h_client: u32, h_object: u32, cmd: u32, params: &[u8]) -> Answer {
        let facts = crate::recorded::Facts {
            fb_bytes: self.profile.fb_bytes,
            gpcs: self.gr_info(0x14).unwrap_or(1),
            tpcs_per_gpc: self.gr_info(0x17).unwrap_or(1),
        };
        if crate::recorded::overrides(cmd)
            && !(self.host.is_some() && crate::recorded::PREFER_RECORDED.contains(&cmd))
        {
            if let Some(r) = crate::recorded::answer(cmd, params, &facts) {
                return r;
            }
        }
        let a = self.control_modelled(h_client, h_object, cmd, params);
        if a.label.starts_with("host") {
            crate::recorded::record_host(cmd, a.status, params, &a.params);
        }
        // Only for an object the guest allocated or the internal client's: a control on a
        // freed or unknown handle stays refused.
        let known = h_client == INTERNAL_CLIENT || self.objects.contains_key(&(h_client, h_object));
        // With a host RM: what it refuses to the device's unprivileged client (0x1b) is not
        // what the guest kernel would get from GSP-RM; the recorded answer stands in.
        let host_refused = a.status == NV_ERR_INSUFFICIENT_PERMISSIONS && a.label.starts_with("host");
        let unmodelled = a.status == NV_ERR_NOT_SUPPORTED
            && (a.label == "unanswered" || a.label.contains("no host"));
        if !(unmodelled || host_refused) || !known {
            return a;
        }
        match crate::recorded::answer(cmd, params, &facts) {
            Some(mut r) => {
                r.note = format!("{} (was: {} [{}])", r.note, a.note, a.label);
                r
            }
            // Diagnostic only (`CARAXES_ACK_UNANSWERED=1`): every control still without an
            // answer gets NV_OK and its request back, to see whether any refusal gates.
            None if std::env::var("CARAXES_ACK_UNANSWERED").is_ok_and(|v| v == "1")
                && !crate::recorded::excepted(cmd) =>
            {
                Answer::ok(
                params.to_vec(),
                "diagnostic (CARAXES_ACK_UNANSWERED: NV_OK echo)",
                format!("{} -> acknowledged (was status {:#x} [{}])", a.note, a.status, a.label),
                )
            }
            None => {
                let mut a = a;
                let head: String = params.iter().take(48).map(|b| format!("{b:02x}")).collect();
                a.note.push_str(&format!("; request {head}"));
                a
            }
        }
    }

    /// A `[gr.emulated_info]` entry of the chip (GPC count 0x14, TPCs per GPC 0x17).
    fn gr_info(&self, index: u32) -> Option<u32> {
        self.chip
            .gr
            .emulated_info
            .as_ref()?
            .entries
            .iter()
            .find(|e| e[0] == index)
            .map(|e| e[1])
            .filter(|&n| (1..32).contains(&n))
    }

    fn control_modelled(&mut self, h_client: u32, h_object: u32, cmd: u32, params: &[u8]) -> Answer {
        // A control on a channel or TSG that runs on the host: the host mirror takes it.
        if MIRROR_FORWARDED.contains(&cmd) {
            if let Some(m) = self.mirror.as_mut() {
                if let Some((st, note)) =
                    m.forward_control(&self.channels, h_client, h_object, cmd, params)
                {
                    return Answer::ok(
                        params.to_vec(),
                        "host (forwarded to the mirror object; the guest sees NV_OK)",
                        format!("{note}; host status {st:#x}"),
                    );
                }
            }
        }
        // The guest's server-reserved PDEs build the mirror VA space right away (mirror.rs
        // `on_server_reserved_pdes`); its page-directory controls move or drop it.
        if cmd == v::CMD_VASPACE_COPY_SERVER_RESERVED_PDES {
            let a = crate::uvm::answer(
                self.chip,
                cmd,
                h_client,
                h_object,
                params,
                &mut self.channels,
                &mut self.uvm,
                self.host.as_mut(),
            );
            if let Some(mut a) = a {
                if let Some(m) = self.mirror.as_mut() {
                    let notes = m.on_server_reserved_pdes(&self.channels, (h_client, h_object));
                    if !notes.is_empty() {
                        a.note.push_str("; mirror: ");
                        a.note.push_str(&notes.join("; "));
                    }
                }
                return a;
            }
        }
        if cmd == v::CMD_DMA_SET_PAGE_DIRECTORY || cmd == v::CMD_DMA_UNSET_PAGE_DIRECTORY {
            let a = crate::uvm::answer(
                self.chip,
                cmd,
                h_client,
                h_object,
                params,
                &mut self.channels,
                &mut self.uvm,
                self.host.as_mut(),
            );
            if let Some(mut a) = a {
                if let Some(m) = self.mirror.as_mut() {
                    let h_vas = get32(
                        params,
                        if cmd == v::CMD_DMA_SET_PAGE_DIRECTORY {
                            v::SET_PD_H_VASPACE
                        } else {
                            v::UNSET_PD_H_VASPACE
                        },
                    );
                    let key = self.channels.resolve((h_client, h_vas));
                    let root = match self.channels.vas_root(key) {
                        Some(crate::fb::Target::Fb(pa)) => Some(pa),
                        _ => None,
                    };
                    let notes = m.on_page_directory(
                        key,
                        root,
                        (cmd == v::CMD_DMA_SET_PAGE_DIRECTORY).then_some(params),
                    );
                    if !notes.is_empty() {
                        a.note.push_str("; mirror: ");
                        a.note.push_str(&notes.join("; "));
                    }
                }
                return a;
            }
        }
        // The display controls (docs/caraxes/display-ab.md) live in display/mod.rs.
        if let Some(a) = self.display.control(cmd, params) {
            return a;
        }
        // Graphics static-info rows (docs/caraxes/phase5-rpcs.md rows 35, 39-52) live in kgr.rs.
        // Pure emulation with `CARAXES_EMU_GR=1`: a whole TU106 GR from the recordings.
        let kgr = if self.host.is_none() && crate::recorded::emu_gr() {
            let facts = crate::recorded::Facts {
                fb_bytes: self.profile.fb_bytes,
                gpcs: self.gr_info(0x14).unwrap_or(1),
                tpcs_per_gpc: self.gr_info(0x17).unwrap_or(1),
            };
            let mut h: Box<dyn HostRm> = Box::new(crate::recorded::EmuGrHost { facts });
            crate::kgr::answer(self.chip, cmd, params, Some(&mut h)).map(|mut a| {
                a.note = format!("{} [{}]", a.note, a.label);
                a.label = crate::recorded::LABEL_EMU_GR;
                a
            })
        } else {
            crate::kgr::answer(self.chip, cmd, params, self.host.as_mut())
        };
        if let Some(a) = kgr {
            return a;
        }
        // The CUDA path's synthesized and derived rows (E5c) live in cuda.rs.
        if let Some(a) = crate::cuda::answer_control(
            self.chip,
            cmd,
            params,
            &mut self.channels,
            self.host.as_mut(),
        ) {
            return a;
        }
        // The controls nvidia-uvm causes (docs/caraxes/uvm.md) live in uvm.rs.
        if let Some(a) = crate::uvm::answer(
            self.chip,
            cmd,
            h_client,
            h_object,
            params,
            &mut self.channels,
            &mut self.uvm,
            self.host.as_mut(),
        ) {
            return a;
        }
        let n = params.len();
        // The kernel's internal client is known under the handles the device chose; a
        // handle the device never saw allocated is taken to be a subdevice.
        let class = self
            .objects
            .get(&(h_client, h_object))
            .or_else(|| self.objects.get(&(INTERNAL_CLIENT, h_object)))
            .copied()
            .unwrap_or(NV20_SUBDEVICE_0);
        // The GSP plan's table-driven public controls (public.rs, docs/caraxes/gsp-plan/).
        let key = self.channels.resolve((h_client, h_object));
        let mut fwd = self.mirror.as_mut().map(|mirror| crate::public::MirrorFwd {
            mirror,
            reg: &self.channels,
        });
        let mut pctx = crate::public::Ctx {
            host: self.host.as_deref_mut(),
            reservation: self.reservation,
            chip: self.chip,
            now_ns: self.channels.now_ns(),
            object: (h_client, h_object),
            key,
            mirror: fwd
                .as_mut()
                .map(|f| f as &mut dyn crate::public::MirrorObjects),
            attrs: &mut self.guest_attrs,
            rc: &mut self.dstate.rc,
            reg: Some(&self.channels),
            link_control: self.link_control,
        };
        if let Some(a) = crate::public::answer(&mut pctx, cmd, params) {
            return a;
        }
        // nvidia-smi telemetry and identity (docs/caraxes/nvml.md) live in nvml.rs.
        if let Some(a) = crate::nvml::answer(
            self.chip,
            cmd,
            class,
            params,
            self.host.as_mut(),
            &self.profile,
        ) {
            return a;
        }
        // Category E refusals per feature family and the category A chip-data table
        // (chipctl.rs, docs/caraxes/gsp-surface.md §3).
        if let Some(a) = crate::chipctl::answer(self.chip, cmd, params) {
            return a;
        }
        // Category D: the models (dmodel.rs, rc.rs, zbc.rs; R2 and the open MODEL rows), then
        // the rule table (R0), docs/caraxes/gsp-plan/d-device-model.md.
        let mut dmirror = self.mirror.as_mut().map(|mirror| crate::rc::MirrorRef {
            mirror,
            reg: &self.channels,
        });
        let mut dcx = crate::dmodel::DCtx {
            reg: &self.channels,
            mirror: dmirror
                .as_mut()
                .map(|m| m as &mut dyn crate::rc::MirrorOps),
            host: self.host.as_deref_mut(),
            bar0: (self.bar_offsets[0], self.bar_sizes.first().copied().unwrap_or(0)),
            object: (h_client, h_object),
        };
        if let Some(a) = self
            .dstate
            .control(&mut dcx, cmd, params)
            .or_else(|| crate::dmodel::answer(cmd, params))
        {
            return a;
        }
        match cmd {
            // 0x20800A36: chip info for a 610 guest, the fabric-probe sync for a 615
            // guest (the id was reused; `ver::Id0A36`).
            CMD_INTERNAL_GPU_GET_CHIP_INFO => self.id_0a36(v::CTRL_0A36, params),
            CMD_INTERNAL_GPU_GET_USER_REGISTER_ACCESS_MAP => {
                // ctrl2080internal.h:775-788: the map size with compressedSize 0 = allow all.
                let m = &self.chip.user_register_access_map;
                let mut b = vec![0u8; n];
                if n >= v::USER_REG_MAP_SIZE + 4 {
                    put32(&mut b, v::USER_REG_MAP_SIZE, m.value);
                }
                Answer::ok(b, &m.label, "user register access map")
            }
            CMD_GPU_GET_CONSTRUCTED_FALCON_INFO => self.forward_or(class, cmd, params, |n| {
                // No host: no constructed falcons (SEC2 is skipped by the kernel anyway).
                Answer::ok(vec![0u8; n], "synthesized fallback (no falcons)", "constructed falcon info: empty")
            }),
            CMD_INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO => Answer {
                status: NV_ERR_NOT_SUPPORTED,
                params: params.to_vec(),
                label: "synthesized (no NVLink; removes KernelNvlink, row 6)",
                note: "nvlink device info: not supported".into(),
            },
            CMD_FIFO_GET_DEVICE_INFO_TABLE => {
                // ctrl2080fifo.h:535-576: baseIndex, numEntries, bMore, then entries of
                // engineData[16] (engine_info.h indices), pbdmaIds[2], pbdmaFaultIds[2],
                // numPbdmas, engineName[16]. The table is the chip's
                // (chips/<chip>/device-info.toml, docs/caraxes/fifo-device-info.md).
                let table = &self.chip.device_info;
                let mut b = vec![0u8; n.max(v::DEVINFO_PARAMS_SIZE)];
                put32(&mut b, v::DEVINFO_NUM_ENTRIES, table.engines.len() as u32);
                b[v::DEVINFO_B_MORE] = 0;
                for (i, e) in table.engines.iter().enumerate() {
                    let o = v::DEVINFO_ENTRIES + v::DEVINFO_ENTRY_SIZE * i;
                    let data = |idx: usize| o + v::DEVINFO_ENGINE_DATA + 4 * idx;
                    put32(&mut b, data(v::ENGINE_INFO_ENG_DESC), e.eng_desc);
                    put32(&mut b, data(v::ENGINE_INFO_RM_ENGINE_TYPE), e.rm_engine_type);
                    put32(&mut b, data(v::ENGINE_INFO_RUNLIST), e.runlist);
                    put32(&mut b, data(v::ENGINE_INFO_MMU_FAULT_ID), e.mmu_fault_id);
                    put32(&mut b, data(v::ENGINE_INFO_MC), e.mc_idx);
                    put32(&mut b, data(v::ENGINE_INFO_DEV_TYPE_ENUM), e.dev_type);
                    put32(&mut b, data(v::ENGINE_INFO_INSTANCE_ID), e.instance);
                    put32(&mut b, data(v::ENGINE_INFO_IS_HOST_DRIVEN_ENGINE), e.host_driven);
                    for (k, [id, fault]) in e.pbdma.iter().enumerate() {
                        put32(&mut b, o + v::DEVINFO_PBDMA_IDS + 4 * k, *id);
                        put32(&mut b, o + v::DEVINFO_PBDMA_FAULT_IDS + 4 * k, *fault);
                    }
                    put32(&mut b, o + v::DEVINFO_NUM_PBDMAS, e.pbdma.len() as u32);
                    let name = o + v::DEVINFO_ENGINE_NAME;
                    b[name..name + e.name.len()].copy_from_slice(e.name.as_bytes());
                }
                b.truncate(n);
                Answer::ok(b, &table.label, format!("fifo device info table: {} entries", table.engines.len()))
            }
            CMD_INTERNAL_INTR_GET_KERNEL_TABLE => {
                // ctrl2080internal.h:1463-1480: tableLen, 128 entries (engineIdx,
                // pmcIntrMask, vectorStall, vectorNonStall), subtreeMap[7]. The table
                // is the chip's (chips/<chip>/interrupts.toml).
                let t = &self.chip.interrupts;
                let mut b = vec![0u8; n.max(v::INTR_TABLE_SIZE)];
                put32(&mut b, v::INTR_TABLE_LEN, t.entries.len() as u32);
                for (i, e) in t.entries.iter().enumerate() {
                    let o = v::INTR_TABLE + v::INTR_ENTRY_SIZE * i;
                    let idx = o + v::INTR_ENTRY_ENGINE_IDX;
                    b[idx..idx + 2].copy_from_slice(&e.mc_idx.to_le_bytes());
                    put32(&mut b, o + v::INTR_ENTRY_VECTOR_STALL, e.vector_stall);
                    put32(&mut b, o + v::INTR_ENTRY_VECTOR_NONSTALL, e.vector_nonstall);
                }
                for &(cat, mask) in &t.subtrees {
                    put64(&mut b, v::INTR_SUBTREE_MAP + v::INTR_SUBTREE_MAP_ENTRY * cat, mask);
                }
                b.truncate(n);
                Answer::ok(b, &t.label, format!("interrupt kernel table: {} entries", t.entries.len()))
            }
            CMD_INTERNAL_MEMSYS_GET_STATIC_CONFIG => {
                // ctrl2080internal.h:94-135; the values are the chip's.
                let m = &self.chip.memsys;
                let mut b = vec![0u8; n.max(v::MEMSYS_SIZE)];
                b[v::MEMSYS_DISABLE_COMPBIT_BACKING] = m.disable_compbit_backing as u8;
                put64(&mut b, v::MEMSYS_L2_CACHE_SIZE, m.l2_cache_size);
                b[v::MEMSYS_FBPA_PRESENT] = m.fbpa_present as u8;
                put32(&mut b, v::MEMSYS_COMPR_PAGE_SIZE, m.compr_page_size);
                put32(&mut b, v::MEMSYS_COMPR_PAGE_SHIFT, m.compr_page_shift);
                put32(&mut b, v::MEMSYS_RAM_TYPE, m.ram_type);
                put32(&mut b, v::MEMSYS_LTC_COUNT, m.ltc_count);
                put32(&mut b, v::MEMSYS_LTS_PER_LTC_COUNT, m.lts_per_ltc_count);
                b.truncate(n);
                Answer::ok(b, &m.label, "memsys static config")
            }
            CMD_BUS_GET_PCI_BAR_INFO => {
                // ctrl2080bus.h:612-646: pciBarCount, then {flags, barSize (MiB),
                // barSizeBytes, barOffset} per BAR.
                let count = self.bar_sizes.len().min(self.bar_offsets.len());
                let mut b = vec![0u8; n.max(v::BAR_INFO_SIZE)];
                put32(&mut b, v::BAR_INFO_COUNT, count as u32);
                for i in 0..count {
                    let o = v::BAR_INFO_ENTRIES + v::BAR_INFO_ENTRY_SIZE * i;
                    put32(&mut b, o + v::BAR_INFO_BAR_SIZE_MIB, (self.bar_sizes[i] >> 20) as u32);
                    put64(&mut b, o + v::BAR_INFO_BAR_SIZE_BYTES, self.bar_sizes[i]);
                    put64(&mut b, o + v::BAR_INFO_BAR_OFFSET, self.bar_offsets[i]);
                }
                b.truncate(n);
                Answer::ok(b, "guest (BAR addresses from GspSystemInfo) + card (emulated BAR sizes)", format!("pci bar info: {count} bars"))
            }
            CMD_INTERNAL_BIF_GET_STATIC_INFO => Answer::ok(vec![0u8; n], "synthesized (single function, no gen4, no C2C)", "bif static info: all false"),
            CMD_INTERNAL_INIT_USER_SHARED_DATA => {
                let pa = get64(params, v::RUSD_PHYS_ADDR);
                self.rusd_pa = Some(pa);
                Answer::ok(params.to_vec(), "guest (RUSD page recorded)", format!("init user shared data: pa {pa:#x}"))
            }
            CMD_INTERNAL_USER_SHARED_DATA_SET_DATA_POLL => Answer::ok(params.to_vec(), "synthesized", "user shared data poll: ok"),
            CMD_EVENT_SET_NOTIFICATION => Answer::ok(params.to_vec(), "synthesized (event registered, never raised yet)", "event set notification: ok"),
            CMD_INTERNAL_GMMU_GET_STATIC_INFO => {
                // ctrl2080internal.h:1417-1426: four u32 sizes; N entries of the chip's
                // fault packet size, non-zero.
                let g = &self.chip.gmmu;
                let size = g.fault_buffer_entries * g.fault_entry_size;
                let mut b = vec![0u8; n.max(v::GMMU_STATIC_SIZE)];
                put32(&mut b, v::GMMU_REPLAYABLE_FAULT_BUFFER_SIZE, size);
                put32(&mut b, v::GMMU_NON_REPLAYABLE_FAULT_BUFFER_SIZE, size);
                b.truncate(n);
                Answer::ok(b, &g.label, "gmmu static info")
            }
            CMD_INTERNAL_MEMSYS_L2_INVALIDATE_EVICT => Answer::ok(params.to_vec(), "synthesized (no cache to evict)", "l2 invalidate/evict: ok"),
            CMD_INTERNAL_BUS_FLUSH_WITH_SYSMEMBAR => Answer::ok(params.to_vec(), "synthesized (nothing to flush)", "bus flush with sysmembar: ok"),
            // Row 30 (ctrl2080ce.h:469-478): lceType/metadata are inputs; the masks stay 0
            // on a chip without a decompression LCE (Turing).
            CMD_INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_TYPE if !self.chip.ce.decomp_lce => {
                let mut b = params.to_vec();
                for o in v::PCE_CONFIG_OUTPUTS {
                    if b.len() >= o + 4 {
                        put32(&mut b, o, 0);
                    }
                }
                Answer::ok(b, &self.chip.ce.decomp_label, "ce pce config for lce type: zeros")
            }
            // Rows 31/33 (ctrl2080ce.h:271-273): stubbedCeMask 0 keeps every advertised CE.
            CMD_CE_UPDATE_CLASS_DB => Answer::ok(vec![0u8; n], "synthesized (stubbedCeMask 0)", "ce update class db: no CE stubbed"),
            // Row 32: real GSP reprograms PCE2LCE; the device only records the request.
            CMD_CE_UPDATE_PCE_LCE_MAPPINGS_V2 => Answer::ok(params.to_vec(), "synthesized (mapping recorded, nothing programmed)", "ce update pce/lce mappings: ok"),
            // Row 79: forwardable as-is; zeros when there is no host.
            CMD_GPU_GET_OEM_BOARD_INFO => self.forward_or(class, cmd, params, |n| Answer::ok(vec![0u8; n], "synthesized fallback (no OEM board info)", "oem board info: zeros")),
            // Row 96 (ctrl2080gsp.h:64-75): gspFeatures @0, bValid @4, bDefaultGspRmGpu @5,
            // firmwareVersion[64] @6. The version string is what nvidia-smi shows.
            CMD_GSP_GET_FEATURES => {
                let mut b = vec![0u8; n];
                // CARAXES_GSP_FW_VERSION: the firmware version string instead of the emulated
                // driver's (a 617.14 Windows guest against the 615-based model).
                let env_ver = std::env::var("CARAXES_GSP_FW_VERSION").ok().filter(|s| s.len() < 64);
                let ver = env_ver.as_deref().unwrap_or(v::DRIVER_VERSION).as_bytes();
                if n >= v::GSP_FEATURES_SIZE {
                    // gspFeatures 1, as the card answers (kmd-controls-2026-09-26).
                    put32(&mut b, 0, 1);
                    b[v::GSP_FEATURES_VALID] = 1;
                    b[v::GSP_FEATURES_DEFAULT_GSP_RM_GPU] = 1;
                    let at = v::GSP_FEATURES_FIRMWARE_VERSION;
                    b[at..at + ver.len()].copy_from_slice(ver);
                }
                Answer::ok(b, "synthesized (driver version string; the emulated GSP is that version)", format!("gsp features: 1, valid, {}", String::from_utf8_lossy(ver)))
            }
            // Row 97 (ctrl2080bios.h:44-45, 97-104): biosInfoListSize @0, entries {index, data} @4.
            CMD_BIOS_GET_INFO_V2 => {
                let (revision, oem, label) = self.vbios;
                let mut b = params.to_vec();
                let count = get32(params, v::BIOS_INFO_LIST_SIZE) as usize;
                for i in 0..count {
                    let o = v::BIOS_INFO_LIST + v::BIOS_INFO_ENTRY_SIZE * i;
                    if o + v::BIOS_INFO_ENTRY_SIZE > b.len() {
                        break;
                    }
                    let data = match get32(params, o) {
                        v::BIOS_INFO_INDEX_REVISION => revision,
                        v::BIOS_INFO_INDEX_OEM_REVISION => oem,
                        _ => 0,
                    };
                    put32(&mut b, o + 4, data);
                }
                Answer::ok(b, label, format!("bios info v2: {count} entries"))
            }
            // Row 98 (ctrl402c.h:110-118): one byte per port, 0 = not present.
            CMD_I2C_GET_PORT_INFO => Answer::ok(vec![0u8; n], "synthesized (no I2C ports)", "i2c port info: none"),
            // Row 80 (ctrl2080internal.h:2576-2580): both statuses NOT_SUPPORTED (0x05).
            CMD_INTERNAL_GET_PCIE_P2P_CAPS => {
                let mut b = vec![0u8; n];
                if n > v::P2P_WRITE_CAPS_STATUS {
                    b[v::P2P_READ_CAPS_STATUS] = v::P2P_CAPS_STATUS_NOT_SUPPORTED;
                    b[v::P2P_WRITE_CAPS_STATUS] = v::P2P_CAPS_STATUS_NOT_SUPPORTED;
                }
                Answer::ok(b, "synthesized (no PCIe P2P)", "pcie p2p caps: not supported")
            }
            CMD_CE_GET_FAULT_METHOD_BUFFER_SIZE => {
                // ctrl2080ce.h:295: {size}; the chip's (docs/caraxes/fifo-device-info.md §4a).
                let f = &self.chip.fifo.fault_method_buffer_size;
                let mut b = vec![0u8; n];
                if n >= v::CE_FAULT_METHOD_BUFFER_SIZE + 4 {
                    put32(&mut b, v::CE_FAULT_METHOD_BUFFER_SIZE, f.value);
                }
                Answer::ok(b, &f.label, "ce fault method buffer size")
            }
            CMD_INTERNAL_PERF_GPU_BOOST_SYNC_GET_INFO => Answer::ok(vec![0u8; n], "synthesized (no boost sync)", "perf boost sync info: zeros"),
            CMD_INTERNAL_FIFO_GET_NUM_CHANNELS => {
                // ctrl2080internal.h:1579-1586: runlistId in, numChannels out.
                let c = &self.chip.fifo.num_channels;
                let mut b = params.to_vec();
                if n >= v::NUM_CHANNELS_SIZE {
                    put32(&mut b, v::NUM_CHANNELS, c.value);
                }
                Answer::ok(b, &c.label, format!("fifo num channels: {}", c.value))
            }
            CMD_GPU_GET_VMMU_SEGMENT_SIZE => self.forward_or(class, cmd, params, |n| Answer {
                status: NV_ERR_NOT_SUPPORTED,
                params: vec![0u8; n],
                label: "synthesized fallback (no host: no VMMU)",
                note: "vmmu segment size: not supported".into(),
            }),
            // The L2 state of the card: forwarded (the host answers for the same silicon),
            // else the Turing default (powered, write-back, no bypass, full RCM).
            CMD_FB_GET_GPU_CACHE_INFO => self.forward_or(class, cmd, params, |n| {
                let mut b = vec![0u8; n];
                if n >= 8 {
                    put32(&mut b, 4, v::GPU_CACHE_INFO_WRITE_MODE_WRITEBACK);
                }
                Answer::ok(b, "synthesized fallback (L2 enabled, write-back)", "gpu cache info")
            }),
            // Context-buffer sizes are the silicon's: forwarded to the host's device.
            CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES => self.forward_or(class, cmd, params, |n| Answer {
                status: NV_ERR_NOT_SUPPORTED,
                params: vec![0u8; n],
                label: "synthesized fallback (no host: sizes unknown)",
                note: "engine context properties: not supported".into(),
            }),
            // The zcull save buffer of a guest 3D channel: recorded. The host context of a
            // mirrored channel keeps its default zcull mode (GLOBAL), which only matters when
            // the channel is preempted mid-frame (display-e.md, open items).
            CMD_GR_CTXSW_ZCULL_BIND => {
                let g32 = |o: usize| params.get(o..o + 4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()));
                let g64 = |o: usize| params.get(o..o + 8).map_or(0, |b| u64::from_le_bytes(b.try_into().unwrap()));
                Answer::ok(
                    params.to_vec(),
                    "guest (recorded; not applied to the host context)",
                    format!(
                        "gr ctxsw zcull bind: client {:#x} channel {:#x} va {:#x} mode {}",
                        g32(0),
                        g32(4),
                        g64(8),
                        g32(16)
                    ),
                )
            }
            _ => Answer {
                status: NV_ERR_NOT_SUPPORTED,
                params: params.to_vec(),
                label: "unanswered",
                note: format!("control {cmd:#x} on {h_object:#x} (class {class:#x}, {n} bytes): not supported"),
            },
        }
    }

    /// 0x20800A36 as the guest's driver version means it (`ver::Id0A36`, chosen by the
    /// guest ABI through `v::CTRL_0A36`).
    fn id_0a36(&self, id: crate::ver::Id0A36, params: &[u8]) -> Answer {
        let n = params.len();
        match id {
            crate::ver::Id0A36::GetChipInfo => {
                // 610 ctrl2080internal.h:574-592: the PCI ids of the function and the
                // chip's register bases (TIMER and USERMODE; all others unsupported).
                let c = &self.chip.chip_info;
                let mut b = vec![0u8; n.max(v::CHIP_INFO_SIZE)];
                put32(&mut b, v::CHIP_INFO_PCI_DEVICE_ID, self.pci_device_id);
                put32(&mut b, v::CHIP_INFO_PCI_SUB_DEVICE_ID, self.pci_sub_device_id);
                put32(&mut b, v::CHIP_INFO_PCI_REVISION_ID, self.pci_revision);
                let base = |i: usize| v::CHIP_INFO_REG_BASES + 4 * i;
                for i in 0..v::CHIP_INFO_REG_BASE_COUNT {
                    put32(&mut b, base(i), v::REG_BASE_NONE);
                }
                put32(&mut b, base(v::REG_BASE_TIMER), c.timer_base);
                put32(&mut b, base(v::REG_BASE_USERMODE), c.usermode_base);
                b.truncate(n);
                Answer::ok(b, &c.label, "chip info")
            }
            // 615 ctrl2080internal.h:572-585: the kernel pushes the NVLink mask a fabric
            // manager reduced (gpu_fabric_probe.c:1912-1943, only on NVLink-fabric
            // systems; the kernel only logs a failure). No fabric on TU106: nothing to
            // apply, and nothing goes to the host.
            crate::ver::Id0A36::SyncFabricProbeInfo => Answer::ok(
                params.to_vec(),
                "synthesized (NV_OK, echo: no NVLink fabric; 615 INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO)",
                format!("sync fabric probe info ({n} bytes): nothing to apply, no NVLink fabric"),
            ),
        }
    }

    /// Forward `cmd` to the host object its id addresses (hostrm.rs `class_of_control`),
    /// not to the guest object's class: a control on any other object is never sent.
    fn forward_or(
        &mut self,
        _class: u32,
        cmd: u32,
        params: &[u8],
        fallback: impl FnOnce(usize) -> Answer,
    ) -> Answer {
        let Some(class) = crate::hostrm::class_of_control(cmd) else {
            return Answer {
                status: NV_ERR_NOT_SUPPORTED,
                params: params.to_vec(),
                label: "refused (no host object for this control's class)",
                note: format!("control {cmd:#x}: not forwarded"),
            };
        };
        match self.host.as_mut() {
            Some(host) => {
                let mut b = params.to_vec();
                let status = host.control(class, cmd, &mut b);
                Answer {
                    status,
                    params: b,
                    label: "host (forwarded as-is)",
                    note: format!("control {cmd:#x} forwarded to the host: status {status:#x}"),
                }
            }
            None => fallback(params.len()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dispatcher(host: Option<Box<dyn HostRm>>) -> Dispatcher {
        let f = crate::fixture::rtx2070();
        Dispatcher::new(f.chip.data(), &f.identity(4 << 30), host)
    }

    /// 0x20800A36 follows the guest ABI: 610 chip info, 615 the fabric-probe sync,
    /// never chip info for a 615 guest.
    #[test]
    fn id_0a36_follows_the_guest_abi() {
        use crate::ver::Id0A36;
        let d = dispatcher(None);
        let chip = d.id_0a36(Id0A36::GetChipInfo, &[0u8; 88]);
        assert_eq!((chip.status, chip.note.as_str()), (NV_OK, "chip info"));
        assert_eq!(get32(&chip.params, 12), 0x1F02_10DE);
        let mut p = [0u8; 16];
        p[0] = 1;
        p[8] = 0xAA;
        let fab = d.id_0a36(Id0A36::SyncFabricProbeInfo, &p);
        assert_eq!((fab.status, &fab.params[..]), (NV_OK, &p[..]));
        assert!(fab.label.contains("SYNC_FABRIC_PROBE_INFO"), "{}", fab.label);
        assert_eq!(
            std::mem::size_of::<
                nvrm_sys::v615::NV2080_CTRL_CMD_INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO_PARAMS,
            >(),
            16
        );
        // Through the dispatcher, the build's guest ABI decides.
        let mut d = dispatcher(None);
        let a = d.control(INTERNAL_CLIENT, 0xCA7A_0003, 0x2080_0A36, &p);
        let want = if v::DRIVER_VERSION == "615.71.09" {
            Id0A36::SyncFabricProbeInfo
        } else {
            Id0A36::GetChipInfo
        };
        assert_eq!(v::CTRL_0A36, want);
        assert_eq!(a.note == "chip info", want == Id0A36::GetChipInfo, "{}", a.note);
    }

    /// The costume board (docs/caraxes/costume-5060.md): chip info carries the 5060 Ti's
    /// PCI ids, the register bases stay the TU106's; the nvidia-smi profile (GET_CHIP_DETAILS
    /// pciDevId) has the costume's device id.
    #[test]
    fn costume_chip_info_has_the_5060_ti_ids_and_tu106_bases() {
        use crate::ver::Id0A36;
        let f = crate::fixture::rtx5060ti_on_tu106();
        let d = Dispatcher::new(f.chip.data(), &f.identity(1 << 30), None);
        let t = dispatcher(None);
        let a = d.id_0a36(Id0A36::GetChipInfo, &[0u8; 88]);
        let b = t.id_0a36(Id0A36::GetChipInfo, &[0u8; 88]);
        assert_eq!(a.status, NV_OK);
        assert_eq!(get32(&a.params, v::CHIP_INFO_PCI_DEVICE_ID), 0x2D04_10DE);
        assert_eq!(
            get32(&a.params, v::CHIP_INFO_PCI_SUB_DEVICE_ID),
            0x8A08_1043
        );
        assert_eq!(get32(&a.params, v::CHIP_INFO_PCI_REVISION_ID), 0xA1);
        assert_eq!(
            a.params[v::CHIP_INFO_REG_BASES..],
            b.params[v::CHIP_INFO_REG_BASES..]
        );
        assert_eq!(d.profile.pci_device_id, 0x2D04);
        assert_eq!(d.bar_sizes, [64 << 20, 256 << 20, 32 << 20, 0x80]);
    }

    #[test]
    fn answers_have_the_request_size() {
        let mut d = dispatcher(None);
        let a = d.id_0a36(crate::ver::Id0A36::GetChipInfo, &[0u8; 88]);
        assert_eq!(a.status, NV_OK);
        assert_eq!(a.params.len(), 88);
        assert_eq!(
            u32::from_le_bytes(a.params[12..16].try_into().unwrap()),
            0x1F02_10DE
        );
        assert_eq!(
            u32::from_le_bytes(a.params[24 + 8..24 + 12].try_into().unwrap()),
            0x9000
        );
        assert_eq!(
            u32::from_le_bytes(a.params[24..28].try_into().unwrap()),
            0xFFFF_FFFF
        );
        assert_eq!(
            std::mem::size_of::<nvrm_sys::v610::NV2080_CTRL_INTERNAL_GPU_GET_CHIP_INFO_PARAMS>(),
            88
        );
        assert_eq!(
            std::mem::size_of::<nvrm_sys::v610::NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS>(),
            12 + 32 * 100
        );
        assert_eq!(
            std::mem::size_of::<
                nvrm_sys::v610::NV2080_CTRL_INTERNAL_GPU_GET_USER_REGISTER_ACCESS_MAP_PARAMS,
            >(),
            20492
        );

        let a = d.control(
            INTERNAL_CLIENT,
            0xCA7A_0003,
            CMD_INTERNAL_INTR_GET_KERNEL_TABLE,
            &[0u8; 2112],
        );
        assert_eq!(a.params.len(), 2112);
        // Six milestone-1 entries plus the GR0/CE2-CE4 non-stall entries of E5b part 2,
        // plus the display vector (display-ab.md) and NVDEC0/NVENC0 (video-engines.md).
        assert_eq!(u32::from_le_bytes(a.params[0..4].try_into().unwrap()), 13);
        assert_eq!(
            u64::from_le_bytes(a.params[2056 + 8..2056 + 16].try_into().unwrap()),
            0x8
        );
        assert_eq!(
            u64::from_le_bytes(a.params[2056 + 40..2056 + 48].try_into().unwrap()),
            0x2
        );

        let a = d.control(
            INTERNAL_CLIENT,
            0xCA7A_0003,
            CMD_INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO,
            &[0u8; 64],
        );
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
        let a = d.control(INTERNAL_CLIENT, 0xCA7A_0003, CMD_BIOS_GET_INFO_V2, &{
            let mut p = [0u8; 20];
            p[0] = 2;
            p[12] = 1;
            p
        });
        assert_eq!(
            (get32(&a.params, 8), get32(&a.params, 16)),
            (0x9006_3D00, 0x76)
        );
        assert_eq!(a.label, "card (VBIOS 90.06.3D.00.76 from the captured BIT)");
        let a = d.control(
            INTERNAL_CLIENT,
            0xCA7A_0003,
            CMD_GSP_GET_FEATURES,
            &[0u8; 72],
        );
        assert_eq!(&a.params[6..15], v::DRIVER_VERSION.as_bytes());
        // An id no driver version defines (0x20800BAD is INTERNAL_NVLINK_RESUME_DISABLE_
        // CHANNELS in 615, refused by the NVLink family rule of chipctl.rs).
        let a = d.control(INTERNAL_CLIENT, 0xCA7A_0003, 0x2080_0FFF, &[0u8; 8]);
        assert_eq!(a.label, "unanswered");

        let a = d.alloc(0x1, 0x1, 0, NV01_ROOT, &[0u8; 8]);
        assert_eq!(a.status, NV_OK);
        let mut p = vec![0u8; 16];
        p[8..16].copy_from_slice(&0x1234_5000u64.to_le_bytes());
        d.control(
            INTERNAL_CLIENT,
            0xCA7A_0003,
            CMD_INTERNAL_INIT_USER_SHARED_DATA,
            &p,
        );
        assert_eq!(d.rusd_pa, Some(0x1234_5000));
        let a = d.free(0x1, 0x1);
        assert_eq!(a.status, NV_OK);
    }

    /// Guest clients own their handle numbers; the class decides the host object.
    #[test]
    fn objects_are_per_client() {
        struct Rec(std::sync::Arc<std::sync::Mutex<Vec<u32>>>);
        impl HostRm for Rec {
            fn control(&mut self, class: u32, _: u32, _: &mut [u8]) -> u32 {
                self.0.lock().unwrap().push(class);
                NV_OK
            }
        }
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut d = dispatcher(Some(Box::new(Rec(seen.clone()))));
        d.alloc(0xA, 0x10, 0xA, NV01_DEVICE_0, &[]);
        d.alloc(0xB, 0x10, 0xB, crate::nvml::NV2081_BINAPI, &[]);
        d.control(0xA, 0x10, 0x0080_0294, &[0u8; 4]);
        d.control(0xB, 0x10, crate::nvml::CMD_BINAPI_0108, &[0u8; 992]);
        d.free(0xB, 0xB);
        // Forgotten with its client: a BINAPI command on it is no longer ours.
        let a = d.control(0xB, 0x10, crate::nvml::CMD_BINAPI_0108, &[0u8; 992]);
        assert_eq!(a.label, "unanswered");
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &[NV01_DEVICE_0, crate::nvml::NV2081_BINAPI]
        );
    }

    /// The guest's L2 cache operations reach the host as FB_FLUSH_GPU_CACHE on the
    /// device's subdevice: full cache, sysmem aperture, the operation's flag bit, in the
    /// selected host's flag layout (the per-host words: hostabi.rs
    /// `flush_gpu_cache_per_host`).
    #[test]
    fn flush_gpu_cache_parameters() {
        use crate::hostabi::{current, L2Flush, FLUSH_GPU_CACHE_FLAGS_OFF};
        type Seen = std::sync::Arc<std::sync::Mutex<Vec<(u32, u32, usize, u32)>>>;
        struct Rec(Seen);
        impl HostRm for Rec {
            fn control(&mut self, class: u32, cmd: u32, p: &mut [u8]) -> u32 {
                let o = FLUSH_GPU_CACHE_FLAGS_OFF;
                let flags = u32::from_le_bytes(p[o..o + 4].try_into().unwrap());
                self.0.lock().unwrap().push((class, cmd, p.len(), flags));
                NV_OK
            }
        }
        assert_eq!(dispatcher(None).host_flush_gpu_cache(L2Flush::WriteBack), None);
        let seen: Seen = Default::default();
        let mut d = dispatcher(Some(Box::new(Rec(seen.clone()))));
        let l = current();
        let (wb, inv) = (
            l.flush_gpu_cache_flags(L2Flush::WriteBack),
            l.flush_gpu_cache_flags(L2Flush::Invalidate),
        );
        assert_eq!(d.host_flush_gpu_cache(L2Flush::WriteBack), Some((wb, NV_OK)));
        assert_eq!(d.host_flush_gpu_cache(L2Flush::Invalidate), Some((inv, NV_OK)));
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &[
                (NV20_SUBDEVICE_0, 0x2080_130E, 4024, wb),
                (NV20_SUBDEVICE_0, 0x2080_130E, 4024, inv)
            ]
        );
    }
}

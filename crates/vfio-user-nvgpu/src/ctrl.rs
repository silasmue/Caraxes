// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Answers for the RM API RPCs of phase 5 (docs/caraxes/phase5-rpcs.md), one handler
//! per row, each labelled with the provenance of its answer (decision D9): `host`
//! (forwarded to the host RM as an unprivileged client), `guest` (derived from what
//! the guest sent), `card` (captured), `synthesized` (a constant chosen here).
//! Layouts come from the SDK headers named in the inventory; sizes are asserted in
//! the tests against the typed bindings in `nvrm-sys` where those exist.

use std::collections::BTreeMap;

mod deferred_context;

use crate::chip::ChipData;
use crate::rpc::StaticIdentity;
// Classes, control numbers and parameter layouts of the emulated driver (src/ver).
use crate::ver::common as v;
pub use crate::ver::common::{
    CMD_BIOS_GET_INFO_V2, CMD_BUS_GET_PCI_BAR_INFO, CMD_CE_GET_FAULT_METHOD_BUFFER_SIZE,
    CMD_CE_UPDATE_CLASS_DB, CMD_CE_UPDATE_PCE_LCE_MAPPINGS_V2, CMD_EVENT_SET_NOTIFICATION,
    CMD_FB_GET_GPU_CACHE_INFO, CMD_FIFO_GET_DEVICE_INFO_TABLE,
    CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES, CMD_GPU_GET_CONSTRUCTED_FALCON_INFO,
    CMD_GPU_GET_OEM_BOARD_INFO, CMD_GPU_GET_VMMU_SEGMENT_SIZE, CMD_GR_CTXSW_ZCULL_BIND,
    CMD_GSP_GET_FEATURES, CMD_I2C_GET_PORT_INFO, CMD_INTERNAL_BIF_GET_STATIC_INFO,
    CMD_INTERNAL_BUS_FLUSH_WITH_SYSMEMBAR, CMD_INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_TYPE,
    CMD_INTERNAL_FIFO_GET_NUM_CHANNELS, CMD_INTERNAL_GET_PCIE_P2P_CAPS,
    CMD_INTERNAL_GMMU_GET_STATIC_INFO, CMD_INTERNAL_GPU_GET_CHIP_INFO,
    CMD_INTERNAL_GPU_GET_USER_REGISTER_ACCESS_MAP, CMD_INTERNAL_INIT_USER_SHARED_DATA,
    CMD_INTERNAL_INTR_GET_KERNEL_TABLE, CMD_INTERNAL_MEMSYS_GET_STATIC_CONFIG,
    CMD_INTERNAL_MEMSYS_L2_INVALIDATE_EVICT, CMD_INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO,
    CMD_INTERNAL_PERF_GPU_BOOST_SYNC_GET_INFO, CMD_INTERNAL_USER_SHARED_DATA_SET_DATA_POLL,
    NV01_DEVICE_0, NV01_EVENT_KERNEL_CALLBACK_EX, NV01_ROOT, NV20_SUBDEVICE_0,
    NV_ERR_NOT_SUPPORTED, NV_OK,
};

/// `NV_ERR_INSUFFICIENT_PERMISSIONS` (nvstatuscodes.h).
/// `NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND` (ctrl2080gr.h:917-929).
const CMD_GR_CTXSW_PREEMPTION_BIND: u32 = 0x2080_1211;
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
    /// The resource handle generator of each client (`handleGenIdx`, rs_client.c) and the
    /// client handle generator (`assign_handle`).
    handle_gen: BTreeMap<u32, u32>,
    client_handle_gen: u32,
    deferred_context: deferred_context::State,
    /// PCI ids of the emulated function (config space).
    pub pci_device_id: u32,
    pub pci_sub_device_id: u32,
    pub pci_revision: u32,
    /// BAR sizes (board data) and the addresses the guest reported (GspSystemInfo).
    pub bar_sizes: Vec<u64>,
    pub bar_offsets: [u64; 4],
    /// The guest driver's `NVGMC_SI_HOST_OS_TYPE` from GSP_INIT (`None` before it or on the
    /// 610 transport): selects the `GSP_GET_FEATURES` UVM bit.
    pub guest_os_type: Option<u32>,
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
    /// Guest and host framebuffer coordinates.
    fb_layout: Option<crate::fb::FbLayout>,
    /// What the guest set on its channels and TSGs (public.rs, gsp-plan/c-guest-state.md).
    pub guest_attrs: crate::public::GuestAttrs,
    /// The category D models of this VM (dmodel.rs, rc.rs, zbc.rs; gsp-plan/d-device-model.md).
    pub dstate: crate::dmodel::DState,
    /// The feature-family models of this VM: ECC, MIG (families/, gsp-plan/e-families.md §4).
    pub families: crate::families::Families,
    /// The emulated PCIe Link Control register as the guest last left it (device.rs sets
    /// it after every config-space write): `NV208F BIF_INFO` answers from it (public.rs).
    pub link_control: u16,
    /// How trapped doorbell values were resolved (`route_token`), for the exit summary.
    pub routing: TokenRouting,
    /// A channel whose work-submit-token entry was given its pool token when its mirror
    /// was scheduled in this doorbell (`pin_pool_token`); `doorbell` does not hand out
    /// again for it.
    pinned_now: Option<u32>,
}

/// Counters of the doorbell value resolution (`route_token`; docs/caraxes/doorbell-token-pool.md
/// (e) rule 9, the 2026-09-26 collision).
#[derive(Default, Debug, Clone)]
pub struct TokenRouting {
    /// Rings resolved to a mirrored channel by its host token (no guest channel of that ChID).
    pub by_host_token: u64,
    /// Rings whose ChID named a guest channel while the value was also another channel's
    /// host token: the old order (host token first) would have rung that other channel.
    pub collisions_avoided: u64,
    /// Mirrored channels whose notifier entry got its pool token at the mirror's scheduling
    /// (`pin_pool_token`), before the guest could read the host token back.
    pub pinned_at_schedule: u64,
}

impl TokenRouting {
    pub fn summary(&self) -> String {
        format!(
            "doorbell routing: {} ring(s) by host token, {} collision(s) avoided (a guest channel's own token that is also another channel's host token; the old order rang the other channel), {} pool token(s) written at the mirror's scheduling",
            self.by_host_token, self.collisions_avoided, self.pinned_at_schedule
        )
    }
}

/// Where a trapped doorbell value goes (`route_token`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    /// The channel rung (and handed a pool token).
    pub chid: u32,
    /// The value was the host token of `chid`'s own mirror.
    pub via_host_token: bool,
    /// A second channel rung as well: the mirrored channel whose host token has the same
    /// value (a spurious doorbell is harmless, a lost one is not).
    pub also: Option<u32>,
}

/// Resolve a trapped, non-pool doorbell value. A guest token is the channel's ChID in
/// bits 11:0 (NV_CTRL_VF_DOORBELL_VECTOR, tu102/dev_ctrl.h:36-37); a mirrored channel
/// without a pool token rings the host token it read back from its notifier (execution.md
/// §6). Host ChIDs and guest ChIDs overlap (both are allocated from 0 on the same
/// runlists), so a value can be both: then the guest channel of that ChID is rung first,
/// because its first ring has no other way to reach the device, and the host token's
/// owner is rung too. Before 2026-09-26 the host token was tried first, and a new GR
/// channel whose guest token equalled an older mirror's host token was never built
/// (vm4-615-cuda-vulkan/NOTES.md, "cause found").
pub fn route_token(reg: &crate::channel::Registry, token: u32, host_owner: Option<u32>) -> Route {
    let guest = token & 0xFFF;
    let named = reg.channel(guest).is_some();
    match (named, host_owner) {
        (true, Some(h)) if h != guest => Route {
            chid: guest,
            via_host_token: false,
            also: Some(h),
        },
        (true, h) => Route {
            chid: guest,
            via_host_token: h == Some(guest),
            also: None,
        },
        (false, Some(h)) => Route {
            chid: h,
            via_host_token: true,
            also: None,
        },
        (false, None) => Route {
            chid: guest,
            via_host_token: false,
            also: None,
        },
    }
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

/// `NV0080_CTRL_DMA_SET_DEFAULT_VASPACE` (ctrl0080dma.h): `{ NvHandle hVASpace; }`.
const CMD_DMA_SET_DEFAULT_VASPACE: u32 = 0x0080_1812;
/// `NV_ERR_INVALID_OBJECT_HANDLE` (nvstatuscodes.h).
const NV_ERR_INVALID_OBJECT_HANDLE: u32 = 0x33;

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
            handle_gen: BTreeMap::new(),
            client_handle_gen: 0,
            deferred_context: deferred_context::State::default(),
            pci_device_id: id.pci.device_id(),
            pci_sub_device_id: id.pci.sub_device_id(),
            pci_revision: id.pci.revision as u32,
            bar_sizes: id.bar_sizes.clone(),
            bar_offsets: [0; 4],
            guest_os_type: None,
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
            fb_layout: None,
            guest_attrs: Default::default(),
            dstate: crate::dmodel::DState::new(chip),
            families: crate::families::Families::new(chip, id.fb_bytes),
            link_control: 0,
            routing: TokenRouting::default(),
            pinned_now: None,
        }
    }

    /// The handle RM gives an allocation whose `hObject` is 0 (`NV01_NULL_OBJECT`), as
    /// resserv assigns it before the class sees the request (`clientAssignResourceHandle`,
    /// rs_client.c:985-1005 of 570.144): a resource gets the next free handle of its
    /// client's generator, `RS_UNIQUE_HANDLE_BASE` + (index mod `RS_UNIQUE_HANDLE_RANGE`)
    /// (0xcaf00000 and 0x80000, resserv/rs_client.h:37-38; `clientGenResourceHandle`
    /// :948-980); a client (the root classes) the next free one from
    /// `RS_CLIENT_HANDLE_BASE` 0xC1D00000 within `RS_CLIENT_HANDLE_MAX` 0x100000
    /// (resserv.h:135, 152). `None` when the range is used up (RM:
    /// NV_ERR_INSUFFICIENT_RESOURCES). nouveau's r535/r570 path allocates the objects a
    /// user channel carries with handle 0 (vm10-boot4: two copy objects on one NVK channel).
    pub fn assign_handle(&mut self, h_client: u32, h_class: u32) -> Option<u32> {
        const RS_UNIQUE_HANDLE_BASE: u32 = 0xcaf0_0000;
        const RS_UNIQUE_HANDLE_RANGE: u32 = 0x0008_0000;
        const RS_CLIENT_HANDLE_BASE: u32 = 0xc1d0_0000;
        const RS_CLIENT_HANDLE_MAX: u32 = 0x0010_0000;
        if matches!(h_class, NV01_ROOT | v::NV01_ROOT_NON_PRIV | v::NV01_ROOT_CLIENT) {
            for _ in 0..RS_CLIENT_HANDLE_MAX {
                let h = RS_CLIENT_HANDLE_BASE + self.client_handle_gen % RS_CLIENT_HANDLE_MAX;
                self.client_handle_gen = self.client_handle_gen.wrapping_add(1);
                if !self.objects.keys().any(|&(c, _)| c == h) {
                    return Some(h);
                }
            }
            return None;
        }
        let idx = self.handle_gen.entry(h_client).or_insert(0);
        for _ in 0..RS_UNIQUE_HANDLE_RANGE {
            let h = RS_UNIQUE_HANDLE_BASE + *idx % RS_UNIQUE_HANDLE_RANGE;
            *idx = idx.wrapping_add(1);
            if !self.objects.contains_key(&(h_client, h))
                && self.channels.engine_object(h_client, h).is_none()
            {
                return Some(h);
            }
        }
        None
    }

    /// The dispatcher of a device holding the VRAM reservation `r` (`FbLayout::reservation`).
    pub fn with_reservation(mut self, r: Option<(u64, u64)>) -> Self {
        self.reservation = r;
        self
    }

    pub fn with_layout(mut self, layout: crate::fb::FbLayout) -> Self {
        self.reservation = layout.host_reservation;
        self.fb_layout = Some(layout);
        self
    }

    /// What this VM's guest is told about compression (compression.rs): `None` is D16,
    /// no compression, today's answers.
    pub fn compression(&self) -> Option<crate::compression::GuestView> {
        crate::compression::GuestView::of(self.fb_layout.as_ref()?, self.chip)
    }

    pub fn take_host(&mut self) -> Option<Box<dyn HostRm>> {
        self.host.take()
    }

    /// The guest channels a doorbell write of `token` rings ([`Self::doorbell`]), without
    /// ringing them: the pool token's channel, else the channel the value names and the
    /// channel whose host token it also is (`route_token`). For the BAR2 memfd's doorbell
    /// rule, which must know them before the ring (boot.rs `bar2_doorbell_sync`).
    pub fn doorbell_targets(&self, token: u32) -> Vec<u32> {
        if let Some(p) = self.channels.tokens.as_ref().filter(|p| p.contains(token)) {
            return p.chid(token).into_iter().collect();
        }
        let host_owner = self
            .mirror
            .as_ref()
            .and_then(|m| m.chid_of_host_token(token));
        let route = route_token(&self.channels, token, host_owner);
        std::iter::once(route.chid).chain(route.also).collect()
    }

    /// The channels with GPFIFO entries published since their last doorbell, which the
    /// token-less doorbell rings ([`Self::doorbell_pending`]).
    pub fn pending_channels(&self, gm: &crate::channel::GpuMem) -> Vec<u32> {
        let chids: Vec<u32> = self.channels.channels().map(|c| c.chid).collect();
        chids
            .into_iter()
            .filter(|c| self.channels.pending(gm, *c))
            .collect()
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
        // The guest channel named by the ChID comes first (`route_token`): the host token
        // is only a fallback for a value that names no guest channel.
        let host_owner = self
            .mirror
            .as_ref()
            .and_then(|m| m.chid_of_host_token(token));
        let route = route_token(&self.channels, token, host_owner);
        let mut notes = Vec::new();
        if let Some(other) = route.also {
            self.routing.collisions_avoided += 1;
            notes.push(format!(
                "doorbell {token:#x}: guest chid {} rung first; the value is also the host token of mirrored chid {other} (the old order rang chid {other} only) [collision avoided]",
                route.chid
            ));
        } else if route.via_host_token && route.chid != (token & 0xFFF) {
            self.routing.by_host_token += 1;
        }
        self.pinned_now = None;
        notes.extend(self.doorbell_chid(gm, token, route.chid, route.via_host_token));
        if self.pinned_now != Some(route.chid) {
            notes.extend(self.hand_out_token(gm, route.chid, token));
        }
        if let Some(other) = route.also {
            self.pinned_now = None;
            notes.extend(self.doorbell_chid(gm, token, other, true));
            if self.pinned_now != Some(other) {
                notes.extend(self.hand_out_token(gm, other, token));
            }
        }
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
        let note = |token: u32, chid: u32| {
            format!(
                "doorbell token {token:#x} ({}) -> chid {chid}",
                if trapped { "trapped" } else { "eventfd" }
            )
        };
        // The software executor decodes a token as ChID 11:0, runlist 22:16; ring the
        // channel with a guest token of it (runlist 0, as the token-less path does).
        if crate::launch::fast() {
            // The fast ring path formats the note after the host doorbell.
            let mut notes = self.doorbell_chid(gm, chid, chid, false);
            notes.insert(0, note(token, chid));
            return notes;
        }
        let mut notes = vec![note(token, chid)];
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
        let host = self
            .mirror
            .as_ref()
            .and_then(|m| m.channel(chid))
            .map(|h| h.token);
        self.hand_out_token_with(gm, chid, rung, host)
    }

    /// `hand_out_token` with the mirror's host token of `chid` given (`None`: no mirror).
    fn hand_out_token_with(
        &mut self,
        gm: &mut crate::channel::GpuMem,
        chid: u32,
        rung: u32,
        host: Option<u32>,
    ) -> Vec<String> {
        // The channel's error notifier: `None` for a channel the guest never announced.
        let notifier = self.channels.channel(chid).map(|c| c.error_notifier);
        // The values that are this channel's own token on its notifier page: the rung one
        // and the mirror's host token, each only if no other channel it rings keeps its
        // notifier on the same page (tokens.rs `own_on_page`).
        let page = |t: crate::fb::Target| match t {
            crate::fb::Target::Fb(pa) => Some((true, pa >> 12)),
            crate::fb::Target::Sys(pa) => Some((false, pa >> 12)),
            _ => None,
        };
        let own: Vec<u32> = match notifier.flatten().and_then(page) {
            Some(p) => std::iter::once(rung)
                .chain(host)
                .filter(|&v| {
                    crate::tokens::own_on_page(chid, &self.doorbell_targets(v), p, |c| {
                        self.channels.channel(c).and_then(|c| c.error_notifier).and_then(page)
                    })
                })
                .collect(),
            None => Vec::new(),
        };
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
        let at_off = |off: u64| match notifier {
            Target::Fb(pa) => Target::Fb(pa + off),
            Target::Sys(pa) => Target::Sys(pa + off),
            other => other,
        };
        let at = at_off(AT);
        let current = gm.read_u32(at);
        // The token's entry at another index (a client's own
        // SET_WORK_SUBMIT_TOKEN_NOTIF_INDEX, which the guest RM handles without a GSP
        // call): found by the RM's signature. SotTR run E: 43273 rings kept their own token
        // because entry 1 held 0, and 57274 rang their own token after a hand-out to entry 1.
        // The scan stays inside the notifier's own page and matches only the channel's own
        // token values with the full signature.
        let base = match notifier {
            Target::Fb(pa) | Target::Sys(pa) => pa,
            _ => 0,
        };
        let span = (16 * crate::tokens::TOKEN_SCAN_ENTRIES).min(0x1000 - (base & 0xFFF) as usize);
        let mut buf = vec![0u8; span];
        let elsewhere: Vec<Target> = if !own.is_empty() && gm.read(notifier, &mut buf) {
            crate::tokens::token_entries(&buf, &own)
                .into_iter()
                .map(|i| at_off(16 * i as u64 + 8))
                .collect()
        } else {
            Vec::new()
        };
        let default_holds = current.is_some_and(|c| own.contains(&c));
        if held.is_some() && current == held && elsewhere.is_empty() {
            pool.stats.own_token_after_handout += 1;
            return Vec::new();
        }
        if !default_holds && elsewhere.is_empty() {
            pool.stats.mismatch += 1;
            let why = format!(
                "chid {chid}: work-submit-token entry at {at:?} holds {current:x?}, not the rung token {rung:#x}{}{}",
                host.map_or(String::new(), |h| format!(" or the host token {h:#x}")),
                if own.len() < 1 + usize::from(host.is_some()) {
                    format!(" (own values {own:x?}: a value another channel on the notifier page rings is not the channel's)")
                } else {
                    String::new()
                }
            );
            pool.note_refused(why.clone());
            return vec![format!(
                "doorbell tokens: {why}; it keeps its own token (trapped)"
            )];
        }
        let Some(token) = held.or_else(|| pool.assign(chid)) else {
            pool.stats.exhausted += 1;
            return vec![format!(
                "doorbell tokens: pool of {} exhausted; chid {chid} keeps token {rung:#x} (trapped)",
                pool.len()
            )];
        };
        let targets: Vec<Target> = default_holds
            .then_some(at)
            .into_iter()
            .chain(elsewhere.iter().copied())
            .collect();
        for t in &targets {
            if !gm.write(*t, &token.to_le_bytes()) {
                if held.is_none() {
                    pool.release(chid);
                }
                pool.stats.mismatch += 1;
                return vec![format!(
                    "doorbell tokens: chid {chid}: write of {token:#x} to {t:?} failed; it keeps its own token (trapped)"
                )];
            }
        }
        if matches!(at, crate::fb::Target::Fb(_)) {
            gm.fb.flush();
        }
        if !elsewhere.is_empty() {
            pool.stats.elsewhere += 1;
        }
        if held.is_some() {
            pool.stats.rewritten += 1;
        } else {
            pool.stats.handed_out += 1;
        }
        vec![format!(
            "doorbell tokens: chid {chid} {} pool token {token:#x} (was {rung:#x}{}), written to its work-submit-token notifier entr{} at {targets:?} [synthesized]",
            if held.is_some() { "gets back its" } else { "gets" },
            match host {
                Some(h) if current == Some(h) => format!("; the entry held the host token {h:#x}"),
                _ => String::new(),
            },
            if targets.len() == 1 { "y" } else { "ies" }
        )]
    }

    /// At the scheduling of `chid`'s host mirror (this doorbell built it): the host RM has
    /// just written the host token `host` into the channel's work-submit-token entry (the
    /// guest's own notifier page). Replace it with the channel's pool token right here, so
    /// a raw host token never reaches the guest: a trapped ring may still be waiting for
    /// this answer, but a ring through an eventfd does not block the guest. Only a proven
    /// host token is overwritten (rule 4); without a pool nothing changes.
    pub(crate) fn pin_pool_token(
        &mut self,
        gm: &mut crate::channel::GpuMem,
        chid: u32,
        host: u32,
    ) -> Vec<String> {
        let Some(Some(notifier)) = self.channels.channel(chid).map(|c| c.error_notifier) else {
            return Vec::new();
        };
        if self.channels.tokens.is_none() {
            return Vec::new();
        }
        use crate::fb::Target;
        use crate::tokens::NOTIFIER_TOKEN_OFFSET as AT;
        let at = match notifier {
            Target::Fb(pa) => Target::Fb(pa + AT),
            Target::Sys(pa) => Target::Sys(pa + AT),
            other => other,
        };
        if gm.read_u32(at) != Some(host) {
            return Vec::new();
        }
        let notes = self.hand_out_token_with(gm, chid, host, Some(host));
        let pool = self.channels.tokens.as_ref();
        if pool.and_then(|p| p.token_of(chid)).is_some() && gm.read_u32(at) != Some(host) {
            self.routing.pinned_at_schedule += 1;
            self.pinned_now = Some(chid);
        }
        notes
    }

    /// Poll private USERDs, then reuse normal submission and scheduling checks.
    /// The USERD poller's rings (main.rs, execution.md §9.11): each channel it found with
    /// entries not yet forwarded is checked again under the device lock as the locked poll
    /// checks it (`Mirror::userd_pending`) and rung as that poll rang it.
    pub fn ring_userd_pending(&mut self, gm: &mut crate::channel::GpuMem, chids: &[u32]) -> Vec<String> {
        let mut notes = Vec::new();
        for &chid in chids {
            let pending = self
                .mirror
                .as_mut()
                .is_some_and(|m| m.userd_pending(&self.channels, gm, chid));
            if pending {
                notes.extend(self.doorbell_chid(gm, chid, chid, false));
            }
        }
        notes
    }

    pub fn poll_shadow_userds(&mut self, gm: &mut crate::channel::GpuMem) -> Vec<String> {
        let pending = self.mirror.as_mut()
            .map(|m| m.poll_userds(&self.channels, gm)).unwrap_or_default();
        let mut notes = Vec::new();
        for chid in pending {
            notes.extend(self.doorbell_chid(gm, chid, chid, false));
        }
        notes
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
        // A channel held for its VAS root (`root_hold`) stays held, quietly, until the
        // guest's invalidates name the walked root again.
        let (run, keep): (Vec<u32>, Vec<u32>) = held.into_iter().partition(|c| {
            !off.contains(c) && self.channels.root_mismatch(*c).is_none()
        });
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
            self.channels
                .forget_engine_object(h_client, h_parent, h_class);
            self.objects.remove(&(h_client, h_object));
        }
        (notes, refused)
    }

    /// Fix 2 of run P17 (pdbwatch.rs): a channel bound for the host whose VAS is walked from
    /// a root the guest's own TLB invalidates no longer name runs nothing there (the host
    /// would fetch its GPFIFO through the wrong tables: Xid 31 `FAULT_PDE`). Its doorbell is
    /// held (`replay_held` runs it once the guest names the walked root); one ERROR per
    /// mismatch, a short note for every further held doorbell.
    fn root_hold(&mut self, chid: u32) -> Option<Vec<String>> {
        let (vas, mm) = self.channels.root_mismatch(chid)?;
        let m = self.mirror.as_ref()?;
        let to_host = m.is_mirrored(chid)
            || m.is_shadow(&self.channels, chid)
            || m.eligible(&self.channels, chid).is_ok();
        if !to_host {
            return None;
        }
        self.dstate.rc.held.insert(chid);
        let watch = self.channels.pdb_watch_mut();
        watch.holds += 1;
        if !watch.first_report(vas, &mm) {
            return Some(vec![format!(
                "chid {chid}: host doorbell held, VAS root mismatch (see the ERROR above) [host protection]"
            )]);
        }
        let text = format!(
            "ERROR: chid {chid}: VAS {:#x}/{:#x} is walked from root {:?}, but the guest's TLB invalidates name its former root {:?} ({} since the change); the host doorbell is held until the guest names the walked root (pdbwatch.rs) [host protection]",
            vas.0, vas.1, mm.walked, mm.named, mm.count
        );
        eprintln!("vfio-user-nvgpu: {text}");
        Some(vec![text])
    }

    fn doorbell_chid(
        &mut self,
        gm: &mut crate::channel::GpuMem,
        token: u32,
        chid: u32,
        via_host_token: bool,
    ) -> Vec<String> {
        // Every route of a guest doorbell ends here: activity for the host clock boost
        // (hostboost.rs; one relaxed load while a renewal is owed).
        crate::hostboost::activity();
        // The device's private pages at the top of the reservation (devce.rs `PRIVATE_AT`:
        // the device CE's and the invalidator's tables and rings) are no guest memory.
        self.channels.software_fb_range = self
            .fb_layout
            .map_or(self.reservation, |l| l.reservation)
            .map(|(b, e)| (b, e.saturating_sub(crate::devce::PRIVATE_AT).max(b)));
        let software_runs = self.channels.software_runs;
        let t0 = std::time::Instant::now();
        crate::dlock::take_spent();
        let tlb0 = gm.fb.exec_tlb_stats();
        let maps0 = gm.fb.vram_stats().map_or(0, |v| v.maps);
        let mut notes = self.doorbell_chid_inner(gm, token, chid, via_host_token);
        let spent = t0.elapsed();
        let [translate, copy] = crate::dlock::take_spent();
        let tlb1 = gm.fb.exec_tlb_stats();
        let maps = gm.fb.vram_stats().map_or(0, |v| v.maps) - maps0;
        let software = self.channels.software_runs != software_runs;
        // The device lock's tags and work table (dlock.rs): what a channel's doorbell cost,
        // with a software run's translations and CPU copies apart.
        let kind = if software { "software" } else { "host" };
        if spent >= std::time::Duration::from_millis(1) {
            crate::dlock::tag(|| {
                format!(
                    "doorbell chid {chid} ({kind}) {} ms (translate {} ms: {} page hit(s), {} unit walk(s); CPU copy {} ms; {maps} window map(s))",
                    spent.as_millis(),
                    translate.as_millis(),
                    tlb1.0 - tlb0.0,
                    tlb1.1 - tlb0.1,
                    copy.as_millis()
                )
            });
        }
        crate::dlock::work("doorbell", kind.to_string(), spent);
        if !software {
            return notes;
        }
        // All software routes (including blocked peers retried by Registry) publish
        // memory before their interrupt, and report a failed method as a guest RC.
        gm.fb.flush();
        for event in self.channels.take_software_events() {
            if let Some(failure) = event.failure {
                let xid = crate::ver::Abi::active()
                    .d_enum("ROBUST_CHANNEL_PREEMPTIVE_REMOVAL")
                    .expect("generated RC removal enum");
                let text = format!("Caraxes software CE chid {}: {failure}", event.chid);
                self.dstate.rc.pending.push(crate::rc::RcRequest::Channel {
                    chid: event.chid,
                    engine_type: event.engine_type,
                    xid,
                    text: text.clone(),
                });
                self.dstate.rc.record(Some(event.chid), xid, text.clone());
                self.dstate.rc.held.remove(&event.chid);
                notes.push(format!("{text}; completion withheld, guest RC queued"));
            } else if event.nonstall {
                match self
                    .mirror
                    .as_ref()
                    .and_then(|m| m.raise_software_nonstall(event.engine_type))
                {
                    Some(vector) => notes.push(format!(
                        "software chid {}: non-stall vector {vector} raised after completion",
                        event.chid
                    )),
                    None => notes.push(format!(
                        "software chid {}: non-stall interrupt unavailable for engine {:#x}",
                        event.chid, event.engine_type
                    )),
                }
            }
        }
        // A mirrored GPU can satisfy a software acquire after this doorbell returns.
        // The existing periodic replay path retries held channels without re-executing
        // the preceding methods (Registry retains the blocking method's resume point).
        self.dstate
            .rc
            .held
            .extend(self.channels.blocked_software_channels());
        notes
    }

    fn doorbell_chid_inner(
        &mut self,
        gm: &mut crate::channel::GpuMem,
        token: u32,
        chid: u32,
        via_host_token: bool,
    ) -> Vec<String> {
        self.channels.note_rung(gm, chid);
        // The software CE's host copy engine (devce.rs), built once with the host at hand.
        if let Some(m) = self.mirror.as_mut() {
            m.ensure_device_ce(gm.fb);
        }
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
        if self.deferred_context.is_management(&self.channels, chid) {
            return self.run_deferred_management(gm, chid, &off);
        }
        if self.deferred_context.target_submission_blocked(&self.channels, chid) {
            return vec![format!(
                "deferred target chid {chid}: submission held until context promotion"
            )];
        }
        if let Some(notes) = self.root_hold(chid) {
            return notes;
        }
        let Some(m) = self.mirror.as_mut() else {
            return self.channels.doorbell_except(gm, token, &off);
        };
        // A GR channel under CARAXES_MIRROR_GR_SHADOW runs on the host-managed shadow VAS
        // (D17 route 4 stage 1, shadow.rs), built at its first doorbell.
        let host_doorbells = m.stats.doorbells;
        if m.is_shadow(&self.channels, chid) {
            let mut notes = m.gr_shadow_doorbell(&self.channels, gm, chid);
            // Builds committed in this call (mirror.rs `take_committed`): their tokens are
            // pinned below.
            let committed = Self::after_mirror_builds(m, &self.channels, &mut self.dstate, gm, &mut notes);
            // The host doorbell write is the last step of a built shadow channel's ring.
            if m.stats.doorbells != host_doorbells {
                self.host_doorbell_at
                    .get_or_insert_with(std::time::Instant::now);
            }
            if let Some(c) = self.channels.channel_mut(chid) {
                c.mirrored = true;
            }
            // Forward the guest's own MEM_OP TLB invalidates too (the shadow VAS's PDB is
            // the host root; the invalidator handles the guest root for the CE channels).
            // Each reaches every mirror VA space, as before the decode read MEM_OP_C PDB 0:0
            // right (pdbwatch.rs): forwarding only the named root waits for a GPU run that
            // shows the mirror roots equal the guest's PDBs.
            let inv = std::mem::take(&mut self.channels.tlb_invalidates);
            if !inv.is_empty() {
                let n: usize = inv.iter().map(|_| m.invalidate_tlb(gm.fb, None)).sum();
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
            for (c, h) in committed {
                notes.extend(self.pin_pool_token(gm, c, h));
            }
            return notes;
        }
        let eligible = m.is_mirrored(chid) || m.eligible(&self.channels, chid).is_ok();
        if !eligible {
            let why = m.eligible(&self.channels, chid).err();
            // The software executor's TLB invalidates reach the physical GPU's TLBs
            // through the mirror VA spaces (every one, as above). One that names a shadow
            // root (nvidia-uvm's page-table updates of a CUDA VA space end in one) stops its
            // push until the shadow worker applied that root, so UVM's semaphore release
            // after it reaches the guest only once the host mappings exist (channel.rs
            // `doorbell_holding`, docs/research/shadow-fault-race-2026-09-28.md).
            let hold = m.memop_hold_roots();
            let mut notes = self.channels.doorbell_holding(
                gm,
                token,
                &off,
                hold,
                &mut |reg, gm, inv| {
                    // The BAR1 area's replicas first (bar1pool.rs `sync_untracked`): a guest
                    // page table the guest CPU wrote through the area (Windows maps its page
                    // tables into BAR1) reaches the walks as a tracked write before the
                    // invalidate plans the shadow, as at the register invalidate
                    // (docs/caraxes/bar1-pool.md §13).
                    crate::bar1pool::sync_untracked_tables(gm.fb);
                    let n: usize = inv.iter().map(|_| m.invalidate_tlb(gm.fb, None)).sum();
                    let mut notes = vec![format!(
                        "mirror: {} software TLB invalidate(s) -> invalidated on {n} mirror VA space(s) [host]",
                        inv.len()
                    )];
                    notes.extend(m.software_invalidated(reg, gm, inv));
                    notes
                },
            );
            if let Some(why) = why {
                notes.push(format!("mirror: chid {chid} in software: {why}"));
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
        let fresh = !m.is_mirrored(chid);
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
                        self.host_doorbell_at
                            .get_or_insert_with(std::time::Instant::now);
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
        // Built in this doorbell: the host token the host RM just wrote into the guest's
        // notifier entry becomes the channel's pool token now (`pin_pool_token`).
        if fresh {
            let host = self
                .mirror
                .as_ref()
                .and_then(|m| m.channel(chid))
                .map(|h| h.token);
            if let Some(h) = host {
                notes.extend(self.pin_pool_token(gm, chid, h));
            }
        }
        notes
    }

    /// After a host RC with a graphics exception (exceptType 13) on mirrored channel `chid`:
    /// the 2D engine's surface state (FERMI_TWOD_A, cl902d.h: SET_DST_* 0x200-0x224,
    /// SET_SRC_* 0x230-0x254) at each blit (`PIXELS_FROM_MEMORY_SRC_Y0_INT` 0x8dc) of the
    /// channel's last 16 GPFIFO entries, each surface VA with the guest's PTE (its kind) and
    /// whether set B covers it (then the host's own context buffer is mapped there, not the
    /// guest's page). For the host Xid 13 "2D KIND Violation" of SotTR (2026-09-28). Read-only,
    /// once per RC. A subchannel whose SET_OBJECT is older than these entries is taken for
    /// 2D when it is subchannel 3 (the NVIDIA driver's binding), and says so.
    pub fn gr_exception_postmortem(&self, gm: &crate::channel::GpuMem, chid: u32) -> Vec<String> {
        use crate::fb::Target;
        const ENTRIES: u32 = 16;
        const MAX_DWORDS: usize = 1 << 16;
        const TWOD: u32 = 0x902D;
        #[derive(Default, Clone, Copy, Debug)]
        struct Surf {
            format: u32,
            layout: u32,
            block: u32,
            pitch: u32,
            width: u32,
            height: u32,
            upper: u32,
            lower: u32,
        }
        impl Surf {
            fn set(&mut self, off: u32, d: u32) {
                match off {
                    0x00 => self.format = d,
                    0x04 => self.layout = d,
                    0x08 => self.block = d,
                    0x14 => self.pitch = d,
                    0x18 => self.width = d,
                    0x1C => self.height = d,
                    0x20 => self.upper = d,
                    0x24 => self.lower = d,
                    _ => {}
                }
            }
            fn va(&self) -> u64 {
                ((self.upper as u64 & 0xFF) << 32) | self.lower as u64
            }
        }
        let mut notes = Vec::new();
        let Some(ch) = self.channels.channel(chid) else {
            return notes;
        };
        let (Some(userd), Some(root)) = (
            ch.userd,
            self.channels.vas_key_of(ch).and_then(|k| self.channels.vas_root(k)),
        ) else {
            notes.push(format!("gr postmortem: chid {chid}: no USERD or VAS root"));
            return notes;
        };
        let at = |t: Target, off: u64| match t {
            Target::Fb(pa) => Target::Fb(pa + off),
            Target::Sys(pa) => Target::Sys(pa + off),
            t => t,
        };
        let (get, put) = (gm.read_u32(at(userd, 0x88)), gm.read_u32(at(userd, 0x8C)));
        let set_b = match (root, self.mirror.as_ref()) {
            (Target::Fb(r), Some(m)) => m.set_b_of(&self.channels, r),
            _ => Vec::new(),
        };
        let walk = |va: u64| -> String {
            let (t, pte) = gm.fb.walk_in(Some(gm.sys), root, va);
            let b = set_b.iter().find(|(lo, hi)| va >= *lo && va < *hi);
            format!(
                "va {va:#x} -> {t:?} pte {pte:#018x} kind {:#04x}{}",
                pte >> 56,
                b.map_or(String::new(), |(lo, hi)| format!(
                    " IN SET B [{lo:#x}, {hi:#x}): the host's context buffer is mapped there"
                ))
            )
        };
        let Some(put) = put else {
            notes.push(format!("gr postmortem: chid {chid}: USERD unreadable"));
            return notes;
        };
        notes.push(format!(
            "ERROR gr postmortem: chid {chid} (host RC exceptType 13): USERD GPGet {get:?} GPPut {put}, engine objects {:x?}, set B {set_b:x?}",
            ch.engine_objects
        ));
        let entries = ch.gpfifo_entries.max(1);
        let mut objects = [0u32; 8];
        let (mut dst, mut src, mut op) = (Surf::default(), Surf::default(), 0u32);
        let mut blits: Vec<(u32, u32, Surf, Surf, u32, bool)> = Vec::new();
        // The other 2D methods (solid fills 0x580-0x6ff, pixels-from-CPU 0x800-0x8ff, ...),
        // the last ones with their data, to name the operation that faulted.
        let mut other: Vec<(u32, u32, u32)> = Vec::new();
        for k in (1..=ENTRIES.min(entries)).rev() {
            let idx = (put + entries - k) % entries;
            let mut e = [0u8; 8];
            if gm.read_va(root, ch.gpfifo_va + 8 * idx as u64, &mut e).is_err() {
                continue;
            }
            let (e0, e1) = (
                u32::from_le_bytes(e[0..4].try_into().unwrap()),
                u32::from_le_bytes(e[4..8].try_into().unwrap()),
            );
            let length = ((e1 >> 10) & 0x1F_FFFF) as usize;
            let pb_va = ((e1 as u64 & 0xFF) << 32) | (e0 as u64 & !3);
            if length == 0 || length > MAX_DWORDS {
                continue;
            }
            let mut raw = vec![0u8; length * 4];
            if gm.read_va(root, pb_va, &mut raw).is_err() {
                notes.push(format!("gr postmortem: chid {chid} gp {idx}: pushbuffer {pb_va:#x} unreadable"));
                continue;
            }
            let w: Vec<u32> = raw
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect();
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
                    if a == 0 {
                        objects[subch] = d & 0xFFFF;
                        continue;
                    }
                    let assumed = objects[subch] == 0 && subch == 3;
                    if objects[subch] != TWOD && !assumed {
                        continue;
                    }
                    match a {
                        0x200..=0x224 => dst.set(a - 0x200, d),
                        0x230..=0x254 => src.set(a - 0x230, d),
                        0x2AC => op = d,
                        0x8DC => {
                            if blits.len() == 4 {
                                blits.remove(0);
                            }
                            blits.push((idx, subch as u32, dst, src, op, assumed));
                        }
                        a if a >= 0x2B0 => {
                            if other.len() == 24 {
                                other.remove(0);
                            }
                            other.push((idx, a, d));
                        }
                        _ => {}
                    }
                }
            }
        }
        let layout = |l: u32| if l == 1 { "PITCH" } else { "BLOCKLINEAR" };
        if blits.is_empty() {
            notes.push(format!(
                "ERROR gr postmortem: chid {chid}: no 2D blit in the last {ENTRIES} GPFIFO entries; 2D state now: dst {dst:x?} src {src:x?}"
            ));
        }
        // The current destination (and source, if set): its first, middle and last page.
        for (name, s) in [("dst", dst), ("src", src)] {
            if s.va() == 0 {
                continue;
            }
            let bytes = (s.width as u64 * 4).max(s.pitch as u64) * s.height.max(1) as u64;
            let probes: Vec<String> = [0, bytes / 2, bytes.saturating_sub(1)]
                .iter()
                .map(|o| walk(s.va() + (o & !0xFFF)))
                .collect();
            notes.push(format!(
                "ERROR gr postmortem: chid {chid} 2D {name} now: format {:#x} {} block {:#x} pitch {:#x} {}x{}, about {bytes:#x} bytes: {}",
                s.format, layout(s.layout), s.block, s.pitch, s.width, s.height, probes.join(" | ")
            ));
        }
        if !other.is_empty() {
            notes.push(format!(
                "ERROR gr postmortem: chid {chid} last 2D methods (gp, method, data): {}",
                other.iter().map(|(i, a, d)| format!("{i}:{a:#x}={d:#x}")).collect::<Vec<_>>().join(" ")
            ));
        }
        for (idx, subch, d, s, op, assumed) in blits {
            notes.push(format!(
                "ERROR gr postmortem: chid {chid} gp {idx} 2D blit on subch {subch}{} op {op:#x}: dst format {:#x} {} block {:#x} pitch {:#x} {}x{} {}; src format {:#x} {} block {:#x} pitch {:#x} {}x{} {}",
                if assumed { " (class assumed 0x902d)" } else { "" },
                d.format, layout(d.layout), d.block, d.pitch, d.width, d.height, walk(d.va()),
                s.format, layout(s.layout), s.block, s.pitch, s.width, s.height, walk(s.va()),
            ));
        }
        notes
    }

    /// After a host RC of mirrored GR channel `chid` (read only, once per RC): the push the
    /// host fetched last, the GPFIFO entry at USERD GPGet-1, with its last methods by
    /// subchannel and whether it launches work: a compute dispatch (SEND_PCAS_A 0x2b4 or
    /// SEND_SIGNALING_PCAS_B 0x2bc on the compute class) or an MME macro call (0x3800 and up
    /// on another class: how the NVIDIA driver issues its draws, direct or indirect).
    /// Subchannel classes come from the SET_OBJECTs of the channel's last 16 entries. For
    /// the Xid 109 of SotTR (docs/research/gr-preemption-2026-09-29.md, step 0).
    pub fn gr_rc_push_note(
        &self,
        gm: &crate::channel::GpuMem,
        chid: u32,
        xid: u32,
    ) -> Option<String> {
        use crate::fb::Target;
        const SCAN: u32 = 16;
        const MAX_DWORDS: usize = 1 << 16;
        let ch = self.channels.channel(chid)?;
        let userd = ch.userd?;
        let root = self
            .channels
            .vas_key_of(ch)
            .and_then(|k| self.channels.vas_root(k))?;
        let at = |off: u64| match userd {
            Target::Fb(pa) => Target::Fb(pa + off),
            Target::Sys(pa) => Target::Sys(pa + off),
            t => t,
        };
        let (get, put) = (gm.read_u32(at(0x88))?, gm.read_u32(at(0x8C))?);
        let entries = ch.gpfifo_entries.max(1);
        if get >= entries || put >= entries {
            return Some(format!(
                "gr rc push: chid {chid} (exceptType {xid}): USERD GPGet {get} GPPut {put} outside the ring of {entries} [guest memory]"
            ));
        }
        // One GPFIFO entry: pushbuffer VA, dwords, subroutine level, and its methods
        // (subchannel, method, data).
        type Entry = (u64, usize, bool, Vec<(usize, u32, u32)>);
        let entry = |idx: u32| -> Option<Entry> {
            let mut e = [0u8; 8];
            gm.read_va(root, ch.gpfifo_va + 8 * idx as u64, &mut e)
                .ok()?;
            let (e0, e1) = (
                u32::from_le_bytes(e[0..4].try_into().unwrap()),
                u32::from_le_bytes(e[4..8].try_into().unwrap()),
            );
            let length = ((e1 >> 10) & 0x1F_FFFF) as usize;
            let pb_va = ((e1 as u64 & 0xFF) << 32) | (e0 as u64 & !3);
            let sub = e1 & (1 << 9) != 0;
            if length == 0 || length > MAX_DWORDS {
                return Some((pb_va, length, sub, Vec::new()));
            }
            let mut raw = vec![0u8; length * 4];
            gm.read_va(root, pb_va, &mut raw).ok()?;
            let w: Vec<u32> = raw
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect();
            let mut methods = Vec::new();
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
                match sec_op {
                    1 | 3 | 5 => {
                        let n = count.min(w.len() - i);
                        for k in 0..n {
                            let a = if sec_op == 3 || (sec_op == 5 && k > 0) {
                                addr
                            } else {
                                addr + 4 * k as u32
                            };
                            methods.push((subch, a, w[i + k]));
                        }
                        i += n;
                    }
                    4 => methods.push((subch, addr, (hdr >> 16) & 0x1FFF)),
                    _ => {}
                }
            }
            Some((pb_va, length, sub, methods))
        };
        let last = (get + entries - 1) % entries;
        let mut classes = [0u32; 8];
        for k in (1..=SCAN.min(entries)).rev() {
            if let Some((_, _, _, m)) = entry((get + entries - k) % entries) {
                for (s, a, d) in m {
                    if a == 0 {
                        classes[s] = d & 0xFFFF;
                    }
                }
            }
        }
        let Some((pb_va, length, sub, methods)) = entry(last) else {
            return Some(format!(
                "gr rc push: chid {chid} (exceptType {xid}): USERD GPGet {get} GPPut {put}; entry {last} (GPGet-1) unreadable"
            ));
        };
        let compute = self.chip.channel.compute;
        let dispatch = methods
            .iter()
            .any(|(s, a, _)| classes[*s] == compute && (*a == 0x2B4 || *a == 0x2BC));
        let mme = methods
            .iter()
            .any(|(s, a, _)| classes[*s] != compute && *a >= 0x3800);
        let kind = match (dispatch, mme) {
            (true, true) => "compute dispatch and MME macro call (draw)",
            (true, false) => "compute dispatch",
            (false, true) => "MME macro call (the driver's draw path)",
            (false, false) => "no dispatch or MME call",
        };
        let tail: Vec<String> = methods
            .iter()
            .rev()
            .take(6)
            .rev()
            .map(|(s, a, d)| format!("{s}[{:#x}]:{a:#x}={d:#x}", classes[*s]))
            .collect();
        let bound: Vec<String> = classes
            .iter()
            .enumerate()
            .filter(|(_, c)| **c != 0)
            .map(|(s, c)| format!("{s}={c:#x}"))
            .collect();
        Some(format!(
            "gr rc push: chid {chid} (exceptType {xid}): USERD GPGet {get} GPPut {put} ({} not fetched); entry {last} (GPGet-1) pb {pb_va:#x}, {length} dword(s){}, {} method(s): {kind}; last (subch[class]:method=data) {}; subchannel classes seen {} [guest memory]",
            (put + entries - get) % entries,
            if sub { ", subroutine" } else { "" },
            methods.len(),
            tail.join(" "),
            if bound.is_empty() { "none".into() } else { bound.join(" ") }
        ))
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

    /// Re-sync the shadow roots whose held set-B ranges a host TSG free released
    /// (mirror.rs `take_released_roots`), right after the guest's FREE.
    pub fn mirror_resync_released(&mut self, gm: &mut crate::channel::GpuMem) -> Vec<String> {
        let reg = &self.channels;
        let Some(m) = self.mirror.as_mut() else {
            return Vec::new();
        };
        m.take_released_roots()
            .into_iter()
            .flat_map(|r| m.shadow_resync(reg, gm, Some(r)))
            .collect()
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
        let st = host.control(
            NV20_SUBDEVICE_0,
            crate::hostabi::CMD_FB_FLUSH_GPU_CACHE,
            &mut p,
        );
        Some((l.flush_gpu_cache_flags(op), st))
    }

    /// The guest kernel's TLB invalidate for `pdb`: forwarded to the mirror VA spaces.
    /// Forward the held rings that are ready and commit the finished mirror builds
    /// (ringgate.rs, execution.md §9.2; the completer thread).
    pub fn complete_held_rings(&mut self, gm: &mut crate::channel::GpuMem) -> Vec<String> {
        let Some(m) = self.mirror.as_mut() else {
            return Vec::new();
        };
        let mut notes = m.complete_held_rings(&self.channels, gm);
        let committed = Self::after_mirror_builds(m, &self.channels, &mut self.dstate, gm, &mut notes);
        for (c, h) in committed {
            notes.extend(self.pin_pool_token(gm, c, h));
        }
        notes
    }

    /// Wait for and commit the mirror builds in flight a guest free may concern (boot.rs,
    /// before [`Self::free`]; mirror.rs `settle_builds_for_free`).
    pub fn settle_builds_for_free(
        &mut self,
        gm: &mut crate::channel::GpuMem,
        h_client: u32,
        h_object: u32,
    ) -> Vec<String> {
        let Some(m) = self.mirror.as_mut() else {
            return Vec::new();
        };
        let mut notes = m.settle_builds_for_free(&self.channels, gm, h_client, h_object);
        let committed = Self::after_mirror_builds(m, &self.channels, &mut self.dstate, gm, &mut notes);
        for (c, h) in committed {
            notes.extend(self.pin_pool_token(gm, c, h));
        }
        notes
    }

    /// After shadow doorbells and mirror build commits: the cached walks of shadow VA spaces
    /// handed to the teardown go, a build the host refused for memory RCs its channel once
    /// (rc.rs), and the channels committed are returned with their host tokens (for
    /// [`Self::pin_pool_token`]).
    fn after_mirror_builds(
        m: &mut crate::mirror::Mirror,
        reg: &crate::channel::Registry,
        dstate: &mut crate::dmodel::DState,
        gm: &mut crate::channel::GpuMem,
        notes: &mut Vec<String>,
    ) -> Vec<(u32, u32)> {
        for r in m.take_reaped_roots() {
            gm.fb.unwatch_tables(crate::fb::Target::Fb(r));
        }
        for c in m.take_exhausted() {
            let engine = reg.channel(c).map_or(0, |ch| ch.engine_type);
            dstate.rc.host_refused(c, engine);
            notes.push(format!(
                "shadow: chid {c} refused by the host for memory: RC queued (PREEMPTIVE_REMOVAL) [host protection]"
            ));
        }
        m.take_committed()
    }

    /// The mirror's ring gate, when there is a mirror.
    pub fn ring_gate(&self) -> Option<std::sync::Arc<crate::ringgate::RingGate>> {
        self.mirror.as_ref().map(|m| m.ring_gate())
    }

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
        if self.channels.engine_object(h_client, h_object).is_some()
            || self.mirror.as_ref().is_some_and(|m| m.engine_handle_in_use(&self.channels, h_client, h_object))
        {
            return Answer {
                status: nvrm_sys::NV_ERR_INVALID_ARGUMENT,
                params: params.to_vec(),
                label: "refused (live engine handle)",
                note: format!("alloc {h_class:#x}: engine {h_client:#x}/{h_object:#x} is still live"),
            };
        }
        // Profiler classes are refused (dmodel.rs, d-device-model.md 6.16); the D models note
        // the allocation (a reused handle, a channel's instance block).
        if let Some(a) = self.dstate.alloc(h_client, h_object, h_class, params) {
            return a;
        }
        // The feature families' objects (the MIG instance references, families/mig.rs).
        if let Some(a) = self
            .families
            .alloc(h_client, h_object, h_parent, h_class, params)
        {
            self.objects.insert((h_client, h_object), h_class);
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
                    note: format!(
                        "alloc {h_class:#x} object {h_object:#x} under {h_parent:#x}: {why}"
                    ),
                };
            }
        }
        if let Some(mut note) = self
            .channels
            .record_alloc(h_client, h_parent, h_object, h_class, params)
        {
            // An engine object on a channel that already runs on the host: add it there.
            if let Some(m) = self.mirror.as_mut() {
                for line in m.retire_replaced_channels(&self.channels) {
                    note.push_str("; ");
                    note.push_str(&line);
                }
                let chid = self
                    .channels
                    .channels()
                    .find(|c| c.h_client == h_client && c.h_object == h_parent)
                    .map(|c| c.chid);
                if let Some(n) = chid.and_then(|c| m.add_engine_object(c, h_object, h_class, params)) {
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
        let mut note =
            format!("alloc {name} {h_class:#x} object {h_object:#x} under {h_parent:#x}");
        // cl0070.h: NV_MEMORY_VIRTUAL_ALLOCATION_PARAMS is {u64 offset, u64
        // limit (IN/OUT), NvHandle hVASpace}, padded to 24 bytes in both ABIs.
        // Log the existing echo precisely; this diagnostic does not implement or
        // change the allocation's zero-limit/default-VAS semantics.
        if h_class == 0x70 {
            if params.len() >= 20 {
                let offset = get64(params, 0);
                let limit = get64(params, 8);
                let vas = get32(params, 16);
                note.push_str(&format!(
                    "; NV01_MEMORY_VIRTUAL {} bytes: input offset {offset:#x} limit {limit:#x} hVASpace {vas:#x}; output offset {offset:#x} limit {limit:#x} hVASpace {vas:#x} (unchanged echo)",
                    params.len()
                ));
            } else {
                note.push_str(&format!(
                    "; NV01_MEMORY_VIRTUAL {} bytes: truncated fields; unchanged echo {}",
                    params.len(),
                    params
                        .iter()
                        .map(|x| format!("{x:02x}"))
                        .collect::<String>()
                ));
            }
        }
        Answer::ok(
            params.to_vec(),
            "synthesized (object recorded, no host object yet)",
            note,
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
        self.families.dup(dst, src);
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
        let registry_notes = self.channels.record_free(h_client, h_object);
        self.deferred_context.prune(&self.channels);
        self.guest_attrs.free(h_client, h_object);
        self.dstate.rc.forget(h_client, h_object);
        self.families.free(h_client, h_object);
        if h_object == h_client {
            self.objects.retain(|(c, _), _| *c != h_client);
        }
        Answer::ok(
            Vec::new(),
            "synthesized",
            format!(
                "free {h_object:#x} class {class:?}{}{}",
                if mirror_notes.is_empty() {
                    String::new()
                } else {
                    format!("; mirror: {}", mirror_notes.join("; "))
                },
                if registry_notes.is_empty() {
                    String::new()
                } else {
                    format!("; registry: {}", registry_notes.join("; "))
                }
            ),
        )
    }

    /// `GSP_RM_CONTROL`. An answer the device has no model for (`unanswered`, or a `no
    /// host` fallback in pure emulation) is filled from the card's recorded answers or chip
    /// facts where they fit (recorded.rs); what stays NOT_SUPPORTED carries the first
    /// request bytes in its note, so a trace shows what the guest asked.
    pub fn control(&mut self, h_client: u32, h_object: u32, cmd: u32, params: &[u8]) -> Answer {
        let mut answer = self.control_inner(h_client, h_object, cmd, params);
        // Compression per VM slice: the forwarded and replayed FB info is this VM's.
        if let Some(view) = self.compression() {
            crate::compression::finish(&view, cmd, &mut answer);
        }
        const DEFERRED: [u32; 4] = [0x5080_0101, 0x5080_0102, 0x5080_0103, 0x5080_0104];
        static CAPTURED: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        if DEFERRED.contains(&cmd)
            && std::env::var("CARAXES_DEFERRED_API_CAPTURE").as_deref() == Ok("1")
            && CAPTURED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 8
        {
            let word = |off: usize| {
                params
                    .get(off..off + 4)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            };
            let fields = if cmd == 0x5080_0102 {
                format!("hApiHandle {:?}", word(0).map(|v| format!("{v:#x}")))
            } else {
                format!(
                    "hApiHandle {:?}, nested cmd {:?}, flags {:?}, hClientVA {:?}, hDeviceVA {:?}",
                    word(0).map(|v| format!("{v:#x}")),
                    word(4).map(|v| format!("{v:#x}")),
                    word(8).map(|v| format!("{v:#x}")),
                    word(12).map(|v| format!("{v:#x}")),
                    word(16).map(|v| format!("{v:#x}")),
                )
            };
            let kept = params.len().min(1024);
            let hex: String = params[..kept]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            answer.note.push_str(&format!(
                "; deferred API capture: client {h_client:#x}, object {h_object:#x}, cmd {cmd:#x}, input {} byte(s), {fields}, return status {:#x}, bytes[0..{kept}] {hex}{}",
                params.len(),
                answer.status,
                if kept < params.len() { " (truncated)" } else { "" }
            ));
        }
        const GR_CTXSW_PREEMPTION_BIND: u32 = CMD_GR_CTXSW_PREEMPTION_BIND;
        static SHADOW_CHANNEL_CAPTURED: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        if cmd == GR_CTXSW_PREEMPTION_BIND
            && std::env::var("CARAXES_SHADOW_CHANNEL_DIAG").as_deref() == Ok("1")
            && SHADOW_CHANNEL_CAPTURED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 8
        {
            let word = |off: usize| {
                params
                    .get(off..off + 4)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            };
            let kept = params.len().min(112);
            let hex: String = params[..kept]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            answer.note.push_str(&format!(
                "; shadow channel capture: ctrl20801211 input {} byte(s), flags {:?}, hClient {:?}, hChannel {:?}, gfxp {:?}, cilp {:?}, bytes[0..{kept}] {hex}{}",
                params.len(),
                word(0).map(|v| format!("{v:#x}")),
                word(4).map(|v| format!("{v:#x}")),
                word(8).map(|v| format!("{v:#x}")),
                word(88).map(|v| format!("{v:#x}")),
                word(92).map(|v| format!("{v:#x}")),
                if kept < params.len() { " (truncated)" } else { "" }
            ));
            if self.deferred_context.enabled {
                if let (Some(mirror), Some(client), Some(target)) =
                    (self.mirror.as_ref(), word(4), word(8))
                {
                    let note = mirror.deferred_context_modes(&self.channels, client, target)
                        .unwrap_or_else(|e| format!("host context modes unavailable: {e}"));
                    answer.note.push_str(&format!("; {note}"));
                }
            }
        }
        answer
    }

    /// `GR_CTXSW_PREEMPTION_BIND` (0x20801211, the Windows KMD's: it sends BINDs, no SET;
    /// ctrl2080gr.h:917-929) on a mirrored device. Its compute part takes the SET's path
    /// (cuda.rs `set_ctxsw_preemption_mode`, preempt.rs): recorded for the guest TSG, bound on
    /// the host TSG at its build or at once when it is built. The graphics part never forwards
    /// the guest's buffers: a host TSG with a 3D object has GFXP from a slot of the VM's slice,
    /// bound at its build (gfxpslot.rs), any other stays WFI; it is acknowledged.
    fn preemption_bind(&mut self, params: &[u8]) -> Answer {
        let word = |o: usize| {
            params
                .get(o..o + 4)
                .map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()))
        };
        let (flags, client, tsg, gfxp, cilp) = (word(0), word(4), word(8), word(88), word(92));
        let pointer_free = params.len() == 112
            && params[12..88].iter().all(|b| *b == 0)
            && params[96..112].iter().all(|b| *b == 0);
        if flags == 1 && pointer_free {
            // As the SET {CILP_SET, hChannel = the TSG, cilp} of the same client.
            use crate::preempt::{FLAG_CILP_SET, SET_CILP, SET_FLAGS, SET_H_CHANNEL};
            let mut set = [0u8; 32];
            set[SET_FLAGS..SET_FLAGS + 4].copy_from_slice(&FLAG_CILP_SET.to_le_bytes());
            set[SET_H_CHANNEL..SET_H_CHANNEL + 4].copy_from_slice(&tsg.to_le_bytes());
            set[SET_CILP..SET_CILP + 4].copy_from_slice(&cilp.to_le_bytes());
            let (mut a, guest) =
                crate::cuda::set_ctxsw_preemption_mode(client, &set, &mut self.channels);
            if let (Some(g), Some(m)) = (guest, self.mirror.as_mut()) {
                if let Some(n) = m.guest_preemption_set(&self.channels, g) {
                    a.note.push_str(&format!("; {n}"));
                }
            }
            a.params = params.to_vec();
            a.note = format!("preemption bind (compute, as a SET): {}", a.note);
            return a;
        }
        Answer {
            status: NV_OK,
            params: params.to_vec(),
            label: "guest (acknowledged: a 3D host TSG has GFXP from a slot of the VM's slice, bound at its build; others WFI; the guest's buffers are never used)",
            note: format!("preemption bind {client:#x}/{tsg:#x} flags {flags:#x} gfxp {gfxp} cilp {cilp}: acknowledged"),
        }
    }

    fn control_inner(&mut self, h_client: u32, h_object: u32, cmd: u32, params: &[u8]) -> Answer {
        // The Windows KMD's graphics-preemption pool (GR_GFX_POOL_QUERY_SIZE and the three
        // pool controls) gets recorded.rs's answer: the pool is the KMD's own heap memory,
        // part of the VM's slice; the host context's GFXP comes from a slot bound at its
        // build (gfxpslot.rs). Refusing it unloads the KMD (P39: Code 43).
        if cmd == CMD_GR_CTXSW_PREEMPTION_BIND && self.mirror.is_some() {
            return self.preemption_bind(params);
        }
        if matches!(cmd, 0x5080_0101..=0x5080_0104)
            && self.deferred_context.enabled {
            return self.register_deferred(h_client, h_object, cmd, params);
        }
        // This control used to be a Windows-only recorded acknowledgement. It changes the
        // device's live VA-space association, so model it before every recorded override.
        if cmd == CMD_DMA_SET_DEFAULT_VASPACE {
            return self.set_default_vaspace(h_client, h_object, params);
        }
        // Compression per VM slice: the compbit-store controls are this VM's, ahead of the
        // chip table (D16 refusals) and of the recorded CARAXES_WIN_COMPBIT answer, which
        // carries the host's store addresses (compression.rs).
        if let Some(view) = self.compression() {
            if let Some(a) = crate::compression::answer(&view, cmd, params) {
                return a;
            }
        }
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
        let host_refused =
            a.status == NV_ERR_INSUFFICIENT_PERMISSIONS && a.label.starts_with("host");
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
                let head: String = params.iter().take(48).map(|b| format!("{b:02x}")).collect();
                Answer::ok(
                    params.to_vec(),
                    "diagnostic (CARAXES_ACK_UNANSWERED: NV_OK echo)",
                    format!(
                        "{} -> acknowledged (was status {:#x} [{}]); request {head}",
                        a.note, a.status, a.label
                    ),
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

    fn set_default_vaspace(&mut self, h_client: u32, h_device: u32, params: &[u8]) -> Answer {
        let invalid = |status, why: String| Answer {
            status,
            params: params.to_vec(),
            label: "guest (validated default VAS association)",
            note: why,
        };
        if params.len() != 4 {
            return invalid(
                v::NV_ERR_INVALID_ARGUMENT,
                format!(
                    "set default vaspace on device {h_device:#x}: expected 4-byte hVASpace, got {} bytes",
                    params.len()
                ),
            );
        }
        if self.objects.get(&(h_client, h_device)) != Some(&NV01_DEVICE_0) {
            return invalid(
                NV_ERR_INVALID_OBJECT_HANDLE,
                format!("set default vaspace: unknown device {h_client:#x}/{h_device:#x}"),
            );
        }
        let h_vas = get32(params, 0);
        if self.objects.get(&(h_client, h_vas)) != Some(&v::FERMI_VASPACE_A) {
            return invalid(
                NV_ERR_INVALID_OBJECT_HANDLE,
                format!(
                    "set default vaspace on {h_client:#x}/{h_device:#x}: {h_vas:#x} is not this client's FERMI_VASPACE_A"
                ),
            );
        }
        match self
            .channels
            .record_set_default_vaspace(h_client, h_device, h_vas)
        {
            Ok(note) => Answer::ok(
                params.to_vec(),
                "guest (device default VAS association recorded)",
                note,
            ),
            // deviceSetDefaultVASpace returns INVALID_OBJECT_HANDLE when the device already
            // owns a default VAS, despite the public header listing INVALID_STATE.
            Err(why) => invalid(NV_ERR_INVALID_OBJECT_HANDLE, why),
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

    fn control_modelled(
        &mut self,
        h_client: u32,
        h_object: u32,
        cmd: u32,
        params: &[u8],
    ) -> Answer {
        // STOP_CHANNEL: once it returns, the guest kernel writes PREEMPTIVE_REMOVAL into the
        // channel's error notifier itself (kchannelNotifyRc, kernel_channel.c:1903). The RC
        // scan (boot.rs `scan_channel_errors`) must not report that as a host RC.
        if cmd == v::CMD_CHANNEL_STOP {
            let chid = self
                .channels
                .channels()
                .find(|c| c.h_client == h_client && c.h_object == h_object)
                .map(|c| c.chid);
            if let Some(c) = chid.and_then(|chid| self.channels.channel_mut(chid)) {
                c.stopped = true;
            }
        }
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
        if let Some(mut a) = kgr {
            // A chip with the MIG model reserves the GR global buffers per GPU instance
            // (families/mig.rs, `global_ctx_buffers`).
            crate::families::mig::global_ctx_buffers(&self.families, cmd, &mut a);
            return a;
        }
        // The guest's GR preemption request (cuda.rs, preempt.rs): NV_OK as before, recorded
        // per guest TSG; its compute mode is bound on the host TSG at the build, or now when
        // that TSG is built already.
        if cmd == v::CMD_GR_SET_CTXSW_PREEMPTION_MODE {
            let (mut a, guest) =
                crate::cuda::set_ctxsw_preemption_mode(h_client, params, &mut self.channels);
            if let (Some(g), Some(m)) = (guest, self.mirror.as_mut()) {
                if let Some(n) = m.guest_preemption_set(&self.channels, g) {
                    a.note.push_str(&format!("; {n}"));
                }
            }
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
        // The feature families the chip models (families/): ahead of the host forwards of
        // public.rs and nvml.rs, whose host card has neither ECC nor MIG, and of the
        // refusals of chipctl.rs.
        if let Some(a) = self.families.control(h_client, h_object, cmd, params) {
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
        if let Some(a) = crate::public::answer_for_layout(&mut pctx, cmd, params, self.fb_layout.as_ref()) {
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
        // Real compression (zbc.rs): the guest kernel's ZBC-surface flag, which the
        // chip-data rules acknowledge otherwise.
        if let Some(a) = self.dstate.zbc_referenced(cmd, params) {
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
            mirror: dmirror.as_mut().map(|m| m as &mut dyn crate::rc::MirrorOps),
            host: self.host.as_deref_mut(),
            bar0: (
                self.bar_offsets[0],
                self.bar_sizes.first().copied().unwrap_or(0),
            ),
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
            CMD_INTERNAL_GPU_GET_CHIP_INFO => self.id_0a36(crate::ver::layouts().ctrl_0a36, params),
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
                // The layout is the guest ABI's (`engineData` has 17 words from 615 on).
                let table = &self.chip.device_info;
                let l = &crate::ver::layouts().devinfo;
                let mut b = vec![0u8; n.max(l.params_size)];
                put32(&mut b, l.num_entries, table.engines.len() as u32);
                b[l.b_more] = 0;
                for (i, e) in table.engines.iter().enumerate() {
                    let o = l.entries + l.entry_size * i;
                    let data = |idx: usize| o + l.engine_data + 4 * idx;
                    put32(&mut b, data(v::ENGINE_INFO_ENG_DESC), e.eng_desc);
                    put32(&mut b, data(v::ENGINE_INFO_RM_ENGINE_TYPE), e.rm_engine_type);
                    put32(&mut b, data(v::ENGINE_INFO_RUNLIST), e.runlist);
                    put32(&mut b, data(v::ENGINE_INFO_MMU_FAULT_ID), e.mmu_fault_id);
                    put32(&mut b, data(v::ENGINE_INFO_MC), e.mc_idx);
                    put32(&mut b, data(v::ENGINE_INFO_DEV_TYPE_ENUM), e.dev_type);
                    put32(&mut b, data(v::ENGINE_INFO_INSTANCE_ID), e.instance);
                    put32(&mut b, data(v::ENGINE_INFO_IS_HOST_DRIVEN_ENGINE), e.host_driven);
                    for (k, [id, fault]) in e.pbdma.iter().enumerate() {
                        put32(&mut b, o + l.pbdma_ids + 4 * k, *id);
                        put32(&mut b, o + l.pbdma_fault_ids + 4 * k, *fault);
                    }
                    put32(&mut b, o + l.num_pbdmas, e.pbdma.len() as u32);
                    let name = o + l.engine_name;
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
                if crate::ver::guest() == crate::ver::GuestAbi::V570 {
                    // 570.144: one (subtreeStart, subtreeEnd) byte pair per category at
                    // 2052, 0xFF for none (ver::v570::INTR_SUBTREE_MAP).
                    use crate::ver::v570 as g;
                    b.resize(b.len().max(g::INTR_TABLE_SIZE), 0);
                    for c in 0..7 {
                        let o = g::INTR_SUBTREE_MAP + g::INTR_SUBTREE_MAP_ENTRY * c;
                        b[o..o + 2].copy_from_slice(&[g::INTR_INVALID_SUBTREE; 2]);
                    }
                    for &(cat, mask) in &t.subtrees {
                        let (start, end) = g::subtree_range(mask);
                        let o = g::INTR_SUBTREE_MAP + g::INTR_SUBTREE_MAP_ENTRY * cat;
                        b[o..o + 2].copy_from_slice(&[start, end]);
                    }
                } else {
                    for &(cat, mask) in &t.subtrees {
                        put64(&mut b, v::INTR_SUBTREE_MAP + v::INTR_SUBTREE_MAP_ENTRY * cat, mask);
                    }
                }
                b.truncate(n);
                Answer::ok(b, &t.label, format!("interrupt kernel table: {} entries", t.entries.len()))
            }
            CMD_INTERNAL_MEMSYS_GET_STATIC_CONFIG => {
                // ctrl2080internal.h:94-135; the values are the chip's.
                let m = &self.chip.memsys;
                let compression = self.compression();
                let mut b = vec![0u8; n.max(v::MEMSYS_SIZE)];
                b[v::MEMSYS_DISABLE_COMPBIT_BACKING] = m.disable_compbit_backing as u8;
                // Compression per VM slice: the card's 1:1 comptag lines and the backing on
                // (compression.rs); without it the chip's D16 answer above.
                if let Some(view) = &compression {
                    crate::compression::memsys_static_config(&mut b, view);
                }
                // ECC in force (families/ecc.rs): what FB_INFO_FBPA_ECC_ENABLED reports
                // (kern_mem_sys_ctrl.c:771-776 of 610.57.04).
                let ecc = crate::families::off(
                    "NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS",
                    "bEnabledEccFBPA",
                );
                b[ecc] = self.families.ecc_enabled() as u8;
                put64(&mut b, v::MEMSYS_L2_CACHE_SIZE, m.l2_cache_size);
                b[v::MEMSYS_FBPA_PRESENT] = m.fbpa_present as u8;
                put32(&mut b, v::MEMSYS_COMPR_PAGE_SIZE, m.compr_page_size);
                put32(&mut b, v::MEMSYS_COMPR_PAGE_SHIFT, m.compr_page_shift);
                put32(&mut b, v::MEMSYS_RAM_TYPE, m.ram_type);
                put32(&mut b, v::MEMSYS_LTC_COUNT, m.ltc_count);
                put32(&mut b, v::MEMSYS_LTS_PER_LTC_COUNT, m.lts_per_ltc_count);
                b.truncate(n);
                let note = match compression {
                    Some(view) => format!(
                        "memsys static config; {} [{}]",
                        view.describe(),
                        view.facts.label
                    ),
                    None => "memsys static config".into(),
                };
                Answer::ok(b, &m.label, note)
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
                // CARAXES_GSP_FW_VERSION: the firmware version string instead of the served
                // guest ABI's driver (a 617.14 Windows guest against the 615-based model).
                let env_ver = std::env::var("CARAXES_GSP_FW_VERSION").ok().filter(|s| s.len() < 64);
                let ver = env_ver.as_deref().unwrap_or(crate::ver::layouts().driver).as_bytes();
                // gspFeatures bit 0 is UVM_ENABLED (ctrl2080gsp.h). A Linux guest's nvidia-uvm
                // would need replayable faults, which never reach the guest (the host RM owns
                // them): libcuda would map through UVM and the first GPU touch of a managed page
                // kills the context with a host Xid 31 (docs/research/uvm-faults-2026-09-28.md).
                // So a Unix guest is told the GPU has no UVM (cudaMallocManaged returns 801, as on
                // NVIDIA vGPU without enable_uvm). A Windows guest keeps 1 as the card answers
                // (kmd-controls-2026-09-26); WDDM managed memory needs no replayable faults.
                let uvm = self.guest_os_type == Some(crate::gmcapi::HOST_OS_TYPE_WINDOWS);
                if n >= v::GSP_FEATURES_SIZE {
                    put32(&mut b, 0, u32::from(uvm));
                    b[v::GSP_FEATURES_VALID] = 1;
                    b[v::GSP_FEATURES_DEFAULT_GSP_RM_GPU] = 1;
                    let at = v::GSP_FEATURES_FIRMWARE_VERSION;
                    b[at..at + ver.len()].copy_from_slice(ver);
                }
                Answer::ok(b, "synthesized (driver version string; the emulated GSP is that version)", format!("gsp features: {} (UVM {} for guest os {:?}), valid, {}", u32::from(uvm), if uvm { "on" } else { "off" }, self.guest_os_type, String::from_utf8_lossy(ver)))
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
                let l = crate::ver::layouts();
                if n > l.p2p_write_caps_status {
                    b[l.p2p_read_caps_status] = v::P2P_CAPS_STATUS_NOT_SUPPORTED;
                    b[l.p2p_write_caps_status] = v::P2P_CAPS_STATUS_NOT_SUPPORTED;
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
    /// guest ABI through `ver::layouts().ctrl_0a36`).
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

    #[test]
    fn virtual_memory_allocation_logging_preserves_the_reply_and_full_addresses() {
        use std::mem::{offset_of, size_of};
        macro_rules! layout {
            ($ty:ty) => {
                assert_eq!(size_of::<$ty>(), 24);
                assert_eq!(offset_of!($ty, offset), 0);
                assert_eq!(offset_of!($ty, limit), 8);
                assert_eq!(offset_of!($ty, hVASpace), 16);
            };
        }
        layout!(nvrm_sys::v610::NV_MEMORY_VIRTUAL_ALLOCATION_PARAMS);
        layout!(nvrm_sys::v615::NV_MEMORY_VIRTUAL_ALLOCATION_PARAMS);
        let mut d = dispatcher(None);
        let mut p = vec![0u8; 24];
        put64(&mut p, 0, 0x1_2345_6000);
        put64(&mut p, 8, 0x1_FFFF_FFFF);
        put32(&mut p, 16, 0xFFFF_FFFF);
        p[20..].fill(0xA5);
        let a = d.alloc(1, 0x70, 0x80, 0x70, &p);
        assert_eq!((a.status, a.params), (NV_OK, p));
        assert_eq!(d.objects.get(&(1, 0x70)), Some(&0x70));
        assert!(a
            .note
            .contains("input offset 0x123456000 limit 0x1ffffffff hVASpace 0xffffffff"));
        assert!(a.note.contains(
            "output offset 0x123456000 limit 0x1ffffffff hVASpace 0xffffffff (unchanged echo)"
        ));
        // An undecodable old request keeps the same success/echo behavior too.
        let a = d.alloc(1, 0x71, 0x80, 0x70, &[0x12, 0x34]);
        assert_eq!((a.status, a.params), (NV_OK, vec![0x12, 0x34]));
        assert!(a.note.contains("truncated fields; unchanged echo 1234"));
    }

    #[test]
    fn set_default_vaspace_validates_and_records_the_device_association() {
        let mut d = dispatcher(None);
        d.alloc(1, 1, 0, NV01_ROOT, &[]);
        d.alloc(1, 0x80, 1, NV01_DEVICE_0, &[0u8; 56]);
        d.alloc(1, 0x21, 0x80, v::FERMI_VASPACE_A, &[0u8; 56]);

        let p = 0x21u32.to_le_bytes();
        let a = d.control(1, 0x80, CMD_DMA_SET_DEFAULT_VASPACE, &p);
        assert_eq!(a.status, NV_OK, "{}", a.note);
        assert!(
            a.note.contains("device 0x80 default vaspace 0x21"),
            "{}",
            a.note
        );

        let a = d.control(1, 0x80, CMD_DMA_SET_DEFAULT_VASPACE, &p);
        assert_eq!(a.status, NV_ERR_INVALID_OBJECT_HANDLE, "{}", a.note);
        let a = d.control(1, 0x80, CMD_DMA_SET_DEFAULT_VASPACE, &[0u8; 8]);
        assert_eq!(a.status, v::NV_ERR_INVALID_ARGUMENT, "{}", a.note);
        let a = d.control(1, 0x81, CMD_DMA_SET_DEFAULT_VASPACE, &p);
        assert_eq!(a.status, NV_ERR_INVALID_OBJECT_HANDLE, "{}", a.note);
        let a = d.control(1, 0x80, CMD_DMA_SET_DEFAULT_VASPACE, &0x22u32.to_le_bytes());
        assert_eq!(a.status, NV_ERR_INVALID_OBJECT_HANDLE, "{}", a.note);
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
        assert!(
            fab.label.contains("SYNC_FABRIC_PROBE_INFO"),
            "{}",
            fab.label
        );
        assert_eq!(
            std::mem::size_of::<
                nvrm_sys::v615::NV2080_CTRL_CMD_INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO_PARAMS,
            >(),
            16
        );
        // Through the dispatcher, the guest ABI in force decides, in one build.
        use crate::ver::{GuestAbi, TestGuest};
        for (g, want) in [
            (GuestAbi::V570, Id0A36::GetChipInfo),
            (GuestAbi::V610, Id0A36::GetChipInfo),
            (GuestAbi::V615, Id0A36::SyncFabricProbeInfo),
        ] {
            let _g = TestGuest::set(g);
            let mut d = dispatcher(None);
            let a = d.control(INTERNAL_CLIENT, 0xCA7A_0003, 0x2080_0A36, &p);
            assert_eq!(crate::ver::layouts().ctrl_0a36, want);
            assert_eq!(
                a.note == "chip info",
                want == Id0A36::GetChipInfo,
                "{g:?}: {}",
                a.note
            );
        }
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

    /// A 570.144 guest reads the kernel interrupt table's subtree map as byte ranges at
    /// 2052 (ver::v570); the entries are where 610 has them.
    #[test]
    fn the_570_kernel_table_has_subtree_ranges() {
        use crate::ver::{v570, GuestAbi, TestGuest};
        let _g = TestGuest::set(GuestAbi::V570);
        let mut d = dispatcher(None);
        let a = d.control(
            INTERNAL_CLIENT,
            0xCA7A_0003,
            CMD_INTERNAL_INTR_GET_KERNEL_TABLE,
            &[0u8; v570::INTR_TABLE_SIZE],
        );
        assert_eq!((a.status, a.params.len()), (NV_OK, 2068));
        assert_eq!(u32::from_le_bytes(a.params[0..4].try_into().unwrap()), 15);
        let range = |cat: usize| {
            let o = v570::INTR_SUBTREE_MAP + 2 * cat;
            (a.params[o], a.params[o + 1])
        };
        // DEFAULT, RUNLIST and RUNLIST_NOTIFICATION have no subtree on TU106;
        // ESCHED_DRIVEN_ENGINE 3, its notification 0, UVM_OWNED 1, UVM_SHARED 2.
        assert_eq!(
            (0..7).map(range).collect::<Vec<_>>(),
            vec![(0xFF, 0xFF), (3, 3), (0, 0), (0xFF, 0xFF), (0xFF, 0xFF), (1, 1), (2, 2)]
        );
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
        // plus the display vector (display-ab.md), NVDEC0/NVENC0 (video-engines.md) and
        // NVDEC1/NVDEC2 (one vector per device-info runlist, open-render-2026-09-29.md).
        assert_eq!(u32::from_le_bytes(a.params[0..4].try_into().unwrap()), 15);
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
        assert_eq!(&a.params[6..15], crate::ver::layouts().driver.as_bytes());
        // UVM_ENABLED (gspFeatures bit 0): off before GSP_INIT and for a Unix guest (no
        // replayable faults reach it), on for a Windows guest (as the card answers).
        assert_eq!(get32(&a.params, 0), 0);
        for (os, want) in [
            (crate::gmcapi::HOST_OS_TYPE_UNIX, 0),
            (crate::gmcapi::HOST_OS_TYPE_WINDOWS, 1),
            (0, 0),
        ] {
            d.guest_os_type = Some(os);
            let a = d.control(
                INTERNAL_CLIENT,
                0xCA7A_0003,
                CMD_GSP_GET_FEATURES,
                &[0u8; 72],
            );
            assert_eq!(get32(&a.params, 0), want, "guest os type {os}");
            assert_eq!(a.params[v::GSP_FEATURES_VALID], 1);
        }
        d.guest_os_type = None;
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
        assert_eq!(
            dispatcher(None).host_flush_gpu_cache(L2Flush::WriteBack),
            None
        );
        let seen: Seen = Default::default();
        let mut d = dispatcher(Some(Box::new(Rec(seen.clone()))));
        let l = current();
        let (wb, inv) = (
            l.flush_gpu_cache_flags(L2Flush::WriteBack),
            l.flush_gpu_cache_flags(L2Flush::Invalidate),
        );
        assert_eq!(
            d.host_flush_gpu_cache(L2Flush::WriteBack),
            Some((wb, NV_OK))
        );
        assert_eq!(
            d.host_flush_gpu_cache(L2Flush::Invalidate),
            Some((inv, NV_OK))
        );
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &[
                (NV20_SUBDEVICE_0, 0x2080_130E, 4024, wb),
                (NV20_SUBDEVICE_0, 0x2080_130E, 4024, inv)
            ]
        );
    }
}

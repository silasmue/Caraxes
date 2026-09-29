// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The vfio-user device: region and interrupt tables, and the `ServerBackend`
//! that routes region accesses to the config space, BAR0 and the ROM. DMA
//! mappings are mapped as guest RAM ([`GuestMemory`]) and, from E5b on, registered
//! with the host RM, which gives the guest DMA table ([`GuestRam`],
//! docs/caraxes/memory-model.md §2). Interrupt eventfds are kept for MSI.
//!
//! With `CARAXES_DOORBELL_IOEVENTFD=1` the usermode doorbell register is offered to the
//! client as an ioeventfd sub-region (`VFIO_USER_DEVICE_GET_REGION_IO_FDS`,
//! patches/README.md, patch 0005): guest writes there signal an eventfd instead of
//! arriving as `VFIO_USER_REGION_WRITE`, and a reader thread rings the device.
//! With `CARAXES_DOORBELL_TOKENS=N` the register is offered as N sub-regions with KVM
//! datamatch instead, one per pool token and eventfd (docs/caraxes/doorbell-token-pool.md),
//! and one epoll loop rings the channel that holds the token signalled.

use std::fs::File;
use std::mem::size_of;
use std::os::fd::{FromRawFd, OwnedFd};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use vfio_bindings::bindings::vfio::{
    vfio_region_info, VFIO_IRQ_INFO_EVENTFD, VFIO_IRQ_INFO_NORESIZE, VFIO_IRQ_SET_ACTION_TRIGGER,
    VFIO_IRQ_SET_DATA_EVENTFD, VFIO_IRQ_SET_DATA_NONE, VFIO_PCI_BAR0_REGION_INDEX, VFIO_PCI_CONFIG_REGION_INDEX,
    VFIO_PCI_INTX_IRQ_INDEX, VFIO_PCI_MSI_IRQ_INDEX, VFIO_PCI_NUM_IRQS, VFIO_PCI_NUM_REGIONS,
    VFIO_PCI_ROM_REGION_INDEX, VFIO_REGION_INFO_FLAG_MMAP, VFIO_REGION_INFO_FLAG_READ,
    VFIO_REGION_INFO_FLAG_WRITE,
};
use vfio_user::{
    DmaMapFlags, DmaUnmapFlags, IoEventFdSubRegion, IrqInfo, ServerBackend, ServerRegion,
    SparseArea,
};

use crate::bar0::{Bar0, WriteEffect};
use crate::board::Board;
use crate::config::{self, ConfigSpace};
use crate::dmamap::{DmaRange, DmaTable};
use crate::guestmem::GuestMemory;
use crate::hostmem::HostMemory;
use crate::intr::IntrShared;
use crate::registrar::{Piece, Registrar};
use crate::trace::{Dir, Trace};

/// What happens to guest RAM the client offers with DMA_MAP.
pub enum GuestRam {
    /// Mapped for the device's own accesses; DMA addresses are guest physical (E0-E5a).
    Plain,
    /// As `Plain`, plus an identity DMA table (IOVA = GPA) published for the guest
    /// module: exercises the table path without host registration.
    Identity,
    /// Registered with the host RM in 1 GiB chunks; the IOVAs form the table.
    Register(Arc<Mutex<HostMemory>>),
}

/// A guest RAM range the client offered for DMA.
#[derive(Debug)]
pub struct DmaMapping {
    pub address: u64,
    pub size: u64,
    pub offset: u64,
    pub fd: Option<File>,
}

pub struct Device {
    cfg: ConfigSpace,
    /// The configuration space as the device came up, for a new client ([`Device::client_gone`]).
    cfg_reset: ConfigSpace,
    bar0: Bar0,
    trace: Trace,
    dma: Vec<DmaMapping>,
    mem: GuestMemory,
    irq_fds: Vec<Vec<File>>,
    guest_ram: GuestRam,
    /// Fixed for this session (with host execution, [`Device::with_guest_dma_gpa`]): the
    /// guest's DMA addresses are its GPAs, registered guest RAM is translated by the host
    /// shadow maps, and no guest helper (caraxes_dma_range) is loaded.
    guest_dma_gpa: bool,
    /// Where the table is published (`<state dir>/dma-ranges`), in the module's format.
    dma_ranges: Option<PathBuf>,
    /// The table entries, per DMA_MAP region.
    ranges: Vec<DmaRange>,
    /// Guest RAM registration off the loop and its worker (registrar.rs), with
    /// `GuestRam::Register`.
    registrar: Option<Arc<Registrar>>,
    registrar_thread: Option<std::thread::JoinHandle<()>>,
    /// The vectors and MSI state the host event thread raises through (E5b part 2).
    intr: Option<Arc<IntrShared>>,
    /// The display engine the display clock and the routed status accesses reach without
    /// the device mutex, republished at every reset (display/core_path.rs).
    display_slot: DisplaySlot,
    /// Doorbell ioeventfd: wakeups of the reader thread and guest writes they counted.
    doorbell_wakeups: u64,
    doorbell_writes: u64,
    /// Guest-token ioeventfds (`CARAXES_DOORBELL_GUEST_TOKENS`): wakeups and guest writes.
    guest_token_rings: u64,
    guest_token_writes: u64,
    /// MSI re-arm ioeventfd (`CARAXES_MSI_REARM_IOEVENTFD=1`): wakeups and guest writes.
    rearm_wakeups: u64,
    /// Posted doorbells are being rung (`ring_posted`): a guest-signal handler reached from
    /// one of them does not ring the newer ones before the older ones left.
    ringing_posted: bool,
    rearm_writes: u64,
    /// BAR0 writes that landed in the config-space mirror (the MSI re-arm among them), after
    /// which the BAR1 pool's poll did not run (`after_bar0_write`), and the polls that held
    /// the device longer than [`SLOW_POOL_POLL`].
    mirror_writes_unsynced: u64,
    slow_pool_polls: u64,
    /// Command-queue head ioeventfd (`CARAXES_RPC_IOEVENTFD=1`): the register's BAR0
    /// offset, and the wakeups that handed writes to [`Device::rpc_ioeventfd`].
    rpc_head: Option<u64>,
    rpc_wakeups: u64,
}

/// A BAR1 pool poll that holds the device longer than this is traced as an event (the
/// display clock, the doorbell reactor and every trapped access wait for the device lock,
/// docs/caraxes/sottr-stage.md §11).
pub const SLOW_POOL_POLL: std::time::Duration = std::time::Duration::from_millis(2);

/// The switch for the MSI re-arm ioeventfd (docs/caraxes/intr-page.md §4).
pub const MSI_REARM_IOEVENTFD_ENV: &str = "CARAXES_MSI_REARM_IOEVENTFD";

/// `NV_XVE_CYA_2` inside the `NV_PCFG` config-space mirror: the RM's MSI re-arm, written
/// once per interrupt service with the value ignored (docs/caraxes/kernel-rm-hw-surface.md:119,
/// `kernel_bif_gm107.c:136-151`).
pub const NV_XVE_CYA_2: u64 = 0x704;

/// Whether `CARAXES_MSI_REARM_IOEVENTFD=1` is set.
pub fn msi_rearm_ioeventfd_requested() -> bool {
    std::env::var(MSI_REARM_IOEVENTFD_ENV).is_ok_and(|v| v.trim() == "1")
}

/// The switch for the command-queue head ioeventfd (docs/caraxes/gsp-transport-615.md §11).
pub const RPC_IOEVENTFD_ENV: &str = "CARAXES_RPC_IOEVENTFD";

/// Whether `CARAXES_RPC_IOEVENTFD=1` is set.
pub fn rpc_ioeventfd_requested() -> bool {
    std::env::var(RPC_IOEVENTFD_ENV).is_ok_and(|v| v.trim() == "1")
}

/// The command-queue head sub-region at BAR0 `offset` (`NV_PGSP_QUEUE_HEAD(0)`): 4
/// bytes, no datamatch (every value the CPU writes must wake the device; the value is
/// reconstructed from the ring, `BootModel::cmd_head_from_ring`), and the eventfd's
/// reading end. The shape is the MSI re-arm's.
pub fn rpc_head_sub_region(offset: u64) -> std::io::Result<(IoEventFdSubRegion, File)> {
    doorbell_sub_region(offset)
}

/// The MSI re-arm sub-region at BAR0 `offset` (the `NV_PCFG` base plus [`NV_XVE_CYA_2`]):
/// 4 bytes, no datamatch (the value is ignored), and the eventfd's reading end
/// ([`doorbell_sub_region`]).
pub fn msi_rearm_sub_region(offset: u64) -> std::io::Result<(IoEventFdSubRegion, File)> {
    doorbell_sub_region(offset)
}

/// A 4-byte sub-region at BAR0 `offset` without datamatch (one eventfd for every value
/// written: the MSI re-arm and the command-queue head) and the eventfd's reading end. The
/// doorbell register itself is offered only with datamatch, one eventfd per token
/// ([`doorbell_token_sub_regions`]): a wildcard next to datamatch entries at one address
/// is refused by KVM (EEXIST).
pub fn doorbell_sub_region(offset: u64) -> std::io::Result<(IoEventFdSubRegion, File)> {
    // SAFETY: a plain syscall; the result is checked before use.
    let raw = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC) };
    if raw < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: a new descriptor that nothing else owns.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let reader = File::from(fd.try_clone()?);
    let sub_region = IoEventFdSubRegion {
        offset,
        size: 4,
        flags: 0,
        datamatch: 0,
        fd: Arc::new(fd),
    };
    Ok((sub_region, reader))
}

/// The doorbell token pool's sub-regions (docs/caraxes/doorbell-token-pool.md): for each
/// of the `n` pool tokens one ioeventfd sub-region of the doorbell register `offset`,
/// size 4, `IOEVENTFD_FLAG_DATAMATCH` with the token as datamatch, and its own eventfd
/// (vfio-user.rst, "Sub-Region IO FD info format (ioeventfd)": offset, size, flags,
/// datamatch; one descriptor each, since sharing one would lose which token rang).
/// Slot i is token `TOKEN_BASE + i`; the readers come back in slot order.
pub fn doorbell_token_sub_regions(
    offset: u64,
    n: usize,
) -> std::io::Result<Vec<(IoEventFdSubRegion, File)>> {
    (0..n)
        .map(|slot| {
            let (mut sub_region, reader) = doorbell_sub_region(offset)?;
            sub_region.flags = vfio_user::IOEVENTFD_FLAG_DATAMATCH;
            sub_region.datamatch = u64::from(crate::tokens::TokenPool::token(slot));
            Ok((sub_region, reader))
        })
        .collect()
}

/// The guest-token sub-regions (tokens.rs `GuestTokens`, docs/caraxes/doorbell-token-pool.md
/// (g)): for each value one ioeventfd sub-region of the doorbell register `offset`, size 4,
/// `IOEVENTFD_FLAG_DATAMATCH` with the value as datamatch, and its own eventfd, so the
/// eventfd names the value written (vfio-user.rst, "Sub-Region IO FD info format
/// (ioeventfd)"). The readers come back in `values` order.
pub fn doorbell_value_sub_regions(
    offset: u64,
    values: &[u32],
) -> std::io::Result<Vec<(IoEventFdSubRegion, File)>> {
    values
        .iter()
        .map(|&v| {
            let (mut sub_region, reader) = doorbell_sub_region(offset)?;
            sub_region.flags = vfio_user::IOEVENTFD_FLAG_DATAMATCH;
            sub_region.datamatch = u64::from(v);
            Ok((sub_region, reader))
        })
        .collect()
}

/// Wait on the pool's eventfds with one epoll loop and ring the channel of every token
/// signalled, for as long as the process runs. `readers[i]` is the eventfd of slot i.
/// `reactor` says how the loop waits (`CARAXES_DOORBELL_POLL`, poll.rs): blocked in
/// `epoll_wait`, spinning, or hybrid. The Device lock is taken once per wakeup, as before.
pub fn spawn_token_reader(
    device: Arc<Mutex<Device>>,
    doorbell: u64,
    readers: Vec<File>,
    reactor: crate::poll::Reactor,
) -> std::io::Result<Arc<crate::poll::ReaderStats>> {
    spawn_token_reader_hooked(
        device,
        doorbell,
        readers,
        Vec::new(),
        reactor,
        None,
        None,
        None,
        Routes::default(),
    )
}

/// As [`spawn_token_reader`], with a hook the reactor runs on every iteration (the
/// interrupt page, intr_page.rs). `rearm`: the MSI re-arm eventfd as one more reader after
/// the tokens' (`CARAXES_MSI_REARM_IOEVENTFD=1`), `(BAR0 offset, eventfd)`; its wakeups go
/// to [`Device::msi_rearm_ioeventfd`], never to a token. `rpc`: the command-queue head
/// eventfd (`CARAXES_RPC_IOEVENTFD=1`) as one more reader after those, `(BAR0 offset,
/// eventfd)`; its wakeups go to [`Device::rpc_ioeventfd`]. `guest`: the guest-token
/// eventfds (`CARAXES_DOORBELL_GUEST_TOKENS`) and their values, read right after the pool's;
/// their wakeups go to [`Device::doorbell_guest_tokens`].
#[allow(clippy::too_many_arguments)]
pub fn spawn_token_reader_hooked(
    device: Arc<Mutex<Device>>,
    doorbell: u64,
    mut readers: Vec<File>,
    guest: Vec<(u32, File)>,
    reactor: crate::poll::Reactor,
    hook: Option<Box<dyn crate::poll::Hook>>,
    rearm: Option<(u64, File)>,
    rpc: Option<(u64, File)>,
    routes: Routes,
) -> std::io::Result<Arc<crate::poll::ReaderStats>> {
    let tokens = readers.len();
    let values: Vec<u32> = guest.iter().map(|(v, _)| *v).collect();
    readers.extend(guest.into_iter().map(|(_, f)| f));
    let guest_end = readers.len();
    let mut extra = |e: Option<(u64, File)>| {
        e.map(|(offset, fd)| {
            readers.push(fd);
            (readers.len() - 1, offset)
        })
    };
    let rearm_at = extra(rearm);
    let rpc_at = extra(rpc);
    reactor.spawn_hooked(
        readers,
        move |rings, seen, st| {
            // The MSI re-arm with the interrupt page: taken by the page, never waiting for
            // the device mutex (`Routes`).
            let routed_rearm = match (rearm_at, routes.intr_page.as_ref()) {
                (Some((i, _)), Some(p)) if p.exact() && rings.iter().any(|r| r.0 == i) => {
                    p.note_routed();
                    p.rearm();
                    Some(i)
                }
                _ => None,
            };
            if routed_rearm.is_some() && rings.iter().all(|r| Some(r.0) == routed_rearm) {
                return;
            }
            let mut d = crate::dlock::lock(&device, "doorbell/re-arm/rpc eventfds");
            st.lock_wait.record(seen.elapsed());
            crate::launch::mark(crate::launch::Step::Lock);
            if rearm_at.is_none() && rpc_at.is_none() && values.is_empty() {
                return d.doorbell_tokens(doorbell, rings, &mut |host| st.ring(seen, host));
            }
            for r in rings.iter().filter(|r| Some(r.0) != routed_rearm) {
                match (rearm_at, rpc_at) {
                    _ if r.0 < tokens => {
                        d.doorbell_tokens(doorbell, std::slice::from_ref(r), &mut |host| {
                            st.ring(seen, host)
                        })
                    }
                    _ if r.0 < guest_end => d.doorbell_guest_tokens(
                        doorbell,
                        &[(values[r.0 - tokens], r.1)],
                        &mut |host| st.ring(seen, host),
                    ),
                    (Some((i, offset)), _) if r.0 == i => d.msi_rearm_ioeventfd(offset, r.1),
                    (_, Some((i, offset))) if r.0 == i => d.rpc_ioeventfd(offset, r.1),
                    _ => {}
                }
            }
        },
        hook,
    )
}

#[cfg(test)]
fn lock(device: &Mutex<Device>) -> std::sync::MutexGuard<'_, Device> {
    device.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The device as the vfio-user server loop sees it when the doorbell reader thread
/// shares it, with the trapped accesses it serves without the device mutex (`Routes`). A
/// trapped write of the usermode doorbell is posted (execution.md §9.9): queued in the
/// ring gate without the device lock and answered at once, so it costs the guest one BAR0
/// exit; the ring completer rings it under the lock (main.rs). Every other access that
/// takes the device lock rings the posted writes first, before its own work, so none
/// passes a doorbell the guest wrote before it (as on PCIe, where a read or write does not
/// pass a posted write), whatever the client does with its token.
pub struct SharedDevice {
    device: Arc<Mutex<Device>>,
    routes: Routes,
    /// The usermode doorbell's BAR0 offset (`None`: no boot model, nothing is posted).
    doorbell: Option<u64>,
    gate: Arc<crate::ringgate::RingGate>,
}

/// The current display engine, for the paths without the device mutex (`Device::display_slot`).
pub type DisplaySlot = Arc<std::sync::RwLock<Option<Arc<Mutex<crate::display::engine::Engine>>>>>;

/// The display's status registers without the device mutex (display/core_path.rs).
#[derive(Clone)]
pub struct DisplayRoute {
    pub slot: DisplaySlot,
    pub mem: crate::guestmem::MemView,
}

impl DisplayRoute {
    fn engine(&self) -> Option<Arc<Mutex<crate::display::engine::Engine>>> {
        self.slot.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    fn mem(&self) -> Arc<GuestMemory> {
        self.mem.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// The display clock's tick when the device mutex is taken: advance the engine
    /// without it. Returns whether the display interrupt line is asserted.
    pub fn tick(&self) -> bool {
        match self.engine() {
            Some(e) => crate::display::core_path::tick(&e, &self.mem()),
            None => false,
        }
    }
}

/// Trapped BAR0 accesses the server serves without the device mutex, each on its own
/// lock (docs/caraxes/display-own-clock.md §2.4): a device operation that holds the mutex
/// (a mirror build, a shadow walk) must not stall the guest's interrupt service.
#[derive(Clone, Default)]
pub struct Routes {
    /// The interrupt page: accesses of its two pages (the tree's stores in exact mode are
    /// applied at once under the page's lock; page 0 is latched memory).
    pub intr_page: Option<Arc<crate::intr_page::IntrPage>>,
    /// BAR0 offset of the MSI re-arm (`NV_PCFG` + [`NV_XVE_CYA_2`]), taken by the page.
    pub rearm: Option<u64>,
    /// The display engine's status registers (`Engine::handles`: the FE interrupt status,
    /// head state, scan position), served under the display lock only.
    pub display: Option<DisplayRoute>,
    /// The display page (display/fe_page.rs): a trapped store to it (a VMM that maps it
    /// read-only) is applied under the page's own lock, as the reactor applies a store to
    /// the mapping.
    pub fe_page: Option<Arc<crate::display::fe_page::FePage>>,
}

impl Routes {
    /// The display page, when the whole access lies in it.
    fn fe_page_for(&self, region: u32, offset: u64, len: usize) -> Option<&Arc<crate::display::fe_page::FePage>> {
        self.fe_page.as_ref().filter(|p| {
            region == VFIO_PCI_BAR0_REGION_INDEX
                && p.covers(offset)
                && p.covers(offset + len.max(1) as u64 - 1)
        })
    }

    /// Serve a trapped write without the device mutex; false when it is not routed.
    pub fn write(&self, region: u32, offset: u64, data: &[u8]) -> bool {
        if let Some(p) = self.fe_page_for(region, offset, data.len()) {
            p.stats.routed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            p.write(offset, data);
            return true;
        }
        if region == VFIO_PCI_BAR0_REGION_INDEX && data.len() == 4 && offset % 4 == 0 {
            if let Some(e) = self.display.as_ref().and_then(|d| d.engine().map(|e| (d, e))) {
                let v = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                if crate::display::core_path::write32(&e.1, &e.0.mem(), offset, v) {
                    return true;
                }
            }
        }
        let Some(p) = self.intr_page.as_ref().filter(|_| region == VFIO_PCI_BAR0_REGION_INDEX) else {
            return false;
        };
        // The re-arm only in exact mode: with the tagged page the trapped re-arm is also
        // where the boot model's own pending MSI goes out (`after_write`), as before.
        if self.rearm == Some(offset) && p.exact() {
            p.note_routed();
            p.rearm();
            return true;
        }
        if p.covers(offset) {
            p.note_routed();
            p.write(offset, data);
            return true;
        }
        false
    }

    /// Serve a trapped read without the device mutex; false when it is not routed.
    pub fn read(&self, region: u32, offset: u64, data: &mut [u8]) -> bool {
        if let Some(p) = self.fe_page_for(region, offset, data.len()) {
            p.stats.routed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            for (i, b) in data.iter_mut().enumerate() {
                let o = offset + i as u64;
                *b = (p.read32(o & !3) >> ((o & 3) * 8)) as u8;
            }
            return true;
        }
        if region == VFIO_PCI_BAR0_REGION_INDEX && data.len() == 4 && offset % 4 == 0 {
            if let Some(e) = self.display.as_ref().and_then(|d| d.engine().map(|e| (d, e))) {
                if let Some(v) = crate::display::core_path::read32(&e.1, &e.0.mem(), offset) {
                    data.copy_from_slice(&v.to_le_bytes());
                    return true;
                }
            }
        }
        let Some(p) = self.intr_page.as_ref().filter(|_| region == VFIO_PCI_BAR0_REGION_INDEX) else {
            return false;
        };
        if !p.covers(offset) || !p.covers(offset + data.len().max(1) as u64 - 1) {
            return false;
        }
        p.note_routed();
        for (i, b) in data.iter_mut().enumerate() {
            let o = offset + i as u64;
            *b = (p.read32(o & !3) >> ((o & 3) * 8)) as u8;
        }
        true
    }
}

impl SharedDevice {
    /// The process's device with the process's ring gate, whose completer (main.rs) rings
    /// the posted doorbells.
    pub fn new(device: Arc<Mutex<Device>>, routes: Routes) -> Self {
        Self::with_gate(device, routes, crate::ringgate::global())
    }

    fn with_gate(device: Arc<Mutex<Device>>, routes: Routes, gate: Arc<crate::ringgate::RingGate>) -> Self {
        let doorbell = device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .bar0
            .boot()
            .map(|b| b.usermode_doorbell());
        Self {
            device,
            routes,
            doorbell,
            gate,
        }
    }

    /// The device lock for a trapped access or another vfio-user message: the posted
    /// doorbells are rung first.
    fn lock(&self, label: &'static str) -> crate::dlock::Held<'_, Device> {
        let mut d = crate::dlock::lock(&self.device, label);
        d.ring_posted(&self.gate, true);
        d
    }

    /// A trapped write of the usermode doorbell, posted.
    fn post_doorbell(&self, region: u32, offset: u64, data: &[u8]) -> bool {
        if region != VFIO_PCI_BAR0_REGION_INDEX || Some(offset) != self.doorbell {
            return false;
        }
        let Ok(token) = <[u8; 4]>::try_from(data) else {
            return false;
        };
        let t0 = std::time::Instant::now();
        self.gate.post(u32::from_le_bytes(token), t0);
        if crate::profile::on() {
            crate::profile::access(region, true, offset, Some("usermode doorbell (posted)"), t0);
        }
        true
    }
}

/// The device-lock label of a trapped access (dlock.rs).
fn access_label(region: u32, write: bool) -> &'static str {
    match (region, write) {
        (VFIO_PCI_BAR0_REGION_INDEX, false) => "trapped BAR0 read",
        (VFIO_PCI_BAR0_REGION_INDEX, true) => "trapped BAR0 write",
        (1, false) => "trapped BAR1 read",
        (1, true) => "trapped BAR1 write",
        (3, false) => "trapped BAR3 read",
        (3, true) => "trapped BAR3 write",
        (_, false) => "trapped read (other region)",
        (_, true) => "trapped write (other region)",
    }
}

/// The device lock for a trapped access at `offset` of `region`, the posted doorbells rung
/// first; a long hold keeps the register (dlock.rs).
fn lock_access<'a>(
    shared: &'a SharedDevice,
    region: u32,
    offset: u64,
    write: bool,
) -> crate::dlock::Held<'a, Device> {
    let mut d = shared.lock(access_label(region, write));
    d.detail(move || format!("region {region} {} {offset:#x}", if write { "W" } else { "R" }));
    d
}

impl SharedDevice {
    /// The state of the reads answered without the device lock (fastread.rs): registered at
    /// the first trapped read, under the lock once.
    fn fast_reads(&self) -> std::sync::Arc<crate::fastread::FastReads> {
        let key = Arc::as_ptr(&self.device) as usize;
        if let Some(f) = crate::fastread::lookup(key) {
            return f;
        }
        let d = crate::dlock::lock(&self.device, "fast reads setup");
        let f = d
            .bar0
            .boot()
            .map(|b| b.fast())
            .unwrap_or_else(|| std::sync::Arc::new(crate::fastread::FastReads::default()));
        // The kinds the trace writes a line for keep the locked path (CARAXES_TRACE_ACCESSES
        // may name a BAR0 range without them).
        let at = |off: Option<u64>| {
            off.is_some_and(|o| d.trace.records_access(VFIO_PCI_BAR0_REGION_INDEX, o))
        };
        let boot = d.bar0.boot();
        let regs = boot.map(|b| b.time_registers());
        let mut traced = 0;
        if at(boot.map(|b| b.mmu_invalidate_register())) {
            traced |= crate::fastread::TRACED_TRIGGER;
        }
        if d.trace.records_access(3, 0) {
            traced |= crate::fastread::TRACED_FLUSH;
        }
        if at(regs.map(|r| r[0])) || at(regs.map(|r| r[1])) {
            traced |= crate::fastread::TRACED_PTIMER;
        }
        if at(regs.map(|r| r[2])) || at(regs.map(|r| r[3])) {
            traced |= crate::fastread::TRACED_USERMODE_TIME;
        }
        f.set_traced(traced);
        drop(d);
        crate::fastread::register(key, &f);
        f
    }
}

impl ServerBackend for SharedDevice {
    fn region_read(&mut self, region: u32, offset: u64, data: &mut [u8]) -> std::io::Result<()> {
        if self.routes.read(region, offset, data) {
            return Ok(());
        }
        // The MMU invalidate's TRIGGER and the BAR2 flush page are status reads on the card:
        // answered from the published state, without the device lock (fastread.rs,
        // docs/caraxes/execution.md §9.10); anything else, and those when the state does not
        // suffice, take the lock below.
        if region == VFIO_PCI_BAR0_REGION_INDEX || region == 3 {
            let t0 = std::time::Instant::now();
            if self.fast_reads().try_read(region, offset, data) {
                if crate::profile::on() {
                    let name = self.fast_reads().name_of(region, offset);
                    crate::profile::access(region, false, offset, name, t0);
                }
                return Ok(());
            }
        }
        if !crate::profile::on() {
            return lock_access(self, region, offset, false).region_read(region, offset, data);
        }
        // The profile's trapped access: from the message to the answer, the lock wait
        // included (profile.rs).
        let t0 = std::time::Instant::now();
        let mut d = lock_access(self, region, offset, false);
        let r = d.region_read(region, offset, data);
        let name = d.register_name(region, offset);
        drop(d);
        crate::profile::access(region, false, offset, name, t0);
        r
    }

    fn region_write(&mut self, region: u32, offset: u64, data: &[u8]) -> std::io::Result<()> {
        if self.post_doorbell(region, offset, data) {
            return Ok(());
        }
        // A display channel's PUT (its USERD page) that the routes serve without the device
        // lock: a flip's semaphore acquire may wait for work a posted doorbell starts, so
        // the posted doorbells are rung first (`lock` rings them), as for every other access.
        if region == VFIO_PCI_BAR0_REGION_INDEX
            && self.gate.has_posted()
            && crate::display::USERD_RANGE.contains(&offset)
        {
            drop(self.lock("display PUT after posted doorbells"));
        }
        if self.routes.write(region, offset, data) {
            return Ok(());
        }
        let timing = crate::launch::on() && region == VFIO_PCI_BAR0_REGION_INDEX;
        if !crate::profile::on() && !timing {
            return lock_access(self, region, offset, true).region_write(region, offset, data);
        }
        let t0 = std::time::Instant::now();
        let mut d = lock_access(self, region, offset, true);
        // A trapped doorbell is a ring of the launch chain too (launch.rs `begin_trapped`).
        let ring = timing && d.is_usermode_doorbell(offset);
        if ring {
            crate::launch::begin_trapped(t0);
            crate::launch::mark(crate::launch::Step::Lock);
        }
        let r = d.region_write(region, offset, data);
        if ring {
            crate::launch::end();
        }
        if crate::profile::on() {
            let name = d.register_name(region, offset);
            drop(d);
            crate::profile::access(region, true, offset, name, t0);
        }
        r
    }

    fn dma_map(
        &mut self,
        flags: DmaMapFlags,
        offset: u64,
        address: u64,
        size: u64,
        fd: Option<File>,
    ) -> std::io::Result<()> {
        self.lock("dma map").dma_map(flags, offset, address, size, fd)
    }

    fn dma_unmap(&mut self, flags: DmaUnmapFlags, address: u64, size: u64) -> std::io::Result<()> {
        self.lock("dma unmap").dma_unmap(flags, address, size)
    }

    fn reset(&mut self) -> std::io::Result<()> {
        self.lock("reset").reset()
    }

    fn set_irqs(
        &mut self,
        index: u32,
        flags: u32,
        start: u32,
        count: u32,
        fds: Vec<File>,
    ) -> std::io::Result<()> {
        self.lock("set irqs").set_irqs(index, flags, start, count, fds)
    }
}

/// Size of each vfio region of `board`, by index (BAR0..5, ROM, config, VGA).
pub fn region_size(board: &Board, index: u32) -> u64 {
    match index {
        i if i < 6 => board.bars[i as usize].size(),
        VFIO_PCI_ROM_REGION_INDEX => board.rom_bar_size,
        VFIO_PCI_CONFIG_REGION_INDEX => config::SIZE as u64,
        _ => 0,
    }
}

/// The region table announced to the client: everything trapped, nothing mmap-able.
pub fn regions(board: &Board) -> Vec<ServerRegion> {
    regions_with(board, None)
}

/// P21 (docs/caraxes/bar1-passthrough.md §6): guest BAR1 `[0, len)` offered for mmap
/// from the dma-buf `fd` of the reservation piece behind it.
#[derive(Clone, Copy, Debug)]
pub struct Bar1Area {
    pub fd: std::os::fd::RawFd,
    pub len: u64,
}

/// The region table, with BAR1 `[0, area.len)` mmap-able when `bar1` is given: region 1
/// gets `VFIO_REGION_INFO_FLAG_MMAP`, file offset 0 and one sparse area `[0, len)`, so
/// the client mmaps the descriptor at offset 0 + 0 (vfio-user.rst, "offset" of
/// `VFIO_USER_DEVICE_GET_REGION_INFO` and of the sparse mmap areas); the rest of BAR1
/// stays trapped.
pub fn regions_with(board: &Board, bar1: Option<Bar1Area>) -> Vec<ServerRegion> {
    regions_with_pages(board, bar1, None, None)
}

/// The interrupt page (docs/caraxes/intr-page.md §2): region 0 mmap-able from the memfd
/// `fd` over the sparse `areas` (offset, size) only.
#[derive(Clone, Debug)]
pub struct Bar0Areas {
    pub fd: std::os::fd::RawFd,
    /// (offset, size): the interrupt page's two pages, and the display page when on.
    pub areas: Vec<(u64, u64)>,
}

/// The region table with the P21 BAR1 area and the interrupt page's BAR0 areas, each when
/// given: region 0 gets `VFIO_REGION_INFO_FLAG_MMAP`, file offset 0 and the two sparse
/// areas (vfio-user.rst:861-877, :943-947); everything outside them stays trapped.
pub fn regions_with_pages(
    board: &Board,
    bar1: Option<Bar1Area>,
    bar2: Option<&Bar2Region>,
    bar0: Option<Bar0Areas>,
) -> Vec<ServerRegion> {
    let mut r = regions_with_bar2(board, bar1, bar2);
    if let Some(a) = bar0 {
        let size = region_size(board, VFIO_PCI_BAR0_REGION_INDEX);
        if a.areas.iter().all(|&(o, l)| l > 0 && o + l <= size) {
            let b0 = &mut r[VFIO_PCI_BAR0_REGION_INDEX as usize];
            b0.region_info.flags |= VFIO_REGION_INFO_FLAG_MMAP;
            b0.region_info.offset = 0;
            b0.sparse_areas = a
                .areas
                .iter()
                .map(|&(offset, size)| SparseArea {
                    area: vfio_bindings::bindings::vfio::vfio_region_sparse_mmap_area {
                        offset,
                        size,
                    },
                })
                .collect();
            b0.mmap_fd = Some(a.fd);
        }
    }
    r
}

/// `CARAXES_BAR2_MEMFD=1` (docs/caraxes/bar2-memfd.md): region 3 (BAR2) offered for mmap
/// from the memfd `fd`, in the sparse areas `(offset, size)` (all of BAR2 but the pages
/// that stay trapped).
#[derive(Clone, Debug)]
pub struct Bar2Region {
    pub fd: std::os::fd::RawFd,
    pub areas: Vec<(u64, u64)>,
}

/// [`regions_with`], and region 3 mmap-able from the BAR2 memfd when `bar2` is given:
/// `VFIO_REGION_INFO_FLAG_MMAP`, file offset 0 (memfd offset = BAR2 offset), one sparse
/// area per stretch between trapped pages. The client maps each area at `0 + offset`
/// (vfio-user.rst, sparse mmap areas) and traps the rest.
pub fn regions_with_bar2(
    board: &Board,
    bar1: Option<Bar1Area>,
    bar2: Option<&Bar2Region>,
) -> Vec<ServerRegion> {
    (0..VFIO_PCI_NUM_REGIONS)
        .map(|index| {
            let size = region_size(board, index);
            let mut flags = 0;
            if size > 0 {
                flags |= VFIO_REGION_INFO_FLAG_READ;
                if index != VFIO_PCI_ROM_REGION_INDEX {
                    flags |= VFIO_REGION_INFO_FLAG_WRITE;
                }
            }
            if let (3, Some(b)) = (index, bar2) {
                let fits = b
                    .areas
                    .iter()
                    .all(|&(o, l)| l > 0 && o.checked_add(l).is_some_and(|e| e <= size));
                if !b.areas.is_empty() && fits {
                    return ServerRegion {
                        region_info: vfio_region_info {
                            argsz: size_of::<vfio_region_info>() as u32,
                            flags: flags | VFIO_REGION_INFO_FLAG_MMAP,
                            index,
                            size,
                            offset: 0,
                            ..Default::default()
                        },
                        sparse_areas: b
                            .areas
                            .iter()
                            .map(|&(offset, size)| SparseArea {
                                area: vfio_bindings::bindings::vfio::vfio_region_sparse_mmap_area {
                                    offset,
                                    size,
                                },
                            })
                            .collect(),
                        mmap_fd: Some(b.fd),
                    };
                }
            }
            if let (1, Some(a)) = (index, bar1) {
                if a.len > 0 && a.len <= size {
                    return ServerRegion {
                        region_info: vfio_region_info {
                            argsz: size_of::<vfio_region_info>() as u32,
                            flags: flags | VFIO_REGION_INFO_FLAG_MMAP,
                            index,
                            size,
                            offset: 0,
                            ..Default::default()
                        },
                        sparse_areas: vec![SparseArea {
                            area: vfio_bindings::bindings::vfio::vfio_region_sparse_mmap_area {
                                offset: 0,
                                size: a.len,
                            },
                        }],
                        mmap_fd: Some(a.fd),
                    };
                }
            }
            ServerRegion {
                region_info: vfio_region_info {
                    argsz: size_of::<vfio_region_info>() as u32,
                    flags,
                    index,
                    size,
                    // The kernel's VFIO_PCI_INDEX_TO_OFFSET convention; unused for
                    // trapped regions but keeps clients that mirror it content.
                    offset: (index as u64) << 40,
                    ..Default::default()
                },
                sparse_areas: Vec::new(),
                mmap_fd: None,
            }
        })
        .collect()
}

/// INTx and one MSI vector, as the card offers them; no MSI-X (open question Q20).
pub fn irqs() -> Vec<IrqInfo> {
    (0..VFIO_PCI_NUM_IRQS)
        .map(|index| {
            let (count, flags) = match index {
                VFIO_PCI_INTX_IRQ_INDEX => (1, VFIO_IRQ_INFO_EVENTFD),
                VFIO_PCI_MSI_IRQ_INDEX => (1, VFIO_IRQ_INFO_EVENTFD | VFIO_IRQ_INFO_NORESIZE),
                _ => (0, 0),
            };
            IrqInfo {
                index,
                flags,
                count,
            }
        })
        .collect()
}

impl Device {
    pub fn new(cfg: ConfigSpace, bar0: Bar0, trace: Trace) -> Self {
        let irq_fds = (0..VFIO_PCI_NUM_IRQS).map(|_| Vec::new()).collect();
        Device {
            cfg_reset: cfg.clone(),
            cfg,
            bar0,
            trace,
            dma: Vec::new(),
            mem: GuestMemory::new(),
            irq_fds,
            guest_ram: GuestRam::Plain,
            guest_dma_gpa: false,
            dma_ranges: None,
            ranges: Vec::new(),
            registrar: None,
            registrar_thread: None,
            intr: None,
            display_slot: DisplaySlot::default(),
            doorbell_wakeups: 0,
            doorbell_writes: 0,
            guest_token_rings: 0,
            guest_token_writes: 0,
            rearm_wakeups: 0,
            ringing_posted: false,
            rearm_writes: 0,
            mirror_writes_unsynced: 0,
            slow_pool_polls: 0,
            rpc_head: None,
            rpc_wakeups: 0,
        }
    }

    /// Serve the command-queue head register at BAR0 `offset` through its ioeventfd
    /// ([`Device::rpc_ioeventfd`]); the boot model holds a duplicate of the eventfd
    /// (`BootModel::with_rpc_eventfd`).
    pub fn set_rpc_ioeventfd(&mut self, offset: u64) {
        self.rpc_head = Some(offset);
    }

    /// Guest writes to the command-queue head register (`NV_PGSP_QUEUE_HEAD(0)`) that
    /// arrived through its ioeventfd (`CARAXES_RPC_IOEVENTFD=1`, gsp-transport-615.md §11):
    /// `writes` of them, values unknown. Each is served as the trapped write is
    /// (`region_write` on BAR0: the model's write, which services the queue and writes
    /// the reply, then the steps after every BAR0 write, which raise a pending MSI), with
    /// the value the boot model reconstructs from the command ring
    /// (`BootModel::cmd_head_from_ring`); a write that published nothing is noted only.
    pub fn rpc_ioeventfd(&mut self, offset: u64, writes: u64) {
        self.ring_posted_first();
        self.rpc_wakeups += 1;
        crate::profile::window("rpc eventfd: wakeups (units: guest head writes)", writes);
        for _ in 0..writes {
            let mem = &self.mem;
            let (value, note) = match self.bar0.boot_mut() {
                Some(b) => b.cmd_head_from_ring(mem),
                None => (None, "no boot model".into()),
            };
            self.trace
                .event(&format!("rpc ioeventfd: head write to BAR0 {offset:#x}: {note}"));
            if let Some(v) = value {
                let effect = self
                    .bar0
                    .write(&mut self.cfg, &self.mem, offset, &v.to_le_bytes());
                self.after_bar0_write(!matches!(effect, WriteEffect::Mirror), offset);
            }
        }
    }

    /// Before a trapped access of the GSP falcon page (0x110000, the queue pointers and
    /// the falcon registers the CPU polls after a head write, gsp-transport-classes.md §2):
    /// serve head writes the eventfd counted and the reactor has not taken yet. The guest's
    /// access follows its head write, and KVM signalled the eventfd inside that write's
    /// exit, so the element is served before the guest reads the status pointer, whatever
    /// the reactor's wake-up latency.
    fn poll_rpc_ioeventfd(&mut self, offset: u64) {
        let Some(head) = self.rpc_head else {
            return;
        };
        if offset >> 12 != head >> 12 {
            return;
        }
        let n = self.bar0.boot_mut().map_or(0, |b| b.take_rpc_writes());
        if n > 0 {
            self.rpc_ioeventfd(head, n);
        }
    }

    /// Guest writes to the MSI re-arm register (`NV_XVE_CYA_2`, BAR0 `offset`) that arrived
    /// through its ioeventfd (`CARAXES_MSI_REARM_IOEVENTFD=1`, intr-page.md §4): `writes` of
    /// them since the last wakeup. The value is ignored, so this does exactly what one
    /// trapped write does (`region_write` on BAR0): the config mirror write
    /// (no writable bit at `NV_XVE_CYA_2`, as for the trapped one) and the steps after
    /// a BAR0 write that lands in the config mirror (`after_bar0_write` without the sync
    /// point: a pending MSI of the model, the MSI enable; no P21 check and no BAR1 pool
    /// poll, docs/caraxes/sottr-stage.md §11). Several writes between two wakeups are one
    /// re-arm: each would repeat the same idempotent steps.
    pub fn msi_rearm_ioeventfd(&mut self, offset: u64, writes: u64) {
        self.ring_posted_first();
        self.rearm_wakeups += 1;
        self.rearm_writes += writes;
        crate::profile::window("msi re-arm eventfd: wakeups (units: guest writes)", writes);
        let effect = self.bar0.write(&mut self.cfg, &self.mem, offset, &0u32.to_le_bytes());
        self.trace.event(&format!(
            "msi re-arm (eventfd): {writes} guest write(s) to BAR0 {offset:#x} (NV_XVE_CYA_2) signalled ({effect:?})"
        ));
        self.after_bar0_write(!matches!(effect, WriteEffect::Mirror), offset);
    }

    /// Pool tokens whose ioeventfds fired: `(slot, guest writes counted)` each. The
    /// eventfd names the token, the pool names the channel (`Dispatcher::doorbell_pool`).
    /// `rung` is called once per token with when its host doorbell was written, if it was
    /// (the reactor's ring-to-host-doorbell time, poll.rs); the trace notes are written
    /// after all tokens are rung.
    pub fn doorbell_tokens(
        &mut self,
        doorbell: u64,
        rings: &[(usize, u64)],
        rung: &mut dyn FnMut(Option<std::time::Instant>),
    ) {
        self.ring_posted_first();
        let mem = &self.mem;
        let Some(boot) = self.bar0.boot_mut() else {
            self.trace
                .event("doorbell tokens: no boot model, nothing rung");
            return;
        };
        let fast = crate::launch::fast();
        let mut notes = Vec::new();
        for &(slot, writes) in rings {
            let token = crate::tokens::TokenPool::token(slot);
            if let Some(p) = boot.token_pool_mut() {
                p.stats.eventfd_rings += 1;
                p.stats.eventfd_writes += writes;
            }
            crate::profile::window("token pool: eventfd rings (units: guest writes)", writes);
            // The fast ring path (launch.rs `FAST_ENV`) formats nothing before the host
            // doorbell: the pool's "doorbell token ... (eventfd)" note names the ring.
            if !fast {
                notes.push(format!(
                    "doorbell ioeventfd: {writes} guest write(s) of token {token:#x} to BAR0 {doorbell:#x} signalled"
                ));
            }
            boot.take_host_doorbell();
            notes.extend(boot.doorbell_token(mem, token));
            rung(boot.take_host_doorbell());
        }
        if fast {
            self.trace.events(&notes);
        } else {
            for n in notes {
                self.trace.event(&n);
            }
        }
    }

    /// Guest tokens whose ioeventfds fired (`CARAXES_DOORBELL_GUEST_TOKENS`): `(value, guest
    /// writes counted)` each. The eventfd names the value, so each is rung exactly as the
    /// trapped write of that value (boot.rs `doorbell_value`, ctrl.rs `doorbell`); writes of
    /// one value coalesced into one wakeup ring once (a doorbell only makes the channel look
    /// at GPPut). `rung` as for [`Self::doorbell_tokens`].
    pub fn doorbell_guest_tokens(
        &mut self,
        doorbell: u64,
        rings: &[(u32, u64)],
        rung: &mut dyn FnMut(Option<std::time::Instant>),
    ) {
        for &(_, writes) in rings {
            self.guest_token_rings += 1;
            self.guest_token_writes += writes;
        }
        let mem = &self.mem;
        let Some(boot) = self.bar0.boot_mut() else {
            self.trace
                .event("doorbell guest tokens: no boot model, nothing rung");
            return;
        };
        let fast = crate::launch::fast();
        let mut notes = Vec::new();
        for &(value, writes) in rings {
            crate::profile::window("guest tokens: eventfd rings (units: guest writes)", writes);
            if !fast {
                notes.push(format!(
                    "doorbell ioeventfd: {writes} guest write(s) of token {value:#x} to BAR0 {doorbell:#x} signalled"
                ));
            }
            boot.take_host_doorbell();
            notes.extend(boot.doorbell_value(mem, value));
            rung(boot.take_host_doorbell());
        }
        if fast {
            self.trace.events(&notes);
        } else {
            for n in notes {
                self.trace.event(&n);
            }
        }
    }

    /// Guest DMA addresses are GPAs (host execution: its shadow maps translate registered
    /// RAM). Call before [`Device::with_guest_ram`].
    pub fn with_guest_dma_gpa(mut self, on: bool) -> Self {
        self.guest_dma_gpa = on;
        self
    }

    /// Choose how guest RAM is handled and where the table is published. Registered guest
    /// RAM gets its registration off the loop (registrar.rs); without the worker thread it
    /// stays on the loop.
    pub fn with_guest_ram(mut self, guest_ram: GuestRam, dma_ranges: Option<PathBuf>) -> Self {
        self.guest_ram = guest_ram;
        if self.guest_dma_gpa && !matches!(self.guest_ram, GuestRam::Plain) {
            self.mem.set_table(Some(DmaTable::default()));
        }
        self.dma_ranges = dma_ranges;
        if let GuestRam::Register(hm) = &self.guest_ram {
            let hm = hm.clone();
            let r = Registrar::new(self.guest_dma_gpa);
            let (h, gpa) = (hm.clone(), self.guest_dma_gpa);
            let register = move |p: &Piece| {
                crate::registrar::prefault(p);
                let mut h = h.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let va = p.va as *mut u8;
                let out = if gpa {
                    h.register_gpa_queued(p.gpa, p.size, va)
                } else {
                    h.register(p.gpa, p.size, va)
                };
                out.map_err(|e| format!("{e:#}"))
            };
            match Registrar::spawn(&r, Box::new(register)) {
                Ok(t) => {
                    hm.lock().unwrap().set_registrar(Some(r.clone()));
                    self.mem.set_registrar(Some(r.clone()));
                    self.registrar = Some(r);
                    self.registrar_thread = Some(t);
                }
                Err(e) => self.note(&format!(
                    "ERROR guest RAM registrar thread: {e}; registering on the loop"
                )),
            }
        }
        self
    }

    /// A `DMA_MAP` answered now: its checks here (the mapping may change, the host RAM
    /// floor counting what is still queued), its registration queued (registrar.rs), in
    /// 1 GiB pieces in GPA mode, whole for the linear registration.
    fn queue_guest_ram(&mut self, address: u64, size: u64) -> std::io::Result<()> {
        let (Some(r), GuestRam::Register(hm)) = (self.registrar.clone(), &self.guest_ram) else {
            return Err(std::io::Error::other("no registrar"));
        };
        let checked = if self.guest_dma_gpa {
            hm.lock().unwrap().check_gpa_mapping_update()
        } else {
            Ok(())
        }
        .and_then(|()| crate::hostmem::check_ram_floor(size.saturating_add(r.outstanding())));
        if let Err(e) = checked {
            self.note(&format!("hostmem: guest RAM {address:#x}+{size:#x}: {e:#}"));
            return Err(std::io::Error::other(format!("{e:#}")));
        }
        let va = self
            .mem
            .host_ptr(address, size)
            .ok_or_else(|| std::io::Error::other("region not mapped"))? as usize;
        let chunk = if self.guest_dma_gpa {
            crate::hostmem::CHUNK
        } else {
            u64::MAX
        };
        let pieces = crate::registrar::pieces(address, size, va, chunk);
        self.note(&format!(
            "hostmem: guest RAM {address:#x}+{size:#x} answered, {} piece(s) queued for registration off the loop ({} MiB queued before)",
            pieces.len(),
            r.outstanding() >> 20
        ));
        r.queue(pieces);
        Ok(())
    }

    /// A `DMA_UNMAP`: the queued pieces of the region are dropped, the one in flight
    /// finishes, and what finished is applied before the region is released.
    fn cancel_guest_ram(&mut self, address: u64, size: u64) -> std::io::Result<()> {
        let Some(r) = self.registrar.clone() else {
            return Ok(());
        };
        if self.guest_dma_gpa {
            // The release refuses the same way; refuse before dropping anything.
            if let GuestRam::Register(hm) = &self.guest_ram {
                hm.lock()
                    .unwrap()
                    .check_gpa_mapping_update()
                    .map_err(std::io::Error::other)?;
            }
        }
        let gone = r.cancel(address, size);
        if !gone.is_empty() {
            self.note(&format!(
                "hostmem: guest RAM {address:#x}+{size:#x} unmapped with {} piece(s) not registered yet: dropped",
                gone.len()
            ));
        }
        self.apply_registrations();
        Ok(())
    }

    /// Apply the pieces the registrar finished: their ranges join the table, published
    /// again. A failed piece keeps no IOVA (DMA to it is refused and counted) and leaves one
    /// ERROR line; after a failed free every DMA is refused, as on the loop.
    pub fn apply_registrations(&mut self) {
        // On every access: one atomic load while nothing finished.
        let Some(r) = self.registrar.as_ref().filter(|r| r.has_done()).cloned() else {
            return;
        };
        let (mut publish, mut failed) = (false, false);
        for (p, out) in r.take_done() {
            match out {
                Ok((ranges, log)) => {
                    for l in log {
                        self.note(&format!("hostmem: {l}"));
                    }
                    self.ranges.extend(ranges);
                    publish = true;
                }
                Err(e) => {
                    self.note(&format!(
                        "ERROR hostmem: guest RAM {:#x}+{:#x} (DMA_MAP answered): registration failed: {e}; DMA to it stays refused",
                        p.gpa, p.size
                    ));
                    failed = true;
                }
            }
        }
        // Only after a failure: hostmem's lock may be the worker's for its next piece.
        let fail_closed = failed && matches!(&self.guest_ram,
            GuestRam::Register(hm) if !hm.lock().unwrap().free_failures().is_empty());
        if fail_closed {
            self.mem.set_table(Some(DmaTable::default()));
        } else if publish {
            if let Err(e) = self.publish_table() {
                self.note(&format!("ERROR dma table: {e}"));
            }
        }
    }

    /// Let the host event thread raise vectors and the MSI (its eventfd and the MSI
    /// enable bit are mirrored into `shared` as the client sets them).
    pub fn with_intr_shared(mut self, shared: Arc<IntrShared>) -> Self {
        self.intr = Some(shared);
        self
    }

    fn sync_msi_enabled(&self) {
        if let Some(s) = &self.intr {
            s.set_msi_enabled(self.cfg.msi_enabled());
        }
    }

    /// Whether BAR0 `offset` is the usermode doorbell (`NOTIFY_CHANNEL_PENDING`).
    pub fn is_usermode_doorbell(&self, offset: u64) -> bool {
        self.bar0
            .boot()
            .is_some_and(|b| b.usermode_doorbell() == offset)
    }

    /// An event for the trace only.
    pub fn log_event(&mut self, s: &str) {
        self.trace.event(s);
    }

    /// Log to the trace and to the device log (gpu.log).
    fn note(&mut self, s: &str) {
        eprintln!("vfio-user-nvgpu: {s}");
        self.trace.event(s);
    }

    /// Make the current entries the translation and publish them for the guest module
    /// (written to a temporary file and renamed, so a reader never sees half a table).
    fn publish_table(&mut self) -> std::io::Result<()> {
        if matches!(self.guest_ram, GuestRam::Plain) {
            return Ok(());
        }
        let table = if self.guest_dma_gpa {
            DmaTable::for_gpa_guest(self.ranges.clone())
        } else {
            DmaTable::new(self.ranges.clone())
        }.map_err(std::io::Error::other)?;
        let aliases = if self.guest_dma_gpa {
            let identity = DmaTable::for_gpa_guest(table.ranges().iter().map(|r|
                DmaRange { gpa: r.gpa, iova: r.gpa, size: r.size }).collect())
                .map_err(std::io::Error::other)?;
            Some(table.aliases_of(&identity).map_err(std::io::Error::other)?)
        } else { None };
        let param = table.module_param();
        let what = format!(
            "dma table: {} range(s), {}; ranges={param}",
            table.ranges().len(),
            if table.is_identity() {
                "identity"
            } else {
                "host IOVAs"
            }
        );
        if let Some(path) = self.dma_ranges.clone() {
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, format!("{param}\n"))?;
            std::fs::rename(&tmp, &path)?;
            self.note(&format!("{what}; published to {}", path.display()));
        } else {
            self.note(&what);
        }
        // Commit only after validation and publication succeed. An empty GPA-mode
        // table refuses DMA; it must not fall back to unregistered identity access.
        self.mem.set_table(if table.is_empty() && !self.guest_dma_gpa { None } else { Some(table) });
        if let Some(aliases) = aliases {
            let n = aliases.len();
            self.mem.set_aliases(aliases);
            self.note(&format!("dma table: guest speaks GPA (host execution): {n} identity alias range(s)"));
        }
        Ok(())
    }

    /// The table the guest installed, as the launcher records it next to the published one
    /// (`<dma-ranges>.guest`, tools/caraxes-vm.sh `dma_load`, written before the module is
    /// loaded and the function is reset). Its swiotlb alias gives some guest RAM a second
    /// DMA address, and its 2 MiB rounding includes ordinary pages: a buffer there reaches
    /// the device at the alias (docs/caraxes/memory-model.md §2, "The alias in the
    /// device"). Read at every reset, checked against the device's own table
    /// (dmamap.rs `aliases_of`); a missing file means no alias, a table that fails the check
    /// is refused with an ERROR line and the aliases stay off.
    /// GPA mode (host execution): a guest without caraxes_dma_range hands the device
    /// guest physical addresses, which under QEMU's vfio-user client without a vIOMMU are the
    /// DMA_MAP addresses. The guest's table is then the identity of the registered guest RAM:
    /// every GPA is an alias of its registered IOVA (checked by dmamap.rs `aliases_of`, so an
    /// alias can name only this VM's registered pages), and the shadow maps and the device's
    /// own accesses translate it (guestmem.rs `host_iova`, `translate`). Returns whether it
    /// applied.
    fn gpa_aliases(&mut self) -> bool {
        if !self.guest_dma_gpa {
            return false;
        }
        let Some(table) = self.mem.table().cloned() else {
            return true;
        };
        let ident: Vec<DmaRange> = table
            .ranges()
            .iter()
            .map(|r| DmaRange { gpa: r.gpa, iova: r.gpa, size: r.size })
            .collect();
        match DmaTable::for_gpa_guest(ident).and_then(|g| table.aliases_of(&g)) {
            Ok(aliases) => {
                let n = aliases.len();
                self.mem.set_aliases(aliases);
                self.note(&format!("dma table: guest speaks GPA (host execution): {n} identity alias range(s)"));
            }
            Err(e) => {
                self.mem.set_aliases(Vec::new());
                self.note(&format!("ERROR dma table: GPA aliases refused: {e}"));
            }
        }
        true
    }

    fn load_guest_aliases(&mut self) {
        if self.gpa_aliases() {
            return;
        }
        let (Some(path), Some(table)) = (self.dma_ranges.clone(), self.mem.table().cloned()) else {
            return;
        };
        let gpath = path.with_extension("guest");
        let text = match std::fs::read_to_string(&gpath) {
            Ok(t) => t,
            Err(_) => {
                if !self.mem.aliases().is_empty() {
                    self.mem.set_aliases(Vec::new());
                    self.note(&format!("dma table: {} gone, guest aliases dropped", gpath.display()));
                }
                return;
            }
        };
        let result = DmaTable::parse(&text).and_then(|g| table.aliases_of(&g));
        match result {
            Ok(aliases) => {
                if aliases.as_slice() == self.mem.aliases() {
                    return;
                }
                let what = if aliases.is_empty() {
                    "none".to_string()
                } else {
                    aliases.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(", ")
                };
                self.mem.set_aliases(aliases);
                self.note(&format!("dma table: guest aliases from {}: {what}", gpath.display()));
            }
            Err(e) => {
                self.mem.set_aliases(Vec::new());
                self.note(&format!(
                    "ERROR dma table: guest table {} refused, no aliases: {e}",
                    gpath.display()
                ));
            }
        }
    }

    /// Register or describe a freshly mapped region; an error refuses the DMA_MAP.
    fn add_guest_ram(&mut self, address: u64, size: u64) -> std::io::Result<()> {
        let old_ranges = self.ranges.clone();
        match &self.guest_ram {
            GuestRam::Plain => return Ok(()),
            GuestRam::Identity => self.ranges.push(DmaRange {
                gpa: address,
                iova: address,
                size,
            }),
            GuestRam::Register(hm) => {
                let hm = hm.clone();
                let ptr = self
                    .mem
                    .host_ptr(address, size)
                    .ok_or_else(|| std::io::Error::other("region not mapped"))?;
                let r = if self.guest_dma_gpa {
                    hm.lock().unwrap().register_gpa(address, size, ptr)
                } else {
                    hm.lock().unwrap().register(address, size, ptr)
                };
                match r {
                    Ok((ranges, log)) => {
                        for l in log {
                            self.note(&format!("hostmem: {l}"));
                        }
                        self.ranges.extend(ranges);
                    }
                    Err(e) => {
                        self.note(&format!("hostmem: guest RAM {address:#x}+{size:#x}: {e:#}"));
                        return Err(std::io::Error::other(format!("{e:#}")));
                    }
                }
            }
        }
        if let Err(e) = self.publish_table() {
            self.ranges = old_ranges;
            if self.guest_dma_gpa {
                if let GuestRam::Register(hm) = &self.guest_ram {
                    hm.lock().unwrap().release_gpa_region(address, size)
                        .map_err(|free| std::io::Error::other(format!("{e}; rollback: {free:#}")))?;
                }
            }
            return Err(e);
        }
        Ok(())
    }

    /// The guest RAM behind a DMA_UNMAP goes: free its descriptors first.
    fn remove_guest_ram(&mut self, address: u64, size: u64) -> std::io::Result<()> {
        let log = match &self.guest_ram {
            GuestRam::Plain => return Ok(()),
            GuestRam::Identity => Vec::new(),
            GuestRam::Register(hm) if self.guest_dma_gpa => {
                // Refuse before touching the table or any CPU mapping if host work
                // can still reference these pages.
                hm.lock().unwrap().check_gpa_mapping_update().map_err(std::io::Error::other)?;
                let result = hm.lock().unwrap().release_gpa_region(address, size);
                match result {
                    Ok(log) => log,
                    Err(e) => {
                        // Some descriptors may already have been freed. Revoke all
                        // translations and retain CPU mappings for failed frees.
                        self.ranges.clear();
                        self.mem.set_table(Some(DmaTable::default()));
                        return Err(std::io::Error::other(e));
                    }
                }
            }
            GuestRam::Register(hm) => hm.lock().unwrap().release_region(address, size),
        };
        for l in log {
            self.note(&format!("hostmem: {l}"));
        }
        self.ranges
            .retain(|r| !(r.gpa >= address && r.gpa + r.size <= address + size));
        if self.guest_dma_gpa {
            self.mem.set_table(Some(DmaTable::default()));
            return self.publish_table();
        }
        if let Err(e) = self.publish_table() {
            self.note(&format!("dma table: {e}"));
        }
        Ok(())
    }

    /// Free the host memory at the end of the session (the client disconnected). The
    /// guest is gone, so nothing uses the IOVAs any more.
    pub fn shutdown(&mut self) {
        // No registration after this; the piece in flight finishes (hostmem frees it with
        // the rest).
        if let Some(r) = self.registrar.take() {
            r.stop();
            if let Some(t) = self.registrar_thread.take() {
                let _ = t.join();
            }
            let (n, t) = r.waits();
            self.note(&format!(
                "hostmem: guest RAM registrar: the shadow waited for it {n} time(s), {} ms in total",
                t.as_millis()
            ));
        }
        // Mirror channels first (they may still be scheduled on the physical GPU), then
        // the descriptors and the reservation (docs/caraxes/memory-model.md §4).
        let mirror_log: Vec<String> = self
            .bar0
            .boot_mut()
            .and_then(|b| b.ctrl.mirror.as_mut())
            .map(|m| m.reset())
            .unwrap_or_default();
        for l in mirror_log {
            self.note(&format!("mirror: {l}"));
        }
        let zbc_log = self
            .bar0
            .boot_mut()
            .map(|b| b.ctrl.dstate.release_zbc())
            .unwrap_or_default();
        for l in zbc_log {
            self.note(&l);
        }
        if let Some(b) = self.bar0.boot_mut() {
            // The display core's FB record windows go with the store's own (their BAR1
            // mappings need the host RM client).
            *self.display_slot.write().unwrap_or_else(PoisonError::into_inner) = None;
            b.ctrl.display.eng().pins.clear();
            b.fb.shutdown();
        }
        if let GuestRam::Register(hm) = &self.guest_ram {
            let log = hm.lock().unwrap().shutdown();
            for l in log {
                self.note(&format!("hostmem: {l}"));
            }
        }
    }

    pub fn bar0(&self) -> &Bar0 {
        &self.bar0
    }

    /// The device model's side of the soak line (soak.rs): the mirror's maps, the FB
    /// store's windows and the cached walks.
    /// The interrupt vectors' raises and the guest's acks (intr.rs `vector_summary`).
    pub fn vector_summary(&self) -> Option<String> {
        self.intr.as_ref().and_then(|i| i.vector_summary())
    }

    pub fn soak_counts(&self) -> crate::soak::DeviceCounts {
        let mut c = crate::soak::DeviceCounts::default();
        if let Some(b) = self.bar0.boot() {
            if let Some(m) = &b.ctrl.mirror {
                (c.shadow_vas, c.ce_vas, c.tsgs, c.channels) = m.soak_counts();
            }
            if let Some(st) = b.fb.vram_stats() {
                c.vram_windows = st.held;
                c.vram_failed = st.failed;
                c.bar1_refused = st.bar1_refused;
            }
            c.walk_caches = b.fb.walk_caches();
        }
        c
    }

    /// The register map's name of a BAR0 offset, for the profile's rows (profile.rs).
    pub fn register_name(&self, region: u32, offset: u64) -> Option<&'static str> {
        (region == VFIO_PCI_BAR0_REGION_INDEX)
            .then(|| self.bar0.name(offset))
            .flatten()
    }

    /// Signal the MSI eventfd the client registered (index 1), if MSI is enabled.
    fn raise_msi(&mut self) {
        use std::io::Write;
        let enabled = self.cfg.msi_enabled();
        match self
            .irq_fds
            .get_mut(VFIO_PCI_MSI_IRQ_INDEX as usize)
            .and_then(|v| v.first_mut())
        {
            Some(fd) if enabled => {
                let r = fd.write_all(&1u64.to_le_bytes());
                self.trace.event(&format!("msi: signalled ({r:?})"));
            }
            Some(_) => self
                .trace
                .event("msi: pending but MSI is disabled in config space"),
            None => self
                .trace
                .event("msi: pending but no MSI eventfd registered"),
        }
    }

    pub fn guest_memory(&self) -> &GuestMemory {
        &self.mem
    }

    /// The channels the USERD poller posted (main.rs, execution.md §9.11), rung under the
    /// device lock after the locked checks.
    pub fn ring_polled_userds(&mut self, gate: &crate::ringgate::RingGate) {
        let chids = gate.take_userd();
        if chids.is_empty() {
            return;
        }
        let t0 = std::time::Instant::now();
        let notes = match self.bar0.boot_mut() {
            Some(b) => b.ring_userd_pending(&self.mem, &chids),
            None => return,
        };
        for n in notes {
            self.trace.event(&format!("boot: {n}"));
        }
        crate::dlock::work("shadow USERD", "rings of the poller".into(), t0.elapsed());
    }

    pub fn poll_shadow_userds(&mut self) {
        let notes = match self.bar0.boot_mut() {
            Some(b) => b.poll_shadow_userds(&self.mem),
            None => return,
        };
        for n in notes {
            self.trace.event(&format!("boot: {n}"));
        }
    }

    /// The next slice of the sliced software CE transfers (channel.rs `CE_SLICE`), on the
    /// `ce-slices` clock of main.rs.
    pub fn continue_ce_slices(&mut self) {
        let notes = match self.bar0.boot_mut() {
            Some(b) => b.continue_ce_slices(&self.mem),
            None => return,
        };
        for n in notes {
            self.trace.event(&format!("boot: {n}"));
        }
    }

    /// A live state dump (main.rs `device-ctl`, "state"): every channel and host object,
    /// the interrupt vectors' raises and acks. Read only; the caller prints it.
    pub fn state_dump(&mut self) -> Vec<String> {
        let mut out = match self.bar0.boot_mut() {
            Some(b) => b.state_lines(&self.mem),
            None => vec!["no boot model".into()],
        };
        if let Some(s) = self.intr.as_ref().and_then(|i| i.vector_summary()) {
            out.push(s);
        }
        out
    }

    /// The software channels whose blocked semaphore acquire is satisfied now continue
    /// (channel.rs `AcquireWait`), on the `ce-slices` clock of main.rs.
    pub fn recheck_acquires(&mut self) {
        let notes = match self.bar0.boot_mut() {
            Some(b) => b.recheck_acquires(&self.mem),
            None => return,
        };
        for n in notes {
            self.trace.event(&format!("boot: {n}"));
        }
    }

    /// The client closed the connection and a new one is waiting (Cloud Hypervisor's
    /// reboot builds a new vfio-user client for the same socket). vfio-user.rst "Client
    /// Disconnection": the old client's memory regions are unmapped and its IRQ eventfds
    /// closed. The device state does not stay as the spec prefers: the host channels the
    /// old guest created may still run work that reaches its RAM, so the model resets
    /// first (its mirror frees them) and the RAM is unregistered after; the configuration
    /// space goes back to its reset state with it, so the new client finds the device a
    /// platform reset leaves (a vfio-pci device is reset when it is opened; Cloud
    /// Hypervisor resets a resettable vfio-user device when it creates it).
    ///
    /// An error leaves the rest as it is: the caller then ends the session as before
    /// (the full teardown), and the new client sees the connection close.
    pub fn client_gone(&mut self) -> Result<(), String> {
        self.trace.event(&format!(
            "client gone: a new client is waiting; the model resets, then {} memory region(s) and the IRQ eventfds of the old client go",
            self.dma.len()
        ));
        for index in 0..self.irq_fds.len() as u32 {
            if !self.irq_fds[index as usize].is_empty() {
                ServerBackend::set_irqs(
                    self,
                    index,
                    VFIO_IRQ_SET_DATA_NONE | VFIO_IRQ_SET_ACTION_TRIGGER,
                    0,
                    0,
                    Vec::new(),
                )
                .map_err(|e| format!("IRQ index {index}: {e}"))?;
            }
        }
        self.reset_model();
        self.cfg = self.cfg_reset.clone();
        if self.guest_dma_gpa {
            if let GuestRam::Register(hm) = &self.guest_ram {
                hm.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .thaw_gpa_mappings()
                    .map_err(|e| format!("{e:#}"))?;
            }
        }
        let maps: Vec<(u64, u64)> = self.dma.iter().map(|m| (m.address, m.size)).collect();
        for (address, size) in maps {
            ServerBackend::dma_unmap(self, DmaUnmapFlags::empty(), address, size)
                .map_err(|e| format!("DMA_UNMAP {address:#x}+{size:#x}: {e}"))?;
        }
        self.sync_msi_enabled();
        self.trace
            .event("client gone: done; the device waits for the new client's messages");
        Ok(())
    }

    /// A device reset (FLR or the client's reset): the interrupt vectors raised before it
    /// and not yet delivered go first, then the model and the guest aliases.
    fn reset_model(&mut self) {
        // The display clock and the routed registers stop reaching the old engine; a tick
        // still running on it finishes (its lock) before the reset, and the old engine's FB
        // record windows go with the old FB store.
        *self.display_slot.write().unwrap_or_else(PoisonError::into_inner) = None;
        // What the old model's FB store, mirror and display did goes with it; the exit
        // summary would only show the fresh model's (C65: the software channel's costs
        // before the Windows TDR were lost with the FLR).
        let before = self.bar0.boot().map(|b| b.exec_summary()).unwrap_or_default();
        for l in before {
            self.trace.event(&format!("reset: before: {l}"));
        }
        if let Some(b) = self.bar0.boot() {
            b.ctrl.display.eng().pins.clear();
        }
        // The model first (its old interrupt sources stop with it: the mirror's host event
        // thread is joined when the old mirror drops), then what they raised goes.
        self.bar0.reset();
        if let Some(s) = &self.intr {
            let dropped = s.reset_pending();
            if dropped.iter().any(|&p| p != 0) {
                self.trace.event(&format!(
                    "reset: raised vectors dropped with the interrupt state, per leaf: {:x?}",
                    dropped
                ));
            }
        }
        self.load_guest_aliases();
        self.publish_display();
    }

    /// The display engine for the paths without the device mutex (the display clock, the
    /// routed status registers), published now and at every reset.
    pub fn display_slot(&mut self) -> DisplaySlot {
        self.publish_display();
        self.display_slot.clone()
    }

    fn publish_display(&self) {
        let e = self
            .bar0
            .boot()
            .filter(|b| b.ctrl.display.enabled)
            .map(|b| b.ctrl.display.engine.clone());
        *self.display_slot.write().unwrap_or_else(PoisonError::into_inner) = e;
    }

    /// The guest memory view for the paths without the device mutex (guestmem.rs).
    pub fn mem_view(&mut self) -> crate::guestmem::MemView {
        self.mem.view()
    }

    /// The display clock's tick (engine.rs `Ticker::set_driver`, display/fe_page.rs):
    /// advance the display engine, trace its notes. Returns whether the display interrupt
    /// line is asserted afterwards (the clock raises the vector only then).
    pub fn display_tick(&mut self) -> bool {
        let notes = match self.bar0.boot_mut() {
            Some(b) => b.display_tick(&self.mem),
            None => return false,
        };
        for n in notes {
            self.trace.event(&format!("boot: {n}"));
        }
        self.bar0.boot().is_some_and(|b| b.ctrl.display.line())
    }

    /// The BAR2 memfd's owed flush (bar2area.rs `DeferredFlush`), for the flush clock.
    pub fn bar2_flush_clock(&self) -> Option<std::sync::Arc<crate::bar2area::DeferredFlush>> {
        self.bar0.boot()?.fb.bar2_area()?.deferred_flush()
    }

    /// The flush clock's settle: a deferred flush read whose sync no hand-over ran within
    /// the bound (docs/caraxes/display-pacing.md).
    pub fn settle_bar2_flush(&mut self) {
        if let Some(b) = self.bar0.boot_mut() {
            b.settle_deferred_flush(&self.mem, crate::bar2area::Settle::Clock);
        }
    }

    /// A guest access that did not trap (an eventfd, a shared page's reactor) rings the
    /// posted doorbells first, as a trapped access does, so it does not pass a doorbell the
    /// guest wrote before it (execution.md §9.9).
    fn ring_posted_first(&mut self) {
        let gate = crate::ringgate::global_ref();
        if gate.has_posted() && !self.ringing_posted {
            self.ring_posted(gate, true);
        }
    }

    /// Ring the doorbell writes posted in `gate` (execution.md §9.9), in the guest's order,
    /// each exactly as its trapped write would have been handled (`region_write` on BAR0:
    /// the trace, the ring, the steps after a BAR0 write). `by_access`: a guest access
    /// rings them before its own work (its launch chain, if timed, stays its own), else the
    /// completer (each ring timed as a trapped ring of the launch chain from its post).
    pub fn ring_posted(&mut self, gate: &crate::ringgate::RingGate, by_access: bool) {
        if !gate.has_posted() || self.ringing_posted {
            return;
        }
        let rings = gate.take_posted();
        if rings.is_empty() {
            return;
        }
        let start = std::time::Instant::now();
        self.ringing_posted = true;
        if let Some(offset) = self.bar0.boot().map(|b| b.usermode_doorbell()) {
            for p in &rings {
                let t0 = std::time::Instant::now();
                crate::profile::record(
                    crate::profile::Key::Window("posted doorbell: posted to rung"),
                    t0.saturating_duration_since(p.at),
                    1,
                    None,
                    "",
                );
                if !by_access {
                    crate::launch::begin_trapped(p.at);
                    crate::launch::mark(crate::launch::Step::Lock);
                }
                let r = ServerBackend::region_write(
                    self,
                    VFIO_PCI_BAR0_REGION_INDEX,
                    offset,
                    &p.token.to_le_bytes(),
                );
                if !by_access {
                    crate::launch::end();
                }
                // The trapped access row keeps the posting (one BAR0 exit); the ring's device
                // time is its own row.
                crate::profile::record(
                    crate::profile::Key::Window("posted doorbell: rung (device time)"),
                    t0.elapsed(),
                    1,
                    None,
                    "",
                );
                if let Err(e) = r {
                    self.note(&format!("posted doorbell {:#x}: {e}", p.token));
                }
            }
        }
        self.ringing_posted = false;
        crate::dlock::work(
            if by_access { "guest access" } else { "posted doorbells" },
            "posted doorbell rings".into(),
            start.elapsed(),
        );
        gate.note_rung(&rings, start, by_access);
    }

    /// The completer thread (main.rs, ringgate.rs): forward the held rings that are ready,
    /// under the device lock briefly, and trace them.
    pub fn complete_held_rings(&mut self) {
        let mem = &self.mem;
        let notes = match self.bar0.boot_mut() {
            Some(b) => b.complete_held_rings(mem),
            None => return,
        };
        for n in notes {
            self.trace.event(&format!("boot: {n}"));
        }
    }

    /// The invalidate page's trigger (inv_page.rs, docs/caraxes/intr-page.md §11.3): run
    /// the invalidate as the trapped trigger write does and trace its notes.
    pub fn invalidate_from_page(
        &mut self,
        t: crate::inv_page::Trigger,
    ) -> Option<crate::invgate::Hold> {
        self.ring_posted_first();
        let mem = &self.mem;
        let (notes, hold) = match self.bar0.boot_mut() {
            Some(b) => b.mmu_invalidate_from_page(mem, t.pdb, t.upper, t.value),
            None => return None,
        };
        self.trace.event(&format!(
            "invalidate page: trigger {:#x} pdb {:#x} upper {:#x} (reactor)",
            t.value, t.pdb, t.upper
        ));
        for n in notes {
            self.trace.event(&format!("boot: {n}"));
        }
        // What every trapped BAR0 write does after it, the trigger write included: the BAR1
        // pool moves pages at this point (bar1-pool.md, P21 design H). Without it the pool
        // saw the guest's new BAR1 mappings only at the next trapped write, and the display
        // gate's KMS readback read an empty pool page (series F, G and H). The offset is the
        // trigger word's, as for the trapped write (`BootModel::pool_merge_after`).
        let offset = self.bar0.boot().map_or(0, |b| b.mmu_invalidate_register());
        // The pool finishes here, waits for the worker included, before TRIGGER can clear
        // (at once without a hold, else when the page's completion thread saw it applied).
        self.after_bar0_write_in(true, offset, crate::bar1pool::Pace::ALL);
        hold
    }

    /// The display kick page's reactor found a `PUT` change (display/chan_page.rs,
    /// docs/caraxes/intr-page.md §12.3): run the kick as the trapped write did, then the
    /// steps after a BAR0 write. Returns the channel's `GET`.
    pub fn display_kick_from_page(&mut self, base: u64, put: u32) -> Option<u32> {
        self.ring_posted_first();
        let mem = &self.mem;
        let (get, notes) = match self.bar0.boot_mut() {
            Some(b) => b.display_kick_from_page(mem, base, put),
            None => return None,
        };
        self.trace.event(&format!(
            "display kick page: PUT {put:#x} at BAR0 {base:#x} (reactor)"
        ));
        for n in notes {
            self.trace.event(&format!("boot: {n}"));
        }
        self.after_bar0_write(true, base);
        get
    }

    /// The usermode page's reactor took a doorbell (`hint`, its value) or runs its
    /// watchdog (`None`): scan the channels and ring the pending ones (um_page.rs,
    /// docs/caraxes/intr-page.md §12.2). Returns the channels rung.
    pub fn doorbell_from_page(&mut self, hint: Option<u32>) -> usize {
        self.ring_posted_first();
        let mem = &self.mem;
        let Some(boot) = self.bar0.boot_mut() else {
            return 0;
        };
        let (rung, notes) = boot.doorbell_from_page(mem, hint);
        if let Some(v) = hint {
            self.doorbell_wakeups += 1;
            self.doorbell_writes += 1;
            self.trace.event(&format!("usermode page: doorbell {v:#x} taken by the reactor"));
        } else if rung > 0 {
            self.trace.event("usermode page: watchdog found pending channel(s)");
        }
        for n in notes {
            self.trace.event(&n);
        }
        rung
    }

    pub fn summary(&self) -> String {
        let mut s = self.trace.summary();
        if self.doorbell_wakeups > 0 {
            s.push_str(&format!(
                "doorbell ioeventfd: {} guest write(s) in {} wakeup(s)\n",
                self.doorbell_writes, self.doorbell_wakeups
            ));
        }
        let posted = crate::ringgate::global().post_stats();
        if posted.writes > 0 {
            s.push_str(&posted.line());
            s.push('\n');
        }
        let (userd_posts, userd_joined) = crate::ringgate::global().userd_stats();
        if userd_posts > 0 {
            s.push_str(&format!(
                "shadow USERD poller (execution.md §9.11): {userd_posts} channel(s) found with entries not forwarded, without the device lock; {userd_joined} joined a queued ring\n"
            ));
        }
        if self.mirror_writes_unsynced > 0 || self.slow_pool_polls > 0 {
            s.push_str(&format!(
                "bar0 config-mirror writes: {} (the MSI re-arm among them), no BAR1 pool poll after them; {} pool poll(s) held the device over {} ms (docs/caraxes/sottr-stage.md §11)\n",
                self.mirror_writes_unsynced,
                self.slow_pool_polls,
                SLOW_POOL_POLL.as_millis()
            ));
        }
        if self.guest_token_rings > 0 {
            s.push_str(&format!(
                "doorbell guest tokens: {} guest write(s) in {} eventfd wakeup(s), each rung as the trapped write of its value\n",
                self.guest_token_writes, self.guest_token_rings
            ));
        }
        if self.rearm_wakeups > 0 {
            s.push_str(&format!(
                "msi re-arm ioeventfd: {} guest write(s) in {} wakeup(s)\n",
                self.rearm_writes, self.rearm_wakeups
            ));
        }
        if let (Some(_), Some(b)) = (self.rpc_head, self.bar0.boot()) {
            let r = b.rpc_eventfd_stats;
            s.push_str(&format!(
                "rpc ioeventfd: {} head write(s) in {} handling(s): {} element(s) served, {} before the link, {} queue(s) created anew; {} taken before a trapped GSP access or at the link\n",
                r.writes, self.rpc_wakeups, r.elements, r.before_link, r.recreated, r.taken_inline
            ));
        }
        let (n, first) = self.mem.misses();
        if self.mem.table().is_some() || n > 0 {
            s.push_str(&format!(
                "dma table: {} DMA address(es) outside the table refused{}\n",
                n,
                if first.is_empty() {
                    String::new()
                } else {
                    format!(
                        " (first: {})",
                        first
                            .iter()
                            .map(|a| format!("{a:#x}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                }
            ));
        }
        if let Some(b) = self.bar0.boot() {
            if let Some((base, end)) = b.layout().reservation {
                let o = &b.fb.outside;
                s.push_str(&format!(
                    "fb: reservation [{base:#x}, {end:#x}); accesses outside it: {} reads, {} writes{}\n",
                    o.reads.get(),
                    o.writes.get(),
                    if o.first.borrow().is_empty() {
                        String::new()
                    } else {
                        format!(
                            " (first: {})",
                            o.first
                                .borrow()
                                .iter()
                                .map(|(k, a, l)| format!("{k}{a:#x}+{l}"))
                                .collect::<Vec<_>>()
                                .join(" ")
                        )
                    }
                ));
            }
            for l in b.exec_summary() {
                s.push_str(&l);
                s.push('\n');
            }
            s.push_str(&b.fast().summary());
            s.push('\n');
            if let Some(l) = self.intr.as_ref().and_then(|i| i.vector_summary()) {
                s.push_str(&l);
                s.push('\n');
            }
            if let Some(l) = b.fb.bar1_pool_summary() {
                s.push_str(&l);
                s.push('\n');
            }
            for l in crate::bar1pool::replica_lines(b) {
                s.push_str(&l);
                s.push('\n');
            }
            let kinds = b.fb.pte_kinds();
            if !kinds.is_empty() {
                // D16's comptag hold, or with compression per VM slice this VM's lines
                // (compression.rs).
                let view = crate::compression::GuestView::of(b.layout(), b.chip().data());
                s.push_str(&format!(
                    "vidmem PTE kinds walked (kind:comptagline x count): {}{}\n",
                    kinds
                        .iter()
                        .map(|(k, c, n)| format!("{k:#04x}:{c:#x}x{n}"))
                        .collect::<Vec<_>>()
                        .join(" "),
                    crate::compression::pte_kinds_verdict(view.as_ref(), &kinds)
                ));
            }
        }
        s
    }

    pub fn dma_mappings(&self) -> &[DmaMapping] {
        &self.dma
    }
}

impl ServerBackend for Device {
    fn region_read(
        &mut self,
        region: u32,
        offset: u64,
        data: &mut [u8],
    ) -> Result<(), std::io::Error> {
        self.apply_registrations();
        let mut note = None;
        match region {
            VFIO_PCI_CONFIG_REGION_INDEX => self.cfg.region_read(offset as usize, data),
            VFIO_PCI_BAR0_REGION_INDEX => {
                self.poll_rpc_ioeventfd(offset);
                // A read of the invalidate trigger while the BAR1 pool has moves left from
                // the invalidate (bar1-pool.md §14): the next slice; TRIGGER reads busy
                // until the last page moved.
                let mut polled = None;
                if let Some(b) = self.bar0.boot_mut() {
                    if offset == b.mmu_invalidate_register() {
                        let t0 = std::time::Instant::now();
                        if crate::bar1pool::continue_model(b, &self.mem, crate::bar1pool::Pace::SLICED) {
                            polled = Some(t0.elapsed());
                        }
                    }
                }
                if let Some(held) = polled {
                    self.after_pool_poll("trigger read", offset, held);
                }
                self.bar0.read(&self.cfg, &self.mem, offset, data);
                note = self.bar0.name(offset);
                if self.bar0.take_captured() {
                    // Answered from the physical card's capture (bar0.rs `CapturedRegisters`).
                    let n =
                        note.map_or_else(|| "captured".to_string(), |n| format!("{n} captured"));
                    self.trace.access(Dir::Read, region, offset, data, Some(&n));
                    return Ok(());
                }
            }
            VFIO_PCI_ROM_REGION_INDEX => {
                let rom = self.bar0.rom();
                for (i, b) in data.iter_mut().enumerate() {
                    *b = rom.get(offset as usize + i).copied().unwrap_or(0xFF);
                }
            }
            1 | 3 => {
                let mem = &self.mem;
                match self.bar0.boot_mut() {
                    Some(boot) => {
                        let t = boot.bar_access(mem, region, offset, data, false);
                        self.trace.access(Dir::Read, region, offset, data, Some(&t));
                        return Ok(());
                    }
                    None => data.fill(0),
                }
            }
            _ => data.fill(0),
        }
        self.trace.access(Dir::Read, region, offset, data, note);
        Ok(())
    }

    fn region_write(
        &mut self,
        region: u32,
        offset: u64,
        data: &[u8],
    ) -> Result<(), std::io::Error> {
        self.apply_registrations();
        // A BAR0 write that lands in the config-space mirror orders no FB, BAR1 or GPU
        // state (`after_bar0_write`).
        let mut sync_point = true;
        let note = match region {
            VFIO_PCI_CONFIG_REGION_INDEX => {
                self.cfg.region_write(offset as usize, data);
                // NV208F BIF_INFO answers from the guest's Link Control (public.rs).
                let lnk = self.cfg.link_control();
                if let Some(b) = self.bar0.boot_mut() {
                    b.ctrl.link_control = lnk;
                }
                None
            }
            VFIO_PCI_BAR0_REGION_INDEX => {
                self.poll_rpc_ioeventfd(offset);
                match self.bar0.write(&mut self.cfg, &self.mem, offset, data) {
                    WriteEffect::Mirror => {
                        sync_point = false;
                        self.bar0.name(offset)
                    }
                    WriteEffect::Model => self.bar0.name(offset),
                    WriteEffect::Latched => Some(self.bar0.name(offset).unwrap_or("latched")),
                    WriteEffect::Dropped => Some("dropped"),
                }
            }
            1 | 3 => {
                let mem = &self.mem;
                match self.bar0.boot_mut() {
                    Some(boot) => {
                        let mut d = data.to_vec();
                        let t = boot.bar_access(mem, region, offset, &mut d, true);
                        self.trace
                            .access(Dir::Write, region, offset, data, Some(&t));
                        if t.contains(crate::boot::GP_PUT_HELD) {
                            // The GPPut barrier (boot.rs `gp_put_barrier`) as an event, so
                            // the event-only windows show it too.
                            self.trace
                                .event(&format!("shadow: BAR{region} write {offset:#x}: {t}"));
                        }
                        return Ok(());
                    }
                    None => Some("dropped"),
                }
            }
            _ => Some("dropped"),
        };
        self.trace.access(Dir::Write, region, offset, data, note);
        if region == VFIO_PCI_BAR0_REGION_INDEX {
            // A trapped write of the MMU invalidate registers: the guest polls the trigger
            // before it uses a new BAR1 mapping, so the pool's moves may run in slices
            // behind it (docs/caraxes/bar1-pool.md §14).
            let pace = if self.bar0.boot().is_some_and(|b| b.is_mmu_invalidate_register(offset)) {
                crate::bar1pool::Pace::SLICED
            } else {
                crate::bar1pool::Pace::NO_WAIT
            };
            self.after_bar0_write_in(sync_point, offset, pace);
        } else {
            self.after_write();
        }
        Ok(())
    }

    fn dma_map(
        &mut self,
        flags: DmaMapFlags,
        offset: u64,
        address: u64,
        size: u64,
        fd: Option<File>,
    ) -> Result<(), std::io::Error> {
        self.trace.event(&format!(
            "dma_map address={address:#x} size={size:#x} offset={offset:#x} fd={:?} flags={flags:?}",
            fd.as_ref().map(std::os::fd::AsRawFd::as_raw_fd)
        ));
        if self.guest_dma_gpa {
            let end = address.checked_add(size).ok_or_else(|| std::io::Error::from_raw_os_error(libc::EINVAL))?;
            if size == 0 || size % 4096 != 0 || address % 4096 != 0 || offset % 4096 != 0
                || offset.checked_add(size).is_none()
                || self.dma.iter().any(|m| address < m.address.saturating_add(m.size) && m.address < end)
            {
                return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
            }
            if let Some(f) = &fd {
                if offset + size > f.metadata()?.len() || offset > i64::MAX as u64 {
                    return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
                }
            }
        }
        self.apply_registrations();
        if let Some(f) = &fd {
            if let Err(e) = self.mem.map(address, size, offset, f) {
                self.trace
                    .event(&format!("dma_map address={address:#x}: mmap failed: {e}"));
                return Err(e);
            } else if self.registrar.is_some() {
                // Answered now, registered off the loop (registrar.rs).
                if let Err(e) = self.queue_guest_ram(address, size) {
                    self.mem.unmap(address, size);
                    return Err(e);
                }
            } else if let Err(e) = self.add_guest_ram(address, size) {
                // Refuse the map: the VM must not run with guest RAM the GPU cannot
                // reach or with a host short of RAM.
                let keep = matches!(&self.guest_ram,
                    GuestRam::Register(hm) if !hm.lock().unwrap().free_failures().is_empty());
                if keep {
                    self.mem.set_table(Some(DmaTable::default()));
                    self.dma.push(DmaMapping { address, size, offset, fd });
                } else {
                    self.mem.unmap(address, size);
                }
                return Err(e);
            }
        }
        self.dma.push(DmaMapping {
            address,
            size,
            offset,
            fd,
        });
        Ok(())
    }

    fn dma_unmap(
        &mut self,
        flags: DmaUnmapFlags,
        address: u64,
        size: u64,
    ) -> Result<(), std::io::Error> {
        self.trace.event(&format!(
            "dma_unmap address={address:#x} size={size:#x} flags={flags:?}"
        ));
        if self.guest_dma_gpa && !self.dma.iter().any(|m| m.address == address && m.size == size) {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
        // QEMU also reports ROM/MMIO extents without a RAM fd. Their lifecycle
        // does not change any registered pages or translation table.
        if self.dma.iter().any(|m| m.address == address && m.size == size && m.fd.is_some()) {
            self.cancel_guest_ram(address, size)?;
            self.remove_guest_ram(address, size)?;
        }
        self.dma.retain(|m| !(m.address == address && m.size == size));
        self.mem.unmap(address, size);
        Ok(())
    }

    fn reset(&mut self) -> Result<(), std::io::Error> {
        self.trace
            .event("reset: device model back to its initial state");
        self.reset_model();
        Ok(())
    }

    fn set_irqs(
        &mut self,
        index: u32,
        flags: u32,
        start: u32,
        count: u32,
        fds: Vec<File>,
    ) -> Result<(), std::io::Error> {
        self.trace.event(&format!(
            "set_irqs index={index} flags={flags:#x} start={start} count={count} fds={}",
            fds.len()
        ));
        // VFIO disables an entire IRQ index with DATA_NONE, ACTION_TRIGGER and
        // count zero. Drop both copies: the host-event thread owns a duplicate
        // of the MSI eventfd and can otherwise signal it after guest teardown.
        if flags == (VFIO_IRQ_SET_DATA_NONE | VFIO_IRQ_SET_ACTION_TRIGGER) && count == 0 {
            if start != 0 || !fds.is_empty() || index as usize >= self.irq_fds.len() {
                return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
            }
            if index == VFIO_PCI_MSI_IRQ_INDEX {
                if let Some(s) = &self.intr {
                    s.set_msi_fd(None);
                }
            }
            self.irq_fds[index as usize].clear();
        } else if flags & VFIO_IRQ_SET_DATA_EVENTFD != 0 && flags & VFIO_IRQ_SET_ACTION_TRIGGER != 0 {
            if index == VFIO_PCI_MSI_IRQ_INDEX {
                if let Some(s) = &self.intr {
                    s.set_msi_fd(fds.first().and_then(|f| f.try_clone().ok()));
                }
            }
            if let Some(slot) = self.irq_fds.get_mut(index as usize) {
                *slot = fds;
            }
        }
        self.sync_msi_enabled();
        Ok(())
    }
}

impl Device {
    /// The steps after every trapped BAR0 write (and after an MSI re-arm that came through
    /// its ioeventfd, [`Device::msi_rearm_ioeventfd`]), then `after_write`.
    ///
    /// `sync_point` is false for a write that landed in the config-space mirror: the RM's
    /// MSI re-arm (`NV_XVE_CYA_2`, once per interrupt service, 570 a second under
    /// Sunshine's capture) and PCI config writes. Such a write publishes nothing the pool or
    /// the P21 check must see (the guest orders its BAR1 changes with the BAR2 flush read
    /// and the MMU invalidate, which stay sync points), so it skips both. The pool's poll
    /// runs under the device lock, which the display clock (vblank, flip latch), the
    /// doorbell reactor and every other trapped access wait for; after the re-arms it held
    /// the lock for 2 to 50 ms at a time (docs/caraxes/sottr-stage.md §11). The poll is
    /// incremental and merges the replicas only after a flush point
    /// (`BootModel::pool_merge_after`, docs/caraxes/bar1-pool.md §11); a poll over
    /// [`SLOW_POOL_POLL`] is traced with its breakdown.
    fn after_bar0_write(&mut self, sync_point: bool, offset: u64) {
        self.after_bar0_write_in(sync_point, offset, crate::bar1pool::Pace::NO_WAIT);
    }

    /// [`Self::after_bar0_write`] at the BAR1 pool's `pace`: whether it may leave moves for
    /// the guest's reads of the invalidate trigger and how long it may wait for the shadow
    /// worker (bar1pool.rs `poll_sliced`, bar1-pool.md §14).
    fn after_bar0_write_in(&mut self, sync_point: bool, offset: u64, pace: crate::bar1pool::Pace) {
        if sync_point {
            // P21 design H: the guest flushes and invalidates through BAR0 after changing a
            // BAR1 mapping and before it uses the mapping; the same point moves pages into
            // and out of the BAR1 pool.
            if let Some(b) = self.bar0.boot_mut() {
                if b.fb.bar1_pool().is_some() {
                    let merge = b.pool_merge_after(offset);
                    // Nothing due and no merge point: the poll would only find that out
                    // (docs/caraxes/bar1-pool.md §16; SotTR run E: 889501 polls, 87 us each).
                    let now = std::time::Instant::now();
                    let skip = !merge && {
                        let mut g = b.fb.pool_mut();
                        match g.as_mut() {
                            Some(p) if !p.work_due(now) => {
                                p.note_poll_skipped();
                                true
                            }
                            _ => false,
                        }
                    };
                    if skip {
                        self.after_write();
                        return;
                    }
                    let t0 = std::time::Instant::now();
                    crate::bar1pool::poll_model(b, &self.mem, merge, pace);
                    crate::dlock::work("BAR0 write", "BAR1 pool poll".into(), t0.elapsed());
                    self.after_pool_poll("write", offset, t0.elapsed());
                }
            }
        } else {
            self.mirror_writes_unsynced += 1;
        }
        self.after_write();
    }

    /// The churn line and the slow-poll event after a BAR1 pool poll that held the device
    /// `held` (after a BAR0 `what` at `offset`).
    fn after_pool_poll(&mut self, what: &str, offset: u64, held: std::time::Duration) {
        let Some(b) = self.bar0.boot_mut() else { return };
        // A batch still running keeps the TRIGGER reads on the locked path, which continues it.
        b.fast().set_pool_busy(b.fb.pool_mut().as_ref().is_some_and(|p| p.batch_pending()));
        // The guest's BAR1 mapping churn, once per second with activity
        // (docs/caraxes/bar1-pool.md §13).
        let churn = b.fb.pool_mut().as_mut().and_then(|p| p.take_churn());
        if let Some(line) = churn {
            self.trace.event(&line);
        }
        if held > SLOW_POOL_POLL {
            self.slow_pool_polls += 1;
            let bd = b
                .fb
                .pool_mut()
                .as_ref()
                .map(|p| p.last.to_string())
                .unwrap_or_default();
            self.trace.event(&format!(
                "bar1 pool: the poll after a BAR0 {what} at {offset:#x} held the device {} us: {bd} (sottr-stage.md §11, bar1-pool.md §11)",
                held.as_micros()
            ));
        }
    }

    /// The steps after every trapped write of BAR0 or config space: the model's notes, a
    /// function-level reset, a pending MSI, the MSI enable.
    ///
    /// The notes go out with one flush: a boot-model write drains the host event thread's
    /// queued lines (`IntrShared::notes`, up to about 190 in one 60 Hz window of vkmark),
    /// and one flush per line was one write syscall per line on the guest's trap
    /// (docs/research/vkmark-frame-2026-09-28.md, trace artefact).
    fn after_write(&mut self) {
        let notes: Vec<String> = self
            .bar0
            .take_notes()
            .into_iter()
            .map(|n| format!("boot: {n}"))
            .collect();
        self.trace.events(&notes);
        if self.cfg.take_flr() {
            self.trace
                .event("flr: function-level reset, device model back to its initial state");
            self.reset_model();
        }
        if self.bar0.boot_mut().is_some_and(|b| b.take_msi()) {
            self.raise_msi();
        }
        self.sync_msi_enabled();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guest_ram_file() -> File {
        let name = std::ffi::CString::new("caraxes-gpa-test").unwrap();
        let fd = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC) };
        assert!(fd >= 0);
        let f = unsafe { File::from_raw_fd(fd) };
        f.set_len(32 << 20).unwrap();
        f
    }

    #[test]
    fn qemu_page_fragments_unmap_remap_and_reject_overlaps() {
        let mut d = rtx2070_device().with_guest_ram(GuestRam::Identity, None);
        d.guest_dma_gpa = true;
        let f = guest_ram_file();
        let map = |d: &mut Device, at, size| d.dma_map(DmaMapFlags::READ_WRITE,
            at, at, size, Some(f.try_clone().unwrap()));
        map(&mut d, 0, 0xa0000).unwrap();
        map(&mut d, 0x100000, 0x1f00000).unwrap();
        assert!(d.mem.covers(0x9f000, 0x1000));
        assert!(!d.mem.covers(0x9f000, 0x2000));
        assert!(!d.mem.covers(0xa0000, 1));
        d.dma_map(DmaMapFlags::READ, 0, 0xc0000, 0x20000, None).unwrap();
        assert!(!d.mem.covers(0xc0000, 1));
        d.dma_unmap(DmaUnmapFlags::empty(), 0xc0000, 0x20000).unwrap();
        assert!(map(&mut d, 0x90000, 0x20000).is_err());
        // Only an exact DMA_MAP can be retired. A partial unmap must not leave
        // published translations to pages that the server unmapped.
        assert!(d.dma_unmap(DmaUnmapFlags::empty(), 0, 0x30000).is_err());
        assert!(d.mem.covers(0x90000, 0x1000));
        d.dma_unmap(DmaUnmapFlags::empty(), 0, 0xa0000).unwrap();
        assert!(!d.mem.covers(0, 1));
        map(&mut d, 0, 0x30000).unwrap();
        map(&mut d, 0x50000, 0x50000).unwrap();
        assert!(!d.mem.covers(0x30000, 1));
        assert!(d.mem.covers(0x50000, 1));
        d.dma_unmap(DmaUnmapFlags::empty(), 0, 0x30000).unwrap();
        d.dma_unmap(DmaUnmapFlags::empty(), 0x50000, 0x50000).unwrap();
        d.dma_unmap(DmaUnmapFlags::empty(), 0x100000, 0x1f00000).unwrap();
        assert!(d.mem.table().unwrap().is_empty());
        assert_eq!(d.mem.host_iova(0, 1), None);
    }

    #[test]
    fn failed_gpa_publication_keeps_the_previous_mapping() {
        let mut d = rtx2070_device().with_guest_ram(GuestRam::Identity, None);
        d.guest_dma_gpa = true;
        let f = guest_ram_file();
        d.dma_map(DmaMapFlags::READ_WRITE, 0, 0, 0xa0000, Some(f.try_clone().unwrap())).unwrap();
        d.dma_ranges = Some(PathBuf::from("/dev/null/dma-ranges"));
        assert!(d.dma_map(DmaMapFlags::READ_WRITE, 0x100000, 0x100000, 0x1000, Some(f)).is_err());
        assert_eq!(d.dma.len(), 1);
        assert_eq!(d.ranges.len(), 1);
        assert!(d.mem.covers(0, 0xa0000));
        assert!(!d.mem.covers(0x100000, 1));
        assert_eq!(d.mem.regions(), vec![(0, 0xa0000)]);
    }

    /// A new client after a disconnect (Cloud Hypervisor's reboot, vfio-user.rst "Client
    /// Disconnection"): the old client's memory regions and IRQ eventfds are gone, the
    /// configuration space is back at its reset state, and the new client maps again.
    #[test]
    fn a_new_client_finds_the_old_clients_memory_and_eventfds_gone() {
        use std::os::fd::FromRawFd;
        let mut d = rtx2070_device().with_guest_ram(GuestRam::Identity, None);
        d.guest_dma_gpa = true;
        let f = guest_ram_file();
        let map = |d: &mut Device, at, size| d.dma_map(DmaMapFlags::READ_WRITE,
            at, at, size, Some(f.try_clone().unwrap()));
        map(&mut d, 0, 0xa0000).unwrap();
        map(&mut d, 0x100000, 0x1f00000).unwrap();
        let fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        assert!(fd >= 0);
        let ev = unsafe { File::from_raw_fd(fd) };
        d.set_irqs(VFIO_PCI_MSI_IRQ_INDEX,
            VFIO_IRQ_SET_DATA_EVENTFD | VFIO_IRQ_SET_ACTION_TRIGGER,
            0, 1, vec![ev.try_clone().unwrap()]).unwrap();
        // The old guest enabled memory decoding and bus mastering.
        let reset = d.cfg.dword(4);
        d.cfg.write(4, &[0x06, 0x00]);
        assert_ne!(d.cfg.dword(4), reset);
        d.client_gone().unwrap();
        assert!(d.dma.is_empty());
        assert!(!d.mem.covers(0, 1));
        assert!(d.mem.table().unwrap().is_empty());
        assert!(d.irq_fds.iter().all(|f| f.is_empty()));
        assert_eq!(d.cfg.dword(4), reset);
        map(&mut d, 0, 0xa0000).unwrap();
        map(&mut d, 0x100000, 0x1f00000).unwrap();
        assert!(d.mem.covers(0x100000, 0x1f00000));
    }

    #[test]
    fn disabling_msi_retires_both_eventfd_copies_and_can_be_rearmed() {
        use std::os::fd::{AsRawFd, FromRawFd};
        let intr = crate::intr::IntrShared::new();
        let mut d = rtx2070_device().with_intr_shared(intr.clone());
        let fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        assert!(fd >= 0);
        let reader = unsafe { File::from_raw_fd(fd) };
        let arm = |d: &mut Device| {
            d.set_irqs(VFIO_PCI_MSI_IRQ_INDEX,
                VFIO_IRQ_SET_DATA_EVENTFD | VFIO_IRQ_SET_ACTION_TRIGGER,
                0, 1, vec![reader.try_clone().unwrap()]).unwrap();
        };
        arm(&mut d);
        d.set_irqs(VFIO_PCI_MSI_IRQ_INDEX,
            VFIO_IRQ_SET_DATA_NONE | VFIO_IRQ_SET_ACTION_TRIGGER,
            0, 0, vec![]).unwrap();
        assert!(d.irq_fds[VFIO_PCI_MSI_IRQ_INDEX as usize].is_empty());
        // Even if the PCI MSI enable bit remains set, the retired fd is silent.
        intr.set_msi_enabled(true);
        assert!(!intr.signal_msi());
        let mut value = 0u64;
        assert_eq!(unsafe { libc::read(reader.as_raw_fd(), (&mut value as *mut u64).cast(), 8) }, -1);
        assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(libc::EAGAIN));
        arm(&mut d);
        intr.set_msi_enabled(true);
        assert!(intr.signal_msi());
        assert_eq!(unsafe { libc::read(reader.as_raw_fd(), (&mut value as *mut u64).cast(), 8) }, 8);
        assert_eq!(value, 1);
    }

    fn rtx2070_device() -> Device {
        let dump = include_str!("../../../caraxes/reference/rtx2070/config-space.txt");
        let board = crate::board::Board::load("rtx2070").unwrap();
        let chip = crate::chip::by_name(&board.chip).unwrap();
        let cfg = ConfigSpace::from_dump(
            crate::config::parse_dump(dump).unwrap(),
            board.bars,
            board.rom_bar_size,
        )
        .unwrap();
        let mut rom = vec![0x55u8, 0xAA, 0x76];
        rom.resize(512, 0);
        Device::new(cfg, Bar0::new(chip, rom, None), Trace::new(None).unwrap())
    }

    /// The MSI re-arm is not a BAR1 pool sync point (sottr-stage.md §11): neither the
    /// trapped write nor the ioeventfd path runs the pool's poll, a BAR0 write the model
    /// takes does, and the exit summary counts the writes that skipped it.
    #[test]
    fn a_config_mirror_write_does_not_poll_the_bar1_pool() {
        const MIB: u64 = 1 << 20;
        let (base, end) = (0x400_0000, 0x800_0000);
        let l = crate::fb::FbLayout::reserved(32 * MIB, 256 * MIB, base, end)
            .unwrap()
            .with_pool(4 * MIB)
            .unwrap();
        let f = crate::fixture::rtx2070();
        let dump = include_str!("../../../caraxes/reference/rtx2070/config-space.txt");
        let board = crate::board::Board::load("rtx2070").unwrap();
        let cfg = ConfigSpace::from_dump(
            crate::config::parse_dump(dump).unwrap(),
            board.bars,
            board.rom_bar_size,
        )
        .unwrap();
        let bar0 = Bar0::new(f.chip, vec![0x55, 0xAA, 0x76], None).with_boot_model(
            crate::boot::BootModel::new(f.chip, l, f.identity(256 * MIB)),
        );
        let mut d = Device::new(cfg, bar0, Trace::new(None).unwrap());
        let (_, root) = l.bar_roots().unwrap();
        let (pool0, _) = l.pool_range().unwrap();
        d.bar0
            .boot_mut()
            .unwrap()
            .fb
            .set_bar1_pool(root, pool0, 4 * MIB, l.usable().unwrap());
        let polls = |d: &mut Device| {
            let s = d.bar0.boot_mut().unwrap().fb.bar1_pool_summary().unwrap();
            s.split("); ").nth(1).unwrap().split(' ').next().unwrap().parse::<u64>().unwrap()
        };
        assert_eq!(polls(&mut d), 0);
        let at = f.chip.data().bar0.pcfg + NV_XVE_CYA_2;
        d.region_write(VFIO_PCI_BAR0_REGION_INDEX, at, &[0; 4]).unwrap();
        d.msi_rearm_ioeventfd(at, 3);
        assert_eq!(polls(&mut d), 0, "the re-arm is no sync point");
        // The guest RM's MMU invalidate PDB register: a model write, a sync point.
        d.region_write(VFIO_PCI_BAR0_REGION_INDEX, 0xb8_30a0, &[0; 4]).unwrap();
        assert_eq!(polls(&mut d), 1);
        assert_eq!(d.mirror_writes_unsynced, 2);
        assert!(d
            .summary()
            .contains("bar0 config-mirror writes: 2 (the MSI re-arm among them), no BAR1 pool poll after them"));
    }

    /// The BAR1 pool's poll after a trapped BAR0 write merges the replicas only at a display
    /// channel's `PUT` (bar1-pool.md §16): the other engine starts merge at their own
    /// hand-over (doorbell, GSP command queue, BAR2 bind, flush clock), and nothing else
    /// merges: not the MMU invalidate's registers (the trigger merges the table replicas),
    /// the doorbell, PTIMER, the GSP registers, the L2 and FB flushes, the display's
    /// interrupt acks.
    #[test]
    fn the_pool_poll_merges_only_at_a_display_kick() {
        let f = crate::fixture::rtx2070();
        let b = crate::boot::BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(4 << 30),
            f.identity(4 << 30),
        );
        let q = b.gsp_queue_registers().expect("TU106 has the queue registers");
        assert_eq!((q.cmd_head, q.msg_tail), (0x11_0c00, 0x11_0c84));
        for off in [
            0xb8_30a0u64, 0xb8_30a4, 0xb8_30b0, 0xbb_0090, 0x9400, 0x9410, 0x11_0c00, 0x11_0c84,
            0x7_0010, 0x7_0000, 0xb8_0f00, 0x11_0c80, 0x11_0c04, 0x61_180c, 0x61_1858, 0x70_c000,
        ] {
            assert!(!b.pool_merge_after(off), "{off:#x}");
        }
        for off in [0x68_0000u64, 0x69_6000, 0x6d_8000, 0x6d_ffff] {
            assert!(b.pool_merge_after(off), "{off:#x}");
        }
        assert!(!b.pool_merge_after(0x6e_0000));
    }

    /// The MSI re-arm ioeventfd (intr-page.md §4): 4 bytes at `NV_PCFG` + `NV_XVE_CYA_2`
    /// without datamatch, accepted by the server beside the token pool's sub-regions; the
    /// token reactor hands its wakeups to `msi_rearm_ioeventfd` and never to a token, and a
    /// re-arm through it leaves config space as the trapped write does.
    /// A trapped doorbell write is posted (execution.md §9.9): answered while another thread
    /// holds the device lock, and rung as the trapped write by the next trapped access
    /// before its own work, or by the completer. A second write of a queued token joins it.
    #[test]
    fn a_trapped_doorbell_is_posted_and_rung_before_the_next_access() {
        const MIB: u64 = 1 << 20;
        let l = crate::fb::FbLayout::reserved(32 * MIB, 256 * MIB, 0x400_0000, 0x800_0000).unwrap();
        let f = crate::fixture::rtx2070();
        let mut d = rtx2070_device();
        d.bar0 = Bar0::new(f.chip, vec![0x55, 0xAA, 0x76], None).with_boot_model(
            crate::boot::BootModel::new(f.chip, l, f.identity(256 * MIB)),
        );
        // A pool token counts its trapped rings (stale here: no channel holds it).
        d.bar0.boot_mut().unwrap().ctrl.channels.tokens = Some(crate::tokens::TokenPool::new(4));
        let rung = |d: &Arc<Mutex<Device>>| {
            lock(d).bar0.boot_mut().unwrap().token_pool_mut().unwrap().stats.trapped_pool_rings
        };
        let doorbell = d.bar0.boot().unwrap().usermode_doorbell();
        let device = Arc::new(Mutex::new(d));
        let gate = Arc::new(crate::ringgate::RingGate::default());
        let mut shared = SharedDevice::with_gate(device.clone(), Routes::default(), gate.clone());
        assert_eq!(shared.doorbell, Some(doorbell));
        let token = crate::tokens::TOKEN_BASE.to_le_bytes();
        // The device lock is held elsewhere: the write is answered anyway.
        let held = lock(&device);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            shared.region_write(VFIO_PCI_BAR0_REGION_INDEX, doorbell, &token).unwrap();
            tx.send(shared).unwrap();
        });
        let mut shared = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the posted doorbell waited for the device lock");
        assert!(gate.has_posted());
        drop(held);
        assert_eq!(rung(&device), 0, "not rung yet");
        // The next trapped access rings it first.
        let mut boot0 = [0u8; 4];
        shared.region_read(VFIO_PCI_BAR0_REGION_INDEX, 0, &mut boot0).unwrap();
        assert!(!gate.has_posted());
        assert_eq!(rung(&device), 1, "rung as the trapped write");
        let s = gate.post_stats();
        assert_eq!((s.writes, s.by_access, s.by_completer), (1, 1, 0));
        // Two writes of one token before the completer: one ring.
        shared.region_write(VFIO_PCI_BAR0_REGION_INDEX, doorbell, &token).unwrap();
        shared.region_write(VFIO_PCI_BAR0_REGION_INDEX, doorbell, &token).unwrap();
        lock(&device).ring_posted(&gate, false);
        assert_eq!(rung(&device), 2);
        let s = gate.post_stats();
        assert_eq!((s.writes, s.coalesced, s.by_access, s.by_completer), (3, 1, 1, 1));
        // Another register's write is not posted.
        shared.region_write(VFIO_PCI_BAR0_REGION_INDEX, 0xb8_30a0, &[0; 4]).unwrap();
        assert!(!gate.has_posted());
        assert_eq!(gate.post_stats().writes, 3);
    }

    #[test]
    fn the_msi_rearm_ioeventfd_is_read_by_the_token_reactor() {
        use std::io::Write;
        let board = crate::board::Board::load("rtx2070").unwrap();
        let chip = crate::chip::by_name(&board.chip).unwrap();
        let at = chip.data().bar0.pcfg + NV_XVE_CYA_2;
        assert_eq!(at, 0x88704);
        let (rearm, rearm_reader) = msi_rearm_sub_region(at).unwrap();
        assert_eq!((rearm.offset, rearm.size, rearm.flags, rearm.datamatch), (at, 4, 0, 0));
        let rearm_writer = File::from(rearm.fd.try_clone().unwrap());
        let subs = doorbell_token_sub_regions(0xbb_0090, 2).unwrap();
        let path = std::env::temp_dir().join(format!("nvgpu-rearm-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut server = vfio_user::Server::new(&path, false, irqs(), regions(&board)).unwrap();
        let mut token_writers = Vec::new();
        let mut token_readers = Vec::new();
        for (s, r) in subs {
            token_writers.push(File::from(s.fd.try_clone().unwrap()));
            token_readers.push(r);
            server.add_ioeventfd(VFIO_PCI_BAR0_REGION_INDEX, s).unwrap();
        }
        server.add_ioeventfd(VFIO_PCI_BAR0_REGION_INDEX, rearm).unwrap();
        // The command-queue head (CARAXES_RPC_IOEVENTFD=1) the same way: one more entry
        // without datamatch, one more reader.
        let (rpc, rpc_reader) = rpc_head_sub_region(0x11_0C00).unwrap();
        assert_eq!((rpc.offset, rpc.size, rpc.flags, rpc.datamatch), (0x11_0C00, 4, 0, 0));
        let rpc_writer = File::from(rpc.fd.try_clone().unwrap());
        server.add_ioeventfd(VFIO_PCI_BAR0_REGION_INDEX, rpc).unwrap();
        drop(server);
        let _ = std::fs::remove_file(&path);

        // A trapped re-arm and one through the eventfd leave the same config space.
        let mut trapped = rtx2070_device();
        let before: Vec<u32> = (0..0x1000).step_by(4).map(|o| trapped.cfg.dword(o)).collect();
        trapped.region_write(VFIO_PCI_BAR0_REGION_INDEX, at, &[0; 4]).unwrap();
        let after: Vec<u32> = (0..0x1000).step_by(4).map(|o| trapped.cfg.dword(o)).collect();
        assert_eq!(before, after);

        // Written before the reactor starts, so both re-arms are one wakeup.
        (&rearm_writer).write_all(&1u64.to_ne_bytes()).unwrap();
        (&rearm_writer).write_all(&1u64.to_ne_bytes()).unwrap();
        (&token_writers[1]).write_all(&1u64.to_ne_bytes()).unwrap();
        (&rpc_writer).write_all(&1u64.to_ne_bytes()).unwrap();
        let device = Arc::new(Mutex::new(rtx2070_device()));
        lock(&device).set_rpc_ioeventfd(0x11_0C00);
        let reactor = crate::poll::Reactor::new("test", crate::poll::Config::default(), None).unwrap();
        let stats = spawn_token_reader_hooked(
            device.clone(),
            0xbb_0090,
            token_readers,
            Vec::new(),
            reactor,
            None,
            Some((at, rearm_reader)),
            Some((0x11_0C00, rpc_reader)),
            Routes::default(),
        )
        .unwrap();
        let t0 = std::time::Instant::now();
        while stats.events.load(std::sync::atomic::Ordering::Relaxed) < 3 {
            assert!(t0.elapsed() < std::time::Duration::from_secs(5), "reactor did not read");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        stats.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let d = lock(&device);
        assert_eq!((d.rearm_wakeups, d.rearm_writes), (1, 2));
        assert_eq!(d.rpc_wakeups, 1, "the head eventfd went to rpc_ioeventfd");
        let cfg: Vec<u32> = (0..0x1000).step_by(4).map(|o| d.cfg.dword(o)).collect();
        assert_eq!(cfg, before);
        assert!(d.summary().contains("msi re-arm ioeventfd: 2 guest write(s) in 1 wakeup(s)"));
    }

    /// The guest-token sub-regions (CARAXES_DOORBELL_GUEST_TOKENS): one datamatch entry and
    /// one eventfd per value, in the order given; the reactor hands a wakeup of one to
    /// `doorbell_guest_tokens` with its value, beside the pool's slots.
    /// Trapped accesses of the interrupt page and the MSI re-arm are served while the
    /// device mutex is held elsewhere (display-own-clock.md §2.4): the tree's stores apply
    /// (exact mode), reads come from the page, the re-arm resends the MSI of a pending
    /// vector; other offsets and the tagged page's re-arm still go to the device.
    #[test]
    fn routed_intr_page_accesses_do_not_need_the_device_mutex() {
        use std::io::Read;
        let device = Arc::new(Mutex::new(rtx2070_device()));
        let shared = crate::intr::IntrShared::new();
        let (msi_w, mut msi_r) = {
            // SAFETY: a plain syscall; checked.
            let raw = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
            assert!(raw >= 0);
            // SAFETY: a new descriptor.
            let f = unsafe { File::from_raw_fd(raw) };
            (f.try_clone().unwrap(), f)
        };
        shared.set_msi_fd(Some(msi_w));
        shared.set_msi_enabled(true);
        let tree = 0x00B8_1000;
        let page = crate::intr_page::IntrPage::create(
            16 << 20,
            tree,
            crate::fixture::rtx2070().chip.data().identity,
            shared.clone(),
            crate::intr_page::Config::default(),
        )
        .unwrap();
        let (_hook, wake) = crate::intr_page::PageHook::new(page.clone()).unwrap();
        shared.attach_page(wake);
        let rearm = 0x88704;
        let routes = Routes { intr_page: Some(page.clone()), rearm: Some(rearm), ..Routes::default() };
        let held = lock(&device);
        // Tagged page: the re-arm is not routed.
        assert!(!routes.write(0, rearm, &0u32.to_le_bytes()));
        // A tree store: exact mode, applied.
        assert!(routes.write(0, tree + 0x200, &0x10u32.to_le_bytes()));
        assert!(routes.write(0, tree + 0x608, &1u32.to_le_bytes()));
        assert!(page.exact());
        let mut b = [0u8; 4];
        assert!(routes.read(0, tree + 0x400, &mut b));
        assert_eq!(u32::from_le_bytes(b), 0x10, "LEAF_EN_CLEAR reads the enables");
        assert!(shared.raise(4));
        page.scan();
        let mut c = [0u8; 8];
        msi_r.read_exact(&mut c).unwrap();
        // The re-arm while vector 4 is pending: routed, the MSI again.
        assert!(routes.write(0, rearm, &0u32.to_le_bytes()));
        msi_r.read_exact(&mut c).unwrap();
        // Not the page's: the device.
        assert!(!routes.write(0, 0x10_0000, &0u32.to_le_bytes()));
        assert!(!routes.read(0, 0x10_0000, &mut b));
        assert!(!routes.write(1, tree, &0u32.to_le_bytes()), "BAR0 only");
        drop(held);
        assert!(page.summary().contains("served without the device mutex"));
    }

    /// The display's status registers are served under the display lock only: a guest
    /// read of the dispatch and a write of an enable complete while the device mutex is
    /// held (display/core_path.rs), and the engine counts them.
    #[test]
    fn routed_display_registers_do_not_need_the_device_mutex() {
        use crate::display::engine::{self, Engine, FE_RM_INTR_DISPATCH, FE_RM_INTR_EN_HEAD_TIMING};
        let device = Arc::new(Mutex::new(rtx2070_device()));
        let e = Arc::new(Mutex::new(Engine::new(4, 8)));
        let mut mem = GuestMemory::new();
        let routes = Routes {
            display: Some(DisplayRoute {
                slot: Arc::new(std::sync::RwLock::new(Some(e.clone()))),
                mem: mem.view(),
            }),
            ..Routes::default()
        };
        let held = lock(&device);
        let mut b = [0u8; 4];
        assert!(routes.read(0, FE_RM_INTR_DISPATCH, &mut b));
        assert_eq!(u32::from_le_bytes(b), 0);
        assert!(routes.write(0, FE_RM_INTR_EN_HEAD_TIMING, &engine::LAST_DATA.to_le_bytes()));
        assert!(routes.read(0, FE_RM_INTR_EN_HEAD_TIMING, &mut b));
        assert_eq!(u32::from_le_bytes(b), engine::LAST_DATA);
        // Not an engine register, or not a whole dword: the device.
        assert!(!routes.read(0, 0x61_0000, &mut b));
        assert!(!routes.read(0, FE_RM_INTR_DISPATCH, &mut b[..2]));
        drop(held);
        let e = engine::lock(&e);
        assert_eq!((e.routed_reads, e.routed_writes), (2, 1));
        assert!(e.notes.iter().any(|n| n.contains("vblank (LAST_DATA) interrupt enabled")), "{:?}", e.notes);
    }

    #[test]
    fn guest_token_eventfds_are_datamatch_entries_read_by_the_token_reactor() {
        use std::io::Write;
        let t = crate::tokens::GuestTokens { hot: vec![0], rest: vec![1], chids: 2 };
        let values = t.values();
        assert_eq!(values, vec![0x0, 0x1, 0x10000, 0x10001]);
        let subs = doorbell_value_sub_regions(0xbb_0090, &values).unwrap();
        let mut fds = std::collections::BTreeSet::new();
        for ((s, _), v) in subs.iter().zip(&values) {
            assert_eq!((s.offset, s.size, s.flags, s.datamatch), (0xbb_0090, 4, vfio_user::IOEVENTFD_FLAG_DATAMATCH, u64::from(*v)));
            fds.insert(std::os::fd::AsRawFd::as_raw_fd(&*s.fd));
        }
        assert_eq!(fds.len(), values.len(), "one eventfd per value: the eventfd names the value");
        let (pool_sub, pool_reader) = doorbell_token_sub_regions(0xbb_0090, 1).unwrap().pop().unwrap();
        let pool_writer = File::from(pool_sub.fd.try_clone().unwrap());
        let mut writers = Vec::new();
        let mut guest = Vec::new();
        for ((s, r), v) in subs.into_iter().zip(values) {
            writers.push(File::from(s.fd.try_clone().unwrap()));
            guest.push((v, r));
        }
        (&writers[2]).write_all(&3u64.to_ne_bytes()).unwrap();
        (&pool_writer).write_all(&1u64.to_ne_bytes()).unwrap();
        let device = Arc::new(Mutex::new(rtx2070_device()));
        let reactor = crate::poll::Reactor::new("test", crate::poll::Config::default(), None).unwrap();
        let stats = spawn_token_reader_hooked(
            device.clone(),
            0xbb_0090,
            vec![pool_reader],
            guest,
            reactor,
            None,
            None,
            None,
            Routes::default(),
        )
        .unwrap();
        let t0 = std::time::Instant::now();
        while lock(&device).guest_token_rings == 0 {
            assert!(t0.elapsed() < std::time::Duration::from_secs(5), "reactor did not read");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        stats.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let d = lock(&device);
        assert_eq!((d.guest_token_rings, d.guest_token_writes), (1, 3));
        assert!(d.summary().contains("doorbell guest tokens: 3 guest write(s) in 1 eventfd wakeup(s)"));
    }

    /// The invalidate page's trigger does what the trapped trigger write does: the same
    /// registers afterwards, TRIGGER reading back clear.
    #[test]
    fn an_invalidate_from_the_page_matches_the_trapped_write() {
        let with_boot = || {
            let mut d = rtx2070_device();
            let f = crate::fixture::rtx2070();
            let bar0 = std::mem::replace(&mut d.bar0, Bar0::new(f.chip, vec![0x55, 0xAA], None));
            d.bar0 = bar0.with_boot_model(crate::boot::BootModel::new(
                f.chip,
                crate::fb::FbLayout::plain(4 << 30),
                f.identity(4 << 30),
            ));
            d
        };
        let mut trapped = with_boot();
        for (off, v) in [(0xB8_30A0u64, 0x1e75520u32), (0xB8_30A4, 0), (0xB8_30B0, 0x8000_00c1)] {
            trapped
                .region_write(VFIO_PCI_BAR0_REGION_INDEX, off, &v.to_le_bytes())
                .unwrap();
        }
        let mut paged = with_boot();
        paged.invalidate_from_page(crate::inv_page::Trigger {
            pdb: 0x1e75520,
            upper: 0,
            value: 0x8000_00c1,
        });
        for off in [0xB8_30A0u64, 0xB8_30A4, 0xB8_30B0] {
            let a = trapped.bar0.boot_mut().unwrap().read32(off);
            let b = paged.bar0.boot_mut().unwrap().read32(off);
            assert_eq!(a, b, "{off:#x}");
        }
        assert_eq!(paged.bar0.boot_mut().unwrap().read32(0xB8_30B0), Some(0xc1));
    }

    /// The pool's sub-regions: one per token at the doorbell, size 4, DATAMATCH with the
    /// token, each with its own eventfd; the vendored server accepts all of them (no two
    /// collide for KVM) and refuses a token-less entry next to them.
    #[test]
    fn token_sub_regions_one_eventfd_per_token() {
        use std::io::Read;
        use std::os::fd::AsRawFd;
        let subs = doorbell_token_sub_regions(0xbb_0090, 8).unwrap();
        assert_eq!(subs.len(), 8);
        let mut fds = std::collections::BTreeSet::new();
        for (slot, (s, reader)) in subs.iter().enumerate() {
            assert_eq!((s.offset, s.size), (0xbb_0090, 4));
            assert_eq!(s.flags, vfio_user::IOEVENTFD_FLAG_DATAMATCH);
            let token = crate::tokens::TokenPool::token(slot);
            assert_eq!(s.datamatch, u64::from(token));
            assert!(fds.insert(s.fd.as_raw_fd()));
            // The reader is the same eventfd.
            use std::io::Write;
            (&File::from(s.fd.try_clone().unwrap()))
                .write_all(&1u64.to_ne_bytes())
                .unwrap();
            let mut c = [0u8; 8];
            (&*reader).read_exact(&mut c).unwrap();
            assert_eq!(u64::from_ne_bytes(c), 1);
        }
        let board = crate::board::Board::load("rtx2070").unwrap();
        let path = std::env::temp_dir().join(format!("nvgpu-tokens-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut server = vfio_user::Server::new(&path, false, irqs(), regions(&board)).unwrap();
        for (s, _) in subs {
            server.add_ioeventfd(VFIO_PCI_BAR0_REGION_INDEX, s).unwrap();
        }
        let (wildcard, _) = doorbell_sub_region(0xbb_0090).unwrap();
        assert!(server
            .add_ioeventfd(VFIO_PCI_BAR0_REGION_INDEX, wildcard)
            .is_err());
        drop(server);
        let _ = std::fs::remove_file(&path);
    }

    /// The interrupt page's areas: region 0 mmap-able at file offset 0 over exactly the
    /// two pages, next to the doorbell's ioeventfd sub-regions, which the server still
    /// accepts; every other region as before.
    #[test]
    fn bar0_areas_make_region_0_mmapable_over_two_pages() {
        let board = crate::board::Board::load("rtx2070").unwrap();
        let plain = regions(&board);
        let areas = Bar0Areas {
            fd: 9,
            areas: vec![(0, 0x1000), (0xb8_1000, 0x1000)],
        };
        let r = regions_with_pages(&board, None, None, Some(areas.clone()));
        let b0 = &r[VFIO_PCI_BAR0_REGION_INDEX as usize];
        assert_eq!(b0.mmap_fd, Some(9));
        assert_ne!(b0.region_info.flags & VFIO_REGION_INFO_FLAG_MMAP, 0);
        assert_ne!(b0.region_info.flags & VFIO_REGION_INFO_FLAG_WRITE, 0);
        assert_eq!(b0.region_info.offset, 0);
        assert_eq!(b0.region_info.size, 16 << 20);
        let got: Vec<(u64, u64)> = b0
            .sparse_areas
            .iter()
            .map(|a| (a.area.offset, a.area.size))
            .collect();
        assert_eq!(got, vec![(0, 0x1000), (0xb8_1000, 0x1000)]);
        for i in 1..r.len() {
            assert_eq!(r[i].region_info.flags, plain[i].region_info.flags);
            assert!(r[i].mmap_fd.is_none());
        }
        // Areas outside the BAR are not offered.
        let bad = Bar0Areas {
            fd: 9,
            areas: vec![(0, 0x1000), (16 << 20, 0x1000)],
        };
        let r = regions_with_pages(&board, None, None, Some(bad));
        assert!(r[0].mmap_fd.is_none());
        // The doorbell's sub-regions still declare next to the areas.
        let path = std::env::temp_dir().join(format!("nvgpu-intrpage-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let r = regions_with_pages(&board, None, None, Some(areas));
        let mut server = vfio_user::Server::new(&path, false, irqs(), r).unwrap();
        for (s, _) in doorbell_token_sub_regions(0xbb_0090, 4).unwrap() {
            server.add_ioeventfd(VFIO_PCI_BAR0_REGION_INDEX, s).unwrap();
        }
        drop(server);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn bar1_area_makes_region_1_mmapable_at_offset_zero() {
        let board = crate::board::Board::load("rtx2070").unwrap();
        let plain = regions(&board);
        assert!(plain
            .iter()
            .all(|r| r.mmap_fd.is_none() && r.sparse_areas.is_empty()));
        assert_eq!(plain[1].region_info.flags & VFIO_REGION_INFO_FLAG_MMAP, 0);

        let area = |len| Some(Bar1Area { fd: 7, len });
        let r = regions_with(&board, area(2 << 20));
        let b1 = &r[1];
        assert_eq!(b1.mmap_fd, Some(7));
        assert_ne!(b1.region_info.flags & VFIO_REGION_INFO_FLAG_MMAP, 0);
        assert_eq!(b1.region_info.offset, 0);
        assert_eq!(b1.region_info.size, 256 << 20);
        assert_eq!(b1.sparse_areas.len(), 1);
        assert_eq!(b1.sparse_areas[0].area.offset, 0);
        assert_eq!(b1.sparse_areas[0].area.size, 2 << 20);
        // Every other region is as before.
        for i in [0usize, 2, 3, 4, 5, 6, 7, 8] {
            assert_eq!(r[i].region_info.flags, plain[i].region_info.flags);
            assert!(r[i].mmap_fd.is_none());
        }
        // An area larger than the BAR is not offered.
        let r = regions_with(&board, area(512 << 20));
        assert!(r[1].mmap_fd.is_none());
    }

    #[test]
    fn bar2_memfd_makes_region_3_mmapable_around_the_trapped_page() {
        let board = crate::board::Board::load("rtx2070").unwrap();
        let plain = regions(&board);
        let size = 32u64 << 20;
        let areas = crate::bar2area::sparse_areas(size, &[crate::bar2area::DEFAULT_TRAP]).unwrap();
        let b2 = Bar2Region { fd: 9, areas };
        let r = regions_with_bar2(&board, None, Some(&b2));
        let b3 = &r[3];
        assert_eq!(b3.mmap_fd, Some(9));
        assert_ne!(b3.region_info.flags & VFIO_REGION_INFO_FLAG_MMAP, 0);
        assert_eq!((b3.region_info.offset, b3.region_info.size), (0, size));
        let got: Vec<(u64, u64)> = b3
            .sparse_areas
            .iter()
            .map(|a| (a.area.offset, a.area.size))
            .collect();
        assert_eq!(got, vec![(0, 0xfe_a000), (0xfe_b000, size - 0xfe_b000)]);
        for i in [0usize, 1, 2, 4, 5, 6, 7, 8] {
            assert_eq!(r[i].region_info.flags, plain[i].region_info.flags);
            assert!(r[i].mmap_fd.is_none());
        }
        // Both areas at once, and an area past the BAR is refused (region 3 stays trapped).
        let both = regions_with_bar2(&board, Some(Bar1Area { fd: 7, len: 1 << 20 }), Some(&b2));
        assert_eq!((both[1].mmap_fd, both[3].mmap_fd), (Some(7), Some(9)));
        let bad = Bar2Region { fd: 9, areas: vec![(0, size + 4096)] };
        assert!(regions_with_bar2(&board, None, Some(&bad))[3].mmap_fd.is_none());
    }
}

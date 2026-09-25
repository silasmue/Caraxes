// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
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
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use vfio_bindings::bindings::vfio::{
    vfio_region_info, VFIO_IRQ_INFO_EVENTFD, VFIO_IRQ_INFO_NORESIZE, VFIO_IRQ_SET_ACTION_TRIGGER,
    VFIO_IRQ_SET_DATA_EVENTFD, VFIO_PCI_BAR0_REGION_INDEX, VFIO_PCI_CONFIG_REGION_INDEX,
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
    bar0: Bar0,
    trace: Trace,
    dma: Vec<DmaMapping>,
    mem: GuestMemory,
    irq_fds: Vec<Vec<File>>,
    guest_ram: GuestRam,
    /// Where the table is published (`<state dir>/dma-ranges`), in the module's format.
    dma_ranges: Option<PathBuf>,
    /// The table entries, per DMA_MAP region.
    ranges: Vec<DmaRange>,
    /// The vectors and MSI state the host event thread raises through (E5b part 2).
    intr: Option<Arc<IntrShared>>,
    /// Doorbell ioeventfd: wakeups of the reader thread and guest writes they counted.
    doorbell_wakeups: u64,
    doorbell_writes: u64,
    /// MSI re-arm ioeventfd (`CARAXES_MSI_REARM_IOEVENTFD=1`): wakeups and guest writes.
    rearm_wakeups: u64,
    rearm_writes: u64,
}

/// The switch for the doorbell ioeventfd.
pub const DOORBELL_IOEVENTFD_ENV: &str = "CARAXES_DOORBELL_IOEVENTFD";

/// Whether `CARAXES_DOORBELL_IOEVENTFD=1` is set.
pub fn doorbell_ioeventfd_requested() -> bool {
    std::env::var(DOORBELL_IOEVENTFD_ENV).is_ok_and(|v| v.trim() == "1")
}

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

/// The MSI re-arm sub-region at BAR0 `offset` (the `NV_PCFG` base plus [`NV_XVE_CYA_2`]):
/// 4 bytes, no datamatch (the value is ignored), and the eventfd's reading end. The shape
/// is the token-less doorbell's ([`doorbell_sub_region`]).
pub fn msi_rearm_sub_region(offset: u64) -> std::io::Result<(IoEventFdSubRegion, File)> {
    doorbell_sub_region(offset)
}

/// The doorbell sub-region to offer on BAR0 and the eventfd's reading end. `offset` is
/// the chip's `registers.usermode_doorbell` (`NOTIFY_CHANNEL_PENDING`, a 32-bit
/// register), so the size is 4. There is no datamatch here: this is the token-less
/// path, one eventfd for every value written. The token pool
/// ([`doorbell_token_sub_regions`]) gives each pool token its own datamatch entry and
/// eventfd instead.
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
    spawn_token_reader_hooked(device, doorbell, readers, reactor, None, None)
}

/// As [`spawn_token_reader`], with a hook the reactor runs on every iteration (the
/// interrupt page, intr_page.rs). `rearm`: the MSI re-arm eventfd as one more reader after
/// the tokens' (`CARAXES_MSI_REARM_IOEVENTFD=1`), `(BAR0 offset, eventfd)`; its wakeups go
/// to [`Device::msi_rearm_ioeventfd`], never to a token.
pub fn spawn_token_reader_hooked(
    device: Arc<Mutex<Device>>,
    doorbell: u64,
    mut readers: Vec<File>,
    reactor: crate::poll::Reactor,
    hook: Option<Box<dyn crate::poll::Hook>>,
    rearm: Option<(u64, File)>,
) -> std::io::Result<Arc<crate::poll::ReaderStats>> {
    let tokens = readers.len();
    let rearm_at = rearm.map(|(offset, fd)| {
        readers.push(fd);
        offset
    });
    reactor.spawn_hooked(
        readers,
        move |rings, seen, st| {
            let mut d = lock(&device);
            st.lock_wait.record(seen.elapsed());
            crate::launch::mark(crate::launch::Step::Lock);
            match rearm_at {
                None => d.doorbell_tokens(doorbell, rings, &mut |host| st.ring(seen, host)),
                Some(offset) => {
                    for r in rings {
                        if r.0 < tokens {
                            d.doorbell_tokens(doorbell, std::slice::from_ref(r), &mut |host| {
                                st.ring(seen, host)
                            });
                        } else {
                            d.msi_rearm_ioeventfd(offset, r.1);
                        }
                    }
                }
            }
        },
        hook,
    )
}

fn lock(device: &Mutex<Device>) -> MutexGuard<'_, Device> {
    device.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Read the doorbell eventfd (the token-less one) and ring the device for as long as
/// the process runs, through the same reactor as the token pool (one eventfd).
pub fn spawn_doorbell_reader(
    device: Arc<Mutex<Device>>,
    doorbell: u64,
    eventfd: File,
    reactor: crate::poll::Reactor,
) -> std::io::Result<Arc<crate::poll::ReaderStats>> {
    spawn_doorbell_reader_hooked(device, doorbell, eventfd, reactor, None)
}

/// As [`spawn_doorbell_reader`], with a reactor hook (the interrupt page).
pub fn spawn_doorbell_reader_hooked(
    device: Arc<Mutex<Device>>,
    doorbell: u64,
    eventfd: File,
    reactor: crate::poll::Reactor,
    hook: Option<Box<dyn crate::poll::Hook>>,
) -> std::io::Result<Arc<crate::poll::ReaderStats>> {
    reactor.spawn_hooked(
        vec![eventfd],
        move |rings, seen, st| {
            let mut d = lock(&device);
            st.lock_wait.record(seen.elapsed());
            crate::launch::mark(crate::launch::Step::Lock);
            for &(_, writes) in rings {
                let host = d.doorbell_ioeventfd(doorbell, writes);
                st.ring(seen, host);
            }
        },
        hook,
    )
}

/// The device as the vfio-user server loop sees it when the doorbell reader thread
/// shares it.
pub struct SharedDevice(pub Arc<Mutex<Device>>);

impl ServerBackend for SharedDevice {
    fn region_read(&mut self, region: u32, offset: u64, data: &mut [u8]) -> std::io::Result<()> {
        lock(&self.0).region_read(region, offset, data)
    }

    fn region_write(&mut self, region: u32, offset: u64, data: &[u8]) -> std::io::Result<()> {
        lock(&self.0).region_write(region, offset, data)
    }

    fn dma_map(
        &mut self,
        flags: DmaMapFlags,
        offset: u64,
        address: u64,
        size: u64,
        fd: Option<File>,
    ) -> std::io::Result<()> {
        lock(&self.0).dma_map(flags, offset, address, size, fd)
    }

    fn dma_unmap(&mut self, flags: DmaUnmapFlags, address: u64, size: u64) -> std::io::Result<()> {
        lock(&self.0).dma_unmap(flags, address, size)
    }

    fn reset(&mut self) -> std::io::Result<()> {
        lock(&self.0).reset()
    }

    fn set_irqs(
        &mut self,
        index: u32,
        flags: u32,
        start: u32,
        count: u32,
        fds: Vec<File>,
    ) -> std::io::Result<()> {
        lock(&self.0).set_irqs(index, flags, start, count, fds)
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
            cfg,
            bar0,
            trace,
            dma: Vec::new(),
            mem: GuestMemory::new(),
            irq_fds,
            guest_ram: GuestRam::Plain,
            dma_ranges: None,
            ranges: Vec::new(),
            intr: None,
            doorbell_wakeups: 0,
            doorbell_writes: 0,
            rearm_wakeups: 0,
            rearm_writes: 0,
        }
    }

    /// Guest writes to the MSI re-arm register (`NV_XVE_CYA_2`, BAR0 `offset`) that arrived
    /// through its ioeventfd (`CARAXES_MSI_REARM_IOEVENTFD=1`, intr-page.md §4): `writes` of
    /// them since the last wakeup. The value is ignored, so this does exactly what one
    /// trapped write does (`region_write` on BAR0): the config mirror write
    /// (no writable bit at `NV_XVE_CYA_2`, as for the trapped one) and the steps after
    /// every trapped BAR0 write (`after_bar0_write`: the P21 check, the BAR1
    /// pool's poll, a pending MSI of the model, the MSI enable). Several writes between two
    /// wakeups are one re-arm: each would repeat the same idempotent steps.
    pub fn msi_rearm_ioeventfd(&mut self, offset: u64, writes: u64) {
        self.rearm_wakeups += 1;
        self.rearm_writes += writes;
        let effect = self.bar0.write(&mut self.cfg, &self.mem, offset, &0u32.to_le_bytes());
        self.trace.event(&format!(
            "msi re-arm (eventfd): {writes} guest write(s) to BAR0 {offset:#x} (NV_XVE_CYA_2) signalled ({effect:?})"
        ));
        self.after_bar0_write();
    }

    /// Guest writes to the doorbell register that arrived through the ioeventfd:
    /// `writes` of them since the last wakeup. KVM does not pass the written token on
    /// (no datamatch), so every channel with GPFIFO entries published since its last
    /// doorbell is rung the way its token would ring it (`Dispatcher::doorbell_pending`:
    /// software, shadow or host mirror); a channel the guest never rang is not pending
    /// and gets no host mirror. Returns when the first host doorbell was written, if one
    /// was.
    pub fn doorbell_ioeventfd(&mut self, doorbell: u64, writes: u64) -> Option<std::time::Instant> {
        self.doorbell_wakeups += 1;
        self.doorbell_writes += writes;
        let mem = &self.mem;
        let (notes, host) = match self.bar0.boot_mut() {
            Some(boot) => {
                boot.take_host_doorbell();
                let n = boot.doorbell_pending(mem);
                (n, boot.take_host_doorbell())
            }
            None => (vec!["no boot model, nothing rung".into()], None),
        };
        self.trace.event(&format!(
            "doorbell ioeventfd: {writes} guest write(s) to BAR0 {doorbell:#x} signalled"
        ));
        for n in notes {
            self.trace.event(&n);
        }
        host
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
        let mem = &self.mem;
        let Some(boot) = self.bar0.boot_mut() else {
            self.trace
                .event("doorbell tokens: no boot model, nothing rung");
            return;
        };
        let mut notes = Vec::new();
        for &(slot, writes) in rings {
            let token = crate::tokens::TokenPool::token(slot);
            if let Some(p) = boot.token_pool_mut() {
                p.stats.eventfd_rings += 1;
                p.stats.eventfd_writes += writes;
            }
            notes.push(format!(
                "doorbell ioeventfd: {writes} guest write(s) of token {token:#x} to BAR0 {doorbell:#x} signalled"
            ));
            boot.take_host_doorbell();
            notes.extend(boot.doorbell_token(mem, token));
            rung(boot.take_host_doorbell());
        }
        for n in notes {
            self.trace.event(&n);
        }
    }

    /// Choose how guest RAM is handled and where the table is published.
    pub fn with_guest_ram(mut self, guest_ram: GuestRam, dma_ranges: Option<PathBuf>) -> Self {
        self.guest_ram = guest_ram;
        self.dma_ranges = dma_ranges;
        self
    }

    /// Let the host event thread raise vectors and the MSI (its eventfd and the MSI
    /// enable bit are mirrored into `shared` as the client sets them).
    pub fn with_intr_shared(mut self, shared: Arc<IntrShared>) -> Self {
        self.intr = Some(shared);
        self
    }

    fn sync_msi_enabled(&self) {
        if let Some(s) = &self.intr {
            s.msi_enabled
                .store(self.cfg.msi_enabled(), std::sync::atomic::Ordering::Relaxed);
        }
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
        let table = DmaTable::new(self.ranges.clone()).map_err(std::io::Error::other)?;
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
        self.mem
            .set_table(if table.is_empty() { None } else { Some(table) });
        if let Some(path) = self.dma_ranges.clone() {
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, format!("{param}\n"))?;
            std::fs::rename(&tmp, &path)?;
            self.note(&format!("{what}; published to {}", path.display()));
        } else {
            self.note(&what);
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
    fn load_guest_aliases(&mut self) {
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
                let r = hm.lock().unwrap().register(address, size, ptr);
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
        self.publish_table()
    }

    /// The guest RAM behind a DMA_UNMAP goes: free its descriptors first.
    fn remove_guest_ram(&mut self, address: u64, size: u64) {
        let log = match &self.guest_ram {
            GuestRam::Plain => return,
            GuestRam::Identity => Vec::new(),
            GuestRam::Register(hm) => hm.lock().unwrap().release_region(address, size),
        };
        for l in log {
            self.note(&format!("hostmem: {l}"));
        }
        self.ranges
            .retain(|r| !(r.gpa >= address && r.gpa + r.size <= address + size));
        if let Err(e) = self.publish_table() {
            self.note(&format!("dma table: {e}"));
        }
    }

    /// Free the host memory at the end of the session (the client disconnected). The
    /// guest is gone, so nothing uses the IOVAs any more.
    pub fn shutdown(&mut self) {
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

    /// The display clock's tick with the display page (engine.rs `Ticker::set_driver`,
    /// display/fe_page.rs): advance the display engine, trace its notes.
    pub fn display_tick(&mut self) {
        let notes = match self.bar0.boot_mut() {
            Some(b) => b.display_tick(&self.mem),
            None => return,
        };
        for n in notes {
            self.trace.event(&format!("boot: {n}"));
        }
    }

    pub fn summary(&self) -> String {
        let mut s = self.trace.summary();
        if self.doorbell_wakeups > 0 {
            s.push_str(&format!(
                "doorbell ioeventfd: {} guest write(s) in {} wakeup(s)\n",
                self.doorbell_writes, self.doorbell_wakeups
            ));
        }
        if self.rearm_wakeups > 0 {
            s.push_str(&format!(
                "msi re-arm ioeventfd: {} guest write(s) in {} wakeup(s)\n",
                self.rearm_writes, self.rearm_wakeups
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
            if let Some(l) = b.fb.bar1_pool_summary() {
                s.push_str(&l);
                s.push('\n');
            }
            let kinds = b.fb.pte_kinds();
            if !kinds.is_empty() {
                let unsafe_pte = kinds
                    .iter()
                    .any(|&(k, c, _)| !(k == 0x06 || k == 0x00) || c != 0);
                s.push_str(&format!(
                    "vidmem PTE kinds walked (kind:comptagline x count): {}{}\n",
                    kinds
                        .iter()
                        .map(|(k, c, n)| format!("{k:#04x}:{c:#x}x{n}"))
                        .collect::<Vec<_>>()
                        .join(" "),
                    if unsafe_pte {
                        " -- COMPRESSIBLE KIND OR NON-ZERO COMPTAGLINE PRESENT (comptag hold, execution.md §7)"
                    } else {
                        " -- all uncompressed (kind 0x06/0x00, comptagline 0): comptag hold satisfied"
                    }
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
        let mut note = None;
        match region {
            VFIO_PCI_CONFIG_REGION_INDEX => self.cfg.read(offset as usize, data),
            VFIO_PCI_BAR0_REGION_INDEX => {
                self.bar0.read(&self.cfg, &self.mem, offset, data);
                note = self.bar0.name(offset);
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
        let note = match region {
            VFIO_PCI_CONFIG_REGION_INDEX => {
                self.cfg.write(offset as usize, data);
                // NV208F BIF_INFO answers from the guest's Link Control (public.rs).
                let lnk = self.cfg.link_control();
                if let Some(b) = self.bar0.boot_mut() {
                    b.ctrl.link_control = lnk;
                }
                None
            }
            VFIO_PCI_BAR0_REGION_INDEX => {
                match self.bar0.write(&mut self.cfg, &self.mem, offset, data) {
                    WriteEffect::Mirror => self.bar0.name(offset),
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
            self.after_bar0_write();
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
        if let Some(f) = &fd {
            if let Err(e) = self.mem.map(address, size, offset, f) {
                self.trace
                    .event(&format!("dma_map address={address:#x}: mmap failed: {e}"));
            } else if let Err(e) = self.add_guest_ram(address, size) {
                // Refuse the map: the VM must not run with guest RAM the GPU cannot
                // reach or with a host short of RAM.
                self.mem.unmap(address, size);
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
        self.dma
            .retain(|m| !(m.address == address && m.size == size));
        self.remove_guest_ram(address, size);
        self.mem.unmap(address, size);
        Ok(())
    }

    fn reset(&mut self) -> Result<(), std::io::Error> {
        self.trace
            .event("reset: device model back to its initial state");
        self.bar0.reset();
        self.load_guest_aliases();
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
        if flags & VFIO_IRQ_SET_DATA_EVENTFD != 0 && flags & VFIO_IRQ_SET_ACTION_TRIGGER != 0 {
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
    fn after_bar0_write(&mut self) {
        // P21: the guest flushes and invalidates through BAR0 after changing a BAR1
        // mapping and before it uses the mapping.
        if let Some(b) = self.bar0.boot() {
            b.fb.poll_bar1_passthrough();
        }
        // P21 design H: the same point moves pages into and out of the BAR1 pool.
        if let Some(b) = self.bar0.boot_mut() {
            crate::bar1pool::poll_model(b, &self.mem);
        }
        self.after_write();
    }

    /// The steps after every trapped write of BAR0 or config space: the model's notes, a
    /// function-level reset, a pending MSI, the MSI enable.
    fn after_write(&mut self) {
        for n in self.bar0.take_notes() {
            self.trace.event(&format!("boot: {n}"));
        }
        if self.cfg.take_flr() {
            self.trace
                .event("flr: function-level reset, device model back to its initial state");
            self.bar0.reset();
            self.load_guest_aliases();
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

    /// The MSI re-arm ioeventfd (intr-page.md §4): 4 bytes at `NV_PCFG` + `NV_XVE_CYA_2`
    /// without datamatch, accepted by the server beside the token pool's sub-regions; the
    /// token reactor hands its wakeups to `msi_rearm_ioeventfd` and never to a token, and a
    /// re-arm through it leaves config space as the trapped write does.
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
        let device = Arc::new(Mutex::new(rtx2070_device()));
        let reactor = crate::poll::Reactor::new("test", crate::poll::Config::default(), None).unwrap();
        let stats = spawn_token_reader_hooked(
            device.clone(),
            0xbb_0090,
            token_readers,
            reactor,
            None,
            Some((at, rearm_reader)),
        )
        .unwrap();
        let t0 = std::time::Instant::now();
        while stats.events.load(std::sync::atomic::Ordering::Relaxed) < 2 {
            assert!(t0.elapsed() < std::time::Duration::from_secs(5), "reactor did not read");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        stats.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let d = lock(&device);
        assert_eq!((d.rearm_wakeups, d.rearm_writes), (1, 2));
        let cfg: Vec<u32> = (0..0x1000).step_by(4).map(|o| d.cfg.dword(o)).collect();
        assert_eq!(cfg, before);
        assert!(d.summary().contains("msi re-arm ioeventfd: 2 guest write(s) in 1 wakeup(s)"));
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

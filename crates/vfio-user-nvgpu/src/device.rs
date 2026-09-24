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
use std::io::Read;
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
}

/// The switch for the doorbell ioeventfd.
pub const DOORBELL_IOEVENTFD_ENV: &str = "CARAXES_DOORBELL_IOEVENTFD";

/// Whether `CARAXES_DOORBELL_IOEVENTFD=1` is set.
pub fn doorbell_ioeventfd_requested() -> bool {
    std::env::var(DOORBELL_IOEVENTFD_ENV).is_ok_and(|v| v.trim() == "1")
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
pub fn spawn_token_reader(
    device: Arc<Mutex<Device>>,
    doorbell: u64,
    readers: Vec<File>,
) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    // SAFETY: a plain syscall; the result is checked before use.
    let ep = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
    if ep < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: a new descriptor that nothing else owns.
    let ep = unsafe { OwnedFd::from_raw_fd(ep) };
    for (slot, r) in readers.iter().enumerate() {
        let mut ev = libc::epoll_event {
            events: libc::EPOLLIN as u32,
            u64: slot as u64,
        };
        // SAFETY: both descriptors are open; `ev` lives across the call.
        if unsafe { libc::epoll_ctl(ep.as_raw_fd(), libc::EPOLL_CTL_ADD, r.as_raw_fd(), &mut ev) }
            < 0
        {
            return Err(std::io::Error::last_os_error());
        }
    }
    std::thread::Builder::new()
        .name("doorbell-tokens".into())
        .spawn(move || {
            let mut events = vec![libc::epoll_event { events: 0, u64: 0 }; readers.len().max(1)];
            loop {
                // SAFETY: `events` has room for the count passed.
                let n = unsafe {
                    libc::epoll_wait(
                        ep.as_raw_fd(),
                        events.as_mut_ptr(),
                        events.len() as i32,
                        -1,
                    )
                };
                if n < 0 {
                    let e = std::io::Error::last_os_error();
                    if e.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    eprintln!("vfio-user-nvgpu: doorbell tokens: epoll_wait: {e}; reader ends, pool tokens are no longer rung");
                    return;
                }
                // Read every signalled eventfd first (EPOLLIN: the read does not
                // block), then ring under one lock.
                let mut rings = Vec::with_capacity(n as usize);
                for ev in &events[..n as usize] {
                    let slot = ev.u64 as usize;
                    let mut count = [0u8; 8];
                    match (&readers[slot]).read_exact(&mut count) {
                        Ok(()) => rings.push((slot, u64::from_ne_bytes(count))),
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(e) => {
                            eprintln!("vfio-user-nvgpu: doorbell tokens: slot {slot}: {e}; reader ends");
                            return;
                        }
                    }
                }
                lock(&device).doorbell_tokens(doorbell, &rings);
            }
        })?;
    Ok(())
}

fn lock(device: &Mutex<Device>) -> MutexGuard<'_, Device> {
    device.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Read the doorbell eventfd and ring the device for as long as the process runs.
pub fn spawn_doorbell_reader(
    device: Arc<Mutex<Device>>,
    doorbell: u64,
    eventfd: File,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("doorbell-ioeventfd".into())
        .spawn(move || {
            let mut count = [0u8; 8];
            loop {
                match (&eventfd).read_exact(&mut count) {
                    Ok(()) => {
                        lock(&device).doorbell_ioeventfd(doorbell, u64::from_ne_bytes(count))
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        eprintln!("vfio-user-nvgpu: doorbell ioeventfd: {e}; reader ends");
                        return;
                    }
                }
            }
        })?;
    Ok(())
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
        }
    }

    /// Guest writes to the doorbell register that arrived through the ioeventfd:
    /// `writes` of them since the last wakeup. KVM does not pass the written token on
    /// (no datamatch), so every channel with GPFIFO entries published since its last
    /// doorbell is rung the way its token would ring it (`Dispatcher::doorbell_pending`:
    /// software, shadow or host mirror); a channel the guest never rang is not pending
    /// and gets no host mirror.
    pub fn doorbell_ioeventfd(&mut self, doorbell: u64, writes: u64) {
        self.doorbell_wakeups += 1;
        self.doorbell_writes += writes;
        let mem = &self.mem;
        let notes = match self.bar0.boot_mut() {
            Some(boot) => boot.doorbell_pending(mem),
            None => vec!["no boot model, nothing rung".into()],
        };
        self.trace.event(&format!(
            "doorbell ioeventfd: {writes} guest write(s) to BAR0 {doorbell:#x} signalled"
        ));
        for n in notes {
            self.trace.event(&n);
        }
    }

    /// Pool tokens whose ioeventfds fired: `(slot, guest writes counted)` each. The
    /// eventfd names the token, the pool names the channel (`Dispatcher::doorbell_pool`).
    pub fn doorbell_tokens(&mut self, doorbell: u64, rings: &[(usize, u64)]) {
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
            notes.extend(boot.doorbell_token(mem, token));
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

    pub fn summary(&self) -> String {
        let mut s = self.trace.summary();
        if self.doorbell_wakeups > 0 {
            s.push_str(&format!(
                "doorbell ioeventfd: {} guest write(s) in {} wakeup(s)\n",
                self.doorbell_writes, self.doorbell_wakeups
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
                        return Ok(());
                    }
                    None => Some("dropped"),
                }
            }
            _ => Some("dropped"),
        };
        self.trace.access(Dir::Write, region, offset, data, note);
        if region == VFIO_PCI_BAR0_REGION_INDEX {
            // P21: the guest flushes and invalidates through BAR0 after changing a BAR1
            // mapping and before it uses the mapping.
            if let Some(b) = self.bar0.boot() {
                b.fb.poll_bar1_passthrough();
            }
        }
        for n in self.bar0.take_notes() {
            self.trace.event(&format!("boot: {n}"));
        }
        if self.cfg.take_flr() {
            self.trace
                .event("flr: function-level reset, device model back to its initial state");
            self.bar0.reset();
        }
        if self.bar0.boot_mut().is_some_and(|b| b.take_msi()) {
            self.raise_msi();
        }
        self.sync_msi_enabled();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The pool's sub-regions: one per token at the doorbell, size 4, DATAMATCH with the
    /// token, each with its own eventfd; the vendored server accepts all of them (no two
    /// collide for KVM) and refuses a token-less entry next to them.
    #[test]
    fn token_sub_regions_one_eventfd_per_token() {
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
}

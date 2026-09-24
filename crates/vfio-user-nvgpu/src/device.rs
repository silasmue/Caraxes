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
    VFIO_PCI_ROM_REGION_INDEX, VFIO_REGION_INFO_FLAG_READ, VFIO_REGION_INFO_FLAG_WRITE,
};
use vfio_user::{
    DmaMapFlags, DmaUnmapFlags, IoEventFdSubRegion, IrqInfo, ServerBackend, ServerRegion,
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
/// register), so the size is 4. There is no datamatch: Cloud Hypervisor's client
/// announces `max_msg_fds` 1, so a region gets at most one eventfd, and per-token
/// datamatch entries would share it without naming the token either.
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
    /// (no datamatch), so every recorded channel is rung through the BAR0 write path a
    /// trapped doorbell takes; a channel without new GPFIFO entries runs nothing.
    pub fn doorbell_ioeventfd(&mut self, doorbell: u64, writes: u64) {
        self.doorbell_wakeups += 1;
        self.doorbell_writes += writes;
        let chids: Vec<u32> = self
            .bar0
            .boot()
            .map(|b| b.ctrl.channels.channels().map(|c| c.chid).collect())
            .unwrap_or_default();
        self.trace.event(&format!(
            "doorbell ioeventfd: {writes} guest write(s) to BAR0 {doorbell:#x} signalled; ringing {} recorded channel(s) {chids:?}",
            chids.len()
        ));
        for chid in chids {
            // region_write() never fails for BAR0.
            let _ = self.region_write(VFIO_PCI_BAR0_REGION_INDEX, doorbell, &chid.to_le_bytes());
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

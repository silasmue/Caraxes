// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Guest RAM as the client offered it through `DMA_MAP` with file descriptors
//! (Cloud Hypervisor maps every memory zone of a `--memory shared=on` VM).
//! The device reads the structures the driver places in sysmem: the bootloader
//! descriptors, the FWSEC command, later the libos arguments and the message
//! queues (docs/caraxes/gsp-boot-turing.md §3).
//!
//! Every address the device takes from the guest is a DMA address. Once guest RAM is
//! registered with the host RM and the guest module `caraxes_dma_range` installed the
//! table (E5b, docs/caraxes/memory-model.md), those are host IOVAs, not guest physical
//! addresses. [`GuestMemory::translate`] is the one place that maps them back: every
//! read and write goes through it. Without a table it is the identity.

use std::fs::File;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use crate::dmamap::{DmaRange, DmaTable};
use crate::registrar::Registrar;

/// One `DMA_MAP` mapping, unmapped when the last region and view holding it is gone.
struct Mapping {
    ptr: *mut u8,
    size: u64,
}

// SAFETY: plain shared memory of the guest; the device copies bytes in and out of it, as
// the guest's vCPUs and the GPU do concurrently (the hardware's DMA has the same races).
unsafe impl Send for Mapping {}
// SAFETY: as above; the mapping itself is never changed after mmap.
unsafe impl Sync for Mapping {}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: pointer and size come from a successful mmap in `GuestMemory::map`.
        unsafe { libc::munmap(self.ptr.cast(), self.size as usize) };
    }
}

#[derive(Clone)]
struct Region {
    address: u64,
    size: u64,
    map: Arc<Mapping>,
}

/// A read-mostly view of guest memory for threads that must not take the device mutex
/// (the display clock, docs/caraxes/display-own-clock.md §2.2): the same mappings, the
/// same translation, replaced by the owner at every change of either.
pub type MemView = Arc<RwLock<Arc<GuestMemory>>>;

#[derive(Default)]
pub struct GuestMemory {
    regions: Vec<Region>,
    /// The active DMA table; `None` is the identity (no registration, E0-E5a).
    table: Option<DmaTable>,
    /// The guest's other DMA addresses for some of that RAM: the swiotlb alias of its
    /// installed table, validated against `table` (dmamap.rs `aliases_of`). Only
    /// consulted for an address `table` does not cover.
    aliases: Vec<DmaRange>,
    /// DMA addresses the table does not cover (count, first few) since the table
    /// was set: each one is an access the device refused.
    misses: AtomicU64,
    first_misses: Mutex<Vec<u64>>,
    /// Guest RAM answered at `DMA_MAP` whose registration runs off the loop (registrar.rs).
    registrar: Option<Arc<Registrar>>,
    /// The published view ([`GuestMemory::view`]), replaced at every change.
    view: Option<MemView>,
}

impl GuestMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// The same mappings and translation, without the view and with fresh counters.
    fn snapshot(&self) -> GuestMemory {
        GuestMemory {
            regions: self.regions.clone(),
            table: self.table.clone(),
            aliases: self.aliases.clone(),
            misses: AtomicU64::new(0),
            first_misses: Mutex::new(Vec::new()),
            registrar: self.registrar.clone(),
            view: None,
        }
    }

    /// The view other threads read guest memory through; this memory replaces it at every
    /// change of its mappings or translation from now on.
    pub fn view(&mut self) -> MemView {
        if self.view.is_none() {
            self.view = Some(Arc::new(RwLock::new(Arc::new(self.snapshot()))));
        }
        self.view.clone().expect("set above")
    }

    fn republish(&self) {
        if let Some(v) = &self.view {
            let snap = Arc::new(self.snapshot());
            *v.write().unwrap_or_else(std::sync::PoisonError::into_inner) = snap;
        }
    }

    /// Map `size` bytes at `offset` of `fd` as guest physical range `address..`.
    pub fn map(&mut self, address: u64, size: u64, offset: u64, fd: &File) -> std::io::Result<()> {
        // SAFETY: mmap of a file descriptor the client handed us; the result is
        // checked and stored with its size; unmapped in Drop.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd.as_raw_fd(),
                offset as libc::off_t,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(std::io::Error::last_os_error());
        }
        self.regions.push(Region {
            address,
            size,
            map: Arc::new(Mapping {
                ptr: ptr.cast(),
                size,
            }),
        });
        self.republish();
        Ok(())
    }

    /// Drop the region; its mapping goes when no view holds it any more.
    pub fn unmap(&mut self, address: u64, size: u64) {
        self.regions.retain(|r| !(r.address == address && r.size == size));
        self.republish();
    }

    /// Guest physical `gpa..gpa + len` in one mapping: the host pointer.
    fn locate_gpa(&self, gpa: u64, len: u64) -> Option<*mut u8> {
        self.regions
            .iter()
            .find(|r| gpa >= r.address && gpa - r.address <= r.size && len <= r.size - (gpa - r.address))
            .map(|r| r.map.ptr.wrapping_add((gpa - r.address) as usize))
    }

    /// The guest physical address of `len` bytes at the DMA address `addr`: through
    /// the DMA table when one is active (an address outside it is refused and
    /// counted, never passed through), else the identity.
    pub fn translate(&self, addr: u64, len: u64) -> Option<u64> {
        match &self.table {
            None => Some(addr),
            Some(t) => {
                let len = len.max(1);
                let gpa = t
                    .iova_to_gpa(addr, len)
                    .or_else(|| {
                        self.aliases
                            .iter()
                            .find(|r| {
                                addr >= r.iova
                                    && addr - r.iova < r.size
                                    && len <= r.size - (addr - r.iova)
                            })
                            .map(|r| r.gpa + (addr - r.iova))
                    })
                    .or_else(|| {
                        // GPA mode: guest RAM still being registered is this VM's own, mapped
                        // here; the device's access goes through (registrar.rs).
                        self.registrar
                            .as_ref()
                            .filter(|r| r.gpa_mode() && r.unsettled(addr, len))
                            .map(|_| addr)
                    });
                if gpa.is_none() {
                    self.misses.fetch_add(1, Ordering::Relaxed);
                    let mut f = self.first_misses.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    if f.len() < 16 {
                        f.push(addr);
                    }
                }
                gpa
            }
        }
    }

    fn locate(&self, addr: u64, len: u64) -> Option<*mut u8> {
        self.locate_gpa(self.translate(addr, len)?, len)
    }

    /// Whether the DMA address range is guest RAM the device can reach.
    pub fn covers(&self, pa: u64, len: u64) -> bool {
        self.locate(pa, len).is_some()
    }

    /// Make `table` the translation for every later access (or the identity). The
    /// guest's aliases were checked against the old table and go with it.
    pub fn set_table(&mut self, table: Option<DmaTable>) {
        self.table = table;
        self.aliases.clear();
        self.misses.store(0, Ordering::Relaxed);
        self.first_misses.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
        self.republish();
    }

    /// Accept the guest's aliases (dmamap.rs `aliases_of`, which checked them against the
    /// active table) for every later access.
    pub fn set_aliases(&mut self, aliases: Vec<DmaRange>) {
        self.aliases = aliases;
        self.republish();
    }

    pub fn aliases(&self) -> &[DmaRange] {
        &self.aliases
    }

    /// Consult `registrar` for guest RAM answered but not registered yet.
    pub fn set_registrar(&mut self, registrar: Option<Arc<Registrar>>) {
        self.registrar = registrar;
        self.republish();
    }

    /// The host IOVA of `len` bytes at the guest's DMA address `addr`: the address itself
    /// when the device's table covers it (the usual case, guest and host agree), the
    /// registered IOVA of the same guest pages for an alias, `None` when neither covers
    /// the whole range in one piece. Counts nothing: the shadow filter reports its own
    /// refusals.
    pub fn host_iova(&self, addr: u64, len: u64) -> Option<u64> {
        let Some(t) = &self.table else {
            return Some(addr);
        };
        let len = len.max(1);
        if t.iova_to_gpa(addr, len).is_some() {
            return Some(addr);
        }
        let Some(r) = self
            .aliases
            .iter()
            .find(|r| addr >= r.iova && addr - r.iova < r.size && len <= r.size - (addr - r.iova))
        else {
            // GPA mode: a piece registered off the loop, not in the table yet (registrar.rs).
            let r = self
                .registrar
                .as_ref()
                .filter(|r| r.gpa_mode())?
                .range_of(addr, len)?;
            return Some(r.iova + (addr - r.gpa));
        };
        let gpa = r.gpa + (addr - r.iova);
        let host = t.gpa_to_iova(gpa)?;
        (t.iova_to_gpa(host, len) == Some(gpa)).then_some(host)
    }

    pub fn table(&self) -> Option<&DmaTable> {
        self.table.as_ref()
    }

    /// Refused DMA addresses: how many, and the first few.
    pub fn misses(&self) -> (u64, Vec<u64>) {
        (
            self.misses.load(Ordering::Relaxed),
            self.first_misses.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone(),
        )
    }

    /// The device's own mapping of guest physical `gpa..gpa + len` (for registering
    /// guest RAM with the host RM), when it lies in one DMA_MAP region.
    pub fn host_ptr(&self, gpa: u64, len: u64) -> Option<*mut u8> {
        self.locate_gpa(gpa, len)
    }

    pub fn regions(&self) -> Vec<(u64, u64)> {
        self.regions.iter().map(|r| (r.address, r.size)).collect()
    }

    /// Copy guest bytes at `pa`; `false` when the range is not mapped.
    pub fn read(&self, pa: u64, buf: &mut [u8]) -> bool {
        match self.locate(pa, buf.len() as u64) {
            Some(p) => {
                // SAFETY: locate() guarantees the range lies inside one mapping.
                unsafe { std::ptr::copy_nonoverlapping(p, buf.as_mut_ptr(), buf.len()) };
                true
            }
            None => false,
        }
    }

    pub fn write(&self, pa: u64, data: &[u8]) -> bool {
        match self.locate(pa, data.len() as u64) {
            Some(p) => {
                // SAFETY: as in read().
                unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len()) };
                true
            }
            None => false,
        }
    }

    pub fn read_u32(&self, pa: u64) -> Option<u32> {
        let mut b = [0u8; 4];
        self.read(pa, &mut b).then(|| u32::from_le_bytes(b))
    }

    pub fn read_u64(&self, pa: u64) -> Option<u64> {
        let mut b = [0u8; 8];
        self.read(pa, &mut b).then(|| u64::from_le_bytes(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn maps_and_reads_a_file() {
        let path = std::env::temp_dir().join(format!("nvgpu-guestmem-{}", std::process::id()));
        let mut f = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        f.write_all(&[0u8; 8192]).unwrap();
        f.write_all(&0x1234_5678u32.to_le_bytes()).unwrap();
        f.write_all(&[0u8; 4092]).unwrap();
        let mut m = GuestMemory::new();
        m.map(0x1000_0000, 4096, 8192, &f).unwrap();
        assert_eq!(m.read_u32(0x1000_0000), Some(0x1234_5678));
        assert_eq!(m.read_u32(0x1000_0ffc), Some(0));
        assert_eq!(m.read_u32(0x1000_1000), None);
        assert!(m.write(0x1000_0010, &[1, 2, 3, 4]));
        assert_eq!(m.read_u32(0x1000_0010), Some(0x0403_0201));
        assert!(m.covers(0x1000_0000, 4096));
        assert!(!m.covers(0x1000_0000, 4097));
        m.unmap(0x1000_0000, 4096);
        assert!(!m.covers(0x1000_0000, 4));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn every_access_goes_through_the_table() {
        let path = std::env::temp_dir().join(format!("nvgpu-guestmem-t-{}", std::process::id()));
        let f = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        f.set_len(4 << 20).unwrap();
        let mut m = GuestMemory::new();
        m.map(0x20_0000, 4 << 20, 0, &f).unwrap();
        assert!(m.write(0x20_0100, &[7, 7, 7, 7]));
        // IOVA 0x7f00_0000_0000 is GPA 0 (a 2 MiB hole, as the first page is in CH).
        let t = DmaTable::parse("0:7f0000000000:600000").unwrap();
        m.set_table(Some(t));
        assert_eq!(m.translate(0x7f00_0020_0100, 4), Some(0x20_0100));
        assert_eq!(m.read_u32(0x7f00_0020_0100), Some(0x0707_0707));
        // The guest physical address itself is no longer a valid DMA address.
        assert_eq!(m.read_u32(0x20_0100), None);
        assert!(m.write(0x7f00_0020_0104, &[1, 0, 0, 0]));
        assert_eq!(m.host_ptr(0x20_0104, 4).map(|p| unsafe { *p }), Some(1));
        assert_eq!(m.misses(), (1, vec![0x20_0100]));
        m.set_table(None);
        assert_eq!(m.read_u32(0x20_0104), Some(1));
        let _ = std::fs::remove_file(&path);
    }

    /// The published view follows every change of mappings and translation, reads from
    /// another thread, and keeps a mapping alive until the last view that holds it is gone.
    #[test]
    fn the_view_follows_changes_and_keeps_its_mappings() {
        let path = std::env::temp_dir().join(format!("nvgpu-guestmem-v-{}", std::process::id()));
        let f = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        f.set_len(2 << 20).unwrap();
        let mut m = GuestMemory::new();
        let view = m.view();
        assert_eq!(view.read().unwrap().read_u32(0x20_0000), None);
        m.map(0x20_0000, 2 << 20, 0, &f).unwrap();
        assert!(m.write(0x20_0040, &[5, 0, 0, 0]));
        let seen = std::thread::spawn({
            let view = view.clone();
            move || view.read().unwrap().read_u32(0x20_0040)
        })
        .join()
        .unwrap();
        assert_eq!(seen, Some(5));
        // A table: the view translates as the owner does.
        m.set_table(Some(DmaTable::parse("200000:7f0000200000:200000").unwrap()));
        assert_eq!(view.read().unwrap().read_u32(0x7f00_0020_0040), Some(5));
        assert_eq!(view.read().unwrap().read_u32(0x20_0040), None);
        // Unmapped: gone for the owner and the next view; a view taken before still reads.
        let old = view.read().unwrap().clone();
        m.unmap(0x20_0000, 2 << 20);
        assert_eq!(view.read().unwrap().read_u32(0x7f00_0020_0040), None);
        assert_eq!(old.read_u32(0x7f00_0020_0040), Some(5));
        let _ = std::fs::remove_file(&path);
    }

    /// The swiotlb alias of the guest's table (memory-model.md §2): the guest reaches GPA
    /// 0x20_0000.. at IOVA 0xf0_0000_0000.. as well, and the device must follow it for its
    /// own accesses and give the shadow the registered IOVA of the same page.
    #[test]
    fn the_guest_alias_reaches_the_same_pages() {
        let path = std::env::temp_dir().join(format!("nvgpu-guestmem-a-{}", std::process::id()));
        let f = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        f.set_len(4 << 20).unwrap();
        let mut m = GuestMemory::new();
        m.map(0, 4 << 20, 0, &f).unwrap();
        let dev = DmaTable::parse("0:7f0000000000:400000").unwrap();
        let guest = dev.with_alias(0x20_0000, 0x1000, 0xF0_0000_0000).unwrap();
        m.set_table(Some(dev.clone()));
        assert!(m.write(0x7f00_0020_0010, &[9, 8, 7, 6]));
        // Before the aliases are known the alias is refused (the Xid 31 case).
        assert_eq!(m.read_u32(0xF0_0000_0010), None);
        assert_eq!(m.host_iova(0xF0_0000_0010, 4), None);
        m.set_aliases(dev.aliases_of(&guest).unwrap());
        assert_eq!(m.read_u32(0xF0_0000_0010), Some(0x0607_0809));
        assert_eq!(m.host_iova(0xF0_0000_1000, 0x1000), Some(0x7f00_0020_1000));
        // A registered address is its own host IOVA; the alias ends where its entry ends.
        assert_eq!(m.host_iova(0x7f00_0000_1000, 0x1000), Some(0x7f00_0000_1000));
        assert_eq!(m.host_iova(0xF0_001F_F000, 0x2000), None);
        assert_eq!(m.host_iova(0xF0_0020_0000, 0x1000), None);
        // A new device table drops them.
        m.set_table(Some(dev));
        assert!(m.aliases().is_empty());
        let _ = std::fs::remove_file(&path);
    }
}

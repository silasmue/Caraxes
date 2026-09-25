// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Host memory the device process holds for its VM (E5b part 1,
//! docs/caraxes/memory-model.md): the fixed-address VRAM reservation (decision D11,
//! docs/caraxes/host-facts.md §3) and guest RAM registered as OS descriptors in 1 GiB
//! chunks, whose IOVAs form the guest DMA table (D13, docs/caraxes/sysmem-iova.md §4).
//!
//! Its own host RM client, next to the forwarding client of `hostrm.rs`, in the same
//! process (D10: one device process per VM, which the host's `nvidia-smi` shows as the
//! owner of the reservation, P6). Everything is freed by [`HostMemory::shutdown`], which
//! the device calls when the client disconnects and the signal thread calls on
//! SIGTERM/SIGINT/SIGHUP; if the process dies anyway, closing `/dev/nvidiactl` frees the
//! client with every object under it and unpins the pages.
//!
//! Only RM escapes: no register access, no mapping of the card. The one privileged
//! control, `GET_SURFACE_PHYS_ATTR`, runs inside [`Privilege::with_admin`].

use std::os::fd::AsRawFd;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use nvrm_abi::nvgpu::{nvos02_flags, nvos32_attr, nvos33_flags, Nvos02WithFd, Nvos33WithFd};
use nvrm_abi::{sys, NvDevice};
use nvrm_client::mem::CpuMap;
use nvrm_client::RmClient;

use crate::dmamap::DmaRange;
use crate::privilege::Privilege;
use crate::ver::active as v;

/// `NV_ESC_RM_GET_EVENT_DATA` (kernel-open/common/inc/nv_escape.h): drain the OS-event
/// records queued on an event ctl fd (`NVOS41_PARAMETERS`).
const NV_ESC_RM_GET_EVENT_DATA: u32 = 0x52;

/// A CPU window onto part of the VRAM reservation, mapped through the host's BAR1
/// (E5b part 2, docs/caraxes/execution.md): `RM_MAP_MEMORY` of the reservation on a
/// fresh `/dev/nvidia0` fd registered to the client, then `mmap` of that fd
/// (host-exec.rs `map_window`; nvrm-client `map_doorbell`). Write-combined: a write is
/// only in VRAM once a read of the window came back, which [`Window::write`] does.
pub struct Window {
    _fd: NvDevice,
    /// What `RM_UNMAP_MEMORY` needs to release the BAR1 mapping when the window goes:
    /// a duplicate of the client's ctl FD, hClient, hDevice, hMemory and the linear
    /// address RM returned. Closing the mapping FD alone does not release it (P20: every
    /// evicted window leaked host BAR1 until the client died, which exhausted the
    /// desktop's BAR1 with two devices).
    unmap: Option<(NvDevice, u32, u32, u32, sys::NvP64)>,
    ptr: *mut u8,
    /// Offset of the window inside the reservation, and its length.
    pub offset: u64,
    pub len: usize,
}

// SAFETY: the mapping is owned by the window and only ever touched from the server
// thread that owns the FB model; the raw pointer carries no thread affinity.
unsafe impl Send for Window {}

impl Window {
    /// Whether `[pa, pa + len)` of the reservation (reservation offsets) lies in the window.
    pub fn covers(&self, off: u64, len: usize) -> bool {
        off >= self.offset && off - self.offset + len as u64 <= self.len as u64
    }

    pub fn read(&self, off: u64, buf: &mut [u8]) {
        debug_assert!(self.covers(off, buf.len()));
        let at = (off - self.offset) as usize;
        // SAFETY: bounds checked against the mapping; volatile because it is device memory.
        // BAR1 reads are uncached PCIe reads, one bus round trip each: read aligned
        // qwords where possible (a byte loop made the shadow VAS's guest-root walk take
        // ~250 ms per GR doorbell, d-gr-launch7), bytes only at unaligned edges.
        unsafe {
            let mut i = 0;
            // Bulk reads (whole table pages, the shadow walk): streaming 16-byte loads
            // (`movntdqa`) fetch full lines from the write-combined BAR1 mapping instead of
            // one bus round trip per qword (docs/caraxes/mapping.md §1).
            if buf.len() >= 64 && (at % 16) == 0 && std::is_x86_feature_detected!("sse4.1") {
                i = stream_read(self.ptr.add(at), buf);
            }
            while i < buf.len() {
                if (at + i) % 8 == 0 && buf.len() - i >= 8 {
                    let q = std::ptr::read_volatile(self.ptr.add(at + i) as *const u64);
                    buf[i..i + 8].copy_from_slice(&q.to_le_bytes());
                    i += 8;
                } else if (at + i) % 4 == 0 && buf.len() - i >= 4 {
                    let d = std::ptr::read_volatile(self.ptr.add(at + i) as *const u32);
                    buf[i..i + 4].copy_from_slice(&d.to_le_bytes());
                    i += 4;
                } else {
                    buf[i] = std::ptr::read_volatile(self.ptr.add(at + i));
                    i += 1;
                }
            }
        }
    }

    /// Store `data`. The BAR1 mapping is write-combined, so the store only reaches VRAM
    /// when the WC buffer drains; [`Window::flush`] forces that before the GPU is told to
    /// read (the host doorbell, a TLB invalidate). A per-write read-back keeps the common
    /// read-after-write of the software executor coherent.
    pub fn write(&self, off: u64, data: &[u8]) {
        debug_assert!(self.covers(off, data.len()));
        let at = (off - self.offset) as usize;
        // SAFETY: as in read; dword stores where aligned keep register-sized writes
        // (USERD GPPut, page-table entries) atomic on the bus.
        unsafe {
            let mut i = 0;
            while i < data.len() {
                if (at + i) % 4 == 0 && data.len() - i >= 4 {
                    let w = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
                    std::ptr::write_volatile(self.ptr.add(at + i) as *mut u32, w);
                    i += 4;
                } else {
                    std::ptr::write_volatile(self.ptr.add(at + i), data[i]);
                    i += 1;
                }
            }
            if !data.is_empty() {
                std::ptr::read_volatile(self.ptr.add(at + data.len() - 1));
            }
        }
    }

    /// Drain the write-combining buffers of this window to VRAM: an `sfence` orders the
    /// WC stores, then a read of the first line forces the buffer to evict. Called before
    /// the GPU is pointed at anything the guest wrote here (host-exec.md: a not-yet-flushed
    /// BAR1 write surfaces to the physical engine as stale VRAM = a PBENTRY/INVALID_ADDRESS
    /// fault microseconds after the doorbell).
    pub fn flush(&self) {
        // SAFETY: sfence has no operands; the read is inside the mapping.
        unsafe {
            std::arch::x86_64::_mm_sfence();
            if self.len > 0 {
                std::ptr::read_volatile(self.ptr);
            }
        }
    }
}

/// Copy the 16-byte-aligned prefix of `buf.len()` bytes from `src` with streaming loads;
/// returns the bytes copied (a multiple of 16).
///
/// # Safety
/// `src..src + buf.len()` must be a readable mapping, `src` 16-byte aligned, and the CPU
/// must have SSE4.1.
#[target_feature(enable = "sse4.1")]
unsafe fn stream_read(src: *const u8, buf: &mut [u8]) -> usize {
    use std::arch::x86_64::{__m128i, _mm_storeu_si128, _mm_stream_load_si128};
    let n = buf.len() & !15;
    let mut i = 0;
    while i < n {
        // SAFETY: inside the mapping and aligned per the caller; the store goes to `buf`.
        unsafe {
            let v = _mm_stream_load_si128(src.add(i) as *const __m128i);
            _mm_storeu_si128(buf.as_mut_ptr().add(i) as *mut __m128i, v);
        }
        i += 16;
    }
    n
}

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: our own mapping.
        unsafe { libc::munmap(self.ptr.cast(), self.len) };
        // Release the RM mapping (and its BAR1 space), then the FD closes with `_fd`.
        if let Some((ctl, root, device, memory, linear)) = self.unmap.take() {
            let mut p = sys::NVOS34_PARAMETERS::default();
            p.hClient = root;
            p.hDevice = device;
            p.hMemory = memory;
            p.pLinearAddress = linear;
            p.flags = 0;
            // SAFETY: NVOS34 is this escape's ABI; plain data, no host pointers besides
            // the RM cookie it returned for this mapping.
            let r = unsafe { ctl.ioctl_raw(sys::NV_ESC_RM_UNMAP_MEMORY, &mut p) };
            if r.is_err() || p.status as u32 != sys::NV_OK {
                eprintln!(
                    "vfio-user-nvgpu: ERROR: fb: RM_UNMAP_MEMORY of the BAR1 window at reservation offset {:#x} (+{:#x}): {:?} status {:#x}",
                    self.offset, self.len, r.err(), p.status as u32
                );
            }
        }
    }
}

/// `RM_MAP_MEMORY` of `len` bytes of the memory object `handle` at `offset` on a fresh
/// `/dev/nvidia0` fd registered to `ctl`, then `mmap` of that fd: a BAR1 window.
fn map_window(
    ctl: &NvDevice,
    gpu: &NvDevice,
    root: u32,
    device: u32,
    handle: u32,
    offset: u64,
    len: usize,
) -> Result<Window, Status> {
    let target = gpu.open_for_mapping(ctl).map_err(|_| 0xFFFF_FFFFu32)?;
    let mut params = sys::NVOS33_PARAMETERS::default();
    params.hClient = root;
    params.hDevice = device;
    params.hMemory = handle;
    params.offset = offset;
    params.length = len as u64;
    params.pLinearAddress = 0usize as sys::NvP64;
    params.flags = nvos33_flags::ACCESS.set(nvos33_flags::ACCESS_READ_WRITE);
    let mut p = Nvos33WithFd::new(params, target.as_raw_fd());
    // SAFETY: NVOS33-with-fd is this escape's ABI; the context lands on `target`.
    if unsafe { ctl.ioctl_raw(sys::NV_ESC_RM_MAP_MEMORY, &mut p) }.is_err() {
        return Err(0xFFFF_FFFF);
    }
    if p.params.status as u32 != sys::NV_OK {
        return Err(p.params.status as u32);
    }
    // SAFETY: RM installed the mapping context on `target`; offset 0 by RM's rule.
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            target.as_raw_fd(),
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        return Err(0xFFFF_FFFF);
    }
    let linear = p.params.pLinearAddress;
    Ok(Window {
        _fd: target,
        unmap: ctl
            .try_clone()
            .ok()
            .map(|c| (c, root, device, handle, linear)),
        ptr: ptr.cast(),
        offset,
        len,
    })
}

/// BAR1 windows of the reservation without the `HostMemory` lock
/// ([`HostMemory::reservation_mapper`]). Only maps; the reservation outlives it because
/// the device frees the reservation after the FB store (and the client's fd close frees
/// every mapping anyway).
pub struct ReservationMapper {
    ctl: NvDevice,
    gpu: NvDevice,
    root: u32,
    device: u32,
    handle: u32,
}

impl ReservationMapper {
    pub fn map(&self, offset: u64, len: usize) -> Result<Window, Status> {
        map_window(&self.ctl, &self.gpu, self.root, self.device, self.handle, offset, len)
    }
}

/// P21 (docs/caraxes/bar1-passthrough.md): a dma-buf of one piece of the reservation,
/// exported with mmap allowed, which the device hands to the VMM as the mmap descriptor
/// of the guest's BAR1 region. Why a dma-buf and not the `RM_MAP_MEMORY` fd of
/// [`Window`]: its mmap accepts a non-zero page offset (nv-dmabuf.c `nv_dma_buf_mmap`),
/// and it holds a duplicate of the reservation handle and its host BAR1 mapping for as
/// long as the VMM keeps the descriptor or a mapping of it (`nv_dma_buf_vma_open` /
/// `_release`), so the host can never hand that BAR1 range or that VRAM to anyone else
/// while the guest can still reach it, even when this process dies first. An
/// `RM_MAP_MEMORY` mapping is released with its client regardless of other processes'
/// mappings (`nv_free_user_mapping` is empty, nv-usermap.c).
pub struct Bar1Export {
    fd: std::os::fd::OwnedFd,
    /// FB address of the first byte, and the length.
    pub fb: u64,
    pub len: u64,
}

impl Bar1Export {
    /// The dma-buf descriptor, for the region table (`ServerRegion::mmap_fd`).
    pub fn raw_fd(&self) -> std::os::fd::RawFd {
        self.fd.as_raw_fd()
    }
}

/// `NV_ESC_EXPORT_TO_DMABUF_FD` of `[offset, offset + len)` of the memory object
/// `handle` of client `root`, one object, mmap allowed (nv-dmabuf.c `nv_dma_buf_export`,
/// osapi.c `RmDmabufVerifyMemHandle`: vidmem, OS-page aligned, inside the object).
fn export_dmabuf(
    gpu: &NvDevice,
    root: u32,
    handle: u32,
    offset: u64,
    len: u64,
) -> Result<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd;
    let mut p = sys::nv_ioctl_export_to_dma_buf_fd {
        fd: -1,
        hClient: root,
        totalObjects: 1,
        numObjects: 1,
        index: 0,
        totalSize: len,
        mappingType: 0, // NV_DMABUF_EXPORT_MAPPING_TYPE_DEFAULT
        bAllowMmap: 1,
        ..Default::default()
    };
    p.handles[0] = handle;
    p.offsets[0] = offset;
    p.sizes[0] = len;
    // SAFETY: the escape's own parameter struct (nv-ioctl.h), plain data.
    unsafe { gpu.ioctl_raw(sys::NV_ESC_EXPORT_TO_DMABUF_FD, &mut p) }
        .map_err(|e| anyhow::anyhow!("NV_ESC_EXPORT_TO_DMABUF_FD: {e}"))?;
    if p.status != sys::NV_OK || p.fd < 0 {
        bail!("NV_ESC_EXPORT_TO_DMABUF_FD: status {:#x}, fd {}", p.status, p.fd);
    }
    // SAFETY: the driver installed a new descriptor for us (dma_buf_fd, nv-dmabuf.c).
    Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(p.fd) })
}

/// One host OS event: a private ctl fd the event is bound to (`allocate_os_event`,
/// osapi.c:598-635: the event lives on the file the escape was issued on, `fd` is an id),
/// the `NV01_EVENT_OS_EVENT` object and the notifier it is armed on.
pub struct OsEvent {
    pub ctl: NvDevice,
    pub id: u32,
    pub handle: u32,
    pub notifier: u32,
}

impl OsEvent {
    /// Drain the records queued on the event fd (`NV_ESC_RM_GET_EVENT_DATA`); returns
    /// `(hObject, notifyIndex, info32, info16)` per record.
    pub fn drain(&self) -> Vec<(u32, u32, u32, u16)> {
        let mut out = Vec::new();
        for _ in 0..64 {
            let mut ev = sys::NvUnixEvent::default();
            let mut p = sys::NVOS41_PARAMETERS::default();
            p.pEvent = &mut ev as *mut _ as usize as sys::NvP64;
            // SAFETY: NVOS41 with a pointer to one NvUnixEvent record.
            if unsafe { self.ctl.ioctl_raw(NV_ESC_RM_GET_EVENT_DATA, &mut p) }.is_err()
                || p.status as u32 != sys::NV_OK
            {
                break;
            }
            out.push((ev.hObject, ev.NotifyIndex, ev.info32, ev.info16));
            if p.MoreEvents == 0 {
                break;
            }
        }
        out
    }
}

const KIB: u64 = 1 << 10;
const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;
const HUGE: u64 = 2 * MIB;
/// Free host VRAM the reservation must leave (host-facts.md §3, rule 2).
pub const VRAM_HEADROOM: u64 = GIB;
/// Host RAM that pinning guest RAM must leave available.
pub const RAM_FLOOR: u64 = 4 * GIB;
/// Guest RAM is registered in chunks of this size, aligned in guest physical space
/// (sysmem-iova.md §4 item 1: one descriptor, one IOVA range, size-aligned).
pub const CHUNK: u64 = GIB;
/// Where each chunk's IOVA is read besides offset 0 and the last page.
const SAMPLE_STEP: u64 = 128 * MIB;
/// The P20 pattern record's two magic words (caraxes/tools/p20-patprobe.c).
const P20_MAGIC: (u32, u32) = (0xCA7A_5E5A, 0x50A7_7E2D);
/// Sampling of [`HostMemory::scan_reservation`]: one 2 MiB BAR1 window every 16 MiB, of
/// which the first 64 KiB are read.
const SCAN_STEP: u64 = 16 * MIB;
const SCAN_READ: usize = 64 * 1024;

/// Count, in `buf`, the 16-byte-aligned P20 pattern records and the non-zero dwords.
pub fn count_pattern(buf: &[u8]) -> (u64, u64) {
    let (mut records, mut nonzero) = (0u64, 0u64);
    for (i, w) in buf.chunks_exact(4).enumerate() {
        let d = u32::from_le_bytes([w[0], w[1], w[2], w[3]]);
        if d != 0 {
            nonzero += 1;
        }
        if i % 4 == 0 && d == P20_MAGIC.0 && buf.len() >= i * 4 + 8 {
            let e = u32::from_le_bytes(buf[i * 4 + 4..i * 4 + 8].try_into().unwrap());
            if e == P20_MAGIC.1 {
                records += 1;
            }
        }
    }
    (records, nonzero)
}

/// Heap attribution tag of the reservation (as host-facts.rs).
const OWNER_TAG: u32 = u32::from_le_bytes(*b"cara");

/// The fixed range of reservation slot `slot` for a profile of `s` bytes (a multiple of
/// 2 MiB) in the host heap region `[heap_base, region_end)`: slot 0 ends at the region
/// end aligned down to 2 MiB, slot n lies n profile sizes below it (P20). Refused when
/// the slot does not fit above `heap_base`.
pub fn slot_range(
    heap_base: u64,
    region_end: u64,
    s: u64,
    slot: u32,
) -> Result<(u64, u64), String> {
    let top = region_end & !(HUGE - 1);
    let fit = (slot as u64)
        .checked_mul(s)
        .and_then(|below| top.checked_sub(below))
        .and_then(|e| e.checked_sub(s).map(|b| (b, e)));
    match fit {
        Some((b, e)) if b >= heap_base => Ok((b, e)),
        _ => Err(format!(
            "refused: slot {slot} of a {} MiB profile ({} MiB below the top) does not fit in the host's FB heap region [{heap_base:#x}, {region_end:#x})",
            s / MIB,
            (slot as u64 + 1).saturating_mul(s) / MIB
        )),
    }
}

/// The fixed-address VRAM range held for the VM.
#[derive(Clone, Debug)]
pub struct Reservation {
    pub handle: u32,
    pub base: u64,
    pub size: u64,
    /// The end of the host's FB heap region (`limit + 1`), before 2 MiB alignment.
    pub region_end: u64,
    /// The host card's FB size (`TOTAL_RAM_SIZE`): the guest's FB address space.
    pub host_fb_total: u64,
    /// The free figures of the rule, in bytes: `HEAP_FREE`, nvidia-smi's FB free.
    pub heap_free: u64,
    pub smi_free: u64,
    pub alloc_ms: u128,
}

impl Reservation {
    pub fn end(&self) -> u64 {
        self.base + self.size
    }
}

/// One registered chunk of guest RAM.
#[derive(Clone, Copy, Debug)]
pub struct Descriptor {
    pub handle: u32,
    pub range: DmaRange,
    /// The device's own mapping of the chunk (guestmem.rs `map`, MAP_SHARED).
    pub host_va: usize,
}

/// A hole filler: `len` bytes of VA at `va` backed by one 2 MiB memfd page mapped over
/// and over, registered as a descriptor so the IOVA allocator places the next chunk
/// below it (docs/caraxes/mapping.md §5).
pub struct Filler {
    pub handle: u32,
    pub gpa: u64,
    pub iova: u64,
    va: *mut libc::c_void,
    len: usize,
    _memfd: std::fs::File,
}

// SAFETY: the filler's mapping is only touched by the RM (pinned) and unmapped on drop.
unsafe impl Send for Filler {}

impl Filler {
    fn new(gpa: u64, len: u64) -> Result<Self> {
        use std::os::fd::FromRawFd;
        const PIECE: usize = 2 << 20;
        let len = len as usize;
        // SAFETY: memfd_create with a static name; the fd is owned by the File below.
        let fd = unsafe { libc::memfd_create(c"caraxes-filler".as_ptr(), libc::MFD_CLOEXEC) };
        if fd < 0 {
            bail!("memfd_create: {}", std::io::Error::last_os_error());
        }
        // SAFETY: fd is a fresh memfd owned from here on.
        let memfd = unsafe { std::fs::File::from_raw_fd(fd) };
        memfd.set_len(PIECE as u64)?;
        // SAFETY: an anonymous PROT_NONE reservation, then MAP_FIXED pieces inside it.
        let va = unsafe {
            libc::mmap(std::ptr::null_mut(), len, libc::PROT_NONE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE, -1, 0)
        };
        if va == libc::MAP_FAILED {
            bail!("filler reservation: {}", std::io::Error::last_os_error());
        }
        for i in 0..len.div_ceil(PIECE) {
            let n = PIECE.min(len - i * PIECE);
            // SAFETY: inside the reservation above.
            let p = unsafe {
                libc::mmap(va.cast::<u8>().add(i * PIECE).cast(), n,
                    libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED | libc::MAP_FIXED,
                    memfd.as_raw_fd(), 0)
            };
            if p == libc::MAP_FAILED {
                let e = std::io::Error::last_os_error();
                // SAFETY: the reservation made above.
                unsafe { libc::munmap(va, len) };
                bail!("filler piece {i}: {e}");
            }
        }
        Ok(Filler { handle: 0, gpa, iova: 0, va, len, _memfd: memfd })
    }
}

impl Drop for Filler {
    fn drop(&mut self) {
        // SAFETY: the mapping made in `new`; the descriptor over it is freed before.
        unsafe { libc::munmap(self.va, self.len) };
    }
}

pub struct HostMemory {
    rm: RmClient,
    /// The GPU node the device allocation needs open, and the node registered to the
    /// client's ctl FD that `RM_ALLOC_MEMORY` runs on (host_pool.rs `Backing::new`).
    _gpu: NvDevice,
    gpu_reg: NvDevice,
    device: u32,
    subdevice: u32,
    privilege: Privilege,
    reservation: Option<Reservation>,
    descriptors: Vec<Descriptor>,
    /// Hole fillers of the linear registration (`register_linear`): descriptors over a
    /// device-owned 2 MiB page mapped many times, which only hold the IOVA space between
    /// guest RAM zones so the zones above keep IOVA = GPA + one offset. Never in the
    /// shadow filter's ranges and never in the guest's table.
    fillers: Vec<Filler>,
    /// Gather descriptors of the shadow (`gather`), live until the shadow frees them; the
    /// shadow teardown frees them before the guest-RAM descriptors (mirror.rs `wait_idle`).
    gathers: Vec<u32>,
    /// Guest RAM zones mapped by the client but not registered yet (the linear
    /// registration waits for all of them, `CARAXES_GUEST_RAM_BYTES`).
    pending: Vec<(u64, u64, usize)>,
    /// The mirror objects of E5b part 2 (`(parent, handle, what)`, allocation order):
    /// freed LIFO before the descriptors, so no host channel can still use an IOVA
    /// when its pages are unpinned (docs/caraxes/memory-model.md §4).
    live: Vec<(u32, u32, &'static str)>,
    /// `nr_foll_pin_acquired`/`_released` when the client opened (host-wide counters).
    pins_at_open: (u64, u64),
    shut: bool,
    /// Host frees that did not return NV_OK over the device's life (`free_object`,
    /// `free_live`), with a line each. Any entry makes the teardown fail closed
    /// ([`run_teardown`]): a host object whose free failed may still reference the
    /// guest's IOVAs or the reservation.
    free_failures: Vec<String>,
    /// Set when the teardown kept the registrations or the reservation (fail closed):
    /// the reason. The process must then exit non-zero (main.rs).
    fail_closed: Option<String>,
    /// The host VRAM floor (hostfloor.rs): every host allocation and map outside the
    /// reservation is admitted here first.
    guard: crate::hostfloor::HostGuard,
}

/// One step of the teardown that frees host state the GPU could still reach.
pub trait TeardownOps {
    /// Free the mirror objects (channels, TSGs, VA spaces); returns how many frees failed.
    fn free_mirrors(&mut self, log: &mut Vec<String>) -> usize;
    /// Guest RAM descriptors still registered.
    fn descriptor_count(&self) -> usize;
    /// Free the last registered descriptor (unpinning its guest RAM); false on failure,
    /// in which case the descriptor stays registered.
    fn free_last_descriptor(&mut self, log: &mut Vec<String>) -> bool;
    /// Scrub and free the reservation (if held); false on failure (kept).
    fn free_reservation(&mut self, log: &mut Vec<String>) -> bool;
    /// Free the host RM client; false on failure.
    fn free_client(&mut self, log: &mut Vec<String>) -> bool;
}

/// How a teardown ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeardownOutcome {
    /// Everything freed.
    Clean,
    /// Fail closed (P20 item 4a, the upstream rule "if ownership or completion is
    /// uncertain, keep the affected VRAM and channels out of the allocators",
    /// nova-vgpu-upstream.md §3.5): a free failed, so nothing after it was freed
    /// explicitly. `descriptors_kept` guest RAM chunks stay pinned, the reservation stays
    /// held when `reservation_kept`; the process exits non-zero and the close of
    /// `/dev/nvidiactl` lets the RM free the client in its own dependency order.
    FailClosed {
        reason: String,
        descriptors_kept: usize,
        reservation_kept: bool,
    },
}

/// The fail-closed teardown order: mirror objects, then the descriptors (unpinning guest
/// RAM), then the reservation, then the client. A failure at any step, or an earlier
/// failed host free (`prior_failures`), stops the sequence there: no descriptor is freed
/// after a failed mirror free, and the reservation is never freed after a failed unpin.
pub fn run_teardown<O: TeardownOps>(
    ops: &mut O,
    prior_failures: usize,
    log: &mut Vec<String>,
) -> TeardownOutcome {
    let failed = ops.free_mirrors(log) + prior_failures;
    let closed = |ops: &O, reason: String, reservation_kept: bool| TeardownOutcome::FailClosed {
        reason,
        descriptors_kept: ops.descriptor_count(),
        reservation_kept,
    };
    if failed > 0 {
        return closed(
            ops,
            format!("{failed} host object free(s) failed before the descriptors"),
            true,
        );
    }
    while ops.descriptor_count() > 0 {
        if !ops.free_last_descriptor(log) {
            return closed(ops, "a guest RAM descriptor free (unpin) failed".into(), true);
        }
    }
    if !ops.free_reservation(log) {
        return closed(ops, "the reservation free failed".into(), true);
    }
    if !ops.free_client(log) {
        return closed(ops, "the host RM client free failed".into(), false);
    }
    TeardownOutcome::Clean
}

/// One RM status, for callers that log the number rather than fail.
pub type Status = u32;

/// Drop every descendant of `handle` from a `live` list (`(parent, handle, what)`): the
/// RM frees an object's whole subtree. Grandchildren included: before, only the direct
/// children were dropped, so an engine object under a channel under a freed TSG stayed
/// listed, and the exit teardown freed a handle the RM no longer had (a free failure that
/// makes the teardown fail closed).
pub fn forget_subtree(live: &mut Vec<(u32, u32, &'static str)>, handle: u32) {
    let mut gone = vec![handle];
    while let Some(p) = gone.pop() {
        let mut i = 0;
        while i < live.len() {
            if live[i].0 == p {
                gone.push(live.remove(i).1);
            } else {
                i += 1;
            }
        }
    }
}

/// `NV2080_CTRL_FB_INFO_INDEX_BAR1_SIZE` and `_BAR1_AVAIL_SIZE` (ctrl2080fb.h:323, :351;
/// KiB), the host's BAR1 for the soak line.
const FB_INFO_BAR1_SIZE: u32 = 0x05;
const FB_INFO_BAR1_AVAIL_SIZE: u32 = 0x1D;

/// `NVOS47_FLAGS_DEFER_TLB_INVALIDATION_TRUE` (nvos.h:2192-2194, field 0:0).
const NVOS47_FLAGS_DEFER_TLB_INVALIDATION_TRUE: u32 = 1;
/// `NV0080_CTRL_CMD_DMA_INVALIDATE_TLB` (ctrl0080dma.h:495), params `{hVASpace, flags}`.
const CMD_DMA_INVALIDATE_TLB: u32 = 0x0080_180C;

fn meminfo_kib(key: &str) -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(key))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(0)
}

/// `nr_foll_pin_acquired`, `nr_foll_pin_released` (host-wide, batched per CPU).
pub fn foll_pins() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/vmstat").unwrap_or_default();
    let get = |k: &str| {
        s.lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0u64)
    };
    (get("nr_foll_pin_acquired "), get("nr_foll_pin_released "))
}

/// nvidia-smi's FB free for GPU 0, in bytes (a read-only NVML query).
fn smi_free() -> Result<u64> {
    let out = std::process::Command::new("nvidia-smi")
        .args([
            "-i",
            "0",
            "--query-gpu=memory.free",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .context("run nvidia-smi (the reservation rule reads its FB free)")?;
    if !out.status.success() {
        bail!("nvidia-smi exited with {}", out.status);
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let mib: u64 = s
        .trim()
        .parse()
        .with_context(|| format!("nvidia-smi FB free {s:?}"))?;
    Ok(mib * MIB)
}

/// `NV0041_CTRL_GET_SURFACE_PHYS_ATTR_PARAMS` as bytes (layout in `ver`).
#[repr(C, align(8))]
struct PhysAttr([u8; v::PHYS_ATTR_SIZE]);

impl HostMemory {
    /// A fresh host RM client with device 0 and subdevice 0.
    pub fn open(privilege: Privilege) -> Result<Self> {
        // The version check is start-up's host-ABI selection (hostabi.rs), not the
        // build default's assertion in `RmClient::new` (which panics on 615.71.09).
        crate::hostabi::selected().map_err(|e| anyhow::anyhow!(e))?;
        let mut rm = RmClient::open_without_version_check()
            .map_err(|e| anyhow::anyhow!("host RM client: {e}"))?;
        let gpu = NvDevice::open_gpu(0).map_err(|e| anyhow::anyhow!("open /dev/nvidia0: {e}"))?;
        let gpu_reg = gpu
            .open_for_mapping(rm.ctl())
            .map_err(|e| anyhow::anyhow!("REGISTER_FD: {e}"))?;
        let root = rm.root();
        let device = rm.next_handle();
        let mut dp = sys::NV0080_ALLOC_PARAMETERS::default();
        dp.deviceId = 0;
        // SAFETY: NV0080_ALLOC_PARAMETERS matches NV01_DEVICE_0; no embedded buffers.
        unsafe { rm.alloc(root, device, sys::NV01_DEVICE_0, Some(&mut dp)) }
            .map_err(|e| anyhow::anyhow!("host NV01_DEVICE_0: {e}"))?;
        let subdevice = rm.next_handle();
        let mut sp = sys::NV2080_ALLOC_PARAMETERS::default();
        sp.subDeviceId = 0;
        // SAFETY: NV2080_ALLOC_PARAMETERS matches NV20_SUBDEVICE_0; no embedded buffers.
        unsafe { rm.alloc(device, subdevice, sys::NV20_SUBDEVICE_0, Some(&mut sp)) }
            .map_err(|e| anyhow::anyhow!("host NV20_SUBDEVICE_0: {e}"))?;
        Ok(HostMemory {
            rm,
            _gpu: gpu,
            gpu_reg,
            device,
            subdevice,
            privilege,
            reservation: None,
            descriptors: Vec::new(),
            fillers: Vec::new(),
            gathers: Vec::new(),
            pending: Vec::new(),
            live: Vec::new(),
            pins_at_open: foll_pins(),
            shut: false,
            free_failures: Vec::new(),
            fail_closed: None,
            guard: crate::hostfloor::HostGuard::from_env(),
        })
    }

    pub fn privilege(&self) -> &Privilege {
        &self.privilege
    }

    pub fn device(&self) -> u32 {
        self.device
    }

    pub fn subdevice(&self) -> u32 {
        self.subdevice
    }

    pub fn root(&self) -> u32 {
        self.rm.root()
    }

    /// The mirror objects held right now, allocation order.
    pub fn live(&self) -> &[(u32, u32, &'static str)] {
        &self.live
    }

    // ---- raw RM escapes for the mirror objects (mirror.rs) ------------------------

    /// `RM_ALLOC` of `class` under `parent` with `params` (empty: no parameters); the
    /// object is tracked in `live` and freed by [`HostMemory::shutdown`] unless
    /// [`HostMemory::free_object`] frees it first.
    pub fn alloc_object(
        &mut self,
        parent: u32,
        class: u32,
        params: &mut [u8],
        what: &'static str,
    ) -> Result<u32, Status> {
        self.admit_class(class, what)?;
        let h = self.rm.next_handle();
        let mut p = sys::NVOS64_PARAMETERS::default();
        p.hRoot = self.rm.root();
        p.hObjectParent = parent;
        p.hObjectNew = h;
        p.hClass = class;
        p.pAllocParms = if params.is_empty() {
            0usize as sys::NvP64
        } else {
            params.as_mut_ptr() as usize as sys::NvP64
        };
        p.pRightsRequested = 0usize as sys::NvP64;
        // SAFETY: NVOS64 is the escape ABI; `params` is the class's parameter block as
        // the guest sent it (validated by the guest RM) or as this crate built it, and
        // RM derives the size from the class.
        if unsafe { self.rm.ctl().ioctl_raw(sys::NV_ESC_RM_ALLOC, &mut p) }.is_err() {
            return Err(0xFFFF_FFFF);
        }
        if p.status as u32 != sys::NV_OK {
            self.guard.note_status(what, p.status as u32, Instant::now());
            return Err(p.status as u32);
        }
        self.live.push((parent, h, what));
        Ok(h)
    }

    /// `alloc_object` inside one CAP_SYS_ADMIN window (D12). Turing derives a channel's
    /// AUTH_LEVEL from the allocating client's privilege at alloc time
    /// (`kchannelConstruct`, kernel_channel.c:257-287 — a GSP-client channel takes the
    /// caller's `rmclientIsAdmin`, which reads the per-ioctl `secInfo.privLevel` set from
    /// `osIsAdministrator()` = CAP_SYS_ADMIN, escape.c:381), so a privileged host method
    /// such as `MEM_OP MMU_TLB_INVALIDATE` (invalidate.rs) is only accepted on a channel
    /// allocated here. The window covers exactly this one `NV_ESC_RM_ALLOC`.
    pub fn alloc_object_admin(
        &mut self,
        parent: u32,
        class: u32,
        params: &mut [u8],
        what: &'static str,
    ) -> Result<u32, Status> {
        self.admit_class(class, what)?;
        let h = self.rm.next_handle();
        let mut p = sys::NVOS64_PARAMETERS::default();
        p.hRoot = self.rm.root();
        p.hObjectParent = parent;
        p.hObjectNew = h;
        p.hClass = class;
        p.pAllocParms = if params.is_empty() {
            0usize as sys::NvP64
        } else {
            params.as_mut_ptr() as usize as sys::NvP64
        };
        p.pRightsRequested = 0usize as sys::NvP64;
        let rm = &self.rm;
        // SAFETY: as `alloc_object`; the admin window is raised only around this ioctl.
        let ioctl = self
            .privilege
            .with_admin(|| unsafe { rm.ctl().ioctl_raw(sys::NV_ESC_RM_ALLOC, &mut p) });
        match ioctl {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Err(0xFFFF_FFFF),
            Err(_) => return Err(0xFFFF_FFFD),
        }
        if p.status as u32 != sys::NV_OK {
            self.guard.note_status(what, p.status as u32, Instant::now());
            return Err(p.status as u32);
        }
        self.live.push((parent, h, what));
        Ok(h)
    }

    /// The floor's admission of an allocation of `class` (hostfloor.rs). Event and
    /// usermode objects hold no VRAM and pass; everything else (VA spaces with their page
    /// directories, virtual objects, TSGs, channels with their instance blocks and
    /// context buffers, engine objects) is checked.
    fn admit_class(&self, class: u32, what: &str) -> Result<(), Status> {
        if class == sys::NV01_EVENT_OS_EVENT || class == sys::VOLTA_USERMODE_A {
            return Ok(());
        }
        self.admit(what)
    }

    /// Admit one host allocation or map outside the reservation (hostfloor.rs): the
    /// host's free VRAM, cached for a short TTL, against the floor.
    pub fn admit(&self, what: &str) -> Result<(), Status> {
        self.guard.admit(what, Instant::now(), || self.heap_free())
    }

    /// The host's `HEAP_FREE` in bytes (read-only `FB_GET_INFO_V2`), `None` when the
    /// control fails.
    pub fn heap_free(&self) -> Option<u64> {
        self.fb_info(&[v::FB_INFO_HEAP_FREE]).ok().map(|v| v[0] * KIB)
    }

    /// The floor's counters and its last free size.
    pub fn guard(&self) -> &crate::hostfloor::HostGuard {
        &self.guard
    }

    /// The host's VRAM and BAR1 figures in bytes, read-only (`FB_GET_INFO_V2`): total,
    /// heap free, BAR1 size, BAR1 available. For the soak line (soak.rs).
    pub fn host_figures(&self) -> Option<crate::soak::HostFigures> {
        let i = self
            .fb_info(&[
                v::FB_INFO_TOTAL_RAM_SIZE,
                v::FB_INFO_HEAP_FREE,
                FB_INFO_BAR1_SIZE,
                FB_INFO_BAR1_AVAIL_SIZE,
            ])
            .ok()?;
        Some(crate::soak::HostFigures {
            vram_total: i[0] * KIB,
            vram_free: i[1] * KIB,
            bar1_total: i[2] * KIB,
            bar1_free: i[3] * KIB,
        })
    }

    /// What this client holds on the host, for the soak line (soak.rs): the RM objects by
    /// group, the registered guest RAM, the gathers, the pins and the floor's counters.
    pub fn soak_held(&self) -> crate::soak::HostHeld {
        let mut objects = std::collections::BTreeMap::new();
        for (_, _, what) in &self.live {
            *objects.entry(crate::soak::group_of(what)).or_insert(0) += 1;
        }
        let g = self.guard.stats();
        crate::soak::HostHeld {
            objects,
            descriptors: self.descriptors.len() + self.fillers.len(),
            pinned_bytes: self.pinned_bytes(),
            gathers: self.gathers.len(),
            foll_pin: self.foll_pin_delta(),
            floor_refused: g.refused(),
            no_memory: g.rm_no_memory,
        }
    }

    /// `nr_foll_pin_acquired - nr_foll_pin_released` now minus the same at open (host-wide
    /// counters; the other VMs' pins move it too).
    pub fn foll_pin_delta(&self) -> i64 {
        let (a, r) = foll_pins();
        let (a0, r0) = self.pins_at_open;
        (a as i64 - r as i64) - (a0 as i64 - r0 as i64)
    }

    /// Allocate an `NV50_MEMORY_VIRTUAL` reservation of `size` inside `vaspace` and
    /// return `(handle, base VA)` (RM writes the base back into the alloc params). The
    /// shadow VA space (`shadow.rs`) maps the guest's set-A into this range at fixed VAs.
    pub fn alloc_virtual(&mut self, vaspace: u32, size: u64) -> Result<(u32, u64), Status> {
        let mut p = sys::NV_MEMORY_ALLOCATION_PARAMS::default();
        p.owner = u32::from_le_bytes(*b"cara");
        p.type_ = sys::NVOS32_TYPE_IMAGE as u32;
        p.flags = sys::NVOS32_ALLOC_FLAGS_VIRTUAL as u32;
        p.attr = nvos32_attr::LOCATION.set(sys::NVOS32_ATTR_LOCATION_PCI as u32)
            | nvos32_attr::PAGE_SIZE.set(sys::NVOS32_ATTR_PAGE_SIZE_4KB as u32);
        p.size = size;
        p.hVASpace = vaspace;
        let device = self.device();
        // SAFETY: plain-data alloc params viewed as bytes for the escape ABI.
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                &mut p as *mut _ as *mut u8,
                std::mem::size_of::<sys::NV_MEMORY_ALLOCATION_PARAMS>(),
            )
        };
        let h = self.alloc_object(device, sys::NV50_MEMORY_VIRTUAL, bytes, "shadow-virtual")?;
        Ok((h, p.offset))
    }

    /// Allocate an `NV50_MEMORY_VIRTUAL` reservation of `size` **fixed at VA `va`** inside
    /// `vaspace`. `page_bytes` is the page-size attribute: 4 KiB, 64 KiB or 2 MiB, or
    /// `None` for `PAGE_SIZE_DEFAULT` (RM locks both 4 KiB and big pages for the range,
    /// the page size is chosen at each map, `gvaspaceApplyDefaultAlignment_IMPL`). A
    /// `DMA_OFFSET_FIXED` map only lands inside the VA range a virtual object reserved
    /// (`dma.c:157-158`, NV_ERR_INVALID_OFFSET otherwise), so the shadow route reserves
    /// one object per 2 MiB region at its VA. `force_align` sets
    /// `ALIGNMENT_FORCE` with the page size as alignment, for an object smaller than the
    /// VAS's compression-page alignment (legal because nothing here is ever compressed,
    /// D16). Returns the handle, or the status (a refusal is a VA collision with the
    /// server-reserved range or the host RM's own context VAs).
    pub fn alloc_virtual_fixed(
        &mut self,
        vaspace: u32,
        va: u64,
        size: u64,
        page_bytes: Option<u64>,
        force_align: bool,
    ) -> Result<u32, Status> {
        let (page_attr, attr2) = match page_bytes {
            None => (sys::NVOS32_ATTR_PAGE_SIZE_DEFAULT, 0),
            Some(0x1000) => (sys::NVOS32_ATTR_PAGE_SIZE_4KB, 0),
            Some(0x1_0000) => (sys::NVOS32_ATTR_PAGE_SIZE_BIG, 0),
            Some(_) => (sys::NVOS32_ATTR_PAGE_SIZE_HUGE, v::ATTR2_PAGE_SIZE_HUGE_2MB),
        };
        let mut p = sys::NV_MEMORY_ALLOCATION_PARAMS::default();
        p.owner = OWNER_TAG;
        p.type_ = sys::NVOS32_TYPE_IMAGE as u32;
        p.flags =
            sys::NVOS32_ALLOC_FLAGS_VIRTUAL as u32 | sys::NVOS32_ALLOC_FLAGS_FIXED_ADDRESS_ALLOCATE;
        if force_align {
            p.flags |= sys::NVOS32_ALLOC_FLAGS_ALIGNMENT_FORCE;
            p.alignment = page_bytes.unwrap_or(0x1000);
        }
        p.attr = nvos32_attr::LOCATION.set(sys::NVOS32_ATTR_LOCATION_PCI as u32)
            | nvos32_attr::PAGE_SIZE.set(page_attr);
        p.attr2 = attr2;
        p.size = size;
        p.offset = va;
        p.hVASpace = vaspace;
        let device = self.device();
        // SAFETY: plain-data alloc params viewed as bytes for the escape ABI.
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                &mut p as *mut _ as *mut u8,
                std::mem::size_of::<sys::NV_MEMORY_ALLOCATION_PARAMS>(),
            )
        };
        self.alloc_object(
            device,
            sys::NV50_MEMORY_VIRTUAL,
            bytes,
            "shadow-virtual-fixed",
        )
    }

    /// `NV50_MEMORY_VIRTUAL` of `size` bytes (4 KiB pages, 4 KiB aligned) wherever the VA
    /// space's allocator puts it (not fixed): its handle and VA. The shadow's probe of the
    /// host RM's first fit (mirror.rs `place_video_object`); the caller frees it.
    pub fn alloc_virtual_probe(&mut self, vaspace: u32, size: u64) -> Result<(u32, u64), Status> {
        let mut p = sys::NV_MEMORY_ALLOCATION_PARAMS::default();
        p.owner = OWNER_TAG;
        p.type_ = sys::NVOS32_TYPE_IMAGE as u32;
        p.flags = sys::NVOS32_ALLOC_FLAGS_VIRTUAL as u32 | sys::NVOS32_ALLOC_FLAGS_ALIGNMENT_FORCE;
        p.alignment = 0x1000;
        p.attr = nvos32_attr::LOCATION.set(sys::NVOS32_ATTR_LOCATION_PCI as u32)
            | nvos32_attr::PAGE_SIZE.set(sys::NVOS32_ATTR_PAGE_SIZE_4KB);
        p.size = size;
        p.hVASpace = vaspace;
        let device = self.device();
        // SAFETY: plain-data alloc params viewed as bytes for the escape ABI; RM writes the
        // chosen VA back into `offset` (virtual_mem.c:609).
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                &mut p as *mut _ as *mut u8,
                std::mem::size_of::<sys::NV_MEMORY_ALLOCATION_PARAMS>(),
            )
        };
        let h = self.alloc_object(device, sys::NV50_MEMORY_VIRTUAL, bytes, "shadow-virtual-probe")?;
        Ok((h, p.offset))
    }

    /// `RM_UNMAP_MEMORY_DMA` (NVOS47) of the mapping of `h_mem` at VA `va`, `length`
    /// bytes, from the virtual object `h_dma` (the inverse of [`Self::map_dma_fixed`];
    /// RM also splits a larger mapping, `virtmemUnmapFrom_IMPL`). RM invalidates the
    /// TLB of the VAS itself unless `defer` sets `NVOS47_FLAGS_DEFER_TLB_INVALIDATION`
    /// (nvos.h:2192, bit 0; honoured by `dmaFreeMapping_GM107`,
    /// virt_mem_allocator_gm107.c:1579); the caller then owes the VAS one
    /// [`Self::invalidate_vas`] before anything relies on the unmap. Returns the status.
    pub fn unmap_dma(&self, h_dma: u32, h_mem: u32, va: u64, length: u64, defer: bool) -> Status {
        let mut p = sys::NVOS47_PARAMETERS::default();
        p.hClient = self.rm.root();
        p.hDevice = self.device();
        p.hDma = h_dma;
        p.hMemory = h_mem;
        p.flags = u32::from(defer) * NVOS47_FLAGS_DEFER_TLB_INVALIDATION_TRUE;
        p.dmaOffset = va;
        p.size = length;
        // SAFETY: NVOS47 is the escape ABI; plain-data params, no host pointers.
        match unsafe {
            self.rm
                .ctl()
                .ioctl_raw(sys::NV_ESC_RM_UNMAP_MEMORY_DMA, &mut p)
        } {
            Ok(()) => p.status as u32,
            Err(_) => 0xFFFF_FFFF,
        }
    }

    /// `RM_MAP_MEMORY_DMA` (NVOS46) of `h_mem` at `[offset, offset+length)` into `h_dma`
    /// (a virtual reservation, or a VA space) at the **fixed** VA `va`, with the given
    /// `page_size` flag (`nvos46_flags::PAGE_SIZE_*`). Snoop for sysmem, off for vidmem.
    /// Read/write, GPU-uncached. `defer` sets `NVOS46_FLAGS_DEFER_TLB_INVALIDATION` (nvos.h:2151,
    /// bit 31): RM then skips its flush and VAS TLB invalidate after the PTE writes
    /// (`dmaUpdateVASpace_GF100`, virt_mem_allocator_gm107.c:2567-2577), and the caller owes
    /// the VAS one [`Self::invalidate_vas`]. Returns the status.
    #[allow(clippy::too_many_arguments)]
    pub fn map_dma_fixed(
        &self,
        h_dma: u32,
        h_mem: u32,
        offset: u64,
        length: u64,
        va: u64,
        page_size: u32,
        snoop: bool,
        kind: Option<u8>,
        defer: bool,
    ) -> Status {
        use nvrm_abi::nvgpu::nvos46_flags as f;
        let mut p = sys::NVOS46_PARAMETERS::default();
        p.hClient = self.rm.root();
        p.hDevice = self.device();
        p.hDma = h_dma;
        p.hMemory = h_mem;
        p.offset = offset;
        p.length = length;
        p.flags = f::ACCESS.set(f::ACCESS_READ_WRITE)
            | f::PAGE_SIZE.set(page_size)
            | f::CACHE_SNOOP.set(if snoop {
                f::CACHE_SNOOP_ENABLE
            } else {
                f::CACHE_SNOOP_DISABLE
            })
            | f::DMA_OFFSET_FIXED.set(f::DMA_OFFSET_FIXED_TRUE)
            | f::DEFER_TLB_INVALIDATION.set(u32::from(defer));
        // A guest depth/stencil kind (shadow.rs `kind_override`): NVOS46_FLAGS_PAGE_KIND_OVERRIDE
        // (nvos.h:2114) with `kindOverride` (nvos.h:2178).
        if let Some(k) = kind {
            p.flags |= f::PAGE_KIND_OVERRIDE.set(1);
            p.kindOverride = k as u32;
        }
        p.dmaOffset = va;
        // The map may allocate page tables in host VRAM: the floor first (hostfloor.rs).
        if let Err(st) = self.admit("shadow map") {
            return st;
        }
        // SAFETY: NVOS46 is the escape ABI; plain-data params, no host pointers.
        // The caller (shadow.rs) logs a refusal once per leaf, with VA, PA and reason.
        let st = match unsafe {
            self.rm
                .ctl()
                .ioctl_raw(sys::NV_ESC_RM_MAP_MEMORY_DMA, &mut p)
        } {
            Ok(()) => p.status as u32,
            Err(_) => 0xFFFF_FFFF,
        };
        self.guard.note_status("shadow map", st, Instant::now());
        st
    }

    /// `NV0080_CTRL_CMD_DMA_INVALIDATE_TLB` (ctrl0080dma.h:467-495) on the device for the VA
    /// space `vas`, flags 0 (`_ALL_FALSE`: only that VAS). NON_PRIVILEGED, runs in the kernel
    /// RM (flags 0x10008, g_device_nvoc.c:1144), where `deviceCtrlCmdDmaInvalidateTLB_IMPL`
    /// (dma.c:967-1010) calls `vaspaceInvalidateTlb(PTE_DOWNGRADE)`: the whole VAS
    /// (`_ALL_VA`, kern_gmmu_gm107.c:194), with the membar of a downgrade. The header names
    /// it the companion of the `DEFER_TLB_INVALIDATION` map and unmap flags. The walker root
    /// it invalidates is the root host channels run on for a normal host-managed VAS (the
    /// shadow's; not for an externally-owned one, invalidate.rs). Returns the status.
    pub fn invalidate_vas(&self, vas: u32) -> Status {
        let mut p = [0u8; 8];
        p[0..4].copy_from_slice(&vas.to_le_bytes());
        self.control(self.device(), CMD_DMA_INVALIDATE_TLB, &mut p)
    }

    /// `RM_CONTROL` `cmd` on `object`; returns the status (0xFFFFFFFF when the ioctl failed).
    pub fn control(&self, object: u32, cmd: u32, params: &mut [u8]) -> Status {
        let mut p = sys::NVOS54_PARAMETERS::default();
        p.hClient = self.rm.root();
        p.hObject = object;
        p.cmd = cmd;
        p.params = if params.is_empty() {
            std::ptr::null_mut()
        } else {
            params.as_mut_ptr() as usize as sys::NvP64
        };
        p.paramsSize = params.len() as u32;
        // SAFETY: NVOS54 is the escape ABI; `params` is the control's parameter block.
        match unsafe { self.rm.ctl().ioctl_raw(sys::NV_ESC_RM_CONTROL, &mut p) } {
            Ok(()) => p.status as u32,
            Err(_) => 0xFFFF_FFFF,
        }
    }

    /// A PRIVILEGED control (D12): `cmd` inside one CAP_SYS_ADMIN window.
    pub fn control_admin(&self, object: u32, cmd: u32, params: &mut [u8]) -> Result<Status> {
        self.privilege
            .with_admin(|| self.control(object, cmd, params))
    }

    /// `RM_FREE` of a tracked object (and of everything RM holds under it).
    pub fn free_object(&mut self, handle: u32) -> Result<(), Status> {
        let Some(i) = self.live.iter().rposition(|(_, h, _)| *h == handle) else {
            return Err(0xFFFF_FFFE);
        };
        let (parent, _, what) = self.live.remove(i);
        // RM frees the subtree; forget all of it this side too.
        forget_subtree(&mut self.live, handle);
        let r = self.rm_free(parent, handle);
        if let Err(st) = r {
            self.free_failures
                .push(format!("host {what} {handle:#x}: status {st:#x}"));
        }
        r
    }

    /// Host frees that failed so far (each makes the teardown fail closed).
    pub fn free_failures(&self) -> &[String] {
        &self.free_failures
    }

    /// Why the teardown failed closed, if it did: the process must exit non-zero.
    pub fn fail_closed(&self) -> Option<&str> {
        self.fail_closed.as_deref()
    }

    fn rm_free(&self, parent: u32, handle: u32) -> Result<(), Status> {
        let mut p = sys::NVOS00_PARAMETERS::default();
        p.hRoot = self.rm.root();
        p.hObjectParent = parent;
        p.hObjectOld = handle;
        // SAFETY: NVOS00 carries only handles and status.
        if unsafe { self.rm.ctl().ioctl_raw(sys::NV_ESC_RM_FREE, &mut p) }.is_err() {
            return Err(0xFFFF_FFFF);
        }
        if p.status as u32 == sys::NV_OK {
            Ok(())
        } else {
            Err(p.status as u32)
        }
    }

    /// Free every mirror object, newest first (channels before their TSGs and VA
    /// spaces). Returns one line per object.
    pub fn free_live(&mut self) -> Vec<String> {
        let mut log = Vec::new();
        while let Some((parent, h, what)) = self.live.pop() {
            match self.rm_free(parent, h) {
                Ok(()) => log.push(format!("freed host {what} {h:#x}")),
                Err(st) => {
                    log.push(format!("ERROR freeing host {what} {h:#x}: status {st:#x}"));
                    self.free_failures
                        .push(format!("host {what} {h:#x}: status {st:#x}"));
                }
            }
        }
        log
    }

    /// An OS descriptor over `len` bytes of the device's own mapping of guest RAM at
    /// `host_va` (a guest USERD or error notifier in sysmem, cuda-path.md §4.1).
    pub fn guest_page_object(
        &mut self,
        host_va: *mut u8,
        len: u64,
        what: &'static str,
    ) -> Result<u32> {
        let h = self.os_descriptor(host_va, len)?;
        self.live.push((self.device, h, what));
        Ok(h)
    }

    /// A sampled read of the whole reservation through BAR1 (P20 item 4b): every 16 MiB
    /// a 2 MiB window, its first 64 KiB read; counts the P20 pattern records and the
    /// non-zero dwords. Only with `CARAXES_P20_SCAN=1` (at the reservation and before its
    /// free). Read-only; the windows are dropped before returning.
    pub fn scan_reservation(&mut self, when: &str) -> Option<String> {
        if std::env::var("CARAXES_P20_SCAN").as_deref() != Ok("1") {
            return None;
        }
        let r = self.reservation.clone()?;
        let t = Instant::now();
        let (mut records, mut nonzero, mut bytes, mut windows, mut refused) = (0u64, 0u64, 0u64, 0u32, 0u32);
        let mut hit_offsets = Vec::new();
        let mut buf = vec![0u8; SCAN_READ];
        let mut off = 0;
        while off < r.size {
            match self.map_reservation(off, (2 * MIB) as usize) {
                Ok(w) => {
                    windows += 1;
                    w.read(off, &mut buf);
                    let (rec, nz) = count_pattern(&buf);
                    if rec > 0 && hit_offsets.len() < 8 {
                        hit_offsets.push(off);
                    }
                    records += rec;
                    nonzero += nz;
                    bytes += SCAN_READ as u64;
                }
                Err(_) => refused += 1,
            }
            off += SCAN_STEP;
        }
        Some(format!(
            "P20 scan {when}: reservation [{:#x}, {:#x}) sampled {} KiB in {windows} window(s) ({refused} refused): {records} pattern record(s), {nonzero} non-zero dword(s) of {}{} in {} ms",
            r.base,
            r.end(),
            bytes / KIB,
            bytes / 4,
            if hit_offsets.is_empty() {
                String::new()
            } else {
                format!(
                    " (records at offsets {})",
                    hit_offsets
                        .iter()
                        .map(|o| format!("{o:#x}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            },
            t.elapsed().as_millis()
        ))
    }

    /// Map `len` bytes of the reservation at reservation offset `offset` through BAR1.
    pub fn map_reservation(&mut self, offset: u64, len: usize) -> Result<Window, Status> {
        let r = self.reservation.as_ref().ok_or(0xFFFF_FFFEu32)?;
        map_window(
            self.rm.ctl(),
            &self._gpu,
            self.rm.root(),
            self.device,
            r.handle,
            offset,
            len,
        )
    }

    /// A mapper for BAR1 windows of the reservation that holds its own duplicates of the
    /// client's FDs, so the FB store maps windows without the `HostMemory` lock. The
    /// shadow sync and the invalidator hold that lock while they walk the FB; a window
    /// map from inside the walk locked it again and deadlocked the server thread once
    /// BAR1 ran short (P20, docs/caraxes/parallel.md §2).
    pub fn reservation_mapper(&self) -> Result<ReservationMapper, Status> {
        let r = self.reservation.as_ref().ok_or(0xFFFF_FFFEu32)?;
        Ok(ReservationMapper {
            ctl: self.rm.ctl().try_clone().map_err(|_| 0xFFFF_FFFFu32)?,
            gpu: self._gpu.try_clone().map_err(|_| 0xFFFF_FFFFu32)?,
            root: self.rm.root(),
            device: self.device,
            handle: r.handle,
        })
    }

    /// P21: export `[fb, fb + len)` of the reservation as a dma-buf for the guest's
    /// BAR1 region (`Bar1Export`). The range must lie inside the reservation; the
    /// caller has checked it against the usable part (`fb::bar1_passthrough_area`).
    pub fn export_bar1_passthrough(&self, fb: u64, len: u64) -> Result<Bar1Export> {
        let r = self
            .reservation
            .as_ref()
            .context("BAR1 passthrough needs the reservation")?;
        if fb < r.base || fb + len > r.base + r.size || len == 0 {
            bail!(
                "BAR1 passthrough [{fb:#x}, {:#x}) is not inside the reservation [{:#x}, {:#x})",
                fb + len,
                r.base,
                r.base + r.size
            );
        }
        let fd = export_dmabuf(&self._gpu, self.rm.root(), r.handle, fb - r.base, len)?;
        Ok(Bar1Export { fd, fb, len })
    }

    /// The host doorbell page: `VOLTA_USERMODE_A` under the subdevice, mapped write-only
    /// (nvrm-client `map_doorbell`). Returns the object handle and the mapping.
    pub fn map_doorbell(&mut self) -> Result<(u32, CpuMap)> {
        let h = self
            .alloc_object(self.subdevice, sys::VOLTA_USERMODE_A, &mut [], "usermode")
            .map_err(|st| anyhow::anyhow!("VOLTA_USERMODE_A: status {st:#x}"))?;
        let map = nvrm_client::mem::map_doorbell(&self.rm, &self._gpu, self.subdevice, h)
            .map_err(|e| anyhow::anyhow!("map the host doorbell: {e}"))?;
        Ok((h, map))
    }

    /// A host OS event on the subdevice's `notifier` (an `NV2080_NOTIFIERS_*` index, with
    /// `NV01_EVENT_NONSTALL_INTR` or-ed in for the engine's non-stall interrupt), armed to
    /// repeat. The event is bound to its own ctl fd, which `poll` then wakes.
    pub fn os_event(&mut self, notifier: u32, id: u32) -> Result<OsEvent> {
        let ctl = NvDevice::open_ctl().map_err(|e| anyhow::anyhow!("event ctl: {e}"))?;
        let mut a = nvrm_abi::nvgpu::IoctlAllocOsEvent {
            h_client: self.rm.root(),
            h_device: self.subdevice,
            fd: id,
            status: 0,
        };
        // SAFETY: nv_ioctl_alloc_os_event_t is this escape's ABI.
        unsafe { ctl.ioctl_raw(nvrm_abi::nvgpu::NV_ESC_ALLOC_OS_EVENT, &mut a) }
            .map_err(|e| anyhow::anyhow!("ALLOC_OS_EVENT: {e}"))?;
        if a.status != sys::NV_OK {
            bail!("ALLOC_OS_EVENT: status {:#x}", a.status);
        }
        let mut ep = sys::NV0005_ALLOC_PARAMETERS::default();
        ep.hParentClient = self.rm.root();
        ep.hSrcResource = self.subdevice;
        ep.hClass = sys::NV01_EVENT_OS_EVENT;
        ep.notifyIndex = notifier;
        ep.data = id as usize as sys::NvP64;
        let size = std::mem::size_of::<sys::NV0005_ALLOC_PARAMETERS>();
        // SAFETY: a plain-data parameter block viewed as bytes for the raw alloc.
        let bytes = unsafe { std::slice::from_raw_parts_mut(&mut ep as *mut _ as *mut u8, size) };
        let handle = self
            .alloc_object(self.subdevice, sys::NV01_EVENT_OS_EVENT, bytes, "os-event")
            .map_err(|st| {
                anyhow::anyhow!("NV01_EVENT_OS_EVENT notifier {notifier:#x}: status {st:#x}")
            })?;
        // NV2080_CTRL_EVENT_SET_NOTIFICATION_PARAMS {event u32 @0, action u32 @4,
        // bNotifyState u8 @8, info32 u32 @12, info16 u16 @16} = 20 bytes
        // (ctrl2080event.h; host-exec.rs EventSetNotification); action REPEAT = 2.
        let mut sn = [0u8; 20];
        sn[0..4].copy_from_slice(&(notifier & 0xFFFF).to_le_bytes());
        sn[4..8].copy_from_slice(&2u32.to_le_bytes());
        sn[8] = 1;
        let st = self.control(self.subdevice, v::CMD_EVENT_SET_NOTIFICATION, &mut sn);
        if st != sys::NV_OK {
            bail!("EVENT_SET_NOTIFICATION notifier {notifier:#x}: status {st:#x}");
        }
        Ok(OsEvent {
            ctl,
            id,
            handle,
            notifier,
        })
    }

    pub fn reservation(&self) -> Option<&Reservation> {
        self.reservation.as_ref()
    }

    pub fn descriptors(&self) -> &[Descriptor] {
        &self.descriptors
    }

    pub fn pinned_bytes(&self) -> u64 {
        self.descriptors.iter().map(|d| d.range.size).sum()
    }

    /// `FB_GET_INFO_V2` for `indices` (KiB values).
    fn fb_info(&self, indices: &[u32]) -> Result<Vec<u64>> {
        let mut p = sys::NV2080_CTRL_FB_GET_INFO_V2_PARAMS::default();
        p.fbInfoListSize = indices.len() as u32;
        for (i, &x) in indices.iter().enumerate() {
            p.fbInfoList[i].index = x;
        }
        // SAFETY: the typed parameter struct of this control; plain data.
        unsafe {
            self.rm
                .control(self.subdevice, v::CMD_FB_GET_INFO_V2, &mut p)
        }
        .map_err(|e| anyhow::anyhow!("FB_GET_INFO_V2: {e}"))?;
        Ok((0..indices.len())
            .map(|i| p.fbInfoList[i].data as u64)
            .collect())
    }

    /// `FB_GET_FB_REGION_INFO`: (base, limit, reserved) per region.
    fn fb_regions(&self) -> Result<Vec<(u64, u64, u64)>> {
        let mut p = v::FbRegionInfoParams::default();
        // SAFETY: the typed parameter struct of this control; plain data.
        unsafe {
            self.rm
                .control(self.subdevice, v::CMD_FB_GET_FB_REGION_INFO, &mut p)
        }
        .map_err(|e| anyhow::anyhow!("FB_GET_FB_REGION_INFO: {e}"))?;
        let n = (p.numFBRegions as usize).min(p.fbRegion.len());
        Ok(p.fbRegion[..n]
            .iter()
            .map(|r| (r.base, r.limit, r.reserved))
            .collect())
    }

    /// Reserve `size` bytes of VRAM at the top of the host's FB heap region by the rule
    /// of host-facts.md §3: `[E - S, E)` with E the region end aligned down to 2 MiB,
    /// refused unless `S + 1 GiB` is free by both `HEAP_FREE` and nvidia-smi, fixed
    /// address and physically contiguous, no floating fallback, the reply checked.
    /// `slot` n places it n profile sizes lower, `[E - (n+1)S, E - nS)` (P20: one slot
    /// per VM, stacked downward; slot 0 is the range above). Returns the log lines.
    pub fn reserve(&mut self, size: u64, slot: u32) -> Result<Vec<String>> {
        let mut log = Vec::new();
        if self.reservation.is_some() {
            bail!("a reservation is already held");
        }
        let s = size.div_ceil(HUGE) * HUGE;
        let regions = self.fb_regions()?;
        log.push(format!(
            "host FB regions: {}",
            regions
                .iter()
                .map(|(b, l, r)| format!("[{b:#x}, {:#x}] reserved {r:#x}", l))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let &(heap_base, heap_limit, _) = regions
            .iter()
            .rfind(|r| r.2 == 0)
            .context("the host reports no non-reserved FB region")?;
        let region_end = heap_limit + 1;
        let (base, e) =
            slot_range(heap_base, region_end, s, slot).map_err(|e| anyhow::anyhow!(e))?;
        let info = self.fb_info(&[
            v::FB_INFO_TOTAL_RAM_SIZE,
            v::FB_INFO_HEAP_FREE,
            v::FB_INFO_HEAP_BASE_KB,
            v::FB_INFO_LARGEST_FREE_REGION_SIZE_KB,
            v::FB_INFO_LARGEST_FREE_REGION_BASE_KB,
        ])?;
        let (total, heap_free) = (info[0] * KIB, info[1] * KIB);
        // LARGEST_FREE_REGION_BASE_KB is relative to the PMA region at HEAP_BASE_KB
        // (host-facts.md H3).
        let largest = format!(
            "largest free region {} MiB at {:#x}",
            info[3] / 1024,
            (info[2] + info[4]) * KIB
        );
        let smi_free = smi_free()?;
        let f = heap_free.min(smi_free);
        log.push(format!(
            "host VRAM: total {} MiB, HEAP_FREE {} MiB, nvidia-smi free {} MiB, {largest}; request {} MiB at [{base:#x}, {e:#x}) (slot {slot}, heap region end {region_end:#x})",
            total / MIB,
            heap_free / MIB,
            smi_free / MIB,
            s / MIB
        ));
        // The floor of the whole device life (hostfloor.rs) or the 1 GiB headroom,
        // whichever keeps more.
        self.guard
            .check_reservation(s, f, VRAM_HEADROOM)
            .map_err(|e| anyhow::anyhow!(e))?;
        let mut p = sys::NV_MEMORY_ALLOCATION_PARAMS::default();
        p.owner = OWNER_TAG;
        p.type_ = sys::NVOS32_TYPE_IMAGE;
        p.flags = sys::NVOS32_ALLOC_FLAGS_FIXED_ADDRESS_ALLOCATE;
        p.attr = nvos32_attr::LOCATION.set(sys::NVOS32_ATTR_LOCATION_VIDMEM)
            | nvos32_attr::PHYSICALITY.set(sys::NVOS32_ATTR_PHYSICALITY_CONTIGUOUS)
            | nvos32_attr::PAGE_SIZE.set(sys::NVOS32_ATTR_PAGE_SIZE_HUGE);
        p.attr2 = v::ATTR2_PAGE_SIZE_HUGE_2MB;
        p.size = s;
        p.offset = base;
        let h = self.rm.next_handle();
        let t = Instant::now();
        // SAFETY: NV_MEMORY_ALLOCATION_PARAMS matches NV01_MEMORY_LOCAL_USER; no pointers.
        let r = unsafe {
            self.rm
                .alloc(self.device, h, v::NV01_MEMORY_LOCAL_USER, Some(&mut p))
        };
        let alloc_ms = t.elapsed().as_millis();
        if let Err(e) = r {
            bail!(
                "refused: the host RM did not give [{base:#x}, {e_:#x}) ({err}); the host holds part of the range or the heap is fragmented ({largest}); no floating fallback (the guest's FB layout is fixed at start)",
                e_ = base + s,
                err = e
            );
        }
        let contiguous =
            nvos32_attr::PHYSICALITY.get(p.attr) == sys::NVOS32_ATTR_PHYSICALITY_CONTIGUOUS;
        if p.offset != base || !contiguous {
            let got = p.offset;
            let _ = self.rm.free(h);
            bail!(
                "refused: the reservation came back at {got:#x} (asked {base:#x}), attr {:#x} (contiguous {contiguous}); freed",
                p.attr
            );
        }
        log.push(format!(
            "reserved host VRAM [{base:#x}, {:#x}) ({} MiB, slot {slot}), handle {h:#x}, attr {:#x} attr2 {:#x}, contiguous, in {alloc_ms} ms",
            base + s,
            s / MIB,
            p.attr,
            p.attr2
        ));
        self.reservation = Some(Reservation {
            handle: h,
            base,
            size: s,
            region_end,
            host_fb_total: total,
            heap_free,
            smi_free,
            alloc_ms,
        });
        if let Some(l) = self.scan_reservation("after the reservation") {
            log.push(l);
        }
        Ok(log)
    }

    /// One OS descriptor over `len` bytes at `host_va` (the device's own mapping of the
    /// guest RAM fd): pins the pages and maps them into the GPU's IOMMU domain
    /// (sysmem-iova.md §2). The flags are Leandro v1's (host_pool.rs `OSDESC_FLAGS`).
    fn os_descriptor(&mut self, host_va: *mut u8, len: u64) -> Result<u32> {
        let h = self.rm.next_handle();
        let mut w = Nvos02WithFd::default();
        w.params.hRoot = self.rm.root();
        w.params.hObjectParent = self.device;
        w.params.hObjectNew = h;
        w.params.hClass = sys::NV01_MEMORY_SYSTEM_OS_DESCRIPTOR;
        w.params.flags = nvos02_flags::PHYSICALITY.set(1)
            | nvos02_flags::LOCATION.set(0)
            | nvos02_flags::COHERENCY.set(1)
            | nvos02_flags::MAPPING.set(nvos02_flags::MAPPING_NO_MAP);
        w.params.pMemory = host_va as usize as sys::NvP64;
        w.params.limit = len - 1;
        w.fd = -1;
        // SAFETY: NVOS02-with-fd is this escape's ABI; `host_va..+len` is the device's
        // live shared mapping of guest RAM, which stays mapped until the descriptor is
        // freed (release_region runs before the munmap).
        unsafe { self.gpu_reg.ioctl_raw(sys::NV_ESC_RM_ALLOC_MEMORY, &mut w) }
            .map_err(|e| anyhow::anyhow!("RM_ALLOC_MEMORY: {e}"))?;
        nvrm_abi::check_status(sys::NV_ESC_RM_ALLOC_MEMORY, w.params.status)
            .map_err(|e| anyhow::anyhow!("OS descriptor of {len:#x} bytes: {e}"))?;
        // RM_ALLOC_MEMORY bypasses RmClient::alloc; its tree does not know the handle,
        // so free() falls back to the root as parent; the device is the real parent.
        Ok(h)
    }

    /// A **gather descriptor** (docs/caraxes/memory-model.md §7): one
    /// `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR` whose offsets `0, 4 KiB, 8 KiB, ...` are the guest
    /// pages `pages` in the given order, so that one `RM_MAP_MEMORY_DMA` maps a VA stretch
    /// whose guest pages are scattered. Every page must lie wholly inside one registered
    /// guest-RAM descriptor, named by its handle and page-aligned offset (the shadow filter's
    /// answer, shadow.rs `resolve_leaf`); anything else is refused before any system call.
    /// The pages are placed back to back in a fresh VA window of the device with
    /// `mremap(old_size 0)` of the device's own shared mapping of guest RAM (a second mapping
    /// of the same pages, mremap(2); consecutive pages in one call), the descriptor is made
    /// over the window (the RM pins the pages and maps them for the GPU, as for a registered
    /// chunk), and the window is unmapped again: the RM holds the pages by their page array
    /// from then on (`os_lock_user_pages`), not by the VA, so no mapping accumulates. The
    /// pages are this VM's registered guest RAM, already pinned; the GPU reaches the same
    /// pages through the descriptor's own IOVAs. Freed with [`Self::free_gather`], after
    /// every mapping of it is gone.
    pub fn gather(&mut self, pages: &[(u32, u64)]) -> Result<u32, String> {
        const PAGE: u64 = 4096;
        if pages.is_empty() {
            return Err("no page".into());
        }
        let mut host: Vec<usize> = Vec::with_capacity(pages.len());
        for &(h, off) in pages {
            let d = self
                .descriptors
                .iter()
                .find(|d| d.handle == h)
                .ok_or_else(|| format!("handle {h:#x} is not a registered guest-RAM descriptor"))?;
            if off % PAGE != 0 || off.checked_add(PAGE).is_none_or(|e| e > d.range.size) || d.host_va == 0 {
                return Err(format!("offset {off:#x} is not a page of descriptor {h:#x}"));
            }
            host.push(d.host_va + off as usize);
        }
        let len = pages.len() * PAGE as usize;
        // SAFETY: an anonymous PROT_NONE reservation; mremap places pages inside it below.
        let win = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_NONE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
                -1,
                0,
            )
        };
        if win == libc::MAP_FAILED {
            return Err(format!("window mmap: {}", std::io::Error::last_os_error()));
        }
        let unmap = |w: *mut libc::c_void| {
            // SAFETY: the window mapped above, `len` bytes.
            unsafe { libc::munmap(w, len) };
        };
        let mut i = 0usize;
        while i < host.len() {
            let mut j = i + 1;
            while j < host.len() && host[j] == host[j - 1] + PAGE as usize {
                j += 1;
            }
            let dst = win.cast::<u8>().wrapping_add(i * PAGE as usize);
            // SAFETY: old_size 0 duplicates the device's MAP_SHARED mapping of guest RAM
            // (`host[i]..` lies in a registered descriptor's mapping, checked above) at
            // `dst`, inside the window this function owns.
            let got = unsafe {
                libc::mremap(
                    host[i] as *mut libc::c_void,
                    0,
                    (j - i) * PAGE as usize,
                    libc::MREMAP_MAYMOVE | libc::MREMAP_FIXED,
                    dst.cast::<libc::c_void>(),
                )
            };
            if got != dst.cast::<libc::c_void>() {
                let e = std::io::Error::last_os_error();
                unmap(win);
                return Err(format!("mremap of page {i}: {e}"));
            }
            i = j;
        }
        let r = self.os_descriptor(win.cast(), len as u64);
        unmap(win);
        let h = r.map_err(|e| format!("{e:#}"))?;
        self.gathers.push(h);
        Ok(h)
    }

    /// Free a gather descriptor (unpins its page references). Returns the status.
    pub fn free_gather(&mut self, h: u32) -> Status {
        let Some(i) = self.gathers.iter().position(|g| *g == h) else {
            return 0xFFFF_FFFE;
        };
        self.gathers.swap_remove(i);
        match self.free_descriptor(h) {
            Ok(()) => sys::NV_OK,
            Err(e) => {
                self.free_failures.push(format!("host gather descriptor {h:#x}: {e:#}"));
                0xFFFF_FFFF
            }
        }
    }

    fn free_descriptor(&mut self, h: u32) -> Result<()> {
        let mut p = sys::NVOS00_PARAMETERS::default();
        p.hRoot = self.rm.root();
        p.hObjectParent = self.device;
        p.hObjectOld = h;
        // SAFETY: NVOS00 carries only handles and status.
        unsafe { self.rm.ctl().ioctl_raw(sys::NV_ESC_RM_FREE, &mut p) }
            .map_err(|e| anyhow::anyhow!("RM_FREE: {e}"))?;
        nvrm_abi::check_status(sys::NV_ESC_RM_FREE, p.status)
            .map_err(|e| anyhow::anyhow!("RM_FREE of descriptor {h:#x}: {e}"))
    }

    /// The IOVA of `offsets` of descriptor `h`, in one privileged window.
    fn phys_attr(&self, h: u32, offsets: &[u64]) -> Result<Vec<(u32, u64, u32)>> {
        let root = self.rm.root();
        let ctl = self.rm.ctl();
        self.privilege.with_admin(|| {
            offsets
                .iter()
                .map(|&off| {
                    let mut a = PhysAttr([0; v::PHYS_ATTR_SIZE]);
                    a.0[v::PHYS_ATTR_MEM_OFFSET..][..8].copy_from_slice(&off.to_le_bytes());
                    let mut p = sys::NVOS54_PARAMETERS::default();
                    p.hClient = root;
                    p.hObject = h;
                    p.cmd = v::CMD_GET_SURFACE_PHYS_ATTR;
                    p.params = a.0.as_mut_ptr() as usize as sys::NvP64;
                    p.paramsSize = v::PHYS_ATTR_SIZE as u32;
                    // SAFETY: NVOS54 escape with the 48-byte parameter block above.
                    let ok = unsafe { ctl.ioctl_raw(sys::NV_ESC_RM_CONTROL, &mut p) }.is_ok();
                    let rd64 = |o: usize| u64::from_le_bytes(a.0[o..o + 8].try_into().unwrap());
                    let rd32 = |o: usize| u32::from_le_bytes(a.0[o..o + 4].try_into().unwrap());
                    (
                        if ok { p.status } else { 0xFFFF_FFFF },
                        rd64(v::PHYS_ATTR_MEM_OFFSET),
                        rd32(v::PHYS_ATTR_MEM_APERTURE),
                    )
                })
                .collect()
        })
    }

    /// Register guest RAM `[gpa, gpa + size)`, mapped by the device at `host_va`, in
    /// 1 GiB chunks aligned in guest physical space. Each chunk must come back as one
    /// IOVA range (checked at offset 0, every 128 MiB and the last page); anything else
    /// is refused and the chunks of this call are freed again. Refused as well when the
    /// pinning would leave the host less than 4 GiB of available RAM.
    pub fn register(
        &mut self,
        gpa: u64,
        size: u64,
        host_va: *mut u8,
    ) -> Result<(Vec<DmaRange>, Vec<String>)> {
        let total = std::env::var("CARAXES_GUEST_RAM_BYTES")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|_| std::env::var("CARAXES_LINEAR_IOVA").as_deref() != Ok("0"));
        let Some(total) = total else {
            return self.register_chunks(gpa, size, host_va);
        };
        self.pending.push((gpa, size, host_va as usize));
        let have: u64 = self.pending.iter().map(|z| z.1).sum();
        if have < total {
            return Ok((
                Vec::new(),
                vec![format!(
                    "guest RAM {gpa:#x}+{size:#x}: registration deferred until all {} MiB are mapped ({} MiB so far), so the whole guest gets one IOVA offset",
                    total / MIB,
                    have / MIB
                )],
            ));
        }
        let zones = std::mem::take(&mut self.pending);
        match self.register_linear(&zones) {
            Ok(r) => Ok(r),
            Err(e) => {
                // Fall back to the per-zone chunks (descending IOVAs): the guest still
                // runs, and a buffer crossing a chunk stays refused by the filter.
                let mut log = vec![format!(
                    "ERROR hostmem: linear registration failed ({e:#}); falling back to independent 1 GiB chunks (a guest buffer crossing a chunk boundary keeps no valid IOVA and is refused)"
                )];
                let mut ranges = Vec::new();
                for (g, sz, va) in zones {
                    let (r, l) = self.register_chunks(g, sz, va as *mut u8)?;
                    ranges.extend(r);
                    log.extend(l);
                }
                Ok((ranges, log))
            }
        }
    }

    /// Register every guest RAM zone so that IOVA = GPA + one offset for the whole guest
    /// (docs/caraxes/mapping.md §5, parallel.md §8.2). A descriptor gets one IOVA range,
    /// size-aligned and allocated top-down (sysmem-iova.md §1.4, §2), so the 1 GiB chunks
    /// are registered from the highest GPA down, and the holes between zones get filler
    /// descriptors of the same size: each chunk then lands right below the previous one.
    /// The result is checked (every chunk's IOVA minus its GPA must be the same); on a
    /// mismatch everything is freed and an error returned.
    fn register_linear(
        &mut self,
        zones: &[(u64, u64, usize)],
    ) -> Result<(Vec<DmaRange>, Vec<String>)> {
        let mut log = Vec::new();
        let total: u64 = zones.iter().map(|z| z.1).sum();
        let avail = meminfo_kib("MemAvailable:") * KIB;
        if avail < total + RAM_FLOOR {
            bail!(
                "refused: pinning {} MiB of guest RAM would leave the host {} MiB available, below the {} MiB floor",
                total / MIB,
                avail.saturating_sub(total) / MIB,
                RAM_FLOOR / MIB
            );
        }
        let mut zs = zones.to_vec();
        zs.sort_by_key(|z| z.0);
        for z in &zs {
            if z.0 % HUGE != 0 || z.1 % HUGE != 0 {
                bail!("guest RAM region {:#x}+{:#x} is not 2 MiB aligned", z.0, z.1);
            }
        }
        // Pieces: (gpa, len, Some(host va) for guest RAM, None for a hole filler).
        let mut pieces: Vec<(u64, u64, Option<usize>)> = Vec::new();
        let split = |a: u64, b: u64, va: Option<usize>, out: &mut Vec<(u64, u64, Option<usize>)>| {
            let mut at = a;
            while at < b {
                let end = ((at / CHUNK + 1) * CHUNK).min(b);
                out.push((at, end - at, va.map(|v| v + (at - a) as usize)));
                at = end;
            }
        };
        for (i, z) in zs.iter().enumerate() {
            split(z.0, z.0 + z.1, Some(z.2), &mut pieces);
            if let Some(n) = zs.get(i + 1) {
                if n.0 > z.0 + z.1 {
                    split(z.0 + z.1, n.0, None, &mut pieces);
                }
            }
        }
        pieces.sort_by_key(|p| std::cmp::Reverse(p.0));
        let pins0 = foll_pins();
        let t = Instant::now();
        let mut descs: Vec<Descriptor> = Vec::new();
        let mut fillers: Vec<Filler> = Vec::new();
        // Pieces placed out of line (another allocation of the host sat in between) stay
        // registered until the end, so the restarted sequence lands below them; then they
        // are freed.
        let mut stale: Vec<u32> = Vec::new();
        let mut stale_fillers: Vec<Filler> = Vec::new();
        let mut restarts = 0usize;
        let result: Result<()> = (|| {
            let mut i = 0usize;
            while i < pieces.len() {
                let (gpa, len, va) = pieces[i];
                let tc = Instant::now();
                let (h, filler) = match va {
                    Some(va) => (self.os_descriptor(va as *mut u8, len)?, None),
                    None => {
                        let f = Filler::new(gpa, len)?;
                        (self.os_descriptor(f.va.cast(), len)?, Some(f))
                    }
                };
                let reg_ms = tc.elapsed().as_millis();
                let mut offsets: Vec<u64> = (0..len).step_by(SAMPLE_STEP as usize).collect();
                offsets.push(len - 4096);
                let answers = match self.phys_attr(h, &offsets) {
                    Ok(a) => a,
                    Err(e) => {
                        let _ = self.free_descriptor(h);
                        return Err(e);
                    }
                };
                let base = answers[0].1;
                if let Some((o, (st, iova, ap))) = offsets
                    .iter()
                    .zip(&answers)
                    .find(|(&o, &(st, iova, ap))| st != 0 || ap != 1 || iova != base + o)
                {
                    let _ = self.free_descriptor(h);
                    bail!("refused: GPA {gpa:#x}+{len:#x} is not one IOVA range: offset {o:#x} gave status {st:#x} aperture {ap} IOVA {iova:#x} (base {base:#x})");
                }
                let expect = descs
                    .first()
                    .map(|d| d.range.iova.wrapping_sub(d.range.gpa))
                    .or_else(|| fillers.first().map(|f| f.iova.wrapping_sub(f.gpa)));
                if let Some(off) = expect.filter(|off| base.wrapping_sub(gpa) != *off) {
                    restarts += 1;
                    log.push(format!(
                        "linear registration: GPA {gpa:#x} landed at IOVA {base:#x} (offset {:#x}, not {off:#x}): a host allocation sits in between; restart {restarts} below it",
                        base.wrapping_sub(gpa)
                    ));
                    stale.push(h);
                    stale.extend(descs.drain(..).map(|d| d.handle));
                    for mut f in fillers.drain(..) {
                        stale.push(f.handle);
                        f.handle = 0;
                        stale_fillers.push(f);
                    }
                    if let Some(f) = filler {
                        stale_fillers.push(f);
                    }
                    if restarts > 4 {
                        bail!("no run of {} free adjacent IOVA slots after {restarts} restarts", pieces.len());
                    }
                    i = 0;
                    continue;
                }
                match filler {
                    Some(mut f) => {
                        f.handle = h;
                        f.iova = base;
                        log.push(format!(
                            "hole filler GPA {gpa:#x}..{:#x} -> IOVA {base:#x}..{:#x} (descriptor {h:#x} over one device-owned 2 MiB page, {reg_ms} ms; never in the guest's table or the filter)",
                            gpa + len,
                            base + len
                        ));
                        fillers.push(f);
                    }
                    None => {
                        let range = DmaRange { gpa, iova: base, size: len };
                        log.push(format!(
                            "placed guest RAM chunk {range} (descriptor {h:#x}, {reg_ms} ms, {} IOVA samples = base + offset)",
                            offsets.len()
                        ));
                        descs.push(Descriptor { handle: h, range, host_va: va.unwrap_or_default() });
                    }
                }
                i += 1;
            }
            Ok(())
        })();
        for h in stale.drain(..) {
            let _ = self.free_descriptor(h);
        }
        drop(stale_fillers);
        let mut offs: Vec<u64> = descs.iter().map(|d| d.range.iova.wrapping_sub(d.range.gpa)).collect();
        offs.extend(fillers.iter().map(|f| f.iova.wrapping_sub(f.gpa)));
        offs.sort_unstable();
        offs.dedup();
        let linear = offs.len() == 1;
        if result.is_err() || !linear {
            for d in descs {
                let _ = self.free_descriptor(d.handle);
            }
            for f in fillers {
                let _ = self.free_descriptor(f.handle);
            }
            result.map_err(|e| anyhow::anyhow!("{e:#}; placements: {}", log.join(" | ")))?;
            bail!("the chunks did not land adjacent: {} distinct IOVA - GPA offsets {:x?}; placements: {}", offs.len(), offs, log.join(" | "));
        }
        let pins1 = foll_pins();
        log.push(format!(
            "guest RAM {} MiB in {} zone(s): {} chunk(s) + {} hole filler(s) registered from the top GPA down in {} ms ({restarts} restart(s)); IOVA = GPA + {:#x} for the whole guest; pinned now {} MiB; host foll_pin acquired +{} released +{}; privileged windows so far {}",
            total / MIB,
            zs.len(),
            descs.len(),
            fillers.len(),
            t.elapsed().as_millis(),
            offs[0],
            (self.pinned_bytes() + total) / MIB,
            pins1.0 - pins0.0,
            pins1.1 - pins0.1,
            self.privilege.windows()
        ));
        descs.sort_by_key(|d| d.range.gpa);
        for d in &descs {
            log.push(format!(
                "registered guest RAM chunk {} (descriptor {:#x}, linear)",
                d.range, d.handle
            ));
        }
        // The guest's table: one entry per contiguous stretch of guest RAM.
        let mut ranges: Vec<DmaRange> = Vec::new();
        for d in &descs {
            match ranges.last_mut() {
                Some(l) if l.gpa + l.size == d.range.gpa && l.iova + l.size == d.range.iova => {
                    l.size += d.range.size
                }
                _ => ranges.push(d.range),
            }
        }
        self.descriptors.extend(descs);
        self.fillers.extend(fillers);
        Ok((ranges, log))
    }

    fn register_chunks(
        &mut self,
        gpa: u64,
        size: u64,
        host_va: *mut u8,
    ) -> Result<(Vec<DmaRange>, Vec<String>)> {
        let mut log = Vec::new();
        if size % HUGE != 0 || gpa % HUGE != 0 {
            bail!("guest RAM region {gpa:#x}+{size:#x} is not 2 MiB aligned (the guest module needs 2 MiB multiples)");
        }
        let avail = meminfo_kib("MemAvailable:") * KIB;
        if avail < size + RAM_FLOOR {
            bail!(
                "refused: pinning {} MiB of guest RAM would leave the host {} MiB available, below the {} MiB floor (MemAvailable {} MiB)",
                size / MIB,
                avail.saturating_sub(size) / MIB,
                RAM_FLOOR / MIB,
                avail / MIB
            );
        }
        let pins0 = foll_pins();
        let t = Instant::now();
        let mut out: Vec<Descriptor> = Vec::new();
        let mut at = gpa;
        let result: Result<()> = (|| {
            while at < gpa + size {
                let end = ((at / CHUNK + 1) * CHUNK).min(gpa + size);
                let len = end - at;
                let va = host_va.wrapping_add((at - gpa) as usize);
                let tc = Instant::now();
                let h = self.os_descriptor(va, len)?;
                let reg_ms = tc.elapsed().as_millis();
                let mut offsets: Vec<u64> = (0..len).step_by(SAMPLE_STEP as usize).collect();
                offsets.push(len - 4096);
                let answers = match self.phys_attr(h, &offsets) {
                    Ok(a) => a,
                    Err(e) => {
                        let _ = self.free_descriptor(h);
                        return Err(e);
                    }
                };
                let base = answers[0].1;
                let bad = offsets
                    .iter()
                    .zip(&answers)
                    .find(|(&o, &(st, iova, ap))| st != 0 || ap != 1 || iova != base + o);
                if let Some((o, (st, iova, ap))) = bad {
                    let _ = self.free_descriptor(h);
                    bail!(
                        "refused: chunk GPA {at:#x}+{len:#x} is not one IOVA range: offset {o:#x} gave status {st:#x} aperture {ap} IOVA {iova:#x} (base {base:#x})"
                    );
                }
                let range = DmaRange {
                    gpa: at,
                    iova: base,
                    size: len,
                };
                log.push(format!(
                    "registered guest RAM chunk {range} (descriptor {h:#x}, {reg_ms} ms, {} IOVA samples = base + offset)",
                    offsets.len()
                ));
                out.push(Descriptor { handle: h, range, host_va: va as usize });
                at = end;
            }
            Ok(())
        })();
        if let Err(e) = result {
            for d in out {
                let _ = self.free_descriptor(d.handle);
            }
            return Err(e);
        }
        let pins1 = foll_pins();
        log.push(format!(
            "guest RAM {gpa:#x}+{size:#x}: {} chunk(s) in {} ms; pinned now {} MiB; host foll_pin acquired +{} released +{}; privileged windows so far {}",
            out.len(),
            t.elapsed().as_millis(),
            (self.pinned_bytes() + size) / MIB,
            pins1.0 - pins0.0,
            pins1.1 - pins0.1,
            self.privilege.windows()
        ));
        let ranges = out.iter().map(|d| d.range).collect();
        self.descriptors.extend(out);
        Ok((ranges, log))
    }

    /// Free the descriptors inside guest RAM `[gpa, gpa + size)` (before the device
    /// unmaps it). The GPU must be idle on the guest's channels first: in part 1 no
    /// host channel exists (docs/caraxes/memory-model.md §4).
    pub fn release_region(&mut self, gpa: u64, size: u64) -> Vec<String> {
        // Never unpin while a host object that may reach the IOVAs lives (fail closed):
        // the descriptors stay registered (the pages stay pinned, which is safe after the
        // device's munmap) and the teardown frees them after the mirror objects.
        if !self.live.is_empty() || !self.free_failures.is_empty() {
            return vec![format!(
                "guest RAM {gpa:#x}+{size:#x} unmapped by the client while {} host mirror object(s) live and {} free(s) failed: descriptors kept until the teardown (fail closed)",
                self.live.len(),
                self.free_failures.len()
            )];
        }
        let (gone, keep): (Vec<Descriptor>, Vec<Descriptor>) = self
            .descriptors
            .iter()
            .partition(|d| d.range.gpa >= gpa && d.range.gpa + d.range.size <= gpa + size);
        self.descriptors = keep;
        gone.into_iter()
            .map(|d| match self.free_descriptor(d.handle) {
                Ok(()) => format!("freed guest RAM descriptor {:#x} ({})", d.handle, d.range),
                Err(e) => format!("ERROR freeing descriptor {:#x}: {e}", d.handle),
            })
            .collect()
    }

    /// Free everything by [`run_teardown`]: the mirror objects, then the descriptors
    /// (unpinning guest RAM), then the reservation, then the client, failing closed at the
    /// first failure (docs/caraxes/parallel.md, P20 item 4a). Idempotent. After a fail
    /// closed teardown [`HostMemory::fail_closed`] names the reason and the caller exits
    /// non-zero. `CARAXES_INJECT_TEARDOWN_FAILURE=1` counts one failed host free (a test of
    /// the rule on a live device; nothing is freed wrongly by it).
    pub fn shutdown(&mut self) -> Vec<String> {
        if self.shut {
            return Vec::new();
        }
        self.shut = true;
        let mut log = Vec::new();
        if std::env::var("CARAXES_INJECT_TEARDOWN_FAILURE").as_deref() == Ok("1") {
            self.free_failures
                .push("injected by CARAXES_INJECT_TEARDOWN_FAILURE=1".into());
        }
        let prior = self.free_failures.len();
        for f in &self.free_failures {
            log.push(format!("earlier failed host free: {f}"));
        }
        let n = self.descriptors.len();
        let bytes = self.pinned_bytes();
        let p0 = foll_pins();
        let outcome = run_teardown(self, prior, &mut log);
        let p1 = foll_pins();
        let freed = n - self.descriptors.len();
        if freed > 0 {
            log.push(format!(
                "freed {freed} guest RAM descriptor(s), {} MiB unpinned; host foll_pin released +{} during the free (since open: acquired +{}, released +{})",
                (bytes - self.pinned_bytes()) / MIB,
                p1.1 - p0.1,
                p1.0 - self.pins_at_open.0,
                p1.1 - self.pins_at_open.1
            ));
        }
        if let TeardownOutcome::FailClosed {
            reason,
            descriptors_kept,
            reservation_kept,
        } = &outcome
        {
            let res = self
                .reservation
                .as_ref()
                .map(|r| format!("[{:#x}, {:#x})", r.base, r.end()))
                .unwrap_or_else(|| "none".into());
            log.push(format!(
                "FAIL-CLOSED teardown: {reason}; kept {descriptors_kept} guest RAM descriptor(s) ({} MiB pinned) and the reservation {res} (kept: {reservation_kept}); the client is not freed explicitly: the process exits non-zero and the close of /dev/nvidiactl lets the host RM free everything in its own order",
                self.pinned_bytes() / MIB
            ));
            self.fail_closed = Some(reason.clone());
        }
        log.push(format!(
            "privileged windows opened in total: {}",
            self.privilege.windows()
        ));
        log
    }
}

impl TeardownOps for HostMemory {
    fn free_mirrors(&mut self, log: &mut Vec<String>) -> usize {
        // Mirror channels first (RM preempts and idles each channel in its free), so that
        // no host channel can still reach the IOVAs the descriptors unpin next.
        if self.live.is_empty() && self.gathers.is_empty() {
            return 0;
        }
        let n = self.live.len();
        let before = self.free_failures.len();
        log.extend(self.free_live());
        // Gather descriptors the shadow did not free (it frees them with their mappings):
        // after the channels and VA spaces, before the guest-RAM descriptors.
        let gathers = self.gathers.clone();
        for g in &gathers {
            self.free_gather(*g);
        }
        if !gathers.is_empty() {
            log.push(format!("freed {} leftover gather descriptor(s) of the shadow", gathers.len()));
        }
        let failed = self.free_failures.len() - before;
        log.push(format!(
            "freed {} of {n} host mirror object(s) before the descriptors",
            n - failed
        ));
        failed
    }

    fn descriptor_count(&self) -> usize {
        self.descriptors.len()
    }

    fn free_last_descriptor(&mut self, log: &mut Vec<String>) -> bool {
        let Some(d) = self.descriptors.last().copied() else {
            return true;
        };
        match self.free_descriptor(d.handle) {
            Ok(()) => {
                self.descriptors.pop();
                true
            }
            Err(e) => {
                log.push(format!(
                    "ERROR freeing descriptor {:#x} ({}): {e}",
                    d.handle, d.range
                ));
                false
            }
        }
    }

    fn free_reservation(&mut self, log: &mut Vec<String>) -> bool {
        // The hole fillers hold only device-owned pages: freed with the guest RAM.
        for f in std::mem::take(&mut self.fillers) {
            match self.free_descriptor(f.handle) {
                Ok(()) => log.push(format!("freed hole filler descriptor {:#x} (GPA {:#x}, IOVA {:#x})", f.handle, f.gpa, f.iova)),
                Err(e) => log.push(format!("ERROR freeing hole filler {:#x}: {e}", f.handle)),
            }
        }
        let Some(r) = self.reservation.clone() else {
            return true;
        };
        // The host RM scrubs a freed vidmem allocation (scrub on free, PMA; parallel.md
        // §4b): the free returns once the scrub is submitted, the next allocation of the
        // range waits for it.
        if let Some(l) = self.scan_reservation("before the free") {
            log.push(l);
        }
        let t = Instant::now();
        match self.rm.free(r.handle) {
            Ok(()) => {
                log.push(format!(
                    "freed the VRAM reservation [{:#x}, {:#x}) ({} MiB) in {} ms (host RM scrub on free)",
                    r.base,
                    r.end(),
                    r.size / MIB,
                    t.elapsed().as_millis()
                ));
                self.reservation = None;
                true
            }
            Err(e) => {
                log.push(format!("ERROR freeing the VRAM reservation: {e}"));
                false
            }
        }
    }

    fn free_client(&mut self, log: &mut Vec<String>) -> bool {
        let root = self.rm.root();
        match self.rm.free(root) {
            Ok(()) => {
                log.push(format!("host RM client {root:#x} freed"));
                true
            }
            Err(e) => {
                log.push(format!(
                    "host RM client {root:#x}: {e} (the fd close frees it)"
                ));
                false
            }
        }
    }
}

impl Drop for HostMemory {
    fn drop(&mut self) {
        for l in self.shutdown() {
            eprintln!("vfio-user-nvgpu: hostmem: {l}");
        }
    }
}

#[cfg(test)]
mod slot_tests {
    use super::*;

    /// P20 placement: slot n is n profile sizes below slot 0, 2 MiB aligned, and a slot
    /// that leaves the heap region is refused. Numbers from the RTX 2070 host
    /// (bench v2-hang gpu log: heap region [0x3600000, 0x1e918ffff]).
    #[test]
    fn slots_stack_downward() {
        let (heap, end) = (0x360_0000u64, 0x1_e919_0000u64);
        // Slot 0 is today's range (the 2026-09-24 log: [0xe9000000, 0x1e9000000)).
        assert_eq!(
            slot_range(heap, end, 4 * GIB, 0),
            Ok((0xe900_0000, 0x1_e900_0000))
        );
        assert_eq!(
            slot_range(heap, end, 2 * GIB, 0),
            Ok((0x1_6900_0000, 0x1_e900_0000))
        );
        assert_eq!(
            slot_range(heap, end, 2 * GIB, 1),
            Ok((0xe900_0000, 0x1_6900_0000))
        );
        assert_eq!(
            slot_range(heap, end, 2 * GIB, 2),
            Ok((0x6900_0000, 0xe900_0000))
        );
        // 2.5 GiB (odd sizes are tested, D11): slot 1 ends where slot 0 begins.
        let s = 2560 * MIB;
        let (b0, _) = slot_range(heap, end, s, 0).unwrap();
        let (b1, e1) = slot_range(heap, end, s, 1).unwrap();
        assert_eq!((e1, b0 - b1), (b0, s));
        assert_eq!(b1 % HUGE, 0);
        // Beyond the heap base: refused, never clamped.
        assert!(slot_range(heap, end, 4 * GIB, 1).is_err());
        assert!(slot_range(heap, end, 2 * GIB, 3).is_err());
        assert!(slot_range(heap, end, 2 * GIB, u32::MAX).is_err());
    }
}

#[cfg(test)]
mod teardown_tests {
    use super::*;

    /// A scripted teardown: which step fails, and what was freed.
    #[derive(Default)]
    struct Fake {
        mirror_failures: usize,
        descriptors: usize,
        fail_descriptor_at: Option<usize>,
        fail_reservation: bool,
        reservation: bool,
        client: bool,
        order: Vec<&'static str>,
    }

    impl TeardownOps for Fake {
        fn free_mirrors(&mut self, _: &mut Vec<String>) -> usize {
            self.order.push("mirrors");
            self.mirror_failures
        }
        fn descriptor_count(&self) -> usize {
            self.descriptors
        }
        fn free_last_descriptor(&mut self, _: &mut Vec<String>) -> bool {
            if self.fail_descriptor_at == Some(self.descriptors) {
                return false;
            }
            self.order.push("descriptor");
            self.descriptors -= 1;
            true
        }
        fn free_reservation(&mut self, _: &mut Vec<String>) -> bool {
            if self.fail_reservation {
                return false;
            }
            self.order.push("reservation");
            self.reservation = false;
            true
        }
        fn free_client(&mut self, _: &mut Vec<String>) -> bool {
            self.order.push("client");
            self.client = false;
            true
        }
    }

    fn fake(descriptors: usize) -> Fake {
        Fake {
            descriptors,
            reservation: true,
            client: true,
            ..Fake::default()
        }
    }

    #[test]
    fn a_freed_object_takes_its_whole_subtree_off_the_live_list() {
        // device 1 > TSG 2 > channel 3 > engine 4; device 1 > VAS 5; channel 6 elsewhere.
        let mut live = vec![(1, 5, "vaspace"), (1, 2, "tsg"), (2, 3, "channel"), (3, 4, "engine-object"), (1, 6, "channel")];
        forget_subtree(&mut live, 2);
        // The object itself is removed by the caller (`free_object`).
        assert_eq!(live, vec![(1, 5, "vaspace"), (1, 2, "tsg"), (1, 6, "channel")]);
    }

    #[test]
    fn a_clean_teardown_frees_in_order() {
        let mut f = fake(4);
        let out = run_teardown(&mut f, 0, &mut Vec::new());
        assert_eq!(out, TeardownOutcome::Clean);
        assert_eq!(
            f.order,
            [
                "mirrors",
                "descriptor",
                "descriptor",
                "descriptor",
                "descriptor",
                "reservation",
                "client"
            ]
        );
    }

    #[test]
    fn a_failed_mirror_free_keeps_every_registration_and_the_reservation() {
        let mut f = fake(4);
        f.mirror_failures = 1;
        let out = run_teardown(&mut f, 0, &mut Vec::new());
        assert!(matches!(
            out,
            TeardownOutcome::FailClosed {
                descriptors_kept: 4,
                reservation_kept: true,
                ..
            }
        ));
        assert_eq!(f.order, ["mirrors"]);
        assert!(f.reservation && f.client);
    }

    #[test]
    fn an_earlier_failed_free_also_fails_closed() {
        let mut f = fake(2);
        let out = run_teardown(&mut f, 1, &mut Vec::new());
        assert!(matches!(
            out,
            TeardownOutcome::FailClosed {
                descriptors_kept: 2,
                ..
            }
        ));
        assert_eq!(f.order, ["mirrors"]);
    }

    #[test]
    fn a_failed_unpin_stops_before_the_reservation() {
        let mut f = fake(4);
        f.fail_descriptor_at = Some(2);
        let out = run_teardown(&mut f, 0, &mut Vec::new());
        assert!(matches!(
            out,
            TeardownOutcome::FailClosed {
                descriptors_kept: 2,
                reservation_kept: true,
                ..
            }
        ));
        assert_eq!(f.order, ["mirrors", "descriptor", "descriptor"]);
        assert!(f.reservation && f.client);
    }

    #[test]
    fn a_failed_reservation_free_keeps_the_client() {
        let mut f = fake(1);
        f.fail_reservation = true;
        let out = run_teardown(&mut f, 0, &mut Vec::new());
        assert!(matches!(
            out,
            TeardownOutcome::FailClosed {
                descriptors_kept: 0,
                reservation_kept: true,
                ..
            }
        ));
        assert!(f.client);
    }
}

#[cfg(test)]
mod scan_tests {
    use super::*;

    #[test]
    fn pattern_records_are_counted_on_16_byte_boundaries() {
        let mut b = vec![0u8; 64];
        for (i, v) in [P20_MAGIC.0, P20_MAGIC.1, 0xA1, 7].iter().enumerate() {
            b[16 + 4 * i..20 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
        assert_eq!(count_pattern(&b), (1, 4));
        // The same words shifted by 4 bytes are not a record.
        let mut c = vec![0u8; 64];
        c[4..8].copy_from_slice(&P20_MAGIC.0.to_le_bytes());
        c[8..12].copy_from_slice(&P20_MAGIC.1.to_le_bytes());
        assert_eq!(count_pattern(&c), (0, 2));
        assert_eq!(count_pattern(&[0u8; 32]), (0, 0));
    }
}

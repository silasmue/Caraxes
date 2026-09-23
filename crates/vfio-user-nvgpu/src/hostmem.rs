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
        unsafe {
            for (i, b) in buf.iter_mut().enumerate() {
                *b = std::ptr::read_volatile(self.ptr.add(at + i));
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

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: our own mapping; the fd closes with `_fd` and releases the RM mapping.
        unsafe { libc::munmap(self.ptr.cast(), self.len) };
    }
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
/// Heap attribution tag of the reservation (as host-facts.rs).
const OWNER_TAG: u32 = u32::from_le_bytes(*b"cara");

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
    /// The mirror objects of E5b part 2 (`(parent, handle, what)`, allocation order):
    /// freed LIFO before the descriptors, so no host channel can still use an IOVA
    /// when its pages are unpinned (docs/caraxes/memory-model.md §4).
    live: Vec<(u32, u32, &'static str)>,
    /// `nr_foll_pin_acquired`/`_released` when the client opened (host-wide counters).
    pins_at_open: (u64, u64),
    shut: bool,
}

/// One RM status, for callers that log the number rather than fail.
pub type Status = u32;

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
        let mut rm = RmClient::new().map_err(|e| anyhow::anyhow!("host RM client: {e}"))?;
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
            live: Vec::new(),
            pins_at_open: foll_pins(),
            shut: false,
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
            return Err(p.status as u32);
        }
        self.live.push((parent, h, what));
        Ok(h)
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

    /// `RM_MAP_MEMORY_DMA` (NVOS46) of `h_mem` at `[offset, offset+length)` into `h_dma`
    /// (a virtual reservation, or a VA space) at the **fixed** VA `va`, with the given
    /// `page_size` flag (`nvos46_flags::PAGE_SIZE_*`). Snoop for sysmem, off for vidmem.
    /// Read/write, GPU-uncached. Returns the status.
    /// Allocate an `NV50_MEMORY_VIRTUAL` reservation of `size` **fixed at VA `va`** inside
    /// `vaspace`, with the page-size attribute for the guest page `page_bytes`
    /// (4K/64K/2M). A `DMA_OFFSET_FIXED` map only lands inside the VA range a virtual
    /// object reserved, so the shadow route reserves one per contiguous run at its VA.
    /// Returns the handle, or the status (a refusal here is a VA collision with the
    /// server-reserved range or the host RM's own context VAs).
    pub fn alloc_virtual_fixed(
        &mut self,
        vaspace: u32,
        va: u64,
        size: u64,
        page_bytes: u64,
    ) -> Result<u32, Status> {
        let (page_attr, attr2) = match page_bytes {
            0x1000 => (sys::NVOS32_ATTR_PAGE_SIZE_4KB, 0),
            0x1_0000 => (sys::NVOS32_ATTR_PAGE_SIZE_BIG, 0),
            _ => (sys::NVOS32_ATTR_PAGE_SIZE_HUGE, v::ATTR2_PAGE_SIZE_HUGE_2MB),
        };
        let mut p = sys::NV_MEMORY_ALLOCATION_PARAMS::default();
        p.owner = OWNER_TAG;
        p.type_ = sys::NVOS32_TYPE_IMAGE as u32;
        p.flags =
            sys::NVOS32_ALLOC_FLAGS_VIRTUAL as u32 | sys::NVOS32_ALLOC_FLAGS_FIXED_ADDRESS_ALLOCATE;
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
            | f::DMA_OFFSET_FIXED.set(f::DMA_OFFSET_FIXED_TRUE);
        p.dmaOffset = va;
        // SAFETY: NVOS46 is the escape ABI; plain-data params, no host pointers.
        let r = match unsafe {
            self.rm
                .ctl()
                .ioctl_raw(sys::NV_ESC_RM_MAP_MEMORY_DMA, &mut p)
        } {
            Ok(()) => p.status as u32,
            Err(_) => 0xFFFF_FFFF,
        };
        if r != sys::NV_OK {
            eprintln!(
                "vfio-user-nvgpu: shadow: MAP_MEMORY_DMA hDma={h_dma:#x} hMem={h_mem:#x} off={offset:#x} len={length:#x} va={va:#x} pgsz={page_size} flags={:#x} -> {r:#x}",
                p.flags
            );
        }
        r
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
        let (parent, _, _) = self.live.remove(i);
        // RM frees the subtree; forget the children this side too.
        self.live.retain(|(p, _, _)| *p != handle);
        self.rm_free(parent, handle)
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
                Err(st) => log.push(format!("ERROR freeing host {what} {h:#x}: status {st:#x}")),
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

    /// Map `len` bytes of the reservation at reservation offset `offset` through BAR1.
    pub fn map_reservation(&mut self, offset: u64, len: usize) -> Result<Window, Status> {
        let r = self.reservation.as_ref().ok_or(0xFFFF_FFFEu32)?;
        let handle = r.handle;
        let target = self
            ._gpu
            .open_for_mapping(self.rm.ctl())
            .map_err(|_| 0xFFFF_FFFFu32)?;
        let mut params = sys::NVOS33_PARAMETERS::default();
        params.hClient = self.rm.root();
        params.hDevice = self.device;
        params.hMemory = handle;
        params.offset = offset;
        params.length = len as u64;
        params.pLinearAddress = 0usize as sys::NvP64;
        params.flags = nvos33_flags::ACCESS.set(nvos33_flags::ACCESS_READ_WRITE);
        let mut p = Nvos33WithFd::new(params, target.as_raw_fd());
        // SAFETY: NVOS33-with-fd is this escape's ABI; the context lands on `target`.
        if unsafe { self.rm.ctl().ioctl_raw(sys::NV_ESC_RM_MAP_MEMORY, &mut p) }.is_err() {
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
        Ok(Window {
            _fd: target,
            ptr: ptr.cast(),
            offset,
            len,
        })
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
    /// Returns the log lines.
    pub fn reserve(&mut self, size: u64) -> Result<Vec<String>> {
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
        let e = region_end & !(HUGE - 1);
        let base = match e.checked_sub(s) {
            Some(b) if b >= heap_base => b,
            _ => bail!(
                "refused: the profile ({} MiB) is larger than the host's FB heap region [{heap_base:#x}, {region_end:#x})",
                s / MIB
            ),
        };
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
            "host VRAM: total {} MiB, HEAP_FREE {} MiB, nvidia-smi free {} MiB, {largest}; request {} MiB at [{base:#x}, {e:#x}) (heap region end {region_end:#x})",
            total / MIB,
            heap_free / MIB,
            smi_free / MIB,
            s / MIB
        ));
        if s + VRAM_HEADROOM > f {
            bail!(
                "refused: reserving {} MiB would leave the host {} MiB of VRAM, below the {} MiB headroom (free {} MiB = min(HEAP_FREE, nvidia-smi); shortfall {} MiB); choose a smaller profile or free host VRAM",
                s / MIB,
                f.saturating_sub(s) / MIB,
                VRAM_HEADROOM / MIB,
                f / MIB,
                (s + VRAM_HEADROOM - f).div_ceil(MIB)
            );
        }
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
            "reserved host VRAM [{base:#x}, {:#x}) ({} MiB), handle {h:#x}, attr {:#x} attr2 {:#x}, contiguous, in {alloc_ms} ms",
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
                out.push(Descriptor { handle: h, range });
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

    /// Free everything: the descriptors (unpinning guest RAM), then the reservation, then
    /// the client. Idempotent. Part 1 has no host channels, so nothing can still be
    /// using the IOVAs.
    pub fn shutdown(&mut self) -> Vec<String> {
        if self.shut {
            return Vec::new();
        }
        self.shut = true;
        let mut log = Vec::new();
        // Mirror channels first (RM preempts and idles each channel in its free), so
        // that no host channel can still reach the IOVAs the descriptors unpin next.
        if !self.live.is_empty() {
            let n = self.live.len();
            log.extend(self.free_live());
            log.push(format!(
                "freed {n} host mirror object(s) before the descriptors"
            ));
        }
        let n = self.descriptors.len();
        let bytes = self.pinned_bytes();
        let p0 = foll_pins();
        for d in std::mem::take(&mut self.descriptors) {
            if let Err(e) = self.free_descriptor(d.handle) {
                log.push(format!("ERROR freeing descriptor {:#x}: {e}", d.handle));
            }
        }
        let p1 = foll_pins();
        if n > 0 {
            log.push(format!(
                "freed {n} guest RAM descriptor(s), {} MiB unpinned; host foll_pin released +{} during the free (since open: acquired +{}, released +{})",
                bytes / MIB,
                p1.1 - p0.1,
                p1.0 - self.pins_at_open.0,
                p1.1 - self.pins_at_open.1
            ));
        }
        if let Some(r) = self.reservation.take() {
            match self.rm.free(r.handle) {
                Ok(()) => log.push(format!(
                    "freed the VRAM reservation [{:#x}, {:#x}) ({} MiB)",
                    r.base,
                    r.end(),
                    r.size / MIB
                )),
                Err(e) => log.push(format!("ERROR freeing the VRAM reservation: {e}")),
            }
        }
        let root = self.rm.root();
        match self.rm.free(root) {
            Ok(()) => log.push(format!("host RM client {root:#x} freed")),
            Err(e) => log.push(format!(
                "host RM client {root:#x}: {e} (the fd close frees it)"
            )),
        }
        log.push(format!(
            "privileged windows opened in total: {}",
            self.privilege.windows()
        ));
        log
    }
}

impl Drop for HostMemory {
    fn drop(&mut self) {
        for l in self.shutdown() {
            eprintln!("vfio-user-nvgpu: hostmem: {l}");
        }
    }
}

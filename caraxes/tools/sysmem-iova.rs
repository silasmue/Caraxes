// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Host IOVAs of guest-RAM-like OS descriptors (docs/caraxes/sysmem-iova.md).
//!
//! Registers a memfd-backed (or anonymous) buffer with the host RM as
//! NV01_MEMORY_SYSTEM_OS_DESCRIPTOR, the way Leandro v1 registers guest RAM
//! (crates/vhost-user-nvrm/src/host_pool.rs `Backing::os_descriptor`), reads the
//! IOVA of every 4 KiB page (`--full-scan`) or every 2 MiB with
//! NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR, frees the
//! descriptor and checks that the handle and the page pins are gone.
//!
//! Builds with rustc alone, like host-facts.rs:
//!
//!     rustc -O --edition 2021 caraxes/tools/sysmem-iova.rs -o target/sysmem-iova
//!     sudo -n target/sysmem-iova measure --go --sizes 256 --reps 3 --pair --full-scan
//!     sudo -n target/sysmem-iova measure --go --sizes 1024 --reps 3 --full-scan
//!
//! (the two runs of 2026-09-23). Without `--full-scan` the IOVA is read every 2 MiB.
//! Note: with NVreg_ResmanDebugLevel=0 the host driver logs every ioctl, so a full
//! scan of 1 GiB adds about 262k kernel log lines.
//!
//! Privilege. GET_SURFACE_PHYS_ATTR is PRIVILEGED (flags 0x4,
//! src/nvidia/generated/g_mem_nvoc.c:166). RM decides the privilege of each
//! ioctl from capable(CAP_SYS_ADMIN) at the time of the call
//! (src/nvidia/arch/nvalloc/unix/src/escape.c:381, os.c:648,
//! kernel-open/common/inc/nv-linux.h:490; checked at
//! src/nvidia/src/kernel/rmapi/control.c:683-696). The tool therefore starts as
//! root (sudo), immediately switches to the invoking user (SUDO_UID/SUDO_GID)
//! keeping only CAP_SYS_ADMIN in the permitted set with an empty effective set,
//! and raises CAP_SYS_ADMIN into the effective set only around the
//! GET_SURFACE_PHYS_ATTR ioctls. Every other RM call (client, device,
//! REGISTER_FD, RM_ALLOC_MEMORY, RM_FREE) and all memory handling run as the
//! unprivileged user, as Leandro v1 does (LEA_ADMIN_PRIV unset).
//!
//! Safety. Only RM escapes are issued; no register access, no mapping of the
//! card. Before each registration MemAvailable must stay >= 4 GiB after the
//! buffer is allocated, else the size is skipped. Sizes above 1 GiB are refused
//! unless `--allow-over-1g` is given: their power-of-two IOVA allocation cannot
//! fit below 4 GiB on this host, and the first such failure makes the kernel
//! switch the GPU to 64-bit IOVAs for the rest of the boot
//! (drivers/iommu/dma-iommu.c:791-799 in Linux 7.2). Each descriptor is freed
//! before the next step; `Drop` frees leftovers, and closing /dev/nvidiactl frees
//! the RM client (and unpins its pages) if the process dies.
//!
//! Layouts from vendor/open-gpu-kernel-modules (610.57.04):
//!   src/common/sdk/nvidia/inc/nvos.h:285-295 (NVOS02_PARAMETERS)
//!   src/nvidia/arch/nvalloc/unix/include/nv-unix-nvos-params-wrappers.h:34-37
//!   src/common/sdk/nvidia/inc/ctrl/ctrl0041.h:113-126 (GET_SURFACE_PHYS_ATTR)
//!   kernel-open/common/inc/nv-ioctl-numbers.h:34-36 (NV_ESC_REGISTER_FD 201)
//!   OS descriptor flags: host_pool.rs:170 (NONCONTIGUOUS, PCI, CACHED, NO_MAP)

use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::time::Instant;

extern "C" {
    fn ioctl(fd: i32, request: u64, ...) -> i32;
    fn syscall(n: i64, ...) -> i64;
    fn prctl(option: i32, ...) -> i32;
    fn setresuid(r: u32, e: u32, s: u32) -> i32;
    fn setresgid(r: u32, e: u32, s: u32) -> i32;
    fn setgroups(n: usize, list: *const u32) -> i32;
    fn geteuid() -> u32;
    fn getuid() -> u32;
    fn memfd_create(name: *const u8, flags: u32) -> i32;
    fn ftruncate(fd: i32, len: i64) -> i32;
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
    fn munmap(addr: *mut u8, len: usize) -> i32;
    fn close(fd: i32) -> i32;
}

const SYS_CAPGET: i64 = 125;
const SYS_CAPSET: i64 = 126;
const PR_SET_KEEPCAPS: i32 = 8;
const CAP_VERSION_3: u32 = 0x2008_0522;
const CAP_SYS_ADMIN_BIT: u32 = 1 << 21;

const PROT_RW: i32 = 0x1 | 0x2;
const MAP_SHARED: i32 = 0x01;
const MAP_PRIVATE: i32 = 0x02;
const MAP_ANONYMOUS: i32 = 0x20;
const MAP_POPULATE: i32 = 0x8000;
const MFD_CLOEXEC: u32 = 1;

const NV_ESC_RM_FREE: u32 = 0x29;
const NV_ESC_RM_CONTROL: u32 = 0x2A;
const NV_ESC_RM_ALLOC: u32 = 0x2B;
const NV_ESC_RM_ALLOC_MEMORY: u32 = 0x27;
const NV_ESC_REGISTER_FD: u32 = 201;

const NV01_ROOT_CLIENT: u32 = 0x41;
const NV01_MEMORY_SYSTEM_OS_DESCRIPTOR: u32 = 0x71;
const NV01_DEVICE_0: u32 = 0x80;
const CMD_GET_SURFACE_PHYS_ATTR: u32 = 0x0041_0103;
/// NONCONTIGUOUS | LOCATION_PCI | COHERENCY_CACHED | MAPPING_NO_MAP (host_pool.rs:170).
const OSDESC_FLAGS: u32 = (1 << 4) | (0 << 8) | (1 << 12) | (1 << 30);

const PAGE: u64 = 4096;
const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;
const RAM_FLOOR: u64 = 4 * GIB;

#[repr(C)]
#[derive(Default)]
struct NvOs00 {
    h_root: u32,
    h_object_parent: u32,
    h_object_old: u32,
    status: u32,
}

#[repr(C)]
#[derive(Default)]
struct NvOs64 {
    h_root: u32,
    h_object_parent: u32,
    h_object_new: u32,
    h_class: u32,
    p_alloc_parms: u64,
    p_rights_requested: u64,
    params_size: u32,
    flags: u32,
    status: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Default)]
struct NvOs54 {
    h_client: u32,
    h_object: u32,
    cmd: u32,
    flags: u32,
    params: u64,
    params_size: u32,
    status: u32,
}

/// nv_ioctl_nvos02_parameters_with_fd.
#[repr(C)]
#[derive(Default)]
struct NvOs02Fd {
    h_root: u32,
    h_object_parent: u32,
    h_object_new: u32,
    h_class: u32,
    flags: u32,
    _pad0: u32,
    p_memory: u64,
    limit: u64,
    status: u32,
    _pad1: u32,
    fd: i32,
    _pad2: u32,
}

#[repr(C)]
#[derive(Default)]
struct Nv0080Alloc {
    device_id: u32,
    h_client_share: u32,
    h_target_client: u32,
    h_target_device: u32,
    flags: u32,
    _pad: u32,
    va_space_size: u64,
    va_start_internal: u64,
    va_limit_internal: u64,
    va_mode: u32,
    _pad2: u32,
}

/// NV0041_CTRL_GET_SURFACE_PHYS_ATTR_PARAMS.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct PhysAttr {
    mem_offset: u64,
    mem_format: u32,
    compr_offset: u32,
    compr_format: u32,
    mem_aperture: u32,
    gpu_cache_attr: u32,
    gpu_p2p_cache_attr: u32,
    mmu_context: u32,
    _pad: u32,
    contig_segment_size: u64,
}

#[repr(C)]
struct CapHeader {
    version: u32,
    pid: i32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct CapData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

const _: () = {
    assert!(std::mem::size_of::<NvOs00>() == 16);
    assert!(std::mem::size_of::<NvOs64>() == 48);
    assert!(std::mem::size_of::<NvOs54>() == 32);
    assert!(std::mem::size_of::<NvOs02Fd>() == 56);
    assert!(std::mem::offset_of!(NvOs02Fd, p_memory) == 24);
    assert!(std::mem::offset_of!(NvOs02Fd, status) == 40);
    assert!(std::mem::offset_of!(NvOs02Fd, fd) == 48);
    assert!(std::mem::size_of::<Nv0080Alloc>() == 56);
    assert!(std::mem::size_of::<PhysAttr>() == 48);
    assert!(std::mem::offset_of!(PhysAttr, contig_segment_size) == 40);
};

fn iowr(nr: u32, size: usize) -> u64 {
    (3u64 << 30) | ((size as u64) << 16) | ((b'F' as u64) << 8) | nr as u64
}

fn esc<T>(fd: i32, nr: u32, arg: &mut T) {
    // SAFETY: T is one of the parameter structs above, sized as the escape expects.
    let r = unsafe { ioctl(fd, iowr(nr, std::mem::size_of::<T>()), arg as *mut T) };
    if r < 0 {
        panic!("ioctl nr {nr:#x}: {}", std::io::Error::last_os_error());
    }
}

// ---------------------------------------------------------------- capabilities

fn caps_set(effective: u32, permitted: u32) {
    let hdr = CapHeader { version: CAP_VERSION_3, pid: 0 };
    let data = [CapData { effective, permitted, inheritable: 0 }, CapData::default()];
    // SAFETY: version-3 header and two capability words.
    let r = unsafe { syscall(SYS_CAPSET, &hdr as *const CapHeader, data.as_ptr()) };
    if r != 0 {
        panic!("capset: {}", std::io::Error::last_os_error());
    }
}

fn caps_get() -> [CapData; 2] {
    let mut hdr = CapHeader { version: CAP_VERSION_3, pid: 0 };
    let mut data = [CapData::default(); 2];
    // SAFETY: as above.
    let r = unsafe { syscall(SYS_CAPGET, &mut hdr as *mut CapHeader, data.as_mut_ptr()) };
    if r != 0 {
        panic!("capget: {}", std::io::Error::last_os_error());
    }
    data
}

/// Counts of privileged windows, for the log.
static mut ADMIN_WINDOWS: u64 = 0;

fn admin_on() {
    caps_set(CAP_SYS_ADMIN_BIT, CAP_SYS_ADMIN_BIT);
    // SAFETY: single-threaded tool.
    unsafe { ADMIN_WINDOWS += 1 };
}

fn admin_off() {
    caps_set(0, CAP_SYS_ADMIN_BIT);
}

/// From root: become SUDO_UID/SUDO_GID with only CAP_SYS_ADMIN permitted, none effective.
fn drop_to_user() {
    // SAFETY: plain syscalls on this process.
    unsafe {
        if geteuid() != 0 {
            eprintln!("run through sudo: GET_SURFACE_PHYS_ATTR needs CAP_SYS_ADMIN for its ioctls");
            std::process::exit(2);
        }
    }
    let uid: u32 = std::env::var("SUDO_UID").ok().and_then(|s| s.parse().ok()).expect("SUDO_UID");
    let gid: u32 = std::env::var("SUDO_GID").ok().and_then(|s| s.parse().ok()).expect("SUDO_GID");
    assert!(uid != 0 && gid != 0, "SUDO_UID/SUDO_GID must name an ordinary user");
    unsafe {
        assert_eq!(prctl(PR_SET_KEEPCAPS, 1u64, 0u64, 0u64, 0u64), 0, "PR_SET_KEEPCAPS");
        assert_eq!(setgroups(0, std::ptr::null()), 0, "setgroups");
        assert_eq!(setresgid(gid, gid, gid), 0, "setresgid");
        assert_eq!(setresuid(uid, uid, uid), 0, "setresuid");
        assert_eq!(prctl(PR_SET_KEEPCAPS, 0u64, 0u64, 0u64, 0u64), 0, "PR_SET_KEEPCAPS off");
    }
    admin_off();
    let c = caps_get();
    println!(
        "privilege: uid={} gid={gid}; caps effective={:#x} permitted={:#x} inheritable={:#x} \
         (CAP_SYS_ADMIN = bit 21 = {CAP_SYS_ADMIN_BIT:#x}, raised only around GET_SURFACE_PHYS_ATTR)",
        unsafe { getuid() },
        c[0].effective,
        c[0].permitted,
        c[0].inheritable
    );
}

// ---------------------------------------------------------------- RM client

struct Rm {
    ctl: File,
    gpu: File,
    client: u32,
    device: u32,
    live: Vec<u32>,
    next: u32,
}

impl Rm {
    fn open() -> Rm {
        let ctl = OpenOptions::new().read(true).write(true).open("/dev/nvidiactl").expect("open /dev/nvidiactl");
        let gpu = OpenOptions::new().read(true).write(true).open("/dev/nvidia0").expect("open /dev/nvidia0");
        let mut root = NvOs64 { h_class: NV01_ROOT_CLIENT, ..Default::default() };
        esc(ctl.as_raw_fd(), NV_ESC_RM_ALLOC, &mut root);
        assert_eq!(root.status, 0, "root client status");
        let mut rm = Rm { ctl, gpu, client: root.h_object_new, device: 0xCA10_0001, live: Vec::new(), next: 0xCA10_0100 };
        let mut dp = Nv0080Alloc::default();
        let mut p = NvOs64 {
            h_root: rm.client,
            h_object_parent: rm.client,
            h_object_new: rm.device,
            h_class: NV01_DEVICE_0,
            p_alloc_parms: &mut dp as *mut _ as u64,
            ..Default::default()
        };
        esc(rm.ctl.as_raw_fd(), NV_ESC_RM_ALLOC, &mut p);
        assert_eq!(p.status, 0, "device status");
        // RM_ALLOC_MEMORY runs only on a GPU node registered to the client's ctl FD
        // (host_pool.rs:416-420).
        let mut reg: i32 = rm.ctl.as_raw_fd();
        esc(rm.gpu.as_raw_fd(), NV_ESC_REGISTER_FD, &mut reg);
        rm.next = rm.client.wrapping_add(0x100);
        rm
    }

    fn os_descriptor(&mut self, va: *mut u8, len: u64) -> (u32, u32) {
        let h = self.next;
        self.next += 1;
        let mut p = NvOs02Fd {
            h_root: self.client,
            h_object_parent: self.device,
            h_object_new: h,
            h_class: NV01_MEMORY_SYSTEM_OS_DESCRIPTOR,
            flags: OSDESC_FLAGS,
            p_memory: va as u64,
            limit: len - 1,
            fd: -1,
            ..Default::default()
        };
        esc(self.gpu.as_raw_fd(), NV_ESC_RM_ALLOC_MEMORY, &mut p);
        if p.status == 0 {
            self.live.push(h);
        }
        (h, p.status)
    }

    /// One GET_SURFACE_PHYS_ATTR. The caller brackets it with admin_on/admin_off.
    fn phys_attr(&self, h: u32, off: u64) -> (u32, PhysAttr) {
        let mut a = PhysAttr { mem_offset: off, ..Default::default() };
        let mut p = NvOs54 {
            h_client: self.client,
            h_object: h,
            cmd: CMD_GET_SURFACE_PHYS_ATTR,
            params: &mut a as *mut _ as u64,
            params_size: std::mem::size_of::<PhysAttr>() as u32,
            ..Default::default()
        };
        esc(self.ctl.as_raw_fd(), NV_ESC_RM_CONTROL, &mut p);
        (p.status, a)
    }

    fn free(&mut self, h: u32) -> u32 {
        let mut p = NvOs00 { h_root: self.client, h_object_parent: self.device, h_object_old: h, status: 0 };
        esc(self.ctl.as_raw_fd(), NV_ESC_RM_FREE, &mut p);
        if p.status == 0 {
            self.live.retain(|&x| x != h);
        }
        p.status
    }
}

impl Drop for Rm {
    fn drop(&mut self) {
        for h in std::mem::take(&mut self.live) {
            let st = self.free(h);
            println!("drop: freed leftover descriptor {h:#x} status={st:#x}");
        }
        let mut p = NvOs00 { h_root: self.client, h_object_parent: self.client, h_object_old: self.client, status: 0 };
        esc(self.ctl.as_raw_fd(), NV_ESC_RM_FREE, &mut p);
        println!("client {:#x} freed status={:#x}", self.client, p.status);
    }
}

// ---------------------------------------------------------------- host memory

struct Buf {
    fd: i32,
    ptr: *mut u8,
    len: u64,
}

impl Buf {
    fn new(len: u64, memfd: bool) -> Buf {
        let fd = if memfd {
            // SAFETY: NUL-terminated name.
            let fd = unsafe { memfd_create(b"sysmem-iova\0".as_ptr(), MFD_CLOEXEC) };
            assert!(fd >= 0, "memfd_create: {}", std::io::Error::last_os_error());
            assert_eq!(unsafe { ftruncate(fd, len as i64) }, 0, "ftruncate");
            fd
        } else {
            -1
        };
        let flags = if memfd { MAP_SHARED | MAP_POPULATE } else { MAP_PRIVATE | MAP_ANONYMOUS | MAP_POPULATE };
        // SAFETY: fresh mapping of our own buffer.
        let ptr = unsafe { mmap(std::ptr::null_mut(), len as usize, PROT_RW, flags, fd, 0) };
        assert!(ptr as isize != -1, "mmap: {}", std::io::Error::last_os_error());
        let mut off = 0;
        while off < len {
            // SAFETY: inside the mapping.
            unsafe { *ptr.add(off as usize) = 0xA5 };
            off += PAGE;
        }
        Buf { fd, ptr, len }
    }
}

impl Drop for Buf {
    fn drop(&mut self) {
        // SAFETY: our mapping and fd.
        unsafe {
            munmap(self.ptr, self.len as usize);
            if self.fd >= 0 {
                close(self.fd);
            }
        }
    }
}

fn meminfo_kib(key: &str) -> u64 {
    let s = std::fs::read_to_string("/proc/meminfo").unwrap();
    s.lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn foll_pins() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/vmstat").unwrap();
    let get = |k: &str| {
        s.lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0u64)
    };
    (get("nr_foll_pin_acquired "), get("nr_foll_pin_released "))
}

fn align_of(x: u64) -> u64 {
    if x == 0 {
        0
    } else {
        1u64 << x.trailing_zeros()
    }
}

fn human(x: u64) -> String {
    if x >= GIB && x % GIB == 0 {
        format!("{} GiB", x / GIB)
    } else if x >= MIB && x % MIB == 0 {
        format!("{} MiB", x / MIB)
    } else {
        format!("{} KiB", x / 1024)
    }
}

// ---------------------------------------------------------------- measurement

/// Query every `step` bytes (4 KiB = every page); return the maximal IOVA-contiguous
/// segments (offset in buffer, IOVA, length) on that grid and the attributes of page 0.
/// With a step above 4 KiB a break is found only if it does not rejoin base + offset at
/// the next grid point; runs 1 and 2 used the full 4 KiB scan.
fn scan(rm: &Rm, h: u32, len: u64, step: u64) -> (Vec<(u64, u64, u64)>, PhysAttr, f64) {
    let mut segs: Vec<(u64, u64, u64)> = Vec::new();
    let t = Instant::now();
    admin_on();
    let (st0, a0) = rm.phys_attr(h, 0);
    let mut bad = None;
    if st0 != 0 {
        bad = Some((0, st0));
    } else {
        segs.push((0, a0.mem_offset, step));
        let mut off = step;
        while off < len {
            let (st, a) = rm.phys_attr(h, off);
            if st != 0 {
                bad = Some((off, st));
                break;
            }
            let last = segs.last_mut().unwrap();
            if a.mem_offset == last.1 + last.2 {
                last.2 += step;
            } else {
                segs.push((off, a.mem_offset, step));
            }
            off += step;
        }
    }
    admin_off();
    let dt = t.elapsed().as_secs_f64();
    if let Some((off, st)) = bad {
        panic!("GET_SURFACE_PHYS_ATTR at offset {off:#x}: status {st:#x}");
    }
    (segs, a0, dt)
}

fn sample_offsets(len: u64) -> Vec<u64> {
    let mut v: Vec<u64> = (0..len).step_by((64 * MIB) as usize).collect();
    v.push(len / 2);
    v.push(len - PAGE);
    v.push(len - 1);
    v.sort();
    v.dedup();
    v
}

fn guard(len: u64) -> bool {
    let avail = meminfo_kib("MemAvailable:") * 1024;
    let ok = avail >= len + RAM_FLOOR;
    println!(
        "  RAM guard: MemAvailable {} MiB, buffer {} MiB, left {} MiB (floor {} MiB): {}",
        avail / MIB,
        len / MIB,
        avail.saturating_sub(len) / MIB,
        RAM_FLOOR / MIB,
        if ok { "ok" } else { "SKIP" }
    );
    ok
}

/// Register one buffer, scan it, free it, verify. Returns the IOVA base.
fn one(rm: &mut Rm, label: &str, buf: &Buf, step: u64) -> Option<u64> {
    let len = buf.len;
    let (a0, r0) = foll_pins();
    let t = Instant::now();
    let (h, st) = rm.os_descriptor(buf.ptr, len);
    let ms = t.elapsed().as_millis();
    let (a1, r1) = foll_pins();
    println!(
        "  {label}: RM_ALLOC_MEMORY(0x71) handle={h:#x} va={:#x} len={:#x} status={st:#x} in {ms} ms; \
         foll_pin acquired +{} released +{}",
        buf.ptr as u64,
        len,
        a1 - a0,
        r1 - r0
    );
    if st != 0 {
        return None;
    }
    let (segs, attr0, secs) = scan(rm, h, len, step);
    println!(
        "  {label}: queried {} offsets (step {step:#x}) in {:.2} s: {} IOVA-contiguous segment(s); aperture={} (1=SYSMEM) \
         format={:#x} gpuCacheAttr={} contigSegmentSize@0={:#x}",
        len / step,
        secs,
        segs.len(),
        attr0.mem_aperture,
        attr0.mem_format,
        attr0.gpu_cache_attr,
        attr0.contig_segment_size
    );
    for (i, (off, iova, l)) in segs.iter().enumerate().take(24) {
        println!(
            "    seg[{i}] buf+{off:#x} -> IOVA {iova:#x}..{:#x} ({}), IOVA alignment {}",
            iova + l - 1,
            human(*l),
            human(align_of(*iova))
        );
    }
    if segs.len() > 24 {
        println!("    ... {} more segments", segs.len() - 24);
    }
    let base = segs[0].1;
    admin_on();
    let samples: Vec<(u64, u32, PhysAttr)> = sample_offsets(len).into_iter().map(|o| {
        let (st, a) = rm.phys_attr(h, o);
        (o, st, a)
    }).collect();
    admin_off();
    for (o, st, a) in samples {
        let expect = base + o;
        println!(
            "    sample buf+{o:#011x}: status={st:#x} IOVA={:#x} {} contigSegmentSize={:#x}",
            a.mem_offset,
            if segs.len() == 1 && a.mem_offset == expect { "(= base + offset)" } else if a.mem_offset == expect { "(= base + offset)" } else { "(differs from base + offset)" },
            a.contig_segment_size
        );
    }
    let two_mib = base % (2 * MIB) == 0;
    println!(
        "  {label}: RESULT base={base:#x} end={:#x} single-range={} base-alignment={} 2MiB-aligned={} below-4GiB={}",
        base + len - 1,
        segs.len() == 1,
        human(align_of(base)),
        two_mib,
        base + len <= 1 << 32
    );
    let (a2, r2) = foll_pins();
    let st = rm.free(h);
    let (a3, r3) = foll_pins();
    println!(
        "  {label}: RM_FREE status={st:#x}; foll_pin acquired +{} released +{} (while freeing)",
        a3 - a2,
        r3 - r2
    );
    admin_on();
    let (st2, _) = rm.phys_attr(h, 0);
    admin_off();
    println!(
        "  {label}: second query after free: GET_SURFACE_PHYS_ATTR on {h:#x} status={st2:#x} ({})",
        match st2 {
            0 => "STILL PRESENT",
            0x33 => "NV_ERR_INVALID_OBJECT_HANDLE: descriptor gone",
            0x57 => "NV_ERR_OBJECT_NOT_FOUND: descriptor gone",
            _ => "error: descriptor not reachable",
        }
    );
    Some(base)
}

fn measure(sizes: &[u64], reps: u32, memfd: bool, allow_over_1g: bool, pair: bool, step: u64) {
    drop_to_user();
    let mut rm = Rm::open();
    println!("RM client={:#x} device={:#x} (as uid {})", rm.client, rm.device, unsafe { getuid() });

    // Control: the same query without CAP_SYS_ADMIN must be refused.
    {
        let buf = Buf::new(4 * MIB, memfd);
        let (h, st) = rm.os_descriptor(buf.ptr, buf.len);
        assert_eq!(st, 0, "control descriptor");
        let (st_user, _) = rm.phys_attr(h, 0);
        admin_on();
        let (st_admin, a) = rm.phys_attr(h, 0);
        admin_off();
        println!(
            "control: 4 MiB descriptor {h:#x}: GET_SURFACE_PHYS_ATTR without CAP_SYS_ADMIN status={st_user:#x} \
             (0x1b = NV_ERR_INSUFFICIENT_PERMISSIONS); with it status={st_admin:#x} IOVA={:#x}",
            a.mem_offset
        );
        let st = rm.free(h);
        println!("control: RM_FREE status={st:#x}");
    }

    for &mib in sizes {
        let len = mib * MIB;
        println!("STEP {} {} buffer", human(len), if memfd { "memfd MAP_SHARED" } else { "anonymous" });
        if len > GIB && !allow_over_1g {
            println!("  SKIP: over 1 GiB needs --allow-over-1g (switches the GPU to 64-bit IOVAs for this boot)");
            continue;
        }
        let mut bases = Vec::new();
        for rep in 0..reps {
            if !guard(len) {
                break;
            }
            let buf = Buf::new(len, memfd);
            if let Some(b) = one(&mut rm, &format!("rep{rep}"), &buf, step) {
                bases.push(b);
            }
            drop(buf);
        }
        // Same buffer registered again (the VM's guest RAM stays mapped; a re-registration
        // after a device restart would look like this).
        if reps > 0 && guard(len) {
            let buf = Buf::new(len, memfd);
            if let Some(b) = one(&mut rm, "same-buffer-1", &buf, step) {
                bases.push(b);
            }
            if let Some(b) = one(&mut rm, "same-buffer-2", &buf, step) {
                bases.push(b);
            }
        }
        println!(
            "  BASES {}: {}",
            human(len),
            bases.iter().map(|b| format!("{b:#x}")).collect::<Vec<_>>().join(" ")
        );
    }

    if pair {
        let len = 256 * MIB;
        println!("STEP pair: two 256 MiB buffers held at once");
        if guard(2 * len) {
            let b1 = Buf::new(len, memfd);
            let b2 = Buf::new(len, memfd);
            let (h1, s1) = rm.os_descriptor(b1.ptr, len);
            let (h2, s2) = rm.os_descriptor(b2.ptr, len);
            admin_on();
            let q = |h: u32, o: u64| rm.phys_attr(h, o).1.mem_offset;
            let (i1, e1, i2, e2) = (q(h1, 0), q(h1, len - PAGE), q(h2, 0), q(h2, len - PAGE));
            admin_off();
            println!("  pair: A {h1:#x} status={s1:#x} IOVA {i1:#x}..{:#x}", e1 + PAGE - 1);
            println!("  pair: B {h2:#x} status={s2:#x} IOVA {i2:#x}..{:#x}", e2 + PAGE - 1);
            println!(
                "  pair: A and B adjacent: {}",
                i2 + len == i1 || i1 + len == i2
            );
            println!("  pair: RM_FREE A status={:#x} B status={:#x}", rm.free(h1), rm.free(h2));
        }
    }

    // SAFETY: single-threaded.
    println!("privileged windows opened: {}", unsafe { ADMIN_WINDOWS });
    drop(rm);
    let (a, r) = foll_pins();
    println!("end: foll_pin acquired={a} released={r} (host-wide counters)");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 || args[1] != "measure" || !args.iter().any(|a| a == "--go") {
        eprintln!(
            "usage: sudo -n {} measure --go [--sizes 256,1024] [--reps N] [--anon] [--allow-over-1g] [--pair] [--full-scan]",
            args[0]
        );
        std::process::exit(2);
    }
    let mut sizes = vec![256u64, 1024];
    let mut reps = 3;
    let mut memfd = true;
    let mut allow = false;
    let mut pair = false;
    // 2 MiB by default. `--full-scan` queries every 4 KiB page; on a host whose driver
    // logs every ioctl (NVreg_ResmanDebugLevel=0) that writes one kernel log line per page.
    let mut step = 2 * MIB;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--sizes" => {
                i += 1;
                sizes = args[i].split(',').map(|s| s.parse().expect("MiB")).collect();
            }
            "--reps" => {
                i += 1;
                reps = args[i].parse().expect("reps");
            }
            "--anon" => memfd = false,
            "--allow-over-1g" => allow = true,
            "--pair" => pair = true,
            "--full-scan" => step = PAGE,
            "--go" => {}
            a => panic!("unknown argument {a}"),
        }
        i += 1;
    }
    measure(&sizes, reps, memfd, allow, pair, step);
}

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Host-side execution primitive for Caraxes (docs/caraxes/host-exec.md).
//!
//! Proves, as an unprivileged RM client (with CAP_SYS_ADMIN raised only around the
//! two controls that need it), that the physical RTX 2070 (TU106) can be made to
//! execute work through a GMMU v2 page table that "the guest" built in "guest" VRAM,
//! exactly the way the vfio-user-nvgpu device (crates/vfio-user-nvgpu) will have to
//! drive it for CUDA in the guest. It builds up in gated stages A..G; each stage
//! records the RM calls it made and their status. This is the standalone form of the
//! execution model in docs/caraxes/cuda-path.md §4 (option (a): identity-mapped VRAM,
//! host channels bound to the guest's page tables).
//!
//! Builds with rustc alone, like host-facts.rs and sysmem-iova.rs:
//!
//!     rustc -O --edition 2021 caraxes/tools/host-exec.rs -o target/host-exec
//!     sudo -n target/host-exec run --go                 # stages A..D (the copy proof)
//!     sudo -n target/host-exec run --go --max-stage F   # also E (event) and F (promote)
//!
//! or through the wrapper caraxes/tools/host-exec.sh, which captures the host journal
//! and nvidia-smi around the run.
//!
//! ## What touches the host card
//! Only the RM API through /dev/nvidiactl and /dev/nvidia0, as an ordinary client. No
//! register access, no host module change, no passthrough. Every object is owned by the
//! one RM client; closing /dev/nvidiactl frees the whole tree (VRAM reservation and the
//! sysmem pins included) in the kernel if the process dies. A SIGINT/SIGTERM handler
//! closes that fd and _exit()s, so nothing is ever left allocated. `Drop` frees
//! everything explicitly on the normal path and prints each status.
//!
//! ## Safety envelope (the desktop runs on this GPU)
//! - The reservation is 64 MiB, fixed at the TOP of the host FB region (host-facts.md
//!   §3 rule): E = region end aligned down to 2 MiB, offset = E - 64 MiB. Refused unless
//!   64 MiB + 1 GiB of VRAM is free (nvidia-smi and FB_GET_INFO_V2 HEAP_FREE, the smaller).
//! - Every page-table entry is validated (aperture, alignment, in-reservation for vidmem,
//!   in-IOVA-range for sysmem) before the doorbell is ever rung.
//! - The pushbuffer is the smallest that proves the point.
//! - /dev/kmsg is opened as root at start; after every stage the tool reads the new
//!   kernel records and stops at the first `Xid` or NVRM error. The RC watchdog is left on.
//! - Nothing touches guest RAM or host framebuffers outside the reservation.
//!
//! ## Privilege (D12, scoped like sysmem-iova.rs)
//! Started with `sudo -n`; before opening any node it drops to SUDO_UID/SUDO_GID keeping
//! only CAP_SYS_ADMIN in the permitted set (effective empty). It raises CAP_SYS_ADMIN into
//! the effective set only around GET_SURFACE_PHYS_ATTR (0x410103, flags 0x4) and
//! DMA_SET_PAGE_DIRECTORY (0x801813, flags 0x14004) / GPU_PROMOTE_CTX (0x2080012b), and
//! drops it again immediately. Everything else runs as the unprivileged user.
//!
//! ## Layouts and ids, vendor/open-gpu-kernel-modules (610.57.04); sdk/ = src/common/sdk/nvidia/inc
//!   sdk/nvos.h: NVOS02(_FD), NVOS33(_FD), NVOS54, NVOS64, NV_MEMORY_ALLOCATION_PARAMS
//!     (:1600), NV_VASPACE_ALLOCATION_PARAMETERS (:3146), NV0005_ALLOC_PARAMETERS,
//!     event flags (:390,:428,:435), NVOS41 GET_EVENT_DATA
//!   sdk/alloc/alloc_channel.h:296 NV_CHANNEL_ALLOC_PARAMS (376 bytes), NV_MEMORY_DESC_PARAMS
//!   sdk/class/cl0040.h NV01_MEMORY_LOCAL_USER 0x40; cl003e NV01_MEMORY_SYSTEM 0x3e;
//!     cl0071 NV01_MEMORY_SYSTEM_OS_DESCRIPTOR 0x71; cl0080 NV01_DEVICE_0; cl2080 NV20_SUBDEVICE_0;
//!     cl90f1 FERMI_VASPACE_A 0x90f1; clc46f TURING_CHANNEL_GPFIFO_A 0xc46f + host methods;
//!     clc5b5 TURING_DMA_COPY_A 0xc5b5 + CE methods; clc5c0 TURING_COMPUTE_A 0xc5c0;
//!     clc361 VOLTA_USERMODE_A 0xc361 (doorbell 0x90); clb0b5sw NVB0B5_ALLOCATION_PARAMETERS
//!   sdk/ctrl: ctrl0041 GET_SURFACE_PHYS_ATTR 0x410103; ctrl0080dma SET_PAGE_DIRECTORY 0x801813,
//!     UNSET 0x801814 (+FLAGS); ctrla06f GPFIFO_SCHEDULE 0xa06f0103, BIND 0xa06f0104;
//!     ctrlc36f GET_WORK_SUBMIT_TOKEN 0xc36f0108; ctrl2080gpu PROMOTE_CTX 0x2080012b;
//!     ctrl2080event EVENT_SET_NOTIFICATION 0x20800301; ctrl2080fb FB_GET_INFO_V2 0x20801303,
//!     FB_GET_FB_REGION_INFO 0x20801320
//!   GMMU v2: pascal/gp100/dev_mmu.h + turing/tu102/dev_mmu.h (PDE/dual-PDE/PTE); fb-model.md §1.5

use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::process::Command;
use std::sync::atomic::{AtomicI32, Ordering};
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
    fn eventfd(initval: u32, flags: i32) -> i32;
    fn lseek(fd: i32, off: i64, whence: i32) -> i64;
    fn signal(signum: i32, handler: usize) -> usize;
    fn _exit(code: i32) -> !;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
}

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

const SYS_CAPGET: i64 = 125;
const SYS_CAPSET: i64 = 126;
const PR_SET_KEEPCAPS: i32 = 8;
const CAP_VERSION_3: u32 = 0x2008_0522;
const CAP_SYS_ADMIN_BIT: u32 = 1 << 21;

const PROT_RW: i32 = 0x1 | 0x2;
const PROT_WRITE: i32 = 0x2;
const MAP_SHARED: i32 = 0x01;
const MAP_POPULATE: i32 = 0x8000;
const MAP_FAILED: isize = -1;
const MFD_CLOEXEC: u32 = 1;
const SEEK_END: i32 = 2;
const EFD_NONBLOCK: i32 = 0o4000;
const POLLIN: i16 = 0x001;
const SIGINT: i32 = 2;
const SIGTERM: i32 = 15;

// Escapes (kernel-open/common/inc/nv_escape.h, nv-ioctl-numbers.h).
const NV_ESC_RM_FREE: u32 = 0x29;
const NV_ESC_RM_CONTROL: u32 = 0x2A;
const NV_ESC_RM_ALLOC: u32 = 0x2B;
const NV_ESC_RM_ALLOC_MEMORY: u32 = 0x27;
const NV_ESC_RM_MAP_MEMORY: u32 = 0x4E;
const NV_ESC_REGISTER_FD: u32 = 201;
const NV_ESC_ALLOC_OS_EVENT: u32 = 206;
const NV_ESC_RM_GET_EVENT_DATA: u32 = 0x52;

// Classes.
const NV01_ROOT_CLIENT: u32 = 0x41;
const NV01_MEMORY_LOCAL_USER: u32 = 0x40;
const NV01_MEMORY_SYSTEM_OS_DESCRIPTOR: u32 = 0x71;
const NV01_DEVICE_0: u32 = 0x80;
const NV20_SUBDEVICE_0: u32 = 0x2080;
const NV01_EVENT_OS_EVENT: u32 = 0x79;
const FERMI_VASPACE_A: u32 = 0x0000_90F1;
const TURING_CHANNEL_GPFIFO_A: u32 = 0x0000_C46F;
const TURING_DMA_COPY_A: u32 = 0x0000_C5B5;
const TURING_COMPUTE_A: u32 = 0x0000_C5C0;
const VOLTA_USERMODE_A: u32 = 0x0000_C361;

// Controls.
const CMD_GET_SURFACE_PHYS_ATTR: u32 = 0x0041_0103; // PRIVILEGED (flags 0x4)
const CMD_FB_GET_INFO_V2: u32 = 0x2080_1303;
const CMD_FB_GET_FB_REGION_INFO: u32 = 0x2080_1320;
const CMD_DMA_SET_PAGE_DIRECTORY: u32 = 0x0080_1813; // PRIVILEGED (flags 0x14004)
const CMD_GPFIFO_SCHEDULE: u32 = 0xA06F_0103;
const CMD_GET_WORK_SUBMIT_TOKEN: u32 = 0xC36F_0108;
const CMD_GPU_PROMOTE_CTX: u32 = 0x2080_012B; // PRIVILEGED (flags 0x10244)
const CMD_EVENT_SET_NOTIFICATION: u32 = 0x2080_0301;

// NV_MEMORY_ALLOCATION_PARAMS attr/attr2/flags (nvos.h).
const ATTR_PAGE_SIZE_HUGE: u32 = 3 << 23;
const ATTR_LOCATION_VIDMEM: u32 = 0 << 25;
const ATTR_PHYSICALITY_CONTIGUOUS: u32 = 2 << 27;
const ATTR2_PAGE_SIZE_HUGE_2MB: u32 = 1 << 20;
const ALLOC_FLAGS_FIXED_ADDRESS_ALLOCATE: u32 = 0x10;
const OWNER_TAG: u32 = u32::from_le_bytes(*b"cara");

// OS descriptor flags NONCONTIGUOUS | PCI | CACHED | NO_MAP (host_pool.rs:170, as
// sysmem-iova.rs). NO_MAP: RM pins and DMA-maps the pages (so GET_SURFACE_PHYS_ATTR
// returns the IOVA) but makes no CPU mapping - we CPU-touch the backing memfd directly.
const OSDESC_FLAGS: u32 = (1 << 4) | (0 << 8) | (1 << 12) | (1 << 30);

// NV_VASPACE_ALLOCATION_FLAGS.
const VAS_FLAGS_IS_EXTERNALLY_OWNED: u32 = 1 << 3;

// SET_PAGE_DIRECTORY flags.
const SETPD_APERTURE_VIDMEM: u32 = 0;
const SETPD_ALL_CHANNELS_TRUE: u32 = 1 << 3;

// GMMU v2 apertures (dev_mmu.h): PDE/PTE bits 2:1.
const APERTURE_VIDEO: u64 = 1; // PDE next-table in vidmem
const PTE_APERTURE_VIDEO: u64 = 0; // PTE points at vidmem (0), sys coherent (2)
const PTE_APERTURE_SYS_COH: u64 = 2;

// clc361 doorbell offset.
const DOORBELL_OFFSET: usize = 0x90;
const USERMODE_SIZE: usize = 0x10000;

// clc46f USERD offsets.
const USERD_GP_GET: usize = 0x88;
const USERD_GP_PUT: usize = 0x8C;

const PAGE: u64 = 4096;
const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;
const RESV_SIZE: u64 = 64 * MIB;
const HOST_HEADROOM: u64 = GIB;

// NV2080_ENGINE_TYPE (cl2080_notification.h).
const ENGINE_TYPE_COPY2: u32 = 0x0B;

// ---- parameter structs ------------------------------------------------------

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

/// nv_ioctl_nvos02_parameters_with_fd (OS descriptor, class 0x71).
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

/// nv_ioctl_nvos33_parameters_with_fd (NV_ESC_RM_MAP_MEMORY).
#[repr(C)]
#[derive(Default)]
struct NvOs33Fd {
    h_client: u32,
    h_device: u32,
    h_memory: u32,
    _pad0: u32,
    offset: u64,
    length: u64,
    p_linear_address: u64,
    status: u32,
    flags: u32,
    fd: i32,
    _pad1: u32,
}

/// nv_ioctl_alloc_os_event_t.
#[repr(C)]
#[derive(Default)]
struct AllocOsEvent {
    h_client: u32,
    h_device: u32,
    fd: u32,
    status: u32,
}

/// NVOS41_PARAMETERS (GET_EVENT_DATA).
#[repr(C)]
#[derive(Default)]
struct NvOs41 {
    p_event: u64,
    more_events: i32,
    status: i32,
}

/// NvUnixEvent.
#[repr(C)]
#[derive(Default)]
struct NvUnixEvent {
    h_object: u32,
    notify_index: u32,
    info32: u32,
    info16: u16,
    _pad: u16,
}

/// NV_MEMORY_ALLOCATION_PARAMS (nvos.h:1600), 128 bytes.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct MemAlloc {
    owner: u32,
    type_: u32,
    flags: u32,
    width: u32,
    height: u32,
    pitch: i32,
    attr: u32,
    attr2: u32,
    format: u32,
    compr_covg: u32,
    zcull_covg: u32,
    _pad: u32,
    range_lo: u64,
    range_hi: u64,
    size: u64,
    alignment: u64,
    offset: u64,
    limit: u64,
    address: u64,
    ctag_offset: u32,
    h_va_space: u32,
    internalflags: u32,
    tag: u32,
    numa_node: i32,
    _pad2: u32,
}

/// NV0080_ALLOC_PARAMETERS (device).
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

/// NV_VASPACE_ALLOCATION_PARAMETERS (nvos.h:3146).
#[repr(C)]
#[derive(Default)]
struct VaspaceParams {
    index: u32,
    flags: u32,
    va_size: u64,
    va_start_internal: u64,
    va_limit_internal: u64,
    big_page_size: u32,
    _pad: u32,
    va_base: u64,
    pasid: u32,
    _pad2: u32,
}

/// NV_MEMORY_DESC_PARAMS (alloc_channel.h).
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct MemDescParams {
    base: u64,
    size: u64,
    address_space: u32,
    cache_attrib: u32,
}

/// NV_CHANNEL_ALLOC_PARAMS (alloc_channel.h:296), 376 bytes.
#[repr(C)]
#[derive(Default)]
struct ChannelParams {
    h_object_error: u32,
    h_object_buffer: u32,
    gp_fifo_offset: u64,
    gp_fifo_entries: u32,
    flags: u32,
    h_context_share: u32,
    h_vaspace: u32,
    h_handle_vaspace: u32,
    h_userd_memory: [u32; 8],
    userd_offset: [u64; 8],
    engine_type: u32,
    cid: u32,
    sub_device_id: u32,
    h_object_ecc_error: u32,
    instance_mem: MemDescParams,
    userd_mem: MemDescParams,
    ramfc_mem: MemDescParams,
    mthdbuf_mem: MemDescParams,
    h_phys_channel_group: u32,
    internal_flags: u32,
    error_notifier_mem: MemDescParams,
    ecc_error_notifier_mem: MemDescParams,
    process_id: u32,
    sub_process_id: u32,
    encrypt_iv: [u32; 3],
    decrypt_iv: [u32; 3],
    hmac_nonce: [u32; 8],
    tpc_config_id: u32,
}

/// NVB0B5_ALLOCATION_PARAMETERS.
#[repr(C)]
#[derive(Default)]
struct CeAllocParams {
    version: u32,
    engine_type: u32,
}

/// NV0005_ALLOC_PARAMETERS.
#[repr(C)]
#[derive(Default)]
struct EventAllocParams {
    h_parent_client: u32,
    h_src_resource: u32,
    h_class: u32,
    notify_index: u32,
    data: u64,
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

/// NV0080_CTRL_DMA_SET_PAGE_DIRECTORY_PARAMS.
#[repr(C)]
#[derive(Default)]
struct SetPageDir {
    phys_address: u64,
    num_entries: u32,
    flags: u32,
    h_vaspace: u32,
    ch_id: u32,
    sub_device_id: u32,
    pasid: u32,
}

/// NVA06F_CTRL_GPFIFO_SCHEDULE_PARAMS.
#[repr(C)]
#[derive(Default)]
struct ScheduleParams {
    b_enable: u8,
    b_skip_submit: u8,
    b_skip_enable: u8,
}

/// NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN_PARAMS.
#[repr(C)]
#[derive(Default)]
struct WorkSubmitToken {
    token: u32,
}

/// NV2080_CTRL_EVENT_SET_NOTIFICATION_PARAMS.
#[repr(C)]
#[derive(Default)]
struct EventSetNotification {
    event: u32,
    action: u32,
    b_notify_state: u8,
    _pad0: [u8; 3],
    info32: u32,
    info16: u16,
    _pad1: u16,
}

/// NV2080_CTRL_GPU_PROMOTE_CTX_BUFFER_ENTRY.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct PromoteEntry {
    gpu_phys_addr: u64,
    gpu_virt_addr: u64,
    size: u64,
    phys_attr: u32,
    buffer_id: u16,
    b_initialize: u8,
    b_nonmapped: u8,
}

/// NV2080_CTRL_GPU_PROMOTE_CTX_PARAMS (16 entries).
#[repr(C)]
struct PromoteCtx {
    engine_type: u32,
    h_client: u32,
    ch_id: u32,
    h_chan_client: u32,
    h_object: u32,
    h_virt_memory: u32,
    virt_address: u64,
    size: u64,
    entry_count: u32,
    _pad: u32,
    entries: [PromoteEntry; 16],
}

impl Default for PromoteCtx {
    fn default() -> Self {
        PromoteCtx {
            engine_type: 0,
            h_client: 0,
            ch_id: 0,
            h_chan_client: 0,
            h_object: 0,
            h_virt_memory: 0,
            virt_address: 0,
            size: 0,
            entry_count: 0,
            _pad: 0,
            entries: [PromoteEntry::default(); 16],
        }
    }
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
    assert!(std::mem::size_of::<NvOs54>() == 32);
    assert!(std::mem::size_of::<NvOs64>() == 48);
    assert!(std::mem::size_of::<NvOs02Fd>() == 56);
    assert!(std::mem::size_of::<NvOs33Fd>() == 56);
    assert!(std::mem::size_of::<MemAlloc>() == 128);
    assert!(std::mem::size_of::<MemDescParams>() == 24);
    assert!(std::mem::size_of::<ChannelParams>() == 376);
    assert!(std::mem::offset_of!(ChannelParams, userd_offset) == 72);
    assert!(std::mem::offset_of!(ChannelParams, engine_type) == 136);
    assert!(std::mem::offset_of!(ChannelParams, instance_mem) == 152);
    assert!(std::mem::size_of::<PhysAttr>() == 48);
    assert!(std::mem::size_of::<VaspaceParams>() == 56);
    assert!(std::mem::size_of::<PromoteEntry>() == 32);
};

fn iowr(nr: u32, size: usize) -> u64 {
    (3u64 << 30) | ((size as u64) << 16) | ((b'F' as u64) << 8) | nr as u64
}

// ---- capability scoping (as sysmem-iova.rs) ---------------------------------

fn caps_set(effective: u32, permitted: u32) {
    let hdr = CapHeader {
        version: CAP_VERSION_3,
        pid: 0,
    };
    let data = [
        CapData {
            effective,
            permitted,
            inheritable: 0,
        },
        CapData::default(),
    ];
    // SAFETY: version-3 header and two capability words.
    let r = unsafe { syscall(SYS_CAPSET, &hdr as *const CapHeader, data.as_ptr()) };
    if r != 0 {
        panic!("capset: {}", std::io::Error::last_os_error());
    }
}

fn caps_get() -> [CapData; 2] {
    let mut hdr = CapHeader {
        version: CAP_VERSION_3,
        pid: 0,
    };
    let mut data = [CapData::default(); 2];
    // SAFETY: as above.
    let r = unsafe { syscall(SYS_CAPGET, &mut hdr as *mut CapHeader, data.as_mut_ptr()) };
    if r != 0 {
        panic!("capget: {}", std::io::Error::last_os_error());
    }
    data
}

static mut ADMIN_WINDOWS: u64 = 0;

fn admin_on() {
    caps_set(CAP_SYS_ADMIN_BIT, CAP_SYS_ADMIN_BIT);
    // SAFETY: single-threaded tool.
    unsafe { ADMIN_WINDOWS += 1 };
}

fn admin_off() {
    caps_set(0, CAP_SYS_ADMIN_BIT);
}

fn drop_to_user() {
    // SAFETY: plain syscalls on this process.
    unsafe {
        if geteuid() != 0 {
            eprintln!("run through sudo: DMA_SET_PAGE_DIRECTORY and GET_SURFACE_PHYS_ATTR need CAP_SYS_ADMIN");
            std::process::exit(2);
        }
    }
    let uid: u32 = std::env::var("SUDO_UID")
        .ok()
        .and_then(|s| s.parse().ok())
        .expect("SUDO_UID");
    let gid: u32 = std::env::var("SUDO_GID")
        .ok()
        .and_then(|s| s.parse().ok())
        .expect("SUDO_GID");
    assert!(
        uid != 0 && gid != 0,
        "SUDO_UID/SUDO_GID must name an ordinary user"
    );
    // SAFETY: drop the process to the invoking user, keep CAP_SYS_ADMIN permitted only.
    unsafe {
        assert_eq!(
            prctl(PR_SET_KEEPCAPS, 1u64, 0u64, 0u64, 0u64),
            0,
            "PR_SET_KEEPCAPS"
        );
        assert_eq!(setgroups(0, std::ptr::null()), 0, "setgroups");
        assert_eq!(setresgid(gid, gid, gid), 0, "setresgid");
        assert_eq!(setresuid(uid, uid, uid), 0, "setresuid");
        assert_eq!(
            prctl(PR_SET_KEEPCAPS, 0u64, 0u64, 0u64, 0u64),
            0,
            "PR_SET_KEEPCAPS off"
        );
    }
    admin_off();
    let c = caps_get();
    println!(
        "privilege: uid={} gid={gid}; caps effective={:#x} permitted={:#x} \
         (CAP_SYS_ADMIN bit21={CAP_SYS_ADMIN_BIT:#x}, raised only around SET_PAGE_DIRECTORY / GET_SURFACE_PHYS_ATTR / PROMOTE_CTX)",
        unsafe { getuid() },
        c[0].effective,
        c[0].permitted
    );
}

// ---- teardown-safe RM client -----------------------------------------------

static CTL_FD: AtomicI32 = AtomicI32::new(-1);

extern "C" fn on_signal(_sig: i32) {
    // async-signal-safe: closing the ctl fd frees the whole RM client tree in kernel.
    let fd = CTL_FD.load(Ordering::SeqCst);
    if fd >= 0 {
        // SAFETY: close is async-signal-safe.
        unsafe { close(fd) };
    }
    // SAFETY: _exit is async-signal-safe.
    unsafe { _exit(1) };
}

struct Rm {
    ctl: File,
    _gpu: File,
    client: u32,
    device: u32,
    subdevice: u32,
    live: Vec<(u32, u32, &'static str)>, // (parent, handle, what) LIFO
    next: u32,
    kmsg: i32,
}

impl Rm {
    fn esc<T>(&self, nr: u32, arg: &mut T) {
        // SAFETY: T is one of the NVOS structs above, sized as the escape expects.
        let r = unsafe {
            ioctl(
                self.ctl.as_raw_fd(),
                iowr(nr, std::mem::size_of::<T>()),
                arg as *mut T,
            )
        };
        if r < 0 {
            panic!("ioctl nr {nr:#x}: {}", std::io::Error::last_os_error());
        }
    }

    fn esc_on<T>(&self, fd: i32, nr: u32, arg: &mut T) {
        // SAFETY: as above, on a specific fd.
        let r = unsafe { ioctl(fd, iowr(nr, std::mem::size_of::<T>()), arg as *mut T) };
        if r < 0 {
            panic!(
                "ioctl nr {nr:#x} on fd {fd}: {}",
                std::io::Error::last_os_error()
            );
        }
    }

    fn open() -> Rm {
        let ctl = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/nvidiactl")
            .expect("open /dev/nvidiactl");
        let gpu = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/nvidia0")
            .expect("open /dev/nvidia0");
        CTL_FD.store(ctl.as_raw_fd(), Ordering::SeqCst);
        // SAFETY: install SIGINT/SIGTERM handlers that free the client on the way out.
        unsafe {
            signal(SIGINT, on_signal as usize);
            signal(SIGTERM, on_signal as usize);
        }
        // Open /dev/kmsg while still root and seek to the end for Xid monitoring.
        let kmsg = OpenOptions::new()
            .read(true)
            .open("/dev/kmsg")
            .map(|f| {
                let fd = f.as_raw_fd();
                std::mem::forget(f);
                // SAFETY: seek to the newest record.
                unsafe { lseek(fd, 0, SEEK_END) };
                fd
            })
            .unwrap_or(-1);

        let mut rm = Rm {
            ctl,
            _gpu: gpu,
            client: 0,
            device: 0xCA20_0001,
            subdevice: 0xCA20_0002,
            live: Vec::new(),
            next: 0xCA20_0100,
            kmsg,
        };

        let mut root = NvOs64 {
            h_class: NV01_ROOT_CLIENT,
            ..Default::default()
        };
        rm.esc(NV_ESC_RM_ALLOC, &mut root);
        assert_eq!(root.status, 0, "root client");
        rm.client = root.h_object_new;
        CTL_FD.store(rm.ctl.as_raw_fd(), Ordering::SeqCst);

        let mut dp = Nv0080Alloc::default();
        let st = rm.alloc(
            rm.client,
            rm.device,
            NV01_DEVICE_0,
            &mut dp as *mut _ as *mut u8,
            "device",
        );
        assert_eq!(st, 0, "device alloc");
        let mut sp = 0u32;
        let st = rm.alloc(
            rm.device,
            rm.subdevice,
            NV20_SUBDEVICE_0,
            &mut sp as *mut _ as *mut u8,
            "subdevice",
        );
        assert_eq!(st, 0, "subdevice alloc");

        // RM_ALLOC_MEMORY of an OS descriptor runs on a GPU FD registered to the ctl FD.
        let mut reg: i32 = rm.ctl.as_raw_fd();
        rm.esc_on(rm._gpu.as_raw_fd(), NV_ESC_REGISTER_FD, &mut reg);
        rm
    }

    fn alloc(
        &mut self,
        parent: u32,
        handle: u32,
        class: u32,
        params: *mut u8,
        what: &'static str,
    ) -> u32 {
        let mut p = NvOs64 {
            h_root: self.client,
            h_object_parent: parent,
            h_object_new: handle,
            h_class: class,
            p_alloc_parms: params as u64,
            ..Default::default()
        };
        self.esc(NV_ESC_RM_ALLOC, &mut p);
        if p.status == 0 {
            self.live.push((parent, handle, what));
        }
        p.status
    }

    fn control_on(&self, object: u32, cmd: u32, buf: &mut [u8]) -> u32 {
        let mut p = NvOs54 {
            h_client: self.client,
            h_object: object,
            cmd,
            params: buf.as_mut_ptr() as u64,
            params_size: buf.len() as u32,
            ..Default::default()
        };
        self.esc(NV_ESC_RM_CONTROL, &mut p);
        p.status
    }

    /// Read new /dev/kmsg records; return Some(line) at the first Xid or NVRM error.
    fn xid_check(&self, stage: &str) -> Option<String> {
        if self.kmsg < 0 {
            return None;
        }
        let mut hit = None;
        let mut buf = [0u8; 8192];
        loop {
            // SAFETY: nonblocking read of the kmsg fd opened at start.
            let n = unsafe { read(self.kmsg, buf.as_mut_ptr(), buf.len()) };
            if n <= 0 {
                break;
            }
            let s = String::from_utf8_lossy(&buf[..n as usize]);
            let low = s.to_lowercase();
            if low.contains("xid")
                || (low.contains("nvrm")
                    && (low.contains("error") || low.contains("fault") || low.contains("timeout")))
            {
                hit = Some(s.trim().to_string());
                break;
            }
        }
        match &hit {
            Some(l) => println!("  [{stage}] kmsg ALERT: {l}"),
            None => println!("  [{stage}] kmsg: no Xid / NVRM error since last check"),
        }
        hit
    }
}

impl Drop for Rm {
    fn drop(&mut self) {
        // Free children first (LIFO), then the client.
        let live = std::mem::take(&mut self.live);
        for &(parent, h, what) in live.iter().rev() {
            let mut p = NvOs00 {
                h_root: self.client,
                h_object_parent: parent,
                h_object_old: h,
                status: 0,
            };
            self.esc(NV_ESC_RM_FREE, &mut p);
            println!("teardown: free {what} {h:#x} status={:#x}", p.status);
        }
        let mut p = NvOs00 {
            h_root: self.client,
            h_object_parent: self.client,
            h_object_old: self.client,
            status: 0,
        };
        self.esc(NV_ESC_RM_FREE, &mut p);
        println!(
            "teardown: client {:#x} freed status={:#x}",
            self.client, p.status
        );
        if self.kmsg >= 0 {
            // SAFETY: our fd.
            unsafe { close(self.kmsg) };
        }
    }
}

// ---- host memory (memfd-backed, CPU-mapped, GPU via OS descriptor) ----------

struct SysBuf {
    fd: i32,
    ptr: *mut u8,
    len: u64,
    handle: u32,
    iova: u64,
}

impl SysBuf {
    fn new(len: u64) -> SysBuf {
        // SAFETY: NUL-terminated name; fresh memfd of our own.
        let fd = unsafe { memfd_create(b"host-exec\0".as_ptr(), MFD_CLOEXEC) };
        assert!(fd >= 0, "memfd_create: {}", std::io::Error::last_os_error());
        assert_eq!(unsafe { ftruncate(fd, len as i64) }, 0, "ftruncate");
        // SAFETY: fresh shared mapping of our own buffer.
        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                len as usize,
                PROT_RW,
                MAP_SHARED | MAP_POPULATE,
                fd,
                0,
            )
        };
        assert!(
            ptr as isize != MAP_FAILED,
            "mmap: {}",
            std::io::Error::last_os_error()
        );
        // Touch every page so the pin sees real pages.
        let mut off = 0;
        while off < len {
            // SAFETY: inside the mapping.
            unsafe { *ptr.add(off as usize) = 0 };
            off += PAGE;
        }
        SysBuf {
            fd,
            ptr,
            len,
            handle: 0,
            iova: 0,
        }
    }

    fn wr32(&self, off: u64, v: u32) {
        assert!(off + 4 <= self.len);
        // SAFETY: bounds checked; our own mapping.
        unsafe { std::ptr::write_volatile(self.ptr.add(off as usize) as *mut u32, v) };
    }
    fn rd32(&self, off: u64) -> u32 {
        assert!(off + 4 <= self.len);
        // SAFETY: bounds checked; our own mapping.
        unsafe { std::ptr::read_volatile(self.ptr.add(off as usize) as *const u32) }
    }
}

impl Drop for SysBuf {
    fn drop(&mut self) {
        // SAFETY: our mapping and fd.
        unsafe {
            munmap(self.ptr, self.len as usize);
            close(self.fd);
        }
    }
}

// ---- GMMU v2 entry encodings (fb-model.md §1.5) -----------------------------

/// PDE (PD3/PD2/PD1) pointing at the next table in vidmem or sysmem.
/// aperture bits 2:1; vid address 32:8 << 12; sys address 53:8 << 12; VOL bit 3.
fn pde(aperture: u64, phys: u64) -> u64 {
    let addr = (phys >> 12) << 8;
    (aperture << 1) | addr
}

/// Dual PDE (PD0, 16 bytes), small-page half (high qword). We use 4 KiB pages, so
/// only the small half is valid; the big half stays 0 (invalid). aperture 66:65,
/// address 96:72 (vid) or 117:72 (sys), both << 12 (dev_mmu.h). Returned as (lo, hi).
fn dual_pde_small(aperture: u64, phys: u64) -> (u64, u64) {
    // high qword bit positions minus 64: aperture (2:1), address (32:8) for vid.
    let addr = (phys >> 12) << 8;
    let hi = (aperture << 1) | addr;
    (0, hi)
}

/// PTE (4 KiB page): VALID bit 0, aperture 2:1 (0 vid, 2 sys coh), address 53:8 << 12.
fn pte(aperture: u64, phys: u64) -> u64 {
    let addr = (phys >> 12) << 8;
    1 | (aperture << 1) | addr
}

// ---- FB region query --------------------------------------------------------

fn rd(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn wr(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// FB_GET_FB_REGION_INFO: return the end (limit+1) of the last non-reserved region.
fn fb_region_end(rm: &Rm) -> u64 {
    // NV2080_CTRL_CMD_FB_GET_FB_REGION_INFO_PARAMS: numFBRegions u32, pad to 8, then
    // fbRegion[16], each 56 bytes: base u64, limit u64, reserved u64, performance u32,
    // supportCompressed u8, supportISO u8, bProtected u8, pad, blackList tag u32 (+pad).
    // Size and stride are host-facts.rs fb_regions: 8 + 56*16, stride 56.
    let mut buf = vec![0u8; 8 + 56 * 16];
    let st = rm.control_on(rm.subdevice, CMD_FB_GET_FB_REGION_INFO, &mut buf);
    assert_eq!(st, 0, "FB_GET_FB_REGION_INFO status {st:#x}");
    let n = rd(&buf, 0) as usize;
    let mut best_end = 0u64;
    for i in 0..n.min(16) {
        let roff = 8 + i * 56;
        if roff + 24 > buf.len() {
            break;
        }
        let base = u64::from_le_bytes(buf[roff..roff + 8].try_into().unwrap());
        let limit = u64::from_le_bytes(buf[roff + 8..roff + 16].try_into().unwrap());
        let reserved = u64::from_le_bytes(buf[roff + 16..roff + 24].try_into().unwrap());
        println!("  FB region[{i}] base={base:#x} limit={limit:#x} reserved={reserved:#x}");
        if reserved == 0 && limit > base {
            best_end = best_end.max(limit + 1);
        }
    }
    assert!(best_end != 0, "no usable FB region found");
    best_end & !(2 * MIB - 1)
}

fn smi_free_mib() -> u64 {
    let out = Command::new("nvidia-smi")
        .args(["-q", "-d", "MEMORY"])
        .output();
    let Ok(out) = out else { return 0 };
    let s = String::from_utf8_lossy(&out.stdout);
    let mut in_fb = false;
    for l in s.lines() {
        let t = l.trim();
        if t.starts_with("FB Memory Usage") {
            in_fb = true;
        } else if in_fb && t.starts_with("Free") {
            if let Some(v) = t.split(':').nth(1) {
                return v.trim().trim_end_matches("MiB").trim().parse().unwrap_or(0);
            }
        }
    }
    0
}

fn heap_free_mib(rm: &Rm) -> u64 {
    // FB_GET_INFO_V2: fbInfoListSize @0, then {index,data}[128] @4 (host-facts.rs fb_info).
    // HEAP_FREE index = 0x16 (KiB).
    let mut buf = vec![0u8; 4 + 8 * 128];
    wr(&mut buf, 0, 1);
    wr(&mut buf, 4, 0x16); // FBI_HEAP_FREE
    let st = rm.control_on(rm.subdevice, CMD_FB_GET_INFO_V2, &mut buf);
    if st != 0 {
        return 0;
    }
    rd(&buf, 8) as u64 / 1024
}

// ---- physical attribute query (admin) ---------------------------------------

fn phys_attr(rm: &Rm, handle: u32, off: u64) -> (u32, PhysAttr) {
    let mut a = PhysAttr {
        mem_offset: off,
        ..Default::default()
    };
    admin_on();
    let st = rm.control_on(handle, CMD_GET_SURFACE_PHYS_ATTR, unsafe {
        std::slice::from_raw_parts_mut(&mut a as *mut _ as *mut u8, std::mem::size_of::<PhysAttr>())
    });
    admin_off();
    (st, a)
}

// ---- run-context shared across stages ---------------------------------------

struct Ctx {
    resv_handle: u32,
    resv_phys: u64,
    bar1: Vec<MapWindow>, // held BAR1 windows of the reservation
    // page-table offsets inside the reservation
    pd3_off: u64,
    // sysmem GPU buffer (gpfifo/pushbuffer/semaphore/data) and USERD
    sysbuf: Option<SysBuf>,
    userd: Option<SysBuf>,
    // VA layout
    va_base: u64,
    vas: u32,
    channel: u32,
    usermode: u32,
    doorbell: Option<MapWindow>,
    token: u32,
    ce_obj: u32,
    gp_slot: u32, // next free GPFIFO entry
}

struct MapWindow {
    fd: i32,
    ptr: *mut u8,
    len: usize,
}

impl MapWindow {
    fn wr32(&self, off: usize, v: u32) {
        // SAFETY: caller keeps off in range; our own mapping.
        unsafe { std::ptr::write_volatile(self.ptr.add(off) as *mut u32, v) };
    }
    fn rd32(&self, off: usize) -> u32 {
        // SAFETY: as above.
        unsafe { std::ptr::read_volatile(self.ptr.add(off) as *const u32) }
    }
}

impl Drop for MapWindow {
    fn drop(&mut self) {
        // SAFETY: our mapping and fd.
        unsafe {
            munmap(self.ptr, self.len);
            close(self.fd);
        }
    }
}

/// Map `length` bytes of a memory object into the process at object `offset`. The
/// NV_ESC_RM_MAP_MEMORY ioctl is issued on the client's main ctl FD (as nvrm-client does);
/// `params.fd` names the target FD that receives the mmap context (RM allows one per FD),
/// and mmap is done on that target. VRAM (BAR1) and the BAR0 usermode/doorbell page both
/// live behind the GPU node, so their target is a fresh /dev/nvidia0 registered to the
/// client (mem.rs `map_doorbell`); `h_device` is the object's parent (device for FB
/// memory, subdevice for the usermode object).
fn map_window(
    rm: &Rm,
    handle: u32,
    h_device: u32,
    offset: u64,
    length: usize,
    write_only: bool,
) -> Result<MapWindow, u32> {
    let target = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/nvidia0")
        .expect("open map target");
    let tfd = target.as_raw_fd();
    std::mem::forget(target);
    let mut reg = rm.ctl.as_raw_fd();
    rm.esc_on(tfd, NV_ESC_REGISTER_FD, &mut reg);

    // ACCESS 1:0 (0 RW, 2 WO).
    let flags = if write_only { 2 } else { 0 };
    let mut p = NvOs33Fd {
        h_client: rm.client,
        h_device,
        h_memory: handle,
        offset,
        length: length as u64,
        p_linear_address: 0,
        flags,
        fd: tfd,
        ..Default::default()
    };
    // Issue on the main client ctl FD; the driver installs the context on params.fd.
    rm.esc(NV_ESC_RM_MAP_MEMORY, &mut p);
    if p.status != 0 {
        // SAFETY: our fd.
        unsafe { close(tfd) };
        return Err(p.status);
    }
    let prot = if write_only { PROT_WRITE } else { PROT_RW };
    // SAFETY: RM installed the mapping context on tfd; mmap at offset 0.
    let ptr = unsafe { mmap(std::ptr::null_mut(), length, prot, MAP_SHARED, tfd, 0) };
    if ptr as isize == MAP_FAILED {
        // SAFETY: our fd.
        unsafe { close(tfd) };
        return Err(0xFFFF_FFFF);
    }
    Ok(MapWindow {
        fd: tfd,
        ptr,
        len: length,
    })
}

// ---- submission --------------------------------------------------------------

/// Header word for an incrementing method group (clc46f.h dma method format).
fn inc_hdr(method: u32, count: u32, subch: u32) -> u32 {
    (1u32 << 29) | (count << 16) | (subch << 13) | (method >> 2)
}

/// Push a host semaphore release (NVC46F SEM_ADDR/PAYLOAD/EXECUTE) of `payload` to
/// `sem_va`, on subchannel 0 (host methods, independent of the bound object).
fn push_host_sem_release(pb: &mut Vec<u32>, sem_va: u64, payload: u32) {
    pb.push(inc_hdr(0x5c, 5, 0)); // SEM_ADDR_LO..SEM_EXECUTE, 5 methods
    pb.push((sem_va as u32) & 0xFFFF_FFFC); // SEM_ADDR_LO (31:2)
    pb.push((sem_va >> 32) as u32 & 0xFF); // SEM_ADDR_HI (7:0)
    pb.push(payload); // SEM_PAYLOAD_LO
    pb.push(0); // SEM_PAYLOAD_HI
    pb.push(0x1); // SEM_EXECUTE: OPERATION_RELEASE, 32-bit, WFI dis
}

/// Write `pb` at the pushbuffer page, clear the semaphore page, enqueue one GPFIFO
/// entry at the next slot, ring the doorbell, and poll the semaphore for `poll_payload`.
/// The semaphore page is sysbuf +0x2000, the pushbuffer page sysbuf +0x1000 (VA pb_va).
#[allow(clippy::too_many_arguments)]
fn submit_and_poll(
    sysbuf: &SysBuf,
    userd: &SysBuf,
    db: &MapWindow,
    token: u32,
    slot: &mut u32,
    pb: &[u32],
    pb_va: u64,
    poll_payload: u32,
) -> (bool, std::time::Duration) {
    for (i, d) in pb.iter().enumerate() {
        sysbuf.wr32(0x1000 + (i * 4) as u64, *d);
    }
    sysbuf.wr32(0x2000, 0); // clear semaphore
    let entry = 8 * *slot as u64;
    sysbuf.wr32(entry, (pb_va as u32) & 0xFFFF_FFFC); // GP_ENTRY0
    sysbuf.wr32(
        entry + 4,
        ((pb_va >> 32) as u32 & 0xFF) | ((pb.len() as u32) << 10),
    ); // GP_ENTRY1 LENGTH
    *slot += 1;
    userd.wr32(USERD_GP_PUT as u64, *slot);
    let t0 = Instant::now();
    db.wr32(DOORBELL_OFFSET, token);
    let mut done = false;
    while t0.elapsed() < std::time::Duration::from_secs(3) {
        if sysbuf.rd32(0x2000) == poll_payload {
            done = true;
            break;
        }
        std::hint::spin_loop();
    }
    (done, t0.elapsed())
}

// ---- stages -----------------------------------------------------------------

/// Stage A: reserve 64 MiB of VRAM at the top of the FB region and map it through BAR1.
fn stage_a(rm: &mut Rm, ctx: &mut Ctx) {
    println!("\n=== Stage A: reserve VRAM at top of FB region, map through BAR1 ===");
    let e = fb_region_end(rm);
    let offset = e - RESV_SIZE;
    let smi = smi_free_mib();
    let heap = heap_free_mib(rm);
    let free = smi.min(heap);
    println!(
        "  FB region end E={e:#x}; reservation [{:#x}, {:#x}) 64 MiB",
        offset, e
    );
    println!(
        "  free VRAM: nvidia-smi {smi} MiB, HEAP_FREE {heap} MiB; need {} MiB",
        (RESV_SIZE + HOST_HEADROOM) / MIB
    );
    assert!(
        free * MIB >= RESV_SIZE + HOST_HEADROOM,
        "refuse: not enough free VRAM (desktop headroom)"
    );

    // Primary: fixed-address contiguous at the top of the FB region (host-facts §3 D11 rule).
    let mut p = MemAlloc {
        owner: OWNER_TAG,
        type_: 0,
        flags: ALLOC_FLAGS_FIXED_ADDRESS_ALLOCATE,
        attr: ATTR_LOCATION_VIDMEM | ATTR_PHYSICALITY_CONTIGUOUS | ATTR_PAGE_SIZE_HUGE,
        attr2: ATTR2_PAGE_SIZE_HUGE_2MB,
        size: RESV_SIZE,
        offset,
        ..Default::default()
    };
    let h = rm.next;
    rm.next += 1;
    let mut st = rm.alloc(
        rm.device,
        h,
        NV01_MEMORY_LOCAL_USER,
        &mut p as *mut _ as *mut u8,
        "resv-vram",
    );
    println!(
        "  RM_ALLOC NV01_MEMORY_LOCAL_USER(0x40) fixed@{offset:#x} status={st:#x} phys={:#x} attr={:#x}",
        p.offset, p.attr
    );
    if st == 0x51 {
        // NV_ERR_NO_MEMORY: the top of the region is occupied (host-facts §5 item 4 -
        // here by the running vfio-user-nvgpu --host-rm VMs). D11 says the device REFUSES
        // (the guest FB layout is fixed at device start). For this standalone execution
        // proof the top address is not essential; fall back to a floating contiguous 64 MiB
        // range and use the physical base RM returns. Recorded as a deviation.
        println!("  fixed-top REFUSED (NO_MEMORY): top range occupied. Falling back to floating contiguous 64 MiB (proof only).");
        p = MemAlloc {
            owner: OWNER_TAG,
            type_: 0,
            flags: 0,
            attr: ATTR_LOCATION_VIDMEM | ATTR_PHYSICALITY_CONTIGUOUS | ATTR_PAGE_SIZE_HUGE,
            attr2: ATTR2_PAGE_SIZE_HUGE_2MB,
            size: RESV_SIZE,
            ..Default::default()
        };
        st = rm.alloc(
            rm.device,
            h,
            NV01_MEMORY_LOCAL_USER,
            &mut p as *mut _ as *mut u8,
            "resv-vram",
        );
        println!(
            "  RM_ALLOC NV01_MEMORY_LOCAL_USER(0x40) floating contiguous status={st:#x} phys={:#x} attr={:#x}",
            p.offset, p.attr
        );
    }
    assert_eq!(st, 0, "VRAM reservation refused");
    assert_eq!((p.attr >> 27) & 3, 2, "reservation not contiguous");
    ctx.resv_handle = h;
    ctx.resv_phys = p.offset;

    // Largest window RM grants: try full 64 MiB down to 16 MiB.
    let mut granted = 0usize;
    for &mib in &[64u64, 32, 16] {
        match map_window(rm, h, rm.device, 0, (mib * MIB) as usize, false) {
            Ok(w) => {
                println!("  RM_MAP_MEMORY window {mib} MiB: NV_OK (BAR1)");
                granted = (mib * MIB) as usize;
                ctx.bar1.push(w);
                break;
            }
            Err(e) => println!("  RM_MAP_MEMORY window {mib} MiB: status {e:#x}"),
        }
    }
    assert!(granted > 0, "no BAR1 window granted");

    // Can several windows be held at once? Map a second, disjoint 16 MiB window.
    match map_window(rm, h, rm.device, 16 * MIB, (16 * MIB) as usize, false) {
        Ok(w) => {
            println!("  second concurrent window (16 MiB @ +16 MiB): NV_OK -> multiple windows can be held");
            ctx.bar1.push(w);
        }
        Err(e) => {
            println!("  second concurrent window: status {e:#x} -> only one window at a time")
        }
    }

    // CPU write / read-back through BAR1 (GPU-visibility is proven by Stage D's copy).
    let w = &ctx.bar1[0];
    w.wr32(0x300000, 0xA5A5_1234);
    let back = w.rd32(0x300000);
    println!(
        "  CPU write/read-back at reservation +0x300000: wrote 0xA5A51234 read {back:#010x} ({})",
        if back == 0xA5A5_1234 {
            "OK"
        } else {
            "MISMATCH"
        }
    );
    assert_eq!(back, 0xA5A5_1234, "BAR1 CPU write not read back");
}

/// Stage B: pin one sysmem buffer, read its IOVA, and build the GMMU v2 page table in VRAM.
fn stage_b(rm: &mut Rm, ctx: &mut Ctx) {
    println!("\n=== Stage B: pin sysmem, read IOVA (admin), build GMMU v2 page table in VRAM ===");

    // One 8-page sysmem buffer: [0]=GPFIFO [1]=pushbuffer [2]=semaphore
    // [3]=sysmem-data-src [4]=sysmem-data-dst. USERD is a separate 1-page buffer.
    let mut sysbuf = SysBuf::new(8 * PAGE);
    let h = rm.next;
    rm.next += 1;
    let mut p = NvOs02Fd {
        h_root: rm.client,
        h_object_parent: rm.device,
        h_object_new: h,
        h_class: NV01_MEMORY_SYSTEM_OS_DESCRIPTOR,
        flags: OSDESC_FLAGS,
        p_memory: sysbuf.ptr as u64,
        limit: sysbuf.len - 1,
        fd: -1,
        ..Default::default()
    };
    rm.esc_on(rm._gpu.as_raw_fd(), NV_ESC_RM_ALLOC_MEMORY, &mut p);
    println!(
        "  RM_ALLOC_MEMORY OS descriptor(0x71) 8 pages status={:#x} handle={h:#x}",
        p.status
    );
    assert_eq!(p.status, 0, "OS descriptor refused");
    rm.live.push((rm.device, h, "sysbuf-osdesc"));
    sysbuf.handle = h;
    let (st, a) = phys_attr(rm, h, 0);
    println!(
        "  GET_SURFACE_PHYS_ATTR(admin) status={st:#x} IOVA={:#x} aperture={} (1=SYSMEM)",
        a.mem_offset, a.mem_aperture
    );
    assert_eq!(st, 0, "IOVA query refused");
    sysbuf.iova = a.mem_offset;

    // USERD page (client-allocated USERD as an OS descriptor).
    let mut userd = SysBuf::new(PAGE);
    let uh = rm.next;
    rm.next += 1;
    let mut up = NvOs02Fd {
        h_root: rm.client,
        h_object_parent: rm.device,
        h_object_new: uh,
        h_class: NV01_MEMORY_SYSTEM_OS_DESCRIPTOR,
        flags: OSDESC_FLAGS,
        p_memory: userd.ptr as u64,
        limit: userd.len - 1,
        fd: -1,
        ..Default::default()
    };
    rm.esc_on(rm._gpu.as_raw_fd(), NV_ESC_RM_ALLOC_MEMORY, &mut up);
    println!(
        "  RM_ALLOC_MEMORY USERD OS descriptor(0x71) status={:#x} handle={uh:#x}",
        up.status
    );
    assert_eq!(up.status, 0, "USERD OS descriptor refused");
    rm.live.push((rm.device, uh, "userd-osdesc"));
    userd.handle = uh;
    let (ust, ua) = phys_attr(rm, uh, 0);
    assert_eq!(ust, 0, "USERD IOVA query refused");
    userd.iova = ua.mem_offset;
    println!("  USERD IOVA={:#x}", userd.iova);

    // VA layout in one 2 MiB region (one PT). va_base 0x200000.
    ctx.va_base = 0x20_0000;
    let sb = sysbuf.iova;
    let va = ctx.va_base;
    // targets: (name, VA, aperture, phys)
    let data_src_vram = ctx.resv_phys + 0x40_0000; // inside reservation, away from PT area
    let targets = [
        ("GPFIFO ", va, PTE_APERTURE_SYS_COH, sb),
        ("pushbuf", va + 0x1000, PTE_APERTURE_SYS_COH, sb + 0x1000),
        ("semaphr", va + 0x2000, PTE_APERTURE_SYS_COH, sb + 0x2000),
        ("vram-dat", va + 0x3000, PTE_APERTURE_VIDEO, data_src_vram),
        ("sys-src", va + 0x4000, PTE_APERTURE_SYS_COH, sb + 0x3000),
        ("sys-dst", va + 0x5000, PTE_APERTURE_SYS_COH, sb + 0x4000),
    ];

    // Page-table area inside the reservation: PD3@0, PD2@0x1000, PD1@0x2000,
    // PD0@0x3000, PT@0x4000 (all 4 KiB). Written through the BAR1 window.
    ctx.pd3_off = 0;
    let (pd3, pd2, pd1, pd0, ptt) = (0u64, 0x1000u64, 0x2000u64, 0x3000u64, 0x4000u64);
    let w = &ctx.bar1[0];
    // zero the 5 table pages
    for pg in [pd3, pd2, pd1, pd0, ptt] {
        for i in 0..(PAGE / 4) {
            w.wr32((pg + i * 4) as usize, 0);
        }
    }
    let phys = |off: u64| ctx.resv_phys + off;
    // PD3[0] -> PD2 (video), PD2[0] -> PD1, PD1[0] -> PD0.
    write_u64(w, pd3, pde(APERTURE_VIDEO, phys(pd2)));
    write_u64(w, pd2, pde(APERTURE_VIDEO, phys(pd1)));
    write_u64(w, pd1, pde(APERTURE_VIDEO, phys(pd0)));
    // PD0 index (VA>>21)&0xFF = 1 for va_base 0x200000; dual PDE small half -> PT (16 bytes/entry).
    let pd0_idx = ((va >> 21) & 0xFF) as u64;
    let (lo, hi) = dual_pde_small(APERTURE_VIDEO, phys(ptt));
    write_u64(w, pd0 + pd0_idx * 16, lo);
    write_u64(w, pd0 + pd0_idx * 16 + 8, hi);
    println!(
        "  PD3@{:#x}={:#018x} PD2@{:#x} PD1@{:#x} PD0[{}] small->PT@{:#x}",
        phys(pd3),
        pde(APERTURE_VIDEO, phys(pd2)),
        phys(pd2),
        phys(pd1),
        pd0_idx,
        phys(ptt)
    );

    // PTEs, with validation before writing.
    for (name, tva, ap, tphys) in targets {
        validate_pte(ap, tphys, ctx.resv_phys, sb, sysbuf.len);
        let pt_idx = ((tva >> 12) & 0x1FF) as u64;
        let e = pte(ap, tphys);
        write_u64(w, ptt + pt_idx * 8, e);
        println!("    PTE[{pt_idx:>3}] {name} VA={tva:#x} ap={ap} phys/iova={tphys:#x} -> {e:#018x} (validated)");
    }

    // Flush the write-combined BAR1 window: read every table page back so the entries
    // are in VRAM before RM walks them (SET_PAGE_DIRECTORY / channel alloc validate the
    // GPFIFO VA against these tables; a stale read yields NV_ERR_INVALID_ADDRESS).
    let mut acc = 0u32;
    for pg in [pd3, pd2, pd1, pd0, ptt] {
        acc ^= w.rd32(pg as usize);
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    println!("  page-table BAR1 write flushed (readback checksum {acc:#x})");

    ctx.sysbuf = Some(sysbuf);
    ctx.userd = Some(userd);
}

fn write_u64(w: &MapWindow, off: u64, v: u64) {
    w.wr32(off as usize, v as u32);
    w.wr32((off + 4) as usize, (v >> 32) as u32);
}

/// Validate a PTE target before it can be walked by the physical GPU.
fn validate_pte(aperture: u64, phys: u64, resv_phys: u64, iova_base: u64, iova_len: u64) {
    assert_eq!(
        phys & (PAGE - 1),
        0,
        "PTE target not page aligned: {phys:#x}"
    );
    match aperture {
        PTE_APERTURE_VIDEO => {
            assert!(
                phys >= resv_phys && phys < resv_phys + RESV_SIZE,
                "vidmem PTE {phys:#x} outside reservation [{resv_phys:#x}, +64 MiB) - REFUSED"
            );
        }
        PTE_APERTURE_SYS_COH => {
            assert!(
                phys >= iova_base && phys < iova_base + iova_len,
                "sysmem PTE {phys:#x} outside pinned IOVA range - REFUSED"
            );
        }
        _ => panic!("unexpected aperture {aperture}"),
    }
}

/// Stage C: VA space (externally owned), channel on it, SET_PAGE_DIRECTORY (admin),
/// schedule, work-submit token, map VOLTA_USERMODE_A doorbell.
fn stage_c(rm: &mut Rm, ctx: &mut Ctx) {
    println!("\n=== Stage C: VA space, channel, SET_PAGE_DIRECTORY, schedule, token, doorbell ===");

    // Externally-owned FERMI_VASPACE_A: RM owns no page tables; we supply the PDB.
    let vas = rm.next;
    rm.next += 1;
    let mut vp = VaspaceParams {
        index: 0, // NEW
        flags: VAS_FLAGS_IS_EXTERNALLY_OWNED,
        va_size: 1u64 << 40, // 1 TiB; root PD3 with 1 entry covers 2^47
        big_page_size: 64 * 1024,
        ..Default::default()
    };
    let st = rm.alloc(
        rm.device,
        vas,
        FERMI_VASPACE_A,
        &mut vp as *mut _ as *mut u8,
        "vaspace",
    );
    println!(
        "  RM_ALLOC FERMI_VASPACE_A externally-owned status={st:#x} vaSize={:#x}",
        vp.va_size
    );
    assert_eq!(st, 0, "VA space alloc refused");
    ctx.vas = vas;

    let userd = ctx.userd.as_ref().unwrap();

    // SET_PAGE_DIRECTORY first: an externally-owned VAS has no PDB until we commit one,
    // and a channel cannot bind to a VAS in that state (channel alloc returns
    // NV_ERR_INVALID_STATE 0x40). Point the VAS at our PD3 in VRAM (PRIVILEGED, scope admin).
    let mut sp = SetPageDir {
        phys_address: ctx.resv_phys + ctx.pd3_off,
        num_entries: 1,
        flags: SETPD_APERTURE_VIDMEM | SETPD_ALL_CHANNELS_TRUE,
        h_vaspace: vas,
        sub_device_id: 1, // subDeviceId = instance+1 (unicast)
        ..Default::default()
    };
    admin_on();
    let st = rm.control_on(rm.device, CMD_DMA_SET_PAGE_DIRECTORY, unsafe {
        std::slice::from_raw_parts_mut(
            &mut sp as *mut _ as *mut u8,
            std::mem::size_of::<SetPageDir>(),
        )
    });
    admin_off();
    println!(
        "  DMA_SET_PAGE_DIRECTORY(admin) PDB={:#x} numEntries=1 aperture=VIDMEM status={st:#x}",
        ctx.resv_phys + ctx.pd3_off
    );
    assert_eq!(st, 0, "SET_PAGE_DIRECTORY refused");

    // TURING_CHANNEL_GPFIFO_A on the VAS. GPFIFO at VA va_base (sysmem via our PTE).
    // USERD is the client-allocated OS-descriptor page. Bare channel: RM wraps a TSG.
    let chan = rm.next;
    rm.next += 1;
    let mut cp = ChannelParams {
        gp_fifo_offset: ctx.va_base, // GPFIFO GPU VA
        gp_fifo_entries: 32,
        flags: 0,
        h_vaspace: vas,
        engine_type: ENGINE_TYPE_COPY2,
        ..Default::default()
    };
    cp.h_userd_memory[0] = userd.handle;
    cp.userd_offset[0] = 0;
    // A bare channel is a child of the device; RM wraps it in an internal TSG. RM validates
    // the GPFIFO VA against our page tables; a not-yet-flushed BAR1 write can still surface
    // as NV_ERR_INVALID_ADDRESS (0x1e), so retry a few times.
    let mut st = 0x1e;
    for attempt in 0..4 {
        st = rm.alloc(
            rm.device,
            chan,
            TURING_CHANNEL_GPFIFO_A,
            &mut cp as *mut _ as *mut u8,
            "channel",
        );
        if st != 0x1e {
            break;
        }
        println!("  channel alloc attempt {attempt}: NV_ERR_INVALID_ADDRESS (page-table read raced); retrying");
    }
    println!(
        "  RM_ALLOC TURING_CHANNEL_GPFIFO_A status={st:#x} (USERD sysmem OS-descriptor, GPFIFO VA {:#x})",
        ctx.va_base
    );
    assert_eq!(st, 0, "channel alloc refused");
    ctx.channel = chan;

    // A CE object on the channel: GPFIFO_SCHEDULE requires an engine object to have been
    // allocated on the channel (or NVA06F_CTRL_CMD_BIND, ctrla06fgpfifo.h:36-45).
    let ce = rm.next;
    rm.next += 1;
    // VERSION_1: engineType is an NV2080_ENGINE_TYPE ordinal (clb0b5sw.h). Match the
    // channel's engine (COPY2).
    let mut cep = CeAllocParams {
        version: 1,
        engine_type: ENGINE_TYPE_COPY2,
    };
    let st = rm.alloc(
        chan,
        ce,
        TURING_DMA_COPY_A,
        &mut cep as *mut _ as *mut u8,
        "ce-object",
    );
    println!("  RM_ALLOC TURING_DMA_COPY_A(version 1, COPY2) on channel status={st:#x}");
    assert_eq!(st, 0, "CE object alloc refused");
    ctx.ce_obj = ce;

    // Schedule the channel.
    let mut schp = ScheduleParams {
        b_enable: 1,
        ..Default::default()
    };
    let st = rm.control_on(chan, CMD_GPFIFO_SCHEDULE, unsafe {
        std::slice::from_raw_parts_mut(
            &mut schp as *mut _ as *mut u8,
            std::mem::size_of::<ScheduleParams>(),
        )
    });
    println!("  GPFIFO_SCHEDULE bEnable=1 status={st:#x}");
    assert_eq!(st, 0, "schedule refused");

    // Work-submit token.
    let mut tok = WorkSubmitToken::default();
    let st = rm.control_on(chan, CMD_GET_WORK_SUBMIT_TOKEN, unsafe {
        std::slice::from_raw_parts_mut(
            &mut tok as *mut _ as *mut u8,
            std::mem::size_of::<WorkSubmitToken>(),
        )
    });
    println!(
        "  GET_WORK_SUBMIT_TOKEN status={st:#x} token={:#x}",
        tok.token
    );
    assert_eq!(st, 0, "token refused");
    ctx.token = tok.token;

    // VOLTA_USERMODE_A + doorbell mapping.
    let um = rm.next;
    rm.next += 1;
    let st = rm.alloc(
        rm.subdevice,
        um,
        VOLTA_USERMODE_A,
        std::ptr::null_mut(),
        "usermode",
    );
    println!("  RM_ALLOC VOLTA_USERMODE_A status={st:#x}",);
    assert_eq!(st, 0, "usermode alloc refused");
    ctx.usermode = um;
    match map_window(rm, um, rm.subdevice, 0, USERMODE_SIZE, true) {
        Ok(w) => {
            println!("  RM_MAP_MEMORY VOLTA_USERMODE_A doorbell page: NV_OK");
            ctx.doorbell = Some(w);
        }
        Err(e) => panic!("doorbell map refused: {e:#x}"),
    }
}

/// Stage D: semaphore release through the guest-built VAS, then a CE copy, verifying bytes.
fn stage_d(rm: &mut Rm, ctx: &mut Ctx) {
    println!(
        "\n=== Stage D: submit through the doorbell; prove the GPU walks the VRAM page table ==="
    );
    let va = ctx.va_base;
    let sem_va = va + 0x2000;
    let pb_va = va + 0x1000;
    let sem_payload = 0x0000_CAFEu32;

    // Part 1: a bare host-semaphore-release pushbuffer. SEM_ADDR is the semaphore VA,
    // which the physical GPU can only reach by walking the page table we built in VRAM.
    let mut pb: Vec<u32> = Vec::new();
    push_host_sem_release(&mut pb, sem_va, sem_payload);

    let sysbuf = ctx.sysbuf.as_ref().unwrap();
    let userd = ctx.userd.as_ref().unwrap();
    let db = ctx.doorbell.as_ref().unwrap();
    println!(
        "  doorbell token {:#x}; submitting host semaphore-release to VA {sem_va:#x} (payload {sem_payload:#x})",
        ctx.token
    );
    let (done, lat) = submit_and_poll(
        sysbuf,
        userd,
        db,
        ctx.token,
        &mut ctx.gp_slot,
        &pb,
        pb_va,
        sem_payload,
    );
    let gp_get = userd.rd32(USERD_GP_GET as u64);
    println!(
        "  semaphore={:#x} GPGet={} latency={:?} => {}",
        sysbuf.rd32(0x2000),
        gp_get,
        lat,
        if done {
            "GPU WALKED THE VRAM PAGE TABLE AND RELEASED THE SEMAPHORE"
        } else {
            "TIMEOUT (no release)"
        }
    );
    if !done {
        println!("  Stage D part 1 did not complete; stopping before the CE copy.");
        return;
    }

    // Part 2: CE copy on the TURING_DMA_COPY_A object from Stage C.
    println!(
        "  CE object (TURING_DMA_COPY_A {:#x}) allocated in Stage C; running the bidirectional copy",
        ctx.ce_obj
    );
    ce_copy_roundtrip(rm, ctx);
}

/// Run one sysmem->VRAM copy and one VRAM->sysmem copy on the copy engine and verify the
/// bytes. Uses physical CE addressing (SET_SRC/DST_PHYS_MODE + OFFSET = physical/IOVA), the
/// mode UVM uses for sysmem chunk copies: it moves data between the reserved VRAM and the
/// pinned sysmem the GPU reaches by IOVA. The page-table walk itself was already proven by
/// the Stage D part 1 semaphore release (a VA translated through the guest-built tables).
fn ce_copy_roundtrip(_rm: &mut Rm, ctx: &mut Ctx) {
    let sysbuf = ctx.sysbuf.as_ref().unwrap();
    let userd = ctx.userd.as_ref().unwrap();
    let db = ctx.doorbell.as_ref().unwrap();
    let sem_va = ctx.va_base + 0x2000;
    let pb_va = ctx.va_base + 0x1000;
    let nbytes = 256u32;

    // Physical targets: sys-src (sysbuf page 3), sys-dst (page 4), and a VRAM page in the
    // reservation (resv_phys + 0x400000, reached via the BAR1 window at offset 0x400000).
    let sys_src = sysbuf.iova + 0x3000;
    let sys_dst = sysbuf.iova + 0x4000;
    let vram_phys = ctx.resv_phys + 0x40_0000;
    let w = &ctx.bar1[0];

    // Virtual VAs mapped by our page table (Stage B PTEs 3/4/5): vram-dat, sys-src, sys-dst.
    let (vram_va, src_va, dst_va) = (
        ctx.va_base + 0x3000,
        ctx.va_base + 0x4000,
        ctx.va_base + 0x5000,
    );
    let _ = (sys_src, sys_dst); // physical values kept for the log only

    // The FIRST CE LAUNCH_DMA on a freshly scheduled channel stalls once (the channel then
    // RC-recovers); every later CE copy runs. So copy 1 is a warm-up (sysmem->sysmem, its
    // result is not required) and copy 2 is the measured VRAM->sysmem read.
    for i in 0..(nbytes / 4) {
        sysbuf.wr32(0x3000 + (i * 4) as u64, 0x5EED_0000 | i);
        sysbuf.wr32(0x4000 + (i * 4) as u64, 0);
    }
    let mut pb: Vec<u32> = Vec::new();
    pb.push(inc_hdr(0x0000, 1, 0)); // SET_OBJECT subch 0 -> TURING_DMA_COPY_A
    pb.push(TURING_DMA_COPY_A);
    push_ce_copy_virt(&mut pb, src_va, dst_va, nbytes);
    push_wfi(&mut pb);
    push_host_sem_release(&mut pb, sem_va, 2);
    let (warm, wlat) = submit_and_poll(
        sysbuf,
        userd,
        db,
        ctx.token,
        &mut ctx.gp_slot,
        &pb,
        pb_va,
        2,
    );
    let warm_ok =
        (0..(nbytes / 4)).all(|i| sysbuf.rd32(0x4000 + (i * 4) as u64) == (0x5EED_0000 | i));
    println!(
        "  warm-up CE copy (sysmem->sysmem) done={warm} latency={wlat:?} bytes {}",
        if warm_ok {
            "MATCH"
        } else {
            "MISMATCH (absorbs the first-launch stall)"
        }
    );

    // Measured copy: the CPU seeds the reserved VRAM through BAR1; the CE reads it at VA
    // vram_va (walking the guest-built tables) and writes sys-dst. Proves the CPU->VRAM path
    // and the CE's page-table walk of the reservation together.
    for i in 0..(nbytes / 4) {
        w.wr32((0x40_0000 + i * 4) as usize, 0xC0DE_0000 | i); // seed VRAM via BAR1
        sysbuf.wr32(0x4000 + (i * 4) as u64, 0); // clear sys-dst
    }
    w.rd32(0x40_0000); // flush write-combined BAR1
    let mut pb: Vec<u32> = Vec::new();
    pb.push(inc_hdr(0x0000, 1, 0));
    pb.push(TURING_DMA_COPY_A);
    push_ce_copy_virt(&mut pb, vram_va, dst_va, nbytes);
    push_wfi(&mut pb);
    push_host_sem_release(&mut pb, sem_va, 3);
    let (done, lat) = submit_and_poll(
        sysbuf,
        userd,
        db,
        ctx.token,
        &mut ctx.gp_slot,
        &pb,
        pb_va,
        3,
    );
    let d2s_ok =
        (0..(nbytes / 4)).all(|i| sysbuf.rd32(0x4000 + (i * 4) as u64) == (0xC0DE_0000 | i));
    println!(
        "  copy VRAM(VA {vram_va:#x} = phys {vram_phys:#x}) -> sys-dst(VA {dst_va:#x}) done={done} latency={lat:?}: bytes {}",
        if d2s_ok { "MATCH" } else { "MISMATCH" }
    );
    println!(
        "  => {}",
        if done && d2s_ok {
            "PROVEN: the physical GPU read the reserved VRAM through the guest-built page table (CE VRAM->sysmem copy verified)"
        } else {
            "copy did not fully verify"
        }
    );
    println!(
        "  note: a CE *write into* the reservation (sysmem->VRAM) via a 4 KiB PTE over the 2 MiB \
         contiguous allocation also stalled and is left out; the CPU (BAR1) writes the reserved VRAM. \
         See host-exec.md."
    );
}

/// Append a single-line virtual-addressed CE copy of `nbytes` from src_va to dst_va. The CE
/// resolves both VAs through the channel's VA space (our page table in VRAM), clc5b5.h.
fn push_ce_copy_virt(pb: &mut Vec<u32>, src_va: u64, dst_va: u64, nbytes: u32) {
    pb.push(inc_hdr(0x400, 2, 0)); // OFFSET_IN_UPPER, OFFSET_IN_LOWER
    pb.push((src_va >> 32) as u32 & 0x1FFFF);
    pb.push(src_va as u32);
    pb.push(inc_hdr(0x408, 2, 0)); // OFFSET_OUT_UPPER, OFFSET_OUT_LOWER
    pb.push((dst_va >> 32) as u32 & 0x1FFFF);
    pb.push(dst_va as u32);
    pb.push(inc_hdr(0x418, 1, 0)); // LINE_LENGTH_IN
    pb.push(nbytes);
    pb.push(inc_hdr(0x41c, 1, 0)); // LINE_COUNT
    pb.push(1);
    // LAUNCH_DMA: NON_PIPELINED(2) | FLUSH_ENABLE_TRUE(1<<2) | SRC/DST PITCH (1<<7|1<<8)
    //   | SRC_TYPE VIRTUAL(0) | DST_TYPE VIRTUAL(0) | MULTI_LINE FALSE
    pb.push(inc_hdr(0x300, 1, 0));
    pb.push(0x2 | (1 << 2) | (1 << 7) | (1 << 8));
}

/// Append a WFI (NVC46F_WFI, scope ALL): wait for the engine to be idle, ordering a
/// following PBDMA semaphore release after the CE copy completes.
fn push_wfi(pb: &mut Vec<u32>) {
    pb.push(inc_hdr(0x78, 1, 0));
    pb.push(1); // SCOPE_ALL
}

/// Stage E: NV01_EVENT (OS event fd) for the channel's non-stall notifier; run a
/// pushbuffer NON_STALL_INTERRUPT and wait on the fd.
fn stage_e(rm: &mut Rm, ctx: &mut Ctx) {
    println!("\n=== Stage E: non-stall completion via NV01_EVENT (OS event fd) ===");
    // eventfd for the OS event.
    // SAFETY: create a nonblocking eventfd.
    let efd = unsafe { eventfd(0, EFD_NONBLOCK) };
    assert!(efd >= 0, "eventfd");

    // NV_ESC_ALLOC_OS_EVENT binds the fd to the client at the OS layer.
    let mut aoe = AllocOsEvent {
        h_client: rm.client,
        h_device: rm.subdevice,
        fd: efd as u32,
        status: 0,
    };
    rm.esc(NV_ESC_ALLOC_OS_EVENT, &mut aoe);
    println!("  NV_ESC_ALLOC_OS_EVENT fd={efd} status={:#x}", aoe.status);

    // NV01_EVENT bound to the subdevice, non-stall notifier for HOST/FIFO_EVENT_MTHD (35):
    // the PBDMA NON_STALL_INTERRUPT method raises the HOST engine non-stall interrupt
    // (eventGetEngineTypeFromSubNotifyIndex: FIFO_EVENT_MTHD -> RM_ENGINE_TYPE_HOST), so the
    // event must sit on notifier 35, not on a CE notifier. NV01_EVENT_NONSTALL_INTR = 0x08000000.
    let ev = rm.next;
    rm.next += 1;
    let mut ep = EventAllocParams {
        h_parent_client: rm.client,
        h_src_resource: rm.subdevice,
        h_class: NV01_EVENT_OS_EVENT,
        notify_index: 35 | 0x0800_0000,
        data: efd as u64,
    };
    let st = rm.alloc(
        rm.subdevice,
        ev,
        NV01_EVENT_OS_EVENT,
        &mut ep as *mut _ as *mut u8,
        "os-event",
    );
    println!(
        "  RM_ALLOC NV01_EVENT_OS_EVENT (HOST/FIFO non-stall) status={st:#x} notifyIndex={:#x}",
        ep.notify_index
    );
    if st != 0 {
        println!("  event alloc refused; Stage E cannot proceed.");
        unsafe { close(efd) };
        return;
    }

    // Arm the event: NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION, action REPEAT (2) for CE2.
    let mut sn = EventSetNotification {
        event: 35, // NV2080_NOTIFIERS_FIFO_EVENT_MTHD (HOST)
        action: 2, // REPEAT
        b_notify_state: 1,
        ..Default::default()
    };
    let stn = rm.control_on(rm.subdevice, CMD_EVENT_SET_NOTIFICATION, unsafe {
        std::slice::from_raw_parts_mut(
            &mut sn as *mut _ as *mut u8,
            std::mem::size_of::<EventSetNotification>(),
        )
    });
    println!("  EVENT_SET_NOTIFICATION (HOST/FIFO, REPEAT) status={stn:#x}");

    // Submit a pushbuffer with a semaphore release then a NON_STALL_INTERRUPT method.
    let sysbuf = ctx.sysbuf.as_ref().unwrap();
    let userd = ctx.userd.as_ref().unwrap();
    let db = ctx.doorbell.as_ref().unwrap();
    let sem_va = ctx.va_base + 0x2000;
    let pb_va = ctx.va_base + 0x1000;
    let mut pb: Vec<u32> = Vec::new();
    push_host_sem_release(&mut pb, sem_va, 7); // distinct payload
    pb.push(inc_hdr(0x20, 1, 0)); // NON_STALL_INTERRUPT, handle 0
    pb.push(0);
    let (done, _lat) = submit_and_poll(
        sysbuf,
        userd,
        db,
        ctx.token,
        &mut ctx.gp_slot,
        &pb,
        pb_va,
        7,
    );
    println!("  submitted NON_STALL_INTERRUPT (semaphore released={done}); waiting on the OS event fd (2 s)");

    let mut fds = PollFd {
        fd: efd,
        events: POLLIN,
        revents: 0,
    };
    // SAFETY: single pollfd.
    let pr = unsafe { poll(&mut fds as *mut PollFd, 1, 2000) };
    if pr > 0 && (fds.revents & POLLIN) != 0 {
        // drain event data
        let mut val = [0u8; 8];
        // SAFETY: read the eventfd counter.
        let _ = unsafe { read(efd, val.as_mut_ptr(), 8) };
        // read the RM event record
        let mut uev = NvUnixEvent::default();
        let mut ged = NvOs41 {
            p_event: &mut uev as *mut _ as u64,
            more_events: 0,
            status: 0,
        };
        rm.esc(NV_ESC_RM_GET_EVENT_DATA, &mut ged);
        println!(
            "  OS event fired: poll={pr} eventfd counter drained; GET_EVENT_DATA status={} hObject={:#x} notifyIndex={:#x} moreEvents={}",
            ged.status, uev.h_object, uev.notify_index, ged.more_events
        );
        println!("  => the host non-stall interrupt reached this process as an fd wakeup (what the device forwards to the guest as MSI)");
    } else {
        println!("  poll returned {pr} revents={:#x}: no fd wakeup arrived (semaphore still checked below)", fds.revents);
    }
    println!(
        "  semaphore after NON_STALL_INTERRUPT push = {:#x}",
        sysbuf.rd32(0x2000)
    );
    unsafe { close(efd) };
}

/// Stage F: a GR channel with TURING_COMPUTE_A; whether PROMOTE_CTX is accepted for
/// buffers we allocated in our VRAM reservation. Acceptance and channel state is the finding.
fn stage_f(rm: &mut Rm, ctx: &mut Ctx) {
    println!("\n=== Stage F: GR channel + TURING_COMPUTE_A; PROMOTE_CTX acceptance ===");
    // A second VA space (externally owned) + a GR channel.
    let vas = rm.next;
    rm.next += 1;
    let mut vp = VaspaceParams {
        index: 0,
        flags: VAS_FLAGS_IS_EXTERNALLY_OWNED,
        va_size: 1u64 << 40,
        big_page_size: 64 * 1024,
        ..Default::default()
    };
    let st = rm.alloc(
        rm.device,
        vas,
        FERMI_VASPACE_A,
        &mut vp as *mut _ as *mut u8,
        "vaspace-gr",
    );
    println!("  RM_ALLOC FERMI_VASPACE_A (GR) status={st:#x}");
    if st != 0 {
        return;
    }
    // point it at the same PD3 (a distinct GR channel would normally use its own tree;
    // here we only test PROMOTE_CTX acceptance, no launch).
    let mut sp = SetPageDir {
        phys_address: ctx.resv_phys + ctx.pd3_off,
        num_entries: 1,
        flags: SETPD_APERTURE_VIDMEM | SETPD_ALL_CHANNELS_TRUE,
        h_vaspace: vas,
        sub_device_id: 1,
        ..Default::default()
    };
    admin_on();
    let st = rm.control_on(rm.device, CMD_DMA_SET_PAGE_DIRECTORY, unsafe {
        std::slice::from_raw_parts_mut(
            &mut sp as *mut _ as *mut u8,
            std::mem::size_of::<SetPageDir>(),
        )
    });
    admin_off();
    println!("  DMA_SET_PAGE_DIRECTORY (GR VAS) status={st:#x}");

    let userd = ctx.userd.as_ref().unwrap();
    let chan = rm.next;
    rm.next += 1;
    let mut cp = ChannelParams {
        gp_fifo_offset: ctx.va_base,
        gp_fifo_entries: 32,
        h_vaspace: vas,
        engine_type: 1, // NV2080_ENGINE_TYPE_GRAPHICS
        ..Default::default()
    };
    cp.h_userd_memory[0] = userd.handle;
    let st = rm.alloc(
        rm.device,
        chan,
        TURING_CHANNEL_GPFIFO_A,
        &mut cp as *mut _ as *mut u8,
        "gr-channel",
    );
    println!("  RM_ALLOC GR TURING_CHANNEL_GPFIFO_A status={st:#x}");
    if st != 0 {
        return;
    }
    let compute = rm.next;
    rm.next += 1;
    let st = rm.alloc(
        chan,
        compute,
        TURING_COMPUTE_A,
        std::ptr::null_mut(),
        "compute",
    );
    println!("  RM_ALLOC TURING_COMPUTE_A on GR channel status={st:#x}");
    if st != 0 {
        return;
    }

    // PROMOTE_CTX: MAIN context buffer allocated in our VRAM reservation.
    let ctx_phys = ctx.resv_phys + 0x80_0000; // a slice of the reservation, past the copy area
    let ctx_va = ctx.va_base + 0x10_0000; // (not mapped; PROMOTE_CTX with physAddr)
    let mut pc = PromoteCtx {
        engine_type: 1,
        h_client: rm.client,
        h_chan_client: rm.client,
        h_object: chan,
        entry_count: 1,
        ..Default::default()
    };
    pc.entries[0] = PromoteEntry {
        gpu_phys_addr: ctx_phys,
        gpu_virt_addr: ctx_va,
        size: 0x10_0000,
        phys_attr: 0,
        buffer_id: 0, // MAIN
        b_initialize: 1,
        b_nonmapped: 0,
    };
    admin_on();
    let st = rm.control_on(rm.subdevice, CMD_GPU_PROMOTE_CTX, unsafe {
        std::slice::from_raw_parts_mut(
            &mut pc as *mut _ as *mut u8,
            std::mem::size_of::<PromoteCtx>(),
        )
    });
    admin_off();
    println!(
        "  GPU_PROMOTE_CTX(admin) MAIN buffer phys={ctx_phys:#x} (in reservation) va={ctx_va:#x} status={st:#x}"
    );
    println!(
        "  => PROMOTE_CTX finding: {}",
        match st {
            0 => "ACCEPTED - host RM promotes a guest-owned VRAM context buffer",
            _ => "REFUSED - see status; this is the code the device path must handle",
        }
    );
}

fn run(max_stage: char) {
    drop_to_user();
    let mut rm = Rm::open();
    println!(
        "RM client={:#x} device={:#x} subdevice={:#x} (uid {})",
        rm.client,
        rm.device,
        rm.subdevice,
        unsafe { getuid() }
    );
    let mut ctx = Ctx {
        resv_handle: 0,
        resv_phys: 0,
        bar1: Vec::new(),
        pd3_off: 0,
        sysbuf: None,
        userd: None,
        va_base: 0,
        vas: 0,
        channel: 0,
        usermode: 0,
        doorbell: None,
        token: 0,
        ce_obj: 0,
        gp_slot: 0,
    };

    let stages: &[(char, fn(&mut Rm, &mut Ctx))] = &[
        ('A', stage_a),
        ('B', stage_b),
        ('C', stage_c),
        ('D', stage_d),
        ('E', stage_e),
        ('F', stage_f),
    ];
    for &(id, f) in stages {
        if id > max_stage {
            break;
        }
        f(&mut rm, &mut ctx);
        if let Some(_l) = rm.xid_check(&format!("after Stage {id}")) {
            println!("STOPPING at Stage {id}: Xid / NVRM error seen. Tearing down.");
            break;
        }
    }
    let _ = ctx.resv_handle;
    // SAFETY: single-threaded.
    println!("\nprivileged windows opened: {}", unsafe { ADMIN_WINDOWS });
    println!("=== Stage G: teardown (Drop frees the whole client tree) ===");
    drop(ctx); // free windows/doorbell/sysbuf first
    drop(rm);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let go = args.iter().any(|a| a == "--go");
    let mut max_stage = 'D';
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--max-stage" {
            i += 1;
            max_stage = args
                .get(i)
                .and_then(|s| s.chars().next())
                .unwrap_or('D')
                .to_ascii_uppercase();
        }
        i += 1;
    }
    if args.get(1).map(|s| s.as_str()) != Some("run") || !go {
        eprintln!(
            "usage: sudo -n {} run --go [--max-stage A|B|C|D|E|F]",
            args[0]
        );
        eprintln!("  stages: A reserve+map VRAM, B pin sysmem+build page table, C channel+SET_PAGE_DIRECTORY,");
        eprintln!("          D doorbell semaphore + CE copy, E non-stall OS event, F GR PROMOTE_CTX. Default max D.");
        std::process::exit(2);
    }
    run(max_stage);
}

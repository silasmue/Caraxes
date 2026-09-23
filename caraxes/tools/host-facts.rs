// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Host facts H2-H7 (docs/caraxes/cuda-path.md §7.2) and the VRAM reservation
//! experiment of decision D11 (docs/caraxes/host-facts.md), as an unprivileged RM client.
//!
//! Builds with rustc alone, no crates, like fifo-host-query.rs:
//!
//!     rustc -O --edition 2021 caraxes/tools/host-facts.rs -o target/host-facts
//!     target/host-facts facts            # read-only controls
//!     target/host-facts reserve --go     # allocation experiment (VRAM on the host card)
//!
//! Only RM escapes are issued: RM_ALLOC (root client, device, subdevice and, in
//! `reserve`, NV01_MEMORY_LOCAL_USER), RM_CONTROL (public controls whose flags in
//! src/nvidia/generated/g_subdevice_nvoc.c carry NON_PRIVILEGED 0x8 and not
//! PRIVILEGED 0x4), RM_FREE. No register access, no mapping.
//!
//! Safety of `reserve`: before every allocation the host's free VRAM is read twice
//! (nvidia-smi -q -d MEMORY and FB_GET_INFO_V2 HEAP_FREE); a request larger than the
//! smaller of the two minus 1 GiB is skipped. Each allocation is freed right after
//! it is measured, the free VRAM must come back before the next step, and the first
//! unexpected result stops the run. Every RM object belongs to the one client, which
//! `Drop` frees; if the process dies, closing /dev/nvidiactl frees the client and all
//! its memory in the kernel (RM client teardown on file close), so nothing can stay
//! allocated past exit.
//!
//! Layouts and ids are from vendor/open-gpu-kernel-modules (610.57.04),
//! `sdk/` = src/common/sdk/nvidia/inc:
//!   sdk/nvos.h:1600-1634 (NV_MEMORY_ALLOCATION_PARAMS), :1063-1083 (ATTR fields),
//!     :1266-1268 (ATTR2 PAGE_SIZE_HUGE), :1441 (FIXED_ADDRESS_ALLOCATE)
//!   sdk/class/cl0040.h:34 (NV01_MEMORY_LOCAL_USER)
//!   sdk/ctrl/ctrl2080/ctrl2080fb.h:318-396 (FB_INFO indices), :489-496 (FB_GET_INFO_V2),
//!     FB_GET_FB_REGION_INFO 0x20801320
//!   sdk/ctrl/ctrl2080/ctrl2080bus.h:612-648 (BUS_GET_PCI_BAR_INFO)
//!   sdk/ctrl/ctrl2080/ctrl2080gr.h:1448-1484, 1816-1823, 1896-1903, 1971-1979
//!   sdk/ctrl/ctrl2080/ctrl2080gpu.h:2209-2221 (GPU_GET_PES_INFO)
//!   sdk/ctrl/ctrl2080/ctrl2080ce.h:91-104, 339-342 (CE_GET_ALL_CAPS)
//!   sdk/ctrl/ctrl2080/ctrl2080mc.h:257-366 (MC_GET_* intr controls)
//! The allocation reply's `offset` is the physical FB address of the first page
//! (src/nvidia/src/kernel/mem_mgr/video_mem.c:836-848, :1102); no privileged control
//! is needed to learn it.

use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::process::Command;
use std::time::{Duration, Instant};

extern "C" {
    fn ioctl(fd: i32, request: u64, ...) -> i32;
}

const NV_ESC_RM_FREE: u32 = 0x29;
const NV_ESC_RM_CONTROL: u32 = 0x2A;
const NV_ESC_RM_ALLOC: u32 = 0x2B;

const NV01_ROOT_CLIENT: u32 = 0x41;
const NV01_MEMORY_LOCAL_USER: u32 = 0x40;
const NV01_DEVICE_0: u32 = 0x80;
const NV20_SUBDEVICE_0: u32 = 0x2080;

const CMD_FB_GET_INFO_V2: u32 = 0x2080_1303;
const CMD_FB_GET_FB_REGION_INFO: u32 = 0x2080_1320;
const CMD_BUS_GET_PCI_BAR_INFO: u32 = 0x2080_1803;
const CMD_GR_GET_GPC_MASK: u32 = 0x2080_122A;
const CMD_GR_GET_TPC_MASK: u32 = 0x2080_122B;
const CMD_GR_GET_NUM_TPCS_FOR_GPC: u32 = 0x2080_1234;
const CMD_GR_GET_ZCULL_MASK: u32 = 0x2080_1237;
const CMD_GR_GET_GFX_GPC_AND_TPC_INFO: u32 = 0x2080_1239;
const CMD_GPU_GET_PES_INFO: u32 = 0x2080_0168;
const CMD_CE_GET_ALL_CAPS: u32 = 0x2080_2A0A;
const CMD_MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS: u32 = 0x2080_170D;
const CMD_MC_GET_STATIC_INTR_TABLE: u32 = 0x2080_170E;
const CMD_MC_GET_INTR_CATEGORY_SUBTREE_MAP: u32 = 0x2080_170F;

// FB_INFO indices (ctrl2080fb.h:318-387).
const FBI_BAR1_SIZE: u32 = 0x05;
const FBI_RAM_SIZE: u32 = 0x07;
const FBI_TOTAL_RAM_SIZE: u32 = 0x08;
const FBI_HEAP_SIZE: u32 = 0x09;
const FBI_MAPPABLE_HEAP_SIZE: u32 = 0x0A;
const FBI_FB_TAX_SIZE_KB: u32 = 0x10;
const FBI_HEAP_BASE_KB: u32 = 0x11;
const FBI_LARGEST_FREE_REGION_SIZE_KB: u32 = 0x12;
const FBI_LARGEST_FREE_REGION_BASE_KB: u32 = 0x13;
const FBI_HEAP_FREE: u32 = 0x16;
const FBI_BAR1_AVAIL_SIZE: u32 = 0x1D;
const FBI_HEAP_START: u32 = 0x1E;
const FBI_BAR1_MAX_CONTIGUOUS_AVAIL_SIZE: u32 = 0x1F;
const FBI_USABLE_RAM_SIZE: u32 = 0x20;
const FBI_HEAP_OFFLINE_SIZE: u32 = 0x27;
const FBI_SUSPEND_RESUME_RSVD_SIZE: u32 = 0x29;

const NV_OK: u32 = 0;
const NV_ERR_NO_MEMORY: u32 = 0x51;

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;
const HUGE: u64 = 2 * MIB;
/// Headroom left to the host (decision D11: the host desktop must never run out).
const HOST_HEADROOM: u64 = GIB;

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

/// NV_MEMORY_ALLOCATION_PARAMS, nvos.h:1600-1634.
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

const _: () = {
    assert!(std::mem::size_of::<NvOs00>() == 16);
    assert!(std::mem::size_of::<NvOs64>() == 48);
    assert!(std::mem::size_of::<NvOs54>() == 32);
    assert!(std::mem::size_of::<Nv0080Alloc>() == 56);
    assert!(std::mem::size_of::<MemAlloc>() == 128);
    assert!(std::mem::offset_of!(MemAlloc, range_lo) == 48);
    assert!(std::mem::offset_of!(MemAlloc, offset) == 80);
    assert!(std::mem::offset_of!(MemAlloc, ctag_offset) == 104);
};

// NVOS32 attribute fields (nvos.h).
const ATTR_PAGE_SIZE_HUGE: u32 = 3 << 23; // 24:23
const ATTR_LOCATION_VIDMEM: u32 = 0 << 25; // 26:25
const ATTR_PHYSICALITY_CONTIGUOUS: u32 = 2 << 27; // 28:27
const ATTR2_PAGE_SIZE_HUGE_2MB: u32 = 1 << 20; // 21:20
const ALLOC_FLAGS_FIXED_ADDRESS_ALLOCATE: u32 = 0x10;
const OWNER_TAG: u32 = u32::from_le_bytes(*b"cara");

fn iowr(nr: u32, size: usize) -> u64 {
    ((3u64) << 30) | ((size as u64) << 16) | ((b'F' as u64) << 8) | nr as u64
}

struct Rm {
    ctl: std::fs::File,
    client: u32,
    device: u32,
    subdevice: u32,
    /// Live memory handles; freed by `free_mem` or, at the latest, in `Drop`.
    live: Vec<u32>,
    next: u32,
}

impl Rm {
    fn esc<T>(&self, nr: u32, arg: &mut T) {
        // SAFETY: T is one of the NVOS structs above, sized as the escape expects.
        let r = unsafe { ioctl(self.ctl.as_raw_fd(), iowr(nr, std::mem::size_of::<T>()), arg as *mut T) };
        if r < 0 {
            panic!("ioctl nr {nr:#x}: {}", std::io::Error::last_os_error());
        }
    }

    fn alloc_raw(&self, parent: u32, handle: u32, class: u32, params: *mut u8) -> u32 {
        let mut p = NvOs64 {
            h_root: self.client,
            h_object_parent: parent,
            h_object_new: handle,
            h_class: class,
            p_alloc_parms: params as u64,
            ..Default::default()
        };
        self.esc(NV_ESC_RM_ALLOC, &mut p);
        p.status
    }

    fn control(&self, cmd: u32, buf: &mut [u8]) -> u32 {
        let mut p = NvOs54 {
            h_client: self.client,
            h_object: self.subdevice,
            cmd,
            params: buf.as_mut_ptr() as u64,
            params_size: buf.len() as u32,
            ..Default::default()
        };
        self.esc(NV_ESC_RM_CONTROL, &mut p);
        p.status
    }

    fn open() -> (Rm, std::fs::File) {
        let ctl = OpenOptions::new().read(true).write(true).open("/dev/nvidiactl").expect("open /dev/nvidiactl");
        // RM_ALLOC of NV01_DEVICE_0 needs the GPU node open in this process.
        let gpu = OpenOptions::new().read(true).write(true).open("/dev/nvidia0").expect("open /dev/nvidia0");
        let mut rm = Rm { ctl, client: 0, device: 0xCAF1_0001, subdevice: 0xCAF1_0002, live: Vec::new(), next: 0xCAF1_0100 };
        let mut root = NvOs64 { h_class: NV01_ROOT_CLIENT, ..Default::default() };
        rm.esc(NV_ESC_RM_ALLOC, &mut root);
        assert_eq!(root.status, 0, "root client");
        rm.client = root.h_object_new;
        let mut dp = Nv0080Alloc::default();
        let st = rm.alloc_raw(rm.client, rm.device, NV01_DEVICE_0, &mut dp as *mut _ as *mut u8);
        assert_eq!(st, 0, "device");
        let mut sp = 0u32; // NV2080_ALLOC_PARAMETERS.subDeviceId = 0
        let st = rm.alloc_raw(rm.device, rm.subdevice, NV20_SUBDEVICE_0, &mut sp as *mut _ as *mut u8);
        assert_eq!(st, 0, "subdevice");
        (rm, gpu)
    }

    fn free_obj(&self, parent: u32, h: u32) -> u32 {
        let mut p = NvOs00 { h_root: self.client, h_object_parent: parent, h_object_old: h, status: 0 };
        self.esc(NV_ESC_RM_FREE, &mut p);
        p.status
    }

    fn free_mem(&mut self, h: u32) {
        let st = self.free_obj(self.device, h);
        self.live.retain(|&x| x != h);
        if st != NV_OK {
            panic!("RM_FREE of memory {h:#x}: status {st:#x}");
        }
    }
}

impl Drop for Rm {
    fn drop(&mut self) {
        for h in std::mem::take(&mut self.live) {
            let st = self.free_obj(self.device, h);
            eprintln!("drop: freed leftover memory {h:#x} status={st:#x}");
        }
        let mut p = NvOs00 { h_root: self.client, h_object_parent: self.client, h_object_old: self.client, status: 0 };
        self.esc(NV_ESC_RM_FREE, &mut p);
    }
}

fn rd(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn rd64(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}
fn wr(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// FB_GET_INFO_V2: fbInfoListSize @0, {index,data}[128] @4.
fn fb_info(rm: &Rm, idx: &[u32]) -> (u32, Vec<u32>) {
    let mut b = vec![0u8; 4 + 8 * 128];
    wr(&mut b, 0, idx.len() as u32);
    for (i, &x) in idx.iter().enumerate() {
        wr(&mut b, 4 + 8 * i, x);
    }
    let st = rm.control(CMD_FB_GET_INFO_V2, &mut b);
    (st, (0..idx.len()).map(|i| rd(&b, 8 + 8 * i)).collect())
}

fn fb_name(i: u32) -> &'static str {
    match i {
        FBI_BAR1_SIZE => "BAR1_SIZE(KiB)",
        FBI_RAM_SIZE => "RAM_SIZE(KiB)",
        FBI_TOTAL_RAM_SIZE => "TOTAL_RAM_SIZE(KiB)",
        FBI_HEAP_SIZE => "HEAP_SIZE(KiB)",
        FBI_MAPPABLE_HEAP_SIZE => "MAPPABLE_HEAP_SIZE(KiB)",
        FBI_FB_TAX_SIZE_KB => "FB_TAX_SIZE_KB",
        FBI_HEAP_BASE_KB => "HEAP_BASE_KB",
        FBI_LARGEST_FREE_REGION_SIZE_KB => "LARGEST_FREE_REGION_SIZE_KB",
        FBI_LARGEST_FREE_REGION_BASE_KB => "LARGEST_FREE_REGION_BASE_KB",
        FBI_HEAP_FREE => "HEAP_FREE(KiB)",
        FBI_BAR1_AVAIL_SIZE => "BAR1_AVAIL_SIZE(KiB)",
        FBI_HEAP_START => "HEAP_START(KiB)",
        FBI_BAR1_MAX_CONTIGUOUS_AVAIL_SIZE => "BAR1_MAX_CONTIGUOUS_AVAIL_SIZE(KiB)",
        FBI_USABLE_RAM_SIZE => "USABLE_RAM_SIZE(KiB)",
        FBI_HEAP_OFFLINE_SIZE => "HEAP_OFFLINE_SIZE(KiB)",
        FBI_SUSPEND_RESUME_RSVD_SIZE => "SUSPEND_RESUME_RSVD_SIZE(bytes)",
        _ => "?",
    }
}

const FB_ALL: [u32; 16] = [
    FBI_BAR1_SIZE,
    FBI_RAM_SIZE,
    FBI_TOTAL_RAM_SIZE,
    FBI_HEAP_SIZE,
    FBI_MAPPABLE_HEAP_SIZE,
    FBI_FB_TAX_SIZE_KB,
    FBI_HEAP_BASE_KB,
    FBI_LARGEST_FREE_REGION_SIZE_KB,
    FBI_LARGEST_FREE_REGION_BASE_KB,
    FBI_HEAP_FREE,
    FBI_BAR1_AVAIL_SIZE,
    FBI_HEAP_START,
    FBI_BAR1_MAX_CONTIGUOUS_AVAIL_SIZE,
    FBI_USABLE_RAM_SIZE,
    FBI_HEAP_OFFLINE_SIZE,
    FBI_SUSPEND_RESUME_RSVD_SIZE,
];

/// One FB region: base, limit, reserved, performance, compressed, iso, protected, tag.
struct Region {
    base: u64,
    limit: u64,
    reserved: u64,
    performance: u32,
    compressed: u8,
    iso: u8,
    protected: u8,
    tag: u32,
}

/// FB_GET_FB_REGION_INFO: numFBRegions @0, fbRegion[16] @8, 56 bytes each
/// (base, limit, reserved u64; performance u32 @24; bools @28..30; blackList[18] @31;
/// regionTag @52).
fn fb_regions(rm: &Rm) -> (u32, Vec<Region>) {
    let mut b = vec![0u8; 8 + 56 * 16];
    let st = rm.control(CMD_FB_GET_FB_REGION_INFO, &mut b);
    let n = (rd(&b, 0) as usize).min(16);
    let v = (0..n)
        .map(|i| {
            let o = 8 + 56 * i;
            Region {
                base: rd64(&b, o),
                limit: rd64(&b, o + 8),
                reserved: rd64(&b, o + 16),
                performance: rd(&b, o + 24),
                compressed: b[o + 28],
                iso: b[o + 29],
                protected: b[o + 30],
                tag: rd(&b, o + 52),
            }
        })
        .collect();
    (st, v)
}

fn facts() {
    let (rm, _gpu) = Rm::open();
    println!("client={:#x} device={:#x} subdevice={:#x}", rm.client, rm.device, rm.subdevice);

    // H2: BUS_GET_PCI_BAR_INFO: pciBarCount @0, {flags,barSize(MiB),barSizeBytes,barOffset}[8] @8.
    let mut b = vec![0u8; 8 + 24 * 8];
    let st = rm.control(CMD_BUS_GET_PCI_BAR_INFO, &mut b);
    let n = rd(&b, 0) as usize;
    println!("H2 BUS_GET_PCI_BAR_INFO 0x20801803 status={st:#x} pciBarCount={n}");
    for i in 0..n.min(8) {
        let o = 8 + 24 * i;
        println!(
            "H2   bar[{i}] flags={:#x} barSize={} MiB barSizeBytes={:#x} barOffset={:#x}",
            rd(&b, o),
            rd(&b, o + 4),
            rd64(&b, o + 8),
            rd64(&b, o + 16)
        );
    }

    // H2/H3: FB_GET_INFO_V2.
    let (st, v) = fb_info(&rm, &FB_ALL);
    println!("H3 FB_GET_INFO_V2 0x20801303 status={st:#x}");
    for (i, &x) in FB_ALL.iter().enumerate() {
        println!("H3   index {x:#04x} {} = {} ({:#x})", fb_name(x), v[i], v[i]);
    }

    // H4: FB_GET_FB_REGION_INFO.
    let (st, regs) = fb_regions(&rm);
    println!("H4 FB_GET_FB_REGION_INFO 0x20801320 status={st:#x} numFBRegions={}", regs.len());
    for (i, r) in regs.iter().enumerate() {
        println!(
            "H4   region[{i}] base={:#x} limit={:#x} ({} MiB) reserved={:#x} performance={} compressed={} iso={} protected={} tag={}",
            r.base,
            r.limit,
            (r.limit + 1 - r.base) / MIB,
            r.reserved,
            r.performance,
            r.compressed,
            r.iso,
            r.protected,
            r.tag
        );
    }

    // H5: GR masks. grRouteInfo {flags u32, route u64} = 16 bytes, zero = TYPE_NONE.
    let mut b = vec![0u8; 24];
    let st = rm.control(CMD_GR_GET_GPC_MASK, &mut b);
    let gpc_mask = rd(&b, 16);
    println!("H5 GR_GET_GPC_MASK 0x2080122a status={st:#x} gpcMask={gpc_mask:#x}");
    let mut b = vec![0u8; 24];
    let st = rm.control(CMD_GR_GET_GFX_GPC_AND_TPC_INFO, &mut b);
    println!("H5 GR_GET_GFX_GPC_AND_TPC_INFO 0x20801239 status={st:#x} physGfxGpcMask={:#x} numGfxTpc={}", rd(&b, 16), rd(&b, 20));
    for g in 0..32u32 {
        if gpc_mask & (1 << g) == 0 {
            continue;
        }
        let mut t = vec![0u8; 24];
        wr(&mut t, 16, g);
        let st1 = rm.control(CMD_GR_GET_TPC_MASK, &mut t);
        let mut n = vec![0u8; 8];
        wr(&mut n, 0, g);
        let st2 = rm.control(CMD_GR_GET_NUM_TPCS_FOR_GPC, &mut n);
        let mut z = vec![0u8; 8];
        wr(&mut z, 0, g);
        let st3 = rm.control(CMD_GR_GET_ZCULL_MASK, &mut z);
        // GPU_GET_PES_INFO: gpcId, numPesInGpc, activePesMask, maxTpcPerGpcCount, tpcToPesMap[10].
        let mut p = vec![0u8; 16 + 4 * 10];
        wr(&mut p, 0, g);
        let st4 = rm.control(CMD_GPU_GET_PES_INFO, &mut p);
        let map: Vec<String> = (0..10).map(|i| rd(&p, 16 + 4 * i).to_string()).collect();
        println!(
            "H5   gpc {g}: TPC_MASK status={st1:#x} tpcMask={:#x}; NUM_TPCS status={st2:#x} numTpcs={}; ZCULL_MASK status={st3:#x} zcullMask={:#x}; PES_INFO status={st4:#x} numPesInGpc={} activePesMask={:#x} maxTpcPerGpcCount={} tpcToPesMap=[{}]",
            rd(&t, 20),
            rd(&n, 4),
            rd(&z, 4),
            rd(&p, 4),
            rd(&p, 8),
            rd(&p, 12),
            map.join(",")
        );
    }

    // H6: CE_GET_ALL_CAPS: capsTbl[64][2] @0, present u64 @128.
    let mut c = vec![0u8; 136];
    let st = rm.control(CMD_CE_GET_ALL_CAPS, &mut c);
    let present = rd64(&c, 128);
    println!("H6 CE_GET_ALL_CAPS 0x20802a0a status={st:#x} present={present:#x}");
    let names0 = ["GRCE", "SHARED", "SYSMEM_READ", "SYSMEM_WRITE", "NVLINK_P2P", "SYSMEM", "P2P", "BL_SIZE_GT_64K"];
    let names1 = ["NONPIPELINED_BL", "PIPELINED_BL", "CC_SECURE", "DECOMP", "CC_WORK_SUBMIT", "bit5", "SCRUB", "bit7"];
    for i in 0..64 {
        if present & (1 << i) != 0 {
            let (c0, c1) = (c[2 * i], c[2 * i + 1]);
            let mut s: Vec<&str> = (0..8).filter(|&k| c0 & (1 << k) != 0).map(|k| names0[k]).collect();
            s.extend((0..8).filter(|&k| c1 & (1 << k) != 0).map(|k| names1[k]));
            println!("H6   ce {i} caps0={c0:#04x} caps1={c1:#04x} [{}]", s.join(","));
        }
    }

    // H7: MC_GET_STATIC_INTR_TABLE: numEntries @0, {type,pmcIntrMask,stall,nonstall}[32] @4.
    let mut m = vec![0u8; 4 + 16 * 32];
    let st = rm.control(CMD_MC_GET_STATIC_INTR_TABLE, &mut m);
    let n = rd(&m, 0) as usize;
    println!("H7 MC_GET_STATIC_INTR_TABLE 0x2080170e status={st:#x} numEntries={n}");
    for i in 0..n.min(32) {
        let o = 4 + 16 * i;
        println!(
            "H7   entry {i}: nv2080IntrType={} pmcIntrMask={:#x} intrVectorStall={:#x} intrVectorNonStall={:#x}",
            rd(&m, o),
            rd(&m, o + 4),
            rd(&m, o + 8),
            rd(&m, o + 12)
        );
    }
    // MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS: numEntries @0, {engineType, vector}[256] @4.
    let mut m = vec![0u8; 4 + 8 * 256];
    let st = rm.control(CMD_MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS, &mut m);
    let n = rd(&m, 0) as usize;
    println!("H7 MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS 0x2080170d status={st:#x} numEntries={n}");
    for i in 0..n.min(256) {
        println!("H7   engine type={:#x} notificationIntrVector={:#x}", rd(&m, 4 + 8 * i), rd(&m, 8 + 8 * i));
    }
    // MC_GET_INTR_CATEGORY_SUBTREE_MAP: subtreeMask u64 [7].
    let mut m = vec![0u8; 8 * 7];
    let st = rm.control(CMD_MC_GET_INTR_CATEGORY_SUBTREE_MAP, &mut m);
    let cats = ["DEFAULT", "ESCHED_DRIVEN_ENGINE", "ESCHED_DRIVEN_ENGINE_NOTIFICATION", "RUNLIST", "RUNLIST_NOTIFICATION", "UVM_OWNED", "UVM_SHARED"];
    println!("H7 MC_GET_INTR_CATEGORY_SUBTREE_MAP 0x2080170f status={st:#x}");
    for (i, c) in cats.iter().enumerate() {
        println!("H7   {c} subtreeMask={:#x}", rd64(&m, 8 * i));
    }
}

/// Free VRAM as nvidia-smi -q -d MEMORY reports it (FB Memory Usage / Free), in MiB.
fn smi_free_mib() -> u64 {
    let t0 = Instant::now();
    let out = Command::new("nvidia-smi").args(["-q", "-d", "MEMORY"]).output().expect("run nvidia-smi");
    if !out.status.success() {
        panic!("nvidia-smi failed: {:?}", out.status);
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let mut in_fb = false;
    for l in s.lines() {
        let t = l.trim();
        if t.starts_with("FB Memory Usage") {
            in_fb = true;
        } else if in_fb && t.starts_with("Free") {
            let v: u64 = t.split(':').nth(1).unwrap().trim().trim_end_matches("MiB").trim().parse().unwrap();
            println!("  nvidia-smi: FB free {v} MiB (answered in {} ms)", t0.elapsed().as_millis());
            return v;
        }
    }
    panic!("nvidia-smi output without FB free");
}

struct Snapshot {
    heap_free: u64,
    smi_free: u64,
    largest: u64,
    largest_base: u64,
}

fn snapshot(rm: &Rm, what: &str) -> Snapshot {
    // LARGEST_FREE_REGION_BASE_KB is an offset inside the PMA region whose base is
    // HEAP_BASE_KB (pmaGetLargestFree, phys_mem_allocator.c:1928-1956;
    // kern_mem_sys_ctrl.c:674-718), so the absolute address is their sum.
    let idx = [FBI_HEAP_FREE, FBI_LARGEST_FREE_REGION_SIZE_KB, FBI_LARGEST_FREE_REGION_BASE_KB, FBI_HEAP_BASE_KB];
    let (st, v) = fb_info(rm, &idx);
    if st != NV_OK {
        panic!("FB_GET_INFO_V2 status {st:#x}");
    }
    println!(
        "  {what}: HEAP_FREE {} MiB, LARGEST_FREE_REGION {} MiB at {:#x} (region offset {:#x} + HEAP_BASE {:#x})",
        v[0] as u64 / 1024,
        v[1] as u64 / 1024,
        (v[3] as u64 + v[2] as u64) * KIB,
        v[2] as u64 * KIB,
        v[3] as u64 * KIB
    );
    let smi = smi_free_mib();
    Snapshot { heap_free: v[0] as u64 * KIB, smi_free: smi * MIB, largest: v[1] as u64 * KIB, largest_base: (v[3] as u64 + v[2] as u64) * KIB }
}

enum Outcome {
    Ok,
    Refused,
    Skipped,
}

/// One allocation step. Returns the outcome; panics (stops) on anything unexpected.
fn step(rm: &mut Rm, label: &str, size: u64, fixed: Option<u64>, results: &mut Vec<String>) -> Outcome {
    println!("STEP {label}: size {} MiB, {}", size / MIB, fixed.map_or("contiguous, any address".to_string(), |o| format!("fixed offset {o:#x}")));
    let before = snapshot(rm, "before");
    let free = before.heap_free.min(before.smi_free);
    if size + HOST_HEADROOM > free {
        println!("  SKIP: {} MiB + 1024 MiB headroom > free {} MiB", size / MIB, free / MIB);
        results.push(format!("RESULT {label} size={}MiB mode={} status=skipped free_before={}MiB", size / MIB, mode(fixed), free / MIB));
        return Outcome::Skipped;
    }
    let mut p = MemAlloc {
        owner: OWNER_TAG,
        type_: 0, // NVOS32_TYPE_IMAGE
        flags: if fixed.is_some() { ALLOC_FLAGS_FIXED_ADDRESS_ALLOCATE } else { 0 },
        attr: ATTR_LOCATION_VIDMEM | ATTR_PHYSICALITY_CONTIGUOUS | ATTR_PAGE_SIZE_HUGE,
        attr2: ATTR2_PAGE_SIZE_HUGE_2MB,
        size,
        offset: fixed.unwrap_or(0),
        ..Default::default()
    };
    let h = rm.next;
    rm.next += 1;
    let t0 = Instant::now();
    let st = rm.alloc_raw(rm.device, h, NV01_MEMORY_LOCAL_USER, &mut p as *mut _ as *mut u8);
    let dt = t0.elapsed();
    if st != NV_OK {
        println!("  RM_ALLOC NV01_MEMORY_LOCAL_USER status={st:#x} after {} ms", dt.as_millis());
        let after = snapshot(rm, "after refusal");
        results.push(format!(
            "RESULT {label} size={}MiB mode={} status={st:#x} addr=- free_before={}MiB free_after={}MiB",
            size / MIB,
            mode(fixed),
            before.smi_free / MIB,
            after.smi_free / MIB
        ));
        if st != NV_ERR_NO_MEMORY {
            panic!("unexpected allocation status {st:#x}; stopping");
        }
        return Outcome::Refused;
    }
    rm.live.push(h);
    println!(
        "  RM_ALLOC ok in {} ms: handle={h:#x} offset(phys)={:#x} size={:#x} limit={:#x} attr={:#x} attr2={:#x} flags={:#x}",
        dt.as_millis(),
        p.offset,
        p.size,
        p.limit,
        p.attr,
        p.attr2,
        p.flags
    );
    let contiguous = (p.attr >> 27) & 3 == 2;
    let during = snapshot(rm, "while held");
    rm.free_mem(h);
    println!("  RM_FREE ok");
    // The freed pages may be scrubbed before PMA reports them free again.
    let deadline = Instant::now() + Duration::from_secs(20);
    let after = loop {
        let s = snapshot(rm, "after free");
        if s.heap_free + 64 * MIB >= before.heap_free || Instant::now() > deadline {
            break s;
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    results.push(format!(
        "RESULT {label} size={}MiB mode={} status=0x0 addr={:#x}..{:#x} contiguous={contiguous} free_before={}MiB free_held={}MiB free_after={}MiB heap_free_before={}MiB heap_free_after={}MiB",
        size / MIB,
        mode(fixed),
        p.offset,
        p.offset + p.size - 1,
        before.smi_free / MIB,
        during.smi_free / MIB,
        after.smi_free / MIB,
        before.heap_free / MIB,
        after.heap_free / MIB
    ));
    if after.heap_free + 64 * MIB < before.heap_free {
        panic!("free VRAM did not come back ({} < {} MiB); stopping", after.heap_free / MIB, before.heap_free / MIB);
    }
    if let Some(o) = fixed {
        if p.offset != o {
            panic!("fixed allocation landed at {:#x}, not {o:#x}; stopping", p.offset);
        }
    }
    if !contiguous {
        panic!("allocation not reported contiguous (attr {:#x}); stopping", p.attr);
    }
    Outcome::Ok
}

fn mode(fixed: Option<u64>) -> String {
    fixed.map_or("contig".to_string(), |o| format!("fixed@{o:#x}"))
}

fn reserve() {
    let (mut rm, _gpu) = Rm::open();
    println!("client={:#x} device={:#x} subdevice={:#x}", rm.client, rm.device, rm.subdevice);
    let (st, regs) = fb_regions(&rm);
    assert_eq!(st, NV_OK, "FB_GET_FB_REGION_INFO");
    for r in &regs {
        println!("region base={:#x} limit={:#x} reserved={:#x} tag={}", r.base, r.limit, r.reserved, r.tag);
    }
    let sizes = [5 * GIB / 2, 3 * GIB, 5 * GIB];
    let mut results = Vec::new();

    // Phase 1: contiguous, any address.
    for &s in &sizes {
        step(&mut rm, &format!("contig-{}MiB", s / MIB), s, None, &mut results);
    }

    // Phase 2: fixed addresses, lowest and highest in the largest free region the host reports.
    for &s in &sizes {
        let snap = snapshot(&rm, "layout");
        let lo = (snap.largest_base + HUGE - 1) & !(HUGE - 1);
        let end = snap.largest_base + snap.largest;
        if end < lo + s {
            println!("STEP fixed-{}MiB: SKIP, largest free region {} MiB is smaller", s / MIB, snap.largest / MIB);
            results.push(format!("RESULT fixed-{}MiB size={}MiB mode=fixed status=skipped largest_free={}MiB", s / MIB, s / MIB, snap.largest / MIB));
            continue;
        }
        let hi = (end - s) & !(HUGE - 1);
        step(&mut rm, &format!("fixed-low-{}MiB", s / MIB), s, Some(lo), &mut results);
        if hi != lo {
            step(&mut rm, &format!("fixed-high-{}MiB", s / MIB), s, Some(hi), &mut results);
        }
    }

    // Phase 3: a fixed range that starts in memory the host holds (the bottom of the
    // usable region, below the largest free region), 2.5 GiB only: shows how RM
    // refuses an occupied range.
    if let Some(r) = regs.iter().find(|r| r.reserved == 0 && r.limit > r.base + 5 * GIB / 2) {
        let lo = (r.base + HUGE - 1) & !(HUGE - 1);
        let snap = snapshot(&rm, "layout");
        if lo < snap.largest_base {
            match step(&mut rm, "fixed-occupied-2560MiB", 5 * GIB / 2, Some(lo), &mut results) {
                Outcome::Ok => println!("  (the bottom range was free after all)"),
                Outcome::Refused | Outcome::Skipped => {}
            }
        } else {
            println!("STEP fixed-occupied: SKIP, region bottom {lo:#x} is inside the largest free region");
        }
    }

    println!();
    for r in &results {
        println!("{r}");
    }
    assert!(rm.live.is_empty(), "memory left allocated");
    drop(rm);
    println!("client freed; nothing allocated");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(|s| s.as_str()).collect::<Vec<_>>().as_slice() {
        ["facts"] => facts(),
        ["reserve", "--go"] => reserve(),
        _ => {
            eprintln!("usage: host-facts facts | host-facts reserve --go");
            std::process::exit(2);
        }
    }
}

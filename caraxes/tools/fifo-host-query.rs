// SPDX-License-Identifier: MIT
//! Read-only engine and FIFO queries on the host card, as an unprivileged RM client.
//!
//! Supplies the host-derived fields of the synthetic TU106 FIFO device info table
//! (docs/caraxes/fifo-device-info.md). Builds with rustc alone, no crates:
//!
//!     rustc -O --edition 2021 caraxes/tools/fifo-host-query.rs -o target/fifo-host-query
//!     target/fifo-host-query
//!
//! Output of the 2026-09-23 run on the RTX 2070 host: docs/caraxes/fifo-device-info.md §5.
//!
//! Only RM controls are issued: RM_ALLOC of NV01_ROOT_CLIENT, NV01_DEVICE_0 and
//! NV20_SUBDEVICE_0, then RM_CONTROL on the subdevice, then RM_FREE of the client.
//! No register access, no mapping. Layouts and ids are from
//! vendor/open-gpu-kernel-modules/src/common/sdk/nvidia/inc (610.57.04):
//!   nvos.h:162-168 (NVOS00), :479-490 (NVOS64), :2230-2239 (NVOS54)
//!   class/cl0080.h:54-64, class/cl2080.h:43-45
//!   ctrl/ctrl2080/ctrl2080gpu.h:778-788, 855-863, 1691-1705, 2932-2940, 2969-2976, 4454-4470
//!   ctrl/ctrl2080/ctrl2080fifo.h:178-194 (FIFO_GET_INFO), 535-576 (DEVICE_INFO_TABLE)
//!   ctrl/ctrl2080/ctrl2080ce.h:171-178, 295, 333-342
//! Ioctl numbers: kernel-open/common/inc/nv-ioctl-numbers.h:33 ('F') and
//! src/nvidia/arch/nvalloc/unix/include/nv_escape.h:33-35 (FREE 0x29, CONTROL 0x2A, ALLOC 0x2B).

use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;

extern "C" {
    fn ioctl(fd: i32, request: u64, ...) -> i32;
}

const NV_ESC_RM_FREE: u32 = 0x29;
const NV_ESC_RM_CONTROL: u32 = 0x2A;
const NV_ESC_RM_ALLOC: u32 = 0x2B;

const NV01_ROOT_CLIENT: u32 = 0x41;
const NV01_DEVICE_0: u32 = 0x80;
const NV20_SUBDEVICE_0: u32 = 0x2080;

const CMD_GPU_GET_ENGINES_V2: u32 = 0x2080_0170;
const CMD_GPU_GET_ENGINE_FAULT_INFO: u32 = 0x2080_0125;
const CMD_GPU_GET_ENGINE_PARTNERLIST: u32 = 0x2080_0147;
const CMD_GPU_GET_ENGINE_RUNLIST_PRI_BASE: u32 = 0x2080_0179;
const CMD_GPU_GET_HW_ENGINE_ID: u32 = 0x2080_017A;
const CMD_GPU_GET_CONSTRUCTED_FALCON_INFO: u32 = 0x2080_01B0;
const CMD_FIFO_GET_INFO: u32 = 0x2080_1109;
const CMD_FIFO_GET_DEVICE_INFO_TABLE: u32 = 0x2080_1112;
const CMD_CE_GET_CE_PCE_MASK: u32 = 0x2080_2A02;
const CMD_CE_GET_FAULT_METHOD_BUFFER_SIZE: u32 = 0x2080_2A08;
const CMD_CE_GET_ALL_CAPS: u32 = 0x2080_2A0A;

/// Partnership class must be a FIFO class (subdevice_ctrl_gpu_kernel.c:1668-1683); clc46f.h:43.
const TURING_CHANNEL_GPFIFO_A: u32 = 0xC46F;

const MAX_ENGINES: usize = 0x54; // NV2080_GPU_MAX_ENGINES_LIST_SIZE, ctrl2080gpu.h:781

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

const _: () = {
    assert!(std::mem::size_of::<NvOs00>() == 16);
    assert!(std::mem::size_of::<NvOs64>() == 48);
    assert!(std::mem::size_of::<NvOs54>() == 32);
    assert!(std::mem::size_of::<Nv0080Alloc>() == 56);
};

fn iowr(nr: u32, size: usize) -> u64 {
    ((3u64) << 30) | ((size as u64) << 16) | ((b'F' as u64) << 8) | nr as u64
}

struct Rm {
    ctl: std::fs::File,
    client: u32,
    subdevice: u32,
}

impl Rm {
    fn esc<T>(&self, nr: u32, arg: &mut T) {
        // SAFETY: T is one of the NVOS structs above, sized as the escape expects.
        let r = unsafe { ioctl(self.ctl.as_raw_fd(), iowr(nr, std::mem::size_of::<T>()), arg as *mut T) };
        if r < 0 {
            panic!("ioctl nr {nr:#x}: {}", std::io::Error::last_os_error());
        }
    }

    fn alloc(&self, parent: u32, handle: u32, class: u32, params: *mut u8) -> u32 {
        let mut p = NvOs64 {
            h_root: self.client,
            h_object_parent: parent,
            h_object_new: handle,
            h_class: class,
            p_alloc_parms: params as u64,
            ..Default::default()
        };
        self.esc(NV_ESC_RM_ALLOC, &mut p);
        if p.status != 0 {
            panic!("RM_ALLOC class {class:#x}: status {:#x}", p.status);
        }
        p.h_object_new
    }

    /// Returns the RM status; `buf` is the params block, updated in place.
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
}

impl Drop for Rm {
    fn drop(&mut self) {
        let mut p = NvOs00 {
            h_root: self.client,
            h_object_parent: self.client,
            h_object_old: self.client,
            status: 0,
        };
        self.esc(NV_ESC_RM_FREE, &mut p);
    }
}

fn rd(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn wr(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// NV2080_ENGINE_TYPE names, class/cl2080_notification.h:283-353.
fn nv2080_name(t: u32) -> String {
    match t {
        0x01 => "GR0".into(),
        0x02..=0x08 => format!("GR{}", t - 0x01),
        0x09..=0x12 => format!("COPY{}", t - 0x09),
        0x13 => "NVDEC0".into(),
        0x14..=0x1a => format!("NVDEC{}", t - 0x13),
        0x1b => "NVENC0".into(),
        0x1c | 0x1d => format!("NVENC{}", t - 0x1b),
        0x3f => "NVENC3".into(),
        0x22 => "SW".into(),
        0x24 => "VIC".into(),
        0x26 => "SEC2".into(),
        0x27 => "HOST".into(),
        0x2b => "NVJPG0".into(),
        0x33 => "OFA0".into(),
        0x34..=0x3d => format!("COPY{}", t - 0x34 + 10),
        _ => format!("type{t:#x}"),
    }
}

fn is_copy(t: u32) -> bool {
    (0x09..=0x12).contains(&t) || (0x34..=0x3d).contains(&t)
}

fn main() {
    let ctl = OpenOptions::new().read(true).write(true).open("/dev/nvidiactl").expect("open /dev/nvidiactl");
    // RM_ALLOC of NV01_DEVICE_0 needs the GPU node open in this process.
    let _gpu = OpenOptions::new().read(true).write(true).open("/dev/nvidia0").expect("open /dev/nvidia0");
    let mut rm = Rm { ctl, client: 0, subdevice: 0 };
    let mut root = NvOs64 { h_class: NV01_ROOT_CLIENT, ..Default::default() };
    rm.esc(NV_ESC_RM_ALLOC, &mut root);
    assert_eq!(root.status, 0, "root client");
    rm.client = root.h_object_new;
    let device = 0xCAF0_0001u32;
    let subdevice = 0xCAF0_0002u32;
    let mut dp = Nv0080Alloc::default();
    rm.alloc(rm.client, device, NV01_DEVICE_0, &mut dp as *mut _ as *mut u8);
    let mut sp = 0u32; // NV2080_ALLOC_PARAMETERS.subDeviceId = 0
    rm.alloc(device, subdevice, NV20_SUBDEVICE_0, &mut sp as *mut _ as *mut u8);
    rm.subdevice = subdevice;

    // GPU_GET_ENGINES_V2: engineCount @0, engineList[0x54] @4.
    let mut b = vec![0u8; 4 + 4 * MAX_ENGINES];
    let st = rm.control(CMD_GPU_GET_ENGINES_V2, &mut b);
    println!("GPU_GET_ENGINES_V2 status={st:#x}");
    let n = rd(&b, 0) as usize;
    let engines: Vec<u32> = (0..n.min(MAX_ENGINES)).map(|i| rd(&b, 4 + 4 * i)).collect();
    println!("engines count={n} list={}", engines.iter().map(|&t| format!("{}({t:#x})", nv2080_name(t))).collect::<Vec<_>>().join(" "));

    for &t in &engines {
        // GPU_GET_ENGINE_FAULT_INFO: engineType @0, mmuFaultId @4, bSubcontextSupported @8.
        let mut f = vec![0u8; 12];
        wr(&mut f, 0, t);
        let st = rm.control(CMD_GPU_GET_ENGINE_FAULT_INFO, &mut f);
        print!("engine {} type={t:#x} fault_info_status={st:#x} mmuFaultId={} subctx={}", nv2080_name(t), rd(&f, 4), f[8]);
        // GPU_GET_ENGINE_PARTNERLIST: engineType, partnershipClassId, runqueue, numPartners, partnerList[32].
        for rq in 0..2u32 {
            let mut p = vec![0u8; 16 + 4 * 32];
            wr(&mut p, 0, t);
            wr(&mut p, 4, TURING_CHANNEL_GPFIFO_A);
            wr(&mut p, 8, rq);
            let st = rm.control(CMD_GPU_GET_ENGINE_PARTNERLIST, &mut p);
            let np = rd(&p, 12) as usize;
            let list: Vec<String> = (0..np.min(32)).map(|i| nv2080_name(rd(&p, 16 + 4 * i))).collect();
            print!(" partners[rq{rq}] status={st:#x} [{}]", list.join(","));
        }
        if is_copy(t) {
            // CE_GET_CE_PCE_MASK: ceEngineType @0, pceMask @4.
            let mut c = vec![0u8; 8];
            wr(&mut c, 0, t);
            let st = rm.control(CMD_CE_GET_CE_PCE_MASK, &mut c);
            print!(" pce_mask_status={st:#x} pceMask={:#x}", rd(&c, 4));
        }
        println!();
    }

    // Membership probe: GET_ENGINE_FAULT_INFO translates RM_ENGINE_TYPE through the host's
    // own FIFO engine table (subdevice_ctrl_gpu_kernel.c:1805-1807), so NV_OK means the
    // engine has a table entry even when it has no client class (absent from ENGINES_V2).
    // Every NV2080_ENGINE_TYPE up to COMP_DECOMP_COPY15 (cl2080_notification.h:283-370)
    // that GPU_GET_ENGINES_V2 did not list; only the answered ones are printed.
    for t in 1u32..=0x4f {
        if engines.contains(&t) {
            continue;
        }
        let mut f = vec![0u8; 12];
        wr(&mut f, 0, t);
        let st = rm.control(CMD_GPU_GET_ENGINE_FAULT_INFO, &mut f);
        if st != 0 {
            continue;
        }
        print!("probe {} type={t:#x} fault_info_status={st:#x} mmuFaultId={}", nv2080_name(t), rd(&f, 4));
        if st == 0 {
            // Runqueue r is accepted only when the entry has more than r PBDMAs
            // (kernel_fifo_gm107.c:975-979), so the status tells numPbdmas.
            for rq in 0..2u32 {
                let mut p = vec![0u8; 16 + 4 * 32];
                wr(&mut p, 0, t);
                wr(&mut p, 4, TURING_CHANNEL_GPFIFO_A);
                wr(&mut p, 8, rq);
                let st = rm.control(CMD_GPU_GET_ENGINE_PARTNERLIST, &mut p);
                print!(" partners[rq{rq}] status={st:#x} n={}", rd(&p, 12));
            }
        }
        println!();
    }

    // CE_GET_ALL_CAPS: capsTbl[64][2] @0, present u64 @128.
    let mut c = vec![0u8; 136];
    let st = rm.control(CMD_CE_GET_ALL_CAPS, &mut c);
    let present = u64::from_le_bytes(c[128..136].try_into().unwrap());
    println!("CE_GET_ALL_CAPS status={st:#x} present={present:#x}");
    for i in 0..64 {
        if present & (1 << i) != 0 {
            println!("ce {i} caps0={:#04x} caps1={:#04x} (caps0 bit0 GRCE, bit1 SHARED; ctrl2080ce.h:91-104)", c[2 * i], c[2 * i + 1]);
        }
    }

    // FIFO_GET_INFO: fifoInfoTblSize @0, {index,data}[256] @4, engineType @2052.
    let idx = [1u32, 2, 3, 5, 7, 10];
    let mut f = vec![0u8; 2056];
    wr(&mut f, 0, idx.len() as u32);
    for (i, &x) in idx.iter().enumerate() {
        wr(&mut f, 4 + 8 * i, x);
    }
    let st = rm.control(CMD_FIFO_GET_INFO, &mut f);
    print!("FIFO_GET_INFO status={st:#x}");
    for (i, &x) in idx.iter().enumerate() {
        print!(" index{x}={}", rd(&f, 8 + 8 * i));
    }
    println!(" (1 MAX_CHANNEL_GROUPS, 2 MAX_CHANNELS_PER_GROUP, 3 MAX_SUBCONTEXT_PER_GROUP, 5 DEFAULT_CHANNEL_TIMESLICE, 7 PER_RUNLIST_CHRAM, 10 MAX_LOWER_SUBCONTEXT)");

    // GPU_GET_CONSTRUCTED_FALCON_INFO: num @0, {engDesc,ctxAttr,ctxBufferSize,addrSpaceList,registerBase}[64] @4.
    let mut f = vec![0u8; 4 + 20 * 64];
    let st = rm.control(CMD_GPU_GET_CONSTRUCTED_FALCON_INFO, &mut f);
    let nf = rd(&f, 0) as usize;
    println!("GPU_GET_CONSTRUCTED_FALCON_INFO status={st:#x} num={nf}");
    for i in 0..nf.min(64) {
        let o = 4 + 20 * i;
        println!(
            "falcon {i} engDesc={:#010x} (class {:#08x} inst {}) ctxAttr={:#x} ctxBufferSize={:#x} addrSpaceList={:#x} registerBase={:#x}",
            rd(&f, o), rd(&f, o) >> 8, rd(&f, o) & 0xff, rd(&f, o + 4), rd(&f, o + 8), rd(&f, o + 12), rd(&f, o + 16)
        );
    }

    // Expected to fail for an unprivileged client or on Turing; recorded for the document.
    let mut h = vec![0u8; 8 * MAX_ENGINES];
    for (i, &t) in engines.iter().enumerate() {
        wr(&mut h, 4 * i, t);
    }
    let st = rm.control(CMD_GPU_GET_HW_ENGINE_ID, &mut h);
    println!("GPU_GET_HW_ENGINE_ID status={st:#x}");
    let mut r = vec![0u8; 12 * MAX_ENGINES];
    for (i, &t) in engines.iter().enumerate() {
        wr(&mut r, 4 * i, t);
    }
    let st = rm.control(CMD_GPU_GET_ENGINE_RUNLIST_PRI_BASE, &mut r);
    println!("GPU_GET_ENGINE_RUNLIST_PRI_BASE status={st:#x}");
    let mut d = vec![0u8; 12 + 32 * 100];
    let st = rm.control(CMD_FIFO_GET_DEVICE_INFO_TABLE, &mut d);
    println!("FIFO_GET_DEVICE_INFO_TABLE status={st:#x} numEntries={}", rd(&d, 4));
    let mut s = vec![0u8; 4];
    let st = rm.control(CMD_CE_GET_FAULT_METHOD_BUFFER_SIZE, &mut s);
    println!("CE_GET_FAULT_METHOD_BUFFER_SIZE status={st:#x} size={:#x}", rd(&s, 0));
    drop(rm);
}

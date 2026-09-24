// SPDX-License-Identifier: MIT
//! Read-only identity queries for the lab capture kit (docs/caraxes/lab-blackwell.md),
//! as an unprivileged RM client: the class list of the device, the architecture and
//! implementation the RM reports (the BOOT_0 equivalent, without a register read), the
//! PCI ids as the RM sees them and the marketing and short name strings.
//!
//! Builds with rustc alone, no crates, like fifo-host-query.rs:
//!
//!     rustc -O --edition 2021 caraxes/tools/lab-rm-query.rs -o target/lab-rm-query
//!     target/lab-rm-query
//!
//! Only RM escapes are issued: RM_ALLOC of NV01_ROOT_CLIENT, NV01_DEVICE_0 and
//! NV20_SUBDEVICE_0, RM_CONTROL of four getters, RM_FREE of the client. No register
//! access, no mapping, nothing is written to the GPU. Layouts are the 610.57.04 ones
//! (src/common/sdk/nvidia/inc): nvos.h (NVOS00/54/64), class/cl0080.h (alloc params),
//! ctrl/ctrl0080/ctrl0080gpu.h:70-80 (GPU_GET_CLASSLIST, unchanged from 570.211.01 to
//! 615.71.09), ctrl/ctrl2080/ctrl2080mc.h:61-70 (MC_GET_ARCH_INFO),
//! ctrl/ctrl2080/ctrl2080bus.h:63-72 (BUS_GET_PCI_INFO), ctrl/ctrl2080/ctrl2080gpu.h:325-367
//! (GPU_GET_NAME_STRING, GPU_GET_SHORT_NAME_STRING). caraxes/tools/lab-abi-check.sh
//! verifies them against the headers of the running driver version.

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

const CMD_GPU_GET_CLASSLIST: u32 = 0x0080_0201;
const CMD_MC_GET_ARCH_INFO: u32 = 0x2080_1701;
const CMD_BUS_GET_PCI_INFO: u32 = 0x2080_1801;
const CMD_GPU_GET_NAME_STRING: u32 = 0x2080_0110;
const CMD_GPU_GET_SHORT_NAME_STRING: u32 = 0x2080_0111;

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
    (3u64 << 30) | ((size as u64) << 16) | ((b'F' as u64) << 8) | nr as u64
}

struct Rm {
    ctl: std::fs::File,
    client: u32,
}

impl Rm {
    fn esc<T>(&self, nr: u32, arg: &mut T) -> Result<(), String> {
        // SAFETY: T is one of the NVOS structs above, sized as the escape expects.
        let r = unsafe { ioctl(self.ctl.as_raw_fd(), iowr(nr, std::mem::size_of::<T>()), arg as *mut T) };
        if r < 0 {
            return Err(format!("ioctl nr {nr:#x}: {}", std::io::Error::last_os_error()));
        }
        Ok(())
    }

    fn alloc(&self, parent: u32, handle: u32, class: u32, params: *mut u8) -> Result<(), String> {
        let mut p = NvOs64 {
            h_root: self.client,
            h_object_parent: parent,
            h_object_new: handle,
            h_class: class,
            p_alloc_parms: params as u64,
            ..Default::default()
        };
        self.esc(NV_ESC_RM_ALLOC, &mut p)?;
        if p.status != 0 {
            return Err(format!("RM_ALLOC class {class:#x}: status {:#x}", p.status));
        }
        Ok(())
    }

    /// Returns the RM status; `buf` is the params block, updated in place.
    fn control(&self, object: u32, cmd: u32, buf: *mut u8, len: usize) -> u32 {
        let mut p = NvOs54 {
            h_client: self.client,
            h_object: object,
            cmd,
            params: buf as u64,
            params_size: len as u32,
            ..Default::default()
        };
        match self.esc(NV_ESC_RM_CONTROL, &mut p) {
            Ok(()) => p.status,
            Err(e) => {
                println!("error: control {cmd:#010x}: {e}");
                0xFFFF_FFFF
            }
        }
    }
}

impl Drop for Rm {
    fn drop(&mut self) {
        if self.client != 0 {
            let mut p = NvOs00 { h_root: self.client, h_object_parent: self.client, h_object_old: self.client, status: 0 };
            let _ = self.esc(NV_ESC_RM_FREE, &mut p);
        }
    }
}

fn rd(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

/// NV2080_CTRL_MC_ARCH_INFO_ARCHITECTURE_* (ctrl2080mc.h:74-86).
fn arch_name(a: u32) -> &'static str {
    match a {
        0x160 => "TU100 (Turing)",
        0x170 => "GA100 (Ampere)",
        0x180 => "GH100 (Hopper)",
        0x190 => "AD100 (Ada)",
        0x1A0 => "GB100 (Blackwell datacenter)",
        0x1B0 => "GB200 (Blackwell GB20x)",
        0x1C0 => "GR100 (Rubin)",
        _ => "unknown to this tool",
    }
}

fn run() -> Result<(), String> {
    let ctl = OpenOptions::new().read(true).write(true).open("/dev/nvidiactl").map_err(|e| format!("open /dev/nvidiactl: {e}"))?;
    // RM_ALLOC of NV01_DEVICE_0 needs the GPU node open in this process.
    let _gpu = OpenOptions::new().read(true).write(true).open("/dev/nvidia0").map_err(|e| format!("open /dev/nvidia0: {e}"))?;
    let mut rm = Rm { ctl, client: 0 };
    let mut root = NvOs64 { h_class: NV01_ROOT_CLIENT, ..Default::default() };
    rm.esc(NV_ESC_RM_ALLOC, &mut root)?;
    if root.status != 0 {
        return Err(format!("root client: status {:#x}", root.status));
    }
    rm.client = root.h_object_new;
    let device = 0xCAF2_0001u32;
    let subdevice = 0xCAF2_0002u32;
    let mut dp = Nv0080Alloc::default();
    rm.alloc(rm.client, device, NV01_DEVICE_0, &mut dp as *mut _ as *mut u8)?;
    let mut sp = 0u32; // NV2080_ALLOC_PARAMETERS.subDeviceId = 0
    rm.alloc(device, subdevice, NV20_SUBDEVICE_0, &mut sp as *mut _ as *mut u8)?;

    // MC_GET_ARCH_INFO: architecture, implementation, revision u32, subRevision u8 (16 bytes).
    let mut a = [0u8; 16];
    let st = rm.control(subdevice, CMD_MC_GET_ARCH_INFO, a.as_mut_ptr(), a.len());
    let (arch, imp, rev) = (rd(&a, 0), rd(&a, 4), rd(&a, 8));
    println!(
        "MC_GET_ARCH_INFO 0x20801701 status={st:#x} architecture={arch:#x} [{}] implementation={imp:#x} revision={rev:#x} subRevision={:#x} chip_id={:#x}",
        arch_name(arch),
        a[12],
        arch | imp
    );

    // BUS_GET_PCI_INFO: pciDeviceId, pciSubSystemId, pciRevisionId, pciExtDeviceId.
    let mut p = [0u8; 16];
    let st = rm.control(subdevice, CMD_BUS_GET_PCI_INFO, p.as_mut_ptr(), p.len());
    println!(
        "BUS_GET_PCI_INFO 0x20801801 status={st:#x} pciDeviceId={:#010x} pciSubSystemId={:#010x} pciRevisionId={:#x} pciExtDeviceId={:#x}",
        rd(&p, 0),
        rd(&p, 4),
        rd(&p, 8),
        rd(&p, 12)
    );

    // GPU_GET_NAME_STRING: flags u32 (0 = ASCII), then 64 bytes (68 in total from
    // 595.x on). Up to 590.x the name was a union with a 64 x u16 unicode buffer, 132
    // bytes in total; the RM refuses the other size, so try the older one on failure.
    let mut n = [0u8; 4 + 128];
    let mut st = rm.control(subdevice, CMD_GPU_GET_NAME_STRING, n.as_mut_ptr(), 4 + 64);
    let mut size = 68;
    if st != 0 {
        st = rm.control(subdevice, CMD_GPU_GET_NAME_STRING, n.as_mut_ptr(), 4 + 128);
        size = 132;
    }
    println!("GPU_GET_NAME_STRING 0x20800110 status={st:#x} params={size} name=\"{}\"", cstr(&n[4..68]));
    let mut s = [0u8; 64];
    let st = rm.control(subdevice, CMD_GPU_GET_SHORT_NAME_STRING, s.as_mut_ptr(), s.len());
    println!("GPU_GET_SHORT_NAME_STRING 0x20800111 status={st:#x} short=\"{}\"", cstr(&s));

    // GPU_GET_CLASSLIST on the device: numClasses u32 @0, classList NvP64 @8 (16 bytes).
    // First pass with a null list returns the count, second pass fills the list.
    let mut c = [0u8; 16];
    let st = rm.control(device, CMD_GPU_GET_CLASSLIST, c.as_mut_ptr(), c.len());
    let count = rd(&c, 0) as usize;
    println!("GPU_GET_CLASSLIST 0x00800201 count pass status={st:#x} numClasses={count}");
    if st == 0 && count > 0 && count <= 4096 {
        let mut list = vec![0u32; count];
        c[8..16].copy_from_slice(&(list.as_mut_ptr() as u64).to_le_bytes());
        let st = rm.control(device, CMD_GPU_GET_CLASSLIST, c.as_mut_ptr(), c.len());
        println!("GPU_GET_CLASSLIST 0x00800201 fill pass status={st:#x}");
        if st == 0 {
            list.sort_unstable();
            for k in &list {
                println!("class {k:#06x}");
            }
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        println!("error: {e}");
        std::process::exit(1);
    }
}

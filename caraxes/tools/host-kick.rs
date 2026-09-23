// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Ring the host doorbell with a given work-submit token from a separate, unprivileged
//! RM client (E5b part 2 diagnosis, docs/caraxes/execution.md): tells whether a channel
//! whose first submission did not run needs a second doorbell (the runlist took the
//! channel after the ring) or is stalled inside its methods.
//!
//!     rustc -O --edition 2021 caraxes/tools/host-kick.rs -o target/host-kick
//!     target/host-kick <token hex> [times]
//!
//! Host GPU access: the RM API only (`/dev/nvidiactl`, `/dev/nvidia0`): a root client, a
//! device, a subdevice, `VOLTA_USERMODE_A` mapped write-only through BAR0 as nvrm-client's
//! `map_doorbell` does, one 32-bit write per ring; everything is freed when the client's
//! fd closes. No register other than the usermode doorbell is touched.

use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;

extern "C" {
    fn ioctl(fd: i32, request: u64, ...) -> i32;
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
}

const NV_ESC_RM_ALLOC: u32 = 0x2B;
const NV_ESC_RM_MAP_MEMORY: u32 = 0x4E;
const NV_ESC_REGISTER_FD: u32 = 201;
const NV01_ROOT_CLIENT: u32 = 0x41;
const NV01_DEVICE_0: u32 = 0x80;
const NV20_SUBDEVICE_0: u32 = 0x2080;
const VOLTA_USERMODE_A: u32 = 0xC361;
const USERMODE_SIZE: usize = 0x10000;
const DOORBELL: usize = 0x90;

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

fn iowr(nr: u32, size: usize) -> u64 {
    (3u64 << 30) | ((size as u64) << 16) | ((b'F' as u64) << 8) | nr as u64
}

fn esc<T>(fd: i32, nr: u32, arg: &mut T) {
    // SAFETY: T is one of the NVOS structs above, sized as the escape expects.
    let r = unsafe { ioctl(fd, iowr(nr, std::mem::size_of::<T>()), arg as *mut T) };
    assert!(r >= 0, "ioctl {nr:#x}: {}", std::io::Error::last_os_error());
}

fn alloc(ctl: i32, root: u32, parent: u32, handle: u32, class: u32, params: *mut u8) -> u32 {
    let mut p = NvOs64 {
        h_root: root,
        h_object_parent: parent,
        h_object_new: handle,
        h_class: class,
        p_alloc_parms: params as u64,
        ..Default::default()
    };
    esc(ctl, NV_ESC_RM_ALLOC, &mut p);
    p.status
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let token = args
        .get(1)
        .and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .expect("usage: host-kick <token hex> [times]");
    let times: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
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
    let c = ctl.as_raw_fd();
    let mut root = NvOs64 {
        h_class: NV01_ROOT_CLIENT,
        ..Default::default()
    };
    esc(c, NV_ESC_RM_ALLOC, &mut root);
    assert_eq!(root.status, 0, "root client");
    let client = root.h_object_new;
    let (device, subdevice, usermode) = (0xCA30_0001, 0xCA30_0002, 0xCA30_0003);
    let mut dp = Nv0080Alloc::default();
    assert_eq!(alloc(c, client, client, device, NV01_DEVICE_0, &mut dp as *mut _ as *mut u8), 0, "device");
    let mut sp = 0u32;
    assert_eq!(alloc(c, client, device, subdevice, NV20_SUBDEVICE_0, &mut sp as *mut _ as *mut u8), 0, "subdevice");
    assert_eq!(alloc(c, client, subdevice, usermode, VOLTA_USERMODE_A, std::ptr::null_mut()), 0, "usermode");
    let mut reg = c;
    esc(gpu.as_raw_fd(), NV_ESC_REGISTER_FD, &mut reg);
    let mut m = NvOs33Fd {
        h_client: client,
        h_device: subdevice,
        h_memory: usermode,
        length: USERMODE_SIZE as u64,
        flags: 2, // ACCESS_WRITE_ONLY
        fd: gpu.as_raw_fd(),
        ..Default::default()
    };
    esc(c, NV_ESC_RM_MAP_MEMORY, &mut m);
    assert_eq!(m.status, 0, "map usermode");
    // SAFETY: RM installed the mapping context on the GPU fd; PROT_WRITE, MAP_SHARED.
    let ptr = unsafe { mmap(std::ptr::null_mut(), USERMODE_SIZE, 0x2, 0x01, gpu.as_raw_fd(), 0) };
    assert!(ptr as isize != -1, "mmap: {}", std::io::Error::last_os_error());
    for i in 0..times {
        // SAFETY: inside the write-only usermode mapping.
        unsafe { std::ptr::write_volatile(ptr.add(DOORBELL) as *mut u32, token) };
        println!("rang the host doorbell with token {token:#x} ({}/{times}) from client {client:#x}", i + 1);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    // Closing the ctl fd frees the client tree.
}

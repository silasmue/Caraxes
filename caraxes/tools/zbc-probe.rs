// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The ZBC clear table as an RM client sees it (docs/caraxes/gsp-plan/d-device-model.md 6.2,
//! Measurement Q5): a `GF100_ZBC_CLEAR` object under the client's own subdevice, then
//! `GET_ZBC_CLEAR_TABLE_SIZE` for colour, depth and stencil and `GET_ZBC_CLEAR_TABLE_ENTRY`
//! for every index of each range. Every control is printed with its RM status.
//!
//! Two uses:
//!
//! * on the host, without arguments: the one read-only sweep of the real table that seeds
//!   the device's guest-local ZBC model (chips/tu106/zbc.toml). Only RM escapes are issued:
//!   RM_ALLOC of NV01_ROOT_CLIENT, NV01_DEVICE_0, NV20_SUBDEVICE_0 and GF100_ZBC_CLEAR
//!   (NON_PRIVILEGED, rmapi/resource_list.h:874-884), the two GET controls, RM_FREE of the
//!   client. No SET control, no register access, no mapping.
//! * in a Caraxes guest, with `--set`: the same sweep, then one `SET_ZBC_COLOR_CLEAR`,
//!   `SET_ZBC_DEPTH_CLEAR` and `SET_ZBC_STENCIL_CLEAR` with values outside the default
//!   table, then the sweep again: the device's model must hand the values back. `--set`
//!   refuses to run unless /proc/cpuinfo names the `hypervisor` flag, so it never writes the
//!   host's shared table.
//!
//! Builds with rustc alone, no crates, like lab-rm-query.rs:
//!
//!     rustc -O --edition 2021 -C target-feature=+crt-static caraxes/tools/zbc-probe.rs -o target/zbc-probe
//!
//! Layouts: ctrl/ctrl9096.h (identical in 610.57.04 and 615.71.09): SET_ZBC_COLOR_CLEAR
//! 40 bytes (:118-123), SET_ZBC_DEPTH_CLEAR 12 (:167-171), SET_ZBC_STENCIL_CLEAR 12
//! (:343-347), GET_ZBC_CLEAR_TABLE_SIZE 12 (:383-387), GET_ZBC_CLEAR_TABLE_ENTRY 56
//! (:431-442); nvos.h for the escapes (NVOS00/54/64).

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
const GF100_ZBC_CLEAR: u32 = 0x9096;

const CMD_SET_ZBC_COLOR_CLEAR: u32 = 0x9096_0101;
const CMD_SET_ZBC_DEPTH_CLEAR: u32 = 0x9096_0102;
const CMD_SET_ZBC_STENCIL_CLEAR: u32 = 0x9096_0105;
const CMD_GET_ZBC_CLEAR_TABLE_SIZE: u32 = 0x9096_0106;
const CMD_GET_ZBC_CLEAR_TABLE_ENTRY: u32 = 0x9096_0107;

/// NV9096_CTRL_ZBC_CLEAR_TABLE_TYPE (ctrl9096.h:371-377).
const TYPES: [(u32, &str); 3] = [(1, "color"), (2, "depth"), (3, "stencil")];

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

const _: () = {
    assert!(std::mem::size_of::<NvOs00>() == 16);
    assert!(std::mem::size_of::<NvOs64>() == 48);
    assert!(std::mem::size_of::<NvOs54>() == 32);
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
        println!("RM_ALLOC class {class:#06x} handle {handle:#x} under {parent:#x}: status {:#x}", p.status);
        if p.status != 0 {
            return Err(format!("RM_ALLOC class {class:#x}: status {:#x}", p.status));
        }
        Ok(())
    }

    /// The RM status; `buf` is the params block, updated in place.
    fn control(&self, object: u32, cmd: u32, buf: &mut [u8]) -> u32 {
        let mut p = NvOs54 {
            h_client: self.client,
            h_object: object,
            cmd,
            params: buf.as_mut_ptr() as u64,
            params_size: buf.len() as u32,
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
            println!("RM_FREE client {:#x}: status {:#x}", self.client, p.status);
        }
    }
}

fn rd(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn wr(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// Every entry of every table: (type, index, valid, value words, format).
fn sweep(rm: &Rm, zbc: u32) -> Vec<(u32, u32, bool, Vec<u32>, u32)> {
    let mut out = Vec::new();
    for (ty, name) in TYPES {
        let mut s = [0u8; 12];
        wr(&mut s, 8, ty);
        let st = rm.control(zbc, CMD_GET_ZBC_CLEAR_TABLE_SIZE, &mut s);
        let (start, end) = (rd(&s, 0), rd(&s, 4));
        println!("GET_ZBC_CLEAR_TABLE_SIZE 0x90960106 tableType={ty} ({name}) status={st:#x} indexStart={start} indexEnd={end}");
        if st != 0 || end < start || end - start > 64 {
            continue;
        }
        for index in start..=end {
            let mut e = [0u8; 56];
            wr(&mut e, 44, index);
            wr(&mut e, 52, ty);
            let st = rm.control(zbc, CMD_GET_ZBC_CLEAR_TABLE_ENTRY, &mut e);
            let words: Vec<u32> = (0..10).map(|i| rd(&e, i * 4)).collect();
            let (format, valid) = (rd(&e, 40), e[48] != 0);
            let value = match ty {
                1 => format!(
                    "colorFB={:#010x},{:#010x},{:#010x},{:#010x} colorDS={:#010x},{:#010x},{:#010x},{:#010x}",
                    words[0], words[1], words[2], words[3], words[4], words[5], words[6], words[7]
                ),
                2 => format!("depth={:#010x}", words[8]),
                _ => format!("stencil={:#010x}", words[9]),
            };
            println!(
                "GET_ZBC_CLEAR_TABLE_ENTRY 0x90960107 tableType={ty} ({name}) index={index} status={st:#x} bIndexValid={} format={format:#x} {value}",
                valid as u8
            );
            if st == 0 {
                out.push((ty, index, valid, words, format));
            }
        }
    }
    out
}

fn in_a_guest() -> bool {
    std::fs::read_to_string("/proc/cpuinfo")
        .map(|c| c.lines().any(|l| l.starts_with("flags") && l.split_whitespace().any(|w| w == "hypervisor")))
        .unwrap_or(false)
}

fn run(set: bool) -> Result<bool, String> {
    let ctl = OpenOptions::new().read(true).write(true).open("/dev/nvidiactl").map_err(|e| format!("open /dev/nvidiactl: {e}"))?;
    // RM_ALLOC of NV01_DEVICE_0 needs the GPU node open in this process.
    let _gpu = OpenOptions::new().read(true).write(true).open("/dev/nvidia0").map_err(|e| format!("open /dev/nvidia0: {e}"))?;
    let mut rm = Rm { ctl, client: 0 };
    let mut root = NvOs64 { h_class: NV01_ROOT_CLIENT, ..Default::default() };
    rm.esc(NV_ESC_RM_ALLOC, &mut root)?;
    println!("RM_ALLOC class {NV01_ROOT_CLIENT:#06x} (root client): status {:#x} handle {:#x}", root.status, root.h_object_new);
    if root.status != 0 {
        return Err(format!("root client: status {:#x}", root.status));
    }
    rm.client = root.h_object_new;
    let (device, subdevice, zbc) = (0xCAF3_0001u32, 0xCAF3_0002u32, 0xCAF3_0003u32);
    // NV0080_ALLOC_PARAMETERS (56 bytes, deviceId 0) and NV2080_ALLOC_PARAMETERS (subDeviceId 0).
    let mut dp = [0u8; 56];
    rm.alloc(rm.client, device, NV01_DEVICE_0, dp.as_mut_ptr())?;
    let mut sp = [0u8; 4];
    rm.alloc(device, subdevice, NV20_SUBDEVICE_0, sp.as_mut_ptr())?;
    rm.alloc(subdevice, zbc, GF100_ZBC_CLEAR, std::ptr::null_mut())?;
    let before = sweep(&rm, zbc);
    println!("sweep: {} entries answered, {} valid", before.len(), before.iter().filter(|e| e.2).count());
    if !set {
        return Ok(true);
    }
    // Values outside any default table: a colour, a depth of 0.25f and stencil 0x5A.
    let color = [0x3F11_1111u32, 0x3F22_2222, 0x3F33_3333, 0x3F80_0000];
    let mut c = [0u8; 40];
    for (i, w) in color.iter().enumerate() {
        wr(&mut c, i * 4, *w);
        wr(&mut c, 16 + i * 4, *w);
    }
    wr(&mut c, 32, 0x04); // FMT_VAL_RF32_GF32_BF32_AF32 (ctrl9096.h:65)
    let st = rm.control(zbc, CMD_SET_ZBC_COLOR_CLEAR, &mut c);
    println!("SET_ZBC_COLOR_CLEAR 0x90960101 status={st:#x}");
    let mut d = [0u8; 12];
    wr(&mut d, 0, 0x3E80_0000);
    wr(&mut d, 4, 1); // FMT_FP32 (ctrl9096.h:141)
    let st = rm.control(zbc, CMD_SET_ZBC_DEPTH_CLEAR, &mut d);
    println!("SET_ZBC_DEPTH_CLEAR 0x90960102 status={st:#x}");
    let mut s = [0u8; 12];
    wr(&mut s, 0, 0x5A);
    wr(&mut s, 4, 1); // FMT_U8 (ctrl9096.h:319)
    let st = rm.control(zbc, CMD_SET_ZBC_STENCIL_CLEAR, &mut s);
    println!("SET_ZBC_STENCIL_CLEAR 0x90960105 status={st:#x}");
    let after = sweep(&rm, zbc);
    let found = |ty: u32, w: &dyn Fn(&[u32]) -> bool| after.iter().any(|e| e.0 == ty && e.2 && w(&e.3));
    let ok_c = found(1, &|v| v[..4] == color);
    let ok_d = found(2, &|v| v[8] == 0x3E80_0000);
    let ok_s = found(3, &|v| v[9] == 0x5A);
    println!("readback: color {ok_c}, depth {ok_d}, stencil {ok_s}");
    Ok(ok_c && ok_d && ok_s)
}

fn main() {
    let set = std::env::args().any(|a| a == "--set");
    if set && !in_a_guest() {
        eprintln!("zbc-probe: --set writes the ZBC table and runs only in a guest (no hypervisor flag in /proc/cpuinfo)");
        std::process::exit(2);
    }
    match run(set) {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("zbc-probe: {e}");
            std::process::exit(1);
        }
    }
}

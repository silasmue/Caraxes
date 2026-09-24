// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The read-only getter sweep of the lab kit (docs/caraxes/gsp-plan/lab-kit.md): every control
//! of the list made by caraxes/tools/sweep-list.py (the category A, B and E getters of one
//! driver version) issued once on the card's own objects of a private RM client, with its
//! status and answer. The qb1-sweep example (crates/vfio-user-nvgpu/examples/qb1-sweep.rs) does
//! the same for 30 controls through the crate; this one builds with rustc alone, like
//! lab-rm-query.rs, so the kit can carry it prebuilt:
//!
//!     rustc -O --edition 2021 caraxes/tools/lab-sweep.rs -o target/lab-sweep
//!     target/lab-sweep LIST [--device N] [--priv-only]
//!
//! Only RM escapes are issued: RM_ALLOC of NV01_ROOT_CLIENT, NV01_DEVICE_0, NV20_SUBDEVICE_0 and,
//! where the list needs them and the RM allows them, NV04_DISPLAY_COMMON (0x0073), GF100_ZBC_CLEAR
//! (0x9096), GF100_SUBDEVICE_INFOROM (0x90E7) and NV20_SUBDEVICE_DIAG (0x208F); RM_CONTROL of the
//! listed getters with a zeroed buffer of the listed size; RM_FREE of the client. No register
//! access, no mapping, no setter (sweep-list.py leaves out every name with a verb that changes
//! state). A size that does not match the running driver is refused by the RM with
//! NV_ERR_INVALID_ARGUMENT before anything is read (control.c:446-461 of 615.71.09, as
//! docs/caraxes/lab-blackwell.md section 5 records), so an older or newer driver costs rows, not
//! correctness.
//!
//! Output, one line per call: id, name (with [index] or [display 0x..] for the per-index and
//! per-display rows), size, NV status, answer. The answer is the hex of the buffer after the
//! call with trailing zero bytes cut (`+N zero`), or, for the controls whose answer can carry an
//! identifier of the machine or its users (GID, UUID, serial, hostname, GUID, PIDs, process and
//! accounting data, OEM and board data), only `redacted: N non-zero bytes`.
//!
//! `--priv-only` issues only the rows marked `priv`; the kit runs it as root in its optional
//! privileged pass (the unprivileged pass records them as NV_ERR_INSUFFICIENT_PERMISSIONS).

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
const NV04_DISPLAY_COMMON: u32 = 0x73;
const GF100_ZBC_CLEAR: u32 = 0x9096;
const GF100_SUBDEVICE_INFOROM: u32 = 0x90E7;
const NV20_SUBDEVICE_DIAG: u32 = 0x208F;

/// NV0073_CTRL_CMD_SYSTEM_GET_SUPPORTED: subDeviceInstance, displayMask, displayMaskDDC
/// (ctrl0073system.h:311-319 of 615.71.09).
const CMD_0073_GET_SUPPORTED: u32 = 0x0073_0107;

/// Name parts whose answer can hold an identifier; printed as a count only.
const REDACT: &[&str] = &[
    "GID", "UUID", "SERIAL", "HOSTNAME", "GUID", "PIDS", "PID_", "PROCESS", "ACCOUNTING", "OEM", "ECID",
    "FRU", "CHASSIS", "BOARD_INFO", "MODULE_ID", "PLATFORM_INFO",
];

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

    /// RM_ALLOC; returns the RM status (0xFFFFFFFF when the ioctl itself failed).
    fn alloc(&self, parent: u32, handle: u32, class: u32, params: *mut u8) -> u32 {
        let mut p = NvOs64 {
            h_root: self.client,
            h_object_parent: parent,
            h_object_new: handle,
            h_class: class,
            p_alloc_parms: params as u64,
            ..Default::default()
        };
        match self.esc(NV_ESC_RM_ALLOC, &mut p) {
            Ok(()) => p.status,
            Err(_) => 0xFFFF_FFFF,
        }
    }

    fn control(&self, object: u32, cmd: u32, buf: &mut [u8]) -> u32 {
        let mut p = NvOs54 {
            h_client: self.client,
            h_object: object,
            cmd,
            params: if buf.is_empty() { 0 } else { buf.as_mut_ptr() as u64 },
            params_size: buf.len() as u32,
            ..Default::default()
        };
        match self.esc(NV_ESC_RM_CONTROL, &mut p) {
            Ok(()) => p.status,
            Err(_) => 0xFFFF_FFFF,
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

struct Row {
    id: u32,
    name: String,
    iface: String,
    size: usize,
    access: String,
    tmpl: String,
}

fn parse_list(path: &str) -> Result<Vec<Row>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    let mut rows = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 9 {
            return Err(format!("{path}:{}: expected 9 fields", n + 1));
        }
        let id = u32::from_str_radix(f[0].trim_start_matches("0x"), 16).map_err(|e| format!("{path}:{}: {e}", n + 1))?;
        let size = f[3].parse::<usize>().map_err(|e| format!("{path}:{}: {e}", n + 1))?;
        rows.push(Row {
            id,
            name: f[1].to_string(),
            iface: f[2].to_string(),
            size,
            access: f[6].to_string(),
            tmpl: f[7].to_string(),
        });
    }
    Ok(rows)
}

fn answer(name: &str, b: &[u8]) -> String {
    let nz = b.iter().filter(|&&x| x != 0).count();
    if REDACT.iter().any(|k| name.contains(k)) {
        return format!("redacted: {nz} non-zero bytes");
    }
    let end = b.iter().rposition(|&x| x != 0).map_or(0, |i| i + 1);
    let hex: String = b[..end].iter().map(|x| format!("{x:02x}")).collect();
    if end < b.len() {
        format!("{hex}+{} zero", b.len() - end)
    } else {
        hex
    }
}

fn u32at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let mut list = None;
    let mut device = 0u32;
    let mut priv_only = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--device" => {
                device = args.get(i + 1).and_then(|s| s.parse().ok()).ok_or("--device needs a number")?;
                i += 1;
            }
            "--priv-only" => priv_only = true,
            "-h" | "--help" => {
                println!("usage: lab-sweep LIST [--device N] [--priv-only]");
                return Ok(());
            }
            s => list = Some(s.to_string()),
        }
        i += 1;
    }
    let list = list.ok_or("usage: lab-sweep LIST [--device N] [--priv-only]")?;
    let rows = parse_list(&list)?;

    let ctl = OpenOptions::new().read(true).write(true).open("/dev/nvidiactl").map_err(|e| format!("open /dev/nvidiactl: {e}"))?;
    let node = format!("/dev/nvidia{device}");
    let _gpu = OpenOptions::new().read(true).write(true).open(&node).map_err(|e| format!("open {node}: {e}"))?;
    let mut rm = Rm { ctl, client: 0 };
    let mut root = NvOs64 { h_class: NV01_ROOT_CLIENT, ..Default::default() };
    rm.esc(NV_ESC_RM_ALLOC, &mut root)?;
    if root.status != 0 {
        return Err(format!("root client: status {:#x}", root.status));
    }
    rm.client = root.h_object_new;
    let (hdev, hsub) = (0xCAF3_0001u32, 0xCAF3_0002u32);
    let mut dp = Nv0080Alloc { device_id: device, ..Default::default() };
    let st = rm.alloc(rm.client, hdev, NV01_DEVICE_0, &mut dp as *mut _ as *mut u8);
    if st != 0 {
        return Err(format!("NV01_DEVICE_0 {device}: status {st:#x}"));
    }
    let mut sp = 0u32;
    let st = rm.alloc(hdev, hsub, NV20_SUBDEVICE_0, &mut sp as *mut _ as *mut u8);
    if st != 0 {
        return Err(format!("NV20_SUBDEVICE_0: status {st:#x}"));
    }
    println!("# lab-sweep: list {list}, {} rows, device {device}{}", rows.len(), if priv_only { ", privileged rows only" } else { "" });

    // The other objects, only when a row needs them; an object the RM refuses leaves its rows
    // with the status "no object".
    let need = |i: &str| rows.iter().any(|r| r.iface == i);
    let mut objects: Vec<(&str, u32)> = vec![("0000", rm.client), ("0080", hdev), ("2080", hsub)];
    for (iface, class, parent, handle) in [
        ("0073", NV04_DISPLAY_COMMON, hdev, 0xCAF3_0073u32),
        ("9096", GF100_ZBC_CLEAR, hsub, 0xCAF3_9096),
        ("90e7", GF100_SUBDEVICE_INFOROM, hsub, 0xCAF3_90E7),
        ("208f", NV20_SUBDEVICE_DIAG, hsub, 0xCAF3_208F),
    ] {
        if !need(iface) {
            continue;
        }
        let st = rm.alloc(parent, handle, class, std::ptr::null_mut());
        println!("# alloc class {class:#06x} for {iface}: status {st:#x}");
        if st == 0 {
            objects.push((iface, handle));
        }
    }
    let object = |iface: &str| objects.iter().find(|(i, _)| *i == iface).map(|&(_, h)| h);

    // The displays of the display common object, for the per-display rows.
    let mut displays = Vec::new();
    if let Some(h) = object("0073") {
        let mut b = [0u8; 12];
        let st = rm.control(h, CMD_0073_GET_SUPPORTED, &mut b);
        let mask = if st == 0 { u32at(&b, 4) } else { 0 };
        println!("# 0073 SYSTEM_GET_SUPPORTED: status {st:#x} displayMask {mask:#x} displayMaskDDC {:#x}", u32at(&b, 8));
        displays = (0..32).map(|i| 1u32 << i).filter(|m| mask & m != 0).collect();
    }

    println!("# id\tname\tsize\tstatus\tanswer");
    for r in &rows {
        if priv_only && r.access != "priv" {
            continue;
        }
        let Some(h) = object(&r.iface) else {
            println!("{:#010x}\t{}\t{}\t-\tno object (class {} not allocated)", r.id, r.name, r.size, r.iface);
            continue;
        };
        if let Some(n) = r.tmpl.strip_prefix("perindex:") {
            // infoListSize u32 @0, then {index u32, data u32} entries @4 (ctrl2080fb.h:494-497,
            // ctrl2080bus.h:641-644, ctrl2080gpu.h:303-306 of 615.71.09).
            let n: u32 = n.parse().unwrap_or(0);
            for idx in 0..n {
                let mut b = vec![0u8; r.size];
                b[0..4].copy_from_slice(&1u32.to_le_bytes());
                b[4..8].copy_from_slice(&idx.to_le_bytes());
                let st = rm.control(h, r.id, &mut b);
                println!("{:#010x}\t{}[{idx}]\t{}\t{st:#x}\t{}", r.id, r.name, r.size, if st == 0 { format!("data {:#x}", u32at(&b, 8)) } else { "-".into() });
            }
        } else if r.tmpl == "perdisplay" {
            if displays.is_empty() {
                println!("{:#010x}\t{}\t{}\t-\tno display", r.id, r.name, r.size);
            }
            for &d in &displays {
                let mut b = vec![0u8; r.size];
                b[4..8].copy_from_slice(&d.to_le_bytes());
                let st = rm.control(h, r.id, &mut b);
                println!("{:#010x}\t{}[display {d:#x}]\t{}\t{st:#x}\t{}", r.id, r.name, r.size, answer(&r.name, &b));
            }
        } else {
            let mut b = vec![0u8; r.size];
            let st = rm.control(h, r.id, &mut b);
            println!("{:#010x}\t{}\t{}\t{st:#x}\t{}", r.id, r.name, r.size, answer(&r.name, &b));
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

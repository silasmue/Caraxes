// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The ZBC path of real compression on this host (zbc.rs `SliceZbc`,
//! docs/research/compression-isolation-2026-09-28.md "ZBC"). The device's own unprivileged
//! host RM client (`hostrm::HostRmClient`, the client every forward uses) reads the host's
//! colour table, sets one colour through the device's setter path (`zbc::slice_control`
//! with `SET_ZBC_COLOR_CLEAR`, as a guest's setter reaches it with
//! `CARAXES_COMPRESSION=slice`), and prints where the host put it and what the guest's view
//! answers there. Then it frees the client, opens a second one and reads the table again:
//! does an entry outlive the client that set it?
//!
//! The table is shared with the host desktop, so without `--allow-new` the set runs only
//! where it changes nothing: the colour is already in the table (the host references it
//! again) or the table is full (the host refuses). Every host call is printed.
//!
//! Host calls, all of the normal RM API on /dev/nvidiactl: RM_ALLOC of NV01_ROOT_CLIENT,
//! NV01_DEVICE_0 and NV20_SUBDEVICE_0 (`HostRmClient::open`); RM_ALLOC of GF100_ZBC_CLEAR
//! under the subdevice (with the first ZBC control, NON_PRIVILEGED,
//! rmapi/resource_list.h:874-884); RM_CONTROL `GET_ZBC_CLEAR_TABLE_SIZE`,
//! `GET_ZBC_CLEAR_TABLE_ENTRY` and at most one `SET_ZBC_COLOR_CLEAR`; the same reads for the
//! second client. The clients are freed when their file descriptors close. No register
//! access, no mapping, no privileged control, no reset.
//!
//! Default colour: opaque black, A8B8G8R8 (format 0x28), the colour of index 7 in the host
//! sweep of 2026-09-24 (caraxes/runs/gsp-round2-d-2026-09-24/q5-host-sweep/sweep.txt).
//!
//! Run: cargo run --release -p vfio-user-nvgpu --example zbc-host
//!      [-- --fb W,W,W,W --ds W,W,W,W --format F --allow-new]

use vfio_user_nvgpu::ctrl::HostRm;
use vfio_user_nvgpu::hostrm::{
    HostRmClient, CMD_GET_ZBC_CLEAR_TABLE_ENTRY, CMD_GET_ZBC_CLEAR_TABLE_SIZE,
    CMD_SET_ZBC_COLOR_CLEAR, GF100_ZBC_CLEAR, ZBC_CONTROLS,
};
use vfio_user_nvgpu::zbc::{self, Entry, SliceZbc};

/// NV9096_CTRL_ZBC_CLEAR_TABLE_TYPE_COLOR (ctrl9096.h:373).
const COLOR: u32 = 1;

fn rd(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn wr(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

fn words(v: &[u32]) -> String {
    v.iter()
        .map(|w| format!("{w:#010x}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// The request and answer of a ZBC control, for the log (layouts ctrl9096.h:118-123,
/// :383-387, :431-442).
fn describe(cmd: u32, b: &[u8], answered: bool) -> String {
    match cmd {
        CMD_GET_ZBC_CLEAR_TABLE_SIZE if answered => format!(
            "tableType {} -> indexStart {} indexEnd {}",
            rd(b, 8),
            rd(b, 0),
            rd(b, 4)
        ),
        CMD_GET_ZBC_CLEAR_TABLE_ENTRY if answered => format!(
            "tableType {} index {} -> bIndexValid {} format {:#x} colorFB {} colorDS {}",
            rd(b, 52),
            rd(b, 44),
            b[48],
            rd(b, 40),
            words(&(0..4).map(|i| rd(b, 4 * i)).collect::<Vec<_>>()),
            words(&(4..8).map(|i| rd(b, 4 * i)).collect::<Vec<_>>())
        ),
        CMD_SET_ZBC_COLOR_CLEAR => format!(
            "colorFB {} colorDS {} format {:#x} bSkipL2Table {}",
            words(&(0..4).map(|i| rd(b, 4 * i)).collect::<Vec<_>>()),
            words(&(4..8).map(|i| rd(b, 4 * i)).collect::<Vec<_>>()),
            rd(b, 32),
            b[36]
        ),
        _ => String::new(),
    }
}

/// The device's host client with every control printed.
struct Logged {
    rm: HostRmClient,
    calls: usize,
    zbc_allocated: bool,
}

impl HostRm for Logged {
    fn control(&mut self, class: u32, cmd: u32, p: &mut [u8]) -> u32 {
        if class == GF100_ZBC_CLEAR && !self.zbc_allocated {
            println!("host RM_ALLOC GF100_ZBC_CLEAR (0x9096) under the client's subdevice, with this first ZBC control");
            self.zbc_allocated = true;
        }
        let name = ZBC_CONTROLS
            .iter()
            .find(|c| c.0 == cmd)
            .map_or("?", |c| c.1);
        let req = if cmd == CMD_SET_ZBC_COLOR_CLEAR {
            describe(cmd, p, false)
        } else {
            String::new()
        };
        let st = self.rm.control(class, cmd, p);
        self.calls += 1;
        let ans = if cmd == CMD_SET_ZBC_COLOR_CLEAR {
            req
        } else {
            describe(cmd, p, true)
        };
        println!("host RM_CONTROL {cmd:#010x} {name}: {ans} status {st:#x}");
        st
    }
}

fn open() -> anyhow::Result<Logged> {
    let rm = HostRmClient::open()?;
    println!("host RM_ALLOC NV01_ROOT_CLIENT, NV01_DEVICE_0 (deviceId 0), NV20_SUBDEVICE_0 (subDeviceId 0): client open");
    Ok(Logged {
        rm,
        calls: 0,
        zbc_allocated: false,
    })
}

/// The colour table: its range and every index with its entry (`None`: unused).
fn sweep(h: &mut Logged) -> anyhow::Result<Vec<(u32, Option<Entry>)>> {
    let mut s = [0u8; 12];
    wr(&mut s, 8, COLOR);
    let st = h.control(GF100_ZBC_CLEAR, CMD_GET_ZBC_CLEAR_TABLE_SIZE, &mut s);
    anyhow::ensure!(st == 0, "GET_ZBC_CLEAR_TABLE_SIZE: status {st:#x}");
    let (start, end) = (rd(&s, 0), rd(&s, 4));
    anyhow::ensure!(start <= end && end - start < 64, "range {start}..={end}");
    let mut out = Vec::new();
    for index in start..=end {
        let mut e = [0u8; 56];
        wr(&mut e, 44, index);
        wr(&mut e, 52, COLOR);
        let st = h.control(GF100_ZBC_CLEAR, CMD_GET_ZBC_CLEAR_TABLE_ENTRY, &mut e);
        anyhow::ensure!(st == 0, "GET_ZBC_CLEAR_TABLE_ENTRY {index}: status {st:#x}");
        let value = (0..8).map(|i| rd(&e, 4 * i)).collect();
        out.push((
            index,
            (e[48] != 0).then_some(Entry {
                value,
                format: rd(&e, 40),
            }),
        ));
    }
    Ok(out)
}

fn arg_words(args: &[String], flag: &str, default: [u32; 4]) -> anyhow::Result<[u32; 4]> {
    let Some(i) = args.iter().position(|a| a == flag) else {
        return Ok(default);
    };
    let v: Vec<u32> = args
        .get(i + 1)
        .ok_or_else(|| anyhow::anyhow!("{flag} needs W,W,W,W"))?
        .split(',')
        .map(|w| u32::from_str_radix(w.trim().trim_start_matches("0x"), 16))
        .collect::<Result<_, _>>()?;
    v.try_into()
        .map_err(|_| anyhow::anyhow!("{flag} needs four words"))
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let fb = arg_words(&args, "--fb", [0xFF00_0000; 4])?;
    let ds = arg_words(&args, "--ds", [0, 0, 0, 0x3F80_0000])?;
    let format = match args.iter().position(|a| a == "--format") {
        Some(i) => u32::from_str_radix(
            args.get(i + 1)
                .map_or("", |s| s.as_str())
                .trim_start_matches("0x"),
            16,
        )?,
        None => 0x28,
    };
    let allow_new = args.iter().any(|a| a == "--allow-new");
    let sel = vfio_user_nvgpu::hostabi::select().map_err(anyhow::Error::msg)?;
    println!(
        "# ZBC host probe: {} (chosen by {})",
        sel.layouts.describe(),
        sel.how
    );
    println!(
        "# colour: colorFB {} colorDS {} format {format:#x}",
        words(&fb),
        words(&ds)
    );
    let want = Entry {
        value: fb.iter().chain(ds.iter()).copied().collect(),
        format,
    };

    let mut h = open()?;
    let before = sweep(&mut h)?;
    let equal = before
        .iter()
        .find(|(_, e)| e.as_ref() == Some(&want))
        .map(|x| x.0);
    let free = before.iter().filter(|(_, e)| e.is_none()).count();
    println!(
        "# before: {} of {} colour slots used, {free} free; the colour is {}",
        before.len() - free,
        before.len(),
        equal.map_or("not in the table".into(), |i| format!("at index {i}"))
    );
    match (equal, free) {
        (Some(_), _) => println!("# the set references the existing entry: no slot is filled"),
        (None, 0) => {
            println!("# the table is full: the host must refuse the set, nothing is written")
        }
        (None, _) if allow_new => println!("# --allow-new: the set fills one free slot"),
        (None, n) => {
            println!("# skipped: the set would fill one of the {n} free slots of the table the desktop shares; pass --allow-new to allow it");
            println!("# host calls: {}", h.calls);
            return Ok(());
        }
    }

    // The guest's setter as the device answers it with real compression.
    let mut p = [0u8; 40];
    for i in 0..4 {
        wr(&mut p, 4 * i, fb[i]);
        wr(&mut p, 16 + 4 * i, ds[i]);
    }
    wr(&mut p, 32, format);
    let mut s = SliceZbc::new("tu106", zbc::DEFAULT_MAX);
    let a = zbc::slice_control(
        &mut s,
        Some(&mut h as &mut dyn HostRm),
        "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR",
        &p,
    )
    .expect("a ZBC control");
    println!(
        "device answer to the guest: status {:#x} [{}] {}",
        a.status, a.label, a.note
    );
    if let Some(r) = s.refs.first().cloned() {
        let mut g = [0u8; 56];
        wr(&mut g, 44, r.index);
        wr(&mut g, 52, COLOR);
        let calls = h.calls;
        let a = zbc::slice_control(
            &mut s,
            Some(&mut h as &mut dyn HostRm),
            "NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY",
            &g,
        )
        .expect("a ZBC control");
        println!(
            "guest view: GET_ZBC_CLEAR_TABLE_ENTRY index {}: {} [{}] ({} host call(s))",
            r.index,
            describe(CMD_GET_ZBC_CLEAR_TABLE_ENTRY, &a.params, true),
            a.label,
            h.calls - calls
        );
    }
    let after = sweep(&mut h)?;
    let changed: Vec<String> = before
        .iter()
        .zip(&after)
        .filter(|(x, y)| x.1 != y.1)
        .map(|(x, _)| x.0.to_string())
        .collect();
    println!(
        "# after: {}",
        if changed.is_empty() {
            "the host table is unchanged".into()
        } else {
            format!("the host table changed at index {}", changed.join(","))
        }
    );
    let at = after
        .iter()
        .find(|(_, e)| e.as_ref() == Some(&want))
        .map(|x| x.0);
    println!("# host calls of the first client: {}", h.calls);
    drop(h);
    println!("host client freed (its file descriptors closed)");

    let mut h = open()?;
    let again = sweep(&mut h)?;
    let still = again
        .iter()
        .find(|(_, e)| e.as_ref() == Some(&want))
        .map(|x| x.0);
    println!(
        "# second client: the colour is {} (it was {} after the set)",
        still.map_or("not in the table".into(), |i| format!("at index {i}")),
        at.map_or("not in the table".into(), |i| format!("at index {i}"))
    );
    println!("# host calls of the second client: {}", h.calls);
    Ok(())
}

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The software CE's data movement on a device-owned host copy engine
//! (docs/research/device-ce-2026-09-29.md).
//!
//! The software executor (channel.rs) keeps decoding the guest's copy-engine pushes in order
//! and validating every piece against the VM's own memory. For a large transfer it hands
//! the bytes to a host copy engine the device owns, instead of writing them through the BAR1
//! windows at 250 MB/s (run P32: the Windows KMD's paging channel cleared 19.7 GiB in 75 s of
//! CPU). The engine's channel follows the invalidator's pattern (invalidate.rs): a COPY TSG on
//! the device's own host client, an externally-owned VA space whose page tables the device
//! writes into its private pages at the top of the reservation. That VA space maps the
//! guest's FB below the carve-out and nothing else, so no transfer can leave the VM by
//! construction; no guest method reaches the host, and the channel is not privileged.
//!
//! One push is outstanding at a time. The software CE waits for it through the held replay
//! (the `ce-slices` clock of main.rs), never in the vfio-user loop. A push not done in
//! [`TIMEOUT`] disables the engine for good: the transfer is redone on the CPU.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nvrm_abi::sys;
use nvrm_client::mem::CpuMap;

use crate::fb::{FakeFb, Target};
use crate::hostmem::HostMemory;
use crate::ver::common as v;

/// The device CE as the FB store holds it (shared with the mirror, which frees it).
pub type SharedEngine = Arc<Mutex<dyn Engine + Send>>;

/// Transfers of at least this many bytes go to the device CE; smaller ones (page tables,
/// semaphores) stay on the CPU, where the FB store's watches see every byte.
pub const MIN_BYTES: u64 = 64 << 10;
/// A push not done in this time disables the engine (a fault or a hang of its channel).
pub const TIMEOUT: Duration = Duration::from_secs(2);
/// At most this many copy or fill operations in one push.
pub const MAX_OPS: usize = 256;
/// The longest single operation (the CE's LINE_LENGTH_IN is 32 bits).
const MAX_OP_BYTES: u64 = 1 << 30;

/// Where the engine sees the guest's FB: guest FB `[guest_base, guest_base + len)` at device
/// VA `dva`. Nothing else is mapped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub guest_base: u64,
    pub len: u64,
    pub dva: u64,
}

impl Window {
    /// The device VA of guest FB `[pa, pa + len)`, when all of it lies in the window.
    pub fn dva(&self, pa: u64, len: u64) -> Option<u64> {
        let off = pa.checked_sub(self.guest_base)?;
        (len > 0 && off.checked_add(len)? <= self.len).then(|| self.dva + off)
    }
}

/// A constant-remap fill's element (the guest's `SET_REMAP_*`, clc5b5.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Remap {
    pub const_a: u32,
    pub const_b: u32,
    pub components: u32,
}

/// One device-CE operation on device VAs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Copy {
        src: u64,
        dst: u64,
        len: u64,
    },
    Fill {
        dst: u64,
        elements: u64,
        remap: Remap,
    },
}

/// The state of a submitted push.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Poll {
    Done,
    Pending,
    Failed(String),
}

/// A host engine that runs device-CE pushes ([`DeviceCe`]; a test double in the tests).
pub trait Engine {
    fn window(&self) -> Window;
    /// Submit `ops`; the push's sequence number. Refused while another push runs, after a
    /// failure, or once freed.
    fn submit(&mut self, fb: &mut FakeFb, ops: &[Op]) -> Result<u32, String>;
    fn poll(&mut self, fb: &mut FakeFb, seq: u32) -> Poll;
    /// Why the engine is off for good (a failed push, freed), if it is.
    fn off(&self) -> Option<String>;
}

/// The FB runs of `pieces` merged where they are contiguous: `(pa, len)`. `None` when a
/// piece is not FB (guest RAM stays on the CPU path for now).
fn fb_runs(pieces: &[(Target, usize)]) -> Option<Vec<(u64, u64)>> {
    let mut out: Vec<(u64, u64)> = Vec::new();
    for &(t, n) in pieces {
        let Target::Fb(pa) = t else {
            return None;
        };
        match out.last_mut() {
            Some((a, l)) if *a + *l == pa => *l += n as u64,
            _ => out.push((pa, n as u64)),
        }
    }
    Some(out)
}

/// Check a run against the window and the store: its device VA.
fn run_dva(w: &Window, fb: &FakeFb, pa: u64, len: u64) -> Result<u64, String> {
    let dva = w.dva(pa, len).ok_or_else(|| {
        format!(
            "FB [{pa:#x}, {:#x}) outside the device CE window [{:#x}, {:#x})",
            pa.saturating_add(len),
            w.guest_base,
            w.guest_base + w.len
        )
    })?;
    if !fb.plain_vram(pa, len) {
        return Err(format!(
            "FB [{pa:#x}, {:#x}) is not plain VRAM (BAR2 memfd or BAR1 pool page)",
            pa + len
        ));
    }
    Ok(dva)
}

/// The operations of a copy of `src` to `dst` (the rows' pieces, equal total length), or
/// why it stays on the CPU. Nothing reaches the host for a refused plan.
pub fn plan_copy(
    w: &Window,
    fb: &FakeFb,
    src: &[(Target, usize)],
    dst: &[(Target, usize)],
) -> Result<Vec<Op>, String> {
    let s = fb_runs(src).ok_or("a source piece is guest RAM")?;
    let d = fb_runs(dst).ok_or("a destination piece is guest RAM")?;
    let (mut si, mut so, mut di, mut doff) = (0usize, 0u64, 0usize, 0u64);
    let mut ops = Vec::new();
    while si < s.len() && di < d.len() {
        let n = (s[si].1 - so).min(d[di].1 - doff).min(MAX_OP_BYTES);
        let src = run_dva(w, fb, s[si].0 + so, n)?;
        let dst = run_dva(w, fb, d[di].0 + doff, n)?;
        ops.push(Op::Copy { src, dst, len: n });
        so += n;
        doff += n;
        if so == s[si].1 {
            si += 1;
            so = 0;
        }
        if doff == d[di].1 {
            di += 1;
            doff = 0;
        }
        if ops.len() > MAX_OPS {
            return Err(format!("more than {MAX_OPS} pieces"));
        }
    }
    if si != s.len() || di != d.len() {
        return Err("source and destination lengths differ".into());
    }
    Ok(ops)
}

/// The operations of a constant-remap fill of `dst` with elements of `elem_len` bytes, or
/// why it stays on the CPU (a run that splits an element: the engine fills whole ones).
pub fn plan_fill(
    w: &Window,
    fb: &FakeFb,
    dst: &[(Target, usize)],
    elem_len: u64,
    remap: Remap,
) -> Result<Vec<Op>, String> {
    let d = fb_runs(dst).ok_or("a destination piece is guest RAM")?;
    let mut ops = Vec::new();
    for (pa, len) in d {
        if elem_len == 0 || len % elem_len != 0 {
            return Err(format!(
                "a run of {len} bytes splits a {elem_len}-byte element"
            ));
        }
        let mut done = 0;
        while done < len {
            let n = (len - done).min(MAX_OP_BYTES / elem_len * elem_len);
            let dst = run_dva(w, fb, pa + done, n)?;
            ops.push(Op::Fill {
                dst,
                elements: n / elem_len,
                remap,
            });
            done += n;
            if ops.len() > MAX_OPS {
                return Err(format!("more than {MAX_OPS} pieces"));
            }
        }
    }
    Ok(ops)
}

// clc5b5.h (TURING_DMA_COPY_A).
const SET_SEMAPHORE_A: u32 = 0x240;
const LAUNCH_DMA: u32 = 0x300;
const OFFSET_IN_UPPER: u32 = 0x400;
const PITCH_IN: u32 = 0x410;
const SET_REMAP_CONST_A: u32 = 0x700;
const LAUNCH_NON_PIPELINED: u32 = 2;
const LAUNCH_FLUSH: u32 = 1 << 2;
const LAUNCH_SEM_ONE_WORD: u32 = 1 << 3;
const LAUNCH_SRC_PITCH: u32 = 1 << 7;
const LAUNCH_DST_PITCH: u32 = 1 << 8;
const LAUNCH_REMAP: u32 = 1 << 10;
/// The subchannel the device binds its copy object to.
const SUBCH: u32 = 4;

fn hdr(method: u32, count: u32) -> u32 {
    (1 << 29) | (count << 16) | (SUBCH << 13) | (method >> 2)
}

/// The push of `ops` ending in a one-word semaphore release of `seq` at `sem_va`.
pub fn push_words(ops: &[Op], copy_class: u32, sem_va: u64, seq: u32) -> Vec<u32> {
    let mut p = vec![hdr(0, 1), copy_class];
    for (i, op) in ops.iter().enumerate() {
        let last = i + 1 == ops.len();
        let (src, dst, len, fill) = match *op {
            Op::Copy { src, dst, len } => (src, dst, len as u32, None),
            Op::Fill {
                dst,
                elements,
                remap,
            } => (0, dst, elements as u32, Some(remap)),
        };
        p.extend([
            hdr(OFFSET_IN_UPPER, 4),
            (src >> 32) as u32,
            src as u32,
            (dst >> 32) as u32,
            dst as u32,
        ]);
        p.extend([hdr(PITCH_IN, 4), len, len, len, 1]);
        let mut launch = LAUNCH_NON_PIPELINED | LAUNCH_SRC_PITCH | LAUNCH_DST_PITCH;
        if let Some(r) = fill {
            p.extend([
                hdr(SET_REMAP_CONST_A, 3),
                r.const_a,
                r.const_b,
                r.components,
            ]);
            launch |= LAUNCH_REMAP;
        }
        if last {
            p.extend([
                hdr(SET_SEMAPHORE_A, 3),
                (sem_va >> 32) as u32,
                sem_va as u32,
                seq,
            ]);
            launch |= LAUNCH_FLUSH | LAUNCH_SEM_ONE_WORD;
        }
        p.extend([hdr(LAUNCH_DMA, 1), launch]);
    }
    p
}

/// The engine's private pages, below the invalidator's (the top 256 KiB of the reservation).
pub const PRIVATE_AT: u64 = 2 * crate::invalidate::CARVE_OUT;
const PD3: u64 = 0x0000;
const PD2: u64 = 0x1000;
const PD1: u64 = 0x2000;
const PD0_RING: u64 = 0x3000;
const PT_RING: u64 = 0x4000;
const GPFIFO: u64 = 0x5000;
const SEMAPHORE: u64 = 0x6000;
const USERD: u64 = 0x7000;
const PUSHBUF: u64 = 0x8000;
const PUSHBUF_PAGES: u64 = 8;
/// The PD0 pages of the FB window: 2 MiB PTEs, 512 MiB each, up to 16 GiB.
const PD0_FB: u64 = 0x10000;
const PD0_FB_PAGES: u64 = 32;
const _: () = assert!(PD0_FB + PD0_FB_PAGES * 0x1000 <= crate::invalidate::CARVE_OUT);
const GPFIFO_VA: u64 = 0x20_0000;
const SEMAPHORE_VA: u64 = 0x20_1000;
const PUSHBUF_VA: u64 = 0x20_2000;
/// The FB window's device VA (PD1 entry 8 of PD2 entry 0).
const WINDOW_VA: u64 = 1 << 32;
const GPFIFO_ENTRIES: u32 = 512;
const KIND_GENERIC: u64 = 0x06;
const VAS_IS_EXTERNALLY_OWNED: u32 = 1 << 3;
const SETPD_ALL_CHANNELS: u32 = 1 << 3;
const CMD_GPFIFO_SCHEDULE: u32 = 0xA06F_0103;
const CMD_GET_WORK_SUBMIT_TOKEN: u32 = 0xC36F_0108;
const DOORBELL_OFFSET: usize = 0x90;
const USERD_GP_GET: u64 = 0x88;
const USERD_GP_PUT: u64 = 0x8C;
const NV_ERR_INVALID_ADDRESS: u32 = 0x1E;

fn pde_vid(pa: u64) -> u64 {
    (1 << 1) | ((pa >> 12) << 8)
}

fn pte_vid(pa: u64) -> u64 {
    1 | ((pa >> 12) << 8) | (KIND_GENERIC << 56)
}

fn status_name(st: u32) -> String {
    match nvrm_abi::status_name(st) {
        Some(n) => format!("{st:#x} {n}"),
        None => format!("{st:#x}"),
    }
}

fn as_bytes<T>(p: &mut T) -> &mut [u8] {
    // SAFETY: plain-data RM parameter structs viewed as bytes for the raw escape.
    unsafe { std::slice::from_raw_parts_mut(p as *mut T as *mut u8, std::mem::size_of::<T>()) }
}

/// The device's own host copy engine.
pub struct DeviceCe {
    /// The private pages, in guest FB coordinates (the store's) and host VRAM (the GMMU's).
    at: u64,
    at_host: u64,
    window: Window,
    copy_class: u32,
    vas: u32,
    tsg: u32,
    channel: u32,
    token: u32,
    doorbell: Option<(u32, CpuMap)>,
    gp_put: u32,
    seq: u32,
    running: Option<(u32, Instant)>,
    off: Option<String>,
    pub pushes: u64,
    pub ops: u64,
    pub bytes: u64,
    pub busy_us: u128,
}

// SAFETY: the doorbell mapping is an MMIO pointer the device only uses under its own lock
// (the Mutex this engine lives in); nothing about it is tied to the creating thread.
unsafe impl Send for DeviceCe {}

impl DeviceCe {
    /// Build the engine: its page tables and ring in the private pages below the
    /// invalidator's, mapping the guest's usable FB `[guest.0, usable_end)` (host VRAM
    /// `[host.0, ...)`) at [`WINDOW_VA`]; `reservation` is the host handle of the VRAM.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        hm: &mut HostMemory,
        fb: &mut FakeFb,
        guest: (u64, u64),
        usable_end: u64,
        host: (u64, u64),
        reservation: u32,
        gpfifo_class: u32,
        copy_class: u32,
    ) -> Result<(Self, Vec<String>), String> {
        let (g0, g1) = guest;
        let (h0, h1) = host;
        if g1 - g0 != h1 - h0 || g1 - g0 <= crate::fb::CARVE_OUT {
            return Err("guest and host reservations differ in size or have no usable part".into());
        }
        // The window maps the guest's usable FB only: not the owned-GFXP slots, the BAR1 pool
        // or the carve-out above it (fb.rs `FbLayout::usable`).
        if usable_end <= g0 || usable_end > g1 - crate::fb::CARVE_OUT {
            return Err(format!(
                "usable end {usable_end:#x} outside [{g0:#x}, {:#x})",
                g1 - crate::fb::CARVE_OUT
            ));
        }
        let len = usable_end - g0;
        if len % (2 << 20) != 0 || len > PD0_FB_PAGES * (512 << 20) {
            return Err(format!(
                "usable FB of {len:#x} bytes: not 2 MiB aligned or above 16 GiB"
            ));
        }
        let at = g1 - PRIVATE_AT;
        let at_host = h1 - PRIVATE_AT;
        let to_host = |g: u64| at_host + (g - at);
        let pages = PD0_FB + PD0_FB_PAGES * 0x1000;
        for off in (0..pages).step_by(0x1000) {
            if !fb.write(at + off, &[0u8; 4096]) {
                return Err(format!("private page {:#x} not writable", at + off));
            }
        }
        let w64 = |fb: &mut FakeFb, pa: u64, val: u64| fb.write(pa, &val.to_le_bytes());
        w64(fb, at + PD3, pde_vid(to_host(at + PD2)));
        w64(fb, at + PD2, pde_vid(to_host(at + PD1)));
        // PD1 entry 0: the ring's PD0; entries 8.. : the FB window's PD0 pages.
        w64(fb, at + PD1, pde_vid(to_host(at + PD0_RING)));
        w64(fb, at + PD0_RING + 16 + 8, pde_vid(to_host(at + PT_RING)));
        w64(fb, at + PT_RING, pte_vid(to_host(at + GPFIFO)));
        w64(fb, at + PT_RING + 8, pte_vid(to_host(at + SEMAPHORE)));
        for k in 0..PUSHBUF_PAGES {
            w64(
                fb,
                at + PT_RING + 16 + 8 * k,
                pte_vid(to_host(at + PUSHBUF + 0x1000 * k)),
            );
        }
        let first_pd1 = WINDOW_VA >> 29;
        for off in (0..len).step_by(2 << 20) {
            let (pd0, slot) = (off >> 29, (off >> 21) & 0xFF);
            if slot == 0 {
                w64(
                    fb,
                    at + PD1 + 8 * (first_pd1 + pd0),
                    pde_vid(to_host(at + PD0_FB + 0x1000 * pd0)),
                );
            }
            // The 2 MiB page: the dual PDE's low qword is a PTE.
            w64(
                fb,
                at + PD0_FB + 0x1000 * pd0 + 16 * slot,
                pte_vid(h0 + off),
            );
        }
        fb.flush();

        let device = hm.device();
        let mut vp = sys::NV_VASPACE_ALLOCATION_PARAMETERS {
            flags: VAS_IS_EXTERNALLY_OWNED,
            vaSize: 1 << 40,
            bigPageSize: 64 * 1024,
            ..Default::default()
        };
        let vas = hm
            .alloc_object(
                device,
                sys::FERMI_VASPACE_A,
                as_bytes(&mut vp),
                "devce-vaspace",
            )
            .map_err(|st| format!("device CE FERMI_VASPACE_A: {}", status_name(st)))?;
        let mut sp = [0u8; 32];
        sp[0..8].copy_from_slice(&to_host(at + PD3).to_le_bytes());
        sp[8..12].copy_from_slice(&1u32.to_le_bytes());
        sp[12..16].copy_from_slice(&SETPD_ALL_CHANNELS.to_le_bytes());
        sp[16..20].copy_from_slice(&vas.to_le_bytes());
        sp[24..28].copy_from_slice(&1u32.to_le_bytes());
        let st = hm
            .control_admin(device, v::CMD_DMA_SET_PAGE_DIRECTORY, &mut sp)
            .map_err(|e| format!("device CE DMA_SET_PAGE_DIRECTORY: {e:#}"))?;
        if st != sys::NV_OK {
            let _ = hm.free_object(vas);
            return Err(format!(
                "device CE DMA_SET_PAGE_DIRECTORY: {}",
                status_name(st)
            ));
        }
        let engine_type = v::lookup(v::NV2080_ENGINE_TYPES, "COPY2").unwrap_or(0x0B);
        let mut tp = crate::hostabi::current().tsg_params(vas, engine_type);
        let tsg = match hm.alloc_object(device, sys::KEPLER_CHANNEL_GROUP_A, &mut tp, "devce-tsg") {
            Ok(h) => h,
            Err(st) => {
                let _ = hm.free_object(vas);
                return Err(format!(
                    "device CE KEPLER_CHANNEL_GROUP_A: {}",
                    status_name(st)
                ));
            }
        };
        let mut cp = crate::hostabi::ChannelAllocParams {
            gpFifoOffset: GPFIFO_VA,
            gpFifoEntries: GPFIFO_ENTRIES,
            ..Default::default()
        };
        cp.hUserdMemory[0] = reservation;
        cp.userdOffset[0] = to_host(at + USERD) - h0;
        // A user-level channel: the device's copies are VA-mode, no privileged method.
        let mut channel = Err(NV_ERR_INVALID_ADDRESS);
        for _ in 0..4 {
            channel = hm.alloc_object(tsg, gpfifo_class, as_bytes(&mut cp), "devce-channel");
            if !matches!(channel, Err(NV_ERR_INVALID_ADDRESS)) {
                break;
            }
            fb.flush();
        }
        let channel = match channel {
            Ok(h) => h,
            Err(st) => {
                let _ = hm.free_object(tsg);
                let _ = hm.free_object(vas);
                return Err(format!(
                    "device CE TURING_CHANNEL_GPFIFO_A: {}",
                    status_name(st)
                ));
            }
        };
        let fail = |hm: &mut HostMemory, what: String| {
            let _ = hm.free_object(channel);
            let _ = hm.free_object(tsg);
            let _ = hm.free_object(vas);
            Err(what)
        };
        let mut ce = [0u8; 8];
        ce[0..4].copy_from_slice(&1u32.to_le_bytes());
        ce[4..8].copy_from_slice(&engine_type.to_le_bytes());
        if let Err(st) = hm.alloc_object(channel, copy_class, &mut ce, "devce-ce") {
            return fail(
                hm,
                format!("device CE TURING_DMA_COPY_A: {}", status_name(st)),
            );
        }
        let mut sched = [1u8, 0, 0];
        let st = hm.control(channel, CMD_GPFIFO_SCHEDULE, &mut sched);
        if st != sys::NV_OK {
            return fail(
                hm,
                format!("device CE GPFIFO_SCHEDULE: {}", status_name(st)),
            );
        }
        let mut tok = [0u8; 4];
        let st = hm.control(channel, CMD_GET_WORK_SUBMIT_TOKEN, &mut tok);
        if st != sys::NV_OK {
            return fail(
                hm,
                format!("device CE GET_WORK_SUBMIT_TOKEN: {}", status_name(st)),
            );
        }
        let token = u32::from_le_bytes(tok);
        let doorbell = match hm.map_doorbell() {
            Ok(d) => d,
            Err(e) => return fail(hm, format!("device CE doorbell: {e:#}")),
        };
        fb.write(at + USERD + USERD_GP_GET, &0u32.to_le_bytes());
        fb.write(at + USERD + USERD_GP_PUT, &0u32.to_le_bytes());
        fb.write(at + SEMAPHORE, &0u32.to_le_bytes());
        fb.flush();
        let window = Window {
            guest_base: g0,
            len,
            dva: WINDOW_VA,
        };
        let notes = vec![format!(
            "device CE: channel {channel:#x} (TSG {tsg:#x} on COPY2, user level; VA space {vas:#x} rooted in the private pages [{at:#x}, {:#x})): guest FB [{g0:#x}, {:#x}) at device VA {WINDOW_VA:#x} (host VRAM [{h0:#x}, {:#x}), 2 MiB PTEs, kind 0x06), nothing else mapped; token {token:#x} (host chid {}) [host]",
            at + PRIVATE_AT / 2,
            g0 + len,
            h0 + len,
            token & 0xFFF
        )];
        Ok((
            DeviceCe {
                at,
                at_host,
                window,
                copy_class,
                vas,
                tsg,
                channel,
                token,
                doorbell: Some(doorbell),
                gp_put: 0,
                seq: 0,
                running: None,
                off: None,
                pushes: 0,
                ops: 0,
                bytes: 0,
                busy_us: 0,
            },
            notes,
        ))
    }

    /// Free the host objects; the engine refuses every push afterwards.
    pub fn free(&mut self, hm: &mut HostMemory) -> String {
        self.off = Some("freed".into());
        let Some((h, map)) = self.doorbell.take() else {
            return "device CE already freed".into();
        };
        drop(map);
        let mut r = Vec::new();
        for (what, handle) in [
            ("channel", self.channel),
            ("tsg", self.tsg),
            ("vaspace", self.vas),
            ("doorbell", h),
        ] {
            if let Err(st) = hm.free_object(handle) {
                r.push(format!("{what} {handle:#x}: {}", status_name(st)));
            }
        }
        format!(
            "device CE freed after {} push(es), {} op(s), {} MiB, {} ms busy; {}",
            self.pushes,
            self.ops,
            self.bytes >> 20,
            self.busy_us / 1000,
            if r.is_empty() {
                "all NV_OK".into()
            } else {
                r.join(", ")
            }
        )
    }

    /// The exit-summary line.
    pub fn summary(&self) -> String {
        format!(
            "device CE: {} push(es), {} op(s), {} MiB moved, {} ms busy{}",
            self.pushes,
            self.ops,
            self.bytes >> 20,
            self.busy_us / 1000,
            self.off
                .as_ref()
                .map_or(String::new(), |w| format!("; off: {w}"))
        )
    }
}

impl Engine for DeviceCe {
    fn window(&self) -> Window {
        self.window
    }

    fn submit(&mut self, fb: &mut FakeFb, ops: &[Op]) -> Result<u32, String> {
        if let Some(why) = &self.off {
            return Err(format!("device CE off: {why}"));
        }
        if self.running.is_some() {
            return Err("device CE busy".into());
        }
        if ops.is_empty() || ops.len() > MAX_OPS {
            return Err(format!("{} operations (1..={MAX_OPS})", ops.len()));
        }
        self.seq = self.seq.wrapping_add(1).max(1);
        let words = push_words(ops, self.copy_class, SEMAPHORE_VA, self.seq);
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        if bytes.len() as u64 > PUSHBUF_PAGES * 0x1000 || !fb.write(self.at + PUSHBUF, &bytes) {
            return Err("device CE push does not fit its pushbuffer".into());
        }
        let e0 = (PUSHBUF_VA as u32) & !3;
        let e1 = ((PUSHBUF_VA >> 32) as u32 & 0xFF) | ((words.len() as u32) << 10);
        let idx = u64::from(self.gp_put % GPFIFO_ENTRIES);
        fb.write(self.at + GPFIFO + 8 * idx, &e0.to_le_bytes());
        fb.write(self.at + GPFIFO + 8 * idx + 4, &e1.to_le_bytes());
        self.gp_put = (self.gp_put + 1) % GPFIFO_ENTRIES;
        fb.write(self.at + USERD + USERD_GP_PUT, &self.gp_put.to_le_bytes());
        fb.flush();
        let Some((_, map)) = self.doorbell.as_ref() else {
            return Err("device CE freed".into());
        };
        map.write_u32(DOORBELL_OFFSET, self.token);
        self.running = Some((self.seq, Instant::now()));
        self.pushes += 1;
        self.ops += ops.len() as u64;
        self.bytes += ops
            .iter()
            .map(|o| match *o {
                Op::Copy { len, .. } => len,
                Op::Fill {
                    elements, remap, ..
                } => {
                    elements
                        * ((((remap.components >> 16) & 3) + 1)
                            * (((remap.components >> 24) & 3) + 1)) as u64
                }
            })
            .sum::<u64>();
        Ok(self.seq)
    }

    fn poll(&mut self, fb: &mut FakeFb, seq: u32) -> Poll {
        let Some((running, since)) = self.running else {
            return Poll::Failed(format!("device CE push {seq} is not running"));
        };
        if running != seq {
            return Poll::Failed(format!("device CE runs push {running}, not {seq}"));
        }
        let mut sem = [0u8; 4];
        fb.read(self.at + SEMAPHORE, &mut sem);
        if u32::from_le_bytes(sem) == seq {
            self.running = None;
            self.busy_us += since.elapsed().as_micros();
            return Poll::Done;
        }
        if since.elapsed() < TIMEOUT {
            return Poll::Pending;
        }
        let mut get = [0u8; 4];
        fb.read(self.at + USERD + USERD_GP_GET, &mut get);
        let why = format!(
            "push {seq} not done in {} s (GPGet {} GPPut {}, host chid {}); the software CE stays on the CPU",
            TIMEOUT.as_secs(),
            u32::from_le_bytes(get),
            self.gp_put,
            self.token & 0xFFF
        );
        self.running = None;
        self.off = Some(why.clone());
        eprintln!("vfio-user-nvgpu: device CE: {why}");
        Poll::Failed(why)
    }

    fn off(&self) -> Option<String> {
        self.off.clone()
    }
}

impl DeviceCe {
    /// The host VRAM address of the private pages (for the notes).
    pub fn private_host(&self) -> u64 {
        self.at_host
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: Window = Window {
        guest_base: 0,
        len: 96 << 20,
        dva: WINDOW_VA,
    };

    fn remap() -> Remap {
        // One 4-byte component, the constant A (the Windows scrubber's fill).
        Remap {
            const_a: 0xAABB_CCDD,
            const_b: 0,
            components: 0x0003_0004,
        }
    }

    /// Condition 1 of the review: the window maps only the guest's usable FB; a piece that
    /// crosses its edge (into the carve-out) or lies beyond it, or a piece of guest RAM, is
    /// refused before anything reaches the host.
    #[test]
    fn a_piece_crossing_the_window_edge_is_refused() {
        let fb = FakeFb::new(128 << 20);
        let edge = W.guest_base + W.len;
        assert!(W.dva(edge - 0x1000, 0x1000).is_some());
        assert_eq!(W.dva(edge - 0x1000, 0x2000), None, "crosses the edge");
        assert_eq!(W.dva(edge, 0x1000), None, "beyond it");
        let crossing = [(Target::Fb(edge - 0x10000), 0x20000usize)];
        let e = plan_fill(&W, &fb, &crossing, 4, remap()).unwrap_err();
        assert!(e.contains("outside the device CE window"), "{e}");
        let inside = [(Target::Fb(0x10000), 0x10000usize)];
        assert!(plan_copy(&W, &fb, &crossing, &[(Target::Fb(0x10000), 0x20000)]).is_err());
        assert!(plan_copy(&W, &fb, &inside, &[(Target::Sys(0x4000_0000), 0x10000)]).is_err());
        assert!(plan_fill(&W, &fb, &[(Target::Sys(0x4000_0000), 0x10000)], 4, remap()).is_err());
    }

    /// Pieces are merged where contiguous, a copy is split where either side breaks, a fill
    /// counts elements, and a run that splits an element stays on the CPU.
    #[test]
    fn plans_merge_split_and_count() {
        let fb = FakeFb::new(128 << 20);
        let dst: Vec<(Target, usize)> = (0..16)
            .map(|k| (Target::Fb(0x100_0000 + 0x1000 * k), 0x1000))
            .collect();
        let ops = plan_fill(&W, &fb, &dst, 4, remap()).unwrap();
        assert_eq!(
            ops,
            vec![Op::Fill {
                dst: WINDOW_VA + 0x100_0000,
                elements: 0x4000,
                remap: remap()
            }]
        );
        let src = [
            (Target::Fb(0x200_0000), 0x3000usize),
            (Target::Fb(0x300_0000), 0x1000),
        ];
        let dst = [
            (Target::Fb(0x400_0000), 0x1000usize),
            (Target::Fb(0x500_0000), 0x3000),
        ];
        assert_eq!(
            plan_copy(&W, &fb, &src, &dst).unwrap(),
            vec![
                Op::Copy {
                    src: WINDOW_VA + 0x200_0000,
                    dst: WINDOW_VA + 0x400_0000,
                    len: 0x1000
                },
                Op::Copy {
                    src: WINDOW_VA + 0x200_1000,
                    dst: WINDOW_VA + 0x500_0000,
                    len: 0x2000
                },
                Op::Copy {
                    src: WINDOW_VA + 0x300_0000,
                    dst: WINDOW_VA + 0x500_2000,
                    len: 0x1000
                },
            ]
        );
        assert!(plan_fill(&W, &fb, &[(Target::Fb(0), 0x1000)], 3, remap()).is_err());
    }

    /// The push: SET_OBJECT, per operation offsets, pitch/line and (fill) the remap, and
    /// only the last launch releases the semaphore after a flush.
    #[test]
    fn the_push_encodes_the_operations() {
        let ops = [
            Op::Copy {
                src: 0x1_0000_1000,
                dst: 0x1_0000_2000,
                len: 0x1000,
            },
            Op::Fill {
                dst: 0x1_0000_3000,
                elements: 16,
                remap: remap(),
            },
        ];
        let p = push_words(&ops, 0xC5B5, SEMAPHORE_VA, 7);
        assert_eq!(&p[..2], &[hdr(0, 1), 0xC5B5]);
        assert_eq!(&p[2..7], &[hdr(OFFSET_IN_UPPER, 4), 1, 0x1000, 1, 0x2000]);
        assert_eq!(&p[7..12], &[hdr(PITCH_IN, 4), 0x1000, 0x1000, 0x1000, 1]);
        assert_eq!(&p[12..14], &[hdr(LAUNCH_DMA, 1), 0x182]);
        let launches: Vec<u32> = p
            .windows(2)
            .filter(|w| w[0] == hdr(LAUNCH_DMA, 1))
            .map(|w| w[1])
            .collect();
        assert_eq!(
            launches,
            vec![
                0x182,
                0x182 | LAUNCH_REMAP | LAUNCH_FLUSH | LAUNCH_SEM_ONE_WORD
            ]
        );
        let sem = p
            .windows(4)
            .find(|w| w[0] == hdr(SET_SEMAPHORE_A, 3))
            .unwrap();
        assert_eq!(sem, &[hdr(SET_SEMAPHORE_A, 3), 0, SEMAPHORE_VA as u32, 7]);
        // The software CE's own decoder reads the same launch fields.
        let v = launches[1];
        assert_eq!(
            (v & 3, (v >> 3) & 3, (v >> 10) & 1, (v >> 12) & 3),
            (2, 1, 1, 0)
        );
        assert!(MAX_OPS * 16 * 4 <= (PUSHBUF_PAGES * 0x1000) as usize);
    }
}

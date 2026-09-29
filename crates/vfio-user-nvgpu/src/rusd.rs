// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The RM user shared data page (RUSD, class 0x00DE `NV00DE_SHARED_DATA`, 615 cl00de.h) as
//! GSP-RM keeps it: `NV2080_CTRL_CMD_INTERNAL_INIT_USER_SHARED_DATA` (bInit, physAddr) hands
//! GSP the page, which GSP then fills and keeps fresh; readers copy a section and accept it
//! only by its leading 64-bit marker (`RUSD_READ_DATA`, `RUSD_IS_DATA_STALE`): a value at or
//! above `RUSD_SEQ_START` is the sequence counter of a non-polled section (odd while a write
//! is open), below it the nanosecond timestamp of a polled section (0 invalid, `u64::MAX`
//! write in progress). The device only recorded the page, so every section read invalid.
//!
//! With `CARAXES_RUSD=1` the device writes the page at bInit=1 and rewrites the polled
//! sections' timestamps (the device clock, as PTIMER reads it) at every GSP RPC while the
//! page is live, and stops at bInit=0. Values: the BAR1 and PMA sizes of the emulated card,
//! the card's clocks, P0, 40 C. Offsets from compiling cl00de.h of 615.71.09 (size 3184).

use crate::guestmem::GuestMemory;

pub const ENV: &str = "CARAXES_RUSD";
pub const SIZE: usize = 3184;
pub const SEQ_START: u64 = 0xFF00_0000_0000_0000;

/// Non-polled sections (marker = sequence counter).
const BAR1: usize = 0;
const PMA: usize = 16;
const SHADOW_ERR_CONT: usize = 40;
const GR_INFO: usize = 56;
/// Polled sections (marker = timestamp), offset of each marker.
const POLLED: [usize; 15] = [
    72,   // clkPublicDomainInfos
    112,  // clkThrottleInfo
    1992, // perfDevUtil
    2040, // memEcc
    2280, // perfCurrentPstate
    2296, // powerLimitGpu
    2392, // memRowRemap
    2440, // memBankRemap
    2472, // avgPowerUsage
    2496, // instPowerUsage
    2520, // powerPolicyStatus
    2536, // pciBusData
    2584, // fanCoolerStatus
    2656, // procUtil
    2312, // temperatures[0] (GPU); the others below
];
const TEMPS: usize = 2312;
const TEMP_STRIDE: usize = 16;
const CLK: usize = 72;

/// The emulated card's facts the page carries.
#[derive(Clone, Copy, Debug)]
pub struct Facts {
    pub bar1_kib: u32,
    pub bar1_avail_kib: u32,
    pub pma_total: u64,
    pub pma_free: u64,
}

#[derive(Default)]
pub struct Rusd {
    pa: Option<u64>,
    poll_mask: u64,
    refreshes: u64,
}

pub fn enabled() -> bool {
    std::env::var(ENV).is_ok_and(|v| v == "1")
}

impl Rusd {
    pub fn pa(&self) -> Option<u64> {
        self.pa
    }

    /// bInit=1: fill the page at `pa`; bInit=0 (`pa` 0): stop.
    pub fn init(&mut self, mem: &GuestMemory, pa: u64, now_ns: u64, f: &Facts) -> String {
        if pa == 0 {
            self.pa = None;
            return format!("rusd: stopped after {} refreshes", self.refreshes);
        }
        let mut b = vec![0u8; SIZE];
        let p64 = |b: &mut [u8], o: usize, v: u64| b[o..o + 8].copy_from_slice(&v.to_le_bytes());
        let p32 = |b: &mut [u8], o: usize, v: u32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        // Non-polled: written once, even sequence counters.
        for o in [BAR1, PMA, SHADOW_ERR_CONT, GR_INFO] {
            p64(&mut b, o, SEQ_START + 2);
        }
        p32(&mut b, BAR1 + 8, f.bar1_kib);
        p32(&mut b, BAR1 + 12, f.bar1_avail_kib);
        p64(&mut b, PMA + 8, f.pma_total);
        p64(&mut b, PMA + 16, f.pma_free);
        // Polled: the card's clocks (graphics, memory, video, SM; target MHz, actual kHz).
        for (i, mhz) in [1410u32, 7000, 1245, 1410].into_iter().enumerate() {
            p32(&mut b, CLK + 8 + 8 * i, mhz);
            p32(&mut b, CLK + 12 + 8 * i, mhz * 1000);
        }
        // Temperatures: s24.8, GPU and board 40 C; the others unsupported (timestamp 0).
        p32(&mut b, TEMPS + 8, 40 << 8);
        p32(&mut b, TEMPS + 2 * TEMP_STRIDE + 8, 40 << 8);
        let ok = mem.write(pa, &b);
        self.pa = ok.then_some(pa);
        self.refresh(mem, now_ns);
        format!(
            "rusd: page at {pa:#x} written ({SIZE} bytes, BAR1 {} KiB, PMA {:#x}/{:#x}){}",
            f.bar1_kib,
            f.pma_total,
            f.pma_free,
            if ok { "" } else { ": NOT MAPPED" }
        )
    }

    pub fn set_poll(&mut self, mask: u64) {
        self.poll_mask = mask;
    }

    /// The polled sections' timestamps: now. The temperatures of the memory, power supply and
    /// HBM sensors stay invalid (0).
    pub fn refresh(&mut self, mem: &GuestMemory, now_ns: u64) {
        let Some(pa) = self.pa else { return };
        let ts = now_ns.clamp(1, SEQ_START - 1).to_le_bytes();
        for o in POLLED {
            mem.write(pa + o as u64, &ts);
        }
        mem.write(pa + (TEMPS + 2 * TEMP_STRIDE) as u64, &ts);
        self.refreshes += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_markers_follow_the_header_rules() {
        use std::io::Write;
        let path = std::env::temp_dir().join(format!("nvgpu-rusd-{}", std::process::id()));
        let mut file = std::fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        file.write_all(&[0u8; 0x1000]).unwrap();
        let mut mem = GuestMemory::new();
        mem.map(0x10_0000, 0x1000, 0, &file).unwrap();
        let mut r = Rusd::default();
        let f = Facts { bar1_kib: 0x4_0000, bar1_avail_kib: 0x3_9F80, pma_total: 1 << 30, pma_free: 1 << 29 };
        let note = r.init(&mem, 0x10_0000, 5_000_000, &f);
        assert!(!note.contains("NOT MAPPED"), "{note}");
        let rd = |o: usize| mem.read_u64(0x10_0000 + o as u64).unwrap();
        // Non-polled: even sequence counters at or above SEQ_START.
        for o in [BAR1, PMA, SHADOW_ERR_CONT, GR_INFO] {
            assert!(rd(o) >= SEQ_START && rd(o) & 1 == 0);
        }
        assert_eq!(mem.read_u32(0x10_0000 + BAR1 as u64 + 8), Some(0x4_0000));
        // Polled: a valid timestamp (below SEQ_START, not 0).
        for o in POLLED {
            assert_eq!(rd(o), 5_000_000, "{o}");
        }
        r.refresh(&mem, 7_000_000);
        assert_eq!(rd(CLK), 7_000_000);
        assert_eq!(mem.read_u32(0x10_0000 + CLK as u64 + 8), Some(1410));
        // The memory temperature stays invalid.
        assert_eq!(rd(TEMPS + TEMP_STRIDE), 0);
        assert!(r.init(&mem, 0, 0, &f).contains("stopped"));
        r.refresh(&mem, 9_000_000);
        assert_eq!(rd(CLK), 7_000_000, "no refresh after bInit=0");
        std::fs::remove_file(path).ok();
    }
}

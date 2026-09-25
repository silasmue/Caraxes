// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! A cache of forwarded answers that are static per card (docs/caraxes/coverage-map.md §4
//! item 8).
//!
//! `nvidia-smi` and libnvidia-ml ask the same host-forwarded getters on every run (each
//! `nvidia-smi -q` process opens its own RM client), and every call is a round trip through
//! the guest's GSP queue, the device and the host RM. For a getter whose host answer cannot
//! change while the VM lives (a capability, a static table, a fixed identity or topology
//! fact) the first answer is the answer: [`CachingHost`] keeps it, keyed by the host object's
//! class, the control id and the request bytes, filled on first use and kept for the
//! device's lifetime.
//!
//! Only the controls of [`STATIC`] are cached, each with the reason from its header text.
//! Everything else passes through: live telemetry (temperatures, power, utilisation,
//! clocks now, counters, samples, the current pstate and PCIe link state), settings the
//! host can change at run time (clock offsets, TDP, LTR, ECC and DRAM-encryption
//! configuration, the illumination value, the scheduler state), and the GSS-legacy and
//! BINAPI commands, whose layouts and meaning no open header gives. The GSS-legacy perf
//! 0x79 behind nvidia-smi -q's 21 ms (678 ms on the first call) is one of those: its
//! answer moves between runs (docs/history/OPEN-QUESTIONS-2026-09-18.md:5917), so it
//! stays live.
//!
//! Only answers NV_OK, NV_ERR_NOT_SUPPORTED and NV_ERR_OBJECT_NOT_FOUND are kept: they are
//! the card's statement about itself. A transient status (busy, timeout, state in use, an
//! ioctl failure) is passed on and asked again next time. `CARAXES_FORWARD_CACHE=0` turns
//! the cache off. The cache holds at most [`MAX_ENTRIES`] answers of [`MAX_BYTES`] in all,
//! so a guest varying its request bytes cannot grow it without bound; past that, calls
//! pass through.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use crate::ctrl::HostRm;

/// The switch: `0` turns the cache off; anything else, or unset, leaves it on.
pub const FORWARD_CACHE_ENV: &str = "CARAXES_FORWARD_CACHE";

/// Answers the cache keeps at most.
pub const MAX_ENTRIES: usize = 1024;
/// Request and answer bytes the cache keeps at most.
pub const MAX_BYTES: usize = 4 << 20;

/// `NV_ERR_OBJECT_NOT_FOUND` (nvstatuscodes.h): an InfoROM object the board does not have.
const NV_ERR_OBJECT_NOT_FOUND: u32 = 0x57;

/// The host-forwarded getters whose answer is static per card, with the reason. Header
/// paths are the 615.71.09 tree's `src/common/sdk/nvidia/inc/ctrl/`.
pub const STATIC: &[(u32, &str, &str)] = &[
    // Identity and board facts.
    (0x0080_0294, "GPU_GET_BRAND_CAPS", "brand capabilities of the board (ctrl0080gpu.h)"),
    (0x2080_013F, "GPU_GET_OEM_BOARD_INFO", "board build date, part numbers, serial (ctrl2080gpu.h), written at manufacture"),
    (0x2080_01A4, "GPU_GET_CHIP_DETAILS", "chip SKU, major and minor revision (ctrl2080gpu.h:4289-4294); the device writes its own pciDevId over the host answer each time"),
    (0x2080_014B, "GPU_GET_INFOROM_OBJECT_VERSION", "InfoROM object versions (ctrl2080gpu.h:1780-1814); no InfoROM write is forwarded"),
    (0x2080_0156, "GPU_GET_INFOROM_IMAGE_VERSION", "InfoROM image version (ctrl2080gpu.h)"),
    (0x2080_0809, "BIOS_GET_POST_TIME", "the VBIOS POST time of the host's last boot (ctrl2080bios.h)"),
    (0x2080_2609, "PMGR_GET_MODULE_INFO", "module id on the baseboard and NVSwitch support (ctrl2080pmgr.h:39-66)"),
    // Capabilities.
    (0x2080_0157, "GPU_QUERY_INFOROM_ECC_SUPPORT", "whether the InfoROM has an ECC object (ctrl2080gpu.h)"),
    (0x2080_1357, "FB_QUERY_DRAM_ENCRYPTION_INFOROM_SUPPORT", "whether the InfoROM supports DRAM encryption (ctrl2080fb.h)"),
    (0x2080_0153, "GPU_QUERY_ILLUM_SUPPORT", "whether an illumination attribute exists (ctrl2080gpu.h:1870-1889); the value itself (GET_ILLUM) stays live"),
    (0x2080_1829, "BUS_GET_PCIE_REQ_ATOMICS_CAPS", "PCIe requester atomics capabilities (ctrl2080bus.h)"),
    (0x2080_1830, "BUS_GET_PCIE_CPL_ATOMICS_CAPS", "PCIe completer atomics capabilities (ctrl2080bus.h)"),
    (0x2080_182A, "BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS", "GPU atomics supported over PCIe (ctrl2080bus.h)"),
    (0x2080_1122, "FIFO_OBJSCHED_GET_CAPS", "supported scheduling policies and timeslice limits (ctrl2080fifo.h:991-1048); the state (GET_STATE) stays live"),
    (0x2080_3118, "FLCN_GET_ENGINE_ARCH", "Falcon or RISC-V per engine (ctrl2080flcn.h:128-154)"),
    (0x2080_014D, "GPU_GET_IP_VERSION", "a unit's IP_VER register (ctrl2080gpu.h:1836-1868)"),
    // Static topology and floorsweeping.
    (0x2080_81F3, "GPU_IS_RESET_COUPLED", "whether the GPU shares a reset with another (ctrl2080gpu.h)"),
    (0x2080_1328, "FB_GET_LTC_INFO_FOR_FBP", "the LTCs of an FBP (ctrl2080fb.h)"),
    (0x2080_1346, "FB_GET_FS_INFO", "FBP, LTC and L2 floorsweeping masks (ctrl2080fb.h:2340-2362)"),
    (0x2080_3801, "GRMGR_GET_GR_FS_INFO", "GPC and TPC floorsweeping (ctrl2080grmgr.h); no MIG on TU106 to repartition it"),
    (0x2080_1236, "GR_GET_GPC_TILE_MAP", "the GPC/TPC tile map (ctrl2080gr.h:1865-1884)"),
    (0x0080_170E, "FIFO_GET_LATENCY_BUFFER_SIZE", "GP and PB entries per engine (ctrl0080fifo.h:189-215)"),
];

/// Whether `cmd` is cached.
pub fn is_static(cmd: u32) -> bool {
    STATIC.iter().any(|&(c, _, _)| c == cmd)
}

/// Whether the switch leaves the cache on (default on).
pub fn enabled_by_env() -> bool {
    std::env::var(FORWARD_CACHE_ENV).map_or(true, |v| v.trim() != "0")
}

/// Device-lifetime counters, for the profile's window rows and the device log.
pub static HITS: AtomicU64 = AtomicU64::new(0);
pub static FILLS: AtomicU64 = AtomicU64::new(0);
/// Calls of a cached control that were not kept (a transient status, or the cache full).
pub static UNKEPT: AtomicU64 = AtomicU64::new(0);

/// The profile's window rows (profile.rs `register_source`).
pub fn counters() -> Vec<(&'static str, u64)> {
    vec![
        ("forward cache: hits", HITS.load(Relaxed)),
        ("forward cache: fills", FILLS.load(Relaxed)),
        ("forward cache: not kept", UNKEPT.load(Relaxed)),
    ]
}

type CacheKey = (u32, u32, Vec<u8>);

/// A [`HostRm`] that answers the [`STATIC`] getters from the first host answer.
pub struct CachingHost {
    inner: Box<dyn HostRm>,
    enabled: bool,
    map: HashMap<CacheKey, (u32, Vec<u8>)>,
    bytes: usize,
}

impl CachingHost {
    pub fn new(inner: Box<dyn HostRm>, enabled: bool) -> Self {
        CachingHost {
            inner,
            enabled,
            map: HashMap::new(),
            bytes: 0,
        }
    }

    /// Wrap `inner` with the cache on or off as `CARAXES_FORWARD_CACHE` says.
    pub fn from_env(inner: Box<dyn HostRm>) -> Self {
        Self::new(inner, enabled_by_env())
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Answers held.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl HostRm for CachingHost {
    fn control(&mut self, class: u32, cmd: u32, params: &mut [u8]) -> u32 {
        if !self.enabled || !is_static(cmd) {
            return self.inner.control(class, cmd, params);
        }
        let key = (class, cmd, params.to_vec());
        if let Some((status, answer)) = self.map.get(&key) {
            params.copy_from_slice(answer);
            HITS.fetch_add(1, Relaxed);
            return *status;
        }
        let status = self.inner.control(class, cmd, params);
        let keep = matches!(
            status,
            crate::ver::active::NV_OK
                | crate::ver::active::NV_ERR_NOT_SUPPORTED
                | NV_ERR_OBJECT_NOT_FOUND
        );
        let size = 2 * params.len();
        if keep && self.map.len() < MAX_ENTRIES && self.bytes + size <= MAX_BYTES {
            self.bytes += size;
            self.map.insert(key, (status, params.to_vec()));
            FILLS.fetch_add(1, Relaxed);
        } else {
            UNKEPT.fetch_add(1, Relaxed);
        }
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrl::{Dispatcher, INTERNAL_CLIENT, INTERNAL_SUBDEVICE};
    use crate::ver::active::{self as v, FORWARD, LEGACY, NV20_SUBDEVICE_0, NV_OK};
    use std::sync::{Arc, Mutex};

    /// A host whose every answer differs from the last: a counter in the first four bytes
    /// of the answer, bumped per call. It records the calls.
    struct Ticking(Arc<Mutex<Vec<u32>>>, u32, u32);
    impl HostRm for Ticking {
        fn control(&mut self, _: u32, cmd: u32, params: &mut [u8]) -> u32 {
            self.0.lock().unwrap().push(cmd);
            self.1 += 1;
            let n = params.len().min(4);
            params[..n].copy_from_slice(&self.1.to_le_bytes()[..n]);
            self.2
        }
    }

    fn host(status: u32) -> (CachingHost, Arc<Mutex<Vec<u32>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        (
            CachingHost::new(Box::new(Ticking(seen.clone(), 0, status)), true),
            seen,
        )
    }

    /// Every cached control is one the device forwards to the host (a FORWARD row, or
    /// one of the two identity forwards of nvml.rs and ctrl.rs), none twice.
    #[test]
    fn every_cached_control_is_a_forward() {
        let mut ids: Vec<u32> = STATIC.iter().map(|s| s.0).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), STATIC.len());
        for &(id, name, why) in STATIC {
            assert!(!why.is_empty(), "{name}");
            let forwarded = FORWARD.iter().any(|(c, _)| *c == id)
                || id == v::CMD_GPU_GET_CHIP_DETAILS
                || id == v::CMD_GPU_GET_OEM_BOARD_INFO;
            assert!(forwarded, "{name} ({id:#x}) is not forwarded");
        }
    }

    /// A static getter reaches the host once per request; the second call is answered
    /// from the first answer, and another request is asked again.
    #[test]
    fn a_static_getter_is_asked_once_per_request() {
        let (mut h, seen) = host(NV_OK);
        let id = 0x2080_014B; // GPU_GET_INFOROM_OBJECT_VERSION
        let mut a = *b"OEM\0\0";
        let mut b = a;
        assert_eq!(h.control(NV20_SUBDEVICE_0, id, &mut a), NV_OK);
        assert_eq!(h.control(NV20_SUBDEVICE_0, id, &mut b), NV_OK);
        assert_eq!(a, b);
        assert_eq!(seen.lock().unwrap().len(), 1);
        let mut c = *b"ECC\0\0";
        h.control(NV20_SUBDEVICE_0, id, &mut c);
        assert_eq!(seen.lock().unwrap().len(), 2);
        assert_eq!(h.len(), 2);
    }

    /// No live getter is ever cached: every FORWARD row outside [`STATIC`], every
    /// GSS-legacy command (perf 0x79 among them) and the named live telemetry reach the
    /// host on every call, and each call sees the host's new answer.
    #[test]
    fn a_live_getter_is_never_cached() {
        let live_named = [
            0x2080_2068u32, // PERF_GET_CURRENT_PSTATE
            0x2080_2087,    // PERF_GET_VID_ENG_PERFMON_SAMPLE
            0x2080_1813,    // BUS_GET_PEX_COUNTERS
            0x2080_1819,    // BUS_GET_PEX_UTIL_COUNTERS
            0x2080_1823,    // BUS_GET_INFO_V2: the current link generation and width
            0x2080_1037,    // CLK_PMUMON_CLK_DOMAINS_GET_SAMPLES
            0x2080_0154,    // GPU_GET_ILLUM
            0x2080_200B,    // PERF_GET_LEVEL_INFO_V2: clock offsets move it
            0x2080_3401,    // ECC_GET_VOLATILE_COUNTS
            0x2080_0513,    // THERMAL_SYSTEM_EXECUTE_V2
        ];
        let live = FORWARD
            .iter()
            .map(|(c, _)| *c)
            .filter(|c| !is_static(*c))
            .chain(LEGACY.iter().map(|(c, _, _)| *c))
            .chain(live_named);
        let mut n = 0;
        for id in live {
            assert!(!is_static(id), "{id:#x} is live and cached");
            let (mut h, seen) = host(NV_OK);
            let mut a = [0u8; 16];
            let mut b = [0u8; 16];
            h.control(NV20_SUBDEVICE_0, id, &mut a);
            h.control(NV20_SUBDEVICE_0, id, &mut b);
            assert_eq!(seen.lock().unwrap().len(), 2, "{id:#x}");
            assert_ne!(a, b, "{id:#x}: the second call saw the first answer");
            assert!(h.is_empty(), "{id:#x}");
            n += 1;
        }
        assert!(n > 40, "{n}");
        assert!(!is_static(0x2080_A079), "GSS-legacy perf 0x79 moves between runs");
    }

    /// `CARAXES_FORWARD_CACHE=0` (the switch, here as `enabled = false`): every call goes
    /// to the host.
    #[test]
    fn the_switch_turns_it_off() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut h = CachingHost::new(Box::new(Ticking(seen.clone(), 0, NV_OK)), false);
        for _ in 0..3 {
            h.control(NV20_SUBDEVICE_0, 0x0080_0294, &mut [0u8; 8]);
        }
        assert_eq!(seen.lock().unwrap().len(), 3);
        assert!(h.is_empty() && !h.enabled());
    }

    /// A transient status is passed on and asked again; NOT_SUPPORTED, the card's own
    /// answer, is kept.
    #[test]
    fn only_the_cards_own_statuses_are_kept() {
        let (mut h, seen) = host(0x0000_0065); // NV_ERR_TIMEOUT
        h.control(NV20_SUBDEVICE_0, 0x2080_0157, &mut []);
        h.control(NV20_SUBDEVICE_0, 0x2080_0157, &mut []);
        assert_eq!(seen.lock().unwrap().len(), 2);
        let (mut h, seen) = host(0xFFFF_FFFF); // the ioctl failed
        h.control(NV20_SUBDEVICE_0, 0x2080_0157, &mut []);
        h.control(NV20_SUBDEVICE_0, 0x2080_0157, &mut []);
        assert_eq!(seen.lock().unwrap().len(), 2);
        let (mut h, seen) = host(v::NV_ERR_NOT_SUPPORTED);
        assert_eq!(h.control(NV20_SUBDEVICE_0, 0x2080_0157, &mut []), 0x56);
        assert_eq!(h.control(NV20_SUBDEVICE_0, 0x2080_0157, &mut []), 0x56);
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    /// The cache is bounded: past [`MAX_ENTRIES`] distinct requests, calls pass through.
    #[test]
    fn the_cache_is_bounded() {
        let (mut h, seen) = host(NV_OK);
        for i in 0..(MAX_ENTRIES as u32 + 10) {
            let mut p = i.to_le_bytes();
            h.control(NV20_SUBDEVICE_0, 0x2080_0156, &mut p);
        }
        assert_eq!(h.len(), MAX_ENTRIES);
        let mut p = (MAX_ENTRIES as u32 + 5).to_le_bytes();
        h.control(NV20_SUBDEVICE_0, 0x2080_0156, &mut p);
        assert_eq!(seen.lock().unwrap().len(), MAX_ENTRIES + 11);
    }

    /// Through the dispatcher: the second nvidia-smi-shaped call of a cached getter does
    /// not reach the host, and a live one does, each time.
    #[test]
    fn through_the_dispatcher() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let h = CachingHost::new(Box::new(Ticking(seen.clone(), 0, NV_OK)), true);
        let f = crate::fixture::rtx2070();
        let mut d = Dispatcher::new(f.chip.data(), &f.identity(4 << 30), Some(Box::new(h)));
        for _ in 0..3 {
            let a = d.control(INTERNAL_CLIENT, INTERNAL_SUBDEVICE, 0x2080_1829, &[0u8; 4]);
            assert_eq!(a.status, NV_OK);
            assert_eq!(a.params, 1u32.to_le_bytes());
            d.control(INTERNAL_CLIENT, INTERNAL_SUBDEVICE, 0x2080_2068, &[0u8; 8]);
        }
        let s = seen.lock().unwrap();
        assert_eq!(s.iter().filter(|&&c| c == 0x2080_1829).count(), 1);
        assert_eq!(s.iter().filter(|&&c| c == 0x2080_2068).count(), 3);
    }
}

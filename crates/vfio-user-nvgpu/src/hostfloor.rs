// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The host VRAM floor (docs/caraxes/host-resources.md): every host allocation the device
//! makes outside the VM's reservation passes [`HostGuard::admit`] first, in
//! `HostMemory::alloc_object`, `alloc_object_admin` and `map_dma_fixed` (hostmem.rs).
//!
//! - **Floor.** The host keeps at least `CARAXES_HOST_VRAM_FLOOR_MIB` (default
//!   [`DEFAULT_FLOOR_MIB`], `0` turns the floor off) of free VRAM. The free size is the RM's
//!   `NV2080_CTRL_CMD_FB_GET_INFO_V2` `HEAP_FREE` (read-only, the same query the reservation
//!   rule makes at start), cached for [`FREE_TTL`] so the check costs a clock read per
//!   allocation. Below the floor, or when the free size cannot be read, the allocation is
//!   refused with `NV_ERR_NO_MEMORY` before it reaches the RM (fail closed).
//! - **Back-off.** An RM answer of `NV_ERR_NO_MEMORY` to an allocation or a map
//!   ([`HostGuard::note_status`]) refuses every further host allocation for [`BACKOFF`]
//!   without an RM call and drops the cached free size: the device tries once and never
//!   retries into an exhausted RM (2026-09-25: 11439 host log lines in three bursts of
//!   refused VA allocations while the desktop froze).
//! - **One log line.** The first refusal is logged, then at most one line per
//!   [`LOG_EVERY`] with the count since; the counters are complete ([`GuardStats`]).
//!
//! The reservation at start is checked by the same floor (`HostMemory::reserve`): refused
//! when it would leave the host less than the floor (or the older 1 GiB headroom rule,
//! whichever is larger).

use std::cell::Cell;
use std::time::{Duration, Instant};

/// The floor in MiB; `0` turns it off.
pub const FLOOR_ENV: &str = "CARAXES_HOST_VRAM_FLOOR_MIB";
/// The default floor: 768 MiB, the host desktop's working headroom on the 8 GiB card
/// beside a 3 GiB game VM and its own 2.1 GiB (2026-09-25).
pub const DEFAULT_FLOOR_MIB: u64 = 768;
/// How long a read free size stands.
pub const FREE_TTL: Duration = Duration::from_millis(250);
/// How long an RM `NV_ERR_NO_MEMORY` stops further host allocations.
pub const BACKOFF: Duration = Duration::from_secs(1);
/// At most one refusal line per this interval (after the first).
pub const LOG_EVERY: Duration = Duration::from_secs(10);
/// `NV_ERR_NO_MEMORY` (nvstatuscodes.h), what a refused allocation returns.
pub const NO_MEMORY: u32 = 0x51;
/// `NV_ERR_INSUFFICIENT_RESOURCES`: the RM's answer to a refused VA allocation
/// (virtual_mem.c `virtmemAllocResources` converts every `vaspaceAlloc` failure to it).
pub const INSUFFICIENT_RESOURCES: u32 = 0x1A;

const MIB: u64 = 1 << 20;

/// Whether an RM status means the host ran out of memory (not a VA collision or a bad
/// argument): the statuses after which the device stops instead of splitting or retrying.
pub fn is_exhaustion(st: u32) -> bool {
    st == NO_MEMORY
}

/// The floor in bytes from the variable's value (`None`: unset, the default).
pub fn parse_floor(v: Option<&str>) -> Result<u64, String> {
    match v.map(str::trim) {
        None | Some("") => Ok(DEFAULT_FLOOR_MIB * MIB),
        Some(s) => s
            .parse::<u64>()
            .ok()
            .filter(|m| *m <= 1 << 20)
            .map(|m| m * MIB)
            .ok_or_else(|| format!("{FLOOR_ENV}={s:?}: a size in MiB (0 turns the floor off)")),
    }
}

/// The floor from the environment; a malformed value is reported and the default used.
pub fn floor_from_env() -> u64 {
    match parse_floor(std::env::var(FLOOR_ENV).ok().as_deref()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("vfio-user-nvgpu: ERROR: {e}; using the default {DEFAULT_FLOOR_MIB} MiB");
            DEFAULT_FLOOR_MIB * MIB
        }
    }
}

/// Counters of the guard, for the soak line and the exit summary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuardStats {
    /// Allocations admitted.
    pub admitted: u64,
    /// Refused because the host's free VRAM was below the floor.
    pub below_floor: u64,
    /// Refused because the free size could not be read.
    pub unknown_free: u64,
    /// Refused during a back-off after an RM `NV_ERR_NO_MEMORY`.
    pub backoff: u64,
    /// RM answers of `NV_ERR_NO_MEMORY` seen.
    pub rm_no_memory: u64,
    /// Free-size reads made (the cache's misses).
    pub reads: u64,
}

impl GuardStats {
    pub fn refused(&self) -> u64 {
        self.below_floor + self.unknown_free + self.backoff
    }
}

/// The floor check with its cached free size, the back-off and the log limiter.
pub struct HostGuard {
    floor: u64,
    cached: Cell<Option<(Instant, Option<u64>)>>,
    backoff_until: Cell<Option<Instant>>,
    last_log: Cell<Option<Instant>>,
    unlogged: Cell<u64>,
    stats: Cell<GuardStats>,
    /// Tests: collect the lines instead of printing them.
    #[cfg(test)]
    pub lines: std::cell::RefCell<Vec<String>>,
}

impl HostGuard {
    pub fn new(floor: u64) -> Self {
        HostGuard {
            floor,
            cached: Cell::new(None),
            backoff_until: Cell::new(None),
            last_log: Cell::new(None),
            unlogged: Cell::new(0),
            stats: Cell::new(GuardStats::default()),
            #[cfg(test)]
            lines: Default::default(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(floor_from_env())
    }

    /// The floor in bytes (0: off).
    pub fn floor(&self) -> u64 {
        self.floor
    }

    pub fn stats(&self) -> GuardStats {
        self.stats.get()
    }

    fn bump(&self, f: impl FnOnce(&mut GuardStats)) {
        let mut s = self.stats.get();
        f(&mut s);
        self.stats.set(s);
    }

    /// The free size, from the cache while it is younger than [`FREE_TTL`].
    fn free(&self, now: Instant, read: impl FnOnce() -> Option<u64>) -> Option<u64> {
        if let Some((at, f)) = self.cached.get() {
            if now.saturating_duration_since(at) < FREE_TTL {
                return f;
            }
        }
        let f = read();
        self.bump(|s| s.reads += 1);
        self.cached.set(Some((now, f)));
        f
    }

    /// The last free size read, if any (for the soak line; no RM call).
    pub fn last_free(&self) -> Option<u64> {
        self.cached.get().and_then(|(_, f)| f)
    }

    /// Admit a host allocation of `what` (`read` gives the host's free VRAM in bytes, or
    /// `None` when it cannot be read). `Err(NO_MEMORY)` refuses it; the caller fails the
    /// guest-visible operation.
    pub fn admit(&self, what: &str, now: Instant, read: impl FnOnce() -> Option<u64>) -> Result<(), u32> {
        if let Some(until) = self.backoff_until.get() {
            if now < until {
                self.bump(|s| s.backoff += 1);
                self.log(now, || format!("host VRAM: {what} refused: back-off after the host RM answered NO_MEMORY"));
                return Err(NO_MEMORY);
            }
            self.backoff_until.set(None);
        }
        if self.floor == 0 {
            self.bump(|s| s.admitted += 1);
            return Ok(());
        }
        match self.free(now, read) {
            Some(f) if f >= self.floor => {
                self.bump(|s| s.admitted += 1);
                Ok(())
            }
            Some(f) => {
                self.bump(|s| s.below_floor += 1);
                self.log(now, || {
                    format!(
                        "host VRAM: {what} refused: {} MiB free, below the floor of {} MiB",
                        f / MIB,
                        self.floor / MIB
                    )
                });
                Err(NO_MEMORY)
            }
            None => {
                self.bump(|s| s.unknown_free += 1);
                self.log(now, || format!("host VRAM: {what} refused: the host's free VRAM cannot be read (fail closed)"));
                Err(NO_MEMORY)
            }
        }
    }

    /// The RM's answer to an admitted allocation or map: `NV_ERR_NO_MEMORY` starts the
    /// back-off and drops the cached free size.
    pub fn note_status(&self, what: &str, st: u32, now: Instant) {
        if !is_exhaustion(st) {
            return;
        }
        self.bump(|s| s.rm_no_memory += 1);
        self.cached.set(None);
        self.backoff_until.set(Some(now + BACKOFF));
        self.log(now, || {
            format!(
                "host VRAM: the host RM answered NO_MEMORY to {what}; no host allocation for {} ms",
                BACKOFF.as_millis()
            )
        });
    }

    /// One line now if none was written in the last [`LOG_EVERY`], else count it.
    fn log(&self, now: Instant, line: impl FnOnce() -> String) {
        let due = self
            .last_log
            .get()
            .is_none_or(|t| now.saturating_duration_since(t) >= LOG_EVERY);
        if !due {
            self.unlogged.set(self.unlogged.get() + 1);
            return;
        }
        let skipped = self.unlogged.replace(0);
        self.last_log.set(Some(now));
        let s = self.stats.get();
        let text = format!(
            "ERROR: {}; {} refusal(s) so far ({} below the floor, {} unknown free, {} in back-off), {} NO_MEMORY answer(s){} [host protection, {FLOOR_ENV}]",
            line(),
            s.refused(),
            s.below_floor,
            s.unknown_free,
            s.backoff,
            s.rm_no_memory,
            if skipped > 0 { format!(", {skipped} event(s) since the last line") } else { String::new() }
        );
        #[cfg(test)]
        self.lines.borrow_mut().push(text.clone());
        eprintln!("vfio-user-nvgpu: {text}");
    }

    /// The check of the reservation at start: `size` more bytes held must leave the host
    /// `max(floor, headroom)` free. Returns the refusal text.
    pub fn check_reservation(&self, size: u64, free: u64, headroom: u64) -> Result<(), String> {
        let keep = self.floor.max(headroom);
        if size.saturating_add(keep) > free {
            return Err(format!(
                "refused: reserving {} MiB would leave the host {} MiB of VRAM, below the {} MiB it keeps (floor {} MiB, {FLOOR_ENV}; headroom {} MiB) (free {} MiB = min(HEAP_FREE, nvidia-smi); shortfall {} MiB); choose a smaller profile or free host VRAM",
                size / MIB,
                free.saturating_sub(size) / MIB,
                keep / MIB,
                self.floor / MIB,
                headroom / MIB,
                free / MIB,
                (size + keep - free).div_ceil(MIB)
            ));
        }
        Ok(())
    }

    /// One summary line.
    pub fn line(&self) -> String {
        let s = self.stats.get();
        format!(
            "host VRAM floor {} MiB{}: {} admitted, {} refused ({} below the floor, {} unknown free, {} in back-off), {} NO_MEMORY answer(s), {} free-size read(s)",
            self.floor / MIB,
            if self.floor == 0 { " (off)" } else { "" },
            s.admitted,
            s.refused(),
            s.below_floor,
            s.unknown_free,
            s.backoff,
            s.rm_no_memory,
            s.reads
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1 << 30;

    #[test]
    fn the_floor_parses_mib_and_zero_turns_it_off() {
        assert_eq!(parse_floor(None), Ok(768 * MIB));
        assert_eq!(parse_floor(Some("")), Ok(768 * MIB));
        assert_eq!(parse_floor(Some("1024")), Ok(GIB));
        assert_eq!(parse_floor(Some("0")), Ok(0));
        assert!(parse_floor(Some("-1")).is_err());
        assert!(parse_floor(Some("lots")).is_err());
    }

    #[test]
    fn below_the_floor_is_refused_and_the_free_size_is_cached() {
        let g = HostGuard::new(768 * MIB);
        let t = Instant::now();
        let reads = Cell::new(0);
        let free = Cell::new(2 * GIB);
        let read = || {
            reads.set(reads.get() + 1);
            Some(free.get())
        };
        assert_eq!(g.admit("channel", t, read), Ok(()));
        // Inside the TTL the cached size stands: no read, even though the host changed.
        free.set(431 * MIB);
        for i in 0..100 {
            assert_eq!(g.admit("map", t + Duration::from_millis(i), read), Ok(()));
        }
        assert_eq!(reads.get(), 1);
        // After the TTL the new size refuses (the 2026-09-25 incident's 431 MiB).
        assert_eq!(g.admit("map", t + FREE_TTL, read), Err(NO_MEMORY));
        assert_eq!(reads.get(), 2);
        let s = g.stats();
        assert_eq!((s.admitted, s.below_floor, s.reads), (101, 1, 2));
        // The host frees memory again: admitted after the next read.
        free.set(900 * MIB);
        assert_eq!(g.admit("map", t + FREE_TTL * 2, read), Ok(()));
    }

    #[test]
    fn an_unreadable_free_size_refuses_and_zero_admits_everything() {
        let g = HostGuard::new(768 * MIB);
        assert_eq!(g.admit("vas", Instant::now(), || None), Err(NO_MEMORY));
        assert_eq!(g.stats().unknown_free, 1);
        let off = HostGuard::new(0);
        let t = Instant::now();
        assert_eq!(off.admit("vas", t, || panic!("no read with the floor off")), Ok(()));
    }

    #[test]
    fn no_memory_backs_off_once_and_logs_once() {
        let g = HostGuard::new(768 * MIB);
        let t = Instant::now();
        let read = || Some(4 * GIB);
        assert_eq!(g.admit("map", t, read), Ok(()));
        g.note_status("map", NO_MEMORY, t);
        // A storm of attempts inside the back-off: none reaches the RM, one line in all.
        for i in 1..1000 {
            assert_eq!(g.admit("map", t + Duration::from_micros(i), read), Err(NO_MEMORY));
        }
        assert_eq!(g.lines.borrow().len(), 1, "{:?}", g.lines.borrow());
        let s = g.stats();
        assert_eq!((s.rm_no_memory, s.backoff), (1, 999));
        // After the back-off one attempt goes through again (and re-reads the free size).
        assert_eq!(g.admit("map", t + BACKOFF, read), Ok(()));
        // Other statuses (a VA collision's INSUFFICIENT_RESOURCES) start nothing.
        g.note_status("vas", INSUFFICIENT_RESOURCES, t + BACKOFF);
        assert_eq!(g.admit("map", t + BACKOFF, read), Ok(()));
        // The next line comes only after LOG_EVERY, with the count of the silent ones.
        g.note_status("map", NO_MEMORY, t + LOG_EVERY + BACKOFF);
        let lines = g.lines.borrow();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("event(s) since the last line"), "{}", lines[1]);
    }

    #[test]
    fn the_reservation_keeps_the_larger_of_floor_and_headroom() {
        let g = HostGuard::new(768 * MIB);
        // 3 GiB profile with 4.5 GiB free: 1.5 GiB left, above both.
        assert!(g.check_reservation(3 * GIB, 4 * GIB + 512 * MIB, GIB).is_ok());
        // 3.5 GiB left 0.9 GiB: below the 1 GiB headroom.
        assert!(g.check_reservation(3 * GIB, 3 * GIB + 900 * MIB, GIB).is_err());
        // A floor above the headroom decides.
        let g = HostGuard::new(2 * GIB);
        let e = g.check_reservation(3 * GIB, 4 * GIB + 512 * MIB, GIB).unwrap_err();
        assert!(e.contains("floor 2048 MiB"), "{e}");
    }
}

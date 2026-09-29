// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Trapped status reads without the device lock (docs/caraxes/execution.md §9.10).
//!
//! Two trapped reads are polled by the guest RM after it started an operation, and on the
//! card they are plain status reads:
//! - `NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE` (TRIGGER, bit 31): the RM polls it until the
//!   invalidate it wrote is done (`kgmmuCheckPendingInvalidates_TU102`). SotTR run E: 507 reads
//!   a second at 152 us each, mostly waiting for the device lock.
//! - the BAR2 flush page (`pReadToFlush`, kern_bus_gv100.c:85-94, read by `kbusFlushSingle_GV100`
//!   with the value ignored): a dedicated RM page whose read only orders the RM's BAR2 writes.
//!   SotTR run E: 587 reads a second at 95 us each.
//!
//! The work those reads wait for runs elsewhere: the trigger write hands its held invalidate
//! off the lock (invgate.rs `Hold`), the BAR1 pool's slices run under the lock, and a flush
//! read only marks the BAR2 sync owed (bar2area.rs `DeferredFlush`). So a read needs only the
//! state those publish here, and the vfio-user server answers it before it takes the lock
//! (device.rs `SharedDevice::region_read`):
//! - TRIGGER: the latched word, busy while a handed-off hold's jobs are not applied (each read
//!   waits at most `POLL_WAIT` for the worker, past `FALLBACK` to the end, without the lock). A
//!   BAR1 pool batch still running, or the trace recording accesses, sends the read to the
//!   locked path, which continues the batch.
//! - The flush page: the owed sync is marked and the page's bytes as the last locked read of it
//!   found them. That copy goes whenever the page could change: a device write to its FB page,
//!   a BAR2 table walk that re-points pages, a reset.
//!
//! Lock order (execution.md §9): the mutexes here are leaves; nothing here takes the device
//! lock, and a device-lock holder only takes them for a state change.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::Duration;

use crate::bar2area::DeferredFlush;
use crate::invgate::{Hold, Target, FALLBACK, POLL_WAIT};

const NONE: u64 = u64::MAX;
const PAGE: u64 = 4096;
const TRIGGER: u32 = 1 << 31;

/// The flush page as the last locked read found it.
struct FlushPage {
    /// Its BAR2 page (offset, page aligned) and FB page (page number).
    bar2_page: u64,
    bytes: Box<[u8]>,
    defer: Arc<DeferredFlush>,
}

#[derive(Default)]
struct Counters {
    trigger_fast: AtomicU64,
    trigger_fast_busy: AtomicU64,
    trigger_locked: AtomicU64,
    flush_fast: AtomicU64,
    flush_locked: AtomicU64,
    flush_published: AtomicU64,
    flush_dropped: AtomicU64,
    held: AtomicU64,
    held_ns: AtomicU64,
    held_max_ns: AtomicU64,
}

/// The published state of one device model (see the module docs).
pub struct FastReads {
    trigger_off: AtomicU64,
    trigger_word: AtomicU32,
    pool_busy: AtomicBool,
    traced: AtomicBool,
    holds: Mutex<Vec<Hold>>,
    flush: Mutex<Option<FlushPage>>,
    flush_fb_page: AtomicU64,
    c: Counters,
}

impl Default for FastReads {
    fn default() -> Self {
        FastReads {
            trigger_off: AtomicU64::new(NONE),
            trigger_word: AtomicU32::new(0),
            pool_busy: AtomicBool::new(false),
            traced: AtomicBool::new(false),
            holds: Mutex::new(Vec::new()),
            flush: Mutex::new(None),
            flush_fb_page: AtomicU64::new(NONE),
            c: Counters::default(),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl FastReads {
    /// The trigger register's BAR0 offset and the word the guest last wrote to it.
    pub fn set_trigger(&self, off: u64, word: u32) {
        self.trigger_word.store(word & !TRIGGER, Ordering::Release);
        self.trigger_off.store(off, Ordering::Release);
    }

    /// A held invalidate the trigger write handed off the lock (invgate.rs `Hold`).
    pub fn push_hold(&self, h: Hold) {
        lock(&self.holds).push(h);
    }

    /// Whether a BAR1 pool batch is running: its slices need the locked path.
    pub fn set_pool_busy(&self, busy: bool) {
        self.pool_busy.store(busy, Ordering::Release);
    }

    /// Whether the trace records accesses: every read takes the locked path then.
    pub fn set_traced(&self, traced: bool) {
        self.traced.store(traced, Ordering::Release);
    }

    /// Whether TRIGGER reads busy: a handed-off hold whose jobs the worker has not applied.
    /// `bounded`: each hold is waited for at most `POLL_WAIT`, past `FALLBACK` to the end.
    /// A hold found done is finished (its roots stop being urgent) and counted.
    pub fn trigger_busy(&self, bounded: bool) -> bool {
        let parts: Vec<_> = lock(&self.holds).iter().map(|h| h.parts()).collect();
        if parts.is_empty() {
            return false;
        }
        for (progress, targets, since) in &parts {
            let t = Target::Roots(targets.clone());
            let done = if !bounded || since.elapsed() >= FALLBACK {
                progress.wait(&t);
                true
            } else {
                progress.wait_at_most(&t, POLL_WAIT)
            };
            if !done {
                return true;
            }
        }
        let finished: Vec<Hold> = {
            let mut g = lock(&self.holds);
            let (done, keep): (Vec<Hold>, Vec<Hold>) = g
                .drain(..)
                .partition(|h| parts.iter().any(|p| p.2 == h.since()));
            *g = keep;
            done
        };
        for h in finished {
            let d = h.wait();
            let ns = d.as_nanos() as u64;
            self.c.held.fetch_add(1, Ordering::Relaxed);
            self.c.held_ns.fetch_add(ns, Ordering::Relaxed);
            self.c.held_max_ns.fetch_max(ns, Ordering::Relaxed);
            crate::profile::record(
                crate::profile::Key::Window("held invalidate: armed to TRIGGER read done"),
                d,
                1,
                None,
                "",
            );
        }
        // A hold pushed while these were waited for keeps TRIGGER busy until its turn.
        !lock(&self.holds).is_empty()
    }

    /// The flush page as a locked read found it (BAR2 page `bar2_page`, FB page `fb_page`).
    pub fn publish_flush(&self, bar2_page: u64, fb_page: u64, bytes: Vec<u8>, defer: Arc<DeferredFlush>) {
        *lock(&self.flush) = Some(FlushPage {
            bar2_page,
            bytes: bytes.into_boxed_slice(),
            defer,
        });
        self.flush_fb_page.store(fb_page, Ordering::Release);
        self.c.flush_published.fetch_add(1, Ordering::Relaxed);
    }

    /// Whether the copy of BAR2 page `bar2_page` is current.
    pub fn flush_published(&self, bar2_page: u64) -> bool {
        lock(&self.flush).as_ref().is_some_and(|f| f.bar2_page == bar2_page)
    }

    /// The flush page could change: its copy goes.
    pub fn drop_flush(&self) {
        if self.flush_fb_page.swap(NONE, Ordering::AcqRel) != NONE {
            self.c.flush_dropped.fetch_add(1, Ordering::Relaxed);
        }
        *lock(&self.flush) = None;
    }

    /// A device write to FB pages `first..=last` (fb.rs `FakeFb::note_write`): the flush
    /// page's copy goes when it is among them. One atomic load otherwise.
    pub fn note_fb_write(&self, first: u64, last: u64) {
        let p = self.flush_fb_page.load(Ordering::Acquire);
        if p != NONE && first <= p && p <= last {
            self.drop_flush();
        }
    }

    /// A device reset: nothing in flight (the worker was drained), no copy.
    pub fn reset(&self) {
        for h in lock(&self.holds).drain(..) {
            h.wait();
        }
        self.drop_flush();
        self.pool_busy.store(false, Ordering::Release);
    }

    /// Count a read of these registers the locked path answered.
    pub fn note_locked_trigger(&self) {
        self.c.trigger_locked.fetch_add(1, Ordering::Relaxed);
    }

    pub fn note_locked_flush(&self) {
        self.c.flush_locked.fetch_add(1, Ordering::Relaxed);
    }

    /// Answer a trapped read without the device lock when it is one of these registers and
    /// the published state suffices; false sends it to the locked path.
    pub fn try_read(&self, region: u32, offset: u64, data: &mut [u8]) -> bool {
        if self.traced.load(Ordering::Acquire) {
            return false;
        }
        match region {
            0 => {
                if data.len() != 4
                    || offset != self.trigger_off.load(Ordering::Acquire)
                    || self.pool_busy.load(Ordering::Acquire)
                {
                    return false;
                }
                let busy = self.trigger_busy(true);
                let v = self.trigger_word.load(Ordering::Acquire) | if busy { TRIGGER } else { 0 };
                data.copy_from_slice(&v.to_le_bytes());
                self.c.trigger_fast.fetch_add(1, Ordering::Relaxed);
                self.c.trigger_fast_busy.fetch_add(u64::from(busy), Ordering::Relaxed);
                true
            }
            3 => {
                let g = lock(&self.flush);
                let Some(f) = g.as_ref() else { return false };
                let at = offset.wrapping_sub(f.bar2_page);
                if at >= PAGE || at + data.len() as u64 > PAGE {
                    return false;
                }
                f.defer.mark();
                data.copy_from_slice(&f.bytes[at as usize..at as usize + data.len()]);
                self.c.flush_fast.fetch_add(1, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }

    /// The exit summary's line.
    pub fn summary(&self) -> String {
        let c = &self.c;
        let held = c.held.load(Ordering::Relaxed);
        format!(
            "reads without the device lock (execution.md §9.10): {} TRIGGER read(s) ({} busy), {} flush-page read(s); under the lock {} TRIGGER, {} flush-page; flush page published {} time(s), dropped {}; {} held invalidate(s) completed by a read (mean {} us, max {} us)",
            c.trigger_fast.load(Ordering::Relaxed),
            c.trigger_fast_busy.load(Ordering::Relaxed),
            c.flush_fast.load(Ordering::Relaxed),
            c.trigger_locked.load(Ordering::Relaxed),
            c.flush_locked.load(Ordering::Relaxed),
            c.flush_published.load(Ordering::Relaxed),
            c.flush_dropped.load(Ordering::Relaxed),
            held,
            c.held_ns.load(Ordering::Relaxed) / held.max(1) / 1000,
            c.held_max_ns.load(Ordering::Relaxed) / 1000
        )
    }
}

/// The published state of each device the vfio-user server serves, by the address of its
/// shared device (device.rs `SharedDevice`): looked up per trapped read, registered at the
/// first one.
static REGISTRY: Mutex<Option<HashMap<usize, Weak<FastReads>>>> = Mutex::new(None);

pub fn lookup(key: usize) -> Option<Arc<FastReads>> {
    lock(&REGISTRY).as_ref()?.get(&key)?.upgrade()
}

pub fn register(key: usize, f: &Arc<FastReads>) {
    let mut g = lock(&REGISTRY);
    let m = g.get_or_insert_with(HashMap::new);
    m.retain(|_, w| w.strong_count() > 0);
    m.insert(key, Arc::downgrade(f));
}

/// `POLL_WAIT` and `FALLBACK`, for the tests and the docs.
pub const READ_WAIT: Duration = POLL_WAIT;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invgate::Progress;
    use std::time::Instant;

    fn word(f: &FastReads, off: u64) -> Option<u32> {
        let mut b = [0u8; 4];
        f.try_read(0, off, &mut b).then(|| u32::from_le_bytes(b))
    }

    /// TRIGGER without the lock: the latched word, busy until the handed-off hold's job is
    /// applied, then idle; a pool batch or the access trace sends the read to the locked path.
    #[test]
    fn the_trigger_reads_busy_until_the_worker_applied_the_hold() {
        let f = FastReads::default();
        assert_eq!(word(&f, 0xb8_30b0), None, "nothing published");
        f.set_trigger(0xb8_30b0, 0x8000_0005);
        assert_eq!(word(&f, 0xb8_30b0), Some(5), "no hold: idle");
        assert_eq!(word(&f, 0xb8_30a0), None, "another register");
        let p = Arc::new(Progress::default());
        p.begin(7, 0xA000);
        f.push_hold(Hold::new(p.clone(), vec![(0xA000, 7)], Instant::now()));
        assert_eq!(word(&f, 0xb8_30b0), Some(TRIGGER | 5), "busy while job 7 runs");
        assert_eq!(p.urgent_roots(), vec![0xA000]);
        f.set_pool_busy(true);
        assert_eq!(word(&f, 0xb8_30b0), None, "a pool batch: the locked path");
        f.set_pool_busy(false);
        p.finish(7);
        assert_eq!(word(&f, 0xb8_30b0), Some(5), "applied: idle");
        assert!(p.urgent_roots().is_empty(), "the root is no longer urgent");
        f.set_traced(true);
        assert_eq!(word(&f, 0xb8_30b0), None);
        assert!(f.summary().contains("3 TRIGGER read(s) (1 busy)"));
        assert!(f.summary().contains("1 held invalidate(s) completed by a read"));
    }

    /// The flush page without the lock: the owed sync is marked and the published bytes
    /// answered; a device write to its FB page drops the copy (the locked path answers).
    #[test]
    fn the_flush_page_marks_the_sync_owed_and_answers_its_copy() {
        let f = FastReads::default();
        let d = Arc::new(DeferredFlush::default());
        let mut b = [0u8; 4];
        assert!(!f.try_read(3, 0xfe_a000, &mut b));
        let mut bytes = vec![0u8; 4096];
        bytes[4..8].copy_from_slice(&0x1234u32.to_le_bytes());
        f.publish_flush(0xfe_a000, 0x4_0123, bytes, d.clone());
        assert!(f.try_read(3, 0xfe_a004, &mut b));
        assert_eq!(u32::from_le_bytes(b), 0x1234);
        assert!(d.pending(), "the read marked the sync owed");
        assert!(!f.try_read(3, 0xfe_b000, &mut b), "another page");
        f.note_fb_write(0x4_0000, 0x4_0100);
        assert!(f.try_read(3, 0xfe_a000, &mut b), "a write elsewhere keeps the copy");
        f.note_fb_write(0x4_0120, 0x4_0123);
        assert!(!f.try_read(3, 0xfe_a000, &mut b), "a write to the page drops it");
    }
}

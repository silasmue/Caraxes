// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Mirror builds off the device lock (docs/caraxes/execution.md §9.2).
//!
//! A shadow channel's first doorbell builds its host mirror: the host TSG, the channel, its
//! USERD and error-notifier objects, the engine objects (the GR object makes the host RM build
//! the context), the owned GFXP bind and the preemption bind. These are host RM calls of up to
//! a second in total (Windows C61: 0.5-1 s under the device lock). The device takes a snapshot
//! of the build's inputs under the lock and hands the RM calls to the shadow worker as one
//! job, after the root's plans (so the set-A mirror is there before the channel's GPFIFO VA
//! is validated). The channel's ring is held until the build is committed under the lock
//! (the scheduling, the work-submit token and the doorbell); nothing else waits for it.
//!
//! This module holds the parts of that protocol that need no host: the host TSG shared by the
//! builds of one guest TSG in flight ([`TsgCell`]), the GPPut the commit writes back after the
//! host RM cleared the guest's USERD ([`put_to_restore`]), and the counters.

use std::sync::{Mutex, PoisonError};
use std::time::Duration;

/// The host TSG of one guest TSG while builds of its channels are in flight: the first build
/// the worker runs makes it (or finds the committed one), the later ones join it. A refused
/// allocation leaves the cell empty, so the next build tries again, as the synchronous build
/// did at the next doorbell.
#[derive(Debug, Default)]
pub struct TsgCell {
    pub handle: Option<u32>,
    /// The GR preemption check (preempt.rs, steps 0 and 1) ran on the TSG: at an earlier
    /// build, or claimed by a build in flight ([`claim_preempt_check`]).
    pub preempt_checked: bool,
}

impl TsgCell {
    /// The cell of a committed host TSG.
    pub fn committed(handle: u32, preempt_checked: bool) -> Self {
        TsgCell {
            handle: Some(handle),
            preempt_checked,
        }
    }
}

/// The build's host TSG, `(handle, made by this build)`: the cell's, or a new one from
/// `alloc` (on the worker, in the root's job order).
pub fn tsg_for_build(
    cell: &Mutex<TsgCell>,
    alloc: impl FnOnce() -> Result<u32, u32>,
) -> Result<(u32, bool), u32> {
    let mut c = cell.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(h) = c.handle {
        return Ok((h, false));
    }
    let h = alloc()?;
    c.handle = Some(h);
    Ok((h, true))
}

/// Whether this build runs the preemption check of its TSG: only the first build with a GR
/// object does, before its channel's first schedule.
pub fn claim_preempt_check(cell: &Mutex<TsgCell>) -> bool {
    let mut c = cell.lock().unwrap_or_else(PoisonError::into_inner);
    !std::mem::replace(&mut c.preempt_checked, true)
}

/// The GPPut the commit writes back into the guest's USERD, if any. The host RM zeroes a
/// sysmem USERD when it allocates the channel (`kfifoSetupUserD_HAL`, kernel_channel.c
/// 2435-2449 of 615.71.09), on the worker, while the guest may still store GPPut and ring.
/// `newest`: the newest GPPut the device read at a doorbell of the build (under the lock);
/// `now`: the USERD's GPPut at the commit. A zero there with a nonzero `newest` is the clear:
/// `newest` goes back. Any other value is the guest's own store after the clear (as new as
/// `newest` or newer; its doorbell follows) and stays. Before 2026-09-29 the build restored
/// the first doorbell's GPPut whenever the USERD held another value, which also overwrote a
/// newer store of the guest.
pub fn put_to_restore(newest: Option<u32>, now: Option<u32>) -> Option<u32> {
    match (newest, now) {
        (Some(n), Some(0)) if n != 0 => Some(n),
        _ => None,
    }
}

/// Counters for the exit summary.
#[derive(Clone, Debug, Default)]
pub struct BuildStats {
    pub started: u64,
    pub committed: u64,
    /// Builds the host refused (committed as a refusal).
    pub failed: u64,
    /// Builds taken down before their commit (their channel, TSG or VAS was freed, a reset).
    pub dropped: u64,
    /// Doorbells of a channel whose build was in flight (held with its first one).
    pub joined: u64,
    /// Builds committed at once because the job had finished already (no worker thread,
    /// `CARAXES_SHADOW_WORKER=0`, a preparation without submission).
    pub inline: u64,
    /// Builds the device waited for under the lock (a free of their objects, a preparation).
    pub waited: u64,
    pub waited_ns: u64,
    pub host_ns: u64,
    pub host_max_ns: u64,
    pub commit_ns: u64,
    pub commit_max_ns: u64,
    /// From the first doorbell to the commit.
    pub first_ring_max_ns: u64,
}

fn ms(ns: u64) -> u64 {
    ns / 1_000_000
}

impl BuildStats {
    pub fn host(&mut self, d: Duration) {
        let ns = d.as_nanos() as u64;
        self.host_ns += ns;
        self.host_max_ns = self.host_max_ns.max(ns);
    }

    pub fn commit(&mut self, d: Duration, since_first: Duration) {
        let ns = d.as_nanos() as u64;
        self.commit_ns += ns;
        self.commit_max_ns = self.commit_max_ns.max(ns);
        self.first_ring_max_ns = self.first_ring_max_ns.max(since_first.as_nanos() as u64);
    }

    pub fn line(&self, pending: usize) -> String {
        format!(
            "mirror builds (execution.md §9.2): {} started, {} committed ({} refused by the host), {} dropped before the commit, {} in flight; {} doorbell(s) joined a build in flight; host side {} ms on the worker (longest {} ms); commit under the lock {} ms (longest {} us); first doorbell to commit longest {} ms; {} committed at once, {} waited for under the lock ({} ms)",
            self.started,
            self.committed,
            self.failed,
            self.dropped,
            pending,
            self.joined,
            ms(self.host_ns),
            ms(self.host_max_ns),
            ms(self.commit_ns),
            self.commit_max_ns / 1000,
            ms(self.first_ring_max_ns),
            self.inline,
            self.waited,
            ms(self.waited_ns),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first build of a guest TSG makes the host TSG, the next ones join it; a refusal
    /// leaves the cell empty for the next build; a committed TSG is joined.
    #[test]
    fn builds_of_one_tsg_share_the_host_tsg_the_first_one_makes() {
        let cell = Mutex::new(TsgCell::default());
        let mut made = 0;
        assert_eq!(tsg_for_build(&cell, || Err(0x51)), Err(0x51));
        assert_eq!(cell.lock().unwrap().handle, None);
        let mut alloc = || {
            made += 1;
            Ok(0xcafe)
        };
        assert_eq!(tsg_for_build(&cell, &mut alloc), Ok((0xcafe, true)));
        assert_eq!(tsg_for_build(&cell, &mut alloc), Ok((0xcafe, false)));
        assert_eq!(made, 1);
        let committed = Mutex::new(TsgCell::committed(0xbeef, false));
        assert_eq!(tsg_for_build(&committed, || Ok(1)), Ok((0xbeef, false)));
    }

    /// Only the first build with a GR object checks the TSG's preemption; a TSG checked at an
    /// earlier build is not checked again.
    #[test]
    fn the_preemption_check_is_claimed_once_per_tsg() {
        let cell = Mutex::new(TsgCell::default());
        assert!(claim_preempt_check(&cell));
        assert!(!claim_preempt_check(&cell));
        let checked = Mutex::new(TsgCell::committed(1, true));
        assert!(!claim_preempt_check(&checked));
    }

    /// The commit writes the newest rung GPPut back only over the host RM's clear; a store
    /// of the guest after the clear stays, and so does a USERD that already holds it.
    #[test]
    fn the_commit_restores_only_what_the_clear_took() {
        assert_eq!(put_to_restore(Some(5), Some(0)), Some(5), "the clear took it");
        assert_eq!(put_to_restore(Some(5), Some(5)), None, "still there");
        assert_eq!(put_to_restore(Some(5), Some(9)), None, "a newer store after the clear");
        assert_eq!(put_to_restore(Some(0), Some(0)), None, "nothing rung");
        assert_eq!(put_to_restore(None, Some(0)), None, "unreadable at the doorbell");
        assert_eq!(put_to_restore(Some(5), None), None, "unreadable now");
    }

    #[test]
    fn the_summary_line_counts_builds() {
        let mut s = BuildStats {
            started: 3,
            committed: 2,
            ..Default::default()
        };
        s.host(Duration::from_millis(700));
        s.commit(Duration::from_micros(900), Duration::from_millis(710));
        let l = s.line(1);
        assert!(l.contains("3 started, 2 committed"), "{l}");
        assert!(l.contains("host side 700 ms"), "{l}");
        assert!(l.contains("longest 900 us"), "{l}");
        assert!(l.contains("1 in flight"), "{l}");
    }
}

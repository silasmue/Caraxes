// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The doorbell token pool (`CARAXES_DOORBELL_TOKENS=N`, docs/caraxes/doorbell-token-pool.md).
//!
//! The guest rings a channel by writing its work-submit token to the usermode doorbell
//! (`NVC361_NOTIFY_CHANNEL_PENDING`, BAR0 0xbb0090 on TU106). libcuda reads that token from
//! the channel's notifier entry `NV_CHANNELGPFIFO_NOTIFICATION_TYPE_WORK_SUBMIT_TOKEN`
//! (nvos.h:2855), and the device can write another one there, as the host RM already does
//! for a mirrored channel (mirror.rs `chid_of_host_token`). The pool is N device-chosen
//! tokens, each declared at start as an ioeventfd sub-region of the doorbell register with
//! KVM datamatch and its own eventfd (vfio-user.rst, "Sub-Region IO FD info format
//! (ioeventfd)"). A channel gets a pool token at its first doorbell, which traps and
//! carries the guest's own token; from then on each ring of that channel is a KVM exit
//! that ends in its eventfd, and the eventfd names the channel.
//!
//! Pool tokens are `TOKEN_BASE + slot`. `TOKEN_BASE` sets bits 31:23, which no
//! `NV_CTRL_VF_DOORBELL` value has (VECTOR 11:0, RUNLIST_ID 22:16, tu102/dev_ctrl.h:36-37),
//! so a pool token is never a guest or host token and the other way round. Slots are
//! handed out lowest first: KVM compares a written value against the ioeventfds of one
//! address in registration order, about 25 ns per entry passed
//! (caraxes/runs/doorbell-tokens-2026-09-24/kvm-ioeventfd-probe.txt), and slot 0 is
//! registered first.

use std::collections::BTreeMap;

/// The switch: `CARAXES_DOORBELL_TOKENS=N` offers N pool tokens.
pub const TOKENS_ENV: &str = "CARAXES_DOORBELL_TOKENS";

/// The first pool token. Bits 31:23 set: outside every `NV_CTRL_VF_DOORBELL` encoding.
pub const TOKEN_BASE: u32 = 0xCA70_0000;

/// At most as many tokens as one vfio-user reply can carry eventfds: one Unix socket
/// message carries at most `SCM_MAX_FD` descriptors (include/net/scm.h:18), and the
/// reply to `VFIO_USER_DEVICE_GET_REGION_IO_FDS` is one message.
pub const MAX_TOKENS: usize = vfio_user::SCM_MAX_FD as usize;

/// The pool size for `CARAXES_DOORBELL_TOKENS=on`: every value that misses the pool (a
/// first ring, a UVM channel's own token) passes all N entries before it exits, about
/// 25 ns each, so N stays near the number of channels a guest keeps.
pub const DEFAULT_TOKENS: usize = 64;

/// `NvNotification` is 16 bytes: timeStamp 0-7, info32 8-11, info16 12-13, status 14-15
/// (nvgputypes.h:57-64). The work-submit token is info32 of entry
/// `NV_CHANNELGPFIFO_NOTIFICATION_TYPE_WORK_SUBMIT_TOKEN` = 1 (nvos.h:2855), written by
/// `kchannelNotifyWorkSubmitToken` (kernel_channel.c:4015-4030).
pub const NOTIFIER_TOKEN_OFFSET: u64 = 16 + 8;

/// The pool size `CARAXES_DOORBELL_TOKENS` asks for: `None` when unset, empty or 0.
pub fn requested() -> Result<Option<usize>, String> {
    let Ok(v) = std::env::var(TOKENS_ENV) else {
        return Ok(None);
    };
    parse(&v)
}

fn parse(v: &str) -> Result<Option<usize>, String> {
    let v = v.trim();
    if v.is_empty() || v == "0" || v == "off" {
        return Ok(None);
    }
    if v == "on" {
        return Ok(Some(DEFAULT_TOKENS));
    }
    let n: usize = v
        .parse()
        .map_err(|_| format!("{TOKENS_ENV}={v:?}: a number of tokens, 1 to {MAX_TOKENS}, or on"))?;
    if n > MAX_TOKENS {
        return Err(format!(
            "{TOKENS_ENV}={n}: at most {MAX_TOKENS} (SCM_MAX_FD: the reply that hands the client one eventfd per token is one socket message)"
        ));
    }
    Ok(Some(n))
}

/// Whether `token` lies in the pool's value range (assigned or not).
pub fn is_pool_token(token: u32, len: usize) -> bool {
    token >= TOKEN_BASE && ((token - TOKEN_BASE) as usize) < len
}

/// Counters for the exit summary.
#[derive(Clone, Debug, Default)]
pub struct PoolStats {
    /// Pool tokens handed out (written into a notifier entry).
    pub handed_out: u64,
    /// Pool tokens written again over an entry rewritten since (the host RM's host token).
    pub rewritten: u64,
    /// Most tokens assigned at once.
    pub peak: usize,
    /// eventfd wakeups and the guest writes they counted.
    pub eventfd_rings: u64,
    pub eventfd_writes: u64,
    /// Pool tokens that arrived trapped (their ioeventfd was not registered).
    pub trapped_pool_rings: u64,
    /// Trapped rings with a guest or host token of a channel that already has a pool
    /// token (a ring that does not read the notifier, e.g. nvidia-uvm's channels).
    pub own_token_after_handout: u64,
    /// Pool tokens rung while unassigned: dropped.
    pub stale: u64,
    /// Channels left on their own token: no notifier, a notifier entry that did not
    /// hold the token just rung, or the pool exhausted.
    pub no_notifier: u64,
    pub mismatch: u64,
    pub exhausted: u64,
    /// The first few stale tokens and mismatches, for the log.
    pub first: Vec<String>,
}

impl PoolStats {
    fn remember(&mut self, s: String) {
        if self.first.len() < 16 {
            self.first.push(s);
        }
    }
}

/// N tokens, each free or held by one guest channel.
#[derive(Clone, Debug)]
pub struct TokenPool {
    slots: Vec<Option<u32>>,
    by_chid: BTreeMap<u32, usize>,
    pub stats: PoolStats,
}

impl TokenPool {
    pub fn new(n: usize) -> Self {
        TokenPool {
            slots: vec![None; n.min(MAX_TOKENS)],
            by_chid: BTreeMap::new(),
            stats: PoolStats::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The token of slot `slot`.
    pub fn token(slot: usize) -> u32 {
        TOKEN_BASE + slot as u32
    }

    pub fn contains(&self, token: u32) -> bool {
        is_pool_token(token, self.slots.len())
    }

    /// The channel holding `token`, if the token is in the pool and assigned.
    pub fn chid(&self, token: u32) -> Option<u32> {
        if !self.contains(token) {
            return None;
        }
        self.slots[(token - TOKEN_BASE) as usize]
    }

    /// The pool token `chid` holds.
    pub fn token_of(&self, chid: u32) -> Option<u32> {
        self.by_chid.get(&chid).map(|&s| Self::token(s))
    }

    /// Give `chid` the lowest free token (its own if it has one); `None` when the pool
    /// is exhausted.
    pub fn assign(&mut self, chid: u32) -> Option<u32> {
        if let Some(t) = self.token_of(chid) {
            return Some(t);
        }
        let slot = self.slots.iter().position(Option::is_none)?;
        self.slots[slot] = Some(chid);
        self.by_chid.insert(chid, slot);
        self.stats.peak = self.stats.peak.max(self.by_chid.len());
        Some(Self::token(slot))
    }

    /// `chid` is gone: its token is free again. Returns the token it held.
    pub fn release(&mut self, chid: u32) -> Option<u32> {
        let slot = self.by_chid.remove(&chid)?;
        self.slots[slot] = None;
        Some(Self::token(slot))
    }

    pub fn assigned(&self) -> usize {
        self.by_chid.len()
    }

    pub fn note_stale(&mut self, token: u32, how: &str) {
        self.stats.stale += 1;
        self.stats
            .remember(format!("stale token {token:#x} ({how})"));
    }

    pub fn note_refused(&mut self, why: String) {
        self.stats.remember(why);
    }

    /// One line for the exit summary.
    pub fn summary(&self) -> String {
        let s = &self.stats;
        format!(
            "doorbell tokens: {} offered ({TOKEN_BASE:#x}..), {} handed out, {} written again, {} assigned at exit (peak {}); {} eventfd wakeup(s) for {} guest write(s); {} pool token(s) trapped; {} trapped ring(s) with a channel's own token after its hand-out; {} stale; left on their own token: {} without notifier, {} notifier mismatch, {} pool exhausted{}",
            self.slots.len(),
            s.handed_out,
            s.rewritten,
            self.by_chid.len(),
            s.peak,
            s.eventfd_rings,
            s.eventfd_writes,
            s.trapped_pool_rings,
            s.own_token_after_handout,
            s.stale,
            s.no_notifier,
            s.mismatch,
            s.exhausted,
            if s.first.is_empty() {
                String::new()
            } else {
                format!(" (first: {})", s.first.join("; "))
            }
        )
    }
}

/// The switch for the guest-token ioeventfds (docs/caraxes/doorbell-token-pool.md (g)):
/// `CARAXES_DOORBELL_GUEST_TOKENS=<runlist>[,<runlist>...]:<chids>` declares, at start, one
/// datamatch sub-region of the doorbell register for every guest token `runlist << 16 |
/// chid` with the runlists in the order given and chids `0..chids`, each with its own
/// eventfd. A write of such a value then ends in its eventfd, and the device rings it
/// exactly as the trapped write of that value (ctrl.rs `doorbell`). For guests that ring
/// their own tokens and never read the work-submit-token notifier entry: the Windows KMD
/// computes its tokens itself (the pool's hand-out cannot reach it).
pub const GUEST_TOKENS_ENV: &str = "CARAXES_DOORBELL_GUEST_TOKENS";

/// `NV_CTRL_VF_DOORBELL` (tu102/dev_ctrl.h:36-37): VECTOR (the ChID) 11:0, RUNLIST_ID 22:16.
pub const TOKEN_VECTOR_BITS: u32 = 12;
pub const TOKEN_RUNLIST_SHIFT: u32 = 16;
pub const TOKEN_RUNLIST_MAX: u32 = 0x7F;

/// The guest token of `chid` on `runlist` (kfifoGenerateWorkSubmitTokenHal_TU102,
/// kernel_fifo_tu102.c:75-130).
pub fn guest_token(runlist: u32, chid: u32) -> u32 {
    (runlist << TOKEN_RUNLIST_SHIFT) | chid
}

/// The guest tokens declared as ioeventfds: `runlists` in registration order (KVM passes
/// the entries of one address in that order, about 25 ns each, so the busiest runlist
/// goes first), chids `0..chids` within each.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestTokens {
    pub runlists: Vec<u32>,
    pub chids: u32,
}

impl GuestTokens {
    /// `None` when `CARAXES_DOORBELL_GUEST_TOKENS` is unset, empty, 0 or off.
    pub fn requested() -> Result<Option<Self>, String> {
        match std::env::var(GUEST_TOKENS_ENV) {
            Ok(v) => Self::parse(&v),
            Err(_) => Ok(None),
        }
    }

    pub fn parse(v: &str) -> Result<Option<Self>, String> {
        let v = v.trim();
        if v.is_empty() || v == "0" || v == "off" {
            return Ok(None);
        }
        let bad = || format!("{GUEST_TOKENS_ENV}={v:?}: <runlist>[,<runlist>...]:<chids>, e.g. 0,1,2,3,7:48");
        let (rl, n) = v.split_once(':').ok_or_else(bad)?;
        let chids: u32 = n.trim().parse().map_err(|_| bad())?;
        let mut runlists = Vec::new();
        for r in rl.split(',') {
            let r: u32 = r.trim().parse().map_err(|_| bad())?;
            if r > TOKEN_RUNLIST_MAX || runlists.contains(&r) {
                return Err(format!("{GUEST_TOKENS_ENV}={v:?}: runlist {r} repeated or beyond RUNLIST_ID 22:16"));
            }
            runlists.push(r);
        }
        if chids == 0 || chids > 1 << TOKEN_VECTOR_BITS {
            return Err(format!("{GUEST_TOKENS_ENV}={v:?}: chids must be 1 to {} (VECTOR 11:0)", 1 << TOKEN_VECTOR_BITS));
        }
        let t = GuestTokens { runlists, chids };
        if t.len() > MAX_TOKENS {
            return Err(format!(
                "{GUEST_TOKENS_ENV}={v:?}: {} tokens, at most {MAX_TOKENS} (SCM_MAX_FD: one eventfd each, one reply)",
                t.len()
            ));
        }
        Ok(Some(t))
    }

    pub fn len(&self) -> usize {
        self.runlists.len() * self.chids as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The tokens in registration order: runlist-major, chids ascending.
    pub fn values(&self) -> Vec<u32> {
        self.runlists
            .iter()
            .flat_map(|&r| (0..self.chids).map(move |c| guest_token(r, c)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_values() {
        assert_eq!(parse(""), Ok(None));
        assert_eq!(parse("0"), Ok(None));
        assert_eq!(parse("off"), Ok(None));
        assert_eq!(parse("on"), Ok(Some(DEFAULT_TOKENS)));
        assert_eq!(parse(" 16 "), Ok(Some(16)));
        assert_eq!(parse("253"), Ok(Some(253)));
        assert!(parse("254").unwrap_err().contains("SCM_MAX_FD"));
        assert!(parse("256").is_err());
        assert!(parse("many").is_err());
    }

    #[test]
    fn pool_tokens_are_never_doorbell_encodings() {
        // NV_CTRL_VF_DOORBELL: VECTOR 11:0, RUNLIST_ID 22:16; everything a guest or host
        // RM generates lies below 1 << 23.
        let highest_encoding = (0x7F << 16) | 0xFFF;
        assert!(TOKEN_BASE > highest_encoding);
        assert!(TOKEN_BASE as usize + MAX_TOKENS < u32::MAX as usize);
        assert!(!is_pool_token(highest_encoding, MAX_TOKENS));
        assert!(!is_pool_token(0x8003a, MAX_TOKENS));
        assert!(is_pool_token(TOKEN_BASE, 1));
        assert!(!is_pool_token(TOKEN_BASE + 1, 1));
    }

    #[test]
    fn lowest_free_first_and_release() {
        let mut p = TokenPool::new(3);
        assert_eq!(p.assign(9), Some(TOKEN_BASE));
        assert_eq!(p.assign(2), Some(TOKEN_BASE + 1));
        // A channel keeps its token.
        assert_eq!(p.assign(9), Some(TOKEN_BASE));
        assert_eq!(p.assign(5), Some(TOKEN_BASE + 2));
        assert_eq!(p.assign(6), None, "exhausted");
        assert_eq!(p.chid(TOKEN_BASE + 1), Some(2));
        assert_eq!(p.token_of(5), Some(TOKEN_BASE + 2));
        assert_eq!(p.release(9), Some(TOKEN_BASE));
        assert_eq!(p.release(9), None);
        assert_eq!(p.chid(TOKEN_BASE), None, "released: a ring of it is stale");
        // The freed lowest slot is reused first (cheapest for KVM to match).
        assert_eq!(p.assign(6), Some(TOKEN_BASE));
        assert_eq!(p.assigned(), 3);
        assert_eq!(p.stats.peak, 3);
        // Outside the pool: never a channel.
        assert_eq!(p.chid(TOKEN_BASE + 3), None);
        assert_eq!(p.chid(2), None);
        assert!(p.summary().contains("3 offered"));
    }

    #[test]
    fn pool_is_bounded() {
        assert_eq!(TokenPool::new(1000).len(), MAX_TOKENS);
        assert_eq!(MAX_TOKENS, 253);
    }

    #[test]
    fn guest_tokens_parse_and_order() {
        use super::{guest_token, GuestTokens};
        assert_eq!(GuestTokens::parse("").unwrap(), None);
        assert_eq!(GuestTokens::parse("off").unwrap(), None);
        let t = GuestTokens::parse("0,1,7:3").unwrap().unwrap();
        assert_eq!(t.values(), vec![0, 1, 2, 0x10000, 0x10001, 0x10002, 0x70000, 0x70001, 0x70002]);
        assert_eq!(guest_token(1, 15), 0x1000f);
        assert!(GuestTokens::parse("0,0:4").is_err());
        assert!(GuestTokens::parse("128:4").is_err());
        assert!(GuestTokens::parse("0:0").is_err());
        assert!(GuestTokens::parse("0:4097").is_err());
        assert!(GuestTokens::parse("0,1,2,3,7,8:51").is_err(), "306 > SCM_MAX_FD");
        assert!(GuestTokens::parse("0,1,2,3,7:48").unwrap().unwrap().len() == 240);
        // Never a pool token: the pool's bits 31:23 lie above RUNLIST_ID 22:16.
        for v in GuestTokens::parse("0,127:1").unwrap().unwrap().values() {
            assert!(!super::is_pool_token(v, super::MAX_TOKENS));
        }
        assert!(!super::is_pool_token(guest_token(127, 4095), super::MAX_TOKENS));
    }
}

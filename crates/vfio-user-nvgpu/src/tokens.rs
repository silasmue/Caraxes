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

/// A client may keep its token at another notifier index
/// (`NVC36F_CTRL_CMD_GPFIFO_SET_WORK_SUBMIT_TOKEN_NOTIF_INDEX`, which the guest RM of a GSP
/// system handles alone, kernel_channel.c:3455-3496 of 615): the entries scanned for it.
pub const TOKEN_SCAN_ENTRIES: usize = 64;

/// The status `kchannelNotifyWorkSubmitToken` writes with a token (kernel_channel.c
/// :4359-4372 of 615): IN_PROGRESS (15:15) TRUE and VALUE (14:0) 0x7FFF; info16 is 0.
pub const TOKEN_ENTRY_STATUS: u16 = 0xFFFF;

/// The notifier entries (index, from 2) of a 16-byte `NvNotification` array `buf` that
/// hold one of `tokens` as the RM writes a work-submit token: info32 the token, info16 0,
/// status [`TOKEN_ENTRY_STATUS`]. Entries 0 (errors) and 1 (the default index) are not
/// scanned.
pub fn token_entries(buf: &[u8], tokens: &[u32]) -> Vec<usize> {
    (2..buf.len() / 16)
        .filter(|&i| {
            let e = &buf[16 * i..16 * i + 16];
            let info32 = u32::from_le_bytes(e[8..12].try_into().unwrap());
            let info16 = u16::from_le_bytes(e[12..14].try_into().unwrap());
            let status = u16::from_le_bytes(e[14..16].try_into().unwrap());
            info16 == 0 && status == TOKEN_ENTRY_STATUS && tokens.contains(&info32)
        })
        .collect()
}

/// A notifier page: in FB (true) or guest RAM, and the page number.
pub type NotifierPage = (bool, u64);

/// Whether a value that is `chid`'s token (the one it rang with, or its mirror's host
/// token) is only `chid`'s where it stands on `chid`'s notifier page `page`: no other
/// channel a ring of it rings (`targets`, ctrl.rs `doorbell_targets`) keeps its notifier on
/// that page, so no other channel's token entry there can hold the same value (a guest
/// token that is also a mirrored channel's host token, both channels on one notifier
/// page). `page_of(c)` is channel `c`'s notifier page, `None` for a channel without one
/// the guest announced.
pub fn own_on_page(
    chid: u32,
    targets: &[u32],
    page: NotifierPage,
    page_of: impl Fn(u32) -> Option<NotifierPage>,
) -> bool {
    targets
        .iter()
        .filter(|&&c| c != chid)
        .all(|&c| page_of(c) != Some(page))
}

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
    /// Pool tokens written into a token entry at another index than the default one.
    pub elsewhere: u64,
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
            "doorbell tokens: {} offered ({TOKEN_BASE:#x}..), {} handed out, {} written again, {} assigned at exit (peak {}); {} eventfd wakeup(s) for {} guest write(s); {} pool token(s) trapped; {} trapped ring(s) with a channel's own token after its hand-out; {} written into the token entry at another notifier index; {} stale; left on their own token: {} without notifier, {} notifier mismatch, {} pool exhausted{}",
            self.slots.len(),
            s.handed_out,
            s.rewritten,
            self.by_chid.len(),
            s.peak,
            s.eventfd_rings,
            s.eventfd_writes,
            s.trapped_pool_rings,
            s.own_token_after_handout,
            s.elsewhere,
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

/// The switch for the guest-token ioeventfds (docs/caraxes/doorbell-token-pool.md (g)).
/// Unset or `auto`: the set is derived from the engines the device offers
/// ([`GuestTokens::auto`]); `off` or `0`: none; for an A/B,
/// `<hot>[,<runlist>...]:<chids>` with the hot runlists joined by `+` (e.g. `0+1,7,2,3:48`).
/// Each value `runlist << 16 | chid` is one datamatch sub-region of the doorbell register
/// with its own eventfd; a write of such a value ends in its eventfd, and the device rings it
/// exactly as the trapped write of that value (ctrl.rs `doorbell`). For guests that ring
/// their own tokens and never read the work-submit-token notifier entry: the Windows KMD
/// computes its tokens itself (the pool's hand-out cannot reach it).
pub const GUEST_TOKENS_ENV: &str = "CARAXES_DOORBELL_GUEST_TOKENS";

/// `NV_CTRL_VF_DOORBELL` (tu102/dev_ctrl.h:36-37): VECTOR (the ChID) 11:0, RUNLIST_ID 22:16.
pub const TOKEN_VECTOR_BITS: u32 = 12;
pub const TOKEN_RUNLIST_SHIFT: u32 = 16;
pub const TOKEN_RUNLIST_MAX: u32 = 0x7F;

/// ChIDs per runlist in the automatic set: the Windows KMD and its applications used ChIDs up
/// to 37 (the ring census of doorbell-token-pool.md (g)); ChIDs are one space for all
/// runlists on Turing, so a guest with more live channels than this has its higher ones trap.
pub const AUTO_CHIDS: u32 = 48;

/// The guest token of `chid` on `runlist` (kfifoGenerateWorkSubmitTokenHal_TU102,
/// kernel_fifo_tu102.c:75-130).
pub fn guest_token(runlist: u32, chid: u32) -> u32 {
    (runlist << TOKEN_RUNLIST_SHIFT) | chid
}

/// What the switch asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuestTokensRequest {
    Off,
    Auto,
    Explicit(GuestTokens),
}

/// The guest tokens declared as ioeventfds, in registration order: KVM passes the entries
/// of one address in that order, about 25 ns each, so the busiest values go first. The
/// `hot` runlists are interleaved per ChID (ChID 0 of each, then ChID 1, ...): the graphics
/// runlist and the first async copy runlist, which carry most rings (the KMD's paging copies
/// and the applications' GR channels); the `rest` follow runlist by runlist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestTokens {
    pub hot: Vec<u32>,
    pub rest: Vec<u32>,
    pub chids: u32,
}

impl GuestTokensRequest {
    pub fn requested() -> Result<Self, String> {
        match std::env::var(GUEST_TOKENS_ENV) {
            Ok(v) => Self::parse(&v),
            Err(_) => Ok(GuestTokensRequest::Auto),
        }
    }

    pub fn parse(v: &str) -> Result<Self, String> {
        let v = v.trim();
        match v {
            "" | "auto" => return Ok(GuestTokensRequest::Auto),
            "0" | "off" => return Ok(GuestTokensRequest::Off),
            _ => {}
        }
        let bad = || {
            format!("{GUEST_TOKENS_ENV}={v:?}: auto, off, or <hot>[,<runlist>...]:<chids> with the hot runlists joined by + (e.g. 0+1,7,2,3:48)")
        };
        let (rl, n) = v.split_once(':').ok_or_else(bad)?;
        let chids: u32 = n.trim().parse().map_err(|_| bad())?;
        let num = |r: &str| r.trim().parse::<u32>().map_err(|_| bad());
        let mut groups = rl.split(',');
        let hot = groups.next().ok_or_else(bad)?.split('+').map(num).collect::<Result<Vec<_>, _>>()?;
        let rest = groups.map(num).collect::<Result<Vec<_>, _>>()?;
        let t = GuestTokens { hot, rest, chids };
        t.check()?;
        Ok(GuestTokensRequest::Explicit(t))
    }
}

impl GuestTokens {
    /// The automatic set from the device-info table the device presents: the graphics
    /// engine's runlist and the lowest async copy runlist interleaved first, then the
    /// video encoders', the other copy runlists and the video decoders', as many as
    /// `budget` eventfds allow at `chids` ChIDs each (at most `AUTO_CHIDS`). Engines that
    /// are not host-driven, SEC2, NVJPG and OFA get none. `engines`: (name, runlist,
    /// host_driven) of each entry.
    pub fn auto(engines: &[(&str, u32, bool)], chids: u32, budget: usize) -> Option<Self> {
        let chids = chids.min(AUTO_CHIDS);
        let rls = |prefix: &str| {
            let mut v: Vec<u32> = engines
                .iter()
                .filter(|(n, _, hd)| *hd && n.starts_with(prefix))
                .map(|(_, r, _)| *r)
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let gr = *rls("gr").first()?;
        let ce: Vec<u32> = rls("ce").into_iter().filter(|r| *r != gr).collect();
        let mut hot = vec![gr];
        hot.extend(ce.first());
        let mut rest: Vec<u32> = Vec::new();
        for r in rls("nvenc").into_iter().chain(ce.iter().skip(1).copied()).chain(rls("nvdec")) {
            if !hot.contains(&r) && !rest.contains(&r) {
                rest.push(r);
            }
        }
        if chids == 0 {
            return None;
        }
        let fit = budget / chids as usize;
        if fit < hot.len() {
            return None;
        }
        rest.truncate(fit - hot.len());
        let t = GuestTokens { hot, rest, chids };
        t.check().ok()?;
        Some(t)
    }

    fn check(&self) -> Result<(), String> {
        let all: Vec<u32> = self.hot.iter().chain(&self.rest).copied().collect();
        for (i, r) in all.iter().enumerate() {
            if *r > TOKEN_RUNLIST_MAX || all[..i].contains(r) {
                return Err(format!("{GUEST_TOKENS_ENV}: runlist {r} repeated or beyond RUNLIST_ID 22:16"));
            }
        }
        if self.hot.is_empty() {
            return Err(format!("{GUEST_TOKENS_ENV}: no runlist"));
        }
        if self.chids == 0 || self.chids > 1 << TOKEN_VECTOR_BITS {
            return Err(format!("{GUEST_TOKENS_ENV}: chids must be 1 to {} (VECTOR 11:0)", 1 << TOKEN_VECTOR_BITS));
        }
        if self.len() > MAX_TOKENS {
            return Err(format!(
                "{GUEST_TOKENS_ENV}: {} tokens, at most {MAX_TOKENS} (SCM_MAX_FD: one eventfd each, one reply)",
                self.len()
            ));
        }
        Ok(())
    }

    pub fn runlists(&self) -> Vec<u32> {
        self.hot.iter().chain(&self.rest).copied().collect()
    }

    pub fn len(&self) -> usize {
        (self.hot.len() + self.rest.len()) * self.chids as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The tokens in registration order: the hot runlists interleaved per ChID, then the
    /// rest runlist by runlist, ChIDs ascending.
    pub fn values(&self) -> Vec<u32> {
        let hot = (0..self.chids).flat_map(|c| self.hot.iter().map(move |&r| guest_token(r, c)));
        let rest = self.rest.iter().flat_map(|&r| (0..self.chids).map(move |c| guest_token(r, c)));
        hot.chain(rest).collect()
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

    /// A value of a channel is its own on its notifier page only if every other channel a
    /// ring of it rings keeps its notifier elsewhere.
    #[test]
    fn a_shared_value_is_own_only_off_the_other_channels_page() {
        let page = (false, 0x40080);
        let pages = |c: u32| match c {
            9 => Some(page),
            10 => Some((false, 0x40081)),
            11 => Some(page),
            _ => None,
        };
        assert!(own_on_page(9, &[9], page, pages));
        assert!(own_on_page(9, &[54], page, pages), "chid 54 has no notifier");
        assert!(own_on_page(9, &[9, 10], page, pages), "chid 10's entries are on its own page");
        assert!(!own_on_page(9, &[9, 11], page, pages), "chid 11 shares the page");
        assert!(!own_on_page(9, &[11], page, pages));
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
        use super::{guest_token, GuestTokens, GuestTokensRequest as Q};
        assert_eq!(Q::parse("").unwrap(), Q::Auto);
        assert_eq!(Q::parse("auto").unwrap(), Q::Auto);
        assert_eq!(Q::parse("off").unwrap(), Q::Off);
        let Q::Explicit(t) = Q::parse("0+1,7:2").unwrap() else { panic!() };
        assert_eq!(t.values(), vec![0x0, 0x10000, 0x1, 0x10001, 0x70000, 0x70001]);
        assert_eq!(guest_token(1, 15), 0x1000f);
        assert!(Q::parse("0,0:4").is_err());
        assert!(Q::parse("128:4").is_err());
        assert!(Q::parse("0:0").is_err());
        assert!(Q::parse("0:4097").is_err());
        assert!(Q::parse("0+1,2,3,7,8:51").is_err(), "306 > SCM_MAX_FD");
        // Never a pool token: the pool's bits 31:23 lie above RUNLIST_ID 22:16.
        let Q::Explicit(t) = Q::parse("0,127:1").unwrap() else { panic!() };
        for v in t.values() {
            assert!(!super::is_pool_token(v, super::MAX_TOKENS));
        }
        assert!(!super::is_pool_token(guest_token(127, 4095), super::MAX_TOKENS));
        let _ = GuestTokens::auto(&[], 48, 253);
    }

    /// The automatic set from TU106's device-info table (chips/tu106/device-info.toml):
    /// GR (0) and CE2 (1) interleaved, then NVENC (7), CE3 (2), CE4 (3); the NVDECs do not
    /// fit 253 eventfds at 48 ChIDs. With the pool's 64 fewer, only NVENC follows.
    #[test]
    fn automatic_guest_tokens_on_tu106() {
        use super::GuestTokens;
        let engines = [
            ("gr0", 0, true), ("ce0", 0, true), ("ce1", 0, true), ("ce2", 1, true),
            ("ce3", 2, true), ("ce4", 3, true), ("nvdec0", 4, true), ("nvdec1", 5, true),
            ("nvdec2", 6, true), ("nvenc0", 7, true), ("sec2", 8, true), ("sw", 0, false),
        ];
        let t = GuestTokens::auto(&engines, 512, 252).unwrap();
        assert_eq!((t.hot.clone(), t.rest.clone(), t.chids, t.len()), (vec![0, 1], vec![7, 2, 3], 48, 240));
        let v = t.values();
        assert_eq!(&v[..4], &[0x0, 0x10000, 0x1, 0x10001]);
        assert_eq!(v.iter().position(|&x| x == 0x1000f), Some(31), "the KMD's paging CE");
        assert_eq!(v.iter().position(|&x| x == 0x1a), Some(52), "a GR channel");
        assert_eq!(v.iter().position(|&x| x == 0x70024), Some(132), "NVENC");
        let t = GuestTokens::auto(&engines, 512, 253 - 64 - 2).unwrap();
        assert_eq!(t.runlists(), vec![0, 1, 7]);
        assert!(GuestTokens::auto(&engines, 512, 50).is_none());
    }
}

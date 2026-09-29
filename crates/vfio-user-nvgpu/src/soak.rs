// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The soak line (docs/caraxes/host-resources.md): one `key=value` line every
//! `CARAXES_SOAK_SECS` seconds (default 60, `0` off) while the device serves a guest, and
//! once at exit, with what the device holds on the host and what the host has left. A
//! leak shows as a count or a size that only grows between two lines at the same guest
//! load; `caraxes/tools/soak-watch.sh` samples the same figures from outside.
//!
//! Keys: `rss_kib` (the device's resident set), `vram_used_mib`/`vram_free_mib` (host,
//! `FB_GET_INFO_V2` total minus `HEAP_FREE`), `bar1_used_mib`/`bar1_free_mib` (host),
//! `foll_pin` (host-wide pinned pages since the device opened its client, other VMs
//! included), `pinned_chunks`/`pinned_mib` (the guest RAM this device registered),
//! `host_objects` and its groups (`vas`, `virtual`, `tsg`, `channel`, `engine`,
//! `guest_page`, `event`, `other`: every RM object the device holds), `gathers`,
//! `shadow_vas`/`ce_vas`/`tsgs`/`channels` (the mirror's own maps), `vram_windows`/
//! `vram_failed` (the FB store's BAR1 windows), `bar1_refused` (windows the host BAR1 floor
//! refused), `walk_caches` (cached page-table walks),
//! `floor_refused`/`no_memory` (hostfloor.rs).

use std::collections::BTreeMap;
use std::time::Duration;

/// The interval in seconds; `0` turns the periodic line off (the exit line stays).
pub const ENV: &str = "CARAXES_SOAK_SECS";
pub const DEFAULT_SECS: u64 = 60;

const MIB: u64 = 1 << 20;

/// The interval from the environment (`None`: off).
pub fn interval() -> Option<Duration> {
    let secs = std::env::var(ENV)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_SECS);
    (secs > 0).then(|| Duration::from_secs(secs))
}

/// The host's VRAM and BAR1 in bytes (hostmem.rs `host_figures`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostFigures {
    pub vram_total: u64,
    pub vram_free: u64,
    pub bar1_total: u64,
    pub bar1_free: u64,
}

/// What the host memory client holds (hostmem.rs).
#[derive(Clone, Debug, Default)]
pub struct HostHeld {
    /// RM objects by group ([`group_of`]).
    pub objects: BTreeMap<&'static str, usize>,
    pub descriptors: usize,
    pub pinned_bytes: u64,
    pub gathers: usize,
    pub foll_pin: i64,
    pub floor_refused: u64,
    pub no_memory: u64,
}

/// What the device model holds (device.rs `soak_counts`).
#[derive(Clone, Copy, Debug, Default)]
pub struct DeviceCounts {
    pub shadow_vas: usize,
    pub ce_vas: usize,
    pub tsgs: usize,
    pub channels: usize,
    pub vram_windows: u64,
    pub vram_failed: u64,
    /// Window maps the host BAR1 floor refused (hostfloor.rs).
    pub bar1_refused: u64,
    pub walk_caches: usize,
}

/// The group of a host object by the name it was allocated under (hostmem.rs `live`).
pub fn group_of(what: &str) -> &'static str {
    match what {
        w if w.ends_with("vaspace") => "vas",
        w if w.starts_with("shadow-virtual") => "virtual",
        w if w.ends_with("tsg") => "tsg",
        w if w.ends_with("channel") => "channel",
        w if w.contains("engine") || w == "inv-ce" => "engine",
        w if w.contains("userd-page") || w.contains("error-notifier") => "guest_page",
        "os-event" | "usermode" => "event",
        _ => "other",
    }
}

/// The device's resident set in KiB (`VmRSS` of /proc/self/status).
pub fn rss_kib() -> Option<u64> {
    parse_rss(&std::fs::read_to_string("/proc/self/status").ok()?)
}

fn parse_rss(status: &str) -> Option<u64> {
    status
        .lines()
        .find(|l| l.starts_with("VmRSS:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
}

/// The line (without the `vfio-user-nvgpu: ` prefix).
pub fn line(
    label: &str,
    rss: Option<u64>,
    host: Option<HostFigures>,
    held: Option<&HostHeld>,
    dev: Option<&DeviceCounts>,
) -> String {
    let opt = |v: Option<u64>| v.map_or("-".to_string(), |v| v.to_string());
    let mut s = format!("soak {label}: rss_kib={}", opt(rss));
    match host {
        Some(h) => s.push_str(&format!(
            " vram_used_mib={} vram_free_mib={} bar1_used_mib={} bar1_free_mib={}",
            h.vram_total.saturating_sub(h.vram_free) / MIB,
            h.vram_free / MIB,
            h.bar1_total.saturating_sub(h.bar1_free) / MIB,
            h.bar1_free / MIB
        )),
        None => s.push_str(" vram_used_mib=- vram_free_mib=- bar1_used_mib=- bar1_free_mib=-"),
    }
    if let Some(h) = held {
        let total: usize = h.objects.values().sum();
        s.push_str(&format!(
            " foll_pin={} pinned_chunks={} pinned_mib={} host_objects={total}",
            h.foll_pin,
            h.descriptors,
            h.pinned_bytes / MIB
        ));
        for g in ["vas", "virtual", "tsg", "channel", "engine", "guest_page", "event", "other"] {
            s.push_str(&format!(" {g}={}", h.objects.get(g).copied().unwrap_or(0)));
        }
        s.push_str(&format!(
            " gathers={} floor_refused={} no_memory={}",
            h.gathers, h.floor_refused, h.no_memory
        ));
    }
    if let Some(d) = dev {
        s.push_str(&format!(
            " shadow_vas={} ce_vas={} tsgs={} channels={} vram_windows={} vram_failed={} bar1_refused={} walk_caches={}",
            d.shadow_vas, d.ce_vas, d.tsgs, d.channels, d.vram_windows, d.vram_failed, d.bar1_refused, d.walk_caches
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_objects_group_by_their_allocation_name() {
        for (w, g) in [
            ("vaspace", "vas"),
            ("shadow-vaspace", "vas"),
            ("inv-vaspace", "vas"),
            ("shadow-virtual-fixed", "virtual"),
            ("shadow-tsg", "tsg"),
            ("inv-tsg", "tsg"),
            ("channel", "channel"),
            ("shadow-channel", "channel"),
            ("inv-channel", "channel"),
            ("engine-object", "engine"),
            ("shadow-engine", "engine"),
            ("inv-ce", "engine"),
            ("userd-page", "guest_page"),
            ("shadow-error-notifier", "guest_page"),
            ("os-event", "event"),
            ("usermode", "event"),
        ] {
            assert_eq!(group_of(w), g, "{w}");
        }
    }

    #[test]
    fn the_line_has_every_key_once() {
        let mut held = HostHeld::default();
        held.objects.insert("channel", 3);
        held.objects.insert("vas", 2);
        let l = line(
            "60s",
            Some(123),
            Some(HostFigures { vram_total: 8 << 30, vram_free: 431 * MIB, bar1_total: 256 * MIB, bar1_free: 40 * MIB }),
            Some(&held),
            Some(&DeviceCounts::default()),
        );
        assert!(l.starts_with("soak 60s: rss_kib=123 vram_used_mib=7761 vram_free_mib=431 bar1_used_mib=216"), "{l}");
        assert!(l.contains(" host_objects=5 vas=2 virtual=0 tsg=0 channel=3 "), "{l}");
        for k in ["foll_pin=", "walk_caches=", "vram_failed=", "floor_refused=", "bar1_refused=", "bar1_free_mib="] {
            assert_eq!(l.matches(k).count(), 1, "{k} in {l}");
        }
        assert_eq!(parse_rss("Name:\tx\nVmRSS:\t  4242 kB\n"), Some(4242));
    }
}

// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The device's environment in one table: every variable it reads, by class, and every
//! variable it no longer reads with what became of it.
//!
//! One configuration serves every guest OS. What host execution needs is on with
//! `--host-exec` (a mode, not an OS): the shadow VA spaces, GPA DMA, the interrupt page,
//! the hybrid doorbell reactor, the BAR1 pool, the host boost. Pure emulation keeps the
//! trapped paths. The classes:
//! - [`Class::Knob`]: the tuning options. With the command line's VRAM size
//!   (`--profile`/`--fb-mib`) and `--display` there are five.
//! - [`Class::Open`]: switches kept as they are until an A/B decides them (the run note
//!   `runs-2026-09-30/flags/NOTE.md` names the A/B each one needs).
//! - [`Class::Diag`]: diagnostics and overrides for bring-up; not tuning.
//! - [`Class::Input`]: facts the launcher passes.
//!
//! At start the device logs its effective configuration in one block (main.rs
//! `log_config`), and one warning line for every removed variable that is still set, so
//! an old wrapper is noticed.

/// What a variable is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Knob,
    Open,
    Diag,
    Input,
}

/// A variable the device reads.
#[derive(Clone, Copy, Debug)]
pub struct Var {
    pub name: &'static str,
    pub class: Class,
    /// The value when unset, as the log shows it.
    pub default: &'static str,
    pub what: &'static str,
}

const fn var(name: &'static str, class: Class, default: &'static str, what: &'static str) -> Var {
    Var { name, class, default, what }
}

use Class::{Diag, Input, Knob, Open};

/// Every variable the device reads.
pub const VARS: &[Var] = &[
    // The knobs (with `--profile`/`--fb-mib` and `--display`).
    var("CARAXES_COMPRESSION", Knob, "off", "off or slice: real compression per VM slice (D16b)"),
    var("CARAXES_DOORBELL_POLL", Knob, "hybrid with --host-exec, else off", "off, hybrid or spin: how the doorbell reactor waits (latency against a core)"),
    var("CARAXES_BAR1_POOL_MIB", Knob, "32 with --host-exec", "the BAR1 pool in MiB, 0 for none: the host BAR1 this VM takes beside its 2 x 16 MiB windows"),
    var("LEA_VRAM_PROFILE_MIB", Knob, "4096", "the guest FB in MiB when --fb-mib and --profile are not given"),
    // Kept as they are until an A/B decides them.
    var("CARAXES_DOORBELL_TOKENS", Open, "off", "N: the doorbell token pool (Linux B, nouveau: 64; Windows A/B open)"),
    var("CARAXES_BAR2_MEMFD", Open, "off", "1: guest BAR2 as a memfd (Linux B, nouveau; Windows A/B open)"),
    var("CARAXES_MSI_REARM_IOEVENTFD", Open, "off", "1: the MSI re-arm as an ioeventfd (Linux B, nouveau; Windows A/B open)"),
    var("CARAXES_DISPLAY_PAGE", Open, "off", "1: the display front-end page as memory (Linux B; Windows A/B open)"),
    var("CARAXES_ALLOC_LAZY", Open, "off", "1: shadow plans deferred to the next doorbell (Linux B)"),
    var("CARAXES_ALLOC_LAZY_MS", Open, "20", "the lazy plans' deadline in ms"),
    var("CARAXES_HOST_BAR1_FLOOR_MIB", Open, "96", "host BAR1 the pool must leave free (Linux wrappers 64, Windows 96)"),
    var("CARAXES_SHADOW_USERD", Open, "off", "1: private USERD (Windows)"),
    var("CARAXES_DEFERRED_CONTEXT", Open, "off", "1: deferred context execution (Windows)"),
    var("CARAXES_FB_ZERO_BASED", Open, "off", "1: the guest FB starts at 0 as on the card (Windows)"),
    var("CARAXES_SHADOW_FENCE", Open, "off", "1: shadow VA spaces fence the guest RM's kernel range (Windows)"),
    var("CARAXES_WIN_ACKS", Open, "off", "1: four controls the KMD needs acknowledged (Windows)"),
    var("CARAXES_WIN_DISPLAY_QUERIES", Open, "off", "1: the KMD's display queries answered (Windows)"),
    var("CARAXES_RUSD", Open, "off", "1: the RM user shared data page kept as GSP-RM keeps it (Windows)"),
    var("CARAXES_LPWR_2806_OK", Open, "off", "1: LPWR 0x2806 answered NV_OK (Windows)"),
    var("CARAXES_EMU_GR", Open, "off", "1: GR static info in pure emulation (Windows; breaks Linux emu-boot)"),
    var("CARAXES_PREFER_RECORDED", Open, "off", "1: recorded answers before the fallbacks in pure emulation (Windows)"),
    var("CARAXES_USERMODE_PAGE", Open, "off", "1: the usermode page as memory (-40 % on 2026-09-28, reactor fixed since)"),
    var("CARAXES_INVALIDATE_PAGE", Open, "off", "1: the MMU invalidate page as memory (fixed since, not GPU-validated)"),
    var("CARAXES_BAR2_FLUSH_PAGE", Open, "trap", "memory: the BAR2 flush page untrapped (needs a GPU run)"),
    var("CARAXES_INTR_DIRECT", Open, "off", "1: the event thread completes interrupts itself (no A/B verdict)"),
    var("CARAXES_INTR_GATE_MS", Open, "off", "ms: foreign non-stall events held while idle (no measured change)"),
    var("CARAXES_RPC_IOEVENTFD", Open, "off", "1: the GSP queue head as an ioeventfd (no verdict)"),
    var("CARAXES_LEGACY_CACHEABLE", Open, "off", "1: experimental CACHEABLE on sampled GSS-legacy candidates"),
    var("CARAXES_EXT_CONFIG", Open, "off", "1: the extended config space in the config region too"),
    // Diagnostics and overrides.
    var("CARAXES_DLOCK_STATS", Diag, "1", "0: skip detailed lock/part statistics; locking and GPU behavior are unchanged"),
    var("CARAXES_PROFILE", Diag, "off", "1: the per-device profile (SIGUSR2 and exit dumps)"),
    var("CARAXES_PROFILE_OUT", Diag, "beside the trace", "the profile's dump prefix"),
    var("CARAXES_TRACE_ACCESSES", Diag, "1", "0, 1 or bar0:<lo>-<hi>: which trapped accesses the trace lists"),
    var("CARAXES_LAUNCH_TIMING", Diag, "off", "1: per-step histograms of the ring path"),
    var("CARAXES_SOAK_SECS", Diag, "60", "the soak line's interval, 0 for none"),
    var("CARAXES_MIRROR_DIAG", Diag, "off", "1: the mirror's push diagnostic"),
    var("CARAXES_GR_CAPTURE", Diag, "off", "1: capture live C597 and MME state before each host doorbell"),
    var("CARAXES_SHADOW_CHANNEL_DIAG", Diag, "off", "1: the shadow channels' coverage queries"),
    var("CARAXES_SEMAPHORE_DIAG", Diag, "off", "1: semaphore diagnostics"),
    var("CARAXES_SEMAPHORE_DIAG_CHID", Diag, "all", "the one ChID of the semaphore diagnostics"),
    var("CARAXES_DEFERRED_API_CAPTURE", Diag, "off", "1: capture the deferred API controls"),
    var("CARAXES_SHADOW_ALWAYS_RESYNC", Diag, "off", "1: walk and diff the whole tree at every resync (the reference)"),
    var("CARAXES_SHADOW_REREAD", Diag, "off", "1: re-read dirty table pages from VRAM (the reference)"),
    var("CARAXES_DUMP_KERNEL", Diag, "off", "1: dump inline-to-memory kernel payloads to the trace"),
    var("CARAXES_RECORD_HOST", Diag, "off", "<file>: every host-RM answer as a recorded-controls line"),
    var("CARAXES_ACK_UNANSWERED", Diag, "off", "1: every unanswered control gets NV_OK (Windows production still sets it)"),
    var("CARAXES_WIN_EXCEPT", Diag, "none", "<hex,...>: controls left to the dispatcher (bisect)"),
    var("CARAXES_WIN_ACK_EXTRA", Diag, "none", "<hex,...>: controls answered NV_OK (bisect)"),
    var("CARAXES_WIN_DISP_CAPS", Diag, "0", "<hex>: NV5070 capsTbl[0]"),
    var("CARAXES_DISPLAY_UNPLUGGED", Diag, "off", "1: the connectors start without a monitor"),
    var("CARAXES_P20_SCAN", Diag, "off", "1: the reservation's isolation scan at start and free"),
    var("CARAXES_INJECT_TEARDOWN_FAILURE", Diag, "off", "1: count one failed host free (a test of the rule)"),
    var("CARAXES_GOE_SELFTEST", Diag, "off", "s: an operational-event self-test every s seconds"),
    var("CARAXES_RC_SELFTEST", Diag, "off", "s: an RC self-test after s seconds"),
    var("CARAXES_HOST_ABI", Diag, "the running driver", "610 or 615: the host layouts, overriding the running driver"),
    // Facts from the launcher.
    var("LEA_ROOT", Input, ".", "the root the default paths are relative to"),
    var("CARAXES_GUEST_RAM_BYTES", Input, "unset", "the guest's RAM size: registered RAM gets one linear IOVA offset"),
    var("LEA_GPU_NAME_FORMAT", Input, "transport", "transport or legacy: the guest-visible name (nvrm_abi::naming, shared with Leandro v1)"),
];

/// A variable the device no longer reads and what became of it.
#[derive(Clone, Copy, Debug)]
pub struct Removed {
    pub name: &'static str,
    pub now: &'static str,
}

const fn gone(name: &'static str, now: &'static str) -> Removed {
    Removed { name, now }
}

/// Every variable removed on 2026-09-30 (flags-2026-09-30): promoted to the default or
/// deleted with its experiment; CARAXES_WIN_COMPBIT retired with D16 (2026-09-29).
pub const REMOVED: &[Removed] = &[
    gone("CARAXES_MIRROR_CHANNELS", "the default with --host-exec"),
    gone("CARAXES_MIRROR_GR_SHADOW", "the default with --host-exec (every host channel on a shadow VA space)"),
    gone("CARAXES_MIRROR_BARE_CE", "the default with --host-exec"),
    gone("CARAXES_CE_SHADOW", "always on (=0 deleted)"),
    gone("CARAXES_MIRROR_GR", "deleted with the c2 GR experiment"),
    gone("CARAXES_MIRROR_CE", "deleted (CE channels always run on the host)"),
    gone("CARAXES_GUEST_DMA_GPA", "the default with --host-exec"),
    gone("CARAXES_INTR_PAGE", "the default with --host-exec"),
    gone("CARAXES_INTR_PAGE_SLEEP_MS", "fixed at 1 ms"),
    gone("CARAXES_INTR_PAGE_RETRIGGER_MS", "fixed at 10 ms"),
    gone("CARAXES_INTR_PAGE_VBLANK_SPIN_US", "deleted (no measured gain)"),
    gone("CARAXES_DOORBELL_POLL_CPU", "the core is chosen at start"),
    gone("CARAXES_DOORBELL_POLL_IDLE_US", "fixed at 200 us"),
    gone("CARAXES_DOORBELL_POLL_ADAPT_US", "deleted (did not pay off)"),
    gone("CARAXES_DOORBELL_IOEVENTFD", "deleted (the token pool and the guest tokens replace it)"),
    gone("CARAXES_DOORBELL_GUEST_TOKENS", "automatic with --boot-model"),
    gone("CARAXES_BAR1_PASSTHROUGH", "the pool is the default with --host-exec (CARAXES_BAR1_POOL_MIB sizes it, 0: none); =1 deleted"),
    gone("CARAXES_BAR1_PASSTHROUGH_UNSAFE_LAYOUT", "deleted with the fixed-layout prototype"),
    gone("CARAXES_BAR1_PT_MIB", "deleted with the fixed-layout prototype"),
    gone("CARAXES_BAR1_PT_FB_OFFSET", "deleted with the fixed-layout prototype"),
    gone("CARAXES_BAR1_POOL_WARM", "deleted (A/B)"),
    gone("CARAXES_BAR1_POOL_PIN", "always on"),
    gone("CARAXES_BAR1_WINDOWS", "fixed at 2"),
    gone("CARAXES_BAR1_WINDOW_MIB", "fixed at 16"),
    gone("CARAXES_HOST_VRAM_FLOOR_MIB", "fixed at 1024"),
    gone("CARAXES_HOST_BOOST", "always on with --host-exec"),
    gone("CARAXES_LAUNCH_FAST", "always 2"),
    gone("CARAXES_DISPLAY_KICK_PAGE", "on with the display page"),
    gone("CARAXES_POSTED_DOORBELL", "always on"),
    gone("CARAXES_FAST_READS", "always on"),
    gone("CARAXES_SHADOW_COALESCE", "always on"),
    gone("CARAXES_SHADOW_BULK_FREE", "always on"),
    gone("CARAXES_SHADOW_REGION_POOL", "always on"),
    gone("CARAXES_SHADOW_GATHER", "always on"),
    gone("CARAXES_SHADOW_WORKER", "always on"),
    gone("CARAXES_SHADOW_DEFER_TLB", "off (measured without gain)"),
    gone("CARAXES_SHADOW_UNTRAPPED_HOLD", "always on"),
    gone("CARAXES_SHADOW_INVALIDATE_ASYNC", "always on"),
    gone("CARAXES_MEMOP_INVALIDATE", "always on"),
    gone("CARAXES_MIRROR_VIDEO_OBJECT", "always on"),
    gone("CARAXES_BAR_TLB", "always on"),
    gone("CARAXES_FORWARD_CACHE", "always on"),
    gone("CARAXES_LINEAR_IOVA", "always on"),
    gone("CARAXES_COMPRESSION_CLEAR", "always on"),
    gone("CARAXES_BAR2_MEMFD_DEFER_FLUSH", "always on"),
    gone("CARAXES_BAR2_MEMFD_TRAP", "the flush page, or none with CARAXES_BAR2_FLUSH_PAGE=memory"),
    gone("CARAXES_BAR2_MEMFD_SYNC_DOORBELL", "deleted (the reference sync)"),
    gone("CARAXES_MAP_BUDGET", "fixed at 32768"),
    gone("CARAXES_ZBC_MAX", "fixed at 4"),
    gone("CARAXES_FALCON_STALLACK", "boot.toml's value"),
    gone("CARAXES_GSP_FW_VERSION", "the served guest ABI's driver"),
    gone("CARAXES_GUEST_ABI", "the guest chooses; --guest-abi pins"),
    gone("CARAXES_TRACE_FLUSH", "flushed at event lines and every 100 ms"),
    gone("CARAXES_WIN_COMPBIT", "retired by D16: without compression the compbit store is refused, with CARAXES_COMPRESSION=slice compression.rs answers it"),
];

/// The variable `name`, if the device reads it.
pub fn lookup(name: &str) -> Option<&'static Var> {
    VARS.iter().find(|v| v.name == name)
}

/// What became of `name`, if the device no longer reads it.
pub fn removed(name: &str) -> Option<&'static Removed> {
    REMOVED.iter().find(|r| r.name == name)
}

/// The environment the device sees: its `CARAXES_` and `LEA_` variables.
pub fn environment() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(k, _)| k.starts_with("CARAXES_") || k.starts_with("LEA_"))
        .collect()
}

/// The value of `name` in `env`.
fn get<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
    env.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

/// `NAME=value` of each variable of `class` set in `env`, in table order.
pub fn set_of(env: &[(String, String)], class: Class) -> Vec<String> {
    VARS.iter()
        .filter(|v| v.class == class)
        .filter_map(|v| get(env, v.name).map(|x| format!("{}={x}", v.name)))
        .collect()
}

/// The knob `name` as the log shows it: `NAME=value` when set, else its default.
pub fn knob(env: &[(String, String)], name: &str) -> String {
    match (get(env, name), lookup(name)) {
        (Some(x), _) => format!("{name}={x} (set)"),
        (None, Some(v)) => format!("{name}={} (default)", v.default),
        (None, None) => format!("{name} (unknown)"),
    }
}

/// One warning line per removed variable set in `env`: the device ignores it.
pub fn removed_warnings(env: &[(String, String)]) -> Vec<String> {
    REMOVED
        .iter()
        .filter_map(|r| {
            get(env, r.name).map(|x| {
                format!(
                    "WARNING {}={x} is no longer read (removed 2026-09-30: {}); ignored",
                    r.name, r.now
                )
            })
        })
        .collect()
}

/// The `CARAXES_` variables in `env` the device neither reads nor removed: launcher
/// settings (the launchers pass their whole environment) or a misspelled name.
pub fn unread(env: &[(String, String)]) -> Vec<String> {
    let mut out: Vec<String> = env
        .iter()
        .map(|(k, _)| k)
        .filter(|k| k.starts_with("CARAXES_") && lookup(k).is_none() && removed(k).is_none())
        .cloned()
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(kv: &[(&str, &str)]) -> Vec<(String, String)> {
        kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// The whole-string `"CARAXES_*"` and `"LEA_*"` literals of the crate's sources outside
    /// this file: the names the device reads (every read names its variable so).
    fn literals() -> std::collections::BTreeSet<String> {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        walk(&root, &mut files);
        let mut names = std::collections::BTreeSet::new();
        for f in files {
            if f.ends_with("settings.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&f).unwrap();
            for prefix in ["\"CARAXES_", "\"LEA_"] {
                for (at, _) in text.match_indices(prefix) {
                    let rest = &text[at + 1..];
                    let len = rest
                        .bytes()
                        .take_while(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || *b == b'_')
                        .count();
                    if rest.as_bytes().get(len) == Some(&b'"') {
                        names.insert(rest[..len].to_string());
                    }
                }
            }
        }
        names
    }

    /// Every name the sources read is in the table, every table entry is read (the one
    /// read outside this crate excepted), and no removed name is read any more.
    #[test]
    fn the_table_is_the_sources_list() {
        let lits = literals();
        for n in &lits {
            assert!(lookup(n).is_some(), "{n} is read but not in settings::VARS");
            assert!(removed(n).is_none(), "{n} is removed but still named in the sources");
        }
        for v in VARS {
            if v.name == "LEA_GPU_NAME_FORMAT" {
                assert_eq!(v.name, nvrm_abi::naming::NAME_FORMAT_ENV);
                continue;
            }
            assert!(lits.contains(v.name), "{} is in settings::VARS but never read", v.name);
        }
    }

    /// Names are unique and no name is both read and removed.
    #[test]
    fn names_are_unique_and_removed_ones_are_not_read() {
        let mut seen = std::collections::BTreeSet::new();
        for n in VARS.iter().map(|v| v.name).chain(REMOVED.iter().map(|r| r.name)) {
            assert!(seen.insert(n), "{n} twice");
        }
    }

    /// At most five tuning knobs: the three variables here and the VRAM size variable, which
    /// is `--fb-mib`/`--profile`'s default, with `--display` on the command line.
    #[test]
    fn at_most_five_knobs() {
        let knobs: Vec<&str> = VARS.iter().filter(|v| v.class == Knob).map(|v| v.name).collect();
        assert_eq!(
            knobs,
            ["CARAXES_COMPRESSION", "CARAXES_DOORBELL_POLL", "CARAXES_BAR1_POOL_MIB", "LEA_VRAM_PROFILE_MIB"]
        );
    }

    #[test]
    fn a_removed_variable_is_warned_about_once() {
        let e = env(&[
            ("CARAXES_MIRROR_GR_SHADOW", "1"),
            ("CARAXES_COMPRESSION", "slice"),
            ("CARAXES_BAR1_PASSTHROUGH", "pool"),
        ]);
        let w = removed_warnings(&e);
        assert_eq!(w.len(), 2, "{w:?}");
        assert!(w[0].starts_with("WARNING CARAXES_MIRROR_GR_SHADOW=1 is no longer read"));
        assert!(w[1].contains("CARAXES_BAR1_POOL_MIB"), "{}", w[1]);
        assert!(removed_warnings(&env(&[("CARAXES_COMPRESSION", "off")])).is_empty());
    }

    #[test]
    fn launcher_variables_and_typos_are_named_as_unread() {
        let e = env(&[
            ("CARAXES_MEM", "8G"),
            ("CARAXES_BAR1_POOL_MB", "32"),
            ("CARAXES_INTR_PAGE", "1"),
            ("CARAXES_PROFILE", "1"),
            ("LEA_ROOT", "/x"),
        ]);
        assert_eq!(unread(&e), ["CARAXES_BAR1_POOL_MB", "CARAXES_MEM"]);
    }

    #[test]
    fn knobs_show_their_value_or_default_and_classes_their_set_members() {
        let e = env(&[("CARAXES_DOORBELL_POLL", "spin"), ("CARAXES_PROFILE", "1"), ("CARAXES_BAR2_MEMFD", "1")]);
        assert_eq!(knob(&e, "CARAXES_DOORBELL_POLL"), "CARAXES_DOORBELL_POLL=spin (set)");
        assert_eq!(knob(&e, "CARAXES_COMPRESSION"), "CARAXES_COMPRESSION=off (default)");
        assert_eq!(set_of(&e, Diag), ["CARAXES_PROFILE=1"]);
        assert_eq!(set_of(&e, Open), ["CARAXES_BAR2_MEMFD=1"]);
        assert!(set_of(&e, Input).is_empty());
    }
}

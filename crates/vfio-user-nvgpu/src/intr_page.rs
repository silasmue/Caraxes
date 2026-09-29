// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The interrupt page (`CARAXES_INTR_PAGE=1`, docs/caraxes/intr-page.md).
//!
//! BAR0 page 0 (`NV_PMC`: boot constants and the latched `NV_PMC_INTR_EN_SET/CLEAR`) and
//! the page of the VF CPU interrupt tree are offered to the client as sparse mmap areas of
//! region 0, backed by a memfd. The guest then reads and writes them as memory, without a
//! trap. The device keeps the tree's page current and learns the guest's stores by
//! polling: the doorbell reactor (poll.rs) calls [`PageHook`] on every loop iteration,
//! which compares the watched words with what the device wrote there last and applies
//! the difference with the semantics of the trapped path (`intr.rs` `IntrTree`).
//!
//! A memory word cannot report a store of the value it already holds, so every register
//! the guest writes shows a value the guest never stores: `TAG` (bit 31, no interrupt
//! vector, checked at start) in the leaves and in `TOP_EN_SET`, 0 in the clear registers,
//! `TRIGGER_IDLE` in the trigger. `LEAF_EN_SET` shows the exact mask (the guest reads and
//! caches it, intr_tu102.c:250-270); a store of that same mask is a no-op anyway.
//!
//! Exact mode: when the VMM maps the tree page read-only (Cloud Hypervisor's
//! `x_readonly_mmap_areas`) or not at all, the guest's stores to it trap. The first such
//! store switches the page to the registers' own values (turing/tu102/dev_vm.h:31-67: a
//! leaf shows its pending bits, `LEAF_EN_SET`/`LEAF_EN_CLEAR` both read the enable mask,
//! `TOP_EN_SET`/`TOP_EN_CLEAR` both the top enables, the write-only `LEAF_TRIGGER` 0, the
//! enables start at their reset value 0) and applies every store as it arrives
//! ([`IntrPage::write`]); no tag, no scan for stores. Reads stay memory. The Windows KMD
//! needs this: with the tag in every leaf it reset the GSP three times (windows-findings.md
//! P37 arm 2).

use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::intr::{
    IntrShared, LEAF, LEAF_EN_CLEAR, LEAF_EN_SET, LEAF_TRIGGER, LEAVES, TOP, TOP_EN_CLEAR,
    TOP_EN_SET,
};

/// The switch.
pub const INTR_PAGE_ENV: &str = "CARAXES_INTR_PAGE";
/// Hybrid reactor: the longest blocking wait while the page is on, in milliseconds.
pub const SLEEP_ENV: &str = "CARAXES_INTR_PAGE_SLEEP_MS";
/// The retrigger watchdog's period in milliseconds (0: off).
pub const RETRIGGER_ENV: &str = "CARAXES_INTR_PAGE_RETRIGGER_MS";

/// Spin from this many microseconds before each display tick (display page only; 0 or
/// unset: off), docs/caraxes/intr-page.md §11.4.
pub const VBLANK_SPIN_ENV: &str = "CARAXES_INTR_PAGE_VBLANK_SPIN_US";

/// `CARAXES_INTR_PAGE_VBLANK_SPIN_US`: microseconds, 0 to 16000.
pub fn vblank_spin(v: Option<&str>) -> Result<u64, String> {
    match v.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(0),
        Some(v) => v
            .parse::<u64>()
            .ok()
            .filter(|&us| us <= 16_000)
            .ok_or_else(|| format!("{VBLANK_SPIN_ENV}={v:?}: microseconds, 0 to 16000")),
    }
}

/// The page granularity of the sparse areas (the host's; checked at start).
pub const PAGE: u64 = 4096;
/// Shown in every leaf and in `TOP_EN_SET`; the guest never stores it.
pub const TAG: u32 = 1 << 31;
/// Shown in `LEAF_TRIGGER`: vector field 0xfff, no vector.
pub const TRIGGER_IDLE: u32 = 0xFFFF_FFFF;
/// The top-level subtrees that exist: two leaves each.
const SUBTREES: u32 = (1 << (LEAVES / 2)) - 1;
/// How often (reactor iterations) the spin loop runs the slow checks.
const TICK_ITERATIONS: u32 = 1024;
/// Page-0 offsets of the boot constants (bar0.rs).
const BOOT_OFFSETS: [u64; 3] = [
    crate::bar0::NV_PMC_BOOT_0,
    crate::bar0::NV_PMC_BOOT_1,
    crate::bar0::NV_PMC_BOOT_42,
];

/// The configuration the environment asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// Hybrid: the longest blocking wait (ms).
    pub sleep_ms: i32,
    /// The watchdog period; `None` turns it off.
    pub retrigger: Option<Duration>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            sleep_ms: 1,
            retrigger: Some(Duration::from_millis(10)),
        }
    }
}

/// `CARAXES_INTR_PAGE` and its two settings; `None` when the page is off. A setting given
/// without the switch is refused rather than ignored.
pub fn requested() -> Result<Option<Config>, String> {
    let get = |k: &str| std::env::var(k).ok();
    parse(
        get(INTR_PAGE_ENV).as_deref(),
        get(SLEEP_ENV).as_deref(),
        get(RETRIGGER_ENV).as_deref(),
    )
}

pub fn parse(
    on: Option<&str>,
    sleep: Option<&str>,
    retrigger: Option<&str>,
) -> Result<Option<Config>, String> {
    fn set(v: Option<&str>) -> Option<&str> {
        v.map(str::trim).filter(|v| !v.is_empty())
    }
    let (on, sleep, retrigger) = (set(on), set(sleep), set(retrigger));
    let on = match on.unwrap_or("0") {
        "0" | "off" => false,
        "1" | "on" => true,
        v => return Err(format!("{INTR_PAGE_ENV}={v:?}: 0 or 1")),
    };
    if !on {
        if sleep.is_some() || retrigger.is_some() {
            return Err(format!(
                "{SLEEP_ENV} or {RETRIGGER_ENV} is set but {INTR_PAGE_ENV} is off"
            ));
        }
        return Ok(None);
    }
    let mut c = Config::default();
    if let Some(v) = sleep {
        c.sleep_ms = v
            .parse::<u16>()
            .ok()
            .filter(|&ms| ms >= 1)
            .map(i32::from)
            .ok_or_else(|| format!("{SLEEP_ENV}={v:?}: milliseconds, 1 to 65535"))?;
    }
    if let Some(v) = retrigger {
        let ms = v
            .parse::<u64>()
            .map_err(|_| format!("{RETRIGGER_ENV}={v:?}: milliseconds, a whole number"))?;
        c.retrigger = (ms > 0).then(|| Duration::from_millis(ms));
    }
    Ok(Some(c))
}

/// The direct completion (docs/caraxes/launch-path.md): a raise from the host's event
/// thread folds the vector into the page and writes the MSI on that thread, instead of
/// waking the reactor to do it (one hop and, for a blocked reactor, one scheduler wake-up
/// less). Needs the page.
pub const DIRECT_ENV: &str = "CARAXES_INTR_DIRECT";

/// `CARAXES_INTR_DIRECT`: whether the direct completion is asked for; refused without the
/// page (`page_on`) rather than ignored.
pub fn direct_requested(v: Option<&str>, page_on: bool) -> Result<bool, String> {
    match v.map(str::trim).filter(|v| !v.is_empty()).unwrap_or("0") {
        "0" | "off" => Ok(false),
        "1" | "on" if page_on => Ok(true),
        "1" | "on" => Err(format!("{DIRECT_ENV}=1 needs {INTR_PAGE_ENV}=1")),
        v => Err(format!("{DIRECT_ENV}={v:?}: 0 or 1")),
    }
}

/// The watched words of the tree page: index -> offset within the tree.
const W_LEAF: usize = 0;
const W_EN_SET: usize = LEAVES;
const W_EN_CLEAR: usize = 2 * LEAVES;
const W_TOP: usize = 3 * LEAVES;
const W_TOP_EN_SET: usize = W_TOP + 1;
const W_TOP_EN_CLEAR: usize = W_TOP + 2;
const W_TRIGGER: usize = W_TOP + 3;
const WATCHED: usize = W_TRIGGER + 1;

fn watched_offset(i: usize) -> u64 {
    match i {
        i if i < W_EN_SET => LEAF + 4 * i as u64,
        i if i < W_EN_CLEAR => LEAF_EN_SET + 4 * (i - W_EN_SET) as u64,
        i if i < W_TOP => LEAF_EN_CLEAR + 4 * (i - W_EN_CLEAR) as u64,
        W_TOP => TOP,
        W_TOP_EN_SET => TOP_EN_SET,
        W_TOP_EN_CLEAR => TOP_EN_CLEAR,
        _ => LEAF_TRIGGER,
    }
}

/// One mapped page of the memfd, as 1024 words the guest also writes (also the display
/// front-end page, display/fe_page.rs).
pub(crate) struct Mapping {
    ptr: NonNull<AtomicU32>,
}

// SAFETY: the mapping is shared memory accessed only through atomics; it lives as long as
// the `Mapping` (unmapped in Drop).
unsafe impl Send for Mapping {}
// SAFETY: as above.
unsafe impl Sync for Mapping {}

impl Mapping {
    pub(crate) fn new(fd: RawFd, offset: u64) -> std::io::Result<Self> {
        // SAFETY: a fresh shared mapping of an fd we own; checked before use.
        let p = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                PAGE as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                offset as libc::off_t,
            )
        };
        if p == libc::MAP_FAILED {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Mapping {
            ptr: NonNull::new(p.cast()).expect("mmap returned a non-null address"),
        })
    }

    /// The word at byte offset `off` (dword-aligned, below `PAGE`).
    pub(crate) fn word(&self, off: u64) -> &AtomicU32 {
        assert!(off < PAGE && off % 4 == 0);
        // SAFETY: in bounds of the page mapped in `new`; AtomicU32 has u32's layout.
        unsafe { &*self.ptr.as_ptr().add((off / 4) as usize) }
    }

    /// The qword at byte offset `off` (qword-aligned, below `PAGE`): one store of it is
    /// one store for a reader of either half (the usermode page's clock, um_page.rs).
    pub(crate) fn qword(&self, off: u64) -> &std::sync::atomic::AtomicU64 {
        assert!(off < PAGE && off % 8 == 0);
        // SAFETY: in bounds and 8-aligned (the mapping is page-aligned); AtomicU64 has
        // u64's layout.
        unsafe { &*self.ptr.as_ptr().cast::<std::sync::atomic::AtomicU64>().add((off / 8) as usize) }
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: the mapping made in `new`.
        unsafe { libc::munmap(self.ptr.as_ptr().cast(), PAGE as usize) };
    }
}

/// The trapped path's model of the tree (`IntrTree`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Model {
    pub leaf: [u32; LEAVES],
    pub leaf_en: [u32; LEAVES],
    pub top_en: u32,
}

impl Model {
    /// Pending subtrees, as `IntrTree::top`.
    pub fn top(&self) -> u32 {
        let mut t = 0;
        for s in 0..LEAVES / 2 {
            if (self.leaf[2 * s] & self.leaf_en[2 * s])
                | (self.leaf[2 * s + 1] & self.leaf_en[2 * s + 1])
                != 0
            {
                t |= 1 << s;
            }
        }
        t
    }

    /// The word the page shows for watched word `i`; `exact`: the register's own value
    /// (the stores trap), else the tagged display a store can be told from.
    fn display(&self, i: usize, exact: bool) -> u32 {
        let tag = if exact { 0 } else { TAG };
        match i {
            i if i < W_EN_SET => self.leaf[i] | tag,
            i if i < W_EN_CLEAR => self.leaf_en[i - W_EN_SET],
            i if i < W_TOP && exact => self.leaf_en[i - W_EN_CLEAR],
            i if i < W_TOP => 0,
            W_TOP => self.top(),
            W_TOP_EN_SET => (self.top_en & SUBTREES) | tag,
            W_TOP_EN_CLEAR if exact => self.top_en & SUBTREES,
            W_TOP_EN_CLEAR => 0,
            _ if exact => 0,
            _ => TRIGGER_IDLE,
        }
    }
}

/// Inconsistencies, each logged once.
#[derive(Clone, Copy, Debug)]
#[repr(usize)]
enum Odd {
    TopStore,
    TagInLeaf,
    TagInEnable,
    BadTrigger,
    BootStore,
    TreeHole,
    ExactStore,
}

const ODD_NAMES: [&str; 7] = [
    "a store to the read-only TOP register (restored)",
    "a leaf store with the tag bit 31 (bit ignored)",
    "a LEAF_EN_SET store with the tag bit 31 (bit ignored)",
    "a LEAF_TRIGGER store outside the 256 vectors or at a tag bit (ignored)",
    "a store to a boot constant in page 0 (restored)",
    "a store to a word of the tree that is no register (restored to 0)",
    "a store to the tree page in exact mode that did not trap (the page is writable; restored)",
];

struct State {
    model: Model,
    /// The leaf enables of the kernel table (`enable_vectors`), kept across a reset.
    table_en: [u32; LEAVES],
    /// Vectors raised again while still pending (coalesced, as `IntrShared::raise` does
    /// while the guest has not looked): the ack that clears them pends them anew.
    again: [u32; LEAVES],
    /// When an enabled vector was last seen pending with no ack or MSI since (watchdog).
    stale_since: Option<Instant>,
    logged: u32,
    /// Trapped stores traced since the last reset or the switch to exact mode (the first
    /// [`STORES_TRACED`] of each, for a driver's (re-)initialisation).
    stores_traced: u32,
    /// The host engines' non-stall vectors ([`IntrPage::set_hint_vectors`]).
    hint: [u32; LEAVES],
    /// MSI re-arms that sent the MSI again for pending host hints only, with no ack of one
    /// of them since ([`IntrPage::rearm`]).
    hint_rearms: u32,
    /// Host hints withdrawn from a handler that does not service them: their raises are
    /// dropped until the guest acknowledges or enables them, a reset, or [`HINT_MUTE`].
    muted: [u32; LEAVES],
    muted_since: Option<Instant>,
    withdrawals_logged: u32,
}

/// Trapped stores traced after each reset (and after the switch to exact mode).
const STORES_TRACED: u32 = 256;

/// Re-arms of pending host hints without an ack after which the handler counts as not
/// servicing them ([`IntrPage::rearm`]). A handler that services a vector acknowledges it
/// at its first pass; C74 and C78 re-armed 16-19 million times without one.
const HINT_REARMS: u32 = 256;

/// How long withdrawn host hints stay muted without a sign from the guest.
const HINT_MUTE: Duration = Duration::from_secs(1);

/// Counters for the exit summary.
#[derive(Default)]
pub struct Stats {
    pub scans: AtomicU64,
    pub acks: AtomicU64,
    pub en_set: AtomicU64,
    pub en_clear: AtomicU64,
    pub top_en_set: AtomicU64,
    pub top_en_clear: AtomicU64,
    pub triggers: AtomicU64,
    pub raised: AtomicU64,
    pub coalesced: AtomicU64,
    pub again: AtomicU64,
    pub msi_raise: AtomicU64,
    pub msi_edge: AtomicU64,
    pub msi_watchdog: AtomicU64,
    pub msi_refused: AtomicU64,
    pub cas_retries: AtomicU64,
    /// MSI re-arms (`NV_XVE_CYA_2`) taken without the device mutex, and the MSIs they sent.
    pub rearms: AtomicU64,
    pub msi_rearm: AtomicU64,
    /// Host hints withdrawn at a re-arm, and their raises dropped while muted.
    pub withdrawn: AtomicU64,
    pub muted_raises: AtomicU64,
    /// Trapped accesses of the two pages served without the device mutex.
    pub routed: AtomicU64,
    pub odd: [AtomicU64; 7],
    pub slow_ns_max: AtomicU64,
}

fn bump(a: &AtomicU64) {
    a.fetch_add(1, Ordering::Relaxed);
}

/// What one scan did (tests and the hook).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// The page or the raised vectors had news.
    pub work: bool,
    /// The MSI eventfd was written.
    pub msi: bool,
}

/// The two pages, their model and the counters.
pub struct IntrPage {
    fd: OwnedFd,
    size: u64,
    tree_base: u64,
    pmc: Mapping,
    tree: Mapping,
    boot: [(u64, u32); 3],
    config: Config,
    shared: Arc<IntrShared>,
    state: Mutex<State>,
    /// The display values last written, for the lock-free comparison.
    expected: [AtomicU32; WATCHED],
    /// The guest's stores to the tree page trap: exact mode (module doc).
    exact: AtomicBool,
    pub stats: Stats,
}

impl IntrPage {
    /// A memfd of `bar0_size` bytes (sparse) with page 0 and the tree page (`tree_base`,
    /// page-aligned) mapped, the boot constants in place, the displays written.
    pub fn create(
        bar0_size: u64,
        tree_base: u64,
        identity: crate::chip::BootIdentity,
        shared: Arc<IntrShared>,
        config: Config,
    ) -> Result<Arc<Self>, String> {
        // SAFETY: sysconf has no preconditions.
        let host_page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if host_page != PAGE as libc::c_long {
            return Err(format!("host page size {host_page}, the areas need {PAGE}"));
        }
        if tree_base % PAGE != 0 || tree_base == 0 || tree_base + PAGE > bar0_size {
            return Err(format!(
                "interrupt tree at BAR0 {tree_base:#x}: not a page of its own within BAR0 ({bar0_size:#x})"
            ));
        }
        // SAFETY: a plain syscall with a NUL-terminated name; checked.
        let raw = unsafe { libc::memfd_create(c"caraxes-bar0-intr".as_ptr(), libc::MFD_CLOEXEC) };
        if raw < 0 {
            return Err(format!("memfd_create: {}", std::io::Error::last_os_error()));
        }
        // SAFETY: a new descriptor that nothing else owns.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        // SAFETY: plain syscall on our fd.
        if unsafe { libc::ftruncate(fd.as_raw_fd(), bar0_size as libc::off_t) } < 0 {
            return Err(format!("ftruncate: {}", std::io::Error::last_os_error()));
        }
        let pmc = Mapping::new(fd.as_raw_fd(), 0).map_err(|e| format!("mmap page 0: {e}"))?;
        let tree = Mapping::new(fd.as_raw_fd(), tree_base)
            .map_err(|e| format!("mmap page {tree_base:#x}: {e}"))?;
        let page = IntrPage {
            fd,
            size: bar0_size,
            tree_base,
            pmc,
            tree,
            boot: [
                (BOOT_OFFSETS[0], identity.boot_0),
                (BOOT_OFFSETS[1], identity.boot_1),
                (BOOT_OFFSETS[2], identity.boot_42),
            ],
            config,
            shared,
            state: Mutex::new(State {
                model: Model::default(),
                table_en: [0; LEAVES],
                again: [0; LEAVES],
                stale_since: None,
                logged: 0,
                stores_traced: 0,
                hint: [0; LEAVES],
                hint_rearms: 0,
                muted: [0; LEAVES],
                muted_since: None,
                withdrawals_logged: 0,
            }),
            expected: std::array::from_fn(|_| AtomicU32::new(0)),
            exact: AtomicBool::new(false),
            stats: Stats::default(),
        };
        page.reset();
        Ok(Arc::new(page))
    }

    pub fn config(&self) -> Config {
        self.config
    }

    /// The descriptor for region 0 (`ServerRegion::mmap_fd`).
    pub fn raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }

    /// The memfd's size (region 0's size).
    pub fn size(&self) -> u64 {
        self.size
    }

    /// The sparse areas of region 0: (offset, size), file offset = region offset.
    pub fn areas(&self) -> [(u64, u64); 2] {
        [(0, PAGE), (self.tree_base, PAGE)]
    }

    /// Map one more page of the memfd (the display front-end page, display/fe_page.rs):
    /// `off` page-aligned, inside BAR0 and neither of the two pages.
    pub(crate) fn map_page(&self, off: u64) -> Result<Mapping, String> {
        if off % PAGE != 0 || off + PAGE > self.size || self.covers(off) {
            return Err(format!(
                "BAR0 {off:#x}: not a free page of its own within BAR0 ({:#x})",
                self.size
            ));
        }
        Mapping::new(self.fd.as_raw_fd(), off).map_err(|e| format!("mmap page {off:#x}: {e}"))
    }

    /// The RM's MSI re-arm (`kbifRearmMSI_GM107`, kernel_bif_gm107.c:129-150: a write of
    /// `NV_PCFG` + `NV_XVE_CYA_2`, "HW only looks at the offset", at the end of every
    /// interrupt service): the card sends its MSI again while an enabled interrupt is still
    /// pending. Exact mode does that; the tagged display leaves the retrigger to its watchdog
    /// as before. Needs no device mutex (docs/caraxes/display-own-clock.md §2.4).
    ///
    /// Host hints ([`Self::set_hint_vectors`]): the card raises an engine's non-stall vector
    /// only for its own driver's work, while the host engines' non-stall events the device
    /// forwards come for every client of the host GPU (C78: 128,200 of 129,504 with no
    /// guest work waiting). A handler in its reset path neither acknowledges nor masks
    /// them but re-arms, and the re-arm's MSI then livelocks the vCPU (C74, C78: 16-19
    /// million MSIs, the Windows KMD's config save before its FLR never finished). When the
    /// MSI would be for pending host hints only and the guest re-armed [`HINT_REARMS`]
    /// times without acknowledging one, they are withdrawn and muted.
    pub fn rearm(&self) -> bool {
        bump(&self.stats.rearms);
        if !self.exact() {
            return false;
        }
        let mut st = self.lock();
        if st.model.top() & st.model.top_en == 0 {
            return false;
        }
        let live: [u32; LEAVES] = std::array::from_fn(|i| {
            if st.model.top_en & (1 << (i / 2)) != 0 {
                st.model.leaf[i] & st.model.leaf_en[i]
            } else {
                0
            }
        });
        if (0..LEAVES).all(|i| live[i] & !st.hint[i] == 0) {
            st.hint_rearms += 1;
            if st.hint_rearms >= HINT_REARMS {
                self.withdraw(&mut st, live);
                return false;
            }
        }
        drop(st);
        std::sync::atomic::fence(Ordering::SeqCst);
        let msi = self.shared.signal_msi();
        bump(if msi {
            &self.stats.msi_rearm
        } else {
            &self.stats.msi_refused
        });
        msi
    }

    /// The host engines' non-stall vectors (mirror.rs `hint_vectors`): notifications the
    /// device forwards from the host, which [`Self::rearm`] may withdraw.
    pub fn set_hint_vectors(&self, vectors: &[u32]) {
        let mut st = self.lock();
        st.hint = [0; LEAVES];
        for &v in vectors.iter().filter(|&&v| v < 256) {
            st.hint[(v / 32) as usize] |= 1 << (v % 32);
        }
    }

    /// Withdraw the pending host hints `live` (the state lock held) and mute their raises.
    fn withdraw(&self, st: &mut State, live: [u32; LEAVES]) {
        let before = (st.model.top(), st.model.top_en, st.model.leaf, st.model.leaf_en);
        for (i, &bits) in live.iter().enumerate() {
            st.model.leaf[i] &= !bits;
            st.again[i] &= !bits;
            st.muted[i] |= bits;
        }
        st.muted_since = Some(Instant::now());
        st.hint_rearms = 0;
        st.stale_since = None;
        self.publish(&st.model);
        bump(&self.stats.withdrawn);
        let n = self.stats.withdrawn.load(Ordering::Relaxed);
        if st.withdrawals_logged < 8 || n % 100 == 0 {
            st.withdrawals_logged += 1;
            let hex = |a: &[u32; LEAVES]| a.iter().map(|x| format!("{x:x}")).collect::<Vec<_>>().join(" ");
            let m = format!(
                "intr page: withdrawal {n}: the guest re-armed the MSI {HINT_REARMS} times without acknowledging the pending host non-stall hint(s) [{}]; withdrawn and muted for {} ms or until the guest acknowledges or enables them (top {:#x} top_en {:#x}, pending [{}], leaf enables [{}])",
                hex(&live),
                HINT_MUTE.as_millis(),
                before.0,
                before.1,
                hex(&before.2),
                hex(&before.3),
            );
            eprintln!("vfio-user-nvgpu: {m}");
            self.shared.note(m);
        }
    }

    /// Unmute `bits` of leaf `i` (the guest acknowledged or enabled them).
    fn unmute(st: &mut State, i: usize, bits: u32) {
        st.muted[i] &= !bits;
        if st.muted.iter().all(|&m| m == 0) {
            st.muted_since = None;
        }
    }

    /// A trapped access of the two pages served without the device mutex (the vfio-user
    /// server's route, device.rs `Routes`): counted here.
    pub fn note_routed(&self) {
        bump(&self.stats.routed);
    }

    /// Whether BAR0 offset `off` lies in one of the two pages.
    pub fn covers(&self, off: u64) -> bool {
        off < PAGE || (self.tree_base..self.tree_base + PAGE).contains(&off)
    }

    fn word_at(&self, off: u64) -> &AtomicU32 {
        let a = off & !3;
        if a < PAGE {
            self.pmc.word(a)
        } else {
            self.tree.word(a - self.tree_base)
        }
    }

    fn watched(&self, i: usize) -> &AtomicU32 {
        self.tree.word(watched_offset(i))
    }

    /// The start-up checks that need the device model: `owns(off)` says whether a model
    /// answers the BAR0 dword `off` (bar0.rs `model_owns`); `vectors` are the chip's table
    /// vectors (stall, non-stall) and the display's. Every dword of both pages must be
    /// the tree's, a boot constant or unowned (latched), and no vector may sit at a tag.
    pub fn check(
        &self,
        owns: &dyn Fn(u64) -> bool,
        tree_size: u64,
        vectors: &[u32],
    ) -> Result<(), String> {
        if tree_size > PAGE {
            return Err(format!(
                "interrupt tree of {tree_size:#x} bytes spans more than a page"
            ));
        }
        for base in [0, self.tree_base] {
            for off in (base..base + PAGE).step_by(4) {
                let tree = (self.tree_base..self.tree_base + tree_size).contains(&off);
                if tree || BOOT_OFFSETS.contains(&off) {
                    continue;
                }
                if owns(off) {
                    return Err(format!(
                        "BAR0 {off:#x} has a register model and lies in a page offered as memory"
                    ));
                }
            }
        }
        if let Some(v) = vectors.iter().find(|&&v| v < 256 && v % 32 == 31) {
            return Err(format!(
                "interrupt vector {v} sits at bit 31 of its leaf, which the page uses as its tag"
            ));
        }
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Device reset: the model back to its initial state, page 0 zero but for the boot
    /// constants, the tree page showing the initial displays.
    pub fn reset(&self) {
        let mut st = self.lock();
        let exact = self.exact();
        // Raised and not yet folded: gone with the rest of the interrupt state.
        let dropped = self.shared.reset_pending();
        if dropped.iter().any(|&p| p != 0) {
            self.shared
                .note(format!("intr page: reset: raised vectors dropped, per leaf: {dropped:x?}"));
        }
        st.model = Model {
            // Exact mode: the enables' reset value (dev_vm.h:61, :65); else every vector of
            // the kernel table (the tagged display's lost-store rule, `scan`).
            leaf_en: if exact { [0; LEAVES] } else { st.table_en },
            ..Model::default()
        };
        st.hint_rearms = 0;
        st.muted = [0; LEAVES];
        st.muted_since = None;
        st.again = [0; LEAVES];
        st.stale_since = None;
        st.stores_traced = 0;
        if exact {
            self.shared
                .note("intr page: reset in exact mode: leaves, leaf and top enables 0 until the guest's stores".into());
        }
        for off in (0..PAGE).step_by(4) {
            self.pmc.word(off).store(0, Ordering::Relaxed);
            self.tree.word(off).store(0, Ordering::Relaxed);
        }
        for (off, v) in self.boot {
            self.pmc.word(off).store(v, Ordering::Relaxed);
        }
        self.publish(&st.model);
        std::sync::atomic::fence(Ordering::SeqCst);
    }

    /// Exact mode (module doc): the guest's stores to the tree page trap.
    pub fn exact(&self) -> bool {
        self.exact.load(Ordering::Acquire)
    }

    /// Every watched word shows `model`'s display (the state lock held).
    fn publish(&self, model: &Model) {
        let exact = self.exact();
        for i in 0..WATCHED {
            let d = model.display(i, exact);
            self.watched(i).store(d, Ordering::Release);
            self.expected[i].store(d, Ordering::Relaxed);
        }
    }

    /// A trapped read of the pages (a client that did not map them): the memory.
    pub fn read32(&self, off: u64) -> u32 {
        self.word_at(off).load(Ordering::Acquire)
    }

    /// A trapped write. To the tree page: applied at once with the register's semantics
    /// (exact mode, switched on by the first); to page 0: stored as the guest would store
    /// it (latched values and boot constants, `tick` restores the latter).
    pub fn write(&self, off: u64, data: &[u8]) {
        if (self.tree_base..self.tree_base + PAGE).contains(&off) {
            self.write_tree(off - self.tree_base, data);
            return;
        }
        for (i, &b) in data.iter().enumerate() {
            let o = off + i as u64;
            let shift = (o & 3) * 8;
            let w = self.word_at(o);
            let mut cur = w.load(Ordering::Acquire);
            loop {
                let new = (cur & !(0xFF << shift)) | (u32::from(b) << shift);
                match w.compare_exchange(cur, new, Ordering::AcqRel, Ordering::Acquire) {
                    Ok(_) => break,
                    Err(c) => cur = c,
                }
            }
        }
    }

    /// A trapped store of `data` at tree-page offset `rel` (exact mode). Byte lanes not
    /// written are 0 in the value, as a narrower store enables only its own lanes.
    fn write_tree(&self, rel: u64, data: &[u8]) {
        let a = rel & !3;
        let mut v = 0u32;
        for (i, &b) in data.iter().enumerate() {
            let o = rel + i as u64;
            if o & !3 == a {
                v |= u32::from(b) << ((o & 3) * 8);
            }
        }
        let mut st = self.lock();
        if !self.exact.swap(true, Ordering::AcqRel) {
            // The first trapped store: the guest's stores reach the device synchronously,
            // so the registers' own values and semantics hold from here; the enables start
            // at their reset value (dev_vm.h:61, :65).
            st.model.leaf_en = [0; LEAVES];
            st.stores_traced = 0;
            let m = "intr page: the guest's stores to the interrupt tree trap (read-only mapping): exact register values from now on".to_string();
            eprintln!("vfio-user-nvgpu: {m}");
            self.shared.note(m);
        }
        let i = (0..WATCHED).find(|&i| watched_offset(i) == a);
        let (mut rearm, mut rearm_by_set, mut acked) = (false, false, false);
        match i {
            Some(i) if i < W_EN_SET => {
                self.shared.note_ack(i, v & st.model.leaf[i]);
                st.model.leaf[i] &= !v;
                acked = true;
                bump(&self.stats.acks);
                if v & st.hint[i] != 0 {
                    // The handler services host hints.
                    st.hint_rearms = 0;
                    Self::unmute(&mut st, i, v);
                }
                let again = st.again[i] & v;
                st.again[i] &= !again;
                st.model.leaf[i] |= again;
                if again != 0 {
                    rearm = true;
                    bump(&self.stats.again);
                }
            }
            Some(i) if i < W_EN_CLEAR => {
                st.model.leaf_en[i - W_EN_SET] |= v;
                Self::unmute(&mut st, i - W_EN_SET, v);
                rearm_by_set = true;
                bump(&self.stats.en_set);
            }
            Some(i) if i < W_TOP => {
                st.model.leaf_en[i - W_EN_CLEAR] &= !v;
                bump(&self.stats.en_clear);
            }
            Some(W_TOP) => self.odd(&mut st, Odd::TopStore),
            Some(W_TOP_EN_SET) => {
                st.model.top_en |= v & SUBTREES;
                rearm_by_set = true;
                bump(&self.stats.top_en_set);
            }
            Some(W_TOP_EN_CLEAR) => {
                st.model.top_en &= !v;
                bump(&self.stats.top_en_clear);
            }
            Some(_) => {
                let t = v & 0xFFF;
                if t < 256 {
                    let (l, b) = ((t / 32) as usize, 1u32 << (t % 32));
                    if st.model.leaf[l] & b == 0 {
                        rearm = true;
                    }
                    st.model.leaf[l] |= b;
                    bump(&self.stats.triggers);
                } else {
                    self.odd(&mut st, Odd::BadTrigger);
                }
            }
            None => self.odd(&mut st, Odd::TreeHole),
        }
        self.publish(&st.model);
        let edge = rearm_by_set && !rearm && st.model.top() != 0;
        if st.stores_traced < STORES_TRACED {
            st.stores_traced += 1;
            let m = &st.model;
            let en: Vec<String> = m.leaf_en.iter().map(|e| format!("{e:x}")).collect();
            self.shared.note(format!(
                "intr page: store {} at tree {rel:#x} = {v:#x}: top {:#x} top_en {:#x}, leaf enables [{}], pending [{}]",
                st.stores_traced,
                m.top(),
                m.top_en,
                en.join(" "),
                m.leaf.iter().map(|l| format!("{l:x}")).collect::<Vec<_>>().join(" ")
            ));
        }
        let mut msi = false;
        if edge || rearm {
            std::sync::atomic::fence(Ordering::SeqCst);
            msi = self.shared.signal_msi();
            bump(if !msi {
                &self.stats.msi_refused
            } else if edge {
                &self.stats.msi_edge
            } else {
                &self.stats.msi_raise
            });
        }
        if acked || msi {
            st.stale_since = None;
        }
    }

    /// The vectors of the chip's kernel table (stall and non-stall; vector 0 counts): the
    /// leaf enables the page reports from the start (§3 of docs/caraxes/intr-page.md).
    pub fn enable_vectors(&self, vectors: &[u32]) {
        {
            let mut st = self.lock();
            for &v in vectors.iter().filter(|&&v| v < 256 && v % 32 != 31) {
                st.table_en[(v / 32) as usize] |= 1 << (v % 32);
            }
        }
        self.reset();
    }

    /// The model as the reactor last applied it (tests, summary).
    pub fn model(&self) -> Model {
        self.lock().model.clone()
    }

    /// The fast check of one reactor iteration: anything to do?
    pub fn dirty(&self) -> bool {
        if self.shared.page_raised.load(Ordering::Acquire) {
            return true;
        }
        (0..WATCHED).any(|i| {
            self.watched(i).load(Ordering::Relaxed) != self.expected[i].load(Ordering::Relaxed)
        })
    }

    fn odd(&self, st: &mut State, what: Odd) {
        bump(&self.stats.odd[what as usize]);
        let bit = 1 << what as usize;
        if st.logged & bit == 0 {
            st.logged |= bit;
            let m = format!("intr page: {} (logged once)", ODD_NAMES[what as usize]);
            eprintln!("vfio-user-nvgpu: {m}");
            self.shared.note(m);
        }
    }

    /// Apply the guest's stores and the raised vectors, write the page, then the MSI.
    pub fn scan(&self) -> Scan {
        self.scan_racing(&mut || {})
    }

    /// `scan` with `between` run after the snapshot and before the write-back (tests: a
    /// guest store racing the reactor).
    fn scan_racing(&self, between: &mut dyn FnMut()) -> Scan {
        if !self.dirty() {
            return Scan::default();
        }
        let t0 = Instant::now();
        let mut st = self.lock();
        bump(&self.stats.scans);
        let w: [u32; WATCHED] = std::array::from_fn(|i| self.watched(i).load(Ordering::Acquire));
        let exp: [u32; WATCHED] = std::array::from_fn(|i| self.expected[i].load(Ordering::Relaxed));
        let changed = |i: usize| w[i] != exp[i];
        // Exact mode: the guest's stores trapped and were applied at once (`write_tree`);
        // the page only folds raises and shows the registers' values.
        let exact = self.exact();
        let (mut rearm_by_set, mut acked, mut rearm) = (false, false, false);
        if !exact {
            // 1. Enable stores. The leaf enables are reported, not modelled (docs/caraxes/
            // intr-page.md §3): the page shows every vector of the kernel table and every bit
            // the guest ever set, because the guest's per-vector enable loops at state load
            // store faster than any scan and lose all but the last value (series B on vm0:
            // LEAF_EN_SET(0) read 0x1 instead of 0xff, the non-stall vectors never showed in
            // TOP, the compositor stalled). Clears are counted and dropped. Every set store is
            // the guest re-enabling (a lock release, intrSetDisplayInterruptEnable,
            // intrEnableTopNonstall): a re-arm, as the edge of the trapped tree is.
            for i in 0..LEAVES {
                if w[W_EN_CLEAR + i] != 0 {
                    bump(&self.stats.en_clear);
                }
                if changed(W_EN_SET + i) {
                    let v = w[W_EN_SET + i];
                    if v & TAG != 0 {
                        self.odd(&mut st, Odd::TagInEnable);
                    }
                    st.model.leaf_en[i] |= v & !TAG;
                    rearm_by_set = true;
                    bump(&self.stats.en_set);
                }
            }
            if w[W_TOP_EN_CLEAR] != 0 {
                st.model.top_en &= !w[W_TOP_EN_CLEAR];
                bump(&self.stats.top_en_clear);
            }
            if changed(W_TOP_EN_SET) {
                st.model.top_en |= w[W_TOP_EN_SET] & SUBTREES;
                rearm_by_set = true;
                bump(&self.stats.top_en_set);
            }
            // 3. Acks: the word lost the tag, the word is the mask.
            for i in 0..LEAVES {
                let v = w[W_LEAF + i];
                if changed(W_LEAF + i) {
                    if v & TAG != 0 {
                        self.odd(&mut st, Odd::TagInLeaf);
                    }
                    let mask = v & !TAG;
                    self.shared.note_ack(i, mask & st.model.leaf[i]);
                    st.model.leaf[i] &= !mask;
                    acked = true;
                    bump(&self.stats.acks);
                    // A raise that came while the bit was pending may be after the guest's
                    // read: pend it again, with its own MSI (below).
                    let again = st.again[i] & mask;
                    st.again[i] &= !again;
                    st.model.leaf[i] |= again;
                    if again != 0 {
                        rearm = true;
                        bump(&self.stats.again);
                    }
                }
            }
            // 4. Triggers: a newly pending vector, with its MSI as a raise.
            if changed(W_TRIGGER) {
                let v = w[W_TRIGGER] & 0xFFF;
                if v < 256 && v % 32 != 31 {
                    let (l, b) = ((v / 32) as usize, 1u32 << (v % 32));
                    if st.model.leaf[l] & b == 0 {
                        rearm = true;
                    }
                    st.model.leaf[l] |= b;
                    bump(&self.stats.triggers);
                } else {
                    self.odd(&mut st, Odd::BadTrigger);
                }
            }
            if changed(W_TOP) {
                self.odd(&mut st, Odd::TopStore);
            }
        }
        // 5. Vectors raised by other threads; their MSI is the raise's, not an edge.
        // A vector still pending in the page costs no second MSI (the guest has not
        // acked it; `IntrShared::raise` coalesces the same way while the guest has not
        // looked); it is remembered and pended again at its ack.
        let mut raise_msi = rearm;
        if st.muted_since.is_some_and(|t| t.elapsed() >= HINT_MUTE) {
            st.muted = [0; LEAVES];
            st.muted_since = None;
        }
        if self.shared.page_raised.swap(false, Ordering::AcqRel) {
            self.shared.page_msi.store(false, Ordering::Release);
            for i in 0..LEAVES {
                let mut p = self.shared.take_pending(i) & !TAG;
                // Withdrawn host hints (`rearm`): dropped while muted.
                if p & st.muted[i] != 0 {
                    self.shared.note_muted(i, p & st.muted[i]);
                    p &= !st.muted[i];
                    bump(&self.stats.muted_raises);
                }
                if p != 0 {
                    let newly = p & !st.model.leaf[i];
                    st.again[i] |= p & st.model.leaf[i];
                    st.model.leaf[i] |= p;
                    bump(&self.stats.raised);
                    if newly != 0 {
                        raise_msi = true;
                    } else {
                        bump(&self.stats.coalesced);
                    }
                }
            }
        }
        between();
        // Write the displays: compare-and-swap from what was read, so a store that
        // landed since is kept for the next scan.
        if exact && (0..WATCHED).any(changed) {
            // Exact mode shows the registers' own values: a store that did not trap means
            // the page is writable after all; it cannot be told from the value it held.
            self.odd(&mut st, Odd::ExactStore);
        }
        for (i, &was) in w.iter().enumerate() {
            let d = st.model.display(i, exact);
            if was != d
                && self
                    .watched(i)
                    .compare_exchange(was, d, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
            {
                bump(&self.stats.cas_retries);
            }
            self.expected[i].store(d, Ordering::Relaxed);
        }
        // The re-arm: a set store while an enabled vector is pending.
        let edge = rearm_by_set && !raise_msi && st.model.top() != 0;
        let mut msi = false;
        if edge || raise_msi {
            // The page stores above come first; the eventfd write follows them.
            std::sync::atomic::fence(Ordering::SeqCst);
            msi = self.shared.signal_msi();
            if msi && raise_msi {
                crate::launch::msi_for_raise();
            }
            if msi {
                bump(if edge {
                    &self.stats.msi_edge
                } else {
                    &self.stats.msi_raise
                });
            } else {
                bump(&self.stats.msi_refused);
            }
        }
        if acked || msi {
            st.stale_since = None;
        }
        let ns = t0.elapsed().as_nanos() as u64;
        if ns > self.stats.slow_ns_max.load(Ordering::Relaxed) {
            self.stats.slow_ns_max.store(ns, Ordering::Relaxed);
        }
        Scan { work: true, msi }
    }

    /// The slow checks: boot constants, the tree's holes, the watchdog. Called about
    /// every millisecond by the reactor. Returns whether it signalled an MSI.
    pub fn tick(&self, now: Instant) -> bool {
        let mut st = self.lock();
        for (off, v) in self.boot {
            let w = self.pmc.word(off);
            let cur = w.load(Ordering::Acquire);
            if cur != v {
                let _ = w.compare_exchange(cur, v, Ordering::AcqRel, Ordering::Relaxed);
                self.odd(&mut st, Odd::BootStore);
            }
        }
        // Words of the tree [0, 0x800) that are no register read 0 when trapped.
        for off in (0..0x800u64).step_by(4) {
            if (0..WATCHED).any(|i| watched_offset(i) == off) {
                continue;
            }
            let w = self.tree.word(off);
            let cur = w.load(Ordering::Acquire);
            if cur != 0 {
                let _ = w.compare_exchange(cur, 0, Ordering::AcqRel, Ordering::Relaxed);
                self.odd(&mut st, Odd::TreeHole);
            }
        }
        let Some(period) = self.config.retrigger else {
            return false;
        };
        let m = &st.model;
        let exact = self.exact();
        // Exact mode sees every store, so no enable can be lost: only a vector pending and
        // enabled at both levels counts, and the MSI is sent again without touching the
        // enables the guest set.
        let enabled = if exact {
            m.top() & m.top_en != 0
        } else {
            (0..LEAVES).any(|i| m.leaf[i] & m.leaf_en[i] != 0)
        };
        if !enabled {
            st.stale_since = None;
            return false;
        }
        match st.stale_since {
            None => {
                st.stale_since = Some(now);
                false
            }
            Some(t) if now.saturating_duration_since(t) >= period => {
                // Late, not lost: every subtree enabled again (the tagged display's lost
                // enables), the MSI again.
                if !exact {
                    st.model.top_en |= SUBTREES;
                }
                let d = st.model.display(W_TOP_EN_SET, self.exact());
                let w = self.watched(W_TOP_EN_SET);
                let was = self.expected[W_TOP_EN_SET].load(Ordering::Relaxed);
                if w.compare_exchange(was, d, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    self.expected[W_TOP_EN_SET].store(d, Ordering::Relaxed);
                }
                st.stale_since = Some(now);
                std::sync::atomic::fence(Ordering::SeqCst);
                let msi = self.shared.signal_msi();
                bump(if msi {
                    &self.stats.msi_watchdog
                } else {
                    &self.stats.msi_refused
                });
                msi
            }
            Some(_) => false,
        }
    }

    /// One line for the exit summary.
    pub fn summary(&self) -> String {
        let s = &self.stats;
        let l = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let odd: Vec<String> = ODD_NAMES
            .iter()
            .zip(&s.odd)
            .filter(|(_, n)| l(n) > 0)
            .map(|(what, n)| format!("{} x {what}", l(n)))
            .collect();
        format!(
            "intr page: {} scan(s) with work (slowest {:.1} us); guest stores applied: {} ack(s), {} LEAF_EN_SET, {} LEAF_EN_CLEAR, {} TOP_EN_SET, {} TOP_EN_CLEAR, {} trigger(s); {} raise fold(s) ({} coalesced while pending, {} pended again at the ack); MSIs: {} for raises, {} re-arms (a set store while a vector is pending), {} by the watchdog, {} at an MSI re-arm ({} re-arm(s)), {} not written (MSI off or no eventfd); host non-stall hints withdrawn from a handler that re-armed without servicing them: {} ({} raise(s) muted); {} trapped access(es) served without the device mutex; {} compare-and-swap(s) lost to a guest store; inconsistencies: {}\n",
            l(&s.scans),
            l(&s.slow_ns_max) as f64 / 1000.0,
            l(&s.acks),
            l(&s.en_set),
            l(&s.en_clear),
            l(&s.top_en_set),
            l(&s.top_en_clear),
            l(&s.triggers),
            l(&s.raised),
            l(&s.coalesced),
            l(&s.again),
            l(&s.msi_raise),
            l(&s.msi_edge),
            l(&s.msi_watchdog),
            l(&s.msi_rearm),
            l(&s.rearms),
            l(&s.msi_refused),
            l(&s.withdrawn),
            l(&s.muted_raises),
            l(&s.routed),
            l(&s.cas_retries),
            if odd.is_empty() { "none".into() } else { odd.join(", ") }
        ) + &if self.shared.direct() {
            format!(
                "intr page direct: {} raise(s) folded and signalled on the raising thread ({DIRECT_ENV}=1)\n",
                l(&self.shared.direct_scans)
            )
        } else {
            String::new()
        }
    }
}

/// The reactor's side (poll.rs `Hook`): scan every iteration, the slow checks about every
/// millisecond, the wake eventfd that raises write.
pub struct PageHook {
    page: Arc<IntrPage>,
    /// The display page (`CARAXES_DISPLAY_PAGE=1`, display/fe_page.rs), scanned first.
    fe: Option<Arc<crate::display::fe_page::FePage>>,
    /// The MMU invalidate page (`CARAXES_INVALIDATE_PAGE=1`, inv_page.rs).
    inv: Option<(Arc<crate::inv_page::InvPage>, Arc<crate::inv_page::Driver>)>,
    /// The usermode page (`CARAXES_USERMODE_PAGE=1`, um_page.rs).
    um: Option<(Arc<crate::um_page::UmPage>, Arc<crate::um_page::Driver>)>,
    /// The display kick page (`CARAXES_DISPLAY_KICK_PAGE=1`, display/chan_page.rs).
    kicks: Option<(
        Arc<crate::display::chan_page::ChanPage>,
        Arc<crate::display::chan_page::Driver>,
    )>,
    /// `CARAXES_INTR_PAGE_VBLANK_SPIN_US`: spin from this long before each display tick
    /// (docs/caraxes/intr-page.md §11.4); 0 off.
    vblank_lead_ns: u64,
    /// From a display raise to the reactor's wake-up for it (a blocked reactor only).
    pub raise_wake: Arc<Mutex<crate::display::engine::Hist>>,
    wake: File,
    iterations: u32,
    last_tick: Instant,
}

impl PageHook {
    /// The hook and the writing end of its wake eventfd, which [`IntrShared::attach_page`]
    /// takes.
    pub fn new(page: Arc<IntrPage>) -> std::io::Result<(Self, File)> {
        // SAFETY: a plain syscall; checked.
        let raw = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if raw < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: a new descriptor that nothing else owns.
        let wake = unsafe { File::from_raw_fd(raw) };
        let writer = wake.try_clone()?;
        Ok((
            PageHook {
                page,
                fe: None,
                inv: None,
                um: None,
                kicks: None,
                vblank_lead_ns: 0,
                raise_wake: Arc::new(Mutex::new(Default::default())),
                wake,
                iterations: 0,
                last_tick: Instant::now(),
            },
            writer,
        ))
    }
}

impl PageHook {
    /// Scan the display page too, before the tree: a raise it makes after an ack is folded
    /// into the tree in the same iteration.
    pub fn with_display(mut self, fe: Arc<crate::display::fe_page::FePage>) -> Self {
        self.fe = Some(fe);
        self
    }

    /// Spin from `lead_us` before each display tick until it is served (0: off).
    pub fn with_vblank_spin(mut self, lead_us: u64) -> Self {
        self.vblank_lead_ns = lead_us * 1000;
        self
    }

    /// The vblank window: a display tick is due within the lead (the clock's next tick).
    fn vblank_window(&self) -> bool {
        if self.vblank_lead_ns == 0 || self.fe.is_none() {
            return false;
        }
        let t = crate::display::engine::ticker();
        t.next_due().is_some_and(|d| {
            crate::display::engine::now_ns() + self.vblank_lead_ns >= d
        })
    }

    /// Scan the MMU invalidate page and run `driver` for each trigger found.
    pub fn with_invalidate(
        mut self,
        page: Arc<crate::inv_page::InvPage>,
        driver: Arc<crate::inv_page::Driver>,
    ) -> Self {
        self.inv = Some((page, driver));
        self
    }

    /// Run the display kicks written to the window channels' USERD pages (`driver`).
    pub fn with_kicks(
        mut self,
        page: Arc<crate::display::chan_page::ChanPage>,
        driver: Arc<crate::display::chan_page::Driver>,
    ) -> Self {
        self.kicks = Some((page, driver));
        self
    }

    /// Own the usermode page: publish its clock every iteration, scan the channels for
    /// each doorbell written to it (`driver`).
    pub fn with_usermode(
        mut self,
        page: Arc<crate::um_page::UmPage>,
        driver: Arc<crate::um_page::Driver>,
    ) -> Self {
        self.um = Some((page, driver));
        self
    }

}

impl crate::poll::Hook for PageHook {
    fn poll(&mut self, woken: bool) -> bool {
        if woken && self.fe.is_some() {
            if let Some(t) = crate::display::engine::ticker().take_raise_for_wake() {
                let us = crate::display::engine::now_ns().saturating_sub(t) / 1000;
                self.raise_wake.lock().unwrap_or_else(PoisonError::into_inner).add(us);
            }
        }
        // An outstanding awaken marker keeps a hybrid reactor spinning, so the guest's
        // CORE_RESET is seen within one iteration (display/fe_page.rs).
        let mut work = self
            .fe
            .as_ref()
            .is_some_and(|f| f.scan() | f.busy(Instant::now()));
        work |= self.page.scan().work;
        // The invalidate settles an owed BAR2 flush itself (its trigger is a flush point).
        if let Some((p, d)) = &self.inv {
            work |= crate::inv_page::poll(p, d.as_ref(), woken);
        }
        if let Some((p, d)) = &self.um {
            work |= crate::um_page::poll(p, d.as_ref());
        }
        if let Some((p, d)) = &self.kicks {
            work |= crate::display::chan_page::poll(p, d.as_ref());
        }
        work |= self.vblank_window();
        self.iterations = self.iterations.wrapping_add(1);
        if woken || self.iterations % TICK_ITERATIONS == 0 {
            let now = Instant::now();
            if now.saturating_duration_since(self.last_tick) >= Duration::from_millis(1) {
                self.last_tick = now;
                work |= self.page.tick(now);
                if let Some((p, _)) = &self.kicks {
                    p.watchdog(now);
                }
                if let Some(f) = &self.fe {
                    work |= f.tick(now);
                }
            }
        }
        work
    }

    fn max_block_ms(&self) -> i32 {
        self.page.config.sleep_ms
    }

    fn wake_fd(&self) -> Option<RawFd> {
        Some(self.wake.as_raw_fd())
    }

    fn drain_wake(&mut self) {
        let mut b = [0u8; 8];
        let _ = std::io::Read::read(&mut self.wake, &mut b);
    }
}

/// For tests: an eventfd pair (writer, reader).
#[cfg(test)]
fn eventfd() -> (File, File) {
    // SAFETY: a plain syscall; checked.
    let raw = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
    assert!(raw >= 0);
    // SAFETY: a new descriptor.
    let f = unsafe { File::from_raw_fd(raw) };
    (f.try_clone().unwrap(), f)
}

#[cfg(test)]
fn count(f: &mut File) -> u64 {
    let mut b = [0u8; 8];
    match std::io::Read::read(f, &mut b) {
        Ok(8) => u64::from_ne_bytes(b),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: u64 = 0x00B8_1000;

    fn identity() -> crate::chip::BootIdentity {
        crate::fixture::rtx2070().chip.data().identity
    }

    /// A page with MSI on and an eventfd; returns the page, the shared state and the
    /// eventfd's reading end.
    fn page(config: Config) -> (Arc<IntrPage>, Arc<IntrShared>, File, File) {
        let shared = IntrShared::new();
        let (msi_w, msi_r) = eventfd();
        shared.set_msi_fd(Some(msi_w));
        shared.set_msi_enabled(true);
        let p = IntrPage::create(16 << 20, BASE, identity(), shared.clone(), config).unwrap();
        let (hook, wake) = PageHook::new(p.clone()).unwrap();
        shared.attach_page(wake);
        (p, shared, msi_r, hook.wake)
    }

    /// The guest's 32-bit store into the tree page.
    fn store(p: &IntrPage, reg: u64, v: u32) {
        p.tree.word(reg).store(v, Ordering::Release);
    }

    fn load(p: &IntrPage, reg: u64) -> u32 {
        p.tree.word(reg).load(Ordering::Acquire)
    }

    #[test]
    fn switch_and_settings() {
        assert_eq!(parse(None, None, None), Ok(None));
        assert_eq!(parse(Some("0"), None, None), Ok(None));
        assert_eq!(parse(Some("1"), None, None), Ok(Some(Config::default())));
        let c = parse(Some("1"), Some("5"), Some("0")).unwrap().unwrap();
        assert_eq!(c.sleep_ms, 5);
        assert_eq!(c.retrigger, None);
        assert!(parse(Some("yes"), None, None).is_err());
        assert!(parse(None, Some("5"), None).unwrap_err().contains("is off"));
        assert!(parse(Some("1"), Some("0"), None).is_err());
        assert!(parse(Some("1"), None, Some("-3")).is_err());
    }

    #[test]
    fn initial_displays_and_page_zero() {
        let (p, _, _, _) = page(Config::default());
        for i in 0..LEAVES as u64 {
            assert_eq!(load(&p, LEAF + 4 * i), TAG);
            assert_eq!(load(&p, LEAF_EN_SET + 4 * i), 0);
            assert_eq!(load(&p, LEAF_EN_CLEAR + 4 * i), 0);
        }
        assert_eq!(load(&p, TOP), 0);
        assert_eq!(load(&p, TOP_EN_SET), TAG);
        assert_eq!(load(&p, TOP_EN_CLEAR), 0);
        assert_eq!(load(&p, LEAF_TRIGGER), TRIGGER_IDLE);
        assert_eq!(p.read32(0), identity().boot_0);
        assert_eq!(p.read32(0xA00), identity().boot_42);
        assert_eq!(p.read32(0x160), 0);
        assert!(p.covers(0xFFC) && p.covers(BASE) && p.covers(BASE + 0xFFC));
        assert!(!p.covers(0x1000) && !p.covers(BASE - 4) && !p.covers(BASE + PAGE));
        assert_eq!(p.areas(), [(0, PAGE), (BASE, PAGE)]);
        assert!(!p.dirty());
        // Page 0 is memory: the RM lock's writes of 0 to 0x160/0x180 need nothing.
        p.write(0x160, &0u32.to_le_bytes());
        p.write(0x180, &0u32.to_le_bytes());
        assert!(!p.dirty());
        assert_eq!(p.scan(), Scan::default());
    }

    /// A raise from another thread: the page shows the bit before the MSI is written; a
    /// second raise before the fold is coalesced; the ack through the page clears it.
    #[test]
    fn raise_writes_the_page_then_the_msi_and_the_ack_clears_it() {
        let (p, shared, mut msi, mut wake) = page(Config::default());
        assert!(shared.raise(4));
        assert!(!shared.raise(4), "coalesced before the fold");
        assert_eq!(count(&mut wake), 1, "one wake for the first raise");
        assert_eq!(count(&mut msi), 0, "no MSI before the page is written");
        let s = p.scan();
        assert!(s.work && s.msi);
        assert_eq!(load(&p, LEAF), 0x10 | TAG);
        assert_eq!(count(&mut msi), 1);
        // The guest enables leaf 0 and the non-stall subtree (no edge MSI: the raise had
        // its MSI; the trapped tree makes this edge too).
        store(&p, LEAF_EN_SET, 0xFF);
        store(&p, TOP_EN_SET, 0x1);
        let s = p.scan();
        assert!(s.msi, "an edge of TOP & TOP_EN, as IntrTree::edge");
        assert_eq!(count(&mut msi), 1);
        assert_eq!(load(&p, TOP), 1);
        assert_eq!(load(&p, LEAF_EN_SET), 0xFF);
        assert_eq!(load(&p, TOP_EN_SET), 1 | TAG);
        // The ack: NVBIT(4), as intrClearLeafVector_TU102 writes it.
        store(&p, LEAF, 0x10);
        assert!(p.dirty());
        let s = p.scan();
        assert!(s.work && !s.msi);
        assert_eq!(load(&p, LEAF), TAG);
        assert_eq!(load(&p, TOP), 0);
        assert_eq!(p.model().leaf[0], 0);
        // The same vector again: a new MSI.
        assert!(shared.raise(4));
        assert!(p.scan().msi);
        assert_eq!(count(&mut msi), 1);
        assert_eq!(p.stats.acks.load(Ordering::Relaxed), 1);
        assert!(p.summary().contains("1 ack(s)"));
    }

    /// Exact mode (read-only mapping, Cloud Hypervisor's `x_readonly_mmap_areas`): the
    /// first trapped store to the tree switches the page to the registers' own values
    /// (dev_vm.h:31-67) and applies each store as it arrives: no tag, the clear registers
    /// read the enables, the enables start at 0, W1C acks, trigger, re-arm edges.
    #[test]
    fn trapped_stores_make_the_page_exact() {
        let (p, shared, mut msi, _wake) = page(Config::default());
        p.enable_vectors(&[4, 33]);
        assert!(!p.exact());
        assert_eq!(load(&p, LEAF), TAG);
        let w = |p: &IntrPage, reg: u64, v: u32| p.write(p.tree_base + reg, &v.to_le_bytes());
        // The first trapped store: the enables at their reset value, then this store.
        w(&p, LEAF_EN_SET, 0x10);
        assert!(p.exact());
        assert_eq!(p.model().leaf_en[0], 0x10);
        assert_eq!(p.model().leaf_en[1], 0, "the kernel table is not shown as enabled");
        assert_eq!(load(&p, LEAF), 0);
        assert_eq!(load(&p, LEAF_EN_SET), 0x10);
        assert_eq!(load(&p, LEAF_EN_CLEAR), 0x10, "EN_CLEAR reads the enables");
        assert_eq!(load(&p, LEAF_TRIGGER), 0);
        w(&p, TOP_EN_SET, 0x1);
        assert_eq!(load(&p, TOP_EN_SET), 1);
        assert_eq!(load(&p, TOP_EN_CLEAR), 1);
        assert_eq!(count(&mut msi), 0, "nothing pending: no edge");
        // A raise: the page shows the bit without a tag, and the MSI.
        assert!(shared.raise(4));
        assert!(p.scan().msi);
        assert_eq!(load(&p, LEAF), 0x10);
        assert_eq!(load(&p, TOP), 1);
        assert_eq!(count(&mut msi), 1);
        // A raise while pending, then the W1C ack: pended again with its MSI.
        assert!(shared.raise(4));
        assert!(!p.scan().msi);
        w(&p, LEAF, 0x10);
        assert_eq!(load(&p, LEAF), 0x10);
        assert_eq!(count(&mut msi), 1);
        w(&p, LEAF, 0x10);
        assert_eq!(load(&p, LEAF), 0);
        assert_eq!(load(&p, TOP), 0);
        // Masking the leaf, a trigger while masked, unmasking: the set store is an edge.
        w(&p, LEAF_EN_CLEAR, 0x10);
        assert_eq!(load(&p, LEAF_EN_SET), 0);
        w(&p, LEAF_TRIGGER, 4);
        assert_eq!(load(&p, LEAF), 0x10);
        assert_eq!(count(&mut msi), 1, "a trigger pends and signals");
        w(&p, LEAF_EN_SET, 0x10);
        assert_eq!(count(&mut msi), 1, "the unmask of a pending vector is an edge");
        w(&p, TOP_EN_CLEAR, 1);
        assert_eq!(load(&p, TOP_EN_SET), 0);
        // A byte store enables only its lane: bits 15:8 of LEAF_EN_SET(1).
        p.write(p.tree_base + LEAF_EN_SET + 4 + 1, &[0x02]);
        assert_eq!(p.model().leaf_en[1], 0x200);
        // A store that did not trap is an inconsistency, restored at the next scan.
        store(&p, LEAF, 0);
        assert!(p.scan().work);
        assert_eq!(load(&p, LEAF), 0x10);
        assert!(p.summary().contains("did not trap"));
        // A reset keeps exact mode, with the enables at 0, and drops a raise not yet folded
        // (the page's reactor had not scanned since).
        assert!(shared.raise(33));
        p.reset();
        assert!(p.exact());
        assert!(!p.scan().msi, "the raise went with the reset");
        assert_eq!(load(&p, LEAF + 4), 0);
        assert_eq!(load(&p, LEAF_EN_SET), 0);
    }

    /// The MSI re-arm (`kbifRearmMSI_GM107`): exact mode sends the MSI again while an
    /// enabled vector is pending, not otherwise; the tagged display leaves it to the watchdog.
    #[test]
    fn rearm_resends_the_msi_of_a_pending_enabled_vector() {
        let (p, shared, mut msi, _wake) = page(Config::default());
        assert!(!p.rearm(), "tagged display: no re-arm MSI");
        let w = |p: &IntrPage, reg: u64, v: u32| p.write(p.tree_base + reg, &v.to_le_bytes());
        w(&p, LEAF_EN_SET, 0x10);
        w(&p, TOP_EN_SET, 1);
        assert!(!p.rearm(), "nothing pending");
        assert!(shared.raise(4));
        p.scan();
        assert_eq!(count(&mut msi), 1);
        assert!(p.rearm(), "still pending after the service: the MSI again");
        assert_eq!(count(&mut msi), 1);
        w(&p, LEAF, 0x10);
        assert!(!p.rearm(), "acked");
        assert!(p.summary().contains("1 at an MSI re-arm"), "{}", p.summary());
    }

    /// A host hint (a host engine's non-stall event) that the guest's handler re-arms
    /// without servicing is withdrawn after `HINT_REARMS` re-arms and muted until the guest
    /// enables it again (C74, C78: the Windows KMD's reset path, 16-19 million MSIs).
    #[test]
    fn rearm_withdraws_a_host_hint_the_handler_does_not_service() {
        let (p, shared, mut msi, _wake) = page(Config::default());
        let w = |p: &IntrPage, reg: u64, v: u32| p.write(p.tree_base + reg, &v.to_le_bytes());
        p.set_hint_vectors(&[4, 5]);
        w(&p, LEAF_EN_SET, 0x30);
        w(&p, TOP_EN_SET, 1);
        assert!(shared.raise(4));
        p.scan();
        assert_eq!(count(&mut msi), 1);
        for n in 1..HINT_REARMS {
            assert!(p.rearm(), "re-arm {n}: the MSI again");
        }
        assert!(!p.rearm(), "withdrawn");
        assert_eq!(load(&p, LEAF), 0, "no longer pending");
        assert_eq!(p.stats.withdrawn.load(Ordering::Relaxed), 1);
        count(&mut msi);
        // Muted: a raise of the vector is dropped, one of the other hint is not.
        assert!(shared.raise(4));
        assert!(!p.scan().msi);
        assert_eq!(load(&p, LEAF), 0);
        assert_eq!(p.stats.muted_raises.load(Ordering::Relaxed), 1);
        assert!(shared.raise(5));
        assert!(p.scan().msi);
        assert_eq!(load(&p, LEAF), 0x20);
        w(&p, LEAF, 0x20);
        // The guest enables the vector again: its raises count again.
        w(&p, LEAF_EN_SET, 0x10);
        assert!(shared.raise(4));
        assert!(p.scan().msi);
        assert_eq!(load(&p, LEAF), 0x10);
        assert!(p.summary().contains("withdrawn from a handler that re-armed without servicing them: 1 (1 raise(s) muted)"), "{}", p.summary());
    }

    /// A handler that services the host hints is never withdrawn from, and a pending vector
    /// of the device's own (the display's) is sent again at every re-arm as before.
    #[test]
    fn rearm_keeps_serviced_hints_and_device_vectors() {
        let (p, shared, _msi, _wake) = page(Config::default());
        let w = |p: &IntrPage, reg: u64, v: u32| p.write(p.tree_base + reg, &v.to_le_bytes());
        p.set_hint_vectors(&[4]);
        w(&p, LEAF_EN_SET, 0x10);
        w(&p, LEAF_EN_SET + 16, 0x1000_0000);
        w(&p, TOP_EN_SET, 0x5);
        for _ in 0..2 * HINT_REARMS {
            assert!(shared.raise(4));
            p.scan();
            assert!(p.rearm());
            w(&p, LEAF, 0x10);
        }
        assert!(shared.raise(156));
        p.scan();
        for _ in 0..2 * HINT_REARMS {
            assert!(p.rearm(), "the display's vector stays");
        }
        assert_eq!(load(&p, LEAF + 16), 0x1000_0000);
        assert_eq!(p.stats.withdrawn.load(Ordering::Relaxed), 0);
    }

    /// Exact mode's watchdog leaves the guest's enables alone: a vector pending under a
    /// masked subtree is no lost interrupt; one pending and enabled gets its MSI again.
    /// (A raise signals as the trapped path's `IntrShared::raise` does, enables or not.)
    #[test]
    fn exact_watchdog_keeps_the_guests_enables() {
        let (p, shared, mut msi, _wake) = page(Config::default());
        let w = |p: &IntrPage, reg: u64, v: u32| p.write(p.tree_base + reg, &v.to_le_bytes());
        w(&p, LEAF_EN_SET, 0x10);
        assert!(shared.raise(4));
        p.scan();
        assert_eq!(count(&mut msi), 1, "the raise's MSI, as the trapped path");
        let t = Instant::now();
        assert!(!p.tick(t));
        assert!(!p.tick(t + Duration::from_millis(50)));
        assert_eq!(load(&p, TOP_EN_SET), 0, "the mask stays");
        w(&p, TOP_EN_SET, 1);
        assert_eq!(count(&mut msi), 1, "the unmask is the edge");
        assert!(!p.tick(t + Duration::from_millis(60)));
        assert!(p.tick(t + Duration::from_millis(80)), "late, not lost: the MSI again");
        assert_eq!(count(&mut msi), 1);
    }

    /// A host event for a vector the page still shows pending costs no MSI (series B on
    /// vm0 without this: 14825 host events became as many MSIs in one Mutter window and
    /// the compositor stalled); the ack that clears it pends it again with one MSI, so an
    /// event after the guest's read is not lost.
    #[test]
    fn raises_while_pending_coalesce_until_the_ack() {
        let (p, shared, mut msi, _) = page(Config::default());
        assert!(shared.raise(4));
        assert!(p.scan().msi);
        assert_eq!(count(&mut msi), 1);
        for _ in 0..100 {
            assert!(shared.raise(4));
            assert!(!p.scan().msi);
        }
        assert_eq!(count(&mut msi), 0);
        assert_eq!(p.stats.coalesced.load(Ordering::Relaxed), 100);
        // The ack: pended again, one MSI.
        store(&p, LEAF, 0x10);
        assert!(p.scan().msi);
        assert_eq!(count(&mut msi), 1);
        assert_eq!(load(&p, LEAF), 0x10 | TAG);
        // The second ack with nothing new: cleared, no MSI.
        store(&p, LEAF, 0x10);
        assert!(!p.scan().msi);
        assert_eq!(load(&p, LEAF), TAG);
        assert_eq!(p.stats.again.load(Ordering::Relaxed), 1);
    }

    /// The direct completion: a raise writes the page and then the MSI on the raising
    /// thread, with no wake of the reactor; a scan without a raise writes no MSI (nothing is
    /// reported before the host reported it); a raise while pending costs no second MSI;
    /// the switch is refused without the page; a dropped page falls back to the wake.
    #[test]
    fn direct_completion_orders_the_page_before_the_msi() {
        assert_eq!(direct_requested(None, false), Ok(false));
        assert_eq!(direct_requested(Some("1"), true), Ok(true));
        assert!(direct_requested(Some("1"), false).unwrap_err().contains("needs"));
        assert!(direct_requested(Some("2"), true).is_err());

        let (p, shared, mut msi, mut wake) = page(Config::default());
        assert!(shared.attach_direct(&p));
        assert!(!shared.attach_direct(&p), "set once");
        assert!(!p.scan().msi, "no raise, no MSI");
        assert_eq!(count(&mut msi), 0);
        assert!(shared.raise(4));
        assert_eq!(load(&p, LEAF), 0x10 | TAG, "the page shows the vector");
        assert_eq!(count(&mut msi), 1, "and the MSI is written, on this thread");
        assert_eq!(count(&mut wake), 0, "the reactor is not woken");
        assert!(shared.raise(4));
        assert_eq!(count(&mut msi), 0, "pending in the page: no second MSI");
        assert_eq!(shared.direct_scans.load(Ordering::Relaxed), 2);
        // The ack pends the second raise again, with its MSI, at the reactor's scan.
        store(&p, LEAF, 0x10);
        assert!(p.scan().msi);
        assert_eq!(count(&mut msi), 1);

        // Fail-closed: without the page the raise wakes the reactor as before.
        let (p2, shared2, mut msi2, mut wake2) = page(Config::default());
        assert!(shared2.attach_direct(&p2));
        drop(p2);
        assert!(shared2.raise(7));
        assert_eq!(count(&mut wake2), 1);
        assert_eq!(count(&mut msi2), 0);
    }

    /// The direct completion racing the reactor's scan: every raise ends with its vector in
    /// the page and exactly one MSI, whichever thread folded it, and the MSI never comes
    /// before the page shows the vector.
    #[test]
    fn direct_completion_racing_the_reactor_loses_nothing() {
        use std::sync::atomic::AtomicBool;
        let (p, shared, mut msi, _wake) = page(Config::default());
        assert!(shared.attach_direct(&p));
        let stop = Arc::new(AtomicBool::new(false));
        let reactor = {
            let (p, stop) = (p.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    p.scan();
                }
            })
        };
        for _ in 0..2000 {
            assert!(shared.raise(4));
            let t = Instant::now();
            let mut n = 0;
            while n == 0 {
                n = count(&mut msi);
                assert!(t.elapsed() < Duration::from_secs(5), "a raise lost its MSI");
            }
            assert_eq!(n, 1);
            assert_eq!(load(&p, LEAF) & 0x10, 0x10, "the MSI came after the page store");
            // The guest's ack; the reactor folds it without an MSI (nothing raised since).
            store(&p, LEAF, 0x10);
            let t = Instant::now();
            while load(&p, LEAF) & 0x10 != 0 {
                assert!(t.elapsed() < Duration::from_secs(5), "the ack was not folded");
            }
        }
        stop.store(true, Ordering::Relaxed);
        reactor.join().unwrap();
        assert_eq!(count(&mut msi), 0, "no MSI without a raise");
    }

    /// The leaf enables are reported, not modelled: the guest's per-vector enable loop
    /// at state load (eight stores to one register before one scan) still shows the whole
    /// table, a bit outside the table is learned, clears are dropped. A set store while a
    /// vector is pending is a re-arm (one MSI), also when a whole lock cycle lands between
    /// two scans; without a pending vector it is nothing.
    #[test]
    fn enables_are_reported_and_a_release_rearms() {
        let (p, shared, mut msi, _) = page(Config::default());
        p.enable_vectors(&[1, 2, 3, 4, 5, 6, 7, 0, 64, 65, 156, 192, 193]);
        assert_eq!(load(&p, LEAF_EN_SET), 0xFF);
        assert_eq!(load(&p, LEAF_EN_SET + 16), 0x1000_0000);
        for b in 0..8 {
            store(&p, LEAF_EN_SET, 1 << b);
        }
        store(&p, LEAF_EN_SET + 16, 1 << 1);
        assert!(!p.scan().msi, "nothing pending: no re-arm");
        assert_eq!(load(&p, LEAF_EN_SET), 0xFF);
        assert_eq!(
            load(&p, LEAF_EN_SET + 16),
            0x1000_0002,
            "the self-test's vector 129 learned"
        );
        store(&p, LEAF_EN_CLEAR + 16, 0xFFFF_FFFF);
        p.scan();
        assert_eq!(
            load(&p, LEAF_EN_SET + 16),
            0x1000_0002,
            "clears are dropped"
        );
        assert_eq!(load(&p, LEAF_EN_CLEAR + 16), 0);
        // The GSP vector pending, its raise MSI taken.
        assert!(shared.raise(192));
        assert!(p.scan().msi);
        assert_eq!(count(&mut msi), 1);
        assert_eq!(load(&p, TOP), 1 << 3);
        // A lock cycle between two scans: acquire, release (0xfffffffe then 0x1).
        store(&p, LEAF_EN_CLEAR + 8, 0x3);
        store(&p, TOP_EN_CLEAR, 0x8);
        store(&p, LEAF_EN_SET + 8, 0x3);
        store(&p, TOP_EN_SET, 0xFFFF_FFFE);
        store(&p, TOP_EN_SET, 0x1);
        assert!(p.scan().msi, "the release re-arms");
        assert_eq!(count(&mut msi), 1);
        assert_eq!(p.stats.msi_edge.load(Ordering::Relaxed), 1);
        // The ack; the next release has nothing pending.
        store(&p, LEAF + 24, 0x1);
        p.scan();
        store(&p, TOP_EN_SET, 0xFFFF_FFFE);
        assert!(!p.scan().msi);
        assert_eq!(count(&mut msi), 0);
        // Reset keeps the table's enables.
        p.reset();
        assert_eq!(load(&p, LEAF_EN_SET), 0xFF);
        assert_eq!(p.model().leaf, [0; LEAVES]);
    }

    #[test]
    fn fail_closed_cases() {
        let (p, _shared, mut msi, _) = page(Config::default());
        // A store to TOP is restored.
        store(&p, TOP, 0x5);
        p.scan();
        assert_eq!(load(&p, TOP), 0);
        // A trigger outside the vectors and one at a tag bit are ignored.
        store(&p, LEAF_TRIGGER, 0x7FF);
        p.scan();
        store(&p, LEAF_TRIGGER, 31);
        p.scan();
        assert_eq!(p.model().leaf, [0; LEAVES]);
        assert_eq!(load(&p, LEAF_TRIGGER), TRIGGER_IDLE);
        // A leaf store with the tag bit: the tag is ignored, the rest applied.
        store(&p, LEAF_TRIGGER, 5);
        p.scan();
        store(&p, LEAF_TRIGGER, 6);
        p.scan();
        assert_eq!(load(&p, LEAF), 0x60 | TAG);
        store(&p, LEAF, TAG | 0x20);
        p.scan();
        assert_eq!(p.model().leaf[0], 0x40);
        assert_eq!(load(&p, LEAF), 0x40 | TAG);
        // Boot constants and tree holes are restored at the tick.
        p.write(0, &0xFFFF_FFFFu32.to_le_bytes());
        store(&p, 0x614, 0xFFFF_FFFF);
        store(&p, 0x900, 0x1234);
        p.tick(Instant::now());
        assert_eq!(p.read32(0), identity().boot_0);
        assert_eq!(load(&p, 0x614), 0, "TOP_EN_CLEAR(1) is no register here");
        assert_eq!(
            load(&p, 0x900),
            0x1234,
            "beyond the tree the page is latched memory"
        );
        let odd: Vec<u64> = p
            .stats
            .odd
            .iter()
            .map(|a| a.load(Ordering::Relaxed))
            .collect();
        assert_eq!(odd, vec![1, 1, 0, 2, 1, 1, 0]);
        assert!(p
            .summary()
            .contains("inconsistencies: 1 x a store to the read-only TOP"));
        // MSI off: the vector stays pending and nothing is written.
        count(&mut msi);
        p.shared.set_msi_enabled(false);
        store(&p, LEAF_EN_SET, 0xFF);
        store(&p, TOP_EN_SET, 1);
        store(&p, LEAF_TRIGGER, 4);
        assert!(!p.scan().msi);
        assert_eq!(count(&mut msi), 0);
        assert_eq!(p.stats.msi_refused.load(Ordering::Relaxed), 1);
        // Reset: everything back.
        p.reset();
        assert_eq!(p.model(), Model::default());
        assert_eq!(load(&p, LEAF), TAG);
        assert!(!p.dirty());
    }

    /// Two TOP_EN_SET stores before one scan lose the first (the stall subtree stays
    /// disabled); the watchdog brings the pending GSP vector back as a late MSI.
    #[test]
    fn watchdog_turns_a_lost_enable_into_a_late_interrupt() {
        let cfg = Config {
            sleep_ms: 1,
            retrigger: Some(Duration::from_millis(10)),
        };
        let (p, shared, mut msi, _) = page(cfg);
        store(&p, LEAF_EN_SET + 24, 0x1);
        store(&p, TOP_EN_SET, 0xF);
        p.scan();
        store(&p, TOP_EN_CLEAR, 0x8);
        p.scan();
        // Release: 0xfffffffe then 0x1 before the reactor looks.
        store(&p, TOP_EN_SET, 0xFFFF_FFFE);
        store(&p, TOP_EN_SET, 0x1);
        assert!(shared.raise(192));
        let t0 = Instant::now();
        assert!(p.scan().msi, "the raise's own MSI");
        assert_eq!(count(&mut msi), 1);
        assert_eq!(p.model().top_en & 0x8, 0, "the lost enable");
        // Suppose the guest dropped that MSI (lock busy): nothing acks.
        assert!(!p.tick(t0));
        assert!(!p.tick(t0 + Duration::from_millis(9)));
        assert!(p.tick(t0 + Duration::from_millis(10)));
        assert_eq!(count(&mut msi), 1);
        assert_eq!(p.model().top_en & SUBTREES, SUBTREES);
        assert_eq!(load(&p, TOP_EN_SET), SUBTREES | TAG);
        // At most once per period.
        assert!(!p.tick(t0 + Duration::from_millis(15)));
        assert!(p.tick(t0 + Duration::from_millis(20)));
        // The ack ends it.
        store(&p, LEAF + 24, 0x1);
        p.scan();
        assert!(!p.tick(t0 + Duration::from_millis(40)));
        assert_eq!(p.stats.msi_watchdog.load(Ordering::Relaxed), 2);
        // Off: never.
        let (q, _, _, _) = page(Config {
            sleep_ms: 1,
            retrigger: None,
        });
        store(&q, LEAF_EN_SET, 1);
        store(&q, LEAF_TRIGGER, 0);
        q.scan();
        assert!(!q.tick(t0) && !q.tick(t0 + Duration::from_secs(1)));
    }

    /// A guest store that lands between the reactor's read and its write-back is kept
    /// and applied by the next scan.
    #[test]
    fn a_store_racing_the_write_back_is_kept() {
        let (p, shared, _msi, _) = page(Config::default());
        assert!(shared.raise(4));
        assert!(shared.raise(5));
        p.scan();
        assert_eq!(load(&p, LEAF), 0x30 | TAG);
        // The first ack is in the snapshot; the second lands after it, before the
        // write-back. The write-back must not overwrite it.
        store(&p, LEAF, 0x10);
        let s = p.scan_racing(&mut || store(&p, LEAF, 0x20));
        assert!(s.work);
        assert_eq!(p.stats.cas_retries.load(Ordering::Relaxed), 1);
        assert_eq!(load(&p, LEAF), 0x20, "the racing store is still there");
        assert_eq!(p.model().leaf[0], 0x20);
        assert!(p.dirty());
        p.scan();
        assert_eq!(p.model().leaf[0], 0);
        assert_eq!(load(&p, LEAF), TAG);
        assert_eq!(p.stats.acks.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn start_up_refusals() {
        let (p, _, _, _) = page(Config::default());
        let none = |_: u64| false;
        assert!(p.check(&none, 0x800, &[1, 4, 156, 192, 199]).is_ok());
        assert!(p
            .check(&none, 0x800, &[159])
            .unwrap_err()
            .contains("bit 31"));
        assert!(p.check(&none, 0x2000, &[]).is_err());
        let owns_pmc = |off: u64| off == 0x200;
        assert!(p
            .check(&owns_pmc, 0x800, &[])
            .unwrap_err()
            .contains("0x200"));
        // The tree's own registers and the boot constants are expected owners.
        let owns_tree = |off: u64| (BASE..BASE + 0x800).contains(&off) || off == 0;
        assert!(p.check(&owns_tree, 0x800, &[]).is_ok());
        let owns_after_tree = |off: u64| off == BASE + 0x900;
        assert!(p.check(&owns_after_tree, 0x800, &[]).is_err());
        assert!(IntrPage::create(
            16 << 20,
            BASE + 4,
            identity(),
            IntrShared::new(),
            Config::default()
        )
        .is_err());
        assert!(IntrPage::create(
            16 << 20,
            0,
            identity(),
            IntrShared::new(),
            Config::default()
        )
        .is_err());
    }

    /// The real TU106 model: no register of page 0 or of the rest of the tree page is
    /// owned by the boot model, and no table vector sits at a tag.
    #[test]
    fn tu106_pages_are_free() {
        let f = crate::fixture::rtx2070();
        let d = f.chip.data();
        let bar0 = crate::bar0::Bar0::new(f.chip, vec![0x55, 0xAA], None).with_boot_model(
            crate::boot::BootModel::new(
                f.chip,
                crate::fb::FbLayout::plain(4 << 30),
                f.identity(4 << 30),
            ),
        );
        let (p, _, _, _) = page(Config::default());
        let mut vectors: Vec<u32> = d
            .interrupts
            .entries
            .iter()
            .flat_map(|e| [e.vector_stall, e.vector_nonstall])
            .collect();
        vectors.push(156);
        p.check(
            &|off| bar0.model_owns(off),
            d.registers.intr_tree_size,
            &vectors,
        )
        .unwrap();
        assert_eq!(d.registers.intr_tree, BASE);
    }

    /// The hook in a real reactor: a spinning one applies an ack; a hybrid one that
    /// blocked wakes on a raise and writes the MSI.
    #[test]
    fn hook_in_the_reactor() {
        use crate::poll::{Config as PollConfig, Mode, Pin, Reactor};
        for mode in [
            Mode::Spin,
            Mode::Hybrid {
                idle: Duration::from_micros(50),
            },
        ] {
            let shared = IntrShared::new();
            let (msi_w, mut msi_r) = eventfd();
            shared.set_msi_fd(Some(msi_w));
            shared.set_msi_enabled(true);
            let p = IntrPage::create(
                16 << 20,
                BASE,
                identity(),
                shared.clone(),
                Config::default(),
            )
            .unwrap();
            let (hook, wake) = PageHook::new(p.clone()).unwrap();
            shared.attach_page(wake);
            // One doorbell eventfd, as the token pool would give.
            let (_db_w, db_r) = eventfd();
            let r = Reactor::new(
                "t",
                PollConfig {
                    mode,
                    pin: Pin::None,
                },
                None,
            )
            .unwrap();
            let stats = r
                .spawn_hooked(vec![db_r], |_, _, _| {}, Some(Box::new(hook)))
                .unwrap();
            std::thread::sleep(Duration::from_millis(5));
            assert!(shared.raise(4));
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut got = 0;
            while got == 0 && Instant::now() < deadline {
                got = count(&mut msi_r);
                std::thread::yield_now();
            }
            assert_eq!(got, 1, "{mode:?}");
            assert_eq!(load(&p, LEAF), 0x10 | TAG);
            store(&p, LEAF, 0x10);
            let deadline = Instant::now() + Duration::from_secs(5);
            while load(&p, LEAF) != TAG && Instant::now() < deadline {
                std::thread::yield_now();
            }
            assert_eq!(load(&p, LEAF), TAG, "{mode:?}: the ack was applied");
            stats.stop.store(true, Ordering::Relaxed);
            assert!(shared.raise(4), "wakes a blocked reactor so it can stop");
        }
    }
}

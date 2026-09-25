// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
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

use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
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

    /// The word the page shows for watched word `i`.
    fn display(&self, i: usize) -> u32 {
        match i {
            i if i < W_EN_SET => self.leaf[i] | TAG,
            i if i < W_EN_CLEAR => self.leaf_en[i - W_EN_SET],
            i if i < W_TOP => 0,
            W_TOP => self.top(),
            W_TOP_EN_SET => (self.top_en & SUBTREES) | TAG,
            W_TOP_EN_CLEAR => 0,
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
}

const ODD_NAMES: [&str; 6] = [
    "a store to the read-only TOP register (restored)",
    "a leaf store with the tag bit 31 (bit ignored)",
    "a LEAF_EN_SET store with the tag bit 31 (bit ignored)",
    "a LEAF_TRIGGER store outside the 256 vectors or at a tag bit (ignored)",
    "a store to a boot constant in page 0 (restored)",
    "a store to a word of the tree that is no register (restored to 0)",
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
}

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
    pub odd: [AtomicU64; 6],
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
            }),
            expected: std::array::from_fn(|_| AtomicU32::new(0)),
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
        st.model = Model {
            leaf_en: st.table_en,
            ..Model::default()
        };
        st.again = [0; LEAVES];
        st.stale_since = None;
        for off in (0..PAGE).step_by(4) {
            self.pmc.word(off).store(0, Ordering::Relaxed);
            self.tree.word(off).store(0, Ordering::Relaxed);
        }
        for (off, v) in self.boot {
            self.pmc.word(off).store(v, Ordering::Relaxed);
        }
        for i in 0..WATCHED {
            let d = st.model.display(i);
            self.watched(i).store(d, Ordering::Relaxed);
            self.expected[i].store(d, Ordering::Relaxed);
        }
        std::sync::atomic::fence(Ordering::SeqCst);
    }

    /// A trapped read of the pages (a client that did not map them): the memory.
    pub fn read32(&self, off: u64) -> u32 {
        self.word_at(off).load(Ordering::Acquire)
    }

    /// A trapped write: stored as the guest would store it, found by the next scan.
    pub fn write(&self, off: u64, data: &[u8]) {
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
        // 1. Enable stores. The leaf enables are reported, not modelled (docs/caraxes/
        // intr-page.md §3): the page shows every vector of the kernel table and every bit
        // the guest ever set, because the guest's per-vector enable loops at state load
        // store faster than any scan and lose all but the last value (series B on vm0:
        // LEAF_EN_SET(0) read 0x1 instead of 0xff, the non-stall vectors never showed in
        // TOP, the compositor stalled). Clears are counted and dropped. Every set store is
        // the guest re-enabling (a lock release, intrSetDisplayInterruptEnable,
        // intrEnableTopNonstall): a re-arm, as the edge of the trapped tree is.
        let mut rearm_by_set = false;
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
        let mut acked = false;
        let mut rearm = false;
        for i in 0..LEAVES {
            let v = w[W_LEAF + i];
            if changed(W_LEAF + i) {
                if v & TAG != 0 {
                    self.odd(&mut st, Odd::TagInLeaf);
                }
                let mask = v & !TAG;
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
        // 5. Vectors raised by other threads; their MSI is the raise's, not an edge.
        // A vector still pending in the page costs no second MSI (the guest has not
        // acked it; `IntrShared::raise` coalesces the same way while the guest has not
        // looked); it is remembered and pended again at its ack.
        let mut raise_msi = rearm;
        if self.shared.page_raised.swap(false, Ordering::AcqRel) {
            self.shared.page_msi.store(false, Ordering::Release);
            for i in 0..LEAVES {
                let p = self.shared.take_pending(i) & !TAG;
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
        for (i, &was) in w.iter().enumerate() {
            let d = st.model.display(i);
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
        let enabled = (0..LEAVES).any(|i| m.leaf[i] & m.leaf_en[i] != 0);
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
                // Late, not lost: every subtree enabled again, the MSI again.
                st.model.top_en |= SUBTREES;
                let d = st.model.display(W_TOP_EN_SET);
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
            "intr page: {} scan(s) with work (slowest {:.1} us); guest stores applied: {} ack(s), {} LEAF_EN_SET, {} LEAF_EN_CLEAR, {} TOP_EN_SET, {} TOP_EN_CLEAR, {} trigger(s); {} raise fold(s) ({} coalesced while pending, {} pended again at the ack); MSIs: {} for raises, {} re-arms (a set store while a vector is pending), {} by the watchdog, {} not written (MSI off or no eventfd); {} compare-and-swap(s) lost to a guest store; inconsistencies: {}\n",
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
            l(&s.msi_refused),
            l(&s.cas_retries),
            if odd.is_empty() { "none".into() } else { odd.join(", ") }
        )
    }
}

/// The reactor's side (poll.rs `Hook`): scan every iteration, the slow checks about every
/// millisecond, the wake eventfd that raises write.
pub struct PageHook {
    page: Arc<IntrPage>,
    /// The display page (`CARAXES_DISPLAY_PAGE=1`, display/fe_page.rs), scanned first.
    fe: Option<Arc<crate::display::fe_page::FePage>>,
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
}

impl crate::poll::Hook for PageHook {
    fn poll(&mut self, woken: bool) -> bool {
        // An outstanding awaken marker keeps a hybrid reactor spinning, so the guest's
        // CORE_RESET is seen within one iteration (display/fe_page.rs).
        let mut work = self
            .fe
            .as_ref()
            .is_some_and(|f| f.scan() | f.busy(Instant::now()));
        work |= self.page.scan().work;
        self.iterations = self.iterations.wrapping_add(1);
        if woken || self.iterations % TICK_ITERATIONS == 0 {
            let now = Instant::now();
            if now.saturating_duration_since(self.last_tick) >= Duration::from_millis(1) {
                self.last_tick = now;
                work |= self.page.tick(now);
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
        shared.msi_enabled.store(true, Ordering::Relaxed);
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
        assert_eq!(odd, vec![1, 1, 0, 2, 1, 1]);
        assert!(p
            .summary()
            .contains("inconsistencies: 1 x a store to the read-only TOP"));
        // MSI off: the vector stays pending and nothing is written.
        count(&mut msi);
        p.shared.msi_enabled.store(false, Ordering::Relaxed);
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
            shared.msi_enabled.store(true, Ordering::Relaxed);
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

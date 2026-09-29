// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The CPU interrupt tree of the virtual-function window (docs/caraxes/
//! kernel-rm-hw-surface.md §C, `dev_vm.h`): 8 leaf registers of 32 vectors,
//! leaf enables, one top register whose bit s summarizes subtree s (leaves 2s
//! and 2s+1), and a top enable. `LEAF_TRIGGER` raises a vector by software; the
//! driver's environment self-test fires vector 129 and expects an MSI. The tree's
//! base is the chip's (`registers.intr_tree`); the layout below is relative to it.

use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

pub(crate) const LEAF: u64 = 0x000;
pub(crate) const LEAF_EN_SET: u64 = 0x200;
pub(crate) const LEAF_EN_CLEAR: u64 = 0x400;
pub(crate) const TOP: u64 = 0x600;
pub(crate) const TOP_EN_SET: u64 = 0x608;
pub(crate) const TOP_EN_CLEAR: u64 = 0x610;
pub(crate) const LEAF_TRIGGER: u64 = 0x640;
pub const LEAVES: usize = 8;

/// Vectors raised from outside the server thread (E5b part 2: the host OS-event thread
/// of `mirror.rs` turns a GR0/CE non-stall interrupt of the physical GPU into the
/// guest's vector, cuda-path.md §5.2). The thread sets the bit here and signals the MSI
/// eventfd itself; the tree folds the bits into its leaves at the guest's next access.
#[derive(Default)]
pub struct IntrShared {
    pending: [AtomicU32; LEAVES],
    msi_fd: Mutex<Option<File>>,
    msi_enabled: AtomicBool,
    /// Serializes raises with changes that make MSI delivery available.
    delivery: Mutex<()>,
    /// Counters for the summary: vectors raised, MSIs written, host RC events seen.
    pub raised: AtomicU64,
    pub msis: AtomicU64,
    pub coalesced: AtomicU64,
    pub rc_events: AtomicU64,
    /// Lines the event thread wants in the trace (drained by the server thread).
    pub notes: Mutex<Vec<String>>,
    /// The interrupt page (`CARAXES_INTR_PAGE=1`, intr_page.rs): when set, a raise does
    /// not write the MSI itself but hands the vector to the reactor, which writes the
    /// page first and then the MSI.
    page: Mutex<Option<File>>,
    page_mode: AtomicBool,
    /// A vector was raised since the reactor last folded them in.
    pub(crate) page_raised: AtomicBool,
    /// A raise asked for an MSI (the trapped path would have written one).
    pub(crate) page_msi: AtomicBool,
    /// The direct completion (`CARAXES_INTR_DIRECT=1`, intr_page.rs `DIRECT_ENV`): the
    /// raising thread folds the vector into the page and writes the MSI itself, in the
    /// page's order (page stores, then the eventfd), instead of waking the reactor.
    direct: OnceLock<Weak<crate::intr_page::IntrPage>>,
    /// Raises the direct completion folded on the raising thread.
    pub direct_scans: AtomicU64,
}

impl IntrShared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Keep a duplicate of the client's MSI eventfd for raising from the thread.
    pub fn set_msi_fd(&self, fd: Option<File>) {
        let _delivery = self.delivery.lock().unwrap();
        let mut slot = self.msi_fd.lock().unwrap();
        let became_available = slot.is_none() && fd.is_some();
        *slot = fd;
        drop(slot);
        if became_available && !self.page_mode.load(Ordering::SeqCst) && self.pending() {
            self.signal_msi_locked();
        }
    }

    /// Mirror the PCI MSI enable bit, replaying one MSI when pending work becomes
    /// deliverable. Pending bits stay set until the guest reads the interrupt tree.
    pub fn set_msi_enabled(&self, enabled: bool) {
        let _delivery = self.delivery.lock().unwrap();
        let was = self.msi_enabled.swap(enabled, Ordering::SeqCst);
        if enabled
            && !was
            && !self.page_mode.load(Ordering::SeqCst)
            && self.pending()
        {
            self.signal_msi_locked();
        }
    }

    /// Function-level reset: the vectors raised before it and not yet folded into the
    /// tree (or read by the guest) go with the rest of the interrupt state. Returns the
    /// vectors dropped, per leaf, for the trace.
    pub fn reset_pending(&self) -> [u32; LEAVES] {
        let _delivery = self.delivery.lock().unwrap();
        self.page_raised.store(false, Ordering::SeqCst);
        self.page_msi.store(false, Ordering::SeqCst);
        std::array::from_fn(|i| self.pending[i].swap(0, Ordering::SeqCst))
    }

    fn pending(&self) -> bool {
        self.pending
            .iter()
            .any(|p| p.load(Ordering::SeqCst) != 0)
    }

    /// Raise `vector` from any thread: mark it pending and write the MSI eventfd when
    /// the guest has MSI enabled. A vector that is already pending (the guest has not
    /// read its leaf since the last raise) costs no second MSI: a host engine's
    /// non-stall interrupt fires for every client's work on that engine (31927 GR0
    /// events in three minutes from the host desktop, measured 2026-09-23), and the
    /// guest kernel re-checks all its waiters on one interrupt anyway. Returns whether
    /// an MSI was written.
    pub fn raise(&self, vector: u32) -> bool {
        use std::io::Write;
        let delivery = self.delivery.lock().unwrap();
        let leaf = (vector / 32) as usize;
        self.raised.fetch_add(1, Ordering::Relaxed);
        crate::profile::window("interrupts: vector raises", 0);
        if leaf < LEAVES {
            let bit = 1u32 << (vector % 32);
            if self.pending[leaf].fetch_or(bit, Ordering::SeqCst) & bit != 0 {
                self.coalesced.fetch_add(1, Ordering::Relaxed);
                crate::profile::window("interrupts: raises coalesced while pending", 0);
                return false;
            }
        }
        if self.page_mode.load(Ordering::SeqCst) {
            // The reactor folds the vector into the page and signals the MSI after it.
            self.page_msi.store(true, Ordering::SeqCst);
            self.page_raised.store(true, Ordering::SeqCst);
            // The direct completion: the reactor's scan on this thread. The page's mutex
            // orders it against the reactor's; whichever takes the raised flag writes the
            // page and then the MSI. Without the page (dropped): the wake below.
            if let Some(p) = self.direct.get().and_then(Weak::upgrade) {
                // The scan signals the MSI through `signal_msi`, which takes `delivery`
                // itself. In page mode no availability change replays a pending vector
                // (`set_msi_fd`, `set_msi_enabled`), so the guard has done its job here.
                drop(delivery);
                p.scan();
                self.direct_scans.fetch_add(1, Ordering::Relaxed);
                return true;
            }
            if let Some(w) = self.page.lock().unwrap().as_mut() {
                let _ = w.write_all(&1u64.to_ne_bytes());
            }
            return true;
        }
        self.signal_msi_locked()
    }

    /// Hand raises to the interrupt page's reactor from now on: `wake` is an eventfd in
    /// the reactor's epoll set (intr_page.rs).
    pub fn attach_page(&self, wake: File) {
        *self.page.lock().unwrap() = Some(wake);
        self.page_mode.store(true, Ordering::SeqCst);
    }

    /// Fold raises into `page` on the raising thread from now on (the direct completion).
    /// Needs [`IntrShared::attach_page`] first; false when refused or already set.
    pub fn attach_direct(&self, page: &Arc<crate::intr_page::IntrPage>) -> bool {
        self.page_mode() && self.direct.set(Arc::downgrade(page)).is_ok()
    }

    /// Whether the direct completion is attached.
    pub fn direct(&self) -> bool {
        self.direct.get().is_some()
    }

    /// Whether raises go to the interrupt page.
    pub fn page_mode(&self) -> bool {
        self.page_mode.load(Ordering::Relaxed)
    }

    /// Write the MSI eventfd when the guest has MSI enabled; whether it was written.
    pub fn signal_msi(&self) -> bool {
        let _delivery = self.delivery.lock().unwrap();
        self.signal_msi_locked()
    }

    fn signal_msi_locked(&self) -> bool {
        use std::io::Write;
        if !self.msi_enabled.load(Ordering::Relaxed) {
            return false;
        }
        let mut g = self.msi_fd.lock().unwrap();
        let Some(f) = g.as_mut() else { return false };
        if f.write_all(&1u64.to_le_bytes()).is_ok() {
            self.msis.fetch_add(1, Ordering::Relaxed);
            crate::profile::window("interrupts: MSIs written", 0);
            true
        } else {
            false
        }
    }

    pub fn note(&self, s: String) {
        self.notes.lock().unwrap().push(s);
    }

    pub fn take_notes(&self) -> Vec<String> {
        std::mem::take(&mut *self.notes.lock().unwrap())
    }

    pub(crate) fn take_pending(&self, leaf: usize) -> u32 {
        self.pending[leaf].swap(0, Ordering::SeqCst)
    }
}

pub struct IntrTree {
    base: u64,
    size: u64,
    leaf: [u32; LEAVES],
    leaf_en: [u32; LEAVES],
    top_en: u32,
    /// Set when a change made `TOP & TOP_EN` non-zero (edge); the device turns it into an MSI.
    msi_pending: bool,
    shared: Option<Arc<IntrShared>>,
}

impl IntrTree {
    pub fn new(base: u64, size: u64) -> Self {
        IntrTree {
            base,
            size,
            leaf: [0; LEAVES],
            leaf_en: [0; LEAVES],
            top_en: 0,
            msi_pending: false,
            shared: None,
        }
    }

    /// Fold vectors raised from other threads into the leaves on every access.
    pub fn with_shared(mut self, shared: Arc<IntrShared>) -> Self {
        self.shared = Some(shared);
        self
    }

    fn merge(&mut self) {
        if let Some(s) = &self.shared {
            for i in 0..LEAVES {
                self.leaf[i] |= s.take_pending(i);
            }
        }
    }

    pub fn handles(&self, off: u64) -> bool {
        (self.base..self.base + self.size).contains(&off)
    }

    /// Pending subtrees: bit s when an enabled vector of leaf 2s or 2s+1 is pending.
    pub fn top(&self) -> u32 {
        let mut t = 0u32;
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

    fn asserted(&self) -> bool {
        self.top() & self.top_en != 0
    }

    fn edge(&mut self, before: bool) {
        if !before && self.asserted() {
            self.msi_pending = true;
        }
    }

    /// Raise `vector` (0..255).
    pub fn trigger(&mut self, vector: u32) {
        let before = self.asserted();
        let leaf = (vector / 32) as usize;
        if leaf < LEAVES {
            self.leaf[leaf] |= 1 << (vector % 32);
        }
        self.edge(before);
    }

    pub fn take_msi(&mut self) -> bool {
        std::mem::take(&mut self.msi_pending)
    }

    pub fn read32(&mut self, off: u64) -> u32 {
        self.merge();
        let r = off - self.base;
        match r {
            o if (LEAF..LEAF + 4 * LEAVES as u64).contains(&o) => {
                self.leaf[((o - LEAF) / 4) as usize]
            }
            o if (LEAF_EN_SET..LEAF_EN_SET + 4 * LEAVES as u64).contains(&o) => {
                self.leaf_en[((o - LEAF_EN_SET) / 4) as usize]
            }
            o if (LEAF_EN_CLEAR..LEAF_EN_CLEAR + 4 * LEAVES as u64).contains(&o) => {
                self.leaf_en[((o - LEAF_EN_CLEAR) / 4) as usize]
            }
            TOP => self.top(),
            TOP_EN_SET | TOP_EN_CLEAR => self.top_en,
            _ => 0,
        }
    }

    pub fn write32(&mut self, off: u64, v: u32) -> Option<String> {
        self.merge();
        let r = off - self.base;
        let before = self.asserted();
        let note = match r {
            o if (LEAF..LEAF + 4 * LEAVES as u64).contains(&o) => {
                // Write 1 to clear.
                self.leaf[((o - LEAF) / 4) as usize] &= !v;
                None
            }
            o if (LEAF_EN_SET..LEAF_EN_SET + 4 * LEAVES as u64).contains(&o) => {
                self.leaf_en[((o - LEAF_EN_SET) / 4) as usize] |= v;
                None
            }
            o if (LEAF_EN_CLEAR..LEAF_EN_CLEAR + 4 * LEAVES as u64).contains(&o) => {
                self.leaf_en[((o - LEAF_EN_CLEAR) / 4) as usize] &= !v;
                None
            }
            TOP_EN_SET => {
                self.top_en |= v;
                None
            }
            TOP_EN_CLEAR => {
                self.top_en &= !v;
                None
            }
            LEAF_TRIGGER => {
                let vector = v & 0xFFF;
                self.trigger(vector);
                Some(format!(
                    "intr trigger vector {vector}: leaf {} bit {} (enabled {}, top {:#x} & en {:#x})",
                    vector / 32,
                    vector % 32,
                    (self.leaf_en.get((vector / 32) as usize).copied().unwrap_or(0) >> (vector % 32)) & 1 == 1,
                    self.top(),
                    self.top_en
                ))
            }
            _ => None,
        };
        self.edge(before);
        note
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    const BASE: u64 = 0x00B8_1000;

    fn eventfd() -> (File, File) {
        // SAFETY: a fresh eventfd, immediately wrapped in an owned File.
        let fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK) };
        assert!(fd >= 0);
        // SAFETY: `fd` is fresh and owned by this test.
        let writer = unsafe { <File as std::os::fd::FromRawFd>::from_raw_fd(fd) };
        (writer.try_clone().unwrap(), writer)
    }

    fn read_eventfd(mut reader: File) -> u64 {
        let mut b = [0u8; 8];
        assert_eq!(reader.read(&mut b).unwrap(), 8);
        u64::from_le_bytes(b)
    }

    #[test]
    fn pending_vector_is_replayed_once_when_msi_is_enabled() {
        let shared = IntrShared::new();
        let (reader, writer) = eventfd();
        shared.set_msi_fd(Some(writer));

        assert!(!shared.raise(4));
        shared.set_msi_enabled(true);
        // Repeating the same state transition must not duplicate the replay.
        shared.set_msi_enabled(true);
        assert_eq!(read_eventfd(reader), 1);
        assert_eq!(shared.msis.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn pending_vector_is_replayed_once_when_eventfd_is_installed() {
        let shared = IntrShared::new();
        shared.set_msi_enabled(true);
        assert!(!shared.raise(4));

        let (reader, writer) = eventfd();
        shared.set_msi_fd(Some(writer));
        assert_eq!(read_eventfd(reader), 1);
        assert_eq!(shared.msis.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn transition_before_raise_delivers_only_the_raise() {
        // Together with the two replay-order tests above, this covers both sides of the
        // race. `delivery` serializes the pending fetch_or with each availability change:
        // the transition either observes the bit and replays it, or the later raise
        // observes the available MSI and delivers it itself.
        let shared = IntrShared::new();
        let (reader, writer) = eventfd();
        shared.set_msi_fd(Some(writer));
        shared.set_msi_enabled(true);
        assert!(shared.raise(4));
        assert_eq!(read_eventfd(reader), 1);
        assert_eq!(shared.msis.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn self_test_vector_raises_msi_when_enabled() {
        let mut t = IntrTree::new(BASE, 0x800);
        assert!(t.handles(BASE + 0x7FC) && !t.handles(BASE + 0x800));
        t.write32(BASE + LEAF_TRIGGER, 129);
        assert_eq!(t.read32(BASE + LEAF + 16), 1 << 1);
        assert_eq!(t.read32(BASE + TOP), 0);
        assert!(!t.take_msi());
        t.write32(BASE + LEAF_EN_SET + 16, 1 << 1);
        assert_eq!(t.read32(BASE + TOP), 1 << 2);
        assert!(!t.take_msi());
        t.write32(BASE + TOP_EN_SET, 1 << 2);
        assert!(t.take_msi());
        assert!(!t.take_msi());
        t.write32(BASE + LEAF + 16, 1 << 1);
        assert_eq!(t.read32(BASE + TOP), 0);
        t.write32(BASE + LEAF_TRIGGER, 129);
        assert!(t.take_msi());
        t.write32(BASE + LEAF_EN_CLEAR + 16, 0xFFFF_FFFF);
        assert_eq!(t.read32(BASE + LEAF_EN_SET + 16), 0);
        t.write32(BASE + TOP_EN_CLEAR, 0xFFFF_FFFF);
        assert_eq!(t.read32(BASE + TOP_EN_SET), 0);
    }

    /// A vector raised from another thread lands in its leaf at the next access and
    /// writes the MSI eventfd when MSI is enabled.
    #[test]
    fn shared_vectors_reach_the_leaves_and_the_msi() {
        let shared = IntrShared::new();
        let mut t = IntrTree::new(BASE, 0x800).with_shared(shared.clone());
        // No MSI fd, MSI disabled: pending only; a second raise before the guest reads
        // the leaf is coalesced.
        assert!(!shared.raise(5));
        assert!(!shared.raise(5));
        assert_eq!(shared.coalesced.load(Ordering::Relaxed), 1);
        assert_eq!(t.read32(BASE + LEAF), 1 << 5);
        assert_eq!(shared.raised.load(Ordering::Relaxed), 2);
        // With an eventfd and MSI enabled, the write lands.
        // SAFETY: a fresh eventfd owned by the test.
        let efd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK) };
        assert!(efd >= 0);
        // SAFETY: `efd` is a valid fd we own.
        shared.set_msi_fd(Some(unsafe {
            <File as std::os::fd::FromRawFd>::from_raw_fd(efd)
        }));
        shared.set_msi_enabled(true);
        let raised = std::thread::spawn({
            let s = shared.clone();
            move || s.raise(6)
        })
        .join()
        .unwrap();
        assert!(raised);
        let mut b = [0u8; 8];
        // SAFETY: reading our own eventfd through a duplicated fd.
        let mut f = unsafe { <File as std::os::fd::FromRawFd>::from_raw_fd(libc::dup(efd)) };
        assert_eq!(f.read(&mut b).unwrap(), 8);
        assert_eq!(u64::from_le_bytes(b), 1);
        t.write32(BASE + LEAF_EN_SET, 1 << 6);
        assert_eq!(t.read32(BASE + LEAF) & (1 << 6), 1 << 6);
        assert_eq!(t.read32(BASE + TOP), 1);
        // Write-1-to-clear takes it away; nothing pending remains.
        t.write32(BASE + LEAF, 1 << 6);
        assert_eq!(t.read32(BASE + LEAF) & (1 << 6), 0);
        assert_eq!(shared.msis.load(Ordering::Relaxed), 1);
        shared.note("x".into());
        assert_eq!(shared.take_notes(), vec!["x".to_string()]);
        assert!(shared.take_notes().is_empty());
    }

    /// A function-level reset drops the vectors raised before it: after the reset the
    /// guest's tree shows none of them (C64: a display raise from before the FLR).
    #[test]
    fn a_reset_drops_the_raised_vectors() {
        let shared = IntrShared::new();
        assert!(!shared.raise(6), "no MSI without an eventfd");
        assert!(!shared.raise(40));
        let dropped = shared.reset_pending();
        assert_eq!(dropped[0], 1 << 6);
        assert_eq!(dropped[1], 1 << 8);
        assert!(!shared.pending());
        let mut t = IntrTree::new(BASE, 0x1000).with_shared(shared.clone());
        t.write32(BASE + LEAF_EN_SET, 1 << 6);
        assert_eq!(t.read32(BASE + LEAF) & (1 << 6), 0);
    }
}

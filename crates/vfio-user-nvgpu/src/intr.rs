// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The CPU interrupt tree of the virtual-function window (docs/caraxes/
//! kernel-rm-hw-surface.md §C, `dev_vm.h`): 8 leaf registers of 32 vectors,
//! leaf enables, one top register whose bit s summarizes subtree s (leaves 2s
//! and 2s+1), and a top enable. `LEAF_TRIGGER` raises a vector by software; the
//! driver's environment self-test fires vector 129 and expects an MSI. The tree's
//! base is the chip's (`registers.intr_tree`); the layout below is relative to it.

use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const LEAF: u64 = 0x000;
const LEAF_EN_SET: u64 = 0x200;
const LEAF_EN_CLEAR: u64 = 0x400;
const TOP: u64 = 0x600;
const TOP_EN_SET: u64 = 0x608;
const TOP_EN_CLEAR: u64 = 0x610;
const LEAF_TRIGGER: u64 = 0x640;
pub const LEAVES: usize = 8;

/// Vectors raised from outside the server thread (E5b part 2: the host OS-event thread
/// of `mirror.rs` turns a GR0/CE non-stall interrupt of the physical GPU into the
/// guest's vector, cuda-path.md §5.2). The thread sets the bit here and signals the MSI
/// eventfd itself; the tree folds the bits into its leaves at the guest's next access.
#[derive(Default)]
pub struct IntrShared {
    pending: [AtomicU32; LEAVES],
    msi_fd: Mutex<Option<File>>,
    pub msi_enabled: AtomicBool,
    /// Counters for the summary: vectors raised, MSIs written, host RC events seen.
    pub raised: AtomicU64,
    pub msis: AtomicU64,
    pub coalesced: AtomicU64,
    pub rc_events: AtomicU64,
    /// Lines the event thread wants in the trace (drained by the server thread).
    pub notes: Mutex<Vec<String>>,
}

impl IntrShared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Keep a duplicate of the client's MSI eventfd for raising from the thread.
    pub fn set_msi_fd(&self, fd: Option<File>) {
        *self.msi_fd.lock().unwrap() = fd;
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
        let leaf = (vector / 32) as usize;
        self.raised.fetch_add(1, Ordering::Relaxed);
        if leaf < LEAVES {
            let bit = 1u32 << (vector % 32);
            if self.pending[leaf].fetch_or(bit, Ordering::SeqCst) & bit != 0 {
                self.coalesced.fetch_add(1, Ordering::Relaxed);
                return false;
            }
        }
        if !self.msi_enabled.load(Ordering::Relaxed) {
            return false;
        }
        let mut g = self.msi_fd.lock().unwrap();
        let Some(f) = g.as_mut() else { return false };
        if f.write_all(&1u64.to_le_bytes()).is_ok() {
            self.msis.fetch_add(1, Ordering::Relaxed);
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

    fn take_pending(&self, leaf: usize) -> u32 {
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

    const BASE: u64 = 0x00B8_1000;

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
        use std::io::Read;
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
        shared.msi_enabled.store(true, Ordering::Relaxed);
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
}

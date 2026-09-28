// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The MMU invalidate page (`CARAXES_INVALIDATE_PAGE=1`, docs/caraxes/intr-page.md §11.3).
//!
//! BAR0 page 0xB83000 of TU106 holds `NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE_PDB`,
//! `_UPPER_PDB` and `_INVALIDATE` (0xB830A0/A4/B0) beside the VF fault-buffer and
//! access-counter pointers (tu102/dev_vm.h:74-113, :207-222). The guest RM invalidates a
//! TLB as `kgmmuCommitTlbInvalidate_TU102` does (kern_gmmu_tu102.c:95-121): it reads
//! `_INVALIDATE` until `TRIGGER` (bit 31) is clear, writes the two PDB words and then
//! `_INVALIDATE` with `TRIGGER`, and polls `_INVALIDATE` again until `TRIGGER` clears
//! (`kgmmuCheckPendingInvalidates_TU102`, :50-84). Trapped, that is three writes and two
//! reads per invalidate, ten per Mutter frame in series D.
//!
//! With this page the words are memory, a fourth sparse area of the interrupt page's memfd
//! (intr_page.rs). The reactor sees `TRIGGER` in the stored word, runs the device's
//! invalidate (the BAR2 memfd sync, the host TLB invalidate on the mirror VA spaces, the
//! shadow resync: boot.rs `mmu_invalidate_from_page`) under the device lock, and only then
//! clears `TRIGGER` in the page, so the guest's poll sees completion exactly when the
//! trapped write would have returned. `TRIGGER` is the hardware's own busy bit, so a memory
//! word can report it: the guest never stores a trigger while one is shown.
//!
//! The rest of the page is latched memory, as the trapped path latches it, except the
//! words the VF model answers (uvm.rs `VfRegs`): the fault-buffer and access-counter PUT
//! and INFO words and the access counter's buffer size are read-only there, and a store to
//! one is restored and counted; the GET words keep what the guest stores, masked to their
//! 20-bit field as the trapped path masks them. Any other register model in the page
//! refuses the start ([`InvPage::check`]).

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use crate::intr_page::{IntrPage, Mapping, PAGE};

/// The switch.
pub const INVALIDATE_PAGE_ENV: &str = "CARAXES_INVALIDATE_PAGE";
/// `NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE_TRIGGER`, bit 31.
pub const TRIGGER: u32 = 1 << 31;
/// `_ALL_PDB`, bit 1.
pub const ALL_PDB: u32 = 1 << 1;
/// The GET pointers' field (20 bits, as uvm.rs `VfRegs::write32` masks them).
const GET_MASK: u32 = 0xF_FFFF;
/// The guest RM's poll of `TRIGGER` gives up after its default GPU timeout, 4 s in
/// graphics mode (`osGetTimeoutParams`, arch/nvalloc/unix/src/os.c:2286, checked by
/// `kgmmuCheckPendingInvalidates_TU102`, kern_gmmu_tu102.c:77). A trigger the device ran
/// longer than that is an inconsistency: the guest saw a timeout the trapped path would
/// have shown as a stalled vCPU.
pub const GUEST_TIMEOUT_US: u64 = 4_000_000;
/// A trigger that took this long or longer is counted as slow (the watchdog's line).
pub const SLOW_US: u64 = 1_000;

/// `CARAXES_INVALIDATE_PAGE`: off, or on with the interrupt page and the boot model.
pub fn requested(intr_page: bool, boot_model: bool) -> Result<bool, String> {
    parse(
        std::env::var(INVALIDATE_PAGE_ENV).ok().as_deref(),
        intr_page,
        boot_model,
    )
}

pub fn parse(v: Option<&str>, intr_page: bool, boot_model: bool) -> Result<bool, String> {
    let on = match v.map(str::trim).filter(|v| !v.is_empty()).unwrap_or("0") {
        "0" | "off" => false,
        "1" | "on" => true,
        v => return Err(format!("{INVALIDATE_PAGE_ENV}={v:?}: 0 or 1")),
    };
    if on && !intr_page {
        return Err(format!(
            "{INVALIDATE_PAGE_ENV}=1 needs CARAXES_INTR_PAGE=1 (its memfd and its reactor)"
        ));
    }
    if on && !boot_model {
        return Err(format!(
            "{INVALIDATE_PAGE_ENV}=1 needs the boot model (it runs the invalidate)"
        ));
    }
    Ok(on)
}

/// The words of the page and how each behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Word {
    /// `_PDB`, `_UPPER_PDB`: latched, read at the trigger.
    Pdb,
    UpperPdb,
    /// `_INVALIDATE`: the trigger.
    Invalidate,
    /// A GET pointer: the guest's, masked to 20 bits.
    Get,
    /// A read-only word with its constant (PUT, INFO, the buffer size).
    Const(u32),
}

/// The registers of the VF window that live in one page, from the chip's table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub base: u64,
    /// (BAR0 offset, behaviour), ascending.
    pub words: Vec<(u64, Word)>,
}

impl Layout {
    /// The page of `_INVALIDATE` and the words of it the models answer. Refused when the
    /// three invalidate words do not share one page.
    pub fn of(r: &crate::chip::Registers, fault_buffer_entries: u32) -> Result<Self, String> {
        let base = r.mmu_invalidate & !(PAGE - 1);
        let mut words = vec![
            (r.mmu_invalidate_pdb, Word::Pdb),
            (r.mmu_invalidate_upper_pdb, Word::UpperPdb),
            (r.mmu_invalidate, Word::Invalidate),
        ];
        if words.iter().any(|(o, _)| o & !(PAGE - 1) != base) {
            return Err(format!(
                "the MMU invalidate registers {:#x}/{:#x}/{:#x} do not share one page",
                r.mmu_invalidate_pdb, r.mmu_invalidate_upper_pdb, r.mmu_invalidate
            ));
        }
        let others = [
            (r.fault_buffer_get[0], Word::Get),
            (r.fault_buffer_get[1], Word::Get),
            (r.fault_buffer_put[0], Word::Const(0)),
            (r.fault_buffer_put[1], Word::Const(0)),
            (
                r.access_counter_notify_buffer_size,
                Word::Const(fault_buffer_entries),
            ),
            (r.access_counter_get, Word::Get),
            (r.access_counter_put, Word::Const(0)),
            (r.access_counter_info, Word::Const(0)),
        ];
        words.extend(
            others
                .into_iter()
                .filter(|(o, _)| o & !(PAGE - 1) == base),
        );
        words.sort_by_key(|w| w.0);
        Ok(Layout { base, words })
    }
}

/// A trigger the reactor found: the three words as the guest stored them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trigger {
    pub pdb: u32,
    pub upper: u32,
    pub value: u32,
}

#[derive(Default, Debug)]
pub struct Stats {
    pub triggers: AtomicU64,
    /// Triggers the reactor found right after a blocking wait (up to its sleep bound late).
    pub after_block: AtomicU64,
    pub all_pdb: AtomicU64,
    pub get_masked: AtomicU64,
    /// Stores to a read-only word, restored.
    pub restored: AtomicU64,
    /// The guest changed `_INVALIDATE` while its trigger was being applied.
    pub changed_under: AtomicU64,
    /// The longest invalidate the device ran for a trigger, in microseconds.
    pub apply_us_max: AtomicU64,
    /// Triggers the device took [`SLOW_US`] or longer for.
    pub slow: AtomicU64,
    /// Triggers the device took [`GUEST_TIMEOUT_US`] or longer for (an inconsistency).
    pub over_timeout: AtomicU64,
    pub apply_us_sum: AtomicU64,
}

pub struct InvPage {
    layout: Layout,
    map: Mapping,
    pub stats: Stats,
}

impl InvPage {
    /// Map the page from the interrupt page's memfd and show the models' values.
    pub fn create(intr: &IntrPage, layout: Layout) -> Result<Arc<Self>, String> {
        let map = intr.map_page(layout.base)?;
        let p = InvPage {
            layout,
            map,
            stats: Stats::default(),
        };
        p.reset();
        Ok(Arc::new(p))
    }

    /// The start-up check: `owns(off)` says whether a model answers the BAR0 dword `off`
    /// (the boot model's `owns_register`). Every such dword of the page must be one of the
    /// layout's words, whose behaviour this page takes over; the rest is latched memory.
    pub fn check(&self, owns: &dyn Fn(u64) -> bool) -> Result<(), String> {
        let base = self.layout.base;
        for off in (base..base + PAGE).step_by(4) {
            let ours = self.layout.words.iter().any(|w| w.0 == off);
            if owns(off) && !ours {
                return Err(format!(
                    "BAR0 {off:#x} has a register model the invalidate page does not take over"
                ));
            }
        }
        Ok(())
    }

    /// The sparse area of region 0: (offset, size).
    pub fn area(&self) -> (u64, u64) {
        (self.layout.base, PAGE)
    }

    pub fn covers(&self, off: u64) -> bool {
        (self.layout.base..self.layout.base + PAGE).contains(&off)
    }

    fn word(&self, off: u64) -> &AtomicU32 {
        self.map.word((off & !3) - self.layout.base)
    }

    fn off_of(&self, w: Word) -> u64 {
        self.layout
            .words
            .iter()
            .find(|x| x.1 == w)
            .map(|x| x.0)
            .expect("the layout has the three invalidate words")
    }

    /// Device reset: zero, the constants in place.
    pub fn reset(&self) {
        for off in (0..PAGE).step_by(4) {
            self.map.word(off).store(0, Ordering::Relaxed);
        }
        for &(off, w) in &self.layout.words {
            if let Word::Const(c) = w {
                self.word(off).store(c, Ordering::Relaxed);
            }
        }
        std::sync::atomic::fence(Ordering::SeqCst);
    }

    /// A trapped read (a client that did not map the page): the memory.
    pub fn read32(&self, off: u64) -> u32 {
        self.word(off).load(Ordering::Acquire)
    }

    /// A trapped write: stored as the guest would store it, found by the next scan.
    pub fn write(&self, off: u64, data: &[u8]) {
        for (i, &b) in data.iter().enumerate() {
            let o = off + i as u64;
            let shift = (o & 3) * 8;
            let w = self.word(o);
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

    /// The reactor's scan: restore the read-only words, mask the GET words, and return a
    /// trigger if the guest stored one. `blocked` says the scan follows a blocking wait.
    pub fn scan(&self, blocked: bool) -> Option<Trigger> {
        for &(off, w) in &self.layout.words {
            let word = self.word(off);
            match w {
                Word::Const(c) => {
                    if word.load(Ordering::Acquire) != c {
                        word.store(c, Ordering::Release);
                        self.stats.restored.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Word::Get => {
                    let v = word.load(Ordering::Acquire);
                    if v & !GET_MASK != 0
                        && word
                            .compare_exchange(v, v & GET_MASK, Ordering::AcqRel, Ordering::Acquire)
                            .is_ok()
                    {
                        self.stats.get_masked.fetch_add(1, Ordering::Relaxed);
                    }
                }
                _ => {}
            }
        }
        let value = self.word(self.off_of(Word::Invalidate)).load(Ordering::Acquire);
        if value & TRIGGER == 0 {
            return None;
        }
        // x86 stores are seen in order: the PDB words the guest wrote before the trigger
        // are in place.
        let t = Trigger {
            pdb: self.word(self.off_of(Word::Pdb)).load(Ordering::Acquire),
            upper: self.word(self.off_of(Word::UpperPdb)).load(Ordering::Acquire),
            value,
        };
        self.stats.triggers.fetch_add(1, Ordering::Relaxed);
        if blocked {
            self.stats.after_block.fetch_add(1, Ordering::Relaxed);
        }
        if value & ALL_PDB != 0 {
            self.stats.all_pdb.fetch_add(1, Ordering::Relaxed);
        }
        Some(t)
    }

    /// The device ran the invalidate of `t` in `us` microseconds: clear `TRIGGER`, which is
    /// what the guest polls for. A word the guest changed meanwhile is left for the next
    /// scan and counted.
    pub fn complete(&self, t: Trigger, us: u64) {
        self.stats.apply_us_sum.fetch_add(us, Ordering::Relaxed);
        self.stats.apply_us_max.fetch_max(us, Ordering::Relaxed);
        if us >= SLOW_US {
            self.stats.slow.fetch_add(1, Ordering::Relaxed);
        }
        if us >= GUEST_TIMEOUT_US {
            self.stats.over_timeout.fetch_add(1, Ordering::Relaxed);
        }
        let w = self.word(self.off_of(Word::Invalidate));
        if w
            .compare_exchange(t.value, t.value & !TRIGGER, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            self.stats.changed_under.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn summary(&self) -> String {
        let s = &self.stats;
        let l = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let n = l(&s.triggers);
        let mut odd = Vec::new();
        if l(&s.restored) > 0 {
            odd.push(format!("{} store(s) to a read-only word restored", l(&s.restored)));
        }
        if l(&s.changed_under) > 0 {
            odd.push(format!(
                "{} change(s) of _INVALIDATE while its trigger was applied",
                l(&s.changed_under)
            ));
        }
        if l(&s.over_timeout) > 0 {
            odd.push(format!(
                "{} trigger(s) held beyond the guest RM's {} s timeout",
                l(&s.over_timeout),
                GUEST_TIMEOUT_US / 1_000_000
            ));
        }
        format!(
            "invalidate page: BAR0 [{:#x}, {:#x}); {} trigger(s) applied by the reactor ({} right after a blocking wait, {} ALL_PDB), device invalidate mean {} us, max {} us, {} at {} ms or more; {} GET store(s) masked; inconsistencies: {}\n",
            self.layout.base,
            self.layout.base + PAGE,
            n,
            l(&s.after_block),
            l(&s.all_pdb),
            l(&s.apply_us_sum).checked_div(n).unwrap_or(0),
            l(&s.apply_us_max),
            l(&s.slow),
            SLOW_US / 1000,
            l(&s.get_masked),
            if odd.is_empty() { "none".into() } else { odd.join(", ") }
        )
    }
}

/// What the reactor runs for a trigger (main.rs: lock the device,
/// `Device::invalidate_from_page`).
pub type Driver = dyn Fn(Trigger) + Send + Sync;

/// The reactor's side: scan, run the driver for a trigger, complete it.
pub fn poll(page: &InvPage, driver: &dyn Fn(Trigger), blocked: bool) -> bool {
    let Some(t) = page.scan(blocked) else {
        return false;
    };
    let t0 = std::time::Instant::now();
    driver(t);
    page.complete(t, t0.elapsed().as_micros() as u64);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intr::IntrShared;

    fn layout() -> Layout {
        let chip = crate::fixture::rtx2070().chip;
        let d = chip.data();
        Layout::of(&d.registers, d.gmmu.fault_buffer_entries).unwrap()
    }

    fn intr() -> Arc<IntrPage> {
        let chip = crate::fixture::rtx2070().chip;
        let d = chip.data();
        IntrPage::create(
            16 << 20,
            d.registers.intr_tree,
            d.identity,
            IntrShared::new(),
            crate::intr_page::Config::default(),
        )
        .unwrap()
    }

    #[test]
    fn switch_needs_the_interrupt_page_and_the_boot_model() {
        assert_eq!(parse(None, false, false), Ok(false));
        assert_eq!(parse(Some("0"), false, false), Ok(false));
        assert_eq!(parse(Some("1"), true, true), Ok(true));
        assert!(parse(Some("1"), false, true).unwrap_err().contains("CARAXES_INTR_PAGE"));
        assert!(parse(Some("1"), true, false).unwrap_err().contains("boot model"));
        assert!(parse(Some("yes"), true, true).is_err());
    }

    /// TU106: the page is 0xB83000 and holds the invalidate words and the VF pointers.
    #[test]
    fn tu106_layout_and_check() {
        let l = layout();
        assert_eq!(l.base, 0xB8_3000);
        assert_eq!(
            l.words.iter().filter(|w| w.1 == Word::Invalidate).count(),
            1
        );
        assert!(l.words.iter().any(|w| w.0 == 0xB8_30A0 && w.1 == Word::Pdb));
        assert!(l.words.iter().any(|w| w.0 == 0xB8_300C && w.1 == Word::Const(0)));
        let p = InvPage::create(&intr(), l.clone()).unwrap();
        // The boot model of the fixture owns exactly these words of the page.
        let f = crate::fixture::rtx2070();
        let model = crate::boot::BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(4 << 30),
            f.identity(4 << 30),
        );
        assert_eq!(p.check(&|off| model.owns_register(off)), Ok(()));
        // A model the page does not take over refuses the start.
        assert!(p.check(&|off| off == 0xB8_3200).is_err());
        // The page is a free page of the memfd: not the tree's, not page 0.
        assert!(intr().map_page(l.base).is_ok());
        assert_eq!(p.area(), (0xB8_3000, PAGE));
    }

    /// The guest's sequence: pending check, PDB words, trigger, poll. The trigger stays
    /// set until the driver has run, the PDB reaches the driver as stored, and the rest of
    /// the page behaves as the trapped path.
    #[test]
    fn a_trigger_is_applied_once_and_completes_in_the_page() {
        let l = layout();
        let p = InvPage::create(&intr(), l).unwrap();
        assert_eq!(p.read32(0xB8_30B0) & TRIGGER, 0);
        assert!(p.scan(false).is_none());
        p.write(0xB8_30A0, &0x1e75520u32.to_le_bytes());
        p.write(0xB8_30A4, &0u32.to_le_bytes());
        p.write(0xB8_30B0, &0x8000_00c1u32.to_le_bytes());
        let seen = std::sync::Mutex::new(Vec::new());
        let driver = |t: Trigger| {
            // While the device runs the invalidate the guest still sees TRIGGER.
            assert_ne!(p.read32(0xB8_30B0) & TRIGGER, 0);
            seen.lock().unwrap().push(t);
        };
        assert!(poll(&p, &driver, true));
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &[Trigger {
                pdb: 0x1e75520,
                upper: 0,
                value: 0x8000_00c1
            }]
        );
        // Completed: the guest's poll reads the word without TRIGGER; no second run.
        assert_eq!(p.read32(0xB8_30B0), 0xc1);
        assert!(!poll(&p, &driver, false));
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert_eq!(p.stats.triggers.load(Ordering::Relaxed), 1);
        assert_eq!(p.stats.after_block.load(Ordering::Relaxed), 1);
        // ALL_PDB is counted.
        p.write(0xB8_30B0, &(TRIGGER | ALL_PDB).to_le_bytes());
        assert!(poll(&p, &|_| {}, false));
        assert_eq!(p.stats.all_pdb.load(Ordering::Relaxed), 1);
        assert!(p.summary().contains("2 trigger(s) applied"));
        assert!(p.summary().contains("inconsistencies: none"));
    }

    /// The watchdog's counters: a slow trigger is counted, one past the guest RM's
    /// timeout is an inconsistency.
    #[test]
    fn slow_and_timed_out_triggers_are_counted() {
        let p = InvPage::create(&intr(), layout()).unwrap();
        let t = Trigger {
            pdb: 0,
            upper: 0,
            value: TRIGGER | 0xc1,
        };
        // Each completion clears a stored trigger, as the reactor's does.
        let run = |us: u64| {
            p.write(0xB8_30B0, &t.value.to_le_bytes());
            p.complete(t, us);
        };
        run(300);
        run(SLOW_US);
        assert_eq!(p.stats.slow.load(Ordering::Relaxed), 1);
        assert!(p.summary().contains("1 at 1 ms or more"));
        assert!(p.summary().contains("inconsistencies: none"));
        run(GUEST_TIMEOUT_US);
        assert_eq!(p.stats.slow.load(Ordering::Relaxed), 2);
        assert!(p
            .summary()
            .contains("inconsistencies: 1 trigger(s) held beyond the guest RM's 4 s timeout"));
    }

    /// The boot model runs a page trigger as it runs the trapped trigger write: the PDB
    /// words from the page, the same note, and the trapped read shows no TRIGGER.
    #[test]
    fn the_boot_model_runs_a_page_trigger_like_the_trapped_write() {
        let f = crate::fixture::rtx2070();
        let mut model = crate::boot::BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(4 << 30),
            f.identity(4 << 30),
        );
        let mem = crate::guestmem::GuestMemory::new();
        let notes = model.mmu_invalidate_from_page(&mem, 0x1e75520, 0, 0x8000_00c1);
        assert!(
            notes.iter().any(|n| n.starts_with("mmu invalidate: pdb 0x1e7552000 flags 0xc1")),
            "{notes:?}"
        );
        assert_eq!(model.read32(0xB8_30A0), Some(0x1e75520));
        assert_eq!(model.read32(0xB8_30B0), Some(0xc1));
    }

    /// Fail closed: read-only words are restored and counted, GET words masked, a change
    /// of the trigger word under the device's invalidate is left for the next scan.
    #[test]
    fn read_only_words_restored_and_races_counted() {
        let l = layout();
        let entries = crate::fixture::rtx2070().chip.data().gmmu.fault_buffer_entries;
        let p = InvPage::create(&intr(), l).unwrap();
        assert_eq!(p.read32(0xB8_3110), entries);
        p.write(0xB8_300C, &5u32.to_le_bytes()); // fault buffer PUT(0)
        p.write(0xB8_3110, &1u32.to_le_bytes()); // access counter buffer size
        p.write(0xB8_3008, &0xFFF0_0010u32.to_le_bytes()); // GET(0), bits above the field
        p.write(0xB8_3060, &0x1234u32.to_le_bytes()); // unowned: latched
        assert!(p.scan(false).is_none());
        assert_eq!(p.read32(0xB8_300C), 0);
        assert_eq!(p.read32(0xB8_3110), entries);
        assert_eq!(p.read32(0xB8_3008), 0x10);
        assert_eq!(p.read32(0xB8_3060), 0x1234);
        assert_eq!(p.stats.restored.load(Ordering::Relaxed), 2);
        assert_eq!(p.stats.get_masked.load(Ordering::Relaxed), 1);
        assert!(p.summary().contains("2 store(s) to a read-only word restored"));
        // The guest rewrites _INVALIDATE while the device runs the trigger.
        p.write(0xB8_30B0, &(TRIGGER | 1).to_le_bytes());
        let t = p.scan(false).unwrap();
        p.write(0xB8_30B0, &(TRIGGER | 3).to_le_bytes());
        p.complete(t, 10);
        assert_eq!(p.read32(0xB8_30B0), TRIGGER | 3);
        assert_eq!(p.stats.changed_under.load(Ordering::Relaxed), 1);
        assert_eq!(p.scan(false).unwrap().value, TRIGGER | 3);
    }
}

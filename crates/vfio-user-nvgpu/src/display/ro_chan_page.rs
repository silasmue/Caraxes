// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! An exact read-only BAR0 view of one emulated window-channel USERD page.
//!
//! Opt-in through `CARAXES_DISPLAY_WINDOW_RO`. A caller must offer `area()` as a read-only
//! sparse mapping and leave *every* store trapped. `Display` publishes changed
//! PUT/GET, method, and latch dwords as it applies trapped writes; a full
//! `refresh` is for initial publication and reset. The production caller must
//! serialize display writes and publication with the device mutex. Reset must drain old
//! operations and refresh the new display before the guest resumes. The page is
//! guest-owned emulated state in the interrupt memfd, never a host BAR mapping.
//! It must not overlap the writable legacy kick-page area.
//!
//! The whole page is eligible: `Display::read32` owns all 1024 dwords, including
//! PUT, GET, method assembly, ARMED state and otherwise-zero holes. Window pages
//! have no clock, interrupt acknowledgement, or read-to-clear registers. The core
//! page is excluded because its scan-line register advances with time.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

use super::{Display, WIN_USERD};
use crate::intr_page::{IntrPage, Mapping, PAGE};

const WINDOW_PAGES: u64 = 32;

pub struct RoChanPage {
    base: u64,
    map: Mapping,
    owner: Weak<IntrPage>,
}

impl RoChanPage {
    /// `base` identifies exactly one NVC57E window instance, never a core,
    /// window-immediate, cursor or host register page.
    pub fn create(intr: &Arc<IntrPage>, base: u64) -> Result<Self, String> {
        if base < WIN_USERD
            || base >= WIN_USERD + WINDOW_PAGES * PAGE
            || (base - WIN_USERD) % PAGE != 0
        {
            return Err(format!("BAR0 {base:#x}: not a window-channel USERD page"));
        }
        Ok(Self {
            base,
            map: intr.map_page(base)?,
            owner: Arc::downgrade(intr),
        })
    }

    /// Attachment must use the same VM's interrupt memfd; a weak identity does
    /// not keep a dead client's page alive or accept a recycled fd number.
    pub fn belongs_to(&self, intr: &Arc<IntrPage>) -> bool {
        self.owner
            .upgrade()
            .is_some_and(|owner| Arc::ptr_eq(&owner, intr))
    }

    pub fn area(&self) -> (u64, u64) {
        (self.base, PAGE)
    }

    pub fn covers_range(&self, off: u64, len: usize) -> bool {
        len != 0
            && off >= self.base
            && off
                .checked_add(len as u64)
                .is_some_and(|end| end <= self.base + PAGE)
    }

    /// Publish every dword from the ordinary BAR0 display model. Initial
    /// publication and reset only; hot writes publish changed words instead.
    /// Production callers hold the device mutex across model publication and
    /// write completion; this method does not acquire that lock itself.
    pub fn refresh(&self, display: &Display) -> Result<(), String> {
        if !display.enabled {
            return Err("read-only window page needs an enabled display".into());
        }
        let mut words = [0u32; (PAGE / 4) as usize];
        for (i, value) in words.iter_mut().enumerate() {
            let off = self.base + (i as u64) * 4;
            *value = display.window_word(off);
        }
        for (i, value) in words.into_iter().enumerate() {
            self.map
                .word((i as u64) * 4)
                .store(value, Ordering::Release);
        }
        Ok(())
    }

    /// Erase the previous client's image before model replacement. A failed
    /// attachment then cannot expose any of that client's register values.
    pub fn clear(&self) {
        for rel in (0..PAGE).step_by(4) {
            self.map.word(rel).store(0, Ordering::Release);
        }
    }

    /// Publish one dword changed by the ordinary display model. The PUT kick's
    /// changed method list (`ChanState::method`) and PUT/GET, or a direct trapped
    /// write's latched dword, supply the offsets. Never infer mutations by
    /// comparing shared memory: repeated guest writes remain transactions.
    pub fn publish_word(&self, display: &Display, off: u64) -> Result<(), String> {
        if off % 4 != 0 || !self.covers_range(off, 4) {
            return Err(format!(
                "BAR0 {off:#x}: outside aligned window-channel page"
            ));
        }
        if !display.enabled {
            return Err("read-only window page needs an enabled display".into());
        }
        let value = display.window_word(off);
        self.map
            .word(off - self.base)
            .store(value, Ordering::Release);
        Ok(())
    }

    /// The internal mutation hook. An offset on a different window page is
    /// expected; an in-page dword uses the same infallible decoder as read32.
    pub(crate) fn publish_if_covered(&self, display: &Display, off: u64) {
        if off % 4 == 0 && self.covers_range(off, 4) {
            self.map
                .word(off - self.base)
                .store(display.window_word(off), Ordering::Release);
        }
    }

    /// Also serves a trapped read with the same little-endian byte lanes as the
    /// mapping. A crossing access is left to the ordinary BAR0 route.
    pub fn read(&self, off: u64, data: &mut [u8]) -> bool {
        if !self.covers_range(off, data.len()) {
            return false;
        }
        for (i, byte) in data.iter_mut().enumerate() {
            let rel = off - self.base + i as u64;
            let value = self.map.word(rel & !3).load(Ordering::Acquire);
            *byte = (value >> ((rel & 3) * 8)) as u8;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{Setup, CHN_ARMED};
    use crate::fb::FakeFb;
    use crate::guestmem::GuestMemory;
    use crate::ver::common::display as d;

    fn display() -> Display {
        let board = crate::board::Board::load("rtx2070").unwrap();
        Display::from_setup(&Setup {
            enabled: true,
            chip: "tu106".into(),
            board: board.display,
        })
        .unwrap()
    }

    fn page(base: u64) -> RoChanPage {
        use std::os::fd::AsRawFd;
        let f = memfd(base + PAGE);
        RoChanPage {
            base,
            map: Mapping::new(f.as_raw_fd(), base).unwrap(),
            owner: Weak::new(),
        }
    }

    fn memfd(len: u64) -> std::fs::File {
        use std::os::fd::FromRawFd;
        let name = std::ffi::CString::new("caraxes-ro-chan-test").unwrap();
        // SAFETY: a successful memfd_create returns a new owned descriptor.
        let fd = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC) };
        assert!(fd >= 0);
        // SAFETY: `fd` was just created and has exactly one owner.
        let f = unsafe { std::fs::File::from_raw_fd(fd) };
        f.set_len(len).unwrap();
        f
    }

    fn equal_full_page(page: &RoChanPage, display: &mut Display) {
        for rel in (0..PAGE).step_by(4) {
            let mut actual = [0u8; 4];
            assert!(page.read(page.base + rel, &mut actual));
            assert_eq!(
                u32::from_le_bytes(actual),
                display.read32(page.base + rel).unwrap(),
                "BAR0 {:#x}",
                page.base + rel
            );
        }
    }

    #[test]
    fn all_window_instances_have_owned_initial_words() {
        let mut display = display();
        for instance in 0..WINDOW_PAGES {
            let base = WIN_USERD + instance * PAGE;
            let page = page(base);
            page.refresh(&mut display).unwrap();
            equal_full_page(&page, &mut display);
        }
    }

    #[test]
    fn full_page_tracks_put_get_methods_armed_and_holes() {
        let base = WIN_USERD + 4 * PAGE;
        let page = Arc::new(page(base));
        let mut display = display();
        display.attach_ro_chan_page(page.clone()).unwrap();
        equal_full_page(&page, &mut display);

        // An allocation sets both control words; a trapped method write is
        // visible in assembly, while an ARMED write is ignored by the model.
        let mut alloc = vec![0; crate::display::d::CHANNELDMA_ALLOC_SIZE];
        super::super::put32(&mut alloc, crate::display::d::CHANNEL_ALLOC_INSTANCE, 4);
        super::super::put32(&mut alloc, crate::display::d::CHANNELDMA_ALLOC_OFFSET, 0x40);
        display
            .alloc(crate::display::d::NVC57E_WINDOW_CHANNEL_DMA, &alloc)
            .unwrap();
        let mem = GuestMemory::new();
        let mut fb = FakeFb::new(1 << 20);
        display.write32(base + 0x22c, 0xcf, &mem, &mut fb).unwrap();
        display
            .write32(base + CHN_ARMED + 0x22c, 0xffff, &mem, &mut fb)
            .unwrap();
        equal_full_page(&page, &mut display);
        let mut lanes = [0u8; 6];
        assert!(page.read(base + 0x229, &mut lanes));
        let expected = [
            display.read32(base + 0x228).unwrap().to_le_bytes(),
            display.read32(base + 0x22c).unwrap().to_le_bytes(),
        ]
        .concat();
        assert_eq!(lanes, expected[1..7]);
        assert!(!page.read(base + PAGE - 1, &mut [0; 2]));
        assert!(!page.read(base - 1, &mut [0; 2]));
    }

    #[test]
    fn window_kick_publishes_hot_armed_words_and_reset_clears_them() {
        let base = WIN_USERD + 4 * PAGE;
        let page = Arc::new(page(base));
        let mut display = display();
        display.attach_ro_chan_page(page.clone()).unwrap();
        let mut mem = GuestMemory::new();
        let f = memfd(PAGE);
        mem.map(0x10_0000, PAGE, 0, &f).unwrap();
        let words = [
            1u32 << 18,
            0x1234_5678,
            1u32 << 18 | 4,
            0x8765_4321,
            1u32 << 18 | 0x100,
            0xface_cafe,
            1u32 << 18 | 0x22c,
            0xcf,
            1u32 << 18 | 0x2ec,
            0xff0,
            1u32 << 18 | 0x200,
            0,
        ];
        for (i, word) in words.iter().enumerate() {
            assert!(mem.write(0x10_0000 + 0x40 + (i as u64) * 4, &word.to_le_bytes()));
        }
        let mut push = vec![0; d::PUSHBUFFER_SIZE];
        super::super::put32(
            &mut push,
            d::PUSHBUFFER_HCLASS,
            d::NVC57E_WINDOW_CHANNEL_DMA,
        );
        super::super::put32(&mut push, d::PUSHBUFFER_CHANNEL_INSTANCE, 4);
        super::super::put32(&mut push, d::PUSHBUFFER_ADDRESS_SPACE, 1);
        push[8..16].copy_from_slice(&0x10_0000u64.to_le_bytes());
        push[16..24].copy_from_slice(&0xfffu64.to_le_bytes());
        push[d::PUSHBUFFER_VALID] = 1;
        display
            .control(d::CMD_INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER, &push)
            .unwrap();
        equal_full_page(&page, &mut display);
        let mut alloc = vec![0; d::CHANNELDMA_ALLOC_SIZE];
        super::super::put32(&mut alloc, d::CHANNEL_ALLOC_INSTANCE, 4);
        super::super::put32(&mut alloc, d::CHANNELDMA_ALLOC_OFFSET, 0x40);
        display.alloc(d::NVC57E_WINDOW_CHANNEL_DMA, &alloc).unwrap();
        equal_full_page(&page, &mut display);

        let mut fb = FakeFb::new(1 << 20);
        display
            .write32(base, 0x40 + (words.len() as u32) * 4, &mem, &mut fb)
            .unwrap();
        equal_full_page(&page, &mut display);
        let mut below_methods = [0; 4];
        assert!(page.read(base + 0x100, &mut below_methods));
        assert_eq!(
            below_methods, [0; 4],
            "assembly below CHN_METHODS is not a readback word"
        );
        let mut armed = [0; 4];
        assert!(page.read(base + CHN_ARMED + 0x22c, &mut armed));
        assert_eq!(u32::from_le_bytes(armed), 0xcf);
        assert!(page.read(base + CHN_ARMED + 0x2ec, &mut armed));
        assert_eq!(u32::from_le_bytes(armed), 0xff0);

        // An opt-in, bounded cost probe for source review. It is deliberately
        // outside the usual test path and reports time only, never a timing gate.
        if std::env::var_os("RO_CHAN_BENCH").is_some() {
            let start = std::time::Instant::now();
            for _ in 0..1000 {
                page.refresh(&mut display).unwrap();
            }
            eprintln!(
                "ro channel full-page refresh: {} ns/call (1000 calls, debug/release as built)",
                start.elapsed().as_nanos() / 1000
            );
            let start = std::time::Instant::now();
            for _ in 0..1000 {
                page.publish_word(&mut display, base + CHN_ARMED + 0x22c)
                    .unwrap();
            }
            eprintln!(
                "ro channel one-word publication: {} ns/call (1000 calls, debug/release as built)",
                start.elapsed().as_nanos() / 1000
            );
        }

        let mut fresh = self::display();
        page.clear();
        fresh.attach_ro_chan_page(page.clone()).unwrap();
        equal_full_page(&page, &mut fresh);
        assert!(page.read(base + CHN_ARMED + 0x22c, &mut armed));
        assert_eq!(u32::from_le_bytes(armed), 0);
    }

    #[test]
    fn rpc_free_keeps_display_channel_state_as_current_model_does() {
        let fixture = crate::fixture::rtx2070();
        let mut ctrl =
            crate::ctrl::Dispatcher::new(fixture.chip.data(), &fixture.identity(4 << 30), None);
        ctrl.display = display();
        let base = WIN_USERD + 4 * PAGE;
        let page = Arc::new(page(base));
        ctrl.display.attach_ro_chan_page(page.clone()).unwrap();
        let mut alloc = vec![0; d::CHANNELDMA_ALLOC_SIZE];
        super::super::put32(&mut alloc, d::CHANNEL_ALLOC_INSTANCE, 4);
        super::super::put32(&mut alloc, d::CHANNELDMA_ALLOC_OFFSET, 0x40);
        assert_eq!(
            ctrl.alloc(0x101, 0x202, 0x101, d::NVC57E_WINDOW_CHANNEL_DMA, &alloc)
                .status,
            crate::ver::common::NV_OK,
        );
        equal_full_page(&page, &mut ctrl.display);
        assert_eq!(ctrl.display.read32(base), Some(0x40));
        ctrl.free(0x101, 0x202);
        equal_full_page(&page, &mut ctrl.display);
        assert_eq!(ctrl.display.read32(base), Some(0x40));
    }
}

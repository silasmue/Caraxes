// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Guest RAM registration off the vfio-user loop.
//!
//! The device answers a `DMA_MAP` once the region is mapped and queues its registration
//! with the host RM here; a worker thread registers it piece by piece (run P34: 8 GiB took
//! 25 s under host memory pressure, and QEMU dropped the device at its 5 s message timeout).
//! Until a piece is registered:
//! - the device's own CPU accesses to it go through in GPA mode (`GuestMemory::translate`):
//!   the pages are this VM's, mapped by the device;
//! - a host IOVA of it waits: the shadow's descriptor snapshot waits for the worker
//!   (`Registrar::wait_idle`, mirror.rs `descs`), and `GuestMemory::host_iova` finds the
//!   pieces the device has not applied yet (`Registrar::range_of`).
//!
//! The device applies finished pieces (`take_done`) at its next access or on the clock of
//! main.rs; a `DMA_UNMAP` cancels the queued pieces of its region and waits for the one
//! in flight (`cancel`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::dmamap::DmaRange;

/// Set when a piece finished; main.rs's clock then lets the device apply it.
static DONE: AtomicBool = AtomicBool::new(false);

/// Whether a registration finished since the last call (main.rs clock).
pub fn registrations_done() -> bool {
    DONE.swap(false, Ordering::AcqRel)
}

/// A guest RAM piece: `size` bytes at guest physical `gpa`, mapped by the device at `va`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub gpa: u64,
    pub size: u64,
    pub va: usize,
}

impl Piece {
    fn holds(&self, gpa: u64, end: u64) -> bool {
        gpa >= self.gpa && end <= self.gpa.saturating_add(self.size)
    }

    fn overlaps(&self, gpa: u64, end: u64) -> bool {
        gpa < self.gpa.saturating_add(self.size) && self.gpa < end
    }
}

/// `[gpa, gpa + size)` cut at every multiple of `chunk` in guest physical space (hostmem.rs
/// `register_chunks` cuts the same way, so each piece is one descriptor).
pub fn pieces(gpa: u64, size: u64, va: usize, chunk: u64) -> Vec<Piece> {
    let mut out = Vec::new();
    let end = gpa.saturating_add(size);
    let mut at = gpa;
    while at < end {
        let next = at.saturating_add(chunk - at % chunk).min(end);
        out.push(Piece {
            gpa: at,
            size: next - at,
            va: va + (at - gpa) as usize,
        });
        at = next;
    }
    out
}

/// A registration's result: the table ranges and log lines, or the error.
pub type Outcome = Result<(Vec<DmaRange>, Vec<String>), String>;

/// What registers one piece (hostmem.rs through the RM; a stand-in in tests).
pub type RegisterFn = Box<dyn FnMut(&Piece) -> Outcome + Send>;

#[derive(Default)]
struct State {
    queued: VecDeque<Piece>,
    running: Option<Piece>,
    /// Finished, not yet taken by the device.
    done: Vec<(Piece, Outcome)>,
    stop: bool,
    /// Waits for the worker, and their time.
    waits: u64,
    waited: Duration,
}

pub struct Registrar {
    /// The guest speaks guest physical addresses (QEMU, Windows).
    gpa_mode: bool,
    state: Mutex<State>,
    cv: Condvar,
    has_done: AtomicBool,
}

impl Registrar {
    pub fn new(gpa_mode: bool) -> Arc<Self> {
        Arc::new(Self {
            gpa_mode,
            state: Mutex::new(State::default()),
            cv: Condvar::new(),
            has_done: AtomicBool::new(false),
        })
    }

    /// Start the worker thread.
    pub fn spawn(
        this: &Arc<Self>,
        mut register: RegisterFn,
    ) -> std::io::Result<std::thread::JoinHandle<()>> {
        let r = this.clone();
        std::thread::Builder::new()
            .name("guest-ram-register".into())
            .spawn(move || r.run(&mut register))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn run(&self, register: &mut RegisterFn) {
        loop {
            let piece = {
                let mut s = self.lock();
                loop {
                    if s.stop {
                        return;
                    }
                    if let Some(p) = s.queued.pop_front() {
                        s.running = Some(p);
                        break p;
                    }
                    s = self.cv.wait(s).unwrap_or_else(PoisonError::into_inner);
                }
            };
            let out = register(&piece);
            let mut s = self.lock();
            s.running = None;
            s.done.push((piece, out));
            self.has_done.store(true, Ordering::Release);
            DONE.store(true, Ordering::Release);
            self.cv.notify_all();
        }
    }

    pub fn gpa_mode(&self) -> bool {
        self.gpa_mode
    }

    pub fn queue(&self, pieces: Vec<Piece>) {
        self.lock().queued.extend(pieces);
        self.cv.notify_all();
    }

    /// Bytes queued or in flight (the host RAM floor counts them as pinned already).
    pub fn outstanding(&self) -> u64 {
        let s = self.lock();
        s.queued
            .iter()
            .chain(s.running.iter())
            .map(|p| p.size)
            .sum()
    }

    /// Whether finished pieces wait for the device (cheap: no lock).
    pub fn has_done(&self) -> bool {
        self.has_done.load(Ordering::Acquire)
    }

    pub fn take_done(&self) -> Vec<(Piece, Outcome)> {
        let mut s = self.lock();
        self.has_done.store(false, Ordering::Release);
        std::mem::take(&mut s.done)
    }

    /// Whether `[gpa, gpa + len)` lies in one piece the device has not applied yet
    /// (queued, in flight or finished).
    pub fn unsettled(&self, gpa: u64, len: u64) -> bool {
        let Some(end) = gpa.checked_add(len.max(1)) else {
            return false;
        };
        let s = self.lock();
        s.queued
            .iter()
            .chain(s.running.iter())
            .chain(s.done.iter().map(|d| &d.0))
            .any(|p| p.holds(gpa, end))
    }

    /// The registered range holding all of `[gpa, gpa + len)` among the pieces the device
    /// has not applied yet; `None` when the piece is not finished or failed.
    pub fn range_of(&self, gpa: u64, len: u64) -> Option<DmaRange> {
        let end = gpa.checked_add(len.max(1))?;
        let s = self.lock();
        s.done
            .iter()
            .filter_map(|(_, o)| o.as_ref().ok())
            .flat_map(|(r, _)| r.iter())
            .find(|r| gpa >= r.gpa && end <= r.gpa.saturating_add(r.size))
            .copied()
    }

    /// Wait until nothing is queued or in flight. Never call it while holding a lock the
    /// worker's register function takes (hostmem's).
    pub fn wait_idle(&self) {
        let mut s = self.lock();
        if s.queued.is_empty() && s.running.is_none() {
            return;
        }
        let t0 = Instant::now();
        while !(s.queued.is_empty() && s.running.is_none()) {
            s = self.cv.wait(s).unwrap_or_else(PoisonError::into_inner);
        }
        s.waits += 1;
        s.waited += t0.elapsed();
    }

    /// A `DMA_UNMAP` of `[gpa, gpa + size)`: drop its queued pieces, wait for the one in
    /// flight. Returns the pieces dropped.
    pub fn cancel(&self, gpa: u64, size: u64) -> Vec<Piece> {
        let end = gpa.saturating_add(size);
        let mut s = self.lock();
        let (gone, keep): (VecDeque<Piece>, VecDeque<Piece>) =
            s.queued.drain(..).partition(|p| p.overlaps(gpa, end));
        s.queued = keep;
        while s.running.is_some_and(|p| p.overlaps(gpa, end)) {
            s = self.cv.wait(s).unwrap_or_else(PoisonError::into_inner);
        }
        gone.into_iter().collect()
    }

    /// End the worker: nothing queued is registered any more; the piece in flight finishes.
    pub fn stop(&self) {
        let mut s = self.lock();
        s.stop = true;
        s.queued.clear();
        self.cv.notify_all();
    }

    /// Waits so far and their total time.
    pub fn waits(&self) -> (u64, Duration) {
        let s = self.lock();
        (s.waits, s.waited)
    }
}

/// Fault the pages of a piece in before it is pinned, outside hostmem's lock (the slow part
/// under host memory pressure). Content is unchanged; errors are ignored (the pin faults
/// them in itself).
pub fn prefault(p: &Piece) {
    // SAFETY: `va..va + size` is the device's own MAP_SHARED mapping of this guest RAM,
    // alive while the piece is queued or in flight (`cancel` before munmap).
    unsafe {
        libc::madvise(
            p.va as *mut libc::c_void,
            p.size as usize,
            libc::MADV_POPULATE_WRITE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guestmem::GuestMemory;
    use std::sync::mpsc;

    const IOVA: u64 = 0x7ff0_0000_0000;

    /// A registrar whose worker registers a piece at `IOVA + gpa` once the test lets it.
    fn gated() -> (
        Arc<Registrar>,
        mpsc::Sender<()>,
        std::thread::JoinHandle<()>,
    ) {
        let r = Registrar::new(true);
        let (tx, rx) = mpsc::channel::<()>();
        let h = Registrar::spawn(
            &r,
            Box::new(move |p: &Piece| {
                rx.recv().map_err(|e| e.to_string())?;
                Ok((
                    vec![DmaRange {
                        gpa: p.gpa,
                        iova: IOVA + p.gpa,
                        size: p.size,
                    }],
                    vec![],
                ))
            }),
        )
        .unwrap();
        (r, tx, h)
    }

    #[test]
    fn pieces_follow_the_chunk_boundaries() {
        const G: u64 = 1 << 30;
        let p = pieces(0x10_0000, 0x7ff0_0000, 0x1000_0000, G);
        assert_eq!(p.len(), 2);
        assert_eq!(
            p[0],
            Piece {
                gpa: 0x10_0000,
                size: G - 0x10_0000,
                va: 0x1000_0000
            }
        );
        assert_eq!(
            p[1],
            Piece {
                gpa: G,
                size: G,
                va: 0x1000_0000 + (G - 0x10_0000) as usize
            }
        );
        assert_eq!(pieces(0, 0xa_0000, 0, G).len(), 1);
    }

    /// The DMA_MAP answer comes first: queueing returns while the worker is still held,
    /// the device's own accesses to the piece go through, and a host IOVA of it is there
    /// once the worker finished, before the device applied it.
    #[test]
    fn an_access_to_a_piece_not_yet_registered_waits_and_never_fails() {
        let path = std::env::temp_dir().join(format!("nvgpu-registrar-{}", std::process::id()));
        let f = std::fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        f.set_len(0x20_0000).unwrap();
        let mut m = GuestMemory::new();
        m.map(0, 0x20_0000, 0, &f).unwrap();
        // GPA mode: an empty table refuses everything not registered.
        m.set_gpa_mode(true);
        m.set_table(Some(crate::dmamap::DmaTable::default()));
        assert!(!m.write(0x1000, &[1, 2, 3, 4]));
        let (r, go, worker) = gated();
        m.set_registrar(Some(r.clone()));
        let t0 = Instant::now();
        r.queue(pieces(
            0,
            0x20_0000,
            m.host_ptr(0, 0x20_0000).unwrap() as usize,
            1 << 30,
        ));
        assert!(
            t0.elapsed() < Duration::from_millis(50),
            "queueing must not wait for the RM"
        );
        // The CPU access to this VM's own pages goes through while the piece is queued.
        assert!(m.write(0x1000, &[1, 2, 3, 4]));
        assert_eq!(m.read_u32(0x1000), Some(0x0403_0201));
        assert_eq!(m.host_iova(0x1000, 0x1000), None);
        // The shadow's snapshot waits for the worker.
        let opener = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            go.send(()).unwrap();
        });
        r.wait_idle();
        assert!(t0.elapsed() >= Duration::from_millis(100));
        assert_eq!(r.waits().0, 1);
        opener.join().unwrap();
        // Registered, not applied yet: the IOVA comes from the registrar.
        assert_eq!(m.host_iova(0x1000, 0x1000), Some(IOVA + 0x1000));
        assert!(m.write(0x2000, &[5]));
        // Applied: the table answers, the registrar has nothing unsettled.
        let done = r.take_done();
        assert_eq!(done.len(), 1);
        let ranges = done[0].1.as_ref().unwrap().0.clone();
        let table = crate::dmamap::DmaTable::for_gpa_guest(ranges.clone()).unwrap();
        m.set_table(Some(table));
        assert!(!r.unsettled(0x1000, 4));
        assert_eq!(m.host_iova(0x1000, 0x1000), Some(IOVA + 0x1000));
        assert_eq!(m.read_u32(0x1000), Some(0x0403_0201));
        r.stop();
        worker.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }

    /// A DMA_UNMAP drops the queued pieces of its region and waits for the one in flight;
    /// other pieces stay queued.
    #[test]
    fn an_unmap_cancels_queued_pieces_and_waits_for_the_one_in_flight() {
        let (r, go, worker) = gated();
        const G: u64 = 1 << 30;
        r.queue(pieces(0, 3 * G, 0x1000_0000, G));
        r.queue(pieces(4 * G, G, 0x9000_0000, G));
        // The worker takes the first piece and waits for the gate.
        while r.lock().running.is_none() {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(r.outstanding(), 4 * G);
        let opener = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            go.send(()).unwrap();
            go
        });
        let gone = r.cancel(0, 3 * G);
        assert_eq!(
            gone.iter().map(|p| p.gpa).collect::<Vec<_>>(),
            vec![G, 2 * G]
        );
        assert!(r.lock().running.is_none_or(|p| p.gpa == 4 * G));
        let done = r.take_done();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].0.gpa, 0);
        let go = opener.join().unwrap();
        go.send(()).unwrap();
        r.wait_idle();
        assert_eq!(r.take_done()[0].0.gpa, 4 * G);
        r.stop();
        worker.join().unwrap();
    }

    /// A failed piece leaves no range: its IOVA stays refused, never a wrong one.
    #[test]
    fn a_failed_piece_gives_no_iova() {
        let r = Registrar::new(true);
        let worker = Registrar::spawn(&r, Box::new(|_: &Piece| Err("refused".into()))).unwrap();
        r.queue(pieces(0, 0x20_0000, 0x1000_0000, 1 << 30));
        r.wait_idle();
        assert!(r.unsettled(0x1000, 4));
        assert_eq!(r.range_of(0x1000, 4), None);
        let done = r.take_done();
        assert!(done[0].1.is_err());
        assert!(!r.unsettled(0x1000, 4));
        r.stop();
        worker.join().unwrap();
    }
}

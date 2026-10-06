// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Ordered ownership of the existing host RM client. The worker never acquires Device
//! or waits for consumption of an asynchronous result, including at a drain barrier.

use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use crate::ctrl::HostRm;

pub const ENV: &str = "CARAXES_HOST_CONTROL_OFFLOCK";
pub const TARGETS: [(u32, usize); 2] = [(0x2080_a612, 54352), (0x2080_a613, 79936)];

pub fn eligible(cmd: u32, size: usize) -> bool {
    TARGETS.contains(&(cmd, size))
}

/// Validate only the selected controls; unrelated Windows compatibility stays intact.
pub fn validate_options(
    ack_unanswered: bool,
    excepted: impl Fn(u32) -> bool,
    ack_extra: impl Fn(u32) -> bool,
    overrides: impl Fn(u32) -> bool,
    cacheable: bool,
    rpc_ioeventfd: bool,
) -> Result<(), String> {
    if rpc_ioeventfd {
        return Err(format!(
            "{ENV}=1 requires trapped RPC head writes; disable CARAXES_RPC_IOEVENTFD"
        ));
    }
    if cacheable {
        return Err(format!("{ENV}=1 requires CARAXES_LEGACY_CACHEABLE=0"));
    }
    for (cmd, _) in TARGETS {
        if ack_unanswered && !excepted(cmd) {
            return Err(format!(
                "{ENV}=1 with CARAXES_ACK_UNANSWERED=1 requires {cmd:08x} in CARAXES_WIN_EXCEPT"
            ));
        }
        if ack_extra(cmd) || overrides(cmd) {
            return Err(format!(
                "{ENV}=1 rejects a recorded/diagnostic override of {cmd:#x}"
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub epoch: u64,
    pub id: u64,
}

pub struct Answer {
    pub status: u32,
    pub params: Vec<u8>,
    /// Host completion to Device publication; present only with profiling on.
    pub ready_at: Option<std::time::Instant>,
}

struct State {
    epoch: u64,
    next_id: u64,
    pending: Option<Ticket>,
    result: Option<(Ticket, Answer)>,
    taken: bool,
    fault: Option<String>,
    closed: bool,
    stopping: bool,
    wake: u64,
    pump_stop: bool,
}

struct Shared {
    state: Mutex<State>,
    admission: Mutex<()>,
    changed: Condvar,
}

impl Shared {
    fn fail(&self, error: String) {
        let mut s = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        s.fault.get_or_insert(error);
        s.wake = s.wake.wrapping_add(1);
        self.changed.notify_all();
    }
}

enum Destination {
    Async(Ticket),
    Sync(mpsc::SyncSender<Answer>),
}

enum Job {
    Call {
        class: u32,
        cmd: u32,
        params: Vec<u8>,
        destination: Destination,
        submitted: Option<std::time::Instant>,
    },
    Barrier(mpsc::SyncSender<()>),
    Stop,
}

#[derive(Clone)]
pub struct Service {
    shared: Arc<Shared>,
    tx: mpsc::SyncSender<Job>,
}

/// Kept outside Device so neither worker nor pump is joined under its mutex.
pub struct Worker {
    service: Service,
    thread: JoinHandle<()>,
}

impl Worker {
    pub fn start(mut host: Box<dyn HostRm>) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                epoch: 1,
                next_id: 1,
                pending: None,
                result: None,
                taken: false,
                fault: None,
                closed: false,
                stopping: false,
                wake: 0,
                pump_stop: false,
            }),
            admission: Mutex::new(()),
            changed: Condvar::new(),
        });
        let (tx, rx) = mpsc::sync_channel(1);
        let service = Service {
            shared: shared.clone(),
            tx,
        };
        let thread = std::thread::Builder::new()
            .name("host-control".into())
            .spawn(move || {
                let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    while let Ok(job) = rx.recv() {
                        match job {
                            Job::Call {
                                class,
                                cmd,
                                mut params,
                                destination,
                                submitted,
                            } => {
                                // Admission errors cannot cancel earlier accepted calls.
                                // A host panic exits this worker; there is no retry.
                                if let Some(t) = submitted {
                                    crate::profile::record(
                                        crate::profile::Key::Window("host control: queue wait"),
                                        t.elapsed(),
                                        1,
                                        None,
                                        "",
                                    );
                                }
                                let started = crate::profile::on().then(std::time::Instant::now);
                                let status = host.control(class, cmd, &mut params);
                                let ready_at = started.map(|t| {
                                    let ready = std::time::Instant::now();
                                    crate::profile::record(
                                        crate::profile::Key::Window("host control: execution"),
                                        ready.duration_since(t),
                                        1,
                                        None,
                                        "",
                                    );
                                    ready
                                });
                                let answer = Answer {
                                    status,
                                    params,
                                    ready_at,
                                };
                                match destination {
                                    Destination::Sync(done) => {
                                        // Capacity one: never wait for the caller to consume it.
                                        if done.send(answer).is_err() {
                                            shared.fail(
                                                "synchronous host-control caller disappeared"
                                                    .into(),
                                            );
                                        }
                                    }
                                    Destination::Async(ticket) => {
                                        let mut s = shared
                                            .state
                                            .lock()
                                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                                        if s.epoch != ticket.epoch {
                                            // Retirement owns the barrier and will discard this
                                            // generation only after its host call has finished.
                                            crate::profile::window(
                                                "host control: discarded old epoch result",
                                                1,
                                            );
                                            continue;
                                        }
                                        if s.pending != Some(ticket) || s.taken {
                                            s.fault.get_or_insert_with(|| {
                                                "host-control completion identity mismatch".into()
                                            });
                                            s.wake = s.wake.wrapping_add(1);
                                            shared.changed.notify_all();
                                            continue;
                                        }
                                        if s.result.is_some() {
                                            s.fault.get_or_insert_with(|| {
                                                "host-control result slot occupied".into()
                                            });
                                            s.wake = s.wake.wrapping_add(1);
                                            shared.changed.notify_all();
                                            continue;
                                        }
                                        s.result = Some((ticket, answer));
                                        s.wake = s.wake.wrapping_add(1);
                                        shared.changed.notify_all();
                                    }
                                }
                            }
                            Job::Barrier(done) => {
                                let _ = done.send(());
                            }
                            Job::Stop => return,
                        }
                    }
                }));
                if run.is_err() {
                    shared.fail(
                        "host-control worker panicked; accepted calls will not be retried".into(),
                    );
                }
            })?;
        Ok(Self { service, thread })
    }

    pub fn service(&self) -> Service {
        self.service.clone()
    }

    pub fn stop(self) -> Result<(), String> {
        self.service.stop_pump();
        let _admission = self
            .service
            .shared
            .admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.service
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stopping = true;
        let sent = self
            .service
            .tx
            .send(Job::Stop)
            .map_err(|_| "host-control worker stopped".to_string());
        self.thread
            .join()
            .map_err(|_| "host-control worker join failed".to_string())?;
        let s = self
            .service
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(error) = &s.fault {
            return Err(error.clone());
        }
        sent
    }
}

impl Service {
    pub fn proxy(&self) -> Box<dyn HostRm> {
        Box::new(Proxy(self.clone()))
    }

    pub fn check(&self) -> Result<(), String> {
        let s = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Self::check_state(&s)
    }

    fn check_fault(&self) -> Result<(), String> {
        let s = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &s.fault {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    fn check_state(s: &State) -> Result<(), String> {
        if let Some(e) = &s.fault {
            return Err(e.clone());
        }
        if s.closed {
            return Err("host-control admission closed".into());
        }
        if s.stopping {
            return Err("host-control worker stopped".into());
        }
        Ok(())
    }

    pub fn submit(&self, class: u32, cmd: u32, params: Vec<u8>) -> Result<Ticket, String> {
        self.check()?; // Fast rejection while close waits for its barrier.
        let _admission = self
            .shared
            .admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ticket = {
            let mut s = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            Self::check_state(&s)?;
            if s.pending.is_some() || s.result.is_some() {
                return Err("host-control asynchronous slot already occupied".into());
            }
            let id = s.next_id;
            s.next_id = id.checked_add(1).ok_or("host-control job id exhausted")?;
            let ticket = Ticket { epoch: s.epoch, id };
            s.pending = Some(ticket);
            s.taken = false;
            ticket
        };
        let job = Job::Call {
            class,
            cmd,
            params,
            destination: Destination::Async(ticket),
            submitted: crate::profile::on().then(std::time::Instant::now),
        };
        if self.tx.try_send(job).is_err() {
            let error =
                "host-control asynchronous admission failed; no synchronous retry".to_string();
            self.shared.fail(error.clone());
            return Err(error);
        }
        Ok(ticket)
    }

    pub fn take(&self, ticket: Ticket) -> Result<Option<Answer>, String> {
        let mut s = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Self::check_state(&s)?;
        if s.epoch != ticket.epoch || s.pending != Some(ticket) {
            return Err("stale host-control completion".into());
        }
        if s.taken {
            return Err("host-control completion already taken".into());
        }
        match s.result.take() {
            Some((actual, answer)) if actual == ticket => {
                s.taken = true;
                Ok(Some(answer))
            }
            Some(_) => {
                s.fault
                    .get_or_insert_with(|| "host-control completion identity mismatch".into());
                s.wake = s.wake.wrapping_add(1);
                self.shared.changed.notify_all();
                Err("host-control completion identity mismatch".into())
            }
            None => Ok(None),
        }
    }

    /// Release admission only after the entire reply entered the ordered status stream.
    pub fn committed(&self, ticket: Ticket) -> Result<(), String> {
        let mut s = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Self::check_state(&s)?;
        if s.pending != Some(ticket) || s.result.is_some() || !s.taken {
            return Err("host-control commit identity/state mismatch".into());
        }
        s.pending = None;
        s.taken = false;
        Ok(())
    }

    /// The caller serializes lifecycle under Device. Accepted work still runs exactly
    /// once; the barrier never depends on the pump or on result consumption.
    pub fn retire(&self) -> Result<(), String> {
        let _admission = self
            .shared
            .admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        {
            let mut s = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(epoch) = s.epoch.checked_add(1) {
                s.epoch = epoch;
            } else {
                s.fault
                    .get_or_insert_with(|| "host-control epoch exhausted".into());
            }
        }
        // Admission failure does not prove an earlier accepted ioctl has returned.
        let result = self.barrier();
        let mut s = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        s.pending = None;
        s.result = None;
        s.taken = false;
        result
    }

    pub fn drain(&self) -> Result<(), String> {
        let _admission = self
            .shared
            .admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.barrier()
    }

    /// Seal admission, then wait for every already accepted host call before host
    /// memory can be torn down by the signal thread. No Device lock or completion
    /// pump is needed. A synchronous caller accepted before this barrier keeps its
    /// nested storage live until its real host answer arrives.
    pub fn close(&self) -> Result<(), String> {
        let _admission = self
            .shared
            .admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        {
            let mut s = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if s.closed {
                return match &s.fault {
                    Some(error) => Err(error.clone()),
                    None => Ok(()),
                };
            }
            s.closed = true;
            s.pump_stop = true;
            self.shared.changed.notify_all();
        }
        self.barrier_inner()?;
        self.check_fault()
    }

    fn barrier(&self) -> Result<(), String> {
        // Caller holds admission. A previous close/stop retained that mutex until
        // its barrier/join completed, so those states already prove the drain.
        let drained = {
            let s = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            s.closed || s.stopping
        };
        if !drained {
            self.barrier_inner()?;
        }
        self.check_fault()
    }

    fn barrier_inner(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.tx.send(Job::Barrier(tx)).map_err(|_| {
            let e = "host-control worker unavailable at barrier".to_string();
            self.shared.fail(e.clone());
            e
        })?;
        rx.recv().map_err(|_| {
            let e = "host-control worker failed at barrier".to_string();
            self.shared.fail(e.clone());
            e
        })
    }

    pub fn wait(&self, seen: &mut u64) -> bool {
        let mut s = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while s.wake == *seen && !s.pump_stop {
            s = self
                .shared
                .changed
                .wait(s)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *seen = s.wake;
        !s.pump_stop
    }

    pub fn stop_pump(&self) {
        let mut s = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        s.pump_stop = true;
        self.shared.changed.notify_all();
    }

    fn call(&self, class: u32, cmd: u32, params: &[u8]) -> Result<Answer, String> {
        // Hold admission only through enqueue, never through the host call or
        // result wait. This orders a signal-thread close after every accepted
        // caller while keeping that caller's nested storage live until receipt.
        self.check()?; // Fast rejection while close waits for its barrier.
        let admission = self
            .shared
            .admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.check()?;
        let (tx, rx) = mpsc::sync_channel(1);
        self.tx
            .send(Job::Call {
                class,
                cmd,
                params: params.to_vec(),
                destination: Destination::Sync(tx),
                submitted: crate::profile::on().then(std::time::Instant::now),
            })
            .map_err(|_| {
                let e = "host-control worker unavailable".to_string();
                self.shared.fail(e.clone());
                e
            })?;
        drop(admission);
        let answer = rx.recv().map_err(|_| {
            let e = "host-control worker failed during synchronous call".to_string();
            self.shared.fail(e.clone());
            e
        })?;
        // Closing admission is not a failure of a call already accepted. It
        // must return the real answer before the caller can drop nested input.
        self.check_fault()?;
        Ok(answer)
    }
}

struct Proxy(Service);
impl HostRm for Proxy {
    fn control(&mut self, class: u32, cmd: u32, params: &mut [u8]) -> u32 {
        match self.0.call(class, cmd, params) {
            Ok(answer) => {
                params.copy_from_slice(&answer.params);
                answer.status
            }
            Err(_) => {
                // Existing trait cannot encode transport failure. The Device checks the
                // shared terminal/closed state before publishing a guest response.
                0xffff_ffff
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::mpsc::{channel, sync_channel, RecvTimeoutError};
    use std::time::{Duration, Instant};

    struct Fake<F>(F);
    impl<F: FnMut(u32, u32, &mut [u8]) -> u32 + Send> HostRm for Fake<F> {
        fn control(&mut self, class: u32, cmd: u32, params: &mut [u8]) -> u32 {
            (self.0)(class, cmd, params)
        }
    }

    #[test]
    fn blocked_async_does_not_need_pump_for_sync_or_barrier() {
        let (entered, in_worker) = sync_channel(1);
        let (release, proceed) = sync_channel(0);
        let worker = Worker::start(Box::new(Fake(move |class, cmd, params: &mut [u8]| {
            if cmd == 1 {
                entered.send(()).unwrap();
                proceed.recv().unwrap();
            }
            params[0] = params[0].wrapping_add(class as u8).wrapping_add(cmd as u8);
            0x1000 + cmd
        })))
        .unwrap();
        let service = worker.service();
        let ticket = service.submit(3, 1, vec![7, 8]).unwrap();
        in_worker.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(
            service.committed(ticket).is_err(),
            "unfinished work cannot release the slot"
        );
        assert!(
            service.submit(3, 3, vec![1]).is_err(),
            "one async slot lasts through commit"
        );
        let sync = service.clone();
        let caller = std::thread::spawn(move || {
            let mut proxy = sync.proxy();
            let mut bytes = [11, 22];
            let status = proxy.control(4, 2, &mut bytes);
            (status, bytes)
        });
        release.send(()).unwrap();
        service.drain().unwrap(); // Queued host work finished; no pump has taken a result.
        assert_eq!(caller.join().unwrap(), (0x1002, [17, 22]));
        let answer = service.take(ticket).unwrap().unwrap();
        assert_eq!((answer.status, answer.params), (0x1001, vec![11, 8]));
        assert!(
            service.take(ticket).is_err(),
            "a completion cannot be consumed twice"
        );
        assert!(
            service.submit(3, 3, vec![1]).is_err(),
            "take alone does not release admission"
        );
        service.committed(ticket).unwrap();
        let next = service.submit(3, 3, vec![1]).unwrap();
        assert!(next.id > ticket.id && next.epoch == ticket.epoch);
        service.drain().unwrap();
        assert_eq!(service.take(next).unwrap().unwrap().params, vec![7]);
        service.committed(next).unwrap();
        worker.stop().unwrap();
    }

    #[test]
    fn identical_large_requests_keep_owned_bytes_and_changing_nv_answers() {
        let mut calls = 0u32;
        let worker = Worker::start(Box::new(Fake(move |class, cmd, params: &mut [u8]| {
            assert_eq!(class, 0x2080);
            assert!(eligible(cmd, params.len()));
            assert_eq!(params[0], 0x37);
            assert_eq!(params[params.len() - 1], 0xa5);
            calls += 1;
            params[0] = calls as u8;
            let last = params.len() - 1;
            params[last] = 0x80 + calls as u8;
            // A host NV failure is an exact guest-visible answer, not a worker fault.
            if calls == 1 {
                0x56
            } else {
                0
            }
        })))
        .unwrap();
        let service = worker.service();
        for (ordinal, (cmd, size)) in TARGETS.into_iter().enumerate() {
            let mut input = vec![0; size];
            input[0] = 0x37;
            input[size - 1] = 0xa5;
            let ticket = service.submit(0x2080, cmd, input.clone()).unwrap();
            service.drain().unwrap();
            let answer = service.take(ticket).unwrap().unwrap();
            assert_eq!(answer.status, if ordinal == 0 { 0x56 } else { 0 });
            assert_eq!(answer.params.len(), size);
            assert_eq!(answer.params[0], (ordinal + 1) as u8);
            assert_eq!(answer.params[size - 1], 0x81 + ordinal as u8);
            assert_eq!(input[0], 0x37);
            assert_eq!(input[size - 1], 0xa5);
            assert!(
                service.submit(0x2080, cmd, vec![0; size]).is_err(),
                "the taken reply still owns admission until committed"
            );
            service.committed(ticket).unwrap();
        }
        // Equal command and equal input still reaches host again. There is no
        // short-TTL answer cache in this worker, even after an NV error.
        let (cmd, size) = TARGETS[0];
        let mut input = vec![0; size];
        input[0] = 0x37;
        input[size - 1] = 0xa5;
        let ticket = service.submit(0x2080, cmd, input).unwrap();
        service.drain().unwrap();
        let answer = service.take(ticket).unwrap().unwrap();
        assert_eq!(
            (answer.status, answer.params[0], answer.params[size - 1]),
            (0, 3, 0x83)
        );
        service.committed(ticket).unwrap();
        worker.stop().unwrap();
    }

    #[test]
    fn retirement_waits_for_host_then_discards_old_generation() {
        let (entered, in_worker) = sync_channel(1);
        let (release, proceed) = sync_channel(0);
        let worker = Worker::start(Box::new(Fake(move |_, cmd, bytes: &mut [u8]| {
            if cmd == 1 {
                entered.send(()).unwrap();
                proceed.recv().unwrap();
            }
            bytes[0] = cmd as u8;
            cmd
        })))
        .unwrap();
        let service = worker.service();
        let old = service.submit(0, 1, vec![0]).unwrap();
        in_worker.recv_timeout(Duration::from_secs(1)).unwrap();
        let retire = service.clone();
        let (done, finished) = channel();
        let join = std::thread::spawn(move || {
            done.send(retire.retire()).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let epoch = service
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .epoch;
            if epoch == old.epoch + 1 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "retire did not advance its generation"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(matches!(
            finished.recv_timeout(Duration::from_millis(20)),
            Err(RecvTimeoutError::Timeout)
        ));
        release.send(()).unwrap();
        finished
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();
        join.join().unwrap();
        assert!(service.take(old).is_err());
        let fresh = service.submit(0, 2, vec![0]).unwrap();
        assert_eq!(fresh.epoch, old.epoch + 1);
        service.drain().unwrap();
        assert_eq!(service.take(fresh).unwrap().unwrap().params, vec![2]);
        service.committed(fresh).unwrap();
        worker.stop().unwrap();
    }

    #[test]
    fn close_orders_accepted_async_and_sync_before_rejecting_late_calls() {
        let (entered, in_worker) = sync_channel(1);
        let (release, proceed) = sync_channel(0);
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let worker = Worker::start(Box::new(Fake(move |_, cmd, bytes: &mut [u8]| {
            seen.fetch_add(1, Ordering::SeqCst);
            if cmd == 1 {
                entered.send(()).unwrap();
                proceed.recv().unwrap();
            }
            bytes[0] = cmd as u8;
            cmd
        })))
        .unwrap();
        let service = worker.service();
        service.submit(0, 1, vec![0]).unwrap();
        in_worker.recv_timeout(Duration::from_secs(1)).unwrap();
        let (reply, accepted_sync) = sync_channel(1);
        service
            .tx
            .try_send(Job::Call {
                class: 0,
                cmd: 2,
                params: vec![0],
                destination: Destination::Sync(reply),
                submitted: None,
            })
            .unwrap();
        let closer = service.clone();
        let (done, finished) = channel();
        let join = std::thread::spawn(move || {
            done.send(closer.close()).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if service
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .closed
            {
                break;
            }
            assert!(Instant::now() < deadline, "close did not seal admission");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(service.submit(0, 3, vec![0]).is_err());
        let mut proxy = service.proxy();
        let mut late = [9];
        assert_eq!(proxy.control(0, 3, &mut late), 0xffff_ffff);
        assert_eq!(late, [9]);
        assert!(
            matches!(
                finished.recv_timeout(Duration::from_millis(20)),
                Err(RecvTimeoutError::Timeout)
            ),
            "close must not release host backing before accepted calls finish"
        );
        release.send(()).unwrap();
        assert_eq!(
            accepted_sync
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .params,
            vec![2]
        );
        finished
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();
        join.join().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(service.check().is_err());
        let mut wake = 0;
        assert!(!service.wait(&mut wake), "close stops the pump");
        worker.stop().unwrap();
    }

    #[test]
    fn full_queue_fails_terminally_without_running_unaccepted_job() {
        let (entered, in_worker) = sync_channel(1);
        let (release, proceed) = sync_channel(0);
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let worker = Worker::start(Box::new(Fake(move |_, cmd, _: &mut [u8]| {
            seen.fetch_add(1, Ordering::SeqCst);
            if cmd == 1 {
                entered.send(()).unwrap();
                proceed.recv().unwrap();
            }
            cmd
        })))
        .unwrap();
        let service = worker.service();
        let first = service.clone();
        let caller = std::thread::spawn(move || first.call(0, 1, &[1]));
        in_worker.recv_timeout(Duration::from_secs(1)).unwrap();
        let (reply, queued_result) = sync_channel(1);
        service
            .tx
            .try_send(Job::Call {
                class: 0,
                cmd: 2,
                params: vec![2],
                destination: Destination::Sync(reply),
                submitted: None,
            })
            .unwrap();
        let error = service.submit(0, 3, vec![3]).unwrap_err();
        assert!(error.contains("admission failed"));
        assert!(service.check().is_err());
        let retiring = service.clone();
        let (drained, observed) = sync_channel(1);
        let retire = std::thread::spawn(move || drained.send(retiring.retire()).unwrap());
        assert!(
            observed.recv_timeout(Duration::from_millis(30)).is_err(),
            "fault does not bypass the accepted-call drain"
        );
        release.send(()).unwrap();
        assert!(observed
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .is_err());
        retire.join().unwrap();
        assert!(caller.join().unwrap().is_err(), "no postfault guest answer");
        assert_eq!(
            queued_result
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .status,
            2
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "both accepted calls run once; rejected third call never runs"
        );
        assert!(worker.stop().is_err());
    }

    #[test]
    fn worker_panic_wakes_pump_and_fails_sync_proxy() {
        let (entered, in_worker) = sync_channel(1);
        let (release, proceed) = sync_channel(0);
        let worker = Worker::start(Box::new(Fake(move |_, _, _: &mut [u8]| -> u32 {
            entered.send(()).unwrap();
            proceed.recv().unwrap();
            panic!("test host failure")
        })))
        .unwrap();
        let service = worker.service();
        service.submit(0, 1, vec![7]).unwrap();
        in_worker.recv_timeout(Duration::from_secs(1)).unwrap();
        let (reply, waiting_sync) = sync_channel(1);
        service
            .tx
            .try_send(Job::Call {
                class: 0,
                cmd: 2,
                params: vec![9],
                destination: Destination::Sync(reply),
                submitted: None,
            })
            .unwrap();
        release.send(()).unwrap();
        assert!(
            matches!(
                waiting_sync.recv_timeout(Duration::from_secs(1)),
                Err(RecvTimeoutError::Disconnected)
            ),
            "a sync caller queued behind a panic must be released"
        );
        let mut seen = 0;
        assert!(
            service.wait(&mut seen),
            "fault wakes the pump for terminal handling"
        );
        assert!(service.check().unwrap_err().contains("panicked"));
        let mut proxy = service.proxy();
        let mut bytes = [9];
        assert_eq!(proxy.control(0, 2, &mut bytes), 0xffff_ffff);
        assert_eq!(bytes, [9], "no fabricated answer or parameter mutation");
        assert!(worker.stop().is_err());
    }

    #[test]
    fn sync_proxy_preserves_caller_owned_nested_storage_until_host_returns() {
        let (entered, in_worker) = sync_channel(1);
        let (release, proceed) = sync_channel(0);
        let worker = Worker::start(Box::new(Fake(move |_, _, params: &mut [u8]| {
            let address =
                usize::from_ne_bytes(params[..std::mem::size_of::<usize>()].try_into().unwrap());
            entered.send(()).unwrap();
            proceed.recv().unwrap();
            // SAFETY: the sync caller owns this stack atomic and cannot return from
            // Proxy::control until this host call has finished and sent its answer.
            let nested = unsafe { &*(address as *const AtomicU64) };
            nested.store(0x1234_5678, Ordering::SeqCst);
            params[std::mem::size_of::<usize>()] = 0x5a;
            0x7788
        })))
        .unwrap();
        let service = worker.service();
        let (done, finished) = channel();
        let caller = std::thread::spawn(move || {
            let nested = AtomicU64::new(0);
            let mut params = vec![0; std::mem::size_of::<usize>() + 1];
            params[..std::mem::size_of::<usize>()]
                .copy_from_slice(&(&nested as *const AtomicU64 as usize).to_ne_bytes());
            let mut proxy = service.proxy();
            let status = proxy.control(0, 9, &mut params);
            done.send((status, params, nested.load(Ordering::SeqCst)))
                .unwrap();
        });
        in_worker.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(
            finished.recv_timeout(Duration::from_millis(20)),
            Err(RecvTimeoutError::Timeout)
        ));
        release.send(()).unwrap();
        let (status, bytes, nested) = finished.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(
            (status, nested, *bytes.last().unwrap()),
            (0x7788, 0x1234_5678, 0x5a)
        );
        caller.join().unwrap();
        worker.stop().unwrap();
    }

    #[test]
    fn idle_wait_blocks_until_explicit_stop() {
        let worker = Worker::start(Box::new(Fake(|_, _, _: &mut [u8]| 0))).unwrap();
        let service = worker.service();
        let waiter = service.clone();
        let (ready, started) = channel();
        let (done, finished) = channel();
        let join = std::thread::spawn(move || {
            ready.send(()).unwrap();
            let mut seen = 0;
            done.send(waiter.wait(&mut seen)).unwrap();
        });
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(
            finished.recv_timeout(Duration::from_millis(20)),
            Err(RecvTimeoutError::Timeout)
        ));
        service.stop_pump();
        assert!(!finished.recv_timeout(Duration::from_secs(1)).unwrap());
        join.join().unwrap();
        worker.stop().unwrap();
    }
}

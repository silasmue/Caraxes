// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! CPU-only tests through the real VFIO server, Device and MsgqV2 serializer.
use super::*;
use crate::boot::tests::{control_request, Guest615};
use crate::host_control::{Service, Worker};
use crate::msgq::{QueueMemory, RpcHeader};
use crate::ver::{GuestAbi, TestGuest};
use std::os::fd::AsRawFd;
use std::sync::mpsc;
use std::time::Duration;

const HEAD: u64 = 0x110c00;
const OUT_HEAD: u64 = 0x110c80;
const OUT_TAIL: u64 = 0x110c84;
const A612: u32 = 0x2080_a612;
const A613: u32 = 0x2080_a613;
const LIMIT: Duration = Duration::from_secs(5);

type Calls = Arc<Mutex<Vec<(u32, Vec<u8>)>>>;
struct Host {
    calls: Calls,
    entered: mpsc::Sender<u32>,
    release: mpsc::Receiver<()>,
    block: bool,
    status: u32,
    panic: bool,
}
impl crate::ctrl::HostRm for Host {
    fn control(&mut self, _: u32, cmd: u32, params: &mut [u8]) -> u32 {
        let n = {
            let mut calls = self.calls.lock().unwrap();
            calls.push((cmd, params.to_vec()));
            calls.len()
        };
        self.entered.send(cmd).unwrap();
        if self.block {
            self.block = false;
            self.release
                .recv_timeout(LIMIT)
                .expect("test releases blocked host call");
        }
        assert!(!self.panic, "injected host worker panic");
        for (i, byte) in params.iter_mut().enumerate() {
            *byte = byte
                .wrapping_add((i as u8).wrapping_mul(7))
                .wrapping_add(n as u8);
        }
        self.status
    }
}

struct Fixture {
    guest: Guest615,
    client: vfio_user::Client,
    device: Arc<Mutex<Device>>,
    worker: Option<Worker>,
    service: Option<Service>,
    server: Option<std::thread::JoinHandle<()>>,
    pump: Option<std::thread::JoinHandle<()>>,
    completed: mpsc::Receiver<Result<(), String>>,
    calls: Calls,
    entered: mpsc::Receiver<u32>,
    release: mpsc::Sender<()>,
    local: Option<(
        Arc<crate::intr_page::IntrPage>,
        vfio_user::local_irq::QueueConsumer,
        File,
        crate::intr_page::PageHook,
    )>,
}
impl Fixture {
    fn new(tag: &str, asynchronous: bool, blocked: bool, status: u32, panic: bool) -> Self {
        Self::with_local(tag, asynchronous, blocked, status, panic, false)
    }

    fn with_local(
        tag: &str,
        asynchronous: bool,
        blocked: bool,
        status: u32,
        panic: bool,
        local_irq: bool,
    ) -> Self {
        let mut guest = Guest615::boot(tag);
        let f = crate::fixture::rtx2070();
        let empty = crate::boot::BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(4 << 30),
            f.identity(4 << 30),
        );
        let boot = std::mem::replace(&mut guest.b, empty);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let (entered_tx, entered) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let host = Box::new(Host {
            calls: calls.clone(),
            entered: entered_tx,
            release: release_rx,
            block: blocked,
            status,
            panic,
        });
        let (boot, worker, service) = if asynchronous {
            let worker = Worker::start(host).unwrap();
            let service = worker.service();
            (
                boot.with_host(service.proxy())
                    .with_host_control(service.clone()),
                Some(worker),
                Some(service),
            )
        } else {
            (boot.with_host(host), None, None)
        };
        let mut device = Device::new(
            super::tests::rtx2070_device().cfg,
            Bar0::new(f.chip, Vec::new(), None).with_boot_model(boot),
            Trace::new(None).unwrap(),
        )
        .with_guest_dma_gpa(true);
        let local = local_irq.then(|| {
            let source = IntrShared::new();
            let page = crate::intr_page::IntrPage::create(
                16 << 20,
                f.chip.data().registers.intr_tree,
                f.chip.data().identity,
                source.clone(),
                crate::intr_page::Config::default(),
            )
            .unwrap();
            let (hook, wake) = crate::intr_page::PageHook::new(page.clone()).unwrap();
            source.attach_page(wake);
            (source, page, hook)
        });
        if let Some((source, page, _)) = &local {
            device.bar0 = device.bar0.with_intr_page(page.clone());
            device = device
                .with_intr_shared(source.clone())
                .with_local_irq(page.clone(), f.chip.data().bar0.pcfg + NV_XVE_CYA_2)
                .unwrap();
        }
        // GPA-only fake host fixture. Plain RAM requires no host registrations.
        device.mem.set_table(None);
        let device = Arc::new(Mutex::new(device));
        let mut backend = SharedDevice::with_gate(
            device.clone(),
            Routes::default(),
            Arc::new(crate::ringgate::RingGate::default()),
        );
        let board = Board::load("rtx2070").unwrap();
        let path =
            std::env::temp_dir().join(format!("caraxes-offlock-{tag}-{}.sock", std::process::id()));
        let server = vfio_user::Server::new(&path, true, irqs(), regions(&board)).unwrap();
        let server = std::thread::spawn(move || {
            let _abi = TestGuest::set(GuestAbi::V615);
            let _ = server.run(&mut backend);
        });
        let mut client = vfio_user::Client::new_with_options(
            &path,
            vfio_user::ClientOptions {
                local_irq_required: local_irq,
                bar_shmem_required: false,
            },
        )
        .unwrap();
        let local = local.map(|(source, page, hook)| {
            let prepared = client.local_irq_prepare().unwrap();
            let consumer = vfio_user::local_irq::QueueConsumer::map(&prepared).unwrap();
            client
                .local_irq_commit(prepared.metadata.token, prepared.metadata.epoch, 0)
                .unwrap();
            // CPU fixture replaces only the hardware execution source attachment.
            // Production with_exec performs this attachment after each reset.
            device.lock().unwrap().bar0.boot_mut().unwrap().intr = crate::intr::IntrTree::new(
                f.chip.data().registers.intr_tree,
                f.chip.data().registers.intr_tree_size,
            )
            .with_shared(source);
            (page, consumer, prepared.event_fd, hook)
        });
        client
            .local_irq_liveness_stream()
            .unwrap()
            .set_read_timeout(Some(LIMIT))
            .unwrap();
        // Initial mappings are installed directly because the already-booted fixture owns
        // this link. Subsequent map/unmap lifecycle tests use actual protocol messages.
        let ram = File::options()
            .read(true)
            .write(true)
            .open(&guest.path)
            .unwrap();
        {
            let mut d = device.lock().unwrap();
            for (address, size, offset) in
                [(0x0769_0000, 0x20000, 0), (0x88a0_0000, 0x81000, 0x20000)]
            {
                d.mem.map(address, size, offset, &ram).unwrap();
                d.dma.push(DmaMapping {
                    address,
                    size,
                    offset,
                    fd: Some(ram.try_clone().unwrap()),
                });
            }
            d.cfg.write(0x6a, &[1]);
        }
        if local_irq {
            client.region_write(0, HEAD, &0u32.to_le_bytes()).unwrap();
            for (offset, value) in [
                (0x110000 + crate::falcon::MAILBOX0, 0x0769_d000u32),
                (0x110000 + crate::falcon::MAILBOX1, 0),
                (0x840000 + crate::falcon::MAILBOX0, 0x0769_c000),
                (0x840000 + crate::falcon::MAILBOX1, 0),
                (0x840000 + crate::falcon::CPUCTL, 2),
            ] {
                client
                    .region_write(0, offset, &value.to_le_bytes())
                    .unwrap();
            }
            let slot0 = std::fs::read("../../caraxes/runs/guest-driver-2026-09-24/615.71.09/linked/gmcapi-gsp-init-slot0.bin").unwrap();
            assert!(guest.mem.write(0x88a0_2000, &slot0));
            client
                .region_write(0, OUT_TAIL, &0u32.to_le_bytes())
                .unwrap();
            client.region_write(0, HEAD, &1u32.to_le_bytes()).unwrap();
            guest.cpu.read_ptr = 0;
            let mut head = [0; 4];
            client.region_read(0, OUT_HEAD, &mut head).unwrap();
            let qm = QueueMemory::new(&guest.mem, &guest.pages);
            assert!(guest.cpu.receive(&qm, u32::from_le_bytes(head)).is_some());
        }
        // The ordinary MSI path is registered to an actual eventfd by each IRQ test.
        client
            .region_write(0, OUT_TAIL, &guest.cpu.read_ptr.to_le_bytes())
            .unwrap();
        let (done, completed) = mpsc::channel();
        let pump = service.as_ref().map(|service| {
            let service = service.clone();
            let device = device.clone();
            std::thread::spawn(move || {
                let _abi = TestGuest::set(GuestAbi::V615);
                let mut seen = 0;
                while service.wait(&mut seen) {
                    let result = device
                        .lock()
                        .unwrap()
                        .complete_host_controls()
                        .map_err(|e| e.to_string());
                    let failed = result.is_err();
                    let _ = done.send(result);
                    if failed {
                        break;
                    }
                }
            })
        });
        Self {
            guest,
            client,
            device,
            worker,
            service,
            server: Some(server),
            pump,
            completed,
            calls,
            entered,
            release,
            local,
        }
    }

    fn put(&mut self, function: u32, payload: &[u8]) {
        let h = RpcHeader {
            header_version: crate::ver::common::RPC_HEADER_VERSION,
            signature: crate::ver::common::RPC_SIGNATURE,
            length: (32 + payload.len()) as u32,
            function,
            rpc_result: u32::MAX,
            rpc_result_private: u32::MAX,
            sequence: self.guest.sequence,
            u: 0,
        };
        self.guest.sequence = self.guest.sequence.wrapping_add(1);
        let mut body = h.encode().to_vec();
        body.extend_from_slice(payload);
        self.guest.cpu.send(
            &QueueMemory::new(&self.guest.mem, &self.guest.pages),
            crate::msgq::v2::NVDM_TYPE_RM_RPC,
            &body,
        );
        self.client
            .region_write(0, HEAD, &self.guest.cpu.write_ptr.to_le_bytes())
            .unwrap();
    }

    fn control(&mut self, cmd: u32, params: &[u8]) {
        let request = control_request(
            crate::ctrl::INTERNAL_CLIENT,
            crate::ctrl::INTERNAL_SUBDEVICE,
            cmd,
            params,
        );
        for (i, chunk) in request
            .chunks(crate::msgq::v2::MAX_RPC_SIZE - 32)
            .enumerate()
        {
            self.put(
                if i == 0 {
                    crate::rpc::FN_GSP_RM_CONTROL
                } else {
                    crate::rpc::FN_CONTINUATION_RECORD
                },
                chunk,
            );
        }
    }

    fn drain(&mut self) -> Vec<(u32, Vec<u8>)> {
        let mut head = [0; 4];
        self.client.region_read(0, OUT_HEAD, &mut head).unwrap();
        let qm = QueueMemory::new(&self.guest.mem, &self.guest.pages);
        let mut out = Vec::new();
        while let Some(record) = self.guest.cpu.receive(&qm, u32::from_le_bytes(head)) {
            out.push(record);
        }
        self.client
            .region_write(0, OUT_TAIL, &self.guest.cpu.read_ptr.to_le_bytes())
            .unwrap();
        out
    }

    fn complete(&self) {
        self.completed
            .recv_timeout(LIMIT)
            .expect("independent completion wake")
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.release.send(());
        let _ = self.client.shutdown();
        if let Some(server) = self.server.take() {
            server.join().unwrap();
        }
        let _ = self.device.lock().unwrap().drain_host_controls();
        if let Some(service) = &self.service {
            service.stop_pump();
        }
        if let Some(pump) = self.pump.take() {
            pump.join().unwrap();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.stop();
        }
    }
}

#[test]
fn offlock_matches_actual_synchronous_msgq_replies_and_calls() {
    let _abi = TestGuest::pin(None);
    for (cmd, size) in [(A612, 54352), (A613, 79936)] {
        for status in [0, 0x57, 0x1b] {
            let input: Vec<u8> = (0..size).map(|i| (i as u8).wrapping_mul(3)).collect();
            let mut sync =
                Fixture::new(&format!("sync-{cmd}-{status}"), false, false, status, false);
            let mut off =
                Fixture::new(&format!("async-{cmd}-{status}"), true, false, status, false);
            for _ in 0..2 {
                sync.control(cmd, &input);
                off.control(cmd, &input);
                off.complete();
                assert_eq!(
                    off.drain(),
                    sync.drain(),
                    "cmd {cmd:#x}, status {status:#x}"
                );
            }
            assert_eq!(*off.calls.lock().unwrap(), *sync.calls.lock().unwrap());
            assert_eq!(
                off.calls.lock().unwrap().len(),
                2,
                "equal inputs are never coalesced or cached"
            );
        }
    }
}

#[test]
fn blocked_ioctl_releases_device_and_does_not_dispatch_the_next_rpc() {
    let _abi = TestGuest::pin(None);
    let mut f = Fixture::new("release-lock", true, true, 0, false);
    f.control(A613, &vec![0x41; 79936]);
    assert_eq!(f.entered.recv_timeout(LIMIT).unwrap(), A613);
    assert!(
        f.device.try_lock().is_ok(),
        "actual host call does not own Device"
    );
    assert!(f.drain().is_empty(), "no early reply while host is blocked");
    f.control(A612, &vec![0x52; 54352]);
    assert_eq!(f.calls.lock().unwrap().len(), 1, "later RPC remains queued");
    f.release.send(()).unwrap();
    f.complete();
    f.complete();
    let out = f.drain();
    let functions: Vec<u32> = out
        .iter()
        .map(|(_, body)| RpcHeader::parse(body).function)
        .collect();
    assert_eq!(
        functions,
        [
            crate::rpc::FN_GSP_RM_CONTROL,
            crate::rpc::FN_CONTINUATION_RECORD,
            crate::rpc::FN_GSP_RM_CONTROL
        ]
    );
    assert_eq!(
        f.calls
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.0)
            .collect::<Vec<_>>(),
        [A613, A612]
    );
}

#[test]
fn malformed_size_does_not_enter_async_admission() {
    let _abi = TestGuest::pin(None);
    for truncated in [false, true] {
        let mut f = Fixture::new(
            if truncated { "truncated" } else { "wrongsize" },
            true,
            false,
            0x57,
            false,
        );
        let mut request = control_request(
            crate::ctrl::INTERNAL_CLIENT,
            crate::ctrl::INTERNAL_SUBDEVICE,
            A612,
            &[0x73; 8],
        );
        if truncated {
            request[16..20].copy_from_slice(&54352u32.to_le_bytes());
        }
        f.put(crate::rpc::FN_GSP_RM_CONTROL, &request);
        assert!(
            !f.drain().is_empty(),
            "ineligible request stays on the synchronous route"
        );
        assert!(
            f.completed.try_recv().is_err(),
            "no asynchronous job/result wake"
        );
        assert_eq!(
            f.calls.lock().unwrap()[0].1.len(),
            if truncated { 0 } else { 8 }
        );
    }
}

#[test]
fn socket_reset_drains_blocked_call_and_discards_its_old_reply() {
    let _abi = TestGuest::pin(None);
    let mut f = Fixture::new("reset-drain", true, true, 0, false);
    f.control(A613, &vec![0x61; 79936]);
    f.entered.recv_timeout(LIMIT).unwrap();
    let before = f
        .guest
        .mem
        .read_u32(
            0x88a0_0000
                + 0x42000
                + u64::from(f.guest.cpu.read_ptr % f.guest.cpu.hdr.msg_count) * 4096,
        )
        .unwrap();
    let (done, observed) = mpsc::channel();
    std::thread::scope(|scope| {
        let client = &mut f.client;
        scope.spawn(move || {
            done.send(client.reset().map_err(|e| e.to_string()))
                .unwrap();
        });
        assert!(
            observed.recv_timeout(Duration::from_millis(30)).is_err(),
            "reset waits for actual call"
        );
        f.release.send(()).unwrap();
        observed.recv_timeout(LIMIT).unwrap().unwrap();
    });
    assert_eq!(f.calls.lock().unwrap().len(), 1);
    assert_eq!(
        f.guest.mem.read_u32(
            0x88a0_0000
                + 0x42000
                + u64::from(f.guest.cpu.read_ptr % f.guest.cpu.hdr.msg_count) * 4096
        ),
        Some(before),
        "no old response published after reset"
    );
    assert!(f
        .device
        .lock()
        .unwrap()
        .bar0
        .boot()
        .unwrap()
        .host_control_error()
        .is_none());
}

#[test]
fn gpa_unmap_distinguishes_queue_backing_and_unrelated_ram() {
    let _abi = TestGuest::pin(None);
    let mut f = Fixture::new("unmap-drain", true, true, 0, false);
    let ram = File::options()
        .read(true)
        .write(true)
        .open(&f.guest.path)
        .unwrap();
    f.client
        .dma_map(0, 0x3000_0000, 0x1000, ram.as_raw_fd())
        .unwrap();
    f.control(A612, &vec![0x19; 54352]);
    f.entered.recv_timeout(LIMIT).unwrap();
    f.client.dma_unmap(0x3000_0000, 0x1000).unwrap();
    assert_eq!(f.calls.lock().unwrap().len(), 1);
    let (done, observed) = mpsc::channel();
    std::thread::scope(|scope| {
        let client = &mut f.client;
        scope.spawn(move || {
            done.send(
                client
                    .dma_unmap(0x88a0_0000, 0x81000)
                    .map_err(|e| e.to_string()),
            )
            .unwrap();
        });
        assert!(
            observed.recv_timeout(Duration::from_millis(30)).is_err(),
            "queue unmap drains accepted ioctl"
        );
        f.release.send(()).unwrap();
        observed.recv_timeout(LIMIT).unwrap().unwrap();
    });
    f.client
        .dma_map(0x20000, 0x88a0_0000, 0x81000, ram.as_raw_fd())
        .unwrap();
    let mut head = [0; 4];
    f.client.region_read(0, OUT_HEAD, &mut head).unwrap();
    assert_eq!(
        u32::from_le_bytes(head),
        0,
        "same-GPA replacement has no old queue link or reply"
    );
}

#[test]
fn completion_publishes_reply_and_msi_without_another_guest_access() {
    let _abi = TestGuest::pin(None);
    let mut f = Fixture::new("independent-msi", true, true, 0, false);
    let fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
    assert!(fd >= 0);
    let reader = unsafe { File::from_raw_fd(fd) };
    {
        let mut d = f.device.lock().unwrap();
        d.set_irqs(
            VFIO_PCI_MSI_IRQ_INDEX,
            VFIO_IRQ_SET_DATA_EVENTFD | VFIO_IRQ_SET_ACTION_TRIGGER,
            0,
            1,
            vec![reader.try_clone().unwrap()],
        )
        .unwrap();
        let boot = d.bar0.boot_mut().unwrap();
        boot.ctrl.display.enabled = false;
        let chip = crate::fixture::rtx2070().chip.data();
        let gsp = crate::ver::common::MC_ENGINE_IDX
            .iter()
            .find(|(name, _)| *name == "GSP")
            .unwrap()
            .1;
        let vector = chip
            .interrupts
            .entries
            .iter()
            .find(|e| e.mc_idx as u32 == gsp)
            .unwrap()
            .vector_stall;
        let base = chip.registers.intr_tree;
        boot.intr.write32(
            base + crate::intr::LEAF_EN_SET + u64::from(vector / 32) * 4,
            1 << (vector % 32),
        );
        boot.intr
            .write32(base + crate::intr::TOP_EN_SET, 1 << (vector / 64));
    }
    let before = f
        .guest
        .mem
        .read_u32(
            0x88a0_0000
                + 0x42000
                + u64::from(f.guest.cpu.read_ptr % f.guest.cpu.hdr.msg_count) * 4096,
        )
        .unwrap();
    f.control(A612, &vec![0x42; 54352]);
    f.entered.recv_timeout(LIMIT).unwrap();
    f.release.send(()).unwrap();
    f.complete();
    // No guest read, tail write, timer tick or unrelated VFIO request drives completion.
    let mut value = 0u64;
    assert_eq!(
        unsafe { libc::read(reader.as_raw_fd(), (&mut value as *mut u64).cast(), 8) },
        8
    );
    assert_eq!(value, 1);
    assert_ne!(
        f.guest.mem.read_u32(
            0x88a0_0000
                + 0x42000
                + u64::from(f.guest.cpu.read_ptr % f.guest.cpu.hdr.msg_count) * 4096
        ),
        Some(before)
    );
    assert_eq!(f.drain().len(), 1);
}

#[test]
fn synchronous_l2_waits_behind_async_host_call_without_waiting_for_pump() {
    let _abi = TestGuest::pin(None);
    let mut f = Fixture::new("l2-order", true, true, 0, false);
    f.device
        .lock()
        .unwrap()
        .bar0
        .boot_mut()
        .unwrap()
        .l2_host_for_test = true;
    f.control(A612, &vec![0x37; 54352]);
    f.entered.recv_timeout(LIMIT).unwrap();
    let (done, observed) = mpsc::channel();
    let offset = crate::fixture::rtx2070()
        .chip
        .data()
        .registers
        .l2_flush_dirty;
    std::thread::scope(|scope| {
        let client = &mut f.client;
        scope.spawn(move || {
            done.send(
                client
                    .region_write(0, offset, &1u32.to_le_bytes())
                    .map_err(|e| e.to_string()),
            )
            .unwrap();
        });
        assert!(observed.recv_timeout(Duration::from_millis(30)).is_err());
        assert!(
            f.device.try_lock().is_err(),
            "ordinary synchronous flush holds Device while waiting for FIFO"
        );
        f.release.send(()).unwrap();
        observed.recv_timeout(LIMIT).unwrap().unwrap();
    });
    f.complete();
    assert_eq!(
        f.calls
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.0)
            .collect::<Vec<_>>(),
        [A612, crate::hostabi::CMD_FB_FLUSH_GPU_CACHE]
    );
    assert_eq!(
        f.device.lock().unwrap().bar0.boot().unwrap().l2_op_counts,
        (1, 1, 0)
    );
    assert_eq!(f.drain().len(), 1);
}

#[test]
fn synchronous_l2_worker_panic_fails_current_and_future_vfio_access() {
    let _abi = TestGuest::pin(None);
    let mut f = Fixture::new("l2-panic", true, false, 0, true);
    f.device
        .lock()
        .unwrap()
        .bar0
        .boot_mut()
        .unwrap()
        .l2_host_for_test = true;
    let offset = crate::fixture::rtx2070()
        .chip
        .data()
        .registers
        .l2_flush_dirty;
    assert!(f
        .client
        .region_write(0, offset, &1u32.to_le_bytes())
        .is_err());
    assert!(f.device.lock().unwrap().reset_error.is_some());
    assert!(f.client.region_read(0, OUT_HEAD, &mut [0; 4]).is_err());
    assert_eq!(
        f.calls
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.0)
            .collect::<Vec<_>>(),
        [crate::hostabi::CMD_FB_FLUSH_GPU_CACHE]
    );
}

#[test]
fn flr_head_zero_and_booter_replacement_drain_before_retiring_reply() {
    let _abi = TestGuest::pin(None);
    for kind in 0..3 {
        let mut f = Fixture::new(&format!("retire-{kind}"), true, true, 0, false);
        // Prepare the firmware mailboxes before a blocked control. Only CPUCTL starts
        // Booter Load and must wait for the old host job before replacing queue state.
        if kind == 2 {
            for (offset, value) in [
                (0x110000 + crate::falcon::MAILBOX0, 0x0769_d000u32),
                (0x110000 + crate::falcon::MAILBOX1, 0),
                (0x840000 + crate::falcon::MAILBOX0, 0x0769_c000),
                (0x840000 + crate::falcon::MAILBOX1, 0),
            ] {
                f.client
                    .region_write(0, offset, &value.to_le_bytes())
                    .unwrap();
            }
        }
        f.control(A612, &vec![0x6c; 54352]);
        f.entered.recv_timeout(LIMIT).unwrap();
        let before = f
            .guest
            .mem
            .read_u32(
                0x88a0_0000
                    + 0x42000
                    + u64::from(f.guest.cpu.read_ptr % f.guest.cpu.hdr.msg_count) * 4096,
            )
            .unwrap();
        let (done, observed) = mpsc::channel();
        std::thread::scope(|scope| {
            let client = &mut f.client;
            scope.spawn(move || {
                let result = match kind {
                    0 => client.region_write(7, 0x80, &0x8000u16.to_le_bytes()),
                    1 => client.region_write(0, HEAD, &0u32.to_le_bytes()),
                    _ => client.region_write(
                        0,
                        0x840000 + crate::falcon::CPUCTL,
                        &2u32.to_le_bytes(),
                    ),
                };
                done.send(result.map_err(|e| e.to_string())).unwrap();
            });
            assert!(
                observed.recv_timeout(Duration::from_millis(30)).is_err(),
                "lifecycle kind {kind} waits for host call"
            );
            f.release.send(()).unwrap();
            observed.recv_timeout(LIMIT).unwrap().unwrap();
        });
        assert_eq!(f.calls.lock().unwrap().len(), 1);
        assert_eq!(
            f.guest.mem.read_u32(
                0x88a0_0000
                    + 0x42000
                    + u64::from(f.guest.cpu.read_ptr % f.guest.cpu.hdr.msg_count) * 4096
            ),
            Some(before),
            "no old reply for lifecycle {kind}"
        );
        assert!(f
            .device
            .lock()
            .unwrap()
            .bar0
            .boot()
            .unwrap()
            .host_control_error()
            .is_none());
    }
}

#[test]
fn socket_disconnect_drains_actual_host_before_retiring_backend() {
    let _abi = TestGuest::pin(None);
    let mut f = Fixture::new("disconnect-drain", true, true, 0, false);
    f.control(A612, &vec![0x9a; 54352]);
    f.entered.recv_timeout(LIMIT).unwrap();
    let before = f
        .guest
        .mem
        .read_u32(
            0x88a0_0000
                + 0x42000
                + u64::from(f.guest.cpu.read_ptr % f.guest.cpu.hdr.msg_count) * 4096,
        )
        .unwrap();
    f.client.shutdown().unwrap();
    let server = f.server.take().unwrap();
    let (done, observed) = mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            server.join().unwrap();
            done.send(()).unwrap();
        });
        assert!(observed.recv_timeout(Duration::from_millis(30)).is_err());
        f.release.send(()).unwrap();
        observed.recv_timeout(LIMIT).unwrap();
    });
    assert_eq!(f.calls.lock().unwrap().len(), 1);
    assert_eq!(
        f.guest.mem.read_u32(
            0x88a0_0000
                + 0x42000
                + u64::from(f.guest.cpu.read_ptr % f.guest.cpu.hdr.msg_count) * 4096
        ),
        Some(before)
    );
}

#[test]
fn local_owner_gets_completion_cause_after_reply_publication_without_guest_access() {
    let _abi = TestGuest::pin(None);
    let mut f = Fixture::with_local("local-completion", true, true, 0, false, true);
    let slot =
        0x88a0_0000 + 0x42000 + u64::from(f.guest.cpu.read_ptr % f.guest.cpu.hdr.msg_count) * 4096;
    let before = f.guest.mem.read_u32(slot).unwrap();
    {
        let (_, consumer, wake, _) = f.local.as_mut().unwrap();
        let tail = consumer.published_tail().unwrap();
        consumer.drain_through(tail).unwrap();
        let mut n = 0u64;
        while unsafe { libc::read(wake.as_raw_fd(), (&mut n as *mut u64).cast(), 8) } == 8 {}
    }
    f.control(A613, &vec![0x2a; 79936]);
    f.entered.recv_timeout(LIMIT).unwrap();
    {
        let (_, consumer, _, _) = f.local.as_ref().unwrap();
        assert_eq!(
            consumer.published_tail().unwrap(),
            consumer.consumed_ticket(),
            "no premature cause while host has no answer"
        );
    }
    f.release.send(()).unwrap();
    f.complete();
    // The same completion pump published the status bytes then the typed IRQ queue.
    assert_ne!(f.guest.mem.read_u32(slot), Some(before));
    let (_, consumer, wake, _) = f.local.as_mut().unwrap();
    let tail = consumer.published_tail().unwrap();
    let records = consumer.drain_through(tail).unwrap();
    let chip = crate::fixture::rtx2070().chip.data();
    let gsp = crate::ver::common::MC_ENGINE_IDX
        .iter()
        .find(|(n, _)| *n == "GSP")
        .unwrap()
        .1;
    let vector = chip
        .interrupts
        .entries
        .iter()
        .find(|e| e.mc_idx as u32 == gsp)
        .unwrap()
        .vector_stall;
    assert!(records.iter().any(
        |record| matches!(record.kind, vfio_user::local_irq::RecordKind::Fold(bits)
        if bits[vector as usize / 32] & (1 << (vector % 32)) != 0)
    ));
    let mut n = 0u64;
    assert_eq!(
        unsafe { libc::read(wake.as_raw_fd(), (&mut n as *mut u64).cast(), 8) },
        8
    );
    assert!(n > 0);
    assert!(
        f.device.lock().unwrap().irq_fds.iter().all(Vec::is_empty),
        "backend never receives VMM MSI eventfd"
    );
    assert_eq!(f.drain().len(), 2);
}

#[test]
fn local_owner_worker_fault_closes_cause_queue_and_socket() {
    let _abi = TestGuest::pin(None);
    let mut f = Fixture::with_local("local-fault", true, true, 0, true, true);
    f.control(A612, &vec![0x28; 54352]);
    f.entered.recv_timeout(LIMIT).unwrap();
    f.release.send(()).unwrap();
    assert!(f.completed.recv_timeout(LIMIT).unwrap().is_err());
    assert!(
        f.local.as_ref().unwrap().1.published_tail().is_err(),
        "VMM learns terminal state independently"
    );
    assert!(f
        .device
        .lock()
        .unwrap()
        .terminal_fault
        .load(Ordering::Acquire));
    assert!(f.client.region_read(0, OUT_HEAD, &mut [0; 4]).is_err());
}

#[test]
fn held_async_batch_is_atomic_and_later_partial_flush_is_terminal() {
    let _abi = TestGuest::pin(None);
    let mut f = Fixture::new("held-batch", true, false, 0, false);
    {
        let mut d = f.device.lock().unwrap();
        let Device { bar0, mem, .. } = &mut *d;
        let events: Vec<_> = (0..126)
            .map(|_| crate::events::Unsolicited::Event(0x1234, vec![1]))
            .collect();
        bar0.boot_mut().unwrap().post_unsolicited(mem, &events);
        assert_eq!(
            crate::boot::tests::host_reply_state(bar0.boot().unwrap()).1,
            63
        );
    }
    f.control(A613, &vec![0x24; 79936]);
    f.complete();
    assert!(
        crate::boot::tests::host_reply_state(f.device.lock().unwrap().bar0.boot().unwrap()).0,
        "63 held records cannot admit either half of a two-record reply"
    );
    assert_eq!(f.calls.lock().unwrap().len(), 1);
    assert_eq!(f.drain().len(), 63);
    let (pending, held, head) =
        crate::boot::tests::host_reply_state(f.device.lock().unwrap().bar0.boot().unwrap());
    assert!(
        !pending && held == 2,
        "both reply records admitted together after the earlier events"
    );
    // Advance only the guest cursor. The next TAIL write must flush the held pair.
    let qm = QueueMemory::new(&f.guest.mem, &f.guest.pages);
    let mut consumed = 0;
    while f.guest.cpu.receive(&qm, head).is_some() {
        consumed += 1;
    }
    assert_eq!(consumed, 63);
    let first_slot = head % f.guest.cpu.hdr.msg_count;
    let second_slot = head.wrapping_add(16) % f.guest.cpu.hdr.msg_count;
    let first_at = 0x88a0_0000 + 0x42000 + u64::from(first_slot) * 4096;
    let before = f.guest.mem.read_u32(first_at);
    // Inject a backing write failure after the first record, without running a DMA
    // lifecycle transition (which correctly drains and clears the whole queue).
    crate::boot::tests::break_host_reply_page(
        f.device.lock().unwrap().bar0.boot_mut().unwrap(),
        66 + second_slot as usize,
    );
    assert!(f
        .client
        .region_write(0, OUT_TAIL, &f.guest.cpu.read_ptr.to_le_bytes())
        .is_err());
    assert_ne!(
        f.guest.mem.read_u32(first_at),
        before,
        "first record was physically written before second failed"
    );
    assert!(f
        .device
        .lock()
        .unwrap()
        .terminal_fault
        .load(Ordering::Acquire));
    assert_eq!(
        f.calls.lock().unwrap().len(),
        1,
        "neither backpressure nor partial failure retries the ioctl"
    );
}

#[test]
fn incomplete_and_rejected_continuations_never_submit_a_host_job() {
    let _abi = TestGuest::pin(None);
    let mut f = Fixture::new("torn-continuation", true, false, 0, false);
    let request = control_request(
        crate::ctrl::INTERNAL_CLIENT,
        crate::ctrl::INTERNAL_SUBDEVICE,
        A613,
        &vec![0x19; 79936],
    );
    let max = crate::msgq::v2::MAX_RPC_SIZE - 32;
    f.put(crate::rpc::FN_GSP_RM_CONTROL, &request[..max]);
    assert!(f.calls.lock().unwrap().is_empty());
    assert!(f.drain().is_empty());
    let header = RpcHeader {
        header_version: crate::ver::common::RPC_HEADER_VERSION,
        signature: crate::ver::common::RPC_SIGNATURE,
        length: (32 + request.len() - max) as u32,
        function: crate::rpc::FN_CONTINUATION_RECORD,
        rpc_result: u32::MAX,
        rpc_result_private: u32::MAX,
        sequence: f.guest.sequence,
        u: 0,
    };
    let mut body = header.encode().to_vec();
    body.extend_from_slice(&request[max..]);
    let qm = QueueMemory::new(&f.guest.mem, &f.guest.pages);
    let start = f.guest.cpu.write_ptr;
    let mut raw = f
        .guest
        .cpu
        .send(&qm, crate::msgq::v2::NVDM_TYPE_RM_RPC, &body);
    raw[0..4].copy_from_slice(&0xffff_ffffu32.to_le_bytes());
    f.guest.cpu.write_ptr = start;
    f.guest.cpu.raw(&qm, &raw);
    f.client
        .region_write(0, HEAD, &f.guest.cpu.write_ptr.to_le_bytes())
        .unwrap();
    assert!(f.calls.lock().unwrap().is_empty());
    assert!(f.completed.try_recv().is_err());
}

#[test]
fn selected_targets_match_active_legacy_abi_and_narrow_flag_gate() {
    for (cmd, size) in crate::host_control::TARGETS {
        assert!(crate::nvml::LEGACY
            .iter()
            .any(|&(c, s, _)| c == cmd && s == size));
    }
    assert!(crate::host_control::validate_options(
        true,
        |_| true,
        |_| false,
        |_| false,
        false,
        false
    )
    .is_ok());
    assert!(crate::host_control::validate_options(
        true,
        |c| c == A612,
        |_| false,
        |_| false,
        false,
        false
    )
    .is_err());
    assert!(crate::host_control::validate_options(
        false,
        |_| false,
        |c| c == A613,
        |_| false,
        false,
        false
    )
    .is_err());
    assert!(crate::host_control::validate_options(
        false,
        |_| false,
        |_| false,
        |c| c == A612,
        false,
        false
    )
    .is_err());
    assert!(crate::host_control::validate_options(
        false,
        |_| false,
        |_| false,
        |_| false,
        true,
        false
    )
    .is_err());
    assert!(crate::host_control::validate_options(
        false,
        |_| false,
        |_| false,
        |_| false,
        false,
        true
    )
    .is_err());
}

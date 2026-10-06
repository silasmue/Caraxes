// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Silas Müller
// Linked by tools/check-local-irq.py against the paired, unmodified source files.

#[cfg(test)]
mod linked {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use vfio_user::{Client, ClientOptions, Server};
    use vfio_user_nvgpu::bar0::Bar0;
    use vfio_user_nvgpu::board::Board;
    use vfio_user_nvgpu::boot::BootModel;
    use vfio_user_nvgpu::chip;
    use vfio_user_nvgpu::config::{ConfigSpace, parse_dump};
    use vfio_user_nvgpu::device::{Device, NV_XVE_CYA_2, Routes, SharedDevice, irqs, regions};
    use vfio_user_nvgpu::fb::FbLayout;
    use vfio_user_nvgpu::intr::IntrShared;
    use vfio_user_nvgpu::intr_page::{Config, IntrPage, PageHook};
    use vfio_user_nvgpu::rpc::StaticIdentity;
    use vfio_user_nvgpu::trace::Trace;
    use vmm_sys_util::eventfd::{EFD_NONBLOCK, EventFd};

    use crate::caraxes_irq::{LEAF, LEAF_EN_SET, LEAF_TRIGGER, PAGE_SIZE, TOP, TOP_EN_SET};
    use crate::caraxes_irq_owner::LocalIrqOwner;

    const TREE: u64 = 0xb81000;

    struct Fixture {
        client: Arc<Mutex<Client>>,
        device: Arc<Mutex<Device>>,
        owner: Arc<LocalIrqOwner>,
        shared: Arc<IntrShared>,
        page: Arc<IntrPage>,
        msi: EventFd,
        exit: EventFd,
        _hook: PageHook,
        server: Option<JoinHandle<()>>,
        socket: PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let board = Board::load("rtx2070").unwrap();
            let chip = chip::by_name(&board.chip).unwrap();
            let rearm = chip.data().bar0.pcfg + NV_XVE_CYA_2;
            let dump =
                std::fs::read_to_string("caraxes/reference/rtx2070/config-space.txt").unwrap();
            let cfg =
                ConfigSpace::from_dump(parse_dump(&dump).unwrap(), board.bars, board.rom_bar_size)
                    .unwrap();
            let identity =
                StaticIdentity::new(chip.data(), &board, cfg.ids(), &board.name, 4 << 30);
            let shared = IntrShared::new();
            let page = IntrPage::create(
                16 << 20,
                TREE,
                chip.data().identity,
                shared.clone(),
                Config {
                    sleep_ms: 1,
                    retrigger: Some(Duration::from_secs(1)),
                },
            )
            .unwrap();
            let (hook, wake) = PageHook::new(page.clone()).unwrap();
            shared.attach_page(wake);
            // Establish a fresh exact tree before the boot-only handoff.
            page.write(TREE + LEAF_EN_SET, &0u32.to_le_bytes());
            let bar0 = Bar0::new(chip, vec![0; 512], None)
                .with_boot_model(BootModel::new(chip, FbLayout::plain(4 << 30), identity))
                .with_intr_page(page.clone());
            let device = Device::new(cfg, bar0, Trace::new(None).unwrap())
                .with_intr_shared(shared.clone())
                .with_local_irq(page.clone(), rearm)
                .unwrap();
            let routes = Routes {
                terminal_fault: Some(device.terminal_fault_handle()),
                intr_page: Some(page.clone()),
                rearm: Some(rearm),
                ..Default::default()
            };
            let device = Arc::new(Mutex::new(device));
            let mut backend = SharedDevice::new(device.clone(), routes);
            let socket = std::env::temp_dir().join(format!(
                "caraxes-local-irq-{}-{name}.sock",
                std::process::id()
            ));
            assert!(
                !socket.exists(),
                "Refusing to replace an existing test socket"
            );
            let server = Server::new(&socket, true, irqs(), regions(&board)).unwrap();
            let server = thread::spawn(move || {
                // Faulted sessions may end with a transport error. Joining still
                // verifies that disconnect/fault wakes and retires the real server.
                let _ = server.run(&mut backend);
            });
            let mut client = Client::new_with_options(
                &socket,
                ClientOptions {
                    local_irq_required: true,
                },
            )
            .unwrap();
            let prepared = client.local_irq_prepare().unwrap();
            let metadata = prepared.metadata.clone();
            let exit = EventFd::new(EFD_NONBLOCK).unwrap();
            let owner = LocalIrqOwner::prepare(
                prepared,
                client.local_irq_liveness_stream().unwrap(),
                exit.try_clone().unwrap(),
            )
            .unwrap();
            client
                .local_irq_commit(metadata.token, metadata.epoch, metadata.initial_ticket)
                .unwrap();
            owner.committed().unwrap();
            let msi = EventFd::new(EFD_NONBLOCK).unwrap();
            owner.set_irq(1, &[&msi]).unwrap();
            let mut control = [0; 2];
            client.region_read(7, 0x6a, &mut control).unwrap();
            control[0] |= 1;
            client.region_write(7, 0x6a, &control).unwrap();
            owner.fence().unwrap();
            Self {
                client: Arc::new(Mutex::new(client)),
                device,
                owner,
                shared,
                page,
                msi,
                exit,
                _hook: hook,
                server: Some(server),
                socket,
            }
        }

        fn write(&self, offset: u64, value: u32) {
            self.owner
                .write(0, TREE + offset, &value.to_le_bytes())
                .unwrap()
                .unwrap();
        }

        fn word(&self, offset: u64) -> u32 {
            let mut bytes = [0; 4];
            self.owner
                .read(0, TREE + offset, &mut bytes)
                .unwrap()
                .unwrap();
            u32::from_le_bytes(bytes)
        }

        fn fold(&self, vector: u32) {
            self.shared.raise(vector);
            self.page.scan();
            self.owner.fence().unwrap();
        }

        fn take_msi(&self) -> u64 {
            match self.msi.read() {
                Ok(count) => count,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => 0,
                Err(error) => panic!("MSI read failed: {error}"),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.owner.stop();
            let _ = self.client.lock().unwrap().shutdown();
            if let Some(server) = self.server.take() {
                server.join().unwrap();
            }
            let _ = std::fs::remove_file(&self.socket);
        }
    }

    #[test]
    fn backend_fold_and_private_page_visible_before_msi() {
        let fixture = Fixture::new("visibility");
        fixture.write(LEAF_EN_SET, 1 << 4);
        fixture.write(TOP_EN_SET, 1);
        assert_eq!(fixture.take_msi(), 0);
        fixture.fold(4);
        assert_eq!(fixture.take_msi(), 1);
        assert_eq!(fixture.word(LEAF), 1 << 4);
        assert_eq!(fixture.word(TOP), 1);
        let mapping = fixture.owner.mapping();
        for offset in (0..PAGE_SIZE).step_by(4) {
            // SAFETY: the owner keeps a live aligned page; all writers use atomic
            // u32 stores, and the completed fence precedes this observation.
            let mapped = unsafe {
                &*mapping
                    .addr()
                    .add(offset as usize)
                    .cast::<std::sync::atomic::AtomicU32>()
            }
            .load(std::sync::atomic::Ordering::Acquire);
            assert_eq!(
                mapped,
                fixture.word(offset),
                "Published word at {offset:#x}"
            );
        }
        assert!(fixture.exit.read().is_err());
    }

    #[test]
    fn one_backend_drain_coalesces_but_distinct_drains_repend_on_ack() {
        let fixture = Fixture::new("grouping");
        fixture.write(LEAF_EN_SET, 1 << 4);
        fixture.write(TOP_EN_SET, 1);
        fixture.shared.raise(4);
        fixture.shared.raise(4);
        fixture.page.scan();
        fixture.owner.fence().unwrap();
        assert_eq!(fixture.take_msi(), 1);
        fixture.write(LEAF, 1 << 4);
        assert_eq!(fixture.word(LEAF), 0);
        assert_eq!(fixture.take_msi(), 0);
        fixture.fold(4);
        assert_eq!(fixture.take_msi(), 1);
        fixture.fold(4);
        assert_eq!(fixture.take_msi(), 0);
        fixture.write(LEAF, 1 << 4);
        assert_eq!(fixture.word(LEAF), 1 << 4);
        assert_eq!(fixture.take_msi(), 1);
        fixture.write(LEAF_EN_SET, 0);
        assert_eq!(fixture.take_msi(), 1, "Repeated zero SET still rearms");
    }

    #[test]
    fn device_reset_completes_local_epoch_before_next_guest_access() {
        let fixture = Fixture::new("reset");
        let old_generation = fixture.shared.producer_generation();
        fixture.write(LEAF_TRIGGER, 4);
        assert_eq!(fixture.take_msi(), 1);
        fixture.client.lock().unwrap().reset().unwrap();
        fixture
            .owner
            .rpc_complete(Ok::<(), std::io::Error>(()))
            .unwrap();
        assert_eq!(fixture.word(LEAF), 0);
        assert_eq!(fixture.word(LEAF_EN_SET), 0);
        assert_eq!(fixture.word(TOP_EN_SET), 0);
        fixture.shared.raise_in_generation(old_generation, 4);
        fixture.page.scan();
        fixture.owner.fence().unwrap();
        assert_eq!(
            fixture.word(LEAF),
            0,
            "Retired callback cannot enter the new epoch"
        );
        fixture.write(LEAF_EN_SET, 1 << 5);
        fixture.write(TOP_EN_SET, 1);
        fixture.fold(5);
        assert_eq!(fixture.word(LEAF), 1 << 5);
        assert_eq!(fixture.take_msi(), 1);
        assert!(fixture.exit.read().is_err());
    }

    #[test]
    fn unexpected_tree_rpc_closes_channel_and_stops_owner() {
        let fixture = Fixture::new("wrong-owner");
        assert!(
            fixture
                .client
                .lock()
                .unwrap()
                .region_write(0, TREE + LEAF_TRIGGER, &4u32.to_le_bytes())
                .is_err()
        );
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if fixture.exit.read().is_ok() {
                break;
            }
            assert!(Instant::now() < deadline, "Owner did not signal fatal stop");
            thread::sleep(Duration::from_millis(1));
        }
        assert!(
            fixture
                .owner
                .write(0, TREE + LEAF_TRIGGER, &5u32.to_le_bytes())
                .unwrap()
                .is_err()
        );
        assert_eq!(fixture.take_msi(), 0);
    }

    #[test]
    fn local_irq_progresses_while_unrelated_rpc_holds_client_and_device() {
        let fixture = Fixture::new("blocked-rpc");
        fixture.write(LEAF_EN_SET, 1 << 4);
        fixture.write(TOP_EN_SET, 1);
        let device_guard = fixture.device.lock().unwrap();
        let client = fixture.client.clone();
        let owner = fixture.owner.clone();
        let (ready, started) = std::sync::mpsc::channel();
        let rpc = thread::spawn(move || {
            let result = {
                let mut client = client.lock().unwrap();
                ready.send(()).unwrap();
                client.region_read(7, 0, &mut [0; 4])
            };
            owner.rpc_complete(result)
        });
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        fixture.fold(4);
        assert_eq!(fixture.take_msi(), 1);
        fixture.write(LEAF, 1 << 4);
        assert_eq!(fixture.word(LEAF), 0);
        assert!(fixture.client.try_lock().is_err(), "RPC still owns Client");
        drop(device_guard);
        rpc.join().unwrap().unwrap();
        assert!(fixture.exit.read().is_err());
    }
}

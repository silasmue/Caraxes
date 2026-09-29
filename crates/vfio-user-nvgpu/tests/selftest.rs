// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! End-to-end check of the E0 device through the vfio-user protocol: a server
//! thread with the real config dump and a synthetic ROM, and the crate's client
//! on the other end. No VM, no hardware.

use std::path::PathBuf;
use std::thread;

use vfio_user::Server;
use vfio_user_nvgpu::bar0::Bar0;
use vfio_user_nvgpu::board::Board;
use vfio_user_nvgpu::chip::{self, Chip};
use vfio_user_nvgpu::config::{parse_dump, ConfigSpace};
use vfio_user_nvgpu::device::{irqs, regions, Device};
use vfio_user_nvgpu::selftest;
use vfio_user_nvgpu::trace::Trace;

/// The RTX 2070 as `main` builds it by default: board data, chip, captured config.
fn rtx2070() -> (&'static dyn Chip, Board, ConfigSpace) {
    let dump = include_str!("../../../caraxes/reference/rtx2070/config-space.txt");
    let board = Board::load("rtx2070").unwrap();
    let chip = chip::by_name(&board.chip).unwrap();
    let cfg =
        ConfigSpace::from_dump(parse_dump(dump).unwrap(), board.bars, board.rom_bar_size).unwrap();
    (chip, board, cfg)
}

#[test]
fn client_sees_the_e0_device() {
    let (chip, board, cfg) = rtx2070();
    let expect_ids = cfg.ids();
    let class_revision = cfg.dword(0x08);
    let cap_ptr = cfg.byte(0x34);
    assert_eq!(
        (expect_ids.device_id(), class_revision, cap_ptr),
        (0x1F02_10DE, 0x0300_00A1, 0x60)
    );
    let mut rom = vec![0x55u8, 0xAA, 0x76];
    rom.resize(512, 0);
    let mut device = Device::new(cfg, Bar0::new(chip, rom, None), Trace::new(None).unwrap());

    let socket = PathBuf::from(format!(
        "{}/vfio-user-nvgpu-test-{}.sock",
        std::env::temp_dir().display(),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&socket);
    let server = Server::new(&socket, true, irqs(), regions(&board)).unwrap();
    let path = socket.clone();
    let handle = thread::spawn(move || {
        server.run(&mut device).unwrap();
        device.summary()
    });

    let expect = selftest::Expect {
        chip,
        board: &board,
        pci: expect_ids,
        class_revision,
        cap_ptr,
    };
    let passed = selftest::run(&path, &expect).unwrap();
    assert_eq!(passed.len(), 6, "{passed:?}");
    let summary = handle.join().unwrap();
    assert!(summary.contains("bar0:"), "{summary}");
    let _ = std::fs::remove_file(&socket);
}

#[test]
fn boot_model_is_visible_through_the_protocol() {
    use vfio_user_nvgpu::boot::BootModel;
    use vfio_user_nvgpu::rpc::StaticIdentity;
    let (chip, board, cfg) = rtx2070();
    let mut rom = vec![0x55u8, 0xAA, 0x76];
    rom.resize(512, 0);
    let id = StaticIdentity::new(chip.data(), &board, cfg.ids(), &board.name, 4 << 30);
    assert_eq!(id.gpu_name, "Leandro VFIO RTX 2070-4G");
    let bar0 = Bar0::new(chip, rom, None).with_boot_model(BootModel::new(
        chip,
        vfio_user_nvgpu::fb::FbLayout::plain(4 << 30),
        id,
    ));
    let mut device = Device::new(cfg, bar0, Trace::new(None).unwrap());
    let socket = PathBuf::from(format!(
        "{}/vfio-user-nvgpu-boot-{}.sock",
        std::env::temp_dir().display(),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&socket);
    let server = Server::new(&socket, true, irqs(), regions(&board)).unwrap();
    let path = socket.clone();
    let handle = thread::spawn(move || server.run(&mut device).unwrap());
    let mut c = vfio_user::Client::new(&path).unwrap();
    let mut b = [0u8; 4];
    c.region_read(0, 0x110100, &mut b).unwrap();
    assert_eq!(u32::from_le_bytes(b), 0x10, "GSP CPUCTL must read HALTED");
    c.region_read(0, 0x100CE0, &mut b).unwrap();
    assert_eq!(u32::from_le_bytes(b), 0x4A);
    c.region_read(0, 0x2174C, &mut b).unwrap();
    assert_eq!(u32::from_le_bytes(b), 1);
    c.shutdown().unwrap();
    handle.join().unwrap();
    let _ = std::fs::remove_file(&socket);
}

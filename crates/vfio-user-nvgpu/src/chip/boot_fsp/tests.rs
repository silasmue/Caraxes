// SPDX-License-Identifier: MIT
//! The FSP boot model driven the way the 610.57.04 kernel RM drives it on GB206
//! (docs/caraxes/gsp-boot-blackwell.md section 2), register by register, up to the
//! linked GSP message queues.

use super::*;
use crate::boot::BootModel;
use crate::fb::FbLayout;
use crate::msgq::tests::{cpu_send, queue_fixture, request};
use crate::msgq::{QueueMemory, TxHeader};

fn data() -> &'static FspBootData {
    crate::chip::gb206::boot_data()
}

/// A BAR0 register interface both the bare model and the generic boot model offer.
trait Regs {
    fn rd(&mut self, off: u64) -> u32;
    fn wr(&mut self, off: u64, v: u32, mem: &GuestMemory) -> Vec<String>;
}

impl Regs for FspBoot {
    fn rd(&mut self, off: u64) -> u32 {
        self.read32(off).expect("owned")
    }
    fn wr(&mut self, off: u64, v: u32, mem: &GuestMemory) -> Vec<String> {
        self.write32(off, v, mem).expect("owned").notes
    }
}

impl Regs for BootModel {
    fn rd(&mut self, off: u64) -> u32 {
        self.read32(off).expect("owned")
    }
    fn wr(&mut self, off: u64, v: u32, mem: &GuestMemory) -> Vec<String> {
        self.write32(off, v, mem).expect("owned")
    }
}

/// `mctpCreateTransportHeader(1, 1, 0, 0, 0)` and `mctpCreateNvdmHeader(t)`
/// (mctp_format.h:78-117).
fn nvdm_packet(nvdm: u8, payload: &[u8]) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&0xC000_0001u32.to_le_bytes());
    p.extend_from_slice(&(0x7E | (0x10DE << 8) | ((nvdm as u32) << 24)).to_le_bytes());
    p.extend_from_slice(payload);
    p
}

/// kfspSendPacket_GH100 then kfspReadPacket_GH100 (kern_fsp_gh100.c:621-725), with
/// the checks the CPU makes. Returns the response dwords and the notes of the send.
fn cpu_exchange(r: &mut impl Regs, packet: &[u8], mem: &GuestMemory) -> (Vec<u32>, Vec<String>) {
    let f = &data().fsp;
    // kfspCanSendPacket_GH100: both queues empty.
    assert_eq!(r.rd(f.queue_head), r.rd(f.queue_tail));
    assert_eq!(r.rd(f.msgq_head), r.rd(f.msgq_tail));
    r.wr(f.ememc, EMEMC_AINCW, mem);
    let start = r.rd(f.ememc);
    let words: Vec<u32> = packet
        .chunks(4)
        .map(|c| {
            let mut b = [0u8; 4];
            b[..c.len()].copy_from_slice(c);
            u32::from_le_bytes(b)
        })
        .collect();
    for &w in &words {
        r.wr(f.ememd, w, mem);
    }
    let end = r.rd(f.ememc);
    let pos = |c: u32| ((c >> 2) & 0x3F) + ((c >> 8) & 0xFF) * 64;
    assert_eq!(
        pos(end) - pos(start),
        words.len() as u32,
        "EMEMC auto-increment"
    );
    let padded = packet.len().div_ceil(4) * 4;
    r.wr(f.queue_tail, padded as u32 - 4, mem);
    let notes = r.wr(f.queue_head, 0, mem);
    // FSP consumed the packet and answered.
    assert_eq!(r.rd(f.queue_head), r.rd(f.queue_tail));
    let (head, tail) = (r.rd(f.msgq_head), r.rd(f.msgq_tail));
    assert_ne!(head, tail, "a response is available");
    let size = tail - head + 4;
    assert!((4..=1024).contains(&size));
    r.wr(f.ememc, EMEMC_AINCR, mem);
    let resp: Vec<u32> = (0..size / 4).map(|_| r.rd(f.ememd)).collect();
    assert_eq!(pos(r.rd(f.ememc)), size / 4);
    r.wr(f.msgq_tail, head, mem);
    r.wr(f.msgq_head, head, mem);
    (resp, notes)
}

/// kfspReadMessage/kfspProcessCommandResponse_GH100: single packet, NVDM
/// FSP_RESPONSE, errorCode 0.
fn assert_fsp_ok(resp: &[u32], nvdm: u8) {
    assert_eq!(resp.len(), 5);
    assert_eq!(resp[0] & 0xC000_0000, 0xC000_0000, "SOM and EOM");
    assert_eq!(resp[1] & 0x7F, 0x7E);
    assert_eq!((resp[1] >> 8) & 0xFFFF, 0x10DE);
    assert_eq!(resp[1] >> 24, NVDM_TYPE_FSP_RESPONSE as u32);
    assert_eq!((resp[3], resp[4]), (nvdm as u32, FSP_OK));
}

fn cot(fmc_params: u64) -> Vec<u8> {
    let mut c = vec![0u8; COT_SIZE];
    c[0..2].copy_from_slice(&2u16.to_le_bytes());
    c[2..4].copy_from_slice(&(COT_SIZE as u16).to_le_bytes());
    c[COT_GSP_FMC_SYSMEM..][..8].copy_from_slice(&0x7_0000_0000u64.to_le_bytes());
    // kfspPrepareBootCommands_GH100 on GB206: FRTS in vidmem 0x1BC0000 below the end,
    // 1 MiB; no sysmem FRTS (PF_KERNEL_ONLY).
    c[COT_FRTS_VIDMEM_OFFSET..][..8].copy_from_slice(&0x1BC_0000u64.to_le_bytes());
    c[COT_FRTS_VIDMEM_SIZE..][..4].copy_from_slice(&0x10_0000u32.to_le_bytes());
    c[COT_GSP_BOOT_ARGS..][..8].copy_from_slice(&fmc_params.to_le_bytes());
    c
}

fn put64(mem: &GuestMemory, pa: u64, v: u64) {
    assert!(mem.write(pa, &v.to_le_bytes()));
}

/// What kgspPopulateWprMeta_GH100 writes (kernel_gsp_gh100.c:304-397): sizes only.
fn cpu_wpr_meta(mem: &GuestMemory, pa: u64) {
    put64(mem, pa + v::WPR_META_MAGIC_OFF as u64, v::WPR_META_MAGIC);
    put64(mem, pa + 8, 1);
    put64(mem, pa + v::WPR_META_SIZE_OF_RADIX3_ELF as u64, 0x240_0000);
    put64(mem, pa + v::WPR_META_SIZE_OF_BOOTLOADER as u64, 0x2_8000);
    put64(mem, pa + v::WPR_META_NON_WPR_HEAP_SIZE as u64, 0x30_0000);
    put64(mem, pa + v::WPR_META_GSP_FW_HEAP_SIZE as u64, 0x620_0000);
    put64(mem, pa + v::WPR_META_FRTS_SIZE as u64, 0x10_0000);
    put64(mem, pa + v::WPR_META_VGA_WORKSPACE_SIZE as u64, 0x2_0000);
    assert!(mem.write(
        pa + v::WPR_META_PMU_RESERVED_SIZE as u64,
        &0x19A_0000u32.to_le_bytes()
    ));
}

/// kgspSetupGspFmcArgs_GH100 (kernel_gsp_gh100.c:419-497).
fn cpu_fmc_params(mem: &GuestMemory, pa: u64, meta: u64, libos: u64) {
    assert!(mem.write(pa + 8, &1u32.to_le_bytes()));
    assert!(mem.write(pa + v::FMC_GSP_RM_DESC_SIZE as u64, &256u32.to_le_bytes()));
    put64(mem, pa + v::FMC_GSP_RM_DESC_OFFSET as u64, meta);
    assert!(mem.write(pa + 36, &[1u8]));
    put64(mem, pa + v::FMC_BOOT_ARGS_OFFSET as u64, libos);
}

#[test]
fn acr_layout_from_the_gb206_requests() {
    let req = WprRequest {
        vga_workspace_size: 0x2_0000,
        pmu_reserved_size: 0x19A_0000,
        size_of_bootloader: 0x2_8000,
        size_of_radix3_elf: 0x240_0000,
        non_wpr_heap_size: 0x30_0000,
        gsp_fw_heap_size: 0x620_0000,
        frts_size: 0x10_0000,
    };
    let fb = 4u64 << 30;
    let l = AcrLayout::compute(fb, &req, 0x1BC_0000, 0x400_0000).unwrap();
    assert_eq!(l.fb_size, fb);
    assert_eq!(l.vga_workspace_offset, fb - 0x2_0000);
    assert_eq!(l.gsp_fw_wpr_end, fb - 0x1BC_0000);
    assert_eq!(l.frts_offset, l.gsp_fw_wpr_end - 0x10_0000);
    // Top down, non-overlapping, with the TU102 alignments.
    assert!(l.boot_bin_offset + 0x2_8000 <= l.frts_offset && l.boot_bin_offset % 0x1000 == 0);
    assert!(l.gsp_fw_offset + 0x240_0000 <= l.boot_bin_offset && l.gsp_fw_offset % 0x1_0000 == 0);
    assert!(l.gsp_fw_heap_offset + l.gsp_fw_heap_size <= l.gsp_fw_offset);
    assert!(l.gsp_fw_heap_size >= 0x620_0000 && l.gsp_fw_heap_offset % MIB == 0);
    assert_eq!(l.gsp_fw_wpr_start, l.gsp_fw_heap_offset - MIB);
    assert_eq!(
        l.non_wpr_heap_offset + l.non_wpr_heap_size,
        l.gsp_fw_wpr_start
    );
    assert_eq!(l.non_wpr_heap_size, 0x30_0000);
    // A 64 MiB FB cannot hold it.
    assert!(AcrLayout::compute(64 << 20, &req, 0x1BC_0000, 0).is_err());
}

#[test]
fn reset_state_and_emem_port() {
    let d = data();
    let mem = GuestMemory::new();
    let mut b = FspBoot::new(d, 4 << 30);
    assert_eq!(b.rd(0xAD_00BC), 0xFF, "FSP boot complete");
    assert_eq!(b.rd(0x82_074C), 1, "production part");
    assert_eq!(b.rd(0x11_83A4), 4096, "usable FB in MiB");
    assert_eq!(b.rd(0x88_A828), 0, "WPR2 down before the boot");
    let hwcfg2 = b.rd(0x11_00F4);
    assert_eq!(hwcfg2 & (1 << 13), 1 << 13, "lockdown held");
    assert_eq!(b.rd(0x11_1388) & (1 << 7), 0, "RISC-V not active");
    assert_eq!(b.read32(0x1234), None);
    // EMEM: AINCW write, AINCR read back, OFFS/BLK carry over the 64-dword block.
    b.wr(d.fsp.ememc, EMEMC_AINCW | (63 << 2), &mem);
    b.wr(d.fsp.ememd, 0x1111, &mem);
    b.wr(d.fsp.ememd, 0x2222, &mem);
    assert_eq!(b.rd(d.fsp.ememc), EMEMC_AINCW | (1 << 8) | (1 << 2));
    b.wr(d.fsp.ememc, EMEMC_AINCR | (63 << 2), &mem);
    assert_eq!((b.rd(d.fsp.ememd), b.rd(d.fsp.ememd)), (0x1111, 0x2222));
    // A doorbell before the COT has no libos arguments.
    let w = b.write32(d.gsp_queue_head, 0, &mem).unwrap();
    assert!(
        w.event.is_none() && w.notes[0].contains("before the COT"),
        "{w:?}"
    );
}

#[test]
fn caps_query_and_clock_boost_are_acknowledged() {
    let mem = GuestMemory::new();
    let mut b = FspBoot::new(data(), 4 << 30);
    // kfspCheckForClockBoostCapability_GB100: CAPS_QUERY, sub-message CLOCK_BOOST.
    let (resp, notes) = cpu_exchange(&mut b, &nvdm_packet(NVDM_TYPE_CAPS_QUERY, &[0x20]), &mem);
    assert_fsp_ok(&resp, NVDM_TYPE_CAPS_QUERY);
    assert!(
        notes[0].contains("CAPS_QUERY sub-message 0x20"),
        "{notes:?}"
    );
    let (resp, _) = cpu_exchange(&mut b, &nvdm_packet(NVDM_TYPE_CLOCK_BOOST, &[1]), &mem);
    assert_fsp_ok(&resp, NVDM_TYPE_CLOCK_BOOST);
    assert_eq!(b.messages, [NVDM_TYPE_CAPS_QUERY, NVDM_TYPE_CLOCK_BOOST]);
}

#[test]
fn multi_packet_messages_are_reassembled() {
    let mem = GuestMemory::new();
    let mut b = FspBoot::new(data(), 4 << 30);
    // A COT split over two packets (kfspSendMessage, kern_fsp.c:485-597): the first
    // without EOM, the second with only the transport header.
    let payload = cot(0xDEAD_0000);
    let mut first = nvdm_packet(NVDM_TYPE_COT, &payload[..500]);
    first[3] = 0x80; // SOM only
    let f = &data().fsp;
    b.wr(f.ememc, EMEMC_AINCW, &mem);
    for c in first.chunks(4) {
        b.wr(f.ememd, u32::from_le_bytes(c.try_into().unwrap()), &mem);
    }
    b.wr(f.queue_tail, first.len() as u32 - 4, &mem);
    assert!(b.wr(f.queue_head, 0, &mem).is_empty());
    assert_eq!(b.rd(f.msgq_head), b.rd(f.msgq_tail), "no response yet");
    let mut second = 0x4000_0001u32.to_le_bytes().to_vec(); // EOM, seq 0
    second.extend_from_slice(&payload[500..]);
    let (resp, notes) = cpu_exchange(&mut b, &second, &mem);
    assert_fsp_ok(&resp, NVDM_TYPE_COT);
    assert!(notes[0].contains("COT version 2 size 860"), "{notes:?}");
    assert!(
        notes.iter().any(|n| n.contains("0xdead0000 not mapped")),
        "{notes:?}"
    );
}

/// The whole GB206 bootstrap as kgspBootstrap_GH100 runs it, against the generic boot
/// model with the GB206 chip, up to the linked message queues. The guest side is the
/// 610 transport (message queue v0 through `msgq::tests`): RMARGS without the 615 magic,
/// which the device reads as message queue v0 (a 615 guest links v2 instead,
/// docs/caraxes/gsp-transport-615.md §8).
#[test]
fn gb206_bootstrap_reaches_the_message_queues() {
    let fx = crate::fixture::rtx5060ti();
    let fb = 4u64 << 30;
    let (mem, pages, base, path) = queue_fixture("gb206-fsp");
    // Guest structures above the queue buffer (0x81000 bytes at `base`).
    let (libos, rmargs, meta, fmc) = (
        base + 0x9_0000,
        base + 0x9_1000,
        base + 0x9_2000,
        base + 0x9_3000,
    );
    put64(&mem, libos + v::LIBOS_ARG_ID8, v::RMARGS_ID8);
    put64(&mem, libos + v::LIBOS_ARG_PA, rmargs);
    put64(&mem, libos + v::LIBOS_ARG_REGION_SIZE, 0x1000);
    put64(&mem, rmargs + crate::ver::v610::MQ_SHARED_MEM_PA, base);
    assert!(mem.write(rmargs + crate::ver::v610::MQ_PAGE_TABLE_ENTRY_COUNT, &129u32.to_le_bytes()));
    put64(&mem, rmargs + crate::ver::v610::MQ_CMD_QUEUE_OFFSET, 0x1000);
    put64(&mem, rmargs + crate::ver::v610::MQ_STAT_QUEUE_OFFSET, 0x4_1000);
    cpu_wpr_meta(&mem, meta);
    cpu_fmc_params(&mem, fmc, meta, libos);

    let mut b = BootModel::new(fx.chip, FbLayout::plain(fb), fx.identity(fb));
    let d = data();
    // 1. kfspWaitForSecureBoot_GB202, kgspIsWpr2Up_GB100, the fuses and the FB size.
    assert_eq!(b.rd(0xAD_00BC), 0xFF);
    assert_eq!(b.rd(0x88_A828) >> 4, 0);
    assert_eq!(b.rd(0x11_83A4), 4096);
    // 2. The clock-boost query, then the COT.
    let (resp, _) = cpu_exchange(&mut b, &nvdm_packet(NVDM_TYPE_CAPS_QUERY, &[0x20]), &mem);
    assert_fsp_ok(&resp, NVDM_TYPE_CAPS_QUERY);
    // gpuResetGspFmcErrorCode_GH100 writes NV_PBUS_SW_SCRATCH(0x37); no model owns it,
    // BAR0 latches it (bar0.rs), so it reads back the 0 written.
    assert!(b.write32(0x14DC, 0, &mem).is_none());
    let (resp, notes) = cpu_exchange(&mut b, &nvdm_packet(NVDM_TYPE_COT, &cot(fmc)), &mem);
    assert_fsp_ok(&resp, NVDM_TYPE_COT);
    let all = notes.join("\n");
    assert!(all.contains(&format!("libos args {libos:#x}")), "{all}");
    assert!(all.contains("GSP-RM running"), "{all}");
    assert!(all.contains("wpr meta: fbSize 0x100000000"), "{all}");
    assert_eq!(b.wpr_meta_pa(), Some(meta));
    let (non_wpr, frts) = {
        let w = b.wpr().expect("the WPR layout ACR would have written");
        assert_eq!(w.fb_size, fb);
        assert_eq!(w.frts_offset, fb - 0x1BC_0000 - 0x10_0000);
        assert!(w.non_wpr_heap_offset > 0 && w.non_wpr_heap_offset < w.gsp_fw_wpr_start);
        (w.non_wpr_heap_offset, w.frts_offset)
    };
    // WPR2 is up.
    assert_ne!(b.rd(0x88_A828) >> 4, 0);
    // 3. Target mask released (_kfspIsGspTargetMaskReleased).
    let hwcfg2 = b.rd(0x11_00F4);
    assert!(hwcfg2 != 0 && hwcfg2 & 0xFFFF_FF00 != 0xBADF_4100);
    // 4. kgspSendInitRpcs: SET_SYSTEM_INFO and SET_REGISTRY in the command queue,
    // then the doorbell NV_PGSP_QUEUE_HEAD(0).
    let qm = QueueMemory::new(&mem, &pages);
    let mut cmd = TxHeader::standard(0x4_0000);
    assert!(qm.write(0x1000, &cmd.encode()));
    cpu_send(
        &qm,
        &mut cmd,
        0,
        &request(crate::rpc::FN_GSP_SET_SYSTEM_INFO),
        &[0u8; 64],
    );
    cpu_send(
        &qm,
        &mut cmd,
        1,
        &request(crate::rpc::FN_SET_REGISTRY),
        &[0u8; 16],
    );
    let notes = b.wr(d.gsp_queue_head, 0, &mem).join("\n");
    assert!(
        notes.contains(&format!("doorbell 1: libos args PA {libos:#x} (mapped)")),
        "{notes}"
    );
    assert!(notes.contains("queues linked"), "{notes}");
    assert!(notes.contains("GSP_SET_SYSTEM_INFO"), "{notes}");
    // 5. Lockdown released, no FMC error, MAILBOX0 0 (_kgspLockdownReleasedOrFmcError).
    assert_eq!(b.rd(0x11_00F4) & (1 << 13), 0);
    assert_eq!(b.rd(0x11_0040), 0);
    assert_eq!(b.read32(0x14DC), None);
    b.wr(0x11_0080, 0x1234, &mem); // NV_PFALCON_FALCON_OS
                                   // 6. kflcnIsRiscvActive_GA102.
    assert_eq!(b.rd(0x11_1388) & (1 << 7), 1 << 7);
    // The status queue carries GSP_INIT_DONE for kgspWaitForRmInitDone.
    assert!(
        qm.read_u32(0x4_1000 + 16).unwrap() >= 1,
        "status write pointer moved"
    );
    // The static info tells the kernel where ACR put the WPR (fwWprLayoutOffset).
    let (info, _) =
        crate::rpc::build_static_info(fx.chip.data(), b.identity(), b.wpr(), b.layout());
    let at = |o: usize| u64::from_le_bytes(info[o..o + 8].try_into().unwrap());
    assert_eq!(at(v::SI_FW_WPR_NON_WPR_HEAP_OFFSET), non_wpr);
    assert_eq!(at(v::SI_FW_WPR_FRTS_OFFSET), frts);
    // A function-level reset brings the FSP back to its post-boot state.
    b.reset();
    assert_eq!(b.rd(0x88_A828), 0);
    assert_eq!(b.rd(0x11_00F4) & (1 << 13), 1 << 13);
    drop(qm);
    let _ = std::fs::remove_file(&path);
}

/// tmrSetCurrentTime_GH100 then _kgspIsHeartbeatTimedOut, as the GB206 kernel runs them
/// (timer_gh100.c:40-100, kernel_gsp.c:2264-2300), against the model's timer and the
/// device's PTIMER clock (both start with the model).
#[test]
fn heartbeats_are_fresh_in_the_kernels_time_base() {
    let mem = GuestMemory::new();
    let t = &data().timer;
    let ptimer = std::time::Instant::now();
    let mut b = FspBoot::new(data(), 4 << 30);
    // The kernel reads the secure timer hi-lo-hi; it must be below the OS time.
    let os_time_ns: u64 = 1_790_000_000_000_000_000;
    let sec = ((b.rd(t.sec_time_1) as u64) << 32) | b.rd(t.sec_time_0) as u64;
    assert!(sec < os_time_ns && sec < 10_000_000_000);
    let offset = os_time_ns - sec;
    b.wr(t.sys_offset_1, (offset >> 32) as u32, &mem);
    let notes = b.wr(t.sys_offset_0, offset as u32 | 1, &mem);
    assert!(notes[0].contains("system-timer offset"), "{notes:?}");
    std::thread::sleep(std::time::Duration::from_millis(20));
    for hb in t.heartbeat {
        let heartbeat = b.rd(hb);
        let current_ms = (ptimer.elapsed().as_nanos() as u64 / 1_000_000) as u32;
        let with_offset = heartbeat.wrapping_add((offset / 1_000_000) as u32);
        let diff = current_ms.wrapping_sub(with_offset);
        // 100 ms of scheduling jitter is allowed: the test runs beside builds and VMs, and the
        // kernel's heartbeat timeout is seconds (kernel_gsp.c:2264-2300).
        assert!(diff <= 100 || diff >= u32::MAX - 100, "diff {diff} ms");
    }
    assert_eq!(b.rd(0x11_8DF8), (offset >> 32) as u32);
}

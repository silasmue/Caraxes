// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The display answers against the ABI rules of display-adapter.md section 7.

use super::*;

fn display() -> Display {
    let board = crate::board::Board::load("rtx2070").unwrap();
    Display::from_setup(&Setup {
        enabled: true,
        chip: "tu106".into(),
        board: board.display,
    })
    .unwrap()
}

const DP: u32 = 0x100;

fn aux(disp: &mut Display, cmd: u32, addr: u32, size: u32, addr_only: bool, data: &[u8]) -> Answer {
    let mut p = vec![0u8; d::AUX_SIZE];
    put32(&mut p, d::DISPLAY_ID, DP);
    p[d::AUX_ADDR_ONLY] = addr_only as u8;
    put32(&mut p, d::AUX_CMD, cmd);
    put32(&mut p, d::AUX_ADDR, addr);
    p[d::AUX_DATA..d::AUX_DATA + data.len()].copy_from_slice(data);
    put32(&mut p, d::AUX_SIZE_FIELD, size);
    disp.control(d::CMD_DP_AUXCH_CTRL, &p).unwrap()
}

#[test]
fn disabled_display_reads_the_fuse_as_absent_and_answers_nothing() {
    let mut disp = Display::disabled();
    assert_eq!(disp.read32(NV_FUSE_STATUS_OPT_DISPLAY), Some(1));
    assert_eq!(disp.read32(NV_PDISP_VGA_WORKSPACE_BASE), None);
    assert!(disp
        .control(d::CMD_INTERNAL_DISPLAY_GET_IP_VERSION, &[0; 4])
        .is_none());
}

#[test]
fn phase_a_answers() {
    let mut disp = display();
    assert_eq!(disp.read32(NV_FUSE_STATUS_OPT_DISPLAY), Some(0));
    assert_eq!(disp.read32(NV_PDISP_VGA_WORKSPACE_BASE), Some(0));
    let a = disp
        .control(d::CMD_INTERNAL_DISPLAY_GET_IP_VERSION, &[0; 4])
        .unwrap();
    assert_eq!((a.status, get32(&a.params, 0)), (NV_OK, 0x0400_0000));
    let a = disp
        .control(d::CMD_INTERNAL_DISPLAY_GET_STATIC_INFO, &[0; 32])
        .unwrap();
    assert_eq!(get32(&a.params, d::STATIC_INFO_NUM_HEADS), 4);
    assert_eq!(get32(&a.params, d::STATIC_INFO_WINDOW_PRESENT_MASK), 0xFF);
    assert_eq!(get32(&a.params, d::STATIC_INFO_FE_HW_SYS_CAP), 0xF);
    assert_eq!(get32(&a.params, d::STATIC_INFO_I2C_PORT), 16);
    assert_eq!(get32(&a.params, d::STATIC_INFO_NUM_DISP_CHANNELS), 81);
    let mut p = vec![0u8; 24];
    p[0..8].copy_from_slice(&0xF75D_0000u64.to_le_bytes());
    p[8..16].copy_from_slice(&0x1_0000u64.to_le_bytes());
    disp.control(d::CMD_INTERNAL_DISPLAY_WRITE_INST_MEM, &p)
        .unwrap();
    assert_eq!(disp.inst_mem, Some((0xF75D_0000, 0x1_0000, 0)));
    // The caps page: 4 heads and 4 SORs, 8 windows.
    assert_eq!(disp.read32(NV_PDISP_FE_SW), Some(0xF0F));
    assert_eq!(disp.read32(NV_PDISP_FE_SW + 4), Some(0xFF));
    assert_eq!(disp.read32(NV_PDISP_FE_SW + 0x8A0), Some(0));
}

#[test]
fn link_answers_agree_on_one_profile() {
    let mut disp = display();
    let mut p = vec![0u8; d::DP_CAPS_SIZE];
    put32(&mut p, 4, 0);
    let caps = disp.control(d::CMD_DP_GET_CAPS, &p).unwrap();
    assert_eq!(get32(&caps.params, d::DP_CAPS_MAX_LINK_RATE), 4);
    assert_eq!(caps.params[d::DP_CAPS_DSC_SUPPORTED], 1);
    let mut p = vec![0u8; 16];
    put32(&mut p, d::DISPLAY_ID, DP);
    let dfp = disp.control(d::CMD_DFP_GET_INFO, &p).unwrap();
    let flags = get32(&dfp.params, d::DFP_INFO_FLAGS);
    assert_eq!(flags & 7, d::DFP_FLAGS_SIGNAL_DISPLAYPORT);
    assert_eq!((flags >> 17) & 7, 4, "DP_LINK_BW 8.1 Gbit/s");
    // The DPCD says HBR3 x4 as well (extended caps at 0x2200 on DPCD 1.4).
    let a = aux(&mut disp, 0x9, 0x2200, 15, false, &[]);
    assert_eq!(a.status, NV_OK);
    assert_eq!(get32(&a.params, d::AUX_SIZE_FIELD), 16);
    let dpcd = &a.params[d::AUX_DATA..d::AUX_DATA + 16];
    assert_eq!((dpcd[0], dpcd[1], dpcd[2] & 0x1F), (0x14, 0x1E, 4));
    // Training through DP_CTRL: HBR3 (0x1E) x4, then the link config says the same.
    let mut p = vec![0u8; 28];
    put32(&mut p, d::DISPLAY_ID, DP);
    put32(&mut p, d::DP_CTRL_CMD, 0x83);
    put32(&mut p, d::DP_CTRL_DATA, (0x1E << 8) | 4);
    assert_eq!(disp.control(d::CMD_DP_CTRL, &p).unwrap().status, NV_OK);
    let mut p = vec![0u8; 24];
    put32(&mut p, d::DISPLAY_ID, DP);
    let lc = disp.control(d::CMD_DP_GET_LINK_CONFIG, &p).unwrap();
    assert_eq!(get32(&lc.params, d::LINK_CONFIG_LANE_COUNT), 4);
    assert_eq!(get32(&lc.params, d::LINK_CONFIG_LINK_BW), 0x1E);
    assert_eq!(get32(&lc.params, d::LINK_CONFIG_DP2_LINK_BW), 0);
    // And the DPCD lane status reflects the trained link.
    let a = aux(&mut disp, 0x9, 0x202, 1, false, &[]);
    assert_eq!(&a.params[d::AUX_DATA..d::AUX_DATA + 2], &[0x77, 0x77]);
}

#[test]
fn aux_encoding() {
    let mut disp = display();
    // 1 byte: size 0 in, 1 out.
    let a = aux(&mut disp, 0x9, 0x000, 0, false, &[]);
    assert_eq!((a.status, get32(&a.params, d::AUX_SIZE_FIELD)), (NV_OK, 1));
    assert_eq!(get32(&a.params, d::AUX_REPLY_TYPE), 0);
    // The DisplayID probe at 0x52 is an I2C NACK with NV_OK.
    let a = aux(&mut disp, 0x4, 0x52, 0, false, &[0]);
    assert_eq!(a.status, NV_OK);
    assert_eq!(get32(&a.params, d::AUX_REPLY_TYPE), 4);
    // Address-only I2C write to 0x50: size 0 with bAddrOnly means no data.
    let a = aux(&mut disp, 0x4, 0x50, 0, true, &[]);
    assert_eq!(
        (
            a.status,
            get32(&a.params, d::AUX_REPLY_TYPE),
            get32(&a.params, d::AUX_SIZE_FIELD)
        ),
        (NV_OK, 0, 0)
    );
    // Offset 0 then 8 reads of 16 bytes (size 15): the EDID.
    aux(&mut disp, 0x4, 0x50, 0, false, &[0]);
    let mut edid = Vec::new();
    for i in 0..8 {
        let cmd = if i < 7 { 0x5 } else { 0x1 };
        let a = aux(&mut disp, cmd, 0x50, 15, false, &[]);
        assert_eq!(get32(&a.params, d::AUX_SIZE_FIELD), 16);
        edid.extend_from_slice(&a.params[d::AUX_DATA..d::AUX_DATA + 16]);
    }
    assert_eq!(edid, &edid::monitor_edid()[..128]);
    // Invalid length (native 17 bytes) is a control error, not a reply type.
    let a = aux(&mut disp, 0x9, 0x000, 16, false, &[]);
    assert_eq!(a.status, NV_ERR_INVALID_ARGUMENT);
    // An unknown display id is an error too.
    let mut p = vec![0u8; d::AUX_SIZE];
    put32(&mut p, d::DISPLAY_ID, 0x200);
    put32(&mut p, d::AUX_CMD, 0x9);
    assert_eq!(
        disp.control(d::CMD_DP_AUXCH_CTRL, &p).unwrap().status,
        NV_ERR_INVALID_ARGUMENT
    );
    assert!(disp.aux_transcript.len() >= 12);
}

#[test]
fn edid_ping_pong() {
    let mut disp = display();
    let get = |disp: &mut Display, flags: u32| {
        let mut p = vec![0u8; d::GET_EDID_V2_SIZE];
        put32(&mut p, d::DISPLAY_ID, DP);
        put32(&mut p, d::GET_EDID_FLAGS, flags);
        let a = disp.control(d::CMD_SPECIFIC_GET_EDID_V2, &p).unwrap();
        let n = get32(&a.params, d::GET_EDID_BUFFER_SIZE) as usize;
        a.params[d::GET_EDID_BUFFER..d::GET_EDID_BUFFER + n].to_vec()
    };
    let set = |disp: &mut Display, bytes: &[u8]| {
        let mut p = vec![0u8; d::SET_EDID_V2_SIZE];
        put32(&mut p, d::DISPLAY_ID, DP);
        put32(&mut p, d::SET_EDID_BUFFER_SIZE, bytes.len() as u32);
        p[d::SET_EDID_BUFFER..d::SET_EDID_BUFFER + bytes.len()].copy_from_slice(bytes);
        disp.control(d::CMD_SPECIFIC_SET_EDID_V2, &p).unwrap()
    };
    let sink = edid::monitor_edid();
    assert_eq!(get(&mut disp, 0), sink);
    // The override wins; size 0 clears it.
    let mut other = sink.clone();
    other[17] ^= 1;
    set(&mut disp, &other);
    assert_eq!(get(&mut disp, 0), other);
    assert_eq!(get(&mut disp, d::GET_EDID_FLAGS_READ_MODE_RAW), sink);
    assert_eq!(set(&mut disp, &[]).status, NV_OK);
    assert_eq!(get(&mut disp, 0), sink);
    assert_eq!(get(&mut disp, d::GET_EDID_FLAGS_COPY_CACHE_YES), sink);
}

#[test]
fn core_channel_update_arms_the_assembly_state() {
    let mut disp = display();
    let mut mem = GuestMemory::new();
    // A pushbuffer in a memfd-backed guest page.
    let f = tempfile_4k();
    mem.map(0x10_0000, 0x1000, 0, &f).unwrap();
    // WINDOW_SET_CONTROL(1) = owner 0 (method 0x1080), then UPDATE (0x200).
    let words = [1u32 << 18 | 0x1080, 0, 1 << 18 | 0x200, 0];
    for (i, w) in words.iter().enumerate() {
        assert!(mem.write(0x10_0000 + 4 * i as u64, &w.to_le_bytes()));
    }
    let mut p = vec![0u8; d::PUSHBUFFER_SIZE];
    put32(&mut p, d::PUSHBUFFER_ADDRESS_SPACE, 1);
    p[8..16].copy_from_slice(&0x10_0000u64.to_le_bytes());
    p[16..24].copy_from_slice(&0xFFFu64.to_le_bytes());
    put32(&mut p, d::PUSHBUFFER_HCLASS, d::NVC57D_CORE_CHANNEL_DMA);
    p[d::PUSHBUFFER_VALID] = 1;
    disp.control(d::CMD_INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER, &p);
    disp.alloc(d::NVC57D_CORE_CHANNEL_DMA, &[0; 40]).unwrap();
    disp.core_assembly.insert(0x1080, 0x3);
    let notes = disp.write32(CORE_USERD, 16, &mem).unwrap();
    assert!(notes[0].contains("2 method(s), 1 UPDATE(s)"), "{notes:?}");
    assert_eq!(disp.read32(CORE_USERD + 4), Some(16));
    assert_eq!(disp.read32(CORE_ARMED + 0x1080), Some(0));
    // Cursor Free is non-zero.
    assert_ne!(disp.read32(CURSOR_USERD + 8), Some(0));
}

fn tempfile_4k() -> std::fs::File {
    let path = std::env::temp_dir().join(format!("caraxes-disp-{}", std::process::id()));
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .unwrap();
    f.set_len(0x1000).unwrap();
    let _ = std::fs::remove_file(&path);
    f
}

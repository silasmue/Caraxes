// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
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
fn caps_page_reports_ihub_resources_without_unmodelled_features() {
    let mut disp = display();
    let ihub = disp.read32(NV_PDISP_FE_SW + 0x10).unwrap();
    // clc573.h: entries[15:0], entry width[17:16], request size[31:30].
    // The 2026-09-27 TU106 capture is 0x81FA1400; only its resource geometry
    // belongs in this model, whose sole IHUB feature is SUPPORT_PLANAR[19].
    let entries = ihub & 0xFFFF;
    let entry_bytes = 32u32 << ((ihub >> 16) & 3);
    let request_bytes = 32u32 << ((ihub >> 30) & 3);
    assert_eq!(entries * entry_bytes, 640 * 1024, "IHUB capacity in bytes");
    assert_eq!(entry_bytes, 128);
    assert_eq!(request_bytes, entry_bytes);
    let resource_fields = 0xC003_FFFF;
    assert_eq!(ihub & !resource_fields, 1 << 19);
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
    // The DP library leaves the sink in D3 after detection (0x600 = 2, run b1); DP_CTRL
    // wakes it before training, as GSP does.
    aux(&mut disp, 0x8, 0x600, 0, false, &[0x02]);
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
    for initial_offset in [0, 0x40] {
        core_channel_update_from_offset(initial_offset);
    }
}

fn core_channel_update_from_offset(initial_offset: u32) {
    let mut disp = display();
    let mut mem = GuestMemory::new();
    // A pushbuffer in a memfd-backed guest page.
    let f = tempfile_4k();
    mem.map(0x10_0000, 0x1000, 0, &f).unwrap();
    // Data before the requested initial GET is stale and must never be replayed.
    if initial_offset != 0 {
        assert!(mem.write(0x10_0000, &(1u32 << 18 | 0x1084).to_le_bytes()));
        assert!(mem.write(0x10_0004, &0xABCDu32.to_le_bytes()));
    }
    // WINDOW_SET_CONTROL(1) = owner 0 (method 0x1080), then UPDATE (0x200).
    let words = [1u32 << 18 | 0x1080, 0, 1 << 18 | 0x200, 0];
    for (i, w) in words.iter().enumerate() {
        assert!(mem.write(
            0x10_0000 + initial_offset as u64 + 4 * i as u64,
            &w.to_le_bytes()
        ));
    }
    let mut p = vec![0u8; d::PUSHBUFFER_SIZE];
    put32(&mut p, d::PUSHBUFFER_ADDRESS_SPACE, 1);
    p[8..16].copy_from_slice(&0x10_0000u64.to_le_bytes());
    p[16..24].copy_from_slice(&0xFFFu64.to_le_bytes());
    put32(&mut p, d::PUSHBUFFER_HCLASS, d::NVC57D_CORE_CHANNEL_DMA);
    p[d::PUSHBUFFER_VALID] = 1;
    disp.control(d::CMD_INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER, &p);
    let mut alloc = vec![0; d::CHANNELDMA_ALLOC_SIZE];
    put32(&mut alloc, d::CHANNELDMA_ALLOC_OFFSET, initial_offset);
    disp.alloc(d::NVC57D_CORE_CHANNEL_DMA, &alloc).unwrap();
    assert_eq!(disp.read32(CORE_USERD), Some(initial_offset));
    assert_eq!(disp.read32(CORE_USERD + 4), Some(initial_offset));
    disp.core_assembly.insert(0x1080, 0x3);
    let mut fb = crate::fb::FakeFb::new(1 << 20);
    let end = initial_offset + 16;
    let notes = disp.write32(CORE_USERD, end, &mem, &mut fb).unwrap();
    assert!(notes[0].contains("2 method(s), 1 UPDATE(s)"), "{notes:?}");
    assert_eq!(disp.read32(CORE_USERD + 4), Some(end));
    assert_eq!(disp.read32(CORE_ARMED + 0x1080), Some(0));
    assert!(!disp.core_assembly.contains_key(&0x1084));
    // Cursor Free is non-zero.
    assert_ne!(disp.read32(CURSOR_USERD + 8), Some(0));
}

#[test]
fn dma_channel_initial_offset_does_not_apply_to_cursor_pio() {
    let mut disp = display();
    for class in [
        d::NVC57D_CORE_CHANNEL_DMA,
        d::NVC57E_WINDOW_CHANNEL_DMA,
        d::NVC57B_WINDOW_IMM_CHANNEL_DMA,
    ] {
        let mut params = vec![0; d::CHANNELDMA_ALLOC_SIZE];
        put32(&mut params, d::CHANNELDMA_ALLOC_OFFSET, 0x80);
        disp.alloc(class, &params).unwrap();
        let base = userd_base(class, 0).unwrap();
        assert_eq!(disp.read32(base), Some(0x80), "class {class:#x}");
        assert_eq!(disp.read32(base + 4), Some(0x80), "class {class:#x}");
    }
    let mut params = vec![0; d::CHANNELPIO_ALLOC_SIZE];
    // Upper half of the PIO pControl pointer, not a DMA offset.
    put32(&mut params, 12, 0xFFFF_8000);
    disp.alloc(d::NVC57A_CURSOR_IMM_CHANNEL_PIO, &params)
        .unwrap();
    assert_eq!(disp.read32(CURSOR_USERD), Some(0));
    assert_eq!(disp.read32(CURSOR_USERD + 4), Some(0));
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

/// Phase D: a hotplug event the guest registered gets one POST_EVENT per connector
/// command, and the EDID change reaches the sink and the plug mask.
#[test]
fn connector_command_posts_hotplug_events() {
    let mut disp = display();
    let mut p = vec![0u8; 24];
    put32(&mut p, 12, NV2080_NOTIFIERS_HOTPLUG);
    disp.record_event(0xc1d0_0008, 0x1_0003, 0x7E, &p);
    disp.record_event(0xc1d0_0008, 0x1_0003, 0x7E, &p);
    put32(&mut p, 12, 7);
    disp.record_event(0xc1d0_0008, 0x1_0004, 0x7E, &p);
    assert_eq!(disp.hotplug_events, vec![(0xc1d0_0008, 0x1_0003)]);
    let ev = disp.connector_command("edid-alt\n");
    assert_eq!(ev.len(), 1);
    assert_eq!(get32(&ev[0], 0), 0xc1d0_0008);
    assert_eq!(get32(&ev[0], 4), 0x1_0003);
    assert_eq!(get32(&ev[0], 8), NV2080_NOTIFIERS_HOTPLUG);
    assert_eq!(get32(&ev[0], 12), 0x100);
    let alt = edid::monitor_edid_alt();
    assert_eq!(disp.connectors[0].sink.edid(), alt.as_slice());
    assert_ne!(alt, edid::monitor_edid());
    assert_eq!(alt[..128].iter().fold(0u8, |a, b| a.wrapping_add(*b)), 0);
    assert!(disp.connectors[0].pending_plug && disp.connectors[0].sink.is_plugged());
    assert!(disp.connector_command("bogus").is_empty());
}

// ---- Category D display controls (docs/caraxes/gsp-plan/d-device-model.md 6.5, roadmap R1)

/// The id and generated layout size of a D control of this build's driver version.
fn d_id(name: &str) -> Option<(u32, usize)> {
    crate::ver::Abi::active()
        .d_rules
        .iter()
        .find(|r| r.name == name)
        .map(|r| (r.id, r.size))
}

fn hdcp(disp: &mut Display, did: u32, sub: &str, fill: u8) -> (Answer, Vec<u8>) {
    const P: &str = "NV0073_CTRL_SPECIFIC_HDCP_CTRL_PARAMS";
    let abi = crate::ver::Abi::active();
    let (id, size) = d_id("NV0073_CTRL_CMD_SPECIFIC_HDCP_CTRL").unwrap();
    assert_eq!(abi.layout(P).unwrap().size, size);
    let mut p = vec![fill; size];
    put_field(&mut p, field(P, "subDeviceInstance"), 0);
    put_field(&mut p, field(P, "displayId"), did as u64);
    let c = abi
        .d_enum(&format!("NV0073_CTRL_SPECIFIC_HDCP_CTRL_CMD_{sub}"))
        .map_or(0x77, u64::from);
    put_field(&mut p, field(P, "cmd"), c);
    put_field(&mut p, field(P, "err"), 0xEE);
    (disp.control(id, &p).unwrap(), p)
}

/// SPECIFIC_HDCP_CTRL per sub-command: the unplug's DISABLE_AUTHENTICATION (every unplug
/// sends it, caraxes/runs/display-cd-2026-09-24/d11-unplug/device-trace.txt:16) succeeds
/// with err 0, a cipher request succeeds with err UNSUCCESSFUL, the queries return no data,
/// an unknown cmd or display is an invalid argument; nothing is `unanswered`.
#[test]
fn hdcp_ctrl_is_answered_per_sub_command() {
    const P: &str = "NV0073_CTRL_SPECIFIC_HDCP_CTRL_PARAMS";
    let mut disp = display();
    let err = field(P, "err");
    for sub in ["DISABLE_AUTHENTICATION", "ABORT_AUTHENTICATION", "NULL"] {
        let (a, req) = hdcp(&mut disp, DP, sub, 0);
        assert_eq!((a.status, a.label), (NV_OK, HDCP), "{sub}");
        assert_eq!(get_field(&a.params, err), 0, "{sub}");
        assert_eq!(a.params.len(), req.len());
    }
    for sub in ["RENEGOTIATE", "VALIDATE_LINK", "READ_LINK_STATUS"] {
        let (a, _) = hdcp(&mut disp, DP, sub, 0);
        assert_eq!(a.status, NV_OK, "{sub}");
        assert_eq!(get_field(&a.params, err) & 1, 1, "{sub}: err UNSUCCESSFUL");
    }
    for sub in ["GET_ALL_FLAGS", "READ_TOPOLOGY", "QUERY_HEAD_CONFIG"] {
        let (a, req) = hdcp(&mut disp, DP, sub, 0xFF);
        assert_eq!(a.status, NV_OK, "{sub}");
        for f in ["displayId", "cmd"] {
            assert_eq!(
                get_field(&a.params, field(P, f)),
                get_field(&req, field(P, f))
            );
        }
        for f in [
            "err",
            "flags",
            "isHdcpCapable",
            "isHdcpAuthOn",
            "numBksvs",
            "bCaps[0]",
            "cS",
        ] {
            assert_eq!(get_field(&a.params, field(P, f)), 0, "{sub}: {f}");
        }
    }
    let (a, _) = hdcp(&mut disp, DP, "NO_SUCH_COMMAND", 0);
    assert_eq!(a.status, NV_ERR_INVALID_ARGUMENT);
    let (a, _) = hdcp(&mut disp, 0x8000, "DISABLE_AUTHENTICATION", 0);
    assert_eq!(a.status, NV_ERR_INVALID_ARGUMENT);
}

/// DP_CONFIGURE_FEC turns FEC on for a trained link of a profile with FEC and
/// DP_GET_LINK_CONFIG reports it; untrained it is an invalid argument; a new training
/// clears it.
#[test]
fn configure_fec_feeds_the_link_config() {
    const P: &str = "NV0073_CTRL_CMD_DP_CONFIGURE_FEC_PARAMS";
    let mut disp = display();
    assert!(
        disp.chip().link.fec,
        "the TU106 link profile has FEC (display.toml [link])"
    );
    let (id, size) = d_id("NV0073_CTRL_CMD_DP_CONFIGURE_FEC").unwrap();
    let mut p = vec![0u8; size];
    put_field(&mut p, field(P, "displayId"), DP as u64);
    put_field(&mut p, field(P, "bEnableFec"), 1);
    assert_eq!(
        disp.control(id, &p).unwrap().status,
        NV_ERR_INVALID_ARGUMENT
    );
    disp.link = Some((0x14, 4));
    let a = disp.control(id, &p).unwrap();
    assert_eq!((a.status, a.label), (NV_OK, SINK));
    let mut lc = vec![0u8; d::LINK_CONFIG_FEC_ENABLED + 4];
    put32(&mut lc, d::DISPLAY_ID, DP);
    let a = disp.control(d::CMD_DP_GET_LINK_CONFIG, &lc).unwrap();
    assert_eq!(get32(&a.params, d::LINK_CONFIG_FEC_ENABLED), 1);
    put_field(&mut p, field(P, "bEnableFec"), 0);
    assert_eq!(disp.control(id, &p).unwrap().status, NV_OK);
    assert!(!disp.fec);
    put_field(&mut p, field(P, "displayId"), 0x8000);
    assert_eq!(
        disp.control(id, &p).unwrap().status,
        NV_ERR_INVALID_ARGUMENT
    );
}

/// GET_RG_STATUS: no merge pair, no scan or flip lock, never stalled; a head the chip does
/// not have is an invalid argument.
#[test]
fn rg_status_reports_unlocked_heads() {
    const P: &str = "NV5070_CTRL_CMD_GET_RG_STATUS_PARAMS";
    let mut disp = display();
    let (id, size) = d_id("NV5070_CTRL_CMD_GET_RG_STATUS").unwrap();
    let mut p = vec![0xFFu8; size];
    put_field(&mut p, field(P, "head"), 1);
    let a = disp.control(id, &p).unwrap();
    assert_eq!((a.status, a.label), (NV_OK, ENGINE));
    for f in ["scanLocked", "flipLocked", "rgStalled"] {
        assert_eq!(get_field(&a.params, field(P, f)), 0, "{f}");
    }
    put_field(&mut p, field(P, "head"), disp.chip().heads as u64);
    assert_eq!(
        disp.control(id, &p).unwrap().status,
        NV_ERR_INVALID_ARGUMENT
    );
}

/// DP_SET_WAR_FLAGS (615 only, every plug and unplug, dp_connectorimpl.cpp:8048, :8475) is
/// acknowledged as the D rule table says and recorded per connector
/// (docs/caraxes/display-615.md §3.2); the 610 ABI has no such control.
#[test]
fn dp_set_war_flags_is_recorded_per_connector() {
    use crate::ver::{GuestAbi, TestGuest};
    for g in [GuestAbi::V610, GuestAbi::V615] {
        let _g = TestGuest::set(g);
        war_flags_case(g);
    }
}

fn war_flags_case(g: crate::ver::GuestAbi) {
    let mut disp = display();
    let Some(id) = crate::ver::layouts().display.cmd_dp_set_war_flags else {
        assert_eq!(g, crate::ver::GuestAbi::V610);
        assert!(d_id("NV0073_CTRL_CMD_DP_SET_WAR_FLAGS").is_none());
        return;
    };
    assert_eq!(g, crate::ver::GuestAbi::V615);
    let (rid, size) = d_id("NV0073_CTRL_CMD_DP_SET_WAR_FLAGS").unwrap();
    assert_eq!((rid, size), (id, 16));
    let war = |did: u32, war: u32, on: bool| {
        let mut p = vec![0u8; size];
        put32(&mut p, d::WAR_FLAGS_DISPLAY_ID, did);
        put32(&mut p, d::WAR_FLAGS_WAR_ID, war);
        p[d::WAR_FLAGS_ENABLE] = on as u8;
        p
    };
    // The DP library's call: clear SW_AUTO_READ on the connector: the rule table's ACK
    // (request echoed), the flag recorded, the WAR named in the note.
    let ack = crate::dmodel::ACKNOWLEDGED;
    let p = war(DP, d::DP_WAR_SW_AUTO_READ_ENABLE, false);
    let a = disp.control(id, &p).unwrap();
    assert_eq!((a.status, a.label, &a.params), (NV_OK, ack, &p));
    assert!(!disp.connector(DP).unwrap().sw_auto_read_war);
    let a = disp
        .control(id, &war(DP, d::DP_WAR_SW_AUTO_READ_ENABLE, true))
        .unwrap();
    assert_eq!((a.status, a.label), (NV_OK, ack));
    assert!(disp.connector(DP).unwrap().sw_auto_read_war);
    assert!(
        a.note.contains("SW_AUTO_READ_ENABLE 1 on 0x100 (was 0"),
        "{}",
        a.note
    );
    // The header's NV_ERR_INVALID_ARGUMENT cases are named, not recorded (the row is ACK).
    let a = disp
        .control(id, &war(0x800, d::DP_WAR_SW_AUTO_READ_ENABLE, false))
        .unwrap();
    assert_eq!(a.status, NV_OK);
    assert!(a.note.contains("0x800 is not a connector"), "{}", a.note);
    let a = disp.control(id, &war(DP, 2, false)).unwrap();
    assert!(a.note.contains("unknown warId 2"), "{}", a.note);
    assert!(disp.connector(DP).unwrap().sw_auto_read_war);
}

/// SYSTEM_GET_ACTIVE before any modeset: no firmware-lit head, so no active display and, from
/// 615 on, no stale head either: the output flag GOP_STALE_HEAD_NEEDS_CLEANUP is cleared and
/// `staleSorIndex` is 0, whatever the caller left in them; the input flag CLIENT is kept.
#[test]
fn get_active_reports_no_head_and_no_stale_head() {
    use crate::ver::{GuestAbi, TestGuest};
    for g in [GuestAbi::V610, GuestAbi::V615] {
        let _g = TestGuest::set(g);
        let l = &crate::ver::layouts().display;
        let mut disp = display();
        let mut p = vec![0xAAu8; l.get_active_size];
        put32(&mut p, d::GET_ACTIVE_HEAD, 1);
        put32(&mut p, d::GET_ACTIVE_FLAGS, 0x3);
        let a = disp.control(d::CMD_SYSTEM_GET_ACTIVE, &p).unwrap();
        assert_eq!(a.status, NV_OK);
        assert_eq!(a.params.len(), l.get_active_size);
        assert_eq!(get32(&a.params, d::GET_ACTIVE_DISPLAY_ID), 0);
        assert_eq!(
            get32(&a.params, d::GET_ACTIVE_FLAGS),
            0x3 & !l.get_active_flags_gop_stale_head
        );
        match l.get_active_stale_sor_index {
            Some(o) => {
                assert_eq!(g, GuestAbi::V615);
                assert_eq!((o, l.get_active_size, get32(&a.params, o)), (16, 20, 0));
                assert_eq!(get32(&a.params, d::GET_ACTIVE_FLAGS), 0x1);
            }
            None => assert_eq!((g, l.get_active_size), (GuestAbi::V610, 16)),
        }
    }
}

/// SYSTEM_GET_ACTIVE after a client's modeset (the Windows KMD asks right after it, the DP
/// library's "is a head attached to this displayId"): the head a SOR drives returns that
/// SOR's connector, with the CLIENT flag or without; other heads return 0; a head beyond the
/// chip's heads is the header's INVALID_ARGUMENT; an idle head is 0 again.
#[test]
fn get_active_reports_the_display_a_client_lit() {
    let mut disp = display();
    let mut fb = FakeFb::new(1 << 20);
    let mem = GuestMemory::new();
    let lit = |on: bool| {
        let mut armed = BTreeMap::new();
        // SOR0 OWNER_MASK = head 0, PROTOCOL DP_A (clc57d.h SOR_SET_CONTROL).
        armed.insert(0x300, if on { 0x1 | (8 << 8) } else { 0 });
        // Head 0: raster 2200x1125, pixel clock 148.5 MHz, viewport 1920x1080.
        armed.insert(0x2000 + 0x64, 2200 | (1125 << 16));
        armed.insert(0x2000 + 0x0C, 148_500_000);
        armed.insert(0x2000 + 0x4C, 1920 | (1080 << 16));
        armed
    };
    let ask = |disp: &mut Display, head: u32, flags: u32| {
        let l = &crate::ver::layouts().display;
        let mut p = vec![0u8; l.get_active_size];
        put32(&mut p, d::GET_ACTIVE_HEAD, head);
        put32(&mut p, d::GET_ACTIVE_FLAGS, flags);
        disp.control(d::CMD_SYSTEM_GET_ACTIVE, &p).unwrap()
    };
    disp.engine.core_update(&lit(true), 1_000, &mut fb, &mem);
    for flags in [0, 1] {
        let a = ask(&mut disp, 0, flags);
        assert_eq!((a.status, get32(&a.params, d::GET_ACTIVE_DISPLAY_ID)), (NV_OK, DP), "{}", a.note);
        assert!(a.note.contains("0x100 (SOR0"), "{}", a.note);
    }
    for head in 1..4 {
        let a = ask(&mut disp, head, 0);
        assert_eq!((a.status, get32(&a.params, d::GET_ACTIVE_DISPLAY_ID)), (NV_OK, 0));
    }
    let a = ask(&mut disp, 4, 0);
    assert_eq!(a.status, NV_ERR_INVALID_ARGUMENT, "{}", a.note);
    disp.engine.core_update(&lit(false), 2_000, &mut fb, &mem);
    let a = ask(&mut disp, 0, 0);
    assert_eq!((a.status, get32(&a.params, d::GET_ACTIVE_DISPLAY_ID)), (NV_OK, 0));
}

/// DP_GET_CAPS: 615's `bIsInternalDpTunnelingSupported` (33, former padding) reads 0 (no
/// on-SOC USB4 tunnel), and DSC stays at 36 in both ABIs.
#[test]
fn dp_caps_claim_no_internal_dp_tunneling() {
    let mut disp = display();
    let mut p = vec![0xFFu8; d::DP_CAPS_SIZE];
    put32(&mut p, 0, 0);
    put32(&mut p, 4, 0);
    let a = disp.control(d::CMD_DP_GET_CAPS, &p).unwrap();
    assert_eq!(
        a.params[d::DP_CAPS_DSC_SUPPORTED],
        disp.chip().link.dsc as u8
    );
    for l in [&crate::ver::L610.display, &crate::ver::L615.display] {
        if let Some(o) = l.dp_caps_internal_dp_tunneling {
            assert_eq!((o, a.params[o]), (33, 0));
        }
    }
    assert_eq!(crate::ver::L615.display.dp_caps_internal_dp_tunneling, Some(33));
}

/// Every field the display's D models use resolves in both ABIs' generated layouts.
#[test]
fn d_model_fields_resolve_in_both_abis() {
    for abi in crate::ver::ABIS {
        for (s, fs) in [
            (
                "NV0073_CTRL_SPECIFIC_HDCP_CTRL_PARAMS",
                &["subDeviceInstance", "displayId", "cmd", "err", "flags"][..],
            ),
            (
                "NV0073_CTRL_CMD_DP_CONFIGURE_FEC_PARAMS",
                &["displayId", "bEnableFec"][..],
            ),
            (
                "NV5070_CTRL_CMD_GET_RG_STATUS_PARAMS",
                &["head", "scanLocked", "flipLocked", "rgStalled"][..],
            ),
        ] {
            let l = abi.layout(s).unwrap_or_else(|| panic!("{}: {s}", abi.name));
            for f in fs {
                l.resolve(f).unwrap();
            }
        }
        for n in [
            "NV0073_CTRL_SPECIFIC_HDCP_CTRL_CMD_DISABLE_AUTHENTICATION",
            "NV0073_CTRL_SPECIFIC_HDCP_CTRL_ERR_UNSUCCESSFUL_YES",
            "NV5070_CTRL_CMD_GET_RG_STATUS_SCANLOCKED_NO",
        ] {
            assert!(abi.d_enum(n).is_some(), "{}: {n}", abi.name);
        }
    }
}

/// NV0073_CTRL_CMD_SYSTEM_GET_HOTPLUG_CONFIG, which the Windows KMD asks at adapter start: every
/// connector hot-plug pollable and interruptible, no events requested, none always attached.
#[test]
fn hotplug_config_names_every_connector() {
    let mut disp = display();
    let m = disp.supported_mask();
    assert_ne!(m, 0);
    let a = disp.control(0x0073_0109, &[0u8; 24]).unwrap();
    assert_eq!(a.status, 0);
    let w = |o: usize| u32::from_le_bytes(a.params[o..o + 4].try_into().unwrap());
    assert_eq!((w(8), w(12), w(16), w(20)), (0, m, m, 0));
}

/// CARAXES_WIN_DISPLAY_QUERIES=1: the KMD's mux, sideband-I2C and ACPI DOD queries answered.
#[test]
fn windows_display_queries() {
    let mut disp = display();
    if std::env::var("CARAXES_WIN_DISPLAY_QUERIES").is_err() {
        let a = disp.control(0x0073_013D, &[0xAAu8; 8]).unwrap();
        assert_ne!(a.status, 0);
        return;
    }
    let mut p = [0u8; 12];
    p[4] = 0;
    p[5] = 1;
    let a = disp.control(0x0073_0285, &p).unwrap();
    assert_eq!((a.status, &a.params[8..12]), (0, &[0xFFu8; 4][..]));
}

/// The kick page's kick (chan_page.rs): a window channel's `PUT` from the page runs the
/// same walk as the trapped write and returns `GET`; a page without a channel is `None`.
#[test]
fn a_page_kick_is_the_trapped_kick() {
    let mut disp = display();
    let mut mem = GuestMemory::new();
    let f = tempfile_4k();
    mem.map(0x10_0000, 0x1000, 0, &f).unwrap();
    // Window 6: SET_PRESENT_CONTROL-like method 0x308 then UPDATE (0x200).
    let words = [1u32 << 18 | 0x308, 0, 1 << 18 | 0x200, 0];
    for (i, w) in words.iter().enumerate() {
        assert!(mem.write(0x10_0000 + 4 * i as u64, &w.to_le_bytes()));
    }
    let mut p = vec![0u8; d::PUSHBUFFER_SIZE];
    put32(&mut p, d::PUSHBUFFER_ADDRESS_SPACE, 1);
    p[8..16].copy_from_slice(&0x10_0000u64.to_le_bytes());
    p[16..24].copy_from_slice(&0xFFFu64.to_le_bytes());
    put32(&mut p, d::PUSHBUFFER_HCLASS, d::NVC57E_WINDOW_CHANNEL_DMA);
    put32(&mut p, d::PUSHBUFFER_CHANNEL_INSTANCE, 6);
    p[d::PUSHBUFFER_VALID] = 1;
    disp.control(d::CMD_INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER, &p);
    let mut fb = crate::fb::FakeFb::new(1 << 20);
    let w6 = WIN_USERD + 6 * 0x1000;
    assert!(disp.page_kick(WIN_USERD + 7 * 0x1000, 16, &mem, &mut fb).is_none());
    let (get, notes) = disp.page_kick(w6, 16, &mem, &mut fb).unwrap();
    assert_eq!(get, 16);
    assert!(notes[0].contains("2 method(s), 1 UPDATE(s)"), "{notes:?}");
    assert_eq!(disp.read32(w6 + 4), Some(16));
    assert_eq!(disp.channels[&w6].kicks, 1);
}

/// A channel page's lower half reads back the assembly state, the upper half (+2048) the
/// ARMED state the last `UPDATE` promoted (dev_display_withoffset.ref.txt:5956-5973): the
/// Windows KMD reads window 0's ARMED `SET_PARAMS` (+0xA2C) and `SET_COMPOSITION_CONTROL`
/// (+0xAEC) after each flip. The ARMED half is read-only; the cursor's PIO methods arm too.
#[test]
fn channel_pages_read_back_assembly_and_armed_state() {
    let mut disp = display();
    let mut mem = GuestMemory::new();
    let f = tempfile_4k();
    mem.map(0x10_0000, 0x1000, 0, &f).unwrap();
    // Window 0: SET_PARAMS (0x22C) = 0xCF, SET_COMPOSITION_CONTROL (0x2EC) = 0xFF0; then
    // UPDATE; then SET_PARAMS = 0xE6 without UPDATE.
    let words = [
        1u32 << 18 | 0x22C, 0xCF,
        1 << 18 | 0x2EC, 0xFF0,
        1 << 18 | 0x200, 0,
        1 << 18 | 0x22C, 0xE6,
    ];
    for (i, w) in words.iter().enumerate() {
        assert!(mem.write(0x10_0000 + 4 * i as u64, &w.to_le_bytes()));
    }
    let mut p = vec![0u8; d::PUSHBUFFER_SIZE];
    put32(&mut p, d::PUSHBUFFER_ADDRESS_SPACE, 1);
    p[8..16].copy_from_slice(&0x10_0000u64.to_le_bytes());
    p[16..24].copy_from_slice(&0xFFFu64.to_le_bytes());
    put32(&mut p, d::PUSHBUFFER_HCLASS, d::NVC57E_WINDOW_CHANNEL_DMA);
    p[d::PUSHBUFFER_VALID] = 1;
    disp.control(d::CMD_INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER, &p);
    disp.alloc(d::NVC57E_WINDOW_CHANNEL_DMA, &vec![0; d::CHANNELDMA_ALLOC_SIZE])
        .unwrap();
    let mut fb = crate::fb::FakeFb::new(1 << 20);
    let w0 = WIN_USERD;
    assert_eq!(disp.read32(w0 + CHN_ARMED + 0x22C), Some(0));
    disp.write32(w0, 24, &mem, &mut fb).unwrap();
    assert_eq!(disp.read32(w0 + 0x22C), Some(0xCF));
    assert_eq!(disp.read32(w0 + CHN_ARMED + 0x22C), Some(0xCF));
    assert_eq!(disp.read32(w0 + CHN_ARMED + 0x2EC), Some(0xFF0));
    // Assembly moves on; ARMED waits for the next UPDATE.
    disp.write32(w0, 32, &mem, &mut fb).unwrap();
    assert_eq!(disp.read32(w0 + 0x22C), Some(0xE6));
    assert_eq!(disp.read32(w0 + CHN_ARMED + 0x22C), Some(0xCF));
    // PUT and GET are unchanged by the readback.
    assert_eq!(disp.read32(w0), Some(32));
    assert_eq!(disp.read32(w0 + 4), Some(32));
    let notes = disp.write32(w0 + CHN_ARMED + 0x22C, 1, &mem, &mut fb).unwrap();
    assert!(notes[0].contains("read-only"), "{notes:?}");
    assert_eq!(disp.read32(w0 + CHN_ARMED + 0x22C), Some(0xCF));
    // The cursor (PIO): a method write shows in the assembly half, UPDATE arms it.
    let c0 = CURSOR_USERD;
    disp.write32(c0 + 0x208, 0x0010_0020, &mem, &mut fb).unwrap();
    assert_eq!(disp.read32(c0 + 0x208), Some(0x0010_0020));
    assert_eq!(disp.read32(c0 + CHN_ARMED + 0x208), Some(0));
    disp.write32(c0 + 0x200, 0, &mem, &mut fb).unwrap();
    assert_eq!(disp.read32(c0 + CHN_ARMED + 0x208), Some(0x0010_0020));
}

/// The core channel's PIO registers (clc57d.h:285-297): SET_GET_BLANKING_CTRL takes a
/// request (BLANK 0:0, UNBLANK 1:1, NO_CHANGE or ENABLE) that the model carries out at once,
/// and a read gives the state reached in the same fields (the Windows KMD writes BLANK and
/// waits for the read to show it); GET_RG_SCAN_LINE gives the raster line and the VBLANK bit
/// and ignores writes.
#[test]
fn core_pio_blanking_and_scan_line() {
    let mut disp = display();
    let mut fb = FakeFb::new(1 << 20);
    let mem = GuestMemory::new();
    let blank = CORE_USERD + CORE_SET_GET_BLANKING_CTRL;
    let n = disp.write32(blank, 0x1, &mem, &mut fb).unwrap();
    assert!(n[0].contains("head 0 SET_GET_BLANKING_CTRL 0x1: blanked"), "{n:?}");
    assert!(disp.engine.heads[0].blanked);
    assert_eq!(disp.read32(blank), Some(0x1), "the state reached: BLANK");
    let n = disp.write32(blank, 0x2, &mem, &mut fb).unwrap();
    assert!(n[0].contains("unblanked"), "{n:?}");
    assert!(!disp.engine.heads[0].blanked);
    assert_eq!(disp.read32(blank), Some(0x2), "the state reached: UNBLANK");
    let n = disp.write32(blank + 4, 0x3, &mem, &mut fb).unwrap();
    assert!(n[0].contains("head 1") && n[0].contains("stays unblanked"), "BLANK then UNBLANK: {n:?}");
    // A head beyond the chip's four: not a PIO register, latched as before.
    assert_eq!(disp.write32(blank + 16, 0x1, &mem, &mut fb), Some(Vec::new()));
    // An idle head: line 0, in the blank; an active one: a line below vtotal, VBLANK set
    // outside [BLANK_END.Y, BLANK_START.Y) = [40, 1120).
    let scan = CORE_USERD + CORE_GET_RG_SCAN_LINE;
    assert_eq!(disp.read32(scan), Some(1 << 16));
    let mut armed = BTreeMap::new();
    armed.insert(0x300, 0x1 | (8 << 8));
    armed.insert(0x2000 + 0x64, 2200 | (1125 << 16));
    armed.insert(0x2000 + 0x6C, 191 | (40 << 16));
    armed.insert(0x2000 + 0x70, 2111 | (1120 << 16));
    armed.insert(0x2000 + 0x0C, 148_500_000);
    armed.insert(0x2000 + 0x4C, 1920 | (1080 << 16));
    disp.engine.core_update(&armed, engine::now_ns(), &mut fb, &mem);
    let v = disp.read32(scan).unwrap();
    let line = v & 0xFFFF;
    assert!(line < 1125, "{v:#x}");
    assert_eq!(v >> 16 & 1 == 1, !(40..1120).contains(&line), "{v:#x}");
    assert!(disp.engine.in_vblank(0, 10) && disp.engine.in_vblank(0, 1121) && !disp.engine.in_vblank(0, 500));
    let n = disp.write32(scan, 0x5, &mem, &mut fb).unwrap();
    assert!(n[0].contains("read-only"), "{n:?}");
}

/// `dump auto`: window 0's scanout is written after a new latch, not again before the period,
/// and the pixels come out of the pitch layout in the right order.
#[test]
fn auto_dump_writes_new_latches_rate_limited() {
    let mut disp = display();
    let mut fb = FakeFb::new(1 << 20);
    let mem = GuestMemory::new();
    // 16 x 2 pixels, A8R8G8B8, pitch layout (one 64-byte unit per row): pixel (x, y) = x + 16y.
    for y in 0..2u64 {
        for x in 0..16u64 {
            let v = (x + 16 * y) as u32;
            fb.write(y * 64 + x * 4, &(v | 0xFF00_0000).to_le_bytes());
        }
    }
    let dir = std::env::temp_dir().join(format!("caraxes-autodump-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    disp.auto_dump = Some(AutoDump::new(dir.clone()));
    let w = &mut disp.engine.windows[0];
    w.head = Some(0);
    w.flips = 1;
    w.latched = engine::Surface {
        mem: Some(engine::MemRef { target: 1, addr: 0 }),
        width: 16,
        height: 2,
        format: 0xCF,
        pitch: 1,
        pitch_layout: true,
        block_height_log2: 0,
    };
    disp.maybe_auto_dump(&fb, &mem);
    disp.auto_dump.as_mut().unwrap().join();
    let f = dir.join("scanout-auto-0.png");
    let png = std::fs::read(&f).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    // IDAT holds stored rows: filter 0, then R, G, B of pixel 0 (0, 0, 0), pixel 1 (0, 0, 1).
    let at = png.windows(4).position(|c| c == b"IDAT").unwrap() + 4 + 2 + 5;
    assert_eq!(&png[at..at + 7], &[0, 0, 0, 0, 0, 0, 1]);
    assert!(disp.notes.iter().any(|n| n.contains("auto dump #0") && n.contains("1 step(s)")), "{:?}", disp.notes);
    // A new flip within the period: no second file.
    disp.engine.windows[0].flips = 2;
    disp.maybe_auto_dump(&fb, &mem);
    assert!(!dir.join("scanout-auto-1.png").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

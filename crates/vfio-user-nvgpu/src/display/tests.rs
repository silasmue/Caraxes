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
    let mut fb = crate::fb::FakeFb::new(1 << 20);
    let notes = disp.write32(CORE_USERD, 16, &mem, &mut fb).unwrap();
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
    let mut disp = display();
    let Some(id) = d::CMD_DP_SET_WAR_FLAGS else {
        assert_eq!(crate::ver::active::DRIVER_VERSION, "610.57.04");
        assert!(d_id("NV0073_CTRL_CMD_DP_SET_WAR_FLAGS").is_none());
        return;
    };
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

/// SYSTEM_GET_ACTIVE: no firmware-lit head, so from 615 on no stale head either: the output
/// flag GOP_STALE_HEAD_NEEDS_CLEANUP is cleared and `staleSorIndex` is 0, whatever the
/// caller left in them; the input flag CLIENT is kept.
#[test]
fn get_active_reports_no_head_and_no_stale_head() {
    let mut disp = display();
    let mut p = vec![0xAAu8; d::GET_ACTIVE_SIZE];
    put32(&mut p, d::GET_ACTIVE_HEAD, 1);
    put32(&mut p, d::GET_ACTIVE_FLAGS, 0x3);
    let a = disp.control(d::CMD_SYSTEM_GET_ACTIVE, &p).unwrap();
    assert_eq!(a.status, NV_OK);
    assert_eq!(a.params.len(), d::GET_ACTIVE_SIZE);
    assert_eq!(get32(&a.params, d::GET_ACTIVE_DISPLAY_ID), 0);
    assert_eq!(
        get32(&a.params, d::GET_ACTIVE_FLAGS),
        0x3 & !d::GET_ACTIVE_FLAGS_GOP_STALE_HEAD
    );
    match d::GET_ACTIVE_STALE_SOR_INDEX {
        Some(o) => {
            assert_eq!((o, d::GET_ACTIVE_SIZE, get32(&a.params, o)), (16, 20, 0));
            assert_eq!(get32(&a.params, d::GET_ACTIVE_FLAGS), 0x1);
        }
        None => assert_eq!(d::GET_ACTIVE_SIZE, 16),
    }
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
    if let Some(o) = d::DP_CAPS_INTERNAL_DP_TUNNELING {
        assert_eq!((o, a.params[o]), (33, 0));
    }
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

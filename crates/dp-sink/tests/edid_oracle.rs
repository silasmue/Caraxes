// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The EDID generator checked by two oracles that share no code with it: a small parser
//! written here from the VESA EDID 1.4 layout, and the external `edid-decode` tool when it
//! is installed (set `EDID_DECODE` to point elsewhere; the test says when it skips).
//! The EDID is also read back over I2C-over-AUX the way the NVIDIA DP library does, so the
//! oracles see what a source would see.

use dp_sink::{AuxRequest, CtaExtension, DpSink, EdidSpec, SinkProfile, DDC_EDID_ADDR};
use std::io::Write;
use std::process::{Command, Stdio};

/// What the independent parser extracts from a base block.
#[derive(Debug, PartialEq)]
struct Parsed {
    manufacturer: String,
    version: (u8, u8),
    digital_dp_8bpc: bool,
    extensions: u8,
    pclk_10khz: u16,
    h: (u16, u16, u16, u16), // active, front, sync, back
    v: (u16, u16, u16, u16),
    h_pos: bool,
    v_pos: bool,
    size_mm: (u16, u16),
    name: String,
    range: (u8, u8, u8, u8, u16), // vmin vmax hmin hmax pclk MHz
    standard: Vec<(u16, u16, u8)>,
}

fn parse_base(b: &[u8]) -> Parsed {
    assert_eq!(b.len(), 128);
    assert_eq!(&b[..8], b"\x00\xff\xff\xff\xff\xff\xff\x00", "header");
    let sum: u32 = b.iter().map(|&x| x as u32).sum();
    assert_eq!(sum % 256, 0, "checksum");
    let id = u16::from_be_bytes([b[8], b[9]]);
    let letter = |s: u16| (b'@' + ((id >> s) & 0x1F) as u8) as char;
    let manufacturer: String = [letter(10), letter(5), letter(0)].iter().collect();
    // Detailed timing descriptor at 0x36 (field layout per EDID 1.4 section 3.10.2).
    let d = &b[0x36..0x48];
    let pclk_10khz = u16::from_le_bytes([d[0], d[1]]);
    assert_ne!(pclk_10khz, 0, "first descriptor must be a timing");
    let hact = d[2] as u16 | ((d[4] as u16 >> 4) << 8);
    let hblank = d[3] as u16 | ((d[4] as u16 & 0xF) << 8);
    let vact = d[5] as u16 | ((d[7] as u16 >> 4) << 8);
    let vblank = d[6] as u16 | ((d[7] as u16 & 0xF) << 8);
    let hfp = d[8] as u16 | (((d[11] >> 6) & 3) as u16) << 8;
    let hsw = d[9] as u16 | (((d[11] >> 4) & 3) as u16) << 8;
    let vfp = (d[10] >> 4) as u16 | (((d[11] >> 2) & 3) as u16) << 4;
    let vsw = (d[10] & 0xF) as u16 | ((d[11] & 3) as u16) << 4;
    let hmm = d[12] as u16 | ((d[14] as u16 >> 4) << 8);
    let vmm = d[13] as u16 | ((d[14] as u16 & 0xF) << 8);
    assert_eq!(d[17] & 0x18, 0x18, "digital separate sync");
    let mut name = String::new();
    let mut range = (0, 0, 0, 0, 0);
    for desc in b[0x48..0x7E].chunks(18) {
        assert_eq!(&desc[..3], &[0, 0, 0], "display descriptor");
        match desc[3] {
            0xFC => {
                name = desc[5..]
                    .iter()
                    .take_while(|&&c| c != 0x0A)
                    .map(|&c| c as char)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            }
            0xFD => {
                range = (desc[5], desc[6], desc[7], desc[8], desc[9] as u16 * 10);
            }
            _ => {}
        }
    }
    let mut standard = Vec::new();
    for st in b[0x26..0x36].chunks(2) {
        if st == [1, 1] {
            continue;
        }
        let h = (st[0] as u16 + 31) * 8;
        let v = match st[1] >> 6 {
            0 => h * 10 / 16,
            1 => h * 3 / 4,
            2 => h * 4 / 5,
            _ => h * 9 / 16,
        };
        standard.push((h, v, (st[1] & 0x3F) + 60));
    }
    Parsed {
        manufacturer,
        version: (b[0x12], b[0x13]),
        digital_dp_8bpc: b[0x14] == 0x80 | 0x20 | 0x05,
        extensions: b[0x7E],
        pclk_10khz,
        h: (hact, hfp, hsw, hblank - hfp - hsw),
        v: (vact, vfp, vsw, vblank - vfp - vsw),
        h_pos: d[17] & 0x02 != 0,
        v_pos: d[17] & 0x04 != 0,
        size_mm: (hmm, vmm),
        name,
        range,
        standard,
    }
}

/// Read `blocks` EDID blocks over I2C-over-AUX like dp_sst_edid.cpp readNextBlock: segment
/// write when needed, offset write with MOT, 16-byte MOT reads, the last read without MOT.
fn read_over_aux(sink: &mut DpSink, blocks: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for blk in 0..blocks {
        let (seg, off) = ((blk / 2) as u8, ((blk % 2) * 128) as u8);
        if seg != 0 {
            let r = sink
                .transact(&AuxRequest::i2c_write(0x30, &[seg], true))
                .unwrap();
            assert_eq!(r.count, 1);
        }
        sink.transact(&AuxRequest::i2c_write(DDC_EDID_ADDR, &[off], true))
            .unwrap();
        let mut got = 0;
        while got < 128 {
            let mot = got + 16 < 128;
            let r = sink
                .transact(&AuxRequest::i2c_read(DDC_EDID_ADDR, 16, mot))
                .unwrap();
            assert_eq!(r.count, 16);
            out.extend_from_slice(&r.data);
            got += r.count;
        }
    }
    out
}

fn sink_with(edid: Vec<u8>) -> DpSink {
    let mut s = DpSink::new(SinkProfile::default(), edid).unwrap();
    s.plug();
    s
}

#[test]
fn independent_parser_rederives_the_first_monitor() {
    let generated = EdidSpec::first_monitor().build().unwrap();
    let mut sink = sink_with(generated.clone());
    let over_aux = read_over_aux(&mut sink, 1);
    assert_eq!(over_aux, generated);
    let p = parse_base(&over_aux);
    // Expected values are the published CVT-RB 1920x1080@60 timing (VESA CVT 1.2 table:
    // 138.50 MHz, H 1920/48/32/80, V 1080/3/5/23, +H -V), not the crate's calculator.
    assert_eq!(
        p,
        Parsed {
            manufacturer: "CRX".into(),
            version: (1, 4),
            digital_dp_8bpc: true,
            extensions: 0,
            pclk_10khz: 13850,
            h: (1920, 48, 32, 80),
            v: (1080, 3, 5, 23),
            h_pos: true,
            v_pos: false,
            size_mm: (527, 296),
            name: "Caraxes DP".into(),
            range: (56, 61, 30, 70, 150),
            standard: vec![
                (1280, 720, 60),
                (1280, 1024, 60),
                (1440, 900, 60),
                (1600, 900, 60),
                (1680, 1050, 60)
            ],
        }
    );
    // The preferred mode fits the range limits it advertises.
    let h_total = 1920u64 + 48 + 32 + 80;
    let v_total = 1080u64 + 3 + 5 + 23;
    let hz_x1000 = p.pclk_10khz as u64 * 10_000 * 1000 / (h_total * v_total);
    assert_eq!(hz_x1000, 59_933);
    assert!((p.range.0 as u64 * 1000..=p.range.1 as u64 * 1000).contains(&hz_x1000));
    assert!(p.pclk_10khz as u16 / 100 <= p.range.4);
}

#[test]
fn cta_extension_block_is_well_formed() {
    let mut spec = EdidSpec::first_monitor();
    spec.cta = Some(CtaExtension::basic());
    let generated = spec.build().unwrap();
    let mut sink = sink_with(generated.clone());
    let over_aux = read_over_aux(&mut sink, 2);
    assert_eq!(over_aux, generated);
    assert_eq!(parse_base(&over_aux[..128]).extensions, 1);
    let ext = &over_aux[128..];
    let sum: u32 = ext.iter().map(|&x| x as u32).sum();
    assert_eq!(sum % 256, 0, "extension checksum");
    assert_eq!((ext[0], ext[1]), (0x02, 0x03), "CTA-861 revision 3");
    // Walk the data block collection up to the DTD offset.
    let dtd_off = ext[2] as usize;
    let mut i = 4;
    let mut tags = Vec::new();
    while i < dtd_off {
        let tag = ext[i] >> 5;
        let len = (ext[i] & 0x1F) as usize;
        tags.push(tag);
        if tag == 2 {
            let vics: Vec<u8> = ext[i + 1..i + 1 + len].iter().map(|v| v & 0x7F).collect();
            assert_eq!(vics, [16, 4, 1]);
        }
        i += 1 + len;
    }
    assert_eq!(i, dtd_off, "data blocks end at the DTD offset");
    assert_eq!(
        tags,
        [2, 7],
        "video data block and extended (VCDB) only; no audio"
    );
    assert!(ext[dtd_off..127].iter().all(|&b| b == 0), "padding");
}

fn edid_decode(bytes: &[u8]) -> Option<String> {
    let tool = std::env::var("EDID_DECODE").unwrap_or_else(|_| "edid-decode".into());
    let mut child = match Command::new(&tool)
        .arg("--check")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => {
            eprintln!("edid-decode not found ({tool}); external oracle skipped");
            return None;
        }
    };
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let out = child.wait_with_output().unwrap();
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn edid_decode_accepts_the_first_monitor() {
    let generated = EdidSpec::first_monitor().build().unwrap();
    let Some(out) = edid_decode(&generated) else {
        return;
    };
    assert!(out.contains("EDID conformity: PASS"), "{out}");
    assert!(!out.contains("Warnings:"), "{out}");
    for needle in [
        "EDID Structure Version & Revision: 1.4",
        "DisplayPort interface",
        "Bits per primary color channel: 8",
        "DTD 1:  1920x1080   59.933878 Hz  16:9     66.587 kHz    138.500000 MHz",
        "Hfront   48 Hsync  32 Hback   80 Hpol P",
        "Vfront    3 Vsync   5 Vback   23 Vpol N",
        "Display Product Name: 'Caraxes DP'",
        "56-61 Hz V, 30-70 kHz H, max dotclock 150 MHz",
    ] {
        assert!(out.contains(needle), "missing {needle:?} in\n{out}");
    }
}

#[test]
fn edid_decode_accepts_the_cta_extension() {
    let mut spec = EdidSpec::first_monitor();
    spec.cta = Some(CtaExtension::basic());
    let Some(out) = edid_decode(&spec.build().unwrap()) else {
        return;
    };
    assert!(out.contains("EDID conformity: PASS"), "{out}");
    assert!(out.contains("Video Capability Data Block"), "{out}");
    assert!(!out.contains("Audio"), "{out}");
}

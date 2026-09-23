// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Briefing section 6: a stored AUX request sequence replayed against the sink with exactly
//! the stored replies, and a test that changes the lane configuration and the EDID offset
//! so that a hard-wired success transcript cannot pass.

use dp_sink::transcript::{format_outcome, record, replay};
use dp_sink::{AuxRequest, DpSink, SinkProfile, DDC_EDID_ADDR};

/// The recorded capture of the stock 610.57.04 guest (display-ab.md, run b3).
const GOLDEN: &str = include_str!("transcripts/nvidia-610-guest-capture.txt");
/// The EDID the guest read in that run (byte-identical to its sysfs `edid`).
const EDID: &[u8] = include_bytes!("transcripts/caraxes-dp-monitor.edid.bin");

fn sink() -> DpSink {
    DpSink::new(SinkProfile::dp14_hbr3_x4_dsc(), EDID.to_vec()).unwrap()
}

#[test]
fn golden_transcript_replays_exactly() {
    let n = replay(&mut sink(), GOLDEN).unwrap();
    assert_eq!(n, 85, "request/reply pairs checked");
}

#[test]
fn replay_detects_a_single_wrong_byte() {
    // The replayer must compare, not just run: flip one EDID byte in the expectation.
    let bad = GOLDEN.replacen("< ack 16 27 24 01 04 a5", "< ack 16 27 24 01 04 a4", 1);
    assert_ne!(bad, GOLDEN);
    let e = replay(&mut sink(), &bad).unwrap_err();
    assert!(e.message.contains("expected"), "{e}");
    // A missing reply line is an error too.
    let e = replay(&mut sink(), "! plug\n> nr 000 1\n").unwrap_err();
    assert_eq!(e.line, 2);
}

/// Replace the first occurrence of `from` with `to`, asserting it exists.
fn edit(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "{from:?} not in transcript");
    text.replacen(from, to, 1)
}

#[test]
fn changed_lane_count_and_edid_offset_are_observed() {
    // 1. Two lanes instead of four: the capture trains through DP_CTRL (`! train`) and
    //    never reads the lane status, so the check reads it right after training.
    let upto = &GOLDEN[..GOLDEN.find("! train 1e 4 ef").unwrap() + "! train 1e 4 ef".len()];
    let four = format!("{upto}\n> nr 202 2\n< ack 2 77 77\n");
    replay(&mut sink(), &four).unwrap();
    let two_lanes = edit(&four, "! train 1e 4 ef", "! train 1e 2 ef");
    let e = replay(&mut sink(), &two_lanes).unwrap_err();
    assert!(e.message.contains("nr 202 2"), "{e}");
    assert!(e.message.contains("ack 2 77 00"), "{e}");
    // With the expected reply corrected for two lanes, it passes.
    replay(
        &mut sink(),
        &edit(&two_lanes, "< ack 2 77 77", "< ack 2 77 00"),
    )
    .unwrap();

    // 2. EDID offset 0x40 instead of 0: the first read must return bytes 0x40..0x50.
    let edid = EDID;
    let moved = edit(GOLDEN, "> iwm 50 00", "> iwm 50 40");
    let e = replay(&mut sink(), &moved).unwrap_err();
    let want = format_outcome(&Ok(dp_sink::AuxReply::read_ack(edid[0x40..0x50].to_vec())));
    assert!(e.message.contains(&want), "{e}");

    // 3. Lane configuration and offset through the API, checked against values derived
    //    here rather than from the sink: per-lane nibble 0x7 for trained lanes only.
    for (lanes, status) in [(1u8, [0x07u8, 0x00]), (2, [0x77, 0x00]), (4, [0x77, 0x77])] {
        let mut s = sink();
        s.plug();
        let script = [
            AuxRequest::native_write(0x100, &[0x06, lanes]),
            AuxRequest::native_write(0x102, &[0x21]),
            AuxRequest::native_write(0x102, &[0x22]),
            AuxRequest::native_write(0x102, &[0x00]),
            AuxRequest::native_read(0x202, 2),
            AuxRequest::i2c_write(DDC_EDID_ADDR, &[0x36], true),
            AuxRequest::i2c_read(DDC_EDID_ADDR, 4, false),
        ];
        let text = record(&mut s, &script);
        let last: Vec<&str> = text.lines().rev().take(5).collect();
        assert_eq!(
            last[4],
            format!("< ack 2 {:02x} {:02x}", status[0], status[1]),
            "{lanes} lanes"
        );
        // 0x36 is the first detailed timing: pixel clock 14850 (0x3A02 LE), h active 0x780.
        assert_eq!(last[0], "< ack 4 02 3a 80 18");
        // The recorded transcript replays against a fresh sink.
        let mut fresh = sink();
        fresh.plug();
        replay(&mut fresh, &text).unwrap();
    }
}

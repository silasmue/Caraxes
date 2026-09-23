// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The AUX cases of briefing sections 2.2 and 6, against a whole sink.

use dp_sink::{
    AuxError, AuxReply, AuxRequest, DpSink, EdidSpec, Fault, HpdEvent, I2cDir, IrqCause,
    LinkConfig, LinkError, LinkRate, LinkState, ReplyCode, SinkPorts, SinkProfile, DDC_EDID_ADDR,
    DDC_SEGMENT_ADDR,
};

fn edid() -> Vec<u8> {
    EdidSpec::first_monitor().build().unwrap()
}

fn sink() -> DpSink {
    let mut s = DpSink::new(SinkProfile::default(), edid()).unwrap();
    s.plug();
    s.take_events();
    s
}

fn ok(s: &mut DpSink, req: AuxRequest) -> AuxReply {
    s.transact(&req).unwrap()
}

#[test]
fn native_read_one_and_sixteen_bytes() {
    let mut s = sink();
    assert_eq!(
        ok(&mut s, AuxRequest::native_read(0x000, 1)),
        AuxReply::read_ack(vec![0x11])
    );
    let r = ok(&mut s, AuxRequest::native_read(0x000, 16));
    assert_eq!(r.code, ReplyCode::Ack);
    assert_eq!(r.count, 16);
    assert_eq!(&r.data[..4], &[0x11, 0x0A, 0x84, 0x01]);
}

#[test]
fn native_write_one_and_sixteen_bytes() {
    let mut s = sink();
    assert_eq!(
        ok(&mut s, AuxRequest::native_write(0x300, &[0xAB])),
        AuxReply::write_ack(1)
    );
    // 16 bytes across writable (0x100-0x108) and undefined (0x109-0x10F) registers.
    let data: Vec<u8> = (0..16).collect();
    assert_eq!(
        ok(&mut s, AuxRequest::native_write(0x100, &data)),
        AuxReply::write_ack(16)
    );
    let r = ok(&mut s, AuxRequest::native_read(0x100, 16));
    assert_eq!(
        r.data,
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn i2c_read_one_and_sixteen_bytes() {
    let e = edid();
    let mut s = sink();
    ok(&mut s, AuxRequest::i2c_write(DDC_EDID_ADDR, &[0x08], true));
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 1, true)),
        AuxReply::read_ack(e[8..9].to_vec())
    );
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 16, false)),
        AuxReply::read_ack(e[9..25].to_vec())
    );
}

#[test]
fn short_reply() {
    let e = edid();
    // Profile limit: at most 5 bytes per I2C read reply.
    let p = SinkProfile {
        max_i2c_reply: 5,
        ..SinkProfile::default()
    };
    let mut s = DpSink::new(p, e.clone()).unwrap();
    s.plug();
    let r = ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 16, true));
    assert_eq!((r.code, r.count), (ReplyCode::Ack, 5));
    assert_eq!(r.data, e[0..5]);
    // The next read continues after the short reply.
    let r = ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 16, false));
    assert_eq!(r.data, e[5..10]);
    // End of the EDID: 8 of 16 bytes, then NACK.
    let mut s = sink();
    ok(&mut s, AuxRequest::i2c_write(DDC_EDID_ADDR, &[120], true));
    let r = ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 16, true));
    assert_eq!(
        (r.code, r.count, r.data.as_slice()),
        (ReplyCode::Ack, 8, &e[120..128])
    );
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 16, false)),
        AuxReply::status(ReplyCode::I2cNack, 0)
    );
}

#[test]
fn address_only_transactions() {
    let mut s = sink();
    // START + address with MOT opens a write transaction without data.
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_write(DDC_EDID_ADDR, &[], true)),
        AuxReply::write_ack(0)
    );
    assert_eq!(s.i2c_state().open, Some((DDC_EDID_ADDR, I2cDir::Write)));
    // Address-only without MOT is START + address + STOP.
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_write(DDC_EDID_ADDR, &[], false)),
        AuxReply::write_ack(0)
    );
    assert_eq!(s.i2c_state().open, None);
    // Address-only read to an absent device: address phase NACK.
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_read(0x37, 0, false)),
        AuxReply::status(ReplyCode::I2cNack, 0)
    );
    // Address-only does not move the EEPROM offset.
    ok(&mut s, AuxRequest::i2c_write(DDC_EDID_ADDR, &[0x10], true));
    ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 0, true));
    assert_eq!(s.i2c_state().offset, 0x10);
}

#[test]
fn mot_continuation_and_segment_pointer() {
    // Four blocks so segment 1 exists.
    let e: Vec<u8> = (0..512).map(|i| (i * 7 % 256) as u8).collect();
    let mut s = DpSink::new(SinkProfile::default(), e.clone()).unwrap();
    s.plug();
    // Segment 1, offset 0x80 in one MOT transaction, then two continued reads.
    ok(&mut s, AuxRequest::i2c_write(DDC_SEGMENT_ADDR, &[1], true));
    ok(&mut s, AuxRequest::i2c_write(DDC_EDID_ADDR, &[0x80], true));
    let a = ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 16, true));
    assert_eq!(a.data, e[384..400]);
    assert_eq!(s.i2c_state().segment, 1);
    let b = ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 16, false));
    assert_eq!(b.data, e[400..416]);
    // STOP reset the segment pointer; the offset continues in segment 0.
    assert_eq!(s.i2c_state().segment, 0);
    let c = ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 16, false));
    assert_eq!(c.data, e[160..176]);
}

#[test]
fn nack_cases() {
    let mut s = sink();
    // I2C: absent device, read of the write-only segment pointer, extra write bytes (M = 1).
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_read(0x51, 1, false)),
        AuxReply::status(ReplyCode::I2cNack, 0)
    );
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_read(DDC_SEGMENT_ADDR, 1, false)),
        AuxReply::status(ReplyCode::I2cNack, 0)
    );
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_write(DDC_EDID_ADDR, &[0, 1], false)),
        AuxReply::status(ReplyCode::I2cNack, 1)
    );
    // Native: a request running past the 20-bit address space.
    assert_eq!(
        ok(&mut s, AuxRequest::native_read(0xF_FFF8, 16)),
        AuxReply::status(ReplyCode::Nack, 0)
    );
    assert_eq!(
        ok(&mut s, AuxRequest::native_read(0xF_FFF0, 16)).code,
        ReplyCode::Ack
    );
    // Injected NACKs.
    s.inject(Fault::AuxNack);
    assert_eq!(
        ok(&mut s, AuxRequest::native_read(0, 1)),
        AuxReply::status(ReplyCode::Nack, 0)
    );
    s.inject(Fault::I2cNack);
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 1, false)),
        AuxReply::status(ReplyCode::I2cNack, 0)
    );
}

#[test]
fn defer_leaves_state_unchanged() {
    let e = edid();
    let mut s = sink();
    s.inject(Fault::AuxDefer);
    s.inject(Fault::I2cDefer);
    assert_eq!(
        ok(&mut s, AuxRequest::native_write(0x100, &[0x0A])),
        AuxReply::status(ReplyCode::Defer, 0)
    );
    assert_eq!(s.dpcd().read_byte(0x100), 0, "deferred write not applied");
    ok(&mut s, AuxRequest::i2c_write(DDC_EDID_ADDR, &[0x20], true));
    // (the second fault hit the offset write above)
    assert_eq!(s.i2c_state().offset, 0);
    ok(&mut s, AuxRequest::i2c_write(DDC_EDID_ADDR, &[0x20], true));
    s.inject(Fault::I2cDefer);
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 16, true)),
        AuxReply::status(ReplyCode::I2cDefer, 0)
    );
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 16, false)).data,
        e[0x20..0x30]
    );
    // An I2C fault on a native request becomes the AUX-level reply.
    s.inject(Fault::I2cDefer);
    assert_eq!(
        ok(&mut s, AuxRequest::native_read(0, 1)).code,
        ReplyCode::Defer
    );
}

#[test]
fn invalid_length_is_a_transport_error() {
    let mut s = sink();
    for req in [
        AuxRequest::native_read(0, 0),
        AuxRequest::native_read(0, 17),
        AuxRequest::native_write(0x100, &[]),
        AuxRequest::native_write(0x100, &[0; 17]),
        AuxRequest::i2c_read(DDC_EDID_ADDR, 17, false),
        AuxRequest::i2c_write(DDC_EDID_ADDR, &[0; 17], false),
    ] {
        assert!(
            matches!(s.transact(&req), Err(AuxError::InvalidLength { .. })),
            "{req:?}"
        );
    }
    // Faults are not consumed by requests that never became transactions.
    s.inject(Fault::AuxDefer);
    assert!(s.transact(&AuxRequest::native_read(0, 0)).is_err());
    assert_eq!(
        ok(&mut s, AuxRequest::native_read(0, 1)).code,
        ReplyCode::Defer
    );
}

#[test]
fn invalid_port_is_a_transport_error() {
    let mut ports = SinkPorts::new();
    ports.insert(0x100, sink());
    assert_eq!(
        ports.transact(0x200, &AuxRequest::native_read(0, 1)),
        Err(AuxError::NoSuchPort(0x200))
    );
    assert_eq!(
        ports
            .transact(0x100, &AuxRequest::native_read(0, 1))
            .unwrap()
            .data,
        [0x11]
    );
    assert_eq!(ports.ports().collect::<Vec<_>>(), [0x100]);
}

#[test]
fn unplugged_sink_times_out() {
    let mut s = DpSink::new(SinkProfile::default(), edid()).unwrap();
    assert_eq!(
        s.transact(&AuxRequest::native_read(0, 1)),
        Err(AuxError::Timeout)
    );
    s.plug();
    s.inject(Fault::Timeout);
    assert_eq!(
        s.transact(&AuxRequest::native_read(0, 1)),
        Err(AuxError::Timeout)
    );
    assert!(s.transact(&AuxRequest::native_read(0, 1)).is_ok());
}

#[test]
fn reset_abandons_transfer_in_progress() {
    let e = edid();
    let mut s = sink();
    ok(&mut s, AuxRequest::i2c_write(DDC_EDID_ADDR, &[0x40], true));
    ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 16, true));
    assert!(s.i2c_state().open.is_some());
    s.aux_reset();
    assert_eq!(s.i2c_state().open, None);
    // Not continued at 0x50: the EEPROM starts over at 0.
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_read(DDC_EDID_ADDR, 16, false)).data,
        e[0..16]
    );
    // Unplug also resets; after replug the DPCD is back at power-on values.
    ok(&mut s, AuxRequest::native_write(0x100, &[0x0A, 0x84]));
    s.unplug();
    s.plug();
    assert_eq!(s.dpcd().read_byte(0x100), 0);
    assert_eq!(s.i2c_state().offset, 0);
}

#[test]
fn write_status_request() {
    let mut s = sink();
    ok(&mut s, AuxRequest::i2c_write(DDC_EDID_ADDR, &[0x00], true));
    assert_eq!(
        ok(&mut s, AuxRequest::i2c_write_status(DDC_EDID_ADDR, true)),
        AuxReply::write_ack(0)
    );
    assert!(s.i2c_state().open.is_some());
    ok(&mut s, AuxRequest::i2c_write_status(DDC_EDID_ADDR, false));
    assert_eq!(s.i2c_state().open, None);
}

#[test]
fn read_only_writes_are_acked_and_ignored() {
    let mut s = sink();
    assert_eq!(
        ok(&mut s, AuxRequest::native_write(0x000, &[0xFF; 16])),
        AuxReply::write_ack(16)
    );
    assert_eq!(ok(&mut s, AuxRequest::native_read(0x000, 1)).data, [0x11]);
    assert_eq!(
        ok(&mut s, AuxRequest::native_write(0x202, &[0xFF; 6])),
        AuxReply::write_ack(6)
    );
    assert_eq!(ok(&mut s, AuxRequest::native_read(0x202, 6)).data, [0; 6]);
}

#[test]
fn direct_training_api() {
    let mut s = sink();
    let cfg = LinkConfig {
        rate: LinkRate::Hbr,
        lanes: 2,
        enhanced_framing: true,
    };
    s.train(cfg).unwrap();
    assert_eq!(s.link_state(), LinkState::Trained);
    assert_eq!(s.dpcd().active_config(), Some(cfg));
    assert_eq!(
        ok(&mut s, AuxRequest::native_read(0x100, 2)).data,
        [0x0A, 0x82]
    );
    assert_eq!(
        ok(&mut s, AuxRequest::native_read(0x202, 2)).data,
        [0x77, 0x00]
    );
    assert_eq!(
        s.train(LinkConfig {
            rate: LinkRate::Hbr2,
            ..cfg
        }),
        Err(LinkError::RateAboveMax(LinkRate::Hbr2))
    );
    assert_eq!(
        s.train(LinkConfig { lanes: 3, ..cfg }),
        Err(LinkError::LaneCount(3))
    );
    s.unplug();
    assert_eq!(s.train(cfg), Err(LinkError::Disconnected));
    let mut h = DpSink::new(SinkProfile::dp14_hbr3_x4_dsc(), edid()).unwrap();
    h.plug();
    h.train(LinkConfig {
        rate: LinkRate::Hbr3,
        lanes: 4,
        enhanced_framing: true,
    })
    .unwrap();
    assert_eq!(h.dpcd().read_byte(0x102), 0);
}

#[test]
fn hotplug_and_irq_are_separate_events() {
    let mut s = DpSink::new(SinkProfile::default(), edid()).unwrap();
    assert!(!s.service_irq(0x02), "no IRQ while unplugged");
    assert!(s.plug());
    assert!(!s.drop_link(), "nothing to lose before training");
    s.train(LinkConfig {
        rate: LinkRate::Rbr,
        lanes: 1,
        enhanced_framing: false,
    })
    .unwrap();
    assert!(s.drop_link());
    assert!(s.hpd().irq_pending());
    assert!(s.service_irq(0x04));
    assert_eq!(
        s.take_events(),
        [
            HpdEvent::Plug,
            HpdEvent::Irq(IrqCause::LinkStatus),
            HpdEvent::Irq(IrqCause::DeviceService(0x04)),
        ]
    );
    // A new EDID while plugged is a replug (two long pulses).
    let mut spec = EdidSpec::first_monitor();
    spec.name = "Caraxes DP 2".into();
    s.set_edid(spec.build().unwrap()).unwrap();
    assert_eq!(s.take_events(), [HpdEvent::Unplug, HpdEvent::Plug]);
    assert_eq!(s.link_state(), LinkState::Connected);
    assert!(s.set_edid(vec![0; 100]).is_err());
}

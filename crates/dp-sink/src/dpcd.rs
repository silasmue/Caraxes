// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The DPCD register model and the sink's link-training state machine.
//!
//! Semantics for addresses the model does not define (decided here, see the README):
//! - reads of undefined or reserved addresses return 0;
//! - writes to read-only, reserved or undefined addresses are acknowledged and ignored
//!   (the AUX reply is ACK with the full byte count). They are never NACKed: a native NACK
//!   means "the sink did not accept the request", and a burst write that crosses a
//!   read-only byte would otherwise turn into a partial write the source has to repair;
//! - writable registers store only their defined bits; reserved bits read back as 0;
//! - a write to a writable register with a value the sink does not support is stored as
//!   written; the refusal shows in the link state and the lane status when training starts
//!   (this is how a source discovers it, by reading 0x202-0x207).

use crate::link::{LinkConfig, LinkError, LinkState, TrainingPattern};
use crate::profile::SinkProfile;

/// DPCD addresses used by the model (names follow the DP specification and Linux
/// `drm_dp.h`).
pub mod reg {
    pub const DPCD_REV: u32 = 0x000;
    pub const MAX_LINK_RATE: u32 = 0x001;
    pub const MAX_LANE_COUNT: u32 = 0x002;
    pub const MAX_DOWNSPREAD: u32 = 0x003;
    pub const NORP: u32 = 0x004;
    pub const DOWNSTREAMPORT_PRESENT: u32 = 0x005;
    pub const MAIN_LINK_CHANNEL_CODING: u32 = 0x006;
    pub const DOWN_STREAM_PORT_COUNT: u32 = 0x007;
    pub const RECEIVE_PORT0_CAP_0: u32 = 0x008;
    pub const RECEIVE_PORT0_CAP_1: u32 = 0x009;
    pub const TRAINING_AUX_RD_INTERVAL: u32 = 0x00E;
    pub const MSTM_CAP: u32 = 0x021;
    pub const DSC_SUPPORT: u32 = 0x060;
    pub const FEC_CAPABILITY: u32 = 0x090;
    pub const LINK_BW_SET: u32 = 0x100;
    pub const LANE_COUNT_SET: u32 = 0x101;
    pub const TRAINING_PATTERN_SET: u32 = 0x102;
    pub const TRAINING_LANE0_SET: u32 = 0x103;
    pub const TRAINING_LANE3_SET: u32 = 0x106;
    pub const DOWNSPREAD_CTRL: u32 = 0x107;
    pub const MAIN_LINK_CHANNEL_CODING_SET: u32 = 0x108;
    pub const FEC_CONFIGURATION: u32 = 0x120;
    pub const DSC_ENABLE: u32 = 0x160;
    pub const SINK_COUNT: u32 = 0x200;
    pub const DEVICE_SERVICE_IRQ_VECTOR: u32 = 0x201;
    pub const LANE0_1_STATUS: u32 = 0x202;
    pub const LANE2_3_STATUS: u32 = 0x203;
    pub const LANE_ALIGN_STATUS_UPDATED: u32 = 0x204;
    pub const SINK_STATUS: u32 = 0x205;
    pub const ADJUST_REQUEST_LANE0_1: u32 = 0x206;
    pub const ADJUST_REQUEST_LANE2_3: u32 = 0x207;
    pub const SOURCE_OUI: u32 = 0x300;
    pub const SET_POWER: u32 = 0x600;
    pub const SINK_COUNT_ESI: u32 = 0x2002;
    pub const DEVICE_SERVICE_IRQ_VECTOR_ESI0: u32 = 0x2003;
    pub const LANE0_1_STATUS_ESI: u32 = 0x200C;
    pub const SINK_STATUS_ESI: u32 = 0x200F;
    pub const DP13_DPCD_REV: u32 = 0x2200;
    /// Highest address the 20-bit AUX address field can carry.
    pub const ADDR_MAX: u32 = 0xF_FFFF;
}

/// Lane status bits (per lane nibble of 0x202/0x203).
pub const LANE_CR_DONE: u8 = 0x1;
pub const LANE_CHANNEL_EQ_DONE: u8 = 0x2;
pub const LANE_SYMBOL_LOCKED: u8 = 0x4;
/// 0x204 bit 0.
pub const INTERLANE_ALIGN_DONE: u8 = 0x01;
/// 0x204 bit 7: set by the sink when it lost the link on its own.
pub const LINK_STATUS_UPDATED: u8 = 0x80;
/// 0x205 bit 0.
pub const RECEIVE_PORT_0_STATUS: u8 = 0x01;
/// `SET_POWER` values.
pub const SET_POWER_D0: u8 = 0x1;
pub const SET_POWER_D3: u8 = 0x2;
pub const SET_POWER_D3_AUX_ON: u8 = 0x5;

/// The register file plus the link state it drives.
#[derive(Clone, Debug)]
pub struct Dpcd {
    profile: SinkProfile,
    state: LinkState,
    /// Configuration in effect while `Training` or `Trained`.
    active: Option<LinkConfig>,
    link_bw_set: u8,
    lane_count_set: u8,
    training_pattern_set: u8,
    training_lane_set: [u8; 4],
    downspread_ctrl: u8,
    channel_coding_set: u8,
    fec_config: u8,
    dsc_enable: u8,
    source_oui: [u8; 3],
    irq_vector: u8,
    set_power: u8,
    lane_status: [u8; 2],
    align_status: u8,
    sink_status: u8,
    adjust: [u8; 2],
}

impl Dpcd {
    /// A register file for `profile`, starting disconnected. The profile must validate.
    pub fn new(profile: SinkProfile) -> Self {
        let mut d = Dpcd {
            profile,
            state: LinkState::Disconnected,
            active: None,
            link_bw_set: 0,
            lane_count_set: 0,
            training_pattern_set: 0,
            training_lane_set: [0; 4],
            downspread_ctrl: 0,
            channel_coding_set: 0,
            fec_config: 0,
            dsc_enable: 0,
            source_oui: [0; 3],
            irq_vector: 0,
            set_power: SET_POWER_D0,
            lane_status: [0; 2],
            align_status: 0,
            sink_status: 0,
            adjust: [0; 2],
        };
        d.power_on_defaults();
        d
    }

    pub fn profile(&self) -> &SinkProfile {
        &self.profile
    }

    pub fn state(&self) -> LinkState {
        self.state
    }

    /// The configuration being trained or in use; `None` unless `Training` or `Trained`.
    pub fn active_config(&self) -> Option<LinkConfig> {
        match self.state {
            LinkState::Training(_) | LinkState::Trained => self.active,
            _ => None,
        }
    }

    fn power_on_defaults(&mut self) {
        self.active = None;
        self.link_bw_set = 0;
        self.lane_count_set = 0;
        self.training_pattern_set = 0;
        self.training_lane_set = [0; 4];
        self.downspread_ctrl = 0;
        self.channel_coding_set = 0;
        self.fec_config = 0;
        self.dsc_enable = 0;
        self.source_oui = [0; 3];
        self.irq_vector = 0;
        self.set_power = SET_POWER_D0;
        self.clear_status();
        self.align_status = 0;
    }

    /// HPD went high: power-on register values, link untrained.
    pub fn connect(&mut self) {
        self.power_on_defaults();
        self.state = LinkState::Connected;
    }

    /// HPD went low.
    pub fn disconnect(&mut self) {
        self.power_on_defaults();
        self.state = LinkState::Disconnected;
    }

    /// The sink lost a trained (or training) link on its own. Returns true when this
    /// changed the link state, in which case the caller raises an HPD IRQ.
    pub fn link_lost(&mut self) -> bool {
        match self.state {
            LinkState::Training(_) | LinkState::Trained => {
                self.state = LinkState::Connected;
                self.active = None;
                self.clear_status();
                self.align_status |= LINK_STATUS_UPDATED;
                true
            }
            _ => false,
        }
    }

    /// Set bits in `DEVICE_SERVICE_IRQ_VECTOR` (0x201); the source clears them by writing 1.
    pub fn raise_service_irq(&mut self, bits: u8) {
        self.irq_vector |= bits;
    }

    fn clear_status(&mut self) {
        self.lane_status = [0; 2];
        self.align_status &= LINK_STATUS_UPDATED;
        self.sink_status = 0;
        self.adjust = [0; 2];
    }

    fn caps_byte(&self, off: u32) -> u8 {
        let p = &self.profile;
        match off {
            0x0 => p.dpcd_rev,
            0x1 => p.max_rate.code(),
            0x2 => {
                p.max_lanes
                    | if p.tps3 { 0x40 } else { 0 }
                    | if p.enhanced_framing { 0x80 } else { 0 }
            }
            0x3 => u8::from(p.downspread) | if p.tps4 { 0x80 } else { 0 },
            // NORP: one receiver port (value is count minus one), no power-control caps.
            0x4 => 0,
            // No branch device, no downstream ports.
            0x5 => 0,
            // ANSI 8b/10b.
            0x6 => 0x01,
            0x7 => 0,
            // LOCAL_EDID_PRESENT.
            0x8 => 0x02,
            0x9 => p.port0_buffer,
            // EXTENDED_RECEIVER_CAP_FIELD_PRESENT from DPCD 1.4 on; 0x2200 mirrors 0x000.
            0xE => {
                if p.dpcd_rev >= 0x14 {
                    0x80
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    /// True if `addr` holds state the source can change (for documentation and tests).
    pub fn is_writable(&self, addr: u32) -> bool {
        let p = &self.profile;
        match addr {
            reg::LINK_BW_SET..=reg::MAIN_LINK_CHANNEL_CODING_SET => true,
            reg::FEC_CONFIGURATION => p.fec,
            reg::DSC_ENABLE => p.dsc.is_some(),
            reg::DEVICE_SERVICE_IRQ_VECTOR => true,
            reg::DEVICE_SERVICE_IRQ_VECTOR_ESI0 => p.dpcd_rev >= 0x12,
            0x300..=0x302 => true,
            reg::SET_POWER => true,
            _ => false,
        }
    }

    /// Read one byte. Never fails: undefined addresses read 0.
    pub fn read_byte(&self, addr: u32) -> u8 {
        let p = &self.profile;
        match addr {
            0x000..=0x00F => self.caps_byte(addr),
            0x060..=0x06F => p.dsc.map_or(0, |d| d.regs[(addr - 0x060) as usize]),
            reg::FEC_CAPABILITY => u8::from(p.fec),
            reg::LINK_BW_SET => self.link_bw_set,
            reg::LANE_COUNT_SET => self.lane_count_set,
            reg::TRAINING_PATTERN_SET => self.training_pattern_set,
            0x103..=0x106 => self.training_lane_set[(addr - 0x103) as usize],
            reg::DOWNSPREAD_CTRL => self.downspread_ctrl,
            reg::MAIN_LINK_CHANNEL_CODING_SET => self.channel_coding_set,
            reg::FEC_CONFIGURATION if p.fec => self.fec_config,
            reg::DSC_ENABLE if p.dsc.is_some() => self.dsc_enable,
            reg::SINK_COUNT => u8::from(self.state != LinkState::Disconnected),
            reg::DEVICE_SERVICE_IRQ_VECTOR => self.irq_vector,
            reg::LANE0_1_STATUS => self.lane_status[0],
            reg::LANE2_3_STATUS => self.lane_status[1],
            reg::LANE_ALIGN_STATUS_UPDATED => self.align_status,
            reg::SINK_STATUS => self.sink_status,
            reg::ADJUST_REQUEST_LANE0_1 => self.adjust[0],
            reg::ADJUST_REQUEST_LANE2_3 => self.adjust[1],
            0x300..=0x302 => self.source_oui[(addr - 0x300) as usize],
            reg::SET_POWER => self.set_power,
            // Event status indicator mirrors (DPCD 1.2).
            reg::SINK_COUNT_ESI if p.dpcd_rev >= 0x12 => self.read_byte(reg::SINK_COUNT),
            reg::DEVICE_SERVICE_IRQ_VECTOR_ESI0 if p.dpcd_rev >= 0x12 => self.irq_vector,
            reg::LANE0_1_STATUS_ESI..=reg::SINK_STATUS_ESI if p.dpcd_rev >= 0x12 => {
                self.read_byte(addr - reg::LANE0_1_STATUS_ESI + reg::LANE0_1_STATUS)
            }
            // Extended receiver capability field (DPCD 1.4).
            0x2200..=0x220F if p.dpcd_rev >= 0x14 => self.caps_byte(addr - 0x2200),
            _ => 0,
        }
    }

    /// Read `buf.len()` bytes from `addr`. The caller has checked the range.
    pub fn read(&self, addr: u32, buf: &mut [u8]) {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = self.read_byte(addr + i as u32);
        }
    }

    /// Write `data` at `addr`, then apply the side effects once for the whole burst (a
    /// source may write 0x102-0x106 in one transaction). The caller has checked the range.
    pub fn write(&mut self, addr: u32, data: &[u8]) {
        let before = (self.link_bw_set, self.lane_count_set);
        let mut link_touched = false;
        let mut lanes_touched = false;
        let mut power_touched = false;
        for (i, &v) in data.iter().enumerate() {
            let a = addr + i as u32;
            match a {
                reg::LINK_BW_SET => {
                    self.link_bw_set = v;
                    link_touched = true;
                }
                reg::LANE_COUNT_SET => {
                    self.lane_count_set = v & 0x9F;
                    link_touched = true;
                }
                reg::TRAINING_PATTERN_SET => {
                    self.training_pattern_set = v;
                    link_touched = true;
                }
                0x103..=0x106 => {
                    self.training_lane_set[(a - 0x103) as usize] = v & 0x3F;
                    lanes_touched = true;
                }
                reg::DOWNSPREAD_CTRL => self.downspread_ctrl = v & 0x90,
                reg::MAIN_LINK_CHANNEL_CODING_SET => self.channel_coding_set = v & 0x01,
                reg::FEC_CONFIGURATION if self.profile.fec => self.fec_config = v & 0x01,
                reg::DSC_ENABLE if self.profile.dsc.is_some() => self.dsc_enable = v & 0x01,
                reg::DEVICE_SERVICE_IRQ_VECTOR => self.irq_vector &= !v,
                reg::DEVICE_SERVICE_IRQ_VECTOR_ESI0 if self.profile.dpcd_rev >= 0x12 => {
                    self.irq_vector &= !v
                }
                0x300..=0x302 => self.source_oui[(a - 0x300) as usize] = v,
                reg::SET_POWER => {
                    let (mask, d3_aux_on) = if self.profile.dpcd_rev >= 0x14 {
                        (0x07, true)
                    } else {
                        (0x03, false)
                    };
                    let val = v & mask;
                    // Reserved power states are ignored (the previous state stays).
                    if val == SET_POWER_D0
                        || val == SET_POWER_D3
                        || (d3_aux_on && val == SET_POWER_D3_AUX_ON)
                    {
                        self.set_power = val;
                        power_touched = true;
                    }
                }
                // Read-only, reserved or undefined: acknowledged, ignored.
                _ => {}
            }
        }
        let config_changed = before != (self.link_bw_set, self.lane_count_set);
        self.evaluate(link_touched, config_changed, lanes_touched, power_touched);
    }

    fn pattern(&self) -> Result<Option<TrainingPattern>, LinkError> {
        let p = &self.profile;
        let code = if p.dpcd_rev >= 0x14 {
            self.training_pattern_set & 0x0F
        } else {
            self.training_pattern_set & 0x03
        };
        match code {
            0 => Ok(None),
            1 => Ok(Some(TrainingPattern::Tps1)),
            2 => Ok(Some(TrainingPattern::Tps2)),
            3 if p.tps3 => Ok(Some(TrainingPattern::Tps3)),
            7 if p.tps4 => Ok(Some(TrainingPattern::Tps4)),
            other => Err(LinkError::UnsupportedPattern(other)),
        }
    }

    fn lane_nibbles(lanes: u8, nibble: u8) -> [u8; 2] {
        let mut s = [0u8; 2];
        for i in 0..lanes as usize {
            s[i / 2] |= nibble << ((i % 2) * 4);
        }
        s
    }

    fn mirror_adjust(&mut self, lanes: u8) {
        let mut a = [0u8; 2];
        for i in 0..lanes as usize {
            let set = self.training_lane_set[i];
            let swing = set & 0x3;
            let pre = (set >> 3) & 0x3;
            a[i / 2] |= (swing | pre << 2) << ((i % 2) * 4);
        }
        self.adjust = a;
    }

    fn refuse(&mut self, e: LinkError) {
        self.state = LinkState::Refused(e);
        self.active = None;
        self.clear_status();
    }

    fn evaluate(&mut self, link: bool, config_changed: bool, lanes: bool, power: bool) {
        if self.state == LinkState::Disconnected {
            return;
        }
        if self.set_power != SET_POWER_D0 {
            if power && self.state != LinkState::Connected {
                self.state = LinkState::Connected;
                self.active = None;
                self.clear_status();
            }
            if link && self.pattern().ok() != Some(None) {
                // Training while powered down: refused until the source sets D0.
                self.refuse(LinkError::NotPowered);
            }
            return;
        }
        if config_changed && self.state == LinkState::Trained {
            // The source reconfigured a trained link: it is no longer trained.
            self.state = LinkState::Connected;
            self.active = None;
            self.clear_status();
        }
        if !(link || power) {
            if lanes {
                if let LinkState::Training(_) = self.state {
                    let n = self.active.map_or(0, |c| c.lanes);
                    self.mirror_adjust(n);
                }
            }
            return;
        }
        let pattern = match self.pattern() {
            Ok(p) => p,
            Err(e) => return self.refuse(e),
        };
        let Some(pattern) = pattern else {
            match self.state {
                LinkState::Training(p) if p != TrainingPattern::Tps1 => {
                    self.state = LinkState::Trained;
                    self.sink_status = RECEIVE_PORT_0_STATUS;
                    self.adjust = [0; 2];
                }
                LinkState::Training(_) | LinkState::Refused(_) => {
                    self.state = LinkState::Connected;
                    self.active = None;
                    self.clear_status();
                }
                _ => {}
            }
            return;
        };
        let cfg = match LinkConfig::from_bytes(&self.profile, self.link_bw_set, self.lane_count_set)
        {
            Ok(c) => c,
            Err(e) => return self.refuse(e),
        };
        match pattern {
            TrainingPattern::Tps1 => {
                self.state = LinkState::Training(TrainingPattern::Tps1);
                self.active = Some(cfg);
                self.align_status = 0;
                self.sink_status = 0;
                self.lane_status = Self::lane_nibbles(cfg.lanes, LANE_CR_DONE);
                self.mirror_adjust(cfg.lanes);
            }
            eq => {
                let cr_done =
                    matches!(self.state, LinkState::Training(_)) && self.active == Some(cfg);
                if !cr_done {
                    return self.refuse(LinkError::ClockRecoveryNotDone);
                }
                self.state = LinkState::Training(eq);
                self.lane_status = Self::lane_nibbles(
                    cfg.lanes,
                    LANE_CR_DONE | LANE_CHANNEL_EQ_DONE | LANE_SYMBOL_LOCKED,
                );
                self.align_status = INTERLANE_ALIGN_DONE;
                self.mirror_adjust(cfg.lanes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::LinkRate;

    fn connected(p: SinkProfile) -> Dpcd {
        let mut d = Dpcd::new(p);
        d.connect();
        d
    }

    fn bytes(d: &Dpcd, addr: u32, n: usize) -> Vec<u8> {
        let mut v = vec![0; n];
        d.read(addr, &mut v);
        v
    }

    #[test]
    fn receiver_caps_of_minimal_profile() {
        let d = connected(SinkProfile::dp11a_hbr_x4());
        assert_eq!(
            bytes(&d, 0, 16),
            [0x11, 0x0A, 0x84, 0x01, 0, 0, 0x01, 0, 0x02, 0, 0, 0, 0, 0, 0, 0]
        );
        // No MST, no DSC, no FEC, no extended caps, no ESI mirror at 1.1.
        assert_eq!(d.read_byte(reg::MSTM_CAP), 0);
        assert_eq!(bytes(&d, reg::DSC_SUPPORT, 16), [0; 16]);
        assert_eq!(d.read_byte(reg::FEC_CAPABILITY), 0);
        assert_eq!(bytes(&d, reg::DP13_DPCD_REV, 16), [0; 16]);
        assert_eq!(d.read_byte(reg::SINK_COUNT_ESI), 0);
        assert_eq!(d.read_byte(reg::SINK_COUNT), 1);
        assert_eq!(d.read_byte(reg::SET_POWER), SET_POWER_D0);
    }

    #[test]
    fn headroom_profile_is_a_data_change() {
        let d = connected(SinkProfile::dp14_hbr3_x4_dsc());
        assert_eq!(
            bytes(&d, 0, 4),
            [0x14, 0x1E, 0xC4, 0x81],
            "rev 1.4, HBR3, EF+TPS3+4 lanes, TPS4+downspread"
        );
        assert_eq!(d.read_byte(reg::TRAINING_AUX_RD_INTERVAL), 0x80);
        assert_eq!(bytes(&d, reg::DP13_DPCD_REV, 16), bytes(&d, 0, 16));
        assert_eq!(d.read_byte(reg::DSC_SUPPORT), 0x01);
        assert_eq!(d.read_byte(reg::FEC_CAPABILITY), 0x01);
        assert_eq!(d.read_byte(reg::SINK_COUNT_ESI), 1);
    }

    #[test]
    fn undefined_reads_zero_and_read_only_writes_are_ignored() {
        let mut d = connected(SinkProfile::dp11a_hbr_x4());
        assert_eq!(d.read_byte(0x0400), 0);
        assert_eq!(d.read_byte(0x7_0000), 0);
        assert_eq!(d.read_byte(reg::ADDR_MAX), 0);
        let caps = bytes(&d, 0, 16);
        d.write(0, &[0xFF; 16]);
        assert_eq!(bytes(&d, 0, 16), caps);
        d.write(reg::LANE0_1_STATUS, &[0xFF; 6]);
        assert_eq!(bytes(&d, reg::LANE0_1_STATUS, 6), [0; 6]);
        d.write(0x0400, &[0x55]);
        assert_eq!(d.read_byte(0x0400), 0);
        // DSC enable does not exist on the minimal profile.
        d.write(reg::DSC_ENABLE, &[1]);
        assert_eq!(d.read_byte(reg::DSC_ENABLE), 0);
        assert!(!d.is_writable(reg::DSC_ENABLE));
        assert!(!d.is_writable(reg::DPCD_REV));
        assert!(d.is_writable(reg::LINK_BW_SET));
    }

    #[test]
    fn writable_registers_keep_defined_bits_only() {
        let mut d = connected(SinkProfile::dp11a_hbr_x4());
        d.write(reg::LANE_COUNT_SET, &[0xFF]);
        assert_eq!(d.read_byte(reg::LANE_COUNT_SET), 0x9F);
        d.write(reg::TRAINING_LANE0_SET, &[0xFF, 0x1B]);
        assert_eq!(bytes(&d, reg::TRAINING_LANE0_SET, 2), [0x3F, 0x1B]);
        d.write(reg::DOWNSPREAD_CTRL, &[0xFF, 0xFF]);
        assert_eq!(bytes(&d, reg::DOWNSPREAD_CTRL, 2), [0x90, 0x01]);
        d.write(reg::SOURCE_OUI, &[0x00, 0x04, 0x4B]);
        assert_eq!(bytes(&d, reg::SOURCE_OUI, 3), [0x00, 0x04, 0x4B]);
    }

    #[test]
    fn irq_vector_is_write_one_to_clear() {
        let mut d = connected(SinkProfile::dp11a_hbr_x4());
        d.raise_service_irq(0x06);
        assert_eq!(d.read_byte(reg::DEVICE_SERVICE_IRQ_VECTOR), 0x06);
        d.write(reg::DEVICE_SERVICE_IRQ_VECTOR, &[0x04]);
        assert_eq!(d.read_byte(reg::DEVICE_SERVICE_IRQ_VECTOR), 0x02);
        d.write(reg::DEVICE_SERVICE_IRQ_VECTOR, &[0x00]);
        assert_eq!(d.read_byte(reg::DEVICE_SERVICE_IRQ_VECTOR), 0x02);
    }

    fn train(d: &mut Dpcd, bw: u8, lanes: u8, eq: u8) {
        d.write(reg::LINK_BW_SET, &[bw, lanes]);
        d.write(reg::TRAINING_PATTERN_SET, &[0x21, 0x00, 0x00, 0x00, 0x00]);
        d.write(reg::TRAINING_PATTERN_SET, &[0x20 | eq]);
        d.write(reg::TRAINING_PATTERN_SET, &[0x00]);
    }

    #[test]
    fn training_sequence_four_lanes() {
        let mut d = connected(SinkProfile::dp11a_hbr_x4());
        assert_eq!(d.state(), LinkState::Connected);
        d.write(reg::LINK_BW_SET, &[0x0A, 0x84]);
        assert_eq!(d.state(), LinkState::Connected);
        d.write(reg::TRAINING_PATTERN_SET, &[0x21, 0x01, 0x01, 0x01, 0x01]);
        assert_eq!(d.state(), LinkState::Training(TrainingPattern::Tps1));
        assert_eq!(bytes(&d, 0x202, 6), [0x11, 0x11, 0x00, 0x00, 0x11, 0x11]);
        // Source raises swing and pre-emphasis on lane 2: adjust request follows.
        d.write(0x105, &[0x0A]);
        assert_eq!(bytes(&d, 0x206, 2), [0x11, 0x16]);
        d.write(reg::TRAINING_PATTERN_SET, &[0x22]);
        assert_eq!(d.state(), LinkState::Training(TrainingPattern::Tps2));
        assert_eq!(bytes(&d, 0x202, 4), [0x77, 0x77, 0x01, 0x00]);
        d.write(reg::TRAINING_PATTERN_SET, &[0x00]);
        assert_eq!(d.state(), LinkState::Trained);
        assert_eq!(bytes(&d, 0x202, 6), [0x77, 0x77, 0x01, 0x01, 0x00, 0x00]);
        assert_eq!(
            d.active_config(),
            Some(LinkConfig {
                rate: LinkRate::Hbr,
                lanes: 4,
                enhanced_framing: true
            })
        );
    }

    #[test]
    fn lane_status_matches_lane_count() {
        let mut d = connected(SinkProfile::dp11a_hbr_x4());
        for (lanes, s01, s23) in [(1u8, 0x07u8, 0u8), (2, 0x77, 0), (4, 0x77, 0x77)] {
            train(&mut d, 0x06, lanes, 2);
            assert_eq!(d.state(), LinkState::Trained, "{lanes} lanes");
            assert_eq!(bytes(&d, 0x202, 2), [s01, s23], "{lanes} lanes");
        }
    }

    #[test]
    fn invalid_configurations_are_refused() {
        let mut d = connected(SinkProfile::dp11a_hbr_x4());
        // HBR2 is above the profile's maximum.
        d.write(reg::LINK_BW_SET, &[0x14, 0x84]);
        d.write(reg::TRAINING_PATTERN_SET, &[0x21]);
        assert_eq!(
            d.state(),
            LinkState::Refused(LinkError::RateAboveMax(LinkRate::Hbr2))
        );
        assert_eq!(bytes(&d, 0x202, 6), [0; 6]);
        // Three lanes.
        d.write(reg::LINK_BW_SET, &[0x0A, 0x83]);
        d.write(reg::TRAINING_PATTERN_SET, &[0x21]);
        assert_eq!(d.state(), LinkState::Refused(LinkError::LaneCount(3)));
        // Pattern 0 leaves the refused state.
        d.write(reg::TRAINING_PATTERN_SET, &[0x00]);
        assert_eq!(d.state(), LinkState::Connected);
        // TPS3 is reserved at DPCD 1.1.
        d.write(reg::LINK_BW_SET, &[0x0A, 0x84]);
        d.write(reg::TRAINING_PATTERN_SET, &[0x21]);
        d.write(reg::TRAINING_PATTERN_SET, &[0x23]);
        assert_eq!(
            d.state(),
            LinkState::Refused(LinkError::UnsupportedPattern(3))
        );
        // Equalization without clock recovery.
        d.write(reg::TRAINING_PATTERN_SET, &[0x00]);
        d.write(reg::TRAINING_PATTERN_SET, &[0x22]);
        assert_eq!(
            d.state(),
            LinkState::Refused(LinkError::ClockRecoveryNotDone)
        );
        // Changing the lane count during equalization invalidates clock recovery.
        d.write(reg::TRAINING_PATTERN_SET, &[0x21]);
        d.write(reg::TRAINING_PATTERN_SET, &[0x22]);
        assert_eq!(d.state(), LinkState::Training(TrainingPattern::Tps2));
        d.write(reg::LANE_COUNT_SET, &[0x82]);
        assert_eq!(
            d.state(),
            LinkState::Refused(LinkError::ClockRecoveryNotDone)
        );
    }

    #[test]
    fn tps3_and_tps4_on_headroom_profile() {
        let mut d = connected(SinkProfile::dp14_hbr3_x4_dsc());
        train(&mut d, 0x1E, 0x84, 7);
        assert_eq!(d.state(), LinkState::Trained);
        train(&mut d, 0x14, 0x84, 3);
        assert_eq!(d.state(), LinkState::Trained);
        // Pattern code 5 is reserved even at 1.4.
        d.write(reg::TRAINING_PATTERN_SET, &[0x21]);
        d.write(reg::TRAINING_PATTERN_SET, &[0x25]);
        assert_eq!(
            d.state(),
            LinkState::Refused(LinkError::UnsupportedPattern(5))
        );
    }

    #[test]
    fn reconfiguring_or_powering_down_untrains() {
        let mut d = connected(SinkProfile::dp11a_hbr_x4());
        train(&mut d, 0x0A, 0x84, 2);
        // Writing the same configuration again keeps the link.
        d.write(reg::LINK_BW_SET, &[0x0A, 0x84]);
        assert_eq!(d.state(), LinkState::Trained);
        d.write(reg::LANE_COUNT_SET, &[0x82]);
        assert_eq!(d.state(), LinkState::Connected);
        assert_eq!(bytes(&d, 0x202, 4), [0; 4]);
        train(&mut d, 0x0A, 0x84, 2);
        d.write(reg::SET_POWER, &[SET_POWER_D3]);
        assert_eq!(d.state(), LinkState::Connected);
        d.write(reg::TRAINING_PATTERN_SET, &[0x21]);
        assert_eq!(d.state(), LinkState::Refused(LinkError::NotPowered));
        // Reserved power state is ignored; D0 allows training again.
        d.write(reg::SET_POWER, &[0x03]);
        assert_eq!(d.read_byte(reg::SET_POWER), SET_POWER_D3);
        d.write(reg::SET_POWER, &[SET_POWER_D0]);
        train(&mut d, 0x0A, 0x84, 2);
        assert_eq!(d.state(), LinkState::Trained);
    }

    #[test]
    fn link_loss_sets_link_status_updated() {
        let mut d = connected(SinkProfile::dp11a_hbr_x4());
        assert!(!d.link_lost());
        train(&mut d, 0x0A, 0x84, 2);
        assert!(d.link_lost());
        assert_eq!(d.state(), LinkState::Connected);
        assert_eq!(bytes(&d, 0x202, 4), [0, 0, LINK_STATUS_UPDATED, 0]);
        // A new training start clears it.
        train(&mut d, 0x0A, 0x84, 2);
        assert_eq!(
            d.read_byte(reg::LANE_ALIGN_STATUS_UPDATED),
            INTERLANE_ALIGN_DONE
        );
    }
}

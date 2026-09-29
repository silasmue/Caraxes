// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The sink profile: every capability the DPCD reports, as data.
//!
//! The default profile is the minimal one of the briefing (section 6): DPCD revision 1.1,
//! up to four lanes at HBR, SST only, no DSC, no FEC. Headroom for decision D15 (HBR2,
//! HBR3, TPS3/TPS4, DSC) is a different value of the same struct, never different code.

use std::fmt;

/// Main-link rate per lane, 8b/10b channel coding only (no UHBR).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LinkRate {
    /// 1.62 Gbit/s, LINK_BW code 0x06.
    Rbr,
    /// 2.7 Gbit/s, code 0x0A.
    Hbr,
    /// 5.4 Gbit/s, code 0x14 (DPCD 1.2).
    Hbr2,
    /// 8.1 Gbit/s, code 0x1E (DPCD 1.4).
    Hbr3,
}

impl LinkRate {
    /// All rates in ascending order.
    pub const ALL: [LinkRate; 4] = [LinkRate::Rbr, LinkRate::Hbr, LinkRate::Hbr2, LinkRate::Hbr3];

    /// The DPCD `MAX_LINK_RATE` / `LINK_BW_SET` code (link rate in units of 0.27 Gbit/s).
    pub const fn code(self) -> u8 {
        match self {
            LinkRate::Rbr => 0x06,
            LinkRate::Hbr => 0x0A,
            LinkRate::Hbr2 => 0x14,
            LinkRate::Hbr3 => 0x1E,
        }
    }

    /// Decode a `LINK_BW_SET` value; `None` for codes that name no 8b/10b rate.
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.code() == code)
    }

    /// Lane rate in Mbit/s (line rate, before 8b/10b).
    pub const fn mbps(self) -> u32 {
        self.code() as u32 * 270
    }

    /// The lowest DPCD revision that defines this rate.
    pub const fn min_dpcd_rev(self) -> u8 {
        match self {
            LinkRate::Rbr | LinkRate::Hbr => 0x10,
            LinkRate::Hbr2 => 0x12,
            LinkRate::Hbr3 => 0x14,
        }
    }
}

/// DSC capability block: the 16 bytes of DPCD 0x060-0x06F, verbatim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DscCaps {
    pub regs: [u8; 16],
}

/// Everything the sink reports about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SinkProfile {
    /// DPCD revision (0x000): 0x11 means DP 1.1/1.1a.
    pub dpcd_rev: u8,
    pub max_rate: LinkRate,
    /// 1, 2 or 4.
    pub max_lanes: u8,
    pub enhanced_framing: bool,
    /// Training pattern 3 (`MAX_LANE_COUNT` bit 6, DPCD 1.2).
    pub tps3: bool,
    /// Training pattern 4 (`MAX_DOWNSPREAD` bit 7, DPCD 1.4).
    pub tps4: bool,
    /// 0.5 % down-spread supported (`MAX_DOWNSPREAD` bit 0).
    pub downspread: bool,
    /// `RECEIVE_PORT0_CAP_1`: buffer size, (value + 1) * 32 bytes per lane.
    pub port0_buffer: u8,
    /// DSC decoder capabilities (DPCD 1.4). `None` reports DSC as absent.
    pub dsc: Option<DscCaps>,
    /// FEC capable (DPCD 0x090 bit 0, DPCD 1.4).
    pub fec: bool,
    /// Upper bound on the data bytes of one I2C-over-AUX read reply (1..=16). The DP
    /// specification lets an I2C read reply carry fewer bytes than requested; 16 never
    /// shortens a reply, smaller values exercise the source's partial-reply handling.
    pub max_i2c_reply: u8,
}

impl Default for SinkProfile {
    fn default() -> Self {
        Self::dp11a_hbr_x4()
    }
}

impl SinkProfile {
    /// The minimal first-monitor profile: DPCD 1.1, HBR, four lanes, enhanced framing.
    pub fn dp11a_hbr_x4() -> Self {
        SinkProfile {
            dpcd_rev: 0x11,
            max_rate: LinkRate::Hbr,
            max_lanes: 4,
            enhanced_framing: true,
            tps3: false,
            tps4: false,
            downspread: true,
            port0_buffer: 0,
            dsc: None,
            fec: false,
            max_i2c_reply: 16,
        }
    }

    /// Headroom profile for D15 (4K at high refresh): DPCD 1.4, HBR3 x4, TPS3/TPS4, FEC and
    /// a DSC 1.2 decoder. Data only: no NVIDIA guest has been run against it.
    pub fn dp14_hbr3_x4_dsc() -> Self {
        SinkProfile {
            dpcd_rev: 0x14,
            max_rate: LinkRate::Hbr3,
            max_lanes: 4,
            enhanced_framing: true,
            tps3: true,
            tps4: true,
            downspread: true,
            port0_buffer: 0,
            dsc: Some(DscCaps {
                // 0x060 decompression supported; 0x061 DSC 1.2; 0x062 RC block 1 KiB;
                // 0x063 RC buffer 6 blocks (6 KiB); 0x064 1, 2 and 4 slices; 0x065 line buffer
                // 8 bits; 0x066 block prediction; 0x067/0x068 max bpp 0 (not reported);
                // 0x069 RGB; 0x06A 8 and 10 bpc; 0x06B throughput 340 MP/s in modes 0 and 1;
                // 0x06C max slice width 8 * 320 = 2560; 0x06D none; 0x06E reserved;
                // 0x06F bpp increment 1/16.
                regs: [
                    0x01, 0x21, 0x00, 0x05, 0x0B, 0x08, 0x01, 0x00, 0x00, 0x01, 0x06, 0x11, 0x08,
                    0x00, 0x00, 0x00,
                ],
            }),
            fec: true,
            max_i2c_reply: 16,
        }
    }

    /// Check the profile for internal consistency (only report what can be answered).
    pub fn validate(&self) -> Result<(), ProfileError> {
        if !(0x10..=0x14).contains(&self.dpcd_rev) {
            return Err(ProfileError::DpcdRevision(self.dpcd_rev));
        }
        if !matches!(self.max_lanes, 1 | 2 | 4) {
            return Err(ProfileError::MaxLanes(self.max_lanes));
        }
        if self.max_rate.min_dpcd_rev() > self.dpcd_rev {
            return Err(ProfileError::RateNeedsRevision(self.max_rate));
        }
        if self.tps3 && self.dpcd_rev < 0x12 {
            return Err(ProfileError::FeatureNeedsRevision("TPS3", 0x12));
        }
        if self.tps4 && self.dpcd_rev < 0x14 {
            return Err(ProfileError::FeatureNeedsRevision("TPS4", 0x14));
        }
        if self.max_rate >= LinkRate::Hbr2 && !self.tps3 {
            return Err(ProfileError::RateNeedsPattern(self.max_rate, 3));
        }
        if self.max_rate >= LinkRate::Hbr3 && !self.tps4 {
            return Err(ProfileError::RateNeedsPattern(self.max_rate, 4));
        }
        if self.dsc.is_some() && self.dpcd_rev < 0x14 {
            return Err(ProfileError::FeatureNeedsRevision("DSC", 0x14));
        }
        if self.fec && self.dpcd_rev < 0x14 {
            return Err(ProfileError::FeatureNeedsRevision("FEC", 0x14));
        }
        if !(1..=16).contains(&self.max_i2c_reply) {
            return Err(ProfileError::MaxI2cReply(self.max_i2c_reply));
        }
        Ok(())
    }

    /// Highest training pattern the sink accepts for equalization (2, 3 or 4).
    pub fn best_eq_pattern(&self) -> u8 {
        if self.tps4 {
            4
        } else if self.tps3 {
            3
        } else {
            2
        }
    }
}

/// Why a profile was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileError {
    DpcdRevision(u8),
    MaxLanes(u8),
    RateNeedsRevision(LinkRate),
    RateNeedsPattern(LinkRate, u8),
    FeatureNeedsRevision(&'static str, u8),
    MaxI2cReply(u8),
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::DpcdRevision(r) => write!(f, "DPCD revision {r:#04x} not modelled"),
            ProfileError::MaxLanes(n) => write!(f, "max lane count {n} is not 1, 2 or 4"),
            ProfileError::RateNeedsRevision(r) => write!(
                f,
                "{r:?} needs DPCD revision {:#04x} or later",
                r.min_dpcd_rev()
            ),
            ProfileError::RateNeedsPattern(r, p) => {
                write!(f, "{r:?} requires training pattern {p} support")
            }
            ProfileError::FeatureNeedsRevision(what, rev) => {
                write!(f, "{what} needs DPCD revision {rev:#04x} or later")
            }
            ProfileError::MaxI2cReply(n) => write!(f, "max I2C reply {n} is not in 1..=16"),
        }
    }
}

impl std::error::Error for ProfileError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_profiles_validate() {
        SinkProfile::dp11a_hbr_x4().validate().unwrap();
        SinkProfile::dp14_hbr3_x4_dsc().validate().unwrap();
    }

    #[test]
    fn inconsistent_profiles_are_rejected() {
        let mut p = SinkProfile::dp11a_hbr_x4();
        p.max_rate = LinkRate::Hbr2;
        assert_eq!(
            p.validate(),
            Err(ProfileError::RateNeedsRevision(LinkRate::Hbr2))
        );
        let mut p = SinkProfile::dp11a_hbr_x4();
        p.max_lanes = 3;
        assert_eq!(p.validate(), Err(ProfileError::MaxLanes(3)));
        let mut p = SinkProfile::dp11a_hbr_x4();
        p.dsc = SinkProfile::dp14_hbr3_x4_dsc().dsc;
        assert!(matches!(
            p.validate(),
            Err(ProfileError::FeatureNeedsRevision("DSC", 0x14))
        ));
        let mut p = SinkProfile::dp14_hbr3_x4_dsc();
        p.tps4 = false;
        assert_eq!(
            p.validate(),
            Err(ProfileError::RateNeedsPattern(LinkRate::Hbr3, 4))
        );
    }

    #[test]
    fn rate_codes_round_trip() {
        for r in LinkRate::ALL {
            assert_eq!(LinkRate::from_code(r.code()), Some(r));
        }
        assert_eq!(LinkRate::from_code(0x0B), None);
        assert_eq!(LinkRate::Hbr.mbps(), 2700);
        assert_eq!(LinkRate::Hbr3.mbps(), 8100);
    }
}

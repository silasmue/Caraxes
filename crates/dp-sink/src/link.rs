// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Link configuration, link state and the reasons a configuration is refused.
//!
//! The transitions themselves live in [`crate::dpcd::Dpcd`], because they are side effects
//! of DPCD writes (0x100-0x106, 0x600). This module holds the vocabulary and the pure
//! validation rules.

use crate::profile::{LinkRate, SinkProfile};
use std::fmt;

/// A main-link configuration as the source writes it to 0x100/0x101.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkConfig {
    pub rate: LinkRate,
    /// 1, 2 or 4.
    pub lanes: u8,
    pub enhanced_framing: bool,
}

impl LinkConfig {
    /// The `LINK_BW_SET` byte.
    pub fn bw_byte(&self) -> u8 {
        self.rate.code()
    }

    /// The `LANE_COUNT_SET` byte.
    pub fn lane_byte(&self) -> u8 {
        self.lanes | if self.enhanced_framing { 0x80 } else { 0 }
    }

    /// Payload bandwidth in kbit/s after 8b/10b (what a stream may use).
    pub fn payload_kbps(&self) -> u64 {
        self.rate.mbps() as u64 * 1000 * self.lanes as u64 * 8 / 10
    }

    /// Validate raw 0x100/0x101 bytes against the profile.
    pub fn from_bytes(profile: &SinkProfile, bw: u8, lane: u8) -> Result<Self, LinkError> {
        let rate = LinkRate::from_code(bw).ok_or(LinkError::UnknownRate(bw))?;
        if rate > profile.max_rate {
            return Err(LinkError::RateAboveMax(rate));
        }
        let lanes = lane & 0x1F;
        if !matches!(lanes, 1 | 2 | 4) || lanes > profile.max_lanes {
            return Err(LinkError::LaneCount(lanes));
        }
        let enhanced_framing = lane & 0x80 != 0;
        if enhanced_framing && !profile.enhanced_framing {
            return Err(LinkError::EnhancedFramingUnsupported);
        }
        Ok(LinkConfig {
            rate,
            lanes,
            enhanced_framing,
        })
    }
}

/// Training pattern currently selected in `TRAINING_PATTERN_SET`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainingPattern {
    /// Clock recovery.
    Tps1,
    /// Channel equalization (all revisions).
    Tps2,
    /// Channel equalization, DPCD 1.2 and later, required for HBR2.
    Tps3,
    /// Channel equalization, DPCD 1.4, required for HBR3.
    Tps4,
}

impl TrainingPattern {
    pub fn number(self) -> u8 {
        match self {
            TrainingPattern::Tps1 => 1,
            TrainingPattern::Tps2 => 2,
            TrainingPattern::Tps3 => 3,
            TrainingPattern::Tps4 => 4,
        }
    }
}

/// The sink-side link state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkState {
    /// HPD low: nothing answers on AUX.
    Disconnected,
    /// HPD high, link not trained (after plug, after a training abort, after link loss,
    /// or while in a power-down state).
    Connected,
    /// A training pattern is set and the sink reports the matching lane status.
    Training(TrainingPattern),
    /// Training finished (pattern 0 after a successful equalization phase).
    Trained,
    /// A training pattern was set with a configuration the sink refuses. The lane status
    /// stays zero until the source sets pattern 0 (back to `Connected`) or retries with a
    /// valid configuration.
    Refused(LinkError),
}

/// Why a link configuration or training step was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkError {
    /// `LINK_BW_SET` holds a value that names no 8b/10b link rate.
    UnknownRate(u8),
    /// The rate exists but exceeds `MAX_LINK_RATE`.
    RateAboveMax(LinkRate),
    /// Lane count is not 1, 2 or 4, or exceeds `MAX_LANE_COUNT`.
    LaneCount(u8),
    /// Enhanced framing requested but not offered.
    EnhancedFramingUnsupported,
    /// Pattern value is reserved at this DPCD revision or not offered by the profile.
    UnsupportedPattern(u8),
    /// An equalization pattern was set before clock recovery succeeded with the same
    /// configuration.
    ClockRecoveryNotDone,
    /// Training attempted while `SET_POWER` is not D0.
    NotPowered,
    /// The sink is unplugged.
    Disconnected,
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::UnknownRate(c) => write!(f, "LINK_BW_SET {c:#04x} names no link rate"),
            LinkError::RateAboveMax(r) => write!(f, "{r:?} exceeds the sink's maximum"),
            LinkError::LaneCount(n) => write!(f, "lane count {n} not supported"),
            LinkError::EnhancedFramingUnsupported => {
                write!(f, "enhanced framing not supported")
            }
            LinkError::UnsupportedPattern(p) => write!(f, "training pattern {p} not supported"),
            LinkError::ClockRecoveryNotDone => {
                write!(f, "equalization requested before clock recovery")
            }
            LinkError::NotPowered => write!(f, "sink is not in D0"),
            LinkError::Disconnected => write!(f, "sink is disconnected"),
        }
    }
}

impl std::error::Error for LinkError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_validation_follows_profile() {
        let p = SinkProfile::dp11a_hbr_x4();
        assert_eq!(
            LinkConfig::from_bytes(&p, 0x0A, 0x84),
            Ok(LinkConfig {
                rate: LinkRate::Hbr,
                lanes: 4,
                enhanced_framing: true
            })
        );
        assert_eq!(
            LinkConfig::from_bytes(&p, 0x14, 0x04),
            Err(LinkError::RateAboveMax(LinkRate::Hbr2))
        );
        assert_eq!(
            LinkConfig::from_bytes(&p, 0x0B, 0x04),
            Err(LinkError::UnknownRate(0x0B))
        );
        assert_eq!(
            LinkConfig::from_bytes(&p, 0x06, 0x03),
            Err(LinkError::LaneCount(3))
        );
        assert_eq!(
            LinkConfig::from_bytes(&p, 0x06, 0x00),
            Err(LinkError::LaneCount(0))
        );
        let mut one = p.clone();
        one.max_lanes = 1;
        one.enhanced_framing = false;
        assert_eq!(
            LinkConfig::from_bytes(&one, 0x06, 0x02),
            Err(LinkError::LaneCount(2))
        );
        assert_eq!(
            LinkConfig::from_bytes(&one, 0x06, 0x81),
            Err(LinkError::EnhancedFramingUnsupported)
        );
    }

    #[test]
    fn payload_bandwidth() {
        let c = LinkConfig {
            rate: LinkRate::Hbr,
            lanes: 4,
            enhanced_framing: true,
        };
        // 4 x 2.7 Gbit/s x 0.8 = 8.64 Gbit/s; 1080p60 CVT-RB at 24 bpp needs 3.324 Gbit/s.
        assert_eq!(c.payload_kbps(), 8_640_000);
    }
}
